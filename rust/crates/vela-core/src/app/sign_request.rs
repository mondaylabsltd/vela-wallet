//! Machine — dApp signing-request lifecycle (spec `017-crux-wallet-state`,
//! `sign_request`).
//!
//! ```text
//! RequestArrived ─► Reviewing ─approve─► GasPrecheck ─► (Sponsoring ─► FundingWait ─retry─┐)
//!      │4902/4100/TTL/replay                │ ok                                          │
//!      ▼                                    ▼                                             │
//!   refused                            Submitting ─OpSubmitted─► record(pending)          │
//!                                           │ final result                 ▲──────────────┘
//!                                           ▼
//!                       record-then-respond ─► settled (rid never signs twice)
//! ```
//!
//! Replaces the four synchronous refs of `dapp-connection.tsx` `approveRequest`
//! (`approveInFlightRef` / `signCancelledRef` / `fundingRidRef` /
//! `lastApproveOptsRef`) with explicit model state:
//!
//! - **BUG-2**: once a request was rejected (4001 sent) nothing may still
//!   submit or answer the same id again — a reject during the gas pre-check
//!   aborts the pipeline (`dapp-connection.tsx:705-709`); once submitting, a
//!   swipe is a *dismiss*, never a reject (`SigningRequestModal.tsx:34-42`).
//! - **BUG-3**: the whole approve pipeline is single-flight — a same-tick
//!   second tap finds `inflight` occupied and is ignored
//!   (`dapp-connection.tsx:632-633`).
//! - **③**: a funding retry replays the *same* rid with the *original capped
//!   opts* (`:918-937`); a late funding outcome never hijacks a newer request
//!   (`:860-864`).
//! - **§4**: the durable record precedes any result a dApp can poll
//!   (`:753-770`; `dapp-history.ts:47-206`).
//! - **F2/F3/F4**: responses go to the transport that *owns* the request, and
//!   sign/display/history use the request's own chain and dApp identity
//!   (`dapp-request-routing.ts`).
//! - **⑥/⑦**: a global chain switch cancels a global-chain pending sign with
//!   4001; an unsupported chain is refused 4902 before any UI; the granted
//!   account is reconciled (explicitly sequenced — the `setTimeout(0)` of
//!   `web-request.tsx:207` becomes an `AccountSwitched` ack) before the
//!   approval surface can act, and an address/grant mismatch is a 4100
//!   refusal, never a silent signer swap.
//! - **⑧**: an extension rid never signs twice in a session, a >5 min payload
//!   never signs at all, and only an explicit user reject carries 4001 — every
//!   other failure uses a recoverable code (`extension-bridge-transport.ts`).
//! - **⑨/⑩**: signing/submission/records use the capped `paramsOverride`; a
//!   batch is refused 5700 for unsupported required capabilities before the
//!   wallet is touched, and `approval_guard::enforce_no_unlimited` is called
//!   at the submit chokepoint for the single tx and every batch leg — unless
//!   the approval surface reported the person kept an unbounded amount as
//!   asked (`SignApproveOpts::unlimited_approved`).
//!
//! Wave-A kernels are composed in Rust: `approval_guard::enforce_no_unlimited`
//! rules at the submit throat, and `fee_policy::tempo_quote_is_stale` guards
//! the displayed-equals-signed Tempo fee before submission. `clear_signing` /
//! `approval_guard` view routing stays with those machines; the shell ANDs
//! this view's `confirm_gate_open` with `GuardView.confirm_allowed` and
//! `FeeView.confirm_fee_ready` (the single fee confirmation gate).
//!
//! Spec 082 (money safety on a bad network):
//!
//! - A submit whose reply was lost is recorded and tracked like an accepted
//!   one, under the locally computed hash (`OpSubmitted{maybe_sent}`, RA3).
//!   The page waits for the chain (owner ruling 2026-10-01, 083): the tx
//!   hash once it lands, the revert as an error, or — when the shell's wait
//!   ends first — "not confirmed yet"; never 4900, never the op hash.
//! - The tracker alone closes on-chain records (RA8): `on_submit` answers the
//!   page and patches nothing; [`ending_of`] + [`ending_state`] give the sheet
//!   its ending from the tracker, never from the answer alone.
//! - [`SignPhase`] words the sheet from the stage and the passkey prompt
//!   (`CeremonyStarted` / `CeremonyDone`, RA9).
//! - A dropped transport stops a pipeline still before the passkey, and
//!   `AskerGone` ends one whose page vanished (RB2).
//!
//! Spec 082 round 2:
//!
//! - **Write-ahead (RJ1)**: the record exists before the bytes leave.
//!   [`Event::OpSigned`] (after the passkey, the local hash and the head read,
//!   before any POST) persists the pending record "may have been sent" and
//!   hands the op to the tracker with no record (a POST is about to leave:
//!   no "not sent" until its verdict); only on its `RecordPersisted` does
//!   [`SignOperation::ClearToPost`] let the shell POST. The POST's verdict
//!   hands the record over — the relay accepting it also patches the record
//!   [`SignRecordClose::Admitted`]; a proven "not sent" before `OpSubmitted`
//!   deletes it ([`SignOperation::DeleteRecord`]) and tells the tracker to
//!   forget it ([`SignView::tracker_withdraw`]).
//! - **Refusals are errors (RJ3)**: a relay-refused op answers `-32603` with
//!   [`crate::user_op::REFUSED_DAPP_DETAIL`], a proven-never-sent one with
//!   [`crate::user_op::NOT_SENT_DAPP_DETAIL`] — never Ok + op hash, and the
//!   sheet never says "try again" for a refusal ([`SignView::failure_refused`],
//!   [`SignEndingState::Refused`]).
//! - **The answer follows the tracker (RJ4)**: [`Event::OpTracked`] answers the
//!   waiting page the moment the tracker knows the outcome.
//!
//! Spec 097 N4: a refusal (or "nothing was sent") the tracker reaches after
//! "Submitted" is held on the sheet, like a failure before the submit (096
//! F8): shown in its own words with Done and no "Try again" (the rid is
//! settled, ⑧), and answered once — when the person closes it, when a new
//! request takes the sheet, or at once when nobody is looking.
//!
//! Spec 097 E: a `wallet_sendCalls` is answered with its id — its user
//! operation's hash, EIP-5792's batch id — the moment the relay accepts it
//! (`OpSubmitted` through the write-ahead), and with that id however a later
//! wait ends; never the tx hash it landed in, which `wallet_getCallsStatus`
//! does not know. One rule for the four shells.
//!
//! Spec 097 G: and in the shape the request declared — EIP-5792 2.0.0's
//! `{ "id": … }`, 1.0's (or no version's) bare id ([`ok_answer`],
//! `dapp_rpc::send_calls_result`). The shells forward the answer untouched.
//!
//! Ported quirks and fail-closed divergences are doc-commented inline.

use crux_core::capability::Operation;
use crux_core::command::AbortHandle;
use crux_core::macros::effect;
use crux_core::render::{render, RenderOperation};
use crux_core::{App, Command};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::approval_guard::enforce_no_unlimited;
use super::fee_policy::{is_tempo_chain, tempo_quote_is_stale, FeeTier, TEMPO_FEE_TOKEN_DECIMALS};
use super::self_call_guard::{detect_self_call, enforce_no_self_call, SelfCallBlock};
use super::token_trust::TrustSimJudgment;
use super::tx_tracker::{TrackEntryView, TrackOutcome, TrackStatus};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// A sign request payload older than this must never be signed —
/// `REQUEST_TTL_MS` (`extension-bridge-transport.ts:68`). Applies to any
/// request that carries a payload timestamp.
pub const EXTENSION_REQUEST_TTL_MS: f64 = 5.0 * 60.0 * 1000.0;

/// EIP-1193 user rejection. The extension transport writes a durable
/// `rejected` for this code ONLY (`extension-bridge-transport.ts:184`).
pub const CODE_USER_REJECTED: i32 = 4001;
/// EIP-1193 unauthorized (§12.1.6 grant mismatch, `web-request.tsx:190-193`).
pub const CODE_UNAUTHORIZED: i32 = 4100;
/// EIP-3085 unrecognized chain (`use-dapp-signing.ts:26`).
pub const CODE_UNSUPPORTED_CHAIN: i32 = 4902;
/// EIP-5792 unsupported non-optional capability (`use-dapp-signing.ts:27`).
pub const CODE_UNSUPPORTED_CAPABILITY: i32 = 5700;
/// JSON-RPC invalid params (`dapp-connection.tsx:361`).
pub const CODE_INVALID_PARAMS: i32 = -32602;
/// JSON-RPC internal error — the generic failure code every non-classified
/// error takes (`dapp-connection.tsx:886, 943`).
pub const CODE_INTERNAL: i32 = -32603;

/// What a page is told when its request names a chain other than the one it
/// would be signed on (089) — a transaction's or a batch's `chainId`, a typed
/// document's domain `chainId`. Answered [`CODE_INVALID_PARAMS`] with
/// [`SignErrorKind::InvalidParams`]: the params are wrong for this
/// connection, which is how MetaMask answers the same request; never 4902
/// (the chain may well be supported) and never 4901, which says the provider
/// lost its connection rather than that the request was refused. For the
/// dApp's developer — EIP-1193 messages are not UI.
pub const CHAIN_MISMATCH_MESSAGE: &str = "chainId does not match the connected chain";

/// How long a dApp on the phones and the web waits for the answer to an
/// on-chain request, measured from the approve tap (spec 082 RA12) — the
/// whole budget, so a slow submit is not followed by a second full wait. The
/// desktop waits [`PAGE_WAIT_CAP_MS`] (083). Either way, a transaction with
/// no transaction hash by then is answered [`NOT_CONFIRMED_MESSAGE`], never
/// its op hash (owner ruling 2026-10-01, superseding 082 rulings 1 and 9):
/// the core reads a shell's [`SignSubmitOutcome::ReceiptPending`] for
/// `eth_sendTransaction` as [`SignSubmitOutcome::NotConfirmed`].
pub const DAPP_TX_ANSWER_WINDOW_MS: f64 = 120_000.0;
/// The shortest receipt wait after a slow submit: the relay still gets a
/// moment to land an op it has only just accepted.
pub const DAPP_RECEIPT_MIN_WAIT_MS: f64 = 10_000.0;

/// How long to wait for the receipt when the submit answered `elapsed_ms`
/// after the approve tap (RA12): what is left of
/// [`DAPP_TX_ANSWER_WINDOW_MS`], never less than
/// [`DAPP_RECEIPT_MIN_WAIT_MS`]. A lost reply decided after ~46 s of relay
/// timeouts therefore answers the page by ~120 s, not ~210 s — well inside
/// the extension page's own 300 s limit.
pub fn dapp_receipt_wait_ms(elapsed_ms: f64) -> f64 {
    let left = DAPP_TX_ANSWER_WINDOW_MS - elapsed_ms.max(0.0);
    left.max(DAPP_RECEIPT_MIN_WAIT_MS)
}

/// How long a shell keeps a dApp's `eth_sendTransaction` / `wallet_sendCalls`
/// waiting for its operation to land before it gives up and reports
/// [`SignSubmitOutcome::NotConfirmed`] (083).
///
/// The page is answered with the hash of the transaction that carried the
/// operation, once the operation is known to have executed — never with the
/// userOpHash, which no node knows: a site that looks it up on its OWN RPC
/// (Uniswap does) waits on "pending" forever. That
/// overturns 079 US1, which answered the operation hash when a 90 s window
/// ran out. Waiting costs a site nothing — EIP-1193 sets no deadline on a
/// wallet request, and a site already shows "confirm in wallet" meanwhile —
/// while the wallet's own column says the operation is still confirming.
///
/// Ten minutes is `tx_tracker`'s own line ([`super::tx_tracker::SLOW_POLL_AFTER_MS`])
/// between an operation that is slow and one that is not landing soon (a
/// fee hold, a stuck relay). Past it the page is answered an ERROR — the
/// truth, "not confirmed yet" — rather than a hash it can never find, and the
/// record stays pending for the tracker, which may still see it land.
pub const PAGE_WAIT_CAP_MS: f64 = super::tx_tracker::SLOW_POLL_AFTER_MS;

/// What a page is told when its operation was included and REVERTED — for
/// the dApp's developer (EIP-1193 messages are not UI); the bundle
/// transaction's hash follows it in parentheses.
pub const REVERTED_MESSAGE: &str = "The transaction was included but reverted";

/// The page's error for a reverted operation: [`REVERTED_MESSAGE`], then the
/// bundle transaction's hash in parentheses.
#[must_use]
pub fn reverted_detail(tx_hash: &str) -> String {
    format!("{REVERTED_MESSAGE} ({tx_hash})")
}

/// The transaction a [`reverted_detail`] names — `None` for any other
/// error. A shell reads it to draw the revert at once — its sentence ("a
/// network fee may still have been charged"), the hash and the explorer —
/// rather than "couldn't be submitted, your funds are safe", which is false
/// for an operation that was included, until its tracker catches up (083).
#[must_use]
pub fn reverted_transaction(detail: &str) -> Option<&str> {
    detail
        .strip_prefix(REVERTED_MESSAGE)?
        .strip_prefix(" (")?
        .strip_suffix(')')
        .filter(|hash| !hash.is_empty())
}

/// What a page is told when its operation had not landed by
/// [`PAGE_WAIT_CAP_MS`]; the operation's hash follows it, named as what it is.
pub const NOT_CONFIRMED_MESSAGE: &str =
    "The transaction was submitted but is not confirmed yet; it may still complete, so check the wallet before sending it again";

/// The page's error for an operation not confirmed yet:
/// [`NOT_CONFIRMED_MESSAGE`], then the operation's hash, named as what it is.
/// The extension's worker mirrors it for a surface that went (spec 082 RJ2).
#[must_use]
pub fn not_confirmed_detail(user_op_hash: &str) -> String {
    format!("{NOT_CONFIRMED_MESSAGE} (user operation {user_op_hash})")
}

/// Bound on the settled-rid registry, mirroring the bounded-map discipline of
/// `MAX_TRACKED_USEROPS` (`use-dapp-signing.ts:41`).
const MAX_SETTLED_RIDS: usize = 256;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// The dApp identity a request carries (F3): the extension origin for a
/// stamped request, else the connection's global `dappInfo` — the shell picks
/// per `requestDApp` and passes the winner here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignDappIdentity {
    pub name: String,
    pub url: Option<String>,
}

/// One wallet account as this machine needs it: the address to sign from and
/// the passkey credential that signs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignAccountRef {
    pub address: String,
    pub credential_id: String,
}

/// The displayed in-band fee, signed verbatim (displayed = signed;
/// `SigningSheet.tsx:558-564`). Amounts are decimal strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignQuotedFee {
    /// Base units of the fee asset, decimal string.
    pub amount: String,
    pub recipient: String,
    /// The speed the displayed fee was priced at (spec 069), named on the
    /// wire beside it — the shell copies it from the same estimate as
    /// `amount`, so the relay is told the tier the person saw. `None` (or a
    /// shell that predates it) names nothing: the pre-068 wire.
    #[serde(default)]
    pub tier: Option<FeeTier>,
}

/// The approve-tap payload — `approveRequest(opts)` (`dapp-connection.tsx:623`),
/// bigints as decimal strings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignApproveOpts {
    /// Wei, decimal string.
    pub max_fee_per_gas: Option<String>,
    /// Raw bundler cost driving the funding pre-check, wei decimal string.
    pub bundler_cost_wei: Option<String>,
    /// `None` = native (the `gasFeeToken` selection).
    pub gas_fee_token: Option<String>,
    pub quoted_fee: Option<SignQuotedFee>,
    /// The Tempo fee collector the fee machine currently displays — the
    /// submit-side staleness reference for `tempo_quote_is_stale`. `None`
    /// skips the recipient half of the check (floor still enforced).
    pub fee_collector: Option<String>,
    /// Rewritten (capped) params, a JSON array — invariant ⑨: when present,
    /// sign/submit/record THESE, never the original request
    /// (`dapp-connection.tsx:638`).
    pub params_override_json: Option<String>,
    /// Clear-signing intent captured at approve time, persisted on the record.
    pub intent: Option<String>,
    /// `approval_guard`'s `unlimited_consented`, copied verbatim: the sheet
    /// showed an unbounded approval and the person kept it as the site asked.
    /// The ONLY waiver of the submit chokepoint's `enforce_no_unlimited`;
    /// absent (a shell that predates it, a sheet that never mounted the
    /// approval surface) still refuses an unbounded amount.
    #[serde(default)]
    pub unlimited_approved: bool,
    /// What the wallet's OWN simulation said this operation moves, exactly as
    /// the sheet drew it under "Balance changes" when the slide fired
    /// (083 F1): `token_trust`'s judgments, in the sheet's order — an
    /// unverified token's line carries no figure here either. Kept on the
    /// record so Activity can say what was approved. It rides the approve and
    /// nothing else: no field of the request can reach it. `None` (a shell
    /// that predates it, a simulation that never answered) records nothing,
    /// and the row draws as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_changes: Option<Vec<TrustSimJudgment>>,
    /// The approval surface's token, as it resolved (`GuardView::meta`),
    /// copied verbatim (spec 093): what lets the record say "100 USDC"
    /// rather than a bare number. Read only once it resolved; `None` (no
    /// approval on the sheet, a shell that predates it) names the token from
    /// the built-in table, or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token_meta: Option<super::approval_guard::GuardTokenMetaView>,
    /// What the sheet's reading named, copied verbatim from
    /// `ClearSigningView::record_reading` (spec 097 N5, N8): the record keeps
    /// the contract's name and the coins' names, so Activity says what the
    /// sheet said. `None` (nothing named, a shell that predates it) names
    /// them from the built-in table, or not at all.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<super::dapp_activity::DappReading>,
}

/// Bundler gas-account funding facts (`FundingNeeded`), amounts as decimal
/// wei strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignFundingNeeded {
    pub deposit_address: String,
    pub safe_address: String,
    pub chain_id: u32,
    pub native_symbol: String,
    pub threshold_wei: String,
    pub recommended_wei: String,
    pub current_balance_wei: String,
}

/// Semantic error vocabulary — the shell owns the words (i18n keys only).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignErrorKind {
    /// 4001 — explicit user reject.
    UserRejected,
    /// 4001 — 'Cancelled: the wallet switched chains'.
    WalletSwitchedChains,
    /// 4902.
    UnsupportedChain,
    /// 4100 — §12.1.6 grant mismatch.
    UnauthorizedAccount,
    /// -32602 / -32603 — malformed or missing params.
    InvalidParams,
    /// 5700 — EIP-5792 required capability this wallet does not support.
    ///
    /// Ported divergence: the TS catch flattened this to -32603
    /// (`dapp-connection.tsx:886`) even though `use-dapp-signing.ts` threw
    /// 5700; inventory invariant ⑩ names 5700, so the honest code wins.
    UnsupportedCapability,
    /// -32603 — `enforce_no_unlimited` refused the final params (fail-closed).
    UnlimitedApproval,
    /// -32603 — the request would have rewritten who controls the account
    /// (spec 081, `self_call_guard`). Never 4001: the user did not reject it,
    /// the wallet refused it.
    SelfCallBlocked,
    /// -32603 — 'Gas account funding cancelled'.
    FundingCancelled,
    /// -32603 — submission failed; `detail` echoes the shell's own message.
    SubmitFailed,
    /// No response is sent for this one: the displayed Tempo fee quote went
    /// stale pre-submit (`fee_policy::tempo_quote_is_stale`) — the sheet must
    /// re-quote, never silently re-price.
    StaleFeeQuote,
}

/// What goes back to the dApp. `Ok { result: None }` serialises the `null`
/// success of `wallet_switchEthereumChain`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignResponsePayload {
    Ok {
        /// The page's `result`, exactly as it is delivered: a hash or a
        /// signature (a string) — or a `wallet_sendCalls` batch's id in the
        /// shape its request declared, `{ "id": … }` for EIP-5792 2.0.0
        /// (spec 097 G, [`super::dapp_rpc::send_calls_result`]). Shells forward it
        /// untouched; [`SignResponsePayload::answered`] reads the hash back.
        #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
        result: Option<Value>,
    },
    Err {
        code: i32,
        kind: SignErrorKind,
        message: Option<String>,
    },
}

impl SignResponsePayload {
    /// The hash or signature an `Ok` answer names — a batch's id read out of
    /// EIP-5792 2.0.0's `{ id }` ([`super::dapp_rpc::batch_id_of`]). `None`
    /// for an error or a `null` answer.
    pub fn answered(&self) -> Option<&str> {
        match self {
            Self::Ok {
                result: Some(result),
            } => super::dapp_rpc::batch_id_of(result),
            _ => None,
        }
    }
}

/// History-record lifecycle (`dapp-history.ts` `status`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignRecordStatus {
    Pending,
    Confirmed,
}

/// Which record shape `buildSigningRecord` builds (`dapp-history.ts:152-174`).
/// Ported quirk: a `wallet_sendCalls` batch is a `DappTx` record whose
/// `txHash` receives the batch id (the userOpHash) and whose `userOpHash`
/// stays empty — exactly what the TS builder produces.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignRecordKind {
    DappTx,
    SignTypedData,
    SignMessage,
}

/// The durable history record. The shell maps this onto `LocalTransaction`
/// (`buildSigningRecord`) and owns `capRequest` clipping and asset-sim
/// serialisation; `params_json` is always the FINAL (capped) params
/// (invariant ⑨).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignRecord {
    /// `dapp-<ms>-tx|typed|msg`, the TS id scheme verbatim.
    pub record_id: String,
    pub kind: SignRecordKind,
    pub method: String,
    pub params_json: String,
    /// Result of the request — `""` while pending.
    pub result: String,
    pub from: String,
    pub chain_id: u32,
    pub now_ms: f64,
    pub status: SignRecordStatus,
    pub user_op_hash: String,
    /// `requestDApp(...)?.name ?? origin` — what the Connections list shows.
    /// A name here is the dApp's own claim, so nothing reads it as a site.
    pub dapp_origin: String,
    /// The origin the request arrived from, as its transport reported it —
    /// never the dApp's self-declared name. Stored beside `dapp_origin`
    /// (`dappUrl`) so Activity can name the site from an address the dApp
    /// did not get to choose (083 H2 review). `""` when there is none.
    #[serde(default)]
    pub dapp_url: String,
    pub intent: Option<String>,
    /// The submit's reply was lost; `user_op_hash` is the locally computed
    /// hash (spec 082 RA3). Persisted with the record, so a restart hands the
    /// tracker a may-have-been-sent op again (`TrackPendingRecord`).
    #[serde(default)]
    pub maybe_sent: bool,
    /// The head read before the first submit POST — where the tracker's
    /// relay-independent landing check starts (ruling 8). Persisted likewise.
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub submit_block: Option<u64>,
    /// A transaction's balance changes as the person approved them — the
    /// sheet's simulation, [`SignApproveOpts::balance_changes`] (083 F1).
    /// Never a signature's, never empty, and absent from the wire when there
    /// are none, so older rows and shells that do not send them read as
    /// before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub balance_changes: Option<Vec<TrustSimJudgment>>,
    /// What the request was, as Activity states it (spec 093) — built once,
    /// at approve time, from the FULL final params. The shell stores it
    /// verbatim with the record (`dappSummary`) and hands it back to the
    /// feed untouched. `None` only on a record from before 093.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<super::dapp_activity::DappSummary>,
    /// The request as the record keeps it: `params_json`, at most
    /// [`super::dapp_activity::STORED_REQUEST_MAX_BYTES`] (spec 093). Every
    /// shell stores this, never its own cut of `params_json`.
    #[serde(default)]
    pub stored_request: String,
    /// `stored_request` is shorter than the request was.
    #[serde(default)]
    pub request_truncated: bool,
}

/// The in-place patch closing a pending record — same id, never a second
/// record (`dapp-connection.tsx:779-784, 880-884`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignRecordClose {
    Confirmed {
        tx_hash: String,
    },
    Failed,
    /// The relay accepted the write-ahead record's op (spec 082 RJ1): the
    /// record's `maybeSent` becomes false and it stays pending — only the
    /// tracker closes it.
    Admitted,
}

/// The one-shot settlement a rid reaches — the extension's durable outcomes
/// (`extension-bridge-transport.ts:53-56`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignSettledOutcome {
    Submitted,
    Rejected,
}

/// Why an arriving request never reached the sheet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignNotice {
    /// Payload older than [`EXTENSION_REQUEST_TTL_MS`] — never signed, no
    /// response written (the page recovers via the 4900 path).
    Expired,
    /// This rid already settled in this session — replay the outcome, never
    /// re-sign (`extension-bridge-transport.ts:112-128`).
    AlreadySettled { outcome: SignSettledOutcome },
}

/// The tx_tracker handoff: the shell feeds this to `tx_tracker::Event::Submitted`
/// the moment it appears (idempotent — the tracker merges by hash).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignTrackerHandoff {
    pub user_op_hash: String,
    /// The records the tracker patches. EMPTY on the write-ahead hand-off
    /// (spec 082 RJ1, second review): a POST of the op is about to leave and
    /// the tracker holds it off "not sent" until the POST's verdict, whose
    /// hand-off names the record. Forwarded as they are, empty or not.
    pub record_ids: Vec<String>,
    pub chain_id: u32,
    /// Forwarded to `tx_tracker::Event::Submitted` (spec 082 RA3).
    #[serde(default)]
    pub maybe_sent: bool,
    #[serde(default)]
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub submit_block: Option<u64>,
    /// Forwarded to `tx_tracker::Event::Submitted` (spec 082 RJ1): the relay
    /// accepted the op the write-ahead hand-off announced.
    #[serde(default)]
    pub admitted: bool,
}

/// A write-ahead record proven never sent (spec 082 RJ1): the shell feeds
/// this to `tx_tracker::Event::Withdrawn` the moment it appears (idempotent —
/// withdrawing ids already gone changes nothing).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignTrackerWithdraw {
    pub user_op_hash: String,
    pub record_ids: Vec<String>,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. The shell owns transports (the
/// `transport_id` → instance table), the 15 s pre-check race (a timeout
/// answers `PreCheck { funding: None }`, exactly as the TS race falls through
/// to submit), passkey ceremonies, bundler RPC, storage, and the message-regex
/// classification of submit failures into [`SignSubmitOutcome`].
///
/// Abortable operations (`CheckBundlerFunding`, `AttemptSponsorship`,
/// `SignAndSubmit` pre-passkey) must honour the bridge's
/// `cancelled_effect_ids` channel for a true abort.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignOperation"))]
// `PersistRecord` carries the whole record (spec 093 put its summary on it).
// Boxing it would buy an allocation per operation and change nothing a shell
// sees — this is a wire type whose JSON shape the generated TypeScript pins.
#[allow(clippy::large_enum_variant)]
pub enum SignOperation {
    /// Answer the transport that OWNS the request (F2) — never a shared ref.
    SendResponse {
        transport_id: String,
        id: String,
        payload: SignResponsePayload,
    },
    /// `checkBundlerFunding` raced with the 15 s timeout — shell-owned.
    /// `bust_cache` mirrors `clearBundlerCache` before a funding retry so the
    /// pre-check reads the freshly funded balance (`dapp-connection.tsx:927-933`).
    CheckBundlerFunding {
        chain_id: u32,
        account: String,
        bundler_cost_wei: Option<String>,
        bust_cache: bool,
    },
    /// `attemptSilentSponsorship` (can take ~25 s).
    AttemptSponsorship {
        funding: SignFundingNeeded,
        force: bool,
    },
    /// The passkey + build + submit pipeline (`handleDAppRequest`). The shell
    /// reports the accepted hash mid-flight via [`Event::OpSubmitted`] and
    /// resolves this operation once with the FINAL outcome.
    SignAndSubmit {
        id: String,
        method: String,
        /// FINAL (capped) params — invariant ⑨.
        params_json: String,
        chain_id: u32,
        address: String,
        credential_id: String,
        max_fee_per_gas: Option<String>,
        gas_fee_token: Option<String>,
        quoted_fee: Option<SignQuotedFee>,
    },
    /// Write a history record (`saveTransaction(buildSigningRecord(...))`).
    /// The shell must serialise Persist/Update per `record_id`.
    PersistRecord { record: SignRecord },
    /// Patch a record in place (`updateTransaction`).
    UpdateRecord {
        record_id: String,
        close: SignRecordClose,
    },
    /// The write-ahead record for request `id` is on disk (spec 082 RJ1): the
    /// shell may now POST `user_op_hash`, and only now — after the asker
    /// check (RB2), immediately before the POST. With no clearance within
    /// `user_op::WRITE_AHEAD_WAIT_MS` the shell does not POST and reports
    /// `Failed` (nothing sent). Answered `Responded`.
    ClearToPost { id: String, user_op_hash: String },
    /// Remove a write-ahead record whose op is proven never sent (RJ1). The
    /// shell serialises it with the record's other writes. Answered
    /// `RecordUpdated`.
    DeleteRecord { record_id: String },
    /// §12.1.6 — switch the active account to the granted one (dispatched to
    /// the session/wallet store); answered with `AccountSwitched` once the
    /// switch landed, which is what sequences "switch first, then the
    /// approval surface may act".
    SwitchActiveAccount { index: u32 },
}

/// Silent-sponsorship outcomes (`attemptSilentSponsorship`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignSponsorship {
    Funded,
    Confirming,
    Denied { reason: Option<String> },
}

/// The FINAL outcome of a `SignAndSubmit`. The shell's result-mapping layer
/// owns every wording regex (`parseBundlerUnderfunded`,
/// `PasskeyErrorCode.CANCELLED`) — the core only sees typed variants.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignSubmitOutcome {
    /// tx: the real tx hash from a receipt whose operation EXECUTED (a
    /// receipt with `success: false` is [`Self::Reverted`], never this);
    /// batch: the tx hash as for a transaction, which the core answers as the
    /// batch's id instead (spec 097 E: the page is answered the op hash, as
    /// [`Self::ReceiptPending`] is); signatures: the EIP-1271 signature hex.
    /// A batch id handed to the page the moment the relay accepts it (the
    /// web's EIP-5792 answer) is the userOpHash, not a receipt: it is
    /// [`Self::ReceiptPending`], or the record closes "confirmed" under a
    /// hash no explorer knows and nothing ever tracks it (083 H2 review). On
    /// the tx path the page is answered and the record left to the tracker,
    /// which alone closes on-chain records (spec 082 RA8, W3).
    Succeeded { result: String },
    /// The bundler accepted the op but its receipt did not arrive inside the
    /// shell's wait (~120 s on the phones and the web). Nothing is known to
    /// have landed, so the pending record is NOT closed here: the tx tracker
    /// (handed the op at [`Event::OpSubmitted`]) settles it to
    /// confirmed/failed when the receipt actually appears (issue 262: an
    /// un-reimbursable op sat in the bundler forever while its record claimed
    /// "confirmed"). The core never tries to tell a tx hash from an op hash —
    /// both are 32-byte hex — so the shell must say which one it holds.
    ///
    /// A site that looks the op hash up on its OWN node never finds it (083,
    /// Uniswap): for `eth_sendTransaction` the core answers this as
    /// [`Self::NotConfirmed`] (owner ruling 2026-10-01). Only a
    /// `wallet_sendCalls` batch id — the userOpHash by EIP-5792's own terms —
    /// is answered with it.
    ReceiptPending { user_op_hash: String },
    /// The operation was included and its execution REVERTED: the receipt
    /// said `success: false` (`UserOperationEvent.success`). The bundle
    /// transaction itself succeeded (status `0x1`), so a site reading that
    /// hash on its own node would call it done — Uniswap showed a swap that
    /// moved nothing (083). The page is answered ONE error
    /// ([`REVERTED_MESSAGE`]), never the hash, and the record closes failed.
    Reverted {
        user_op_hash: String,
        tx_hash: String,
    },
    /// The shell waited the whole [`PAGE_WAIT_CAP_MS`] and the relay reported
    /// no transaction for the op. The page is answered an error saying so
    /// ([`NOT_CONFIRMED_MESSAGE`]) — never the op hash, which is not a
    /// transaction — and, as with [`Self::ReceiptPending`], the record stays
    /// pending for the tracker: a timeout is not a failure.
    NotConfirmed { user_op_hash: String },
    /// User dismissed the passkey sheet — never an error, never a response
    /// (`dapp-connection.tsx:808-812`).
    PasskeyCancelled,
    /// Bundler gas account underfunded. `funding` is the composed facts
    /// (live account info with the parsed error as fallback,
    /// `dapp-connection.tsx:826-846`); `None` when no deposit address could
    /// be established → generic failure.
    Underfunded {
        message: String,
        funding: Option<SignFundingNeeded>,
    },
    Failed {
        message: String,
        /// The relay refused the op (spec 082 RJ3): a submit-time `NotSent`
        /// with a rejection that is not "relayer unavailable". The page is
        /// answered `user_op::REFUSED_DAPP_DETAIL`, whatever `message` says,
        /// and the sheet's failure is a refusal ([`SignView::failure_refused`]).
        #[serde(default)]
        refused: bool,
    },
    /// The shell proved, before the passkey or between the passkey and the
    /// relay POST, that the asking page no longer exists (spec 082 RB2):
    /// nothing was sent, nobody is left to answer, nothing is recorded, and
    /// the sheet clears if it still shows this request.
    AskerGone,
}

/// What the shell observed. Every clock-bearing variant carries `now_ms`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignShellResult"))]
pub enum SignShellResult {
    /// `None` = no funding needed OR the 15 s race timed out / errored — all
    /// of which proceed to submit (`dapp-connection.tsx:666-698`).
    PreCheck {
        funding: Option<SignFundingNeeded>,
    },
    Sponsorship {
        outcome: SignSponsorship,
    },
    Submit {
        outcome: SignSubmitOutcome,
        now_ms: f64,
    },
    Responded,
    RecordPersisted,
    RecordUpdated,
    AccountSwitched,
}

impl Operation for SignOperation {
    type Output = SignShellResult;
}

#[effect]
pub enum SignEffect {
    Render(RenderOperation),
    Shell(SignOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SignEvent"))]
pub enum Event {
    /// The supported network set (`getAllNetworksSync`). Until this arrives
    /// every chain is unsupported — fail-closed, so a shell that forgets to
    /// send it fails loudly instead of signing on an unvetted chain.
    NetworksChanged { chain_ids: Vec<u32> },
    /// Wallet accounts snapshot + active index (the `accountsRef` /
    /// `activeAccountRef` mirrors, as events).
    AccountsChanged {
        accounts: Vec<SignAccountRef>,
        active_index: u32,
    },
    /// A signing request landed (`handleIncoming` for signing methods).
    /// `dedicated_transport` = the request rides its own one-shot transport
    /// (`__transport` stamped: extension / web popup); `per_request_chain` =
    /// `__chainId` (F4). `request_ts_ms` is the payload timestamp when the
    /// source has one (the extension mailbox `ts`).
    RequestArrived {
        id: String,
        method: String,
        /// The raw JSON-RPC params array, verbatim and untrusted.
        params_json: String,
        origin: String,
        transport_id: String,
        dedicated_transport: bool,
        per_request_chain: Option<u32>,
        dapp: Option<SignDappIdentity>,
        /// §12.1.6: the address the origin was granted.
        granted_address: Option<String>,
        /// §12.1.6: the address the request asks to act as (popup path).
        requested_address: Option<String>,
        request_ts_ms: Option<f64>,
        now_ms: f64,
    },
    /// `wallet_switchEthereumChain` (dApp-driven when `id` is present) or an
    /// in-wallet chain switch (`id: None`). `chain_id_param` is the raw
    /// `params[0].chainId` string — hex or decimal, parsed here so the
    /// -32602 / 4902 split matches `dapp-connection.tsx:354-388`.
    ChainSwitchRequested {
        id: Option<String>,
        transport_id: Option<String>,
        chain_id_param: Option<String>,
    },
    /// The slide-to-confirm fired.
    ApproveTapped { opts: SignApproveOpts },
    /// Explicit reject (sheet closed pre-submit).
    RejectTapped,
    /// Close after an error / after submission — response already handled.
    DismissTapped,
    /// "Try again" on a failure that sent nothing (spec 096 F8,
    /// [`SignView::failure_retryable`]): the request goes back to review,
    /// still unanswered.
    RetryTapped,
    /// The modal was swipe-dismissed — the core dispatches by phase
    /// (`SigningRequestModal.tsx:34-42`): funding view → funding cancel;
    /// error / submitted / submitting → dismiss; else → reject.
    SwipeDismissed,
    /// Funding view "Continue" after a top-up.
    FundingCompleteTapped,
    /// Funding view cancel (and swipe over the funding view).
    FundingCancelled,
    /// The bundler accepted the op (`onSubmitted`) — mid-flight, before the
    /// final `Submit` result. Accepted (not `maybe_sent`) through the
    /// write-ahead, a `wallet_sendCalls` is answered here with its id, the op
    /// hash (spec 097 E): the shell's later `Submit` is dropped.
    OpSubmitted {
        id: String,
        user_op_hash: String,
        now_ms: f64,
        /// The submit's reply was lost and `user_op_hash` is the local hash
        /// (spec 082 RA2/RA3): the op is recorded and tracked all the same,
        /// and the page is answered from the chain — the tx hash once it
        /// lands, the revert, or "not confirmed yet" — never 4900 and never
        /// that hash (owner ruling 2026-10-01).
        #[serde(default)]
        maybe_sent: bool,
        /// The head read before the first POST (ruling 8); `None` = unknown.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
    },
    /// The op for request `id` is signed and its hash computed — after the
    /// passkey, the local hash and the head read, BEFORE any POST (spec 082
    /// RJ1). The core writes the record ahead and answers
    /// [`SignOperation::ClearToPost`] once it is on disk. Accepted only in the
    /// submit stage, once per pipeline, before `OpSubmitted`.
    OpSigned {
        id: String,
        user_op_hash: String,
        /// The head read before the first POST (ruling 8); `None` = unknown.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        submit_block: Option<u64>,
        now_ms: f64,
    },
    /// The tracker's entry for an op changed (spec 082 RJ4): the shell
    /// forwards `status` and `tx_hash` of the entry for the in-flight op
    /// whenever they change. Once the op is past `OpSubmitted` and the page
    /// is still waiting: Confirmed with a tx hash answers Ok(tx hash) — a
    /// batch Ok(its id, the op hash; spec 097 E);
    /// Dropped with one (included, reverted) answers -32603
    /// [`reverted_detail`] (083, owner ruling 2026-10-01); Rejected answers
    /// -32603 refused; NotSent answers -32603 not sent; anything else waits.
    /// A refusal or "not sent" for the request on the sheet is held there
    /// until the close (spec 097 N4, the rule of 096 F8).
    OpTracked {
        user_op_hash: String,
        status: TrackStatus,
        #[serde(default)]
        tx_hash: Option<String>,
        now_ms: f64,
    },
    /// The passkey (or Trusted Signer) prompt for request `id` opened
    /// (spec 082 RA9). Accepted only while that request's pipeline is in its
    /// submit stage; any other id is stale and dropped.
    CeremonyStarted { id: String },
    /// That prompt returned a signature (RA9); same guard.
    CeremonyDone { id: String },
    /// A durable transport disconnected. Owner-aware clear
    /// (`dapp-connection.tsx:420-431`). Since spec 082 (RB2) it also stops a
    /// pipeline of that transport still before the passkey (pre-check,
    /// sponsoring, reactive sponsoring); past the commitment point nothing
    /// changes.
    TransportDropped { transport_id: String },
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made; a result carrying an older attempt belongs to a
    /// rejected pipeline and is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: SignShellResult,
    },
}

// ---------------------------------------------------------------------------
// Pure helpers — line-by-line ports
// ---------------------------------------------------------------------------

/// `isSigningMethod` (`use-dapp-signing.ts:490-496`): the one predicate
/// (`dapp_rpc::is_signing_method`, spec 070) plus `eth_sign`, which this
/// machine renders as the blind-signing rung when a transport forwards it —
/// the in-app browsers never do (`dapp_rpc::classify` refuses it, 4200).
pub fn is_signing_method(method: &str) -> bool {
    method == "eth_sign" || super::dapp_rpc::is_signing_method(method)
}

/// The signing-surface classification the sheet routes on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignMethodKind {
    Transaction,
    Batch,
    PersonalSign,
    EthSign,
    TypedData,
    Generic,
}

/// Classify a signing method for view routing.
pub fn method_kind(method: &str) -> SignMethodKind {
    if method == "eth_sendTransaction" {
        SignMethodKind::Transaction
    } else if method == "wallet_sendCalls" {
        SignMethodKind::Batch
    } else if method == "personal_sign" {
        SignMethodKind::PersonalSign
    } else if method == "eth_sign" {
        SignMethodKind::EthSign
    } else if crate::typed_data_request::TypedDataMethod::of(method).is_some() {
        SignMethodKind::TypedData
    } else {
        SignMethodKind::Generic
    }
}

/// `signAccountIndex` (`dapp-request-routing.ts:67-76`): the index of the
/// granted address, else the current active index — the fallback keeps the
/// real signer VISIBLE, never silent.
pub fn sign_account_index(
    accounts: &[SignAccountRef],
    active_index: u32,
    granted_address: Option<&str>,
) -> u32 {
    let Some(granted) = granted_address else {
        return active_index;
    };
    accounts
        .iter()
        .position(|a| a.address.eq_ignore_ascii_case(granted))
        .and_then(|i| u32::try_from(i).ok())
        .unwrap_or(active_index)
}

/// `'0x…'` hex or decimal chain string → number. `parseInt` semantics minus
/// the leading-digits leniency (a partial parse fails here — stricter, never
/// looser).
fn parse_chain_str(s: &str) -> Option<u32> {
    let t = s.trim();
    if let Some(hex) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
        u32::from_str_radix(hex, 16).ok()
    } else {
        t.parse::<u32>().ok()
    }
}

/// The `resolveChainId` coercion of one candidate (`use-dapp-signing.ts:90-99`):
/// string hex/dec or positive number.
fn chain_from_value(v: &Value) -> Option<u32> {
    match v {
        Value::Number(n) => {
            let f = n.as_f64()?;
            if f > 0.0 && f.fract() == 0.0 && f <= f64::from(u32::MAX) {
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                Some(f as u32)
            } else {
                None
            }
        }
        Value::String(s) => parse_chain_str(s).filter(|n| *n > 0),
        _ => None,
    }
}

/// The `chainId` a request writes, verbatim — `params[0].chainId` of a
/// transaction or a batch, the typed document's domain `chainId` — or `None`
/// when it writes none (absent, or `null`). Raw, so a caller can tell a
/// request that names no chain from one that names something no chain id
/// reads as.
fn embedded_chain_value(method: &str, params: &Value) -> Option<Value> {
    let arr = params.as_array()?;
    let value = if crate::typed_data_request::looks_like_typed_data(method) {
        // The one document the request is read as (audit 2026-10-01) — a
        // request that is not one carries no chain, and is refused at arrival.
        let read = crate::typed_data_request::canonical(method, params).ok()?;
        read.document.get("domain")?.get("chainId")?.clone()
    } else if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
        arr.first()?.get("chainId")?.clone()
    } else {
        return None;
    };
    (!value.is_null()).then_some(value)
}

/// `pickTypedDataParam` + `extractRequestChainId` (`use-dapp-signing.ts:124-156`):
/// the chain a request embeds, or `None`.
pub fn extract_request_chain_id(method: &str, params: &Value) -> Option<u32> {
    chain_from_value(&embedded_chain_value(method, params)?)
}

/// Whether a request names a chain other than `chain` (089): it writes a
/// `chainId` ([`extract_request_chain_id`]'s places) that is not `chain`, or
/// that does not read as a chain at all. A request that names none is on
/// `chain`. Such a request is refused [`CODE_INVALID_PARAMS`]
/// ([`CHAIN_MISMATCH_MESSAGE`]) — never signed or sent on `chain`, where
/// calldata and addresses prepared for another chain mean something else.
pub fn request_names_other_chain(method: &str, params: &Value, chain: u32) -> bool {
    embedded_chain_value(method, params)
        .is_some_and(|value| chain_from_value(&value) != Some(chain))
}

/// `assertNoRequiredCapabilities` (`use-dapp-signing.ts:66-84`): every
/// capability is REQUIRED unless explicitly `{ optional: true }`. Returns the
/// sorted, de-duplicated required names.
pub fn required_capabilities(payload: &Value) -> Vec<String> {
    let mut required = std::collections::BTreeSet::new();
    let mut scan = |caps: Option<&Value>| {
        let Some(obj) = caps.and_then(Value::as_object) else {
            return;
        };
        for (name, value) in obj {
            let optional = value
                .get("optional")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if !optional {
                required.insert(name.clone());
            }
        }
    };
    scan(payload.get("capabilities"));
    if let Some(calls) = payload.get("calls").and_then(Value::as_array) {
        for call in calls {
            scan(call.get("capabilities"));
        }
    }
    required.into_iter().collect()
}

// ---------------------------------------------------------------------------
// The ending of a request (spec 082 RA8) — one rule for the four sheets
// ---------------------------------------------------------------------------

/// What the answer to a request stands for, before the tracker is asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignEnding {
    /// A message or typed-data signature: signed, nothing goes on chain.
    Signed,
    /// The receipt arrived inside the wait: the page got the tx hash.
    /// `user_op_hash` is the op the tracker follows, when the shell knows it.
    Landed {
        tx_hash: String,
        user_op_hash: Option<String>,
    },
    /// Included and reverted (083): the page was answered the revert, which
    /// names the transaction ([`reverted_detail`]). Drawn as reverted at
    /// once — the shell holds the receipt the tracker will read too.
    Reverted {
        tx_hash: String,
        user_op_hash: Option<String>,
    },
    /// The wait ran out: the page was told the transaction is not confirmed
    /// yet ([`NOT_CONFIRMED_MESSAGE`], 083) — or, for a batch, got the op
    /// hash as its id — and the tracker keeps following it.
    StillConfirming { user_op_hash: String },
}

/// The ending of `payload` — the answer that went to the page — or `None`
/// when there is nothing to show (a refusal, or an empty answer).
/// `submitted_user_op` is the op this request handed the tracker: an answer
/// that IS that hash means the wait ran out before a receipt. Replaces the
/// desktop's, iOS's and Android's copies (RA8 1).
pub fn ending_of(
    method: &str,
    payload: &SignResponsePayload,
    submitted_user_op: Option<&str>,
) -> Option<SignEnding> {
    let on_chain = matches!(
        method_kind(method),
        SignMethodKind::Transaction | SignMethodKind::Batch
    );
    let op = submitted_user_op.filter(|op| !op.trim().is_empty());
    let result = match payload {
        // A batch answered in EIP-5792 2.0.0's `{ id }` names its id the same
        // (spec 097 G).
        SignResponsePayload::Ok { .. } => payload.answered()?,
        // 083's two errors for an operation that went out (owner ruling
        // 2026-10-01): a revert names its transaction; "not confirmed yet"
        // leaves the operation to the tracker. Every other error is no
        // ending — nothing went out, or the sheet shows the refusal.
        SignResponsePayload::Err {
            message: Some(message),
            ..
        } if on_chain => {
            if let Some(tx_hash) = reverted_transaction(message) {
                return Some(SignEnding::Reverted {
                    tx_hash: tx_hash.to_owned(),
                    user_op_hash: op.map(str::to_owned),
                });
            }
            if message.starts_with(NOT_CONFIRMED_MESSAGE) {
                return op.map(|op| SignEnding::StillConfirming {
                    user_op_hash: op.to_owned(),
                });
            }
            return None;
        }
        _ => return None,
    };
    if result.trim().is_empty() {
        return None;
    }
    if !on_chain {
        return Some(SignEnding::Signed);
    }
    Some(match op {
        Some(op) if op.eq_ignore_ascii_case(result) => SignEnding::StillConfirming {
            user_op_hash: op.to_owned(),
        },
        _ => SignEnding::Landed {
            tx_hash: result.to_owned(),
            user_op_hash: op.map(str::to_owned),
        },
    })
}

/// What the sheet draws for an ending, once the tracker has had its say.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignEndingState {
    /// Tick, "signed", closes by itself.
    Signed,
    /// Tick, the short hash, the explorer.
    Confirmed { tx_hash: String },
    /// Cross, `statusFailed` + `failedHint`, the explorer: it landed and
    /// reverted (including a Safe `ExecutionFailure`).
    Reverted { tx_hash: String },
    /// Cross, `statusFailed` + `txErrorGeneric`: the relay never had it
    /// (the tracker's `NotSent`) — sending it again is safe advice.
    NotSent,
    /// Cross, `statusFailed` + `componentsUi.signing.refused`, no Retry words
    /// (spec 082 RJ3): the relay refused it (the tracker's `Rejected`) —
    /// nothing was sent, and the same request would be refused again.
    Refused,
    /// Still on its way: the words come from `outcome` (`MaybeSent` →
    /// `maybeSent`, `Landing` → the ring, `StillConfirming`, `Unknown`);
    /// `fee_held` swaps in the fee-hold line; `relay_funding` the line that
    /// the relay is topping up its gas on this chain before it sends.
    Following {
        user_op_hash: String,
        outcome: TrackOutcome,
        fee_held: bool,
        #[serde(default)]
        relay_funding: bool,
    },
}

/// The ending as the tracker knows it (RA8 2): a landed or still-confirming
/// on-chain request is never drawn "confirmed" until the tracker says so —
/// the W3 fault, where a reverted op read as done. `track` is the tracker's
/// entry for the op (an entry for another op is ignored); `None` = the
/// tracker has not taken it yet, which is `Following(Landing)`.
pub fn ending_state(ending: &SignEnding, track: Option<&TrackEntryView>) -> SignEndingState {
    let (op, answered_tx) = match ending {
        SignEnding::Signed => return SignEndingState::Signed,
        SignEnding::Landed {
            tx_hash,
            user_op_hash,
        } => (user_op_hash.clone().unwrap_or_default(), Some(tx_hash)),
        SignEnding::Reverted { tx_hash, .. } => {
            return SignEndingState::Reverted {
                tx_hash: tx_hash.clone(),
            }
        }
        SignEnding::StillConfirming { user_op_hash } => (user_op_hash.clone(), None),
    };
    let landed = answered_tx.is_some();
    let entry =
        track.filter(|entry| !op.is_empty() && entry.user_op_hash.eq_ignore_ascii_case(&op));
    let Some(entry) = entry else {
        return SignEndingState::Following {
            user_op_hash: op,
            outcome: TrackOutcome::Landing,
            fee_held: false,
            relay_funding: false,
        };
    };
    let tx_hash = || {
        entry
            .tx_hash
            .clone()
            .or_else(|| answered_tx.cloned())
            .unwrap_or_default()
    };
    match entry.status {
        TrackStatus::Confirmed => SignEndingState::Confirmed { tx_hash: tx_hash() },
        TrackStatus::Dropped => SignEndingState::Reverted { tx_hash: tx_hash() },
        // A landed answer holds a tx hash (a receipt, or the tracker's own
        // verdict): "never sent" / "refused" against it can only be stale —
        // reached while the POST was still out — and would say "send it
        // again" over money that moved. It follows the tracker to its real
        // verdict (082 round-2 review, DX6's rule for the other two words).
        TrackStatus::NotSent | TrackStatus::Rejected if landed => SignEndingState::Following {
            user_op_hash: op,
            outcome: TrackOutcome::Landing,
            fee_held: false,
            relay_funding: false,
        },
        TrackStatus::NotSent => SignEndingState::NotSent,
        TrackStatus::Rejected => SignEndingState::Refused,
        TrackStatus::Pending
        | TrackStatus::FeeHeld
        | TrackStatus::RelayFunding
        | TrackStatus::Unreachable
        | TrackStatus::AcceptedNotLanded => SignEndingState::Following {
            user_op_hash: op,
            // DX6: the page has its tx hash while the tracker still has the
            // op in doubt (its relay was silent). A landed answer is never
            // "may have been sent" — it is landing.
            outcome: if landed && entry.outcome == TrackOutcome::MaybeSent {
                TrackOutcome::Landing
            } else {
                entry.outcome
            },
            fee_held: entry.status == TrackStatus::FeeHeld,
            relay_funding: entry.status == TrackStatus::RelayFunding,
        },
    }
}

/// `buildSigningRecord`'s shape split (`dapp-history.ts:162-174`).
fn record_shape(method: &str) -> (SignRecordKind, &'static str) {
    if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
        (SignRecordKind::DappTx, "tx")
    } else if method.contains("signTypedData") {
        (SignRecordKind::SignTypedData, "typed")
    } else {
        (SignRecordKind::SignMessage, "msg")
    }
}

/// `dapp-<ms>-<suffix>` — the TS id scheme (`dapp-history.ts:165-173`).
fn record_id_for(method: &str, now_ms: f64) -> String {
    let (_, suffix) = record_shape(method);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let ms = now_ms.max(0.0) as u64;
    format!("dapp-{ms}-{suffix}")
}

/// The intent a plain native send records when the sheet hands none (083 H2).
///
/// A transfer with no calldata has no decoded result, so no shell has an
/// intent to pass — yet its confirm already reads "Send" (`clear_signing`,
/// `ReqKind::TxPlain`). Recording that word is what lets Activity say
/// "Send 0.01 ETH" instead of calling somebody's payment a contract
/// interaction. `data` is the field the submit path reads, so "no calldata"
/// here is what actually goes on chain.
fn plain_send_intent(method: &str, params: &Value) -> Option<String> {
    if method != "eth_sendTransaction" {
        return None;
    }
    let data = params
        .get(0)?
        .get("data")
        .and_then(Value::as_str)
        .unwrap_or_default();
    (data.is_empty() || data == "0x").then(|| "Send".to_owned())
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// The request currently owning the sheet (`incomingRequest`).
#[derive(Clone, Debug)]
struct Pending {
    id: String,
    method: String,
    params_json: String,
    origin: String,
    transport_id: String,
    dedicated_transport: bool,
    per_request_chain: Option<u32>,
    dapp: Option<SignDappIdentity>,
    /// An error response was already sent for this id — a later approve must
    /// never produce a second response for the same id (invariant ① family).
    responded: bool,
    /// The failure answer this request is owed, held while its sheet shows
    /// the failure (spec 096 F8) — sent when the person closes the sheet, or
    /// dropped when they retry. The extension worker closes the request
    /// window the moment a request is answered, so an answer sent with the
    /// failure took the failure off the screen before anyone read it (the
    /// same reason a refused request waits, spec 081).
    held: Option<SignResponsePayload>,
}

/// Where the single-flight approve pipeline is.
#[derive(Clone, Debug)]
enum Stage {
    /// `checkBundlerFunding` racing its 15 s timeout.
    Precheck,
    /// Proactive silent sponsorship (pre-submit).
    Sponsoring { funding: SignFundingNeeded },
    /// Reactive sponsorship after an underfunded submit failure.
    ReactiveSponsoring {
        funding: SignFundingNeeded,
        message: String,
    },
    /// Passkey + submit in flight — the commitment window (BUG-2).
    Submitting,
    /// §4: persisting the record BEFORE the response goes out.
    PersistingResult { then: AfterRecord },
}

/// What a request's answer is, once its durable record is down (§4).
#[derive(Clone, Debug)]
enum AfterRecord {
    /// The result — the page's `Ok`, and the sheet closes.
    Result(String),
    /// A reverted operation: the error, the record closed failed, and the
    /// failure stays on the sheet.
    Reverted(String),
    /// Not confirmed by the cap: the error, and the sheet keeps following
    /// the operation, which may still land.
    NotConfirmed(String),
}

/// The one in-flight approve pipeline (`approveInFlightRef` as data). It
/// captures the request at approve time, so a newer request taking the sheet
/// never redirects a response (F2) — the pipeline finishes against its own
/// id, transport and chain.
#[derive(Clone, Debug)]
struct Inflight {
    id: String,
    transport_id: String,
    method: String,
    /// FINAL (capped) params — invariant ⑨.
    params_json: String,
    /// `SignApproveOpts::unlimited_approved`, captured at approve time.
    unlimited_approved: bool,
    chain_id: u32,
    address: String,
    credential_id: String,
    /// `requestDApp(...)?.name ?? request.origin` (`dapp-connection.tsx:729`).
    record_origin: String,
    /// `request.origin` alone — the record's `dapp_url`.
    record_url: String,
    intent: Option<String>,
    /// The approve's `balance_changes`, for a transaction only (083 F1).
    balance_changes: Option<Vec<TrustSimJudgment>>,
    /// What the request was, for Activity (spec 093), from the final params.
    summary: super::dapp_activity::DappSummary,
    max_fee_per_gas: Option<String>,
    gas_fee_token: Option<String>,
    quoted_fee: Option<SignQuotedFee>,
    stage: Stage,
    record_id: Option<String>,
    op_hash: Option<String>,
    /// Where the passkey prompt is, within `Stage::Submitting` (RA9).
    ceremony: Ceremony,
    /// The record written ahead of the POST (spec 082 RJ1), until the
    /// relay's verdict (`OpSubmitted`) or a proven "not sent" withdraws it.
    write_ahead: Option<WriteAhead>,
    /// `OpSubmitted` said the relay took the op (`maybe_sent: false`): the
    /// tracker's `NotSent` — a verdict on an op the relay never showed it
    /// holds — can only be stale for it (082 round-2 review).
    accepted: bool,
    /// 083: this request's own transport dropped while the pipeline ran — the
    /// page that asked is gone and already has its answer (4900) from the
    /// browser. A passkey that then comes back cancelled owes it nothing more;
    /// see `on_submit`.
    page_gone: bool,
    /// Spec 097 G: a `wallet_sendCalls` whose declared EIP-5792 version wants
    /// its id as `{ id }` ([`super::dapp_rpc::send_calls_answers_object`]) —
    /// read from the params the page SENT, whatever the sheet rewrote.
    batch_answers_object: bool,
}

/// The write-ahead record of one submit (RJ1).
#[derive(Clone, Debug)]
struct WriteAhead {
    user_op_hash: String,
    record_id: String,
    submit_block: Option<u64>,
    /// `ClearToPost` went out (the record's persist was acked).
    cleared: bool,
}

/// The passkey / Trusted Signer prompt of one submit (spec 082 RA9).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Ceremony {
    NotYet,
    Up,
    Done,
}

/// Funding view state (`fundingNeeded` + `fundingRidRef`).
#[derive(Clone, Debug)]
struct FundingState {
    data: SignFundingNeeded,
    presentation: SignFundingPresentation,
    denial_reason: Option<String>,
}

#[derive(Default)]
pub struct Model {
    supported_chains: Vec<u32>,
    /// `chainId` state; TS initialises to 1 — mirrored in `global_chain_id()`.
    global_chain: Option<u32>,
    accounts: Vec<SignAccountRef>,
    active_index: u32,
    /// §12.1.6 — false while a granted-account switch awaits its ack.
    reconciled: bool,
    pending: Option<Pending>,
    inflight: Option<Inflight>,
    funding: Option<FundingState>,
    funding_pinned_rid: Option<String>,
    last_opts: Option<SignApproveOpts>,
    sign_error: Option<SignErrorNotice>,
    /// The error above is a refusal by the relay (spec 082 RJ3). Set and
    /// cleared with `sign_error`.
    sign_error_refused: bool,
    pending_op_hash: Option<String>,
    /// The sheet's op was submitted "may have been sent" (spec 082 RA3).
    pending_op_maybe_sent: bool,
    tracker_handoff: Option<SignTrackerHandoff>,
    /// A write-ahead record proven never sent (RJ1), for the tracker.
    tracker_withdraw: Option<SignTrackerWithdraw>,
    notice: Option<SignNotice>,
    /// Set with `pending` when the self-call guard refuses a request.
    blocked: Option<SignBlockedView>,
    /// Same-session rid → outcome; a settled rid never signs twice (⑧).
    settled: Vec<(String, SignSettledOutcome)>,
    /// Bumped when a pre-submit pipeline is killed (reject / chain switch).
    attempt: u64,
    abort: Option<AbortHandle>,
}

impl Model {
    fn global_chain_id(&self) -> u32 {
        self.global_chain.unwrap_or(1)
    }

    fn chain_supported(&self, chain_id: u32) -> bool {
        self.supported_chains.contains(&chain_id)
    }

    fn inflight_matches_pending(&self) -> bool {
        match (&self.inflight, &self.pending) {
            (Some(fl), Some(p)) => fl.id == p.id,
            _ => false,
        }
    }

    /// Is the matching pipeline past the commitment point (BUG-2 window)?
    fn committed(&self) -> bool {
        self.pending_op_hash.is_some()
            || (self.inflight_matches_pending()
                && matches!(
                    self.inflight.as_ref().map(|f| &f.stage),
                    Some(Stage::Submitting | Stage::PersistingResult { .. })
                ))
    }

    /// `id` went to the relay in this session (⑧).
    fn settled_submitted(&self, id: &str) -> bool {
        self.settled
            .iter()
            .any(|(rid, outcome)| rid == id && *outcome == SignSettledOutcome::Submitted)
    }

    fn settle(&mut self, id: &str, outcome: SignSettledOutcome) {
        if let Some(slot) = self.settled.iter_mut().find(|(rid, _)| rid == id) {
            slot.1 = outcome;
            return;
        }
        if self.settled.len() >= MAX_SETTLED_RIDS {
            self.settled.remove(0);
        }
        self.settled.push((id.to_owned(), outcome));
    }

    /// [`SignView::failure_retryable`]: a held failure that sent nothing and
    /// was no refusal — by the relay (RJ3) or by a rule of this machine — of
    /// a request whose operation never reached the relay. One that did is
    /// settled `Submitted` (⑧: a settled rid never signs twice), so a
    /// verdict the tracker reached after the submit (spec 097 N4) is shown
    /// and answered, never signed again from the sheet.
    fn failure_retryable(&self) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|p| p.held.is_some() && !self.settled_submitted(&p.id))
            && !self.sign_error_refused
            && self
                .sign_error
                .as_ref()
                .is_some_and(|e| e.kind == SignErrorKind::SubmitFailed)
    }

    /// The held failure answer, sent now (spec 096 F8): the request counts
    /// as answered from here on.
    fn answer_held(&mut self) -> Option<SignOperation> {
        let pending = self.pending.as_mut()?;
        let payload = pending.held.take()?;
        pending.responded = true;
        Some(respond_op(&pending.transport_id, &pending.id, payload))
    }

    fn clear_sheet(&mut self) {
        self.pending = None;
        self.blocked = None;
        self.funding = None;
        self.funding_pinned_rid = None;
        self.sign_error = None;
        self.sign_error_refused = false;
        self.pending_op_hash = None;
        self.pending_op_maybe_sent = false;
    }
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignSurface {
    Hidden,
    /// The signing sheet (view routing within it belongs to
    /// `approval_guard.surface` + `clear_signing`).
    Sheet,
    /// The in-sheet funding swap (BUG-1: never a stacked second modal).
    Funding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignFundingPresentation {
    Topup,
    Confirming,
}

/// What a swipe-dismiss means right now (`SigningRequestModal` onClose).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignSwipeAction {
    None,
    Reject,
    Dismiss,
    FundingCancel,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignErrorNotice {
    pub kind: SignErrorKind,
    pub detail: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignFundingView {
    pub data: SignFundingNeeded,
    pub presentation: SignFundingPresentation,
    pub denial_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignRequestView {
    pub id: String,
    pub method: String,
    pub kind: SignMethodKind,
    pub params_json: String,
    pub origin: String,
    pub dapp: Option<SignDappIdentity>,
    /// The request's OWN chain (F4): `__chainId` when stamped, else the
    /// global chain — live, like `reqChainId(incomingRequest, chainId)`.
    pub chain_id: u32,
    pub signer_address: Option<String>,
}

/// Why a request is refused outright, for the sheet to explain (spec 081).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignBlockedView {
    /// Stable wire name of the Safe function, e.g. `enableModule`.
    pub function: String,
    pub selector: String,
    /// 1-based position in a batch, when that is where it was found.
    pub leg_index: Option<u32>,
    /// Found inside a `multiSend` or `execTransaction` payload.
    pub nested: bool,
}

/// What the signing sheet is doing, in the words a person reads (spec 082
/// RA9, G22): the network work before the passkey is "preparing", never
/// "waiting for your signature".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignPhase {
    /// No pipeline for the request on the sheet: the form.
    #[default]
    Idle,
    /// Pre-check, sponsoring, or the submit stage before its prompt opened
    /// (`send.txPreparing`).
    Preparing,
    /// The passkey / Trusted Signer prompt is up (`send.txSigning`, or
    /// `componentsUi.signing.signing` for a message).
    AwaitingSignature,
    /// Signed; going to the relay, or recovering after it
    /// (`send.txSubmitting` + the background hint).
    Submitting,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignView {
    pub surface: SignSurface,
    pub request: Option<SignRequestView>,
    /// Kept until every shell reads `phase` (spec 082 RA9).
    pub is_signing: bool,
    /// Kept until every shell reads `phase` (spec 082 RA9).
    pub is_submitting: bool,
    /// The sheet's words, derived from the pipeline stage and the prompt.
    #[serde(default)]
    pub phase: SignPhase,
    pub pending_op_hash: Option<String>,
    /// The sheet's op may have been sent — its reply was lost (spec 082
    /// RA3): the caption says so, and there is no Retry.
    #[serde(default)]
    pub pending_op_maybe_sent: bool,
    pub error: Option<SignErrorNotice>,
    pub funding: Option<SignFundingView>,
    /// This machine's own approval gate: a reviewable request with the
    /// granted account reconciled and no pipeline in flight. The shell must
    /// AND it with `GuardView.confirm_allowed` and `FeeView.confirm_fee_ready`.
    pub confirm_gate_open: bool,
    /// §12.1.6: the granted-account switch has not acked yet.
    pub reconcile_pending: bool,
    pub swipe_action: SignSwipeAction,
    pub tracker_handoff: Option<SignTrackerHandoff>,
    /// A write-ahead record proven never sent (spec 082 RJ1): the shell feeds
    /// it to `tx_tracker::Event::Withdrawn` the moment it appears.
    #[serde(default)]
    pub tracker_withdraw: Option<SignTrackerWithdraw>,
    /// `error` is the relay refusing the op (spec 082 RJ3): the sheet says
    /// `componentsUi.signing.refused` under `statusFailed`, never "try again".
    #[serde(default)]
    pub failure_refused: bool,
    /// The failure on the sheet sent nothing, was not a refusal, and its
    /// answer is still held (spec 096 F8): the sheet offers "Try again"
    /// (`send.txRetryBtn` → [`Event::RetryTapped`]) beside its close, which
    /// answers the page the failure. Never for a request whose operation
    /// reached the relay — a verdict after the submit (spec 097 N4) has only
    /// the close.
    #[serde(default)]
    pub failure_retryable: bool,
    pub notice: Option<SignNotice>,
    pub global_chain_id: u32,
    /// Present when the request was refused because it would have changed who
    /// controls the account; the sheet shows it and offers only Dismiss.
    pub blocked: Option<SignBlockedView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct SignRequest;

impl App for SignRequest {
    type Event = Event;
    type Model = Model;
    type ViewModel = SignView;
    type Effect = SignEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<SignEffect, Event> {
        match event {
            Event::NetworksChanged { chain_ids } => {
                model.supported_chains = chain_ids;
                render()
            }
            Event::AccountsChanged {
                accounts,
                active_index,
            } => {
                model.accounts = accounts;
                model.active_index = active_index;
                render()
            }
            Event::RequestArrived {
                id,
                method,
                params_json,
                origin,
                transport_id,
                dedicated_transport,
                per_request_chain,
                dapp,
                granted_address,
                requested_address,
                request_ts_ms,
                now_ms,
            } => on_request_arrived(
                model,
                Arrival {
                    id,
                    method,
                    params_json,
                    origin,
                    transport_id,
                    dedicated_transport,
                    per_request_chain,
                    dapp,
                    granted_address,
                    requested_address,
                    request_ts_ms,
                    now_ms,
                },
            ),
            Event::ChainSwitchRequested {
                id,
                transport_id,
                chain_id_param,
            } => on_chain_switch(model, id, transport_id, chain_id_param),
            Event::ApproveTapped { opts } => {
                model.last_opts = Some(opts.clone());
                approve_with(model, opts, false)
            }
            Event::RejectTapped => reject(model),
            Event::DismissTapped => dismiss(model),
            Event::RetryTapped => retry(model),
            Event::SwipeDismissed => match swipe_action(model) {
                SignSwipeAction::Reject => reject(model),
                SignSwipeAction::Dismiss => dismiss(model),
                SignSwipeAction::FundingCancel => funding_cancel(model),
                SignSwipeAction::None => Command::done(),
            },
            Event::FundingCompleteTapped => funding_complete(model),
            Event::FundingCancelled => funding_cancel(model),
            Event::OpSubmitted {
                id,
                user_op_hash,
                now_ms,
                maybe_sent,
                submit_block,
            } => on_op_submitted(model, &id, user_op_hash, now_ms, (maybe_sent, submit_block)),
            Event::OpSigned {
                id,
                user_op_hash,
                submit_block,
                now_ms,
            } => on_op_signed(model, &id, user_op_hash, submit_block, now_ms),
            Event::OpTracked {
                user_op_hash,
                status,
                tx_hash,
                now_ms: _,
            } => on_op_tracked(model, &user_op_hash, status, tx_hash),
            Event::CeremonyStarted { id } => on_ceremony(model, &id, Ceremony::Up),
            Event::CeremonyDone { id } => on_ceremony(model, &id, Ceremony::Done),
            Event::TransportDropped { transport_id } => {
                let mut cleared_inflight_request = false;
                if let Some(p) = &model.pending {
                    // Owner-aware (`dapp-connection.tsx:429`): keep the request
                    // only when it is stamped onto a DIFFERENT transport than
                    // the one that dropped.
                    let keep = p.dedicated_transport && p.transport_id != transport_id;
                    if !keep {
                        cleared_inflight_request = model.inflight_matches_pending();
                        model.clear_sheet();
                    }
                }
                // 083: a pipeline whose page went away is not refused later
                // — the page already has its 4900 from the browser, and a
                // second answer is one too many.
                if let Some(fl) = model.inflight.as_mut() {
                    if fl.transport_id == transport_id {
                        fl.page_gone = true;
                    }
                }
                // RB2: nobody is left to answer, so a pipeline of that
                // transport (or the one whose sheet just went) that has not
                // reached the passkey stops here — its late pre-check or
                // sponsorship answer can no longer lead to a signature.
                let asker_gone = model
                    .inflight
                    .as_ref()
                    .is_some_and(|fl| fl.transport_id == transport_id || cleared_inflight_request);
                if asker_gone {
                    stop_presubmit_inflight(model);
                }
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                // The account switch belongs to the request that asked for
                // it, never to a pipeline: a pipeline killed or answered early
                // (the attempt moving on) must not strand a newer request's
                // switch ack, or that request stays `reconcile_pending` and
                // can never be approved (082 round-2 review).
                if attempt != model.attempt && !matches!(result, SignShellResult::AccountSwitched) {
                    // A result from a rejected pipeline (BUG-2): the 4001 is
                    // out, nothing may still submit or answer this id.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> SignView {
        let matching = model.inflight_matches_pending();
        let stage = model.inflight.as_ref().map(|f| &f.stage);
        let is_signing = matching
            && matches!(
                stage,
                Some(
                    Stage::Precheck
                        | Stage::Sponsoring { .. }
                        | Stage::Submitting
                        | Stage::PersistingResult { .. }
                )
            );
        // TS: isSubmitting flips at the commitment point and stays through the
        // reactive underfunded recovery (`finally` clears it).
        let is_submitting = matching
            && matches!(
                stage,
                Some(
                    Stage::Submitting
                        | Stage::PersistingResult { .. }
                        | Stage::ReactiveSponsoring { .. }
                )
            );

        let phase = if !matching {
            SignPhase::Idle
        } else {
            match model.inflight.as_ref().map(|f| (&f.stage, f.ceremony)) {
                None => SignPhase::Idle,
                Some((Stage::Precheck | Stage::Sponsoring { .. }, _)) => SignPhase::Preparing,
                Some((Stage::Submitting, Ceremony::NotYet)) => SignPhase::Preparing,
                Some((Stage::Submitting, Ceremony::Up)) => SignPhase::AwaitingSignature,
                Some((
                    Stage::Submitting
                    | Stage::ReactiveSponsoring { .. }
                    | Stage::PersistingResult { .. },
                    _,
                )) => SignPhase::Submitting,
            }
        };

        let request = model.pending.as_ref().map(|p| SignRequestView {
            id: p.id.clone(),
            method: p.method.clone(),
            kind: method_kind(&p.method),
            params_json: p.params_json.clone(),
            origin: p.origin.clone(),
            dapp: p.dapp.clone(),
            chain_id: p
                .per_request_chain
                .unwrap_or_else(|| model.global_chain_id()),
            signer_address: model
                .accounts
                .get(model.active_index as usize)
                .map(|a| a.address.clone()),
        });

        let surface = if model.pending.is_none() {
            SignSurface::Hidden
        } else if model.funding.is_some() {
            SignSurface::Funding
        } else {
            SignSurface::Sheet
        };

        SignView {
            surface,
            confirm_gate_open: model.pending.is_some()
                // Spec 081: a refused request is never signable, however the
                // shell asks.
                && model.blocked.is_none()
                && model.inflight.is_none()
                && model.funding.is_none()
                && model.reconciled
                && !model.pending.as_ref().is_some_and(|p| p.responded || p.held.is_some()),
            request,
            is_signing,
            is_submitting,
            phase,
            pending_op_hash: model.pending_op_hash.clone(),
            pending_op_maybe_sent: model.pending_op_maybe_sent,
            error: model.sign_error.clone(),
            funding: model.funding.as_ref().map(|f| SignFundingView {
                data: f.data.clone(),
                presentation: f.presentation,
                denial_reason: f.denial_reason.clone(),
            }),
            reconcile_pending: !model.reconciled,
            swipe_action: swipe_action(model),
            tracker_handoff: model.tracker_handoff.clone(),
            tracker_withdraw: model.tracker_withdraw.clone(),
            failure_refused: model.sign_error.is_some() && model.sign_error_refused,
            failure_retryable: model.failure_retryable(),
            notice: model.notice,
            global_chain_id: model.global_chain_id(),
            blocked: model.blocked.clone(),
        }
    }
}

/// The swipe dispatch (`SigningRequestModal` onClose), phase-derived.
fn swipe_action(model: &Model) -> SignSwipeAction {
    if model.pending.is_none() {
        return SignSwipeAction::None;
    }
    if model.funding.is_some() {
        return SignSwipeAction::FundingCancel;
    }
    // Spec 081: a REFUSED request must still be answered, and dismissing the
    // sheet is how a person closes it. `sign_error` is set on a refusal too,
    // so without this the check below turned the dismissal into `Dismiss` —
    // which sends nothing. The dApp then waited forever and every later
    // request got "another request is open", app-wide, until the process was
    // killed. Found on a device (FR-019); the shells send `SwipeDismissed`,
    // not `RejectTapped`, which is why the unit test missed it.
    if model.blocked.is_some() {
        return SignSwipeAction::Reject;
    }
    if model.sign_error.is_some() || model.pending_op_hash.is_some() || {
        model.inflight_matches_pending()
            && matches!(
                model.inflight.as_ref().map(|f| &f.stage),
                Some(
                    Stage::Submitting
                        | Stage::PersistingResult { .. }
                        | Stage::ReactiveSponsoring { .. }
                )
            )
    } {
        return SignSwipeAction::Dismiss;
    }
    SignSwipeAction::Reject
}

// ---------------------------------------------------------------------------
// Command plumbing
// ---------------------------------------------------------------------------

fn respond_op(transport_id: &str, id: &str, payload: SignResponsePayload) -> SignOperation {
    SignOperation::SendResponse {
        transport_id: transport_id.to_owned(),
        id: id.to_owned(),
        payload,
    }
}

fn blocked_view(block: &SelfCallBlock) -> SignBlockedView {
    SignBlockedView {
        function: block.function.as_str().to_owned(),
        selector: block.selector.clone(),
        leg_index: block.leg_index,
        nested: block.nested,
    }
}

fn err_payload(code: i32, kind: SignErrorKind, message: Option<String>) -> SignResponsePayload {
    SignResponsePayload::Err {
        code,
        kind,
        message,
    }
}

/// Issue one operation, correlating its answer to the current pipeline.
fn request_op(
    model: &mut Model,
    operation: SignOperation,
    abortable: bool,
) -> Command<SignEffect, Event> {
    let attempt = model.attempt;
    let command = Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result });
    if abortable {
        model.abort = Some(command.abort_handle());
    }
    command
}

fn ops_and_render(model: &mut Model, operations: Vec<SignOperation>) -> Command<SignEffect, Event> {
    let mut commands: Vec<Command<SignEffect, Event>> = operations
        .into_iter()
        .map(|op| request_op(model, op, false))
        .collect();
    commands.push(render());
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// Arrival
// ---------------------------------------------------------------------------

struct Arrival {
    id: String,
    method: String,
    params_json: String,
    origin: String,
    transport_id: String,
    dedicated_transport: bool,
    per_request_chain: Option<u32>,
    dapp: Option<SignDappIdentity>,
    granted_address: Option<String>,
    requested_address: Option<String>,
    request_ts_ms: Option<f64>,
    now_ms: f64,
}

fn on_request_arrived(model: &mut Model, arrival: Arrival) -> Command<SignEffect, Event> {
    model.notice = None;
    // A fresh signing request supersedes any funding prompt left over from a
    // prior request (invariant ③, `dapp-connection.tsx:320-322`).
    model.funding = None;
    model.funding_pinned_rid = None;

    if !is_signing_method(&arrival.method) {
        // Read-only routing and `eth_requestAccounts` stay in the shell.
        return render();
    }

    // ⑧ same-session one-shot: a settled rid never re-signs — replay only.
    if let Some((_, outcome)) = model.settled.iter().find(|(rid, _)| rid == &arrival.id) {
        model.notice = Some(SignNotice::AlreadySettled { outcome: *outcome });
        return render();
    }

    // ⑧ payload TTL: a stale request is never signed and never answered — the
    // page recovers via the 4900 path (`extension-bridge-transport.ts:136-141`).
    if let Some(ts) = arrival.request_ts_ms {
        if arrival.now_ms - ts > EXTENSION_REQUEST_TTL_MS {
            model.notice = Some(SignNotice::Expired);
            return render();
        }
    }

    // What you see is what you sign (audit 2026-10-01): a typed-data request
    // is read ONCE, strictly — the method's own two params, one document — and
    // refused -32602 before any sheet when it is not. What passes goes on
    // rebuilt around that one document, so the preview, both guards, the
    // passkey and the Trusted Signer page all hold the same bytes; the account
    // it names must be the granted one (4100 below).
    let mut arrival = arrival;
    if crate::typed_data_request::looks_like_typed_data(&arrival.method) {
        match crate::typed_data_request::signable_json(&arrival.method, &arrival.params_json) {
            Err(error) => {
                let op = respond_op(
                    &arrival.transport_id,
                    &arrival.id,
                    err_payload(
                        CODE_INVALID_PARAMS,
                        SignErrorKind::InvalidParams,
                        Some(error.message()),
                    ),
                );
                return ops_and_render(model, vec![op]);
            }
            Ok(read) => {
                if let Some(granted) = &arrival.granted_address {
                    if !read.account.eq_ignore_ascii_case(granted) {
                        let op = respond_op(
                            &arrival.transport_id,
                            &arrival.id,
                            err_payload(
                                CODE_UNAUTHORIZED,
                                SignErrorKind::UnauthorizedAccount,
                                None,
                            ),
                        );
                        return ops_and_render(model, vec![op]);
                    }
                }
                arrival.params_json = read.params_json();
                if arrival.requested_address.is_none() {
                    arrival.requested_address = Some(read.account);
                }
            }
        }
    }

    // The same for a transaction's value (spec 096 F1): read ONCE by the
    // card's rule, refused -32602 before any sheet when it breaks it, and
    // carried on in its canonical text — so the card, the fee quote, both
    // guards and every shell's submit hold one number, never one each.
    match crate::tx_request::canonical_params_json(&arrival.method, &arrival.params_json) {
        Err(error) => {
            let op = respond_op(
                &arrival.transport_id,
                &arrival.id,
                err_payload(
                    CODE_INVALID_PARAMS,
                    SignErrorKind::InvalidParams,
                    Some(error.message()),
                ),
            );
            return ops_and_render(model, vec![op]);
        }
        Ok(Some(canonical)) => arrival.params_json = canonical,
        Ok(None) => {}
    }

    // §12.1.6: the requested account must BE the granted one — 4100, never a
    // silent signer swap (`web-request.tsx:190-193`).
    if let (Some(req_addr), Some(granted)) = (&arrival.requested_address, &arrival.granted_address)
    {
        if !req_addr.eq_ignore_ascii_case(granted) {
            let op = respond_op(
                &arrival.transport_id,
                &arrival.id,
                err_payload(CODE_UNAUTHORIZED, SignErrorKind::UnauthorizedAccount, None),
            );
            return ops_and_render(model, vec![op]);
        }
    }

    // ⑥ chain routing: an unsupported chain is refused 4902 BEFORE any UI.
    if let Some(cid) = arrival.per_request_chain {
        // Per-request chain (F4) — never touches the global chain.
        if !model.chain_supported(cid) {
            let op = respond_op(
                &arrival.transport_id,
                &arrival.id,
                err_payload(
                    CODE_UNSUPPORTED_CHAIN,
                    SignErrorKind::UnsupportedChain,
                    None,
                ),
            );
            return ops_and_render(model, vec![op]);
        }
        // 089: the stamped chain is the SITE's chain in the wallet; a request
        // that names another one was prepared for that other chain. It is
        // refused before any sheet — never shown and sent on this one (the
        // in-app path below may switch, because there the wallet's chain IS
        // the site's; a stamp cannot be switched from here). Mirrors
        // `dapp_session::assert_request_chain_context` and MetaMask.
        let names_other = serde_json::from_str::<Value>(&arrival.params_json)
            .is_ok_and(|params| request_names_other_chain(&arrival.method, &params, cid));
        if names_other {
            let op = respond_op(
                &arrival.transport_id,
                &arrival.id,
                err_payload(
                    CODE_INVALID_PARAMS,
                    SignErrorKind::InvalidParams,
                    Some(CHAIN_MISMATCH_MESSAGE.to_owned()),
                ),
            );
            return ops_and_render(model, vec![op]);
        }
    } else {
        // Ordinary request — auto-switch the global chain to an embedded
        // request chainId (`dapp-connection.tsx:337-349`). A malformed params
        // array simply carries no hint (the TS try/catch).
        let parsed: Option<Value> = serde_json::from_str(&arrival.params_json).ok();
        if let Some(embedded) = parsed
            .as_ref()
            .and_then(|p| extract_request_chain_id(&arrival.method, p))
        {
            if embedded != model.global_chain_id() {
                if !model.chain_supported(embedded) {
                    let op = respond_op(
                        &arrival.transport_id,
                        &arrival.id,
                        err_payload(
                            CODE_UNSUPPORTED_CHAIN,
                            SignErrorKind::UnsupportedChain,
                            None,
                        ),
                    );
                    return ops_and_render(model, vec![op]);
                }
                model.global_chain = Some(embedded);
            }
        }
    }

    // §12.1.6 reconcile: switch to the granted account FIRST; the approval
    // surface opens only on the `AccountSwitched` ack (explicit sequencing —
    // the `setTimeout(0)` of `web-request.tsx:207` made a rule).
    let mut commands: Vec<Command<SignEffect, Event>> = Vec::new();
    // A failure still on screen for the request this one replaces is answered
    // now (spec 096 F8) — its page must not wait on a sheet that is gone —
    // and its words go with it.
    if let Some(op) = model.answer_held() {
        model.sign_error = None;
        model.sign_error_refused = false;
        commands.push(request_op(model, op, false));
    }
    model.reconciled = true;
    if let Some(granted) = &arrival.granted_address {
        let idx = sign_account_index(&model.accounts, model.active_index, Some(granted));
        if idx != model.active_index {
            model.active_index = idx;
            model.reconciled = false;
            commands.push(request_op(
                model,
                SignOperation::SwitchActiveAccount { index: idx },
                false,
            ));
        }
    }

    // Spec 081 FR-005: a dApp may not ask this account to rewrite who controls
    // it. Decided here, before the request is reviewable, so the sheet opens
    // already refused — and answered, so the page is never left hanging.
    let signer = model
        .accounts
        .get(sign_account_index(
            &model.accounts,
            model.active_index,
            arrival.granted_address.as_deref(),
        ) as usize)
        .map(|a| a.address.clone())
        .unwrap_or_default();
    let refusal = serde_json::from_str::<Value>(&arrival.params_json)
        .ok()
        .and_then(|params| detect_self_call(&arrival.method, Some(&params), &signer));
    if let Some(block) = refusal {
        // The sheet opens refused, and the answer waits for the dismissal.
        //
        // Answering here instead would be tidier for the page — but the window
        // that shows this sheet IS the answer surface: the extension worker
        // closes it the moment the request settles (`background.js`, "a window
        // closed without a decision answers 4001"). An immediate answer
        // therefore took the explanation off the screen before anyone could
        // read it, which is the whole point of refusing visibly. So the
        // request stays pending, unsignable, until the person closes it —
        // exactly like every other request that waits for a decision.
        model.blocked = Some(blocked_view(&block));
        model.sign_error_refused = false;
        model.sign_error = Some(SignErrorNotice {
            kind: SignErrorKind::SelfCallBlocked,
            detail: Some(block.function.as_str().to_owned()),
        });
        model.pending = Some(Pending {
            id: arrival.id,
            method: arrival.method,
            params_json: arrival.params_json,
            origin: arrival.origin,
            transport_id: arrival.transport_id,
            dedicated_transport: arrival.dedicated_transport,
            per_request_chain: arrival.per_request_chain,
            dapp: arrival.dapp,
            responded: false,
            held: None,
        });
        commands.push(render());
        return Command::all(commands);
    }
    model.blocked = None;

    // Ported quirk: a lingering `signError` from a previous request is NOT
    // cleared by an arrival (`handleIncoming` never touches it) — the sheet
    // may briefly show the old error over the new request.
    model.pending = Some(Pending {
        id: arrival.id,
        method: arrival.method,
        params_json: arrival.params_json,
        origin: arrival.origin,
        transport_id: arrival.transport_id,
        dedicated_transport: arrival.dedicated_transport,
        per_request_chain: arrival.per_request_chain,
        dapp: arrival.dapp,
        responded: false,
        held: None,
    });
    commands.push(render());
    Command::all(commands)
}

// ---------------------------------------------------------------------------
// Chain switch
// ---------------------------------------------------------------------------

fn on_chain_switch(
    model: &mut Model,
    id: Option<String>,
    transport_id: Option<String>,
    chain_id_param: Option<String>,
) -> Command<SignEffect, Event> {
    let responder = match (&id, &transport_id) {
        (Some(id), Some(tid)) => Some((tid.clone(), id.clone())),
        _ => None,
    };

    let Some(new_chain) = chain_id_param.as_deref().and_then(parse_chain_str) else {
        // Missing/malformed chainId — never a phantom success
        // (`dapp-connection.tsx:359-363`).
        let ops = responder
            .map(|(tid, rid)| {
                vec![respond_op(
                    &tid,
                    &rid,
                    err_payload(CODE_INVALID_PARAMS, SignErrorKind::InvalidParams, None),
                )]
            })
            .unwrap_or_default();
        return ops_and_render(model, ops);
    };

    if !model.chain_supported(new_chain) {
        let ops = responder
            .map(|(tid, rid)| {
                vec![respond_op(
                    &tid,
                    &rid,
                    err_payload(
                        CODE_UNSUPPORTED_CHAIN,
                        SignErrorKind::UnsupportedChain,
                        None,
                    ),
                )]
            })
            .unwrap_or_default();
        return ops_and_render(model, ops);
    }

    let mut ops: Vec<SignOperation> = Vec::new();

    // ⑥: a pending sign bound to the previous GLOBAL chain must be cancelled;
    // per-request signs carry their own chain and are left intact
    // (`dapp-connection.tsx:373-383`). Fail-closed divergence from TS: once
    // the pipeline is past the commitment point the request is NOT cancelled
    // (the TS would 4001 and then still broadcast + double-respond — the
    // exact BUG-2 shape invariant ① forbids).
    let cancel = model
        .pending
        .as_ref()
        .is_some_and(|p| p.per_request_chain.is_none() && !p.responded)
        && !model.committed();
    if cancel {
        // A failure still on screen is owed its own answer (spec 096 F8, 097
        // N4) — the request failed, the person never cancelled it — so the
        // switch sends that one, never a 4001 in its place.
        if let Some(held) = model.answer_held() {
            ops.push(held);
        } else if let Some(p) = &model.pending {
            ops.push(respond_op(
                &p.transport_id,
                &p.id,
                err_payload(
                    CODE_USER_REJECTED,
                    SignErrorKind::WalletSwitchedChains,
                    None,
                ),
            ));
        }
        kill_presubmit_pipeline(model);
        model.clear_sheet();
    }

    model.global_chain = Some(new_chain);
    if let Some((tid, rid)) = responder {
        ops.push(respond_op(
            &tid,
            &rid,
            SignResponsePayload::Ok { result: None },
        ));
    }
    ops_and_render(model, ops)
}

/// Stop the in-flight pipeline if it has not reached the passkey (spec 082
/// RB2) — whatever request the sheet shows. Same mechanics as a reject:
/// cleared, attempt bumped (late answers dropped), effect aborted.
fn stop_presubmit_inflight(model: &mut Model) {
    let before_passkey = matches!(
        model.inflight.as_ref().map(|f| &f.stage),
        Some(Stage::Precheck | Stage::Sponsoring { .. } | Stage::ReactiveSponsoring { .. })
    );
    if !before_passkey {
        return;
    }
    model.inflight = None;
    model.attempt += 1;
    if let Some(handle) = model.abort.take() {
        handle.abort();
    }
}

/// Kill a pre-submit pipeline: late results are identified by the stale
/// attempt, and the outstanding operation is truly aborted.
fn kill_presubmit_pipeline(model: &mut Model) {
    if model.inflight_matches_pending()
        && matches!(
            model.inflight.as_ref().map(|f| &f.stage),
            Some(Stage::Precheck | Stage::Sponsoring { .. } | Stage::ReactiveSponsoring { .. })
        )
    {
        model.inflight = None;
        model.attempt += 1;
        if let Some(handle) = model.abort.take() {
            handle.abort();
        }
    }
}

// ---------------------------------------------------------------------------
// Approve pipeline
// ---------------------------------------------------------------------------

fn approve_with(
    model: &mut Model,
    opts: SignApproveOpts,
    bust_cache: bool,
) -> Command<SignEffect, Event> {
    // BUG-3: the pipeline is single-flight — a same-tick second tap finds it
    // occupied (`approveInFlightRef`, `dapp-connection.tsx:632-633`).
    if model.inflight.is_some() {
        return Command::done();
    }
    let Some(pending) = model.pending.clone() else {
        return Command::done();
    };
    // A response already went out for this id — never a second one (①) —
    // or one is held for a failure still on screen (spec 096 F8: "Try again"
    // clears it first).
    if pending.responded || pending.held.is_some() {
        return Command::done();
    }
    if model.funding.is_some() {
        return Command::done();
    }
    // §12.1.6: the approval surface may not act before the granted-account
    // switch acked.
    if !model.reconciled {
        return Command::done();
    }
    let Some(signer) = model.accounts.get(model.active_index as usize).cloned() else {
        return Command::done();
    };

    // Immediate feedback + fresh error state (`dapp-connection.tsx:656-658`).
    model.sign_error = None;
    model.sign_error_refused = false;
    model.pending_op_hash = None;
    model.pending_op_maybe_sent = false;

    // ⑨: sign/submit/record the CAPPED params when the sheet provided them.
    let final_params = opts
        .params_override_json
        .clone()
        .unwrap_or_else(|| pending.params_json.clone());
    let Ok(parsed) = serde_json::from_str::<Value>(&final_params) else {
        // Fail-closed: params this machine cannot even parse are never signed.
        return fail_pending(
            model,
            CODE_INTERNAL,
            SignErrorKind::InvalidParams,
            Some("malformed params".to_owned()),
        );
    };
    let chain_id = pending
        .per_request_chain
        .unwrap_or_else(|| model.global_chain_id());

    // 089, at the chokepoint: what is signed names no chain but the one it is
    // signed on — whatever the sheet rewrote the params to, on either path.
    if request_names_other_chain(&pending.method, &parsed, chain_id) {
        return fail_pending(
            model,
            CODE_INVALID_PARAMS,
            SignErrorKind::InvalidParams,
            Some(CHAIN_MISMATCH_MESSAGE.to_owned()),
        );
    }

    if pending.method == "wallet_sendCalls" {
        let payload = parsed.get(0).cloned().unwrap_or(Value::Null);
        // ⑩: reject unsupported REQUIRED capabilities (5700) before touching
        // the wallet — checked ahead of even the funding pre-check (the
        // invariant's ordering; the TS checked inside `handleSendCalls`).
        let required = required_capabilities(&payload);
        if !required.is_empty() {
            return fail_pending(
                model,
                CODE_UNSUPPORTED_CAPABILITY,
                SignErrorKind::UnsupportedCapability,
                Some(required.join(", ")),
            );
        }
        let empty = payload
            .get("calls")
            .and_then(Value::as_array)
            .is_none_or(Vec::is_empty);
        if empty {
            // 'No calls provided' → the generic -32603 catch.
            return fail_pending(
                model,
                CODE_INTERNAL,
                SignErrorKind::InvalidParams,
                Some("no calls provided".to_owned()),
            );
        }
    }

    // Tempo displayed-fee staleness (fee_policy kernel, submit-side guard):
    // a stale quote is re-reviewed, never silently re-priced. No response is
    // sent — the request stays reviewable with a fresh quote.
    if is_tempo_chain(chain_id) {
        if let Some(qf) = &opts.quoted_fee {
            let stale = match qf.amount.trim().parse::<u128>() {
                Ok(amount) => {
                    let collector = opts.fee_collector.as_deref().unwrap_or(&qf.recipient);
                    tempo_quote_is_stale(amount, &qf.recipient, collector, TEMPO_FEE_TOKEN_DECIMALS)
                }
                Err(_) => true, // unparseable displayed amount — fail closed
            };
            if stale {
                model.sign_error_refused = false;
                model.sign_error = Some(SignErrorNotice {
                    kind: SignErrorKind::StaleFeeQuote,
                    detail: None,
                });
                return render();
            }
        }
    }

    let record_origin = pending
        .dapp
        .as_ref()
        .map(|d| d.name.clone())
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| pending.origin.clone());
    let intent = opts
        .intent
        .clone()
        .filter(|intent| !intent.trim().is_empty())
        .or_else(|| plain_send_intent(&pending.method, &parsed));
    // What the sheet's simulation showed, from the approve alone (083 F1). A
    // signature moves nothing, so it keeps none; an empty list is nothing to
    // keep.
    let balance_changes = opts
        .balance_changes
        .clone()
        .filter(|changes| !changes.is_empty())
        .filter(|_| {
            matches!(
                method_kind(&pending.method),
                SignMethodKind::Transaction | SignMethodKind::Batch
            )
        });

    // What Activity will say the request was (spec 093): read here, once,
    // from the WHOLE final params — before any shell clips what it stores —
    // against the origin the row will name.
    let summary = super::dapp_activity::summarize(
        &pending.method,
        &parsed,
        chain_id,
        &pending.origin,
        opts.token_meta.as_ref(),
    );
    // Spec 097: what the sheet called the contract, and the fee the wallet
    // adds in a token — the one `Transfer` of the receipt that is not the
    // dApp's.
    let summary = super::dapp_activity::with_approve_facts(
        summary,
        opts.reading.as_ref(),
        opts.gas_fee_token.as_deref(),
        opts.quoted_fee.as_ref().map(|fee| fee.amount.as_str()),
    );

    model.inflight = Some(Inflight {
        id: pending.id.clone(),
        transport_id: pending.transport_id.clone(),
        method: pending.method.clone(),
        params_json: final_params,
        unlimited_approved: opts.unlimited_approved,
        chain_id,
        address: signer.address.clone(),
        credential_id: signer.credential_id,
        record_origin,
        record_url: pending.origin.clone(),
        intent,
        balance_changes,
        summary,
        max_fee_per_gas: opts.max_fee_per_gas.clone(),
        gas_fee_token: opts.gas_fee_token.clone(),
        // The tier the shell copied from the displayed estimate, filtered to
        // one the relay accepts (spec 069): never the dead `rapid`.
        quoted_fee: opts.quoted_fee.clone().map(|fee| SignQuotedFee {
            tier: fee.tier.and_then(super::fee_speed::wire_tier),
            ..fee
        }),
        stage: Stage::Precheck,
        record_id: None,
        op_hash: None,
        ceremony: Ceremony::NotYet,
        write_ahead: None,
        accepted: false,
        page_gone: false,
        batch_answers_object: method_kind(&pending.method) == SignMethodKind::Batch
            && serde_json::from_str::<Value>(&pending.params_json)
                .is_ok_and(|sent| super::dapp_rpc::send_calls_answers_object(&sent)),
    });

    if matches!(
        method_kind(&pending.method),
        SignMethodKind::Transaction | SignMethodKind::Batch
    ) {
        // Proactive gas pre-check, mirrored from the Send flow — resolve
        // funding BEFORE the passkey prompt (`dapp-connection.tsx:660-698`).
        let command = request_op(
            model,
            SignOperation::CheckBundlerFunding {
                chain_id,
                account: signer.address,
                bundler_cost_wei: opts.bundler_cost_wei.clone(),
                bust_cache,
            },
            true,
        );
        Command::all([command, render()])
    } else {
        proceed_submit(model)
    }
}

/// The submit chokepoint. `enforce_no_unlimited` rules here for the single
/// request AND every batch leg (`use-dapp-signing.ts:364, 413-415`) — waived
/// only when the approval surface showed the unbounded amount and the person
/// kept it (`Inflight::unlimited_approved`, 2026-09-26 ruling in
/// `approval_guard`'s module doc).
fn proceed_submit(model: &mut Model) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&fl.params_json) else {
        return fail_inflight(
            model,
            CODE_INTERNAL,
            SignErrorKind::InvalidParams,
            Some("malformed params".to_owned()),
        );
    };

    // Spec 081 FR-005 again, on the FINAL params: the shell may hand back a
    // rewritten `params_override_json`, and those are the bytes that get signed.
    let signer = model
        .accounts
        .get(model.active_index as usize)
        .map(|a| a.address.clone())
        .unwrap_or_default();
    if let Err(block) = enforce_no_self_call(&fl.method, Some(&parsed), &signer) {
        model.blocked = Some(blocked_view(&block));
        return fail_inflight(
            model,
            CODE_INTERNAL,
            SignErrorKind::SelfCallBlocked,
            Some(block.function.as_str().to_owned()),
        );
    }

    if fl.unlimited_approved {
        // Seen and kept — the site's bytes go out as asked.
    } else if let Err(refusal) = enforce_no_unlimited(&fl.method, Some(&parsed)) {
        return fail_inflight(
            model,
            CODE_INTERNAL,
            SignErrorKind::UnlimitedApproval,
            Some(refusal.amount_raw),
        );
    }
    if fl.method == "wallet_sendCalls" && !fl.unlimited_approved {
        // A batch must not smuggle an unbounded approval past the per-tx
        // guard — check every leg as a standalone transaction.
        let calls = parsed
            .get(0)
            .and_then(|p| p.get("calls"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for call in &calls {
            let leg = json!([{
                "to": call.get("to").cloned().unwrap_or(Value::Null),
                "data": call.get("data").cloned().unwrap_or(Value::Null),
                "value": call.get("value").cloned().unwrap_or(Value::Null),
            }]);
            if let Err(refusal) = enforce_no_unlimited("eth_sendTransaction", Some(&leg)) {
                return fail_inflight(
                    model,
                    CODE_INTERNAL,
                    SignErrorKind::UnlimitedApproval,
                    Some(refusal.amount_raw),
                );
            }
        }
    }

    if let Some(inner) = model.inflight.as_mut() {
        inner.stage = Stage::Submitting;
    }
    let command = request_op(
        model,
        SignOperation::SignAndSubmit {
            id: fl.id,
            method: fl.method,
            params_json: fl.params_json,
            chain_id: fl.chain_id,
            address: fl.address,
            credential_id: fl.credential_id,
            max_fee_per_gas: fl.max_fee_per_gas,
            gas_fee_token: fl.gas_fee_token,
            quoted_fee: fl.quoted_fee,
        },
        true,
    );
    Command::all([command, render()])
}

/// Refuse an approve before any pipeline work: error response to the pending
/// owner, error shown, modal stays open (the TS catch path).
fn fail_pending(
    model: &mut Model,
    code: i32,
    kind: SignErrorKind,
    detail: Option<String>,
) -> Command<SignEffect, Event> {
    model.inflight = None;
    let Some(p) = model.pending.as_mut() else {
        return render();
    };
    p.responded = true;
    let op = respond_op(
        &p.transport_id.clone(),
        &p.id.clone(),
        err_payload(code, kind, detail.clone()),
    );
    model.sign_error = Some(SignErrorNotice { kind, detail });
    model.sign_error_refused = false;
    ops_and_render(model, vec![op])
}

/// Terminal pipeline failure: respond to the pipeline's OWN owner (F2), patch
/// its record failed, surface the error if it still owns the sheet.
fn fail_inflight(
    model: &mut Model,
    code: i32,
    kind: SignErrorKind,
    detail: Option<String>,
) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.take() else {
        return render();
    };
    let mut ops = Vec::new();
    if let Some(record_id) = &fl.record_id {
        // A submitted-then-failed op must not linger 'pending' forever
        // (`dapp-connection.tsx:880-884`).
        ops.push(SignOperation::UpdateRecord {
            record_id: record_id.clone(),
            close: SignRecordClose::Failed,
        });
    }
    let payload = err_payload(code, kind, detail.clone());
    match model.pending.as_mut().filter(|p| p.id == fl.id) {
        // Its sheet is up: the failure is shown, and the answer waits for the
        // person to close it — or is dropped when they try again (spec 096
        // F8). Answering now let the extension worker close the window over
        // the failure before a word of it was read.
        Some(p) => {
            p.held = Some(payload);
            model.sign_error = Some(SignErrorNotice { kind, detail });
            model.sign_error_refused = false;
        }
        // Nobody is looking (dismissed while it ran, or superseded): the
        // page is answered at once.
        None => ops.push(respond_op(&fl.transport_id, &fl.id, payload)),
    }
    ops_and_render(model, ops)
}

// ---------------------------------------------------------------------------
// Reject / dismiss / funding
// ---------------------------------------------------------------------------

fn reject(model: &mut Model) -> Command<SignEffect, Event> {
    let Some(pending) = model.pending.clone() else {
        return Command::done();
    };
    // ①: past the commitment point a reject is impossible — the swipe already
    // routes to dismiss; a stray reject event is ignored rather than letting
    // a 4001 precede a broadcast (fail-closed).
    if model.committed() {
        return Command::done();
    }
    // A response already went out (error shown) — closing is a dismiss, never
    // a second response for the same id. A HELD failure is closed the same
    // way, and the dismissal sends it (spec 096 F8) — never a 4001 for a
    // request the person approved.
    if pending.responded || pending.held.is_some() {
        return dismiss(model);
    }
    // Spec 081: the wallet refused this one, not the person — the dApp is told
    // which, so a page cannot report "user rejected" for a decision the user
    // was never offered.
    let op = match model.blocked.as_ref() {
        Some(blocked) => respond_op(
            &pending.transport_id,
            &pending.id,
            err_payload(
                CODE_INTERNAL,
                SignErrorKind::SelfCallBlocked,
                Some(blocked.function.clone()),
            ),
        ),
        None => respond_op(
            &pending.transport_id,
            &pending.id,
            err_payload(CODE_USER_REJECTED, SignErrorKind::UserRejected, None),
        ),
    };
    model.settle(&pending.id, SignSettledOutcome::Rejected);
    // BUG-2: a reject DURING the pre-check/sponsorship aborts the pipeline
    // before it can submit (`signCancelledRef`, `dapp-connection.tsx:701-709`).
    kill_presubmit_pipeline(model);
    model.clear_sheet();
    ops_and_render(model, vec![op])
}

fn dismiss(model: &mut Model) -> Command<SignEffect, Event> {
    // No response, no pipeline abort: a dismissed-but-committed op proceeds
    // and its real result is still delivered (`dismissRequest`) — or, when its
    // passkey comes back cancelled instead, its refusal (083, `on_submit`).
    // The one answer a dismissal sends is a failure held for the sheet
    // (spec 096 F8): the page hears it now that it has been read.
    let held = model.answer_held();
    model.clear_sheet();
    model.notice = None;
    ops_and_render(model, held.into_iter().collect())
}

/// "Try again" (spec 096 F8): a failure that sent nothing goes back to
/// review — the held answer is dropped, the request is unanswered again and
/// the slide is live once more. A refusal, or a failure with no held answer,
/// has nothing to retry.
fn retry(model: &mut Model) -> Command<SignEffect, Event> {
    if !model.failure_retryable() {
        return Command::done();
    }
    if let Some(pending) = model.pending.as_mut() {
        pending.held = None;
    }
    model.sign_error = None;
    model.sign_error_refused = false;
    render()
}

fn funding_cancel(model: &mut Model) -> Command<SignEffect, Event> {
    model.funding = None;
    model.funding_pinned_rid = None;
    let Some(pending) = model.pending.take() else {
        return render();
    };
    model.sign_error = None;
    model.sign_error_refused = false;
    model.pending_op_hash = None;
    // ⑧: funding cancellation is NOT a user reject — recoverable -32603, so
    // the extension writes no durable 'rejected' (`dapp-connection.tsx:940-946`).
    let op = respond_op(
        &pending.transport_id,
        &pending.id,
        err_payload(CODE_INTERNAL, SignErrorKind::FundingCancelled, None),
    );
    ops_and_render(model, vec![op])
}

fn funding_complete(model: &mut Model) -> Command<SignEffect, Event> {
    if model.funding.take().is_none() {
        return Command::done();
    }
    let pinned = model.funding_pinned_rid.take();
    let Some(pending) = model.pending.as_ref() else {
        return render();
    };
    // ③ request-bind: replay ONLY if the request that asked for funding still
    // owns the sheet — pinned opts under a different id would submit the
    // wrong params (`dapp-connection.tsx:918-926`).
    if let Some(rid) = pinned {
        if pending.id != rid {
            return render();
        }
    }
    // Retry with the SAME opts (especially the capped paramsOverride), fresh
    // bundler cache (`:927-936`).
    let opts = model.last_opts.clone().unwrap_or_default();
    approve_with(model, opts, true)
}

// ---------------------------------------------------------------------------
// Mid-flight submission facts
// ---------------------------------------------------------------------------

/// The write-ahead (spec 082 RJ1): the op for `id` is signed and hashed, and
/// nothing has been POSTed. The record is written pending, "may have been
/// sent", and the op handed to the tracker; `ClearToPost` follows its ack.
///
/// The hand-off names NO record (082 second review): it tells the tracker a
/// POST of the op is about to leave, and the tracker holds the op off "not
/// sent" until the POST's verdict — which names the record ([`handoff_of`]).
/// Named here, the not-found grace ran from before the bytes left, and a POST
/// slower than it (15 s per relay endpoint, "currently processing" retries)
/// could end the op NotSent — answered "nothing was sent", its record failed —
/// while the POST was still delivering it.
fn on_op_signed(
    model: &mut Model,
    id: &str,
    user_op_hash: String,
    submit_block: Option<u64>,
    now_ms: f64,
) -> Command<SignEffect, Event> {
    let accepts = model.inflight.as_ref().is_some_and(|fl| {
        fl.id == id
            && matches!(fl.stage, Stage::Submitting)
            && fl.write_ahead.is_none()
            && fl.op_hash.is_none()
    });
    if !accepts {
        return Command::done();
    }
    let record_id = record_id_for("eth_sendTransaction", now_ms);
    let (record, chain_id) = {
        let Some(fl) = model.inflight.as_mut() else {
            return Command::done();
        };
        // Signed: whatever the shell said about the prompt (RA9).
        fl.ceremony = Ceremony::Done;
        fl.write_ahead = Some(WriteAhead {
            user_op_hash: user_op_hash.clone(),
            record_id: record_id.clone(),
            submit_block,
            cleared: false,
        });
        (
            pending_tx_record(fl, &record_id, &user_op_hash, now_ms, (true, submit_block)),
            fl.chain_id,
        )
    };
    model.tracker_handoff = Some(SignTrackerHandoff {
        user_op_hash,
        record_ids: Vec::new(),
        chain_id,
        maybe_sent: true,
        submit_block,
        admitted: false,
    });
    let command = request_op(model, SignOperation::PersistRecord { record }, false);
    Command::all([command, render()])
}

/// The hand-off of a write-ahead record once the POST's verdict is in (RJ1):
/// the relay took the op (`admitted`), or its reply was lost (`maybe_sent`).
/// It names the record — the tracker's patches find it on disk — and ends
/// the tracker's hold on the op (its not-found grace counts from here).
fn handoff_of(wa: &WriteAhead, chain_id: u32, admitted: bool) -> SignTrackerHandoff {
    SignTrackerHandoff {
        user_op_hash: wa.user_op_hash.clone(),
        record_ids: vec![wa.record_id.clone()],
        chain_id,
        maybe_sent: !admitted,
        submit_block: wa.submit_block,
        admitted,
    }
}

/// The pending dApp-tx record of `fl` under `user_op_hash`.
fn pending_tx_record(
    fl: &Inflight,
    record_id: &str,
    user_op_hash: &str,
    now_ms: f64,
    (maybe_sent, submit_block): (bool, Option<u64>),
) -> SignRecord {
    SignRecord {
        record_id: record_id.to_owned(),
        kind: SignRecordKind::DappTx,
        method: fl.method.clone(),
        params_json: fl.params_json.clone(),
        result: String::new(),
        from: fl.address.clone(),
        chain_id: fl.chain_id,
        now_ms,
        status: SignRecordStatus::Pending,
        user_op_hash: user_op_hash.to_owned(),
        dapp_origin: fl.record_origin.clone(),
        dapp_url: fl.record_url.clone(),
        intent: fl.intent.clone(),
        maybe_sent,
        submit_block,
        balance_changes: fl.balance_changes.clone(),
        ..kept_request(fl)
    }
}

/// The record's spec-093 fields for `fl`: its summary and its request as
/// stored. The rest of a [`SignRecord`] is the caller's.
fn kept_request(fl: &Inflight) -> SignRecord {
    let (stored_request, request_truncated) = super::dapp_activity::stored_request(&fl.params_json);
    SignRecord {
        record_id: String::new(),
        kind: SignRecordKind::DappTx,
        method: String::new(),
        params_json: String::new(),
        result: String::new(),
        from: String::new(),
        chain_id: 0,
        now_ms: 0.0,
        status: SignRecordStatus::Pending,
        user_op_hash: String::new(),
        dapp_origin: String::new(),
        dapp_url: String::new(),
        intent: None,
        maybe_sent: false,
        submit_block: None,
        balance_changes: None,
        summary: Some(fl.summary.clone()),
        stored_request,
        request_truncated,
    }
}

/// Withdraw the in-flight pipeline's write-ahead record, if it has one: its
/// op is proven never sent (RJ1). Returns the delete for the shell; the
/// tracker learns it through [`SignView::tracker_withdraw`], and a hand-off
/// still naming the op is taken back so no shell feeds it again.
fn withdraw_write_ahead(model: &mut Model) -> Vec<SignOperation> {
    let Some(wa) = model.inflight.as_mut().and_then(|fl| fl.write_ahead.take()) else {
        return Vec::new();
    };
    if model
        .tracker_handoff
        .as_ref()
        .is_some_and(|h| h.user_op_hash.eq_ignore_ascii_case(&wa.user_op_hash))
    {
        model.tracker_handoff = None;
    }
    model.tracker_withdraw = Some(SignTrackerWithdraw {
        user_op_hash: wa.user_op_hash,
        record_ids: vec![wa.record_id.clone()],
    });
    vec![SignOperation::DeleteRecord {
        record_id: wa.record_id,
    }]
}

/// Adopt the write-ahead record as the op's record: the relay's verdict (or
/// a result standing for it) names the same op.
fn adopt_write_ahead(model: &mut Model) {
    if let Some(fl) = model.inflight.as_mut() {
        if let Some(wa) = &fl.write_ahead {
            fl.op_hash = Some(wa.user_op_hash.clone());
            fl.record_id = Some(wa.record_id.clone());
        }
    }
}

fn on_op_submitted(
    model: &mut Model,
    id: &str,
    user_op_hash: String,
    now_ms: f64,
    (maybe_sent, submit_block): (bool, Option<u64>),
) -> Command<SignEffect, Event> {
    let matches_pipeline = model.inflight.as_ref().is_some_and(|fl| {
        fl.id == id && matches!(fl.stage, Stage::Submitting) && fl.op_hash.is_none()
    });
    if !matches_pipeline {
        return Command::done();
    }
    let write_ahead = model
        .inflight
        .as_ref()
        .and_then(|fl| fl.write_ahead.clone());
    // RJ1: the write-ahead record already names this op — no second record.
    // The relay taking it patches the record Admitted and tells the tracker
    // (an accepted op never reads "may have been sent"); a lost reply writes
    // nothing, the record already says so. Either way the tracker is handed
    // the record now: the POST is over (082 second review).
    if let Some(wa) = write_ahead
        .as_ref()
        .filter(|wa| wa.user_op_hash.eq_ignore_ascii_case(&user_op_hash))
    {
        adopt_write_ahead(model);
        let chain_id = match model.inflight.as_mut() {
            Some(fl) => {
                fl.ceremony = Ceremony::Done;
                fl.accepted = !maybe_sent;
                fl.chain_id
            }
            None => return Command::done(),
        };
        if model.inflight_matches_pending() {
            model.pending_op_hash = Some(wa.user_op_hash.clone());
            model.pending_op_maybe_sent = maybe_sent;
        }
        let submit_block = wa.submit_block.or(submit_block);
        let wa = WriteAhead {
            submit_block,
            ..wa.clone()
        };
        model.tracker_handoff = Some(handoff_of(&wa, chain_id, !maybe_sent));
        if maybe_sent {
            return render();
        }
        let mut operations = vec![SignOperation::UpdateRecord {
            record_id: wa.record_id.clone(),
            close: SignRecordClose::Admitted,
        }];
        // The relay has the batch, and its record is on disk: the page gets
        // its id now (spec 097 E).
        operations.extend(answer_batch_id(model));
        return ops_and_render(model, operations);
    }
    // A relay hash that is not the local one (`userop.hash_mismatch`): the
    // write-ahead op will never land under its hash — withdraw it, and
    // record the op under the relay's (today's path).
    let mut operations = withdraw_write_ahead(model);
    let record_id = record_id_for("eth_sendTransaction", now_ms);
    let (record, chain_id) = {
        let Some(fl) = model.inflight.as_mut() else {
            return Command::done();
        };
        fl.op_hash = Some(user_op_hash.clone());
        fl.record_id = Some(record_id.clone());
        fl.accepted = !maybe_sent;
        // Whatever the shell said about the prompt, an op on its way to the
        // relay has been signed (RA9).
        fl.ceremony = Ceremony::Done;
        (
            pending_tx_record(
                fl,
                &record_id,
                &user_op_hash,
                now_ms,
                (maybe_sent, submit_block),
            ),
            fl.chain_id,
        )
    };
    if model.inflight_matches_pending() {
        model.pending_op_hash = Some(user_op_hash.clone());
        model.pending_op_maybe_sent = maybe_sent;
    }
    // §4: the durable record precedes anything the dApp can poll — persisted
    // the moment the bundler accepts, before the receipt wait
    // (`dapp-connection.tsx:718-746`). The tracker handoff rides the view;
    // the shell feeds `tx_tracker::Event::Submitted` (idempotent per hash).
    model.tracker_handoff = Some(SignTrackerHandoff {
        user_op_hash,
        record_ids: vec![record_id],
        chain_id,
        maybe_sent,
        submit_block,
        admitted: false,
    });
    operations.push(SignOperation::PersistRecord { record });
    ops_and_render(model, operations)
}

/// The tracker's word on the in-flight op (spec 082 RJ3, RJ4). Answers the
/// waiting page when it is final — at once, or, for a refusal or "not sent"
/// the sheet shows, when the person closes it (spec 097 N4) — and only
/// then: a relay's `submitted` tx hash can still be replaced by a fee bump,
/// and `included` becomes Confirmed within one poll through the tracker's
/// `TxReceipt`.
/// The inflight clears and the attempt moves on, so the shell's own late
/// `Submit` result (the window's end) is dropped — never a second answer,
/// never an answer to a newer request.
fn on_op_tracked(
    model: &mut Model,
    user_op_hash: &str,
    status: TrackStatus,
    tx_hash: Option<String>,
) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    let ours = matches!(fl.stage, Stage::Submitting)
        && fl.record_id.is_some()
        && fl
            .op_hash
            .as_deref()
            .is_some_and(|op| op.eq_ignore_ascii_case(user_op_hash));
    if !ours {
        return Command::done();
    }
    let tx_hash = tx_hash.filter(|hash| !hash.trim().is_empty());
    let (payload, refusal) = match (status, tx_hash) {
        // A batch's answer is its id, never the tx it landed in (spec 097 E).
        (TrackStatus::Confirmed, Some(tx_hash)) => {
            (ok_answer(&fl, batch_id(&fl).unwrap_or(tx_hash)), None)
        }
        // Included and reverted (the tracker's `Dropped` with a transaction):
        // the page hears the revert, never the hash a site would read as
        // done (083, owner ruling 2026-10-01). The tracker has closed the
        // record failed; the sheet hands over to the ending, as for the
        // shell's own [`SignSubmitOutcome::Reverted`].
        (TrackStatus::Dropped, Some(tx_hash)) => (
            err_payload(
                CODE_INTERNAL,
                SignErrorKind::SubmitFailed,
                Some(reverted_detail(&tx_hash)),
            ),
            None,
        ),
        (TrackStatus::Rejected, _) => (
            err_payload(
                CODE_INTERNAL,
                SignErrorKind::SubmitFailed,
                Some(crate::user_op::REFUSED_DAPP_DETAIL.to_owned()),
            ),
            Some(true),
        ),
        // Stale for an op the relay accepted: the tracker reached it before
        // it learned `admitted` (a verdict while the POST was still out). The
        // tracker's own verdict follows; "nothing was sent" never answers an
        // op on its way (082 round-2 review).
        (TrackStatus::NotSent, _) if fl.accepted => return Command::done(),
        (TrackStatus::NotSent, _) => (
            err_payload(
                CODE_INTERNAL,
                SignErrorKind::SubmitFailed,
                Some(crate::user_op::NOT_SENT_DAPP_DETAIL.to_owned()),
            ),
            Some(false),
        ),
        _ => return Command::done(),
    };
    model.settle(&fl.id, SignSettledOutcome::Submitted);
    model.inflight = None;
    model.attempt += 1;
    let shows_it = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
    match refusal {
        None => {
            if shows_it {
                model.clear_sheet();
            }
        }
        // Nothing was sent: the sheet says it failed — as a refusal, with no
        // Retry, when the relay refused it — and no longer "submitting". The
        // answer is held while it shows, as for a failure before the submit
        // (spec 096 F8), and the close sends it (spec 097 N4): answered here,
        // the extension worker closed the request window over the words ~12 s
        // after "Submitted", and the person never read why. The rid is
        // settled `Submitted`, so the sheet offers no "Try again" (⑧).
        Some(refused) if shows_it => {
            if let SignResponsePayload::Err { kind, message, .. } = &payload {
                model.sign_error = Some(SignErrorNotice {
                    kind: *kind,
                    detail: message.clone(),
                });
            }
            model.sign_error_refused = refused;
            model.pending_op_hash = None;
            model.pending_op_maybe_sent = false;
            if let Some(p) = model.pending.as_mut() {
                p.held = Some(payload);
            }
            return render();
        }
        // Nobody is looking (closed while it ran, or superseded): the page
        // is answered at once.
        Some(_) => {}
    }
    ops_and_render(model, vec![respond_op(&fl.transport_id, &fl.id, payload)])
}

/// The passkey prompt of the pipeline for `id` opened or returned (RA9).
/// Id-guarded, and only inside the submit stage: a stale or early event
/// changes nothing, so the network wait before the prompt can never read
/// "waiting for your signature" (G22).
fn on_ceremony(model: &mut Model, id: &str, next: Ceremony) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.as_mut() else {
        return Command::done();
    };
    if fl.id != id || !matches!(fl.stage, Stage::Submitting) {
        return Command::done();
    }
    // Forward only: a late "started" never re-opens a finished prompt.
    let forward = matches!(
        (fl.ceremony, next),
        (Ceremony::NotYet, Ceremony::Up | Ceremony::Done) | (Ceremony::Up, Ceremony::Done)
    );
    if !forward {
        return Command::done();
    }
    fl.ceremony = next;
    render()
}

// ---------------------------------------------------------------------------
// Shell results — accepted by (stage, result) pairing
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: SignShellResult) -> Command<SignEffect, Event> {
    match result {
        SignShellResult::AccountSwitched => {
            // §12.1.6: the granted account is active — the approval surface
            // may now act.
            model.reconciled = true;
            render()
        }
        SignShellResult::Responded | SignShellResult::RecordUpdated => Command::done(),
        SignShellResult::RecordPersisted => on_record_persisted(model),
        SignShellResult::PreCheck { funding } => on_precheck(model, funding),
        SignShellResult::Sponsorship { outcome } => on_sponsorship(model, outcome),
        SignShellResult::Submit { outcome, now_ms } => on_submit(model, outcome, now_ms),
    }
}

fn on_precheck(
    model: &mut Model,
    funding: Option<SignFundingNeeded>,
) -> Command<SignEffect, Event> {
    if !model
        .inflight
        .as_ref()
        .is_some_and(|fl| matches!(fl.stage, Stage::Precheck))
    {
        return Command::done();
    }
    match funding {
        // No funding needed / timeout / error — proceed; the post-submit
        // classification is the safety net (`dapp-connection.tsx:666-698`).
        None => proceed_submit(model),
        Some(f) => {
            // Silent sponsorship first — the approve tap IS the commitment
            // moment; only a non-funded outcome surfaces UI.
            if let Some(fl) = model.inflight.as_mut() {
                fl.stage = Stage::Sponsoring { funding: f.clone() };
            }
            let command = request_op(
                model,
                SignOperation::AttemptSponsorship {
                    funding: f,
                    force: false,
                },
                true,
            );
            Command::all([command, render()])
        }
    }
}

fn on_sponsorship(model: &mut Model, outcome: SignSponsorship) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    match fl.stage {
        Stage::Sponsoring { funding } => {
            // ③ (fail-closed divergence): if a NEWER request owns the sheet,
            // the funding view must not hijack it — the superseded pipeline is
            // answered -32603 instead of left hanging (the TS proactive path
            // had no id check and could cover the new request; `:860-864` is
            // the reactive precedent this generalises).
            if !model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
                return fail_inflight(
                    model,
                    CODE_INTERNAL,
                    SignErrorKind::SubmitFailed,
                    Some("superseded before funding".to_owned()),
                );
            }
            match outcome {
                SignSponsorship::Funded => proceed_submit(model),
                SignSponsorship::Confirming => {
                    to_funding_wait(model, funding, SignFundingPresentation::Confirming, None)
                }
                SignSponsorship::Denied { reason } => {
                    to_funding_wait(model, funding, SignFundingPresentation::Topup, reason)
                }
            }
        }
        Stage::ReactiveSponsoring { funding, message } => {
            // ③: a late funding sheet never hijacks a newer request — the
            // superseded pipeline falls to its generic error response
            // (`dapp-connection.tsx:860-864`).
            if !model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
                return fail_inflight(
                    model,
                    CODE_INTERNAL,
                    SignErrorKind::SubmitFailed,
                    Some(message),
                );
            }
            match outcome {
                SignSponsorship::Denied { reason } => {
                    to_funding_wait(model, funding, SignFundingPresentation::Topup, reason)
                }
                // 'funded' from the forced retry still shows the confirming
                // beat — its first poll flips and replays (`:865-869`).
                _ => to_funding_wait(model, funding, SignFundingPresentation::Confirming, None),
            }
        }
        _ => Command::done(),
    }
}

/// Hand off to the in-sheet funding view, pinned to THIS request (③). The
/// pipeline ends here; the funding "Continue" re-approves with the saved opts.
fn to_funding_wait(
    model: &mut Model,
    funding: SignFundingNeeded,
    presentation: SignFundingPresentation,
    denial_reason: Option<String>,
) -> Command<SignEffect, Event> {
    let rid = model.inflight.as_ref().map(|fl| fl.id.clone());
    model.inflight = None;
    model.funding = Some(FundingState {
        data: funding,
        presentation,
        denial_reason,
    });
    model.funding_pinned_rid = rid;
    render()
}

fn on_submit(
    model: &mut Model,
    outcome: SignSubmitOutcome,
    now_ms: f64,
) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    if !matches!(fl.stage, Stage::Submitting) {
        return Command::done();
    }
    // Spec 082 RJ1: a write-ahead record with no `OpSubmitted` yet. A failure
    // now is proven "not sent" — nothing was POSTed without `ClearToPost`,
    // and a POST whose reply was lost is reported `OpSubmitted{maybe_sent}`,
    // not a failure — so the record is withdrawn, then the outcome is handled
    // as today. A result that stands for the same op adopts the record, and
    // the tracker is handed it: the POST is over (a receipt: the relay took
    // it; the window's end with the local hash: it may have been sent).
    let mut record_ops = Vec::new();
    if let Some(wa) = fl.write_ahead.as_ref().filter(|_| fl.record_id.is_none()) {
        let same_op = match &outcome {
            SignSubmitOutcome::Succeeded { .. } => true,
            SignSubmitOutcome::ReceiptPending { user_op_hash }
            | SignSubmitOutcome::Reverted { user_op_hash, .. }
            | SignSubmitOutcome::NotConfirmed { user_op_hash } => {
                user_op_hash.eq_ignore_ascii_case(&wa.user_op_hash)
            }
            _ => false,
        };
        if same_op {
            adopt_write_ahead(model);
            // A receipt, reverted or not, is the relay having taken it.
            let admitted = matches!(
                outcome,
                SignSubmitOutcome::Succeeded { .. } | SignSubmitOutcome::Reverted { .. }
            );
            model.tracker_handoff = Some(handoff_of(wa, fl.chain_id, admitted));
            if admitted {
                record_ops.push(SignOperation::UpdateRecord {
                    record_id: wa.record_id.clone(),
                    close: SignRecordClose::Admitted,
                });
            }
        } else {
            let ack_in_flight = !wa.cleared;
            record_ops = withdraw_write_ahead(model);
            if ack_in_flight {
                // `RecordPersisted` names no record: the withdrawn record's
                // ack, still out (the shell's wait ran out on a stalled
                // disk), must never clear a NEWER request's POST before its
                // own record is on disk. This is the pipeline's last result,
                // so moving the attempt on drops only that ack (082 review).
                model.attempt += 1;
            }
        }
    }
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    let command = on_submit_outcome(model, &fl, outcome, now_ms);
    if record_ops.is_empty() {
        command
    } else {
        Command::all([ops_and_render(model, record_ops), command])
    }
}

fn on_submit_outcome(
    model: &mut Model,
    fl: &Inflight,
    outcome: SignSubmitOutcome,
    now_ms: f64,
) -> Command<SignEffect, Event> {
    // Spec 082 RA2/RA8 (G21): once `OpSubmitted` handed the op to the tracker
    // it is on its way — or may be. A failure would tell the page "not sent"
    // (it asks again: the double payment), show "submit failed", and patch a
    // record only the tracker may close. So whatever else a shell reports now
    // is answered as the wait running out.
    //
    // And the wait running out is never answered with the op hash for a
    // transaction (owner ruling 2026-10-01, 083): no node knows that hash, so
    // a site that looks it up waits on "pending" forever. The page is told
    // the transaction is not confirmed yet; the record stays pending for the
    // tracker. A `wallet_sendCalls` batch id IS the op hash (EIP-5792).
    //
    // Spec 097 E (S2): and a batch is answered with that id however the wait
    // ended — a receipt in time included. The desktop answered its tx hash,
    // and `wallet_getCallsStatus(<tx hash>)` read "Unknown bundle id" while
    // PancakeSwap said "Proceed in your wallet" over a swap that had landed.
    let is_tx = fl.method == "eth_sendTransaction";
    let is_batch = method_kind(&fl.method) == SignMethodKind::Batch;
    let outcome = match (&fl.op_hash, fl.record_id.is_some(), outcome) {
        (
            Some(op_hash),
            true,
            SignSubmitOutcome::Succeeded { .. } | SignSubmitOutcome::NotConfirmed { .. },
        ) if is_batch => SignSubmitOutcome::ReceiptPending {
            user_op_hash: op_hash.clone(),
        },
        (
            Some(op_hash),
            true,
            SignSubmitOutcome::Failed { .. }
            | SignSubmitOutcome::Underfunded { .. }
            | SignSubmitOutcome::PasskeyCancelled
            | SignSubmitOutcome::AskerGone,
        ) if is_tx => SignSubmitOutcome::NotConfirmed {
            user_op_hash: op_hash.clone(),
        },
        (
            Some(op_hash),
            true,
            SignSubmitOutcome::Failed { .. }
            | SignSubmitOutcome::Underfunded { .. }
            | SignSubmitOutcome::PasskeyCancelled
            | SignSubmitOutcome::AskerGone,
        ) => SignSubmitOutcome::ReceiptPending {
            user_op_hash: op_hash.clone(),
        },
        (_, _, SignSubmitOutcome::ReceiptPending { user_op_hash }) if is_tx => {
            SignSubmitOutcome::NotConfirmed { user_op_hash }
        }
        (_, _, outcome) => outcome,
    };
    match outcome {
        SignSubmitOutcome::PasskeyCancelled => {
            model.inflight = None;
            // 083: "keep the modal open for a retry" means nothing once the
            // request is off the sheet — closed while the signature was still
            // to come (a dismiss: the pipeline was past the commitment point),
            // or replaced by a newer one — and nobody can retry or refuse it
            // any more. Sending nothing left the page's promise unsettled for
            // good (the desktop's tab queued every later request behind it).
            // The prompt came back unsigned: the person's refusal, once — the
            // same rule as `on_sponsorship`, which answers a superseded
            // pipeline rather than leave it hanging. Not to a page already
            // gone (it has its 4900), not while the request is back on the
            // sheet (open for a retry like any other), and never after a
            // submission (①) — a cancelled prompt has none, and an op hash
            // would make the answer the chain's. On the phones (one core per
            // request, so never superseded) this is only their ✕ during the
            // commitment window followed by a dismissed passkey sheet, which
            // answered nothing before.
            let on_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
            if !on_sheet && !fl.page_gone && fl.op_hash.is_none() {
                model.settle(&fl.id, SignSettledOutcome::Rejected);
                let op = respond_op(
                    &fl.transport_id,
                    &fl.id,
                    err_payload(CODE_USER_REJECTED, SignErrorKind::UserRejected, None),
                );
                return ops_and_render(model, vec![op]);
            }
            // Otherwise keep the modal open, send nothing — never an error,
            // never a durable 'rejected' (`dapp-connection.tsx:808-812`; ⑧).
            render()
        }
        SignSubmitOutcome::AskerGone => {
            // Spec 082 RB2: the page that asked is gone and nothing was sent.
            // No answer (nobody to take it), no record (nothing to track),
            // and no settle — the rid never signed.
            model.inflight = None;
            if model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
                model.clear_sheet();
            }
            render()
        }
        SignSubmitOutcome::Succeeded { result } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            if fl.record_id.is_some() {
                // tx path: the pending record already exists (§4) and the
                // tracker holds its op (`tracker_handoff`). Answer the page
                // only: the tracker alone closes on-chain records (spec 082
                // RA8, W3) — it is the one place a revert (including a Safe
                // `ExecutionFailure` inside a "successful" op) is judged, and
                // two writers of one record race. A receipt that reverted is
                // the shell's `Reverted`, never this (083).
                let ops = vec![respond_op(&fl.transport_id, &fl.id, ok_answer(fl, result))];
                let clears_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
                model.inflight = None;
                if clears_sheet {
                    model.clear_sheet();
                }
                ops_and_render(model, ops)
            } else {
                // §4 signature/batch path: the durable record must land BEFORE
                // the result the dApp polls — persist, then respond on the ack
                // (`dapp-connection.tsx:752-770`).
                let (kind, _) = record_shape(&fl.method);
                let record = SignRecord {
                    record_id: record_id_for(&fl.method, now_ms),
                    kind,
                    method: fl.method.clone(),
                    params_json: fl.params_json.clone(),
                    // A signature stays with the page that asked for it
                    // (spec 093): the disk keeps that it was given, never the
                    // signature itself. A batch's result is its id, which the
                    // tracker follows.
                    result: if kind == SignRecordKind::DappTx {
                        result.clone()
                    } else {
                        String::new()
                    },
                    from: fl.address.clone(),
                    chain_id: fl.chain_id,
                    now_ms,
                    status: SignRecordStatus::Confirmed,
                    user_op_hash: fl.op_hash.clone().unwrap_or_default(),
                    dapp_origin: fl.record_origin.clone(),
                    dapp_url: fl.record_url.clone(),
                    intent: fl.intent.clone(),
                    maybe_sent: false,
                    submit_block: None,
                    balance_changes: fl.balance_changes.clone(),
                    ..kept_request(fl)
                };
                if let Some(inner) = model.inflight.as_mut() {
                    inner.record_id = Some(record.record_id.clone());
                    inner.stage = Stage::PersistingResult {
                        then: AfterRecord::Result(result),
                    };
                }
                let command = request_op(model, SignOperation::PersistRecord { record }, false);
                Command::all([command, render()])
            }
        }
        SignSubmitOutcome::ReceiptPending { user_op_hash } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            if fl.record_id.is_none() {
                return persist_pending_then(
                    model,
                    fl,
                    user_op_hash.clone(),
                    now_ms,
                    AfterRecord::Result(user_op_hash),
                );
            }
            // The pending record from `OpSubmitted` stays pending — the
            // tracker already holds it (`tracker_handoff`) and alone may
            // close it. Answer the page, emit NO confirming patch.
            let respond = respond_op(&fl.transport_id, &fl.id, ok_answer(fl, user_op_hash));
            let clears_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
            model.inflight = None;
            if clears_sheet {
                model.clear_sheet();
            }
            ops_and_render(model, vec![respond])
        }
        // 083: included and reverted — one error, the record failed. The
        // hash is the transaction's, and a site that read it on its own
        // node would have called the swap done.
        SignSubmitOutcome::Reverted {
            user_op_hash,
            tx_hash,
        } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            let detail = reverted_detail(&tx_hash);
            if fl.record_id.is_none() {
                return persist_pending_then(
                    model,
                    fl,
                    user_op_hash,
                    now_ms,
                    AfterRecord::Reverted(detail),
                );
            }
            answer_reverted(model, detail)
        }
        // 083: the whole cap went by with no transaction. The page hears
        // that, not a hash; the record stays pending, the sheet follows the
        // operation, and the tracker alone may still close it either way.
        SignSubmitOutcome::NotConfirmed { user_op_hash } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            let detail = not_confirmed_detail(&user_op_hash);
            if fl.record_id.is_none() {
                return persist_pending_then(
                    model,
                    fl,
                    user_op_hash,
                    now_ms,
                    AfterRecord::NotConfirmed(detail),
                );
            }
            answer_still_landing(model, detail)
        }
        SignSubmitOutcome::Underfunded { message, funding } => match funding {
            Some(f) => {
                // Reactive recovery: try to heal silently before asking the
                // user for anything (`dapp-connection.tsx:850-854`).
                if let Some(inner) = model.inflight.as_mut() {
                    inner.stage = Stage::ReactiveSponsoring {
                        funding: f.clone(),
                        message,
                    };
                }
                let command = request_op(
                    model,
                    SignOperation::AttemptSponsorship {
                        funding: f,
                        force: true,
                    },
                    false,
                );
                Command::all([command, render()])
            }
            None => fail_inflight(
                model,
                CODE_INTERNAL,
                SignErrorKind::SubmitFailed,
                Some(message),
            ),
        },
        SignSubmitOutcome::Failed { message, refused } => {
            // RJ3: the relay refused it — the page is told so in the fixed
            // sentence (the relay's words are diagnostics), and the sheet's
            // failure is a refusal, never "try again".
            let detail = if refused {
                crate::user_op::REFUSED_DAPP_DETAIL.to_owned()
            } else {
                message
            };
            let command = fail_inflight(
                model,
                CODE_INTERNAL,
                SignErrorKind::SubmitFailed,
                Some(detail),
            );
            model.sign_error_refused = refused && model.sign_error.is_some();
            command
        }
    }
}

/// No `OpSubmitted` was seen, yet the shell reports a submitted op: the
/// durable record must still precede the answer (§4) — persisted PENDING
/// under the op hash and handed to the tracker, then `then` on the ack.
fn persist_pending_then(
    model: &mut Model,
    fl: &Inflight,
    user_op_hash: String,
    now_ms: f64,
    then: AfterRecord,
) -> Command<SignEffect, Event> {
    let (kind, _) = record_shape(&fl.method);
    let record = SignRecord {
        record_id: record_id_for(&fl.method, now_ms),
        kind,
        method: fl.method.clone(),
        params_json: fl.params_json.clone(),
        result: String::new(),
        from: fl.address.clone(),
        chain_id: fl.chain_id,
        now_ms,
        status: SignRecordStatus::Pending,
        user_op_hash: user_op_hash.clone(),
        dapp_origin: fl.record_origin.clone(),
        dapp_url: fl.record_url.clone(),
        intent: fl.intent.clone(),
        maybe_sent: false,
        submit_block: None,
        balance_changes: fl.balance_changes.clone(),
        ..kept_request(fl)
    };
    model.tracker_handoff = Some(SignTrackerHandoff {
        user_op_hash: user_op_hash.clone(),
        record_ids: vec![record.record_id.clone()],
        chain_id: fl.chain_id,
        maybe_sent: false,
        submit_block: None,
        admitted: false,
    });
    // A sheet that stays up after the answer follows the operation, as it
    // would have from `OpSubmitted`; one the answer closes needs nothing.
    if !matches!(then, AfterRecord::Result(_)) && model.inflight_matches_pending() {
        model.pending_op_hash = Some(user_op_hash.clone());
    }
    if let Some(inner) = model.inflight.as_mut() {
        inner.op_hash = Some(user_op_hash);
        inner.record_id = Some(record.record_id.clone());
        inner.stage = Stage::PersistingResult { then };
    }
    let command = request_op(model, SignOperation::PersistRecord { record }, false);
    Command::all([command, render()])
}

/// Answer a request whose operation may still land (083,
/// [`SignSubmitOutcome::NotConfirmed`]): the page gets the error, once; the
/// record is left pending for the tracker. The sheet hands over to the
/// ending — [`ending_of`] reads this answer as
/// [`SignEnding::StillConfirming`], which follows the operation rather than
/// calling it failed (spec 082 RA8).
/// EIP-5792's id for the batch `fl` is submitting: its user operation's hash
/// — what `wallet_getCallsStatus` asks about and [`crate::app::dapp_rpc::calls_status`]
/// reads back (200 with the receipt, 100 pending, 400 refused). `None` for a
/// request that is not a batch, or before its operation exists.
fn batch_id(fl: &Inflight) -> Option<String> {
    (method_kind(&fl.method) == SignMethodKind::Batch)
        .then(|| fl.op_hash.clone())
        .flatten()
}

/// The page's `Ok` answer to `fl`, which went out as `result`: the hash or
/// signature itself — except a batch's id, which takes the shape its request
/// declared (spec 097 G: EIP-5792 2.0.0 is answered `{ id }`, 1.0 the bare
/// id; [`super::dapp_rpc::send_calls_result`]). Every `Ok` with a result goes
/// through here, so no answer path can skip the rule.
fn ok_answer(fl: &Inflight, result: String) -> SignResponsePayload {
    SignResponsePayload::Ok {
        result: Some(super::dapp_rpc::send_calls_result(
            &result,
            fl.batch_answers_object,
        )),
    }
}

/// Spec 097 E (S2): a batch is answered with its id the moment the relay has
/// taken its operation — on every shell, whatever its own receipt wait would
/// have said. The page follows the batch by that id (`wallet_getCallsStatus`);
/// the sheet follows the tracker ([`ending_of`] reads the id as
/// [`SignEnding::StillConfirming`]), and the tracker alone closes the record.
/// The desktop, iOS and Android waited for the receipt and answered the tx
/// hash, which no status lookup knows. Like [`on_op_tracked`], the inflight
/// clears and the attempt moves on, so the shell's own late `Submit` result
/// is dropped — never a second answer.
fn answer_batch_id(model: &mut Model) -> Vec<SignOperation> {
    let Some(fl) = model.inflight.clone() else {
        return Vec::new();
    };
    let Some(id) = batch_id(&fl) else {
        return Vec::new();
    };
    model.settle(&fl.id, SignSettledOutcome::Submitted);
    model.inflight = None;
    model.attempt += 1;
    if model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
        model.clear_sheet();
    }
    vec![respond_op(&fl.transport_id, &fl.id, ok_answer(&fl, id))]
}

fn answer_still_landing(model: &mut Model, detail: String) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.take() else {
        return render();
    };
    if model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
        model.clear_sheet();
    }
    let op = respond_op(
        &fl.transport_id,
        &fl.id,
        err_payload(CODE_INTERNAL, SignErrorKind::SubmitFailed, Some(detail)),
    );
    ops_and_render(model, vec![op])
}

/// Answer a request whose operation was included and REVERTED (083): the
/// page gets the one error naming the transaction, and the sheet hands over
/// to the ending, which [`ending_of`] reads as [`SignEnding::Reverted`]
/// (never "couldn't be submitted, your funds are safe": it was included).
/// The record is the tracker's to close (spec 082 RA8): it holds the op and
/// reads the same receipt — failed, the one place a revert is judged.
fn answer_reverted(model: &mut Model, detail: String) -> Command<SignEffect, Event> {
    let Some(fl) = model.inflight.take() else {
        return render();
    };
    let op = respond_op(
        &fl.transport_id,
        &fl.id,
        err_payload(CODE_INTERNAL, SignErrorKind::SubmitFailed, Some(detail)),
    );
    if model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
        model.clear_sheet();
    }
    ops_and_render(model, vec![op])
}

fn on_record_persisted(model: &mut Model) -> Command<SignEffect, Event> {
    // The §4 record-then-respond step, and the write-ahead record's ack
    // (spec 082 RJ1): on disk, so the shell may POST. Any other ack of the
    // tx pending record (stage Submitting) needs no transition.
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    if matches!(fl.stage, Stage::Submitting) {
        let clear = fl
            .write_ahead
            .as_ref()
            .filter(|wa| !wa.cleared && fl.op_hash.is_none())
            .map(|wa| wa.user_op_hash.clone());
        let Some(user_op_hash) = clear else {
            return Command::done();
        };
        if let Some(wa) = model.inflight.as_mut().and_then(|f| f.write_ahead.as_mut()) {
            wa.cleared = true;
        }
        return ops_and_render(
            model,
            vec![SignOperation::ClearToPost {
                id: fl.id,
                user_op_hash,
            }],
        );
    }
    let Stage::PersistingResult { then } = &fl.stage else {
        return Command::done();
    };
    match then.clone() {
        AfterRecord::Result(result) => {
            let op = respond_op(&fl.transport_id, &fl.id, ok_answer(&fl, result));
            let clears_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
            model.inflight = None;
            if clears_sheet {
                model.clear_sheet();
            }
            ops_and_render(model, vec![op])
        }
        // The record just written pending is closed failed with the answer.
        AfterRecord::Reverted(detail) => answer_reverted(model, detail),
        AfterRecord::NotConfirmed(detail) => answer_still_landing(model, detail),
    }
}

impl super::SplitEffect for SignEffect {
    type Op = SignOperation;
    fn into_shell(self) -> Option<crux_core::Request<SignOperation>> {
        match self {
            SignEffect::Render(_) => None,
            SignEffect::Shell(request) => Some(request),
        }
    }
}
