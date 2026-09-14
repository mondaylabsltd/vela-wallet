//
//  PaymentRequestStore.swift
//  VelaWallet
//
//  The `payment_request` machine, for the half this cut needs: a `/pay` link's
//  verdict.
//
//  **The validation is the CORE's.** A link carries seven strings and whether
//  they add up to a request this wallet can honour — a real address, a chain it
//  has, decimals that make sense of the amount — is 600 lines of decisions that
//  already exist. A shell that decided here would be a second opinion about
//  somebody's money.
//
//  058 adds the ASSET half: which coin the code is for. The mode toggle and
//  the amount field are still drawn nowhere — on EITHER phone, as it turns out
//  (`WalletController.receiveMode` and `receiveAmount` have no callers on
//  Android either), so that stays recorded rather than invented.
//

import Foundation
import Observation
import VelaCore

extension PaymentRequestCore: CoreBridge {}

@MainActor
@Observable
final class PaymentRequestStore {

    private(set) var view: PaymentRequestViewWire?

    private var core: CoreStore<PaymentRequestViewWire>!

    init(executor: PaymentRequestExecutor) {
        self.core = CoreStore(
            bridge: PaymentRequestCore(),
            perform: { operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view },
            onFault: { print("[vela-wallet] payment_request fault: \($0)") }
        )
    }

    /// A link opened. The machine has no boot event of its own, so the first
    /// event boots it — the trap 054 phase 7 found on `batch_import`.
    func linkOpened(_ event: [String: Any]) {
        send(event)
    }

    /// The receive screen opened. The machine needs the account before it can
    /// answer anything about a code for it — and it re-clamps the asset to the
    /// account's own chain set.
    func start(account: String, recipient: String, baseUrl: String) {
        send([
            "type": "start",
            "account": account,
            "recipient": recipient,
            "base_url": baseUrl,
        ])
    }

    /// Which asset the code is for (058 US3): a token's page asks for THAT
    /// token, a network row asks for that network's own coin.
    ///
    /// The core re-clamps the amount to the new asset's precision, which is
    /// the reason the pick goes through it rather than being kept in a
    /// `@State` beside the sheet.
    func pickAsset(
        chainId: Int,
        tokenAddress: String?,
        symbol: String,
        decimals: Int,
        networkName: String
    ) {
        send([
            "type": "asset_picked",
            "chain_id": chainId,
            "token_address": tokenAddress ?? NSNull(),
            "symbol": symbol,
            "decimals": decimals,
            "network_name": networkName,
        ])
    }

    /// Boot on the first event, dispatch after — the machine has no boot event
    /// of its own and an event sent before one would be dropped.
    private func send(_ event: [String: Any]) {
        let json = CoreJSON.string(event)
        if !core.boot(json) { core.dispatch(json) }
    }
}
