//
//  BalanceDisplay.swift
//  VelaWallet
//
//  BalanceDisplay + BalanceStatusLine (spec 015 vocabulary #4/#5): label
//  line (总余额 · USD), hero amount with de-emphasised decimals, exactly
//  one of normal / zero-live (pulsing dot + 实时·监听收款中) / loading
//  (skeleton block) / hidden (six dots + eye-off), plus an optional
//  warning/refreshing status line, then the refresh control (issue 462).
//

import SwiftUI

struct BalanceDisplay: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: BalanceModel
    /// Where the status line goes. Absent in the gallery, where the line is a
    /// picture of a state rather than a way out of one.
    var onStatusTap: (() -> Void)?
    /// Issue 462: the refresh control's tap. Absent in the gallery, where the
    /// control is drawn and takes no tap.
    var onRefresh: (() -> Void)?
    /// Spec 051's "tap the figure to hide it", as VoiceOver hears it: the
    /// button trait, the "Hide balance" hint and the action sit on the FIGURE
    /// alone. This stack is not one accessibility element, so a hint set on
    /// it reached every element inside — and overrode their own: the refresh
    /// control under the figure was announced "…, button, Hide balance",
    /// and double-tapping it refreshed. The sighted tap stays on the caller's
    /// stack. Absent in the gallery, where a tap would mutate a picture.
    var onToggle: (() -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            // The currency is named once it is known — the person's stored
            // choice while its figure is on its way, nothing before that.
            Text(verbatim: model.currency.isEmpty ? model.label : "\(model.label) · \(model.currency)")
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            amount
            // A drawing that carries both lines keeps both, as drawn.
            if let live = model.liveText, model.status != nil {
                liveRow(live)
            }
            statusSlot
            if let refresh = model.refresh {
                BalanceRefreshControl(model: refresh, onRefresh: onRefresh)
            }
        }
    }

    /// The ONE line under the figure, held from the first frame (PR 3 note
    /// 26b). "Can't reach …", "Some balances are still updating." arrive
    /// after the figure and go again, and each arrival pushed the refresh
    /// control, Receive / Send and every row under them down by the line's
    /// height and the gap above it — 24.7 pt on an iPhone 17 — and each
    /// departure pulled them back. The line's room is always there now: the
    /// first read's "Checking…" stands in it, or the status line, or a zero
    /// wallet's "live" line, or nothing, and the page under the hero is
    /// where it was in all four.
    ///
    /// **One line is what is held, and one line is all it ever is** (final
    /// note F16). A sentence longer than the line — "No podemos cargar la
    /// lista de tokens de Tempo por ahora", "Something went wrong inside
    /// Vela. If it keeps happening, reopen the app." — is set smaller, to
    /// 85 % and no lower, then cut with an ellipsis. It used to take a
    /// second line, which grew the room it was promised and pushed the page
    /// down after all. Nothing is lost: the whole sentence is what VoiceOver
    /// reads, and it stands in full at the top of the sheet the line opens.
    private var statusSlot: some View {
        ZStack(alignment: .leading) {
            // The room: a status row's own height, whatever stands in it.
            statusRow(BalanceStatusModel(kind: .warning, text: "0"))
                .hidden()
                .accessibilityHidden(true)
            // "Checking…" stands alone (final note F19): the first read is
            // out, so there is no reason to give yet and no "live" to claim.
            if let checking = model.checkingText {
                checkingRow(checking)
            } else if let status = model.status {
                statusDoor(status)
            } else if let live = model.liveText {
                liveRow(live)
            }
        }
    }

    private var amount: some View {
        ZStack(alignment: .leading) {
            heroLine
            // Hidden, the figure is already called "Show balance": no hint to add.
            figure.modifier(BalanceToggleA11y(
                hint: model.state == .hidden ? nil : model.a11yHide, onToggle: onToggle
            ))
        }
    }

    /// The figure's own line, held in every state (PR 2 polish): the mask and
    /// the skeleton stand in the height the figure takes, so hiding or showing
    /// the balance — or the first figure landing — moves nothing under it. The
    /// mask's row was 9 pt shorter than the figure on an iPhone 11, and Receive,
    /// Send and the activity jumped by that much at every toggle.
    private var heroLine: some View {
        (Text(verbatim: "0")
            .font(Typography.amountHero.scaled(textScale).font)
            + Text(verbatim: ".00")
            .font(Typography.amountHeroDecimals.scaled(textScale).font))
            .lineLimit(1)
            .hidden()
            .accessibilityHidden(true)
    }

    @ViewBuilder private var figure: some View {
        switch model.state {
        case .loading:
            SkeletonBlock(width: WalletGeometry.skeletonBalanceWidth, height: WalletGeometry.skeletonBalanceHeight)
        case .hidden:
            HStack(spacing: Tokens.Space.s16) {
                HStack(spacing: Tokens.Space.s8) {
                    ForEach(0..<WalletGeometry.hiddenDotCount, id: \.self) { _ in
                        Circle()
                            .fill(theme.fgBase)
                            .frame(width: WalletGeometry.hiddenDot, height: WalletGeometry.hiddenDot)
                    }
                }
                LucideIcon(.eyeOff, size: LucideIconSize.eye)
                    .foregroundStyle(theme.fgMuted)
            }
            .frame(minHeight: WalletGeometry.skeletonBalanceHeight)
            .accessibilityElement(children: .ignore)
            .accessibilityLabel(Text(verbatim: model.a11yShow))
        case .normal, .zeroLive:
            (Text(verbatim: model.integer ?? "")
                .font(Typography.amountHero.scaled(textScale).font)
                .foregroundStyle(theme.fgBase)
                + Text(verbatim: model.decimals.map { ".\($0)" } ?? "")
                .font(Typography.amountHeroDecimals.scaled(textScale).font)
                .foregroundStyle(theme.fgMuted))
                .lineLimit(1)
                .minimumScaleFactor(WalletGeometry.heroMinScale)
                .accessibilityLabel(Text(verbatim: "\(model.integer ?? "").\(model.decimals ?? "") \(model.currency)"))
        }
    }

    private func liveRow(_ text: String) -> some View {
        HStack(spacing: Tokens.Space.s8) {
            PulsingDot(color: theme.successBase)
            oneLine(text)
                .foregroundStyle(theme.successBase)
        }
    }

    /// The line's words, on ONE line: smaller down to 85 %, then "…". The
    /// whole sentence stays its accessibility label.
    private func oneLine(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub.scaled(textScale))
            .lineLimit(1)
            .minimumScaleFactor(WalletGeometry.statusMinScale)
            .truncationMode(.tail)
            .accessibilityLabel(Text(verbatim: text))
    }

    /// "Checking…" — the live line before anything has answered: the same
    /// dot, not green yet, and quiet ink. No chevron and no tap: there is
    /// nothing to open about a read that has not come back.
    private func checkingRow(_ text: String) -> some View {
        HStack(spacing: Tokens.Space.s8) {
            PulsingDot(color: theme.fgSubtle)
            oneLine(text)
                .foregroundStyle(theme.fgMuted)
        }
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: text))
        .accessibilityIdentifier("balance-checking")
    }

    /// The line, as a control where there is somewhere to go.
    ///
    /// It has always drawn a chevron — a promise of a destination — and had
    /// none behind it (row 12 of 057's audit). Android and the web agree on
    /// what it opens: a failed chain's RPC fix, anything else the breakdown.
    @ViewBuilder private func statusDoor(_ status: BalanceStatusModel) -> some View {
        if let onStatusTap {
            Button(action: onStatusTap) {
                statusRow(status).contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            // The sentence in full, however much of it the line shows.
            .accessibilityLabel(Text(verbatim: status.text))
            .accessibilityIdentifier(Self.statusId)
        } else {
            statusRow(status)
                .accessibilityElement(children: .ignore)
                .accessibilityLabel(Text(verbatim: status.text))
                .accessibilityIdentifier(Self.statusId)
        }
    }

    /// The status line's test hook.
    static let statusId = "balance-status"

    private func statusRow(_ status: BalanceStatusModel) -> some View {
        let tint = status.kind == .warning ? theme.warningBase : theme.fgMuted
        return HStack(spacing: Tokens.Space.s8) {
            LucideIcon(status.kind == .warning ? .triangleAlert : .refreshCw, size: LucideIconSize.statusIcon)
                .foregroundStyle(tint)
            oneLine(status.text)
                .foregroundStyle(tint)
            LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                .foregroundStyle(theme.fgSubtle)
        }
    }
}

/// The figure's hide/show switch for VoiceOver (`BalanceDisplay.onToggle`).
private struct BalanceToggleA11y: ViewModifier {
    let hint: String?
    let onToggle: (() -> Void)?

    func body(content: Content) -> some View {
        if let onToggle {
            content
                .accessibilityAddTraits(.isButton)
                .accessibilityHint(Text(verbatim: hint ?? ""))
                .accessibilityAction { onToggle() }
        } else {
            content
        }
    }
}

/// The hero's "↻ Updated 2m" (issue 462) — the same control on all four
/// shells, under the total and its status line.
///
/// Tapping it reads every chain again (the caller's `onRefresh`:
/// `RefreshRequested{force, pull}` plus the activity tick). While that is out
/// (`model.refreshing`, which the store holds for at least 650 ms) the glyph
/// turns, the words read "Updating…", and a second tap does nothing. Under
/// Reduce Motion the glyph stays still and the words alone say it is reading,
/// as on the web. At rest
/// before any read has settled it draws the glyph alone, and is called
/// "Refresh balance" (`home.refreshBalance`) — never "Updating…" over a
/// control that is not.
///
/// **Nothing moves.** Both labels are laid out in one box, the one not
/// showing invisible, so the box is the wider one's width in both states and
/// its height never changes: the control does not slide out from under the
/// finger that tapped it. Quiet ink like the status line — a fresh figure is
/// the normal case.
struct BalanceRefreshControl: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion

    let model: BalanceRefreshModel
    var onRefresh: (() -> Void)?

    /// The control's test hook — the desktop's element id for the same control.
    static let testId = "balance-refresh"

    var body: some View {
        Button { if !model.refreshing { onRefresh?() } } label: {
            HStack(spacing: Tokens.Space.s8) {
                glyph
                ZStack(alignment: .leading) {
                    label(model.updated ?? "", shown: !model.refreshing)
                    label(model.updating, shown: model.refreshing)
                }
            }
            // The words swap at once. The press's spring rides the same
            // transaction as the tap's state change, and cross-faded the two
            // labels into each other ("更新中新 · 刚刚") — on a simulator run.
            .transaction { $0.animation = nil }
            .frame(minHeight: Tokens.Control.sm)
            .contentShape(Rectangle())
        }
        // Inert while it turns — and in the gallery — without dimming: a
        // refresh in flight is busy, not disabled. The tap is still taken
        // (and dropped) rather than let through: under the control the hero
        // hides the figure on a tap.
        .buttonStyle(RefreshPressStyle(live: onRefresh != nil && !model.refreshing))
        .accessibilityElement(children: .ignore)
        .accessibilityLabel(Text(verbatim: Self.spoken(model)))
        // "Updated 2m" names when, not what a double-tap does.
        .accessibilityHint(Text(verbatim: model.refreshing || model.updated == nil ? "" : model.named))
        .accessibilityAddTraits(.isButton)
        .accessibilityAddTraits(model.refreshing ? .updatesFrequently : [])
        .accessibilityIdentifier(Self.testId)
    }

    /// Whether the glyph turns: while a refresh is out, never under Reduce
    /// Motion — "Updating…" says it there.
    static func turns(_ model: BalanceRefreshModel, reduceMotion: Bool) -> Bool {
        model.refreshing && !reduceMotion
    }

    /// What VoiceOver calls the control: "Updating…" only while it turns.
    static func spoken(_ model: BalanceRefreshModel) -> String {
        model.refreshing ? model.updating : (model.updated ?? model.named)
    }

    @ViewBuilder private var glyph: some View {
        if Self.turns(model, reduceMotion: reduceMotion) {
            // A clock-driven turn, composed only while turning: an idle home
            // runs no frame clock, and no implicit animation can leak into the
            // hero's layout. One revolution at the CTA spinner's speed.
            TimelineView(.animation) { context in
                let period = Tokens.Motion.slow * 2
                let turn = context.date.timeIntervalSinceReferenceDate
                    .truncatingRemainder(dividingBy: period) / period
                LucideIcon(.refreshCw, size: LucideIconSize.statusIcon)
                    .foregroundStyle(theme.fgSubtle)
                    .rotationEffect(.degrees(turn * 360))
            }
        } else {
            LucideIcon(.refreshCw, size: LucideIconSize.statusIcon)
                .foregroundStyle(theme.fgSubtle)
        }
    }

    private func label(_ text: String, shown: Bool) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub.scaled(textScale))
            .foregroundStyle(theme.fgSubtle)
            .lineLimit(1)
            .opacity(shown ? 1 : 0)
    }
}

/// The refresh control's press: it dims and taps back like every text button
/// here while it can be pressed, and answers nothing while it cannot. No
/// scale — the control must not move under the finger.
private struct RefreshPressStyle: ButtonStyle {
    let live: Bool

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .opacity(live && configuration.isPressed ? Interaction.pressedOpacity : 1)
            .animation(Interaction.pressSpring, value: configuration.isPressed)
            .onChange(of: configuration.isPressed) { _, pressed in
                if live && pressed { VelaHaptic.press.play() }
            }
    }
}

/// Zero-live indicator dot — opacity pulse, static under Reduce Motion.
private struct PulsingDot: View {
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var dimmed = false
    let color: Color

    var body: some View {
        Circle()
            .fill(color)
            .frame(width: WalletGeometry.liveDot, height: WalletGeometry.liveDot)
            .opacity(dimmed && !reduceMotion ? Tokens.Opacity.dim : 1)
            .animation(
                reduceMotion ? nil : .easeInOut(duration: Tokens.Motion.slow).repeatForever(autoreverses: true),
                value: dimmed
            )
            .onAppear { dimmed = true }
    }
}

#Preview("Balance states dark") {
    VStack(alignment: .leading, spacing: Tokens.Space.s24) {
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .normal,
            integer: "$1,383", decimals: "28", liveText: nil, status: nil,
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .zeroLive,
            integer: "$0", decimals: "00", liveText: "实时 · 监听收款中", status: nil,
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .loading,
            integer: nil, decimals: nil, liveText: nil, status: nil,
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .hidden,
            integer: "••••••", decimals: nil, liveText: nil, status: nil,
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .normal,
            integer: "$1,383", decimals: "46", liveText: nil,
            status: BalanceStatusModel(kind: .warning, text: "部分代币无法获取价格。"),
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
        BalanceDisplay(model: BalanceModel(
            label: "总余额", currency: "USD", state: .normal,
            integer: "$1,383", decimals: "28", liveText: nil,
            status: BalanceStatusModel(kind: .refreshing, text: "部分余额仍在更新。"),
            a11yHide: "隐藏余额", a11yShow: "显示余额"
        ))
    }
    .padding(Tokens.Space.s24)
    .frame(maxWidth: .infinity, alignment: .leading)
    .background(Tokens.dark.bgBase.color)
    .themed(.dark)
}

#Preview("Balance light") {
    BalanceDisplay(model: BalanceModel(
        label: "Total balance", currency: "USD", state: .normal,
        integer: "$1,383", decimals: "28", liveText: nil, status: nil,
        a11yHide: "Hide balance", a11yShow: "Show balance"
    ))
    .padding(Tokens.Space.s24)
    .themed(.light)
}
