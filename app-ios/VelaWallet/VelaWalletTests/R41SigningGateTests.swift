//
//  R41SigningGateTests.swift
//  VelaWalletTests
//
//  §41 — the shell's half of the core's P2-3 and P2-5:
//
//  - P2-5: the "… anyway" gate is the signing machines' own. An approve on
//    the sheet hands back what the sheet showed (`review`: the reading, the
//    guard and the verdict, as the core wrote them, and Refuse blind
//    signing) and whether the press was the "… anyway" one
//    (`anyway_confirmed`); every multisig signing event carries
//    `anyway_confirmed` and `refuse_blind`. No review, nothing is signed.
//  - P2-3: the simulation reads NFTs, approvals and what it cannot read —
//    three judgments the sheet draws as balance rows.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

// MARK: - A review the signing machine admits

/// §41 (P2-5): the twin of the core's `support::plain_review` — approve
/// options' `review` the signing machine admits on a plain press: a request
/// read in full (`clear_sign`, nothing resolving), no approval-guard concern
/// (the guard core's own first view), and a simulation two independent nodes
/// agreed on. A test about the posture builds its own.
@MainActor
enum ApproveReviewFixture {
    static func plain() throws -> [String: Any] {
        var clear = try CoreJSON.object(ClearSigningCore().view())
        clear["resolving"] = false
        clear["surface"] = "clear_sign"
        clear["sign_anyway"] = false
        let guardView = try CoreJSON.object(ApprovalGuardCore().view())
        // A clean run nothing moves in, the witness agreeing.
        let clean: [String: Any] = ["result": [["calls": [["status": "0x1", "logs": [Any]()]]]]]
        var envelope = clean
        envelope["witness"] = clean
        let record = simOutcome(
            user: "0x88cca0eedbf2c4426110bbfc998f048689266894", replyJson: CoreJSON.string(envelope), valueWei: nil
        )
        return [
            "clear": clear,
            "guard": guardView,
            "sim": SigningController.simVerdictObject(of: record),
            "refuse_blind": false,
        ]
    }
}

// MARK: - P2-5: the approve carries the review and the "… anyway"

@MainActor
struct R41ApproveReviewTests {

    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let now = 1_757_000_000_000.0

    /// The review is the very views the confirm's gate reads, as the core
    /// wrote them, with the person's pref; and the press's "… anyway".
    @Test func anApproveCarriesTheReviewAndTheAnyway() throws {
        let clearJson = try ClearSigningCore().view()
        let guardJson = try ApprovalGuardCore().view()
        let sim = #"{"kind":"deltas","deltas":[],"recheck":false,"source":"corroborated"}"#
        let review = try #require(SigningController.approveReview(
            clear: clearJson, guard: guardJson, sim: sim, refuseBlind: true
        ))
        #expect((review["clear"] as? NSDictionary)?.isEqual(to: try CoreJSON.object(clearJson)) == true)
        #expect((review["guard"] as? NSDictionary)?.isEqual(to: try CoreJSON.object(guardJson)) == true)
        #expect((review["sim"] as? [String: Any])?["source"] as? String == "corroborated")
        #expect(review["refuse_blind"] as? Bool == true)

        let opts = SigningController.approveOpts(
            fee: nil, clear: .empty, guard: .empty, review: review, anywayConfirmed: true
        )
        #expect((opts["review"] as? NSDictionary)?.isEqual(to: review) == true)
        #expect(opts["anyway_confirmed"] as? Bool == true)

        // No sim yet: the review still goes, its verdict null.
        let unread = try #require(SigningController.approveReview(
            clear: clearJson, guard: guardJson, sim: nil, refuseBlind: false
        ))
        #expect(unread["sim"] is NSNull)
        // A view missing: no review — the core signs nothing.
        #expect(SigningController.approveReview(clear: nil, guard: guardJson, sim: sim, refuseBlind: false) == nil)
        #expect(SigningController.approveReview(clear: clearJson, guard: nil, sim: sim, refuseBlind: false) == nil)

        let bare = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty)
        #expect(bare["review"] is NSNull)
        #expect(bare["anyway_confirmed"] as? Bool == false)
    }

    private func arrived() throws -> SignRequestCore {
        let core = SignRequestCore()
        _ = try core.dispatch(eventJson: CoreJSON.string(["type": "networks_changed", "chain_ids": [100]]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "accounts_changed",
            "accounts": [["address": me, "credential_id": "cred-1"]], "active_index": 0,
        ]))
        _ = try core.dispatch(eventJson: CoreJSON.string([
            "type": "request_arrived", "id": "req-1", "method": "eth_sendTransaction",
            "params_json": #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            "origin": "https://app.example", "transport_id": "tab-1",
            "dedicated_transport": true, "per_request_chain": 100, "dapp": NSNull(),
            "granted_address": me, "requested_address": NSNull(), "request_ts_ms": NSNull(),
            "now_ms": now,
        ]))
        return core
    }

    private func approve(_ core: SignRequestCore, review: [String: Any]?, anyway: Bool = false) throws -> BridgeRun {
        var opts = SigningController.approveOpts(fee: nil, clear: .empty, guard: .empty, review: review, anywayConfirmed: anyway)
        opts["quoted_fee"] = ["amount": "1000", "recipient": me, "tier": "fast"]
        var run = BridgeRun()
        try run.take(core.dispatch(eventJson: CoreJSON.string(["type": "approve_tapped", "opts": opts])))
        return run
    }

    /// Through the real signing core: no review, nothing goes; the review
    /// the sheet showed, and the pipeline starts.
    @Test func theCoreSignsNothingWithoutTheReview() throws {
        let refused = try approve(try arrived(), review: nil)
        #expect(refused.pending.isEmpty, "no review: no operation \(refused.pending)")
        #expect(try CoreJSON.decode(SignViewWire.self, from: refused.view).phase != .preparing)

        let taken = try approve(try arrived(), review: try ApproveReviewFixture.plain())
        #expect(try CoreJSON.decode(SignViewWire.self, from: taken.view).phase == .preparing)
        #expect(!taken.pending.isEmpty)
    }

    /// An "… anyway" posture (a simulation nobody checked) is taken only on
    /// the "… anyway" press, and never with Refuse blind signing on.
    @Test func anAnywayPostureNeedsTheAnywayPressAndNoRefuseBlind() throws {
        var unchecked = try ApproveReviewFixture.plain()
        unchecked["sim"] = NSNull()
        #expect(try approve(try arrived(), review: unchecked).pending.isEmpty, "a plain press of an anyway posture")
        unchecked["refuse_blind"] = true
        #expect(try approve(try arrived(), review: unchecked, anyway: true).pending.isEmpty, "refuse blind")
        unchecked["refuse_blind"] = false
        let taken = try approve(try arrived(), review: unchecked, anyway: true)
        #expect(try CoreJSON.decode(SignViewWire.self, from: taken.view).phase == .preparing)
    }

    /// Every multisig signing press carries the door's "… anyway" and the pref.
    @Test func aMultisigPressCarriesItsAnywayAndRefuseBlind() {
        let anyway = MsSignDoor(shows: .review, enabled: true, anyway: true, reasonKey: nil, retry: false)
        let fields = MsRules.pressFields(anyway, refuseBlind: true)
        #expect(fields["anyway_confirmed"] as? Bool == true)
        #expect(fields["refuse_blind"] as? Bool == true)
        let plain = MsRules.pressFields(.unanswered, refuseBlind: false)
        #expect(plain["anyway_confirmed"] as? Bool == false)
        #expect(plain["refuse_blind"] as? Bool == false)
    }
}

// MARK: - P2-3: NFTs, approvals, and what Vela can't read

@MainActor
struct R41BalanceRowsTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let me = "0x88cca0eedbf2c4426110bbfc998f048689266894"
    private let nft = "0x5af0d9827e0c53e4799bb226655a1de152a425a5"
    private let spender = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"

    private var judgmentsJson: String {
        #"""
        [{"type":"nft","token":"\#(nft)","token_id":"7","standard":"erc721","direction":"in","amount":"1"},
         {"type":"nft","token":"\#(nft)","token_id":"9","standard":"erc1155","direction":"out","amount":"3"},
         {"type":"nft","token":"\#(nft)","token_id":"11","standard":"erc1155","direction":"still","amount":"0"},
         {"type":"approval","token":"\#(nft)","spender":"\#(spender)","unlimited":true},
         {"type":"approval","token":"\#(nft)","spender":"\#(spender)","unlimited":false,"token_id":"7"},
         {"type":"unread","token":null}]
        """#
    }

    /// The three judgments decode, and go back to the core exactly as it
    /// wrote them (the record's `balance_changes`).
    @Test func theNewJudgmentsDecodeAndGoBackVerbatim() throws {
        let raw = try #require(JSONSerialization.jsonObject(with: Data(judgmentsJson.utf8)) as? [[String: Any]])
        let judgments = try CoreJSON.decoder.decode([TrustSimJudgmentWire].self, from: Data(judgmentsJson.utf8))
        #expect(judgments[0] == .nft(token: nft, tokenId: "7", standard: "erc721", direction: .in, amount: "1"))
        #expect(judgments[3] == .approval(token: nft, spender: spender, unlimited: true, tokenId: nil))
        #expect(judgments[4] == .approval(token: nft, spender: spender, unlimited: false, tokenId: "7"))
        #expect(judgments[5] == .unread(token: nil))
        for (judgment, written) in zip(judgments, raw) {
            #expect((judgment.wire as NSDictionary).isEqual(to: written), "\(judgment.wire) vs \(written)")
        }
    }

    /// The real trust core judges the simulation's new delta kinds into them.
    @Test func theRealCoreJudgesNftApprovalAndUnreadDeltas() throws {
        let sim = try #require(TrustCoreScene.sim([
            ["kind": "erc721", "token": nft, "delta": "1", "token_id": "7"],
            ["kind": "approval", "token": nft, "delta": "1", "spender": spender, "token_id": "7"],
            ["kind": "unread", "token": nft, "delta": "9"],
        ], chainId: 100))
        #expect(sim.ready)
        #expect(sim.judgments.contains { if case .nft(_, "7", _, .in, _) = $0 { return true }; return false }, "\(sim.judgments)")
        #expect(sim.judgments.contains { if case .approval(_, let who, _, "7") = $0 { return who == spender }; return false }, "\(sim.judgments)")
        #expect(sim.judgments.contains { if case .unread = $0 { return true }; return false }, "\(sim.judgments)")
    }

    /// Each is drawn as a balance row: an NFT by its id with its sign (and
    /// how many, past one), no row for a move of nothing, an approval in
    /// danger naming who may spend, and the change Vela can't read — with the
    /// unverified caution under the card.
    @Test func theSheetDrawsTheirRows() throws {
        let sim = try CoreJSON.decoder.decode(
            TrustSimViewWire.self, from: Data(#"{"ready":true,"judgments":\#(judgmentsJson)}"#.utf8)
        )
        let context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: me, origin: "https://app.example",
            sim: sim, simulation: .answered
        )
        let blocks = SigningLive.balanceBlocks(isTransaction: true, context: context)
        guard blocks.count == 1, case .balances(_, let rows, let note, let tone)? = blocks.first else {
            Issue.record("a balance card: \(blocks.map(\.id))")
            return
        }
        let short = AddressText.short(spender)
        #expect(rows.map(\.symbol) == [
            loc.t("componentsUi.signing.balanceNft", vars: ["id": "7"]),
            loc.t("componentsUi.signing.balanceNft", vars: ["id": "9"]),
            loc.t("componentsUi.signing.balanceApprovalAll", vars: ["spender": short]),
            loc.t("componentsUi.signing.balanceApproval", vars: ["spender": short]),
            loc.t("componentsUi.signing.balanceUnread"),
        ])
        #expect(rows[0].symbol == "NFT #7")
        #expect(rows.map(\.delta) == ["+", "\u{2212}3", "", "", ""])
        #expect(rows.map(\.tone) == [.caution, .caution, .danger, .danger, .caution])
        #expect(rows[4].symbol == "This changes something Vela can’t read")
        #expect(note == loc.t("componentsUi.signing.unverifiedWarning"))
        #expect(tone == .caution)
    }
}
