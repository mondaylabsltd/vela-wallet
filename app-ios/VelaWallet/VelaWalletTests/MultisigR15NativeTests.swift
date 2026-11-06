//
//  MultisigR15NativeTests.swift
//  VelaWalletTests
//
//  Round 15's iOS findings and the checks R15 left unconcluded on iOS
//  (docs/multisig-machines.md §23):
//
//  - r15-1: the home's Activity (and History) is the shown account's — never
//    another account's rows, never "No activity yet" before it was looked at.
//  - r15-5: a press that did not go — the fee-moved stop above all — is said
//    beside the button pressed, in the core's words, with ONE "That didn't
//    go through:".
//  - r15-8: a Safe imported by its row's Try again says "Imported".
//  - r15-9: Switch account wears one check, in neutral ink.
//  - r15-14: the fee note is said once.
//  - r14-10: the camera's way out says "Open Settings" (the label choice).
//  - The newest link wins over an older one still opening (through the real
//    accounts machine).
//

#if DEBUG
import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct MultisigR15NativeTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one
    private static let two = MultisigViewsTests.two

    // MARK: - r15-1: Activity is the shown account's

    private static let interleave = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private static let und = "0x7f3cE90000000000000000000000000000A0de87"

    private func feed(account: String, settled: Bool, rows: Bool) throws -> FeedViewWire {
        let row = #"""
        {"type":"header","id":"day-1","day_start_ms":1789948800000,"timestamp":1790000000},
        {"type":"item","item":{"id":"r1","direction":"in","counterparty":"0x3e9b9555EC4b80d0aaf59Da8E956A494ab31692D","alias":null,
          "value":"0.3","symbol":"USDC","decimals":6,"usd_value":0.3,"priced":true,"chain_id":5042002,"timestamp":1790000000,
          "day_start_ms":1789948800000,"tx_hash":"0x01","batch":null,"kind":"receive","status":"confirmed","site":null,
          "counterparty_role":"recipient","dapp":null,"subtitle":[],"figure_maskable":true}}
        """#
        let list = rows ? "[\(row)]" : "[]"
        let json = """
        {"rows":\(list),"home_rows":\(list),"transactions":[],"new_item_id":null,"toast":null,"hidden":false,
         "history_empty_key":"history.emptyTitle","home_empty_key":"home.emptyNoActivity","contact_rows":[],
         "account":"\(account.lowercased())","settled":\(settled)}
        """
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return try decoder.decode(FeedViewWire.self, from: Data(json.utf8))
    }

    private func home(_ feed: FeedViewWire, shown: String?) -> WalletHomeModel {
        let drawn = WalletFixtures.buildMobileState(.h1, loc: loc)
        let balance = BalanceViewWire(
            address: shown ?? Self.interleave,
            displayTotalUsd: 0, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [], unpricedTokens: [], failedChainIds: [],
            rateLimitedChainIds: [], holdingsLoading: false,
            cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        return WalletLive.apply(balance, feed: feed, feedRead: true, on: drawn, loc: loc, shownAccount: shown)
    }

    @Test func anotherAccountsRowsAreNeverDrawnUnderTheAccountInFront() throws {
        let theirs = try feed(account: Self.interleave, settled: true, rows: true)
        #expect(theirs.account == Self.interleave.lowercased())
        let under = home(theirs, shown: Self.und)
        #expect(under.activityGroups.isEmpty, "never Interleave's rows under R15 Und")
        #expect(under.activitySection.mode == .loading, "being read, never 'No activity yet'")
        let own = home(theirs, shown: Self.interleave)
        #expect(own.activitySection.mode == .rows)
        #expect(own.activityGroups.first?.rows.count == 1)
    }

    @Test func anEmptyActivityNotLookedAtIsBeingRead() throws {
        // Interleave's own home, its history unread (the tester's frame).
        #expect(home(try feed(account: Self.interleave, settled: false, rows: false), shown: Self.interleave)
            .activitySection.mode == .loading)
        // Looked at: "No activity yet" is the core's claim to make.
        let looked = home(try feed(account: Self.interleave, settled: true, rows: false), shown: Self.interleave)
        #expect(looked.activitySection.mode == .empty)
        #expect(looked.activitySection.empty?.title == loc.t("home.emptyNoActivity"))
        // History follows the same rule.
        let drawn = HistoryModel(header: FlowHeaderModel(title: "History", backLabel: "", action: nil, pill: nil),
                                 mode: .rows, emptyText: "", groups: [])
        let other = FlowsLive.history(try feed(account: Self.interleave, settled: true, rows: true), on: drawn, loc: loc,
                                      hidden: false, shownAccount: Self.und)
        #expect(other.groups.isEmpty && other.mode == .loading)
        let mine = FlowsLive.history(try feed(account: Self.und, settled: true, rows: false), on: drawn, loc: loc,
                                     hidden: false, shownAccount: Self.und)
        #expect(mine.mode == .empty)
    }

    @Test func theHomeAndHistoryNameTheAccountShown() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/App/RootView.swift"), encoding: .utf8)
        #expect(source.components(separatedBy: "shownAccount: shownAddress").count - 1 == 2)
    }

    // MARK: - r15-5: a press that did not go is said where it was pressed

    private func model(status: String, approved: Int, canApprove: [String], canExecute: [String],
                       action: String, oneTouch: Bool = false) throws -> MsProposalModel {
        let item = MultisigViewsTests.item(approved: approved, status: status, canApprove: canApprove, canExecute: canExecute)
        var json = MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":\#(approved),"threshold":2}"#,
            extra: oneTouch ? #","approve_execute":{"100:\#(Self.hash)":["\#(canApprove.first ?? "")"]}"# : ""
        )
        json = json.replacingOccurrences(of: #""actions":{}"#, with: #""actions":{"execute:\#(Self.hash)":\#(action)}"#)
        let view = try #require(decode(MsProposalsView.self, json))
        return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    private static let feeMoved: String = {
        let detail = #"{"kind":"fee_moved","amount":"120000000000000000","chainId":100,"executor":"\#(MultisigViewsTests.two)"}"#
        let escaped = detail.replacingOccurrences(of: "\"", with: "\\\"")
        return #"{"kind":"failed","detail":"\#(escaped)"}"#
    }()

    @Test func aFeeThatMovedIsSaidBesideTheOnePressNeverOnlyAtTheHead() throws {
        // Not ready (1 of 2): the one press approves and executes — its stop
        // is said under it.
        let m = try model(status: "pending", approved: 1, canApprove: [Self.two], canExecute: [Self.two],
                          action: Self.feeMoved, oneTouch: true)
        let beside = m.pressFailures(at: .approve)
        #expect(beside.count == 1, "\(beside)")
        let line = try #require(beside.first?.text)
        #expect(line.contains("0.12"), "\(line)")
        #expect(beside.first?.tone == .danger)
        // One "That didn't go through:", never two (the web's r15-6).
        let prefix = words.t("proposal.failed", ["reason": ""]).trimmingCharacters(in: .whitespaces)
        #expect(line.components(separatedBy: prefix).count - 1 == 1, "\(line)")
        #expect(!m.notes.contains { $0.id == "executeFailed" }, "said once, where it was pressed")
        #expect(m.pressFailures(at: .execute).isEmpty)
    }

    @Test func aFeeThatMovedIsSaidUnderExecuteWhenReady() throws {
        let m = try model(status: "ready", approved: 2, canApprove: [], canExecute: [Self.two], action: Self.feeMoved)
        #expect(m.pressFailures(at: .execute).count == 1)
        #expect(m.pressFailures(at: .approve).isEmpty)
        #expect(!m.notes.contains { $0.id == "executeFailed" })
    }

    @Test func withNoButtonForItAFailureIsStillSaidAtTheHead() throws {
        // Nothing here can act on it any more: the head says why the press did not go.
        let m = try model(status: "pending", approved: 1, canApprove: [], canExecute: [], action: Self.feeMoved)
        #expect(m.notes.contains { $0.id == "executeFailed" })
    }

    @Test func theSheetDrawsPressFailuresBesideItsButtons() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift"), encoding: .utf8)
        #expect(source.contains("pressFailures(model, at: .approve)"))
        #expect(source.contains("pressFailures(model, at: .execute)"))
    }

    // MARK: - r15-14: the fee note once

    @Test func theFeeNoteIsSaidOnce() throws {
        let m = try model(status: "pending", approved: 1, canApprove: [Self.two], canExecute: [Self.two],
                          action: "null", oneTouch: true)
        // The one press says it under itself; Execute's place does not again.
        #expect(!m.feeNoteUnderExecute(oneTouchOffered: true))
        #expect(m.feeNoteUnderExecute(oneTouchOffered: false))
        let ready = try model(status: "ready", approved: 2, canApprove: [], canExecute: [Self.two], action: "null")
        #expect(!ready.feeNoteUnderExecute(oneTouchOffered: false), "Execute drawn: its own fee line")
    }

    // MARK: - r15-8: Imported, never "Already added"

    @Test func aSafeImportedByTryAgainSaysImported() throws {
        let imported = try #require(decode(MsJSON.self, #"{"kind":"imported"}"#))
        let already = MultisigAddSheet.importAlready(here: true, outcome: imported)
        #expect(!already)
        #expect(MultisigAddSheet.importStatus(imported, already: already, words: words) == words.t("import.imported"))
        #expect(MultisigAddSheet.importAlready(here: true, outcome: nil))
        #expect(!MultisigAddSheet.importAlready(here: false, outcome: nil))
    }

    // MARK: - iOS import busy, with Try again (R15 not concluded)

    @Test func aBusyImportOffersTryAgainAndItsRowThenSaysImported() throws {
        let safe = MultisigViewsTests.safe
        let busyView = try #require(decode(MsImportView.self, """
        {"safes":[{"address":"\(safe)","chains":[{"chainId":100,"deployed":true}]}],"error":null,"busy":null,
         "outcomes":{"\(safe.lowercased())":{"kind":"busy","chain_id":100}},"retry":["\(safe.lowercased())"]}
        """))
        let busy = busyView.outcomes[safe.lowercased()]
        let here = false
        #expect(busyView.offersRetry(safe) && !here, "Try again is offered")
        let said = MultisigAddSheet.importStatus(busy, already: MultisigAddSheet.importAlready(here: here, outcome: busy), words: words)
        #expect(said == words.t("add.busy", ["network": words.network(100)]))
        // Try again asks the same import again…
        let event = MultisigAddSheet.importEvent(safe)
        #expect(event["type"] as? String == "import" && event["address"] as? String == safe)
        // …and it lands: the Safe is here now and its row says what came of it.
        let done = try #require(decode(MsImportView.self, """
        {"safes":[],"error":null,"busy":null,"outcomes":{"\(safe.lowercased())":{"kind":"imported"}},"retry":[]}
        """))
        let outcome = done.outcomes[safe.lowercased()]
        #expect(!done.offersRetry(safe))
        #expect(MultisigAddSheet.importStatus(outcome, already: MultisigAddSheet.importAlready(here: true, outcome: outcome),
                                              words: words) == words.t("import.imported"))
    }

    // MARK: - r15-9: one check, neutral ink

    @Test func switchAccountWearsOneCheckInNeutralInk() throws {
        let source = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/Features/Settings/SettingsSheet.swift"), encoding: .utf8)
        #expect(!source.contains("row.selected ? theme.accentBase"), "the selected name in neutral ink")
        #expect(source.contains("LucideIcon(.check, size: LucideIconSize.action)\n                            .foregroundStyle(theme.fgBase)"))
        let root = try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("VelaWallet/App/RootView.swift"), encoding: .utf8)
        #expect(root.contains("multisigShown: shownMultisig != nil"), "Settings → Switch account: the multisig's row alone wears the check")
    }

    @Test func aShownMultisigTakesTheCheckFromTheVelaAccountsRow() throws {
        let session = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.interleave,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Interleave", "address": Self.interleave]]],
        ])
        let base = SettingsFixtures.build(.st1, loc: loc)
        let shown = SettingsLive.withAccounts(session: session, balances: [], display: .live(nil, language: "en"),
                                              on: base, loc: loc, multisigShown: true)
        #expect(shown.accountsSheet.rows.allSatisfy { !$0.selected })
        let plain = SettingsLive.withAccounts(session: session, balances: [], display: .live(nil, language: "en"),
                                              on: base, loc: loc)
        #expect(plain.accountsSheet.rows.first?.selected == true)
    }

    /// The Accounts sheet as drawn: the account in front's name and check in
    /// the ink every other row's text wears — no accent anywhere on it.
    @Test func theAccountsSheetDrawsItsCheckInInk() async throws {
        let session = try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": Self.interleave,
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": [["index": 0, "account": ["name": "Interleave", "address": Self.interleave]]],
        ])
        let model = SettingsLive.withAccounts(session: session, balances: [], display: .live(nil, language: "en"),
                                              on: SettingsFixtures.build(.st1, loc: loc), loc: loc)
        #expect(model.accountsSheet.rows.first?.selected == true)
        let screen = HostedScreen(
            SettingsSheet(model: model, overlay: .accounts, loc: loc, onDismiss: {}, onSignOut: {}).themed(.light)
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.says("Interleave") }
        #expect(seen.says("Interleave"))
        // No accent-orange pixel anywhere on the sheet (the light theme's
        // accent is orange-red; ink and greys are not).
        let picture = try await screen.picture(settled: true)
        #expect(Self.accentPixels(picture) == 0, "nothing on Switch account wears the accent")
        try await screen.save("i-r15-9-accounts-after")
    }

    /// How many pixels in the top of the sheet, right of the avatars (an
    /// identicon may be any colour), are the accent's orange (#E8572A).
    private static func accentPixels(_ picture: HostedScreen.Picture) -> Int {
        var count = 0
        for y in stride(from: 0, to: 400, by: 2) {
            for x in stride(from: 80, to: 390, by: 2) {
                let p = picture.pixel(x, y)
                if p.r > 200, p.g > 60, p.g < 140, p.b < 80 { count += 1 }
            }
        }
        return count
    }

    // MARK: - r14-10: "Open Settings" (R15 not concluded: the simulator has no camera)

    @Test func theCamerasWayOutIsLabelledByWhatItDoes() {
        // Refused for good: the button opens this app's Settings — and says so.
        #expect(CameraScanner.Refusal.denied.actionLabel(loc) == "Open Settings")
        #expect(CameraScanner.Refusal.denied.actionLabel(loc) != loc.t("componentsUi.scanner.grantPermission"))
        // Settings cannot grant these: no button at all.
        for refusal in [CameraScanner.Refusal.restricted, .noCamera, .unavailable] {
            #expect(refusal.actionLabel(loc) == nil, "\(refusal)")
        }
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        #expect(CameraScanner.Refusal.denied.actionLabel(zh) == zh.t("onboarding.create.openSettings"))
        #expect(CameraScanner.Refusal.denied.actionLabel(zh) != "Open Settings")
    }

    // MARK: - The newest link wins (R15 not concluded on iOS)

    /// Two links opened one after the other, the OLDER one's multisig looked
    /// up on a slow service: the newer one is shown and opened, and when the
    /// older lookup lands nothing of it is seen — no stand, no switch, no
    /// sheet (the core's `LinkState.seq`, §19.11), through the real accounts
    /// machine and the iOS executor.
    @Test func theNewestLinkWinsOverAnOlderOneStillOpening() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0])
        let me = world.locals[0].address
        let bo = "0x00000000000000000000000000000000000000bb"
        func genesis(_ owners: [String], _ threshold: Int, _ salt: String) throws -> (address: String, g: [String: Any]) {
            let g = try #require(try MsSimWorld.door("genesis", [
                "owners": owners, "threshold": threshold, "name": "Link \(salt)", "saltNonce": salt,
            ]) as? [String: Any])
            return (try #require(g["address"] as? String), g)
        }
        // The older link's multisig is not on this device: looked up, slowly.
        let older = try genesis([me], 1, "0x5b")
        world.records[older.address.lowercased()] = [
            "found": true, "pending": false, "genesis": older.g["genesis"] ?? [:], "recordedOn": [100], "readFrom": 100,
        ]
        world.holdGenesis = [older.address.lowercased()]
        // The newer one's is, with its member here.
        let newer = try genesis([me, bo], 2, "0x5c")
        world.store["vela.multisigAccounts"] = MsSimWorld.text([[
            "address": newer.address, "genesis": newer.g["genesis"] ?? [:], "owners": newer.g["owners"] ?? [me, bo],
            "threshold": 2, "recorded": true, "chainIds": [100], "addedMs": 1,
        ]])

        let machine = MultisigMachine(core: MultisigAccountsCore(), ports: world, name: "accounts")
        defer { machine.dispose() }
        machine.dispatch(["type": "start", "service_url": MsSimWorld.service, "accounts": world.localsJson, "networks": [100]])
        await Wait.until({ machine.accountsView.loaded }, orIdle: { machine.isIdle })
        world.notices.removeAll()

        machine.dispatch(["type": "open_link", "hash": "#ms=100.\(older.address).\(Self.hash)", "take_kept": false])
        await Wait.until { world.httpLog.contains { $0.lowercased().contains("/api/genesis/\(older.address.lowercased())") } }
        #expect(machine.accountsView.link?.kind == "opening")

        machine.dispatch(["type": "open_link", "hash": "#ms=100.\(newer.address).\(Self.hash)", "take_kept": false])
        await Wait.until {
            machine.accountsView.link?.kind == "member"
                && machine.accountsView.active?.caseInsensitiveCompare(newer.address) == .orderedSame
        }

        // The older lookup lands now.
        world.holdGenesis = []
        await Wait.until({ false }, orIdle: { machine.isIdle })
        let link = try #require(machine.accountsView.link)
        #expect(link.kind == "member")
        #expect(link.address?.caseInsensitiveCompare(newer.address) == .orderedSame, "the newest link's stand")
        #expect(machine.accountsView.active?.caseInsensitiveCompare(newer.address) == .orderedSame, "the account shown is the newest link's")
        let opened = world.notices.filter { $0["kind"] as? String == "open_proposal" }
        #expect(opened.count == 1, "\(opened)")
        #expect((opened.first?["address"] as? String)?.caseInsensitiveCompare(newer.address) == .orderedSame)
        #expect(!world.notices.contains {
            $0["kind"] as? String == "activated" && ($0["address"] as? String)?.caseInsensitiveCompare(older.address) == .orderedSame
        }, "the older link switched nothing")
    }
}
#endif
