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
//  `GET /` — twice.
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

    init(html: String) throws {
        body = Data(html.utf8)
        let parameters = NWParameters.tcp
        parameters.allowLocalEndpointReuse = true
        listener = try NWListener(using: parameters, on: NWEndpoint.Port(rawValue: Self.port)!)
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

    func start() {
        listener.newConnectionHandler = { [body] connection in
            connection.start(queue: .global())
            connection.receive(minimumIncompleteLength: 1, maximumLength: 64 * 1024) { _, _, _, _ in
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
}
