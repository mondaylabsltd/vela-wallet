//
//  SigningFeeRetryTests.swift
//  VelaWalletTests
//
//  Spec 079 US2: the signing sheet's fee row carries the send form's refresh
//  control, says why a quote failed when the network is the reason, and asks
//  again by itself on the core's schedule while the sheet can still use it.
//  The Android twin is in `SigningLiveTest.kt`. Hermetic.
//
//  Spec 082 T115 (RF5, W10): when the account's deployment cannot be read the
//  quote cannot even start, and the row used to say "estimating…" for ever
//  with no reason. It now says what a failed quote says, and a refresh tap
//  cancels the loop already waiting before it starts one.
//
//  Spec 082 round 2 (T240, RJ12, RJ13, G47, G48): that failure is the
//  core's `ChainRead` — the chain's node, rate-limited or unreachable, never
//  "can't reach Vela" — the words are the core's (`feeFailureReasonKey`), and
//  the schedule is 3 s, 6 s, then every 8 s.
//

import Foundation
import os
import SwiftUI
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SigningFeeRetryTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])

    private func context() -> SigningLive.Context {
        SigningLive.Context(
            loc: loc, chainName: "Gnosis", chainDot: .green, nativeSymbol: "xDAI",
            walletName: "Me", walletAddress: "0x88cca0eedbf2c4426110bbfc998f048689266894"
        )
    }

    private func clear(_ surface: ClearSurface) -> ClearSigningViewWire {
        ClearSigningViewWire(
            resolving: false, resolved: true, result: nil, message: nil,
            surface: surface, confirm: .confirm, blindTyped: nil, dangerHaptic: false
        )
    }

    private func fee(busy: Bool = false, failed: String? = nil, options: Int = 1) -> FeeViewWire {
        let coin = FeeOptionWire(
            symbol: "xDAI", contract: nil, decimals: 18, balance: "1000000000000000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "1000",
            insufficient: false, selected: true
        )
        let other = FeeOptionWire(
            symbol: "USDC", contract: "0xusdc", decimals: 6, balance: "1000000",
            recipient: "0xrelay", usdBalance: "1", usdPrice: nil, amount: "10",
            insufficient: false, selected: false
        )
        return FeeViewWire(
            busy: busy, failed: failed,
            fee: failed == nil && !busy
                ? FeeEstimateWire(
                    chainId: 100, totalWei: "1000", maxFeePerGas: "1", totalGas: "21000",
                    deployed: true, quoted: true, feeAsset: .native, feeRecipient: "0xrelay"
                )
                : nil,
            stale: false, feeToken: nil, options: options > 1 ? [coin, other] : [coin],
            confirmFeeReady: failed == nil && !busy
        )
    }

    private func warning(_ model: FeeModel) -> String? {
        if case .onchain(_, _, _, let warning, _) = model { return warning }
        return nil
    }

    private func tappable(_ model: FeeModel) -> Bool {
        if case .onchain(_, _, _, _, let tappable) = model { return tappable }
        return false
    }

    /// The fee can always be asked again, and a fee that could not be quoted
    /// says why — but only when the network is why.
    @Test func theFeeRowCarriesTheSendFormsRefreshAndSaysWhyAQuoteFailed() {
        let shown = SigningLive.feeRefresh(clear: clear(.clearSign), fee: fee(), loc: loc)
        #expect(shown?.label == loc.t("send.feeRefresh"))
        #expect(shown?.refreshing == false)
        #expect(SigningLive.feeRefresh(clear: clear(.clearSign), fee: fee(busy: true), loc: loc)?.refreshing == true,
                "dimmed while a measurement is out")
        #expect(SigningLive.feeRefresh(clear: clear(.messageSign), fee: nil, loc: loc) == nil,
                "a message has no network fee to refresh")

        let down = SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: "quote_unavailable"), context: context())
        #expect(warning(down) == loc.t("componentsUi.funding.denialNetworkError"))
        #expect(tappable(down), "a failed quote is tapped to ask again")
        for failure in ["fee_token_unavailable", "estimate_failed", "gas_quote_too_high"] {
            #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: failure), context: context()))
                    == loc.t("componentsUi.funding.denialNetworkError"), "\(failure)")
        }
        for failure in ["missing_public_key", "calculation_failed", "would_fail"] {
            #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: failure), context: context()))
                    == nil, "no network sentence for \(failure): the network did not cause it")
        }
        #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(), context: context())) == nil)

        // G48: a chain read the fee is blocked on names the chain's node —
        // rate-limited, or the chain by name — never Vela.
        let limited = SigningLive.feeModel(
            clear: clear(.clearSign),
            fee: fee(failed: FeeFailureText.chainRead(rateLimited: true).text), context: context()
        )
        #expect(warning(limited) == loc.t("home.balanceDetailStatusRetrying"))
        let unreachable = SigningLive.feeModel(
            clear: clear(.clearSign),
            fee: fee(failed: FeeFailureText.chainRead(rateLimited: false).text), context: context()
        )
        #expect(warning(unreachable) == loc.t("explore.chainDown", vars: ["chain": "Gnosis"]))
        #expect(warning(unreachable)?.contains("Gnosis") == true)
        #expect(warning(unreachable) != loc.t("componentsUi.funding.denialNetworkError"))
    }

    /// The fee view takes `FeeFailure`'s object form (RJ13) — a string
    /// decode failed the whole view — and keeps it as the text the core's
    /// functions take back.
    @Test func theChainReadFailureDecodesAndGoesBackToTheCore() throws {
        let view = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": false, "failed": ["chain_read": ["rate_limited": true]], "fee": NSNull(),
            "stale": false, "fee_token": NSNull(), "options": [], "confirm_fee_ready": false,
        ])
        let failed = try #require(view.failed)
        #expect(failed == FeeFailureText.chainRead(rateLimited: true).text)
        #expect(feeFailureReasonKey(failure: failed) == "home.balanceDetailStatusRetrying")
        #expect(feeRequoteDelayMs(failure: failed, attempt: 1) == 3_000)
        #expect(FeeFailureText(failed).cause == "chain_read(rate_limited)")
        // The string form is untouched.
        let plain = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": false, "failed": "quote_unavailable", "fee": NSNull(),
            "stale": false, "fee_token": NSNull(), "options": [], "confirm_fee_ready": false,
        ])
        #expect(plain.failed == "quote_unavailable")
        #expect(feeFailureReasonKey(failure: "missing_public_key") == nil)
    }

    /// G47: the fee's two log lines, in the words every client writes, and
    /// the report's ring gets the class alone.
    @Test func aFailedQuoteIsOneFeeLineInTheRing() {
        VelaLog.resetRecentFailures()
        VelaLog.feeQuoteFailed(chain: 100, cause: "quote_unavailable", requote: 2, inMs: 6_000)
        VelaLog.feeQuoteBack(chain: 100, after: 2)
        #expect(VelaLog.recentFailures == ["fee: quote_failed"])
        #expect(feeRequoteTimeoutMs() == 6_000, "each automatic re-quote is bounded")
    }

    /// The sheet's model: a chevron only where a tap opens a coin list; a
    /// refresh wherever there is a network fee; none on a refused request.
    @Test func theSheetDrawsTheRefreshAndAChevronOnlyForACoinList() {
        let request = SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x38d7ea4c68000"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        )
        let sign = SignViewWire(
            surface: .sheet, request: nil, isSigning: false, isSubmitting: false,
            pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: true,
            reconcilePending: false, swipeAction: .reject, trackerHandoff: nil,
            notice: nil, globalChainId: 100, blocked: nil
        )
        func model(_ fee: FeeViewWire) -> SigningModel {
            SigningLive.model(
                fallback: SigningFixtures.build(.cs1, loc: loc), request: request, sign: sign,
                clear: clear(.none), guard: .empty, fee: fee, context: context()
            )
        }
        let one = model(fee())
        #expect(one.feeRefresh?.label == loc.t("send.feeRefresh"))
        #expect(!one.feeChevron, "one coin: no list, so no chevron")
        #expect(model(fee(options: 2)).feeChevron)
        #expect(!model(fee(failed: "quote_unavailable")).feeChevron,
                "a failed quote is retried by the refresh, not a list")
    }

    /// The re-quote schedule is the core's — 3 s, 6 s, then every 8 s (RJ12)
    /// — and only while the sheet can still use a fee.
    @Test func aRecoverableFailureIsAskedAgainOnTheCoresSchedule() {
        let down = fee(failed: "quote_unavailable")
        #expect(SigningController.requoteDelay(down, attempt: 1, allowed: true) == 3_000)
        #expect(SigningController.requoteDelay(down, attempt: 2, allowed: true) == 6_000)
        #expect(SigningController.requoteDelay(down, attempt: 3, allowed: true) == 8_000)
        #expect(SigningController.requoteDelay(down, attempt: 4, allowed: true) == 8_000)
        #expect(SigningController.requoteDelay(down, attempt: 9, allowed: true) == 8_000)
        let chain = fee(failed: FeeFailureText.chainRead(rateLimited: false).text)
        #expect(SigningController.requoteDelay(chain, attempt: 2, allowed: true) == 6_000)

        #expect(SigningController.requoteDelay(down, attempt: 1, allowed: false) == nil,
                "approved, answered or closed: nothing more is asked")
        #expect(SigningController.requoteDelay(fee(busy: true, failed: "quote_unavailable"), attempt: 1, allowed: true) == nil,
                "a measurement already out is not doubled")
        #expect(SigningController.requoteDelay(fee(failed: "missing_public_key"), attempt: 1, allowed: true) == nil)
        #expect(SigningController.requoteDelay(fee(), attempt: 1, allowed: true) == nil, "a quote needs no retry")
    }

    /// The quote could not start: the row says the chain's node is why
    /// (RJ13) — never Vela — and a tap asks again.
    @Test func aQuoteThatCouldNotStartSaysWhy() {
        var ctx = context()
        ctx.feeStartFailure = FeeFailureText.chainRead(rateLimited: false).text
        let down = SigningLive.feeModel(clear: clear(.clearSign), fee: nil, context: ctx)
        #expect(warning(down) == loc.t("explore.chainDown", vars: ["chain": "Gnosis"]))
        #expect(tappable(down))
        if case .onchain(_, let value, _, _, _) = down {
            #expect(value == loc.t("componentsUi.gas.estimateFailed"))
            #expect(value != loc.t("componentsUi.gas.estimating"))
        }
        // No failure: still estimating, no reason, nothing to tap.
        let waiting = SigningLive.feeModel(clear: clear(.clearSign), fee: nil, context: context())
        #expect(warning(waiting) == nil)
        #expect(!tappable(waiting))
    }

    /// The deployment read fails; the controller names the core's failure,
    /// and two refresh taps leave exactly one loop asking again.
    @Test func aRefreshTapLeavesOneLoop() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let port = ScriptedRelayPort()   // eth_getCode unscripted: the chain is silent
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let controller = SigningController(
            wallet: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults)),
            ports: SigningController.Ports(knownChains: { [100] })
        )
        controller.open(SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        ))
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.quoteStartFailure == unreachable }
        #expect(controller.fee == nil)

        controller.refreshFee()
        controller.refreshFee()
        let reads = { port.calls.filter { $0 == "eth_getCode" }.count }
        // Two taps in a row share one read (the relay's single flight).
        await Wait.until { reads() >= 2 && controller.startLoopsAlive == 1 }
        // Each loop now waits the core's 3 s before asking again; a moment
        // later there is still only the one.
        try? await Task.sleep(nanoseconds: 200_000_000)
        #expect(controller.startLoopsAlive == 1, "a refresh tap cancels the loop already waiting")
        #expect(controller.quoteStartFailure == unreachable)
        controller.swipeDismissed()
    }

    /// 082 iPhone pass (IX6): a chain that never answers the deployment read
    /// — every node black-holed — is said within the first read's bound, not
    /// after the pool's every pass (估算中… for 4 min 35 s with no reason).
    @Test func aDeploymentReadThatNeverAnswersIsSaidInTime() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let port = SilentCodePort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let controller = SigningController(
            wallet: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults)),
            ports: SigningController.Ports(knownChains: { [100] }),
            firstDeploymentReadMs: 300
        )
        controller.open(SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 100
        ))
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.quoteStartFailure == unreachable }
        #expect(controller.quoteStartFailure == unreachable)
        // Proved by order, never by a stopwatch (a loaded CI runner took
        // 34.6 s for a 0.3 s bound, #398): the failure is said while every
        // deployment read is still held open — none ran its 120 s, so it is
        // the bound speaking, not the pool giving up.
        #expect(port.codeReadsThatRanOut == 0, "said within the bound, not after the pool gave up")
        #expect(SigningController.firstDeploymentReadMs == 15_000)
        controller.swipeDismissed()
    }

    /// G48: a public node's rate limit on the deployment read is a
    /// rate-limited chain read — "retrying automatically" — never "can't
    /// reach Vela" and never a shut slide for ever.
    @Test func aRateLimitedDeploymentReadIsARateLimitedChainRead() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .failed(rateLimited: true)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let controller = SigningController(
            wallet: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults)),
            ports: SigningController.Ports(knownChains: { [1] })
        )
        controller.open(SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: 1
        ))
        let limited = FeeFailureText.chainRead(rateLimited: true).text
        await Wait.until { controller.quoteStartFailure == limited }
        var ctx = context()
        ctx.feeStartFailure = controller.quoteStartFailure
        #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: nil, context: ctx))
                == loc.t("home.balanceDetailStatusRetrying"))
        controller.swipeDismissed()
    }
}


/// A relay port whose chain never answers `eth_getCode` — every node of the
/// chain black-holed (IX6). Everything else is the scripted port's.
@MainActor
final class SilentCodePort: RelayPort {
    let inner = ScriptedRelayPort()
    /// `eth_getCode` reads that ran their whole 120 s — the pool giving up,
    /// not the bound (a bound cancels the read, ending its sleep early).
    private let ranOut = OSAllocatedUnfairLock(initialState: 0)
    var codeReadsThatRanOut: Int { ranOut.withLock { $0 } }
    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        if method == "eth_getCode" {
            if (try? await Task.sleep(nanoseconds: 120_000_000_000)) != nil {
                ranOut.withLock { $0 += 1 }
            }
            return .failed(rateLimited: false)
        }
        return await inner.call(chainId: chainId, method: method, params: params, kind: kind)
    }
    func bundlerBase(chainId: Int) async -> String? { await inner.bundlerBase(chainId: chainId) }
    func bestRpcUrl(chainId: Int) async -> String? { await inner.bestRpcUrl(chainId: chainId) }
    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer {
        await inner.restGet(url: url, xRpcUrl: xRpcUrl)
    }
}
