//
//  VelaStore.swift
//  VelaWallet
//
//  The device's `vela.*` key-value store — the cross-client compatibility seam.
//
//  This is `AccountStore`'s storage half, generalised so the wallet-state
//  machines can reach the same shelf the account list already lives on. It adds
//  no policy: the account rules stay in `AccountStore`, the business rules stay
//  in `vela-core`, and this file only knows how to put JSON text under a name.
//
//  ## Values are JSON TEXT, and that is the contract
//
//  Not a plist array, not an archived object — the exact string web writes to
//  `localStorage` and the Expo client writes to `AsyncStorage`. That is what
//  makes "a contact written on the web reads on the phone" literally true
//  rather than approximately true, and it is why every accessor here goes
//  through `JSONSerialization` instead of `UserDefaults`' typed getters.
//
//  ## Corrupt reads answer empty — they never throw
//
//  A damaged blob must not make a screen permanently unopenable, and it must
//  not be reported to the core as a failure either: `vela-core`'s own operation
//  docs assume it ("Unreadable/corrupt answers as empty, exactly as the TS
//  loaders' `catch { [] }` does"). A machine that receives a *failed* store read
//  stays unloaded forever and silently drops every write that follows.
//
//  ## Records come out whole
//
//  The accessors deal in `[String: Any]`, deliberately. `AccountStore`'s module
//  doc explains what a typed struct costs here: a mapper that copies an account
//  field by field drops `keys`, and the next restore derives a DIFFERENT Safe
//  address. Views get decoded into types (`CoreViews.swift`); records that go
//  back out to disk or to the core are carried, not rebuilt.
//

import Foundation

struct VelaStore {

    /// Every `vela.*` name this app reads or writes, in one place.
    ///
    /// Centralised because they are a contract with three other clients rather
    /// than this app's private business — a typo here is not a local bug, it is
    /// a wallet that cannot see the address book it wrote yesterday. The
    /// capitalised initialisms are load-bearing too: shipped data says
    /// `rpcURL`, not `rpcUrl`.
    enum Key {
        // Owned by onboarding (spec 019).
        static let accounts = "vela.accounts"
        static let activeIndex = "vela.activeAccountIndex"
        static let pendingUploads = "vela.pendingUploads"

        // Owned by `contacts` (spec 050).
        static let contacts = "vela.contacts"
        static let contactsDismissed = "vela.contacts.dismissed"
        static let contactGroups = "vela.contactGroups"

        // Owned by `network_admin` (spec 050).
        static let customNetworks = "vela.customNetworks"
        static let networkConfig = "vela.networkConfig"
        static let rpcProviders = "vela.rpcProviders"

        /// Written by BOTH `network_admin` and `AccountStore` — reach it only
        /// through `AccountStore`, never directly (data-model §5). Two
        /// independent writers is how a person's custom passkey-index endpoint
        /// disappears the first time they open 设置 → 端点.
        static let serviceEndpoints = "vela.serviceEndpoints"

        // Owned by `display_currency` (spec 050).
        static let displayCurrency = "vela.displayCurrency"

        // Owned by the read path (spec 051).
        /// `address → { usd, at }`, 24-hour TTL. The last total the wallet
        /// knew, so a cold start shows a figure instead of a spinner.
        static let balanceCache = "vela.balanceCache"
        /// The ERC-20s the wallet reads balances for.
        static let customTokens = "vela.customTokens"
        /// The local transaction store — the activity feed's single source of
        /// truth, capped at the newest 200 records on every client.
        static let transactionHistory = "vela.transactionHistory"
        /// USD → fiat, as Chainlink's mainnet feeds last stated it (5-minute
        /// TTL in memory; persisted so a cold or offline start still converts).
        static let fiatRates = "vela.fiatRates.v1"
        /// Those feeds' ENS-resolved addresses. The proxies are immutable, so
        /// this is a 30-day cache rather than a lookup on every launch.
        static let fiatFeedAddrs = "vela.fiatFeedAddrs.v1"
        /// The configurable endpoint's USD-based rate map, keyed by the URL it
        /// came from so changing the endpoint refetches.
        static let fxRates = "vela.fxRates.v1"
    }

    private let defaults: UserDefaults

    init(defaults: UserDefaults = .standard) {
        self.defaults = defaults
    }

    // MARK: - Lists

    /// Absent, unparseable, or not-an-array all read as `[]`.
    func readList(_ key: String) -> [[String: Any]] {
        json(key) as? [[String: Any]] ?? []
    }

    func writeList(_ key: String, _ value: [[String: Any]]) {
        write(key, value)
    }

    // MARK: - Objects

    /// Absent, unparseable, or not-an-object all read as `[:]`.
    func readObject(_ key: String) -> [String: Any] {
        json(key) as? [String: Any] ?? [:]
    }

    func writeObject(_ key: String, _ value: [String: Any]) {
        write(key, value)
    }

    // MARK: - Bare strings

    /// `nil` means the key was never written — which several machines read as a
    /// real answer ("the person never chose a currency"), distinct from any
    /// value the key could hold.
    /// Whether this key has ever been written.
    ///
    /// The difference between "nothing here" and "could not be read" is a real
    /// one for the send history: an empty list tells the core nobody has ever
    /// been paid, and it would then treat every address as a first
    /// interaction — the address-poisoning warning, shown to everyone, forever.
    func hasKey(_ key: String) -> Bool {
        defaults.object(forKey: key) != nil
    }

    func readString(_ key: String) -> String? {
        guard let raw = defaults.string(forKey: key), !raw.isEmpty else { return nil }
        return raw
    }

    func writeString(_ key: String, _ value: String?) {
        guard let value, !value.isEmpty else {
            defaults.removeObject(forKey: key)
            return
        }
        defaults.set(value, forKey: key)
    }

    func remove(_ key: String) {
        defaults.removeObject(forKey: key)
    }

    // MARK: - The one parse, the one write

    private func json(_ key: String) -> Any? {
        guard let raw = defaults.string(forKey: key),
              let data = raw.data(using: .utf8)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: data)
    }

    /// A value that cannot be serialized is DROPPED rather than written badly.
    ///
    /// Writing half a record, or writing `"[object Object]"`, would corrupt a
    /// shelf three other clients read. Keeping yesterday's good bytes is the
    /// better failure: the core's ledger is authoritative in memory, so the
    /// person's session is unaffected and the next successful write repairs it.
    private func write(_ key: String, _ value: Any) {
        guard JSONSerialization.isValidJSONObject(value),
              let data = try? JSONSerialization.data(withJSONObject: value),
              let text = String(data: data, encoding: .utf8)
        else { return }
        defaults.set(text, forKey: key)
    }
}
