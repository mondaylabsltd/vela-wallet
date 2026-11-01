//! `VELA_MULTISIG_SCRIPT=<steps.json>` (developer builds with
//! `dev-fixtures` only): the multisig host driven by a script, the way the
//! person's presses drive it — each step one machine event (the same JSON
//! the web's `MultisigAccountsCore.dispatch` takes), a wait on a view, a
//! capture of a value for later steps, a page action, a screenshot marker.
//! It is how the live proof runs the REAL app (its passkeys in the parallel
//! space, its HTTP, its RPC pool, its relay, its PeerConnect) against the
//! fork harness with no UI driver: nothing here decides — every step is a
//! press the person could make.
//!
//! Steps (a JSON array):
//!
//! ```text
//! {"accounts": <event>} | {"proposals": <event>} | {"members": <event>} | {"session": <event>}
//! {"until": {"machine": "proposals", "pointer": "/actions/propose/kind", "equals": "done"},
//!  "timeout_ms": 60000}               (or "exists": true)
//! {"capture": {"name": "hash", "machine": "proposals", "pointer": "/actions/propose/safe_tx_hash"}}
//! {"wait_ms": 2000}
//! {"page": "open_sheet", "chain_id": 100, "safe_tx_hash": "${hash}"} | {"page": "close_sheet"}
//!   | {"page": "send", "to": "0x…", "amount": "0.1"}           (the send flow, as the person opens it)
//!   | {"page": "space", "to": "testnet"|"mainnet"}             (the network space switched)
//!   | {"page": "section", "to": "wallet"|"explore"|…}          (a sidebar section pressed)
//!   | {"page": "browse", "url": "http://…"}                    (a site in a new browser tab)
//!   | {"page": "tabs"}                                         (the browser's tabs, logged)
//!   | {"page": "consent"}                                      (the open connection request's Connect)
//!   | {"page": "space_notes"}                                  (the sheets' space-switch notes, logged)
//!   | {"page": "notify", "on": true}                           (Settings' multisig-notifications switch)
//! {"answer_risk": true}
//! {"shot": "name"}                    (writes `SHOT name`, then waits 1.5 s)
//! {"log": "text"}
//! ```
//!
//! `${name}` in any string is replaced by a captured value. Each step and
//! every view fact it waited for is written to `VELA_MULTISIG_SCRIPT_LOG`
//! (else stderr), one line each: the evidence of a run.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::time::{Duration, Instant};

use gpui::{App, AsyncApp, Entity};
use serde_json::Value;

use super::host::{self, MultisigHost};

/// A page action the script asks of the wallet page (it reads it on its
/// next frame).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PageAsk(pub Option<Value>);

impl gpui::Global for PageAsk {}

fn log_line(line: &str) {
    let stamped = format!("[{:.3}] {line}", crate::executor::now_ms() / 1000.0);
    match std::env::var("VELA_MULTISIG_SCRIPT_LOG") {
        Ok(path) => {
            if let Ok(mut file) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
            {
                let _ = writeln!(file, "{stamped}");
            }
        }
        Err(_) => eprintln!("[multisig-script] {stamped}"),
    }
}

/// A line in the script's log from the page (what a page step came to).
pub fn note(line: &str) {
    log_line(line);
}

fn substitute(value: &Value, captured: &BTreeMap<String, Value>) -> Value {
    match value {
        Value::String(text) => {
            // A whole-string capture keeps its type (a number stays a number).
            if let Some(v) = text
                .strip_prefix("${")
                .and_then(|t| t.strip_suffix('}'))
                .and_then(|name| captured.get(name))
            {
                return v.clone();
            }
            let mut out = text.clone();
            for (name, v) in captured {
                let raw = v.as_str().map_or_else(|| v.to_string(), str::to_owned);
                out = out.replace(&format!("${{{name}}}"), &raw);
            }
            Value::String(out)
        }
        Value::Array(items) => {
            Value::Array(items.iter().map(|v| substitute(v, captured)).collect())
        }
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, captured)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// The view a step reads, as JSON.
fn view_of(host: &MultisigHost, machine: &str) -> Value {
    let m = host.machines();
    match machine {
        "accounts" => serde_json::to_value(&m.accounts_view).unwrap_or(Value::Null),
        "members" => serde_json::to_value(&m.members_view).unwrap_or(Value::Null),
        "session" => m
            .session_view()
            .and_then(|(v, _)| serde_json::to_value(v).ok())
            .unwrap_or(Value::Null),
        _ => serde_json::to_value(&m.proposals_view).unwrap_or(Value::Null),
    }
}

/// A view value by JSON pointer, case-folding the last key when the view
/// keys by a lowercase address or hash.
fn pointed(view: &Value, pointer: &str) -> Option<Value> {
    view.pointer(pointer)
        .or_else(|| view.pointer(&pointer.to_ascii_lowercase()))
        .cloned()
}

fn dispatch(
    host: &Entity<MultisigHost>,
    machine: &str,
    event: Value,
    cx: &mut App,
) -> Result<(), String> {
    host.update(cx, |host, cx| -> Result<(), String> {
        match machine {
            "accounts" => host.accounts(
                serde_json::from_value(event).map_err(|e| e.to_string())?,
                cx,
            ),
            "proposals" => host.proposals(
                serde_json::from_value(event).map_err(|e| e.to_string())?,
                cx,
            ),
            "members" => host.members(
                serde_json::from_value(event).map_err(|e| e.to_string())?,
                cx,
            ),
            "session" => host.session(
                serde_json::from_value(event).map_err(|e| e.to_string())?,
                cx,
            ),
            other => return Err(format!("no machine {other}")),
        }
        Ok(())
    })
}

/// Start the script, if one is named (dev-fixtures builds).
pub fn start(cx: &mut App) {
    let Some(path) = crate::dev_env::var!("VELA_MULTISIG_SCRIPT") else {
        return;
    };
    // Once a process: the wallet page is rebuilt when the account shown
    // changes (a multisig activated), and the script runs on across it.
    static STARTED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if STARTED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let steps: Vec<Value> = match std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|e| e.to_string()))
    {
        Ok(steps) => steps,
        Err(why) => {
            log_line(&format!("FAIL script unreadable: {why}"));
            return;
        }
    };
    log_line(&format!("script {path}: {} steps", steps.len()));
    cx.spawn(async move |cx: &mut AsyncApp| {
        let mut captured: BTreeMap<String, Value> = BTreeMap::new();
        let host = cx.update(host::host);
        for (i, raw) in steps.iter().enumerate() {
            let step = substitute(raw, &captured);
            let ok = run_step(i, &step, &host, &mut captured, cx).await;
            if !ok {
                log_line(&format!("FAIL step {i}: {step}"));
                return;
            }
        }
        log_line("DONE");
    })
    .detach();
}

async fn run_step(
    i: usize,
    step: &Value,
    host: &Entity<MultisigHost>,
    captured: &mut BTreeMap<String, Value>,
    cx: &mut AsyncApp,
) -> bool {
    for machine in ["accounts", "proposals", "members", "session"] {
        if let Some(event) = step.get(machine) {
            log_line(&format!("step {i} {machine} {event}"));
            let event = event.clone();
            let done = cx.update(|cx| dispatch(host, machine, event, cx));
            return match done {
                Ok(()) => true,
                Err(why) => {
                    log_line(&format!("step {i} refused: {why}"));
                    false
                }
            };
        }
    }
    if let Some(ms) = step.get("wait_ms").and_then(Value::as_u64) {
        cx.background_executor()
            .timer(Duration::from_millis(ms))
            .await;
        return true;
    }
    if let Some(until) = step.get("until") {
        let machine = until
            .get("machine")
            .and_then(Value::as_str)
            .unwrap_or("proposals")
            .to_owned();
        let pointer = until
            .get("pointer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let wanted = until.get("equals").cloned();
        let timeout = Duration::from_millis(
            step.get("timeout_ms")
                .and_then(Value::as_u64)
                .unwrap_or(60_000),
        );
        let started = Instant::now();
        loop {
            let seen = cx.update(|cx| pointed(&view_of(host.read(cx), &machine), &pointer));
            let met = match (&wanted, &seen) {
                (Some(w), Some(s)) => w == s,
                (None, Some(s)) => !s.is_null(),
                _ => false,
            };
            if met {
                log_line(&format!(
                    "step {i} until {machine}{pointer} = {}",
                    seen.unwrap_or(Value::Null)
                ));
                return true;
            }
            if started.elapsed() > timeout {
                log_line(&format!(
                    "step {i} until {machine}{pointer} timed out; last {}",
                    seen.unwrap_or(Value::Null)
                ));
                return false;
            }
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
        }
    }
    if let Some(capture) = step.get("capture") {
        let name = capture
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("value")
            .to_owned();
        let machine = capture
            .get("machine")
            .and_then(Value::as_str)
            .unwrap_or("proposals")
            .to_owned();
        let pointer = capture
            .get("pointer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let seen = cx.update(|cx| pointed(&view_of(host.read(cx), &machine), &pointer));
        return match seen {
            Some(v) if !v.is_null() => {
                log_line(&format!("step {i} capture {name} = {v}"));
                captured.insert(name, v);
                true
            }
            _ => {
                log_line(&format!(
                    "step {i} capture {name}: nothing at {machine}{pointer}"
                ));
                false
            }
        };
    }
    if step.get("log_connect").is_some() {
        // The connect link the session's sheet shows, for the scripted
        // outside owner to open.
        let link = cx.update(|cx| {
            let host = host.read(cx);
            let (view, _) = host.machines().session_view()?;
            let tx = view
                .tx
                .as_ref()
                .map(|t| vela_core::multisig_link::ProposalLink {
                    chain_id: t.chain_id,
                    multisig: t.safe.clone(),
                    safe_tx_hash: t.safe_tx_hash.clone(),
                });
            view.invite.as_ref()?;
            Some(vela_core::multisig_link::connect_link(
                &format!("{}/en/connect", vela_core::multisig_link::SITE_ORIGIN),
                view.invite.as_deref(),
                &view.members,
                tx.as_ref(),
            ))
        });
        return match link {
            Some(link) => {
                log_line(&format!("CONNECT {link}"));
                true
            }
            None => {
                log_line(&format!("step {i} log_connect: no session room yet"));
                false
            }
        };
    }
    if let Some(dump) = step.get("dump") {
        let machine = dump
            .get("machine")
            .and_then(Value::as_str)
            .unwrap_or("proposals")
            .to_owned();
        let pointer = dump
            .get("pointer")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_owned();
        let seen = cx.update(|cx| pointed(&view_of(host.read(cx), &machine), &pointer));
        log_line(&format!(
            "step {i} dump {machine}{pointer} = {}",
            seen.unwrap_or(Value::Null)
        ));
        return true;
    }
    if step.get("page").is_some() {
        log_line(&format!("step {i} page {step}"));
        let ask = step.clone();
        cx.update(|cx| {
            cx.set_global(PageAsk(Some(ask)));
            cx.refresh_windows();
        });
        cx.background_executor()
            .timer(Duration::from_millis(500))
            .await;
        return true;
    }
    if let Some(ask) = step.get("propose_send") {
        // The send flow's confirm, exactly as `money.rs` hands it over.
        log_line(&format!("step {i} propose_send {ask}"));
        let chain_id = ask.get("chain_id").and_then(Value::as_u64).unwrap_or(100);
        let multisig = ask
            .get("multisig")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let transfers: Vec<vela_core::multisig::Transfer> = ask
            .get("transfers")
            .cloned()
            .and_then(|t| serde_json::from_value(t).ok())
            .unwrap_or_default();
        let answer = cx.update(|cx| {
            host.update(cx, |host, cx| {
                host.propose_for_send(chain_id, &multisig, transfers, cx)
            })
        });
        cx.background_executor()
            .spawn(async move {
                let end = answer.await;
                log_line(&format!("propose_send answered {end:?}"));
            })
            .detach();
        return true;
    }
    if let Some(address) = step.get("show_members").and_then(Value::as_str) {
        // Settings showing the multisig: its members read per network.
        log_line(&format!("step {i} show_members {address}"));
        let address = address.to_owned();
        cx.update(|cx| host.update(cx, |host, cx| host.show_members(&address, cx)));
        return true;
    }
    if let Some(ask) = step.get("propose") {
        // A Settings proposal (a member change, a rename), as its screen
        // hands it over: the review first, then the person's word.
        log_line(&format!("step {i} propose {ask}"));
        let chain_id = ask.get("chain_id").and_then(Value::as_u64).unwrap_or(100);
        let multisig = ask
            .get("multisig")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let Some(proposal) = ask
            .get("proposal")
            .cloned()
            .and_then(|p| serde_json::from_value(p).ok())
        else {
            log_line(&format!("step {i} propose: not a proposal"));
            return false;
        };
        let answer = cx.update(|cx| {
            host.update(cx, |host, cx| {
                host.propose(chain_id, &multisig, proposal, cx)
            })
        });
        cx.background_executor()
            .spawn(async move {
                let end = answer.await;
                log_line(&format!("propose answered {end:?}"));
            })
            .detach();
        return true;
    }
    if let Some(signs) = step.get("answer_risk").and_then(Value::as_bool) {
        log_line(&format!("step {i} answer_risk {signs}"));
        cx.update(|cx| host.update(cx, |host, cx| host.answer_risk(signs, cx)));
        return true;
    }
    if let Some(name) = step.get("shot").and_then(Value::as_str) {
        cx.update(|cx| cx.refresh_windows());
        cx.background_executor()
            .timer(Duration::from_millis(700))
            .await;
        log_line(&format!("SHOT {name}"));
        cx.background_executor()
            .timer(Duration::from_millis(1500))
            .await;
        return true;
    }
    if let Some(text) = step.get("log") {
        log_line(&format!("LOG {text}"));
        return true;
    }
    log_line(&format!("step {i}: not a step"));
    false
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn captured_values_fill_later_steps() {
        let captured = BTreeMap::from([
            ("hash".to_owned(), json!("0xabc")),
            ("chain".to_owned(), json!(100)),
        ]);
        let step = json!({"proposals": {"type": "approve", "chain_id": "${chain}", "safe_tx_hash": "${hash}", "owner": "x-${hash}"}});
        let out = substitute(&step, &captured);
        assert_eq!(
            out["proposals"]["chain_id"],
            json!(100),
            "a whole capture keeps its type"
        );
        assert_eq!(out["proposals"]["safe_tx_hash"], json!("0xabc"));
        assert_eq!(out["proposals"]["owner"], json!("x-0xabc"));
    }

    #[test]
    fn a_pointer_reads_a_view_by_its_lowercase_key_too() {
        let view = json!({"actions": {"approve:0xab": {"kind": "done"}}});
        assert_eq!(
            pointed(&view, "/actions/approve:0xAB/kind"),
            Some(json!("done"))
        );
        assert_eq!(pointed(&view, "/actions/none"), None);
    }
}
