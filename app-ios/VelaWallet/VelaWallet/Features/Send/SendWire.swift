//
//  SendWire.swift
//  VelaWallet
//
//  The `send` machine's view model, in Swift.
//
//  4,417 lines of Rust decide what this carries: three modes, live validation,
//  a string-exact Max, the same-asset fee ceiling, the treasury pre-check and
//  the sign→submit lifecycle with its cancel checkpoints. Nothing here is a
//  judgement; every field is one the core already made.
//
//  ## A view may be a subset; an operation may not
//
//  Spec 052 wires single mode. The multi/split fields are carried anyway —
//  they cost nothing to decode and their absence is what makes a later cut's
//  "it renders but does nothing" bug. What is NOT carried is called out where
//  it sits.
//

import Foundation

/// Which drawn state the flow is in. The shell renders by this rather than by
/// remembering where it navigated from.
enum SendStageWire: String, Decodable {
    case lockError = "lock_error"
    case lockResolving = "lock_resolving"
    case receipt
    case selectToken = "select_token"
    case enterDetails = "enter_details"
    case confirm
}

/// One holding the picker offers.
struct SendTokenWire: Decodable, Equatable {
    let network: String
    let chainId: Int
    let symbol: String
    /// A HUMAN decimal string, not raw units — the trap spec 051 phase 2a
    /// pinned with a test on both sides.
    let balance: String
    let decimals: Int
    /// `nil` = the chain's native coin.
    let tokenAddress: String?
    let priceUsd: Double?
    let logoUrls: [String]
    let spam: Bool

    /// `SendToken::id` — the core's own key, and the only spelling of it.
    var id: String { "\(network)_\(tokenAddress ?? "native")_\(symbol)" }
}

/// One row of a split, as the core holds it.
struct SendRecipientDraftWire: Decodable, Equatable {
    let id: String
    let address: String
    let amount: String
    let name: String?
}

/// Why the amount cannot be what was typed.
enum SendAmountWarningWire: Equatable {
    case notEnoughToken(symbol: String)
    case insufficientForGas(symbol: String?)
    case needGas(symbol: String?)
    case cannotConvert(code: String, symbol: String)
}

extension SendAmountWarningWire: Decodable {
    private enum Key: String, CodingKey { case type, symbol, code }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Key.self)
        switch try container.decode(String.self, forKey: .type) {
        case "not_enough_token":
            self = .notEnoughToken(symbol: try container.decode(String.self, forKey: .symbol))
        case "insufficient_for_gas":
            self = .insufficientForGas(symbol: try container.decodeIfPresent(String.self, forKey: .symbol))
        case "need_gas":
            self = .needGas(symbol: try container.decodeIfPresent(String.self, forKey: .symbol))
        case "cannot_convert":
            self = .cannotConvert(
                code: try container.decode(String.self, forKey: .code),
                symbol: try container.decode(String.self, forKey: .symbol)
            )
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown amount warning \(other)"
            )
        }
    }
}

/// The same-asset ceiling: sending a coin that also pays the fee.
///
/// Every figure is a **base-unit decimal string**, and the shell formats them.
/// Android's first cut printed `5000000000000000000 XDAI` on a phone; that is
/// what this comment exists to prevent happening twice.
struct SendFeeIssueWire: Decodable, Equatable {
    let symbol: String
    let transferAmount: String
    let balance: String
    let feeAmount: String
    let total: String
    let maxTransferAmount: String
}

/// Why a unit conversion cannot happen.
struct SendUnitIssueWire: Decodable, Equatable {
    let code: String
    let symbol: String
}

/// Who the recipient is, if anybody could say.
struct SendRecipientIdentityWire: Decodable, Equatable {
    let name: String?
    let source: String?
}

/// What the recipient is. `nil` on either field means **not judged** — never
/// "no". A delegated EOA is a wallet, and the core owns that carve-out.
struct SendRecipientRiskWire: Decodable, Equatable {
    let isContract: Bool?
    let firstTime: Bool?
}

/// The relay's float on this chain, when it is short.
struct SendTreasuryStatusWire: Decodable, Equatable {
    let chainId: Int
    let address: String
    /// `native` or `path_usd`.
    let asset: String
    let balance: String
    let floor: String
    let bootstrapNeeded: Bool
}

/// One leg of a receipt.
struct SendReceiptTransferWire: Decodable, Equatable {
    let to: String
    let toName: String?
    let amount: String
    let symbol: String
    let logoUrls: [String]
    let usdValue: Double
}

struct SendReceiptWire: Decodable, Equatable {
    /// `submitted` / `confirmed` / `failed`.
    let status: String
    /// `fee_hold` / `fee_rejected`.
    let holdReason: String?
    /// `split` / `multi_select`; absent for a plain transfer.
    let kind: String?
    let transfers: [SendReceiptTransferWire]
    let amount: String
    let usdValue: Double
    let submittedAtMs: Double?
    let typicalInclusionS: Int?
}

struct SendViewWire: Decodable, Equatable {
    let stage: SendStageWire
    let loading: Bool
    let locked: Bool
    let amountLocked: Bool
    let resolvingLock: Bool
    let addingNetwork: Bool

    let tokens: [SendTokenWire]
    let selectedToken: SendTokenWire?
    let recipient: String
    /// As typed, in whatever unit `amountFiatCode` names.
    let amount: String
    /// `nil` = the token's own units. **The figure's own code**, not the
    /// display currency: the two differ for one instant after a currency
    /// commit, and that instant is when a figure typed in one currency used to
    /// be printed with another's glyph.
    let amountFiatCode: String?
    let denomToggleShown: Bool
    let denomToggleEnabled: Bool
    let denomToggleReason: SendUnitIssueWire?
    let confirmAmountIssue: SendUnitIssueWire?
    /// `amount` already resolved through the conversion — the ONE number the
    /// confirm page may print, because it is the number the signed batch is
    /// built from.
    let tokenAmount: String
    /// The headline beside From/To, always in token units. A split restates the
    /// sum the money gates already read, rather than the shell adding the rows
    /// up a second time.
    let confirmAmount: String

    let splitMode: Bool
    let recipients: [SendRecipientDraftWire]
    let splitOverBalance: Bool

    let multiSelectMode: Bool
    let multiSelectedIds: [String]
    let multiValuableIds: [String]
    let multiChainId: Int?

    let showScanner: Bool
    let showContactPicker: Bool
    let showBatchImport: Bool

    let estimatingGas: Bool
    let feeBusy: Bool
    /// Chain-guarded by the core: never a previous network's quote.
    let fee: FeeEstimateWire?
    let gasFeeToken: String?
    let amountWarning: SendAmountWarningWire?
    let sameAssetFeeIssue: SendFeeIssueWire?

    let canContinue: Bool
    /// Fee settled ∧ nothing re-quoting ∧ no same-asset breach ∧ idle.
    let canConfirm: Bool
    let sending: Bool
    /// `idle` / `preparing` / `signing` / `submitting` / `confirmed` / `error`.
    let txStatus: String
    /// `generic` / `bundler_fund`.
    let txError: String?
    let txHash: String?
    let userOpHash: String?
    let receipt: SendReceiptWire?
    let treasuryBootstrap: SendTreasuryStatusWire?
    let recipientIdentity: SendRecipientIdentityWire?
    let recipientRisk: SendRecipientRiskWire?
}
