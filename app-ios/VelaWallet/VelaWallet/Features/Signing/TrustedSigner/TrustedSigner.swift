//
//  TrustedSigner.swift
//  VelaWallet
//
//  The Trusted Signer on the phone: not a place a passkey is, but a separate
//  page that decodes the request from its own bytes, derives what must be
//  signed itself and only then runs the ceremony.
//
//  Spec 071 made it a fourth way to SIGN. Spec 075 makes it a fourth **passkey
//  route**, beside this device, a nearby device and a security key: it creates
//  keys, signs in, proves and confirms membership, wherever the other three
//  are offered.
//
//  There is ONE channel: a `TrustedSignerChannel` — the custom scheme the
//  page answers over — and the page in an `SFSafariViewController`. The tab
//  suspends this app while it is up, which the socket channel could not
//  survive and this one does not mind: the system delivers a
//  `velawallet://sign-result` to a suspended app. Under the tab is the one
//  sheet (`TrustedSignerSheets`), which is where "open the page again" and
//  cancel live.
//
//  The cross-device channels went on 2026-09-23 (the owner: "客户端支持回环 +
//  蓝牙就够了"，then "我确定砍掉蓝牙"): only a page THIS device fetched can be
//  checked against what it is supposed to be, which is the property the whole
//  route exists for.
//
//  **One session per flow** (contract §1.5). A create is a key and then its
//  member proof; a recovery is two proofs. The page is opened once, the
//  requests go to it in turn, and `endFlow()` says `bye`. A second flow gets
//  a fresh page.
//
//  Every verdict is the core's. What an ending MEANS for the request — closed
//  behaves like a cancelled passkey sheet, every refusal is shown, nothing is
//  submitted — is the spine's, the sheets' and the machines'.
//

import SafariServices
import SwiftUI
import UIKit
import VelaCore

/// The Trusted Signer as the SPINE reaches it — a port, so the money path can
/// be tested without a browser.
protocol TrustedSignerPort: AnyObject {
    /// Opens the page for `requestJson` (`trustedSignerRequest`'s) and waits for
    /// an answer the core verified over `digest` by one of `keys`.
    ///
    /// `signerOrigin` is the page the KEY lives behind (spec 075,
    /// `signRoute`): a key minted on somebody's own signer page is reachable
    /// nowhere else. Empty or `nil` opens the page from Settings.
    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> TrustedSignerChannel.Ending
}

extension TrustedSignerPort {
    func sign(requestJson: String, digest: Data, keys: [WalletKeyRecord]) async -> TrustedSignerChannel.Ending {
        await sign(requestJson: requestJson, digest: digest, keys: keys, signerOrigin: nil)
    }
}

/// Spec 075: the Trusted Signer as the onboarding MACHINES reach it — a passkey
/// ceremony that runs on a page instead of in the OS sheet.
protocol TrustedSignerCeremonyPort: AnyObject {
    /// Runs `operationJson`'s ceremony (`RegisterPasskey`,
    /// `AuthenticatePasskey`, `SignProof`, `SignMemberProof` as their wire
    /// JSON) on this flow's session, opening it if this is the first one.
    ///
    /// - Parameters:
    ///   - walletName: what the page's card names.
    ///   - registry: where the page fetches a member challenge.
    ///   - expectedMemberChallenge: the challenge the WALLET fetched for the
    ///     same inputs — the page must have signed exactly it.
    ///   - page: the operation's own `signer_origin` — the page a key that
    ///     already exists lives behind. Empty opens the one from Settings.
    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String?
    ) async -> TrustedSignerCeremonyStep

    /// The flow is over, however it ended.
    func endFlow()
}

/// A ceremony's end, already in the machines' vocabulary.
enum TrustedSignerCeremonyStep: Equatable {
    /// The core's `Registration`, as its wire JSON.
    case registered(String)
    /// The core's `Assertion`, as its wire JSON.
    case asserted(String)
    /// `cancelled` for a page that was closed or declined — never an error
    /// alert — and `other` carrying the 071 sentence for everything else.
    case failed(kind: FailureKind, message: String?)
}

/// The sentence an ended ceremony leaves on the sheet (contract §5). Four,
/// for eight refusals: a person needs to know whether they closed it, the page
/// refused, or the answer did not match — not which byte gave it away.
enum TrustedSignerNotice: Equatable {
    /// Declined, closed, cancelled.
    case closed
    /// The page's own rules said no.
    case refused
    /// An answer the wallet would not accept: another digest, another key, a
    /// bad signature, no user verification, the wrong token, the wrong shape.
    case mismatch
    case timeout

    init(_ refusal: TrustedSignerRefusal) {
        switch refusal {
        case .declined: self = .closed
        case .pageRefused: self = .refused
        case .wrongChallenge, .foreignKey, .badSignature, .notVerified, .wrongToken, .malformed:
            self = .mismatch
        }
    }

    /// The notice for a whole ending — `nil` when something was signed.
    init?(ending: TrustedSignerChannel.Ending) {
        switch ending {
        case .outcome(.accepted), .ceremony(.registered), .ceremony(.asserted): return nil
        case .outcome(.refused(let refusal)), .ceremony(.refused(let refusal)): self.init(refusal)
        case .timedOut: self = .timeout
        case .unavailable: self = .mismatch
        }
    }

    var key: String {
        switch self {
        case .closed: "componentsUi.signing.trustedSignerClosed"
        case .refused: "componentsUi.signing.trustedSignerRefused"
        case .mismatch: "componentsUi.signing.trustedSignerMismatch"
        case .timeout: "componentsUi.signing.trustedSignerTimeout"
        }
    }
}

final class TrustedSigner: NSObject, TrustedSignerPort, TrustedSignerCeremonyPort, SFSafariViewControllerDelegate {

    private let loc: Loc
    /// The page Settings names (`sign_pref`); `nil` before it has said, which
    /// is the official page.
    private let signerUrl: () -> String?
    /// The flow's live session, while one is open.
    private var conversation: TrustedSignerConversation?
    /// The same session, typed: it owns the tab's URL and the clock.
    private var channel: TrustedSignerChannel?
    /// The page this session is talking to; every answer's origin is checked
    /// against it by the core.
    private var openPage: String?

    private var hostSheet: UIViewController?
    private var model: TrustedSignerSheetModel?
    /// One request at a time: the sheet is modal, so a second can only come
    /// from a machine, and stacking a page on a page is never the answer.
    private var busy = false
    /// The person pressed cancel while no question was pending.
    private var declined = false

    init(loc: Loc, signerUrl: @escaping () -> String?) {
        self.loc = loc
        self.signerUrl = signerUrl
        super.init()
    }

    // MARK: - The spine's door (071)

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> TrustedSignerChannel.Ending {
        let ask = TrustedSignerAsk.signature(request: requestJson, digest: digest, keys: keys)
        let ending = await run(ask, page: signerOrigin)
        // A signature is a flow of one: the page is told so rather than left
        // on its waiting screen.
        endFlow()
        return ending
    }

    // MARK: - The machines' door (075)

    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String?
    ) async -> TrustedSignerCeremonyStep {
        let id = UUID().uuidString.lowercased()
        guard let request = trustedSignerCeremonyRequest(
            operationJson: operationJson, id: id, walletName: walletName, registry: registry
        ) else {
            // Not a ceremony the core knows: the machine must still be told
            // something it can act on.
            return .failed(kind: .other, message: loc.t(TrustedSignerNotice.mismatch.key))
        }
        let ask = TrustedSignerAsk.ceremony(
            request: request, operationJson: operationJson, memberChallenge: expectedMemberChallenge
        )
        switch await run(ask, page: page) {
        case .ceremony(.registered(let json)):
            VelaHaptic.success.play()
            return .registered(json)
        case .ceremony(.asserted(let json)):
            VelaHaptic.success.play()
            return .asserted(json)
        case let ending:
            let notice = TrustedSignerNotice(ending: ending) ?? .mismatch
            endFlow()
            // Closed or declined is a cancelled sheet, never an alert.
            return notice == .closed
                ? .failed(kind: .cancelled, message: nil)
                : .failed(kind: .other, message: loc.t(notice.key))
        }
    }

    func endFlow() {
        conversation?.end()
        conversation = nil
        channel = nil
        openPage = nil
        dismiss()
    }

    // MARK: - One request

    /// Puts `ask` to the flow's session, opening one — with the where-choice,
    /// the tab or the pairing sheet — when there is none.
    private func run(_ ask: TrustedSignerAsk, page: String?) async -> TrustedSignerChannel.Ending {
        if let conversation {
            model?.stage = .waiting
            let ending = await conversation.send(ask)
            hideTab()
            return ending
        }
        guard !busy else { return .outcome(.refused(refusal: .declined)) }
        busy = true
        declined = false
        defer { busy = false }

        let wanted = page.flatMap { $0.isEmpty ? nil : $0 } ?? signerUrl() ?? trustedSignerDefaultUrl()
        openPage = wanted
        let model = TrustedSignerSheetModel()
        self.model = model
        model.cancel = { [weak self] in self?.cancelled() }
        guard present(model) else { return .unavailable }
        let ending = await onThisDevice(ask, page: wanted, model: model)
        hideTab()
        return ending
    }

    /// A visit ends with its answer, so the page goes and the wallet's own
    /// sheet is what the person sees again.
    ///
    /// On the custom-scheme channel there is no session for the page to hold:
    /// the next request of a flow opens it again. Leaving the tab up is what
    /// the socket channel did, and it strands a person on the page's own
    /// "handed back to the wallet" screen while the wallet, which has the
    /// answer, sits behind it (owner, 2026-09-24).
    private func hideTab() {
        guard let sheet = hostSheet, sheet.presentedViewController != nil else { return }
        sheet.dismiss(animated: true)
    }

    /// The custom-scheme channel and the in-app tab (071, 076).
    private func onThisDevice(
        _ ask: TrustedSignerAsk, page: String, model: TrustedSignerSheetModel
    ) async -> TrustedSignerChannel.Ending {
        let channel = TrustedSignerChannel(signerUrl: page, first: ask)
        self.channel = channel
        // The flow's later requests are their own visits, and the channel has
        // no way to present one: opening a page is this object's business.
        channel.openPage = { [weak self] url in self?.show(url) }
        guard let url = channel.start() else {
            self.channel = nil
            return .unavailable
        }
        conversation = channel
        model.reopen = { [weak self] in self?.reopen() }
        model.stage = .waiting
        show(url)
        return await channel.ending()
    }

    /// The page, over the waiting sheet. Dismisses a tab that is already up —
    /// a flow's next request is a new URL, and presenting over a presented
    /// controller does nothing at all.
    private func show(_ url: URL) {
        guard let sheet = hostSheet else { return }
        guard sheet.presentedViewController != nil else {
            sheet.present(self.page(url), animated: true)
            return
        }
        sheet.dismiss(animated: false) { [weak self] in
            guard let self, let sheet = self.hostSheet else { return }
            sheet.present(self.page(url), animated: true)
        }
    }

    // MARK: - Waiting for the person

    /// The sheet's cancel: the person declined (contract §2).
    private func cancelled() {
        cancel()
    }

    /// Declined — the request stays open, as after a cancelled passkey sheet.
    func cancel() {
        declined = true
        channel?.cancel()
    }

    /// The same URL, the same one-time token: a person who dismissed the tab
    /// by accident gets the same request back, and the answer still belongs
    /// to the attempt that is waiting (contract §2).
    func reopen() {
        guard let url = channel?.launchUrl, let sheet = hostSheet,
              sheet.presentedViewController == nil
        else { return }
        sheet.present(page(url), animated: true)
    }

    // MARK: - Presentation

    @discardableResult
    private func present(_ model: TrustedSignerSheetModel) -> Bool {
        guard let presenter = Self.presenter else {
            // No window to show a page in: the ceremony cannot happen, and
            // waiting five minutes for it would be a spinner with no page.
            return false
        }
        let scheme: ColorScheme = presenter.traitCollection.userInterfaceStyle == .dark ? .dark : .light
        let sheet = UIHostingController(
            rootView: TrustedSignerSheet(loc: loc, model: model).themed(scheme)
        )
        sheet.modalPresentationStyle = .pageSheet
        sheet.sheetPresentationController?.detents = [.large()]
        sheet.sheetPresentationController?.prefersGrabberVisible = false
        // Cancel is a button, not a swipe: a swipe that ended the ceremony
        // would be a decline nobody meant.
        sheet.isModalInPresentation = true
        hostSheet = sheet
        // The sheet goes up unanimated, so what a person sees is the choice —
        // and, on this device, the page opening over it.
        presenter.present(sheet, animated: false)
        return true
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
        guard let sheet = hostSheet else { return }
        hostSheet = nil
        model = nil
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
