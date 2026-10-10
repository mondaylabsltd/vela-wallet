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
//  PR 3: the row is the CORE's (`BackupState::row`) — its words, its tone
//  and what a tap does ride on the finished step, and the shell hands them
//  on. "Not copied yet" is neutral and optional; "couldn't check" asks again
//  (iOS had no retry); a wallet that can never be copied is a calm end, never
//  a "could not check" that retries for ever.
//

import Foundation
import Testing
import VelaCore
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
        // The row the core attaches to each state (`BackupState::row`); none
        // for the two states that draw nothing.
        let rows: [String: (String, String, String)] = [
            "backed_up": ("backedUp", "positive", "none"),
            "not_backed_up": ("notBackedUp", "neutral", "copy"),
            "could_not_check": ("couldNotCheck", "neutral", "retry"),
            "not_copyable": ("cannotCopy", "neutral", "none"),
        ]
        let row = rows[state].map { subtitle, tone, action in
            #","row":{"title_key":"settingsModals.backup.title","subtitle_key":"settingsModals.backup.\#(subtitle)","tone":"\#(tone)","action":"\#(action)"}"#
        } ?? ""
        return #"{"type":"done","state":"\#(state)","call":\#(call),"unit_id":10\#(row)}"#
    }

    /// A finished check in `state`, as the transport reads the core's step.
    private func check(_ state: String) async -> RegistryBackup.Check {
        await backup(Script([done(state, withCall: state == "not_backed_up")]))
            .check(address: "0xsafe", foundingKeyHex: "04ab")
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
            // Its own state: read as "could not check" it retried for ever.
            ("not_copyable", .notCopyable),
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

    /// The core's row rides on the finished step and is handed on as it
    /// came: the corpus keys, the tone, and what a tap does.
    @Test func theCoresRowIsHandedOnAsItCame() async {
        let title = "settingsModals.backup.title"
        #expect(await check("backed_up").row == RegistryBackup.Row(
            titleKey: title, subtitleKey: "settingsModals.backup.backedUp", tone: .positive, action: .none))
        #expect(await check("not_backed_up").row == RegistryBackup.Row(
            titleKey: title, subtitleKey: "settingsModals.backup.notBackedUp", tone: .neutral, action: .copy))
        #expect(await check("could_not_check").row == RegistryBackup.Row(
            titleKey: title, subtitleKey: "settingsModals.backup.couldNotCheck", tone: .neutral, action: .retry))
        #expect(await check("not_copyable").row == RegistryBackup.Row(
            titleKey: title, subtitleKey: "settingsModals.backup.cannotCopy", tone: .neutral, action: .none))
        // Nothing is drawn where there is nothing to copy to, or from.
        #expect(await check("unavailable").row == nil)
        #expect(await check("not_registered").row == nil)
        // A silence this transport caused has no core row to hand on: its
        // own "could not check", which asks again.
        let unread = await backup(Script(["not json"])).check(address: "0xsafe", foundingKeyHex: "04ab")
        #expect(unread.state == .couldNotCheck)
        #expect(unread.row == .couldNotCheck)
        #expect(unread.row?.action == .retry)
        // A tone or an action this build has never heard of is the quiet one.
        let odd = RegistryBackup.Row(json: [
            "title_key": title, "subtitle_key": "settingsModals.backup.backedUp", "tone": "sparkly", "action": "explode",
        ])
        #expect(odd?.tone == .neutral && odd?.action == RegistryBackup.Row.Action.none)
        #expect(RegistryBackup.Row(json: ["title_key": title]) == nil)
    }

    /// The REAL core, with nobody answering: "could not check", and its row
    /// under the names this transport reads — a rename there would leave the
    /// row with nothing to tap again.
    @Test func theRealCoreWordsASilenceAsARetry() async {
        let silent = await RegistryBackup(ethCall: { _, _, _ in nil })
            .check(address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                   foundingKeyHex: "04" + String(repeating: "ab", count: 64))
        #expect(silent.state == .couldNotCheck)
        #expect(silent.row == .couldNotCheck, "the core's row is not the one this file falls back to")
        let row = SettingsLive.ethereumBackupRow(silent, loc: loc)
        #expect(row?.subtitle == "Couldn't check. Tap to try again.")
        #expect(row?.trailing == .retry)
    }

    private var loc: Loc { Loc(overrideTag: "en", preferredLanguages: []) }
    private var base: SettingsScreenModel { SettingsFixtures.build(.st1, loc: loc) }

    /// The row reads the core's words, tone and action. "Not copied yet" is
    /// neutral — a copy is optional and costs a fee — and the only state
    /// with a chevron; "couldn't check" asks again; a wallet that can never
    /// be copied is a calm end with nothing to tap.
    @Test func theBackupRowIsTheCoresWordsToneAndAction() async {
        func row(_ state: String?) async -> SettingsRowModel? {
            guard let state else { return SettingsLive.ethereumBackupRow(nil, loc: loc) }
            return SettingsLive.ethereumBackupRow(await check(state), loc: loc)
        }
        // Dark where there is nothing to offer.
        #expect(await row("unavailable") == nil)
        #expect(await row("not_registered") == nil)

        let asking = await row(nil)
        #expect(asking?.id == SettingsLive.ethereumBackupRow)
        #expect(asking?.title == "Copy this wallet's record to Ethereum")
        #expect(asking?.subtitle == "Checking…")
        #expect(asking?.trailing == RowTrailing.none)

        let copied = await row("backed_up")
        #expect(copied?.subtitle == "Copied to Ethereum")
        #expect(copied?.subtitleTone == .positive)
        #expect(copied?.trailing == RowTrailing.none)

        let notYet = await row("not_backed_up")
        #expect(notYet?.title == "Copy this wallet's record to Ethereum")
        #expect(notYet?.subtitle == "Not copied yet (optional)")
        #expect(notYet?.subtitleTone == .standard, "an optional, paid copy is not a warning")
        #expect(notYet?.trailing == .chevron)

        let silent = await row("could_not_check")
        #expect(silent?.subtitle == "Couldn't check. Tap to try again.")
        #expect(silent?.subtitleTone == .standard)
        #expect(silent?.trailing == .retry, "the row said to tap and had nothing to tap")

        let never = await row("not_copyable")
        #expect(never?.subtitle == "This older wallet can't be copied")
        #expect(never?.subtitleTone == .standard)
        #expect(never?.trailing == RowTrailing.none, "a state that never changes offers nothing to retry")

        // The same row in the reader's language.
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let drawn = SettingsLive.ethereumBackupRow(await check("not_backed_up"), loc: zh)
        #expect(drawn?.title == "把钱包记录复制到以太坊")
        #expect(drawn?.subtitle == "尚未复制（可选）")
        #expect(SettingsLive.ethereumBackupRow(await check("not_copyable"), loc: zh)?.subtitle
                == "这个较早创建的钱包无法复制")
    }

    private func key(
        _ name: String = "", provider: String = "", method: KeyMethod = .platform, synced: Bool? = true
    ) -> WalletKeys.Row {
        WalletKeys.Row(
            key: CreateKeyRow(
                name: name, authenticatorAttachment: "platform", transports: "internal", confirmed: true,
                synced: synced ?? true, syncedKnown: synced != nil, aaguid: "", providerName: provider,
                method: method, kind: method
            ),
            synced: synced,
            publicKeyHex: "04" + String(repeating: "ab", count: 64)
        )
    }

    @Test func theKeysBlockNamesEveryKeyAndBadgesOnlyWhatItCanVouchFor() async {
        let registry = WalletKeys.Result(source: .registry, rows: [
            key("Interleave", provider: "Apple Passwords"),
            key(method: .securityKey, synced: false),
            key(method: .hybrid),
        ])
        let live = SettingsLive.withWalletKeys(registry, backup: await check("not_backed_up"), on: base, loc: loc)
        let block = live.keys
        #expect(block?.title == "Keys")
        #expect(block?.count == "3")
        #expect(block?.rows.map(\.name) == ["Interleave", "Key 2", "Key 3"])
        // The holder line now names WHERE the key lives (issue 207): a hybrid
        // key is reached on a phone or tablet, not a nameless "Passkey".
        #expect(block?.rows.map(\.holder) == ["Apple Passwords", "Security key", "Phone or tablet"])
        #expect(block?.rows.map { $0.pills.map(\.text) } == [["Cloud-synced"], ["Device-bound"], ["Cloud-synced"]])
        #expect(block?.rows.first?.details.map(\.label) == ["Public key", "Transport"])
        // What a copy is, said whole: what is already public (the name and
        // every key's name among it), that it is a transaction the person
        // pays for, and that it moves no money and recovers nothing.
        let explain = block?.backupExplain ?? ""
        for said in ["its address and name", "credential ID", "pay its network fee",
                     "can't move money or bring back a lost passkey"] {
            #expect(explain.contains(said), "the explanation no longer says \"\(said)\"")
        }
        #expect(!explain.contains("Only public keys"), "the old under-statement is back")
        #expect(block?.rows.first?.fingerprint == "abab…abab")
        #expect(block?.note == nil)
        #expect(block?.backup?.trailing == .chevron)
        // The block is its own thing, not a row smuggled into a section.
        #expect(live.sections.count == base.sections.count)
    }

    @Test func stillAskingRegistrySilentAndRegistryEmptyAreThreeDifferentThings() async {
        let asking = SettingsLive.withWalletKeys(nil, backup: nil, on: base, loc: loc).keys
        #expect(asking?.loading == true)
        #expect(asking?.count == "")
        #expect(asking?.rows.isEmpty == true)

        let silent = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .device, rows: [key("Mine", synced: nil)]),
            backup: await check("could_not_check"), on: base, loc: loc
        ).keys
        #expect(silent?.note == "Couldn't reach the registry. Showing what this device remembers.")
        #expect(silent?.rows.first?.pills.isEmpty == true)

        // A registry that answered with nothing was not unreachable.
        let empty = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .notRegistered, rows: [key("Mine", synced: nil)]),
            backup: await check("not_registered"), on: base, loc: loc
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
                step: { _, _, answers, _ in
                    let parsed = (try? JSONSerialization.jsonObject(with: Data(answers.utf8))) as? [[String: Any]]
                    script.transcripts.append(parsed ?? [])
                    return script.rounds.removeFirst()
                }
            )
            let result = await reader.read(
                address: "0xsafe", device: [WalletKeys.DeviceKey(publicKeyHex: "04ab", name: "A", transports: "")],
                signInCredential: ""
            )
            #expect(result.source == source)
            #expect(result.rows.first?.key.method == .securityKey)
            #expect(result.rows.first?.synced == nil)
            #expect(script.transcripts.last?.first?["outcome"] as? String == "failed")
        }
    }

    /// Spec 102: a key is captioned by where it LIVES — never "Trusted
    /// Signer". A key made on a page is an ordinary passkey on that page's
    /// domain; the page is where a person reviews and signs, not where the key
    /// is. The record still carries `signer_origin` (older builds route by
    /// it), and nothing in the list reads it: no page detail row, no page
    /// caption.
    @Test func aKeyMadeOnAPageIsCaptionedByItsPlace() async throws {
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        let page = "https://sign.example"
        let madeOnAPage = "04" + String(repeating: "11", count: 64)
        let onThisDevice = "04" + String(repeating: "22", count: 64)
        let store = AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        await store.saveAccount([
            "id": "cred-page", "address": address, "name": "Mine",
            "public_key_hex": madeOnAPage,
            "keys": [
                [
                    "credential_id": "cred-page", "public_key_hex": madeOnAPage,
                    "name": "Mine", "transports": "internal", "signer_origin": page,
                ],
                [
                    "credential_id": "cred-here", "public_key_hex": onThisDevice,
                    "name": "", "transports": "internal",
                ],
            ],
        ])

        let device = await WalletKeys.deviceKeys(
            of: address, walletName: "Mine", in: SendAccountPort(accounts: store)
        )
        #expect(device.map(\.credentialId) == ["cred-page", "cred-here"])

        let rows = await WalletKeys(ethCall: { _, _, _ in nil })
            .read(address: "", device: device, signInCredential: "").rows
        #expect(rows.map(\.key.method) == [.platform, .platform], "a fourth method came back")

        let drawn = SettingsLive.withWalletKeys(
            WalletKeys.Result(source: .device, rows: rows), backup: nil, on: base, loc: loc
        ).keys?.rows ?? []
        #expect(drawn.map(\.holder) == ["Built-in passkey", "Built-in passkey"])
        #expect(!drawn.contains { $0.details.contains { $0.value == page } }, "the page came back as a detail")
        #expect(!drawn.contains { $0.holder.contains("Trusted Signer") })
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
