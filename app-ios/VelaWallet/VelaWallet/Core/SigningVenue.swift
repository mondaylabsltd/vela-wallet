//
//  SigningVenue.swift
//  VelaWallet
//
//  Where a person reviews and signs (spec 102), in Swift.
//
//  Every account has three things, and the core owns all of them:
//
//  - where its key LIVES — this device, a phone or tablet, a USB key
//    (`KeyMethod`, three values, no fourth);
//  - its signing VENUE — Vela's own sheet, or a saved trusted page — chosen per
//    account on this device and changeable without touching a key;
//  - its signing DOMAIN — the RP ID its keys live under (`getvela.app`, or the
//    person's own domain for a wallet made on a self-hosted page).
//
//  This file only mirrors the core's answers (`signingPlan`,
//  `signingVenueChoices`, `SigningPagesCore`'s view) so screens read typed
//  values. Nothing here decides reachability, which venue a signature goes to
//  or which key is used — the rules are `vela_core::signing_venue`'s.
//

import Foundation
import VelaCore

/// `signing_venue::SigningVenue` — `{"type":"in_vela"}` or
/// `{"type":"page","url":…}`.
enum SigningVenueWire: Equatable, Hashable {
    case inVela
    case page(url: String)

    /// The page's address, for a page venue.
    var pageUrl: String? {
        if case .page(let url) = self { return url }
        return nil
    }

    /// The wire object, as the core reads it back (`signing_venue_chosen`,
    /// `signingVenueChoices`' `active_json`).
    var object: [String: Any] {
        switch self {
        case .inVela: ["type": "in_vela"]
        case .page(let url): ["type": "page", "url": url]
        }
    }

    var json: String { CoreJSON.string(object) }

    init?(object: [String: Any]?) {
        switch object?["type"] as? String {
        case "in_vela": self = .inVela
        case "page":
            guard let url = object?["url"] as? String, !url.isEmpty else { return nil }
            self = .page(url: url)
        default: return nil
        }
    }
}

extension SigningVenueWire: Decodable {
    private enum CodingKeys: String, CodingKey { case type, url }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        switch try values.decode(String.self, forKey: .type) {
        case "page": self = .page(url: try values.decodeIfPresent(String.self, forKey: .url) ?? "")
        default: self = .inVela
        }
    }
}

/// `signing_venue::VenueBlock` — why a venue cannot reach an account's keys
/// (R1). The sentence is the corpus's (`key`), with both sides named.
enum VenueBlockWire: Equatable, Hashable {
    /// Vela's own sheet signs only with `getvela.app` keys.
    case appCannotReach(domain: String)
    /// A browser lets a page use only its own site's passkeys.
    case pageOnOtherDomain(pageDomain: String, domain: String)

    /// `VenueBlock::key`.
    var key: String {
        switch self {
        case .appCannotReach: "settings.venue.blockedApp"
        case .pageOnOtherDomain: "settings.venue.blockedPage"
        }
    }

    var vars: [String: String] {
        switch self {
        case .appCannotReach(let domain): ["domain": domain]
        case .pageOnOtherDomain(let pageDomain, let domain): ["pageDomain": pageDomain, "domain": domain]
        }
    }

    func text(_ loc: Loc) -> String { loc.t(key, vars: vars) }
}

extension VenueBlockWire: Decodable {
    private enum CodingKeys: String, CodingKey { case type, domain, pageDomain }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        let domain = try values.decodeIfPresent(String.self, forKey: .domain) ?? ""
        switch try values.decode(String.self, forKey: .type) {
        case "page_on_other_domain":
            self = .pageOnOtherDomain(
                pageDomain: try values.decodeIfPresent(String.self, forKey: .pageDomain) ?? "",
                domain: domain
            )
        default: self = .appCannotReach(domain: domain)
        }
    }
}

/// `signing_venue::KeyRoute` (R5) — which key a signature is pinned to and
/// where it lives. A native ceremony is pinned with it; the page is told it
/// (`key_route_json`) so the browser goes straight to that key.
struct KeyRouteWire: Decodable, Equatable, Hashable {
    /// Hex, no `0x`.
    let credentialId: String
    /// `platform` | `hybrid` | `security_key`.
    let method: String
    let transports: String
    let hints: [String]

    private enum CodingKeys: String, CodingKey { case credentialId, method, transports, hints }

    init(credentialId: String, method: String, transports: String, hints: [String] = []) {
        self.credentialId = credentialId
        self.method = method
        self.transports = transports
        self.hints = hints
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        credentialId = try values.decodeIfPresent(String.self, forKey: .credentialId) ?? ""
        method = try values.decodeIfPresent(String.self, forKey: .method) ?? ""
        transports = try values.decodeIfPresent(String.self, forKey: .transports) ?? ""
        hints = try values.decodeIfPresent([String].self, forKey: .hints) ?? []
    }

    /// The place, typed — `nil` for anything that is not one of the three.
    var place: KeyMethod? { KeyMethod(rawValue: method) }

    /// The route as the core wrote it, for `TrustedSignerInput.key_route_json`.
    var json: String {
        CoreJSON.string([
            "credential_id": credentialId, "method": method,
            "transports": transports, "hints": hints,
        ])
    }
}

/// `signing_venue::SigningPlan` — how an account signs on this device, in one
/// answer (`signingPlan(accountJson:)`).
struct SigningPlanWire: Decodable, Equatable {
    /// The RP ID the account's keys live under.
    let domain: String
    /// Where its transactions and messages are reviewed and signed — already
    /// checked against R1 by the core.
    let venue: SigningVenueWire
    /// Set only when nothing on this device can reach the account's keys. The
    /// shell signs nothing and says this.
    let blocked: VenueBlockWire?
    /// The key this device signs with. `nil` for a record from before the
    /// sign-in key was kept, which signs as it always did.
    let key: KeyRouteWire?
    /// The record's own label for that key ("Confirm with {{key}}" names it,
    /// else its place). Not on the wire: read from the same record.
    var keyName = ""

    private enum CodingKeys: String, CodingKey { case domain, venue, blocked, key }

    init(
        domain: String, venue: SigningVenueWire, blocked: VenueBlockWire? = nil,
        key: KeyRouteWire? = nil, keyName: String = ""
    ) {
        self.domain = domain
        self.venue = venue
        self.blocked = blocked
        self.key = key
        self.keyName = keyName
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        domain = try values.decodeIfPresent(String.self, forKey: .domain) ?? SigningPlanWire.appDomain
        venue = try values.decodeIfPresent(SigningVenueWire.self, forKey: .venue) ?? .inVela
        blocked = try? values.decodeIfPresent(VenueBlockWire.self, forKey: .blocked)
        key = try? values.decodeIfPresent(KeyRouteWire.self, forKey: .key)
    }

    /// `signing_venue::APP_DOMAIN`.
    static let appDomain = "getvela.app"

    /// The account is on Vela's own domain — its keys answer in the app.
    var onAppDomain: Bool { domain.caseInsensitiveCompare(Self.appDomain) == .orderedSame }

    /// The core's plan for a stored account record — `nil` for a record this
    /// build cannot read.
    static func of(accountJson: String) -> SigningPlanWire? {
        guard var plan = signingPlan(accountJson: accountJson).flatMap({
            try? CoreJSON.decoder.decode(SigningPlanWire.self, from: Data($0.utf8))
        }) else { return nil }
        if let id = plan.key?.credentialId,
           let record = try? CoreJSON.object(accountJson),
           let keys = record["keys"] as? [[String: Any]],
           let key = keys.first(where: { ($0["credential_id"] as? String)?.caseInsensitiveCompare(id) == .orderedSame }) {
            plan.keyName = (key["name"] as? String ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        }
        return plan
    }
}

/// `signing_venue::SigningPage` — one saved page.
struct SigningPageWire: Codable, Equatable, Hashable {
    let url: String
    var name: String = ""

    private enum CodingKeys: String, CodingKey { case url, name }

    init(url: String, name: String = "") {
        self.url = url
        self.name = name
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        url = try values.decodeIfPresent(String.self, forKey: .url) ?? ""
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? ""
    }
}

/// `signing_venue::VenueChoice` — one row of "Where you review and sign".
struct VenueChoiceWire: Decodable, Equatable, Identifiable {
    let venue: SigningVenueWire
    /// A saved page's label; empty for Vela's sheet, the official page and an
    /// unnamed page — the screen names those.
    let name: String
    /// The domain whose keys this choice can use.
    let domain: String
    let official: Bool
    /// The account's venue now.
    let active: Bool
    /// Why this choice cannot be used for the account — drawn disabled, with
    /// this reason under it.
    let blocked: VenueBlockWire?

    var id: String { venue.pageUrl ?? "in_vela" }

    private enum CodingKeys: String, CodingKey { case venue, name, domain, official, active, blocked }

    init(venue: SigningVenueWire, name: String, domain: String, official: Bool, active: Bool, blocked: VenueBlockWire?) {
        self.venue = venue
        self.name = name
        self.domain = domain
        self.official = official
        self.active = active
        self.blocked = blocked
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        venue = try values.decode(SigningVenueWire.self, forKey: .venue)
        name = try values.decodeIfPresent(String.self, forKey: .name) ?? ""
        domain = try values.decodeIfPresent(String.self, forKey: .domain) ?? ""
        official = try values.decodeIfPresent(Bool.self, forKey: .official) ?? false
        active = try values.decodeIfPresent(Bool.self, forKey: .active) ?? false
        blocked = try? values.decodeIfPresent(VenueBlockWire.self, forKey: .blocked)
    }

    /// R1 + R2 for an account: every venue it could pick, in the core's order.
    static func choices(
        domain: String, active: SigningVenueWire, saved: [SigningPageWire]
    ) -> [VenueChoiceWire] {
        let savedJson = (try? JSONEncoder().encode(saved)).map { String(decoding: $0, as: UTF8.self) } ?? "[]"
        guard let json = signingVenueChoices(domain: domain, activeJson: active.json, savedJson: savedJson)
        else { return [] }
        return (try? CoreJSON.decoder.decode([VenueChoiceWire].self, from: Data(json.utf8))) ?? []
    }
}

// MARK: - SigningPagesCore (Settings → Signing pages)

/// `app::signing_pages::SigningPageRow`.
struct SigningPageRowWire: Decodable, Equatable, Identifiable {
    let url: String
    /// The person's label; empty ⇒ the screen names it by its host (or, for
    /// the official page, "Official").
    let name: String
    /// The domain whose keys this page can use — drawn on the row, so a
    /// person sees which accounts it can sign for before choosing it.
    let domain: String
    /// Always first, never removed or renamed.
    let official: Bool

    var id: String { url }
}

/// `app::signing_pages::SigningPagesView`.
struct SigningPagesViewWire: Decodable, Equatable {
    /// The official page, then the saved ones in the order they were added.
    let pages: [SigningPageRowWire]
    /// The saved pages as stored — what `signingVenueChoices` takes.
    let saved: [SigningPageWire]
    /// `invalid` | `insecure` | `duplicate` — the last address was refused
    /// and nothing was stored.
    let addError: String?
    /// The list has been read; until then edits are not offered.
    let loaded: Bool

    private enum CodingKeys: String, CodingKey { case pages, saved, addError, loaded }

    init(pages: [SigningPageRowWire], saved: [SigningPageWire], addError: String? = nil, loaded: Bool) {
        self.pages = pages
        self.saved = saved
        self.addError = addError
        self.loaded = loaded
    }

    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        pages = try values.decodeIfPresent([SigningPageRowWire].self, forKey: .pages) ?? []
        saved = try values.decodeIfPresent([SigningPageWire].self, forKey: .saved) ?? []
        addError = try values.decodeIfPresent(String.self, forKey: .addError)
        loaded = try values.decodeIfPresent(Bool.self, forKey: .loaded) ?? false
    }

    /// What the machine says before it has read anything: the official page
    /// alone.
    static var initial: SigningPagesViewWire? {
        (try? SigningPagesCore().view()).flatMap {
            try? CoreJSON.decode(SigningPagesViewWire.self, from: CoreJSON.object($0))
        }
    }

    /// The corpus line for `addError`.
    static func addErrorKey(_ error: String?) -> String? {
        switch error {
        case "invalid": "settings.signing.pageInvalid"
        case "insecure": "settings.signing.pageInsecure"
        case "duplicate": "settings.signing.pageDuplicate"
        default: nil
        }
    }
}

/// How a page is named on screen: the person's label, "Official", or its host.
enum SigningPageNames {
    static func name(url: String, label: String, official: Bool, loc: Loc) -> String {
        if !label.trimmingCharacters(in: .whitespaces).isEmpty { return label }
        if official { return loc.t("settings.signing.pageOfficial") }
        return host(url)
    }

    /// The page's host (and port, which a loopback page carries), or the
    /// address as typed.
    static func host(_ url: String) -> String {
        guard let parsed = URL(string: url), let host = parsed.host, !host.isEmpty else { return url }
        return parsed.port.map { "\(host):\($0)" } ?? host
    }
}
