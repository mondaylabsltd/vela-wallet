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
//  The REQUEST-building half of this machine (the mode toggle, the amount, the
//  shareable pay-link) has no drawn home on this client; recorded in 056's
//  results rather than invented.
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
        let json = CoreJSON.string(event)
        if !core.boot(json) { core.dispatch(json) }
    }
}
