//
//  TrustedSignerUnreachableTests.swift
//  VelaWalletTests
//
//  Spec 079 US7 (T057, the second half): back from a signing page that never
//  answered, the page's address is asked (a HEAD, no fragment, no query);
//  when it does not answer, the waiting card says the page could not open and
//  offers a retry, and the request stays open. The Android twins are
//  `TrustedSignerChannelTest` "back from a page whose address does not
//  answer…" and `SigningReceiptTest` "a page that never opened…". Hermetic:
//  the probe is a scripted verdict, the page is never opened.
//

import Foundation
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct TrustedSignerUnreachableTests {

    private let loc = Loc(overrideTag: "zh", preferredLanguages: [])

    /// A channel with a visit under way, a scripted probe, and a record of
    /// what was probed and what the card was told.
    private func channel(reachable: Bool) throws -> (TrustedSignerChannel, () -> [URL], () -> [Bool]) {
        let fixture = TrustedSignerFixture()
        let request = try trustedSignerRequest(
            input: UserOpSpine.trustedSignerInput(
                asked: .init(method: "personal_sign", paramsJson: #"["0x68656c6c6f","\#(fixture.account)"]"#,
                             origin: "https://app.example"),
                chainId: 100, account: fixture.account, accountName: "Mine", keys: fixture.keys, calls: []
            ),
            draft: nil
        )
        let channel = TrustedSignerChannel(
            signerUrl: fixture.signerUrl, requestJson: request,
            digest: Data(repeating: 0xAB, count: 32), keys: fixture.keys
        )
        var probed: [URL] = []
        var told: [Bool] = []
        channel.reachable = { url in
            probed.append(url)
            return reachable
        }
        channel.onUnreachableChanged = { told.append($0) }
        #expect(channel.start() != nil)
        return (channel, { probed }, { told })
    }

    /// Back from a page whose address does not answer: the card says it, the
    /// request stays open, and a retry clears it — the same visit, the same
    /// URL. A page whose address answers is left as it was.
    @Test func backFromAPageWhoseAddressDoesNotAnswerTheCardSaysItAndARetryClearsIt() async throws {
        let (down, probed, told) = try channel(reachable: false)
        let url = try #require(down.launchUrl)
        await down.personReturned()
        #expect(down.unreachable)
        #expect(told() == [true])
        #expect(down.launchUrl == url, "the same visit, not a new one")

        // The address alone: nothing of the request reaches a server.
        let asked = try #require(probed().first)
        #expect(asked.fragment == nil)
        #expect(asked.query == nil)
        #expect(url.absoluteString.hasPrefix(asked.absoluteString))

        // Asked once: a second return does not ask again.
        await down.personReturned()
        #expect(probed().count == 1)

        down.retried()
        #expect(!down.unreachable, "a retry is waiting again")
        #expect(told() == [true, false])
        #expect(down.launchUrl == url, "the retry opens the same page")

        // The request was never answered: it is still open.
        down.cancel()
        let ending = await down.ending()
        #expect(ending == .outcome(.refused(refusal: .declined)))

        let (up, _, upTold) = try channel(reachable: true)
        await up.personReturned()
        #expect(!up.unreachable)
        #expect(upTold().isEmpty)
        up.cancel()
    }

    /// The tab's own signals (iOS's return signal): a page that reported
    /// loading is not "a page that could not open" — it may have come from
    /// the cache — so closing it asks nothing; a first load that failed is
    /// asked about at once.
    @Test func theTabsOwnSignalsDecideWhetherToAsk() async throws {
        let (loaded, probed, _) = try channel(reachable: false)
        await loaded.pageLoaded(true)
        await loaded.personReturned()
        #expect(probed().isEmpty, "a page that opened is not asked about")
        #expect(!loaded.unreachable)
        loaded.cancel()

        let (failed, failedProbed, _) = try channel(reachable: false)
        await failed.pageLoaded(false)
        #expect(failedProbed().count == 1)
        #expect(failed.unreachable)
        failed.cancel()

        // A request that ended is asked about no more.
        let (ended, endedProbed, _) = try channel(reachable: false)
        ended.cancel()
        await ended.personReturned()
        #expect(endedProbed().isEmpty)
        #expect(!ended.unreachable)
    }

    @Test func theProbeIsTheAddressAlone() throws {
        let launch = try #require(URL(string: "https://sign.getvela.app/b/e3ef90a6/sign?ch=url#i=secret&t=token"))
        #expect(TrustedSignerChannel.probeUrl(launch)?.absoluteString == "https://sign.getvela.app/b/e3ef90a6/sign")
        #expect(TrustedSignerChannel.probeUrl(try #require(URL(string: "velawallet://sign-result?x=1"))) == nil,
                "never anything but a web address")
        #expect(TrustedSignerChannel.returnGrace == 1.2)
    }

    /// The card: "签名页没能打开，请检查网络。", no hint, Retry as its primary
    /// action, Cancel as ever — and the waiting card otherwise.
    @Test func aPageThatNeverOpenedIsTheWaitingCardsToSayWithARetry() {
        let down = TrustedSignerSheetModel.copy(unreachable: true, loc: loc)
        #expect(down.title == loc.t("componentsUi.signing.signerDown"))
        #expect(down.hint == nil)
        #expect(down.reopen == loc.t("connect.browser.retry"))
        #expect(down.reopenPrimary)
        #expect(down.cancel == loc.t("common.cancel"))
        #expect(!down.busy)

        let waiting = TrustedSignerSheetModel.copy(unreachable: false, loc: loc)
        #expect(waiting.title == loc.t("componentsUi.signing.trustedSignerWaiting"))
        #expect(waiting.hint == loc.t("componentsUi.signing.trustedSignerWaitingHint"))
        #expect(waiting.reopen == loc.t("componentsUi.signing.trustedSignerReopen"))
        #expect(!waiting.reopenPrimary)
        #expect(waiting.busy)
    }

    /// Under the waiting card the signing sheet says nothing of its own about
    /// the signature: no "签名中…" and no receipt — the card speaks for it.
    @Test func noSigningSentenceSitsUnderTheWaitingCard() {
        let request = SigningController.Incoming(
            id: "r1", method: "personal_sign",
            paramsJson: #"["0x48","0x88cCA0EeDbF2C4426110bbFc998F048689266894"]"#,
            origin: "http://127.0.0.1:8137", transportId: "tab-1", chainId: 100
        )
        // Spec 082: the prompt is up — the core's `awaiting_signature` — for
        // a message.
        let signing = SignViewWire(
            surface: .sheet,
            request: SignRequestViewWire(
                id: "r1", method: "personal_sign", kind: .personalSign, paramsJson: "[]",
                origin: "http://127.0.0.1:8137", dapp: nil, chainId: 100, signerAddress: nil
            ),
            isSigning: true, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil, phase: .awaitingSignature
        )
        var context = SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "XDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
        func sentence(_ blocks: [SigningBlock]) -> Bool {
            blocks.contains { block in
                if case .sentence(let text, _) = block { return text == loc.t("componentsUi.signing.signing") }
                return false
            }
        }
        func model() -> SigningModel {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: signing,
                clear: .empty, guard: .empty, fee: nil, context: context
            )
        }
        #expect(sentence(model().blocks), "a passkey signature says it is signing")
        #expect(model().receipt != nil)

        context.trustedSignerRoute = true
        let card = model()
        #expect(!sentence(card.blocks), "no \"签名中…\" above a page that did not open")
        #expect(card.receipt == nil, "no receipt under the card either")

        #expect(!sentence(SigningLive.statusBlocks(sign: signing, loc: loc, signerPageOpen: true)))
        #expect(sentence(SigningLive.statusBlocks(sign: signing, loc: loc)))
    }
}
