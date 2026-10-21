//! Transfers through the intents (spec 103): a draft lands on the wallet's
//! own review — the screen a person reaches by filling the form — and its
//! operation is followed from there to the chain. The web's
//! `transfers.svelte.ts`, on the desktop's send column.
//!
//! Nothing about sending is decided here. Drafting is the send core's own
//! events, sent in the order the form's controls send them; where an operation
//! stands is READ — from the send while it is open, from the passkey's
//! ceremony, and from the chain tracker once it has a hash — by the core
//! (`intent_rules::operation_progress`), the same for every shell.

use std::time::Duration;

use gpui::{App, AsyncApp, Entity, EntityId, Global};
use vela_core::app::balance_dashboard::BalanceDashboard;
use vela_core::app::intent_rules::{self, Ceremony, Progress, SendFacts, TrackFacts};
use vela_core::app::intents::{Actor, Operation, OperationState, Reason, ReasonCode, Recipient};
use vela_core::app::send::{
    Event as SendEvent, SendOpenParams, SendRecipientDraft, SendStage, SendView,
};
use vela_core::app::tx_tracker::TxTracker;

use super::intents::Performed;
use crate::resident;
use crate::session;
use crate::wallet::money::SendHost;

/// A transfer as it is followed here.
struct Tracked {
    operation: Operation,
    /// The send it was drafted on.
    session: EntityId,
    /// How many passkey ceremonies had begun when confirm was pressed for it:
    /// a later one is its passkey. A count, not a clock.
    confirmed_after: Option<u64>,
    /// The finished transfer whose receipt this draft closed (its Done).
    closed: Option<String>,
    /// Who put it on the review.
    drafted_by: Actor,
}

/// Every operation drafted or confirmed this run, newest first, and the
/// passkey's last ceremony.
#[derive(Default)]
pub struct Transfers {
    operations: Vec<Tracked>,
    next: u64,
    ceremony: Option<Ceremony>,
    ceremonies: u64,
}

impl Global for Transfers {}

fn book(cx: &mut App) -> &mut Transfers {
    cx.default_global::<Transfers>()
}

/// The passkey's ceremony moved: `started` (its prompt is up) or `signed`.
/// Told by the send column (`SendHost::dispatch`).
pub fn ceremony(event: &str, cx: &mut App) {
    let transfers = book(cx);
    transfers.ceremonies += 1;
    transfers.ceremony = Some(Ceremony {
        event: event.to_owned(),
        seq: transfers.ceremonies,
    });
}

const FINAL: [OperationState; 3] = [
    OperationState::Confirmed,
    OperationState::Failed,
    OperationState::Cancelled,
];

fn reason(code: ReasonCode, detail: impl Into<String>) -> Reason {
    Reason {
        code,
        detail: detail.into(),
    }
}

fn refused(code: ReasonCode, detail: impl Into<String>) -> Performed {
    Performed::Refused(reason(code, detail))
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// A core value by its own name: `amount: insufficient_balance`.
fn named(value: &impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        Ok(other) => other
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| other.to_string(), str::to_owned),
        Err(_) => String::new(),
    }
}

/// `.5` as the form writes it: `0.5`.
fn decimal(amount: &str) -> String {
    let amount = amount.trim();
    if amount.starts_with('.') {
        format!("0{amount}")
    } else {
        amount.to_owned()
    }
}

/// The send open on the page, if any.
fn desk(cx: &mut App) -> Option<Entity<SendHost>> {
    super::page(cx)?
        .read_with(cx, |page, _| page.agent_send())
        .ok()
        .flatten()
}

/// Bring every operation up to date with what the machines say now.
pub fn sync(cx: &mut App) {
    let open = desk(cx).map(|host| (host.entity_id(), host.read(cx).view.clone()));
    let tracker: Vec<TrackFacts> = {
        let entries = resident::resident::<TxTracker>(cx).read(cx).view().entries;
        serde_json::from_value(serde_json::json!(entries)).unwrap_or_default()
    };
    let transfers = book(cx);
    let ceremony = transfers.ceremony.clone();
    for tracked in &mut transfers.operations {
        if FINAL.contains(&tracked.operation.state) {
            continue;
        }
        let send: Option<SendFacts> = open
            .as_ref()
            .filter(|(session, _)| *session == tracked.session)
            .and_then(|(_, view)| serde_json::from_value(serde_json::json!(view)).ok());
        let progress = intent_rules::operation_progress(
            Progress {
                operation: tracked.operation.clone(),
                confirmed_after: tracked.confirmed_after,
            },
            send.as_ref(),
            ceremony.as_ref(),
            &tracker,
        );
        tracked.operation = progress.operation;
    }
}

/// The operations as an agent reads them, each brought up to date.
pub fn list(cx: &mut App) -> Vec<Operation> {
    sync(cx);
    book(cx)
        .operations
        .iter()
        .map(|tracked| tracked.operation.clone())
        .collect()
}

pub fn get(id: &str, cx: &mut App) -> Option<Operation> {
    sync(cx);
    book(cx)
        .operations
        .iter()
        .find(|tracked| tracked.operation.id == id)
        .map(|tracked| tracked.operation.clone())
}

/// Who drafted `id` onto its review.
pub fn drafted_by(id: &str, cx: &mut App) -> Option<Actor> {
    book(cx)
        .operations
        .iter()
        .find(|tracked| tracked.operation.id == id)
        .map(|tracked| tracked.drafted_by)
}

/// The open send's operation, adopted if it has none yet — a person who
/// filled the form themselves confirms through the same intent.
pub fn current(cx: &mut App) -> Option<Operation> {
    let host = desk(cx)?;
    let view = host.read(cx).view.clone();
    if view.stage != SendStage::Confirm {
        return None;
    }
    let chain_id = view.selected_token.as_ref().map(|token| token.chain_id);
    let index = adopt(host.entity_id(), chain_id, cx);
    Some(book(cx).operations[index].operation.clone())
}

/// The operation drafted on `session`, made if there is none.
fn adopt(session: EntityId, chain_id: Option<u32>, cx: &mut App) -> usize {
    let transfers = book(cx);
    if let Some(index) = transfers
        .operations
        .iter()
        .position(|tracked| tracked.session == session)
    {
        return index;
    }
    transfers.next += 1;
    let id = format!("transfer-{}", transfers.next);
    transfers.operations.insert(
        0,
        Tracked {
            operation: Operation {
                id,
                state: OperationState::AwaitingConfirmation,
                chain_id,
                user_op_hash: None,
                tx_hash: None,
                reason: None,
            },
            session,
            confirmed_after: None,
            closed: None,
            drafted_by: Actor::Person,
        },
    );
    0
}

/// How long each step of a draft may take before it is given up.
const PAGE_MS: u64 = 8_000;
const OPEN_MS: u64 = 10_000;
const AMOUNT_MS: u64 = 15_000;
const REVIEW_MS: u64 = 20_000;

async fn until<T>(
    cx: &mut AsyncApp,
    ms: u64,
    mut read: impl FnMut(&mut App) -> Option<T>,
) -> Option<T> {
    let mut waited = 0;
    loop {
        if let Some(value) = cx.update(&mut read) {
            return Some(value);
        }
        if waited >= ms {
            return None;
        }
        cx.background_executor()
            .timer(Duration::from_millis(50))
            .await;
        waited += 50;
    }
}

fn view_of(host: &Entity<SendHost>, cx: &App) -> SendView {
    host.read(cx).view.clone()
}

fn close(cx: &mut App) {
    if let Some(page) = super::page(cx) {
        let _ = page.update(cx, |page, cx| page.agent_close_send(cx));
    }
}

/// Draft a transfer into the review. Answers the operation, or why it could
/// not be drafted — in which case nothing is left open.
pub async fn prepare(
    chain_id: u32,
    token: Option<&str>,
    to: &str,
    amount: &str,
    by: Actor,
    cx: &mut AsyncApp,
) -> Performed {
    let amount = decimal(amount);
    draft(by, chain_id, token, Some(to), cx, move |host, cx| {
        let event = if amount == "max" {
            SendEvent::TapMax
        } else {
            SendEvent::SetAmount {
                amount: amount.clone(),
            }
        };
        host.update(cx, |host, cx| host.dispatch(event, cx));
        Box::new(|view: &SendView| {
            if let Some(warning) = &view.amount_warning {
                return Some(format!("amount: {}", named(warning)));
            }
            view.can_continue.then(|| "ok".to_owned())
        })
    })
    .await
}

/// Draft one token to several recipients — the form's several-recipients
/// mode, its rows set as a whole as the importer sets them.
pub async fn prepare_batch(
    chain_id: u32,
    token: Option<&str>,
    recipients: &[Recipient],
    by: Actor,
    cx: &mut AsyncApp,
) -> Performed {
    let rows: Vec<SendRecipientDraft> = recipients
        .iter()
        .map(|row| SendRecipientDraft {
            id: String::new(),
            address: row.to.trim().to_owned(),
            amount: decimal(&row.amount),
            name: None,
        })
        .collect();
    let wanted = rows.len();
    draft(by, chain_id, token, None, cx, move |host, cx| {
        host.update(cx, |host, cx| {
            host.dispatch(
                SendEvent::SeedSplitRecipients {
                    recipients: rows.clone(),
                },
                cx,
            );
        });
        Box::new(move |view: &SendView| {
            if view.recipients.len() != wanted {
                return Some(format!(
                    "recipients: the form took {} of {wanted}",
                    view.recipients.len()
                ));
            }
            if let Some(issue) = view.split_row_issues.first() {
                return Some(format!(
                    "recipient {}: address {}, amount {}",
                    issue.ordinal,
                    named(&issue.address),
                    named(&issue.amount)
                ));
            }
            if view.split_over_balance {
                return Some("amount: the total is more than the balance".to_owned());
            }
            view.can_continue.then(|| "ok".to_owned())
        })
    })
    .await
}

/// Says when the form holds the draft: `Some("ok")`, `Some(why not)`, or
/// `None` while it is still settling.
type Settled = Box<dyn Fn(&SendView) -> Option<String>>;

/// Open the send on the token, let `fill` put the draft on the form, then
/// Continue to the review — the order the form's own controls go in.
async fn draft(
    by: Actor,
    chain_id: u32,
    contract: Option<&str>,
    recipient: Option<&str>,
    cx: &mut AsyncApp,
    fill: impl FnOnce(&Entity<SendHost>, &mut App) -> Settled,
) -> Performed {
    if cx.update(|cx| super::page(cx)).is_none() {
        return refused(ReasonCode::NotReady, "the wallet is not open");
    }
    // What is open now: a receipt gives way to the next draft (its own Done),
    // anything else is the person's.
    let mut closed = None;
    let busy = cx.update(|cx| {
        let host = desk(cx)?;
        let view = view_of(&host, cx);
        if view.stage == SendStage::Receipt {
            sync(cx);
            let session = host.entity_id();
            closed = book(cx)
                .operations
                .iter()
                .find(|tracked| tracked.session == session)
                .map(|tracked| tracked.operation.id.clone());
            host.update(cx, |host, cx| host.dispatch(SendEvent::Done, cx));
            close(cx);
            return None;
        }
        let review = (view.stage == SendStage::Confirm)
            .then(|| current(cx))
            .flatten();
        Some(match review {
            Some(review) => format!(
                "transfer {} is on its review ({}): submit or cancel it first",
                review.id,
                named(&review.state)
            ),
            None => {
                "the person is filling in a send in the wallet: wait for them to finish or close it"
                    .to_owned()
            }
        })
    });
    if let Some(busy) = busy {
        return refused(ReasonCode::Busy, busy);
    }

    // "Not held" is only true of balances that have been read: right after
    // the page opens, they may not have been yet.
    let account = cx.update(|cx| session::view(cx).address);
    let _ = until(cx, OPEN_MS, |_| {
        crate::executor::balance_dashboard::settled(&account).map(|_| ())
    })
    .await;
    let token = cx.update(|cx| {
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        view.tokens.into_iter().find(|held| {
            !held.spam
                && held.chain_id == chain_id
                && match (contract, &held.token_address) {
                    (None, None) => true,
                    (Some(wanted), Some(address)) => same(wanted, address),
                    _ => false,
                }
        })
    });
    let Some(token) = token else {
        return refused(
            ReasonCode::NotFound,
            "the wallet holds no such token on that chain",
        );
    };

    let params = SendOpenParams {
        preselected_symbol: Some(token.symbol.clone()),
        preselected_network: Some(crate::executor::send::network_id(token.chain_id)),
        prefilled_recipient: recipient.map(|to| to.trim().to_owned()),
        ..SendOpenParams::default()
    };
    let opened = cx.update(|cx| {
        let page = super::page(cx)?;
        page.update(cx, |page, cx| page.agent_open_send(params, cx))
            .ok()
            .flatten()
    });
    let Some(host) = opened else {
        return refused(ReasonCode::NotReady, "the send did not open");
    };
    let entered = until(cx, OPEN_MS.max(PAGE_MS), |cx| {
        let view = view_of(&host, cx);
        if view.loading {
            return None;
        }
        Some(view.selected_token.is_some() && view.stage == SendStage::EnterDetails)
    })
    .await;
    if entered != Some(true) {
        cx.update(close);
        return refused(ReasonCode::NotFound, "the send did not offer that token");
    }

    let settled = cx.update(|cx| fill(&host, cx));
    let ready = until(cx, AMOUNT_MS, |cx| settled(&view_of(&host, cx))).await;
    if ready.as_deref() != Some("ok") {
        cx.update(close);
        let why = ready.unwrap_or_else(|| "the form could not be checked in time".to_owned());
        return refused(ReasonCode::Refused, why);
    }

    cx.update(|cx| host.update(cx, |host, cx| host.dispatch(SendEvent::Continue, cx)));
    let reviewed = until(cx, REVIEW_MS, |cx| {
        let host = host.read(cx);
        if let Some(alert) = &host.alert {
            return Some(format!("alert: {}", named(alert)));
        }
        (host.view.stage == SendStage::Confirm).then(|| "ok".to_owned())
    })
    .await;
    if reviewed.as_deref() != Some("ok") {
        cx.update(close);
        let why = reviewed.unwrap_or_else(|| "the review did not open in time".to_owned());
        return refused(ReasonCode::Refused, why);
    }
    cx.update(|cx| {
        let index = adopt(host.entity_id(), Some(token.chain_id), cx);
        let tracked = &mut book(cx).operations[index];
        tracked.closed = closed;
        tracked.drafted_by = by;
        Performed::Started {
            operation: tracked.operation.clone(),
            closed: tracked.closed.clone(),
        }
    })
}

/// Confirm the operation's review. The person's confirm presses it, and their
/// passkey signs. An agent's only asks them (owner, 2026-10-21): the passkey
/// prompt shows no transaction, so it is never raised over a review the
/// person did not choose to confirm — their own click on the review is the
/// only way to it.
pub fn submit(id: &str, by: Actor, cx: &mut App) -> Performed {
    let Some(host) = desk(cx) else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    let session = host.entity_id();
    let Some(index) = book(cx)
        .operations
        .iter()
        .position(|tracked| tracked.operation.id == id && tracked.session == session)
    else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    sync(cx);
    let view = view_of(&host, cx);
    // One network, one nonce: the core holds the confirm while the account's
    // previous transfer there is still going through, and opens it itself.
    if let Some(waiting) = &view.previous_pending {
        return refused(
            ReasonCode::NotReady,
            format!(
                "the previous transfer on this network ({}) is still going through: the confirm opens once it is final",
                waiting.user_op_hash
            ),
        );
    }
    if by == Actor::Agent {
        // The review is the screen in front of the person; the bar says the
        // agent asks them to confirm it. Nothing is pressed.
        let tracked = &mut book(cx).operations[index];
        if tracked.operation.state == OperationState::AwaitingConfirmation {
            tracked.operation.reason =
                Some("waiting for the person to confirm it on the review".to_owned());
        }
        return Performed::Started {
            operation: tracked.operation.clone(),
            closed: tracked.closed.clone(),
        };
    }
    if !view.can_confirm {
        return refused(ReasonCode::Refused, "the review cannot be confirmed yet");
    }
    let transfers = book(cx);
    let ceremonies = transfers.ceremonies;
    let tracked = &mut transfers.operations[index];
    tracked.operation.reason = None;
    tracked.confirmed_after = Some(ceremonies);
    host.update(cx, |host, cx| host.dispatch(SendEvent::SlideConfirm, cx));
    sync(cx);
    let tracked = &book(cx).operations[index];
    Performed::Started {
        operation: tracked.operation.clone(),
        closed: tracked.closed.clone(),
    }
}

/// Leave the operation's review without sending — a failed one's too.
pub fn cancel(id: &str, cx: &mut App) -> Performed {
    let Some(host) = desk(cx) else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    let session = host.entity_id();
    let Some(index) = book(cx)
        .operations
        .iter()
        .position(|tracked| tracked.operation.id == id && tracked.session == session)
    else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    close(cx);
    // A review that failed before anything was sent is left like any other,
    // and keeps why it failed. One still waiting says it was left: what it
    // was waiting for is over.
    let tracked = &mut book(cx).operations[index];
    if tracked.operation.state == OperationState::Failed && tracked.operation.user_op_hash.is_none()
    {
        tracked.operation.state = OperationState::Cancelled;
    } else {
        tracked.operation.reason = None;
    }
    sync(cx);
    Performed::Done
}
