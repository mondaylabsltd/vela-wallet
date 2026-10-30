//
//  PerfSeed.swift
//  VelaWallet
//
//  The performance tests' wallet (native parity Phase 4): a read-only account
//  with a cached home and a full history, written by a Debug launch so a
//  RELEASE build — which reads no launch input at all — can be measured on it.
//
//      VELA_PERF_SEED=200  VELA_PERF_ADDRESS=0x…
//
//  writes, for that address:
//
//  - `vela.transactionHistory`: 200 confirmed records (the store's own cap),
//    sends and receives over five networks, five hours apart — a full
//    Activity list;
//  - `vela.holdingsCache` (mainnet): a handful of rows the home opens on;
//  - `vela.balanceCache`: the cached total the hero states before any chain
//    answers.
//
//  The account itself is someone else's to write: the parallel space's door
//  (`VELA_PARALLEL_SPACE=1`, its golden Safe) for the runner, or the key-less
//  `VELA_ACCOUNT` seed (`DevAccountSeed`) for a read-only one — the address
//  falls back to `VELA_ACCOUNT` when `VELA_PERF_ADDRESS` is not given.
//
//  The records REPLACE the stored history: this is a simulator's fixture
//  (simulator-only, like `DevAccountSeed`) and never runs on a phone, whose
//  history it would erase. Excluded from Release with the rest of Dev/
//  (`EXCLUDED_SOURCE_FILE_NAMES`; `check-ios-store-binary.sh` proves it).
//
//  Driven by `app-ios/scripts/perf/run-perf.sh`.
//

#if DEBUG

import Foundation

@MainActor
enum PerfSeed {

    static let seedVar = "VELA_PERF_SEED"
    static let addressVar = "VELA_PERF_ADDRESS"
    static let accountVar = "VELA_ACCOUNT"

    /// Read the launch environment and write what it names. Once, from the
    /// app's init, before the object graph reads a store.
    static func install() {
        #if targetEnvironment(simulator)
        apply(environment: ProcessInfo.processInfo.environment, store: VelaStore())
        #endif
    }

    /// What a test drives: the environment and the shelf handed in. `false`
    /// when nothing was asked for.
    @discardableResult
    static func apply(
        environment: [String: String],
        store: VelaStore,
        now: Date = Date()
    ) -> Bool {
        guard let raw = environment[seedVar], let count = Int(raw), count > 0,
              let address = (environment[addressVar] ?? environment[accountVar])?
                .trimmingCharacters(in: .whitespaces),
              address.hasPrefix("0x"), address.count == 42
        else { return false }
        let records = Self.records(count: min(count, TxRecords.cap), address: address, now: now)
        store.writeList(VelaStore.Key.transactionHistory, records)
        HoldingsCache.save(
            address: address, space: NetworkSpaceKind.mainnet.rawValue, tokens: holdings,
            store: store, nowMs: now.timeIntervalSince1970 * 1000
        )
        var totals = store.readObject(VelaStore.Key.balanceCache)
        totals[address.lowercased()] = ["usd": 12_345.67, "at": now.timeIntervalSince1970 * 1000]
        store.writeObject(VelaStore.Key.balanceCache, totals)
        print("[vela-wallet] \(seedVar) wrote \(records.count) records and a cached home")
        return true
    }

    /// The networks the history spreads over, with each one's coin.
    private static let networks: [(chainId: Int, symbol: String, decimals: Int)] = [
        (1, "ETH", 18), (8453, "USDC", 6), (100, "xDAI", 18), (137, "POL", 18), (42161, "ETH", 18),
    ]

    static func records(count: Int, address: String, now: Date) -> [[String: Any]] {
        let nowSec = now.timeIntervalSince1970.rounded(.down)
        return (0..<count).map { index in
            let network = networks[index % networks.count]
            let receive = index % 3 == 0
            let peer = "0x" + String(format: "%040x", index + 1)
            var record: [String: Any] = [
                "id": "perf-\(index)",
                "userOpHash": receive ? "" : "0x" + String(format: "%064x", index + 1),
                "txHash": "0x" + String(format: "%064x", index + 100_000),
                "from": receive ? peer : address,
                "to": receive ? address : peer,
                "value": "\(index % 17 + 1).\(index % 100)",
                "symbol": network.symbol,
                "decimals": network.decimals,
                "chainId": network.chainId,
                // Five hours apart: forty-odd days of headings.
                "timestamp": nowSec - Double(index) * 5 * 3600,
                "status": "confirmed",
                "type": receive ? "receive" : "send",
                "usd": "$\(index % 50 + 1).00",
                "logoUrls": [String](),
            ]
            // A receive's time is its block's: nothing to re-read.
            if receive { record["timeVerified"] = true }
            return record
        }
    }

    /// The rows the home opens on, as the core writes them (`BalanceToken`).
    static let holdings: [[String: Any]] = [
        token(1, "ETH", "Ether", "1.25", 18, nil, 3200),
        token(8453, "USDC", "USD Coin", "4200.5", 6, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913", 1),
        token(100, "xDAI", "xDAI", "310.2", 18, nil, 1),
        token(137, "POL", "Polygon", "820", 18, nil, 0.45),
        token(42161, "ETH", "Ether", "0.4", 18, nil, 3200),
    ]

    private static func token(
        _ chainId: Int, _ symbol: String, _ name: String, _ balance: String, _ decimals: Int,
        _ contract: String?, _ price: Double
    ) -> [String: Any] {
        [
            "chain_id": chainId, "symbol": symbol, "name": name, "balance": balance,
            "decimals": decimals, "token_address": contract ?? NSNull(), "price_usd": price,
            "spam": false,
        ]
    }
}

#endif
