//! The desktop's send, as the shared transfers reach it
//! (`vela_agent::transfers`, the one copy of drafting and following a
//! transfer): the send column's host, the balances, the chain tracker. The
//! ledger is a gpui global, so the screens that confirm through it and the
//! agent's calls read the same one.

use std::time::Duration;

use gpui::{App, AsyncApp, Entity, Global};
use vela_agent::intents::Performed;
use vela_agent::transfers::{self as shared, SendDesk, Transfers};
use vela_core::app::balance_dashboard::{BalanceDashboard, BalanceToken};
use vela_core::app::intent_rules::TrackFacts;
use vela_core::app::intents::{Actor, Operation, Recipient};
use vela_core::app::send::{Event as SendEvent, SendOpenParams, SendView};
use vela_core::app::tx_tracker::TxTracker;

use crate::resident;
use crate::session;
use crate::wallet::money::SendHost;

#[derive(Default)]
struct Ledger(Transfers);

impl Global for Ledger {}

fn ledger(cx: &mut App) -> &mut Transfers {
    &mut cx.default_global::<Ledger>().0
}

/// The passkey's ceremony moved: `started` (its prompt is up) or `signed`.
/// Told by the send column (`SendHost::dispatch`).
pub fn ceremony(event: &str, cx: &mut App) {
    ledger(cx).ceremony(event);
}

/// The send open on the page, if any.
fn host(cx: &mut App) -> Option<Entity<SendHost>> {
    super::page(cx)?
        .read_with(cx, |page, _| page.agent_send())
        .ok()
        .flatten()
}

/// The open send, by its host's id, and its view.
fn open(cx: &mut App) -> Option<(u64, SendView)> {
    let host = host(cx)?;
    Some((host.entity_id().as_u64(), host.read(cx).view.clone()))
}

fn tracker(cx: &mut App) -> Vec<TrackFacts> {
    let entries = resident::resident::<TxTracker>(cx).read(cx).view().entries;
    serde_json::from_value(serde_json::json!(entries)).unwrap_or_default()
}

/// Bring every operation up to date with what the machines say now.
pub fn sync(cx: &mut App) {
    let open = open(cx);
    let tracker = tracker(cx);
    ledger(cx).sync(open.as_ref(), &tracker);
}

/// The operations as an agent reads them, each brought up to date.
pub fn list(cx: &mut App) -> Vec<Operation> {
    sync(cx);
    ledger(cx).list()
}

pub fn get(id: &str, cx: &mut App) -> Option<Operation> {
    sync(cx);
    ledger(cx).get(id)
}

/// Who drafted `id` onto its review.
pub fn drafted_by(id: &str, cx: &mut App) -> Option<Actor> {
    ledger(cx).drafted_by(id)
}

/// The open send's operation on its review, adopted if it has none yet — a
/// person who filled the form themselves confirms through the same intent.
pub fn current(cx: &mut App) -> Option<Operation> {
    let (session, view) = open(cx)?;
    ledger(cx).on_review(session, &view, shared::wall_ms())
}

/// The send column, from an async call.
struct Desk<'a> {
    cx: &'a mut AsyncApp,
}

impl SendDesk for Desk<'_> {
    fn open(&mut self) -> Option<(u64, SendView)> {
        self.cx.update(open)
    }

    fn alert(&mut self) -> Option<String> {
        self.cx
            .update(|cx| host(cx).and_then(|host| host.read(cx).alert.as_ref().map(shared::named)))
    }

    fn start(&mut self, params: SendOpenParams) -> Option<u64> {
        self.cx.update(|cx| {
            let page = super::page(cx)?;
            let host = page
                .update(cx, |page, cx| page.agent_open_send(params, cx))
                .ok()??;
            Some(host.entity_id().as_u64())
        })
    }

    fn dispatch(&mut self, event: SendEvent) {
        self.cx.update(|cx| {
            if let Some(host) = host(cx) {
                host.update(cx, |host, cx| host.dispatch(event, cx));
            }
        });
    }

    fn close(&mut self) {
        self.cx.update(|cx| {
            if let Some(page) = super::page(cx) {
                let _ = page.update(cx, |page, cx| page.agent_close_send(cx));
            }
        });
    }

    fn account(&mut self) -> String {
        self.cx.update(|cx| session::view(cx).address)
    }

    fn settled(&mut self, account: &str) -> bool {
        crate::executor::balance_dashboard::settled(account).is_some()
    }

    fn holdings(&mut self) -> Vec<BalanceToken> {
        self.cx.update(|cx| {
            resident::resident::<BalanceDashboard>(cx)
                .read(cx)
                .view()
                .tokens
        })
    }

    fn tracker(&mut self) -> Vec<TrackFacts> {
        self.cx.update(tracker)
    }

    fn network_id(&mut self, chain_id: u32) -> String {
        crate::executor::send::network_id(chain_id)
    }

    async fn sleep(&mut self, ms: u64) {
        self.cx
            .background_executor()
            .timer(Duration::from_millis(ms))
            .await;
    }

    fn ledger<R>(&mut self, change: impl FnOnce(&mut Transfers) -> R) -> R {
        self.cx.update(|cx| change(ledger(cx)))
    }

    fn halted(&mut self) -> bool {
        !self.cx.update(|cx| super::enabled(cx))
    }
}

pub async fn prepare(
    chain_id: u32,
    token: Option<&str>,
    to: &str,
    amount: &str,
    by: Actor,
    cx: &mut AsyncApp,
) -> Performed {
    shared::prepare(chain_id, token, to, amount, by, &mut Desk { cx }).await
}

pub async fn prepare_batch(
    chain_id: u32,
    token: Option<&str>,
    recipients: &[Recipient],
    by: Actor,
    cx: &mut AsyncApp,
) -> Performed {
    shared::prepare_batch(chain_id, token, recipients, by, &mut Desk { cx }).await
}

pub fn submit(id: &str, by: Actor, cx: &mut AsyncApp) -> Performed {
    shared::submit(id, by, &mut Desk { cx })
}

pub fn cancel(id: &str, cx: &mut AsyncApp) -> Performed {
    shared::cancel(id, &mut Desk { cx })
}
