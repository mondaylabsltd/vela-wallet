//
//  DpermWire.swift
//  VelaWallet
//
//  The `dapp_permissions` machine's view model and its value types, in Swift.
//
//  ## What this machine owns, and what it refuses to
//
//  It owns whether an origin may be told anything: which address a grant is
//  pinned to, when a sheet must open, what a revoke undoes, and which error
//  code a refusal carries. The shell owns none of that. What the shell owns is
//  the **origin** — read natively from the web view, never from the envelope —
//  and the transport that carries the answer.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

/// One per-origin grant, one-for-one with the `vela.perm.<origin>` document.
struct DpermGrantWire: Codable, Equatable {
    let origin: String
    /// The address the person granted. The grant is **pinned here**, never to
    /// the wallet's active account: switching accounts must not silently hand
    /// a site a different identity.
    let address: String
    let chainId: Int
    /// Stamped by the shell at grant time. It participates in no decision —
    /// grants have no expiry today — and is kept because the other clients
    /// write it.
    let grantedAtMs: Double
}

/// Why a request was refused. Nine reasons; the core picks, the shell says.
enum DpermRejectReason: String {
    /// A cross-origin subframe asked for accounts, consent or a signature.
    case unauthorizedFrame = "unauthorized_frame"
    case noAccountAvailable = "no_account_available"
    /// A second origin collided with the open consent sheet. A page cannot
    /// queue two connect sheets.
    case consentBusy = "consent_busy"
    /// Signing on a public non-TLS origin.
    case insecureOrigin = "insecure_origin"
    case userRejected = "user_rejected"
    /// The document navigated away with the answer still pending.
    case navigatedAway = "navigated_away"
    case browserClosed = "browser_closed"
    case notConnected = "not_connected"
    /// The request pinned an address that is no longer the granted one.
    case staleAuthorizedAddress = "stale_authorized_address"
}

/// One request coalesced into the open consent sheet.
struct DpermQueuedRequestWire: Decodable, Equatable, Identifiable {
    let id: String
    let method: String
}

/// The consent sheet's contents.
struct DpermConsentViewWire: Decodable, Equatable {
    let origin: String
    /// One entry per coalesced request — the sheet shows the origin **once**,
    /// however many times the page asked.
    let methods: [String]
}

struct DpermViewWire: Decodable, Equatable {
    let consent: DpermConsentViewWire?
    /// The connected chip. `nil` is the disconnected view.
    let connectedAddress: String?
    let currentOrigin: String?

    static let empty = DpermViewWire(consent: nil, connectedAddress: nil, currentOrigin: nil)

    var isConnected: Bool { connectedAddress != nil }
}
