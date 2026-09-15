//
//  SessionController.swift
//  VelaWallet
//
//  The session machine, app-resident.
//
//  One per process, outliving every screen — which is the whole reason it is not
//  a per-screen model. `allowedRoute` is the route guard for the entire app, and
//  a guard recreated whenever a screen is recreated would spend the first frame
//  after every rebuild reporting `loading` and bouncing the person back to
//  onboarding.
//
//  The division of labour is the contract's: **the core decides WHAT is allowed,
//  this class decides nothing, and RootView decides WHEN to move.**
//

import Foundation
import Observation
import VelaCore

@MainActor
@Observable
final class SessionController {

    private(set) var view: SessionView = .booting

    private let store: AccountStore
    private let executor: SessionExecutor
    private var driver: CoreDriver!

    init(store: AccountStore) {
        self.store = store
        self.executor = SessionExecutor(store: store)
        self.driver = CoreDriver(
            bridge: SessionCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] json in
                guard let decoded = try? CoreJSON.decode(SessionView.self, from: json) else { return }
                self?.view = decoded
            },
            onFault: { error in
                print("[vela-wallet] session fault: \(error)")
            }
        )
        // A refused answer becomes the machine's OWN failure, once (contract
        // 048 §2). Without it the session sits in `restoring` forever, which a
        // person sees as a wallet that will not open — and the one thing worse
        // than "your accounts cannot be read" is silence where that sentence
        // belongs.
        driver.toFailure = { _, _ in
            CoreJSON.string(["type": "accounts_unavailable"])
        }
    }

    /// Read storage and settle on a route. Called once, at launch.
    func boot() { driver.dispatch(Self.event("boot")) }

    /// Hand a finished onboarding over.
    ///
    /// `mode` is the core's own `CompletionMode` object, forwarded UNTOUCHED
    /// from the onboarding machine to the session machine. It carries either a
    /// whole restored account list or a single new account, and reshaping it
    /// here — for instance by pulling out an address and rebuilding a record —
    /// is exactly the field-by-field copy that drops `keys` and re-derives a
    /// different, wrong, single-key wallet.
    func accountEstablished(mode: [String: Any]) {
        driver.dispatch(CoreJSON.string(["type": "account_established", "mode": mode]))
    }

    func switchAccount(index: Int) {
        driver.dispatch(CoreJSON.string(["type": "switch_account", "index": index]))
    }

    func signOut() { driver.dispatch(Self.event("sign_out")) }
    func signOutConfirmed() { driver.dispatch(Self.event("sign_out_confirmed")) }
    func signOutDismissed() { driver.dispatch(Self.event("sign_out_dismissed")) }

    /// The endpoint override, for the surface an unreachable index opens.
    func registryURL() async -> String {
        await store.loadRegistryURL() ?? RegistryClient.defaultURL
    }

    func setRegistryURL(_ url: String) async {
        let normalized = RegistryClient.normalize(url)
        await store.saveRegistryURL(normalized == RegistryClient.defaultURL ? nil : normalized)
    }

    private static func event(_ type: String) -> String {
        CoreJSON.string(["type": type])
    }
}
