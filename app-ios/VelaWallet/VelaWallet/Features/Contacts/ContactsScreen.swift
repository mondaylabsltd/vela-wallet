//
//  ContactsScreen.swift
//  VelaWallet
//
//  The mobile contacts home (spec 018 FR-002, mocks C1 / C1s / C1f / C3 /
//  C5): page header, search field, 分组 section, A–Z sectioned contact list
//  with the index rail, and the tab bar with 通讯录 selected. Screens
//  compose components, never re-implement them (FR-001) — the section
//  headers, empty artwork, identicon avatars and tab bar are the spec-015
//  components.
//
//  Everything renders from a fixture-built ContactsHomeModel; the index
//  rail drives a ScrollViewReader jump, and every tap is an action sink.
//

import SwiftUI

struct ContactsScreen: View {
    @Environment(\.theme) private var theme

    let model: ContactsHomeModel
    var onOpenContact: (ContactModel) -> Void = { _ in }
    var onOpenGroup: (GroupRowModel) -> Void = { _ in }
    /// Leaving 通讯录. Without it this screen is a place a person can reach and
    /// not get out of — which is what the tab bar looked like before spec 050.
    var onSelectTab: (WalletTab) -> Void = { _ in }
    /// The row swipe's 删除, **after** the drawn second confirmation. Carries
    /// the full address rather than the row model, because the address is the
    /// core's key and the row's `id` is a `UUID()` minted for SwiftUI.
    var onDelete: (String) -> Void = { _ in }
    /// 添加联系人 — the header's +, and the empty state's CTA.
    var onAdd: () -> Void = {}
    /// 从文件导入, the menu's second row.
    var onImport: () -> Void = {}
    /// 导出, the menu's third.
    var onExport: () -> Void = {}
    /// The live search field. Absent in the gallery.
    var searchText: Binding<String>?
    var onClearSearch: () -> Void = {}
    /// The import report was read. The core's one-shot leaves the view on it.
    var onAcknowledge: () -> Void = {}
    /// The live form's fields and CTA. Absent in the gallery, where the sheet
    /// is a picture.
    var formName: Binding<String>?
    var formAddress: Binding<String>?
    var onSaveForm: () -> Void = {}
    var onCancelForm: () -> Void = {}

    /// Whether the + menu (or a row's delete confirm) is up. The FORM's
    /// presence is the core's answer and needs no flag of its own.
    @State private var menuShown = false
    @State private var confirming: ContactModel?

    /// What the one sheet is showing. The form outranks the menu: a form is
    /// open because somebody chose something in the menu.
    private enum Presented {
        case menu(ActionMenuModel)
        case form(ContactFormModel)
    }

    private var presented: Presented? {
        if let form = model.form { return .form(form) }
        if menuShown, let menu = presentedSheet { return .menu(menu) }
        return nil
    }

    private func dismissSheet() {
        menuShown = false
        confirming = nil
        if model.form != nil { onCancelForm() }
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            header
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s8)
            ContactsSearchField(model: model.search, onClear: onClearSearch, text: searchText)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s16)

            listArea

            WalletTabBar(tabs: model.tabs, selected: .contacts, onSelect: onSelectTab)
        }
        .background(theme.bgBase.ignoresSafeArea())
        .walletTextScale(model.textScale)
        // ONE sheet, whose CONTENT changes — never two `.sheet` modifiers on
        // one view.
        //
        // Adding a second one is what stopped the contact rows responding to
        // taps at all: the + button still worked, the list did not, and the
        // screen looked exactly like a list whose rows were never wired. iOS
        // has punished stacked modals in this app before (2026-07-06, the
        // invisible funding sheet); the rule is the same both times.
        .sheet(isPresented: Binding(
            get: { presented != nil },
            set: { if !$0 { dismissSheet() } }
        )) {
            switch presented {
            case .menu(let menu):
                ActionMenuSheet(model: menu, onItem: { item in confirm(item) },
                                onCancel: { dismissSheet() })
                    .walletTextScale(model.textScale)
            case .form(let form):
                ContactFormSheet(
                    model: form,
                    nameText: formName,
                    addressText: formAddress,
                    onSave: onSaveForm,
                    onCancel: { dismissSheet() }
                )
                .walletTextScale(model.textScale)
            case nil:
                EmptyView()
            }
        }
        .onAppear {
            // C5 is the state that opens WITH the menu up. Every other state
            // has the same menu available behind +, and starts with it closed.
            //
            // Keying on `sheet != nil` was right while only the C5 FIXTURE
            // carried one; the moment the live home carried the menu too, the
            // address book opened with a sheet nobody had asked for.
            menuShown = model.state == .c5
        }
        .onChange(of: confirming?.id) { _, _ in menuShown = confirming != nil || menuShown }
        .alert(
            model.notice?.title ?? "",
            isPresented: Binding(
                get: { model.notice != nil },
                set: { if !$0 { onAcknowledge() } }
            )
        ) {
            // The same system word the flow host's alerts use.
            Button("OK") { onAcknowledge() }
        } message: {
            Text(verbatim: model.notice?.message ?? "")
        }
    }

    /// The fixture sheet (C5) or the pre-resolved delete confirm raised by
    /// a row swipe — both arrive as display-ready models.
    private var presentedSheet: ActionMenuModel? {
        if let confirming { return model.deleteConfirms[confirming.id] }
        return model.sheet
    }

    /// A tap inside the presented sheet.
    ///
    /// Only the delete confirm has anywhere to go: the add/import/export menu
    /// (C5) is a picture of three choices whose destinations nothing has drawn,
    /// so its items dismiss rather than pretending to act. Routing them
    /// somewhere invented would be worse than the honest nothing.
    private func confirm(_ item: MenuItemModel) {
        defer { menuShown = false; confirming = nil }
        if let target = confirming {
            guard item.destructive else { return }
            onDelete(target.addressFull)
            return
        }
        // The add/import/export menu (C5). Its three rows shipped dismissing —
        // honest at the time, because nothing was drawn behind them; each one
        // now has somewhere to go.
        guard let index = model.sheet?.items.firstIndex(where: { $0.id == item.id })
        else { return }
        switch index {
        case 0: onAdd()
        case 1: onImport()
        default: onExport()
        }
    }

    // MARK: - Header (large title + add button)

    private var header: some View {
        HStack(alignment: .center, spacing: Tokens.Space.s12) {
            Text(verbatim: model.title)
                .typeRole(Typography.pageTitle.scaled(model.textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
            Spacer(minLength: Tokens.Space.s12)
            Button {
                menuShown = true
            } label: {
                LucideIcon(.userRoundPlus, size: LucideIconSize.action)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.addLabel)
            // The empty state's CTA carries the same WORD, so the identifier is
            // what tells the two apart for a test driving this screen.
            .accessibilityIdentifier("contacts.add")
        }
    }

    // MARK: - List

    @ViewBuilder private var listArea: some View {
        if let empty = model.empty ?? model.searchEmpty {
            ScrollView {
                EmptyStateCTA(model: empty, onPrimary: onAdd, onSecondary: onImport)
                    .padding(.top, Tokens.Space.s48)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            ScrollViewReader { scroll in
                ScrollView {
                    VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                        groupsBlock
                        contactsBlock
                    }
                    .padding(.bottom, Tokens.Space.s24)
                }
                .overlay(alignment: .trailing) {
                    AlphaIndexRail(
                        letters: model.indexLetters,
                        populated: Set(model.sections.map(\.letter)),
                        onSelect: { letter in jump(to: letter, with: scroll) }
                    )
                    .padding(.vertical, Tokens.Space.s24)
                }
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)
        }
    }

    @ViewBuilder private var groupsBlock: some View {
        if let header = model.groupsHeader {
            WalletSectionHeader(title: header.title, action: header.action)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s24)
            VStack(spacing: Tokens.Space.s0) {
                ForEach(Array(model.groups.enumerated()), id: \.element.id) { index, group in
                    if index > 0 { divider }
                    GroupRow(model: group, onTap: { onOpenGroup(group) })
                }
            }
            .padding(.top, Tokens.Space.s8)
        }
    }

    @ViewBuilder private var contactsBlock: some View {
        if let header = model.contactsHeader {
            WalletSectionHeader(title: header.title, action: header.action, chevron: false)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s32)
            ForEach(model.sections) { section in
                ContactLetterHeader(letter: section.letter)
                    .padding(.top, Tokens.Space.s12)
                    .id(section.letter)
                ForEach(Array(section.contacts.enumerated()), id: \.element.id) { index, contact in
                    if index > 0 { divider }
                    ContactRow(
                        contact: contact,
                        swipe: model.reveal,
                        forceRevealed: model.reveal?.contactId == contact.id,
                        onTap: { onOpenContact(contact) },
                        onDelete: {
                            confirming = contact
                            menuShown = true
                        }
                    )
                }
            }
        }
    }

    private var divider: some View {
        Rectangle()
            .fill(theme.borderBase)
            .frame(height: Tokens.BorderWidth.hairline)
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
    }

    /// A letter with no section jumps to the nearest existing one
    /// (data-model.md §Contacts).
    private func jump(to letter: String, with scroll: ScrollViewProxy) {
        let present = model.sections.map(\.letter)
        guard !present.isEmpty else { return }
        let target = present.contains(letter)
            ? letter
            : present.min(by: { distance($0, letter) < distance($1, letter) }) ?? present[0]
        scroll.scrollTo(target, anchor: .top)
    }

    private func distance(_ a: String, _ b: String) -> Int {
        let av = Int(a.unicodeScalars.first?.value ?? 0)
        let bv = Int(b.unicodeScalars.first?.value ?? 0)
        return abs(av - bv)
    }
}

#Preview("C1 default dark") {
    ContactsScreen(model: ContactsFixtures.buildMobileState(.c1, loc: ContactsPreviewData.loc).home!)
        .themed(.dark)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("C3 empty light") {
    ContactsScreen(model: ContactsFixtures.buildMobileState(.c3, loc: ContactsPreviewData.loc).home!)
        .themed(.light)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}
