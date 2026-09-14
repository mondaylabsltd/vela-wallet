//
//  RelayLiveTests.swift
//  VelaWalletTests
//
//  What the real relay answers, behind `-DVELA_LIVE_TESTS`.
//
//  These read; they never submit. They exist because the fee quote settled in
//  20 ms against a scripted relay and did not settle at all on the founder's
//  iPhone — and the only way to tell a wiring defect from an answer this client
//  does not understand is to ask the real one and print what comes back.
//

#if VELA_LIVE_TESTS

import Foundation
import Testing
@testable import VelaWallet

@MainActor
struct RelayLiveTests {
    private let golden = "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
    private let gnosis = 100

    private func live() -> (RelayClient, RpcPool) {
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        let store = VelaStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: AccountStore(defaults: defaults))
        pool.boot()
        return (RelayClient(port: PoolRelayPort(pool: pool)), pool)
    }

    /// The relay's fee assets for the golden Safe on Gnosis.
    @Test func theRelayQuotesFeeAssets() async {
        let (relay, _) = live()
        let base = await relay.bundlerBaseForTest(chainId: gnosis)
        print("[live] bundler base: \(base ?? "nil")")
        let quotes = await relay.inBandQuotes(chainId: gnosis, safe: golden)
        print("[live] in-band quotes: \(quotes?.count ?? -1)")
        for quote in quotes ?? [] {
            print("[live]   \(quote["symbol"] ?? "?") asset=\(quote["asset"] ?? "?") " +
                  "balance=\(quote["balance"] ?? "?") price=\(quote["usd_price"] ?? "nil")")
        }
        // Not an assertion about the answer — an assertion that there IS one.
        // A relay that cannot be asked is the thing being distinguished here.
        #expect(quotes != nil, "the relay answered nothing usable for the golden Safe")
    }

    /// The gas signals the core prices with.
    @Test func theChainAnswersItsGasSignals() async {
        let (relay, _) = live()
        let signals = await relay.gasSignals(chainId: gnosis, wantTip: true)
        print("[live] gasPrice=\(signals.ethGasPrice ?? "nil") " +
              "baseFee=\(signals.baseFee ?? "nil") tip=\(signals.priorityFee ?? "nil")")
        #expect(signals.ethGasPrice != nil, "eth_gasPrice is the one signal with no fallback")
    }

    /// The bundler's own tier quote.
    @Test func theBundlerQuotesAGasPrice() async {
        let (relay, _) = live()
        let quote = await relay.bundlerQuote(chainId: gnosis, tier: "fast")
        print("[live] bundler fast quote: \(quote ?? [:])")
    }

    /// The treasury, which gates whether a send can be fronted at all.
    @Test func theTreasuryAnswers() async {
        let (relay, _) = live()
        switch await relay.probeTreasury(chainId: gnosis) {
        case .covered: print("[live] treasury: covered")
        case .uncovered: print("[live] treasury: UNCOVERED — this chain has no sponsorship")
        case .unknown: print("[live] treasury: unknown — the relay could not be asked")
        case .lowFloat(let status): print("[live] treasury: low float \(status)")
        }
    }

    /// Whether the golden Safe is deployed, and its nonce.
    @Test func theAccountAnswers() async {
        let (relay, _) = live()
        let deployed = await relay.isDeployed(chainId: gnosis, address: golden)
        print("[live] deployed: \(deployed.map(String.init) ?? "nil")")
        if deployed == true {
            print("[live] nonce: \(await relay.nonce(chainId: gnosis, sender: golden) ?? "nil")")
        }
    }
    /// Poll one operation this device actually submitted.
    ///
    /// `VELA_USER_OP=0x…` names it. This is how a send is proven to have
    /// LANDED rather than merely been accepted — the relay accepting an
    /// operation and the chain including it are different facts, and a receipt
    /// screen that says "submitted" is only reporting the first.
    @Test func aSubmittedOperationReachesAReceipt() async {
        guard let hash = ProcessInfo.processInfo.environment["VELA_USER_OP"], !hash.isEmpty else {
            print("[live] set VELA_USER_OP to poll a specific operation")
            return
        }
        let (relay, _) = live()
        switch await relay.userOpReceipt(chainId: gnosis, userOpHash: hash) {
        case .resolved(let confirmed, let txHash, let sender, let logs):
            print("[live] receipt: confirmed=\(confirmed) tx=\(txHash) " +
                  "sender=\(sender ?? "nil") logs=\(logs.count)")
            #expect(confirmed, "the operation landed but reverted")
        case .pending:
            print("[live] receipt: still pending")
        case .unreachable:
            print("[live] receipt: the relay could not be asked")
        }
    }

}

#endif
