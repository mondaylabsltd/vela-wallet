//
//  ProviderScriptTests.swift
//  VelaWalletTests
//
//  The page's side of the channel (spec 070).
//
//  Until 070 the provider was two web files copied into the bundle by a build
//  phase and assembled here by stripping module keywords, with a bridge string
//  of this app's own — `ProviderBundleTests` pinned the bytes and the routing
//  table. The script is the CORE's now (`dappProviderScript`), the table is the
//  core's (`dapp_rpc::classify`, pinned against `protocol.js` by the core's own
//  suite), and what is left to prove here is that iOS injects exactly that
//  script and speaks WebKit correctly around it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ProviderScriptTests {

    // MARK: - The script

    /// The injected script IS the core's, byte for byte — no second copy, no
    /// assembly, no bridge string of this app's own.
    @Test func theInjectedScriptIsTheCoresIosScript() {
        #expect(ProviderBridge.script == dappProviderScript(host: "ios"))
        #expect(!ProviderBridge.script.isEmpty)
    }

    /// It carries the iOS bridge — WebKit's message handler — and not another
    /// host's transport, and it installs nothing in a subframe.
    @Test func theScriptCarriesTheIosBridge() {
        let script = ProviderBridge.script
        #expect(script.contains("window.webkit.messageHandlers.\(ProviderBridge.handlerName).postMessage(s)"))
        #expect(!script.contains("window.ipc.postMessage"), "that is the desktop's transport")
        #expect(!script.contains("(s) => VelaHost.postMessage"), "that is Android's transport")
        #expect(script.hasPrefix("(function () {\n\tif (window.top !== window) return;"),
                "a subframe gets no provider, rather than one that can never be answered")
        #expect(script.contains("__velaDeliver"))
        #expect(script.contains("t: 'hello'"), "every document says hello before its own scripts run")
        #expect(script.contains("eip6963:announceProvider"), "the provider itself is in there")
        #expect(!script.contains("__HOST_POST__"))
    }

    // MARK: - Delivery

    /// A message is handed over as ONE JavaScript string literal, whatever
    /// quotes, backslashes or line breaks the page's data carries.
    @Test func aDeliveryIsOneStringLiteral() throws {
        let message = #"{"doc":"d1","dir":"res","id":"1","result":"it's \"quoted\"\n\\ and </script>"}"#
        let expression = try #require(ProviderBridge.deliverExpression(message))
        #expect(expression.hasPrefix("window.__velaDeliver(\""))
        #expect(expression.hasSuffix("\")"))
        // The argument decodes back to exactly the message.
        let literal = String(expression.dropFirst("window.__velaDeliver(".count).dropLast())
        let decoded = try JSONSerialization.jsonObject(
            with: Data(("[" + literal + "]").utf8)
        ) as? [String]
        #expect(decoded == [message])
    }

    // MARK: - The frame's origin

    /// `frame_origin` is WebKit's security origin of the SENDING frame,
    /// written as an origin: the default port omitted, an IPv6 host bracketed.
    @Test func theFrameOriginIsWrittenAsAnOrigin() {
        #expect(ProviderBridge.frameOrigin(protocol: "https", host: "app.uniswap.org", port: 0)
                == "https://app.uniswap.org")
        #expect(ProviderBridge.frameOrigin(protocol: "http", host: "127.0.0.1", port: 8137)
                == "http://127.0.0.1:8137")
        #expect(ProviderBridge.frameOrigin(protocol: "http", host: "::1", port: 5173)
                == "http://[::1]:5173")
        #expect(ProviderBridge.frameOrigin(protocol: "", host: "", port: 0) == "",
                "an opaque origin is no origin — the core ignores the message")
    }

    // MARK: - What the page may navigate to

    /// Other schemes go to the system ONLY for a link the person tapped in the
    /// top frame — never from a script, never from an iframe.
    @Test func externalSchemesNeedAMainFrameLinkTap() {
        #expect(BrowserEngine.policy(scheme: "https", isMainFrame: true, linkActivated: false) == .allow)
        #expect(BrowserEngine.policy(scheme: "http", isMainFrame: false, linkActivated: false) == .allow)
        #expect(BrowserEngine.policy(scheme: "mailto", isMainFrame: true, linkActivated: true) == .handToSystem)
        #expect(BrowserEngine.policy(scheme: "tel", isMainFrame: true, linkActivated: true) == .handToSystem)
        #expect(BrowserEngine.policy(scheme: "metamask", isMainFrame: true, linkActivated: false) == .cancel,
                "a page's script cannot launch an app")
        #expect(BrowserEngine.policy(scheme: "mailto", isMainFrame: false, linkActivated: true) == .cancel,
                "an ad iframe cannot launch anything either")
        #expect(BrowserEngine.policy(scheme: "javascript", isMainFrame: true, linkActivated: true) == .cancel)
        #expect(BrowserEngine.policy(scheme: "file", isMainFrame: true, linkActivated: true) == .cancel)
        #expect(BrowserEngine.policy(scheme: "about", isMainFrame: false, linkActivated: false) == .allow)
        #expect(BrowserEngine.policy(scheme: "data", isMainFrame: true, linkActivated: true) == .cancel,
                "a top-level data: document is a phishing staple")
    }
}
