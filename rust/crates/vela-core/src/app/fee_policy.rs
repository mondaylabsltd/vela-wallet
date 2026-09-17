//! Machine — fee policy (spec `017`, inventory `### fee_policy (P1)`).
//!
//! The quote lifecycle plus the pure money math that the Send flow and the
//! dApp signing flow share (`GasFeeCard`/`FeeTokenSelector` are one component
//! serving both). One machine, so the "displayed = signed" chain can never be
//! split into two diverging implementations.
//!
//! ```text
//! QuoteRequested{fee_token} ─► Gathering ──(gas price ∥ bundler quote ∥
//!   in-band quotes)──► Estimating ──(real-calldata UserOp simulation, fee leg
//!   included)──► Quoted ──30s──► stale
//!        │                                              │
//!        └── any fatal step ─► Failed{semantic variant}  └─ SelectFeeAsset:
//!                                                           local recompute
//!                                                           (never on Tempo)
//! ```
//!
//! The fee token is a QUOTE PARAMETER, carried on `QuoteRequested` and fixed
//! before anything is simulated, because it changes the operation being
//! priced: a stablecoin fee leg is `token.transfer(recipient, amount)`, 68
//! bytes and one ERC-20 SSTORE heavier than a native `{to, value}` leg.
//! `SelectFeeAsset` is the in-place chip switch on top of a settled generic
//! in-band quote, and only there — Tempo re-prices through its own model.
//!
//! Faithful port of the TypeScript sources — behavior aligned line by line,
//! magic numbers and wording classifications preserved:
//!
//! - `src/services/safe-transaction.ts:242-434` — tier multipliers, in-band
//!   pricing (USD 8-dp fixed point), `sameAssetFeeLimit`, `rawBundlerGasCost`
//! - `src/services/safe-transaction.ts:551-765` — estimation orchestration
//! - `src/services/safe-transaction.ts:1918-2086` — gas-price derivation and
//!   the bundler quote acceptance rules (zero quote ⇒ cannot quote)
//! - `src/services/tempo.ts` — the whole Tempo stablecoin gas model
//! - `src/services/batch-send.ts:136-201` — reserve math + string-exact Max
//! - `src/hooks/use-inband-fee-tokens.ts` — fee-asset option shaping
//! - `src/components/ui/GasFeeCard.tsx` — requote loop + local chip switch
//! - `src/components/ui/FeeTokenSelector.tsx:74` — the balance<fee gate
//! - `src/screens/wallet/useSendController.ts:116-166, 468-473, 718-786` —
//!   quote/chain validity and the leave-confirm reset
//!
//! Ported quirks, kept verbatim (see inventory open questions):
//!
//! - `safe-transaction.ts:257` annotates `BUNDLER_MARGIN_NUM` with `// 150`,
//!   but `100 + BUNDLER_MARGIN_PERCENT(100)` is **200**; the jest vectors
//!   (`calcMaxFeePerGas` ×2.0) prove 200 is the shipped behavior. 200 here.
//! - `isTempoChain` also consults `chainMeta(id)?.gasModel === 'tempo'`; the
//!   chain registry is shell master data, so the core owns only the static id
//!   set {4217, 42431}. A registry-added Tempo chain needs a core update.
//! - The 8s in-band quote cache and its in-flight coalescing stay in the shell
//!   executor (`bundler-service.ts:588-640`), as the inventory prescribes.
//!
//! Deliberate strictness where TS bigint semantics cannot map onto `u128`
//! (all explicit, all fail toward *never undercharging*):
//!
//! - `to_base_units` returns `None` for negative or non-numeric input where
//!   `BigInt(...)` would produce a negative value or throw mid-render.
//! - Every money product/quotient is evaluated in 256-bit space (`U256`, the
//!   same width `approval_guard` uses) and only the FINAL value is narrowed to
//!   `u128`. A result that does not fit is a refusal (`None` → a `FeeFailure`)
//!   or, where the signature cannot carry one, `u128::MAX` — an unpayable
//!   number the balance gate then rejects out loud. See the arithmetic section
//!   below for why an intermediate clamp is the one thing that is forbidden.
//! - An unparseable USD price string prices as `None` ("cannot quote"), the
//!   same refusal the TS regex produces.

use alloy_primitives::U256;
use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants — every value mirrors the TS source it is named after
// ---------------------------------------------------------------------------

/// `BUNDLER_MARGIN_PERCENT = 100` → ×2.0 markup (`safe-transaction.ts:256-259`;
/// the stale `// 150` comment there is wrong — the jest vectors pin ×2.0).
const BUNDLER_MARGIN_NUM: u128 = 200;
const BUNDLER_MARGIN_DEN: u128 = 100;

/// In-band reimbursement markup: gas basis × 3 (`safe-transaction.ts:336`).
const INBAND_MARKUP: u128 = 3;

/// A bundler gas quote is refused, not signed, when it exceeds the client's
/// OWN on-chain gas measurement (`derive_chain_gas_price`) by more than this
/// multiple. A quote that far above real cost is not one we pay 3× on top of;
/// rejecting it protects the user from a runaway or hostile relayer quote
/// (requirement G05 / manual-test clue #14, "GasQuoteTooHigh").
const MAX_QUOTE_VS_CHAIN_MULTIPLE: u128 = 3;

/// USD prices are fixed-point with 8 decimals (`safe-transaction.ts:337-339`).
pub const USD_PRICE_DECIMALS: u32 = 8;
const USD_PRICE_SCALE: u128 = 100_000_000;
/// $0.01 — the stablecoin fee floor (`safe-transaction.ts:340`).
const STABLE_MIN_USD_SCALED: u128 = USD_PRICE_SCALE / 100;

// Static gas model (`safe-transaction.ts:39-52`).
const VERIFICATION_GAS_DEPLOYED: u128 = 300_000;
const VERIFICATION_GAS_UNDEPLOYED: u128 = 2_000_000;
const CALL_GAS_LIMIT: u128 = 200_000;
const PRE_VERIFICATION_GAS: u128 = 100_000;
/// EVM identity precompile — targeting the Safe itself would revert
/// (`safe-transaction.ts:46, 616-624`).
const ESTIMATION_DUMMY_TARGET: &str = "0x0000000000000000000000000000000000000004";
/// Same payload size as ERC-20 transfer calldata (`safe-transaction.ts:47`).
const ESTIMATION_DUMMY_DATA_LENGTH: usize = 68;
/// Above this calldata size a failed simulation must surface instead of the
/// static fallback, which would show a misleading number for an op the submit
/// path would refuse anyway (`safe-transaction.ts:52, 703-713`).
const ESTIMATION_REQUIRED_CALLDATA: usize = 1024;

// L2 rollup data-fee adders for the static fallback (`safe-transaction.ts:723-731`).
const ARBITRUM_CHAIN_IDS: [u32; 2] = [42_161, 421_614];
const OP_STACK_CHAIN_IDS: [u32; 4] = [10, 8_453, 11_155_420, 84_532];
const ARBITRUM_STATIC_GAS_ADDER: u128 = 600_000;
const OP_STACK_STATIC_GAS_ADDER: u128 = 150_000;

/// 5 gwei — the gas-price fallback when `eth_gasPrice` fails
/// (`safe-transaction.ts:1983`).
const FALLBACK_GAS_PRICE_WEI: u128 = 5_000_000_000;

/// Arc mainnet (5042) and its testnet (5042002) enforce a 20 gwei minimum base
/// fee (spec 060).
const ARC_CHAIN_IDS: [u32; 2] = [5_042, 5_042_002];
/// Arc's minimum base fee, in wei of its 18-decimal native USDC.
const ARC_MIN_GAS_PRICE_WEI: u128 = 20_000_000_000;

/// The lowest gas price a chain will ACCEPT.
///
/// On most chains an underpriced transaction sits in the mempool and can be
/// replaced — visibly pending, recoverable. Arc does not do that: a
/// `maxFeePerGas` below its 20 gwei floor is **discarded silently**, with no
/// error to the sender and no trace on chain. The payment looks submitted and
/// simply never happens, which is the worst failure this wallet can produce.
///
/// The trigger is not exotic. [`FALLBACK_GAS_PRICE_WEI`] is 5 gwei, so any Arc
/// quote taken after a failed `eth_gasPrice` read would land at 10 gwei once
/// the ×2 bundler margin is applied — under the floor, every time.
///
/// `0` for every other chain, which makes the floor a no-op there: the
/// twelve pre-existing networks must price bit-identically to before.
pub const fn min_gas_price_wei(chain_id: u32) -> u128 {
    if ARC_CHAIN_IDS[0] == chain_id || ARC_CHAIN_IDS[1] == chain_id {
        ARC_MIN_GAS_PRICE_WEI
    } else {
        0
    }
}

/// Quote TTL. The inventory's `Timer(8s/30s TTL)`: the 8s response cache is
/// the shell's; this 30s marks a *displayed* quote stale so the surface can
/// offer a refresh. Staleness is advisory — it does not disable confirm,
/// because today's UI does not either; the submit-side guards
/// (`tempo_quote_is_stale`, the bundler's in-band gate) reject a genuinely
/// expired quote loudly.
const QUOTE_TTL_MS: u32 = 30_000;

// Tempo gas model (`src/services/tempo.ts`, constants kept verbatim).
/// Tempo mainnet (4217) + Moderato testnet (42431) (`tempo.ts:33`).
pub const TEMPO_CHAIN_IDS: [u32; 2] = [4_217, 42_431];
/// pathUSD, in the reserved TIP-20 0x20c0… range (`tempo.ts:44`).
pub const TEMPO_DEFAULT_FEE_TOKEN: &str = "0x20c0000000000000000000000000000000000000";
/// Every Tempo TIP-20 USD stablecoin uses 6 decimals (`tempo.ts:47`).
pub const TEMPO_FEE_TOKEN_DECIMALS: u32 = 6;
/// Never below one cent (`tempo.ts:50`).
pub const TEMPO_MIN_FEE_USD_CENTS: u128 = 1;
/// 20e9 attodollars/gas protocol base-fee fallback (`tempo.ts:66`).
pub const TEMPO_BASE_FEE_ATTO: u128 = 20_000_000_000;
/// Outer 0x76 overhead beyond the UserOp limits (`tempo.ts:69`).
pub const TEMPO_OUTER_OVERHEAD_GAS: u128 = 150_000;
/// TIP-20 transfers meter ~308k; 380k = measured + headroom (`tempo.ts:80`).
pub const TEMPO_CALL_GAS_PER_SUBCALL: u128 = 380_000;
/// 2× charge = 100% margin over realistic cost (`tempo.ts:100-101`).
pub const TEMPO_FEE_MARGIN_NUM: u128 = 2;
pub const TEMPO_FEE_MARGIN_DEN: u128 = 1;
/// Realistic gas measured on Tempo mainnet (`tempo.ts:109-111`).
pub const TEMPO_DEPLOYED_GAS_EST: u128 = 250_000;
pub const TEMPO_DEPLOY_GAS_EST: u128 = 4_150_000;
pub const TEMPO_PER_SUBCALL_GAS_EST: u128 = 110_000;
/// Bundler-side accept-check buffer — must match vela-relay (`tempo.ts:160`).
pub const TEMPO_COST_BUFFER_GAS: u128 = 80_000;
/// Flat EOA-floor cushion (`tempo.ts:164`).
pub const TEMPO_SPLIT_SAFETY_GAS: u128 = 20_000;
/// Proportional cushion, basis points — the deploy-rejection fix
/// (`tempo.ts:167-175`: reimbursed=89700 < cost=90025 incident).
pub const TEMPO_SPLIT_SAFETY_BPS: u128 = 300;

// ---------------------------------------------------------------------------
// Money arithmetic — exact in 256 bits, narrowed once, never clamped midway
// ---------------------------------------------------------------------------
//
// TS `bigint` cannot overflow, so the port has to answer for every product the
// canon takes for granted. The rule that matters is NOT "clamp upward": it is
// **never clamp an intermediate**.
//
// The formula this module is built around is `ceil(a × b × c / (d × e))`. A
// clamp applied to the numerator is silently undone by the division: for an
// 18-decimal fee asset (DAI, and USDT/USDC on BNB Chain) the true numerator is
// ~1.26e46, twenty-four decimal orders past `u128::MAX`, and `MAX / 1e26`
// lands on 3.4e12 — a number small enough to disappear under the $0.01 floor.
// That shipped as a 12,600× undercharge while every gate stayed green, because
// every vector and every scenario used a 6-decimal token.
//
// So: evaluate in `U256` (1.16e77 — the width `approval_guard` already uses,
// no new dependency), and narrow exactly once, at the end.
//   * where the signature can refuse — `Option` — a value that does not fit is
//     `None`, which the machine turns into a `FeeFailure`;
//   * where it cannot, the final narrow clamps to `u128::MAX`, an amount no
//     balance can cover, so `fee_row_insufficient` refuses it out loud.
// Both are "expensive and refused". Neither can be mistaken for a real price.

/// Widen. Named for brevity — this file is dense with conversions.
fn w(value: u128) -> U256 {
    U256::from(value)
}

/// `a × b` for two `u128`s. `(2^128 − 1)^2 < 2^256`, so this is EXACT and
/// total: no clamp, no panic, no `checked_` ceremony at the call sites.
fn mul_wide(a: u128, b: u128) -> U256 {
    w(a) * w(b)
}

/// `10^d` in 256-bit space. `None` past `10^77`, where not even `U256` can
/// hold the unit — a refusal, never a clamped unit that would deflate a
/// quotient.
fn pow10_wide(d: u32) -> Option<U256> {
    U256::from(10u64).checked_pow(U256::from(d))
}

/// The one narrowing point. `None` when the exact value does not fit.
fn narrow(value: U256) -> Option<u128> {
    u128::try_from(value).ok()
}

/// Narrow for the signatures that cannot carry a refusal (the parity corpus
/// pins them as plain `u128`, and the TypeScript half returns a `bigint`).
/// `u128::MAX` is unpayable by construction, so it surfaces as "this asset
/// cannot cover the fee" rather than as a price.
fn narrow_saturating(value: U256) -> u128 {
    u128::try_from(value).unwrap_or(u128::MAX)
}

/// `ceilDiv` (`safe-transaction.ts:342-344`) in 256-bit space. A zero
/// denominator is unreachable (zero USD prices are filtered as unpriceable
/// first) and answers `None` rather than panicking.
fn ceil_div_wide(numerator: U256, denominator: U256) -> Option<U256> {
    if denominator.is_zero() {
        return None;
    }
    Some(numerator.checked_add(denominator - U256::from(1u64))? / denominator)
}

/// Attodollars are USD × 1e-18 (`tempo.ts:117-126`).
const ATTO_SCALE: u128 = 1_000_000_000_000_000_000;

/// Attodollars → fee-token base units, exact. `None` when the token's unit or
/// the product cannot be represented at all.
fn atto_units_wide(atto: U256, decimals: u32) -> Option<U256> {
    if atto.is_zero() {
        return Some(U256::ZERO);
    }
    let unit = pow10_wide(decimals)?;
    Some(atto.checked_mul(unit)? / w(ATTO_SCALE))
}

/// Saturating addition. Unlike a product, a sum cannot be pulled back into a
/// plausible range by a later division in this module — every `add` here feeds
/// a `max`/`+` chain of gas units, never a numerator that gets divided down.
fn add(a: u128, b: u128) -> u128 {
    a.saturating_add(b)
}

/// `10^d` for the one call site whose `d` is a compile-time constant
/// (`18 − TEMPO_FEE_TOKEN_DECIMALS` = 12) and therefore provably exact. Every
/// wire-supplied `decimals` goes through [`pow10_wide`], which refuses instead
/// of clamping.
fn pow10(d: u32) -> u128 {
    10u128.checked_pow(d).unwrap_or(u128::MAX)
}

fn parse_units(s: &str) -> Option<u128> {
    s.trim().parse::<u128>().ok()
}

fn is_hex_address(s: &str) -> bool {
    s.len() == 42 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// Gas speed tier (`safe-transaction.ts:242-249`). Labels are UI words and
/// stay in the shell. Both flows run `fast` today; the vocabulary is kept
/// because the multiplier table is load-bearing math.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeTier {
    Slow,
    Standard,
    Rapid,
    #[default]
    Fast,
}

/// `GAS_TIER_MULTIPLIERS` (`safe-transaction.ts:244-249`), as (num, den).
pub fn tier_multiplier(tier: FeeTier) -> (u128, u128) {
    match tier {
        FeeTier::Slow => (11, 10),
        FeeTier::Standard => (12, 10),
        FeeTier::Rapid => (15, 10),
        FeeTier::Fast => (20, 10),
    }
}

/// One call of the transaction being priced — the REAL calldata shape, so the
/// estimate prices the actual send (invariant ⑨; the padded rough model
/// over-charged ~8× on Arbitrum, `useSendController.ts:724-753`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeCall {
    pub to: String,
    /// Wei as a decimal string (never a JSON number).
    pub value: String,
    /// 0x-hex calldata; `"0x"` for a plain transfer.
    pub data: String,
}

/// One fee-asset row of `vela_getInBandGasQuote`
/// (`bundler-service.ts:570-584`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeAssetQuote {
    pub recipient: String,
    pub asset: FeeAssetKind,
    /// `None` = the native coin; else the stablecoin contract.
    pub fee_token: Option<String>,
    /// Base units as a decimal string.
    pub balance: String,
    pub decimals: u32,
    pub symbol: String,
    /// Decimal strings preserved so conversion never loses precision.
    pub usd_balance: String,
    /// `None` when this network has no price source. Native gas still works.
    pub usd_price: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeAssetKind {
    Native,
    Erc20,
}

/// The requested tier's row of `pimlico_getUserOperationGasPrice`
/// (`safe-transaction.ts:2030-2094`), numbers as decimal strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeBundlerQuote {
    pub max_fee_per_gas: String,
    /// `None` when a generic bundler omits the Vela extension fields.
    pub network_fee_per_gas: Option<String>,
    pub relayer_fee_per_gas: Option<String>,
}

/// `eth_estimateUserOperationGas` outcome. `ContextUnavailable` is the shell
/// reporting it could not build a truthful dummy op (nonce/initCode) — "a
/// nonce failure is not permission to simulate as nonce 0"
/// (`safe-transaction.ts:585-588, 1177`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeGasOutcome {
    Estimated {
        verification_gas_limit: String,
        call_gas_limit: String,
        pre_verification_gas: String,
    },
    SimulationFailed,
    ContextUnavailable,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. The shell owns transports,
/// RPC pooling, the 8s quote cache and the 15s estimate timeout — it answers
/// each sentence exactly once (a timeout answers with the failure variant).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeOperation {
    /// The three chain price signals (`safe-transaction.ts:1949-1985`).
    /// `want_tip` is false on Tempo, where `eth_maxPriorityFeePerGas` is
    /// meaningless and would corrupt the stablecoin reimbursement.
    FetchGasPrice { chain_id: u32, want_tip: bool },
    /// `pimlico_getUserOperationGasPrice` for one tier.
    FetchBundlerQuote { chain_id: u32, tier: FeeTier },
    /// `vela_getInBandGasQuote` — every fee asset in one address-only call.
    FetchInBandQuotes { chain_id: u32, account: String },
    /// Tempo settlement recipient (`fetchBundlerAccountInfo`:
    /// `settlementRecipient ?? depositAddress`, `safe-transaction.ts:461-466`).
    FetchFeeRecipient { chain_id: u32, account: String },
    /// Simulate the REAL batch (user calls + fee leg). The shell builds the
    /// dummy op with true nonce/initCode context.
    EstimateUserOpGas {
        chain_id: u32,
        account: String,
        deployed: bool,
        calls: Vec<FeeCall>,
    },
    /// Quote staleness timer.
    StartTtl { ms: u32 },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeShellResult {
    /// `None` fields are failed/skipped reads; only a failed `eth_gasPrice`
    /// triggers the 5-gwei default, exactly as `getGasPrices` degrades.
    GasPrice {
        eth_gas_price: Option<String>,
        base_fee: Option<String>,
        priority_fee: Option<String>,
    },
    /// `None` = method unsupported / transport failure → local fallback.
    BundlerQuote {
        quote: Option<FeeBundlerQuote>,
    },
    /// `None` = unavailable/malformed response (`fetchInBandGasQuotes`).
    InBandQuotes {
        quotes: Option<Vec<FeeAssetQuote>>,
    },
    FeeRecipient {
        recipient: Option<String>,
    },
    UserOpGas {
        outcome: FeeGasOutcome,
    },
    TtlElapsed,
}

impl Operation for FeeOperation {
    type Output = FeeShellResult;
}

#[effect]
pub enum FeeEffect {
    Render(RenderOperation),
    Shell(FeeOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeEvent"))]
pub enum Event {
    /// Confirm is about to open (send) or a signing request arrived (dApp).
    ///
    /// `fee_token` is the asset the caller wants this quote denominated in
    /// (`None` = native — the SigningSheet reset,
    /// `SigningSheet.tsx:247-249`, is the caller passing `None`). It is a
    /// QUOTE PARAMETER, not a post-quote adjustment: the fee leg that gets
    /// batched into the simulated UserOp is a native transfer for `None` and
    /// an ERC-20 `transfer` for `Some` — 68 bytes of calldata and one real
    /// SSTORE apart. Quoting the native shape and then re-denominating would
    /// price a DIFFERENT operation than the one `sendUserOpInBand` submits,
    /// which is the exact class of mismatch this machine exists to prevent
    /// (invariant ⑨; `estimateTransactionFee` passes the same `gasFeeToken`
    /// into `buildInBandFeeLeg`, `safe-transaction.ts:665-668`).
    ///
    /// `deployed` and `public_key_available` are account context the caller
    /// already holds; an undeployed account without its public key can never
    /// build the real initCode, so it must never estimate (invariant ⑤,
    /// `safe-transaction.ts:634-642`).
    QuoteRequested {
        chain_id: u32,
        account: String,
        deployed: bool,
        public_key_available: bool,
        tier: FeeTier,
        calls: Vec<FeeCall>,
        /// `None` = native. On Tempo, `None` means the default TIP-20
        /// (`estimateTempoFee`'s `gasFeeToken ?? TEMPO_DEFAULT_FEE_TOKEN`).
        fee_token: Option<String>,
    },
    /// A fee-asset chip tap. `None` = native.
    SelectFeeAsset { token: Option<String> },
    /// The refresh affordance (`GasFeeCard.handleRefresh`).
    Requote,
    /// Leaving the confirm step (`useSendController.ts:467-473`).
    LeaveConfirm,
    /// The form now targets a different chain — every earlier quote is
    /// invalid for it (invariant ①, `useSendController.ts:119-121`).
    ChainChanged { chain_id: u32 },
    /// External staleness signal (e.g. app resumed after a long background).
    QuoteExpired,
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made; an older attempt belongs to a superseded run and
    /// is dropped — this IS the "late old-chain quote never pollutes the new
    /// form" rule.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: FeeShellResult,
    },
}

// ---------------------------------------------------------------------------
// Failure vocabulary — semantic variants, words stay in the shell
// ---------------------------------------------------------------------------

/// Why there is no quote. One variant per distinct user-facing message in the
/// TS sources; the shell maps them to its i18n keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeFailure {
    /// 'Could not load the passkey public key required to build the
    /// UserOperation initCode' (`safe-transaction.ts:641`).
    MissingPublicKey,
    /// 'The gas relayer cannot quote the selected fee token right now.'
    /// (`safe-transaction.ts:658`).
    FeeTokenUnavailable,
    /// 'Could not load the in-band gas quote.' (`safe-transaction.ts:660`).
    QuoteUnavailable,
    /// 'Could not calculate the in-band gas fee.' (`safe-transaction.ts:739`).
    CalculationFailed,
    /// Gas estimation failed and no honest fallback exists (large calldata,
    /// missing account context, or a Tempo contract call).
    EstimateFailed,
    /// The bundler quoted a gas price more than `MAX_QUOTE_VS_CHAIN_MULTIPLE`
    /// times the client's own on-chain measurement — refused rather than
    /// signed, to protect the user from a runaway or hostile relayer quote.
    GasQuoteTooHigh,
}

// ---------------------------------------------------------------------------
// Pure money math — line-by-line ports, shared with the send/sign machines
// ---------------------------------------------------------------------------

/// The pricing facts of one asset — `Pick<InBandGasQuote, 'asset' | 'decimals'
/// | 'usdPrice'>` in the TS signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetPricing {
    pub is_native: bool,
    pub decimals: u32,
    pub usd_price: Option<String>,
}

/// Decimal string → USD 8-dp fixed point (`safe-transaction.ts:346-357`).
/// Rounding native **up** and the fee token **down** is what makes conversion
/// never undercharge (invariant ②).
pub fn usd_price_scaled(value: Option<&str>, round_up: bool) -> Option<u128> {
    let value = value?.trim();
    // `^(\d+)(?:\.(\d+))?$` — a bare trailing dot or any non-digit fails.
    let (integer, fraction) = match value.split_once('.') {
        Some((i, f)) => (i, f),
        None => (value, ""),
    };
    if integer.is_empty() || !integer.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if value.contains('.') && (fraction.is_empty() || !fraction.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let integer: u128 = integer.parse().ok()?;
    let mut kept: String = fraction.chars().take(USD_PRICE_DECIMALS as usize).collect();
    while kept.len() < USD_PRICE_DECIMALS as usize {
        kept.push('0');
    }
    let kept: u128 = kept.parse().ok()?;
    // Exact. A price whose scaled form does not fit `u128` is unpriceable, not
    // clamped: it is the DENOMINATOR of the conversion for a fee token and the
    // NUMERATOR for the native coin, so a clamp would move the quote in
    // opposite directions depending on which side asked. `None` = "cannot
    // quote", the same answer the TS regex gives for garbage.
    let mut scaled = w(integer)
        .checked_mul(w(USD_PRICE_SCALE))?
        .checked_add(w(kept))?;
    let tail_has_digit = fraction
        .bytes()
        .skip(USD_PRICE_DECIMALS as usize)
        .any(|b| (b'1'..=b'9').contains(&b));
    if round_up && tail_has_digit {
        scaled = scaled.checked_add(U256::from(1u64))?;
    }
    narrow(scaled)
}

/// Exact in-band reimbursement from the transaction's gas basis
/// (`safe-transaction.ts:359-390`). `requiredAmount` from the RPC is
/// intentionally not used. The native minimum is value-consistent — $0.01 worth
/// of native (never below the 0.00001-coin admission floor), or a flat 0.001-coin
/// fallback when the coin is unpriced — so a native reimbursement can still be
/// computed without a price. The STABLECOIN path, however, returns `None` when it
/// cannot price safely: a zero/absent USD price is "cannot quote", never rate 1
/// (invariant ④'s sibling: 0 is not a price).
///
/// Every step runs in `U256`, because the numerator here is where the port's
/// worst bug lived: for an 18-decimal fee asset the exact numerator is ~1.26e46
/// (`6.3e16 × 2e11 × 1e18` for a 700k-gas send at 30 gwei with ETH at $2,000),
/// and clamping it to `u128::MAX` before dividing by `1e26` produced 3.4e12 —
/// under the $0.01 floor, i.e. a 126 DAI fee quoted as one cent. The only
/// honest answers are the exact one or a refusal; `None` is the refusal, and
/// the machine turns it into `FeeFailure::CalculationFailed`.
pub fn calculate_in_band_fee_amount(
    total_gas: u128,
    gas_price: u128,
    fee_asset: &AssetPricing,
    native_asset: &AssetPricing,
) -> Option<u128> {
    if !native_asset.is_native {
        return None;
    }
    let native_unit = pow10_wide(native_asset.decimals)?;
    // 0.00001 native coin — the relay's admission floor (`admission.rs`); the
    // client never reimburses below it. Below 5 decimals it collapses to one
    // base unit.
    let admission_floor = if native_asset.decimals >= 5 {
        pow10_wide(native_asset.decimals - 5)?
    } else {
        U256::from(1u64)
    };
    // Client-self-imposed value floor, harmonizing the native minimum with the
    // stablecoin $0.01 floor: reimburse at least $0.01 worth of the native coin
    // when we can price it — but never below the 0.00001-coin admission floor (on
    // a coin dearer than $1000, $0.01 is worth *less* than 0.00001 of it, and the
    // admission floor must still be met). With no native USD price we cannot value
    // it, so a flat 0.001-coin blind floor applies (below 3 decimals, one base
    // unit). `!nativeUsdPrice` is falsy for 0 too — a zero price is unpriceable.
    let native_usd = usd_price_scaled(native_asset.usd_price.as_deref(), true).filter(|v| *v != 0);
    let native_minimum = match native_usd {
        Some(price) => ceil_div_wide(w(STABLE_MIN_USD_SCALED).checked_mul(native_unit)?, w(price))?
            .max(admission_floor),
        None if native_asset.decimals >= 3 => pow10_wide(native_asset.decimals - 3)?,
        None => U256::from(1u64),
    };
    let native_amount = mul_wide(total_gas, gas_price)
        .checked_mul(w(INBAND_MARKUP))?
        .max(native_minimum);
    if fee_asset.is_native {
        return narrow(native_amount);
    }
    // Stablecoin path needs the native USD price (parsed above) and the fee
    // token's; either absent/zero is "cannot quote", never rate 1.
    let native_usd = native_usd?;
    let fee_usd = usd_price_scaled(fee_asset.usd_price.as_deref(), false).filter(|v| *v != 0)?;
    let fee_unit = pow10_wide(fee_asset.decimals)?;
    let converted = ceil_div_wide(
        native_amount
            .checked_mul(w(native_usd))?
            .checked_mul(fee_unit)?,
        native_unit.checked_mul(w(fee_usd))?,
    )?;
    let stable_minimum =
        ceil_div_wide(w(STABLE_MIN_USD_SCALED).checked_mul(fee_unit)?, w(fee_usd))?;
    narrow(converted.max(stable_minimum))
}

/// Invariant ⑧ (`FeeTokenSelector.tsx:74`): a fee asset that cannot cover this
/// transaction's fee is shown for context but is NOT selectable — paying gas in
/// it would only produce a doomed op. An unpriceable asset (`None`) is just as
/// unselectable as a short one. Replayed against `FeeTokenSelector`'s own copy
/// by the fee-policy parity corpus.
pub fn fee_row_insufficient(balance: u128, amount: Option<u128>) -> bool {
    amount.is_none_or(|a| balance < a)
}

/// What the fee costs and what may still be transferred when the transfer and
/// its fee draw from the same asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FeeLimit {
    pub fee_amount: u128,
    pub max_transfer_amount: u128,
}

/// `sameAssetFeeLimit` (`safe-transaction.ts:300-332`): `None` when the fee
/// uses a different asset, so a caller must not apply this reserve to an
/// unrelated token balance (invariant ⑧'s cousin on the amount screen).
pub fn same_asset_fee_limit(
    fee: Option<&FeeEstimate>,
    transfer_asset: Option<&str>,
    balance: u128,
) -> Option<FeeLimit> {
    let fee = fee?;
    let fee_amount = match &fee.fee_asset {
        FeeAsset::Erc20 { token, amount, .. } => {
            let transfer = transfer_asset?;
            if !token.eq_ignore_ascii_case(transfer) {
                return None;
            }
            *amount
        }
        // Legacy native estimates consume the native asset via totalWei.
        FeeAsset::Native => {
            if transfer_asset.is_some() {
                return None;
            }
            fee.total_wei
        }
    };
    Some(FeeLimit {
        fee_amount,
        max_transfer_amount: balance.saturating_sub(fee_amount),
    })
}

/// `rawBundlerGasCost` (`safe-transaction.ts:424-427`): divide the tier
/// markup back out so a funding pre-check compares against what the bundler
/// actually requires.
pub fn raw_bundler_gas_cost(fee: &FeeEstimate) -> u128 {
    let (num, den) = tier_multiplier(fee.tier);
    // `den < num` for every tier, so the exact quotient is ≤ `total_wei` and
    // the narrow is provably lossless. Computed wide anyway so that a future
    // tier with `den > num` cannot reintroduce the clamp-then-divide shape:
    // a funding pre-check that reads LOW is the one that lets an underfunded
    // op through.
    narrow_saturating(mul_wide(fee.total_wei, den) / w(num))
}

/// `calcMaxFeePerGas` (`safe-transaction.ts:2005-2011`):
/// gasPrice × speedTier × BUNDLER_MARGIN, floored at 1 wei.
pub fn calc_max_fee_per_gas(gas_price: u128, tier: FeeTier) -> u128 {
    let (num, den) = tier_multiplier(tier);
    // ×4.0 at most, evaluated exactly; the clamp can only fire for a chain
    // gas price above ~8.5e37 wei (there is not that much ether in existence),
    // and it clamps the per-gas price UP-scale, never into a plausible number.
    let max_fee = narrow_saturating(
        mul_wide(gas_price, num)
            .checked_mul(w(BUNDLER_MARGIN_NUM))
            .unwrap_or(U256::MAX)
            / w(den * BUNDLER_MARGIN_DEN),
    );
    max_fee.max(1)
}

/// The raw chain signals (`deriveChainGasPrice` input).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GasSignals {
    /// The chain being priced — carried so [`derive_chain_gas_price`] can apply
    /// that chain's [`min_gas_price_wei`] floor itself. A caller cannot forget
    /// a floor it does not have to remember.
    pub chain_id: u32,
    pub eth_gas_price: u128,
    pub base_fee: u128,
    pub priority_fee: u128,
    /// `None` = derive from whether a positive tip was supplied.
    pub tip_measured: Option<bool>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChainGasPrice {
    pub gas_price: u128,
    pub base_fee: u128,
    pub priority_fee: u128,
    pub tip_measured: bool,
}

/// `deriveChainGasPrice` (`safe-transaction.ts:1918-1932`): networkPrice =
/// max(eth_gasPrice, baseFee + tip). The tip is LOAD-BEARING on Gnosis
/// (baseFee≈0, tip is essentially the whole price — the old formula
/// under-priced ~40× and the bundler rejected the op).
pub fn derive_chain_gas_price(signals: &GasSignals) -> ChainGasPrice {
    let priority_fee = if signals.priority_fee > 0 {
        signals.priority_fee
    } else {
        signals.eth_gas_price.saturating_sub(signals.base_fee)
    };
    let with_tip = add(signals.base_fee, priority_fee);
    // The chain's own floor is the last word: a price under it is not a cheap
    // transaction, it is a transaction the chain throws away without saying so
    // (see `min_gas_price_wei`). The floor only ever RAISES a price, and it is
    // 0 on every chain but Arc, so nothing else moves.
    let floor = min_gas_price_wei(signals.chain_id);
    let gas_price = signals.eth_gas_price.max(with_tip).max(floor);
    let tip_measured = signals.tip_measured.unwrap_or(signals.priority_fee > 0);
    ChainGasPrice {
        gas_price,
        base_fee: signals.base_fee.max(floor),
        priority_fee,
        tip_measured,
    }
}

// -- base-unit string math (`eip681.ts:48-64`, shared by the reserve math) --

/// Human decimal string → base units. Port of `toBaseUnits` with `u128`
/// strictness: negative or non-numeric input answers `None` where `BigInt`
/// would go negative or throw (deliberate — garbage must never mint units).
/// The TS quirk of ignoring everything after a second dot is kept.
pub fn to_base_units(amount: &str, decimals: u32) -> Option<u128> {
    let cleaned = amount.trim();
    if cleaned.is_empty() {
        return Some(0);
    }
    // `split('.')` destructuring takes the first two parts; the rest vanish.
    let mut parts = cleaned.split('.');
    let int_part = parts.next().unwrap_or("");
    let frac_part = parts.next().unwrap_or("");
    if !int_part.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let mut frac: String = frac_part.chars().take(decimals as usize).collect();
    if !frac.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    while frac.len() < decimals as usize {
        frac.push('0');
    }
    let joined = format!(
        "{}{}",
        if int_part.is_empty() { "0" } else { int_part },
        frac
    );
    let stripped = joined.trim_start_matches('0');
    if stripped.is_empty() {
        Some(0)
    } else {
        stripped.parse::<u128>().ok()
    }
}

/// Integer base units → trimmed human decimal string (`fromBaseUnits`).
pub fn from_base_units(value: u128, decimals: u32) -> String {
    let s = value.to_string();
    if decimals == 0 {
        return s;
    }
    let width = decimals as usize + 1;
    let padded = if s.len() < width {
        format!("{}{}", "0".repeat(width - s.len()), s)
    } else {
        s
    };
    let split = padded.len() - decimals as usize;
    let int_part = &padded[..split];
    let frac = padded[split..].trim_end_matches('0');
    if frac.is_empty() {
        int_part.to_owned()
    } else {
        format!("{int_part}.{frac}")
    }
}

/// One token line of a multiSelect batch (`batch-send.ts` `MultiTokenSpec`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MultiTokenSpec {
    /// `None` = the chain's native coin.
    pub token_address: Option<String>,
    pub decimals: u32,
    /// Human decimal string (usually the full balance).
    pub amount: String,
}

/// `reserveNativeGas` (`batch-send.ts:150-167`): trim the native line by the
/// EntryPoint prefund reserve; drop it when nothing remains. A malformed
/// amount is treated as 0 (dropped) — TS throws; either way the reserve is
/// never swept.
pub fn reserve_native_gas(tokens: &[MultiTokenSpec], reserve_wei: u128) -> Vec<MultiTokenSpec> {
    if reserve_wei == 0 {
        return tokens.to_vec();
    }
    tokens
        .iter()
        .filter_map(|tk| {
            if tk.token_address.is_some() {
                return Some(tk.clone()); // ERC-20 — gas isn't paid in it
            }
            let held = to_base_units(&tk.amount, tk.decimals).unwrap_or(0);
            let left = held.checked_sub(reserve_wei)?;
            if left == 0 {
                return None; // native balance can't even cover the gas reserve
            }
            Some(MultiTokenSpec {
                amount: from_base_units(left, tk.decimals),
                ..tk.clone()
            })
        })
        .collect()
}

/// `reserveFeeToken` (`batch-send.ts:183-201`): the ERC-20 analogue — only
/// the selected fee-asset line is trimmed, case-insensitively.
pub fn reserve_fee_token(
    tokens: &[MultiTokenSpec],
    fee_token_address: &str,
    reserve_units: u128,
) -> Vec<MultiTokenSpec> {
    if reserve_units == 0 {
        return tokens.to_vec();
    }
    tokens
        .iter()
        .filter_map(|tk| {
            let is_fee = tk
                .token_address
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(fee_token_address));
            if !is_fee {
                return Some(tk.clone());
            }
            let held = to_base_units(&tk.amount, tk.decimals).unwrap_or(0);
            let left = held.checked_sub(reserve_units)?;
            if left == 0 {
                return None; // whole fee-token balance is needed for gas
            }
            Some(MultiTokenSpec {
                amount: from_base_units(left, tk.decimals),
                ..tk.clone()
            })
        })
        .collect()
}

/// `maxNativeSendable` (`batch-send.ts:169-181`): balance − reserve,
/// string-exact, so `to_base_units(result) + reserve == balance` and the
/// send screen's own "insufficient for gas" pre-check never trips on its own
/// Max fill (invariant ⑨ of the send machine; the vector tests pin it here).
pub fn max_native_sendable(balance_wei: u128, reserve_wei: u128, decimals: u32) -> String {
    if balance_wei <= reserve_wei {
        return "0".to_owned();
    }
    from_base_units(balance_wei - reserve_wei, decimals)
}

// -- Tempo gas model (`src/services/tempo.ts`) ------------------------------

/// `isTempoChain` minus the shell-side `chainMeta` escape hatch (see module
/// doc — ported verbatim otherwise).
pub fn is_tempo_chain(chain_id: u32) -> bool {
    TEMPO_CHAIN_IDS.contains(&chain_id)
}

/// The smallest representable $0.01 fee (`tempo.ts:53-61`): rounds UP to a
/// transferable unit for low-decimal tokens rather than undercharge.
pub fn tempo_minimum_fee_token_units(decimals: u32) -> u128 {
    let Some(unit) = pow10_wide(decimals) else {
        return u128::MAX;
    };
    let Some(scaled) = unit.checked_mul(w(TEMPO_MIN_FEE_USD_CENTS)) else {
        return u128::MAX;
    };
    ceil_div_wide(scaled, w(100)).map_or(u128::MAX, narrow_saturating)
}

/// Attodollars (USD×1e-18) → fee-token base units (`tempo.ts:117-126`).
///
/// This is the second `x × 10^decimals / 10^18` in the file, i.e. the same
/// clamp-then-divide shape as the in-band conversion; a clamped numerator here
/// would deflate by up to 1e18. Exact in `U256`; an unrepresentable product
/// answers `u128::MAX` (unpayable, hence refused), never a shrunken number.
pub fn atto_to_token_units(atto: u128, decimals: u32) -> u128 {
    atto_units_wide(w(atto), decimals).map_or(u128::MAX, narrow_saturating)
}

/// Raw gas fee incl. outer-tx overhead, no margin (`tempo.ts:128-141`).
pub fn tempo_fee_token_units(total_gas: u128, gas_price_atto: u128, decimals: u32) -> u128 {
    let price = if gas_price_atto > 0 {
        gas_price_atto
    } else {
        TEMPO_BASE_FEE_ATTO
    };
    atto_units_wide(
        mul_wide(add(total_gas, TEMPO_OUTER_OVERHEAD_GAS), price),
        decimals,
    )
    .map_or(u128::MAX, narrow_saturating)
}

/// callGasLimit floor for a batch of `sub_calls` (`tempo.ts:82-84`).
pub fn tempo_call_gas_limit(sub_calls: u32) -> u128 {
    // `u32 × 380_000 < 2^69`: cannot overflow `u128`.
    u128::from(sub_calls.max(1)) * TEMPO_CALL_GAS_PER_SUBCALL
}

/// Realistic total gas for a batch (`tempo.ts:113-116`) — used to PRICE the
/// reimbursement, never to set the padded UserOp limits.
pub fn tempo_expected_gas(deployed: bool, sub_calls: u32) -> u128 {
    let fixed = if deployed {
        TEMPO_DEPLOYED_GAS_EST
    } else {
        TEMPO_DEPLOY_GAS_EST
    };
    // `u32 × 110_000 + 4.15e6 < 2^68`: cannot overflow `u128`.
    fixed + u128::from(sub_calls.max(1)) * TEMPO_PER_SUBCALL_GAS_EST
}

/// Stablecoin reimbursement = realistic cost × 2, floored at $0.01
/// (`tempo.ts:143-157`).
pub fn tempo_reimbursement(expected_gas: u128, gas_price_atto: u128, decimals: u32) -> u128 {
    let price = if gas_price_atto > 0 {
        gas_price_atto
    } else {
        TEMPO_BASE_FEE_ATTO
    };
    // One exact expression: gas × price → token units → ×2. The margin is
    // applied to the WIDE base, so the doubling can never be swallowed by a
    // clamp that already happened.
    let with_margin = atto_units_wide(mul_wide(expected_gas, price), decimals)
        .and_then(|base| base.checked_mul(w(TEMPO_FEE_MARGIN_NUM)))
        .map_or(u128::MAX, |v| {
            narrow_saturating(v / w(TEMPO_FEE_MARGIN_DEN))
        });
    with_margin.max(tempo_minimum_fee_token_units(decimals))
}

/// EOA-floor cushion: max(flat 20k, 3% of the priced gas) (`tempo.ts:177-180`).
pub fn tempo_split_safety_gas(expected_gas: u128) -> u128 {
    // 3% of a `u128` always fits a `u128`; wide only so the product is exact.
    let proportional =
        narrow_saturating(mul_wide(expected_gas, TEMPO_SPLIT_SAFETY_BPS) / w(10_000));
    proportional.max(TEMPO_SPLIT_SAFETY_GAS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TempoSplit {
    pub eoa: u128,
    pub treasury: u128,
}

/// Split the reimbursement between bundler EOA and treasury
/// (`tempo.ts:182-203`): the EOA is floored at the bundler's cost so the
/// transfer always clears its accept check; too thin a margin keeps
/// everything on the EOA (treasury 0) so the tx is never rejected.
pub fn tempo_settlement_split(
    reimbursement: u128,
    expected_gas: u128,
    gas_price_atto: u128,
    decimals: u32,
) -> TempoSplit {
    let price = if gas_price_atto > 0 {
        gas_price_atto
    } else {
        TEMPO_BASE_FEE_ATTO
    };
    let eoa_floor = atto_units_wide(
        mul_wide(
            add(
                add(expected_gas, TEMPO_COST_BUFFER_GAS),
                tempo_split_safety_gas(expected_gas),
            ),
            price,
        ),
        decimals,
    )
    // A floor that cannot be represented keeps EVERYTHING on the bundler EOA
    // (treasury 0) — the branch below that never lets the accept check fail.
    .map_or(u128::MAX, narrow_saturating);
    if reimbursement <= eoa_floor {
        return TempoSplit {
            eoa: reimbursement,
            treasury: 0,
        };
    }
    TempoSplit {
        eoa: eoa_floor,
        treasury: reimbursement - eoa_floor,
    }
}

/// The Tempo submit-side "sign what was displayed" guard
/// (`safe-transaction.ts:1085-1095`, invariant ③): a quote whose recipient
/// changed, or whose amount predates the $0.01 floor, is stale and must be
/// re-reviewed — never silently re-priced.
pub fn tempo_quote_is_stale(
    quoted_amount: u128,
    quoted_recipient: &str,
    fee_collector: &str,
    decimals: u32,
) -> bool {
    !quoted_recipient.eq_ignore_ascii_case(fee_collector)
        || quoted_amount < tempo_reimbursement(0, 0, decimals)
}

// -- calldata shape helpers -------------------------------------------------

/// `transfer(address,uint256)` calldata (`batch-send.ts:59-64`). `None` for
/// an invalid recipient where TS throws `BatchSendError`.
pub fn encode_erc20_transfer(to: &str, amount: u128) -> Option<String> {
    if !is_hex_address(to) {
        return None;
    }
    Some(format!(
        "0xa9059cbb{:0>64}{:0>64}",
        to[2..].to_lowercase(),
        format!("{amount:x}")
    ))
}

fn call_data_bytes(data_hex: &str) -> usize {
    data_hex.trim_start_matches("0x").len() / 2
}

fn pad32_len(len: usize) -> usize {
    len + ((32 - (len % 32)) % 32)
}

/// Byte length of `buildMultiSendExecuteCallData(calls)`
/// (`safe-transaction.ts:1407-1440`) computed without building it — the only
/// fact the core needs from the encoding is whether a failed simulation may
/// fall back to the static model (`> ESTIMATION_REQUIRED_CALLDATA` must not).
pub(crate) fn multisend_execute_calldata_len(calls: &[FeeCall]) -> usize {
    // Packed sub-call: 1 (op) + 20 (to) + 32 (value) + 32 (len) + data.
    let packed: usize = calls
        .iter()
        .map(|call| 85 + call_data_bytes(&call.data))
        .sum();
    // multiSend payload: selector + offset + length + packed + pad.
    let payload = 68 + pad32_len(packed);
    // executeUserOp wrapper: selector + 5 words + payload + pad.
    164 + pad32_len(payload)
}

/// The single fee leg batched into an in-band UserOp
/// (`safe-transaction.ts:1151-1161`): a plain native transfer to the
/// bundler's recipient, or a stablecoin `transfer`. The submit path replaces
/// the placeholder amount with the signed quote; the estimate uses 1 unit.
fn in_band_fee_leg(gas_fee_token: Option<&str>, recipient: &str, amount: u128) -> Option<FeeCall> {
    match gas_fee_token {
        Some(token) => Some(FeeCall {
            to: token.to_owned(),
            value: "0".to_owned(),
            data: encode_erc20_transfer(recipient, amount)?,
        }),
        None => Some(FeeCall {
            to: recipient.to_owned(),
            value: amount.to_string(),
            data: "0x".to_owned(),
        }),
    }
}

fn dummy_estimation_call() -> FeeCall {
    FeeCall {
        to: ESTIMATION_DUMMY_TARGET.to_owned(),
        value: "0".to_owned(),
        data: format!("0x{}", "00".repeat(ESTIMATION_DUMMY_DATA_LENGTH)),
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The estimate every consumer prices against — the `TransactionFeeEstimate`
/// port (`safe-transaction.ts:264-296`), `u128` internally, strings on wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeeEstimate {
    /// Chain this quote was calculated for. Never reuse across networks.
    pub chain_id: u32,
    pub total_wei: u128,
    pub max_fee_per_gas: u128,
    pub network_fee_per_gas: u128,
    pub relayer_fee_per_gas: u128,
    pub bundler_gas_price: u128,
    /// `max(chain gas price, bundler network fee)` — the gas basis the in-band
    /// reimbursement was priced against (and re-priced against when the fee
    /// asset is switched). See `MAX_QUOTE_VS_CHAIN_MULTIPLE`.
    pub in_band_gas_basis: u128,
    pub total_gas: u128,
    pub deployed: bool,
    pub tier: FeeTier,
    pub quoted: bool,
    pub fee_asset: FeeAsset,
    /// The quote's transfer recipient — the submit path signs THIS verbatim.
    pub fee_recipient: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FeeAsset {
    Native,
    Erc20 {
        token: String,
        decimals: u32,
        amount: u128,
        symbol: Option<String>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ParsedQuote {
    recipient: String,
    is_native: bool,
    fee_token: Option<String>,
    balance: u128,
    decimals: u32,
    symbol: String,
    usd_balance: String,
    usd_price: Option<String>,
}

impl ParsedQuote {
    fn pricing(&self) -> AssetPricing {
        AssetPricing {
            is_native: self.is_native,
            decimals: self.decimals,
            usd_price: self.usd_price.clone(),
        }
    }
}

/// `findInBandGasQuote` (`bundler-service.ts:652-661`).
fn find_quote<'a>(quotes: &'a [ParsedQuote], fee_token: Option<&str>) -> Option<&'a ParsedQuote> {
    quotes.iter().find(|quote| match fee_token {
        Some(wanted) => {
            !quote.is_native
                && quote
                    .fee_token
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(wanted))
        }
        None => quote.is_native,
    })
}

/// The request being priced, kept so `Requote`/`SelectFeeAsset` re-run the
/// SAME transaction shape (invariant ⑨ holds across requotes too).
#[derive(Clone, Debug)]
struct RequestCtx {
    chain_id: u32,
    account: String,
    deployed: bool,
    public_key_available: bool,
    tier: FeeTier,
    calls: Vec<FeeCall>,
}

/// Who started the run — decides what a failure does. The initial estimate
/// surfaces its failure; a refresh swallows it (the old quote keeps showing,
/// `GasFeeCard.handleRefresh`'s `catch {}`); a chip-switch reverts the
/// selection (`handleFeeTokenSelect`'s `catch → onFeeTokenChange(prev)`).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
enum Origin {
    #[default]
    Initial,
    Refresh,
    Select {
        previous: Option<String>,
    },
}

#[derive(Clone, Debug, Default)]
struct Pending {
    gas: Option<ChainGasPrice>,
    bundler: Option<Option<FeeBundlerQuote>>,
    quotes: Option<Option<Vec<ParsedQuote>>>,
    recipient: Option<Option<String>>,
}

/// What the pricing step needs once the simulation answers.
#[derive(Clone, Debug, PartialEq, Eq)]
enum PricePlan {
    Generic {
        network_fee_per_gas: u128,
        relayer_fee_per_gas: u128,
        bundler_gas_price: u128,
        /// `max(chain gas price, bundler network fee)` — the basis the in-band
        /// reimbursement is priced against, so the 3× buffer rides on the
        /// larger of our own measurement and the bundler's quote, never on an
        /// unvetted quote alone.
        in_band_gas_basis: u128,
        quoted: bool,
        est_calldata_len: usize,
    },
    Tempo {
        gas_price_atto: u128,
        static_gas: u128,
        fee_token: String,
        recipient: Option<String>,
    },
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    /// Waiting for the parallel context reads.
    Gathering,
    /// Waiting for the UserOp gas simulation.
    Estimating(PricePlan),
    Quoted,
    Failed(FeeFailure),
}

#[derive(Default)]
pub struct Model {
    /// The chain the form currently targets — the estimate is exposed only
    /// while its own `chain_id` matches (invariant ①,
    /// `useSendController.ts:119-121`).
    form_chain_id: Option<u32>,
    ctx: Option<RequestCtx>,
    phase: Phase,
    origin: Origin,
    pending: Pending,
    /// The full in-band quote rows (unfiltered — the estimate path prices
    /// against all rows; the picker filter is applied in the view).
    quotes: Vec<ParsedQuote>,
    /// Selected fee asset: `None` = native, else a stablecoin contract.
    fee_token: Option<String>,
    estimate: Option<FeeEstimate>,
    stale: bool,
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// Wire projection of [`FeeEstimate`] — amounts as decimal strings (never
/// JSON numbers; inventory open question 4).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeEstimateView {
    pub chain_id: u32,
    pub total_wei: String,
    pub max_fee_per_gas: String,
    pub network_fee_per_gas: String,
    pub relayer_fee_per_gas: String,
    pub bundler_gas_price: String,
    pub in_band_gas_basis: String,
    pub total_gas: String,
    pub deployed: bool,
    pub tier: FeeTier,
    pub quoted: bool,
    pub fee_asset: FeeAssetView,
    pub fee_recipient: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeeAssetView {
    Native,
    Erc20 {
        token: String,
        decimals: u32,
        amount: String,
        symbol: Option<String>,
    },
}

/// One selector row (`FeeTokenSelector`). `insufficient` is the core-owned
/// balance<fee gate (invariant ⑧, `FeeTokenSelector.tsx:74`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeOptionView {
    pub symbol: String,
    /// `None` = the native coin.
    pub contract: Option<String>,
    pub decimals: u32,
    pub balance: String,
    pub recipient: String,
    pub usd_balance: String,
    pub usd_price: Option<String>,
    /// Cost of THIS tx in this coin, or `None` when it cannot be priced.
    pub amount: Option<String>,
    pub insufficient: bool,
    pub selected: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeView {
    /// Estimating or requoting — the confirm slide must stay disabled
    /// (invariant ⑦, `SigningSheet.tsx:576-583`).
    pub busy: bool,
    pub failed: Option<FeeFailure>,
    /// Present only when valid for the current form chain (invariant ①).
    pub fee: Option<FeeEstimateView>,
    /// The 30s TTL elapsed — advisory; the shell shows a refresh affordance.
    pub stale: bool,
    pub fee_token: Option<String>,
    pub options: Vec<FeeOptionView>,
    /// The single gate consumers AND into their confirm button.
    pub confirm_fee_ready: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FeePolicy;

impl App for FeePolicy {
    type Event = Event;
    type Model = Model;
    type ViewModel = FeeView;
    type Effect = FeeEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<FeeEffect, Event> {
        match event {
            Event::QuoteRequested {
                chain_id,
                account,
                deployed,
                public_key_available,
                tier,
                calls,
                fee_token,
            } => {
                model.attempt += 1;
                model.form_chain_id = Some(chain_id);
                model.ctx = Some(RequestCtx {
                    chain_id,
                    account,
                    deployed,
                    public_key_available,
                    tier,
                    calls,
                });
                // The requested denomination becomes THE fee token for this
                // run, before anything is simulated, so the op that is priced
                // carries the same fee leg as the op that is submitted. A
                // caller that wants the SigningSheet's native reset
                // (`SigningSheet.tsx:247-249`) asks for `None`.
                // No leftover estimate survives (`useSendController.ts:723`:
                // setFeeEstimate(null)).
                model.fee_token = fee_token;
                model.estimate = None;
                model.stale = false;
                model.quotes.clear();
                model.origin = Origin::Initial;
                begin_pipeline(model)
            }
            Event::Requote => {
                // `GasFeeCard.handleRefresh`: ignored while one is running.
                if model.ctx.is_none() || is_busy(&model.phase) {
                    return Command::done();
                }
                model.attempt += 1;
                model.origin = Origin::Refresh;
                begin_pipeline(model)
            }
            Event::SelectFeeAsset { token } => select_fee_asset(model, token),
            Event::LeaveConfirm => {
                // Invariant ⑥ (`useSendController.ts:467-473`): reset the
                // fee-asset choice and clear a stale erc20 estimate
                // (totalWei=0) so the gas-reserve/warning math downstream
                // never reads 0. A native estimate survives, as today.
                model.attempt += 1; // in-flight card work is abandoned
                model.pending = Pending::default();
                model.fee_token = None;
                if matches!(
                    model.estimate.as_ref().map(|e| &e.fee_asset),
                    Some(FeeAsset::Erc20 { .. })
                ) {
                    model.estimate = None;
                    model.stale = false;
                }
                model.phase = if model.estimate.is_some() {
                    Phase::Quoted
                } else {
                    Phase::Idle
                };
                render()
            }
            Event::ChainChanged { chain_id } => {
                // Invariant ①: the quote rows and any in-flight work belong
                // to the old chain. The estimate is kept but hidden by the
                // view's chain guard, mirroring `selectedFeeEstimate`.
                model.attempt += 1;
                model.form_chain_id = Some(chain_id);
                model.pending = Pending::default();
                model.quotes.clear();
                model.fee_token = None;
                model.phase = if model.estimate.is_some() {
                    Phase::Quoted
                } else {
                    Phase::Idle
                };
                render()
            }
            Event::QuoteExpired => {
                if model.phase != Phase::Quoted {
                    return Command::done();
                }
                model.stale = true;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded run — most importantly, an old-chain quote
                    // arriving after the form moved on. Dropping it IS
                    // invariant ①'s second half.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> FeeView {
        let busy = is_busy(&model.phase);
        let failed = match &model.phase {
            Phase::Failed(kind) => Some(*kind),
            _ => None,
        };
        // A quote is valid only for the network it was calculated on
        // (`useSendController.ts:119-121`; `TransactionFeeEstimate.chainId`).
        let fee = model
            .estimate
            .as_ref()
            .filter(|estimate| Some(estimate.chain_id) == model.form_chain_id)
            .map(estimate_view);
        let confirm_fee_ready = !busy && failed.is_none() && fee.is_some();
        FeeView {
            busy,
            failed,
            fee,
            stale: model.stale,
            fee_token: model.fee_token.clone(),
            options: option_views(model),
            confirm_fee_ready,
        }
    }
}

fn is_busy(phase: &Phase) -> bool {
    matches!(phase, Phase::Gathering | Phase::Estimating(_))
}

// ---------------------------------------------------------------------------
// Pipeline
// ---------------------------------------------------------------------------

fn begin_pipeline(model: &mut Model) -> Command<FeeEffect, Event> {
    let Some(ctx) = model.ctx.clone() else {
        return Command::done();
    };
    let tempo = is_tempo_chain(ctx.chain_id);
    // Invariant ⑤ (`safe-transaction.ts:634-642`): an undeployed account
    // without its public key cannot build the real initCode — a draft that
    // cannot match the final operation must never be estimated.
    //
    // Tempo is exempt because it never estimates one: `advance_tempo` only
    // simulates for a contract call on a DEPLOYED Safe, so an undeployed Tempo
    // quote is pure static model and needs no initCode at all — which is why
    // `estimateTempoFee` takes no `publicKeyHex` and quotes it happily
    // (`safe-transaction.ts:464-546`). Refusing here would have blocked a new
    // user's first Tempo send on web only.
    if !tempo && !ctx.deployed && !ctx.public_key_available {
        return fail(model, FeeFailure::MissingPublicKey);
    }
    model.pending = Pending::default();
    model.phase = Phase::Gathering;
    let mut operations = vec![FeeOperation::FetchGasPrice {
        chain_id: ctx.chain_id,
        // Tempo is excluded from the tip query — attodollar gas makes
        // eth_maxPriorityFeePerGas meaningless (`safe-transaction.ts:1943`).
        want_tip: !tempo,
    }];
    if tempo {
        operations.push(FeeOperation::FetchFeeRecipient {
            chain_id: ctx.chain_id,
            account: ctx.account.clone(),
        });
    } else {
        operations.push(FeeOperation::FetchBundlerQuote {
            chain_id: ctx.chain_id,
            tier: ctx.tier,
        });
    }
    operations.push(FeeOperation::FetchInBandQuotes {
        chain_id: ctx.chain_id,
        account: ctx.account.clone(),
    });
    requests(model, operations)
}

fn accept(model: &mut Model, result: FeeShellResult) -> Command<FeeEffect, Event> {
    match (&model.phase, result) {
        (
            Phase::Gathering,
            FeeShellResult::GasPrice {
                eth_gas_price,
                base_fee,
                priority_fee,
            },
        ) => {
            let chain_id = model.ctx.as_ref().map(|c| c.chain_id).unwrap_or(0);
            let tempo = is_tempo_chain(chain_id);
            model.pending.gas = Some(resolve_gas_price(
                chain_id,
                eth_gas_price.as_deref(),
                base_fee.as_deref(),
                priority_fee.as_deref(),
                !tempo,
            ));
            try_advance(model)
        }
        (Phase::Gathering, FeeShellResult::BundlerQuote { quote }) => {
            model.pending.bundler = Some(quote);
            try_advance(model)
        }
        (Phase::Gathering, FeeShellResult::InBandQuotes { quotes }) => {
            model.pending.quotes =
                Some(quotes.map(|rows| rows.iter().map(parse_quote_row).collect::<Vec<_>>()));
            try_advance(model)
        }
        (Phase::Gathering, FeeShellResult::FeeRecipient { recipient }) => {
            model.pending.recipient = Some(recipient);
            try_advance(model)
        }
        (Phase::Estimating(plan), FeeShellResult::UserOpGas { outcome }) => {
            let plan = plan.clone();
            accept_gas_outcome(model, &plan, outcome)
        }
        (Phase::Quoted, FeeShellResult::TtlElapsed) => {
            model.stale = true;
            render()
        }
        // A result for a phase that no longer expects it. Never an error,
        // never a state change.
        _ => Command::done(),
    }
}

/// `getGasPrices` degradation (`safe-transaction.ts:1949-1985`): only a
/// failed/zero `eth_gasPrice` falls to the 5-gwei default; a failed tip read
/// just leaves `tip_measured` false.
fn resolve_gas_price(
    chain_id: u32,
    eth_gas_price: Option<&str>,
    base_fee: Option<&str>,
    priority_fee: Option<&str>,
    want_tip: bool,
) -> ChainGasPrice {
    // The static fallback is exactly where Arc's silent-drop hazard lives: 5
    // gwei doubled by the bundler margin is 10 gwei, under Arc's floor. So the
    // fallback is floored too — a chain that cannot be read is still a chain
    // whose rules apply.
    let floor = min_gas_price_wei(chain_id);
    let fallback = ChainGasPrice {
        gas_price: FALLBACK_GAS_PRICE_WEI.max(floor),
        base_fee: FALLBACK_GAS_PRICE_WEI.max(floor),
        priority_fee: 0,
        tip_measured: false,
    };
    let Some(eth) = eth_gas_price.and_then(parse_units) else {
        return fallback;
    };
    let base = base_fee.and_then(parse_units).unwrap_or(0);
    // A present result (even "0" on L2s) is a real measurement.
    let tip_measured = want_tip && priority_fee.is_some();
    let tip = priority_fee.and_then(parse_units).unwrap_or(0);
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id,
        eth_gas_price: eth,
        base_fee: base,
        priority_fee: tip,
        tip_measured: Some(tip_measured),
    });
    if derived.gas_price > 0 {
        derived
    } else {
        fallback
    }
}

fn parse_quote_row(row: &FeeAssetQuote) -> ParsedQuote {
    ParsedQuote {
        recipient: row.recipient.clone(),
        is_native: row.asset == FeeAssetKind::Native,
        fee_token: row.fee_token.clone(),
        balance: parse_units(&row.balance).unwrap_or(0),
        decimals: row.decimals,
        symbol: row.symbol.clone(),
        usd_balance: row.usd_balance.clone(),
        usd_price: row.usd_price.clone(),
    }
}

fn try_advance(model: &mut Model) -> Command<FeeEffect, Event> {
    let Some(ctx) = model.ctx.clone() else {
        return Command::done();
    };
    if is_tempo_chain(ctx.chain_id) {
        let (Some(gas), Some(recipient), Some(quotes)) = (
            model.pending.gas,
            model.pending.recipient.clone(),
            model.pending.quotes.clone(),
        ) else {
            return Command::done();
        };
        // Tempo pricing needs no in-band quote rows; they only feed the
        // fee-asset picker, and their absence means "no selector"
        // (`use-inband-fee-tokens.ts`: null → native only).
        model.quotes = quotes.unwrap_or_default();
        advance_tempo(model, &ctx, gas, recipient)
    } else {
        let (Some(gas), Some(bundler), Some(quotes)) = (
            model.pending.gas,
            model.pending.bundler.clone(),
            model.pending.quotes.clone(),
        ) else {
            return Command::done();
        };
        advance_generic(model, &ctx, gas, bundler, quotes)
    }
}

/// The bundler quote acceptance rules (`safe-transaction.ts:2030-2094`).
/// A zero `maxFeePerGas` is degenerate, not authoritative — pricing an op at
/// 0 is self-refuting, so it is "cannot quote" → local fallback (invariant ④,
/// `safe-transaction.ts:2070-2076`).
struct AcceptedQuote {
    network_fee_per_gas: u128,
    relayer_fee_per_gas: u128,
}

fn accept_bundler_quote(
    raw: Option<&FeeBundlerQuote>,
    chain_gas_price: u128,
) -> Option<AcceptedQuote> {
    let raw = raw?;
    let max_fee = parse_units(&raw.max_fee_per_gas)?;
    if max_fee == 0 {
        return None;
    }
    let reported_network = raw
        .network_fee_per_gas
        .as_deref()
        .and_then(parse_units)
        .unwrap_or(0);
    let network_fee_per_gas = if reported_network > 0 {
        reported_network
    } else {
        chain_gas_price
    };
    let relayer_fee_per_gas = match raw.relayer_fee_per_gas.as_deref().and_then(parse_units) {
        Some(value) => value,
        None => max_fee.saturating_sub(network_fee_per_gas),
    };
    Some(AcceptedQuote {
        network_fee_per_gas,
        relayer_fee_per_gas,
    })
}

fn advance_generic(
    model: &mut Model,
    ctx: &RequestCtx,
    gas: ChainGasPrice,
    bundler: Option<FeeBundlerQuote>,
    quotes: Option<Vec<ParsedQuote>>,
) -> Command<FeeEffect, Event> {
    let accepted = accept_bundler_quote(bundler.as_ref(), gas.gas_price);
    let quoted = accepted.is_some();
    let (network_fee_per_gas, relayer_fee_per_gas, bundler_gas_price) = match accepted {
        Some(quote) => (
            quote.network_fee_per_gas,
            quote.relayer_fee_per_gas,
            quote.network_fee_per_gas,
        ),
        None => {
            // Local fallback (`safe-transaction.ts:600-608`).
            let local_max = calc_max_fee_per_gas(gas.gas_price, ctx.tier);
            let (num, den) = tier_multiplier(ctx.tier);
            let bgp = narrow_saturating(mul_wide(gas.gas_price, num) / w(den)).max(1);
            (bgp, local_max.saturating_sub(bgp), bgp)
        }
    };

    // Protect the user (requirement G05): a bundler quote more than
    // `MAX_QUOTE_VS_CHAIN_MULTIPLE` times our own on-chain gas measurement is
    // refused, not signed. Only a real bundler quote is vetted — the local
    // fallback derives from the same measurement and cannot be "too high".
    let chain_gas_price = gas.gas_price;
    if quoted
        && chain_gas_price > 0
        && network_fee_per_gas > chain_gas_price.saturating_mul(MAX_QUOTE_VS_CHAIN_MULTIPLE)
    {
        return fail(model, FeeFailure::GasQuoteTooHigh);
    }
    // Anchor the in-band reimbursement on the LARGER of our own measurement and
    // the bundler's quote: never underpay when the bundler under-reports the
    // network fee, and never let the 3× buffer ride on an unvetted quote alone.
    let in_band_gas_basis = network_fee_per_gas.max(chain_gas_price);

    // The in-band quote is mandatory on every supported network
    // (`safe-transaction.ts:645-661`).
    let Some(rows) = quotes else {
        return fail(model, missing_quote_failure(model.fee_token.is_some()));
    };
    let selected = find_quote(&rows, model.fee_token.as_deref()).cloned();
    let native = find_quote(&rows, None).cloned();
    let (Some(selected), Some(_native)) = (selected, native) else {
        return fail(model, missing_quote_failure(model.fee_token.is_some()));
    };
    model.quotes = rows;

    // The estimate simulates the REAL batch: the user's calls plus the fee
    // leg to the quote's recipient — the same MultiSend the submit path
    // builds, so the charge basis prices the actual send (invariant ⑨).
    let mut est_calls: Vec<FeeCall> = if ctx.calls.is_empty() {
        vec![dummy_estimation_call()]
    } else {
        ctx.calls.clone()
    };
    let Some(leg) = in_band_fee_leg(model.fee_token.as_deref(), &selected.recipient, 1) else {
        return fail(model, FeeFailure::EstimateFailed);
    };
    est_calls.push(leg);
    let est_calldata_len = multisend_execute_calldata_len(&est_calls);

    model.phase = Phase::Estimating(PricePlan::Generic {
        network_fee_per_gas,
        relayer_fee_per_gas,
        bundler_gas_price,
        in_band_gas_basis,
        quoted,
        est_calldata_len,
    });
    requests(
        model,
        vec![FeeOperation::EstimateUserOpGas {
            chain_id: ctx.chain_id,
            account: ctx.account.clone(),
            deployed: ctx.deployed,
            calls: est_calls,
        }],
    )
}

fn missing_quote_failure(fee_token_selected: bool) -> FeeFailure {
    if fee_token_selected {
        FeeFailure::FeeTokenUnavailable
    } else {
        FeeFailure::QuoteUnavailable
    }
}

fn advance_tempo(
    model: &mut Model,
    ctx: &RequestCtx,
    gas: ChainGasPrice,
    recipient: Option<String>,
) -> Command<FeeEffect, Event> {
    // `estimateTempoFee` (`safe-transaction.ts:451-546`).
    let fee_token = model
        .fee_token
        .clone()
        .unwrap_or_else(|| TEMPO_DEFAULT_FEE_TOKEN.to_owned());
    let has_contract_call = ctx
        .calls
        .iter()
        .any(|call| !call.data.is_empty() && call.data != "0x");
    let inner_call_count = (ctx.calls.len() as u32).max(1);
    let reimbursement_legs: u32 = if has_contract_call { 2 } else { 1 };
    let sub_call_count = inner_call_count + reimbursement_legs;
    let static_gas = tempo_expected_gas(ctx.deployed, sub_call_count);

    let plan = PricePlan::Tempo {
        gas_price_atto: gas.gas_price,
        static_gas,
        fee_token: fee_token.clone(),
        recipient,
    };

    if has_contract_call && ctx.deployed {
        // Refine off the bundler's estimate of the REAL call, with two
        // placeholder reimbursement transfers (the split case) so the quote
        // doesn't under-count (`safe-transaction.ts:483-514`).
        let Some(leg_data) = encode_erc20_transfer(&ctx.account, 1) else {
            return fail(model, FeeFailure::EstimateFailed);
        };
        let leg = FeeCall {
            to: fee_token,
            value: "0".to_owned(),
            data: leg_data,
        };
        let mut est_calls = ctx.calls.clone();
        est_calls.push(leg.clone());
        est_calls.push(leg);
        model.phase = Phase::Estimating(plan);
        return requests(
            model,
            vec![FeeOperation::EstimateUserOpGas {
                chain_id: ctx.chain_id,
                account: ctx.account.clone(),
                deployed: ctx.deployed,
                calls: est_calls,
            }],
        );
    }
    price_tempo(model, ctx, &plan, static_gas)
}

fn accept_gas_outcome(
    model: &mut Model,
    plan: &PricePlan,
    outcome: FeeGasOutcome,
) -> Command<FeeEffect, Event> {
    let Some(ctx) = model.ctx.clone() else {
        return Command::done();
    };
    match plan {
        PricePlan::Generic {
            est_calldata_len, ..
        } => match outcome {
            FeeGasOutcome::Estimated {
                verification_gas_limit,
                call_gas_limit,
                pre_verification_gas,
            } => {
                let (Some(vgl), Some(cgl), Some(pvg)) = (
                    parse_units(&verification_gas_limit),
                    parse_units(&call_gas_limit),
                    parse_units(&pre_verification_gas),
                ) else {
                    return fail(model, FeeFailure::EstimateFailed);
                };
                // Padding (`safe-transaction.ts:697-702`): ×1.5 with floors —
                // 2M for an undeployed account's deploy.
                // ×1.5 exactly: a clamped `vgl × 15` divided by 10 would
                // UNDER-count the gas the fee is priced from.
                let est_vgl = narrow_saturating(mul_wide(vgl, 15) / w(10)).max(if ctx.deployed {
                    VERIFICATION_GAS_DEPLOYED
                } else {
                    2_000_000
                });
                let est_cgl = narrow_saturating(mul_wide(cgl, 15) / w(10)).max(100_000);
                let est_pvg = add(pvg, 10_000);
                let total_gas = add(add(est_vgl, est_cgl), est_pvg);
                price_generic(model, &ctx, plan, total_gas)
            }
            FeeGasOutcome::ContextUnavailable => fail(model, FeeFailure::EstimateFailed),
            FeeGasOutcome::SimulationFailed => {
                // For a large/complex op the static fallback would show a
                // misleading number and the submit would refuse it anyway
                // (`safe-transaction.ts:703-713`).
                if *est_calldata_len > ESTIMATION_REQUIRED_CALLDATA {
                    return fail(model, FeeFailure::EstimateFailed);
                }
                let verification_gas = if ctx.deployed {
                    VERIFICATION_GAS_DEPLOYED
                } else {
                    VERIFICATION_GAS_UNDEPLOYED
                };
                let mut total_gas =
                    add(add(verification_gas, CALL_GAS_LIMIT), PRE_VERIFICATION_GAS);
                // L2 rollup data-fee adjustments (`safe-transaction.ts:723-731`).
                if ARBITRUM_CHAIN_IDS.contains(&ctx.chain_id) {
                    total_gas = add(total_gas, ARBITRUM_STATIC_GAS_ADDER);
                } else if OP_STACK_CHAIN_IDS.contains(&ctx.chain_id) {
                    total_gas = add(total_gas, OP_STACK_STATIC_GAS_ADDER);
                }
                price_generic(model, &ctx, plan, total_gas)
            }
        },
        PricePlan::Tempo { static_gas, .. } => match outcome {
            FeeGasOutcome::Estimated {
                verification_gas_limit,
                call_gas_limit,
                pre_verification_gas,
            } => {
                let (Some(vgl), Some(cgl), Some(pvg)) = (
                    parse_units(&verification_gas_limit),
                    parse_units(&call_gas_limit),
                    parse_units(&pre_verification_gas),
                ) else {
                    return fail(model, FeeFailure::EstimateFailed);
                };
                // The refine uses the UN-padded sum (`safe-transaction.ts:514`).
                let expected = (*static_gas).max(add(add(vgl, cgl), pvg));
                let plan = plan.clone();
                price_tempo(model, &ctx, &plan, expected)
            }
            // A contract call we can't estimate must surface — a
            // transfer-sized fee would mislead and then be rejected
            // (`safe-transaction.ts:515-520`).
            _ => fail(model, FeeFailure::EstimateFailed),
        },
    }
}

fn price_generic(
    model: &mut Model,
    ctx: &RequestCtx,
    plan: &PricePlan,
    total_gas: u128,
) -> Command<FeeEffect, Event> {
    let PricePlan::Generic {
        network_fee_per_gas,
        relayer_fee_per_gas,
        bundler_gas_price,
        in_band_gas_basis,
        quoted,
        ..
    } = plan
    else {
        return Command::done();
    };
    let (Some(selected), Some(native)) = (
        find_quote(&model.quotes, model.fee_token.as_deref()).cloned(),
        find_quote(&model.quotes, None).cloned(),
    ) else {
        return fail(model, FeeFailure::CalculationFailed);
    };
    let Some(fee_amount) = calculate_in_band_fee_amount(
        total_gas,
        *in_band_gas_basis,
        &selected.pricing(),
        &native.pricing(),
    ) else {
        return fail(model, FeeFailure::CalculationFailed);
    };
    // Invariant ⑧ (`FeeTokenSelector.tsx:74`), applied to a REQUESTED fee
    // token: an asset whose balance cannot cover this transaction's fee would
    // only produce a doomed op, so it is refused out loud with the sentence
    // that tells the user to pick another asset — never silently downgraded to
    // a native quote nobody asked for. It lives here (and not before the
    // simulation) because the amount is only knowable once the gas is.
    // The native row is NOT gated: quoting it is the caller's only remaining
    // option, and `estimateTransactionFee` does not gate it either.
    if model.fee_token.is_some() && fee_row_insufficient(selected.balance, Some(fee_amount)) {
        return fail(model, FeeFailure::FeeTokenUnavailable);
    }
    // Every signed UserOp pays maxFeePerGas = 0; the fee rides in the leg.
    // `feeRecipient` rides along so the submit path signs EXACTLY this quote
    // — displayed = signed, never a silent mismatch
    // (`safe-transaction.ts:741-765`).
    let fee_asset = match (&selected.is_native, &selected.fee_token) {
        (false, Some(token)) => FeeAsset::Erc20 {
            token: token.clone(),
            decimals: selected.decimals,
            amount: fee_amount,
            // The relay published this row's ticker and the quote is about
            // THAT coin, so it travels with the fee. Left out, every surface
            // fell back to the chain's native symbol: a USDC fee was drawn as
            // an amount of ETH, with ETH's mark beside it (issue #211's
            // follow-up report).
            symbol: Some(selected.symbol.clone()),
        },
        _ => FeeAsset::Native,
    };
    let total_wei = match fee_asset {
        FeeAsset::Native => fee_amount,
        FeeAsset::Erc20 { .. } => 0,
    };
    model.estimate = Some(FeeEstimate {
        chain_id: ctx.chain_id,
        total_wei,
        max_fee_per_gas: 0,
        network_fee_per_gas: *network_fee_per_gas,
        relayer_fee_per_gas: *relayer_fee_per_gas,
        bundler_gas_price: *bundler_gas_price,
        in_band_gas_basis: *in_band_gas_basis,
        total_gas,
        deployed: ctx.deployed,
        tier: ctx.tier,
        quoted: *quoted,
        fee_asset,
        fee_recipient: Some(selected.recipient),
    });
    settle_quoted(model)
}

fn price_tempo(
    model: &mut Model,
    ctx: &RequestCtx,
    plan: &PricePlan,
    expected_gas: u128,
) -> Command<FeeEffect, Event> {
    let PricePlan::Tempo {
        gas_price_atto,
        fee_token,
        recipient,
        ..
    } = plan
    else {
        return Command::done();
    };
    let reimbursement =
        tempo_reimbursement(expected_gas, *gas_price_atto, TEMPO_FEE_TOKEN_DECIMALS);
    // `totalWei` is the reimbursement scaled to attodollars so the USD
    // display path (totalWei / 1e18) renders it (`safe-transaction.ts:447-450`).
    let total_wei = narrow_saturating(mul_wide(
        reimbursement,
        pow10(18 - TEMPO_FEE_TOKEN_DECIMALS),
    ));
    let symbol = if fee_token.eq_ignore_ascii_case(TEMPO_DEFAULT_FEE_TOKEN) {
        Some("pathUSD".to_owned())
    } else {
        None
    };
    model.estimate = Some(FeeEstimate {
        chain_id: ctx.chain_id,
        total_wei,
        max_fee_per_gas: *gas_price_atto,
        network_fee_per_gas: *gas_price_atto,
        relayer_fee_per_gas: 0,
        bundler_gas_price: *gas_price_atto,
        // Tempo prices in pathUSD attodollars, not the generic gas basis; this
        // field is unused on the Tempo path (fee_amount_for_option reads
        // network_fee_per_gas there) and mirrors it for shape completeness.
        in_band_gas_basis: *gas_price_atto,
        total_gas: expected_gas,
        deployed: ctx.deployed,
        tier: ctx.tier,
        quoted: false,
        fee_asset: FeeAsset::Erc20 {
            token: fee_token.clone(),
            decimals: TEMPO_FEE_TOKEN_DECIMALS,
            amount: reimbursement,
            symbol,
        },
        // Only a well-formed address may become part of the signed fee
        // instruction (`safe-transaction.ts:545`).
        fee_recipient: recipient.clone().filter(|r| is_hex_address(r)),
    });
    settle_quoted(model)
}

fn settle_quoted(model: &mut Model) -> Command<FeeEffect, Event> {
    model.phase = Phase::Quoted;
    model.stale = false;
    model.origin = Origin::Initial;
    requests(model, vec![FeeOperation::StartTtl { ms: QUOTE_TTL_MS }])
}

/// A pipeline step failed. What happens depends on who started the run:
/// the initial estimate surfaces the failure (send alert / SigningSheet
/// `gasEstimateFailed`); a refresh keeps the old quote showing; a chip
/// switch reverts the selection (`GasFeeCard` catch handlers).
fn fail(model: &mut Model, kind: FeeFailure) -> Command<FeeEffect, Event> {
    match std::mem::take(&mut model.origin) {
        Origin::Initial => {
            model.phase = Phase::Failed(kind);
        }
        Origin::Refresh => {
            model.phase = if model.estimate.is_some() {
                Phase::Quoted
            } else {
                Phase::Failed(kind)
            };
        }
        Origin::Select { previous } => {
            model.fee_token = previous;
            model.phase = if model.estimate.is_some() {
                Phase::Quoted
            } else {
                Phase::Failed(kind)
            };
        }
    }
    render()
}

// ---------------------------------------------------------------------------
// Fee-asset selection
// ---------------------------------------------------------------------------

/// `GasFeeCard.handleFeeTokenSelect` + the `FeeTokenSelector` row gate.
/// On a generic in-band chain a known option recomputes locally from the
/// shared gas basis — no RPC; an unknown one (and EVERY Tempo one) falls back
/// to a full re-estimate whose failure reverts the selection.
fn select_fee_asset(model: &mut Model, token: Option<String>) -> Command<FeeEffect, Event> {
    if model.ctx.is_none() || model.phase != Phase::Quoted {
        return Command::done();
    }
    let same = match (&model.fee_token, &token) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    if same {
        return Command::done();
    }
    let option = find_option(model, token.as_deref()).cloned();
    if let Some(option) = &option {
        let amount = fee_amount_for_option(model, option);
        // Invariant ⑧ (`FeeTokenSelector.tsx:74`): a coin that can't cover
        // the fee is shown for context but NOT selectable — paying gas in it
        // would only produce a doomed op.
        if fee_row_insufficient(option.balance, amount) {
            return Command::done();
        }
        // Tempo's estimate is NOT a generic in-band quote and must never be
        // patched like one: its `total_wei` is the reimbursement in
        // attodollars (not 0), its symbol comes from the fee token, and its
        // recipient is the bundler's `settlementRecipient`, which
        // `sendUserOpTempo` compares byte for byte before signing
        // (`safe-transaction.ts:1196-1198`). Overwriting those with an in-band
        // picker row's recipient made every Tempo send throw "The gas quote
        // has expired". Re-price through `advance_tempo` instead — the same
        // path `estimateTempoFee` takes on native.
        let tempo = model
            .ctx
            .as_ref()
            .is_some_and(|ctx| is_tempo_chain(ctx.chain_id));
        if !tempo {
            if let (Some(amount), Some(estimate)) = (amount, model.estimate.as_mut()) {
                // Local recompute from the shared basis (`GasFeeCard.tsx:193-215`):
                // the displayed amount switches immediately, recipient included,
                // so approve/submit sends exactly what was quoted.
                estimate.total_wei = if option.is_native { amount } else { 0 };
                estimate.fee_recipient = Some(option.recipient.clone());
                estimate.fee_asset = match &option.fee_token {
                    None => FeeAsset::Native,
                    Some(contract) => FeeAsset::Erc20 {
                        token: contract.clone(),
                        decimals: option.decimals,
                        amount,
                        // The picked row's own ticker — the switch is supposed
                        // to be visible, and a fee with no symbol reads as the
                        // native coin everywhere it is drawn.
                        symbol: Some(option.symbol.clone()),
                    },
                };
                model.fee_token = token;
                return render();
            }
        }
    }
    // The quote may have expired while the sheet stayed open — fall back to a
    // full estimate; on failure the selection reverts (`GasFeeCard.tsx:216-227`).
    let previous = model.fee_token.clone();
    model.fee_token = token;
    model.attempt += 1;
    model.origin = Origin::Select { previous };
    begin_pipeline(model)
}

/// The picker's option set (`use-inband-fee-tokens.ts:loadInBandFeeTokenOptions`):
/// the native row always, zero-balance stables never.
fn picker_rows(model: &Model) -> impl Iterator<Item = &ParsedQuote> {
    model
        .quotes
        .iter()
        .filter(|quote| quote.is_native || quote.balance > 0)
}

fn find_option<'a>(model: &'a Model, token: Option<&str>) -> Option<&'a ParsedQuote> {
    model
        .quotes
        .iter()
        .filter(|quote| quote.is_native || quote.balance > 0)
        .find(|quote| match token {
            None => quote.is_native,
            Some(wanted) => quote
                .fee_token
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(wanted)),
        })
}

/// `GasFeeCard.feeAmountForOption`: the exact cost of THIS tx in one option's
/// coin, derived from the shared gas basis. Tempo prices erc20 rows off
/// `tempoReimbursement`; native is unpriceable there (no native gas).
fn fee_amount_for_option(model: &Model, option: &ParsedQuote) -> Option<u128> {
    let estimate = model.estimate.as_ref()?;
    if is_tempo_chain(estimate.chain_id) {
        if option.is_native {
            return None;
        }
        return Some(tempo_reimbursement(
            estimate.total_gas,
            estimate.network_fee_per_gas,
            option.decimals,
        ));
    }
    let native = find_quote(&model.quotes, None)?;
    calculate_in_band_fee_amount(
        estimate.total_gas,
        estimate.in_band_gas_basis,
        &option.pricing(),
        &native.pricing(),
    )
}

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

fn estimate_view(estimate: &FeeEstimate) -> FeeEstimateView {
    FeeEstimateView {
        chain_id: estimate.chain_id,
        total_wei: estimate.total_wei.to_string(),
        max_fee_per_gas: estimate.max_fee_per_gas.to_string(),
        network_fee_per_gas: estimate.network_fee_per_gas.to_string(),
        relayer_fee_per_gas: estimate.relayer_fee_per_gas.to_string(),
        bundler_gas_price: estimate.bundler_gas_price.to_string(),
        in_band_gas_basis: estimate.in_band_gas_basis.to_string(),
        total_gas: estimate.total_gas.to_string(),
        deployed: estimate.deployed,
        tier: estimate.tier,
        quoted: estimate.quoted,
        fee_asset: match &estimate.fee_asset {
            FeeAsset::Native => FeeAssetView::Native,
            FeeAsset::Erc20 {
                token,
                decimals,
                amount,
                symbol,
            } => FeeAssetView::Erc20 {
                token: token.clone(),
                decimals: *decimals,
                amount: amount.to_string(),
                symbol: symbol.clone(),
            },
        },
        fee_recipient: estimate.fee_recipient.clone(),
    }
}

fn option_views(model: &Model) -> Vec<FeeOptionView> {
    picker_rows(model)
        .map(|row| {
            let amount = fee_amount_for_option(model, row);
            let insufficient = fee_row_insufficient(row.balance, amount);
            let selected = match (&model.fee_token, &row.fee_token, row.is_native) {
                (None, _, true) => true,
                (Some(sel), Some(contract), false) => sel.eq_ignore_ascii_case(contract),
                _ => false,
            };
            FeeOptionView {
                symbol: row.symbol.clone(),
                contract: row.fee_token.clone(),
                decimals: row.decimals,
                balance: row.balance.to_string(),
                recipient: row.recipient.clone(),
                usd_balance: row.usd_balance.clone(),
                usd_price: row.usd_price.clone(),
                amount: amount.map(|a| a.to_string()),
                insufficient,
                selected,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must match the current attempt.
fn requests(model: &Model, operations: Vec<FeeOperation>) -> Command<FeeEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<FeeEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for FeeEffect {
    type Op = FeeOperation;
    fn into_shell(self) -> Option<crux_core::Request<FeeOperation>> {
        match self {
            FeeEffect::Render(_) => None,
            FeeEffect::Shell(request) => Some(request),
        }
    }
}
