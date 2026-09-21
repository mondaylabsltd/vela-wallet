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

    private var loc: Loc { Loc(overrideTag: "en", preferredLanguages: []) }
    private var base: SettingsScreenModel { SettingsFixtures.build(.st1, loc: loc) }

    @Test func theBackupRowIsAButtonOnlyWhileThereIsSomethingToDo() {
        func row(_ state: RegistryBackup.State?) -> SettingsRowModel? {
            SettingsLive.ethereumBackupRow(state, loc: loc)
        }
        // Dark where there is nothing to offer.
        #expect(row(.unavailable) == nil)
        #expect(row(.notRegistered) == nil)

        #expect(row(nil)?.id == SettingsLive.ethereumBackupRow)
        #expect(row(nil)?.title == "Back up public keys to Ethereum")
        #expect(row(nil)?.subtitle == "Checking…")
        #expect(row(.backedUp)?.subtitle == "Backed up on Ethereum")
        #expect(row(.notBackedUp)?.subtitle == "Not backed up yet")
        #expect(row(.couldNotCheck)?.subtitle == "Could not check")
        #expect(row(.notBackedUp)?.trailing == .chevron)
        for quiet in [nil, RegistryBackup.State.backedUp, .couldNotCheck] {
            #expect(row(quiet)?.trailing == RowTrailing.none)
        }
    }

    private func key(
        _ name: String = "", provider: String = "", method: KeyMethod = .platform, synced: Bool? = true
    ) -> WalletKeys.Row {
        WalletKeys.Row(
            key: CreateKeyRow(
                name: name, authenticatorAttachment: "platform", transports: "internal", confirmed: true,
                synced: synced ?? true, aaguid: "", providerName: provider, method: method
            ),
            synced: synced,
            publicKeyHex: "04" + String(repeating: "ab", count: 64)
        )
    }

    @Test func theKeysBlockNamesEveryKeyAndBadgesOnlyWhatItCanVouchFor() {
        let registry = WalletKeys.Result(source: .registry, rows: [
            key("Interleave", provider: "Apple Passwords"),
            key(method: .securityKey, synced: false),
            key(method: .hybrid),
        ])
        let live = SettingsLive.withWalletKeys(registry, backup: .notBackedUp, on: base, loc: loc)
        let block = live.keys
        #expect(block?.title == "Keys")
        #expect(block?.count == "3")
        #expect(block?.rows.map(\.name) == ["Interleave", "Key 2", "Key 3"])
        // The holder line now names WHERE the key lives (issue 207): a hybrid
        // key is reached on a phone or tablet, not a nameless "Passkey".
        #expect(block?.rows.map(\.holder) == ["Apple Passwords", "Security key", "Phone or tablet"])
        #expect(block?.rows.map { $0.pills.map(\.text) } == [["Cloud-synced"], ["Device-bound"], ["Cloud-synced"]])
        #expect(block?.rows.first?.details.map(\.label) == ["Public key", "Transport"])
        #expect(block?.backupExplain.contains("Private keys never leave") == true)
        #expect(block?.rows.first?.fingerprint == "abab…abab")
        #expect(block?.note == nil)
        #expect(block?.backup?.trailing == .chevron)
        // The block is its own thing, not a row smuggled into a section.
        #expect(live.sections.count == base.sections.count)
    }

    @Test func stillAskingRegistrySilentAndRegistryEmptyAreThreeDifferentThings() {
        let asking = SettingsLive.withWalletKeys(nil, backup: nil, on: base, loc: loc).keys
        #expect(asking?.loading == true)
        #expect(asking?.count == "")
        #expect(asking?.rows.isEmpty == true)

        let silent = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .device, rows: [key("Mine", synced: nil)]),
            backup: .couldNotCheck, on: base, loc: loc
        ).keys
        #expect(silent?.note == "Couldn't reach the registry. Showing what this device remembers.")
        #expect(silent?.rows.first?.pills.isEmpty == true)

        // A registry that answered with nothing was not unreachable.
        let empty = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .notRegistered, rows: [key("Mine", synced: nil)]),
            backup: .notRegistered, on: base, loc: loc
        ).keys
        #expect(empty?.note == nil)
        #expect(empty?.backup == nil)
    }

    @Test func theKeysTransportCarriesRequestsAndTellsTheThreeSourcesApart() async {
        let ask = #"{"type":"ask","requests":[{"type":"eth_call","id":"groups@100","chain_id":100,"to":"0xreg","data":"0xd"}]}"#
        func done(_ source: String) -> String {
            #"{"type":"done","source":"\#(source)","chain_id":null,"keys":[{"name":"A","method":"security_key","synced":null,"public_key_hex":"04ab","provider_name":"","aaguid":"","transports":"usb","authenticator_attachment":""}]}"#
        }
        let table: [(String, WalletKeys.Source)] = [
            ("registry", .registry), ("device", .device), ("not_registered", .notRegistered),
        ]
        for (wire, source) in table {
            let script = Script([ask, done(wire)])
            let reader = WalletKeys(
                ethCall: { _, _, _ in nil },
                step: { _, _, answers in
                    let parsed = (try? JSONSerialization.jsonObject(with: Data(answers.utf8))) as? [[String: Any]]
                    script.transcripts.append(parsed ?? [])
                    return script.rounds.removeFirst()
                }
            )
            let result = await reader.read(
                address: "0xsafe", device: [WalletKeys.DeviceKey(publicKeyHex: "04ab", name: "A", transports: "")]
            )
            #expect(result.source == source)
            #expect(result.rows.first?.key.method == .securityKey)
            #expect(result.rows.first?.synced == nil)
            #expect(script.transcripts.last?.first?["outcome"] as? String == "failed")
        }
    }

    @Test func aLongHexValueWrapsWithoutGainingCharacters() {
        // SwiftUI hyphenates a long unbroken "word"; a hyphen inside a public
        // key is a character that is not in the key.
        let shown = WalletKeysBlock.breakable("0x0123456789abcdef0123")
        #expect(!shown.contains("-"))
        #expect(shown.replacingOccurrences(of: "\u{200B}", with: "") == "0x0123456789abcdef0123")
        #expect(shown.contains("\u{200B}"))
    }
}
