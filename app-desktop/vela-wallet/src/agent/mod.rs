//! The AI agent (spec 103, spec 104 phase 5): the desktop's end of a ClientMCP
//! bridge, and the intents an agent and the person both ask through.
//!
//! - **The person** configures it in Settings → AI agent (`settings.rs`): off
//!   unless they turn it on, and never changed by the agent.
//! - **The connection** is ClientMCP's Rust client on a thread of its own
//!   (`vela_agent::connection`), dialled through the app's proxy route; every
//!   call it receives is handed to gpui and answered
//!   there (`tools.rs`), through the same intents the screens use.
//! - **The intents** (`intents.rs`, `perform.rs`, `transfers.rs`): the core
//!   judges, the desktop's residents and page do; the agent's manage-level
//!   requests wait in the [`intents::Book`] for the person.
//! - **The page** (`wallet/page/agent.rs`) draws the bar, the approval sheet
//!   and the settings page, and does what only it can (open a screen, a
//!   preference it redraws by, the send column).

#[cfg(test)]
mod coverage;
pub use vela_agent::intents;
pub mod link;
mod perform;
pub mod settings;
pub mod tools;
pub mod transfers;
pub mod world;

use std::time::Duration;

use clientmcp_client::{Close, Status};
use gpui::{App, AsyncApp, BorrowAppContext as _, Context, Global, WeakEntity, Window};
use vela_core::app::intent_rules::World;
use vela_core::app::intents::{
    self as core_intents, Action, Actor, Check, Decision, Grants, Screen,
};

use intents::{Book, Deps, Outcome, Performed};
use settings::{AgentChange, AgentConfig};
use vela_agent::connection::{Connection, Dial};

use crate::wallet::page::WalletPage;

/// The runner's book as a gpui global: the screens watch it (the capsule,
/// the approval sheet, Settings' last action).
#[derive(Default)]
pub struct Requests(pub Book);

impl Global for Requests {}

/// The book, to read.
pub fn book(cx: &App) -> Option<&Book> {
    cx.try_global::<Requests>().map(|requests| &requests.0)
}

/// The connection as Settings and the bar show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// Switched off, or not fully configured.
    Off,
    Connecting,
    Online,
    Offline,
    /// The bridge refused or closed it for good, saying why.
    Closed(Close),
}

/// The agent's settings and connection.
pub struct Agent {
    pub config: AgentConfig,
    pub link: Link,
    connection: Option<(u64, Connection)>,
    /// What the connection was dialled with: `url\ntoken`, or empty.
    dialled: String,
    serial: u64,
    /// Into gpui, from the connection's thread.
    events: futures::channel::mpsc::UnboundedSender<tools::Event>,
}

impl Global for Agent {}

/// The open wallet page, for what only it can do. Registered as it opens.
struct PagePort(WeakEntity<WalletPage>);

impl Global for PagePort {}

/// Start the agent: read its settings, and dial if they say so. Once, at launch.
pub fn boot(cx: &mut App) {
    let (events, mut incoming) = futures::channel::mpsc::unbounded();
    cx.set_global(Requests::default());
    cx.set_global(Agent {
        config: AgentConfig::load(),
        link: Link::Off,
        connection: None,
        dialled: String::new(),
        serial: 0,
        events,
    });
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        while let Some(event) = incoming.next().await {
            tools::receive(event, cx);
        }
    })
    .detach();
    sync(cx);
}

pub fn register_page(page: WeakEntity<WalletPage>, cx: &mut App) {
    cx.set_global(PagePort(page));
}

pub(crate) fn page(cx: &App) -> Option<WeakEntity<WalletPage>> {
    cx.try_global::<PagePort>()
        .map(|port| port.0.clone())
        .filter(|page| page.upgrade().is_some())
}

/// Run `f` on the open page, in its window. `None` while no page is open.
pub(crate) fn on_page<R>(
    cx: &mut AsyncApp,
    f: impl FnOnce(&mut WalletPage, &mut Window, &mut Context<WalletPage>) -> R,
) -> Option<R> {
    let page = cx.update(|cx| page(cx))?;
    page.update_in(cx, f).ok()
}

pub fn enabled(cx: &App) -> bool {
    cx.try_global::<Agent>()
        .is_some_and(|agent| agent.config.enabled)
}

/// The person's change to the agent's settings (`configure_agent`): saved,
/// and the connection follows it.
pub(crate) fn configure(change: &AgentChange, cx: &mut App) {
    if let Some(agent) = cx.try_global::<Agent>() {
        let mut config = agent.config.clone();
        config.apply(change);
        config.save();
        cx.update_global::<Agent, _>(|agent, _| agent.config = config);
        sync(cx);
    }
}

/// Spec 106: after the person's `configure_agent` with what clientmcp.dev
/// issued, whose it is — kept so Settings can say so.
pub(crate) fn set_linked(
    linked: Option<settings::AgentLinked>,
    agent_url: Option<String>,
    cx: &mut App,
) {
    if let Some(agent) = cx.try_global::<Agent>() {
        let mut config = agent.config.clone();
        config.agent_url = if linked.is_some() {
            agent_url.unwrap_or_default()
        } else {
            String::new()
        };
        config.linked = linked;
        config.save();
        cx.update_global::<Agent, _>(|agent, _| agent.config = config);
    }
}

/// Connect, reconnect or hang up so the connection matches the settings.
pub fn sync(cx: &mut App) {
    let Some(agent) = cx.try_global::<Agent>() else {
        return;
    };
    let dial = if agent.config.ready() {
        format!("{}\n{}", agent.config.url.trim(), agent.config.token.trim())
    } else {
        String::new()
    };
    if dial == agent.dialled {
        return;
    }
    let events = agent.events.clone();
    let serial = agent.serial + 1;
    let previous = cx.update_global::<Agent, _>(|agent, _| {
        agent.dialled.clone_from(&dial);
        agent.serial = serial;
        agent.connection.take()
    });
    if let Some((_, connection)) = previous {
        connection.close();
    }
    if dial.is_empty() {
        cx.update_global::<Agent, _>(|agent, _| agent.link = Link::Off);
        // Nothing of a disconnected agent's still waits for the person.
        cx.update_global::<Requests, _>(|requests, _| requests.0.stop());
        return;
    }
    let (url, token) = dial.split_once('\n').unwrap_or_default();
    let definitions = vela_agent::tools::definitions();
    let status_events = events.clone();
    let opened = Connection::open(
        url,
        token,
        &vela_agent::tools::instructions("computer"),
        move |status, close| {
            let _ = status_events.unbounded_send(tools::Event::Status {
                serial,
                status,
                close: close.cloned(),
            });
        },
        Some(proxy_dial(url.trim_start().starts_with("wss:"))),
        |client| vela_agent::tools::expose(client, &definitions, &tools::forward(&events)),
    );
    cx.update_global::<Agent, _>(|agent, _| {
        agent.link = if opened.is_some() {
            Link::Connecting
        } else {
            Link::Offline
        };
        agent.connection = opened.map(|connection| (serial, connection));
    });
}

/// The bridge's socket, the way the rest of the app reaches the network:
/// through the proxy the system names for it (the cable tunnel's dialler).
fn proxy_dial(secure: bool) -> Dial {
    std::sync::Arc::new(move |host: String, port: u16| {
        let scheme = if secure { "https" } else { "http" };
        crate::ctap::cable::dial(&format!("{scheme}://{host}:{port}/"), &host, port, |line| {
            crate::diag::vlog!("agent", "{line}");
        })
        .map_err(|error| std::io::Error::other(error.to_string()))
    })
}

/// The connection's status changed (on gpui, from its thread).
fn status(serial: u64, status: Status, close: Option<Close>, cx: &mut App) {
    cx.update_global::<Agent, _>(|agent, _| {
        if agent.serial != serial || agent.connection.is_none() {
            return;
        }
        agent.link = match status {
            Status::Connecting => Link::Connecting,
            Status::Online => Link::Online,
            Status::Offline => Link::Offline,
            Status::Closed => Link::Closed(close.unwrap_or(Close {
                code: 1000,
                reason: String::new(),
            })),
        };
    });
}

/// The person asks: their own click, through the same intents an agent's
/// request takes (spec 103 D3). Who asks only changes the authorization.
pub fn invoke_person(action: Action, cx: &mut App) {
    invoke_person_then(action, cx, |_, _| {});
}

/// The same, and `then` once it has ended — for a screen that follows the
/// change (the contact just saved, opened).
pub fn invoke_person_then(
    action: Action,
    cx: &mut App,
    then: impl FnOnce(&Outcome, &mut App) + 'static,
) {
    cx.spawn(async move |cx| {
        let mut live = Live { cx };
        let outcome = intents::invoke(&mut live, Actor::Person, action, &|| false).await;
        cx.update(|cx| then(&outcome, cx));
    })
    .detach();
}

/// The person stopped the agent: nothing of its still waits, and it is
/// switched off (their own setting, through the same intent Settings uses).
pub fn stop(cx: &mut App) {
    if cx.has_global::<Requests>() {
        cx.update_global::<Requests, _>(|requests, _| requests.0.stop());
    }
    // At once: what the agent asks from here on is refused as off, and a
    // draft under way is given up, before the intent has saved the setting.
    if cx.has_global::<Agent>() {
        cx.update_global::<Agent, _>(|agent, _| agent.config.enabled = false);
    }
    invoke_person(
        Action::ConfigureAgent {
            enabled: Some(false),
            bridge_url: None,
            client_token: None,
            wallets: None,
        },
        cx,
    );
}

/// The person answered an agent's request.
pub fn answer(id: u64, approved: bool, cx: &mut App) {
    if cx.has_global::<Requests>() {
        cx.update_global::<Requests, _>(|requests, _| requests.0.answer(id, approved));
    }
}

/// The intents over the live wallet.
pub(crate) struct Live<'a> {
    pub cx: &'a mut AsyncApp,
}

impl Deps for Live<'_> {
    fn check(&mut self, actor: Actor, action: &Action) -> Check {
        // The switch, and the wallets the agent may use (spec 107 E8).
        let grants = self.cx.update(|cx| Grants {
            agent_enabled: enabled(cx),
            ..vela_core::app::intent_rules::grants(&world::world(cx, &[], true))
        });
        let mut checked = core_intents::check(actor, action, &grants);
        if !matches!(checked.decision, Decision::Deny { .. })
            && let Some(reason) = perform::unavailable(action)
        {
            checked.decision = Decision::Deny { reason };
        }
        checked
    }

    fn world(&mut self) -> World {
        self.cx.update(|cx| {
            let operations = transfers::list(cx);
            world::world(cx, &operations, enabled(cx))
        })
    }

    async fn perform(
        &mut self,
        action: &Action,
        world: &World,
        now_ms: f64,
        actor: Actor,
    ) -> Performed {
        perform::perform(action, world, now_ms, actor, self.cx).await
    }

    fn now_ms(&mut self) -> f64 {
        crate::executor::now_ms()
    }

    async fn sleep(&mut self, ms: u64) {
        self.cx
            .background_executor()
            .timer(Duration::from_millis(ms))
            .await;
    }

    async fn reveal(&mut self, screen: &Screen) {
        let _ = on_page(self.cx, |page, _, cx| page.agent_show(screen, cx));
    }

    fn book<R>(&mut self, change: impl FnOnce(&mut Book) -> R) -> R {
        self.cx.update(|cx| {
            if !cx.has_global::<Requests>() {
                cx.set_global(Requests::default());
            }
            cx.update_global::<Requests, _>(|requests, _| change(&mut requests.0))
        })
    }
}

/// The agent's request, run as the agent.
pub(crate) async fn invoke_agent(
    action: Action,
    gone: &dyn Fn() -> bool,
    cx: &mut AsyncApp,
) -> Outcome {
    let mut live = Live { cx };
    intents::invoke(&mut live, Actor::Agent, action, gone).await
}
