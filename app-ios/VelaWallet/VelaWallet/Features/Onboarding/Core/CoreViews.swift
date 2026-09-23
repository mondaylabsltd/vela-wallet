//
//  CoreViews.swift
//  VelaWallet
//
//  The core's view models, in Swift (spec 019).
//
//  The bridge speaks JSON in both directions
//  (contracts/shell-operations.md §3), so this file is the one place a field
//  name from `vela-core` is spelled out on iOS. Everything downstream reads a
//  typed value.
//
//  Views are `Decodable` with `.convertFromSnakeCase`; OPERATIONS and RESULTS
//  stay dictionaries. That split is deliberate rather than lazy: an operation is
//  a tagged union of eighteen shapes, and a Swift enum for it would be ~300
//  lines of hand-written `init(from:)` whose only job is to reproduce a
//  discriminator the executor immediately switches on again. A view is a flat
//  record read by screens, where types earn their keep.
//

import Foundation

/// `CreateStage` — which screen of the create journey the core is in.
enum CreateStage: String, Decodable {
    case form
    case addKeys = "add_keys"
    case syncFailed = "sync_failed"
    case created
}

/// `StatusKey` — the transient line the core reports. Semantic, never words.
enum StatusKey: String, Decodable, CaseIterable {
    case settingUpIdentity = "setting_up_identity"
    case verifyingIdentity = "verifying_identity"
    case extractingKey = "extracting_key"
    case computingAddress = "computing_address"
    case syncingKey = "syncing_key"
    case setupCancelled = "setup_cancelled"
    case verifyCancelled = "verify_cancelled"
}

/// `SubmitLabel` — which word the create form's primary button carries.
enum SubmitLabel: String, Decodable, CaseIterable {
    case create
    case finishVerify = "finish_verify"
}

/// `KeyMethod` — how the person chose to mint a founding key.
///
/// The CHOICE, not the report: `CreateKeyRow` separately carries what the
/// authenticator said about itself, and the two can legitimately disagree. The
/// ceremony follows the choice; the row's provider line shows the report.
///
/// Spec 075 adds a FOURTH, and it is a peer of the other three rather than a
/// special case: the Trusted Signer is our own passkey route — a page that shows
/// what is being signed and runs the ceremony itself — offered wherever "this
/// device", "a nearby device" and "a security key" are. `allCases` is what the
/// create and sign-in choosers list, so it is also what makes it appear on
/// both of them.
enum KeyMethod: String, Decodable, CaseIterable {
    case platform
    case hybrid
    case securityKey = "security_key"
    case trustedSigner = "trusted_signer"
}

/// `SessionRoute` — where the app is allowed to be.
enum SessionRoute: String, Decodable {
    case loading
    case onboarding
    case wallet
}

/// `CreateKeyRow` — one row of the founding-key list.
struct CreateKeyRow: Decodable, Equatable, Identifiable {
    let name: String
    let authenticatorAttachment: String
    let transports: String
    /// The key confirmed its group membership at creation. A `false` row offers
    /// a per-row retry, and finishing is gated on every row being `true`.
    let confirmed: Bool
    /// Backed up to a sync fabric. Unknown attestation reads as `true` —
    /// display and the second-key gate both fail open.
    let synced: Bool
    /// Is `synced` a FACT, or the benefit of the doubt? `false` when the
    /// attestation was unreadable — the row then draws no badge (issue #207).
    let syncedKnown: Bool
    let aaguid: String
    /// The vault holding this key, resolved by the core from `aaguid`. Empty
    /// when the catalog does not know the model — the row then says what it
    /// always said, from `method`.
    let providerName: String
    let method: KeyMethod
    /// What this key IS, from the authenticator's own report. Rows draw their
    /// icon AND caption from this one field (issue #207); `method` is routing.
    let kind: KeyMethod

    /// Position-based, because the core's list has no ids and position IS the
    /// canonical founding order the address derivation pins.
    var id: String { "\(name)-\(aaguid)-\(method.rawValue)" }
}

/// `AddBlocked` — why a key route is not on offer (spec 075).
///
/// A wallet's keys all belong to ONE relying party, because the registry files
/// a unit under one `rpId`. Once the first key is minted, a route that would
/// mint for a different party cannot add to the set — and the row says so,
/// naming both sides, because the Trusted Signer's page is a setting the person
/// can change.
struct AddBlocked: Decodable, Equatable {
    let relyingParty: String
    let page: String?
    let pageRelyingParty: String?
}

/// `CreateView`.
struct CreateView: Decodable, Equatable {
    let stage: CreateStage
    let name: String
    let nameEditable: Bool
    let nameTooLong: Bool
    let acks: [Bool]
    let canSubmit: Bool
    let submitLabel: SubmitLabel
    let showStartOver: Bool
    let busy: Bool
    let status: StatusKey?
    let keys: [CreateKeyRow]
    let canAddKey: Bool
    let canFinish: Bool
    let needsSecondKey: Bool
    let canGoBack: Bool
    let address: String?
    let syncErrorDetail: String?
    /// Spec 075: the routes that may still mint a key for THIS set.
    let addMethods: [KeyMethod]
    /// Why the others may not, when some may not.
    let addBlocked: AddBlocked?
}

/// `LoginView` — two booleans, and it stays that way (data-model §4).
struct LoginView: Decodable, Equatable {
    let busy: Bool
    let endpointUnreachable: Bool

    static let idle = LoginView(busy: false, endpointUnreachable: false)
}

/// One row of the account switcher. `index` is the position in the ORIGINAL
/// list — exactly what `SwitchAccount` expects — so a re-sorted display can
/// never dispatch a display position.
struct SessionAccountRow: Decodable, Equatable {
    struct Account: Decodable, Equatable {
        let name: String
        let address: String
    }

    let index: Int
    let account: Account
}

/// Present iff the sign-out confirmation dialog is open.
///
/// `Identifiable` so `.sheet(item:)` can drive off it directly: the sheet's
/// existence IS this value's existence, and a separate `@State` bool would be a
/// second source of truth for a fact the core already owns.
struct SessionSignOutView: Decodable, Equatable, Identifiable {
    let pendingUploadWarning: Bool
    /// How many wallets this device is signed into — the sheet says so when it
    /// is more than one, because "nothing is deleted, it all comes back" says
    /// nothing about signing in six times (2026-09-23).
    let accountCount: Int

    var id: Bool { pendingUploadWarning }
}

/// `SessionView` — the route guard and the account list.
struct SessionView: Decodable, Equatable {
    let loading: Bool
    let hasWallet: Bool
    /// The active account's address, `""` when there is none — derived, so it
    /// is `accounts[activeIndex].address` by construction.
    let address: String
    let activeIndex: Int
    let accounts: [SessionAccountRow]
    let allowedRoute: SessionRoute
    let signOut: SessionSignOutView?

    /// The active account's display NAME, `""` when there is none.
    ///
    /// The address rides in `SessionView` pre-derived; the name does not, so
    /// every screen that wants it would otherwise re-index the account list —
    /// and an out-of-range `activeIndex` from a torn view would crash rather
    /// than render an empty header.
    var activeName: String {
        accounts.indices.contains(activeIndex) ? accounts[activeIndex].account.name : ""
    }

    static let booting = SessionView(
        loading: true,
        hasWallet: false,
        address: "",
        activeIndex: 0,
        accounts: [],
        allowedRoute: .loading,
        signOut: nil
    )
}

/// `PromptKind` — a question or a notice.
///
/// `detail` is the platform's own words on the two variants that carry them,
/// and it is forwarded verbatim: it goes into the bug report, and prettifying it
/// here would lose the only part worth filing.
struct PromptKind: Equatable, Identifiable {
    let type: String
    let detail: String?

    var id: String { detail.map { "\(type)|\($0)" } ?? type }

    init(type: String, detail: String? = nil) {
        self.type = type
        self.detail = detail
    }

    init(json: [String: Any]) {
        self.type = json["type"] as? String ?? ""
        self.detail = json["detail"] as? String
    }
}

// MARK: - Decoding

enum CoreJSON {
    /// The one decoder every view goes through.
    static let decoder: JSONDecoder = {
        let decoder = JSONDecoder()
        decoder.keyDecodingStrategy = .convertFromSnakeCase
        return decoder
    }()

    /// Decode one view from the bridge's JSON object.
    ///
    /// Throws rather than returning `nil`: a view the app cannot read means the
    /// core and this client disagree about the wire, and rendering a default
    /// would show a person a screen the machine is not in.
    static func decode<T: Decodable>(_ type: T.Type, from object: [String: Any]) throws -> T {
        let data = try JSONSerialization.data(withJSONObject: object)
        return try decoder.decode(type, from: data)
    }

    /// Serialize a result dictionary for the bridge.
    static func string(_ object: [String: Any]) -> String {
        guard let data = try? JSONSerialization.data(withJSONObject: object),
              let text = String(data: data, encoding: .utf8)
        else {
            // Cannot happen for the dictionaries this app builds — every value
            // is a String, Bool, Int or NSNull. If it ever does, the core must
            // still be unblocked with something it can parse.
            return #"{"type":"storage_failed","message":"could not serialize the shell result"}"#
        }
        return text
    }

    static func object(_ text: String) throws -> [String: Any] {
        guard let data = text.data(using: .utf8),
              let object = try JSONSerialization.jsonObject(with: data) as? [String: Any]
        else {
            throw CoreBridgeError.malformed(text)
        }
        return object
    }
}

enum CoreBridgeError: Error, LocalizedError {
    case malformed(String)

    var errorDescription: String? {
        switch self {
        case .malformed(let text):
            return "the core returned something that is not a JSON object: \(text)"
        }
    }
}
