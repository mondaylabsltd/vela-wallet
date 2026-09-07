//
//  FiatFx.swift
//  VelaWallet
//
//  USD → fiat, from the endpoint the person can change.
//
//  Ported from `app-web/vela-wallet/src/lib/services/fiat-fx.ts`. The default
//  is Vela's own Frankfurter instance (FOSS, no key, ~160 currencies including
//  VND); `设置 → 端点 → 汇率` replaces it, and `network_admin` owns that field.
//
//  The waterfall's SECOND rung: `FiatRates` (Chainlink) is asked first for the
//  sixteen currencies it can price, and this answers for everything else.
//
//  ## Two response shapes, because the provider is swappable
//
//  `normalize` accepts Frankfurter's array and the `{ rates: {...} }` object
//  both — which is what makes the endpoint genuinely configurable rather than
//  configurable-if-you-pick-the-same-vendor.
//
//  ## `vela.fxRates.v1` keeps the endpoint it came from
//
//  Web's field names and its six-hour TTL (FR-005). The URL rides in the cache
//  entry so changing the endpoint refetches instead of quietly serving the old
//  provider's numbers under the new one's name.
//

import Foundation

@MainActor
final class FiatFx {

    /// `network_admin`'s own default (`DEFAULT_FIAT_RATES_URL`), mirrored for
    /// the one case the shell needs before it has read the store.
    static let defaultURL = "https://vela-currency.getvela.app/v2/rates?base=USD"

    private static let cacheKey = VelaStore.Key.fxRates
    /// ECB/Frankfurter publish once per business day; six hours is plenty.
    private static let ttlMs: Double = 6 * 60 * 60 * 1000

    private let store: VelaStore
    private let accounts: AccountStore
    private var cached: (url: String, rates: [String: Double], atMs: Double)?

    init(store: VelaStore, accounts: AccountStore) {
        self.store = store
        self.accounts = accounts
    }

    /// USD → `code`, or `nil` when the endpoint cannot price it.
    func rate(_ code: String) async -> Double? {
        let upper = code.uppercased()
        if upper == "USD" { return 1 }
        let rates = await allRates()
        guard let rate = rates[upper], rate > 0 else { return nil }
        return rate
    }

    /// The whole USD-based map. Cached six hours in memory, persisted for a
    /// cold or offline start.
    func allRates() async -> [String: Double] {
        let url = await endpoint()
        let now = Date().timeIntervalSince1970 * 1000
        if let cached, cached.url == url, now - cached.atMs < Self.ttlMs { return cached.rates }

        let body = await CoreHTTP.getJSON(url, timeout: CoreHTTP.Timeout.fiatRates)
        guard let rates = Self.normalize(body) else {
            print("[vela-wallet] fx: no usable rates from \(url)")
            return persisted(url: url)
        }
        cached = (url, rates, now)
        store.writeObject(Self.cacheKey, ["url": url, "rates": rates, "at": now])
        return rates
    }

    /// Which currencies this endpoint can quote — the provider-driven list the
    /// picker is meant to offer rather than a constant.
    func supportedCodes() async -> [String] {
        Array(await allRates().keys)
    }

    private func endpoint() async -> String {
        let stored = await accounts.loadServiceEndpoints()["fiatRatesURL"] as? String
        guard let stored, !stored.isEmpty else { return Self.defaultURL }
        return stored
    }

    /// A provider response → `{ USD: 1, X: rate }`, or `nil` when it carried no
    /// rate at all.
    ///
    /// `nil` rather than `{ USD: 1 }`: a map with only the base in it can price
    /// nothing, and returning it would cache an answer that is indistinguishable
    /// from a working endpoint whose rates all happened to be missing.
    nonisolated static func normalize(_ body: Any?) -> [String: Double]? {
        var out: [String: Double] = ["USD": 1]

        if let rows = body as? [[String: Any]] {
            // Frankfurter v2: [{ base, quote, rate }, …]
            for row in rows {
                guard let code = (row["quote"] as? String)?.uppercased(), !code.isEmpty,
                      let rate = number(row["rate"]), rate > 0
                else { continue }
                out[code] = rate
            }
        } else if let object = body as? [String: Any],
                  let rates = object["rates"] as? [String: Any] {
            // open.er-api / v1: { rates: { EUR: 0.92, … } }
            for (code, value) in rates {
                guard let rate = number(value), rate > 0 else { continue }
                out[code.uppercased()] = rate
            }
        }

        return out.count > 1 ? out : nil
    }

    /// A JSON number or a numeric string — providers send both, and a rate that
    /// arrived as `"0.92"` is still a rate.
    nonisolated private static func number(_ value: Any?) -> Double? {
        if let number = value as? NSNumber { return number.doubleValue }
        if let text = value as? String { return Double(text) }
        return nil
    }

    /// The last map that reached disk, if it came from this same endpoint.
    private func persisted(url: String) -> [String: Double] {
        if let cached, cached.url == url { return cached.rates }
        let stored = store.readObject(Self.cacheKey)
        guard stored["url"] as? String == url else { return [:] }
        let rates = FiatRates.decodeRates(stored["rates"])
        guard !rates.isEmpty else { return [:] }
        cached = (url, rates, (stored["at"] as? NSNumber)?.doubleValue ?? 0)
        return rates
    }
}

/// One rung of the display currency's rate waterfall.
///
/// Two conform: `FiatRates` (Chainlink's on-chain fiat feeds) and `FiatFx` (the
/// configurable endpoint). A rung that cannot price a code answers `nil` and
/// the next is asked — which is why neither of them needs to know it is in a
/// waterfall at all.
@MainActor
protocol FiatRateSource {
    /// USD → `code`, or `nil` when this source cannot state it.
    func rate(_ code: String) async -> Double?
}

extension FiatFx: FiatRateSource {}
extension FiatRates: FiatRateSource {}
