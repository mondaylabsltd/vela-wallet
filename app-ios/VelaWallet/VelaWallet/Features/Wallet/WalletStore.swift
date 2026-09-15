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
        // The port closes over this store, so it is installed after it exists
        // — the same shape `SendStore` uses for its two.
        executor.onChainAssets = { [weak self] tokens in
            self?.chainAssetsArrived(tokens)
        }
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

    /// A mid-fetch snapshot from the executor's fan-out.
    ///
    /// **Tagged with the account it is for.** The core requires it — "a stale
    /// account's stream can never paint the new account (invariant ⑤)" — and
    /// this client left the field out, so every snapshot was REFUSED:
    ///
    ///     balance_dashboard fault: invalid event from shell: missing field `address`
    ///
    /// once per chain, per refresh, forever. The progressive total the fan-out
    /// exists to paint never arrived; the home only moved when a later event
    /// happened to carry the whole answer. Found in the device console on
    /// 2026-09-15 while looking at something else entirely.
    func chainAssetsArrived(_ tokens: [[String: Any]]) {
        guard let address = scopedTo, !address.isEmpty else { return }
        core.dispatch(Self.chainAssetsEvent(address: address, tokens: tokens))
    }

    /// The event, as a value, so a test can hand the REAL core the very JSON
    /// this store sends. A test that rebuilt the shape itself would have
    /// passed for the two specs this field was missing.
    static func chainAssetsEvent(address: String, tokens: [[String: Any]]) -> String {
        CoreJSON.string([
            "type": "chain_assets_arrived", "address": address, "tokens": tokens,
        ])
    }

    /// The account switcher opened (spec 056).
    ///
    /// The roster travels because the SESSION machine owns accounts and this
    /// one owns balances — the core reads the cache for every address at once
    /// so the sheet opens on numbers instead of on spinners, then refreshes
    /// behind them.
    func switcherOpened(addresses: [String]) {
        core.dispatch(CoreJSON.string([
            "type": "switcher_opened", "addresses": addresses,
        ]))
    }

    func switcherClosed() { core.dispatch(CoreJSON.string(["type": "switcher_closed"])) }

    /// A chain that failed, retried. Clears it from the failed list and
    /// re-fetches — the "fix this network" affordance's other half.
    func fixChain(_ chainId: Int) {
        core.dispatch(CoreJSON.string(["type": "fix_chain_resolved", "chain_id": chainId]))
    }

    func appFocused() { core.dispatch(CoreJSON.string(["type": "app_focused"])) }
    func appBackgrounded() { core.dispatch(CoreJSON.string(["type": "app_backgrounded"])) }
}
