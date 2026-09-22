//
//  SignWire.swift
//  VelaWallet
//
//  The `sign_request` machine's view model, in Swift.
//
//  One request's whole life: it arrives, it is reviewed, the gas account is
//  checked, it is signed, it is submitted, it is recorded, and the page is
//  answered exactly once. Which of those may happen next is entirely the
//  core's — this file only gives the answer a Swift shape.
//
//  ## The three gates are ANDed, and this machine holds only one
//
//  `confirmGateOpen` is `sign_request`'s own opinion: the request is
//  reviewable, the granted account is reconciled, no pipeline is in flight.
//  The slide may only arm when it is ANDed with `approval_guard`'s
//  `confirmAllowed` and `fee_policy`'s `confirmFeeReady`. Arming on one of the
//  three is how an unlimited approval gets past a guard that had not finished
//  reading the token.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

/// Which surface the request is on right now.
enum SignSurface: String, Decodable {
    case hidden
    /// The signing sheet. Routing *within* it belongs to `approval_guard`'s
    /// surface and to `clear_signing`.
    case sheet
    /// The in-sheet funding swap. **Never a stacked second modal** — on iOS a
    /// modal presented over a modal is simply invisible, which is how the
    /// 2026-07-06 bug made a gas top-up do nothing at all.
    case funding
}

enum SignFundingPresentation: String, Decodable {
    case topup
    case confirming
}

/// What a swipe-dismiss means **right now**. The core routes by phase; the
/// sheet must dispatch `swipe_dismissed` and let it decide, never guess
/// between reject and dismiss itself.
enum SignSwipeAction: String, Decodable {
    case none
    case reject
    case dismiss
    case fundingCancel = "funding_cancel"
}

/// The semantic error vocabulary. The core picks the kind; the shell owns the
/// words.
enum SignErrorKind: String, Decodable {
    /// 4001 — the person said no.
    case userRejected = "user_rejected"
    /// 4001 — cancelled because the wallet changed chains underneath.
    case walletSwitchedChains = "wallet_switched_chains"
    /// 4902.
    case unsupportedChain = "unsupported_chain"
    /// 4100 — the request asked to act as an account this origin was not
    /// granted.
    case unauthorizedAccount = "unauthorized_account"
    case invalidParams = "invalid_params"
    /// 5700 — an EIP-5792 capability this wallet does not support.
    case unsupportedCapability = "unsupported_capability"
    /// -32603 — the final params still carried an unlimited approval, refused
    /// fail-closed.
    case unlimitedApproval = "unlimited_approval"
    /// -32603 — the request would have changed who controls the account
    /// (spec 081). Refused by the wallet, not by the person.
    case selfCallBlocked = "self_call_blocked"
    case fundingCancelled = "funding_cancelled"
    case submitFailed = "submit_failed"
    /// **No response is sent for this one.** The displayed fee quote went
    /// stale before submit; the sheet re-quotes rather than silently
    /// re-pricing what somebody already read.
    case staleFeeQuote = "stale_fee_quote"
}

/// How a request was classified for view routing.
enum SignMethodKind: String, Decodable {
    case transaction
    case batch
    case personalSign = "personal_sign"
    case ethSign = "eth_sign"
    case typedData = "typed_data"
    case generic
}

/// The dApp's own claim about who it is. The origin beside it is the fact.
struct SignDappIdentityWire: Decodable, Equatable {
    let name: String
    let url: String?
}

struct SignErrorNoticeWire: Decodable, Equatable {
    let kind: SignErrorKind
    let detail: String?
}

/// Bundler gas-account funding facts. Amounts are decimal wei strings.
struct SignFundingNeededWire: Decodable, Equatable {
    let depositAddress: String
    let safeAddress: String
    let chainId: Int
    let nativeSymbol: String
    let thresholdWei: String
    let recommendedWei: String
    let currentBalanceWei: String
}

struct SignFundingViewWire: Decodable, Equatable {
    let data: SignFundingNeededWire
    let presentation: SignFundingPresentation
    let denialReason: String?
}

struct SignRequestViewWire: Decodable, Equatable {
    let id: String
    let method: String
    let kind: SignMethodKind
    /// The raw JSON-RPC params array, verbatim and untrusted. The shell hands
    /// it on; it does not interpret it.
    let paramsJson: String
    let origin: String
    let dapp: SignDappIdentityWire?
    /// The request's **own** chain, which may differ from the wallet's global
    /// one when the request stamped a chain.
    let chainId: Int
    let signerAddress: String?
}

/// What the tracker must be handed the moment it appears. Idempotent — the
/// tracker merges by hash.
struct SignTrackerHandoffWire: Decodable, Equatable {
    let userOpHash: String
    let recordIds: [String]
    let chainId: Int
}

/// Why an arriving request never reached the sheet.
///
/// Neither case can occur in the in-app browser: both are properties of a
/// *mailbox* transport (the extension's), where a payload can be stale on
/// arrival or a request id can already have settled in an earlier window. It
/// is decoded anyway, because a view field the shell does not read is a
/// decision the core made and nobody heard — and because 056's dropped-
/// judgement ruler looks for exactly this.
enum SignNoticeWire: Decodable, Equatable {
    /// The payload was older than the transport's TTL. Never signed, and no
    /// response written — the page recovers through the 4900 path.
    case expired
    /// This id already settled in this session. Replay the outcome; never
    /// re-sign.
    case alreadySettled(submitted: Bool)

    private enum Keys: String, CodingKey { case type, outcome }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "expired": self = .expired
        case "already_settled":
            self = .alreadySettled(submitted: try container.decode(String.self, forKey: .outcome) == "submitted")
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown sign notice '\(other)'"
            )
        }
    }
}

/// Why a request is refused outright: the Safe function it would have called,
/// and where in the request it sat.
struct SignBlockedViewWire: Decodable, Equatable {
    let function: String
    let selector: String
    let legIndex: Int?
    let nested: Bool
}

struct SignViewWire: Decodable, Equatable {
    let surface: SignSurface
    let request: SignRequestViewWire?
    let isSigning: Bool
    let isSubmitting: Bool
    let pendingOpHash: String?
    let error: SignErrorNoticeWire?
    let funding: SignFundingViewWire?
    /// **One of three gates.** See the file header.
    let confirmGateOpen: Bool
    /// The granted-account switch has not acknowledged yet.
    let reconcilePending: Bool
    let swipeAction: SignSwipeAction
    let trackerHandoff: SignTrackerHandoffWire?
    /// Never set by the in-app browser — see `SignNoticeWire`.
    let notice: SignNoticeWire?
    let globalChainId: Int
    /// Present when the self-call guard refused the request (spec 081).
    let blocked: SignBlockedViewWire?

    static let empty = SignViewWire(
        surface: .hidden, request: nil, isSigning: false, isSubmitting: false,
        pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: false,
        reconcilePending: false, swipeAction: .none, trackerHandoff: nil,
        notice: nil, globalChainId: 0, blocked: nil
    )

    var isVisible: Bool { surface != .hidden }
}
