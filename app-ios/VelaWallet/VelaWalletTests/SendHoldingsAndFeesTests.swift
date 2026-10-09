//
//  SendHoldingsAndFeesTests.swift
//  VelaWalletTests
//
//  Spec 078 on iOS: Send reads the asset list's holdings (one source) and
//  follows its rounds; the fee coin is the fee machine's pick until somebody
//  taps one; the fee reads are single-flight, so the three speeds are priced
//  from ONE request each; the picker warms the fee caches; the amount figure
//  steps down the hero ladder.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

/// A relay whose every REPEAT of a method answers later than the one before —
/// the live relay under three identical requests at once. Uniform latency would
/// let three separate reads land together by luck; with this, only a shared
/// read makes the three answers arrive at the same moment.
@MainActor
final class StaggeredRelayPort: RelayPort {
    var rpc: [String: RpcOutcome] = [:]
    var rest: [String: CoreHTTP.RestAnswer] = [:]
    var baseMs: UInt64 = 30
    var staggerMs: UInt64 = 150
    private(set) var counts: [String: Int] = [:]

    func call(chainId: Int, method: String, params: [Any], kind: String) async -> RpcOutcome {
        let repeatIndex = counts[method, default: 0]
        counts[method] = repeatIndex + 1
        try? await Task.sleep(nanoseconds: (baseMs + UInt64(repeatIndex) * staggerMs) * 1_000_000)
        return rpc[method] ?? .failed(rateLimited: false)
    }

    func bundlerBase(chainId: Int) async -> String? { "https://relay.test" }
    func bestRpcUrl(chainId: Int) async -> String? { "https://rpc.test" }

    func restGet(url: String, xRpcUrl: String?) async -> CoreHTTP.RestAnswer {
        counts["REST", default: 0] += 1
        for (suffix, answer) in rest where url.hasSuffix(suffix) { return answer }
        return .failed
    }
}

/// `timeLimit`: several waits here are for a port the executor calls, which no
/// machine's idleness bounds (`Waits.swift`); one that never comes is a hang,
/// and this is what reports it. Far above any real run, loaded or not.
///
/// Every fee store here runs with the core's timers stopped (`timers:
/// .stopped`). The quote's 15 s deadline (spec 094 S9) used to run on the
/// wall clock under these scripted relays, and a starved CI runner — 18 s to
/// hand back a 30 ms read — outlasted it: the automatic quote came back
/// `quote_unavailable` and no speed row ever settled (`chain_read`), for
/// quotes the code never failed.
@MainActor
@Suite(.timeLimit(.minutes(10)))
struct SendHoldingsAndFeesTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let usdc = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"

    // MARK: - Fixtures

    /// Everything a Gnosis quote reads, answered. `nativeWei` / `usdcUnits`
    /// are what the account holds of each fee coin (hex, as the relay writes).
    private func scriptFeeReads(_ port: StaggeredRelayPort, nativeWei: String = "0xde0b6b3a7640000", usdcUnits: String? = nil) {
        port.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x3b9aca00")
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "slow": ["maxFeePerGas": "0x3b9aca00"] as [String: Any],
            "standard": ["maxFeePerGas": "0x59682f00"] as [String: Any],
            "fast": ["maxFeePerGas": "0x77359400"] as [String: Any],
        ] as [String: Any])
        var rows: [[String: Any]] = [[
            "recipient": golden, "asset": "native", "feeToken": NSNull(),
            "balance": nativeWei, "decimals": 18, "symbol": "XDAI",
            "usdBalance": nativeWei == "0x0" ? "0" : "1", "usdPrice": "1",
        ]]
        if let usdcUnits {
            rows.append([
                "recipient": golden, "asset": "erc20", "feeToken": usdc,
                "balance": usdcUnits, "decimals": 6, "symbol": "USDC",
                "usdBalance": "500", "usdPrice": "1",
            ])
        }
        port.rpc["vela_getInBandGasQuote"] = .ok(rows)
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0",
            "callGasLimit": "0x186a0",
            "preVerificationGas": "0x186a0",
        ] as [String: Any])
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
    }

    private func balance(
        _ tokens: [(symbol: String, amount: String, price: Double?)], refreshedAt: Double? = 1,
        failed: [Int] = [], unreachable: Bool = false
    ) throws -> BalanceViewWire {
        let rows = tokens.map { token in
            """
            {"chain_id": 100, "symbol": "\(token.symbol)", "name": "\(token.symbol)",
             "balance": "\(token.amount)", "decimals": 18,
             "token_address": \(token.symbol == "xDAI" ? "null" : "\"0x00000000000000000000000000000000000000\(token.symbol.count)\""),
             "price_usd": \(token.price.map { String($0) } ?? "null"), "spam": false}
            """
        }.joined(separator: ",")
        let unpriced = tokens.filter { $0.price == nil }.map { token in
            """
            {"chain_id": 100, "symbol": "\(token.symbol)", "name": "\(token.symbol)",
             "balance": "\(token.amount)", "decimals": 18,
             "token_address": "0x00000000000000000000000000000000000000\(token.symbol.count)",
             "price_usd": null, "spam": false}
            """
        }.joined(separator: ",")
        let json = """
        {
          "address": "\(golden)",
          "display_total_usd": 0.53, "balance_unknown": false, "balance_partial": false,
          "unreachable": \(unreachable), "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": \(refreshedAt.map { String($0) } ?? "null"),
          "tokens": [\(rows)], "unpriced_tokens": [\(unpriced)],
          "failed_chain_ids": \(failed), "rate_limited_chain_ids": [], "unreachable_networks": [],
          "holdings_loading": false, "cached_total_usd": null,
          "switcher": { "open": false, "loading": false, "balances": [] }
        }
        """
        return try CoreJSON.decode(BalanceViewWire.self, from: CoreJSON.object(json))
    }

    private func networks() throws -> NetViewWire {
        let json = """
        {
          "loaded": true, "last_added_chain_id": null, "endpoints": [], "providers": [],
          "networks": [{
            "id": "chain-100", "chain_id": 100, "display_name": "Gnosis",
            "native_symbol": "xDAI", "is_custom": false,
            "rpc_url": "", "explorer_url": "", "bundler_url": "",
            "rpc_health": null, "explorer_health": null, "rpc_chain_mismatch": null,
            "rpc_save_deferred": false
          }],
          "wizard": {
            "phase": "idle", "query": "", "custom_rpc": "", "suggestions": [],
            "chain_info": null, "compat": null, "error": null, "can_add": false
          }
        }
        """
        return try CoreJSON.decode(NetViewWire.self, from: CoreJSON.object(json))
    }

    private func executor(
        relay: RelayClient,
        fees: FeeStore,
        balances: @escaping () -> BalanceViewWire?,
        round: @escaping (String) -> Int? = { _ in 0 },
        open: @escaping (String) -> Void = { _ in },
        ports: SendExecutor.Ports = SendExecutor.Ports()
    ) throws -> SendExecutor {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accounts)
        let accountPort = ScriptedAccounts()
        let nets = try networks()
        return SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort, fees: fees,
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts,
            balances: balances, networks: { nets },
            holdingsRound: round, openHoldings: open,
            ports: ports
        )
    }

    // MARK: - Single flight

    /// N callers of one key while it is out → one run of the work, one answer.
    ///
    /// Gated, not timed: the first flight is held open until the other three
    /// have provably joined it, so a loaded machine cannot turn "concurrent"
    /// into "one after another" (which would, correctly, run the work again).
    @Test func concurrentCallersOfOneKeyShareOneFlight() async {
        let flight = SingleFlight<String, Int>()
        var runs = 0
        var release: CheckedContinuation<Void, Never>?
        let first = Task {
            await flight.run("k") {
                runs += 1
                await withCheckedContinuation { release = $0 }
                return 7
            }
        }
        while release == nil { await Task.yield() }
        let others = (0..<3).map { index in
            Task { await flight.run("k") { runs += 1; return 8 + index } }
        }
        while flight.joined("k") < 3 { await Task.yield() }
        release?.resume()
        var answers = [await first.value]
        for other in others { answers.append(await other.value) }
        #expect(runs == 1, "four callers, \(runs) runs")
        #expect(flight.started == 1)
        #expect(answers == [7, 7, 7, 7], "every caller gets the one answer: \(answers)")
        #expect(!flight.inFlight("k"))
        // Finished flights are forgotten: the next caller runs again.
        #expect(await flight.run("k") { runs += 1; return 11 } == 11)
        #expect(runs == 2)
        // And a different key never joins another's flight.
        var gate: CheckedContinuation<Void, Never>?
        let x = Task { await flight.run("x") { runs += 1; await withCheckedContinuation { gate = $0 }; return 1 } }
        while gate == nil { await Task.yield() }
        #expect(await flight.run("y") { runs += 1; return 2 } == 2)
        gate?.resume()
        #expect(await x.value == 1)
        #expect(runs == 4)
    }

    /// Three tiers asked at once → ONE `pimlico_getUserOperationGasPrice`, and
    /// every tier served from it — the three rows arrive together.
    @Test func threeTiersAreServedFromOneGasPriceRead() async {
        let port = StaggeredRelayPort()
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let clock = ContinuousClock()
        let start = clock.now
        // Main-actor tasks, enqueued back to back: each reaches the relay
        // client before the first read can answer (`async let` children start
        // on the global pool, which a loaded suite can delay past it).
        let asks = ["fast", "standard", "slow"].map { tier in
            Task { await timed(clock, start) { await relay.bundlerQuote(chainId: 100, tier: tier) } }
        }
        var rows: [(value: [String: Any]?, ms: Int)] = []
        for ask in asks { rows.append(await ask.value) }
        #expect(port.counts["pimlico_getUserOperationGasPrice"] == 1)
        #expect(relay.bundlerPriceReads == 1)
        #expect(rows[0].value?["max_fee_per_gas"] as? String == "2000000000")
        #expect(rows[1].value?["max_fee_per_gas"] as? String == "1500000000")
        #expect(rows[2].value?["max_fee_per_gas"] as? String == "1000000000")
        // Printed, not asserted: the whole suite shares the main actor, so
        // wall-clock spread measures the other tests too. The count above is
        // the proof — one read cannot answer three rows at three moments.
        // (Run alone: [51, 51, 51] ms; with single-flight disabled as a
        // control: 3 reads, [33, 187, 334] ms.)
        print("[078] three tiers settled at \(rows.map(\.ms)) ms after asking")
        // Held for the window, for every tier.
        _ = await relay.bundlerQuote(chainId: 100, tier: "standard")
        #expect(port.counts["pimlico_getUserOperationGasPrice"] == 1)
        // A fresh reading asked for → a new read, not the held one.
        relay.invalidateFeeSignals(chainId: 100)
        _ = await relay.bundlerQuote(chainId: 100, tier: "fast")
        #expect(port.counts["pimlico_getUserOperationGasPrice"] == 2)
    }

    private func timed<T>(
        _ clock: ContinuousClock, _ start: ContinuousClock.Instant, _ work: () async -> T
    ) async -> (value: T, ms: Int) {
        let value = await work()
        let elapsed = clock.now - start
        return (value, Int(elapsed.components.seconds * 1000 + elapsed.components.attoseconds / 1_000_000_000_000_000))
    }

    /// The gas signals, the in-band rows and the deployment read: concurrent
    /// identical asks are one read each.
    @Test func concurrentFeeReadsAreSingleFlight() async {
        let port = StaggeredRelayPort()
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let g = (0..<3).map { _ in Task { await relay.gasSignals(chainId: 100, wantTip: true) } }
        let q = (0..<2).map { _ in Task { await relay.inBandQuotes(chainId: 100, safe: golden) } }
        let d = (0..<2).map { _ in Task { await relay.isDeployed(chainId: 100, address: golden) } }
        var signals: [RelayClient.GasSignals] = []
        for task in g { signals.append(await task.value) }
        var quotes: [[[String: Any]]?] = []
        for task in q { quotes.append(await task.value) }
        var deployed: [Bool?] = []
        for task in d { deployed.append(await task.value) }
        #expect(port.counts["eth_gasPrice"] == 1)
        #expect(port.counts["vela_getInBandGasQuote"] == 1)
        #expect(port.counts["eth_getCode"] == 1)
        #expect(Set(signals.map(\.ethGasPrice)) == ["1000000000"])
        #expect(quotes.allSatisfy { $0?.count == 1 })
        #expect(deployed == [true, true])
    }

    /// A deployed Safe is remembered for good; an undeployed one is asked
    /// every time (the next send deploys it).
    @Test func deploymentIsHeldOnlyOnceTrue() async {
        let port = StaggeredRelayPort()
        port.baseMs = 0
        port.staggerMs = 0
        port.rpc["eth_getCode"] = .ok("0x")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        #expect(await relay.isDeployed(chainId: 100, address: golden) == false)
        #expect(await relay.isDeployed(chainId: 100, address: golden) == false)
        #expect(port.counts["eth_getCode"] == 2)
        port.rpc["eth_getCode"] = .ok("0x6080")
        #expect(await relay.isDeployed(chainId: 100, address: golden) == true)
        port.rpc["eth_getCode"] = .ok("0x")
        #expect(await relay.isDeployed(chainId: 100, address: golden) == true)
        relay.invalidateFeeSignals(chainId: 100)
        #expect(await relay.isDeployed(chainId: 100, address: golden) == true)
        #expect(port.counts["eth_getCode"] == 3)
    }

    /// The simulation of one exact operation, asked by three sessions at once
    /// → one relay estimate; held until the signals are dropped.
    @Test func theSimulationIsOneReadForEverySessionAskingIt() async throws {
        let port = StaggeredRelayPort()
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let executor = FeeExecutor(relay: relay, accounts: ScriptedAccounts())
        let op: [String: Any] = [
            "type": "estimate_user_op_gas", "chain_id": 100, "account": golden, "deployed": true,
            "calls": [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
        ]
        let asks = (0..<3).map { _ in Task { await executor.perform(op) } }
        var answers: [String] = []
        for ask in asks { answers.append(await ask.value) }
        #expect(port.counts["eth_estimateUserOperationGas"] == 1)
        #expect(Set(answers).count == 1)
        let outcome = try CoreJSON.object(answers[0])["outcome"] as? [String: Any]
        #expect(outcome?["type"] as? String == "estimated")
        _ = await executor.perform(op)
        #expect(port.counts["eth_estimateUserOperationGas"] == 1, "held for the window")
        // A submit drops it with the signals.
        relay.invalidateFeeSignals(chainId: 100)
        _ = await executor.perform(op)
        #expect(port.counts["eth_estimateUserOperationGas"] == 2)
    }

    /// The Ethereum fee fix: the relay's `settlementGas` crosses to the core
    /// beside the limits — "0x0" or an absent field is no figure, never "0" —
    /// and each tier's published in-band price crosses beside its cap.
    @Test func theRelaysSettlementGasAndInBandPriceCrossToTheCore() async throws {
        let port = StaggeredRelayPort()
        port.staggerMs = 0
        scriptFeeReads(port)
        let limits: [String: Any] = [
            "verificationGasLimit": "0x186a0", "callGasLimit": "0x1c0ce", "preVerificationGas": "0x18cf5",
        ]
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        for (settlement, expected) in [("0x3086c", "198764"), ("0x0", nil), (nil, nil)] as [(String?, String?)] {
            var answer = limits
            if let settlement { answer["settlementGas"] = settlement }
            port.rpc["eth_estimateUserOperationGas"] = .ok(answer)
            switch await relay.estimateUserOpGas(chainId: 1, opJson: "{}") {
            case .estimated(let verification, let call, let preVerification, let settled):
                #expect([verification, call, preVerification] == ["100000", "114894", "101621"])
                #expect(settled == expected, "\(settlement ?? "absent")")
            default:
                Issue.record("expected an estimate")
            }
        }
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "standard": ["maxFeePerGas": "0x186f6d82e", "inBandFeePerGas": "0x20fbf28f2"] as [String: Any],
            "fast": ["maxFeePerGas": "0x77359400"] as [String: Any],
        ] as [String: Any])
        let executor = FeeExecutor(relay: relay, accounts: ScriptedAccounts())
        let standard = try CoreJSON.object(await executor.perform(["type": "fetch_bundler_quote", "chain_id": 1, "tier": "standard"]))
        #expect((standard["quote"] as? [String: Any])?["in_band_fee_per_gas"] as? String == "8854120690")
        let fast = try CoreJSON.object(await executor.perform(["type": "fetch_bundler_quote", "chain_id": 1, "tier": "fast"]))
        #expect((fast["quote"] as? [String: Any])?["in_band_fee_per_gas"] is NSNull)
    }

    /// The speed control open: the session in force and both previews price
    /// the same operation, and every relay question goes out ONCE — the three
    /// rows settle together instead of one after another.
    @Test func theThreeSpeedsSettleFromOneRequestEach() async throws {
        let port = StaggeredRelayPort()
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        fees.configureSpeed(preferred: "fast", number: "comma_dot")
        fees.speedStage(onForm: true)
        fees.toggleSpeed()
        let clock = ContinuousClock()
        let start = clock.now
        var settledAt: [String: Int] = [:]
        let returned = Flag()
        let quote = Task {
            let view = await fees.quote(
                chainId: 100, account: golden, deployed: true, publicKeyAvailable: true,
                calls: [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
                feeToken: nil
            )
            returned.set()
            return view
        }
        // Until every row has settled, or nothing is left in flight that could
        // settle one — never 600 × 5 ms (`Waits.swift`).
        await Wait.until({
            for tier in ["fast", "standard", "slow"] where settledAt[tier] == nil {
                if let view = fees.view(of: tier), !view.busy, view.fee != nil {
                    let elapsed = clock.now - start
                    settledAt[tier] = Int(elapsed.components.seconds * 1000
                        + elapsed.components.attoseconds / 1_000_000_000_000_000)
                }
            }
            return settledAt.count == 3
        }, orIdle: { returned.isSet && fees.isIdle })
        _ = await quote.value
        print("[078] speed rows settled at \(settledAt) ms; relay calls \(port.counts)")
        #expect(settledAt.count == 3, "every speed priced: \(settledAt)")
        #expect(port.counts["pimlico_getUserOperationGasPrice"] == 1)
        #expect(port.counts["eth_estimateUserOperationGas"] == 1)
        #expect(port.counts["vela_getInBandGasQuote"] == 1)
        #expect(port.counts["eth_gasPrice"] == 1)
        // The timestamps are printed rather than asserted, for the reason
        // given above. Run alone they read 184/184/184 ms; with single-flight
        // disabled, 171/558/1015 ms and three reads of each question.
    }

    /// The stopped clock every store here runs on: the core's timers are held,
    /// never answered — a quote cannot be failed by its deadline however long
    /// the runner takes to hand it its answers, nor go stale — and `isIdle`
    /// does not wait on them. The app's clock answers both.
    @Test func aStoppedClockHoldsTheTimersAndNotTheIdleBound() async throws {
        let port = StaggeredRelayPort()
        port.baseMs = 0
        port.staggerMs = 0
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let view = await fees.quote(
            chainId: 100, account: golden, deployed: true, publicKeyAvailable: true,
            calls: [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
            feeToken: nil
        )
        #expect(view?.fee != nil && view?.failed == nil, "\(String(describing: view))")
        // Priced, with the deadline and the staleness timer held: idle. (Were
        // they counted, this would wait out the suite's time limit.)
        await Wait.until { fees.isIdle }
        #expect(fees.view?.stale == false, "no time passed")
        // The app's clock answers them — after 0 ms here: nothing is timed.
        let executor = FeeExecutor(relay: relay, accounts: ScriptedAccounts())
        for (timer, elapsed) in [("start_deadline", "deadline_elapsed"), ("start_ttl", "ttl_elapsed")] {
            let answer = try CoreJSON.object(await executor.perform(["type": timer, "ms": 0]))
            #expect(answer["type"] as? String == elapsed)
        }
    }

    // MARK: - The core prices the quote on screen again by itself

    /// The clock a test moves: what the relay holds ages on it, and
    /// `FeeStore.elapse` runs the fee machine's timer out on it.
    private final class TestClock {
        var ms: Double = 0
    }

    /// One block on Gnosis — `requote_interval_ms` is 6 s there (the floor of
    /// 6–30 s): the chain's held readings (5 s) are past their window, the
    /// relay's fee rows (8 s) are not, and the fee machine's re-pricing timer
    /// runs out. Waits until whatever that set going has settled.
    private func aBlockPasses(_ clock: TestClock, _ fees: FeeStore) async {
        clock.ms += 6_000
        fees.elapse("start_ttl")
        await Wait.until { fees.isIdle }
    }

    /// The chain twice as dear as `scriptFeeReads` has it: 2 gwei where it says 1.
    private func scriptADearerBlock(_ port: StaggeredRelayPort) {
        port.rpc["eth_gasPrice"] = .ok("0x77359400")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x77359400"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x77359400")
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "slow": ["maxFeePerGas": "0x77359400"] as [String: Any],
            "standard": ["maxFeePerGas": "0xb2d05e00"] as [String: Any],
            "fast": ["maxFeePerGas": "0xee6b2800"] as [String: Any],
        ] as [String: Any])
    }

    private func settledQuote(_ port: StaggeredRelayPort, _ clock: TestClock) async throws -> (FeeStore, FeeEstimateWire) {
        port.baseMs = 0
        port.staggerMs = 0
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { clock.ms }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let view = await fees.quote(
            chainId: 100, account: golden, deployed: true, publicKeyAvailable: true,
            calls: [["to": golden, "value": "1000", "data": "0x"] as [String: Any]],
            feeToken: nil
        )
        let fee = try #require(view?.fee, "\(String(describing: view))")
        await Wait.until { fees.isIdle }
        return (fees, fee)
    }

    /// The fee core prices the quote on screen again by itself, a block on
    /// (`requote_interval_ms`): a dearer block reaches the fee row with
    /// nothing in the shell asking. That is why the send confirm and the
    /// signing sheet no longer re-ask a `stale` quote (spec 045 US4's TTL
    /// re-ask): the core already does, every block.
    @Test func theCorePricesASettledQuoteAgainByItselfABlockLater() async throws {
        let port = StaggeredRelayPort()
        let clock = TestClock()
        let (fees, first) = try await settledQuote(port, clock)
        #expect(first.networkFeePerGas != "0", "\(first)")
        var seen: [FeeViewWire] = []
        fees.onInForce = { seen.append($0) }

        scriptADearerBlock(port)
        await aBlockPasses(clock, fees)

        let next = try #require(fees.view?.fee, "\(String(describing: fees.view))")
        #expect(
            (Decimal(string: next.networkFeePerGas) ?? 0) > (Decimal(string: first.networkFeePerGas) ?? 0),
            "priced at the dearer block: \(first.networkFeePerGas) → \(next.networkFeePerGas)"
        )
        #expect((port.counts["eth_gasPrice"] ?? 0) > 1, "the chain was read again: \(port.counts)")
        #expect(!seen.isEmpty, "the re-pricing never reached the fee row")
        #expect(!seen.contains { $0.stale && !$0.busy }, "stale and idle: \(seen)")
    }

    /// And when that pricing fails, the figure older than a block is priced
    /// again at once — `stale` and `busy` together, so every confirm waits —
    /// and a second failure takes the figure away and says so. A view that is
    /// stale and idle never shows, so a shell's "stale and not busy: ask
    /// again" could never fire.
    @Test func aRepricingThatFailsIsCaughtUpByTheCoreNeverLeftStaleAndIdle() async throws {
        let port = StaggeredRelayPort()
        let clock = TestClock()
        let (fees, _) = try await settledQuote(port, clock)
        var seen: [FeeViewWire] = []
        fees.onInForce = { seen.append($0) }

        // The relay's fee rows go down. The first block re-prices from the
        // rows read with the quote (held 8 s); the next one cannot.
        port.rpc["vela_getInBandGasQuote"] = .failed(rateLimited: false)
        await aBlockPasses(clock, fees)
        #expect(fees.view?.fee != nil && fees.view?.failed == nil, "\(String(describing: fees.view))")
        await aBlockPasses(clock, fees)

        let failed = try #require(fees.view)
        #expect(failed.failed != nil, "the core's failure is said: \(failed)")
        #expect(failed.fee == nil, "the figure older than a block is taken away")
        #expect(seen.contains { $0.stale && $0.busy }, "caught up at once, stale and busy: \(seen)")
        #expect(!seen.contains { $0.stale && !$0.busy }, "stale and idle: \(seen)")
    }

    // MARK: - The fee coin nobody chose

    /// 0 xDAI and 500 USDC: asked with the coin left to the machine, the quote
    /// is in USDC and the view says so — the default just works.
    @Test func anAutomaticQuotePaysInACoinThatCan() async throws {
        let port = StaggeredRelayPort()
        port.baseMs = 0
        port.staggerMs = 0
        scriptFeeReads(port, nativeWei: "0x0", usdcUnits: "0x1dcd6500")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let send = try executor(relay: relay, fees: fees, balances: { nil })
        let answer = try CoreJSON.object(await send.perform([
            "type": "estimate_fee", "chain_id": 100, "account": golden,
            "tx": ["to": golden, "value": "1000", "data": "0x"] as [String: Any],
            "batch": NSNull(), "gas_fee_token": NSNull(),
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "auto_fee_token": true,
        ]))
        let outcome = try #require(answer["outcome"] as? [String: Any])
        #expect(outcome["type"] as? String == "ok", "\(answer)")
        let asset = (outcome["estimate"] as? [String: Any])?["fee_asset"] as? [String: Any]
        #expect(asset?["type"] as? String == "erc20")
        #expect((asset?["token"] as? String)?.lowercased() == usdc.lowercased())
        #expect(fees.view?.feeToken?.lowercased() == usdc.lowercased())
        #expect(fees.view?.options.first { $0.contract?.lowercased() == usdc.lowercased() }?.selected == true)

        // A tap on the native chip is the person's own pick, priced as asked.
        fees.chooseFeeToken(nil)
        await Wait.until({ fees.view?.busy == false && fees.view?.feeToken == nil }, orIdle: { fees.isIdle })
        #expect(fees.view?.feeToken == nil, "the tap is not overridden by the machine's pick")
    }

    // MARK: - Reading ahead

    /// `prewarm_fees` is answered at once, and the reads land in the caches
    /// a quote reads: asked again, nothing goes out.
    @Test func prewarmFillsTheCachesTheQuoteReads() async throws {
        let port = StaggeredRelayPort()
        port.baseMs = 20
        port.staggerMs = 0
        scriptFeeReads(port)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let send = try executor(relay: relay, fees: fees, balances: { nil })
        let answer = try CoreJSON.object(await send.perform([
            "type": "prewarm_fees", "account": golden, "chain_ids": [100],
        ]))
        #expect(answer["type"] as? String == "fees_prewarmed")
        #expect(port.counts.isEmpty, "answered before a single read went out: \(port.counts)")
        let expected = [
            "eth_getCode", "eth_gasPrice", "eth_getBlockByNumber", "eth_maxPriorityFeePerGas",
            "pimlico_getUserOperationGasPrice", "vela_getInBandGasQuote",
        ]
        // Every read made AND landed in its cache: the warm-up itself, awaited
        // — it was a 5 s count of sleeps and then 150 ms "to let the last
        // answers land", which a busy machine does not promise.
        await send.prewarming?.value
        #expect(expected.allSatisfy { port.counts[$0] != nil }, "not every read went out: \(port.counts)")
        let warmed = port.counts
        #expect(warmed["eth_getCode"] == 1)
        #expect(warmed["eth_gasPrice"] == 1)
        #expect(warmed["pimlico_getUserOperationGasPrice"] == 1)
        #expect(warmed["vela_getInBandGasQuote"] == 1)
        _ = await relay.isDeployed(chainId: 100, address: golden)
        _ = await relay.gasSignals(chainId: 100, wantTip: true)
        _ = await relay.bundlerQuote(chainId: 100, tier: "standard")
        _ = await relay.inBandQuotes(chainId: 100, safe: golden)
        #expect(port.counts == warmed, "every read was already held")
    }

    // MARK: - One source for holdings

    /// Unpriced holdings are a SUBSET of `tokens`, not extra rows.
    @Test func theMappingListsEveryHoldingOnce() throws {
        let view = try balance([("xDAI", "0.5", 1.0), ("ODD", "12", nil)])
        let tokens = SendExecutor.sendTokens(view)
        #expect(tokens.count == 2)
        #expect(tokens.compactMap { $0["symbol"] as? String } == ["xDAI", "ODD"])
    }

    /// Each coin carries its logo candidates into the send machine — the
    /// core copies them into the records and the receipt, so a send made on
    /// this phone still wears its coins' logos when any shell opens it. They
    /// were empty.
    @Test func everyHoldingCarriesItsLogoCandidates() {
        let usdc = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"
        let view = BalanceViewWire(
            address: golden,
            displayTotalUsd: 1, balanceUnknown: false, balancePartial: false,
            notice: nil, hidden: false, refreshing: false, lastRefreshedAtMs: nil,
            tokens: [
                BalanceTokenWire(chainId: 100, symbol: "xDAI", name: "xDAI", balance: "1",
                                 decimals: 18, tokenAddress: nil, priceUsd: 1, spam: false),
                BalanceTokenWire(chainId: 100, symbol: "USDC", name: "USD Coin", balance: "1",
                                 decimals: 6, tokenAddress: usdc, priceUsd: 1, spam: false),
            ],
            unpricedTokens: [], failedChainIds: [], rateLimitedChainIds: [],
            holdingsLoading: false, cachedTotalUsd: nil,
            switcher: BalanceSwitcherViewWire(open: false, loading: false, balances: [])
        )
        let urls = SendExecutor.sendTokens(view).map { $0["logo_urls"] as? [String] ?? [] }
        #expect(urls[0] == [Marks.chainLogoURL(100)].compactMap { $0 }, "xDAI wears Gnosis's logo")
        #expect(urls[1] == Marks.token(chainId: 100, symbol: "USDC", tokenAddress: usdc).logoUrls)
        #expect(urls[1].count == 2, "checksummed, then lowercase")
    }

    /// A flow opened before the asset list settled for this account waits for
    /// the first round — streaming what has arrived — instead of "could not
    /// load".
    @Test func fetchTokensWaitsForTheFirstRound() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        var current: BalanceViewWire?
        var settled: Int?
        var opened: [String] = []
        var partials: [[[String: Any]]] = []
        var ports = SendExecutor.Ports()
        ports.tokensPartial = { partials.append($0) }
        let send = try executor(
            relay: relay, fees: fees, balances: { current },
            round: { _ in settled }, open: { opened.append($0) }, ports: ports
        )
        let asked = Task { await send.perform(["type": "fetch_tokens", "address": golden]) }
        // What the executor does, waited for as it happens — not 200 and 400
        // ms of sleep that a loaded runner can outlast (`Waits.swift`).
        await Wait.until { !opened.isEmpty }
        #expect(opened == [golden], "the dashboard is pointed at the account")
        // One chain has answered: shown, not answered.
        current = try balance([("xDAI", "0.5", 1.0)], refreshedAt: nil)
        await Wait.until { !partials.isEmpty }
        #expect(partials.count == 1)
        // The round settles.
        current = try balance([("xDAI", "0.5", 1.0), ("USDC", "3", 1.0)])
        settled = 1
        let answer = try CoreJSON.object(await asked.value)
        #expect(answer["type"] as? String == "tokens_loaded")
        let tokens = try #require(answer["tokens"] as? [[String: Any]], "never null for money on its way")
        #expect(tokens.count == 2)
    }

    /// A load that reached nothing — a launch-time proxy blip — is read ONCE
    /// more before anything is answered, and the next full load answers
    /// whatever it holds.
    @Test func aLoadThatReachedNothingIsReadOnceMore() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        var current = try balance([], failed: [1, 100])
        var round: Int? = 1
        var refreshes = 0
        var ports = SendExecutor.Ports()
        ports.refreshBalances = { refreshes += 1 }
        let send = try executor(
            relay: relay, fees: fees, balances: { current }, round: { _ in round }, ports: ports
        )
        let asked = Task { await send.perform(["type": "fetch_tokens", "address": golden]) }
        await Wait.until { refreshes > 0 }
        #expect(refreshes == 1, "one forced re-read")
        // The re-read is still out: nothing answered, nothing asked again. (A
        // window, not a wait: a busy machine can only make it pass for less.)
        try? await Task.sleep(nanoseconds: 300_000_000)
        #expect(refreshes == 1)
        current = try balance([("xDAI", "0.5", 1.0)], failed: [1])
        round = 2
        let answer = try CoreJSON.object(await asked.value)
        let tokens = try #require(answer["tokens"] as? [[String: Any]], "the next load answers: \(answer)")
        #expect(tokens.count == 1)
        #expect(refreshes == 1)
    }

    /// Only a SECOND load that reached nothing is "could not load".
    @Test func onlyASecondEmptyLoadAnswersNull() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        var current = try balance([], failed: [100])
        var round: Int? = 4
        var refreshes = 0
        var ports = SendExecutor.Ports()
        ports.refreshBalances = { refreshes += 1 }
        let send = try executor(
            relay: relay, fees: fees, balances: { current }, round: { _ in round }, ports: ports
        )
        let asked = Task { await send.perform(["type": "fetch_tokens", "address": golden]) }
        await Wait.until { refreshes > 0 }
        #expect(refreshes == 1)
        current = try balance([], failed: [100])
        round = 5
        let answer = try CoreJSON.object(await asked.value)
        #expect(answer["tokens"] is NSNull)
        #expect(refreshes == 1, "one re-read, never a loop")
    }

    /// The asset list could read nothing at all (no round, unreachable): read
    /// once more, and a next load that still reached nothing is the refusal.
    @Test func anUnreachableDashboardIsReadOnceMoreThenRefused() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        var current = try balance([], refreshedAt: nil, failed: [100], unreachable: true)
        var round: Int?
        var refreshes = 0
        var ports = SendExecutor.Ports()
        ports.refreshBalances = { refreshes += 1 }
        let send = try executor(
            relay: relay, fees: fees, balances: { current }, round: { _ in round }, ports: ports
        )
        let asked = Task { await send.perform(["type": "fetch_tokens", "address": golden]) }
        await Wait.until { refreshes > 0 }
        #expect(refreshes == 1)
        current = try balance([], failed: [100])
        round = 1
        let answer = try CoreJSON.object(await asked.value)
        #expect(answer["tokens"] is NSNull)
    }

    /// An account that holds nothing is an EMPTY list — an answer, not a
    /// failure, and nothing is read again.
    @Test func anEmptyWalletIsAnAnswerNotARetry() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let empty = try balance([])
        var refreshes = 0
        var ports = SendExecutor.Ports()
        ports.refreshBalances = { refreshes += 1 }
        let send = try executor(relay: relay, fees: fees, balances: { empty }, ports: ports)
        let answer = try CoreJSON.object(await send.perform(["type": "fetch_tokens", "address": golden]))
        #expect((answer["tokens"] as? [Any])?.isEmpty == true)
        #expect(refreshes == 0)
    }

    /// A round settled while the journey is open reaches the picker AND the
    /// form's selected balance — the core's `holdings_updated`, through the
    /// iOS wire.
    @Test func aNewRoundReachesThePickerAndTheForm() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let first = try balance([("xDAI", "0.5", 1.0)])
        let send = SendStore(executor: try executor(relay: relay, fees: fees, balances: { first }))
        send.open(accountId: "cred-0", address: golden, name: nil, displayCode: "USD", displayRate: 1, fiatDecimals: 2)
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
        #expect(send.view?.tokens.count == 1)

        send.holdingsUpdated(try balance([("xDAI", "0.5", 1.0), ("USDC", "3", 1.0)]), round: 2)
        #expect(send.view?.tokens.count == 2, "the picker follows")

        send.selectToken(id: "chain-100_native_xDAI")
        await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
        send.holdingsUpdated(try balance([("xDAI", "0.75", 1.0), ("USDC", "3", 1.0)]), round: 3)
        #expect(send.view?.selectedToken?.balance == "0.75", "the form's balance follows")
        // The same round twice is handed over once; another account's never.
        send.holdingsUpdated(try balance([("xDAI", "9", 1.0)]), round: 3)
        #expect(send.view?.selectedToken?.balance == "0.75")
    }

    // MARK: - The fee card's coin, into the send machine

    /// The bridge beside `fee_busy_changed`: the card's coin in force reaches
    /// the send machine (`fee_token_changed`) whenever it differs from what
    /// this journey was last told, only while the fee session prices the
    /// form's own chain — and a fresh journey has been told nothing. The core
    /// names the fee row's coin from it (`SendView.fee_coin`).
    @Test func theFeeCardsCoinIsToldOncePerChangeAndAgainToANewJourney() async throws {
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let held = try balance([("xDAI", "0.5", 1.0), ("USDC", "3", 1.0)])
        let send = SendStore(executor: try executor(relay: relay, fees: fees, balances: { held }))

        func enter() async throws -> String {
            send.open(accountId: "cred-0", address: golden, name: nil, displayCode: "USD", displayRate: 1, fiatDecimals: 2)
            await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
            let usdc = try #require(send.view?.tokens.first { $0.symbol == "USDC" }?.tokenAddress)
            // No chain on the form yet: a word now is about no chain at all.
            send.feeTokenChanged(usdc, pricing: 100)
            send.selectToken(id: "chain-100_native_xDAI")
            await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
            #expect(send.view?.feeCoin?.contract == nil, "nothing was told before the form had a chain")
            return usdc
        }

        let usdc = try await enter()
        let journey = send.journey
        send.feeTokenChanged(usdc, pricing: 100)
        #expect(send.view?.feeCoin == SendFeeCoinWire(symbol: "USDC", contract: usdc, chainId: 100),
                "the card's pick names the row's coin")

        // The person picks the chain's own coin: newer than the card's word.
        send.chooseFeeToken(nil)
        #expect(send.view?.feeCoin?.contract == nil)
        // The card has not spoken again — the same word is not said twice,
        // or it would undo the person's pick.
        send.feeTokenChanged(usdc, pricing: 100)
        #expect(send.view?.feeCoin?.contract == nil, "an unchanged coin is not told again")
        send.feeTokenChanged(nil, pricing: 100)
        #expect(send.view?.feeCoin?.contract == nil)
        // A session still pricing another chain says nothing about this one.
        send.feeTokenChanged(usdc, pricing: 1)
        #expect(send.view?.feeCoin?.contract == nil, "another chain's coin is not told")
        send.feeTokenChanged(usdc, pricing: 100)
        #expect(send.view?.feeCoin?.contract == usdc, "a change is told")

        // A new journey has been told nothing: the same coin is told again.
        send.leave()
        _ = try await enter()
        #expect(send.journey == journey + 1)
        send.feeTokenChanged(usdc, pricing: 100)
        #expect(send.view?.feeCoin?.contract == usdc, "a fresh journey hears the card's coin")
    }

    // MARK: - One token-amount rule

    /// The core's own vectors (`app_send.rs`
    /// `a_max_reads_on_the_balance_lines_ladder`), so the row, the card and the
    /// figure Max writes are one rule, digit for digit.
    @Test func tokenAmountsReadOnTheCoresLadder() {
        let before = Formats.current
        defer { Formats.current = before }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        let vectors: [(String, String)] = [
            ("0.043790209243313861", "0.04379"),
            ("0.0439686", "0.043969"),
            ("1.22456789123456789", "1.2246"),
            ("1234.567", "1234.57"),
            ("2", "2"),
            ("0", "0"),
            ("0.9999996", "1"),
            ("999.99996", "1000"),
            ("0.0000001234", "0.00000012"),
            ("5.000000", "5"),
        ]
        for (exact, shown) in vectors {
            #expect(WalletLive.tokenAmountText(exact) == shown, "\(exact)")
        }
        // Half up where the old six-place cut truncated.
        #expect(WalletLive.tokenAmountText("0.0437909") == "0.043791")
        #expect(WalletLive.trimBalance("0.0437909") == "0.04379")
        // A ceiling typed back has to fit: rounded DOWN.
        #expect(WalletLive.tokenAmountText("0.0409086", rounding: .down) == "0.040908")
        #expect(WalletLive.tokenAmountText("0.9999996", rounding: .down) == "0.999999")
        #expect(WalletLive.tokenAmountText("0.0409086") == "0.040909")
        // What is not a plain decimal passes through rather than gaining digits.
        #expect(WalletLive.tokenAmountText("") == "")
        #expect(WalletLive.tokenAmountText("-1.5") == "-1.5")
        #expect(WalletLive.tokenAmountText("1e-7") == "1e-7")
        // Past uint64, still exact: never a Double.
        #expect(WalletLive.tokenAmountText("123456789012345678901234.567") == "123456789012345678901234.57")
        // The person's decimal mark, never grouping.
        Formats.current = Formats.Current(number: .dotComma, date: .iso, time: .h24)
        #expect(WalletLive.tokenAmountText("12345.678901234") == "12345,68")
        #expect(WalletLive.tokenAmountText("0.0000001234") == "0,00000012")
    }

    /// A ≥ 1 balance: the card and the figure Max writes are the same digits
    /// when the fee is paid in another coin — which the machine now prefers.
    @Test func theCardAndTheMaxAgreeAboveOne() async throws {
        let before = Formats.current
        defer { Formats.current = before }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        let loc = Loc(overrideTag: "en")
        let port = StaggeredRelayPort()
        port.baseMs = 0
        port.staggerMs = 0
        // 12.345678 xDAI held; 500 USDC pays the fee.
        scriptFeeReads(port, nativeWei: "0xab54a8bb155ae000", usdcUnits: "0x1dcd6500")
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let held = try balance([("xDAI", "12.345678", 1.0)])
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let accounts = AccountStore(defaults: defaults)
        await accounts.saveAccount([
            "id": "cred-0", "address": golden, "name": "Parallel One",
            "public_key_hex": "04" + String(repeating: "11", count: 64),
            "created_at_iso": "2026-09-26T00:00:00Z",
        ])
        let pool = RpcPool(store: store, accounts: accounts)
        let accountPort = ScriptedAccounts()
        let nets = try networks()
        let send = SendStore(executor: SendExecutor(
            store: store, relay: relay, pool: pool,
            spine: UserOpSpine(relay: relay, accounts: accountPort, signer: { CountingSigner() }),
            accounts: accountPort, fees: fees,
            identity: RecipientIdentity(store: store, pool: pool, accounts: accounts),
            metadata: TokenMetadata(store: store, pool: pool),
            accountStore: accounts, balances: { held }, networks: { nets },
            ports: SendExecutor.Ports()
        ))
        send.open(accountId: "cred-0", address: golden, name: nil, displayCode: "USD", displayRate: 1, fiatDecimals: 2)
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
        send.selectToken(id: "chain-100_native_xDAI")
        await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
        send.tapMax()
        await Wait.until({ !(send.view?.amount ?? "").isEmpty }, orIdle: { send.isIdle })
        let view = try #require(send.view)
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("sd2 is not a send form")
            return
        }
        let model = SendLive.form(view, fee: fees.view, display: .usd, on: drawn, loc: loc)
        #expect(view.amount == "12.3457", "Max wrote \(view.amount)")
        #expect(model.token?.detail == "Gnosis · Balance 12.3457")
        #expect(view.tokenAmount == "12.345678", "the exact amount stays behind the figure")
        if case .erc20(let token, _, _, _)? = view.fee?.feeAsset {
            #expect(token.lowercased() == usdc.lowercased(), "the fee was paid in the coin not being sent")
        } else {
            Issue.record("the fee was not paid in USDC: \(String(describing: view.fee?.feeAsset))")
        }
    }

    // MARK: - Fiat rounding (round 2, item E)

    /// Every fiat figure rounds to NEAREST, an exact tie going up — the web's
    /// `toFixed(2)`. printf alone broke an exact binary tie to even.
    @Test func fiatRoundsHalfUpLikeToFixed() {
        let before = Formats.current
        defer { Formats.current = before }
        Formats.current = Formats.Current(number: .commaDot, date: .iso, time: .h24)
        func money(_ value: Double) -> String {
            Formats.number(value, minimumFractionDigits: 2, maximumFractionDigits: 2)
        }
        #expect(money(0.125) == "0.13", "an exact tie goes up (printf said 0.12)")
        #expect(money(0.375) == "0.38")
        #expect(money(10.625) == "10.63")
        #expect(money(0.995) == "0.99", "0.995 is 0.99499… in binary — toFixed says 0.99 too")
        #expect(money(1.005) == "1.00", "1.00499… in binary")
        #expect(money(2.675) == "2.67", "2.67499… in binary")
        #expect(money(12.34) == "12.34", "never truncated")
        #expect(money(12.339) == "12.34", "rounded, not cut")
        #expect(money(0.005) == "0.01", "0.005000000000000000104… rounds up")
        #expect(Formats.halfUp(9.995, places: 2) == String(format: "%.2f", 9.995))
        #expect(Formats.halfUp(0.5, places: 0) == "1", "a whole tie goes up")
        #expect(Formats.halfUp(1.5, places: 0) == "2")
        #expect(Formats.halfUp(2.5, places: 0) == "3", "printf says 2")
        #expect(Formats.halfUp(99.5, places: 0) == "100")
    }

    // MARK: - The figure

    /// 46 / 38 / 31 by drawn length — figure + prefix + half the suffix — the
    /// web's and the desktop's count.
    @Test func theFigureStepsDownTheHeroLadder() {
        #expect(AmountRung.of(figure: "0.5", prefix: nil, suffix: "XDAI") == .hero)
        #expect(AmountRung.of(figure: "12345678", prefix: nil, suffix: nil) == .hero)
        #expect(AmountRung.of(figure: "0.043790", prefix: nil, suffix: "ETH") == .compact)
        #expect(AmountRung.of(figure: "1234.56", prefix: "$", suffix: nil) == .hero)
        #expect(AmountRung.of(figure: "123456789", prefix: "$", suffix: nil) == .compact)
        #expect(AmountRung.of(figure: "0.043790209243313861", prefix: nil, suffix: "XDAI") == .tight)
        #expect(Typography.amountEntry.size == 46)
        #expect(Typography.amountEntryCompact.size == 38)
        #expect(Typography.amountEntryTight.size == 31)
    }

    /// The token card says "Balance", and the figure is the asset list's —
    /// never the core's eighteen places.
    @Test func theTokenCardReadsLikeTheAssetList() async throws {
        let loc = Loc(overrideTag: "en")
        let raw = "0.043790209243313861"
        let relay = RelayClient(port: StaggeredRelayPort(), now: { 0 }, retryDelayMs: 0)
        let fees = FeeStore(relay: relay, accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        let held = try balance([("xDAI", raw, 1.0)])
        let send = SendStore(executor: try executor(relay: relay, fees: fees, balances: { held }))
        send.open(accountId: "cred-0", address: golden, name: nil, displayCode: "USD", displayRate: 1, fiatDecimals: 2)
        await Wait.until({ !(send.view?.tokens.isEmpty ?? true) }, orIdle: { send.isIdle })
        send.selectToken(id: "chain-100_native_xDAI")
        await Wait.until({ send.view?.stage == .enterDetails }, orIdle: { send.isIdle })
        let view = try #require(send.view)
        guard case .sendForm(let drawn) = WalletFlowFixtures.build(.sd2, loc: loc).base else {
            Issue.record("sd2 is not a send form")
            return
        }
        let model = SendLive.form(view, fee: nil, display: .usd, on: drawn, loc: loc)
        let detail = try #require(model.token?.detail)
        #expect(detail == "Gnosis · Balance 0.04379", "\(detail)")
        #expect(!detail.contains(raw))
    }
}
