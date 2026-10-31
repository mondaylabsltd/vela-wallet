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
//  - The core's round-9 fields, drawn: the refusal's own reason (r9-9),
//    nothing in the state guard's place beside it, "none signed in here"
//    only when no member is (members_here), one name per member and the
//    passkey index asked for them (r9-12), the invite from the core's
//    members, plural approval counts, "anyway" to the trusted page (r9-10).
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

    // MARK: - r9-9: the refusal's own reason, said once

    @Test func theRefusalIsTheCoresOwnReasonByItsKey() throws {
        let delegate = try #require(decode(MsJSON.self,
            #"{"sign":{"allowed":false,"anyway":false},"refused":{"why":"delegatecall","key":"home.multisig.review.refusedWhy.delegatecall"}}"#))
        let line = words.refusalLine(delegate)
        #expect(line == loc.t("home.multisig.review.refusedWhy.delegatecall"))
        #expect(line != words.t("review.blocked"), "never 'would change who controls the multisig' for a delegatecall")
        let state = try #require(decode(MsJSON.self,
            #"{"sign":{"allowed":false},"refused":{"why":"stateChange","key":"home.multisig.review.blocked"}}"#))
        #expect(words.refusalLine(state) == words.t("review.blocked"))
        for why in ["tooDeep", "refund", "other"] {
            let review = try #require(decode(MsJSON.self,
                #"{"refused":{"why":"\#(why)","key":"home.multisig.review.refusedWhy.\#(why)"}}"#))
            #expect(words.refusalLine(review) != "home.multisig.review.refusedWhy.\(why)", "a raw key for \(why)")
        }
        #expect(words.refusalLine(nil) == words.t("review.blocked"))
        // The "… anyway" step names the same reason.
        let concern = try #require(decode(MsJSON.self, #"{"kind":"notSigned","why":"delegatecall to a contract"}"#))
        #expect(MsReviewBody.concernText(concern, review: delegate, words: words) == line)
    }

    @Test func aRefusalIsNotSaidTwiceOnTheSheet() throws {
        let item = MultisigViewsTests.item(approved: 1, status: "pending")
            .replacingOccurrences(of: #""refusal":null"#, with: #""refusal":"delegatecall to a contract Vela does not sign for""#)
        let review = #"{"100:\#(Self.hash)":{"sign":{"allowed":false,"anyway":false},"refused":{"why":"delegatecall","key":"home.multisig.review.refusedWhy.delegatecall"},"concerns":[],"state":{"source":"probe","verdict":{"kind":"withheld"}},"simulation":{"kind":"deltas","deltas":[]},"recipients":[],"fields":[]}}"#
        let model = try model(view(item: item, replacing: ["reviews": review]))
        #expect(!model.notes.contains { $0.id == "refused" }, "the review's body says it")
        #expect(!model.notes.contains { $0.text.contains("Vela does not sign for") }, "never the core's English")
        #expect(model.review?["state"]?["verdict"]?.kind == "withheld")
    }

    @Test func noneSignedInHereOnlyWhenNoMemberIsHere() throws {
        let account = try #require(decode(MsAccountsView.self, MultisigViewsTests.accountsJson)).activeRow?.account
        func notes(_ membersHere: String) throws -> [String] {
            let item = MultisigViewsTests.item(approved: 1, status: "pending")
                .replacingOccurrences(of: #""unsent":false"#, with: #""unsent":false,"membersHere":\#(membersHere)"#)
            let view = try view(item: item)
            return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: account)
                .notes.map(\.id)
        }
        // Parallel One here already approved: it can approve nothing more,
        // yet it IS a member here — never "none is signed in".
        #expect(!(try notes(#"["\#(Self.one)"]"#)).contains("notMember"))
        #expect(try notes("[]").contains("notMember"))
    }

    // MARK: - r9-12: one name per member, the core's

    @Test func aMemberIsNamedByTheCoreNotByThisDevice() {
        // This device calls its account "Parallel One"; the passkey index
        // (the core's rule) says "Interleave" — the core's name is drawn.
        let named = MsWords(loc: loc, hub: nil, localNames: [Self.one.lowercased(): "Parallel One"],
                            labels: [Self.one.lowercased(): "Interleave", Self.two.lowercased(): "Bob"])
        #expect(named.label(Self.one) == "Interleave")
        #expect(named.label(Self.two) == "Bob")
        #expect(named.who(Self.one).hasPrefix("Interleave ("))
        // Still this device's account, for what only this device can do.
        #expect(named.localName(Self.one) == "Parallel One")
        // A member the core has not named yet: as before.
        #expect(named.label(Self.three) == MsWords.short(Self.three))
    }

    @Test func theAccountsViewCarriesTheNamesTheIndexWantsAndTheInvite() throws {
        let json = MultisigViewsTests.accountsJson
            .replacingOccurrences(of: #""invite_shared":false}"#, with: #""invite_shared":false,"invite_members":[{"address":"\#(Self.one)","name":"Interleave","kind":"passkey"},{"address":"\#(Self.three)"}]}"#)
            .replacingOccurrences(of: #""member_kinds":{}"#, with: #""member_kinds":{},"member_labels":{"\#(Self.one.lowercased())":"Interleave"},"index_wanted":["\#(Self.two.lowercased())"]"#)
        let view = try #require(decode(MsAccountsView.self, json))
        #expect(view.memberLabels[Self.one.lowercased()] == "Interleave")
        #expect(view.indexWanted == [Self.two.lowercased()])
        let row = try #require(view.activeRow)
        #expect(row.inviteMembers.array.count == 2)
        let link = try #require(MsRules.addLink(origin: "https://getvela.app", locale: "en", multisig: Self.safe,
                                                chainIds: row.account.chainIds, members: row.inviteMembers.any))
        #expect(link.contains("#msa="))
        // The link carries the core's name for each member.
        let parsed = MsRules.call("parseAddLink", ["hash": String(link[link.firstIndex(of: "#")!...])]) as? [String: Any]
        let members = parsed?["members"] as? [[String: Any]] ?? []
        #expect(members.contains { ($0["name"] as? String) == "Interleave" })
    }

    @Test func theIndexIsAskedOnceAndAgainOnlyAfterAWhile() {
        let now = Date()
        let wanted = [Self.one, Self.two.uppercased(), Self.one]
        #expect(MultisigHub.indexDue(wanted, asked: [:], now: now, retry: 300)
            == [Self.one.lowercased(), Self.two.lowercased()])
        let asked = [Self.one.lowercased(): now.addingTimeInterval(-10), Self.two.lowercased(): now.addingTimeInterval(-400)]
        #expect(MultisigHub.indexDue(wanted, asked: asked, now: now, retry: 300) == [Self.two.lowercased()])
    }

    @Test func theIndexsVerdictIsANameAMissOrNothingSaid() async {
        let defaults = UserDefaults(suiteName: "ms.r9.index.\(UUID().uuidString)")!
        let store = VelaStore(defaults: defaults)
        func lookup(_ done: String) -> RegistryNameLookup {
            RegistryNameLookup(store: store, ethCall: { _, _, _ in nil }, indexGet: { _ in .failed }, step: { _, _ in done })
        }
        let hit = await lookup(#"{"type":"done","found":{"name":"Interleave","public_key":"04ab"},"remember":"forever"}"#)
            .lookup(for: Self.one)
        #expect(hit == RegistryNameLookup.Verdict(vela: true, name: "Interleave"))
        let silent = await lookup(#"{"type":"done","found":null,"remember":"no"}"#).lookup(for: Self.two)
        #expect(silent == nil, "nobody answered: nothing is said, asked again later")
        let miss = await lookup(#"{"type":"done","found":null,"remember":"briefly"}"#).lookup(for: Self.three)
        #expect(miss == RegistryNameLookup.Verdict(vela: false, name: nil))
        // Kept: asked again, the store answers.
        let kept = await lookup(#"{"type":"done","found":null,"remember":"no"}"#).lookup(for: Self.one)
        #expect(kept?.name == "Interleave")
    }

    // MARK: - Plurals: "Require 1 approval"

    @Test func approvalCountsArePlural() throws {
        let one = try #require(decode(MsJSON.self, #"{"kind":"changeThreshold","threshold":"1"}"#))
        let two = try #require(decode(MsJSON.self, #"{"kind":"changeThreshold","threshold":"2"}"#))
        #expect(words.actionTitleNow(chainId: 100, action: one, nonce: "0") == "Require 1 approval")
        #expect(words.actionTitleNow(chainId: 100, action: two, nonce: "0") == "Require 2 approvals")
        #expect(words.t("proposal.rejectHint", count: 1, ["count": "1", "threshold": "1"]).contains("1 approval too"))
    }
}
