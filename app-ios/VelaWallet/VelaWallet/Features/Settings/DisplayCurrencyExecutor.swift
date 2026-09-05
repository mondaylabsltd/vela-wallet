//
//  DisplayCurrencyExecutor.swift
//  VelaWallet
//
//  The `display_currency` core's four operations.
//
//  Ported from `app-web/vela-wallet/src/lib/settings/core/currency-executor.ts`
//  (spec 024). Three are live; pricing waits for 051.
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
//  mispayment" — so until 051 has a price source, the honest answer is that
//  nobody knows.
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

    init(store: VelaStore) {
        self.store = store
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

        // live in 051 — the price path.
        case "resolve_rate":
            return CoreJSON.string([
                "type": "rate_resolved",
                "code": operation["code"] as? String ?? "",
                "rate": NSNull(),
            ])

        default:
            // See `ContactsExecutor`: logged, not trapped. `stored_code` with no
            // code leaves the machine in the state it starts in rather than
            // stalled.
            print("[vela-wallet] display_currency: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "stored_code", "code": NSNull()])
        }
    }
}
