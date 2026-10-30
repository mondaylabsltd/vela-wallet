//! Which calls of a multisig transaction the review reads as the signing
//! sheet would (§3.6) — the same blocks a Vela account would see for the
//! same intent. The shell asks here (`calls.readable`, `calls.reviewed`)
//! and hands the calls to the clear-signing machines; it never picks them.

use serde::Serialize;
use serde_json::Value;

use super::{arg, to_json};
use crate::app::multisig_shell::quantity;
use crate::multisig::{Action, SafeTx};
use crate::CoreError;

/// One call as a site would have sent it from the multisig.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InnerCall {
    pub to: String,
    /// Base units, as the action carries it.
    pub value: String,
    pub data: String,
}

/// The calls the clear reading reads (`readableCalls`): a call, or every
/// call of a batch that is not transfers alone; `None` for anything else —
/// a transfer, a batch of transfers, a member change, a rename, a
/// cancellation are the multisig's own reading.
pub fn readable(action: Option<&Action>) -> Option<Vec<InnerCall>> {
    match action? {
        Action::Call { to, value, data } => Some(vec![InnerCall {
            to: to.clone(),
            value: value.clone(),
            data: data.clone(),
        }]),
        Action::Batch { calls }
            if !calls
                .iter()
                .all(|c| matches!(c.action, Action::Transfer { .. })) =>
        {
            Some(
                calls
                    .iter()
                    .map(|c| InnerCall {
                        to: c.to.clone(),
                        value: c.value.clone(),
                        data: c.data.clone(),
                    })
                    .collect(),
            )
        }
        _ => None,
    }
}

/// The calls the one review reads (`reviewedCalls`): every call and every
/// transfer, alone or in a batch, as a site would have sent it from the
/// multisig — a lone transfer is the SafeTx's own call (`tx`). A member
/// change, a rename or a cancellation is the multisig's own reading
/// (`None`); so is a transfer whose value does not read.
pub fn reviewed(action: Option<&Action>, tx: &SafeTx) -> Option<Vec<InnerCall>> {
    match action? {
        Action::Call { to, value, data } => Some(vec![InnerCall {
            to: to.clone(),
            value: value.clone(),
            data: data.clone(),
        }]),
        Action::Batch { calls } => Some(
            calls
                .iter()
                .map(|c| InnerCall {
                    to: c.to.clone(),
                    value: c.value.clone(),
                    data: c.data.clone(),
                })
                .collect(),
        ),
        Action::Transfer { .. } => {
            let value = quantity(if tx.value.is_empty() { "0" } else { &tx.value })?;
            Some(vec![InnerCall {
                to: tx.to.clone(),
                value: value.to_string(),
                data: if tx.data.is_empty() {
                    "0x".to_owned()
                } else {
                    tx.data.clone()
                },
            }])
        }
        _ => None,
    }
}

/// `calls.readable { action }`, `calls.reviewed { action, tx }`.
pub fn call(method: &str, args: &Value) -> Result<String, CoreError> {
    let action: Option<Action> = arg(args, "action")?;
    match method {
        "readable" => to_json(&readable(action.as_ref())),
        "reviewed" => {
            let tx: SafeTx = arg(args, "tx")?;
            to_json(&reviewed(action.as_ref(), &tx))
        }
        other => Err(CoreError::Multisig(format!(
            "unknown calls method: {other}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use serde_json::json;

    fn action(v: Value) -> Action {
        serde_json::from_value(v).unwrap()
    }

    const TO: &str = "0x00000000000000000000000000000000000000b0";

    fn tx() -> SafeTx {
        serde_json::from_value(json!({
            "to": TO, "value": "0x10", "data": "0x", "operation": 0,
            "safeTxGas": "0x0", "baseGas": "0x0", "gasPrice": "0x0",
            "gasToken": "0x0000000000000000000000000000000000000000",
            "refundReceiver": "0x0000000000000000000000000000000000000000",
            "nonce": "0x1"
        }))
        .unwrap()
    }

    #[test]
    fn a_call_and_a_mixed_batch_are_read_transfers_alone_are_not() {
        let call = action(json!({"kind": "call", "to": TO, "value": "0", "data": "0x095ea7b3"}));
        assert_eq!(readable(Some(&call)).unwrap().len(), 1);
        let transfer = json!({"kind": "transfer", "token": null, "to": TO, "amount": "1"});
        let leg = |a: Value| json!({"to": TO, "value": "0", "data": "0x", "action": a});
        let transfers = action(
            json!({"kind": "batch", "calls": [leg(transfer.clone()), leg(transfer.clone())]}),
        );
        assert_eq!(readable(Some(&transfers)), None);
        let call_leg = json!({"kind": "call", "to": TO, "value": "0", "data": "0x095ea7b3"});
        let mixed = action(json!({"kind": "batch", "calls": [leg(transfer), leg(call_leg)]}));
        assert_eq!(readable(Some(&mixed)).unwrap().len(), 2);
        assert_eq!(readable(None), None);
    }

    #[test]
    fn the_review_reads_a_lone_transfer_as_the_safe_tx_s_own_call() {
        let transfer = action(json!({"kind": "transfer", "token": null, "to": TO, "amount": "16"}));
        let calls = reviewed(Some(&transfer), &tx()).unwrap();
        assert_eq!(
            calls,
            vec![InnerCall {
                to: TO.to_owned(),
                value: "16".to_owned(),
                data: "0x".to_owned()
            }]
        );
        let rename = action(json!({"kind": "rename", "name": "Team"}));
        assert_eq!(reviewed(Some(&rename), &tx()), None);
    }
}
