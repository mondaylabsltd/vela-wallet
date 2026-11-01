//
//  DeviceHandUITests.swift
//  VelaWalletUITests
//
//  A hand on a real iPhone for a device pass (app-ios/devicetest/README.md):
//  one long test that listens on a port and does what it is told — launch the
//  app with launch inputs, tap, type, swipe, read the screen, take a
//  screenshot, answer a system alert, open a link — so a person at the Mac (or
//  a script) can walk the app on hardware the way they walk a simulator.
//
//  Opt-in, and only for the device-test build: it runs when
//  `VELA_DEVICE_HAND=1` reaches the runner and, on a device, only when the
//  runner is the `.devicetest` one (`VELA_BUNDLE_ID_SUFFIX=.devicetest`), so
//  it can never drive the Vela the phone's owner keeps.
//
//      TEST_RUNNER_VELA_DEVICE_HAND=1 xcodebuild test-without-building … \
//        -only-testing:VelaWalletUITests/DeviceHandUITests/testHand
//      iproxy 9774:9774 -u <udid> &          # the port, over the cable
//      curl -s localhost:9774 -d '{"op":"shot"}' > shot.png
//
//  One JSON command per POST; the answer is JSON, or PNG bytes for `shot`.
//  `{"op":"quit"}` ends the test.
//

import Network
import XCTest

final class DeviceHandUITests: XCTestCase {

    private let env = ProcessInfo.processInfo.environment

    func testHand() throws {
        try XCTSkipUnless(env["VELA_DEVICE_HAND"] == "1", "set TEST_RUNNER_VELA_DEVICE_HAND=1 to open the hand")
        #if !targetEnvironment(simulator)
        let runner = Bundle.main.bundleIdentifier ?? ""
        try XCTSkipUnless(
            runner.contains(".devicetest"),
            "on a device the hand drives the .devicetest build only (runner \(runner))"
        )
        #endif
        let port = UInt16(env["VELA_DEVICE_HAND_PORT"] ?? "") ?? 9774
        let hand = Hand()
        let server = try HandServer(port: port, hand: hand)
        server.start()
        print("[hand] listening on \(port)")
        // The harness on the phone's loopback (`VELA_DEVICE_HAND_REVERSE=
        // "9545,9587,…"`): Android's `adb reverse`, for an iPhone.
        let tunnels = (env["VELA_DEVICE_HAND_REVERSE"] ?? "").split(separator: ",")
            .compactMap { UInt16($0) }.compactMap(ReverseTunnel.init(port:))
        tunnels.forEach { $0.start() }
        while !hand.done {
            hand.drain()
            RunLoop.current.run(until: Date(timeIntervalSinceNow: 0.05))
        }
        server.stop()
        withExtendedLifetime(tunnels) {}
    }
}

// MARK: - The commands

/// Commands arrive on the network queue and are run on the main thread, one
/// at a time, by the test's loop.
final class Hand: @unchecked Sendable {
    private let lock = NSLock()
    private var queue: [(Data, (Data, String) -> Void)] = []
    private(set) var done = false
    private var app = XCUIApplication()

    func submit(_ body: Data, reply: @escaping (Data, String) -> Void) {
        lock.lock(); queue.append((body, reply)); lock.unlock()
    }

    func drain() {
        lock.lock(); let work = queue; queue.removeAll(); lock.unlock()
        for (body, reply) in work {
            do {
                let (data, type) = try run(body)
                reply(data, type)
            } catch {
                reply(Self.json(["ok": false, "error": "\(error)"]), "application/json")
            }
        }
    }

    private static func json(_ object: [String: Any]) -> Data {
        (try? JSONSerialization.data(withJSONObject: object, options: [.sortedKeys])) ?? Data()
    }

    private func ok(_ extra: [String: Any] = [:]) -> (Data, String) {
        var o = extra; o["ok"] = true
        return (Self.json(o), "application/json")
    }

    struct Failure: Error, CustomStringConvertible { let description: String }

    /// The app a command acts on: the app under test, or another by bundle id.
    private func target(_ c: [String: Any]) -> XCUIApplication {
        if let bundle = c["bundle"] as? String { return XCUIApplication(bundleIdentifier: bundle) }
        return app
    }

    /// The element a command names: `id` (accessibility identifier), `label`
    /// (exact), `contains` (label substring), with `type` and `index`.
    private func element(_ c: [String: Any], in root: XCUIApplication) -> XCUIElement? {
        let type: XCUIElement.ElementType = {
            switch c["type"] as? String {
            case "button": return .button
            case "text": return .staticText
            case "field": return .textField
            case "secure": return .secureTextField
            case "cell": return .cell
            case "switch": return .switch
            case "link": return .link
            case "image": return .image
            case "other": return .other
            default: return .any
            }
        }()
        let predicate: NSPredicate
        if let id = c["id"] as? String {
            predicate = NSPredicate(format: "identifier == %@", id)
        } else if let label = c["label"] as? String {
            predicate = NSPredicate(format: "label == %@", label)
        } else if let part = c["contains"] as? String {
            predicate = NSPredicate(format: "label CONTAINS[c] %@ OR identifier CONTAINS[c] %@ OR value CONTAINS[c] %@", part, part, part)
        } else {
            return nil
        }
        let query = root.descendants(matching: type).matching(predicate)
        let index = c["index"] as? Int ?? 0
        let timeout = c["timeout"] as? Double ?? 5
        let first = query.element(boundBy: index)
        _ = first.waitForExistence(timeout: timeout)
        return first.exists ? first : nil
    }

    private func run(_ body: Data) throws -> (Data, String) {
        guard let c = try JSONSerialization.jsonObject(with: body) as? [String: Any],
              let op = c["op"] as? String
        else { throw Failure(description: "not a command") }
        switch op {
        case "ping":
            return ok(["state": app.state.rawValue])
        case "launch":
            app = XCUIApplication()
            app.launchEnvironment = (c["env"] as? [String: String]) ?? [:]
            app.launchArguments = (c["args"] as? [String]) ?? ["-AppleLanguages", "(en)", "-AppleLocale", "en_US"]
            let started = Date()
            app.launch()
            return ok(["seconds": Date().timeIntervalSince(started)])
        case "activate":
            let t = target(c); t.activate(); return ok(["state": t.state.rawValue])
        case "terminate":
            target(c).terminate(); return ok()
        case "home":
            XCUIDevice.shared.press(.home); return ok()
        case "open":
            guard let raw = c["url"] as? String, let url = URL(string: raw) else { throw Failure(description: "no url") }
            XCUIDevice.shared.system.open(url); return ok()
        case "state":
            return ok(["state": target(c).state.rawValue])
        case "tap":
            let t = target(c)
            if let x = c["x"] as? Double, let y = c["y"] as? Double {
                t.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: x, dy: y)).tap()
                return ok()
            }
            guard let e = element(c, in: t) else { throw Failure(description: "not found") }
            // Read before the tap: the element may be gone after it, and a
            // query that matches nothing fails the test — and closes the hand.
            let frame = NSCoder.string(for: e.frame)
            // An element behind another (a sheet over a page) is not
            // hittable, and `tap()` on it fails the test: tap where it is.
            let hittable = e.isHittable
            let at = hittable ? e.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
                : t.coordinate(withNormalizedOffset: .zero).withOffset(CGVector(dx: e.frame.midX, dy: e.frame.midY))
            if c["long"] as? Bool == true { at.press(forDuration: 1.0) } else { at.tap() }
            return ok(["frame": frame, "hittable": hittable])
        case "exists":
            let e = element(c, in: target(c))
            return ok(["exists": e != nil, "frame": e.map { NSCoder.string(for: $0.frame) } ?? ""])
        case "value":
            guard let e = element(c, in: target(c)) else { throw Failure(description: "not found") }
            return ok(["label": e.label, "value": "\(e.value ?? "")", "enabled": e.isEnabled])
        case "type":
            let t = target(c)
            if c["id"] != nil || c["label"] != nil || c["contains"] != nil {
                guard let e = element(c, in: t) else { throw Failure(description: "not found") }
                e.tap()
                e.typeText(c["text"] as? String ?? "")
            } else {
                t.typeText(c["text"] as? String ?? "")
            }
            return ok()
        case "swipe":
            let t = target(c)
            let on: XCUIElement = element(c, in: t) ?? t
            switch c["dir"] as? String {
            case "up": on.swipeUp()
            case "down": on.swipeDown()
            case "left": on.swipeLeft()
            default: on.swipeRight()
            }
            return ok()
        case "drag":
            let t = target(c)
            let o = t.coordinate(withNormalizedOffset: .zero)
            let from = o.withOffset(CGVector(dx: c["x1"] as? Double ?? 0, dy: c["y1"] as? Double ?? 0))
            let to = o.withOffset(CGVector(dx: c["x2"] as? Double ?? 0, dy: c["y2"] as? Double ?? 0))
            from.press(forDuration: c["hold"] as? Double ?? 0.05, thenDragTo: to)
            return ok()
        case "tree":
            return (Data(target(c).debugDescription.utf8), "text/plain")
        case "shot":
            return (XCUIScreen.main.screenshot().pngRepresentation, "image/png")
        case "alert":
            // A system alert (notifications, local network, camera): the
            // springboard's button by label.
            let springboard = XCUIApplication(bundleIdentifier: "com.apple.springboard")
            let button = springboard.buttons[c["label"] as? String ?? "Allow"]
            guard button.waitForExistence(timeout: c["timeout"] as? Double ?? 5) else {
                throw Failure(description: "no such alert button")
            }
            button.tap()
            return ok()
        case "sleep":
            Thread.sleep(forTimeInterval: c["seconds"] as? Double ?? 1); return ok()
        case "quit":
            done = true; return ok()
        default:
            throw Failure(description: "unknown op \(op)")
        }
    }
}

// MARK: - A small HTTP server

final class HandServer: @unchecked Sendable {
    private let listener: NWListener
    private let hand: Hand
    private let queue = DispatchQueue(label: "hand.server")

    init(port: UInt16, hand: Hand) throws {
        self.hand = hand
        listener = try NWListener(using: .tcp, on: NWEndpoint.Port(rawValue: port)!)
    }

    func start() {
        listener.newConnectionHandler = { [weak self] connection in self?.accept(connection) }
        listener.start(queue: queue)
    }

    func stop() { listener.cancel() }

    private func accept(_ connection: NWConnection) {
        connection.start(queue: queue)
        read(connection, buffer: Data())
    }

    private func read(_ connection: NWConnection, buffer: Data) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 1 << 20) { [weak self] data, _, complete, error in
            guard let self else { return }
            var buffer = buffer
            if let data { buffer.append(data) }
            if let request = Self.parse(buffer) {
                self.hand.submit(request) { body, type in
                    var head = "HTTP/1.1 200 OK\r\nContent-Type: \(type)\r\nContent-Length: \(body.count)\r\nConnection: close\r\n\r\n"
                    if body.isEmpty { head = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n" }
                    connection.send(content: Data(head.utf8) + body, completion: .contentProcessed { _ in connection.cancel() })
                }
            } else if complete || error != nil {
                connection.cancel()
            } else {
                self.read(connection, buffer: buffer)
            }
        }
    }

    /// The body of a complete request, or nil while it is still arriving.
    static func parse(_ buffer: Data) -> Data? {
        guard let end = buffer.range(of: Data("\r\n\r\n".utf8)) else { return nil }
        let head = String(decoding: buffer[..<end.lowerBound], as: UTF8.self)
        let length = head.split(separator: "\r\n")
            .first { $0.lowercased().hasPrefix("content-length:") }
            .flatMap { Int($0.split(separator: ":")[1].trimmingCharacters(in: .whitespaces)) } ?? 0
        let body = buffer[end.upperBound...]
        guard body.count >= length else { return nil }
        return Data(body.prefix(length))
    }
}

// MARK: - Reverse tunnels (adb reverse, for an iPhone)

/// The app talks plain http only to localhost (`CoreHTTP`, the core's
/// `is_localhost_http`), and an XCTest runner is not granted the local
/// network, so the harness on the Mac is reached over the CABLE: the runner
/// listens on `<port>` for the app and on `<port>+20000` for the Mac, which
/// keeps a few idle connections there through `iproxy`
/// (`app-ios/devicetest/reverse.py`). An app connection is paired with an
/// idle Mac one, the Mac is told so with one byte, and it connects to its own
/// `127.0.0.1:<port>`.
final class ReverseTunnel: @unchecked Sendable {
    private let appSide: NWListener
    private let macSide: NWListener
    private let queue = DispatchQueue(label: "hand.reverse")
    private var idleMac: [NWConnection] = []
    private var waitingApp: [NWConnection] = []

    init?(port: UInt16) {
        guard let p = NWEndpoint.Port(rawValue: port), let m = NWEndpoint.Port(rawValue: port &+ 20000),
              let app = try? NWListener(using: .tcp, on: p), let mac = try? NWListener(using: .tcp, on: m)
        else { return nil }
        appSide = app
        macSide = mac
    }

    func start() {
        appSide.newConnectionHandler = { [weak self] c in
            guard let self else { return }
            c.start(queue: self.queue)
            self.waitingApp.append(c)
            self.pair()
        }
        macSide.newConnectionHandler = { [weak self] c in
            guard let self else { return }
            c.start(queue: self.queue)
            self.idleMac.append(c)
            self.pair()
        }
        appSide.start(queue: queue)
        macSide.start(queue: queue)
    }

    private func pair() {
        while !waitingApp.isEmpty, !idleMac.isEmpty {
            let app = waitingApp.removeFirst()
            let mac = idleMac.removeFirst()
            mac.send(content: Data([1]), completion: .contentProcessed { _ in })
            Self.pipe(app, mac)
            Self.pipe(mac, app)
        }
    }

    private static func pipe(_ from: NWConnection, _ to: NWConnection) {
        from.receive(minimumIncompleteLength: 1, maximumLength: 1 << 16) { data, _, complete, error in
            if let data, !data.isEmpty {
                to.send(content: data, completion: .contentProcessed { _ in })
            }
            if complete || error != nil {
                to.send(content: nil, contentContext: .finalMessage, isComplete: true, completion: .contentProcessed { _ in })
                return
            }
            pipe(from, to)
        }
    }
}
