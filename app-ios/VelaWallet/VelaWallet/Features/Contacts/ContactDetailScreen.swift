//
//  ContactDetailScreen.swift
//  VelaWallet
//
//  Mobile contact detail (spec 018 FR-002, mocks C2 / C2s): back chevron +
//  edit pencil, identicon hero, name, short address, group chips, the three
//  action cards, the full mono address block, 最近往来 over spec-015
//  ActivityRow instances, and the centered destructive 删除联系人 which
//  raises the confirm sheet (C2s).
//
//  Reused from spec 015: IdenticonAvatar, ActionButtonRow, WalletSectionHeader,
//  ActivityRowView, WalletEmptyState (the no-activity edge case).
//

import SwiftUI

struct ContactDetailScreen: View {
    @Environment(\.theme) private var theme

    let model: ContactDetailModel
    var onBack: () -> Void = {}
    /// The pencil. Opens the edit form, and it shipped doing nothing.
    var onEdit: () -> Void = {}
    /// The star. Every client's `ToggleFavorite`.
    var onFavourite: () -> Void = {}
    var onDelete: () -> Void = {}
    /// The live form's fields and CTA. Absent in the gallery, where the sheet
    /// is a picture.
    var formName: Binding<String>?
    var formAddress: Binding<String>?
    var onSaveForm: () -> Void = {}
    var onCancelForm: () -> Void = {}
    /// + 分组 — open the group picker, tick a row, commit the set.
    var onOpenGroups: () -> Void = {}
    var onToggleGroup: (String) -> Void = { _ in }
    var onSaveGroups: () -> Void = {}
    var onCancelGroups: () -> Void = {}
    /// 转账 — a send already addressed to this person.
    var onSendTo: (() -> Void)?
    /// 收款 and 二维码, which are about the wallet's OWN address.
    var onReceive: (() -> Void)?
    var onShowQr: (() -> Void)?

    /// Whether the delete confirmation is up. The form's and the picker's
    /// presence are the CORE's answers and need no flags of their own.
    @State private var confirmShown = false

    /// What the one sheet is showing, in order of precedence: a form or a
    /// picker is open because somebody asked for it just now.
    private enum Presented {
        case confirm(ActionMenuModel)
        case form(ContactFormModel)
        case groups(MultiPickModel)
    }

    private var presented: Presented? {
        if let form = model.form { return .form(form) }
        if let pick = model.groupPick { return .groups(pick) }
        if confirmShown, let sheet = model.sheet { return .confirm(sheet) }
        return nil
    }

    private func dismissSheet() {
        if model.form != nil { onCancelForm() }
        if model.groupPick != nil { onCancelGroups() }
        confirmShown = false
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            navBar
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    hero
                    // Three cards that did nothing until 057. 转账 opens a
                    // send already addressed to this person; 收款 and 二维码
                    // are about the WALLET's own address, so they go where that
                    // lives rather than pretending to be about the contact.
                    ActionButtonRow(items: [
                        ActionCardItem(icon: .arrowUpRight, label: model.actions.send,
                                       action: onSendTo),
                        ActionCardItem(icon: .arrowDownLeft, label: model.actions.receive,
                                       action: onReceive),
                        ActionCardItem(icon: .qrCode, label: model.actions.qr,
                                       action: onShowQr),
                    ])
                    .padding(.top, Tokens.Space.s24)

                    hairline.padding(.top, Tokens.Space.s24)

                    AddressBlock(
                        label: model.addressLabel,
                        lines: model.addressLines,
                        copyLabel: model.copyLabel,
                        copiedLabel: model.copiedLabel
                    )
                    .padding(.top, Tokens.Space.s24)

                    hairline.padding(.top, Tokens.Space.s24)

                    WalletSectionHeader(title: model.activityTitle, action: model.activityAction)
                        .padding(.top, Tokens.Space.s24)
                    activity

                    deleteAction
                        .padding(.top, Tokens.Space.s32)
                }
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.bottom, Tokens.Space.s32)
            }
        }
        .background(theme.bgBase.ignoresSafeArea())
        .walletTextScale(model.textScale)
        // ONE sheet, whose CONTENT changes. Three `.sheet` modifiers on one
        // view is what stopped the contacts list responding to taps at all
        // (the same defect, on the screen before this one).
        .sheet(isPresented: Binding(
            get: { presented != nil },
            set: { if !$0 { dismissSheet() } }
        )) {
            switch presented {
            case .confirm(let menu):
                ActionMenuSheet(
                    model: menu,
                    // The one item this sheet has is the destructive one: it is
                    // the second confirmation on 删除联系人, and until spec 054
                    // it went nowhere — the button raised a sheet the live
                    // builder left empty, which is a confirmation dialog for a
                    // deletion that never happened.
                    onItem: { item in
                        confirmShown = false
                        if item.destructive { onDelete() }
                    },
                    onCancel: { confirmShown = false }
                )
                .walletTextScale(model.textScale)
            case .form(let form):
                ContactFormSheet(
                    model: form,
                    nameText: formName,
                    addressText: formAddress,
                    onSave: onSaveForm,
                    onCancel: { onCancelForm() }
                )
                .walletTextScale(model.textScale)
            case .groups(let pick):
                MultiPickSheet(
                    model: pick,
                    onToggle: onToggleGroup,
                    onSave: onSaveGroups,
                    onCancel: { onCancelGroups() }
                )
                .walletTextScale(model.textScale)
            case nil:
                EmptyView()
            }
        }
        // C2s is the state that opens WITH the confirm up; every other one has
        // it behind 删除联系人 and starts closed.
        .onAppear { confirmShown = model.state == .c2s }
    }

    private func inspectionTag(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub.scaled(model.textScale))
            .foregroundStyle(theme.fgMuted)
            .padding(.horizontal, Tokens.Space.s8)
            .padding(.vertical, Tokens.Space.s2)
            .background(Capsule().fill(theme.bgRaised))
    }

    // MARK: - Chrome

    private var navBar: some View {
        HStack(spacing: Tokens.Space.s12) {
            Button(action: onBack) {
                LucideIcon(.chevronLeft, size: LucideIconSize.action)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.backLabel)
            Spacer(minLength: Tokens.Space.s12)
            if let favourite = model.favourite {
                Button(action: onFavourite) {
                    LucideIcon(favourite.on ? .starSolid : .star, size: LucideIconSize.menuRow)
                        .foregroundStyle(favourite.on ? theme.accentBase : theme.fgMuted)
                        .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(favourite.label)
                .accessibilityAddTraits(favourite.on ? [.isButton, .isSelected] : .isButton)
            }
            Button(action: onEdit) {
                LucideIcon(.pencil, size: LucideIconSize.menuRow)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.editLabel)
        }
        .padding(.horizontal, Tokens.Space.s12)
    }

    private var hero: some View {
        VStack(spacing: Tokens.Space.s0) {
            IdenticonAvatar(seed: model.contact.addressFull, size: ContactsGeometry.detailAvatar,
                             name: model.contact.name)
            Text(verbatim: model.contact.name)
                .typeRole(Typography.title.scaled(model.textScale))
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
                .padding(.top, Tokens.Space.s16)
            Text(verbatim: model.contact.addressDisplay)
                .monoRole(Typography.monoAddressDetail.scaled(model.textScale))
                .foregroundStyle(theme.fgMuted)
                .padding(.top, Tokens.Space.s8)
            // What the core found out about this address when the page opened:
            // whether it is a contract wallet, and whether this person has ever
            // been paid before. Two neutral tags — never a warning, because
            // neither fact is one.
            if let inspection = model.inspection,
               inspection.tag != nil || inspection.firstTime != nil {
                HStack(spacing: Tokens.Space.s8) {
                    if let tag = inspection.tag { inspectionTag(tag) }
                    if let first = inspection.firstTime { inspectionTag(first) }
                }
                .padding(.top, Tokens.Space.s8)
            }
            GroupChips(chips: model.chips, addLabel: model.addChip, onAdd: onOpenGroups)
                .padding(.top, Tokens.Space.s12)
        }
        .frame(maxWidth: .infinity)
        .padding(.top, Tokens.Space.s16)
    }

    @ViewBuilder private var activity: some View {
        if model.activity.isEmpty, let empty = model.activityEmpty {
            WalletEmptyState(icon: .inbox, model: empty)
        } else {
            VStack(spacing: Tokens.Space.s0) {
                ForEach(Array(model.activity.enumerated()), id: \.element.id) { index, row in
                    if index > 0 {
                        Rectangle()
                            .fill(theme.borderBase)
                            .frame(height: Tokens.BorderWidth.hairline)
                            .padding(.leading, WalletGeometry.rowDividerInset)
                    }
                    ActivityRowView(model: row)
                }
            }
        }
    }

    private var deleteAction: some View {
        Button {
            confirmShown = true
        } label: {
            Text(verbatim: model.deleteLabel)
                .typeRole(Typography.button.scaled(model.textScale))
                .foregroundStyle(theme.errorBase)
                .frame(maxWidth: .infinity)
                .frame(minHeight: Tokens.Control.md)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var hairline: some View {
        Rectangle()
            .fill(theme.borderBase)
            .frame(height: Tokens.BorderWidth.hairline)
    }
}

#Preview("C2 detail dark") {
    ContactDetailScreen(model: ContactsFixtures.buildMobileState(.c2, loc: ContactsPreviewData.loc).detail!)
        .themed(.dark)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("C2 detail light") {
    ContactDetailScreen(model: ContactsFixtures.buildMobileState(.c2, loc: ContactsPreviewData.loc).detail!)
        .themed(.light)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}
