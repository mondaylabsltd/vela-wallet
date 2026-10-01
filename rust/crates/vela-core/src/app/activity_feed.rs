//! Machine — the home Activity feed (spec `016-crux-wallet-state`,
//! activity_feed P3).
//!
//! ```text
//! AccountSwitched/FocusTick/LiveTick ─► ReadTxStore + ScanIncomingTransfers
//!        StoreLoaded ─► dedupe ► batch-fold ► tombstone-filter ► commit ► alias ops
//!        SyncCompleted{n>0} ─► re-read (celebrate only after the first pass)
//!        ReconcileCompleted{n>0} (tx_tracker's verdicts landed) ─► re-read
//!        DeleteRequested ─► tombstone + optimistic remove ─► DeleteTxRecord
//! ```
//!
//! The LOCAL tx store is the single source of truth; this machine only reads
//! it, folds it and decides when to read it again. Ported from
//! `src/services/activity.ts` (feed adapter, batch folding, stablecoin
//! valuation) and `src/screens/wallet/useHomeController.ts` (load pipeline,
//! celebration, tombstones, alias memoisation, chain filter, date grouping).
//! The pending→confirmed convergence policy itself belongs to
//! [`super::tx_tracker`] — this machine merely consumes its outcome as
//! [`Event::ReconcileCompleted`].
//!
//! # The contract change (inventory-approved)
//!
//! Today's `ActivityItem` carries pre-formatted strings (`"+1 USDT"`,
//! `"$1.00"`), which forces a full adapter re-run on locale change and makes
//! the celebration toast reverse-parse its own amount string
//! (`useHomeController.ts:225-227`). Here every item carries **structured**
//! values — `value` (decimal string) + `decimals` + `symbol` + `usd_value`
//! (f64) — and the shell formats. Both hacks disappear: locale changes are a
//! pure re-render, and the toast carries `value`/`symbol` directly.
//!
//! # Shell contract
//!
//! - `ReadTxStore` answers `StoreLoaded` with every stored record mapped to
//!   [`FeedTxRecord`]; a storage failure answers an EMPTY list, exactly as
//!   `loadTransactions().catch(() => [])` does today (ported verbatim — yes,
//!   that blanks the feed; the store is the source of truth even about
//!   emptiness).
//! - `day_start_ms` is the record's LOCAL-midnight epoch key, computed by the
//!   shell (it owns the device timezone) — the port of `dayStartMs`.
//! - `ScanIncomingTransfers` runs the whole `syncReceivedTransfers` discovery
//!   (admission routed through token_trust) and answers the count of
//!   genuinely-new persisted receipts; any failure answers `0` (the TS
//!   `catch { return 0 }`).
//! - `ResolveRecipientIdentity` must check the user's OWN accounts first and
//!   answer the local name without touching the network
//!   (`useHomeController.ts:432-434` — the shell owns the accounts list); the
//!   core's half of invariant ⑦ is that a stored `to_name` never even asks,
//!   and an attempted address is never asked twice.
//! - `FocusTick`/`LiveTick` cadence (focus + 30s auto-refresh, 10s while the
//!   Activity tab is visible) stays in the shell: which tab is visible is
//!   render-domain state the core never sees. The core owns the toast timer.
//! - After every `PersistRecord` / `UpdateRecord` a shell dispatches
//!   `ReconcileCompleted { resolved_count: 1 }`, so a new or patched row
//!   shows at once instead of on the next tick (spec 082 RG3).
//!
//! # Rows the core decides (spec 082, L-D3 / L-D7)
//!
//! Every [`FeedItem`] says what it is ([`FeedItem::kind`]), where its record
//! stands ([`FeedItem::status`]) and, for a transaction a dApp asked for,
//! which site asked ([`FeedItem::site`]); the shells delete their own guesses.
//! Message signatures and connects never become rows. The empty lines of
//! History and of the home Activity are corpus keys on [`FeedView`], chosen by
//! the chain filter.
//!
//! # Fidelity notes
//!
//! - The TS pipeline is one sequential async function (read → reconcile →
//!   scan); here the read and the scan are issued together and each answer
//!   drives its own step. The visible invariants — cache paints first, sync
//!   re-reads only when `new_count > 0` — are unchanged.
//! - `DeleteFailed` still drops the tombstone (the TS `.finally()`), so a
//!   failed delete lets the next reload resurrect the row — ported verbatim;
//!   the record genuinely still exists in storage.
//! - `alias_map`/`alias_attempted` survive an account switch (both are
//!   session-lived refs in TS) — an address→name fact is account-agnostic.
//! - The web-only `velaSimulateReceipt` dev hook is not ported.
//! - A dApp's transaction (`dapp_tx`) is a row too (083 H2, 079 D3); the TS
//!   feed showed only sends and receives, so a stuck dApp swap was visible
//!   nowhere. Signatures and connections stay out: they move nothing.

use std::collections::{BTreeMap, BTreeSet};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::clear_signing::ClearTerm;
use super::token_trust::TrustSimJudgment;

/// Toast lifetime — `setTimeout(() => setReceipt(null), 2800)`.
pub const TOAST_MS: u32 = 2_800;

/// History with nothing to show on every network (spec 082 RG5, L-D7).
pub const HISTORY_EMPTY_ALL: &str = "history.emptyTitle";
/// History with nothing to show on the chosen network.
pub const HISTORY_EMPTY_FILTERED: &str = "history.emptyFilter";
/// The home Activity with nothing to show on every network.
pub const HOME_EMPTY_ALL: &str = "home.emptyNoActivity";
/// The home Activity with nothing to show on the chosen network.
pub const HOME_EMPTY_FILTERED: &str = "home.emptyNoActivityNetwork";

/// The most decimals a dApp row's stored figure is scaled by (083 H2
/// review). Every client stores 18; a `u128` has 39 digits, so anything past
/// this could only print a string of zeros — and an unbounded one sizes an
/// allocation from a number read off the disk.
pub const MAX_DAPP_DECIMALS: u32 = 36;

/// Symbols treated as ≈ $1 so stablecoin transfers are never shown as $0.00
/// (`activity.ts:150-152`, verbatim).
pub const STABLE_SYMBOLS: [&str; 15] = [
    "USDT", "USDT0", "USDC", "USDC.E", "DAI", "BUSD", "TUSD", "FDUSD", "USDE", "PYUSD", "USDP",
    "GUSD", "LUSD", "FRAX", "USDD",
];

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// The `LocalTransaction.type` union (`storage.ts:390-391`). A record with no
/// type is a legacy row and defaults to `send` — which is also the
/// [`Default`], so a [`FeedItem`] decoded from before spec 082 reads as one.
///
/// A [`FeedItem`] only ever carries `Send`, `Receive` or `DappTx`: message
/// signatures and connects never become rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeedTxKind {
    #[default]
    Send,
    Receive,
    DappTx,
    SignMessage,
    SignTypedData,
    Connect,
}

/// Storage lifecycle vocabulary (`status: 'pending' | 'confirmed' | 'failed'`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeedTxStatus {
    Pending,
    Confirmed,
    Failed,
}

/// One stored transaction, as the shell maps `LocalTransaction` in — only the
/// fields the feed folds. Amounts are decimal strings, timestamps are the
/// stored epoch SECONDS (f64 — no u64 crosses the wire), and `usd` keeps the
/// legacy pre-formatted string exactly as persisted (parsed here, once, the
/// way `txUsdValue` parses it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedTxRecord {
    pub id: String,
    /// Empty for receives and off-chain signatures.
    pub user_op_hash: String,
    /// Empty string when no on-chain hash exists (`storage.ts:371`).
    pub tx_hash: String,
    pub from: String,
    pub to: String,
    /// Recipient name captured at send time (`toName`).
    pub to_name: Option<String>,
    /// Token amount, decimal string (`value`).
    pub value: String,
    pub symbol: String,
    pub decimals: u32,
    /// Ordered token-logo URL candidates captured at write time.
    pub logo_urls: Option<Vec<String>>,
    pub chain_id: u32,
    /// Stored epoch seconds.
    pub timestamp: f64,
    /// LOCAL-midnight epoch ms for `timestamp` — computed by the shell, which
    /// owns the device timezone (the `dayStartMs` port; invariant ⑥).
    pub day_start_ms: f64,
    pub status: FeedTxStatus,
    /// `None` = legacy untyped record ⇒ treated as `send` (`t.type ?? 'send'`).
    pub kind: Option<FeedTxKind>,
    /// Legacy pre-formatted USD (e.g. `"$1.00"`), as stored.
    pub usd: Option<String>,
    /// `dapp_tx` only (083 H2): the origin the request arrived from, as the
    /// signing path stored it (`dappUrl`, `SignRecord::dapp_url`). Never
    /// `dappOrigin`: that holds the dApp's self-declared name when it gave
    /// one, and a site named from it would be whatever the dApp said it was.
    /// Absent on every other kind and from a shell that does not map it yet.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dapp_url: Option<String>,
    /// `dapp_tx` only (083 H2): the intent recorded at approve time
    /// (`intent`, e.g. "Swap").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intent: Option<String>,
    /// `dapp_tx` only (083 F1): what the wallet's own simulation said the
    /// operation moves, as the signing sheet drew it when the person approved
    /// (`SignRecord::balance_changes`, stored by the shell). The one account
    /// of a dApp call's money that its page did not write. Absent on every
    /// other kind, on older rows and from a shell that does not map it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_changes: Option<Vec<TrustSimJudgment>>,
    /// `dapp_tx` only (083 F3): whether the transaction carried calldata, as
    /// the shell read it off the stored request — `true` makes `to` the
    /// contract it called (a router, a token), not somebody who received
    /// anything. `None` when the shell cannot say.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calldata: Option<bool>,
    /// `dapp_tx` only (spec 082 RJ16): the call's `data` hex, which the shells
    /// map from the stored request (`signedRequest`), so a token `transfer`
    /// can name who got the tokens. `None` for every other kind, for a plain
    /// send, and for a shell that does not map it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub call_data: Option<String>,
}

impl FeedTxRecord {
    fn kind(&self) -> FeedTxKind {
        self.kind.unwrap_or_default()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeedDirection {
    In,
    Out,
}

/// Who a row's `counterparty` is (spec 082 RJ16, G52).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeedCounterpartyRole {
    /// The party the money went to (or, for an incoming row, came from —
    /// `direction` picks the words): the send's recipient, or the recipient
    /// a dApp's token `transfer` names.
    #[default]
    Recipient,
    /// The contract a dApp's call went to, which is not who got anything
    /// (`componentsUi.signing.interactingLabel`).
    Contract,
}

/// `split` = one token → N recipients; `multi_select` = N tokens → 1 recipient.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FeedBatchKind {
    Split,
    MultiSelect,
}

/// One line in a batch send breakdown (`ActivityBatchTransfer`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedBatchTransfer {
    pub to: String,
    pub to_name: Option<String>,
    pub value: String,
    pub symbol: String,
    pub decimals: u32,
    pub usd_value: f64,
    pub logo_urls: Option<Vec<String>>,
}

/// A batch send summarized from its per-line records (`buildBatchView`,
/// `activity.ts:210-236`, ported verbatim including the kind ternary).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedBatch {
    pub kind: FeedBatchKind,
    pub count: u32,
    pub total_usd: f64,
    pub transfers: Vec<FeedBatchTransfer>,
    /// Stored record ids of the siblings — for live status reconciliation.
    pub ids: Vec<String>,
    pub from: String,
    pub chain_id: u32,
    pub timestamp: f64,
    pub status: FeedTxStatus,
    pub tx_hash: String,
    pub user_op_hash: String,
    /// split only: the single token symbol + its logo.
    pub symbol: Option<String>,
    pub logo_urls: Option<Vec<String>>,
    /// multi_select only: the single recipient.
    pub to: Option<String>,
    pub to_name: Option<String>,
}

/// One feed row's payload — the structured replacement for `ActivityItem`.
/// The shell formats: sign from `direction`, amount from `value`/`decimals`/
/// `symbol` (compact at a glance, exact in detail), fiat from `usd_value` ×
/// the display-currency rate, counterparty label from `alias` falling back to
/// a shortened `counterparty`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedItem {
    /// Row identity (FlatList key): the record id, or the shared
    /// `user_op_hash` for a folded batch row.
    pub id: String,
    pub direction: FeedDirection,
    /// Counterparty address — sender for `in`, recipient for `out`; `None`
    /// for a split batch row (no single recipient).
    pub counterparty: Option<String>,
    /// In the MODEL: the stored local name (`toName`). In the VIEW: the
    /// resolved overlay — `alias_map[addr] ?? stored`, the
    /// `HomeScreen.tsx:244-250` precedence.
    pub alias: Option<String>,
    /// Token amount, decimal string. `None` for a multi_select batch row
    /// (mixed tokens can't sum — the shell shows the asset count from
    /// `batch.count` instead).
    pub value: Option<String>,
    /// Empty for multi_select batch rows (`token: b.symbol ?? ''`).
    pub symbol: String,
    pub decimals: Option<u32>,
    /// Numeric USD (0 when unknown) — the `txUsdValue` port, stablecoin
    /// face-value fallback included (invariant ⑧).
    pub usd_value: f64,
    pub chain_id: u32,
    /// Epoch seconds (drives `relativeTime`/`dayGroupLabel` in the shell).
    pub timestamp: f64,
    /// The record's local-midnight grouping key (shell-computed).
    pub day_start_ms: f64,
    pub tx_hash: Option<String>,
    pub batch: Option<FeedBatch>,
    /// What the row is (spec 082 RG1): `Send` (a folded batch too),
    /// `Receive` or `DappTx`. The shell draws from this and never guesses.
    #[serde(default)]
    pub kind: FeedTxKind,
    /// The record's lifecycle; a folded batch carries its first line's. The
    /// tracker is the only thing that moves a record off `Pending`, so a row
    /// says "confirmed" or "failed" only when the stored record does.
    #[serde(default = "status_unknown")]
    pub status: FeedTxStatus,
    /// `DappTx` only: the site that asked, read from the origin the request
    /// arrived from (`dapp_url`, never the dApp's own name) — the same value
    /// as `dapp.site`.
    #[serde(default)]
    pub site: Option<String>,
    /// Whether `counterparty` got the money or is the contract a call went
    /// to (spec 082 RJ16). Always `Recipient` except on a `DappTx` row.
    #[serde(default)]
    pub counterparty_role: FeedCounterpartyRole,
    /// A dApp transaction's site, intent and what it moved (083 H2); `None`
    /// on every other row. Its `site` and `contract_call` are the row's own
    /// [`FeedItem::site`] and [`FeedItem::counterparty_role`], read once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dapp: Option<FeedDapp>,
}

/// The status of a [`FeedItem`] decoded from before spec 082, which carried
/// none. `Pending` because it claims nothing: only the tracker closes a
/// record, and a default must never say money landed or failed.
fn status_unknown() -> FeedTxStatus {
    FeedTxStatus::Pending
}

/// What a dApp transaction row says beyond its money (083 H2, 079 D3): which
/// site asked and what the call did.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedDapp {
    /// The site as a person reads it: the host of the origin the request
    /// arrived from (`app.uniswap.org`, `127.0.0.1`). `None` when the record
    /// holds no origin that parses as one — never the dApp's own name.
    pub site: Option<String>,
    /// The intent recorded at approve time, as the descriptor wrote it
    /// ("Swap"; "Send" for a plain native transfer). `None` is a call nobody
    /// decoded, which the shell reads as "Contract interaction".
    pub intent: Option<String>,
    /// `intent` as a word the shell can translate
    /// (`componentsUi.signing.<leaf>`).
    pub intent_term: Option<ClearTerm>,
    /// What the operation moved, as the wallet's own simulation measured it
    /// when the person approved — the signing sheet's "Balance changes"
    /// lines, in its order (083 F1). Empty for a record that kept none (an
    /// older row, a shell that does not record them, a simulation that could
    /// not run) and for an operation that FAILED, which moved nothing: the
    /// row then draws as it did before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub changes: Vec<FeedDappChange>,
    /// A swap-shaped operation's one inflow — exactly one line out, which is
    /// the row's figure, and exactly one in, both with a figure, both of a
    /// coin the wallet trusts. Drawn beside the figure as what the
    /// simulation EXPECTED: the chain may deliver another amount (slippage).
    /// `None` otherwise, and always for a failed operation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub received: Option<FeedDappChange>,
    /// The row's figure is the simulation's expectation, not an amount the
    /// wallet can vouch for (083 F1 review): an outflow the sheet measured,
    /// which an exact-output swap may overspend or underspend on chain. The
    /// shell marks it "≈". `false` for the call's own value and for a
    /// native outflow equal to it — what the wallet itself submitted.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub estimated: bool,
    /// The transaction carried calldata, so the row's counterparty is the
    /// contract it called — never labelled a recipient (083 F3). `false` for
    /// a plain transfer of the chain's coin and for a record that cannot say.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub contract_call: bool,
}

/// One line of what a dApp transaction moved (083 F1), from the signing
/// sheet's own simulation — never from anything the page supplied.
///
/// The sheet's asymmetry holds here too, because it is the same judgment
/// (`token_trust`, invariant ⑥): an outflow carries its figure whenever the
/// token's metadata resolved; an inflow of a token the wallet does not trust
/// is `verified: false` and carries NO figure — a site can emit any
/// `Transfer` it likes from a contract it controls.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedDappChange {
    /// `out` leaves the account; `in` arrives. An inflow is what the
    /// simulation expected, which the chain may not deliver to the unit.
    pub direction: FeedDirection,
    /// `false` for a token the sheet showed as unverified: the shell names it
    /// "Unverified token" and draws a direction, never a number.
    pub verified: bool,
    /// The coin's symbol — the chain's own for a native line. Empty when
    /// unverified, or for a native coin the record's chain does not name.
    pub symbol: String,
    /// The amount, unsigned, as a human decimal (`"0.1"`). `None` when there
    /// is no figure to show: unverified, or a stored delta that will not read.
    pub value: Option<String>,
    pub decimals: Option<u32>,
    /// The wallet can vouch for this figure to the unit: a native outflow
    /// equal to the value the wallet itself submitted. Every other line is
    /// what the simulation expected — an inflow may arrive short (slippage)
    /// and an exact-output swap's outflow may differ too, and the wallet
    /// cannot tell which kind of swap it signed — so the shell marks it "≈".
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub exact: bool,
}

/// A date header or an item — the grouped feed, in render order
/// (invariant ⑥: headers can never inter-sort with items because the core
/// emits them already interleaved).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
// Boxing `Item` to even the variants out would buy an allocation per row and
// change nothing the shell sees — this is a wire type whose JSON shape is
// pinned by the generated TypeScript.
#[allow(clippy::large_enum_variant)]
pub enum FeedRow {
    Header {
        /// `day-<dayStartMs>` — the stable list key.
        id: String,
        day_start_ms: f64,
        /// A representative timestamp inside the day — the shell derives
        /// "Today" / "Yesterday" / date from it plus its own `now`.
        timestamp: f64,
    },
    Item {
        item: FeedItem,
    },
}

/// The receipt toast, structured — the shell formats `value` + `symbol`
/// (this retires the strip-the-symbol string hack).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedToast {
    pub item_id: String,
    pub value: String,
    pub symbol: String,
    /// `celebrated_at + 2800` — epoch ms, from the celebrating result's clock.
    pub deadline_ms: f64,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeedOperation"))]
pub enum FeedOperation {
    /// Read the whole local tx store (`loadTransactions`); the shell maps rows
    /// to [`FeedTxRecord`] and answers a load failure with an empty list.
    ///
    /// `read_id` must be echoed back on [`FeedShellResult::StoreLoaded`]. It is
    /// what binds a celebration to the read that earned it: a tick issues this
    /// read and the incoming-transfer scan together, so without the echo a
    /// scan that answered first would let the older read consume the flag and
    /// a genuine receipt would land with no toast, glow or haptic. The
    /// TypeScript original got that binding for free from a closure over one
    /// `loadData` call.
    ReadTxStore { address: String, read_id: u32 },
    /// Run receipt discovery + persistence (`syncReceivedTransfers`), token
    /// admission via token_trust; answers the count of genuinely-new records
    /// (0 on any failure).
    ScanIncomingTransfers { address: String },
    /// Delete one stored record (`deleteTransaction`).
    DeleteTxRecord { id: String },
    /// Resolve a counterparty name. The shell checks the user's OWN accounts
    /// first (local name, no network), then ENS/.bnb/Vela/etc.
    ResolveRecipientIdentity { addr: String },
    /// Toast countdown. `generation` is echoed back so a superseded timer
    /// (a newer celebration re-armed the toast) can never clear it early.
    Timer { ms: u32, generation: u32 },
    /// `hapticSuccess()` — money-in buzz.
    Haptic,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeedShellResult"))]
pub enum FeedShellResult {
    /// `now_ms` rides on the result (the 011 pattern) — it stamps the toast
    /// deadline when this read was celebration-flagged.
    StoreLoaded {
        records: Vec<FeedTxRecord>,
        now_ms: f64,
        /// Echoed from the [`FeedOperation::ReadTxStore`] that produced it.
        read_id: u32,
    },
    /// The scan finished; `new_count` new receipts were persisted.
    SyncCompleted {
        new_count: u32,
    },
    DeleteCommitted {
        id: String,
    },
    DeleteFailed {
        id: String,
    },
    /// `name: None` ⇒ nothing resolved (never retried this session).
    AliasResolved {
        addr: String,
        name: Option<String>,
    },
    ToastExpired {
        generation: u32,
    },
    HapticPlayed,
}

impl Operation for FeedOperation {
    type Output = FeedShellResult;
}

#[effect]
pub enum FeedEffect {
    Render(RenderOperation),
    Shell(FeedOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeedEvent"))]
pub enum Event {
    /// The active account changed (or first mounted). Resets the first-pass
    /// flag and the celebration, bumps `attempt` so every in-flight answer
    /// for the previous account is dropped — the core twin of the
    /// `addressRef.current !== address` guard. The previous feed keeps
    /// painting until the new account's read commits (ported verbatim: the
    /// screen never flashes empty on switch).
    AccountSwitched { address: String },
    /// Home gained focus / the 30s auto-refresh fired.
    FocusTick,
    /// The 10s near-real-time poll while the Activity tab is visible.
    LiveTick,
    /// tx_tracker converged pending submissions (`reconcilePendingTransactions`
    /// returned); a positive count re-reads the store — never celebrates
    /// (`useHomeController.ts:284-295`).
    ReconcileCompleted { resolved_count: u32 },
    /// Balance privacy toggled. While hidden the toast is suppressed
    /// (invariant ④, `HomeScreen.tsx:176-180`) — but the row glow and the
    /// haptic still happen, exactly as today.
    PrivacyChanged { hidden: bool },
    /// The network chip filter (`selectedChainId`); `None` = all chains.
    ChainFilterChanged { chain_id: Option<u32> },
    /// Optimistic per-row delete: tombstone + instant removal + storage write.
    DeleteRequested { id: String },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: FeedShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct ToastState {
    value: String,
    symbol: String,
    /// `now_ms + TOAST_MS` at celebration time — informational; expiry is
    /// driven by the echoed `generation`, not by clock math in the core.
    deadline_ms: f64,
    generation: u32,
}

#[derive(Clone, Debug)]
struct Celebration {
    /// The glowing row (`newItemId`) — persists after the toast expires,
    /// cleared only by an account switch or the next celebration.
    item_id: String,
    toast: Option<ToastState>,
}

/// Mint the next read id and the operation carrying it.
fn read_store(model: &mut Model) -> (u32, FeedOperation) {
    model.next_read_id = model.next_read_id.wrapping_add(1);
    (
        model.next_read_id,
        FeedOperation::ReadTxStore {
            address: model.address.clone(),
            read_id: model.next_read_id,
        },
    )
}

#[derive(Default)]
pub struct Model {
    address: String,
    /// The folded feed (deduped, batch-folded, tombstone-filtered, sorted
    /// newest-first) — rebuilt only by the reload path (invariant ⑤'s
    /// "single setter").
    items: Vec<FeedItem>,
    /// Raw account-filtered records (`loadActivityTransactions` /
    /// `txByIdRef`) — powers the detail sheet. Deliberately NOT
    /// tombstone-filtered, mirroring `txByIdRef`.
    records: Vec<FeedTxRecord>,
    /// First sync pass done (`initializedRef`) — the backlog gate
    /// (invariant ③).
    initialized: bool,
    /// The next `StoreLoaded` commit is a post-sync re-read that may
    /// celebrate its newest incoming item.
    celebrate_read_id: Option<u32>,
    /// Mints [`FeedOperation::ReadTxStore`] ids. Wraps harmlessly: a
    /// collision needs 2^32 reads outstanding at once.
    next_read_id: u32,
    /// Ids mid-delete: filtered out of every reload commit until the storage
    /// write settles (invariant ⑤).
    tombstones: BTreeSet<String>,
    celebration: Option<Celebration>,
    /// Monotonic toast generation — the anti-stale token echoed through
    /// [`FeedOperation::Timer`].
    generation: u32,
    /// Lowercased address → resolved name. Session-lived, survives account
    /// switches (as the TS refs do).
    alias_map: BTreeMap<String, String>,
    /// Addresses already asked — never asked again this session
    /// (invariant ⑦).
    alias_attempted: BTreeSet<String>,
    chain_filter: Option<u32>,
    privacy_hidden: bool,
    /// Bumped ONLY on account switch — any in-flight answer for a previous
    /// account is stale and dropped.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeedView {
    /// Date headers + items, chain-filtered, in render order.
    pub rows: Vec<FeedRow>,
    /// Raw account-scoped records for the detail sheet
    /// (`loadActivityTransactions`).
    pub transactions: Vec<FeedTxRecord>,
    /// The glowing "just landed" row (`newItemId`).
    pub new_item_id: Option<String>,
    /// `None` while balance privacy is on — invariant ④ enforced here, not
    /// in the shell.
    pub toast: Option<FeedToast>,
    /// The corpus key of History's empty line (spec 082 RG5):
    /// [`HISTORY_EMPTY_ALL`] with no chain filter, [`HISTORY_EMPTY_FILTERED`]
    /// with one. Whether the list is loading or empty stays the shell's.
    #[serde(default)]
    pub history_empty_key: String,
    /// The corpus key of the home Activity's empty line: [`HOME_EMPTY_ALL`] /
    /// [`HOME_EMPTY_FILTERED`], chosen the same way.
    #[serde(default)]
    pub home_empty_key: String,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct ActivityFeed;

impl App for ActivityFeed {
    type Event = Event;
    type Model = Model;
    type ViewModel = FeedView;
    type Effect = FeedEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<FeedEffect, Event> {
        match event {
            Event::AccountSwitched { address } => {
                model.attempt += 1;
                model.address = address;
                model.initialized = false;
                model.celebrate_read_id = None;
                // setNewItemId(null); setReceipt(null) — the account-change
                // reset (`useHomeController.ts:399-402`).
                model.celebration = None;
                load_pipeline(model)
            }
            Event::FocusTick | Event::LiveTick => {
                // `if (!address) return` (`useHomeController.ts:262`).
                if model.address.is_empty() {
                    return Command::done();
                }
                load_pipeline(model)
            }
            Event::ReconcileCompleted { resolved_count } => {
                if resolved_count == 0 || model.address.is_empty() {
                    return Command::done();
                }
                // Converged records changed in place — re-read, never
                // celebrate (`useHomeController.ts:287-294`).
                let (_, op) = read_store(model);
                shell_request(model.attempt, op)
            }
            Event::PrivacyChanged { hidden } => {
                model.privacy_hidden = hidden;
                render()
            }
            Event::ChainFilterChanged { chain_id } => {
                model.chain_filter = chain_id;
                render()
            }
            Event::DeleteRequested { id } => {
                // Optimistic remove + tombstone until the write settles, so a
                // concurrent reload can't repaint the just-deleted row
                // (`useHomeController.ts:602-610`).
                model.tombstones.insert(id.clone());
                model.items.retain(|item| item.id != id);
                Command::all([
                    shell_request(model.attempt, FeedOperation::DeleteTxRecord { id }),
                    render(),
                ])
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A previous account's answer — the addressRef guard.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> FeedView {
        // Chain filter first, then date headers over the filtered list —
        // exactly `filteredActivity` → `activityFeed`
        // (`useHomeController.ts:543-563`).
        let mut rows = Vec::new();
        let mut last_day: Option<f64> = None;
        for item in &model.items {
            if let Some(chain) = model.chain_filter {
                if item.chain_id != chain {
                    continue;
                }
            }
            if last_day != Some(item.day_start_ms) {
                rows.push(FeedRow::Header {
                    id: format!("day-{}", item.day_start_ms),
                    day_start_ms: item.day_start_ms,
                    timestamp: item.timestamp,
                });
                last_day = Some(item.day_start_ms);
            }
            let mut out = item.clone();
            if let Some(addr) = &out.counterparty {
                // Resolved name wins over the stored one — the
                // `aliasMap.get(...) ?? item.alias` precedence.
                if let Some(name) = model.alias_map.get(&addr.to_lowercase()) {
                    out.alias = Some(name.clone());
                }
            }
            rows.push(FeedRow::Item { item: out });
        }

        // Toast suppressed while privacy is on (invariant ④) — the state
        // still exists (haptic fired, glow shows), only the number-bearing
        // surface is withheld.
        let toast = if model.privacy_hidden {
            None
        } else {
            model.celebration.as_ref().and_then(|c| {
                c.toast.as_ref().map(|t| FeedToast {
                    item_id: c.item_id.clone(),
                    value: t.value.clone(),
                    symbol: t.symbol.clone(),
                    deadline_ms: t.deadline_ms,
                })
            })
        };

        FeedView {
            rows,
            transactions: model.records.clone(),
            new_item_id: model.celebration.as_ref().map(|c| c.item_id.clone()),
            toast,
            history_empty_key: history_empty_key(model.chain_filter).to_owned(),
            home_empty_key: home_empty_key(model.chain_filter).to_owned(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: FeedShellResult) -> Command<FeedEffect, Event> {
    match result {
        FeedShellResult::StoreLoaded {
            records,
            now_ms,
            read_id,
        } => {
            // The raw account-scoped list (`loadActivityTransactions`).
            let lc = model.address.to_lowercase();
            model.records = records
                .iter()
                .filter(|t| match t.kind() {
                    FeedTxKind::Receive => t.to.to_lowercase() == lc,
                    // 083 H2: a dApp's transaction is this account's money
                    // moving too — its detail needs the record like a send's.
                    FeedTxKind::Send | FeedTxKind::DappTx => t.from.to_lowercase() == lc,
                    // Message signatures and connects move nothing: never rows.
                    FeedTxKind::SignMessage | FeedTxKind::SignTypedData | FeedTxKind::Connect => {
                        false
                    }
                })
                .cloned()
                .collect();

            // Fold, then tombstone-filter — the reload path is the ONLY
            // setter, and it is the tombstones' single enforcement point
            // (invariant ⑤).
            let mut items = build_items(&records, &model.address);
            items.retain(|item| !model.tombstones.contains(&item.id));
            model.items = items;

            let mut commands = Vec::new();

            // A post-sync re-read may celebrate its newest incoming item —
            // never the first pass (invariant ③ was consumed upstream: the
            // flag is only ever set after `initialized`).
            // Only the read the sync named may celebrate. A stale read that
            // raced ahead commits its records and leaves the flag standing.
            if model.celebrate_read_id == Some(read_id) {
                model.celebrate_read_id = None;
                let newest_in = model
                    .items
                    .iter()
                    .find(|item| item.direction == FeedDirection::In);
                if let Some(item) = newest_in {
                    // Incoming items always carry a value; if one ever
                    // doesn't, fail closed: no toast rather than a wrong one.
                    if let Some(value) = item.value.clone() {
                        model.generation = model.generation.wrapping_add(1);
                        let generation = model.generation;
                        model.celebration = Some(Celebration {
                            item_id: item.id.clone(),
                            toast: Some(ToastState {
                                value,
                                symbol: item.symbol.clone(),
                                deadline_ms: now_ms + f64::from(TOAST_MS),
                                generation,
                            }),
                        });
                        commands.push(shell_request(model.attempt, FeedOperation::Haptic));
                        commands.push(shell_request(
                            model.attempt,
                            FeedOperation::Timer {
                                ms: TOAST_MS,
                                generation,
                            },
                        ));
                    }
                }
            }

            // Alias resolution — memoised per session: items already carrying
            // a stored local name never ask (invariant ⑦'s "local wins"),
            // attempted addresses never ask again
            // (`useHomeController.ts:422-427`).
            for item in &model.items {
                if item.alias.is_some() {
                    continue;
                }
                let Some(addr) = &item.counterparty else {
                    continue;
                };
                let addr = addr.to_lowercase();
                if !model.alias_attempted.insert(addr.clone()) {
                    continue;
                }
                commands.push(shell_request(
                    model.attempt,
                    FeedOperation::ResolveRecipientIdentity { addr },
                ));
            }

            commands.push(render());
            Command::all(commands)
        }

        FeedShellResult::SyncCompleted { new_count } => {
            // `initializedRef.current = true` runs whether or not anything
            // landed — the first pass is spent either way (invariant ③).
            let first_pass = !model.initialized;
            model.initialized = true;
            if new_count == 0 {
                // No re-read: the feed never flickers behind a no-op sync
                // (invariant ②).
                return Command::done();
            }
            let (read_id, op) = read_store(model);
            if !first_pass {
                // Name the read this celebration belongs to — any OTHER
                // StoreLoaded, including one already in flight, leaves it alone.
                model.celebrate_read_id = Some(read_id);
            }
            shell_request(model.attempt, op)
        }

        FeedShellResult::AliasResolved { addr, name } => match name {
            // `if (id?.name)` — an empty name is falsy in TS; same here.
            Some(name) if !name.is_empty() => {
                model.alias_map.insert(addr.to_lowercase(), name);
                render()
            }
            _ => Command::done(),
        },

        // The `.finally()` port: the tombstone drops on failure too, so a
        // failed delete lets the next reload resurrect the row — ported
        // verbatim (the record really is still in storage).
        FeedShellResult::DeleteCommitted { id } | FeedShellResult::DeleteFailed { id } => {
            model.tombstones.remove(&id);
            Command::done()
        }

        FeedShellResult::ToastExpired { generation } => {
            if let Some(celebration) = &mut model.celebration {
                let current = celebration
                    .toast
                    .as_ref()
                    .is_some_and(|t| t.generation == generation);
                if current {
                    // The toast goes; the row glow stays (`newItemId` is
                    // never cleared by the timer).
                    celebration.toast = None;
                    return render();
                }
            }
            // A superseded timer (`clearTimeout` in TS) — a no-op.
            Command::done()
        }

        FeedShellResult::HapticPlayed => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Pure folding — the `loadActivityItems` port (`activity.ts:428-478`)
// ---------------------------------------------------------------------------

/// Fold raw records into feed items: same-id dedupe first, then batch
/// folding by shared `userOpHash`, newest-first (invariant ①).
fn build_items(records: &[FeedTxRecord], address: &str) -> Vec<FeedItem> {
    if address.is_empty() {
        return Vec::new();
    }
    let lc = address.to_lowercase();

    // Group 'send' siblings by userOpHash — deduping by id FIRST so a legacy
    // same-id duplicate (a resubmitted single send) is NOT mistaken for a
    // batch; only genuinely distinct lines count toward the group size
    // (`activity.ts:439-451`).
    let mut send_groups: BTreeMap<&str, Vec<&FeedTxRecord>> = BTreeMap::new();
    let mut group_seen: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    for t in records {
        if t.kind() != FeedTxKind::Send {
            continue;
        }
        if t.from.to_lowercase() != lc || t.user_op_hash.is_empty() {
            continue;
        }
        let ids = group_seen.entry(&t.user_op_hash).or_default();
        if !ids.insert(&t.id) {
            continue; // same-id duplicate — count the line once
        }
        send_groups.entry(&t.user_op_hash).or_default().push(t);
    }

    // `item.id` is the list key — guard against legacy duplicate-id records
    // so a row can never render twice, and skip every member of a batch once
    // its grouped row has been emitted (`activity.ts:453-476`).
    let mut items = Vec::new();
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    for t in records {
        if seen.contains(t.id.as_str()) {
            continue;
        }
        let item = match t.kind() {
            FeedTxKind::Receive if t.to.to_lowercase() == lc => Some(receive_item(t)),
            FeedTxKind::Send if t.from.to_lowercase() == lc => {
                let group = if t.user_op_hash.is_empty() {
                    None
                } else {
                    send_groups.get(t.user_op_hash.as_str())
                };
                match group {
                    Some(group) if group.len() > 1 => {
                        for member in group {
                            seen.insert(&member.id);
                        }
                        batch_item(group)
                    }
                    _ => Some(send_item(t)),
                }
            }
            FeedTxKind::DappTx if t.from.to_lowercase() == lc => Some(dapp_item(t)),
            _ => None,
        };
        let Some(item) = item else {
            continue;
        };
        seen.insert(&t.id);
        items.push(item);
    }

    // `sort((a, b) => b.timestamp - a.timestamp)` — stable in both runtimes.
    items.sort_by(|a, b| b.timestamp.total_cmp(&a.timestamp));
    items
}

/// `receiveRecordToActivity` — structured (no pre-formatting).
fn receive_item(t: &FeedTxRecord) -> FeedItem {
    FeedItem {
        id: t.id.clone(),
        direction: FeedDirection::In,
        counterparty: Some(t.from.clone()),
        // Receives never carry a stored name (toName is the send-side field).
        alias: None,
        value: Some(t.value.clone()),
        symbol: t.symbol.clone(),
        decimals: Some(t.decimals),
        usd_value: tx_usd_value(t),
        chain_id: t.chain_id,
        timestamp: t.timestamp,
        day_start_ms: t.day_start_ms,
        tx_hash: non_empty(&t.tx_hash),
        batch: None,
        kind: FeedTxKind::Receive,
        status: t.status,
        site: None,
        counterparty_role: FeedCounterpartyRole::Recipient,
        dapp: None,
    }
}

/// `sendTxToActivity` — structured.
fn send_item(t: &FeedTxRecord) -> FeedItem {
    FeedItem {
        id: t.id.clone(),
        direction: FeedDirection::Out,
        counterparty: Some(t.to.clone()),
        alias: t.to_name.clone(),
        value: Some(t.value.clone()),
        symbol: t.symbol.clone(),
        decimals: Some(t.decimals),
        usd_value: tx_usd_value(t),
        chain_id: t.chain_id,
        timestamp: t.timestamp,
        day_start_ms: t.day_start_ms,
        tx_hash: non_empty(&t.tx_hash),
        batch: None,
        kind: FeedTxKind::Send,
        status: t.status,
        site: None,
        counterparty_role: FeedCounterpartyRole::Recipient,
        dapp: None,
    }
}

/// A dApp's transaction (083 H2, 079 D3). Every client has stored these since
/// the signing path learned to (`buildSigningRecord`) and the feed dropped
/// them, so a swap stuck in the relay was visible nowhere. Status needs
/// nothing here: the tracker settles the record like a send's, and the
/// detail reads it from the record.
///
/// The stored `value` is the call's own wei figure as the page sent it
/// (`0x…`), scaled here into the human decimal every other row carries. A
/// call that moves no coin shows no amount rather than "0 ETH" — what it did
/// to tokens is the intent's to say, not a figure this record holds.
///
/// `decimals` is read off the disk, so it is bounded before it sizes
/// anything: the scaling pads to `decimals + 1` digits, and a corrupted or
/// imported row saying four billion would take the feed down on every read
/// (083 H2 review). Every client writes 18; past [`MAX_DAPP_DECIMALS`] the row
/// shows no figure rather than a guess.
///
/// What the operation moved comes first when the record kept it (083 F1):
/// the sheet's own simulation, recorded at approve time. Its one outflow is
/// the row's figure — a swap of 0.1 USDC reads "≈ −0.1 USDC", not nothing —
/// and a swap-shaped operation's one inflow rides beside it as `received`.
/// Anything else (no lines kept, two coins out, an outflow the sheet could
/// not put a number on) leaves the figure to the call's own value, as before.
///
/// Two rules keep the page's hand off the figure (083 F1 review):
/// - only the chain's coin or a token the wallet already trusts can be it. An
///   outflow renders on the sheet on metadata alone, and a contract a site
///   deployed can emit `Transfer(you, …)` and answer `symbol()` with "USDC";
///   on the sheet that overstates a spend, but as Activity's headline — and
///   priced by its symbol — it would be the site writing the wallet's record.
///   Such a line stays in the detail, as the sheet drew it.
/// - a FAILED operation moved nothing, so it keeps none of what the sheet
///   expected: no figure from it, nothing "≈ back", no lines — the row draws
///   as before, beside its failed status.
///
/// Who the row names (spec 082 RJ16, G52): a call that is exactly an ERC-20
/// `transfer(address,uint256)` names the transfer's recipient (EIP-55) as the
/// recipient; any other call names `to` as the contract it went to; no call
/// data names `to` as the recipient. And an operation hash is never an
/// explorer link: a `tx_hash` equal to the record's `user_op_hash` (a relay
/// rejection, a batch id) is no tx hash.
fn dapp_item(t: &FeedTxRecord) -> FeedItem {
    let recorded = if t.status == FeedTxStatus::Failed {
        Vec::new()
    } else {
        recorded_changes(t)
    };
    let figured = |line: &&RecordedChange| {
        line.known
            && line.change.verified
            && line.change.value.is_some()
            && !line.change.symbol.is_empty()
    };
    let only = |direction: FeedDirection| {
        let mut lines = recorded.iter().filter(|l| l.change.direction == direction);
        match (lines.next(), lines.next()) {
            (Some(line), None) => Some(line),
            _ => None,
        }
    };
    let taken = only(FeedDirection::Out).filter(figured);
    let received = taken
        .and(only(FeedDirection::In))
        .filter(figured)
        .map(|line| line.change.clone());
    let estimated = taken.is_some_and(|line| !line.change.exact);
    let (value, symbol, decimals) = match taken {
        Some(out) => (
            out.change.value.clone(),
            out.change.symbol.clone(),
            out.change.decimals,
        ),
        None => {
            let value = wei(&t.value)
                .filter(|wei| *wei > 0 && t.decimals <= MAX_DAPP_DECIMALS)
                .map(|wei| super::fee_policy::from_base_units(wei, t.decimals));
            let symbol = if value.is_some() {
                t.symbol.clone()
            } else {
                String::new()
            };
            let decimals = value.as_ref().map(|_| t.decimals);
            (value, symbol, decimals)
        }
    };
    let usd_value = value.as_ref().map_or(0.0, |value| {
        tx_usd_value(&FeedTxRecord {
            value: value.clone(),
            symbol: symbol.clone(),
            ..t.clone()
        })
    });
    let intent = t
        .intent
        .as_deref()
        .map(str::trim)
        .filter(|intent| !intent.is_empty())
        .map(str::to_owned);
    // The contract or recipient — and only when it IS an address. A batch
    // (`wallet_sendCalls`) submits no top-level `to`, so whatever a record
    // holds there is the page's to write, and the sheet never showed it: text
    // that is not an address is no counterparty to draw, shorten or look a
    // name up for (083 H2 review). A contract deployment has none either.
    let to = Some(t.to.trim())
        .filter(|to| super::contacts::is_address(to))
        .map(str::to_owned);
    let call = t
        .call_data
        .as_deref()
        .map(str::trim)
        .filter(|data| !data.is_empty() && *data != "0x" && *data != "0X");
    let recipient = call.and_then(transfer_recipient);
    let contract_call = recipient.is_none() && (call.is_some() || t.calldata == Some(true));
    let counterparty_role = if contract_call {
        FeedCounterpartyRole::Contract
    } else {
        FeedCounterpartyRole::Recipient
    };
    // A stored name belongs to the address it was stored beside (`to`), never
    // to a recipient read out of the call.
    let alias = match recipient {
        Some(_) => None,
        None => to.as_ref().and(t.to_name.clone()),
    };
    let counterparty = recipient.or(to);
    let site = t.dapp_url.as_deref().and_then(site_of);
    let tx_hash = non_empty(&t.tx_hash)
        .filter(|hash| t.user_op_hash.is_empty() || !hash.eq_ignore_ascii_case(&t.user_op_hash));
    FeedItem {
        id: t.id.clone(),
        direction: FeedDirection::Out,
        alias,
        counterparty,
        symbol,
        decimals,
        value,
        usd_value,
        chain_id: t.chain_id,
        timestamp: t.timestamp,
        day_start_ms: t.day_start_ms,
        tx_hash,
        batch: None,
        kind: FeedTxKind::DappTx,
        status: t.status,
        site: site.clone(),
        counterparty_role,
        dapp: Some(FeedDapp {
            site,
            intent_term: intent.as_deref().and_then(ClearTerm::of),
            intent,
            received,
            estimated,
            changes: recorded.into_iter().map(|line| line.change).collect(),
            contract_call,
        }),
    }
}

/// The recipient of call data that is exactly `transfer(address,uint256)`:
/// the selector, a 32-byte address word with its top 12 bytes zero, and a
/// 32-byte amount — nothing more, nothing less. EIP-55 spelled; `None` for
/// anything else.
fn transfer_recipient(data: &str) -> Option<String> {
    const TRANSFER_SELECTOR: &str = "a9059cbb";
    let hex = data
        .strip_prefix("0x")
        .or_else(|| data.strip_prefix("0X"))?;
    if hex.len() != 8 + 64 + 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if !hex[..8].eq_ignore_ascii_case(TRANSFER_SELECTOR) {
        return None;
    }
    let word = &hex[8..72];
    if word[..24].bytes().any(|b| b != b'0') {
        return None;
    }
    crate::primitives::checksum_address(&format!("0x{}", &word[24..])).ok()
}

/// One recorded line, with what the row needs to know of it and the wire
/// does not carry: whether its coin is one the wallet trusts (the chain's
/// own, or `TrustSimJudgment::Erc20Trusted::in_trusted_set`).
struct RecordedChange {
    change: FeedDappChange,
    known: bool,
}

/// The record's balance changes as lines a person reads (083 F1).
///
/// Only what the record kept from the approve, and only lines that say
/// something: a delta that is zero moved nothing, and one that does not even
/// read as a signed number has no direction to state. The sheet's judgment
/// is kept as it was made — an unverified line stays figureless here, however
/// large its delta.
///
/// A line is `exact` only when the wallet can vouch for it: the chain's coin
/// leaving in exactly the amount the transaction itself sent (`value`, which
/// the wallet submitted). Everything else is the simulation's expectation.
fn recorded_changes(t: &FeedTxRecord) -> Vec<RecordedChange> {
    let Some(judgments) = t.balance_changes.as_ref() else {
        return Vec::new();
    };
    // The chain's coin: the record names it (every client stores the native
    // symbol on a dApp row), else the built-in table.
    let native = Some(t.symbol.trim())
        .filter(|symbol| !symbol.is_empty())
        .map(str::to_owned)
        .or_else(|| {
            super::network_admin::BUILTIN_CHAINS
                .iter()
                .find(|chain| chain.chain_id == t.chain_id)
                .map(|chain| chain.native_symbol.to_owned())
        })
        .unwrap_or_default();
    let sent = wei(&t.value);
    judgments
        .iter()
        .filter_map(|judgment| {
            let (delta, verified, known, symbol, decimals) = match judgment {
                TrustSimJudgment::Native { delta } => (delta, true, true, native.clone(), Some(18)),
                TrustSimJudgment::Erc20Trusted {
                    delta,
                    symbol,
                    decimals,
                    in_trusted_set,
                    ..
                } => (
                    delta,
                    true,
                    *in_trusted_set,
                    symbol.trim().to_owned(),
                    Some(*decimals),
                ),
                TrustSimJudgment::Erc20Unverified { delta, .. } => {
                    (delta, false, false, String::new(), None)
                }
            };
            let (out, magnitude) = signed_delta(delta)?;
            // A coin arriving that the wallet does not already trust is the
            // one line a site fully controls (its own token, any symbol, any
            // figure): the record draws it as an unverified token whatever
            // the storing shell judged, so no stored row can say "≈ +1000
            // USDC" for it. An outflow is of a coin the account holds, which
            // the trusted set covers.
            let verified = verified && (known || out);
            let symbol = if verified { symbol } else { String::new() };
            let value = magnitude
                .filter(|_| verified)
                .zip(decimals.filter(|decimals| *decimals <= MAX_DAPP_DECIMALS))
                .map(|(magnitude, decimals)| {
                    super::fee_policy::from_base_units(magnitude, decimals)
                });
            // What the wallet itself sent, leaving whole: nothing else is
            // provable from the record (an exact-output swap's outflow is an
            // estimate too, and nothing here says which kind was signed).
            let exact = matches!(judgment, TrustSimJudgment::Native { .. })
                && out
                && value.is_some()
                && magnitude.is_some()
                && magnitude == sent;
            Some(RecordedChange {
                change: FeedDappChange {
                    direction: if out {
                        FeedDirection::Out
                    } else {
                        FeedDirection::In
                    },
                    verified,
                    symbol,
                    decimals: value.as_ref().and(decimals),
                    value,
                    exact,
                },
                known,
            })
        })
        .collect()
}

/// A signed base-unit delta: whether it leaves the account, and its size
/// when it fits a `u128`. `None` for text that is not a signed integer, and
/// for zero — nothing moved.
fn signed_delta(delta: &str) -> Option<(bool, Option<u128>)> {
    let delta = delta.trim();
    let (out, digits) = match delta.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, delta.strip_prefix('+').unwrap_or(delta)),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if digits.bytes().all(|b| b == b'0') {
        return None;
    }
    Some((out, digits.parse::<u128>().ok()))
}

/// A JSON-RPC quantity — `0x` hex as a page sends it, or decimal digits.
/// `None` when it is neither, or beyond `u128`: no figure beats a wrong one.
fn wei(value: &str) -> Option<u128> {
    let value = value.trim();
    match value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        Some("") => Some(0),
        Some(hex) => u128::from_str_radix(hex, 16).ok(),
        None if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) => {
            value.parse().ok()
        }
        None => None,
    }
}

/// The site a record's origin names, as a person reads it (083 H2) — the
/// same `host[:port]` the browser's address bar shows (spec 082 RG1):
/// `https://app.uniswap.org` → `app.uniswap.org`, `http://127.0.0.1:5173` →
/// `127.0.0.1:5173`.
///
/// Only an origin — a scheme and a host. A bare word is not one, and a value
/// that never carried a scheme is a name somebody typed, not an address the
/// request came from (083 H2 review). An origin that will not parse names
/// nothing rather than something half-read — and neither does `"null"`, the
/// origin an opaque page (`data:`, a sandboxed frame) reports.
fn site_of(origin: &str) -> Option<String> {
    let origin = origin.trim();
    if origin.eq_ignore_ascii_case("null") || !origin.contains("://") {
        return None;
    }
    dapp_site(origin)
}

/// `host[:port]` of an http(s) origin — lower-cased host, default port
/// dropped, as the browser's address bar names it (spec 082 RG1). `None` for
/// an origin that is not http(s) or does not parse.
pub fn dapp_site(origin: &str) -> Option<String> {
    let origin = super::dapp_permissions::origin_of(origin.trim())?;
    origin
        .split_once("://")
        .map(|(_, host_port)| host_port.to_owned())
}

/// A wei amount (hex `0x…` or decimal) as a human decimal of the native coin
/// — the figure a dApp row shows for the call's own value; `None` for zero,
/// and for anything that is not a whole number that fits.
pub fn native_amount(value: &str) -> Option<String> {
    wei(value)
        .filter(|wei| *wei > 0)
        .map(|wei| super::fee_policy::from_base_units(wei, 18))
}

/// `batchSendToActivity`: one row for the whole group, per-line breakdown
/// attached. Split sums to one token figure; multi_select can't sum mixed
/// tokens, so `value` is `None` and the shell leads with the fiat total +
/// asset count.
fn batch_item(group: &[&FeedTxRecord]) -> Option<FeedItem> {
    let first = group.first()?;
    let batch = build_batch(group)?;
    let split = batch.kind == FeedBatchKind::Split;
    let value = if split {
        // `transfers.reduce((s, x) => s + (parseFloat(x.value) || 0), 0)`.
        let sum: f64 = batch
            .transfers
            .iter()
            .map(|x| {
                let v = js_parse_float(&x.value);
                if v.is_finite() {
                    v
                } else {
                    0.0
                }
            })
            .sum();
        Some(format!("{sum}"))
    } else {
        None
    };
    Some(FeedItem {
        // `b.userOpHash || group[0].id` — ported fallback.
        id: if batch.user_op_hash.is_empty() {
            first.id.clone()
        } else {
            batch.user_op_hash.clone()
        },
        direction: FeedDirection::Out,
        counterparty: batch.to.clone(),
        alias: batch.to_name.clone(),
        value,
        symbol: batch.symbol.clone().unwrap_or_default(),
        decimals: if split { Some(first.decimals) } else { None },
        usd_value: batch.total_usd,
        chain_id: batch.chain_id,
        timestamp: batch.timestamp,
        day_start_ms: first.day_start_ms,
        tx_hash: non_empty(&batch.tx_hash),
        kind: FeedTxKind::Send,
        // `build_batch` took the first line's.
        status: batch.status,
        site: None,
        counterparty_role: FeedCounterpartyRole::Recipient,
        batch: Some(batch),
        dapp: None,
    })
}

/// `buildBatchView` (`activity.ts:210-236`) — the kind ternary ported
/// verbatim, ambiguous mixed groups included.
fn build_batch(group: &[&FeedTxRecord]) -> Option<FeedBatch> {
    let first = group.first()?;
    let symbols: BTreeSet<&str> = group.iter().map(|g| g.symbol.as_str()).collect();
    let recipients: BTreeSet<String> = group.iter().map(|g| g.to.to_lowercase()).collect();
    let kind = if symbols.len() <= 1 && recipients.len() > 1 {
        FeedBatchKind::Split
    } else if recipients.len() <= 1 && symbols.len() > 1 {
        FeedBatchKind::MultiSelect
    } else if symbols.len() <= 1 {
        FeedBatchKind::Split
    } else {
        FeedBatchKind::MultiSelect
    };
    let split = kind == FeedBatchKind::Split;
    Some(FeedBatch {
        kind,
        count: u32::try_from(group.len()).unwrap_or(u32::MAX),
        total_usd: group.iter().map(|g| tx_usd_value(g)).sum(),
        transfers: group
            .iter()
            .map(|g| FeedBatchTransfer {
                to: g.to.clone(),
                to_name: g.to_name.clone(),
                value: g.value.clone(),
                symbol: g.symbol.clone(),
                decimals: g.decimals,
                usd_value: tx_usd_value(g),
                logo_urls: g.logo_urls.clone(),
            })
            .collect(),
        ids: group.iter().map(|g| g.id.clone()).collect(),
        from: first.from.clone(),
        chain_id: first.chain_id,
        timestamp: first.timestamp,
        status: first.status,
        tx_hash: first.tx_hash.clone(),
        user_op_hash: first.user_op_hash.clone(),
        symbol: if split {
            Some(first.symbol.clone())
        } else {
            None
        },
        logo_urls: if split { first.logo_urls.clone() } else { None },
        to: if split { None } else { Some(first.to.clone()) },
        to_name: if split { None } else { first.to_name.clone() },
    })
}

// ---------------------------------------------------------------------------
// Valuation — the `txUsdValue` port (`activity.ts:149-181`)
// ---------------------------------------------------------------------------

/// Numeric USD for a record. Prefers the value stored at event time; if
/// missing/zero but the token is a known stablecoin, falls back to the token
/// amount (≈ $1 each) — a received USDT never shows $0.00 (invariant ⑧).
pub fn tx_usd_value(t: &FeedTxRecord) -> f64 {
    usd_value_of(t.usd.as_deref(), &t.symbol, &t.value)
}

/// [`tx_usd_value`] over its three inputs, so a dApp row (whose stored
/// `value` is wei) is valued on its human amount.
fn usd_value_of(usd: Option<&str>, symbol: &str, value: &str) -> f64 {
    // `tx.usd ? parseFloat(tx.usd.replace(/[^0-9.]/g, '')) : 0` — an absent
    // OR empty string is falsy.
    let stored = match usd {
        Some(usd) if !usd.is_empty() => {
            let cleaned: String = usd
                .chars()
                .filter(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            js_parse_float(&cleaned)
        }
        _ => 0.0,
    };
    if stored.is_finite() && stored > 0.0 {
        return stored;
    }
    if is_stable(symbol) {
        // `parseFloat(tx.value || '0')`.
        let raw = if value.is_empty() { "0" } else { value };
        let amount = js_parse_float(raw);
        if amount.is_finite() && amount > 0.0 {
            return amount;
        }
    }
    0.0
}

/// `isStable`: upper-case, fold the Tether glyph "₮" to "T" so the on-chain
/// "USD₮0" matches "USDT0" (`stableKey`, ported verbatim).
pub fn is_stable(symbol: &str) -> bool {
    let key = symbol.to_uppercase().replace('₮', "T");
    STABLE_SYMBOLS.iter().any(|s| *s == key)
}

/// JS `parseFloat`: longest valid numeric prefix after leading whitespace;
/// no parseable prefix ⇒ NaN. (The `Infinity` literal is not accepted here —
/// no feed string ever contains it, and rejecting it fails closed to NaN ⇒ 0.)
fn js_parse_float(s: &str) -> f64 {
    let t = s.trim_start();
    let b = t.as_bytes();
    let mut i = 0usize;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let mut digits = 0usize;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
        digits += 1;
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return f64::NAN;
    }
    let mut end = i;
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let mut exp_digits = 0usize;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
            exp_digits += 1;
        }
        if exp_digits > 0 {
            end = j;
        }
    }
    t.get(..end)
        .and_then(|prefix| prefix.parse::<f64>().ok())
        .unwrap_or(f64::NAN)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// History's empty-line key for a chain filter (spec 082 RG5).
pub fn history_empty_key(chain_filter: Option<u32>) -> &'static str {
    match chain_filter {
        None => HISTORY_EMPTY_ALL,
        Some(_) => HISTORY_EMPTY_FILTERED,
    }
}

/// The home Activity's empty-line key for a chain filter (spec 082 RG5, RX:
/// the desktop home under a filter needs the "on this network" line).
pub fn home_empty_key(chain_filter: Option<u32>) -> &'static str {
    match chain_filter {
        None => HOME_EMPTY_ALL,
        Some(_) => HOME_EMPTY_FILTERED,
    }
}

/// `txHash || undefined`.
fn non_empty(s: &str) -> Option<String> {
    if s.is_empty() {
        None
    } else {
        Some(s.to_owned())
    }
}

/// The tick pipeline: paint from cache + discover receipts. The TS original
/// runs these sequentially inside one `loadData`; issuing both at once keeps
/// the same visible behavior (the read commits the cached feed the moment it
/// answers; the scan re-reads only when something landed).
fn load_pipeline(model: &mut Model) -> Command<FeedEffect, Event> {
    if model.address.is_empty() {
        return render();
    }
    let attempt = model.attempt;
    let (_, read_op) = read_store(model);
    Command::all([
        shell_request(attempt, read_op),
        shell_request(
            attempt,
            FeedOperation::ScanIncomingTransfers {
                address: model.address.clone(),
            },
        ),
        render(),
    ])
}

/// Issue one operation whose answer must match the current attempt.
fn shell_request(attempt: u64, operation: FeedOperation) -> Command<FeedEffect, Event> {
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

impl super::SplitEffect for FeedEffect {
    type Op = FeedOperation;
    fn into_shell(self) -> Option<crux_core::Request<FeedOperation>> {
        match self {
            FeedEffect::Render(_) => None,
            FeedEffect::Shell(request) => Some(request),
        }
    }
}
