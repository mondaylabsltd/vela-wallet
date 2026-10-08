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

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: "\(model.label) · \(model.currency)")
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
            amount
            if let live = model.liveText {
                liveRow(live)
            }
            if let status = model.status {
                statusDoor(status)
            }
            if let refresh = model.refresh {
                BalanceRefreshControl(model: refresh, onRefresh: onRefresh)
            }
        }
    }

    @ViewBuilder private var amount: some View {
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
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.successBase)
        }
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
        } else {
            statusRow(status)
        }
    }

    private func statusRow(_ status: BalanceStatusModel) -> some View {
        let tint = status.kind == .warning ? theme.warningBase : theme.fgMuted
        return HStack(spacing: Tokens.Space.s8) {
            LucideIcon(status.kind == .warning ? .triangleAlert : .refreshCw, size: LucideIconSize.statusIcon)
                .foregroundStyle(tint)
            Text(verbatim: status.text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(tint)
            LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                .foregroundStyle(theme.fgSubtle)
        }
    }
}

/// The hero's "↻ Updated 2m" (issue 462) — the same control on all four
/// shells, under the total and its status line.
///
/// Tapping it reads every chain again (the caller's `onRefresh`:
/// `RefreshRequested{force, pull}` plus the activity tick). While that is out
/// (`model.refreshing`, which the store holds for at least 650 ms) the glyph
/// turns, the words read "Updating…", and a second tap does nothing.
///
/// **Nothing moves.** Both labels are laid out in one box, the one not
/// showing invisible, so the box is the wider one's width in both states and
/// its height never changes: the control does not slide out from under the
/// finger that tapped it. Quiet ink like the status line — a fresh figure is
/// the normal case.
struct BalanceRefreshControl: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

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
        .accessibilityLabel(Text(verbatim: model.refreshing ? model.updating : (model.updated ?? model.updating)))
        .accessibilityAddTraits(.isButton)
        .accessibilityAddTraits(model.refreshing ? .updatesFrequently : [])
        .accessibilityIdentifier(Self.testId)
    }

    @ViewBuilder private var glyph: some View {
        if model.refreshing {
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
