//
//  TrustedSignerChannel.swift
//  VelaWallet
//
//  The phone's end of the Trusted Signer's channel (specs 071 and 076): the
//  request goes out in the URL the page is opened with, and the answer comes
//  back as a `velawallet://sign-result?…` the system hands this app.
//
//  Everything that decides anything is the core's. This file builds no URL of
//  its own (`trustedSignerUrlLaunch`), reads no callback of its own
//  (`trustedSignerParseCallback`) and judges no answer of its own
//  (`trustedSignerVerify`, `trustedSignerVerifyCeremony`). It owns a clock, a
//  one-time token and the bookkeeping of who is waiting — and nothing else.
//
//  ## Why a scheme, and not the loopback socket it replaces
//
//  Two measured reasons, not a preference:
//
//  - the page published under spec 076 carries `default-src 'none'` INSIDE
//    the bytes that are hashed, so a page whose hash this wallet accepts
//    cannot open a WebSocket at all — measured in a real browser, the
//    listening server saw no byte. A NAVIGATION is not governed by that CSP,
//    also measured.
//  - an iOS app is suspended the moment `SFSafariViewController` covers it,
//    so it could not accept a loopback connection anyway. The system delivers
//    a scheme to a suspended app; nothing delivers a TCP connection to one.
//
//  The owner cut the loopback on 2026-09-23 (「回环 WebSocket 不做呀，现在
//  就是纯 custom schema」).
//
//  ## What the transport is not
//
//  Any app may register `velawallet://`, and which one wins is undefined on
//  iOS. So this carries an answer and authorises nothing: the core accepts
//  only a `webauthn.get` over exactly the digest this wallet computed, by a
//  credential it holds. What an interception costs is availability (the
//  wallet waits) and confidentiality (the interceptor learns the assertion).
//
//  ## One visit per request (spec 075 over this transport)
//
//  A flow — create then the member proof, a sign-in then recovery's two
//  proofs — is still one flow, ended by `end()`. But a URL carries exactly
//  one request, so each request is its own page visit with its own one-time
//  token, and `send(_:)` opens the page again rather than writing down a
//  socket that is not there.
//

import Foundation
import Network
import VelaCore

/// One request of a session: a passkey ceremony (spec 075) or a signature
/// (071). Both ride the same channels; only who judges the answer differs,
/// and that judge is the core's.
enum TrustedSignerAsk {
    /// `request` is `trustedSignerCeremonyRequest`'s, `operationJson` the
    /// machine operation's own wire JSON, `memberChallenge` the registry
    /// challenge the WALLET fetched (a member proof only).
    case ceremony(request: String, operationJson: String, memberChallenge: Data?)
    /// `request` is `trustedSignerRequest`'s; the answer must sign `digest`
    /// with one of `keys`.
    case signature(request: String, digest: Data, keys: [WalletKeyRecord])
}

/// The page this flow is talking to. One implementation — the cross-device
/// ones were retired by the owner on 2026-09-23 — kept as a protocol because
/// it is what the driver holds while a flow is open.
protocol TrustedSignerConversation: AnyObject {
    /// The flow's next request: a new visit to the page, with a new one-time
    /// token.
    func send(_ ask: TrustedSignerAsk) async -> TrustedSignerChannel.Ending
    /// The flow is over.
    func end()
}

/// The page's refusal codes in the core's vocabulary.
///
/// What used to live here — building the `intent` envelope, reading a
/// `result` frame — belonged to the socket channels and went with them. On
/// the custom scheme the core writes the whole launch URL and reads the whole
/// callback, so a shell has nothing left to shape.
enum TrustedSignerAnswer {

    /// The page's `code`, in the core's vocabulary — `trusted_signer::refusal`,
    /// which the bindings do not export on its own. A signing answer is the
    /// only place a shell needs it: a ceremony's `error` goes to
    /// `trustedSignerVerifyCeremony`, which reads the code itself.
    static func refusal(code: String) -> TrustedSignerRefusal {
        code.isEmpty || code == "user_rejected" ? .declined : .pageRefused(code: code)
    }
}

final class TrustedSignerChannel: TrustedSignerConversation {

    /// How one REQUEST of a flow ended.
    enum Ending: Equatable {
        /// The core's verdict on a signature: an answer it verified, or why
        /// there is none — `declined` for a page that went away holding the
        /// request, and for a cancel.
        case outcome(TrustedSignerOutcome)
        /// Spec 075: the core's verdict on a passkey ceremony.
        case ceremony(TrustedSignerCeremonyOutcome)
        /// Nobody answered in time. The core has no clock; this is the shell's.
        case timedOut
        /// The request could not be put to a page at all.
        case unavailable
    }

    /// The contract's five minutes — idle, so a flow of several requests is
    /// not cut off halfway (contract §1.5).
    static let defaultTimeout: TimeInterval = 5 * 60

    let signerUrl: String
    /// The CURRENT visit's one-time token. A new one per request, so
    /// "one-time" is literally true and no answer can be read as another
    /// request's.
    private(set) var token: String
    /// The URL the current visit was opened with — what "open the page again"
    /// re-opens, unchanged, token and all.
    private(set) var launchUrl: URL?

    /// The request in flight. A refusal is shaped like the request that
    /// earned it, so a ceremony's decline never arrives as a signing verdict
    /// (and the spine never reads one as a shell bug).
    private var current: TrustedSignerAsk
    private let timeout: TimeInterval

    /// Opens a page. Set by whoever owns the tab, because presenting is its
    /// business and not this class's.
    var openPage: ((URL) -> Void)?

    /// Terminal: a timeout, a cancel, or a request that could not go out.
    /// Answered to every later caller.
    private var terminal: Ending?
    /// The flow has been shut down (`end()`), with no terminal verdict.
    private var closed = false
    /// A verdict nobody was waiting for yet.
    private var pendingEndings: [Ending] = []
    private var waiting: [CheckedContinuation<Ending, Never>] = []
    private var deadline: DispatchWorkItem?

    init(
        signerUrl: String,
        first: TrustedSignerAsk,
        timeout: TimeInterval = TrustedSignerChannel.defaultTimeout
    ) {
        self.signerUrl = signerUrl
        self.current = first
        self.timeout = timeout
        self.token = Self.freshToken()
    }

    /// The 071 door: one signature, one page visit.
    convenience init(
        signerUrl: String,
        requestJson: String,
        digest: Data,
        keys: [WalletKeyRecord],
        timeout: TimeInterval = TrustedSignerChannel.defaultTimeout
    ) {
        self.init(
            signerUrl: signerUrl,
            first: .signature(request: requestJson, digest: digest, keys: keys),
            timeout: timeout
        )
    }

    /// Whether anything more can happen here.
    private var stopped: Bool { closed || terminal != nil }

    // MARK: - Putting a request

    /// The first request: build its URL, start waiting for its callback, and
    /// hand the URL back so the caller can open the page. `nil` when the core
    /// would not build one — a request that is not JSON, which is a bug in
    /// this app rather than anything a person did.
    func start() -> URL? {
        guard !stopped, let url = visit(current) else {
            finish(.unavailable)
            return nil
        }
        armDeadline()
        return url
    }

    /// The flow's next request. On this transport that is a NEW visit: a new
    /// token, a new URL, and the page opened again — there is no socket to
    /// write down.
    func send(_ ask: TrustedSignerAsk) async -> Ending {
        guard !stopped else { return .unavailable }
        TrustedSignerCallbacks.forget(token)
        token = Self.freshToken()
        current = ask
        guard let url = visit(ask) else { return .unavailable }
        armDeadline()
        openPage?(url)
        return await ending()
    }

    /// Build this visit's URL and register for its callback.
    private func visit(_ ask: TrustedSignerAsk) -> URL? {
        let requestJson = switch ask {
        case .ceremony(let request, _, _): request
        case .signature(let request, _, _): request
        }
        guard let launch = try? trustedSignerUrlLaunch(
            base: signerUrl, requestJson: requestJson, token: token
        ), let url = URL(string: launch) else { return nil }
        launchUrl = url
        TrustedSignerCallbacks.await(token) { [weak self] callback in
            self?.answered(callback)
        }
        return url
    }

    /// Waits for the CURRENT request's verdict. A cancelled task cancels the
    /// ceremony — the send screen's cancel is a `Task` cancel, and it must
    /// not leave a request waiting on a callback that will never come.
    func ending() async -> Ending {
        if let next = take() { return next }
        return await withTaskCancellationHandler {
            await withCheckedContinuation { continuation in
                if let next = take() {
                    continuation.resume(returning: next)
                } else {
                    waiting.append(continuation)
                }
            }
        } onCancel: {
            Task { @MainActor [weak self] in self?.cancel() }
        }
    }

    /// The verdict already in hand, if there is one.
    private func take() -> Ending? {
        if !pendingEndings.isEmpty { return pendingEndings.removeFirst() }
        if let terminal { return terminal }
        return closed ? .unavailable : nil
    }

    // MARK: - The answer

    /// A `velawallet://sign-result?…` for THIS visit's token. The core reads
    /// it and the core judges what it carried.
    private func answered(_ callback: URL) {
        guard !stopped else { return }
        let read = trustedSignerParseCallback(url: callback.absoluteString, token: token)
        switch read {
        case .answered(let answerJson):
            // Unwrapped, because that is what the URL channel carries: a
            // signature's `result` object, or a ceremony's own envelope
            // (`intake.js`: `body.result !== undefined ? body.result : body`).
            switch current {
            case .signature(_, let digest, let keys):
                deliver(.outcome(trustedSignerVerify(
                    resultJson: answerJson, digest: digest, keys: keys
                )))
            case .ceremony(_, let operationJson, let memberChallenge):
                deliver(.ceremony(trustedSignerVerifyCeremony(
                    operationJson: operationJson, answerJson: answerJson,
                    signerOrigin: signerUrl, expectedMemberChallenge: memberChallenge
                )))
            }
        case .refused(let refusal):
            deliver(refused(refusal))
        }
    }

    // MARK: - Endings

    /// A refusal shaped like the request that earned it.
    static func declined(for ask: TrustedSignerAsk) -> Ending {
        refused(.declined, for: ask)
    }

    static func refused(_ refusal: TrustedSignerRefusal, for ask: TrustedSignerAsk) -> Ending {
        switch ask {
        case .ceremony: .ceremony(.refused(refusal: refusal))
        case .signature: .outcome(.refused(refusal: refusal))
        }
    }

    /// The same, for the request in flight.
    private func refused(_ refusal: TrustedSignerRefusal) -> Ending {
        Self.refused(refusal, for: current)
    }

    /// The flow is done.
    func end() {
        guard !stopped else { return }
        closed = true
        shutDown()
        let waiters = waiting
        waiting.removeAll()
        waiters.forEach { $0.resume(returning: .unavailable) }
    }

    /// The waiting sheet's cancel: the person declined (contract §2).
    func cancel() {
        finish(refused(.declined))
    }

    /// The person closed the tab.
    ///
    /// On this transport that is NOT an answer. A page can be closed after it
    /// has navigated to the callback — the tab often outlives the navigation
    /// — so reading a dismissal as "closed without signing" would turn a
    /// signature the person just approved into a refusal. The callback that
    /// is on its way settles the request; if none comes, the clock does, and
    /// the waiting sheet keeps its two ways on in the meantime.
    func pageClosed() {}

    /// One request's verdict, to whoever is waiting for it — or kept for the
    /// caller that has not asked yet.
    private func deliver(_ ending: Ending) {
        guard !stopped else { return }
        armDeadline()
        if waiting.isEmpty {
            pendingEndings.append(ending)
        } else {
            waiting.removeFirst().resume(returning: ending)
        }
    }

    /// A terminal ending: the first one wins, and everything closes with it.
    private func finish(_ ending: Ending) {
        guard !stopped else { return }
        terminal = ending
        shutDown()
        let waiters = waiting
        waiting.removeAll()
        waiters.forEach { $0.resume(returning: ending) }
    }

    private func shutDown() {
        deadline?.cancel()
        deadline = nil
        TrustedSignerCallbacks.forget(token)
    }

    // MARK: - The clock

    /// The idle clock, restarted. A flow of several requests is cut off only
    /// when nothing has moved for the whole window (contract §1.5).
    private func armDeadline() {
        deadline?.cancel()
        guard !stopped else { return }
        let deadline = DispatchWorkItem { [weak self] in
            MainActor.assumeIsolated { self?.finish(.timedOut) }
        }
        self.deadline = deadline
        DispatchQueue.main.asyncAfter(deadline: .now() + timeout, execute: deadline)
    }

    // MARK: - The token

    /// 16 bytes from the system's CSPRNG, base64url without padding.
    static func freshToken() -> String {
        var generator = SystemRandomNumberGenerator()
        let bytes = (0..<16).map { _ in UInt8.random(in: .min ... .max, using: &generator) }
        return Data(bytes).base64EncodedString()
            .replacingOccurrences(of: "+", with: "-")
            .replacingOccurrences(of: "/", with: "_")
            .replacingOccurrences(of: "=", with: "")
    }
}

/// The callbacks this app is waiting for, by the one-time token they name.
///
/// App-wide rather than held by a view, because the two ends are in different
/// places: a request waits inside whatever task asked for a signature, and the
/// callback arrives at `onOpenURL` on the root view.
///
/// Keyed by the token, and only the token: it is what says WHICH request a
/// callback belongs to. A callback for a token nobody awaits reaches nothing
/// and is dropped in silence — no route change, no state change, no error a
/// person cannot act on.
@MainActor
enum TrustedSignerCallbacks {

    private static var waiting: [String: (URL) -> Void] = [:]

    static func await(_ token: String, _ sink: @escaping (URL) -> Void) {
        waiting[token] = sink
    }

    static func forget(_ token: String) {
        waiting.removeValue(forKey: token)
    }

    /// A `velawallet://…` the system handed this app.
    ///
    /// **It is an event for a pending request, not a navigation.** `true` when
    /// it settled one, and `false` for every other URL — the wallet's own
    /// `pay` and `open` links included — which the caller then routes as it
    /// always did. Nothing here touches the screen either way.
    @discardableResult
    static func deliver(_ url: URL) -> Bool {
        guard let token = trustedSignerCallbackToken(url: url.absoluteString),
              let sink = waiting.removeValue(forKey: token)
        else { return false }
        sink(url)
        return true
    }
}
