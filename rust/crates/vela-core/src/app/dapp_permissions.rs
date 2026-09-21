//! Machine — per-origin dApp permissions and the in-app browser consent flow
//! (spec `017-crux-wallet-state-complete`, inventory `### dapp_permissions (P2)`).
//!
//! ```text
//! ProviderRequest ─► grant mirror ──unknown──► ReadGrant ─► park request
//!        │ known                                    │ GrantRead
//!        ▼                                          ▼
//!  decide (pure) ─► Respond / Reject / consent sheet / ForwardToSigning
//!        │approve                        │navigation / close
//!        ▼                               ▼
//!  WriteGrant + audit row + responses    settle EVERYTHING with 4900 — never 4001
//!  + accountsChanged + chainChanged
//! ```
//!
//! What converges here: today the grant-check orchestration exists three times
//! (`browser.tsx` `onProviderRequest`, `web-request.tsx`'s popup flow, the
//! Safari extension background) sharing pure functions but not orchestration.
//! This core owns the one browser decision path ([`decide_browser_request`]),
//! the popup entry's drifted variant ([`decide_popup_request`]) — kept separate
//! and explicit so neither entry's rules silently adopt the other's — and the
//! single method-set source of truth ([`CONNECT_METHODS`], [`SIGNING_METHODS`]).
//!
//! Ported line by line from:
//!
//! - `src/services/dapp-permissions.ts` — grant store semantics,
//!   `resolveGranted` / `shouldDropGrant` including the load-bearing rule:
//!   NEVER drop a grant on a cold/empty account read, or a transient empty
//!   state logs the user out of every open dApp.
//! - `src/services/wallet-browser-router.ts` — `classifyBrowserRequest`,
//!   `decideBrowserRequest`, the insecure-public-http signing block with its
//!   fully-anchored IP exemptions (`10.0.0.1.evil.com` is a public FQDN an
//!   attacker can register and MUST NOT be exempt).
//! - `src/app/browser.tsx:51-360` — consent queue (same-origin coalesce,
//!   cross-origin 4001), approve/reject, `NAV_SETTLE_ERROR` (4900)
//!   settle-on-navigation, the approve-vs-navigation guard, disconnect,
//!   account switch re-pinning, `chainChanged` gating, `hexChainId`.
//! - `src/app/web-request.tsx:57-250` — the popup entry's grant checks
//!   (connect / not-connected 4100 / pinned-address 4100 / forward).
//! - `src/services/webview-transport.ts:49-133` — the settle vocabulary
//!   (4900 on navigate/close) and the iframe gate on forwarded traffic. The
//!   transport instance is a live object: it never crosses the JSON boundary —
//!   the core only names [`DpermOperation::SettleForwarded`] and the shell
//!   applies it to the transport's pending set, which also keeps the
//!   exactly-one-response-per-id gate where it lives today (invariant ⑩).
//!
//! Quirks kept verbatim (all doc-marked below): `grantedAt` never participates
//! in any decision (grants have no TTL — open question in the inventory); a
//! request-path grant drop does not refresh the connected chip until the next
//! navigation; the consent sheet only opens for a not-yet-granted origin, so
//! the audit row fires once per connection, not on revisit.
//!
//! Fail-closed deviations from JS (each marked at its site): requests still
//! parked when a navigation lands are settled with the same 4900 the rest of
//! the in-flight work gets (in TS their block-scoped continuations answered a
//! dead document — an unobservable race, not a rule); non-http(s) URLs never
//! become an origin; an unparseable origin is treated as insecure, exactly as
//! the TS `catch` does.

use std::collections::BTreeMap;

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

/// `CONNECT_METHODS` — the only methods that may open the consent sheet.
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

/// The insecure-origin gate's set: [`SIGNING_METHODS`] AND anything else
/// `dapp_rpc` calls a signature (spec 070 — the two definitions had drifted:
/// `eth_signTypedData_v2` escaped this gate yet reached a sheet).
pub fn is_signing_method(method: &str) -> bool {
    SIGNING_METHODS.contains(&method) || super::dapp_rpc::is_signing_method(method)
}

// ---------------------------------------------------------------------------
// EIP-1193 error codes — load-bearing: 4900 vs 4001 is invariant ⑤.
// ---------------------------------------------------------------------------

pub const CODE_UNAUTHORIZED: u32 = 4100;
pub const CODE_USER_REJECTED: u32 = 4001;
/// "Unknown-pending": the request may have landed — a dApp must NOT treat it
/// as safe to retry (which it does with 4001).
pub const CODE_UNKNOWN_PENDING: u32 = 4900;

/// Why a request was refused. Semantic — the shell owns the words; the code
/// is the core's because the code IS the behavior contract with the dApp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermRejectReason {
    /// A cross-origin iframe asked for accounts/consent/signing (invariant ①).
    UnauthorizedFrame,
    /// Connect with no wallet account available (`browser.tsx` decide: 4001).
    NoAccountAvailable,
    /// A second ORIGIN collided with the open consent sheet — a page can't
    /// queue two connect sheets (invariant ④).
    ConsentBusy,
    /// Signing on a public non-TLS origin (invariant ③).
    InsecureOrigin,
    /// The user pressed reject on the consent sheet.
    UserRejected,
    /// The document navigated away — `NAV_SETTLE_ERROR`, always 4900
    /// (invariant ⑤).
    NavigatedAway,
    /// The browser closed with the answer still pending — 4900 for the same
    /// double-spend reason (`webview-transport.ts:77-79`).
    BrowserClosed,
    /// Popup entry: a non-connect request from a never-connected origin
    /// (`web-request.tsx:187`).
    NotConnected,
    /// Popup entry: the request pinned an address that is no longer the
    /// granted one (`web-request.tsx:190-192`).
    StaleAuthorizedAddress,
}

impl DpermRejectReason {
    pub fn code(self) -> u32 {
        match self {
            Self::UnauthorizedFrame
            | Self::InsecureOrigin
            | Self::NotConnected
            | Self::StaleAuthorizedAddress => CODE_UNAUTHORIZED,
            Self::NoAccountAvailable | Self::ConsentBusy | Self::UserRejected => CODE_USER_REJECTED,
            Self::NavigatedAway | Self::BrowserClosed => CODE_UNKNOWN_PENDING,
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
/// (`granted ? [{parentCapability:'eth_accounts'}] : []`); `Error` is
/// `{code, message}` with the words looked up from the reason.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermRespondPayload {
    Accounts {
        addresses: Vec<String>,
    },
    Permissions {
        granted: bool,
    },
    Error {
        code: u32,
        reason: DpermRejectReason,
    },
}

/// An EIP-1193 event pushed to the page via the bridge.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum DpermPageEvent {
    AccountsChanged { addresses: Vec<String> },
    ChainChanged { chain_id_hex: String },
    Disconnect,
}

/// One request coalesced into the open consent sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermQueuedRequest {
    pub id: String,
    pub method: String,
}

/// The web-popup entry's verdict, on the wire.
///
/// A projection of [`DpermPopupDecision`] — [`decide_popup_request`] keeps
/// exactly the semantics it was ported with; this only gives the answer a
/// serialisable shape so the popup window can ASK for it. `ForwardToSigning`
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
    /// Open the popup's connect consent.
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
    /// [`resolve_granted`]'s answer for this origin — exposed so the popup
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
    /// Read `vela.perm.<origin>`. A parse/storage error answers `None`,
    /// exactly as `getGrant`'s catch does.
    ReadGrant { origin: String },
    /// Persist `vela.perm.<origin>` — best-effort; the shell swallows storage
    /// errors (`setGrant`).
    WriteGrant { grant: DpermGrant },
    /// Remove `vela.perm.<origin>` — best-effort (`revokeGrant`).
    RemoveGrant { origin: String },
    /// Answer one provider request via the bridge. Local (connect/state)
    /// responses bypass the transport's pending-id gate, as today.
    Respond {
        id: String,
        payload: DpermRespondPayload,
    },
    /// Push an EIP-1193 event to the page.
    EmitEvent { event: DpermPageEvent },
    /// Settle every request the transport still holds pending with this
    /// terminal error (`transport.settlePending`). The transport is a live
    /// object the core never holds — the exactly-one-response-per-id gate
    /// stays there (invariant ⑩); the core owns WHICH code is used.
    SettleForwarded {
        code: u32,
        reason: DpermRejectReason,
    },
    /// Write the "Connected to <app>" audit row (`buildConnectionRecord` +
    /// `saveTransaction` — the shell derives the display host and stamps its
    /// own clock, both presentation).
    SaveConnectionRecord {
        address: String,
        chain_id: u32,
        origin: String,
    },
    /// Hand to the WebViewTransport → signing pipeline (the `sign_request`
    /// machine). `params_json` is the raw JSON-RPC params array, verbatim and
    /// untrusted — this core never interprets it.
    ForwardToSigning {
        id: String,
        method: String,
        params_json: String,
        origin: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DpermShellResult"))]
pub enum DpermShellResult {
    GrantRead {
        origin: String,
        grant: Option<DpermGrant>,
    },
    /// Every other operation is fire-and-forget from the core's view.
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
    /// A provider request bubbled up by the WebView. `origin` MUST be the
    /// native-observed committed origin (iOS `frameInfo.securityOrigin`,
    /// Android `sourceOrigin`) — never a value the page put in the message
    /// body (`webview-transport.ts:100-107`).
    ProviderRequest {
        id: String,
        method: String,
        params_json: String,
        origin: String,
        is_main_frame: bool,
    },
    /// The user approved the consent sheet. `now_ms` stamps the grant — the
    /// core owns no clock.
    ConsentApproved { now_ms: f64 },
    /// The user rejected the consent sheet.
    ConsentRejected,
    /// A fresh document load started (`onNavigationChange` with `loading`;
    /// SPA pushState does NOT fire this, so same-page route changes keep
    /// their pending state — ported verbatim).
    NavigationStarted { url: String },
    /// The browser screen closed.
    BrowserClosed,
    /// ALL wallet addresses (not just the active one) — a grant is pinned to
    /// the address it was made for, so grant judgments need the full set
    /// (`browser.tsx:174-178`). `None` while loading → cold-load safe.
    AccountsUpdated { addresses: Option<Vec<String>> },
    /// The wallet's active account changed (initial load included). `now_ms`
    /// re-stamps the grant when a connected origin is re-pinned.
    AccountSwitched { address: String, now_ms: f64 },
    /// The global chain changed. The shell must seed the initial chain with
    /// this before the first consent can be approved.
    ChainChanged { chain_id: u32 },
    /// Disconnect. `None` = the current origin (the browser chip); a named
    /// origin revokes silently (no page events — the page for that origin is
    /// not in front of us).
    RevokeRequested { origin: Option<String> },
    /// The web-popup entry (`web-request.tsx:169-193`) asks for one request's
    /// verdict.
    ///
    /// PURE, on purpose: it requests no shell operation, touches none of the
    /// browser state above, and only publishes its answer on the view — the
    /// `validate_pay_query` pattern (`payment_request.rs`), because the popup
    /// is a one-shot window that owns its own grant I/O and its own transport
    /// and has no document to emit page events into. Without it
    /// [`decide_popup_request`] is authored, tested and exported but never
    /// executed anywhere, which is worse than not having it: it reads as the
    /// source of truth for rules the shell is actually re-implementing.
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
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: DpermShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq)]
struct Consent {
    origin: String,
    requests: Vec<DpermQueuedRequest>,
}

#[derive(Clone, Debug, PartialEq)]
struct PendingRequest {
    id: String,
    method: String,
    params_json: String,
    origin: String,
    is_main_frame: bool,
}

#[derive(Default)]
pub struct Model {
    /// Mirror of the KV grant store; an entry present means the state is
    /// KNOWN (`Some` = granted, `None` = known-absent). This core is the only
    /// in-session writer, so mirror and store cannot drift.
    grants: BTreeMap<String, Option<DpermGrant>>,
    /// Origins with a `ReadGrant` in flight whose answer is still expected. A
    /// write/revoke removes the origin here so a stale store snapshot can
    /// never clobber a fresher mirror entry.
    reads_in_flight: Vec<String>,
    /// Main-frame requests parked on a grant read.
    parked: Vec<PendingRequest>,
    consent: Option<Consent>,
    /// The committed main-frame origin; empty until the first navigation.
    current_origin: String,
    /// A navigation-triggered read refreshes the connected chip when it
    /// lands; request-path reads deliberately do not (ported verbatim — the
    /// chip refreshes on navigation, not on every request).
    nav_refresh: Option<String>,
    connected_addr: Option<String>,
    wallet_addresses: Option<Vec<String>>,
    active_address: Option<String>,
    chain_id: u32,
    /// Bumped on navigation/close; a result carrying an older attempt belongs
    /// to a torn-down document and is dropped.
    attempt: u64,
    /// The last [`Event::PopupRequest`] verdict. Separate from every field
    /// above: the popup is a different entry with a different window, and its
    /// answer must never be confused with the in-app browser's state.
    popup: Option<DpermPopupView>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermConsentView {
    pub origin: String,
    /// One entry per coalesced request — the sheet shows the origin once,
    /// however many times the page asked.
    pub methods: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DpermView {
    pub consent: Option<DpermConsentView>,
    /// The connected chip. `None` = disconnected view.
    pub connected_address: Option<String>,
    pub current_origin: Option<String>,
    /// The answer to the last [`Event::PopupRequest`]. `None` on every core
    /// that has never been asked one — the in-app browser never sets it.
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
            Event::ProviderRequest {
                id,
                method,
                params_json,
                origin,
                is_main_frame,
            } => provider_request(
                model,
                PendingRequest {
                    id,
                    method,
                    params_json,
                    origin,
                    is_main_frame,
                },
            ),
            Event::ConsentApproved { now_ms } => consent_approved(model, now_ms),
            Event::ConsentRejected => consent_rejected(model),
            Event::NavigationStarted { url } => navigation_started(model, &url),
            Event::BrowserClosed => browser_closed(model),
            Event::AccountsUpdated { addresses } => {
                model.wallet_addresses = addresses;
                render()
            }
            Event::AccountSwitched { address, now_ms } => account_switched(model, address, now_ms),
            Event::ChainChanged { chain_id } => chain_changed(model, chain_id),
            Event::RevokeRequested { origin } => revoke_requested(model, origin),
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
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> DpermView {
        DpermView {
            consent: model.consent.as_ref().map(|consent| DpermConsentView {
                origin: consent.origin.clone(),
                methods: consent
                    .requests
                    .iter()
                    .map(|request| request.method.clone())
                    .collect(),
            }),
            connected_address: model.connected_addr.clone(),
            current_origin: if model.current_origin.is_empty() {
                None
            } else {
                Some(model.current_origin.clone())
            },
            popup: model.popup.clone(),
        }
    }
}

// ---------------------------------------------------------------------------
// Update handlers
// ---------------------------------------------------------------------------

fn provider_request(model: &mut Model, request: PendingRequest) -> Command<DpermEffect, Event> {
    // A subframe never reads the store — it gets a disconnected view derived
    // right now (`browser.tsx:192`: grant is only fetched for the main frame).
    if request.is_main_frame && !model.grants.contains_key(&request.origin) {
        let origin = request.origin.clone();
        model.parked.push(request);
        if model.reads_in_flight.iter().any(|o| o == &origin) {
            // A read for this origin is already in flight — one store round
            // trip answers every request parked on it.
            return render();
        }
        model.reads_in_flight.push(origin.clone());
        return finish(model, vec![DpermOperation::ReadGrant { origin }]);
    }
    let mut ops = Vec::new();
    execute_decision(model, request, &mut ops);
    finish(model, ops)
}

fn execute_decision(model: &mut Model, request: PendingRequest, ops: &mut Vec<DpermOperation>) {
    let grant = if request.is_main_frame {
        model.grants.get(&request.origin).cloned().flatten()
    } else {
        None
    };

    // Physically clean up a grant whose account was DELETED from the wallet
    // (`browser.tsx:193-196`). `should_drop_grant` is false on a cold/empty
    // read — dropping there would revoke every dApp on app launch
    // (invariant ②). Ported verbatim: the connected chip is NOT refreshed
    // here; it catches up on the next navigation.
    if should_drop_grant(grant.as_ref(), model.wallet_addresses.as_deref()) {
        model.grants.insert(request.origin.clone(), None);
        ops.push(DpermOperation::RemoveGrant {
            origin: request.origin.clone(),
        });
    }

    let granted = if request.is_main_frame {
        resolve_granted(grant.as_ref(), model.wallet_addresses.as_deref())
    } else {
        Vec::new()
    };

    let decision = decide_browser_request(
        &request.method,
        &request.origin,
        request.is_main_frame,
        &granted,
        model.active_address.is_some(),
        model.consent.as_ref().map(|c| c.origin.as_str()),
    );

    let entry = DpermQueuedRequest {
        id: request.id.clone(),
        method: request.method.clone(),
    };
    match decision {
        DpermDecision::Respond(payload) => ops.push(DpermOperation::Respond {
            id: request.id,
            payload,
        }),
        DpermDecision::Reject(reason) => ops.push(respond_error(request.id, reason)),
        DpermDecision::OpenConsent => {
            model.consent = Some(Consent {
                origin: request.origin,
                requests: vec![entry],
            });
        }
        DpermDecision::MergeConsent => {
            if let Some(consent) = &mut model.consent {
                consent.requests.push(entry);
            }
        }
        DpermDecision::Forward => ops.push(DpermOperation::ForwardToSigning {
            id: request.id,
            method: request.method,
            params_json: request.params_json,
            origin: request.origin,
        }),
    }
}

fn consent_approved(model: &mut Model, now_ms: f64) -> Command<DpermEffect, Event> {
    // A navigation during the user's decision already settled and cleared
    // this sheet with 4900 — a late approve must not respond, must not
    // persist a grant for the OLD origin, and must not push
    // `accountsChanged` into the NEW origin's document (invariant ⑥;
    // `browser.tsx:245-248`). The old async `consentRef.current !== c` guard
    // is this same rule, made synchronous by the single-core update.
    if model.active_address.is_none() {
        return Command::done();
    }
    let Some(consent) = model.consent.take() else {
        return Command::done();
    };
    let Some(active) = model.active_address.clone() else {
        return Command::done();
    };

    let grant = DpermGrant {
        origin: consent.origin.clone(),
        address: active.clone(),
        chain_id: model.chain_id,
        granted_at_ms: now_ms,
    };
    model
        .grants
        .insert(consent.origin.clone(), Some(grant.clone()));
    model.reads_in_flight.retain(|o| o != &consent.origin);
    model.connected_addr = Some(active.clone());

    // Effect order made explicit, matching today's sequence: persist first,
    // audit row ("Connected to <app>" — fires once per connection because the
    // sheet only opens for a not-yet-granted origin), then answer every
    // coalesced request with its method-appropriate result, then announce the
    // connection to the page (the live channel the extension lacks).
    let mut ops = vec![
        DpermOperation::WriteGrant { grant },
        DpermOperation::SaveConnectionRecord {
            address: active.clone(),
            chain_id: model.chain_id,
            origin: consent.origin.clone(),
        },
    ];
    for request in &consent.requests {
        let payload = if request.method == "wallet_requestPermissions" {
            DpermRespondPayload::Permissions { granted: true }
        } else {
            DpermRespondPayload::Accounts {
                addresses: vec![active.clone()],
            }
        };
        ops.push(DpermOperation::Respond {
            id: request.id.clone(),
            payload,
        });
    }
    ops.push(DpermOperation::EmitEvent {
        event: DpermPageEvent::AccountsChanged {
            addresses: vec![active],
        },
    });
    ops.push(DpermOperation::EmitEvent {
        event: DpermPageEvent::ChainChanged {
            chain_id_hex: hex_chain_id(model.chain_id),
        },
    });
    finish(model, ops)
}

fn consent_rejected(model: &mut Model) -> Command<DpermEffect, Event> {
    let Some(consent) = model.consent.take() else {
        return Command::done();
    };
    let ops = consent
        .requests
        .into_iter()
        .map(|request| respond_error(request.id, DpermRejectReason::UserRejected))
        .collect();
    finish(model, ops)
}

fn navigation_started(model: &mut Model, url: &str) -> Command<DpermEffect, Event> {
    // Everything in flight belongs to the document being torn down. Settle it
    // ALL with 4900 unknown-pending — NEVER 4001: a dApp treats an explicit
    // "user rejected" as safe to retry, double-spending a request that may
    // already have landed (invariant ⑤; `browser.tsx:57, 330-341`).
    model.attempt += 1;
    model.reads_in_flight.clear();
    model.nav_refresh = None;

    let mut ops = vec![DpermOperation::SettleForwarded {
        code: CODE_UNKNOWN_PENDING,
        reason: DpermRejectReason::NavigatedAway,
    }];
    settle_local(model, DpermRejectReason::NavigatedAway, &mut ops);

    // Reset the per-origin connection view only when the origin actually
    // changes (a reload keeps the chip; `browser.tsx:351-356`).
    if let Some(new_origin) = origin_of(url) {
        if new_origin != model.current_origin {
            model.current_origin = new_origin.clone();
            if let Some(known) = model.grants.get(&new_origin) {
                model.connected_addr =
                    resolve_granted(known.as_ref(), model.wallet_addresses.as_deref())
                        .into_iter()
                        .next();
            } else {
                model.nav_refresh = Some(new_origin.clone());
                model.reads_in_flight.push(new_origin.clone());
                ops.push(DpermOperation::ReadGrant { origin: new_origin });
            }
        }
    }
    finish(model, ops)
}

fn browser_closed(model: &mut Model) -> Command<DpermEffect, Event> {
    model.attempt += 1;
    model.reads_in_flight.clear();
    model.nav_refresh = None;
    model.current_origin.clear();
    model.connected_addr = None;

    let mut ops = vec![DpermOperation::SettleForwarded {
        code: CODE_UNKNOWN_PENDING,
        reason: DpermRejectReason::BrowserClosed,
    }];
    // Best-effort: the WebView may already be gone, in which case the bridge
    // drops these — but the core's bookkeeping stays honest (one terminal
    // answer per id it ever owned).
    settle_local(model, DpermRejectReason::BrowserClosed, &mut ops);
    finish(model, ops)
}

/// Settle the consent sheet and any parked requests with a terminal error.
/// Parked requests are settled here as a fail-closed convergence: in TS their
/// block-scoped continuations ran after the navigation and answered a dead
/// document (an unobservable race, not a rule).
fn settle_local(model: &mut Model, reason: DpermRejectReason, ops: &mut Vec<DpermOperation>) {
    if let Some(consent) = model.consent.take() {
        for request in consent.requests {
            ops.push(respond_error(request.id, reason));
        }
    }
    for parked in std::mem::take(&mut model.parked) {
        ops.push(respond_error(parked.id, reason));
    }
}

fn account_switched(
    model: &mut Model,
    address: String,
    now_ms: f64,
) -> Command<DpermEffect, Event> {
    model.active_address = Some(address.clone());
    // Only a CONNECTED main-frame origin hears about the switch — never leak
    // an address to a site that never connected, and never emit while
    // disconnected (invariant ⑦; `browser.tsx:301-311`). The grant is
    // re-pinned to the NEW address so grant + page + signer stay reconciled.
    if model.connected_addr.is_none() || model.current_origin.is_empty() {
        return render();
    }
    let origin = model.current_origin.clone();
    let grant = DpermGrant {
        origin: origin.clone(),
        address: address.clone(),
        chain_id: model.chain_id,
        granted_at_ms: now_ms,
    };
    model.grants.insert(origin.clone(), Some(grant.clone()));
    model.reads_in_flight.retain(|o| o != &origin);
    model.connected_addr = Some(address.clone());
    finish(
        model,
        vec![
            DpermOperation::WriteGrant { grant },
            DpermOperation::EmitEvent {
                event: DpermPageEvent::AccountsChanged {
                    addresses: vec![address],
                },
            },
        ],
    )
}

fn chain_changed(model: &mut Model, chain_id: u32) -> Command<DpermEffect, Event> {
    let changed = model.chain_id != chain_id;
    model.chain_id = chain_id;
    // Only when connected and only on an actual change — never the address
    // (`browser.tsx:163-172`: no accountsChanged leak from this channel).
    if changed && model.connected_addr.is_some() {
        return finish(
            model,
            vec![DpermOperation::EmitEvent {
                event: DpermPageEvent::ChainChanged {
                    chain_id_hex: hex_chain_id(chain_id),
                },
            }],
        );
    }
    render()
}

fn revoke_requested(model: &mut Model, origin: Option<String>) -> Command<DpermEffect, Event> {
    let target = match origin {
        Some(origin) => origin,
        None => model.current_origin.clone(),
    };
    if target.is_empty() {
        return Command::done();
    }
    model.grants.insert(target.clone(), None);
    model.reads_in_flight.retain(|o| o != &target);
    let mut ops = vec![DpermOperation::RemoveGrant {
        origin: target.clone(),
    }];
    if target == model.current_origin {
        ops.push(DpermOperation::EmitEvent {
            event: DpermPageEvent::AccountsChanged {
                addresses: Vec::new(),
            },
        });
        ops.push(DpermOperation::EmitEvent {
            event: DpermPageEvent::Disconnect,
        });
        model.connected_addr = None;
    }
    finish(model, ops)
}

/// The popup entry's one question, answered on the view.
///
/// Both halves of the answer come from the pure policy below — nothing is
/// re-decided here: [`resolve_granted`] says what this origin may see (and
/// refuses to log it out on a cold read, invariant ②), [`decide_popup_request`]
/// says what to do about it. The three rules the popup exists to enforce are
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

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: DpermShellResult) -> Command<DpermEffect, Event> {
    match result {
        DpermShellResult::GrantRead { origin, grant } => {
            if let Some(index) = model.reads_in_flight.iter().position(|o| o == &origin) {
                model.reads_in_flight.remove(index);
                model.grants.insert(origin.clone(), grant);
            }
            // else: a write/revoke superseded this read while it was in
            // flight — the mirror is already fresher than the store snapshot,
            // so the snapshot must not clobber it.

            let mut ops = Vec::new();
            if model.nav_refresh.as_deref() == Some(origin.as_str()) {
                model.nav_refresh = None;
                let known = model.grants.get(&origin).cloned().flatten();
                model.connected_addr =
                    resolve_granted(known.as_ref(), model.wallet_addresses.as_deref())
                        .into_iter()
                        .next();
            }
            let (ready, parked): (Vec<_>, Vec<_>) = std::mem::take(&mut model.parked)
                .into_iter()
                .partition(|request| request.origin == origin);
            model.parked = parked;
            for request in ready {
                execute_decision(model, request, &mut ops);
            }
            finish(model, ops)
        }
        DpermShellResult::Ack => Command::done(),
    }
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

// ---------------------------------------------------------------------------
// Pure policy — the browser decision (`wallet-browser-router.ts:30-166`)
// ---------------------------------------------------------------------------

/// The FULL browser decision for one provider request — the single source of
/// truth the executor above (and every test) exercises.
#[derive(Clone, Debug, PartialEq)]
pub enum DpermDecision {
    Respond(DpermRespondPayload),
    Reject(DpermRejectReason),
    OpenConsent,
    MergeConsent,
    Forward,
}

pub fn decide_browser_request(
    method: &str,
    origin: &str,
    is_main_frame: bool,
    granted: &[String],
    has_active_account: bool,
    pending_consent_origin: Option<&str>,
) -> DpermDecision {
    // `eth_accounts` reflects the current grant and NEVER prompts
    // (invariant ⑧).
    if method == "eth_accounts" {
        return DpermDecision::Respond(DpermRespondPayload::Accounts {
            addresses: granted.to_vec(),
        });
    }
    // EIP-2255 introspection mirrors the grant.
    if method == "wallet_getPermissions" {
        return DpermDecision::Respond(DpermRespondPayload::Permissions {
            granted: !granted.is_empty(),
        });
    }
    if is_connect_method(method) {
        // Already granted → answer immediately, no prompt on revisit.
        if !granted.is_empty() {
            return DpermDecision::Respond(connect_payload(method, granted.to_vec()));
        }
        // §5.2 — a cross-origin iframe can never request accounts
        // (invariant ①).
        if !is_main_frame {
            return DpermDecision::Reject(DpermRejectReason::UnauthorizedFrame);
        }
        if !has_active_account {
            return DpermDecision::Reject(DpermRejectReason::NoAccountAvailable);
        }
        // Coalesce duplicate prompts from the same origin so the earlier
        // promise never hangs; reject a colliding second origin — a page
        // can't queue two connect sheets (invariant ④).
        return match pending_consent_origin {
            Some(pending) if pending == origin => DpermDecision::MergeConsent,
            Some(_) => DpermDecision::Reject(DpermRejectReason::ConsentBusy),
            None => DpermDecision::OpenConsent,
        };
    }
    // Forward: read-only RPC / chain switch / signing — but never sign on
    // insecure http (invariant ③; checked before the frame gate, matching
    // today's order: router first, transport second).
    if should_block_insecure_signing(method, origin) {
        return DpermDecision::Reject(DpermRejectReason::InsecureOrigin);
    }
    // Security: iframe provider traffic never reaches the signing pipeline
    // (`webview-transport.ts:116-120`, converged here).
    if !is_main_frame {
        return DpermDecision::Reject(DpermRejectReason::UnauthorizedFrame);
    }
    DpermDecision::Forward
}

/// The connect-consent result: accounts, or the EIP-2255 permission shape for
/// `wallet_requestPermissions`.
fn connect_payload(method: &str, granted: Vec<String>) -> DpermRespondPayload {
    if method == "wallet_requestPermissions" {
        DpermRespondPayload::Permissions { granted: true }
    } else {
        DpermRespondPayload::Accounts { addresses: granted }
    }
}

// ---------------------------------------------------------------------------
// Pure policy — the popup entry's decision (`web-request.tsx:169-193`)
// ---------------------------------------------------------------------------

/// The web-popup entry decides differently from the in-app browser — most
/// notably a non-connect request from a never-connected origin is REFUSED
/// (4100) rather than forwarded, and a request may pin the address it was
/// built for. Kept as its own explicit function so the drift between the two
/// entries is visible in one file instead of three.
#[derive(Clone, Debug, PartialEq)]
pub enum DpermPopupDecision {
    Respond(DpermRespondPayload),
    /// Show the popup's connect consent.
    Consent,
    Reject(DpermRejectReason),
    /// Hand to the WebPopupTransport → signing pipeline.
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

// ---------------------------------------------------------------------------
// Pure policy — origin security (`wallet-browser-router.ts:78-118`)
// ---------------------------------------------------------------------------

/// Whether a signing/value-moving request must be refused on this origin
/// (insecure public http).
pub fn should_block_insecure_signing(method: &str, origin: &str) -> bool {
    is_signing_method(method) && is_insecure_public_origin(origin)
}

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

/// `chainId` number → EIP-1193 hex string (`1` → `"0x1"`). The TS clamps with
/// `Math.max(0, Math.floor(...))`; a `u32` on this wire already satisfies it.
pub fn hex_chain_id(chain_id: u32) -> String {
    format!("0x{chain_id:x}")
}

// ---------------------------------------------------------------------------
// Command plumbing
// ---------------------------------------------------------------------------

fn respond_error(id: String, reason: DpermRejectReason) -> DpermOperation {
    DpermOperation::Respond {
        id,
        payload: DpermRespondPayload::Error {
            code: reason.code(),
            reason,
        },
    }
}

/// Issue the operations (answers must match the current attempt), then render.
fn finish(model: &Model, ops: Vec<DpermOperation>) -> Command<DpermEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<DpermEffect, Event>> = ops
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
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
