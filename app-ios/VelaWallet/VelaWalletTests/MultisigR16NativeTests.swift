//
//  MultisigR16NativeTests.swift
//  VelaWalletTests
//
//  Round 16's iOS findings (docs/multisig-machines.md §26):
//
//  - r16-4 (core, iOS adoption): the one press does both or nothing; a fee
//    not worked out at the press is said beside it with Check again.
//  - r16-5 (Android's, iOS parity): a Reject that did not go says why beside
//    Reject, with Try again.
//  - r16-6: Create's other-space line is the core's over the space in front.
//  - r16-7: a quote asked beside another is never answered "failed".
//  - r16-8: while this device executes it, the chip says "Executing…".
//  - r16-9: the create sheet's action is its foot, above the keyboard, and
//    the keyboard always has a way away.
//  - r16-10: one total, from every account's figure; none in the testnet space.
//  - r16-11: a cold start never flashes another account's home.
//

#if DEBUG
import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct MultisigR16NativeTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private static let hash = MultisigViewsTests.hash
    private static let two = MultisigViewsTests.two

    private func source(_ path: String) throws -> String {
        try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent(path), encoding: .utf8)
    }

    /// A 1-of-2 waiting on `two`, whose press would complete it; `look` is
    /// their look, `action` the `execute:` action's state.
    private func model(look: String?, action: String = "null", executing: Bool = false, status: String = "pending",
                       approved: Int = 1) throws -> MsProposalModel {
        var item = MultisigViewsTests.item(approved: approved, status: status, canApprove: approved < 2 ? [Self.two] : [],
                                           canExecute: [Self.two])
        if executing { item = item.replacingOccurrences(of: #""executing":false"#, with: #""executing":true"#) }
        var json = MultisigViewsTests.proposalsJson(
            item: item, standing: #"{"kind":"progress","approved":\#(approved),"threshold":2}"#,
            extra: #","approve_execute":{"100:\#(Self.hash)":["\#(Self.two)"]}"#
        )
        json = json.replacingOccurrences(of: #""actions":{}"#, with: #""actions":{"execute:\#(Self.hash)":\#(action)}"#)
        if let look {
            json = json.replacingOccurrences(of: #""previews":{}"#, with: #""previews":{"100:\#(Self.hash):\#(Self.two.lowercased())":\#(look)}"#)
        }
        let view = try #require(decode(MsProposalsView.self, json))
        return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    // MARK: - r16-4: both or nothing, said with Check again

    @Test func aOnePressStoppedForAFeeItCouldNotWorkOutOffersCheckAgainBesideIt() throws {
        let stop = #"{"kind":"stopped","preview":{"kind":"unpriced"}}"#
        let m = try model(look: #"{"kind":"unpriced"}"#, action: stop)
        let beside = m.pressFailures(at: .approve)
        #expect(beside.map(\.text) == [words.t("proposal.feeUnpriced")], "\(beside)")
        #expect(m.executeStopped, "the stop stands in the fee line's place")
        #expect(m.oneTouchChecksFee(Self.two))
        // Priced: nothing to check again.
        #expect(try !model(look: #"{"kind":"priced","fee":{"amount":"1"},"canPay":true}"#).oneTouchChecksFee(Self.two))
        // An unpriced look with no stop: still offered.
        #expect(try model(look: #"{"kind":"unpriced"}"#).oneTouchChecksFee(Self.two))
        let sheet = try source("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift")
        #expect(sheet.contains("multisig.proposal.oneTouchFeeRetry"))
    }

    // MARK: - r16-5: Reject's failure beside Reject

    @Test func aRejectThatCouldNotBeDraftedSaysWhyBesideReject() throws {
        var m = try model(look: nil)
        m.rejectState = try #require(decode(MsTagged.self, #"{"kind":"unread","reason_key":"home.multisig.proposal.unread.chain"}"#))
        #expect(m.offersReject)
        let line = try #require(m.rejectLine)
        #expect(line.text == words.unreadText("home.multisig.proposal.unread.chain", chainId: 100))
        #expect(line.tone == .warning)
        #expect(!m.notes.contains { $0.id == "rejectFailed" }, "said beside Reject, not at the head")
        let sheet = try source("VelaWallet/Features/Multisig/Views/MultisigProposalSheet.swift")
        #expect(sheet.contains("multisig.proposal.rejectRetry"))
    }

    // MARK: - r16-6: the other space is the core's over the space in front

    @Test func createsOtherSpaceLineFollowsTheSpaceInFront() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0])
        let mainnet = [1, 10]
        let testnet = [11155111, 5042002]
        let localsJson = world.localsJson
        let hub = MultisigHub(
            ports: world, loc: loc,
            shelf: VelaStore(defaults: UserDefaults(suiteName: "vela.tests.r16space.\(UUID().uuidString)")!), tokens: nil,
            facts: { .init(serviceUrl: MsSimWorld.service, accounts: localsJson, networks: mainnet + testnet, space: mainnet, known: []) }
        )
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        await hub.boot()
        #expect(Set(hub.accountsView.create.otherSpace) == Set(testnet), "booted in Mainnet: the testnets are elsewhere")
        // Switched to Testnet: the line names Mainnet's networks, never the
        // testnets the picker offers.
        await hub.spaceChanged(testnet)
        await Wait.until({ Set(hub.accountsView.create.otherSpace) == Set(mainnet) }, orIdle: { hub.accounts.isIdle })
        #expect(Set(hub.accountsView.create.otherSpace) == Set(mainnet))
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains("await multisig.spaceChanged(chains)"))
    }

    // MARK: - r16-7: a quote beside another is never "failed"

    @Test func twoQuotesAskedSideBySideEachGetTheirOwnAnswer() async throws {
        let line = MsQuoteLine()
        var running = 0
        var overlapped = false
        func work(_ id: String) -> @MainActor () async -> MsQuoteLine.Answer {
            {
                running += 1
                if running > 1 { overlapped = true }
                try? await Task.sleep(for: .milliseconds(30))
                running -= 1
                return MsQuoteLine.Answer(["id": id])
            }
        }
        async let a = line.run(key: "a", work("a"))
        async let b = line.run(key: "b", work("b"))
        let (first, second) = await (a, b)
        #expect(first.value["id"] as? String == "a")
        #expect(second.value["id"] as? String == "b")
        #expect(!overlapped, "one quote at a time: none supersedes another")
        // The same ask while one is out shares its answer (asked once).
        var asked = 0
        let same: @MainActor () async -> MsQuoteLine.Answer = {
            asked += 1
            try? await Task.sleep(for: .milliseconds(30))
            return MsQuoteLine.Answer(["n": asked])
        }
        async let c = line.run(key: "same", same)
        async let d = line.run(key: "same", same)
        let (x, y) = await (c, d)
        #expect(asked == 1)
        #expect(x.value["n"] as? Int == y.value["n"] as? Int)
        let live = try source("VelaWallet/Features/Multisig/Shell/MultisigLive.swift")
        #expect(live.contains("await quoteLine.run(key: MsQuoteLine.key(op))"))
    }

    // MARK: - r16-8: "Executing…" while this device executes it

    @Test func whileThisDeviceExecutesItTheChipSaysExecuting() throws {
        let working = try model(look: nil, action: #"{"kind":"working"}"#)
        #expect(working.statusLine == words.t("status.executing"))
        let flying = try model(look: nil, executing: true)
        #expect(flying.statusLine == words.t("status.executing"))
        let idle = try model(look: nil)
        #expect(idle.statusLine != words.t("status.executing"))
    }

    // MARK: - r16-9: Next above the keyboard, and the keyboard dismissible

    @Test func theCreateSheetsActionIsItsFootAndTheKeyboardAlwaysHasAWayAway() throws {
        let components = try source("VelaWallet/Features/Multisig/Views/MultisigComponents.swift")
        #expect(components.contains(".safeAreaInset(edge: .bottom, spacing: 0)"), "the foot rides above the keyboard")
        #expect(components.contains("ToolbarItemGroup(placement: .keyboard)"), "a Done above the keyboard")
        #expect(components.contains(".onSubmit { MsKeyboard.dismiss() }"), "Return puts it away")
        #expect(components.contains(".scrollDismissesKeyboard(.immediately)"))
        #expect(components.contains(".onTapGesture { MsKeyboard.dismiss() }"), "a tap outside puts it away")
        let create = try source("VelaWallet/Features/Multisig/Views/MultisigCreateSheet.swift")
        #expect(create.contains("footer: stepFooter"))
        // Next and Create live in the foot only, never in the scrolling body.
        #expect(create.components(separatedBy: "\"multisig.create.next\"").count - 1 == 1)
        #expect(create.components(separatedBy: "\"multisig.create.create\"").count - 1 == 1)
        let foot = try #require(create.components(separatedBy: "private var stepFooter").last)
        #expect(foot.contains("multisig.create.next") && foot.contains("multisig.create.toReview") && foot.contains("multisig.create.create"))
    }

    /// The create sheet as drawn on the SE's short screen: Next is whole on
    /// screen at open — the sheet's foot, never scrolled out of reach.
    @Test func theCreateSheetsNextIsWholeOnScreenAtOpen() async throws {
        let world = try MsSimWorld()
        world.locals = try MsSimWorld.fixtures([0, 1])
        let localsJson = world.localsJson
        let hub = MultisigHub(
            ports: world, loc: loc,
            shelf: VelaStore(defaults: UserDefaults(suiteName: "vela.tests.r16create.\(UUID().uuidString)")!), tokens: nil,
            facts: { .init(serviceUrl: MsSimWorld.service, accounts: localsJson, networks: [100], space: [100], known: []) }
        )
        defer { hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose() }
        await hub.boot()
        let accounts = world.locals.map { (address: $0.address, name: "Member") }
        let screen = HostedScreen(
            MultisigCreateSheet(hub: hub, words: MsWords(loc: loc, hub: hub), accounts: accounts, contacts: [],
                                spaceNetworks: [100], onClose: {}, onCreated: { _ in }, onInvite: { _ in })
                .themed(.light),
            size: HostedScreen.se
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.element("multisig.create.next") != nil }
        let next = try #require(seen.element("multisig.create.next"))
        #expect(next.frame.maxY <= screen.size.height - screen.safeBottom + 0.5, "Next at \(next.frame) on \(screen.size)")
        #expect(next.frame.minY > 0)
        try await screen.save("i-r16-9-create-next-after")
    }

    // MARK: - r16-10: one total; none in the testnet space

    private func session(_ accounts: [String]) throws -> SessionView {
        try CoreJSON.decode(SessionView.self, from: [
            "loading": false, "has_wallet": true, "address": accounts[0],
            "active_index": 0, "allowed_route": "wallet", "sign_out": NSNull(),
            "accounts": accounts.enumerated().map { ["index": $0.offset, "account": ["name": "A\($0.offset)", "address": $0.element]] },
        ])
    }

    @Test func theAccountsTotalIsEveryAccountsFigureOrNone() throws {
        let a = "0x00000000000000000000000000000000000000a1"
        let b = "0x00000000000000000000000000000000000000b2"
        let display = WalletLive.Display.from(CurrencyViewWire(code: "CNY", rate: 7, committed: true), language: "en")
        let base = SettingsFixtures.build(.st1, loc: loc)
        let both = SettingsLive.withAccounts(session: try session([a, b]),
                                             balances: [BalanceCacheEntryWire(address: a, usd: 0.5), BalanceCacheEntryWire(address: b, usd: 0.3)],
                                             display: display, on: base, loc: loc)
        #expect(both.accountsSheet.summary.contains(display.fiat(0.8) ?? "?"), "\(both.accountsSheet.summary)")
        // One figure not known: never summed as zero — the count alone.
        let one = SettingsLive.withAccounts(session: try session([a, b]),
                                            balances: [BalanceCacheEntryWire(address: a, usd: 0.5)],
                                            display: display, on: base, loc: loc)
        #expect(!one.accountsSheet.summary.contains(display.fiat(0.5) ?? "?"), "\(one.accountsSheet.summary)")
        #expect(one.accountsSheet.summary == SettingsLive.accountsSummary(count: loc.t(I18nKeys.SettingsUi.accountsCount, count: 2), total: nil))
        #expect(SettingsLive.accountsTotal([0.5, nil]) == nil)
        #expect(SettingsLive.accountsTotal([]) == nil)
        // The testnet space: no worth on a row, no total — as its home says.
        let testnet = SettingsLive.withAccounts(session: try session([a, b]),
                                                balances: [BalanceCacheEntryWire(address: a, usd: 0.5), BalanceCacheEntryWire(address: b, usd: 0.3)],
                                                display: display, on: base, loc: loc, noValue: true)
        #expect(testnet.accountsSheet.rows.allSatisfy { $0.amount.isEmpty })
        #expect(testnet.accountsSheet.summary == SettingsLive.accountsSummary(count: loc.t(I18nKeys.SettingsUi.accountsCount, count: 2), total: nil))
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains("noValue: space.current == .testnet"))
    }

    @Test func theTestnetAccountsSheetShowsNoWorth() async throws {
        let a = "0x00000000000000000000000000000000000000a1"
        let b = "0x00000000000000000000000000000000000000b2"
        let display = WalletLive.Display.from(CurrencyViewWire(code: "CNY", rate: 7, committed: true), language: "en")
        let model = SettingsLive.withAccounts(session: try session([a, b]),
                                              balances: [BalanceCacheEntryWire(address: a, usd: 0.5), BalanceCacheEntryWire(address: b, usd: 0.3)],
                                              display: display, on: SettingsFixtures.build(.st1, loc: loc), loc: loc, noValue: true)
        let screen = HostedScreen(
            SettingsSheet(model: model, overlay: .accounts, loc: loc, onDismiss: {}, onSignOut: {}).themed(.light)
        )
        defer { screen.close() }
        let seen = try await screen.read { $0.says("A0") }
        #expect(!seen.says("CN¥"), "\(seen.words)")
        try await screen.save("i-r16-10-testnet-accounts-after")
    }

    // MARK: - r16-11: a cold start on the account in front

    @Test func aColdStartHoldsTheHomeUntilTheMultisigInFrontIsRead() throws {
        let store = VelaStore(defaults: UserDefaults(suiteName: "vela.tests.r16launch.\(UUID().uuidString)")!)
        #expect(LaunchFront.storedMultisig(store) == nil)
        #expect(!LaunchFront.holds(storedMultisig: nil, hubLoaded: false, expired: false), "a Vela account in front: no hold")
        store.writeString(LaunchFront.activeKey, MultisigViewsTests.safe)
        let stored = LaunchFront.storedMultisig(store)
        #expect(stored?.caseInsensitiveCompare(MultisigViewsTests.safe) == .orderedSame)
        #expect(LaunchFront.holds(storedMultisig: stored, hubLoaded: false, expired: false))
        #expect(!LaunchFront.holds(storedMultisig: stored, hubLoaded: true, expired: false), "read: the multisig's home")
        #expect(!LaunchFront.holds(storedMultisig: stored, hubLoaded: false, expired: true), "bounded")
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains("LaunchFront.holds(storedMultisig: launchMultisig, hubLoaded: multisig.loaded, expired: launchHoldOver)"))
    }
}
#endif
