//
//  DbrWire.swift
//  VelaWallet
//
//  The `dapp_browser` machine's view model and its value types, in Swift
//  (spec 070).
//
//  ## What this machine owns, and what it leaves to the shell
//
//  Every decision a page's message leads to: which document it came from,
//  whether the frame may speak, what the answer is, which tab and which
//  document the answer goes to, the per-origin chain, the one signing sheet at
//  a time, the read bounds. The shell owns the web views, posts strings,
//  performs reads and draws — and reports the one fact only it can see, the
//  PLATFORM's origin of the frame that sent a message.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations stay
//  dictionaries, the house rule every wire file here repeats.
//  `BrowserWireDriftTests` decodes these from the real core.
//

import Foundation

/// One per-origin grant, one-for-one with the `vela.perm.<origin>` document
/// (`dapp_permissions::DpermGrant`, which this machine reuses verbatim).
struct DpermGrantWire: Codable, Equatable {
    let origin: String
    /// The address the site was shown. The core re-pins it when the wallet's
    /// active account changes (the extension's `followActiveAccount`).
    let address: String
    let chainId: Int
    let grantedAtMs: Double

    /// The stored shape: snake_case, exactly what the core serialises and
    /// what this app has written under `vela.perm.<origin>` since spec 053.
    var stored: [String: Any] {
        ["origin": origin, "address": address, "chain_id": chainId, "granted_at_ms": grantedAtMs]
    }

    /// A stored grant, or `nil` when the text is not one — a corrupt key is
    /// one site forgotten, never the whole list refused.
    static func read(_ text: String?) -> DpermGrantWire? {
        guard let text, let data = text.data(using: .utf8) else { return nil }
        return try? CoreJSON.decoder.decode(DpermGrantWire.self, from: data)
    }
}

/// The connection sheet's contents: a site is ASKING.
struct DbrConsentViewWire: Decodable, Equatable {
    /// The tab whose page asked first.
    let tab: String
    let origin: String
    /// One entry per coalesced request — the sheet names the origin once.
    let methods: [String]
    /// The account a grant would be made for (the active one).
    let address: String?
    /// The chain the grant would start on — the site's own.
    let chainId: Int
}

/// One open tab, as the core sees it.
struct DbrTabViewWire: Decodable, Equatable, Identifiable {
    let tab: String
    /// The origin of the tab's document, or of the URL it shows while no
    /// document has said hello yet.
    let origin: String?
    /// The address this origin may see. `nil` is the disconnected chip.
    let connectedAddress: String?
    /// The site's chain — per origin, persisted (`vela.chain.<origin>`).
    let chainId: Int
    /// https, or a loopback / private-network http host. The lock is drawn
    /// from this and from nothing else.
    let secure: Bool
    /// The renderer died. Cleared by the next document's hello.
    let crashed: Bool
    /// Something of this tab's is open — a request, a read, a consent, a
    /// signature (spec 099 R2): its engine is never suspended.
    var busy: Bool = false
    /// The browser layer (`DbrPageState` wire name: `blank`, `loading`,
    /// `ready`, `crashed`). A string, so a state this build has never heard of
    /// cannot fail the whole view.
    var page: String = "blank"
    /// The provider layer (`DbrProviderState` wire name: `pending`,
    /// `offered`, `insecure_origin`, `no_hello`).
    var provider: String = "pending"
    var openRequests: Int = 0
    /// Failures worth attention among the tab's last requests.
    var failedRecent: Int = 0
    /// The latest of them — the status entry's line.
    var lastFailure: DbrFailureNoteWire? = nil

    var id: String { tab }
}

/// The status entry's one line about a tab's latest trouble
/// (`dapp_record::DbrFailureNote`). `layer` and `reason` are wire names;
/// `key` is the corpus line the core chose for the reason.
struct DbrFailureNoteWire: Decodable, Equatable {
    let layer: String
    let reason: String
    let method: String
    let key: String
}

/// One request a page sent (`dapp_record::DbrRequestRow`). Every enum is its
/// wire name, for the reason `DbrTabViewWire.page` is.
struct DbrRequestRowWire: Decodable, Equatable, Identifiable {
    /// The page's JSON-RPC id.
    let id: String
    let method: String
    /// `local` · `consent` · `read` · `relay_read` · `signing`.
    let `class`: String
    /// The shell's clock; `0` = unknown.
    let startedMs: Double
    let endedMs: Double?
    /// `open` · `answered` · `failed`.
    let outcome: String
    let code: Int?
    let layer: String?
    let reason: String?

    /// Milliseconds it took, when both ends are known (the core's
    /// `DbrRequestRow::duration_ms`).
    var durationMs: Double? {
        guard let endedMs, startedMs > 0, endedMs >= startedMs else { return nil }
        return endedMs - startedMs
    }
}

/// The inspected tab, whole (`dapp_record::DbrInspectorView`): the status
/// panel draws it, and `report` is what Copy copies.
struct DbrInspectorViewWire: Decodable, Equatable {
    let tab: String
    let origin: String?
    let page: String
    let provider: String
    let connected: Bool
    let chainId: Int
    /// Oldest first.
    let rows: [DbrRequestRowWire]
    /// The record as plain text, the same on every client.
    let report: String
}

/// The corpus lines of the record's vocabulary — `dapp_record`'s
/// `DbrReason::key`, `DbrPageState::key` and `DbrProviderState::key`,
/// mirrored name for name (the core hands the key over only for a tab's
/// `last_failure`; the inspector's rows carry the reason's wire name).
/// `BrowserStatusTests` checks every line exists and that the core's own
/// key agrees for the reasons it can be driven to.
enum DbrRecordWords {
    static let reasons: [String: String] = [
        "navigated_away": "componentsUi.browserStatus.reason.navigatedAway",
        "page_crashed": "componentsUi.browserStatus.reason.pageCrashed",
        // Never shown — a closed tab has no panel — so it has no line of its own.
        "tab_closed": "componentsUi.browserStatus.reason.navigatedAway",
        "wallet_withdrawn": "componentsUi.browserStatus.reason.walletWithdrawn",
        "insecure_origin": "componentsUi.browserStatus.reason.insecureOrigin",
        "not_connected": "componentsUi.browserStatus.reason.notConnected",
        "account_mismatch": "componentsUi.browserStatus.reason.accountMismatch",
        "no_account": "componentsUi.browserStatus.reason.noAccount",
        "consent_busy": "componentsUi.browserStatus.reason.consentBusy",
        "unsupported_method": "componentsUi.browserStatus.reason.unsupportedMethod",
        "unknown_chain": "componentsUi.browserStatus.reason.unknownChain",
        "bad_params": "componentsUi.browserStatus.reason.badParams",
        "too_many_reads": "componentsUi.browserStatus.reason.tooManyReads",
        "unknown_batch": "componentsUi.browserStatus.reason.unknownBatch",
        "wallet_refused": "componentsUi.browserStatus.reason.walletRefused",
        "no_endpoint": "componentsUi.browserStatus.reason.noEndpoint",
        "timed_out": "componentsUi.browserStatus.reason.timedOut",
        "rate_limited": "componentsUi.browserStatus.reason.rateLimited",
        "endpoint_error": "componentsUi.browserStatus.reason.endpointError",
        "reverted": "componentsUi.browserStatus.reason.reverted",
        "rejected_by_person": "componentsUi.browserStatus.reason.rejectedByPerson",
        "relay_refused": "componentsUi.browserStatus.reason.relayRefused",
        "relay_unreachable": "componentsUi.browserStatus.reason.relayUnreachable",
        "not_confirmed_yet": "componentsUi.browserStatus.reason.notConfirmedYet",
        "relay_failed": "componentsUi.browserStatus.reason.relayFailed",
        "signer_unavailable": "componentsUi.browserStatus.reason.signerUnavailable",
        "signer_not_discoverable": "componentsUi.browserStatus.reason.signerNotDiscoverable",
        "signer_failed": "componentsUi.browserStatus.reason.signerFailed",
    ]

    static let pages: [String: String] = [
        "blank": "componentsUi.browserStatus.page.blank",
        "loading": "componentsUi.browserStatus.page.loading",
        "ready": "componentsUi.browserStatus.page.ready",
        "crashed": "componentsUi.browserStatus.page.crashed",
    ]

    static let providers: [String: String] = [
        "pending": "componentsUi.browserStatus.provider.pending",
        "offered": "componentsUi.browserStatus.provider.offered",
        "insecure_origin": "componentsUi.browserStatus.provider.insecureOrigin",
        "no_hello": "componentsUi.browserStatus.provider.noHello",
    ]

    /// `nil` for a name this build has never heard of — the panel then shows
    /// the code rather than a guess.
    static func reason(_ name: String?) -> String? { name.flatMap { reasons[$0] } }
    static func page(_ name: String) -> String? { pages[name] }
    static func provider(_ name: String) -> String? { providers[name] }
}

/// One connected site — Settings' list.
struct DbrSiteViewWire: Decodable, Equatable, Identifiable {
    let origin: String
    let address: String
    let chainId: Int
    let grantedAtMs: Double

    var id: String { origin }
}

/// The request whose signing sheet should be up.
struct DbrSigningViewWire: Decodable, Equatable {
    let tab: String
    let id: String
}

struct DbrViewWire: Decodable, Equatable {
    /// The stored sites have been read.
    let ready: Bool
    let consent: DbrConsentViewWire?
    let tabs: [DbrTabViewWire]
    /// Every connected site, newest first.
    let sites: [DbrSiteViewWire]
    let signing: DbrSigningViewWire?
    let queuedSigning: Int
    /// The inspected tab's whole record (`inspector_opened`), else `nil`.
    var inspector: DbrInspectorViewWire? = nil

    static let empty = DbrViewWire(
        ready: false, consent: nil, tabs: [], sites: [], signing: nil, queuedSigning: 0
    )

    /// The core's facts about one tab, if it has any yet.
    func tab(_ id: String?) -> DbrTabViewWire? {
        guard let id else { return nil }
        return tabs.first { $0.tab == id }
    }
}

/// A request the core forwarded to the signing sheet (`forward_to_signing`).
struct DbrForward: Equatable {
    let tab: String
    let id: String
    let method: String
    let paramsJson: String
    let origin: String
    /// The SITE's chain — the request is signed on it, never on a global one.
    let chainId: Int
    /// The address the site was shown. The signer is pinned to it.
    let grantedAddress: String

    init(tab: String, id: String, method: String, paramsJson: String,
         origin: String, chainId: Int, grantedAddress: String) {
        self.tab = tab
        self.id = id
        self.method = method
        self.paramsJson = paramsJson
        self.origin = origin
        self.chainId = chainId
        self.grantedAddress = grantedAddress
    }

    init(operation: [String: Any]) {
        self.init(
            tab: operation["tab"] as? String ?? "",
            id: operation["id"] as? String ?? "",
            method: operation["method"] as? String ?? "",
            paramsJson: operation["params_json"] as? String ?? "[]",
            origin: operation["origin"] as? String ?? "",
            chainId: (operation["chain_id"] as? NSNumber)?.intValue ?? 0,
            grantedAddress: operation["granted_address"] as? String ?? ""
        )
    }
}
