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
//!   shells import from here so the rules exist once, and the extension's
//!   [`granted_to_signed_in`] (a grant is answered only for the signed-in
//!   account, spec 086, issue 315);
//! - **the web request window's entries** — a one-shot window with no tab, no
//!   document and no navigation, which owns its own grant I/O and its own
//!   transport: it asks [`Event::PopupRequest`] for a verdict, states
//!   [`Event::PopupApproved`] when the person presses Connect,
//!   [`Event::PopupAccountSwitch`] when the wallet changes account, and reads
//!   [`settle_on_close`] for the code a still-pending request is settled with;
//! - **the origin-security helpers** — [`is_insecure_public_origin`] with its
//!   fully-anchored IP exemptions, [`is_secure_context`] (spec 088),
//!   [`offers_wallet`] (who is offered the wallet at all, debug mode included,
//!   spec 091) with [`private_host_js`], its host rule written as JavaScript,
//!   and [`origin_of`]. All are imported by `dapp_browser` / `dapp_rpc`; none
//!   is re-implemented there.
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
//! **One spelling toward dApps (spec 082 RG10, L-D6).** Every address this
//! machine writes into a grant or answers a site with goes through
//! [`dapp_spelling`] (EIP-55), so the connect answer and a later
//! `accountsChanged` name the same account the same way. Comparisons stay
//! case-insensitive ([`resolve_granted`] and [`granted_to_signed_in`]: the
//! latter's JavaScript twin in the extension's worker cannot checksum).
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
    /// A signature asked for by a PUBLIC plain-http origin
    /// ([`is_insecure_public_origin`]), where anyone on the path can rewrite
    /// the page that asks. The in-app browsers' sign gate (`dapp_browser`)
    /// refused it from the start; the request window did not (spec 089).
    InsecureOrigin,
}

impl DpermRejectReason {
    pub fn code(self) -> u32 {
        match self {
            Self::NotConnected | Self::StaleAuthorizedAddress | Self::InsecureOrigin => {
                CODE_UNAUTHORIZED
            }
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
    /// [`granted_to_signed_in`]'s answer for this origin — exposed so the
    /// window never re-derives which account a site may see itself.
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
        /// The account signed in on this device right now (`SessionView`'s
        /// address, read once the session has settled). `None` — or empty —
        /// when nobody is. A grant for any other account is not answered
        /// ([`granted_to_signed_in`], spec 086, issue 315).
        signed_in: Option<String>,
        /// `peer.request.address`, the address the request pins itself to.
        /// The shell maps the TS empty string to `None`. Ignored when
        /// `params_json` is given: the core then reads the address itself.
        pinned_address: Option<String>,
        /// The asking page's origin — the browser's fact, never the page's
        /// claim. A signature asked for by a public plain-http origin is
        /// refused ([`DpermRejectReason::InsecureOrigin`]), as every in-app
        /// browser refuses it (spec 089). `None`: no origin rule is asked.
        origin: Option<String>,
        /// The request's params, as JSON. When given, the address the request
        /// names is read from them by [`super::dapp_rpc::requested_address`] —
        /// a transaction's `from`, `personal_sign`'s second param, typed
        /// data's account — the rule every in-app browser's sign gate reads
        /// (spec 089). The window's own by-shape guess never saw a
        /// transaction's `from`, so a transaction naming another account was
        /// signed by the granted one instead of refused.
        params_json: Option<String>,
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
                signed_in,
                pinned_address,
                origin,
                params_json,
            } => popup_request(
                model,
                &method,
                grant.as_ref(),
                signed_in.as_deref(),
                PopupFacts {
                    pinned_address: pinned_address.as_deref(),
                    origin: origin.as_deref(),
                    params_json: params_json.as_deref(),
                },
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
/// re-decided here: [`granted_to_signed_in`] says what this origin may see,
/// [`decide_popup_request`] says what to do about it. The rules the window
/// exists to enforce are therefore stated once, in Rust:
///
/// - a never-connected origin gets no address — 4100, not a forward;
/// - a grant for an account that is not the signed-in one is no grant: the
///   site is asked again, and the person connects the account they are in
///   (spec 086, issue 315);
/// - the forward is pinned to the GRANT's address, never re-pointed at some
///   other account (invariant ⑨) — which, by the rule above, is the signed-in
///   account;
/// - a request pinning some other address is refused 4100 — never a silent
///   swap of the signer for one the dApp did not ask for; the address a
///   request names is read by the core's own rule when the params come with
///   the question (spec 089);
/// - a signature asked for by a public plain-http origin is refused 4100,
///   before anything else is asked of it — the in-app browsers' gate.
fn popup_request(
    model: &mut Model,
    method: &str,
    grant: Option<&DpermGrant>,
    signed_in: Option<&str>,
    facts: PopupFacts<'_>,
) -> Command<DpermEffect, Event> {
    // The window answers in the one spelling too, whatever case the stored
    // grant was written in before 082 (RG10). Only the case changes: the match
    // above is case-insensitive and stays so.
    let granted: Vec<String> = granted_to_signed_in(grant, signed_in)
        .iter()
        .map(|address| dapp_spelling(address))
        .collect();
    let outcome = if let Some(reason) = popup_origin_refusal(method, facts.origin) {
        DpermPopupOutcome::Reject {
            code: reason.code(),
            reason,
        }
    } else {
        popup_outcome(method, &granted, facts.pinned(method).as_deref())
    };
    model.popup = Some(DpermPopupView { outcome, granted });
    render()
}

/// What the window states about one request beyond the grant: the address it
/// pinned (its own guess, the pre-089 shape), the origin, and the params.
#[derive(Clone, Copy, Debug, Default)]
struct PopupFacts<'a> {
    pinned_address: Option<&'a str>,
    origin: Option<&'a str>,
    params_json: Option<&'a str>,
}

impl PopupFacts<'_> {
    /// The address the request names: read from the params by
    /// [`super::dapp_rpc::requested_address`] when they were given (an
    /// unparseable list names nobody), else the window's own pin.
    fn pinned(&self, method: &str) -> Option<String> {
        match self.params_json {
            Some(raw) => serde_json::from_str::<serde_json::Value>(raw)
                .ok()
                .and_then(|params| super::dapp_rpc::requested_address(method, &params)),
            None => self.pinned_address.map(str::to_owned),
        }
    }
}

/// The origin rule of the window's question (spec 089): a signing method
/// asked for by a public plain-http origin is refused, before the grant is
/// looked at — the order `dapp_browser`'s sign gate keeps. Connecting is not
/// refused here, as it is not in an in-app browser.
#[must_use]
pub fn popup_origin_refusal(method: &str, origin: Option<&str>) -> Option<DpermRejectReason> {
    let origin = origin?;
    (is_signing_method(method) && is_insecure_public_origin(origin))
        .then_some(DpermRejectReason::InsecureOrigin)
}

/// [`decide_popup_request`]'s decision, on the wire.
fn popup_outcome(method: &str, granted: &[String], pinned: Option<&str>) -> DpermPopupOutcome {
    match decide_popup_request(method, granted, pinned) {
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
    }
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
    // The grant, the audit row and the answer all carry the one spelling a
    // dApp will see from here on (RG10).
    let address = dapp_spelling(address);
    let address = address.as_str();
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
            address: dapp_spelling(active_address),
            // The chain the site CONNECTED on: an audit fact a switch of
            // account must not rewrite.
            chain_id: grant.chain_id,
            granted_at_ms: now_ms,
        },
    }])
}

// ---------------------------------------------------------------------------
// Pure policy — the address spelling a dApp sees (spec 082 RG10)
// ---------------------------------------------------------------------------

/// The spelling of `address` every dApp is given: EIP-55
/// ([`crate::primitives::checksum_address`], `0x`-prefixed). Input that is
/// not an address is returned unchanged: this changes how an account is
/// spelled, never which account is named, and it must not invent one.
///
/// Before 082 the connect answer was checksummed while the page's
/// `accountsChanged` arrived lower-cased, so a strict dApp saw one account as
/// two (L-D6).
pub fn dapp_spelling(address: &str) -> String {
    crate::primitives::checksum_address(address).unwrap_or_else(|_| address.to_owned())
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

/// The accounts the EXTENSION may tell an origin: its grant, while — and only
/// while — the grant's account is the one signed in (spec 086, issue 315).
///
/// - No grant → `[]`.
/// - Nobody signed in (`None`, or empty) → `[]`: signing out of a wallet ends
///   what every site may learn of it. The grants themselves stay (signing back
///   in lines them back up, `session.rs`); they are only not answered.
/// - Grant for the signed-in account → `[address]` (any case matches; the
///   grant is returned as stored).
/// - Grant for ANY other account → `[]`, even one the device still holds. The
///   site is asked again, and the person connects the account they are in.
///
/// Why the extension asks this and not [`resolve_granted`]: an in-app browser
/// re-pins every grant to the active account inside its own machine, on every
/// change and on its first load (`dapp_browser::reconcile_grants`), so for it
/// "held" and "signed in" coincide. The extension's two answerers — the
/// service worker and the request surface — read grants another document
/// wrote, and that document's re-pin is a moment that can be missed (a sign-in
/// in a fresh document, a side panel open across a switch). A grant answered
/// for any HELD account then handed a site the previous account (issue 315). Asking
/// "is this the signed-in account?" holds whatever was missed.
///
/// There is no cold read here: the worker reads a snapshot the wallet keeps
/// in persistent storage (absent = nobody is signed in, `ext_cache`'s own
/// RemoveSnapshot on logout), and the surface asks once its session has
/// settled. The service worker's twin (`resolveGrantedAccounts` in
/// `extension/lib/protocol.js`) is pinned to this function by
/// `instant.test.ts`.
pub fn granted_to_signed_in(grant: Option<&DpermGrant>, signed_in: Option<&str>) -> Vec<String> {
    match (grant, signed_in) {
        (Some(grant), Some(active))
            if !active.is_empty() && grant.address.eq_ignore_ascii_case(active) =>
        {
            vec![grant.address.clone()]
        }
        _ => Vec::new(),
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

/// Whether a page at `origin` is a secure context (spec 088 FR-004): `https`,
/// or `http` on this device's own loopback — the web platform's "secure
/// context", the same pair the Trusted Signer page is held to.
///
/// Narrower than [`is_insecure_public_origin`] on purpose: a private-LAN
/// `http` page is reachable by anyone on the same network, so outside debug
/// mode it may load but gets no provider ([`offers_wallet`]). A dApp under
/// development is served on loopback (`adb reverse`, the simulator), over
/// https, or — with debug mode on — over http on the developer's own network.
///
/// Loopback is EXACT: `localhost`, a `*.localhost` name (RFC 6761 reserves
/// them for loopback), a full `127.x.y.z` dotted quad, or `[::1]` — never
/// `127.0.0.1.evil.com`.
pub fn is_secure_context(origin: &str) -> bool {
    match parse_origin(origin) {
        Some((scheme, _, _)) if scheme == "https" => true,
        Some((scheme, host, _)) if scheme == "http" => is_loopback_host(&host),
        _ => false,
    }
}

/// Whether a page at `origin` is offered the wallet (spec 091) — the ONE
/// answer the in-app browsers' page gate ([`super::dapp_browser`]) and the
/// script they inject ([`super::dapp_rpc::provider_script`]) both follow.
///
/// - Always: a secure context ([`is_secure_context`]).
/// - With debug mode on (Settings → About in a developer build only,
///   revealed by tapping the version [`crate::prefs::VERSION_TAPS`] times;
///   a store build's mode is always off, `crate::prefs::debug_mode`): also
///   plain `http` on this device's own network — RFC 1918 and link-local
///   IPv4, IPv6 unique-local and link-local, `.local` — matched exactly by
///   [`is_loopback_or_private_host`], never by a name that only starts with
///   those digits (`10.0.0.1.evil.com`).
/// - Never: public `http`, debug mode or not; any other scheme; an origin
///   that does not parse.
pub fn offers_wallet(origin: &str, debug_mode: bool) -> bool {
    is_secure_context(origin)
        || (debug_mode
            && matches!(
                parse_origin(origin),
                Some((scheme, host, _)) if scheme == "http" && is_loopback_or_private_host(&host)
            ))
}

/// `localhost`, `*.localhost`, `127.0.0.0/8` as a full dotted quad, `[::1]`.
fn is_loopback_host(host: &str) -> bool {
    let h = host.to_ascii_lowercase();
    h == "localhost"
        || h.ends_with(".localhost")
        || h == "[::1]"
        || dotted_quad(&h).is_some_and(|[a, ..]| a == 127)
}

// The private-network rule as data (spec 091): [`is_loopback_or_private_host`]
// reads these tables, and [`private_host_js`] writes them into the injected
// script, so the page gate and the script decide from the same numbers.

/// Hosts named exactly (lower-case, brackets stripped).
const PRIVATE_NAMES: [&str; 2] = ["localhost", "::1"];
/// Name suffixes that resolve only on the local link (mDNS).
const PRIVATE_SUFFIXES: [&str; 1] = [".local"];
/// IPv6: the first group, written as exactly four hex digits and a colon, in
/// one of these ranges — `fc00::/7` unique-local and `fe80:` link-local.
const PRIVATE_V6_FIRST_GROUP: [(u16, u16); 2] = [(0xfc00, 0xfdff), (0xfe80, 0xfe80)];
/// IPv4, a complete dotted quad: `(first octet, second octet from, to)`.
const PRIVATE_V4: [(u16, u16, u16); 5] = [
    (127, 0, 255),   // loopback
    (10, 0, 255),    // private
    (192, 168, 168), // private
    (172, 16, 31),   // private
    (169, 254, 254), // link-local
];

/// A loopback / private-LAN / link-local host — the ONLY http origins exempt
/// from the insecure-signing block, and the only ones debug mode offers the
/// wallet to.
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

    if PRIVATE_NAMES.contains(&h) || PRIVATE_SUFFIXES.iter().any(|suffix| h.ends_with(suffix)) {
        return true;
    }
    if let Some(group) = first_v6_group(h) {
        return PRIVATE_V6_FIRST_GROUP
            .iter()
            .any(|&(from, to)| (from..=to).contains(&group));
    }
    // IPv4 — must be a COMPLETE dotted quad, each octet 0–255. Anything with
    // more (or fewer) labels is a hostname, not an IP.
    dotted_quad(h).is_some_and(|[a, b, ..]| {
        PRIVATE_V4
            .iter()
            .any(|&(first, from, to)| a == first && (from..=to).contains(&b))
    })
}

/// `xxxx:…` — the first group of an IPv6 address when it is written as
/// exactly four hex digits.
fn first_v6_group(h: &str) -> Option<u16> {
    let (group, _) = h.split_once(':')?;
    if group.len() != 4 || !group.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    u16::from_str_radix(group, 16).ok()
}

/// A complete dotted quad: four decimal labels of one to three digits, each
/// 0–255.
fn dotted_quad(h: &str) -> Option<[u16; 4]> {
    let mut octets = [0u16; 4];
    let mut parts = h.split('.');
    for slot in &mut octets {
        let part = parts.next()?;
        if part.is_empty() || part.len() > 3 || !part.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        *slot = part.parse::<u16>().ok().filter(|n| *n <= 255)?;
    }
    parts.next().is_none().then_some(octets)
}

/// [`is_loopback_or_private_host`] as a JavaScript function expression of one
/// argument, a `location.hostname` (spec 091). It is written out of the same
/// tables, so the script [`super::dapp_rpc::provider_script`] injects in
/// debug mode and the page gate cannot disagree about a host; the web suite
/// runs it against this rule on a table of hosts (`core-table.test.ts`).
pub fn private_host_js() -> String {
    PRIVATE_HOST_JS
        .replace("__NAMES__", &serde_json::json!(PRIVATE_NAMES).to_string())
        .replace(
            "__SUFFIXES__",
            &serde_json::json!(PRIVATE_SUFFIXES).to_string(),
        )
        .replace(
            "__V6__",
            &serde_json::json!(PRIVATE_V6_FIRST_GROUP).to_string(),
        )
        .replace("__V4__", &serde_json::json!(PRIVATE_V4).to_string())
}

/// What [`private_host_js`] writes, step for step the Rust above; the
/// `__TABLE__` markers are the tables.
const PRIVATE_HOST_JS: &str = r#"function (host) {
		let h = String(host).toLowerCase();
		if (h.startsWith('[')) h = h.slice(1);
		if (h.endsWith(']')) h = h.slice(0, -1);
		if (__NAMES__.includes(h) || __SUFFIXES__.some((s) => h.endsWith(s))) return true;
		const v6 = /^([0-9a-f]{4}):/.exec(h);
		if (v6) {
			const group = parseInt(v6[1], 16);
			return __V6__.some(([from, to]) => group >= from && group <= to);
		}
		const parts = h.split('.');
		if (parts.length !== 4) return false;
		if (!parts.every((p) => /^[0-9]{1,3}$/.test(p) && Number(p) <= 255)) return false;
		const a = Number(parts[0]);
		const b = Number(parts[1]);
		return __V4__.some(([first, from, to]) => a === first && b >= from && b <= to);
	}"#;

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
