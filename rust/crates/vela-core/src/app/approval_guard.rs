//! Machine — the approval guard (spec `017`, inventory `### approval_guard (P1)`).
//!
//! The "unlimited never leaves the wallet unseen" core: detection of every
//! approval-granting shape straight off the raw calldata / typed data (a token
//! drainer's #1 tool is an unbounded `approve` / `permit` / `setApprovalForAll`,
//! and a descriptor lookup is exactly what fails on novel/hostile contracts),
//! the finite re-encode, the descriptor-independent submit guard, plus the
//! spending-cap editor's choice derivation (single AND per batch leg — one
//! `init_editor` / `derive_editor` pair serves both) and the EIP-5792 per-leg
//! gating.
//!
//! **An unbounded request is kept as asked unless the person lowers it**
//! (founder ruling 2026-09-26, replacing "never unlimited"). Permit2 is built
//! on a standing unlimited `approve(Permit2, MAX)`, and a smart account's
//! EIP-5792 bundle spends that allowance in the SAME atomic batch: a wallet
//! that re-encodes the approve leg to a smaller number makes the dApp's swap
//! leg revert and takes the whole bundle down with it. So the "Requested"
//! chip is preselected for an unbounded amount too (choice
//! [`GuardChoice::Unlimited`] — the bytes stay the dApp's), the surface warns,
//! a cap is one chip away, and the submit guard lets an unbounded amount
//! through only when this machine's view said the person saw it
//! ([`GuardView::unlimited_consented`]).
//!
//! ```text
//! ApprovalDetected ─► detect (8 shapes, pure) ─► editor init
//!        │                                        │
//!        ├─ ReadTokenMetadata / ReadErc20Allowance / ReadErc20Balance (RPC)
//!        ▼                                        ▼
//!   typed-path permit ─► sign-verbatim surface   calldata ─► cap editor
//!   (never rewritten — consent, not capping)     Requested preselected;
//!                                                a cap re-encodes one word
//! ```
//!
//! Faithful port of the TypeScript sources — behavior aligned line by line,
//! the cap constants (2^128 for every amount field since §37 — they were 2^200
//! for uint256 and 2^152 for uint160) and
//! every classification kept verbatim:
//!
//! - `src/services/approval-guard.ts` (whole file) — detectApproval's 8
//!   shapes, rewriteApprovalParams + assertOnlyWordChanged, enforceNoUnlimited,
//!   parse/format token amounts
//! - `src/components/signing/EditableApproveCard.tsx:85-107` — the
//!   mode → choice derivation (a custom amount ≥ cap derives `None` + an
//!   error, never a choice — keeping the unbounded ask is the Requested
//!   chip's job, not the cap field's; the TS started an unbounded request
//!   with NO choice, the 2026-09-26 ruling above preselects it), `:217` (a
//!   boolean grant-all is never preselected), `:200-202` (unverified
//!   decimals warning)
//! - `src/components/signing/SigningSheet.tsx:196-228, 316-387, 527-583` —
//!   metadata resolution + fallbacks, batch leg resolution, the confirm-time
//!   rewrite (fail-closed: a rewrite error leaves params untouched for the
//!   submit guard to refuse), and the confirm gate
//! - `src/components/signing/views/ApprovalView.tsx:40-66, 143-171` —
//!   allowance/balance reads and the increaseAllowance resulting-total rule
//!   ("increase by 100" must never read as "cap at 100")
//! - `src/components/signing/views/BatchCallsView.tsx:40-53` —
//!   `legNeedsChoice` / `legGrantsBroad`; `:74-77` — the "a token sent to its
//!   own contract is a burn" banner (the shell forwards the descriptor
//!   pipeline's recipients, this machine decides)
//! - `src/components/signing/views/PermitSignView.tsx:103-105` — the bounded
//!   unverified-decimals warning on the permit surface
//! - `src/hooks/use-dapp-signing.ts:364, 413-415` — the submit chokepoints
//!   this module's [`enforce_no_unlimited`] backs (single tx + every batch
//!   leg as if standalone)
//!
//! Ported quirks, kept verbatim (see inventory open questions):
//!
//! - The editor's custom text is seeded at detection time with the 18-decimals
//!   fallback (metadata has not resolved yet) and is NOT re-seeded when the
//!   real decimals arrive — only a preset press re-seeds. Exactly today's
//!   mount-time `useState` + `key={requestId}` behavior.
//! - The balance read fires even for `decreaseAllowance` (where the Balance
//!   preset is then suppressed) — `ApprovalView.tsx:57-66` only skips NFTs.
//! - A single approval's failed metadata read falls back to a
//!   `0xA0b8…`-style short symbol; a failed BATCH metadata read yields an
//!   empty map (legs show `…`/18/unverified) while an individually missing
//!   token still gets the short-symbol fallback — two different fallbacks,
//!   both today's (`SigningSheet.tsx:219-226, 373-385`).
//!
//! Deliberate strictness where JS semantics cannot map onto `U256` (all
//! explicit, all failing toward *never granting more*):
//!
//! - Calldata containing non-ASCII or non-hex characters detects as "no
//!   approval" instead of throwing mid-render as `BigInt('0x…')` does; such
//!   bytes can never be submitted anyway.
//! - `toBig`'s exotic `String(v)` coercions (arrays/objects) evaluate to 0.
//! - Values ≥ 2^256 saturate at `U256::MAX` — still ≥ every cap, so the
//!   classification ("unlimited") is unchanged.
//! - Re-serialized typed data has its JSON keys sorted (serde_json map
//!   order); the EIP-712 hash is key-order independent.

use std::collections::BTreeMap;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use alloy_primitives::U256;

/// Re-exported for the shells.
///
/// [`format_token_amount`] takes the width the amounts in this machine are
/// held at, and a shell that wanted to call it had to depend on
/// `alloy-primitives` itself — which the desktop deliberately does not (its
/// `executor/abi.rs` says so in as many words: the ABI crate lives here, not
/// there). One re-export beside the function that needs it is cheaper than a
/// second formatter in every shell, and a second formatter would be a second
/// answer to "how much am I approving".
pub use alloy_primitives::U256 as GuardAmount;

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Caps — the whole invariant in two numbers
// ---------------------------------------------------------------------------

/// §37 (review 1-12): "unlimited" from 2^128 up, for every amount field —
/// uint256 (ERC-20 approve / increaseAllowance / ERC-2612 value / Permit2
/// transfers) and Permit2's uint160 alike. 2^128 ≈ 3.4 × 10^38 base units:
/// 3.4 × 10^20 whole tokens at 18 decimals, beyond any real supply
/// (SHIB's is below 2^110 base units), so no amount at or above it is about
/// what this transaction moves. The line was 2^200 (2^152 for uint160),
/// which let 2^199 — every coin anybody will ever hold — read as "a big
/// finite number"; a drainer needs no sentinel, only a number nobody holds.
pub const UNLIMITED_CAP_256: U256 = U256::from_limbs([0, 0, 1, 0]);
/// The same line for Permit2's uint160 fields (sentinel 2^160-1): 2^128.
pub const UNLIMITED_CAP_160: U256 = UNLIMITED_CAP_256;

/// §37 (review 1-12): an approval for at least this many times what the
/// account holds of the token is flagged as unlimited-like (said with
/// [`FAR_ABOVE_BALANCE_KEY`]), where the balance was read and is not zero.
/// A hundredfold is no longer about this transaction: an allowance that
/// size is a standing claim on whatever arrives later. Lower multiples are
/// common and honest — a round number above the balance, an approval before
/// a top-up — and a warning on them would teach people to skip it. A zero
/// balance flags nothing by this rule: there is nothing at stake yet, and
/// every first approval would read as a drain; [`UNLIMITED_CAP_256`] still
/// holds.
pub const FAR_ABOVE_BALANCE_TIMES: u64 = 100;

/// The danger sentence for an unbounded amount (spec 094 S8).
pub const UNLIMITED_WARNING_KEY: &str = "componentsUi.signing.unlimitedWarning";
/// §37: the danger sentence for an amount at least
/// [`FAR_ABOVE_BALANCE_TIMES`] times the balance — "far beyond your balance",
/// never "unlimited", because it is a number and the sheet shows it.
pub const FAR_ABOVE_BALANCE_KEY: &str = "componentsUi.signing.farAboveBalanceWarning";

/// Field width of an approval amount (drives the cap + the re-encode).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AmountBits {
    B256,
    B160,
}

pub fn cap_for_bits(bits: AmountBits) -> U256 {
    match bits {
        AmountBits::B160 => UNLIMITED_CAP_160,
        AmountBits::B256 => UNLIMITED_CAP_256,
    }
}

/// True when an amount is "effectively unlimited" for its field width.
pub fn is_unbounded_amount(amount: U256, bits: AmountBits) -> bool {
    amount >= cap_for_bits(bits)
}

fn bits_from_wire(bits: Option<u32>) -> AmountBits {
    // `detected.amountBits ?? 256` — the TS default.
    if bits == Some(160) {
        AmountBits::B160
    } else {
        AmountBits::B256
    }
}

// ---------------------------------------------------------------------------
// Selectors — pinned by a test against `primitives::function_selector`
// ---------------------------------------------------------------------------

/// `approve(address,uint256)` — also ERC-721 `approve(operator, tokenId)`; a
/// tokenId is never ≥ cap, so capping stays safe.
const SEL_APPROVE: &str = "095ea7b3";
const SEL_INCREASE_ALLOWANCE: &str = "39509351";
const SEL_DECREASE_ALLOWANCE: &str = "a457c2d7";
const SEL_SET_APPROVAL_FOR_ALL: &str = "a22cb465";
/// Permit2 on-chain `approve(address,address,uint160,uint48)`.
const SEL_PERMIT2_APPROVE: &str = "87517c45";

// §37 (review 1-12): the calls that CARRY calls. An approval inside one is
// read as the account's own: whether the carrier keeps `msg.sender` (an
// OpenZeppelin / Uniswap `multicall` delegatecalls itself and does; a
// Multicall3 `aggregate` does not; a Safe's `multiSend` does when the Safe
// delegatecalls it) is the carrier's code to decide, not something the
// calldata says — so the guard errs toward seeing a grant, never toward
// missing one.
/// `multicall(bytes[])` — OpenZeppelin `Multicall`, Uniswap's routers: each
/// call made to the contract itself, as the sender.
pub const SEL_MULTICALL: &str = "ac9650d8";
/// `multicall(uint256,bytes[])` — the same, with a deadline.
pub const SEL_MULTICALL_DEADLINE: &str = "5ae401dc";
/// `multicall(bytes32,bytes[])` — the same, with a previous block hash.
pub const SEL_MULTICALL_BLOCKHASH: &str = "1f0464d1";
/// Multicall3 `aggregate((address,bytes)[])`.
pub const SEL_AGGREGATE: &str = "252dba42";
/// Multicall3 `tryAggregate(bool,(address,bytes)[])`.
pub const SEL_TRY_AGGREGATE: &str = "bce38bd7";
/// Multicall3 `blockAndAggregate((address,bytes)[])`.
pub const SEL_BLOCK_AND_AGGREGATE: &str = "c3077fa9";
/// Multicall3 `tryBlockAndAggregate(bool,(address,bytes)[])`.
pub const SEL_TRY_BLOCK_AND_AGGREGATE: &str = "399542e9";
/// Multicall3 `aggregate3((address,bool,bytes)[])`.
pub const SEL_AGGREGATE3: &str = "82ad56cb";
/// Multicall3 `aggregate3Value((address,bool,uint256,bytes)[])`.
pub const SEL_AGGREGATE3_VALUE: &str = "174dea71";
/// Safe `multiSend(bytes)` (MultiSend / MultiSendCallOnly): packed legs.
pub const SEL_MULTI_SEND: &str = "8d80ff0a";
/// How deep carried calls are read — a carrier inside a carrier inside a
/// carrier inside a carrier. Deeper than this no honest dApp goes; the
/// approvals found to that depth are every one the guard judges.
const MAX_NESTING: u8 = 4;
/// The most calls one carrier is read for.
const MAX_CARRIED: usize = 64;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardApprovalKind {
    Erc20Approve,
    IncreaseAllowance,
    DecreaseAllowance,
    SetApprovalForAll,
    Erc2612Permit,
    DaiPermit,
    Permit2Single,
    Permit2Batch,
}

/// Why editing is blocked (semantic — the shell owns the words).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardBlockReason {
    /// Off-chain permit — the dApp submits its OWN amount on-chain, so the
    /// wallet can't cap it (rewriting desyncs the signature and reverts the
    /// dApp's tx). Limit spending with an on-chain approval instead.
    OffChainPermit,
    /// DAI permit grants full-balance access; sign as requested or reject.
    DaiPermitFullBalance,
}

/// Where the amount lives, for the rewrite step.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardLocus {
    CalldataWord { word_index: u32 },
    TypedPath { path: String },
}

/// A detected approval-granting request. Amounts travel as DECIMAL STRINGS —
/// never JSON numbers (JS number precision loss is exactly the bug
/// `approval-guard.ts:359-361` exists to avoid).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardDetectedApproval {
    pub kind: GuardApprovalKind,
    /// ERC-20 token / NFT collection / Permit2 token address, if known.
    /// (TS uses `''` for an absent typed-data contract; every consumer treats
    /// that as falsy, so it maps to `None` here.)
    pub token_address: Option<String>,
    /// Who is being granted spending power.
    pub spender: String,
    /// Granted amount in raw base units (decimal string). `None` for boolean
    /// grants.
    pub amount_raw: Option<String>,
    /// 256 or 160.
    pub amount_bits: Option<u32>,
    /// Effectively unlimited (amount ≥ cap) OR a boolean grant-all of `true`.
    pub is_unbounded: bool,
    /// Boolean grant (setApprovalForAll / DAI allowed) — no amount to cap.
    pub is_boolean_grant: bool,
    /// Reduces risk (decreaseAllowance / revoke) — render as safe.
    pub is_reducing: bool,
    /// Whether the wallet can safely re-encode a finite amount for this shape.
    pub editable: bool,
    pub block_reason: Option<GuardBlockReason>,
    /// Expiry/deadline (unix seconds, decimal string), when the shape carries
    /// one.
    pub deadline: Option<String>,
    pub locus: GuardLocus,
    /// §37 (review 1-12): the approval is a call CARRIED by the one sent — a
    /// `multicall(bytes[])`, a Multicall3 `aggregate*`, a Safe `multiSend` —
    /// whose own calldata starts this many bytes into the request's data.
    /// [`GuardLocus::CalldataWord`]'s word counts from there, so a cap
    /// re-encodes exactly that word in place (the length never changes).
    /// `None`: the call itself. Absent on the wire reads `None`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub nested_at: Option<u32>,
}

/// The user's decision for one approval.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardChoice {
    /// A finite cap, raw base units as a decimal string.
    Amount { amount_raw: String },
    /// 0 / false.
    Revoke,
    /// Keep a boolean `true` — explicit and deliberate, setApprovalForAll /
    /// DAI permit only.
    Grant,
    /// Keep the dApp's own UNBOUNDED amount, byte for byte — nothing is
    /// re-encoded. What the "Requested" chip means on an unbounded request;
    /// never derived for a finite one (that is `Amount { requested }`).
    Unlimited,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do — RPC reads only; everything
/// else in this domain is pure.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "GuardOperation"))]
pub enum GuardOperation {
    /// symbol/decimals for each token, one Multicall3 round trip
    /// (`SigningSheet.tsx:220, 373`).
    ReadTokenMetadata { chain_id: u32, tokens: Vec<String> },
    /// `allowance(owner, spender)` — powers the increaseAllowance resulting
    /// total (`ApprovalView.tsx:47`).
    ReadErc20Allowance {
        chain_id: u32,
        token: String,
        owner: String,
        spender: String,
    },
    /// `balanceOf(owner)` — powers the one-tap finite "Balance" cap
    /// (issue #86; `ApprovalView.tsx:62`).
    ReadErc20Balance {
        chain_id: u32,
        token: String,
        owner: String,
    },
}

/// One token's on-chain metadata, as the shell read it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardTokenMetaEntry {
    /// Lowercased address, the map key.
    pub token: String,
    pub symbol: String,
    pub decimals: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "GuardShellResult"))]
pub enum GuardShellResult {
    /// `None` = the whole read failed; a token absent from `Some(list)` was
    /// simply not resolvable. The two produce different fallbacks (see module
    /// doc).
    MetaResolved {
        metas: Option<Vec<GuardTokenMetaEntry>>,
    },
    /// Raw allowance in base units (decimal string); `None` = read failed —
    /// the resulting-total row then still warns the increment ADDS to an
    /// existing allowance rather than hiding (`ApprovalView.tsx:164-167`).
    AllowanceRead { allowance: Option<String> },
    /// Raw balance (decimal string); `None` = read failed → no Balance preset.
    BalanceRead { balance: Option<String> },
}

impl Operation for GuardOperation {
    type Output = GuardShellResult;
}

#[effect]
pub enum GuardEffect {
    Render(RenderOperation),
    Shell(GuardOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardEditorMode {
    Requested,
    Balance,
    Custom,
    Revoke,
    Grant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "GuardEvent"))]
pub enum Event {
    /// A new signing request arrived. `params_json` is the raw JSON-RPC
    /// params array, verbatim and untrusted; `now_ms` is the shell's clock
    /// (the core owns none) for deadline classification.
    ApprovalDetected {
        method: String,
        params_json: String,
        chain_id: u32,
        wallet_address: Option<String>,
        read_only: bool,
        now_ms: f64,
    },
    /// A preset chip on the amount editor (`grant` is invalid here — boolean
    /// grants go through the deliberate events below).
    PresetSelected { mode: GuardEditorMode },
    /// The custom amount input changed. Already dot-normalized by the shell
    /// (`parseLocaleNumber`), as with payment_request.
    CustomAmountChanged { text: String },
    /// The deliberate tap on "grant all anyway" — never preselected
    /// (`EditableApproveCard.tsx:217`).
    GrantDeliberatelyChosen,
    /// The boolean card's revoke button.
    RevokeChosen,
    /// A preset chip on ONE batch leg's inline cap editor. The leg editors are
    /// this machine's too (`BatchCallsView` mounts the same card per leg);
    /// only the index travels, never a derived choice.
    LegPresetSelected { index: u32, mode: GuardEditorMode },
    /// One batch leg's custom amount input changed.
    LegCustomAmountChanged { index: u32, text: String },
    /// The deliberate "grant all anyway" tap on one batch leg.
    LegGrantDeliberatelyChosen { index: u32 },
    /// One batch leg's revoke button.
    LegRevokeChosen { index: u32 },
    /// The recipients the shell's descriptor pipeline resolved for each leg,
    /// in leg order (`ClearSignField { role: recipient }.address`). Raw data,
    /// not a verdict: the "sending a token to its own contract burns it" rule
    /// is decided here (`BatchCallsView.tsx:74-77`).
    BatchRecipientsResolved { recipients: Vec<Vec<String>> },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: GuardShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BoolPick {
    Grant,
    Revoke,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Editor {
    #[default]
    None,
    Amount {
        mode: GuardEditorMode,
        custom_text: String,
    },
    Boolean {
        selected: Option<BoolPick>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct TokenMetaInfo {
    symbol: String,
    decimals: u32,
    verified: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum MetaState {
    Loading,
    Resolved(Option<TokenMetaInfo>),
}

impl Default for MetaState {
    fn default() -> Self {
        MetaState::Resolved(None)
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
enum AllowanceState {
    #[default]
    NotApplicable,
    Loading,
    Resolved(Option<U256>),
}

#[derive(Clone, Debug, PartialEq)]
struct Leg {
    /// The original call object, kept verbatim for the confirm-time rebuild
    /// (`{ ...c, data: rw.data }` preserves value/capabilities).
    call: Value,
    to: String,
    detected: Option<GuardDetectedApproval>,
    /// This leg's inline cap editor. `Editor::None` for every leg that shows
    /// no editor (finite / reducing / non-approval legs, and every leg in a
    /// read-only replay) — exactly the legs `BatchCallsView` mounts no card
    /// for, so they can never acquire a choice and the confirm-time rebuild
    /// leaves them byte-identical.
    editor: Editor,
}

#[derive(Clone, Debug, PartialEq)]
enum BatchMeta {
    Loading { tokens: Vec<String> },
    Resolved(BTreeMap<String, TokenMetaInfo>),
}

#[derive(Clone, Debug, PartialEq)]
struct BatchState {
    legs: Vec<Leg>,
    meta: BatchMeta,
    /// Per-leg recipient addresses as the descriptor pipeline resolved them
    /// (empty until `BatchRecipientsResolved` arrives, which is exactly the
    /// "descriptors still loading → no banner" state today).
    recipients: Vec<Vec<String>>,
}

#[derive(Default)]
pub struct Model {
    method: String,
    params: Option<Value>,
    chain_id: u32,
    wallet: Option<String>,
    read_only: bool,
    now_ms: f64,
    detected: Option<GuardDetectedApproval>,
    editor: Editor,
    meta: MetaState,
    allowance: AllowanceState,
    balance: Option<U256>,
    batch: Option<BatchState>,
    /// Bumped per request; a result carrying an older attempt belongs to a
    /// superseded request (the previous sheet's slow metadata read) and is
    /// dropped — the "cancelled" flags of today's effects, centralized.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardSurface {
    /// Not an approval — this machine has nothing to gate.
    None,
    /// Off-chain permit: sign verbatim under deliberate consent, never the
    /// cap editor (`SigningSheet.tsx:421-423`).
    PermitSign,
    /// The editable never-unlimited spending-cap editor.
    ApprovalEditor,
    /// EIP-5792 per-leg breakdown.
    Batch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardTokenMetaView {
    pub symbol: String,
    pub decimals: u32,
    pub verified: bool,
    pub loading: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum GuardAmountError {
    InvalidAmount,
    /// A typed cap so large it is no cap at all (≥ 2^128, §37). The cap
    /// field limits; keeping the site's unlimited ask is the Requested chip.
    UnlimitedDisabled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardEditorView {
    /// `None` only on the boolean card before the deliberate tap.
    pub mode: Option<GuardEditorMode>,
    pub custom_text: String,
    pub error: Option<GuardAmountError>,
    /// `None` ⇒ confirm stays disabled (the whole point).
    pub choice: Option<GuardChoice>,
    /// What the value row shows (raw base units, decimal string).
    pub display_amount_raw: Option<String>,
    /// The "Requested" chip exists (a finite, non-zero incoming amount).
    pub requested_finite: bool,
    /// The "Requested" chip exists and keeps an UNBOUNDED amount as the site
    /// asked (choice [`GuardChoice::Unlimited`]). Exclusive with
    /// `requested_finite`; the chip is offered when either is true. A shell
    /// that reads this `true` with the chip selected says the allowance is
    /// unlimited, in the danger tone.
    pub requested_unlimited: bool,
    /// The one-tap finite Balance cap is offered (issue #86).
    pub has_balance_cap: bool,
    /// The Revoke chip is offered. Not on `increaseAllowance`: only the
    /// increment can be re-encoded, so "revoke" would sign an increase of 0
    /// and leave the existing allowance where it was — a chip that says one
    /// thing and signs another.
    pub revoke_offered: bool,
    pub balance_raw: Option<String>,
}

/// The increaseAllowance resulting-total row: "increase by 100" must never
/// read as "cap at 100" (`ApprovalView.tsx:143-171`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardIncreaseTotalView {
    /// On-chain allowance when the read succeeded.
    pub current: Option<String>,
    pub increment: String,
    /// `Some` = show "current + increment = total" (or plain `0` after a
    /// revoke); `None` = the read failed — still warn the increment ADDS to
    /// an existing allowance.
    pub total: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardLegView {
    pub to: String,
    pub approval: Option<GuardDetectedApproval>,
    pub meta: GuardTokenMetaView,
    /// This leg's inline cap editor, when one is shown. Same projection the
    /// single-approval surface gets — the leg card renders it, it derives
    /// nothing.
    pub editor: Option<GuardEditorView>,
    pub choice: Option<GuardChoice>,
    /// Show the inline cap editor (unbounded / grant-all only —
    /// `BatchCallsView.tsx:98`).
    pub needs_editor: bool,
    /// This leg still blocks confirm (`BatchCallsView.tsx:40-45`).
    pub needs_choice: bool,
    /// After the user's choice, still grants broad/unbounded access
    /// (`BatchCallsView.tsx:48-53`).
    pub grants_broad: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardBatchView {
    pub legs: Vec<GuardLegView>,
    /// The effective-state danger banner.
    pub any_uncapped: bool,
    /// A leg that sends a token to its OWN contract burns it — the same
    /// fat-finger the single-send path flags, easy to miss buried in a batch
    /// (F13, `BatchCallsView.tsx:74-77`).
    pub any_to_own_token: bool,
    pub all_settled: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct GuardView {
    pub surface: GuardSurface,
    pub detected: Option<GuardDetectedApproval>,
    pub meta: GuardTokenMetaView,
    pub editor: Option<GuardEditorView>,
    /// This machine's contribution to `confirmDisabled`
    /// (`SigningSheet.tsx:576-583`): `false` while an editable approval has
    /// no choice or a batch leg is unsettled.
    pub confirm_allowed: bool,
    /// The finite re-encode of the whole request, ready to submit. `None`
    /// when nothing was rewritten — including a rewrite failure, which fails
    /// CLOSED: the untouched params still hit [`enforce_no_unlimited`] at the
    /// submit chokepoint.
    pub rewritten_params_json: Option<String>,
    /// The request still grants an unbounded allowance after the person's
    /// choices — and it does so because this surface showed it and the
    /// Requested chip was left on (single approval, or any batch leg). The
    /// shell copies it to `SignApproveOpts::unlimited_approved`; it is the
    /// only thing that lets an unbounded amount past the submit guard, so a
    /// request this machine never saw still cannot carry one.
    pub unlimited_consented: bool,
    /// §37 (review 1-12): the batch legs (0-based) whose unbounded approval
    /// this surface showed and the person kept on Requested — each waives
    /// the submit guard for THAT leg's one approval and no other. The shell
    /// copies it to `SignApproveOpts::unlimited_approved_legs`; a batch is
    /// never waived by [`Self::unlimited_consented`] alone. Empty for a
    /// single request.
    #[serde(default)]
    pub unlimited_consented_legs: Vec<u32>,
    /// The request grants an unbounded allowance as it stands, and the
    /// person is told so in the danger tone — the ONE place every shell reads
    /// that sentence from (spec 094 S8): the single approval kept on its
    /// Requested chip, any batch leg left so, or an off-chain permit for an
    /// unbounded amount (EIP-2612, Permit2 `PermitSingle`/`PermitBatch`, a DAI
    /// `allowed: true`). A permit has no cap editor — the dApp redeems its OWN
    /// struct, so a capped signature would only revert its transaction
    /// ([`GuardBlockReason::OffChainPermit`]) — which is why, before this, an
    /// unlimited Permit2 drew a red hero and no warning at all (089 F22).
    pub unlimited_warning: bool,
    /// §37: the sentence [`Self::unlimited_warning`] says —
    /// [`UNLIMITED_WARNING_KEY`], or [`FAR_ABOVE_BALANCE_KEY`] for a finite
    /// amount at least [`FAR_ABOVE_BALANCE_TIMES`] times the balance read
    /// (unlimited-like: it is a number, and the sheet shows it). Every shell
    /// draws this key, never one of its own.
    #[serde(default = "unlimited_warning_key")]
    pub unlimited_warning_key: String,
    pub increase_total: Option<GuardIncreaseTotalView>,
    /// Unverified decimals must be explicitly flagged
    /// (`EditableApproveCard.tsx:200-202`; `PermitSignView.tsx:103-105`).
    pub decimals_unverified: bool,
    pub expired: bool,
    pub batch: Option<GuardBatchView>,
    /// §41 (P2-4): a call of the request carries calls this guard did not
    /// read — nested past its depth, more than it reads, or an encoding that
    /// does not hold together ([`carriers_unread`]). Not "no approval": the
    /// confirm is "… anyway" (`sign_confirm::guarded_posture`), and Refuse
    /// blind signing refuses it.
    #[serde(default)]
    pub carrier_unread: bool,
}

fn unlimited_warning_key() -> String {
    UNLIMITED_WARNING_KEY.to_owned()
}

/// §37: `amount` is at least [`FAR_ABOVE_BALANCE_TIMES`] times `balance` —
/// read, and not zero.
pub fn far_above_balance(amount: U256, balance: Option<U256>) -> bool {
    match balance {
        Some(balance) if !balance.is_zero() => {
            amount >= balance.saturating_mul(U256::from(FAR_ABOVE_BALANCE_TIMES))
        }
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ApprovalGuard;

impl App for ApprovalGuard {
    type Event = Event;
    type Model = Model;
    type ViewModel = GuardView;
    type Effect = GuardEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<GuardEffect, Event> {
        match event {
            Event::ApprovalDetected {
                method,
                params_json,
                chain_id,
                wallet_address,
                read_only,
                now_ms,
            } => start(
                model,
                method,
                &params_json,
                chain_id,
                wallet_address,
                read_only,
                now_ms,
            ),
            Event::PresetSelected { mode } => {
                let Some(detected) = model.detected.clone() else {
                    return Command::done();
                };
                let meta = effective_meta(model);
                let balance = model.balance;
                apply_preset(&mut model.editor, &detected, &meta, balance, mode)
            }
            Event::CustomAmountChanged { text } => set_custom_text(&mut model.editor, text),
            Event::GrantDeliberatelyChosen => set_bool_pick(&mut model.editor, BoolPick::Grant),
            Event::RevokeChosen => set_bool_pick(&mut model.editor, BoolPick::Revoke),
            Event::LegPresetSelected { index, mode } => {
                let Some((detected, meta)) = leg_context(model, index) else {
                    return Command::done();
                };
                let Some(leg) = leg_mut(model, index) else {
                    return Command::done();
                };
                // A batch leg card is mounted without a balance, so it never
                // offers the Balance chip (`BatchCallsView.tsx:107-116`).
                apply_preset(&mut leg.editor, &detected, &meta, None, mode)
            }
            Event::LegCustomAmountChanged { index, text } => match leg_mut(model, index) {
                Some(leg) => set_custom_text(&mut leg.editor, text),
                None => Command::done(),
            },
            Event::LegGrantDeliberatelyChosen { index } => match leg_mut(model, index) {
                Some(leg) => set_bool_pick(&mut leg.editor, BoolPick::Grant),
                None => Command::done(),
            },
            Event::LegRevokeChosen { index } => match leg_mut(model, index) {
                Some(leg) => set_bool_pick(&mut leg.editor, BoolPick::Revoke),
                None => Command::done(),
            },
            Event::BatchRecipientsResolved { recipients } => {
                let Some(batch) = &mut model.batch else {
                    return Command::done();
                };
                batch.recipients = recipients;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded request's slow read — dropping it is what
                    // today's 7 `cancelled` flags did, in one place.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> GuardView {
        if let Some(batch) = &model.batch {
            let batch_view = build_batch_view(model, batch);
            let confirm_allowed = batch_view.all_settled;
            let (rewritten_params_json, all_capped) =
                rewritten_batch_params(model, batch, &batch_view);
            // §37: consent is per leg — the legs whose own unbounded approval
            // was shown and kept, and only while every cap re-encoded.
            let unlimited_consented_legs: Vec<u32> = if all_capped {
                batch_view
                    .legs
                    .iter()
                    .enumerate()
                    .filter(|(_, leg)| matches!(leg.choice, Some(GuardChoice::Unlimited)))
                    .filter_map(|(i, _)| u32::try_from(i).ok())
                    .collect()
            } else {
                Vec::new()
            };
            let unlimited_consented = !unlimited_consented_legs.is_empty();
            let unlimited_warning = batch_view.any_uncapped;
            return GuardView {
                surface: GuardSurface::Batch,
                detected: None,
                meta: GuardTokenMetaView {
                    symbol: "…".to_owned(),
                    decimals: 18,
                    verified: false,
                    loading: matches!(batch.meta, BatchMeta::Loading { .. }),
                },
                editor: None,
                confirm_allowed,
                rewritten_params_json,
                unlimited_consented,
                unlimited_consented_legs,
                unlimited_warning,
                unlimited_warning_key: unlimited_warning_key(),
                increase_total: None,
                decimals_unverified: false,
                expired: false,
                batch: Some(batch_view),
                carrier_unread: carriers_unread(&model.method, model.params.as_ref()),
            };
        }

        let meta = effective_meta(model);
        let Some(detected) = &model.detected else {
            return GuardView {
                surface: GuardSurface::None,
                detected: None,
                meta,
                editor: None,
                confirm_allowed: true,
                rewritten_params_json: None,
                unlimited_consented: false,
                unlimited_consented_legs: Vec::new(),
                unlimited_warning: false,
                unlimited_warning_key: unlimited_warning_key(),
                increase_total: None,
                decimals_unverified: false,
                expired: false,
                batch: None,
                carrier_unread: carriers_unread(&model.method, model.params.as_ref()),
            };
        };

        let editor = derive_editor(&model.editor, detected, &meta, model.balance);
        let choice = editor.as_ref().and_then(|e| e.choice.clone());

        let surface = if matches!(detected.locus, GuardLocus::TypedPath { .. }) {
            GuardSurface::PermitSign
        } else {
            GuardSurface::ApprovalEditor
        };

        let confirm_allowed = !(detected.editable && choice.is_none());

        let unlimited_consented = matches!(choice, Some(GuardChoice::Unlimited));
        // Kept unbounded on the editor, or a permit nobody can cap — or
        // (§41, P2-4) a grant of a whole collection (`setApprovalForAll(true)`,
        // a DAI permit's `allowed`) not revoked: the same held danger it is
        // as a batch leg.
        let unbounded_kept = unlimited_consented
            || (surface == GuardSurface::PermitSign
                && detected.is_unbounded
                && !detected.is_reducing)
            || (detected.is_boolean_grant
                && detected.is_unbounded
                && !detected.is_reducing
                && !matches!(choice, Some(GuardChoice::Revoke)));
        // §37: a finite amount kept at a hundredfold the balance or more is
        // unlimited-like — said in the same tone, in its own words.
        let kept_amount = match &choice {
            Some(GuardChoice::Amount { amount_raw }) => parse_dec_u256(amount_raw),
            _ => None,
        };
        let far_above = !unbounded_kept
            && !detected.is_reducing
            && !detected.is_boolean_grant
            && kept_amount.is_some_and(|amount| far_above_balance(amount, model.balance));
        let unlimited_warning = unbounded_kept || far_above;
        let unlimited_warning_key = if far_above {
            FAR_ABOVE_BALANCE_KEY.to_owned()
        } else {
            unlimited_warning_key()
        };
        let rewritten_params_json = match (&choice, &model.params) {
            // Kept as asked: the site's own bytes go out, nothing to re-encode.
            (Some(GuardChoice::Unlimited), _) => None,
            (Some(choice), Some(params)) if detected.editable => {
                rewrite_approval_params(&model.method, params, detected, choice)
                    .ok()
                    .and_then(|value| serde_json::to_string(&value).ok())
            }
            _ => None,
        };

        // Unverified decimals are flagged on the amount editor always
        // (`EditableApproveCard.tsx:200-202`), and on the permit surface only
        // for a bounded amount (`PermitSignView.tsx:103-105`); the boolean
        // card scales no amount, so it has no warning to show.
        let decimals_unverified = match (&surface, &model.editor) {
            (GuardSurface::ApprovalEditor, Editor::Amount { .. }) => !meta.verified,
            (GuardSurface::PermitSign, _) => {
                !detected.is_boolean_grant && !detected.is_unbounded && !meta.verified
            }
            _ => false,
        };

        GuardView {
            surface,
            detected: Some(detected.clone()),
            expired: is_expired(detected, model.now_ms),
            increase_total: increase_total(model, detected, &choice),
            editor,
            confirm_allowed,
            rewritten_params_json,
            unlimited_consented,
            unlimited_consented_legs: Vec::new(),
            unlimited_warning,
            unlimited_warning_key,
            decimals_unverified,
            meta,
            batch: None,
            carrier_unread: carriers_unread(&model.method, model.params.as_ref()),
        }
    }
}

// ---------------------------------------------------------------------------
// Update handlers
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn start(
    model: &mut Model,
    method: String,
    params_json: &str,
    chain_id: u32,
    wallet_address: Option<String>,
    read_only: bool,
    now_ms: f64,
) -> Command<GuardEffect, Event> {
    model.attempt += 1;
    model.method = method;
    model.chain_id = chain_id;
    model.wallet = wallet_address;
    model.read_only = read_only;
    model.now_ms = now_ms;
    model.detected = None;
    model.editor = Editor::None;
    model.meta = MetaState::Resolved(None);
    model.allowance = AllowanceState::NotApplicable;
    model.balance = None;
    model.batch = None;
    model.params = serde_json::from_str(params_json).ok();

    let mut ops: Vec<GuardOperation> = Vec::new();

    if model.method == "wallet_sendCalls" {
        start_batch(model, &mut ops);
    } else if let Some(detected) = detect_approval(&model.method, model.params.as_ref()) {
        start_single(model, detected, &mut ops);
    }

    if ops.is_empty() {
        render()
    } else {
        requests(model, ops)
    }
}

fn start_batch(model: &mut Model, ops: &mut Vec<GuardOperation>) {
    let calls: Vec<Value> = match model
        .params
        .as_ref()
        .and_then(|p| p.get(0))
        .and_then(|first| first.get("calls"))
        .and_then(Value::as_array)
    {
        Some(calls) if !calls.is_empty() => calls.clone(),
        // `!Array.isArray(calls) || calls.length === 0` → no batch surface.
        _ => return,
    };

    let editable = !model.read_only;
    let legs: Vec<Leg> = calls
        .iter()
        .map(|call| {
            let detected = surfaced(calldata_approvals(
                call.get("to").and_then(Value::as_str),
                call.get("data").and_then(Value::as_str),
            ));
            Leg {
                to: call
                    .get("to")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
                // Only the legs that MOUNT a card get an editor — the same
                // `needsEditor` predicate the view reports, evaluated once at
                // detection so a finite leg can never acquire a choice (and so
                // the confirm-time rebuild leaves it byte-identical).
                editor: match &detected {
                    Some(d) if leg_shows_editor(editable, d) => init_editor(d),
                    _ => Editor::None,
                },
                detected,
                call: call.clone(),
            }
        })
        .collect();

    // Unique tokens, first-occurrence order (`Array.from(new Set(...))`).
    let mut tokens: Vec<String> = Vec::new();
    for leg in &legs {
        if let Some(token) = leg.detected.as_ref().and_then(|d| d.token_address.clone()) {
            if !tokens.contains(&token) {
                tokens.push(token);
            }
        }
    }

    let meta = if tokens.is_empty() {
        BatchMeta::Resolved(BTreeMap::new())
    } else {
        ops.push(GuardOperation::ReadTokenMetadata {
            chain_id: model.chain_id,
            tokens: tokens.clone(),
        });
        BatchMeta::Loading { tokens }
    };

    model.batch = Some(BatchState {
        legs,
        meta,
        recipients: Vec::new(),
    });
}

/// Show the inline cap editor for this leg? Unbounded / grant-all only —
/// a bounded approve is already capped (`BatchCallsView.tsx:98`).
fn leg_shows_editor(editable: bool, detected: &GuardDetectedApproval) -> bool {
    editable
        && detected.editable
        && !detected.is_reducing
        && (detected.is_unbounded || detected.is_boolean_grant)
}

/// The editor a freshly-detected calldata approval mounts with. Shared by the
/// single-approval surface and every batch leg, so "a grant-all preselects
/// nothing" and "an unbounded amount starts blank" are one rule.
fn init_editor(detected: &GuardDetectedApproval) -> Editor {
    if detected.is_boolean_grant {
        return Editor::Boolean {
            // A grant-all request preselects NOTHING — the deliberate tap is
            // the consent (`EditableApproveCard.tsx:217`). An incoming revoke
            // preselects the safe action.
            selected: if detected.is_unbounded {
                None
            } else {
                Some(BoolPick::Revoke)
            },
        };
    }
    let requested = parse_signed_dec(detected.amount_raw.as_deref().unwrap_or("0"));
    let requested_finite = !detected.is_unbounded && !requested.neg && !requested.mag.is_zero();
    if requested_finite {
        // Seeded with the 18-decimals fallback — metadata has not resolved
        // yet. Ported verbatim (module doc quirks).
        Editor::Amount {
            mode: GuardEditorMode::Requested,
            custom_text: format_token_amount(requested.mag, 18, 6, "", ".", false),
        }
    } else if detected.is_unbounded {
        // Kept as the site asked (module doc, 2026-09-26): re-encoding it by
        // default is what reverted Permit2 bundles. The cap field starts
        // blank — there is no finite number to seed it with.
        Editor::Amount {
            mode: GuardEditorMode::Requested,
            custom_text: String::new(),
        }
    } else {
        // approve(spender, 0) — nothing requested, nothing to keep.
        Editor::Amount {
            mode: GuardEditorMode::Custom,
            custom_text: String::new(),
        }
    }
}

fn start_single(model: &mut Model, detected: GuardDetectedApproval, ops: &mut Vec<GuardOperation>) {
    if let Some(token) = detected.token_address.clone() {
        model.meta = MetaState::Loading;
        ops.push(GuardOperation::ReadTokenMetadata {
            chain_id: model.chain_id,
            tokens: vec![token.clone()],
        });

        if detected.kind == GuardApprovalKind::IncreaseAllowance {
            if let Some(owner) = model.wallet.clone() {
                model.allowance = AllowanceState::Loading;
                ops.push(GuardOperation::ReadErc20Allowance {
                    chain_id: model.chain_id,
                    token: token.clone(),
                    owner,
                    spender: detected.spender.clone(),
                });
            }
        }

        // The balance read fires for every calldata approval except NFTs —
        // including decreaseAllowance, where the preset is then suppressed
        // (ported verbatim, `ApprovalView.tsx:57-66`).
        if matches!(detected.locus, GuardLocus::CalldataWord { .. })
            && detected.kind != GuardApprovalKind::SetApprovalForAll
        {
            if let Some(owner) = model.wallet.clone() {
                ops.push(GuardOperation::ReadErc20Balance {
                    chain_id: model.chain_id,
                    token,
                    owner,
                });
            }
        }
    }

    if matches!(detected.locus, GuardLocus::CalldataWord { .. }) {
        model.editor = init_editor(&detected);
    }

    model.detected = Some(detected);
}

/// The leg's identity for a preset press: its detection + the metadata its
/// card is currently rendering with (decimals drive the re-seed).
fn leg_context(model: &Model, index: u32) -> Option<(GuardDetectedApproval, GuardTokenMetaView)> {
    let batch = model.batch.as_ref()?;
    let leg = batch.legs.get(index as usize)?;
    let detected = leg.detected.clone()?;
    Some((detected.clone(), leg_meta(batch, &detected)))
}

fn leg_mut(model: &mut Model, index: u32) -> Option<&mut Leg> {
    model.batch.as_mut()?.legs.get_mut(index as usize)
}

/// Typing a cap IS choosing a custom cap. An unbounded request now opens on
/// Requested, so a keystroke that left the mode there would be a number on
/// screen the choice ignores.
fn set_custom_text(editor: &mut Editor, text: String) -> Command<GuardEffect, Event> {
    if let Editor::Amount { mode, custom_text } = editor {
        *mode = GuardEditorMode::Custom;
        *custom_text = text;
        render()
    } else {
        Command::done()
    }
}

fn set_bool_pick(editor: &mut Editor, pick: BoolPick) -> Command<GuardEffect, Event> {
    if let Editor::Boolean { selected } = editor {
        *selected = Some(pick);
        render()
    } else {
        Command::done()
    }
}

/// Revoke is a real revoke only where the amount word IS the allowance.
fn revoke_offered(detected: &GuardDetectedApproval) -> bool {
    detected.kind != GuardApprovalKind::IncreaseAllowance
}

fn apply_preset(
    editor: &mut Editor,
    detected: &GuardDetectedApproval,
    meta: &GuardTokenMetaView,
    balance: Option<U256>,
    mode: GuardEditorMode,
) -> Command<GuardEffect, Event> {
    let requested = parse_signed_dec(detected.amount_raw.as_deref().unwrap_or("0"));
    let requested_finite = !detected.is_unbounded && !requested.neg && !requested.mag.is_zero();
    let card_reducing = detected.kind == GuardApprovalKind::DecreaseAllowance;
    let has_balance_cap = !card_reducing && balance.is_some_and(|b| !b.is_zero());

    // The boolean card (setApprovalForAll / DAI permit) answers only Revoke
    // from the chip row — the shells draw one row for both cards, and this is
    // what made the NFT card's 撤销 do nothing.
    if let Editor::Boolean { selected } = editor {
        if mode == GuardEditorMode::Revoke {
            *selected = Some(BoolPick::Revoke);
            return render();
        }
        return Command::done();
    }
    let Editor::Amount {
        mode: current,
        custom_text,
    } = editor
    else {
        return Command::done();
    };
    match mode {
        // A preset press re-seeds the input with the CURRENT decimals, so a
        // later switch to Custom starts from the accepted value.
        GuardEditorMode::Requested if requested_finite => {
            *current = GuardEditorMode::Requested;
            *custom_text = format_token_amount(requested.mag, meta.decimals, 6, "", ".", false);
        }
        // Back to the site's unlimited ask; the cap field keeps whatever the
        // person typed, so switching to Custom again loses nothing.
        GuardEditorMode::Requested if detected.is_unbounded => {
            *current = GuardEditorMode::Requested;
        }
        GuardEditorMode::Balance if has_balance_cap => {
            *current = GuardEditorMode::Balance;
            if let Some(balance) = balance {
                *custom_text = format_token_amount(balance, meta.decimals, 6, "", ".", false);
            }
        }
        GuardEditorMode::Custom => *current = GuardEditorMode::Custom,
        GuardEditorMode::Revoke if revoke_offered(detected) => *current = GuardEditorMode::Revoke,
        // `grant` is not an amount-card mode, and a chip that isn't rendered
        // can't be pressed.
        _ => return Command::done(),
    }
    render()
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: GuardShellResult) -> Command<GuardEffect, Event> {
    match result {
        GuardShellResult::MetaResolved { metas } => {
            if let Some(batch) = &mut model.batch {
                let BatchMeta::Loading { tokens } = &batch.meta else {
                    return Command::done();
                };
                let map: BTreeMap<String, TokenMetaInfo> = match metas {
                    // The whole read failed → empty map; legs render the
                    // `…`/18/unverified defaults (`SigningSheet.tsx:385`).
                    None => BTreeMap::new(),
                    Some(list) => tokens
                        .iter()
                        .map(|token| {
                            let info = list
                                .iter()
                                .find(|entry| entry.token.eq_ignore_ascii_case(token))
                                .map(|entry| TokenMetaInfo {
                                    symbol: entry.symbol.clone(),
                                    decimals: entry.decimals,
                                    verified: true,
                                })
                                .unwrap_or_else(|| fallback_meta(token));
                            (token.clone(), info)
                        })
                        .collect(),
                };
                batch.meta = BatchMeta::Resolved(map);
                return render();
            }
            if model.meta != MetaState::Loading {
                return Command::done();
            }
            let token = model
                .detected
                .as_ref()
                .and_then(|d| d.token_address.clone())
                .unwrap_or_default();
            let info = metas
                .and_then(|list| {
                    list.iter()
                        .find(|entry| entry.token.eq_ignore_ascii_case(&token))
                        .map(|entry| TokenMetaInfo {
                            symbol: entry.symbol.clone(),
                            decimals: entry.decimals,
                            verified: true,
                        })
                })
                .unwrap_or_else(|| fallback_meta(&token));
            model.meta = MetaState::Resolved(Some(info));
            render()
        }
        GuardShellResult::AllowanceRead { allowance } => {
            if model.allowance != AllowanceState::Loading {
                return Command::done();
            }
            model.allowance =
                AllowanceState::Resolved(allowance.as_deref().and_then(parse_dec_u256));
            render()
        }
        GuardShellResult::BalanceRead { balance } => {
            if model.detected.is_none() || model.batch.is_some() {
                return Command::done();
            }
            model.balance = balance.as_deref().and_then(parse_dec_u256);
            render()
        }
    }
}

/// `{ symbol: addr.slice(0, 6)…, decimals: 18, verified: false }` — the
/// single-token fallback (`SigningSheet.tsx:219`).
fn fallback_meta(token: &str) -> TokenMetaInfo {
    let head: String = token.chars().take(6).collect();
    TokenMetaInfo {
        symbol: format!("{head}…"),
        decimals: 18,
        verified: false,
    }
}

// ---------------------------------------------------------------------------
// View derivation
// ---------------------------------------------------------------------------

fn effective_meta(model: &Model) -> GuardTokenMetaView {
    match &model.meta {
        MetaState::Loading => GuardTokenMetaView {
            symbol: "…".to_owned(),
            decimals: 18,
            verified: false,
            loading: true,
        },
        MetaState::Resolved(None) => GuardTokenMetaView {
            symbol: "…".to_owned(),
            decimals: 18,
            verified: false,
            loading: false,
        },
        MetaState::Resolved(Some(info)) => GuardTokenMetaView {
            symbol: info.symbol.clone(),
            decimals: info.decimals,
            verified: info.verified,
            loading: false,
        },
    }
}

/// The mode → choice derivation, ported from
/// `EditableApproveCard.tsx:85-107` (amount card) and `:217-221` (boolean
/// card). `choice == None` is what keeps confirm disabled.
fn derive_editor(
    editor: &Editor,
    detected: &GuardDetectedApproval,
    meta: &GuardTokenMetaView,
    balance: Option<U256>,
) -> Option<GuardEditorView> {
    match editor {
        Editor::None => None,
        Editor::Boolean { selected } => Some(GuardEditorView {
            mode: selected.map(|pick| match pick {
                BoolPick::Grant => GuardEditorMode::Grant,
                BoolPick::Revoke => GuardEditorMode::Revoke,
            }),
            custom_text: String::new(),
            error: None,
            choice: selected.map(|pick| match pick {
                BoolPick::Grant => GuardChoice::Grant,
                BoolPick::Revoke => GuardChoice::Revoke,
            }),
            display_amount_raw: None,
            requested_finite: false,
            requested_unlimited: false,
            has_balance_cap: false,
            revoke_offered: true,
            balance_raw: None,
        }),
        Editor::Amount { mode, custom_text } => {
            let requested = parse_signed_dec(detected.amount_raw.as_deref().unwrap_or("0"));
            let requested_finite =
                !detected.is_unbounded && !requested.neg && !requested.mag.is_zero();
            // The card's `isReducing` is by KIND (decrease only), unlike the
            // detection flag which also covers approve-to-0.
            let card_reducing = detected.kind == GuardApprovalKind::DecreaseAllowance;
            let has_balance_cap = !card_reducing && balance.is_some_and(|b| !b.is_zero());
            let bits = bits_from_wire(detected.amount_bits);

            let requested_unlimited = detected.is_unbounded && !detected.is_boolean_grant;

            let (choice, error, display) = match mode {
                GuardEditorMode::Revoke => (Some(GuardChoice::Revoke), None, Some(U256::ZERO)),
                // No display amount: the value row says "Unlimited", never
                // 2^256-1 spelled out in token units.
                GuardEditorMode::Requested if requested_unlimited => {
                    (Some(GuardChoice::Unlimited), None, None)
                }
                GuardEditorMode::Requested => (
                    Some(GuardChoice::Amount {
                        amount_raw: requested.mag.to_string(),
                    }),
                    None,
                    Some(requested.mag),
                ),
                GuardEditorMode::Balance if has_balance_cap => {
                    let balance = balance.unwrap_or(U256::ZERO);
                    (
                        Some(GuardChoice::Amount {
                            amount_raw: balance.to_string(),
                        }),
                        None,
                        Some(balance),
                    )
                }
                // Balance without a cap falls through to the custom
                // evaluation, exactly as the TS `useMemo` does.
                GuardEditorMode::Balance | GuardEditorMode::Custom | GuardEditorMode::Grant => {
                    let trimmed = custom_text.trim();
                    if trimmed.is_empty() {
                        (None, None, None)
                    } else {
                        match parse_token_amount(trimmed, meta.decimals) {
                            None => (None, Some(GuardAmountError::InvalidAmount), None),
                            Some(raw) if is_unbounded_amount(raw, bits) => {
                                // custom ≥ cap → NO choice, confirm stays
                                // disabled, the error names why.
                                (None, Some(GuardAmountError::UnlimitedDisabled), Some(raw))
                            }
                            Some(raw) => (
                                Some(GuardChoice::Amount {
                                    amount_raw: raw.to_string(),
                                }),
                                None,
                                Some(raw),
                            ),
                        }
                    }
                }
            };

            Some(GuardEditorView {
                mode: Some(*mode),
                custom_text: custom_text.clone(),
                error,
                choice,
                display_amount_raw: display.map(|d| d.to_string()),
                requested_finite,
                requested_unlimited,
                has_balance_cap,
                revoke_offered: revoke_offered(detected),
                balance_raw: balance.map(|b| b.to_string()),
            })
        }
    }
}

fn increase_total(
    model: &Model,
    detected: &GuardDetectedApproval,
    choice: &Option<GuardChoice>,
) -> Option<GuardIncreaseTotalView> {
    if detected.kind != GuardApprovalKind::IncreaseAllowance {
        return None;
    }
    // The row appears only once the read RESOLVED (either way) — never a
    // half-rendered total while the RPC is in flight.
    let AllowanceState::Resolved(current) = &model.allowance else {
        return None;
    };
    if matches!(choice, Some(GuardChoice::Revoke)) {
        // Revoke zeroes the allowance outright — the increment math no longer
        // applies (`ApprovalView.tsx:146-155`).
        return Some(GuardIncreaseTotalView {
            current: None,
            increment: "0".to_owned(),
            total: Some("0".to_owned()),
        });
    }
    let increment = match choice {
        Some(GuardChoice::Amount { amount_raw }) => {
            parse_dec_u256(amount_raw).unwrap_or(U256::ZERO)
        }
        _ => {
            let requested = parse_signed_dec(detected.amount_raw.as_deref().unwrap_or("0"));
            if requested.neg {
                U256::ZERO
            } else {
                requested.mag
            }
        }
    };
    match current {
        Some(current) => Some(GuardIncreaseTotalView {
            current: Some(current.to_string()),
            increment: increment.to_string(),
            total: Some(
                current
                    .checked_add(increment)
                    .unwrap_or(U256::MAX)
                    .to_string(),
            ),
        }),
        None => Some(GuardIncreaseTotalView {
            current: None,
            increment: increment.to_string(),
            total: None,
        }),
    }
}

fn is_expired(detected: &GuardDetectedApproval, now_ms: f64) -> bool {
    // `Number(approval.deadline)` — a bigint→Number conversion, precision
    // loss included, ported verbatim.
    let deadline_sec = detected
        .deadline
        .as_deref()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0);
    deadline_sec > 0.0 && deadline_sec < (now_ms / 1000.0).floor()
}

/// The metadata one leg's card renders with — the batch map when it resolved,
/// the `…`/18/unverified default otherwise (`SigningSheet.tsx:373-385`).
fn leg_meta(batch: &BatchState, detected: &GuardDetectedApproval) -> GuardTokenMetaView {
    let meta_loading = matches!(batch.meta, BatchMeta::Loading { .. });
    detected
        .token_address
        .as_ref()
        .and_then(|token| match &batch.meta {
            BatchMeta::Resolved(map) => map.get(token),
            BatchMeta::Loading { .. } => None,
        })
        .map(|info| GuardTokenMetaView {
            symbol: info.symbol.clone(),
            decimals: info.decimals,
            verified: info.verified,
            loading: false,
        })
        .unwrap_or(GuardTokenMetaView {
            symbol: "…".to_owned(),
            decimals: 18,
            verified: false,
            loading: meta_loading,
        })
}

fn build_batch_view(model: &Model, batch: &BatchState) -> GuardBatchView {
    let editable = !model.read_only;
    let meta_loading = matches!(batch.meta, BatchMeta::Loading { .. });

    let legs: Vec<GuardLegView> = batch
        .legs
        .iter()
        .map(|leg| {
            let approval = leg.detected.as_ref();
            let meta = leg
                .detected
                .as_ref()
                .map(|detected| leg_meta(batch, detected))
                .unwrap_or(GuardTokenMetaView {
                    symbol: "…".to_owned(),
                    decimals: 18,
                    verified: false,
                    loading: meta_loading,
                });
            // A leg card carries no balance, so it never offers the Balance
            // chip — matching today's `<EditableApproveCard>` per leg, which
            // is mounted without `balanceRaw`.
            let editor =
                approval.and_then(|detected| derive_editor(&leg.editor, detected, &meta, None));
            let choice = editor.as_ref().and_then(|e| e.choice.clone());
            GuardLegView {
                to: leg.to.clone(),
                approval: leg.detected.clone(),
                meta,
                needs_editor: approval.is_some_and(|a| leg_shows_editor(editable, a)),
                needs_choice: leg_needs_choice(approval, choice.as_ref()),
                grants_broad: leg_grants_broad(approval, choice.as_ref()),
                editor,
                choice,
            }
        })
        .collect();

    // Banner reflects the EFFECTIVE state when editable; a read-only replay
    // flags the raw request (`BatchCallsView.tsx:69-71`).
    let any_uncapped = if editable {
        legs.iter().any(|leg| leg.grants_broad)
    } else {
        batch.legs.iter().any(|leg| {
            leg.detected
                .as_ref()
                .is_some_and(|a| a.is_unbounded && !a.is_reducing && !a.is_boolean_grant)
        })
    };
    let all_settled = !legs.iter().any(|leg| leg.needs_choice);
    // `!!to && fields.some(f => f.role === 'recipient' && f.address === to)`
    // — a leg whose token is being sent to the token's own contract.
    let any_to_own_token = batch
        .legs
        .iter()
        .zip(batch.recipients.iter())
        .any(|(leg, rs)| {
            !leg.to.is_empty()
                && rs
                    .iter()
                    .any(|recipient| recipient.eq_ignore_ascii_case(&leg.to))
        });

    GuardBatchView {
        legs,
        any_uncapped,
        any_to_own_token,
        all_settled,
    }
}

/// Does this batch approval leg still need a deliberate decision before the
/// bundle can be confirmed? Finite amounts are pre-accepted — editing them is
/// optional — and so, since 2026-09-26, is an unbounded amount (its editor
/// starts on Requested = [`GuardChoice::Unlimited`]). Only a grant-all still
/// waits for the deliberate tap. Port of `BatchCallsView.tsx:40-45`.
pub fn leg_needs_choice(
    approval: Option<&GuardDetectedApproval>,
    choice: Option<&GuardChoice>,
) -> bool {
    let Some(ap) = approval else { return false };
    if !ap.editable || ap.is_reducing {
        return false;
    }
    if ap.is_boolean_grant {
        return choice.is_none();
    }
    if ap.is_unbounded {
        return !matches!(
            choice,
            Some(GuardChoice::Amount { .. })
                | Some(GuardChoice::Revoke)
                | Some(GuardChoice::Unlimited)
        );
    }
    false
}

/// After the user's choice, does this leg still grant broad/unbounded access?
/// Port of `BatchCallsView.tsx:48-53`.
pub fn leg_grants_broad(
    approval: Option<&GuardDetectedApproval>,
    choice: Option<&GuardChoice>,
) -> bool {
    let Some(ap) = approval else { return false };
    if ap.is_reducing {
        return false;
    }
    if ap.is_boolean_grant {
        return matches!(choice, Some(GuardChoice::Grant)) || choice.is_none();
    }
    if ap.is_unbounded {
        return !matches!(
            choice,
            Some(GuardChoice::Amount { .. }) | Some(GuardChoice::Revoke)
        );
    }
    false
}

/// The confirm-time batch rebuild (`SigningSheet.tsx:531-549`): re-encode
/// each leg the user capped/revoked; a per-leg rewrite failure keeps the
/// original call, which the per-leg submit guard then refuses — fail closed.
/// Also reports whether every leg the person CAPPED was actually re-encoded:
/// a leg whose rewrite failed goes out with the site's bytes, so the bundle
/// must not carry the unlimited consent a different leg earned — the submit
/// guard then refuses it (fail closed, per leg, with a request-wide flag).
fn rewritten_batch_params(
    model: &Model,
    batch: &BatchState,
    view: &GuardBatchView,
) -> (Option<String>, bool) {
    let mut all_capped = true;
    let rebuilt = rebuild_batch(model, batch, view, &mut all_capped);
    (rebuilt, all_capped)
}

fn rebuild_batch(
    model: &Model,
    batch: &BatchState,
    view: &GuardBatchView,
    all_capped: &mut bool,
) -> Option<String> {
    let params = model.params.as_ref()?;
    let mut changed = false;
    let new_calls: Vec<Value> = batch
        .legs
        .iter()
        .zip(view.legs.iter())
        .map(|(leg, projected)| {
            if let (Some(detected), Some(choice)) = (&leg.detected, &projected.choice) {
                // Kept as asked — the leg goes out byte-identical, which is
                // the whole point: the next leg spends against it.
                if detected.editable && *choice != GuardChoice::Unlimited {
                    if let Some(data) = leg.call.get("data").and_then(Value::as_str) {
                        if let Ok(new_data) = rewrite_calldata(data, detected, choice) {
                            let mut call = leg.call.clone();
                            if let Some(map) = call.as_object_mut() {
                                map.insert("data".to_owned(), Value::String(new_data));
                                changed = true;
                                return call;
                            }
                        }
                    }
                    *all_capped = false;
                }
            }
            leg.call.clone()
        })
        .collect();
    if !changed {
        return None;
    }
    let mut out = params.clone();
    let first = out.get_mut(0)?.as_object_mut()?;
    first.insert("calls".to_owned(), Value::Array(new_calls));
    serde_json::to_string(&out).ok()
}

// ---------------------------------------------------------------------------
// Detection (pure) — `approval-guard.ts:115-274`
// ---------------------------------------------------------------------------

/// Detect an approval-granting request from raw `(method, params)`.
/// Returns `None` when the request grants no spending power.
pub fn detect_approval(method: &str, params: Option<&Value>) -> Option<GuardDetectedApproval> {
    let arr = params?.as_array()?;
    if arr.is_empty() {
        return None;
    }

    if method == "eth_sendTransaction" {
        let tx = arr.first()?;
        if !tx.is_object() {
            return None;
        }
        return surfaced(calldata_approvals(
            tx.get("to").and_then(Value::as_str),
            tx.get("data").and_then(Value::as_str),
        ));
    }

    if crate::typed_data_request::looks_like_typed_data(method) {
        // The request's ONE document, read strictly (audit 2026-10-01): the
        // legacy order put the address at `params[1]`, so a legacy Permit was
        // never seen, and `[permit, benign]` was judged by the benign one.
        let read = crate::typed_data_request::canonical(method, params?).ok()?;
        return detect_typed_data_approval(&read.document);
    }

    None
}

pub fn detect_calldata_approval(
    to: Option<&str>,
    data: Option<&str>,
) -> Option<GuardDetectedApproval> {
    let data = data?;
    if data.is_empty() || data == "0x" {
        return None;
    }
    let stripped = data.strip_prefix("0x").unwrap_or(data);
    if !stripped.is_ascii() {
        // `BigInt('0x…')` would throw mid-render on such bytes; they can
        // never be submitted, so "no approval" is the honest classification.
        return None;
    }
    let hex = stripped.to_ascii_lowercase();
    if hex.len() < 8 {
        return None;
    }
    let selector = hex.get(..8).unwrap_or("");
    let word = |i: usize| -> &str {
        let start = (8 + i * 64).min(hex.len());
        let end = (start + 64).min(hex.len());
        hex.get(start..end).unwrap_or("")
    };
    let addr_from_word = |w: &str| -> String {
        let tail = w.get(24.min(w.len())..).unwrap_or("");
        format!("0x{tail}")
    };
    let token_from_to = || to.map(str::to_lowercase);

    match selector {
        SEL_APPROVE => {
            // approve(address spender, uint256 amount). Same selector as
            // ERC-721 approve(operator, tokenId) — a tokenId is never ≥ cap,
            // so capping is still safe; metadata resolution upstream refines
            // ERC-20 vs NFT display.
            let spender = addr_from_word(word(0));
            let amount = big_from_word(word(1))?;
            Some(GuardDetectedApproval {
                kind: GuardApprovalKind::Erc20Approve,
                token_address: token_from_to(),
                spender,
                amount_raw: Some(amount.to_string()),
                amount_bits: Some(256),
                is_unbounded: is_unbounded_amount(amount, AmountBits::B256),
                is_boolean_grant: false,
                is_reducing: amount.is_zero(),
                editable: true,
                block_reason: None,
                deadline: None,
                locus: GuardLocus::CalldataWord { word_index: 1 },
                nested_at: None,
            })
        }
        SEL_INCREASE_ALLOWANCE => {
            let spender = addr_from_word(word(0));
            let amount = big_from_word(word(1))?;
            Some(GuardDetectedApproval {
                kind: GuardApprovalKind::IncreaseAllowance,
                token_address: token_from_to(),
                spender,
                amount_raw: Some(amount.to_string()),
                amount_bits: Some(256),
                is_unbounded: is_unbounded_amount(amount, AmountBits::B256),
                is_boolean_grant: false,
                is_reducing: false,
                editable: true,
                block_reason: None,
                deadline: None,
                locus: GuardLocus::CalldataWord { word_index: 1 },
                nested_at: None,
            })
        }
        SEL_DECREASE_ALLOWANCE => {
            let spender = addr_from_word(word(0));
            let amount = big_from_word(word(1))?;
            Some(GuardDetectedApproval {
                kind: GuardApprovalKind::DecreaseAllowance,
                token_address: token_from_to(),
                spender,
                amount_raw: Some(amount.to_string()),
                amount_bits: Some(256),
                is_unbounded: false,
                is_boolean_grant: false,
                is_reducing: true,
                editable: true,
                block_reason: None,
                deadline: None,
                locus: GuardLocus::CalldataWord { word_index: 1 },
                nested_at: None,
            })
        }
        SEL_PERMIT2_APPROVE => {
            // Permit2 on-chain approve(token, spender, uint160 amount, uint48
            // expiration). The token is the FIRST arg (not the tx `to`, which
            // is the Permit2 contract), and the amount is a uint160 — same
            // unlimited sentinel width as PermitSingle.
            let token = addr_from_word(word(0));
            let spender = addr_from_word(word(1));
            let amount = big_from_word(word(2))?;
            let deadline = big_from_word(word(3))?;
            Some(GuardDetectedApproval {
                kind: GuardApprovalKind::Permit2Single,
                token_address: Some(token).filter(|t| t != "0x"),
                spender,
                amount_raw: Some(amount.to_string()),
                amount_bits: Some(160),
                is_unbounded: is_unbounded_amount(amount, AmountBits::B160),
                is_boolean_grant: false,
                is_reducing: amount.is_zero(),
                editable: true,
                block_reason: None,
                deadline: Some(deadline.to_string()),
                locus: GuardLocus::CalldataWord { word_index: 2 },
                nested_at: None,
            })
        }
        SEL_SET_APPROVAL_FOR_ALL => {
            let operator = addr_from_word(word(0));
            let approved = !big_from_word(word(1))?.is_zero();
            Some(GuardDetectedApproval {
                kind: GuardApprovalKind::SetApprovalForAll,
                token_address: token_from_to(),
                spender: operator,
                amount_raw: None,
                amount_bits: None,
                is_unbounded: approved,
                is_boolean_grant: true,
                is_reducing: !approved,
                // No finite amount exists; the only safe rewrite is revoke.
                // Granting is allowed but only via an explicit, deliberate
                // confirmation in the UI.
                editable: true,
                block_reason: None,
                deadline: None,
                locus: GuardLocus::CalldataWord { word_index: 1 },
                nested_at: None,
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Carried calls (§37, review 1-12): approvals inside multicall / MultiSend
// ---------------------------------------------------------------------------

/// Every approval `data` sent to `to` grants: the call itself when it is one,
/// else — depth-first, to [`MAX_NESTING`] carriers — each call it carries
/// that is ([`SEL_MULTICALL`] and kin, Multicall3, `multiSend`). A carried
/// approval is marked [`GuardDetectedApproval::nested_at`], so its cap
/// re-encodes its own word where it sits. Empty when nothing grants.
pub fn calldata_approvals(to: Option<&str>, data: Option<&str>) -> Vec<GuardDetectedApproval> {
    calldata_approvals_read(to, data).0
}

/// [`calldata_approvals`], and — §41 (P2-4) — whether a carrier went
/// unread: one nested past [`MAX_NESTING`], one carrying more than
/// [`MAX_CARRIED`] calls, or one whose encoding does not hold together. Such
/// a call is not "no approval": the guard could not read it, and the confirm
/// is "… anyway" ([`GuardView::carrier_unread`]).
pub fn calldata_approvals_read(
    to: Option<&str>,
    data: Option<&str>,
) -> (Vec<GuardDetectedApproval>, bool) {
    let Some(data) = data else {
        return (Vec::new(), false);
    };
    let body = data.strip_prefix("0x").unwrap_or(data);
    if !body.is_ascii() || body.len() % 2 != 0 || !body.bytes().all(|b| b.is_ascii_hexdigit()) {
        // A top-level shape still reads as it always did (it tolerates what
        // it can); carried calls are read only from bytes that parse.
        return (
            detect_calldata_approval(to, Some(data))
                .into_iter()
                .collect(),
            false,
        );
    }
    let Ok(bytes) = crate::primitives::from_hex(body) else {
        return (Vec::new(), false);
    };
    let mut out = Vec::new();
    let mut unread = false;
    walk_calls(to, &bytes, 0, bytes.len(), 0, &mut out, &mut unread);
    (out, unread)
}

/// §41 (P2-4): a call of the request (`eth_sendTransaction`'s, or any
/// `wallet_sendCalls` leg's) carries calls this guard could not read
/// ([`calldata_approvals_read`]).
#[must_use]
pub fn carriers_unread(method: &str, params: Option<&Value>) -> bool {
    let Some(first) = params.and_then(Value::as_array).and_then(|a| a.first()) else {
        return false;
    };
    let unread = |call: &Value| {
        calldata_approvals_read(
            call.get("to").and_then(Value::as_str),
            call.get("data").and_then(Value::as_str),
        )
        .1
    };
    match method {
        "eth_sendTransaction" => unread(first),
        "wallet_sendCalls" => first
            .get("calls")
            .and_then(Value::as_array)
            .is_some_and(|calls| calls.iter().any(unread)),
        _ => false,
    }
}

/// The approval the sheet surfaces from `approvals` (one per call): the
/// first that grants broadly (an unbounded amount, a grant-all), else the
/// first that grants at all, else the first.
pub fn surfaced(approvals: Vec<GuardDetectedApproval>) -> Option<GuardDetectedApproval> {
    let broad = approvals
        .iter()
        .position(|a| a.is_unbounded && !a.is_reducing);
    let granting = approvals.iter().position(|a| !a.is_reducing);
    let at = broad.or(granting).unwrap_or(0);
    approvals.into_iter().nth(at)
}

fn walk_calls(
    to: Option<&str>,
    all: &[u8],
    start: usize,
    len: usize,
    depth: u8,
    out: &mut Vec<GuardDetectedApproval>,
    unread: &mut bool,
) {
    let Some(call) = all.get(start..start.saturating_add(len)) else {
        return;
    };
    let hex = crate::primitives::to_hex(call, true);
    if let Some(mut found) = detect_calldata_approval(to, Some(&hex)) {
        if start > 0 {
            let Ok(at) = u32::try_from(start) else {
                return;
            };
            found.nested_at = Some(at);
        }
        out.push(found);
        // An approval carries nothing.
        return;
    }
    let Ok(carried) = carried_calls(call) else {
        // A carrier whose calls do not read (too many, or an encoding that
        // does not hold together): not "no approval" — unread.
        *unread = true;
        return;
    };
    if depth >= MAX_NESTING {
        // Past the depth this guard follows, a carrier still carrying calls
        // is a call it did not read.
        *unread |= !carried.is_empty();
        return;
    }
    for inner in carried {
        let target = inner.to.as_deref().or(to);
        walk_calls(
            target,
            all,
            start + inner.start,
            inner.len,
            depth + 1,
            out,
            unread,
        );
    }
}

/// One call a carrier carries: where its calldata sits in the carrier's
/// (bytes, from the carrier's selector), and whom it is made to — `None`:
/// the carrier itself (a `multicall` calls its own contract).
struct Carried {
    to: Option<String>,
    start: usize,
    len: usize,
}

fn usize_at(bytes: &[u8], at: usize) -> Option<usize> {
    let word = bytes.get(at..at.checked_add(32)?)?;
    let n = U256::from_be_slice(word);
    usize::try_from(n).ok().filter(|n| *n <= bytes.len())
}

fn address_at(bytes: &[u8], at: usize) -> Option<String> {
    let word = bytes.get(at..at.checked_add(32)?)?;
    word[..12]
        .iter()
        .all(|b| *b == 0)
        .then(|| crate::primitives::to_hex(&word[12..], true).to_lowercase())
}

/// `bytes` whose length word is at `at`: where its content starts, and how long.
fn bytes_at(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let len = usize_at(bytes, at)?;
    let start = at.checked_add(32)?;
    (start.checked_add(len)? <= bytes.len()).then_some((start, len))
}

/// The element heads of a dynamic array whose length word is at `at`: each
/// element's absolute position (element offsets count from the first head).
fn array_elements(bytes: &[u8], at: usize) -> Option<Vec<usize>> {
    let n = usize_at(bytes, at)?;
    if n > MAX_CARRIED {
        return None;
    }
    let heads = at.checked_add(32)?;
    (0..n)
        .map(|i| {
            let off = usize_at(bytes, heads.checked_add(i.checked_mul(32)?)?)?;
            heads.checked_add(off)
        })
        .collect()
}

/// The calls `call` (a selector and its arguments) carries, if it is a
/// carrier this guard reads; empty when it is none. `Err` (§41): a carrier
/// whose encoding does not hold together, or that carries more than
/// [`MAX_CARRIED`] calls — what it carries was not read.
fn carried_calls(call: &[u8]) -> Result<Vec<Carried>, ()> {
    const ARGS: usize = 4;
    let Some(selector) = call.get(..ARGS) else {
        return Ok(Vec::new());
    };
    let selector = crate::primitives::to_hex(selector, false).to_lowercase();
    // `bytes[]` at argument `arg`: each to the carrier itself.
    let self_calls = |arg: usize| -> Option<Vec<Carried>> {
        let array = ARGS.checked_add(usize_at(call, ARGS + arg * 32)?)?;
        array_elements(call, array)?
            .into_iter()
            .map(|el| {
                let (start, len) = bytes_at(call, el)?;
                Some(Carried {
                    to: None,
                    start,
                    len,
                })
            })
            .collect()
    };
    // `(address target, …, bytes callData)[]` at argument `arg`, the bytes'
    // offset in word `data_word` of each tuple.
    let tuples = |arg: usize, data_word: usize| -> Option<Vec<Carried>> {
        let array = ARGS.checked_add(usize_at(call, ARGS + arg * 32)?)?;
        array_elements(call, array)?
            .into_iter()
            .map(|tuple| {
                let to = address_at(call, tuple)?;
                let data = tuple.checked_add(usize_at(call, tuple + data_word * 32)?)?;
                let (start, len) = bytes_at(call, data)?;
                Some(Carried {
                    to: Some(to),
                    start,
                    len,
                })
            })
            .collect()
    };
    let multi_send = || -> Option<Vec<Carried>> {
        let at = ARGS.checked_add(usize_at(call, ARGS)?)?;
        let (payload, len) = bytes_at(call, at)?;
        let mut out = Vec::new();
        let mut i = 0usize;
        while i + 85 <= len {
            let leg = payload + i;
            let to = crate::primitives::to_hex(call.get(leg + 1..leg + 21)?, true).to_lowercase();
            let data_len = usize_at(call, leg + 53)?;
            let start = leg + 85;
            let end = start.checked_add(data_len)?;
            if end > payload + len || out.len() >= MAX_CARRIED {
                return None;
            }
            out.push(Carried {
                to: Some(to),
                start,
                len: data_len,
            });
            i = end - payload;
        }
        Some(out)
    };
    match selector.as_str() {
        SEL_MULTICALL => self_calls(0),
        SEL_MULTICALL_DEADLINE | SEL_MULTICALL_BLOCKHASH => self_calls(1),
        SEL_AGGREGATE | SEL_BLOCK_AND_AGGREGATE => tuples(0, 1),
        SEL_TRY_AGGREGATE | SEL_TRY_BLOCK_AND_AGGREGATE => tuples(1, 1),
        SEL_AGGREGATE3 => tuples(0, 2),
        SEL_AGGREGATE3_VALUE => tuples(0, 3),
        SEL_MULTI_SEND => multi_send(),
        _ => return Ok(Vec::new()),
    }
    .ok_or(())
}

fn detect_typed_data_approval(td: &Value) -> Option<GuardDetectedApproval> {
    if !td.is_object() {
        return None;
    }
    let pt = td.get("primaryType").and_then(Value::as_str).unwrap_or("");
    let empty = Value::Object(serde_json::Map::new());
    let msg = match td.get("message") {
        None | Some(Value::Null) => &empty,
        Some(value) => value,
    };
    let domain = match td.get("domain") {
        None | Some(Value::Null) => &empty,
        Some(value) => value,
    };

    // OFF-CHAIN PERMIT SIGNATURES (everything below) are redeemed by the
    // dApp, which submits its OWN permit struct on-chain. The wallet only
    // signs — it can't change what the dApp submits — so rewriting the signed
    // amount would desync the signature from the on-chain struct and revert
    // the dApp's tx (the classic "signed the Permit2, but Uniswap's swap
    // fails" bug). Hence NOT editable: surfaced as a risk and signed verbatim
    // under explicit, deliberate consent, never silently capped. (On-chain
    // `approve` calldata stays editable — there we DO control the bytes.)

    // DAI-style permit: Permit(holder, spender, nonce, expiry, allowed)
    if pt == "Permit" && msg.get("allowed").is_some() {
        let allowed = is_js_one_or_true(msg.get("allowed"));
        return Some(GuardDetectedApproval {
            kind: GuardApprovalKind::DaiPermit,
            token_address: nonempty(lc(domain.get("verifyingContract"))),
            spender: lc(msg.get("spender")),
            amount_raw: None,
            amount_bits: None,
            is_unbounded: allowed,
            is_boolean_grant: true,
            is_reducing: !allowed,
            editable: false,
            block_reason: allowed.then_some(GuardBlockReason::DaiPermitFullBalance),
            deadline: Some(to_big(msg.get("expiry")).to_dec()),
            locus: GuardLocus::TypedPath {
                path: "allowed".to_owned(),
            },
            nested_at: None,
        });
    }

    // ERC-2612 permit: Permit(owner, spender, value, nonce, deadline)
    if pt == "Permit" && msg.get("value").is_some() {
        let amount = to_big(msg.get("value"));
        return Some(GuardDetectedApproval {
            kind: GuardApprovalKind::Erc2612Permit,
            token_address: nonempty(lc(domain.get("verifyingContract"))),
            spender: lc(msg.get("spender")),
            is_unbounded: !amount.neg && amount.mag >= UNLIMITED_CAP_256,
            is_reducing: amount.is_zero(),
            amount_raw: Some(amount.to_dec()),
            amount_bits: Some(256),
            is_boolean_grant: false,
            editable: false,
            block_reason: Some(GuardBlockReason::OffChainPermit),
            deadline: Some(to_big(msg.get("deadline")).to_dec()),
            locus: GuardLocus::TypedPath {
                path: "value".to_owned(),
            },
            nested_at: None,
        });
    }

    // Permit2 PermitSingle: { details: {token, amount(uint160), expiration,
    // nonce}, spender, sigDeadline }
    if pt == "PermitSingle" && js_truthy(msg.get("details")) {
        let details = msg.get("details").unwrap_or(&empty);
        let amount = to_big(details.get("amount"));
        return Some(GuardDetectedApproval {
            kind: GuardApprovalKind::Permit2Single,
            token_address: nonempty(lc(details.get("token"))),
            spender: lc(msg.get("spender")),
            is_unbounded: !amount.neg && amount.mag >= UNLIMITED_CAP_160,
            is_reducing: amount.is_zero(),
            amount_raw: Some(amount.to_dec()),
            amount_bits: Some(160),
            is_boolean_grant: false,
            editable: false,
            block_reason: Some(GuardBlockReason::OffChainPermit),
            deadline: Some(to_big(details.get("expiration")).to_dec()),
            locus: GuardLocus::TypedPath {
                path: "details.amount".to_owned(),
            },
            nested_at: None,
        });
    }

    // Permit2 PermitBatch: details[].
    if pt == "PermitBatch" {
        if let Some(list) = msg.get("details").and_then(Value::as_array) {
            let any_unbounded = list.iter().any(|d| {
                let amount = to_big(d.get("amount"));
                !amount.neg && amount.mag >= UNLIMITED_CAP_160
            });
            return Some(GuardDetectedApproval {
                kind: GuardApprovalKind::Permit2Batch,
                token_address: None,
                spender: lc(msg.get("spender")),
                amount_raw: None,
                amount_bits: Some(160),
                is_unbounded: any_unbounded,
                is_boolean_grant: false,
                is_reducing: false,
                editable: false,
                block_reason: Some(GuardBlockReason::OffChainPermit),
                deadline: None,
                locus: GuardLocus::TypedPath {
                    path: "details".to_owned(),
                },
                nested_at: None,
            });
        }
    }

    // Permit2 SignatureTransfer — a ONE-SHOT authorization for `spender` to
    // take up to `permitted.amount` (uint256) of the token, redeemable by the
    // spender until `deadline`. It moves the coins itself, so it is at least as
    // dangerous as an allowance; the audit of 2026-10-01 found the guard had
    // no rule for it. Surfaced under the Permit2 kinds (no new kind: every
    // shell decodes the kind, and an unknown one refuses the whole view), never
    // editable, signed verbatim only under the off-chain-permit consent.
    // §37 (review 1-12): the WITNESS forms (`PermitWitnessTransferFrom`,
    // `PermitBatchWitnessTransferFrom`) are the same authorization with the
    // dApp's own struct (an order, an intent) bound into the signature — what
    // UniswapX and most intent protocols ask for, and what a drainer asks for
    // when the plain form would be recognised. They fell through to blind
    // typed data; they are read exactly as the plain forms.
    if (pt == "PermitTransferFrom" || pt == "PermitWitnessTransferFrom")
        && js_truthy(msg.get("permitted"))
    {
        let permitted = msg.get("permitted").unwrap_or(&empty);
        let amount = to_big(permitted.get("amount"));
        return Some(GuardDetectedApproval {
            kind: GuardApprovalKind::Permit2Single,
            token_address: nonempty(lc(permitted.get("token"))),
            spender: lc(msg.get("spender")),
            is_unbounded: !amount.neg && amount.mag >= UNLIMITED_CAP_256,
            is_reducing: amount.is_zero(),
            amount_raw: Some(amount.to_dec()),
            amount_bits: Some(256),
            is_boolean_grant: false,
            editable: false,
            block_reason: Some(GuardBlockReason::OffChainPermit),
            deadline: Some(to_big(msg.get("deadline")).to_dec()),
            locus: GuardLocus::TypedPath {
                path: "permitted.amount".to_owned(),
            },
            nested_at: None,
        });
    }
    if pt == "PermitBatchTransferFrom" || pt == "PermitBatchWitnessTransferFrom" {
        if let Some(list) = msg.get("permitted").and_then(Value::as_array) {
            let any_unbounded = list.iter().any(|p| {
                let amount = to_big(p.get("amount"));
                !amount.neg && amount.mag >= UNLIMITED_CAP_256
            });
            return Some(GuardDetectedApproval {
                kind: GuardApprovalKind::Permit2Batch,
                token_address: None,
                spender: lc(msg.get("spender")),
                amount_raw: None,
                amount_bits: Some(256),
                is_unbounded: any_unbounded,
                is_boolean_grant: false,
                is_reducing: false,
                editable: false,
                block_reason: Some(GuardBlockReason::OffChainPermit),
                deadline: Some(to_big(msg.get("deadline")).to_dec()),
                locus: GuardLocus::TypedPath {
                    path: "permitted".to_owned(),
                },
                nested_at: None,
            });
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Rewrite (pure) — `approval-guard.ts:281-362`
// ---------------------------------------------------------------------------

/// Why a rewrite (or a choice) was refused. Semantic — one variant per
/// distinct TS `throw`; the shell owns any wording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GuardRewriteError {
    /// `Cannot rewrite approval for this request`
    UnsupportedMethod,
    /// `not a calldata approval` / `not a typed-data approval`
    WrongLocus,
    /// Structurally unusable input (missing tx data / unparseable typed JSON)
    /// — TS surfaces these as generic runtime throws.
    MalformedParams,
    /// `calldata too short to rewrite`
    CalldataTooShort,
    /// `setApprovalForAll has no amount to set` / `DAI permit has no amount
    /// to set`
    AmountForBooleanShape,
    /// `grant choice is only valid for boolean approvals`
    GrantForAmountShape,
    /// `amount must be non-negative` (also: an unparseable wire amount)
    InvalidChoiceAmount,
    /// `Unlimited approvals are disabled — choose a finite amount.`
    UnlimitedAmount,
    /// `rewrite path not found`
    PathNotFound,
    /// `rewrite changed calldata length`
    RewriteChangedLength,
    /// `rewrite altered a byte outside the amount word`
    RewriteTouchedOtherBytes,
}

/// Produce a NEW params array with the chosen finite amount. Never mutates
/// the input. Errors if it would emit an unbounded allowance.
pub fn rewrite_approval_params(
    method: &str,
    params: &Value,
    detected: &GuardDetectedApproval,
    choice: &GuardChoice,
) -> Result<Value, GuardRewriteError> {
    let arr = params
        .as_array()
        .ok_or(GuardRewriteError::MalformedParams)?;

    if method == "eth_sendTransaction" {
        let tx = arr.first().ok_or(GuardRewriteError::MalformedParams)?;
        let data = tx
            .get("data")
            .and_then(Value::as_str)
            .ok_or(GuardRewriteError::MalformedParams)?;
        let new_data = rewrite_calldata(data, detected, choice)?;
        let mut new_tx = tx.clone();
        new_tx
            .as_object_mut()
            .ok_or(GuardRewriteError::MalformedParams)?
            .insert("data".to_owned(), Value::String(new_data));
        let mut out = arr.clone();
        if let Some(slot) = out.first_mut() {
            *slot = new_tx;
        }
        return Ok(Value::Array(out));
    }

    if crate::typed_data_request::looks_like_typed_data(method) {
        // The document's own slot for the method (audit 2026-10-01).
        let idx = crate::typed_data_request::TypedDataMethod::of(method)
            .ok_or(GuardRewriteError::MalformedParams)?
            .document_index();
        let raw = arr.get(idx).ok_or(GuardRewriteError::MalformedParams)?;
        let was_string = matches!(raw, Value::String(_));
        let mut td = match raw {
            Value::String(s) => {
                serde_json::from_str::<Value>(s).map_err(|_| GuardRewriteError::MalformedParams)?
            }
            other => other.clone(),
        };
        rewrite_typed_data(&mut td, detected, choice)?;
        let mut out = arr.clone();
        if let Some(slot) = out.get_mut(idx) {
            *slot = if was_string {
                Value::String(
                    serde_json::to_string(&td).map_err(|_| GuardRewriteError::MalformedParams)?,
                )
            } else {
                td
            };
        }
        return Ok(Value::Array(out));
    }

    Err(GuardRewriteError::UnsupportedMethod)
}

fn chosen_amount(
    detected: &GuardDetectedApproval,
    choice: &GuardChoice,
) -> Result<U256, GuardRewriteError> {
    match choice {
        GuardChoice::Revoke => Ok(U256::ZERO),
        GuardChoice::Grant => {
            if !detected.is_boolean_grant {
                return Err(GuardRewriteError::GrantForAmountShape);
            }
            Ok(U256::from(1u64)) // boolean true
        }
        // The site's own unbounded word, re-encoded to itself — the view
        // never asks for this rewrite (it sends the original bytes), but the
        // pure function stays total: "keep it" is an identity, and it is
        // refused for a finite request, where it would mean nothing.
        GuardChoice::Unlimited => {
            if !detected.is_unbounded || detected.is_boolean_grant {
                return Err(GuardRewriteError::InvalidChoiceAmount);
            }
            detected
                .amount_raw
                .as_deref()
                .and_then(parse_dec_u256)
                .ok_or(GuardRewriteError::InvalidChoiceAmount)
        }
        GuardChoice::Amount { amount_raw } => {
            // The wire carries decimal digits only; a negative or garbled
            // amount is the TS `amountRaw < 0n` refusal.
            let amount =
                parse_dec_u256(amount_raw).ok_or(GuardRewriteError::InvalidChoiceAmount)?;
            if is_unbounded_amount(amount, bits_from_wire(detected.amount_bits)) {
                return Err(GuardRewriteError::UnlimitedAmount);
            }
            Ok(amount)
        }
    }
}

pub fn rewrite_calldata(
    data: &str,
    detected: &GuardDetectedApproval,
    choice: &GuardChoice,
) -> Result<String, GuardRewriteError> {
    let GuardLocus::CalldataWord { word_index } = &detected.locus else {
        return Err(GuardRewriteError::WrongLocus);
    };
    let hex = data.strip_prefix("0x").unwrap_or(data);
    if !hex.is_ascii() {
        return Err(GuardRewriteError::MalformedParams);
    }
    // §37: a carried approval's word counts from its own selector.
    let call_start = detected.nested_at.map_or(0, |at| at as usize * 2);
    let word_start = call_start + 8 + (*word_index as usize) * 64;
    let word_end = word_start + 64;
    if hex.len() < word_end {
        return Err(GuardRewriteError::CalldataTooShort);
    }
    // The call the approval IS must still read as that approval, where it
    // is: a carried offset is never trusted to point at a word blind.
    let at_call = hex
        .get(call_start..)
        .ok_or(GuardRewriteError::CalldataTooShort)?;
    let still = detect_calldata_approval(detected.token_address.as_deref(), Some(at_call));
    if still.as_ref().map(|d| (d.kind, &d.locus)) != Some((detected.kind, &detected.locus)) {
        return Err(GuardRewriteError::WrongLocus);
    }
    let selector = hex.get(..8).ok_or(GuardRewriteError::CalldataTooShort)?;

    let new_word = if detected.kind == GuardApprovalKind::SetApprovalForAll {
        // boolean: grant → true, revoke → false. No "amount" to cap.
        match choice {
            GuardChoice::Amount { .. } | GuardChoice::Unlimited => {
                return Err(GuardRewriteError::AmountForBooleanShape)
            }
            GuardChoice::Grant => format!("{:064x}", U256::from(1u64)),
            GuardChoice::Revoke => format!("{:064x}", U256::ZERO),
        }
    } else {
        format!("{:064x}", chosen_amount(detected, choice)?)
    };

    let head = hex
        .get(8..word_start)
        .ok_or(GuardRewriteError::CalldataTooShort)?;
    let tail = hex
        .get(word_end..)
        .ok_or(GuardRewriteError::CalldataTooShort)?;
    let out = format!("0x{selector}{head}{new_word}{tail}");

    // Round-trip safety: assert only the intended word changed.
    assert_only_word_changed(data, &out, word_start, word_end)?;
    Ok(out)
}

fn rewrite_typed_data(
    td: &mut Value,
    detected: &GuardDetectedApproval,
    choice: &GuardChoice,
) -> Result<(), GuardRewriteError> {
    let GuardLocus::TypedPath { path } = &detected.locus else {
        return Err(GuardRewriteError::WrongLocus);
    };

    if detected.kind == GuardApprovalKind::DaiPermit {
        if matches!(choice, GuardChoice::Amount { .. } | GuardChoice::Unlimited) {
            return Err(GuardRewriteError::AmountForBooleanShape);
        }
        let allowed = matches!(choice, GuardChoice::Grant);
        return set_path(td, path, Value::Bool(allowed));
    }

    // amount-bearing typed data — store as a DECIMAL STRING (avoid JS number
    // precision loss).
    let amount = chosen_amount(detected, choice)?;
    set_path(td, path, Value::String(amount.to_string()))
}

/// Set a dot-path on `td.message` (e.g. `"details.amount"`), failing loudly
/// if absent — a silent no-op would sign the ORIGINAL unbounded struct.
fn set_path(td: &mut Value, path: &str, value: Value) -> Result<(), GuardRewriteError> {
    let mut cur = td
        .get_mut("message")
        .ok_or(GuardRewriteError::PathNotFound)?;
    let parts: Vec<&str> = path.split('.').collect();
    let (leaf, walk) = parts.split_last().ok_or(GuardRewriteError::PathNotFound)?;
    for part in walk {
        cur = cur.get_mut(*part).ok_or(GuardRewriteError::PathNotFound)?;
    }
    let map = cur.as_object_mut().ok_or(GuardRewriteError::PathNotFound)?;
    if !map.contains_key(*leaf) {
        return Err(GuardRewriteError::PathNotFound);
    }
    map.insert((*leaf).to_owned(), value);
    Ok(())
}

/// Assert that exactly the `[start, end)` hex window changed between two
/// calldatas.
fn assert_only_word_changed(
    before: &str,
    after: &str,
    start: usize,
    end: usize,
) -> Result<(), GuardRewriteError> {
    let a = before.strip_prefix("0x").unwrap_or(before);
    let b = after.strip_prefix("0x").unwrap_or(after);
    if a.len() != b.len() {
        return Err(GuardRewriteError::RewriteChangedLength);
    }
    for (i, (ca, cb)) in a.chars().zip(b.chars()).enumerate() {
        if i >= start && i < end {
            continue;
        }
        if ca != cb {
            return Err(GuardRewriteError::RewriteTouchedOtherBytes);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// enforceNoUnlimited (pure) — the independent, descriptor-free submit guard
// (`approval-guard.ts:376-395`; chokepoints `use-dapp-signing.ts:364,
// 413-415`)
// ---------------------------------------------------------------------------

/// The refusal: this request would grant an unbounded allowance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardUnlimitedApproval {
    pub kind: GuardApprovalKind,
    pub amount_raw: String,
}

/// Errors if the FINAL request would grant an unbounded allowance — in the
/// call itself or in any call it carries (§37). The submit chokepoint
/// (`sign_request::proceed_submit`) waives ONE such approval per call, and
/// only when the approval surface reported the person kept it
/// ([`enforce_within_consent`]); everything else — a shell that never
/// mounted the surface, a shape no descriptor decodes — is refused here.
pub fn enforce_no_unlimited(
    method: &str,
    params: Option<&Value>,
) -> Result<(), GuardUnlimitedApproval> {
    enforce_within_consent(method, params, false)
}

/// The grants the FINAL request makes that the submit guard judges: every
/// unbounded amount on a transaction the wallet submits, carried calls
/// included. Off-chain permit SIGNATURES (typed data) are redeemed by the
/// dApp with its OWN struct — the wallet can't cap what it doesn't submit,
/// so a forced cap only desyncs the signature and reverts the dApp's tx;
/// those are gated by the sheet's explicit risk consent, not this guard.
/// Boolean grants and reductions carry no amount to judge.
fn unbounded_grants(method: &str, params: Option<&Value>) -> Vec<GuardUnlimitedApproval> {
    if method != "eth_sendTransaction" {
        return Vec::new();
    }
    let Some(tx) = params.and_then(Value::as_array).and_then(|a| a.first()) else {
        return Vec::new();
    };
    calldata_approvals(
        tx.get("to").and_then(Value::as_str),
        tx.get("data").and_then(Value::as_str),
    )
    .into_iter()
    .filter(|d| !d.is_boolean_grant && !d.is_reducing)
    .filter_map(|d| {
        let amount_raw = d.amount_raw.clone()?;
        let bits = d.amount_bits?;
        let amount = parse_signed_dec(&amount_raw);
        (!amount.neg && is_unbounded_amount(amount.mag, bits_from_wire(Some(bits)))).then_some(
            GuardUnlimitedApproval {
                kind: d.kind,
                amount_raw,
            },
        )
    })
    .collect()
}

/// §37 (review 1-12): [`enforce_no_unlimited`] with the person's consent for
/// this one call. `consented` waives exactly ONE unbounded approval — the
/// one the sheet surfaces for a call ([`surfaced`]) — and never a second:
/// a call that grants two cannot have shown both.
pub fn enforce_within_consent(
    method: &str,
    params: Option<&Value>,
    consented: bool,
) -> Result<(), GuardUnlimitedApproval> {
    let grants = unbounded_grants(method, params);
    let waived = usize::from(consented);
    match grants.into_iter().nth(waived) {
        Some(refused) => Err(refused),
        None => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Pure money text — `approval-guard.ts:446-488`
// ---------------------------------------------------------------------------

/// Parse a human token amount (e.g. `"1,234.5"`) into raw base units for
/// `decimals`. Returns `None` on invalid input. Pure; no locale assumptions
/// beyond `'.'`/`','`. Values ≥ 2^256 saturate at `U256::MAX` (≥ every cap —
/// same classification the TS bigint reaches).
pub fn parse_token_amount(human: &str, decimals: u32) -> Option<U256> {
    let cleaned: String = human.replace(',', "").trim().to_owned();
    if cleaned.is_empty() {
        return None;
    }
    // `^\d*\.?\d*$`
    let mut dots = 0usize;
    for b in cleaned.bytes() {
        match b {
            b'0'..=b'9' => {}
            b'.' => dots += 1,
            _ => return None,
        }
    }
    if dots > 1 {
        return None;
    }
    let (whole, frac) = cleaned.split_once('.').unwrap_or((cleaned.as_str(), ""));
    if frac.len() > decimals as usize {
        return None; // more precision than the token has
    }
    let mut frac_padded = frac.to_owned();
    while frac_padded.len() < decimals as usize {
        frac_padded.push('0');
    }
    let whole_units = parse_dec_saturating(if whole.is_empty() { "0" } else { whole });
    let frac_units = parse_dec_saturating(if frac_padded.is_empty() {
        "0"
    } else {
        &frac_padded
    });
    let scale = U256::from(10u64)
        .checked_pow(U256::from(decimals))
        .unwrap_or(U256::MAX);
    Some(
        whole_units
            .checked_mul(scale)
            .unwrap_or(U256::MAX)
            .checked_add(frac_units)
            .unwrap_or(U256::MAX),
    )
}

/// Group the integer part with thousands separators (no `Intl`) so a
/// permit/approve amount reads `1,000` like every other amount, not `1000`
/// (F4). Indian grouping is 2-2-3.
fn group_thousands(digits: &str, group: &str, indian: bool) -> String {
    if digits.len() <= 3 {
        return digits.to_owned();
    }
    let insert_every = |s: &str, step: usize| -> String {
        let mut out = String::with_capacity(s.len() * 2);
        let len = s.len();
        for (i, c) in s.chars().enumerate() {
            if i > 0 && (len - i) % step == 0 {
                out.push_str(group);
            }
            out.push(c);
        }
        out
    };
    if !indian {
        return insert_every(digits, 3);
    }
    let split = digits.len().saturating_sub(3);
    let head = digits.get(..split).unwrap_or("");
    let tail = digits.get(split..).unwrap_or(digits);
    format!("{}{group}{tail}", insert_every(head, 2))
}

/// Format raw base units back to a human string for `decimals` (trims zeros).
/// `group`/`decimal` localize WITHOUT casting to a float — defaults in the TS
/// original reproduce the canonical `1,234.5`.
pub fn format_token_amount(
    raw: U256,
    decimals: u32,
    max_frac: u32,
    group: &str,
    decimal: &str,
    indian: bool,
) -> String {
    let digits = raw.to_string();
    if decimals == 0 {
        return group_thousands(&digits, group, indian);
    }
    let decimals = decimals as usize;
    let (whole, frac) = if digits.len() > decimals {
        let split = digits.len() - decimals;
        (
            digits.get(..split).unwrap_or("0").to_owned(),
            digits.get(split..).unwrap_or("").to_owned(),
        )
    } else {
        let mut frac = String::new();
        for _ in 0..(decimals - digits.len()) {
            frac.push('0');
        }
        frac.push_str(&digits);
        ("0".to_owned(), frac)
    };
    let whole_str = group_thousands(&whole, group, indian);
    if frac.bytes().all(|b| b == b'0') {
        return whole_str;
    }
    let frac_str: String = frac
        .chars()
        .take(max_frac as usize)
        .collect::<String>()
        .trim_end_matches('0')
        .to_owned();
    if frac_str.is_empty() {
        whole_str
    } else {
        format!("{whole_str}{decimal}{frac_str}")
    }
}

// ---------------------------------------------------------------------------
// Number plumbing
// ---------------------------------------------------------------------------

/// A signed big value — only detection needs the sign (a hostile typed-data
/// `value: "-1"` is a negative bigint in TS: not unbounded, not reducing).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BigVal {
    neg: bool,
    mag: U256,
}

impl BigVal {
    const ZERO: BigVal = BigVal {
        neg: false,
        mag: U256::ZERO,
    };

    fn is_zero(self) -> bool {
        self.mag.is_zero()
    }

    fn to_dec(self) -> String {
        if self.neg && !self.mag.is_zero() {
            format!("-{}", self.mag)
        } else {
            self.mag.to_string()
        }
    }
}

/// `toBig` (`approval-guard.ts:405-416`): anything unconvertible is 0, never
/// a throw. Exotic `String(v)` coercions of arrays/objects evaluate to 0
/// (documented deviation).
fn to_big(v: Option<&Value>) -> BigVal {
    let Some(v) = v else { return BigVal::ZERO };
    match v {
        Value::Null => BigVal::ZERO,
        Value::Number(n) => {
            if let Some(u) = n.as_u64() {
                BigVal {
                    neg: false,
                    mag: U256::from(u),
                }
            } else if let Some(i) = n.as_i64() {
                BigVal {
                    neg: i < 0,
                    mag: U256::from(i.unsigned_abs()),
                }
            } else if let Some(f) = n.as_f64() {
                f64_to_big(f)
            } else {
                BigVal::ZERO
            }
        }
        Value::String(s) => str_to_big(s.trim()),
        _ => BigVal::ZERO,
    }
}

/// `BigInt(Math.trunc(x))` — exact float→integer conversion (a float is
/// always an exact integer once ≥ 2^53).
fn f64_to_big(f: f64) -> BigVal {
    if !f.is_finite() {
        return BigVal::ZERO; // BigInt(NaN/Infinity) throws → caught → 0n
    }
    let t = f.trunc();
    if t == 0.0 {
        return BigVal::ZERO;
    }
    let neg = t < 0.0;
    let a = t.abs();
    if a < 9_007_199_254_740_992.0 {
        // < 2^53: the cast is exact.
        return BigVal {
            neg,
            mag: U256::from(a as u64),
        };
    }
    let bits = a.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64 - 1075;
    let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    if !(0..256).contains(&exponent) {
        // ≥ 2^256 saturates (≥ every cap — same classification).
        return BigVal {
            neg,
            mag: U256::MAX,
        };
    }
    let mag = U256::from(mantissa)
        .checked_shl(exponent as usize)
        .unwrap_or(U256::MAX);
    BigVal { neg, mag }
}

/// JS `StringToBigInt`: decimal with an optional leading `-`, or unsigned
/// `0x`/`0b`/`0o` literals. Anything else → 0 (the TS `catch`).
fn str_to_big(s: &str) -> BigVal {
    if s.is_empty() {
        return BigVal::ZERO;
    }
    let radix: Option<(u32, &str)> = if let Some(body) = strip_prefix_ci(s, "0x") {
        Some((16, body))
    } else if let Some(body) = strip_prefix_ci(s, "0b") {
        Some((2, body))
    } else if let Some(body) = strip_prefix_ci(s, "0o") {
        Some((8, body))
    } else {
        None
    };
    if let Some((radix, body)) = radix {
        if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
            return BigVal::ZERO;
        }
        return BigVal {
            neg: false,
            mag: U256::from_str_radix(body, radix as u64).unwrap_or(U256::MAX),
        };
    }
    let (neg, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return BigVal::ZERO;
    }
    let mag = parse_dec_saturating(digits);
    BigVal {
        neg: neg && !mag.is_zero(),
        mag,
    }
}

fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    if head.eq_ignore_ascii_case(prefix) {
        s.get(prefix.len()..)
    } else {
        None
    }
}

/// Pre-validated decimal digits → U256, saturating on overflow.
fn parse_dec_saturating(digits: &str) -> U256 {
    U256::from_str_radix(digits, 10).unwrap_or(U256::MAX)
}

/// Strict wire amount: unsigned decimal digits only. `None` on anything else.
fn parse_dec_u256(s: &str) -> Option<U256> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    U256::from_str_radix(s, 10).ok()
}

/// A detected amount string (our own encoding: optional `-`, then digits).
fn parse_signed_dec(s: &str) -> BigVal {
    str_to_big(s)
}

/// `word` → U256; a short word is right-padded (`padEnd(64, '0')`), a
/// non-hex word means the calldata is junk → detection declines.
fn big_from_word(w: &str) -> Option<U256> {
    if w.is_empty() {
        return Some(U256::ZERO);
    }
    if !w.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut padded = w.to_owned();
    while padded.len() < 64 {
        padded.push('0');
    }
    U256::from_str_radix(&padded, 16).ok()
}

/// `lc`: lowercase a string value, `''` for anything else.
fn lc(v: Option<&Value>) -> String {
    v.and_then(Value::as_str)
        .map(str::to_lowercase)
        .unwrap_or_default()
}

fn nonempty(s: String) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// JS truthiness for the `msg.details` guard.
fn js_truthy(v: Option<&Value>) -> bool {
    match v {
        None | Some(Value::Null) => false,
        Some(Value::Bool(b)) => *b,
        Some(Value::Number(n)) => n.as_f64().is_some_and(|f| f != 0.0 && !f.is_nan()),
        Some(Value::String(s)) => !s.is_empty(),
        Some(Value::Array(_)) | Some(Value::Object(_)) => true,
    }
}

/// `allowed === true || allowed === 'true' || allowed === 1 || allowed === '1'`
fn is_js_one_or_true(v: Option<&Value>) -> bool {
    match v {
        Some(Value::Bool(true)) => true,
        Some(Value::String(s)) => s == "true" || s == "1",
        Some(Value::Number(n)) => n.as_f64() == Some(1.0),
        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must match the current attempt.
fn requests(model: &Model, operations: Vec<GuardOperation>) -> Command<GuardEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<GuardEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for GuardEffect {
    type Op = GuardOperation;
    fn into_shell(self) -> Option<crux_core::Request<GuardOperation>> {
        match self {
            GuardEffect::Render(_) => None,
            GuardEffect::Shell(request) => Some(request),
        }
    }
}
