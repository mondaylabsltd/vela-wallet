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
    let label: String
    let value: String
    let format: String
    /// A normalised, validated token address, for the logo lookup.
    let tokenAddress: String?
    /// A high-risk field — an unlimited approval, say. Renders as danger.
    let warning: Bool
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
}

/// The resolved result, ready to draw.
struct ClearSignResultWire: Decodable, Equatable {
    /// The canonical English key — the shell localises it. Printing this
    /// verbatim is how Android shipped a confirm button reading "确认send".
    let intent: String
    let contractName: String?
    let owner: String?
    let fields: [ClearSignFieldWire]
    let risk: ClearRisk
    let contractAddress: String?
    let verified: Bool
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
    /// the shell to localise — never printed raw.
    case confirmIntent(String)

    private enum Keys: String, CodingKey { case type, intent }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "sign": self = .sign
        case "confirm": self = .confirm
        case "confirm_intent":
            self = .confirmIntent(try container.decode(String.self, forKey: .intent))
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
    let result: ClearSignResultWire?
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

    static let empty = ClearSigningViewWire(
        resolving: false, resolved: false, result: nil, message: nil,
        surface: .none, confirm: .confirm, blindTyped: nil, dangerHaptic: false
    )
}
