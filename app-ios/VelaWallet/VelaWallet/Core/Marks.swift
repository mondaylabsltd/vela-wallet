//
//  Marks.swift
//  VelaWallet
//
//  Which picture a token or a network wears — the port of Android's
//  `core/marks/Marks.kt`, itself the web's `services/tokens-model.ts` +
//  `flows/marks.ts` (the founder's rulings of 2026-09-05 and 2026-09-12).
//
//  iOS drew a three-letter glyph everywhere. Android and the web have shown
//  real logos since 047, which is why the same wallet looks like two products
//  side by side — the founder asked for this on 2026-09-15.
//
//  Four rules, and each one is a decision rather than a detail:
//
//  1. **Logos come from the chain-data endpoint, with the drawn glyph as the
//     fallback.** Not a blank circle: the fallback is a whole mark, so a phone
//     with no network still names every asset.
//  2. **A native coin's logo is the COIN's chain** — ETH on Base is still
//     Ethereum's logo, because that is what the coin IS.
//  3. **The chain badge is hidden when it would repeat the token**: ETH on
//     Ethereum, XDAI on Gnosis. Two identical circles say nothing twice.
//  4. **A token's logo is `assets/eip155-<chain>/<checksummed>/logo.png`**,
//     with the lowercase path as a second candidate, because the index has
//     both spellings and neither is guaranteed.
//

import Foundation
import VelaCore

enum Marks {

    /// The chain-data endpoint's base. Empty = glyphs only, which is exactly
    /// what this client drew before 058.
    nonisolated(unsafe) static var base: String = NetDefaults.ethereumDataURL

    /// Adopt the person's own endpoint, if they set one.
    static func adopt(_ endpoints: [String: Any]) {
        let configured = (endpoints["ethereumDataURL"] as? String)
            .flatMap { $0.isEmpty ? nil : $0 }
        base = configured ?? NetDefaults.ethereumDataURL
    }

    static func chainLogoURL(_ chainId: Int) -> String? {
        let root = base.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !root.isEmpty else { return nil }
        return "\(root)/chainlogos/eip155-\(chainId).png"
    }

    /// Rule 2 — which chain's logo a native coin wears.
    static func nativeCoinChainId(symbol: String, fallback: Int) -> Int {
        switch symbol.uppercased() {
        case "ETH": return 1
        case "BNB": return 56
        case "POL", "MATIC": return 137
        case "AVAX": return 43114
        case "XDAI": return 100
        default: return fallback
        }
    }

    /// Rule 3 — the badge's chain, or `nil` when the badge would repeat the
    /// token's own logo.
    static func badgeChainId(chainId: Int, symbol: String, tokenAddress: String?) -> Int? {
        if tokenAddress == nil, nativeCoinChainId(symbol: symbol, fallback: chainId) == chainId {
            return nil
        }
        return chainId
    }

    /// Rule 4 — where a token's logo lives, best candidate first.
    static func tokenLogoURLs(chainId: Int, symbol: String, tokenAddress: String?) -> [String] {
        guard let tokenAddress, !tokenAddress.isEmpty else {
            return [chainLogoURL(nativeCoinChainId(symbol: symbol, fallback: chainId))].compactMap { $0 }
        }
        let root = base.trimmingCharacters(in: CharacterSet(charactersIn: "/"))
        guard !root.isEmpty, isAddress(tokenAddress) else { return [] }
        let lower = tokenAddress.lowercased()
        let checksummed = (try? checksumAddress(addressHex: lower)) ?? lower
        var urls = ["\(root)/assets/eip155-\(chainId)/\(checksummed)/logo.png"]
        if lower != checksummed {
            urls.append("\(root)/assets/eip155-\(chainId)/\(lower)/logo.png")
        }
        return urls
    }

    /// Everything a token's mark needs. `named` are the core's own
    /// `logo_urls`, which win: an index that states a URL knows better than a
    /// path this file guessed.
    static func token(
        chainId: Int,
        symbol: String,
        tokenAddress: String?,
        named: [String] = []
    ) -> (logoURLs: [String], badgeLogoURL: String?, badgeHidden: Bool) {
        let badge = badgeChainId(chainId: chainId, symbol: symbol, tokenAddress: tokenAddress)
        let stated = named.filter { !$0.isEmpty }
        let known = tokenLogoURLs(chainId: chainId, symbol: symbol, tokenAddress: tokenAddress)
            .filter { !stated.contains($0) }
        return (stated + known, badge.flatMap(chainLogoURL), badge == nil)
    }

    private static func isAddress(_ value: String) -> Bool {
        value.count == 42 && value.hasPrefix("0x")
            && value.dropFirst(2).allSatisfy(\.isHexDigit)
    }
}
