//
//  ContactsLabels.swift
//  VelaWallet
//
//  The contacts screen's vocabulary — the parts that are pure `loc.t` and carry
//  no data at all.
//
//  It exists because the screen now has TWO builders: `ContactsFixtures`, which
//  is the gallery's canon, and `ContactsLive`, which renders the person's own
//  book. What the tab bar says and how "3 人" is composed is the same question
//  for both, and answering it twice is how a live screen and its gallery board
//  quietly drift apart.
//
//  Nothing canonical lives here. No names, no addresses, no counts — only the
//  keys, so moving a helper in changed no fixture value and the screenshot
//  sweep proves it.
//

import SwiftUI

enum ContactsLabels {

    /// The full A–Z rail plus `#`, always rendered — letters without a section
    /// jump to the nearest existing one (spec 018 research D4).
    static let indexLetters: [String] =
        (UnicodeScalar("A").value...UnicodeScalar("Z").value).map { String(UnicodeScalar($0)!) } + ["#"]

    static func tabs(loc: Loc) -> TabsModel {
        TabsModel(
            wallet: loc.t("componentsUi.mainNav.wallet"),
            contacts: loc.t("componentsUi.mainNav.contacts"),
            explore: loc.t("componentsUi.mainNav.explore"),
            settings: loc.t("componentsUi.mainNav.settings")
        )
    }

    static func search(loc: Loc, query: String? = nil) -> ContactsSearchModel {
        ContactsSearchModel(
            placeholder: loc.t("contacts.searchPlaceholder"),
            query: query,
            clearLabel: loc.t("contacts.cancel")
        )
    }

    /// `3 人` / `3 位成员` — the corpus has no ICU, so a count is a variable.
    static func count(_ loc: Loc, _ key: String, _ value: Int) -> String {
        loc.t(key, vars: ["count": String(value)])
    }
}
