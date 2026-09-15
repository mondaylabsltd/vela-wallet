//
//  FiatRates.swift
//  VelaWallet
//
//  USD → fiat, from Chainlink's FX feeds on Ethereum mainnet.
//
//  Ported from `app-web/vela-wallet/src/lib/services/fiat-rates.ts`, itself the
//  Expo client's. The feeds are ENS-addressed (`<ccy>-usd.data.eth`) and read
//  with `latestRoundData()`:
//
//      feed answer      = USD value of one unit of the fiat currency
//      USD → fiat rate  = 1 / answer
//
//  This is the FIRST rung of `display_currency`'s waterfall. The second is the
//  configurable endpoint (`FiatFx`), and there is no third: a code neither can
//  price answers `nil`, and `nil` is not `1` (FR-009).
//
//  ## Feed decimals are read, never assumed
//
//  Most are 8 and PHP is 18. Assuming 8 would report a Philippine peso rate
//  10^10 out — which formats as a plausible-looking number rather than an
//  obvious error, and that is precisely the kind of wrong this client cannot
//  afford on a money figure.
//
//  ## The storage keys are the cross-client bytes
//
//  `vela.fiatRates.v1` and `vela.fiatFeedAddrs.v1`, with web's field names and
//  TTLs (FR-005): the persisted map is what gives a cold, offline start a rate
//  instead of an unconverted figure.
//

import Foundation
import VelaCore

@MainActor
final class FiatRates {

    /// Every fiat currency with a live `<ccy>-usd.data.eth` feed on mainnet.
    static let feedCodes = [
        "EUR", "GBP", "JPY", "CNY", "AUD", "CAD", "CHF", "KRW",
        "BRL", "MXN", "PHP", "SGD", "NZD", "TRY", "IDR", "ARS",
    ]

    /// Known-good feed addresses — the last resort when ENS cannot be reached.
    ///
    /// Not a preference: the proxies are immutable but the registry is the
    /// authority, so ENS is asked first and this only answers when it cannot
    /// be. A hard-coded map that silently outranked ENS would be a wallet
    /// pricing money from a list nobody can update.
    private static let fallbackAddresses: [String: String] = [
        "EUR": "0xb49f677943BC038e9857d61E7d053CaA2C1734C1",
        "GBP": "0x5c0Ab2d9b5a7ed9f470386e82BB36A3613cDd4b5",
        "JPY": "0xBcE206caE7f0ec07b545EddE332A47C2F75bbeb3",
        "CNY": "0xeF8A4aF35cd47424672E3C590aBD37FBB7A7759a",
        "AUD": "0x77F9710E7d0A19669A13c055F62cd80d313dF022",
        "CAD": "0xa34317DB73e77d453b1B8d04550c44D10e981C8e",
        "CHF": "0x449d117117838fFA61263B61dA6301AA2a88B13A",
        "KRW": "0x01435677FB11763550905594A16B645847C1d0F3",
        "BRL": "0x3126E7F38D5f60f4E2B6ec3511C7bdbD79317Df1",
        "MXN": "0xdb4881Ab0ad6b8423f76dd8C9d65542749a1dB77",
        "PHP": "0x3C7dB4D25deAb7c89660512C5494Dc9A3FC40f78",
        "SGD": "0xe25277fF4bbF9081C75Ab0EB13B4A13a721f3E13",
        "NZD": "0x3977CFc9e4f29C184D4675f4EB8e0013236e5f3e",
        "TRY": "0xB09fC5fD3f11Cf9eb5E1C5Dba43114e3C9f477b5",
        "IDR": "0x91b99C9b75aF469a71eE1AB528e8da994A5D7030",
        "ARS": "0xE41cD2DcC63EB63A9D9e62f2a3D9b49e6d0C0A1d",
    ]

    private static let ensRegistry = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e"

    private static let addressKey = VelaStore.Key.fiatFeedAddrs
    private static let rateKey = VelaStore.Key.fiatRates
    /// The proxies are immutable, so a month between resolutions is generous.
    private static let addressTTLMs: Double = 30 * 24 * 60 * 60 * 1000
    /// FX moves slowly and the feeds update on deviation; five minutes matches
    /// web.
    private static let rateTTLMs: Double = 5 * 60 * 1000

    private let store: VelaStore
    private let pool: RpcPool
    private var addresses: [String: String]?
    private var cached: (rates: [String: Double], atMs: Double)?

    init(store: VelaStore, pool: RpcPool) {
        self.store = store
        self.pool = pool
    }

    /// Whether `code` has a Chainlink fiat/USD feed at all.
    nonisolated static func isChainlinkFiat(_ code: String) -> Bool {
        feedCodes.contains(code.uppercased())
    }

    /// The ENS name for a currency's feed, e.g. `gbp-usd.data.eth`.
    nonisolated static func feedName(_ code: String) -> String {
        "\(code.lowercased())-usd.data.eth"
    }

    /// USD → `code`, or `nil` when this rung cannot price it.
    func rate(_ code: String) async -> Double? {
        guard Self.isChainlinkFiat(code) else { return nil }
        let rates = await allRates()
        guard let rate = rates[code.uppercased()], rate > 0 else { return nil }
        return rate
    }

    /// Every supported currency's rate, in one batch. Cached five minutes in
    /// memory, and persisted so a cold start has something before the network
    /// answers.
    func allRates() async -> [String: Double] {
        let now = Date().timeIntervalSince1970 * 1000
        if let cached, now - cached.atMs < Self.rateTTLMs { return cached.rates }

        let addresses = await feedAddresses()
        let codes = addresses.keys.sorted()
        guard !codes.isEmpty,
              let latestRound = Multicall.selector("latestRoundData()"),
              let decimalsCall = Multicall.selector("decimals()")
        else { return persistedRates() }

        // Two calls per feed — the round and the scale — so a feed whose
        // `decimals()` read fails still prices at the documented default
        // instead of dropping out.
        var calls: [Multicall3Call] = []
        for code in codes {
            guard let address = addresses[code] else { continue }
            calls.append(Multicall.call(address, latestRound))
            calls.append(Multicall.call(address, decimalsCall))
        }

        guard case .ok(let results) = await Multicall.aggregate3(
            chainId: 1, calls: calls, pool: pool
        ), results.count == calls.count else {
            return persistedRates()
        }

        var rates: [String: Double] = [:]
        for (index, code) in codes.enumerated() {
            let round = results[index * 2]
            let scale = results[index * 2 + 1]
            guard round.success else { continue }
            let decimals = scale.success ? Prices.feedDecimals(scale.returnData) : 8
            guard let usdPerUnit = Prices.chainlinkAnswer(round.returnData, decimals: decimals),
                  usdPerUnit > 0
            else { continue }
            // The feed states <CCY>/USD; the wallet converts the other way.
            rates[code] = 1 / usdPerUnit
        }

        guard !rates.isEmpty else { return persistedRates() }
        cached = (rates, now)
        store.writeObject(Self.rateKey, ["rates": rates, "at": now])
        return rates
    }

    // MARK: - Where the feeds live

    /// Resolve every feed's ENS name to an address: `registry.resolver(node)`,
    /// then `resolver.addr(node)`, two batches on mainnet. Cached thirty days.
    private func feedAddresses() async -> [String: String] {
        if let addresses { return addresses }

        let now = Date().timeIntervalSince1970 * 1000
        let stored = store.readObject(Self.addressKey)
        if let map = stored["addrs"] as? [String: String], !map.isEmpty,
           let at = (stored["at"] as? NSNumber)?.doubleValue, now - at < Self.addressTTLMs {
            addresses = map
            return map
        }

        if let resolved = await resolveFeedAddresses(), !resolved.isEmpty {
            addresses = resolved
            store.writeObject(Self.addressKey, ["addrs": resolved, "at": now])
            return resolved
        }

        addresses = Self.fallbackAddresses
        return Self.fallbackAddresses
    }

    private func resolveFeedAddresses() async -> [String: String]? {
        guard let resolverSelector = Multicall.selector("resolver(bytes32)"),
              let addrSelector = Multicall.selector("addr(bytes32)")
        else { return nil }

        let codes = Self.feedCodes
        let nodes = codes.map { Self.namehash(Self.feedName($0)) }

        let resolverCalls = nodes.map {
            Multicall.call(Self.ensRegistry, resolverSelector + $0)
        }
        guard case .ok(let resolvers) = await Multicall.aggregate3(
            chainId: 1, calls: resolverCalls, pool: pool
        ), resolvers.count == codes.count else { return nil }

        var addrCalls: [Multicall3Call] = []
        var codeForCall: [String] = []
        for (index, result) in resolvers.enumerated() {
            guard result.success,
                  let resolver = Self.addressWord(result.returnData),
                  !Self.isZeroAddress(resolver)
            else { continue }
            addrCalls.append(Multicall.call(resolver, addrSelector + nodes[index]))
            codeForCall.append(codes[index])
        }
        guard !addrCalls.isEmpty,
              case .ok(let answers) = await Multicall.aggregate3(
                  chainId: 1, calls: addrCalls, pool: pool
              ), answers.count == addrCalls.count
        else { return nil }

        var out: [String: String] = [:]
        for (index, result) in answers.enumerated() {
            guard result.success,
                  let address = Self.addressWord(result.returnData),
                  !Self.isZeroAddress(address)
            else { continue }
            out[codeForCall[index]] = address
        }
        return out
    }

    // MARK: - Pure helpers

    /// EIP-137 namehash, over the bridge's keccak — not a second hash
    /// implementation (FR-006).
    nonisolated static func namehash(_ name: String) -> Data {
        var node = Data(repeating: 0, count: 32)
        guard !name.isEmpty else { return node }
        for label in name.split(separator: ".").reversed() {
            let labelHash = keccak256(data: Data(label.utf8))
            node = keccak256(data: node + labelHash)
        }
        return node
    }

    /// The low 20 bytes of a 32-byte word, as a `0x` address.
    nonisolated static func addressWord(_ data: Data) -> String? {
        guard data.count >= 32 else { return nil }
        return "0x" + data.subdata(in: 12..<32).hexString
    }

    /// A stored rate map, element by element.
    ///
    /// `as? [String: Double]` on a JSON object is all-or-nothing: one field
    /// somebody wrote as a string throws away every rate in the file. The last
    /// map that reached disk is what an offline start converts with, so it is
    /// worth reading generously.
    nonisolated static func decodeRates(_ stored: Any?) -> [String: Double] {
        guard let object = stored as? [String: Any] else { return [:] }
        var out: [String: Double] = [:]
        for (code, value) in object {
            guard let rate = (value as? NSNumber)?.doubleValue ?? (value as? String).flatMap(Double.init),
                  rate > 0, rate.isFinite
            else { continue }
            out[code.uppercased()] = rate
        }
        return out
    }

    nonisolated static func isZeroAddress(_ address: String) -> Bool {
        let digits = address.hasPrefix("0x") ? String(address.dropFirst(2)) : address
        return !digits.isEmpty && digits.allSatisfy { $0 == "0" }
    }

    /// The last map that reached disk. Returned whenever the network cannot
    /// answer — an old rate is a rate somebody's device once verified, and the
    /// alternative is refusing to state a currency the person chose.
    private func persistedRates() -> [String: Double] {
        if let cached { return cached.rates }
        let stored = store.readObject(Self.rateKey)
        let rates = Self.decodeRates(stored["rates"])
        guard !rates.isEmpty else { return [:] }
        cached = (rates, (stored["at"] as? NSNumber)?.doubleValue ?? 0)
        return rates
    }
}
