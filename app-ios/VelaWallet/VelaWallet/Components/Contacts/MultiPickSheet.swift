//
//  MultiPickSheet.swift
//  VelaWallet
//
//  C10 — a list with tick boxes, and one button that commits the whole list.
//
//  Two questions have the same shape and so share one sheet: "which groups is
//  this person in" and "who is in this group". Both are answered as a SET, not
//  as a stream of adds and removes, because that is how the core takes them —
//  `set_contact_groups` and `set_group_members` each replace the membership
//  outright. A sheet that emitted one event per tap would leave a half-applied
//  membership behind if somebody closed it midway.
//
//  Composed from 018's vocabulary — the sheet chrome of `ActionMenuSheet`, the
//  rows of `ContactRow`, the CTA of every other form — rather than inventing a
//  new visual language for a list of choices.
//

import SwiftUI

/// One tickable line. `subtitle` is the address for a person and the member
/// count for a group.
struct MultiPickRowModel: Identifiable {
    let id: String
    let title: String
    let subtitle: String
    /// An address, when the row is a person — the identicon's seed.
    var identiconSeed: String?
    var picked: Bool
}

struct MultiPickModel {
    let title: String
    let rows: [MultiPickRowModel]
    /// Shown INSTEAD of the rows when there are none — "save somebody first".
    let emptyText: String
    let save: String
    let cancel: String
}

struct MultiPickSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: MultiPickModel
    var onToggle: (String) -> Void = { _ in }
    var onSave: () -> Void = {}
    var onCancel: () -> Void = {}

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s0) {
            Capsule()
                .fill(theme.borderStrong)
                .frame(width: WalletGeometry.sheetHandleWidth, height: WalletGeometry.sheetHandleHeight)
                .frame(maxWidth: .infinity)
                .padding(.top, Tokens.Space.s8)

            Text(verbatim: model.title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .padding(.top, Tokens.Space.s20)

            if model.rows.isEmpty {
                Text(verbatim: model.emptyText)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
                    .padding(.vertical, Tokens.Space.s24)
            } else {
                ScrollView {
                    VStack(spacing: Tokens.Space.s0) {
                        ForEach(model.rows) { row in
                            Button { onToggle(row.id) } label: {
                                pickRow(row)
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
                .frame(maxHeight: ContactsGeometry.pickSheetMaxHeight)
                .padding(.top, Tokens.Space.s8)
            }

            VelaButton(title: model.save, kind: .primary, action: onSave)
                .padding(.top, Tokens.Space.s16)

            Button(action: onCancel) {
                Text(verbatim: model.cancel)
                    .typeRole(Typography.button.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .frame(maxWidth: .infinity)
                    .frame(minHeight: Tokens.Control.lg)
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .padding(.top, Tokens.Space.s4)
            .padding(.bottom, Tokens.Space.s24)
        }
        .padding(.horizontal, Tokens.Layout.screenPaddingX)
        .frame(maxWidth: .infinity, alignment: .leading)
        .presentationDetents([.medium, .large])
        .presentationDragIndicator(.hidden)
        .presentationCornerRadius(Tokens.Radius.r20)
        .presentationBackground(theme.bgRaised)
    }

    private func pickRow(_ row: MultiPickRowModel) -> some View {
        HStack(spacing: Tokens.Space.s12) {
            if let seed = row.identiconSeed {
                IdenticonAvatar(seed: seed, size: ContactsGeometry.rowAvatar, tappable: false)
            } else {
                Circle()
                    .strokeBorder(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
                    .frame(width: ContactsGeometry.rowAvatar, height: ContactsGeometry.rowAvatar)
                    .overlay {
                        LucideIcon(.usersRound, size: LucideIconSize.checkmark)
                            .foregroundStyle(theme.fgSubtle)
                    }
            }
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: row.title)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                Text(verbatim: row.subtitle)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s12)
            // The tick, and its state in the accessibility tree too: a check
            // mark that only exists as a drawing is invisible to somebody who
            // cannot see it.
            LucideIcon(row.picked ? .check : .plus, size: LucideIconSize.checkmark)
                .foregroundStyle(row.picked ? theme.accentBase : theme.fgSubtle)
        }
        .padding(.vertical, Tokens.Space.s12)
        .contentShape(Rectangle())
        .accessibilityElement(children: .combine)
        .accessibilityAddTraits(row.picked ? [.isButton, .isSelected] : .isButton)
    }
}
