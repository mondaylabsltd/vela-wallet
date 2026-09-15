//
//  FlowChrome.swift
//  VelaWallet
//
//  The chrome and the inputs of the wallet flows (spec 021 components
//  1, 4–7). Grouped in one file the way the contacts primitives are: these
//  are small, they are only ever used together, and a file each would be
//  more import blocks than code.
//

import SwiftUI

/// The phone page frame for every non-sheet screen (component 1).
///
/// Back chevron on its own line, then a large title that may carry a trailing
/// text action, a network pill, or neither. R1, A1, T1, SD1, SD2, SD3 and SD4
/// are all this frame with a different body — the mocks differ in what sits
/// under the title, not in how the title sits.
struct FlowScaffold<Content: View, Footer: View>: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let header: FlowHeaderModel
    var onBack: () -> Void = {}
    var onAction: () -> Void = {}
    var onPill: () -> Void = {}
    @ViewBuilder let content: () -> Content
    @ViewBuilder let footer: () -> Footer

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            ScrollView {
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    topBar
                        .padding(.top, Tokens.Space.s8)
                    titleRow
                        .padding(.top, Tokens.Space.s8)
                        .padding(.bottom, Tokens.Space.s16)
                    content()
                }
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.bottom, Tokens.Space.s24)
            }
            footer()
        }
        .background(theme.bgBase.ignoresSafeArea())
    }

    private var topBar: some View {
        HStack(spacing: Tokens.Space.s0) {
            Button(action: onBack) {
                LucideIcon(.chevronLeft, size: LucideIconSize.flowBack)
                    .foregroundStyle(theme.fgBase)
                    .frame(width: Tokens.Layout.hitTarget, height: Tokens.Layout.hitTarget)
                    // The chevron's own glyph inset already reads as padding;
                    // pulling the button back by it puts the STROKE on the
                    // screen margin, where the title below it starts.
                    .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(header.backLabel)
            .padding(.leading, -Tokens.Space.s12)

            Spacer(minLength: Tokens.Space.s12)

            if let action = header.action {
                Button(action: onAction) {
                    Text(verbatim: action)
                        .typeRole(Typography.fieldLabel.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .frame(minHeight: Tokens.Layout.hitTarget)
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
    }

    private var titleRow: some View {
        HStack(alignment: .center, spacing: Tokens.Space.s12) {
            Text(verbatim: header.title)
                .typeRole(Typography.pageTitle.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(2)
            Spacer(minLength: Tokens.Space.s0)
            if let pill = header.pill {
                Button(action: onPill) { FlowNetworkPill(pill: pill) }
                    .buttonStyle(.plain)
            }
        }
    }
}

extension FlowScaffold where Footer == EmptyView {
    init(
        header: FlowHeaderModel,
        onBack: @escaping () -> Void = {},
        onAction: @escaping () -> Void = {},
        onPill: @escaping () -> Void = {},
        @ViewBuilder content: @escaping () -> Content
    ) {
        self.init(
            header: header, onBack: onBack, onAction: onAction, onPill: onPill,
            content: content, footer: { EmptyView() }
        )
    }
}

/// The header's chain filter. Overlapped dots, not spaced: the cluster stands
/// for "several networks", and three separate dots read as three controls.
struct FlowNetworkPill: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let pill: FlowPillModel

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            HStack(spacing: -WalletGeometry.pillDotOverlap) {
                ForEach(Array(pill.dots.enumerated()), id: \.offset) { _, dot in
                    Circle()
                        .fill(dot)
                        .frame(width: WalletGeometry.pillDot, height: WalletGeometry.pillDot)
                        .overlay(
                            Circle().stroke(theme.bgRaised, lineWidth: Tokens.BorderWidth.emphasis)
                        )
                }
            }
            Text(verbatim: pill.label)
                .typeRole(Typography.caption.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            LucideIcon(.chevronDown, size: LucideIconSize.smallChevron)
                .foregroundStyle(theme.fgMuted)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .frame(height: WalletGeometry.pillHeight)
        .background(Capsule().fill(theme.bgRaised))
        .accessibilityElement(children: .combine)
    }
}

/// The filled search field (component 4) — R1's network search, T1's and SD1's
/// token search, SD2e's contact search.
///
/// Filtering is live and animation-free by design (SPEC 动效 · 收款): rows leave
/// as the query narrows, and a transition on a list that changes on every
/// keystroke reads as lag rather than as polish.
struct FlowSearchField: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let placeholder: String
    @Binding var text: String

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            LucideIcon(.search, size: LucideIconSize.sheetSearch)
                .foregroundStyle(theme.fgSubtle)
            TextField(
                "",
                text: $text,
                prompt: Text(verbatim: placeholder).foregroundStyle(theme.fgSubtle)
            )
            .font(Typography.body.scaled(textScale).font)
            .foregroundStyle(theme.fgBase)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .accessibilityLabel(placeholder)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .frame(height: Tokens.Control.lg)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
    }
}

/// The two-segment toggle (component 5) — T3's ERC-20 / native tabs and SD2c's
/// fiat / token pricing switch.
///
/// The design review made this the ONE segmented control in the product, so it
/// takes its segments as data rather than growing a variant per caller.
struct FlowSegmentedToggle: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let options: [(id: String, label: String)]
    let selectedId: String
    var onSelect: (String) -> Void = { _ in }

    var body: some View {
        HStack(spacing: Tokens.Space.s2) {
            ForEach(options, id: \.id) { option in
                let on = option.id == selectedId
                Button { onSelect(option.id) } label: {
                    Text(verbatim: option.label)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(on ? theme.fgBase : theme.fgMuted)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Tokens.Space.s8)
                        .background(
                            RoundedRectangle(cornerRadius: Tokens.Radius.r8)
                                .fill(on ? theme.bgRaised : .clear)
                        )
                        .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(Tokens.Space.s2)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgSunken))
    }
}

/// SD1's token-class filter chips (component 6).
///
/// Distinct from `FlowSegmentedToggle` on purpose. That control divides ONE
/// space into named halves and always fills its width; this is a row of
/// independent narrowings that hugs its labels and scrolls past the screen edge
/// when a locale needs the room.
struct FlowFilterChips: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let options: [FilterChipModel]
    var onSelect: (String) -> Void = { _ in }

    var body: some View {
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: Tokens.Space.s4) {
                ForEach(options) { option in
                    Button { onSelect(option.id) } label: {
                        Text(verbatim: option.label)
                            .typeRole(Typography.chip.scaled(textScale))
                            // The selected chip inverts rather than taking the
                            // accent: accent means "moves money" in this
                            // product, and narrowing a list does not.
                            .foregroundStyle(option.selected ? theme.bgBase : theme.fgMuted)
                            .padding(.horizontal, Tokens.Space.s12)
                            .padding(.vertical, Tokens.Space.s8)
                            .background(
                                Capsule().fill(option.selected ? theme.fgBase : theme.bgRaised)
                            )
                            .contentShape(Capsule())
                    }
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

/// The monospace field (component 7): T3's contract address, T3b's network
/// query, SD2c's pasted recipient list.
///
/// Addresses are compared character by character by the people pasting them,
/// which is the reason for the mono face — and the reason the error state
/// colours the BORDER and prints underneath rather than tinting the text,
/// which would make the characters harder to read exactly when they most need
/// reading.
/// The same field, typed into.
///
/// A field somebody cannot type into is a picture of a field, and spec 021's
/// flows layer was drawn as pictures. This keeps every drawn property — the
/// mono role, the filled ground, the error stroke and the message under it —
/// and adds the one thing the mock could not show: a cursor.
///
/// It exists beside `FlowMonoField` rather than replacing it so the gallery
/// boards and the screenshot sweep keep rendering the picture (FR-004).
struct FlowMonoInput: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    @Binding var value: String
    var label: String?
    var placeholder: String = ""
    var error: String?
    /// The well behind the field. The default is the send form's, which sits
    /// over `bgBase`; a field on a RAISED surface needs the sunken one, or the
    /// well and its surface are the same colour and the field disappears —
    /// which is exactly what C7 showed.
    var well: Color?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            if let label {
                Text(verbatim: label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
            TextField(
                "",
                text: $value,
                prompt: Text(verbatim: placeholder).foregroundStyle(theme.fgSubtle)
            )
            // `monoRole` is a `Text` extension (the sanctioned styling seam);
            // a `TextField` takes the same role's font directly.
            .font(Typography.monoAddressDetail.scaled(textScale).font)
            .foregroundStyle(theme.fgBase)
            .textInputAutocapitalization(.never)
            .autocorrectionDisabled()
            .textContentType(.oneTimeCode)
            .keyboardType(.asciiCapable)
            .submitLabel(.done)
            .frame(maxWidth: .infinity, alignment: .leading)
            .padding(Tokens.Space.s12)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(well ?? theme.bgRaised))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .stroke(
                        error == nil ? .clear : theme.errorBase,
                        lineWidth: Tokens.BorderWidth.hairline
                    )
            )
            .accessibilityLabel(Text(verbatim: label ?? placeholder))
            if let error {
                Text(verbatim: error)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
            }
        }
    }
}

struct FlowMonoField: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let value: String
    var label: String?
    var error: String?
    var lineLimit: Int = 1

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            if let label {
                Text(verbatim: label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
            Text(verbatim: value)
                .monoRole(Typography.monoAddressDetail.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .lineLimit(lineLimit, reservesSpace: lineLimit > 1)
                .frame(maxWidth: .infinity, alignment: .leading)
                .padding(Tokens.Space.s12)
                .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
                .overlay(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                        .stroke(
                            error == nil ? .clear : theme.errorBase,
                            lineWidth: Tokens.BorderWidth.hairline
                        )
                )
            if let error {
                Text(verbatim: error)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
            }
        }
    }
}

/// The hairline that separates rows in every list in the flows.
struct FlowDivider: View {
    @Environment(\.theme) private var theme

    var body: some View {
        Rectangle()
            .fill(theme.borderBase)
            .frame(height: Tokens.BorderWidth.hairline)
    }
}
