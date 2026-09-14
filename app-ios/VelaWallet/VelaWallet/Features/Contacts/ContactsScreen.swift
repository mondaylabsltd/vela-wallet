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
    /// The live form's fields and CTA. Absent in the gallery, where the sheet
    /// is a picture.
    var formName: Binding<String>?
    var formAddress: Binding<String>?
    var onSaveForm: () -> Void = {}
    var onCancelForm: () -> Void = {}

    @State private var sheetShown = false
    @State private var formShown = false
    @State private var confirming: ContactModel?

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            header
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s8)
            ContactsSearchField(model: model.search)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.top, Tokens.Space.s16)

            listArea

            WalletTabBar(tabs: model.tabs, selected: .contacts, onSelect: onSelectTab)
        }
        .background(theme.bgBase.ignoresSafeArea())
        .environment(\.walletTextScale, model.textScale)
        .sheet(isPresented: $sheetShown) {
            if let sheet = presentedSheet {
                ActionMenuSheet(model: sheet, onItem: { item in confirm(item) },
                                onCancel: { sheetShown = false })
                    .environment(\.walletTextScale, model.textScale)
            }
        }
        .sheet(isPresented: $formShown) {
            if let form = model.form {
                ContactFormSheet(
                    model: form,
                    nameText: formName,
                    addressText: formAddress,
                    onSave: onSaveForm,
                    onCancel: {
                        formShown = false
                        onCancelForm()
                    }
                )
                .environment(\.walletTextScale, model.textScale)
            }
        }
        .onAppear {
            sheetShown = model.sheet != nil
            formShown = model.form != nil
        }
        .onChange(of: confirming?.id) { _, _ in sheetShown = presentedSheet != nil }
        // The form's PRESENCE is the core's answer, so the sheet follows it: a
        // save that succeeds closes the form by removing it.
        .onChange(of: model.form != nil) { _, shown in formShown = shown }
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
        defer { sheetShown = false; confirming = nil }
        guard let target = confirming, item.destructive else { return }
        onDelete(target.addressFull)
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
                sheetShown = true
            } label: {
                LucideIcon(.userRoundPlus, size: LucideIconSize.action)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.addLabel)
        }
    }

    // MARK: - List

    @ViewBuilder private var listArea: some View {
        if let empty = model.empty ?? model.searchEmpty {
            ScrollView {
                EmptyStateCTA(model: empty)
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
                        onDelete: { confirming = contact }
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
