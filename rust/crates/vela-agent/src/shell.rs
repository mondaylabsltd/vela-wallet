//! The agent for a shell that answers from its own threads — the phones
//! (spec 104 phase 7). Everything runs on the agent's thread
//! ([`crate::connection`]); the shell is reached through [`Shell`], whose
//! methods are called there (never on the shell's main thread) and may block
//! while the shell hops to its own.
//!
//! The shell keeps only what is its: its machines (`perform`, its views), its
//! screens (`reveal`, `page`), its send. The runner, the transfers, the tools,
//! the connection and the person's approvals are this crate's.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use clientmcp_client::{Close, HandlerResult, Reply, Status};
use serde_json::{Value, json};
use vela_core::app::balance_dashboard::BalanceToken;
use vela_core::app::intent_rules::{self, ReviewInput, TrackFacts, World};
use vela_core::app::intents::{self as core_intents, Action, Actor, Check, Grants, Reason, Screen};
use vela_core::app::send::{Event as SendEvent, SendOpenParams, SendStage, SendView};

use crate::bridge_url::bridge_url_problem;
use crate::connection::Connection;
use crate::intents::{self, Book, Deps, Outcome, Performed};
use crate::tools::{self, Forward, Wait};
use crate::transfers::{self, SendDesk, Transfers};

/// What a shell does for its agent. Called on the agent's thread.
pub trait Shell: Send + Sync {
    /// The wallet's views, as one JSON object under the names the intents
    /// read: `session`, `contacts`, `balance`, `feed`, `networks` (once
    /// loaded), `currency`, `fee_tier`, `preferences`.
    fn views(&self) -> String;
    /// Do `action` (an intent, as JSON) through the shell's own machines —
    /// every action but a transfer's — and answer once every call was made:
    /// `{"done":true}`, or `{"refused":{"code":…,"detail":…}}`.
    fn perform(&self, action: String) -> String;
    /// Open `screen` (an intent's screen, as JSON) for the person.
    fn reveal(&self, screen: String);
    /// The page the person is on: `wallet`, `contacts`, `explore`, `settings`,
    /// or `welcome` while nobody is in.
    fn page(&self) -> String;
    /// The dApp signing request on its sheet (`SignRequestView` JSON), if any.
    fn signing(&self) -> Option<String>;
    /// Something the agent's screens show changed: its link, its requests.
    fn changed(&self);
    /// The send open now: the shell's own id for it, and its view as JSON.
    fn send_open(&self) -> Option<(u64, String)>;
    /// The send's alert, by its name, while one is up.
    fn send_alert(&self) -> Option<String>;
    /// Open a send on `params` (`SendOpenParams` JSON); its id.
    fn send_start(&self, params: String) -> Option<u64>;
    /// One of the send's events, as JSON.
    fn send_dispatch(&self, event: String);
    /// Leave the send, as its own Back off the form does.
    fn send_close(&self);
    /// The signed-in account.
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

/// The connection as the shell's screens show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Link {
    /// Switched off, or not fully set up.
    Off,
    Connecting,
    Online,
    Offline,
    /// The bridge refused or closed it for good, saying why.
    Closed(Close),
}

struct State {
    enabled: bool,
    url: String,
    token: String,
    link: Link,
    connection: Option<Connection>,
    /// What the connection was dialled with: `url\ntoken`, or empty.
    dialled: String,
    serial: u64,
    book: Book,
    transfers: Transfers,
}

struct Inner {
    shell: Arc<dyn Shell>,
    /// Where the wallet is open, for the model: "phone".
    device: String,
    state: Mutex<State>,
}

impl Inner {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// The phone's agent: one per app.
#[derive(Clone)]
pub struct Agent {
    inner: Arc<Inner>,
}

/// The book's requests and recent calls, compared to tell the shell only of a
/// change.
fn fingerprint(book: &Book) -> (Value, Value) {
    tools::book_json(book)
}

impl Agent {
    #[must_use]
    pub fn new(shell: Arc<dyn Shell>, device: &str) -> Agent {
        Agent {
            inner: Arc::new(Inner {
                shell,
                device: device.to_owned(),
                state: Mutex::new(State {
                    enabled: false,
                    url: String::new(),
                    token: String::new(),
                    link: Link::Off,
                    connection: None,
                    dialled: String::new(),
                    serial: 0,
                    book: Book::default(),
                    transfers: Transfers::default(),
                }),
            }),
        }
    }

    /// The person's settings (stored by the shell): connect, reconnect or
    /// hang up to match them.
    pub fn configure(&self, enabled: bool, url: &str, token: &str) {
        let (url, token) = (url.trim(), token.trim());
        let ready = enabled && bridge_url_problem(url).is_none() && !token.is_empty();
        let dial = if ready {
            format!("{url}\n{token}")
        } else {
            String::new()
        };
        let (previous, serial) = {
            let mut state = self.inner.state();
            state.enabled = enabled;
            state.url = url.to_owned();
            state.token = token.to_owned();
            if dial == state.dialled {
                return;
            }
            state.dialled.clone_from(&dial);
            state.serial += 1;
            if !dial.is_empty() {
                // Before it dials: its first status may come before `open` returns.
                state.link = Link::Connecting;
            }
            (state.connection.take(), state.serial)
        };
        if let Some(connection) = previous {
            connection.close();
        }
        if dial.is_empty() {
            {
                let mut state = self.inner.state();
                state.link = Link::Off;
                // Nothing of a disconnected agent's still waits for the person.
                state.book.stop();
            }
            self.inner.shell.changed();
            return;
        }
        let watcher = Arc::downgrade(&self.inner);
        let definitions = tools::definitions();
        let forward = forward(&self.inner);
        let opened = Connection::open(
            url,
            token,
            &tools::instructions(&self.inner.device),
            move |status, close| {
                let Some(inner) = watcher.upgrade() else {
                    return;
                };
                {
                    let mut state = inner.state();
                    if state.serial != serial {
                        return;
                    }
                    state.link = match status {
                        Status::Connecting => Link::Connecting,
                        Status::Online => Link::Online,
                        Status::Offline => Link::Offline,
                        Status::Closed => Link::Closed(close.cloned().unwrap_or(Close {
                            code: 1000,
                            reason: String::new(),
                        })),
                    };
                }
                inner.shell.changed();
            },
            None,
            |client| tools::expose(client, &definitions, &forward),
        );
        let stale = {
            let mut state = self.inner.state();
            if state.serial == serial {
                if opened.is_none() {
                    state.link = Link::Offline;
                }
                state.connection = opened;
                None
            } else {
                // Configured again meanwhile: this one is already out of date.
                opened
            }
        };
        if let Some(stale) = stale {
            stale.close();
        }
        self.inner.shell.changed();
    }

    #[must_use]
    pub fn link(&self) -> Link {
        self.inner.state().link.clone()
    }

    /// Whether the person turned the agent on.
    #[must_use]
    pub fn enabled(&self) -> bool {
        self.inner.state().enabled
    }

    /// The agent's requests waiting for the person, oldest first (JSON).
    #[must_use]
    pub fn approvals(&self) -> String {
        json!(self.inner.state().book.approvals).to_string()
    }

    /// The agent's recent calls, newest first (JSON).
    #[must_use]
    pub fn activity(&self) -> String {
        json!(self.inner.state().book.activity).to_string()
    }

    /// The person answered an agent's request.
    pub fn answer(&self, id: u64, approved: bool) {
        self.inner.state().book.answer(id, approved);
        self.inner.shell.changed();
    }

    /// The person stopped the agent: nothing of its still waits, and it is
    /// switched off — their own setting, through the same intent Settings
    /// uses (the shell saves it, and configures this again).
    pub fn stop(&self) {
        {
            let mut state = self.inner.state();
            // At once: what the agent asks from here on is refused as off,
            // and a draft under way is given up, before the shell has saved
            // the setting and hung up.
            state.enabled = false;
            state.book.stop();
        }
        self.inner.shell.changed();
        self.invoke_person(
            Action::ConfigureAgent {
                enabled: Some(false),
                bridge_url: None,
                client_token: None,
                wallets: None,
            },
            None,
        );
    }

    /// The person asks: their own tap, through the same intents an agent's
    /// request takes (spec 103 D3). `then` hears how it ended (an outcome's
    /// JSON), on the agent's thread.
    pub fn invoke_person(&self, action: Action, then: Option<Box<dyn FnOnce(String) + Send>>) {
        let Some(handle) = crate::connection::handle() else {
            return;
        };
        let inner = Arc::clone(&self.inner);
        handle.spawn(async move {
            let mut live = Live { inner };
            let outcome = intents::invoke(&mut live, Actor::Person, action, &|| false).await;
            if let Some(then) = then {
                then(outcome_json(&outcome));
            }
        });
    }

    /// The passkey's ceremony moved: `started` (its prompt is up) or `signed`.
    pub fn ceremony(&self, event: &str) {
        self.inner.state().transfers.ceremony(event);
    }

    /// The open send's operation on its review (JSON), adopted if it has none
    /// yet — a person who filled the form themselves confirms through the
    /// same intent. Calls the shell: from its main thread, it must answer
    /// there without hopping.
    #[must_use]
    pub fn current_review(&self) -> Option<String> {
        let mut desk = Desk {
            inner: Arc::clone(&self.inner),
        };
        transfers::current(&mut desk).map(|operation| json!(operation).to_string())
    }
}

/// The runner's wallet: the shell, reached from the agent's thread.
struct Live {
    inner: Arc<Inner>,
}

fn now_ms() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0., |since| since.as_secs_f64() * 1000.)
}

fn performed(answer: &str) -> Performed {
    let value: Value = serde_json::from_str(answer).unwrap_or(Value::Null);
    match value
        .get("refused")
        .cloned()
        .map(serde_json::from_value::<Reason>)
    {
        Some(Ok(reason)) => Performed::Refused(reason),
        Some(Err(_)) => Performed::refused("the wallet refused it"),
        None => Performed::Done,
    }
}

impl Live {
    fn desk(&self) -> Desk {
        Desk {
            inner: Arc::clone(&self.inner),
        }
    }

    /// The views as JSON, with the operations and the agent's switch.
    fn views(&self) -> Value {
        let operations = transfers::list(&mut self.desk());
        let enabled = self.inner.state().enabled;
        let mut views: Value =
            serde_json::from_str(&self.inner.shell.views()).unwrap_or_else(|_| json!({}));
        if !views.is_object() {
            views = json!({});
        }
        views["operations"] = json!(operations);
        views["agent_enabled"] = Value::Bool(enabled);
        views
    }
}

impl Deps for Live {
    fn check(&mut self, actor: Actor, action: &Action) -> Check {
        // The switch is the core's own state; the wallets it may use are the
        // world's (spec 107 E8).
        let agent_enabled = self.inner.state().enabled;
        let world: vela_core::app::intent_rules::World =
            serde_json::from_value(self.views()).unwrap_or_default();
        let grants = Grants {
            agent_enabled,
            ..vela_core::app::intent_rules::grants(&world)
        };
        core_intents::check(actor, action, &grants)
    }

    fn world(&mut self) -> World {
        serde_json::from_value(self.views()).unwrap_or_default()
    }

    async fn perform(&mut self, action: &Action, _: &World, _: f64, actor: Actor) -> Performed {
        let mut desk = self.desk();
        match action {
            Action::PrepareTransfer {
                chain_id,
                token,
                to,
                amount,
            } => {
                transfers::prepare(*chain_id, token.as_deref(), to, amount, actor, &mut desk).await
            }
            Action::PrepareBatchTransfer {
                chain_id,
                token,
                recipients,
            } => {
                transfers::prepare_batch(*chain_id, token.as_deref(), recipients, actor, &mut desk)
                    .await
            }
            Action::SubmitTransfer { operation, review } => {
                transfers::submit(operation, review.as_deref(), actor, &mut desk)
            }
            Action::CancelTransfer { operation } => transfers::cancel(operation, &mut desk),
            _ => {
                let shell = Arc::clone(&self.inner.shell);
                let action = json!(action).to_string();
                match tokio::task::spawn_blocking(move || shell.perform(action)).await {
                    Ok(answer) => performed(&answer),
                    Err(_) => Performed::refused("the wallet could not do it"),
                }
            }
        }
    }

    fn now_ms(&mut self) -> f64 {
        now_ms()
    }

    async fn sleep(&mut self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    async fn reveal(&mut self, screen: &Screen) {
        let shell = Arc::clone(&self.inner.shell);
        let screen = json!(screen).to_string();
        let _ = tokio::task::spawn_blocking(move || shell.reveal(screen)).await;
    }

    fn book<R>(&mut self, change: impl FnOnce(&mut Book) -> R) -> R {
        let (result, changed) = {
            let mut state = self.inner.state();
            let before = fingerprint(&state.book);
            let result = change(&mut state.book);
            (result, fingerprint(&state.book) != before)
        };
        if changed {
            self.inner.shell.changed();
        }
        result
    }
}

/// The shell's send, from the agent's thread.
struct Desk {
    inner: Arc<Inner>,
}

impl SendDesk for Desk {
    fn open(&mut self) -> Option<(u64, SendView)> {
        let (session, view) = self.inner.shell.send_open()?;
        Some((session, serde_json::from_str(&view).ok()?))
    }

    fn alert(&mut self) -> Option<String> {
        self.inner.shell.send_alert()
    }

    fn start(&mut self, params: SendOpenParams) -> Option<u64> {
        self.inner.shell.send_start(json!(params).to_string())
    }

    fn dispatch(&mut self, event: SendEvent) {
        self.inner.shell.send_dispatch(json!(event).to_string());
    }

    fn close(&mut self) {
        self.inner.shell.send_close();
    }

    fn account(&mut self) -> String {
        self.inner.shell.account()
    }

    fn settled(&mut self, account: &str) -> bool {
        self.inner.shell.balances_settled(account.to_owned())
    }

    fn holdings(&mut self) -> Vec<BalanceToken> {
        serde_json::from_str(&self.inner.shell.holdings()).unwrap_or_default()
    }

    fn tracker(&mut self) -> Vec<TrackFacts> {
        serde_json::from_str(&self.inner.shell.tracker()).unwrap_or_default()
    }

    fn network_id(&mut self, chain_id: u32) -> String {
        self.inner.shell.network_id(chain_id)
    }

    async fn sleep(&mut self, ms: u64) {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    fn ledger<R>(&mut self, change: impl FnOnce(&mut Transfers) -> R) -> R {
        change(&mut self.inner.state().transfers)
    }

    fn halted(&mut self) -> bool {
        // Stopped, switched off, or the app in the background (the shell
        // configures it off there).
        !self.inner.state().enabled
    }
}

/// Sets the flag when dropped: the bridge gave up on the call.
struct GoneOnDrop(Arc<AtomicBool>);

impl Drop for GoneOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Each tool's handler: answer the call as a task of its own, so that a call
/// the bridge gives up on (its handler dropped) still ends as the runner
/// says — an approval withdrawn, nothing done after — rather than mid-step.
fn forward(inner: &Arc<Inner>) -> Forward {
    let inner = Arc::downgrade(inner);
    Arc::new(move |name, args| {
        let inner = inner.clone();
        Box::pin(async move {
            let inner = inner
                .upgrade()
                .ok_or_else(|| "the wallet is closing".to_owned())?;
            let gone = Arc::new(AtomicBool::new(false));
            let _guard = GoneOnDrop(Arc::clone(&gone));
            tokio::spawn(answer(inner, name, args, gone))
                .await
                .unwrap_or_else(|_| Err("the wallet's agent stopped".to_owned()))
        })
    })
}

async fn answer(
    inner: Arc<Inner>,
    name: String,
    args: Value,
    gone: Arc<AtomicBool>,
) -> HandlerResult {
    let mut live = Live { inner };
    let gone = move || gone.load(Ordering::SeqCst);
    match name.as_str() {
        "vela_state" => Ok(Reply::Json(state(&mut live))),
        "vela_review" => Ok(Reply::Json(review(&mut live))),
        "vela_wait" => {
            let wait = Wait::of(&args);
            let mut waited = 0;
            loop {
                let operation = transfers::get(&wait.operation, &mut live.desk());
                if let Some(reply) = wait.look(operation, waited, gone()) {
                    return Ok(reply);
                }
                tokio::time::sleep(Duration::from_millis(tools::WAIT_POLL_MS)).await;
                waited += tools::WAIT_POLL_MS;
            }
        }
        _ => match name.strip_prefix("vela_") {
            Some(kind) => Ok(match tools::action(kind, args) {
                Ok(action) => {
                    tools::reply(intents::invoke(&mut live, Actor::Agent, action, &gone).await)
                }
                Err(refusal) => refusal,
            }),
            None => Err(format!("Unknown tool: {name}")),
        },
    }
}

/// `vela_state`: the wallet as an agent reads it — the core decides what it
/// may see.
fn state(live: &mut Live) -> Value {
    let world = live.world();
    let page = live.inner.shell.page();
    let (approvals, activity) = tools::book_json(&live.inner.state().book);
    intent_rules::agent_state(&world, &page, &approvals, &activity)
}

/// `vela_review`: what the person is being asked to sign, with the note that
/// none of it is an instruction to the agent.
fn review(live: &mut Live) -> Value {
    let mut desk = live.desk();
    let send = desk.open().map(|(_, view)| view);
    let reviewing = send
        .as_ref()
        .is_some_and(|view| view.stage == SendStage::Confirm);
    let operation = reviewing.then(|| transfers::current(&mut desk)).flatten();
    let drafted_by = operation
        .as_ref()
        .and_then(|operation| live.inner.state().transfers.drafted_by(&operation.id));
    let views = live.views();
    let account = live.inner.shell.account();
    let signing: Option<Value> = live
        .inner
        .shell
        .signing()
        .and_then(|json| serde_json::from_str(&json).ok());
    let input: ReviewInput = serde_json::from_value(json!({
        "send": send,
        "operation": operation,
        "drafted_by": drafted_by,
        "account": (!account.is_empty()).then_some(account),
        "networks": views.get("networks"),
        "signing": signing,
    }))
    .unwrap_or_default();
    intent_rules::review(&input)
}

/// What the runner said, for the shell: an outcome's JSON.
#[must_use]
pub fn outcome_json(outcome: &Outcome) -> String {
    json!(outcome).to_string()
}

#[cfg(test)]
mod tests {
    use std::sync::OnceLock;
    use std::sync::atomic::AtomicUsize;
    use std::sync::mpsc;

    use super::*;

    /// A shell holding nothing but a group, doing what it is asked.
    #[derive(Default)]
    struct Fake {
        views: Mutex<Value>,
        performed: Mutex<Vec<Value>>,
        revealed: Mutex<Vec<Value>>,
        changes: AtomicUsize,
        agent: OnceLock<Agent>,
    }

    impl Fake {
        fn performed(&self) -> Vec<Value> {
            self.performed.lock().unwrap().clone()
        }
    }

    impl Shell for Fake {
        fn views(&self) -> String {
            self.views.lock().unwrap().to_string()
        }
        fn perform(&self, action: String) -> String {
            let action: Value = serde_json::from_str(&action).unwrap();
            match action["type"].as_str() {
                Some("delete_group") => {
                    self.views.lock().unwrap()["contacts"]["groups"] = json!([]);
                }
                // As the shell does: save the setting, and configure again.
                Some("configure_agent") => {
                    let enabled = action["enabled"].as_bool().unwrap_or(true);
                    self.agent.get().unwrap().configure(enabled, "", "");
                }
                _ => {}
            }
            self.performed.lock().unwrap().push(action);
            json!({ "done": true }).to_string()
        }
        fn reveal(&self, screen: String) {
            self.revealed
                .lock()
                .unwrap()
                .push(serde_json::from_str(&screen).unwrap());
        }
        fn page(&self) -> String {
            "contacts".into()
        }
        fn signing(&self) -> Option<String> {
            None
        }
        fn changed(&self) {
            self.changes.fetch_add(1, Ordering::SeqCst);
        }
        fn send_open(&self) -> Option<(u64, String)> {
            None
        }
        fn send_alert(&self) -> Option<String> {
            None
        }
        fn send_start(&self, _: String) -> Option<u64> {
            None
        }
        fn send_dispatch(&self, _: String) {}
        fn send_close(&self) {}
        fn account(&self) -> String {
            String::new()
        }
        fn balances_settled(&self, _: String) -> bool {
            false
        }
        fn holdings(&self) -> String {
            "[]".into()
        }
        fn tracker(&self) -> String {
            "[]".into()
        }
        fn network_id(&self, chain_id: u32) -> String {
            chain_id.to_string()
        }
    }

    /// The agent switched on (not connected: these calls come straight in).
    fn wallet() -> (Arc<Fake>, Agent) {
        let fake = Arc::new(Fake {
            views: Mutex::new(json!({
                "contacts": { "loaded": true, "contacts": [], "groups": [{ "id": "g1", "name": "Team" }] }
            })),
            ..Fake::default()
        });
        let agent = Agent::new(Arc::clone(&fake) as Arc<dyn Shell>, "phone");
        assert!(fake.agent.set(agent.clone()).is_ok());
        agent.configure(true, "", "");
        (fake, agent)
    }

    fn delete_group() -> Action {
        serde_json::from_value(json!({ "type": "delete_group", "id": "g1" })).unwrap()
    }

    /// An agent's call as the bridge makes it, on the agent's thread.
    fn call(
        agent: &Agent,
        name: &str,
        args: Value,
    ) -> (mpsc::Receiver<HandlerResult>, tokio::task::JoinHandle<()>) {
        let (tx, rx) = mpsc::channel();
        let handler = forward(&agent.inner)(name.to_owned(), args);
        let task = crate::connection::handle().unwrap().spawn(async move {
            let _ = tx.send(handler.await);
        });
        (rx, task)
    }

    fn until(what: &str, done: impl Fn() -> bool) {
        for _ in 0..500 {
            if done() {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("never: {what}");
    }

    fn outcome(result: HandlerResult) -> Value {
        match result {
            Ok(Reply::Result(result)) => result["structuredContent"].clone(),
            other => panic!("not an outcome: {other:?}"),
        }
    }

    fn approvals(agent: &Agent) -> Vec<Value> {
        serde_json::from_str(&agent.approvals()).unwrap()
    }

    const WAIT: Duration = Duration::from_secs(5);

    #[test]
    fn the_persons_tap_runs_through_the_shell_and_is_heard() {
        let (fake, agent) = wallet();
        let (tx, rx) = mpsc::channel();
        agent.invoke_person(
            delete_group(),
            Some(Box::new(move |outcome| {
                let _ = tx.send(outcome);
            })),
        );
        let outcome: Value = serde_json::from_str(&rx.recv_timeout(WAIT).unwrap()).unwrap();
        assert_eq!(outcome["status"], "completed");
        assert_eq!(fake.performed()[0]["type"], "delete_group");
        assert!(
            fake.revealed.lock().unwrap().is_empty(),
            "the person is where they did it"
        );
        assert_eq!(agent.activity(), "[]", "not the agent's activity");
    }

    #[test]
    fn an_agents_request_waits_for_the_person_then_shows_where_it_happened() {
        let (fake, agent) = wallet();
        let before = fake.changes.load(Ordering::SeqCst);
        let (rx, _task) = call(&agent, "vela_delete_group", json!({ "id": "g1" }));
        until("asked", || approvals(&agent).len() == 1);
        assert!(
            fake.changes.load(Ordering::SeqCst) > before,
            "the shell was told"
        );
        assert!(
            fake.performed().is_empty(),
            "nothing before the person answers"
        );

        let id = approvals(&agent)[0]["id"].as_u64().unwrap();
        agent.answer(id, true);
        assert_eq!(
            outcome(rx.recv_timeout(WAIT).unwrap())["status"],
            "completed"
        );
        assert_eq!(fake.performed()[0]["type"], "delete_group");
        assert_eq!(
            *fake.revealed.lock().unwrap(),
            [json!({ "type": "contacts" })]
        );
        assert!(approvals(&agent).is_empty());
        let activity: Value = serde_json::from_str(&agent.activity()).unwrap();
        assert_eq!(activity[0]["outcome"]["status"], "completed");
    }

    #[test]
    fn a_call_given_up_withdraws_its_request_and_does_nothing() {
        let (fake, agent) = wallet();
        let (_rx, task) = call(&agent, "vela_delete_group", json!({ "id": "g1" }));
        until("asked", || approvals(&agent).len() == 1);
        // The bridge's timeout, a lost connection: the handler is dropped.
        task.abort();
        until("withdrawn", || approvals(&agent).is_empty());
        until("ended", || {
            let activity: Value = serde_json::from_str(&agent.activity()).unwrap();
            activity[0]["outcome"]["reason"]["code"] == "stopped"
        });
        assert!(fake.performed().is_empty());
    }

    #[test]
    fn stop_withdraws_what_waits_and_switches_the_agent_off() {
        let (fake, agent) = wallet();
        let (rx, _task) = call(&agent, "vela_delete_group", json!({ "id": "g1" }));
        until("asked", || approvals(&agent).len() == 1);
        agent.stop();
        assert!(
            !agent.enabled(),
            "off at once, before the shell has saved it"
        );
        let ended = outcome(rx.recv_timeout(WAIT).unwrap());
        assert_eq!(ended["reason"]["code"], "stopped");
        until("saved", || !fake.performed().is_empty());
        let performed = fake.performed();
        assert_eq!(performed.len(), 1, "the group is kept: {performed:?}");
        assert_eq!(performed[0]["type"], "configure_agent");
        assert_eq!(performed[0]["enabled"], false);

        // And an agent switched off is refused.
        let (rx, _task) = call(&agent, "vela_delete_group", json!({ "id": "g1" }));
        assert_eq!(
            outcome(rx.recv_timeout(WAIT).unwrap())["reason"]["code"],
            "agent_off"
        );
    }

    #[test]
    fn vela_state_is_the_shell_and_the_book_as_the_core_reads_them() {
        let (_fake, agent) = wallet();
        let (asked, _task) = call(&agent, "vela_delete_group", json!({ "id": "g1" }));
        until("asked", || approvals(&agent).len() == 1);
        let (rx, _task) = call(&agent, "vela_state", Value::Null);
        let Ok(Reply::Json(state)) = rx.recv_timeout(WAIT).unwrap() else {
            panic!("a reading");
        };
        assert_eq!(state["page"], "contacts");
        assert_eq!(state["groups"][0]["name"], "Team");
        assert_eq!(
            state["waiting_for_person"][0]["action"]["type"],
            "delete_group"
        );
        assert_eq!(state["transfers"], json!([]));
        agent.answer(approvals(&agent)[0]["id"].as_u64().unwrap(), false);
        assert_eq!(
            outcome(asked.recv_timeout(WAIT).unwrap())["reason"]["code"],
            "denied"
        );

        let (rx, _task) = call(&agent, "vela_nope", Value::Null);
        assert_eq!(
            outcome(rx.recv_timeout(WAIT).unwrap())["reason"]["code"],
            "invalid_input"
        );
    }

    #[test]
    fn settings_not_fully_set_up_stay_off() {
        let (_fake, agent) = wallet();
        for (enabled, url, token) in [
            (true, "", "token"),
            (true, "ws://bridge.example/connect", "token"),
            (true, "wss://bridge.example/connect", " "),
            (false, "wss://bridge.example/connect", "token"),
        ] {
            agent.configure(enabled, url, token);
            assert_eq!(agent.link(), Link::Off, "{enabled} {url} {token:?}");
            assert!(agent.inner.state().connection.is_none(), "nothing dialled");
        }
        assert!(!agent.enabled());
    }
}
