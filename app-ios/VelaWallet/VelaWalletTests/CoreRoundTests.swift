//
//  CoreRoundTests.swift
//  VelaWalletTests
//
//  PR 2's integration round — what the core now says and this shell draws or
//  calls, pinned where the shell has wiring of its own:
//
//  - note 1/10: the fee's failure is ONE state for the row and the footer, on
//    the Send form, the Send confirm and the signing sheet; the core retries
//    it itself, and a fee session whose surface has gone stops asking;
//  - note 13: Continue's estimate passes the fee machine's failure through as
//    it is, and the alert is worded by its cause, the chain by its name;
//  - note 11: a balance read that never left the app is `internal`, and the
//    home says Vela's own fault — never "Can't reach Ethereum";
//  - the new corpus keys resolve in every language this suite checks.
//
//  Hermetic: the fee and balance views come from the real cores
//  (`FeeCoreScene`, `BalanceCoreScene`), driven with nothing sent.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct CoreRoundTests {

    private let loc = Loc(overrideTag: "en", preferredLanguages: [])
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"

    private func feeView(_ json: String) throws -> FeeViewWire {
        try CoreJSON.decoder.decode(FeeViewWire.self, from: Data(json.utf8))
    }

    private func sendView(stage: String, feeBusy: Bool = false) throws -> SendViewWire {
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = stage
        object["selected_token"] = [
            "network": "chain-100", "chain_id": 100, "symbol": "xDAI", "balance": "5",
            "decimals": 18, "token_address": NSNull(), "price_usd": 1.0, "logo_urls": [], "spam": false,
        ]
        object["recipient"] = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
        object["amount"] = "1"
        object["token_amount"] = "1"
        object["confirm_amount"] = "1"
        object["fee_busy"] = feeBusy
        return try CoreJSON.decode(SendViewWire.self, from: object)
    }

    private var speed: SendLive.SpeedInputs? {
        HandoffFeeFixtures.speedView.map { SendLive.SpeedInputs(view: $0, feeView: { _ in nil }) }
    }

    // MARK: - Note 1/10: one truth for the row and the footer

    /// The core's own views carry the failure through its re-ask.
    @Test func theFeeMachineSaysItsFailureOnceAndKeepsItThroughTheReAsk() throws {
        let down = try #require(FeeCoreScene.chainDown.views(chainId: 100))
        let failed = try feeView(down.failed)
        let failure = try #require(failed.failure)
        #expect(failure.autoRetry && !failure.retrying)
        #expect(failure.figureKey == nil, "never \"Tap to retry\" while the core retries")
        #expect(failure.footerKey == "componentsUi.signing.confirmBlock.feeRetrying")
        #expect(failure.reasonKey == "componentsUi.gas.reasonChainDown")

        let retrying = try feeView(try #require(down.retrying))
        #expect(retrying.busy && retrying.failed == nil)
        #expect(retrying.failure?.retrying == true)
        #expect(retrying.failure?.footerKey == "componentsUi.signing.confirmBlock.feeRetrying")

        let internalFault = try feeView(try #require(FeeCoreScene.internalFault.views(chainId: 100)).failed)
        #expect(internalFault.failure?.reasonKey == "componentsUi.gas.reasonInternal")

        let tap = try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 100)).failed)
        #expect(tap.failure?.autoRetry == false)
        #expect(tap.failure?.figureKey == "componentsUi.gas.estimateFailed")
        #expect(tap.failure?.footerKey == "componentsUi.signing.confirmBlock.feeFailed")
    }

    /// The Send form's row: the reason kept and the measuring sign turning
    /// through the re-ask, the dash — never "Estimating…" and back.
    @Test func theSendFormKeepsTheReasonThroughTheReAsk() throws {
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("SD2 does not draw the form")
            return
        }
        let down = try #require(FeeCoreScene.chainDown.views(chainId: 100))
        let retrying = try feeView(try #require(down.retrying))
        let row = SendLive.form(
            try sendView(stage: "enter_details", feeBusy: true), fee: retrying, display: .usd,
            on: drawn, loc: loc, speed: speed
        ).fee
        #expect(row.value == "—")
        #expect(row.value != loc.t("send.estimatingFee"))
        #expect(row.refreshing, "the measuring sign turns while the core re-asks")
        #expect(row.failNote == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(row.tapRetries)

        // Between re-asks nothing is measuring: the sign is still, and the
        // reason and the dash stay — nothing on the row moves.
        let failed = try feeView(down.failed)
        let between = SendLive.form(
            try sendView(stage: "enter_details"), fee: failed, display: .usd,
            on: drawn, loc: loc, speed: speed
        ).fee
        #expect(!between.refreshing, "the measuring sign turned with nothing out")
        #expect(between.value == row.value)
        #expect(between.failNote == row.failNote)
    }

    /// The confirm: the fee row's own figure and reason, and the one line
    /// under the held button — the sheet's line for the same state; the
    /// previous transaction's line first when both hold.
    @Test func theSendConfirmSaysTheFeesFailureInTheSheetsLine() throws {
        guard case .sendConfirm(let drawn) = WalletFlowFixtures.build(.sd3, loc: loc).base else {
            Issue.record("SD3 does not draw the confirm")
            return
        }
        let down = try #require(FeeCoreScene.chainDown.views(chainId: 100))
        let retrying = try feeView(try #require(down.retrying))
        let view = try sendView(stage: "confirm", feeBusy: true)
        let confirm = SendLive.confirm(
            view, from: (golden, nil), display: .usd, on: drawn, loc: loc, fee: retrying
        )
        let feeFact = try #require(confirm.facts.first { $0.tapRetries })
        #expect(feeFact.value == "—")
        #expect(feeFact.note == loc.t("componentsUi.gas.reasonChainDown", vars: ["chain": "Gnosis"]))
        #expect(confirm.heldNote == loc.t("componentsUi.signing.confirmBlock.feeRetrying"))

        let tap = try feeView(try #require(FeeCoreScene.missingKey.views(chainId: 100)).failed)
        #expect(SendLive.heldNote(view, fee: tap, loc: loc)
                == loc.t("componentsUi.signing.confirmBlock.feeFailed"))
        // No failure: nothing new under the button.
        #expect(SendLive.heldNote(try sendView(stage: "confirm"), fee: nil, loc: loc) == nil)
        // The previous transaction first, whatever the fee says (PR 2 §3).
        var object = try CoreJSON.object(SendCore().view())
        object["stage"] = "confirm"
        object["previous_pending"] = [
            "chain_id": 100, "user_op_hash": "0x" + String(repeating: "a1", count: 32),
            "key": "componentsUi.signing.confirmBlock.previousPending",
        ]
        let held = try CoreJSON.decode(SendViewWire.self, from: object)
        #expect(SendLive.heldNote(held, fee: retrying, loc: loc)
                == loc.t("componentsUi.signing.confirmBlock.previousPending"))
    }

    /// A fee session whose surface has gone asks and answers nothing more:
    /// its timers are dropped, a caller waiting on a quote hears `nil`, and
    /// the next question starts afresh.
    @Test func anEndedFeeStoreStopsAskingAndAnswersItsWaiterNil() async throws {
        let port = SilentCodePort()
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let call: [[String: Any]] = [["to": "0x76875e38fc6bc2dedcaed807ce00782db5c0d141", "value": "1", "data": "0x"]]
        let waiter = Task {
            await fees.quote(
                chainId: 100, account: golden, deployed: false, publicKeyAvailable: true,
                calls: call, feeToken: nil, readDeployment: true
            )
        }
        await Wait.until { port.codeReadsStarted > 0 }
        #expect(fees.view?.busy == true)
        fees.end()
        let answered = await waiter.value
        #expect(answered == nil, "the waiter was left hanging")
        #expect(fees.view == nil, "a journey's fee outlived it")
        await Wait.until { fees.isIdle }
        let started = port.codeReadsStarted
        fees.elapse("start_deadline")
        fees.elapse("start_ttl")
        for _ in 0..<50 { await Task.yield() }
        #expect(port.codeReadsStarted == started, "an ended session asked again")
        #expect(port.codeReadsThatRanOut == 0, "the read out was not cancelled")
    }

    // MARK: - Note 13: Continue's estimate by its cause

    /// The fee machine's failure goes to the send machine as it is — the
    /// object form as an object — and `other` only when none was said.
    @Test func continuesEstimatePassesTheFeeFailureThrough() throws {
        #expect(SendExecutor.estimateFailure("internal") as? String == "internal")
        #expect(SendExecutor.estimateFailure("would_fail") as? String == "would_fail")
        #expect(SendExecutor.estimateFailure("quote_unavailable") as? String == "quote_unavailable")
        let chainRead = try #require(
            SendExecutor.estimateFailure(FeeFailureText.chainRead(rateLimited: false).text) as? [String: Any]
        )
        #expect((chainRead["chain_read"] as? [String: Any])?["rate_limited"] as? Bool == false)
        #expect(SendExecutor.estimateFailure(nil) as? String == "other")
        // …and the core reads every one of them back as itself: none falls
        // to the general sentence for want of a word.
        let kinds: [Any] = ["internal", "would_fail", "quote_unavailable", "timeout", "other", chainRead]
        let keys = kinds.map { sendEstimateFailureBodyKey(failure: SendLive.estimateFailureText($0)) }
        #expect(keys == [
            "componentsUi.gas.reasonInternal", "send.alertEstimateFailedBody", "send.alertEstimateFailedBody",
            "send.alertEstimateFailedBody", "send.alertEstimateFailedBody", "send.alertEstimateChainDownBody",
        ], "\(keys)")
    }

    /// The alert's body names the cause: the chain by its name, Vela's own
    /// fault as that, else the general sentence.
    @Test func continuesAlertIsWordedByItsCause() throws {
        func body(_ kind: Any) -> String {
            SendLive.alertText(["type": "estimate_failed", "kind": kind], loc: loc, chain: "Gnosis").body
        }
        let chainRead = SendExecutor.estimateFailure(FeeFailureText.chainRead(rateLimited: false).text)
        #expect(body(chainRead) == loc.t("send.alertEstimateChainDownBody", vars: ["chain": "Gnosis"]))
        #expect(body(chainRead).contains("Gnosis"))
        #expect(body("internal") == loc.t("componentsUi.gas.reasonInternal"))
        #expect(!body("internal").contains("Gnosis"))
        #expect(body("quote_unavailable") == loc.t("send.alertEstimateFailedBody"))
        #expect(body("other") == loc.t("send.alertEstimateFailedBody"))
        #expect(SendLive.alertText(["type": "estimate_failed", "kind": "internal"], loc: loc).title
                == loc.t("send.alertEstimateFailedTitle"))
        for kind in [chainRead, "internal", "other"] as [Any] {
            #expect(!body(kind).contains("{{"), "\(kind)")
        }
    }

    // MARK: - Note 11: home with an internal fault

    /// A read that cannot leave the app is Vela's fault: not tried, failed,
    /// and said as `internal` — never the network out of reach.
    @Test func aReadThatNeverLeftTheAppIsInternal() async throws {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: AccountStore(defaults: defaults), offline: true)
        pool.faultChain(1)
        #expect(pool.unsendable(chainId: 1) != nil)
        #expect(pool.unsendable(chainId: 100) == nil)

        let faulted = await TokenReads.read(address: golden, chainId: 1, tokens: [], pool: pool)
        #expect(faulted.failed && faulted.internalFault)
        // No network (the offline pool): the chain's failure, not Vela's.
        let offline = await TokenReads.read(address: golden, chainId: 100, tokens: [], pool: pool)
        #expect(offline.failed && !offline.internalFault)
        // A request this build could not write: nothing was sent.
        let noAddress = await TokenReads.read(address: "nope", chainId: 100, tokens: [], pool: pool)
        #expect(noAddress.failed && noAddress.internalFault)

        let executor = BalanceExecutor(store: store, pool: pool, held: HeldTokens(), chainDeadlineMs: .max)
        let settled = try CoreJSON.object(await executor.perform([
            "type": "fetch_tokens", "address": golden, "pull": false,
        ]))
        #expect(settled["type"] as? String == "fetch_settled")
        let internalIds = settled["internal_chain_ids"] as? [Int] ?? []
        let failedIds = settled["failed_chain_ids"] as? [Int] ?? []
        #expect(internalIds == [1], "\(internalIds)")
        #expect(Set(internalIds).isSubset(of: Set(failedIds)), "internal ⊆ failed")
        // The whole fetch with no account to read failed inside the app.
        let errored = try CoreJSON.object(await executor.perform([
            "type": "fetch_tokens", "address": "", "pull": false,
        ]))
        #expect(errored["type"] as? String == "fetch_errored")
        #expect(errored["internal"] as? Bool == true)
    }

    /// The fee's account read through a pool that cannot send is `internal`.
    @Test func aFaultedPoolsDeploymentReadIsInternal() async {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let pool = RpcPool(store: VelaStore(defaults: defaults), accounts: AccountStore(defaults: defaults))
        pool.faultChain(137)
        let relay = RelayClient(port: PoolRelayPort(pool: pool), now: { 0 }, retryDelayMs: 0)
        let read = await relay.deploymentRead(chainId: 137, address: golden)
        guard case .internal = read else {
            Issue.record("a faulted pool's read was \(read)")
            return
        }
        let outcome = await pool.callDetailed(chainId: 137, method: "eth_blockNumber")
        #expect(!outcome.maybeDelivered)
        #expect(!pool.booted, "nothing was routed")
    }

    /// The home says Vela's own fault where the unreachable line goes — and a
    /// chain really down still reads "Can't reach Ethereum".
    @Test func homeWithAnInternalFaultNeverReadsCantReachEthereum() throws {
        let fallback = WalletFixtures.buildMobileState(.h1, loc: loc).balance
        let faulted = try #require(BalanceCoreScene.view(internalFault: true))
        #expect(faulted.internalChainIds == [1])
        #expect(faulted.internalKey == "componentsUi.gas.reasonInternal")
        #expect(faulted.unreachableNetworks.isEmpty, "an internal chain is no network out of reach")
        let line = WalletLive.balance(faulted, fallback: fallback, loc: loc).status?.text
        #expect(line == loc.t("componentsUi.gas.reasonInternal"))
        #expect(line?.contains("Ethereum") == false)

        let down = try #require(BalanceCoreScene.view(internalFault: false))
        #expect(down.internalKey == nil)
        let downLine = WalletLive.balance(down, fallback: fallback, loc: loc).status?.text
        #expect(downLine == loc.t("assets.unreachableOne", vars: ["name": "Ethereum"]), "\(String(describing: downLine))")

        // The breakdown lists the chain with the same sentence, never "RPC unavailable".
        let detail = SettingsLive.withBalanceDetail(
            faulted, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).balanceDetail
        let row = try #require(detail.pending.first { $0.id == "1" })
        #expect(row.status == loc.t("componentsUi.gas.reasonInternal"))
    }

    /// PR 3 note 4 — a token list that can't be loaded is not a network out
    /// of reach. The REAL `balance_dashboard` core over a round in which
    /// Tempo failed for want of its list (`registry_chain_ids`): the home
    /// says the LIST could not be loaded, never "Can't reach Tempo"; the
    /// row says what kept it from being read and that an RPC fix cannot
    /// help; and no surface offers one — not the list, not its door.
    @Test func aTokenListThatCantLoadIsNotANetworkOutOfReach() throws {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        let fallback = WalletFixtures.buildMobileState(.h1, loc: loc).balance
        let view = try #require(BalanceCoreScene.view(failedChain: 4217, internalFault: false, tokenListFault: true))
        #expect(view.unreachableKey == I18nKeys.SettingsUi.tokenListUnreachable)
        #expect(view.unreachableKey == "assets.tokenListUnreachable")
        let row = try #require(view.unreachableNetworks.first)
        #expect(view.unreachableNetworks.count == 1 && row.chainId == 4217)
        #expect(row.cause == .tokenList)
        #expect(!row.rpcFixable, "the core says an RPC fix can help a chain whose RPC is fine")

        // The home's line, resolved like any other key the core names.
        let line = WalletLive.balance(view, fallback: fallback, loc: loc).status?.text
        #expect(line == "Can't load Tempo's token list right now", "\(String(describing: line))")
        #expect(line?.contains("Can't reach") == false)
        #expect(WalletLive.balance(view, fallback: fallback, loc: zh).status?.text == "暂时读不到 Tempo 的代币列表")
        #expect(WalletLive.unreachableLine(view, loc: loc)?.contains("{{") == false)

        // The list it opens: Tempo's row, its line, and NO "Fix".
        let list = SettingsLive.withUnreachable(
            view, display: .usd, on: SettingsFixtures.build(.sr6, loc: loc), loc: loc
        ).unreachable
        #expect(list.title == line, "the list is titled by the home's own line")
        #expect(list.rows.count == 1)
        #expect(list.rows.first?.name == "Tempo")
        #expect(list.rows.first?.action == nil, "a chain whose RPC is fine is offered an RPC fix")
        #expect(list.rows.first?.line == loc.t("assets.notReadYet"))

        // The balance detail's row never reads "RPC unavailable" for it: its
        // short status is the core's (`statusKey`, final note F21) — two
        // words like every other row's, not the home's whole sentence.
        #expect(row.statusKey == "home.balanceDetailStatusTokenList")
        let detail = SettingsLive.withBalanceDetail(
            view, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).balanceDetail
        let pending = try #require(detail.pending.first { $0.id == "4217" })
        #expect(pending.status == "Token list unavailable")
        #expect(pending.status != loc.t("home.balanceDetailStatusFailed"))
        #expect(SettingsLive.withBalanceDetail(
            view, display: .usd, on: SettingsFixtures.build(.st1, loc: zh), loc: zh
        ).balanceDetail.pending.first { $0.id == "4217" }?.status == "代币列表无法读取")

        // The same chain really out of reach is the network's, with its Fix.
        let down = try #require(BalanceCoreScene.view(failedChain: 4217, internalFault: false))
        let downRow = try #require(down.unreachableNetworks.first)
        #expect(downRow.cause == .network && downRow.rpcFixable)
        #expect(down.unreachableKey == I18nKeys.SettingsUi.unreachableOne)
        #expect(WalletLive.balance(down, fallback: fallback, loc: loc).status?.text == "Can't reach Tempo right now")
        let downList = SettingsLive.withUnreachable(
            down, display: .usd, on: SettingsFixtures.build(.sr6, loc: loc), loc: loc
        ).unreachable
        #expect(downList.rows.first?.action == loc.t("assets.rpcFix"))
        #expect(downRow.statusKey == "home.balanceDetailStatusFailed")
        #expect(SettingsLive.withBalanceDetail(
            down, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).balanceDetail.pending.first { $0.id == "4217" }?.status == "RPC unavailable")
    }

    /// Final note F21 — the row's status is whatever key the core names, and
    /// a rate-limited chain keeps "retrying"; a row from before the key reads
    /// "RPC unavailable", the only status there was.
    @Test func theBreakdownsStatusIsTheCoresKey() throws {
        var view = try #require(BalanceCoreScene.view(failedChain: 4217, internalFault: false, tokenListFault: true))
        func status(_ id: Int) -> String? {
            SettingsLive.withBalanceDetail(
                view, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
            ).balanceDetail.pending.first { $0.id == String(id) }?.status
        }
        // Not derived from `rpcFixable` or `cause`: the key alone decides.
        view.unreachableNetworks = [UnreachableNetworkWire(
            chainId: 4217, lastKnown: "not_read", lastSeenUsd: nil, lineKey: "assets.notReadYet",
            cause: .network, rpcFixable: true, statusKey: "home.balanceDetailStatusTokenList"
        )]
        #expect(status(4217) == "Token list unavailable")
        // The transient subset: it resolves itself, and says so.
        view = BalanceViewWire(
            address: view.address, displayTotalUsd: view.displayTotalUsd,
            balanceUnknown: false, balancePartial: true, notice: nil, hidden: false,
            refreshing: false, lastRefreshedAtMs: nil, tokens: view.tokens, unpricedTokens: [],
            failedChainIds: [4217], rateLimitedChainIds: [4217],
            unreachableNetworks: view.unreachableNetworks, unreachableKey: view.unreachableKey,
            holdingsLoading: false, cachedTotalUsd: nil, switcher: view.switcher
        )
        #expect(status(4217) == loc.t("home.balanceDetailStatusRetrying"))

        let old = try CoreJSON.decode(UnreachableNetworkWire.self, from: [
            "chain_id": 56, "last_known": "not_read", "last_seen_usd": NSNull(),
            "line_key": "assets.notReadYet",
        ])
        #expect(old.statusKey == "home.balanceDetailStatusFailed")
        let keyed = try CoreJSON.decode(UnreachableNetworkWire.self, from: [
            "chain_id": 4217, "last_known": "not_read", "last_seen_usd": NSNull(),
            "line_key": "assets.notReadYet", "cause": "token_list", "rpc_fixable": false,
            "status_key": "home.balanceDetailStatusTokenList",
        ])
        #expect(keyed.statusKey == "home.balanceDetailStatusTokenList")
        // The two statuses resolve in every language.
        for lang in ["en", "zh", "zh-TW", "zh-HK", "ja", "ko", "de", "fr", "es-MX", "it", "pt-BR", "ru", "tr", "vi", "id"] {
            let other = Loc(overrideTag: lang, preferredLanguages: [])
            for key in ["home.balanceDetailStatusTokenList", "home.balanceDetailStatusFailed"] {
                #expect(other.t(key) != key, "\(key) \(lang)")
            }
        }
    }

    /// Final note F20 — a heading is drawn with its rows and not without
    /// them. A healthy wallet's breakdown (opened from "Some tokens couldn't
    /// be priced.") has no network out: it must not be told "These networks
    /// couldn't be reached, so your cached balance is shown".
    @Test func theBreakdownHeadsOnlyTheRowsItHas() throws {
        let healthy = try #require(BalanceCoreScene.zeroWallet()).settled
        let none = SettingsLive.withBalanceDetail(
            healthy, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).balanceDetail
        #expect(none.pending.isEmpty && none.done.isEmpty)
        #expect(!BalanceDetailSheetBody.headsPending(none), "\"Networks still updating\" heads an empty list")
        #expect(!BalanceDetailSheetBody.headsDone(none), "\"Updated\" heads an empty list")

        let down = try #require(BalanceCoreScene.view(internalFault: false))
        let some = SettingsLive.withBalanceDetail(
            down, display: .usd, on: SettingsFixtures.build(.st1, loc: loc), loc: loc
        ).balanceDetail
        #expect(BalanceDetailSheetBody.headsPending(some) && BalanceDetailSheetBody.headsDone(some))
    }

    /// The two new fields ride on the wire under these names; a row from
    /// before them (a hand-written view, an older core) is the network's,
    /// fixable — the core's own defaults — and a cause this build has never
    /// heard of still decodes, its `rpc_fixable` deciding the button.
    @Test func theUnreachableRowsCauseDecodesAndAnAbsentOneIsTheNetworks() throws {
        func decode(_ extra: [String: Any]) throws -> UnreachableNetworkWire {
            var json: [String: Any] = [
                "chain_id": 4217, "last_known": "not_read", "last_seen_usd": NSNull(),
                "line_key": "assets.notReadYet",
            ]
            json.merge(extra) { _, new in new }
            return try CoreJSON.decode(UnreachableNetworkWire.self, from: json)
        }
        let list = try decode(["cause": "token_list", "rpc_fixable": false])
        #expect(list.cause == .tokenList && !list.rpcFixable)
        let network = try decode(["cause": "network", "rpc_fixable": true])
        #expect(network.cause == .network && network.rpcFixable)
        let old = try decode([:])
        #expect(old.cause == .network && old.rpcFixable)
        let novel = try decode(["cause": "something_new", "rpc_fixable": false])
        #expect(!novel.rpcFixable, "an unknown cause lost the core's own answer about the button")
        // A view carrying a list row draws no Fix; several networks are still counted.
        var many = try #require(BalanceCoreScene.view(internalFault: false))
        many.unreachableNetworks = [network, list]
        many.unreachableKey = I18nKeys.SettingsUi.unreachableMany
        #expect(WalletLive.unreachableLine(many, loc: loc) == "Can't reach 2 networks right now")
        let rows = SettingsLive.withUnreachable(
            many, display: .usd, on: SettingsFixtures.build(.sr6, loc: loc), loc: loc
        ).unreachable.rows
        #expect(rows.map { $0.action != nil } == [true, false])
    }

    /// Every chain's read failed, nothing cached (PR 2 integration): no
    /// settled "$0.00" and no "Deposit your first asset" under the reason —
    /// the skeleton, and Vela's own sentence when the fault was its own, else
    /// the request that never arrived.
    @Test func homeWithNothingReadIsASkeletonAndItsReason() throws {
        let home = WalletFixtures.buildMobileState(.h1, loc: loc)
        let faulted = try #require(BalanceCoreScene.view(internalFault: true, everyChain: true))
        #expect(faulted.unreachable == true, "every chain failed and nothing is known")
        let hero = WalletLive.balance(faulted, fallback: home.balance, loc: loc)
        #expect(hero.state == .loading)
        #expect(hero.integer == nil, "a figure was drawn over nothing read")
        #expect(hero.status?.text == loc.t("componentsUi.gas.reasonInternal"))
        let section = WalletLive.assetsSection(faulted, rows: [], fallback: home.assetsSection)
        #expect(section.mode == .loading, "\"nothing here\" is a claim nobody could make")

        let down = try #require(BalanceCoreScene.view(internalFault: false, everyChain: true))
        #expect(down.unreachable == true)
        #expect(WalletLive.balance(down, fallback: home.balance, loc: loc).status?.text
                == loc.t("onboarding.common.networkBody"))
    }

    // MARK: - The corpus

    /// Every key the core round hands over resolves, in English and Chinese.
    @Test func theCoreRoundsKeysResolve() {
        let zh = Loc(overrideTag: "zh", preferredLanguages: [])
        for key in I18nKeys.CoreRound.all {
            #expect(I18nKeys.all.contains(key))
            #expect(loc.t(key) != key, "\(key) en")
            #expect(zh.t(key) != key, "\(key) zh")
        }
    }
}
