//
//  SendReceiptFollowsTrackerTests.swift
//  VelaWalletTests
//
//  Spec 082 round 2 (T237, T238, RJ1, RJ4), reviewed: the wallet's own Send
//  hands its op to the tracker BEFORE the POST (the write-ahead), so the
//  tracker can reach its verdict while the relay's reply is still out — the
//  chain check finds the landed op while the reply is lost. The receipt is
//  named its op only when that reply (or its loss) comes back; it must then
//  read the verdict the tracker ALREADY has, not wait for a tracker change
//  that never comes. Driven through the real `SendStore`, its real executor
//  and the real `send` core, as `RootView` wires them.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
@Suite(.timeLimit(.minutes(3)))
struct SendReceiptFollowsTrackerTests {

    private let fixture = TrustedSignerFixture()
    private let payee = "0x76875e38fc6bc2dedcaed807ce00782db5c0d141"
    private let feeRecipient = "0x7777777777777777777777777777777777777777"
    private let tx = "0x" + String(repeating: "ef", count: 32)

    private func balance() throws -> BalanceViewWire {
        let json = """
        {
          "address": "\(fixture.account)",
          "display_total_usd": 2.0, "balance_unknown": false, "balance_partial": false,
          "notice": null, "hidden": false, "refreshing": false,
          "last_refreshed_at_ms": null,
          "tokens": [{
            "chain_id": 100, "symbol": "xDAI", "name": "xDAI",
            "balance": "2", "decimals": 18, "token_address": null,
            "price_usd": 1.0, "spam": false
          }],
          "unpriced_tokens": [],
          "failed_chain_ids": [], "rate_limited_chain_ids": [], "unreachable_networks": [],
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


    /// PR 2 integration: the fee machine failed (or is re-asking after a
    /// failure) while the confirm is up. The bridge tells the send machine
    /// (`fee_failed_changed`) — once per change, and again to a new journey —
    /// and the machine holds the confirm and drops the figure it kept, so the
    /// row draws the failure instead of a figure the fee machine discarded,
    /// and the confirm no longer opens between the core's re-asks. A settled
    /// quote (`fee_updated`) clears it.
    @Test func aFailedFeeHoldsTheConfirmAndDropsItsFigure() async throws {
        let scripted = ScriptedRelayPort()
        scripted.rpc["eth_getCode"] = .ok("0x")
        scripted.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        scripted.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        scripted.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        scripted.rpc["eth_maxPriorityFeePerGas"] = .ok("0x1")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x77359400", "maxPriorityFeePerGas": "0x1",
            "networkFeePerGas": "0x3b9aca00", "relayerFeePerGas": "0x3b9aca00",
        ]
        scripted.rpc["pimlico_getUserOperationGasPrice"] = .ok(
            ["slow": tier, "standard": tier, "fast": tier] as [String: Any]
        )
        scripted.rpc["vela_getInBandGasQuote"] = .ok([[
            "recipient": feeRecipient, "asset": "native", "feeToken": NSNull(), "decimals": 18,
            "symbol": "XDAI", "balance": "0xde0b6b3a7640000", "usdPrice": "1", "usdBalance": "1",
        ] as [String: Any]])
        scripted.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])

        let port = WriteAheadProbePort(scripted)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.pageRecordJson
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        let fixture = self.fixture
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        defaults.set("[\(fixture.pageRecordJson)]",
                     forKey: VelaStore.Key.accounts)
        let store = VelaStore(defaults: defaults)
        let accountStore = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accountStore, offline: true)
        let held = try balance()
        let nets = try networks()

        var tracked: [TrackSubmission] = []
        let executor = SendExecutor(
            store: store, relay: relay, pool: pool, spine: spine, accounts: accounts,
            fees: FeeStore(relay: relay, accounts: accounts, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accountStore),
            metadata: TokenMetadata(store: store, pool: pool), accountStore: accountStore,
            balances: { held }, networks: { nets },
            ports: SendExecutor.Ports(trackSubmitted: { tracked.append($0) }),
            clearanceWaitMs: 600_000
        )
        let send = SendStore(executor: executor)
        send.open(
            accountId: fixture.credentialHex, address: fixture.account, name: "Mine",
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        await Wait.until { !(send.view?.tokens.isEmpty ?? true) }
        let token = try #require(send.view?.tokens.first?.id)
        send.selectToken(id: token)
        await Wait.until { send.view?.stage == .enterDetails }
        send.setRecipient(payee)
        send.setAmount("0.001")
        await Wait.until { send.view?.canContinue == true }
        send.advance()
        await Wait.until { send.view?.stage == .confirm && send.view?.canConfirm == true }
        let kept = try #require(send.view?.fee)
        _ = tracked

        send.feeFailedChanged(true)
        #expect(send.view?.canConfirm == false, "the confirm opened over a fee the fee machine discarded")
        #expect(send.view?.fee == nil, "the confirm kept a figure the fee machine discarded")
        // Said once: the same word again changes nothing.
        send.feeFailedChanged(true)
        #expect(send.view?.canConfirm == false)
        // A settled quote stands again: the confirm opens on it.
        send.feeUpdated(kept)
        send.feeFailedChanged(false)
        await Wait.until { send.view?.canConfirm == true }
        #expect(send.view?.canConfirm == true)
        #expect(send.view?.fee != nil)
    }

    /// The Send's receipt reads "may have been sent" only while nobody knows
    /// better. Here the tracker found the landed op while the relay's reply
    /// was being lost: once the lost reply names the op to the receipt, the
    /// receipt says confirmed — at once, with no further tracker change.
    @Test func aVerdictTheTrackerAlreadyHasReachesTheReceipt() async throws {
        let scripted = ScriptedRelayPort()
        scripted.rpc["eth_getCode"] = .ok("0x")
        scripted.rpc["eth_blockNumber"] = .ok("0x2e3b9d9")
        scripted.rpc["eth_gasPrice"] = .ok("0x3b9aca00")
        scripted.rpc["eth_getBlockByNumber"] = .ok(["baseFeePerGas": "0x3b9aca00"] as [String: Any])
        scripted.rpc["eth_maxPriorityFeePerGas"] = .ok("0x1")
        let tier: [String: Any] = [
            "maxFeePerGas": "0x77359400", "maxPriorityFeePerGas": "0x1",
            "networkFeePerGas": "0x3b9aca00", "relayerFeePerGas": "0x3b9aca00",
        ]
        scripted.rpc["pimlico_getUserOperationGasPrice"] = .ok(
            ["slow": tier, "standard": tier, "fast": tier] as [String: Any]
        )
        scripted.rpc["vela_getInBandGasQuote"] = .ok([[
            "recipient": feeRecipient, "asset": "native", "feeToken": NSNull(), "decimals": 18,
            "symbol": "XDAI", "balance": "0xde0b6b3a7640000", "usdPrice": "1", "usdBalance": "1",
        ] as [String: Any]])
        scripted.rpc["eth_estimateUserOperationGas"] = .ok([
            "verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350",
        ] as [String: Any])

        let port = WriteAheadProbePort(scripted)
        let relay = RelayClient(port: port, now: { 0 }, retryDelayMs: 0)
        let accounts = ScriptedAccounts()
        accounts.keyList = fixture.keys
        accounts.recordJson = fixture.pageRecordJson
        let spine = UserOpSpine(relay: relay, accounts: accounts, signer: { CountingSigner() })
        let fixture = self.fixture
        spine.trustedSigner = ScriptedTrustedSigner { digest in
            let data = try! JSONSerialization.data(withJSONObject: fixture.result(for: digest))
            return .outcome(trustedSignerVerify(
                resultJson: String(decoding: data, as: UTF8.self), digest: digest, keys: fixture.keys
            ))
        }
        let defaults = UserDefaults(suiteName: UUID().uuidString)!
        defaults.set("[\(fixture.pageRecordJson)]",
                     forKey: VelaStore.Key.accounts)
        let store = VelaStore(defaults: defaults)
        let accountStore = AccountStore(defaults: defaults)
        let pool = RpcPool(store: store, accounts: accountStore, offline: true)
        let held = try balance()
        let nets = try networks()

        var tracked: [TrackSubmission] = []
        let executor = SendExecutor(
            store: store, relay: relay, pool: pool, spine: spine, accounts: accounts,
            fees: FeeStore(relay: relay, accounts: accounts, settleDeadline: nil, timers: .stopped),
            identity: RecipientIdentity(store: store, pool: pool, accounts: accountStore),
            metadata: TokenMetadata(store: store, pool: pool), accountStore: accountStore,
            balances: { held }, networks: { nets },
            ports: SendExecutor.Ports(trackSubmitted: { tracked.append($0) }),
            clearanceWaitMs: 600_000
        )
        let send = SendStore(executor: executor)

        // The POST's reply is lost, every time: may have been sent. While it
        // is out, the tracker — handed the op before the POST — finds it
        // landed on chain, and tells the Send (as `RootView` wires it).
        let tx = self.tx
        port.onPost = {
            if let op = tracked.first?.userOpHash {
                send.trackerChanged(TrackViewWire(entries: [TrackEntryWire(
                    userOpHash: op, chainId: 100, recordIds: tracked.first?.recordIds ?? [],
                    status: "confirmed", txHash: tx, polling: false,
                    submittedAtMs: 1_757_000_000_000, outcome: "final"
                )]))
            }
            return RpcCallResult(outcome: .failed(rateLimited: false), maybeDelivered: true, heldErrorJson: nil)
        }

        send.open(
            accountId: fixture.credentialHex, address: fixture.account, name: "Mine",
            displayCode: "USD", displayRate: 1, fiatDecimals: 2
        )
        await Wait.until { !(send.view?.tokens.isEmpty ?? true) }
        let token = try #require(send.view?.tokens.first?.id)
        send.selectToken(id: token)
        await Wait.until { send.view?.stage == .enterDetails }
        send.setRecipient(payee)
        send.setAmount("0.001")
        await Wait.until { send.view?.canContinue == true }
        send.advance()
        await Wait.until { send.view?.stage == .confirm && send.view?.canConfirm == true }
        send.slideConfirm()

        // The lost reply names the op: "may have been sent".
        await Wait.until { send.view?.userOpHash != nil }
        let op = try #require(send.view?.userOpHash)
        #expect(op.lowercased() == tracked.first?.userOpHash.lowercased())
        #expect(port.posts >= 1)
        #expect(tracked.first?.maybeSent == true, "handed to the tracker before the POST")

        // The tracker already said confirmed; nothing more will come from
        // it. The receipt must read it now.
        try await Task.sleep(nanoseconds: 500_000_000)
        #expect(send.view?.receipt?.status == "confirmed",
                "the receipt still reads \(send.view?.receipt?.status ?? "nil") over an op the tracker saw land")
        #expect(send.view?.txHash == tx)
    }
}
