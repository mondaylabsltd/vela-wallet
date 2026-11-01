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
//! - the machine's answer goes to the browser machine as it is
//!   (`MultisigAnswered`, core-ms-2): a batch's id is then the browser
//!   machine's to keep (`vela.dapp.multisigBatches`, across a restart).
//! - `wallet_getCallsStatus` of such a batch: the browser machine's read of
//!   `MULTISIG_CALLS_STATUS_METHOD`, asked of the proposals machine
//!   (`CallsStatus`) and answered with its body ([`read_body`]).

use std::collections::BTreeMap;

use serde_json::{Value, json};
use vela_core::app::multisig_accounts::AccountRow;
use vela_core::app::multisig_dapp::{DappAnswer, HeldRequest, HeldStage};
use vela_core::app::multisig_proposals::{Event, ProposalsView};
use vela_core::app::multisig_shell::MultisigAccount;

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
    /// The machine answered: the site hears its answer, then the machine is
    /// told (`DappSettled`).
    Answer {
        rid: String,
        tab: String,
        id: String,
        /// The machine's answer, delivered as it is (`MultisigAnswered`).
        answer: Box<DappAnswer>,
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
            let close = (answer.error.is_some() && !held.told)
                .then(|| held.safe_tx_hash.clone().map(|h| (held.chain_id, h)))
                .flatten();
            steps.push(Step::Answer {
                rid: rid.clone(),
                tab: holding.tab.clone(),
                id: holding.id.clone(),
                answer: Box::new(answer.clone()),
                close,
            });
            self.held.remove(&rid);
        }
        steps
    }

    /// A batch read the browser asked: from the machine's last word on it
    /// when this is the read that follows one, else asked of the machine.
    pub fn calls(&mut self, read: CallsRead) -> Event {
        self.next += 1;
        let ask = format!("calls-{}", self.next);
        let event = Event::CallsStatus {
            id: ask.clone(),
            chain_id: u64::from(read.chain_id),
            calls_id: read.batch.clone(),
        };
        self.asks.insert(ask, read);
        event
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
            out.push((read.read, read_body(answer)));
        }
        out
    }
}

/// The machine's `wallet_getCallsStatus` answer as the body of the browser
/// machine's read of it (`MULTISIG_CALLS_STATUS_METHOD`): a JSON-RPC body,
/// `{result}` or `{error}`, delivered as it is — `data` kept (core-ms-2).
#[must_use]
pub fn read_body(answer: &DappAnswer) -> String {
    serde_json::to_string(answer).unwrap_or_else(|_| json!({ "result": null }).to_string())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;
    use vela_core::app::multisig_dapp::DappError;
    use vela_core::app::dapp_rpc;

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
            answer,
            close,
            ..
        } = &steps[0]
        else {
            panic!("{steps:?}")
        };
        assert_eq!(
            (tab.as_str(), id.as_str(), answer, close),
            ("t1", "7", &Box::new(DappAnswer::ok(json!("0xcd"))), &None)
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
        // The batch's id goes to the browser machine as the machine formed
        // it: from there it is the browser machine's batch to follow.
        let Some(Step::Answer { answer, .. }) = steps.last() else {
            panic!()
        };
        assert_eq!(answer.result.as_ref().and_then(dapp_rpc::batch_id_of).map(str::to_ascii_lowercase), Some(HASH.to_owned()));

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

    /// core-ms-2: the browser machine's read of a multisig batch is
    /// answered with the machine's own body — its status whole, or its
    /// error with its `data`.
    #[test]
    fn a_batch_read_is_the_machines_answer_as_it_is() {
        let status = dapp_rpc::multisig_calls_status(HASH, 100, None, false);
        let body: Value = serde_json::from_str(&read_body(&DappAnswer::ok(status.clone()))).unwrap();
        assert_eq!(body, json!({ "result": status }));
        let refused = DappError {
            code: 5730,
            message: "Unknown bundle id".into(),
            data: Some(json!({ "x": 1 })),
        };
        let body: Value = serde_json::from_str(&read_body(&DappAnswer::err(refused))).unwrap();
        assert_eq!(body["error"]["code"], 5730);
        assert_eq!(body["error"]["data"], json!({ "x": 1 }));
    }

    #[test]
    fn each_batch_read_is_asked_of_the_machine_once() {
        let mut holds = Holds::default();
        let read = |n: u64| CallsRead {
            read: n,
            method: vela_core::app::dapp_browser::MULTISIG_CALLS_STATUS_METHOD.into(),
            batch: HASH.into(),
            chain_id: 100,
        };
        let Event::CallsStatus { id, calls_id, .. } = holds.calls(read(1)) else {
            panic!()
        };
        assert_eq!(calls_id, HASH);
        let mut view = ProposalsView::default();
        let status = dapp_rpc::multisig_calls_status(HASH, 100, None, true);
        view.calls_status.insert(id, DappAnswer::ok(status.clone()));
        let answered = holds.calls_answered(&view);
        assert_eq!(answered.len(), 1);
        assert_eq!(answered[0].0, 1);
        assert!(answered[0].1.contains("\"status\""));
        assert!(holds.calls_answered(&view).is_empty(), "once");
    }
}
