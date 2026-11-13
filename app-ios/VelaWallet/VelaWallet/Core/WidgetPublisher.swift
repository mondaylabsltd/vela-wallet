//
//  WidgetPublisher.swift
//  VelaWallet
//
//  The app's side of the widgets (native parity N-I2): the hero the home
//  just drew, as the snapshot the widget extension reads.
//
//  It is made from the SAME `BalanceModel` the hero draws (`WalletLive`),
//  so the widget can never show a figure, a mask or a "still updating" the
//  home did not. Nothing here reads a chain, a price or a key; the snapshot
//  carries words the corpus already said and the figure's own digits.
//
//  Written only when it changes, and the timelines reloaded then — a widget
//  is redrawn when there is something new to draw, never on a timer.
//

import Foundation
import WidgetKit

enum WidgetPublisher {
    /// The snapshot for the hero `balance`, read at `asOfMs`.
    ///
    /// - The figure only when the hero draws one (`normal`, `zeroLive`).
    /// - Hidden is the mask; the digits are not written at all.
    /// - Loading / not yet read: no figure — and the hero's own line
    ///   ("Checking…") when it has one, so the widget says the same.
    /// - The testnet space: its words in place of a total.
    static func snapshot(
        balance: BalanceModel,
        asOfMs: Double?,
        accountName: String?,
        loc: Loc
    ) -> WidgetSnapshot {
        let figure: WidgetSnapshot.Figure
        var spoken: String
        switch balance.state {
        case .normal, .zeroLive:
            if let integer = balance.integer, !integer.isEmpty {
                figure = .amount
                spoken = "\(integer).\(balance.decimals ?? "") \(balance.currency)"
            } else {
                figure = .none
                spoken = balance.label
            }
        case .hidden:
            figure = .hidden
            spoken = balance.a11yShow
        case .loading, .noValue:
            figure = .none
            spoken = balance.label
        }
        let warning = balance.status?.kind == .warning
        let note: String? = {
            if let checking = balance.checkingText { return checking }
            if balance.state == .noValue { return balance.noValueText }
            // Only the hero's warning is the widget's to repeat: "Updating…"
            // under a refresh the person asked for is a moment in the app.
            if warning { return balance.status?.text }
            return nil
        }()
        return WidgetSnapshot(
            title: balance.label,
            currency: balance.currency,
            figure: figure,
            integer: figure == .amount ? balance.integer : nil,
            decimals: figure == .amount ? balance.decimals : nil,
            spoken: spoken,
            partial: figure == .amount && warning,
            note: note,
            // A figure not read yet has no time, and a mask needs none.
            asOf: figure == .amount ? asOfMs.map { Date(timeIntervalSince1970: $0 / 1000) } : nil,
            updatedLabel: loc.t(I18nKeys.Widgets.updated),
            accountName: accountName.flatMap { $0.isEmpty ? nil : $0 },
            sendLabel: loc.t(VelaEntry.send.titleKey),
            receiveLabel: loc.t(VelaEntry.receive.titleKey),
            scanLabel: loc.t(VelaEntry.scan.titleKey),
            language: loc.resolvedLanguage
        )
    }

    /// Write `snapshot` when it differs from the last written, and redraw
    /// the widgets. `nil` withdraws it: nobody is signed in.
    @MainActor
    static func publish(_ snapshot: WidgetSnapshot?, store given: WidgetSnapshotStore? = nil,
                        reload: () -> Void = { WidgetCenter.shared.reloadAllTimelines() }) {
        let store = given ?? .shared
        let current = store.read()
        guard current != snapshot else { return }
        if let snapshot {
            guard store.write(snapshot) else { return }
        } else {
            guard current != nil else { return }
            store.clear()
        }
        reload()
    }
}
