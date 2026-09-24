//
//  CoreStore.swift
//  VelaWallet
//
//  One Crux machine, alive for as long as the app, with its view decoded.
//
//  `CoreDriver` already knows how to run the effect loop; what it cannot know
//  is which view type comes back or when a machine should first be spoken to.
//  This is that half, written once and generic over the machine, because N
//  copies of "decode, remember, boot exactly once" is N chances to get one of
//  them subtly wrong.
//
//      screen        machine store        CoreStore<View>        CoreDriver
//        │ .task ───► boot() ────────────► boot(event) ─────────► dispatch
//        │                                       │◄── onView(json) ──┤
//        │◄──── @Observable view ◄── onView(View) ┘
//
//  ## Why residency
//
//  A machine that dies with its screen re-probes the world every time somebody
//  opens 设置, and a debounce armed against a dead core is an effect nobody will
//  answer. `SessionController` made this argument first for the session machine
//  and it holds for every wallet-state machine: the core's model IS the app's
//  state, so its lifetime is the app's.
//
//  ## Why boot is lazy while construction is eager
//
//  Constructing a core is a `Model::default()` and an empty map — cheap enough
//  to do at launch for every machine. The FIRST EVENT is what reads storage and
//  opens sockets, so that is what waits for a screen to actually appear.
//  `boot` is idempotent: a screen that appears, disappears and reappears must
//  not re-read the world, because the resident model already holds it.
//
//  ## What this class deliberately does NOT do
//
//  It holds no opinion about the machine behind it. No product vocabulary, no
//  per-machine events, no display models. Those live in the machine's own store
//  and its live builder — which is what makes wiring the third machine cost
//  nothing here (spec 050 SC-004).
//

import Foundation

@MainActor
final class CoreStore<View: Decodable> {

    /// The last view the core committed, decoded. `nil` means the machine has
    /// not been booted yet — a real state, and the one FR-009's neutral surface
    /// renders from. It is never a stand-in for fixture data.
    private(set) var view: View?

    private let driver: CoreDriver
    private let onView: (View) -> Void
    private let onFault: (Error) -> Void
    private var booted = false

    /// - Parameters:
    ///   - bridge: the uniffi object for this machine.
    ///   - perform: the machine's executor. Must not throw — see `CoreDriver`.
    ///   - onView: called on every committed view, in the core's own order.
    ///   - onFault: a shell fault (a malformed event, an unreadable view).
    ///     Never a user-facing error.
    ///   - neutralAnswer: the machine's own answer for an operation whose
    ///     result the core refused — see `CoreDriver.neutralAnswer`. `nil`
    ///     leaves the fault reported and nothing else.
    init(
        bridge: CoreBridge,
        perform: @escaping ([String: Any]) async -> String,
        onView: @escaping (View) -> Void = { _ in },
        onFault: @escaping (Error) -> Void = { _ in },
        neutralAnswer: (([String: Any]) -> String?)? = nil
    ) {
        self.onView = onView
        self.onFault = onFault
        // `self` is captured after full initialisation, so the closures cannot
        // observe a half-built store.
        var commit: (([String: Any]) -> Void)!
        var fault: ((Error) -> Void)!
        self.driver = CoreDriver(
            bridge: bridge,
            perform: perform,
            onView: { json in commit(json) },
            onFault: { error in fault(error) }
        )
        driver.neutralAnswer = neutralAnswer
        commit = { [weak self] json in self?.commit(json) }
        fault = { [weak self] error in self?.onFault(error) }
    }

    /// Send the machine's first event, at most once for the life of the app.
    ///
    /// Returns `true` when this call is the one that booted it, which is what
    /// lets a caller decide whether a follow-up event is redundant.
    @discardableResult
    func boot(_ eventJson: String) -> Bool {
        guard !booted else { return false }
        booted = true
        driver.dispatch(eventJson)
        return true
    }

    /// Send one event, as the JSON the machine's `Event` deserializes from.
    ///
    /// Dropped before boot on purpose: every machine here reads its stores on
    /// the boot event, and an event that lands first would act on an empty
    /// model and be silently discarded by the core.
    func dispatch(_ eventJson: String) {
        guard booted else { return }
        driver.dispatch(eventJson)
    }

    /// Written for the compiler, not for the runtime: it is empty on purpose.
    ///
    /// The target builds with `-default-isolation MainActor`, which makes every
    /// deinit an ISOLATED deinit (SE-0371, new in Swift 6.2). Optimising the
    /// deallocating destructor of a GENERIC class in that mode crashes
    /// swift-frontend 6.2.4 — `EarlyPerfInliner`, inside
    /// `isCallerAndCalleeLayoutConstraintsCompatible`. It reproduces in three
    /// lines (`app-ios/scripts/check-generic-class-deinit.mjs` carries them) and
    /// it is why Release and Archive could not be built at all until 058.
    /// Writing the deinit `nonisolated` is the whole fix: it restores the
    /// pre-6.2 destructor, which is what this class always had — nothing here
    /// touches main-actor state on the way out.
    ///
    /// Delete it when the toolchain is fixed, and let the guard tell you.
    nonisolated deinit {}

    private func commit(_ json: [String: Any]) {
        do {
            let decoded = try CoreJSON.decode(View.self, from: json)
            view = decoded
            onView(decoded)
        } catch {
            // A view this app cannot read means the core and this client
            // disagree about the wire. Reporting it beats rendering a default,
            // which would show somebody a screen the machine is not in.
            onFault(error)
        }
    }
}
