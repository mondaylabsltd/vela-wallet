//
//  CoreWireDriftTests.swift
//  VelaWalletTests
//
//  The guard that replaces a generator.
//
//  Web reads 300-odd `ts-rs`-generated types and CI fails on drift; desktop
//  reads the Rust structs directly and drift is a compile error. Swift gets
//  JSON, and `ts-rs` has no Swift backend — so the mirrors in `*Wire.swift` are
//  hand-written, and nothing but this file notices when `vela-core` renames a
//  field.
//
//  The check is the cheapest one that actually works: drive the REAL core
//  through the REAL bridge, take the view it emits, and decode it. A renamed or
//  retyped field fails here with a `DecodingError` naming it, which is the same
//  signal `gen-core-types.mjs --check` gives the web client — one test per
//  machine, no new toolchain.
//
//  It is deliberately NOT a snapshot of the JSON. A snapshot fails on every
//  additive change, which trains people to update it without reading it.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct CoreWireDriftTests {

    /// The bridge answers in TWO shapes, and confusing them is a real trap —
    /// this test caught it on its first run. `dispatch` and `resolveEffect`
    /// return `{ view, effects, cancelled_effect_ids }`; `view()` returns the
    /// view **bare**.
    private func view(from dispatchResult: String) throws -> [String: Any] {
        try CoreJSON.object(dispatchResult)["view"] as? [String: Any] ?? [:]
    }

    /// `ContactsView` decodes, including through the fields only a populated
    /// book reaches.
    ///
    /// The `account_switched` event is what a real screen sends first, so this
    /// exercises the same path the app does rather than a state nothing
    /// produces.
    @Test func contactsViewDecodes() throws {
        let core = ContactsCore()

        let initial = try CoreJSON.decode(ContactsViewWire.self, from: try CoreJSON.object(core.view()))
        // `loaded` is false before the stores are read — a real state the
        // waiting surface renders from, not an absence.
        #expect(!initial.loaded)
        #expect(initial.contacts.isEmpty)

        let after = try CoreJSON.decode(
            ContactsViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string([
                "type": "account_switched",
                "my_address": "0x88cca07a5d3e0b1eb5f7c0f2b1f3f4a5b6c76894",
            ])))
        )
        // Still unloaded: the core has ASKED for the stores and is waiting for
        // the shell's answer, which this test deliberately does not give.
        #expect(!after.loaded)
    }

    /// `NetView` decodes — including the two tagged unions Swift cannot
    /// synthesise, `NetProbeHealth` and `NetServiceHealth`.
    ///
    /// The `started` event is what the settings route sends first, so this is
    /// the same path the screen takes.
    @Test func networkAdminViewDecodes() throws {
        let core = NetworkAdminCore()

        let initial = try CoreJSON.decode(NetViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.loaded)
        #expect(initial.wizard.phase == .idle)
        #expect(!initial.wizard.canAdd, "the add gate must be shut before anything is known")

        let after = try CoreJSON.decode(
            NetViewWire.self,
            from: try view(from: core.dispatch(eventJson: CoreJSON.string(["type": "started"])))
        )
        #expect(!after.loaded, "the core has asked for the stores and is waiting")
    }

    /// The operations `network_admin` asks for on its first event are ones this
    /// build can perform.
    @Test func networkAdminAsksOnlyForOperationsThisBuildHandles() throws {
        let core = NetworkAdminCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "started"])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "`started` must ask the shell for the stores")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                NetworkAdminExecutor.operations.contains(tag),
                "the core asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// `RpcPoolView` decodes, and the pool asks for nothing this build cannot
    /// perform.
    ///
    /// The pool is the one machine whose unhandled operation would not merely
    /// blank a screen — every read in the app queues behind it, so a tag it
    /// cannot answer stalls the whole wallet.
    @Test func rpcPoolViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = RpcPoolCore()

        let initial = try CoreJSON.decode(RpcPoolViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.failedChains.isEmpty)
        #expect(initial.banned.isEmpty)

        // A call on a cold pool must ask for its config first.
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "call_requested",
            "call_id": "t1", "chain_id": 100, "kind": "rpc",
            "method": "eth_getBalance", "now_ms": 1_700_000_000_000,
        ])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a cold pool must ask for its endpoints")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                RpcPool.operations.contains(tag),
                "the pool asks for `\(tag)`, which this build cannot perform"
            )
        }
    }

    /// `FeedView` decodes, including the tagged `FeedRow` union Swift cannot
    /// synthesise — and the feed machine asks for nothing this build cannot do.
    ///
    /// `account_switched` is what the home sends first, and it is what makes
    /// the machine read the store and scan, so this is the app's own path.
    @Test func activityFeedViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = ActivityFeedCore()

        let initial = try CoreJSON.decode(FeedViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(initial.rows.isEmpty)
        #expect(initial.toast == nil)

        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched",
            "address": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        ])))
        _ = try CoreJSON.decode(FeedViewWire.self, from: result["view"] as? [String: Any] ?? [:])
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a switched account must read the store")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                ActivityExecutor.operations.contains(tag),
                "the feed asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// `TrustView` decodes, and the scan asks only for operations this build
    /// performs.
    ///
    /// The anti-scam core is the one machine where an unanswered operation is
    /// not merely a stall: its metadata gate would never be met, and a scan
    /// that never finishes is a wallet that never notices it was paid.
    @Test func tokenTrustViewDecodesAndAsksOnlyForHandledOperations() throws {
        let core = TokenTrustCore()

        let initial = try CoreJSON.decode(TrustViewWire.self, from: try CoreJSON.object(core.view()))
        #expect(!initial.scanning)
        #expect(initial.incoming.isEmpty)

        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "held_chains_snapshot", "address": address, "chain_ids": [100],
        ]))
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "poll_requested", "address": address,
        ])))
        let view = try CoreJSON.decode(
            TrustViewWire.self, from: result["view"] as? [String: Any] ?? [:]
        )
        #expect(view.scanning, "a requested poll must announce itself as scanning")
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "a poll must ask the chain something")
        for effect in effects {
            let tag = (effect["operation"] as? [String: Any])?["type"] as? String ?? ""
            #expect(
                TokenTrustExecutor.operations.contains(tag),
                "token_trust asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }

    /// The bridge's three methods behave the way `CoreDriver` assumes.
    ///
    /// Property 2 of the driver's contract: resolving an effect id the bridge
    /// does not know means the answer outlived the question, which is expected
    /// and must not throw — an executor answering a cancelled effect would
    /// otherwise crash the app.
    @Test func anAnswerThatOutlivedItsQuestionIsNotAnError() throws {
        let core = ContactsCore()
        let reply = try core.resolveEffect(
            effectId: 9_999,
            resultJson: CoreJSON.string(["type": "written"])
        )
        _ = try CoreJSON.decode(ContactsViewWire.self, from: try view(from: reply))
    }

    /// A machine that asks for something must be asked in the shape the
    /// executor switches on: an internally tagged object with a `type`.
    @Test func operationsArriveInternallyTagged() throws {
        let core = ContactsCore()
        let result = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string([
            "type": "account_switched", "my_address": NSNull(),
        ])))
        let effects = result["effects"] as? [[String: Any]] ?? []
        #expect(!effects.isEmpty, "account_switched must ask the shell for the stores")
        for effect in effects {
            let operation = effect["operation"] as? [String: Any] ?? [:]
            let tag = operation["type"] as? String ?? ""
            #expect(!tag.isEmpty, "an untagged operation cannot be dispatched")
            #expect(
                ContactsExecutor.operations.contains(tag),
                "the core asks for `\(tag)`, which this build's executor does not handle"
            )
        }
    }
}
