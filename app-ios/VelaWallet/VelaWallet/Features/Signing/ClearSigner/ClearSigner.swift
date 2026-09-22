//
//  ClearSigner.swift
//  VelaWallet
//
//  The Clear Signer on the phone: not a place a passkey is, but a separate
//  page that decodes the request from its own bytes, derives what must be
//  signed itself and only then runs the ceremony.
//
//  Spec 071 made it a fourth way to SIGN. Spec 075 makes it a fourth **passkey
//  route**, beside this device, a nearby device and a security key: it creates
//  keys, signs in, proves and confirms membership, wherever the other three
//  are offered. So this file now owns four things rather than three:
//
//  - **where** the signer is — this device, or another one (`clearSignerWhere`);
//  - on this device, a `ClearSignerChannel` — the loopback listener the page
//    talks to — and the page in an `SFSafariViewController`. An in-app tab
//    keeps this app in the foreground, which is what keeps the listener alive
//    for the whole ceremony (research R2);
//  - on another device, a `ClearSignerRelayConversation` — the QR, the link,
//    and the six digits the person compares before anything is sent;
//  - the one sheet under all of it (`ClearSignerSheets`), which is also where
//    "open the page again" and cancel live.
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

/// The Clear Signer as the SPINE reaches it — a port, so the money path can
/// be tested without a browser.
protocol ClearSignerPort: AnyObject {
    /// Opens the page for `requestJson` (`clearSignerRequest`'s) and waits for
    /// an answer the core verified over `digest` by one of `keys`.
    ///
    /// `signerOrigin` is the page the KEY lives behind (spec 075,
    /// `signRoute`): a key minted on somebody's own signer page is reachable
    /// nowhere else. Empty or `nil` opens the page from Settings.
    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> ClearSignerChannel.Ending
}

extension ClearSignerPort {
    func sign(requestJson: String, digest: Data, keys: [WalletKeyRecord]) async -> ClearSignerChannel.Ending {
        await sign(requestJson: requestJson, digest: digest, keys: keys, signerOrigin: nil)
    }
}

/// Spec 075: the Clear Signer as the onboarding MACHINES reach it — a passkey
/// ceremony that runs on a page instead of in the OS sheet.
protocol ClearSignerCeremonyPort: AnyObject {
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
    ) async -> ClearSignerCeremonyStep

    /// The flow is over, however it ended.
    func endFlow()
}

/// A ceremony's end, already in the machines' vocabulary.
enum ClearSignerCeremonyStep: Equatable {
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

    /// The notice for a whole ending — `nil` when something was signed.
    init?(ending: ClearSignerChannel.Ending) {
        switch ending {
        case .outcome(.accepted), .ceremony(.registered), .ceremony(.asserted): return nil
        case .outcome(.refused(let refusal)), .ceremony(.refused(let refusal)): self.init(refusal)
        case .timedOut: self = .timeout
        case .unavailable: self = .mismatch
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

final class ClearSigner: NSObject, ClearSignerPort, ClearSignerCeremonyPort, SFSafariViewControllerDelegate {

    private let loc: Loc
    /// The page Settings names (`sign_pref`); `nil` before it has said, which
    /// is the official page.
    private let signerUrl: () -> String?
    /// The relay Settings names (`sign_pref`, spec 075).
    private let relayUrl: () -> String?
    /// The relay conversation, as a seam for tests.
    private let makeRelay: (_ signerUrl: String, _ relay: String) -> ClearSignerRelayConversation?

    /// The flow's live session, while one is open.
    private var conversation: ClearSignerConversation?
    /// The same session when it is the loopback one — it alone has a tab.
    private var channel: ClearSignerChannel?
    private var relay: ClearSignerRelayConversation?
    /// The page this session is talking to; every answer's origin is checked
    /// against it by the core.
    private var openPage: String?
    private var launchUrl: URL?

    private var hostSheet: UIViewController?
    private var model: ClearSignerSheetModel?
    /// The where-choice and the code confirmation, while somebody is looking.
    private var asking: CheckedContinuation<Bool, Never>?
    /// One request at a time: the sheet is modal, so a second can only come
    /// from a machine, and stacking a page on a page is never the answer.
    private var busy = false

    init(
        loc: Loc,
        signerUrl: @escaping () -> String?,
        relayUrl: @escaping () -> String? = { nil },
        makeRelay: @escaping (String, String) -> ClearSignerRelayConversation? = { signer, relay in
            ClearSignerRelayConversation(signerUrl: signer, relay: relay)
        }
    ) {
        self.loc = loc
        self.signerUrl = signerUrl
        self.relayUrl = relayUrl
        self.makeRelay = makeRelay
        super.init()
    }

    // MARK: - The spine's door (071)

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], signerOrigin: String?
    ) async -> ClearSignerChannel.Ending {
        let ask = ClearSignerAsk.signature(request: requestJson, digest: digest, keys: keys)
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
    ) async -> ClearSignerCeremonyStep {
        let id = UUID().uuidString.lowercased()
        guard let request = clearSignerCeremonyRequest(
            operationJson: operationJson, id: id, walletName: walletName, registry: registry
        ) else {
            // Not a ceremony the core knows: the machine must still be told
            // something it can act on.
            return .failed(kind: .other, message: loc.t(ClearSignerNotice.mismatch.key))
        }
        let ask = ClearSignerAsk.ceremony(
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
            let notice = ClearSignerNotice(ending: ending) ?? .mismatch
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
        relay = nil
        openPage = nil
        launchUrl = nil
        finishAsk(pick: nil)
        dismiss()
    }

    // MARK: - One request

    /// Puts `ask` to the flow's session, opening one — with the where-choice,
    /// the tab or the pairing sheet — when there is none.
    private func run(_ ask: ClearSignerAsk, page: String?) async -> ClearSignerChannel.Ending {
        if let conversation {
            model?.stage = .waiting
            return await conversation.send(ask)
        }
        guard !busy else { return .outcome(.refused(refusal: .declined)) }
        busy = true
        defer { busy = false }

        let wanted = page.flatMap { $0.isEmpty ? nil : $0 } ?? signerUrl() ?? clearSignerDefaultUrl()
        openPage = wanted
        let model = ClearSignerSheetModel()
        self.model = model
        model.cancel = { [weak self] in self?.cancelled() }
        model.pick = { [weak self] here in self?.finishAsk(pick: here) }
        model.confirmCode = { [weak self] in
            VelaHaptic.success.play()
            self?.finishAsk(pick: true)
        }
        model.copyLink = { [weak self] in
            guard let self, let link = self.relay?.link else { return }
            VelaHaptic.select.play()
            UIPasteboard.general.string = link
            self.model?.copied = true
        }
        guard present(model) else { return .unavailable }

        while true {
            // Where is it? A cancel here is a decline, exactly as a dismissed
            // passkey sheet is.
            guard await awaitAnswer(), let here = lastPick else {
                return .outcome(.refused(refusal: .declined))
            }
            if here {
                return await onThisDevice(ask, page: wanted, model: model)
            }
            if let ending = await onAnotherDevice(ask, page: wanted, model: model) {
                return ending
            }
            // The relay would not come up. The choice is offered again with
            // the reason under it, rather than dying on a spinner.
            model.stage = .relayDown
        }
    }

    /// The loopback channel and the in-app tab (071).
    private func onThisDevice(
        _ ask: ClearSignerAsk, page: String, model: ClearSignerSheetModel
    ) async -> ClearSignerChannel.Ending {
        let channel = ClearSignerChannel(signerUrl: page, first: ask)
        self.channel = channel
        guard await channel.open() != nil, let url = channel.launchUrl else {
            self.channel = nil
            return .unavailable
        }
        conversation = channel
        launchUrl = url
        model.reopen = { [weak self] in self?.reopen() }
        model.stage = .waiting
        hostSheet?.present(self.page(url), animated: true)
        return await channel.ending()
    }

    /// The relay: the QR, the link, and the six digits. `nil` when the relay
    /// never came up — the caller offers the choice again.
    private func onAnotherDevice(
        _ ask: ClearSignerAsk, page: String, model: ClearSignerSheetModel
    ) async -> ClearSignerChannel.Ending? {
        guard let relay = makeRelay(page, relayUrl() ?? clearSignerDefaultRelay()) else { return nil }
        self.relay = relay
        model.link = relay.link
        model.copied = false
        model.stage = .pairing
        guard let code = await relay.pair() else {
            relay.cancel()
            self.relay = nil
            return nil
        }
        model.stage = .code(code)
        // NOTHING is sent until the person says both screens show the same
        // digits: that is the whole defence against a stand-in page.
        guard await awaitAnswer() else {
            relay.cancel()
            self.relay = nil
            return .outcome(.refused(refusal: .declined))
        }
        conversation = relay
        model.stage = .waiting
        return await relay.begin(ask)
    }

    // MARK: - Waiting for the person

    /// What the last answered question picked — `true` for this device, and
    /// `true` for a confirmed code. `nil` when it was cancelled.
    private var lastPick: Bool?

    /// Suspends until a button on the sheet answers. `false` means the person
    /// cancelled rather than chose; WHAT they chose is `lastPick`.
    private func awaitAnswer() async -> Bool {
        await withCheckedContinuation { continuation in
            asking?.resume(returning: false)
            asking = continuation
        }
    }

    private func finishAsk(pick: Bool?) {
        guard let continuation = asking else { return }
        asking = nil
        lastPick = pick
        continuation.resume(returning: pick != nil)
    }

    /// The sheet's cancel: the person declined (contract §2).
    private func cancelled() {
        if asking != nil {
            finishAsk(pick: nil)
            return
        }
        cancel()
    }

    /// Declined — the request stays open, as after a cancelled passkey sheet.
    func cancel() {
        channel?.cancel()
        relay?.cancel()
    }

    /// The same URL, the same port, the same token: the core accepts a new
    /// socket that proves itself while none has an outcome (contract §2).
    func reopen() {
        guard let url = launchUrl, let sheet = hostSheet,
              sheet.presentedViewController == nil
        else { return }
        sheet.present(page(url), animated: true)
    }

    // MARK: - Presentation

    @discardableResult
    private func present(_ model: ClearSignerSheetModel) -> Bool {
        guard let presenter = Self.presenter else {
            // No window to show a page in: the ceremony cannot happen, and
            // waiting five minutes for it would be a spinner with no page.
            return false
        }
        let scheme: ColorScheme = presenter.traitCollection.userInterfaceStyle == .dark ? .dark : .light
        let sheet = UIHostingController(
            rootView: ClearSignerSheet(loc: loc, model: model).themed(scheme)
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
