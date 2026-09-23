//
//  SettingsWire.swift
//  VelaWallet
//
//  The `network_admin` machine's view model, in Swift — and, since spec 071,
//  `sign_pref`'s.
//
//  Same rule as `CoreViews.swift` and `ContactsWire.swift`: views are
//  `Decodable` through `CoreJSON.decoder` (`.convertFromSnakeCase`); operations
//  and results stay dictionaries.
//
//  ## Two tagged unions, and why they need hand-written decoders
//
//  `NetProbeHealth` and `NetServiceHealth` are `#[serde(tag = "type")]` enums
//  with **payloads on some cases and not others** — `ok { latency_ms }` beside a
//  bare `checking`. Swift's synthesised `Decodable` cannot express that, so the
//  two below read the discriminator and then their own fields. Everything else
//  here is a plain struct or a bare string enum and is synthesised.
//
//  A missing case throws rather than defaulting. `NetServiceHealth.checking` is
//  a real state and so is `.unreachable`; silently collapsing an unrecognised
//  one into either would tell somebody their endpoint is fine, or broken, on no
//  evidence.
//

import Foundation
import VelaCore

// MARK: - Health

/// How an RPC or explorer endpoint answered.
enum NetProbeHealthWire: Decodable, Equatable {
    case checking
    case ok(latencyMs: Double)
    case error

    private enum Keys: String, CodingKey { case type, latencyMs }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "checking": self = .checking
        case "ok": self = .ok(latencyMs: try container.decode(Double.self, forKey: .latencyMs))
        case "error": self = .error
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown NetProbeHealth `\(other)`"
            )
        }
    }
}

/// How one of the four service endpoints answered.
enum NetServiceHealthWire: Decodable, Equatable {
    case checking
    case ok(latencyMs: Double, rateCount: Int?)
    /// Refused before any request: the URL is not https.
    case notHttps
    case unreachable(httpStatus: Int?, latencyMs: Double?)
    /// Answered, but not with anything this service should return.
    case invalidResponse(latencyMs: Double)

    private enum Keys: String, CodingKey { case type, latencyMs, rateCount, httpStatus }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "checking":
            self = .checking
        case "ok":
            self = .ok(
                latencyMs: try container.decode(Double.self, forKey: .latencyMs),
                rateCount: try container.decodeIfPresent(Int.self, forKey: .rateCount)
            )
        case "not_https":
            self = .notHttps
        case "unreachable":
            self = .unreachable(
                httpStatus: try container.decodeIfPresent(Int.self, forKey: .httpStatus),
                latencyMs: try container.decodeIfPresent(Double.self, forKey: .latencyMs)
            )
        case "invalid_response":
            self = .invalidResponse(latencyMs: try container.decode(Double.self, forKey: .latencyMs))
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown NetServiceHealth `\(other)`"
            )
        }
    }
}

// MARK: - The networks list

/// The RPC reports a different chain than the one this network claims to be.
///
/// A refusal, not a warning: the core writes nothing while this is set. An RPC
/// pointed at the wrong chain would have the wallet reading one chain's
/// balances under another chain's name.
struct NetChainMismatchWire: Decodable, Equatable {
    let expectedChainId: Int
    let reportedChainId: Int
}

struct NetNetworkRowWire: Decodable, Equatable {
    let id: String
    let chainId: Int
    let displayName: String
    let nativeSymbol: String
    /// Added by this person, as opposed to shipped with the app.
    let isCustom: Bool
    let rpcUrl: String
    let explorerUrl: String
    let bundlerUrl: String
    let rpcHealth: NetProbeHealthWire?
    let explorerHealth: NetProbeHealthWire?
    let rpcChainMismatch: NetChainMismatchWire?
    /// An edited RPC the core is holding back until its probe answers.
    let rpcSaveDeferred: Bool
}

// MARK: - The add-network wizard

enum NetWizardPhaseWire: String, Decodable {
    case idle, searching, suggested, resolving, checking, checked, error
}

struct NetChainIndexEntryWire: Decodable, Equatable {
    let chainId: Int
    let name: String
    let shortName: String
    let nativeCurrencySymbol: String
    let hasLogo: Bool
}

struct NetChainInfoWire: Decodable, Equatable {
    let chainId: Int
    let name: String
    let shortName: String
    let nativeName: String
    let nativeSymbol: String
    let nativeDecimals: Int
    let rpcUrl: String
    let rpcUrls: [String]
    let explorerUrl: String
    let logoUrl: String
    let isTestnet: Bool
}

struct NetContractStatusWire: Decodable, Equatable {
    let name: String
    let address: String
    let deployed: Bool
}

enum NetRpcFailureKindWire: String, Decodable {
    case noHttpsCandidates = "no_https_candidates"
    case allProbesFailed = "all_probes_failed"
}

/// The core's verdict on whether a chain can carry this wallet.
///
/// `compatible` is the gate the 添加 button reads, and the core refuses a
/// candidate whose compatibility was never verified — which is why the probes
/// travel with the settings executor rather than waiting for 051's pool.
struct NetCompatibilityWire: Decodable, Equatable {
    let chainId: Int
    let compatible: Bool
    let contracts: [NetContractStatusWire]
    /// `nil` = the precompile could not be probed, which is not the same as
    /// absent.
    let p256Available: Bool?
    let bestRpcUrl: String?
    let bestRpcLatencyMs: Double?
    let rpcFailure: NetRpcFailureKindWire?
}

/// Why the wizard cannot proceed. Tagged, with a chain id on four of five.
enum NetWizardErrorWire: Decodable, Equatable {
    case alreadyAdded(chainId: Int)
    case notFound(chainId: Int)
    case noRpcEndpoint
    case notCompatible(chainId: Int)
    /// The probes failed, so nothing was learned about the chain (spec 038
    /// #E1). Not a verdict: worded "unable to verify", never "incompatible".
    /// Missing here, a view carrying it failed to decode and the whole
    /// settings screen stopped hearing the core.
    case checkFailed(chainId: Int)

    private enum Keys: String, CodingKey { case type, chainId }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: Keys.self)
        let chainId = { try? container.decode(Int.self, forKey: .chainId) }
        switch try container.decode(String.self, forKey: .type) {
        case "already_added": self = .alreadyAdded(chainId: chainId() ?? 0)
        case "not_found": self = .notFound(chainId: chainId() ?? 0)
        case "no_rpc_endpoint": self = .noRpcEndpoint
        case "not_compatible": self = .notCompatible(chainId: chainId() ?? 0)
        case "check_failed": self = .checkFailed(chainId: chainId() ?? 0)
        case let other:
            throw DecodingError.dataCorruptedError(
                forKey: .type, in: container,
                debugDescription: "unknown NetWizardErrorKind `\(other)`"
            )
        }
    }
}

struct NetWizardViewWire: Decodable, Equatable {
    let phase: NetWizardPhaseWire
    let query: String
    let customRpc: String
    let suggestions: [NetChainIndexEntryWire]
    let chainInfo: NetChainInfoWire?
    let compat: NetCompatibilityWire?
    let error: NetWizardErrorWire?
    /// **The add gate.** Never re-derived in Swift: the core owns what makes a
    /// candidate addable, and a second opinion here is how a screen offers to
    /// add a chain the core will refuse.
    let canAdd: Bool
}

// MARK: - Endpoints and providers

/// The four service endpoints, spelled as the core's `NetEndpointField` —
/// the same raw values go back out in `endpoint_edited` / `endpoint_blurred`.
enum NetEndpointFieldWire: String, Decodable, CaseIterable {
    case ethereumData = "ethereum_data"
    case passkeyIndex = "passkey_index"
    case bundlerService = "bundler_service"
    case fiatRates = "fiat_rates"
}

struct NetEndpointViewWire: Decodable, Equatable {
    let field: NetEndpointFieldWire
    let value: String
    let defaultValue: String
    let health: NetServiceHealthWire
}

/// `NetProviderId` — the view's spelling and the events' `provider`.
enum NetProviderIdWire: String, Decodable, CaseIterable {
    case alchemy, drpc, ankr
}

/// `NetOverrideField`: which of a network's two editable fields an
/// `override_field_edited` is about.
enum NetOverrideFieldWire: String, CaseIterable {
    case rpc, explorer
}

struct NetProviderNetRowWire: Decodable, Equatable {
    let chainId: Int
    let ok: Bool
    let latencyMs: Double
}

struct NetProviderTestViewWire: Decodable, Equatable {
    let done: Bool
    let results: [NetProviderNetRowWire]
    let okCount: Int
    let total: Int
}

struct NetProviderViewWire: Decodable, Equatable {
    let provider: NetProviderIdWire
    let key: String
    /// Distinct from `key.isEmpty`: a cleared key is REMOVED from storage, so
    /// this is the core's answer about whether the provider is configured.
    let hasKey: Bool
    let test: NetProviderTestViewWire?
}

// MARK: - display_currency

/// Which currency amounts are shown in, and whether one can be priced.
struct CurrencyViewWire: Decodable, Equatable {
    let code: String
    /// USD → `code`, or `nil` when nothing could price it right now.
    ///
    /// **`nil` is not `1`.** Formatting may degrade — show the USD figure
    /// rather than label an unconverted number with a ¥ — but converting may
    /// not: a fiat amount multiplied by a defaulted 1 is a real mispayment.
    let rate: Double?
    /// `false` ⇒ the USD placeholder is showing and the person has not chosen.
    let committed: Bool
}

// MARK: - The whole view

struct NetViewWire: Decodable, Equatable {
    /// The four stores have been read. Mutations before this are dropped by the
    /// core, so the screen must not offer them.
    let loaded: Bool
    let networks: [NetNetworkRowWire]
    let wizard: NetWizardViewWire
    let endpoints: [NetEndpointViewWire]
    let providers: [NetProviderViewWire]
    let lastAddedChainId: Int?
}

// MARK: - sign_pref (spec 071)

/// `sign_pref`'s view (spec 071): the "Sign with" every signing sheet starts
/// at, and the Trusted Signer page. Every judgement in it is the core's.
struct SignPrefViewWire: Decodable, Equatable {
    /// Always an offered name; `auto` when nothing was chosen.
    let method: String
    let methodCommitted: Bool
    /// Every "Sign with" value, in the order a picker lists them.
    let offered: [String]
    /// The page the Trusted Signer opens. Always usable.
    let signerUrl: String
    let signerUrlIsDefault: Bool
    /// `invalid` | `insecure` — the last address typed was refused and
    /// nothing was stored.
    let signerUrlError: String?
    /// Whether a page there can use this wallet's `getvela.app` passkeys.
    let signerUsesWalletPasskeys: Bool
    /// Spelled out because a hand-written `init(from:)` suppresses the
    /// synthesized set; the names are the decoder's post-`convertFromSnakeCase`
    /// ones.
    private enum CodingKeys: String, CodingKey {
        case method, methodCommitted, offered, signerUrl, signerUrlIsDefault
        case signerUrlError, signerUsesWalletPasskeys
    }

    /// `decodeIfPresent` throughout, with the core's own defaults behind it.
    ///
    /// This app is not the only thing that writes this view's JSON — the
    /// fixtures and the gallery do too. A mirror that hard-required a field
    /// the wire grew (or dropped) would refuse to decode the whole view and
    /// leave Settings with no signing section at all, which is the failure
    /// this file exists to stop rather than cause.
    init(from decoder: Decoder) throws {
        let values = try decoder.container(keyedBy: CodingKeys.self)
        method = try values.decodeIfPresent(String.self, forKey: .method) ?? "auto"
        methodCommitted = try values.decodeIfPresent(Bool.self, forKey: .methodCommitted) ?? false
        offered = try values.decodeIfPresent([String].self, forKey: .offered) ?? []
        signerUrl = try values.decodeIfPresent(String.self, forKey: .signerUrl) ?? trustedSignerDefaultUrl()
        signerUrlIsDefault = try values.decodeIfPresent(Bool.self, forKey: .signerUrlIsDefault) ?? true
        signerUrlError = try values.decodeIfPresent(String.self, forKey: .signerUrlError)
        signerUsesWalletPasskeys =
            try values.decodeIfPresent(Bool.self, forKey: .signerUsesWalletPasskeys) ?? true
    }

    /// What the machine says before it has read anything: `auto`, the
    /// official page, and everything it offers.
    static var initial: SignPrefViewWire? {
        (try? SignPrefCore().view()).flatMap {
            try? CoreJSON.decode(SignPrefViewWire.self, from: CoreJSON.object($0))
        }
    }
}
