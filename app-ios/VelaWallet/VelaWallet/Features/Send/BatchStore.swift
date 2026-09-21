//
//  BatchStore.swift
//  VelaWallet
//
//  The `batch_import` machine, hosted beside the send.
//
//  Its own machine rather than a corner of `send`'s, exactly as on every other
//  client: parsing a payroll file, pricing it in somebody's own currency and
//  judging 200 rows is 1,698 lines of decisions that have nothing to do with
//  the send journey. The two meet at ONE point — `Apply` hands the send machine
//  a list of recipients it then mints ids for, and nothing is recomputed on the
//  way across.
//
//  Not resident: the sheet is opened from the form, and `Open` is a full reset
//  by design (a stale paste from a previous open is never reused). The store
//  lives as long as the app so the view survives a SwiftUI rebuild, and the
//  RESET is the core's `Open`.
//

import Foundation
import Observation
import VelaCore

extension BatchImportCore: CoreBridge {}

@MainActor
@Observable
final class BatchStore {

    private(set) var view: BatchViewWire = .empty

    private var core: CoreStore<BatchViewWire>!

    /// How many people one operation will carry. The machine is TOLD this
    /// rather than knowing it, so every client must pass the SAME number —
    /// `BATCH_MAX_RECIPIENTS`, 60 on the web, the desktop and Android. A client
    /// that passed its own would accept a file the others reject.
    static let maxRecipients = 60

    init(executor: BatchExecutor) {
        self.core = CoreStore(
            bridge: BatchImportCore(),
            perform: { operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.view = view },
            onFault: { print("[vela-wallet] batch_import fault: \($0)") }
        )
    }

    /// This machine has no boot event of its own: its first event IS `Open`,
    /// which is a full reset. `CoreStore` DROPS everything sent before boot —
    /// so a store that only ever dispatched would silently discard every
    /// event, and the sheet would render the empty view forever. That is
    /// exactly what the device showed: "解析结果 · 0 条" over a paste box with
    /// two perfectly good lines in it.
    private func dispatch(_ event: [String: Any]) {
        let json = CoreJSON.string(event)
        if !core.boot(json) { core.dispatch(json) }
    }

    /// The sheet opened on a token. A FULL reset — the core's rule, not a
    /// convenience: an old paste priced at an old rate would otherwise be
    /// waiting when somebody opens the sheet for a different coin.
    ///
    /// `maxRecipients` is the send machine's `split_import_room` — the cap
    /// less the rows already on the form. Opened at a flat sixty, the sheet's
    /// "only the first N will be sent" was a promise the append then broke by
    /// truncating past it (web #265).
    func open(
        symbol: String, decimals: Int, balance: String, priceUsd: Double?, currencyCode: String,
        maxRecipients: Int = BatchStore.maxRecipients
    ) {
        dispatch([
            "type": "open",
            "token": [
                "symbol": symbol,
                "decimals": decimals,
                "balance": balance,
                "price_usd": priceUsd.map { $0 as Any } ?? NSNull(),
            ],
            "currency_code": currencyCode,
            "max_recipients": maxRecipients,
        ])
    }

    func setUnit(_ unit: BatchUnit) { dispatch(["type": "set_unit", "unit": unit.rawValue]) }
    func setText(_ text: String) { dispatch(["type": "set_raw_text", "text": text]) }
    func pickFile() { dispatch(["type": "pick_file_requested"]) }
    func saveTemplate() { dispatch(["type": "save_template_requested"]) }
    func editRate(_ text: String) { dispatch(["type": "edit_rate", "text": text]) }
    func resetRate() { dispatch(["type": "reset_rate_to_auto"]) }

    /// Apply, and the recipients that go with it.
    ///
    /// The gate is the core's single `canApply`, never a conjunction assembled
    /// here — and the rows handed over are the core's own, so the addresses the
    /// preview showed as valid are exactly the addresses that get paid.
    ///
    /// Returns `nil` when there is nothing to apply, so the caller cannot seed
    /// an empty split and land on a form with no rows.
    func apply() -> [[String: Any]]? {
        guard view.canApply, !view.recipients.isEmpty else { return nil }
        dispatch(["type": "apply"])
        return view.recipients.map { row in
            [
                "id": "",
                "address": row.address,
                "amount": row.amount,
                "name": row.name.map { $0 as Any } ?? NSNull(),
            ]
        }
    }
}
