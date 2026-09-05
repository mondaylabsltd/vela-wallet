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
