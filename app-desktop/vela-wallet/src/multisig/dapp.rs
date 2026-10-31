//! A site's request to a multisig, from the in-app browser (AUDIT M5) — the
//! web's `shell/dapp-requests.svelte.ts` and the multisig part of
//! `DappRequestHost.svelte`, over the SAME proposals machine
//! (`multisig_proposals::Event::DappRequest`, `vela_core::app::multisig_dapp`).
//!
//! The machine decides everything: what a multisig refuses (a message —
//! dapp-7), how the site's transaction reads, its review before any passkey,
//! when it is answered and with what (the transaction that executed it, a
//! batch's id, "proposed, waiting", the person's 4001). This file only
//! carries the request between the browser machine and the proposals
//! machine, gpui-free so the scenarios drive it:
//!
//! - [`multisig_of`]: is the account the site was granted a multisig on this
//!   device? (Then its request is the proposals machine's, never the Vela
//!   signing pipeline's.)
//! - [`Holds`]: the requests handed over, by the machine's id, and what each
//!   view of the machine asks of the shell ([`Step`]): open the proposal's
//!   sheet once it exists, answer the site once the machine answered it
//!   (then `DappSettled`).
//! - [`payload`]: the machine's answer, as the browser machine's
//!   `SigningAnswered` carries it to the page.
//! - `wallet_getCallsStatus` of a batch a multisig answered with: asked of
//!   the machine (`CallsStatus`), and its answer handed to the browser
//!   machine's read ([`read_body`]).

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vela_core::app::dapp_rpc;
use vela_core::app::multisig_accounts::AccountRow;
use vela_core::app::multisig_dapp::{DappAnswer, HeldRequest, HeldStage, declined};
use vela_core::app::multisig_proposals::{Event, ProposalsView};
use vela_core::app::multisig_shell::MultisigAccount;
use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

use crate::executor::dapp_browser::Forwarded;

/// The multisig on this device a site was granted, if `granted` is one —
/// the request is then the proposals machine's.
#[must_use]
pub fn multisig_of<'a>(granted: &str, accounts: &'a [AccountRow]) -> Option<&'a MultisigAccount> {
    accounts
        .iter()
        .map(|row| &row.account)
        .find(|account| account.address.eq_ignore_ascii_case(granted))
}

/// The machine's event for a site's request to `account` (`dapp_request`):
/// the site's chain, as the browser machine keeps it; `limit_ms`, how long
/// from the claim before the passkey the site can still be answered.
#[must_use]
pub fn request_event(
    rid: &str,
    account: &MultisigAccount,
    asked: &Forwarded,
    limit_ms: Option<f64>,
) -> Event {
    Event::DappRequest {
        id: rid.to_owned(),
        account: Box::new(account.clone()),
        method: asked.method.clone(),
        params: serde_json::from_str(&asked.params_json).unwrap_or(Value::Null),
        origin: asked.origin.clone(),
        chain_id: u64::from(asked.chain_id),
        limit_ms,
    }
}

/// The machine's answer as the browser machine carries it to the page
/// (`SigningAnswered`). A decline is the person's own rejection, worded by
/// the core; any other error keeps the machine's code and message exactly
/// (carried as a refusal with its own words, as the busy answer is).
#[must_use]
pub fn payload(answer: &DappAnswer) -> SignResponsePayload {
    match &answer.error {
        None => SignResponsePayload::Ok {
            result: Some(answer.result.clone().unwrap_or(Value::Null)),
        },
        Some(error) if *error == declined() => SignResponsePayload::Err {
            code: i32::try_from(error.code).unwrap_or(4001),
            kind: SignErrorKind::UserRejected,
            message: None,
        },
        Some(error) => SignResponsePayload::Err {
            code: i32::try_from(error.code).unwrap_or(-32603),
            kind: SignErrorKind::SubmitFailed,
            message: Some(error.message.clone()),
        },
    }
}

/// One request handed to the machine.
#[derive(Clone, Debug)]
struct Holding {
    tab: String,
    id: String,
    /// What the machine was asked (asked again on the person's Retry).
    ask: Event,
    /// Its proposal's sheet was opened.
    opened: bool,
}

/// What a view of the machine asks of the shell.
#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    /// The request's proposal exists: its sheet opens on it (once).
    Open { chain_id: u64, safe_tx_hash: String },
    /// The machine answered: the site hears `payload`, then the machine is
    /// told (`DappSettled`).
    Answer {
        rid: String,
        tab: String,
        id: String,
        payload: SignResponsePayload,
        /// A batch answered its id: `wallet_getCallsStatus` follows it here.
        batch: Option<(String, u32)>,
        /// An error the site was not told of before: its sheet goes.
        close: Option<(u64, String)>,
    },
}

/// A `wallet_getCallsStatus` read the browser machine asked, for a batch a
/// multisig answered with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallsRead {
    /// The browser machine's pending read.
    pub read: u64,
    /// The bundler method the browser asked (its receipt, then its status).
    pub method: String,
    pub batch: String,
    pub chain_id: u32,
}

/// What to do for a [`CallsRead`].
#[derive(Clone, Debug)]
pub enum CallsNext {
    /// Ask the machine (`CallsStatus`).
    Ask(Box<Event>),
    /// Answer the read now, from the machine's last word on the batch.
    Answer { read: u64, body_json: String },
}

/// The requests handed to the proposals machine, and the batch reads out.
#[derive(Debug, Default)]
pub struct Holds {
    next: u64,
    held: BTreeMap<String, Holding>,
    /// The request whose proposal's sheet is drawn: as the machine last
    /// held it — kept after it was answered (a sheet whose site was told
    /// stays, saying so).
    seen: Option<HeldRequest>,
    /// `CallsStatus` asked: the machine's id → the read it answers.
    asks: BTreeMap<String, CallsRead>,
    /// The machine's last status of a batch, for the browser's second read.
    last: BTreeMap<String, DappAnswer>,
}

impl Holds {
    /// Hand `asked` over: the machine's id for it.
    /// Hand `asked`, granted `account`, over: the machine's event for it
    /// ([`request_event`]), under an id of its own.
    pub fn hold(
        &mut self,
        asked: &Forwarded,
        account: &MultisigAccount,
        limit_ms: Option<f64>,
    ) -> Event {
        self.next += 1;
        let rid = format!("dapp-{}", self.next);
        let ask = request_event(&rid, account, asked, limit_ms);
        self.held.insert(
            rid,
            Holding {
                tab: asked.tab.clone(),
                id: asked.id.clone(),
                ask: ask.clone(),
                opened: false,
            },
        );
        ask
    }

    /// The person's Retry on a request whose reading failed: asked again.
    #[must_use]
    pub fn again(&self, rid: &str) -> Option<Event> {
        self.held.get(rid).map(|h| h.ask.clone())
    }

    /// Nothing is held.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }

    /// The machine's id of the browser's request `id` in `tab`, if held.
    #[must_use]
    pub fn find(&self, tab: &str, id: &str) -> Option<String> {
        self.held
            .iter()
            .find(|(_, h)| h.tab == tab && h.id == id)
            .map(|(rid, _)| rid.clone())
    }

    /// The held request still at `stage`, unanswered (the one on screen).
    #[must_use]
    pub fn at(&self, view: &ProposalsView, stage: HeldStage) -> Option<(String, HeldRequest)> {
        self.held.keys().find_map(|rid| {
            view.dapp
                .get(rid)
                .filter(|h| h.stage == stage && h.answer.is_none())
                .map(|h| (rid.clone(), h.clone()))
        })
    }

    /// The held request waiting on proposal `hash`, unanswered.
    #[must_use]
    pub fn waiting_on(&self, view: &ProposalsView, chain_id: u64, hash: &str) -> Option<String> {
        self.held.keys().find_map(|rid| {
            view.dapp
                .get(rid)
                .filter(|h| {
                    h.answer.is_none()
                        && h.chain_id == chain_id
                        && h.safe_tx_hash
                            .as_deref()
                            .is_some_and(|s| s.eq_ignore_ascii_case(hash))
                })
                .map(|_| rid.clone())
        })
    }

    /// The request drawn with proposal `hash`'s sheet, as last seen.
    #[must_use]
    pub fn seen(&self, chain_id: u64, hash: &str) -> Option<&HeldRequest> {
        self.seen.as_ref().filter(|h| {
            h.chain_id == chain_id
                && h.safe_tx_hash
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case(hash))
        })
    }

    /// The sheet went: nothing is drawn with it any more.
    pub fn unsee(&mut self) {
        self.seen = None;
    }

    /// What the machine's `view` asks of the shell now — each step once.
    pub fn step(&mut self, view: &ProposalsView) -> Vec<Step> {
        let mut steps = Vec::new();
        let rids: Vec<String> = self.held.keys().cloned().collect();
        for rid in rids {
            let Some(held) = view.dapp.get(&rid) else {
                continue;
            };
            let Some(holding) = self.held.get_mut(&rid) else {
                continue;
            };
            if let Some(hash) = held.safe_tx_hash.clone() {
                self.seen = Some(held.clone());
                if !holding.opened {
                    holding.opened = true;
                    steps.push(Step::Open {
                        chain_id: held.chain_id,
                        safe_tx_hash: hash,
                    });
                }
            }
            let Some(answer) = &held.answer else {
                continue;
            };
            let batch = (held.method == "wallet_sendCalls")
                .then_some(answer.result.as_ref())
                .flatten()
                .and_then(dapp_rpc::batch_id_of)
                .map(|id| {
                    (
                        id.to_ascii_lowercase(),
                        u32::try_from(held.chain_id).unwrap_or_default(),
                    )
                });
            let close = (answer.error.is_some() && !held.told)
                .then(|| held.safe_tx_hash.clone().map(|h| (held.chain_id, h)))
                .flatten();
            steps.push(Step::Answer {
                rid: rid.clone(),
                tab: holding.tab.clone(),
                id: holding.id.clone(),
                payload: payload(answer),
                batch,
                close,
            });
            self.held.remove(&rid);
        }
        steps
    }

    /// A batch read the browser asked: from the machine's last word on it
    /// when this is the read that follows one, else asked of the machine.
    pub fn calls(&mut self, read: CallsRead) -> CallsNext {
        if read.method == dapp_rpc::USER_OP_STATUS_METHOD
            && let Some(last) = self.last.remove(&read.batch)
        {
            return CallsNext::Answer {
                read: read.read,
                body_json: read_body(&read.method, &last),
            };
        }
        self.next += 1;
        let ask = format!("calls-{}", self.next);
        let event = Event::CallsStatus {
            id: ask.clone(),
            chain_id: u64::from(read.chain_id),
            calls_id: read.batch.clone(),
        };
        self.asks.insert(ask, read);
        CallsNext::Ask(Box::new(event))
    }

    /// The batch reads the machine answered: `(read, body)` each, once.
    pub fn calls_answered(&mut self, view: &ProposalsView) -> Vec<(u64, String)> {
        let done: Vec<String> = self
            .asks
            .keys()
            .filter(|ask| view.calls_status.contains_key(*ask))
            .cloned()
            .collect();
        let mut out = Vec::new();
        for ask in done {
            let (Some(read), Some(answer)) = (self.asks.remove(&ask), view.calls_status.get(&ask))
            else {
                continue;
            };
            if read.method != dapp_rpc::USER_OP_STATUS_METHOD {
                self.last.insert(read.batch.clone(), answer.clone());
            }
            out.push((read.read, read_body(&read.method, answer)));
        }
        out
    }
}

/// The machine's `wallet_getCallsStatus` answer (`multisig_calls_status`),
/// as the bundler read the browser machine asked would have answered it: the
/// browser machine forms the site's answer from it
/// (`dapp_rpc::calls_status`), and that answer is the machine's, field for
/// field (proved in the tests below). The browser machine knows only a user
/// operation's batch (a core gap: it has no multisig batch), so its read is
/// answered in its own shape.
///
/// - its receipt read (`eth_getUserOperationReceipt`): the execution's
///   receipt when it ran (200 / 500), else none;
/// - its status read ([`dapp_rpc::USER_OP_STATUS_METHOD`], asked when there
///   is no receipt): refused before any block when the number was used by
///   another transaction (400), else nothing.
///
/// An error from the machine (an id it does not know) reads as nothing yet:
/// the browser machine answers a batch read it cannot read as pending.
#[must_use]
pub fn read_body(method: &str, answer: &DappAnswer) -> String {
    let status = answer.result.as_ref();
    let code = status.and_then(|s| s.get("status")).and_then(Value::as_u64);
    if method == dapp_rpc::USER_OP_STATUS_METHOD {
        let result = if code == Some(u64::from(dapp_rpc::CALLS_STATUS_OFFCHAIN_FAILURE)) {
            json!({ "status": "rejected" })
        } else {
            Value::Null
        };
        return json!({ "result": result }).to_string();
    }
    let receipt = status
        .and_then(|s| s.get("receipts"))
        .and_then(|r| r.get(0))
        .filter(|_| matches!(code, Some(200 | 500)));
    let result = receipt.map_or(Value::Null, |r| {
        let field = |key: &str| r.get(key).cloned().unwrap_or(Value::Null);
        json!({
            "success": code == Some(200),
            "logs": field("logs"),
            "receipt": {
                "transactionHash": field("transactionHash"),
                "blockHash": field("blockHash"),
                "blockNumber": field("blockNumber"),
                "gasUsed": field("gasUsed"),
                "logs": field("logs"),
            },
        })
    });
    json!({ "result": result }).to_string()
}

/// The batch a browser read asks about, when it is a `wallet_getCallsStatus`
/// read of one a multisig answered with: its receipt, or its status.
#[must_use]
pub fn batch_read(
    method: &str,
    params_json: &str,
    bundler: bool,
    batches: &BTreeMap<String, u32>,
) -> Option<(String, u32)> {
    if !bundler
        || !(method == "eth_getUserOperationReceipt" || method == dapp_rpc::USER_OP_STATUS_METHOD)
    {
        return None;
    }
    let params: Value = serde_json::from_str(params_json).ok()?;
    let batch = dapp_rpc::calls_status_id(&params)?;
    batches.get(&batch).map(|chain| (batch, *chain))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use vela_core::app::multisig_dapp::{DappError, left_proposed, multisig_refusal};

    const HASH: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

    fn asked(method: &str) -> Forwarded {
        Forwarded {
            tab: "t1".into(),
            id: "7".into(),
            method: method.into(),
            params_json: r#"[{"to":"0x000000000000000000000000000000000000dEaD","value":"0x1"}]"#
                .into(),
            origin: "https://app.example".into(),
            chain_id: 100,
            granted_address: "0xAbC0000000000000000000000000000000000001".into(),
        }
    }

    fn row(address: &str) -> AccountRow {
        serde_json::from_value(json!({
            "account": {
                "address": address,
                "genesis": { "factory": "0x", "singleton": "0x", "saltNonce": "0x0", "initializer": "0x" },
                "owners": [],
                "threshold": 2,
                "recorded": false,
                "chainIds": [100],
            },
            "name": null,
            "shared": false,
            "record_waiting": false,
            "local_members": [],
            "high_risk_on": [],
            "refused_on": [],
            "checked_on": [],
            "checking_on": [],
            "invite_shared": false,
        }))
        .unwrap()
    }

    #[test]
    fn a_grant_to_a_multisig_here_is_the_machines() {
        let rows = [row("0xabc0000000000000000000000000000000000001")];
        assert!(multisig_of("0xAbC0000000000000000000000000000000000001", &rows).is_some());
        assert!(multisig_of("0x0000000000000000000000000000000000000002", &rows).is_none());
        assert!(multisig_of("0xabc", &[]).is_none());
    }

    #[test]
    fn the_request_event_carries_the_sites_chain_and_params() {
        let rows = [row("0xabc0000000000000000000000000000000000001")];
        let event = request_event(
            "dapp-1",
            &rows[0].account,
            &asked("eth_sendTransaction"),
            Some(1.0),
        );
        let Event::DappRequest {
            id,
            method,
            params,
            origin,
            chain_id,
            limit_ms,
            ..
        } = event
        else {
            panic!("not a dapp request");
        };
        assert_eq!(
            (
                id.as_str(),
                method.as_str(),
                origin.as_str(),
                chain_id,
                limit_ms
            ),
            (
                "dapp-1",
                "eth_sendTransaction",
                "https://app.example",
                100,
                Some(1.0)
            )
        );
        assert_eq!(params[0]["value"], "0x1");
    }

    #[test]
    fn every_answer_keeps_the_machines_code_and_words() {
        assert_eq!(
            payload(&DappAnswer::ok(json!("0xabc"))),
            SignResponsePayload::Ok {
                result: Some(json!("0xabc"))
            }
        );
        assert_eq!(
            payload(&DappAnswer::err(declined())),
            SignResponsePayload::Err {
                code: 4001,
                kind: SignErrorKind::UserRejected,
                message: None
            }
        );
        let proposed = left_proposed(HASH);
        let SignResponsePayload::Err {
            code,
            kind,
            message,
        } = payload(&DappAnswer::err(proposed.clone()))
        else {
            panic!()
        };
        assert_eq!((code, kind), (-32000, SignErrorKind::SubmitFailed));
        // The words the page reads are the machine's, whole.
        assert_eq!(
            dapp_rpc::sign_error_words(kind, message.as_deref()),
            proposed.message
        );
        let refusal = multisig_refusal("personal_sign").unwrap();
        let SignResponsePayload::Err { code, .. } = payload(&DappAnswer::err(refusal)) else {
            panic!()
        };
        assert_eq!(code, 4200);
    }

    fn hold(holds: &mut Holds, method: &str) -> String {
        let account = row("0xabc0000000000000000000000000000000000001").account;
        let Event::DappRequest { id, .. } = holds.hold(&asked(method), &account, None) else {
            panic!("not a dapp request");
        };
        assert!(
            matches!(holds.again(&id), Some(Event::DappRequest { id: again, .. }) if again == id),
            "asked again as it was"
        );
        id
    }

    fn held(rid: &str, stage: HeldStage) -> HeldRequest {
        HeldRequest {
            id: rid.into(),
            method: "eth_sendTransaction".into(),
            origin: "https://app.example".into(),
            chain_id: 100,
            safe: "0xabc".into(),
            stage,
            calls: Vec::new(),
            safe_tx_hash: None,
            nonce: None,
            tell_at: None,
            told: false,
            refusal: None,
            answer: None,
        }
    }

    #[test]
    fn a_held_request_opens_its_sheet_once_and_is_answered_once() {
        let mut holds = Holds::default();
        let rid = hold(&mut holds, "eth_sendTransaction");
        assert_eq!(holds.find("t1", "7"), Some(rid.clone()));
        let mut view = ProposalsView::default();
        view.dapp
            .insert(rid.clone(), held(&rid, HeldStage::Reviewing));
        assert!(holds.step(&view).is_empty(), "reviewed: nothing yet");
        assert!(holds.at(&view, HeldStage::Reviewing).is_some());

        let waiting = view.dapp.get_mut(&rid).unwrap();
        waiting.stage = HeldStage::Waiting;
        waiting.safe_tx_hash = Some(HASH.into());
        assert_eq!(
            holds.step(&view),
            vec![Step::Open {
                chain_id: 100,
                safe_tx_hash: HASH.into()
            }]
        );
        assert!(holds.step(&view).is_empty(), "opened once");
        assert_eq!(
            holds.waiting_on(&view, 100, &HASH.to_uppercase()),
            Some(rid.clone())
        );

        view.dapp
            .get_mut(&rid)
            .unwrap()
            .settle(DappAnswer::ok(json!("0xcd")));
        let steps = holds.step(&view);
        assert_eq!(steps.len(), 1);
        let Step::Answer {
            tab,
            id,
            batch,
            close,
            ..
        } = &steps[0]
        else {
            panic!("{steps:?}")
        };
        assert_eq!(
            (tab.as_str(), id.as_str(), batch, close),
            ("t1", "7", &None, &None)
        );
        assert!(holds.is_empty());
        assert!(holds.step(&view).is_empty(), "answered once");
        assert!(holds.seen(100, HASH).is_some(), "the sheet keeps it");
    }

    #[test]
    fn a_batch_answered_is_followed_and_an_untold_error_takes_its_sheet() {
        let mut holds = Holds::default();
        let rid = hold(&mut holds, "wallet_sendCalls");
        let mut view = ProposalsView::default();
        let mut h = held(&rid, HeldStage::Waiting);
        h.method = "wallet_sendCalls".into();
        h.safe_tx_hash = Some(HASH.into());
        h.settle(DappAnswer::ok(
            json!({ "id": HASH.to_uppercase().replace("0X", "0x") }),
        ));
        view.dapp.insert(rid.clone(), h);
        let steps = holds.step(&view);
        let Some(Step::Answer { batch, .. }) = steps.last() else {
            panic!()
        };
        assert_eq!(batch, &Some((HASH.to_owned(), 100)));

        let rid = hold(&mut holds, "eth_sendTransaction");
        let mut h = held(&rid, HeldStage::Waiting);
        h.safe_tx_hash = Some(HASH.into());
        h.settle(DappAnswer::err(DappError {
            code: 4001,
            message: "The members cancelled the transaction".into(),
            data: None,
        }));
        view.dapp.insert(rid, h);
        let steps = holds.step(&view);
        let Some(Step::Answer { close, .. }) = steps.last() else {
            panic!()
        };
        assert_eq!(close, &Some((100, HASH.to_owned())));
    }

    /// The browser machine's answer from [`read_body`] is the multisig
    /// machine's status, field for field: pending, executed, reverted, and
    /// its number used by another transaction.
    #[test]
    fn a_batch_read_answers_the_machines_status_exactly() {
        let receipt = json!({
            "transactionHash": format!("0x{}", "cd".repeat(32)),
            "blockHash": format!("0x{}", "ef".repeat(32)),
            "blockNumber": "0x64",
            "gasUsed": "0x5208",
            "status": "0x1",
            "logs": [{ "address": "0xabc" }],
        });
        let mut reverted = receipt.clone();
        reverted["status"] = json!("0x0");
        for (execution, used) in [
            (None, false),
            (Some(&receipt), false),
            (Some(&reverted), false),
            (None, true),
        ] {
            let status = dapp_rpc::multisig_calls_status(HASH, 100, execution, used);
            let answer = DappAnswer::ok(status.clone());
            let first: Value =
                serde_json::from_str(&read_body("eth_getUserOperationReceipt", &answer)).unwrap();
            let second: Value =
                serde_json::from_str(&read_body(dapp_rpc::USER_OP_STATUS_METHOD, &answer)).unwrap();
            let landed = dapp_rpc::calls_status_landed(first.get("result"));
            let relay = (!landed).then(|| second["result"].clone());
            let browser = dapp_rpc::calls_status(
                HASH,
                100,
                first.get("result").filter(|r| !r.is_null()),
                relay.as_ref().filter(|r| !r.is_null()),
            );
            assert_eq!(browser, status, "{execution:?} {used}");
        }
    }

    #[test]
    fn only_a_multisigs_batch_read_is_the_machines() {
        let batches = BTreeMap::from([(HASH.to_owned(), 100)]);
        let params = format!(r#"["{}"]"#, HASH.to_uppercase().replace("0X", "0x"));
        assert_eq!(
            batch_read("eth_getUserOperationReceipt", &params, true, &batches),
            Some((HASH.to_owned(), 100))
        );
        assert!(batch_read(dapp_rpc::USER_OP_STATUS_METHOD, &params, true, &batches).is_some());
        assert!(batch_read("eth_getUserOperationReceipt", &params, false, &batches).is_none());
        assert!(batch_read("eth_getTransactionReceipt", &params, true, &batches).is_none());
        assert!(
            batch_read(
                "eth_getUserOperationReceipt",
                &params,
                true,
                &BTreeMap::new()
            )
            .is_none()
        );
    }

    #[test]
    fn the_status_read_after_a_receipt_read_is_answered_from_it() {
        let mut holds = Holds::default();
        let read = |n: u64, method: &str| CallsRead {
            read: n,
            method: method.into(),
            batch: HASH.into(),
            chain_id: 100,
        };
        let CallsNext::Ask(ask) = holds.calls(read(1, "eth_getUserOperationReceipt")) else {
            panic!()
        };
        let Event::CallsStatus { id, calls_id, .. } = *ask else {
            panic!()
        };
        assert_eq!(calls_id, HASH);
        let mut view = ProposalsView::default();
        view.calls_status.insert(
            id,
            DappAnswer::ok(dapp_rpc::multisig_calls_status(HASH, 100, None, true)),
        );
        let answered = holds.calls_answered(&view);
        assert_eq!(answered.len(), 1);
        assert!(holds.calls_answered(&view).is_empty(), "once");
        let CallsNext::Answer { read, body_json } =
            holds.calls(read(2, dapp_rpc::USER_OP_STATUS_METHOD))
        else {
            panic!()
        };
        assert_eq!(read, 2);
        assert!(body_json.contains("rejected"));
    }
}
