//
//  MarksTests.swift
//  VelaWalletTests
//
//  Which logo a token or a network wears is the core's one rule now
//  (`vela_core::app::remote_mark`). These replay the core's own vectors —
//  `rust/crates/vela-core/tests/vectors/marks.json`, the file the core, the
//  wasm, the Kotlin and Swift harnesses and every shell's tests read — through
//  THIS shell's module (`Marks`) and the model the screens draw
//  (`TokenMarkModel`), so a shell that went back to its own copy of the table,
//  or mapped the core's answer wrongly onto a badge, fails here.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MarksTests {

    /// One case of the suite, as the file spells it.
    private struct Case {
        let name: String
        let fn: String
        let input: [String: Any]
        let expect: [String: Any]
    }

    private struct Missing: Error {}

    /// The vector file, read from the repository this app is built from.
    private static func cases() throws -> [Case] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()   // VelaWalletTests
            .deletingLastPathComponent()   // app-ios/VelaWallet
            .deletingLastPathComponent()   // app-ios
            .deletingLastPathComponent()   // repo root
            .appendingPathComponent("rust/crates/vela-core/tests/vectors/marks.json")
        let json = try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any]
        guard json?["suite"] as? String == "marks", let raw = json?["cases"] as? [[String: Any]]
        else { throw Missing() }
        return raw.compactMap { entry in
            guard let name = entry["name"] as? String, let fn = entry["fn"] as? String,
                  let input = entry["input"] as? [String: Any],
                  let expect = entry["expect"] as? [String: Any]
            else { return nil }
            return Case(name: name, fn: fn, input: input, expect: expect)
        }
    }

    private static func int(_ value: Any?) -> Int? { (value as? NSNumber)?.intValue }
    private static func string(_ value: Any?) -> String? { value as? String }

    /// The suite is there, whole, and every case names a question this shell
    /// asks — a case for a function nobody replays is a case nobody checks.
    @Test func theCoresVectorsAreHereAndEveryOneIsAQuestionThisShellAsks() throws {
        let cases = try Self.cases()
        #expect(cases.count >= 36, "the suite shipped with 36 hand-written cases")
        let known: Set<String> = ["token_mark", "chain_mark", "chain_logo_url"]
        for entry in cases {
            #expect(known.contains(entry.fn), "\(entry.name): no replay for `\(entry.fn)`")
        }
        // All three questions are asked at least once.
        #expect(Set(cases.map(\.fn)) == known)
    }

    /// Every case, through `Marks` — the module every screen asks — field by
    /// field, with an absent badge as `nil`.
    @Test func everyVectorAnswersTheSameThroughThisShellsModule() throws {
        for entry in try Self.cases() {
            let endpoint = Self.string(entry.input["ethereum_data_url"]) ?? ""
            let chainId = Self.int(entry.input["chain_id"]) ?? -1
            switch entry.fn {
            case "chain_logo_url":
                let got = Marks.chainLogoURL(chainId, endpoint: endpoint)
                #expect(got == Self.string(entry.expect["value"]), "\(entry.name)")
            case "token_mark", "chain_mark":
                let mark = entry.fn == "token_mark"
                    ? Marks.token(
                        chainId: chainId,
                        symbol: Self.string(entry.input["symbol"]) ?? "",
                        tokenAddress: Self.string(entry.input["token_address"]),
                        named: entry.input["named"] as? [String] ?? [],
                        endpoint: endpoint
                    )
                    : Marks.chain(
                        chainId: chainId,
                        nativeSymbol: Self.string(entry.input["native_symbol"]) ?? "",
                        endpoint: endpoint
                    )
                #expect(mark.glyph == Self.string(entry.expect["glyph"]), "\(entry.name): glyph")
                #expect(mark.logoUrls == entry.expect["logo_urls"] as? [String], "\(entry.name): logos")
                #expect(mark.badgeChainId.map(Int.init) == Self.int(entry.expect["badge_chain_id"]),
                        "\(entry.name): badge chain")
                #expect(mark.badgeLogoUrl == Self.string(entry.expect["badge_logo_url"]),
                        "\(entry.name): badge logo")
            default:
                Issue.record("\(entry.name): no replay for `\(entry.fn)`")
            }
        }
    }

    /// …and on to the model the circle is drawn from: the logos as the core
    /// ordered them, the badge drawn exactly when the core names its chain
    /// (its logo the core's), and the circle's letters the core's glyph. The
    /// gallery's marks, which never ask the core, letter themselves by the
    /// same rule.
    @Test func everyVectorDrawsTheSameCircle() throws {
        for entry in try Self.cases() where entry.fn != "chain_logo_url" {
            let endpoint = Self.string(entry.input["ethereum_data_url"]) ?? ""
            let chainId = Self.int(entry.input["chain_id"]) ?? -1
            let ticker = Self.string(entry.input[entry.fn == "token_mark" ? "symbol" : "native_symbol"]) ?? ""
            let view = entry.fn == "token_mark"
                ? Marks.token(chainId: chainId, symbol: ticker,
                              tokenAddress: Self.string(entry.input["token_address"]),
                              named: entry.input["named"] as? [String] ?? [], endpoint: endpoint)
                : Marks.chain(chainId: chainId, nativeSymbol: ticker, endpoint: endpoint)
            let model = TokenMarkModel.from(view, ticker: ticker, color: .clear)
            let badge = Self.int(entry.expect["badge_chain_id"])
            #expect(model.logoURLs == entry.expect["logo_urls"] as? [String], "\(entry.name): logos")
            #expect(model.badgeHidden == (badge == nil), "\(entry.name): badge shown")
            #expect(model.badgeLogoURL == Self.string(entry.expect["badge_logo_url"]),
                    "\(entry.name): badge logo")
            #expect(model.glyph == Self.string(entry.expect["glyph"]), "\(entry.name): letters")
            // A drawn mark with the same ticker letters itself the same way.
            #expect(TokenMarkModel(ticker: ticker, badgeColor: .clear).glyph == model.glyph,
                    "\(entry.name): the gallery's letters")
        }
    }

    /// The kind rule, through the model the screens use: ETH sent on Base —
    /// its NETWORK row wears Base's logo and no badge, its COIN wears
    /// Ethereum's logo with Base's badge.
    @Test func aNetworkRowWearsTheNetworkAndACoinWearsTheCoin() {
        let network = TokenMarkModel.chain(chainId: 8453, symbol: "ETH", color: .clear)
        let coin = TokenMarkModel.of(chainId: 8453, symbol: "ETH", color: .clear)
        #expect(network.logoURLs == [Marks.chainLogoURL(8453)].compactMap { $0 })
        #expect(network.badgeHidden)
        #expect(coin.logoURLs == [Marks.chainLogoURL(1)].compactMap { $0 })
        #expect(!coin.badgeHidden)
        #expect(coin.badgeLogoURL == Marks.chainLogoURL(8453))
    }

    /// Chain 0 names no network: nothing is asked of the endpoint for it, and
    /// a chain id the core's `u32` cannot hold is treated the same.
    @Test func chainZeroAndAnImpossibleChainAskForNothing() {
        #expect(Marks.chainLogoURL(0) == nil)
        #expect(Marks.chainLogoURL(-1) == nil)
        #expect(Marks.chainLogoURL(Int(UInt32.max) + 1) == nil)
        let usdc = Marks.token(chainId: 0, symbol: "USDC",
                               tokenAddress: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48")
        #expect(usdc.logoUrls.isEmpty)
        #expect(usdc.glyph == "USD")
    }

    /// Settings' endpoint write reaches whoever the app named to hear it,
    /// with the stored blob as it now reads — which is how the logos follow a
    /// changed 服务节点 without a relaunch. Nothing here touches `Marks.base`.
    @Test func anEndpointsWriteIsHeardWithTheStoredBlob() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let accounts = AccountStore(defaults: defaults)
        let executor = NetworkAdminExecutor(store: VelaStore(defaults: defaults), accounts: accounts)
        var heard: [[String: Any]] = []
        executor.onEndpointsWritten = { heard.append($0) }
        _ = await executor.perform([
            "type": "write_service_endpoints",
            "endpoints": [
                "ethereum_data_url": "https://mirror.test/",
                "passkey_index_url": "", "bundler_service_url": "", "fiat_rates_url": "",
            ],
        ])
        #expect(heard.count == 1)
        #expect(heard.first?["ethereumDataURL"] as? String == "https://mirror.test/")
        // The core trims the slash; the mark asks the mirror.
        #expect(Marks.chainLogoURL(100, endpoint: heard.first?["ethereumDataURL"] as? String)
                == "https://mirror.test/chainlogos/eip155-100.png")

        // Emptied, the field is gone from the blob — the built-in endpoint.
        _ = await executor.perform([
            "type": "write_service_endpoints",
            "endpoints": [
                "ethereum_data_url": "", "passkey_index_url": "",
                "bundler_service_url": "", "fiat_rates_url": "",
            ],
        ])
        #expect(heard.count == 2)
        #expect(heard.last?["ethereumDataURL"] == nil)
        #expect(Marks.chainLogoURL(100, endpoint: heard.last?["ethereumDataURL"] as? String ?? "")
                == "https://ethereum-data.getvela.app/chainlogos/eip155-100.png")
    }
}
