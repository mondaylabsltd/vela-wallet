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
import JavaScriptCore
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ProviderScriptTests {

    // MARK: - The script

    /// The injected script IS the core's, byte for byte, for each debug mode
    /// (spec 091) — no second copy, no assembly, no bridge string of this
    /// app's own.
    @Test func theInjectedScriptIsTheCoresIosScript() {
        for debugMode in [false, true] {
            #expect(ProviderBridge.script(debugMode: debugMode)
                    == dappProviderScript(host: "ios", debugMode: debugMode))
            #expect(!ProviderBridge.script(debugMode: debugMode).isEmpty)
        }
        #expect(ProviderBridge.script(debugMode: false) != ProviderBridge.script(debugMode: true))
    }

    /// It carries the iOS bridge — WebKit's message handler — and not another
    /// host's transport, and it installs nothing in a subframe.
    @Test func theScriptCarriesTheIosBridge() {
        for debugMode in [false, true] {
            let script = ProviderBridge.script(debugMode: debugMode)
            #expect(script.contains("window.webkit.messageHandlers.\(ProviderBridge.handlerName).postMessage(s)"))
            #expect(!script.contains("window.ipc.postMessage"), "that is the desktop's transport")
            #expect(!script.contains("(s) => VelaHost.postMessage"), "that is Android's transport")
            #expect(script.hasPrefix("(function () {\n\tif (window.top !== window || "),
                    "a subframe gets no provider, rather than one that can never be answered")
            #expect(script.contains("__velaDeliver"))
            #expect(script.contains("t: 'hello'"), "every document says hello before its own scripts run")
            #expect(script.contains("eip6963:announceProvider"), "the provider itself is in there")
            #expect(!script.contains("__HOST_POST__"))
        }
        // Debug mode off is spec 088's script, unchanged: a secure context only.
        #expect(ProviderBridge.script(debugMode: false)
            .hasPrefix("(function () {\n\tif (window.top !== window || !window.isSecureContext) return;"),
                "a page off a secure context gets no provider (spec 088)")
        #expect(!ProviderBridge.script(debugMode: false).contains("location.hostname"))
    }

    // MARK: - Who the script offers the wallet to (spec 091)

    /// The script's own gate, run in JavaScriptCore against the core's rule.
    ///
    /// Nothing is cut out of the script: the WHOLE document-start script runs
    /// in a context that plays a top-level page at each origin — `window`,
    /// `location` as a browser spells it, `isSecureContext` as WebKit decides
    /// it (https or loopback, which is debug mode off's answer), and WebKit's
    /// message handler recording what is posted. A page offered the wallet
    /// says hello; one that is not says nothing. Whether it said hello must be
    /// exactly `dappOffersWallet` — the gate the core applies again to every
    /// message — for debug mode off and on.
    @Test func theScriptsGateIsTheCoresRule() throws {
        let origins = [
            "http://192.168.1.5:3000", "http://10.0.0.1.evil.com", "http://[fd00::1]:3000",
            "http://foo.local", "http://foo.local.evil.com", "http://dapp.example",
            "https://dapp.example", "http://127.0.0.1:8137", "http://172.32.0.1",
            "http://[2001:db8::1]", "http://192.168.1.5.nip.io", "http://10.0.0.1",
            "http://172.16.0.1", "http://172.31.255.255", "http://169.254.10.20",
            "http://[fe80::1]", "http://localhost:5173", "http://[::1]:5173",
            "http://10.evil.com", "https://192.168.1.5",
        ]
        for debugMode in [false, true] {
            let script = ProviderBridge.script(debugMode: debugMode)
            for origin in origins {
                let hello = try saysHello(script: script, origin: origin)
                #expect(hello == dappOffersWallet(origin: origin, debugMode: debugMode),
                        "\(origin), debug mode \(debugMode ? "on" : "off")")
            }
        }
        // Not vacuous: a LAN page is offered the wallet in debug mode only,
        // and a public https page always.
        #expect(try saysHello(script: ProviderBridge.script(debugMode: true), origin: "http://192.168.1.5:3000"))
        #expect(try !saysHello(script: ProviderBridge.script(debugMode: false), origin: "http://192.168.1.5:3000"))
        #expect(try saysHello(script: ProviderBridge.script(debugMode: false), origin: "https://dapp.example"))
        #expect(try !saysHello(script: ProviderBridge.script(debugMode: true), origin: "http://dapp.example"))
    }

    /// Run `script` as the top document at `origin`: did it say hello?
    private func saysHello(script: String, origin: String) throws -> Bool {
        let url = try #require(URL(string: origin))
        let scheme = try #require(url.scheme)
        var host = try #require(url.host)
        // `URL.host` drops an IPv6 literal's brackets; `location.hostname`
        // keeps them.
        if host.contains(":"), !host.hasPrefix("[") { host = "[\(host)]" }
        let context = try #require(JSContext())
        var exception: String?
        context.exceptionHandler = { _, value in exception = value?.toString() }
        context.evaluateScript("""
        var window = globalThis;
        window.top = window;
        window.isSecureContext = \(dappOffersWallet(origin: origin, debugMode: false));
        window.location = {
            href: \(jsString(origin + "/")), origin: \(jsString(origin)),
            protocol: \(jsString(scheme + ":")), hostname: \(jsString(host))
        };
        var location = window.location;
        window.crypto = { randomUUID: function () { return 'doc-1'; } };
        window.addEventListener = function () {};
        window.dispatchEvent = function () { return true; };
        window.postMessage = function () {};
        window.setTimeout = function () { return 0; };
        window.clearTimeout = function () {};
        window.console = { log: function () {}, error: function () {}, warn: function () {} };
        var document = { documentElement: { setAttribute: function () {} } };
        function Event(type) { this.type = type; }
        function CustomEvent(type, init) { this.type = type; this.detail = init && init.detail; }
        var posted = [];
        window.webkit = { messageHandlers: { VelaHost: { postMessage: function (s) { posted.push(s); } } } };
        """)
        #expect(exception == nil, "the stubs: \(exception ?? "")")
        context.evaluateScript(script)
        #expect(exception == nil, "\(origin): \(exception ?? "")")
        let posts = context.evaluateScript("JSON.stringify(posted)")?.toString() ?? "[]"
        let messages = (try? JSONSerialization.jsonObject(with: Data(posts.utf8)) as? [String]) ?? []
        return messages.contains { message in
            let decoded = try? JSONSerialization.jsonObject(with: Data(message.utf8)) as? [String: Any]
            return decoded?["t"] as? String == "hello"
        }
    }

    /// A JavaScript string literal for `text`.
    private func jsString(_ text: String) -> String {
        let data = (try? JSONSerialization.data(withJSONObject: [text])) ?? Data("[\"\"]".utf8)
        return String(String(decoding: data, as: UTF8.self).dropFirst().dropLast())
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

    // MARK: - One address spelling (spec 082 T121, RG10, T042)

    /// The page hears the wallet's own spelling — EIP-55 — and an answer that
    /// differs only in case is not a change. Run on the core's own
    /// `applyAccounts`, cut from the very script this app injects.
    @Test func accountsChangedKeepsEip55() throws {
        let script = ProviderBridge.script(debugMode: false)
        let start = try #require(script.range(of: "function applyAccounts(next) {"))
        var depth = 0
        var end = start.upperBound
        for index in script[start.lowerBound...].indices {
            let character = script[index]
            if character == "{" { depth += 1 }
            if character == "}" {
                depth -= 1
                if depth == 0 { end = script.index(after: index); break }
            }
        }
        let applyAccounts = String(script[start.lowerBound..<end])
        let context = try #require(JSContext())
        context.evaluateScript("""
        var session = { accounts: [] }; var emitted = [];
        function emit(name, value) { emitted.push([name, value]); }
        \(applyAccounts)
        """)
        let checksummed = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        context.evaluateScript("applyAccounts(['\(checksummed)']);")
        #expect(context.evaluateScript("emitted.length").toInt32() == 1)
        #expect(context.evaluateScript("emitted[0][0]").toString() == "accountsChanged")
        #expect(context.evaluateScript("emitted[0][1][0]").toString() == checksummed,
                "the page is handed the wallet's spelling, never lower case")
        context.evaluateScript("applyAccounts(['\(checksummed.lowercased())']);")
        #expect(context.evaluateScript("emitted.length").toInt32() == 1,
                "a difference of case alone is not accountsChanged")
    }
}
