//
//  BalanceDisplay.swift
//  VelaWallet
//
//  BalanceDisplay + BalanceStatusLine (spec 015 vocabulary #4/#5): label
//  line (总余额 · USD), hero amount with de-emphasised decimals, exactly
//  one of normal / zero-live (pulsing dot + 实时·监听收款中) / loading
//  (skeleton block) / hidden (six dots + eye-off), plus an optional
//  warning/refreshing status line.
//

import SwiftUI

struct BalanceDisplay: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: BalanceModel
    /// Where the status line goes. Absent in the gallery, where the line is a
    /// picture of a state rather than a way out of one.
    var onStatusTap: (() -> Void)?

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
