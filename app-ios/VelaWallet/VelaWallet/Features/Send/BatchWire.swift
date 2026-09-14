//
//  BatchWire.swift
//  VelaWallet
//
//  The `batch_import` machine's view model, in Swift.
//
//  A list of people, from a paste or a file, priced and checked row by row. The
//  parsing and the pricing are the core's — 1,698 lines of them. What crosses
//  this boundary is a grid of strings and a rate, and what comes back is a
//  verdict per row.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

// `BatchUnit` is the drawn model's, in `FlowModels` — same two cases, same two
// spellings, and the drift test would catch a divergence before a screen could.

enum BatchRateStatus: String, Decodable {
    case loading, ok, failed
}

/// One line of the preview, with the core's verdict on it.
struct BatchPreviewRowWire: Decodable, Equatable, Identifiable {
    /// The line number in the file, so a person can find it.
    let line: Int
    let name: String?
    let address: String
    let valid: Bool
    /// This address appears earlier in the same list.
    let dup: Bool
    /// What the file said.
    let rawAmount: String
    /// What that is in the token, after the rate.
    let tokenAmount: String
    /// The row survives everything.
    let ok: Bool

    var id: Int { line }
}

struct BatchRecipientWire: Decodable, Equatable {
    let address: String
    let amount: String
    let name: String?
}

struct BatchViewWire: Decodable, Equatable {
    let opened: Bool
    let unit: BatchUnit
    let fiatCode: String
    let rawText: String
    let fileName: String?
    let busy: Bool
    /// The file could not be read **at all**. Distinct from rows that failed:
    /// one is "this is not a list", the other is "these lines are wrong".
    let fileError: Bool
    let templateSaved: Bool
    /// Amounts are being read as fiat and converted.
    let priced: Bool
    let rateStatus: BatchRateStatus
    let rateInput: String
    /// Somebody typed their own rate, so the automatic one must not overwrite
    /// it.
    let rateEdited: Bool
    let preview: [BatchPreviewRowWire]
    /// More rows than this wallet will send in one operation.
    let overCap: Bool
    let rejected: Int
    let recipientCount: Int
    let totalToken: String
    let totalFiat: String?
    let overBalance: Bool
    /// The single gate. Consumers AND this rather than assembling their own
    /// conjunction of `busy`, `rejected` and `overBalance`.
    let canApply: Bool
    let recipients: [BatchRecipientWire]
    let applied: Bool

    static let empty = BatchViewWire(
        opened: false, unit: .token, fiatCode: "USD", rawText: "", fileName: nil,
        busy: false, fileError: false, templateSaved: false, priced: false,
        rateStatus: .ok, rateInput: "", rateEdited: false, preview: [],
        overCap: false, rejected: 0, recipientCount: 0, totalToken: "0",
        totalFiat: nil, overBalance: false, canApply: false, recipients: [],
        applied: false
    )
}
