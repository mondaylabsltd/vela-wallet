//! The in-app browser's machine, driven from gpui (spec 070).
//!
//! `vela_core::app::dapp_browser` is the whole decision half of the browser:
//! it parses what the page posted, routes every method through one table,
//! keeps each tab's document and its open requests, the per-origin chain and
//! the grants, serialises signing, and says which document every answer is
//! for. Before 070 this file hosted `dapp_permissions` — a machine that
//! modelled ONE document — and the page kept the rest itself: a routing table
//! (`executor::dapp_rpc`), a column-wide `browser_chain`, a list of open ids to
//! settle, and a signing column that silently replaced the request it was
//! showing when a second one arrived.
//!
//! ## What this file must never decide
//!
//! Whether an origin is connected, whether a request is a read or a
//! signature, which chain a site is on, which document an answer belongs to,
//! and which error code a refusal carries. The core owns every one of those,
//! and each is a security rule. This file reads the store, runs a read on a
//! worker, hands the page's strings to the webview, and hands the signing
//! column its orders.
//!
//! ## Two halves
//!
//! [`BrowserDriver`] runs the machine with no gpui in it: a local operation is
//! answered inline, a network call and a delivery and a signing order come
//! back out as [`Outbound`]. That is what the tests drive, against the real
//! machine and a temporary store. [`BrowserHost`] is the entity around it —
//! it puts a delivery into the webview, a read on the background executor,
//! and an order where the page will pick it up.

use std::collections::VecDeque;

use gpui::Context;

use vela_core::app::dapp_browser::{DappBrowser, DbrOperation, DbrShellResult, DbrView, Event};

use crate::core_host::{CoreHost, Pending};
use crate::executor::dapp_browser::{self as executor, Performed, SigningOrder};
use crate::executor::now_ms;

/// The one tab this shell has.
///
/// wry gives the desktop ONE webview (see `webview.rs`), and the explore tab
/// strip is a list of remembered URLs that it re-navigates between. So the
/// core's "tab" — the thing that holds documents — is that webview, and a
/// switch between strip tabs is what it really is: a navigation, whose new
/// document retires the old one.
pub const BROWSER_TAB: &str = "browser";

/// What the machine needs done that only the entity around it can do.
pub enum Outbound {
    /// Into the page in `tab`, now.
    Deliver { tab: String, message_json: String },
    /// A network call: run it off the main thread and resolve `id` with its
    /// answer — or with `neutral` if it could not finish.
    Work {
        id: u64,
        work: Box<dyn FnOnce() -> DbrShellResult + Send>,
        neutral: DbrShellResult,
    },
    /// For the signing column.
    Signing(SigningOrder),
}

/// The machine and its operations, with no gpui in the way.
pub struct BrowserDriver {
    core: CoreHost<DappBrowser>,
}

impl Default for BrowserDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl BrowserDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            core: CoreHost::new(),
        }
    }

    #[must_use]
    pub fn view(&self) -> DbrView {
        self.core.view()
    }

    /// Send an event; perform what can be performed now.
    pub fn dispatch(&mut self, event: Event) -> Vec<Outbound> {
        let pending = self.core.dispatch(event);
        self.pump(pending)
    }

    /// A network call came back.
    pub fn resolve(&mut self, id: u64, result: DbrShellResult) -> Vec<Outbound> {
        let pending = self.core.resolve(id, result);
        self.pump(pending)
    }

    /// Drain the machine: local answers go straight back in (in the order the
    /// core asked), everything else goes out. A delivery and a signing order
    /// are acked the moment they are handed over — the core owes the page
    /// nothing more for them, and a later answer comes back as an event.
    fn pump(&mut self, pending: Vec<Pending<DbrOperation>>) -> Vec<Outbound> {
        let mut out = Vec::new();
        let mut queue: VecDeque<Pending<DbrOperation>> = pending.into();
        while let Some(next) = queue.pop_front() {
            let answer = match executor::perform(&next.operation) {
                Performed::Now(result) => result,
                Performed::Deliver { tab, message_json } => {
                    out.push(Outbound::Deliver { tab, message_json });
                    DbrShellResult::Ack
                }
                Performed::Signing(order) => {
                    out.push(Outbound::Signing(order));
                    DbrShellResult::Ack
                }
                Performed::Blocking(work) => {
                    out.push(Outbound::Work {
                        id: next.id,
                        work,
                        neutral: executor::neutral(&next.operation),
                    });
                    continue;
                }
            };
            queue.extend(self.core.resolve(next.id, answer));
        }
        out
    }
}

/// The browser machine, as the page's entity.
pub struct BrowserHost {
    driver: BrowserDriver,
    pub view: DbrView,
    /// Orders for the signing column, drained by the page on every
    /// observation. The column is the page's: a host that opened one would be
    /// two owners of the same column.
    orders: Vec<SigningOrder>,
    /// What the machine was last told, so a refresh that changed nothing
    /// says nothing.
    chains: Vec<u32>,
    wallet: Option<(Vec<String>, String)>,
    /// Spec 079 US4 / 082 RF1: the pool's view of which chains are down, as
    /// last read — whether the chain of the page in front can be reached.
    chain_health: ChainHealth,
    /// A re-read is scheduled while something is down.
    health_watching: bool,
    /// The chain whose notice Retry is out (spec 082 RF4): busy until its one
    /// read settles.
    retrying: Option<u32>,
}

/// The pool's three lists the chain notice reads (spec 082 RF1).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ChainHealth {
    /// Every endpoint failed every pass of a call.
    pub failed: Vec<u32>,
    /// A call's FIRST pass reached no endpoint, with no rate limit in sight —
    /// the notice's early half, a pass sooner than `failed`.
    pub unreached: Vec<u32>,
    pub rate_limited: Vec<u32>,
}

/// Whether the page's chain gets the notice (spec 079 FR-014, 082 RF1): its
/// pool failed — or its first pass reached nothing — and not merely because
/// providers rate-limit: a rate limit lifts on its own, and the wallet's
/// standing rule is to stay quiet about it. The home RPC banner keeps reading
/// `failed` alone.
#[must_use]
pub fn chain_down(chain_id: u32, health: &ChainHealth) -> bool {
    (health.failed.contains(&chain_id) || health.unreached.contains(&chain_id))
        && !health.rate_limited.contains(&chain_id)
}

/// How often the health is read again while a chain is down: from the view,
/// never from the network — an answer from anywhere in the app clears it.
const HEALTH_RECHECK: std::time::Duration = std::time::Duration::from_secs(5);

impl BrowserHost {
    /// The machine, told about the world before it reads the store.
    ///
    /// The order is the core's own boot: accounts and networks first, then
    /// `Start` lists the sites. Listed last, the grants are judged against a
    /// KNOWN account list and follow the active account without being
    /// re-stamped — told the account after the list, every grant would look
    /// freshly made on every launch.
    pub fn new(cx: &mut Context<Self>) -> Self {
        let driver = BrowserDriver::new();
        let view = driver.view();
        let mut host = Self {
            driver,
            view,
            orders: Vec::new(),
            chains: Vec::new(),
            wallet: None,
            chain_health: ChainHealth::default(),
            health_watching: false,
            retrying: None,
        };
        host.follow_wallet(cx);
        host.follow_networks(cx);
        host.dispatch(Event::Start, cx);
        // An account switch is a session change; so is everything else the
        // session does, which is why `follow_wallet` compares before it speaks.
        cx.observe_global::<crate::session::SessionState>(|host, cx| host.follow_wallet(cx))
            .detach();
        host
    }

    pub fn dispatch(&mut self, event: Event, cx: &mut Context<Self>) {
        let out = self.driver.dispatch(event);
        self.act(out, cx);
    }

    fn resolve(&mut self, id: u64, result: DbrShellResult, cx: &mut Context<Self>) {
        let out = self.driver.resolve(id, result);
        self.act(out, cx);
    }

    fn act(&mut self, out: Vec<Outbound>, cx: &mut Context<Self>) {
        for next in out {
            match next {
                Outbound::Deliver { tab, message_json } => {
                    crate::webview::deliver(&tab, &message_json);
                }
                Outbound::Work { id, work, neutral } => {
                    cx.spawn(async move |host, cx| {
                        // A panic in the work is survived (spec 038) and the
                        // page still gets its neutral answer — a read that
                        // never settles is the one outcome this must not have.
                        let result = cx
                            .background_executor()
                            .spawn(async move { crate::panic_report::guarded(work) })
                            .await
                            .unwrap_or(neutral);
                        host.update(cx, |host, cx| {
                            host.resolve(id, result, cx);
                            // The read went through the pool: whether the
                            // page's chain answered is known now (spec 079).
                            host.refresh_health(cx);
                        })
                        .ok();
                    })
                    .detach();
                }
                Outbound::Signing(order) => self.orders.push(order),
            }
        }
        self.view = self.driver.view();
        cx.notify();
    }

    /// One string from the page, with the URL of the document that sent it
    /// and whether that document is the top one — both the platform's word,
    /// never the page's (see `webview::on_ipc`).
    ///
    /// A switch or an add is judged against the wallet's networks, which the
    /// person edits in Settings; those are re-read here, before the one kind of
    /// request that needs them, rather than on every read a page makes.
    pub fn page_message(
        &mut self,
        frame_url: String,
        is_main_frame: bool,
        message_json: String,
        cx: &mut Context<Self>,
    ) {
        if message_json.contains("wallet_switchEthereumChain")
            || message_json.contains("wallet_addEthereumChain")
        {
            self.follow_networks(cx);
        }
        self.dispatch(
            Event::PageMessage {
                tab: BROWSER_TAB.to_owned(),
                // The SENDER's URL — wry reads it from the message's own
                // frame — so a subframe that reaches the IPC channel directly
                // is named by its own origin, never by the page around it.
                frame_origin: frame_url,
                is_main_frame,
                message_json,
            },
            cx,
        );
    }

    /// The chains a site may switch or add to — the same list the network
    /// settings show.
    pub fn follow_networks(&mut self, cx: &mut Context<Self>) {
        let chains = crate::wallet::signing_host::known_chain_ids();
        if chains != self.chains {
            self.chains.clone_from(&chains);
            self.dispatch(Event::NetworksChanged { chain_ids: chains }, cx);
        }
    }

    /// Every account and the active one, from the session.
    ///
    /// While the session is still reading storage it says nothing: `None` is
    /// the core's "not known yet", and a cold read must never log a site out.
    fn follow_wallet(&mut self, cx: &mut Context<Self>) {
        let session = crate::session::view(cx);
        if session.loading {
            return;
        }
        let addresses: Vec<String> = session
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        let active = session.address;
        let told = (addresses.clone(), active.clone());
        if self.wallet.as_ref() == Some(&told) {
            return;
        }
        let switched = self.wallet.as_ref().map(|(_, was)| was) != Some(&active);
        self.wallet = Some(told);
        self.dispatch(
            Event::AccountsUpdated {
                addresses: (!addresses.is_empty()).then_some(addresses),
            },
            cx,
        );
        if switched && !active.is_empty() {
            // Every grant follows it (`followActiveAccount`), and each open
            // page of those sites hears `accountsChanged` — the core's rule.
            self.dispatch(
                Event::AccountSwitched {
                    address: active,
                    now_ms: now_ms(),
                },
                cx,
            );
        }
    }

    /// Read the pool's view of which chains are down, off the frame.
    pub fn refresh_health(&mut self, cx: &mut Context<Self>) {
        cx.spawn(async move |host, cx| {
            let health = cx
                .background_executor()
                .spawn(async move {
                    ChainHealth {
                        failed: crate::executor::pool::failed_chains(),
                        unreached: crate::executor::pool::unreached_chains(),
                        rate_limited: crate::executor::pool::rate_limited_chains(),
                    }
                })
                .await;
            host.update(cx, |host, cx| host.health_read(health, cx))
                .ok();
        })
        .detach();
    }

    fn health_read(&mut self, health: ChainHealth, cx: &mut Context<Self>) {
        if health != self.chain_health {
            self.chain_health = health;
            cx.notify();
        }
        // While something is down, look again: the page may have stopped
        // asking, and the notice must go by itself once the chain answers
        // anyone.
        let down = !self.chain_health.failed.is_empty() || !self.chain_health.unreached.is_empty();
        if down && !self.health_watching {
            self.health_watching = true;
            cx.spawn(async move |host, cx| {
                cx.background_executor().timer(HEALTH_RECHECK).await;
                host.update(cx, |host, cx| {
                    host.health_watching = false;
                    host.refresh_health(cx);
                })
                .ok();
            })
            .detach();
        }
    }

    /// Is `chain_id` unreachable, by the pool's last word?
    #[must_use]
    pub fn chain_unreachable(&self, chain_id: u32) -> bool {
        chain_down(chain_id, &self.chain_health)
    }

    /// Is the notice's Retry for `chain_id` still out?
    #[must_use]
    pub fn retrying_chain(&self, chain_id: u32) -> bool {
        self.retrying == Some(chain_id)
    }

    /// The notice's Retry (spec 082 RF4): one `eth_blockNumber` through the
    /// pool — busy until it settles, a second tap ignored — then the pool's
    /// word again: a good read clears the notice, a failed one leaves it.
    pub fn retry_chain(&mut self, chain_id: u32, cx: &mut Context<Self>) {
        if self.retrying.is_some() {
            return;
        }
        self.retrying = Some(chain_id);
        cx.notify();
        cx.spawn(async move |host, cx| {
            let answered = cx
                .background_executor()
                .spawn(async move {
                    crate::executor::pool::call(chain_id, "eth_blockNumber", serde_json::json!([]))
                        .is_ok_and(|body| body.get("result").is_some())
                })
                .await;
            crate::diag::vlog!(
                "chain notice",
                "retry chain={chain_id} → {}",
                if answered { "ok" } else { "failed" }
            );
            host.update(cx, |host, cx| {
                host.retrying = None;
                host.refresh_health(cx);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// What the page takes away, exactly once.
    pub fn take_orders(&mut self) -> Vec<SigningOrder> {
        std::mem::take(&mut self.orders)
    }

    /// The view of the one tab, if the machine has heard of it.
    #[must_use]
    pub fn tab(&self) -> Option<&vela_core::app::dapp_browser::DbrTabView> {
        self.view.tabs.iter().find(|tab| tab.tab == BROWSER_TAB)
    }
}

// ---------------------------------------------------------------------------
// Holding navigation while a request is open (spec 082 RD1, ruling 4, W14)
// ---------------------------------------------------------------------------

/// Whether the one webview must stay on its page: a connect or a signing
/// request of it is open, or one is waiting in line. The request's page is
/// what would be retired by a navigation — the core answers 4900 for a
/// document that goes — so the person's glance at another tab is held
/// instead (ruling 4).
#[must_use]
pub fn holds_navigation(view: &DbrView) -> bool {
    view.consent.is_some() || view.signing.is_some() || view.queued_signing > 0
}

/// Every way the page asks the one webview to go somewhere else — the one
/// funnel `WalletPage::browser_go` runs them through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Go {
    /// Another tab of the strip, with the address it remembers (`None`: a
    /// start-page tab).
    Tab {
        id: String,
        url: Option<String>,
    },
    /// The strip's +.
    NewTab,
    /// Enter in the address bar, with what it resolved to.
    Typed(String),
    /// A favourite or a Recents row.
    Open(String),
    /// The site menu's "Open in a new tab".
    OpenInNewTab(String),
    Reload,
    Back,
    Forward,
}

impl Go {
    fn word(&self) -> &'static str {
        match self {
            Self::Tab { .. } => "tab",
            Self::NewTab => "new tab",
            Self::Typed(_) => "enter",
            Self::Open(_) => "open",
            Self::OpenInNewTab(_) => "open in new tab",
            Self::Reload => "reload",
            Self::Back => "back",
            Self::Forward => "forward",
        }
    }
}

/// What a [`Go`] does.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Went {
    /// Held: a request is open. Nothing navigates; the request's column comes
    /// forward and the bar says why.
    Held,
    /// Nothing to do: the tab asked for is the one on screen (a re-click used
    /// to reload it, which cancelled whatever it had asked).
    AlreadyThere,
    /// Go.
    Go,
}

/// The one rule for every navigation the page starts (RD1): held while a
/// request is open; a click on the tab already shown is nothing.
#[must_use]
pub fn went(go: &Go, holds: bool, shown_tab: Option<&str>) -> Went {
    if let Go::Tab { id, .. } = go
        && shown_tab == Some(id.as_str())
    {
        return Went::AlreadyThere;
    }
    if holds {
        crate::diag::vlog!("browser", "navigation held ({})", go.word());
        return Went::Held;
    }
    Went::Go
}

/// The tab the strip lights (spec 082 RD6): the one whose page is in the
/// webview while a page shows; over the start page, the selected tab only
/// when it IS a start-page tab. A restored tab waits unlit — the start page
/// drawn under a lit site tab was G2's "this tab" over nothing.
#[must_use]
pub fn lit_tab(
    view: &vela_core::app::explore_sites::ExploreView,
    shown: Option<&str>,
    browsing: bool,
) -> Option<String> {
    if browsing {
        return shown.map(str::to_owned);
    }
    let selected = view.selected_tab.as_deref()?;
    view.tabs
        .iter()
        .find(|tab| tab.id == selected && tab.url.is_none())
        .map(|tab| tab.id.clone())
}

/// Which tab an address typed or picked opens in (RD6): the page on screen;
/// over the start page, the selected start-page tab; otherwise `None` — a new
/// tab, so a restored tab waiting unlit is left intact.
#[must_use]
pub fn open_target(
    view: &vela_core::app::explore_sites::ExploreView,
    shown: Option<&str>,
    browsing: bool,
) -> Option<String> {
    if browsing && let Some(shown) = shown {
        return Some(shown.to_owned());
    }
    lit_tab(view, None, false)
}

/// Where a page's own report — its title, its address — is filed (RD6, RJ5).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MetaTarget {
    /// Rename this tab: its document sent it.
    Tab(String),
    /// No tab has this page: open one for it.
    NewTab,
    /// It came from the page of a tab already closed, still in the webview
    /// until the next tab's load commits: nobody's.
    Nobody,
}

/// The tab a page's report belongs to: the tab whose document the webview
/// holds (RJ5) — never the tab on screen while it is veiled, whose own page
/// has not committed yet, and never a new tab's start page. With no owner
/// known, the RD6 rule ([`open_target`]).
#[must_use]
pub fn meta_target(
    view: &vela_core::app::explore_sites::ExploreView,
    doc_tab: Option<&str>,
    shown: Option<&str>,
    browsing: bool,
) -> MetaTarget {
    match doc_tab {
        Some(doc) if view.tabs.iter().any(|tab| tab.id == doc) => MetaTarget::Tab(doc.to_owned()),
        Some(_) => MetaTarget::Nobody,
        None => open_target(view, shown, browsing).map_or(MetaTarget::NewTab, MetaTarget::Tab),
    }
}

/// What the bar names (RE1): the committed document `shown`, a load pending,
/// or the failure a panel is up for — the core's `address_bar` over the
/// watch.
#[must_use]
pub fn bar_of(shown: Option<&str>, watch: &LoadWatch) -> browser_load::AddressBar {
    let pending = watch.url.as_deref().filter(|_| watch.loading);
    let failed = watch.url.as_deref().filter(|_| watch.failure.is_some());
    browser_load::address_bar(shown, pending, failed)
}

/// Back, forward and reload (RD6, RJ5): what the engine says it can do while
/// a page is in front; all off on the start page.
///
/// One webview serves every tab, so its history is every tab's. `back_floor`
/// is the shown tab's own floor — the back-list length at its first commit
/// — and `None` while the tab has no document of its own yet (veiled): then
/// nothing behind or ahead is this tab's, and a reload would reload another
/// tab's page. Back acts only above the floor; Forward needs none once the
/// tab has committed (a new load truncates WebKit's forward list); reload
/// over a failure panel is its Retry. `back_len` `None` (WebView2 cannot say)
/// leaves Back to the engine alone.
#[must_use]
pub fn nav_enabled(
    browsing: bool,
    can_back: bool,
    can_forward: bool,
    back_len: Option<usize>,
    back_floor: Option<usize>,
    failed: bool,
) -> [bool; 3] {
    let own = back_floor.is_some();
    let above_floor = match (back_len, back_floor) {
        (Some(len), Some(floor)) => len > floor,
        (None, Some(_)) => true,
        (_, None) => false,
    };
    [
        browsing && can_back && above_floor,
        browsing && can_forward && own,
        browsing && (own || failed),
    ]
}

/// Whether the column hides the webview (spec 082 RJ5, G42): a page is
/// wanted, and the one webview still holds another tab's document — a new
/// or restored tab before its own load commits. Showing that page under
/// the new tab's host was the G28 class: the bar naming a site the page is
/// not, with its connection dot. `doc_tab` `None`: nothing of any tab is
/// there to hide (the view is new, or blank).
#[must_use]
pub fn veiled(shown_tab: Option<&str>, doc_tab: Option<&str>, browsing: bool) -> bool {
    browsing && doc_tab.is_some() && shown_tab != doc_tab
}

/// Which tab's document the one webview holds, and the shown tab's Back
/// floor (spec 082 RJ5). Pure: the page reports the wallet's own requests,
/// the commits and the tab it shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TabDoc {
    /// The tab whose document the webview holds.
    pub doc_tab: Option<String>,
    /// The shown tab's floor: the back-list length at its first commit.
    pub back_floor: Option<usize>,
    /// The tab the wallet's last own load was asked for, until it commits.
    asked_for: Option<Option<String>>,
    /// The shown tab changed: its next commit sets a new floor.
    floor_due: bool,
}

impl TabDoc {
    /// The page shows another tab.
    pub fn shown_changed(&mut self) {
        self.floor_due = true;
    }

    /// The wallet asked the engine for a load (`Load::Requested`) while
    /// `shown` was on screen. Heard in platform order, so a commit the old
    /// page reported before this request never counts for `shown`.
    pub fn requested(&mut self, shown: Option<&str>) {
        self.asked_for = Some(shown.map(str::to_owned));
    }

    /// A document committed (`Load::Started`), with the back list as the
    /// engine has it now. The shown tab owns it when its own load was asked;
    /// a page's own navigation stays with the tab that already owned the
    /// document.
    pub fn committed(&mut self, shown: Option<&str>, back_len: Option<usize>) {
        let for_shown = self
            .asked_for
            .as_ref()
            .is_some_and(|asked| asked.as_deref() == shown);
        if !for_shown {
            return;
        }
        self.asked_for = None;
        if self.doc_tab.as_deref() != shown || self.floor_due {
            self.doc_tab = shown.map(str::to_owned);
            self.back_floor = Some(back_len.unwrap_or(0));
            self.floor_due = false;
        }
    }

    /// A page the webview holds was filed under a tab the page opened for
    /// it, with no load of the wallet's heard (RD6's new tab): that tab owns
    /// it, from here.
    pub fn adopted(&mut self, tab: Option<&str>, back_len: Option<usize>) {
        if self.doc_tab.is_none() && tab.is_some() {
            self.doc_tab = tab.map(str::to_owned);
            self.back_floor = Some(back_len.unwrap_or(0));
            self.floor_due = false;
        }
    }

    /// [`veiled`] for this webview.
    #[must_use]
    pub fn veiled(&self, shown: Option<&str>, browsing: bool) -> bool {
        veiled(shown, self.doc_tab.as_deref(), browsing)
    }

    /// The shown tab's floor, or `None` while it has no document of its own.
    #[must_use]
    pub fn floor(&self, shown: Option<&str>) -> Option<usize> {
        self.back_floor
            .filter(|_| self.doc_tab.is_some() && self.doc_tab.as_deref() == shown)
    }
}

// ---------------------------------------------------------------------------
// The page's load (spec 079 US3, spec 082 RD3–RD7): the core's `LoadWatch`,
// driven by the page's timers, the probe and WebKit's own state
// ---------------------------------------------------------------------------

use vela_core::app::browser_load::{
    self, Asked, EngineSample, EngineVerdict, GIVE_UP_MS, LoadWatch, Probed, RetryAction,
    WATCHDOG_MS,
};

use crate::explore::probe::{ProbeAnswer, verdict_word};

/// What the page does next for its load.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadStep {
    /// Tell the engine to load this address — an attempt the watch asked for.
    Navigate(String),
    /// Run [`LoadDriver::watchdog`] for this load after `after_ms`.
    Watchdog { generation: u64, after_ms: u32 },
    /// Probe `url` off the frame; hand the answer to [`LoadDriver::probed`].
    Probe { generation: u64, url: String },
    /// Run [`LoadDriver::give_up`] for this load after `after_ms`.
    GiveUp { generation: u64, after_ms: u32 },
    /// Run [`LoadDriver::retry_fired`] for this load after `after_ms`.
    Retry { generation: u64, after_ms: u32 },
}

/// The page's load: the core's watch, and the log lines it earns (contract
/// §15). Pure — the page turns each [`LoadStep`] into a timer, a probe or a
/// navigation — so a stream of engine samples can be driven in a test.
#[derive(Clone, Debug, Default)]
pub struct LoadDriver {
    pub watch: LoadWatch,
}

fn host(url: &str) -> String {
    crate::diag::host_of(url)
}

fn how_word(asked: Asked) -> &'static str {
    match asked {
        Asked::Navigation => "navigation",
        Asked::Retry => "retry",
        Asked::AutoRetry => "auto",
        Asked::Page => "page",
    }
}

fn millis_until(at_ms: f64, now_ms: f64) -> u32 {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "clamped to a u32 of milliseconds"
    )]
    let ms = (at_ms - now_ms).clamp(0.0, f64::from(u32::MAX)) as u32;
    ms
}

impl LoadDriver {
    fn watched_host(&self) -> String {
        self.watch.url.as_deref().map(host).unwrap_or_default()
    }

    /// The engine was asked to load `url` (the wallet's own request).
    pub fn requested(&mut self, url: &str, now_ms: f64) -> Vec<LoadStep> {
        let how = self.watch.next_asked.unwrap_or(Asked::Navigation);
        let Some(generation) = self.watch.requested(url, now_ms) else {
            return Vec::new();
        };
        crate::diag::vlog!(
            "browser",
            "asked host={} gen={generation} how={}",
            host(url),
            how_word(how)
        );
        vec![LoadStep::Watchdog {
            generation,
            after_ms: WATCHDOG_MS,
        }]
    }

    /// A document committed at `url`.
    pub fn committed(&mut self, url: &str) {
        let waiting = self.watch.loading || self.watch.failure.is_some();
        self.watch.committed(url);
        if waiting && self.watch.committed {
            crate::diag::vlog!(
                "browser",
                "committed host={} gen={}",
                host(url),
                self.watch.generation
            );
        }
    }

    /// The load of `url` finished.
    pub fn finished(&mut self, url: &str) {
        let committed = self.watch.committed;
        self.watch.finished(url);
        if committed && !self.watch.committed {
            crate::diag::vlog!("browser", "finished host={}", host(url));
        }
    }

    /// One poll of the engine (RD3, RD7).
    pub fn engine(&mut self, sample: &EngineSample, now_ms: f64) -> Vec<LoadStep> {
        match self.watch.engine(sample, now_ms) {
            EngineVerdict::Nothing => Vec::new(),
            EngineVerdict::PageStarted { generation, url } => {
                crate::diag::vlog!(
                    "browser",
                    "asked host={} gen={generation} how=page",
                    host(&url)
                );
                vec![LoadStep::Watchdog {
                    generation,
                    after_ms: WATCHDOG_MS,
                }]
            }
            EngineVerdict::StoppedWithoutCommit { generation, url } => {
                crate::diag::vlog!(
                    "browser",
                    "engine stopped without commit host={} gen={generation}",
                    host(&url)
                );
                vec![LoadStep::Probe { generation, url }]
            }
        }
    }

    /// [`WATCHDOG_MS`] after load `generation` was asked for.
    pub fn watchdog(&mut self, generation: u64) -> Vec<LoadStep> {
        let Some(url) = self.watch.watchdog(generation) else {
            return Vec::new();
        };
        crate::diag::vlog!(
            "browser",
            "watchdog host={} gen={generation} (no commit in {WATCHDOG_MS} ms)",
            host(&url)
        );
        vec![LoadStep::Probe { generation, url }]
    }

    /// The probe of load `generation` answered.
    pub fn probed(&mut self, generation: u64, answer: &ProbeAnswer, now_ms: f64) -> Vec<LoadStep> {
        if generation == self.watch.generation {
            crate::diag::vlog!(
                "browser",
                "probe host={} gen={generation} verdict={} route={}",
                self.watched_host(),
                verdict_word(answer.verdict),
                answer
                    .proxy
                    .as_ref()
                    .map_or_else(|| "system".to_owned(), |failure| failure.proxy.clone())
            );
        }
        match self.watch.probed(generation, answer.verdict) {
            Probed::Ignored => Vec::new(),
            Probed::WaitUntil(at_ms) => vec![LoadStep::GiveUp {
                generation,
                after_ms: millis_until(at_ms, now_ms),
            }],
            // RD3: a certificate verdict while WebKit is still loading is not
            // believed — and not a pass either: the 20 s give-up stays armed.
            Probed::Deferred => vec![LoadStep::GiveUp {
                generation,
                after_ms: millis_until(self.watch.requested_at_ms + f64::from(GIVE_UP_MS), now_ms),
            }],
            Probed::Failed => self.failed(),
        }
    }

    /// [`GIVE_UP_MS`] after the load was asked for.
    pub fn give_up(&mut self, generation: u64) -> Vec<LoadStep> {
        if self.watch.give_up(generation) {
            self.failed()
        } else {
            Vec::new()
        }
    }

    fn failed(&mut self) -> Vec<LoadStep> {
        let class = self
            .watch
            .failure
            .as_ref()
            .map_or_else(String::new, |failure| {
                format!("{:?}", failure.class).to_lowercase()
            });
        crate::diag::vlog!(
            "browser",
            "failed host={} class={class}{}",
            self.watched_host(),
            if self.watch.page_initiated {
                " (the page's own load: retried by hand only)"
            } else {
                ""
            }
        );
        self.schedule()
    }

    fn schedule(&mut self) -> Vec<LoadStep> {
        self.watch
            .schedule_retry()
            .map(|(generation, after_ms)| LoadStep::Retry {
                generation,
                after_ms,
            })
            .into_iter()
            .collect()
    }

    fn attempt(&mut self, action: RetryAction) -> Vec<LoadStep> {
        match action {
            RetryAction::Load(url) => {
                crate::diag::vlog!(
                    "browser",
                    "retry host={} attempt={}",
                    host(&url),
                    self.watch.attempt
                );
                vec![LoadStep::Navigate(url)]
            }
            // W7: WebKit is still on this load — no second request, and the
            // attempt comes due again on the same wait.
            RetryAction::EngineStillLoading => {
                crate::diag::vlog!(
                    "browser",
                    "retry host={} skipped (engine still loading)",
                    self.watched_host()
                );
                self.schedule()
            }
            RetryAction::NotInFront | RetryAction::Nothing => Vec::new(),
        }
    }

    /// An automatic attempt for load `generation` came due.
    pub fn retry_fired(&mut self, generation: u64, in_front: bool) -> Vec<LoadStep> {
        let action = self.watch.retry_fired(generation, in_front);
        self.attempt(action)
    }

    /// The page is in front again: an attempt that came due while it was not.
    pub fn take_due(&mut self) -> Vec<LoadStep> {
        let action = self.watch.take_due();
        self.attempt(action)
    }

    /// The panel's Retry.
    pub fn retry(&mut self) -> Vec<LoadStep> {
        self.watch
            .retry()
            .map(|url| {
                crate::diag::vlog!("browser", "retry host={} (by hand)", host(&url));
                LoadStep::Navigate(url)
            })
            .into_iter()
            .collect()
    }

    /// The network came back (spec 082 RE3, T069): a failed page whose class
    /// a returning network can heal is loaded again, once — never a load the
    /// page itself started (it may have been a POST; RD7).
    pub fn network_came_back(&mut self) -> Vec<LoadStep> {
        let eligible = self
            .watch
            .failure
            .as_ref()
            .is_some_and(|failure| browser_load::retry_when_network_returns(failure.class))
            && !self.watch.page_initiated
            && !self.watch.loading;
        if !eligible {
            return Vec::new();
        }
        crate::diag::vlog!(
            "browser",
            "network came back; loading host={} again",
            self.watched_host()
        );
        self.retry()
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use vela_core::app::dapp_permissions::DpermGrant;
    use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

    use super::*;
    use crate::executor::dapp_browser::Forwarded;
    use crate::executor::storage;

    const DAPP: &str = "https://dapp.example";
    const OTHER: &str = "https://other.example";
    const A1: &str = "0x1111111111111111111111111111111111111111";
    const A2: &str = "0x2222222222222222222222222222222222222222";

    fn seed_grant(origin: &str, address: &str, chain_id: u32) {
        let grant = DpermGrant {
            origin: origin.to_owned(),
            address: address.to_owned(),
            chain_id,
            granted_at_ms: 1_757_000_000_000.0,
        };
        let Ok(value) = serde_json::to_value(grant) else {
            unreachable!("a grant serialises");
        };
        if storage::write_value(&format!("vela.perm.{origin}"), value).is_err() {
            unreachable!("could not seed the grant");
        }
    }

    /// Booted the way `BrowserHost::new` boots it: the wallet, the networks,
    /// then the store.
    fn booted() -> BrowserDriver {
        let mut driver = BrowserDriver::new();
        driver.dispatch(Event::AccountsUpdated {
            addresses: Some(vec![A1.to_owned(), A2.to_owned()]),
        });
        driver.dispatch(Event::AccountSwitched {
            address: A1.to_owned(),
            now_ms: 1_757_000_000_000.0,
        });
        driver.dispatch(Event::NetworksChanged {
            chain_ids: vec![1, 100, 8453],
        });
        let out = driver.dispatch(Event::Start);
        assert!(out.is_empty(), "listing the sites is answered inline");
        assert!(driver.view().ready);
        driver
    }

    fn page(driver: &mut BrowserDriver, origin: &str, message: Value) -> Vec<Outbound> {
        driver.dispatch(Event::PageMessage {
            tab: BROWSER_TAB.to_owned(),
            frame_origin: format!("{origin}/app"),
            is_main_frame: true,
            message_json: message.to_string(),
        })
    }

    fn ask(
        driver: &mut BrowserDriver,
        origin: &str,
        doc: &str,
        id: &str,
        method: &str,
        params: Value,
    ) -> Vec<Outbound> {
        page(
            driver,
            origin,
            json!({"t":"req","doc":doc,"id":id,"method":method,"params":params}),
        )
    }

    /// Every delivery, parsed, with the tab it was addressed to.
    fn delivered(out: &[Outbound]) -> Vec<(String, Value)> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Deliver { tab, message_json } => Some((
                    tab.clone(),
                    serde_json::from_str(message_json).unwrap_or(Value::Null),
                )),
                _ => None,
            })
            .collect()
    }

    fn answers(out: &[Outbound]) -> Vec<Value> {
        delivered(out)
            .into_iter()
            .filter(|(_, message)| message["dir"] == "res")
            .map(|(_, message)| message)
            .collect()
    }

    fn forwarded(out: &[Outbound]) -> Vec<Forwarded> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Signing(SigningOrder::Forward(forward)) => Some(forward.clone()),
                _ => None,
            })
            .collect()
    }

    fn cancelled(out: &[Outbound]) -> Vec<String> {
        out.iter()
            .filter_map(|next| match next {
                Outbound::Signing(SigningOrder::Cancel { id, .. }) => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    fn signed(driver: &mut BrowserDriver, id: &str, result: &str) -> Vec<Outbound> {
        driver.dispatch(Event::SigningAnswered {
            tab: BROWSER_TAB.to_owned(),
            id: id.to_owned(),
            payload: SignResponsePayload::Ok {
                result: Some(result.to_owned()),
            },
            user_op_hash: None,
        })
    }

    /// hello → a request → one answer, into the webview that asked and
    /// addressed to the document that asked.
    #[test]
    fn a_request_is_answered_into_the_document_that_asked() {
        storage::tests::with_temp_state("dbr-host-hello", || {
            let mut driver = booted();
            let hello = page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            assert!(delivered(&hello).is_empty(), "a hello is not answered");
            let out = ask(&mut driver, DAPP, "d1", "1", "eth_chainId", json!([]));
            let delivered = delivered(&out);
            assert_eq!(delivered.len(), 1);
            let (tab, message) = &delivered[0];
            assert_eq!(tab, BROWSER_TAB);
            assert_eq!(message["doc"], json!("d1"), "the bridge drops any other");
            assert_eq!(message["id"], json!("1"));
            assert_eq!(
                message["result"],
                json!("0x1"),
                "a new site starts on Ethereum"
            );
            // And an unknown method is 4200, never the 4900 this shell used
            // to answer with.
            let out = ask(&mut driver, DAPP, "d1", "2", "eth_sign", json!([]));
            assert_eq!(answers(&out)[0]["error"]["code"], json!(4200));
        });
    }

    /// A signature goes to the column with the site's OWN chain and the
    /// address the site was shown — never the column-wide chain, never
    /// whichever account happens to be active.
    #[test]
    fn a_signature_is_forwarded_with_the_granted_address_and_the_sites_chain() {
        storage::tests::with_temp_state("dbr-host-forward", || {
            seed_grant(DAPP, A1, 100);
            let _ = storage::write_value("vela.chain.https://dapp.example", json!(8453));
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(
                &mut driver,
                DAPP,
                "d1",
                "7",
                "personal_sign",
                json!(["0x00", A1]),
            );
            assert!(answers(&out).is_empty(), "the column answers, later");
            assert_eq!(
                forwarded(&out),
                vec![Forwarded {
                    tab: BROWSER_TAB.to_owned(),
                    id: "7".to_owned(),
                    method: "personal_sign".to_owned(),
                    params_json: json!(["0x00", A1]).to_string(),
                    origin: DAPP.to_owned(),
                    chain_id: 8453,
                    granted_address: A1.to_owned(),
                }]
            );
            let out = signed(&mut driver, "7", "0xsig");
            assert_eq!(answers(&out)[0]["result"], json!("0xsig"));
        });
    }

    /// The desktop used to DROP the first request when a second arrived. Now
    /// the second waits in line and opens when the first is answered.
    #[test]
    fn a_second_signature_waits_instead_of_being_dropped() {
        storage::tests::with_temp_state("dbr-host-queue", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let first = ask(&mut driver, DAPP, "d1", "1", "personal_sign", json!([]));
            let second = ask(&mut driver, DAPP, "d1", "2", "personal_sign", json!([]));
            assert_eq!(forwarded(&first).len(), 1);
            assert!(forwarded(&second).is_empty(), "one sheet at a time");
            assert!(answers(&second).is_empty(), "neither dropped nor refused");
            assert_eq!(driver.view().queued_signing, 1);

            let out = signed(&mut driver, "1", "0x11");
            assert_eq!(answers(&out)[0]["id"], json!("1"));
            let next = forwarded(&out);
            assert_eq!(next.len(), 1);
            assert_eq!(next[0].id, "2", "the one that waited opens next");

            // A column that cannot open (the wallet's own signing is up) says
            // so with -32002, and the line moves on rather than sticking.
            let out = driver.dispatch(Event::SigningAnswered {
                tab: BROWSER_TAB.to_owned(),
                id: "2".to_owned(),
                payload: SignResponsePayload::Err {
                    code: -32002,
                    kind: SignErrorKind::SubmitFailed,
                    message: Some("Another signing request is open".to_owned()),
                },
                user_op_hash: None,
            });
            assert_eq!(answers(&out)[0]["error"]["code"], json!(-32002));
            assert!(driver.view().signing.is_none());
        });
    }

    /// The page navigated away under an open sheet: its promise gets 4900
    /// once, and the column is told to close — nothing is delivered into the
    /// next document.
    #[test]
    fn a_navigation_under_an_open_sheet_cancels_it() {
        storage::tests::with_temp_state("dbr-host-cancel", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(
                &mut driver,
                DAPP,
                "d1",
                "1",
                "eth_sendTransaction",
                json!([{}]),
            );

            // Leaving Explore is not a navigation: nothing happens at all
            // until the webview reports one.
            let started = driver.dispatch(Event::NavigationStarted {
                tab: BROWSER_TAB.to_owned(),
                url: format!("{OTHER}/"),
            });
            assert!(started.is_empty(), "a load beginning settles nothing");
            let out = page(&mut driver, OTHER, json!({"t":"hello","doc":"d2"}));
            assert_eq!(cancelled(&out), vec!["1".to_owned()]);
            let settled = answers(&out);
            assert_eq!(settled.len(), 1);
            assert_eq!(settled[0]["error"]["code"], json!(4900));
            assert_eq!(settled[0]["doc"], json!("d1"), "addressed to the dead page");

            // The late answer from the column reaches nobody.
            let late = signed(&mut driver, "1", "0xlate");
            assert!(answers(&late).is_empty());
        });
    }

    // -- spec 082 T060: navigation is held while a request is open ----------

    #[test]
    fn a_request_holds_navigation() {
        storage::tests::with_temp_state("dbr-host-hold", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            assert!(!holds_navigation(&driver.view()), "nothing open");
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(&mut driver, DAPP, "d1", "1", "personal_sign", json!([]));
            assert!(holds_navigation(&driver.view()), "a signature is open");
            // A second waits in line: still held once the first is answered.
            ask(&mut driver, DAPP, "d1", "2", "personal_sign", json!([]));
            signed(&mut driver, "1", "0xsig");
            assert!(holds_navigation(&driver.view()), "one still in line");
            signed(&mut driver, "2", "0xsig2");
            assert!(!holds_navigation(&driver.view()), "all answered");
        });
        // A connect question holds too.
        storage::tests::with_temp_state("dbr-host-hold-consent", || {
            let mut driver = booted();
            page(&mut driver, OTHER, json!({"t":"hello","doc":"d1"}));
            ask(
                &mut driver,
                OTHER,
                "d1",
                "1",
                "eth_requestAccounts",
                json!([]),
            );
            assert!(driver.view().consent.is_some());
            assert!(holds_navigation(&driver.view()));
        });
    }

    /// Every way the page goes elsewhere is held while a request is open —
    /// and a click on the tab already shown is nothing, held or not.
    #[test]
    fn every_navigation_is_held_and_the_shown_tab_is_nothing() {
        let every = [
            Go::Tab {
                id: "t2".to_owned(),
                url: Some("https://example.com/".to_owned()),
            },
            Go::Tab {
                id: "t3".to_owned(),
                url: None,
            },
            Go::NewTab,
            Go::Typed("https://example.com/".to_owned()),
            Go::Open("https://app.uniswap.org/".to_owned()),
            Go::OpenInNewTab("https://app.uniswap.org/".to_owned()),
            Go::Reload,
            Go::Back,
            Go::Forward,
        ];
        for go in &every {
            assert_eq!(went(go, true, Some("t1")), Went::Held, "{go:?}");
            assert_eq!(went(go, false, Some("t1")), Went::Go, "{go:?}");
        }
        let shown = Go::Tab {
            id: "t1".to_owned(),
            url: Some("http://127.0.0.1:8137/".to_owned()),
        };
        assert_eq!(went(&shown, true, Some("t1")), Went::AlreadyThere);
        assert_eq!(
            went(&shown, false, Some("t1")),
            Went::AlreadyThere,
            "a re-click used to reload the page, cancelling its request"
        );
    }

    /// Closing the request's own tab is NOT held: its page goes, the core
    /// settles the request (4900 — the page that asked is gone, so there is
    /// no one left to deliver it to) and withdraws its sheet; nothing holds
    /// after (ruling 4 holds glances, not closes).
    #[test]
    fn closing_the_requests_own_tab_settles_it() {
        storage::tests::with_temp_state("dbr-host-close-own", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(
                &mut driver,
                DAPP,
                "d1",
                "1",
                "eth_sendTransaction",
                json!([{}]),
            );
            assert!(holds_navigation(&driver.view()));
            let out = driver.dispatch(Event::TabClosed {
                tab: BROWSER_TAB.to_owned(),
            });
            assert_eq!(cancelled(&out), vec!["1".to_owned()], "the sheet goes");
            assert!(
                answers(&out).is_empty(),
                "nothing is delivered into a closed page"
            );
            assert!(!holds_navigation(&driver.view()));
            // The column's late answer reaches nobody.
            assert!(answers(&signed(&mut driver, "1", "0xlate")).is_empty());
        });
    }

    /// Spec 082 RD11 (G11, T065): the consent names the account a grant
    /// would be made for and the site's network — and the network can be
    /// changed before the answer, which the grant then carries.
    #[test]
    fn the_consent_carries_the_account_and_a_changeable_network() {
        storage::tests::with_temp_state("dbr-host-consent-rows", || {
            let mut driver = booted();
            page(&mut driver, OTHER, json!({"t":"hello","doc":"d1"}));
            ask(
                &mut driver,
                OTHER,
                "d1",
                "1",
                "eth_requestAccounts",
                json!([]),
            );
            let consent = driver
                .view()
                .consent
                .unwrap_or_else(|| unreachable!("the site asked"));
            assert_eq!(consent.address.as_deref(), Some(A1), "the active account");
            let before = consent.chain_id;
            driver.dispatch(Event::SiteChainPicked {
                origin: OTHER.to_owned(),
                chain_id: 100,
            });
            let consent = driver
                .view()
                .consent
                .unwrap_or_else(|| unreachable!("still asking"));
            assert_eq!(
                consent.chain_id, 100,
                "changed from {before} before the answer"
            );
        });
    }

    // -- spec 082 T062, T063: the bar, the lit tab, the nav buttons ---------

    fn explore(
        tabs: &[(&str, Option<&str>)],
        selected: Option<&str>,
    ) -> vela_core::app::explore_sites::ExploreView {
        vela_core::app::explore_sites::ExploreView {
            favorites: Vec::new(),
            groups: Vec::new(),
            tabs: tabs
                .iter()
                .map(|(id, url)| vela_core::app::explore_sites::ExploreTab {
                    id: (*id).to_owned(),
                    url: url.map(str::to_owned),
                    title: String::new(),
                    host: url.map(crate::diag::host_of).unwrap_or_default(),
                })
                .collect(),
            selected_tab: selected.map(str::to_owned),
            favorites_hidden: false,
            recent_hidden: false,
            favorites_full: false,
            tabs_full: false,
            ready: true,
        }
    }

    /// G2 (RD6): launched with a restored Uniswap tab, the start page shows,
    /// the tab waits unlit, and every nav button is off. Enter over the start
    /// page opens a NEW tab and leaves the restored one intact.
    #[test]
    fn a_restored_tab_waits_unlit_and_enter_opens_a_new_one() {
        let view = explore(&[("t1", Some("https://app.uniswap.org/"))], Some("t1"));
        assert_eq!(
            lit_tab(&view, None, false),
            None,
            "nothing lit over the start page"
        );
        assert_eq!(
            nav_enabled(false, true, true, Some(3), Some(0), false),
            [false; 3],
            "all nav off"
        );
        assert_eq!(open_target(&view, None, false), None, "a new tab");
        // A start-page tab selected: lit, and it is where Enter goes.
        let view = explore(
            &[("t1", Some("https://app.uniswap.org/")), ("t2", None)],
            Some("t2"),
        );
        assert_eq!(lit_tab(&view, None, false).as_deref(), Some("t2"));
        assert_eq!(open_target(&view, None, false).as_deref(), Some("t2"));
        // A page on screen: its tab, lit, and where Enter goes.
        assert_eq!(lit_tab(&view, Some("t1"), true).as_deref(), Some("t1"));
        assert_eq!(open_target(&view, Some("t1"), true).as_deref(), Some("t1"));
        assert_eq!(
            nav_enabled(true, false, true, Some(0), Some(0), false),
            [false, true, true]
        );
    }

    // -- spec 082 RJ5 (G42): one webview, a veil and a per-tab Back floor ----

    /// DX14: a new tab typed while another tab's connected page is up. The
    /// webview holds the other tab's page until the new one commits: veiled
    /// (hidden, no chip, no chain), Back, Forward and reload dark; after the
    /// commit it is the new tab's, and Back stops at its first page.
    #[test]
    fn a_new_tab_is_veiled_until_its_own_page_commits() {
        let mut doc = TabDoc::default();
        // The test dApp on tab A, two pages deep.
        doc.shown_changed();
        doc.requested(Some("A"));
        doc.committed(Some("A"), Some(0));
        assert!(!doc.veiled(Some("A"), true));
        // + then Enter on app.uniswap.org: tab N is shown, its load asked.
        doc.shown_changed();
        doc.requested(Some("N"));
        assert!(doc.veiled(Some("N"), true), "A's page under N's host");
        assert_eq!(doc.floor(Some("N")), None);
        assert_eq!(
            nav_enabled(true, true, true, Some(2), doc.floor(Some("N")), false),
            [false, false, false],
            "nothing behind, ahead or under a veiled tab is its own"
        );
        // Over a failure panel reload is the Retry, veiled or not.
        assert!(nav_enabled(true, true, false, Some(2), None, true)[2]);
        // Its page commits with three entries behind it — all A's.
        doc.committed(Some("N"), Some(3));
        assert!(!doc.veiled(Some("N"), true));
        assert_eq!(doc.doc_tab.as_deref(), Some("N"));
        assert_eq!(
            nav_enabled(true, true, false, Some(3), doc.floor(Some("N")), false),
            [false, false, true],
            "Back stops at the tab's first page"
        );
        // Not browsing (the start page): nothing is veiled.
        assert!(!doc.veiled(Some("X"), false));
    }

    /// DX11: a restored tab clicked while another tab's page is up — veiled
    /// until its commit, then Back is dark at the floor; an SPA's pushState
    /// raises the back list above it and Back is this tab's again.
    #[test]
    fn a_restored_tab_stops_back_at_its_floor_and_an_spa_lifts_it() {
        let mut doc = TabDoc::default();
        doc.requested(Some("test"));
        doc.committed(Some("test"), Some(0));
        // Click the restored Uniswap tab.
        doc.shown_changed();
        doc.requested(Some("uni"));
        assert!(doc.veiled(Some("uni"), true));
        doc.committed(Some("uni"), Some(1));
        let floor = doc.floor(Some("uni"));
        assert_eq!(floor, Some(1));
        assert!(
            !nav_enabled(true, true, false, Some(1), floor, false)[0],
            "Back would cross into the test dApp's tab"
        );
        // Uniswap pushes a route: one more entry, this tab's own.
        assert!(nav_enabled(true, true, false, Some(2), floor, false)[0]);
        // A link inside the page commits without a request of the wallet's:
        // the tab keeps its document and its floor.
        doc.committed(Some("uni"), Some(3));
        assert_eq!(doc.floor(Some("uni")), Some(1));
        // Back to the test dApp's tab: a new floor at its next commit.
        doc.shown_changed();
        doc.requested(Some("test"));
        assert!(doc.veiled(Some("test"), true));
        doc.committed(Some("test"), Some(4));
        assert_eq!(doc.floor(Some("test")), Some(4));
    }

    /// A commit the old page reported before the wallet asked for the new
    /// tab's load (heard first, in platform order) is not the new tab's.
    #[test]
    fn a_commit_heard_before_the_request_stays_with_the_old_tab() {
        let mut doc = TabDoc::default();
        doc.requested(Some("A"));
        doc.committed(Some("A"), Some(0));
        doc.shown_changed();
        // A's own navigation commits; then the wallet's request for B.
        doc.committed(Some("B"), Some(1));
        assert_eq!(doc.doc_tab.as_deref(), Some("A"));
        assert!(doc.veiled(Some("B"), true));
        doc.requested(Some("B"));
        doc.committed(Some("B"), Some(2));
        assert_eq!(doc.doc_tab.as_deref(), Some("B"));
        // WebView2 cannot read the back list: the floor is there, the engine
        // alone decides Back.
        let mut windows = TabDoc::default();
        windows.requested(Some("A"));
        windows.committed(Some("A"), None);
        assert!(nav_enabled(true, true, false, None, windows.floor(Some("A")), false)[0]);
    }

    /// A page's own report (its title) is filed under the tab whose page it
    /// is: the document's tab while another is veiled or the start page is
    /// up; nobody's once that tab is closed; the RD6 rule with no owner known.
    #[test]
    fn a_page_report_names_the_tab_whose_document_sent_it() {
        let view = explore(&[("a", Some("https://a.example/")), ("n", None)], Some("n"));
        assert_eq!(
            meta_target(&view, Some("a"), Some("n"), true),
            MetaTarget::Tab("a".to_owned()),
            "not the veiled tab"
        );
        assert_eq!(
            meta_target(&view, Some("a"), None, false),
            MetaTarget::Tab("a".to_owned()),
            "not the new tab's start page"
        );
        assert_eq!(
            meta_target(&view, Some("gone"), Some("n"), true),
            MetaTarget::Nobody
        );
        assert_eq!(
            meta_target(&view, None, Some("a"), true),
            MetaTarget::Tab("a".to_owned())
        );
        let restored = explore(&[("t1", Some("https://app.uniswap.org/"))], Some("t1"));
        assert_eq!(meta_target(&restored, None, None, true), MetaTarget::NewTab);
        let mut doc = TabDoc::default();
        doc.adopted(Some("t2"), Some(0));
        assert_eq!(doc.doc_tab.as_deref(), Some("t2"));
    }

    /// G30 (RE1, RD5): a first load that failed names the failed host with NO
    /// lock — nothing of it is on screen — and a committed page its own host
    /// with its lock; a load pending in an empty tab names its host unlocked.
    #[test]
    fn the_bar_names_the_failed_host_without_a_lock() {
        use vela_core::app::browser_load::BarLock;
        let mut load = LoadDriver::default();
        load.requested("https://app.uniswap.org/#/swap", 0.0);
        let pending = bar_of(None, &load.watch);
        assert_eq!(
            (pending.host.as_str(), pending.lock),
            ("app.uniswap.org", BarLock::None)
        );
        load.watchdog(load.watch.generation);
        let generation_n = load.watch.generation;
        load.probed(
            generation_n,
            &ProbeAnswer {
                verdict: Err(vela_core::app::browser_load::probe_code::REFUSED),
                proxy: None,
            },
            3_500.0,
        );
        assert!(load.watch.failure.is_some());
        let failed = bar_of(None, &load.watch);
        assert_eq!(failed.host, "app.uniswap.org");
        assert_eq!(failed.lock, BarLock::None, "no lock over a failure panel");
        assert_eq!(
            failed.url, "https://app.uniswap.org/#/swap",
            "what editing starts from"
        );
        // A committed page: its host, locked.
        let page = bar_of(Some("https://app.uniswap.org/"), &LoadWatch::default());
        assert_eq!(
            (page.host.as_str(), page.lock),
            ("app.uniswap.org", BarLock::Closed)
        );
    }

    // -- spec 082 T059: WebKit's own load state first ------------------------

    fn sample(loading: bool, progress: f64, url: &str) -> EngineSample {
        EngineSample {
            loading,
            progress,
            url: Some(url.to_owned()),
        }
    }

    /// W7: a site whose first byte takes 9 s behind a blackholed probe. The
    /// probe times out at 8 s and the panel goes up; the retry that falls
    /// due at 10 s finds WebKit still on the load — or the page already
    /// committed at 9 s — and never asks for it a second time.
    #[test]
    fn a_slow_first_byte_is_asked_for_once() {
        const SITE: &str = "https://app.uniswap.org/";
        let mut load = LoadDriver::default();
        let mut navigations = 0;
        let steps = load.requested(SITE, 0.0);
        assert!(matches!(
            steps.as_slice(),
            [LoadStep::Watchdog {
                after_ms: 3_000,
                ..
            }]
        ));
        let generation_n = load.watch.generation;
        // WebKit is on it from the start, provisionally.
        for t in [250.0, 500.0, 2_750.0] {
            assert!(load.engine(&sample(true, 0.1, SITE), t).is_empty());
        }
        assert!(matches!(
            load.watchdog(generation_n).as_slice(),
            [LoadStep::Probe { .. }]
        ));
        // The probe's own 5 s run out at 8 s.
        let steps = load.probed(
            generation_n,
            &ProbeAnswer {
                verdict: Err(vela_core::app::browser_load::probe_code::TIMEOUT),
                proxy: None,
            },
            8_000.0,
        );
        let retry = steps.iter().find_map(|step| match step {
            LoadStep::Retry {
                generation,
                after_ms,
            } => Some((*generation, *after_ms)),
            _ => None,
        });
        assert!(load.watch.failure.is_some(), "the panel is up");
        // 10 s: WebKit is still loading — no second request.
        let (retry_gen, _) = retry.unwrap_or_else(|| unreachable!("an attempt is scheduled"));
        for step in load.retry_fired(retry_gen, true) {
            if let LoadStep::Navigate(_) = step {
                navigations += 1;
            }
        }
        assert_eq!(
            navigations, 0,
            "no second loadRequest while WebKit is on it"
        );
        // The page commits: the panel goes by itself.
        load.committed(SITE);
        assert!(load.watch.failure.is_none());
        assert!(load.engine(&sample(false, 1.0, SITE), 12_000.0).is_empty());
        load.finished(SITE);
        assert!(!load.watch.busy());
    }

    /// W18 (RD7): the page started a load of its own (a script set
    /// `location.href`) and it failed — a panel naming that host, with
    /// Retry by hand only: the original may have been a POST.
    #[test]
    fn a_page_started_load_gets_a_panel_and_no_automatic_retry() {
        const SITE: &str = "http://127.0.0.1:8137/";
        const AWAY: &str = "https://example.org/";
        let mut load = LoadDriver::default();
        load.requested(SITE, 0.0);
        load.committed(SITE);
        load.finished(SITE);
        assert!(load.engine(&sample(false, 1.0, SITE), 1_000.0).is_empty());
        let steps = load.engine(&sample(true, 0.1, AWAY), 2_000.0);
        let generation_n = match steps.as_slice() {
            [LoadStep::Watchdog { generation, .. }] => *generation,
            other => unreachable!("the page's load is watched: {other:?}"),
        };
        assert!(load.watch.page_initiated);
        assert_eq!(
            bar_of(Some(SITE), &load.watch).host,
            "127.0.0.1:8137",
            "the bar keeps the page"
        );
        // WebKit gives up with nothing committed: the probe says why.
        let steps = load.engine(&sample(false, 0.1, SITE), 6_000.0);
        assert!(matches!(steps.as_slice(), [LoadStep::Probe { .. }]));
        let steps = load.probed(
            generation_n,
            &ProbeAnswer {
                verdict: Err(vela_core::app::browser_load::probe_code::TIMEOUT),
                proxy: None,
            },
            7_000.0,
        );
        assert!(load.watch.failure.is_some(), "the panel is up");
        assert!(
            !steps
                .iter()
                .any(|step| matches!(step, LoadStep::Retry { .. })),
            "no automatic retry for a load the page started"
        );
        assert_eq!(bar_of(Some(SITE), &load.watch).host, "example.org");
        // Retry by hand still works.
        assert_eq!(load.retry(), vec![LoadStep::Navigate(AWAY.to_owned())]);
    }

    /// RD3: a certificate verdict from the probe while WebKit is still
    /// loading is not believed — and the 20 s give-up stays armed.
    #[test]
    fn a_deferred_verdict_keeps_the_give_up_armed() {
        const SITE: &str = "https://self-signed.example/";
        let mut load = LoadDriver::default();
        load.requested(SITE, 0.0);
        load.engine(&sample(true, 0.1, SITE), 500.0);
        let generation_n = load.watch.generation;
        load.watchdog(generation_n);
        let steps = load.probed(
            generation_n,
            &ProbeAnswer {
                verdict: Err(vela_core::app::browser_load::probe_code::TLS),
                proxy: None,
            },
            4_000.0,
        );
        assert_eq!(
            steps,
            vec![LoadStep::GiveUp {
                generation: generation_n,
                after_ms: 16_000
            }]
        );
        assert!(load.watch.failure.is_none());
        // Still nothing at 20 s and WebKit never got anywhere: a timeout.
        let steps = load.give_up(generation_n);
        assert!(load.watch.failure.is_some());
        assert!(
            steps
                .iter()
                .any(|step| matches!(step, LoadStep::Retry { .. }))
        );
    }

    // -- spec 082 T069: network-back parity ---------------------------------

    /// Three misses take the network offline; the first reach after them
    /// brings it back, and the failed page is loaded again — once. A load
    /// the page started, and a name that does not resolve, are not.
    #[test]
    fn three_misses_then_a_reach_retry_the_failed_page_once() {
        use vela_core::app::net_health::{NetEdge, NetHealth, net_health_step};
        const SITE: &str = "https://app.uniswap.org/";
        let failed_load = |code: i64| {
            let mut load = LoadDriver::default();
            load.requested(SITE, 0.0);
            let generation_n = load.watch.generation;
            load.watchdog(generation_n);
            load.probed(
                generation_n,
                &ProbeAnswer {
                    verdict: Err(code),
                    proxy: None,
                },
                3_500.0,
            );
            assert!(load.watch.failure.is_some());
            load
        };
        let mut load = failed_load(vela_core::app::browser_load::probe_code::CONNECT);
        let mut health = NetHealth {
            misses: 0,
            online: true,
        };
        let mut retries = 0;
        for reached in [false, false, false, true, true] {
            let (next, edge) = net_health_step(health, reached);
            health = next;
            if edge == Some(NetEdge::CameBack) {
                retries += load
                    .network_came_back()
                    .iter()
                    .filter(|step| matches!(step, LoadStep::Navigate(_)))
                    .count();
            }
        }
        assert_eq!(retries, 1, "one retry of the failed page");
        // A name that does not resolve is not the network.
        let mut typo = failed_load(vela_core::app::browser_load::probe_code::DNS);
        assert!(typo.network_came_back().is_empty());
        // The page's own load is retried by hand only.
        let mut page = failed_load(vela_core::app::browser_load::probe_code::CONNECT);
        page.watch.page_initiated = true;
        assert!(page.network_came_back().is_empty());
    }

    /// A load that finishes with no document saying hello (an error page)
    /// retires the old document too.
    #[test]
    fn a_load_without_a_provider_settles_the_old_page() {
        storage::tests::with_temp_state("dbr-host-load", || {
            seed_grant(DAPP, A1, 100);
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            ask(&mut driver, DAPP, "d1", "1", "personal_sign", json!([]));
            driver.dispatch(Event::NavigationStarted {
                tab: BROWSER_TAB.to_owned(),
                url: "https://broken.example/".to_owned(),
            });
            let out = driver.dispatch(Event::LoadFinished {
                tab: BROWSER_TAB.to_owned(),
                url: "https://broken.example/".to_owned(),
            });
            assert_eq!(cancelled(&out), vec!["1".to_owned()]);
            assert_eq!(answers(&out)[0]["error"]["code"], json!(4900));
        });
    }

    /// A read goes to the network off the main thread, and the node's body
    /// comes back as the page's answer.
    #[test]
    fn a_read_waits_for_the_network_and_comes_back() {
        storage::tests::with_temp_state("dbr-host-read", || {
            let _ = storage::write_value("vela.chain.https://dapp.example", json!(100));
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(&mut driver, DAPP, "d1", "9", "eth_blockNumber", json!([]));
            assert!(answers(&out).is_empty());
            let ids: Vec<u64> = out
                .iter()
                .filter_map(|next| match next {
                    Outbound::Work { id, neutral, .. } => {
                        assert_eq!(*neutral, DbrShellResult::ReadAnswered { body_json: None });
                        Some(*id)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(ids.len(), 1, "one call, not run here");
            let out = driver.resolve(
                ids[0],
                DbrShellResult::ReadAnswered {
                    body_json: Some(r#"{"jsonrpc":"2.0","id":1,"result":"0x10"}"#.to_owned()),
                },
            );
            assert_eq!(answers(&out)[0]["result"], json!("0x10"));
        });
    }

    /// Spec 079 FR-014: the page's chain gets the notice when its whole pool
    /// failed — and never when providers are only rate-limiting.
    #[test]
    fn the_chain_notice_is_for_a_chain_that_is_down_not_throttled() {
        let health = |failed: &[u32], unreached: &[u32], rate_limited: &[u32]| ChainHealth {
            failed: failed.to_vec(),
            unreached: unreached.to_vec(),
            rate_limited: rate_limited.to_vec(),
        };
        assert!(chain_down(100, &health(&[100, 1], &[], &[])));
        assert!(
            !chain_down(100, &health(&[100], &[], &[100])),
            "rate-limited stays quiet"
        );
        assert!(
            !chain_down(100, &health(&[1], &[], &[])),
            "another chain's trouble"
        );
        assert!(
            !chain_down(100, &health(&[], &[], &[])),
            "answered again: gone"
        );
        // Spec 082 RF1: a first pass that reached nothing is enough — the
        // notice shows while the dApp still waits, a pass before `failed`.
        assert!(chain_down(100, &health(&[], &[100], &[])));
        assert!(
            !chain_down(100, &health(&[], &[100], &[100])),
            "unreached but throttled: still quiet"
        );
    }

    /// Consent: one approval writes the grant under the shared key, the
    /// history row, and answers the page with the active account.
    #[test]
    fn approving_a_connection_writes_the_grant_and_answers() {
        storage::tests::with_temp_state("dbr-host-consent", || {
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = ask(
                &mut driver,
                DAPP,
                "d1",
                "1",
                "eth_requestAccounts",
                json!([]),
            );
            assert!(answers(&out).is_empty(), "the person is asked first");
            assert_eq!(
                driver.view().consent.map(|consent| consent.origin),
                Some(DAPP.to_owned())
            );
            let out = driver.dispatch(Event::ConsentApproved {
                now_ms: 1_757_000_000_000.0,
            });
            assert_eq!(answers(&out)[0]["result"], json!([A1]));
            let stored = storage::read_value("vela.perm.https://dapp.example")
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("no grant was written"));
            assert_eq!(stored["address"], json!(A1));
            let history = storage::read_value("vela.transactionHistory")
                .ok()
                .flatten()
                .unwrap_or_default();
            assert_eq!(history[0]["type"], json!("connect"));
        });
    }

    /// Settings' list: every stored grant, and a revoke from there reaches
    /// the store and any open page of that site.
    #[test]
    fn connected_sites_are_listed_and_revoked() {
        storage::tests::with_temp_state("dbr-host-sites", || {
            seed_grant(DAPP, A1, 100);
            seed_grant(OTHER, A1, 1);
            let mut driver = booted();
            let origins: Vec<String> = driver
                .view()
                .sites
                .iter()
                .map(|site| site.origin.clone())
                .collect();
            assert_eq!(origins.len(), 2);
            assert!(origins.contains(&DAPP.to_owned()) && origins.contains(&OTHER.to_owned()));

            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = driver.dispatch(Event::RevokeRequested {
                origin: DAPP.to_owned(),
            });
            let events: Vec<Value> = delivered(&out)
                .into_iter()
                .map(|(_, message)| message["event"].clone())
                .collect();
            assert_eq!(events, vec![json!("accountsChanged"), json!("disconnect")]);
            assert!(
                storage::read_value("vela.perm.https://dapp.example")
                    .ok()
                    .flatten()
                    .is_none()
            );
            assert_eq!(driver.view().sites.len(), 1);

            driver.dispatch(Event::RevokeAll);
            assert!(driver.view().sites.is_empty());
            assert!(
                storage::read_value("vela.perm.https://other.example")
                    .ok()
                    .flatten()
                    .is_none(),
                "Storage's clear reaches the live session and the file"
            );
        });
    }

    /// The connection panel's network row: the site moves, the choice is
    /// kept per origin, and a chain the wallet does not have is not a choice.
    #[test]
    fn a_person_can_pick_a_sites_network() {
        storage::tests::with_temp_state("dbr-host-pick", || {
            let mut driver = booted();
            page(&mut driver, DAPP, json!({"t":"hello","doc":"d1"}));
            let out = driver.dispatch(Event::SiteChainPicked {
                origin: DAPP.to_owned(),
                chain_id: 100,
            });
            let events: Vec<(Value, Value)> = delivered(&out)
                .into_iter()
                .map(|(_, message)| (message["event"].clone(), message["data"].clone()))
                .collect();
            assert_eq!(events, vec![(json!("chainChanged"), json!("0x64"))]);
            assert_eq!(
                storage::read_value("vela.chain.https://dapp.example")
                    .ok()
                    .flatten(),
                Some(json!(100))
            );
            let tab = driver.view().tabs.into_iter().next();
            assert_eq!(tab.map(|tab| tab.chain_id), Some(100));

            driver.dispatch(Event::SiteChainPicked {
                origin: DAPP.to_owned(),
                chain_id: 424_242,
            });
            assert_eq!(driver.view().tabs[0].chain_id, 100);
        });
    }
}
