//! Machine — send (spec `017-crux-wallet-state`, inventory `### send (P1)`).
//!
//! ```text
//! Open ─► SelectToken ─► EnterDetails ─► Continue{estimate ∥ treasury, 15s cap}
//!            │ multi picker                  │ pass ─► Confirm ─tap───► lock.begin
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
//! Spec 082 round 2 (RJ1) — the records exist before the bytes leave: the
//! shell reports [`Event::OpSigned`] (signed, hashed, nothing POSTed), the
//! core writes the records "may have been sent" and hands the op to the
//! tracker with no record (a POST is about to leave: no "not sent" until its
//! verdict), and only then [`SendOperation::ClearToPost`] lets the shell POST.
//! The relay's verdict hands the records over (marking them admitted when the
//! relay took the op), or a proven "not sent" deletes and withdraws them; the
//! receipt screen still waits for that verdict. A submit the person cancels
//! (✕ while "signing") before its records are written is never cleared to
//! POST — and, as `OpSigned` names no submit, no `OpSigned` is cleared while
//! that submit's result is still owed (082 second review).
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
    encode_erc20_transfer, from_base_units, is_tempo_chain, max_native_sendable, reserve_fee_token,
    reserve_native_gas, same_asset_fee_limit, to_base_units, FeeAsset, FeeAssetView, FeeCall,
    FeeEstimate, FeeEstimateView, FeeTier, MultiTokenSpec, TEMPO_DEFAULT_FEE_TOKEN,
    TEMPO_FEE_TOKEN_DECIMALS,
};
use super::money::{js_parse_float, Denom, DenominatedAmount, TokenPrice};
use super::tx_tracker::{TrackEntryView, TrackOutcome, TrackStatus};

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

/// How many chains the picker reads fees ahead for ([`SendOperation::PrewarmFees`]):
/// enough for where nearly everybody's money is, few enough that opening Send
/// is not a burst of a dozen relay calls.
pub const PREWARM_CHAINS: usize = 4;

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

/// The split editor's unfinished rows, field by field — the reasons behind
/// [`SendView::can_continue`] in a split.
///
/// The gate was one boolean over every row, so a dark `Continue` could not say
/// WHICH of forty recipients was the problem; and it judged an amount by its
/// leading digits (`recipients_are_valid`, ported verbatim), so `1,5` armed the
/// button while [`sum_split_base_units`] could not read it — `Continue` then
/// returned without a word and the total went blank. An amount is `Ok` here
/// only when BOTH hold: it is positive, and it is a figure the batch can
/// actually be built from. The gate now asks this list, so the button, the
/// row's own message and the signature cannot disagree.
pub fn split_row_issues(
    recipients: &[SendRecipientDraft],
    decimals: u32,
) -> Vec<SendSplitRowIssue> {
    recipients
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let address = row.address.trim();
            let address_state = if address.is_empty() {
                SendRowFieldState::Empty
            } else if is_valid_address(address) {
                SendRowFieldState::Ok
            } else {
                SendRowFieldState::Invalid
            };
            let amount = row.amount.trim();
            let amount_state = if amount.is_empty() {
                SendRowFieldState::Empty
            } else if js_parse_float(amount) > 0.0
                && to_base_units(amount, decimals).is_some_and(|units| units > 0)
            {
                SendRowFieldState::Ok
            } else {
                SendRowFieldState::Invalid
            };
            (address_state != SendRowFieldState::Ok || amount_state != SendRowFieldState::Ok).then(
                || SendSplitRowIssue {
                    id: row.id.clone(),
                    ordinal: index as u32 + 1,
                    address: address_state,
                    amount: amount_state,
                },
            )
        })
        .collect()
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
    /// Whether this is a network Vela ships, and therefore one whose relayer
    /// the OPERATOR is expected to keep funded
    /// ([`network_admin::is_builtin_chain`]). The shell does not send this —
    /// the core fills it when it publishes the sheet — because it decides
    /// which of two different things the person is asked to do: tell the
    /// operator, or fund it themselves. On a network someone added, the
    /// operator may have no way to hold gas there at all.
    #[serde(default)]
    pub operator_served: bool,
    /// `balance` and `floor` in the coin they are counted in, and what the
    /// stop asks for (issue 422). Like `operator_served`, the shell does not
    /// send this: the core fills it when it publishes the stop. `None` on a
    /// published stop only when the relay's figures could not be read.
    #[serde(default)]
    pub coin: Option<SendTreasuryCoin>,
}

/// The relay's treasury figures in the coin they are counted in (issue 422).
///
/// The relay reports base units of the asset the treasury pays gas with —
/// wei of the chain's own coin, or micro-pathUSD on Tempo — and nothing else.
/// Every shell used to turn those into words by itself: the coin's name from
/// a lookup of its own (Android's fell back to "ETH" for any chain name it did
/// not recognise; the others to nothing), the decimals as "18 unless
/// pathUSD", and the amount to ask for as the shortfall — or, on the web
/// alone, at least a flat `0.01` of whatever the coin was, which is tens of
/// dollars of ETH and a tenth of a cent of POL. One stop, four answers. The
/// core says it once, for the chain the stop is about.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendTreasuryCoin {
    /// The coin the treasury pays gas with: pathUSD on Tempo, otherwise the
    /// chain's own coin, as the wallet writes it. `None` when the wallet does
    /// not know that chain's coin — never another chain's, never a default.
    pub symbol: Option<String>,
    /// What the treasury holds, in whole coin (`"0.00004"`). A plain decimal:
    /// the shell writes the decimal mark its way and nothing else.
    pub balance: String,
    /// The floor the relay says it needs, in whole coin.
    pub floor: String,
    /// The suggested contribution (spec 098 §4): what the treasury lacks of
    /// its floor, `floor − balance`, in whole coin — the relay's own figure
    /// for this chain, so it is sized in this chain's coin by construction.
    /// The stop closes by itself once the relay reports the floor reached.
    pub suggested: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTreasuryAsset {
    Native,
    PathUsd,
}

impl SendTreasuryAsset {
    /// The decimals of the base units the relay counts this asset in: wei of
    /// the chain's own coin, or micro-pathUSD (vela-relay `treasury.rs`,
    /// `TreasuryStatus::balance`).
    #[must_use]
    pub const fn decimals(self) -> u32 {
        match self {
            Self::Native => 18,
            Self::PathUsd => TEMPO_FEE_TOKEN_DECIMALS,
        }
    }
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

/// The relay said it cannot serve this chain (spec 098 §2): the treasury probe
/// came back `Uncovered` — a 404, which the relay answers when it has no RPC it
/// can use for the chain, and a self-hosted one answers for a chain it does not
/// list. Funding cannot fix that, so it is NOT the treasury sheet: until 098
/// this case walked straight on to the passkey and failed after the person had
/// signed, without a word.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendRelayUnreachable {
    pub chain_id: u32,
    /// As [`SendTreasuryStatus::operator_served`]: on a network Vela ships the
    /// relay is the operator's to fix; on one the person added, reaching it is
    /// theirs (a public `https` RPC, or a relay beside their node). Filled by
    /// the core when it publishes the sheet.
    #[serde(default)]
    pub operator_served: bool,
}

/// What "Report this" files about a relay stop on a network Vela ships
/// (issue 466): built here once, so every shell files the same words under
/// the same dedup key through the in-app reporter it already has (preview,
/// consent, send, the prefilled form as the fallback).
///
/// English on purpose: it is read on the tracker by whoever runs the relay,
/// not by the person — data, not corpus. It names the relay treasury's full
/// address and figures. Those are the OPERATOR's, and public (the relay
/// serves them at `/v1/treasury/<chain>`), never the person's: which is why
/// they ride in `what`, which the reporter files as written, and never in
/// the device facts it scrubs of addresses.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendRelayReport {
    /// What happened. The first line is the issue's title, at most 80
    /// characters ("Relayer out of gas on Unichain (130)"); the facts follow
    /// after a blank line.
    pub what: String,
    /// How the person got there, numbered as the bug form asks.
    pub steps: String,
    /// The bug form's area option, verbatim: `"Send"`.
    pub area: String,
    /// The dedup key — `relay-gas-<chain>` or `relay-unreachable-<chain>`: one
    /// open issue per outage per chain, however many people report it and
    /// whatever the balance reads meanwhile.
    pub fingerprint: String,
}

/// [`SendRelayReport::area`]: the bug form's "Send" option.
pub const RELAY_REPORT_AREA: &str = "Send";

/// The coin the form's fee row names ([`SendView::fee_coin`]): what the shell
/// hands its token mark — the core's `remote_mark::token_mark` — to draw.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendFeeCoin {
    /// The coin's ticker as the wallet writes it. Empty only for an ERC-20
    /// nothing on the form names — the mark then draws its logo alone.
    pub symbol: String,
    /// `None` = the chain's own coin.
    pub contract: Option<String>,
    /// The chain the fee is paid on — never 0.
    pub chain_id: u32,
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
    /// Where the name came from, as the shell's resolver labels it: `self`
    /// (one of the person's own accounts), `passkey` (the public wallet
    /// registry), or a name service's label (`ENS`, `.bnb`, `Basename`…).
    pub source: Option<String>,
}

/// Whose word a payee's name is (spec 097 F, S2).
///
/// The public wallet registry is a name anyone can register for their own
/// address: "Wallet", "Binance", the name of the friend someone is about to
/// pay. On the page that signs, such a name is a claim about the address,
/// never the address itself, and never the person's own name for them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendNameSource {
    /// The person's own word: one of their own accounts, a contact they
    /// named, a name column in a list they imported. Drawn with no tag.
    Own,
    /// The public wallet registry (the passkey index). Drawn with
    /// `send.velaUser` beside it.
    Registry,
    /// A name service whose name resolves forward to this address
    /// (`name_verify`). `label` is the service's own name ("ENS", ".bnb"),
    /// drawn as it is.
    Service { label: String },
}

/// One payee as the confirm page names them (spec 097 F, S2): the address
/// always, and a name only beside it, with whose word that name is.
///
/// The shell draws `name` (or, without one, the short address in mono), the
/// short address under a name, the tag for a name that is not the person's
/// own, and the full address on tap — it never decides any of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendPayee {
    /// The address the money goes to, in full, as it will be signed.
    pub address: String,
    pub name: Option<String>,
    /// `Some` exactly when `name` is.
    pub name_source: Option<SendNameSource>,
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
    /// The speed this fee was priced at, named on the wire as
    /// `eth_sendUserOperation`'s third parameter (spec 068's relay contract).
    ///
    /// Taken from the SAME estimate as the amount (spec 069), so the tier the
    /// relay is told and the figure the person approved are one measurement
    /// — no shell keeps a second copy of which speed this send is. `None`
    /// names nothing (the pre-068 wire): no estimate, or the dead `rapid`,
    /// which the relay refuses and nothing offers.
    #[serde(default)]
    pub tier: Option<FeeTier>,
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
    /// The submit's reply was lost; `user_op_hash` is the locally computed
    /// hash (spec 082 RA4). Persisted with the record so a restart hands the
    /// tracker a may-have-been-sent op again (`TrackPendingRecord`).
    #[serde(default)]
    pub maybe_sent: bool,
    /// The head read before the first submit POST, persisted likewise —
    /// where the tracker's relay-independent landing check starts (ruling 8).
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub submit_block: Option<u64>,
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
    /// Spec 102: the shell refused to sign because the account's venue cannot
    /// be used here — `signing_plan`'s `blocked`, or the web's
    /// (`signingPlan(…, "web")`). Nothing was signed or sent; the confirm
    /// screen says why in the person's language
    /// ([`SendTxErrorKey::VenueBlocked`], [`SendView::tx_venue_block`]).
    VenueBlocked {
        block: crate::signing_venue::VenueBlock,
    },
    /// The relay refused it: another transaction of this account holds the
    /// nonce (`RelayRejection::NonceHeld` — the relay's `nonce_in_flight`, or
    /// an older relay's `[existingHash:…]`). Nothing was sent.
    PreviousPending,
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
        /// The relay never had a may-have-been-sent op (tracker `NotSent`,
        /// spec 082 RA4): "not sent", never the fee-rejected words.
        #[serde(default)]
        not_sent: bool,
        /// `rejected` only: why the relay refused it (the tracker entry's
        /// `refusal`). Only [`RefusalReason::FeeBelowMarket`] is told as
        /// "network fees stayed above the amount you approved"; `None` is the
        /// plain refusal.
        ///
        /// [`RefusalReason::FeeBelowMarket`]: super::tx_tracker::RefusalReason::FeeBelowMarket
        #[serde(default, skip_serializing_if = "Option::is_none")]
        refusal: Option<super::tx_tracker::RefusalReason>,
    },
    /// The relay parked the op until fees settle — pending, new wording only
    /// (invariant ⑦).
    FeeHeld,
    /// The relay is topping up its gas on the chain before it sends the op
    /// (tracker `RelayFunding`): pending, its own words, gone once the relay
    /// moves on.
    RelayFunding,
    /// The relay has shown it holds an op whose submit reply was lost (spec
    /// 082 RA10): the receipt goes back to the ordinary "submitted" words.
    Acknowledged,
}

/// The receipt verdict a tracker entry stands for, or `None` while there is
/// nothing new to say (spec 082 — the one mapping, where the desktop and the
/// web each had their own). Only a definitive drop, rejection or never-sent
/// may be `Failed`; a slow, unreachable or 24 h-old op sends nothing
/// (invariant ⑤).
pub fn receipt_outcome_of(entry: &TrackEntryView) -> Option<SendReceiptOutcome> {
    match entry.status {
        TrackStatus::Confirmed => entry
            .tx_hash
            .clone()
            .filter(|hash| !hash.is_empty())
            .map(|tx_hash| SendReceiptOutcome::Confirmed { tx_hash }),
        TrackStatus::Dropped => Some(SendReceiptOutcome::Failed {
            rejected: false,
            not_sent: false,
            refusal: None,
        }),
        TrackStatus::Rejected => Some(SendReceiptOutcome::Failed {
            rejected: true,
            not_sent: false,
            refusal: entry.refusal,
        }),
        TrackStatus::NotSent => Some(SendReceiptOutcome::Failed {
            rejected: false,
            not_sent: true,
            refusal: None,
        }),
        TrackStatus::FeeHeld => Some(SendReceiptOutcome::FeeHeld),
        TrackStatus::RelayFunding => Some(SendReceiptOutcome::RelayFunding),
        TrackStatus::Pending | TrackStatus::Unreachable | TrackStatus::AcceptedNotLanded => {
            match entry.outcome {
                TrackOutcome::Landing | TrackOutcome::StillConfirming => {
                    Some(SendReceiptOutcome::Acknowledged)
                }
                TrackOutcome::MaybeSent | TrackOutcome::Unknown | TrackOutcome::Final => None,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendTimerTag {
    EstimateTimeout,
    /// The form's own quote, debounced (spec 028 Phase 9, T490).
    FormEstimate,
    /// The treasury sheet is open: ask the relay again (spec 098 §4), so the
    /// send carries on by itself once somebody has funded it.
    TreasuryWatch,
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
        /// Nobody has chosen the fee coin on this form: the shell passes it
        /// on as `fee_policy`'s `auto_fee_token`, the fee machine pays in a
        /// coin that can, and the estimate's `fee_asset` says which. Every
        /// reader downstream — Max, the balance gates, the submit — takes
        /// the coin from that estimate, never from `gas_fee_token`.
        #[serde(default)]
        auto_fee_token: bool,
    },
    /// `probeTreasury(chainId)`.
    ProbeTreasury {
        chain_id: u32,
    },
    /// Read ahead, into the shell's own fee caches, what a quote on each of
    /// these chains will need — the deployment read, the gas signals, the
    /// relay's gas quote, the in-band rows (Tempo: the fee recipient). Nothing
    /// is priced and nothing comes back: answer `FeesPrewarmed` at once.
    ///
    /// Asked when the picker opens, for the chains the person holds value on
    /// (highest first, at most [`PREWARM_CHAINS`]). Measured on the live relay
    /// those reads are 3–5 s of a 4.5–6 s first quote and cacheable for 8–15 s;
    /// read while the person is choosing, the quote a pick starts is left
    /// with only its simulation.
    PrewarmFees {
        account: String,
        chain_ids: Vec<u32>,
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
        /// EMPTY on the write-ahead's hand-off (spec 082 RJ1, second review):
        /// a POST is about to leave and the tracker holds the op off "not
        /// sent" until the POST's verdict, whose hand-off names the records.
        /// Forwarded as they are, empty or not.
        record_ids: Vec<String>,
        chain_id: u32,
        /// Forwarded to `tx_tracker::Event::Submitted` (spec 082 RA4).
        #[serde(default)]
        maybe_sent: bool,
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
        /// Forwarded to `tx_tracker::Event::Submitted` (spec 082 RJ1): the
        /// relay accepted the op the write-ahead hand-off announced.
        #[serde(default)]
        admitted: bool,
        /// The account that signed it, forwarded to
        /// `tx_tracker::Event::Submitted` — what makes it an op this account
        /// must wait for on this chain (`tx_tracker::in_flight_ops`).
        #[serde(default)]
        sender: Option<String>,
    },
    /// The write-ahead records are on disk and the tracker holds them (spec
    /// 082 RJ1): the shell may now POST `user_op_hash`, and only now. With no
    /// clearance within `user_op::WRITE_AHEAD_WAIT_MS` it does not POST and
    /// answers `SubmitFailed` (nothing sent). Answered `PostCleared`.
    ClearToPost {
        user_op_hash: String,
    },
    /// The relay accepted the written-ahead op (RJ1): patch these records'
    /// `maybeSent` to false, in ONE write; they stay pending. Answered
    /// `RecordsPersisted`.
    MarkAdmitted {
        record_ids: Vec<String>,
    },
    /// Remove written-ahead records whose op is proven never sent (RJ1), in
    /// ONE write. Answered `RecordsPersisted`.
    DeleteTxRecords {
        ids: Vec<String>,
    },
    /// Forwarded to `tx_tracker::Event::Withdrawn` (RJ1). Answered
    /// `TrackHandedOff`.
    TrackWithdrawn {
        user_op_hash: String,
        record_ids: Vec<String>,
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
// The same reason `SendFeeOutcome` below carries it, and this enum inherits the
// size from exactly that variant: a wire type whose JSON shape is pinned by the
// generated TS. Boxing would buy an allocation per shell answer and change
// nothing a shell can see. It crossed the lint's 200-byte threshold when
// `FeeEstimateView` grew its effective gas price (issue 684) — 280 bytes to
// 304 against an 80-byte neighbour — which is a fact about the lint's default,
// not about this enum.
#[allow(clippy::large_enum_variant)]
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
    /// `PrewarmFees` was taken; the reads run on without the core.
    FeesPrewarmed,
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
        /// The submit's reply was lost and `user_op_hash` is the local hash
        /// (spec 082 RA4): the payment may be on its way, so it is recorded,
        /// tracked and shown as such — never an error, never "try again".
        #[serde(default)]
        maybe_sent: bool,
        /// The head read before the first POST (ruling 8); `None` = unknown.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
    },
    SubmitFailed {
        failure: SendSubmitFailure,
    },
    PasskeyCancelAcknowledged,
    RecordsPersisted,
    TrackHandedOff,
    /// `ClearToPost` was taken (spec 082 RJ1).
    PostCleared,
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
    /// The asset list's holdings changed while this screen is open.
    ///
    /// Send shows the SAME holdings the asset list does — one source, so the
    /// balance beside the token here is the balance on the home row, and the
    /// screen opens on what is already in memory instead of walking every
    /// chain again. The shell answers `FetchTokens` from its balance machine
    /// and dispatches this whenever that machine's tokens move afterwards (a
    /// pull, a poll, a confirmed transfer), mapped exactly as it answers
    /// `FetchTokens`. Without it a refresh started from here landed on the
    /// home screen only, and this screen kept the old balance.
    HoldingsUpdated {
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
    /// The same rows, ADDED to what the person has already entered instead of
    /// replacing it ([`append_split`]). A list brought to a form that already
    /// has people on it is, nearly always, more people — and the seed above
    /// threw the typed ones away without a word. A new variant rather than a
    /// flag on the old one: every shell that still sends the old event keeps
    /// exactly the behaviour it had.
    AppendSplitRecipients {
        recipients: Vec<SendRecipientDraft>,
    },
    /// The split editor's whole-array onChange; ≤1 row collapses back to
    /// single mode carrying the remaining row.
    RecipientsChanged {
        recipients: Vec<SendRecipientDraft>,
    },
    /// `target` = the split row the picker fills; `None` = the single-mode
    /// recipient field, or — in a split — the first row with no address,
    /// else a new row.
    OpenContactPicker {
        target: Option<String>,
    },
    CloseContactPicker,
    /// The contact picker chose an address. It lands where
    /// [`Event::OpenContactPicker`]'s `target` says; a shell never adds a
    /// blank row of its own to aim a split's pick.
    PickedAddress {
        address: String,
    },
    OpenScanner,
    CloseScanner,
    /// A scan, parsed by the shell. Routing (`SendScreen.tsx:181-203`): in a
    /// split, ONLY the address, into the row a pick would take (invariant
    /// ⑬); a full request re-locks the whole flow; anything else fills the
    /// recipient.
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
    /// The form's token card (issue 326): back to the asset picker to choose
    /// another asset, keeping whoever the form is for — typed, scanned or
    /// handed over. Offered only where [`SendView::can_change_token`] says.
    ChangeToken,
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
    /// The fee card's coin in force — `FeeView.fee_token`, verbatim (`None` =
    /// the chain's own coin). It names the fee row's coin while no estimate
    /// is in hand ([`SendView::fee_coin`]) — when nobody chose, the fee
    /// machine picks a coin that can pay, and a quote that then fails leaves
    /// that coin in force with no estimate to say so. It prices and signs
    /// nothing.
    ///
    /// This machine files the word against the form's chain AT THE MOMENT it
    /// is said (a contract is an address on ONE chain), and DROPS it while
    /// the form has no chain yet. So every shell's bridge — the same one that
    /// mirrors [`Event::FeeBusyChanged`] — keeps one rule:
    ///
    /// - it speaks only while the fee session prices the form's own chain:
    ///   the chain of the session's last question equals the form's chain
    ///   (the selected token's, else the sweep's), both known;
    /// - it dedupes on the pair (chain id, token): it sends whenever that
    ///   pair differs from what this send journey was last told — so a coin
    ///   first seen before the form had a chain, or while the session still
    ///   priced the network just left, is told once the form is on the
    ///   priced chain, and the same coin is told again for a new chain;
    /// - a fresh journey has been told nothing: its first word always goes,
    ///   `None` included.
    FeeTokenChanged {
        fee_token: Option<String>,
    },
    /// The confirm button was tapped. The name (and its `slide_confirm`
    /// wire tag) predates the tap: every shell dispatches it, so it stays.
    SlideConfirm,
    /// The passkey sheet opened inside `SubmitUserOp`.
    SigningStarted,
    /// Inside `SubmitUserOp`: the op is signed and its hash computed, and
    /// nothing has been POSTed (spec 082 RJ1). The core writes the records
    /// ahead and answers [`SendOperation::ClearToPost`] once they are on disk
    /// and the tracker holds them. Once per submit.
    OpSigned {
        user_op_hash: String,
        /// The head read before the first POST (ruling 8); `None` = unknown.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
        now_ms: f64,
    },
    /// The confirm screen's cancel (✕) during preparing/signing.
    CancelSigning,
    /// Treasury sheet "retry" — re-runs the step-appropriate flow.
    RetryAfterBootstrap,
    DismissTreasurySheet,
    /// The "relay can't reach this network" sheet's retry (spec 098 §2) —
    /// after the person changed the network's RPC or the relay, re-runs the
    /// step-appropriate flow, as the treasury sheet's does.
    RetryRelayUnreachable,
    DismissRelayUnreachable,
    /// The error panel's retry — back to idle.
    RetryAfterError,
    /// Every operation in flight on this device, as the tracker last said
    /// (`tx_tracker::in_flight_ops_json` of its view — forwarded on every
    /// tracker render). While the account has one on the form's chain, the
    /// confirm is held ([`SendView::previous_pending`]): a second send signed
    /// now would take the same nonce. It opens once the first is final, or
    /// has made no progress for ten minutes (`tx_tracker::IN_FLIGHT_STALL_MS`
    /// — the tracker then leaves it out of the list).
    InFlightOps {
        ops: Vec<super::tx_tracker::InFlightOp>,
    },
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
    /// The relay refused the submit because another transaction of this
    /// account holds the nonce (`RelayRejection::NonceHeld`, from another
    /// device, or one this device could not follow): nothing was sent. The
    /// line is [`super::sign_confirm::PREVIOUS_PENDING_KEY`]'s; "Try again"
    /// waits for it like any held confirm.
    PreviousPending,
    /// `send.txErrorBundlerFund`.
    BundlerFund,
    /// Spec 102: the account cannot sign here — the line is
    /// [`SendView::tx_venue_block`]'s reason (`VenueBlock::key()`, with its
    /// domains), not a generic failure.
    VenueBlocked,
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
    /// `None` for the native coin — what tells a sweep's coins apart.
    token_address: Option<String>,
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
/// What the pre-check's treasury half concluded (spec 098 §2). `Unknown` — the
/// relay did not answer — is `Serves`: transient, and a submission that then
/// fails re-probes, as it always has.
#[derive(Clone, Debug, PartialEq)]
enum TreasuryAnswer {
    Serves,
    LowFloat(SendTreasuryStatus),
    Unreachable,
}

impl TreasuryAnswer {
    fn of(probe: &SendTreasuryProbe) -> Self {
        match probe {
            SendTreasuryProbe::LowFloat { status } => Self::LowFloat(status.clone()),
            SendTreasuryProbe::Uncovered => Self::Unreachable,
            SendTreasuryProbe::Covered | SendTreasuryProbe::Unknown => Self::Serves,
        }
    }
}

/// The open treasury sheet's watch (spec 098 §4): the timer in flight, then
/// the probe it started. Ids, so a late answer to a sheet that has since
/// closed changes nothing.
#[derive(Clone, Debug, PartialEq)]
struct TreasuryWatch {
    chain_id: u32,
    timer_id: u64,
    probe_id: Option<u64>,
}

/// How often the open treasury sheet asks the relay again (spec 098 §4).
pub const TREASURY_WATCH_MS: u32 = 10_000;

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
        treasury: Option<TreasuryAnswer>,
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
    /// confirm.
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
    maybe_sent: bool,
    submit_block: Option<u64>,
    /// The write-ahead's records (RJ1): their ack also clears the POST.
    write_ahead: bool,
}

/// The records written ahead of one submit's POST (spec 082 RJ1).
#[derive(Clone, Debug, PartialEq)]
struct WriteAhead {
    /// The `Pipeline::Submitting` id they belong to.
    pipeline_id: u64,
    user_op_hash: String,
    record_ids: Vec<String>,
    submit_block: Option<u64>,
}

/// What `Max` stands for while it is in force.
#[derive(Clone, Debug, PartialEq)]
enum MaxFill {
    /// Pressed before a quote for the fee coin was in hand. The field stays
    /// blank until the quote ALREADY in flight answers — never a second,
    /// competing estimate: that is what made an early Max take twice as long.
    Waiting,
    /// The exact sendable amount, in token units. The field shows it rounded
    /// on the balance line's ladder ([`max_figure`]); every gate, the confirm
    /// figure and the signed call read this ([`model_token_amount`]).
    Exact(String),
}

/// What [`Event::FeeTokenChanged`] said, and the form's chain when it said
/// it: a contract is an address on ONE chain, and a word about the network
/// the form just left never names this one's coin.
#[derive(Clone, Debug, PartialEq)]
struct FeeCardCoin {
    chain_id: u32,
    token: Option<String>,
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
    /// The recipient came from OUTSIDE the field — a hand-off, a scan, a pick
    /// from the book — rather than being typed into it, so it survives the
    /// trip back to the picker ([`handle_back`]). Typing over it makes it the
    /// person's own again.
    recipient_handed_in: bool,
    /// The network a payment request named while leaving the asset to the
    /// payer (`ethereum:<payee>@<chain>`, no token, no amount — issue #312).
    /// Only holdings on it are listed, offered or selectable.
    request_chain: Option<u32>,
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
    /// `Max` is in force (see [`tap_max`]); `None` once the figure is typed,
    /// the token changes, or a batch mode takes over.
    max_fill: Option<MaxFill>,
    /// The TOKEN figure the ⇄ was last pressed on, kept so pressing it twice
    /// gives back what the person typed.
    ///
    /// A conversion is lossy in one direction: the fiat leg is rounded to two
    /// decimals, and dividing that back by the price gives eighteen digits of
    /// arithmetic noise. Typing `1` and toggling there-and-back left
    /// `1.000293813380387808` on screen — a different amount, in the field a
    /// signature is built from. So the toggle RESTORES rather than recomputes,
    /// and only when the fiat figure is still the one it produced: edit it,
    /// and the conversion is what you get.
    fiat_origin: Option<String>,
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
    /// The person picked the fee coin (078 M-03). Until then `None` is "the
    /// fee machine's choice" and a quote in any coin is its answer; after a
    /// pick, `None` is native, said on purpose.
    fee_coin_chosen: bool,
    /// The fee card's coin in force, as [`Event::FeeTokenChanged`] last said
    /// it — `None` until it has. Read for the fee row's coin alone.
    fee_card_coin: Option<FeeCardCoin>,
    treasury_bootstrap: Option<SendTreasuryStatus>,
    treasury_watch: Option<TreasuryWatch>,
    relay_unreachable: Option<SendRelayUnreachable>,
    lock: ReentryLock,
    /// The ported `sendCancelledRef` intent: every pre-sign hop checks it.
    cancelled: bool,
    tx: SendTxStatus,
    tx_error: Option<SendTxErrorKey>,
    /// The reason behind a [`SendTxErrorKey::VenueBlocked`]; read only while
    /// `tx_error` says so.
    venue_block: Option<crate::signing_venue::VenueBlock>,
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
    /// The failure is the relay never having the op (spec 082 RA4).
    receipt_not_sent: bool,
    /// The submit's reply was lost; cleared when the relay acknowledges.
    receipt_maybe_sent: bool,
    fee_held: bool,
    /// The relay is topping up its gas before it sends (098 follow-up). Not
    /// sticky: the next outcome clears it.
    relay_funding: bool,
    /// The submit result's clock (#D3); `None` until the relay accepted.
    submitted_at_ms: Option<f64>,
    fee_rejected: bool,
    /// The relay refused the submitted op (the tracker's `Rejected`).
    refused: bool,
    /// Why the relay refused the submitted op, when it did and said.
    refusal: Option<super::tx_tracker::RefusalReason>,
    /// [`Event::InFlightOps`]: every operation in flight on this device.
    in_flight: Vec<super::tx_tracker::InFlightOp>,
    recipient_identity: Option<SendRecipientIdentity>,
    recipient_risk: Option<SendRecipientRisk>,
    sim_json: Option<String>,
    /// What the confirm's simulation was last asked — the chain, the account
    /// and the calls byte for byte ([`SimInput`]). A quote priced again while
    /// the confirm is up runs it again only when this moved ([`fee_updated`]).
    sim_input: Option<SimInput>,
    /// `prefetchedAccount.current?.publicKeyHex`.
    public_key_hex: Option<String>,
    pipeline: Pipeline,
    /// The write-ahead of the submit in flight (RJ1), until its verdict.
    write_ahead: Option<WriteAhead>,
    /// A `SubmitUserOp` the person cancelled (✕ while "signing") before its
    /// records were written — its result still owed. `OpSigned` names no
    /// submit, so while one is owed none is cleared to POST: the cancelled
    /// payment never goes out, and its late `OpSigned` can never ride a
    /// retry's submit (082 second review).
    withdrawn_submit: Option<u64>,
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

/// What one field of a split row still needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendRowFieldState {
    Ok,
    /// Nothing typed yet. Not a mistake — the row is unfinished, and a screen
    /// that paints an untouched field red is shouting at someone mid-sentence.
    Empty,
    /// Something typed that is not an address / not an amount that can be
    /// sent: a truncated `0x…`, `1,5`, `1e5`, `0`.
    Invalid,
}

/// One split row that `Continue` will not take, and which of its fields is
/// why ([`split_row_issues`]). Rows that are fine are not listed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendSplitRowIssue {
    pub id: String,
    /// 1-based position, the number the row wears ("Recipient 2").
    pub ordinal: u32,
    pub address: SendRowFieldState,
    pub amount: SendRowFieldState,
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

/// One coin the operation sent, summed over its recipients (spec 097 F, S3):
/// a sweep lists each of its coins, a split its one coin's total.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendReceiptCoin {
    /// Token units, as signed.
    pub amount: String,
    pub symbol: String,
    pub logo_urls: Vec<String>,
    /// `None` for the native coin.
    pub token_address: Option<String>,
    pub usd_value: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendReceiptStatus {
    Submitted,
    Confirmed,
    Failed,
    /// The submit's reply was lost and the relay has not yet shown it holds
    /// the op (spec 082 RA10): "It may have been sent. Vela keeps checking —
    /// don't send it again." No Retry, and no success haptic was played.
    MaybeSent,
    /// The relay never had it (spec 082 RA4): "not sent" — never the
    /// fee-rejected words.
    NotSent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SendHoldReason {
    FeeHold,
    FeeRejected,
    /// The relay is topping up the gas it pays with on this chain; it sends
    /// the operation once that lands.
    RelayFunding,
}

/// [`SendView::previous_pending`]: the transaction this send waits for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendPreviousPending {
    pub chain_id: u32,
    pub user_op_hash: String,
    /// The line under the held confirm
    /// ([`super::sign_confirm::PREVIOUS_PENDING_KEY`]).
    pub key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SendReceiptView {
    pub status: SendReceiptStatus,
    pub hold_reason: Option<SendHoldReason>,
    /// The relay refused it: the corpus key of the sentence that says why
    /// (`tx_tracker::RefusalReason::key` — the fee words only for a fee
    /// refusal, "another transaction from this account went first" for a
    /// spent nonce, else the plain "refused, nothing was sent"). Drawn in
    /// place of `hold_reason`'s words. `None` unless refused.
    #[serde(default)]
    pub refusal_key: Option<String>,
    pub kind: Option<SendReceiptKind>,
    pub transfers: Vec<SendReceiptTransfer>,
    /// Every coin the operation sent, in the order signed (spec 097 F, S3).
    /// One for a single send or a split (its total); one per coin for a
    /// sweep. The screen lists these; it never picks one to stand for the
    /// rest.
    pub coins: Vec<SendReceiptCoin>,
    /// The headline figure when the operation sent ONE coin: `coins[0]`'s
    /// amount (a split's total, not its first row). Empty for a sweep of
    /// several coins, which has no one figure.
    pub amount: String,
    /// What everything sent was worth at signing, all coins together.
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
    /// The network the scanned code or link named for the payer to choose an
    /// asset on (issue 312). While set, `tokens` holds only that network's
    /// holdings — empty when the payer has nothing there — and the picker says
    /// which network it is; a shell's own network filter does not apply.
    pub request_chain_id: Option<u32>,
    /// The form's token card opens the asset picker (issue 326): a single
    /// send whose token is the payer's to choose — not a request that named
    /// its token or amount, not a split or a sweep, not while Continue's
    /// pre-check is out.
    pub can_change_token: bool,
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
    /// **Why** the confirm is disabled, when what disabled it is the
    /// money.
    ///
    /// [`SendView::can_confirm`] never looked at the amount at all: a
    /// display-currency commit landing while the confirm page is open
    /// re-denominates the field to empty (`redenominate_to_display`), and the
    /// confirm stayed enabled over a figure that resolved to nothing — a
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
    /// Split mode only: the rows `Continue` will not take, and which field of
    /// each is why ([`split_row_issues`]). Empty exactly when the rows pass the
    /// gate — it IS the gate's reason, so a shell never re-derives the address
    /// or amount rule to explain a dark button.
    pub split_row_issues: Vec<SendSplitRowIssue>,
    /// Split mode only: the balance less the rows' sum, in token units —
    /// "how much is left to give out". `None` while a row cannot be summed or
    /// the sum is over the balance (`split_over_balance` says that instead).
    /// The fee is NOT held back: for the native coin the pre-check still has
    /// the last word, and this figure never promises otherwise.
    pub split_remaining: Option<String>,
    /// How many more recipients an import may add before the cap: the cap less
    /// the rows already started (blank rows do not count — an import drops
    /// them). The shell opens the importer with this as ITS cap.
    pub split_import_room: u32,
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
    /// The coin the form's fee row wears, whether or not a figure is beside
    /// it. The four shells each had their own answer for the frames with no
    /// estimate — a quote out, a quote that failed, a speed being measured —
    /// and drew one state three ways (an empty disc, the chain's coin, the
    /// chosen coin). In order:
    ///
    /// 1. the estimate in hand — this speed's own when it has one, otherwise
    ///    the speed just left's, which this machine keeps across a speed
    ///    change and which names the coin that will pay (the coin does not
    ///    change with the speed; the fee machine's `keep_quote_coin` keeps a
    ///    quote on screen in the coin in force);
    /// 2. the coin in force — the fee card's (`FeeView.fee_token`, mirrored by
    ///    [`Event::FeeTokenChanged`] under its bridge rule, and only when it
    ///    was said about this chain), else the person's pick on this form;
    ///    named by the form's holdings;
    /// 3. the chain's own coin;
    ///
    /// on the selected token's chain, else the sweep's, else the estimate's.
    /// `None` only while no chain is known. The figure is not here: the row
    /// still shows only this speed's own (issue 681).
    #[serde(default)]
    pub fee_coin: Option<SendFeeCoin>,
    pub amount_warning: Option<SendAmountWarning>,
    pub same_asset_fee_issue: Option<SendFeeIssueView>,
    /// The form's button gate — the whole of it. While a relay stop is up the
    /// button is that stop's retry, and pressing it sends [`Event::Continue`]
    /// all the same; a shell draws this flag and adds no reason of its own
    /// (issue 424).
    pub can_continue: bool,
    /// The confirm gate, the whole of it: fee settled ∧ nothing
    /// re-quoting ∧ no same-asset breach ∧ idle ∧ no signature under way ∧ no
    /// refused submit ∧ no relay stop up. [`Event::SlideConfirm`] refuses on
    /// the same predicate; a shell adds nothing to it (issue 424).
    pub can_confirm: bool,
    /// The account's previous transaction on this network is still going
    /// through, so the confirm is held (part of `can_confirm`): this one
    /// would take the same nonce. Drawn as its `key` under the confirm — the
    /// one line for as long as it holds, unchanged while the fee re-measures,
    /// no countdown; the confirm opens by itself once the first is final or
    /// has made no progress for ten minutes
    /// ([`super::tx_tracker::IN_FLIGHT_STALL_MS`]). `None` otherwise.
    #[serde(default)]
    pub previous_pending: Option<SendPreviousPending>,
    pub sending: bool,
    pub tx_status: SendTxStatus,
    pub tx_error: Option<SendTxErrorKey>,
    /// Spec 102: with `tx_error` = [`SendTxErrorKey::VenueBlocked`], why this
    /// account cannot sign here — drawn as `VenueBlock::key()` with its
    /// domains. `None` otherwise.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional = nullable))]
    pub tx_venue_block: Option<crate::signing_venue::VenueBlock>,
    pub tx_hash: Option<String>,
    pub user_op_hash: Option<String>,
    pub receipt: Option<SendReceiptView>,
    pub treasury_bootstrap: Option<SendTreasuryStatus>,
    /// Spec 098 §2: the relay cannot serve this chain; the send stops here.
    pub relay_unreachable: Option<SendRelayUnreachable>,
    /// What the stop's "Report this" files (issue 466). `Some` exactly while
    /// a relay stop is up on a network Vela ships — the stops whose
    /// `operator_served` is true; on a network the person added there is no
    /// operator to tell. The shell snapshots it when the button is pressed:
    /// the stop may close (funded) while the report is being read.
    pub relay_report: Option<SendRelayReport>,
    pub recipient_identity: Option<SendRecipientIdentity>,
    /// Who the money goes to, as the form's recipient line and the confirm
    /// page name them (spec 097 F, S2): the address always, a name only
    /// beside it, with whose word that name is ([`SendPayee`]). One payee
    /// for a single send or a sweep (none until the address is whole); one
    /// per row, in `recipients` order, for a split.
    ///
    /// The confirm's To row used to print `recipient_identity.name` alone:
    /// "Wallet", from the public registry, where anyone can register any
    /// name — the address was one tap away on the identicon. A name never
    /// stands in for the address on the page that signs.
    pub payees: Vec<SendPayee>,
    pub recipient_risk: Option<SendRecipientRisk>,
    /// The recipient is a token's own contract on the network the money moves
    /// on (spec 096 F12): the token being sent, or any token in the person's
    /// list there — the registry's stablecoins and wrapped coin they hold, and
    /// tokens they added. A token contract almost never has a way to give
    /// back what is sent to it, so the form and the confirm page say so
    /// plainly before the confirm; it does not block. Not asked of a split's
    /// rows.
    pub recipient_is_token_contract: bool,
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
        let cmd = Self::step(event, model);
        // Whatever the event, a relay stop never outlives the chain it is
        // about (issue #422).
        drop_foreign_relay_stops(model);
        cmd
    }

    fn view(&self, model: &Model) -> SendView {
        Self::project(model)
    }
}

impl Send {
    /// One event, before the relay-stop guard [`App::update`] runs after it.
    fn step(event: Event, model: &mut Model) -> Cmd {
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
            Event::HoldingsUpdated { tokens } => holdings_updated(model, tokens),
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
                // Typed over, a handed-in recipient is the person's own: Back
                // clears it like any other typing. An echo of the same value
                // changes nothing.
                if recipient != model.recipient {
                    model.recipient_handed_in = false;
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
                // A typed figure is the person's, so Max is over.
                model.max_fill = None;
                model.amount = model.amount.with_value(amount);
                Command::all([schedule_form_estimate(model), render()])
            }
            Event::ToggleFiatInput => toggle_fiat_input(model),
            Event::TapMax => tap_max(model),
            Event::EnterSplitMode => enter_split_mode(model),
            Event::SeedSplitRecipients { recipients } => seed_split(model, recipients),
            Event::AppendSplitRecipients { recipients } => append_split(model, recipients),
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
            Event::Continue => {
                // On the form a relay stop's button IS Continue (spec 098):
                // pressing it asks the relay again, so the stop comes down
                // first. Issue #424: Continue used to leave it up, and a check
                // that then passed carried the old stop onto the confirm page.
                // One event for the form's one button, whatever it says — no
                // shell has to know which stop is up to press it.
                if model.step == SendStep::EnterDetails {
                    clear_relay_stops(model);
                }
                handle_continue(model)
            }
            Event::Back => handle_back(model),
            Event::ChangeToken => change_token(model),
            Event::EditAmount => edit_amount(model),
            Event::ChooseFeeToken { token } => {
                model.gas_fee_token = token;
                model.fee_coin_chosen = true;
                // The pick is newer than the card's last word: it names the
                // row's coin until the card says what it took up.
                model.fee_card_coin = None;
                // On the form the fee coin is part of what the quote is about
                // — and so is a Max, which holds back a fee only in the coin
                // being sent.
                Command::all([schedule_form_estimate(model), follow_max(model), render()])
            }
            Event::FeeUpdated { estimate } => fee_updated(model, estimate),
            Event::FeeBusyChanged { busy } => {
                model.fee_busy = busy;
                render()
            }
            Event::FeeTokenChanged { fee_token } => {
                model.fee_card_coin = form_chain(model).map(|chain_id| FeeCardCoin {
                    chain_id,
                    token: fee_token,
                });
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
            Event::OpSigned {
                user_op_hash,
                submit_block,
                now_ms,
            } => op_signed(model, user_op_hash, submit_block, now_ms),
            Event::CancelSigning => cancel_signing(model),
            Event::RetryAfterBootstrap => retry_after_bootstrap(model),
            Event::DismissTreasurySheet => {
                model.treasury_bootstrap = None;
                model.treasury_watch = None;
                render()
            }
            Event::RetryRelayUnreachable => {
                clear_relay_stops(model);
                match model.step {
                    SendStep::EnterDetails => handle_continue(model),
                    SendStep::Confirm => slide_confirm(model),
                    SendStep::SelectToken => render(),
                }
            }
            Event::DismissRelayUnreachable => {
                model.relay_unreachable = None;
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
            Event::InFlightOps { ops } => {
                if model.in_flight == ops {
                    return Command::done();
                }
                model.in_flight = ops;
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

    /// The view [`App::view`] publishes.
    fn project(model: &Model) -> SendView {
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
        // The split's gate and its reasons are one computation — asked whenever
        // there IS a split, token or not. A split can exist before a token does
        // (the book hands recipients over while the token list is still out),
        // and "no token, so no issues" would arm `Continue` over blank rows.
        // Eighteen places is the most any row could need; the addresses and the
        // emptiness of an amount do not depend on it at all.
        let split_issues = if model.split_mode {
            let decimals = model
                .selected_token
                .as_ref()
                .map_or(18, |token| token.decimals);
            split_row_issues(&model.recipients, decimals)
        } else {
            Vec::new()
        };
        let can_continue = !model.estimating_gas
            && !(locked && warning.is_some())
            && if model.split_mode {
                !model.recipients.is_empty() && split_issues.is_empty()
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

        let split_remaining = model
            .selected_token
            .as_ref()
            .filter(|_| model.split_mode)
            .and_then(|token| {
                let total = sum_split_base_units(&model.recipients, token.decimals)?;
                let balance = to_base_units(&full_balance(token), token.decimals)?;
                balance
                    .checked_sub(total)
                    .map(|left| from_base_units(left, token.decimals))
            });

        // The confirm's gate — and the same amount question `Continue`
        // asks, which this twin never asked.
        //
        // Everything it checked was about the FEE and the pipeline; the money
        // itself was never re-examined after `Continue`. But the confirm page
        // is a page someone can sit on, and a `display_changed` commit landing
        // underneath re-denominates the field to empty
        // (`redenominate_to_display`) — leaving a confirm enabled over a figure
        // that resolves to nothing. Confirming signed a zero-value transfer
        // with no warning anywhere. The batch modes carry their money in
        // `recipients`/`multi_specs`, not in `model.amount`, so they are asked
        // the same question `can_continue` asks them.
        let confirm_amount_ok = model.split_mode || model.multi_select_mode || amount_resolves;
        // The whole gate, here and only here (issue #424): the shells used to
        // AND their own reasons onto it — a relay stop, a signature under way,
        // a refused submit — and did not all AND the same ones, so one client
        // enabled a confirm another held. `slide_confirm` refuses on the same
        // predicate.
        let can_confirm = stage == SendStage::Confirm
            && confirm_gate_open(model)
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
            request_chain_id: model.request_chain,
            can_change_token: can_change_token(model),
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
            split_row_issues: split_issues,
            split_remaining,
            split_import_room: split_import_room(model),
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
            gas_fee_token: quoted_fee_token(model, selected_fee(model)),
            fee_coin: fee_coin(model),
            amount_warning: warning,
            same_asset_fee_issue: issue,
            can_continue,
            can_confirm,
            previous_pending: (stage == SendStage::Confirm)
                .then(|| previous_in_flight(model))
                .flatten()
                .map(|op| SendPreviousPending {
                    chain_id: op.chain_id,
                    user_op_hash: op.user_op_hash.clone(),
                    key: super::sign_confirm::PREVIOUS_PENDING_KEY.to_owned(),
                }),
            sending: model.lock.busy(),
            tx_status: model.tx,
            tx_error: model.tx_error,
            tx_venue_block: model
                .venue_block
                .clone()
                .filter(|_| model.tx_error == Some(SendTxErrorKey::VenueBlocked)),
            tx_hash: model.tx_hash.clone(),
            user_op_hash: model.user_op_hash.clone(),
            receipt: receipt_view(model, stage),
            treasury_bootstrap: model.treasury_bootstrap.clone().map(|mut status| {
                status.operator_served = super::network_admin::is_builtin_chain(status.chain_id);
                status.coin = treasury_coin(model, &status);
                status
            }),
            relay_unreachable: model.relay_unreachable.clone().map(|mut sheet| {
                sheet.operator_served = super::network_admin::is_builtin_chain(sheet.chain_id);
                sheet
            }),
            relay_report: relay_report(model),
            recipient_identity: model.recipient_identity.clone(),
            payees: payees(model),
            recipient_risk: model.recipient_risk.clone(),
            recipient_is_token_contract: recipient_is_token_contract(model),
            sim_json: model.sim_json.clone(),
        }
    }
}

/// [`SendView::payees`]. A split row's name is the person's own word (their
/// contact's name, a list's name column); the single recipient's comes from
/// the identity the shell resolved, and says where from.
fn payees(model: &Model) -> Vec<SendPayee> {
    if model.split_mode {
        return model
            .recipients
            .iter()
            .map(|row| {
                let name = row
                    .name
                    .as_deref()
                    .map(str::trim)
                    .filter(|name| !name.is_empty());
                SendPayee {
                    address: row.address.trim().to_owned(),
                    name: name.map(str::to_owned),
                    name_source: name.map(|_| SendNameSource::Own),
                }
            })
            .collect();
    }
    let address = model.recipient.trim();
    if !is_valid_address(address) {
        return Vec::new();
    }
    let identity = model.recipient_identity.as_ref();
    let name = identity
        .and_then(|identity| identity.name.as_deref())
        .map(str::trim)
        .filter(|name| super::valid_display_name(name));
    let source = identity.and_then(|identity| name_source_of(identity.source.as_deref()));
    // A name with no say of where it came from is not drawn: it could only be
    // drawn untagged, which is how the person's own names look.
    let (name, name_source) = match (name, source) {
        (Some(name), Some(source)) => (Some(name.to_owned()), Some(source)),
        _ => (None, None),
    };
    vec![SendPayee {
        address: address.to_owned(),
        name,
        name_source,
    }]
}

/// The resolver's source label, read: `self` is one of the person's own
/// accounts, `passkey` the public wallet registry, anything else a name
/// service's label. Blank says nothing.
fn name_source_of(source: Option<&str>) -> Option<SendNameSource> {
    match source.map(str::trim)? {
        "" => None,
        "self" => Some(SendNameSource::Own),
        "passkey" => Some(SendNameSource::Registry),
        label => Some(SendNameSource::Service {
            label: label.to_owned(),
        }),
    }
}

/// [`SendView::recipient_is_token_contract`]: the single recipient (a
/// one-to-one send or a sweep) is, on the selected token's network, the
/// contract of the token being sent or of any token the person's list
/// holds there. Compared as addresses, case aside.
fn recipient_is_token_contract(model: &Model) -> bool {
    if model.split_mode {
        return false;
    }
    let Some(token) = model.selected_token.as_ref() else {
        return false;
    };
    let recipient = model.recipient.trim();
    if !is_valid_address(recipient) {
        return false;
    }
    let names_it = |held: &SendToken| {
        held.token_address
            .as_deref()
            .is_some_and(|contract| contract.trim().eq_ignore_ascii_case(recipient))
    };
    names_it(token)
        || model
            .tokens
            .iter()
            .filter(|held| held.chain_id == token.chain_id)
            .any(names_it)
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

/// A hand-off that names the TOKEN steps to the form before the list
/// answers. A recipient alone does not (issue #312): which asset to send is the
/// payer's to choose, on the picker, with the recipient already shown there
/// (issue #332) — the form used to open on the balance's top token on
/// whatever chain it was, which for a code from a BNB Chain wallet was XDAI on
/// Gnosis.
fn has_preselection(params: &SendOpenParams) -> bool {
    params.preselected_multi.is_some()
        || (params.preselected_symbol.is_some() && params.preselected_network.is_some())
}

/// The network a locked request names while leaving the asset open: a chain,
/// and neither a token nor an amount (`ethereum:<payee>@<chain>`). That is the
/// shape of a "pay me on this network" code, and an amount-less native request
/// cannot be told apart from it — so it is read the way the payer can act on
/// it: any of their holdings on that network, never a coin they do not hold
/// on it, and never another network (issue #312). A request that names a
/// token or an amount stays exactly that request.
fn request_network(params: &SendOpenParams) -> Option<u32> {
    if !params.locked
        || params.prefilled_token_address.is_some()
        || params.prefilled_amount_base.is_some()
    {
        return None;
    }
    params
        .prefilled_chain_id
        .as_deref()
        .and_then(parse_int_prefix)
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
    // wait on the token list to show who the money is for. The picker shows
    // it while the list loads (issue #332), and keeps it if the load fails.
    model.recipient = params.prefilled_recipient.clone().unwrap_or_default();
    model.recipient_handed_in = params.prefilled_recipient.is_some();
    model.request_chain = request_network(&params);
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

/// Every token list the shell hands over, with each built-in chain's coin
/// written the wallet's way (`network_admin::display_native_symbol`) — the
/// same spelling the asset list shows, so a token's id (`network_address_symbol`)
/// is one id whichever door the list came through.
fn with_display_symbols(tokens: Vec<SendToken>) -> Vec<SendToken> {
    tokens
        .into_iter()
        .map(|mut token| {
            token.symbol = super::network_admin::display_native_symbol(
                token.chain_id,
                token.token_address.as_deref(),
                &token.symbol,
            );
            token
        })
        .collect()
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

/// What the picker lists: the non-zero holdings, highest value first — and,
/// when a request named a network for the payer to choose on, only that
/// network's (issue #312). Every reader of `model.tokens` (the list, a pick,
/// a sweep, the fee read-ahead) sees the same narrowing.
fn offered_tokens(model: &Model, tokens: &[SendToken]) -> Vec<SendToken> {
    let mut list = non_zero_sorted(tokens);
    if let Some(chain_id) = model.request_chain {
        list.retain(|token| token.chain_id == chain_id);
    }
    list
}

fn tokens_partial(model: &mut Model, tokens: Vec<SendToken>) -> Cmd {
    let tokens = with_display_symbols(tokens);
    // Progressive display only, and only while a load is actually running.
    if model.flights.tokens.is_none() {
        return Command::done();
    }
    model.tokens = offered_tokens(model, &tokens);
    model.loading = false;
    render()
}

/// The asset list moved (see [`Event::HoldingsUpdated`]): the picker always
/// follows; on the form, so do the selected token's balance and price and a
/// Max that depends on them.
///
/// Only on the form. A confirm page is about the token as it was confirmed —
/// a Max refilled from a new balance there would change the signed amount
/// under a person who has already read it — and a receipt is history.
///
/// A first load still in flight owns the list — its answer is what a locked
/// request and a hand-off are matched against — so an update meanwhile is
/// left to it. A token the update no longer lists keeps the row it had: a
/// chain that did not answer this round is not an emptied balance.
fn holdings_updated(model: &mut Model, tokens: Vec<SendToken>) -> Cmd {
    let tokens = with_display_symbols(tokens);
    if matches!(model.flights.tokens, Some((_, TokensPurpose::Initial))) {
        return Command::done();
    }
    model.tokens = offered_tokens(model, &tokens);
    if model.step != SendStep::EnterDetails {
        return render();
    }
    if let Some(selected) = model.selected_token.as_ref() {
        let id = selected.id();
        if let Some(fresh) = tokens.iter().find(|t| t.id() == id) {
            if fresh != selected {
                model.selected_token = Some(fresh.clone());
            }
        }
    }
    Command::all([follow_max(model), render()])
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
    let Some(full) = tokens.map(with_display_symbols) else {
        model.loading = false;
        return match purpose {
            // `catch(() => showAlert(...))`.
            TokensPurpose::Initial => alert(model, SendAlertKind::LoadTokensFailed),
            // `refreshTokens`'s `.catch(() => {})`.
            TokensPurpose::Refresh => render(),
        };
    };
    model.tokens = offered_tokens(model, &full);
    model.loading = false;
    if purpose == TokensPurpose::Refresh {
        // A refresh answers the same question an asset-list update does.
        return holdings_updated(model, full);
    }

    if model.params.locked {
        return resolve_locked_request(model, &full);
    }

    // Multi-token hand-off via params → land in multiSelect mode.
    if let Some(joined) = model.params.preselected_multi.clone() {
        let wanted: Vec<&str> = joined.split(',').collect();
        // Ids carry the symbol, and a hand-off may still spell a chain's coin
        // the chain document's way ("XDAI") where the list now says "xDAI".
        let picked: Vec<SendToken> = model
            .tokens
            .iter()
            .filter(|t| wanted.iter().any(|w| w.eq_ignore_ascii_case(&t.id())))
            .cloned()
            .collect();
        if let Some(first) = picked.first().cloned() {
            model.multi_selected_ids = picked.iter().map(|t| t.id()).collect();
            model.multi_chain_id = Some(first.chain_id);
            model.multi_select_mode = true;
            model.max_fill = None;
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
            .find(|t| t.symbol.eq_ignore_ascii_case(&symbol) && t.network == network)
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

    // A recipient handed in without a token — a scanned address, a contact —
    // lands on the picker with the recipient on it: the asset is the payer's
    // to choose (issue #312). The old quick-send took the balance's top token
    // for them, on whichever chain it lived.
    if model.params.prefilled_recipient.is_some() {
        return Command::all([sync_identity(model), picker_without_a_token(model)]);
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
        return Command::all([prewarm_fees(model), render()]);
    }
    render()
}

/// The picker is open: read ahead the fees of the chains the person holds
/// value on, highest first, so the quote a pick starts finds its reads done.
fn prewarm_fees(model: &mut Model) -> Cmd {
    let Some(account) = model.account.as_ref().map(|a| a.address.clone()) else {
        return Command::done();
    };
    let mut chain_ids: Vec<u32> = Vec::new();
    for token in model.tokens.iter().filter(|t| !t.spam) {
        if !chain_ids.contains(&token.chain_id) {
            chain_ids.push(token.chain_id);
        }
    }
    chain_ids.truncate(PREWARM_CHAINS);
    if chain_ids.is_empty() {
        return Command::done();
    }
    let id = next(model);
    issue(id, SendOperation::PrewarmFees { account, chain_ids })
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

    // A network named with the asset left open (issue #312): what the payer
    // holds there, and only there. One holding is the obvious choice and opens
    // the form on it; several are the picker's, narrowed to that network; none
    // is the same picker, empty, saying which network it is — never a coin
    // they do not hold, and never another network.
    if model.request_chain.is_some() {
        model.lock_error = None;
        model.resolving_lock = false;
        if let [only] = model.tokens.as_slice() {
            let only = only.clone();
            return finish_lock_resolution(model, only);
        }
        model.step = SendStep::SelectToken;
        return Command::all([sync_identity(model), prewarm_fees(model), render()]);
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
            model.max_fill = None;
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
            // Re-run resolution now that the chain exists (`lockRetry`) —
            // resolving again, as at open: without it the stage fell through
            // to an EMPTY form (no token, no amount) for as long as the fresh
            // token load took, and a failed load left it there (078 W-04).
            model.lock_error = None;
            model.resolving_lock = true;
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
                               // A fee coin picked for another token's form is a contract on that
                               // token's chain; carried across, it priced this send in a coin the chain
                               // may not have. The new form starts from the fee machine's own pick.
    model.gas_fee_token = None;
    model.fee_coin_chosen = false;
    model.max_fill = None;
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
    model.max_fill = None;
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
    if let Some(MaxFill::Exact(exact)) = &model.max_fill {
        return exact.clone();
    }
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
    model.max_fill = None; // the field empties, and a Max behind it with it
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
    // Max stays Max across the swap: the exact amount is still what is sent,
    // only the figure changes denomination — restated from the exact amount,
    // never from the rounded figure on the field.
    if let Some(MaxFill::Exact(exact)) = model.max_fill.clone() {
        model.fiat_origin = None;
        model.amount = if target.is_fiat() {
            DenominatedAmount::token(exact)
                .convert(
                    &target,
                    price.as_ref(),
                    token.decimals,
                    model.display.fiat_decimals,
                )
                .unwrap_or_else(|_| DenominatedAmount::token(""))
        } else {
            DenominatedAmount::token(max_figure(&exact))
        };
        return render();
    }
    let converted = model
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

    if target.is_fiat() {
        // Going IN: remember what this fiat figure was made from.
        model.fiat_origin = Some(model.amount.value().to_owned());
        model.amount = converted;
        return render();
    }

    // Coming OUT. If the fiat figure is still exactly the one the remembered
    // token amount produces, the person did not change it — so give back the
    // digits they typed rather than the division's remainder.
    let untouched = model.fiat_origin.as_ref().is_some_and(|origin| {
        DenominatedAmount::token(origin.clone())
            .convert(
                &model.display.denom(),
                price.as_ref(),
                token.decimals,
                model.display.fiat_decimals,
            )
            .is_ok_and(|round_trip| round_trip.value() == model.amount.value())
    });
    model.amount = match (untouched, model.fiat_origin.take()) {
        (true, Some(origin)) => DenominatedAmount::token(origin),
        _ => converted,
    };
    render()
}

fn tap_max(model: &mut Model) -> Cmd {
    // `Max` fills the SINGLE amount, and neither batch mode has one: the rows
    // are the amounts. It used to write that hidden field anyway (and could
    // start a fee estimate for it), so the button changed nothing a person
    // could see and left a stale figure behind for `amount_warning` to keep
    // judging.
    //
    // A SWEEP is the same refusal for a stronger reason: its rows are already
    // each token's maximum. `multi_token_specs` gives every picked token its
    // whole balance less the reserve whichever asset pays the fee needs, and
    // recomputes it on every quote — there is no per-row figure for a Max to
    // fill and no second arithmetic to run. What the event DID do there was
    // act on `selected_token`, the first token of the pick: it rewrote the
    // hidden single amount of one row (or started a rival `EstimateFee` for
    // it) and `derive_amount_warning`, which no batch mode escapes, then
    // painted a verdict about that token over a form that shows several.
    if model.split_mode || model.multi_select_mode {
        return Command::done();
    }
    // A Continue in flight owns the form (078 M-01), as it owns the pipeline
    // slot `schedule_form_estimate` defers to. Max used to take that slot for
    // its own estimate: during the pre-check the check's answer then found
    // nobody waiting and `estimating_gas` stayed set — Continue read
    // "Estimating…" for good — and during the credential load the Continue
    // was silently dropped. The figure under check stays the figure, and Max
    // works again once the check has answered.
    if matches!(
        model.pipeline,
        Pipeline::ContinueCredential { .. } | Pipeline::PreCheck { .. }
    ) {
        return Command::done();
    }
    let Some(token) = model.selected_token.clone() else {
        return Command::done();
    };
    // Max always fills in token units. Every exit below writes a token
    // figure; the one that waits for an estimate leaves the field blank
    // meanwhile rather than re-labelling whatever was typed before.
    model.amount = DenominatedAmount::token("");
    model.fiat_origin = None;
    model.max_fill = Some(MaxFill::Waiting);
    max_follow_up(model, &token)
}

/// Bring a Max in force up to date with what is in hand: fill it from the
/// quote for the fee coin in force, wait for the quote already on its way, or
/// — only when nothing is on its way — ask for one.
///
/// The tap calls it, and so does everything a Max depends on: every quote
/// that lands, a fee-coin switch, a balance refresh. A Max used to be a
/// figure written once; after the fee machine picked another coin, or the
/// balance moved, it was a stale number the gates then refused.
fn max_follow_up(model: &mut Model, token: &SendToken) -> Cmd {
    if model.max_fill.is_none() {
        return Command::done();
    }
    // Only a quote for the fee coin chosen NOW (078 M-03): after a switch the
    // quote in hand priced the other coin, and its reserve is a figure about
    // an asset this send no longer pays in.
    if let Some(fee) = max_quote(model).cloned() {
        set_max(model, max_with_fee(token, &fee));
        return render();
    }
    let Some(account) = model
        .account
        .as_ref()
        .map(|a| a.address.clone())
        .filter(|_| token.is_native() || token.token_address.is_some())
    else {
        set_max(model, full_balance(token));
        return render();
    };
    // A quote is already on its way — the warm-up the token pick started, or
    // the form's own. It prices this transfer; wait for it rather than race
    // it with a second estimate, which is what made an early Max twice as slow
    // (the first answer then found nobody waiting and was thrown away).
    if matches!(
        model.pipeline,
        Pipeline::WarmCredential { .. }
            | Pipeline::WarmEstimate { .. }
            | Pipeline::FormDebounce { .. }
            | Pipeline::FormEstimate { .. }
            | Pipeline::MaxEstimate { .. }
    ) {
        return render();
    }
    if model.pipeline != Pipeline::Idle {
        // A pre-check or a submit owns the slot and brings its own quote.
        return render();
    }
    // Nothing on its way → estimate on demand, like `handleMaxAmount`'s
    // `await estimateTransactionFee(...)` — of the transfer Max is about to
    // fill (078 M-02), not a placeholder call.
    let tx = max_estimate_call(model, token, &account);
    let id = next(model);
    model.pipeline = Pipeline::MaxEstimate { id };
    Command::all([
        issue(
            id,
            SendOperation::EstimateFee {
                chain_id: token.chain_id,
                account,
                tx,
                batch: None,
                gas_fee_token: model.gas_fee_token.clone(),
                public_key_hex: model.public_key_hex.clone(),
                auto_fee_token: !model.fee_coin_chosen,
            },
        ),
        render(),
    ])
}

/// [`max_follow_up`] for whatever token is selected — the form after a quote
/// or a balance lands.
fn follow_max(model: &mut Model) -> Cmd {
    match model.selected_token.clone() {
        Some(token) if model.max_fill.is_some() => max_follow_up(model, &token),
        _ => Command::done(),
    }
}

/// Put an exact Max on the field: the figure rounded as the balance line
/// rounds, the exact amount behind it for everything that counts.
fn set_max(model: &mut Model, exact: String) {
    model.amount = DenominatedAmount::token(max_figure(&exact));
    model.max_fill = Some(MaxFill::Exact(exact));
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
            auto_fee_token: !model.fee_coin_chosen,
        },
    )
}

/// The transfer `Max` is about to fill, as the call to quote (078 M-02): the
/// payee typed so far — or the account itself, a transfer's gas does not
/// depend on who receives it — and the whole balance for a token. For the
/// chain's coin the value is ONE wei: the call takes the value-bearing path a
/// real transfer takes, without asking a simulation to move the whole balance
/// the fee must also come out of. `None` when the call cannot be encoded; the
/// executor then quotes its rough shape, as before.
fn max_estimate_call(model: &Model, token: &SendToken, account: &str) -> Option<FeeCall> {
    let payee = if is_valid_address(&model.recipient) {
        model.recipient.trim()
    } else {
        account
    };
    match token.token_address.as_deref() {
        None => Some(FeeCall {
            to: payee.to_owned(),
            value: "1".to_owned(),
            data: "0x".to_owned(),
        }),
        Some(addr) => {
            let units = to_base_units(&full_balance(token), token.decimals)?;
            Some(FeeCall {
                to: addr.to_owned(),
                value: "0".to_owned(),
                data: encode_erc20_transfer(payee, units)?,
            })
        }
    }
}

/// Whether a quote priced the fee coin chosen NOW (078 M-03): a picked token,
/// that token; a picked native, the chain's coin. Nothing picked yet, the
/// fee machine's own choice — whatever coin its quote is in.
fn quote_is_for_fee_coin(model: &Model, fee: &FeeEstimate) -> bool {
    match (&model.gas_fee_token, &fee.fee_asset) {
        (Some(chosen), FeeAsset::Erc20 { token, .. }) => chosen.eq_ignore_ascii_case(token),
        (Some(_), FeeAsset::Native) => false,
        (None, FeeAsset::Native) => true,
        (None, FeeAsset::Erc20 { .. }) => !model.fee_coin_chosen,
    }
}

/// The quote `Max` and its sentence may reserve against: on this chain, AND
/// for this fee coin.
fn max_quote(model: &Model) -> Option<&FeeEstimate> {
    selected_fee(model).filter(|fee| quote_is_for_fee_coin(model, fee))
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
/// One rule, two readers: the fill ([`max_with_fee`]) and the sentence
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

/// The exact Max given a fee (`useSendController.ts:801-848`).
fn max_with_fee(token: &SendToken, fee: &FeeEstimate) -> String {
    // Gas paid in native or a separate fee asset — full balance sendable.
    let Some(reserve) = max_fee_reserve(token, fee) else {
        return full_balance(token);
    };
    match to_base_units(&token.balance, token.decimals) {
        // String-exact `balance − reserve`: `to_base_units(result) + reserve
        // == balance`, so the gas pre-check never trips on its own Max fill
        // (invariant ⑨). A reserve at or above the balance answers `"0"`, and
        // `derive_amount_warning` is what says so out loud.
        Some(balance) => max_native_sendable(balance, reserve, token.decimals),
        // TS `balanceToWei` would throw → catch → full balance.
        None => full_balance(token),
    }
}

/// How a Max reads on the field: the exact amount on the ladder the balance
/// line uses (`l10n::number::format_token_amount` — 2 places from 1000, 4
/// from 1, 6 below), half up, trailing zeros dropped, so the figure Max
/// writes and the balance above it agree digit for digit. A figure too small
/// to survive six places keeps two significant digits instead of reading `0`.
///
/// Display only. The exact amount stays in [`MaxFill::Exact`] and is what
/// every gate and the signed call read: `0.043790209243313861` — balance less
/// a fee to the wei — used to be written onto the field whole, and ran off
/// both sides of it on every shell.
pub fn max_figure(exact: &str) -> String {
    let exact = exact.trim();
    let (int_raw, frac_raw) = exact.split_once('.').unwrap_or((exact, ""));
    if int_raw.is_empty() && frac_raw.is_empty()
        || !int_raw.bytes().all(|b| b.is_ascii_digit())
        || !frac_raw.bytes().all(|b| b.is_ascii_digit())
    {
        return exact.to_owned();
    }
    let int_digits = int_raw.trim_start_matches('0');
    let places = match int_digits.len() {
        0 => 6,
        1..=3 => 4,
        _ => 2,
    };
    let mut digits: Vec<u8> = int_digits
        .bytes()
        .chain(frac_raw.bytes().chain(std::iter::repeat(b'0')).take(places))
        .map(|b| b - b'0')
        .collect();
    let round_up = frac_raw.as_bytes().get(places).is_some_and(|b| *b >= b'5');
    if round_up {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, 1);
                break;
            }
            i -= 1;
            if digits[i] == 9 {
                digits[i] = 0;
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let split = digits.len() - places;
    let int_part: String = digits[..split]
        .iter()
        .map(|d| char::from(b'0' + d))
        .collect();
    let frac_part: String = digits[split..]
        .iter()
        .map(|d| char::from(b'0' + d))
        .collect();
    let int_part = if int_part.is_empty() {
        "0".to_owned()
    } else {
        int_part
    };
    let frac_part = frac_part.trim_end_matches('0');
    if int_part == "0" && frac_part.is_empty() {
        // Below the ladder's last place: two significant digits, cut — never
        // a bare zero for an amount that is not one.
        let lead = frac_raw.bytes().take_while(|b| *b == b'0').count();
        if lead == frac_raw.len() {
            return "0".to_owned();
        }
        let keep = (lead + 2).min(frac_raw.len());
        return format!("0.{}", frac_raw[..keep].trim_end_matches('0'));
    }
    if frac_part.is_empty() {
        int_part
    } else {
        format!("{int_part}.{frac_part}")
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
    // Entering a split twice must not be a way to lose one. This rebuilt the
    // rows as `[the single recipient, blank]` every time it was called, so a
    // shell whose "+ add recipient" sends this event while ALREADY in a split
    // wiped forty imported people with one press.
    if split_locked_out(model) || model.split_mode {
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
    model.max_fill = None;
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
    model.max_fill = None;
    model.amount = DenominatedAmount::token("");
    model.recipients = rows;
    model.split_mode = true;
    model.show_batch_import = false;
    model.show_contact_picker = false;
    render()
}

/// A row the person has started: an address or an amount in it. Blank rows are
/// scaffolding — the split opens with one — and are not carried past an import.
fn row_is_started(row: &SendRecipientDraft) -> bool {
    !row.address.trim().is_empty() || !row.amount.trim().is_empty()
}

/// How many more recipients an import may add: the cap less the rows already
/// started. The importer is opened with this as its own cap, so its "only the
/// first N will be sent" is true of what [`append_split`] then does.
fn split_import_room(model: &Model) -> u32 {
    let started = if model.split_mode {
        model
            .recipients
            .iter()
            .filter(|r| row_is_started(r))
            .count()
    } else {
        usize::from(!model.recipient.trim().is_empty() || !model.amount.is_empty())
    };
    BATCH_MAX_RECIPIENTS.saturating_sub(started) as u32
}

/// [`Event::AppendSplitRecipients`]: the started rows stay, in their order and
/// with their ids; the blank ones go; the new rows follow. From the single
/// form, the one recipient being typed becomes the first row, exactly as
/// `enter_split_mode` would have made it.
///
/// Nothing is de-duplicated here. The importer already dropped repeats WITHIN
/// the list it read; a new row that repeats a typed one is named by
/// [`duplicate_recipient_rows`] like any other repeat — warned, never dropped,
/// because both were entered on purpose.
fn append_split(model: &mut Model, rows: Vec<SendRecipientDraft>) -> Cmd {
    if rows.is_empty() || split_locked_out(model) {
        return Command::done();
    }
    let mut kept: Vec<SendRecipientDraft> = if model.split_mode {
        model
            .recipients
            .iter()
            .filter(|r| row_is_started(r))
            .cloned()
            .collect()
    } else if !model.recipient.trim().is_empty() || !model.amount.is_empty() {
        let amount = match model.selected_token.clone() {
            Some(token) if !model.amount.is_empty() => model_token_amount(model, &token),
            _ => String::new(),
        };
        vec![SendRecipientDraft {
            id: make_recipient_id(model),
            address: model.recipient.clone(),
            amount,
            name: None,
        }]
    } else {
        Vec::new()
    };
    // Incoming ids are not trusted to be unique ACROSS imports: a shell that
    // numbers its rows `b0…bN` sends the same ids the second time, and an id
    // is what the duplicate warning, the contact picker's target and the row
    // issues all point at. Every appended row gets a fresh one.
    for mut row in rows {
        row.id = make_recipient_id(model);
        kept.push(row);
    }
    kept.truncate(BATCH_MAX_RECIPIENTS);
    model.max_fill = None;
    model.amount = DenominatedAmount::token("");
    model.recipients = kept;
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
///
/// Which row, in a split: the one the picker was opened for; with no row
/// named (a split's "from contacts" opens the picker for the split as a
/// whole), the first row with no address yet, else a new row at the end. The
/// single form's recipient is hidden while a split is up, and a pick that
/// went there was a person the screen never showed and the send never paid.
/// The split's cap holds: with every one of its rows taken, nothing is added.
fn apply_picked_address(model: &mut Model, address: String) -> Cmd {
    model.show_contact_picker = false;
    if !model.split_mode {
        return hand_in_recipient(model, address);
    }
    let named = model
        .picker_target
        .as_ref()
        .and_then(|target| model.recipients.iter().position(|row| &row.id == target));
    let slot = named.or_else(|| {
        model
            .recipients
            .iter()
            .position(|row| row.address.trim().is_empty())
    });
    match slot {
        Some(index) => {
            if let Some(row) = model.recipients.get_mut(index) {
                // A row's name is the person's word for whoever its address
                // was (a group member, a list's name column); another address
                // is somebody else. The same address in another letter case
                // (a list's lowercase, the book's checksum) is the same one.
                if !row.address.trim().eq_ignore_ascii_case(address.trim()) {
                    row.name = None;
                }
                row.address = address;
            }
        }
        None if model.recipients.len() < BATCH_MAX_RECIPIENTS => {
            let id = make_recipient_id(model);
            model.recipients.push(SendRecipientDraft {
                id,
                address,
                amount: String::new(),
                name: None,
            });
        }
        None => {}
    }
    render()
}

/// A recipient that arrived from outside the field — a scan, a pick from the
/// book — fills the single recipient and is remembered as handed in, so going
/// back to the picker to change the asset does not throw it away (issue #332:
/// a scan from the home lands on the picker, and Back from the form used to
/// clear the address it had just read).
fn hand_in_recipient(model: &mut Model, address: String) -> Cmd {
    model.recipient = address;
    model.recipient_handed_in = true;
    Command::all([
        sync_identity(model),
        schedule_form_estimate(model),
        render(),
    ])
}

fn scan_resolved(model: &mut Model, scan: SendScan) -> Cmd {
    model.show_scanner = false;
    // The picker's "scan to fill" row opened this scanner, and the scan IS the
    // pick: leaving the picker up put it back over the address it had just
    // filled (issue #270), on every shell. The targeted path below closes it
    // through `apply_picked_address`; a re-lock reopens the flow either way.
    model.show_contact_picker = false;
    // A scan in split mode — just the address, into the row it was for or
    // the row a pick would take; a full-request re-lock would blow away the
    // other recipients (invariant ⑬). The picker's scan row is a pick, so a
    // split's targetless picker scans into the split, never into the hidden
    // single recipient.
    if model.split_mode {
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
        SendScan::Request { recipient, .. } => hand_in_recipient(model, recipient),
        SendScan::Text { data } => hand_in_recipient(model, data),
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
    model.sim_input = None;
    model.recipient_risk = None;
    model.flights.risk = None;
    if model.step != SendStep::Confirm {
        return Command::done();
    }
    let Some(token) = model.selected_token.clone() else {
        return Command::done();
    };
    if model.account.is_none() {
        return Command::done();
    }

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

    if let Some(input) = sim_input(model) {
        cmds.push(simulate(model, input));
    }
    Command::all(cmds)
}

/// What the confirm's simulation is asked about. The shell simulates exactly
/// this — the chain, the account and the calls — and nothing of the fee: not
/// its gas, its limits, its price or its coin.
#[derive(Clone, Debug, PartialEq, Eq)]
struct SimInput {
    chain_id: u32,
    account: String,
    calls: Vec<FeeCall>,
}

/// The simulation this confirm asks for, or `None` when it asks for none: an
/// incomplete or invalid form, or a malformed amount (the `catch`).
fn sim_input(model: &Model) -> Option<SimInput> {
    let token = model.selected_token.as_ref()?;
    let account = model.account.as_ref()?;
    let ok_single =
        !model.split_mode && !model.multi_select_mode && is_valid_address(&model.recipient);
    let ok_split = model.split_mode && recipients_are_valid(&model.recipients);
    let ok_multi = model.multi_select_mode
        && is_valid_address(&model.recipient)
        && !picked_tokens(model).is_empty();
    if !(ok_single || ok_split || ok_multi) {
        return None;
    }
    Some(SimInput {
        chain_id: token.chain_id,
        account: account.address.clone(),
        calls: build_sim_calls(model, token)?,
    })
}

fn simulate(model: &mut Model, input: SimInput) -> Cmd {
    let id = next(model);
    model.flights.sim = Some(id);
    model.sim_input = Some(input.clone());
    issue(
        id,
        SendOperation::SimulateCalls {
            chain_id: input.chain_id,
            account: input.account,
            calls: input.calls,
        },
    )
}

/// The confirm's probes after its quote was priced again — which the fee
/// machine does once a block while the confirm is up
/// (`fee_policy::requote_interval_ms`), besides a refresh or a coin tap.
///
/// The risk is about the recipient on this chain, and neither moves while the
/// confirm is up: it is never asked again here. The simulation is asked again
/// only when the calls it simulates moved — only a sweep's are built from the
/// fee, which it holds back from the line in the fee's coin
/// ([`multi_token_specs`]); a single transfer's and a split's never are. Asking
/// again for the same calls would blank what the confirm shows (the
/// simulation and the recipient's risk) and fetch both once a block, for the
/// same answer.
fn reprobe_after_fee(model: &mut Model) -> Cmd {
    let input = sim_input(model);
    if input == model.sim_input {
        return Command::done();
    }
    model.sim_json = None;
    model.flights.sim = None;
    model.sim_input = None;
    match input {
        Some(input) => simulate(model, input),
        None => Command::done(),
    }
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

/// The chain the form is on: the selected token's, else the sweep's.
fn form_chain(model: &Model) -> Option<u32> {
    model
        .selected_token
        .as_ref()
        .map(|token| token.chain_id)
        .or(model.multi_chain_id)
}

/// [`SendView::fee_coin`].
fn fee_coin(model: &Model) -> Option<SendFeeCoin> {
    // 1. The estimate in hand. This machine holds one: each quote the fee card
    // settles is mirrored here (`FeeUpdated`) and none is dropped for a speed
    // change, so it is this speed's own once that has landed, and the speed
    // just left's while it is measured — in the coin that will pay either way.
    if let Some(fee) = selected_fee(model) {
        let chain_id = fee.chain_id;
        return Some(match &fee.fee_asset {
            FeeAsset::Native => native_fee_coin(model, chain_id),
            FeeAsset::Erc20 { token, symbol, .. } => SendFeeCoin {
                symbol: symbol
                    .clone()
                    .filter(|symbol| !symbol.trim().is_empty())
                    .or_else(|| held_symbol(model, chain_id, token))
                    .unwrap_or_default(),
                contract: Some(token.clone()),
                chain_id,
            },
        });
    }
    let chain_id = form_chain(model).or_else(|| model.fee_estimate.as_ref().map(|f| f.chain_id))?;
    // 2. The coin in force: the card's word when it spoke about this chain,
    // else the person's pick. `None` is the chain's own coin.
    let in_force = match &model.fee_card_coin {
        Some(card) if card.chain_id == chain_id => card.token.clone(),
        _ if model.fee_coin_chosen => model.gas_fee_token.clone(),
        _ => None,
    };
    // 3. The chain's own coin.
    Some(match in_force.filter(|token| !token.trim().is_empty()) {
        Some(token) => SendFeeCoin {
            symbol: held_symbol(model, chain_id, &token).unwrap_or_default(),
            contract: Some(token),
            chain_id,
        },
        None => native_fee_coin(model, chain_id),
    })
}

/// The chain's own coin as the fee row names it: the network list's symbol in
/// the registry's spelling, else the registry's, else the holding's.
fn native_fee_coin(model: &Model, chain_id: u32) -> SendFeeCoin {
    let symbol = chain_native_symbol(model, chain_id)
        .filter(|symbol| !symbol.trim().is_empty())
        .map(|symbol| super::network_admin::display_native_symbol(chain_id, None, &symbol))
        .or_else(|| super::network_admin::builtin_native_symbol(chain_id).map(str::to_owned))
        .or_else(|| {
            held(model)
                .find(|token| token.chain_id == chain_id && token.is_native())
                .map(|token| token.symbol.clone())
        })
        .unwrap_or_default();
    SendFeeCoin {
        symbol,
        contract: None,
        chain_id,
    }
}

/// An ERC-20's ticker from the form's holdings on that chain, contracts
/// compared case aside.
fn held_symbol(model: &Model, chain_id: u32, contract: &str) -> Option<String> {
    held(model)
        .find(|token| {
            token.chain_id == chain_id
                && token
                    .token_address
                    .as_deref()
                    .is_some_and(|address| address.trim().eq_ignore_ascii_case(contract.trim()))
        })
        .map(|token| token.symbol.clone())
        .filter(|symbol| !symbol.trim().is_empty())
}

/// The form's holdings: the selected token first, then the list.
fn held(model: &Model) -> impl Iterator<Item = &SendToken> {
    model.selected_token.iter().chain(model.tokens.iter())
}

/// Nothing is sendable: the fee is drawn from the asset being sent, and the
/// reserve `Max` holds back for it meets or exceeds the whole balance.
///
/// Reads the same [`max_fee_reserve`] the fill reads, so this sentence appears
/// exactly when `Max` resolves to `"0"` — never on a balance that could still
/// pay, and never absent on one that could not.
fn fee_over_balance(model: &Model, token: &SendToken) -> Option<SendAmountWarning> {
    let fee = max_quote(model)?;
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
    // This verdict is about the SINGLE amount, and a split has none. Entering a
    // split carries the typed figure into the first row and leaves its twin
    // behind in `model.amount`, where this kept judging it: "not enough ETH"
    // stayed on the form after every row had been corrected. A split has its
    // own live verdicts — `split_over_balance`, `split_row_issues`.
    if model.split_mode {
        return None;
    }
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

/// The `Continue` / confirm reading of [`fee_asset_shortfall`]: the fee coin has
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
                auto_fee_token: !model.fee_coin_chosen,
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
    match treasury {
        // A depleted relayer opens the bootstrap sheet HERE, replacing the
        // personal funding sheet entirely; confirm is not entered.
        TreasuryAnswer::LowFloat(status) => return open_bootstrap(model, status),
        // Spec 098 §2: a relay that cannot serve the chain stops the send
        // here too — before 098 it went on to the passkey and failed there.
        TreasuryAnswer::Unreachable => return open_unreachable(model),
        // The relay serves this chain (or could not be asked, which proceeds
        // as ever): no stop stands over the confirm this opens (issue #422).
        TreasuryAnswer::Serves => clear_relay_stops(model),
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
    // The reset is back to "nobody chose" — the fee machine's pick — rather
    // than to the native coin, which is what "no choice" used to mean.
    model.fee_coin_chosen = false;
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
    // The gate `can_confirm` publishes, asked again here: a disabled control
    // is a suggestion, and a confirm from a stale frame must not sign over a
    // relay stop, a refused submit or a re-quote in flight (issue #424). A
    // stop's own retry lowers the stop before it confirms.
    if !confirm_gate_open(model) {
        return Command::done();
    }
    let (Some(_), Some(account)) = (model.selected_token.as_ref(), model.account.clone()) else {
        return Command::done();
    };
    // A fee re-quote can turn a valid amount into an unpayable same-token
    // send — never let the confirm reach signing in that state (invariant ⑧).
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
    // out loud rather than bouncing: the person is on the confirm screen, and
    // the amount is not what is wrong.
    if let Some(warning) = fee_asset_gate(model) {
        return alert(
            model,
            SendAlertKind::InsufficientBalance {
                warning: Some(warning),
            },
        );
    }
    // Synchronous single-flight lock: a second confirm in the same tick is a
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
    model.receipt_not_sent = false;
    model.receipt_maybe_sent = false;
    model.submitted_at_ms = None;
    model.receipt_signed = None;
    model.fee_held = false;
    model.relay_funding = false;
    model.fee_rejected = false;
    model.refused = false;
    model.refusal = None;

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

/// The coin this send pays its fee in: the person's pick when there is one,
/// otherwise the coin the quote in hand is denominated in — the fee machine's
/// own choice (`auto_fee_token`).
///
/// The submit signs `quoted_fee.amount` in THIS coin. With the choice left to
/// the fee machine, `gas_fee_token` stays `None` while the quote is in USDC;
/// handing `None` on would have built a native fee leg carrying a USDC amount.
/// So the coin comes from the same estimate as the amount — displayed = signed.
/// Tempo's default TIP-20 stays `None`, which is how that chain names it.
fn quoted_fee_token(model: &Model, fee: Option<&FeeEstimate>) -> Option<String> {
    if model.fee_coin_chosen {
        return model.gas_fee_token.clone();
    }
    match fee.map(|f| (&f.fee_asset, f.chain_id)) {
        Some((FeeAsset::Erc20 { token, .. }, chain_id))
            if !(is_tempo_chain(chain_id)
                && token.eq_ignore_ascii_case(TEMPO_DEFAULT_FEE_TOKEN)) =>
        {
            Some(token.clone())
        }
        _ => None,
    }
}

/// Build the submit batch + activity lines and hand them to the shell's
/// sign→submit orchestration.
fn submit_user_op(model: &mut Model, gen: u64, public_key_hex: String) -> Cmd {
    let (Some(token), Some(account)) = (model.selected_token.clone(), model.account.clone()) else {
        return Command::done();
    };
    let chain_id = token.chain_id;

    // In-band: sign EXACTLY the fee the confirm screen displayed (amount +
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
            tier: super::fee_speed::wire_tier(f.tier),
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
                        token_address: tk.token_address.clone(),
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
                    token_address: token.token_address.clone(),
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
                token_address: token.token_address.clone(),
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
                gas_fee_token: quoted_fee_token(model, current_fee.as_ref()),
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
    // Spec 082 RJ1: once the records are written ahead the POST is cleared —
    // on its way; a cancel must not pretend to stop it.
    if let Pipeline::Submitting { id, .. } = model.pipeline {
        if model
            .write_ahead
            .as_ref()
            .is_some_and(|wa| wa.pipeline_id == id)
        {
            return Command::done();
        }
    }
    model.cancelled = true;
    // Release the lock so a retry starts, and invalidate the cancelled run's
    // pending `end()` (issue #91).
    model.lock.cancel();
    // Kill the PRE-SIGN pipeline: after cancel, neither a passkey prompt nor
    // a funding sheet may resurrect (invariant ③ — the intent behind TS's
    // never-read `sendCancelledRef`, made real here). An in-flight
    // sign/submit (`Submitting`) is left to its shell outcome, exactly as
    // `Passkey.cancelSign()` behaves today — except that its POST is never
    // cleared (spec 082 RJ1, second review): the passkey may already have
    // returned while the shell reads the head, and the person withdrew it.
    if matches!(
        model.pipeline,
        Pipeline::SubmitCredential { .. } | Pipeline::SubmitTreasury { .. }
    ) {
        model.pipeline = Pipeline::Idle;
    }
    if let Pipeline::Submitting { id, .. } = model.pipeline {
        model.withdrawn_submit = Some(id);
    }
    model.tx = SendTxStatus::Idle;
    fire(model, SendOperation::CancelPasskeySign)
}

fn retry_after_bootstrap(model: &mut Model) -> Cmd {
    clear_relay_stops(model);
    // After funding the relayer, return through the step-appropriate flow
    // (`SendScreen.tsx:214-224`): enter-details re-runs the pre-confirm
    // pre-check; the confirm step confirms again.
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
                return render();
            }
            // A recipient HANDED IN — a scan, a contact, a payment request —
            // survives the trip back to the picker (owner, 2026-09-23): going
            // back is how a person changes the asset, and it used to throw
            // the scan away.
            //
            // A recipient the person TYPED still goes, because there starting
            // over is theirs to redo and a stale address in an empty-looking
            // flow is worse than a cleared field.
            //
            // "Handed in" is the recipient's own history, not the open params:
            // on the phones a home scan opens the picker FIRST and the code
            // arrives afterwards (`scan_resolved`), so the params never named
            // it and Back threw the scan away (issue #332).
            let keep_recipient = model.recipient_handed_in;
            back_to_picker(model, keep_recipient)
        }
        SendStep::SelectToken => fire(model, SendOperation::Close),
    }
}

/// The single form, left for the asset picker: the token and the figure typed
/// for it go (a figure means nothing in another coin), the split rows go, and
/// the recipient stays when `keep_recipient` says so.
fn back_to_picker(model: &mut Model, keep_recipient: bool) -> Cmd {
    model.selected_token = None;
    model.max_fill = None;
    model.amount = DenominatedAmount::token("");
    if !keep_recipient {
        model.recipient.clear();
    }
    model.split_mode = false;
    model.recipients.clear();
    model.step = SendStep::SelectToken;
    render()
}

/// Whether the form's token card opens the picker (issue #326). The token is
/// the payer's to change on a single send — including one whose request named
/// only a network, where the picker stays on that network. Not where a request
/// named the token or the amount, not in a split (its rows are figures in this
/// token) or a sweep (no one token), and not while Continue is out: its
/// pre-check would answer for a token the form no longer has. The form's own
/// background quotes are another matter — they are guarded by token and chain
/// on arrival, exactly as they are for Back.
fn can_change_token(model: &Model) -> bool {
    model.step == SendStep::EnterDetails
        && model.selected_token.is_some()
        && model.lock_error.is_none()
        && (!model.params.locked || model.request_chain.is_some())
        && !model.split_mode
        && !model.multi_select_mode
        && !model.estimating_gas
        && !matches!(
            model.pipeline,
            Pipeline::ContinueCredential { .. } | Pipeline::PreCheck { .. }
        )
}

/// The token card (issue #326): to the picker, keeping whoever the form is
/// for — this is a change of WHAT is sent, not of whom to. Back kept only a
/// handed-in recipient; the card keeps a typed one too, because nothing about
/// changing the coin says the person meant to change the payee.
fn change_token(model: &mut Model) -> Cmd {
    if !can_change_token(model) {
        return Command::done();
    }
    back_to_picker(model, true)
}

fn fee_updated(model: &mut Model, estimate: FeeEstimateView) -> Cmd {
    // A wire estimate that doesn't parse is refused, not guessed at.
    let Some(fee) = parse_fee_view(&estimate) else {
        return Command::done();
    };
    model.fee_estimate = Some(fee);
    if model.step == SendStep::Confirm {
        // A sweep's sim depends on the estimate (reserve math): re-run it
        // when its calls moved, and only then.
        return Command::all([reprobe_after_fee(model), render()]);
    }
    // A requote or a chip switch on the form moves what Max holds back.
    Command::all([follow_max(model), render()])
}

fn receipt_update(model: &mut Model, user_op_hash: &str, outcome: SendReceiptOutcome) -> Cmd {
    if model.user_op_hash.as_deref() != Some(user_op_hash) {
        return Command::done(); // a stale hash — some earlier submission
    }
    match outcome {
        SendReceiptOutcome::Confirmed { tx_hash } => {
            model.tx_hash = Some(tx_hash);
        }
        // `NotSent` judges an op the relay never showed it holds: for one it
        // accepted (or has since acknowledged) it can only be stale — the
        // write-ahead hands the op to the tracker before its POST (RJ1), and
        // a slow POST can outlast the grace. The tracker revives the entry
        // on the admitted hand-off; the receipt waits for that verdict
        // (082 round-2 review).
        SendReceiptOutcome::Failed { not_sent: true, .. } if !model.receipt_maybe_sent => {}
        SendReceiptOutcome::Failed {
            rejected,
            not_sent,
            refusal,
        } => {
            // A definitive failure stamps the receipt — it never turns the
            // submitted payment back into an error state (invariant ⑤).
            model.receipt_failed = true;
            if not_sent {
                // Never sent is its own ending (spec 082), and it is not the
                // fee-rejected one whatever else the shell says.
                model.receipt_not_sent = true;
            } else if rejected {
                // Told by its reason: the fee words only when fees were the
                // reason — a nonce another send spent, a simulation that
                // fails, a relay that gave up are not "network fees stayed
                // above the amount you approved".
                model.refusal = refusal;
                model.fee_rejected =
                    refusal == Some(super::tx_tracker::RefusalReason::FeeBelowMarket);
                model.refused = true;
            }
        }
        SendReceiptOutcome::FeeHeld => {
            // Waiting, not failure: queued until fees settle (invariant ⑦).
            // The hold stage comes only from the relay's status, so the relay
            // holds the op: no longer "may have been sent" (RA10).
            model.fee_held = true;
            model.relay_funding = false;
            model.receipt_maybe_sent = false;
        }
        SendReceiptOutcome::RelayFunding => {
            // Waiting on the relay's own gas, said as such — and the stage
            // comes only from the relay's status, so it holds the op (RA10).
            model.relay_funding = true;
            model.receipt_maybe_sent = false;
        }
        SendReceiptOutcome::Acknowledged => {
            // The relay holds it: the ordinary "submitted" words (RA10).
            model.relay_funding = false;
            model.receipt_maybe_sent = false;
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
            maybe_sent,
            submit_block,
        } => {
            if model.withdrawn_submit == Some(id) {
                // Never cleared, yet it went out (a shell that POSTs without
                // the write-ahead): sent is sent — recorded as any other.
                model.withdrawn_submit = None;
            }
            accept_submitted(model, id, user_op_hash, now_ms, (maybe_sent, submit_block))
        }
        R::SubmitFailed { failure } => {
            if model.withdrawn_submit == Some(id) {
                // The submit the person cancelled ended, and nothing was
                // sent: no error, no haptic — the form, as they left it.
                model.withdrawn_submit = None;
                if matches!(model.pipeline, Pipeline::Submitting { id: expect, .. } if expect == id)
                {
                    model.pipeline = Pipeline::Idle;
                }
                return render();
            }
            accept_submit_failed(model, id, failure)
        }
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
            let clear = ctx.write_ahead.then(|| ctx.user_op_hash.clone());
            // The write-ahead's hand-off names NO record (RJ1, 082 second
            // review): a POST of the op is about to leave, and the tracker
            // holds it off "not sent" until the POST's verdict — whose
            // hand-off names the records. Named here, the not-found grace ran
            // from before the bytes left, and a POST slower than it ended the
            // op NotSent (the receipt "not sent") while it was delivering it.
            let record_ids = if ctx.write_ahead {
                Vec::new()
            } else {
                ctx.record_ids
            };
            let track = issue(
                track_id,
                SendOperation::TrackSubmitted {
                    user_op_hash: ctx.user_op_hash,
                    record_ids,
                    chain_id: ctx.chain_id,
                    maybe_sent: ctx.maybe_sent,
                    submit_block: ctx.submit_block,
                    admitted: false,
                    sender: model
                        .account
                        .as_ref()
                        .map(|account| account.address.clone()),
                },
            );
            match clear {
                // RJ1: on disk and tracked — the shell may POST.
                Some(user_op_hash) => {
                    let clear_id = next(model);
                    Command::all([
                        track,
                        issue(clear_id, SendOperation::ClearToPost { user_op_hash }),
                    ])
                }
                None => track,
            }
        }
        R::PostCleared => Command::done(),
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
        | R::FeesPrewarmed
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
                        return follow_max(model);
                    };
                    // The single form warms the very transfer Max fills —
                    // the token's whole balance to the payee, or to the
                    // account itself while there is none — so the quote in
                    // hand when Max is pressed is one Max can use as it is,
                    // and the fee machine sees what the send moves when it
                    // picks the coin. A sweep keeps its rough shape.
                    let tx = if model.multi_select_mode {
                        None
                    } else {
                        max_estimate_call(model, &token, &account.address)
                    };
                    let fee_id = next(model);
                    model.pipeline = Pipeline::WarmEstimate { id: fee_id };
                    issue(
                        fee_id,
                        SendOperation::EstimateFee {
                            chain_id: token.chain_id,
                            account: account.address,
                            tx,
                            batch: None,
                            gas_fee_token: model.gas_fee_token.clone(),
                            public_key_hex: Some(pk),
                            auto_fee_token: !model.fee_coin_chosen,
                        },
                    )
                }
                None => {
                    // Best-effort warm-up: failure is swallowed (`catch {}`)
                    // — but a Max pressed while it was out was waiting for
                    // the quote it would have started, and asks for its own.
                    model.pipeline = Pipeline::Idle;
                    follow_max(model)
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
            if model.max_fill.is_none() {
                // Typed over while it was in flight: the quote still prices
                // this transfer, so it is kept — only the fill is not wanted.
                if let SendFeeOutcome::Ok { estimate } = outcome {
                    model.fee_estimate = parse_fee_view(&estimate);
                }
                return Command::all([schedule_form_estimate(model), render()]);
            }
            match outcome {
                SendFeeOutcome::Ok { estimate } => match parse_fee_view(&estimate) {
                    Some(fee) => {
                        // The quote Max reserved against is the one the fee
                        // row shows: two different figures for one fee is
                        // how a screen stops adding up.
                        model.fee_estimate = Some(fee.clone());
                        set_max(model, max_with_fee(&token, &fee));
                        render()
                    }
                    None => {
                        set_max(model, full_balance(&token));
                        render()
                    }
                },
                SendFeeOutcome::Failed { .. } => {
                    // Estimation failed — full balance; the pre-check still
                    // warns (`useSendController.ts:819-821, 841-843`).
                    set_max(model, full_balance(&token));
                    render()
                }
            }
        }
        Pipeline::WarmEstimate { id: expect } if expect == id => {
            model.pipeline = Pipeline::Idle;
            if let SendFeeOutcome::Ok { estimate } = outcome {
                if let Some(fee) = parse_fee_view(&estimate) {
                    model.fee_estimate = Some(fee);
                }
            }
            // The warm quote knows no payee. A form that became complete while
            // it was in flight could not arm its own quote (the pipeline slot
            // was taken); it is armed now, and the warm figure stays on screen
            // until it answers. A Max pressed meanwhile was waiting for THIS
            // answer — and if it failed, Max asks for its own now. A failed
            // warm-up is otherwise swallowed.
            Command::all([follow_max(model), schedule_form_estimate(model), render()])
        }
        Pipeline::FormEstimate { id: expect, key } if expect == id => {
            model.pipeline = Pipeline::Idle;
            if let SendFeeOutcome::Ok { estimate } = outcome {
                if let Some(fee) = parse_fee_view(&estimate) {
                    model.fee_estimate = Some(fee);
                    model.form_quote_key = Some(key);
                    return Command::all([follow_max(model), render()]);
                }
            }
            // Best-effort: a failed form quote is not a refusal. Continue's
            // pre-check asks again and says so if it must (invariant ②). A Max
            // waiting on it asks for its own.
            follow_max(model)
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
    if model
        .treasury_watch
        .as_ref()
        .is_some_and(|watch| watch.timer_id == id && watch.probe_id.is_none())
    {
        return watch_timer_fired(model);
    }
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
    // The open sheet's watch is answered first: it is not a pipeline, and
    // runs while the pipeline is idle (spec 098 §4).
    if model
        .treasury_watch
        .as_ref()
        .is_some_and(|watch| watch.probe_id == Some(id))
    {
        return accept_watch_probe(model, probe);
    }
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
                treasury: Some(TreasuryAnswer::of(&probe)),
            };
            precheck_settle(model)
        }
        Pipeline::SubmitTreasury {
            id: expect,
            gen,
            public_key_hex,
        } if expect == id => match TreasuryAnswer::of(&probe) {
            TreasuryAnswer::LowFloat(status) => {
                // The float fell below its floor after the preflight —
                // stop BEFORE the passkey (invariant ⑭).
                model.pipeline = Pipeline::Idle;
                model.tx = SendTxStatus::Idle;
                model.lock.end(gen);
                open_bootstrap(model, status)
            }
            TreasuryAnswer::Unreachable => {
                // Spec 098 §2, and for the same reason: nothing to sign for.
                model.pipeline = Pipeline::Idle;
                model.tx = SendTxStatus::Idle;
                model.lock.end(gen);
                open_unreachable(model)
            }
            TreasuryAnswer::Serves => submit_user_op(model, gen, public_key_hex),
        },
        Pipeline::FailureProbe {
            id: expect,
            gen,
            fallback,
        } if expect == id => {
            model.pipeline = Pipeline::Idle;
            let cmd = match TreasuryAnswer::of(&probe) {
                TreasuryAnswer::LowFloat(status) => {
                    // The honest ask: the community bootstrap sheet, not a
                    // "try again" loop.
                    model.tx = SendTxStatus::Idle;
                    open_bootstrap(model, status)
                }
                TreasuryAnswer::Unreachable => {
                    // The submission failed because the relay cannot serve
                    // this chain: say that, not "something went wrong".
                    model.tx = SendTxStatus::Idle;
                    open_unreachable(model)
                }
                TreasuryAnswer::Serves => {
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

/// Close both relay stops, and the treasury stop's watch with them.
fn clear_relay_stops(model: &mut Model) {
    model.treasury_bootstrap = None;
    model.treasury_watch = None;
    model.relay_unreachable = None;
}

/// A relay stop is a fact about ONE chain: the relay that was asked about it
/// has no gas there, or cannot reach it. Once the send is no longer on that
/// chain the stop is not about this send at all, and it goes — with the watch
/// that would keep asking about the old chain every ten seconds, and that on
/// a "funded" answer would press Continue on whatever the form now holds.
///
/// Issue #422. A send on a network whose relayer was empty stopped at
/// Continue; the person picked another coin on Polygon, whose relayer was
/// funded; Polygon's pre-check said so and confirm opened — under the other
/// chain's stop, out of gas, asking 0.0001 of the other chain's coin. Nothing
/// cleared a stop but its own two buttons, so every way of leaving it (Back,
/// the token card, a new pick in the sweep picker) carried it along. On a
/// phone that drew the stop only on confirm (v0.9.6) the person never saw it
/// open: Continue just "did nothing" on the first chain.
///
/// [`App::update`] runs this after every event, so no path that changes the
/// coin — today's or a future one — has to remember it.
fn drop_foreign_relay_stops(model: &mut Model) {
    let chain = model.selected_token.as_ref().map(|token| token.chain_id);
    let stop_chain = model
        .treasury_bootstrap
        .as_ref()
        .map(|status| status.chain_id)
        .or_else(|| model.relay_unreachable.as_ref().map(|sheet| sheet.chain_id))
        .or_else(|| model.treasury_watch.as_ref().map(|watch| watch.chain_id));
    if stop_chain.is_some_and(|stop| Some(stop) != chain) {
        clear_relay_stops(model);
    }
}

/// [`SendTreasuryStatus::coin`]: the relay's figures for this stop's chain,
/// in that chain's coin (issue #422). `None` when either figure is not a
/// whole number of base units — a balance nobody can read is not a zero.
fn treasury_coin(model: &Model, status: &SendTreasuryStatus) -> Option<SendTreasuryCoin> {
    let units = |figure: &str| figure.trim().parse::<u128>().ok();
    let (balance, floor) = (units(&status.balance)?, units(&status.floor)?);
    let decimals = status.asset.decimals();
    Some(SendTreasuryCoin {
        symbol: treasury_symbol(model, status),
        balance: from_base_units(balance, decimals),
        floor: from_base_units(floor, decimals),
        // The final difference, never below nothing: a treasury at or above
        // its floor lacks nothing.
        suggested: from_base_units(floor.saturating_sub(balance), decimals),
    })
}

/// The coin a treasury stop's figures are in. pathUSD on Tempo; otherwise
/// the chain's own coin — from the networks the shell handed over (which
/// include ones the person added), the built-in registry, or a holding of
/// that coin, in that order. Never a guess: an unknown chain names no coin.
fn treasury_symbol(model: &Model, status: &SendTreasuryStatus) -> Option<String> {
    let chain_id = status.chain_id;
    match status.asset {
        SendTreasuryAsset::PathUsd => Some("pathUSD".to_owned()),
        SendTreasuryAsset::Native => chain_native_symbol(model, chain_id)
            .filter(|symbol| !symbol.trim().is_empty())
            .map(|symbol| super::network_admin::display_native_symbol(chain_id, None, &symbol))
            .or_else(|| super::network_admin::builtin_native_symbol(chain_id).map(str::to_owned))
            .or_else(|| {
                model
                    .selected_token
                    .iter()
                    .chain(&model.tokens)
                    .find(|token| token.chain_id == chain_id && token.is_native())
                    .map(|token| token.symbol.clone())
            }),
    }
}

/// [`SendView::relay_report`]: the report for the relay stop that is up, on
/// a network Vela ships (the `operator_served` predicate), else `None`.
fn relay_report(model: &Model) -> Option<SendRelayReport> {
    // Where the stop met the person: the form's Continue, or the pre-sign
    // recheck on the confirm page.
    let pressed = if model.step == SendStep::Confirm {
        "Confirm"
    } else {
        "Continue"
    };
    if let Some(status) = &model.treasury_bootstrap {
        let chain = relay_report_chain(status.chain_id)?;
        return Some(SendRelayReport {
            what: format!(
                "Relayer out of gas on {chain}\n\nTreasury: {}\n{}",
                status.address,
                treasury_figures(status, treasury_coin(model, status).as_ref()),
            ),
            steps: format!(
                "1. Send on {chain}\n2. {pressed}: the relay's treasury check stopped \
                 the send — its relayer is out of gas"
            ),
            area: RELAY_REPORT_AREA.to_owned(),
            fingerprint: format!("relay-gas-{}", status.chain_id),
        });
    }
    let sheet = model.relay_unreachable.as_ref()?;
    let chain = relay_report_chain(sheet.chain_id)?;
    Some(SendRelayReport {
        what: format!(
            "Relay can't reach {chain}\n\nThe relay's treasury check answered that it \
             cannot serve this network: it has no RPC it can use for it."
        ),
        steps: format!(
            "1. Send on {chain}\n2. {pressed}: the relay's treasury check stopped the \
             send — it cannot serve this network"
        ),
        area: RELAY_REPORT_AREA.to_owned(),
        fingerprint: format!("relay-unreachable-{}", sheet.chain_id),
    })
}

/// "Has 0 ETH of its 0.0001 ETH floor (short 0.0001 ETH)." — in whole coin
/// with its symbol, as the stop shows them. Figures that cannot be put in a
/// coin are written as the relay's own base units, with their unit, rather
/// than not at all.
fn treasury_figures(status: &SendTreasuryStatus, coin: Option<&SendTreasuryCoin>) -> String {
    if let Some((coin, symbol)) = coin.and_then(|coin| Some((coin, coin.symbol.as_deref()?))) {
        return format!(
            "Has {} {symbol} of its {} {symbol} floor (short {} {symbol}).",
            coin.balance, coin.floor, coin.suggested
        );
    }
    let unit = match status.asset {
        SendTreasuryAsset::Native => "wei",
        SendTreasuryAsset::PathUsd => "micro-pathUSD",
    };
    format!(
        "Has {} {unit} of its {} {unit} floor.",
        status.balance.trim(),
        status.floor.trim()
    )
}

/// "Unichain (130)": a network Vela ships, by its name and id — `None` for
/// any other, which has no operator to report to.
fn relay_report_chain(chain_id: u32) -> Option<String> {
    if !super::network_admin::is_builtin_chain(chain_id) {
        return None;
    }
    super::network_admin::builtin_display_name(chain_id).map(|name| format!("{name} ({chain_id})"))
}

/// Open the treasury sheet and start watching the relay (spec 098 §4).
fn open_bootstrap(model: &mut Model, status: SendTreasuryStatus) -> Cmd {
    let chain_id = status.chain_id;
    model.treasury_bootstrap = Some(status);
    model.relay_unreachable = None;
    Command::all([watch_treasury(model, chain_id), render()])
}

/// A relay stop is up: the send waits on the relay, and says so.
fn relay_stopped(model: &Model) -> bool {
    model.treasury_bootstrap.is_some() || model.relay_unreachable.is_some()
}

/// The confirm's gate, less the two figure checks (a same-asset breach
/// and a figure that stopped resolving), which the confirm answers by going back
/// to the amount rather than by refusing. Published in `can_confirm`, and the
/// confirm refuses on it (issue #424): every reason here is already on the page
/// — the stop's own notice, the fee row re-quoting, the receipt or the error
/// panel — so a refusal is never silent.
fn confirm_gate_open(model: &Model) -> bool {
    model.tx == SendTxStatus::Idle
        && !model.lock.busy()
        && model.tx_error.is_none()
        && !model.estimating_gas
        && !model.fee_busy
        && !relay_stopped(model)
        && previous_in_flight(model).is_none()
}

/// The operation this send must wait for: the account's previous one on the
/// form's chain, still in flight on this device — never the op this journey
/// itself submitted ([`Event::InFlightOps`]).
fn previous_in_flight(model: &Model) -> Option<&super::tx_tracker::InFlightOp> {
    let sender = &model.account.as_ref()?.address;
    super::tx_tracker::previous_in_flight(
        &model.in_flight,
        sender,
        form_chain(model)?,
        model.user_op_hash.as_deref(),
    )
}

/// Open the "relay can't reach this network" sheet (spec 098 §2).
fn open_unreachable(model: &mut Model) -> Cmd {
    let Some(chain_id) = model.selected_token.as_ref().map(|token| token.chain_id) else {
        return render();
    };
    model.treasury_bootstrap = None;
    model.treasury_watch = None;
    model.relay_unreachable = Some(SendRelayUnreachable {
        chain_id,
        operator_served: false,
    });
    render()
}

/// Start the next wait of the open sheet's watch.
fn watch_treasury(model: &mut Model, chain_id: u32) -> Cmd {
    let timer_id = next(model);
    model.treasury_watch = Some(TreasuryWatch {
        chain_id,
        timer_id,
        probe_id: None,
    });
    issue(
        timer_id,
        SendOperation::StartTimer {
            ms: TREASURY_WATCH_MS,
            tag: SendTimerTag::TreasuryWatch,
        },
    )
}

/// The watch's wait elapsed: ask the relay, if the sheet is still open.
fn watch_timer_fired(model: &mut Model) -> Cmd {
    let Some(watch) = model.treasury_watch.clone() else {
        return Command::done();
    };
    if model.treasury_bootstrap.is_none() {
        model.treasury_watch = None;
        return Command::done();
    }
    let probe_id = next(model);
    model.treasury_watch = Some(TreasuryWatch {
        probe_id: Some(probe_id),
        ..watch.clone()
    });
    issue(
        probe_id,
        SendOperation::ProbeTreasury {
            chain_id: watch.chain_id,
        },
    )
}

/// The watch's probe answered (spec 098 §4).
///
/// Funded: the sheet closes. On the form the send goes on to confirm by
/// itself — that is a screen, not a signature. On confirm it only closes:
/// the passkey is the person's to start, and browsers refuse a WebAuthn
/// prompt nobody pressed for. Still short: the balance shown is refreshed and
/// the watch waits again. No answer: it waits again — transient, as always.
fn accept_watch_probe(model: &mut Model, probe: SendTreasuryProbe) -> Cmd {
    let Some(watch) = model.treasury_watch.clone() else {
        return Command::done();
    };
    if model.treasury_bootstrap.is_none() {
        model.treasury_watch = None;
        return Command::done();
    }
    match TreasuryAnswer::of(&probe) {
        TreasuryAnswer::LowFloat(status) => {
            model.treasury_bootstrap = Some(status);
            Command::all([watch_treasury(model, watch.chain_id), render()])
        }
        TreasuryAnswer::Serves if matches!(probe, SendTreasuryProbe::Covered) => {
            model.treasury_watch = None;
            match model.step {
                SendStep::EnterDetails => retry_after_bootstrap(model),
                _ => {
                    model.treasury_bootstrap = None;
                    render()
                }
            }
        }
        // `Unknown`, or a relay that has since stopped serving the chain
        // while a sheet about its gas is open: keep the sheet, ask again.
        _ => watch_treasury(model, watch.chain_id),
    }
}

/// One activity record per recipient of `lines`, under `user_op_hash`.
fn tx_records(
    model: &Model,
    lines: &[SendLine],
    chain_id: u32,
    user_op_hash: &str,
    now_ms: f64,
    (maybe_sent, submit_block): (bool, Option<u64>),
) -> Vec<SendTxRecord> {
    let from = model
        .account
        .as_ref()
        .map(|a| a.address.clone())
        .unwrap_or_default();
    let timestamp_s = (now_ms / 1000.0).floor();
    lines
        .iter()
        .enumerate()
        .map(|(i, ln)| {
            let usd = parse_float_or_zero(&ln.amount) * ln.price_usd;
            SendTxRecord {
                id: if lines.len() > 1 {
                    format!("{user_op_hash}-{i}")
                } else {
                    user_op_hash.to_owned()
                },
                user_op_hash: user_op_hash.to_owned(),
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
                maybe_sent,
                submit_block,
            }
        })
        .collect()
}

/// The write-ahead (spec 082 RJ1): signed, nothing POSTed. The records are
/// written "may have been sent" in ONE write; their ack hands them to the
/// tracker and clears the POST. The receipt still waits for the verdict.
fn op_signed(
    model: &mut Model,
    user_op_hash: String,
    submit_block: Option<u64>,
    now_ms: f64,
) -> Cmd {
    let Pipeline::Submitting {
        id: pipeline_id,
        chain_id,
        lines,
        ..
    } = model.pipeline.clone()
    else {
        return Command::done();
    };
    if model
        .write_ahead
        .as_ref()
        .is_some_and(|wa| wa.pipeline_id == pipeline_id)
    {
        return Command::done(); // once per submit
    }
    if model.withdrawn_submit.is_some() {
        // A cancelled submit's result is still owed, and `OpSigned` names no
        // submit: this may be the cancelled one's. Nothing is written or
        // cleared — the shell's wait runs out and nothing is sent (a retry
        // caught in this window ends "not sent", and can be slid again).
        return Command::done();
    }
    let records = tx_records(
        model,
        &lines,
        chain_id,
        &user_op_hash,
        now_ms,
        (true, submit_block),
    );
    let record_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    model.write_ahead = Some(WriteAhead {
        pipeline_id,
        user_op_hash: user_op_hash.clone(),
        record_ids: record_ids.clone(),
        submit_block,
    });
    // Signed: the sheet is past the passkey, on its way to the relay.
    if model.tx == SendTxStatus::Signing {
        model.tx = SendTxStatus::Submitting;
    }
    let persist_id = next(model);
    model.flights.persist = Some((
        persist_id,
        PersistCtx {
            user_op_hash,
            record_ids,
            chain_id,
            maybe_sent: true,
            submit_block,
            write_ahead: true,
        },
    ));
    Command::all([
        issue(persist_id, SendOperation::PersistTxRecords { records }),
        render(),
    ])
}

/// Withdraw the write-ahead of pipeline `pipeline_id`, if any (RJ1): its op
/// is proven never sent. Deletes the records in one write and tells the
/// tracker to forget them; a persist of them still in flight is dropped.
fn withdraw_write_ahead(model: &mut Model, pipeline_id: u64) -> Vec<Cmd> {
    let Some(wa) = model
        .write_ahead
        .take_if(|wa| wa.pipeline_id == pipeline_id)
    else {
        return Vec::new();
    };
    if model
        .flights
        .persist
        .as_ref()
        .is_some_and(|(_, ctx)| ctx.write_ahead)
    {
        model.flights.persist = None;
    }
    let delete_id = next(model);
    let withdraw_id = next(model);
    vec![
        issue(
            delete_id,
            SendOperation::DeleteTxRecords {
                ids: wa.record_ids.clone(),
            },
        ),
        issue(
            withdraw_id,
            SendOperation::TrackWithdrawn {
                user_op_hash: wa.user_op_hash,
                record_ids: wa.record_ids,
            },
        ),
    ]
}

fn accept_submitted(
    model: &mut Model,
    id: u64,
    user_op_hash: String,
    now_ms: f64,
    (maybe_sent, submit_block): (bool, Option<u64>),
) -> Cmd {
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

    // RJ1: the records written ahead name this op — no second write. Another
    // hash (the relay's own) withdraws them, and today's write follows.
    let mut commands = Vec::new();
    let written = model
        .write_ahead
        .take_if(|wa| wa.pipeline_id == id && wa.user_op_hash.eq_ignore_ascii_case(&user_op_hash));
    if written.is_none() {
        commands.extend(withdraw_write_ahead(model, id));
    }

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
    model.receipt_maybe_sent = maybe_sent;
    model.tx = SendTxStatus::Confirmed;
    model.lock.end(gen);

    let from = model
        .account
        .as_ref()
        .map(|a| a.address.clone())
        .unwrap_or_default();
    if !maybe_sent {
        // The success haptic says "sent" — a lost reply has not earned it
        // (spec 082 RA10); the receipt says "may have been sent" instead.
        let haptic_id = next(model);
        commands.push(issue(
            haptic_id,
            SendOperation::Haptic {
                kind: SendHapticKind::Success,
            },
        ));
    }
    let clear_id = next(model);
    commands.push(issue(
        clear_id,
        SendOperation::ClearTokenCache { address: from },
    ));

    if let Some(wa) = written {
        // Accepted: the records stop saying "may have been sent" and the
        // tracker learns the relay has it. A lost reply writes nothing — the
        // records already say so. Either way the tracker is handed the
        // records now: the POST is over (082 second review).
        if !maybe_sent {
            let mark_id = next(model);
            commands.push(issue(
                mark_id,
                SendOperation::MarkAdmitted {
                    record_ids: wa.record_ids.clone(),
                },
            ));
        }
        let track_id = next(model);
        model.flights.track = Some(track_id);
        commands.push(issue(
            track_id,
            SendOperation::TrackSubmitted {
                user_op_hash: wa.user_op_hash,
                record_ids: wa.record_ids,
                chain_id,
                maybe_sent,
                submit_block: wa.submit_block.or(submit_block),
                admitted: !maybe_sent,
                sender: model
                    .account
                    .as_ref()
                    .map(|account| account.address.clone()),
            },
        ));
        commands.push(render());
        return Command::all(commands);
    }

    // One activity record per recipient, all persisted in ONE atomic write
    // (invariant ⑥) — a per-record write would drop every sibling but one.
    let records = tx_records(
        model,
        &lines,
        chain_id,
        &user_op_hash,
        now_ms,
        (maybe_sent, submit_block),
    );
    let record_ids: Vec<String> = records.iter().map(|r| r.id.clone()).collect();
    let persist_id = next(model);
    model.flights.persist = Some((
        persist_id,
        PersistCtx {
            user_op_hash,
            record_ids,
            chain_id,
            maybe_sent,
            submit_block,
            write_ahead: false,
        },
    ));
    commands.push(issue(
        persist_id,
        SendOperation::PersistTxRecords { records },
    ));
    commands.push(render());
    Command::all(commands)
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
    // RJ1: a failure is proven "not sent" — nothing is POSTed without
    // `ClearToPost`, and a POST whose reply was lost is `Submitted{maybe_sent}`.
    // The written-ahead records go, and the tracker forgets them.
    let withdrawn = withdraw_write_ahead(model, id);
    let command = submit_failed(model, gen, failure);
    if withdrawn.is_empty() {
        command
    } else {
        Command::all(withdrawn.into_iter().chain([command]))
    }
}

fn submit_failed(model: &mut Model, gen: u64, failure: SendSubmitFailure) -> Cmd {
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
        SendSubmitFailure::PreviousPending => {
            // Not a failure of the network: the account's previous
            // transaction still holds the nonce. Said as such, and "Try
            // again" waits for it like any held confirm.
            model.tx = SendTxStatus::Error;
            model.tx_error = Some(SendTxErrorKey::PreviousPending);
            model.lock.end(gen);
            fire(
                model,
                SendOperation::Haptic {
                    kind: SendHapticKind::Error,
                },
            )
        }
        SendSubmitFailure::VenueBlocked { block } => {
            // Spec 102: not a failure of the network or the relay — this
            // account cannot sign here, and the screen says why.
            model.tx = SendTxStatus::Error;
            model.tx_error = Some(SendTxErrorKey::VenueBlocked);
            model.venue_block = Some(block);
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
        effective_gas_price: fee.effective_gas_price.map(|wei| wei.to_string()),
        max_gas_price: fee.max_gas_price.map(|wei| wei.to_string()),
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
        // Display only, so a figure that will not parse is dropped rather than
        // refusing the whole estimate — unlike every field above it, which the
        // send is priced and signed against (issue 684).
        effective_gas_price: view.effective_gas_price.as_deref().and_then(parse),
        max_gas_price: view.max_gas_price.as_deref().and_then(parse),
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
    //
    // Every coin the signature moved (097 F, S3): a sweep's success screen
    // named its first coin alone — "Sent 0.000418 ETH" over a transaction
    // that also moved 0.034929 USDC — and a split's headline was its first
    // row, not its total. The lines are the batch's when it was one, else the
    // one signed line.
    let lines = model
        .receipt_lines
        .as_deref()
        .unwrap_or(model.receipt_signed.as_slice());
    let coins = receipt_coins(lines);
    let amount = match coins.as_slice() {
        [one] => one.amount.clone(),
        _ => String::new(),
    };
    let usd_value: f64 = coins.iter().map(|coin| coin.usd_value).sum();
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
        status: if model.receipt_failed && model.receipt_not_sent {
            SendReceiptStatus::NotSent
        } else if model.receipt_failed {
            SendReceiptStatus::Failed
        } else if model.tx_hash.is_some() {
            SendReceiptStatus::Confirmed
        } else if model.receipt_maybe_sent {
            SendReceiptStatus::MaybeSent
        } else {
            SendReceiptStatus::Submitted
        },
        refusal_key: model
            .refused
            .then(|| super::tx_tracker::refusal_key(model.refusal).to_owned()),
        hold_reason: if model.fee_rejected {
            Some(SendHoldReason::FeeRejected)
        } else if model.fee_held {
            Some(SendHoldReason::FeeHold)
        } else if model.relay_funding {
            Some(SendHoldReason::RelayFunding)
        } else {
            None
        },
        kind: model.receipt_kind,
        transfers,
        coins,
        amount,
        usd_value: if usd_value.is_nan() { 0.0 } else { usd_value },
        submitted_at_ms: model.submitted_at_ms,
        typical_inclusion_s: model
            .selected_token
            .as_ref()
            .and_then(|token| super::network_admin::typical_inclusion_s(token.chain_id)),
    })
}

/// The coins of `lines`, each summed over its recipients, in the order first
/// signed ([`SendReceiptView::coins`]). A coin with one line keeps that
/// line's figure exactly; a coin paid to several recipients is their exact
/// base-unit sum (empty, never a guess, if a line could not be read).
fn receipt_coins(lines: &[SendLine]) -> Vec<SendReceiptCoin> {
    struct Tally<'a> {
        first: &'a SendLine,
        units: Option<u128>,
        count: usize,
        usd: f64,
    }
    let mut tallies: Vec<Tally> = Vec::new();
    for line in lines {
        let usd = js_parse_float(&line.amount).max(0.0) * line.price_usd;
        let usd = if usd.is_nan() { 0.0 } else { usd };
        let units = to_base_units(&line.amount, line.decimals);
        let same_coin = |tally: &&mut Tally| {
            tally.first.decimals == line.decimals
                && match (&tally.first.token_address, &line.token_address) {
                    (None, None) => true,
                    (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                    _ => false,
                }
        };
        match tallies.iter_mut().find(same_coin) {
            Some(tally) => {
                tally.units = tally.units.zip(units).and_then(|(a, b)| a.checked_add(b));
                tally.count += 1;
                tally.usd += usd;
            }
            None => tallies.push(Tally {
                first: line,
                units,
                count: 1,
                usd,
            }),
        }
    }
    tallies
        .into_iter()
        .map(|tally| SendReceiptCoin {
            amount: if tally.count == 1 {
                tally.first.amount.clone()
            } else {
                tally
                    .units
                    .map(|units| from_base_units(units, tally.first.decimals))
                    .unwrap_or_default()
            },
            symbol: tally.first.symbol.clone(),
            logo_urls: tally.first.logo_urls.clone(),
            token_address: tally.first.token_address.clone(),
            usd_value: tally.usd,
        })
        .collect()
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

#[cfg(test)]
mod relay_report_tests {
    use super::{relay_report, Model, SendRelayUnreachable, SendTreasuryAsset, SendTreasuryStatus};
    use crate::app::network_admin::BUILTIN_CHAINS;

    /// Every network Vela ships gets a title the tracker can show whole.
    #[test]
    fn every_built_in_chain_s_title_fits_in_80_characters() {
        for chain in BUILTIN_CHAINS {
            let unreachable = Model {
                relay_unreachable: Some(SendRelayUnreachable {
                    chain_id: chain.chain_id,
                    operator_served: false,
                }),
                ..Model::default()
            };
            let gas = Model {
                treasury_bootstrap: Some(SendTreasuryStatus {
                    chain_id: chain.chain_id,
                    address: "0x3e59292e18417f814112f731e7163534c6d2fe3c".to_owned(),
                    asset: SendTreasuryAsset::Native,
                    balance: "0".to_owned(),
                    floor: "100000000000000".to_owned(),
                    bootstrap_needed: true,
                    operator_served: false,
                    coin: None,
                }),
                ..Model::default()
            };
            for model in [unreachable, gas] {
                let Some(report) = relay_report(&model) else {
                    unreachable!("{} ships, so its stop has a report", chain.display_name)
                };
                let title = report.what.lines().next().unwrap_or_default();
                assert!(
                    title.chars().count() <= 80 && title.contains(chain.display_name),
                    "{title:?}"
                );
            }
        }
    }
}
