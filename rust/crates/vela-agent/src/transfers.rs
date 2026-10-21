//! Transfers through the intents (spec 103): a draft lands on the wallet's
//! own review — the screen a person reaches by filling the form — and its
//! operation is followed from there to the chain. One copy for every native
//! shell (the web's `transfers.svelte.ts` is the same algorithm).
//!
//! Nothing about sending is decided here. Drafting is the send core's own
//! events, sent in the order the form's controls send them; where an operation
//! stands is READ — from the send while it is open, from the passkey's
//! ceremony, and from the chain tracker once it has a hash — by the core
//! (`intent_rules::operation_progress`). The shell's send is reached through
//! [`SendDesk`].

use vela_core::app::balance_dashboard::BalanceToken;
use vela_core::app::intent_rules::{self, Ceremony, Progress, SendFacts, TrackFacts};
use vela_core::app::intents::{Actor, Operation, OperationState, Reason, ReasonCode, Recipient};
use vela_core::app::send::{
    Event as SendEvent, SendOpenParams, SendRecipientDraft, SendStage, SendView,
};

use crate::intents::Performed;

/// A transfer as it is followed here.
#[derive(Clone, Debug)]
struct Tracked {
    operation: Operation,
    /// The send it was drafted on (the shell's own id for it).
    session: u64,
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
#[derive(Clone, Debug, Default)]
pub struct Transfers {
    operations: Vec<Tracked>,
    next: u64,
    ceremony: Option<Ceremony>,
    ceremonies: u64,
}

impl Transfers {
    /// The passkey's ceremony moved: `started` (its prompt is up) or `signed`.
    pub fn ceremony(&mut self, event: &str) {
        self.ceremonies += 1;
        self.ceremony = Some(Ceremony {
            event: event.to_owned(),
            seq: self.ceremonies,
        });
    }

    /// Who drafted `id` onto its review.
    #[must_use]
    pub fn drafted_by(&self, id: &str) -> Option<Actor> {
        self.find(id).map(|tracked| tracked.drafted_by)
    }

    fn find(&self, id: &str) -> Option<&Tracked> {
        self.operations
            .iter()
            .find(|tracked| tracked.operation.id == id)
    }

    /// The operation drafted on `session`, made if there is none.
    fn adopt(&mut self, session: u64, chain_id: Option<u32>) -> usize {
        if let Some(index) = self
            .operations
            .iter()
            .position(|tracked| tracked.session == session)
        {
            return index;
        }
        self.next += 1;
        self.operations.insert(
            0,
            Tracked {
                operation: Operation {
                    id: format!("transfer-{}", self.next),
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

    /// Bring every operation up to date: the open send (`open`, by its
    /// session), else the chain tracker, and the passkey's ceremony.
    pub fn sync(&mut self, open: Option<&(u64, SendView)>, tracker: &[TrackFacts]) {
        let ceremony = self.ceremony.clone();
        for tracked in &mut self.operations {
            if tracked.operation.state.is_final() {
                continue;
            }
            let send: Option<SendFacts> = open
                .filter(|(session, _)| *session == tracked.session)
                .and_then(|(_, view)| serde_json::to_value(view).ok())
                .and_then(|value| serde_json::from_value(value).ok());
            let progress = intent_rules::operation_progress(
                Progress {
                    operation: tracked.operation.clone(),
                    confirmed_after: tracked.confirmed_after,
                },
                send.as_ref(),
                ceremony.as_ref(),
                tracker,
            );
            tracked.operation = progress.operation;
        }
    }

    /// The operation of the send open as `session` with `view`, when it is on
    /// its review — adopted if it has none yet: a person who filled the form
    /// themselves confirms through the same intent.
    pub fn on_review(&mut self, session: u64, view: &SendView) -> Option<Operation> {
        if view.stage != SendStage::Confirm {
            return None;
        }
        let chain_id = view.selected_token.as_ref().map(|token| token.chain_id);
        let index = self.adopt(session, chain_id);
        Some(self.operations[index].operation.clone())
    }

    #[must_use]
    pub fn list(&self) -> Vec<Operation> {
        self.operations
            .iter()
            .map(|tracked| tracked.operation.clone())
            .collect()
    }

    #[must_use]
    pub fn get(&self, id: &str) -> Option<Operation> {
        self.find(id).map(|tracked| tracked.operation.clone())
    }
}

/// The shell's send, as the transfers reach it.
///
/// Implemented by the shells for their own concrete types, whose futures are
/// then known whole (`Send` included where the shell needs it).
#[allow(async_fn_in_trait)]
pub trait SendDesk {
    /// The send open now, by the shell's own id for it, and its view.
    fn open(&mut self) -> Option<(u64, SendView)>;
    /// The send's alert, by its name, while one is up.
    fn alert(&mut self) -> Option<String>;
    /// Open a send on `params` (the wallet's send screen); its id, or `None`
    /// when the wallet is not open.
    fn start(&mut self, params: SendOpenParams) -> Option<u64>;
    fn dispatch(&mut self, event: SendEvent);
    /// Leave the send, as its own Back off the form does.
    fn close(&mut self);
    /// The signed-in account.
    fn account(&mut self) -> String;
    /// Whether the balances of `account` have been read at least once.
    fn settled(&mut self, account: &str) -> bool;
    /// The holdings the balances show.
    fn holdings(&mut self) -> Vec<BalanceToken>;
    /// The chain tracker's entries.
    fn tracker(&mut self) -> Vec<TrackFacts>;
    /// A network as this shell's send spells it in `SendToken.network` (the
    /// core matches the two strings; any other spelling lands on the picker).
    fn network_id(&mut self, chain_id: u32) -> String;
    async fn sleep(&mut self, ms: u64);
    /// The transfers' own ledger, kept by the shell.
    fn ledger<R>(&mut self, change: impl FnOnce(&mut Transfers) -> R) -> R;
}

/// Bring the ledger up to date with what the machines say now.
pub fn sync(desk: &mut impl SendDesk) {
    let open = desk.open();
    let tracker = desk.tracker();
    desk.ledger(|ledger| ledger.sync(open.as_ref(), &tracker));
}

/// The operations as an agent reads them, each brought up to date.
pub fn list(desk: &mut impl SendDesk) -> Vec<Operation> {
    sync(desk);
    desk.ledger(|ledger| ledger.list())
}

pub fn get(id: &str, desk: &mut impl SendDesk) -> Option<Operation> {
    sync(desk);
    desk.ledger(|ledger| ledger.get(id))
}

/// The open send's operation, adopted if it has none yet — a person who
/// filled the form themselves confirms through the same intent.
pub fn current(desk: &mut impl SendDesk) -> Option<Operation> {
    let (session, view) = desk.open()?;
    desk.ledger(|ledger| ledger.on_review(session, &view))
}

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

/// A core value by its own name: `insufficient_balance`, a tagged one's `type`.
#[must_use]
pub fn named(value: &impl serde::Serialize) -> String {
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

/// How long each step of a draft may take before it is given up.
const OPEN_MS: u64 = 10_000;
const AMOUNT_MS: u64 = 15_000;
const REVIEW_MS: u64 = 20_000;
const POLL_MS: u64 = 50;

async fn until<D: SendDesk, T>(
    desk: &mut D,
    ms: u64,
    mut read: impl FnMut(&mut D) -> Option<T>,
) -> Option<T> {
    let mut waited = 0;
    loop {
        if let Some(value) = read(desk) {
            return Some(value);
        }
        if waited >= ms {
            return None;
        }
        desk.sleep(POLL_MS).await;
        waited += POLL_MS;
    }
}

fn view_of(desk: &mut impl SendDesk, session: u64) -> Option<SendView> {
    desk.open()
        .filter(|(open, _)| *open == session)
        .map(|(_, view)| view)
}

/// Says when the form holds the draft: `Some("ok")`, `Some(why not)`, or
/// `None` while it is still settling.
type Settled = Box<dyn Fn(&SendView) -> Option<String> + Send + Sync>;

/// Draft a transfer into the review. Answers the operation, or why it could
/// not be drafted — in which case nothing is left open.
pub async fn prepare(
    chain_id: u32,
    token: Option<&str>,
    to: &str,
    amount: &str,
    by: Actor,
    desk: &mut impl SendDesk,
) -> Performed {
    let amount = decimal(amount);
    draft(by, chain_id, token, Some(to), desk, move |desk| {
        desk.dispatch(if amount == "max" {
            SendEvent::TapMax
        } else {
            SendEvent::SetAmount {
                amount: amount.clone(),
            }
        });
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
    desk: &mut impl SendDesk,
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
    draft(by, chain_id, token, None, desk, move |desk| {
        desk.dispatch(SendEvent::SeedSplitRecipients { recipients: rows });
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

/// Open the send on the token, let `fill` put the draft on the form, then
/// Continue to the review — the order the form's own controls go in.
async fn draft<D: SendDesk>(
    by: Actor,
    chain_id: u32,
    contract: Option<&str>,
    recipient: Option<&str>,
    desk: &mut D,
    fill: impl FnOnce(&mut D) -> Settled,
) -> Performed {
    // What is open now: a receipt gives way to the next draft (its own Done),
    // anything else is the person's.
    let mut closed = None;
    if let Some((session, view)) = desk.open() {
        if view.stage == SendStage::Receipt {
            sync(desk);
            closed = desk.ledger(|ledger| {
                ledger
                    .operations
                    .iter()
                    .find(|tracked| tracked.session == session)
                    .map(|tracked| tracked.operation.id.clone())
            });
            desk.dispatch(SendEvent::Done);
            desk.close();
        } else {
            let review = (view.stage == SendStage::Confirm)
                .then(|| current(desk))
                .flatten();
            let busy = match review {
                Some(review) => format!(
                    "transfer {} is on its review ({}): submit or cancel it first",
                    review.id,
                    named(&review.state)
                ),
                None => "the person is filling in a send in the wallet: wait for them to finish or close it"
                    .to_owned(),
            };
            return refused(ReasonCode::Busy, busy);
        }
    }

    // "Not held" is only true of balances that have been read: right after
    // the page opens, they may not have been yet.
    let account = desk.account();
    let _ = until(desk, OPEN_MS, |desk| desk.settled(&account).then_some(())).await;
    let token = desk.holdings().into_iter().find(|held| {
        !held.spam
            && held.chain_id == chain_id
            && match (contract, &held.token_address) {
                (None, None) => true,
                (Some(wanted), Some(address)) => same(wanted, address),
                _ => false,
            }
    });
    let Some(token) = token else {
        return refused(
            ReasonCode::NotFound,
            "the wallet holds no such token on that chain",
        );
    };

    let params = SendOpenParams {
        preselected_symbol: Some(token.symbol.clone()),
        preselected_network: Some(desk.network_id(token.chain_id)),
        prefilled_recipient: recipient.map(|to| to.trim().to_owned()),
        ..SendOpenParams::default()
    };
    let Some(session) = desk.start(params) else {
        return refused(ReasonCode::NotReady, "the send did not open");
    };
    let entered = until(desk, OPEN_MS, |desk| {
        let view = view_of(desk, session)?;
        if view.loading {
            return None;
        }
        Some(view.selected_token.is_some() && view.stage == SendStage::EnterDetails)
    })
    .await;
    if entered != Some(true) {
        desk.close();
        return refused(ReasonCode::NotFound, "the send did not offer that token");
    }

    let settled = fill(desk);
    let ready = until(desk, AMOUNT_MS, |desk| match view_of(desk, session) {
        Some(view) => settled(&view),
        None => Some("the send was closed".to_owned()),
    })
    .await;
    if ready.as_deref() != Some("ok") {
        desk.close();
        let why = ready.unwrap_or_else(|| "the form could not be checked in time".to_owned());
        return refused(ReasonCode::Refused, why);
    }

    desk.dispatch(SendEvent::Continue);
    let reviewed = until(desk, REVIEW_MS, |desk| {
        if let Some(alert) = desk.alert() {
            return Some(format!("alert: {alert}"));
        }
        match view_of(desk, session) {
            None => Some("the send was closed".to_owned()),
            Some(view) => (view.stage == SendStage::Confirm).then(|| "ok".to_owned()),
        }
    })
    .await;
    if reviewed.as_deref() != Some("ok") {
        desk.close();
        let why = reviewed.unwrap_or_else(|| "the review did not open in time".to_owned());
        return refused(ReasonCode::Refused, why);
    }
    desk.ledger(|ledger| {
        let index = ledger.adopt(session, Some(token.chain_id));
        let tracked = &mut ledger.operations[index];
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
/// person did not choose to confirm — their own tap on the review is the
/// only way to it.
pub fn submit(id: &str, by: Actor, desk: &mut impl SendDesk) -> Performed {
    let Some((session, _)) = desk.open() else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    let known = desk.ledger(|ledger| {
        ledger
            .operations
            .iter()
            .position(|tracked| tracked.operation.id == id && tracked.session == session)
    });
    let Some(index) = known else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    sync(desk);
    let Some(view) = view_of(desk, session) else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
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
        // The review is the screen in front of the person; the capsule says
        // the agent asks them to confirm it. Nothing is pressed.
        return desk.ledger(|ledger| {
            let tracked = &mut ledger.operations[index];
            if tracked.operation.state == OperationState::AwaitingConfirmation {
                tracked.operation.reason =
                    Some("waiting for the person to confirm it on the review".to_owned());
            }
            Performed::Started {
                operation: tracked.operation.clone(),
                closed: tracked.closed.clone(),
            }
        });
    }
    if !view.can_confirm {
        return refused(ReasonCode::Refused, "the review cannot be confirmed yet");
    }
    desk.ledger(|ledger| {
        let ceremonies = ledger.ceremonies;
        let tracked = &mut ledger.operations[index];
        tracked.operation.reason = None;
        tracked.confirmed_after = Some(ceremonies);
    });
    desk.dispatch(SendEvent::SlideConfirm);
    sync(desk);
    desk.ledger(|ledger| {
        let tracked = &ledger.operations[index];
        Performed::Started {
            operation: tracked.operation.clone(),
            closed: tracked.closed.clone(),
        }
    })
}

/// Leave the operation's review without sending — a failed one's too.
pub fn cancel(id: &str, desk: &mut impl SendDesk) -> Performed {
    let Some((session, _)) = desk.open() else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    let known = desk.ledger(|ledger| {
        ledger
            .operations
            .iter()
            .position(|tracked| tracked.operation.id == id && tracked.session == session)
    });
    let Some(index) = known else {
        return refused(
            ReasonCode::NotFound,
            "that transfer is no longer on the review screen",
        );
    };
    desk.close();
    // A review that failed before anything was sent is left like any other,
    // and keeps why it failed. One still waiting says it was left: what it
    // was waiting for is over.
    desk.ledger(|ledger| {
        let tracked = &mut ledger.operations[index];
        if tracked.operation.state == OperationState::Failed
            && tracked.operation.user_op_hash.is_none()
        {
            tracked.operation.state = OperationState::Cancelled;
        } else {
            tracked.operation.reason = None;
        }
    });
    sync(desk);
    Performed::Done
}

#[cfg(test)]
mod tests {
    use crux_core::App as _;
    use futures::executor::block_on;
    use serde_json::json;
    use vela_core::app::send::{Model, Send, SendAmountWarning, SendToken};

    use super::*;

    const ARC: u32 = 5_042_002;
    const TO: &str = "0x56814A3966E19e12B3516e20ae46b1785fEE0329";

    fn token() -> SendToken {
        serde_json::from_value(json!({
            "network": "chain-5042002", "chain_id": ARC, "symbol": "USDC", "balance": "18.5",
            "decimals": 18, "token_address": null, "price_usd": 1.0, "logo_urls": [], "spam": false,
        }))
        .unwrap()
    }

    /// A send machine, played: what the form does with each event.
    #[derive(Default)]
    struct Fake {
        open: Option<(u64, SendView)>,
        sessions: u64,
        sent: Vec<String>,
        ledger: Transfers,
        /// The form refuses the amount with this warning.
        warning: Option<SendAmountWarning>,
    }

    impl Fake {
        fn view(&mut self) -> &mut SendView {
            &mut self.open.as_mut().expect("a send is open").1
        }
    }

    impl SendDesk for Fake {
        fn open(&mut self) -> Option<(u64, SendView)> {
            self.open.clone()
        }
        fn alert(&mut self) -> Option<String> {
            None
        }
        fn start(&mut self, params: SendOpenParams) -> Option<u64> {
            assert_eq!(params.preselected_symbol.as_deref(), Some("USDC"));
            assert_eq!(params.preselected_network.as_deref(), Some("chain-5042002"));
            self.sessions += 1;
            let mut view = Send.view(&Model::default());
            view.loading = false;
            view.stage = SendStage::EnterDetails;
            view.selected_token = Some(token());
            self.open = Some((self.sessions, view));
            Some(self.sessions)
        }
        fn dispatch(&mut self, event: SendEvent) {
            self.sent.push(named(&event));
            match event {
                SendEvent::SetAmount { .. } | SendEvent::TapMax => match self.warning.clone() {
                    Some(warning) => self.view().amount_warning = Some(warning),
                    None => self.view().can_continue = true,
                },
                SendEvent::Continue => self.view().stage = SendStage::Confirm,
                SendEvent::SlideConfirm => {
                    self.view().tx_status = serde_json::from_value(json!("preparing")).unwrap()
                }
                SendEvent::Done => self.open = None,
                _ => {}
            }
        }
        fn close(&mut self) {
            self.sent.push("close".into());
            self.open = None;
        }
        fn account(&mut self) -> String {
            "0xD400866e00B055B20752a826CD5C89b811de130b".into()
        }
        fn settled(&mut self, _: &str) -> bool {
            true
        }
        fn holdings(&mut self) -> Vec<BalanceToken> {
            vec![
                serde_json::from_value(json!({
                    "chain_id": ARC, "symbol": "USDC", "name": "USD Coin", "balance": "18.5",
                    "decimals": 18, "token_address": null, "price_usd": 1.0, "spam": false,
                }))
                .unwrap(),
            ]
        }
        fn tracker(&mut self) -> Vec<TrackFacts> {
            Vec::new()
        }
        fn network_id(&mut self, chain_id: u32) -> String {
            format!("chain-{chain_id}")
        }
        async fn sleep(&mut self, _: u64) {}
        fn ledger<R>(&mut self, change: impl FnOnce(&mut Transfers) -> R) -> R {
            change(&mut self.ledger)
        }
    }

    fn prepared(desk: &mut Fake, by: Actor) -> Performed {
        block_on(prepare(ARC, None, TO, ".1", by, desk))
    }

    #[test]
    fn a_draft_lands_on_the_review_in_the_forms_own_order() {
        let mut desk = Fake::default();
        let Performed::Started { operation, closed } = prepared(&mut desk, Actor::Agent) else {
            panic!("drafted");
        };
        assert_eq!(desk.sent, ["set_amount", "continue"]);
        assert_eq!(operation.state, OperationState::AwaitingConfirmation);
        assert_eq!(operation.chain_id, Some(ARC));
        assert_eq!(closed, None);
        assert_eq!(desk.ledger.drafted_by(&operation.id), Some(Actor::Agent));
    }

    #[test]
    fn a_review_still_waiting_is_busy_and_a_receipt_gives_way() {
        let mut desk = Fake::default();
        let Performed::Started {
            operation: first, ..
        } = prepared(&mut desk, Actor::Agent)
        else {
            panic!("drafted");
        };
        let Performed::Refused(reason) = prepared(&mut desk, Actor::Agent) else {
            panic!("busy");
        };
        assert_eq!(reason.code, ReasonCode::Busy);
        assert!(reason.detail.contains(&first.id), "{}", reason.detail);

        desk.view().stage = SendStage::Receipt;
        let Performed::Started {
            operation: second,
            closed,
        } = prepared(&mut desk, Actor::Agent)
        else {
            panic!("drafted after the receipt");
        };
        assert_eq!(
            closed,
            Some(first.id.clone()),
            "its receipt's Done closed it"
        );
        assert_ne!(second.id, first.id);
    }

    #[test]
    fn an_agents_submit_only_asks_and_the_persons_confirms() {
        let mut desk = Fake::default();
        let Performed::Started { operation, .. } = prepared(&mut desk, Actor::Agent) else {
            panic!("drafted");
        };
        desk.view().can_confirm = true;
        let Performed::Started {
            operation: asked, ..
        } = submit(&operation.id, Actor::Agent, &mut desk)
        else {
            panic!("asked");
        };
        assert!(
            !desk.sent.contains(&"slide_confirm".to_owned()),
            "the agent presses nothing"
        );
        assert_eq!(
            asked.reason.as_deref(),
            Some("waiting for the person to confirm it on the review")
        );

        desk.ledger.ceremony("started");
        submit(&operation.id, Actor::Person, &mut desk);
        assert_eq!(desk.sent.last().map(String::as_str), Some("slide_confirm"));
        // Its passkey is a ceremony begun after the confirm, not the one before.
        desk.ledger.ceremony("signed");
        sync(&mut desk);
        assert_eq!(
            desk.ledger.get(&operation.id).unwrap().state,
            OperationState::Submitting
        );
    }

    #[test]
    fn a_refused_amount_or_a_token_not_held_leaves_nothing_open() {
        let mut desk = Fake {
            warning: Some(SendAmountWarning::NotEnoughToken {
                symbol: "USDC".into(),
            }),
            ..Fake::default()
        };
        let Performed::Refused(reason) = prepared(&mut desk, Actor::Agent) else {
            panic!("refused");
        };
        assert_eq!(reason.code, ReasonCode::Refused);
        assert!(reason.detail.starts_with("amount: "), "{}", reason.detail);
        assert!(desk.open.is_none());

        let mut desk = Fake::default();
        let Performed::Refused(reason) =
            block_on(prepare(1, None, TO, "1", Actor::Agent, &mut desk))
        else {
            panic!("not held");
        };
        assert_eq!(reason.code, ReasonCode::NotFound);
        assert!(desk.sent.is_empty() && desk.open.is_none());
    }

    #[test]
    fn cancelling_leaves_the_review_and_says_so() {
        let mut desk = Fake::default();
        let Performed::Started { operation, .. } = prepared(&mut desk, Actor::Agent) else {
            panic!("drafted");
        };
        assert!(matches!(cancel(&operation.id, &mut desk), Performed::Done));
        assert!(desk.open.is_none());
        assert_eq!(
            desk.ledger.get(&operation.id).unwrap().state,
            OperationState::Cancelled
        );
    }
}
