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
        query: String? = nil,
        form: ContactDraft? = nil
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

        // The add form rides the home, over whichever list is behind it —
        // including the empty one, which is the state most likely to raise it.
        model.form = form.map { formModel($0, loc: loc) }
        // The + raises C5's three choices. Live it was nil, so the button
        // presented an empty sheet: the drawn menu is the live one.
        model.sheet = ContactsFixtures.addMenu(loc: loc)
        model.notice = importNotice(view, loc: loc)

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
            // 新建分组, as Android's header says. It read 管理 and led nowhere
            // — a header action naming a page this client does not have.
            // Recorded in 058's results as a label changed to match the phone
            // beside it, not a screen invented here.
            model.groupsHeader = (loc.t("contacts.sectionGroups"), loc.t("contacts.groupNew"))
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

    /// What an import had to say — the report, or the refusal.
    ///
    /// A file that was read and a file that could not be are two different
    /// sentences, and the core keeps them apart: `lastImport` counts rows,
    /// `importFailure` says the whole file was refused before ANY write. A
    /// screen that collapsed them would tell somebody "0 added" about a file
    /// nothing had even parsed.
    static func importNotice(_ view: ContactsViewWire, loc: Loc) -> FlowAlertModel? {
        if view.importFailure != nil {
            return FlowAlertModel(
                title: loc.t("contacts.importFailTitle"),
                message: loc.t("contacts.importFailBody")
            )
        }
        guard let report = view.lastImport else { return nil }
        var message = loc.t("contacts.importDoneBody", vars: [
            "added": String(report.added), "skipped": String(report.skipped),
        ])
        // The invalid count is a SECOND sentence and only when there is one:
        // "另有 0 条地址无效" is noise on a clean import.
        if report.invalid > 0 {
            message += " " + loc.t("contacts.importDoneInvalid", vars: [
                "invalid": String(report.invalid),
            ])
        }
        return FlowAlertModel(title: loc.t("contacts.importDoneTitle"), message: message)
    }

    /// One contact's page.
    ///
    /// The activity block is **this device's own record** of what passed
    /// between the two of you — the same local store the feed reads, narrowed
    /// to this address. It is not a history of the address: a hundred-block
    /// scan and a local store are all this wallet has, and claiming more would
    /// be inventing an indexer.
    static func detail(
        _ contact: ContactWire,
        view: ContactsViewWire,
        records: [[String: Any]] = [],
        loc: Loc,
        form: ContactDraft? = nil,
        groupPick: Set<String>? = nil
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
            activity: activityRows(with: contact.address, records: records, loc: loc),
            activityEmpty: ContactsFixtures.activityEmpty(loc: loc),
            deleteLabel: loc.t("contacts.deleteContact"),
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            editLabel: loc.t("contacts.edit"),
            // The star, always drawn on a page that has one — lit or not. A
            // control that appears only when it is already on is a control
            // nobody can turn on.
            favourite: FavouriteControlModel(
                on: contact.favorite,
                label: loc.t("contacts.sectionFavorites")
            ),
            // What the core found out about THIS address, and only when it was
            // asked about this one: a projection left over from the previous
            // recipient would tag the wrong person.
            inspection: inspection(view.recipient, of: contact.address, loc: loc),
            // 删除联系人's second confirmation. Drawn as C2s; live it was `nil`,
            // so the button raised an empty sheet and the contact stayed.
            sheet: ContactsFixtures.deleteConfirm(loc: loc, name: displayName(contact)),
            form: form.map { formModel($0, loc: loc) },
            groupPick: groupPick.map { picked in
                MultiPickModel(
                    title: loc.t("contacts.sectionGroups"),
                    rows: view.groups.map { group in
                        MultiPickRowModel(
                            id: group.id,
                            title: group.name,
                            subtitle: ContactsLabels.count(
                                loc, "contacts.groupMembers", group.members.count
                            ),
                            identiconSeed: nil,
                            picked: picked.contains(group.id)
                        )
                    },
                    emptyText: loc.t("contacts.groupNoContacts"),
                    save: loc.t("contacts.save"),
                    cancel: loc.t("contacts.cancel")
                )
            },
            textScale: 1
        )
    }

    /// Which groups currently hold this address — the set the picker opens on.
    ///
    /// Read from the core's own membership rather than from the chips on
    /// screen: the chips are display names, and two groups may share one.
    static func groupsHolding(_ address: String, in view: ContactsViewWire) -> Set<String> {
        Set(view.groups.filter { group in
            group.members.contains { $0.address.caseInsensitiveCompare(address) == .orderedSame }
        }.map(\.id))
    }

    /// The core's reading of an address, as two neutral tags.
    ///
    /// Neutral is the whole point: "a contract wallet" and "never paid before"
    /// are facts, not warnings, and dressing either as a warning would teach
    /// people to ignore the one that is.
    private static func inspection(
        _ recipient: ContactRecipientWire?, of address: String, loc: Loc
    ) -> ContactInspectionModel? {
        guard let recipient,
              recipient.address.caseInsensitiveCompare(address) == .orderedSame
        else { return nil }
        var model = ContactInspectionModel()
        // `nil` is "unknown or unreachable" and says nothing — never a false
        // alarm, and never a false reassurance either.
        if recipient.isContract == true {
            model.tag = loc.t("componentsUi.signing.contractTag")
        } else if recipient.isContract == false {
            model.tag = loc.t("componentsUi.signing.walletTag")
        }
        if recipient.firstInteraction {
            model.firstTime = loc.t("componentsUi.signing.firstTimeTagNeutral")
        }
        return (model.tag == nil && model.firstTime == nil) ? nil : model
    }

    /// What is in the form right now, as the shell holds it. The core owns the
    /// contact; this is the two strings being typed at it.
    struct ContactDraft {
        /// The address being edited, or `nil` for a new contact.
        var editing: String?
        var name: String
        var address: String
        /// The core's refusal, once there is something to refuse.
        var error: String?

        var isEdit: Bool { editing != nil }
    }

    /// C7 / C8, live.
    ///
    /// **Save is gated on the address alone**, because a contact with no name
    /// is a legitimate saved address and a contact with no address is nothing
    /// at all. The shape check here is the shell's own cheap one; the CORE
    /// still validates and can refuse — which is what `error` shows.
    static func formModel(_ draft: ContactDraft, loc: Loc) -> ContactFormModel {
        ContactFormModel(
            title: loc.t(draft.isEdit ? "contacts.editTitle" : "contacts.addTitle"),
            nameLabel: loc.t("contacts.nameLabel"),
            namePlaceholder: loc.t("contacts.namePlaceholder"),
            addressLabel: loc.t("contacts.addressLabel"),
            addressPlaceholder: loc.t("contacts.addressPlaceholder"),
            name: draft.name,
            address: draft.address,
            error: draft.error,
            save: loc.t("contacts.save"),
            cancel: loc.t("contacts.cancel"),
            saveEnabled: draft.isEdit || isAddress(draft.address),
            addressLocked: draft.isEdit
        )
    }

    /// The shell's cheap shape check — 0x and forty hex digits. Not a
    /// substitute for the core's judgement, which is what actually decides
    /// whether a contact may be saved; this only keeps the button from
    /// offering to save an obviously empty field.
    static func isAddress(_ value: String) -> Bool {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        guard trimmed.count == 42, trimmed.hasPrefix("0x") else { return false }
        return trimmed.dropFirst(2).allSatisfy(\.isHexDigit)
    }

    /// One group's page. Members arrive already resolved by the core, in
    /// membership order, with unsaved addresses synthesised — so a payee is
    /// never silently dropped from a batch send.
    static func group(
        _ group: ContactGroupWire,
        view: ContactsViewWire,
        loc: Loc,
        memberPick: Set<String>? = nil
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
            memberPick: memberPick.map { picked in
                MultiPickModel(
                    title: loc.t("contacts.addMember"),
                    // Every SAVED contact is a candidate. A member with no
                    // saved contact is synthesised into the group's own list by
                    // the core, and offering to "add" them again would be
                    // offering a row that is already there.
                    rows: view.contacts.map { contact in
                        MultiPickRowModel(
                            id: contact.address,
                            title: displayName(contact),
                            subtitle: AddressText.short(contact.address),
                            identiconSeed: contact.address,
                            picked: picked.contains(contact.address)
                        )
                    },
                    emptyText: loc.t("contacts.groupNoContacts"),
                    save: loc.t("contacts.save"),
                    cancel: loc.t("contacts.cancel")
                )
            },
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

    /// What this device recorded between the wallet and one address.
    ///
    /// Both directions: a send TO them and a receipt FROM them are equally
    /// "what passed between us", and a page that showed only one half would
    /// read as a ledger with a missing column.
    static func activityRows(
        with address: String, records: [[String: Any]], loc: Loc
    ) -> [ActivityRowModel] {
        let wanted = address.lowercased()
        return records.compactMap { record -> ActivityRowModel? in
            let to = (record["to"] as? String)?.lowercased()
            let from = (record["from"] as? String)?.lowercased()
            let outgoing = to == wanted
            guard outgoing || from == wanted else { return nil }
            let symbol = record["symbol"] as? String ?? ""
            let amount = record["value"] as? String ?? ""
            guard !amount.isEmpty else { return nil }
            let seconds = (record["timestamp"] as? NSNumber)?.doubleValue ?? 0
            return ActivityRowModel(
                kind: outgoing ? .sent : .received,
                title: loc.t(outgoing ? "history.labelSent" : "history.labelReceived"),
                subtitle: Self.day(seconds, loc: loc),
                amount: (outgoing ? "−" : "+") + SendLive.trim(amount),
                unit: symbol,
                positive: !outgoing,
                masked: false,
                badgeColor: SendLive.chainColor(
                    (record["chainId"] as? NSNumber)?.intValue ?? 0
                )
            )
        }
    }

    /// The day a record belongs to, in the device's own timezone — the same
    /// grouping key the feed uses, because a record written at 23:30 local
    /// heads its own day.
    private static func day(_ seconds: Double, loc: Loc) -> String {
        let date = Date(timeIntervalSince1970: seconds)
        var calendar = Calendar(identifier: .gregorian)
        calendar.timeZone = .current
        if calendar.isDateInToday(date) { return loc.t("time.today") }
        if calendar.isDateInYesterday(date) { return loc.t("time.yesterday") }
        // The person's own preset (spec 056), like every other date this app
        // prints.
        return Formats.date(date)
    }

}
