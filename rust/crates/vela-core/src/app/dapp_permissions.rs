//! Machine — the per-origin dApp grant store, and the request window's entries
//! into it (spec `017-crux-wallet-state-complete`, inventory
//! `### dapp_permissions (P2)`; narrowed to this by spec 070 T063).
//!
//! ```text
//!  PopupRequest       ─► decide_popup_request ─► verdict on the view (pure)
//!  PopupApproved      ─► WriteGrant + SaveConnectionRecord + Respond
//!  PopupAccountSwitch ─► WriteGrant (re-pinned) | RemoveGrant | nothing
//!  settle_on_close()  ─► 4900 unknown-pending, never 4001
//! ```
//!
//! **The in-app browser consent flow is NOT here any more.** Spec 070 moved
//! every in-app browser — Android, iOS, desktop and web — onto
//! [`super::dapp_browser`], which owns tabs, documents, navigation, the consent
//! sheet, the page events and the forwarding of provider traffic to signing.
//! T063 then deleted this machine's copy of it: `ProviderRequest`,
//! `ConsentApproved`, `ConsentRejected`, `NavigationStarted`, `BrowserClosed`,
//! the consent queue, the parked requests, the in-flight grant reads and the
//! connected chip are gone, with the operations only they could author
//! (`ReadGrant`, `SettleForwarded`, `EmitEvent`, `ForwardToSigning`). Nobody
//! should re-add them: two decision paths for one set of rules is exactly the
//! drift spec 070 existed to end.
//!
//! What is left is the part no browser owns:
//!
//! - **the grant store's vocabulary** — [`DpermGrant`] (the `vela.perm.<origin>`
//!   value), the store writes ([`DpermOperation::WriteGrant`],
//!   [`DpermOperation::RemoveGrant`]) and the two pure rules that read it,
//!   [`resolve_granted`] and [`should_drop_grant`], which `dapp_browser` and the
//!   shells import from here so the rules exist once;
//! - **the web request window's entries** — a one-shot window with no tab, no
//!   document and no navigation, which owns its own grant I/O and its own
//!   transport: it asks [`Event::PopupRequest`] for a verdict, states
//!   [`Event::PopupApproved`] when the person presses Connect,
//!   [`Event::PopupAccountSwitch`] when the wallet changes account, and reads
//!   [`settle_on_close`] for the code a still-pending request is settled with;
//! - **the origin-security helpers** — [`is_insecure_public_origin`] with its
//!   fully-anchored IP exemptions, and [`origin_of`]. Both are imported by
//!   `dapp_browser` / `dapp_rpc`; neither is re-implemented there.
//!
//! Ported line by line from:
//!
//! - `src/services/dapp-permissions.ts` — grant store semantics,
//!   `resolveGranted` / `shouldDropGrant` including the load-bearing rule:
//!   NEVER drop a grant on a cold/empty account read, or a transient empty
//!   state logs the user out of every open dApp (invariant ②).
//! - `src/app/web-request.tsx:57-250` — the popup entry's grant checks
//!   (connect / not-connected 4100 / pinned-address 4100 / forward), and what
//!   its approve authors (grant, audit row, answer).
//! - `src/services/wallet-browser-router.ts:78-118` — the insecure-public-http
//!   classification with its fully-anchored IP exemptions
//!   (`10.0.0.1.evil.com` is a public FQDN an attacker can register and MUST
//!   NOT be exempt, invariant ③).
//! - `src/app/browser.tsx:66-73` — `originOf`.
//! - `src/services/webview-transport.ts:49-133` — the settle vocabulary: 4900
//!   on a window going away, never 4001 (invariant ⑤).
//!
//! Quirks kept verbatim: `grantedAt` never participates in any decision (grants
//! have no TTL — open question in the inventory); an account switch re-pins a
//! grant's ADDRESS but never rewrites its chain, which records the chain the
//! site connected on.
//!
//! Fail-closed deviations from JS (each marked at its site): non-http(s) URLs
//! never become an origin; an unparseable origin is treated as insecure,
//! exactly as the TS `catch` does.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Method sets — the single point (`wallet-browser-router.ts:20, 59-68`).
// Today SUPPORTED/SIGNING method knowledge lives in four places that must be
// hand-synchronized; every consumer now reads these.
// ---------------------------------------------------------------------------

/// `CONNECT_METHODS` — the only methods that may ask a person to connect.
pub const CONNECT_METHODS: [&str; 2] = ["eth_requestAccounts", "wallet_requestPermissions"];

/// Methods that move value or produce a signature (`wallet-browser-router.ts:59-68`).
pub const SIGNING_METHODS: [&str; 8] = [
    "eth_sendTransaction",
    "personal_sign",
    "eth_sign",
    "eth_signTypedData",
    "eth_signTypedData_v1",
    "eth_signTypedData_v3",
    "eth_signTypedData_v4",
    "wallet_sendCalls",
];

pub fn is_connect_method(method: &str) -> bool {
    CONNECT_METHODS.contains(&method)
}

/// [`SIGNING_METHODS`] AND anything else `dapp_rpc` calls a signature (spec
/// 070 — the two definitions had drifted: `eth_signTypedData_v2` escaped the
/// insecure-origin gate yet reached a sheet).
pub fn is_signing_method(method: &str) -> bool {
    SIGNING_METHODS.contains(&method) || super::dapp_rpc::is_signing_method(method)
}

// ---------------------------------------------------------------------------
// EIP-1193 error codes — load-bearing: 4900 vs 4001 is invariant ⑤.
// ---------------------------------------------------------------------------

pub const CODE_UNAUTHORIZED: u32 = 4100;
/// "Unknown-pending": the request may have landed — a dApp must NOT treat it
/// as safe to retry (which it does with 4001).
pub const CODE_UNKNOWN_PENDING: u32 = 4900;

/// Why a request was refused. Semantic — the shell owns the words; the code
/// is the core's because the code IS the behavior contract with the dApp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermRejectReason {
    /// A non-connect request from a never-connected origin
    /// (`web-request.tsx:187`).
    NotConnected,
    /// The request pinned an address that is no longer the granted one
    /// (`web-request.tsx:190-192`).
    StaleAuthorizedAddress,
    /// The window closed with the answer still pending — 4900, for the
    /// double-spend reason (`webview-transport.ts:77-79`).
    BrowserClosed,
}

impl DpermRejectReason {
    pub fn code(self) -> u32 {
        match self {
            Self::NotConnected | Self::StaleAuthorizedAddress => CODE_UNAUTHORIZED,
            Self::BrowserClosed => CODE_UNKNOWN_PENDING,
        }
    }
}

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One per-origin grant — serialises 1:1 with the `vela.perm.<origin>` KV
/// value (`DAppGrant`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermGrant {
    pub origin: String,
    /// The address the user granted — the grant is PINNED here, never to the
    /// wallet's active account (invariant ⑨).
    pub address: String,
    pub chain_id: u32,
    /// `Date.now()` at grant time, from the shell. Ported verbatim: this
    /// field never participates in any decision — grants have no TTL today
    /// (inventory open question owns whether that ever changes).
    pub granted_at_ms: f64,
}

/// What a `Respond` puts on the wire. The shell encodes the JSON: `Accounts`
/// is the address array; `Permissions` is the EIP-2255 shape
/// (`granted ? [{parentCapability:'eth_accounts'}] : []`). A refusal is not
/// here: it arrives as [`DpermPopupOutcome::Reject`], carrying its own code.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermRespondPayload {
    Accounts { addresses: Vec<String> },
    Permissions { granted: bool },
}

/// The web request window's verdict, on the wire.
///
/// A projection of [`DpermPopupDecision`] — [`decide_popup_request`] keeps
/// exactly the semantics it was ported with; this only gives the answer a
/// serialisable shape so the window can ASK for it. `ForwardToSigning`
/// carries the granted address because that is the address the sign path must
/// be pinned to (invariant ⑨: the grant's own address, never the wallet's
/// active account).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermPopupOutcome {
    Respond {
        payload: DpermRespondPayload,
    },
    /// Ask the person: show the window's connect consent.
    Consent,
    Reject {
        code: u32,
        reason: DpermRejectReason,
    },
    ForwardToSigning {
        granted_address: String,
    },
}

/// The answer to one [`Event::PopupRequest`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermPopupView {
    pub outcome: DpermPopupOutcome,
    /// [`resolve_granted`]'s answer for this origin — exposed so the window
    /// never re-derives the load-bearing cold-read rule (invariant ②) itself.
    pub granted: Vec<String>,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DpermOperation"))]
pub enum DpermOperation {
    /// Persist `vela.perm.<origin>` — best-effort; the shell swallows storage
    /// errors (`setGrant`).
    WriteGrant { grant: DpermGrant },
    /// Remove `vela.perm.<origin>` — best-effort (`revokeGrant`).
    RemoveGrant { origin: String },
    /// Answer one request via the window's transport.
    Respond {
        id: String,
        payload: DpermRespondPayload,
    },
    /// Write the "Connected to <app>" audit row (`buildConnectionRecord` +
    /// `saveTransaction` — the shell derives the display host and stamps its
    /// own clock, both presentation). A connection nobody can see is a
    /// connection nobody can revoke, so this is not optional.
    SaveConnectionRecord {
        address: String,
        chain_id: u32,
        origin: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DpermShellResult"))]
pub enum DpermShellResult {
    /// Every operation is fire-and-forget from the core's view.
    Ack,
}

impl Operation for DpermOperation {
    type Output = DpermShellResult;
}

#[effect]
pub enum DpermEffect {
    Render(RenderOperation),
    Shell(DpermOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DpermEvent"))]
pub enum Event {
    /// The web request window (`web-request.tsx:169-193`) asks for one
    /// request's verdict.
    ///
    /// PURE, on purpose: it requests no shell operation and only publishes its
    /// answer on the view — the `validate_pay_query` pattern
    /// (`payment_request.rs`), because the window is a one-shot surface that
    /// owns its own grant I/O and its own transport and has no document to emit
    /// page events into. Without it [`decide_popup_request`] is authored,
    /// tested and exported but never executed anywhere, which is worse than not
    /// having it: it reads as the source of truth for rules the shell is
    /// actually re-implementing.
    PopupRequest {
        method: String,
        /// The stored `vela.perm.<origin>` value — `None` when absent or
        /// unreadable (`getGrant`'s catch).
        grant: Option<DpermGrant>,
        /// Every wallet address. `None`/empty = not known yet (cold load), and
        /// [`resolve_granted`] must NOT log the origin out on that.
        current_addresses: Option<Vec<String>>,
        /// `peer.request.address`, the address the request pins itself to.
        /// The shell maps the TS empty string to `None`.
        pinned_address: Option<String>,
    },
    /// The request window's person pressed Connect (spec 070 T063).
    ///
    /// The window used to reach this by IMPERSONATING an in-app browser: it
    /// dispatched `provider_request`, answered the grant read, checked that a
    /// consent sheet had opened for its origin, then `consent_approved` — five
    /// events and a loop to author three operations, through a browser model
    /// the window has no part of (no tab, no document, no navigation).
    ///
    /// It says what it means now. The rules that matter are the same and still
    /// the core's: the grant is written for the ACTIVE address, the audit row
    /// that gives the person a trail is written with it, and the answer's shape
    /// follows the method — which is exactly the trio the shell was assembling
    /// by hand before spec 027 moved it here.
    PopupApproved {
        origin: String,
        /// The request this window is answering — `Respond` carries it back.
        request_id: String,
        /// `wallet_requestPermissions` answers with permissions; everything
        /// else answers with the address.
        method: String,
        /// The account the grant pins to: the active one.
        address: String,
        chain_id: u32,
        now_ms: f64,
    },
    /// The wallet switched accounts, asked about ONE connected site (spec 070
    /// T063).
    ///
    /// The window used to reach this by replaying a navigation and a page's
    /// first `eth_accounts` — a browser's opening moves, to make a grant-store
    /// machine re-pin one row. The rules are unchanged and still here:
    /// [`should_drop_grant`] physically removes a grant whose own account left
    /// the wallet, and a grant for an address the wallet still holds follows
    /// the active one. The chain is NOT rewritten: it records the chain the
    /// site connected on, which is an audit fact.
    PopupAccountSwitch {
        origin: String,
        /// The stored `vela.perm.<origin>` value.
        grant: Option<DpermGrant>,
        /// Every wallet address; `None`/empty = not known yet, and invariant ②
        /// says never to log a site out on that.
        current_addresses: Option<Vec<String>>,
        /// The account the wallet switched TO.
        active_address: String,
        now_ms: f64,
    },
    /// One authored operation came back from the shell. Every operation here is
    /// fire-and-forget ([`DpermShellResult::Ack`]); the core keeps no
    /// correlation state to update.
    #[serde(skip)]
    ShellCompleted,
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Model {
    /// The last [`Event::PopupRequest`] verdict — the only thing this machine
    /// remembers. The grant store itself lives in the shell's KV; the core
    /// authors its writes and is handed the value back on every question, so it
    /// keeps no mirror of it (nothing here would read one).
    popup: Option<DpermPopupView>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermView {
    /// The answer to the last [`Event::PopupRequest`]. `None` on every core
    /// that has never been asked one.
    pub popup: Option<DpermPopupView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct DappPermissions;

impl App for DappPermissions {
    type Event = Event;
    type Model = Model;
    type ViewModel = DpermView;
    type Effect = DpermEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<DpermEffect, Event> {
        match event {
            Event::PopupRequest {
                method,
                grant,
                current_addresses,
                pinned_address,
            } => popup_request(
                model,
                &method,
                grant.as_ref(),
                current_addresses.as_deref(),
                pinned_address.as_deref(),
            ),
            Event::PopupApproved {
                origin,
                request_id,
                method,
                address,
                chain_id,
                now_ms,
            } => popup_approved(&origin, &request_id, &method, &address, chain_id, now_ms),
            Event::PopupAccountSwitch {
                origin,
                grant,
                current_addresses,
                active_address,
                now_ms,
            } => popup_account_switch(
                &origin,
                grant.as_ref(),
                current_addresses.as_deref(),
                &active_address,
                now_ms,
            ),
            Event::ShellCompleted => Command::done(),
        }
    }

    fn view(&self, model: &Model) -> DpermView {
        DpermView {
            popup: model.popup.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Update handlers
// ---------------------------------------------------------------------------

/// The request window's one question, answered on the view.
///
/// Both halves of the answer come from the pure policy below — nothing is
/// re-decided here: [`resolve_granted`] says what this origin may see (and
/// refuses to log it out on a cold read, invariant ②), [`decide_popup_request`]
/// says what to do about it. The three rules the window exists to enforce are
/// therefore stated once, in Rust:
///
/// - a never-connected origin gets no address — 4100, not a forward;
/// - the forward is pinned to the GRANT's address, never the wallet's active
///   account (invariant ⑨);
/// - a request pinning some other address is refused 4100 — never a silent
///   swap of the signer for one the dApp did not ask for.
fn popup_request(
    model: &mut Model,
    method: &str,
    grant: Option<&DpermGrant>,
    current_addresses: Option<&[String]>,
    pinned_address: Option<&str>,
) -> Command<DpermEffect, Event> {
    let granted = resolve_granted(grant, current_addresses);
    let outcome = match decide_popup_request(method, &granted, pinned_address) {
        DpermPopupDecision::Respond(payload) => DpermPopupOutcome::Respond { payload },
        DpermPopupDecision::Consent => DpermPopupOutcome::Consent,
        DpermPopupDecision::Reject(reason) => DpermPopupOutcome::Reject {
            code: reason.code(),
            reason,
        },
        DpermPopupDecision::ForwardToSigning => match granted.first() {
            Some(address) => DpermPopupOutcome::ForwardToSigning {
                granted_address: address.clone(),
            },
            // Unreachable — `decide_popup_request` only forwards with a
            // non-empty grant. Fail closed rather than forward with no address
            // to pin the signer to.
            None => DpermPopupOutcome::Reject {
                code: DpermRejectReason::NotConnected.code(),
                reason: DpermRejectReason::NotConnected,
            },
        },
    };
    model.popup = Some(DpermPopupView { outcome, granted });
    render()
}

/// The request window's approve (spec 070 T063).
///
/// Three operations and no page events: this window has no document to push
/// `accountsChanged` into — its `Respond` IS the announcement.
fn popup_approved(
    origin: &str,
    request_id: &str,
    method: &str,
    address: &str,
    chain_id: u32,
    now_ms: f64,
) -> Command<DpermEffect, Event> {
    if origin.is_empty() || address.is_empty() {
        return Command::done();
    }
    let grant = DpermGrant {
        origin: origin.to_owned(),
        address: address.to_owned(),
        chain_id,
        granted_at_ms: now_ms,
    };
    let payload = if method == "wallet_requestPermissions" {
        DpermRespondPayload::Permissions { granted: true }
    } else {
        DpermRespondPayload::Accounts {
            addresses: vec![address.to_owned()],
        }
    };
    // Effect order made explicit, matching the flow the browser path ran:
    // persist first, then the audit row ("Connected to <app>"), then answer the
    // request that asked.
    finish(vec![
        DpermOperation::WriteGrant { grant },
        DpermOperation::SaveConnectionRecord {
            address: address.to_owned(),
            chain_id,
            origin: origin.to_owned(),
        },
        DpermOperation::Respond {
            id: request_id.to_owned(),
            payload,
        },
    ])
}

/// What one connected site hears when the wallet switches account (070 T063).
///
/// Authored from the two pure rules: a grant whose account LEFT the wallet is
/// removed, and one whose account is still held is re-pinned to the active
/// address. Everything else — including a cold read, where `current_addresses`
/// is not known yet — writes nothing, because invariant ② forbids logging a
/// site out on an empty read.
fn popup_account_switch(
    origin: &str,
    grant: Option<&DpermGrant>,
    current_addresses: Option<&[String]>,
    active_address: &str,
    now_ms: f64,
) -> Command<DpermEffect, Event> {
    let Some(grant) = grant else {
        return Command::done();
    };
    if origin.is_empty() || active_address.is_empty() {
        return Command::done();
    }
    if should_drop_grant(Some(grant), current_addresses) {
        return finish(vec![DpermOperation::RemoveGrant {
            origin: origin.to_owned(),
        }]);
    }
    if grant.address.eq_ignore_ascii_case(active_address) {
        return Command::done();
    }
    finish(vec![DpermOperation::WriteGrant {
        grant: DpermGrant {
            origin: origin.to_owned(),
            address: active_address.to_owned(),
            // The chain the site CONNECTED on: an audit fact a switch of
            // account must not rewrite.
            chain_id: grant.chain_id,
            granted_at_ms: now_ms,
        },
    }])
}

// ---------------------------------------------------------------------------
// Pure policy — grant resolution (`dapp-permissions.ts:53-78`)
// ---------------------------------------------------------------------------

/// The accounts to expose to an origin given the wallet's current addresses.
///
/// - No grant → `[]` (a disconnected wallet; `eth_accounts` never prompts).
/// - Grant + address still present → `[address]`.
/// - Grant + address gone → `[]`.
/// - Grant + UNKNOWN current addresses (cold load, `None`/empty) →
///   `[address]`: trust the grant, do NOT log the user out on a transient
///   empty read (the load-bearing rule, ported verbatim; invariant ②).
///
/// Address comparison is ASCII-case-insensitive, exactly what the TS
/// `toLowerCase()` on hex addresses does.
pub fn resolve_granted(
    grant: Option<&DpermGrant>,
    current_addresses: Option<&[String]>,
) -> Vec<String> {
    let Some(grant) = grant else {
        return Vec::new();
    };
    let Some(addresses) = current_addresses else {
        return vec![grant.address.clone()];
    };
    if addresses.is_empty() {
        return vec![grant.address.clone()];
    }
    if addresses
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&grant.address))
    {
        vec![grant.address.clone()]
    } else {
        Vec::new()
    }
}

/// Whether a grant should be dropped: ONLY when the account list is known
/// (present + non-empty) and no longer contains the granted address. Never on
/// a cold/empty read — that would revoke every dApp on app launch.
pub fn should_drop_grant(grant: Option<&DpermGrant>, current_addresses: Option<&[String]>) -> bool {
    let Some(grant) = grant else {
        return false;
    };
    let Some(addresses) = current_addresses else {
        return false;
    };
    if addresses.is_empty() {
        return false;
    }
    !addresses
        .iter()
        .any(|a| a.eq_ignore_ascii_case(&grant.address))
}

/// How a request still pending when a window goes away is settled (070 T063).
///
/// 4900, never 4001 — invariant ⑤, and the whole reason a settle carries a code
/// at all: a dApp treats an explicit "user rejected" as safe to retry, which
/// double-spends a request that may already have landed. The request window
/// asks for this rather than restating it, the way it used to ask by
/// dispatching `browser_closed` and reading the operation back.
#[must_use]
pub fn settle_on_close() -> (u32, DpermRejectReason) {
    (CODE_UNKNOWN_PENDING, DpermRejectReason::BrowserClosed)
}

// ---------------------------------------------------------------------------
// Pure policy — the request window's decision (`web-request.tsx:169-193`)
// ---------------------------------------------------------------------------

/// The request window decides differently from an in-app browser — most
/// notably a non-connect request from a never-connected origin is REFUSED
/// (4100) rather than forwarded, and a request may pin the address it was
/// built for. Kept as its own explicit function because that difference is a
/// rule, not an accident.
#[derive(Clone, Debug, PartialEq)]
pub enum DpermPopupDecision {
    Respond(DpermRespondPayload),
    /// Show the window's connect consent.
    Consent,
    Reject(DpermRejectReason),
    /// Hand to the window transport → signing pipeline.
    ForwardToSigning,
}

/// `pinned_address` is `peer.request.address` — `None` when the request
/// didn't pin one (the TS treats the empty string as absent; the shell maps
/// `''` → `None`). Chain support is asserted upstream (the network machine),
/// exactly as `assertChainSupported` runs before these checks today.
pub fn decide_popup_request(
    method: &str,
    granted: &[String],
    pinned_address: Option<&str>,
) -> DpermPopupDecision {
    if is_connect_method(method) {
        if granted.is_empty() {
            return DpermPopupDecision::Consent;
        }
        return DpermPopupDecision::Respond(connect_payload(method, granted.to_vec()));
    }
    let Some(first) = granted.first() else {
        return DpermPopupDecision::Reject(DpermRejectReason::NotConnected);
    };
    if let Some(pinned) = pinned_address {
        if !pinned.eq_ignore_ascii_case(first) {
            return DpermPopupDecision::Reject(DpermRejectReason::StaleAuthorizedAddress);
        }
    }
    DpermPopupDecision::ForwardToSigning
}

/// The connect result: accounts, or the EIP-2255 permission shape for
/// `wallet_requestPermissions`.
fn connect_payload(method: &str, granted: Vec<String>) -> DpermRespondPayload {
    if method == "wallet_requestPermissions" {
        DpermRespondPayload::Permissions { granted: true }
    } else {
        DpermRespondPayload::Accounts { addresses: granted }
    }
}

// ---------------------------------------------------------------------------
// Pure policy — origin security (`wallet-browser-router.ts:78-118`)
// ---------------------------------------------------------------------------

/// A PUBLIC http (non-TLS) origin, where a MITM can inject page script.
/// Loopback / private-LAN / link-local hosts and `.local` are exempt so
/// local/dev dApps (and the on-device test dApp, served over the LAN) still
/// work. An unparseable origin is treated as insecure — the TS `catch` branch,
/// ported verbatim (fail closed).
pub fn is_insecure_public_origin(origin: &str) -> bool {
    match parse_origin(origin) {
        None => true,
        Some((scheme, host, _)) => scheme == "http" && !is_loopback_or_private_host(&host),
    }
}

/// A loopback / private-LAN / link-local host — the ONLY http origins exempt
/// from the insecure-signing block.
///
/// Matches EXACT IPs only (a fully-anchored dotted quad or IPv6), never a
/// hostname that merely starts with those digits: `10.0.0.1.evil.com` is a
/// public FQDN an attacker can register (DNS labels may start with a digit)
/// and MUST NOT be exempt (invariant ③).
fn is_loopback_or_private_host(host: &str) -> bool {
    // URL.hostname returns IPv6 in bracketed form ("[::1]") — strip the
    // brackets (one leading, one trailing, as the TS regex does).
    let lower = host.to_ascii_lowercase();
    let h = lower.strip_prefix('[').unwrap_or(&lower);
    let h = h.strip_suffix(']').unwrap_or(h);

    if h == "localhost" || h.ends_with(".local") {
        return true;
    }
    // IPv6
    if h == "::1" {
        return true;
    }
    // fc00::/7 unique-local — `^f[cd][0-9a-f]{2}:` (h is already lowercase).
    let bytes = h.as_bytes();
    if bytes.len() >= 5
        && bytes[0] == b'f'
        && (bytes[1] == b'c' || bytes[1] == b'd')
        && bytes[2].is_ascii_hexdigit()
        && bytes[3].is_ascii_hexdigit()
        && bytes[4] == b':'
    {
        return true;
    }
    if h.starts_with("fe80:") {
        return true; // link-local
    }
    // IPv4 — must be a COMPLETE dotted quad, each octet 0–255. Anything with
    // more (or fewer) labels is a hostname, not an IP.
    let parts: Vec<&str> = h.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    let mut octets = [0u16; 4];
    for (slot, part) in octets.iter_mut().zip(&parts) {
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return false;
        }
        *slot = part.parse::<u16>().unwrap_or(999);
    }
    if octets.iter().any(|&n| n > 255) {
        return false;
    }
    let (a, b) = (octets[0], octets[1]);
    a == 127 // loopback
        || a == 10 // private
        || (a == 192 && b == 168) // private
        || (a == 172 && (16..=31).contains(&b)) // private
        || (a == 169 && b == 254) // link-local
}

// ---------------------------------------------------------------------------
// Pure policy — URL → origin (`browser.tsx:66-73` `originOf`)
// ---------------------------------------------------------------------------

/// The origin of a URL, normalized the way `new URL()` normalizes it:
/// lowercased scheme + host, default ports (80/http, 443/https) stripped,
/// userinfo dropped. `None` where the TS returned `''` (unparseable) — plus a
/// fail-closed deviation: non-http(s) schemes never become an origin (the
/// browser only ever loads http(s); `coerceBrowserUrl` guarantees it
/// upstream).
pub fn origin_of(url: &str) -> Option<String> {
    let (scheme, host, port) = parse_origin(url)?;
    if scheme != "http" && scheme != "https" {
        return None;
    }
    let default_port = if scheme == "http" { 80 } else { 443 };
    match port {
        Some(port) if port != default_port => Some(format!("{scheme}://{host}:{port}")),
        _ => Some(format!("{scheme}://{host}")),
    }
}

/// Minimal `new URL(x).{protocol, hostname, port}` for the shapes this
/// machine meets (origins reported by the native WebView, and page URLs).
/// Returns `None` exactly where the URL constructor throws for those shapes.
fn parse_origin(value: &str) -> Option<(String, String, Option<u32>)> {
    let (scheme, rest) = value.split_once("://")?;
    let mut scheme_bytes = scheme.bytes();
    let first = scheme_bytes.next()?;
    if !first.is_ascii_alphabetic()
        || !scheme_bytes.all(|b| b.is_ascii_alphanumeric() || b == b'+' || b == b'-' || b == b'.')
    {
        return None;
    }
    let scheme = scheme.to_ascii_lowercase();

    // Authority: everything up to the first path/query/fragment delimiter.
    let end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = rest.get(..end)?;
    // Drop userinfo (`user:pass@host`).
    let host_port = authority
        .rsplit_once('@')
        .map(|(_, host)| host)
        .unwrap_or(authority);

    let (host_raw, port_raw) = if host_port.starts_with('[') {
        // Bracketed IPv6 — the hostname keeps its brackets, as URL.hostname
        // does.
        let close = host_port.find(']')?;
        let host = host_port.get(..=close)?;
        let after = host_port.get(close + 1..)?;
        if after.is_empty() {
            (host, None)
        } else {
            (host, Some(after.strip_prefix(':')?))
        }
    } else {
        match host_port.split_once(':') {
            Some((host, port)) => (host, Some(port)),
            None => (host_port, None),
        }
    };

    if host_raw.is_empty()
        || host_raw
            .chars()
            .any(|c| c.is_whitespace() || c.is_control())
    {
        return None;
    }
    let port = match port_raw {
        None | Some("") => None, // "http://a.com:" — URL drops the empty port
        Some(digits) if digits.bytes().all(|b| b.is_ascii_digit()) => {
            let port = digits.parse::<u32>().ok()?;
            if port > 65_535 {
                return None; // URL constructor throws
            }
            Some(port)
        }
        Some(_) => return None, // non-numeric port → URL constructor throws
    };
    Some((scheme, host_raw.to_ascii_lowercase(), port))
}

// ---------------------------------------------------------------------------
// Command plumbing
// ---------------------------------------------------------------------------

/// Issue the operations, then render.
fn finish(ops: Vec<DpermOperation>) -> Command<DpermEffect, Event> {
    let mut commands: Vec<Command<DpermEffect, Event>> = ops
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation).then_send(|_result| Event::ShellCompleted)
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for DpermEffect {
    type Op = DpermOperation;
    fn into_shell(self) -> Option<crux_core::Request<DpermOperation>> {
        match self {
            DpermEffect::Render(_) => None,
            DpermEffect::Shell(request) => Some(request),
        }
    }
}
