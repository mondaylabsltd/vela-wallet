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

/// `KeyMethod` — where a key lives: this device, a phone or tablet, a USB
/// security key. Three, and no fourth (spec 102).
///
/// The CHOICE, not the report: `CreateKeyRow` separately carries what the
/// authenticator said about itself, and the two can legitimately disagree. The
/// ceremony follows the choice; the row's provider line shows the report.
///
/// Spec 075 drew the Trusted Signer as a fourth place beside these. It never
/// was one — the same three places exist on the page too — so spec 102 made it
/// what the code always said it was: WHERE a person reviews and signs (the
/// account's signing venue), chosen apart from where its key lives. A key
/// ceremony that runs on a page says so with the operation's own `page`, never
/// with a method. `allCases` is what the create and sign-in choosers list.
enum KeyMethod: String, Decodable, CaseIterable {
    case platform
    case hybrid
    case securityKey = "security_key"
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

/// `CreateView`.
struct CreateView: Equatable {
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
    /// The places a key may be minted in — always the three (spec 102).
    let addMethods: [KeyMethod]
    /// Spec 102: the domain this wallet's keys are minted for — `getvela.app`,
    /// or the domain of the page chosen with "Use a trusted signing page". Shown,
    /// so a person sees which site their keys will belong to.
    var signingDomain: String = "getvela.app"
    /// That page, normalised, when one was chosen. On a custom domain every
    /// ceremony runs there (R3); a `getvela.app` page runs them in the app and
    /// becomes the new account's venue.
    var signingPage: String? = nil
    /// May a signing page still be chosen? Only before the first key: the
    /// first key commits the set to one domain.
    var canChoosePage: Bool = false
}

extension CreateView: Decodable {
    private enum CodingKeys: String, CodingKey {
        case stage, name, nameEditable, nameTooLong, acks, canSubmit, submitLabel
        case showStartOver, busy, status, keys, canAddKey, canFinish, needsSecondKey
        case canGoBack, address, syncErrorDetail, addMethods
        case signingDomain, signingPage, canChoosePage
    }

    /// Written out for the three spec-102 fields: a decode failure here is
    /// swallowed by the model (`OnboardingModel`'s `try?`), which freezes the
    /// create screen on its last view with nothing in any log — so a field the
    /// wire grows or drops costs only itself. The rest stay required: a view
    /// missing them is not one this build can draw.
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        stage = try values.decode(CreateStage.self, forKey: .stage)
        name = try values.decode(String.self, forKey: .name)
        nameEditable = try values.decode(Bool.self, forKey: .nameEditable)
        nameTooLong = try values.decode(Bool.self, forKey: .nameTooLong)
        acks = try values.decode([Bool].self, forKey: .acks)
        canSubmit = try values.decode(Bool.self, forKey: .canSubmit)
        submitLabel = try values.decode(SubmitLabel.self, forKey: .submitLabel)
        showStartOver = try values.decode(Bool.self, forKey: .showStartOver)
        busy = try values.decode(Bool.self, forKey: .busy)
        status = try values.decodeIfPresent(StatusKey.self, forKey: .status)
        keys = try values.decode([CreateKeyRow].self, forKey: .keys)
        canAddKey = try values.decode(Bool.self, forKey: .canAddKey)
        canFinish = try values.decode(Bool.self, forKey: .canFinish)
        needsSecondKey = try values.decode(Bool.self, forKey: .needsSecondKey)
        canGoBack = try values.decode(Bool.self, forKey: .canGoBack)
        address = try values.decodeIfPresent(String.self, forKey: .address)
        syncErrorDetail = try values.decodeIfPresent(String.self, forKey: .syncErrorDetail)
        addMethods = try values.decodeIfPresent([KeyMethod].self, forKey: .addMethods) ?? KeyMethod.allCases
        signingDomain = try values.decodeIfPresent(String.self, forKey: .signingDomain) ?? "getvela.app"
        signingPage = try values.decodeIfPresent(String.self, forKey: .signingPage)
        canChoosePage = try values.decodeIfPresent(Bool.self, forKey: .canChoosePage) ?? false
    }
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
    /// The core's verdict that the link to the other device failed, not the
    /// authenticator (issue #446): the sheet says to scan again.
    let phoneLink: Bool
    /// The core's verdict that a not-supported ceremony was a SECURITY KEY's
    /// (issue #450): the sheet talks about the key, never about Face ID.
    let securityKey: Bool
    /// `registry_unreachable`: every lookup failed without leaving the device
    /// (no network) — the sheet says "check your network" rather than "Vela
    /// couldn't check".
    let local: Bool

    var id: String { detail.map { "\(type)|\($0)" } ?? (local ? "\(type)|local" : type) }

    init(
        type: String, detail: String? = nil, phoneLink: Bool = false, securityKey: Bool = false,
        local: Bool = false
    ) {
        self.type = type
        self.detail = detail
        self.phoneLink = phoneLink
        self.securityKey = securityKey
        self.local = local
    }

    init(json: [String: Any]) {
        self.type = json["type"] as? String ?? ""
        self.detail = json["detail"] as? String
        self.phoneLink = json["phone_link"] as? Bool ?? false
        self.securityKey = json["security_key"] as? Bool ?? false
        self.local = json["local"] as? Bool ?? false
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
