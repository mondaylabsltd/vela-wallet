//
//  ActivityRowView.swift
//  VelaWallet
//
//  ActivityRow (spec 015 vocabulary #8): leading direction glyph in a
//  raised circle with a chain-dot badge, title + subtitle, trailing signed
//  amount (+success / −foreground) with unit. `masked` renders the amount
//  as dots while units stay visible (H5); received rows keep the success
//  color. The trailing text concatenation lets long amounts wrap the unit
//  onto a second line (H7 −0.0000001 BNB) instead of clipping.
//

import SwiftUI

struct ActivityRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ActivityRowModel

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            iconCircle
            // Amounts always render fully (spec: numbers never clip); the
            // subtitle is the yielding element — it middle-truncates when the
            // row runs out of width. The title is never cut: a dApp's title
            // names its place (spec 093, "Spending permit on Uniswap"), so a
            // long one takes a second line rather than squeezing the figure
            // ("Unlimited USDC") into an ellipsis.
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                if let parts = model.titlePlace {
                    placeTitle(parts)
                } else {
                    Text(verbatim: model.title)
                        .typeRole(Typography.rowTitle.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(2)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Text(verbatim: model.subtitle)
                    .typeRole(Typography.rowSub.literal.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
            }
            Spacer(minLength: Tokens.Space.s12)
            // 087 F11: a dApp call that moved no coin of ours has no figure,
            // and draws no amount cell — never an empty one.
            if !model.amount.isEmpty || !model.unit.isEmpty || model.received != nil {
                VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
                    if !model.amount.isEmpty || !model.unit.isEmpty {
                        // Inline when it fits (H1 −2 POL); otherwise the unit
                        // drops to a second line (mock H7 −0.0000001 / BNB).
                        ViewThatFits(in: .horizontal) {
                            HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s4) {
                                amountText.lineLimit(1)
                                unitText.lineLimit(1)
                            }
                            VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
                                amountText
                                    .lineLimit(1)
                                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                                unitText.lineLimit(1)
                            }
                        }
                    }
                    // A swap's coin back, under what left (spec 093).
                    if let received = model.received {
                        Text(verbatim: received)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.successBase)
                            .lineLimit(1)
                    }
                }
                .layoutPriority(1)
            }
        }
        .frame(minHeight: WalletGeometry.rowMinHeight)
    }

    /// A dApp's title: the verb and its words whole, the place cut in its
    /// middle when the row runs short (`PlaceTitleLayout`). Said as the one
    /// title it is.
    private func placeTitle(_ parts: TitlePlace) -> some View {
        let role = Typography.rowTitle.scaled(textScale)
        return PlaceTitleLayout(gapBefore: parts.gapBefore, gapAfter: parts.gapAfter, gap: role.size * 0.28) {
            Text(verbatim: parts.lead).typeRole(role).lineLimit(2)
            Text(verbatim: parts.place).typeRole(role).lineLimit(1).truncationMode(.middle)
            Text(verbatim: parts.trail).typeRole(role).lineLimit(2)
        }
        .foregroundStyle(theme.fgBase)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: model.title))
        .accessibilityAddTraits(.isStaticText)
    }

    private var amountColor: Color {
        if model.danger { return theme.errorBase }
        return model.positive ? theme.successBase : theme.fgBase
    }

    private var amountText: Text {
        Text(verbatim: model.amount)
            .font(Typography.rowValue.scaled(textScale).font)
            .foregroundStyle(amountColor)
    }

    private var unitText: Text {
        Text(verbatim: model.unit)
            .font(Typography.rowSub.scaled(textScale).font)
            .foregroundStyle(theme.fgMuted)
    }

    private var glyph: LucideGlyph {
        switch model.kind {
        case .sent: .arrowUpRight
        case .received: .arrowDownLeft
        case .dapp: .link2
        }
    }

    private var iconCircle: some View {
        ZStack(alignment: .bottomTrailing) {
            Circle()
                .fill(theme.bgRaised)
                .frame(width: WalletGeometry.rowIcon, height: WalletGeometry.rowIcon)
                .overlay {
                    LucideIcon(glyph, size: LucideIconSize.rowGlyph)
                        .foregroundStyle(model.kind == .received ? theme.successBase : theme.fgBase)
                }
            if let url = model.badgeLogoURL {
                RemoteLogoView(urls: [url], size: WalletGeometry.badgeLogo) {
                    Circle().fill(model.badgeColor)
                }
                .padding(WalletGeometry.badgeLogoRingWidth)
                .background(Circle().fill(theme.bgBase))
            } else {
                ChainBadgeDot(color: model.badgeColor)
            }
        }
    }
}

/// Chain-dot badge shared by activity rows and token icons: fixture color
/// dot ringed by the base background so it reads on any circle.
struct ChainBadgeDot: View {
    @Environment(\.theme) private var theme
    let color: Color

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: WalletGeometry.badge, height: WalletGeometry.badge)
            .padding(WalletGeometry.badgeRing)
            .background(Circle().fill(theme.bgBase))
    }
}

#Preview("Activity rows dark") {
    VStack(spacing: Tokens.Space.s0) {
        ActivityRowView(model: ActivityRowModel(
            kind: .sent, title: "已发送", subtitle: "至 hold on",
            amount: "−2", unit: "POL", positive: false, masked: false,
            badgeColor: ChainPalette.polygon
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .received, title: "已收到", subtitle: "来自 0x9F3c…21aE",
            amount: "+120", unit: "USDT", positive: true, masked: false,
            badgeColor: ChainPalette.ethereum
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .dapp, title: "dApp 交易", subtitle: "PancakeSwap · BNB Chain",
            amount: "−0.05", unit: "BNB", positive: false, masked: false,
            badgeColor: ChainPalette.bnb
        ))
        ActivityRowView(model: ActivityRowModel(
            kind: .received, title: "已收到", subtitle: "来自 Alice",
            amount: "••••", unit: "USDC", positive: true, masked: true,
            badgeColor: ChainPalette.base
        ))
    }
    .padding(Tokens.Space.s24)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Activity row light") {
    ActivityRowView(model: ActivityRowModel(
        kind: .sent, title: "Sent", subtitle: "To Alexandra",
        amount: "−1234.5678", unit: "POL", positive: false, masked: false,
        badgeColor: ChainPalette.polygon
    ))
    .padding(Tokens.Space.s24)
    .themed(.light)
}

/// Lays a title out as lead · place · trail (`TitlePlace`): on one line when
/// it fits; else broken at the place's edge — never inside the place, and
/// never by cutting the words either side of it, which wrap (two lines at
/// most) rather than lose a word.
///
/// - a trail (the verb after the place — 「在 ⟨place⟩ 合约交互」, 「⟨place⟩で署名」):
///   first the lead and the place, cut to what is left; under them the trail.
/// - no trail (the verb first — "Contract interaction on ⟨place⟩"): first the
///   lead; under it the place.
struct PlaceTitleLayout: Layout {
    var gapBefore: Bool
    var gapAfter: Bool
    /// A space's width in the title's face.
    var gap: CGFloat

    /// Where each of lead, place and trail goes, from their one-line widths.
    struct Slot: Equatable {
        let x: CGFloat
        let line: Int
        let width: CGFloat
    }

    static func arrange(
        lead: CGFloat, place: CGFloat, trail: CGFloat,
        gapBefore: CGFloat, gapAfter: CGFloat, available: CGFloat
    ) -> [Slot] {
        let before = lead > 0 ? gapBefore : 0
        let after = trail > 0 ? gapAfter : 0
        if lead + before + place + after + trail <= available {
            return [
                Slot(x: 0, line: 0, width: lead),
                Slot(x: lead + before, line: 0, width: place),
                Slot(x: lead + before + place + after, line: 0, width: trail),
            ]
        }
        if trail > 0 {
            let leadWidth = min(lead, available)
            let room = max(0, available - leadWidth - before)
            return [
                Slot(x: 0, line: 0, width: leadWidth),
                Slot(x: leadWidth + before, line: 0, width: min(place, room)),
                Slot(x: 0, line: 1, width: min(trail, available)),
            ]
        }
        return [
            Slot(x: 0, line: 0, width: min(lead, available)),
            Slot(x: 0, line: 1, width: min(place, available)),
            Slot(x: 0, line: 1, width: 0),
        ]
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        let (slots, heights, available) = layout(proposal.width, subviews)
        let used = slots.map { $0.x + $0.width }.max() ?? 0
        return CGSize(width: proposal.width == nil ? used : min(used, available), height: heights.reduce(0, +))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let (slots, heights, _) = layout(bounds.width, subviews)
        for (subview, slot) in zip(subviews, slots) {
            let top = heights.prefix(slot.line).reduce(0, +)
            subview.place(
                at: CGPoint(x: bounds.minX + slot.x, y: bounds.minY + top),
                proposal: ProposedViewSize(width: slot.width, height: heights[slot.line])
            )
        }
    }

    /// The slots, the height of each of their lines (a part that wraps makes
    /// its line taller), and the width laid out in.
    private func layout(_ width: CGFloat?, _ subviews: Subviews) -> ([Slot], [CGFloat], CGFloat) {
        let ideal = subviews.map { $0.sizeThatFits(.unspecified) }
        func w(_ index: Int) -> CGFloat { index < ideal.count ? ideal[index].width : 0 }
        let available = width ?? (w(0) + w(1) + w(2) + 2 * gap)
        let slots = Self.arrange(
            lead: w(0), place: w(1), trail: w(2),
            gapBefore: gapBefore ? gap : 0, gapAfter: gapAfter ? gap : 0, available: available
        )
        let lines = (slots.map(\.line).max() ?? 0) + 1
        var heights = [CGFloat](repeating: 0, count: lines)
        for (subview, slot) in zip(subviews, slots) where slot.width > 0 {
            let drawn = subview.sizeThatFits(ProposedViewSize(width: slot.width, height: nil)).height
            heights[slot.line] = max(heights[slot.line], drawn)
        }
        return (slots, heights, available)
    }
}
