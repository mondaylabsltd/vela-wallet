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
//  ## The confirm's gate is one core function (spec 099 R7)
//
//  `confirmGateOpen` is `sign_request`'s own opinion: the request is
//  reviewable, the granted account is reconciled, no pipeline is in flight.
//  Whether the confirm may arm is the core's `signConfirmState` over this view,
//  `approval_guard`'s, `clear_signing`'s and `fee_policy`'s — with the rules
//  every client used to add on top (a message has no fee, another speed's
//  figure is not this speed's, a request still being read is not signable)
//  and the line that says which part is shut. This app had two copies of the
//  gate that disagreed; there are none now.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation
import VelaCore

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
    /// -32603 — no passkey can be used here (an unsigned build, a device
    /// without one; spec 099 R8, the passkey classifier's `not_supported`).
    case signerUnavailable = "signer_unavailable"
    /// -32603 — a passkey sign-in would never offer (`not_discoverable`).
    case signerNotDiscoverable = "signer_not_discoverable"
    /// -32603 — the passkey prompt failed for another reason (`other`).
    case signerFailed = "signer_failed"
    /// -32603 — this account cannot sign here (spec 102): nothing on this
    /// device can reach its keys. Never retried; the notice's `venueBlock`
    /// says why.
    case venueBlocked = "venue_blocked"

    /// A kind this build has never heard of reads as a failed submission —
    /// the generic failure, never a view that cannot be drawn.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = SignErrorKind(rawValue: raw) ?? .submitFailed
    }

    /// The passkey is what failed (spec 099 R8): the sheet says so in the
    /// signer layer's words, the request record's own line.
    var signerReasonKey: String? {
        switch self {
        case .signerUnavailable: "componentsUi.browserStatus.reason.signerUnavailable"
        case .signerNotDiscoverable: "componentsUi.browserStatus.reason.signerNotDiscoverable"
        case .signerFailed: "componentsUi.browserStatus.reason.signerFailed"
        default: nil
        }
    }
}

/// May the signing confirm arm, and if not, why not — the core's
/// `sign_confirm::ConfirmState`, from `signConfirmState` over the four views
/// as the core last wrote them (spec 099 R7).
struct SignConfirmStateWire: Decodable, Equatable {
    let enabled: Bool
    /// `ConfirmBlock` wire name: which part of the gate is shut.
    let block: String?
    /// The line under the shut confirm (`componentsUi.signing.confirmBlock.*`),
    /// or `nil` where the sheet already says it its own way.
    let key: String?

    /// Shut, with nothing to say — what a view that does not read gets.
    static let shut = SignConfirmStateWire(enabled: false, block: nil, key: nil)

    /// The core's verdict. `fee` is `nil` with no fee session (a message);
    /// `speedTier` the speed in force, `nil` with no speed control. Any view
    /// missing or unreadable keeps the confirm shut.
    static func of(
        sign: String?, guard guardJson: String?, clear: String?, fee: String?, speedTier: String?
    ) -> SignConfirmStateWire {
        guard let sign, let guardJson, let clear,
              let json = signConfirmState(
                  signJson: sign, guardJson: guardJson, clearJson: clear,
                  feeJson: fee, speedTier: speedTier
              ),
              let state = try? CoreJSON.decoder.decode(SignConfirmStateWire.self, from: Data(json.utf8))
        else { return .shut }
        return state
    }
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
    /// Spec 102: for `venueBlocked`, why — drawn as `VenueBlock::key()` with
    /// its domains, in the person's language.
    var venueBlock: VenueBlockWire? = nil

    private enum CodingKeys: String, CodingKey { case kind, detail, venueBlock }

    init(kind: SignErrorKind, detail: String?, venueBlock: VenueBlockWire? = nil) {
        self.kind = kind
        self.detail = detail
        self.venueBlock = venueBlock
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        kind = try values.decode(SignErrorKind.self, forKey: .kind)
        detail = try values.decodeIfPresent(String.self, forKey: .detail)
        venueBlock = try? values.decodeIfPresent(VenueBlockWire.self, forKey: .venueBlock)
    }

    /// The venue's reason, for a `venueBlocked` notice — `nil` for any other.
    func venueReason(_ loc: Loc) -> String? {
        kind == .venueBlocked ? venueBlock?.text(loc) : nil
    }
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
    /// The wallet's own request (the key backup), never a page's: the shell
    /// said so at `request_arrived` — the one place it raises one — and the
    /// core carries it here. Never derived from the reading, which a site can
    /// submit byte for byte. The core always sends it; the default is only
    /// for hand-built views.
    var firstParty: Bool = false
}

/// What the tracker must be handed the moment it appears. Idempotent — the
/// tracker merges by hash.
struct SignTrackerHandoffWire: Decodable, Equatable {
    let userOpHash: String
    let recordIds: [String]
    let chainId: Int
    /// The submit's reply was lost and `userOpHash` is the local hash (spec
    /// 082 RA3): forwarded to the tracker's `submitted`, which then follows
    /// the op as "may have been sent" to its end.
    var maybeSent: Bool = false
    /// The chain head read before the first submit POST — where the tracker's
    /// relay-independent landing check starts (ruling 8). `nil` = unknown.
    var submitBlock: Int? = nil
    /// The relay took the op the write-ahead hand-off announced (spec 082
    /// RJ1): forwarded to the tracker's `submitted`, so an accepted op never
    /// reads "may have been sent". Same hash and ids as the write-ahead
    /// hand-off — which is why the shell's de-duplication key carries it.
    var admitted: Bool = false
    /// The account that signed it (PR 2 §3), forwarded to the tracker's
    /// `submitted`: what makes the op one this account must wait for.
    var sender: String? = nil
}

/// A write-ahead record proven never sent (spec 082 RJ1): fed to the
/// tracker's `withdrawn` the moment it appears, once per value.
struct SignTrackerWithdrawWire: Decodable, Equatable {
    let userOpHash: String
    let recordIds: [String]
}

/// What the signing sheet is doing, in the words a person reads (spec 082
/// RA9, G22). The network work before the passkey is "preparing", never
/// "waiting for your signature": the core moves to `awaitingSignature` only
/// when the shell says the prompt is up (`ceremony_started`).
///
/// A phase this build has never heard of reads as `preparing` rather than
/// failing the whole view — a sheet that says "preparing" a moment too long is
/// honest; a sheet that cannot render is not.
enum SignPhaseWire: String, Decodable, Equatable {
    case idle
    case preparing
    case awaitingSignature = "awaiting_signature"
    case submitting

    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = SignPhaseWire(rawValue: raw) ?? .preparing
    }
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
    /// This machine's own part of the gate. See the file header: the confirm
    /// reads `SignConfirmStateWire`, never this alone.
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
    /// The sheet's words (spec 082 RA9) — read instead of `isSigning` /
    /// `isSubmitting`, which the core keeps only until every shell reads this.
    var phase: SignPhaseWire = .idle
    /// The sheet's op may have been sent: its submit reply was lost (spec 082
    /// RA3). The caption says so, and there is no Retry.
    var pendingOpMaybeSent: Bool = false
    /// A write-ahead record the core proved never sent (spec 082 RJ1): the
    /// tracker is told to drop it, once.
    var trackerWithdraw: SignTrackerWithdrawWire? = nil
    /// `error` is the relay refusing the op (spec 082 RJ3): the sheet says
    /// `componentsUi.signing.refused` under `statusFailed`, never "try again".
    var failureRefused: Bool = false
    /// WHY the relay did not take it (PR 2 note 9), the one sentence under
    /// the sheet's failure for both ways a refusal arrives: at submit (the
    /// account's previous op holds the nonce → `componentsUi.signing.notSentBody`,
    /// under `failureNotSent`'s calm title, retryable; any other refusal →
    /// `componentsUi.signing.refused`) and after it (the tracker's verdict,
    /// forwarded with `op_tracked.refusal` — the entry's own `refusal_key`).
    /// `nil` for a failure that was no refusal.
    var failureRefusalKey: String? = nil
    /// The failure on the sheet is no failure (PR 2 polish): the relay turned
    /// the operation back at submit because the account's previous one on
    /// this network still holds the nonce. Nothing was sent and nothing went
    /// wrong — the sheet says "Not sent yet" over `failureRefusalKey`'s
    /// sentence, calmly (no red mark, no error tint), with Try again
    /// (`failureRetryable`). Optional so a view without it (an older core, a
    /// hand-written fixture) decodes; read it through `notSent`.
    var failureNotSent: Bool? = nil
    /// Spec 096 F8: the failure on the sheet sent nothing and was no refusal,
    /// and its answer is still held — the receipt offers Try again
    /// (`retry_tapped`) beside Done, which answers the page.
    var failureRetryable: Bool = false

    static let empty = SignViewWire(
        surface: .hidden, request: nil, isSigning: false, isSubmitting: false,
        pendingOpHash: nil, error: nil, funding: nil, confirmGateOpen: false,
        reconcilePending: false, swipeAction: .none, trackerHandoff: nil,
        notice: nil, globalChainId: 0, blocked: nil
    )

    var isVisible: Bool { surface != .hidden }

    /// `failureNotSent`, absent read as `false`.
    var notSent: Bool { failureNotSent == true }
}
