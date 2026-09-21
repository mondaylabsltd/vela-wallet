//
//  ClearSigner.swift
//  VelaWallet
//
//  The Clear Signer on the phone (spec 071): a signing method that is not a
//  place a passkey is, but a separate page that decodes the request from its
//  own bytes, derives the digest itself and only then runs the ceremony.
//
//  One ceremony is three things, and this file owns their lifetimes:
//
//  - a `ClearSignerChannel` — the loopback listener the page talks to;
//  - the page, in an `SFSafariViewController`. An in-app tab keeps this app in
//    the foreground, which is what keeps the listener alive for the whole
//    ceremony (research R2) — the system browser would suspend it;
//  - the waiting sheet under the tab: what is happening, what to do if the
//    browser asks about apps on this device, "open the page again" and cancel
//    (contract §5). Closing the tab lands on it rather than on a spinner.
//
//  Every verdict is the core's. What an ending MEANS for the request — closed
//  behaves like a cancelled passkey sheet, every refusal is shown, nothing is
//  submitted — is the spine's and the sheets'.
//

import SafariServices
import SwiftUI
import UIKit
import VelaCore

/// The Clear Signer as the spine reaches it — a port, so the money path can
/// be tested without a browser.
protocol ClearSignerPort: AnyObject {
    /// Opens the page for `requestJson` (`clearSignerRequest`'s) and waits for
    /// an answer the core verified over `digest` by one of `keys`.
    func sign(requestJson: String, digest: Data, keys: [WalletKeyRecord]) async -> ClearSignerChannel.Ending
}

/// The sentence an ended ceremony leaves on the sheet (contract §5). Four,
/// for eight refusals: a person needs to know whether they closed it, the page
/// refused, or the answer did not match — not which byte gave it away.
enum ClearSignerNotice: Equatable {
    /// Declined, closed, cancelled.
    case closed
    /// The page's own rules said no.
    case refused
    /// An answer the wallet would not accept: another digest, another key, a
    /// bad signature, no user verification, the wrong token, the wrong shape.
    case mismatch
    case timeout

    init(_ refusal: ClearSignerRefusal) {
        switch refusal {
        case .declined: self = .closed
        case .pageRefused: self = .refused
        case .wrongChallenge, .foreignKey, .badSignature, .notVerified, .wrongToken, .malformed:
            self = .mismatch
        }
    }

    var key: String {
        switch self {
        case .closed: "componentsUi.signing.clearSignerClosed"
        case .refused: "componentsUi.signing.clearSignerRefused"
        case .mismatch: "componentsUi.signing.clearSignerMismatch"
        case .timeout: "componentsUi.signing.clearSignerTimeout"
        }
    }
}

final class ClearSigner: NSObject, ClearSignerPort, SFSafariViewControllerDelegate {

    private let loc: Loc
    /// The page Settings names (`sign_pref`); `nil` before it has said, which
    /// is the official page.
    private let signerUrl: () -> String?

    private var channel: ClearSignerChannel?
    private var launchUrl: URL?
    private var waitingSheet: UIViewController?

    init(loc: Loc, signerUrl: @escaping () -> String?) {
        self.loc = loc
        self.signerUrl = signerUrl
        super.init()
    }

    func sign(requestJson: String, digest: Data, keys: [WalletKeyRecord]) async -> ClearSignerChannel.Ending {
        // One ceremony at a time. The waiting sheet is modal over everything,
        // so a second request cannot come from a person; one that comes
        // anyway is declined rather than stacking a page on a page.
        guard channel == nil else { return .outcome(.refused(refusal: .declined)) }
        let channel = ClearSignerChannel(
            signerUrl: signerUrl() ?? clearSignerDefaultUrl(),
            requestJson: requestJson, digest: digest, keys: keys
        )
        self.channel = channel
        defer { self.channel = nil }
        guard await channel.open() != nil, let url = channel.launchUrl else {
            return await channel.ending()
        }
        launchUrl = url
        present(url, for: channel)
        let ending = await channel.ending()
        dismiss()
        return ending
    }

    // MARK: - The waiting sheet's two buttons

    /// The same URL, the same port, the same token: the core accepts a new
    /// socket that proves itself while none has an outcome (contract §2).
    func reopen() {
        guard let url = launchUrl, let sheet = waitingSheet,
              sheet.presentedViewController == nil
        else { return }
        sheet.present(page(url), animated: true)
    }

    /// Declined — the request stays open, as after a cancelled passkey sheet.
    func cancel() {
        channel?.cancel()
    }

    // MARK: - Presentation

    private func present(_ url: URL, for channel: ClearSignerChannel) {
        guard let presenter = Self.presenter else {
            // No window to show a page in: the ceremony cannot happen, and
            // waiting five minutes for it would be a spinner with no page.
            channel.cancel()
            return
        }
        let scheme: ColorScheme = presenter.traitCollection.userInterfaceStyle == .dark ? .dark : .light
        let sheet = UIHostingController(rootView: ClearSignerWaitingSheet(
            title: loc.t("componentsUi.signing.clearSignerWaiting"),
            hint: loc.t("componentsUi.signing.clearSignerWaitingHint"),
            reopen: loc.t("componentsUi.signing.clearSignerReopen"),
            cancel: loc.t("common.cancel"),
            onReopen: { [weak self] in self?.reopen() },
            onCancel: { [weak self] in self?.cancel() }
        ).themed(scheme))
        sheet.modalPresentationStyle = .pageSheet
        sheet.sheetPresentationController?.detents = [.medium()]
        sheet.sheetPresentationController?.prefersGrabberVisible = false
        // Cancel is a button, not a swipe: a swipe that ended the ceremony
        // would be a decline nobody meant.
        sheet.isModalInPresentation = true
        waitingSheet = sheet
        // The sheet goes up under the page, unanimated, so what a person sees
        // is the page opening — and, when they close the tab, the sheet.
        presenter.present(sheet, animated: false) { [weak self] in
            guard let self else { return }
            sheet.present(page(url), animated: true)
        }
    }

    private func page(_ url: URL) -> SFSafariViewController {
        let configuration = SFSafariViewController.Configuration()
        configuration.barCollapsingEnabled = false
        let page = SFSafariViewController(url: url, configuration: configuration)
        page.dismissButtonStyle = .close
        page.delegate = self
        return page
    }

    /// The tab and the sheet under it, together. Dismissed from BELOW: asking
    /// the sheet itself would only close the tab it presents.
    private func dismiss() {
        guard let sheet = waitingSheet else { return }
        waitingSheet = nil
        launchUrl = nil
        sheet.presentingViewController?.dismiss(animated: true)
    }

    // MARK: - SFSafariViewControllerDelegate

    /// The person closed the tab. A page that had the request was closed
    /// without signing — the sheet goes back to "not signed" (spec US1); one
    /// that never connected leaves the waiting sheet, with its two ways on.
    func safariViewControllerDidFinish(_ controller: SFSafariViewController) {
        channel?.pageClosed()
    }

    /// The window's topmost controller — over the signing sheet, over Send.
    private static var presenter: UIViewController? {
        UIApplication.shared.connectedScenes
            .compactMap { $0 as? UIWindowScene }
            .first { $0.activationState == .foregroundActive }?
            .windows
            .first { $0.isKeyWindow }?
            .rootViewController?
            .topmost
    }
}

/// Under the tab while the Clear Signer has the request.
struct ClearSignerWaitingSheet: View {
    @Environment(\.theme) private var theme
    let title: String
    let hint: String
    let reopen: String
    let cancel: String
    let onReopen: () -> Void
    let onCancel: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            HStack(spacing: Tokens.Space.s12) {
                ProgressView()
                Text(title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
            }
            Text(hint)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: Tokens.Space.s8)
            VelaButton(title: reopen, kind: .secondary, action: onReopen)
            VelaButton(title: cancel, kind: .secondary, action: onCancel)
        }
        .padding(Tokens.Space.s24)
        .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
        .background(theme.bgBase.ignoresSafeArea())
    }
}
