//
//  BrowserWireDriftTests.swift
//  VelaWalletTests
//
//  The drift gate for the browser's and the signing sheet's machines
//  (053's six, with `dapp_browser` in place of `dapp_permissions` since 070).
//
//  Same check as `CoreWireDriftTests`, same reason: `ts-rs` has no Swift
//  backend, the mirrors in `*Wire.swift` are hand-written, and nothing but
//  this notices when `vela-core` renames a field. Drive the REAL core through
//  the REAL bridge, take the view it emits, decode it. A renamed or retyped
//  field fails here with a `DecodingError` naming it.
//
//  Kept in its own file because six machines is a lot to bolt onto a file that
//  already carries eleven.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct BrowserWireDriftTests {

    /// `dispatch` and `resolveEffect` answer `{ view, effects,
    /// cancelled_effect_ids }`; `view()` answers the view **bare**. Confusing
    /// the two is a real trap and cost the first run of the older file.
    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    private func effects(from dispatchResult: String) throws -> [[String: Any]] {
        try CoreJSON.object(dispatchResult)["effects"] as? [[String: Any]] ?? []
    }

    private func tags(_ effects: [[String: Any]]) -> [String] {
        effects.compactMap { ($0["operation"] as? [String: Any])?["type"] as? String }
    }

    // MARK: - explore_sites

    @Test func exploreViewDecodes() throws {
        let core = ExploreSitesCore()

        let initial = try CoreJSON.decode(ExploreViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.ready, "the mirror is not live until the document has been read")
        #expect(initial.favorites.isEmpty)
        #expect(initial.selectedTab == nil)

        let started = try core.dispatch(eventJson: CoreJSON.string(["type": "start"]))
        #expect(tags(try effects(from: started)).contains("read_explore"))

        let after = try CoreJSON.decode(ExploreViewWire.self, from: try view(from: started))
        #expect(!after.ready, "still hydrating — the shell has not answered yet")
    }

    /// Every operation this machine can ask for is one the executor handles.
    @Test func exploreAsksOnlyWhatTheExecutorAnswers() throws {
        let core = ExploreSitesCore()
        var asked = tags(try effects(from: core.dispatch(eventJson: CoreJSON.string(["type": "start"]))))
        asked += tags(try effects(from: core.resolveEffect(
            effectId: 1,
            resultJson: CoreJSON.string(["type": "loaded", "doc": NSNull()])
        )))
        asked += tags(try effects(from: core.dispatch(eventJson: CoreJSON.string([
            "type": "favorite_added",
            "url": "https://app.uniswap.org/swap",
            "title": "Uniswap",
            "now_ms": 1_757_000_000_000,
        ]))))
        #expect(asked.contains("write_explore"), "pinning a site must reach the store")
        for tag in asked {
            #expect(
                ExploreExecutor.operations.contains(tag),
                "explore_sites asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    // MARK: - browser_history

    @Test func historyViewDecodes() throws {
        let core = BrowserHistoryCore()

        let initial = try CoreJSON.decode(BhistViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.entries.isEmpty)

        let started = try core.dispatch(eventJson: CoreJSON.string(["type": "start"]))
        #expect(tags(try effects(from: started)).contains("read_history"))
    }

    /// **The trap this machine carries**: there is no `ready` flag, and a
    /// visit recorded before the load lands is dropped without a word.
    ///
    /// Proving it here means the controller's wait is not superstition.
    @Test func aVisitBeforeTheLoadIsDropped() throws {
        let core = BrowserHistoryCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "start"]))

        let tooEarly = try core.dispatch(eventJson: CoreJSON.string([
            "type": "visit_recorded",
            "url": "https://app.uniswap.org/",
            "title": "Uniswap",
            "favicon": NSNull(),
            "now_ms": 1_757_000_000_000,
        ]))
        let early = try CoreJSON.decode(BhistViewWire.self, from: try view(from: tooEarly))
        #expect(early.entries.isEmpty, "recorded before hydration — dropped, silently, by design")
        #expect(!tags(try effects(from: tooEarly)).contains("write_history"))

        _ = try core.resolveEffect(
            effectId: 1, resultJson: CoreJSON.string(["type": "loaded", "entries": []])
        )
        let recorded = try core.dispatch(eventJson: CoreJSON.string([
            "type": "visit_recorded",
            "url": "https://app.uniswap.org/",
            "title": "Uniswap",
            "favicon": NSNull(),
            "now_ms": 1_757_000_000_000,
        ]))
        let loaded = try CoreJSON.decode(BhistViewWire.self, from: try view(from: recorded))
        #expect(loaded.entries.count == 1, "after the load, the same visit is kept")
        #expect(loaded.entries.first?.origin == "https://app.uniswap.org")

        for tag in tags(try effects(from: recorded)) {
            #expect(
                BhistExecutor.operations.contains(tag),
                "browser_history asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    // MARK: - dapp_browser (spec 070)

    private func dispatch(_ core: DappBrowserCore, _ event: [String: Any]) throws -> String {
        try core.dispatch(eventJson: CoreJSON.string(event))
    }

    /// The view decodes at every stage a live browser goes through, and every
    /// operation the machine asks along the way is one the executor answers.
    @Test func browserViewDecodesAndAsksOnlyWhatTheExecutorAnswers() throws {
        let core = DappBrowserCore()
        let initial = try CoreJSON.decode(DbrViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.ready, "not ready until the stored sites are read")
        #expect(initial.tabs.isEmpty)

        var asked: [String] = []
        let started = try dispatch(core, ["type": "start"])
        asked += tags(try effects(from: started))
        #expect(asked == ["list_sites"])
        let listId = (try effects(from: started).first?["id"] as? NSNumber)?.uint64Value ?? 0

        _ = try dispatch(core, ["type": "networks_changed", "chain_ids": [1, 100, 8453]])
        _ = try dispatch(core, [
            "type": "accounts_updated", "addresses": ["0x88cca0eedbf2c4426110bbfc998f048689266894"],
        ])
        _ = try dispatch(core, [
            "type": "account_switched",
            "address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "now_ms": 1_757_000_000_000,
        ])
        let listed = try core.resolveEffect(effectId: listId, resultJson: CoreJSON.string([
            "type": "sites_listed",
            "sites": [[
                "origin": "https://app.uniswap.org",
                "grant": [
                    "origin": "https://app.uniswap.org",
                    "address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
                    "chain_id": 100, "granted_at_ms": 1_757_000_000_000,
                ],
                "chain_id": 8453,
            ]],
        ]))
        let ready = try CoreJSON.decode(DbrViewWire.self, from: try view(from: listed))
        #expect(ready.ready)
        #expect(ready.sites.map(\.origin) == ["https://app.uniswap.org"])
        #expect(ready.sites.first?.chainId == 8453, "the stored chain outranks the grant's")

        func page(_ message: [String: Any]) throws -> String {
            try dispatch(core, [
                "type": "page_message", "tab": "t1",
                "frame_origin": "https://app.uniswap.org", "is_main_frame": true,
                "message_json": CoreJSON.string(message),
            ])
        }
        asked += tags(try effects(from: try page(["t": "hello", "doc": "d1"])))
        asked += tags(try effects(from: try page([
            "t": "req", "doc": "d1", "id": "1", "method": "eth_blockNumber", "params": [],
        ])))
        let signed = try page([
            "t": "req", "doc": "d1", "id": "2", "method": "personal_sign", "params": ["0x68656c6c6f", "0x0"],
        ])
        asked += tags(try effects(from: signed))
        asked += tags(try effects(from: try page([
            "t": "req", "doc": "d1", "id": "3", "method": "wallet_switchEthereumChain",
            "params": [["chainId": "0x1"]],
        ])))
        let tab = try CoreJSON.decode(DbrViewWire.self, from: try view(from: signed))
        #expect(tab.tab("t1")?.connectedAddress == "0x88cca0eedbf2c4426110bbfc998f048689266894")
        #expect(tab.signing == DbrSigningViewWire(tab: "t1", id: "2"))
        // The page goes away with its sheet up: the sheet is cancelled.
        asked += tags(try effects(from: try page(["t": "hello", "doc": "d2"])))
        asked += tags(try effects(from: try dispatch(core, ["type": "revoke_requested", "origin": "https://app.uniswap.org"])))
        asked += tags(try effects(from: try dispatch(core, ["type": "renderer_gone", "tab": "t1"])))
        let crashed = try CoreJSON.decode(
            DbrViewWire.self, from: try view(from: try dispatch(core, ["type": "tab_closed", "tab": "t1"]))
        )
        #expect(crashed.tabs.isEmpty, "a closed tab is forgotten")

        for expected in ["deliver", "read", "forward_to_signing", "write_site_chain",
                         "cancel_signing", "remove_grant"] {
            #expect(asked.contains(expected), "the walk never asked `\(expected)`")
        }
        for tag in asked {
            #expect(
                DbrExecutor.operations.contains(tag),
                "dapp_browser asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    // MARK: - sign_request

    @Test func signViewDecodes() throws {
        let core = SignRequestCore()

        let initial = try CoreJSON.decode(SignViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.surface == .hidden)
        #expect(!initial.confirmGateOpen, "the gate is shut before anything is known")

        // The order a live sheet uses. Told nothing, this machine refuses
        // every transaction with 4902 — fail-closed, and it looks exactly
        // like a broken chain.
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "networks_changed", "chain_ids": [100]]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": "0x88cca0eedbf2c4426110bbfc998f048689266894", "credential_id": "cred-1"]],
            "active_index": 0,
        ]))

        let arrived = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived",
            "id": "req-1",
            "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            "origin": "https://app.uniswap.org",
            "transport_id": "tab-1",
            "dedicated_transport": false,
            "per_request_chain": NSNull(),
            "dapp": NSNull(),
            "granted_address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "requested_address": NSNull(),
            "request_ts_ms": NSNull(),
            "now_ms": 1_757_000_000_000,
        ]))
        let sheet = try CoreJSON.decode(SignViewWire.self, from: try view(from: arrived))
        #expect(sheet.surface == .sheet, "a well-formed request on a known chain reaches the sheet")
        #expect(sheet.request?.kind == .transaction)
        #expect(sheet.swipeAction == .reject, "a swipe before commitment is a refusal")

        for tag in tags(try effects(from: arrived)) {
            #expect(
                SignExecutor.operations.contains(tag),
                "sign_request asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// **A finding, and a correction to what was ported.**
    ///
    /// Android's research says an untold machine "refuses every transaction
    /// with 4902". It does not. The chain check runs against a chain the
    /// REQUEST names — a stamped `per_request_chain` or a `chainId` embedded
    /// in the params — and a plain `eth_sendTransaction` carrying neither
    /// reaches the sheet whatever the machine has been told.
    ///
    /// That makes the seeding order matter for a narrower, sharper reason: a
    /// dApp that names its chain (which every multi-chain dApp does) is
    /// refused 4902 until `networks_changed` has landed. Believing the wider
    /// claim would have sent somebody hunting for a phantom bug on the
    /// requests that DO work.
    @Test func anUntoldMachineRefusesAChainTheRequestNames() throws {
        let core = SignRequestCore()
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": "0x88cca0eedbf2c4426110bbfc998f048689266894", "credential_id": "cred-1"]],
            "active_index": 0,
        ]))
        let arrived = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived",
            "id": "req-1",
            "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            "origin": "https://app.uniswap.org",
            "transport_id": "tab-1",
            "dedicated_transport": false,
            "per_request_chain": 100,
            "dapp": NSNull(),
            "granted_address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "requested_address": NSNull(),
            "request_ts_ms": NSNull(),
            "now_ms": 1_757_000_000_000,
        ]))
        let refused = try CoreJSON.decode(SignViewWire.self, from: try view(from: arrived))
        #expect(refused.surface == .hidden, "a named chain nobody vouched for, so no sheet")
        let responded = tags(try effects(from: arrived)).contains("send_response")
        #expect(responded, "the page is told 4902, rather than left waiting")
    }

    // MARK: - clear_signing

    @Test func clearSigningViewDecodes() throws {
        let core = ClearSigningCore()

        let initial = try CoreJSON.decode(ClearSigningViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.surface == .none)
        #expect(!initial.resolved)

        let presented = try core.dispatch(eventJson: CoreJSON.string([
            "type": "message_presented",
            "method": "personal_sign",
            "params": ["0x48656c6c6f2c2056656c61"],
            "request_origin": "https://app.uniswap.org",
        ]))
        let message = try CoreJSON.decode(ClearSigningViewWire.self, from: try view(from: presented))
        #expect(message.surface == .messageSign)
        #expect(message.message?.decodedText == "Hello, Vela", "a readable message is read, not previewed as hex")
        #expect(message.confirm == .sign)
    }

    @Test func clearSigningAsksOnlyWhatTheExecutorAnswers() throws {
        let core = ClearSigningCore()
        let resolving = try core.dispatch(eventJson: CoreJSON.string([
            "type": "resolve_transaction",
            "to": "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83",
            "data": "0xa9059cbb0000000000000000000000007687" + String(repeating: "0", count: 56),
            "value": "0",
            "chain_id": 100,
            "locale": [
                "number_format": "comma_dot",
                "date_format": "iso",
                "time_format": "h24",
                "tz_offset_minutes": 0,
            ],
        ]))
        let asked = tags(try effects(from: resolving))
        #expect(!asked.isEmpty, "resolving a call must ask somebody something")
        for tag in asked {
            #expect(
                ClearExecutor.operations.contains(tag),
                "clear_signing asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    // MARK: - approval_guard

    @Test func guardViewDecodes() throws {
        let core = ApprovalGuardCore()

        let initial = try CoreJSON.decode(GuardViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.surface == .none)
        #expect(initial.confirmAllowed, "nothing detected, so nothing to gate")

        // approve(spender, 2^256-1) — the shape this machine exists for.
        let unlimited = "0x095ea7b3"
            + String(repeating: "0", count: 24) + String(repeating: "1", count: 40)
            + String(repeating: "f", count: 64)
        let detected = try core.dispatch(eventJson: CoreJSON.string([
            "type": "approval_detected",
            "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0xddafbb505ad214d7b80b1f830fccc89b60fb7a83","data":"\#(unlimited)"}]"#,
            "chain_id": 100,
            "wallet_address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "read_only": false,
            "now_ms": 1_757_000_000_000,
        ]))
        let editor = try CoreJSON.decode(GuardViewWire.self, from: try view(from: detected))
        #expect(editor.surface == .approvalEditor)
        #expect(editor.detected?.isUnbounded == true, "2^256-1 is the unbounded case")
        // Kept as the site asked (2026-09-26): the requested chip, the
        // `unlimited` choice, and the consent the approve opts carry.
        #expect(editor.confirmAllowed, "an unlimited approval is signable as asked")
        #expect(editor.editor?.requestedFinite == false, "there is no finite figure")
        #expect(editor.editor?.requestedUnlimited == true)
        #expect(editor.editor?.choice == .unlimited, "the `unlimited` choice decodes")
        #expect(editor.unlimitedConsented)

        for tag in tags(try effects(from: detected)) {
            #expect(
                GuardExecutor.operations.contains(tag),
                "approval_guard asks for `\(tag)`, which this build's executor does not handle"
            )
        }

        // **Choosing a cap must actually reach the machine.**
        //
        // Device-found: 撤销 was tapped and the sheet kept saying 无限额. The
        // first suspicion was the wrong event name; this pins the shape
        // instead of guessing at it.
        let revoked = try CoreJSON.decode(
            GuardViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string([
                "type": "preset_selected", "mode": "revoke",
            ])))
        )
        #expect(revoked.editor?.choice == .revoke, "the revoke preset did not land")
        #expect(revoked.confirmAllowed, "a chosen cap must open this machine's gate")
        #expect(revoked.rewrittenParamsJson != nil, "a chosen cap must produce params to sign")
    }
}
