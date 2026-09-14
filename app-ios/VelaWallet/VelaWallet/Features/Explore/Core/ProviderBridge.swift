//
//  ProviderBridge.swift
//  VelaWallet
//
//  The page's side of the channel: one provider script, one bridge script,
//  one message handler, one delivery function.
//
//  Contract: `specs/053-ios-dapp-browser-signing/contracts/page-envelope.md`.
//
//  ## Everything here is `.page`, and that is not a detail
//
//  `WKUserScript` and `add(_:name:)` default to `.defaultClient`, an isolated
//  content world. A `window.ethereum` defined there is **invisible to the
//  page** — the scripts run, nothing errors, and every dApp reports no wallet.
//  Both scripts, the handler and every `evaluateJavaScript` name `.page`
//  explicitly.
//
//  ## The origin is read here, from the web view, at message time
//
//  Not from a navigation callback: a document-start script asks before
//  `didStartProvisionalNavigation` has reported anything, so the provider's
//  own warm-up `eth_chainId` and `eth_accounts` would carry an empty origin.
//  Android found that on a device. Not from the envelope either, ever — a page
//  that can name its own origin can sign as any site.
//

import Foundation
import WebKit
import VelaCore

/// One request, as it arrived.
struct ProviderIncoming {
    let id: String
    let method: String
    let paramsJson: String
    /// The **web view's** own URL at the moment the envelope arrived — never
    /// the envelope's claim.
    let url: String
}

enum ProviderBridge {

    /// The message-handler name both the bridge script and the handler use.
    static let handlerName = "VelaHost"

    /// A page can post a megabyte. Bounded before anything else sees it.
    static let maxParamsBytes = 256 * 1024

    /// The provider as ONE classic script, assembled from the bundled files.
    ///
    /// Two module keywords are stripped line by line — `export ` from the
    /// front of every `protocol.js` line, and any `import ` line in
    /// `inpage.js` — and the pair is wrapped in a single IIFE so the constants
    /// reach the provider and nothing reaches the page. The same transform the
    /// desktop's `provider_script()` and Android's `providerScript()` apply,
    /// which is why the bundled bytes can stay byte-identical to the web
    /// tree's and still run as a classic script.
    static func providerScript(bundle: Bundle = .main) -> String? {
        guard let protocolText = text(named: "protocol", bundle: bundle),
              let inpageText = text(named: "inpage", bundle: bundle)
        else { return nil }
        return assemble(protocolText: protocolText, inpageText: inpageText)
    }

    static func assemble(protocolText: String, inpageText: String) -> String {
        let constants = protocolText
            .components(separatedBy: "\n")
            .map { $0.hasPrefix("export ") ? String($0.dropFirst("export ".count)) : $0 }
            .joined(separator: "\n")
        let provider = inpageText
            .components(separatedBy: "\n")
            .filter { !$0.trimmingCharacters(in: .whitespaces).hasPrefix("import ") }
            .joined(separator: "\n")
        return "(() => {\n\(constants)\n\(provider)\n})();"
    }

    static func text(named name: String, bundle: Bundle = .main) -> String? {
        guard let url = bundle.url(forResource: name, withExtension: "js", subdirectory: "provider")
            ?? bundle.url(forResource: name, withExtension: "js")
        else { return nil }
        return try? String(contentsOf: url, encoding: .utf8)
    }

    /// The desktop's `BRIDGE_JS`, with one substitution: WebKit's message
    /// handler in place of `window.ipc.postMessage`.
    ///
    /// `window.top !== window` is the **entire** `is_main_frame` proof on
    /// every client. `forMainFrameOnly: true` is a second lock on the same
    /// door, not a replacement for this one.
    static let bridgeScript = """
    (() => {
      if (window.top !== window) return;
      const CHANNEL = 'vela-1193';
      window.addEventListener('message', (ev) => {
        if (ev.source !== window) return;
        const d = ev.data;
        if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
        window.webkit.messageHandlers.\(handlerName).postMessage(
          JSON.stringify({ id: d.id, method: d.method, params: d.params }));
      });
      window.__velaDeliver = (json) => {
        const m = JSON.parse(json);
        window.postMessage({ ch: CHANNEL, ...m }, window.location.origin);
      };
    })();
    """

    /// Both scripts, in the order the page needs them.
    static func scripts(bundle: Bundle = .main) -> [String] {
        guard let provider = providerScript(bundle: bundle) else {
            print("[vela-wallet] browser: the provider script is missing from the bundle")
            return [bridgeScript]
        }
        return [provider, bridgeScript]
    }

    /// Install both scripts and the handler into a controller.
    static func install(
        into controller: WKUserContentController,
        handler: WKScriptMessageHandler,
        bundle: Bundle = .main
    ) {
        for source in scripts(bundle: bundle) {
            controller.addUserScript(WKUserScript(
                source: source,
                injectionTime: .atDocumentStart,
                forMainFrameOnly: true,
                in: .page
            ))
        }
        controller.add(handler, contentWorld: .page, name: handlerName)
    }

    /// Deliver one envelope to the page.
    static func deliver(_ json: [String: Any], to webView: WKWebView) {
        guard let data = try? JSONSerialization.data(withJSONObject: json),
              let text = String(data: data, encoding: .utf8),
              let quoted = try? JSONSerialization.data(withJSONObject: [text]),
              let literal = String(data: quoted, encoding: .utf8)
        else { return }
        // `[".."]` minus its brackets is a JSON string literal, which is a
        // valid JavaScript string literal. Hand-escaping is how a page's own
        // quote character becomes a syntax error, or worse.
        let argument = String(literal.dropFirst().dropLast())
        webView.evaluateJavaScript(
            "window.__velaDeliver(\(argument))", in: nil, in: .page
        ) { _ in }
    }

    /// One arriving message, validated and no further.
    ///
    /// Four checks — blank id, blank method, oversized params, non-main-frame
    /// — and they are the **only** shell-side validation. Everything after
    /// them is the core's judgement.
    static func incoming(from message: WKScriptMessage) -> ProviderIncoming? {
        guard message.frameInfo.isMainFrame else { return nil }
        guard let body = message.body as? String,
              let data = body.data(using: .utf8),
              let envelope = try? JSONSerialization.jsonObject(with: data) as? [String: Any]
        else { return nil }

        guard let id = envelope["id"] as? String, !id.isEmpty,
              let method = envelope["method"] as? String, !method.isEmpty
        else { return nil }

        let params = envelope["params"] ?? []
        let paramsJson: String
        if let encoded = try? JSONSerialization.data(withJSONObject: params, options: [.fragmentsAllowed]),
           let text = String(data: encoded, encoding: .utf8) {
            paramsJson = text
        } else {
            paramsJson = "[]"
        }
        guard paramsJson.utf8.count <= maxParamsBytes else { return nil }

        return ProviderIncoming(
            id: id,
            method: method,
            paramsJson: paramsJson,
            url: message.webView?.url?.absoluteString ?? ""
        )
    }

    /// The origin, through the core's own rule. Never the envelope's claim.
    static func origin(of url: String) -> String {
        dappOriginOf(url: url) ?? ""
    }
}
