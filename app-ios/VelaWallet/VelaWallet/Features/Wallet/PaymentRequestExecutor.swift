//
//  PaymentRequestExecutor.swift
//  VelaWallet
//
//  Two operations: has this account seen the receive warning, and remember
//  that it has.
//
//  `vela.receiveWarned.{account}` — the same key on every client. A read that
//  fails answers `false`, which SHOWS the gate: a warning nobody has
//  acknowledged is the state to assume when the answer is unknown.
//

import Foundation

@MainActor
final class PaymentRequestExecutor {

    static let operations = ["read_ack", "write_ack"]

    private let store: VelaStore

    init(store: VelaStore) {
        self.store = store
    }

    func perform(_ operation: [String: Any]) async -> String {
        let account = operation["account"] as? String ?? ""
        switch operation["type"] as? String ?? "" {
        case "read_ack":
            return CoreJSON.string([
                "type": "ack_flag",
                "acknowledged": store.readString(Self.key(account)) == "1",
            ])
        case "write_ack":
            // Best effort by contract: the core's model is authoritative and a
            // storage failure it cannot undo must not stall the machine.
            store.writeString(Self.key(account), "1")
            return CoreJSON.string(["type": "ack_written"])
        default:
            print("[vela-wallet] payment_request: unhandled operation \(operation["type"] ?? "?")")
            return Self.neutralAnswer(operation)
        }
    }

    static func neutralAnswer(_ operation: [String: Any]) -> String {
        operation["type"] as? String == "write_ack"
            ? CoreJSON.string(["type": "ack_written"])
            // Unknown means the gate SHOWS, which is the safe half.
            : CoreJSON.string(["type": "ack_flag", "acknowledged": false])
    }

    private static func key(_ account: String) -> String {
        "vela.receiveWarned.\(account.lowercased())"
    }
}
