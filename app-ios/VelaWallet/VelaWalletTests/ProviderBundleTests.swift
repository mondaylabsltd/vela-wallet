//
//  ProviderBundleTests.swift
//  VelaWalletTests
//
//  The page-side provider is COPIED from the web tree, never ported. These
//  tests are what makes that claim true rather than aspirational.
//
//  Two properties:
//
//  1. **The bundled bytes equal the web tree's.** If the build phase ever
//     transforms, minifies or reorders anything, this fails.
//  2. **The routing table equals `protocol.js`'s.** A method the script
//     classifies one way and Swift another is a method two clients answer
//     differently, and one of the two answers is wrong.
//

import Foundation
import Testing
@testable import VelaWallet

struct ProviderBundleTests {

    /// The repo root, from this file. The precedent is `ReceiveTests`.
    static let repoRoot = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent()   // VelaWalletTests
        .deletingLastPathComponent()   // app-ios/VelaWallet
        .deletingLastPathComponent()   // app-ios
        .deletingLastPathComponent()   // repo root

    static func webTree(_ name: String) throws -> String {
        try String(
            contentsOf: repoRoot.appendingPathComponent("app-web/vela-wallet/extension/\(name)"),
            encoding: .utf8
        )
    }

    // MARK: - The bytes

    @Test func theBundledProviderIsTheWebTreesBytes() throws {
        for (bundled, source) in [("protocol", "lib/protocol.js"), ("inpage", "inpage.js")] {
            // Unit tests are hosted in the app, so `Bundle.main` IS
            // VelaWallet.app and the provider is one of its resources.
            let inBundle = ProviderBridge.text(named: bundled)
            #expect(inBundle != nil, "\(bundled).js is not in the app bundle — the build phase did not run")
            #expect(
                inBundle == (try Self.webTree(source)),
                "\(bundled).js in the bundle differs from app-web's. One provider, four clients."
            )
        }
    }

    /// The assembled script is a classic script: no `export`, no `import`, and
    /// one IIFE so nothing leaks onto the page.
    @Test func assemblyStripsExactlyTwoModuleKeywords() throws {
        let script = ProviderBridge.assemble(
            protocolText: try Self.webTree("lib/protocol.js"),
            inpageText: try Self.webTree("inpage.js")
        )
        #expect(script.hasPrefix("(() => {"))
        #expect(script.hasSuffix("})();"))
        for line in script.components(separatedBy: "\n") {
            #expect(!line.hasPrefix("export "), "an `export` survived: \(line)")
            #expect(
                !line.trimmingCharacters(in: .whitespaces).hasPrefix("import "),
                "an `import` survived: \(line)"
            )
        }
        // The provider's own identity must survive the transform.
        #expect(script.contains("vela-1193"))
        #expect(script.contains("app.getvela"))
    }

    /// The bridge script posts through WebKit's handler and keeps the
    /// top-frame guard, which is the entire `is_main_frame` proof.
    @Test func theBridgeKeepsTheTopFrameGuard() {
        #expect(ProviderBridge.bridgeScript.contains("if (window.top !== window) return;"))
        #expect(ProviderBridge.bridgeScript.contains("window.webkit.messageHandlers.VelaHost.postMessage"))
        #expect(ProviderBridge.bridgeScript.contains("window.__velaDeliver"))
        #expect(!ProviderBridge.bridgeScript.contains("window.ipc"), "that is the desktop's transport")
    }

    // MARK: - The routing table

    /// One JS array/Set literal, as a set of strings.
    ///
    /// Anchored on the DECLARATION, not on the name: `protocol.js` names each
    /// of these in a header comment before declaring it, and matching the
    /// comment collects whichever array happens to come next. That is exactly
    /// how this test first "passed" a set it had never looked at.
    static func jsSet(_ source: String, named name: String) -> Set<String> {
        guard let start = source.range(of: "const \(name) = ") else { return [] }
        let tail = source[start.upperBound...]
        guard let open = tail.firstIndex(of: "["),
              let close = tail[open...].firstIndex(of: "]")
        else { return [] }
        let body = tail[tail.index(after: open)..<close]
        var found: Set<String> = []
        var current = ""
        var inside = false
        for character in body {
            if character == "'" || character == "\"" {
                if inside { found.insert(current); current = "" }
                inside.toggle()
            } else if inside {
                current.append(character)
            }
        }
        return found
    }

    @Test func theReadAllowlistMatchesTheSharedScript() throws {
        let script = try Self.webTree("lib/protocol.js")

        #expect(
            Self.jsSet(script, named: "READ_ONLY_RPC_METHODS") == DappRpc.readOnlyRpcMethods,
            "the node-read allowlist drifted from protocol.js"
        )
        #expect(
            Self.jsSet(script, named: "BUNDLER_METHODS") == DappRpc.bundlerMethods,
            "the bundler method set drifted from protocol.js"
        )

        // `READ_PROXY_METHODS` spreads the other two and then lists the rest.
        // The extras are what it adds on top.
        let proxy = Self.jsSet(script, named: "READ_PROXY_METHODS")
        #expect(
            proxy == DappRpc.extraReadMethods,
            "the extra proxied reads drifted from protocol.js"
        )
    }

    /// Every method either script classifies, classified the same way.
    @Test func everyClassifiedMethodAgrees() throws {
        let cases: [(String, DappRoute)] = [
            ("eth_sendTransaction", .sign),
            ("wallet_sendCalls", .sign),
            ("personal_sign", .sign),
            ("eth_signTypedData_v4", .sign),
            // Refused as POLICY, before the signing test would catch it.
            ("eth_sign", .unsupported),
            ("eth_chainId", .state),
            ("net_version", .state),
            ("wallet_switchEthereumChain", .switchChain),
            ("wallet_addEthereumChain", .ack),
            ("wallet_watchAsset", .ack),
            ("eth_blockNumber", .read(bundler: false)),
            ("eth_call", .read(bundler: false)),
            ("web3_clientVersion", .read(bundler: false)),
            ("eth_sendUserOperation", .read(bundler: true)),
            ("pimlico_getUserOperationGasPrice", .read(bundler: true)),
            // The method a denylist fails open on. It is not caught by the
            // signing predicate, so a catch-all read bucket would proxy a
            // signing request to a public node.
            ("eth_signTransaction", .unsupported),
            ("debug_traceTransaction", .unsupported),
            ("wallet_invokeSnap", .unsupported),
        ]
        for (method, expected) in cases {
            #expect(DappRpc.route(method) == expected, "\(method) routed to the wrong answer")
        }
    }

    @Test func aChainSwitchParamIsReadInEitherSpelling() {
        #expect(DappRpc.switchChainParam(#"[{"chainId":"0x64"}]"#) == 100)
        #expect(DappRpc.switchChainParam(#"[{"chainId":"100"}]"#) == 100)
        #expect(DappRpc.switchChainParam(#"[{"chainId":"0x"}]"#) == nil)
        #expect(DappRpc.switchChainParam(#"[{}]"#) == nil)
        #expect(DappRpc.switchChainParam("not json") == nil)
        // Beyond u32 — no chain needs it, and the core's field is a u32.
        #expect(DappRpc.switchChainParam(#"[{"chainId":"0x1FFFFFFFF"}]"#) == nil)
        #expect(DappRpc.hexChainId(100) == "0x64")
    }
}
