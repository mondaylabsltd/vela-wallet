//
//  RelayReportSheet.swift
//  VelaWallet
//
//  Issue #466: a relay stop's "Report this", over the send screen it was
//  tapped on.
//
//  It is the app's EXISTING report sheet (Settings → 反馈's), seeded with the
//  report the core built for the stop — the title line, the treasury, what it
//  has against its floor, the steps — and filed under the core's area and
//  fingerprint, so every report of one outage lands on one issue as a +1.
//  The person reads it, may edit it, and sends it; the endpoint's refusals
//  fall back to the prefilled GitHub form exactly as Settings' do.
//
//  ONE sheet, over the flow's base screen: the button lives on the form and on
//  confirm, never on a sheet, so nothing is stacked. The sender outlives the
//  sheet (as Settings' does): a report whose sheet was closed mid-send still
//  lands, and its outcome is said here as the toast Settings shows.
//

import SwiftUI

struct RelayReportSheet: ViewModifier {
    /// The report, as snapshotted at the tap; `nil` when no sheet is up.
    @Binding var seed: BugReport.Seed?
    let sender: FeedbackSender
    /// The settings model the sheet's words and preview lines come from —
    /// the same `withFeedback` lines Settings' sheet shows and sends.
    let model: () -> SettingsScreenModel
    let scheme: ColorScheme

    @State private var toast: FeedbackOutcomeToast.Model?
    @Environment(\.openURL) private var openURL

    func body(content: Content) -> some View {
        content
            .sheet(item: $seed) { shown in
                SettingsSheet(
                    model: model(),
                    overlay: .feedback,
                    onDismiss: { seed = nil },
                    onSignOut: {},
                    feedbackSender: sender,
                    feedbackSeed: shown
                )
                .themed(scheme)
            }
            .onChange(of: sender.state) { _, state in
                // Only when the sheet is gone: an open sheet says it itself.
                guard seed == nil,
                      let outcome = FeedbackOutcomeToast.Model.from(state, words: model().feedback)
                else { return }
                show(outcome)
            }
            .overlay(alignment: .bottom) {
                if let toast {
                    FeedbackOutcomeToast(
                        model: toast,
                        closeLabel: model().closeLabel,
                        onAction: {
                            self.toast = nil
                            if let url = URL(string: toast.url) { openURL(url) }
                        },
                        onClose: { self.toast = nil }
                    )
                    .padding(.horizontal, Tokens.Space.s16)
                    .padding(.bottom, Tokens.Space.s12)
                    .transition(.move(edge: .bottom).combined(with: .opacity))
                }
            }
    }

    /// Settings' rule: a filed report goes quietly after a while; a fallback
    /// is the only road left for that report, so it waits for the person.
    private func show(_ outcome: FeedbackOutcomeToast.Model) {
        withAnimation(.easeOut(duration: Tokens.Motion.fast)) { toast = outcome }
        (outcome.success ? VelaHaptic.success : VelaHaptic.reject).play()
        UIAccessibility.post(notification: .announcement, argument: outcome.title)
        guard outcome.success else { return }
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(8))
            if toast == outcome {
                withAnimation(.easeIn(duration: Tokens.Motion.fast)) { toast = nil }
            }
        }
    }
}
