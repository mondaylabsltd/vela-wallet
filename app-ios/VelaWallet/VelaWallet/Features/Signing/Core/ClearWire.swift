//
//  ClearWire.swift
//  VelaWallet
//
//  The `clear_signing` machine's view model, in Swift.
//
//  What a transaction DOES, in words, before it says what it is in hex. Five
//  thousand lines of core decide that — the descriptor fetch, the 4-byte
//  fallback, the chain probes, the six degradation rungs — and none of it is
//  re-derived here.
//
//  ## `surface` is the dispatch, not a hint
//
//  The sheet's full order is: typed permit → editable approval → LOADING →
//  CLEAR SIGN → batch → ETH_SIGN → MESSAGE → BLIND TYPED → BLIND TX. The two
//  approval surfaces and the batch belong to `approval_guard`; everything in
//  capitals is decided here. The sheet interleaves exactly two verdicts and
//  decides nothing itself.
//
//  `loading` holds the sheet on purpose: a blind view must never flash before
//  the clear one, because somebody reading fast would see the scary version of
//  a transaction that turns out to be fine, or worse, the calm version of one
//  that is not.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

/// Risk level for visual treatment.
enum ClearRisk: String, Decodable {
    case safe, normal, caution, danger

    /// The order risks rise in — "the worst call sets a batch's tone".
    var rank: Int {
        switch self {
        case .safe: 0
        case .normal: 1
        case .caution: 2
        case .danger: 3
        }
    }
}

/// Layout role hint — which slot a field belongs in.
enum ClearFieldRole: String, Decodable {
    case sendAmount = "send_amount"
    case receiveAmount = "receive_amount"
    case recipient
    case spender
    case generic
}

enum ClearSignType: String, Decodable {
    case transaction, signature
}

/// One resolved display field.
struct ClearSignFieldWire: Decodable, Equatable {
    /// `var`: the shell relabels the wallet's OWN built-in result (spec 062).
    var label: String
    /// `var`: a capped unlimited approval reads its cap here (see `capped`).
    var value: String
    let format: String
    /// A normalised, validated token address, for the logo lookup.
    let tokenAddress: String?
    /// A high-risk field — an unlimited approval, say. Renders as danger.
    var warning: Bool
    /// An amount shown with decimals that were never verified on-chain.
    /// Caution, not danger: it may well be right, and it may be off by
    /// several orders of magnitude.
    let unverified: Bool
    let role: ClearFieldRole
    /// Collapsed under the advanced disclosure.
    let detail: Bool
    /// A deadline already in the past.
    let expired: Bool
    /// The full lowercased address, for address-name fields.
    let address: String?
    let usdValue: Double?
    /// `label` / `value` as words the shell translates (`ClearTerm`: the key
    /// leaf under `componentsUi.signing`). See `SigningLive.localizedTerms`.
    var labelTerm: String? = nil
    var valueTerm: String? = nil
}

/// Where a description came from (spec 081 FR-008) — the ground `verified`
/// stands on. `fetched` is the descriptor service's word, over plain HTTP,
/// from a base URL the person can edit; `none` is a sheet that no descriptor
/// described at all.
enum ClearProvenance: String, Decodable {
    case builtIn = "built_in"
    case pinnedMatch = "pinned_match"
    case fetched
    case standard
    case selectorDb = "selector_db"
    case none
}

/// The resolved result, ready to draw.
struct ClearSignResultWire: Decodable, Equatable {
    /// The canonical English key — the shell localises it. Printing this
    /// verbatim is how Android shipped a confirm button reading "确认send".
    /// `var`, with `fields`: see `relabelled`.
    var intent: String
    /// `intent` as a word the shell translates (`ClearTerm`).
    var intentTerm: String? = nil
    let contractName: String?
    let owner: String?
    var fields: [ClearSignFieldWire]
    var risk: ClearRisk
    let contractAddress: String?
    /// Derived by the core from `provenance`, never claimed on its own.
    let verified: Bool
    let provenance: ClearProvenance
    let signType: ClearSignType
    /// The descriptor declared more fields than resolved. Say "incomplete"
    /// loudly rather than showing a partial list as if it were whole.
    let partial: Bool
    /// Recovered through the 4-byte database and decoded generically. Best
    /// effort, and it must read that way.
    let bestEffort: Bool
    /// A recipient of this call **is** the contract being called: the token
    /// is being sent to its own contract, which burns it irreversibly.
    let toOwnToken: Bool
}

enum ClearSignMethod: String, Decodable {
    case personalSign = "personal_sign"
    /// `eth_sign` signs an opaque hash. It gets the hard-warning surface,
    /// never the calm message view.
    case ethSign = "eth_sign"
}

/// Parsed EIP-4361 sign-in fields.
struct ClearSiweFieldsWire: Decodable, Equatable {
    let domain: String
    /// The lowercased host the binding was actually **compared on**, or `nil`
    /// when the authority is unparseable. The domain row renders this, so the
    /// string on screen is the string that was adjudicated — showing a
    /// prettier one than the check ran against is how a lookalike slips past.
    let domainHost: String?
    let address: String?
    let statement: String?
    let uri: String?
    let chainId: Int?
    let nonce: String?
}

enum ClearSiweBinding: String, Decodable {
    case ok, mismatch, unknown
}

/// Which danger treatment the message surface takes.
///
/// `siweOk` covers both a matching binding and an unknown one — the calm
/// sign-in layout. The verified badge renders only on a match, and only a
/// **mismatch** escalates. An unknown origin never asserts a match, and is
/// not evidence of phishing either.
enum ClearDangerClass: String, Decodable {
    case plain
    case siweOk = "siwe_ok"
    case siwePhish = "siwe_phish"
    case opaqueHash = "opaque_hash"
    case ethSign = "eth_sign"
}

/// Everything the message view needs, computed once.
struct ClearMessageViewWire: Decodable, Equatable {
    /// The param this method actually signs, **already chosen**: `params[0]`
    /// for `personal_sign`, `params[1]` for `eth_sign`, falling back to
    /// `params[0]` for a malformed single-param `eth_sign`. The shell does not
    /// pick.
    let payload: String
    let isHex: Bool
    /// Hex decoded as UTF-8, or the verbatim non-hex payload.
    let decodedText: String?
    /// A short preview for genuinely binary payloads.
    let binaryPreview: String?
    let nonPrintable: Bool
    let siwe: ClearSiweFieldsWire?
    let binding: ClearSiweBinding?
    let dangerClass: ClearDangerClass
}

/// One raw entry of an undecodable EIP-712 payload, already rendered to the
/// single line the sheet shows.
struct ClearBlindFieldWire: Decodable, Equatable {
    let key: String
    let value: String
}

/// The blind typed-data projection.
///
/// Deliberately **not** reinterpreted: no decimals, no timestamp guessing. The
/// descriptor is unknown, so an honest raw value beats a confident wrong one.
struct ClearBlindTypedWire: Decodable, Equatable {
    let primaryType: String?
    let hasDomain: Bool
    let domainName: String?
    let verifyingContract: String?
    /// The first five message entries, in payload order.
    let fields: [ClearBlindFieldWire]
}

enum ClearSurface: String, Decodable {
    /// Nothing presented, or a method this machine does not adjudicate.
    case none
    /// A descriptor is resolving. **Holds the sheet.**
    case loading
    case clearSign = "clear_sign"
    case ethSign = "eth_sign"
    case messageSign = "message_sign"
    case blindTypedData = "blind_typed_data"
    case blindTransaction = "blind_transaction"
    /// A dApp's plain value transfer — empty calldata (spec 082 RC1): the
    /// same amount card the wallet's own send draws, from `plainSend`.
    case plainSend = "plain_send"
    /// 089 S1: a batch of two or more calls — every call drawn, from `batch`.
    case batch

    /// A surface this build has never heard of is drawn as the blind
    /// transaction — the most cautious rung — rather than failing the view: a
    /// sheet that cannot render is a request nobody can refuse.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = ClearSurface(rawValue: raw) ?? .blindTransaction
    }
}

/// What a plain native send moves, for the `plainSend` card (spec 082
/// RC1–RC5). Every figure is exact and the core's; the coin symbol is the
/// shell's fee-row one (RC5), so a card never shows "xDAI" beside "XDAI".
struct ClearPlainSendWire: Decodable, Equatable {
    /// The recipient, EIP-55.
    let to: String
    /// The value in wei, plain decimal digits.
    let valueWei: String
    /// `valueWei / 10^18`, exact, with the request's locale marks; `"0"` when
    /// nothing moves.
    let amount: String
    /// Zero value: "Send · 0 <coin>", no minus sign, a neutral Confirm (RC3).
    let noValue: Bool
}

/// 089 S1: one call of a batch as the core read it — by the ladder a lone
/// transaction climbs. `surface` is `clearSign` (from `result`), `plainSend`
/// (from `plainSend`) or `blindTransaction`; never omitted.
struct ClearBatchCallWire: Decodable, Equatable {
    let index: Int
    let surface: ClearSurface
    var result: ClearSignResultWire?
    let plainSend: ClearPlainSendWire?
    /// The call's target as sent, EIP-55 when it is an address.
    let to: String?
    /// What "unable to decode" names.
    let dataBytes: Int
    /// The native coin this call moves, exact; `nil` when unreadable.
    let valueWei: String?
    let amount: String?
    var risk: ClearRisk
}

/// 089 S1: every call of a batch, what it moves in all, and its worst call's risk.
struct ClearBatchViewWire: Decodable, Equatable {
    var calls: [ClearBatchCallWire]
    /// `nil` when any call's value cannot be read exactly — no sum is stated.
    let totalValueWei: String?
    let totalAmount: String?
    var risk: ClearRisk
}

/// The confirm button's **semantics**. The words stay in the shell, and so
/// does the measurement of whether a localised intent fits — that is a
/// property of the translated string, not of the request.
enum ClearConfirmWire: Decodable, Equatable {
    /// A pure signature.
    case sign
    /// Neutral. **Never "approve"**: that verb belongs only to an actual token
    /// approval, which is `approval_guard`'s surface.
    case confirm
    /// "Confirm {intent}", where `intent` is the canonical English key for
    /// the shell to localise — never printed raw — and `term` the core's name
    /// for it when it has one (`ClearTerm`).
    case confirmIntent(String, term: String? = nil)

    /// Post-`convertFromSnakeCase` names (`CoreJSON.decoder`): `intent_term`
    /// arrives as `intentTerm`.
    private enum Keys: String, CodingKey { case type, intent, intentTerm }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "sign": self = .sign
        case "confirm": self = .confirm
        case "confirm_intent":
            self = .confirmIntent(
                try container.decode(String.self, forKey: .intent),
                term: try container.decodeIfPresent(String.self, forKey: .intentTerm)
            )
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown clear confirm '\(other)'"
            )
        }
    }
}

struct ClearSigningViewWire: Decodable, Equatable {
    let resolving: Bool
    let resolved: Bool
    var result: ClearSignResultWire?
    let message: ClearMessageViewWire?
    let surface: ClearSurface
    let confirm: ClearConfirmWire
    /// Present for every typed-data request; reached only once resolution has
    /// concluded with no descriptor.
    let blindTyped: ClearBlindTypedWire?
    /// The sheet buzzes a warning on open. Computed **once**, here, so the
    /// haptic and the red banner can never disagree. The unbounded-approval
    /// half of the same buzz is `approval_guard`'s verdict; the sheet ORs the
    /// two and decides nothing.
    let dangerHaptic: Bool
    /// The plain send card (spec 082 RC1): present exactly when `surface` is
    /// `plainSend`.
    var plainSend: ClearPlainSendWire? = nil
    /// 089 S1: every call of a batch — present exactly when `surface` is
    /// `batch`, and then `result` and `plainSend` are `nil`.
    var batch: ClearBatchViewWire? = nil
    /// The verb the request's record keeps — what Activity titles it with
    /// (spec 093). The core decides it (no best-effort guess; a batch's one
    /// shared verb); the approve copies it as `intent`. `nil` when nothing
    /// may be recorded.
    var recordIntent: String? = nil

    static let empty = ClearSigningViewWire(
        resolving: false, resolved: false, result: nil, message: nil,
        surface: .none, confirm: .confirm, blindTyped: nil, dangerHaptic: false
    )
}


extension ClearSignResultWire {
    /// The same result wearing the shell's words: a new intent, and the first
    /// `labels.count` field labels replaced in order. Values, roles, risk and
    /// verification are the core's and are not touched.
    func relabelled(intent: String, labels: [String]) -> ClearSignResultWire {
        var next = self
        next.intent = intent
        for index in next.fields.indices where index < labels.count {
            next.fields[index].label = labels[index]
        }
        return next
    }

    /// The same result reading the cap the person chose instead of the
    /// request's "Unlimited": the approval's warning amount field takes the
    /// cap and stops being a warning, and if it was the only warning the risk
    /// falls to what an approve is anyway — caution (`assess_risk`). Same rule
    /// in every shell (`SigningLive.cappedApproval`).
    func capped(to cap: String) -> ClearSignResultWire {
        var next = self
        for index in next.fields.indices where next.fields[index].warning && next.fields[index].format == "tokenAmount" {
            next.fields[index].value = cap
            next.fields[index].warning = false
        }
        if next.risk == .danger, !next.fields.contains(where: { $0.warning }) {
            next.risk = .caution
        }
        return next
    }
}
