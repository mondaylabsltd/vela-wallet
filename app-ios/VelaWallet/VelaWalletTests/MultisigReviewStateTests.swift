//
//  MultisigReviewStateTests.swift
//  VelaWalletTests
//
//  §12 (docs/multisig-machines.md): every reviewed thing has a state —
//  `loading`, `ready`, `failed {reason_key}` — and a sign / propose /
//  approve / execute button opens ONLY on the core's sign door
//  (`review.door`): ready, and the review's own rule allows it.
//
//  The iOS lane's bug this pins: the draft sheet read a missing review as
//  allowed (`review?["sign"]?["allowed"]?.bool ?? true`), so "Propose and
//  approve" could be pressed before any review was seen; and a review that
//  failed looked like one still loading.
//
//  The real machine's half (the draft drawn before its review, the core's
//  backstop, `review_again`) is in `MultisigFlowTests`.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct MultisigReviewStateTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private var words: MsWords { MsWords(loc: loc, hub: nil) }
    private static let hash = MultisigViewsTests.hash
    private static let one = MultisigViewsTests.one

    private static let vectorsURL = URL(fileURLWithPath: #filePath)
        .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
        .deletingLastPathComponent()
        .appendingPathComponent("rust/crates/vela-core/tests/vectors/multisig/review-state.json")

    private static func vectors() throws -> [String: Any] {
        let data = try Data(contentsOf: vectorsURL)
        return try #require(try JSONSerialization.jsonObject(with: data) as? [String: Any])
    }

    private func decode<T: Decodable>(_ type: T.Type, _ json: String) -> T? {
        MultisigMachine.decode(type, from: try! JSONSerialization.jsonObject(with: Data(json.utf8)))
    }

    // MARK: - The shared vectors, through the shell's own call

    /// Every `door` case of the shared vectors, replayed through
    /// `MsRules.reviewDoor` (the link door's `review.door`) — exactly what
    /// the sheets ask.
    @Test func theDoorVectorsReplayThroughTheShellsLinkDoor() throws {
        let cases = try #require(try Self.vectors()["door"] as? [[String: Any]])
        #expect(cases.count >= 10)
        for c in cases {
            let name = c["name"] as? String ?? "?"
            let args = try #require(c["args"] as? [String: Any], "\(name)")
            let expect = try #require(c["expect"] as? [String: Any], "\(name)")
            let state = args["state"].map { MsJSON(any: $0) }
            let review = args["review"].map { MsJSON(any: $0) }
            // R22: the person's "Refuse blind signing" as the case states it.
            let door = MsRules.reviewDoor(state: state, review: review, refuseBlind: args["refuseBlind"] as? Bool ?? false)
            #expect(door.shows.rawValue == expect["shows"] as? String, "\(name): shows \(door.shows)")
            #expect(door.holdMs == (expect["hold_ms"] as? NSNumber)?.intValue, "\(name): hold_ms \(door.holdMs)")
            #expect(door.enabled == (expect["enabled"] as? Bool), "\(name): enabled")
            #expect(door.anyway == (expect["anyway"] as? Bool), "\(name): anyway")
            #expect(door.reasonKey == expect["reason_key"] as? String, "\(name): reason_key")
            #expect(door.retry == (expect["retry"] as? Bool), "\(name): retry")
        }
    }

    /// A door the core did not answer opens nothing.
    @Test func anUnansweredDoorIsShut() {
        for answer: Any? in [nil, "nope", ["shows": "whatever", "enabled": true], ["enabled": true]] {
            let door = MsSignDoor(answer: answer)
            #expect(!door.enabled && !door.anyway, "\(String(describing: answer))")
            #expect(door.shows == .failed && door.retry)
        }
        #expect(!MsSignDoor(answer: ["shows": "review"]).enabled, "no `enabled`: shut")
    }

    /// Every word the state is said with, in every locale the app ships
    /// (the on-demand multisig catalog, copied from `assets/i18n/multisig`).
    @Test func theStateWordsAreInEveryLocale() throws {
        let keys = try #require(try Self.vectors()["keys"] as? [String])
        #expect(keys.count == 8) // r14-13: + chainUnread
        for tag in Loc.supported {
            let loc = Loc(overrideTag: tag, preferredLanguages: [])
            for key in keys {
                let text = loc.t(key)
                #expect(text != key && !text.isEmpty, "\(tag): \(key) is missing")
            }
        }
        #expect(loc.t("home.multisig.review.state.loading") == "Reading the transaction…")
        #expect(loc.t("home.multisig.review.state.tryAgain") == "Try again")
    }

    /// Round 14 (r14-13): a review whose network answered none of its reads
    /// is failed — never "… anyway" — said with the network's name, and
    /// §18 (b): a sheet opened without a link whose service did not answer
    /// says so, with Try again (the core's `review.missing`).
    @Test func aReviewItsNetworkNeverAnsweredIsFailedWithItsNetworkNamed() throws {
        let door = MsRules.reviewDoor(
            state: MsJSON(any: ["kind": "failed", "reason_key": "home.multisig.review.state.chainUnread"]),
            review: nil
        )
        #expect(!door.enabled && door.retry)
        let words = MsWords(loc: Loc(overrideTag: "en", preferredLanguages: []), hub: nil)
        let line = MsReviewPending.failedLine(door, words: words, network: "Arc Testnet")
        #expect(line.hasPrefix("Couldn't reach Arc Testnet to read this transaction."), "\(line)")
        let missing = MsRules.missingSaid(loading: false, unreadable: false, mailbox: false)
        #expect(missing.key == "home.multisig.proposal.unread.linkService" && missing.retry)
        #expect(MsRules.missingSaid(loading: false, unreadable: false, mailbox: true).retry == false)
    }

    // MARK: - The draft sheet ("Propose and approve", "Approve and execute")

    private func draft(state: String?, review: String = #"{"sign":{"allowed":true,"anyway":false},"concerns":[]}"#) throws -> MsDraft {
        let item = MultisigViewsTests.item(approved: 0, status: "pending")
        let stateField = state.map { #","reviewState":\#($0)"# } ?? ""
        return try #require(decode(MsDraft.self, """
            {"item":\(item),"review":\(review)\(stateField),"membersHere":["\(Self.one)"],"defaultMember":"\(Self.one)","executeNow":false}
            """))
    }

    /// The bug: a draft whose review is not read yet is never pressable —
    /// not with no review, not with one from before, not from a core that
    /// says nothing of its state.
    @Test func proposeIsShutWhileTheDraftsReviewIsLoading() throws {
        for d in [
            try draft(state: #"{"kind":"loading"}"#, review: "null"),
            try draft(state: #"{"kind":"loading"}"#),
            try draft(state: nil, review: "null"),
            try draft(state: nil),
        ] {
            let door = MultisigDraftReviewSheet.door(d)
            #expect(!door.enabled, "pressable before its review: \(String(describing: d.reviewState))")
            #expect(door.shows == .loading)
            #expect(!door.retry)
        }
    }

    /// Failed: shut, its reason with what it means, and Try again.
    @Test func proposeIsBlockedWhenTheDraftsReviewFailed() throws {
        let d = try draft(state: #"{"kind":"failed","reason_key":"home.multisig.review.state.timedOut"}"#, review: "null")
        let door = MultisigDraftReviewSheet.door(d)
        #expect(!door.enabled)
        #expect(door.shows == .failed)
        #expect(door.retry)
        #expect(door.reasonKey == "home.multisig.review.state.timedOut")
        let line = MsReviewPending.failedLine(door, words: words)
        #expect(line == "Couldn't read it in time. Nothing can be signed until it's read.", "\(line)")
    }

    /// Read again (`review_again`) and ready: open on the review's own rule —
    /// and still shut where that rule refuses, or says nothing.
    @Test func proposeOpensOnceTheDraftsReviewIsReadyAndItsRuleAllows() throws {
        let open = MultisigDraftReviewSheet.door(try draft(state: #"{"kind":"ready"}"#))
        #expect(open.enabled && !open.anyway && open.shows == .review)
        let anyway = MultisigDraftReviewSheet.door(try draft(
            state: #"{"kind":"ready"}"#, review: #"{"sign":{"allowed":true,"anyway":true},"concerns":[]}"#))
        #expect(anyway.enabled && anyway.anyway)
        let refused = MultisigDraftReviewSheet.door(try draft(
            state: #"{"kind":"ready"}"#, review: #"{"sign":{"allowed":false,"anyway":false}}"#))
        #expect(!refused.enabled && refused.shows == .review)
        let silent = MultisigDraftReviewSheet.door(try draft(state: #"{"kind":"ready"}"#, review: "{}"))
        #expect(!silent.enabled, "a review without a sign rule is never allowed by default")
    }

    // MARK: - The proposal sheet (Approve, Approve and execute, Execute)

    private func proposal(reviews: String, states: String) throws -> MsProposalModel {
        var json = MultisigViewsTests.proposalsJson(
            item: MultisigViewsTests.item(approved: 2, status: "ready", canApprove: [Self.one], canExecute: [Self.one]),
            standing: #"{"kind":"ready"}"#
        )
        json = json.replacingOccurrences(of: #""reviews":{}"#, with: #""reviews":\#(reviews)"#)
        json = json.replacingOccurrences(of: #""review_states":{}"#, with: #""review_states":\#(states)"#)
        let view = try #require(decode(MsProposalsView.self, json))
        return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
    }

    private var allowed: String { #"{"100:\#(Self.hash)":{"sign":{"allowed":true,"anyway":false},"concerns":[]}}"# }

    @Test func approveAndExecuteAreShutWhileAProposalsReviewIsLoading() throws {
        // Not asked yet (no state): loading.
        let unasked = try proposal(reviews: "{}", states: "{}")
        #expect(unasked.reviewState == .loading && !unasked.signable)
        // Read again over a review from before: still shut until it is read.
        let again = try proposal(reviews: allowed, states: #"{"100:\#(Self.hash)":{"kind":"loading"}}"#)
        #expect(again.reviewState == .loading)
        #expect(!again.signable, "the review from before is not this read")
        #expect(again.review == nil)
    }

    @Test func approveAndExecuteAreBlockedWhenAProposalsReviewFailed() throws {
        let failed = try proposal(
            reviews: #"{"100:\#(Self.hash)":null}"#,
            states: #"{"100:\#(Self.hash)":{"kind":"failed","reason_key":"home.multisig.review.state.unreadable"}}"#
        )
        #expect(failed.reviewState == .failed)
        #expect(!failed.signable && !failed.signAnyway)
        #expect(failed.door.retry)
        #expect(MsReviewPending.failedLine(failed.door, words: words)
                == "Couldn't read this transaction. Nothing can be signed until it's read.")
        // Not asked again behind the person's back: Try again is theirs.
        #expect(!failed.reviewUnread)
    }

    // MARK: - D1: neither safety check read (real-device pass 2)

    /// Each read gave up at its bound: the state guard and the simulation
    /// both unread. Drawn (`ready`), but in danger with Try again, and every
    /// press — Approve, Approve and execute, Execute, Propose and approve —
    /// goes through the "… anyway" step.
    private static let unchecked = #"{"sign":{"allowed":true,"anyway":true},"concerns":[{"kind":"stateUnread","reason":"unreachable"},{"kind":"unsimulated"},{"kind":"unchecked"}]}"#
    private static let guardUnread = #"{"sign":{"allowed":true,"anyway":false},"concerns":[{"kind":"stateUnread","reason":"unreachable"}]}"#

    @Test func aReviewNeitherCheckReadSignsOnlyAnywayUnderADangerLine() throws {
        let model = try proposal(reviews: #"{"100:\#(Self.hash)":\#(Self.unchecked)}"#,
                                 states: #"{"100:\#(Self.hash)":{"kind":"ready"}}"#)
        #expect(model.signable)
        #expect(model.door.unchecked && model.door.retry)
        #expect(model.signAnyway, "one plain tap on a review nothing checked")
        #expect(MsReviewPending.uncheckedLine(model.door, words: words)
                == "Vela couldn't check what this transaction changes.")
        #expect(model.signLabel("proposal.approveAndExecute", anyway: "review.approveExecuteAnyway")
                == words.t("review.approveExecuteAnyway"))
        #expect(model.signLabel("proposal.approve", anyway: "review.approveAnyway") == words.t("review.approveAnyway"))
        // A draft: "Propose and approve" the same.
        let door = MultisigDraftReviewSheet.door(try draft(state: #"{"kind":"ready"}"#, review: Self.unchecked))
        #expect(door.enabled && door.anyway && door.unchecked && door.retry)
    }

    /// D8 (real-device pass 2): while the review is read and Approve is shut,
    /// the fee line said "you can still approve". It says only what is true:
    /// Execute checks again; "you can still approve" where Approve opens.
    @Test func theFeeLineNeverSaysYouCanApproveUnderAShutApprove() throws {
        func model(_ states: String) throws -> MsProposalModel {
            var json = MultisigViewsTests.proposalsJson(
                item: MultisigViewsTests.item(approved: 2, status: "ready", canApprove: [Self.one], canExecute: [Self.one]),
                standing: #"{"kind":"ready"}"#
            )
            json = json.replacingOccurrences(of: #""reviews":{}"#, with: #""reviews":\#(allowed)"#)
            json = json.replacingOccurrences(of: #""review_states":{}"#, with: #""review_states":\#(states)"#)
            json = json.replacingOccurrences(
                of: #""previews":{}"#,
                with: #""previews":{"100:\#(Self.hash.lowercased()):\#(Self.one.lowercased())":{"kind":"unchecked"}}"#)
            let view = try #require(decode(MsProposalsView.self, json))
            return MsProposalModel(item: try #require(view.items.first), view: view, words: words, account: nil)
        }
        let reading = try model(#"{"100:\#(Self.hash)":{"kind":"loading"}}"#)
        #expect(!reading.signable)
        #expect(reading.feeLine(for: Self.one)?.text == "Couldn’t check whether it would run — Execute checks again.")
        let open = try model(#"{"100:\#(Self.hash)":{"kind":"ready"}}"#)
        #expect(open.signable)
        #expect(open.feeLine(for: Self.one)?.text
                == "Couldn’t check whether it would run — you can still approve; Execute checks again.")
    }

    /// One of the two checks read keeps today's caution: no danger line, a
    /// plain press.
    @Test func oneCheckUnreadAloneStaysAPlainCaution() throws {
        let model = try proposal(reviews: #"{"100:\#(Self.hash)":\#(Self.guardUnread)}"#,
                                 states: #"{"100:\#(Self.hash)":{"kind":"ready"}}"#)
        #expect(model.signable && !model.signAnyway)
        #expect(!model.door.unchecked && !model.door.retry)
    }

    @Test func approveAndExecuteOpenOnceAProposalsReviewIsReady() throws {
        let ready = try proposal(reviews: allowed, states: #"{"100:\#(Self.hash)":{"kind":"ready"}}"#)
        #expect(ready.signable)
        #expect(ready.review != nil)
        // The hash's spelling in the key is the core's; either is found.
        let upper = Self.hash.uppercased().replacingOccurrences(of: "0X", with: "0x")
        let spelled = try proposal(
            reviews: #"{"100:\#(upper)":{"sign":{"allowed":true,"anyway":false},"concerns":[]}}"#,
            states: #"{"100:\#(upper)":{"kind":"ready"}}"#
        )
        #expect(spelled.signable)
    }

    // MARK: - The wire

    @Test func theViewsReadEveryReviewState() throws {
        let d = try draft(state: #"{"kind":"failed","reason_key":"home.multisig.review.state.gone"}"#)
        #expect(d.reviewState?.kind == "failed")
        #expect(d.reviewState?["reason_key"]?.string == "home.multisig.review.state.gone")
        var json = MultisigViewsTests.proposalsJson(item: MultisigViewsTests.item(approved: 1, status: "pending"),
                                                    standing: #"{"kind":"progress","approved":1,"threshold":2}"#)
        json = json.replacingOccurrences(of: #""review_states":{}"#, with: #""review_states":{"100:\#(Self.hash)":{"kind":"ready"}}"#)
        let view = try #require(decode(MsProposalsView.self, json))
        let item = try #require(view.items.first)
        #expect(view.reviewState(item)?.kind == "ready")
    }
}
