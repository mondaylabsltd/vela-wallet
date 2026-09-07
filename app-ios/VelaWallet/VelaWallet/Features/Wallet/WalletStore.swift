//
//  WalletStore.swift
//  VelaWallet
//
//  The `balance_dashboard` machine, app-resident.
//
//  Residency earns its keep here more visibly than anywhere: the machine holds
//  the fetched holdings, the cached total and a retry timer. One that died with
//  the home screen would re-read twelve chains every time somebody came back
//  from the address book.
//
//  It owns no rules. What the total is, whether it is partial, whether a chain
//  should be retried, whether a token is spam — all decided in Rust.
//

import Foundation
import Observation
import VelaCore

extension BalanceDashboardCore: CoreBridge {}

@MainActor
@Observable
final class WalletStore {

    /// The core's view, decoded. `nil` until the home screen boots the machine.
    private(set) var balance: BalanceViewWire?

    private let executor: BalanceExecutor
    private var core: CoreStore<BalanceViewWire>!
    /// The address the dashboard is currently reading for, so re-entering the
    /// tab does not re-announce the same account and throw away its holdings.
    private var scopedTo: String?

    init(store: VelaStore, pool: RpcPool, held: HeldTokens) {
        self.executor = BalanceExecutor(store: store, pool: pool, held: held)
        self.core = CoreStore(
            bridge: BalanceDashboardCore(),
            perform: { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.balance = view },
            onFault: { print("[vela-wallet] balance_dashboard fault: \($0)") }
        )
    }

    /// Called from the home screen's `.task`, with the signed-in address.
    ///
    /// The privacy state is hydrated **before** the account, so a person who
    /// hid their balance never sees it flash on a cold start.
    func open(address: String) {
        guard !address.isEmpty else { return }
        if core.boot(CoreJSON.string(["type": "privacy_hydrated",
                                      "hidden": executor.hiddenAtLaunch])) {
            scopedTo = nil
        }
        guard scopedTo != address else { return }
        scopedTo = address
        core.dispatch(CoreJSON.string(["type": "account_changed", "address": address]))
    }

    /// Pull to refresh. `pull` is carried so the core can tell a person's own
    /// gesture from a background refresh and answer it differently.
    func refresh(pull: Bool = true) {
        core.dispatch(CoreJSON.string([
            "type": "refresh_requested", "force": true, "pull": pull,
        ]))
    }

    /// Hold a pull gesture open until the refresh it started is done.
    ///
    /// `refresh(pull:)` dispatches and returns, so awaiting nothing would snap
    /// the spinner away before a single chain had answered — which reads as
    /// "already up to date" over stale figures. Two waits, because the core
    /// does not flip `refreshing` until its first effect resolves: first for
    /// the refresh to START, then for it to finish.
    ///
    /// The cap is a give-up, not a decision: twelve chains can genuinely be
    /// slow, but a spinner that never stops is worse than a figure that is a
    /// few seconds old.
    func settled(timeout: TimeInterval = 20) async {
        let started = Date()
        while Date().timeIntervalSince(started) < 2, balance?.refreshing != true {
            try? await Task.sleep(nanoseconds: 100_000_000)
        }
        while Date().timeIntervalSince(started) < timeout, balance?.refreshing == true {
            try? await Task.sleep(nanoseconds: 200_000_000)
        }
    }

    /// Tap the figure to hide it. A display state the core owns, so it survives
    /// a relaunch rather than being re-hidden by hand each time.
    func togglePrivacy() {
        core.dispatch(CoreJSON.string(["type": "privacy_toggled"]))
    }

    func appFocused() { core.dispatch(CoreJSON.string(["type": "app_focused"])) }
    func appBackgrounded() { core.dispatch(CoreJSON.string(["type": "app_backgrounded"])) }
}
