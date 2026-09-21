//! Machine — the in-app browser's whole decision half, for every tab
//! (spec 070).
//!
//! ```text
//! page_message ─► parse ─► hello ─► the tab's document changes ─► settle the old one (4900)
//!                   │
//!                   └─► request ─► route (dapp_rpc::classify)
//!                          ├─ accounts / chain / permissions ─► deliver now
//!                          ├─ connect ─► grant? deliver : consent sheet
//!                          ├─ switch / add ─► per-origin chain ─► chainChanged to that origin's tabs
//!                          ├─ read ─► bounded queue ─► `read` op ─► deliver
//!                          └─ sign ─► granted? ─► FIFO ─► `forward_to_signing` ─► signing_answered ─► deliver
//! ```
//!
//! ## Why it exists
//!
//! `dapp_permissions` decides for ONE document: one current origin, one
//! connected address, one attempt counter. Android and iOS share that one
//! core across every tab, and the audits of 2026-09-22 traced every tab bug
//! to it — a background tab's navigation settling the front tab's requests,
//! answers falling back to the front tab, a background tab's approval
//! announcing an address to the front page. Around it each native shell had
//! its own routing table, request router, open-id set and provider assembler
//! (four tables, three routers, three assemblers). This machine owns all of
//! it; a shell owns a WebView, posts strings, runs RPC calls and draws.
//!
//! ## Documents, not navigations (research R2)
//!
//! The bridge mints a document id at document start and says `hello` before
//! the page's own scripts run. A new document means the old one is gone, and
//! its open requests are settled with 4900 — never 4001: a dApp retries a
//! 4001, double-spending what may already be at the bundler. A load that
//! finishes with no `hello` (an error page, a page the script could not reach)
//! settles the old document too. `navigation_started` settles nothing: on
//! Android the new document's first messages can arrive before it, and
//! settling there would answer the new page's warm-up with 4900.
//!
//! ## Rules kept from `dapp_permissions` (and reused, not re-typed)
//!
//! [`resolve_granted`] / [`should_drop_grant`] (never drop a grant on a cold
//! or empty account read — invariant ②), the insecure-public-http signing
//! block ([`is_insecure_public_origin`]), [`origin_of`]. A connect with no
//! account is 4001; a second origin colliding with the open consent sheet is
//! 4001; the same origin merges.
//!
//! ## Rules this machine adds, each the extension's where it had one
//!
//! - The chain is per ORIGIN and persisted (`vela.chain.<origin>`); a new
//!   origin starts on its grant's chain, else [`DEFAULT_CHAIN_ID`].
//! - `wallet_addEthereumChain` for a chain the wallet has IS a switch.
//! - Every grant follows the wallet's active account (`followActiveAccount`).
//! - Signing needs a grant (4100), is pinned to the granted address, and is
//!   serialised: one sheet at a time, the rest in order.
//! - Reads are bounded per tab ([`READS_IN_FLIGHT`], [`READS_QUEUED`], then
//!   -32005 — requirement F06, never implemented natively before).
//! - A page that was answered with a user-operation hash can look up its
//!   receipt by that hash; the machine asks the shell to resolve it.
//! - Messages from a subframe are ignored: the script installs nothing there,
//!   so only a direct call to the host bridge can produce one, and there is no
//!   way to answer a subframe that is not also a way to speak as the top page.

use std::collections::{BTreeMap, VecDeque};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::dapp_permissions::{
    is_insecure_public_origin, origin_of, resolve_granted, should_drop_grant, DpermGrant,
};
use super::dapp_rpc::{
    self, chain_param, classify, error_body_json, error_json, event_json, hex_chain_id,
    parse_page_message, result_json, PageMessage, PageRequest, Route,
};
use super::sign_request::SignResponsePayload;

/// A site with no stored chain and no grant starts on Ethereum — the
/// extension's `DEFAULT_EXT_CHAIN_ID` (research R5).
pub const DEFAULT_CHAIN_ID: u32 = 1;
/// Reads one tab may have at the shell at once.
pub const READS_IN_FLIGHT: usize = 8;
/// Reads one tab may have waiting behind those; the next is -32005.
pub const READS_QUEUED: usize = 256;
/// Documents a tab remembers having retired, so a straggler from one of them
/// is recognised as stale rather than mistaken for a new page.
const RETIRED_DOCS: usize = 8;
/// Closed tabs remembered, so a closed page's late message is not a new page.
const CLOSED_TABS: usize = 64;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// One stored site: its grant (`vela.perm.<origin>`) and its chain
/// (`vela.chain.<origin>`), either of which may be absent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrStoredSite {
    pub origin: String,
    pub grant: Option<DpermGrant>,
    pub chain_id: Option<u32>,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DbrOperation"))]
pub enum DbrOperation {
    /// Every `vela.perm.*` and `vela.chain.*` the store holds.
    ListSites,
    WriteGrant {
        grant: DpermGrant,
    },
    RemoveGrant {
        origin: String,
    },
    WriteSiteChain {
        origin: String,
        chain_id: u32,
    },
    /// Hand `message_json` to `window.__velaDeliver` in `tab`. The bridge
    /// drops it unless its document is `doc`; a shell with no such tab drops
    /// it too — there is no "front tab" fallback anywhere.
    Deliver {
        tab: String,
        doc: String,
        message_json: String,
    },
    /// A node (`bundler: false`) or bundler read on `chain_id`, through the
    /// person's own endpoints.
    Read {
        tab: String,
        id: String,
        chain_id: u32,
        method: String,
        params_json: String,
        bundler: bool,
    },
    /// The transaction hash a user operation landed in, if it has.
    ResolveUserOp {
        chain_id: u32,
        user_op_hash: String,
    },
    /// Open the signing sheet for this request. Answer with
    /// [`Event::SigningAnswered`] — exactly once, whatever happens.
    ForwardToSigning {
        tab: String,
        id: String,
        method: String,
        params_json: String,
        origin: String,
        chain_id: u32,
        /// The address the site was shown — the signer is pinned to it.
        granted_address: String,
    },
    /// The page that asked is gone (it already has its 4900): close the
    /// sheet for this request without answering it.
    CancelSigning {
        tab: String,
        id: String,
    },
    /// The "Connected to <app>" history row.
    SaveConnectionRecord {
        address: String,
        chain_id: u32,
        origin: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DbrShellResult"))]
pub enum DbrShellResult {
    SitesListed {
        sites: Vec<DbrStoredSite>,
    },
    /// The JSON-RPC response body — `{"result":…}` or `{"error":{…}}` — or
    /// `None` when no endpoint answered.
    ReadAnswered {
        body_json: Option<String>,
    },
    UserOpResolved {
        tx_hash: Option<String>,
    },
    Ack,
}

impl Operation for DbrOperation {
    type Output = DbrShellResult;
}

#[effect]
pub enum DbrEffect {
    Render(RenderOperation),
    Shell(DbrOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "DbrEvent"))]
pub enum Event {
    /// The browser came up: list the stored sites once. Page messages that
    /// arrive before the list are held, not refused.
    Start,
    /// The chains a site may switch or add to.
    NetworksChanged {
        chain_ids: Vec<u32>,
    },
    /// Every wallet address. `None` while unknown — cold-load safe.
    AccountsUpdated {
        addresses: Option<Vec<String>>,
    },
    /// The active account changed (initial load included). Every grant
    /// follows it.
    AccountSwitched {
        address: String,
        now_ms: f64,
    },
    /// One string from the bridge. `frame_origin` is the PLATFORM's origin of
    /// the frame that sent it (Android `sourceOrigin`, iOS
    /// `frameInfo.securityOrigin`, desktop the webview's URL) — never
    /// anything the page wrote.
    PageMessage {
        tab: String,
        frame_origin: String,
        is_main_frame: bool,
        message_json: String,
    },
    /// A load began in `tab`. Settles nothing by itself (see the module doc).
    NavigationStarted {
        tab: String,
        url: String,
    },
    /// A load in `tab` finished or failed. If no document said hello since it
    /// began, the old document is gone and its requests are settled.
    LoadFinished {
        tab: String,
        url: String,
    },
    TabClosed {
        tab: String,
    },
    /// The tab's renderer died. Its requests are settled; the tab shows as
    /// crashed until its next document says hello.
    RendererGone {
        tab: String,
    },
    ConsentApproved {
        now_ms: f64,
    },
    ConsentRejected,
    /// The person picked a network for a site (the connection panel).
    SiteChainPicked {
        origin: String,
        chain_id: u32,
    },
    /// Disconnect one site (Settings, the connection panel).
    RevokeRequested {
        origin: String,
    },
    /// Every grant is gone (Settings → Storage → dApp connections).
    RevokeAll,
    /// The signing pipeline's answer to a forwarded request. `user_op_hash`
    /// is set when the page was answered with a user-operation hash, so its
    /// receipt lookups can be translated.
    SigningAnswered {
        tab: String,
        id: String,
        payload: SignResponsePayload,
        user_op_hash: Option<String>,
    },
    #[serde(skip)]
    SitesListed {
        sites: Vec<DbrStoredSite>,
    },
    #[serde(skip)]
    ReadDone {
        tab: String,
        id: String,
        body_json: Option<String>,
    },
    #[serde(skip)]
    UserOpDone {
        tab: String,
        id: String,
        chain_id: u32,
        method: String,
        tx_hash: Option<String>,
    },
    #[serde(skip)]
    Acked,
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
struct Site {
    grant: Option<DpermGrant>,
    chain_id: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenKind {
    Consent,
    Read,
    Signing,
    QueuedSigning,
}

#[derive(Clone, Debug)]
struct QueuedRead {
    id: String,
    chain_id: u32,
    method: String,
    params: Value,
    bundler: bool,
}

#[derive(Clone, Debug, Default)]
struct Tab {
    /// The current document and its origin.
    doc: Option<String>,
    doc_origin: Option<String>,
    /// The current document's own URL, from its hello.
    doc_href: Option<String>,
    /// The current document's load has finished at least once.
    doc_loaded: bool,
    /// The origin the tab SHOWS (from its URL) — for the chip, even while a
    /// document has not said hello yet.
    shown_origin: Option<String>,
    /// A load began and no document has said hello since.
    loading: bool,
    crashed: bool,
    retired: VecDeque<String>,
    /// Requests of the current document still owed an answer.
    open: BTreeMap<String, OpenKind>,
    reads_in_flight: usize,
    read_queue: VecDeque<QueuedRead>,
}

#[derive(Clone, Debug)]
struct ConsentEntry {
    tab: String,
    doc: String,
    id: String,
    method: String,
}

#[derive(Clone, Debug)]
struct Consent {
    origin: String,
    entries: Vec<ConsentEntry>,
}

#[derive(Clone, Debug)]
struct SignJob {
    tab: String,
    doc: String,
    id: String,
    method: String,
    params_json: String,
    origin: String,
}

#[derive(Clone, Debug)]
struct HeldMessage {
    tab: String,
    frame_origin: String,
    is_main_frame: bool,
    message_json: String,
}

#[derive(Default)]
pub struct Model {
    started: bool,
    ready: bool,
    held: Vec<HeldMessage>,
    sites: BTreeMap<String, Site>,
    chains: Vec<u32>,
    wallet_addresses: Option<Vec<String>>,
    active_address: Option<String>,
    tabs: BTreeMap<String, Tab>,
    consent: Option<Consent>,
    signing: Option<SignJob>,
    sign_queue: VecDeque<SignJob>,
    /// Lower-cased user-operation hashes pages were answered with.
    user_ops: Vec<String>,
    /// Tabs the shell closed. A straggler from one of them is ignored, never
    /// adopted as a new page.
    closed_tabs: VecDeque<String>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrConsentView {
    pub tab: String,
    pub origin: String,
    pub methods: Vec<String>,
    /// The account a grant would be made for (the active one).
    pub address: Option<String>,
    pub chain_id: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrTabView {
    pub tab: String,
    pub origin: Option<String>,
    pub connected_address: Option<String>,
    pub chain_id: u32,
    /// https, or a loopback / private-network http host — the lock tells the
    /// truth from this.
    pub secure: bool,
    pub crashed: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrSiteView {
    pub origin: String,
    pub address: String,
    pub chain_id: u32,
    pub granted_at_ms: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrSigningView {
    pub tab: String,
    pub id: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct DbrView {
    /// The stored sites have been read.
    pub ready: bool,
    pub consent: Option<DbrConsentView>,
    pub tabs: Vec<DbrTabView>,
    /// Every connected site, newest first — Settings' list.
    pub sites: Vec<DbrSiteView>,
    pub signing: Option<DbrSigningView>,
    pub queued_signing: u32,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct DappBrowser;

type Cmd = Command<DbrEffect, Event>;

impl App for DappBrowser {
    type Event = Event;
    type Model = Model;
    type ViewModel = DbrView;
    type Effect = DbrEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        let mut out = Out::default();
        match event {
            Event::Start => {
                if !model.started {
                    model.started = true;
                    out.request_list();
                }
            }
            Event::NetworksChanged { chain_ids } => model.chains = chain_ids,
            Event::AccountsUpdated { addresses } => {
                model.wallet_addresses = addresses;
                reconcile_grants(model, None, &mut out);
            }
            Event::AccountSwitched { address, now_ms } => {
                model.active_address = Some(address);
                reconcile_grants(model, Some(now_ms), &mut out);
            }
            Event::SitesListed { sites } => {
                sites_listed(model, sites, &mut out);
            }
            Event::PageMessage {
                tab,
                frame_origin,
                is_main_frame,
                message_json,
            } => {
                let held = HeldMessage {
                    tab,
                    frame_origin,
                    is_main_frame,
                    message_json,
                };
                if model.ready {
                    page_message(model, held, &mut out);
                } else {
                    model.held.push(held);
                }
            }
            Event::NavigationStarted { tab, url } => {
                if model.closed_tabs.contains(&tab) {
                    return render();
                }
                let entry = model.tabs.entry(tab).or_default();
                entry.crashed = false;
                entry.shown_origin = origin_of(&url);
                // On Android the new document's hello can come BEFORE the
                // platform says the load started. A current document that has
                // not finished loading, at exactly this URL, IS this load's
                // document — marking the load as hello-less would retire it
                // when the load finishes.
                let already_here = entry.doc.is_some()
                    && !entry.doc_loaded
                    && entry.doc_href.as_deref().map(without_fragment)
                        == Some(without_fragment(&url));
                entry.loading = !already_here;
            }
            Event::LoadFinished { tab, url } => {
                let Some(entry) = model.tabs.get_mut(&tab) else {
                    return render();
                };
                if let Some(origin) = origin_of(&url) {
                    entry.shown_origin = Some(origin);
                }
                // A load that began and ended with no document saying hello:
                // whatever was open belongs to a document that is gone.
                let orphaned = std::mem::replace(&mut entry.loading, false) && entry.doc.is_some();
                if !orphaned {
                    entry.doc_loaded = entry.doc.is_some();
                }
                if orphaned {
                    retire_document(model, &tab, true, &mut out);
                    if let Some(entry) = model.tabs.get_mut(&tab) {
                        entry.doc_origin = None;
                    }
                }
            }
            Event::TabClosed { tab } => {
                retire_document(model, &tab, false, &mut out);
                model.tabs.remove(&tab);
                if !model.closed_tabs.contains(&tab) {
                    model.closed_tabs.push_back(tab);
                    while model.closed_tabs.len() > CLOSED_TABS {
                        model.closed_tabs.pop_front();
                    }
                }
            }
            Event::RendererGone { tab } => {
                retire_document(model, &tab, false, &mut out);
                if let Some(entry) = model.tabs.get_mut(&tab) {
                    entry.crashed = true;
                    entry.loading = false;
                    entry.doc_origin = None;
                }
            }
            Event::ConsentApproved { now_ms } => consent_approved(model, now_ms, &mut out),
            Event::ConsentRejected => {
                if let Some(consent) = model.consent.take() {
                    for entry in consent.entries {
                        answer_error(
                            model,
                            &entry.tab,
                            &entry.doc,
                            &entry.id,
                            4001,
                            "User rejected the request",
                            &mut out,
                        );
                    }
                }
            }
            Event::SiteChainPicked { origin, chain_id } => {
                if model.chains.contains(&chain_id) {
                    set_site_chain(model, &origin, chain_id, &mut out);
                }
            }
            Event::RevokeRequested { origin } => revoke(model, &origin, &mut out),
            Event::RevokeAll => {
                let granted: Vec<String> = model
                    .sites
                    .iter()
                    .filter(|(_, site)| site.grant.is_some())
                    .map(|(origin, _)| origin.clone())
                    .collect();
                for origin in granted {
                    revoke(model, &origin, &mut out);
                }
            }
            Event::SigningAnswered {
                tab,
                id,
                payload,
                user_op_hash,
            } => signing_answered(model, &tab, &id, payload, user_op_hash, &mut out),
            Event::ReadDone { tab, id, body_json } => {
                read_done(model, &tab, &id, body_json, &mut out)
            }
            Event::UserOpDone {
                tab,
                id,
                chain_id,
                method,
                tx_hash,
            } => user_op_done(model, &tab, &id, chain_id, &method, tx_hash, &mut out),
            Event::Acked => return Command::done(),
        }
        out.into_command()
    }

    fn view(&self, model: &Model) -> DbrView {
        let mut sites: Vec<DbrSiteView> = model
            .sites
            .iter()
            .filter_map(|(origin, site)| {
                let grant = site.grant.as_ref()?;
                Some(DbrSiteView {
                    origin: origin.clone(),
                    address: grant.address.clone(),
                    chain_id: site_chain(model, origin),
                    granted_at_ms: grant.granted_at_ms,
                })
            })
            .collect();
        sites.sort_by(|a, b| {
            b.granted_at_ms
                .partial_cmp(&a.granted_at_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.origin.cmp(&b.origin))
        });
        DbrView {
            ready: model.ready,
            consent: model.consent.as_ref().map(|consent| DbrConsentView {
                tab: consent
                    .entries
                    .first()
                    .map(|entry| entry.tab.clone())
                    .unwrap_or_default(),
                origin: consent.origin.clone(),
                methods: consent.entries.iter().map(|e| e.method.clone()).collect(),
                address: model.active_address.clone(),
                chain_id: site_chain(model, &consent.origin),
            }),
            tabs: model
                .tabs
                .iter()
                .map(|(id, tab)| {
                    let origin = tab.doc_origin.clone().or(tab.shown_origin.clone());
                    DbrTabView {
                        tab: id.clone(),
                        connected_address: origin
                            .as_deref()
                            .and_then(|origin| granted(model, origin).into_iter().next()),
                        chain_id: origin
                            .as_deref()
                            .map(|origin| site_chain(model, origin))
                            .unwrap_or(DEFAULT_CHAIN_ID),
                        secure: origin
                            .as_deref()
                            .is_some_and(|origin| !is_insecure_public_origin(origin)),
                        crashed: tab.crashed,
                        origin,
                    }
                })
                .collect(),
            sites,
            signing: model.signing.as_ref().map(|job| DbrSigningView {
                tab: job.tab.clone(),
                id: job.id.clone(),
            }),
            queued_signing: model.sign_queue.len() as u32,
        }
    }
}

// ---------------------------------------------------------------------------
// Output collection
// ---------------------------------------------------------------------------

#[derive(Default)]
struct Out {
    commands: Vec<Cmd>,
}

impl Out {
    fn op(&mut self, operation: DbrOperation) {
        self.commands.push(
            Command::request_from_shell(operation).then_send(|_: DbrShellResult| Event::Acked),
        );
    }

    fn request_list(&mut self) {
        self.commands.push(
            Command::request_from_shell(DbrOperation::ListSites).then_send(|result| match result {
                DbrShellResult::SitesListed { sites } => Event::SitesListed { sites },
                // A neutral or wrong answer: nothing stored.
                _ => Event::SitesListed { sites: Vec::new() },
            }),
        );
    }

    fn read(&mut self, tab: &str, request: &QueuedRead) {
        let (tab_id, id) = (tab.to_owned(), request.id.clone());
        self.commands.push(
            Command::request_from_shell(DbrOperation::Read {
                tab: tab.to_owned(),
                id: request.id.clone(),
                chain_id: request.chain_id,
                method: request.method.clone(),
                params_json: request.params.to_string(),
                bundler: request.bundler,
            })
            .then_send(move |result| Event::ReadDone {
                tab: tab_id,
                id,
                body_json: match result {
                    DbrShellResult::ReadAnswered { body_json } => body_json,
                    _ => None,
                },
            }),
        );
    }

    fn resolve_user_op(&mut self, tab: &str, request: &QueuedRead, hash: String) {
        let (tab_id, id, chain_id, method) = (
            tab.to_owned(),
            request.id.clone(),
            request.chain_id,
            request.method.clone(),
        );
        self.commands.push(
            Command::request_from_shell(DbrOperation::ResolveUserOp {
                chain_id: request.chain_id,
                user_op_hash: hash,
            })
            .then_send(move |result| Event::UserOpDone {
                tab: tab_id,
                id,
                chain_id,
                method,
                tx_hash: match result {
                    DbrShellResult::UserOpResolved { tx_hash } => tx_hash,
                    _ => None,
                },
            }),
        );
    }

    fn into_command(mut self) -> Cmd {
        self.commands.push(render());
        Command::all(self.commands)
    }
}

// ---------------------------------------------------------------------------
// Sites
// ---------------------------------------------------------------------------

fn sites_listed(model: &mut Model, sites: Vec<DbrStoredSite>, out: &mut Out) {
    if model.ready {
        return;
    }
    for stored in sites {
        let Some(origin) = origin_of(&stored.origin) else {
            continue;
        };
        let site = model.sites.entry(origin.clone()).or_default();
        if let Some(grant) = stored.grant {
            // A grant is only ever FOR the origin it is filed under.
            if origin_of(&grant.origin).as_deref() == Some(origin.as_str()) {
                site.grant = Some(grant);
            }
        }
        if let Some(chain_id) = stored.chain_id.filter(|chain| *chain > 0) {
            site.chain_id = Some(chain_id);
        }
    }
    model.ready = true;
    reconcile_grants(model, None, out);
    for held in std::mem::take(&mut model.held) {
        page_message(model, held, out);
    }
}

fn site_chain(model: &Model, origin: &str) -> u32 {
    model
        .sites
        .get(origin)
        .and_then(|site| {
            site.chain_id
                .or(site.grant.as_ref().map(|grant| grant.chain_id))
        })
        .filter(|chain| *chain > 0)
        .unwrap_or(DEFAULT_CHAIN_ID)
}

/// The addresses this origin may see — the grant, unless its account left a
/// KNOWN wallet (a cold read never logs a site out).
fn granted(model: &Model, origin: &str) -> Vec<String> {
    let grant = model.sites.get(origin).and_then(|site| site.grant.as_ref());
    resolve_granted(grant, model.wallet_addresses.as_deref())
}

/// Drop grants whose account left the wallet; re-pin the rest to the active
/// account (`followActiveAccount`). `now_ms` is `Some` on an account switch —
/// the only time a re-pin re-stamps the grant.
fn reconcile_grants(model: &mut Model, now_ms: Option<f64>, out: &mut Out) {
    if !model.ready {
        return;
    }
    let origins: Vec<String> = model
        .sites
        .iter()
        .filter(|(_, site)| site.grant.is_some())
        .map(|(origin, _)| origin.clone())
        .collect();
    for origin in origins {
        let Some(grant) = model.sites.get(&origin).and_then(|s| s.grant.clone()) else {
            continue;
        };
        if should_drop_grant(Some(&grant), model.wallet_addresses.as_deref()) {
            revoke(model, &origin, out);
            continue;
        }
        let Some(active) = model.active_address.clone() else {
            continue;
        };
        if grant.address.eq_ignore_ascii_case(&active) {
            continue;
        }
        let repinned = DpermGrant {
            address: active.clone(),
            granted_at_ms: now_ms.unwrap_or(grant.granted_at_ms),
            ..grant
        };
        if let Some(site) = model.sites.get_mut(&origin) {
            site.grant = Some(repinned.clone());
        }
        out.op(DbrOperation::WriteGrant { grant: repinned });
        emit_to_origin(
            model,
            &origin,
            "accountsChanged",
            Some(json!([active])),
            out,
        );
    }
}

fn revoke(model: &mut Model, origin: &str, out: &mut Out) {
    let Some(site) = model.sites.get_mut(origin) else {
        return;
    };
    if site.grant.take().is_none() {
        return;
    }
    out.op(DbrOperation::RemoveGrant {
        origin: origin.to_owned(),
    });
    emit_to_origin(model, origin, "accountsChanged", Some(json!([])), out);
    emit_to_origin(model, origin, "disconnect", None, out);
}

fn set_site_chain(model: &mut Model, origin: &str, chain_id: u32, out: &mut Out) {
    if model.sites.get(origin).and_then(|s| s.chain_id) == Some(chain_id) {
        return;
    }
    let changed = site_chain(model, origin) != chain_id;
    model.sites.entry(origin.to_owned()).or_default().chain_id = Some(chain_id);
    out.op(DbrOperation::WriteSiteChain {
        origin: origin.to_owned(),
        chain_id,
    });
    if changed {
        emit_to_origin(
            model,
            origin,
            "chainChanged",
            Some(Value::String(hex_chain_id(chain_id))),
            out,
        );
    }
}

/// An event to every tab whose current document is `origin`.
fn emit_to_origin(model: &Model, origin: &str, event: &str, data: Option<Value>, out: &mut Out) {
    for (tab_id, tab) in &model.tabs {
        if tab.doc_origin.as_deref() != Some(origin) {
            continue;
        }
        if let Some(doc) = &tab.doc {
            out.op(DbrOperation::Deliver {
                tab: tab_id.clone(),
                doc: doc.clone(),
                message_json: event_json(doc, event, data.clone()),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

/// The tab's current document is gone: settle everything it had open, pull
/// its entries out of the consent sheet and the signing line, and cancel a
/// sheet showing one of its requests. `deliver` = the tab still exists, so
/// the (stale) answers are posted — the bridge of a newer document drops them.
fn retire_document(model: &mut Model, tab_id: &str, deliver: bool, out: &mut Out) {
    let Some(tab) = model.tabs.get_mut(tab_id) else {
        return;
    };
    let Some(doc) = tab.doc.take() else {
        return;
    };
    tab.retired.push_back(doc.clone());
    while tab.retired.len() > RETIRED_DOCS {
        tab.retired.pop_front();
    }
    let open = std::mem::take(&mut tab.open);
    tab.read_queue.clear();

    if deliver {
        for id in open.keys() {
            out.op(DbrOperation::Deliver {
                tab: tab_id.to_owned(),
                doc: doc.clone(),
                message_json: error_json(&doc, id, 4900, "The page navigated away"),
            });
        }
    }

    if let Some(consent) = &mut model.consent {
        consent
            .entries
            .retain(|entry| !(entry.tab == tab_id && entry.doc == doc));
        if consent.entries.is_empty() {
            model.consent = None;
        }
    }
    model
        .sign_queue
        .retain(|job| !(job.tab == tab_id && job.doc == doc));
    if let Some(job) = model
        .signing
        .take_if(|job| job.tab == tab_id && job.doc == doc)
    {
        out.op(DbrOperation::CancelSigning {
            tab: job.tab,
            id: job.id,
        });
        forward_next(model, out);
    }
}

fn page_message(model: &mut Model, message: HeldMessage, out: &mut Out) {
    // A subframe cannot be answered without speaking as the top page; the
    // script installs nothing there, so this is a direct call to the host.
    if !message.is_main_frame || model.closed_tabs.contains(&message.tab) {
        return;
    }
    let Some(frame_origin) = origin_of(&message.frame_origin) else {
        return;
    };
    match parse_page_message(&message.message_json) {
        PageMessage::Ignored => {}
        PageMessage::Hello { doc, href } => {
            hello(model, &message.tab, doc, frame_origin, out);
            if let Some(tab) = model.tabs.get_mut(&message.tab) {
                tab.doc_href = href;
            }
        }
        PageMessage::Invalid {
            doc,
            id,
            code,
            message: words,
        } => {
            let (Some(doc), Some(id)) = (doc, id) else {
                return;
            };
            if current_document(model, &message.tab, &doc, &frame_origin, out) {
                out.op(DbrOperation::Deliver {
                    tab: message.tab.clone(),
                    doc: doc.clone(),
                    message_json: error_json(&doc, &id, code, words),
                });
            }
        }
        PageMessage::Request(request) => {
            if current_document(model, &message.tab, &request.doc, &frame_origin, out) {
                handle_request(model, &message.tab, request, out);
            }
        }
    }
}

fn hello(model: &mut Model, tab_id: &str, doc: String, origin: String, out: &mut Out) {
    let tab = model.tabs.entry(tab_id.to_owned()).or_default();
    tab.loading = false;
    tab.crashed = false;
    if tab.doc.as_deref() == Some(doc.as_str()) {
        return;
    }
    retire_document(model, tab_id, true, out);
    let tab = model.tabs.entry(tab_id.to_owned()).or_default();
    tab.retired.retain(|retired| retired != &doc);
    tab.doc = Some(doc);
    tab.doc_origin = Some(origin.clone());
    tab.doc_href = None;
    tab.doc_loaded = false;
    tab.shown_origin = Some(origin);
}

/// A URL without its `#fragment` — a fragment change is not a new load.
fn without_fragment(url: &str) -> &str {
    url.split('#').next().unwrap_or(url)
}

/// Is `doc` the tab's current document? A never-seen document is adopted (its
/// hello was lost — the request is proof it exists); a retired one is stale.
fn current_document(
    model: &mut Model,
    tab_id: &str,
    doc: &str,
    frame_origin: &str,
    out: &mut Out,
) -> bool {
    let tab = model.tabs.entry(tab_id.to_owned()).or_default();
    if tab.doc.as_deref() == Some(doc) {
        return tab.doc_origin.as_deref() == Some(frame_origin);
    }
    if tab.retired.iter().any(|retired| retired == doc) {
        return false;
    }
    hello(model, tab_id, doc.to_owned(), frame_origin.to_owned(), out);
    true
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

fn handle_request(model: &mut Model, tab_id: &str, request: PageRequest, out: &mut Out) {
    let Some(origin) = model
        .tabs
        .get(tab_id)
        .and_then(|tab| tab.doc_origin.clone())
    else {
        return;
    };
    let doc = request.doc.clone();
    let id = request.id.clone();
    if model
        .tabs
        .get(tab_id)
        .is_some_and(|tab| tab.open.contains_key(&id))
    {
        deliver_error(tab_id, &doc, &id, -32602, "Duplicate request id", out);
        return;
    }

    let grant_addresses = granted(model, &origin);
    // A grant whose account left a KNOWN wallet is physically dropped the
    // moment it is consulted (the core's rule since 017).
    if model
        .sites
        .get(&origin)
        .and_then(|site| site.grant.as_ref())
        .is_some_and(|grant| should_drop_grant(Some(grant), model.wallet_addresses.as_deref()))
    {
        revoke(model, &origin, out);
    }

    let chain_id = site_chain(model, &origin);
    match classify(&request.method) {
        Route::Accounts => deliver_result(tab_id, &doc, &id, json!(grant_addresses), out),
        Route::Coinbase => deliver_result(
            tab_id,
            &doc,
            &id,
            grant_addresses
                .first()
                .map(|a| Value::String(a.clone()))
                .unwrap_or(Value::Null),
            out,
        ),
        Route::Permissions => deliver_result(
            tab_id,
            &doc,
            &id,
            permissions_json(!grant_addresses.is_empty()),
            out,
        ),
        Route::ChainId => deliver_result(
            tab_id,
            &doc,
            &id,
            Value::String(hex_chain_id(chain_id)),
            out,
        ),
        Route::NetVersion => {
            deliver_result(tab_id, &doc, &id, Value::String(chain_id.to_string()), out)
        }
        Route::WatchAsset => deliver_result(tab_id, &doc, &id, Value::Bool(false), out),
        Route::RevokePermissions => {
            revoke(model, &origin, out);
            deliver_result(tab_id, &doc, &id, Value::Null, out);
        }
        Route::Connect => {
            if !grant_addresses.is_empty() {
                let payload = if request.method == "wallet_requestPermissions" {
                    permissions_json(true)
                } else {
                    json!(grant_addresses)
                };
                deliver_result(tab_id, &doc, &id, payload, out);
                return;
            }
            if model.active_address.is_none() {
                deliver_error(tab_id, &doc, &id, 4001, "No wallet account available", out);
                return;
            }
            // One sheet: the same origin joins it, another origin is told
            // no rather than queued behind a decision it cannot see.
            if model
                .consent
                .as_ref()
                .is_some_and(|consent| consent.origin != origin)
            {
                deliver_error(
                    tab_id,
                    &doc,
                    &id,
                    4001,
                    "Another connection request is open",
                    out,
                );
                return;
            }
            let entry = ConsentEntry {
                tab: tab_id.to_owned(),
                doc,
                id: id.clone(),
                method: request.method,
            };
            model
                .consent
                .get_or_insert_with(|| Consent {
                    origin,
                    entries: Vec::new(),
                })
                .entries
                .push(entry);
            open(model, tab_id, &id, OpenKind::Consent);
        }
        route @ (Route::SwitchChain | Route::AddChain) => {
            let Some(wanted) = chain_param(&request.params) else {
                deliver_error(tab_id, &doc, &id, -32602, "Expected [{ chainId }]", out);
                return;
            };
            if !model.chains.contains(&wanted) {
                let words = if route == Route::AddChain {
                    format!("Add chain {wanted} in Vela's network settings first")
                } else {
                    format!("Chain {wanted} is not in Vela's networks")
                };
                deliver_error(tab_id, &doc, &id, 4902, &words, out);
                return;
            }
            set_site_chain(model, &origin, wanted, out);
            deliver_result(tab_id, &doc, &id, Value::Null, out);
        }
        Route::Sign => {
            if is_insecure_public_origin(&origin) {
                deliver_error(
                    tab_id,
                    &doc,
                    &id,
                    4100,
                    "Signing requires a secure origin",
                    out,
                );
                return;
            }
            if grant_addresses.is_empty() {
                deliver_error(tab_id, &doc, &id, 4100, "This site is not connected", out);
                return;
            }
            // A request naming an account must name the one this site was
            // shown — never a silent swap of the signer (the popup entry's
            // `StaleAuthorizedAddress`, now every entry's).
            if let Some(asked) = dapp_rpc::requested_address(&request.method, &request.params) {
                if !grant_addresses
                    .iter()
                    .any(|granted| granted.eq_ignore_ascii_case(&asked))
                {
                    deliver_error(
                        tab_id,
                        &doc,
                        &id,
                        4100,
                        "The requested account is not connected to this site",
                        out,
                    );
                    return;
                }
            }
            let job = SignJob {
                tab: tab_id.to_owned(),
                doc,
                id: id.clone(),
                method: request.method,
                params_json: request.params.to_string(),
                origin,
            };
            if model.signing.is_none() {
                open(model, tab_id, &id, OpenKind::Signing);
                forward(model, job, out);
            } else {
                open(model, tab_id, &id, OpenKind::QueuedSigning);
                model.sign_queue.push_back(job);
            }
        }
        Route::Read { bundler } => {
            let read = QueuedRead {
                id: id.clone(),
                chain_id,
                method: request.method,
                params: request.params,
                bundler,
            };
            let Some(tab) = model.tabs.get_mut(tab_id) else {
                return;
            };
            if tab.reads_in_flight < READS_IN_FLIGHT {
                tab.open.insert(id, OpenKind::Read);
                start_read(model, tab_id, read, out);
            } else if tab.read_queue.len() < READS_QUEUED {
                tab.open.insert(id, OpenKind::Read);
                tab.read_queue.push_back(read);
            } else {
                deliver_error(tab_id, &doc, &id, -32005, "Limit exceeded", out);
            }
        }
        Route::Unsupported => {
            let words = format!("Vela does not support {}", request.method);
            deliver_error(tab_id, &doc, &id, 4200, &words, out);
        }
    }
}

fn open(model: &mut Model, tab_id: &str, id: &str, kind: OpenKind) {
    if let Some(tab) = model.tabs.get_mut(tab_id) {
        tab.open.insert(id.to_owned(), kind);
    }
}

fn permissions_json(granted: bool) -> Value {
    if granted {
        json!([{ "parentCapability": "eth_accounts" }])
    } else {
        json!([])
    }
}

fn deliver_result(tab: &str, doc: &str, id: &str, result: Value, out: &mut Out) {
    out.op(DbrOperation::Deliver {
        tab: tab.to_owned(),
        doc: doc.to_owned(),
        message_json: result_json(doc, id, &result),
    });
}

fn deliver_error(tab: &str, doc: &str, id: &str, code: i64, words: &str, out: &mut Out) {
    out.op(DbrOperation::Deliver {
        tab: tab.to_owned(),
        doc: doc.to_owned(),
        message_json: error_json(doc, id, code, words),
    });
}

/// Answer an OPEN request of the tab's current document, exactly once.
/// Returns whether it was still open.
fn close_open(model: &mut Model, tab_id: &str, doc: &str, id: &str) -> bool {
    let Some(tab) = model.tabs.get_mut(tab_id) else {
        return false;
    };
    if tab.doc.as_deref() != Some(doc) {
        return false;
    }
    tab.open.remove(id).is_some()
}

fn answer_error(
    model: &mut Model,
    tab: &str,
    doc: &str,
    id: &str,
    code: i64,
    words: &str,
    out: &mut Out,
) {
    if close_open(model, tab, doc, id) {
        deliver_error(tab, doc, id, code, words, out);
    }
}

fn consent_approved(model: &mut Model, now_ms: f64, out: &mut Out) {
    let Some(active) = model.active_address.clone() else {
        return;
    };
    let Some(consent) = model.consent.take() else {
        return;
    };
    let origin = consent.origin.clone();
    let chain_id = site_chain(model, &origin);
    let grant = DpermGrant {
        origin: origin.clone(),
        address: active.clone(),
        chain_id,
        granted_at_ms: now_ms,
    };
    model.sites.entry(origin.clone()).or_default().grant = Some(grant.clone());
    // Persist, audit, answer, then announce — the order every client used.
    out.op(DbrOperation::WriteGrant { grant });
    out.op(DbrOperation::SaveConnectionRecord {
        address: active.clone(),
        chain_id,
        origin: origin.clone(),
    });
    for entry in consent.entries {
        if close_open(model, &entry.tab, &entry.doc, &entry.id) {
            let payload = if entry.method == "wallet_requestPermissions" {
                permissions_json(true)
            } else {
                json!([active.clone()])
            };
            deliver_result(&entry.tab, &entry.doc, &entry.id, payload, out);
        }
    }
    emit_to_origin(
        model,
        &origin,
        "accountsChanged",
        Some(json!([active])),
        out,
    );
    emit_to_origin(
        model,
        &origin,
        "chainChanged",
        Some(Value::String(hex_chain_id(chain_id))),
        out,
    );
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

fn forward(model: &mut Model, job: SignJob, out: &mut Out) {
    let granted_address = granted(model, &job.origin).into_iter().next();
    let Some(granted_address) = granted_address else {
        // The grant went away while the request waited in line.
        answer_error(
            model,
            &job.tab,
            &job.doc,
            &job.id,
            4100,
            "This site is not connected",
            out,
        );
        forward_next(model, out);
        return;
    };
    if let Some(tab) = model.tabs.get_mut(&job.tab) {
        tab.open.insert(job.id.clone(), OpenKind::Signing);
    }
    out.op(DbrOperation::ForwardToSigning {
        tab: job.tab.clone(),
        id: job.id.clone(),
        method: job.method.clone(),
        params_json: job.params_json.clone(),
        origin: job.origin.clone(),
        chain_id: site_chain(model, &job.origin),
        granted_address,
    });
    model.signing = Some(job);
}

fn forward_next(model: &mut Model, out: &mut Out) {
    while model.signing.is_none() {
        let Some(job) = model.sign_queue.pop_front() else {
            return;
        };
        let still_open = model.tabs.get(&job.tab).is_some_and(|tab| {
            tab.doc.as_deref() == Some(job.doc.as_str()) && tab.open.contains_key(&job.id)
        });
        if still_open {
            forward(model, job, out);
        }
    }
}

fn signing_answered(
    model: &mut Model,
    tab_id: &str,
    id: &str,
    payload: SignResponsePayload,
    user_op_hash: Option<String>,
    out: &mut Out,
) {
    let job = match &model.signing {
        Some(job) if job.tab == tab_id && job.id == id => model.signing.take(),
        _ => None,
    };
    if let Some(hash) = user_op_hash {
        let hash = hash.to_ascii_lowercase();
        if !model.user_ops.contains(&hash) {
            model.user_ops.push(hash);
        }
    }
    if let Some(job) = job {
        if close_open(model, tab_id, &job.doc, id) {
            let message = match payload {
                SignResponsePayload::Ok { result } => result_json(
                    &job.doc,
                    id,
                    &result.map(Value::String).unwrap_or(Value::Null),
                ),
                SignResponsePayload::Err {
                    code,
                    kind,
                    message,
                } => error_json(
                    &job.doc,
                    id,
                    i64::from(code),
                    message
                        .as_deref()
                        .filter(|m| !m.is_empty())
                        .unwrap_or(dapp_rpc::sign_error_message(kind)),
                ),
            };
            out.op(DbrOperation::Deliver {
                tab: tab_id.to_owned(),
                doc: job.doc,
                message_json: message,
            });
        }
    }
    forward_next(model, out);
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

fn start_read(model: &mut Model, tab_id: &str, read: QueuedRead, out: &mut Out) {
    if let Some(tab) = model.tabs.get_mut(tab_id) {
        tab.reads_in_flight += 1;
    }
    let receipt_lookup = matches!(
        read.method.as_str(),
        "eth_getTransactionReceipt" | "eth_getTransactionByHash"
    );
    if receipt_lookup {
        let hash = read
            .params
            .as_array()
            .and_then(|params| params.first())
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase);
        if let Some(hash) = hash.filter(|hash| model.user_ops.contains(hash)) {
            out.resolve_user_op(tab_id, &read, hash);
            return;
        }
    }
    out.read(tab_id, &read);
}

fn read_done(model: &mut Model, tab_id: &str, id: &str, body_json: Option<String>, out: &mut Out) {
    let Some(tab) = model.tabs.get_mut(tab_id) else {
        return;
    };
    tab.reads_in_flight = tab.reads_in_flight.saturating_sub(1);
    let doc = tab.doc.clone();
    if let Some(doc) = doc {
        if close_open(model, tab_id, &doc, id) {
            let message = match body_json
                .as_deref()
                .and_then(|body| serde_json::from_str::<Value>(body).ok())
            {
                None => error_json(&doc, id, -32603, "No endpoint answered"),
                Some(body) => match body.get("error").filter(|e| !e.is_null()) {
                    Some(error) => error_body_json(&doc, id, error),
                    None => result_json(&doc, id, body.get("result").unwrap_or(&Value::Null)),
                },
            };
            out.op(DbrOperation::Deliver {
                tab: tab_id.to_owned(),
                doc,
                message_json: message,
            });
        }
    }
    pump_reads(model, tab_id, out);
}

fn user_op_done(
    model: &mut Model,
    tab_id: &str,
    id: &str,
    chain_id: u32,
    method: &str,
    tx_hash: Option<String>,
    out: &mut Out,
) {
    match tx_hash {
        // Landed: the node's answer for the real transaction. The read stays
        // in flight — the same slot, one more hop.
        Some(tx_hash) => out.read(
            tab_id,
            &QueuedRead {
                id: id.to_owned(),
                chain_id,
                method: method.to_owned(),
                params: json!([tx_hash]),
                bundler: false,
            },
        ),
        // Not yet: `null`, as a node answers for a transaction still pending.
        None => read_done(
            model,
            tab_id,
            id,
            Some(json!({ "result": null }).to_string()),
            out,
        ),
    }
}

fn pump_reads(model: &mut Model, tab_id: &str, out: &mut Out) {
    loop {
        let Some(tab) = model.tabs.get_mut(tab_id) else {
            return;
        };
        if tab.reads_in_flight >= READS_IN_FLIGHT {
            return;
        }
        let Some(next) = tab.read_queue.pop_front() else {
            return;
        };
        start_read(model, tab_id, next, out);
    }
}

impl super::SplitEffect for DbrEffect {
    type Op = DbrOperation;
    fn into_shell(self) -> Option<crux_core::Request<DbrOperation>> {
        match self {
            DbrEffect::Render(_) => None,
            DbrEffect::Shell(request) => Some(request),
        }
    }
}
