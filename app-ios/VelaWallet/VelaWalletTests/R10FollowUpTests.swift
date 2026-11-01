//
//  R10FollowUpTests.swift
//  VelaWalletTests
//
//  The round-10 follow-ups this shell sends and draws
//  (docs/multisig-machines.md §10) — each a send or a draw, through the real
//  core:
//
//  - the incoming scan: `interacting {on}` holds a due scan until `off`;
//    `refreshed` reads the store and scans; `sent` scans alone;
//  - the in-app browser: `space_networks_changed` — the other space's
//    network refused naming that space; a site moved by a space switch
//    carries `space_note` into its next sheet, and the sign machine's view
//    carries it to the sheet's line (`space_noted`);
//  - keys: the record's own key names and the account's name beside them —
//    a key row never takes the account's name ("Interleave");
//  - Send: `tokens_failed_key` drawn with "Try again" and no alert; `left`.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct R10FollowUpTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    // MARK: - The incoming scan's quiet periods

    private func presence(_ now: Double) -> [String: Any] {
        ["visible": true, "since_interaction_ms": 0, "now_ms": now]
    }

    private func ops(_ answer: [String: Any]) -> [String] {
        (answer["effects"] as? [[String: Any]] ?? []).compactMap { ($0["operation"] as? [String: Any])?["type"] as? String }
    }

    /// A feed following the cadence whose hand-off scan has landed quiet:
    /// the next scan is armed.
    private func armedFeed() throws -> (ActivityFeedCore, FeedNextScanWire) {
        let core = ActivityFeedCore()
        let switched = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(["type": "account_switched", "address": golden])))
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "presence_changed", "presence": presence(1_000), "change": "started"]))
        let effects = switched["effects"] as? [[String: Any]] ?? []
        let scan = try #require(effects.first { ($0["operation"] as? [String: Any])?["type"] as? String == "scan_incoming_transfers" })
        _ = try core.resolveEffect(effectId: try #require((scan["id"] as? NSNumber)?.uint64Value),
                                   resultJson: CoreJSON.string(["type": "sync_completed", "new_count": 0]))
        let view = try CoreJSON.decode(FeedViewWire.self, from: try CoreJSON.object(core.view()))
        return (core, try #require(view.nextScan))
    }

    /// The list moving under a finger holds the scan that comes due; the
    /// list settling runs it — the event as `ActivityStore.scrolling` sends it.
    @Test func aScanDueWhileTheListMovesWaitsForItToSettle() throws {
        let (core, next) = try armedFeed()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "interacting", "on": true, "presence": presence(2_000)]))
        let held = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(
            ["type": "scan_due", "seq": next.seq, "presence": presence(16_000)]
        )))
        #expect(!ops(held).contains("scan_incoming_transfers"), "no scan in a gesture's way: \(ops(held))")
        let settled = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(
            ["type": "interacting", "on": false, "presence": presence(17_000)]
        )))
        #expect(ops(settled).contains("scan_incoming_transfers"), "the held scan runs at off: \(ops(settled))")
    }

    /// A pull to refresh reads the store and scans; a send scans alone.
    @Test func aPullReadsAndScansAndASendScans() throws {
        let (core, _) = try armedFeed()
        let pulled = try CoreJSON.object(core.dispatch(eventJson: CoreJSON.string(
            ["type": "presence_changed", "presence": presence(3_000), "change": "refreshed"]
        )))
        #expect(ops(pulled).contains("read_tx_store"), "\(ops(pulled))")
        let (sentCore, _) = try armedFeed()
        let sent = try CoreJSON.object(sentCore.dispatch(eventJson: CoreJSON.string(
            ["type": "presence_changed", "presence": presence(3_000), "change": "sent"]
        )))
        #expect(ops(sent).contains("scan_incoming_transfers"), "\(ops(sent))")
        #expect(!ops(sent).contains("read_tx_store"), "the send's record re-reads the store itself")
    }

    /// The pull, the send and the lists moving are what the screens say —
    /// not the old tick.
    @Test func theScreensSayRefreshedSentAndScrolling() throws {
        let root = try source("App/RootView.swift")
        #expect(root.contains("activity.refreshed()"))
        #expect(!root.contains("activity.focusTick()"))
        #expect(root.contains("activity.sent(userOpHash:"))
        #expect(root.contains(#"activity.scrolling("history", $0)"#))
        #expect(root.contains(#"activity.scrolling("home", $0)"#))
        #expect(try source("App/AppGraph.swift").contains("activityStore?.sent(userOpHash:"))
    }

    // MARK: - The in-app browser and the network space

    private let dapp = "https://dapp.example"
    private let a1 = "0x1111111111111111111111111111111111111111"
    private let sepolia = 11_155_111

    private func spaceHarness() async -> BrowserHarness {
        let h = BrowserHarness(seed: { store in
            store.writeString("vela.perm.\(self.dapp)", BrowserHarness.grant(self.dapp, self.a1, chain: 100))
            store.writeString("vela.chain.\(self.dapp)", "100")
        })
        h.browser.startConnections()
        h.browser.networksChanged([1, 100], space: .mainnet, known: [1, 100, sepolia])
        h.browser.accountsChanged(addresses: [a1], active: a1)
        await h.until { h.browser.dbr.ready }
        await h.hello("t1", doc: "d1", origin: dapp)
        return h
    }

    /// The other space's network is 4902 naming that space — never "not in
    /// Vela's networks".
    @Test func theOtherSpacesNetworkIsRefusedNamingThatSpace() async {
        let h = await spaceHarness()
        await h.ask("t1", doc: "d1", origin: dapp, id: "sw", method: "wallet_switchEthereumChain",
                    params: [["chainId": "0xaa36a7"]])
        await h.until { h.answer("t1", id: "sw") != nil }
        let error = h.answer("t1", id: "sw")?.message["error"] as? [String: Any]
        #expect((error?["code"] as? NSNumber)?.intValue == 4902)
        let message = error?["message"] as? String ?? ""
        #expect(message.contains("testnet"), "\(message)")
        #expect(!message.contains("not in Vela's networks"), "\(message)")
    }

    /// A space switch moves the site to the new space's network, and its
    /// next request carries the note into the sheet it opens.
    @Test func aSiteMovedByASpaceSwitchCarriesTheNoteIntoItsSheet() async {
        let h = await spaceHarness()
        h.browser.networksChanged([sepolia], space: .testnet, known: [1, 100, sepolia])
        await h.settle()
        await h.ask("t1", doc: "d1", origin: dapp, id: "s1", method: "personal_sign",
                    params: ["0x68656c6c6f", a1])
        await h.until { !h.forwards.isEmpty }
        #expect(h.forwards.first?.chainId == sepolia, "the site is on the space's network now")
        #expect(h.forwards.first?.spaceNote == sepolia, "the switch is said on its sheet")
        #expect(h.browser.dbr.signing?.spaceNote?.key == "connect.browser.spaceMoved")
    }

    /// The sign machine hears the note (`space_noted`) and its view carries
    /// it to the sheet, in the network's own name.
    @Test func theSigningSheetSaysTheSiteMovedOnce() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.r10b.\(UUID().uuidString)")!)
        let port = ScriptedRelayPort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let controller = SigningController(
            wallet: (address: golden, credentialId: "cred-1"),
            relay: relay,
            accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store,
            pool: RpcPool(store: store, accounts: AccountStore(), offline: true),
            ports: SigningController.Ports(knownChains: { [100, 11_155_111] })
        )
        controller.open(SigningController.Incoming(
            id: "req-1", method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(golden)"]"#,
            origin: dapp, transportId: "tab-1", chainId: sepolia, spaceNote: sepolia
        ))
        let note = try #require(controller.sign.request?.spaceNote)
        #expect(note.chainId == sepolia)
        let line = try #require(SigningLive.spaceNote(controller.sign, networks: .builtin, loc: loc))
        #expect(line == loc.t("connect.browser.spaceMoved", vars: ["network": "Sepolia"]), "\(line)")
    }

    /// The connection sheet draws the consent's note under the site.
    @Test func theConnectionSheetSaysTheSiteMoved() throws {
        let dbr = try CoreJSON.decode(DbrViewWire.self, from: [
            "ready": true, "tabs": [], "sites": [], "signing": NSNull(), "queued_signing": 0,
            "consent": [
                "tab": "t1", "origin": dapp, "methods": ["eth_requestAccounts"], "address": a1,
                "chain_id": sepolia, "space_note": ["chain_id": sepolia, "key": "connect.browser.spaceMoved"],
            ],
        ])
        let model = ExploreLive.connectionModel(
            dbr: dbr, tab: nil, engine: nil, identity: (name: "Mine", address: a1), loc: loc
        )
        #expect(model.statusLine == loc.t("connect.browser.spaceMoved", vars: ["network": "Sepolia"]))
    }

    // MARK: - Keys are not accounts

    /// The owner's "Interleave" first key: the account's name came in as key
    /// 0's label. Each key now goes by its own name.
    @Test func noKeyRowTakesTheAccountsName() async throws {
        let store = AccountStore(defaults: UserDefaults(suiteName: UUID().uuidString)!)
        func key(_ n: Int, _ name: String) -> [String: Any] {
            ["credential_id": "cred-\(n)", "public_key_hex": "04" + String(repeating: "\(n)\(n)", count: 64),
             "name": name, "transports": "internal"]
        }
        await store.saveAccount([
            "id": "cred-1", "address": golden, "name": "Interleave",
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "keys": [key(1, "Parallel One"), key(2, "Parallel Two"), key(3, "Interleave")],
        ])
        let device = await WalletKeys.deviceKeys(of: golden, in: SendAccountPort(accounts: store))
        #expect(device.map(\.name) == ["Parallel One", "Parallel Two", "Interleave"])
        let rows = WalletKeys(ethCall: { _, _, _ in nil })
            .deviceOnly(accountName: "Interleave", device: device, signInCredential: "").rows
        #expect(rows.map(\.key.name) == ["Parallel One", "Parallel Two", ""],
                "a key labelled with the account's name is labelled with nothing (\"Key 3\")")
        #expect(try source("App/RootView.swift").contains("accountName: accountName"))
    }

    // MARK: - Send

    private func sendView(_ extra: [String: Any]) throws -> SendViewWire {
        var json: [String: Any] = try CoreJSON.object(SendCore().view())
        for (key, value) in extra { json[key] = value }
        return try CoreJSON.decode(SendViewWire.self, from: json)
    }

    /// A list that could not be read says so on the picker, with "Try
    /// again" — never "nothing held".
    @Test func aFailedTokenListIsThePickersOwnState() throws {
        let base = SendPickModel(
            header: FlowHeaderModel(title: "Send", backLabel: ""), searchPlaceholder: "", filters: [], rows: [],
            cta: SendCtaModel(label: "", accent: false)
        )
        let failed = SendLive.pick(try sendView(["tokens_failed_key": "send.tokensUnavailable", "loading": false]),
                                   on: base, loc: loc)
        #expect(failed.empty == loc.t("send.tokensUnavailable"))
        #expect(failed.retry == loc.t("common.tryAgain"))
        let fine = SendLive.pick(try sendView(["loading": false]), on: base, loc: loc)
        #expect(fine.retry == nil)
        #expect(fine.empty == loc.t("send.noTokensWithBalance"))
    }

    /// The core's `load_tokens_failed` alert is for a shell that does not
    /// draw the state; this one does, and leaving Send says `left`.
    @Test func theFailedListRaisesNoAlertAndLeavingSaysLeft() throws {
        let store = try source("Features/Send/SendStore.swift")
        #expect(store.contains(#"kind["type"] as? String != "load_tokens_failed""#))
        #expect(store.contains(#"dispatch(["type": "retry_tokens"])"#))
        #expect(store.contains(#"dispatch(["type": "left"])"#))
        #expect(try source("App/RootView.swift").contains("if was, !open { send.left() }"))
    }

    // MARK: - Plumbing

    private func source(_ path: String) throws -> String {
        try String(contentsOf: URL(fileURLWithPath: #filePath).deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/\(path)"), encoding: .utf8)
    }
}
