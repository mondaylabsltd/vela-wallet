//
//  ContactsFixtures.swift
//  VelaWallet
//
//  Canonical contacts fixtures (spec 018, data-model.md — the single canon
//  all four platforms port; web reference: src/lib/contacts/fixtures.ts).
//  Names, addresses, amounts and dates are verbatim data (FR-012); every
//  label resolves through the corpus keys in contracts/i18n-keys.md.
//  Pure data + assembly: no fetching, no sorting, no formatting rules.
//
//  Identicon seeds are the FULL addresses; they pass through vela-core's
//  `normalize_seed` inside IdenticonAvatar and are never lowercased here
//  (spec 003 rule / FR-006). Only Alice's full address appears in a mock;
//  the other eight are pinned inventions whose first/last four hex chars
//  match the mock's truncated display (research.md D7).
//

import SwiftUI

enum ContactsFixtures {
    // MARK: - Canon

    struct ContactCanon {
        let name: String
        let addressDisplay: String
        let addressFull: String
        let section: String
        var groups: [String] = []
    }

    /// The eight-contact roster (data-model.md §Contacts).
    static let roster: [ContactCanon] = [
        ContactCanon(name: "Alice", addressDisplay: "0x9F3c…21aE",
                     addressFull: "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE",
                     section: "A", groups: [familyGroupName]),
        ContactCanon(name: "阿豪", addressDisplay: "0x77Bd…4F02",
                     addressFull: "0x77Bd59A302cC93D23dB0d0BA6a45C6830EF74F02",
                     section: "A"),
        ContactCanon(name: "Bartholomew Vanderbilt-Konstantinopoulos.eth", addressDisplay: "0x31c9…E77a",
                     addressFull: "0x31c9A100517d2436E9E1350D383A7d0aAeC1E77a",
                     section: "B"),
        ContactCanon(name: "Bob · 泵泵", addressDisplay: "0x44Aa…9C21",
                     addressFull: "0x44AaF19cE84f22101b5D6cbA918B92DcA5f19C21",
                     section: "B"),
        ContactCanon(name: "Charlie", addressDisplay: "0x5eF0…3a9C",
                     addressFull: "0x5eF0FF25a1A24E5cCb2a6D939B87F5DAb2003a9C",
                     section: "C"),
        ContactCanon(name: "DAO 金库", addressDisplay: "0xF00d…C0de",
                     addressFull: "0xF00dBaBe8712004343cD00926Ab004D6C042C0de",
                     section: "D"),
        ContactCanon(name: "hold on", addressDisplay: "0xCafe…F00d",
                     addressFull: "0xCafe9078B1c2A04d33Ff21B0BC934eB8A812F00d",
                     section: "H"),
        ContactCanon(name: "妈妈", addressDisplay: "0x88Ce…12aB",
                     addressFull: "0x88Ce02FdB0e50D9C21e33c0F9B58a3E38f7612aB",
                     section: "M", groups: [familyGroupName]),
    ]

    /// Group-only member (the recorded DC1 mock inconsistency).
    static let cousin = ContactCanon(
        name: "表弟", addressDisplay: "0xA1c3…88dD",
        addressFull: "0xA1c3D3d7085B90AF14E5d21C86e6dB49F30a88dD",
        section: "B", groups: [familyGroupName]
    )

    static let familyGroupName = "家人"
    static let workGroupName = "工作"
    static let exchangeGroupName = "交易所"

    /// 家人 members in mock order (C4).
    static var familyMembers: [ContactCanon] { [roster[7], cousin, roster[0]] }

    static let familyCount = 3
    static let workCount = 5
    static let exchangeCount = 2
    static let totalCount = 8

    /// Letter sections present in the roster, in order.
    static let sectionLetters = ["A", "B", "C", "D", "H", "M"]

    /// The rail always renders the full alphabet plus `#` (research D4);
    /// letters without a section jump to the nearest existing one.
    ///
    /// Lives in `ContactsLabels` since spec 050, because the live builder
    /// renders the same rail; the value is unchanged.
    static let indexLetters: [String] = ContactsLabels.indexLetters

    /// C1f search-active query (pre-filtered by the fixture layer).
    static let searchQuery = "Ali"

    /// Alice's mono address block — exactly the mock's two lines.
    static let aliceAddressLines = ["0x9F3cA71b04E82f5C55d9", "B21aE00734F8Dd8021aE"]

    /// Identicon-board seeds (SC-003 parity set): the 8+1 canon addresses
    /// plus the empty seed that exercises the placeholder path.
    static var identiconBoardSeeds: [String] {
        roster.map(\.addressFull) + [cousin.addressFull, ""]
    }

    // MARK: - Activity (Alice detail — mirrors the 015 wallet entry)

    private struct ActivityCanon {
        let kind: ActivityKind
        /// nil → the literal day text is used verbatim.
        let dayKey: String?
        let dayLiteral: String
        let trailing: String
        let amount: String
        let unit: String
        let positive: Bool
        let badgeColor: Color
    }

    private static let aliceActivity: [ActivityCanon] = [
        ActivityCanon(kind: .received, dayKey: "componentsUi.dayGroup.yesterday", dayLiteral: "",
                      trailing: "20:15 · Ethereum", amount: "+50", unit: "USDC",
                      positive: true, badgeColor: ChainPalette.ethereum),
        ActivityCanon(kind: .sent, dayKey: nil, dayLiteral: "8 月 5 日",
                      trailing: "· Arbitrum", amount: "−0.2", unit: "ETH",
                      positive: false, badgeColor: ChainPalette.arbitrum),
    ]

    private static func activityRow(_ canon: ActivityCanon, loc: Loc) -> ActivityRowModel {
        let day = canon.dayKey.map { loc.t($0) } ?? canon.dayLiteral
        let title = canon.kind == .received
            ? loc.t("history.labelReceived")
            : loc.t("history.labelSent")
        return ActivityRowModel(
            kind: canon.kind,
            title: title,
            subtitle: "\(day) \(canon.trailing)",
            amount: canon.amount,
            unit: canon.unit,
            positive: canon.positive,
            masked: false,
            badgeColor: canon.badgeColor
        )
    }

    // MARK: - Assembly helpers

    private static func contact(_ canon: ContactCanon) -> ContactModel {
        ContactModel(
            name: canon.name,
            addressDisplay: canon.addressDisplay,
            addressFull: canon.addressFull,
            sectionKey: canon.section,
            groups: canon.groups
        )
    }

    private static func sections(_ canons: [ContactCanon]) -> [ContactSectionModel] {
        var out: [(letter: String, contacts: [ContactModel])] = []
        for canon in canons {
            let row = contact(canon)
            if let last = out.indices.last, out[last].letter == canon.section {
                out[last].contacts.append(row)
            } else {
                out.append((canon.section, [row]))
            }
        }
        return out.map { ContactSectionModel(letter: $0.letter, contacts: $0.contacts) }
    }

    private static func groups(loc: Loc) -> [GroupRowModel] {
        [(familyGroupName, familyCount), (workGroupName, workCount), (exchangeGroupName, exchangeCount)]
            .map { GroupRowModel(name: $0.0, countLabel: count(loc, "contacts.groupMembers", $0.1)) }
    }

    // Shared with `ContactsLive` since spec 050 — one implementation of what
    // the tab bar says and how a count is composed, so a live screen and its
    // gallery board cannot drift. Same keys, same output.

    private static func count(_ loc: Loc, _ key: String, _ value: Int) -> String {
        ContactsLabels.count(loc, key, value)
    }

    private static func tabs(loc: Loc) -> TabsModel {
        ContactsLabels.tabs(loc: loc)
    }

    private static func search(loc: Loc, query: String? = nil) -> ContactsSearchModel {
        ContactsLabels.search(loc: loc, query: query)
    }

    // MARK: - Menus (data-model.md §Menus)

    static func addMenu(loc: Loc) -> ActionMenuModel {
        ActionMenuModel(
            items: [
                MenuItemModel(icon: .userRoundPlus, label: loc.t("contacts.addTitle")),
                MenuItemModel(icon: .download, label: loc.t("contacts.importFile")),
                MenuItemModel(icon: .upload, label: loc.t("contacts.exportTitle")),
            ],
            cancel: loc.t("contacts.cancel")
        )
    }

    static func groupMenu(loc: Loc) -> ActionMenuModel {
        ActionMenuModel(
            items: [
                MenuItemModel(icon: .pencil, label: loc.t("contacts.groupEdit")),
                MenuItemModel(icon: .download, label: loc.t("contacts.importGroup")),
                MenuItemModel(icon: .upload, label: loc.t("contacts.exportGroup")),
                MenuItemModel(icon: .trash2, label: loc.t("contacts.groupDelete"),
                              destructive: true, dividerAbove: true),
            ],
            cancel: loc.t("contacts.cancel")
        )
    }

    static func deleteConfirm(loc: Loc, name: String) -> ActionMenuModel {
        ActionMenuModel(
            title: loc.t("contacts.deleteTitle"),
            body: loc.t("contacts.deleteBody", vars: ["name": name]),
            items: [
                MenuItemModel(icon: .trash2, label: loc.t("contacts.delete"), destructive: true),
            ],
            cancel: loc.t("contacts.cancel")
        )
    }

    // MARK: - Screen builders

    /// Assemble the mobile scene for one C-state (FR-002). `messages` is
    /// injected so tests can stub the locale (015 convention).
    static func buildMobileState(_ state: ContactsStateId, loc: Loc) -> ContactsScene {
        switch state {
        case .c1: .home(home(state, loc: loc))
        case .c1s: .home(home(state, loc: loc))
        case .c1f: .home(home(state, loc: loc))
        case .c3: .home(home(state, loc: loc))
        case .c5: .home(home(state, loc: loc))
        case .c2, .c2s: .detail(detail(state, loc: loc))
        case .c4, .c6: .group(groupDetail(state, loc: loc))
        }
    }

    private static func home(_ state: ContactsStateId, loc: Loc) -> ContactsHomeModel {
        let empty = state == .c3
        let filtered = state == .c1f
        let visible = filtered ? Array(roster.prefix(1)) : roster

        var model = ContactsHomeModel(
            state: state,
            title: loc.t("contacts.title"),
            addLabel: loc.t("contacts.addContact"),
            search: search(loc: loc, query: filtered ? searchQuery : nil),
            groupsHeader: nil,
            groups: [],
            contactsHeader: nil,
            sections: [],
            indexLetters: [],
            empty: nil,
            searchEmpty: nil,
            tabs: tabs(loc: loc),
            reveal: nil,
            sheet: nil,
            textScale: 1
        )

        if empty {
            model.empty = EmptyCTAModel(
                title: loc.t("contacts.empty"),
                caption: loc.t("contacts.emptyHint"),
                primary: loc.t("contacts.addContact"),
                secondary: loc.t("contacts.importFile")
            )
            return model
        }

        model.groupsHeader = (loc.t("contacts.sectionGroups"), loc.t("contacts.manage"))
        model.groups = groups(loc: loc)
        model.contactsHeader = (
            loc.t("contacts.sectionContacts"),
            count(loc, "contacts.countPeople", filtered ? visible.count : totalCount)
        )
        model.sections = sections(visible)
        model.indexLetters = indexLetters
        model.deleteConfirms = Dictionary(
            uniqueKeysWithValues: model.sections.flatMap(\.contacts)
                .map { ($0.id, deleteConfirm(loc: loc, name: $0.name)) }
        )

        if state == .c1s, let target = model.sections.first?.contacts.last {
            // 阿豪 — the second row of section A (data-model C1s).
            model.reveal = SwipeRevealModel(
                contactId: target.id,
                sendLabel: loc.t("componentsUi.dock.send"),
                deleteLabel: loc.t("contacts.delete")
            )
        }
        if state == .c5 {
            model.sheet = addMenu(loc: loc)
        }
        return model
    }

    private static func detail(_ state: ContactsStateId, loc: Loc) -> ContactDetailModel {
        let alice = contact(roster[0])
        return ContactDetailModel(
            state: state,
            contact: alice,
            chips: alice.groups,
            addChip: loc.t("contacts.sectionGroups"),
            actions: ContactActionsModel(
                send: loc.t("componentsUi.dock.send"),
                receive: loc.t("componentsUi.dock.receive"),
                qr: loc.t("contacts.actionQr")
            ),
            addressLabel: loc.t("contacts.addressLabel"),
            addressLines: aliceAddressLines,
            copyLabel: loc.t("componentsUi.identiconViewer.copyAddress"),
            copiedLabel: loc.t("componentsUi.identiconViewer.copied"),
            activityTitle: loc.t("contacts.recentActivity"),
            activityAction: loc.t("history.filterAll"),
            activity: aliceActivity.map { activityRow($0, loc: loc) },
            activityEmpty: nil,
            deleteLabel: loc.t("contacts.deleteContact"),
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            editLabel: loc.t("contacts.edit"),
            sheet: state == .c2s ? deleteConfirm(loc: loc, name: alice.name) : nil,
            textScale: 1
        )
    }

    private static func groupDetail(_ state: ContactsStateId, loc: Loc) -> GroupDetailModel {
        let members = familyMembers.map(contact)
        return GroupDetailModel(
            state: state,
            name: familyGroupName,
            membersLabel: count(loc, "contacts.membersCount", members.count),
            members: members,
            addMemberLabel: loc.t("contacts.addMember"),
            ctaLabel: loc.t("contacts.batchSend"),
            ctaCaption: count(loc, "contacts.batchSendHint", members.count),
            ctaEnabled: !members.isEmpty,
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            moreLabel: loc.t("contacts.manage"),
            sheet: state == .c6 ? groupMenu(loc: loc) : nil,
            textScale: 1
        )
    }

    // MARK: - Component-board extras (gallery only)

    /// Search-with-no-matches treatment (spec edge case — reuses the
    /// empty component with `contacts.noResults`).
    static func searchEmpty(loc: Loc, query: String) -> EmptyCTAModel {
        EmptyCTAModel(
            title: loc.t("contacts.noResults", vars: ["query": query]),
            caption: loc.t("contacts.emptyHint"),
            primary: loc.t("contacts.addContact"),
            secondary: loc.t("contacts.importFile")
        )
    }

    /// Contact with no activity (spec edge case — reuses WalletEmptyState).
    static func activityEmpty(loc: Loc) -> SectionEmptyModel {
        SectionEmptyModel(
            title: loc.t("home.emptyNoActivity"),
            caption: loc.t("home.emptySubtitle")
        )
    }

    /// Empty group (0 members) — pinned CTA disabled (spec edge case).
    static func emptyGroup(loc: Loc) -> GroupDetailModel {
        GroupDetailModel(
            state: .c4,
            name: familyGroupName,
            membersLabel: count(loc, "contacts.membersCount", 0),
            members: [],
            addMemberLabel: loc.t("contacts.addMember"),
            ctaLabel: loc.t("contacts.batchSend"),
            ctaCaption: count(loc, "contacts.batchSendHint", 0),
            ctaEnabled: false,
            backLabel: loc.t("componentsUi.mainNav.contacts"),
            moreLabel: loc.t("contacts.manage"),
            sheet: nil,
            textScale: 1
        )
    }
}
