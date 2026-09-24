//
//  SettingsRows.swift
//  VelaWallet
//
//  The settings list's rows (spec 023). Every entry on ST1/ST1b is
//  `SettingsRow`; every choice in the five pickers is `SelectRow`; every
//  network is `SettingsNetworkRow`. There is no second row component anywhere
//  in this feature.
//

import SwiftUI
import UIKit

/// One settings row: an optional leading glyph, a title, an optional second
/// line, an optional right-aligned value, and a trailing chevron or external
/// mark. `.danger` is the red 退出登录 / 清理数据 tone.
struct SettingsRow: View {
    @Environment(\.theme) private var theme
    let row: SettingsRowModel
    var divider = true
    var onTap: (String) -> Void = { _ in }

    private var tint: Color {
        switch row.tone {
        case .standard: theme.fgBase
        case .accent: theme.accentBase
        case .danger: theme.errorBase
        }
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Tokens.Space.s12) {
                if let icon = row.icon {
                    LucideIcon(icon, size: LucideIconSize.action)
                        .foregroundStyle(row.tone == .standard ? theme.fgMuted : tint)
                        .frame(width: LucideIconSize.action)
                }
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(row.title)
                        .typeRole(Typography.fieldLabel)
                        .fontWeight(.semibold)
                        .foregroundStyle(tint)
                        .lineLimit(1)
                    if let subtitle = row.subtitle {
                        Text(subtitle)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgSubtle)
                            .lineLimit(1)
                    }
                }
                Spacer(minLength: Tokens.Space.s8)
                if let value = row.value {
                    Text(value)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                }
                switch row.trailing {
                case .chevron:
                    LucideIcon(.chevronRight, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                case .external:
                    LucideIcon(.externalLink, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                case .none:
                    EmptyView()
                }
            }
            .frame(minHeight: 52)
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
            .onTapGesture { onTap(row.id) }
            if divider { SettingsDivider() }
        }
    }
}

/// ST1's identity block: identicon, name, truncated address, and a trailing
/// TEXT action rather than a bare chevron — "切换账户 ›" says what the tap
/// does, which a chevron alone does not.
struct SettingsAccountRow: View {
    @Environment(\.theme) private var theme
    let account: SettingsAccountRowModel
    var onTap: () -> Void = {}

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: account.addressFull, size: 40)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(account.name)
                        .typeRole(Typography.title)
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                    Text(account.addressDisplay)
                        .typeRole(Typography.monoSmall)
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer(minLength: Tokens.Space.s8)
                Text(account.action)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgMuted)
                LucideIcon(.chevronRight, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgSubtle)
            }
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
            .onTapGesture(perform: onTap)
            SettingsDivider()
        }
    }
}

/// One choice in a picker. Five sheets are made of nothing else: language,
/// currency, number, date and time. The differences are all data.
struct SelectRow: View {
    @Environment(\.theme) private var theme
    let row: SelectRowModel
    /// `nil` when somebody ELSE owns the tap.
    ///
    /// It was a defaulted no-op, and that is the whole of the bug the founder
    /// hit on 2026-09-15: every select sheet wraps this row in a `Button`, the
    /// row's own `onTapGesture` sat INSIDE that button and swallowed the tap,
    /// and the no-op ran. Picking a language, a currency, or any of the three
    /// formats did nothing at all — four settings, dead through one default
    /// argument. An optional handler cannot do that: with no handler there is
    /// no gesture to eat the button's.
    var onTap: ((String) -> Void)?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Tokens.Space.s8) {
                if let glyph = row.glyph {
                    Circle()
                        .fill(theme.bgRaised)
                        .frame(width: 32, height: 32)
                        .overlay(
                            Text(glyph)
                                .typeRole(Typography.body)
                                .foregroundStyle(theme.fgMuted)
                        )
                }
                // The chosen row is stated twice — accent text and a check —
                // because the check alone disappears at the note's type size.
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(row.label)
                        .typeRole(row.mono ? Typography.mono : Typography.fieldLabel)
                        .fontWeight(row.selected ? .semibold : .regular)
                        .foregroundStyle(row.selected ? theme.accentBase : theme.fgBase)
                    if let detail = row.detail {
                        Text(detail)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgSubtle)
                    }
                }
                if let caption = row.caption {
                    Text(caption)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer(minLength: Tokens.Space.s8)
                if let note = row.note {
                    Text(note)
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgSubtle)
                        .lineLimit(1)
                }
                if row.selected {
                    LucideIcon(.check, size: LucideIconSize.action)
                        .foregroundStyle(theme.accentBase)
                }
            }
            .frame(minHeight: 52)
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
            .modifier(RowTap(onTap: onTap.map { handler in { handler(row.id) } }))
            SettingsDivider()
        }
    }
}

/// A tap gesture, attached only when there is something to call.
///
/// Attaching one unconditionally is how a row inside a `Button` stops working:
/// the inner gesture wins and the button never hears the tap.
private struct RowTap: ViewModifier {
    let onTap: (() -> Void)?

    func body(content: Content) -> some View {
        if let onTap {
            content.onTapGesture(perform: onTap)
        } else {
            content
        }
    }
}

/// One network row (ST9 / ST10's results): mark, name, chain-id line, optional
/// latency pill, optional 自定义 tag, and a bin for the ones that can go.
struct SettingsNetworkRow: View {
    @Environment(\.theme) private var theme
    let row: SettingsNetworkRowModel
    /// The bin's accessible name. It was handed "Add network" (spec 072):
    /// VoiceOver read the one destructive control on the row as its opposite.
    var deleteLabel: String?
    var onTap: (String) -> Void = { _ in }
    /// The bin, as a button — the host asks before anything goes. `nil` keeps
    /// the drawn glyph, which is what every gallery board shows.
    var onRemove: ((String) -> Void)?

    var body: some View {
        VStack(spacing: 0) {
            HStack(spacing: Tokens.Space.s12) {
                ChainMark(mark: row.mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    HStack(spacing: Tokens.Space.s8) {
                        Text(row.name)
                            .typeRole(Typography.fieldLabel)
                            .fontWeight(.semibold)
                            .foregroundStyle(theme.fgBase)
                        if let tag = row.tag {
                            Text(tag)
                                .typeRole(Typography.label)
                                .foregroundStyle(theme.warningBase)
                                .padding(.horizontal, Tokens.Space.s8)
                                .padding(.vertical, Tokens.Space.s2)
                                .background(theme.warningSoft,
                                            in: RoundedRectangle(cornerRadius: Tokens.Radius.r4))
                        }
                    }
                    Text(row.meta)
                        .typeRole(Typography.monoSmall)
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer(minLength: Tokens.Space.s8)
                if let badge = row.badge { StatusPill(pill: badge) }
                if row.removable, let deleteLabel, let onRemove {
                    Button { onRemove(row.id) } label: {
                        LucideIcon(.trash2, size: LucideIconSize.action)
                            .foregroundStyle(theme.fgSubtle)
                            .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .accessibilityLabel(deleteLabel)
                } else if row.removable, let deleteLabel {
                    LucideIcon(.trash2, size: LucideIconSize.action)
                        .foregroundStyle(theme.fgSubtle)
                        .accessibilityLabel(deleteLabel)
                }
                LucideIcon(.chevronRight, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgSubtle)
            }
            .padding(.vertical, Tokens.Space.s12)
            .contentShape(Rectangle())
            .onTapGesture { onTap(row.id) }
            SettingsDivider()
        }
    }
}

/// ST10b/ST10c's compatibility checklist. Both verdicts show all four rows — a
/// shortened list would hide WHICH requirement failed, and that is the only
/// useful part of an "incompatible" answer.
struct SettingsCheckList: View {
    @Environment(\.theme) private var theme
    let title: String
    let items: [CheckItemModel]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            Text(title)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
            ForEach(items) { item in
                HStack(spacing: Tokens.Space.s12) {
                    LucideIcon(item.ok ? .check : .close, size: LucideIconSize.action)
                        .foregroundStyle(item.ok ? theme.successBase : theme.errorBase)
                    Text(item.label)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.fgBase)
                }
            }
        }
        .frame(maxWidth: .infinity, alignment: .leading)
        .padding(Tokens.Space.s16)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .strokeBorder(theme.borderBase, lineWidth: 1)
        )
    }
}

/// ST13's stacked bar plus legend. Shares, not pixels — true at any width.
struct StorageBar: View {
    @Environment(\.theme) private var theme
    let segments: [StorageSegmentModel]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            GeometryReader { geo in
                HStack(spacing: 0) {
                    ForEach(segments) { segment in
                        Rectangle()
                            .fill(segment.color)
                            .frame(width: geo.size.width * segment.fraction)
                    }
                }
            }
            .frame(height: 8)
            .clipShape(RoundedRectangle(cornerRadius: Tokens.Radius.r4))
            HStack(spacing: Tokens.Space.s16) {
                ForEach(segments) { segment in
                    HStack(spacing: Tokens.Space.s8) {
                        Circle().fill(segment.color).frame(width: 8, height: 8)
                        Text(segment.label)
                            .typeRole(Typography.label)
                            .foregroundStyle(theme.fgSubtle)
                    }
                }
            }
        }
    }
}

/// One storage group. The group label carries the consequence — "清除后无法找回"
/// against "清除后自动重建" — which is why the same word 清除 is red in the
/// first group and plain in the second.
struct StorageGroupView: View {
    @Environment(\.theme) private var theme
    let group: StorageGroupModel
    var onGroupAction: () -> Void = {}
    /// One row's 清除, by item id. Absent in the gallery, where nothing should
    /// be removable by looking at it.
    var onItemAction: ((String) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            Text(group.label)
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
                .padding(.top, Tokens.Space.s16)
                .padding(.bottom, Tokens.Space.s4)
            ForEach(group.items) { item in
                HStack(spacing: Tokens.Space.s12) {
                    Text(item.label)
                        .typeRole(Typography.fieldLabel)
                        .foregroundStyle(theme.fgBase)
                        // "Custom tokens and networks" does not fit beside its
                        // size and its Clear on a 392pt screen, and one line
                        // clipped it to "Custom tokens and netw…". The label is
                        // what the row IS, so it wraps and keeps the space.
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                    Spacer(minLength: Tokens.Space.s8)
                    Text(item.meta)
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgSubtle)
                        .lineLimit(1)
                        // The size is a number pair — "5 items · 1…" is worse
                        // than useless. It and the action hold their width, and
                        // the label wraps into whatever is left.
                        .fixedSize()
                        .layoutPriority(1)
                    // The row's own action. It was a label until 058 — the
                    // page offered to clear eight things and could clear none.
                    Button { onItemAction?(item.id) } label: {
                        Text(item.action)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(item.destructive ? theme.errorBase : theme.fgMuted)
                            .contentShape(Rectangle())
                    }
                    .buttonStyle(.plain)
                    .disabled(onItemAction == nil)
                }
                .frame(minHeight: 44)
                .padding(.vertical, Tokens.Space.s12)
                SettingsDivider()
            }
            if let action = group.action {
                Text(action)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.infoBase)
                    .frame(maxWidth: .infinity)
                    .padding(.top, Tokens.Space.s16)
                    .contentShape(Rectangle())
                    .onTapGesture(perform: onGroupAction)
            }
        }
    }
}

/// About's technical-detail and link rows: label at the start, value at the
/// end, mono where the value is an identifier, external mark where it is a
/// place.
struct KeyValueRow: View {
    @Environment(\.theme) private var theme
    let row: KeyValueRowModel
    /// An external row's destination opens here. `nil` — the gallery — keeps
    /// the row a picture of a link.
    var onOpen: ((String) -> Void)?

    var body: some View {
        if let link = row.link, let url = URL(string: link) {
            Button { UIApplication.shared.open(url) } label: { content }
                .buttonStyle(.plain)
                .accessibilityAddTraits(.isLink)
        } else {
            content
        }
    }

    private var content: some View {
        VStack(spacing: 0) {
            HStack(spacing: Tokens.Space.s12) {
                Text(row.label)
                    .typeRole(Typography.flowCaption)
                    .fontWeight(row.external ? .semibold : .regular)
                    .foregroundStyle(row.external ? theme.fgBase : theme.fgMuted)
                Spacer(minLength: Tokens.Space.s8)
                Text(row.value)
                    .typeRole(row.mono ? Typography.monoSmall : Typography.flowCaption)
                    .foregroundStyle(row.external ? theme.fgSubtle : theme.fgBase)
                    .lineLimit(1)
                    .truncationMode(.middle)
                if row.external {
                    LucideIcon(.externalLink, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            .frame(minHeight: 44)
            .padding(.vertical, Tokens.Space.s12)
            SettingsDivider()
        }
    }
}

/// ST1b's 清理数据 card — the one place in settings drawn as a bordered box
/// rather than a hairline row, because it is the only action on the screen
/// that cannot be undone.
struct DangerCard: View {
    @Environment(\.theme) private var theme
    let title: String
    let subtitle: String
    var onTap: () -> Void = {}

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(title)
                    .typeRole(Typography.fieldLabel)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.errorBase)
                Text(subtitle)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgMuted)
            }
            Spacer(minLength: Tokens.Space.s8)
            LucideIcon(.trash2, size: LucideIconSize.action)
                .foregroundStyle(theme.errorBase)
        }
        .padding(Tokens.Space.s16)
        .background(theme.errorSoft, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .strokeBorder(theme.errorBase, lineWidth: 1)
        )
        // strokeBorder alone leaves only the outline hittable (button-feedback
        // rule), so the whole card is made a shape before it takes the tap.
        .contentShape(Rectangle())
        .onTapGesture(perform: onTap)
    }
}
