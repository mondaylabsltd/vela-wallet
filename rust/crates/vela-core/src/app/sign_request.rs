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
//!   at the submit chokepoint for the single tx and every batch leg.
//!
//! Wave-A kernels are composed in Rust: `approval_guard::enforce_no_unlimited`
//! rules at the submit throat, and `fee_policy::tempo_quote_is_stale` guards
//! the displayed-equals-signed Tempo fee before submission. `clear_signing` /
//! `approval_guard` view routing stays with those machines; the shell ANDs
//! this view's `confirm_gate_open` with `GuardView.confirm_allowed` and
//! `FeeView.confirm_fee_ready` (the single fee confirmation gate).
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
        result: Option<String>,
    },
    Err {
        code: i32,
        kind: SignErrorKind,
        message: Option<String>,
    },
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
    pub dapp_origin: String,
    pub intent: Option<String>,
}

/// The in-place patch closing a pending record — same id, never a second
/// record (`dapp-connection.tsx:779-784, 880-884`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignRecordClose {
    Confirmed { tx_hash: String },
    Failed,
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
    pub record_ids: Vec<String>,
    pub chain_id: u32,
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
    /// tx: the real tx hash after the receipt wait; batch: the userOpHash;
    /// signatures: the EIP-1271 signature hex.
    Succeeded {
        result: String,
    },
    /// The bundler accepted the op but its receipt did not arrive inside the
    /// shell's wait (~120 s). The page is still answered — with the op hash,
    /// since a dApp expects SOME hash — but nothing is known to have landed,
    /// so the pending record is NOT closed here: the tx tracker (handed the
    /// op at [`Event::OpSubmitted`]) settles it to confirmed/failed when the
    /// receipt actually appears (issue 262: an un-reimbursable op sat in the
    /// bundler forever while its record claimed "confirmed"). The core never
    /// tries to tell a tx hash from an op hash — both are 32-byte hex — so
    /// the shell must say which one it holds.
    ReceiptPending {
        user_op_hash: String,
    },
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
    },
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
    /// The modal was swipe-dismissed — the core dispatches by phase
    /// (`SigningRequestModal.tsx:34-42`): funding view → funding cancel;
    /// error / submitted / submitting → dismiss; else → reject.
    SwipeDismissed,
    /// Funding view "Continue" after a top-up.
    FundingCompleteTapped,
    /// Funding view cancel (and swipe over the funding view).
    FundingCancelled,
    /// The bundler accepted the op (`onSubmitted`) — mid-flight, before the
    /// final `Submit` result.
    OpSubmitted {
        id: String,
        user_op_hash: String,
        now_ms: f64,
    },
    /// A durable transport disconnected. Owner-aware clear
    /// (`dapp-connection.tsx:420-431`).
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
    } else if method.contains("signTypedData") {
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

/// `pickTypedDataParam` + `extractRequestChainId` (`use-dapp-signing.ts:124-156`):
/// the chain a request embeds, or `None`.
pub fn extract_request_chain_id(method: &str, params: &Value) -> Option<u32> {
    let arr = params.as_array()?;
    if method.contains("signTypedData") {
        let raw = if method == "eth_signTypedData" || method == "eth_signTypedData_v1" {
            arr.first()?
        } else {
            // `params[1] ?? params[0]` — null falls back too.
            match arr.get(1) {
                None | Some(Value::Null) => arr.first()?,
                Some(v) => v,
            }
        };
        let parsed;
        let typed = match raw {
            Value::String(s) => {
                parsed = serde_json::from_str::<Value>(s).ok()?;
                &parsed
            }
            other => other,
        };
        chain_from_value(typed.get("domain")?.get("chainId")?)
    } else if method == "eth_sendTransaction" || method == "wallet_sendCalls" {
        chain_from_value(arr.first()?.get("chainId")?)
    } else {
        None
    }
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
    /// §4: persisting the confirmed record BEFORE the response goes out.
    PersistingResult { result: String },
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
    chain_id: u32,
    address: String,
    credential_id: String,
    /// `requestDApp(...)?.name ?? request.origin` (`dapp-connection.tsx:729`).
    record_origin: String,
    intent: Option<String>,
    max_fee_per_gas: Option<String>,
    gas_fee_token: Option<String>,
    quoted_fee: Option<SignQuotedFee>,
    stage: Stage,
    record_id: Option<String>,
    op_hash: Option<String>,
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
    pending_op_hash: Option<String>,
    tracker_handoff: Option<SignTrackerHandoff>,
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

    fn clear_sheet(&mut self) {
        self.pending = None;
        self.blocked = None;
        self.funding = None;
        self.funding_pinned_rid = None;
        self.sign_error = None;
        self.pending_op_hash = None;
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

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignView {
    pub surface: SignSurface,
    pub request: Option<SignRequestView>,
    pub is_signing: bool,
    pub is_submitting: bool,
    pub pending_op_hash: Option<String>,
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
            } => on_op_submitted(model, &id, user_op_hash, now_ms),
            Event::TransportDropped { transport_id } => {
                if let Some(p) = &model.pending {
                    // Owner-aware (`dapp-connection.tsx:429`): keep the request
                    // only when it is stamped onto a DIFFERENT transport than
                    // the one that dropped.
                    let keep = p.dedicated_transport && p.transport_id != transport_id;
                    if !keep {
                        model.clear_sheet();
                    }
                }
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
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
                && !model.pending.as_ref().is_some_and(|p| p.responded),
            request,
            is_signing,
            is_submitting,
            pending_op_hash: model.pending_op_hash.clone(),
            error: model.sign_error.clone(),
            funding: model.funding.as_ref().map(|f| SignFundingView {
                data: f.data.clone(),
                presentation: f.presentation,
                denial_reason: f.denial_reason.clone(),
            }),
            reconcile_pending: !model.reconciled,
            swipe_action: swipe_action(model),
            tracker_handoff: model.tracker_handoff.clone(),
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
        if let Some(p) = &model.pending {
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
    // A response already went out for this id — never a second one (①).
    if pending.responded {
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
    model.pending_op_hash = None;

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

    model.inflight = Some(Inflight {
        id: pending.id.clone(),
        transport_id: pending.transport_id.clone(),
        method: pending.method.clone(),
        params_json: final_params,
        chain_id,
        address: signer.address.clone(),
        credential_id: signer.credential_id,
        record_origin,
        intent: opts.intent.clone(),
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
/// request AND every batch leg — the wallet-side terminal review of the
/// never-unlimited mandate (`use-dapp-signing.ts:364, 413-415`).
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

    if let Err(refusal) = enforce_no_unlimited(&fl.method, Some(&parsed)) {
        return fail_inflight(
            model,
            CODE_INTERNAL,
            SignErrorKind::UnlimitedApproval,
            Some(refusal.amount_raw),
        );
    }
    if fl.method == "wallet_sendCalls" {
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
    ops.push(respond_op(
        &fl.transport_id,
        &fl.id,
        err_payload(code, kind, detail.clone()),
    ));
    if model.pending.as_ref().is_some_and(|p| p.id == fl.id) {
        if let Some(p) = model.pending.as_mut() {
            p.responded = true;
        }
        model.sign_error = Some(SignErrorNotice { kind, detail });
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
    // a second response for the same id.
    if pending.responded {
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
    // and its real result is still delivered (`dismissRequest`).
    model.clear_sheet();
    model.notice = None;
    render()
}

fn funding_cancel(model: &mut Model) -> Command<SignEffect, Event> {
    model.funding = None;
    model.funding_pinned_rid = None;
    let Some(pending) = model.pending.take() else {
        return render();
    };
    model.sign_error = None;
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

fn on_op_submitted(
    model: &mut Model,
    id: &str,
    user_op_hash: String,
    now_ms: f64,
) -> Command<SignEffect, Event> {
    let matches_pipeline = model
        .inflight
        .as_ref()
        .is_some_and(|fl| fl.id == id && matches!(fl.stage, Stage::Submitting));
    if !matches_pipeline {
        return Command::done();
    }
    let record_id = record_id_for("eth_sendTransaction", now_ms);
    let (record, chain_id) = {
        let Some(fl) = model.inflight.as_mut() else {
            return Command::done();
        };
        fl.op_hash = Some(user_op_hash.clone());
        fl.record_id = Some(record_id.clone());
        (
            SignRecord {
                record_id: record_id.clone(),
                kind: SignRecordKind::DappTx,
                method: fl.method.clone(),
                params_json: fl.params_json.clone(),
                result: String::new(),
                from: fl.address.clone(),
                chain_id: fl.chain_id,
                now_ms,
                status: SignRecordStatus::Pending,
                user_op_hash: user_op_hash.clone(),
                dapp_origin: fl.record_origin.clone(),
                intent: fl.intent.clone(),
            },
            fl.chain_id,
        )
    };
    if model.inflight_matches_pending() {
        model.pending_op_hash = Some(user_op_hash.clone());
    }
    // §4: the durable record precedes anything the dApp can poll — persisted
    // the moment the bundler accepts, before the receipt wait
    // (`dapp-connection.tsx:718-746`). The tracker handoff rides the view;
    // the shell feeds `tx_tracker::Event::Submitted` (idempotent per hash).
    model.tracker_handoff = Some(SignTrackerHandoff {
        user_op_hash,
        record_ids: vec![record_id],
        chain_id,
    });
    let command = request_op(model, SignOperation::PersistRecord { record }, false);
    Command::all([command, render()])
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
    match outcome {
        SignSubmitOutcome::PasskeyCancelled => {
            // Keep the modal open, send nothing — never an error, never a
            // durable 'rejected' (`dapp-connection.tsx:808-812`; ⑧).
            model.inflight = None;
            render()
        }
        SignSubmitOutcome::Succeeded { result } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            if let Some(record_id) = fl.record_id.clone() {
                // tx path: the pending record already exists (§4) — respond,
                // then flip it confirmed in place, same id
                // (`dapp-connection.tsx:775-784`).
                let ops = vec![
                    respond_op(
                        &fl.transport_id,
                        &fl.id,
                        SignResponsePayload::Ok {
                            result: Some(result.clone()),
                        },
                    ),
                    SignOperation::UpdateRecord {
                        record_id,
                        close: SignRecordClose::Confirmed { tx_hash: result },
                    },
                ];
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
                    result: result.clone(),
                    from: fl.address.clone(),
                    chain_id: fl.chain_id,
                    now_ms,
                    status: SignRecordStatus::Confirmed,
                    user_op_hash: fl.op_hash.clone().unwrap_or_default(),
                    dapp_origin: fl.record_origin.clone(),
                    intent: fl.intent.clone(),
                };
                if let Some(inner) = model.inflight.as_mut() {
                    inner.record_id = Some(record.record_id.clone());
                    inner.stage = Stage::PersistingResult { result };
                }
                let command = request_op(model, SignOperation::PersistRecord { record }, false);
                Command::all([command, render()])
            }
        }
        SignSubmitOutcome::ReceiptPending { user_op_hash } => {
            model.settle(&fl.id, SignSettledOutcome::Submitted);
            let respond = respond_op(
                &fl.transport_id,
                &fl.id,
                SignResponsePayload::Ok {
                    result: Some(user_op_hash.clone()),
                },
            );
            if fl.record_id.is_some() {
                // The pending record from `OpSubmitted` stays pending — the
                // tracker already holds it (`tracker_handoff`) and alone may
                // close it. Answer the page, emit NO confirming patch.
                let clears_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
                model.inflight = None;
                if clears_sheet {
                    model.clear_sheet();
                }
                ops_and_render(model, vec![respond])
            } else {
                // No `OpSubmitted` was seen: the durable record must still
                // precede the answer (§4) — persisted PENDING under the op
                // hash and handed to the tracker, then answered on the ack.
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
                    intent: fl.intent.clone(),
                };
                model.tracker_handoff = Some(SignTrackerHandoff {
                    user_op_hash: user_op_hash.clone(),
                    record_ids: vec![record.record_id.clone()],
                    chain_id: fl.chain_id,
                });
                if let Some(inner) = model.inflight.as_mut() {
                    inner.op_hash = Some(user_op_hash.clone());
                    inner.record_id = Some(record.record_id.clone());
                    inner.stage = Stage::PersistingResult {
                        result: user_op_hash,
                    };
                }
                let command = request_op(model, SignOperation::PersistRecord { record }, false);
                Command::all([command, render()])
            }
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
        SignSubmitOutcome::Failed { message } => fail_inflight(
            model,
            CODE_INTERNAL,
            SignErrorKind::SubmitFailed,
            Some(message),
        ),
    }
}

fn on_record_persisted(model: &mut Model) -> Command<SignEffect, Event> {
    // Only the §4 record-then-respond step reacts here; the tx pending-record
    // ack (stage Submitting) needs no transition.
    let Some(fl) = model.inflight.clone() else {
        return Command::done();
    };
    let Stage::PersistingResult { result } = fl.stage else {
        return Command::done();
    };
    let op = respond_op(
        &fl.transport_id,
        &fl.id,
        SignResponsePayload::Ok {
            result: Some(result),
        },
    );
    let clears_sheet = model.pending.as_ref().is_some_and(|p| p.id == fl.id);
    model.inflight = None;
    if clears_sheet {
        model.clear_sheet();
    }
    ops_and_render(model, vec![op])
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
