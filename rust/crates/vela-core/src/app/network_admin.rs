//! Machine — network & endpoint administration (spec `017`, inventory
//! `### network_admin (P2)`).
//!
//! ```text
//! SearchInput ─300ms─► index fetch ─► ranked suggestions
//! ChainSelected ─► dedup gate ─► Resolving ─► Probing (eth_chainId race) ─►
//!   CheckingContracts (getCode ×11 ∥ P256 call) ─► Checked ─► AddConfirmed ─► saved
//! OverrideBlurred ─► upsert config (bundler preserved) ─► pool + bundler-cache flush
//! EndpointBlurred ─► trim/CRLF-strip ─► persist ─► pool flush ─► re-probe all 4
//! ProviderKeyBlurred ─► persist cleaned keys ─► pool flush ─► per-chain probe
//! ```
//!
//! Faithful port of the TypeScript sources, line-aligned:
//!
//! - `src/screens/settings/SettingsScreen.tsx:93-516` — endpoint/override
//!   health checks, `SERVICE_IDENTITY`, the URL trim + CR/LF strip
//! - `src/screens/settings/SettingsScreen.tsx:570-853` — the add-network
//!   wizard (debounce, dedup, compatibility rendering, retry affordances)
//! - `src/services/add-network.ts` — the EIP-681 scan recovery path
//! - `src/services/network-checker.ts` — `REQUIRED_CONTRACTS`, the RPC race,
//!   the P256 two-strategy probe, `checkCode`
//! - `src/services/chain-registry.ts:84-176` — search ranking + RPC/explorer
//!   extraction (placeholder filtering)
//! - `src/models/network.ts:70-78` — `explorerBaseURL` (unknown chain ⇒ NO link)
//! - `src/screens/settings/RpcProvidersModal.tsx` + `src/services/rpc-providers.ts`
//!   — provider key management and the per-chain capability probe
//! - `src/services/storage.ts:218-340` — persistence shapes and the
//!   clear-key-removes-provider rule
//!
//! The four near-identical `eth_chainId` probes (`checkEndpointHealth`,
//! `testRpcLatency`, `probeRpcChainId`, the provider modal's probe) are ONE
//! operation here: [`NetOperation::ProbeRpc`]. Their timeout differences
//! (6s/10s) and the ws:// vs https:// transport split stay in the shell, as
//! the inventory prescribes.
//!
//! Ported quirks & recorded gaps (all verbatim, all annotated inline):
//!
//! - **Invariant ① unification**: today the modal dedups a chainId
//!   (SettingsScreen.tsx:620-626) but the scan path (`add-network.ts:42-53`)
//!   does not — the sources have diverged. Per the inventory, the core is the
//!   single implementation and BOTH entry points pass the same gate.
//! - **Invariant ④ — the save gate (decided, spec `017`)**: the TypeScript
//!   sources never called `probeRpcChainId` before an override save
//!   (SettingsScreen.tsx:178-328), so a wrong-chain URL entered the pool at the
//!   highest tier and silently poisoned every balance read. The gate now lives
//!   here, in [`Event::OverrideBlurred`], and it refuses only on PROOF: the
//!   endpoint answered `eth_chainId` with a *different* id. A probe that timed
//!   out, was refused, or returned nothing parseable is "unable to verify" and
//!   the save proceeds — the same discipline the compatibility checker already
//!   applies (an unreachable chain gets a Retry, never a condemnation;
//!   [`NetRpcFailureKind`], invariant ③). The gate costs no extra request: it
//!   consumes the `eth_chainId` answer the card's own health probe already
//!   asks for, so a blur that lands before the probe does merely *waits*
//!   ([`NetNetworkRow::rpc_save_deferred`]) instead of writing blind.
//! - Health badges never gate saves (current behavior, kept).
//! - "Reset to defaults" persists the defaults but does NOT flush the pools
//!   (SettingsScreen.tsx:509 has no `invalidateAllPools()`); a field blur does.
//! - The compatibility checker's internal second `fetchChainInfo` (only used
//!   to append `info.rpcUrl` to the candidates, network-checker.ts:52-57) is
//!   folded onto the already-resolved chain info — same data, one fetch.
//! - `addedAt` is stamped by the shell on the *triggering event* (`now_iso`),
//!   not at write time as TS does; the drift is a few seconds of metadata.
//!   The core has no clock (016 rule) and no save-side event exists to ride.
//! - The scan path flattens an inconclusive RPC failure into `not-compatible`
//!   (`add-network.ts:47` has no `rpcFailed` branch) — ported verbatim; only
//!   the wizard keeps the invariant-③ distinction via `rpc_failure`.
//! - The wizard's fastest-RPC pick counts any JSON-RPC answer as responsive
//!   without matching the reported chain id (`testRpcLatency` checks only
//!   `json.result`) — verbatim. Only the provider probe matches ids
//!   (invariant ⑦, rpc-providers modal:78). On the unified vocabulary an
//!   unparseable id reports as failed — fail-closed vs TS's "truthy garbage
//!   counts", the only tightening.
//! - Alchemy's slug map carries X Layer (196) which is not in the canonical
//!   `CHAINS` table, so `provider_chain_ids` never surfaces it — dead data,
//!   ported verbatim so the maps stay diffable against the source.
//! - Provider key storage is plaintext AsyncStorage (storage.ts:289-297 says
//!   so itself) — a recorded security debt, owned by the shell.

use std::collections::BTreeMap;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants — every value mirrors the TS source it is named after
// ---------------------------------------------------------------------------

/// `REQUIRED_CONTRACTS` (network-checker.ts:20-32). Order matches the
/// biubiu.tools Vela Wallet Chain Setup listing. A chain missing ANY of these
/// (or the P256 precompile) can accept deposits the wallet can never sign out
/// of — invariant ②.
/// Spec 081 (FR-009): twelve, and the third field says whether an entry is
/// needed only by a wallet with more than one key.
///
/// Two changes from the eleven this list carried since it was ported:
/// - **added** Safe's passkey signer factory and its singleton. Keys two to
///   seven are signer contracts the factory creates inside the Safe's setup
///   (`safe.rs`), so on a chain without them a multi-key wallet cannot be
///   deployed at all — and the old check called that chain compatible.
/// - **removed** the CompatibilityFallbackHandler. Vela wallets set the 4337
///   module as their fallback handler; nothing here ever used that address, so
///   requiring it turned away chains the wallet works on perfectly.
pub const REQUIRED_CONTRACTS: [(&str, &str, bool); 12] = [
    (
        "Deterministic Deployment Proxy",
        "0x4e59b44847b379578588920cA78FbF26c0B4956C",
        false,
    ),
    (
        "Safe Singleton Factory",
        "0x914d7Fec6aaC8cd542e72Bca78B30650d45643d7",
        false,
    ),
    (
        "Multicall3",
        "0xcA11bde05977b3631167028862bE2a173976CA11",
        false,
    ),
    (
        "EntryPoint v0.7",
        "0x0000000071727De22E5E9d8BAf0edAc6f37da032",
        false,
    ),
    ("Safe L2", "0x29fcB43b46531BcA003ddC8FCB67FFE91900C762", false),
    (
        "Safe Proxy Factory",
        "0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67",
        false,
    ),
    (
        "Safe 4337 Module",
        "0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226",
        false,
    ),
    (
        "Safe Module Setup",
        "0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47",
        false,
    ),
    (
        "WebAuthn Signer",
        "0x94a4F6affBd8975951142c3999aEAB7ecee555c2",
        false,
    ),
    (
        "MultiSend",
        "0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526",
        false,
    ),
    (
        "Safe Passkey Signer Factory",
        "0x1d31F259eE307358a26dFb23EB365939E8641195",
        true,
    ),
    (
        "Safe Passkey Signer Singleton",
        "0x4E27b51350e6c2083EE19011120F50DAfEc5CA50",
        true,
    ),
];

/// RIP-7212 P256 precompile address (network-checker.ts:180).
pub const P256_PRECOMPILE: &str = "0x0000000000000000000000000000000000000100";

/// sha256("test") signed with a known P-256 key — the `eth_call` payload of
/// probe strategy 1 (network-checker.ts:183-189). The shell sends it with
/// `gas: 0x100000` for zkSync compatibility; exported so it cannot drift.
pub const VALID_P256_CALL: &str = "0x\
9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\
7bf0e18d07660f15994adce5c3836d7bd6167cdb5726f631098f433ebe0be9c0\
3936edbe5c791477e714e58244afb690b9b88b833ff4acdf0fbd1b28bf0b1182\
3be8cbcb3f590087711ae5ed74b9cd06a88058d0bbe700b5f0ec5a1bfac15592\
f989ef9bfaae0fee03c36625e88eae99806a879d813411f876e7e03a2ffd8314";

/// The wizard's search debounce (SettingsScreen.tsx:606).
pub const SEARCH_DEBOUNCE_MS: u32 = 300;

// `DEFAULT_SERVICE_ENDPOINTS` (models/types.ts:317-324).
pub const DEFAULT_ETHEREUM_DATA_URL: &str = "https://ethereum-data.getvela.app";
pub const DEFAULT_PASSKEY_INDEX_URL: &str = "https://p256-index-v2.getvela.app";
pub const DEFAULT_BUNDLER_SERVICE_URL: &str = "https://vela-relay-cf.getvela.app";
pub const DEFAULT_FIAT_RATES_URL: &str = "https://vela-currency.getvela.app/v2/rates?base=USD";

/// `SERVICE_IDENTITY` (SettingsScreen.tsx:340-344) — the `/api/health`
/// `service` field each endpoint must report. A passkey index pointed at the
/// wrong service is a LOGIN SAFETY problem, not a latency problem
/// (invariant ⑥).
pub fn service_identity(field: NetEndpointField) -> Option<&'static str> {
    match field {
        NetEndpointField::EthereumData => Some("ethereum-data"),
        NetEndpointField::PasskeyIndex => Some("webauthn-p256-publickey-index"),
        NetEndpointField::BundlerService => Some("vela-relay"),
        // Third-party — validated by rate-map shape, not identity.
        NetEndpointField::FiatRates => None,
    }
}

/// Whether a health response's `service` field is one this endpoint accepts.
/// The passkey index accepts BOTH the legacy `…-index` and the v2 `…-registry`
/// identities during the migration to the possession-proven registry.
pub fn identity_accepted(field: NetEndpointField, service: Option<&str>) -> bool {
    match field {
        NetEndpointField::PasskeyIndex => matches!(
            service,
            Some("webauthn-p256-publickey-index") | Some("webauthn-p256-publickey-registry")
        ),
        _ => match service_identity(field) {
            Some(expected) => service == Some(expected),
            None => false,
        },
    }
}

/// One built-in network (models/chains.ts `CHAINS`, the fields this machine
/// needs). The full table is the canon: chainId dedup, override defaults and
/// provider probe targets all read from here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetBuiltinChain {
    pub id: &'static str,
    pub display_name: &'static str,
    pub chain_id: u32,
    pub native_symbol: &'static str,
    pub rpc_url: &'static str,
    pub explorer_url: &'static str,
    /// How long a submitted operation USUALLY takes to land, in seconds —
    /// block time × the relay's usual inclusion depth (spec 038 Part D, #D3).
    /// A person waiting on the receipt screen gets this as "usually about
    /// Ns", and "longer than usual" past twice it. Never a promise, never a
    /// failure: a slow chain is still a submitted payment (invariant ⑤).
    pub typical_inclusion_s: u16,
}

pub const BUILTIN_CHAINS: [NetBuiltinChain; 24] = [
    NetBuiltinChain {
        id: "ethereum",
        display_name: "Ethereum",
        chain_id: 1,
        native_symbol: "ETH",
        rpc_url: "https://ethereum-rpc.publicnode.com",
        explorer_url: "https://etherscan.io",
        typical_inclusion_s: 24,
    },
    NetBuiltinChain {
        id: "bnb",
        display_name: "BNB Chain",
        chain_id: 56,
        native_symbol: "BNB",
        rpc_url: "https://bsc-dataseed.binance.org",
        explorer_url: "https://bscscan.com",
        typical_inclusion_s: 9,
    },
    NetBuiltinChain {
        id: "polygon",
        display_name: "Polygon",
        chain_id: 137,
        native_symbol: "POL",
        rpc_url: "https://polygon-bor-rpc.publicnode.com",
        explorer_url: "https://polygonscan.com",
        typical_inclusion_s: 6,
    },
    NetBuiltinChain {
        id: "arbitrum",
        display_name: "Arbitrum",
        chain_id: 42161,
        native_symbol: "ETH",
        rpc_url: "https://arb1.arbitrum.io/rpc",
        explorer_url: "https://arbiscan.io",
        typical_inclusion_s: 4,
    },
    NetBuiltinChain {
        id: "optimism",
        display_name: "Optimism",
        chain_id: 10,
        native_symbol: "ETH",
        rpc_url: "https://mainnet.optimism.io",
        explorer_url: "https://optimistic.etherscan.io",
        typical_inclusion_s: 4,
    },
    NetBuiltinChain {
        id: "base",
        display_name: "Base",
        chain_id: 8453,
        native_symbol: "ETH",
        rpc_url: "https://mainnet.base.org",
        explorer_url: "https://basescan.org",
        typical_inclusion_s: 4,
    },
    NetBuiltinChain {
        id: "avalanche",
        display_name: "Avalanche",
        chain_id: 43114,
        native_symbol: "AVAX",
        rpc_url: "https://api.avax.network/ext/bc/C/rpc",
        explorer_url: "https://snowtrace.io",
        typical_inclusion_s: 4,
    },
    NetBuiltinChain {
        id: "gnosis",
        display_name: "Gnosis",
        chain_id: 100,
        native_symbol: "xDAI",
        rpc_url: "https://rpc.gnosischain.com",
        explorer_url: "https://gnosisscan.io",
        typical_inclusion_s: 15,
    },
    NetBuiltinChain {
        id: "unichain",
        display_name: "Unichain",
        chain_id: 130,
        native_symbol: "ETH",
        rpc_url: "https://mainnet.unichain.org",
        explorer_url: "https://uniscan.xyz",
        typical_inclusion_s: 3,
    },
    NetBuiltinChain {
        id: "tempo",
        display_name: "Tempo",
        chain_id: 4217,
        native_symbol: "USD",
        rpc_url: "https://rpc.mainnet.tempo.xyz",
        explorer_url: "https://explore.tempo.xyz",
        typical_inclusion_s: 5,
    },
    NetBuiltinChain {
        id: "monad",
        display_name: "Monad",
        chain_id: 143,
        native_symbol: "MON",
        rpc_url: "https://rpc.monad.xyz",
        explorer_url: "https://monadscan.com",
        typical_inclusion_s: 3,
    },
    NetBuiltinChain {
        id: "worldchain",
        display_name: "World Chain",
        chain_id: 480,
        native_symbol: "ETH",
        rpc_url: "https://worldchain.drpc.org",
        explorer_url: "https://worldscan.org",
        typical_inclusion_s: 4,
    },
    // Circle's USDC-native L1 (spec 060). The native coin IS USDC — 18 decimals
    // on-chain, with a 6-decimal ERC-20 view of the SAME balance at
    // 0x3600…0000 that must never be listed as a token. ~0.48 s blocks with
    // deterministic finality, so 2 s covers the relay's inclusion depth.
    NetBuiltinChain {
        id: "arc",
        display_name: "Arc",
        chain_id: 5042,
        native_symbol: "USDC",
        rpc_url: "https://rpc.mainnet.arc.io",
        explorer_url: "https://explorer.arc.io",
        typical_inclusion_s: 2,
    }, // The eleven admitted together (spec 061): every one probed live for all
    // eleven required contracts and the RIP-7212 precompile before it was listed.
    // Polygon CDK zk-rollup; OKB is the gas coin.
    NetBuiltinChain {
        id: "xlayer",
        display_name: "X Layer",
        chain_id: 196,
        native_symbol: "OKB",
        rpc_url: "https://rpc.xlayer.tech",
        explorer_url: "https://www.oklink.com/xlayer",
        typical_inclusion_s: 3,
    },
    // Tether's L1. The native coin IS USDT0 (18 dp) and 0x779Ded…3736 is a 6-dp ERC-20 view of the SAME balance — never a token; see `manage_tokens::native_alias_token`.
    NetBuiltinChain {
        id: "stable",
        display_name: "Stable",
        chain_id: 988,
        native_symbol: "USDT0",
        rpc_url: "https://rpc.stable.xyz",
        explorer_url: "https://stablescan.xyz",
        typical_inclusion_s: 2,
    },
    // OP-stack L2 (Sony).
    NetBuiltinChain {
        id: "soneium",
        display_name: "Soneium",
        chain_id: 1868,
        native_symbol: "ETH",
        rpc_url: "https://rpc.soneium.org",
        explorer_url: "https://soneium.blockscout.com",
        typical_inclusion_s: 6,
    },
    // Real-time L2; ~1 s blocks at the RPC boundary.
    NetBuiltinChain {
        id: "megaeth",
        display_name: "MegaETH",
        chain_id: 4326,
        native_symbol: "ETH",
        rpc_url: "https://megaeth.drpc.org",
        explorer_url: "https://megaeth.blockscout.com",
        typical_inclusion_s: 3,
    },
    // Arbitrum Orbit L2; ~0.1 s blocks.
    NetBuiltinChain {
        id: "robinhood",
        display_name: "Robinhood Chain",
        chain_id: 4663,
        native_symbol: "ETH",
        rpc_url: "https://rpc.mainnet.chain.robinhood.com",
        explorer_url: "https://robinhoodchain.blockscout.com",
        typical_inclusion_s: 2,
    },
    // OP-derived L2 with its own token-ratio gas metering; MNT is the gas coin.
    NetBuiltinChain {
        id: "mantle",
        display_name: "Mantle",
        chain_id: 5000,
        native_symbol: "MNT",
        rpc_url: "https://rpc.mantle.xyz",
        explorer_url: "https://mantlescan.xyz",
        typical_inclusion_s: 6,
    },
    // Kaia (ex-Klaytn) L1, 1 s blocks.
    NetBuiltinChain {
        id: "kaia",
        display_name: "Kaia",
        chain_id: 8217,
        native_symbol: "KAIA",
        rpc_url: "https://public-en.node.kaia.io",
        explorer_url: "https://kaiascope.com",
        typical_inclusion_s: 3,
    },
    // OP-stack L2 since 2025; CELO is the gas coin.
    NetBuiltinChain {
        id: "celo",
        display_name: "Celo",
        chain_id: 42220,
        native_symbol: "CELO",
        rpc_url: "https://forno.celo.org",
        explorer_url: "https://celoscan.io",
        typical_inclusion_s: 3,
    },
    // OP-stack L2 (Kraken).
    NetBuiltinChain {
        id: "ink",
        display_name: "Ink",
        chain_id: 57073,
        native_symbol: "ETH",
        rpc_url: "https://rpc-gel.inkonchain.com",
        explorer_url: "https://explorer.inkonchain.com",
        typical_inclusion_s: 3,
    },
    // Arbitrum Orbit L2; PLUME is the gas coin.
    NetBuiltinChain {
        id: "plume",
        display_name: "Plume",
        chain_id: 98866,
        native_symbol: "PLUME",
        rpc_url: "https://rpc.plume.org",
        explorer_url: "https://explorer.plume.org",
        typical_inclusion_s: 2,
    },
    // Cosmos-EVM sidechain, ~6 s blocks; XRP is the gas coin. No provider serves it.
    NetBuiltinChain {
        id: "xrplevm",
        display_name: "XRPL EVM",
        chain_id: 1440000,
        native_symbol: "XRP",
        rpc_url: "https://rpc.xrplevm.org",
        explorer_url: "https://explorer.xrplevm.org",
        typical_inclusion_s: 18,
    },
];

fn builtin(chain_id: u32) -> Option<&'static NetBuiltinChain> {
    BUILTIN_CHAINS.iter().find(|c| c.chain_id == chain_id)
}

/// Whether this is a network Vela ships, as opposed to one a person added.
///
/// The distinction decides WHO can fix an out-of-gas relayer. On a network we
/// ship, the operator runs that relayer and can refill it — the useful thing a
/// person can do is tell them. On a network someone added — a local devnet, a
/// company's internal chain, anything the operator cannot reach — there may be
/// no way for the operator to hold gas there at all, and funding it is the
/// person's own call. Offering the wrong one of those two is either a shrug or
/// a request for money that should never have been asked for.
pub fn is_builtin_chain(chain_id: u32) -> bool {
    builtin(chain_id).is_some()
}

/// `PROVIDER_ORDER` (rpc-providers.ts:37).
pub const PROVIDER_ORDER: [NetProviderId; 3] = [
    NetProviderId::Alchemy,
    NetProviderId::Drpc,
    NetProviderId::Ankr,
];

/// `PROVIDER_CHAIN_SLUGS` (rpc-providers.ts:74-118), verbatim — including
/// Alchemy's X Layer (196) entry that the canonical chain table filters out.
fn provider_slug(id: NetProviderId, chain_id: u32) -> Option<&'static str> {
    match id {
        NetProviderId::Alchemy => match chain_id {
            1 => Some("eth-mainnet"),
            56 => Some("bnb-mainnet"),
            196 => Some("xlayer-mainnet"),
            137 => Some("polygon-mainnet"),
            42161 => Some("arb-mainnet"),
            10 => Some("opt-mainnet"),
            8453 => Some("base-mainnet"),
            43114 => Some("avax-mainnet"),
            100 => Some("gnosis-mainnet"),
            130 => Some("unichain-mainnet"),
            4217 => Some("tempo-mainnet"),
            143 => Some("monad-mainnet"),
            480 => Some("worldchain-mainnet"),
            5042 => Some("arc-mainnet"),
            988 => Some("stable-mainnet"),
            1868 => Some("soneium-mainnet"),
            4326 => Some("megaeth-mainnet"),
            4663 => Some("robinhood-mainnet"),
            5000 => Some("mantle-mainnet"),
            8217 => Some("kaia-mainnet"),
            42220 => Some("celo-mainnet"),
            57073 => Some("ink-mainnet"),
            98866 => Some("plume-mainnet"),
            _ => None,
        },
        NetProviderId::Drpc => match chain_id {
            1 => Some("ethereum"),
            56 => Some("bsc"),
            137 => Some("polygon"),
            42161 => Some("arbitrum"),
            10 => Some("optimism"),
            8453 => Some("base"),
            43114 => Some("avalanche"),
            100 => Some("gnosis"),
            130 => Some("unichain"),
            4217 => Some("tempo"),
            143 => Some("monad"),
            480 => Some("worldchain"),
            5042 => Some("arc"),
            196 => Some("xlayer"),
            988 => Some("stable"),
            1868 => Some("soneium"),
            4326 => Some("megaeth"),
            4663 => Some("robinhood"),
            5000 => Some("mantle"),
            8217 => Some("kaia"),
            42220 => Some("celo"),
            57073 => Some("ink"),
            98866 => Some("plume"),
            _ => None,
        },
        // Ankr serves none of the chains added since Gnosis; a slug is listed only
        // once its endpoint has been seen to answer (spec 060 R5), so none are.
        NetProviderId::Ankr => match chain_id {
            1 => Some("eth"),
            56 => Some("bsc"),
            137 => Some("polygon"),
            42161 => Some("arbitrum"),
            10 => Some("optimism"),
            8453 => Some("base"),
            43114 => Some("avalanche"),
            100 => Some("gnosis"),
            _ => None,
        },
    }
}

/// `buildProviderRpcUrl` (rpc-providers.ts:121-132).
pub fn build_provider_rpc_url(id: NetProviderId, chain_id: u32, key: &str) -> Option<String> {
    let slug = provider_slug(id, chain_id)?;
    if key.is_empty() {
        return None;
    }
    Some(match id {
        NetProviderId::Alchemy => format!("https://{slug}.g.alchemy.com/v2/{key}"),
        NetProviderId::Drpc => format!("https://lb.drpc.org/ogrpc?network={slug}&dkey={key}"),
        NetProviderId::Ankr => format!("https://rpc.ankr.com/{slug}/{key}"),
    })
}

/// `providerChainIds` (rpc-providers.ts:135-138) — canonical CHAINS order,
/// filtered to chains the provider has a slug for.
pub fn provider_chain_ids(id: NetProviderId) -> Vec<u32> {
    BUILTIN_CHAINS
        .iter()
        .filter(|c| provider_slug(id, c.chain_id).is_some())
        .map(|c| c.chain_id)
        .collect()
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetProviderId {
    Alchemy,
    Drpc,
    Ankr,
}

/// A stored custom network — serialises 1:1 to the TS `CustomNetwork` under
/// `vela.customNetworks` (the shell maps field-name casing).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetCustomNetwork {
    /// `custom-{chainId}`.
    pub id: String,
    pub display_name: String,
    pub chain_id: u32,
    pub icon_label: String,
    pub icon_color: String,
    pub icon_bg: String,
    pub logo_url: String,
    pub is_l2: bool,
    pub rpc_url: String,
    pub explorer_url: String,
    pub bundler_url: String,
    pub native_symbol: String,
    /// ISO 8601 — stamped by the shell (`now_iso` on the triggering event).
    pub added_at_iso: String,
}

/// A per-network endpoint override — 1:1 `NetworkConfig` under
/// `vela.networkConfig`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetNetworkConfig {
    pub chain_id: u32,
    pub rpc_url: String,
    pub explorer_url: String,
    pub bundler_url: String,
}

/// The four service endpoints — 1:1 `ServiceEndpoints` under
/// `vela.serviceEndpoints`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetServiceEndpoints {
    pub ethereum_data_url: String,
    pub passkey_index_url: String,
    pub bundler_service_url: String,
    pub fiat_rates_url: String,
}

impl Default for NetServiceEndpoints {
    fn default() -> Self {
        Self {
            ethereum_data_url: DEFAULT_ETHEREUM_DATA_URL.to_owned(),
            passkey_index_url: DEFAULT_PASSKEY_INDEX_URL.to_owned(),
            bundler_service_url: DEFAULT_BUNDLER_SERVICE_URL.to_owned(),
            fiat_rates_url: DEFAULT_FIAT_RATES_URL.to_owned(),
        }
    }
}

impl NetServiceEndpoints {
    /// The URL a caller should actually use for `field`: the configured one,
    /// or the default when it is unset or blank — the `||` of
    /// `getEthereumDataURL()` and its three siblings (storage.ts:194-210),
    /// written once here so no shell has to remember it.
    ///
    /// Spec 081 FR-002: three shells had each re-implemented "empty means the
    /// default" beside a cached URL, and one of them cached it for the life of
    /// the process — so a person could change their public-key index in
    /// Settings and still be registered against ours.
    #[must_use]
    pub fn effective(&self, field: NetEndpointField) -> &str {
        let configured = self.get(field).trim();
        if configured.is_empty() {
            default_endpoint(field)
        } else {
            self.get(field)
        }
    }

    fn get(&self, field: NetEndpointField) -> &str {
        match field {
            NetEndpointField::EthereumData => &self.ethereum_data_url,
            NetEndpointField::PasskeyIndex => &self.passkey_index_url,
            NetEndpointField::BundlerService => &self.bundler_service_url,
            NetEndpointField::FiatRates => &self.fiat_rates_url,
        }
    }

    fn set(&mut self, field: NetEndpointField, value: String) {
        match field {
            NetEndpointField::EthereumData => self.ethereum_data_url = value,
            NetEndpointField::PasskeyIndex => self.passkey_index_url = value,
            NetEndpointField::BundlerService => self.bundler_service_url = value,
            NetEndpointField::FiatRates => self.fiat_rates_url = value,
        }
    }
}

/// The shipped default for one endpoint. Every "unset means this" answer in
/// the wallet comes from here.
#[must_use]
pub fn default_endpoint(field: NetEndpointField) -> &'static str {
    match field {
        NetEndpointField::EthereumData => DEFAULT_ETHEREUM_DATA_URL,
        NetEndpointField::PasskeyIndex => DEFAULT_PASSKEY_INDEX_URL,
        NetEndpointField::BundlerService => DEFAULT_BUNDLER_SERVICE_URL,
        NetEndpointField::FiatRates => DEFAULT_FIAT_RATES_URL,
    }
}

/// The stored (possibly partial) endpoints blob — a missing field falls back
/// to its default, exactly as `{...DEFAULT_SERVICE_ENDPOINTS, ...JSON.parse}`
/// merges (storage.ts:177).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetStoredEndpoints {
    pub ethereum_data_url: Option<String>,
    pub passkey_index_url: Option<String>,
    pub bundler_service_url: Option<String>,
    pub fiat_rates_url: Option<String>,
}

/// One API key per provider — 1:1 `RpcProviderKeys` under `vela.rpcProviders`.
/// `None` = provider not configured (a cleared key is REMOVED, invariant ⑦).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetProviderKeys {
    pub alchemy: Option<String>,
    pub drpc: Option<String>,
    pub ankr: Option<String>,
}

impl NetProviderKeys {
    fn get(&self, id: NetProviderId) -> Option<&String> {
        match id {
            NetProviderId::Alchemy => self.alchemy.as_ref(),
            NetProviderId::Drpc => self.drpc.as_ref(),
            NetProviderId::Ankr => self.ankr.as_ref(),
        }
    }

    fn set(&mut self, id: NetProviderId, value: Option<String>) {
        match id {
            NetProviderId::Alchemy => self.alchemy = value,
            NetProviderId::Drpc => self.drpc = value,
            NetProviderId::Ankr => self.ankr = value,
        }
    }
}

/// One row of the chain search index (`fuse-chains.json`, `ChainSearchResult`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetChainIndexEntry {
    pub chain_id: u32,
    pub name: String,
    pub short_name: String,
    pub native_currency_symbol: String,
    #[serde(default)]
    pub has_logo: bool,
}

/// The raw shape of `/chains/eip155-{id}.json`, as the shell hands it over.
/// All parsing decisions (defaults, HTTPS filtering, placeholder rejection)
/// happen in the core — see [`parse_chain_data`].
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetRawChainData {
    pub chain_id: Option<u32>,
    pub name: Option<String>,
    pub short_name: Option<String>,
    pub native_currency_name: Option<String>,
    pub native_currency_symbol: Option<String>,
    pub native_currency_decimals: Option<u32>,
    /// The full `rpc` array, unfiltered (ws://, http://, placeholder URLs and
    /// all) — filtering is a core rule (invariant ⑧).
    #[serde(default)]
    pub rpc: Vec<String>,
    /// Each explorer entry's `url` (`''` when the entry has none).
    #[serde(default)]
    pub explorers: Vec<String>,
    #[serde(default)]
    pub testnet: bool,
}

/// Parsed chain metadata — mirror of `ChainInfo` (chain-registry.ts:17-32).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetChainInfo {
    pub chain_id: u32,
    pub name: String,
    pub short_name: String,
    pub native_name: String,
    pub native_symbol: String,
    pub native_decimals: u32,
    /// The single "best" URL (`extractRpcUrl`) — may still carry a key
    /// placeholder when nothing clean exists, verbatim.
    pub rpc_url: String,
    /// All clean HTTPS URLs (`extractAllRpcUrls`) — placeholders NEVER appear
    /// here (invariant ⑧).
    pub rpc_urls: Vec<String>,
    pub explorer_url: String,
    pub logo_url: String,
    pub is_testnet: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetContractStatus {
    pub name: String,
    pub address: String,
    pub deployed: bool,
    /// Spec 081: only a wallet with more than one key needs this one.
    pub multi_key_only: bool,
}

/// Why the compatibility check could not reach a verdict. Distinct from
/// "incompatible" by design: an unreachable chain gets a Retry, never a
/// condemnation (invariant ③, SettingsScreen.tsx:813-827).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetRpcFailureKind {
    /// 'No valid HTTPS RPC endpoints available' (network-checker.ts:59-67).
    NoHttpsCandidates,
    /// 'All RPC endpoints failed or timed out' (network-checker.ts:73-81).
    AllProbesFailed,
}

/// Mirror of `CompatibilityResult` (models/types.ts:257-271), with the
/// English `error` strings replaced by data the shell words itself.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetCompatibility {
    pub chain_id: u32,
    /// A one-key wallet can be created and can sign here.
    pub compatible: bool,
    /// Spec 081: and a wallet with two to seven keys can too — it also needs
    /// Safe's passkey signer factory and its singleton. `compatible` without
    /// this is a real state: the chain works, but only for a single-key wallet.
    pub multi_key_ready: bool,
    pub contracts: Vec<NetContractStatus>,
    /// `None` = never probed (RPC failure short-circuited).
    pub p256_available: Option<bool>,
    pub best_rpc_url: Option<String>,
    pub best_rpc_latency_ms: Option<f64>,
    /// `Some` = inconclusive, show "unable to verify" + Retry — NEVER
    /// "not compatible" (invariant ③).
    pub rpc_failure: Option<NetRpcFailureKind>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetEndpointField {
    EthereumData,
    PasskeyIndex,
    BundlerService,
    FiatRates,
}

/// The four fields in UI order (SettingsScreen.tsx:462-467).
pub const ENDPOINT_FIELDS: [NetEndpointField; 4] = [
    NetEndpointField::EthereumData,
    NetEndpointField::PasskeyIndex,
    NetEndpointField::BundlerService,
    NetEndpointField::FiatRates,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetOverrideField {
    Rpc,
    Explorer,
}

/// A per-network override save the core REFUSED (invariant ④'s gate).
///
/// Only ever produced from PROOF: the endpoint answered `eth_chainId` with an
/// id, and it was not this network's. A silent endpoint produces `None`, not a
/// refusal — "unable to verify" is not "incompatible".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetChainMismatch {
    /// The network whose card was being edited.
    pub expected_chain_id: u32,
    /// What the endpoint said it serves.
    pub reported_chain_id: u32,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Storage keys, HTTP transports,
/// timeouts (6s/10s per legacy call site), the ws:// probe transport and the
/// `?_t=` cache-buster all live behind these sentences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetOperation {
    /// Read all four stores (`vela.customNetworks`, `vela.networkConfig`,
    /// `vela.serviceEndpoints`, `vela.rpcProviders`) at once. Unreadable or
    /// corrupt answers as empty/absent, as the TS loaders' `catch` does.
    ReadStore,
    /// Best-effort persists — the shell swallows storage errors; the
    /// in-memory ledger stays authoritative.
    WriteCustomNetworks {
        networks: Vec<NetCustomNetwork>,
    },
    WriteNetworkConfigs {
        configs: Vec<NetNetworkConfig>,
    },
    WriteServiceEndpoints {
        endpoints: NetServiceEndpoints,
    },
    WriteRpcProviders {
        keys: NetProviderKeys,
    },
    /// The 300ms search debounce (a plain timer).
    StartSearchDebounce {
        ms: u32,
    },
    /// `/index/fuse-chains.json` — the shell owns its 30-minute cache; a
    /// fetch failure answers the stale cache or an empty list, verbatim
    /// (chain-registry.ts:88-109).
    FetchSearchIndex,
    /// `/chains/eip155-{chain_id}.json` → raw data, or `None` on any failure.
    FetchChainInfo {
        chain_id: u32,
    },
    /// THE unified `eth_chainId` probe (inventory: one Probe word for the
    /// four legacy implementations). The shell answers the reported chain id
    /// (`None` = no/invalid/timeout answer) plus measured latency.
    ProbeRpc {
        url: String,
    },
    /// Explorer-style liveness: GET, `no-cors` — "resolved without throwing"
    /// is the only honest cross-origin signal (SettingsScreen.tsx:137-145).
    ProbeReachable {
        url: String,
    },
    /// `eth_getCode(address, 'latest')` → the raw code string, `None` on RPC
    /// error. The deployment verdict is the core's ([`is_code_deployed`]).
    RpcGetCode {
        url: String,
        address: String,
    },
    /// Probe strategy 1: `eth_call` of [`VALID_P256_CALL`] against
    /// [`P256_PRECOMPILE`] with `gas: 0x100000` → the raw result, `None` on
    /// RPC error. Acceptance is the core's ([`p256_call_indicates_support`]).
    RpcCallP256 {
        url: String,
    },
    /// GET `{base_url}/api/health` (+ shell-side `?_t=` cache-buster).
    FetchServiceHealth {
        field: NetEndpointField,
        base_url: String,
    },
    /// GET the fiat-rates URL itself; the shell reports the rate count
    /// (array length or `rates` key count — `normalizeRates` shapes).
    FetchFiatRates {
        url: String,
    },
    /// `refreshPool(chain_id)` / `invalidateAllPools()` (invariant ⑤).
    InvalidatePools {
        chain_id: Option<u32>,
    },
    /// `clearBundlerCache(chain_id)`.
    ClearBundlerCache {
        chain_id: u32,
    },
}

/// What one `/api/health` (or fiat) fetch came back as.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetHealthBody {
    /// The fetch threw, timed out, or the body was not JSON (TS lands all
    /// three in `catch` → "Connection failed").
    Failed,
    /// `!res.ok`.
    HttpError { status: u32 },
    /// Parsed `/api/health` body.
    Identity {
        service: Option<String>,
        status: Option<String>,
    },
    /// Parsed fiat body — how many currencies the map/array carried.
    Rates { rate_count: u32 },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetShellResult {
    StoreLoaded {
        custom_networks: Vec<NetCustomNetwork>,
        network_configs: Vec<NetNetworkConfig>,
        endpoints: NetStoredEndpoints,
        provider_keys: NetProviderKeys,
    },
    /// A best-effort write acknowledged. Never changes state.
    Written,
    DebounceElapsed,
    SearchIndex {
        chains: Vec<NetChainIndexEntry>,
    },
    ChainInfo {
        chain_id: u32,
        data: Option<NetRawChainData>,
    },
    Probed {
        url: String,
        /// `None` = failed/timed out/no parseable id.
        reported_chain_id: Option<u32>,
        latency_ms: f64,
    },
    Reachable {
        url: String,
        ok: bool,
        latency_ms: f64,
    },
    Code {
        url: String,
        address: String,
        code: Option<String>,
    },
    P256Call {
        url: String,
        result: Option<String>,
    },
    ServiceHealth {
        field: NetEndpointField,
        body: NetHealthBody,
        latency_ms: f64,
    },
    FiatRates {
        body: NetHealthBody,
        latency_ms: f64,
    },
    Invalidated,
    BundlerCacheCleared,
}

impl Operation for NetOperation {
    type Output = NetShellResult;
}

#[effect]
pub enum NetEffect {
    Render(RenderOperation),
    Shell(NetOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "NetEvent"))]
pub enum Event {
    /// App start — load the four stores. This machine lives for the app's
    /// lifetime; a second `Started` re-reads (the TS stores re-load lazily).
    Started,

    // -- add-network wizard -------------------------------------------------
    /// A keystroke in the wizard search box. Clears any selected chain state
    /// (`handleQueryChange`) and (re)starts the 300ms debounce.
    SearchInput {
        query: String,
    },
    /// A suggestion tap, or the wizard's "Recheck" buttons
    /// (`keep_custom_rpc: true` preserves the user-typed RPC, matching
    /// `handleSelect(chainId, keepCustomRpc)`).
    ChainSelected {
        chain_id: u32,
        keep_custom_rpc: bool,
    },
    CustomRpcEdited {
        value: String,
    },
    /// The "Add network" button. Only acts from `Checked` + compatible —
    /// `handleAdd`'s `if (!chainInfo || !compatResult?.compatible) return`.
    /// `now_iso` stamps `added_at_iso` (shell clock, 016 rule).
    AddConfirmed {
        now_iso: String,
    },
    /// Modal closed — `reset()`.
    WizardReset,
    /// The EIP-681 scan recovery path (`addCustomNetworkByChainId`): resolve
    /// → check → save without user confirmation. Shares the wizard pipeline
    /// AND the invariant-① dedup gate the TS scan path was missing.
    AddByChainIdRequested {
        chain_id: u32,
        now_iso: String,
    },
    /// The delete affordance on a custom network's card (the confirm dialog
    /// is the shell's).
    DeleteConfirmed {
        id: String,
    },

    // -- per-network overrides ----------------------------------------------
    /// A network card expanded — seed the drafts and run the two health
    /// probes (SettingsScreen.tsx:214-223).
    OverrideExpanded {
        chain_id: u32,
    },
    /// A keystroke in the RPC/Explorer field. Re-probes BOTH fields, exactly
    /// as the TS effect's `[expanded, rpcURL, explorerURL]` deps do.
    OverrideFieldEdited {
        chain_id: u32,
        field: NetOverrideField,
        value: String,
    },
    /// Field blur — save, behind the invariant-④ chain-id gate. The write is
    /// refused only when the RPC draft positively identified ANOTHER chain;
    /// an unverifiable endpoint saves. A blur that arrives while the card's
    /// `eth_chainId` probe is still in flight is held until it answers (see
    /// the module doc).
    OverrideBlurred {
        chain_id: u32,
    },

    // -- service endpoints --------------------------------------------------
    /// The endpoint editor opened — probe all four fields. (TS also re-reads
    /// storage here; the core model is the always-loaded mirror.)
    EndpointsOpened,
    EndpointEdited {
        field: NetEndpointField,
        value: String,
    },
    /// Blur — trim + CR/LF-strip (invariant ⑥), persist, flush pools,
    /// re-probe all four.
    EndpointBlurred {
        field: NetEndpointField,
    },
    /// The refresh affordance.
    EndpointsRefreshRequested,
    /// Persists the defaults and re-probes. Quirk kept: TS does NOT flush
    /// the pools on reset (SettingsScreen.tsx:509), unlike a field blur.
    ResetEndpointsToDefaults,

    // -- RPC providers ------------------------------------------------------
    /// The provider modal opened — seed drafts from the saved keys and
    /// auto-test every configured provider.
    ProvidersOpened,
    /// A keystroke in a key field — drops that provider's stale test results
    /// (RpcProvidersModal.tsx:111-115).
    ProviderKeyEdited {
        provider: NetProviderId,
        value: String,
    },
    /// Blur — persist ALL drafts (trimmed, empties dropped — a cleared key
    /// fully removes the provider, invariant ⑦), flush pools, re-test.
    ProviderKeyBlurred {
        provider: NetProviderId,
    },
    /// The explicit "Test" button.
    ProviderTestRequested {
        provider: NetProviderId,
    },

    /// Internal: an effect resolved. `attempt` is the issuing wave's
    /// generation (drawn from one machine-wide counter, so values never
    /// collide across subsystems); a result carrying a superseded generation
    /// is dropped — stale probes can never repaint fresh state.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: NetShellResult,
    },
}

// ---------------------------------------------------------------------------
// Failure vocabulary
// ---------------------------------------------------------------------------

/// Why the wizard stopped. One variant per distinct TS outcome; the shell
/// owns the words.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetWizardErrorKind {
    /// 'This network is already added' — the invariant-① gate, now serving
    /// BOTH the wizard and the scan path.
    AlreadyAdded { chain_id: u32 },
    /// `Chain {id} not found` — the registry has no such chain.
    NotFound { chain_id: u32 },
    /// 'No RPC endpoint available for this network' (wizard only:
    /// SettingsScreen.tsx:641-643).
    NoRpcEndpoint,
    /// Scan path only: the checker said no. Verbatim to `add-network.ts:47`,
    /// this FLATTENS an inconclusive RPC failure into not-compatible; the
    /// wizard keeps the distinction via [`NetCompatibility::rpc_failure`].
    NotCompatible { chain_id: u32 },
    /// The check could not reach a verdict — the RPC probes failed, so
    /// nothing was learned about the chain (spec 038 #E1). Distinct from
    /// [`Self::NotCompatible`], which is a verdict: the scan path used to
    /// flatten both into "not compatible" (ported verbatim from
    /// `add-network.ts:47`), and a machine whose proxy was refusing told the
    /// founder that Celo was incompatible. Celo is not. Additive.
    CheckFailed { chain_id: u32 },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// One candidate URL in the fastest-RPC race.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    url: String,
    /// `None` = pending; `Some(None)` = failed; `Some(Some(ms))` = answered.
    outcome: Option<Option<f64>>,
}

/// The two-strategy RIP-7212 probe (network-checker.ts:191-212).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum P256Probe {
    AwaitingCall,
    AwaitingCode,
    Done(bool),
}

#[derive(Clone, Debug, Default, PartialEq)]
enum WizardPhase {
    #[default]
    Idle,
    /// Debounce and/or index fetch in flight.
    Searching,
    Suggested,
    Resolving {
        chain_id: u32,
    },
    /// Racing the candidate probes for the fastest responsive RPC.
    Probing {
        chain_id: u32,
        candidates: Vec<Candidate>,
    },
    /// `eth_getCode` ×11 + the P256 probe against the race winner.
    CheckingContracts {
        chain_id: u32,
        best_url: String,
        best_latency_ms: f64,
        deployed: Vec<Option<bool>>,
        p256: P256Probe,
    },
    Checked,
    Error {
        kind: NetWizardErrorKind,
    },
}

#[derive(Default)]
struct Wizard {
    phase: WizardPhase,
    query: String,
    custom_rpc: String,
    suggestions: Vec<NetChainIndexEntry>,
    chain_info: Option<NetChainInfo>,
    compat: Option<NetCompatibility>,
    /// `true` = the scan path: auto-save on compatible, no confirm step.
    auto: bool,
    /// The scan path's `added_at_iso` stamp, carried on its request event.
    auto_now_iso: String,
}

/// One expanded network card's editing + health state.
#[derive(Clone, Debug, PartialEq)]
struct OverrideCard {
    rpc_draft: String,
    explorer_draft: String,
    rpc_health: NetProbeHealth,
    explorer_health: NetProbeHealth,
    /// What the current `rpc_draft` claims to be, for the invariant-④ gate:
    /// `None` = the probe has not answered yet, `Some(None)` = it could not be
    /// verified (timeout, refusal, unparseable id, or an empty URL — nothing to
    /// verify), `Some(Some(id))` = the endpoint positively identified `id`.
    ///
    /// Distinguishing "not yet" from "cannot" is the whole gate: only the third
    /// case can ever refuse a save.
    rpc_chain_id: Option<Option<u32>>,
    /// A blur landed while `rpc_chain_id` was still `None`. The save is owed
    /// and runs the moment the verdict arrives — writing first and checking
    /// afterwards would have already poisoned the pool.
    save_deferred: bool,
    /// The refusal the last blur produced, cleared by the next probe wave (any
    /// edit starts one), so a card never shows a verdict about a URL that is no
    /// longer in the field.
    mismatch: Option<NetChainMismatch>,
}

/// One provider's per-chain probe run.
#[derive(Clone, Debug, PartialEq)]
struct ProviderTest {
    done: bool,
    rows: Vec<ProviderRow>,
}

#[derive(Clone, Debug, PartialEq)]
struct ProviderRow {
    chain_id: u32,
    /// `None` = no URL buildable (unsupported chain/empty key) — the row
    /// reads unavailable immediately, verbatim (RpcProvidersModal.tsx:75).
    url: Option<String>,
    /// `None` = probe pending.
    outcome: Option<ProviderOutcome>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProviderOutcome {
    ok: bool,
    latency_ms: f64,
}

#[derive(Default)]
pub struct Model {
    /// The four stores have been read. Writes before that are dropped —
    /// mutating a ledger that is not the ledger would fabricate state.
    loaded: bool,
    custom_networks: Vec<NetCustomNetwork>,
    /// Storage order preserved (`vela.networkConfig` is an array).
    overrides: Vec<NetNetworkConfig>,
    endpoints: NetServiceEndpoints,
    endpoint_drafts: NetServiceEndpoints,
    endpoint_health: BTreeMap<NetEndpointField, NetServiceHealth>,
    provider_keys: NetProviderKeys,
    provider_drafts: BTreeMap<NetProviderId, String>,
    provider_tests: BTreeMap<NetProviderId, ProviderTest>,
    override_cards: BTreeMap<u32, OverrideCard>,
    wizard: Wizard,
    last_added_chain_id: Option<u32>,

    /// One machine-wide wave counter — every subsystem's current generation
    /// is drawn from it, so an `attempt` value identifies exactly one wave.
    counter: u64,
    load_gen: u64,
    search_gen: u64,
    wizard_gen: u64,
    endpoint_gen: u64,
    override_gens: BTreeMap<u32, u64>,
    provider_gens: BTreeMap<NetProviderId, u64>,
}

fn next_gen(model: &mut Model) -> u64 {
    model.counter += 1;
    model.counter
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// A card-field health badge (`EndpointHealth`, SettingsScreen.tsx:93).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetProbeHealth {
    Checking,
    Ok { latency_ms: f64 },
    Error,
}

/// A service-endpoint badge (`ServiceHealth`, SettingsScreen.tsx:334-338),
/// with the English `detail` strings replaced by data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetServiceHealth {
    Checking,
    Ok {
        latency_ms: f64,
        /// Fiat only: how many currencies the endpoint served.
        rate_count: Option<u32>,
    },
    NotHttps,
    Unreachable {
        /// `Some` = the server answered with a non-2xx status.
        http_status: Option<u32>,
        latency_ms: Option<f64>,
    },
    /// Reachable but not the service it must be (identity mismatch, empty
    /// rate map). A warning badge — it does NOT gate saves (current
    /// behavior, kept).
    InvalidResponse {
        latency_ms: f64,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum NetWizardPhase {
    Idle,
    Searching,
    Suggested,
    Resolving,
    Checking,
    Checked,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetWizardView {
    pub phase: NetWizardPhase,
    pub query: String,
    pub custom_rpc: String,
    pub suggestions: Vec<NetChainIndexEntry>,
    pub chain_info: Option<NetChainInfo>,
    pub compat: Option<NetCompatibility>,
    pub error: Option<NetWizardErrorKind>,
    /// The "Add network" button renders only when this is true.
    pub can_add: bool,
}

/// One row of the network editor (defaults first, then customs — the
/// `getAllNetworks` order).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetNetworkRow {
    pub id: String,
    pub chain_id: u32,
    pub display_name: String,
    pub native_symbol: String,
    /// Custom networks get the delete affordance.
    pub is_custom: bool,
    /// Draft if the card is open, else override value, else the default —
    /// `savedConfig?.rpcURL ?? network.rpcURL` semantics (an override's empty
    /// string passes through, verbatim).
    pub rpc_url: String,
    pub explorer_url: String,
    /// Not editable per network; shown for completeness.
    pub bundler_url: String,
    pub rpc_health: Option<NetProbeHealth>,
    pub explorer_health: Option<NetProbeHealth>,
    /// `Some` ⇒ the last blur was REFUSED: the RPC in the field answered
    /// `eth_chainId` with another chain's id, so nothing was written and the
    /// pool still serves the previous endpoint. The shell words it (invariant
    /// ④); the next keystroke clears it.
    pub rpc_chain_mismatch: Option<NetChainMismatch>,
    /// `true` ⇒ a blur is waiting on the chain-id verdict; the override has NOT
    /// been written yet. The card's `rpc_health` is `Checking` for the same
    /// reason, so a shell that renders nothing extra is still honest.
    pub rpc_save_deferred: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetEndpointView {
    pub field: NetEndpointField,
    pub value: String,
    /// The placeholder.
    pub default_value: String,
    pub health: NetServiceHealth,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetProviderNetRow {
    pub chain_id: u32,
    pub ok: bool,
    pub latency_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetProviderTestView {
    pub done: bool,
    pub results: Vec<NetProviderNetRow>,
    pub ok_count: u32,
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetProviderView {
    pub provider: NetProviderId,
    pub key: String,
    pub has_key: bool,
    pub test: Option<NetProviderTestView>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct NetView {
    pub loaded: bool,
    pub networks: Vec<NetNetworkRow>,
    pub wizard: NetWizardView,
    pub endpoints: Vec<NetEndpointView>,
    pub providers: Vec<NetProviderView>,
    pub last_added_chain_id: Option<u32>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct NetworkAdmin;

impl App for NetworkAdmin {
    type Event = Event;
    type Model = Model;
    type ViewModel = NetView;
    type Effect = NetEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<NetEffect, Event> {
        match event {
            Event::Started => {
                model.load_gen = next_gen(model);
                requests_with(model.load_gen, vec![NetOperation::ReadStore])
            }

            // -- wizard ------------------------------------------------------
            Event::SearchInput { query } => search_input(model, query),
            Event::ChainSelected {
                chain_id,
                keep_custom_rpc,
            } => select_chain(model, chain_id, keep_custom_rpc, false, String::new()),
            Event::CustomRpcEdited { value } => {
                model.wizard.custom_rpc = value;
                render()
            }
            Event::AddConfirmed { now_iso } => add_confirmed(model, now_iso),
            Event::WizardReset => {
                model.wizard = Wizard::default();
                // Orphan any in-flight wizard results.
                model.wizard_gen = next_gen(model);
                render()
            }
            Event::AddByChainIdRequested { chain_id, now_iso } => {
                select_chain(model, chain_id, false, true, now_iso)
            }
            Event::DeleteConfirmed { id } => delete_custom(model, &id),

            // -- overrides ---------------------------------------------------
            Event::OverrideExpanded { chain_id } => override_expanded(model, chain_id),
            Event::OverrideFieldEdited {
                chain_id,
                field,
                value,
            } => override_edited(model, chain_id, field, value),
            Event::OverrideBlurred { chain_id } => override_blurred(model, chain_id),

            // -- endpoints ---------------------------------------------------
            Event::EndpointsOpened | Event::EndpointsRefreshRequested => endpoint_probe_wave(model),
            Event::EndpointEdited { field, value } => {
                model.endpoint_drafts.set(field, value);
                render()
            }
            Event::EndpointBlurred { field } => endpoint_blurred(model, field),
            Event::ResetEndpointsToDefaults => reset_endpoints(model),

            // -- providers ---------------------------------------------------
            Event::ProvidersOpened => providers_opened(model),
            Event::ProviderKeyEdited { provider, value } => {
                model.provider_drafts.insert(provider, value);
                model.provider_tests.remove(&provider);
                // Orphan the in-flight wave — a stale latency must never be
                // shown against the new key (RpcProvidersModal.tsx:113-114).
                let gen = next_gen(model);
                model.provider_gens.insert(provider, gen);
                render()
            }
            Event::ProviderKeyBlurred { provider } => provider_blurred(model, provider),
            Event::ProviderTestRequested { provider } => {
                let raw = model
                    .provider_drafts
                    .get(&provider)
                    .cloned()
                    .unwrap_or_default();
                let (cmd, _) = run_provider_test(model, provider, &raw);
                cmd
            }

            Event::ShellCompleted { attempt, result } => accept(model, attempt, result),
        }
    }

    fn view(&self, model: &Model) -> NetView {
        NetView {
            loaded: model.loaded,
            networks: network_rows(model),
            wizard: wizard_view(model),
            endpoints: ENDPOINT_FIELDS
                .iter()
                .map(|&field| NetEndpointView {
                    field,
                    value: model.endpoint_drafts.get(field).to_owned(),
                    default_value: NetServiceEndpoints::default().get(field).to_owned(),
                    health: model
                        .endpoint_health
                        .get(&field)
                        .cloned()
                        .unwrap_or(NetServiceHealth::Checking),
                })
                .collect(),
            providers: PROVIDER_ORDER
                .iter()
                .map(|&provider| provider_view(model, provider))
                .collect(),
            last_added_chain_id: model.last_added_chain_id,
        }
    }
}

// ---------------------------------------------------------------------------
// Wizard — search
// ---------------------------------------------------------------------------

fn search_input(model: &mut Model, query: String) -> Command<NetEffect, Event> {
    // `handleQueryChange`: typing clears the selected chain, its info, the
    // compat result and any error — the new query owns the screen.
    model.wizard.query = query.clone();
    model.wizard.chain_info = None;
    model.wizard.compat = None;
    model.wizard.auto = false;
    // Any in-flight resolve/check belongs to the cleared selection.
    model.wizard_gen = next_gen(model);
    if query.trim().is_empty() {
        model.wizard.suggestions.clear();
        model.wizard.phase = WizardPhase::Idle;
        return render();
    }
    model.wizard.phase = WizardPhase::Searching;
    model.search_gen = next_gen(model);
    requests_with(
        model.search_gen,
        vec![NetOperation::StartSearchDebounce {
            ms: SEARCH_DEBOUNCE_MS,
        }],
    )
}

/// `searchChains` (chain-registry.ts:116-145): exact chainId hit first
/// (invariant ⑧), then substring matches over name/symbol/shortName/id,
/// deduped, capped at 10.
pub fn rank_search(chains: &[NetChainIndexEntry], query: &str) -> Vec<NetChainIndexEntry> {
    let trimmed = query.trim();
    if trimmed.is_empty() {
        return Vec::new();
    }
    let q = trimmed.to_lowercase();

    let exact =
        parse_int_prefix(&q).and_then(|n| chains.iter().find(|c| i64::from(c.chain_id) == n));

    let mut results: Vec<NetChainIndexEntry> = Vec::new();
    if let Some(hit) = exact {
        results.push(hit.clone());
    }
    for c in chains {
        let matches = c.name.to_lowercase().contains(&q)
            || c.native_currency_symbol.to_lowercase().contains(&q)
            || c.short_name.to_lowercase().contains(&q)
            || c.chain_id.to_string().contains(&q);
        if matches && !results.iter().any(|r| r.chain_id == c.chain_id) {
            results.push(c.clone());
        }
        if results.len() >= 10 {
            break;
        }
    }
    results
}

/// `parseInt(q, 10)` — an optional sign then leading decimal digits; anything
/// else is NaN. (So "137abc" → 137 and "0x89" → 0, exactly as JS.)
fn parse_int_prefix(s: &str) -> Option<i64> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let mut negative = false;
    if i < bytes.len() && (bytes[i] == b'+' || bytes[i] == b'-') {
        negative = bytes[i] == b'-';
        i += 1;
    }
    let start = i;
    let mut value: i64 = 0;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        value = value
            .saturating_mul(10)
            .saturating_add(i64::from(bytes[i] - b'0'));
        i += 1;
    }
    if i == start {
        return None;
    }
    Some(if negative { -value } else { value })
}

// ---------------------------------------------------------------------------
// Wizard — select, resolve, check
// ---------------------------------------------------------------------------

fn select_chain(
    model: &mut Model,
    chain_id: u32,
    keep_custom_rpc: bool,
    auto: bool,
    now_iso: String,
) -> Command<NetEffect, Event> {
    // The dedup gate reads the custom-network ledger; acting before it is
    // loaded could add a duplicate. Fail closed.
    if !model.loaded {
        return Command::done();
    }
    if !keep_custom_rpc {
        model.wizard.custom_rpc.clear();
    }
    model.wizard.suggestions.clear();
    model.wizard.chain_info = None;
    model.wizard.compat = None;
    model.wizard.auto = auto;
    model.wizard.auto_now_iso = now_iso;
    model.wizard_gen = next_gen(model);

    // Invariant ① — ONE dedup implementation for both callers. (Today the
    // modal checks and the scan path does not; the inventory collapses the
    // divergence onto this gate.)
    let duplicate =
        builtin(chain_id).is_some() || model.custom_networks.iter().any(|n| n.chain_id == chain_id);
    if duplicate {
        model.wizard.phase = WizardPhase::Error {
            kind: NetWizardErrorKind::AlreadyAdded { chain_id },
        };
        return render();
    }

    model.wizard.phase = WizardPhase::Resolving { chain_id };
    requests_with(
        model.wizard_gen,
        vec![NetOperation::FetchChainInfo { chain_id }],
    )
}

/// `parseChainData` (chain-registry.ts:62-78) — every `??` default kept.
pub fn parse_chain_data(
    data: &NetRawChainData,
    requested_chain_id: u32,
    ethereum_data_url: &str,
) -> NetChainInfo {
    NetChainInfo {
        chain_id: data.chain_id.unwrap_or(requested_chain_id),
        name: data
            .name
            .clone()
            .unwrap_or_else(|| format!("Chain {requested_chain_id}")),
        short_name: data.short_name.clone().unwrap_or_default(),
        native_name: data
            .native_currency_name
            .clone()
            .unwrap_or_else(|| "Ether".to_owned()),
        native_symbol: data
            .native_currency_symbol
            .clone()
            .unwrap_or_else(|| "ETH".to_owned()),
        native_decimals: data.native_currency_decimals.unwrap_or(18),
        rpc_url: extract_rpc_url(&data.rpc),
        rpc_urls: extract_all_rpc_urls(&data.rpc),
        explorer_url: data.explorers.first().cloned().unwrap_or_default(),
        logo_url: format!("{ethereum_data_url}/chainlogos/eip155-{requested_chain_id}.png"),
        is_testnet: data.testnet,
    }
}

/// `extractAllRpcUrls` (chain-registry.ts:151-156): HTTPS only, and a URL
/// carrying a key placeholder (`${` or `API_KEY`) NEVER enters the candidate
/// list (invariant ⑧).
pub fn extract_all_rpc_urls(rpc: &[String]) -> Vec<String> {
    rpc.iter()
        .filter(|u| u.starts_with("https://") && !u.contains("${") && !u.contains("API_KEY"))
        .cloned()
        .collect()
}

/// `extractRpcUrl` (chain-registry.ts:158-169): prefer a clean HTTPS URL;
/// fall back to the first HTTPS one even with a placeholder — verbatim.
pub fn extract_rpc_url(rpc: &[String]) -> String {
    let https: Vec<&String> = rpc.iter().filter(|u| u.starts_with("https://")).collect();
    https
        .iter()
        .find(|u| !u.contains("${") && !u.contains("API_KEY"))
        .or_else(|| https.first())
        .map(|u| (*u).clone())
        .unwrap_or_default()
}

fn chain_info_fetched(
    model: &mut Model,
    chain_id: u32,
    data: Option<NetRawChainData>,
) -> Command<NetEffect, Event> {
    let WizardPhase::Resolving { chain_id: expected } = model.wizard.phase else {
        return Command::done();
    };
    if expected != chain_id {
        return Command::done();
    }

    let Some(raw) = data else {
        model.wizard.phase = WizardPhase::Error {
            kind: NetWizardErrorKind::NotFound { chain_id },
        };
        return render();
    };

    let info = parse_chain_data(&raw, chain_id, effective_ethereum_data_url(model));
    if !model.wizard.auto {
        // `setQuery(info.name)` — the search box shows the resolved name.
        model.wizard.query = info.name.clone();
    }

    // Candidate assembly. Wizard (SettingsScreen.tsx:634-643): the typed RPC
    // first, then the registry list (or its single URL). Scan path
    // (add-network.ts:46): the registry list as-is.
    let mut rpcs: Vec<String> = Vec::new();
    if !model.wizard.auto {
        let custom = model.wizard.custom_rpc.trim();
        if !custom.is_empty() {
            rpcs.push(custom.to_owned());
        }
    }
    if !info.rpc_urls.is_empty() {
        rpcs.extend(info.rpc_urls.iter().cloned());
    } else if !info.rpc_url.is_empty() {
        rpcs.push(info.rpc_url.clone());
    }

    if !model.wizard.auto && rpcs.is_empty() {
        model.wizard.chain_info = Some(info);
        model.wizard.phase = WizardPhase::Error {
            kind: NetWizardErrorKind::NoRpcEndpoint,
        };
        return render();
    }

    // `checkNetworkCompatibility` step 1 (network-checker.ts:49-57): HTTPS
    // filter + Set dedup, then the registry's single URL appended. (The TS
    // re-fetches chain info for that append; folded here — module doc.)
    let mut candidates: Vec<String> = Vec::new();
    for url in rpcs {
        if url.starts_with("https://") && !candidates.contains(&url) {
            candidates.push(url);
        }
    }
    if !info.rpc_url.is_empty() && !candidates.contains(&info.rpc_url) {
        candidates.push(info.rpc_url.clone());
    }

    model.wizard.chain_info = Some(info);

    if candidates.is_empty() {
        return conclude_rpc_failure(model, chain_id, NetRpcFailureKind::NoHttpsCandidates);
    }

    let ops: Vec<NetOperation> = candidates
        .iter()
        .map(|url| NetOperation::ProbeRpc { url: url.clone() })
        .collect();
    model.wizard.phase = WizardPhase::Probing {
        chain_id,
        candidates: candidates
            .into_iter()
            .map(|url| Candidate { url, outcome: None })
            .collect(),
    };
    requests_with(model.wizard_gen, ops)
}

/// All-contracts-undeployed + the failure kind — 'unable to verify', never
/// 'not compatible' (invariant ③).
fn conclude_rpc_failure(
    model: &mut Model,
    chain_id: u32,
    kind: NetRpcFailureKind,
) -> Command<NetEffect, Event> {
    let compat = NetCompatibility {
        chain_id,
        compatible: false,
        multi_key_ready: false,
        contracts: REQUIRED_CONTRACTS
            .iter()
            .map(|(name, address, multi_key_only)| NetContractStatus {
                name: (*name).to_owned(),
                address: (*address).to_owned(),
                deployed: false,
                multi_key_only: *multi_key_only,
            })
            .collect(),
        p256_available: None,
        best_rpc_url: None,
        best_rpc_latency_ms: None,
        rpc_failure: Some(kind),
    };
    finish_check(model, compat)
}

fn wizard_probed(
    model: &mut Model,
    url: &str,
    reported: Option<u32>,
    latency_ms: f64,
) -> Command<NetEffect, Event> {
    let WizardPhase::Probing {
        chain_id,
        ref mut candidates,
    } = model.wizard.phase
    else {
        return Command::done();
    };
    let Some(candidate) = candidates
        .iter_mut()
        .find(|c| c.url == url && c.outcome.is_none())
    else {
        return Command::done();
    };
    // `testRpcLatency`: any parsed JSON-RPC answer counts as responsive —
    // the reported id is NOT matched here (verbatim; only the provider probe
    // matches, invariant ⑦ / module doc).
    candidate.outcome = Some(reported.map(|_| latency_ms));

    if candidates.iter().any(|c| c.outcome.is_none()) {
        return Command::done();
    }

    // `pickFastestRpc`: fastest responsive candidate; ties keep list order
    // (JS stable sort).
    let mut best: Option<(String, f64)> = None;
    for c in candidates.iter() {
        if let Some(Some(latency)) = c.outcome {
            let better = match &best {
                None => true,
                Some((_, current)) => latency < *current,
            };
            if better {
                best = Some((c.url.clone(), latency));
            }
        }
    }

    let Some((best_url, best_latency_ms)) = best else {
        return conclude_rpc_failure(model, chain_id, NetRpcFailureKind::AllProbesFailed);
    };

    let mut ops: Vec<NetOperation> = REQUIRED_CONTRACTS
        .iter()
        .map(|(_, address, _)| NetOperation::RpcGetCode {
            url: best_url.clone(),
            address: (*address).to_owned(),
        })
        .collect();
    ops.push(NetOperation::RpcCallP256 {
        url: best_url.clone(),
    });
    model.wizard.phase = WizardPhase::CheckingContracts {
        chain_id,
        best_url,
        best_latency_ms,
        deployed: vec![None; REQUIRED_CONTRACTS.len()],
        p256: P256Probe::AwaitingCall,
    };
    requests_with(model.wizard_gen, ops)
}

/// `checkCode` (network-checker.ts:244-250): empty / `0x` / `0x0` /
/// RPC-error ⇒ not deployed; any longer answer (even a zkSync bytecode
/// hash) ⇒ deployed.
pub fn is_code_deployed(code: Option<&str>) -> bool {
    match code {
        None | Some("") | Some("0x") | Some("0x0") => false,
        Some(other) => other.len() > 2,
    }
}

/// Probe strategy 1 acceptance (network-checker.ts:197-204): a non-`0x`
/// result of at least 66 chars whose numeric value is exactly 1.
pub fn p256_call_indicates_support(result: Option<&str>) -> bool {
    let Some(r) = result else { return false };
    // `if (callResult)` — empty string is falsy.
    if r.is_empty() || r == "0x" || r.len() < 66 {
        return false;
    }
    big_int_is_one(r)
}

/// `BigInt(s) === 1n`, including the throw-⇒-false behavior of the TS
/// `try/catch` (a malformed digit falls through to strategy 2).
fn big_int_is_one(s: &str) -> bool {
    let (digits, is_hex) = match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        Some(rest) => (rest, true),
        None => (s, false),
    };
    if digits.is_empty() {
        return false;
    }
    let valid = if is_hex {
        digits.bytes().all(|b| b.is_ascii_hexdigit())
    } else {
        digits.bytes().all(|b| b.is_ascii_digit())
    };
    if !valid {
        return false;
    }
    digits.trim_start_matches('0') == "1"
}

fn wizard_code(
    model: &mut Model,
    url: &str,
    address: &str,
    code: Option<String>,
) -> Command<NetEffect, Event> {
    let WizardPhase::CheckingContracts {
        ref best_url,
        ref mut deployed,
        ref mut p256,
        ..
    } = model.wizard.phase
    else {
        return Command::done();
    };
    if url != best_url {
        return Command::done();
    }
    if address == P256_PRECOMPILE {
        // Strategy 2: code at the precompile address == RIP-7212 present
        // (network-checker.ts:206-211).
        if *p256 == P256Probe::AwaitingCode {
            *p256 = P256Probe::Done(is_code_deployed(code.as_deref()));
        }
    } else if let Some(index) = REQUIRED_CONTRACTS.iter().position(|(_, a, _)| *a == address) {
        if let Some(slot) = deployed.get_mut(index) {
            if slot.is_none() {
                *slot = Some(is_code_deployed(code.as_deref()));
            }
        }
    } else {
        return Command::done();
    }
    maybe_finish_contracts(model)
}

fn wizard_p256_call(
    model: &mut Model,
    url: &str,
    result: Option<String>,
) -> Command<NetEffect, Event> {
    let WizardPhase::CheckingContracts {
        ref best_url,
        ref mut p256,
        ..
    } = model.wizard.phase
    else {
        return Command::done();
    };
    if url != best_url || *p256 != P256Probe::AwaitingCall {
        return Command::done();
    }
    if p256_call_indicates_support(result.as_deref()) {
        *p256 = P256Probe::Done(true);
        return maybe_finish_contracts(model);
    }
    *p256 = P256Probe::AwaitingCode;
    let op = NetOperation::RpcGetCode {
        url: best_url.clone(),
        address: P256_PRECOMPILE.to_owned(),
    };
    requests_with(model.wizard_gen, vec![op])
}

fn maybe_finish_contracts(model: &mut Model) -> Command<NetEffect, Event> {
    let WizardPhase::CheckingContracts {
        chain_id,
        ref best_url,
        best_latency_ms,
        ref deployed,
        p256,
    } = model.wizard.phase
    else {
        return Command::done();
    };
    let all_answered = deployed.iter().all(Option::is_some);
    let P256Probe::Done(p256_available) = p256 else {
        return Command::done();
    };
    if !all_answered {
        return Command::done();
    }

    let contracts: Vec<NetContractStatus> = REQUIRED_CONTRACTS
        .iter()
        .zip(deployed.iter())
        .map(|((name, address, multi_key_only), status)| NetContractStatus {
            name: (*name).to_owned(),
            address: (*address).to_owned(),
            deployed: status.unwrap_or(false),
            multi_key_only: *multi_key_only,
        })
        .collect();
    // Invariant ②, split by spec 081: a one-key wallet needs the ten common
    // contracts and the P256 precompile; a multi-key wallet needs the signer
    // factory and its singleton as well, because its extra keys are signer
    // contracts created inside the Safe's setup.
    let single_key_deployed = contracts
        .iter()
        .filter(|c| !c.multi_key_only)
        .all(|c| c.deployed);
    let compatible = single_key_deployed && p256_available;
    let multi_key_ready = compatible && contracts.iter().all(|c| c.deployed);

    let compat = NetCompatibility {
        chain_id,
        compatible,
        multi_key_ready,
        contracts,
        p256_available: Some(p256_available),
        best_rpc_url: Some(best_url.clone()),
        best_rpc_latency_ms: Some(best_latency_ms),
        rpc_failure: None,
    };
    finish_check(model, compat)
}

fn finish_check(model: &mut Model, compat: NetCompatibility) -> Command<NetEffect, Event> {
    if !model.wizard.auto {
        model.wizard.compat = Some(compat);
        model.wizard.phase = WizardPhase::Checked;
        return render();
    }

    // Scan path: auto-save on compatible, no confirm step.
    if compat.compatible {
        let Some(info) = model.wizard.chain_info.clone() else {
            // Unreachable by construction; refuse rather than save garbage.
            model.wizard.phase = WizardPhase::Error {
                kind: NetWizardErrorKind::NotCompatible {
                    chain_id: compat.chain_id,
                },
            };
            return render();
        };
        let now_iso = model.wizard.auto_now_iso.clone();
        let record = build_custom_network(model, &info, compat.best_rpc_url.as_deref(), now_iso);
        model.wizard = Wizard::default();
        return save_custom_network(model, record);
    }

    // Two verdicts, kept apart (spec 038 #E1): a probe that failed says
    // "could not check" — with the retry the wizard already draws — and
    // never "not compatible". `add-network.ts:47` flattened them; that was
    // the trap, not a rule worth porting.
    model.wizard.phase = WizardPhase::Error {
        kind: if compat.rpc_failure.is_some() {
            NetWizardErrorKind::CheckFailed {
                chain_id: compat.chain_id,
            }
        } else {
            NetWizardErrorKind::NotCompatible {
                chain_id: compat.chain_id,
            }
        },
    };
    render()
}

fn add_confirmed(model: &mut Model, now_iso: String) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    // `if (!chainInfo || !compatResult?.compatible) return` — and only the
    // wizard's own Checked state may confirm.
    if model.wizard.phase != WizardPhase::Checked || model.wizard.auto {
        return Command::done();
    }
    let (Some(info), Some(compat)) = (model.wizard.chain_info.clone(), model.wizard.compat.clone())
    else {
        return Command::done();
    };
    if !compat.compatible {
        return Command::done();
    }
    let record = build_custom_network(model, &info, compat.best_rpc_url.as_deref(), now_iso);
    // `onAdded(); reset(); onClose();`
    model.wizard = Wizard::default();
    save_custom_network(model, record)
}

/// The `CustomNetwork` record both entry points build (SettingsScreen.tsx:
/// 653-667 and `chainInfoToCustomNetwork` — identical given the parser's
/// defaults).
fn build_custom_network(
    model: &Model,
    info: &NetChainInfo,
    best_rpc_url: Option<&str>,
    now_iso: String,
) -> NetCustomNetwork {
    NetCustomNetwork {
        id: format!("custom-{}", info.chain_id),
        display_name: info.name.clone(),
        chain_id: info.chain_id,
        icon_label: info.native_symbol.chars().take(4).collect(),
        icon_color: "#888888".to_owned(),
        icon_bg: "#F0F0F0".to_owned(),
        logo_url: info.logo_url.clone(),
        is_l2: false,
        // The fastest RPC wins; the registry's single URL is the fallback.
        rpc_url: best_rpc_url.unwrap_or(&info.rpc_url).to_owned(),
        explorer_url: info.explorer_url.clone(),
        bundler_url: format!("{}/{}", effective_bundler_service_url(model), info.chain_id),
        native_symbol: info.native_symbol.clone(),
        added_at_iso: now_iso,
    }
}

/// `saveCustomNetwork` semantics: upsert by id, then persist the whole array.
fn save_custom_network(model: &mut Model, record: NetCustomNetwork) -> Command<NetEffect, Event> {
    model.last_added_chain_id = Some(record.chain_id);
    model.custom_networks.retain(|n| n.id != record.id);
    model.custom_networks.push(record);
    let gen = model.wizard_gen;
    requests_with(
        gen,
        vec![NetOperation::WriteCustomNetworks {
            networks: model.custom_networks.clone(),
        }],
    )
}

fn delete_custom(model: &mut Model, id: &str) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    model.custom_networks.retain(|n| n.id != id);
    // `removeCustomNetwork` + `invalidateAllPools()` — the removed chain's
    // cached endpoints must not keep serving (invariant ⑤). The chain's
    // override record is NOT removed — verbatim leftover.
    let gen = model.load_gen;
    requests_with(
        gen,
        vec![
            NetOperation::WriteCustomNetworks {
                networks: model.custom_networks.clone(),
            },
            NetOperation::InvalidatePools { chain_id: None },
        ],
    )
}

// ---------------------------------------------------------------------------
// Per-network overrides
// ---------------------------------------------------------------------------

/// The card's fallback values: a built-in row from the canonical table, a
/// custom row from its stored record.
struct NetworkDefaults {
    rpc_url: String,
    explorer_url: String,
    bundler_url: String,
}

fn network_defaults(model: &Model, chain_id: u32) -> Option<NetworkDefaults> {
    if let Some(b) = builtin(chain_id) {
        return Some(NetworkDefaults {
            rpc_url: b.rpc_url.to_owned(),
            explorer_url: b.explorer_url.to_owned(),
            // Empty, NOT the relay URL: a built-in network has no bundler
            // of its own, and a snapshot of today's relay written here is
            // what `network_row` would later mistake for a per-network
            // override — freezing the chain on the relay that happened to be
            // configured when somebody edited its RPC.
            bundler_url: String::new(),
        });
    }
    model
        .custom_networks
        .iter()
        .find(|n| n.chain_id == chain_id)
        .map(|n| NetworkDefaults {
            rpc_url: n.rpc_url.clone(),
            explorer_url: n.explorer_url.clone(),
            bundler_url: n.bundler_url.clone(),
        })
}

fn override_for(model: &Model, chain_id: u32) -> Option<&NetNetworkConfig> {
    model.overrides.iter().find(|c| c.chain_id == chain_id)
}

fn override_expanded(model: &mut Model, chain_id: u32) -> Command<NetEffect, Event> {
    let Some(defaults) = network_defaults(model, chain_id) else {
        return Command::done();
    };
    if !model.override_cards.contains_key(&chain_id) {
        // Seed `savedConfig?.rpcURL ?? network.rpcURL` — an override's empty
        // string passes through (it is not nullish), verbatim.
        let (rpc, explorer) = match override_for(model, chain_id) {
            Some(config) => (config.rpc_url.clone(), config.explorer_url.clone()),
            None => (defaults.rpc_url.clone(), defaults.explorer_url.clone()),
        };
        model.override_cards.insert(
            chain_id,
            OverrideCard {
                rpc_draft: rpc,
                explorer_draft: explorer,
                rpc_health: NetProbeHealth::Checking,
                explorer_health: NetProbeHealth::Checking,
                rpc_chain_id: None,
                save_deferred: false,
                mismatch: None,
            },
        );
    }
    override_probe_wave(model, chain_id)
}

fn override_edited(
    model: &mut Model,
    chain_id: u32,
    field: NetOverrideField,
    value: String,
) -> Command<NetEffect, Event> {
    let Some(card) = model.override_cards.get_mut(&chain_id) else {
        return Command::done();
    };
    match field {
        NetOverrideField::Rpc => card.rpc_draft = value,
        NetOverrideField::Explorer => card.explorer_draft = value,
    }
    // The TS effect re-probes BOTH fields on any change of either.
    override_probe_wave(model, chain_id)
}

/// Both health probes for one card — a fresh wave; a stale answer from the
/// previous wave is dropped by generation.
fn override_probe_wave(model: &mut Model, chain_id: u32) -> Command<NetEffect, Event> {
    let gen = next_gen(model);
    model.override_gens.insert(chain_id, gen);
    let Some(card) = model.override_cards.get_mut(&chain_id) else {
        return Command::done();
    };
    // A fresh wave is a fresh question: whatever the last one concluded about
    // the previous URL says nothing about this one.
    card.mismatch = None;
    let mut ops = Vec::new();
    if card.rpc_draft.is_empty() {
        // `checkEndpointHealth('')` answers error without fetching.
        card.rpc_health = NetProbeHealth::Error;
        // An empty URL claims no chain, so the gate has nothing to disprove —
        // "unverifiable", which saves (verbatim: today an empty override is
        // storable, and `savedConfig?.rpcURL ?? ...` passes it through).
        card.rpc_chain_id = Some(None);
    } else {
        card.rpc_health = NetProbeHealth::Checking;
        card.rpc_chain_id = None;
        ops.push(NetOperation::ProbeRpc {
            url: card.rpc_draft.clone(),
        });
    }
    if card.explorer_draft.is_empty() {
        card.explorer_health = NetProbeHealth::Error;
    } else {
        card.explorer_health = NetProbeHealth::Checking;
        ops.push(NetOperation::ProbeReachable {
            url: card.explorer_draft.clone(),
        });
    }
    let probes = requests_with(gen, ops);
    // The wave just answered the gate itself (empty URL ⇒ no probe to wait
    // for). An owed save must not be stranded waiting for a `Probed` that will
    // never arrive.
    let owed = model
        .override_cards
        .get(&chain_id)
        .is_some_and(|c| c.save_deferred && c.rpc_chain_id == Some(None));
    if owed {
        return Command::all([probes, resolve_override_save(model, chain_id, None)]);
    }
    probes
}

/// Field blur. Invariant ④'s gate: a save happens unless the endpoint in the
/// RPC field PROVED it serves a different chain.
fn override_blurred(model: &mut Model, chain_id: u32) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    if network_defaults(model, chain_id).is_none() {
        return Command::done();
    }
    let Some(card) = model.override_cards.get_mut(&chain_id) else {
        return Command::done();
    };
    match card.rpc_chain_id {
        // The verdict is in — decide now.
        Some(reported) => resolve_override_save(model, chain_id, reported),
        // Still probing. Hold the write: the whole point of the gate is that a
        // wrong-chain URL must never reach the pool's top tier, and a save that
        // is undone a second later has already served balances from the wrong
        // chain.
        None => {
            card.save_deferred = true;
            render()
        }
    }
}

/// Apply the gate to a known verdict: refuse a CONFIRMED mismatch, save
/// anything else (including "could not verify" — invariant ③'s discipline).
fn resolve_override_save(
    model: &mut Model,
    chain_id: u32,
    reported: Option<u32>,
) -> Command<NetEffect, Event> {
    let Some(card) = model.override_cards.get_mut(&chain_id) else {
        return Command::done();
    };
    card.save_deferred = false;
    match reported {
        Some(reported_chain_id) if reported_chain_id != chain_id => {
            card.mismatch = Some(NetChainMismatch {
                expected_chain_id: chain_id,
                reported_chain_id,
            });
            // Nothing written, nothing flushed: the previously saved endpoint
            // keeps serving, which is the safe half of the two.
            render()
        }
        _ => {
            card.mismatch = None;
            commit_override(model, chain_id)
        }
    }
}

/// The write itself — the pre-gate `OverrideBlurred` body, unchanged.
fn commit_override(model: &mut Model, chain_id: u32) -> Command<NetEffect, Event> {
    let Some(defaults) = network_defaults(model, chain_id) else {
        return Command::done();
    };
    let Some(card) = model.override_cards.get(&chain_id) else {
        return Command::done();
    };
    // Invariant ⑤: the bundler is NOT editable here — preserve whatever was
    // already saved so a custom network's bundler is never clobbered
    // (SettingsScreen.tsx:204-211).
    let bundler_url = override_for(model, chain_id)
        .map(|c| c.bundler_url.clone())
        .unwrap_or(defaults.bundler_url);
    let config = NetNetworkConfig {
        chain_id,
        rpc_url: card.rpc_draft.clone(),
        explorer_url: card.explorer_draft.clone(),
        bundler_url,
    };
    // `saveNetworkConfig`: upsert by chainId.
    model.overrides.retain(|c| c.chain_id != chain_id);
    model.overrides.push(config);

    // Invariant ⑤ second half: the old pool/bundler caches must not keep
    // serving the replaced endpoints (SettingsScreen.tsx:288-308).
    let gen = model.load_gen;
    requests_with(
        gen,
        vec![
            NetOperation::WriteNetworkConfigs {
                configs: model.overrides.clone(),
            },
            NetOperation::InvalidatePools {
                chain_id: Some(chain_id),
            },
            NetOperation::ClearBundlerCache { chain_id },
        ],
    )
}

// ---------------------------------------------------------------------------
// Service endpoints
// ---------------------------------------------------------------------------

/// `getEthereumDataURL()` — the configured value, or the default when unset
/// or empty (`||` semantics, storage.ts:194-196).
fn effective_ethereum_data_url(model: &Model) -> &str {
    model.endpoints.effective(NetEndpointField::EthereumData)
}

/// `getBundlerServiceURL()` (storage.ts:202-204).
fn effective_bundler_service_url(model: &Model) -> &str {
    model.endpoints.effective(NetEndpointField::BundlerService)
}

/// `getPasskeyIndexURL()` (storage.ts:198-200). Spec 081 FR-002 — the one
/// endpoint no shell was asking the core about.
#[must_use]
pub fn effective_passkey_index_url(model: &Model) -> &str {
    model.endpoints.effective(NetEndpointField::PasskeyIndex)
}

/// The HTTPS exception: `/^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/`
/// (SettingsScreen.tsx:352). Anchored on the FULL host label —
/// `http://127.0.0.1.evil.com` is NOT localhost.
pub fn is_localhost_http(url: &str) -> bool {
    let Some(rest) = url.strip_prefix("http://") else {
        return false;
    };
    let host = ["localhost", "127.0.0.1"]
        .iter()
        .find_map(|h| rest.strip_prefix(h));
    let Some(mut tail) = host else {
        return false;
    };
    if let Some(port) = tail.strip_prefix(':') {
        let digits = port.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return false;
        }
        tail = &port[digits..];
    }
    tail.is_empty() || tail.starts_with('/')
}

/// `value.trim().replace(/[\r\n]/g, '')` — the header-injection guard
/// (invariant ⑥, SettingsScreen.tsx:453-460).
pub fn clean_endpoint_value(value: &str) -> String {
    value.trim().replace(['\r', '\n'], "")
}

/// Probe all four endpoint fields — a fresh wave.
fn endpoint_probe_wave(model: &mut Model) -> Command<NetEffect, Event> {
    model.endpoint_gen = next_gen(model);
    let mut ops = Vec::new();
    for &field in &ENDPOINT_FIELDS {
        let url = model.endpoint_drafts.get(field).to_owned();
        if url.is_empty() {
            // 'Empty URL' (SettingsScreen.tsx:349).
            model.endpoint_health.insert(
                field,
                NetServiceHealth::Unreachable {
                    http_status: None,
                    latency_ms: None,
                },
            );
            continue;
        }
        // The HTTPS gate runs on the RAW value (a leading space fails it) —
        // verbatim ordering of checkServiceEndpointHealth.
        if !url.starts_with("https://") && !is_localhost_http(&url) {
            model
                .endpoint_health
                .insert(field, NetServiceHealth::NotHttps);
            continue;
        }
        model
            .endpoint_health
            .insert(field, NetServiceHealth::Checking);
        if field == NetEndpointField::FiatRates {
            // Fiat keeps its query string; only trim + CR/LF strip.
            ops.push(NetOperation::FetchFiatRates {
                url: clean_endpoint_value(&url),
            });
        } else {
            // `/api/health` identity check: trim, CR/LF strip, one trailing
            // slash off.
            let base = clean_endpoint_value(&url);
            let base = base.strip_suffix('/').unwrap_or(&base).to_owned();
            ops.push(NetOperation::FetchServiceHealth {
                field,
                base_url: base,
            });
        }
    }
    requests_with(model.endpoint_gen, ops)
}

fn endpoint_blurred(model: &mut Model, field: NetEndpointField) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    let clean = clean_endpoint_value(model.endpoint_drafts.get(field));
    model.endpoint_drafts.set(field, clean.clone());
    model.endpoints.set(field, clean);
    // `handleSave`: persist, flush EVERY pool (invariant ⑤ — endpoint
    // changes must not leave stale pool state), then re-probe.
    let write = requests_with(
        model.endpoint_gen,
        vec![
            NetOperation::WriteServiceEndpoints {
                endpoints: model.endpoints.clone(),
            },
            NetOperation::InvalidatePools { chain_id: None },
        ],
    );
    let probes = endpoint_probe_wave(model);
    Command::all([write, probes])
}

fn reset_endpoints(model: &mut Model) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    model.endpoints = NetServiceEndpoints::default();
    model.endpoint_drafts = NetServiceEndpoints::default();
    // Quirk kept: reset persists and re-probes but does NOT flush the pools
    // (SettingsScreen.tsx:509 — no `invalidateAllPools()`), unlike a blur.
    let write = requests_with(
        model.endpoint_gen,
        vec![NetOperation::WriteServiceEndpoints {
            endpoints: model.endpoints.clone(),
        }],
    );
    let probes = endpoint_probe_wave(model);
    Command::all([write, probes])
}

fn service_health_result(
    model: &mut Model,
    field: NetEndpointField,
    body: NetHealthBody,
    latency_ms: f64,
) -> Command<NetEffect, Event> {
    let health = match body {
        NetHealthBody::Failed => NetServiceHealth::Unreachable {
            http_status: None,
            latency_ms: None,
        },
        NetHealthBody::HttpError { status } => NetServiceHealth::Unreachable {
            http_status: Some(status),
            latency_ms: Some(latency_ms),
        },
        NetHealthBody::Identity { service, status } => {
            // Invariant ⑥: the endpoint must BE the service it claims —
            // a passkey index pointed elsewhere is a login-safety failure.
            let identity_ok =
                identity_accepted(field, service.as_deref()) && status.as_deref() == Some("ok");
            if identity_ok {
                NetServiceHealth::Ok {
                    latency_ms,
                    rate_count: None,
                }
            } else {
                NetServiceHealth::InvalidResponse { latency_ms }
            }
        }
        // An identity endpoint answering a rates body is not that service.
        NetHealthBody::Rates { .. } => NetServiceHealth::InvalidResponse { latency_ms },
    };
    model.endpoint_health.insert(field, health);
    render()
}

fn fiat_health_result(
    model: &mut Model,
    body: NetHealthBody,
    latency_ms: f64,
) -> Command<NetEffect, Event> {
    let health = match body {
        NetHealthBody::Failed => NetServiceHealth::Unreachable {
            http_status: None,
            latency_ms: None,
        },
        NetHealthBody::HttpError { status } => NetServiceHealth::Unreachable {
            http_status: Some(status),
            latency_ms: Some(latency_ms),
        },
        // 'No rates returned' — reachable but not a rates service.
        NetHealthBody::Rates { rate_count: 0 } => NetServiceHealth::InvalidResponse { latency_ms },
        NetHealthBody::Rates { rate_count } => NetServiceHealth::Ok {
            latency_ms,
            rate_count: Some(rate_count),
        },
        // A fiat endpoint answering an identity body has no rates.
        NetHealthBody::Identity { .. } => NetServiceHealth::InvalidResponse { latency_ms },
    };
    model
        .endpoint_health
        .insert(NetEndpointField::FiatRates, health);
    render()
}

// ---------------------------------------------------------------------------
// RPC providers
// ---------------------------------------------------------------------------

fn providers_opened(model: &mut Model) -> Command<NetEffect, Event> {
    // Seed drafts from the saved keys; drop stale test results; auto-test
    // every configured provider (RpcProvidersModal.tsx:85-96).
    model.provider_drafts.clear();
    model.provider_tests.clear();
    let mut commands: Vec<Command<NetEffect, Event>> = Vec::new();
    for &provider in &PROVIDER_ORDER {
        let saved = model.provider_keys.get(provider).cloned();
        if let Some(key) = saved {
            model.provider_drafts.insert(provider, key.clone());
            let (cmd, _) = run_provider_test(model, provider, &key);
            commands.push(cmd);
        }
    }
    commands.push(render());
    Command::all(commands)
}

fn provider_blurred(model: &mut Model, provider: NetProviderId) -> Command<NetEffect, Event> {
    if !model.loaded {
        return Command::done();
    }
    let trimmed = model
        .provider_drafts
        .get(&provider)
        .map(|k| k.trim().to_owned())
        .unwrap_or_default();
    model.provider_drafts.insert(provider, trimmed.clone());

    // `saveRpcProviders`: trim every entry, DROP empties — a cleared key
    // fully removes the provider (invariant ⑦, storage.ts:323-335). Note
    // that other providers' in-progress drafts persist too, verbatim
    // (`persist(next)` writes the whole draft map).
    let mut cleaned = NetProviderKeys::default();
    for &id in &PROVIDER_ORDER {
        let value = model
            .provider_drafts
            .get(&id)
            .map(|k| k.trim().to_owned())
            .unwrap_or_default();
        cleaned.set(id, (!value.is_empty()).then_some(value));
    }
    model.provider_keys = cleaned;

    // Key change ⇒ every chain's pool must rebuild (invariant ⑦).
    let write = requests_with(
        model.load_gen,
        vec![
            NetOperation::WriteRpcProviders {
                keys: model.provider_keys.clone(),
            },
            NetOperation::InvalidatePools { chain_id: None },
        ],
    );
    let (test, _) = run_provider_test(model, provider, &trimmed);
    Command::all([write, test])
}

/// `runTest` (RpcProvidersModal.tsx:51-82): probe every supported chain in
/// parallel; a row is ok only when the REPORTED id equals the target
/// (invariant ⑦ — the one probe site that matches ids).
fn run_provider_test(
    model: &mut Model,
    provider: NetProviderId,
    raw_key: &str,
) -> (Command<NetEffect, Event>, bool) {
    let key = raw_key.trim().to_owned();
    let gen = next_gen(model);
    model.provider_gens.insert(provider, gen);
    if key.is_empty() {
        model.provider_tests.remove(&provider);
        return (render(), false);
    }
    let rows: Vec<ProviderRow> = provider_chain_ids(provider)
        .into_iter()
        .map(|chain_id| {
            let url = build_provider_rpc_url(provider, chain_id, &key);
            let outcome = url.is_none().then_some(ProviderOutcome {
                ok: false,
                latency_ms: 0.0,
            });
            ProviderRow {
                chain_id,
                url,
                outcome,
            }
        })
        .collect();
    let ops: Vec<NetOperation> = rows
        .iter()
        .filter_map(|r| r.url.clone())
        .map(|url| NetOperation::ProbeRpc { url })
        .collect();
    let done = ops.is_empty();
    model
        .provider_tests
        .insert(provider, ProviderTest { done, rows });
    (requests_with(gen, ops), done)
}

fn provider_probed(
    model: &mut Model,
    provider: NetProviderId,
    url: &str,
    reported: Option<u32>,
    latency_ms: f64,
) -> Command<NetEffect, Event> {
    let Some(test) = model.provider_tests.get_mut(&provider) else {
        return Command::done();
    };
    let Some(row) = test
        .rows
        .iter_mut()
        .find(|r| r.outcome.is_none() && r.url.as_deref() == Some(url))
    else {
        return Command::done();
    };
    row.outcome = Some(ProviderOutcome {
        // `ok: reported === r.chainId` (rpc-providers modal:78) — a probe
        // reporting the WRONG chain is unavailable, never "fast".
        ok: reported == Some(row.chain_id),
        latency_ms,
    });
    if test.rows.iter().all(|r| r.outcome.is_some()) {
        test.done = true;
    }
    render()
}

// ---------------------------------------------------------------------------
// Explorer links (invariant ⑨)
// ---------------------------------------------------------------------------

/// `explorerBaseURL` (network.ts:87-90): the chain's explorer with one
/// trailing slash stripped — or `None` for an unknown chain or an empty
/// configured URL. Security surfaces (the signing sheet) must show NO link
/// rather than a misleading etherscan.io one; the etherscan fallback lives
/// only in the display-side tx/address/token builders, which stay in the
/// shell.
pub fn explorer_base_url(chain_id: u32, custom_networks: &[NetCustomNetwork]) -> Option<String> {
    let url = builtin(chain_id)
        .map(|b| b.explorer_url.to_owned())
        .or_else(|| {
            custom_networks
                .iter()
                .find(|n| n.chain_id == chain_id)
                .map(|n| n.explorer_url.clone())
        })?;
    if url.is_empty() {
        return None;
    }
    Some(url.strip_suffix('/').unwrap_or(&url).to_owned())
}

// ---------------------------------------------------------------------------
// Shell results — routing
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, attempt: u64, result: NetShellResult) -> Command<NetEffect, Event> {
    match result {
        NetShellResult::StoreLoaded {
            custom_networks,
            network_configs,
            endpoints,
            provider_keys,
        } => {
            if attempt != model.load_gen {
                return Command::done();
            }
            model.custom_networks = custom_networks;
            model.overrides = network_configs;
            model.endpoints = NetServiceEndpoints {
                ethereum_data_url: endpoints
                    .ethereum_data_url
                    .unwrap_or_else(|| DEFAULT_ETHEREUM_DATA_URL.to_owned()),
                passkey_index_url: endpoints
                    .passkey_index_url
                    .unwrap_or_else(|| DEFAULT_PASSKEY_INDEX_URL.to_owned()),
                bundler_service_url: endpoints
                    .bundler_service_url
                    .unwrap_or_else(|| DEFAULT_BUNDLER_SERVICE_URL.to_owned()),
                fiat_rates_url: endpoints
                    .fiat_rates_url
                    .unwrap_or_else(|| DEFAULT_FIAT_RATES_URL.to_owned()),
            };
            model.endpoint_drafts = model.endpoints.clone();
            model.provider_keys = provider_keys;
            model.loaded = true;
            render()
        }

        NetShellResult::DebounceElapsed => {
            if attempt != model.search_gen {
                // A superseded keystroke's timer — dropping it IS the
                // debounce (only the last timer may search).
                return Command::done();
            }
            requests_with(model.search_gen, vec![NetOperation::FetchSearchIndex])
        }
        NetShellResult::SearchIndex { chains } => {
            if attempt != model.search_gen {
                return Command::done();
            }
            model.wizard.suggestions = rank_search(&chains, &model.wizard.query);
            model.wizard.phase = WizardPhase::Suggested;
            render()
        }

        NetShellResult::ChainInfo { chain_id, data } => {
            if attempt != model.wizard_gen {
                return Command::done();
            }
            chain_info_fetched(model, chain_id, data)
        }
        NetShellResult::Code { url, address, code } => {
            if attempt != model.wizard_gen {
                return Command::done();
            }
            wizard_code(model, &url, &address, code)
        }
        NetShellResult::P256Call { url, result } => {
            if attempt != model.wizard_gen {
                return Command::done();
            }
            wizard_p256_call(model, &url, result)
        }

        NetShellResult::Probed {
            url,
            reported_chain_id,
            latency_ms,
        } => {
            // The unified probe serves three consumers; the wave generation
            // (globally unique) says exactly which one asked.
            if attempt == model.wizard_gen {
                return wizard_probed(model, &url, reported_chain_id, latency_ms);
            }
            if let Some((&chain_id, _)) =
                model.override_gens.iter().find(|(_, &gen)| gen == attempt)
            {
                if let Some(card) = model.override_cards.get_mut(&chain_id) {
                    // `checkEndpointHealth` rpc: any JSON-RPC answer is ok.
                    // The badge stays a LIVENESS badge — a wrong-chain node is
                    // still a node that answered, and conflating the two would
                    // make "offline" mean two different things. The chain
                    // identity is a separate verdict, below.
                    card.rpc_health = match reported_chain_id {
                        Some(_) => NetProbeHealth::Ok { latency_ms },
                        None => NetProbeHealth::Error,
                    };
                    // Invariant ④'s gate gets its answer from THIS probe — the
                    // card already asks the endpoint what chain it is, so the
                    // save needs no request of its own.
                    card.rpc_chain_id = Some(reported_chain_id);
                    if card.save_deferred {
                        return resolve_override_save(model, chain_id, reported_chain_id);
                    }
                    return render();
                }
                return Command::done();
            }
            if let Some((&provider, _)) =
                model.provider_gens.iter().find(|(_, &gen)| gen == attempt)
            {
                return provider_probed(model, provider, &url, reported_chain_id, latency_ms);
            }
            Command::done()
        }
        NetShellResult::Reachable {
            url: _,
            ok,
            latency_ms,
        } => {
            if let Some((&chain_id, _)) =
                model.override_gens.iter().find(|(_, &gen)| gen == attempt)
            {
                if let Some(card) = model.override_cards.get_mut(&chain_id) {
                    card.explorer_health = if ok {
                        NetProbeHealth::Ok { latency_ms }
                    } else {
                        NetProbeHealth::Error
                    };
                    return render();
                }
            }
            Command::done()
        }

        NetShellResult::ServiceHealth {
            field,
            body,
            latency_ms,
        } => {
            if attempt != model.endpoint_gen {
                return Command::done();
            }
            service_health_result(model, field, body, latency_ms)
        }
        NetShellResult::FiatRates { body, latency_ms } => {
            if attempt != model.endpoint_gen {
                return Command::done();
            }
            fiat_health_result(model, body, latency_ms)
        }

        // Best-effort acknowledgements — never change state.
        NetShellResult::Written
        | NetShellResult::Invalidated
        | NetShellResult::BundlerCacheCleared => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// View assembly
// ---------------------------------------------------------------------------

fn network_rows(model: &Model) -> Vec<NetNetworkRow> {
    let mut rows: Vec<NetNetworkRow> = Vec::new();
    for b in &BUILTIN_CHAINS {
        rows.push(network_row(
            model,
            b.id.to_owned(),
            b.chain_id,
            b.display_name.to_owned(),
            b.native_symbol.to_owned(),
            false,
            b.rpc_url.to_owned(),
            b.explorer_url.to_owned(),
            format!("{}/{}", effective_bundler_service_url(model), b.chain_id),
        ));
    }
    for n in &model.custom_networks {
        rows.push(network_row(
            model,
            n.id.clone(),
            n.chain_id,
            n.display_name.clone(),
            n.native_symbol.clone(),
            true,
            n.rpc_url.clone(),
            n.explorer_url.clone(),
            n.bundler_url.clone(),
        ));
    }
    rows
}

#[allow(clippy::too_many_arguments)]
fn network_row(
    model: &Model,
    id: String,
    chain_id: u32,
    display_name: String,
    native_symbol: String,
    is_custom: bool,
    default_rpc: String,
    default_explorer: String,
    default_bundler: String,
) -> NetNetworkRow {
    let saved = override_for(model, chain_id);
    let card = model.override_cards.get(&chain_id);
    let rpc_url = card
        .map(|c| c.rpc_draft.clone())
        .or_else(|| saved.map(|c| c.rpc_url.clone()))
        .unwrap_or(default_rpc);
    let explorer_url = card
        .map(|c| c.explorer_draft.clone())
        .or_else(|| saved.map(|c| c.explorer_url.clone()))
        .unwrap_or(default_explorer);
    // An override's bundler counts only when it is one: empty is "none", and
    // so is a record written before this was fixed, which snapshotted the
    // shipped relay verbatim. Either way the default — the CONFIGURED service
    // endpoint for a built-in, the record's own URL for a custom network —
    // must win, or Settings › Service nodes › Vela Relay would go on being a
    // field that changes nothing.
    let shipped = format!("{DEFAULT_BUNDLER_SERVICE_URL}/{chain_id}");
    let bundler_url = saved
        .map(|c| c.bundler_url.clone())
        .filter(|url| !url.is_empty() && *url != shipped)
        .unwrap_or(default_bundler);
    NetNetworkRow {
        id,
        chain_id,
        display_name,
        native_symbol,
        is_custom,
        rpc_url,
        explorer_url,
        bundler_url,
        rpc_health: card.map(|c| c.rpc_health.clone()),
        explorer_health: card.map(|c| c.explorer_health.clone()),
        rpc_chain_mismatch: card.and_then(|c| c.mismatch),
        rpc_save_deferred: card.is_some_and(|c| c.save_deferred),
    }
}

fn wizard_view(model: &Model) -> NetWizardView {
    let (phase, error) = match &model.wizard.phase {
        WizardPhase::Idle => (NetWizardPhase::Idle, None),
        WizardPhase::Searching => (NetWizardPhase::Searching, None),
        WizardPhase::Suggested => (NetWizardPhase::Suggested, None),
        WizardPhase::Resolving { .. } => (NetWizardPhase::Resolving, None),
        WizardPhase::Probing { .. } | WizardPhase::CheckingContracts { .. } => {
            (NetWizardPhase::Checking, None)
        }
        WizardPhase::Checked => (NetWizardPhase::Checked, None),
        WizardPhase::Error { kind } => (NetWizardPhase::Error, Some(kind.clone())),
    };
    let can_add = phase == NetWizardPhase::Checked
        && !model.wizard.auto
        && model.wizard.compat.as_ref().is_some_and(|c| c.compatible);
    NetWizardView {
        phase,
        query: model.wizard.query.clone(),
        custom_rpc: model.wizard.custom_rpc.clone(),
        suggestions: model.wizard.suggestions.clone(),
        chain_info: model.wizard.chain_info.clone(),
        compat: model.wizard.compat.clone(),
        error,
        can_add,
    }
}

fn provider_view(model: &Model, provider: NetProviderId) -> NetProviderView {
    let key = model
        .provider_drafts
        .get(&provider)
        .cloned()
        .unwrap_or_default();
    let test = model.provider_tests.get(&provider).map(|t| {
        let results: Vec<NetProviderNetRow> = t
            .rows
            .iter()
            .map(|r| {
                let outcome = r.outcome.unwrap_or(ProviderOutcome {
                    ok: false,
                    latency_ms: 0.0,
                });
                NetProviderNetRow {
                    chain_id: r.chain_id,
                    ok: outcome.ok,
                    latency_ms: outcome.latency_ms,
                }
            })
            .collect();
        let ok_count = results.iter().filter(|r| r.ok).count();
        NetProviderTestView {
            done: t.done,
            total: u32::try_from(results.len()).unwrap_or(u32::MAX),
            ok_count: u32::try_from(ok_count).unwrap_or(u32::MAX),
            results,
        }
    });
    NetProviderView {
        provider,
        has_key: !key.trim().is_empty(),
        key,
        test,
    }
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must carry `gen` to be accepted.
fn requests_with(gen: u64, operations: Vec<NetOperation>) -> Command<NetEffect, Event> {
    let mut commands: Vec<Command<NetEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation).then_send(move |result| Event::ShellCompleted {
                attempt: gen,
                result,
            })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for NetEffect {
    type Op = NetOperation;
    fn into_shell(self) -> Option<crux_core::Request<NetOperation>> {
        match self {
            NetEffect::Render(_) => None,
            NetEffect::Shell(request) => Some(request),
        }
    }
}
