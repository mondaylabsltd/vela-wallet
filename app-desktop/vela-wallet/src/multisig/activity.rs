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

/// r12-8: a member change's title by the core's one rule
/// (`names::change_title`): each member as `names.who` with the machine's
/// labels, in `loc`'s language — the sheet's words, on Activity too, worded
/// when drawn, never frozen as written. `None`: not a member change.
#[must_use]
pub fn member_title(
    loc: &Loc,
    action: &Action,
    labels: &std::collections::BTreeMap<String, String>,
) -> Option<String> {
    let title = vela_core::multisig_rules::names::change_title(action, labels)?;
    let said = match title.count.as_deref().and_then(|c| c.parse::<usize>().ok()) {
        Some(count) => loc.t_count(title.key, count).to_string(),
        None => {
            let vars: Vec<(&str, &str)> =
                title.params.iter().map(|(k, v)| (*k, v.as_str())).collect();
            loc.t_texts(title.key, &vars).to_string()
        }
    };
    (said != title.key).then_some(said)
}

/// r12-8: every stored executed member change (a `multisig:` row with its
/// `multisigChange`) titled again in `loc`'s words with `labels` — the web's
/// `rewordMemberRows`. A row is rewritten only when its title moved; rows
/// written before r12 carry no action and keep theirs. Answers how many.
pub fn reword_member_rows(
    rows: &mut [Value],
    loc: &Loc,
    labels: &std::collections::BTreeMap<String, String>,
) -> usize {
    let mut changed = 0;
    for row in rows.iter_mut() {
        let ours = row
            .get("id")
            .and_then(Value::as_str)
            .is_some_and(|id| id.starts_with("multisig:"));
        let Some(action) = row
            .get("multisigChange")
            .cloned()
            .and_then(|a| serde_json::from_value::<Action>(a).ok())
            .filter(|_| ours)
        else {
            continue;
        };
        let Some(title) = member_title(loc, &action, labels) else {
            continue;
        };
        if row.get("intent").and_then(Value::as_str) != Some(title.as_str()) {
            row["intent"] = json!(title);
            changed += 1;
        }
    }
    changed
}

fn is_member_change(action: &Action) -> bool {
    matches!(
        action,
        Action::AddOwner { .. }
            | Action::RemoveOwner { .. }
            | Action::SwapOwner { .. }
            | Action::ChangeThreshold { .. }
    )
}

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
pub fn worded(e: &Value, loc: &Loc, token: &dyn Fn(u64, Option<&str>) -> TokenInfo) -> Vec<Value> {
    let Some(planned) = e.get("rows").and_then(Value::as_array) else {
        // A row already a transaction's is kept as it is.
        return vec![e.clone()];
    };
    let text = |k: &str| {
        e.get(k)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let chain_id = e.get("chainId").and_then(Value::as_u64).unwrap_or_default();
    let safe_tx_hash = text("safeTxHash");
    let action: Option<Action> = e
        .get("action")
        .cloned()
        .and_then(|a| serde_json::from_value(a).ok());
    let nonce = text("nonce");
    let labels = crate::multisig::shown::labels();
    let member = action
        .as_ref()
        .filter(|a| is_member_change(a))
        .and_then(|a| member_title(loc, a, &labels));
    let title = || {
        if let Some(member) = &member {
            return member.clone();
        }
        words::action_title(
            loc,
            action.as_ref(),
            &nonce,
            // r11-6: a member named as every title names one (the core's
            // `names::who`, with its one label) — never a bare short form
            // where the other surfaces say "Interleave (0x88cCA0…266894)".
            &|a: &str| words::who(a, crate::multisig::shown::label(a).as_deref()),
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
                // r12-8: a member change keeps its action, so its row is
                // titled again in the page's words and the names as they
                // stand (`reword_member_rows`).
                if member.is_some() {
                    base["multisigChange"] = e.get("action").cloned().unwrap_or(Value::Null);
                }
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

    /// r12-8: an executed member change is titled by the core's one rule
    /// in the page's language, keeps its action, and is titled again when a
    /// name lands — never "Add 0xD40086…de130b" frozen beside the sheet's
    /// "Add Dana Local (…)".
    #[test]
    fn a_member_change_row_is_titled_by_the_one_rule_and_reworded() {
        let dana = "0xD400866e00B055B20752a826CD5C89b811de130b";
        let action: Action =
            serde_json::from_value(json!({"kind": "addOwner", "owner": dana, "threshold": "2"}))
                .unwrap();
        let en = Loc::for_tag("en");
        let none = std::collections::BTreeMap::new();
        let bare = member_title(&en, &action, &none).unwrap();
        assert_eq!(bare, "Add 0xD40086…de130b");
        let named = std::collections::BTreeMap::from([(
            dana.to_ascii_lowercase(),
            "Dana Local".to_owned(),
        )]);
        assert_eq!(
            member_title(&en, &action, &named).unwrap(),
            "Add Dana Local (0xD40086…de130b)"
        );
        let zh = member_title(&Loc::for_tag("zh"), &action, &named).unwrap();
        assert!(!zh.starts_with("Add") && zh.contains("Dana Local"), "{zh}");
        let threshold: Action =
            serde_json::from_value(json!({"kind": "changeThreshold", "threshold": "1"})).unwrap();
        assert_eq!(
            member_title(&en, &threshold, &none).unwrap(),
            "Require 1 approval"
        );

        let rows = worded(
            &executed(
                json!([{"kind": "call", "index": 0, "to": "0xSafe", "sends": "0", "calls": 1}]),
                json!({"kind": "addOwner", "owner": dana, "threshold": "2"}),
            ),
            &en,
            &coin,
        );
        assert_eq!(rows[0]["multisigChange"]["kind"], "addOwner");
        let mut stored = vec![rows[0].clone(), json!({"id": "other", "intent": "Sent"})];
        stored[0]["intent"] = json!(bare);
        assert_eq!(reword_member_rows(&mut stored, &en, &named), 1);
        assert_eq!(stored[0]["intent"], "Add Dana Local (0xD40086…de130b)");
        assert_eq!(
            reword_member_rows(&mut stored, &en, &named),
            0,
            "only a title that moved"
        );
        assert_eq!(stored[1]["intent"], "Sent");
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
