//
//  RegistryBackupTests.swift
//  VelaWalletTests
//
//  Spec 062: the iOS transport for `vela_core::registry_backup`, and the
//  settings row it feeds. The walk is the core's and is tested there; pinned
//  here is that requests are carried faithfully (a bare `0x` is an ANSWER, a
//  `nil` a silence), that a call is only ever offered with "not backed up",
//  and that the row is a button only while there is something to do.
//

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct RegistryBackupTests {
    private final class Script {
        var rounds: [String]
        var transcripts: [[[String: Any]]] = []
        init(_ rounds: [String]) { self.rounds = rounds }
    }

    private func ask(_ id: String, chain: Int) -> String {
        #"{"type":"ask","requests":[{"type":"eth_call","id":"\#(id)","chain_id":\#(chain),"to":"0xreg","data":"0xdata"}]}"#
    }

    private func done(_ state: String, withCall: Bool = false) -> String {
        let call = withCall ? #"{"chain_id":1,"to":"0xreg","value":"0","data":"0xcd438f9b"}"# : "null"
        return #"{"type":"done","state":"\#(state)","call":\#(call),"unit_id":10}"#
    }

    private func backup(_ script: Script, chain: @escaping (Int) -> String? = { _ in "0x" }) -> RegistryBackup {
        RegistryBackup(
            ethCall: { chainId, _, _ in chain(chainId) },
            step: { _, _, answers, _ in
                let parsed = (try? JSONSerialization.jsonObject(with: Data(answers.utf8))) as? [[String: Any]]
                script.transcripts.append(parsed ?? [])
                return script.rounds.removeFirst()
            }
        )
    }

    @Test func answersAreHandedBackVerbatim() async {
        let script = Script([ask("target", chain: 1), ask("groups", chain: 100), done("not_backed_up", withCall: true)])
        let check = await backup(script) { $0 == 1 ? "0x" : nil }
            .check(address: "0xsafe", foundingKeyHex: "04ab")

        #expect(check.state == .notBackedUp)
        #expect(check.call == RegistryBackup.Call(chainId: 1, to: "0xreg", data: "0xcd438f9b"))
        let last = script.transcripts.last ?? []
        #expect(last.count == 2)
        // A bare `0x` is a chain answering "nothing here"; silence is a failure.
        #expect(last.first?["outcome"] as? String == "ok")
        #expect(last.first?["body"] as? String == "0x")
        #expect(last.last?["outcome"] as? String == "failed")
        #expect(last.last?["body"] is NSNull)
    }

    @Test func everyStateMapsAndACallWithoutItsStateIsNotBelieved() async {
        let table: [(String, RegistryBackup.State)] = [
            ("unavailable", .unavailable), ("not_registered", .notRegistered),
            ("backed_up", .backedUp), ("could_not_check", .couldNotCheck),
            ("something new", .couldNotCheck),
        ]
        for (wire, state) in table {
            let check = await backup(Script([done(wire, withCall: true)]))
                .check(address: "0xsafe", foundingKeyHex: "04ab")
            #expect(check.state == state)
            #expect(check.call == nil)
        }
        // "Not backed up" with nothing to send is not something to show a fee for.
        let bare = await backup(Script([done("not_backed_up")])).check(address: "0xsafe", foundingKeyHex: "04ab")
        #expect(bare.state == .couldNotCheck)
    }

    @Test func theRowIsAButtonOnlyWhileThereIsSomethingToDo() {
        let loc = Loc(overrideTag: "en", preferredLanguages: [])
        let model = SettingsFixtures.build(.st1, loc: loc)
        func row(_ state: RegistryBackup.State?) -> SettingsRowModel? {
            SettingsLive.withEthereumBackup(state, on: model, loc: loc).sections.first?.rows.first
        }

        // Dark where there is nothing to offer.
        #expect(SettingsLive.withEthereumBackup(.unavailable, on: model, loc: loc).sections.count == model.sections.count)
        #expect(SettingsLive.withEthereumBackup(.notRegistered, on: model, loc: loc).sections.count == model.sections.count)

        #expect(row(nil)?.id == SettingsLive.ethereumBackupRow)
        #expect(row(nil)?.title == "Back up keys to Ethereum")
        #expect(row(nil)?.subtitle == "Checking…")
        #expect(row(.backedUp)?.subtitle == "Backed up on Ethereum")
        #expect(row(.notBackedUp)?.subtitle == "Not backed up yet")
        #expect(row(.couldNotCheck)?.subtitle == "Could not check")

        #expect(row(.notBackedUp)?.trailing == .chevron)
        for quiet in [nil, RegistryBackup.State.backedUp, .couldNotCheck] {
            #expect(row(quiet)?.trailing == RowTrailing.none)
        }
    }
}
