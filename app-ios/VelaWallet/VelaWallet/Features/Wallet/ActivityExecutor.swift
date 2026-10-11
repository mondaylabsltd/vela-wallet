//
//  ActivityExecutor.swift
//  VelaWallet
//
//  The only place the `activity_feed` core touches the outside world.
//
//  Ported from `app-web/vela-wallet/src/lib/wallet/core/feed-executor.ts` and
//  the write half of `services/activity.ts`. Eight operations, one service call
//  each; the dedupe, the batch fold, the tombstone filter, the celebration gate
//  and every "when to read again" decision live in Rust.
//
//  ## The scan is where a wallet learns it was paid
//
//  `ScanIncomingTransfers` is the whole receipt pipeline: ask `token_trust` for
//  the judged incoming feed, turn each judged transfer into a stored `receive`
//  record, merge, and answer **how many were new**. That count is what the core
//  celebrates on, which is why a repeat of the same window must answer zero and
//  a failed scan answers zero too — a failed scan is a scan that found nothing,
//  never a feed that flickers.
//
//  ## The one judgement this file owns
//
//  The **ingest valuation**: a real price when the last balance read had one,
//  else ≈$1 for a stablecoin, else unknown. It is the shell's on purpose (web's
//  `activity.ts` says so), and the core re-derives from the stored string on
//  read with the same table — `STABLE_SYMBOLS`, mirrored in
//  `activity_feed.rs`, so the two sides cannot disagree.
//
//  ## A receipt's time is its block's time (PR 3)
//
//  Three receipts of 2026-09-29 stood under "Today" on 2026-10-10: the scan
//  could not read their block, the core stamped them with the clock this
//  shell handed it, and this file stored that for good. The scan no longer
//  does it (`token_trust` invariant ⑨), so every `receive` written here is
//  marked `timeVerified` — its time is a block's by construction. The records
//  stored before carry no mark, and the core repairs them through two
//  operations this file only carries out: `read_receive_time` (the receipt,
//  then its block, through the pool the scan reads through — `null` when
//  either gave no answer, never a clock's time, never a second try) and
//  `write_receive_time` (that one record's time and its mark, nothing else).
//  Which records, when, and when to ask again are the core's.
//

import Foundation
import VelaCore

@MainActor
final class ActivityExecutor {

    /// Every operation this executor is required to handle.
    static let operations = [
        "read_tx_store",
        "scan_incoming_transfers",
        "delete_tx_record",
        "resolve_recipient_identity",
        "timer",
        "haptic",
        "read_receive_time",
        "write_receive_time",
    ]

    /// One JSON-RPC read through the app's pool. `@MainActor`: the executor
    /// calls it there, and an unannotated async function type is a call to
    /// NULL on iOS 17 (087 F33).
    typealias ChainRead = @MainActor (_ chainId: Int, _ method: String, _ params: [Any]) async -> RpcOutcome

    /// The chains a brand-new wallet is watched on, so a first receipt is
    /// still caught before it holds anything. Mirrored from
    /// `token_trust.rs`'s `DEFAULT_MONITOR_CHAINS`.
    static let defaultMonitorChains = [1, 56, 137, 42_161, 8_453, 100]

    /// Symbols treated as ≈ $1 so a stablecoin receipt is never stored as
    /// "$0.00". Copied verbatim from `activity_feed.rs`'s `STABLE_SYMBOLS`.
    private static let stableSymbols: Set<String> = [
        "USDT", "USDT0", "USDC", "USDC.E", "DAI", "BUSD", "TUSD", "FDUSD",
        "USDE", "PYUSD", "USDP", "GUSD", "LUSD", "FRAX", "USDD",
    ]

    private let store: VelaStore
    private let accounts: AccountStore
    private let held: HeldTokens
    private let trust: TokenTrustStore
    /// The same waterfall `contacts::resolve_identity` uses. One resolver, so a
    /// counterparty cannot be named one thing in the feed and another in the
    /// address book — and so the 24-hour cache is shared rather than doubled.
    private let identity: RecipientIdentity?
    /// The chain, for a receipt's block time: the pool `token_trust` scans
    /// through (`TokenTrustStore.chainRead`), unless a test hands its own.
    private let read: ChainRead
    /// One scan per account at a time. The shell issues this from three places
    /// that can overlap — the account hand-off, the focus tick and the 10s
    /// poll — and a follower answering the leader's count would make the core
    /// believe two batches landed and celebrate a backlog it already spent.
    private var scanning: Set<String> = []

    init(
        store: VelaStore,
        accounts: AccountStore,
        held: HeldTokens,
        trust: TokenTrustStore,
        identity: RecipientIdentity? = nil,
        read: ChainRead? = nil
    ) {
        self.store = store
        self.accounts = accounts
        self.held = held
        self.trust = trust
        self.identity = identity
        self.read = read ?? { [trust] chainId, method, params in
            await trust.chainRead(chainId: chainId, method: method, params: params)
        }
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        case "read_tx_store":
            // The WHOLE store, unfiltered: the core owns the account filter,
            // and needs the un-owned rows to fold a batch's siblings.
            return CoreJSON.string([
                "type": "store_loaded",
                "records": TxRecords.load(store: store).compactMap(TxRecords.toWire),
                "now_ms": Date().timeIntervalSince1970 * 1000,
                // Echoed so the core knows WHICH read this answers. A tick
                // issues the read and the scan together; without the echo a
                // stale read consumes the celebration the sync earned, and a
                // real receipt lands with no toast, glow or haptic.
                "read_id": (operation["read_id"] as? NSNumber)?.intValue ?? 0,
            ])

        case "scan_incoming_transfers":
            let address = operation["address"] as? String ?? ""
            return CoreJSON.string([
                "type": "sync_completed", "new_count": await scan(address: address),
            ])

        case "delete_tx_record":
            let id = operation["id"] as? String ?? ""
            TxRecords.delete(id: id, store: store)
            return CoreJSON.string(["type": "delete_committed", "id": id])

        case "resolve_recipient_identity":
            let addr = operation["addr"] as? String ?? ""
            return CoreJSON.string([
                "type": "alias_resolved", "addr": addr,
                "name": await ownAccountName(addr).map { $0 as Any } ?? NSNull(),
            ])

        case "timer":
            let ms = (operation["ms"] as? NSNumber)?.doubleValue ?? 0
            try? await Task.sleep(nanoseconds: UInt64(max(0, ms) * 1_000_000))
            return CoreJSON.string([
                "type": "toast_expired",
                "generation": (operation["generation"] as? NSNumber)?.intValue ?? 0,
            ])

        case "haptic":
            // Money in. The one buzz this screen has, and it fires even when
            // the balance is hidden — the core withholds the toast there, not
            // the feeling that something arrived.
            VelaHaptic.success.play()
            return CoreJSON.string(["type": "haptic_played"])

        case "read_receive_time":
            let id = operation["id"] as? String ?? ""
            let seconds = await blockTime(
                chainId: (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
                txHash: operation["tx_hash"] as? String ?? ""
            )
            return CoreJSON.string([
                "type": "receive_time_read", "id": id,
                // The block's own seconds, or `null`: not read. Never a
                // clock's time — the record then keeps the one it has, and
                // the core asks again when it chooses to.
                "timestamp_sec": seconds.map { $0 as Any } ?? NSNull(),
            ])

        case "write_receive_time":
            let id = operation["id"] as? String ?? ""
            let written = (operation["timestamp_sec"] as? NSNumber).map {
                TxRecords.writeReceiveTime(id: id, timestampSec: $0.doubleValue, store: store)
            } ?? false
            return CoreJSON.string(["type": "receive_time_written", "id": id, "ok": written])

        default:
            // See `ContactsExecutor`: logged, not trapped. An empty store load
            // leaves the core able to read again rather than stalled holding
            // the feed.
            print("[vela-wallet] activity_feed: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string([
                "type": "store_loaded", "records": [],
                "now_ms": Date().timeIntervalSince1970 * 1000, "read_id": 0,
            ])
        }
    }

    // MARK: - Receipt discovery

    /// Discover and persist new receipts. Answers how many landed.
    private func scan(address: String) async -> Int {
        guard !address.isEmpty, !scanning.contains(address.lowercased()) else { return 0 }
        scanning.insert(address.lowercased())
        defer { scanning.remove(address.lowercased()) }

        // Only the chains the wallet actually uses — twelve chains for an
        // account that holds one is twenty-four `eth_getLogs` nobody asked for.
        let active = held.chainIds(address: address)
        let chainIds = active.isEmpty ? Self.defaultMonitorChains : active

        let incoming = await trust.pollIncoming(address: address, chainIds: chainIds)
        return ingest(incoming, address: address)
    }

    /// The judged feed, stored: each transfer as a `receive` record, merged.
    /// Answers how many were new.
    func ingest(_ incoming: [TrustIncomingWire], address: String) -> Int {
        let records = incoming.compactMap { record($0, address: address) }
        guard !records.isEmpty else { return 0 }
        return TxRecords.merge(records, store: store)
    }

    // MARK: - A receipt's block time

    /// The time of the block that holds `txHash`, as the chain gives it: the
    /// receipt for its block, that block for its `timestamp`. `nil` whenever
    /// either read gave no usable answer — no receipt, an RPC error, a hex
    /// nobody can read. One pass and no retry: when to ask again is the
    /// core's, and "now" is never an answer.
    private func blockTime(chainId: Int, txHash: String) async -> Double? {
        guard chainId > 0, !txHash.isEmpty,
              case .ok(let found) = await read(chainId, "eth_getTransactionReceipt", [txHash]),
              let receipt = found as? [String: Any],
              let block = receipt["blockNumber"] as? String,
              TokenTrustExecutor.hexToNumber(block) != nil,
              case .ok(let held) = await read(chainId, "eth_getBlockByNumber", [block, false]),
              let header = held as? [String: Any],
              let hex = header["timestamp"] as? String,
              let seconds = TokenTrustExecutor.hexToNumber(hex), seconds > 0
        else { return nil }
        return seconds
    }

    /// One judged transfer as a persistable `receive` record.
    ///
    /// A non-native transfer whose symbol the core could not resolve is
    /// **dropped**: `token_trust` withholds those already (invariant ③), and
    /// the fallback that would let one through — 18 decimals and the word
    /// "tokens" — is exactly what records a 6-decimals stablecoin as "+0".
    private func record(_ transfer: TrustIncomingWire, address: String) -> [String: Any]? {
        let meta = ChainCatalog.meta(transfer.chainId)
        guard let symbol = transfer.symbol ?? (transfer.isNative ? meta?.nativeSymbol : nil),
              !symbol.isEmpty
        else { return nil }
        let decimals = transfer.decimals ?? 18
        guard let amount = TokenReads.scaled(decimal: transfer.value, decimals: decimals)
        else { return nil }

        var stored: [String: Any] = [
            "id": transfer.id,
            "userOpHash": "",
            "txHash": transfer.txHash,
            "from": transfer.from,
            "to": address,
            "value": amount,
            "symbol": symbol,
            "decimals": decimals,
            "chainId": transfer.chainId,
            "timestamp": transfer.timestampSec,
            // The feed's time is the transfer's own block's (`token_trust`
            // invariant ⑨), so this record's needs no checking again.
            "timeVerified": true,
            "status": "confirmed",
            "type": "receive",
            "usd": usd(amount: amount, symbol: symbol, transfer: transfer, address: address),
        ]
        let logos = logoURLs(transfer, symbol: symbol)
        if !logos.isEmpty { stored["logoUrls"] = logos }
        return stored
    }

    /// The ingest valuation — the shell's one judgement here.
    private func usd(
        amount: String, symbol: String, transfer: TrustIncomingWire, address: String
    ) -> String {
        let value = Double(amount) ?? 0
        if let price = held.holding(address: address, chainId: transfer.chainId,
                                    token: transfer.isNative ? nil : transfer.token)?.priceUsd {
            return Self.formatUsd(value * price)
        }
        if Self.isStable(symbol) { return Self.formatUsd(value) }
        // Not "$0.00 because it is worthless" — "$0.00 because nobody could
        // price it", which is the string every client has always stored and
        // which the core re-reads as an unknown rather than a zero.
        return Self.formatUsd(0)
    }

    /// Logo candidates, captured now while the contract address is in hand.
    ///
    /// The URLs are the same ones web writes, so a record synced between
    /// clients keeps its artwork. Nothing here fetches them.
    private func logoURLs(_ transfer: TrustIncomingWire, symbol: String) -> [String] {
        let base = NetDefaults.ethereumDataURL
        if transfer.isNative {
            return ["\(base)/chainlogos/eip155-\(transfer.chainId).png"]
        }
        guard let token = transfer.token, !token.isEmpty else { return [] }
        let lowercased = token.lowercased()
        guard let checksummed = try? checksumAddress(addressHex: token) else {
            return ["\(base)/assets/eip155-\(transfer.chainId)/\(lowercased)/logo.png"]
        }
        var urls = ["\(base)/assets/eip155-\(transfer.chainId)/\(checksummed)/logo.png"]
        if lowercased != checksummed {
            urls.append("\(base)/assets/eip155-\(transfer.chainId)/\(lowercased)/logo.png")
        }
        return urls
    }

    // MARK: - Naming

    /// The person's OWN accounts first, then the shared waterfall.
    ///
    /// Own accounts first is not an optimisation: a name you gave your own
    /// wallet beats anything a registry says about it, and answering it costs
    /// no network at all. Only then the passkey index and the name services —
    /// the same resolver the address book uses, so one address has one name.
    private func ownAccountName(_ addr: String) async -> String? {
        let wanted = addr.lowercased()
        for account in await accounts.loadAccounts() {
            guard let address = account["address"] as? String,
                  address.lowercased() == wanted,
                  let name = account["name"] as? String, !name.isEmpty
            else { continue }
            return name
        }
        return await identity?.resolve(addr)?.name
    }

    // MARK: - Formatting the stored string

    /// `"$1,234.56"` — the `en-US` grouping every client has stored, so the
    /// bytes match whatever wrote the record.
    static func formatUsd(_ value: Double) -> String {
        guard value.isFinite, value > 0 else { return "$0.00" }
        let formatter = NumberFormatter()
        formatter.locale = Locale(identifier: "en_US")
        formatter.numberStyle = .decimal
        formatter.minimumFractionDigits = 2
        formatter.maximumFractionDigits = 2
        return "$" + (formatter.string(from: NSNumber(value: value))
            ?? String(format: "%.2f", value))
    }

    /// Upper-case and fold the Tether glyph `₮` to `T`, so `USD₮0` matches
    /// `USDT0`.
    static func isStable(_ symbol: String) -> Bool {
        stableSymbols.contains(symbol.uppercased().replacingOccurrences(of: "₮", with: "T"))
    }
}
