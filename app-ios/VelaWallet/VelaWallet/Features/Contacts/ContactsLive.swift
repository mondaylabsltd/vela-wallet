//
//  ContactsLive.swift
//  VelaWallet
//
//  The person's own address book, in the display models the drawn components
//  already consume.
//
//  A **sibling** of `ContactsFixtures`, not a replacement: the galleries and the
//  `VELA_PAGE=contacts-gallery` route keep rendering canon, and every component
//  below this line is unchanged. Ported from
//  `app-web/vela-wallet/src/lib/contacts/live.ts` (spec 024).
//
//  ## Two presentation judgements live here, and only two
//
//  Everything else — which contacts exist, their order, whether one is
//  favourited, which are tombstoned, how a group's members resolve — is the
//  core's, already decided and already tested in Rust.
//
//  1. **Letter sectioning.** The core's list order is authoritative
//     (favourites first, then most recent). The PAGE is an A–Z directory, so
//     rows are grouped by initial and keep the core's relative order inside
//     each letter. That is the same class of work as date-grouping a feed.
//  2. **Search narrowing.** The core has no list-search event — its
//     `matches_query` serves the recipient picker — so filtering the rendered
//     list by the box's text is display-side narrowing of core-ruled rows.
//
//  ## What it refuses to do
//
//  It never invents a name, never sorts, never dedupes, and never renders
//  anything at all before `loaded`. A screen that showed rows while the machine
//  is still unloaded would offer actions the core drops on the floor.
//

import SwiftUI

enum ContactsLive {

    /// The screen before the core has read the stores.
    ///
    /// Real chrome — the title, the search field, the tab bar a person needs to
    /// leave with — over an empty list. Deliberately **not** the C3 empty state:
    /// "你还没有联系人" is a claim about the book, and the book has not been read
    /// yet. Saying it early would tell somebody their address book is gone
    /// (FR-009).
    static func waiting(loc: Loc) -> ContactsHomeModel {
        ContactsHomeModel(
            state: .c1,
            title: loc.t("contacts.title"),
            addLabel: loc.t("contacts.addContact"),
            search: ContactsLabels.search(loc: loc),
            groupsHeader: nil,
            groups: [],
            contactsHeader: nil,
            sections: [],
            indexLetters: ContactsLabels.indexLetters,
            empty: nil,
            searchEmpty: nil,
            tabs: ContactsLabels.tabs(loc: loc),
            reveal: nil,
            sheet: nil,
            textScale: 1
        )
    }

    /// The home screen for a loaded book.
    ///
    /// - Parameter query: what is in the search box, or `nil` when it is idle.
    static func home(
        _ view: ContactsViewWire,
        loc: Loc,
        query: String? = nil
    ) -> ContactsHomeModel {
        let searching = !(query ?? "").trimmingCharacters(in: .whitespaces).isEmpty
        let rows = searching ? narrowed(view.contacts, to: query ?? "") : view.contacts

        var model = ContactsHomeModel(
            state: .c1,
            title: loc.t("contacts.title"),
            addLabel: loc.t("contacts.addContact"),
            search: ContactsLabels.search(loc: loc, query: query),
            groupsHeader: nil,
            groups: [],
            contactsHeader: nil,
            sections: [],
            indexLetters: [],
            empty: nil,
            searchEmpty: nil,
            tabs: ContactsLabels.tabs(loc: loc),
            reveal: nil,
            sheet: nil,
            textScale: 1
        )

        // A book with nothing in it gets the drawn C3 treatment — the invitation
        // to add somebody, not an empty list under a "8 位" header.
        guard !view.contacts.isEmpty else {
            model.empty = EmptyCTAModel(
                title: loc.t("contacts.empty"),
                caption: loc.t("contacts.emptyHint"),
                primary: loc.t("contacts.addContact"),
                secondary: loc.t("contacts.importFile")
            )
            return model
        }

        // Searching with no matches reuses the empty treatment with the
        // no-results wording, which is what the drawing does.
        if searching && rows.isEmpty {
            model.searchEmpty = ContactsFixtures.searchEmpty(loc: loc, query: query ?? "")
            return model
        }

        if !view.groups.isEmpty {
            model.groupsHeader = (loc.t("contacts.sectionGroups"), loc.t("contacts.manage"))
            model.groups = view.groups.map {
                GroupRowModel(
                    name: $0.name,
                    countLabel: ContactsLabels.count(loc, "contacts.groupMembers", $0.members.count)
                )
            }
        }

        model.contactsHeader = (
            loc.t("contacts.sectionContacts"),
            ContactsLabels.count(loc, "contacts.countPeople", rows.count)
        )
        model.sections = sections(rows, groups: view.groups)
        model.indexLetters = ContactsLabels.indexLetters
        model.deleteConfirms = Dictionary(
            uniqueKeysWithValues: model.sections.flatMap(\.contacts)
                .map { ($0.id, ContactsFixtures.deleteConfirm(loc: loc, name: $0.name)) }
        )
        return model
    }

    /// One contact's page.
    ///
    /// The activity block is the honest gap: there is no transaction store
    /// until spec 052, so it renders the drawn "no activity" treatment rather
    /// than a fixture's two rows under a real person's name.
    static func detail(
        _ contact: ContactWire,
        view: ContactsViewWire,
        loc: Loc
    ) -> ContactDetailModel {
        let row = contactModel(contact, groups: view.groups)
        return ContactDetailModel(
            state: .c2,
            contact: row,
            chips: row.groups,
            addChip: loc.t("contacts.sectionGroups"),
            actions: ContactActionsModel(
                send: loc.t("componentsUi.dock.send"),
                receive: loc.t("componentsUi.dock.receive"),
                qr: loc.t("contacts.actionQr")
            ),
            addressLabel: loc.t("contacts.addressLabel"),
            addressLines: AddressText.lines(contact.address),
            copyLabel: loc.t("componentsUi.identiconViewer.copyAddress"),
            copiedLabel: loc.t("componentsUi.identiconViewer.copied"),
            activityTitle: loc.t("contacts.recentActivity"),
            activityAction: loc.t("history.filterAll"),
            // live in 052 — the transaction store.
            activity: [],
            activityEmpty: ContactsFixtures.activityEmpty(loc: loc),
            deleteLabel: loc.t("contacts.deleteContact"),
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            editLabel: loc.t("contacts.edit"),
            sheet: nil,
            textScale: 1
        )
    }

    /// One group's page. Members arrive already resolved by the core, in
    /// membership order, with unsaved addresses synthesised — so a payee is
    /// never silently dropped from a batch send.
    static func group(
        _ group: ContactGroupWire,
        view: ContactsViewWire,
        loc: Loc
    ) -> GroupDetailModel {
        GroupDetailModel(
            state: .c4,
            name: group.name,
            membersLabel: ContactsLabels.count(loc, "contacts.membersCount", group.members.count),
            members: group.members.map { contactModel($0, groups: view.groups) },
            addMemberLabel: loc.t("contacts.addMember"),
            ctaLabel: loc.t("contacts.batchSend"),
            ctaCaption: ContactsLabels.count(loc, "contacts.batchSendHint", group.members.count),
            ctaEnabled: !group.members.isEmpty,
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            moreLabel: loc.t("contacts.manage"),
            // The ⋯ menu is drawn (C6) and its destructive item is the one with
            // a machine behind it; the screen raises the sheet on demand.
            sheet: ContactsFixtures.groupMenu(loc: loc),
            textScale: 1
        )
    }

    // MARK: - Display

    /// What the person calls this contact — their own name wins over a resolved
    /// one, and an unnamed address introduces itself by its short form rather
    /// than as a blank row.
    static func displayName(_ contact: ContactWire) -> String {
        contact.name ?? contact.resolvedName ?? AddressText.short(contact.address)
    }

    private static func contactModel(
        _ contact: ContactWire,
        groups: [ContactGroupWire]
    ) -> ContactModel {
        ContactModel(
            name: displayName(contact),
            addressDisplay: AddressText.short(contact.address),
            // The FULL address is the identicon seed; vela-core normalises it,
            // and lowercasing at the call site would draw a different picture
            // for the same person (spec 003 rule).
            addressFull: contact.address,
            sectionKey: sectionLetter(displayName(contact)),
            groups: groups
                .filter { $0.members.contains { $0.address == contact.address } }
                .map(\.name)
        )
    }

    /// Group by initial, preserving the core's order inside each letter.
    ///
    /// The core's ordering is favourites-first then most-recent, so a letter's
    /// rows come out in that order — which is deliberate: the directory is a way
    /// to FIND somebody, and inside one letter the person you use most should
    /// still be first.
    private static func sections(
        _ contacts: [ContactWire],
        groups: [ContactGroupWire]
    ) -> [ContactSectionModel] {
        var order: [String] = []
        var byLetter: [String: [ContactModel]] = [:]
        for contact in contacts {
            let row = contactModel(contact, groups: groups)
            if byLetter[row.sectionKey] == nil {
                order.append(row.sectionKey)
                byLetter[row.sectionKey] = []
            }
            byLetter[row.sectionKey]?.append(row)
        }
        // Letters ascend; `#` sorts last, which is where the rail puts it.
        return order.sorted { lhs, rhs in
            if lhs == "#" { return false }
            if rhs == "#" { return true }
            return lhs < rhs
        }
        .map { ContactSectionModel(letter: $0, contacts: byLetter[$0] ?? []) }
    }

    /// Which letter a name files under.
    ///
    /// **CJK names are transliterated, not dumped under `#`** — 妈妈 files under
    /// M, 阿豪 under A. That is what the drawing says, and `ContactsFixtures`
    /// pins it: its roster puts 阿豪 in section A and 妈妈 in section M, and
    /// `sectionLetters` is `["A","B","C","D","H","M"]`.
    ///
    /// It is also the only reading that works. The straight
    /// `("A"..."Z").contains` rule — which is what web's `live.ts` does, and
    /// what this function did until a simulator screenshot showed 妈妈 sitting
    /// under `#` — sends **every** Chinese name to one bucket at the bottom of
    /// the rail. For a wallet whose first market writes Chinese names, that is
    /// an A–Z directory with the directory taken out.
    ///
    /// Everything that still has no Latin initial — digits, emoji, an unnamed
    /// address — keeps `#`, which is the bucket the rail always draws last.
    ///
    /// (Recorded as a cross-client divergence: web files these under `#` today.)
    private static func sectionLetter(_ name: String) -> String {
        guard let first = name.trimmingCharacters(in: .whitespaces).first else { return "#" }
        for candidate in [String(first), transliterated(String(first))] {
            guard let initial = candidate?.first else { continue }
            let upper = String(initial).uppercased()
            if ("A"..."Z").contains(upper) { return upper }
        }
        return "#"
    }

    /// 妈 → "mā" → "ma". Latin input passes through unchanged, so the common
    /// case costs one identity transform.
    private static func transliterated(_ character: String) -> String? {
        character
            .applyingTransform(.toLatin, reverse: false)?
            .applyingTransform(.stripDiacritics, reverse: false)
    }

    /// Case-insensitive contains over the name, the resolved name and the
    /// address — narrowing what the core already ruled on, never re-ordering it.
    private static func narrowed(_ contacts: [ContactWire], to query: String) -> [ContactWire] {
        let needle = query.trimmingCharacters(in: .whitespaces).lowercased()
        guard !needle.isEmpty else { return contacts }
        return contacts.filter { contact in
            [contact.name, contact.resolvedName, contact.address]
                .compactMap { $0?.lowercased() }
                .contains { $0.contains(needle) }
        }
    }

}
