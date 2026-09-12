//! Machine — clear-signing resolution + risk adjudication (spec
//! `016-crux-wallet-state`, clear_signing; authored under spec 017).
//!
//! ```text
//! ResolveTransaction ─► Now ─► local descriptor ─► contract descriptor (HTTP)
//!        │                        │ no match          │ no match
//!        │                        ▼                    ▼
//!        │              token-standard selectors ─► ERC-165 probes (3s cap)
//!        │                        │ no match
//!        │                        ▼
//!        │              ERC calldata fallbacks ─► 4-byte selector DB ─► blind
//!        │  (any match) ─► decimals warm (4s cap) ─► fields ─► risk ─► result
//! MessagePresented ─► hex/text split ─► SIWE parse ─► domain binding ─► danger class
//! ```
//!
//! This is the "parse pipeline + risk verdict" machine: every HTTP descriptor
//! fetch, RPC `eth_call` and timeout is an [`ClearOperation`] the shell
//! executes; every *judgment* — five-level decode fallback, ERC-165
//! disambiguation, decimals trust, partial/unverified/expired risk floors,
//! SIWE phishing adjudication — lives here. Ported from
//! `src/services/clear-signing.ts`, `siwe.ts`, `decode-sign-message.ts`,
//! `local-descriptors.ts` and the dispatch rules of
//! `SigningSheet.tsx`/`MessageSignView.tsx`.
//!
//! The rules that bought this machine:
//!
//! - **Never silently assume 18 decimals** for an unknown token — a 6-decimal
//!   token rendered at 18 shows an amount 10¹²× wrong on a security surface.
//!   On-chain `decimals()` is prefetched; only when that fails does 18 apply,
//!   with an explicit `unverified` flag that floors risk at caution
//!   (`clear-signing.ts:362-363`, `:1022-1029`).
//! - **ERC-165 caches only definitive verdicts.** `transferFrom`/`approve`
//!   share a selector across ERC-20 and ERC-721; a wrong guess renders an
//!   *amount* as a *tokenId* or vice versa. RPC-unreachable is `unknown`, NOT
//!   "unsupported" — caching it would permanently misclassify a real NFT
//!   (`clear-signing.ts:186-201`). Late probe answers still teach the cache,
//!   exactly like the TS module-level maps.
//! - **`partial` / `unverified` / `expired` never read as safe** — risk floors
//!   at caution; any `warning` field (unlimited approval) is danger
//!   (`clear-signing.ts:1266-1270`).
//! - **Zero resolved fields is a blind sign, never a half-truth**
//!   (`clear-signing.ts:587-590`).
//! - **SIWE domain must be a bare RFC-3986 authority** — userinfo, path or
//!   scheme in the first line means "not SIWE at all" (`siwe.ts:45`); CRLF is
//!   normalized so the line-1 anchor can't be defeated (`siwe.ts:33-36`); an
//!   unparseable request origin is `unknown`, never a spurious match
//!   (`siwe.ts:88-92`). Detection uses the request's own dApp identity (F3).
//! - **Display and signing share one hex predicate** (`decode-sign-message.ts:
//!   33-66` / `use-dapp-signing.ts:180-193`): a non-hex payload is UTF-8 text,
//!   shown verbatim and signed verbatim.
//! - **`eth_sign` is never a calm message view** (`SigningSheet.tsx:465-470`),
//!   and while a descriptor resolves the view stays "loading" — a blind view
//!   must never flash first (`SigningSheet.tsx:441-447`).
//!
//! ## Canon rulings (inventory integration notes)
//!
//! - `siwePhish` was computed twice (SigningSheet's haptic effect at
//!   `SigningSheet.tsx:180-190` and `MessageSignView.tsx:40-44`) with the same
//!   inputs; here it is computed once, in [`ClearMessageView::danger_class`].
//! - `nonPrintable` had TWO predicates: the Unicode-aware signing-path one
//!   (`decode-sign-message.ts isBinaryChar`, canon) and MessageSignView's
//!   ASCII-only regex (`MessageSignView.tsx:29-36`) which additionally flagged
//!   readable CJK/emoji as "non printable" (and ran on un-prefixed odd-length
//!   hex). The signing path is canon per the inventory ruling; the view-side
//!   ASCII variant is intentionally NOT ported (recorded difference).
//! - Simulation asymmetric trust and `replaySim` (invariant ⑧,
//!   `SigningSheet.tsx:68-93, 407-415`) belong to the tx-simulation service and
//!   replay persistence, not this model (the inventory Model carries no sim
//!   state); the shell passes `simConfident` alongside this machine's view.
//!
//! ## Shell contract
//!
//! - [`ClearOperation::HttpGet`]: fetch `getEthereumDataURL() + path` with
//!   `NET_TIMEOUTS.descriptor` (5s); answer [`ClearShellResult::DescriptorFetched`]
//!   with the raw body on 200 and `None` on !ok / timeout / error — the same
//!   observable outcomes `fetchWithTimeout` produced (the descriptor timeout
//!   stays in the fetch layer; ERC-165/decimals timeouts are core [`ClearOperation::Timer`]s).
//! - [`ClearOperation::RpcEthCall`]: `poolRpcCall('eth_call', [{to,data},'latest'], chain)`;
//!   echo `probe`/`chain_id`/`to`, answer `rpc_error: true` when the RPC
//!   returned an error object, `result: None` when it threw / was unreachable.
//!   Interpretation (revert ⇒ not-ERC-165, garbage ⇒ unknown) happens HERE.
//! - [`ClearOperation::SelectorDbLookup`]: `lookupSelector` (selector-registry.ts,
//!   its own network policy + cache); answer the merged candidate list.
//! - [`ClearOperation::Timer`]: fire once after `ms`, echoing `token`.
//! - [`ClearOperation::Now`]: epoch milliseconds — the core owns expiry
//!   judgment but no clock (011 `now_iso` pattern).
//!
//! Locale note: number/date/time rendering uses the semantic preset enums of
//! `locale-format.ts` (the shell resolves `auto` before dispatching, and
//! passes its timezone offset). No locale detection, no prose, no clock here.

use std::collections::{BTreeMap, BTreeSet};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::abi;
use crate::primitives;

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Ported constants (clear-signing.ts)
// ---------------------------------------------------------------------------

/// `INTERFACE_DETECT_TIMEOUT_MS` — the ERC-165 probe race cap.
pub const ERC165_DETECT_TIMEOUT_MS: u32 = 3_000;
/// `DECIMALS_WARM_TIMEOUT_MS` — how long the sheet waits on `decimals()`.
pub const DECIMALS_WARM_TIMEOUT_MS: u32 = 4_000;

const SUPPORTS_INTERFACE_SELECTOR: &str = "0x01ffc9a7";
const ERC20_DECIMALS_SELECTOR: &str = "0x313ce567";
/// `symbol()` — asked for the same reason `decimals()` is: without it an
/// amount is rendered beside a contract address, and nobody can read what
/// they are sending.
const ERC20_SYMBOL_SELECTOR: &str = "0x95d89b41";
/// ERC-165 interface ids (local-descriptors.ts `INTERFACE_IDS`).
const IFACE_ERC721: &str = "80ac58cd";
const IFACE_ERC1155: &str = "d9b67a26";

/// Year ~2100 in unix seconds — beyond it is a "no deadline" sentinel
/// (Permit2 uint48-max etc.), not a date (`clear-signing.ts:1100-1102`).
const NO_DEADLINE_THRESHOLD: f64 = 4_102_444_800.0;

/// Canonical CREATE2 factories most tooling deploys through.
const CREATE2_DEPLOYERS: [&str; 1] = ["0x4e59b44847b379578588920ca78fbf26c0b4956c"];

const SEL_TRANSFER: &str = "0xa9059cbb";
const SEL_TRANSFER_FROM: &str = "0x23b872dd";
const SEL_APPROVE: &str = "0x095ea7b3";
const SEL_INCREASE_ALLOWANCE: &str = "0x39509351";
const SEL_DECREASE_ALLOWANCE: &str = "0xa457c2d7";
const SEL_SAFE_TRANSFER_721: &str = "0x42842e0e";
const SEL_SAFE_TRANSFER_721_DATA: &str = "0xb88d4fde";
const SEL_SET_APPROVAL_ALL: &str = "0xa22cb465";
const SEL_SAFE_TRANSFER_1155: &str = "0xf242432a";
const SEL_SAFE_BATCH_1155: &str = "0x2eb2c2d6";

const TOKEN_STD_SELECTORS: [&str; 10] = [
    SEL_TRANSFER,
    SEL_TRANSFER_FROM,
    SEL_APPROVE,
    SEL_INCREASE_ALLOWANCE,
    SEL_DECREASE_ALLOWANCE,
    SEL_SAFE_TRANSFER_721,
    SEL_SAFE_TRANSFER_721_DATA,
    SEL_SET_APPROVAL_ALL,
    SEL_SAFE_TRANSFER_1155,
    SEL_SAFE_BATCH_1155,
];

const ERC_CALLDATA_FALLBACKS: [&str; 3] = [
    "/erc7730/ercs/calldata-erc20-tokens.json",
    "/erc7730/ercs/calldata-erc721-nfts.json",
    "/erc7730/ercs/calldata-erc4626-vaults.json",
];
const PERMIT_FALLBACK_PATH: &str = "/erc7730/ercs/eip712-erc2612-permit.json";

/// USD-pegged stablecoins valued at ~$1 with no price lookup
/// (`clear-signing.ts STABLE_SYMBOLS`).
const STABLE_SYMBOLS: [&str; 12] = [
    "USDC", "USDT", "DAI", "USDC.e", "USD₮0", "BUSD", "TUSD", "USDP", "FRAX", "LUSD", "GUSD",
    "PYUSD",
];

/// Well-known ERC-20 static metadata (`services/tokens.ts KNOWN_TOKENS`).
const KNOWN_TOKENS: [(&str, &str, u32); 19] = [
    ("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48", "USDC", 6),
    ("0xdac17f958d2ee523a2206206994597c13d831ec7", "USDT", 6),
    ("0x6b175474e89094c44da98b954eedeac495271d0f", "DAI", 18),
    ("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2", "WETH", 18),
    ("0x2260fac5e5542a773aa44fbcfedf7c193bc2c599", "WBTC", 8),
    ("0x514910771af9ca656af840dff83e8264ecf986ca", "LINK", 18),
    ("0x1f9840a85d5af5bf1d1762f925bdaddc4201f984", "UNI", 18),
    ("0xae7ab96520de3a18e5e111b5eaab095312d7fe84", "stETH", 18),
    ("0xbe9895146f7af43049ca1c1ae358b0541ea49704", "cbETH", 18),
    ("0xae78736cd615f374d3085123a210448e74fc6393", "rETH", 18),
    ("0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0", "wstETH", 18),
    ("0x5a98fcbea516cf06857215779fd812ca3bef1b32", "LDO", 18),
    ("0xd533a949740bb3306d119cc777fa900ba034cd52", "CRV", 18),
    ("0x7fc66500c84a76ad7e9c93437bfc5ac33e2ddae9", "AAVE", 18),
    ("0xc00e94cb662c3520282e6f5717214004a7f26888", "COMP", 18),
    ("0x9f8f72aa9304c8b593d555f12ef6589cc3a579a2", "MKR", 18),
    ("0x3c499c542cef5e3811e1192ce70d8cc03d5c3359", "USDC", 6),
    ("0x2791bca1f2de4661ed88a30c99a7a9449aa84174", "USDC.e", 6),
    ("0xaf88d065e77c8cc2239327c5edb3a432268e5831", "USDC", 6),
];

/// Chain id → native coin ticker (`models/chains.ts CHAINS`). Custom networks
/// live in shell storage; unknown ids fall back to "ETH" exactly as
/// `nativeSymbol` does when its custom-network cache misses.
const NATIVE_SYMBOLS: [(u32, &str); 12] = [
    (1, "ETH"),
    (56, "BNB"),
    (137, "POL"),
    (42161, "ETH"),
    (10, "ETH"),
    (8453, "ETH"),
    (43114, "AVAX"),
    (100, "xDAI"),
    (130, "ETH"),
    (4217, "USD"),
    (143, "MON"),
    (480, "ETH"),
];

/// Address → protocol name/owner (`local-descriptors.ts KNOWN_CONTRACTS`).
const KNOWN_CONTRACTS: [(&str, &str, &str); 28] = [
    (
        "0x7a250d5630b4cf539739df2c5dacb4c659f2488d",
        "Uniswap V2 Router",
        "Uniswap",
    ),
    (
        "0xe592427a0aece92de3edee1f18e0157c05861564",
        "Uniswap V3 Router",
        "Uniswap",
    ),
    (
        "0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45",
        "Uniswap V3 Router 2",
        "Uniswap",
    ),
    (
        "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
        "Uniswap Universal Router",
        "Uniswap",
    ),
    (
        "0x66a9893cc07d91d95644aedd05d03f95e1dba8af",
        "Uniswap Universal Router",
        "Uniswap",
    ),
    (
        "0x1111111254eeb25477b68fb85ed929f73a960582",
        "1inch Router (V5)",
        "1inch",
    ),
    (
        "0x111111125421ca6dc452d289314280a0f8842a65",
        "1inch Router (V6)",
        "1inch",
    ),
    (
        "0x000000000022d473030f116ddee9f6b43ac78ba3",
        "Permit2",
        "Uniswap",
    ),
    (
        "0x00000000000000adc04c56bf30ac9d3c0aaf14dc",
        "Seaport 1.5",
        "OpenSea",
    ),
    (
        "0x0000000000000068f116a894984e2db1123eb395",
        "Seaport 1.6",
        "OpenSea",
    ),
    (
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2",
        "Wrapped Ether",
        "WETH",
    ),
    (
        "0x4200000000000000000000000000000000000006",
        "Wrapped Ether",
        "WETH",
    ),
    (
        "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2",
        "Aave V3 Pool",
        "Aave",
    ),
    (
        "0xae7ab96520de3a18e5e111b5eaab095312d7fe84",
        "Lido (stETH)",
        "Lido",
    ),
    (
        "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0",
        "Wrapped stETH",
        "Lido",
    ),
    (
        "0xdef1c0ded9bec7f1a1670819833240f027b25eff",
        "0x Exchange Proxy",
        "0x",
    ),
    (
        "0x9008d19f58aabd9ed0d60971565aa8510560ab41",
        "CoW Protocol",
        "CoW",
    ),
    (
        "0xe66b31678d6c16e9ebf358268a790b763c133750",
        "Coinbase Smart Wallet",
        "Coinbase",
    ),
    (
        "0x10ed43c718714eb63d5aa57b78b54704e256024e",
        "PancakeSwap V2 Router",
        "PancakeSwap",
    ),
    (
        "0x1b81d678ffb9c0263b24a97847620c99d213eb14",
        "PancakeSwap V3 Router",
        "PancakeSwap",
    ),
    (
        "0x13f4ea83d0bd40e75c8222255bc855a974568dd4",
        "PancakeSwap Smart Router",
        "PancakeSwap",
    ),
    (
        "0xd9c500dff816a1da21a48a732d3498bf09dc9aeb",
        "PancakeSwap Universal Router",
        "PancakeSwap",
    ),
    (
        "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f",
        "SushiSwap Router",
        "SushiSwap",
    ),
    (
        "0xba12222222228d8ba445958a75a0704d566bf2c8",
        "Balancer Vault",
        "Balancer",
    ),
    (
        "0x1231deb6f5749ef6ce6943a275a1d3e7486f4eae",
        "LI.FI",
        "LI.FI",
    ),
    (
        "0x6131b5fae19ea4f9d964eac0408e4408b66337b5",
        "KyberSwap Router",
        "KyberSwap",
    ),
    (
        "0x1111111254fb6c44bac0bed2854e76f90643097d",
        "1inch Router (V4)",
        "1inch",
    ),
    (
        "0x9008d19f58aabd9ed0d60971565aa8510560ab42",
        "CoW Protocol",
        "CoW",
    ),
];

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What the shell performs on behalf of a resolution run. Judgment never
/// crosses this boundary — the shell fetches/calls/waits and reports.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearOperation"))]
pub enum ClearOperation {
    /// GET `getEthereumDataURL() + path` (5s `NET_TIMEOUTS.descriptor`).
    /// Answer the body text on 200, `None` otherwise — never an exception.
    HttpGet { path: String },
    /// `eth_call` via the RPC pool. `probe` is echoed back so the core can
    /// route the raw answer without trusting shell interpretation.
    RpcEthCall {
        chain_id: u32,
        to: String,
        data: String,
        probe: ClearProbe,
    },
    /// 4-byte selector database lookup (`selector-registry.ts lookupSelector`
    /// with its merge/dedup/cache policy). `[]` when nothing was found.
    SelectorDbLookup { selector: String },
    /// Fire once after `ms`, echoing `token` (3s ERC-165 / 4s decimals caps,
    /// modeled as explicit timeout results per the inventory).
    Timer { ms: u32, token: u32 },
    /// Read the clock (expiry judgment only — the core owns no time).
    Now,
}

/// Which question an `RpcEthCall` asked. Echoed verbatim in the answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearProbe"))]
pub enum ClearProbe {
    SupportsErc721,
    SupportsErc1155,
    Decimals,
    /// `symbol()`. Additive on purpose: every shell answers `RpcEthCall` by
    /// echoing the probe back, so none of them needed a line to serve this.
    Symbol,
}

/// What the shell observed. Raw on purpose: revert-vs-unreachable and
/// value-vs-garbage judgments happen in the core (`clear-signing.ts:186-201`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearShellResult"))]
pub enum ClearShellResult {
    /// Body text on HTTP 200, `None` on !ok / timeout / network error —
    /// exactly the outcomes `fetchWithTimeout` collapsed to `null`.
    DescriptorFetched {
        path: String,
        json: Option<String>,
    },
    /// `rpc_error`: the RPC answered with an error object (a revert).
    /// `result: None` with `rpc_error: false`: threw / unreachable / no result.
    RpcAnswer {
        probe: ClearProbe,
        chain_id: u32,
        to: String,
        result: Option<String>,
        rpc_error: bool,
    },
    /// Candidate signatures, most-likely first, deduped.
    SelectorCandidates {
        sigs: Vec<String>,
    },
    TimedOut {
        token: u32,
    },
    Clock {
        now_ms: f64,
    },
}

impl Operation for ClearOperation {
    type Output = ClearShellResult;
}

#[effect]
pub enum ClearSigningEffect {
    Render(RenderOperation),
    Shell(ClearOperation),
}

// ---------------------------------------------------------------------------
// Locale presets (semantic enums — locale-format.ts; `auto` resolves in shell)
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearNumberFormat"))]
pub enum ClearNumberFormat {
    #[default]
    CommaDot,
    DotComma,
    SpaceComma,
    Indian,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearDateFormat"))]
pub enum ClearDateFormat {
    YmdSlash,
    Iso,
    DmySlash,
    DmyDot,
    #[default]
    MdySlash,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearTimeFormat"))]
pub enum ClearTimeFormat {
    #[default]
    H24,
    H12,
}

/// The shell's resolved locale conventions for this run. `tz_offset_minutes`
/// is minutes to ADD to UTC to obtain local time (the negation of JS
/// `getTimezoneOffset()`), sampled once per request — the DST-boundary
/// imprecision this introduces for far-away deadlines is cosmetic.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearLocale"))]
pub struct ClearLocale {
    pub number_format: ClearNumberFormat,
    pub date_format: ClearDateFormat,
    pub time_format: ClearTimeFormat,
    pub tz_offset_minutes: i32,
}

impl ClearLocale {
    fn separators(&self) -> (char, char, bool) {
        // (group, decimal, indian) — NUMBER_STYLES, locale-format.ts:33-38.
        match self.number_format {
            ClearNumberFormat::CommaDot => (',', '.', false),
            ClearNumberFormat::DotComma => ('.', ',', false),
            ClearNumberFormat::SpaceComma => (' ', ',', false),
            ClearNumberFormat::Indian => (',', '.', true),
        }
    }
}

// ---------------------------------------------------------------------------
// Wire value types — the resolved result
// ---------------------------------------------------------------------------

/// Risk level for visual treatment (`SigningRisk`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearRisk"))]
pub enum ClearRisk {
    Safe,
    Normal,
    Caution,
    Danger,
}

/// Layout role hint (`FieldRole`; kebab-case in TS, snake_case on this wire).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearFieldRole"))]
pub enum ClearFieldRole {
    SendAmount,
    ReceiveAmount,
    Recipient,
    Spender,
    Generic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSignType"))]
pub enum ClearSignType {
    Transaction,
    Signature,
}

/// One resolved display field (`ClearSignField` in TS; optional booleans
/// became plain flags). `value` keywords ("Unlimited") are descriptor-borne
/// vocabulary the shell localizes exactly as it localizes `intent`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSignField"))]
pub struct ClearSignField {
    pub label: String,
    pub value: String,
    pub format: String,
    /// For tokenAmount: normalized+validated token address (logo lookup).
    pub token_address: Option<String>,
    /// High-risk field (e.g. unlimited approval) → danger.
    pub warning: bool,
    /// Amount shown with unverified decimals → caution, not danger.
    pub unverified: bool,
    pub role: ClearFieldRole,
    /// Collapsed under "Advanced — view raw data" (best-effort params).
    pub detail: bool,
    /// A deadline already in the past → caution.
    pub expired: bool,
    /// Full lowercased address for addressName fields.
    pub address: Option<String>,
    /// USD magnitude when cheaply known (stablecoin peg = $1).
    pub usd_value: Option<f64>,
}

/// Resolved clear-signing result, ready for display (`ClearSignResult`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSignResult"))]
pub struct ClearSignResult {
    pub intent: String,
    pub contract_name: Option<String>,
    pub owner: Option<String>,
    pub fields: Vec<ClearSignField>,
    pub risk: ClearRisk,
    pub contract_address: Option<String>,
    pub verified: bool,
    pub sign_type: ClearSignType,
    /// Descriptor declared more fields than resolved — loud "incomplete".
    pub partial: bool,
    /// Recovered via 4-byte DB, decoded generically — "best effort, not verified".
    pub best_effort: bool,
    /// A recipient of this call IS the contract being called: the token is
    /// being sent to its own contract, which burns it irreversibly. The
    /// single-call twin of [`super::approval_guard::GuardBatchView::any_to_own_token`]
    /// — same predicate, same ASCII-case-insensitive address compare, so one
    /// burn does not warn in a batch and stay silent on its own (and the single
    /// send is the far more common entry point).
    ///
    /// PROJECTION, not resolution state: filled in by [`to_own_token`] when the
    /// view is built, never at construction. Every builder therefore writes
    /// `false` here and cannot get it wrong by forgetting.
    pub to_own_token: bool,
}

/// Is any `recipient` field of this result the contract being called?
///
/// ASCII-case-insensitive, like the batch twin and for the same reason. Both
/// sides HAPPEN to be lowercase today — `start_tx` lowercases `tx.to`,
/// `verifying_contract` lowercases the EIP-712 domain, `build_deploy_result`
/// lowercases its own — but that is four separate normalisations in four
/// builders holding up a security rule from a distance. The dApp sends `tx.to`
/// EIP-55 checksummed; the day one builder forgets, a byte-compare would
/// silently downgrade an irreversible burn to an ordinary transfer with no
/// test failing anywhere. Comparing case-insensitively makes the rule true on
/// its own terms.
fn to_own_token(result: &ClearSignResult) -> bool {
    let Some(contract) = result.contract_address.as_deref() else {
        return false;
    };
    if contract.is_empty() {
        return false;
    }
    result.fields.iter().any(|f| {
        f.role == ClearFieldRole::Recipient
            && f.address
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(contract))
    })
}

// ---------------------------------------------------------------------------
// Wire value types — personal_sign / eth_sign analysis
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSignMethod"))]
pub enum ClearSignMethod {
    PersonalSign,
    /// `eth_sign` signs an OPAQUE hash. It gets the hard-warning surface,
    /// never the calm message view (`SigningSheet.tsx:465-470`).
    EthSign,
}

/// Parsed EIP-4361 fields (`SiweFields`). `chain_id` wider than u32 is
/// dropped (TS `parseInt` kept a float; no real chain needs it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSiweFields"))]
pub struct ClearSiweFields {
    pub domain: String,
    /// The lowercased host the binding was actually COMPARED on (`siweHost`),
    /// or `None` when the authority is unparseable. The domain row renders this
    /// so the string on screen is the string that was adjudicated — showing a
    /// prettier one than the check ran against is how a lookalike slips past.
    pub domain_host: Option<String>,
    pub address: Option<String>,
    pub statement: Option<String>,
    pub uri: Option<String>,
    pub chain_id: Option<u32>,
    pub nonce: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSiweBinding"))]
pub enum ClearSiweBinding {
    Ok,
    Mismatch,
    Unknown,
}

/// Danger dispatch class for the message surfaces. `SiweOk` covers both
/// `binding == Ok` and `Unknown` — the calm sign-in layout; the verified
/// badge renders only on `Ok`, and only `Mismatch` escalates (fail-safe:
/// an unknown origin never asserts a match, but is not phishing evidence).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearDangerClass"))]
pub enum ClearDangerClass {
    Plain,
    SiweOk,
    SiwePhish,
    OpaqueHash,
    EthSign,
}

/// Everything the message view needs, computed once (canon ruling above).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearMessageView"))]
pub struct ClearMessageView {
    /// The param this method actually signs, already chosen (inventory ⑩):
    /// `params[0]` for `personal_sign`, `params[1]` for `eth_sign` — falling
    /// back to `params[0]` for a malformed single-param `eth_sign`. The
    /// `eth_sign` surface renders this string verbatim as the opaque digest.
    pub payload: String,
    /// The single `isHexPayload` predicate both display and signer branch on.
    pub is_hex: bool,
    /// Readable text (hex-decoded UTF-8, or the verbatim non-hex payload).
    pub decoded_text: Option<String>,
    /// Short hex preview for genuinely binary payloads.
    pub binary_preview: Option<String>,
    /// Canon predicate: hex payload whose bytes are not readable text.
    pub non_printable: bool,
    pub siwe: Option<ClearSiweFields>,
    pub binding: Option<ClearSiweBinding>,
    pub danger_class: ClearDangerClass,
}

// ---------------------------------------------------------------------------
// Wire value types — blind typed data (EIP-712 with no descriptor)
// ---------------------------------------------------------------------------

/// One raw `message` entry of an undecodable EIP-712 payload, already rendered
/// to the single line the sheet shows (`BlindTypedDataView.formatBlindValue`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearBlindField"))]
pub struct ClearBlindField {
    pub key: String,
    pub value: String,
}

/// The blind typed-data projection (`BlindTypedDataView.parseTypedDataForDisplay`,
/// inventory ㉓). Computed for EVERY typed-data request the moment it is
/// presented — it is a pure read of the untrusted payload, and the sheet only
/// reaches for it once resolution has concluded with no descriptor.
///
/// Deliberately NOT reinterpreted: no decimals, no timestamp guessing. The
/// descriptor is unknown, so an honest raw value beats a confident wrong one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearBlindTyped"))]
pub struct ClearBlindTyped {
    pub primary_type: Option<String>,
    /// `domain` is present and truthy — the "signing for" bar renders.
    pub has_domain: bool,
    pub domain_name: Option<String>,
    /// `domain.verifyingContract`, lowercased.
    pub verifying_contract: Option<String>,
    /// The first five `message` entries, in payload order.
    pub fields: Vec<ClearBlindField>,
}

// ---------------------------------------------------------------------------
// Wire value types — sheet dispatch (inventory ⑨ and ㉔)
// ---------------------------------------------------------------------------

/// Which surface this machine's slice of the sheet dispatch order resolves to
/// (`SigningSheet.tsx:407-487`).
///
/// The full order is `typed permit → editable approval → LOADING → CLEAR SIGN →
/// batch → ETH_SIGN → MESSAGE → BLIND TYPED → BLIND TX`; the two approval
/// surfaces and the batch list belong to `approval_guard`, so the sheet
/// interleaves exactly two verdicts and decides nothing itself. Everything
/// upper-cased above is decided here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSurface"))]
pub enum ClearSurface {
    /// Nothing presented (or a method this machine does not adjudicate) — the
    /// sheet's shield fallback.
    None,
    /// A descriptor is resolving. Holds the sheet: a blind view must never
    /// flash before the clear one (invariant ⑦).
    Loading,
    ClearSign,
    /// `eth_sign` — the hard-warning surface, never the calm message view.
    EthSign,
    MessageSign,
    BlindTypedData,
    BlindTransaction,
}

/// The SEMANTICS of the confirm button (inventory ㉔). The words stay in the
/// shell — 14+ locales never enter wasm (011) — and so does the "the localized
/// intent is too long to fit" measurement, which is a property of the
/// translated string, not of the request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearConfirm"))]
pub enum ClearConfirm {
    /// A pure signature — "Sign".
    Sign,
    /// Neutral "Confirm". Never "Approve": that verb belongs only to an actual
    /// token approval, which is `approval_guard`'s surface.
    Confirm,
    /// "Confirm {intent}" — the descriptor intent (or `send` for a plain native
    /// transfer) travels as the canonical English key for the shell to localize.
    ConfirmIntent { intent: String },
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSigningEvent"))]
pub enum Event {
    /// `eth_sendTransaction` (or one EIP-5792 batch leg) needs resolution.
    /// Supersedes any in-flight run — a slower previous request must never
    /// overwrite the current one (`SigningSheet.tsx:242-244`).
    ResolveTransaction {
        to: Option<String>,
        data: Option<String>,
        value: Option<String>,
        chain_id: u32,
        #[serde(default)]
        locale: ClearLocale,
    },
    /// `eth_signTypedData*`; the raw JSON is untrusted (`JSON.parse` failures
    /// resolve blind, `SigningSheet.tsx:288-297`).
    ResolveTypedData {
        typed_data_json: String,
        chain_id: u32,
        #[serde(default)]
        locale: ClearLocale,
    },
    /// `personal_sign` / `eth_sign` presented. Pure derivation.
    ///
    /// `params` is the JSON-RPC parameter list, string-coerced, VERBATIM: WHICH
    /// param carries the signed bytes is a rule, not plumbing, so it is decided
    /// here (`SigningSheet.tsx:467-473`) — `params[0]` for `personal_sign`,
    /// `params[1]` for `eth_sign(address, data)`, falling back to `params[0]`
    /// for a malformed single-param `eth_sign`. A shell that picked the wrong
    /// one would show the ADDRESS where the opaque digest belongs.
    /// `request_origin` is `dappInfo.url ?? request.origin` (F3).
    MessagePresented {
        method: ClearSignMethod,
        params: Vec<String>,
        request_origin: Option<String>,
    },
    /// The sheet closed / the request was replaced with nothing.
    Cleared,
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made; a result carrying an older attempt belongs to a
    /// superseded run (its cacheable facts are still absorbed — the TS
    /// module-level caches learned from late answers too).
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: ClearShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokenStandard {
    Erc20,
    Erc721,
    Erc1155,
}

/// Partial ERC-165 answers for one `${chain}:${addr}` — the in-flight probe
/// pair. Survives run supersession so late answers can still conclude and
/// teach the cache (the TS probe promise kept running past the 3s race).
#[derive(Clone, Copy, Debug, Default)]
struct Erc165Scratch {
    is721: Option<Option<bool>>,
    is1155: Option<Option<bool>>,
}

#[derive(Clone, Debug)]
enum Req {
    Tx {
        /// Lowercased. `None` = raw create.
        to: Option<String>,
        data: String,
        value: Option<String>,
        chain_id: u32,
    },
    Typed {
        typed: Value,
        chain_id: u32,
    },
}

impl Req {
    fn chain_id(&self) -> u32 {
        match self {
            Req::Tx { chain_id, .. } | Req::Typed { chain_id, .. } => *chain_id,
        }
    }
}

/// What `resolveCalldataDescriptor` / `resolveEip712*` should do after the
/// decimals warm completes, and where the pipeline goes on a null result.
#[derive(Clone, Debug)]
enum WarmThen {
    Calldata {
        descriptor: Value,
        matched_sig: String,
        is_specific: bool,
        next: TxNext,
    },
    Eip712 {
        descriptor: Value,
        matched_sig: String,
        contract_specific: bool,
        next: TypedNext,
    },
}

#[derive(Clone, Copy, Debug)]
enum TxNext {
    /// The local descriptor lacked this selector — fetch the contract one.
    AfterLocal,
    /// The contract descriptor didn't resolve — try token-standard selectors.
    AfterContract,
    /// Interface descriptors didn't resolve — try the ERC fallbacks.
    AfterTokenStd,
    /// An ERC fallback matched: its outcome is FINAL, even null — the TS
    /// pipeline never falls from a matched fallback to the selector DB.
    Final,
}

#[derive(Clone, Copy, Debug)]
enum TypedNext {
    /// The contract-specific eip712 entry didn't resolve — try ERC-2612.
    AfterEntry,
    Final,
}

#[derive(Clone, Debug)]
// The shared `Await` prefix IS the meaning: every variant is this machine
// parked on one outstanding request. Naming them otherwise loses that.
#[allow(clippy::enum_variant_names)]
enum Step {
    AwaitClock,
    AwaitContractDescriptor,
    /// ERC-165 disambiguation of transferFrom/approve/setApprovalForAll.
    AwaitStandard {
        addr: String,
        selector: String,
        timer: u32,
    },
    AwaitErcFallback {
        index: usize,
    },
    AwaitSelectorSigs,
    /// On-chain `decimals()` and `symbol()` prefetch for unknown tokens.
    ///
    /// Two sets, because the two probes are two facts and either can be the
    /// last one in. Both must land — see [`begin_warm`] — and the 4s timer is
    /// still what bounds the wait.
    AwaitWarm {
        pending: BTreeSet<String>,
        symbols: BTreeSet<String>,
        timer: u32,
        then: WarmThen,
    },
    AwaitTypedDescriptor,
    AwaitPermitDescriptor,
}

#[derive(Clone, Debug)]
struct Run {
    req: Req,
    locale: ClearLocale,
    now_ms: f64,
    step: Step,
}

/// What was last presented. The dispatch order (⑨) and the confirm semantics
/// (㉔) both need to know WHICH request this is once resolution has finished —
/// `run` is gone by then and `result` is `None` for every blind outcome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum ReqKind {
    #[default]
    None,
    /// `eth_sendTransaction` with calldata.
    TxCall,
    /// `eth_sendTransaction` with no calldata — the plain native transfer that
    /// reads "Confirm Send", matching its eyebrow.
    TxPlain,
    Typed,
    PersonalSign,
    EthSign,
}

#[derive(Default)]
pub struct Model {
    /// The in-flight resolution — `Some` IS the "resolving" flag the sheet
    /// uses to hold the loading view (never flash blind, invariant ⑦).
    run: Option<Run>,
    /// A resolution concluded (result may still be `None` ⇒ blind sign).
    resolved: bool,
    result: Option<ClearSignResult>,
    message: Option<ClearMessageView>,
    /// Survives the run so the surface/confirm verdicts stay decidable.
    kind: ReqKind,
    /// The raw EIP-712 projection, held for the blind outcome.
    blind_typed: Option<ClearBlindTyped>,
    /// path → parsed descriptor (`null` failures are cached too, exactly as
    /// `descriptorCache` does — a 404 shouldn't be re-fetched every render).
    descriptor_cache: BTreeMap<String, Option<Value>>,
    /// `${chain}:${addr}` → standard. DEFINITIVE verdicts only (invariant ②).
    token_standard_cache: BTreeMap<String, TokenStandard>,
    /// `${chain}:${addr}` → on-chain `decimals()`. Valid answers only (0–36).
    decimals_cache: BTreeMap<String, u32>,
    /// Token symbols learned from the chain, keyed like the decimals cache.
    ///
    /// `KNOWN_TOKENS` is nineteen addresses with no chain id in them, so it
    /// answers for almost nothing outside Ethereum mainnet. Everything else
    /// used to render as `0x2a22…` beside its amount — on every shell, since
    /// they all run this machine.
    symbol_cache: BTreeMap<String, String>,
    erc165_scratch: BTreeMap<String, Erc165Scratch>,
    /// Bumped per request; a result carrying an older attempt is dropped
    /// (after its cache facts are absorbed).
    attempt: u64,
    /// Discriminates timers: a stale 3s/4s timer from an earlier phase of
    /// the same run must not end the current wait.
    timer_seq: u32,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// The sheet's dispatch VERDICTS (`SigningSheet.tsx:407-487`). [`Self::surface`]
/// already answers "which view", [`Self::confirm`] "which button semantics";
/// `resolving`/`resolved`/`result`/`message` remain because the surfaces render
/// from them (and `resolving` also gates the confirm action).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ClearSigningView"))]
pub struct ClearSigningView {
    pub resolving: bool,
    pub resolved: bool,
    pub result: Option<ClearSignResult>,
    pub message: Option<ClearMessageView>,
    /// This machine's slice of the dispatch order (⑨).
    pub surface: ClearSurface,
    /// Confirm-button semantics (㉔) — words stay in the shell.
    pub confirm: ClearConfirm,
    /// The raw EIP-712 projection (㉓); present for every typed-data request.
    pub blind_typed: Option<ClearBlindTyped>,
    /// The sheet buzzes a warning on open (⑤ of the haptics rules, inventory ㉑)
    /// — `eth_sign`, or a SIWE message whose domain does not bind to the
    /// requesting origin. Computed ONCE here: the haptic and the red banner
    /// must never be able to disagree (canon ruling above). The unbounded
    /// approval half of the same buzz is `approval_guard`'s verdict; the sheet
    /// ORs the two machines' answers and decides nothing.
    pub danger_haptic: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ClearSigning;

impl App for ClearSigning {
    type Event = Event;
    type Model = Model;
    type ViewModel = ClearSigningView;
    type Effect = ClearSigningEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<ClearSigningEffect, Event> {
        match event {
            Event::ResolveTransaction {
                to,
                data,
                value,
                chain_id,
                locale,
            } => start_tx(model, to, data, value, chain_id, locale),
            Event::ResolveTypedData {
                typed_data_json,
                chain_id,
                locale,
            } => start_typed(model, &typed_data_json, chain_id, locale),
            Event::MessagePresented {
                method,
                params,
                request_origin,
            } => {
                // A new message request supersedes any in-flight resolution.
                model.attempt += 1;
                model.run = None;
                model.resolved = false;
                model.result = None;
                model.blind_typed = None;
                model.kind = match method {
                    ClearSignMethod::PersonalSign => ReqKind::PersonalSign,
                    ClearSignMethod::EthSign => ReqKind::EthSign,
                };
                // WHICH param carries the signed bytes is decided here (⑩).
                let payload = match method {
                    ClearSignMethod::PersonalSign => params.first(),
                    // `eth_sign(address, data)` — `params[0]` only for a
                    // malformed single-param request.
                    ClearSignMethod::EthSign => params.get(1).or_else(|| params.first()),
                }
                .cloned()
                .unwrap_or_default();
                model.message = Some(analyze_message(method, &payload, request_origin.as_deref()));
                render()
            }
            Event::Cleared => {
                model.attempt += 1;
                model.run = None;
                model.resolved = false;
                model.result = None;
                model.message = None;
                model.blind_typed = None;
                model.kind = ReqKind::None;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                // Cache-bearing facts are absorbed even from superseded runs:
                // the TS caches were module singletons that learned from every
                // answer, and losing a definitive ERC-165 verdict would force
                // a re-probe (and another 3s stall) on the next request.
                absorb_cache_facts(model, &result);
                if attempt != model.attempt {
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> ClearSigningView {
        ClearSigningView {
            resolving: model.run.is_some(),
            resolved: model.resolved,
            // The burn verdict is graded HERE, on the finished result, so every
            // builder — descriptor, typed data, best-effort, deploy — is covered
            // by one rule and a future fifth builder cannot forget it.
            result: model.result.clone().map(|mut r| {
                r.to_own_token = to_own_token(&r);
                r
            }),
            message: model.message.clone(),
            surface: surface_of(model),
            confirm: confirm_of(model),
            blind_typed: model.blind_typed.clone(),
            danger_haptic: matches!(
                model.message.as_ref().map(|m| m.danger_class),
                Some(ClearDangerClass::EthSign) | Some(ClearDangerClass::SiwePhish)
            ),
        }
    }
}

/// The dispatch verdict (⑨). Resolution ALWAYS outranks a blind surface — the
/// sheet must never flash a red "Unknown" that a descriptor is about to replace.
fn surface_of(model: &Model) -> ClearSurface {
    if model.run.is_some() {
        return ClearSurface::Loading;
    }
    match model.kind {
        ReqKind::None => ClearSurface::None,
        ReqKind::EthSign => ClearSurface::EthSign,
        ReqKind::PersonalSign => ClearSurface::MessageSign,
        ReqKind::TxCall | ReqKind::TxPlain => {
            if model.result.is_some() {
                ClearSurface::ClearSign
            } else {
                ClearSurface::BlindTransaction
            }
        }
        ReqKind::Typed => {
            if model.result.is_some() {
                ClearSurface::ClearSign
            } else {
                ClearSurface::BlindTypedData
            }
        }
    }
}

/// Confirm-button semantics (㉔), in the order `buttonLabel()` reads them.
fn confirm_of(model: &Model) -> ClearConfirm {
    if let Some(result) = &model.result {
        return match result.sign_type {
            ClearSignType::Signature => ClearConfirm::Sign,
            ClearSignType::Transaction => ClearConfirm::ConfirmIntent {
                intent: result.intent.clone(),
            },
        };
    }
    match model.kind {
        ReqKind::PersonalSign | ReqKind::Typed => ClearConfirm::Sign,
        // A plain native send reads "Confirm Send", matching its eyebrow —
        // the same sentence the decoded ERC-20 transfer gets.
        ReqKind::TxPlain => ClearConfirm::ConfirmIntent {
            intent: "send".to_owned(),
        },
        // Blind contract call, `eth_sign`, nothing presented: a neutral
        // "Confirm", never "Approve".
        ReqKind::TxCall | ReqKind::EthSign | ReqKind::None => ClearConfirm::Confirm,
    }
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

fn start_tx(
    model: &mut Model,
    to: Option<String>,
    data: Option<String>,
    value: Option<String>,
    chain_id: u32,
    locale: ClearLocale,
) -> Command<ClearSigningEffect, Event> {
    model.attempt += 1;
    model.run = None;
    model.resolved = false;
    model.result = None;
    model.message = None;
    model.blind_typed = None;
    model.kind = ReqKind::TxCall;

    let data = data.unwrap_or_default();
    if data.is_empty() || data == "0x" {
        // Plain ETH transfer — the modal shows its native transfer UI.
        model.kind = ReqKind::TxPlain;
        model.resolved = true;
        return render();
    }

    // Contract deployment — calm "Deploy contract", never a scary red
    // "Unknown" (raw create, or the canonical CREATE2 deployers).
    let to = to.filter(|t| !t.is_empty());
    let is_deploy = match &to {
        None => true,
        Some(t) => CREATE2_DEPLOYERS.contains(&t.to_lowercase().as_str()),
    };
    if is_deploy {
        model.result = Some(build_deploy_result(to.as_deref(), &data));
        model.resolved = true;
        return render();
    }

    let to = to.map(|t| t.to_lowercase());
    model.run = Some(Run {
        req: Req::Tx {
            to,
            data,
            value,
            chain_id,
        },
        locale,
        now_ms: 0.0,
        step: Step::AwaitClock,
    });
    requests(model, vec![ClearOperation::Now])
}

fn start_typed(
    model: &mut Model,
    typed_data_json: &str,
    chain_id: u32,
    locale: ClearLocale,
) -> Command<ClearSigningEffect, Event> {
    model.attempt += 1;
    model.run = None;
    model.resolved = false;
    model.result = None;
    model.message = None;
    model.kind = ReqKind::Typed;

    let Ok(typed) = serde_json::from_str::<Value>(typed_data_json) else {
        // Untrusted JSON that doesn't parse resolves blind, exactly as the
        // sheet's try/catch does — and the blind surface shows nothing, exactly
        // as `parseTypedDataForDisplay`'s own catch does.
        model.blind_typed = Some(ClearBlindTyped::empty());
        model.resolved = true;
        return render();
    };
    // The raw projection is computed for EVERY typed request, up front: the
    // blind surface needs it whichever way the pipeline ends, and it is a pure
    // read of the payload the shell already holds.
    model.blind_typed = Some(project_blind_typed(typed_data_json));
    if typed
        .get("domain")
        .and_then(|d| d.get("verifyingContract"))
        .and_then(Value::as_str)
        .is_none()
    {
        model.resolved = true;
        return render();
    }

    model.run = Some(Run {
        req: Req::Typed { typed, chain_id },
        locale,
        now_ms: 0.0,
        step: Step::AwaitClock,
    });
    requests(model, vec![ClearOperation::Now])
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

/// Cache-bearing facts land regardless of which run asked (see the callers).
fn absorb_cache_facts(model: &mut Model, result: &ClearShellResult) {
    match result {
        ClearShellResult::DescriptorFetched { path, json } => {
            let parsed = json
                .as_ref()
                .and_then(|body| serde_json::from_str::<Value>(body).ok());
            model.descriptor_cache.insert(path.clone(), parsed);
        }
        ClearShellResult::RpcAnswer {
            probe: ClearProbe::SupportsErc721,
            chain_id,
            to,
            result,
            rpc_error,
        } => {
            let verdict = interpret_supports(result.as_deref(), *rpc_error);
            record_supports(model, *chain_id, to, verdict, true);
        }
        ClearShellResult::RpcAnswer {
            probe: ClearProbe::SupportsErc1155,
            chain_id,
            to,
            result,
            rpc_error,
        } => {
            let verdict = interpret_supports(result.as_deref(), *rpc_error);
            record_supports(model, *chain_id, to, verdict, false);
        }
        ClearShellResult::RpcAnswer {
            probe: ClearProbe::Symbol,
            chain_id,
            to,
            result,
            rpc_error: _,
        } => {
            // Same rule the decimals cache follows: only a real word teaches
            // it. A revert, an empty answer or a token that returns something
            // unreadable leaves the fallback in place rather than caching a
            // guess about what somebody is sending.
            if let Some(symbol) = result.as_deref().and_then(decode_abi_string) {
                model.symbol_cache.insert(std_key(*chain_id, to), symbol);
            }
        }
        ClearShellResult::RpcAnswer {
            probe: ClearProbe::Decimals,
            chain_id,
            to,
            result,
            rpc_error: _,
        } => {
            // `if (res.result && res.result !== '0x')` — only a real result
            // word teaches the cache (`clear-signing.ts:380-384`).
            if let Some(word) = result.as_deref() {
                if word != "0x" {
                    if let Some(d) = quantity_as_small_uint(word) {
                        if d <= 36 {
                            model.decimals_cache.insert(std_key(*chain_id, to), d);
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

/// `callSupportsInterface` verdict mapping (`clear-signing.ts:188-201`):
/// revert ⇒ definitively false, result word ⇒ `== 1`, anything else ⇒ unknown.
fn interpret_supports(result: Option<&str>, rpc_error: bool) -> Option<bool> {
    if rpc_error {
        return Some(false);
    }
    let word = result?;
    if word.len() < 66 {
        return None;
    }
    let dec = if let Some(body) = word.strip_prefix("0x") {
        hex_to_dec(body)?
    } else if word.bytes().all(|b| b.is_ascii_digit()) {
        dec_normalize(word)
    } else {
        return None;
    };
    Some(dec == "1")
}

/// Only cache "not an NFT" when BOTH probes definitively said false; either
/// `true` caches that standard; any unknown caches NOTHING (invariant ②).
fn record_supports(model: &mut Model, chain_id: u32, to: &str, verdict: Option<bool>, is721: bool) {
    let key = std_key(chain_id, to);
    let entry = model.erc165_scratch.entry(key.clone()).or_default();
    if is721 {
        entry.is721 = Some(verdict);
    } else {
        entry.is1155 = Some(verdict);
    }
    if let (Some(a), Some(b)) = (entry.is721, entry.is1155) {
        if b == Some(true) {
            model
                .token_standard_cache
                .insert(key, TokenStandard::Erc1155);
        } else if a == Some(true) {
            model
                .token_standard_cache
                .insert(key, TokenStandard::Erc721);
        } else if a == Some(false) && b == Some(false) {
            model.token_standard_cache.insert(key, TokenStandard::Erc20);
        }
    }
}

fn accept(model: &mut Model, result: ClearShellResult) -> Command<ClearSigningEffect, Event> {
    let Some(mut run) = model.run.take() else {
        // A late answer for a run that already concluded (e.g. the ERC-165
        // timer firing after the pipeline finished). Its facts were absorbed.
        return Command::done();
    };
    let step = std::mem::replace(&mut run.step, Step::AwaitClock);
    match (step, result) {
        (Step::AwaitClock, ClearShellResult::Clock { now_ms }) => {
            run.now_ms = now_ms;
            match &run.req {
                Req::Tx { .. } => tx_begin(model, run),
                Req::Typed { .. } => typed_begin(model, run),
            }
        }

        (Step::AwaitContractDescriptor, ClearShellResult::DescriptorFetched { .. }) => {
            let path = contract_descriptor_path(&run.req);
            match model.descriptor_cache.get(&path).cloned().flatten() {
                Some(descriptor) => {
                    try_calldata(model, run, descriptor, true, TxNext::AfterContract)
                }
                None => tx_continue(model, run, TxNext::AfterContract),
            }
        }

        (
            Step::AwaitStandard {
                addr,
                selector,
                timer,
            },
            ClearShellResult::RpcAnswer {
                probe: probe @ (ClearProbe::SupportsErc721 | ClearProbe::SupportsErc1155),
                to,
                ..
            },
        ) => {
            let _ = probe;
            if to.to_lowercase() != addr {
                run.step = Step::AwaitStandard {
                    addr,
                    selector,
                    timer,
                };
                model.run = Some(run);
                return Command::done();
            }
            let key = std_key(run.req.chain_id(), &addr);
            if let Some(kind) = model.token_standard_cache.get(&key).copied() {
                model.erc165_scratch.remove(&key);
                return standard_decided(model, run, &selector, kind);
            }
            let both_answered = model
                .erc165_scratch
                .get(&key)
                .is_some_and(|s| s.is721.is_some() && s.is1155.is_some());
            if both_answered {
                // Both probes answered but at least one was unreachable —
                // render as ERC-20 NOW but cache nothing; re-probe next time.
                model.erc165_scratch.remove(&key);
                return standard_decided(model, run, &selector, TokenStandard::Erc20);
            }
            run.step = Step::AwaitStandard {
                addr,
                selector,
                timer,
            };
            model.run = Some(run);
            Command::done()
        }
        (
            Step::AwaitStandard {
                addr,
                selector,
                timer,
            },
            ClearShellResult::TimedOut { token },
        ) if token == timer => {
            // The 3s race lost: fall back to the common case (ERC-20) with no
            // cache write. The probes stay in flight and still teach the
            // cache when they land (`clear-signing.ts:209-239`).
            let _ = addr;
            standard_decided(model, run, &selector, TokenStandard::Erc20)
        }

        (Step::AwaitErcFallback { index }, ClearShellResult::DescriptorFetched { .. }) => {
            erc_fallback_step(model, run, index)
        }

        (Step::AwaitSelectorSigs, ClearShellResult::SelectorCandidates { sigs }) => {
            let result = best_effort_result(&run, &sigs);
            conclude(model, result)
        }

        (
            Step::AwaitWarm {
                mut pending,
                mut symbols,
                timer,
                then,
            },
            ClearShellResult::RpcAnswer {
                probe: probe @ (ClearProbe::Decimals | ClearProbe::Symbol),
                to,
                ..
            },
        ) => {
            let answered = to.to_lowercase();
            match probe {
                ClearProbe::Symbol => symbols.remove(&answered),
                _ => pending.remove(&answered),
            };
            if pending.is_empty() && symbols.is_empty() {
                warm_done(model, run, then)
            } else {
                run.step = Step::AwaitWarm {
                    pending,
                    symbols,
                    timer,
                    then,
                };
                model.run = Some(run);
                Command::done()
            }
        }
        (
            Step::AwaitWarm {
                pending,
                symbols,
                timer,
                then,
            },
            ClearShellResult::TimedOut { token },
        ) if token == timer => {
            // Never let a slow RPC stall the sheet: format with what's known
            // (18 + unverified for the rest); in-flight lookups still fill
            // the cache for next time (`clear-signing.ts:389-397`).
            let _ = (pending, symbols);
            warm_done(model, run, then)
        }

        (Step::AwaitTypedDescriptor, ClearShellResult::DescriptorFetched { .. }) => {
            typed_entry_lookup(model, run)
        }
        (Step::AwaitPermitDescriptor, ClearShellResult::DescriptorFetched { .. }) => {
            match model
                .descriptor_cache
                .get(PERMIT_FALLBACK_PATH)
                .cloned()
                .flatten()
            {
                Some(descriptor) => try_eip712(model, run, descriptor, false, TypedNext::Final),
                None => conclude(model, None),
            }
        }

        // A result for a wait that no longer expects it (stale timer from an
        // earlier phase, a probe answer after the timeout already decided).
        // It may not change flow state.
        (step, _) => {
            run.step = step;
            model.run = Some(run);
            Command::done()
        }
    }
}

// ---------------------------------------------------------------------------
// eth_sendTransaction pipeline (resolveTransaction, clear-signing.ts:300-356)
// ---------------------------------------------------------------------------

fn tx_begin(model: &mut Model, run: Run) -> Command<ClearSigningEffect, Event> {
    // 0. Built-in descriptors for top protocols — richest, zero round-trips.
    let to = match &run.req {
        Req::Tx { to: Some(to), .. } => to.clone(),
        _ => return conclude(model, None),
    };
    if let Some(local) = local_descriptor(&to) {
        return try_calldata(model, run, local, true, TxNext::AfterLocal);
    }
    tx_continue(model, run, TxNext::AfterLocal)
}

fn tx_continue(
    model: &mut Model,
    mut run: Run,
    next: TxNext,
) -> Command<ClearSigningEffect, Event> {
    match next {
        // 1. Contract-specific descriptor — wins over generic interfaces.
        TxNext::AfterLocal => {
            let path = contract_descriptor_path(&run.req);
            match model.descriptor_cache.get(&path) {
                Some(Some(descriptor)) => {
                    let descriptor = descriptor.clone();
                    try_calldata(model, run, descriptor, true, TxNext::AfterContract)
                }
                Some(None) => tx_continue(model, run, TxNext::AfterContract),
                None => {
                    run.step = Step::AwaitContractDescriptor;
                    model.run = Some(run);
                    requests(model, vec![ClearOperation::HttpGet { path }])
                }
            }
        }
        // 2. Standard token methods — MUST precede the ERC fallbacks, which
        // mis-route the shared transferFrom/approve (`clear-signing.ts:336-343`).
        TxNext::AfterContract => {
            let selector = match &run.req {
                Req::Tx { data, .. } => selector_of(data),
                Req::Typed { .. } => String::new(),
            };
            if TOKEN_STD_SELECTORS.contains(&selector.as_str()) {
                resolve_token_standard(model, run, &selector)
            } else {
                tx_continue(model, run, TxNext::AfterTokenStd)
            }
        }
        // 3. Other ERC standards (ERC-4626 vaults, etc.)
        TxNext::AfterTokenStd => erc_fallback_step(model, run, 0),
        TxNext::Final => conclude(model, None),
    }
}

fn contract_descriptor_path(req: &Req) -> String {
    match req {
        Req::Tx { to, chain_id, .. } => format!(
            "/erc7730/calldata/eip155-{chain_id}/{}.json",
            to.as_deref().unwrap_or_default()
        ),
        Req::Typed { typed, chain_id } => {
            let contract = verifying_contract(typed).unwrap_or_default();
            format!("/erc7730/eip712/eip155-{chain_id}/{contract}.json")
        }
    }
}

/// Steps 2b: resolve a standard token method against the right interface
/// descriptor, disambiguating shared selectors via ERC-165
/// (`resolveTokenStandard`, clear-signing.ts:270-290).
fn resolve_token_standard(
    model: &mut Model,
    mut run: Run,
    selector: &str,
) -> Command<ClearSigningEffect, Event> {
    let kind = match selector {
        s if s == SEL_SAFE_TRANSFER_1155 || s == SEL_SAFE_BATCH_1155 => {
            Some(TokenStandard::Erc1155)
        }
        s if s == SEL_SAFE_TRANSFER_721 || s == SEL_SAFE_TRANSFER_721_DATA => {
            Some(TokenStandard::Erc721)
        }
        s if s == SEL_TRANSFER || s == SEL_INCREASE_ALLOWANCE || s == SEL_DECREASE_ALLOWANCE => {
            Some(TokenStandard::Erc20)
        }
        _ => None,
    };
    if let Some(kind) = kind {
        let descriptor = interface_descriptor(kind);
        return try_calldata(model, run, descriptor, false, TxNext::AfterTokenStd);
    }

    // transferFrom / approve / setApprovalForAll — query the chain.
    let (to, chain_id) = match &run.req {
        Req::Tx {
            to: Some(to),
            chain_id,
            ..
        } => (to.clone(), *chain_id),
        _ => return conclude(model, None),
    };
    let key = std_key(chain_id, &to);
    if let Some(kind) = model.token_standard_cache.get(&key).copied() {
        return standard_decided(model, run, selector, kind);
    }

    model.erc165_scratch.insert(key, Erc165Scratch::default());
    model.timer_seq += 1;
    let token = model.timer_seq;
    run.step = Step::AwaitStandard {
        addr: to.clone(),
        selector: selector.to_owned(),
        timer: token,
    };
    model.run = Some(run);
    requests(
        model,
        vec![
            ClearOperation::RpcEthCall {
                chain_id,
                to: to.clone(),
                data: supports_interface_calldata(IFACE_ERC721),
                probe: ClearProbe::SupportsErc721,
            },
            ClearOperation::RpcEthCall {
                chain_id,
                to,
                data: supports_interface_calldata(IFACE_ERC1155),
                probe: ClearProbe::SupportsErc1155,
            },
            ClearOperation::Timer {
                ms: ERC165_DETECT_TIMEOUT_MS,
                token,
            },
        ],
    )
}

fn standard_decided(
    model: &mut Model,
    run: Run,
    selector: &str,
    kind: TokenStandard,
) -> Command<ClearSigningEffect, Event> {
    // setApprovalForAll is NFT-only; an "erc20" verdict for it means the
    // ERC-165 answer was useless — render the NFT shape.
    let kind = if selector == SEL_SET_APPROVAL_ALL && kind == TokenStandard::Erc20 {
        TokenStandard::Erc721
    } else {
        kind
    };
    let descriptor = interface_descriptor(kind);
    try_calldata(model, run, descriptor, false, TxNext::AfterTokenStd)
}

/// Steps 3–4: ERC fallbacks then the 4-byte selector DB (`tryErcFallbacks`
/// + `resolveBySelector`).
fn erc_fallback_step(
    model: &mut Model,
    mut run: Run,
    start_index: usize,
) -> Command<ClearSigningEffect, Event> {
    let data = match &run.req {
        Req::Tx { data, .. } => data.clone(),
        Req::Typed { .. } => return conclude(model, None),
    };
    let data_bytes = primitives::from_hex(&data).unwrap_or_default();

    let mut index = start_index;
    while index < ERC_CALLDATA_FALLBACKS.len() {
        let path = ERC_CALLDATA_FALLBACKS[index];
        match model.descriptor_cache.get(path) {
            Some(Some(descriptor)) => {
                let formats_match = descriptor
                    .get("display")
                    .and_then(|d| d.get("formats"))
                    .and_then(Value::as_object)
                    .is_some_and(|formats| {
                        formats
                            .keys()
                            .any(|sig| abi::match_selector(sig, &data_bytes).unwrap_or(false))
                    });
                if formats_match {
                    let descriptor = descriptor.clone();
                    return try_calldata(model, run, descriptor, false, TxNext::Final);
                }
                index += 1;
            }
            Some(None) => index += 1,
            None => {
                run.step = Step::AwaitErcFallback { index };
                model.run = Some(run);
                return requests(
                    model,
                    vec![ClearOperation::HttpGet {
                        path: path.to_owned(),
                    }],
                );
            }
        }
    }

    // 4. No descriptor anywhere — DON'T blind-sign lazily; recover from the
    // selector database and decode generically (`clear-signing.ts:351-355`).
    run.step = Step::AwaitSelectorSigs;
    let selector = selector_of(&data);
    model.run = Some(run);
    requests(model, vec![ClearOperation::SelectorDbLookup { selector }])
}

// ---------------------------------------------------------------------------
// eth_signTypedData pipeline (resolveTypedData, clear-signing.ts:621-654)
// ---------------------------------------------------------------------------

fn typed_begin(model: &mut Model, mut run: Run) -> Command<ClearSigningEffect, Event> {
    let path = contract_descriptor_path(&run.req);
    match model.descriptor_cache.get(&path) {
        Some(_) => typed_entry_lookup(model, run),
        None => {
            run.step = Step::AwaitTypedDescriptor;
            model.run = Some(run);
            requests(model, vec![ClearOperation::HttpGet { path }])
        }
    }
}

/// EIP-712 descriptors are keyed by the primary type's `encodeType` hash;
/// the legacy `toHex` had no 0x prefix, so both keys are tried
/// (`clear-signing.ts:636-641`).
fn typed_entry_lookup(model: &mut Model, run: Run) -> Command<ClearSigningEffect, Event> {
    let Req::Typed { typed, .. } = &run.req else {
        return conclude(model, None);
    };
    let path = contract_descriptor_path(&run.req);
    let descriptor = model.descriptor_cache.get(&path).cloned().flatten();

    let entry = descriptor.and_then(|descriptor| {
        let type_hash = typed_data_type_hash(typed)?;
        let bare = descriptor.get(&type_hash).cloned();
        let prefixed = descriptor.get(format!("0x{type_hash}")).cloned();
        bare.or(prefixed).filter(value_is_truthy)
    });

    match entry {
        Some(entry) => try_eip712(model, run, entry, true, TypedNext::AfterEntry),
        None => typed_continue(model, run, TypedNext::AfterEntry),
    }
}

fn typed_continue(
    model: &mut Model,
    mut run: Run,
    next: TypedNext,
) -> Command<ClearSigningEffect, Event> {
    match next {
        TypedNext::AfterEntry => match model.descriptor_cache.get(PERMIT_FALLBACK_PATH) {
            Some(Some(descriptor)) => {
                let descriptor = descriptor.clone();
                try_eip712(model, run, descriptor, false, TypedNext::Final)
            }
            Some(None) => conclude(model, None),
            None => {
                run.step = Step::AwaitPermitDescriptor;
                model.run = Some(run);
                requests(
                    model,
                    vec![ClearOperation::HttpGet {
                        path: PERMIT_FALLBACK_PATH.to_owned(),
                    }],
                )
            }
        },
        TypedNext::Final => conclude(model, None),
    }
}

fn typed_data_type_hash(typed: &Value) -> Option<String> {
    let primary = typed.get("primaryType").and_then(Value::as_str)?;
    let types = typed.get("types")?;
    let encode_type = build_encode_type(primary, types);
    let hash = primitives::keccak256(encode_type.as_bytes());
    Some(primitives::to_hex(&hash, false))
}

/// `buildEncodeType` (clear-signing.ts:769-794): primary type first, then
/// dependencies alphabetically, each as `Name(type name,…)`.
fn build_encode_type(primary: &str, types: &Value) -> String {
    let mut deps: BTreeSet<String> = BTreeSet::new();
    collect_deps(primary, types, &mut deps);
    deps.remove(primary);
    let mut ordered: Vec<&str> = vec![primary];
    ordered.extend(deps.iter().map(String::as_str));
    ordered
        .iter()
        .map(|t| {
            let Some(fields) = types.get(*t).and_then(Value::as_array) else {
                return String::new();
            };
            let inner = fields
                .iter()
                .map(|f| {
                    format!(
                        "{} {}",
                        f.get("type").and_then(Value::as_str).unwrap_or_default(),
                        f.get("name").and_then(Value::as_str).unwrap_or_default()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!("{t}({inner})")
        })
        .collect::<Vec<_>>()
        .join("")
}

fn collect_deps(ty: &str, types: &Value, deps: &mut BTreeSet<String>) {
    if deps.contains(ty) {
        return;
    }
    let Some(fields) = types.get(ty).and_then(Value::as_array) else {
        return;
    };
    deps.insert(ty.to_owned());
    for f in fields {
        let Some(field_ty) = f.get("type").and_then(Value::as_str) else {
            continue;
        };
        let base = strip_array_suffix(field_ty);
        if types.get(base).is_some() {
            collect_deps(base, types, deps);
        }
    }
}

/// The `/\[\d*\]$/` strip — one trailing array suffix.
fn strip_array_suffix(ty: &str) -> &str {
    if let Some(open) = ty.rfind('[') {
        if ty.ends_with(']')
            && ty[open + 1..ty.len() - 1]
                .bytes()
                .all(|b| b.is_ascii_digit())
        {
            return &ty[..open];
        }
    }
    ty
}

// ---------------------------------------------------------------------------
// Descriptor resolution (shared by every pipeline arm)
// ---------------------------------------------------------------------------

/// Match + decode, then either warm unknown decimals (async) or finish
/// (pure). A null outcome falls through to `next`.
fn try_calldata(
    model: &mut Model,
    run: Run,
    descriptor: Value,
    is_specific: bool,
    next: TxNext,
) -> Command<ClearSigningEffect, Event> {
    let Req::Tx { data, .. } = &run.req else {
        return conclude(model, None);
    };
    let data_bytes = primitives::from_hex(data).unwrap_or_default();
    let Some(matched_sig) = match_descriptor_sig(&descriptor, &data_bytes) else {
        return tx_continue(model, run, next);
    };
    if abi::decode_calldata(&matched_sig, &data_bytes).is_err() {
        return tx_continue(model, run, next);
    }

    let ctx = calldata_context(&run.req, &matched_sig);
    let unknown = unknown_token_addrs(model, &run, &descriptor, &matched_sig, &ctx);
    if unknown.is_empty() {
        let outcome = finish_calldata(model, &run, &descriptor, &matched_sig, is_specific);
        return match outcome {
            Some(result) => conclude(model, Some(result)),
            None => tx_continue(model, run, next),
        };
    }
    let then = WarmThen::Calldata {
        descriptor,
        matched_sig,
        is_specific,
        next,
    };
    begin_warm(model, run, unknown, then)
}

fn try_eip712(
    model: &mut Model,
    run: Run,
    descriptor: Value,
    contract_specific: bool,
    next: TypedNext,
) -> Command<ClearSigningEffect, Event> {
    let Req::Typed { typed, .. } = &run.req else {
        return conclude(model, None);
    };
    let Some(primary) = typed.get("primaryType").and_then(Value::as_str) else {
        return typed_continue(model, run, next);
    };
    let matched_sig = descriptor
        .get("display")
        .and_then(|d| d.get("formats"))
        .and_then(Value::as_object)
        .and_then(|formats| {
            formats
                .keys()
                .find(|sig| sig.starts_with(&format!("{primary}(")))
                .cloned()
        });
    let Some(matched_sig) = matched_sig else {
        return typed_continue(model, run, next);
    };

    let ctx = eip712_context(typed);
    let unknown = unknown_token_addrs(model, &run, &descriptor, &matched_sig, &ctx);
    if unknown.is_empty() {
        let outcome = finish_eip712(model, &run, &descriptor, &matched_sig, contract_specific);
        return match outcome {
            Some(result) => conclude(model, Some(result)),
            None => typed_continue(model, run, next),
        };
    }
    let then = WarmThen::Eip712 {
        descriptor,
        matched_sig,
        contract_specific,
        next,
    };
    begin_warm(model, run, unknown, then)
}

fn begin_warm(
    model: &mut Model,
    mut run: Run,
    pending: BTreeSet<String>,
    then: WarmThen,
) -> Command<ClearSigningEffect, Event> {
    model.timer_seq += 1;
    let token = model.timer_seq;
    let chain_id = run.req.chain_id();
    let mut ops: Vec<ClearOperation> = pending
        .iter()
        .flat_map(|addr| {
            [
                ClearOperation::RpcEthCall {
                    chain_id,
                    to: addr.clone(),
                    data: ERC20_DECIMALS_SELECTOR.to_owned(),
                    probe: ClearProbe::Decimals,
                },
                // Gating, like decimals — phase 23 shipped this probe as
                // non-gating and running it showed why that could not work:
                // the two ride ONE round trip and finish milliseconds apart
                // (622ms and 642ms against Gnosis), so whichever loses the
                // coin flip is only in the cache, and the sheet formats
                // without it. Non-gating did not mean "sometimes late"; it
                // meant "almost never shown the first time a token is seen".
                //
                // What it costs: a token that answers decimals and hangs on
                // symbol now waits — but only until the same 4s timer that
                // already bounds this step, which then formats with what is
                // known. The floor is unchanged; only the common case moved.
                ClearOperation::RpcEthCall {
                    chain_id,
                    to: addr.clone(),
                    data: ERC20_SYMBOL_SELECTOR.to_owned(),
                    probe: ClearProbe::Symbol,
                },
            ]
        })
        .collect();
    ops.push(ClearOperation::Timer {
        ms: DECIMALS_WARM_TIMEOUT_MS,
        token,
    });
    run.step = Step::AwaitWarm {
        symbols: pending.clone(),
        pending,
        timer: token,
        then,
    };
    model.run = Some(run);
    requests(model, ops)
}

fn warm_done(model: &mut Model, run: Run, then: WarmThen) -> Command<ClearSigningEffect, Event> {
    match then {
        WarmThen::Calldata {
            descriptor,
            matched_sig,
            is_specific,
            next,
        } => {
            let outcome = finish_calldata(model, &run, &descriptor, &matched_sig, is_specific);
            match outcome {
                Some(result) => conclude(model, Some(result)),
                None => tx_continue(model, run, next),
            }
        }
        WarmThen::Eip712 {
            descriptor,
            matched_sig,
            contract_specific,
            next,
        } => {
            let outcome = finish_eip712(model, &run, &descriptor, &matched_sig, contract_specific);
            match outcome {
                Some(result) => conclude(model, Some(result)),
                None => typed_continue(model, run, next),
            }
        }
    }
}

fn conclude(
    model: &mut Model,
    result: Option<ClearSignResult>,
) -> Command<ClearSigningEffect, Event> {
    model.result = result;
    model.resolved = true;
    model.run = None;
    render()
}

/// First format signature whose selector matches the calldata
/// (`matchSelector`, abi-decode.ts:187-196).
fn match_descriptor_sig(descriptor: &Value, data_bytes: &[u8]) -> Option<String> {
    descriptor
        .get("display")
        .and_then(|d| d.get("formats"))
        .and_then(Value::as_object)?
        .keys()
        .find(|sig| abi::match_selector(sig, data_bytes).unwrap_or(false))
        .cloned()
}

/// `resolveCalldataDescriptor` minus the warm (already done): fields, the
/// 0-fields blind rule, partial, roles, risk (`clear-signing.ts:547-612`).
fn finish_calldata(
    model: &Model,
    run: &Run,
    descriptor: &Value,
    matched_sig: &str,
    is_specific: bool,
) -> Option<ClearSignResult> {
    let Req::Tx { to, .. } = &run.req else {
        return None;
    };
    let ctx = calldata_context(&run.req, matched_sig);
    let format = descriptor
        .get("display")?
        .get("formats")?
        .get(matched_sig)?;
    let field_defs = format
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let metadata = descriptor.get("metadata").cloned().unwrap_or(Value::Null);
    let definitions = descriptor
        .get("display")
        .and_then(|d| d.get("definitions"))
        .cloned()
        .unwrap_or(Value::Null);

    let fields = resolve_fields(model, run, &field_defs, &ctx, &metadata, &definitions);
    let declared_visible = declared_visible_count(&field_defs);
    if fields.is_empty() {
        // If NOTHING decoded there is nothing trustworthy to show → blind
        // sign, never a half-truth (`clear-signing.ts:587-590`).
        return None;
    }
    let partial = declared_visible > 0 && fields.len() < declared_visible.div_ceil(2);

    let intent = format
        .get("intent")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| {
            matched_sig
                .split('(')
                .next()
                .unwrap_or(matched_sig)
                .to_owned()
        });
    let fields = infer_field_roles(fields, &intent);
    let risk = assess_risk(&intent, &fields, partial);

    Some(ClearSignResult {
        contract_name: if is_specific {
            metadata
                .get("contractName")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    descriptor
                        .get("context")
                        .and_then(|c| c.get("$id"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
        } else {
            None
        },
        owner: if is_specific {
            metadata
                .get("owner")
                .and_then(Value::as_str)
                .map(str::to_owned)
        } else {
            None
        },
        risk,
        intent,
        fields,
        contract_address: to.clone(),
        verified: is_specific,
        sign_type: ClearSignType::Transaction,
        partial,
        best_effort: false,
        // Filled by `to_own_token` in `view()` — the burn verdict is a
        // projection over the finished fields, never a builder's business.
        to_own_token: false,
    })
}

/// `resolveEip712Entry` / `resolveEip712Formats` post-warm halves
/// (`clear-signing.ts:656-763`). `contract_specific` selects the entry-path
/// naming + `verified: true`.
fn finish_eip712(
    model: &Model,
    run: &Run,
    descriptor: &Value,
    matched_sig: &str,
    contract_specific: bool,
) -> Option<ClearSignResult> {
    let Req::Typed { typed, .. } = &run.req else {
        return None;
    };
    let ctx = eip712_context(typed);
    let format = descriptor
        .get("display")?
        .get("formats")?
        .get(matched_sig)?;
    let field_defs = format
        .get("fields")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let metadata = descriptor.get("metadata").cloned().unwrap_or(Value::Null);
    let definitions = descriptor
        .get("display")
        .and_then(|d| d.get("definitions"))
        .cloned()
        .unwrap_or(Value::Null);

    let fields = resolve_fields(model, run, &field_defs, &ctx, &metadata, &definitions);
    let declared_visible = declared_visible_count(&field_defs);
    if fields.is_empty() {
        return None;
    }
    let partial = declared_visible > 0 && fields.len() < declared_visible.div_ceil(2);

    let primary = typed
        .get("primaryType")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let intent = format
        .get("intent")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .unwrap_or_else(|| primary.to_owned());
    let fields = infer_field_roles(fields, &intent);
    let risk = assess_risk(&intent, &fields, partial);

    Some(ClearSignResult {
        contract_name: if contract_specific {
            metadata
                .get("contractName")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .or_else(|| {
                    descriptor
                        .get("context")
                        .and_then(|c| c.get("eip712"))
                        .and_then(|e| e.get("domain"))
                        .and_then(|d| d.get("name"))
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                })
        } else {
            None
        },
        owner: if contract_specific {
            metadata
                .get("owner")
                .and_then(Value::as_str)
                .map(str::to_owned)
        } else {
            None
        },
        risk,
        intent,
        fields,
        contract_address: verifying_contract(typed),
        verified: contract_specific,
        sign_type: ClearSignType::Signature,
        partial,
        best_effort: false,
        // Filled by `to_own_token` in `view()` — the burn verdict is a
        // projection over the finished fields, never a builder's business.
        to_own_token: false,
    })
}

fn verifying_contract(typed: &Value) -> Option<String> {
    typed
        .get("domain")
        .and_then(|d| d.get("verifyingContract"))
        .and_then(Value::as_str)
        .map(str::to_lowercase)
}

/// `visible !== 'never' && f.label` over the RAW field defs — a `$ref`
/// field whose label lives only in the definition intentionally doesn't
/// count, exactly as the TS filter reads (`clear-signing.ts:584-586`).
fn declared_visible_count(field_defs: &[Value]) -> usize {
    field_defs
        .iter()
        .filter(|f| {
            f.get("visible").and_then(Value::as_str) != Some("never")
                && f.get("label")
                    .and_then(Value::as_str)
                    .is_some_and(|l| !l.is_empty())
        })
        .count()
}

// ---------------------------------------------------------------------------
// Best-effort decode (resolveBySelector, clear-signing.ts:450-545)
// ---------------------------------------------------------------------------

fn best_effort_result(run: &Run, sigs: &[String]) -> Option<ClearSignResult> {
    let Req::Tx { to, data, .. } = &run.req else {
        return None;
    };
    let to = to.clone()?;
    let data_bytes = primitives::from_hex(data).ok()?;
    for sig in sigs {
        let Ok(tree) = abi::decode_calldata(sig, &data_bytes) else {
            continue;
        };
        let fn_name = sig.split('(').next().unwrap_or(sig);
        let known = known_contract(&to);
        return Some(ClearSignResult {
            intent: humanize_fn_name(fn_name),
            contract_name: known.map(|(name, _)| name.to_owned()),
            owner: known.map(|(_, owner)| owner.to_owned()),
            fields: build_best_effort_fields(&tree, &run.locale),
            // Decoded but unverified — never reads as safe.
            risk: ClearRisk::Caution,
            contract_address: Some(to),
            verified: false,
            sign_type: ClearSignType::Transaction,
            partial: false,
            best_effort: true,
            // Filled by `to_own_token` in `view()`.
            to_own_token: false,
        });
    }
    None
}

/// "swapExactTokensForTokens" → "Swap exact tokens for tokens".
fn humanize_fn_name(name: &str) -> String {
    let mut spaced = String::with_capacity(name.len() + 4);
    let mut prev_lower_digit = false;
    for c in name.chars() {
        if prev_lower_digit && c.is_ascii_uppercase() {
            spaced.push(' ');
        }
        prev_lower_digit = c.is_ascii_lowercase() || c.is_ascii_digit();
        spaced.push(if c == '_' { ' ' } else { c });
    }
    let spaced = spaced.trim();
    let mut chars = spaced.chars();
    match chars.next() {
        Some(first) => format!("{}{}", first.to_uppercase(), chars.as_str().to_lowercase()),
        None => name.to_owned(),
    }
}

fn build_best_effort_fields(tree: &abi::AbiValue, locale: &ClearLocale) -> Vec<ClearSignField> {
    tree.children
        .iter()
        .map(|child| {
            let ctx = ctx_from_abi(child);
            // A full 0x address so the detail panel can resolve identity —
            // exact-type "address" only, as the TS regex reads.
            let address = if child.kind == "address" {
                match &ctx {
                    Ctx::Str(s) if is_hex_address_shape(s) => Some(s.clone()),
                    _ => None,
                }
            } else {
                None
            };
            ClearSignField {
                label: if child.name.is_empty() {
                    pretty_type(&child.kind)
                } else {
                    child.name.clone()
                },
                value: format_generic_value(&ctx, &child.kind, locale),
                format: "raw".to_owned(),
                token_address: None,
                warning: false,
                unverified: false,
                role: ClearFieldRole::Generic,
                // Unverified guesses stay out of the headline body.
                detail: true,
                expired: false,
                address,
                usd_value: None,
            }
        })
        .collect()
}

fn pretty_type(ty: &str) -> String {
    if ty.starts_with("address") {
        return if ty.ends_with("[]") {
            "Addresses"
        } else {
            "Address"
        }
        .to_owned();
    }
    // A bare integer in a best-effort decode is NOT necessarily an amount
    // (deadline, min-out, index…) — the neutral "Value", never "Amount".
    if ty.starts_with("uint") || ty.starts_with("int") {
        return "Value".to_owned();
    }
    match ty {
        "bool" => "Flag".to_owned(),
        "string" => "Text".to_owned(),
        t if t.starts_with("bytes") => "Data".to_owned(),
        t => t.to_owned(),
    }
}

fn short_hex(s: &str) -> String {
    if s.chars().count() > 22 {
        format!("{}…{}", take_chars(s, 0, 10), last_chars(s, 6))
    } else {
        s.to_owned()
    }
}

fn format_generic_value(v: &Ctx, ty: &str, locale: &ClearLocale) -> String {
    match v {
        Ctx::Arr(items) => {
            let elem_ty = ty.strip_suffix("[]").unwrap_or(ty);
            let shown: Vec<String> = items
                .iter()
                .take(4)
                .map(|x| format_generic_value(x, elem_ty, locale))
                .collect();
            let extra = if items.len() > 4 {
                format!(", +{}", items.len() - 4)
            } else {
                String::new()
            };
            format!("[{}{extra}]", shown.join(", "))
        }
        Ctx::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
        Ctx::Num(d) => group_digits(d, locale),
        other => {
            let s = js_string(other);
            if is_hex_address_shape(&s) {
                format!("{}…{}", take_chars(&s, 0, 8), last_chars(&s, 6))
            } else {
                short_hex(&s)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Deploy detection (buildDeployResult, clear-signing.ts:129-161)
// ---------------------------------------------------------------------------

fn build_deploy_result(to: Option<&str>, data: &str) -> ClearSignResult {
    let mut predicted: Option<String> = None;
    if let Some(to) = to {
        if CREATE2_DEPLOYERS.contains(&to.to_lowercase().as_str()) {
            let hex = data.strip_prefix("0x").unwrap_or(data);
            if hex.len() >= 64 {
                let salt = primitives::from_hex(&hex[..64]).ok();
                let init_code = primitives::from_hex(&hex[64..]).ok();
                if let (Some(salt), Some(init_code)) = (salt, init_code) {
                    predicted =
                        primitives::create2_address(to, &salt, &primitives::keccak256(&init_code))
                            .ok();
                }
            }
        }
    }

    let fields = predicted
        .map(|p| {
            vec![ClearSignField {
                label: "New contract".to_owned(),
                value: format!("{}...{}", take_chars(&p, 0, 8), last_chars(&p, 6)),
                format: "addressName".to_owned(),
                token_address: None,
                warning: false,
                unverified: false,
                role: ClearFieldRole::Generic,
                detail: false,
                expired: false,
                address: None,
                usd_value: None,
            }]
        })
        .unwrap_or_default();

    ClearSignResult {
        intent: "Deploy contract".to_owned(),
        contract_name: to.map(|_| "CREATE2 Deployer".to_owned()),
        owner: None,
        fields,
        risk: ClearRisk::Normal,
        contract_address: to.map(str::to_lowercase),
        verified: false,
        sign_type: ClearSignType::Transaction,
        partial: false,
        best_effort: false,
        // Filled by `to_own_token` in `view()` — the burn verdict is a
        // projection over the finished fields, never a builder's business.
        to_own_token: false,
    }
}

// ---------------------------------------------------------------------------
// Context trees + path resolution (resolvePath, clear-signing.ts:839-898)
// ---------------------------------------------------------------------------

/// Decoded/typed values in the legacy `DecodedValue` shape: `Num` is a JS
/// bigint (decimal string), `JsNum` a JSON number — `toBigInt` treats a JS
/// number as 0n, a ported quirk (`clear-signing.ts:1160-1171` falls through).
#[derive(Clone, Debug)]
enum Ctx {
    Null,
    Bool(bool),
    Num(String),
    JsNum(f64),
    Str(String),
    Arr(Vec<Ctx>),
    Map(Vec<(String, Ctx)>),
}

impl Ctx {
    fn get(&self, key: &str) -> Option<&Ctx> {
        match self {
            Ctx::Map(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }
}

/// `nodeToLegacy` (services/vela-core/convert.ts): addresses lowercase,
/// integers as bigints, tuples keyed by name or `_i`.
fn ctx_from_abi(node: &abi::AbiValue) -> Ctx {
    if node.kind == "tuple" {
        return Ctx::Map(
            node.children
                .iter()
                .enumerate()
                .map(|(i, child)| {
                    let key = if child.name.is_empty() {
                        format!("_{i}")
                    } else {
                        child.name.clone()
                    };
                    (key, ctx_from_abi(child))
                })
                .collect(),
        );
    }
    if node.kind.ends_with(']') {
        return Ctx::Arr(node.children.iter().map(ctx_from_abi).collect());
    }
    if node.kind == "address" {
        return Ctx::Str(node.value.to_lowercase());
    }
    if node.kind == "bool" {
        return Ctx::Bool(node.value == "true");
    }
    if node.kind.starts_with("uint") || node.kind.starts_with("int") {
        let (sign, body) = match node.value.strip_prefix('-') {
            Some(rest) => ("-", rest),
            None => ("", node.value.as_str()),
        };
        let dec = body
            .strip_prefix("0x")
            .and_then(hex_to_dec)
            .unwrap_or_else(|| "0".to_owned());
        return Ctx::Num(if dec == "0" {
            dec
        } else {
            format!("{sign}{dec}")
        });
    }
    Ctx::Str(node.value.clone())
}

fn ctx_from_json(v: &Value) -> Ctx {
    match v {
        Value::Null => Ctx::Null,
        Value::Bool(b) => Ctx::Bool(*b),
        Value::Number(n) => Ctx::JsNum(n.as_f64().unwrap_or(0.0)),
        Value::String(s) => Ctx::Str(s.clone()),
        Value::Array(items) => Ctx::Arr(items.iter().map(ctx_from_json).collect()),
        Value::Object(map) => Ctx::Map(
            map.iter()
                .map(|(k, v)| (k.clone(), ctx_from_json(v)))
                .collect(),
        ),
    }
}

fn calldata_context(req: &Req, matched_sig: &str) -> Ctx {
    let Req::Tx {
        to, data, value, ..
    } = req
    else {
        return Ctx::Null;
    };
    let data_bytes = primitives::from_hex(data).unwrap_or_default();
    let mut entries = match abi::decode_calldata(matched_sig, &data_bytes).map(|t| ctx_from_abi(&t))
    {
        Ok(Ctx::Map(entries)) => entries,
        _ => Vec::new(),
    };
    entries.push((
        "@".to_owned(),
        Ctx::Map(vec![
            ("to".to_owned(), Ctx::Str(to.clone().unwrap_or_default())),
            (
                "value".to_owned(),
                Ctx::Str(value.clone().unwrap_or_else(|| "0x0".to_owned())),
            ),
            ("from".to_owned(), Ctx::Str(String::new())),
        ]),
    ));
    Ctx::Map(entries)
}

/// EIP-712 has no tx-level `@`, but ERC-2612 descriptors reference the token
/// via `@.to` — bind the verifying contract so permit amounts scale right
/// (`clear-signing.ts:670-674`).
fn eip712_context(typed: &Value) -> Ctx {
    let vc = typed
        .get("domain")
        .and_then(|d| d.get("verifyingContract"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut entries = match typed.get("message").map(ctx_from_json) {
        Some(Ctx::Map(entries)) => entries,
        _ => Vec::new(),
    };
    entries.push((
        "@".to_owned(),
        Ctx::Map(vec![
            ("to".to_owned(), Ctx::Str(vc.clone())),
            ("verifyingContract".to_owned(), Ctx::Str(vc)),
            ("from".to_owned(), Ctx::Str(String::new())),
        ]),
    ));
    Ctx::Map(entries)
}

fn resolve_path(path: &str, ctx: &Ctx) -> Option<Ctx> {
    if path.is_empty() {
        return None;
    }
    if let Some(key) = path.strip_prefix("@.") {
        return ctx.get("@").and_then(|at| at.get(key)).cloned();
    }
    if path.starts_with("$.") {
        return None; // metadata refs resolve in formatField
    }

    let mut current = ctx.clone();
    for part in path.split('.') {
        if matches!(current, Ctx::Null) {
            return None;
        }
        // Slice notation like "[-20:]" or "[0:20]" over 0x-hex bytes.
        if part.contains('[') && part.contains(':') {
            let base = part.split('[').next().unwrap_or_default();
            if !base.is_empty() {
                current = current.get(base).cloned()?;
            }
            if let Some((start, end)) = parse_slice(part) {
                if let Ctx::Str(s) = &current {
                    let body = s.strip_prefix("0x").unwrap_or(s);
                    let sliced = if start < 0 {
                        js_slice(body, start * 2, None)
                    } else {
                        js_slice(
                            body,
                            start * 2,
                            Some(end.unwrap_or((body.len() / 2) as i64) * 2),
                        )
                    };
                    current = Ctx::Str(format!("0x{sliced}"));
                }
            }
            continue;
        }
        // Array iteration "[]" — joins and RETURNS, as the TS does.
        if part == "[]" {
            if let Ctx::Arr(items) = &current {
                let joined = items.iter().map(js_string).collect::<Vec<_>>().join(", ");
                return Some(Ctx::Str(joined));
            }
            continue;
        }
        // Array index, incl. negative ("path.0" / "path.-1").
        if let Ctx::Arr(items) = &current {
            if is_int_shape(part) {
                let n: i64 = part.parse().ok()?;
                let idx = if n < 0 {
                    let from_end = items.len() as i64 + n;
                    if from_end < 0 {
                        return None;
                    }
                    from_end as usize
                } else {
                    n as usize
                };
                current = items.get(idx).cloned()?;
                continue;
            }
        }
        current = current.get(part).cloned()?;
    }
    Some(current)
}

fn is_int_shape(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit())
}

fn parse_slice(part: &str) -> Option<(i64, Option<i64>)> {
    let open = part.find('[')?;
    let close = part.rfind(']')?;
    let body = part.get(open + 1..close)?;
    let (a, b) = body.split_once(':')?;
    let start = if a.is_empty() { 0 } else { a.parse().ok()? };
    let end = if b.is_empty() {
        None
    } else {
        Some(b.parse().ok()?)
    };
    Some((start, end))
}

/// JS `String.prototype.slice` over ASCII, with negative indices + clamping.
fn js_slice(s: &str, start: i64, end: Option<i64>) -> String {
    let len = s.len() as i64;
    let norm = |i: i64| -> i64 {
        if i < 0 {
            (len + i).max(0)
        } else {
            i.min(len)
        }
    };
    let from = norm(start);
    let to = norm(end.unwrap_or(len));
    if from >= to {
        return String::new();
    }
    s.get(from as usize..to as usize)
        .unwrap_or_default()
        .to_owned()
}

fn resolve_metadata_ref(path: &str, metadata: &Value) -> Value {
    if path.is_empty() || metadata.is_null() {
        return Value::Null;
    }
    let stripped = path.strip_prefix("$.metadata.").unwrap_or(path);
    let mut current = metadata;
    for part in stripped.split('.') {
        match current.get(part) {
            Some(v) => current = v,
            None => return Value::Null,
        }
    }
    current.clone()
}

fn js_string(v: &Ctx) -> String {
    match v {
        Ctx::Null => "null".to_owned(),
        Ctx::Bool(b) => if *b { "true" } else { "false" }.to_owned(),
        Ctx::Num(d) => d.clone(),
        Ctx::JsNum(f) => {
            if f.is_finite() {
                let mut buffer = ryu_js::Buffer::new();
                buffer.format(*f).to_owned()
            } else if f.is_nan() {
                "NaN".to_owned()
            } else if *f > 0.0 {
                "Infinity".to_owned()
            } else {
                "-Infinity".to_owned()
            }
        }
        Ctx::Str(s) => s.clone(),
        Ctx::Arr(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Ctx::Map(_) => "[object Object]".to_owned(),
    }
}

// ---------------------------------------------------------------------------
// Field resolution + formatting (clear-signing.ts:800-1154)
// ---------------------------------------------------------------------------

struct Formatted {
    value: String,
    format: String,
    token_address: Option<String>,
    warning: bool,
    unverified: bool,
    expired: bool,
    address: Option<String>,
    usd_value: Option<f64>,
}

impl Formatted {
    fn plain(value: String, format: &str) -> Formatted {
        Formatted {
            value,
            format: format.to_owned(),
            token_address: None,
            warning: false,
            unverified: false,
            expired: false,
            address: None,
            usd_value: None,
        }
    }
}

/// `$ref` merge: `{...definitions[refPath], ...fd}` — the field def wins.
fn merged_def(fd: &Value, definitions: &Value) -> Value {
    let Some(ref_path) = fd.get("$ref").and_then(Value::as_str) else {
        return fd.clone();
    };
    if definitions.is_null() {
        return fd.clone();
    }
    let ref_key = ref_path
        .strip_prefix("$.display.definitions.")
        .unwrap_or(ref_path);
    let mut base = definitions.get(ref_key).cloned().unwrap_or(Value::Null);
    if let (Some(base_map), Some(fd_map)) = (base.as_object_mut(), fd.as_object()) {
        for (k, v) in fd_map {
            if k != "$ref" {
                base_map.insert(k.clone(), v.clone());
            }
        }
        return base;
    }
    fd.clone()
}

fn resolve_fields(
    model: &Model,
    run: &Run,
    field_defs: &[Value],
    ctx: &Ctx,
    metadata: &Value,
    definitions: &Value,
) -> Vec<ClearSignField> {
    let mut fields = Vec::new();
    for fd in field_defs {
        let def = merged_def(fd, definitions);
        if def.get("visible").and_then(Value::as_str) == Some("never") {
            continue;
        }
        let label = def
            .get("label")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let format = def
            .get("format")
            .and_then(Value::as_str)
            .unwrap_or("raw")
            .to_owned();
        let raw = def
            .get("path")
            .and_then(Value::as_str)
            .and_then(|p| resolve_path(p, ctx));
        let params = def.get("params").cloned().unwrap_or(Value::Null);

        let Some(formatted) =
            format_field(model, run, raw.as_ref(), &format, &params, ctx, metadata)
        else {
            continue;
        };
        fields.push(ClearSignField {
            label,
            value: formatted.value,
            format: formatted.format,
            token_address: formatted.token_address,
            warning: formatted.warning,
            unverified: formatted.unverified,
            role: ClearFieldRole::Generic, // assigned by infer_field_roles
            detail: false,
            expired: formatted.expired,
            address: formatted.address,
            usd_value: formatted.usd_value,
        });
    }
    fields
}

fn format_field(
    model: &Model,
    run: &Run,
    raw: Option<&Ctx>,
    format: &str,
    params: &Value,
    ctx: &Ctx,
    metadata: &Value,
) -> Option<Formatted> {
    if raw.is_none() && format != "amount" {
        return None;
    }
    match format {
        "tokenAmount" => format_token_amount(model, run, raw, params, ctx, metadata),
        "addressName" => format_address(raw),
        "amount" => format_native_amount(run, raw),
        "raw" => format_raw(raw),
        "date" => format_date_field(run, raw),
        "duration" => format_duration(raw),
        "enum" => format_enum(raw, params, metadata),
        "nftName" => format_nft_name(raw, &run.locale),
        "unit" => format_unit(raw, params, &run.locale),
        "calldata" => {
            let s = raw.map(js_string).unwrap_or_default();
            Some(Formatted::plain(truncate_hex(&s), "raw"))
        }
        other => {
            let s = raw.map(js_string).unwrap_or_default();
            Some(Formatted::plain(s, other))
        }
    }
}

fn format_token_amount(
    model: &Model,
    run: &Run,
    raw: Option<&Ctx>,
    params: &Value,
    ctx: &Ctx,
    metadata: &Value,
) -> Option<Formatted> {
    let amount = to_bigint(raw);

    // Threshold for unlimited approvals — checked FIRST, before any token
    // identity resolution, exactly as the TS does.
    if let Some(threshold) = params.get("threshold").and_then(Value::as_str) {
        let threshold_dec = threshold
            .strip_prefix("0x")
            .and_then(hex_to_dec)
            .or_else(|| {
                threshold
                    .bytes()
                    .all(|b| b.is_ascii_digit())
                    .then(|| dec_normalize(threshold))
            });
        if let Some(threshold_dec) = threshold_dec {
            if dec_ge(&amount, &threshold_dec) {
                let message = params
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("Unlimited");
                return Some(Formatted {
                    warning: true,
                    ..Formatted::plain(message.to_owned(), "tokenAmount")
                });
            }
        }
    }

    // Token address: path → metadata ref → the EIP-712 verifying contract.
    let mut token_addr: Option<String> = None;
    if let Some(token_path) = params.get("tokenPath").and_then(Value::as_str) {
        if let Some(Ctx::Str(s)) = resolve_path(token_path, ctx) {
            token_addr = Some(s);
        }
    }
    if let Some(token_ref) = params.get("token").and_then(Value::as_str) {
        if let Value::String(s) = resolve_metadata_ref(token_ref, metadata) {
            token_addr = Some(s);
        }
    }
    if token_addr.is_none() {
        if let Some(Ctx::Str(vc)) = ctx
            .get("@")
            .and_then(|at| at.get("verifyingContract"))
            .cloned()
        {
            if is_hex_address_shape(&vc) {
                token_addr = Some(vc);
            }
        }
    }

    // Native-currency sentinel addresses drop the token reference.
    if let Some(native_refs) = params
        .get("nativeCurrencyAddress")
        .and_then(Value::as_array)
    {
        for ref_path in native_refs {
            let Some(ref_path) = ref_path.as_str() else {
                continue;
            };
            let addr = resolve_metadata_ref(ref_path, metadata);
            if addr.is_null() {
                continue;
            }
            let addr_s = match &addr {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            if let Some(t) = &token_addr {
                if t.to_lowercase() == addr_s.to_lowercase() {
                    token_addr = None;
                    break;
                }
            }
        }
    }

    // A token reference must be a REAL 20-byte address; placeholders ("0x0")
    // become "unidentified token" — never a malformed tokenAddress
    // downstream (`clear-signing.ts:1009-1020`).
    let mut token_invalid = false;
    if let Some(t) = token_addr.take() {
        let norm = if t.starts_with("0x") {
            t
        } else {
            format!("0x{t}")
        };
        if is_hex_address_shape(&norm) {
            token_addr = Some(norm.to_lowercase());
        } else {
            token_invalid = true;
        }
    }

    // Known decimals → on-chain (prefetched) → 18 + unverified (invariant ①).
    let (decimals, decimals_verified) =
        guess_token_decimals(model, run.req.chain_id(), token_addr.as_deref());
    let verified = decimals_verified && !token_invalid;
    // An unverified amount is not a small amount. Formatting 1000000 raw units
    // with the 18-decimal fallback printed "0" on a transfer of 1 USDC — a
    // signing sheet stating a confident, wrong number on the one line a person
    // is being asked to judge. Where the decimals are unknown the magnitude is
    // unknown, so the core says nothing rather than something: the em dash is
    // this wallet's word for "no number here" (the fee card's, when nothing is
    // priced), and `unverified` is already set for the shells that have a
    // phrase of their own.
    //
    // Both halves are needed. The dash alone under a warning is honest; the
    // number was not.
    let display = if verified {
        format_token_value(&amount, decimals, &run.locale)
    } else {
        UNKNOWN_AMOUNT.to_owned()
    };
    // The static table first (it is the TS's and stays authoritative for the
    // nineteen it holds), then what the chain itself answered, then the
    // address. Only the last of those leaves somebody reading a contract
    // address where a symbol belongs.
    let symbol = match &token_addr {
        Some(addr) => known_token_symbol(addr)
            .map(str::to_owned)
            .or_else(|| {
                model
                    .symbol_cache
                    .get(&std_key(run.req.chain_id(), addr))
                    .cloned()
            })
            .unwrap_or_else(|| format!("{}...", take_chars(addr, 0, 6))),
        None => "tokens".to_owned(),
    };
    let usd_value = if verified && STABLE_SYMBOLS.contains(&symbol.as_str()) {
        usd_magnitude(&amount, decimals)
    } else {
        None
    };

    Some(Formatted {
        value: format!("{display} {symbol}"),
        format: "tokenAmount".to_owned(),
        token_address: token_addr,
        warning: false,
        unverified: !verified,
        expired: false,
        address: None,
        usd_value,
    })
}

fn format_address(raw: Option<&Ctx>) -> Option<Formatted> {
    let raw = raw?;
    if is_falsy(raw) {
        return None;
    }
    let addr = js_string(raw);
    if addr.chars().count() < 10 {
        return Some(Formatted::plain(addr, "addressName"));
    }
    let address = is_hex_address_shape(&addr).then(|| addr.to_lowercase());
    Some(Formatted {
        address,
        ..Formatted::plain(
            format!("{}...{}", take_chars(&addr, 0, 8), last_chars(&addr, 6)),
            "addressName",
        )
    })
}

fn format_native_amount(run: &Run, raw: Option<&Ctx>) -> Option<Formatted> {
    let amount = to_bigint(raw);
    if amount == "0" {
        return None;
    }
    // Ticker included so the summary reads "0.5 ETH", not a bare "0.5".
    Some(Formatted::plain(
        format!(
            "{} {}",
            format_wei_amount(&amount, &run.locale),
            native_symbol(run.req.chain_id())
        ),
        "amount",
    ))
}

fn format_raw(raw: Option<&Ctx>) -> Option<Formatted> {
    let raw = raw?;
    Some(Formatted::plain(truncate_hex(&js_string(raw)), "raw"))
}

fn format_date_field(run: &Run, raw: Option<&Ctx>) -> Option<Formatted> {
    let ts = dec_to_f64(&to_bigint(raw));
    if ts == 0.0 {
        return None;
    }
    // "No expiry" sentinels overflow into "Invalid Date" — omit instead.
    if ts >= NO_DEADLINE_THRESHOLD || !ts.is_finite() {
        return None;
    }
    let now_sec = (run.now_ms / 1000.0).floor();
    let expired = ts > 1_000_000_000.0 && ts < now_sec;
    Some(Formatted {
        expired,
        ..Formatted::plain(format_date_time(ts as i64, &run.locale), "date")
    })
}

fn format_duration(raw: Option<&Ctx>) -> Option<Formatted> {
    let secs = dec_to_f64(&to_bigint(raw));
    if secs == 0.0 {
        return None;
    }
    let value = if secs < 60.0 {
        format!("{}s", secs as u64)
    } else if secs < 3_600.0 {
        format!("{}m", (secs / 60.0).floor() as u64)
    } else if secs < 86_400.0 {
        format!("{}h", (secs / 3_600.0).floor() as u64)
    } else {
        format!("{}d", (secs / 86_400.0).floor() as u64)
    };
    Some(Formatted::plain(value, "duration"))
}

fn format_enum(raw: Option<&Ctx>, params: &Value, metadata: &Value) -> Option<Formatted> {
    let raw = raw?;
    let key = js_string(raw);
    if let Some(ref_path) = params.get("$ref").and_then(Value::as_str) {
        let enum_def = resolve_metadata_ref(ref_path, metadata);
        if let Some(mapped) = enum_def.get(&key).and_then(Value::as_str) {
            return Some(Formatted::plain(mapped.to_owned(), "enum"));
        }
    }
    Some(Formatted::plain(key, "enum"))
}

/// NFT token id → "#1,234"; anything else (a name) passes through.
fn format_nft_name(raw: Option<&Ctx>, locale: &ClearLocale) -> Option<Formatted> {
    let Some(raw) = raw else {
        return Some(Formatted::plain("NFT".to_owned(), "nftName"));
    };
    if matches!(raw, Ctx::Null) {
        return Some(Formatted::plain("NFT".to_owned(), "nftName"));
    }
    let s = js_string(raw);
    let value = if !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) {
        format!("#{}", group_digits(&s, locale))
    } else {
        s
    };
    Some(Formatted::plain(value, "nftName"))
}

fn format_unit(raw: Option<&Ctx>, params: &Value, locale: &ClearLocale) -> Option<Formatted> {
    let raw = raw?;
    let num = dec_to_f64(&to_bigint(Some(raw)));
    let decimals = params.get("decimals").and_then(Value::as_u64).unwrap_or(0) as u32;
    let base = params.get("base").and_then(Value::as_str).unwrap_or("");
    let prefix = params
        .get("prefix")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let display = if decimals > 0 {
        format_number(
            num / 10f64.powi(decimals as i32),
            decimals as usize,
            decimals as usize,
            locale,
        )
    } else {
        format_number(num, 0, 0, locale)
    };
    let formatted = if prefix {
        format!("{base}{display}")
    } else {
        format!("{display}{base}")
    };
    Some(Formatted::plain(formatted.trim().to_owned(), "unit"))
}

fn truncate_hex(s: &str) -> String {
    if s.chars().count() <= 20 {
        s.to_owned()
    } else {
        format!("{}...{}", take_chars(s, 0, 10), last_chars(s, 8))
    }
}

/// Token addresses referenced by tokenAmount fields, for the decimals warm
/// (`collectTokenAddrs`, clear-signing.ts:401-422).
fn collect_token_addrs(
    field_defs: &[Value],
    ctx: &Ctx,
    metadata: &Value,
    definitions: &Value,
) -> Vec<String> {
    let mut addrs = Vec::new();
    for fd in field_defs {
        let def = merged_def(fd, definitions);
        if def.get("format").and_then(Value::as_str) != Some("tokenAmount") {
            continue;
        }
        let params = def.get("params").cloned().unwrap_or(Value::Null);
        let mut token_addr: Option<String> = None;
        if let Some(token_path) = params.get("tokenPath").and_then(Value::as_str) {
            if let Some(Ctx::Str(s)) = resolve_path(token_path, ctx) {
                token_addr = Some(s);
            }
        }
        if let Some(token_ref) = params.get("token").and_then(Value::as_str) {
            if let Value::String(s) = resolve_metadata_ref(token_ref, metadata) {
                token_addr = Some(s);
            }
        }
        if let Some(addr) = token_addr {
            if is_hex_address_shape(&addr) {
                addrs.push(addr);
            }
        }
    }
    addrs
}

/// The warm set: unique lowercased addresses with no catalog entry and no
/// cached on-chain answer (`warmTokenDecimals`, clear-signing.ts:372-377).
fn unknown_token_addrs(
    model: &Model,
    run: &Run,
    descriptor: &Value,
    matched_sig: &str,
    ctx: &Ctx,
) -> BTreeSet<String> {
    let format = descriptor
        .get("display")
        .and_then(|d| d.get("formats"))
        .and_then(|f| f.get(matched_sig));
    let field_defs = format
        .and_then(|f| f.get("fields"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let metadata = descriptor.get("metadata").cloned().unwrap_or(Value::Null);
    let definitions = descriptor
        .get("display")
        .and_then(|d| d.get("definitions"))
        .cloned()
        .unwrap_or(Value::Null);
    let chain_id = run.req.chain_id();

    collect_token_addrs(&field_defs, ctx, &metadata, &definitions)
        .into_iter()
        .map(|a| a.to_lowercase())
        .filter(|a| {
            known_token_decimals(a).is_none()
                && !model.decimals_cache.contains_key(&std_key(chain_id, a))
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Risk assessment + role inference (clear-signing.ts:1250-1307)
// ---------------------------------------------------------------------------

fn assess_risk(intent: &str, fields: &[ClearSignField], partial: bool) -> ClearRisk {
    // Any warning field (e.g. unlimited approval) → danger. Full stop.
    if fields.iter().any(|f| f.warning) {
        return ClearRisk::Danger;
    }
    let i = intent.to_lowercase();
    let base = if contains_any(&i, &["approve", "permit", "authorize"]) {
        ClearRisk::Caution
    } else if contains_any(&i, &["stake", "deposit", "claim", "supply"]) {
        ClearRisk::Safe
    } else {
        ClearRisk::Normal
    };
    // Incomplete decode / unverified amount / expired deadline: never let it
    // read as safe or normal — floor at caution (invariant ③).
    let uncertain = partial || fields.iter().any(|f| f.unverified || f.expired);
    if uncertain && matches!(base, ClearRisk::Safe | ClearRisk::Normal) {
        return ClearRisk::Caution;
    }
    base
}

fn infer_field_roles(fields: Vec<ClearSignField>, intent: &str) -> Vec<ClearSignField> {
    let i = intent.to_lowercase();
    fields
        .into_iter()
        .map(|mut f| {
            let label = f.label.to_lowercase();
            f.role = if f.format == "tokenAmount" || f.format == "amount" {
                if contains_any(&label, &["receive", "output", "min", "return", "get"]) {
                    ClearFieldRole::ReceiveAmount
                } else if contains_any(
                    &label,
                    &["send", "pay", "input", "deposit", "spend", "stake"],
                ) {
                    ClearFieldRole::SendAmount
                } else if contains_any(&i, &["withdraw", "redeem", "unstake", "claim"])
                    || contains_any(&label, &["withdraw", "redeem"])
                {
                    // Withdraw/redeem/unstake/claim bring assets INTO the
                    // wallet — a receive even when labelled "amount" (F7).
                    ClearFieldRole::ReceiveAmount
                } else {
                    ClearFieldRole::SendAmount
                }
            } else if f.format == "addressName" {
                if contains_any(&label, &["spender", "operator"]) {
                    ClearFieldRole::Spender
                } else if contains_any(&label, &["to", "recipient", "receiver", "destination"]) {
                    ClearFieldRole::Recipient
                } else {
                    ClearFieldRole::Generic
                }
            } else {
                ClearFieldRole::Generic
            };
            f
        })
        .collect()
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|n| haystack.contains(n))
}

// ---------------------------------------------------------------------------
// Blind typed data (BlindTypedDataView.parseTypedDataForDisplay, ㉓)
// ---------------------------------------------------------------------------

/// A JSON value that remembers the order its keys were WRITTEN in.
///
/// `serde_json::Map` is a `BTreeMap` here (no `preserve_order`), which would
/// alphabetise both the rows an EIP-712 payload declared in a meaningful order
/// and the keys inside a nested struct rendered as JSON. `Object.entries` and
/// `JSON.stringify` do neither, and neither may we: the first five entries are
/// what the user reads (so re-ordering changes WHICH five are shown), and a
/// re-ordered nested struct is a visibly different line on a security surface.
enum OrderedValue {
    /// null / bool / number / string.
    Scalar(Value),
    Array(Vec<OrderedValue>),
    Object(Vec<(String, OrderedValue)>),
}

impl OrderedValue {
    fn is_container(&self) -> bool {
        matches!(self, OrderedValue::Array(_) | OrderedValue::Object(_))
    }

    /// `JSON.stringify` for this value: compact, insertion-ordered, and with
    /// numbers printed the way JavaScript prints them.
    fn to_json(&self) -> String {
        let mut out = String::new();
        self.write_json(&mut out);
        out
    }

    fn write_json(&self, out: &mut String) {
        match self {
            OrderedValue::Scalar(Value::Number(n)) => {
                out.push_str(&n.as_f64().map(js_number_to_string).unwrap_or_default());
            }
            OrderedValue::Scalar(value) => {
                out.push_str(&serde_json::to_string(value).unwrap_or_default());
            }
            OrderedValue::Array(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    item.write_json(out);
                }
                out.push(']');
            }
            OrderedValue::Object(entries) => {
                out.push('{');
                for (i, (key, value)) in entries.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&serde_json::to_string(key).unwrap_or_default());
                    out.push(':');
                    value.write_json(out);
                }
                out.push('}');
            }
        }
    }
}

impl<'de> Deserialize<'de> for OrderedValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = OrderedValue;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("any JSON value")
            }
            fn visit_unit<E>(self) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::Null))
            }
            fn visit_none<E>(self) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::Null))
            }
            fn visit_some<D2>(self, d: D2) -> Result<OrderedValue, D2::Error>
            where
                D2: serde::Deserializer<'de>,
            {
                OrderedValue::deserialize(d)
            }
            fn visit_bool<E>(self, v: bool) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::Bool(v)))
            }
            fn visit_i64<E>(self, v: i64) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::from(v)))
            }
            fn visit_u64<E>(self, v: u64) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::from(v)))
            }
            fn visit_f64<E>(self, v: f64) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::from(v)))
            }
            fn visit_str<E>(self, v: &str) -> Result<OrderedValue, E> {
                Ok(OrderedValue::Scalar(Value::from(v)))
            }
            fn visit_seq<A>(self, mut seq: A) -> Result<OrderedValue, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(item) = seq.next_element::<OrderedValue>()? {
                    out.push(item);
                }
                Ok(OrderedValue::Array(out))
            }
            fn visit_map<A>(self, mut map: A) -> Result<OrderedValue, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(entry) = map.next_entry::<String, OrderedValue>()? {
                    out.push(entry);
                }
                Ok(OrderedValue::Object(out))
            }
        }
        deserializer.deserialize_any(Visit)
    }
}

/// `message`'s own entries, in payload order.
struct OrderedEntries(Vec<(String, OrderedValue)>);

impl<'de> Deserialize<'de> for OrderedEntries {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Visit;
        impl<'de> serde::de::Visitor<'de> for Visit {
            type Value = OrderedEntries;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A>(self, mut map: A) -> Result<OrderedEntries, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut out = Vec::new();
                while let Some(entry) = map.next_entry::<String, OrderedValue>()? {
                    out.push(entry);
                }
                Ok(OrderedEntries(out))
            }
        }
        deserializer.deserialize_map(Visit)
    }
}

/// `message` may be anything at all — it is untrusted. Only an object has
/// entries to show; everything else renders no rows, as `Object.entries` on a
/// non-object yields nothing the view can label.
#[derive(Deserialize)]
#[serde(untagged)]
enum MessageRaw {
    Ordered(OrderedEntries),
    Other(#[allow(dead_code)] Value),
}

#[derive(Deserialize)]
struct TypedRaw {
    #[serde(default, rename = "primaryType")]
    primary_type: Option<Value>,
    #[serde(default)]
    domain: Option<Value>,
    #[serde(default)]
    message: Option<MessageRaw>,
}

impl ClearBlindTyped {
    fn empty() -> Self {
        ClearBlindTyped {
            primary_type: None,
            has_domain: false,
            domain_name: None,
            verifying_contract: None,
            fields: Vec::new(),
        }
    }
}

/// How many `message` rows the blind surface shows (`slice(0, 5)`).
const BLIND_FIELD_LIMIT: usize = 5;
/// `formatBlindValue`'s cap (`slice(0, 60)`).
const BLIND_VALUE_LIMIT: usize = 60;

fn project_blind_typed(typed_data_json: &str) -> ClearBlindTyped {
    let Ok(raw) = serde_json::from_str::<TypedRaw>(typed_data_json) else {
        // Valid JSON that isn't an object — nothing to project, exactly as
        // `data?.primaryType` / `data?.domain` read `undefined`.
        return ClearBlindTyped::empty();
    };
    let domain = raw.domain.filter(is_truthy);
    ClearBlindTyped {
        primary_type: raw
            .primary_type
            .filter(is_truthy)
            .as_ref()
            .map(js_scalar_string),
        has_domain: domain.is_some(),
        domain_name: domain
            .as_ref()
            .and_then(|d| d.get("name"))
            .filter(|v| is_truthy(v))
            .map(js_scalar_string),
        // `.toLowerCase()` is only reachable on a string; anything else is no
        // address at all rather than a coerced one.
        verifying_contract: domain
            .as_ref()
            .and_then(|d| d.get("verifyingContract"))
            .and_then(Value::as_str)
            .map(str::to_lowercase),
        fields: match raw.message {
            Some(MessageRaw::Ordered(entries)) => entries
                .0
                .into_iter()
                .take(BLIND_FIELD_LIMIT)
                .map(|(key, value)| ClearBlindField {
                    key,
                    value: format_blind_value(&value),
                })
                .collect(),
            _ => Vec::new(),
        },
    }
}

/// JavaScript truthiness — `{domain && …}` / `{primaryType && …}`.
fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

/// One raw typed-data value on a single line (`formatBlindValue`). A long hex
/// blob is mid-truncated so it never wraps into a two-line hex wall.
fn format_blind_value(value: &OrderedValue) -> String {
    if value.is_container() {
        return take_chars(&value.to_json(), 0, BLIND_VALUE_LIMIT);
    }
    let OrderedValue::Scalar(value) = value else {
        unreachable!("containers handled above");
    };
    let s = js_scalar_string(value);
    if is_long_hex_word(&s) {
        let bytes = s.as_bytes();
        let head = &s[..10];
        let tail = std::str::from_utf8(&bytes[bytes.len() - 8..]).unwrap_or("");
        return format!("{head}…{tail}");
    }
    take_chars(&s, 0, BLIND_VALUE_LIMIT)
}

/// `/^0x[0-9a-fA-F]{21,}$/` — an address / salt / bytes blob.
fn is_long_hex_word(s: &str) -> bool {
    let Some(body) = s.strip_prefix("0x") else {
        return false;
    };
    body.len() >= 21 && body.bytes().all(|b| b.is_ascii_hexdigit())
}

/// `String(v)` for a JSON scalar.
fn js_scalar_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(true) => "true".to_owned(),
        Value::Bool(false) => "false".to_owned(),
        Value::String(s) => s.clone(),
        Value::Number(n) => n.as_f64().map(js_number_to_string).unwrap_or_default(),
        // Unreachable from `format_blind_value`; JSON is the honest rendering.
        other => serde_json::to_string(other).unwrap_or_default(),
    }
}

/// ECMAScript `Number::toString` (radix 10). `serde_json` would print `1e21`
/// as `1000000000000000000000` and `100.0` for an integral float; JS prints
/// `1e+21` and `100`. This is a displayed value on a signing surface, so it
/// matches the engine that produced today's screen, digit for digit.
fn js_number_to_string(f: f64) -> String {
    if f.is_nan() {
        return "NaN".to_owned();
    }
    if f.is_infinite() {
        return if f > 0.0 { "Infinity" } else { "-Infinity" }.to_owned();
    }
    if f == 0.0 {
        return "0".to_owned();
    }
    let sign = if f < 0.0 { "-" } else { "" };
    // `{:e}` is the shortest round-trip form: "1e21", "1.5e-7", "1.234e2".
    let repr = format!("{:e}", f.abs());
    let (mantissa, exponent) = match repr.split_once('e') {
        Some(parts) => parts,
        None => return format!("{sign}{repr}"),
    };
    let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
    let k = digits.len() as i32;
    let n = exponent.parse::<i32>().unwrap_or(0) + 1;

    let body = if k <= n && n <= 21 {
        // Digits, then n−k zeros.
        format!("{digits}{}", "0".repeat((n - k) as usize))
    } else if 0 < n && n <= 21 {
        format!("{}.{}", &digits[..n as usize], &digits[n as usize..])
    } else if -6 < n && n <= 0 {
        format!("0.{}{digits}", "0".repeat((-n) as usize))
    } else {
        let e = n - 1;
        let esign = if e >= 0 { "+" } else { "-" };
        if k == 1 {
            format!("{digits}e{esign}{}", e.abs())
        } else {
            format!("{}.{}e{esign}{}", &digits[..1], &digits[1..], e.abs())
        }
    };
    format!("{sign}{body}")
}

// ---------------------------------------------------------------------------
// personal_sign / eth_sign analysis (decode-sign-message.ts + siwe.ts)
// ---------------------------------------------------------------------------

fn analyze_message(
    method: ClearSignMethod,
    payload: &str,
    request_origin: Option<&str>,
) -> ClearMessageView {
    let is_hex = is_hex_payload(payload);
    let decoded = decode_personal_message(payload);
    let (decoded_text, binary_preview, non_printable) = match &decoded {
        Decoded::Text(text) => (Some(text.clone()), None, false),
        Decoded::Binary(preview) => (None, Some(preview.clone()), true),
    };

    if method == ClearSignMethod::EthSign {
        // The classic blind-sign trap — hard warning, no message analysis.
        return ClearMessageView {
            payload: payload.to_owned(),
            is_hex,
            decoded_text,
            binary_preview,
            non_printable,
            siwe: None,
            binding: None,
            danger_class: ClearDangerClass::EthSign,
        };
    }

    let siwe = decoded_text.as_deref().and_then(parse_siwe);
    let binding = siwe
        .as_ref()
        .map(|s| check_siwe_domain_binding(Some(&s.domain), request_origin));
    let danger_class = match (&siwe, binding) {
        (Some(_), Some(ClearSiweBinding::Mismatch)) => ClearDangerClass::SiwePhish,
        (Some(_), _) => ClearDangerClass::SiweOk,
        (None, _) if non_printable => ClearDangerClass::OpaqueHash,
        _ => ClearDangerClass::Plain,
    };

    ClearMessageView {
        payload: payload.to_owned(),
        is_hex,
        decoded_text,
        binary_preview,
        non_printable,
        siwe,
        binding,
        danger_class,
    }
}

/// THE hex/text predicate both display and signer branch on (MetaMask's
/// rule): only a `0x`-prefixed, even-length, all-hex payload is hex
/// (`decode-sign-message.ts:44-48`).
fn is_hex_payload(payload: &str) -> bool {
    let Some(body) = payload.strip_prefix("0x") else {
        return false;
    };
    body.len() % 2 == 0 && body.bytes().all(|b| b.is_ascii_hexdigit())
}

enum Decoded {
    Text(String),
    Binary(String),
}

/// `decodePersonalMessage` (decode-sign-message.ts:50-66). The lossy UTF-8
/// decode substitutes U+FFFD exactly like a non-fatal TextDecoder, and the
/// Unicode-aware binary guard keeps emoji/CJK as text (issue #82).
fn decode_personal_message(payload: &str) -> Decoded {
    if !is_hex_payload(payload) {
        return Decoded::Text(payload.to_owned()); // plain UTF-8, verbatim
    }
    let clean = payload.strip_prefix("0x").unwrap_or(payload);
    if clean.is_empty() {
        return Decoded::Text(String::new());
    }
    let bytes = primitives::from_hex(clean).unwrap_or_default();
    let decoded = String::from_utf8_lossy(&bytes);
    if decoded.chars().any(is_binary_char) {
        let head = take_chars(clean, 0, 64);
        let ellipsis = if clean.len() > 64 { "..." } else { "" };
        return Decoded::Binary(format!("0x{head}{ellipsis}"));
    }
    Decoded::Text(decoded.into_owned())
}

/// A char that marks a payload as binary: C0 (minus tab/LF/CR), DEL, C1, or
/// U+FFFD (`decode-sign-message.ts:25-30`).
fn is_binary_char(c: char) -> bool {
    let code = c as u32;
    if code < 0x20 {
        return code != 0x09 && code != 0x0a && code != 0x0d;
    }
    if code == 0x7f {
        return true;
    }
    if (0x80..=0x9f).contains(&code) {
        return true;
    }
    code == 0xfffd
}

const SIWE_ANCHOR: &str = " wants you to sign in with your Ethereum account:";
const SIWE_FIELD_PREFIXES: [&str; 9] = [
    "URI:",
    "Version:",
    "Chain ID:",
    "Nonce:",
    "Issued At:",
    "Expiration Time:",
    "Not Before:",
    "Request ID:",
    "Resources:",
];

/// `parseSiwe` (siwe.ts:31-77). Conservative: the canonical first line is
/// required so arbitrary prose is never mis-parsed as a sign-in.
fn parse_siwe(message: &str) -> Option<ClearSiweFields> {
    if message.is_empty() {
        return None;
    }
    // CRLF/CR → LF, or a trailing "\r" breaks the line-1 anchor and silently
    // disables phishing detection (siwe.ts:33-36).
    let normalized = message.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let first = lines.first()?;
    let domain = first.strip_suffix(SIWE_ANCHOR)?;
    if domain.is_empty() || domain.chars().any(char::is_whitespace) {
        return None;
    }
    // A bare RFC-3986 authority only: userinfo, path, backslash, query,
    // fragment or scheme means "not SIWE" (siwe.ts:45). The `://` case is
    // subsumed by `/`.
    if domain.contains(['@', '/', '\\', '?', '#']) {
        return None;
    }

    let mut out = ClearSiweFields {
        domain: domain.to_owned(),
        domain_host: siwe_host(Some(domain)),
        address: None,
        statement: None,
        uri: None,
        chain_id: None,
        nonce: None,
    };

    if let Some(line2) = lines.get(1) {
        let candidate = line2.trim();
        if is_hex_address_shape(candidate) {
            out.address = Some(candidate.to_owned());
        }
    }

    for line in &lines {
        if let Some(rest) = line.strip_prefix("URI:") {
            let uri = rest.trim_start();
            if !uri.is_empty() {
                out.uri = Some(uri.trim().to_owned());
            }
        }
        if let Some(rest) = line.strip_prefix("Chain ID:") {
            let digits = rest.trim_start();
            if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
                out.chain_id = digits.parse().ok();
            }
        }
        if let Some(rest) = line.strip_prefix("Nonce:") {
            let nonce = rest.trim_start();
            if !nonce.is_empty() {
                out.nonce = Some(nonce.trim().to_owned());
            }
        }
    }

    // The statement is the optional block between two blank lines.
    if let Some(first_blank) = lines.iter().position(|l| l.is_empty()) {
        if first_blank >= 2 && first_blank + 1 < lines.len() {
            let after = lines[first_blank + 1];
            let is_field = SIWE_FIELD_PREFIXES.iter().any(|p| after.starts_with(p));
            if !after.is_empty() && !is_field {
                let second_blank = lines
                    .iter()
                    .skip(first_blank + 1)
                    .position(|l| l.is_empty())
                    .map(|i| i + first_blank + 1);
                let end = match second_blank {
                    Some(i) if i > first_blank => i,
                    _ => first_blank + 2,
                };
                let stmt = lines
                    .get(first_blank + 1..end)
                    .unwrap_or_default()
                    .join(" ")
                    .trim()
                    .to_owned();
                if !stmt.is_empty() {
                    out.statement = Some(stmt);
                }
            }
        }
    }

    Some(out)
}

/// `siweHost` (siwe.ts:80-93): lowercased host, no port, single trailing
/// FQDN dot stripped; unparseable is `None` — fail safe, never a half-parsed
/// host that could spuriously match.
fn siwe_host(value: Option<&str>) -> Option<String> {
    let value = value?;
    if value.is_empty() {
        return None;
    }
    // Accept bare hosts, full origins and authority[:port].
    let rest = match split_scheme(value) {
        Some(rest) => rest,
        None => value,
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    // Userinfo before the LAST '@' — matching WHATWG (and letting
    // "trusted.org@evil.com" resolve to evil.com, the attack this catches).
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = if host_port.starts_with('[') {
        // IPv6 literal keeps its brackets, port after ']' drops.
        let end = host_port.find(']')?;
        host_port.get(..=end).unwrap_or_default()
    } else {
        match host_port.rsplit_once(':') {
            Some((h, port)) if port.bytes().all(|b| b.is_ascii_digit()) => h,
            Some(_) => return None, // a colon with a non-numeric port is no URL
            None => host_port,
        }
    };
    let host = host.to_lowercase();
    let host = host.strip_suffix('.').unwrap_or(&host).to_owned();
    if host.is_empty()
        || host
            .chars()
            .any(|c| c.is_whitespace() || "@/\\?#:".contains(c))
    {
        return None;
    }
    Some(host)
}

fn split_scheme(value: &str) -> Option<&str> {
    // /^[a-z][a-z0-9+.-]*:\/\//i
    let idx = value.find("://")?;
    let scheme = value.get(..idx)?;
    let mut chars = scheme.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c)) {
        return None;
    }
    value.get(idx + 3..)
}

/// `checkSiweDomainBinding` (siwe.ts:104-113).
fn check_siwe_domain_binding(
    siwe_domain: Option<&str>,
    request_origin: Option<&str>,
) -> ClearSiweBinding {
    let Some(a) = siwe_host(siwe_domain) else {
        return ClearSiweBinding::Unknown;
    };
    let Some(b) = siwe_host(request_origin) else {
        return ClearSiweBinding::Unknown;
    };
    if a == b {
        ClearSiweBinding::Ok
    } else {
        ClearSiweBinding::Mismatch
    }
}

// ---------------------------------------------------------------------------
// Static catalogs
// ---------------------------------------------------------------------------

fn std_key(chain_id: u32, addr: &str) -> String {
    format!("{chain_id}:{}", addr.to_lowercase())
}

fn known_token_symbol(addr: &str) -> Option<&'static str> {
    let lc = addr.to_lowercase();
    KNOWN_TOKENS
        .iter()
        .find(|(a, _, _)| *a == lc)
        .map(|(_, s, _)| *s)
}

fn known_token_decimals(addr: &str) -> Option<u32> {
    let lc = addr.to_lowercase();
    KNOWN_TOKENS
        .iter()
        .find(|(a, _, _)| *a == lc)
        .map(|(_, _, d)| *d)
}

fn known_contract(addr: &str) -> Option<(&'static str, &'static str)> {
    let lc = addr.to_lowercase();
    KNOWN_CONTRACTS
        .iter()
        .find(|(a, _, _)| *a == lc)
        .map(|(_, name, owner)| (*name, *owner))
}

fn native_symbol(chain_id: u32) -> &'static str {
    NATIVE_SYMBOLS
        .iter()
        .find(|(id, _)| *id == chain_id)
        .map(|(_, s)| *s)
        .unwrap_or("ETH")
}

fn guess_token_decimals(model: &Model, chain_id: u32, token_addr: Option<&str>) -> (u32, bool) {
    let Some(addr) = token_addr else {
        return (18, true); // native currency
    };
    let lc = addr.to_lowercase();
    if let Some(d) = known_token_decimals(&lc) {
        return (d, true);
    }
    if let Some(d) = model.decimals_cache.get(&std_key(chain_id, &lc)) {
        return (*d, true);
    }
    (18, false) // unknown — flagged so the UI can warn (invariant ①)
}

fn supports_interface_calldata(iface: &str) -> String {
    // bytes4 occupies the high-order bytes: id ‖ 28 zero bytes.
    format!("{SUPPORTS_INTERFACE_SELECTOR}{iface}{}", "0".repeat(56))
}

fn selector_of(data: &str) -> String {
    let body = data.strip_prefix("0x").unwrap_or(data);
    format!("0x{}", take_chars(body, 0, 8).to_lowercase())
}

fn is_hex_address_shape(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_falsy(v: &Ctx) -> bool {
    match v {
        Ctx::Null => true,
        Ctx::Bool(b) => !b,
        Ctx::Num(d) => d == "0",
        Ctx::JsNum(f) => *f == 0.0 || f.is_nan(),
        Ctx::Str(s) => s.is_empty(),
        Ctx::Arr(_) | Ctx::Map(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Local descriptors (local-descriptors.ts — ERC-7730-shaped, verbatim)
// ---------------------------------------------------------------------------

fn recipient_field() -> Value {
    json!({ "path": "to", "label": "Recipient", "format": "addressName" })
}

fn deadline_field() -> Value {
    json!({ "path": "deadline", "label": "Deadline", "format": "date" })
}

/// Uniswap-V2-style router — the classic swap ABI shared verbatim by every
/// V2 fork, fee-on-transfer variants included.
fn v2_router_descriptor(contract_name: &str, owner: &str) -> Value {
    let pay_in = json!({ "path": "amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "path.0" } });
    let recv_min = json!({ "path": "amountOutMin", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "path.-1" } });
    let tokens_for_tokens = json!({ "intent": "Swap", "fields": [pay_in.clone(), recv_min.clone(), recipient_field(), deadline_field()] });
    let eth_for_tokens = json!({
        "intent": "Swap",
        "fields": [{ "path": "@.value", "label": "You pay", "format": "amount" }, recv_min, recipient_field(), deadline_field()],
    });
    let tokens_for_eth = json!({
        "intent": "Swap",
        "fields": [pay_in, { "path": "amountOutMin", "label": "You receive (min)", "format": "amount" }, recipient_field(), deadline_field()],
    });
    json!({
        "metadata": { "contractName": contract_name, "owner": owner },
        "display": {
            "formats": {
                "swapExactTokensForTokens(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)": tokens_for_tokens.clone(),
                "swapExactTokensForTokensSupportingFeeOnTransferTokens(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)": tokens_for_tokens,
                "swapTokensForExactTokens(uint256 amountOut,uint256 amountInMax,address[] path,address to,uint256 deadline)": {
                    "intent": "Swap",
                    "fields": [
                        { "path": "amountInMax", "label": "You pay (max)", "format": "tokenAmount", "params": { "tokenPath": "path.0" } },
                        { "path": "amountOut", "label": "You receive", "format": "tokenAmount", "params": { "tokenPath": "path.-1" } },
                        recipient_field(), deadline_field(),
                    ],
                },
                "swapExactETHForTokens(uint256 amountOutMin,address[] path,address to,uint256 deadline)": eth_for_tokens.clone(),
                "swapExactETHForTokensSupportingFeeOnTransferTokens(uint256 amountOutMin,address[] path,address to,uint256 deadline)": eth_for_tokens,
                "swapExactTokensForETH(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)": tokens_for_eth.clone(),
                "swapExactTokensForETHSupportingFeeOnTransferTokens(uint256 amountIn,uint256 amountOutMin,address[] path,address to,uint256 deadline)": tokens_for_eth,
            },
        },
    })
}

/// Uniswap-V3-style SwapRouter02 (no deadline in the structs).
fn v3_router02_descriptor(contract_name: &str, owner: &str) -> Value {
    json!({
        "metadata": { "contractName": contract_name, "owner": owner },
        "display": {
            "formats": {
                "exactInputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 amountIn,uint256 amountOutMinimum,uint160 sqrtPriceLimitX96) params)": {
                    "intent": "Swap",
                    "fields": [
                        { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.tokenIn" } },
                        { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.tokenOut" } },
                        { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                    ],
                },
                "exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)": {
                    "intent": "Swap",
                    "fields": [
                        { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.path[0:20]" } },
                        { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.path[-20:]" } },
                        { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                    ],
                },
            },
        },
    })
}

fn seaport_descriptor() -> Value {
    let fields = json!([
        { "path": "parameters.offerToken", "label": "NFT", "format": "addressName" },
        { "path": "parameters.offerIdentifier", "label": "Token ID", "format": "nftName" },
        { "path": "parameters.considerationAmount", "label": "Price", "format": "tokenAmount", "params": { "tokenPath": "parameters.considerationToken", "nativeCurrencyAddress": ["$.metadata.constants.native"] } },
        { "path": "parameters.offerer", "label": "Seller", "format": "addressName" },
        { "path": "parameters.endTime", "label": "Deadline", "format": "date" },
    ]);
    let tuple = "(address considerationToken,uint256 considerationIdentifier,uint256 considerationAmount,address offerer,address zone,address offerToken,uint256 offerIdentifier,uint256 offerAmount,uint8 basicOrderType,uint256 startTime,uint256 endTime,bytes32 zoneHash,uint256 salt,bytes32 offererConduitKey,bytes32 fulfillerConduitKey,uint256 totalOriginalAdditionalRecipients,(uint256 amount,address recipient)[] additionalRecipients,bytes signature) parameters";
    let entry = json!({ "intent": "Buy NFT", "fields": fields });
    let mut formats = serde_json::Map::new();
    formats.insert(format!("fulfillBasicOrder({tuple})"), entry.clone());
    formats.insert(
        format!("fulfillBasicOrder_efficient_6GL6yc({tuple})"),
        entry,
    );
    json!({
        "metadata": { "contractName": "Seaport", "owner": "OpenSea", "constants": { "native": "0x0000000000000000000000000000000000000000" } },
        "display": { "formats": Value::Object(formats) },
    })
}

fn local_descriptor(addr: &str) -> Option<Value> {
    Some(match addr {
        "0x7a250d5630b4cf539739df2c5dacb4c659f2488d" => {
            v2_router_descriptor("Uniswap V2 Router", "Uniswap")
        }
        "0x10ed43c718714eb63d5aa57b78b54704e256024e" => {
            v2_router_descriptor("PancakeSwap V2 Router", "PancakeSwap")
        }
        "0xd9e1ce17f2641f24ae83637ab66a2cca9c378b9f" => {
            v2_router_descriptor("SushiSwap Router", "SushiSwap")
        }
        "0x1b81d678ffb9c0263b24a97847620c99d213eb14" => {
            v3_router02_descriptor("PancakeSwap V3 Router", "PancakeSwap")
        }
        "0x13f4ea83d0bd40e75c8222255bc855a974568dd4" => {
            v3_router02_descriptor("PancakeSwap Smart Router", "PancakeSwap")
        }
        "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2" => json!({
            "metadata": { "contractName": "Wrapped Ether", "owner": "WETH" },
            "display": {
                "formats": {
                    "deposit()": { "intent": "Wrap ETH", "fields": [{ "path": "@.value", "label": "Amount", "format": "amount" }] },
                    "withdraw(uint256 wad)": { "intent": "Unwrap WETH", "fields": [{ "path": "wad", "label": "Amount", "format": "amount" }] },
                },
            },
        }),
        "0x87870bca3f3fd6335c3f4ce8392d69350b4fa4e2" => json!({
            "metadata": { "contractName": "Aave V3 Pool", "owner": "Aave" },
            "display": {
                "formats": {
                    "supply(address asset,uint256 amount,address onBehalfOf,uint16 referralCode)": {
                        "intent": "Supply",
                        "fields": [
                            { "path": "amount", "label": "Supply", "format": "tokenAmount", "params": { "tokenPath": "asset" } },
                            { "path": "onBehalfOf", "label": "On behalf of", "format": "addressName" },
                        ],
                    },
                    "withdraw(address asset,uint256 amount,address to)": {
                        "intent": "Withdraw",
                        "fields": [
                            { "path": "amount", "label": "Withdraw", "format": "tokenAmount", "params": { "tokenPath": "asset" } },
                            { "path": "to", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                    "borrow(address asset,uint256 amount,uint256 interestRateMode,uint16 referralCode,address onBehalfOf)": {
                        "intent": "Borrow",
                        "fields": [
                            { "path": "amount", "label": "Borrow", "format": "tokenAmount", "params": { "tokenPath": "asset" } },
                            { "path": "onBehalfOf", "label": "On behalf of", "format": "addressName" },
                        ],
                    },
                    "repay(address asset,uint256 amount,uint256 interestRateMode,address onBehalfOf)": {
                        "intent": "Repay",
                        "fields": [
                            { "path": "amount", "label": "Repay", "format": "tokenAmount", "params": { "tokenPath": "asset" } },
                            { "path": "onBehalfOf", "label": "On behalf of", "format": "addressName" },
                        ],
                    },
                },
            },
        }),
        "0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45" => json!({
            "metadata": { "contractName": "Uniswap V3 Router", "owner": "Uniswap" },
            "display": {
                "formats": {
                    "exactInputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 amountIn,uint256 amountOutMinimum,uint160 sqrtPriceLimitX96) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.tokenIn" } },
                            { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.tokenOut" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                    "exactOutputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 amountOut,uint256 amountInMaximum,uint160 sqrtPriceLimitX96) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountInMaximum", "label": "You pay (max)", "format": "tokenAmount", "params": { "tokenPath": "params.tokenIn" } },
                            { "path": "params.amountOut", "label": "You receive", "format": "tokenAmount", "params": { "tokenPath": "params.tokenOut" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                    "exactInput((bytes path,address recipient,uint256 amountIn,uint256 amountOutMinimum) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.path[0:20]" } },
                            { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.path[-20:]" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                    "exactOutput((bytes path,address recipient,uint256 amountOut,uint256 amountInMaximum) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountInMaximum", "label": "You pay (max)", "format": "tokenAmount", "params": { "tokenPath": "params.path[-20:]" } },
                            { "path": "params.amountOut", "label": "You receive", "format": "tokenAmount", "params": { "tokenPath": "params.path[0:20]" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                },
            },
        }),
        "0xe592427a0aece92de3edee1f18e0157c05861564" => json!({
            "metadata": { "contractName": "Uniswap V3 Router", "owner": "Uniswap" },
            "display": {
                "formats": {
                    "exactInputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 deadline,uint256 amountIn,uint256 amountOutMinimum,uint160 sqrtPriceLimitX96) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.tokenIn" } },
                            { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.tokenOut" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                            { "path": "params.deadline", "label": "Deadline", "format": "date" },
                        ],
                    },
                    "exactOutputSingle((address tokenIn,address tokenOut,uint24 fee,address recipient,uint256 deadline,uint256 amountOut,uint256 amountInMaximum,uint160 sqrtPriceLimitX96) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountInMaximum", "label": "You pay (max)", "format": "tokenAmount", "params": { "tokenPath": "params.tokenIn" } },
                            { "path": "params.amountOut", "label": "You receive", "format": "tokenAmount", "params": { "tokenPath": "params.tokenOut" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                            { "path": "params.deadline", "label": "Deadline", "format": "date" },
                        ],
                    },
                    "exactInput((bytes path,address recipient,uint256 deadline,uint256 amountIn,uint256 amountOutMinimum) params)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "params.amountIn", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "params.path[0:20]" } },
                            { "path": "params.amountOutMinimum", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "params.path[-20:]" } },
                            { "path": "params.recipient", "label": "Recipient", "format": "addressName" },
                            { "path": "params.deadline", "label": "Deadline", "format": "date" },
                        ],
                    },
                },
            },
        }),
        "0x1111111254eeb25477b68fb85ed929f73a960582" => json!({
            "metadata": { "contractName": "1inch Router", "owner": "1inch" },
            "display": {
                "formats": {
                    "swap(address executor,(address srcToken,address dstToken,address srcReceiver,address dstReceiver,uint256 amount,uint256 minReturnAmount,uint256 flags) desc,bytes permit,bytes data)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "desc.amount", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "desc.srcToken" } },
                            { "path": "desc.minReturnAmount", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "desc.dstToken" } },
                            { "path": "desc.dstReceiver", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                },
            },
        }),
        "0x111111125421ca6dc452d289314280a0f8842a65" => json!({
            "metadata": { "contractName": "1inch Router", "owner": "1inch" },
            "display": {
                "formats": {
                    "swap(address executor,(address srcToken,address dstToken,address srcReceiver,address dstReceiver,uint256 amount,uint256 minReturnAmount,uint256 flags) desc,bytes data)": {
                        "intent": "Swap",
                        "fields": [
                            { "path": "desc.amount", "label": "You pay", "format": "tokenAmount", "params": { "tokenPath": "desc.srcToken" } },
                            { "path": "desc.minReturnAmount", "label": "You receive (min)", "format": "tokenAmount", "params": { "tokenPath": "desc.dstToken" } },
                            { "path": "desc.dstReceiver", "label": "Recipient", "format": "addressName" },
                        ],
                    },
                },
            },
        }),
        "0xae7ab96520de3a18e5e111b5eaab095312d7fe84" => json!({
            "metadata": { "contractName": "Lido", "owner": "Lido" },
            "display": {
                "formats": {
                    "submit(address _referral)": { "intent": "Stake", "fields": [{ "path": "@.value", "label": "Stake", "format": "amount" }] },
                },
            },
        }),
        "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0" => json!({
            "metadata": { "contractName": "Wrapped stETH", "owner": "Lido", "constants": { "steth": "0xae7ab96520de3a18e5e111b5eaab095312d7fe84" } },
            "display": {
                "formats": {
                    "wrap(uint256 _stETHAmount)": {
                        "intent": "Wrap",
                        "fields": [{ "path": "_stETHAmount", "label": "Wrap", "format": "tokenAmount", "params": { "token": "$.metadata.constants.steth" } }],
                    },
                    "unwrap(uint256 _wstETHAmount)": {
                        "intent": "Unwrap",
                        "fields": [{ "path": "_wstETHAmount", "label": "Unwrap", "format": "tokenAmount", "params": { "tokenPath": "@.to" } }],
                    },
                },
            },
        }),
        "0x00000000000000adc04c56bf30ac9d3c0aaf14dc"
        | "0x0000000000000068f116a894984e2db1123eb395" => seaport_descriptor(),
        _ => return None,
    })
}

/// Interface-level token descriptors, keyed by standard
/// (local-descriptors.ts:392-524). `0x8000…` (2^255) is the "Unlimited"
/// threshold sentinel.
fn interface_descriptor(kind: TokenStandard) -> Value {
    const UNLIMITED: &str = "0x8000000000000000000000000000000000000000000000000000000000000000";
    match kind {
        TokenStandard::Erc20 => json!({
            "metadata": { "standard": "erc20" },
            "display": {
                "formats": {
                    "transfer(address to,uint256 amount)": {
                        "intent": "Send",
                        "fields": [
                            { "path": "amount", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" } },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "transferFrom(address from,address to,uint256 amount)": {
                        "intent": "Transfer",
                        "fields": [
                            { "path": "amount", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" } },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "approve(address spender,uint256 amount)": {
                        "intent": "Approve",
                        "fields": [
                            { "path": "amount", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to", "threshold": UNLIMITED } },
                            { "path": "spender", "label": "Spender", "format": "addressName" },
                        ],
                    },
                    "increaseAllowance(address spender,uint256 addedValue)": {
                        "intent": "Approve",
                        "fields": [
                            { "path": "addedValue", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" } },
                            { "path": "spender", "label": "Spender", "format": "addressName" },
                        ],
                    },
                    "decreaseAllowance(address spender,uint256 subtractedValue)": {
                        "intent": "Revoke",
                        "fields": [
                            { "path": "subtractedValue", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" } },
                            { "path": "spender", "label": "Spender", "format": "addressName" },
                        ],
                    },
                },
            },
        }),
        TokenStandard::Erc721 => json!({
            "metadata": { "standard": "erc721" },
            "display": {
                "formats": {
                    "transferFrom(address from,address to,uint256 tokenId)": {
                        "intent": "Transfer NFT",
                        "fields": [
                            { "path": "tokenId", "label": "Token ID", "format": "nftName" },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "safeTransferFrom(address from,address to,uint256 tokenId)": {
                        "intent": "Transfer NFT",
                        "fields": [
                            { "path": "tokenId", "label": "Token ID", "format": "nftName" },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "safeTransferFrom(address from,address to,uint256 tokenId,bytes data)": {
                        "intent": "Transfer NFT",
                        "fields": [
                            { "path": "tokenId", "label": "Token ID", "format": "nftName" },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "approve(address to,uint256 tokenId)": {
                        "intent": "Approve NFT",
                        "fields": [
                            { "path": "tokenId", "label": "Token ID", "format": "nftName" },
                            { "path": "to", "label": "Approved", "format": "addressName" },
                        ],
                    },
                    "setApprovalForAll(address operator,bool approved)": {
                        "intent": "Approve all NFTs",
                        "fields": [
                            { "path": "operator", "label": "Operator", "format": "addressName" },
                            { "path": "approved", "label": "Approved", "format": "raw" },
                        ],
                    },
                },
            },
        }),
        TokenStandard::Erc1155 => json!({
            "metadata": { "standard": "erc1155" },
            "display": {
                "formats": {
                    "safeTransferFrom(address from,address to,uint256 id,uint256 amount,bytes data)": {
                        "intent": "Transfer NFT",
                        "fields": [
                            { "path": "id", "label": "Token ID", "format": "nftName" },
                            { "path": "amount", "label": "Quantity", "format": "raw" },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "safeBatchTransferFrom(address from,address to,uint256[] ids,uint256[] amounts,bytes data)": {
                        "intent": "Transfer NFTs",
                        "fields": [
                            { "path": "ids", "label": "Token IDs", "format": "raw" },
                            { "path": "amounts", "label": "Quantities", "format": "raw" },
                            { "path": "from", "label": "From", "format": "addressName" },
                            { "path": "to", "label": "To", "format": "addressName" },
                        ],
                    },
                    "setApprovalForAll(address operator,bool approved)": {
                        "intent": "Approve all NFTs",
                        "fields": [
                            { "path": "operator", "label": "Operator", "format": "addressName" },
                            { "path": "approved", "label": "Approved", "format": "raw" },
                        ],
                    },
                },
            },
        }),
    }
}

// ---------------------------------------------------------------------------
// Decimal string arithmetic (no floats near money, no bignum dependency)
// ---------------------------------------------------------------------------

/// Hex digits → decimal string. `None` on empty/invalid (JS `BigInt('0x')`
/// throws; the callers' catch produced 0n / skip).
fn hex_to_dec(body: &str) -> Option<String> {
    if body.is_empty() {
        return None;
    }
    let mut digits: Vec<u8> = vec![0]; // least-significant first
    for c in body.chars() {
        let v = c.to_digit(16)?;
        let mut carry = v;
        for d in digits.iter_mut() {
            let cur = u32::from(*d) * 16 + carry;
            *d = (cur % 10) as u8;
            carry = cur / 10;
        }
        while carry > 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    while digits.len() > 1 && digits.last() == Some(&0) {
        digits.pop();
    }
    Some(digits.iter().rev().map(|d| char::from(b'0' + d)).collect())
}

fn dec_normalize(s: &str) -> String {
    let trimmed = s.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// `a >= b` over non-negative decimal strings (negative `a` is never ≥).
fn dec_ge(a: &str, b: &str) -> bool {
    if a.starts_with('-') {
        return false;
    }
    let (a, b) = (dec_normalize(a), dec_normalize(b));
    match a.len().cmp(&b.len()) {
        std::cmp::Ordering::Greater => true,
        std::cmp::Ordering::Less => false,
        std::cmp::Ordering::Equal => a >= b,
    }
}

/// `toBigInt` (clear-signing.ts:1160-1171) → a decimal string. JS numbers
/// (JSON numbers in typed data) fall through to 0n — ported quirk.
fn to_bigint(v: Option<&Ctx>) -> String {
    match v {
        None => "0".to_owned(),
        Some(Ctx::Null) => "0".to_owned(),
        Some(Ctx::Num(d)) => d.clone(),
        Some(Ctx::Bool(b)) => if *b { "1" } else { "0" }.to_owned(),
        Some(Ctx::Str(s)) => {
            let t = s.trim();
            if t.is_empty() {
                return "0".to_owned(); // BigInt('') === 0n
            }
            if let Some(body) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
                return hex_to_dec(body).unwrap_or_else(|| "0".to_owned());
            }
            let (sign, body) = match t.strip_prefix('-') {
                Some(rest) => ("-", rest),
                None => ("", t),
            };
            if !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit()) {
                let n = dec_normalize(body);
                if n == "0" {
                    n
                } else {
                    format!("{sign}{n}")
                }
            } else {
                "0".to_owned()
            }
        }
        Some(Ctx::JsNum(_)) | Some(Ctx::Arr(_)) | Some(Ctx::Map(_)) => "0".to_owned(),
    }
}

fn dec_to_f64(dec: &str) -> f64 {
    dec.parse::<f64>().unwrap_or(0.0)
}

/// A JSON-RPC result word as a small uint (for `decimals()`): oversized or
/// malformed answers are `None`, exactly as the `BigInt` try/catch skipped.
/// An ABI `string` return — what `symbol()` answers.
///
/// Two layouts, because ERC-20 predates the convention: the usual
/// `[offset][length][data]`, and the **bytes32** a legacy token (MKR and its
/// generation) returns as one fixed word. A declared length that does not fit
/// the payload is read as the second shape rather than trusted — an
/// out-of-range length is exactly what a bytes32 answer looks like to an
/// offset reader.
///
/// UTF-8 or nothing: a multibyte symbol (`USD₮0`) must survive, and a lossy
/// replacement is worse than the address fallback, because mojibake beside an
/// amount reads as a real symbol somebody has never heard of.
fn decode_abi_string(hex: &str) -> Option<String> {
    let data = hex.strip_prefix("0x").unwrap_or(hex);
    if data.len() < 64 {
        return None;
    }
    // A single word with no header: bytes32.
    if data.len() < 128 {
        return utf8_from_hex(data.get(..64)?);
    }
    let length = usize::from_str_radix(data.get(64..128)?.trim_start_matches('0'), 16).ok()?;
    let end = length.checked_mul(2).and_then(|len| len.checked_add(128));
    match end {
        Some(end) if length > 0 && length <= 4096 && end <= data.len() => {
            utf8_from_hex(data.get(128..end)?)
        }
        // Not offset-encoded after all — read the head as bytes32.
        _ => utf8_from_hex(data.get(..64)?),
    }
}

/// Hex bytes as UTF-8, stopping at the first NUL: a bytes32 answer is
/// zero-padded and the terminator is not part of the symbol.
fn utf8_from_hex(hex: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in 0..hex.len() / 2 {
        let byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    let text = String::from_utf8(bytes).ok()?;
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

fn quantity_as_small_uint(word: &str) -> Option<u32> {
    let body = word.strip_prefix("0x")?;
    if body.is_empty() || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let trimmed = body.trim_start_matches('0');
    if trimmed.len() > 8 {
        return None;
    }
    let v = if trimmed.is_empty() {
        0
    } else {
        u32::from_str_radix(trimmed, 16).ok()?
    };
    Some(v)
}

// ---------------------------------------------------------------------------
// Locale-preset formatting (ports of locale-format.ts helpers)
// ---------------------------------------------------------------------------

fn group_integer(digits: &str, group: char, indian: bool) -> String {
    let (sign, body) = match digits.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", digits),
    };
    if body.len() <= 3 {
        return digits.to_owned();
    }
    let grouped = if !indian {
        insert_every_from_right(body, 3, group)
    } else {
        let split = body.len() - 3;
        let (head, tail) = (&body[..split], &body[split..]);
        if head.is_empty() {
            tail.to_owned()
        } else {
            format!("{}{group}{tail}", insert_every_from_right(head, 2, group))
        }
    };
    format!("{sign}{grouped}")
}

fn insert_every_from_right(digits: &str, every: usize, sep: char) -> String {
    let bytes = digits.as_bytes();
    let mut out = String::with_capacity(digits.len() + digits.len() / every + 1);
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 && (bytes.len() - i) % every == 0 {
            out.push(sep);
        }
        out.push(char::from(*b));
    }
    out
}

/// `groupDigits` — bigint-safe integer grouping (locale-format.ts:162-165).
fn group_digits(digits: &str, locale: &ClearLocale) -> String {
    let (group, _, indian) = locale.separators();
    group_integer(digits, group, indian)
}

/// `formatNumber` (locale-format.ts:170-184).
fn format_number(value: f64, min_frac: usize, max_frac: usize, locale: &ClearLocale) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let (group, decimal, indian) = locale.separators();
    let min_frac = min_frac.min(max_frac);
    let sign = if value < 0.0 { "-" } else { "" };
    let fixed = format!("{:.*}", max_frac, value.abs());
    let (int_part, frac_part) = match fixed.split_once('.') {
        Some((i, f)) => (i, f),
        None => (fixed.as_str(), ""),
    };
    let mut frac = frac_part.to_owned();
    while frac.len() > min_frac && frac.ends_with('0') {
        frac.pop();
    }
    let grouped = group_integer(int_part, group, indian);
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{decimal}{frac}")
    }
}

/// `formatTokenValue` (clear-signing.ts:1197-1214): BigInt division, up to 4
/// significant fractional digits, trailing zeros trimmed.
/// What a `tokenAmount` reads when the decimals could not be verified.
///
/// An em dash, not a zero and not a guess. The shells may replace it with a
/// phrase of their own — they know `unverified` — but every shell that renders
/// the value as it comes still shows the absence rather than a wrong number.
pub const UNKNOWN_AMOUNT: &str = "—";

fn format_token_value(raw_dec: &str, decimals: u32, locale: &ClearLocale) -> String {
    let (sign, digits) = match raw_dec.strip_prefix('-') {
        Some(rest) => ("-", rest),
        None => ("", raw_dec),
    };
    let digits = dec_normalize(digits);
    if digits == "0" {
        return "0".to_owned();
    }
    let d = decimals as usize;
    let (whole, frac) = if digits.len() <= d {
        ("0".to_owned(), format!("{:0>width$}", digits, width = d))
    } else {
        let split = digits.len() - d;
        (digits[..split].to_owned(), digits[split..].to_owned())
    };
    let (_, decimal_sep, _) = locale.separators();
    let whole_grouped = group_digits(&whole, locale);
    if frac.bytes().all(|b| b == b'0') {
        return format!("{sign}{whole_grouped}");
    }
    let keep = 4.min(d);
    let trimmed = frac
        .get(..keep)
        .unwrap_or(&frac)
        .trim_end_matches('0')
        .to_owned();
    if trimmed.is_empty() {
        format!("{sign}{whole_grouped}")
    } else {
        format!("{sign}{whole_grouped}{decimal_sep}{trimmed}")
    }
}

/// `formatWeiAmount` (clear-signing.ts:1173-1181) — the native amount is
/// already a JS-number path, so f64 introduces no new loss.
fn format_wei_amount(raw_dec: &str, locale: &ClearLocale) -> String {
    if dec_normalize(raw_dec) == "0" {
        return "0".to_owned();
    }
    let eth = dec_to_f64(raw_dec) / 1e18;
    if eth >= 0.0001 {
        return format_number(eth, 0, 6, locale);
    }
    if !dec_ge(raw_dec, "1000000") {
        return format!("{raw_dec} wei");
    }
    format_number(eth, 0, 8, locale)
}

/// Stablecoin base units → USD magnitude, peg = $1, rounded to cents
/// (`usdMagnitude`, clear-signing.ts:1059-1065).
fn usd_magnitude(raw_dec: &str, decimals: u32) -> Option<f64> {
    if raw_dec.starts_with('-') {
        return None;
    }
    let digits = dec_normalize(raw_dec);
    let d = decimals as usize;
    let (whole, frac) = if digits.len() <= d {
        ("0".to_owned(), format!("{:0>width$}", digits, width = d))
    } else {
        let split = digits.len() - d;
        (digits[..split].to_owned(), digits[split..].to_owned())
    };
    let whole_f: f64 = whole.parse().ok()?;
    if d == 0 {
        return Some(whole_f);
    }
    let base = 10u128.checked_pow(decimals)?;
    let frac_n: u128 = frac.parse().ok()?;
    let cents = (frac_n.checked_mul(100)? + base / 2) / base;
    Some(whole_f + cents as f64 / 100.0)
}

/// Days-since-epoch → (year, month, day). Howard Hinnant's civil_from_days.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m as u32, d as u32)
}

/// `formatDateTime` (locale-format.ts:275-278) at the shell-provided offset.
fn format_date_time(ts_secs: i64, locale: &ClearLocale) -> String {
    let local = ts_secs + i64::from(locale.tz_offset_minutes) * 60;
    let days = local.div_euclid(86_400);
    let rem = local.rem_euclid(86_400);
    let (y, m, d) = civil_from_days(days);
    let (hh, mm) = ((rem / 3_600) as u32, ((rem % 3_600) / 60) as u32);
    let date = match locale.date_format {
        ClearDateFormat::YmdSlash => format!("{y}/{m:02}/{d:02}"),
        ClearDateFormat::Iso => format!("{y}-{m:02}-{d:02}"),
        ClearDateFormat::DmySlash => format!("{d:02}/{m:02}/{y}"),
        ClearDateFormat::DmyDot => format!("{d:02}.{m:02}.{y}"),
        ClearDateFormat::MdySlash => format!("{m:02}/{d:02}/{y}"),
    };
    let time = match locale.time_format {
        ClearTimeFormat::H24 => format!("{hh:02}:{mm:02}"),
        ClearTimeFormat::H12 => {
            let h12 = if hh % 12 == 0 { 12 } else { hh % 12 };
            format!("{h12}:{mm:02} {}", if hh < 12 { "AM" } else { "PM" })
        }
    };
    format!("{date}, {time}")
}

// ---------------------------------------------------------------------------
// Small string helpers (char-safe slicing)
// ---------------------------------------------------------------------------

fn take_chars(s: &str, from: usize, count: usize) -> String {
    s.chars().skip(from).take(count).collect()
}

fn last_chars(s: &str, count: usize) -> String {
    let total = s.chars().count();
    s.chars().skip(total.saturating_sub(count)).collect()
}

fn value_is_truthy(v: &Value) -> bool {
    match v {
        Value::Null => false,
        Value::Bool(b) => *b,
        Value::Number(n) => n.as_f64().is_some_and(|f| f != 0.0),
        Value::String(s) => !s.is_empty(),
        Value::Array(_) | Value::Object(_) => true,
    }
}

// ---------------------------------------------------------------------------
// Command plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must match the current attempt.
fn requests(model: &Model, operations: Vec<ClearOperation>) -> Command<ClearSigningEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<ClearSigningEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for ClearSigningEffect {
    type Op = ClearOperation;
    fn into_shell(self) -> Option<crux_core::Request<ClearOperation>> {
        match self {
            ClearSigningEffect::Render(_) => None,
            ClearSigningEffect::Shell(request) => Some(request),
        }
    }
}

#[cfg(test)]
mod to_own_token_tests {
    use super::*;

    fn field(role: ClearFieldRole, address: Option<&str>) -> ClearSignField {
        ClearSignField {
            label: "To".to_owned(),
            value: "0x…".to_owned(),
            format: "addressName".to_owned(),
            token_address: None,
            warning: false,
            unverified: false,
            role,
            detail: false,
            expired: false,
            address: address.map(str::to_owned),
            usd_value: None,
        }
    }

    fn result(contract: Option<&str>, fields: Vec<ClearSignField>) -> ClearSignResult {
        ClearSignResult {
            intent: "Send".to_owned(),
            contract_name: None,
            owner: None,
            fields,
            risk: ClearRisk::Normal,
            contract_address: contract.map(str::to_owned),
            verified: false,
            sign_type: ClearSignType::Transaction,
            partial: false,
            best_effort: false,
            to_own_token: false,
        }
    }

    const LOWER: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const EIP55: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

    /// The rule the whole field exists for, stated on its own terms — no
    /// builder, no ingest normalisation propping it up.
    #[test]
    fn case_never_decides_a_burn() {
        assert!(to_own_token(&result(
            Some(EIP55),
            vec![field(ClearFieldRole::Recipient, Some(LOWER))]
        )));
        assert!(to_own_token(&result(
            Some(LOWER),
            vec![field(ClearFieldRole::Recipient, Some(EIP55))]
        )));
        assert!(to_own_token(&result(
            Some(EIP55),
            vec![field(ClearFieldRole::Recipient, Some(EIP55))]
        )));
    }

    /// Only `recipient` counts. A spender equal to the contract grants an
    /// allowance; nothing moves, nothing burns.
    #[test]
    fn only_the_recipient_role_counts() {
        for role in [
            ClearFieldRole::Spender,
            ClearFieldRole::Generic,
            ClearFieldRole::SendAmount,
            ClearFieldRole::ReceiveAmount,
        ] {
            assert!(
                !to_own_token(&result(Some(EIP55), vec![field(role, Some(LOWER))])),
                "{role:?} must not read as a burn",
            );
        }
    }

    /// No contract, an empty contract, or a recipient with no resolved address
    /// is never a burn — the batch twin's `!leg.to.is_empty()` guard.
    #[test]
    fn missing_facts_are_never_a_burn() {
        assert!(!to_own_token(&result(
            None,
            vec![field(ClearFieldRole::Recipient, Some(LOWER))]
        )));
        assert!(!to_own_token(&result(
            Some(""),
            vec![field(ClearFieldRole::Recipient, Some(""))]
        )));
        assert!(!to_own_token(&result(
            Some(LOWER),
            vec![field(ClearFieldRole::Recipient, None)]
        )));
        assert!(!to_own_token(&result(Some(LOWER), vec![])));
    }
}

#[cfg(test)]
mod symbol_probe_tests {
    use super::{decode_abi_string, ClearProbe};

    /// The two shapes `symbol()` answers in.
    ///
    /// ERC-20 predates the string convention, so a legacy token returns one
    /// fixed word instead of an offset. Reading a bytes32 answer as an offset
    /// produces either nothing or garbage, and garbage beside an amount reads
    /// as a real symbol nobody has heard of.
    #[test]
    fn a_symbol_decodes_from_either_layout() {
        // Dynamic: offset(0x20), length(4), "USDC" padded.
        let dynamic = "0x\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000004\
            5553444300000000000000000000000000000000000000000000000000000000";
        assert_eq!(decode_abi_string(dynamic).as_deref(), Some("USDC"));

        // bytes32, the MKR generation: one word, zero-padded.
        let fixed = "0x4d4b520000000000000000000000000000000000000000000000000000000000";
        assert_eq!(decode_abi_string(fixed).as_deref(), Some("MKR"));
    }

    /// A multibyte symbol survives; anything unreadable answers `None` so the
    /// address fallback stands rather than mojibake.
    #[test]
    fn only_real_utf8_teaches_the_cache() {
        // "USD₮0" — three-byte ₮ inside a dynamic string.
        let multi = "0x\
            0000000000000000000000000000000000000000000000000000000000000020\
            0000000000000000000000000000000000000000000000000000000000000007\
            555344e282ae30000000000000000000000000000000000000000000000000000";
        assert_eq!(decode_abi_string(multi).as_deref(), Some("USD₮0"));

        // A revert, an empty answer, and a word that is not UTF-8.
        assert_eq!(decode_abi_string("0x"), None);
        assert_eq!(decode_abi_string(""), None);
        let invalid = "0xfffe000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(decode_abi_string(invalid), None);
        // All zeroes is a padded empty string, not the symbol "".
        let empty = "0x0000000000000000000000000000000000000000000000000000000000000000";
        assert_eq!(decode_abi_string(empty), None);
    }

    /// The probe is a distinct question, so an answer to it can never be read
    /// as an answer to the decimals one.
    #[test]
    fn the_symbol_probe_is_its_own_question() {
        assert_ne!(ClearProbe::Symbol, ClearProbe::Decimals);
    }
}
