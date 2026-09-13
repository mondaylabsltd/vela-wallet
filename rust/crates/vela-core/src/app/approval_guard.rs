//! Machine — the approval guard (spec `017`, inventory `### approval_guard (P1)`).
//!
//! The "unlimited can never leave the wallet" core: detection of every
//! approval-granting shape straight off the raw calldata / typed data (a token
//! drainer's #1 tool is an unbounded `approve` / `permit` / `setApprovalForAll`,
//! and a descriptor lookup is exactly what fails on novel/hostile contracts),
//! the finite re-encode, the descriptor-independent submit guard, plus the
//! spending-cap editor's choice derivation (single AND per batch leg — one
//! `init_editor` / `derive_editor` pair serves both) and the EIP-5792 per-leg
//! gating.
//!
//! ```text
//! ApprovalDetected ─► detect (8 shapes, pure) ─► editor init
//!        │                                        │
//!        ├─ ReadTokenMetadata / ReadErc20Allowance / ReadErc20Balance (RPC)
//!        ▼                                        ▼
//!   typed-path permit ─► sign-verbatim surface   calldata ─► cap editor
//!   (never rewritten — consent, not capping)     choice=None ⇒ confirm gated
//! ```
//!
//! Faithful port of the TypeScript sources — behavior aligned line by line,
//! the cap constants (2^200 for uint256 fields, 2^152 for uint160 fields) and
//! every classification kept verbatim:
//!
//! - `src/services/approval-guard.ts` (whole file) — detectApproval's 8
//!   shapes, rewriteApprovalParams + assertOnlyWordChanged, enforceNoUnlimited,
//!   parse/format token amounts
//! - `src/components/signing/EditableApproveCard.tsx:85-107` — the
//!   mode → choice derivation (unbounded starts with NO choice; a custom
//!   amount ≥ cap derives `None` + an error, never a choice), `:217` (a
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

/// uint256 amount fields (ERC-20 approve / increaseAllowance / ERC-2612
/// value): 2^200. Far above any legitimate amount (total_supply × 10^decimals
/// ≈ 2^128) and far below the "unlimited" sentinels (2^256-1, 2^255), so it
/// cleanly separates "a big finite number the user chose" from "unlimited".
pub const UNLIMITED_CAP_256: U256 = U256::from_limbs([0, 0, 0, 1 << 8]);
/// Permit2 uint160 amount fields (sentinel is 2^160-1): 2^152.
pub const UNLIMITED_CAP_160: U256 = U256::from_limbs([0, 0, 1 << 24, 0]);

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
    /// The one-tap finite Balance cap is offered (issue #86).
    pub has_balance_cap: bool,
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
    pub increase_total: Option<GuardIncreaseTotalView>,
    /// Unverified decimals must be explicitly flagged
    /// (`EditableApproveCard.tsx:200-202`; `PermitSignView.tsx:103-105`).
    pub decimals_unverified: bool,
    pub expired: bool,
    pub batch: Option<GuardBatchView>,
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
            let rewritten_params_json = rewritten_batch_params(model, batch, &batch_view);
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
                increase_total: None,
                decimals_unverified: false,
                expired: false,
                batch: Some(batch_view),
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
                increase_total: None,
                decimals_unverified: false,
                expired: false,
                batch: None,
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

        let rewritten_params_json = match (&choice, &model.params) {
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
            decimals_unverified,
            meta,
            batch: None,
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
            let detected = detect_calldata_approval(
                call.get("to").and_then(Value::as_str),
                call.get("data").and_then(Value::as_str),
            );
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
    } else {
        // An unbounded request forces a deliberate choice.
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

fn set_custom_text(editor: &mut Editor, text: String) -> Command<GuardEffect, Event> {
    if let Editor::Amount { custom_text, .. } = editor {
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
        GuardEditorMode::Balance if has_balance_cap => {
            *current = GuardEditorMode::Balance;
            if let Some(balance) = balance {
                *custom_text = format_token_amount(balance, meta.decimals, 6, "", ".", false);
            }
        }
        GuardEditorMode::Custom => *current = GuardEditorMode::Custom,
        GuardEditorMode::Revoke => *current = GuardEditorMode::Revoke,
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
            has_balance_cap: false,
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

            let (choice, error, display) = match mode {
                GuardEditorMode::Revoke => (Some(GuardChoice::Revoke), None, Some(U256::ZERO)),
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
                has_balance_cap,
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
/// optional. Port of `BatchCallsView.tsx:40-45`.
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
            Some(GuardChoice::Amount { .. }) | Some(GuardChoice::Revoke)
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
fn rewritten_batch_params(
    model: &Model,
    batch: &BatchState,
    view: &GuardBatchView,
) -> Option<String> {
    let params = model.params.as_ref()?;
    let mut changed = false;
    let new_calls: Vec<Value> = batch
        .legs
        .iter()
        .zip(view.legs.iter())
        .map(|(leg, projected)| {
            if let (Some(detected), Some(choice)) = (&leg.detected, &projected.choice) {
                if detected.editable {
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
        return detect_calldata_approval(
            tx.get("to").and_then(Value::as_str),
            tx.get("data").and_then(Value::as_str),
        );
    }

    if method.contains("signTypedData") {
        // `params[1] ?? params[0]`
        let raw = match arr.get(1) {
            None | Some(Value::Null) => arr.first()?,
            Some(value) => value,
        };
        let parsed;
        let td = match raw {
            Value::String(s) => {
                parsed = serde_json::from_str::<Value>(s).ok()?;
                &parsed
            }
            other => other,
        };
        return detect_typed_data_approval(td);
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
            })
        }
        _ => None,
    }
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

    if method.contains("signTypedData") {
        // `typeof params[1] === 'string' || (params[1] && typeof === 'object')`
        let idx = match arr.get(1) {
            Some(Value::String(_)) | Some(Value::Object(_)) | Some(Value::Array(_)) => 1,
            _ => 0,
        };
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
    let word_start = 8 + (*word_index as usize) * 64;
    let word_end = word_start + 64;
    if hex.len() < word_end {
        return Err(GuardRewriteError::CalldataTooShort);
    }
    let selector = hex.get(..8).ok_or(GuardRewriteError::CalldataTooShort)?;

    let new_word = if detected.kind == GuardApprovalKind::SetApprovalForAll {
        // boolean: grant → true, revoke → false. No "amount" to cap.
        match choice {
            GuardChoice::Amount { .. } => return Err(GuardRewriteError::AmountForBooleanShape),
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
        if matches!(choice, GuardChoice::Amount { .. }) {
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

/// Errors if the FINAL request would grant an unbounded allowance. The UI
/// caps approvals up-front; this catches anything that bypassed it (incl.
/// shapes no descriptor decodes).
pub fn enforce_no_unlimited(
    method: &str,
    params: Option<&Value>,
) -> Result<(), GuardUnlimitedApproval> {
    let Some(detected) = detect_approval(method, params) else {
        return Ok(());
    };
    // Off-chain permit SIGNATURES (typed data) are redeemed by the dApp with
    // its OWN struct — the wallet can't cap what it doesn't submit, so a
    // forced cap only desyncs the signature and reverts the dApp's tx. These
    // are gated by an explicit, deliberate UI risk-consent (slide-to-confirm),
    // not by this amount guard, which only governs txs the WALLET submits.
    if matches!(detected.locus, GuardLocus::TypedPath { .. }) {
        return Ok(());
    }
    // Boolean grants (setApprovalForAll true) are handled by explicit UI
    // consent — there is no finite amount to enforce.
    if detected.is_boolean_grant {
        return Ok(());
    }
    // Reducing the allowance (decreaseAllowance / approve-to-0) never grants.
    if detected.is_reducing {
        return Ok(());
    }
    if let (Some(amount_raw), Some(bits)) = (&detected.amount_raw, detected.amount_bits) {
        let amount = parse_signed_dec(amount_raw);
        if !amount.neg && is_unbounded_amount(amount.mag, bits_from_wire(Some(bits))) {
            return Err(GuardUnlimitedApproval {
                kind: detected.kind,
                amount_raw: amount_raw.clone(),
            });
        }
    }
    Ok(())
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
