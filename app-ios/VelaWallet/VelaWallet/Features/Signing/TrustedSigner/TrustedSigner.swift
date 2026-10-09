//
//  TrustedSigner.swift
//  VelaWallet
//
//  The trusted signing page on the phone: a separate page that decodes the
//  request from its own bytes, derives what must be signed itself and only
//  then runs the ceremony.
//
//  Spec 102 made it what the code always said it was: not a place a passkey
//  is, but WHERE a person reviews and signs — an account's signing venue. Its
//  key still lives in one of the three places (this device, a phone, a USB
//  key), and the page is told which (`KeyRoute`, R5) so the browser goes
//  straight to it. Two doors reach it:
//
//  - the spine's, for an account whose venue is a page (its transactions and
//    messages — R4);
//  - the machines', for a key ceremony an operation names a page for — only a
//    wallet on its own domain, whose keys answer nowhere else (R3).
//
//  **Nothing opens unchecked** (R6). Before a page opens, this phone fetches
//  the very version it will open, hashes it, and the core rules
//  (`SignerPageChecks`); only that ruling builds the launch URL. The hand-off
//  card (D4) shows the key and the ruling's line, and Open is enabled only
//  when the core admitted the page. A refusal is said on the card and the page
//  never opens.
//
//  There is ONE channel: a `TrustedSignerChannel` — the custom scheme the
//  page answers over — and the page in an `SFSafariViewController`. The tab
//  suspends this app while it is up, which the socket channel could not
//  survive and this one does not mind: the system delivers a
//  `velawallet://sign-result` to a suspended app. Under the tab is the one
//  sheet (`TrustedSignerSheets`), which is where "open the page again" and
//  cancel live.
//
//  **One session per flow** (contract §1.5). A create on a custom domain is a
//  key and then its member proof; a recovery is two proofs. The card is shown
//  once, the requests go to the page in turn, and `endFlow()` says `bye`. A
//  second flow gets a fresh card.
//
//  Every verdict is the core's. What an ending MEANS for the request — closed
//  behaves like a cancelled passkey sheet, every refusal is shown, nothing is
//  submitted — is the spine's, the sheets' and the machines'.
//

import SafariServices
import SwiftUI
import UIKit
import VelaCore

/// The trusted page as the SPINE reaches it — a port, so the money path can
/// be tested without a browser.
protocol TrustedSignerPort: AnyObject {
    /// Opens `page` (the account's venue) for `requestJson`
    /// (`trustedSignerRequest`'s) and waits for an answer the core verified
    /// over `digest` by one of `keys` — the keys the page was offered, which
    /// for an account with a sign-in key is that key alone.
    ///
    /// `keyName` (the record's label for the key) and `place` (where it
    /// lives) are what the hand-off card says the person will confirm with —
    /// the name, else the place. `place` is `nil` for a record from before the
    /// sign-in key was kept.
    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], page: String,
        keyName: String, place: KeyMethod?
    ) async -> TrustedSignerChannel.Ending
}

/// The trusted page as the onboarding MACHINES reach it — a passkey ceremony
/// that runs on a page instead of in the OS sheet, for a wallet whose keys
/// live on the page's own domain (spec 102 R3).
protocol TrustedSignerCeremonyPort: AnyObject {
    /// Runs `operationJson`'s ceremony (`RegisterPasskey`,
    /// `AuthenticatePasskey`, `SignProof`, `SignMemberProof` as their wire
    /// JSON) on this flow's session, opening it if this is the first one.
    ///
    /// - Parameters:
    ///   - walletName: what the page's card names.
    ///   - registry: the registry service, named on the page's card.
    ///   - expectedMemberChallenge: the challenge the WALLET fetched for the
    ///     same inputs — the page must have signed exactly it.
    ///   - page: the operation's own `page` — the one its keys live on. The
    ///     operation's `method` is the place the key lives, which the card
    ///     names and the page is told as hints.
    ///   - deployment: the chain and registry contract a member proof is bound
    ///     to. The page computes the challenge from them, because the published
    ///     page reaches no network to look them up (076).
    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String,
        deployment: SignerRegistryDeployment?
    ) async -> TrustedSignerCeremonyStep

    /// The flow is over.
    func endFlow()
}

/// A ceremony's end, already in the machines' vocabulary.
enum TrustedSignerCeremonyStep: Equatable {
    /// The core's `Registration`, as its wire JSON.
    case registered(String)
    /// The core's `Assertion`, as its wire JSON.
    case asserted(String)
    /// `cancelled` for a page that was closed or declined — never an error
    /// alert — and `other` carrying the sentence for everything else.
    case failed(kind: FailureKind, message: String?)
}

/// The sentence an ended request leaves on the sheet (contract §5, spec 102).
/// A person needs to know whether they closed it, the page refused, the
/// answer did not match, or the page was never opened — and why — not which
/// byte gave it away.
enum TrustedSignerNotice: Equatable {
    /// Declined, closed, cancelled.
    case closed
    /// The page's own rules said no.
    case refused
    /// An answer the wallet would not accept: another digest, another key, a
    /// bad signature, no user verification, the wrong token, the wrong shape.
    case mismatch
    case timeout
    /// The page could not be put on screen at all.
    case unavailable
    /// R6: the page's check did not admit it; it was never opened.
    case notOpened(SignerIntegrityLine)
    /// R1: nothing on this device can reach the account's keys.
    case blocked(VenueBlockWire)

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
        case .unavailable: self = .unavailable
        case .cancelled: self = .closed
        case .notOpened(let line): self = .notOpened(line)
        }
    }

    /// The corpus key of the sentence.
    var key: String {
        switch self {
        case .closed: "componentsUi.signing.trustedSignerClosed"
        case .refused: "componentsUi.signing.trustedSignerRefused"
        case .mismatch: "componentsUi.signing.trustedSignerMismatch"
        case .timeout: "componentsUi.signing.trustedSignerTimeout"
        case .unavailable: "componentsUi.signing.signerDown"
        case .notOpened(let line): line.key
        case .blocked(let block): block.key
        }
    }

    /// The sentence, in the person's language.
    func text(_ loc: Loc) -> String {
        switch self {
        case .notOpened(let line): line.text(loc)
        case .blocked(let block): block.text(loc)
        default: loc.t(key)
        }
    }

    /// Closed is the person's own decision, told calmly; everything else is
    /// not.
    var calm: Bool { self == .closed }
}

final class TrustedSigner: NSObject, TrustedSignerPort, TrustedSignerCeremonyPort, SFSafariViewControllerDelegate {

    private let loc: Loc
    /// The integrity checks — the app's one checker, so the card, the signing
    /// sheet and Settings read the same rulings.
    private let checks: SignerPageChecks
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
    /// The hand-off card is waiting for Open (`true`) or Cancel (`false`).
    private var handoffAnswer: CheckedContinuation<Bool, Never>?

    init(loc: Loc, checks: SignerPageChecks) {
        self.loc = loc
        self.checks = checks
        super.init()
    }

    // MARK: - The spine's door (071, 102 R4)

    func sign(
        requestJson: String, digest: Data, keys: [WalletKeyRecord], page: String,
        keyName: String, place: KeyMethod?
    ) async -> TrustedSignerChannel.Ending {
        let ask = TrustedSignerAsk.signature(request: requestJson, digest: digest, keys: keys)
        let keyLabel = Self.keyLabel(name: keyName, place: place, loc: loc)
        // The signing sheet was the hand-off card, and its Open is what asked
        // for this: a page that is admitted opens at once. One that is not
        // keeps the card up, with the line that says why.
        let ending = await run(ask, page: page, keyLabel: keyLabel, waitForOpen: false)
        // A signature is a flow of one: the page is told so rather than left
        // on its waiting screen.
        endFlow()
        return ending
    }

    // MARK: - The machines' door (075, 102 R3)

    func ceremony(
        operationJson: String, walletName: String, registry: String,
        expectedMemberChallenge: Data?, page: String,
        deployment: SignerRegistryDeployment? = nil
    ) async -> TrustedSignerCeremonyStep {
        let id = UUID().uuidString.lowercased()
        let operation = try? CoreJSON.object(operationJson)
        let place = (operation?["method"] as? String).flatMap(KeyMethod.init(rawValue:))
        // A key being made carries its name; a sign-in names none.
        let keyLabel = Self.keyLabel(name: operation?["name"] as? String ?? "", place: place, loc: loc)
        guard let request = trustedSignerCeremonyRequest(
            operationJson: operationJson, id: id, walletName: walletName, registry: registry,
            deployment: deployment
        ) else {
            // Not a ceremony the core knows: the machine must still be told
            // something it can act on.
            return .failed(kind: .other, message: loc.t(TrustedSignerNotice.mismatch.key))
        }
        let ask = TrustedSignerAsk.ceremony(
            request: request, operationJson: operationJson, memberChallenge: expectedMemberChallenge
        )
        // Nothing in the app said "a page will open" before a ceremony: the
        // card does, and waits for the person's Open.
        switch await run(ask, page: page, keyLabel: keyLabel, waitForOpen: true) {
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
                : .failed(kind: .other, message: notice.text(loc))
        }
    }

    /// "Confirm with This device" — the place's title, as the choosers name
    /// it (`keyMethodWords`).
    static func keyLabel(_ place: KeyMethod, loc: Loc) -> String {
        methodCopy(place, chooser: .signIn, loc: loc).title
    }

    /// "Confirm with {{key}}": the key's own name, else its place's title;
    /// `nil` when neither is known.
    static func keyLabel(name: String, place: KeyMethod?, loc: Loc) -> String? {
        let name = name.trimmingCharacters(in: .whitespacesAndNewlines)
        if !name.isEmpty { return name }
        return place.map { keyLabel($0, loc: loc) }
    }

    func endFlow() {
        conversation?.end()
        conversation = nil
        channel = nil
        openPage = nil
        answerHandoff(false)
        dismiss()
    }

    // MARK: - One request

    /// Puts `ask` to the flow's session, opening one — through the hand-off
    /// card and the integrity check — when there is none.
    private func run(
        _ ask: TrustedSignerAsk, page: String, keyLabel: String?, waitForOpen: Bool
    ) async -> TrustedSignerChannel.Ending {
        if let conversation {
            model?.stage = .waiting
            let ending = await conversation.send(ask)
            hideTab()
            return ending
        }
        guard !busy else { return TrustedSignerChannel.declined(for: ask) }
        busy = true
        declined = false
        defer { busy = false }

        openPage = page
        let model = TrustedSignerSheetModel()
        model.page = page
        model.keyLabel = keyLabel
        model.line = checks.line(for: page)
        self.model = model
        model.cancel = { [weak self] in self?.cancelled() }
        guard present(model) else { return .unavailable }

        guard let admission = await admitted(page, model: model, waitForOpen: waitForOpen) else {
            // The person left the card. A page the check refused says why on
            // the request's sheet; one that could have opened is a cancel.
            let line = model.line
            dismiss()
            return line.opens || line.state == .checking ? .cancelled : .notOpened(line)
        }
        let ending = await onThisDevice(ask, page: page, admission: admission, model: model)
        hideTab()
        return ending
    }

    /// The hand-off card, until the page may open — and, when `waitForOpen`,
    /// until the person taps Open. `nil` when they left instead.
    ///
    /// The ruling is fetched fresh when none stands (24 hours, the core's),
    /// and checked again at the tap if it went stale while the card was up:
    /// the URL that opens is always one a live check admitted.
    private func admitted(
        _ page: String, model: TrustedSignerSheetModel, waitForOpen: Bool
    ) async -> SignerPageAdmission? {
        model.stage = .handoff
        model.recheck = { [weak self, weak model] in
            guard let self, let model else { return }
            model.line = SignerPageChecks.checking
            Task { @MainActor in
                let ruling = await self.checks.check(page)
                model.line = ruling.line(nowMs: self.checks.nowMs)
            }
        }
        let ruling = await checks.ensure(page)
        model.line = ruling.line(nowMs: checks.nowMs)
        if !waitForOpen, let ready = checks.openable(page) { return ready }
        while true {
            let opened = await withTaskCancellationHandler {
                await withCheckedContinuation { continuation in
                    handoffAnswer = continuation
                    model.open = { [weak self] in self?.answerHandoff(true) }
                }
            } onCancel: {
                Task { @MainActor [weak self] in self?.answerHandoff(false) }
            }
            model.open = nil
            guard opened else { return nil }
            if let ready = checks.openable(page) { return ready }
            // Stale since the card went up: check again, then open.
            model.line = SignerPageChecks.checking
            let again = await checks.check(page)
            model.line = again.line(nowMs: checks.nowMs)
            if let ready = checks.openable(page) { return ready }
        }
    }

    private func answerHandoff(_ open: Bool) {
        let waiting = handoffAnswer
        handoffAnswer = nil
        waiting?.resume(returning: open)
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

    /// The custom-scheme channel and the in-app tab (071, 076), launched
    /// through the admitted page (102 R6).
    private func onThisDevice(
        _ ask: TrustedSignerAsk, page: String, admission: SignerPageAdmission,
        model: TrustedSignerSheetModel
    ) async -> TrustedSignerChannel.Ending {
        let channel = TrustedSignerChannel(
            signerUrl: page, launch: TrustedSignerChannel.launcher(admission, lang: loc.resolvedLanguage),
            first: ask
        )
        self.channel = channel
        // The flow's later requests are their own visits, and the channel has
        // no way to present one: opening a page is this object's business.
        channel.openPage = { [weak self] url in self?.show(url) }
        // Spec 079: a page that could not open is the card's to say, with a
        // retry — asked of the page's address alone.
        channel.reachable = { url in await TrustedSignerChannel.headProbe(url) }
        channel.onUnreachableChanged = { [weak model] unreachable in model?.unreachable = unreachable }
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
    /// On the hand-off card, nothing was opened: the card goes.
    func cancel() {
        declined = true
        answerHandoff(false)
        channel?.cancel()
    }

    /// The same URL, the same one-time token: a person who dismissed the tab
    /// by accident gets the same request back, and the answer still belongs
    /// to the attempt that is waiting (contract §2). On a page that could not
    /// open it is the card's Retry (spec 079), and the card waits again.
    func reopen() {
        guard let url = channel?.launchUrl, let sheet = hostSheet,
              sheet.presentedViewController == nil
        else { return }
        channel?.retried()
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
        // Unanimated: an admitted page is presented over this sheet in the
        // same moment, and UIKit drops a presentation made while another is
        // still animating in.
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
    //
    // Spec 079's return signal. The page is an in-app tab, so the app never
    // leaves the foreground: the tab itself says when the person closed it
    // and whether its first load worked, and those two are what start the
    // question "could the page open at all?" (a HEAD to its address).

    /// The person closed the tab. Not an answer (see `pageClosed`): after a
    /// moment's grace, a page that never loaded is asked about, and when its
    /// address does not answer the waiting card says the page could not open.
    func safariViewControllerDidFinish(_ controller: SFSafariViewController) {
        channel?.pageClosed()
    }

    /// The tab's first load. A page that loaded is left alone whatever the
    /// network does next. One that did not is asked about at once; when its
    /// address does not answer, the tab goes — the app, not the browser's
    /// error page, says the page could not be reached (FR-028) — and the card
    /// offers the retry.
    func safariViewController(_ controller: SFSafariViewController, didCompleteInitialLoad didLoadSuccessfully: Bool) {
        guard let channel else { return }
        Task { @MainActor [weak self] in
            await channel.pageLoaded(didLoadSuccessfully)
            guard let self, !didLoadSuccessfully, channel.unreachable, self.channel === channel else { return }
            self.hideTab()
        }
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
