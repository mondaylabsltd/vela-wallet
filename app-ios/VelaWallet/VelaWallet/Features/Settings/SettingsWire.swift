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
    /// Spec 081: only a wallet holding more than one passkey needs this one.
    let multiKeyOnly: Bool
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
    /// Spec 081: `compatible` answers for one key. This one answers for two to
    /// seven — a chain can be genuinely usable and still refuse such a wallet,
    /// because each extra key is a signer contract Safe's factory creates.
    let multiKeyReady: Bool
    let contracts: [NetContractStatusWire]
    /// `nil` = the precompile could not be probed, which is not the same as
    /// absent.
    let p256Available: Bool?
    let bestRpcUrl: String?
    let bestRpcLatencyMs: Double?
    let rpcFailure: NetRpcFailureKindWire?
    /// WHY the network was refused (PR 3): `no_p256` — the chain has no P-256
    /// verifier, which nobody can deploy — or `missing_contracts`. Set
    /// exactly when the check answered and `compatible` is false. A plain
    /// string on purpose: a reason a later core adds must not fail the decode.
    var blocker: String? = nil
    /// The corpus key of the line under the refusal, chosen by the core.
    var hintKey: String? = nil
    /// Where "Open Chain Setup Tool" goes, with `?chain=<id>` — only for
    /// missing contracts. `nil` is "no such button".
    var setupUrl: String? = nil
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

/// `NetRpcField` — whether the wizard's result draws the field where a
/// person names an RPC endpoint of their own.
enum NetRpcFieldWire: String, Decodable {
    /// No field, no re-check: searching and checking; already added or not
    /// found; and a REFUSAL another endpoint would not change.
    case none
    /// The check passed, or could not reach a verdict.
    case optional
    /// The network lists no endpoint: one typed here is the only way on.
    case required
}

struct NetWizardViewWire: Decodable, Equatable {
    let phase: NetWizardPhaseWire
    let query: String
    let customRpc: String
    let suggestions: [NetChainIndexEntryWire]
    let chainInfo: NetChainInfoWire?
    /// The check's result. Present in the `checked` phase — and, since PR 3
    /// (notes 5, 10), ALSO beside `phase: error` when the check itself raised
    /// the stop (the scan / auto-add path: `not_compatible`, `check_failed`),
    /// so a refusal there can say why and offer Chain Setup where it applies.
    let compat: NetCompatibilityWire?
    let error: NetWizardErrorWire?
    /// The corpus key of the SENTENCE for `error`, chosen by the core (PR 3
    /// notes 5/10/18): already added, not found, no RPC endpoint listed,
    /// unable to verify, and for a refusal the check's own reason. The shell
    /// draws `t(errorKey)` and maps no `error.type` to words — it used to,
    /// and "no RPC endpoint" borrowed "unable to verify". Absent with no
    /// error (and from a hand-built view): then there is no sentence.
    var errorKey: String? = nil
    /// The RPC field under the result, and "Re-check with this RPC" with it
    /// (final notes F4, F14, F22) — the core's ONE rule for every surface
    /// that draws this wizard: the field exactly when this is not `.none`,
    /// and the re-check exactly where the field is. Absent on the wire (and
    /// from a hand-built view) reads `.none`, the core's own default.
    var rpcField: NetRpcFieldWire = .none
    /// The corpus key of that field's label: "Custom RPC (optional)", or
    /// plain "RPC URL" where it is the one thing asked for. `nil` with no
    /// field.
    var rpcFieldLabelKey: String? = nil
    /// **The add gate.** Never re-derived in Swift: the core owns what makes a
    /// candidate addable, and a second opinion here is how a screen offers to
    /// add a chain the core will refuse.
    let canAdd: Bool
}

/// Decoded by hand, in an extension so the memberwise initialiser stays: a
/// view from before `rpc_field` — a stored fixture, an older core — still
/// decodes, and reads as "no field" rather than stopping the whole settings
/// screen hearing the core. A value this build has never heard of reads the
/// same way: no field, no re-check.
extension NetWizardViewWire {
    private enum CodingKeys: String, CodingKey {
        case phase, query, customRpc, suggestions, chainInfo, compat, error, errorKey
        case rpcField, rpcFieldLabelKey, canAdd
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        self.init(
            phase: try c.decode(NetWizardPhaseWire.self, forKey: .phase),
            query: try c.decode(String.self, forKey: .query),
            customRpc: try c.decode(String.self, forKey: .customRpc),
            suggestions: try c.decode([NetChainIndexEntryWire].self, forKey: .suggestions),
            chainInfo: try c.decodeIfPresent(NetChainInfoWire.self, forKey: .chainInfo),
            compat: try c.decodeIfPresent(NetCompatibilityWire.self, forKey: .compat),
            error: try c.decodeIfPresent(NetWizardErrorWire.self, forKey: .error),
            errorKey: try c.decodeIfPresent(String.self, forKey: .errorKey),
            rpcField: try c.decodeIfPresent(String.self, forKey: .rpcField)
                .flatMap(NetRpcFieldWire.init(rawValue:)) ?? .none,
            rpcFieldLabelKey: try c.decodeIfPresent(String.self, forKey: .rpcFieldLabelKey),
            canAdd: try c.decode(Bool.self, forKey: .canAdd)
        )
    }
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
    ///
    /// **While `false`, no money figure is drawn** (the core's rule, PR 3):
    /// the placeholder is not the person's currency — the home drew "USD
    /// $1,234" for a few seconds and then jumped to "¥8,876".
    let committed: Bool
    /// The person's own stored choice on its way: its rate is being fetched
    /// and nothing is committed yet. A surface that names its currency apart
    /// from the figure may name this one while the figure waits. `nil` once
    /// committed, before the preference is read, and on a first launch.
    var pending: String? = nil

    /// The view before the machine has answered at all: nothing committed,
    /// nothing known to be on its way. What a live screen reads for the
    /// frames before the first view — waiting, never the dollar placeholder.
    static let unread = CurrencyViewWire(code: "USD", rate: nil, committed: false)
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
    /// Spec 100: the add-network sheet a page opened.
    var dappAdd: NetDappAddViewWire? = nil
}

// MARK: - A page asks to add a network (spec 100)

/// Where a page's add-network request stands (`NetDappAddPhase`).
enum NetDappAddPhaseWire: String, Decodable {
    case checking, ready
    case notCompatible = "not_compatible"
    case checkFailed = "check_failed"
    case wrongRpc = "wrong_rpc"
    case noRpc = "no_rpc"
}

/// The add-network sheet — who asks, for what, and the check. Every judgement
/// is the core's (`NetDappAddView`).
struct NetDappAddViewWire: Decodable, Equatable {
    let tab: String
    let id: String
    let origin: String
    let host: String
    let chainId: Int
    /// The catalog's name; the page's when `fromSite`; empty until the catalog answered.
    let name: String
    let nativeSymbol: String
    let rpcHost: String?
    let explorerHost: String?
    let fromSite: Bool
    let phase: NetDappAddPhaseWire
    let reportedChainId: Int?
    let compat: NetCompatibilityWire?
    let canAdd: Bool
}

// MARK: - fee_tier_pref (spec 069)

/// `fee_tier_pref`'s view (spec 069): the tier every send STARTS at — always a
/// real one, the factory `standard` when nothing was chosen.
struct FeeTierPrefViewWire: Decodable, Equatable {
    let tier: String
    /// `false` ⇒ the factory default is showing, not a choice.
    let committed: Bool
    /// The tiers Settings may offer, fastest first — never the dead `rapid`.
    let offered: [String]
}

