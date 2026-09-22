//
//  ContactRow.swift
//  VelaWallet
//
//  ContactRow (spec 018 vocabulary #1): IdenticonAvatar seeded by the FULL
//  address + name (truncates, never wraps) + mono middle-truncated address.
//  Variants: standard (C1), member (C4 — tighter avatar/height), selected
//  (raised wash, component board), and the swipe-revealed pair 转账 / 删除
//  (SPEC 动效 · 行滑动操作, 250ms ease-out). 删除 never deletes — it raises
//  the destructive confirm (action sinks only, spec Assumptions).
//

import SwiftUI

struct ContactRow: View {
    enum Size { case standard, member }

    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let contact: ContactModel
    var size: Size = .standard
    /// Raised background (desktop parity variant; also the pressed wash).
    var selected: Bool = false
    /// Non-nil enables the swipe affordance and supplies its labels.
    var swipe: SwipeRevealModel?
    /// Fixture-forced reveal so C1s renders statically in the gallery.
    var forceRevealed: Bool = false
    var onTap: () -> Void = {}
    var onSend: () -> Void = {}
    var onDelete: () -> Void = {}

    @State private var settled: CGFloat = 0
    @GestureState private var dragging: CGFloat = 0

    private var avatar: CGFloat {
        size == .standard ? ContactsGeometry.rowAvatar : ContactsGeometry.memberAvatar
    }

    private var minHeight: CGFloat {
        size == .standard ? ContactsGeometry.rowMinHeight : ContactsGeometry.memberRowHeight
    }

    private var offset: CGFloat {
        guard swipe != nil else { return 0 }
        return min(0, max(-ContactsGeometry.swipeRevealWidth, settled + dragging))
    }

    var body: some View {
        ZStack(alignment: .trailing) {
            if let swipe {
                actions(swipe)
            }
            content
                .offset(x: offset)
                // `GestureMask.none` disables EVERY gesture in the subtree,
                // not just the one being attached — including the row's own
                // button. With no swipe model (which is every row on every
                // screen but C1s) the whole list was inert: taps landed, the
                // element reported itself hittable, and nothing happened.
                //
                // So the gesture is attached only when there IS one.
                .gesture(swipe == nil ? nil : dragGesture)
        }
        .frame(minHeight: minHeight)
        .clipped()
        .onAppear {
            if forceRevealed { settled = -ContactsGeometry.swipeRevealWidth }
        }
    }

    // MARK: - Row content

    private var content: some View {
        Button(action: onTap) {
            HStack(spacing: Tokens.Space.s12) {
                IdenticonAvatar(seed: contact.addressFull, size: avatar)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(verbatim: contact.name)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                        .truncationMode(.tail)
                    Text(verbatim: contact.addressDisplay)
                        .monoRole(Typography.monoAddressDetail.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .lineLimit(1)
                        .truncationMode(.middle)
                }
                Spacer(minLength: Tokens.Space.s12)
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .frame(maxWidth: .infinity, alignment: .leading)
            .frame(minHeight: minHeight)
            .background(selected ? theme.bgRaised : theme.bgBase)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityElement(children: .combine)
    }

    // MARK: - Swipe actions

    private func actions(_ model: SwipeRevealModel) -> some View {
        HStack(spacing: Tokens.Space.s0) {
            actionTile(label: model.sendLabel, glyph: .arrowUpRight, tint: theme.accentBase, action: onSend)
            actionTile(label: model.deleteLabel, glyph: .trash2, tint: theme.errorBase, action: onDelete)
        }
        .frame(width: ContactsGeometry.swipeRevealWidth)
        .frame(maxHeight: .infinity)
    }

    private func actionTile(label: String, glyph: LucideGlyph, tint: Color, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            VStack(spacing: Tokens.Space.s4) {
                LucideIcon(glyph, size: LucideIconSize.ghostPlus)
                Text(verbatim: label)
                    .typeRole(Typography.chip.scaled(textScale))
                    .lineLimit(1)
            }
            .foregroundStyle(theme.onAccent)
            .frame(width: ContactsGeometry.swipeActionWidth)
            .frame(maxHeight: .infinity)
            .background(tint)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
    }

    private var dragGesture: some Gesture {
        DragGesture(minimumDistance: Tokens.Space.s8)
            .updating($dragging) { value, state, _ in
                // Horizontal intent only — vertical drags stay with the list.
                guard abs(value.translation.width) > abs(value.translation.height) else { return }
                state = value.translation.width
            }
            .onEnded { value in
                let open = settled + value.translation.width < -ContactsGeometry.swipeOpenThreshold
                let target: CGFloat = open ? -ContactsGeometry.swipeRevealWidth : 0
                if reduceMotion {
                    settled = target
                } else {
                    withAnimation(.easeOut(duration: ContactsMotion.swipeReveal)) { settled = target }
                }
            }
    }
}

#Preview("Contact rows dark") {
    VStack(spacing: Tokens.Space.s0) {
        ContactRow(contact: ContactsPreviewData.alice)
        ContactRow(contact: ContactsPreviewData.longName)
        ContactRow(contact: ContactsPreviewData.alice, selected: true)
        ContactRow(contact: ContactsPreviewData.mother, size: .member)
        ContactRow(
            contact: ContactsPreviewData.ahao,
            swipe: SwipeRevealModel(contactId: UUID(), sendLabel: "转账", deleteLabel: "删除"),
            forceRevealed: true
        )
    }
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
    .environment(\.identiconProvider, .previewSafe)
    .environment(\.lucideIconProvider, .previewSafe)
}

#Preview("Contact rows light") {
    VStack(spacing: Tokens.Space.s0) {
        ContactRow(contact: ContactsPreviewData.alice)
        ContactRow(contact: ContactsPreviewData.longName)
    }
    .themed(.light)
    .environment(\.identiconProvider, .previewSafe)
    .environment(\.lucideIconProvider, .previewSafe)
}

/// Preview-only slices of the canon (keeps #Previews offline and terse).
enum ContactsPreviewData {
    static let loc = Loc(overrideTag: "zh")
    static var alice: ContactModel { row(0) }
    static var ahao: ContactModel { row(1) }
    static var longName: ContactModel { row(2) }
    static var mother: ContactModel { row(7) }

    private static func row(_ index: Int) -> ContactModel {
        let canon = ContactsFixtures.roster[index]
        return ContactModel(
            name: canon.name,
            addressDisplay: canon.addressDisplay,
            addressFull: canon.addressFull,
            sectionKey: canon.section,
            groups: canon.groups
        )
    }
}
