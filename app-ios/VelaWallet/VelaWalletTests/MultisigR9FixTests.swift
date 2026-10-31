//
//  MultisigR9FixTests.swift
//  VelaWalletTests
//
//  Round 9's iOS findings, each on the model the screen draws from:
//
//  - r9-7: a member still to approve whom the machine's split has not
//    placed yet (a read refused right after this device's own proposal) is
//    asked for again — never left out of every way to approve.
//  - r9-8: a background round says what its OWN read found (it waits for a
//    read made after it began), and a read landing later while away is said.
//  - r9-17: the receipt scan reads the networks the web reads — the space's
//    own for an account holding nothing yet in a space without the main
//    payment chains.
//  - r9-20: a multisig's send form draws who must approve and who pays, no
//    fee and no speed.
//  - S4: the fee before "Approve and execute", History's times, a danger
//    call's intent as a danger tag.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR9FixTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords {
        MsWords(loc: loc, hub: nil, localNames: [MultisigViewsTests.one.lowercased(): "Parallel One"])
    }

    private static let safe = MultisigViewsTests.safe
    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one
    private static let two = MultisigViewsTests.two
    private static let three = MultisigViewsTests.three

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    private func view(item: String, standing: String = #"{"kind":"progress","approved":1,"threshold":2}"#,
                      replacing: [String: String] = [:], extra: String = "") throws -> MsProposalsView {
        var json = MultisigViewsTests.proposalsJson(item: item, standing: standing, extra: extra)
        for (key, value) in replacing {
            json = json.replacingOccurrences(of: "\"\(key)\":{}", with: "\"\(key)\":\(value)")
        }
        return try #require(decode(MsProposalsView.self, json))
    }

    private func model(_ view: MsProposalsView) throws -> MsProposalModel {
        MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    /// Parallel One (here) proposed and approved; two and three still to approve.
    private var proposed: String {
        MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [])
    }

    private func signers(_ rows: [(String, String?)], outside: [String] = [], inVela: [String] = [],
                         either: [String] = [], enough: Bool = false) -> String {
        let r = rows.map { #"{"owner":"\#($0.0)","kind":\#($0.1.map { "\"\($0)\"" } ?? "null")}"# }.joined(separator: ",")
        let list = { (a: [String]) in a.map { "\"\($0)\"" }.joined(separator: ",") }
        return #"{"100:\#(Self.hash)":{"rows":[\#(r)],"outside":[\#(list(outside))],"in_vela":[\#(list(inVela))],"either":[\#(list(either))],"contract":[],"enough":\#(enough)}}"#
    }

    // MARK: - r9-7: an unplaced member is asked for again

    @Test func beforeTheSplitIsInEveryoneElseIsUnread() throws {
        let model = try model(view(item: proposed))
        #expect(model.signers == nil)
        #expect(model.signersUnread, "nothing placed yet: the sheet must ask")
    }

    @Test func aMemberWhoseReadFailedIsUnreadAndOfferedNothing() throws {
        // The r9 sheet: the Vela member placed, the outside owner's read refused.
        let model = try model(view(item: proposed, replacing: [
            "signers": signers([(Self.two, "vela"), (Self.three, nil)], inVela: [Self.two]),
        ]))
        #expect(model.signersUnread)
        #expect(model.outside.isEmpty && model.either.isEmpty, "nothing is offered on a guess")
        #expect(model.outsideLink == nil)
        #expect(!model.offersOutsideWallet)
    }

    @Test func onceEveryMemberIsPlacedNothingIsAskedAgain() throws {
        let model = try model(view(item: proposed, replacing: [
            "signers": signers([(Self.two, "vela"), (Self.three, "wallet")], outside: [Self.three], inVela: [Self.two]),
        ]))
        #expect(!model.signersUnread)
        #expect(model.outsideLink != nil)
        #expect(model.offersOutsideWallet)
    }

    @Test func enoughApprovalsOrAFinishedOneAsksNobody() throws {
        let enough = try model(view(
            item: MultisigViewsTests.item(approved: 2, status: "ready"), standing: #"{"kind":"ready"}"#,
            replacing: ["signers": signers([(Self.three, nil)], enough: true)]
        ))
        #expect(!enough.signersUnread)
        let done = try model(view(item: MultisigViewsTests.item(approved: 2, status: "done"), standing: #"{"kind":"ready"}"#))
        #expect(!done.signersUnread)
    }

    @Test func theRetryIsBoundedPastTheCoresOwnReadBound() {
        // One read is bounded at 8 s (`SIGNER_READ_MS`): a retry never
        // overlaps it, and a network that never answers costs a few reads.
        #expect(MultisigProposalSheet.signersRetrySeconds > 8)
        #expect((1...10).contains(MultisigProposalSheet.signersRetries))
    }

    // MARK: - r9-8: the round says its own read; a late read while away is said

    @Test func aRoundWaitsForAReadMadeAfterItBegan() {
        let since = 1_000_000.0
        #expect(!MultisigHub.roundRead(["read_at": 999_000, "loading": false], since: since),
                "the list as it was before the round: nothing new can be in it")
        #expect(!MultisigHub.roundRead(["read_at": 1_000_500, "loading": true], since: since))
        #expect(!MultisigHub.roundRead(["loading": false], since: since))
        #expect(MultisigHub.roundRead(["read_at": 1_000_500, "loading": false], since: since))
        #expect(MultisigHub.roundWait < 30, "within the time iOS gives a refresh")
    }

    @Test func aViewLandingInTheBackgroundIsSaidOnScreenRecordedInactiveKept() {
        #expect(MultisigHub.notifying(appActive: true, inBackground: false) == false)
        #expect(MultisigHub.notifying(appActive: false, inBackground: true) == true)
        #expect(MultisigHub.notifying(appActive: false, inBackground: false) == nil,
                "under a system sheet: neither said over the app nor recorded unsaid")
    }

    @Test func aNewProposalFromAnotherMemberIsSaidWhenItsReadLandsAway() throws {
        let notifier = MultisigNotifier(loc: loc, defaults: UserDefaults(suiteName: "ms.r9.\(UUID().uuidString)")!)
        var posted: [MultisigNotifier.Note] = []
        notifier.post = { posted.append($0) }
        let accounts = try #require(decode(MsAccountsView.self, MultisigViewsTests.accountsJson))
        let before = try view(item: proposed)
        // On screen: what waits is recorded.
        notifier.evaluate(accounts: accounts, proposals: before, words: words,
                          post: try #require(MultisigHub.notifying(appActive: true, inBackground: false)))
        #expect(posted.isEmpty)
        // Away, the round's own read: another member's proposal needs this device.
        let json = MultisigViewsTests.proposalsJson(item: proposed, standing: #"{"kind":"yourTurn"}"#, ready: 0)
            .replacingOccurrences(of: #""needs_you":0"#, with: #""needs_you":1"#)
        let after = try #require(decode(MsProposalsView.self, json))
        notifier.evaluate(accounts: accounts, proposals: after, words: words,
                          post: try #require(MultisigHub.notifying(appActive: false, inBackground: true)))
        let note = try #require(posted.first { $0.id.hasPrefix("ms-needs-") })
        #expect(note.body == words.t("presence.needsYou", count: 1, ["count": "1"]))
        #expect(note.safeTxHash == Self.hash)
    }

    // MARK: - r9-17: the receipt scan's networks, as the web's

    @Test func theScanReadsHeldNetworksElseTheSpacesOwn() {
        let mainnets = ActivityExecutor.Space(all: [1, 8453, 100, 137], added: [])
        #expect(ActivityExecutor.scanChains(active: [5_042_002], space: mainnets) == [5_042_002], "what it holds")
        #expect(ActivityExecutor.scanChains(active: [], space: mainnets) == ActivityExecutor.defaultMonitorChains)
        // The testnet space: none of the main payment chains — its own
        // networks, the one the person added first, never mainnets.
        let testnet = ActivityExecutor.Space(all: [11_155_111, 84_532, 421_614, 11_155_420, 80_002, 10_200, 5_042_002],
                                             added: [5_042_002])
        let chains = ActivityExecutor.scanChains(active: [], space: testnet)
        #expect(chains.first == 5_042_002)
        #expect(chains.count == 7 && !chains.contains(1))
        #expect(ActivityExecutor.scanChains(active: [], space: .init(all: [], added: [])) == ActivityExecutor.defaultMonitorChains)
    }

    // MARK: - r9-20: the form draws who must approve, no fee, no speed

    @Test func aMultisigsFormSaysWhoMustApproveNotAFeeOrASpeed() throws {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in [
            "stage": "enter_details", "recipient": Self.three,
            "selected_token": [
                "symbol": "xDAI", "network": "Gnosis", "chain_id": 100, "token_address": NSNull(), "decimals": 18,
                "balance": "10", "price_usd": 1.0, "logo_urls": [], "spam": false,
            ] as [String: Any],
            "multisig": ["threshold": 2, "owners": 3],
        ] as [String: Any] {
            object[key] = value
        }
        let send = try CoreJSON.decode(SendViewWire.self, from: object)
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("no form fixture")
            return
        }
        let words = MultisigConfirmWords(label: "Approvals", rule: "Needs 2 of 3 members")
        let form = SendLive.form(send, fee: nil, display: .usd, on: drawn, loc: loc, multisig: words)
        #expect(form.fee.plain)
        #expect(form.fee.label == "Approvals")
        #expect(form.fee.value == "Needs 2 of 3 members")
        #expect(form.fee.staleNote == loc.t("home.multisig.proposal.feeNote"))
        #expect(form.fee.tap == .nothing && form.fee.refreshLabel == nil)
        #expect(form.speed == nil, "no speed to pick: the executor pays")
        // Without words read yet: the multisig's own rule.
        let own = SendLive.form(send, fee: nil, display: .usd, on: drawn, loc: loc)
        #expect(own.fee.value == loc.t("home.multisig.panel.title", vars: ["m": "2", "n": "3"]))
        // A Vela account's form keeps its fee row.
        object["multisig"] = NSNull()
        let plain = SendLive.form(try CoreJSON.decode(SendViewWire.self, from: object), fee: nil, display: .usd, on: drawn, loc: loc)
        #expect(!plain.fee.plain)
        #expect(plain.fee.label == drawn.fee.label)
    }

    // MARK: - S4: the fee before "Approve and execute"

    @Test func theOnePressSaysItsFeeBeforeThePasskey() throws {
        let preview = #"{"100:\#(Self.hash.lowercased()):\#(Self.one.lowercased())":{"kind":"priced","fee":{"amount":"420000000000000"},"canPay":true}}"#
        let model = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one], canExecute: [Self.one]),
            replacing: ["previews": preview],
            extra: #","approve_execute":{"100:\#(Self.hash.lowercased())":["\#(Self.one)"]}"#
        ))
        let presser = try #require(model.oneTouchExecutor(chosen: nil))
        #expect(presser == Self.one)
        #expect(model.oneTouchExecutor(chosen: Self.two) == Self.one, "a member not offered the press is not its executor")
        let line = try #require(model.feeLine(for: presser))
        #expect(line.text.contains("0.00042"))
        #expect(line.tone == .neutral)
    }

    @Test func noOnePressNoExecutor() throws {
        let model = try model(view(item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one])))
        #expect(model.oneTouchExecutor(chosen: Self.one) == nil)
    }

    // MARK: - S4: History's times

    @Test func eachHistoryStepCarriesItsTime() throws {
        let model = try model(view(item: proposed))
        let at = 1_793_000_000_000.0
        let created = try #require(decode(MsTagged.self, #"{"kind":"created","who":"\#(Self.one)","at_ms":\#(at)}"#))
        #expect(model.timelineWhen(created) == Formats.dateTime(Date(timeIntervalSince1970: at / 1000)))
        let untimed = try #require(decode(MsTagged.self, #"{"kind":"approved","who":"\#(Self.one)","at_ms":null}"#))
        #expect(model.timelineWhen(untimed) == nil, "no time the machine has not read")
        #expect(model.timelineWhen(try #require(decode(MsTagged.self, #"{"kind":"executing"}"#))) == nil)
    }

    // MARK: - S4: a danger call's intent is a tag, not red words

    @Test func onlyADangerIntentIsDrawnAsATag() {
        #expect(MsReadingView.isTag(.danger))
        for tone in [SigningTone.neutral, .accent, .success, .caution] {
            #expect(!MsReadingView.isTag(tone))
        }
    }
}
