//
//  OnboardingModel.swift
//  VelaWallet
//
//  Both onboarding machines, and the state the screens read.
//
//  The two machines are separate cores with separate drivers and ONE executor
//  between them: six of the eighteen operations are used by both flows, and the
//  contract is a single vocabulary. Two executors would be two places for those
//  six to drift.
//

import Foundation
import Observation
import VelaCore

@MainActor
@Observable
final class OnboardingModel {

    /// The create machine's view; nil until the flow has started.
    private(set) var createView: CreateView?

    private(set) var loginView: LoginView = .idle

    /// The prompt currently on screen, and the answer it is waiting for.
    private(set) var pending: PendingPrompt?

    /// Set once onboarding is over — the host navigates and clears it.
    private(set) var finished = false

    /// The endpoint surface, opened by `endpointUnreachable` or by hand.
    var endpointSheetOpen = false
    private(set) var endpointURL = RegistryClient.defaultURL

    /// A shell fault. Never a user error — it means this app has a bug.
    private(set) var fault: String?

    /// The name field's text, mirrored so typing is not a round trip.
    ///
    /// The CORE is still the authority — every keystroke dispatches
    /// `name_changed` and the view comes back — but a `TextField` bound
    /// straight to the view model loses the caret on every re-render, so the
    /// text lives here and the core corrects it.
    var name: String = "" {
        didSet {
            guard name != oldValue else { return }
            create?.dispatch(Self.event("name_changed", ["name": name]))
        }
    }

    struct PendingPrompt: Identifiable {
        let kind: PromptKind
        let confirmable: Bool
        let answer: CheckedContinuation<Bool, Never>

        var id: String { kind.id }
    }

    // MARK: - The app-owned CCID ceremony's prompts

    /// The security key's PIN prompt (app-owned CCID path only). `answer` is
    /// called by the UI and unblocks the ceremony's background thread.
    private(set) var pendingPin: PendingPin?
    /// Several wallets on one key — the picker.
    private(set) var pendingWalletPick: PendingWalletPick?
    /// Set while a card is blinking; the screen shows "touch your key".
    private(set) var usbTouch: UsbTouch?
    /// The caBLE QR to show (the OTHER phone scans it), or nil when none is up.
    private(set) var cableQr: String?

    // MARK: - The one onboarding bottom sheet

    /// Every app-owned onboarding ceremony prompt — the sign-in method picker,
    /// the "connecting…" hold, the PIN, the touch, the wallet picker, and the
    /// flow prompt — shares ONE `.sheet`. Presenting a second sheet while a
    /// first is dismissing fails silently on iOS (the nesting bug the founder
    /// hit); one sheet whose CONTENT swaps never dismisses between steps.

    /// The sign-in method picker is open (opened from Welcome's "I already have
    /// a wallet"). Lives here, not on the screen, so the one shared sheet owns it.
    var showSignInMethods = false

    /// Held from the instant an app-owned sign-in method is chosen until the
    /// login machine goes idle — it keeps the shared sheet on screen across the
    /// gap between the method pick and the first ceremony prompt, so the sheet
    /// swaps content instead of dismissing and re-presenting.
    private(set) var signInConnecting = false

    /// The ceremony has actually started spinning (login reported `busy`) since
    /// `signInConnecting` was raised. Without it, the login machine's initial
    /// idle view — which arrives before `sign_in` is dispatched — would clear the
    /// hold in the same frame it was set and the sheet would flicker shut.
    private var sawBusySinceConnect = false

    /// True when any app-owned onboarding prompt should be on screen. Drives the
    /// single `.sheet(isPresented:)`; the content is chosen by priority.
    var onboardingSheetPresented: Bool {
        pendingPin != nil || pendingWalletPick != nil || usbTouch != nil
            || cableQr != nil || pending != nil || signInConnecting || showSignInMethods
    }

    /// A swipe-to-dismiss on the shared sheet. Cancels whatever the active
    /// prompt is waiting for; the PIN and wallet picker answer nil (cancel), the
    /// flow prompt answers false, the method picker just closes. The touch and
    /// the connecting hold are non-dismissable (their sheet content disables the
    /// interactive dismiss), so they never reach here.
    func dismissOnboardingSheet() {
        if pendingPin != nil {
            answerPin(nil)
        } else if pendingWalletPick != nil {
            answerWalletPick(nil)
        } else if pending != nil {
            answerPrompt(false)
        } else if showSignInMethods {
            showSignInMethods = false
        }
    }

    /// A method was chosen in the sign-in picker. Platform hands off to the
    /// system passkey sheet (our sheet closes; the OS draws its own), so it needs
    /// no hold. Every app-owned method keeps the shared sheet up via
    /// `signInConnecting` while the ceremony spins up.
    func pickSignInMethod(_ method: KeyMethod) {
        showSignInMethods = false
        signInMethod = method
        if method != .platform {
            signInConnecting = true
            sawBusySinceConnect = false
        }
        signIn(method: method)
    }

    /// The route the person picked, so the connecting hold's words match it —
    /// a scan must never sit behind "plug in your USB security key".
    private(set) var signInMethod: KeyMethod = .platform

    struct PendingPin: Identifiable {
        let product: String
        let retries: Int
        let isRetry: Bool
        let answer: (String?) -> Void
        let id = UUID()
    }

    struct PendingWalletPick: Identifiable {
        let choices: [CtapCredentialChoice]
        let answer: (Int?) -> Void
        let id = UUID()
    }

    struct UsbTouch: Identifiable {
        let kind: String
        let product: String
        let id = UUID()
    }

    func answerPin(_ pin: String?) {
        let prompt = pendingPin
        pendingPin = nil
        prompt?.answer(pin)
    }

    func answerWalletPick(_ index: Int?) {
        let prompt = pendingWalletPick
        pendingWalletPick = nil
        prompt?.answer(index)
    }

    // Set from the (nonisolated) prompts bridge; the sheet reads them.
    fileprivate func presentPin(_ pin: PendingPin) { pendingPin = pin }
    fileprivate func presentWalletPick(_ pick: PendingWalletPick) { pendingWalletPick = pick }
    fileprivate func presentTouch(_ touch: UsbTouch?) { usbTouch = touch }
    fileprivate func presentQr(_ payload: String?) { cableQr = payload }

    private let session: SessionController
    private let store: AccountStore
    private let registry: RegistryClient
    private let passkey = PasskeyExecutor()

    private var create: CoreDriver?
    private var login: CoreDriver?

    init(session: SessionController, store: AccountStore, registry: RegistryClient = RegistryClient()) {
        self.session = session
        self.store = store
        self.registry = registry
        // The app-owned CCID security-key path, wired to this model's prompts.
        passkey.smartCard = SmartCardCtapCeremony(prompts: UsbPromptsBridge(model: self))
        // The caBLE "sign in with your phone" path — same prompts (the touch
        // prompt reads "look at your phone"), plus the QR the other phone scans.
        passkey.hybrid = HybridCeremony(
            prompts: UsbPromptsBridge(model: self),
            showQr: { [weak self] payload in self?.presentQr(payload) }
        )
        // The stored override, applied before any machine can ask a question:
        // a flow that started against the default and then switched mid-way
        // would query two different registries for one wallet. Each flow
        // re-applies it too — see `applyConfiguredRegistry`.
        applyConfiguredRegistry()
    }

    /// Spec 081 FR-002. The index is re-read at the start of every flow, not
    /// snapshotted once in `init`: this model lives as long as the process, so
    /// somebody who changed their public-key index in Settings and then signed
    /// in was still talking to ours. `setBaseURL` is idempotent and costs one
    /// actor hop, which is nothing against a ceremony.
    private func applyConfiguredRegistry() {
        Task {
            let url = await session.registryURL()
            endpointURL = url
            await registry.setBaseURL(url)
        }
    }

    // MARK: - Create

    func startCreate() {
        guard create == nil else { return }
        applyConfiguredRegistry()
        let driver = CoreDriver(
            bridge: CreateWalletCore(),
            perform: { [weak self] operation in
                guard let self else { return CoreJSON.string(["type": "onboarding_completed"]) }
                return await self.executor().perform(operation)
            },
            onView: { [weak self] json in
                guard let self else { return }
                guard let decoded = try? CoreJSON.decode(CreateView.self, from: json) else { return }
                self.createView = decoded
                // The core is the authority on the name — a `StartOver` clears
                // it, and the field has to follow.
                if decoded.name != self.name { self.name = decoded.name }
            },
            onFault: { [weak self] error in self?.fault = error.localizedDescription }
        )
        create = driver
        driver.dispatch(Self.event("start"))
    }

    func toggleAck(_ index: Int) { create?.dispatch(Self.event("ack_toggled", ["index": index])) }
    func submit() { create?.dispatch(Self.event("submit")) }
    func addKey(_ method: KeyMethod) {
        create?.dispatch(Self.event("add_key", ["name": "", "method": method.rawValue]))
    }
    func confirmKey(_ index: Int) { create?.dispatch(Self.event("confirm_key", ["index": index])) }
    func removeKey(_ index: Int) { create?.dispatch(Self.event("remove_key", ["index": index])) }
    func finishKeys() { create?.dispatch(Self.event("finish_keys")) }
    func startOver() { create?.dispatch(Self.event("start_over")) }
    func retryUpload() { create?.dispatch(Self.event("retry_upload")) }
    func enterWallet() { create?.dispatch(Self.event("enter_wallet")) }
    func goBack() { create?.dispatch(Self.event("go_back")) }

    /// Leave the create flow.
    ///
    /// The driver is disposed and dropped rather than kept for a later
    /// re-entry: a create machine holds drafted passkeys, and reusing one across
    /// an exit would show the person a half-built wallet they thought they had
    /// abandoned. Re-entering starts a fresh core — which finds any real draft
    /// in storage.
    func disposeCreate() {
        create?.dispose()
        create = nil
        createView = nil
        name = ""
    }

    // MARK: - Sign in

    func signIn(method: KeyMethod = .platform) {
        applyConfiguredRegistry()
        if login == nil {
            let driver = CoreDriver(
                bridge: LoginCore(),
                perform: { [weak self] operation in
                    guard let self else { return CoreJSON.string(["type": "onboarding_completed"]) }
                    return await self.executor().perform(operation)
                },
                onView: { [weak self] json in
                    guard let self else { return }
                    guard let decoded = try? CoreJSON.decode(LoginView.self, from: json) else { return }
                    // The endpoint surface opens the moment the health probe
                    // says the index is unreachable — and sign-in stays
                    // permitted while it is open. It is a warning with a fix
                    // attached, not a gate.
                    if decoded.endpointUnreachable && !self.loginView.endpointUnreachable {
                        self.endpointSheetOpen = true
                    }
                    self.loginView = decoded
                    // Release the "connecting" hold once the ceremony has spun
                    // up and then wound down — never on the initial idle view
                    // that precedes `sign_in`. A pending error keeps the sheet
                    // up on its own; a success finishes onboarding.
                    if decoded.busy {
                        self.sawBusySinceConnect = true
                    } else if self.sawBusySinceConnect {
                        self.signInConnecting = false
                    }
                },
                onFault: { [weak self] error in self?.fault = error.localizedDescription }
            )
            login = driver
            driver.dispatch(Self.event("start"))
        }
        login?.dispatch(Self.event("sign_in", ["method": method.rawValue]))
    }

    // MARK: - Prompts

    func answerPrompt(_ accepted: Bool) {
        guard let prompt = pending else { return }
        pending = nil
        prompt.answer.resume(returning: accepted)
    }

    // MARK: - Endpoint

    func saveEndpoint(_ url: String) {
        endpointSheetOpen = false
        let normalized = RegistryClient.normalize(url)
        endpointURL = normalized
        Task {
            await registry.setBaseURL(normalized)
            await session.setRegistryURL(normalized)
        }
    }

    func consumeFinished() { finished = false }

    // MARK: - Wiring

    private func executor() -> OnboardingExecutor {
        OnboardingExecutor(passkey: passkey, registry: registry, store: store, deps: self)
    }

    private static func event(_ type: String, _ fields: [String: Any] = [:]) -> String {
        var object: [String: Any] = ["type": type]
        object.merge(fields) { _, new in new }
        return CoreJSON.string(object)
    }
}

extension OnboardingModel: OnboardingExecutorDeps {
    func prompt(kind: PromptKind, confirmable: Bool) async -> Bool {
        await withCheckedContinuation { continuation in
            pending = PendingPrompt(kind: kind, confirmable: confirmable, answer: continuation)
        }
    }

    func complete(mode: [String: Any]) async {
        // Straight through to the session machine, untouched. The onboarding
        // core is finished; whether there is a wallet to route to is the session
        // machine's ruling, not this model's.
        session.accountEstablished(mode: mode)
        finished = true
        // Drop the connecting hold HERE, not from the busy transition: a
        // successful login parks in `Stage::Completing` forever (see below),
        // so `busy` never falls and the hold sheet sat on top of the wallet
        // the person had just entered (device-found 2026-08-28).
        signInConnecting = false

        // A FINISHED machine is not a BUSY one.
        //
        // `login.rs` parks in `Stage::Completing` forever after a successful
        // sign-in — deliberately, because it is done and will never act again —
        // and `busy` is derived as `stage != Idle`, so it reads `true` from then
        // on. Welcome renders that as a disabled "I already have a wallet".
        //
        // Device-found 2026-08-25 on Android: sign in, sign out, and BOTH
        // Welcome buttons are dead — the one-way door replaced by a dead end.
        // The machine is right; rendering "done" as "working" was the bug.
        loginView = .idle
        login?.dispose()
        login = nil
    }
}

/// Bridges the app-owned CCID ceremony's SYNCHRONOUS host callbacks — called on
/// a background thread — to the `@MainActor` model and back.
///
/// A CTAP host callback cannot suspend, so each one blocks its background thread
/// on a semaphore while the main actor puts the prompt on screen; the UI's
/// answer signals the semaphore. The main actor is never blocked (the ceremony
/// runs off it), so there is no deadlock. `@unchecked Sendable` because the
/// hand-off is guarded by the semaphore's happens-before, not by the type.
private final class UsbPromptsBridge: SmartCardCtapCeremony.Prompts, @unchecked Sendable {
    private weak var model: OnboardingModel?

    init(model: OnboardingModel) {
        self.model = model
    }

    private final class Box<T>: @unchecked Sendable {
        var value: T
        init(_ value: T) { self.value = value }
        /// A generic class needs a `nonisolated deinit` to survive `-O` under
        /// this target's default MainActor isolation — see `CoreStore`, and
        /// `app-ios/scripts/check-generic-class-deinit.mjs`.
        nonisolated deinit {}
    }

    func askPin(product: String, retries: Int, isRetry: Bool) -> String? {
        let semaphore = DispatchSemaphore(value: 0)
        let box = Box<String?>(nil)
        Task { @MainActor [weak model] in
            guard let model else { semaphore.signal(); return }
            model.presentPin(
                OnboardingModel.PendingPin(product: product, retries: retries, isRetry: isRetry) { pin in
                    box.value = pin
                    semaphore.signal()
                }
            )
        }
        semaphore.wait()
        return box.value
    }

    func askWhichWallet(_ choices: [CtapCredentialChoice]) -> Int? {
        let semaphore = DispatchSemaphore(value: 0)
        let box = Box<Int?>(nil)
        Task { @MainActor [weak model] in
            guard let model else { semaphore.signal(); return }
            model.presentWalletPick(
                OnboardingModel.PendingWalletPick(choices: choices) { index in
                    box.value = index
                    semaphore.signal()
                }
            )
        }
        semaphore.wait()
        return box.value
    }

    func touchWaiting(kind: String?, product: String) {
        Task { @MainActor [weak model] in
            model?.presentTouch(kind.map { OnboardingModel.UsbTouch(kind: $0, product: product) })
        }
    }
}
