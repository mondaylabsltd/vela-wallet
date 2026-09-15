//
//  ContactsWire.swift
//  VelaWallet
//
//  The `contacts` machine's view model, in Swift.
//
//  Same role `CoreViews.swift` plays for onboarding, and the same rule: views
//  are `Decodable` through `CoreJSON.decoder` (`.convertFromSnakeCase`), while
//  operations and results stay dictionaries. A view is a flat record screens
//  read, where types earn their keep; an operation is a tagged union the
//  executor switches on anyway.
//
//  Every field name below is `vela-core`'s. Nothing here reshapes a record —
//  the STORED shape is a different alphabet (camelCase, `resolvedName`,
//  `firstSeen`), and translating between them is `ContactsExecutor`'s job and
//  nowhere else's.
//

import Foundation

/// `ContactKind` — EOA / smart-contract account / not yet classified.
///
/// `unknown` is a real answer, not a missing one: the core never guesses, so a
/// recipient it could not reach stays unknown rather than being shown as an
/// EOA (invariant ⑦).
enum ContactKindWire: String, Decodable {
    case eoa
    case account
    case unknown
}

/// `manual` = the person saved this; `auto` = derived from send history.
enum ContactSourceWire: String, Decodable {
    case manual
    case auto
}

/// One entry of the unified book — saved ⊕ history-derived, tombstones already
/// applied, ordered favourites-first then most-recent **by the core**.
struct ContactWire: Decodable, Equatable {
    /// Lowercased. The canonical key everywhere.
    let address: String
    /// What the person called them. Wins over `resolvedName` for display.
    let name: String?
    /// A cached identity name (ENS / Basename / passkey).
    let resolvedName: String?
    /// The source label for that identity — "ENS", "passkey", ".bnb".
    let resolvedSource: String?
    let kind: ContactKindWire
    let favorite: Bool
    let note: String?
    let txCount: Int
    /// Epoch ms. `Double`, because the core sends `f64` and an `Int` decode
    /// would fail on a value with a fractional part.
    let lastUsedMs: Double
    let firstSeenMs: Double
    let source: ContactSourceWire
}

/// A group with its members already resolved to contacts, in membership order.
///
/// A member with no saved contact arrives as a synthesised `auto` entry rather
/// than being dropped, so sending to a group never silently loses a payee.
struct ContactGroupWire: Decodable, Equatable {
    let id: String
    let name: String
    let color: String?
    let members: [ContactWire]
}

/// What the last import did. Counts only — an invalid row is counted and
/// discarded, never stored.
struct ContactImportReportWire: Decodable, Equatable {
    let added: Int
    let skipped: Int
    let invalid: Int
    let groupsCreated: Int
}

/// Why a file could not be read AT ALL — distinct from rows that failed. The
/// core refuses the whole file before any write, so nothing was half-imported.
///
/// Tagged, not bare: the core serialises it as `{"type": "malformed_json"}`,
/// and reading it as a string is the mistake the drift gate exists to catch.
struct ContactImportFailureWire: Decodable, Equatable {
    enum Reason: String, Decodable, Equatable {
        case malformedJson = "malformed_json"
        case noAddressColumn = "no_address_column"
        case empty
        case unknownGroup = "unknown_group"
        /// A variant this build has not heard of. It still means "the file was
        /// refused", which is the half the screen acts on.
        case unrecognised
    }

    let reason: Reason

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let raw = try container.decode(String.self, forKey: .type)
        reason = Reason(rawValue: raw) ?? .unrecognised
    }

    private enum CodingKeys: String, CodingKey { case type }
}

/// A file the core has written for the shell to hand over. One-shot: it sits
/// in the view until `export_taken`.
struct ContactExportFileWire: Decodable, Equatable {
    let filename: String
    let mime: String
    let content: String
    /// How many contacts the file carries — the shell's confirmation line.
    let contacts: Int
}

/// The A–Z index, as the core computed it. The shell never re-sorts: the
/// letter a name files under is a locale decision and the core owns it.
struct ContactSectionWire: Decodable, Equatable {
    let letter: String
    /// Lowercased addresses, keys into `ContactsViewWire.contacts`.
    let addresses: [String]
}

/// The trust line for the recipient currently on screen.
struct ContactRecipientWire: Decodable, Equatable {
    let address: String
    let saved: Bool
    /// Saved **and** starred — the only state that earns the green check. A
    /// poisoned look-alike address is never a starred contact.
    let verified: Bool
    let displayName: String?
    let identity: ContactIdentityWire?
    let kind: ContactKindWire
    /// `true` = bytecode present; `false` = an EOA; `nil` = unknown or
    /// unreachable. Never a false alarm.
    let isContract: Bool?
    /// No prior outgoing send to this address in local history — the
    /// address-poisoning tell.
    let firstInteraction: Bool
}

struct ContactIdentityWire: Decodable, Equatable {
    let name: String
    let source: String
}

/// The whole machine's view.
struct ContactsViewWire: Decodable, Equatable {
    /// The three stores have been read. **Not a spinner flag** — the core drops
    /// every mutation that arrives before it, so a screen that renders as if
    /// loaded would offer actions the machine will discard.
    let loaded: Bool
    let contacts: [ContactWire]
    let sections: [ContactSectionWire]
    let groups: [ContactGroupWire]
    let lastImport: ContactImportReportWire?
    /// The file could not be read at all. `lastImport` and this are the two
    /// outcomes of one pick, and they are never both set.
    let importFailure: ContactImportFailureWire?
    let export: ContactExportFileWire?
    let recipient: ContactRecipientWire?
}
