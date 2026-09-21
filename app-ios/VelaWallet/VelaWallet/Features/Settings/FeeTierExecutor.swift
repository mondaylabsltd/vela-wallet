//
//  FeeTierExecutor.swift
//  VelaWallet
//
//  The only place the `fee_tier_pref` core touches the outside world (spec
//  068; iOS's since 069). The same two sentences the display currency speaks,
//  against the same store.
//
//  The stored value goes back RAW — whether a string is a tier is the core's
//  call (`parse_stored`), so a name this build does not know reads as the
//  factory `fast` instead of being coerced here into something that would then
//  go on the wire. `vela.feeTier` survives sign-out, which clears only the
//  accounts and the active index: a speed preference belongs to the person
//  and the device.
//

import Foundation

@MainActor
final class FeeTierExecutor {

    static let operations = ["read_stored_tier", "write_stored_tier"]

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) -> String {
        switch operation["type"] as? String ?? "" {
        // Absent ALWAYS means "never chose".
        case "read_stored_tier":
            return CoreJSON.string([
                "type": "stored_tier",
                "raw": store.readString(VelaStore.Key.feeTier) ?? NSNull(),
            ])
        // Best effort: the committed choice stays on screen either way.
        case "write_stored_tier":
            store.writeString(VelaStore.Key.feeTier, operation["tier"] as? String)
            return CoreJSON.string(["type": "tier_written"])
        default:
            print("[vela-wallet] fee_tier_pref: unhandled operation \(operation["type"] ?? "?")")
            return CoreJSON.string(["type": "stored_tier", "raw": NSNull()])
        }
    }
}
