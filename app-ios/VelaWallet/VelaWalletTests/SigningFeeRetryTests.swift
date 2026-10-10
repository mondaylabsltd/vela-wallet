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
//  Issue #483: the account read is the fee MACHINE's (`read_deployment`), so
//  its failure is in the fee view — the row, the footer (the core's gate) and
//  the retry name the same cause — and a read that never left the app is
//  `internal`, never "Can't reach <chain>". The shell's own "quote could not
//  start" state is gone: its footer said "Working out the network fee…" under
//  a row that said "Tap to retry".
//
//  PR 2 note 1 (the integration's core round): the CORE asks a failed fee
//  again by itself, on `start_ttl` (3 s, 6 s, then every 8 s) — the sheet's
//  own re-quote loop is gone — and `FeeView.failure` says the failure ONCE
//  for the row and the footer: "Retrying…" while the core retries (never
//  "Tap to retry" then), the reason kept through the re-ask, and a tap only
//  where a tap is the one way. A sheet that is over stops asking.
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

    /// The failure as the core says it (`FeeFailureView::of`): its reason,
    /// whether it asks again by itself, the row's figure and the footer.
    private func failure(_ failed: String, reasonKey: String?, autoRetry: Bool = true,
                         retrying: Bool = false) -> FeeFailureViewWire {
        FeeFailureViewWire(
            failure: failed, reasonKey: reasonKey, autoRetry: autoRetry, retrying: retrying,
            figureKey: autoRetry || retrying ? nil : "componentsUi.gas.estimateFailed",
            footerKey: autoRetry || retrying
                ? "componentsUi.signing.confirmBlock.feeRetrying"
                : "componentsUi.signing.confirmBlock.feeFailed"
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
            confirmFeeReady: failed == nil && !busy,
            // The core's own state for it, written by its own functions.
            failure: failed.map {
                failure($0, reasonKey: feeFailureReasonKey(failure: $0),
                        autoRetry: feeRequoteDelayMs(failure: $0, attempt: 1) != nil)
            }
        )
    }

    private func value(_ model: FeeModel) -> String? {
        if case .onchain(_, let value, _, _, _) = model { return value }
        return nil
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
        #expect(warning(down) == loc.t("componentsUi.gas.reasonQuote"))
        #expect(tappable(down), "a failed quote is tapped to ask again")
        // PR 2 note 1: the core retries it by itself — the dash, never "Tap
        // to retry" over a row that is asking again anyway.
        #expect(value(down) == "—")
        #expect(value(down) != loc.t("componentsUi.gas.estimateFailed"))
        // Only a failure no retry fixes asks for the tap.
        #expect(value(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: "missing_public_key"),
                                           context: context())) == loc.t("componentsUi.gas.estimateFailed"))
        // Each relay failure says which it was — never "check your
        // connection" over a simulation the relay did not answer.
        for (failure, key) in [
            ("fee_token_unavailable", "componentsUi.gas.reasonFeeToken"),
            ("estimate_failed", "componentsUi.gas.reasonSimulation"),
            ("gas_quote_too_high", "componentsUi.gas.reasonQuoteHigh"),
        ] {
            #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: failure), context: context()))
                    == loc.t(key), "\(failure)")
            #expect(loc.t(key) != key, "\(key) resolves")
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
        #expect(warning(unreachable) == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(warning(unreachable)?.contains("Gnosis") == true)
        #expect(warning(unreachable) != loc.t("componentsUi.gas.reasonQuote"))
        // Issue #483: a read that never left the app is Vela's fault, said
        // as that — never the chain's name.
        let internalFault = SigningLive.feeModel(clear: clear(.clearSign), fee: fee(failed: "internal"), context: context())
        #expect(warning(internalFault) == loc.t("componentsUi.gas.reasonInternal"))
        #expect(warning(internalFault)?.contains("Gnosis") == false)
        #expect(tappable(internalFault))
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
        // …and `failure` (PR 2 note 1), its `failure` the same object form.
        let said = try CoreJSON.decode(FeeViewWire.self, from: [
            "busy": true, "failed": NSNull(), "fee": NSNull(),
            "stale": false, "fee_token": NSNull(), "options": [], "confirm_fee_ready": false,
            "failure": [
                "failure": ["chain_read": ["rate_limited": false]],
                "reason_key": "componentsUi.gas.reasonChainDown", "auto_retry": true,
                "retrying": true, "figure_key": NSNull(),
                "footer_key": "componentsUi.signing.confirmBlock.feeRetrying",
            ] as [String: Any],
        ])
        let kept = try #require(said.failure)
        #expect(kept.failure == FeeFailureText.chainRead(rateLimited: false).text)
        #expect(kept.retrying && kept.autoRetry && kept.figureKey == nil)
        #expect(kept.figure(loc) == "—")
        #expect(kept.reason(loc, chain: "Gnosis") == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
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

    /// PR 2 note 1: a failure that can pass is asked again by the CORE, on
    /// its own timer (`start_ttl`) — every re-ask a real new read of the
    /// account — and the footer says "Retrying…" all the way through, the
    /// row keeping its reason; nothing asks again without the timer, and a
    /// sheet that is over asks nothing more.
    @Test func theCoreAsksAFailedFeeAgainByItselfAndTheSheetStopsWithIt() async {
        let port = ScriptedRelayPort()   // eth_getCode unscripted: the chain is silent
        let controller = controller(port, timers: .stopped)
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.fee?.failed == unreachable }
        let reads = { port.calls.filter { $0 == "eth_getCode" }.count }
        let first = reads()
        #expect(controller.fee?.failure?.autoRetry == true)
        #expect(controller.confirmState.key == "componentsUi.signing.confirmBlock.feeRetrying",
                "\(String(describing: controller.confirmState.key))")
        let row = SigningLive.feeModel(clear: clear(.clearSign), fee: controller.fee, context: context())
        #expect(value(row) == "—", "never \"Tap to retry\" while the core retries")
        #expect(warning(row) == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))

        // Nothing re-asks until the core's own timer runs out…
        await Wait.until { controller.feeIdle }
        #expect(reads() == first, "the shell asked again by itself")
        // …and then the account is read again, and the failure said again.
        controller.elapseFeeTimer("start_ttl")
        await Wait.until { reads() > first }
        #expect(reads() > first, "the core's re-ask did not read the chain again")
        await Wait.until { controller.fee?.failed == unreachable }
        #expect(controller.confirmState.key == "componentsUi.signing.confirmBlock.feeRetrying")

        // The sheet is over: its fee asks nothing more, whatever the clock does.
        await Wait.until { controller.feeIdle }
        controller.swipeDismissed()
        await Wait.until { controller.hasAnswered }
        let settled = reads()
        controller.elapseFeeTimer("start_ttl")
        controller.elapseFeeTimer("start_deadline")
        for _ in 0..<50 { await Task.yield() }
        #expect(controller.feeIdle)
        #expect(reads() == settled, "a sheet that is over kept asking the chain")
    }

    /// The re-ask out is no new state on screen (PR 2 note 1): the row keeps
    /// the failure's reason beside the measuring sign, its figure the dash —
    /// never "Estimating…" — and the footer still says it is retrying.
    @Test func theReAskKeepsTheReasonOnTheRowAndTheFooter() throws {
        let views = try #require(FeeCoreScene.chainDown.views(chainId: 100))
        let retryingJson = try #require(views.retrying)
        let retrying = try CoreJSON.decoder.decode(FeeViewWire.self, from: Data(retryingJson.utf8))
        #expect(retrying.busy && retrying.failed == nil)
        let failure = try #require(retrying.failure, "the core said nothing of the failure through its re-ask")
        #expect(failure.retrying)
        let row = SigningLive.feeModel(clear: clear(.clearSign), fee: retrying, context: context())
        #expect(value(row) == "—")
        #expect(value(row) != loc.t("componentsUi.gas.estimating"))
        #expect(warning(row) == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(SigningLive.feeRefresh(clear: clear(.clearSign), fee: retrying, loc: loc)?.refreshing == true,
                "the measuring sign turns while the core re-asks")
        // The footer is the core's gate over the views, the fee's among them.
        let gate = Self.gate(fee: retryingJson)
        #expect(!gate.enabled)
        #expect(gate.key == "componentsUi.signing.confirmBlock.feeRetrying", "\(String(describing: gate.key))")

        // Only a tap retries an account with no key: "Tap to retry", "Tap it
        // to retry" — the core's own pair.
        let tapOnly = try #require(FeeCoreScene.missingKey.views(chainId: 100))
        let tap = try CoreJSON.decoder.decode(FeeViewWire.self, from: Data(tapOnly.failed.utf8))
        #expect(tap.failure?.autoRetry == false)
        #expect(tap.failure?.footerKey == "componentsUi.signing.confirmBlock.feeFailed")
        #expect(value(SigningLive.feeModel(clear: clear(.clearSign), fee: tap, context: context()))
                == loc.t("componentsUi.gas.estimateFailed"))
        #expect(Self.gate(fee: tapOnly.failed).key == "componentsUi.signing.confirmBlock.feeFailed")
    }

    /// `signConfirmState` over a sheet that is ready but for its fee.
    static func gate(fee: String) -> SignConfirmStateWire {
        var sign = (try? CoreJSON.object(SignRequestCore().view())) ?? [:]
        sign["surface"] = "sheet"
        sign["confirm_gate_open"] = true
        var clear = (try? CoreJSON.object(ClearSigningCore().view())) ?? [:]
        clear["resolving"] = false
        clear["resolved"] = true
        clear["surface"] = "clear_sign"
        var guardView = (try? CoreJSON.object(ApprovalGuardCore().view())) ?? [:]
        guardView["confirm_allowed"] = true
        return SignConfirmStateWire.of(
            sign: CoreJSON.string(sign), guard: CoreJSON.string(guardView), clear: CoreJSON.string(clear),
            fee: fee, speedTier: "standard"
        )
    }

    private func controller(
        _ port: RelayPort, chainId: Int = 100, timers: FeeStore.Timers = .wallClock
    ) -> SigningController {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        let controller = SigningController(
            wallet: (address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894", credentialId: "cred-1"),
            relay: relay, accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store, pool: RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true),
            ports: SigningController.Ports(knownChains: { [chainId] }),
            timers: timers
        )
        controller.open(SigningController.Incoming(
            id: "r1", method: "eth_sendTransaction",
            paramsJson: #"[{"to":"0x76875e38fc6bc2dedcaed807ce00782db5c0d141","value":"0x1"}]"#,
            origin: "https://x.test", transportId: "tab-1", chainId: chainId
        ))
        return controller
    }

    /// The account read failed: it is the FEE's failure (issue #483) — the
    /// row says "Tap to retry" with the chain's reason, and the footer, the
    /// core's gate over the same fee view, says the fee failed. Before, the
    /// failure was the shell's and the gate never saw it: "Working out the
    /// network fee…" under "Tap to retry".
    @Test func aFailedAccountReadIsTheFeesOwnFailureOnTheRowAndTheFooter() async {
        let port = ScriptedRelayPort()   // eth_getCode unscripted: the chain is silent
        let controller = controller(port)
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.fee?.failed == unreachable }
        #expect(controller.fee?.failed == unreachable)

        let row = SigningLive.feeModel(clear: clear(.clearSign), fee: controller.fee, context: context())
        // PR 2 note 1: the core retries a chain read by itself — the dash on
        // the row, "Retrying…" under the confirm; never a tap asked for.
        #expect(value(row) == "—")
        #expect(warning(row) == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(tappable(row), "a tap still asks at once")
        let footer = controller.confirmState
        #expect(!footer.enabled)
        #expect(footer.key == "componentsUi.signing.confirmBlock.feeRetrying",
                "the footer names another cause than the row: \(String(describing: footer.key))")
        #expect(footer.key != "componentsUi.signing.confirmBlock.feeMeasuring")
        #expect(footer.key != "componentsUi.signing.confirmBlock.feeFailed")
        controller.swipeDismissed()
    }

    /// Every retry is a real read: a refresh after the failure asks the
    /// chain for the account again (`fresh`), it does not replay the failure.
    @Test func aRefreshAfterAFailedAccountReadReadsAgain() async {
        let port = ScriptedRelayPort()
        let controller = controller(port)
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.fee?.failed == unreachable }
        let reads = { port.calls.filter { $0 == "eth_getCode" }.count }
        let before = reads()

        // The chain answers now: the refresh reads it and the run goes on
        // past the account read.
        port.rpc["eth_getCode"] = .ok("0x6080")
        controller.refreshFee()
        await Wait.until { reads() > before }
        #expect(reads() > before, "the refresh did not read the account again")
        await Wait.until { controller.fee?.failed != unreachable }
        #expect(controller.fee?.failed != unreachable, "the refresh replayed the failure")
        controller.swipeDismissed()
    }

    /// 082 iPhone pass (IX6): a chain that never answers the account read —
    /// every node black-holed — is said within the run's bound (the core's
    /// `start_deadline`, run out here by the test's clock), not after the
    /// pool's every pass (估算中… for 4 min 35 s with no reason).
    @Test func aDeploymentReadThatNeverAnswersIsSaidWithinTheRunsBound() async {
        let port = SilentCodePort()
        let controller = controller(port, timers: .stopped)
        await Wait.until { port.codeReadsStarted > 0 }
        #expect(controller.fee?.busy == true, "the row is measuring while the read is out")
        controller.elapseFeeTimer("start_deadline")
        let unreachable = FeeFailureText.chainRead(rateLimited: false).text
        await Wait.until { controller.fee?.failed == unreachable }
        #expect(controller.fee?.failed == unreachable)
        #expect(port.codeReadsThatRanOut == 0, "said within the bound, not after the pool gave up")
        controller.swipeDismissed()
    }

    /// G48: a public node's rate limit on the account read is a rate-limited
    /// chain read — "retrying automatically" — never "can't reach Vela" and
    /// never a shut confirm for ever.
    @Test func aRateLimitedDeploymentReadIsARateLimitedChainRead() async {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .failed(rateLimited: true)
        let controller = controller(port, chainId: 1)
        let limited = FeeFailureText.chainRead(rateLimited: true).text
        await Wait.until { controller.fee?.failed == limited }
        #expect(warning(SigningLive.feeModel(clear: clear(.clearSign), fee: controller.fee, context: context()))
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
    /// `eth_getCode` reads asked for.
    private(set) var codeReadsStarted = 0
    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        if method == "eth_getCode" {
            codeReadsStarted += 1
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
