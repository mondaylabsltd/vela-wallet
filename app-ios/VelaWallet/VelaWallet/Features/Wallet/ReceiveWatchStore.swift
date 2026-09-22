//
//  ReceiveWatchStore.swift
//  VelaWallet
//
//  Watching for the money while the code is on screen.
//
//  Ported from `app-web/vela-wallet/src/lib/wallet/core/receive-watch-*.ts`.
//  Three operations and no rules: fetch the balances, wait, buzz. The cadence
//  (3s for the first minute, then 60s, stop at five), the baseline and — the
//  one that matters — **when a difference counts as a deposit** are the core's.
//
//  ## The rule this machine exists for
//
//  A recipient standing at a counter judges "the money arrived" by this screen,
//  so a fetch that comes back with FEWER tokens than the baseline is not
//  compared at all: an RPC probably failed, and diffing against a shrunken set
//  would invent a deposit that never happened. That decision is in Rust, and
//  the only way the shell can break it is by lying about what it fetched — so
//  a failed fetch is reported as failed, never as an empty wallet.
//
//  ## What it can and cannot show
//
//  The buzz is real and so is the balance refresh. The drawn receive screens
//  have **no deposit surface** — `ReceiveWatchView.deposits` has nowhere to go —
//  which is recorded in results.md rather than papered over with a banner
//  nobody drew.
//

import Foundation
import Observation
import UIKit
import VelaCore

extension ReceiveWatchCore: CoreBridge {}

@MainActor
@Observable
final class ReceiveWatchStore {

    /// Every operation this store is required to handle.
    static let operations = ["fetch_tokens", "wait", "signal_deposit"]

    private(set) var view: ReceiveWatchViewWire?

    private let store: VelaStore
    private let pool: RpcPool
    private let held: HeldTokens
    private let prices: Prices
    /// What a detected deposit does besides buzz: the balances are re-read, so
    /// the figure behind the code is the new one by the time somebody looks.
    private let onDeposit: () -> Void
    private var core: CoreStore<ReceiveWatchViewWire>!
    private var address = ""

    init(
        store: VelaStore,
        pool: RpcPool,
        held: HeldTokens,
        onDeposit: @escaping () -> Void = {}
    ) {
        self.store = store
        self.pool = pool
        self.held = held
        self.prices = Prices(pool: pool)
        self.onDeposit = onDeposit
        self.core = CoreStore(
            bridge: ReceiveWatchCore(),
            perform: { [weak self] operation in
                await self?.perform(operation) ?? CoreJSON.string(["type": "inactive"])
            },
            onView: { [weak self] view in self?.view = view },
            onFault: { print("[vela-wallet] receive_watch fault: \($0)") }
        )
    }

    /// The receive screen opened. Single-shot by design: one session per
    /// account, so a previous account's baseline can never bleed into a new
    /// one.
    func open(address: String) {
        guard !address.isEmpty else { return }
        watch(address: address)
        core.boot(CoreJSON.string(["type": "start"]))
    }

    /// The address this session watches, without starting it — the seam the
    /// failure test drives `perform` through.
    func watch(address: String) {
        self.address = address
    }

    // MARK: - The three operations

    /// Internal rather than private so the failure contract can be tested:
    /// "every chain failed" must answer `fetch_failed`, and the difference
    /// between that and an empty wallet is what stops a deposit being invented.
    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "fetch_tokens":
            // The shell checks app activity FIRST, as web does: a tick that
            // fires while the app is in somebody's pocket answers `inactive`
            // and the core stops rather than sweeping twelve chains for a
            // screen nobody is looking at.
            guard UIApplication.shared.applicationState == .active else {
                return CoreJSON.string(["type": "inactive"])
            }
            return await fetch()

        case "wait":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string([
                "type": "waited", "now_ms": Date().timeIntervalSince1970 * 1000,
            ])

        case "signal_deposit":
            VelaHaptic.success.play()
            onDeposit()
            return CoreJSON.string(["type": "signalled"])

        default:
            // See `ContactsExecutor`: logged, not trapped. `inactive` stops the
            // watcher cleanly rather than leaving it waiting on an answer that
            // will never come.
            print("[vela-wallet] receive_watch: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "inactive"])
        }
    }

    /// Every chain's holdings, in the snapshot shape the core diffs.
    ///
    /// A failed sweep is `fetch_failed`, never an empty token list: the core
    /// treats a shrunken set as "an RPC broke" and refuses to diff it, and that
    /// protection only works if the shell reports the failure honestly.
    private func fetch() async -> String {
        let chainlink = await prices.mainnetPrices()
        let results = await withTaskGroup(of: TokenReads.ChainResult.self) { group in
            for chainId in chainIds() {
                let tokens = customTokens(chainId: chainId)
                group.addTask { [pool, address] in
                    await TokenReads.read(address: address, chainId: chainId,
                                          tokens: tokens, pool: pool,
                                          chainlinkPrices: chainlink)
                }
            }
            var collected: [TokenReads.ChainResult] = []
            for await result in group { collected.append(result) }
            return collected
        }

        let now = Date().timeIntervalSince1970 * 1000
        let tokens = results.flatMap(\.tokens)

        // Nothing came back AND something broke ⇒ say it broke.
        //
        // Not `allSatisfy(failed)`: a chain with nothing to ask — Tempo has no
        // native coin, and a chain with no custom tokens and no price feed
        // sends no request at all — reports success without having vouched for
        // anything, and one of those in the list would turn a total blackout
        // into "your wallet is empty". The core reads an empty snapshot as a
        // baseline, and the next successful fetch would then look like a
        // deposit of everything somebody owns.
        //
        // A PARTIAL failure is still reported as success, as web does: the core
        // refuses to diff a set smaller than its baseline, which is the same
        // protection one layer up.
        guard !(tokens.isEmpty && results.contains(where: \.failed)) else {
            return CoreJSON.string(["type": "fetch_failed", "now_ms": now])
        }
        held.record(address: address, tokens: tokens)
        return CoreJSON.string([
            "type": "tokens_fetched",
            "now_ms": now,
            "tokens": tokens.compactMap(Self.snapshot),
        ])
    }

    /// One holding as the core's baseline key + balance.
    ///
    /// The balance crosses as a **number** here, unlike everywhere else in this
    /// cut — `receive_watch.rs` says so, because its detection threshold is
    /// bit-identical to the TypeScript's `tokenBalanceDouble`. A deposit is
    /// noticed by comparing two of these, so both sides must round the same
    /// way.
    static func snapshot(_ token: [String: Any]) -> [String: Any]? {
        guard let chainId = (token["chain_id"] as? NSNumber)?.intValue,
              let symbol = token["symbol"] as? String,
              let balance = Double(token["balance"] as? String ?? "")
        else { return nil }
        let contract = (token["token_address"] as? String)?.lowercased() ?? "native"
        return [
            "id": "\(chainId):\(contract)",
            "symbol": symbol,
            "chain_id": chainId,
            "balance": balance,
            "price_usd": (token["price_usd"] as? NSNumber).map { $0.doubleValue as Any }
                ?? NSNull(),
        ]
    }

    private func chainIds() -> [Int] {
        var ids = ChainCatalog.chains.map(\.chainId)
        for network in store.readList(VelaStore.Key.customNetworks) {
            guard let chainId = (network["chainId"] as? NSNumber)?.intValue,
                  !ids.contains(chainId)
            else { continue }
            ids.append(chainId)
        }
        return ids
    }

    private func customTokens(chainId: Int) -> [CustomTokenRef] {
        CustomTokens.load(store: store).compactMap { stored in
            guard (stored["chainId"] as? NSNumber)?.intValue == chainId,
                  let address = stored["contractAddress"] as? String, !address.isEmpty
            else { return nil }
            return CustomTokenRef(
                address: address,
                symbol: stored["symbol"] as? String ?? "",
                name: stored["name"] as? String ?? "",
                decimals: (stored["decimals"] as? NSNumber)?.intValue ?? 18,
                chainId: chainId
            )
        }
    }
}

/// One deposit the watcher noticed.
struct DepositItemWire: Decodable, Equatable {
    let symbol: String
    /// The raw balance delta — the shell formats it.
    let amount: Double
    let chainId: Int
    /// `delta × price` when the token is priced.
    let usd: Double?
}

struct DepositEntryWire: Decodable, Equatable {
    let atEpochMs: Double
    let items: [DepositItemWire]
}

struct ReceiveWatchViewWire: Decodable, Equatable {
    let detected: Bool
    /// Newest first.
    let deposits: [DepositEntryWire]
}
