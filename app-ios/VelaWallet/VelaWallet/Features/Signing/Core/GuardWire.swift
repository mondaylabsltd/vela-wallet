//
//  GuardWire.swift
//  VelaWallet
//
//  The `approval_guard` machine's view model, in Swift.
//
//  ## The mandate this machine exists to keep
//
//  An unlimited approval never leaves this wallet. Not as a warning somebody
//  can tap past — there is no "grant all anyway" chip, by the founder's own
//  ruling — but as a gate: `confirmAllowed` stays false until the person has
//  named a finite cap, and `rewrittenParamsJson` is what gets signed.
//
//  **`rewrittenParamsJson` being `nil` is not a green light.** It means
//  nothing was rewritten, *including when a rewrite failed*, and the untouched
//  params then meet the core's own refusal at the submit chokepoint. Failing
//  closed at two places is deliberate.
//
//  Amounts are DECIMAL STRINGS everywhere, never numbers. A `Double` cannot
//  hold 2^256 - 1, and rounding somebody's spending cap is the bug this whole
//  file exists to avoid.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

enum GuardSurface: String, Decodable {
    /// Not an approval — this machine has nothing to gate.
    case none
    /// An off-chain permit: signed verbatim under deliberate consent, never
    /// through the cap editor.
    case permitSign = "permit_sign"
    /// The editable never-unlimited spending-cap editor.
    case approvalEditor = "approval_editor"
    /// The EIP-5792 per-leg breakdown.
    case batch
}

enum GuardApprovalKind: String, Decodable {
    case erc20Approve = "erc20_approve"
    case increaseAllowance = "increase_allowance"
    case decreaseAllowance = "decrease_allowance"
    case setApprovalForAll = "set_approval_for_all"
    case erc2612Permit = "erc2612_permit"
    case daiPermit = "dai_permit"
    case permit2Single = "permit2_single"
    case permit2Batch = "permit2_batch"
}

/// Why editing is blocked. Semantic — the shell owns the words.
enum GuardBlockReason: String, Decodable {
    /// The dApp submits its own amount on-chain, so the wallet cannot cap it:
    /// rewriting would desync the signature and revert the dApp's own
    /// transaction. The honest advice is an on-chain approval instead.
    case offChainPermit = "off_chain_permit"
    /// A DAI permit grants full-balance access. Sign as requested, or reject.
    case daiPermitFullBalance = "dai_permit_full_balance"
}

enum GuardEditorMode: String, Decodable {
    case requested, balance, custom, revoke, grant
}

enum GuardAmountError: String, Decodable {
    case invalidAmount = "invalid_amount"
    case unlimitedDisabled = "unlimited_disabled"
}

/// Where the amount lives, for the rewrite step. The shell never uses this to
/// rewrite anything — the core does the re-encoding — but it decodes so the
/// view stays a faithful mirror.
enum GuardLocusWire: Decodable, Equatable {
    case calldataWord(index: Int)
    case typedPath(String)

    private enum Keys: String, CodingKey { case type, wordIndex, path }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "calldata_word":
            self = .calldataWord(index: try container.decode(Int.self, forKey: .wordIndex))
        case "typed_path":
            self = .typedPath(try container.decode(String.self, forKey: .path))
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown guard locus '\(other)'"
            )
        }
    }
}

/// The person's decision for one approval.
enum GuardChoiceWire: Decodable, Equatable {
    /// A finite cap, raw base units as a decimal string.
    case amount(raw: String)
    /// 0 / false.
    case revoke
    /// Keep a boolean `true` — explicit and deliberate, and only ever for
    /// `setApprovalForAll` or a DAI permit.
    case grant

    private enum Keys: String, CodingKey { case type, amountRaw }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "amount": self = .amount(raw: try container.decode(String.self, forKey: .amountRaw))
        case "revoke": self = .revoke
        case "grant": self = .grant
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown guard choice '\(other)'"
            )
        }
    }
}

struct GuardDetectedApprovalWire: Decodable, Equatable {
    let kind: GuardApprovalKind
    /// The ERC-20 token, NFT collection or Permit2 token, when known.
    let tokenAddress: String?
    /// Who is being granted spending power.
    let spender: String
    /// Raw base units, decimal string. `nil` for a boolean grant.
    let amountRaw: String?
    /// 256 or 160.
    let amountBits: Int?
    /// Effectively unlimited — at or above the cap, or a boolean grant-all.
    let isUnbounded: Bool
    /// A boolean grant: there is no amount to cap.
    let isBooleanGrant: Bool
    /// Reduces risk (a decrease or a revoke). Rendered as safe.
    let isReducing: Bool
    /// Whether a finite amount can be safely re-encoded for this shape.
    let editable: Bool
    let blockReason: GuardBlockReason?
    /// Unix seconds as a decimal string, when the shape carries a deadline.
    let deadline: String?
    let locus: GuardLocusWire
}

struct GuardTokenMetaViewWire: Decodable, Equatable {
    let symbol: String
    let decimals: Int
    /// The decimals came from the chain. When false the shell must say so —
    /// a cap read in the wrong decimals is off by orders of magnitude.
    let verified: Bool
    let loading: Bool
}

struct GuardEditorViewWire: Decodable, Equatable {
    /// `nil` only on the boolean card, before the deliberate tap.
    let mode: GuardEditorMode?
    let customText: String
    let error: GuardAmountError?
    /// `nil` ⇒ confirm stays disabled. That is the entire point of the
    /// machine.
    let choice: GuardChoiceWire?
    /// What the value row shows, raw base units.
    let displayAmountRaw: String?
    /// The "as requested" chip exists at all — a finite, non-zero incoming
    /// amount. When the request is unlimited the chip is drawn **disabled**,
    /// not merely unselected.
    let requestedFinite: Bool
    /// The one-tap finite balance cap is offered.
    let hasBalanceCap: Bool
    let balanceRaw: String?
}

/// The resulting-total row: "increase by 100" must never read as "cap at 100".
struct GuardIncreaseTotalViewWire: Decodable, Equatable {
    /// The on-chain allowance, when the read succeeded.
    let current: String?
    let increment: String
    /// `nil` means the read failed — and the row still warns that the
    /// increment ADDS to an existing allowance rather than hiding.
    let total: String?
}

struct GuardLegViewWire: Decodable, Equatable {
    let to: String
    let approval: GuardDetectedApprovalWire?
    let meta: GuardTokenMetaViewWire
    /// This leg's inline editor. The card renders it; it derives nothing.
    let editor: GuardEditorViewWire?
    let choice: GuardChoiceWire?
    let needsEditor: Bool
    /// This leg still blocks confirm.
    let needsChoice: Bool
    /// After the person's choice, this still grants broad access.
    let grantsBroad: Bool
}

struct GuardBatchViewWire: Decodable, Equatable {
    let legs: [GuardLegViewWire]
    let anyUncapped: Bool
    /// A leg that sends a token to its own contract burns it — the same
    /// fat-finger the single-send path flags, and far easier to miss buried
    /// in a batch.
    let anyToOwnToken: Bool
    let allSettled: Bool
}

struct GuardViewWire: Decodable, Equatable {
    let surface: GuardSurface
    let detected: GuardDetectedApprovalWire?
    let meta: GuardTokenMetaViewWire
    let editor: GuardEditorViewWire?
    /// **One of three gates.** False while an editable approval has no choice
    /// or a batch leg is unsettled. AND it with `sign_request`'s
    /// `confirmGateOpen` and `fee_policy`'s `confirmFeeReady`.
    let confirmAllowed: Bool
    /// The finite re-encode of the whole request, ready to submit. See the
    /// file header: `nil` is not a green light.
    let rewrittenParamsJson: String?
    let increaseTotal: GuardIncreaseTotalViewWire?
    /// Unverified decimals must be flagged explicitly on screen.
    let decimalsUnverified: Bool
    let expired: Bool
    let batch: GuardBatchViewWire?

    static let empty = GuardViewWire(
        surface: .none, detected: nil,
        meta: GuardTokenMetaViewWire(symbol: "", decimals: 18, verified: false, loading: false),
        editor: nil, confirmAllowed: true, rewrittenParamsJson: nil,
        increaseTotal: nil, decimalsUnverified: false, expired: false, batch: nil
    )
}
