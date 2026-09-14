//
//  SplitRows.swift
//  VelaWallet
//
//  Editing a list of recipients, as four pure functions.
//
//  The core takes the **whole list** on every change (`recipients_changed`),
//  so the shell's job is to produce the next list from the current one and
//  nothing else. Ported from `app-android/.../feature/send/core/SplitRows.kt`
//  (spec 045 research D1).
//
//  Three details carry the weight:
//
//  - **An untouched row comes back unchanged.** The guard on `!=` matters: the
//    core keeps a name and an identity it resolved against a row, and rewriting
//    a row that did not change throws those away and re-resolves them.
//  - **Changing an address clears that row's name.** The name belonged to the
//    old address. Keeping it would put somebody's name above a stranger's
//    address, which is the single most expensive mislabelling this screen can
//    do.
//  - **A new row has an EMPTY id.** The core mints `rcpt_{n}`; a shell that
//    invented one would eventually collide with the core's.
//

import Foundation

enum SplitRows {

    /// One row's draft, as the shell hands it back.
    struct Draft: Equatable {
        var id: String
        var address: String
        var amount: String
        var name: String?

        var json: [String: Any] {
            ["id": id, "address": address, "amount": amount, "name": name as Any? ?? NSNull()]
        }
    }

    static func drafts(from view: SendViewWire) -> [Draft] {
        view.recipients.map {
            Draft(id: $0.id, address: $0.address, amount: $0.amount, name: $0.name)
        }
    }

    static func amountEdited(_ rows: [Draft], id: String, amount: String) -> [Draft] {
        rows.map { row in
            guard row.id == id, row.amount != amount else { return row }
            var next = row
            next.amount = amount
            return next
        }
    }

    static func addressEdited(_ rows: [Draft], id: String, address: String) -> [Draft] {
        rows.map { row in
            guard row.id == id, row.address != address else { return row }
            var next = row
            next.address = address
            // The name belonged to the old address.
            next.name = nil
            return next
        }
    }

    static func removed(_ rows: [Draft], id: String) -> [Draft] {
        rows.filter { $0.id != id }
    }

    /// Append an empty row. The core mints its id.
    static func appended(_ rows: [Draft]) -> [Draft] {
        rows + [Draft(id: "", address: "", amount: "", name: nil)]
    }
}
