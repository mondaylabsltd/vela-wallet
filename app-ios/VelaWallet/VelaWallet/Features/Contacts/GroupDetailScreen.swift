//
//  GroupDetailScreen.swift
//  VelaWallet
//
//  Mobile group detail (spec 018 FR-002, mocks C4 / C6): back chevron + ⋯,
//  large group title with the member count, member rows (the ContactRow
//  member variant), the ghost 添加成员 row, and the bottom-pinned 群发转账
//  CTA with its caption. The ⋯ button raises the group ActionMenuSheet (C6).
//

import SwiftUI

struct GroupDetailScreen: View {
    @Environment(\.theme) private var theme

    let model: GroupDetailModel
    var onBack: () -> Void = {}
    var onOpenMember: (ContactModel) -> Void = { _ in }
    /// 删除分组, from the ⋯ menu (spec 050).
    ///
    /// No second confirmation, because the drawing has none and none is owed:
    /// deleting a group removes the grouping, not the people. The members are
    /// still in the address book afterwards, which is what makes this different
    /// from deleting a contact — that one IS drawn with a confirm, and has one.
    var onDeleteGroup: () -> Void = {}
    /// 添加成员 — open the member picker, tick rows, commit the set.
    var onOpenMembers: () -> Void = {}
    var onToggleMember: (String) -> Void = { _ in }
    var onSaveMembers: () -> Void = {}
    var onCancelMembers: () -> Void = {}
    /// 群发转账 — the group's whole membership becomes a split.
    var onBatchSend: () -> Void = {}
    /// 导入到本组 and 导出本组, from the ⋯ menu.
    var onImportIntoGroup: () -> Void = {}
    var onExportGroup: () -> Void = {}

    /// Whether the ⋯ menu is up. The picker's presence is the core's answer.
    @State private var menuShown = false

    private enum Presented {
        case menu(ActionMenuModel)
        case members(MultiPickModel)
    }

    private var presented: Presented? {
        if let pick = model.memberPick { return .members(pick) }
        if menuShown, let sheet = model.sheet { return .menu(sheet) }
        return nil
    }

    private func dismissSheet() {
        if model.memberPick != nil { onCancelMembers() }
        menuShown = false
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            navBar
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    title
                    membersBlock
                        .padding(.top, Tokens.Space.s16)
                    GhostAddRow(label: model.addMemberLabel, onTap: onOpenMembers)
                }
                .padding(.bottom, Tokens.Space.s24)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            PinnedCTABar(
                title: model.ctaLabel, caption: model.ctaCaption,
                enabled: model.ctaEnabled, onTap: onBatchSend
            )
                .padding(.bottom, Tokens.Space.s8)
        }
        .background(theme.bgBase.ignoresSafeArea())
        .walletTextScale(model.textScale)
        // ONE sheet, whose content changes — the rule the contacts list paid
        // for in dead taps.
        .sheet(isPresented: Binding(
            get: { presented != nil },
            set: { if !$0 { dismissSheet() } }
        )) {
            switch presented {
            case .menu(let sheet):
                ActionMenuSheet(
                    model: sheet,
                    // 编辑分组 · 导入到本组 · 导出本组 · 删除分组, in the drawn
                    // order. The first still has no form drawn for it and
                    // dismisses; the other three now act.
                    onItem: { item in
                        menuShown = false
                        if item.destructive {
                            onDeleteGroup()
                        } else if let index = sheet.items.firstIndex(where: { $0.id == item.id }) {
                            if index == 1 { onImportIntoGroup() }
                            if index == 2 { onExportGroup() }
                        }
                    },
                    onCancel: { menuShown = false }
                )
                .walletTextScale(model.textScale)
            case .members(let pick):
                MultiPickSheet(
                    model: pick,
                    onToggle: onToggleMember,
                    onSave: onSaveMembers,
                    onCancel: { onCancelMembers() }
                )
                .walletTextScale(model.textScale)
            case nil:
                EmptyView()
            }
        }
        // C6 is the state that opens WITH the menu up; every other state has
        // the same menu behind ⋯ and starts with it closed.
        .onAppear { menuShown = model.state == .c6 }
    }

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
            Button {
                menuShown = true
            } label: {
                LucideIcon(.ellipsis, size: LucideIconSize.action)
                    .foregroundStyle(theme.fgMuted)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(model.moreLabel)
        }
        .padding(.horizontal, Tokens.Space.s12)
    }

    private var title: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: model.name)
                .typeRole(Typography.pageTitle.scaled(model.textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
            Text(verbatim: model.membersLabel)
                .typeRole(Typography.rowSub.scaled(model.textScale))
                .foregroundStyle(theme.fgMuted)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .padding(.top, Tokens.Space.s16)
    }

    private var membersBlock: some View {
        VStack(spacing: Tokens.Space.s0) {
            ForEach(Array(model.members.enumerated()), id: \.element.id) { index, member in
                if index > 0 {
                    Rectangle()
                        .fill(theme.borderBase)
                        .frame(height: Tokens.BorderWidth.hairline)
                        .padding(.horizontal, Tokens.Layout.screenPaddingX)
                }
                ContactRow(contact: member, size: .member, onTap: { onOpenMember(member) })
            }
        }
    }
}

#Preview("C4 group dark") {
    GroupDetailScreen(model: ContactsFixtures.buildMobileState(.c4, loc: ContactsPreviewData.loc).group!)
        .themed(.dark)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("C4 empty group light") {
    GroupDetailScreen(model: ContactsFixtures.emptyGroup(loc: ContactsPreviewData.loc))
        .themed(.light)
        .environment(\.identiconProvider, .previewSafe)
        .environment(\.lucideIconProvider, .previewSafe)
}
