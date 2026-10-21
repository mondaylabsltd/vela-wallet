//! The AI agent for Android and iOS (spec 104 phase 7): `vela-agent`'s one
//! copy of the runner, the transfers, the tools and the bridge connection —
//! the desktop's own — behind [`AgentShell`], what Swift and Kotlin provide.
//! Mirror types and wrappers only; the logic is `vela_agent::shell`'s.
//!
//! Threads: every [`AgentShell`] call comes from the agent's own thread,
//! never the main one, and may block while the shell hops to its main thread
//! — except under [`AgentCore::current_review`], which the shell calls itself
//! and is answered on the caller's thread. [`AgentDone`] is heard on the
//! agent's thread.

use std::sync::Arc;

use vela_agent::bridge_url::{bridge_url_problem, mcp_endpoint, UrlProblem};
use vela_agent::shell::{Agent, Link, Shell};
use vela_core::app::intents::Action;

/// The send open now.
#[derive(uniffi::Record)]
pub struct AgentSendOpen {
    /// The shell's own id for this send: a new one for each send opened.
    pub session: u64,
    /// Its `SendView`, as JSON.
    pub view: String,
}

/// What a shell does for its agent — `vela_agent::shell::Shell`, as Swift
/// and Kotlin see it.
#[uniffi::export(with_foreign)]
pub trait AgentShell: Send + Sync {
    /// The wallet's views as one JSON object: `session`, `contacts`,
    /// `balance`, `feed`, `networks` (once loaded), `currency`, `fee_tier`,
    /// `preferences` — each the machine's view JSON.
    fn views(&self) -> String;
    /// Do an action (an intent's JSON) — any but a transfer's — through the
    /// shell's own machines, as its screens do; answer once every call was
    /// made: `{"done":true}`, or `{"refused":{"code":…,"detail":…}}`.
    fn perform(&self, action: String) -> String;
    /// Open a screen (an intent's `Screen` JSON) for the person.
    fn reveal(&self, screen: String);
    /// The page the person is on: `wallet`, `contacts`, `explore`,
    /// `settings`, or `welcome` while nobody is in.
    fn page(&self) -> String;
    /// The dApp signing request on its sheet (`SignRequestView` JSON), if any.
    fn signing(&self) -> Option<String>;
    /// What the agent's screens show changed: its link, its requests. Read
    /// them again (on the main thread).
    fn changed(&self);
    /// The send open now, if any.
    fn send_open(&self) -> Option<AgentSendOpen>;
    /// The send's alert while one is up, by its name (`SendAlert` JSON's
    /// tag, or the string it is).
    fn send_alert(&self) -> Option<String>;
    /// Open a send on `params` (`SendOpenParams` JSON), as its own entry
    /// does; its session.
    fn send_start(&self, params: String) -> Option<u64>;
    /// One of the open send's events (`SendEvent` JSON).
    fn send_dispatch(&self, event: String);
    /// Leave the send, as its own Back off the form does.
    fn send_close(&self);
    /// The signed-in account's address, or empty.
    fn account(&self) -> String;
    /// Whether `account`'s balances have been read at least once.
    fn balances_settled(&self, account: String) -> bool;
    /// The holdings (`BalanceToken[]` JSON).
    fn holdings(&self) -> String;
    /// The chain tracker's entries (`TrackEntryView[]` JSON).
    fn tracker(&self) -> String;
    /// A network as this shell's send spells it in `SendToken.network`.
    fn network_id(&self, chain_id: u32) -> String;
}

/// Hears how a person's own request ended: an outcome's JSON
/// (`{"status":"completed"}`, `{"status":"rejected","reason":…}`, …).
#[uniffi::export(with_foreign)]
pub trait AgentDone: Send + Sync {
    fn done(&self, outcome: String);
}

struct Foreign(Arc<dyn AgentShell>);

impl Shell for Foreign {
    fn views(&self) -> String {
        self.0.views()
    }
    fn perform(&self, action: String) -> String {
        self.0.perform(action)
    }
    fn reveal(&self, screen: String) {
        self.0.reveal(screen);
    }
    fn page(&self) -> String {
        self.0.page()
    }
    fn signing(&self) -> Option<String> {
        self.0.signing()
    }
    fn changed(&self) {
        self.0.changed();
    }
    fn send_open(&self) -> Option<(u64, String)> {
        self.0.send_open().map(|open| (open.session, open.view))
    }
    fn send_alert(&self) -> Option<String> {
        self.0.send_alert()
    }
    fn send_start(&self, params: String) -> Option<u64> {
        self.0.send_start(params)
    }
    fn send_dispatch(&self, event: String) {
        self.0.send_dispatch(event);
    }
    fn send_close(&self) {
        self.0.send_close();
    }
    fn account(&self) -> String {
        self.0.account()
    }
    fn balances_settled(&self, account: String) -> bool {
        self.0.balances_settled(account)
    }
    fn holdings(&self) -> String {
        self.0.holdings()
    }
    fn tracker(&self) -> String {
        self.0.tracker()
    }
    fn network_id(&self, chain_id: u32) -> String {
        self.0.network_id(chain_id)
    }
}

/// The connection as the agent's screens show it.
#[derive(uniffi::Enum, Clone, Debug, PartialEq, Eq)]
pub enum AgentLink {
    /// Switched off, or not fully set up.
    Off,
    Connecting,
    Online,
    Offline,
    /// The bridge refused or closed it for good, saying why.
    Closed {
        code: u16,
        reason: String,
    },
}

impl From<Link> for AgentLink {
    fn from(link: Link) -> Self {
        match link {
            Link::Off => AgentLink::Off,
            Link::Connecting => AgentLink::Connecting,
            Link::Online => AgentLink::Online,
            Link::Offline => AgentLink::Offline,
            Link::Closed(close) => AgentLink::Closed {
                code: close.code,
                reason: close.reason,
            },
        }
    }
}

/// The phone's AI agent: one for the app's life.
#[derive(uniffi::Object)]
pub struct AgentCore {
    agent: Agent,
}

#[uniffi::export]
impl AgentCore {
    /// `device` is where the wallet is open, for the model: `"phone"`.
    #[uniffi::constructor]
    #[must_use]
    pub fn new(shell: Arc<dyn AgentShell>, device: String) -> Arc<Self> {
        Arc::new(AgentCore {
            agent: Agent::new(Arc::new(Foreign(shell)), &device),
        })
    }

    /// The person's settings, as stored: connect, reconnect or hang up to
    /// match them. Also the app's coming to the foreground (`enabled` as set)
    /// and leaving it (`false`).
    pub fn configure(&self, enabled: bool, bridge_url: String, client_token: String) {
        self.agent.configure(enabled, &bridge_url, &client_token);
    }

    #[must_use]
    pub fn link(&self) -> AgentLink {
        self.agent.link().into()
    }

    /// The agent's requests waiting for the person, oldest first (JSON).
    #[must_use]
    pub fn approvals(&self) -> String {
        self.agent.approvals()
    }

    /// The agent's recent calls, newest first (JSON).
    #[must_use]
    pub fn activity(&self) -> String {
        self.agent.activity()
    }

    /// The person answered an agent's request.
    pub fn answer(&self, id: u64, approved: bool) {
        self.agent.answer(id, approved);
    }

    /// The person stopped the agent: nothing of its still waits, and it is
    /// switched off through `configure_agent` (the shell's `perform` saves
    /// it and configures again).
    pub fn stop(&self) {
        self.agent.stop();
    }

    /// The person's own tap, through the same intents an agent's request
    /// takes (spec 103 D3). `false` when `action_json` is no action (nothing
    /// is done, `done` is not called).
    pub fn invoke_person(&self, action_json: String, done: Option<Arc<dyn AgentDone>>) -> bool {
        let Ok(action) = serde_json::from_str::<Action>(&action_json) else {
            return false;
        };
        self.agent.invoke_person(
            action,
            done.map(|done| Box::new(move |outcome: String| done.done(outcome)) as Box<_>),
        );
        true
    }

    /// The passkey's ceremony moved: `"started"` (its prompt is up) or
    /// `"signed"`.
    pub fn ceremony(&self, event: String) {
        self.agent.ceremony(&event);
    }

    /// The open send's operation on its review (an `Operation`'s JSON),
    /// adopted if it has none yet — so a person who filled the form
    /// themselves confirms through `submit_transfer` too. Calls the shell's
    /// `send_open` on this thread.
    #[must_use]
    pub fn current_review(&self) -> Option<String> {
        self.agent.current_review()
    }
}

/// Why a bridge URL cannot be used — `"invalid"`, `"insecure"` (`ws:` to
/// another machine) — or `None`.
#[uniffi::export]
#[must_use]
pub fn agent_bridge_url_problem(url: String) -> Option<String> {
    bridge_url_problem(&url).map(|problem| {
        match problem {
            UrlProblem::Invalid => "invalid",
            UrlProblem::Insecure => "insecure",
        }
        .to_owned()
    })
}

/// The MCP endpoint an agent adds, derived from the bridge URL.
#[uniffi::export]
#[must_use]
pub fn agent_mcp_endpoint(url: String) -> Option<String> {
    mcp_endpoint(&url)
}
