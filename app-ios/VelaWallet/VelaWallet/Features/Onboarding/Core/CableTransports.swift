//
//  CableTransports.swift
//  VelaWallet
//
//  The two caBLE data channels, as one message-oriented connection — a port of
//  the founder's proven demo (apppasskeysdemo-ios/CableConn.swift). Everything
//  the frames MEAN (Noise, the transport cipher, CTAP) lives in vela-core
//  behind CableFramePort; these classes move bytes and nothing else.
//
//   - L2capCableConn     — CTAP 2.3 local BLE (L2CAP CoC), 4-byte big-endian
//     length prefix per message. No tunnel server, no internet.
//   - WebSocketCableConn — CTAP 2.2 WebSocket tunnel, one binary WS frame per
//     message, subprotocol fido.cable.
//
//  Everything lives on the MainActor: the L2CAP streams are scheduled on
//  RunLoop.main and the URLSession delegate queue is .main, so all callbacks
//  land on the main thread — one serialization domain for the read buffer and
//  the continuation queue. The Rust ceremony runs on a BACKGROUND thread and
//  its CableFramePort calls are synchronous; CableConnPort bridges the two
//  worlds with a semaphore per call (safe precisely because the ceremony
//  thread is never the main thread — the SmartCard path set that precedent).
//

import Foundation
import CoreBluetooth
import os
import VelaCore

nonisolated enum CableConnError: Error, CustomStringConvertible {
    case closed
    case badLength(Int)
    case connectFailed(String)
    case timeout
    var description: String {
        switch self {
        case .closed: return "the channel closed"
        case let .badLength(n): return "bad frame length \(n)"
        case let .connectFailed(m): return "connect failed: \(m)"
        case .timeout: return "timed out"
        }
    }
}

@MainActor
protocol CableConn: AnyObject {
    var channel: String { get }
    func writeFrame(_ bytes: Data) throws
    func readFrame() async throws -> Data
    func close()
}

/// A [CableConn] as the Rust core's frame port. The core calls these from its
/// blocking ceremony thread; each call hops to the MainActor and blocks on a
/// semaphore until the actor answers.
final class CableConnPort: CableFramePort, @unchecked Sendable {
    private let conn: CableConn
    private let name: String

    @MainActor
    init(_ conn: CableConn) {
        self.conn = conn
        self.name = conn.channel
    }

    func writeFrame(frame: Data) -> String? {
        let semaphore = DispatchSemaphore(value: 0)
        var failure: String?
        Task { @MainActor [conn] in
            do { try conn.writeFrame(frame) } catch { failure = "\(error)" }
            semaphore.signal()
        }
        semaphore.wait()
        if let failure { print("[vela-cable] → write FAILED via \(name): \(failure)") }
        return failure
    }

    func readFrame() -> CableFrameOutcome {
        let semaphore = DispatchSemaphore(value: 0)
        var outcome: CableFrameOutcome = .timedOut
        Task { @MainActor [conn] in
            do {
                outcome = .frame(bytes: try await conn.readFrame())
            } catch CableConnError.timeout {
                outcome = .timedOut
            } catch {
                outcome = .failed(detail: "\(error)")
            }
            semaphore.signal()
        }
        semaphore.wait()
        switch outcome {
        case .frame: break // proven path; the stage lines narrate enough
        case .timedOut: print("[vela-cable] ← read timed out via \(name)")
        case let .failed(detail): print("[vela-cable] ← read FAILED via \(name): \(detail)")
        }
        return outcome
    }

    func channel() -> String { name }

    /// Fire-and-forget: called from the ceremony's `defer`, whose executor must
    /// not block on the main actor just to close a socket.
    func close() {
        Task { @MainActor [conn] in conn.close() }
    }
}

/// BLE L2CAP CoC. Each message = 4-byte big-endian length prefix + payload
/// (the same framing every other Vela client speaks on this channel).
@MainActor
final class L2capCableConn: NSObject, CableConn {
    nonisolated let channel = "L2CAP"

    /// Held so CoreBluetooth keeps the connection alive for the port's
    /// lifetime — the streams do NOT retain their channel, and a deallocated
    /// CBL2CAPChannel tears the link down. Without this the channel died
    /// ~150ms after opening, before the first Noise byte ("connection closed
    /// by client" on the authenticator; device-found 2026-08-28, two-sided
    /// logs). The desktop's Rust port keeps the identical reference for the
    /// identical reason.
    private let l2capChannel: CBL2CAPChannel
    /// Same story one level up: CoreBluetooth cancels the connection when the
    /// last strong reference to the peripheral goes away.
    private let peripheral: CBPeripheral?

    private let input: InputStream
    private let output: OutputStream
    private var rxBuffer = Data()
    private var pendingReads: [CheckedContinuation<Data, Error>] = []
    private var writeQueue = Data()
    private var closed = false

    private static let maxFrame = 1 << 20

    init(_ ch: CBL2CAPChannel, peripheral: CBPeripheral? = nil) {
        self.l2capChannel = ch
        self.peripheral = peripheral
        self.input = ch.inputStream
        self.output = ch.outputStream
        super.init()
        for s in [input, output] {
            s.delegate = self
            s.schedule(in: .main, forMode: .default)
            s.open()
        }
    }

    func writeFrame(_ bytes: Data) throws {
        guard !closed else { throw CableConnError.closed }
        var header = Data(count: 4)
        let n = UInt32(bytes.count)
        header[0] = UInt8((n >> 24) & 0xff)
        header[1] = UInt8((n >> 16) & 0xff)
        header[2] = UInt8((n >> 8) & 0xff)
        header[3] = UInt8(n & 0xff)
        writeQueue.append(header)
        writeQueue.append(bytes)
        pumpWrite()
    }

    func readFrame() async throws -> Data {
        // A read must not outlive the ceremony budget (the person approving on
        // the other phone): a stream that never delivers would otherwise pin
        // the Rust thread forever. The watchdog fails ALL pending reads; a
        // frame that arrives first cancels it via the normal resume path.
        let watchdog = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: 130_000_000_000)
            // Load-bearing: cancelling a sleeping Task makes the sleep THROW
            // IMMEDIATELY, and `try?` swallows that — without this check the
            // "cancelled" watchdog fell through and killed the connection the
            // instant a frame arrived. That was the whole BLE-only failure:
            // msg2 read fine, the deferred cancel fired the watchdog, and the
            // very next read found the channel "closed" (device-found
            // 2026-08-28, two-sided logs, 51ms from success to teardown).
            guard !Task.isCancelled else { return }
            guard let self, !self.closed else { return }
            self.fail(CableConnError.timeout)
        }
        defer { watchdog.cancel() }
        return try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Data, Error>) in
            if closed { cont.resume(throwing: CableConnError.closed); return }
            pendingReads.append(cont)
            drainFrames()
        }
    }

    private func handleEvent(_ e: Stream.Event) {
        switch e {
        case .hasBytesAvailable:
            var tmp = [UInt8](repeating: 0, count: 4096)
            let read = input.read(&tmp, maxLength: tmp.count)
            if read > 0 { rxBuffer.append(contentsOf: tmp[0..<read]); drainFrames() }
            else if read < 0 { fail(CableConnError.closed) }
        case .hasSpaceAvailable:
            pumpWrite()
        case .errorOccurred, .endEncountered:
            fail(CableConnError.closed)
        default:
            break
        }
    }

    private func drainFrames() {
        while !pendingReads.isEmpty, rxBuffer.count >= 4 {
            let b = [UInt8](rxBuffer.prefix(4))
            let len = (Int(b[0]) << 24) | (Int(b[1]) << 16) | (Int(b[2]) << 8) | Int(b[3])
            guard len >= 0, len <= Self.maxFrame else { fail(CableConnError.badLength(len)); return }
            guard rxBuffer.count >= 4 + len else { break }
            let payload = Data(rxBuffer.subdata(in: 4..<(4 + len)))
            rxBuffer.removeSubrange(0..<(4 + len))
            pendingReads.removeFirst().resume(returning: payload)
        }
    }

    private func pumpWrite() {
        // Attempt the write directly rather than gating on hasSpaceAvailable:
        // that property can report false when a write would succeed, and the
        // one-shot event fired right after open() is wasted if the queue was
        // still empty then. 0 (buffer full) or <0 (not open yet) leaves the
        // bytes queued for the next .hasSpaceAvailable event.
        while !writeQueue.isEmpty {
            let n = writeQueue.count
            let wrote = writeQueue.withUnsafeBytes { raw -> Int in
                guard let base = raw.bindMemory(to: UInt8.self).baseAddress else { return 0 }
                return output.write(base, maxLength: n)
            }
            if wrote > 0 { writeQueue.removeSubrange(0..<wrote) } else { break }
        }
    }

    private func fail(_ err: Error) {
        guard !closed else { return }
        closed = true
        let waiting = pendingReads
        pendingReads.removeAll()
        for c in waiting { c.resume(throwing: err) }
    }

    func close() {
        guard !closed else { return }
        closed = true
        for s in [input, output] {
            s.close()
            s.remove(from: .main, forMode: .default)
        }
        let waiting = pendingReads
        pendingReads.removeAll()
        for c in waiting { c.resume(throwing: CableConnError.closed) }
    }
}

extension L2capCableConn: StreamDelegate {
    // Streams are scheduled on RunLoop.main → this always fires on the main thread.
    nonisolated func stream(_ s: Stream, handle e: Stream.Event) {
        MainActor.assumeIsolated { self.handleEvent(e) }
    }
}

/// WebSocket tunnel: one binary WS frame per message, subprotocol fido.cable.
@MainActor
final class WebSocketCableConn: NSObject, CableConn {
    nonisolated let channel = "WebSocket"
    private var task: URLSessionWebSocketTask!
    private var session: URLSession!
    private var openCont: CheckedContinuation<Void, Error>?
    private var sendError: Error?

    static let subprotocol = "fido.cable"

    static func connect(url: URL, timeoutMs: Int) async throws -> WebSocketCableConn {
        let c = WebSocketCableConn()
        let cfg = URLSessionConfiguration.default
        cfg.timeoutIntervalForRequest = TimeInterval(timeoutMs) / 1000
        // delegateQueue = .main so the delegate callbacks land on the MainActor.
        c.session = URLSession.vela(cfg, delegate: c, delegateQueue: .main)
        c.task = c.session.webSocketTask(with: url, protocols: [subprotocol])
        print("[vela-cable] opening tunnel: \(url)")

        // A watchdog that resumes the SAME continuation, not a task-group race:
        // a group would wait for the continuation child to finish, and a
        // continuation ignores cancellation — a tunnel that never answered
        // would have hung the ceremony forever exactly where a timeout was due.
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            c.openCont = cont
            c.task.resume()
            Task { @MainActor in
                try? await Task.sleep(nanoseconds: UInt64(timeoutMs) * 1_000_000)
                guard let pending = c.openCont else { return }
                c.openCont = nil
                c.task.cancel()
                pending.resume(throwing: CableConnError.timeout)
            }
        }
        print("[vela-cable] WebSocket tunnel established (fido.cable)")
        return c
    }

    func writeFrame(_ bytes: Data) throws {
        if let e = sendError { throw e }
        task.send(.data(bytes)) { [weak self] error in
            guard let error else { return }
            // completion runs on the .main delegate queue → main thread.
            MainActor.assumeIsolated { self?.sendError = error }
        }
    }

    func readFrame() async throws -> Data {
        if let e = sendError { throw e }
        // The same 130s ceremony-budget watchdog the L2CAP channel carries:
        // `receive()` on an idle socket is not bounded by
        // `timeoutIntervalForRequest`, so a tunnel that goes silent while the
        // person walks away would pin the Rust ceremony thread forever.
        // Cancelling the task fails the pending receive; the flag tells a
        // deliberate timeout apart from an ordinary socket error.
        let timedOut = OSAllocatedUnfairLock(initialState: false)
        let watchdog = Task { @MainActor [task] in
            try? await Task.sleep(nanoseconds: 130_000_000_000)
            timedOut.withLock { $0 = true }
            task?.cancel()
        }
        defer { watchdog.cancel() }
        do {
            switch try await task.receive() {
            case let .data(d): return d
            case .string: throw CableConnError.closed // text frames illegal on the tunnel
            @unknown default: throw CableConnError.closed
            }
        } catch {
            if timedOut.withLock({ $0 }) { throw CableConnError.timeout }
            throw error
        }
    }

    func close() {
        task?.cancel(with: .normalClosure, reason: nil)
        session?.invalidateAndCancel()
    }
}

extension WebSocketCableConn: URLSessionWebSocketDelegate {
    nonisolated func urlSession(_ s: URLSession, webSocketTask t: URLSessionWebSocketTask, didOpenWithProtocol proto: String?) {
        MainActor.assumeIsolated { self.openCont?.resume(); self.openCont = nil }
    }

    nonisolated func urlSession(_ s: URLSession, task t: URLSessionTask, didCompleteWithError error: Error?) {
        MainActor.assumeIsolated {
            if let e = error, let cont = self.openCont { self.openCont = nil; cont.resume(throwing: e) }
        }
    }
}
