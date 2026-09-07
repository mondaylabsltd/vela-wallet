//
//  DisplayCurrencyExecutor.swift
//  VelaWallet
//
//  The `display_currency` core's four operations.
//
//  Ported from `app-web/vela-wallet/src/lib/settings/core/currency-executor.ts`
//  (spec 024). All four are live since spec 051 phase 2c.
//
//  ## `read_device_currency` is a platform call here, and was a debt elsewhere
//
//  Desktop carried this as an open item — it needed a region→ISO-4217 table.
//  iOS has `Locale.current.currency`, so the answer is the system's rather than
//  a table somebody has to maintain. `nil` for a regionless locale is a real
//  answer and not a failure: the core reads it as "the device has no opinion".
//
//  ## `rate: null` is not `1`
//
//  The whole reason `resolve_rate` fails closed rather than returning a
//  plausible number. A fiat amount multiplied by a defaulted 1 is a real
//  mispayment, not a cosmetic slip — the core's own doc calls it "a real 7x
//  mispayment" — so a code neither rung can price still answers `null`.
//
//  ## The waterfall, and why it is in that order
//
//  `services/currency-rate.ts`: Chainlink's on-chain fiat feeds first, then the
//  configurable endpoint. On-chain first because it is the source with no
//  operator — and second because sixteen currencies is not a list anybody
//  should be limited to, so the endpoint answers for the other ~145.
//

import Foundation

@MainActor
final class DisplayCurrencyExecutor {

    /// Every operation this executor is required to handle
    /// (contracts/shell-operations.md).
    static let operations = [
        "read_stored_code",
        "write_stored_code",
        "read_device_currency",
        "resolve_rate",
    ]

    private let store: VelaStore
    /// The waterfall, in the order it is asked. Each rung answers `nil` for a
    /// code it cannot price — `FiatRates` for anything without an on-chain
    /// feed — so the ORDER here is the whole rule and there is no second copy
    /// of it inside the executor.
    private let sources: [any FiatRateSource]

    convenience init(store: VelaStore, accounts: AccountStore, pool: RpcPool) {
        self.init(store: store, sources: [
            FiatRates(store: store, pool: pool),
            FiatFx(store: store, accounts: accounts),
        ])
    }

    /// The seam the hermetic tests use. No source means nothing can price
    /// anything, which is the state the `rate: null` rule is about — and it is
    /// reachable in production too, on a device with no network.
    init(store: VelaStore, sources: [any FiatRateSource]) {
        self.store = store
        self.sources = sources
    }

    func perform(_ operation: [String: Any]) async -> String {
        switch operation["type"] as? String ?? "" {

        // Absent ALWAYS means "the person never chose" — which the core reads
        // differently from "chose USD". Returning a default here would erase
        // that difference and skip the device-region seed entirely.
        case "read_stored_code":
            return CoreJSON.string([
                "type": "stored_code",
                "code": store.readString(VelaStore.Key.displayCurrency) ?? NSNull(),
            ])

        case "write_stored_code":
            store.writeString(VelaStore.Key.displayCurrency, operation["code"] as? String)
            return CoreJSON.string(["type": "code_written"])

        case "read_device_currency":
            return CoreJSON.string([
                "type": "device_currency",
                "code": Locale.current.currency?.identifier ?? NSNull(),
            ])

        case "resolve_rate":
            let code = operation["code"] as? String ?? ""
            return CoreJSON.string([
                "type": "rate_resolved",
                "code": code,
                "rate": await resolve(code).map { $0 as Any } ?? NSNull(),
            ])

        default:
            // See `ContactsExecutor`: logged, not trapped. `stored_code` with no
            // code leaves the machine in the state it starts in rather than
            // stalled.
            print("[vela-wallet] display_currency: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "stored_code", "code": NSNull()])
        }
    }

    // MARK: - The rate waterfall

    /// USD → `code`, or `nil` when no source can price it.
    ///
    /// `USD` short-circuits at 1 — the identity, not a default. Everything else
    /// asks the chain, then the endpoint, and answers `nil` rather than the
    /// plausible number that would silently mis-state somebody's money.
    private func resolve(_ code: String) async -> Double? {
        let upper = code.uppercased()
        if upper == "USD" { return 1 }
        for source in sources {
            if let rate = await source.rate(upper), rate > 0, rate.isFinite { return rate }
        }
        return nil
    }
}
