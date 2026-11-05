//
//  R13FollowupsTests.swift
//  VelaWalletTests
//
//  Follow-ups after Round 13 (docs/multisig-machines.md §18):
//
//  - item 1: a proposal link whose read failed — not in time (`failed`), its
//    network silent (`unreadable`), the multisig service silent (`notFound`)
//    — is said in the core's words for that read and offers Try again
//    whenever the core's `retry` says so; `notInMailbox` (an answer) offers
//    none, and nothing says "it hasn't been shared" for a read that failed.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct R13FollowupsTests {

    private let loc = Loc(overrideTag: "en")
    private var words: MsWords { MsWords(loc: loc, hub: nil, localNames: [:]) }

    private func linked(_ json: String) throws -> MsLinkedView {
        try JSONDecoder().decode(MsLinkedView.self, from: Data(json.utf8))
    }

    // MARK: - item 1: every link the core says is retryable offers Try again

    @Test func everyLinkTheCoreSaysRetryableOffersTryAgain() throws {
        for (stand, leaf) in [("failed", "linkChain"), ("unreadable", "linkChain"), ("notFound", "linkService")] {
            let view = try linked(
                #"{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"\#(stand)","#
                    + #""reason_key":"home.multisig.proposal.unread.\#(leaf)","retry":true}"#
            )
            #expect(view.offersTryAgain, "\(stand)")
            let said = view.failure(words)
            #expect(said == words.t("proposal.unread.\(leaf)", ["network": words.network(100)]), "\(stand)")
            #expect(said != words.t("proposal.notFound"), "\(stand)")
        }
        let answered = try linked(
            #"{"safe":"0x1","chain_id":100,"safe_tx_hash":"0x2","preview":false,"stand":"notInMailbox","reason_key":null,"retry":false}"#
        )
        #expect(!answered.offersTryAgain)
        #expect(answered.failure(words) == nil)
    }
}
