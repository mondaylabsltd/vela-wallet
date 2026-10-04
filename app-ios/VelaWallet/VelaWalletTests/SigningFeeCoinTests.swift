//
//  SigningFeeCoinTests.swift
//  VelaWalletTests
//
//  Issue #411 (seen on Android 0.9.6; the iOS sheet had the same gap): a
//  Uniswap swap on Polygon in the in-app browser — pUSD −128.51 → USDC
//  +128.4946 — opened with POL as the fee coin. The account held no POL; pUSD
//  (257.02) and USDC (99.99) could both pay ~0.317. The router's swap path
//  names both stablecoins, so until a simulation says what the swap leaves of
//  them the fee machine cannot count on either (spec 096 F2), and the coin
//  asked for — native — stood. The sheet DID simulate the swap, for its
//  balance block, and never told the fee machine.
//
//  Through the real `sign_request`, `sim_outcome` and `fee_policy` cores with
//  a scripted relay and a scripted simulation. Hermetic: the pool is never
//  booted, so the inner-call measurement fails closed at once.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(2)))
struct SigningFeeCoinTests {

    private let safe = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let recipient = "0x2222222222222222222222222222222222222222"
    private let router = "0x1095692A6237d83C6a72F3F5eFEdb9A670C49223"
    private let pool = "0x3333333333333333333333333333333333333333"
    /// Polygon's native USDC, checksummed as the relay writes it.
    private let usdc = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359"
    /// The swap's input coin (synthetic: only its role matters).
    private let pusd = "0x7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a"
    private let transferTopic = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"

    private func bare(_ address: String) -> String { String(address.dropFirst(2)).lowercased() }

    /// A Universal Router `execute` whose V3 path is pUSD → USDC: both coins
    /// named as packed 20-byte words, no `transfer` anywhere.
    private var swapData: String {
        "0x3593564c" + String(repeating: "00", count: 32 * 9) + bare(pusd) + "000064" + bare(usdc)
            + String(repeating: "00", count: 32 * 2 + 9)
    }

    /// A deployed Safe on Polygon at 2185 gwei: the quote is 2.94975 POL,
    /// ≈ $0.317 in a stablecoin — and the three rows the sheet listed.
    private func scriptedRelay() -> RelayClient {
        let port = ScriptedRelayPort()
        port.rpc["eth_getCode"] = .ok("0x6080604052")
        port.rpc["eth_call"] = .ok("0x" + String(repeating: "0", count: 63) + "1")
        port.rpc["eth_gasPrice"] = .ok("0x1fcbc261a00")
        port.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x0"] as [String: Any])
        port.rpc["eth_maxPriorityFeePerGas"] = .ok("0x0")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x3f9784c3400", "networkFeePerGas": "0x1fcbc261a00",
            "relayerFeePerGas": "0x1fcbc261a00",
        ]
        port.rpc["pimlico_getUserOperationGasPrice"] = .ok([
            "fast": tier, "standard": tier, "slow": tier,
        ] as [String: Any])
        port.rpc["vela_getInBandGasQuote"] = .ok([
            ["recipient": recipient, "asset": "native", "feeToken": NSNull(), "balance": "0x0",
             "decimals": 18, "symbol": "POL", "usdBalance": "0", "usdPrice": "0.1075"] as [String: Any],
            ["recipient": recipient, "asset": "erc20", "feeToken": pusd, "balance": "0xf51d060",
             "decimals": 6, "symbol": "pUSD", "usdBalance": "257.02", "usdPrice": "1"] as [String: Any],
            ["recipient": recipient, "asset": "erc20", "feeToken": usdc, "balance": "0x5f5b9f0",
             "decimals": 6, "symbol": "USDC", "usdBalance": "99.99", "usdPrice": "1"] as [String: Any],
        ])
        port.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x186a0", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])
        port.rest["/v1/account/137/\(safe.lowercased())"] = .ok([
            "activeDepositAddress": recipient, "status": "ACTIVE",
        ])
        return RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
    }

    private func transferLog(_ token: String, from: String, to: String, value: Int) -> [String: Any] {
        [
            "address": token,
            "topics": [transferTopic, "0x" + String(repeating: "0", count: 24) + bare(from),
                       "0x" + String(repeating: "0", count: 24) + bare(to)],
            "data": "0x" + String(format: "%064x", value),
        ]
    }

    /// The node's answer for the swap: the coins' own `Transfer` logs.
    private var swapSimulated: RpcOutcome {
        .ok([["calls": [[
            "status": "0x1", "returnData": "0x",
            "logs": [
                transferLog(pusd, from: safe, to: pool, value: 128_510_000),
                transferLog(usdc, from: pool, to: safe, value: 128_494_600),
            ],
        ] as [String: Any]]] as [String: Any]])
    }

    private func controller(simulate: @escaping () -> RpcOutcome) -> SigningController {
        let suite = "vela.tests.signing411.\(UUID().uuidString)"
        let store = VelaStore(defaults: UserDefaults(suiteName: suite)!)
        let relay = scriptedRelay()
        let accounts = ScriptedAccounts()
        return SigningController(
            wallet: (address: safe, credentialId: "cred-1"),
            relay: relay,
            accounts: accounts,
            spine: UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() }),
            store: store,
            // Never booted: a read through it fails closed at once.
            pool: RpcPool(store: store, accounts: AccountStore()),
            ports: SigningController.Ports(
                nativeSymbol: { _ in "POL" },
                knownChains: { [137] },
                simulate: { _, _ in simulate() }
            )
        )
    }

    private var swap: SigningController.Incoming {
        SigningController.Incoming(
            id: "r411", method: "eth_sendTransaction",
            paramsJson: #"[{"from":"\#(safe)","to":"\#(router)","value":"0x0","data":"\#(swapData)"}]"#,
            origin: "https://app.uniswap.org", transportId: "tab-1", chainId: 137
        )
    }

    private func coin(_ view: FeeViewWire, _ contract: String?) -> FeeOptionWire? {
        view.options.first { $0.contract?.lowercased() == contract?.lowercased() }
    }

    /// The simulation answered and the quote settled: nothing left to move it.
    private func settled(_ controller: SigningController) -> Bool {
        guard controller.simulation != .pending, let fee = controller.fee else { return false }
        return !fee.busy && (fee.fee != nil || fee.failed != nil)
    }

    @Test func theSwapTheSheetSimulatedPaysInACoinThatCanNeverThePOLItLacks() async throws {
        let controller = controller { swapSimulated }
        controller.open(swap)
        await Wait.until { settled(controller) }
        #expect(controller.simulation == .answered)
        let fee = try #require(controller.fee)
        #expect(fee.feeToken?.lowercased() == pusd, "the larger of the two coins that pay: \(fee)")
        #expect(fee.confirmFeeReady)
        guard case .erc20(let token, _, _, let symbol)? = fee.fee?.feeAsset else {
            Issue.record("not paid in a stablecoin: \(String(describing: fee.fee))")
            return
        }
        #expect(token.lowercased() == pusd)
        #expect(symbol == "pUSD")
        let pol = try #require(coin(fee, nil))
        #expect(!pol.selected, "never the POL it does not hold")
        #expect(pol.insufficient)
        for contract in [pusd, usdc] {
            let row = try #require(coin(fee, contract))
            #expect(!row.insufficient && row.spentByOperation != true, "\(row)")
        }
    }

    @Test func aSimulationThatCouldNotCheckTellsTheFeeMachineNothing() async throws {
        let controller = controller { .failed(rateLimited: false) }
        controller.open(swap)
        await Wait.until { settled(controller) }
        guard case .notice = controller.simulation else {
            Issue.record("a node that did not answer is a notice: \(controller.simulation)")
            return
        }
        let fee = try #require(controller.fee)
        #expect(fee.feeToken == nil, "no measurement: the fallback stands — POL, short")
        #expect(coin(fee, nil)?.selected == true && coin(fee, nil)?.insufficient == true)
        #expect(!fee.confirmFeeReady)
        #expect(coin(fee, pusd)?.spentByOperation == true && coin(fee, usdc)?.spentByOperation == true)
    }

    // MARK: - The sessions: told after every question about the measured calls

    private var swapCalls: [[String: Any]] { [["to": router, "value": "0", "data": swapData]] }
    private var swapChanges: [[String: Any]] {
        [["token": pusd, "delta": "-128510000"], ["token": usdc.lowercased(), "delta": "128494600"]]
    }

    @Test func aMeasurementBeforeTheQuestionIsToldAfterItAndAfterEveryReAsk() async throws {
        let fees = FeeStore(relay: scriptedRelay(), accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        fees.balanceChanges(calls: swapCalls, changes: swapChanges)
        let first = await fees.quote(
            chainId: 137, account: safe, deployed: true, publicKeyAvailable: true,
            calls: swapCalls, feeToken: nil, autoFeeToken: true
        )
        #expect(first?.feeToken?.lowercased() == pusd, "pUSD from the start: \(String(describing: first))")
        // The same question again (a stale quote's re-ask, an automatic
        // re-quote): the machine forgot, and was told again — answered by
        // THIS question's own settled view, never the last one's.
        let again = await fees.quote(
            chainId: 137, account: safe, deployed: true, publicKeyAvailable: true,
            calls: swapCalls, feeToken: nil, autoFeeToken: true
        )
        #expect(again?.feeToken?.lowercased() == pusd, "still pUSD: \(String(describing: again))")
        #expect(again?.confirmFeeReady == true)
    }

    @Test func aMeasurementOfOtherCallsIsToldToNoSession() async {
        let fees = FeeStore(relay: scriptedRelay(), accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        fees.balanceChanges(calls: [["to": router, "value": "0", "data": swapData + "00"]], changes: swapChanges)
        let view = await fees.quote(
            chainId: 137, account: safe, deployed: true, publicKeyAvailable: true,
            calls: swapCalls, feeToken: nil, autoFeeToken: true
        )
        #expect(view?.feeToken == nil, "not this operation's measurement")
    }

    @Test func theOtherSpeedsAreToldToo() async throws {
        let fees = FeeStore(relay: scriptedRelay(), accounts: ScriptedAccounts(), settleDeadline: nil, timers: .stopped)
        _ = await fees.quote(
            chainId: 137, account: safe, deployed: true, publicKeyAvailable: true,
            calls: swapCalls, feeToken: nil, autoFeeToken: true
        )
        #expect(fees.view?.feeToken == nil, "unmeasured: today's fallback")
        fees.toggleSpeed()
        await Wait.until({ !(fees.speed?.previews.isEmpty ?? true) }, orIdle: { fees.isIdle })
        let tiers = try #require(fees.speed?.previews)
        fees.balanceChanges(calls: swapCalls, changes: swapChanges)
        func paid(_ tier: String) -> Bool {
            guard let view = fees.view(of: tier) else { return false }
            return !view.busy && view.feeToken?.lowercased() == pusd
        }
        await Wait.until({ tiers.allSatisfy(paid) && fees.view?.feeToken?.lowercased() == pusd },
                         orIdle: { fees.isIdle })
        #expect(fees.view?.feeToken?.lowercased() == pusd)
        for tier in tiers {
            #expect(paid(tier), "\(tier) is priced in pUSD too")
        }
    }

    // MARK: - The mapping (the desktop's `fee_balance_changes`)

    @Test func theDeltasMapAsTheDesktopsDo() throws {
        let changes = try #require(SimDeltas.feeBalanceChanges([
            ["kind": "native", "token": NSNull(), "delta": "-1000"],
            ["kind": "erc20", "token": pusd, "delta": "-128510000"],
            // A token move with no contract names no coin: dropped, never native.
            ["kind": "erc20", "token": NSNull(), "delta": "5"],
        ]))
        #expect(changes.count == 2)
        #expect(changes[0]["token"] is NSNull)
        #expect(changes[0]["delta"] as? String == "-1000")
        #expect(changes[1]["token"] as? String == pusd)
        // Moves this build cannot read are no measurement at all.
        #expect(SimDeltas.feeBalanceChanges([["kind": "a_kind_with_no_name", "token": NSNull(), "delta": "1"]]) == nil)
    }
}
