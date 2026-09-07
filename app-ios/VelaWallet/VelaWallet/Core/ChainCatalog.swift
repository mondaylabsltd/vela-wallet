//
//  ChainCatalog.swift
//  VelaWallet
//
//  The built-in chain table — shell config, and the core says so.
//
//  Ported from `app-web/vela-wallet/src/lib/services/chains.ts`, itself ported
//  from the Expo client's `models/chains.ts`. `rpc_pool.rs`'s doc is explicit
//  about where this belongs:
//
//      Collect the endpoint lists for one chain (user override, provider keys,
//      defaults, curated public, chain index — all shell config;
//      `collectRpcUrls`/`collectBundlerUrls` stay there).
//
//  So the core owns *which endpoint to try and when to ban it*, and this owns
//  *what endpoints exist at all*. The twelve chains here are the same twelve
//  `network_admin::BUILTIN_CHAINS` knows, and `ChainCatalogTests` pins that
//  they do not drift — because a chain the settings screen lists and the pool
//  cannot route for is a network that reads as present and returns nothing.
//
//  `apiNetworkId` is the balance API's name for the chain and rides on every
//  token it returns; it and `chainId` are an inverse pair, which is a property
//  worth a test rather than a comment.
//

import Foundation

struct ChainMeta {
    /// Stable string id used in code and storage (`ethereum`, `bnb`).
    let id: String
    let displayName: String
    let chainId: Int
    /// The balance API's identifier, carried on every token it returns.
    let apiNetworkId: String
    /// The native gas token as the wallet displays it (POL, xDAI, USD…).
    let nativeSymbol: String
    let isL2: Bool
    let rpcURL: String
    let explorerURL: String
    /// How gas is settled. `.tempo` marks a chain with **no native coin**,
    /// where gas is paid in a USD stablecoin — the distinction that made a
    /// desktop cut show 4e57 dollars of balance when it was ignored.
    let gasModel: GasModel

    enum GasModel { case native, tempo }
}

enum ChainCatalog {

    static let chains: [ChainMeta] = [
        ChainMeta(id: "ethereum", displayName: "Ethereum", chainId: 1,
                  apiNetworkId: "eth-mainnet", nativeSymbol: "ETH", isL2: false,
                  rpcURL: "https://ethereum-rpc.publicnode.com",
                  explorerURL: "https://etherscan.io", gasModel: .native),
        ChainMeta(id: "bnb", displayName: "BNB Chain", chainId: 56,
                  apiNetworkId: "bnb-mainnet", nativeSymbol: "BNB", isL2: false,
                  rpcURL: "https://bsc-dataseed.binance.org",
                  explorerURL: "https://bscscan.com", gasModel: .native),
        ChainMeta(id: "polygon", displayName: "Polygon", chainId: 137,
                  apiNetworkId: "matic-mainnet", nativeSymbol: "POL", isL2: true,
                  rpcURL: "https://polygon-bor-rpc.publicnode.com",
                  explorerURL: "https://polygonscan.com", gasModel: .native),
        ChainMeta(id: "arbitrum", displayName: "Arbitrum", chainId: 42_161,
                  apiNetworkId: "arb-mainnet", nativeSymbol: "ETH", isL2: true,
                  rpcURL: "https://arb1.arbitrum.io/rpc",
                  explorerURL: "https://arbiscan.io", gasModel: .native),
        ChainMeta(id: "optimism", displayName: "Optimism", chainId: 10,
                  apiNetworkId: "opt-mainnet", nativeSymbol: "ETH", isL2: true,
                  rpcURL: "https://mainnet.optimism.io",
                  explorerURL: "https://optimistic.etherscan.io", gasModel: .native),
        ChainMeta(id: "base", displayName: "Base", chainId: 8_453,
                  apiNetworkId: "base-mainnet", nativeSymbol: "ETH", isL2: true,
                  rpcURL: "https://mainnet.base.org",
                  explorerURL: "https://basescan.org", gasModel: .native),
        ChainMeta(id: "avalanche", displayName: "Avalanche", chainId: 43_114,
                  apiNetworkId: "avax-mainnet", nativeSymbol: "AVAX", isL2: false,
                  rpcURL: "https://api.avax.network/ext/bc/C/rpc",
                  explorerURL: "https://snowtrace.io", gasModel: .native),
        ChainMeta(id: "gnosis", displayName: "Gnosis", chainId: 100,
                  apiNetworkId: "gnosis-mainnet", nativeSymbol: "xDAI", isL2: false,
                  rpcURL: "https://rpc.gnosischain.com",
                  explorerURL: "https://gnosisscan.io", gasModel: .native),
        ChainMeta(id: "unichain", displayName: "Unichain", chainId: 130,
                  apiNetworkId: "unichain-mainnet", nativeSymbol: "ETH", isL2: true,
                  rpcURL: "https://mainnet.unichain.org",
                  explorerURL: "https://uniscan.xyz", gasModel: .native),
        ChainMeta(id: "tempo", displayName: "Tempo", chainId: 4_217,
                  apiNetworkId: "tempo-mainnet", nativeSymbol: "USD", isL2: false,
                  rpcURL: "https://rpc.mainnet.tempo.xyz",
                  explorerURL: "https://explore.tempo.xyz", gasModel: .tempo),
        ChainMeta(id: "monad", displayName: "Monad", chainId: 143,
                  apiNetworkId: "monad-mainnet", nativeSymbol: "MON", isL2: false,
                  rpcURL: "https://rpc.monad.xyz",
                  explorerURL: "https://monadscan.com", gasModel: .native),
        ChainMeta(id: "worldchain", displayName: "World Chain", chainId: 480,
                  apiNetworkId: "worldchain-mainnet", nativeSymbol: "ETH", isL2: true,
                  rpcURL: "https://worldchain.drpc.org",
                  explorerURL: "https://worldscan.org", gasModel: .native),
    ]

    static func meta(_ chainId: Int) -> ChainMeta? {
        chains.first { $0.chainId == chainId }
    }

    static func meta(apiNetworkId: String) -> ChainMeta? {
        chains.first { $0.apiNetworkId == apiNetworkId }
    }

    /// Curated public endpoints, CORS-friendly, the Expo table verbatim.
    ///
    /// A **fallback tier**, not a preference: the core scores them below a
    /// user's own endpoint and below a paid provider, and only measured latency
    /// moves anything after that.
    static let publicRPCs: [Int: [String]] = [
        1: ["https://ethereum-rpc.publicnode.com", "https://1rpc.io/eth"],
        56: ["https://bsc-rpc.publicnode.com", "https://bsc.drpc.org",
             "https://bsc.meowrpc.com"],
        137: ["https://polygon-bor-rpc.publicnode.com", "https://1rpc.io/matic"],
        42_161: ["https://arbitrum-one-rpc.publicnode.com", "https://1rpc.io/arb"],
        10: ["https://optimism-rpc.publicnode.com", "https://1rpc.io/op"],
        8_453: ["https://base-rpc.publicnode.com", "https://1rpc.io/base"],
        43_114: ["https://avalanche-c-chain-rpc.publicnode.com", "https://1rpc.io/avax/c"],
        100: ["https://gnosis-rpc.publicnode.com", "https://1rpc.io/gnosis"],
        196: ["https://rpc.xlayer.tech", "https://xlayer.drpc.org"],
    ]
}

// MARK: - Providers

/// The three RPC providers a person can bring a key for.
///
/// Order is the **cold-start tiebreak only** — the core replaces it with
/// measured latency as soon as it has any, so this is not a ranking of quality.
enum RpcProvider: String, CaseIterable {
    case alchemy, drpc, ankr

    /// `chainId` → the provider's own name for that chain. A chain missing from
    /// a provider's map means no URL is built for it there — Ankr genuinely
    /// does not serve Unichain, World Chain, Monad or Tempo, and inventing a
    /// slug would produce an endpoint that 404s on every call.
    private static let slugs: [RpcProvider: [Int: String]] = [
        .alchemy: [
            1: "eth-mainnet", 56: "bnb-mainnet", 196: "xlayer-mainnet",
            137: "polygon-mainnet", 42_161: "arb-mainnet", 10: "opt-mainnet",
            8_453: "base-mainnet", 43_114: "avax-mainnet", 100: "gnosis-mainnet",
            130: "unichain-mainnet", 4_217: "tempo-mainnet", 143: "monad-mainnet",
            480: "worldchain-mainnet",
        ],
        .drpc: [
            1: "ethereum", 56: "bsc", 137: "polygon", 42_161: "arbitrum",
            10: "optimism", 8_453: "base", 43_114: "avalanche", 100: "gnosis",
            130: "unichain", 4_217: "tempo", 143: "monad", 480: "worldchain",
        ],
        .ankr: [
            1: "eth", 56: "bsc", 137: "polygon", 42_161: "arbitrum",
            10: "optimism", 8_453: "base", 43_114: "avalanche", 100: "gnosis",
        ],
    ]

    /// The endpoint for this provider on this chain, or `nil` when either the
    /// key or the slug is missing.
    func url(chainId: Int, key: String) -> String? {
        guard !key.isEmpty, let slug = Self.slugs[self]?[chainId] else { return nil }
        switch self {
        case .alchemy: return "https://\(slug).g.alchemy.com/v2/\(key)"
        case .drpc: return "https://lb.drpc.org/ogrpc?network=\(slug)&dkey=\(key)"
        case .ankr: return "https://rpc.ankr.com/\(slug)/\(key)"
        }
    }
}
