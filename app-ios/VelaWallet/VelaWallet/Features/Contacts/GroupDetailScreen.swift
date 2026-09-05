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

    @State private var sheetShown = false

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            navBar
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    title
                    membersBlock
                        .padding(.top, Tokens.Space.s16)
                    GhostAddRow(label: model.addMemberLabel)
                }
                .padding(.bottom, Tokens.Space.s24)
            }
            .frame(maxWidth: .infinity, maxHeight: .infinity)

            PinnedCTABar(title: model.ctaLabel, caption: model.ctaCaption, enabled: model.ctaEnabled)
                .padding(.bottom, Tokens.Space.s8)
        }
        .background(theme.bgBase.ignoresSafeArea())
        .environment(\.walletTextScale, model.textScale)
        .sheet(isPresented: $sheetShown) {
            if let sheet = model.sheet {
                ActionMenuSheet(
                    model: sheet,
                    // Only the destructive item has anywhere to go: 编辑分组,
                    // 导入 and 导出 are choices whose destinations nothing has
                    // drawn, so they dismiss rather than pretending to act.
                    onItem: { item in
                        sheetShown = false
                        if item.destructive { onDeleteGroup() }
                    },
                    onCancel: { sheetShown = false }
                )
                    .environment(\.walletTextScale, model.textScale)
            }
        }
        // C6 is the state that opens WITH the menu up; every other state has
        // the same menu available behind ⋯ and starts with it closed. Keying on
        // the state rather than on `sheet != nil` is what lets a live group
        // carry the menu without it popping open on arrival — and it is
        // frame-identical for the fixtures, where only C6 has a sheet.
        .onAppear { sheetShown = model.state == .c6 }
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
                sheetShown = true
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
