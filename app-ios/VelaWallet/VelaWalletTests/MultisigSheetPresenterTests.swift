//
//  MultisigSheetPresenterTests.swift
//  VelaWalletTests
//
//  The root's one multisig sheet, handed to SwiftUI by its lifecycle
//  (`MsSheetPresenter`, through `MultisigHub`): a sheet asked for while
//  another is going is handed over only once that one has gone; the
//  binding's `nil` echo of a dismissal the hub asked for is never the
//  person's swipe; and a sheet the hub holds but that is not on screen opens
//  again when a person asks for it.
//
//  The UIKit side — what CI's slow simulator did to a sheet swapped in
//  mid-dismissal — is `MultisigHermeticUITests.
//  testAProposedSendsSheetOpensAndOpensAgainFromTheHome`; here are the
//  states, with the lifecycle's calls made by hand in the order SwiftUI
//  makes them (`MultisigHost`).
//

#if DEBUG
import Foundation
import Testing
@testable import VelaWallet

@MainActor
@Suite(.hangLimit)
struct MultisigSheetPresenterTests {

    private static let hash = "0x" + String(repeating: "ab", count: 32)
    private static let proposal = MultisigSheet.proposal(chainId: 100, safeTxHash: hash)

    /// A hub over the simulated world, never booted: the sheet's lifecycle
    /// asks nothing of the machines.
    private static func hub() throws -> MultisigHub {
        let world = try MsSimWorld()
        return MultisigHub(
            ports: world, loc: Loc(overrideTag: "en", preferredLanguages: []),
            shelf: VelaStore(defaults: UserDefaults(suiteName: "vela.tests.mssheet.\(UUID().uuidString)")!),
            tokens: nil,
            facts: { .init(serviceUrl: MsSimWorld.service, accounts: [], networks: [100], space: [], known: []) }
        )
    }

    private func source(_ path: String) throws -> String {
        try String(contentsOf: URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent(path), encoding: .utf8)
    }

    private static func dispose(_ hub: MultisigHub) {
        hub.proposals.dispose(); hub.accounts.dispose(); hub.members.dispose()
    }

    /// The sheet the root shows, appeared, as SwiftUI reports it.
    private static func appear(_ hub: MultisigHub) throws -> MsSheetPresenter.Shown {
        let shown = try #require(hub.presenter.shown, "nothing handed over")
        hub.sheetAppeared(shown)
        return shown
    }

    // MARK: - The propose path

    /// A send's propose: its review, on screen, is moved aside on "working"
    /// and the transaction's sheet asked for while the review is still going.
    /// Nothing is handed over until the review has gone; the binding's `nil`
    /// echo of that dismissal closes nothing; then the transaction's sheet.
    @Test func theTransactionsSheetWaitsForTheReviewToHaveGone() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .review(chainId: 100)
        let review = try Self.appear(hub)
        #expect(review.sheet == .review(chainId: 100))

        // "working": the review moved aside, then the mailbox answered.
        hub.sheet = nil
        hub.open(Self.proposal)
        #expect(hub.sheet == Self.proposal, "what the hub asks for")
        #expect(hub.presenter.shown == nil, "nothing handed over into a dismissal still running")
        #expect(!hub.presenter.settled)

        // SwiftUI's echo as the review's dismissal finishes: not a swipe.
        hub.sheetBindingCleared()
        #expect(hub.sheet == Self.proposal, "the echo closed the sheet asked for next")

        // Gone (its onDismiss): the transaction's sheet now.
        hub.sheetGone()
        #expect(hub.presenter.shown?.sheet == Self.proposal)
        // The review's content leaving after its onDismiss hands nothing twice.
        let handed = hub.presenter.shown
        hub.sheetDisappeared(review)
        #expect(hub.presenter.shown == handed)
        let shown = try Self.appear(hub)
        #expect(hub.presenter.onScreen && shown.sheet == Self.proposal)
    }

    /// Should the dismissed sheet's onDismiss never be heard, its content
    /// leaving the screen (which comes after UIKit finished too) hands the
    /// next over: nothing waits for ever.
    @Test func aSheetsContentLeavingHandsTheNextOverWithoutItsOnDismiss() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .review(chainId: 100)
        let review = try Self.appear(hub)
        hub.sheet = nil
        hub.open(Self.proposal)
        #expect(hub.presenter.shown == nil)
        hub.sheetDisappeared(review)
        #expect(hub.presenter.shown?.sheet == Self.proposal)
        hub.sheetGone()
        #expect(hub.presenter.shown?.sheet == Self.proposal, "a late onDismiss hands nothing again")
    }

    /// From one sheet straight to another (Settings → Members): the first
    /// goes, the second follows once it has.
    @Test func oneSheetToAnotherGoesThroughTheFirstsDismissal() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .settings(address: "0xabc")
        _ = try Self.appear(hub)
        hub.sheet = .members(address: "0xabc")
        #expect(hub.presenter.shown == nil)
        hub.sheetGone()
        #expect(hub.presenter.shown?.sheet == .members(address: "0xabc"))
    }

    /// Asked for and then let go while the one before was still going:
    /// nothing is handed over when it has gone.
    @Test func aSheetLetGoWhileTheOneBeforeWasGoingIsNeverShown() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .create
        _ = try Self.appear(hub)
        hub.sheet = .link
        hub.sheet = nil
        hub.sheetGone()
        #expect(hub.presenter.shown == nil)
        #expect(hub.presenter.settled)
    }

    /// Handed over and not on screen yet: the next needs no dismissal to
    /// wait for, and is handed over at once.
    @Test func aSheetNotYetOnScreenIsReplacedAtOnce() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .link
        #expect(hub.presenter.shown?.sheet == .link)
        hub.open(Self.proposal)
        #expect(hub.presenter.shown?.sheet == Self.proposal)
    }

    // MARK: - The person's swipe

    /// A person's swipe of the sheet on screen is still its own way out
    /// (`sheetDismissed`): closed, and nothing comes after it.
    @Test func aSwipeOfTheSheetOnScreenClosesIt() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.open(Self.proposal)
        _ = try Self.appear(hub)
        hub.sheetBindingCleared()
        #expect(hub.sheet == nil, "the person closed it")
        hub.sheetGone()
        #expect(hub.presenter.shown == nil && hub.presenter.settled)
    }

    /// A review not yet on screen cannot have been swiped: the binding's
    /// `nil` then withdraws nothing.
    @Test func aReviewNotOnScreenIsNeverWithdrawnByTheBindingsNil() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .review(chainId: 100)
        hub.sheetBindingCleared()
        #expect(hub.sheet == .review(chainId: 100))
    }

    // MARK: - Never claimed and not on screen

    /// The hub holds the transaction's sheet but its showing was lost (it
    /// never appeared): the home's row — the same sheet asked for again —
    /// hands it over again, as a new identity SwiftUI must present.
    @Test func aSheetHeldButNeverOnScreenOpensAgainFromAPersonsDoor() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.open(Self.proposal)
        let lost = try #require(hub.presenter.shown)
        // Nothing appeared. The home card's tap:
        hub.open(Self.proposal)
        let again = try #require(hub.presenter.shown)
        #expect(again.sheet == Self.proposal)
        #expect(again.id != lost.id, "the same identity again: SwiftUI sees no change and presents nothing")
        // A lifecycle call for the lost showing changes nothing.
        hub.sheetAppeared(lost)
        #expect(!hub.presenter.onScreen)
        hub.sheetAppeared(again)
        #expect(hub.presenter.onScreen)
        // On screen, asking again leaves it as it is.
        hub.open(Self.proposal)
        #expect(hub.presenter.shown == again)
    }

    /// Taken off the screen under the hub (its content gone, no dismissal
    /// asked for): asked for again, it opens again.
    @Test func aSheetTakenOffTheScreenUnderTheHubOpensAgain() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.open(Self.proposal)
        let first = try Self.appear(hub)
        hub.sheetDisappeared(first)
        #expect(!hub.presenter.onScreen)
        #expect(hub.sheet == Self.proposal, "still asked for")
        hub.open(Self.proposal)
        #expect(hub.presenter.shown?.id != first.id)
        #expect(hub.presenter.shown?.sheet == Self.proposal)
    }

    /// While a sheet is going, asking for it again waits with everything
    /// else: its dismissal hands over what is asked for then.
    @Test func askingAgainWhileASheetIsGoingWaitsForItsDismissal() throws {
        let hub = try Self.hub()
        defer { Self.dispose(hub) }

        hub.sheet = .review(chainId: 100)
        _ = try Self.appear(hub)
        hub.sheet = nil
        hub.open(Self.proposal)
        hub.open(Self.proposal)
        #expect(hub.presenter.shown == nil)
        hub.sheetGone()
        #expect(hub.presenter.shown?.sheet == Self.proposal)
    }

    // MARK: - The wiring

    /// Every door to a transaction's sheet goes through `open`, and the
    /// root's sheet reads the presenter, its lifecycle told back.
    @Test func theDoorsAndTheRootGoThroughThePresenter() throws {
        let graph = try source("VelaWallet/App/AppGraph.swift")
        #expect(graph.contains("multisig?.open(.proposal(chainId: chainId, safeTxHash: hash))"))
        let root = try source("VelaWallet/App/RootView.swift")
        #expect(root.contains("onOpen: { multisig.open($0) }"))
        #expect(root.contains("multisig.open(.proposal(chainId: tx.chainId, safeTxHash: tx.safeTxHash))"))
        #expect(!root.contains("multisig.sheet = .proposal"))
        let notifier = try source("VelaWallet/Features/Multisig/Shell/MultisigNotifier.swift")
        #expect(notifier.contains("hub.open(.proposal(chainId: chainId, safeTxHash: hash))"))
        let host = try source("VelaWallet/Features/Multisig/Views/MultisigHost.swift")
        #expect(host.contains("get: { hub.presenter.shown }"))
        #expect(host.contains("hub.sheetGone()"))
        #expect(host.contains("hub.sheetAppeared(shown)"))
        #expect(host.contains("hub.sheetDisappeared(shown)"))
        #expect(host.contains("hub.sheetBindingCleared()"))
    }
}
#endif
