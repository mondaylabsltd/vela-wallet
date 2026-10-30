//
//  ParityP2Tests.swift
//  VelaWalletTests
//
//  Native parity Phase 2 — P2.3 (signing depth), P2.4 (home / outbox), P2.5
//  (contact notes and history) and P2.6 (the on-demand multisig catalog),
//  each branch against the web's words and the real core where it answers.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct ParityP2Tests {

    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])

    // MARK: - P2.3 ClearReading: provenance, certain lines, contract

    private func result(_ patch: [String: Any]) throws -> ClearSignResultWire {
        var object: [String: Any] = [
            "intent": "Send", "intent_term": NSNull(), "contract_name": NSNull(), "owner": NSNull(),
            "fields": [], "risk": "normal", "contract_address": "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            "verified": true, "provenance": "built_in", "sign_type": "transaction", "partial": false,
            "best_effort": false, "to_own_token": false,
        ]
        for (key, value) in patch { object[key] = value }
        return try CoreJSON.decode(ClearSignResultWire.self, from: object)
    }

    private var context: SigningLive.Context {
        var context = SigningLive.Context(
            loc: en, chainName: "Base", chainDot: .clear, nativeSymbol: "ETH",
            walletName: "Vela", walletAddress: "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        )
        context.descriptorHost = "data.getvela.app"
        return context
    }

    private func texts(_ block: SigningBlock) -> String? {
        switch block {
        case .source(let text, _), .warning(_, let text), .sentence(let text, _): return text
        default: return nil
        }
    }

    @Test func theReadingSaysWhereItCameFrom() throws {
        let vela = try result(["reading": ["source": "vela"]])
        #expect(texts(SigningLive.provenance(vela, reading: vela.reading!, context: context))
                == "✓ Read by Vela from the transaction")
        let built = try result(["reading": ["source": "built_in", "descriptor_owner": "Uniswap"]])
        #expect(texts(SigningLive.provenance(built, reading: built.reading!, context: context))?
            .hasPrefix("△ Described by Vela's built-in ERC-7730 file for Uniswap") == true)
        let fetched = try result(["reading": ["source": "fetched"]])
        #expect(texts(SigningLive.provenance(fetched, reading: fetched.reading!, context: context))?
            .contains("data.getvela.app") == true)
        let named = try result(["reading": ["source": "selector_db", "selector_name": "claimRewards(uint256)"]])
        guard case .warning(let tone, let text) = SigningLive.provenance(named, reading: named.reading!, context: context) else {
            Issue.record("a public-database name is not a caution"); return
        }
        #expect(tone == .caution && text.hasSuffix("claimRewards(uint256)"))
        // An older core sends no reading: ungraded, Vela's own.
        let old = try result([:])
        #expect(old.reading == nil)
    }

    @Test func theCertainDecodeRaisesItsLines() throws {
        let graded = try result([
            "fields": [[
                "label": "Amount", "value": "250 USDC?", "format": "tokenAmount",
                "token_address": "0x1111111111111111111111111111111111111111", "warning": false,
                "unverified": false, "role": "send_amount", "detail": false, "expired": false,
                "address": NSNull(), "usd_value": NSNull(), "token_named_by_contract": true,
            ]],
            "reading": ["source": "vela", "contradictions": ["Claim"], "goes_elsewhere": true, "dust_minimum": true],
        ])
        let lines = SigningLive.certainLines(graded, reading: graded.reading!, loc: en).compactMap(texts)
        #expect(lines.contains("A description calls this “Claim” — that is not what the transaction does"))
        #expect(lines.contains("Goes to an address that is not this account"))
        #expect(lines.contains { $0.hasPrefix("The least this accepts back is almost nothing") })
        #expect(lines.contains("USDC? · 0x1111…1111 — name given by the token's own contract"))
        let leg = SigningLive.certainLines(graded, reading: graded.reading!, loc: en, prefix: "#2 ").compactMap(texts)
        #expect(leg.allSatisfy { $0.hasPrefix("#2 ") })
    }

    @Test func theContractIsNamedWithWhoNamedIt() throws {
        let unnamed = try result(["reading": ["source": "vela"]])
        #expect(SigningLive.contractLine(unnamed, reading: unnamed.reading!, context: context).flatMap(texts)
                == "Contract · 0x8335…2913 · no name known")
        let named = try result(["contract_name": "USD Coin", "reading": ["source": "vela", "contract_named_by": "built_in"]])
        #expect(SigningLive.contractLine(named, reading: named.reading!, context: context).flatMap(texts)
                == "Contract · 0x8335…2913 · USD Coin, called so by Vela's list of known contracts")
    }

    // MARK: - P2.3 per-leg grant and the boolean card

    @Test func aGrantAllCardOffersAllowAllOrRevoke() throws {
        let editor = try CoreJSON.decode(GuardEditorViewWire.self, from: [
            "mode": NSNull(), "custom_text": "", "error": NSNull(), "choice": NSNull(),
            "display_amount_raw": NSNull(), "requested_finite": false, "requested_unlimited": false,
            "has_balance_cap": false, "revoke_offered": true, "balance_raw": NSNull(),
        ] as [String: Any])
        let meta = try CoreJSON.decode(GuardTokenMetaViewWire.self, from: [
            "symbol": "BAYC", "decimals": 0, "verified": true, "loading": false,
        ] as [String: Any])
        let block = SigningLive.allowanceBlock(
            editor: editor, meta: meta, increase: nil, decimalsUnverified: false, expired: false,
            loc: en, leg: 1, boolean: true
        )
        guard case .allowance(let label, let value, let tone, let chips, _, _, _, let leg) = block else {
            Issue.record("no allowance card"); return
        }
        #expect(label == en.t("componentsUi.signing.labelOperator"))
        #expect(value == "—" && tone == .neutral, "a grant-all must not look chosen before it is")
        #expect(chips.map(\.id) == ["grant", "revoke"])
        #expect(chips.first?.label == "Allow all")
        #expect(leg == 1)
    }

    // MARK: - P2.4 the rows kept from the last visit

    @Test func theRowsKeptAreBelievedForADay() {
        let defaults = UserDefaults(suiteName: "vela.tests.holdings.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        let row: [String: Any] = [
            "chain_id": 100, "symbol": "xDAI", "name": "xDAI", "balance": "1.5", "decimals": 18,
            "token_address": NSNull(), "price_usd": 1.0, "spam": false,
        ]
        HoldingsCache.save(address: " 0xABC ", space: "mainnet", tokens: [row, ["junk": true]], store: store, nowMs: 1_000)
        #expect(HoldingsCache.load(address: "0xabc", space: "mainnet", store: store, nowMs: 2_000).count == 1)
        #expect(HoldingsCache.load(address: "0xabc", space: "testnet", store: store, nowMs: 2_000).isEmpty)
        #expect(HoldingsCache.load(address: "0xabc", space: "mainnet", store: store,
                                   nowMs: 1_000 + HoldingsCache.ttlMs + 1).isEmpty)
        // The key the core's storage catalog calls a cache — cleared with them.
        #expect(storageIsCacheKey(key: HoldingsCache.storageKey))
    }

    /// The events this home now sends are ones the core reads.
    @Test func theHomesNewEventsAreTheCoresOwn() throws {
        let core = BalanceDashboardCore()
        for event: [String: Any] in [
            ["type": "account_noted", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894"],
            ["type": "space_changed"],
            ["type": "holdings_restored", "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894", "tokens": [[
                "chain_id": 100, "symbol": "xDAI", "name": "xDAI", "balance": "1.5", "decimals": 18,
                "token_address": NSNull(), "price_usd": 1.0, "spam": false,
            ]]],
        ] {
            #expect(throws: Never.self) { _ = try core.dispatch(eventJson: CoreJSON.string(event)) }
        }
    }

    @Test func theSyncLineIsTheWebs() {
        let owed = [SessionUnsyncedWire(name: "Mine", address: "0xAbC", retrying: true)]
        #expect(WalletSyncLine.of(unsynced: owed, address: "0xabc", failedRounds: 0, loc: en)
                == WalletSyncLine(text: "Saved on this device · syncing…", stalled: false))
        #expect(WalletSyncLine.of(unsynced: owed, address: "0xabc", failedRounds: 2, loc: en)
                == WalletSyncLine(text: "Saved on this device · not synced yet", stalled: true, retry: "Retry now"))
        let stuck = [SessionUnsyncedWire(name: "Old", address: "0xabc", retrying: false)]
        #expect(WalletSyncLine.of(unsynced: stuck, address: "0xabc", failedRounds: 0, loc: en)?.retry == nil)
        #expect(WalletSyncLine.of(unsynced: owed, address: "0xdef", failedRounds: 9, loc: en) == nil)
        #expect(WalletSyncLine.of(unsynced: owed, address: "0xabc", failedRounds: 0, loc: zh)?.text == "已保存在这台设备上 · 同步中…")
    }

    // MARK: - P2.5 notes

    @Test func aNoteIsCleanedAsTheWebCleansIt() {
        #expect(ContactsStore.cleanNote("  hi\u{202E}there\u{0007}\nok\t ") == "hithere\nok")
        #expect(ContactsStore.cleanNote(String(repeating: "a", count: 400)).count == 280)
        #expect(ContactsLive.noteModel("  ", loc: en) == nil)
        #expect(ContactsLive.noteModel("Met at ETHGlobal", loc: en) == ContactNoteModel(label: "Note", text: "Met at ETHGlobal"))
    }

    /// Through the REAL contacts core: a note saved is kept, `nil` keeps it,
    /// an empty one clears it, and `history_changed` is an event it reads.
    @Test func aNoteIsSavedKeptAndCleared() throws {
        let core = ContactsCore()
        func drive(_ result: String) throws -> [String: Any] {
            var out = try CoreJSON.object(result)
            var rounds = 0
            while rounds < 12, let effects = out["effects"] as? [[String: Any]], !effects.isEmpty {
                rounds += 1
                var last = out
                for effect in effects {
                    guard let id = (effect["id"] as? NSNumber)?.uint64Value,
                          let type = (effect["operation"] as? [String: Any])?["type"] as? String else { continue }
                    let answer: [String: Any] = switch type {
                    case "read_store": ["type": "store_loaded", "contacts": [], "tombstones": [], "groups": []]
                    case "load_send_history": ["type": "history_loaded", "txs": []]
                    case "resolve_identity": ["type": "identity_resolved", "address": "", "identity": NSNull()]
                    default: ["type": "written"]
                    }
                    last = try CoreJSON.object(core.resolveEffect(effectId: id, resultJson: CoreJSON.string(answer)))
                }
                out = last
            }
            return out
        }
        _ = try drive(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched", "my_address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ])))
        let alice = "0x9F3cA71B04e82F5c55d9B21ae00734f8dd8021aE"
        func save(_ note: Any) throws {
            _ = try drive(core.dispatch(eventJson: CoreJSON.string([
                "type": "save",
                "input": ["address": alice, "name": "Alice", "note": note, "favorite": NSNull(), "kind": NSNull(),
                          "resolved_name": NSNull(), "resolved_source": NSNull()],
                "now_ms": 1_700_000_000_000,
            ])))
        }
        func note() throws -> String? {
            try CoreJSON.decode(ContactsViewWire.self, from: try CoreJSON.object(core.view())).contacts.first?.note
        }
        try save("Met at ETHGlobal")
        #expect(try note() == "Met at ETHGlobal")
        try save(NSNull())
        #expect(try note() == "Met at ETHGlobal", "nil must keep the note")
        try save("")
        #expect(try (note() ?? "").isEmpty, "an empty note must clear it")
        #expect(throws: Never.self) { _ = try core.dispatch(eventJson: CoreJSON.string(["type": "history_changed"])) }
    }

    // MARK: - P2.6 the on-demand multisig catalog

    /// Every `home.multisig` leaf shipped under `multisig/<lng>.json` resolves
    /// through the merged catalog — in English and Chinese — and the main
    /// catalog's own keys still do.
    @Test func theMultisigCatalogIsMergedIn() throws {
        for (lang, loc) in [("en", en), ("zh", zh)] {
            let url = try #require(Bundle.main.url(forResource: lang, withExtension: "json", subdirectory: "multisig"),
                                   "multisig/\(lang).json is not bundled")
            let object = try #require(try JSONSerialization.jsonObject(with: Data(contentsOf: url)) as? [String: Any])
            var leaves: [(String, String)] = []
            func walk(_ node: Any, _ path: [String]) {
                if let dict = node as? [String: Any] {
                    for (key, value) in dict { walk(value, path + [key]) }
                } else if let text = node as? String {
                    leaves.append((path.joined(separator: "."), text))
                }
            }
            walk(object, [])
            #expect(leaves.count > 500, "\(lang): \(leaves.count) leaves")
            let unresolved = leaves.filter { key, _ in loc.t(key) == key }.map(\.0)
            #expect(unresolved.isEmpty, "\(lang): \(unresolved.prefix(5))")
            // A plain leaf reads exactly as shipped.
            #expect(loc.t("home.multisig.switcher.section") == leaves.first { $0.0 == "home.multisig.switcher.section" }?.1)
        }
        #expect(en.t("contacts.noteLabel") == "Note")
    }

    @Test func aMergeKeepsBothHalves() throws {
        let main = Data(#"{"home":{"title":"Home"},"x":1}"#.utf8)
        let more = Data(#"{"home":{"multisig":{"a":"A"}}}"#.utf8)
        let merged = try #require(Loc.merged(main, [more]))
        let object = try #require(try JSONSerialization.jsonObject(with: merged) as? [String: Any])
        let home = try #require(object["home"] as? [String: Any])
        #expect(home["title"] as? String == "Home")
        #expect((home["multisig"] as? [String: Any])?["a"] as? String == "A")
        #expect(Loc.merged(main, [Data("[]".utf8)]) == nil)
    }
}
