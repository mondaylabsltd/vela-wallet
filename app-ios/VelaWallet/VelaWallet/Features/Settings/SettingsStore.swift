//
//  SettingsStore.swift
//  VelaWallet
//
//  The `network_admin` machine, app-resident.
//
//  Residency matters more here than anywhere else in this cut. This machine
//  probes endpoints over the network and holds a search debounce; one that died
//  with the settings route would re-probe every chain each time somebody opened
//  设置, and a debounce armed against a dead core is an effect nobody will
//  answer.
//
//  It owns no rules. Which chains exist, whether one may be added, what makes an
//  RPC compatible, when an edit is held back — all of it is decided and tested
//  in Rust. This file is a lifetime, a decoded view, and the events the drawn
//  controls can raise.
//

import Foundation
import Observation
import VelaCore

/// uniffi generates one class per exported machine with no shared supertype.
/// Declared beside its user so adding a machine touches no shared file.
extension NetworkAdminCore: CoreBridge {}
extension DisplayCurrencyCore: CoreBridge {}
extension FeeTierPrefCore: CoreBridge {}
extension SignPrefCore: CoreBridge {}

/// `fee_tier_pref`'s view (spec 069): the tier every send STARTS at — always a
/// real one, the factory `fast` when nothing was chosen.
struct FeeTierPrefViewWire: Decodable, Equatable {
    let tier: String
    /// `false` ⇒ the factory default is showing, not a choice.
    let committed: Bool
    /// The tiers Settings may offer, fastest first — never the dead `rapid`.
    let offered: [String]
}

@MainActor
@Observable
final class SettingsStore {

    /// The core's view, decoded. `nil` until the settings route boots the
    /// machine — a real state, and the one the fixture-shaped neutral surface
    /// renders from.
    private(set) var networkAdmin: NetViewWire?

    /// Which currency amounts are shown in (spec 050, phase 4).
    ///
    /// The second machine on this screen, and it lives here rather than in its
    /// own store for the reason the screen is one value: 设置 is one surface,
    /// and a caller should not have to know how many cores are behind it.
    private(set) var currency: CurrencyViewWire?

    /// The default transaction speed (spec 069) — app-wide like the currency:
    /// Settings shows it and every send starts at it.
    private(set) var feeTier: FeeTierPrefViewWire?

    /// How this device signs by default, and which Clear Signer page it opens
    /// (spec 071) — app-wide like the speed: every signing sheet starts at it.
    /// Seeded with the machine's own first view, so a sheet raised before the
    /// stored values land still lists what the core offers.
    private(set) var signPref: SignPrefViewWire?

    /// `true` once the core has read all four stores. Mutations sent before it
    /// are dropped by the core.
    var isLoaded: Bool { networkAdmin?.loaded == true }

    private let executor: NetworkAdminExecutor
    private let currencyExecutor: DisplayCurrencyExecutor
    private var core: CoreStore<NetViewWire>!
    private var currencyCore: CoreStore<CurrencyViewWire>!
    private var feeTierCore: CoreStore<FeeTierPrefViewWire>!
    private var signPrefCore: CoreStore<SignPrefViewWire>!

    /// `pool` is the app's one `rpc_pool` session (FR-002). The currency
    /// machine needs it because its first rate rung is Chainlink's fiat feeds
    /// on Ethereum mainnet — a chain read, and therefore a routed one.
    ///
    /// `networkPerform` answers the networks machine instead of its executor —
    /// the hermetic tests' door (spec 072): every event this class sends is put
    /// to the REAL machine there, with no network behind it, so a spelling the
    /// core cannot read fails a test rather than a person's screen.
    init(
        store: VelaStore,
        accounts: AccountStore,
        pool: RpcPool,
        networkPerform: (([String: Any]) async -> String)? = nil
    ) {
        self.executor = NetworkAdminExecutor(store: store, accounts: accounts)
        self.currencyExecutor = DisplayCurrencyExecutor(store: store, accounts: accounts, pool: pool)
        self.core = CoreStore(
            bridge: NetworkAdminCore(),
            perform: networkPerform ?? { [executor] operation in await executor.perform(operation) },
            onView: { [weak self] view in self?.networkAdmin = view },
            // A shell fault here is a malformed event or a view this build
            // cannot read — never a person's mistake. Swallowing it silently is
            // how a screen stops responding with nothing in any log to say why.
            onFault: { print("[vela-wallet] network_admin fault: \($0)") }
        )
        self.currencyCore = CoreStore(
            bridge: DisplayCurrencyCore(),
            perform: { [currencyExecutor] operation in await currencyExecutor.perform(operation) },
            onView: { [weak self] view in self?.currency = view },
            onFault: { print("[vela-wallet] display_currency fault: \($0)") }
        )
        let feeTierExecutor = FeeTierExecutor(store: store)
        self.feeTierCore = CoreStore(
            bridge: FeeTierPrefCore(),
            perform: { operation in feeTierExecutor.perform(operation) },
            onView: { [weak self] view in self?.feeTier = view },
            onFault: { print("[vela-wallet] fee_tier_pref fault: \($0)") }
        )
        self.signPref = SignPrefViewWire.initial
        let signPrefExecutor = SignPrefExecutor(store: store)
        self.signPrefCore = CoreStore(
            bridge: SignPrefCore(),
            perform: { operation in signPrefExecutor.perform(operation) },
            onView: { [weak self] view in self?.signPref = view },
            onFault: { print("[vela-wallet] sign_pref fault: \($0)") }
        )
    }

    /// Boot the default-speed machine. App-wide and idempotent, like the
    /// currency: the send form's folded control shows THIS, not a hardcoded
    /// tier, so it has to be read before anybody opens Settings.
    func openFeeTier() {
        feeTierCore.boot(CoreJSON.string(["type": "refresh"]))
    }

    /// A row of the speed sheet — and only that sheet: a pick on the send
    /// screen is one-shot and never comes here.
    func chooseFeeTier(_ tier: String) {
        feeTierCore.dispatch(CoreJSON.string(["type": "user_chose", "tier": tier]))
    }

    /// Boot the signing-preferences machine. App-wide and idempotent: the
    /// first signing sheet starts at what it read, whether or not anybody has
    /// opened Settings.
    func openSignPref() {
        signPrefCore.boot(CoreJSON.string(["type": "refresh"]))
    }

    /// Settings' "Sign with" — the default, and only from there: a sheet's own
    /// pick is one request's and never comes here.
    func chooseSignMethod(_ method: String) {
        signPrefCore.dispatch(CoreJSON.string(["type": "method_chosen", "method": method]))
    }

    /// The Clear Signer page, as typed. The core validates, and stores
    /// nothing it refuses.
    func submitSignerUrl(_ text: String) {
        signPrefCore.dispatch(CoreJSON.string(["type": "signer_url_submitted", "text": text]))
    }

    /// Back to the official page.
    func resetSignerUrl() {
        signPrefCore.dispatch(CoreJSON.string(["type": "signer_url_reset"]))
    }

    /// USD → that currency, through the display machine's own waterfall.
    ///
    /// The payroll importer's rate port (spec 054 US3). Exposed here rather
    /// than rebuilt there so a currency the wallet cannot price stays
    /// unpriceable in both places — see `DisplayCurrencyExecutor.resolve`.
    func usdRate(_ code: String) async -> Double? {
        await currencyExecutor.resolve(code)
    }

    /// Called from the settings route's `.task`. Idempotent: the machines read
    /// their stores once and keep them.
    func open() {
        openNetworks()
        openCurrency()
        // The page's two signing rows read what is stored, however Settings
        // was reached (spec 071).
        openSignPref()
    }

    /// Boot the networks machine alone — it reads its four stores once and
    /// keeps them. Idempotent, like `open()`.
    func openNetworks() {
        core.boot(CoreJSON.string(["type": "started"]))
    }

    /// Boot the currency machine alone.
    ///
    /// Which currency somebody reads their money in is **app-wide**, not a
    /// property of the settings screen: the home screen's hero is the figure
    /// that matters most, and waiting for a visit to 设置 to learn the person
    /// chose CNY would show them a dollar figure for as long as they never went
    /// looking. Idempotent, like `open()`.
    func openCurrency() {
        currencyCore.boot(CoreJSON.string(["type": "refresh"]))
    }

    /// A row of the currency picker.
    func chooseCurrency(_ code: String) {
        currencyCore.dispatch(CoreJSON.string(["type": "user_chose", "code": code]))
    }

    // MARK: - What the drawn controls raise

    /// The wizard's search box.
    func search(_ query: String) {
        core.dispatch(CoreJSON.string(["type": "search_input", "query": query]))
    }

    /// A row of the search results.
    func selectChain(_ chainId: Int, keepCustomRpc: Bool = false) {
        core.dispatch(CoreJSON.string([
            "type": "chain_selected",
            "chain_id": chainId,
            "keep_custom_rpc": keepCustomRpc,
        ]))
    }

    /// The wizard's RPC-override field.
    func editCustomRpc(_ value: String) {
        core.dispatch(CoreJSON.string(["type": "custom_rpc_edited", "value": value]))
    }

    /// 添加.
    ///
    /// `now_iso` is the shell's clock because the core has none; it dates the
    /// stored record. The core still refuses a candidate it never verified, so
    /// this cannot add a chain past a failed probe.
    func confirmAdd() {
        core.dispatch(CoreJSON.string(["type": "add_confirmed", "now_iso": Self.nowISO]))
    }

    func resetWizard() {
        core.dispatch(CoreJSON.string(["type": "wizard_reset"]))
    }

    /// Opening a network's detail page.
    ///
    /// This is what makes the health pills real: the core seeds them
    /// `checking` and starts a probe wave. Without it the page would sit on
    /// whatever it was born with, which is how it came to show the fixture's
    /// 45ms over an endpoint nothing had contacted.
    func expandNetwork(chainId: Int) {
        core.dispatch(CoreJSON.string(["type": "override_expanded", "chain_id": chainId]))
    }

    func deleteNetwork(id: String) {
        core.dispatch(CoreJSON.string(["type": "delete_confirmed", "id": id]))
    }

    // MARK: - Lookups the screens need

    /// One event, to the networks machine.
    private func dispatch(_ event: [String: Any]) {
        core.dispatch(CoreJSON.string(event))
    }

    // MARK: - The endpoints and providers pages (spec 056 US2)
    //
    // Every event below is spelled as `network_admin.rs`' `Event` spells it.
    // Until 072 five of them sent `"id"` where the core reads `field` or
    // `provider`, one left out `field`, and one sent `text` for a chain id —
    // the core refused each one, so both pages and the RPC fix were drawings
    // that took typing and kept none of it. `NetworkEventsTests` sends every
    // one to the real machine.

    /// A chain id to add without the wizard (the EIP-681 recovery path). The
    /// core resolves, checks and saves it behind the same dedup gate.
    func addByChainId(_ chainId: Int) {
        dispatch(["type": "add_by_chain_id_requested", "chain_id": chainId, "now_iso": Self.nowISO])
    }

    /// ST12 opened. The core probes all four fields.
    func openEndpoints() { dispatch(["type": "endpoints_opened"]) }

    /// One endpoint field, as it is typed and when it is left.
    ///
    /// Two events rather than one because they mean different things: EDITED is
    /// what is on screen, BLURRED is what the person is done saying — and only
    /// the second is worth writing to storage.
    func editEndpoint(_ field: NetEndpointFieldWire, value: String) {
        dispatch(["type": "endpoint_edited", "field": field.rawValue, "value": value])
    }

    func blurEndpoint(_ field: NetEndpointFieldWire) {
        dispatch(["type": "endpoint_blurred", "field": field.rawValue])
    }

    func resetEndpoints() { dispatch(["type": "reset_endpoints_to_defaults"]) }

    /// ST11 opened. The core seeds the drafts from the saved keys and tests
    /// every configured provider.
    func openProviders() { dispatch(["type": "providers_opened"]) }

    func editProviderKey(_ provider: NetProviderIdWire, value: String) {
        dispatch(["type": "provider_key_edited", "provider": provider.rawValue, "value": value])
    }

    func blurProviderKey(_ provider: NetProviderIdWire) {
        dispatch(["type": "provider_key_blurred", "provider": provider.rawValue])
    }

    /// 测试 — the core asks the provider whether the key works.
    func testProvider(_ provider: NetProviderIdWire) {
        dispatch(["type": "provider_test_requested", "provider": provider.rawValue])
    }

    /// One field of a network's detail page, as it is typed and when it is
    /// left. The core probes on every edit and saves on the blur — refusing
    /// only an RPC that proved it serves another chain.
    func editOverride(chainId: Int, field: NetOverrideFieldWire, value: String) {
        dispatch([
            "type": "override_field_edited",
            "chain_id": chainId,
            "field": field.rawValue,
            "value": value,
        ])
    }

    func blurOverride(chainId: Int) {
        dispatch(["type": "override_blurred", "chain_id": chainId])
    }

    /// 用此 RPC 重新检查 — the wizard's chain again, through the RPC typed.
    ///
    /// It used to send only the RPC, which the core stores and does nothing
    /// else with: the link took the tap and re-checked nothing.
    func recheck(customRpc: String) {
        guard let chainId = networkAdmin?.wizard.chainInfo?.chainId else { return }
        editCustomRpc(customRpc)
        selectChain(chainId, keepCustomRpc: true)
    }

    func network(id: String) -> NetNetworkRowWire? {
        networkAdmin?.networks.first { $0.id == id }
    }

    /// ISO-8601 with a `Z`, which is the shape the stored `addedAt` has carried
    /// since the Expo client wrote it.
    private static var nowISO: String {
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime]
        return formatter.string(from: Date())
    }
}
