//! Machine — send (spec `017-crux-wallet-state`, inventory `### send (P1)`).
//!
//! ```text
//! Open ─► SelectToken ─► EnterDetails ─► Continue{estimate ∥ treasury, 15s cap}
//!            │ multi picker                  │ pass ─► Confirm ─slide─► lock.begin
//!            └ split editor                  └ fail/low-float ─► stay   │
//!   Confirm: credential ─► treasury recheck ─► SubmitUserOp ─► Submitted│
//!            (each hop is a cancel checkpoint — a passkey never          ▼
//!             resurrects after Cancel)      PersistTxRecords ─► TrackSubmitted
//! ```
//!
//! The whole `useSendController.ts` (1273 lines, ~40 useState/useRef) as one
//! machine: three modes (single / split 一币多人 / multiSelect 多币一人), the
//! step state machine, EIP-681 locked-request resolution, live amount
//! validation, string-exact Max math, the same-asset fee ceiling, the treasury
//! bootstrap pre-check, the sign→submit lifecycle, and the single-flight
//! re-entry lock with generation tokens (issue #91).
//!
//! Composition with the wave-A kernels happens HERE, in Rust — never by wiring
//! core sessions together in the shell:
//!
//! - `fee_policy` supplies the money math (`same_asset_fee_limit`,
//!   `to_base_units`/`from_base_units`, `max_native_sendable`,
//!   `reserve_native_gas`/`reserve_fee_token`, `encode_erc20_transfer`) and the
//!   [`fee_policy::FeeEstimate`] the confirm screen displays. The
//!   `displayed = signed` gate: the quoted fee handed to `SubmitUserOp` is
//!   built from the very estimate this model renders (invariant ①).
//! - `tx_tracker` takes over after submission: once the pending records are
//!   persisted (invariant ⑥ ordering), the core emits [`SendOperation::TrackSubmitted`]
//!   and the shell forwards it as `tx_tracker::Event::Submitted`. Receipt
//!   convergence flows back as [`Event::ReceiptUpdate`] typed variants — all
//!   terminal-wording regexes live in the shell's result-mapping layer.
//!
//! Faithful port — behavior aligned line by line with the TS sources named per
//! item; quirks kept and marked "ported verbatim". Deliberate deviations, all
//! mandated by the migration notes:
//!
//! - `sendCancelledRef` was *written but never read* in TS (the comment claims
//!   pre-sign checkpoints that don't exist). The inventory prescribes the
//!   intent: here `Cancel` really does kill the pre-sign pipeline, so a
//!   passkey prompt can never resurrect after cancel (invariant ③).
//! - TS throws `BatchSendError` / raw `Error` in a few build paths; the JS
//!   exception has no Rust twin, so every such path fails closed into the
//!   same user-visible outcome the `catch` produced (noted per site).
//! - `makeRecipientId`'s module counter became a deterministic model counter.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

use super::fee_policy::{
    encode_erc20_transfer, from_base_units, max_native_sendable, reserve_fee_token,
    reserve_native_gas, same_asset_fee_limit, to_base_units, FeeAsset, FeeAssetView, FeeCall,
    FeeEstimate, FeeEstimateView, MultiTokenSpec,
};
use super::money::{js_parse_float, Denom, DenominatedAmount, TokenPrice};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Max recipients in one split/batch UserOp (`batch-send.ts:42`). The editor
/// and the payroll importer both cap here; the core truncates any longer seed
/// exactly as the importer trims (invariant ⑩'s "≤60 行").
pub const BATCH_MAX_RECIPIENTS: usize = 60;

/// The `Promise.race` cap on the pre-confirm estimate + treasury pre-check
/// (`useSendController.ts:768-770`). A timeout NEVER advances to confirm with
/// a fabricated preview (invariant ②).
pub const ESTIMATE_TIMEOUT_MS: u32 = 15_000;

/// How long a complete form sits still before it asks for a quote (spec 028
/// Phase 9, T490). Long enough that a person typing an amount is not quoted on
/// every digit; short enough that the fee is on the row before their thumb
/// reaches Continue.
pub const FORM_ESTIMATE_DEBOUNCE_MS: u32 = 400;

/// The literal fallback in `t('send.warnNeedGas', { sym: ... ?? 'gas token' })`
/// stays in the shell: the core reports `symbol: None` and the shell words it.
const _DOC_NEED_GAS_FALLBACK: () = ();

// ---------------------------------------------------------------------------
// Re-entry lock — ported from `src/services/reentry-lock.ts:27-51` (issue #91)
// ---------------------------------------------------------------------------

/// Single-flight re-entry lock with generation tokens. `begin` acquires (or
/// answers `None` when held) and mints a generation; `end(token)` releases only
/// if that token is still current; `cancel` force-releases AND invalidates the
/// holder, so the cancelled promise's stale `end()` can never clear a *newer*
/// send's lock (invariant ④).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReentryLock {
    held: bool,
    generation: u64,
}

impl ReentryLock {
    pub fn begin(&mut self) -> Option<u64> {
        if self.held {
            return None;
        }
        self.held = true;
        self.generation += 1;
        Some(self.generation)
    }

    pub fn end(&mut self, token: u64) -> bool {
        if self.held && token == self.generation {
            self.held = false;
            return true;
        }
        false
    }

    pub fn cancel(&mut self) {
        self.generation += 1; // invalidate the current holder's pending end()
        self.held = false;
    }

    pub fn busy(&self) -> bool {
        self.held
    }
}

// ---------------------------------------------------------------------------
// Pure helpers — JS-number semantics kept where the TS display path uses them
// ---------------------------------------------------------------------------

/// `parseFloat(x) || 0` — the `tokenBalanceDouble` shape.
fn parse_float_or_zero(s: &str) -> f64 {
    let v = js_parse_float(s);
    if v.is_nan() {
        0.0
    } else {
        v
    }
}

// `resolve_token_amount` used to live here as the free-function twin of the TS
// `resolveTokenAmount`. It took `in_fiat: bool` and no currency code, so it had
// to label the figure AND the price with the same placeholder — which made the
// currency half of `DenominatedAmount`'s guard compare `"" == ""` and pass
// unconditionally. A guard that is switched off by the only helper anyone calls
// is not a guard, so the helper is gone: every caller now names the code, on
// both platforms ([`model_token_amount`] here, `useSendController`'s
// `tokenUnitsFor` there).

/// `isValidAddress` (`send-utils.ts:8-10`).
pub fn is_valid_address(addr: &str) -> bool {
    addr.len() == 42
        && addr.starts_with("0x")
        && addr.as_bytes()[2..].iter().all(|b| b.is_ascii_hexdigit())
}

/// `recipientsAreValid` (`MultiRecipientEditor.tsx:56-60`): at least one row,
/// every row a valid (trimmed) address and a positive amount.
pub fn recipients_are_valid(recipients: &[SendRecipientDraft]) -> bool {
    !recipients.is_empty()
        && recipients
            .iter()
            .all(|r| is_valid_address(r.address.trim()) && js_parse_float(&r.amount) > 0.0)
}

/// The split editor's repeated payees: for every row whose address a row ABOVE
/// it already carries, that row's id and the 1-based position of the row it
/// repeats.
///
/// The importer already refuses a repeat (`batch_import::derived` — de-dupe by
/// lowercase address, first occurrence keeps the payment), but rows typed in,
/// picked from the book or added one at a time never met that rule, so the same
/// address could take two lines of one batch with nothing on screen saying so
/// (issue 203). Silently DROPPING a repeat here would be worse than the bug:
/// the importer drops rows a person pasted in bulk and reviews in a preview,
/// while these rows were each entered deliberately, and two payments to one
/// payee is a real thing to want. So the machine names them and leaves the
/// batch exactly as it was asked for.
///
/// Matching is the importer's: trimmed, lowercased, valid addresses only — an
/// unfinished row is not a repeat of anything, it is simply not an address yet.
pub fn duplicate_recipient_rows(recipients: &[SendRecipientDraft]) -> Vec<SendDuplicateRowView> {
    let mut first_seen: BTreeMap<String, u32> = BTreeMap::new();
    let mut repeats = Vec::new();
    for (index, row) in recipients.iter().enumerate() {
        let address = row.address.trim();
        if !is_valid_address(address) {
            continue;
        }
        let ordinal = index as u32 + 1;
        match first_seen.entry(address.to_lowercase()) {
            Entry::Vacant(slot) => {
                slot.insert(ordinal);
            }
            Entry::Occupied(slot) => repeats.push(SendDuplicateRowView {
                id: row.id.clone(),
                first_ordinal: *slot.get(),
            }),
        }
    }
    repeats
}

/// `sumSplitBaseUnits` (`batch-send.ts:100-102`). `None` when any row's amount
/// is unparsable — where TS `toBaseUnits` throws, the machine refuses instead
/// of guessing (the caller decides what the refusal means).
pub fn sum_split_base_units(recipients: &[SendRecipientDraft], decimals: u32) -> Option<u128> {
    let mut sum: u128 = 0;
    for r in recipients {
        sum = sum.saturating_add(to_base_units(&r.amount, decimals)?);
    }
    Some(sum)
}

/// `canCoverNativeTransfer` (`send-utils.ts:37-39`).
fn can_cover_native_transfer(amount_wei: u128, balance_wei: u128, quoted_fee_wei: u128) -> bool {
    amount_wei
        .checked_add(quoted_fee_wei)
        .is_some_and(|total| total <= balance_wei)
}

/// `buildTransferCall` (`batch-send.ts:68-76`): `None` where TS throws
/// `BatchSendError` (invalid recipient/token, non-positive amount) — a batch
/// that cannot be built truthfully is never built at all.
fn build_transfer_call(token_address: Option<&str>, to: &str, amount: u128) -> Option<FeeCall> {
    if !is_valid_address(to) || amount == 0 {
        return None;
    }
    match token_address {
        None => Some(FeeCall {
            to: to.to_owned(),
            value: amount.to_string(),
            data: "0x".to_owned(),
        }),
        Some(token) => {
            if !is_valid_address(token) {
                return None;
            }
            Some(FeeCall {
                to: token.to_owned(),
                value: "0".to_owned(),
                data: encode_erc20_transfer(to, amount)?,
            })
        }
    }
}

/// `buildSplitCalls` (`batch-send.ts:92-97`) — the ONE helper both the preview
/// and the submission use (invariant ⑩).
pub fn build_split_calls(
    token_address: Option<&str>,
    decimals: u32,
    recipients: &[SendRecipientDraft],
) -> Option<Vec<FeeCall>> {
    if recipients.is_empty() {
        return None;
    }
    recipients
        .iter()
        .map(|r| {
            build_transfer_call(
                token_address,
                r.address.trim(),
                to_base_units(&r.amount, decimals)?,
            )
        })
        .collect()
}

/// `buildMultiTokenCalls` (`batch-send.ts:111-114`).
pub fn build_multi_token_calls(recipient: &str, specs: &[MultiTokenSpec]) -> Option<Vec<FeeCall>> {
    if specs.is_empty() {
        return None;
    }
    specs
        .iter()
        .map(|spec| {
            build_transfer_call(
                spec.token_address.as_deref(),
                recipient,
                to_base_units(&spec.amount, spec.decimals)?,
            )
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// The slice of `APIToken` the machine needs, `chain_id` pre-resolved by the
/// shell (`apiNetworkToChainId` is registry master data).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendToken {
    /// The API network id (`APIToken.network`) — half of `tokenId()`.
    pub network: String,
    pub chain_id: u32,
    pub symbol: String,
    /// Human decimal string, exactly as the API reports it.
    pub balance: String,
    pub decimals: u32,
    /// `None` = the chain's native coin.
    pub token_address: Option<String>,
    pub price_usd: Option<f64>,
    pub logo_urls: Vec<String>,
    pub spam: bool,
}

impl SendToken {
    /// `tokenId` (`models/types.ts:56-58`).
    pub fn id(&self) -> String {
        format!(
            "{}_{}_{}",
            self.network,
            self.token_address.as_deref().unwrap_or("native"),
            self.symbol
        )
    }

    fn is_native(&self) -> bool {
        self.token_address.is_none()
    }

    /// `tokenBalanceDouble`.
    fn balance_double(&self) -> f64 {
        parse_float_or_zero(&self.balance)
    }

    /// `tokenUsdValue`.
    fn usd_value(&self) -> f64 {
        self.balance_double() * self.price_usd.unwrap_or(0.0)
    }

    /// `isMultiSelectable(tok, true)` (`batch-send.ts:121-126`) — the
    /// "select all valuable" predicate.
    fn is_valuable(&self) -> bool {
        !self.spam && self.balance_double() > 0.0 && self.usd_value() > 0.0
    }
}

/// One split-mode recipient row (`MultiRecipientEditor.RecipientDraft`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendRecipientDraft {
    /// Row identity. May arrive empty from a seed — the core assigns its
    /// deterministic `rcpt_{n}` counter (the ported `makeRecipientId`).
    pub id: String,
    pub address: String,
    /// Human decimal string.
    pub amount: String,
    /// Optional label carried from the payroll importer's name column.
    pub name: Option<String>,
}

/// The active wallet account, from `useWallet()`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendAccountRef {
    pub id: String,
    pub address: String,
    pub name: Option<String>,
}

/// Route params (`useLocalSearchParams`), pre-split by the shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendOpenParams {
    pub preselected_symbol: Option<String>,
    pub preselected_network: Option<String>,
    pub prefilled_recipient: Option<String>,
    /// Kept as the raw string so `parseInt` semantics survive verbatim.
    pub prefilled_chain_id: Option<String>,
    pub prefilled_token_address: Option<String>,
    /// Base units as a decimal string.
    pub prefilled_amount_base: Option<String>,
    pub locked: bool,
    /// Comma-joined `tokenId()`s → multiSelect hand-off.
    pub preselected_multi: Option<String>,
}

/// Display-currency context (`useDisplayCurrency`): the USD→fiat rate and the
/// fiat input precision (0 for zero-decimal codes, else 2).
///
/// `rate: None` — the shell could not price the display currency — is the one
/// state this struct exists to keep expressible. It arrives straight from
/// `display_currency`'s committed pair and means the fiat-denominated amount
/// input is unavailable: the ⇄ toggle will not enter it, and
/// [`DenominatedAmount::to_token_units`] converts nothing while it is set.
/// Token-denominated sending is untouched, because it never multiplies by this
/// number.
///
/// `code` is not decoration: it is half of what `rate` MEANS, and it is what
/// lets a figure already typed on this screen remember which currency it is
/// counted in when the display currency changes under it. Without it, "5000"
/// typed in CNY and a rate that is now USD's look identical.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendDisplayContext {
    /// The display-currency code the `rate` is quoted in ("USD", "CNY").
    pub code: String,
    /// USD → display currency. `null` ⇒ unpriceable; never 1-by-default.
    pub rate: Option<f64>,
    pub fiat_decimals: u32,
}

impl Default for SendDisplayContext {
    fn default() -> Self {
        Self {
            // The default context is USD, which really is 1 against itself.
            code: "USD".to_owned(),
            rate: Some(1.0),
            fiat_decimals: 2,
        }
    }
}

impl SendDisplayContext {
    /// This screen's fiat unit.
    fn denom(&self) -> Denom {
        Denom::fiat(self.code.clone())
    }
}

/// One supported chain, as the shell's registry knows it — what the core needs
/// to validate a locked request's network and synthesize placeholder tokens
/// (`synthNativeToken`, `send-utils.ts:49-52`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendChainInfo {
    pub chain_id: u32,
    /// The API network id (`networkId(chainId)`).
    pub network: String,
    pub native_symbol: String,
}

/// Resolved ERC-20 metadata for a locked request's unknown token.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendTokenMeta {
    pub symbol: String,
    pub decimals: u32,
}

/// `TreasuryStatus` (`bundler-service.ts`), amounts as decimal strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendTreasuryStatus {
    pub chain_id: u32,
    pub address: String,
    pub asset: SendTreasuryAsset,
    pub balance: String,
    pub floor: String,
    pub bootstrap_needed: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTreasuryAsset {
    Native,
    PathUsd,
}

/// `probeTreasury`'s four-way outcome (`bundler-service.ts:781-813`) — typed,
/// so "unknown" (transient) can never be routed as "uncovered".
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTreasuryProbe {
    LowFloat { status: SendTreasuryStatus },
    Covered,
    Uncovered,
    Unknown,
}

/// A scan, already parsed by the shell (`parseEIP681` — the parser itself is
/// wave D's `payment_request`; this machine only consumes the parse).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendScan {
    Request {
        recipient: String,
        chain_id: Option<u32>,
        token_address: Option<String>,
        /// Base units as a decimal string.
        amount_base_units: Option<String>,
    },
    /// Unparseable — the raw text is used as an address, as today.
    Text { data: String },
}

/// `addCustomNetworkByChainId`'s outcome, typed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendAddNetworkOutcome {
    Added,
    NotFound,
    NotCompatible { detail: Option<String> },
    Error,
}

/// The line under the add-network button (semantic — shell owns the words).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendAddNetworkMsg {
    NetNotFound,
    NetNotCompatible { detail: Option<String> },
    NetAddError,
}

/// Why a locked request cannot be fulfilled (`useSendController.ts:86-89`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendLockError {
    Network { chain_id: u32 },
    Token,
}

/// Recipient identity (passkey index → ENS), best-effort display data.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendRecipientIdentity {
    pub name: Option<String>,
    pub source: Option<String>,
}

/// Recipient risk signals for the confirm step, best-effort.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendRecipientRisk {
    pub is_contract: Option<bool>,
    pub first_time: Option<bool>,
}

/// The fee the submit path signs — EXACTLY what the confirm screen displayed
/// (invariant ①, `useSendController.ts:933-943`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendQuotedFee {
    /// Base units as a decimal string.
    pub amount: String,
    pub recipient: String,
}

/// One pending activity record (`useSendController.ts:1014-1034`). The shell
/// maps this onto `LocalTransaction`, adding the constant `status: 'pending'`
/// and `type: 'send'`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendTxRecord {
    pub id: String,
    pub user_op_hash: String,
    /// Always empty at persist time — the tracker patches it in later.
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    pub to_name: Option<String>,
    /// Human decimal amount string.
    pub value: String,
    pub symbol: String,
    pub decimals: u32,
    pub logo_urls: Vec<String>,
    pub chain_id: u32,
    /// Epoch seconds (`Math.floor(Date.now() / 1000)` — derived from the
    /// submit result's `now_ms`; the core holds no clock).
    pub timestamp_s: f64,
    /// `'$' + usd.toFixed(2)` when > 0 — a stored-record format, not i18n
    /// (ported verbatim).
    pub usd: Option<String>,
}

/// Estimate failure vocabulary — `fee_policy::FeeFailure` plus the send-side
/// timeout; the shell maps service errors into these.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendEstimateFailure {
    MissingPublicKey,
    FeeTokenUnavailable,
    QuoteUnavailable,
    CalculationFailed,
    EstimateFailed,
    /// The bundler quoted a gas price far above the client's own on-chain
    /// measurement — refused, not signed (mirrors `FeeFailure::GasQuoteTooHigh`;
    /// the send vocabulary is the fee vocabulary).
    GasQuoteTooHigh,
    /// The 15s race lost (`useSendController.ts:768-770`).
    Timeout,
    Other,
}

/// How a submit failed — the shell's result-mapping layer runs
/// `parseBundlerUnderfunded` and the `/gas relayer is unavailable/i` regex
/// (`useSendController.ts:1072-1104`); the core only ever sees typed variants
/// and only ever emits semantic error keys (invariant ⑮).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendSubmitFailure {
    PasskeyCancelled,
    RelayerUnavailable,
    BundlerUnderfunded,
    /// `message` is diagnostics-only (the TS path logs it) — it never reaches
    /// the view.
    Other {
        message: Option<String>,
    },
}

/// Post-submit receipt convergence, fed by the shell from `tx_tracker`
/// outcomes. Typed: only a definitive drop/revert/rejection may arrive as
/// `Failed` — a slow or unreachable poll simply never sends anything
/// (invariant ⑤).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendReceiptOutcome {
    Confirmed {
        tx_hash: String,
    },
    Failed {
        rejected: bool,
    },
    /// The relay parked the op until fees settle — pending, new wording only
    /// (invariant ⑦).
    FeeHeld,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTimerTag {
    EstimateTimeout,
    /// The form's own quote, debounced (spec 028 Phase 9, T490).
    FormEstimate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendHapticKind {
    Success,
    Error,
}

/// Live amount validation (semantic; the shell owns the words and resolves a
/// `None` symbol from its chain registry).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendAmountWarning {
    NotEnoughToken {
        symbol: String,
    },
    InsufficientForGas {
        symbol: Option<String>,
    },
    /// The fee alone is more than the whole balance of the asset that pays it:
    /// there is no amount this account can send on this network right now.
    ///
    /// This is the state `Max` resolves to `"0"` — correct, and for a long time
    /// silent. A control that fills a figure has to say why the figure is
    /// nothing, or the person is left deciding between "the fee ate my balance"
    /// and "the button is broken" (issue #210).
    InsufficientGas {
        symbol: Option<String>,
    },
    NeedGas {
        symbol: Option<String>,
    },
    /// A fiat-denominated figure this screen cannot restate in token units:
    /// the digits are fine, it is the FACTOR that is missing (no rate for
    /// `code`, or no price for the token). Without this the screen showed a
    /// perfectly ordinary "5000", a `⇅ 0 SYM` row, and a `Continue` that
    /// refused with nothing said — the amount resolved to `"0"` and no surface
    /// admitted why.
    CannotConvert {
        code: String,
        symbol: String,
    },
}

/// The two nouns every "these units cannot be crossed" sentence on this screen
/// needs: the currency on screen and the token being sent.
///
/// Carried by the fields that explain a REFUSAL — a control that visibly
/// declines ([`SendView::denom_toggle_reason`]) and a gate that silently
/// declines ([`SendView::confirm_amount_issue`]). Both refusals existed before
/// this type; neither said anything, which is the same defect twice.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendUnitIssue {
    /// The display currency this screen would have to cross into.
    pub code: String,
    /// The selected token's symbol.
    pub symbol: String,
}

/// One `showAlert` call site each (semantic keys only).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendAlertKind {
    InvalidAddress,
    InvalidAmount,
    InsufficientBalance { warning: Option<SendAmountWarning> },
    SplitOverBalance,
    LoadTokensFailed,
    EstimateFailed { kind: SendEstimateFailure },
    AccountUnavailable,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Sentences — the shell owns
/// transports, caches (including `prefetchForSend` warming), the passkey
/// ceremony inside `SubmitUserOp`, and every wording regex.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendOperation {
    /// `fetchTokens(address)` — answers ONCE with the full list (progressive
    /// chain results arrive as [`Event::TokensPartial`]).
    FetchTokens {
        address: String,
    },
    ClearTokenCache {
        address: String,
    },
    /// `resolveTokenMetadata(chain, [addr])` for a locked request's unknown
    /// token.
    ResolveTokenMetadata {
        chain_id: u32,
        address: String,
    },
    /// `addCustomNetworkByChainId`.
    AddNetwork {
        chain_id: u32,
    },
    /// `estimateTransactionFee(address, chain, 'fast', tx, batch, feeToken,
    /// publicKeyHex)`. The shell may satisfy it with the `fee_policy` machine;
    /// the answer is the same wire estimate either way.
    EstimateFee {
        chain_id: u32,
        account: String,
        tx: Option<FeeCall>,
        batch: Option<Vec<FeeCall>>,
        gas_fee_token: Option<String>,
        public_key_hex: Option<String>,
    },
    /// `probeTreasury(chainId)`.
    ProbeTreasury {
        chain_id: u32,
    },
    /// `findAccountByCredentialId(id)` → the stored public key.
    LoadAccountCredential {
        account_id: String,
    },
    /// The whole sign→submit orchestration (`sendNative`/`sendERC20`/
    /// `sendBatchCalls`). The shell dispatches [`Event::SigningStarted`] when
    /// the passkey sheet opens and answers exactly once with
    /// `Submitted`/`SubmitFailed`.
    SubmitUserOp {
        chain_id: u32,
        account: String,
        public_key_hex: String,
        calls: Vec<FeeCall>,
        max_fee_per_gas: Option<String>,
        gas_fee_token: Option<String>,
        /// Present ⇔ in-band: sign EXACTLY this (invariant ①).
        quoted_fee: Option<SendQuotedFee>,
    },
    /// `Passkey.cancelSign()`.
    CancelPasskeySign,
    /// Persist ALL sibling records in ONE atomic write (invariant ⑥ —
    /// `saveTransactions(records)`, never per-record).
    PersistTxRecords {
        records: Vec<SendTxRecord>,
    },
    /// Hand the accepted op to `tx_tracker` (`Event::Submitted` there).
    /// Emitted only AFTER `RecordsPersisted`, so the tracker's patches always
    /// find their records (invariant ⑥'s ordering half).
    TrackSubmitted {
        user_op_hash: String,
        record_ids: Vec<String>,
        chain_id: u32,
    },
    /// `resolveRecipientIdentity(addr)`.
    ResolveIdentity {
        address: String,
    },
    /// `resolveRecipientRisk(chain, addr)`.
    ResolveRisk {
        chain_id: u32,
        address: String,
    },
    /// `simulateAssetChanges(account, calls, chain)`.
    SimulateCalls {
        chain_id: u32,
        account: String,
        calls: Vec<FeeCall>,
    },
    StartTimer {
        ms: u32,
        tag: SendTimerTag,
    },
    Haptic {
        kind: SendHapticKind,
    },
    ShowAlert {
        kind: SendAlertKind,
    },
    /// Leave the Send flow (`router.back()` from the first step / receipt).
    Close,
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendShellResult {
    /// `tokens: None` = the load failed (`catch` →
    /// `send.alertLoadTokensError`). The FULL list — the core derives the
    /// non-zero sorted display list and matches locked requests against
    /// everything, exactly as today.
    TokensLoaded {
        tokens: Option<Vec<SendToken>>,
        chains: Vec<SendChainInfo>,
    },
    TokenCacheCleared,
    TokenMetadata {
        meta: Option<SendTokenMeta>,
    },
    NetworkAdded {
        outcome: SendAddNetworkOutcome,
    },
    FeeEstimated {
        outcome: SendFeeOutcome,
    },
    TreasuryProbed {
        probe: SendTreasuryProbe,
    },
    /// `None` = the account record is missing its public key, or the read
    /// threw — both alert `send.alertAccountUnavailableBody` today.
    AccountCredential {
        public_key_hex: Option<String>,
    },
    Submitted {
        user_op_hash: String,
        now_ms: f64,
    },
    SubmitFailed {
        failure: SendSubmitFailure,
    },
    PasskeyCancelAcknowledged,
    RecordsPersisted,
    TrackHandedOff,
    IdentityResolved {
        identity: Option<SendRecipientIdentity>,
    },
    RiskResolved {
        risk: Option<SendRecipientRisk>,
    },
    /// Opaque `AssetSimResult` JSON — display-only, the core never decides on
    /// it.
    SimResolved {
        sim_json: Option<String>,
    },
    TimerElapsed {
        tag: SendTimerTag,
    },
    AlertAcknowledged,
    HapticPlayed,
    Closed,
}

/// An estimate, or a typed refusal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
#[allow(clippy::large_enum_variant)] // a wire type: the JSON shape is pinned by the generated TS
pub enum SendFeeOutcome {
    Ok { estimate: FeeEstimateView },
    Failed { kind: SendEstimateFailure },
}

impl Operation for SendOperation {
    type Output = SendShellResult;
}

#[effect]
pub enum SendEffect {
    Render(RenderOperation),
    Shell(SendOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SendEvent"))]
pub enum Event {
    /// Screen mount. Carries everything the controller read from its hooks.
    Open {
        account: Option<SendAccountRef>,
        params: SendOpenParams,
        display: SendDisplayContext,
    },
    /// The display currency changed while the screen is open.
    DisplayChanged {
        display: SendDisplayContext,
    },
    /// A progressive `fetchTokens` chunk (`onProgress`) — display-only, never
    /// consulted by lock resolution.
    TokensPartial {
        tokens: Vec<SendToken>,
    },
    /// The user added/removed a custom token — re-pull without a page refresh.
    RefreshTokens,
    SelectToken {
        token_id: String,
    },
    ToggleMultiToken {
        token_id: String,
    },
    /// The picker's master "select all valuable" row.
    ///
    /// `visible_ids` is what the picker is SHOWING (its search/category/chain
    /// filtered rows, in `TokenSelector`'s order) — sweeping a token the user
    /// cannot see is a fund-safety regression, so the shell states the scope.
    /// Which of those are worth sweeping, and whether the row toggles on or
    /// off, stay this machine's ([`SendToken::is_valuable`], the ported
    /// `isMultiSelectable(tok, true)`): the shell owns no money predicate.
    /// An id the machine does not hold is ignored.
    ToggleAllMultiTokens {
        visible_ids: Vec<String>,
    },
    SetMultiNetwork {
        chain_id: Option<u32>,
    },
    ConfirmMultiSelection,
    SetRecipient {
        recipient: String,
    },
    SetAmount {
        amount: String,
    },
    /// The ⇄ conversion toggle (`EnterDetailsStep.tsx:165-176`) — converts the
    /// typed amount across the fiat boundary, then flips the mode.
    ToggleFiatInput,
    TapMax,
    EnterSplitMode,
    /// Batch import / whole-group pick → seed split rows directly.
    SeedSplitRecipients {
        recipients: Vec<SendRecipientDraft>,
    },
    /// The split editor's whole-array onChange; ≤1 row collapses back to
    /// single mode carrying the remaining row.
    RecipientsChanged {
        recipients: Vec<SendRecipientDraft>,
    },
    /// `target` = the split row the picker fills; `None` = the single-mode
    /// recipient field.
    OpenContactPicker {
        target: Option<String>,
    },
    CloseContactPicker,
    /// The contact picker chose an address.
    PickedAddress {
        address: String,
    },
    OpenScanner,
    CloseScanner,
    /// A scan, parsed by the shell. Routing (`SendScreen.tsx:181-203`): a
    /// targeted split row takes ONLY the address (invariant ⑬); a full
    /// request re-locks the whole flow; anything else fills the recipient.
    ScanResolved {
        scan: SendScan,
    },
    OpenBatchImport,
    CloseBatchImport,
    /// "Add this network" on the locked-request exception screen.
    AddNetworkTapped {
        chain_id: u32,
    },
    Continue,
    Back,
    /// "Edit amount" — the recovery from a blocked confirmation.
    EditAmount,
    /// Fee-asset chip (`setGasFeeToken`); the embedded fee card re-quotes and
    /// answers via [`Event::FeeUpdated`].
    ChooseFeeToken {
        token: Option<String>,
    },
    /// The fee card settled a (re)quote (`GasFeeCard.onFeeUpdate`).
    FeeUpdated {
        estimate: FeeEstimateView,
    },
    /// The fee card is re-quoting (`onBusyChange`) — confirm stays disabled
    /// while true.
    FeeBusyChanged {
        busy: bool,
    },
    /// The slide-to-confirm completed.
    SlideConfirm,
    /// The passkey sheet opened inside `SubmitUserOp`.
    SigningStarted,
    /// The confirm screen's cancel (✕) during preparing/signing.
    CancelSigning,
    /// Treasury sheet "retry" — re-runs the step-appropriate flow.
    RetryAfterBootstrap,
    DismissTreasurySheet,
    /// The error panel's retry — back to idle.
    RetryAfterError,
    /// Receipt convergence from the tracker (shell-mapped, typed).
    ReceiptUpdate {
        user_op_hash: String,
        outcome: SendReceiptOutcome,
    },
    /// Receipt "Done".
    Done,
    /// Internal: an effect resolved. `attempt` is the per-request id captured
    /// by the core when the request was made; a result whose id no flight
    /// expects belongs to a superseded run and is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: SendShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendStep {
    #[default]
    SelectToken,
    EnterDetails,
    Confirm,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTxStatus {
    #[default]
    Idle,
    Preparing,
    Signing,
    Submitting,
    Confirmed,
    Error,
}

/// The two error wordings the confirm screen may show (invariant ⑮: semantic
/// keys only — a raw RPC/library message never reaches this enum).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTxErrorKey {
    /// `send.txErrorGeneric`.
    Generic,
    /// `send.txErrorBundlerFund`.
    BundlerFund,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendReceiptKind {
    Split,
    MultiSelect,
}

/// Why did we ask for the token list?
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TokensPurpose {
    /// Mount / lock retry — runs the preselection & lock routing afterwards.
    Initial,
    /// The refresh after a custom-token edit — errors are swallowed.
    Refresh,
}

/// One activity line captured at submit time (`lines`,
/// `useSendController.ts:950-979`).
#[derive(Clone, Debug, PartialEq)]
struct SendLine {
    to: String,
    to_name: Option<String>,
    amount: String,
    symbol: String,
    decimals: u32,
    price_usd: f64,
    logo_urls: Vec<String>,
}

/// What a generic-vs-funded failure falls back to once the treasury probe
/// answers (`useSendController.ts:1078-1096`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FailureFallback {
    Generic,
    BundlerFund,
}

/// The single in-flight orchestration. Every variant embeds the request ids it
/// is waiting on; a result carrying any other id is stale and dropped.
#[derive(Clone, Debug, Default, PartialEq)]
// One `Pipeline` exists at a time, inside the model. Boxing the wide variants
// would add an allocation to every state transition to save bytes nothing counts.
#[allow(clippy::large_enum_variant)]
enum Pipeline {
    #[default]
    Idle,
    /// `handleContinue`'s credential load (prefetch missed).
    ContinueCredential {
        id: u64,
    },
    /// The `Promise.all([estimate, treasury])` racing the 15s timer.
    PreCheck {
        fee_id: u64,
        treasury_id: u64,
        timer_id: u64,
        fee: Option<FeeEstimate>,
        treasury: Option<Option<SendTreasuryStatus>>,
    },
    /// After the timeout alert: a late successful estimate still lands in the
    /// model (ported verbatim — TS's raced-out `preCheck` keeps running its
    /// `setFeeEstimate`); a late failure or treasury answer is dropped.
    LateFee {
        fee_id: u64,
    },
    /// `handleMaxAmount`'s on-demand estimate.
    MaxEstimate {
        id: u64,
    },
    /// The form's quote, waiting for the typing to stop (T490). Re-armed on
    /// every change; only the newest timer is answered.
    FormDebounce {
        timer_id: u64,
    },
    /// The form's quote in flight. `key` names what it is about (token, payee,
    /// fee coin), so the same form is not quoted twice.
    FormEstimate {
        id: u64,
        key: String,
    },
    /// `confirmSelection` / `preselectedMulti` warm-up: credential, then a
    /// best-effort background estimate whose failure is swallowed.
    WarmCredential {
        id: u64,
    },
    WarmEstimate {
        id: u64,
    },
    /// `executeTransaction` pre-sign hops — cancel checkpoints (invariant ③).
    SubmitCredential {
        id: u64,
        gen: u64,
    },
    SubmitTreasury {
        id: u64,
        gen: u64,
        public_key_hex: String,
    },
    /// The sign→submit is in flight. Kept alive across a Cancel-during-signing
    /// (the shell's outcome decides, exactly as TS), replaced by any newer
    /// slide.
    Submitting {
        id: u64,
        gen: u64,
        chain_id: u32,
        lines: Vec<SendLine>,
    },
    /// A classified failure is re-probing the treasury before wording the
    /// error (`useSendController.ts:1078-1096`).
    FailureProbe {
        id: u64,
        gen: u64,
        fallback: FailureFallback,
    },
}

/// Free-floating flights that legitimately overlap the pipeline.
#[derive(Clone, Debug, Default, PartialEq)]
struct Flights {
    tokens: Option<(u64, TokensPurpose)>,
    /// (id, chain_id, token_address) of a locked request's metadata lookup.
    lock_meta: Option<(u64, u32, String)>,
    add_network: Option<u64>,
    /// The token-select prefetch (`findAccountByCredentialId` warm).
    prefetch_credential: Option<u64>,
    identity: Option<u64>,
    risk: Option<u64>,
    sim: Option<u64>,
    /// The post-submit persistence chain: records first, tracker second.
    persist: Option<(u64, PersistCtx)>,
    track: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
struct PersistCtx {
    user_op_hash: String,
    record_ids: Vec<String>,
    chain_id: u32,
}

#[derive(Default)]
pub struct Model {
    account: Option<SendAccountRef>,
    params: SendOpenParams,
    display: SendDisplayContext,
    step: SendStep,
    lock_error: Option<SendLockError>,
    resolving_lock: bool,
    adding_network: bool,
    add_network_msg: Option<SendAddNetworkMsg>,
    /// Non-zero balances, sorted by USD value descending — the display list.
    tokens: Vec<SendToken>,
    chains: Vec<SendChainInfo>,
    loading: bool,
    selected_token: Option<SendToken>,
    recipient: String,
    /// Canonical dot-decimal, exactly as typed/sanitized — **plus the unit it
    /// is counted in**.
    ///
    /// This used to be a `String` and a separate `input_in_fiat: bool`, and
    /// that pair is precisely how the last defect was written: flip the bool,
    /// leave the digits, and a figure typed in CNY became a figure of USDC
    /// without anything ever multiplying by anything. [`DenominatedAmount`]'s
    /// fields are private to `money`, so from here the unit can only change by
    /// [`DenominatedAmount::convert`] — which restates the digits or fails.
    amount: DenominatedAmount,
    split_mode: bool,
    recipients: Vec<SendRecipientDraft>,
    picker_target: Option<String>,
    /// The ported `makeRecipientId` counter — survives a scan re-lock, like
    /// the TS module counter survives a remount.
    recipient_seq: u64,
    multi_select_mode: bool,
    multi_selected_ids: Vec<String>,
    multi_chain_id: Option<u32>,
    show_scanner: bool,
    show_contact_picker: bool,
    show_batch_import: bool,
    /// The one estimate every surface prices against; exposed only while its
    /// own `chain_id` matches the selected token (`selectedFeeEstimate`,
    /// `useSendController.ts:119-121`).
    fee_estimate: Option<FeeEstimate>,
    estimating_gas: bool,
    fee_busy: bool,
    gas_fee_token: Option<String>,
    treasury_bootstrap: Option<SendTreasuryStatus>,
    lock: ReentryLock,
    /// The ported `sendCancelledRef` intent: every pre-sign hop checks it.
    cancelled: bool,
    tx: SendTxStatus,
    tx_error: Option<SendTxErrorKey>,
    tx_hash: Option<String>,
    user_op_hash: Option<String>,
    receipt_lines: Option<Vec<SendLine>>,
    receipt_kind: Option<SendReceiptKind>,
    /// **The money that was signed**, captured at the instant the bundler
    /// accepted the UserOp — not re-derived afterwards.
    ///
    /// The receipt used to ask [`model_token_amount`] for its headline figure,
    /// which re-runs the fiat↔token conversion against whatever display
    /// context is on screen *now*. That is a live computation about a fact
    /// that stopped being live the moment the calldata was signed: change the
    /// display currency on the receipt and the number changed with it (and
    /// with the rate gone it read `0`), so a receipt could show token amounts
    /// that were never in any signature. An amount already on-chain cannot be
    /// rewritten by a currency picker, so it is snapshotted here and the
    /// receipt only ever reads it.
    ///
    /// Holds the FIRST signed line (the whole of a single send; the batch
    /// modes additionally carry every line in `receipt_lines`, which is the
    /// same snapshot discipline — those were already captured at submit).
    receipt_signed: Option<SendLine>,
    receipt_failed: bool,
    fee_held: bool,
    /// The submit result's clock (#D3); `None` until the relay accepted.
    submitted_at_ms: Option<f64>,
    fee_rejected: bool,
    recipient_identity: Option<SendRecipientIdentity>,
    recipient_risk: Option<SendRecipientRisk>,
    sim_json: Option<String>,
    /// `prefetchedAccount.current?.publicKeyHex`.
    public_key_hex: Option<String>,
    pipeline: Pipeline,
    flights: Flights,
    /// Monotonic per-request id source (every request gets a fresh one).
    attempt: u64,
    /// What the quote on the form is about (see [`form_estimate_key`]), once
    /// one has landed — the same form is not quoted again until it changes.
    form_quote_key: Option<String>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// Which surface the screen shows — the `SendScreen.tsx:142-177` routing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendStage {
    LockError,
    LockResolving,
    Receipt,
    SelectToken,
    EnterDetails,
    Confirm,
}

/// A same-asset ceiling breach (`sameAssetFeeIssue`) — base-unit decimal
/// strings; the shell formats.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendFeeIssueView {
    pub symbol: String,
    pub transfer_amount: String,
    pub balance: String,
    pub fee_amount: String,
    pub total: String,
    pub max_transfer_amount: String,
}

/// One multiSelect line, net of its gas reserve — the EXACT amounts a submit
/// would move (invariant ⑪: preview and signature share `multi_token_specs`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendMultiSpecView {
    pub token_address: Option<String>,
    pub decimals: u32,
    pub amount: String,
}

/// One split row that pays an address an earlier row already pays — the row's
/// draft id and the 1-based position of the row it repeats
/// ([`duplicate_recipient_rows`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendDuplicateRowView {
    pub id: String,
    pub first_ordinal: u32,
}

/// One receipt line for batch sends (`ReceiptTransfer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendReceiptTransfer {
    pub to: String,
    pub to_name: Option<String>,
    pub amount: String,
    pub symbol: String,
    pub logo_urls: Vec<String>,
    pub usd_value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendReceiptStatus {
    Submitted,
    Confirmed,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendHoldReason {
    FeeHold,
    FeeRejected,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendReceiptView {
    pub status: SendReceiptStatus,
    pub hold_reason: Option<SendHoldReason>,
    pub kind: Option<SendReceiptKind>,
    pub transfers: Vec<SendReceiptTransfer>,
    /// The single-send scalar amount (token units, resolved).
    pub amount: String,
    pub usd_value: f64,
    /// When the relay accepted the op (the submit result's clock), so the
    /// shell can show how long the wait has been (spec 038 #D3). The core is
    /// clockless: elapsed is the shell's subtraction, with its own clock.
    pub submitted_at_ms: Option<f64>,
    /// The chain's usual time to land, from the builtin table; `None` for a
    /// custom network, where the shell says nothing rather than guess.
    pub typical_inclusion_s: Option<u16>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendView {
    pub stage: SendStage,
    pub loading: bool,
    pub locked: bool,
    /// The amount is fixed only when the locked request actually named one.
    pub amount_locked: bool,
    pub lock_error: Option<SendLockError>,
    pub resolving_lock: bool,
    pub adding_network: bool,
    pub add_network_msg: Option<SendAddNetworkMsg>,
    pub tokens: Vec<SendToken>,
    pub selected_token: Option<SendToken>,
    pub recipient: String,
    pub amount: String,
    /// The unit `amount` is counted in: `None` = the selected token's own
    /// units, `Some(code)` = that fiat currency.
    ///
    /// This is the figure's OWN code, straight off [`DenominatedAmount`] — not
    /// the display currency. The two can differ for exactly one instant (a
    /// commit lands under a screen that already has a figure on it), and that
    /// instant is when the screen used to lie: it had only a `bool` here, so it
    /// labelled the number with whatever `dc.code` happened to be, and a figure
    /// typed in USD was printed as CNY. A boolean cannot name a currency, so
    /// the boolean is gone; the screen renders THIS and never re-derives the
    /// unit from the display context.
    ///
    /// (`display_changed` re-denominates the field, so the mismatch does not
    /// outlive the event — see `redenominate_to_display`. This field is what
    /// makes that unnecessary to trust.)
    pub amount_fiat_code: Option<String>,
    /// Whether the ⇄ row is offered at all.
    ///
    /// Ported condition: the token has a price. Plus one addition — it is ALSO
    /// offered whenever the figure is already fiat-denominated, because that
    /// row is the only way back out, and a token that loses its price while a
    /// fiat figure is on screen used to take the exit with it.
    pub denom_toggle_shown: bool,
    /// Whether pressing ⇄ would change anything. Entering fiat needs a price in
    /// the display currency; leaving is always allowed. Without this the
    /// control looked live and did nothing at all when the currency was
    /// unpriceable — the refusal was real but invisible.
    pub denom_toggle_enabled: bool,
    /// **Why** ⇄ is inert, when it is inert.
    ///
    /// The previous round made the refusal VISIBLE (the row dims) and stopped
    /// there, so this was the one branch on the screen where nothing said what
    /// was wrong: a priced token whose display currency has no rate leaves the
    /// figure in token units, which resolves perfectly, so no amount warning
    /// fires either. A dimmed control with no sentence is a refusal the user
    /// cannot act on. `Some` exactly when `denom_toggle_shown && !enabled`.
    pub denom_toggle_reason: Option<SendUnitIssue>,
    /// **Why** the confirm slide is disarmed, when what disarmed it is the
    /// money.
    ///
    /// [`SendView::can_confirm`] never looked at the amount at all: a
    /// display-currency commit landing while the confirm page is open
    /// re-denominates the field to empty (`redenominate_to_display`), and the
    /// slider stayed armed over a figure that resolved to nothing — a
    /// zero-value transfer, signable, unexplained. The gate now asks the same
    /// question `can_continue` asks, and this is the sentence that goes with
    /// the refusal (`send.warnCannotConvert`, the key that round added).
    pub confirm_amount_issue: Option<SendUnitIssue>,
    /// `amount` already resolved through the fiat↔token conversion — the ONE
    /// number the confirm page may display, because it is the very number the
    /// signed batch is built from (`resolve_token_amount`, invariant "displayed
    /// == signed"). Empty while no token is selected.
    pub token_amount: String,
    /// The single figure the confirm page prints beside From/To — always in
    /// TOKEN units, always this machine's.
    ///
    /// A 1→1 send restates [`SendView::token_amount`]. A SPLIT restates the
    /// sum the money gates already read: the same [`sum_split_base_units`]
    /// that `Continue` refuses an over-balance batch on, that
    /// [`derive_same_asset_issue`] measures against the fee ceiling, and that
    /// `build_split_calls` turns into the signed transfers. It is not a second
    /// derivation of the total — it is that total, said out loud.
    ///
    /// The shell used to sum the rows itself (`ConfirmStep.tsx:85`), which put
    /// a number on the signing page that nothing else in the flow had agreed
    /// to, and whose TS `toBaseUnits` THREW on a row this machine merely
    /// declines — a white confirm page instead of a refusal. An unresolvable
    /// row now answers `""` here (the shell prints its own zero), and the
    /// existing gates keep the batch off the passkey.
    ///
    /// Empty in multiSelect: that mode has no single headline (the per-token
    /// rows come from [`SendView::multi_specs`]).
    pub confirm_amount: String,
    pub split_mode: bool,
    pub recipients: Vec<SendRecipientDraft>,
    /// Split mode only: the rows' total exceeds the selected token's balance.
    /// The same predicate the `Continue` gate refuses on
    /// (`SendAlertKind::SplitOverBalance`), so the live hint and the gate can
    /// never disagree.
    pub split_over_balance: bool,
    /// Split mode only: the rows that repeat a payee an earlier row already has
    /// (issue 203). The batch is still exactly what was asked for — this is the
    /// sentence beside the repeating row, not a refusal, and never flags the
    /// first occurrence: that is the row the repeat repeats.
    pub split_duplicates: Vec<SendDuplicateRowView>,
    pub picker_target: Option<String>,
    pub multi_select_mode: bool,
    pub multi_selected_ids: Vec<String>,
    /// Every held id on the filtered chain that "select all valuable" would
    /// sweep. The picker's master tick is `visible ∩ this`, all selected — the
    /// shell narrows the SCOPE to what is on screen and never re-decides what
    /// counts as valuable.
    pub multi_valuable_ids: Vec<String>,
    pub multi_chain_id: Option<u32>,
    /// Reserved multiSelect amounts for the selected token's chain.
    pub multi_specs: Vec<SendMultiSpecView>,
    pub show_scanner: bool,
    pub show_contact_picker: bool,
    pub show_batch_import: bool,
    pub estimating_gas: bool,
    pub fee_busy: bool,
    /// Chain-guarded (`selectedFeeEstimate`) — never a prior network's quote.
    pub fee: Option<FeeEstimateView>,
    pub gas_fee_token: Option<String>,
    pub amount_warning: Option<SendAmountWarning>,
    pub same_asset_fee_issue: Option<SendFeeIssueView>,
    pub can_continue: bool,
    /// The confirm slide gate: fee settled ∧ nothing re-quoting ∧ no
    /// same-asset breach ∧ idle.
    pub can_confirm: bool,
    pub sending: bool,
    pub tx_status: SendTxStatus,
    pub tx_error: Option<SendTxErrorKey>,
    pub tx_hash: Option<String>,
    pub user_op_hash: Option<String>,
    pub receipt: Option<SendReceiptView>,
    pub treasury_bootstrap: Option<SendTreasuryStatus>,
    pub recipient_identity: Option<SendRecipientIdentity>,
    pub recipient_risk: Option<SendRecipientRisk>,
    pub sim_json: Option<String>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Send;

type Cmd = Command<SendEffect, Event>;

impl App for Send {
    type Event = Event;
    type Model = Model;
    type ViewModel = SendView;
    type Effect = SendEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        match event {
            Event::Open {
                account,
                params,
                display,
            } => open(model, account, params, display),
            Event::DisplayChanged { display } => {
                model.display = display;
                redenominate_to_display(model);
                render()
            }
            Event::TokensPartial { tokens } => tokens_partial(model, tokens),
            Event::RefreshTokens => refresh_tokens(model),
            Event::SelectToken { token_id } => select_token(model, &token_id),
            Event::ToggleMultiToken { token_id } => toggle_multi_token(model, token_id),
            Event::ToggleAllMultiTokens { visible_ids } => toggle_all_multi(model, &visible_ids),
            Event::SetMultiNetwork { chain_id } => {
                // A batch is one chain: changing the filter clears the pick
                // (`use-token-multi-select.ts:49-52`, invariant ⑪).
                model.multi_selected_ids.clear();
                model.multi_chain_id = chain_id;
                render()
            }
            Event::ConfirmMultiSelection => confirm_multi_selection(model),
            Event::SetRecipient { recipient } => {
                if view_recipient_locked(model) {
                    // The field is not editable then — same shape as the
                    // amount lock below. A refusal here can never surprise
                    // anyone: the control that would send this event renders
                    // disabled, so nothing is typed to be swallowed. (The
                    // contact picker and the scanner are NOT gated here for
                    // the opposite reason — they are removed from the screen
                    // rather than shown disabled, so a silent no-op on them
                    // would be a dead button.)
                    return Command::done();
                }
                model.recipient = recipient;
                Command::all([
                    sync_identity(model),
                    schedule_form_estimate(model),
                    render(),
                ])
            }
            Event::SetAmount { amount } => {
                if view_amount_locked(model) {
                    return Command::done(); // the field is not editable then
                }
                // The text field edits the figure; the unit is whatever the
                // ⇄ toggle last established and only `convert` may change it.
                model.amount = model.amount.with_value(amount);
                Command::all([schedule_form_estimate(model), render()])
            }
            Event::ToggleFiatInput => toggle_fiat_input(model),
            Event::TapMax => tap_max(model),
            Event::EnterSplitMode => enter_split_mode(model),
            Event::SeedSplitRecipients { recipients } => seed_split(model, recipients),
            Event::RecipientsChanged { recipients } => recipients_changed(model, recipients),
            Event::OpenContactPicker { target } => {
                model.picker_target = target;
                model.show_contact_picker = true;
                render()
            }
            Event::CloseContactPicker => {
                model.show_contact_picker = false;
                render()
            }
            Event::PickedAddress { address } => apply_picked_address(model, address),
            Event::OpenScanner => {
                model.show_scanner = true;
                render()
            }
            Event::CloseScanner => {
                model.show_scanner = false;
                render()
            }
            Event::ScanResolved { scan } => scan_resolved(model, scan),
            Event::OpenBatchImport => {
                model.show_batch_import = true;
                render()
            }
            Event::CloseBatchImport => {
                model.show_batch_import = false;
                render()
            }
            Event::AddNetworkTapped { chain_id } => add_network(model, chain_id),
            Event::Continue => handle_continue(model),
            Event::Back => handle_back(model),
            Event::EditAmount => edit_amount(model),
            Event::ChooseFeeToken { token } => {
                model.gas_fee_token = token;
                // On the form the fee coin is part of what the quote is about.
                Command::all([schedule_form_estimate(model), render()])
            }
            Event::FeeUpdated { estimate } => fee_updated(model, estimate),
            Event::FeeBusyChanged { busy } => {
                model.fee_busy = busy;
                render()
            }
            Event::SlideConfirm => slide_confirm(model),
            Event::SigningStarted => {
                // Order is preparing → submitting → signing, as today
                // (`setTxStatus('submitting')` precedes `signFn`).
                if model.tx == SendTxStatus::Submitting
                    && matches!(model.pipeline, Pipeline::Submitting { .. })
                {
                    model.tx = SendTxStatus::Signing;
                    return render();
                }
                Command::done()
            }
            Event::CancelSigning => cancel_signing(model),
            Event::RetryAfterBootstrap => retry_after_bootstrap(model),
            Event::DismissTreasurySheet => {
                model.treasury_bootstrap = None;
                render()
            }
            Event::RetryAfterError => {
                if model.tx != SendTxStatus::Error {
                    return Command::done();
                }
                model.tx = SendTxStatus::Idle;
                model.tx_error = None;
                render()
            }
            Event::ReceiptUpdate {
                user_op_hash,
                outcome,
            } => receipt_update(model, &user_op_hash, outcome),
            Event::Done => fire(model, SendOperation::Close),
            Event::ShellCompleted { attempt, result } => accept(model, attempt, result),
        }
    }

    fn view(&self, model: &Model) -> SendView {
        let locked = model.params.locked;
        let stage = if model.lock_error.is_some() {
            SendStage::LockError
        } else if locked && model.resolving_lock && model.selected_token.is_none() {
            SendStage::LockResolving
        } else if model.tx == SendTxStatus::Confirmed && model.selected_token.is_some() {
            SendStage::Receipt
        } else {
            match model.step {
                SendStep::SelectToken => SendStage::SelectToken,
                SendStep::EnterDetails => SendStage::EnterDetails,
                SendStep::Confirm => SendStage::Confirm,
            }
        };

        let warning = derive_amount_warning(model);
        let issue = derive_same_asset_issue(model);
        let picked = picked_tokens(model);

        // The confirm page's headline amount (`ConfirmStep.tsx:80`). Derived
        // here, not in the shell, so what is shown is by construction the
        // string the submit path turns into base units.
        let token_amount = model
            .selected_token
            .as_ref()
            .map(|token| model_token_amount(model, token))
            .unwrap_or_default();

        // The Continue button gate (`EnterDetailsStep.tsx:372`), plus the one
        // condition it never had: the figure must actually RESOLVE.
        //
        // `!amount.is_empty()` alone let the button light up on an amount that
        // could never become base units — a fiat figure with no rate resolves
        // to "0", so `Continue` was armed on a number the submit path was
        // guaranteed to reject with `InvalidAmount`, over and over, with
        // nothing on screen to explain it. The gate now asks the very string
        // the signature would be built from, which is also the string the ⇅ row
        // prints: button, row and signature cannot disagree.
        // The confirm page's ONE headline figure, in token units. Split mode
        // reads the very sum the gates below read; multiSelect has no headline.
        let confirm_amount = match model.selected_token.as_ref() {
            Some(token) if model.multi_select_mode => {
                let _ = token;
                String::new()
            }
            Some(token) if model.split_mode => {
                sum_split_base_units(&model.recipients, token.decimals)
                    .map(|total| from_base_units(total, token.decimals))
                    .unwrap_or_default()
            }
            Some(_) => token_amount.clone(),
            None => String::new(),
        };

        let amount_resolves = js_parse_float(&token_amount) > 0.0;
        let can_continue = !model.estimating_gas
            && !(locked && warning.is_some())
            && if model.split_mode {
                recipients_are_valid(&model.recipients)
            } else if model.multi_select_mode {
                is_valid_address(&model.recipient) && !picked.is_empty()
            } else {
                !model.recipient.is_empty() && !model.amount.is_empty() && amount_resolves
            };

        let (denom_toggle_shown, denom_toggle_enabled) = denom_toggle(model);
        // A control that declines must also say why. The only way to be shown
        // and refuse is "entering fiat, but nothing prices this token in the
        // currency on screen" — so the sentence names exactly that pair.
        let denom_toggle_reason = (denom_toggle_shown && !denom_toggle_enabled)
            .then(|| unit_issue(model))
            .flatten();

        // The split editor's live over-balance hint (`MultiRecipientEditor.tsx:99-101`),
        // decided with the very helpers the `Continue` gate uses. An unparsable
        // row (where TS `toBaseUnits` throws) is not "over balance" — the row's
        // own invalid-amount state owns that case.
        let split_over_balance = model.split_mode
            && model.selected_token.as_ref().is_some_and(|token| {
                match sum_split_base_units(&model.recipients, token.decimals) {
                    Some(total) => {
                        total > to_base_units(&full_balance(token), token.decimals).unwrap_or(0)
                    }
                    None => false,
                }
            });

        // The confirm slide's gate — and the same amount question `Continue`
        // asks, which this twin never asked.
        //
        // Everything it checked was about the FEE and the pipeline; the money
        // itself was never re-examined after `Continue`. But the confirm page
        // is a page someone can sit on, and a `display_changed` commit landing
        // underneath re-denominates the field to empty
        // (`redenominate_to_display`) — leaving a slider armed over a figure
        // that resolves to nothing. Sliding it signed a zero-value transfer
        // with no warning anywhere. The batch modes carry their money in
        // `recipients`/`multi_specs`, not in `model.amount`, so they are asked
        // the same question `can_continue` asks them.
        let confirm_amount_ok = model.split_mode || model.multi_select_mode || amount_resolves;
        let can_confirm = stage == SendStage::Confirm
            && model.tx == SendTxStatus::Idle
            && !model.estimating_gas
            && !model.fee_busy
            && issue.is_none()
            && confirm_amount_ok;
        // …and the refusal is not allowed to be silent. Only on the page the
        // gate governs: the entry screen already has `amount_warning`.
        let confirm_amount_issue = (stage == SendStage::Confirm && !confirm_amount_ok)
            .then(|| unit_issue(model))
            .flatten();

        let multi_specs = model
            .selected_token
            .as_ref()
            .filter(|_| model.multi_select_mode)
            .map(|token| {
                multi_token_specs(model, token.chain_id)
                    .into_iter()
                    .map(|s| SendMultiSpecView {
                        token_address: s.token_address,
                        decimals: s.decimals,
                        amount: s.amount,
                    })
                    .collect()
            })
            .unwrap_or_default();

        SendView {
            stage,
            loading: model.loading,
            locked,
            amount_locked: view_amount_locked(model),
            lock_error: model.lock_error.clone(),
            resolving_lock: model.resolving_lock,
            adding_network: model.adding_network,
            add_network_msg: model.add_network_msg.clone(),
            tokens: model.tokens.clone(),
            selected_token: model.selected_token.clone(),
            recipient: model.recipient.clone(),
            amount: model.amount.value().to_owned(),
            // Derived, not stored: the view's unit and the figure's unit are
            // the same fact, so they cannot drift apart.
            amount_fiat_code: model.amount.fiat_code().map(str::to_owned),
            denom_toggle_shown,
            denom_toggle_enabled,
            denom_toggle_reason,
            confirm_amount_issue,
            token_amount,
            confirm_amount,
            split_mode: model.split_mode,
            recipients: model.recipients.clone(),
            split_over_balance,
            split_duplicates: if model.split_mode {
                duplicate_recipient_rows(&model.recipients)
            } else {
                Vec::new()
            },
            picker_target: model.picker_target.clone(),
            multi_select_mode: model.multi_select_mode,
            multi_selected_ids: model.multi_selected_ids.clone(),
            multi_valuable_ids: valuable_multi_ids(model),
            multi_chain_id: model.multi_chain_id,
            multi_specs,
            show_scanner: model.show_scanner,
            show_contact_picker: model.show_contact_picker,
            show_batch_import: model.show_batch_import,
            estimating_gas: model.estimating_gas,
            fee_busy: model.fee_busy,
            fee: selected_fee(model).map(fee_to_view),
            gas_fee_token: model.gas_fee_token.clone(),
            amount_warning: warning,
            same_asset_fee_issue: issue,
            can_continue,
            can_confirm,
            sending: model.lock.busy(),
            tx_status: model.tx,
            tx_error: model.tx_error,
            tx_hash: model.tx_hash.clone(),
            user_op_hash: model.user_op_hash.clone(),
            receipt: receipt_view(model, stage),
            treasury_bootstrap: model.treasury_bootstrap.clone(),
            recipient_identity: model.recipient_identity.clone(),
            recipient_risk: model.recipient_risk.clone(),
            sim_json: model.sim_json.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Request plumbing
// ---------------------------------------------------------------------------

/// Mint a fresh per-request id.
fn next(model: &mut Model) -> u64 {
    model.attempt += 1;
    model.attempt
}

/// One tracked request: the answer must quote `id` back.
fn issue(id: u64, operation: SendOperation) -> Cmd {
    Command::request_from_shell(operation).then_send(move |result| Event::ShellCompleted {
        attempt: id,
        result,
    })
}

/// Fire-and-forget (alerts, haptics, cancels) — the ack is dropped on arrival.
fn fire(model: &mut Model, operation: SendOperation) -> Cmd {
    let id = next(model);
    Command::all([issue(id, operation), render()])
}

fn alert(model: &mut Model, kind: SendAlertKind) -> Cmd {
    fire(model, SendOperation::ShowAlert { kind })
}

// ---------------------------------------------------------------------------
// Boot & tokens
// ---------------------------------------------------------------------------

fn has_preselection(params: &SendOpenParams) -> bool {
    params.prefilled_recipient.is_some()
        || params.preselected_multi.is_some()
        || (params.preselected_symbol.is_some() && params.preselected_network.is_some())
}

fn open(
    model: &mut Model,
    account: Option<SendAccountRef>,
    params: SendOpenParams,
    display: SendDisplayContext,
) -> Cmd {
    // A remount: everything resets except the request-id source (so stale
    // results can never collide with fresh flights) and the recipient-row
    // counter (a module global in TS).
    let attempt = model.attempt;
    let recipient_seq = model.recipient_seq;
    *model = Model {
        attempt,
        recipient_seq,
        ..Model::default()
    };
    model.account = account;
    model.display = display;
    model.step = if has_preselection(&params) {
        SendStep::EnterDetails
    } else {
        SendStep::SelectToken
    };
    model.resolving_lock = params.locked;
    // A prefilled recipient IS the recipient from the first frame (spec 028
    // US5): a hand-off from the address book, or a scanned address, must not
    // wait on the token list to show who the money is for — and a fetch that
    // fails must not leave the form open on nobody. `tokens_fetched` still
    // restates it beside the token it picks.
    model.recipient = params.prefilled_recipient.clone().unwrap_or_default();
    model.params = params;
    model.loading = true;
    boot_fetch(model)
}

fn boot_fetch(model: &mut Model) -> Cmd {
    let Some(address) = model.account.as_ref().map(|a| a.address.clone()) else {
        // No wallet: the effect never runs (`if (!address) return`) — the
        // skeleton stays, exactly as today.
        return render();
    };
    model.loading = true;
    let id = next(model);
    model.flights.tokens = Some((id, TokensPurpose::Initial));
    Command::all([issue(id, SendOperation::FetchTokens { address }), render()])
}

/// Non-zero balances, highest USD value first (`useSendController.ts:248-257`).
fn non_zero_sorted(tokens: &[SendToken]) -> Vec<SendToken> {
    let mut list: Vec<SendToken> = tokens
        .iter()
        .filter(|t| t.balance_double() > 0.0)
        .cloned()
        .collect();
    list.sort_by(|a, b| {
        b.usd_value()
            .partial_cmp(&a.usd_value())
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    list
}

fn tokens_partial(model: &mut Model, tokens: Vec<SendToken>) -> Cmd {
    // Progressive display only, and only while a load is actually running.
    if model.flights.tokens.is_none() {
        return Command::done();
    }
    model.tokens = non_zero_sorted(&tokens);
    model.loading = false;
    render()
}

fn refresh_tokens(model: &mut Model) -> Cmd {
    let Some(address) = model.account.as_ref().map(|a| a.address.clone()) else {
        return Command::done();
    };
    let clear_id = next(model);
    let id = next(model);
    model.flights.tokens = Some((id, TokensPurpose::Refresh));
    Command::all([
        issue(
            clear_id,
            SendOperation::ClearTokenCache {
                address: address.clone(),
            },
        ),
        issue(id, SendOperation::FetchTokens { address }),
        render(),
    ])
}

/// `parseInt(x, 10)` on the raw route param — leading integer or `None`.
fn parse_int_prefix(s: &str) -> Option<u32> {
    let t = s.trim();
    let end = t.bytes().take_while(|b| b.is_ascii_digit()).count();
    if end == 0 {
        return None;
    }
    t[..end].parse().ok()
}

fn tokens_loaded(model: &mut Model, tokens: Option<Vec<SendToken>>, purpose: TokensPurpose) -> Cmd {
    let Some(full) = tokens else {
        model.loading = false;
        return match purpose {
            // `catch(() => showAlert(...))`.
            TokensPurpose::Initial => alert(model, SendAlertKind::LoadTokensFailed),
            // `refreshTokens`'s `.catch(() => {})`.
            TokensPurpose::Refresh => render(),
        };
    };
    model.tokens = non_zero_sorted(&full);
    model.loading = false;
    if purpose == TokensPurpose::Refresh {
        return render();
    }

    if model.params.locked {
        return resolve_locked_request(model, &full);
    }

    // Multi-token hand-off via params → land in multiSelect mode.
    if let Some(joined) = model.params.preselected_multi.clone() {
        let wanted: Vec<&str> = joined.split(',').collect();
        let picked: Vec<SendToken> = model
            .tokens
            .iter()
            .filter(|t| wanted.contains(&t.id().as_str()))
            .cloned()
            .collect();
        if let Some(first) = picked.first().cloned() {
            model.multi_selected_ids = picked.iter().map(|t| t.id()).collect();
            model.multi_chain_id = Some(first.chain_id);
            model.multi_select_mode = true;
            model.selected_token = Some(first);
            model.step = SendStep::EnterDetails;
            return warm_estimate_start(model);
        }
        return picker_without_a_token(model);
    }

    if let (Some(symbol), Some(network)) = (
        model.params.preselected_symbol.clone(),
        model.params.preselected_network.clone(),
    ) {
        if let Some(found) = model
            .tokens
            .iter()
            .find(|t| t.symbol == symbol && t.network == network)
            .cloned()
        {
            model.selected_token = Some(found);
            model.step = SendStep::EnterDetails;
            // A token's own 转账 door lands on the form: warm its quote as a
            // picked row would (spec 028 Phase 10).
            return warm_estimate_start(model);
        }
        return picker_without_a_token(model);
    }

    if let Some(prefilled) = model.params.prefilled_recipient.clone() {
        if let Some(first) = model.tokens.first().cloned() {
            // Quick-send from scan: highest-value token, prefilled recipient.
            model.selected_token = Some(first);
            model.recipient = prefilled;
            model.step = SendStep::EnterDetails;
            // The warm quote first; the form's own (payee-aware) quote is
            // armed by its landing, once an amount is typed.
            return Command::all([sync_identity(model), warm_estimate_start(model)]);
        }
    }
    picker_without_a_token(model)
}

/// The hand-off asked for a form and the list could not name a token for it
/// (issue #209).
///
/// `open` steps to `EnterDetails` the moment a hand-off carries a recipient or
/// a token, before the list answers — that optimism is the point: who the
/// money is for must not wait on a fetch. But once the list HAS answered and
/// nothing in it could be selected — an account that holds nothing, a symbol
/// that is not held any more, ids that match no row — the optimism is spent.
/// A form about no token is not a form: it has no balance, no chain and no
/// Max, every shell fills those from the token, and the two that fall back to
/// their drawn card end up showing the mocks' holdings to somebody who owns
/// nothing. The picker is what that person actually needs, and it says "no
/// tokens with balance" out of the very list that came back empty. The
/// recipient stays on the model, so picking a token later lands on the form
/// with the person still filled in.
fn picker_without_a_token(model: &mut Model) -> Cmd {
    if model.selected_token.is_none() {
        model.step = SendStep::SelectToken;
    }
    render()
}

// ---------------------------------------------------------------------------
// EIP-681 locked-request resolution (`useSendController.ts:188-222`)
// ---------------------------------------------------------------------------

fn resolve_locked_request(model: &mut Model, full: &[SendToken]) -> Cmd {
    model.resolving_lock = true;
    let Some(chain_id) = model
        .params
        .prefilled_chain_id
        .as_deref()
        .and_then(parse_int_prefix)
    else {
        // `!Number.isFinite(chainId)` → no error surface, resolution simply
        // ends (ported verbatim).
        model.lock_error = None;
        model.resolving_lock = false;
        return render();
    };
    if !model.chains.iter().any(|c| c.chain_id == chain_id) {
        model.lock_error = Some(SendLockError::Network { chain_id });
        model.resolving_lock = false;
        return render();
    }

    let want_addr = model
        .params
        .prefilled_token_address
        .as_deref()
        .map(str::to_lowercase);
    let found = full
        .iter()
        .find(|tk| {
            tk.chain_id == chain_id
                && match &want_addr {
                    Some(want) => {
                        !tk.is_native()
                            && tk
                                .token_address
                                .as_deref()
                                .is_some_and(|a| a.to_lowercase() == *want)
                    }
                    None => tk.is_native(),
                }
        })
        .cloned();

    match (found, want_addr) {
        (Some(tok), _) => finish_lock_resolution(model, tok),
        (None, None) => {
            let tok = synth_native_token(model, chain_id);
            finish_lock_resolution(model, tok)
        }
        (None, Some(want)) => {
            // Unknown token → resolve on-chain metadata; the amount will be
            // restored with the REAL decimals, not whatever the link claimed
            // (invariant ⑫, `useSendController.ts:205-216`).
            let id = next(model);
            model.flights.lock_meta = Some((id, chain_id, want.clone()));
            Command::all([
                issue(
                    id,
                    SendOperation::ResolveTokenMetadata {
                        chain_id,
                        address: want,
                    },
                ),
                render(),
            ])
        }
    }
}

/// `synthNativeToken` (`send-utils.ts:49-52`) from the shell-supplied registry.
fn synth_native_token(model: &Model, chain_id: u32) -> SendToken {
    let info = model.chains.iter().find(|c| c.chain_id == chain_id);
    let symbol = info.map(|c| c.native_symbol.clone()).unwrap_or_default();
    SendToken {
        network: info.map(|c| c.network.clone()).unwrap_or_default(),
        chain_id,
        symbol,
        balance: "0".to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: None,
        logo_urls: Vec::new(),
        spam: false,
    }
}

fn finish_lock_resolution(model: &mut Model, token: SendToken) -> Cmd {
    model.lock_error = None;
    model.recipient = model.params.prefilled_recipient.clone().unwrap_or_default();
    if let Some(base) = model.params.prefilled_amount_base.as_deref() {
        // `fromBaseUnits(BigInt(base), tok.decimals)` in a try/catch — an
        // unparsable amount is simply skipped.
        if let Ok(units) = base.trim().parse::<u128>() {
            model.amount = DenominatedAmount::token(from_base_units(units, token.decimals));
        }
    }
    model.selected_token = Some(token);
    model.step = SendStep::EnterDetails;
    model.resolving_lock = false;
    Command::all([
        sync_identity(model),
        schedule_form_estimate(model),
        render(),
    ])
}

fn lock_meta_resolved(model: &mut Model, meta: Option<SendTokenMeta>) -> Cmd {
    let Some((_, chain_id, address)) = model.flights.lock_meta.take() else {
        return Command::done();
    };
    match meta {
        None => {
            model.lock_error = Some(SendLockError::Token);
            model.resolving_lock = false;
            render()
        }
        Some(meta) => {
            // `synthErc20Token` — a zero-balance placeholder with resolved
            // symbol/decimals. The original (non-lowercased) param address is
            // used, as today.
            let info = model.chains.iter().find(|c| c.chain_id == chain_id);
            let token = SendToken {
                network: info.map(|c| c.network.clone()).unwrap_or_default(),
                chain_id,
                symbol: meta.symbol,
                balance: "0".to_owned(),
                decimals: meta.decimals,
                token_address: Some(
                    model
                        .params
                        .prefilled_token_address
                        .clone()
                        .unwrap_or(address),
                ),
                price_usd: None,
                logo_urls: Vec::new(),
                spam: false,
            };
            finish_lock_resolution(model, token)
        }
    }
}

fn add_network(model: &mut Model, chain_id: u32) -> Cmd {
    model.adding_network = true;
    model.add_network_msg = None;
    let id = next(model);
    model.flights.add_network = Some(id);
    Command::all([issue(id, SendOperation::AddNetwork { chain_id }), render()])
}

fn network_added(model: &mut Model, outcome: SendAddNetworkOutcome) -> Cmd {
    model.flights.add_network = None;
    model.adding_network = false;
    match outcome {
        SendAddNetworkOutcome::Added => {
            // Re-run resolution now that the chain exists (`lockRetry`).
            model.lock_error = None;
            boot_fetch(model)
        }
        SendAddNetworkOutcome::NotFound => {
            model.add_network_msg = Some(SendAddNetworkMsg::NetNotFound);
            render()
        }
        SendAddNetworkOutcome::NotCompatible { detail } => {
            model.add_network_msg = Some(SendAddNetworkMsg::NetNotCompatible { detail });
            render()
        }
        SendAddNetworkOutcome::Error => {
            model.add_network_msg = Some(SendAddNetworkMsg::NetAddError);
            render()
        }
    }
}

// ---------------------------------------------------------------------------
// Token selection & multiSelect
// ---------------------------------------------------------------------------

fn select_token(model: &mut Model, token_id: &str) -> Cmd {
    let Some(token) = model.tokens.iter().find(|t| t.id() == token_id).cloned() else {
        return Command::done();
    };
    model.multi_select_mode = false; // single-token path
    model.fee_estimate = None; // a prior network's quote must never gate this token
    model.selected_token = Some(token);
    model.step = SendStep::EnterDetails;
    // The credential prefetch (`useSendController.ts:643-648`) AND a warm
    // quote (spec 028 Phase 10): the form's fee row and its Max used to wait
    // for the whole estimate pipeline — deployment read, gas signals, relay
    // quote, in-band rows, simulation — which the founder measured at five to
    // six seconds on the web, and only once the form was complete. The sweep
    // path has warmed a transfer-sized quote since 026; the single-token path
    // now does the same, so the fee is usually in hand before the person has
    // finished typing. Best-effort: a failed warm-up is swallowed, and the
    // form's own debounced quote re-asks with the real payee once it lands.
    warm_estimate_start(model)
}

/// One tap on one row. Deselecting is ALWAYS allowed — a row that somehow got
/// into the pick must never be impossible to take back out — but selecting is
/// scoped by the same `visible_multi_tokens` the master tick uses, so "a batch
/// is one chain" (invariant ⑪) holds in the machine and not only in the
/// picker's `chainFilter != null`. An id this machine does not hold selects
/// nothing, exactly as `toggle_all_multi` already promised.
fn toggle_multi_token(model: &mut Model, token_id: String) -> Cmd {
    if let Some(pos) = model
        .multi_selected_ids
        .iter()
        .position(|id| *id == token_id)
    {
        model.multi_selected_ids.remove(pos);
    } else {
        if !visible_multi_tokens(model)
            .iter()
            .any(|token| token.id() == token_id)
        {
            return Command::done();
        }
        model.multi_selected_ids.push(token_id);
    }
    render()
}

fn visible_multi_tokens(model: &Model) -> Vec<&SendToken> {
    model
        .tokens
        .iter()
        .filter(|t| model.multi_chain_id.is_none_or(|c| t.chain_id == c))
        .collect()
}

/// Every id on the filtered chain this machine would sweep — the projection
/// behind the picker's master checkbox tick. The shell intersects it with the
/// rows it is showing (a display scope); the predicate itself never leaves
/// here.
fn valuable_multi_ids(model: &Model) -> Vec<String> {
    visible_multi_tokens(model)
        .into_iter()
        .filter(|t| t.is_valuable())
        .map(|t| t.id())
        .collect()
}

/// `visible_ids` scopes the sweep to what the picker is showing; the chain
/// filter and [`SendToken::is_valuable`] still decide which of those count.
/// Unknown ids simply do not match a held token, so a stale list can never
/// select something this machine does not hold.
fn toggle_all_multi(model: &mut Model, visible_ids: &[String]) -> Cmd {
    let valuable: Vec<String> = valuable_multi_ids(model)
        .into_iter()
        .filter(|id| visible_ids.contains(id))
        .collect();
    if valuable.is_empty() {
        return Command::done();
    }
    let all_on = valuable
        .iter()
        .all(|id| model.multi_selected_ids.contains(id));
    if all_on {
        model.multi_selected_ids.retain(|id| !valuable.contains(id));
    } else {
        for id in valuable {
            if !model.multi_selected_ids.contains(&id) {
                model.multi_selected_ids.push(id);
            }
        }
    }
    render()
}

fn picked_tokens(model: &Model) -> Vec<SendToken> {
    model
        .tokens
        .iter()
        .filter(|t| model.multi_selected_ids.contains(&t.id()))
        .cloned()
        .collect()
}

fn confirm_multi_selection(model: &mut Model) -> Cmd {
    let picked = picked_tokens(model);
    let Some(first) = picked.first().cloned() else {
        return Command::done();
    };
    if picked.len() == 1 {
        // ONE token is a normal amount-send, not a full-balance multiSelect.
        let id = first.id();
        return select_token(model, &id);
    }
    model.multi_select_mode = true;
    model.selected_token = Some(first);
    model.step = SendStep::EnterDetails;
    warm_estimate_start(model)
}

/// The multiSelect entry warm-up (`useSendController.ts:616-630`): credential,
/// then a best-effort rough estimate so the detail list can show the native
/// line net of its reserve before confirm.
fn warm_estimate_start(model: &mut Model) -> Cmd {
    let Some(account_id) = model.account.as_ref().map(|a| a.id.clone()) else {
        return render();
    };
    let id = next(model);
    model.pipeline = Pipeline::WarmCredential { id };
    Command::all([
        issue(id, SendOperation::LoadAccountCredential { account_id }),
        render(),
    ])
}

// ---------------------------------------------------------------------------
// Amount, fiat toggle, Max
// ---------------------------------------------------------------------------

fn view_amount_locked(model: &Model) -> bool {
    model.params.locked && model.params.prefilled_amount_base.is_some()
}

/// A locked request pins WHO is paid exactly as `amount_locked` pins how much.
///
/// Both locks used to live on the same screen at different depths: the amount's
/// refusal was written here, the recipient's existed only as
/// `editable={!prefilledRecipient}` in `EnterDetailsStep.tsx`. A scanned
/// EIP-681 request names a payee; if the shell ever stops passing that prop the
/// machine would happily re-point the transfer while still calling itself
/// locked, so the rule sits with the machine that builds the call.
///
/// The condition is `locked && prefilled_recipient`, NOT `prefilled_recipient`
/// alone: an unlocked prefill (a contact tapped "Send") legitimately re-sets
/// the recipient — `changeToken` dispatches `Back` (which clears it) and then
/// `SetRecipient` to carry it across. Refusing that would leave the user on an
/// uneditable EMPTY recipient field, which is precisely the kind of gate that
/// stops the wrong thing and the right thing at once.
fn view_recipient_locked(model: &Model) -> bool {
    model.params.locked && model.params.prefilled_recipient.is_some()
}

/// This screen's token price, in the display currency, or `None` when either
/// factor is missing. Never a defaulted 1 — see [`TokenPrice::new`].
fn display_price(model: &Model, token: &SendToken) -> Option<TokenPrice> {
    TokenPrice::new(token.price_usd, model.display.rate, &model.display.code)
}

/// The typed figure resolved into token units — the number every gate, every
/// call builder and the confirm screen read (`resolveTokenAmount`'s job, now
/// asked of the figure itself so the unit cannot be lost on the way).
fn model_token_amount(model: &Model, token: &SendToken) -> String {
    model
        .amount
        .to_token_units(display_price(model, token).as_ref(), token.decimals)
}

/// Whether the ⇄ row is offered, and whether pressing it would do anything.
///
/// One function so the control's appearance and its behaviour are decided by
/// the same sentence. They were decided in two places before: `send.rs` refused
/// to enter fiat without a price while `EnterDetailsStep` rendered the row on
/// `priceUsd > 0` alone, so the control looked live and swallowed the tap.
fn denom_toggle(model: &Model) -> (bool, bool) {
    let Some(token) = model.selected_token.as_ref() else {
        return (false, false);
    };
    // The ported render condition (`EnterDetailsStep.tsx:170`).
    let priced = token.price_usd.is_some_and(|p| p > 0.0);
    let in_fiat = model.amount.is_fiat();
    // A door in must have a door out: while the figure is fiat the row is
    // shown even for an unpriced token, because leaving is the only escape
    // from a mode whose amount can no longer resolve.
    let shown = priced || in_fiat;
    // Leaving is always allowed; entering needs a price in the display
    // currency — the same condition `toggle_fiat_input` refuses on, asked here
    // so the refusal is visible instead of silent.
    let enabled = in_fiat || display_price(model, token).is_some();
    (shown, enabled)
}

/// The currency/token pair every refusal sentence on this screen names.
///
/// The currency is the FIGURE's when it has one and the display currency
/// otherwise — the same rule [`redenominate_to_display`] keeps true, so the
/// two only ever differ inside the event that is fixing them. Never invents a
/// currency: no token, no sentence.
fn unit_issue(model: &Model) -> Option<SendUnitIssue> {
    let token = model.selected_token.as_ref()?;
    Some(SendUnitIssue {
        code: model
            .amount
            .fiat_code()
            .unwrap_or(&model.display.code)
            .to_owned(),
        symbol: token.symbol.clone(),
    })
}

/// Keep the typed figure's currency and the display currency the same currency.
///
/// A commit can land under a screen that already has a figure on it (the shell
/// boots on a placeholder `USD` pair and replaces it once AsyncStorage and the
/// FX/Chainlink round trip answer). The figure keeps its own code, which is
/// what stops it being *relabelled* — but left alone it also becomes
/// permanently unresolvable: `to_token_units` refuses a price quoted in another
/// currency, so the amount reads `"0"` for ever, and `with_value` preserves the
/// stale unit, so **retyping cannot fix it**. That was the trap: `Continue`
/// disabled (or worse, armed) on every figure the user could possibly enter.
///
/// The figure cannot come across — a CNY↔USD cross rate is not something this
/// screen has, and inventing one is the defect this whole area exists to
/// forbid. So the FIGURE is dropped and the CURRENCY is adopted: the field is
/// re-denominated in the currency now on screen, empty, ready to be typed in.
/// Empty is the one state that claims nothing (it crosses units with no factor
/// at all), and it is the same answer `toggle_fiat_input` gives when leaving an
/// unconvertible fiat mode.
///
/// What it does NOT touch is the MODE. Whether money is typed in tokens or in
/// currency is the user's choice, made through ⇄ and unmade only there; a
/// display-currency commit landing in the background is not a reason to move
/// someone out of the mode they picked. So even an unpriceable new currency
/// keeps them in fiat — with an empty field, a stated reason
/// ([`SendAmountWarning::CannotConvert`]) once they type, and the ⇄ row shown
/// and enabled so the way out is one tap away.
///
/// The invariant this establishes, and which the rest of the file may rely on:
/// **`model.amount`'s fiat code, when there is one, is `model.display.code`.**
/// A figure and a rate on this screen are never about different currencies for
/// longer than the event that made them so.
fn redenominate_to_display(model: &mut Model) {
    let Some(code) = model.amount.fiat_code() else {
        return; // token units are not denominated in anyone's currency
    };
    if code == model.display.code {
        return; // same currency: the figure stands, rate change or not
    }
    model.amount = DenominatedAmount::fiat("", &model.display.code);
}

/// The ⇄ toggle (`EnterDetailsStep.tsx:165-176`).
///
/// The whole operation is one [`DenominatedAmount::convert`]. It cannot be
/// written any other way from here: `model.amount`'s unit is private to
/// `money`, so "flip the label, keep the digits" — the defect this replaces —
/// is not expressible. What is left is deciding what an *unconvertible* figure
/// should become, and there are only honest options:
///
/// - **Entering** fiat mode is what commits someone to typing money in a
///   currency the app must divide by. With no price for that currency there is
///   nothing to divide by, so the door stays shut and the typed token amount is
///   left exactly alone.
/// - **Leaving** is always allowed, because a currency can go unpriceable while
///   a fiat figure is already typed and trapping someone in a mode whose amount
///   can never resolve is its own bug. But the figure does NOT come with them:
///   5000 CNY is not 5000 USDC, and there is no rate to say what it is, so the
///   field is emptied. An empty field is the one state that claims nothing —
///   `can_continue` already refuses it, and the ⇅ row already reads `0 SYM`.
///
/// A blank or zero figure converts freely in both directions with no rate at
/// all (zero is zero in every unit), which is why an untouched screen can still
/// flip modes when the currency is unpriceable... except into fiat, where there
/// would be nothing to type against.
fn toggle_fiat_input(model: &mut Model) -> Cmd {
    let Some(token) = model.selected_token.clone() else {
        return Command::done();
    };
    let price = display_price(model, &token);
    let target = if model.amount.is_fiat() {
        Denom::Token
    } else {
        model.display.denom()
    };
    if target.is_fiat() && price.is_none() {
        return Command::done(); // the door into fiat stays shut
    }
    model.amount = model
        .amount
        .convert(
            &target,
            price.as_ref(),
            token.decimals,
            model.display.fiat_decimals,
        )
        // Unconvertible on the way OUT of fiat: leave the mode, drop the
        // figure. Never carry the digits across the unit boundary.
        .unwrap_or_else(|_| DenominatedAmount::token(""));
    render()
}

fn tap_max(model: &mut Model) -> Cmd {
    let Some(token) = model.selected_token.clone() else {
        return Command::done();
    };
    // Max always fills in token units. Every exit below writes a token
    // figure; the one that waits for an estimate leaves the field blank
    // meanwhile rather than re-labelling whatever was typed before.
    model.amount = DenominatedAmount::token("");

    if let Some(fee) = selected_fee(model).cloned() {
        apply_max_with_fee(model, &token, &fee);
        return render();
    }

    // No usable quote yet → estimate on demand (rough shape), exactly like
    // `handleMaxAmount`'s `await estimateTransactionFee(...)`.
    let needs_estimate =
        model.account.is_some() && (token.is_native() || token.token_address.is_some());
    if !needs_estimate {
        model.amount = DenominatedAmount::token(full_balance(&token));
        return render();
    }
    let account = match model.account.as_ref() {
        Some(a) => a.address.clone(),
        None => {
            model.amount = DenominatedAmount::token(full_balance(&token));
            return render();
        }
    };
    let id = next(model);
    model.pipeline = Pipeline::MaxEstimate { id };
    Command::all([
        issue(
            id,
            SendOperation::EstimateFee {
                chain_id: token.chain_id,
                account,
                tx: None,
                batch: None,
                gas_fee_token: model.gas_fee_token.clone(),
                public_key_hex: model.public_key_hex.clone(),
            },
        ),
        render(),
    ])
}

// ---------------------------------------------------------------------------
// The form's own quote (spec 028 Phase 9, T490)
// ---------------------------------------------------------------------------
//
// Until this phase the machine asked for a fee only on Continue (the
// pre-check) and for a sweep's warm-up, so the form's fee row read "—" and
// the fee-coin picker had nothing to list until the person had already
// committed. Expo estimated on the form as it was typed. This is that rule,
// in the machine: a complete form asks once it has sat still, the answer is
// the same chain-guarded `fee_estimate` every surface reads, and Continue's
// pre-check still re-quotes the exact amount and refuses on failure — this
// quote is best-effort and never a gate.

/// What the form's quote is about: the token, the payee and the fee coin. The
/// amount is deliberately not part of it — a transfer's gas does not move with
/// the figure, and the pre-check re-quotes the exact amount anyway. `None`
/// when the form is not complete, or is not the single-transfer form.
fn form_estimate_key(model: &Model) -> Option<String> {
    if model.step != SendStep::EnterDetails || model.multi_select_mode || model.split_mode {
        return None;
    }
    let token = model.selected_token.as_ref()?;
    model.account.as_ref()?;
    if !is_valid_address(&model.recipient) {
        return None;
    }
    let amount = js_parse_float(&model_token_amount(model, token));
    if amount.is_nan() || amount <= 0.0 {
        return None;
    }
    Some(format!(
        "{}|{}|{}",
        token.id(),
        model.recipient.trim().to_lowercase(),
        model.gas_fee_token.as_deref().unwrap_or("")
    ))
}

/// Arm — or re-arm — the debounce, when the form is complete, nothing heavier
/// is in flight, and the quote in hand (if any) is not already about this.
fn schedule_form_estimate(model: &mut Model) -> Cmd {
    let Some(key) = form_estimate_key(model) else {
        return Command::done();
    };
    if model.form_quote_key.as_deref() == Some(key.as_str()) && selected_fee(model).is_some() {
        return Command::done();
    }
    match model.pipeline {
        Pipeline::Idle | Pipeline::FormDebounce { .. } => {}
        // A pre-check, a Max, a warm-up or a submit owns the slot: they all
        // produce a quote of their own, or leave the form.
        _ => return Command::done(),
    }
    let timer_id = next(model);
    model.pipeline = Pipeline::FormDebounce { timer_id };
    issue(
        timer_id,
        SendOperation::StartTimer {
            ms: FORM_ESTIMATE_DEBOUNCE_MS,
            tag: SendTimerTag::FormEstimate,
        },
    )
}

/// The debounce elapsed on a form that is still complete: ask.
fn form_estimate_fire(model: &mut Model) -> Cmd {
    let (Some(key), Some(token), Some(account)) = (
        form_estimate_key(model),
        model.selected_token.clone(),
        model.account.clone(),
    ) else {
        model.pipeline = Pipeline::Idle;
        return Command::done();
    };
    let (tx, batch) = build_estimate_shape(model, &token);
    let id = next(model);
    model.pipeline = Pipeline::FormEstimate { id, key };
    issue(
        id,
        SendOperation::EstimateFee {
            chain_id: token.chain_id,
            account: account.address,
            tx,
            batch,
            gas_fee_token: model.gas_fee_token.clone(),
            public_key_hex: model.public_key_hex.clone(),
        },
    )
}

fn full_balance(token: &SendToken) -> String {
    if token.balance.is_empty() {
        "0".to_owned()
    } else {
        token.balance.clone()
    }
}

/// What `Max` holds back, when the fee is drawn from the very asset being
/// sent. `None` — gas is paid in a separate asset, so the whole balance is
/// sendable.
///
/// One rule, two readers: the fill ([`apply_max_with_fee`]) and the sentence
/// that explains a fill of nothing ([`fee_over_balance`]). Written twice they
/// would eventually disagree, and the shape of that disagreement is a screen
/// showing `0` while insisting the balance covers the fee.
fn max_fee_reserve(token: &SendToken, fee: &FeeEstimate) -> Option<u128> {
    if token.is_native() {
        return Some(fee.total_wei);
    }
    let addr = token.token_address.as_deref()?;
    match &fee.fee_asset {
        FeeAsset::Erc20 {
            token: fee_token,
            amount,
            ..
        } if fee_token.eq_ignore_ascii_case(addr) => {
            // Reserve 1.5× the quoted fee (+50% for send-time re-quote drift).
            Some(amount.saturating_mul(3) / 2)
        }
        _ => None,
    }
}

/// The Max fill given a fee (`useSendController.ts:801-848`).
fn apply_max_with_fee(model: &mut Model, token: &SendToken, fee: &FeeEstimate) {
    // Gas paid in native or a separate fee asset — full balance sendable.
    let Some(reserve) = max_fee_reserve(token, fee) else {
        model.amount = DenominatedAmount::token(full_balance(token));
        return;
    };
    match to_base_units(&token.balance, token.decimals) {
        // String-exact `balance − reserve`: `to_base_units(result) + reserve
        // == balance`, so the gas pre-check never trips on its own Max fill
        // (invariant ⑨). A reserve at or above the balance answers `"0"`, and
        // `derive_amount_warning` is what says so out loud.
        Some(balance) => {
            model.amount =
                DenominatedAmount::token(max_native_sendable(balance, reserve, token.decimals));
        }
        // TS `balanceToWei` would throw → catch → full balance.
        None => model.amount = DenominatedAmount::token(full_balance(token)),
    }
}

// ---------------------------------------------------------------------------
// Split mode (`useSendController.ts:503-549`)
// ---------------------------------------------------------------------------

fn make_recipient_id(model: &mut Model) -> String {
    model.recipient_seq += 1;
    format!("rcpt_{}", model.recipient_seq)
}

/// A locked request is one payment to one payee: it may not become a split or
/// a batch. The entry points (`Add recipient`, `Import list`) are already
/// absent from the screen while locked — this is the same sentence said where
/// the calls are built, so the mode cannot be entered by any other door.
fn split_locked_out(model: &Model) -> bool {
    model.params.locked
}

fn enter_split_mode(model: &mut Model) -> Cmd {
    if split_locked_out(model) {
        return Command::done();
    }
    let Some(token) = model.selected_token.clone() else {
        return Command::done();
    };
    let token_amt = model_token_amount(model, &token);
    let row_amount = if model.amount.is_empty() {
        String::new()
    } else {
        token_amt
    };
    let first = SendRecipientDraft {
        id: make_recipient_id(model),
        address: model.recipient.clone(),
        amount: row_amount.clone(),
        name: None,
    };
    let empty = SendRecipientDraft {
        id: make_recipient_id(model),
        address: String::new(),
        amount: String::new(),
        name: None,
    };
    model.recipients = vec![first, empty];
    // Split rows are token-denominated, so the single-send figure follows them
    // into token units — RESTATED through the same resolution the first row
    // got, not merely re-labelled.
    model.amount = DenominatedAmount::token(row_amount);
    model.split_mode = true;
    render()
}

fn assign_ids(model: &mut Model, mut rows: Vec<SendRecipientDraft>) -> Vec<SendRecipientDraft> {
    for row in &mut rows {
        if row.id.is_empty() {
            row.id = make_recipient_id(model);
        }
    }
    rows
}

fn seed_split(model: &mut Model, rows: Vec<SendRecipientDraft>) -> Cmd {
    if rows.is_empty() || split_locked_out(model) {
        return Command::done();
    }
    let mut rows = assign_ids(model, rows);
    rows.truncate(BATCH_MAX_RECIPIENTS); // the importer's trim (invariant ⑩)
                                         // The imported rows replace the single-send figure outright; there is
                                         // nothing left to restate, so the field goes empty in token units.
    model.amount = DenominatedAmount::token("");
    model.recipients = rows;
    model.split_mode = true;
    model.show_batch_import = false;
    model.show_contact_picker = false;
    render()
}

fn recipients_changed(model: &mut Model, rows: Vec<SendRecipientDraft>) -> Cmd {
    if rows.len() <= 1 {
        // Collapse back to single mode, carrying the remaining row.
        model.recipient = rows.first().map(|r| r.address.clone()).unwrap_or_default();
        // A split row's amount is token-denominated by construction.
        model.amount =
            DenominatedAmount::token(rows.first().map(|r| r.amount.clone()).unwrap_or_default());
        model.split_mode = false;
        model.recipients.clear();
        return Command::all([
            sync_identity(model),
            schedule_form_estimate(model),
            render(),
        ]);
    }
    let mut rows = assign_ids(model, rows);
    rows.truncate(BATCH_MAX_RECIPIENTS);
    model.recipients = rows;
    render()
}

/// A pick from the book lands in its row and closes the picker — the same
/// way `seed_split` does for a whole group. Leaving the sheet up after the
/// person chose would make the shell dispatch a second event to say what the
/// first one already meant (spec 028 US5).
fn apply_picked_address(model: &mut Model, address: String) -> Cmd {
    model.show_contact_picker = false;
    match model.picker_target.clone() {
        Some(target) => {
            for row in &mut model.recipients {
                if row.id == target {
                    row.address = address.clone();
                }
            }
            render()
        }
        None => {
            model.recipient = address;
            Command::all([
                sync_identity(model),
                schedule_form_estimate(model),
                render(),
            ])
        }
    }
}

fn scan_resolved(model: &mut Model, scan: SendScan) -> Cmd {
    model.show_scanner = false;
    // Per-row scan in split mode — just the address; a full-request re-lock
    // would blow away the other recipients (invariant ⑬).
    if model.picker_target.is_some() {
        let address = match scan {
            SendScan::Request { recipient, .. } => recipient,
            SendScan::Text { data } => data,
        };
        return apply_picked_address(model, address);
    }
    match scan {
        SendScan::Request {
            recipient,
            chain_id: Some(chain_id),
            token_address,
            amount_base_units,
        } => {
            // A full EIP-681 request re-opens Send locked (`router.replace`).
            let account = model.account.clone();
            let display = model.display.clone();
            let params = SendOpenParams {
                prefilled_recipient: Some(recipient),
                prefilled_chain_id: Some(chain_id.to_string()),
                prefilled_token_address: token_address,
                prefilled_amount_base: amount_base_units,
                locked: true,
                ..SendOpenParams::default()
            };
            open(model, account, params, display)
        }
        SendScan::Request { recipient, .. } => {
            model.recipient = recipient;
            Command::all([
                sync_identity(model),
                schedule_form_estimate(model),
                render(),
            ])
        }
        SendScan::Text { data } => {
            model.recipient = data;
            Command::all([
                sync_identity(model),
                schedule_form_estimate(model),
                render(),
            ])
        }
    }
}

// ---------------------------------------------------------------------------
// Recipient identity / confirm-step probes
// ---------------------------------------------------------------------------

/// The `[recipient]` effect (`useSendController.ts:401-410`): clear, then
/// resolve when the address is well-formed. Risk is confirm-scoped and clears
/// with any recipient change.
fn sync_identity(model: &mut Model) -> Cmd {
    model.recipient_identity = None;
    model.recipient_risk = None;
    model.flights.risk = None;
    if !is_valid_address(&model.recipient) {
        model.flights.identity = None;
        return Command::done();
    }
    let id = next(model);
    model.flights.identity = Some(id);
    issue(
        id,
        SendOperation::ResolveIdentity {
            address: model.recipient.clone(),
        },
    )
}

/// The confirm-step sim + risk effects (`useSendController.ts:415-464`) —
/// best-effort; failures leave the surfaces empty.
fn confirm_probes(model: &mut Model) -> Cmd {
    model.sim_json = None;
    model.flights.sim = None;
    model.recipient_risk = None;
    model.flights.risk = None;
    if model.step != SendStep::Confirm {
        return Command::done();
    }
    let (Some(token), Some(account)) = (model.selected_token.clone(), model.account.clone()) else {
        return Command::done();
    };

    let mut cmds: Vec<Cmd> = Vec::new();

    if is_valid_address(&model.recipient) {
        let id = next(model);
        model.flights.risk = Some(id);
        cmds.push(issue(
            id,
            SendOperation::ResolveRisk {
                chain_id: token.chain_id,
                address: model.recipient.clone(),
            },
        ));
    }

    let ok_single =
        !model.split_mode && !model.multi_select_mode && is_valid_address(&model.recipient);
    let ok_split = model.split_mode && recipients_are_valid(&model.recipients);
    let ok_multi = model.multi_select_mode
        && is_valid_address(&model.recipient)
        && !picked_tokens(model).is_empty();
    if ok_single || ok_split || ok_multi {
        if let Some(calls) = build_sim_calls(model, &token) {
            let id = next(model);
            model.flights.sim = Some(id);
            cmds.push(issue(
                id,
                SendOperation::SimulateCalls {
                    chain_id: token.chain_id,
                    account: account.address,
                    calls,
                },
            ));
        }
        // A malformed amount → no sim (the `catch`).
    }
    Command::all(cmds)
}

/// The sim's call batch (`useSendController.ts:429-443`): multiSelect uses the
/// RESERVED specs; split and single mirror their submit shapes.
fn build_sim_calls(model: &Model, token: &SendToken) -> Option<Vec<FeeCall>> {
    if model.multi_select_mode {
        build_multi_token_calls(
            model.recipient.trim(),
            &multi_token_specs(model, token.chain_id),
        )
    } else if model.split_mode {
        build_split_calls(
            token.token_address.as_deref(),
            token.decimals,
            &model.recipients,
        )
    } else {
        let amount = model_token_amount(model, token);
        let units = to_base_units(&amount, token.decimals)?;
        Some(vec![match token.token_address.as_deref() {
            None => FeeCall {
                to: model.recipient.clone(),
                value: units.to_string(),
                data: "0x".to_owned(),
            },
            Some(addr) => FeeCall {
                to: addr.to_owned(),
                value: "0".to_owned(),
                data: encode_erc20_transfer(&model.recipient, units)?,
            },
        }])
    }
}

// ---------------------------------------------------------------------------
// Derived money rules
// ---------------------------------------------------------------------------

/// The chain-guarded estimate (`selectedFeeEstimate`) — invariant ①'s display
/// half: a quote is valid only for the network it was calculated on.
fn selected_fee(model: &Model) -> Option<&FeeEstimate> {
    let token = model.selected_token.as_ref()?;
    let fee = model.fee_estimate.as_ref()?;
    (fee.chain_id == token.chain_id).then_some(fee)
}

/// The chain's own coin, when the registry knows it — the symbol a `None` in
/// [`SendAmountWarning`] leaves the shell to resolve.
fn chain_native_symbol(model: &Model, chain_id: u32) -> Option<String> {
    model
        .chains
        .iter()
        .find(|c| c.chain_id == chain_id)
        .map(|c| c.native_symbol.clone())
}

/// Nothing is sendable: the fee is drawn from the asset being sent, and the
/// reserve `Max` holds back for it meets or exceeds the whole balance.
///
/// Reads the same [`max_fee_reserve`] the fill reads, so this sentence appears
/// exactly when `Max` resolves to `"0"` — never on a balance that could still
/// pay, and never absent on one that could not.
fn fee_over_balance(model: &Model, token: &SendToken) -> Option<SendAmountWarning> {
    let fee = selected_fee(model)?;
    let reserve = max_fee_reserve(token, fee)?;
    // A quote of nothing (sponsored, or not yet priced) reserves nothing: an
    // empty balance is then a balance, not a fee that ate it.
    if reserve == 0 {
        return None;
    }
    let balance = to_base_units(&token.balance, token.decimals)?;
    if balance > reserve {
        return None;
    }
    let symbol = if token.is_native() {
        chain_native_symbol(model, token.chain_id)
    } else {
        match &fee.fee_asset {
            FeeAsset::Erc20 { symbol, .. } => symbol.clone().or_else(|| Some(token.symbol.clone())),
            FeeAsset::Native => chain_native_symbol(model, token.chain_id),
        }
    };
    Some(SendAmountWarning::InsufficientGas { symbol })
}

/// The live amount warning (`useSendController.ts:326-398`), as a pure
/// derivation instead of a `useEffect` + `useState` pair.
#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN is an unresolved amount, not a valid one
fn derive_amount_warning(model: &Model) -> Option<SendAmountWarning> {
    let token = model.selected_token.as_ref()?;
    if model.amount.is_empty() {
        return None;
    }
    let token_amount = model_token_amount(model, token);
    let amount_num = js_parse_float(if token_amount.is_empty() {
        "0"
    } else {
        &token_amount
    });
    if !(amount_num > 0.0) {
        // Typed digits that resolve to nothing are not "no amount" — they are
        // an amount whose FACTOR is missing (no rate for the display currency,
        // or no price for the token). `Continue` refuses it either way; this is
        // the sentence that says so, and it names the way out (the ⇄ row, which
        // `denom_toggle` keeps reachable for exactly this reason).
        if model.amount.as_f64() > 0.0 {
            if let Some(code) = model.amount.fiat_code() {
                return Some(SendAmountWarning::CannotConvert {
                    code: code.to_owned(),
                    symbol: token.symbol.clone(),
                });
            }
        }
        // A figure that IS zero — which is what `Max` writes when the fee has
        // already claimed the whole balance. The zero is right; saying nothing
        // about it is not (issue #210).
        return fee_over_balance(model, token);
    }
    let fee = selected_fee(model);

    if token.is_native() {
        // Unparsable strings read as 0 — where TS `BigInt` would throw inside
        // the effect, the machine degrades to "no units" (fail-closed: an
        // over-large amount still warns).
        let balance_wei = to_base_units(&token.balance, token.decimals).unwrap_or(0);
        let amount_wei = to_base_units(&token_amount, token.decimals).unwrap_or(0);
        if amount_wei > balance_wei {
            return Some(SendAmountWarning::NotEnoughToken {
                symbol: token.symbol.clone(),
            });
        }
        if let Some(fee) = fee {
            // totalWei is the fully marked-up, reviewed in-band reimbursement.
            if !can_cover_native_transfer(amount_wei, balance_wei, fee.total_wei) {
                return Some(SendAmountWarning::InsufficientForGas {
                    symbol: chain_native_symbol(model, token.chain_id),
                });
            }
        }
        // …and when the fee rides on a DIFFERENT asset, that asset's balance
        // is what has to cover it. This branch measured only the coin being
        // sent, so switching the fee to a stablecoin the account barely holds
        // left a native send with nothing said (issue #211's other half).
        return fee_asset_shortfall(model, token.chain_id, &[None]);
    }

    // ERC-20: token balance first…
    if amount_num > token.balance_double() {
        return Some(SendAmountWarning::NotEnoughToken {
            symbol: token.symbol.clone(),
        });
    }
    // …then the fee asset, exactly like any other token (no Tempo special
    // case): sending the fee asset reserves its fee; otherwise the separate
    // fee-token balance must cover it.
    if let Some(fee) = fee {
        // Sending the fee asset itself: the amount and the fee come out of one
        // balance, so the ceiling is their sum.
        if let FeeAsset::Erc20 {
            token: fee_token,
            amount: fee_amount,
            symbol: fee_symbol,
            ..
        } = &fee.fee_asset
        {
            let is_fee_token = token
                .token_address
                .as_deref()
                .is_some_and(|a| a.eq_ignore_ascii_case(fee_token));
            if is_fee_token {
                let balance_units = to_base_units(&token.balance, token.decimals).unwrap_or(0);
                let send_units = to_base_units(&token_amount, token.decimals).unwrap_or(0);
                if send_units.saturating_add(*fee_amount) > balance_units {
                    return Some(SendAmountWarning::InsufficientForGas {
                        symbol: Some(fee_symbol.clone().unwrap_or_else(|| token.symbol.clone())),
                    });
                }
                return None;
            }
        }
    }
    // Any other fee asset — a second token, or the native coin, whose own
    // balance is what must cover it. The native half was missing entirely
    // (issue #211): the fee rode on POL, the account held none, nothing
    // measured it, and a send that could only revert reached the passkey.
    fee_asset_shortfall(model, token.chain_id, &[token.token_address.as_deref()])
}

/// The `Continue` / slide reading of [`fee_asset_shortfall`]: the fee coin has
/// to be there, whatever the mode. A sweep is the exception it names — its fee
/// asset is reserved out of the very line that would pay it
/// (`reserve_native_gas` / `reserve_fee_token`), so a picked fee asset answers
/// to that reserve, not to this.
fn fee_asset_gate(model: &Model) -> Option<SendAmountWarning> {
    let chain_id = fee_chain_id(model)?;
    let picked;
    let sent: Vec<Option<&str>> = if model.multi_select_mode {
        picked = picked_tokens(model);
        picked
            .iter()
            .map(|tk| tk.token_address.as_deref())
            .collect()
    } else {
        model
            .selected_token
            .as_ref()
            .map(|token| vec![token.token_address.as_deref()])
            .unwrap_or_default()
    };
    fee_asset_shortfall(model, chain_id, &sent)
}

/// The chain this form's fee belongs to: the sweep's own network, or the
/// selected token's (invariant ① — a quote is valid only where it was made).
fn fee_chain_id(model: &Model) -> Option<u32> {
    if model.multi_select_mode {
        model.multi_chain_id
    } else {
        model.selected_token.as_ref().map(|token| token.chain_id)
    }
}

/// The coin that pays this fee, measured against what the account holds of it
/// (issue #211). `sent` is every asset this operation moves (`None` = the
/// native coin); when the fee rides on one of THOSE this answers `None`,
/// because the ceiling is then amount + fee against one balance —
/// `can_cover_native_transfer`, `same_asset_fee_limit` and the sweep's own
/// reserve own that, not this.
///
/// A row the account does not hold is not in `tokens` at all — zero balances
/// are filtered out of the holdings before they reach the machine — so an
/// absent row reads as zero, fail-closed.
fn fee_asset_shortfall(
    model: &Model,
    chain_id: u32,
    sent: &[Option<&str>],
) -> Option<SendAmountWarning> {
    let fee = model
        .fee_estimate
        .as_ref()
        .filter(|fee| fee.chain_id == chain_id)?;
    let (contract, owed, quoted_symbol) = match &fee.fee_asset {
        FeeAsset::Native => (None, fee.total_wei, None),
        FeeAsset::Erc20 {
            token,
            amount,
            symbol,
            ..
        } => (Some(token.as_str()), *amount, symbol.clone()),
    };
    // The fee comes out of an asset this operation is already moving: the
    // ceiling is then amount + fee against one balance, which is the caller's
    // own rule, not this one.
    if sent.iter().any(|asset| match (contract, asset) {
        (None, None) => true,
        (Some(fee_token), Some(addr)) => fee_token.eq_ignore_ascii_case(addr),
        _ => false,
    }) {
        return None;
    }
    let row = model.tokens.iter().find(|tk| {
        tk.chain_id == chain_id
            && match (contract, tk.token_address.as_deref()) {
                (None, None) => true,
                (Some(fee_token), Some(addr)) => fee_token.eq_ignore_ascii_case(addr),
                _ => false,
            }
    });
    let balance = row
        .and_then(|tk| to_base_units(&tk.balance, tk.decimals))
        .unwrap_or(0);
    if balance >= owed {
        return None;
    }
    Some(SendAmountWarning::NeedGas {
        symbol: quoted_symbol
            .or_else(|| row.map(|tk| tk.symbol.clone()))
            .or_else(|| {
                contract.is_none().then(|| {
                    model
                        .chains
                        .iter()
                        .find(|c| c.chain_id == chain_id)
                        .map(|c| c.native_symbol.clone())
                })?
            }),
    })
}

/// `sameAssetFeeIssue` (`useSendController.ts:574-602`): the fee learned at
/// confirm can make a previously valid same-token amount unpayable — surface
/// the exact ceiling instead of letting a doomed batch reach the passkey
/// (invariant ⑧). Any parse failure answers `None` (the TS `catch`: input
/// validation owns malformed amounts; never a false financial warning).
fn derive_same_asset_issue(model: &Model) -> Option<SendFeeIssueView> {
    let token = model.selected_token.as_ref()?;
    if model.multi_select_mode {
        return None;
    }
    let fee = selected_fee(model)?;
    let transfer_amount = if model.split_mode {
        sum_split_base_units(&model.recipients, token.decimals)?
    } else {
        to_base_units(&model_token_amount(model, token), token.decimals)?
    };
    let balance = to_base_units(&token.balance, token.decimals)?;
    let limit = same_asset_fee_limit(Some(fee), token.token_address.as_deref(), balance)?;
    if transfer_amount <= limit.max_transfer_amount {
        return None;
    }
    Some(SendFeeIssueView {
        symbol: token.symbol.clone(),
        transfer_amount: transfer_amount.to_string(),
        balance: balance.to_string(),
        fee_amount: limit.fee_amount.to_string(),
        total: transfer_amount.saturating_add(limit.fee_amount).to_string(),
        max_transfer_amount: limit.max_transfer_amount.to_string(),
    })
}

/// `multiTokenSpecs` (`useSendController.ts:559-568`): the EXACT per-token
/// amounts a multiSelect submits — reserve whichever asset pays the displayed
/// fee so preview and signed MultiSend stay identical (invariant ⑪).
fn multi_token_specs(model: &Model, chain_id: u32) -> Vec<MultiTokenSpec> {
    let specs: Vec<MultiTokenSpec> = picked_tokens(model)
        .iter()
        .map(|tk| MultiTokenSpec {
            token_address: tk.token_address.clone(),
            decimals: tk.decimals,
            amount: full_balance(tk),
        })
        .collect();
    let fee = model
        .fee_estimate
        .as_ref()
        .filter(|f| f.chain_id == chain_id);
    match fee.map(|f| &f.fee_asset) {
        Some(FeeAsset::Erc20 { token, amount, .. }) => {
            // 2×: a sweep has more sub-calls than its initial quote and may
            // also deploy the Safe; the signed fee still uses the reviewed
            // quote.
            reserve_fee_token(&specs, token, amount.saturating_mul(2))
        }
        _ => reserve_native_gas(&specs, fee.map(|f| f.total_wei).unwrap_or(0)),
    }
}

// ---------------------------------------------------------------------------
// Continue → pre-check → confirm (`useSendController.ts:651-791`)
// ---------------------------------------------------------------------------

fn handle_continue(model: &mut Model) -> Cmd {
    if model.multi_select_mode {
        if !is_valid_address(&model.recipient) {
            return alert(model, SendAlertKind::InvalidAddress);
        }
        if picked_tokens(model).is_empty() {
            return Command::done();
        }
    } else if model.split_mode {
        if !recipients_are_valid(&model.recipients) {
            return alert(model, SendAlertKind::InvalidAddress);
        }
        if let Some(token) = model.selected_token.clone() {
            let Some(total) = sum_split_base_units(&model.recipients, token.decimals) else {
                // TS `toBaseUnits` would throw out of the handler — no state
                // change, no alert (ported verbatim).
                return Command::done();
            };
            let balance = to_base_units(&full_balance(&token), token.decimals).unwrap_or(0);
            if total > balance {
                return alert(model, SendAlertKind::SplitOverBalance);
            }
        }
    } else {
        if !is_valid_address(&model.recipient) {
            return alert(model, SendAlertKind::InvalidAddress);
        }
        let token = model.selected_token.clone();
        let amount_num = token
            .as_ref()
            .map(|tk| js_parse_float(&model_token_amount(model, tk)));
        if let Some(n) = amount_num {
            if n.is_nan() || n <= 0.0 {
                return alert(model, SendAlertKind::InvalidAmount);
            }
        }
        if let Some(warning) = derive_amount_warning(model) {
            return alert(
                model,
                SendAlertKind::InsufficientBalance {
                    warning: Some(warning),
                },
            );
        }
    }

    // Whatever the mode, the coin that pays the fee has to be there (issue
    // #211). The single send asks this through `derive_amount_warning` above;
    // a split asks nothing of the fee at all, and a sweep of ERC-20s keeps
    // building its lines after `reserve_native_gas` has dropped a native one
    // it could not reserve from. All three reach the same refusal here.
    if let Some(warning) = fee_asset_gate(model) {
        return alert(
            model,
            SendAlertKind::InsufficientBalance {
                warning: Some(warning),
            },
        );
    }

    let (Some(_), Some(_)) = (model.selected_token.as_ref(), model.account.as_ref()) else {
        // No token/account context: jump straight to confirm (the TS `else`).
        model.step = SendStep::Confirm;
        return Command::all([confirm_probes(model), render()]);
    };

    match model.public_key_hex.clone() {
        Some(pk) => start_precheck(model, pk),
        None => {
            let account_id = model
                .account
                .as_ref()
                .map(|a| a.id.clone())
                .unwrap_or_default();
            let id = next(model);
            model.pipeline = Pipeline::ContinueCredential { id };
            Command::all([
                issue(id, SendOperation::LoadAccountCredential { account_id }),
                render(),
            ])
        }
    }
}

/// The real-shape estimate context (`useSendController.ts:732-753`): the batch
/// modes use the RAW transfer legs (no circular fee dependency); any build
/// failure falls back to the rough basis.
fn build_estimate_shape(
    model: &Model,
    token: &SendToken,
) -> (Option<FeeCall>, Option<Vec<FeeCall>>) {
    if model.multi_select_mode {
        let raw: Vec<MultiTokenSpec> = picked_tokens(model)
            .iter()
            .map(|tk| MultiTokenSpec {
                token_address: tk.token_address.clone(),
                decimals: tk.decimals,
                amount: full_balance(tk),
            })
            .collect();
        return match build_multi_token_calls(model.recipient.trim(), &raw) {
            Some(batch) => (None, Some(batch)),
            None => (None, None),
        };
    }
    if model.split_mode {
        return match build_split_calls(
            token.token_address.as_deref(),
            token.decimals,
            &model.recipients,
        ) {
            Some(batch) => (None, Some(batch)),
            None => (None, None),
        };
    }
    if !model.amount.is_empty() && is_valid_address(&model.recipient) {
        let amount = model_token_amount(model, token);
        let Some(units) = to_base_units(&amount, token.decimals) else {
            return (None, None);
        };
        let call = match token.token_address.as_deref() {
            None => FeeCall {
                to: model.recipient.trim().to_owned(),
                value: units.to_string(),
                data: "0x".to_owned(),
            },
            Some(addr) => {
                let Some(data) = encode_erc20_transfer(model.recipient.trim(), units) else {
                    return (None, None);
                };
                FeeCall {
                    to: addr.to_owned(),
                    value: "0".to_owned(),
                    data,
                }
            }
        };
        return (Some(call), None);
    }
    (None, None)
}

fn start_precheck(model: &mut Model, public_key_hex: String) -> Cmd {
    let (Some(token), Some(account)) = (model.selected_token.clone(), model.account.clone()) else {
        return Command::done();
    };
    model.public_key_hex = Some(public_key_hex.clone());
    // The estimate is mandatory; a stale one must never gate this run.
    model.estimating_gas = true;
    model.fee_estimate = None;
    let chain_id = token.chain_id;
    let (tx, batch) = build_estimate_shape(model, &token);

    let fee_id = next(model);
    let treasury_id = next(model);
    let timer_id = next(model);
    model.pipeline = Pipeline::PreCheck {
        fee_id,
        treasury_id,
        timer_id,
        fee: None,
        treasury: None,
    };
    Command::all([
        issue(
            fee_id,
            SendOperation::EstimateFee {
                chain_id,
                account: account.address,
                tx,
                batch,
                gas_fee_token: model.gas_fee_token.clone(),
                public_key_hex: Some(public_key_hex),
            },
        ),
        issue(treasury_id, SendOperation::ProbeTreasury { chain_id }),
        issue(
            timer_id,
            SendOperation::StartTimer {
                ms: ESTIMATE_TIMEOUT_MS,
                tag: SendTimerTag::EstimateTimeout,
            },
        ),
        render(),
    ])
}

fn precheck_settle(model: &mut Model) -> Cmd {
    let Pipeline::PreCheck { fee, treasury, .. } = &model.pipeline else {
        return Command::done();
    };
    let (Some(fee), Some(treasury)) = (fee.clone(), treasury.clone()) else {
        return Command::done(); // still waiting for the other half
    };
    model.pipeline = Pipeline::Idle;
    model.estimating_gas = false;
    model.fee_estimate = Some(fee);
    if let Some(status) = treasury {
        // A depleted relayer opens the bootstrap sheet HERE, replacing the
        // personal funding sheet entirely; confirm is not entered.
        model.treasury_bootstrap = Some(status);
        return render();
    }
    model.step = SendStep::Confirm;
    Command::all([confirm_probes(model), render()])
}

// ---------------------------------------------------------------------------
// Confirm → sign → submit (`useSendController.ts:860-1111`)
// ---------------------------------------------------------------------------

fn edit_amount(model: &mut Model) -> Cmd {
    model.tx = SendTxStatus::Idle;
    model.tx_error = None;
    leave_confirm(model);
    model.step = SendStep::EnterDetails;
    Command::all([schedule_form_estimate(model), render()])
}

/// Leaving confirm resets the fee-asset choice and clears a stale erc20
/// estimate so downstream reserve math never reads 0
/// (`useSendController.ts:467-473`).
fn leave_confirm(model: &mut Model) {
    model.gas_fee_token = None;
    if matches!(
        model.fee_estimate.as_ref().map(|f| &f.fee_asset),
        Some(FeeAsset::Erc20 { .. })
    ) {
        model.fee_estimate = None;
    }
}

#[allow(clippy::neg_cmp_op_on_partial_ord)] // NaN must fall to `edit_amount`, never to signing
fn slide_confirm(model: &mut Model) -> Cmd {
    if model.step != SendStep::Confirm {
        return Command::done();
    }
    let (Some(_), Some(account)) = (model.selected_token.as_ref(), model.account.clone()) else {
        return Command::done();
    };
    // A fee re-quote can turn a valid amount into an unpayable same-token
    // send — never let the slide reach signing in that state (invariant ⑧).
    if derive_same_asset_issue(model).is_some() {
        return edit_amount(model);
    }
    // …and the same for a figure that stopped resolving. `can_confirm` disables
    // the control, but a disabled control is a suggestion — the event can still
    // arrive from a stale frame, and `to_base_units("0", d)` is a perfectly
    // valid `Some(0)`, so the build path would have happily encoded a
    // zero-value transfer and asked for a passkey over it. The recovery is the
    // one the same-asset breach already gets: back to the amount field.
    let unresolved = !model.split_mode
        && !model.multi_select_mode
        && model
            .selected_token
            .as_ref()
            .is_some_and(|token| !(js_parse_float(&model_token_amount(model, token)) > 0.0));
    if unresolved {
        return edit_amount(model);
    }

    // …and a re-quote can also outgrow the native balance that has to pay it
    // (issue #211). This is the last gate before the passkey, so it answers
    // out loud rather than bouncing: the slide is on the confirm screen, and
    // the amount is not what is wrong.
    if let Some(warning) = fee_asset_gate(model) {
        return alert(
            model,
            SendAlertKind::InsufficientBalance {
                warning: Some(warning),
            },
        );
    }
    // Synchronous single-flight lock: a second slide in the same tick is a
    // no-op (invariant ④'s acquisition half).
    let Some(gen) = model.lock.begin() else {
        return Command::done();
    };
    model.cancelled = false;
    model.tx = SendTxStatus::Preparing;
    model.tx_hash = None;
    model.user_op_hash = None;
    model.tx_error = None;
    model.receipt_failed = false;
    model.submitted_at_ms = None;
    model.receipt_signed = None;
    model.fee_held = false;
    model.fee_rejected = false;

    match model.public_key_hex.clone() {
        Some(pk) => submit_treasury_recheck(model, gen, pk),
        None => {
            let id = next(model);
            model.pipeline = Pipeline::SubmitCredential { id, gen };
            Command::all([
                issue(
                    id,
                    SendOperation::LoadAccountCredential {
                        account_id: account.id,
                    },
                ),
                render(),
            ])
        }
    }
}

/// Recheck the relayer float immediately before signing — covers the race
/// window after the send-page preflight (invariant ⑭,
/// `useSendController.ts:926-930`).
fn submit_treasury_recheck(model: &mut Model, gen: u64, public_key_hex: String) -> Cmd {
    let Some(token) = model.selected_token.as_ref() else {
        return Command::done();
    };
    let chain_id = token.chain_id;
    let id = next(model);
    model.pipeline = Pipeline::SubmitTreasury {
        id,
        gen,
        public_key_hex,
    };
    Command::all([
        issue(id, SendOperation::ProbeTreasury { chain_id }),
        render(),
    ])
}

/// Build the submit batch + activity lines and hand them to the shell's
/// sign→submit orchestration.
fn submit_user_op(model: &mut Model, gen: u64, public_key_hex: String) -> Cmd {
    let (Some(token), Some(account)) = (model.selected_token.clone(), model.account.clone()) else {
        return Command::done();
    };
    let chain_id = token.chain_id;

    // In-band: sign EXACTLY the fee the confirm slide displayed (amount +
    // recipient) — the bundler's 2× gate rejects a stale quote loudly and the
    // user re-confirms a NEW number, never a silent mismatch (invariant ①).
    let current_fee = model
        .fee_estimate
        .clone()
        .filter(|f| f.chain_id == chain_id);
    let max_fee_per_gas = current_fee.as_ref().map(|f| f.max_fee_per_gas.to_string());
    let quoted_fee = current_fee.as_ref().and_then(|f| {
        let recipient = f.fee_recipient.clone()?;
        let amount = match &f.fee_asset {
            FeeAsset::Erc20 { amount, .. } => *amount,
            FeeAsset::Native => f.total_wei,
        };
        Some(SendQuotedFee {
            amount: amount.to_string(),
            recipient,
        })
    });

    // One send line per output; split/multiSelect settle as ONE MultiSend
    // UserOp (one signature, one gas).
    let built: Option<(Vec<FeeCall>, Vec<SendLine>)> = if model.multi_select_mode {
        let specs = multi_token_specs(model, chain_id);
        let picked = picked_tokens(model);
        let recipient = model.recipient.trim().to_owned();
        // `specs.length === 0` throws `multiSendNoFundsAfterGas`; the generic
        // catch then words it `txErrorGeneric` (ported verbatim — the specific
        // message is minted and immediately discarded).
        build_multi_token_calls(&recipient, &specs).map(|calls| {
            let lines = specs
                .iter()
                .filter_map(|spec| {
                    let tk = picked
                        .iter()
                        .find(|t| t.token_address == spec.token_address)?;
                    Some(SendLine {
                        to: recipient.clone(),
                        to_name: model
                            .recipient_identity
                            .as_ref()
                            .and_then(|i| i.name.clone()),
                        amount: spec.amount.clone(),
                        symbol: tk.symbol.clone(),
                        decimals: tk.decimals,
                        price_usd: tk.price_usd.unwrap_or(0.0),
                        logo_urls: tk.logo_urls.clone(),
                    })
                })
                .collect();
            (calls, lines)
        })
    } else if model.split_mode {
        build_split_calls(
            token.token_address.as_deref(),
            token.decimals,
            &model.recipients,
        )
        .map(|calls| {
            let lines = model
                .recipients
                .iter()
                .map(|r| SendLine {
                    to: r.address.trim().to_owned(),
                    to_name: r
                        .name
                        .as_deref()
                        .map(str::trim)
                        .filter(|n| !n.is_empty())
                        .map(str::to_owned),
                    amount: r.amount.clone(),
                    symbol: token.symbol.clone(),
                    decimals: token.decimals,
                    price_usd: token.price_usd.unwrap_or(0.0),
                    logo_urls: token.logo_urls.clone(),
                })
                .collect();
            (calls, lines)
        })
    } else {
        let amount = model_token_amount(model, &token);
        to_base_units(&amount, token.decimals).and_then(|units| {
            let call = match token.token_address.as_deref() {
                None => FeeCall {
                    to: model.recipient.clone(),
                    value: units.to_string(),
                    data: "0x".to_owned(),
                },
                Some(addr) => FeeCall {
                    to: addr.to_owned(),
                    value: "0".to_owned(),
                    data: encode_erc20_transfer(&model.recipient, units)?,
                },
            };
            let line = SendLine {
                to: model.recipient.clone(),
                to_name: model
                    .recipient_identity
                    .as_ref()
                    .and_then(|i| i.name.clone()),
                amount,
                symbol: token.symbol.clone(),
                decimals: token.decimals,
                price_usd: token.price_usd.unwrap_or(0.0),
                logo_urls: token.logo_urls.clone(),
            };
            Some((vec![call], vec![line]))
        })
    };

    let Some((calls, lines)) = built else {
        // Any build refusal is what a thrown `BatchSendError` became: the
        // generic catch — calm, localized, semantic (invariant ⑮).
        return submit_generic_error(model, gen);
    };

    model.tx = SendTxStatus::Submitting;
    let id = next(model);
    model.pipeline = Pipeline::Submitting {
        id,
        gen,
        chain_id,
        lines,
    };
    Command::all([
        issue(
            id,
            SendOperation::SubmitUserOp {
                chain_id,
                account: account.address,
                public_key_hex,
                calls,
                max_fee_per_gas,
                gas_fee_token: model.gas_fee_token.clone(),
                quoted_fee,
            },
        ),
        render(),
    ])
}

fn submit_generic_error(model: &mut Model, gen: u64) -> Cmd {
    model.pipeline = Pipeline::Idle;
    model.tx = SendTxStatus::Error;
    model.tx_error = Some(SendTxErrorKey::Generic);
    model.lock.end(gen);
    fire(
        model,
        SendOperation::Haptic {
            kind: SendHapticKind::Error,
        },
    )
}

fn cancel_signing(model: &mut Model) -> Cmd {
    // The ✕ exists only while preparing/signing; during `submitting` (op
    // already signed and en route) cancel must not pretend to stop a payment.
    if !matches!(model.tx, SendTxStatus::Preparing | SendTxStatus::Signing) {
        return Command::done();
    }
    model.cancelled = true;
    // Release the lock so a retry starts, and invalidate the cancelled run's
    // pending `end()` (issue #91).
    model.lock.cancel();
    // Kill the PRE-SIGN pipeline: after cancel, neither a passkey prompt nor
    // a funding sheet may resurrect (invariant ③ — the intent behind TS's
    // never-read `sendCancelledRef`, made real here). An in-flight
    // sign/submit (`Submitting`) is left to its shell outcome, exactly as
    // `Passkey.cancelSign()` behaves today.
    if matches!(
        model.pipeline,
        Pipeline::SubmitCredential { .. } | Pipeline::SubmitTreasury { .. }
    ) {
        model.pipeline = Pipeline::Idle;
    }
    model.tx = SendTxStatus::Idle;
    fire(model, SendOperation::CancelPasskeySign)
}

fn retry_after_bootstrap(model: &mut Model) -> Cmd {
    model.treasury_bootstrap = None;
    // After funding the relayer, return through the step-appropriate flow
    // (`SendScreen.tsx:214-224`): enter-details re-runs the pre-confirm
    // pre-check; confirm re-runs the slide.
    match model.step {
        SendStep::EnterDetails => handle_continue(model),
        SendStep::Confirm => slide_confirm(model),
        SendStep::SelectToken => render(),
    }
}

fn handle_back(model: &mut Model) -> Cmd {
    match model.step {
        SendStep::Confirm => {
            // Never go back while a transaction is in progress (invariant ③).
            if !matches!(
                model.tx,
                SendTxStatus::Idle | SendTxStatus::Confirmed | SendTxStatus::Error
            ) {
                return Command::done();
            }
            model.tx = SendTxStatus::Idle;
            model.tx_hash = None;
            model.tx_error = None;
            leave_confirm(model);
            model.step = SendStep::EnterDetails;
            Command::all([schedule_form_estimate(model), render()])
        }
        SendStep::EnterDetails => {
            if model.multi_select_mode {
                // Back to the picker, preserving the multiSelect selection.
                model.step = SendStep::SelectToken;
            } else {
                model.selected_token = None;
                model.amount = DenominatedAmount::token("");
                model.recipient.clear();
                model.split_mode = false;
                model.recipients.clear();
                model.step = SendStep::SelectToken;
            }
            render()
        }
        SendStep::SelectToken => fire(model, SendOperation::Close),
    }
}

fn fee_updated(model: &mut Model, estimate: FeeEstimateView) -> Cmd {
    // A wire estimate that doesn't parse is refused, not guessed at.
    let Some(fee) = parse_fee_view(&estimate) else {
        return Command::done();
    };
    model.fee_estimate = Some(fee);
    if model.step == SendStep::Confirm {
        // The sim depends on the estimate (reserve math) — re-run it.
        return Command::all([confirm_probes(model), render()]);
    }
    render()
}

fn receipt_update(model: &mut Model, user_op_hash: &str, outcome: SendReceiptOutcome) -> Cmd {
    if model.user_op_hash.as_deref() != Some(user_op_hash) {
        return Command::done(); // a stale hash — some earlier submission
    }
    match outcome {
        SendReceiptOutcome::Confirmed { tx_hash } => {
            model.tx_hash = Some(tx_hash);
        }
        SendReceiptOutcome::Failed { rejected } => {
            // A definitive failure stamps the receipt — it never turns the
            // submitted payment back into an error state (invariant ⑤).
            model.receipt_failed = true;
            if rejected {
                model.fee_rejected = true;
            }
        }
        SendReceiptOutcome::FeeHeld => {
            // Waiting, not failure: queued until fees settle (invariant ⑦).
            model.fee_held = true;
        }
    }
    render()
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, id: u64, result: SendShellResult) -> Cmd {
    use SendShellResult as R;
    match result {
        R::TokensLoaded { tokens, chains } => {
            let Some((expect, purpose)) = model.flights.tokens else {
                return Command::done();
            };
            if expect != id {
                return Command::done();
            }
            model.flights.tokens = None;
            model.chains = chains;
            tokens_loaded(model, tokens, purpose)
        }
        R::TokenMetadata { meta } => match &model.flights.lock_meta {
            Some((expect, _, _)) if *expect == id => lock_meta_resolved(model, meta),
            _ => Command::done(),
        },
        R::NetworkAdded { outcome } => {
            if model.flights.add_network != Some(id) {
                return Command::done();
            }
            network_added(model, outcome)
        }
        R::AccountCredential { public_key_hex } => accept_credential(model, id, public_key_hex),
        R::FeeEstimated { outcome } => accept_fee(model, id, outcome),
        R::TreasuryProbed { probe } => accept_treasury(model, id, probe),
        R::TimerElapsed { .. } => accept_timer(model, id),
        R::Submitted {
            user_op_hash,
            now_ms,
        } => accept_submitted(model, id, user_op_hash, now_ms),
        R::SubmitFailed { failure } => accept_submit_failed(model, id, failure),
        R::RecordsPersisted => {
            let Some((expect, ctx)) = model.flights.persist.clone() else {
                return Command::done();
            };
            if expect != id {
                return Command::done();
            }
            model.flights.persist = None;
            // Only NOW may the tracker learn about the op — its patches must
            // find the records they target (invariant ⑥).
            let track_id = next(model);
            model.flights.track = Some(track_id);
            issue(
                track_id,
                SendOperation::TrackSubmitted {
                    user_op_hash: ctx.user_op_hash,
                    record_ids: ctx.record_ids,
                    chain_id: ctx.chain_id,
                },
            )
        }
        R::TrackHandedOff => {
            if model.flights.track == Some(id) {
                model.flights.track = None;
            }
            Command::done()
        }
        R::IdentityResolved { identity } => {
            if model.flights.identity != Some(id) {
                return Command::done();
            }
            model.flights.identity = None;
            model.recipient_identity = identity;
            render()
        }
        R::RiskResolved { risk } => {
            if model.flights.risk != Some(id) {
                return Command::done();
            }
            model.flights.risk = None;
            model.recipient_risk = risk;
            render()
        }
        R::SimResolved { sim_json } => {
            if model.flights.sim != Some(id) {
                return Command::done();
            }
            model.flights.sim = None;
            model.sim_json = sim_json;
            render()
        }
        // Fire-and-forget acknowledgements.
        R::TokenCacheCleared
        | R::PasskeyCancelAcknowledged
        | R::AlertAcknowledged
        | R::HapticPlayed
        | R::Closed => Command::done(),
    }
}

fn accept_credential(model: &mut Model, id: u64, public_key_hex: Option<String>) -> Cmd {
    if model.flights.prefetch_credential == Some(id) {
        model.flights.prefetch_credential = None;
        if public_key_hex.is_some() {
            model.public_key_hex = public_key_hex;
        }
        return Command::done();
    }
    match model.pipeline.clone() {
        Pipeline::ContinueCredential { id: expect } if expect == id => {
            model.pipeline = Pipeline::Idle;
            match public_key_hex {
                Some(pk) => start_precheck(model, pk),
                None => alert(model, SendAlertKind::AccountUnavailable),
            }
        }
        Pipeline::WarmCredential { id: expect } if expect == id => {
            match public_key_hex {
                Some(pk) => {
                    model.public_key_hex = Some(pk.clone());
                    let (Some(token), Some(account)) =
                        (model.selected_token.clone(), model.account.clone())
                    else {
                        model.pipeline = Pipeline::Idle;
                        return Command::done();
                    };
                    let fee_id = next(model);
                    model.pipeline = Pipeline::WarmEstimate { id: fee_id };
                    issue(
                        fee_id,
                        SendOperation::EstimateFee {
                            chain_id: token.chain_id,
                            account: account.address,
                            tx: None,
                            batch: None,
                            gas_fee_token: model.gas_fee_token.clone(),
                            public_key_hex: Some(pk),
                        },
                    )
                }
                None => {
                    // Best-effort warm-up: failure is swallowed (`catch {}`).
                    model.pipeline = Pipeline::Idle;
                    Command::done()
                }
            }
        }
        Pipeline::SubmitCredential { id: expect, gen } if expect == id => {
            match public_key_hex {
                Some(pk) => submit_treasury_recheck(model, gen, pk),
                None => {
                    // `throw new Error(txErrorPublicKey)` → the generic catch
                    // words it `txErrorGeneric` (ported verbatim).
                    submit_generic_error(model, gen)
                }
            }
        }
        _ => Command::done(),
    }
}

fn accept_fee(model: &mut Model, id: u64, outcome: SendFeeOutcome) -> Cmd {
    match model.pipeline.clone() {
        Pipeline::PreCheck {
            fee_id,
            treasury_id,
            timer_id,
            treasury,
            ..
        } if fee_id == id => match outcome {
            SendFeeOutcome::Ok { estimate } => match parse_fee_view(&estimate) {
                Some(fee) => {
                    model.pipeline = Pipeline::PreCheck {
                        fee_id,
                        treasury_id,
                        timer_id,
                        fee: Some(fee),
                        treasury,
                    };
                    precheck_settle(model)
                }
                // An unparsable wire estimate is a refusal, never a guess.
                None => precheck_fail(model, SendEstimateFailure::CalculationFailed),
            },
            SendFeeOutcome::Failed { kind } => precheck_fail(model, kind),
        },
        Pipeline::LateFee { fee_id } if fee_id == id => {
            model.pipeline = Pipeline::Idle;
            if let SendFeeOutcome::Ok { estimate } = outcome {
                // TS's raced-out `preCheck` still runs `setFeeEstimate` —
                // ported verbatim. A late failure is the unhandled rejection:
                // dropped.
                if let Some(fee) = parse_fee_view(&estimate) {
                    model.fee_estimate = Some(fee);
                    return render();
                }
            }
            Command::done()
        }
        Pipeline::MaxEstimate { id: expect } if expect == id => {
            model.pipeline = Pipeline::Idle;
            let Some(token) = model.selected_token.clone() else {
                return Command::done();
            };
            match outcome {
                SendFeeOutcome::Ok { estimate } => match parse_fee_view(&estimate) {
                    Some(fee) => {
                        apply_max_with_fee(model, &token, &fee);
                        render()
                    }
                    None => {
                        model.amount = DenominatedAmount::token(full_balance(&token));
                        render()
                    }
                },
                SendFeeOutcome::Failed { .. } => {
                    // Estimation failed — full balance; the pre-check still
                    // warns (`useSendController.ts:819-821, 841-843`).
                    model.amount = DenominatedAmount::token(full_balance(&token));
                    render()
                }
            }
        }
        Pipeline::WarmEstimate { id: expect } if expect == id => {
            model.pipeline = Pipeline::Idle;
            if let SendFeeOutcome::Ok { estimate } = outcome {
                if let Some(fee) = parse_fee_view(&estimate) {
                    model.fee_estimate = Some(fee);
                    // The warm quote knows no payee. A form that became
                    // complete while it was in flight could not arm its own
                    // quote (the pipeline slot was taken); it is armed now,
                    // and the warm figure stays on screen until it answers.
                    return Command::all([schedule_form_estimate(model), render()]);
                }
            }
            // Warm-up failure swallowed — and the form's quote gets its turn.
            Command::all([schedule_form_estimate(model), render()])
        }
        Pipeline::FormEstimate { id: expect, key } if expect == id => {
            model.pipeline = Pipeline::Idle;
            if let SendFeeOutcome::Ok { estimate } = outcome {
                if let Some(fee) = parse_fee_view(&estimate) {
                    model.fee_estimate = Some(fee);
                    model.form_quote_key = Some(key);
                    return render();
                }
            }
            // Best-effort: a failed form quote is not a refusal. Continue's
            // pre-check asks again and says so if it must (invariant ②).
            Command::done()
        }
        _ => Command::done(),
    }
}

fn precheck_fail(model: &mut Model, kind: SendEstimateFailure) -> Cmd {
    // `Promise.all` rejects as one: the treasury answer and the timer are
    // abandoned with it. Never continue with a fabricated preview
    // (invariant ②).
    model.pipeline = Pipeline::Idle;
    model.estimating_gas = false;
    alert(model, SendAlertKind::EstimateFailed { kind })
}

fn accept_timer(model: &mut Model, id: u64) -> Cmd {
    if let Pipeline::FormDebounce { timer_id } = model.pipeline {
        // Only the newest debounce is answered; an older timer firing late is
        // a keystroke that was typed past.
        return if timer_id == id {
            form_estimate_fire(model)
        } else {
            Command::done()
        };
    }
    let Pipeline::PreCheck {
        timer_id,
        fee_id,
        fee,
        ..
    } = model.pipeline.clone()
    else {
        return Command::done();
    };
    if timer_id != id {
        return Command::done();
    }
    if fee.is_some() {
        // The estimate is in; only the treasury is late. TS's race would have
        // rejected here too — same alert, and the settled estimate stays.
        model.pipeline = Pipeline::Idle;
    } else {
        // Keep listening for the late estimate (display-path quirk, ported
        // verbatim); the treasury answer is dropped with the race.
        model.pipeline = Pipeline::LateFee { fee_id };
    }
    model.estimating_gas = false;
    alert(
        model,
        SendAlertKind::EstimateFailed {
            kind: SendEstimateFailure::Timeout,
        },
    )
}

fn accept_treasury(model: &mut Model, id: u64, probe: SendTreasuryProbe) -> Cmd {
    let low_float = |probe: &SendTreasuryProbe| match probe {
        SendTreasuryProbe::LowFloat { status } => Some(status.clone()),
        _ => None,
    };
    match model.pipeline.clone() {
        Pipeline::PreCheck {
            fee_id,
            treasury_id,
            timer_id,
            fee,
            ..
        } if treasury_id == id => {
            model.pipeline = Pipeline::PreCheck {
                fee_id,
                treasury_id,
                timer_id,
                fee,
                treasury: Some(low_float(&probe)),
            };
            precheck_settle(model)
        }
        Pipeline::SubmitTreasury {
            id: expect,
            gen,
            public_key_hex,
        } if expect == id => {
            match low_float(&probe) {
                Some(status) => {
                    // The float fell below its floor after the preflight —
                    // stop BEFORE the passkey (invariant ⑭).
                    model.pipeline = Pipeline::Idle;
                    model.treasury_bootstrap = Some(status);
                    model.tx = SendTxStatus::Idle;
                    model.lock.end(gen);
                    render()
                }
                None => submit_user_op(model, gen, public_key_hex),
            }
        }
        Pipeline::FailureProbe {
            id: expect,
            gen,
            fallback,
        } if expect == id => {
            model.pipeline = Pipeline::Idle;
            let cmd = match low_float(&probe) {
                Some(status) => {
                    // The honest ask: the community bootstrap sheet, not a
                    // "try again" loop.
                    model.treasury_bootstrap = Some(status);
                    model.tx = SendTxStatus::Idle;
                    render()
                }
                None => {
                    model.tx = SendTxStatus::Error;
                    model.tx_error = Some(match fallback {
                        FailureFallback::Generic => SendTxErrorKey::Generic,
                        FailureFallback::BundlerFund => SendTxErrorKey::BundlerFund,
                    });
                    fire(
                        model,
                        SendOperation::Haptic {
                            kind: SendHapticKind::Error,
                        },
                    )
                }
            };
            // The TS `finally` runs once the catch (incl. this probe) ends.
            model.lock.end(gen);
            cmd
        }
        _ => Command::done(),
    }
}

fn accept_submitted(model: &mut Model, id: u64, user_op_hash: String, now_ms: f64) -> Cmd {
    let Pipeline::Submitting {
        id: expect,
        gen,
        chain_id,
        lines,
    } = model.pipeline.clone()
    else {
        return Command::done();
    };
    if expect != id {
        return Command::done();
    }
    model.pipeline = Pipeline::Idle;

    // Bundler accepted — the payment is sent NOW. The tx hash resolves in the
    // background; a slow poll can never turn this into an error (invariant ⑤).
    let is_batch = model.multi_select_mode || model.split_mode;
    if is_batch {
        model.receipt_lines = Some(lines.clone());
        model.receipt_kind = Some(if model.multi_select_mode {
            SendReceiptKind::MultiSelect
        } else {
            SendReceiptKind::Split
        });
    } else {
        model.receipt_lines = None;
        model.receipt_kind = None;
    }
    // The signature is now a fact. Freeze the money it moved (and the price it
    // moved at) so no later display-currency commit can restate it — the
    // receipt reads THIS and never converts again.
    model.receipt_signed = lines.first().cloned();
    model.user_op_hash = Some(user_op_hash.clone());
    model.submitted_at_ms = Some(now_ms);
    model.tx = SendTxStatus::Confirmed;
    model.lock.end(gen);

    // One activity record per recipient, all persisted in ONE atomic write
    // (invariant ⑥) — a per-record write would drop every sibling but one.
    let from = model
        .account
        .as_ref()
        .map(|a| a.address.clone())
        .unwrap_or_default();
    let timestamp_s = (now_ms / 1000.0).floor();
    let records: Vec<SendTxRecord> = lines
        .iter()
        .enumerate()
        .map(|(i, ln)| {
            let usd = parse_float_or_zero(&ln.amount) * ln.price_usd;
            SendTxRecord {
                id: if lines.len() > 1 {
                    format!("{user_op_hash}-{i}")
                } else {
                    user_op_hash.clone()
                },
                user_op_hash: user_op_hash.clone(),
                tx_hash: String::new(),
                from: from.clone(),
                to: ln.to.clone(),
                to_name: ln.to_name.clone(),
                value: ln.amount.clone(),
                symbol: ln.symbol.clone(),
                decimals: ln.decimals,
                logo_urls: ln.logo_urls.clone(),
                chain_id,
                timestamp_s,
                usd: (usd > 0.0).then(|| format!("${usd:.2}")),
            }
        })
        .collect();
    let record_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();

    let clear_id = next(model);
    let haptic_id = next(model);
    let persist_id = next(model);
    model.flights.persist = Some((
        persist_id,
        PersistCtx {
            user_op_hash,
            record_ids,
            chain_id,
        },
    ));
    Command::all([
        issue(
            haptic_id,
            SendOperation::Haptic {
                kind: SendHapticKind::Success,
            },
        ),
        issue(clear_id, SendOperation::ClearTokenCache { address: from }),
        issue(persist_id, SendOperation::PersistTxRecords { records }),
        render(),
    ])
}

fn accept_submit_failed(model: &mut Model, id: u64, failure: SendSubmitFailure) -> Cmd {
    let Pipeline::Submitting {
        id: expect, gen, ..
    } = model.pipeline.clone()
    else {
        return Command::done();
    };
    if expect != id {
        return Command::done();
    }
    model.pipeline = Pipeline::Idle;
    match failure {
        SendSubmitFailure::PasskeyCancelled => {
            // Never an error state, never an alert.
            model.tx = SendTxStatus::Idle;
            model.lock.end(gen);
            render()
        }
        SendSubmitFailure::RelayerUnavailable => {
            // No usable relayer float — ask the treasury whether the honest
            // surface is the bootstrap sheet; a transient blip falls through
            // to the generic error.
            failure_probe(model, gen, FailureFallback::Generic)
        }
        SendSubmitFailure::BundlerUnderfunded => {
            // Never open the personal top-up sheet from a reactive bundler
            // error: recheck only the relayer treasury.
            failure_probe(model, gen, FailureFallback::BundlerFund)
        }
        SendSubmitFailure::Other { .. } => {
            // Raw RPC/library wording never reaches the money screen — the
            // shell logged it; the view gets the calm semantic key
            // (invariant ⑮).
            model.tx = SendTxStatus::Error;
            model.tx_error = Some(SendTxErrorKey::Generic);
            model.lock.end(gen);
            fire(
                model,
                SendOperation::Haptic {
                    kind: SendHapticKind::Error,
                },
            )
        }
    }
}

fn failure_probe(model: &mut Model, gen: u64, fallback: FailureFallback) -> Cmd {
    let Some(token) = model.selected_token.as_ref() else {
        return submit_generic_error(model, gen);
    };
    let chain_id = token.chain_id;
    let id = next(model);
    model.pipeline = Pipeline::FailureProbe { id, gen, fallback };
    Command::all([
        issue(id, SendOperation::ProbeTreasury { chain_id }),
        render(),
    ])
}

// ---------------------------------------------------------------------------
// View helpers
// ---------------------------------------------------------------------------

fn fee_to_view(fee: &FeeEstimate) -> FeeEstimateView {
    FeeEstimateView {
        chain_id: fee.chain_id,
        total_wei: fee.total_wei.to_string(),
        max_fee_per_gas: fee.max_fee_per_gas.to_string(),
        network_fee_per_gas: fee.network_fee_per_gas.to_string(),
        relayer_fee_per_gas: fee.relayer_fee_per_gas.to_string(),
        bundler_gas_price: fee.bundler_gas_price.to_string(),
        in_band_gas_basis: fee.in_band_gas_basis.to_string(),
        total_gas: fee.total_gas.to_string(),
        deployed: fee.deployed,
        tier: fee.tier,
        quoted: fee.quoted,
        fee_asset: match &fee.fee_asset {
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
        fee_recipient: fee.fee_recipient.clone(),
    }
}

fn parse_fee_view(view: &FeeEstimateView) -> Option<FeeEstimate> {
    let parse = |s: &str| s.trim().parse::<u128>().ok();
    Some(FeeEstimate {
        chain_id: view.chain_id,
        total_wei: parse(&view.total_wei)?,
        max_fee_per_gas: parse(&view.max_fee_per_gas)?,
        network_fee_per_gas: parse(&view.network_fee_per_gas)?,
        relayer_fee_per_gas: parse(&view.relayer_fee_per_gas)?,
        bundler_gas_price: parse(&view.bundler_gas_price)?,
        in_band_gas_basis: parse(&view.in_band_gas_basis)?,
        total_gas: parse(&view.total_gas)?,
        deployed: view.deployed,
        tier: view.tier,
        quoted: view.quoted,
        fee_asset: match &view.fee_asset {
            FeeAssetView::Native => FeeAsset::Native,
            FeeAssetView::Erc20 {
                token,
                decimals,
                amount,
                symbol,
            } => FeeAsset::Erc20 {
                token: token.clone(),
                decimals: *decimals,
                amount: parse(amount)?,
                symbol: symbol.clone(),
            },
        },
        fee_recipient: view.fee_recipient.clone(),
    })
}

fn receipt_view(model: &Model, stage: SendStage) -> Option<SendReceiptView> {
    if stage != SendStage::Receipt {
        return None;
    }
    // The receipt is a screen about a selected token (`SendScreen.tsx:147`).
    model.selected_token.as_ref()?;
    // READ, never re-derive. `model_token_amount` used to be called here, which
    // re-ran the fiat↔token conversion against the display context of the
    // moment: the receipt's number then tracked the currency picker instead of
    // the signature, and printed `0` as soon as the rate went away. Both the
    // figure and the price it was worth are read off the submit-time snapshot,
    // which is the same discipline `transfers` below has always had (its lines
    // are captured at submit too).
    let signed = model.receipt_signed.as_ref();
    let amount = signed.map(|ln| ln.amount.clone()).unwrap_or_default();
    let usd_value = signed.map_or(0.0, |ln| js_parse_float(&ln.amount).max(0.0) * ln.price_usd);
    let transfers = model
        .receipt_lines
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|ln| SendReceiptTransfer {
            to: ln.to.clone(),
            to_name: ln.to_name.clone(),
            amount: ln.amount.clone(),
            symbol: ln.symbol.clone(),
            logo_urls: ln.logo_urls.clone(),
            usd_value: parse_float_or_zero(&ln.amount) * ln.price_usd,
        })
        .collect();
    Some(SendReceiptView {
        status: if model.receipt_failed {
            SendReceiptStatus::Failed
        } else if model.tx_hash.is_some() {
            SendReceiptStatus::Confirmed
        } else {
            SendReceiptStatus::Submitted
        },
        hold_reason: if model.fee_rejected {
            Some(SendHoldReason::FeeRejected)
        } else if model.fee_held {
            Some(SendHoldReason::FeeHold)
        } else {
            None
        },
        kind: model.receipt_kind,
        transfers,
        amount,
        usd_value: if usd_value.is_nan() { 0.0 } else { usd_value },
        submitted_at_ms: model.submitted_at_ms,
        typical_inclusion_s: model.selected_token.as_ref().and_then(|token| {
            super::network_admin::BUILTIN_CHAINS
                .iter()
                .find(|chain| chain.chain_id == token.chain_id)
                .map(|chain| chain.typical_inclusion_s)
        }),
    })
}

impl super::SplitEffect for SendEffect {
    type Op = SendOperation;
    fn into_shell(self) -> Option<crux_core::Request<SendOperation>> {
        match self {
            SendEffect::Render(_) => None,
            SendEffect::Shell(request) => Some(request),
        }
    }
}
