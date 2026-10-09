//
//  LocalDappServer.swift
//  VelaWalletUITests
//
//  The device harness's web server: one page, one port, inside the test runner.
//
//  ## Why not `file://`
//
//  `dapp_origin_of` gives a file URL **no origin at all**, so the permissions
//  machine would refuse every request as coming from nowhere and the harness
//  would prove nothing. A page has to be served over a scheme the origin rule
//  understands, and `http://127.0.0.1` is the one that needs no certificate.
//
//  On a physical device the runner and the app share loopback, so the same
//  test drives both ends with no `adb reverse` equivalent needed.
//
//  Deliberately about forty lines of HTTP. A real server here would be a
//  dependency in a test target, and the request this ever answers is
//  `GET /` — which it counts (`pageRequests`), so a test can tell a page
//  that loaded behind the screen from one that never loaded.
//

import Foundation
import Network

final class LocalDappServer {

    /// The desktop's origin tests use this port too, so a person reading two
    /// suites meets one number.
    static let port: UInt16 = 8137
    static var url: String { "http://127.0.0.1:\(port)/" }

    private let listener: NWListener
    private let body: Data
    private let queue = DispatchQueue(label: "vela.testdapp")
    private let counter = Counter()
    /// This server's own port — the shared one unless a suite that runs
    /// beside the browser's needs another (spec 071).
    let boundPort: UInt16
    var pageUrl: String { "http://127.0.0.1:\(boundPort)/" }

    init(html: String, port: UInt16 = LocalDappServer.port) throws {
        body = Data(html.utf8)
        boundPort = port
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: port)!)
    }

    /// The harness page, from the test bundle. Not `#filePath`: that is a path
    /// on the Mac, and this runs on the phone.
    static func page() throws -> String {
        let bundle = Bundle(for: LocalDappServer.self)
        guard let url = bundle.url(forResource: "testdapp", withExtension: "html") else {
            throw NSError(domain: "vela.testdapp", code: 1, userInfo: [
                NSLocalizedDescriptionKey:
                    "testdapp.html is not in the UI-test bundle — check it is a resource of VelaWalletUITests",
            ])
        }
        return try String(contentsOf: url, encoding: .utf8)
    }

    /// How many times the page itself has been asked for (`GET /`), counted
    /// as each request arrives.
    ///
    /// What a test reads to know whether a tab loaded the page — or did NOT.
    /// XCUITest sees only what is painted on screen, and a page that loads
    /// behind the Explore home paints nothing it can see; the request still
    /// reaches this server.
    var pageRequests: Int { counter.value }

    func start() {
        listener.newConnectionHandler = { [body, counter] connection in
            connection.start(queue: .global())
            connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { data, _, _, _ in
                if let data, Self.asksForThePage(data) { counter.add() }
                // Built by concatenation rather than a multiline literal: CRLF
                // framing and a literal that swallows its own last newline is a
                // combination that fails as a browser hanging on a request,
                // which is the least debuggable way for a harness to break.
                let header = "HTTP/1.1 200 OK\r\n"
                    + "Content-Type: text/html; charset=utf-8\r\n"
                    + "Content-Length: \(body.count)\r\n"
                    + "Cache-Control: no-store\r\n"
                    + "Connection: close\r\n\r\n"
                var response = Data(header.utf8)
                response.append(body)
                connection.send(content: response, completion: .contentProcessed { _ in
                    connection.cancel()
                })
            }
        }
        listener.start(queue: queue)
    }

    func stop() {
        listener.cancel()
    }

    /// The request names the page: `GET /` or `GET /?…`, nothing else.
    static func asksForThePage(_ request: Data) -> Bool {
        guard let head = String(data: request.prefix(32), encoding: .utf8) else { return false }
        return head.hasPrefix("GET / ") || head.hasPrefix("GET /?")
    }

    /// A count the server's connections add to and the test reads.
    private final class Counter: @unchecked Sendable {
        private let lock = NSLock()
        private var count = 0

        var value: Int { lock.withLock { count } }

        func add() { lock.withLock { count += 1 } }
    }
}

/// A site that is there and never answers (spec 082 T122, RH4): a loopback
/// listener that accepts every connection, reads what it is sent, and says
/// nothing back — ever. The stall the probe never covered, with no proxy and
/// nothing outside this test runner: WebKit connects, sends its request, and
/// waits. Only the browser's own watchdog can end that.
final class SilentListener {

    private let listener: NWListener
    private let queue = DispatchQueue(label: "vela.silent")
    /// Held so a connection is never torn down by being forgotten — a close
    /// would be an answer.
    private var connections: [NWConnection] = []
    private let lock = NSLock()
    /// The port the system gave it. Not a fixed number: loopback is shared
    /// with the Mac on a simulator, and a fixed port another suite (or a
    /// device pass's own test dApp) already serves would answer for it.
    private(set) var port: UInt16 = 0
    var url: String { "http://127.0.0.1:\(port)/" }

    init() throws {
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = false
        listener = try NWListener(using: parameters, on: .any)
    }

    /// Starts listening and waits (up to 5 s) for the port it was given.
    @discardableResult
    func start() -> Bool {
        let ready = DispatchSemaphore(value: 0)
        listener.stateUpdateHandler = { state in
            if case .ready = state { ready.signal() }
            if case .failed = state { ready.signal() }
        }
        listener.newConnectionHandler = { [weak self] connection in
            self?.hold(connection)
            connection.start(queue: .global())
            self?.drain(connection)
        }
        listener.start(queue: queue)
        _ = ready.wait(timeout: .now() + 5)
        port = listener.port?.rawValue ?? 0
        return port != 0
    }

    /// Read and discard, forever — the request arrives, nothing leaves.
    private func drain(_ connection: NWConnection) {
        connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { [weak self] _, _, complete, error in
            guard !complete, error == nil else { return }
            self?.drain(connection)
        }
    }

    private func hold(_ connection: NWConnection) {
        lock.lock()
        connections.append(connection)
        lock.unlock()
    }

    func stop() {
        lock.lock()
        let held = connections
        connections.removeAll()
        lock.unlock()
        held.forEach { $0.cancel() }
        listener.cancel()
    }
}
