//! The desktop's end of the shared tools (`vela_agent::tools`): a call
//! arrives on the connection's thread, and its handler hands it to gpui
//! ([`Event::Call`]) and waits. There it runs through the same intents as the
//! person's own clicks. A call the bridge gives up on drops its handler, which
//! drops the reply's receiver: the gpui side sees that
//! ([`oneshot::Sender::is_canceled`]) and withdraws whatever still waits for
//! the person.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use clientmcp_client::{Close, HandlerResult, Reply, Status};
use futures::channel::{mpsc, oneshot};
use gpui::{App, AsyncApp};
use serde_json::{Value, json};
use vela_agent::tools::{self as shared, Forward, Wait};
use vela_core::app::intent_rules::{self, ReviewInput};
use vela_core::app::network_admin::NetworkAdmin;

use super::{transfers, world};
use crate::resident;
use crate::session;

/// From the connection's thread into gpui.
pub enum Event {
    Status {
        serial: u64,
        status: Status,
        close: Option<Close>,
    },
    Call {
        name: String,
        args: Value,
        reply: oneshot::Sender<HandlerResult>,
    },
}

/// Each tool's handler: hand the call to gpui, and wait for its answer.
pub fn forward(events: &mpsc::UnboundedSender<Event>) -> Forward {
    let events = events.clone();
    Arc::new(move |name, args| {
        let events = events.clone();
        Box::pin(async move {
            let (reply, answer) = oneshot::channel();
            events
                .unbounded_send(Event::Call { name, args, reply })
                .map_err(|_| "the wallet is closing".to_owned())?;
            answer
                .await
                .unwrap_or_else(|_| Err("the wallet is closing".to_owned()))
        })
    })
}

/// One event from the connection's thread, on gpui.
pub fn receive(event: Event, cx: &mut AsyncApp) {
    match event {
        Event::Status {
            serial,
            status,
            close,
        } => cx.update(|cx| super::status(serial, status, close, cx)),
        Event::Call { name, args, reply } => {
            cx.spawn(async move |cx| answer(name, args, reply, cx).await)
                .detach();
        }
    }
}

async fn answer(
    name: String,
    args: Value,
    reply: oneshot::Sender<HandlerResult>,
    cx: &mut AsyncApp,
) {
    let reply = Rc::new(RefCell::new(Some(reply)));
    let watched = Rc::clone(&reply);
    // The bridge gave up on the call (its timeout, a lost connection).
    let gone = move || {
        watched
            .borrow()
            .as_ref()
            .is_none_or(oneshot::Sender::is_canceled)
    };
    let result = match name.as_str() {
        "vela_state" => Ok(Reply::Json(cx.update(state))),
        "vela_review" => Ok(Reply::Json(cx.update(review))),
        "vela_wait" => Ok(wait(&args, &gone, cx).await),
        _ => match name.strip_prefix("vela_") {
            Some(kind) => Ok(match shared::action(kind, args) {
                Ok(action) => shared::reply(super::invoke_agent(action, &gone, cx).await),
                Err(refusal) => refusal,
            }),
            None => Err(format!("Unknown tool: {name}")),
        },
    };
    if let Some(sender) = reply.borrow_mut().take() {
        let _ = sender.send(result);
    }
}

/// The page the person is on, by its name (`welcome` while nobody is in).
fn page_name(cx: &mut App) -> String {
    super::page(cx)
        .and_then(|page| page.read_with(cx, |page, _| page.agent_page_name()).ok())
        .unwrap_or("welcome")
        .to_owned()
}

/// `vela_state`: the wallet as an agent reads it — the core decides what it
/// may see (hidden balances stay hidden; a provider key is only "set").
fn state(cx: &mut App) -> Value {
    let operations = transfers::list(cx);
    let world = world::world(cx, &operations, super::enabled(cx));
    let page = page_name(cx);
    let (approvals, activity) = super::book(cx)
        .map(shared::book_json)
        .unwrap_or_else(|| (json!([]), json!([])));
    intent_rules::agent_state(&world, &page, &approvals, &activity)
}

/// `vela_review`: what the person is being asked to sign, with the note that
/// none of it is an instruction to the agent.
fn review(cx: &mut App) -> Value {
    let page = super::page(cx);
    let send = page
        .as_ref()
        .and_then(|page| {
            page.read_with(cx, |page, _| page.agent_send())
                .ok()
                .flatten()
        })
        .map(|host| host.read(cx).view.clone());
    let reviewing = send
        .as_ref()
        .is_some_and(|view| view.stage == vela_core::app::send::SendStage::Confirm);
    let operation = reviewing.then(|| transfers::current(cx)).flatten();
    let drafted_by = operation
        .as_ref()
        .and_then(|op| transfers::drafted_by(&op.id, cx));
    let signing = page.as_ref().and_then(|page| {
        page.read_with(cx, |page, cx| page.agent_signing(cx))
            .ok()
            .flatten()
    });
    let account = session::view(cx).address;
    let networks = (!account.is_empty())
        .then(|| resident::resident::<NetworkAdmin>(cx).read(cx).view())
        .filter(|view| view.loaded);
    let input: ReviewInput = serde_json::from_value(json!({
        "send": send,
        "operation": operation,
        "drafted_by": drafted_by,
        "account": (!account.is_empty()).then_some(account),
        "networks": networks,
        "signing": signing,
    }))
    .unwrap_or_default();
    intent_rules::review(&input)
}

/// `vela_wait`: until the operation reaches one of `until` (or any final
/// state), time is up, or the agent's call is given up.
async fn wait(args: &Value, gone: &dyn Fn() -> bool, cx: &mut AsyncApp) -> Reply {
    let wait = Wait::of(args);
    let mut waited = 0;
    loop {
        let operation = cx.update(|cx| transfers::get(&wait.operation, cx));
        if let Some(reply) = wait.look(operation, waited, gone()) {
            return reply;
        }
        cx.background_executor()
            .timer(Duration::from_millis(shared::WAIT_POLL_MS))
            .await;
        waited += shared::WAIT_POLL_MS;
    }
}
