//
//  MultisigR8FixTests.swift
//  VelaWalletTests
//
//  Round 8's iOS findings, each on the model the screen draws from — the
//  core's views as they come, nothing re-derived:
//
//  - r8-1: the approve sheet signs only from the core's review — nothing
//    while it is out, nothing when its rule refuses, and "… anyway" (danger,
//    with a step naming the reasons) when its rule asks for that.
//  - r8-8: the outside-wallet path from the machine's split of the members:
//    the connect link and its code for an outside wallet, the Vela link only
//    for the Vela members, "Approve with another wallet".
//  - r8-9: a multisig's send confirm names the multisig as the source and
//    says who must approve where a fee would stand.
//  - r8-14: each thing the review says is said once.
//  - r8-17: an executed transaction as it was when it ran (who executed it,
//    whose approvals counted — never "1/2 approved, Waiting"), and its
//    Activity row is the multisig's transaction, not a local record.
//  - r8-19: the review's wait is bounded.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigR8FixTests {

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

    /// The proposals view with `item`, its standing, and these extra fields
    /// replacing the empty ones (`"reviews":{…}` and so on).
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

    private static func review(allowed: Bool = true, anyway: Bool = false, concerns: String = "[]",
                               state: String = #"{"source":"probe","verdict":{"kind":"unchanged"}}"#,
                               simulation: String = #"{"kind":"deltas","deltas":[]}"#) -> String {
        """
        {"\(100):\(hash)":{"sign":{"allowed":\(allowed),"anyway":\(anyway)},"concerns":\(concerns),
         "state":\(state),"simulation":\(simulation),"recipients":[],"fields":[],
         "hashes":{"domainHash":"0xd","messageHash":"0xm","safeTxHash":"\(hash)"}}}
        """
    }

    // MARK: - r8-1: signing only from the core's review, with its "anyway"

    @Test func nothingIsSignableWhileTheReviewIsOut() throws {
        let model = try model(view(item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one])))
        #expect(model.reviewState == .holding)
        #expect(model.reviewHolding)
        #expect(!model.signable, "an approval before the review says what is signed is blind")
        #expect(!model.signAnyway)
    }

    @Test func aReviewThatAsksForAnywayLabelsAndStepsBeforeSigning() throws {
        let concerns = #"[{"kind":"unsimulated"},{"kind":"blind"}]"#
        let model = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]),
            replacing: ["reviews": Self.review(anyway: true, concerns: concerns)]
        ))
        #expect(model.signable)
        #expect(model.signAnyway)
        #expect(model.signLabel("proposal.approve", anyway: "review.approveAnyway") == words.t("review.approveAnyway"))
        #expect(model.signLabel("proposal.execute", anyway: "review.executeAnyway") == words.t("review.executeAnyway"))
        // The step names what the person signs past, in the review's words.
        #expect(model.anywayReasons == [
            loc.t("componentsUi.signing.simUnavailableWarning"), words.t("action.unsupported"),
        ])
        #expect(MsSignRule(review: model.review) == MsSignRule(review: model.review))
        #expect(MsSignRule(review: model.review).anyway)
    }

    @Test func aReviewThatRefusesOffersNoSignatureAndNoOverride() throws {
        let model = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]),
            replacing: ["reviews": Self.review(allowed: false, anyway: false, concerns: #"[{"kind":"stateChange"}]"#)]
        ))
        #expect(!model.signAllowed)
        #expect(!model.signable)
        #expect(!MsSignRule(review: model.review).allowed)
    }

    @Test func aPlainReviewSignsPlainly() throws {
        let model = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]),
            replacing: ["reviews": Self.review()]
        ))
        #expect(model.signable)
        #expect(!model.signAnyway)
        #expect(model.signLabel("proposal.approve", anyway: "review.approveAnyway") == words.t("proposal.approve"))
    }

    // MARK: - r8-14: each thing said once

    @Test func onlyWhatNoBlockSaysIsAConcernLine() {
        let review = MsJSON(any: ["concerns": [
            ["kind": "unsimulated"], ["kind": "firstTime", "address": Self.three],
            ["kind": "toContract", "address": Self.three], ["kind": "stateUnread", "reason": "x"],
            ["kind": "notSigned", "why": "delegatecall"], ["kind": "blind"],
        ]])
        // The verdict's place says "couldn't check", the recipient's row its
        // first time and its contract, the guard its own, the sheet's note
        // the refusal: only the unread call is left for a line of its own.
        #expect(MsReviewBody.ownLines(review).map(\.kind) == ["blind"])
        #expect(MsReviewBody.concernText(MsJSON(any: ["kind": "stateUnread", "reason": "x"]), words: words) == nil)
        #expect(MsReviewBody.tone(MsJSON(any: ["kind": "lookalike"])) == .danger)
        #expect(MsReviewBody.tone(MsJSON(any: ["kind": "blind"])) == .warning)
    }

    // MARK: - r8-19: the review's wait is bounded

    @Test func anUnreadReviewIsAskedAgainSoonAndTheWaitIsCapped() throws {
        let unread = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]),
            replacing: ["reviews": Self.review(state: #"{"source":"none","verdict":{"kind":"unavailable","reason":"unreadable"}}"#)]
        ))
        #expect(unread.reviewUnread)
        let failed = try model(view(
            item: MultisigViewsTests.item(approved: 1, status: "pending", canApprove: [Self.one]),
            replacing: ["reviews": #"{"100:\#(Self.hash)":null}"#]
        ))
        #expect(failed.reviewState == .failed)
        #expect(failed.reviewUnread)
        // Bounded by the machine's own cap on a look (`PREVIEW_CAP_MS`).
        #expect(MultisigProposalSheet.reviewWaitCap == .seconds(15))
        #expect(MultisigProposalSheet.reviewRetrySeconds < MultisigProposalSheet.reviewFreshSeconds)
    }

    // MARK: - r8-8: the outside-wallet path

    @Test func anOutsideMemberGetsTheConnectLinkAndTheVelaMemberTheVelaLink() throws {
        let signers = #"{"100:\#(Self.hash)":{"rows":[{"owner":"\#(Self.two)","kind":"vela"},{"owner":"\#(Self.three)","kind":"wallet"}],"outside":["\#(Self.three)"],"in_vela":["\#(Self.two)"],"either":[],"contract":[],"enough":false}}"#
        let model = MsProposalModel(
            item: try #require(view(
                item: MultisigViewsTests.item(approved: 1, status: "pending"), replacing: ["signers": signers]
            ).items.first),
            view: try view(item: MultisigViewsTests.item(approved: 1, status: "pending"), replacing: ["signers": signers]),
            words: words, account: nil
        )
        #expect(model.stillNeeded.map { $0.lowercased() } == [Self.two.lowercased(), Self.three.lowercased()])
        #expect(model.outside == [Self.three])
        #expect(model.inVela == [Self.two])
        #expect(model.velaMissing == [Self.two], "the Vela link is never offered to an outside wallet")
        #expect(model.showsVelaLink)
        #expect(model.signerLines.map(\.id) == ["inVela"])
        let link = try #require(model.outsideLink)
        #expect(link.hasPrefix(MultisigServiceURL.connectPage()), "the connect page: \(link)")
        #expect(model.offersOutsideWallet)
    }

    @Test func enoughApprovalsAskNobody() throws {
        let signers = #"{"100:\#(Self.hash)":{"rows":[],"outside":["\#(Self.three)"],"in_vela":[],"either":[],"contract":[],"enough":true}}"#
        let model = try model(view(item: MultisigViewsTests.item(approved: 2, status: "ready", canExecute: [Self.one]),
                                   standing: #"{"kind":"ready"}"#, replacing: ["signers": signers]))
        #expect(model.outside.isEmpty)
        #expect(model.outsideLink == nil)
        #expect(!model.showsVelaLink)
        #expect(!model.offersOutsideWallet)
    }

    @Test func aMemberNothingSaysYetIsOfferedBothWays() throws {
        let signers = #"{"100:\#(Self.hash)":{"rows":[{"owner":"\#(Self.three)","kind":"unsure"}],"outside":[],"in_vela":[],"either":["\#(Self.three)"],"contract":[],"enough":false}}"#
        let model = try model(view(item: MultisigViewsTests.item(approved: 1, status: "pending"), replacing: ["signers": signers]))
        #expect(model.either == [Self.three])
        #expect(model.offersOutsideWallet)
        #expect(model.signerLines.map(\.id) == ["either"])
        #expect(model.outsideLink == nil, "no outside link for one that may be a Vela member")
    }

    // MARK: - r8-17: as it was when it ran

    @Test func anOutsideOwnerWhoApprovedByExecutingIsCountedAndNamed() throws {
        // Approved by Parallel One; the outside owner executed it, approving
        // as msg.sender: the receipt's facts, never today's count.
        let frozen = #"{"100:\#(Self.hash.lowercased())":{"executor":"\#(Self.three)","threshold":2,"approvers":["\#(Self.one)","\#(Self.three)"],"executorApproved":true,"source":"chain","atMs":1761900000000}}"#
        var json = MultisigViewsTests.proposalsJson(item: MultisigViewsTests.item(approved: 1, status: "done"),
                                                    standing: #"{"kind":"progress","approved":1,"threshold":2}"#)
        json = json.replacingOccurrences(of: #""frozen":{}"#, with: #""frozen":"# + frozen)
        json = json.replacingOccurrences(of: #""executions":{}"#, with: #""executions":{"100:\#(Self.hash)":{"kind":"executed","txHash":"0xe1"}}"#)
        let view = try #require(decode(MsProposalsView.self, json))
        let model = try model(view)
        #expect(model.past)
        #expect(model.statusLine == words.t("proposal.executed"), "not \"1/2 approved\"")
        #expect(model.approvalsCount == words.t("proposal.executed"))
        #expect(model.listed == [Self.one, Self.three])
        #expect(model.memberWord(Self.three) == .executed, "not \"Waiting\"")
        #expect(model.memberWord(Self.one) == .approved)
        let facts = try #require(model.executedFacts)
        #expect(facts.contains(words.t("proposal.executedBy", ["name": MsWords.short(Self.three)])))
        #expect(facts.contains("2 of 2"))
        // Nothing more is asked of anyone, nor checked.
        #expect(model.stillNeeded.isEmpty)
        #expect(!model.showsVelaLink)
        #expect(model.outsideLink == nil)
    }

    @Test func aFinishedOneWithoutItsFactsListsOnlyWhoApproved() throws {
        var json = MultisigViewsTests.proposalsJson(item: MultisigViewsTests.item(approved: 1, status: "done"),
                                                    standing: #"{"kind":"progress","approved":1,"threshold":2}"#)
        json = json.replacingOccurrences(of: #""executions":{}"#, with: #""executions":{"100:\#(Self.hash)":{"kind":"executed","txHash":"0xe1"}}"#)
        let model = try model(try #require(decode(MsProposalsView.self, json)))
        #expect(model.listed == [Self.one], "never a Waiting row on one that can no longer move")
        #expect(model.executedFacts == words.t("proposal.approvalsSigned"))
    }

    @Test func anExecutedRowIsTheMultisigsTransactionNotALocalRecord() {
        let row = MsRules.executedRowId(chainId: 100, safeTxHash: Self.hash, index: 0)
        let tx = RootView.multisigTransaction(ofRow: row, feed: nil)
        #expect(tx?.chainId == 100)
        #expect(tx?.safeTxHash.lowercased() == Self.hash.lowercased())
        #expect(RootView.multisigTransaction(ofRow: "0xabc", feed: nil) == nil)
        #expect(RootView.multisigTransaction(ofRow: nil, feed: nil) == nil)
    }

    // MARK: - r8-9: the multisig is the source, its rule where the fee was

    @Test func theConfirmSaysWhoMustApproveOnTheSendsNetwork() throws {
        let ref = SendMultisigRefWire(threshold: 2, owners: 3)
        // Its members on the network now: one was added since.
        let members = try #require(decode(MsChainMembers.self,
            #"{"deployed":true,"owners":["\#(Self.one)","\#(Self.two)","\#(Self.three)","0x0000000000000000000000000000000000000004"],"threshold":2,"nonce":"3"}"#))
        let there = MultisigConfirmWords.of(ref: ref, members: members, loc: loc)
        #expect(there.label == loc.t("home.multisig.proposal.approvals"))
        #expect(there.rule == loc.t("home.multisig.panel.title", vars: ["m": "2", "n": "4"]))
        // Not read there yet: its own rule.
        #expect(MultisigConfirmWords.of(ref: ref, members: nil, loc: loc).rule
            == loc.t("home.multisig.panel.title", vars: ["m": "2", "n": "3"]))
        #expect(MultisigConfirmWords.rule(m: 1, n: 1, loc: loc) == loc.t("home.multisig.panel.titleSolo"))
    }

    @Test func theConfirmNamesTheMultisigAndItsRuleNotAFee() throws {
        var object = try CoreJSON.object(SendCore().view())
        for (key, value) in [
            "stage": "confirm", "recipient": Self.three, "confirm_amount": "0.05",
            "selected_token": [
                "symbol": "xDAI", "network": "Gnosis", "chain_id": 100, "token_address": NSNull(), "decimals": 18,
                "balance": "10", "price_usd": 1.0, "logo_urls": [], "spam": false,
            ] as [String: Any],
            "multisig": ["threshold": 2, "owners": 4],
        ] as [String: Any] {
            object[key] = value
        }
        let send = try CoreJSON.decode(SendViewWire.self, from: object)
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            Issue.record("no confirm fixture")
            return
        }
        let words = MultisigConfirmWords(label: "Approvals", rule: "Needs 2 of 4 members")
        let model = SendLive.confirm(send, from: (address: Self.safe, name: "R8 Team"), display: .usd, on: drawn, loc: loc,
                                     multisig: words)
        let from = try #require(model.facts.first)
        #expect(from.value == "R8 Team")
        if case .identicon(let seed)? = from.lead { #expect(seed == Self.safe) } else { Issue.record("no face") }
        let rule = try #require(model.facts.first { $0.label == "Approvals" })
        #expect(rule.value == "Needs 2 of 4 members")
        #expect(rule.note == loc.t("home.multisig.proposal.feeNote"))
        #expect(!model.facts.contains { $0.label == loc.t("send.feeSpeedLabel") })
        #expect(model.cta == loc.t("home.multisig.send.propose"))
    }
}
