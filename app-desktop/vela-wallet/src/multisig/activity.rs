//! An executed multisig transaction in Activity (the web's `records.ts`):
//! which rows it writes is the core's plan (`execution_rows`, on the
//! proposals machine's `activity_rows` notice — one per transfer, else one
//! for the proposal itself); here they are only worded — a token's symbol
//! and decimals, the proposal's title — and shaped as the transaction
//! history's rows (`LocalTransaction`, camelCase).

use serde_json::{Value, json};
use vela_core::multisig::Action;

use super::words::{self, TokenInfo};
use crate::loc::Loc;

/// The rows `executed` (the notice's values) write, worded.
#[must_use]
pub fn worded_rows(executed: &[Value]) -> Vec<Value> {
    let loc = Loc::from_env();
    executed
        .iter()
        .flat_map(|e| worded(e, &loc, &|chain, token| words::token_info(chain, token)))
        .collect()
}

/// One executed transaction's rows, worded with `token` (a reader).
pub fn worded(
    e: &Value,
    loc: &Loc,
    token: &dyn Fn(u64, Option<&str>) -> TokenInfo,
) -> Vec<Value> {
    let Some(planned) = e.get("rows").and_then(Value::as_array) else {
        // A row already a transaction's is kept as it is.
        return vec![e.clone()];
    };
    let text = |k: &str| e.get(k).and_then(Value::as_str).unwrap_or_default().to_owned();
    let chain_id = e.get("chainId").and_then(Value::as_u64).unwrap_or_default();
    let safe_tx_hash = text("safeTxHash");
    let action: Option<Action> = e
        .get("action")
        .cloned()
        .and_then(|a| serde_json::from_value(a).ok());
    let nonce = text("nonce");
    let title = || {
        words::action_title(
            loc,
            action.as_ref(),
            &nonce,
            &|a: &str| words::short(a),
            &|t: Option<&str>| token(chain_id, t),
        )
    };
    let mut out = Vec::new();
    for row in planned {
        let index = row.get("index").and_then(Value::as_u64).unwrap_or_default();
        let to = row.get("to").and_then(Value::as_str).unwrap_or_default();
        let mut base = json!({
            "id": vela_core::multisig_link::executed_row_id(chain_id, &safe_tx_hash, index),
            "userOpHash": text("userOpHash"),
            "txHash": text("txHash"),
            "from": text("from"),
            "to": to,
            "chainId": chain_id,
            "timestamp": e.get("timestamp").cloned().unwrap_or(json!(0)),
            "status": "confirmed",
        });
        let plain = |amount: &str, decimals: u32| {
            vela_core::multisig_link::plain_units(amount, decimals)
                .unwrap_or_else(|_| "0".to_owned())
        };
        match row.get("kind").and_then(Value::as_str) {
            Some("send") => {
                let token_address = row.get("token").and_then(Value::as_str);
                let info = token(chain_id, token_address);
                let amount = row.get("amount").and_then(Value::as_str).unwrap_or("0");
                // r8-2: a token that never said its decimals keeps its raw
                // integer (0 places), never a friendly amount in 18.
                let decimals = info.decimals.unwrap_or(0);
                base["value"] = json!(plain(amount, decimals));
                base["symbol"] = json!(info.symbol);
                base["decimals"] = json!(decimals);
                base["type"] = json!("send");
                if let Some(address) = token_address {
                    base["token"] = json!(address);
                }
            }
            _ => {
                let sends = row.get("sends").and_then(Value::as_str).unwrap_or("0");
                let moves = vela_core::app::multisig_shell::quantity(sends).is_some_and(|v| v > 0);
                let info = if moves {
                    token(chain_id, None)
                } else {
                    TokenInfo::coin(String::new())
                };
                base["value"] = json!(if moves {
                    plain(sends, info.decimals.unwrap_or(18))
                } else {
                    "0".to_owned()
                });
                base["symbol"] = json!(info.symbol);
                base["decimals"] = json!(info.decimals.unwrap_or(18));
                base["type"] = json!("dapp_tx");
                base["intent"] = json!(title());
                base["dappSummary"] = json!({
                    "action": "call",
                    "calls": row.get("calls").cloned().unwrap_or(json!(1)),
                    "contract": row.get("contract").cloned().unwrap_or(Value::Null),
                });
            }
        }
        out.push(base);
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn coin(_: u64, token: Option<&str>) -> TokenInfo {
        match token {
            None => TokenInfo::coin("xDAI".to_owned()),
            Some(_) => TokenInfo {
                symbol: "USDC".to_owned(),
                decimals: Some(6),
            },
        }
    }

    fn executed(rows: Value, action: Value) -> Value {
        json!({
            "id": "multisig:100:0xaa:0", "userOpHash": "0xop", "txHash": "0xtx",
            "from": "0xSafe", "chainId": 100, "timestamp": 1_793_000_000,
            "status": "confirmed", "safeTxHash": format!("0x{}", "aa".repeat(32)), "nonce": "3",
            "action": action, "rows": rows,
        })
    }

    #[test]
    fn a_transfer_is_one_send_row_in_its_tokens_units() {
        let loc = Loc::for_tag("en");
        let e = executed(
            json!([{"index": 0, "kind": "send", "to": "0xbb", "token": "0xusdc", "amount": "2500000"}]),
            json!({"kind": "transfer", "token": "0xusdc", "to": "0xbb", "amount": "2500000"}),
        );
        let rows = worded(&e, &loc, &coin);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row["type"], "send");
        assert_eq!(row["value"], "2.5");
        assert_eq!(row["symbol"], "USDC");
        assert_eq!(row["token"], "0xusdc");
        assert_eq!(
            row["id"],
            format!("multisig:100:0x{}:0", "aa".repeat(32)),
            "a tap opens the transaction"
        );
    }

    #[test]
    fn anything_else_is_one_row_titled_as_its_proposal() {
        let loc = Loc::for_tag("en");
        let e = executed(
            json!([{"index": 0, "kind": "call", "to": "0xSafe", "sends": "0", "calls": 1, "contract": "0xsafe"}]),
            json!({"kind": "changeThreshold", "threshold": "3"}),
        );
        let rows = worded(&e, &loc, &coin);
        assert_eq!(rows[0]["type"], "dapp_tx");
        assert_eq!(rows[0]["intent"], "Require 3 approvals");
        assert_eq!(rows[0]["value"], "0");
        assert_eq!(rows[0]["dappSummary"]["calls"], 1);
    }

    #[test]
    fn a_row_already_a_transactions_is_kept() {
        let loc = Loc::for_tag("en");
        let row = json!({"id": "x", "type": "send"});
        assert_eq!(worded(&row, &loc, &coin), vec![row]);
    }
}
