//
//  ProviderBridge.swift
//  VelaWallet
//
//  The page's side of the channel, as far as WebKit is concerned: one script
//  (the core's), one message handler, one delivery expression, and the one
//  fact only WebKit knows — the origin of the frame a message came from.
//
//  Contract: `specs/070-dapp-browser-core/contracts/dapp-browser.md`.
//
//  ## The script is the core's
//
//  Until 070 this file assembled the provider out of two bundled web files —
//  stripping `export` and `import` line by line — and appended a bridge string
//  of its own; desktop and Android each did the same, three hand-typed bridges
//  with three frame rules. `dappProviderScript(host: "ios")` is now THE
//  provider (`vela-core/provider/inpage.js`) plus THE bridge, whose only iOS
//  difference is `window.webkit.messageHandlers.VelaHost.postMessage(s)`. It
//  posts strings, mints a document id, says `hello` before the page's own
//  scripts run, and `__velaDeliver` drops anything addressed to another
//  document.
//
//  ## Everything here is `.page`, and that is not a detail
//
//  `WKUserScript` and `add(_:name:)` default to `.defaultClient`, an isolated
//  content world. A `window.ethereum` defined there is **invisible to the
//  page** — the scripts run, nothing errors, and every dApp reports no wallet.
//  The script, the handler and every `evaluateJavaScript` name `.page`.
//
//  ## The origin is the FRAME's, from WebKit
//
//  Never the web view's URL at message time — a document being navigated away
//  from can still post, and it would post under the next site's name (the 070
//  audit's iOS finding) — and never anything the page wrote.
//

import Foundation
import WebKit
import VelaCore

enum ProviderBridge {

    /// The message-handler name the core's iOS bridge posts to.
    static let handlerName = "VelaHost"

    /// The whole document-start script, from the core. Computed once: it is a
    /// few dozen kilobytes and the same for every tab.
    static let script: String = dappProviderScript(host: "ios")

    /// Install the script and the handler into a tab's controller.
    ///
    /// The handler is held through `WeakScriptHandler`: a
    /// `WKUserContentController` retains its handlers, the controller belongs
    /// to the web view, and the web view belongs to the engine — so handing it
    /// the engine directly is a cycle no tab ever leaves.
    static func install(into controller: WKUserContentController, handler: WKScriptMessageHandler) {
        controller.addUserScript(WKUserScript(
            source: script,
            injectionTime: .atDocumentStart,
            forMainFrameOnly: true,
            in: .page
        ))
        controller.add(WeakScriptHandler(handler), contentWorld: .page, name: handlerName)
    }

    /// Undo `install`: a closed tab's page must not be able to reach the
    /// wallet, even while WebKit finishes tearing it down.
    static func uninstall(from controller: WKUserContentController) {
        controller.removeScriptMessageHandler(forName: handlerName, contentWorld: .page)
        controller.removeAllUserScripts()
    }

    /// `window.__velaDeliver("<message>")`, or `nil` for text that cannot be
    /// made a literal.
    ///
    /// `[".."]` minus its brackets is a JSON string literal, which is a valid
    /// JavaScript string literal. Hand-escaping is how a page's own quote
    /// character becomes a syntax error, or worse.
    static func deliverExpression(_ messageJson: String) -> String? {
        guard let quoted = try? JSONSerialization.data(withJSONObject: [messageJson]),
              let literal = String(data: quoted, encoding: .utf8)
        else { return nil }
        return "window.__velaDeliver(\(literal.dropFirst().dropLast()))"
    }

    /// `scheme://host[:port]` of the frame that posted — what the core is told
    /// as `frame_origin`. WebKit reports port 0 for a scheme's default port.
    static func frameOrigin(protocol scheme: String, host: String, port: Int) -> String {
        guard !scheme.isEmpty, !host.isEmpty else { return "" }
        // An IPv6 literal arrives bare; an origin writes it in brackets.
        let shownHost = host.contains(":") && !host.hasPrefix("[") ? "[\(host)]" : host
        return port > 0 ? "\(scheme)://\(shownHost):\(port)" : "\(scheme)://\(shownHost)"
    }

    static func frameOrigin(of frame: WKFrameInfo) -> String {
        let origin = frame.securityOrigin
        return frameOrigin(protocol: origin.protocol, host: origin.host, port: origin.port)
    }

    /// The origin of a URL, through the core's own rule. `""` when it has
    /// none — an `about:` page, a `file:` page.
    static func origin(of url: String) -> String {
        dappOriginOf(url: url) ?? ""
    }
}

/// A script-message handler that does not keep its target alive.
final class WeakScriptHandler: NSObject, WKScriptMessageHandler {
    private weak var target: WKScriptMessageHandler?

    init(_ target: WKScriptMessageHandler) {
        self.target = target
    }

    nonisolated func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        MainActor.assumeIsolated {
            target?.userContentController(userContentController, didReceive: message)
        }
    }
}
