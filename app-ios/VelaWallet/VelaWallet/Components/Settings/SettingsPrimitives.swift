//
//  SettingsPrimitives.swift
//  VelaWallet
//
//  The settings vocabulary's smallest pieces (spec 023). Every one of the
//  forty mocks in `design/settings/` is assembled from these plus the rows
//  beside them. Nothing here reads a model bigger than it draws, and nothing
//  formats.
//

import SwiftUI

/// The one badge every settings screen uses. Latency, reachability, provider
/// state and compatibility are all this object in the mocks, differing only in
/// tone — so they are one component and not four.
struct StatusPill: View {
    @Environment(\.theme) private var theme
    let pill: StatusPillModel

    private var colors: (fg: Color, bg: Color) {
        switch pill.tone {
        case .ok: (theme.successBase, theme.successSoft)
        case .warn: (theme.warningBase, theme.warningSoft)
        case .error: (theme.errorBase, theme.errorSoft)
        // Unset, not failed — the mocks grey these rather than colouring them.
        case .neutral: (theme.fgSubtle, theme.bgRaised)
        }
    }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            if pill.dot {
                Circle().fill(colors.fg).frame(width: 6, height: 6)
            }
            Text(pill.label)
                .typeRole(Typography.label)
                .foregroundStyle(colors.fg)
        }
        .padding(.horizontal, Tokens.Space.s8)
        .padding(.vertical, Tokens.Space.s2)
        .background(colors.bg, in: Capsule())
    }
}

/// The tinted explanation box. Eight mocks use it; `.success` swaps the
/// triangle for a check, because a green triangle reads as an alarm.
struct SettingsCallout: View {
    @Environment(\.theme) private var theme
    let callout: CalloutModel

    private var style: (fg: Color, bg: Color, glyph: LucideGlyph) {
        switch callout.tone {
        case .warning: (theme.warningBase, theme.warningSoft, .triangleAlert)
        case .danger: (theme.errorBase, theme.errorSoft, .triangleAlert)
        case .info: (theme.infoBase, theme.infoSoft, .info)
        case .success: (theme.successBase, theme.successSoft, .check)
        }
    }

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(style.glyph, size: LucideIconSize.rowGlyph)
                .foregroundStyle(style.fg)
                // Optical alignment with the first line, not the box.
                .padding(.top, Tokens.Space.s2)
            Text(callout.text)
                .typeRole(Typography.body)
                .foregroundStyle(style.fg)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(Tokens.Space.s12)
        .background(style.bg, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
    }
}

/// A chain's circular avatar — its own logo when the chain-data endpoint has
/// one (spec 047), else one letter over its brand colour, which is also what
/// shows while the logo loads.
struct ChainMark: View {
    let mark: ChainMarkModel
    var size: CGFloat = 32

    var body: some View {
        RemoteLogoView(urls: mark.logoUrl.map { [$0] } ?? [], size: size) {
            Circle()
                .fill(mark.color)
                .frame(width: size, height: size)
                .overlay(
                    Text(mark.letter)
                        .typeRole(Typography.body)
                        .fontWeight(.bold)
                        .foregroundStyle(.white)
                )
        }
    }
}

/// The hairline the mocks draw between rows.
struct SettingsDivider: View {
    @Environment(\.theme) private var theme

    var body: some View {
        Rectangle().fill(theme.borderBase).frame(height: 1)
    }
}

/// The small caps label above a group of rows — 外观 / 区域格式 / 高级. 高级 is
/// the one that collapses (ST1b), so the chevron is optional and the whole
/// label becomes tappable only when it is present.
struct SettingsSectionLabel: View {
    @Environment(\.theme) private var theme
    let label: String
    var collapsible = false
    var collapsed = false
    var onToggle: () -> Void = {}

    var body: some View {
        HStack {
            Text(label)
                .typeRole(Typography.body)
                .foregroundStyle(theme.fgSubtle)
            Spacer()
            if collapsible {
                LucideIcon(collapsed ? .chevronDown : .chevronRight, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.top, Tokens.Space.s20)
        .padding(.bottom, Tokens.Space.s8)
        .contentShape(Rectangle())
        .onTapGesture { if collapsible { onToggle() } }
    }
}

/// A labelled mono field. Every endpoint on ST9b / ST11 / ST12 / SR2 / SR5 is
/// one of these: a label row that may carry a latency pill, the value in a
/// sunken box, an optional in-field action, and an optional hint under it.
struct SettingsUrlField: View {
    @Environment(\.theme) private var theme
    let field: UrlFieldModel

    /// Whether this box is the one being typed into.
    ///
    /// Spec 081: the doc on `onCommit` below has always said "on submit **or
    /// on losing focus**", and only `.onSubmit` was wired — so a person who
    /// typed an endpoint and tapped anywhere else had saved nothing, while the
    /// health badge went on reporting the OLD host as online. Only the
    /// keyboard's Done key committed. The core persists on `EndpointBlurred`;
    /// `EndpointEdited` sets a draft and nothing more.
    @FocusState private var focused: Bool

    /// The editable mode (spec 050).
    ///
    /// **`nil` is the drawn state and renders exactly as it always has** — a
    /// `Text` in a bordered box, byte-for-byte the same view for every gallery
    /// board and every fixture call site, which is what keeps the screenshot
    /// sweep meaningful.
    ///
    /// Passing a binding turns the same box into a `TextField` with the same
    /// type role, the same colours and the same placeholder. That is the whole
    /// of the control this feature adds: the wizard was drawn in full — ST10,
    /// ST10b 兼容, ST10c 不兼容 — with no editable field anywhere, so it could
    /// be shown and could not be used, and `network_admin`'s sixteen operations
    /// had no way to receive a chain id from a person.
    ///
    /// An atom the drawing was missing, not a screen the drawing never had.
    var text: Binding<String>?
    /// Committed on submit or on losing focus — where the core runs its probes.
    var onCommit: () -> Void = {}

    private var border: Color {
        switch field.tone {
        case .error: theme.errorBase
        case .ok: theme.successBase
        // A hairline even at rest: on dark, sunken and base are one step apart
        // and the box would otherwise have no edge at all.
        default: theme.borderBase
        }
    }

    /// The mono face both branches wear, defined once.
    ///
    /// The editable and read-only halves must be typographically identical, or
    /// the box would visibly change shape the moment a screen went live. Written
    /// as `Font.system` rather than hidden behind implicit-member syntax so the
    /// literal audit still counts it — one honest violation instead of two.
    private var monoFace: Font {
        Font.system(size: Tokens.TextSize.t13, design: .monospaced)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            if !field.label.isEmpty || field.badge != nil {
                HStack {
                    if !field.label.isEmpty {
                        Text(field.label)
                            .typeRole(Typography.label)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer()
                    if let badge = field.badge { StatusPill(pill: badge) }
                }
            }
            HStack(spacing: Tokens.Space.s8) {
                if let text {
                    TextField(field.placeholder ?? "", text: text)
                        .font(monoFace)
                        .foregroundStyle(theme.fgBase)
                        .textInputAutocapitalization(.never)
                        .autocorrectionDisabled()
                        // A chain id is digits and an RPC is a URL; neither
                        // wants a capitalising, autocorrecting keyboard.
                        .keyboardType(.URL)
                        .submitLabel(.done)
                        .onSubmit(onCommit)
                        .focused($focused)
                        .onChange(of: focused) { wasFocused, isFocused in
                            // Losing focus IS the blur the core waits for.
                            if wasFocused && !isFocused { onCommit() }
                        }
                        .frame(maxWidth: .infinity, alignment: .leading)
                } else {
                    Text(field.value.isEmpty ? (field.placeholder ?? "") : field.value)
                        .font(monoFace)
                        .foregroundStyle(field.value.isEmpty ? theme.fgSubtle : theme.fgBase)
                        .lineLimit(1)
                        .truncationMode(.middle)
                        .frame(maxWidth: .infinity, alignment: .leading)
                }
                if let action = field.action {
                    Text(action)
                        .typeRole(Typography.body)
                        .foregroundStyle(theme.infoBase)
                }
            }
            .padding(.horizontal, Tokens.Space.s12)
            .frame(minHeight: 44)
            .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .strokeBorder(border, lineWidth: 1)
            )
            if let hint = field.hint {
                Text(hint)
                    .typeRole(Typography.body)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
    }
}

/// The product's ONE segmented control (design review 2026-07): three-up for
/// the theme picker, two-up for the avatar style.
struct SettingsSegmentedControl: View {
    @Environment(\.theme) private var theme
    let model: SegmentedModel
    var onSelect: (String) -> Void = { _ in }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            ForEach(model.segments) { segment in
                let selected = segment.id == model.selected
                HStack(spacing: Tokens.Space.s4) {
                    if let icon = segment.icon {
                        LucideIcon(icon, size: 14)
                            .foregroundStyle(selected ? theme.fgBase : theme.fgMuted)
                    }
                    Text(segment.label)
                        .typeRole(Typography.body)
                        .fontWeight(selected ? .semibold : .regular)
                        .foregroundStyle(selected ? theme.fgBase : theme.fgMuted)
                        .lineLimit(1)
                    // Three equal thirds of a 392pt screen do not hold "Follow
                    // System": clipping it to "Follow" is a different, wrong
                    // promise. Shrinking is the failure that still tells the truth.
                    .minimumScaleFactor(0.72)
                }
                .frame(maxWidth: .infinity, minHeight: 36)
                .background(
                    selected ? theme.bgRaised : Color.clear,
                    in: RoundedRectangle(cornerRadius: Tokens.Radius.r8)
                )
                // strokeBorder leaves the fill untouched, so the whole cell has
                // to be made hittable explicitly (button-feedback rule).
                .contentShape(Rectangle())
                .onTapGesture { onSelect(segment.id) }
            }
        }
        .padding(Tokens.Space.s4)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            // Dark mode sinks sunken BELOW raised, so the unselected track needs
            // a hairline to stay legible against bg.base.
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .strokeBorder(theme.borderBase, lineWidth: 1)
        )
        .accessibilityElement(children: .contain)
        .accessibilityLabel(model.label)
    }
}

/// A ——●—— A. The tick row plus the two glyph ends, sized to what they promise.
/// A picture of the control: spec 023 is UI only, so nothing moves yet.
struct TextScaleSlider: View {
    @Environment(\.theme) private var theme
    let model: TextScaleModel
    /// Which stop was chosen. Absent in the gallery, where the slider is a
    /// picture of a size already set.
    var onSelect: ((Int) -> Void)?

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            Text("A")
                .font(.system(size: Tokens.TextSize.t13, weight: .bold))
                .foregroundStyle(theme.fgBase)
            HStack {
                ForEach(0..<model.steps, id: \.self) { index in
                    Circle()
                        .fill(index == model.index ? theme.fgMuted : theme.borderStrong)
                        .frame(width: index == model.index ? 18 : 4,
                               height: index == model.index ? 18 : 4)
                        // A 4pt dot is not a target. The tappable area is the
                        // whole stop's share of the track, which is what a
                        // finger aims at anyway.
                        .frame(minWidth: Tokens.Layout.hitTarget,
                               minHeight: Tokens.Layout.hitTarget)
                        .contentShape(Rectangle())
                        .onTapGesture { onSelect?(index) }
                    if index < model.steps - 1 { Spacer() }
                }
            }
            Text("A")
                .font(.system(size: Tokens.TextSize.t20, weight: .bold))
                .foregroundStyle(theme.fgBase)
        }
        .padding(.vertical, Tokens.Space.s12)
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(model.label)
        // The stop, spoken and adjustable — a slider that only sighted fingers
        // can move is not a slider.
        .accessibilityValue(Text(verbatim: String(model.index + 1)))
        .accessibilityAdjustableAction { direction in
            let next = direction == .increment ? model.index + 1 : model.index - 1
            guard next >= 0, next < model.steps else { return }
            onSelect?(next)
        }
    }
}
