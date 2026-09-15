//
//  ActivityWire.swift
//  VelaWallet
//
//  The `activity_feed` machine's view model, in Swift.
//
//  ## Structured values, not formatted strings
//
//  The Expo original carried `"+1 USDT"` and `"$1.00"` through its model, which
//  forced a whole re-adapt on a language change and made the receipt toast
//  reverse-parse its own amount string. Here every row carries `value` +
//  `decimals` + `symbol` + `usdValue`, and the **shell formats**. A locale
//  change is a re-render.
//
//  ## Headers are rows
//
//  The core emits date headers already interleaved with items, in render
//  order, so a header can never inter-sort with an item — that is invariant ⑥,
//  and it is why this is one array rather than a dictionary the shell would
//  have to sort back into shape.
//

import Foundation

enum FeedDirectionWire: String, Decodable {
    case `in`, out
}

enum FeedTxStatusWire: String, Decodable {
    case pending, confirmed, failed
}

enum FeedTxKindWire: String, Decodable {
    case send, receive
    case dappTx = "dapp_tx"
    case signMessage = "sign_message"
    case signTypedData = "sign_typed_data"
    case connect
}

enum FeedBatchKindWire: String, Decodable {
    /// One token → N recipients.
    case split
    /// N tokens → one recipient.
    case multiSelect = "multi_select"
}

/// One stored transaction, as the core reads it back to the detail sheet.
struct FeedTxRecordWire: Decodable, Equatable {
    let id: String
    let userOpHash: String
    let txHash: String
    let from: String
    let to: String
    let toName: String?
    let value: String
    let symbol: String
    let decimals: Int
    let logoUrls: [String]?
    let chainId: Int
    /// Stored epoch **seconds**.
    let timestamp: Double
    let dayStartMs: Double
    let status: FeedTxStatusWire
    /// `nil` = a legacy untyped record, which the core reads as `send`.
    let kind: FeedTxKindWire?
    let usd: String?
}

struct FeedBatchTransferWire: Decodable, Equatable {
    let to: String
    let toName: String?
    let value: String
    let symbol: String
    let decimals: Int
    let usdValue: Double
    let logoUrls: [String]?
}

/// A batch send, summarised from its per-line records.
struct FeedBatchWire: Decodable, Equatable {
    let kind: FeedBatchKindWire
    let count: Int
    let totalUsd: Double
    let transfers: [FeedBatchTransferWire]
    let ids: [String]
    let from: String
    let chainId: Int
    let timestamp: Double
    let status: FeedTxStatusWire
    let txHash: String
    let userOpHash: String
    let symbol: String?
    let logoUrls: [String]?
    let to: String?
    let toName: String?
}

/// One row's payload.
struct FeedItemWire: Decodable, Equatable {
    let id: String
    let direction: FeedDirectionWire
    /// Sender for an inbound row, recipient for an outbound one; `nil` for a
    /// split batch, which has no single counterparty.
    let counterparty: String?
    /// The resolved name, or the one captured at send time. Never invented.
    let alias: String?
    /// `nil` for a multi-token batch row — mixed tokens cannot be summed, and
    /// the drawing shows the asset count instead.
    let value: String?
    let symbol: String
    let decimals: Int?
    /// Numeric USD; `0` when nothing could price it.
    let usdValue: Double
    let chainId: Int
    /// Epoch seconds.
    let timestamp: Double
    let dayStartMs: Double
    let txHash: String?
    let batch: FeedBatchWire?
}

/// A date header or an item, in render order.
enum FeedRowWire: Decodable, Equatable {
    case header(id: String, dayStartMs: Double, timestamp: Double)
    case item(FeedItemWire)

    private enum CodingKeys: String, CodingKey {
        case type, id, dayStartMs, timestamp, item
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "header":
            self = .header(
                id: try container.decode(String.self, forKey: .id),
                dayStartMs: try container.decode(Double.self, forKey: .dayStartMs),
                timestamp: try container.decode(Double.self, forKey: .timestamp)
            )
        case "item":
            self = .item(try container.decode(FeedItemWire.self, forKey: .item))
        case let other:
            // A row kind this build has never heard of. Reported rather than
            // dropped: a silently skipped row is a transaction missing from
            // somebody's history with nothing to say why.
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown feed row `\(other)`"
            )
        }
    }
}

/// The receipt toast — structured, so the shell formats the amount rather than
/// stripping a symbol back off a string.
struct FeedToastWire: Decodable, Equatable {
    let itemId: String
    let value: String
    let symbol: String
    let deadlineMs: Double
}

struct FeedViewWire: Decodable, Equatable {
    let rows: [FeedRowWire]
    /// The raw account-scoped records, for the detail sheet.
    let transactions: [FeedTxRecordWire]
    /// The row that just landed, still glowing.
    let newItemId: String?
    /// `nil` while balance privacy is on — the core withholds it there rather
    /// than trusting the shell to mask it.
    let toast: FeedToastWire?
}
