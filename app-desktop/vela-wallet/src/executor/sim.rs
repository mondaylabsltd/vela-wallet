//! What a transaction would DO, asked of the chain itself.
//!
//! `eth_simulateV1` runs the inner calls in a simulated block against live
//! state through this person's own RPC pool — no third-party "simulation"
//! service — and the wallet's asset deltas are derived from the logs it
//! returns.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/sim/sim-engine-rpc.ts`
//! and `sim-assets.ts` (`rpcSimulate`, `deriveAssetDeltas`).
//!
//! ## Why this is the only part of a signing sheet a site cannot author
//!
//! Everything else on that sheet starts as something the dApp said: the
//! calldata, the intent, the token it names. This starts as what the CHAIN
//! says would happen if it ran. That is why the deeper degradation rungs
//! promote it from a footnote to the protagonist — and why it is worth having
//! even when it can only answer sometimes.
//!
//! ## What it must never do
//!
//! Decide. The deltas are untrusted input by construction (the core's
//! invariants ⑤/⑥): a site can emit any `Transfer` log it likes from a
//! contract it controls. Which of them may be shown with a confident amount is
//! `token_trust`'s judgment, and this module hands them over without an
//! opinion. It also must never reach a WRITE — the founder's own ruling, from
//! the token auto-add work: admission comes from confirm-time receipt logs,
//! never from a sign-time simulation.

use serde_json::{Value, json};

use vela_core::app::fee_policy::FeeCall;
use vela_core::app::token_trust::{TrustAssetDelta, TrustDeltaKind};

/// `Transfer(address,address,uint256)`.
const TRANSFER_TOPIC: &str = "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";
/// `traceTransfers` reports native value moves as synthetic `Transfer` logs
/// from this address, so ONE log parser covers native and ERC-20 both.
const NATIVE_SENTINEL: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

/// Simulate `calls` as `from`, and net what moves for `from`.
///
/// `None` means the question could not be asked — the endpoint does not
/// implement the method, the params were refused, every RPC was unreachable.
/// It is NOT "nothing moves": the sheet degrades to saying it could not check,
/// which is a different sentence and a different amount of trust.
#[must_use]
pub fn simulate(from: &str, calls: &[FeeCall], chain_id: u32) -> Option<Vec<TrustAssetDelta>> {
    let first = calls.first()?;
    if first.to.is_empty() {
        return None;
    }
    let payload = json!({
        "blockStateCalls": [{
            "calls": calls.iter().map(|call| {
                let mut entry = json!({
                    "from": from,
                    "to": call.to,
                    "value": hex_value(&call.value),
                });
                if !call.data.is_empty() && call.data != "0x" {
                    entry["data"] = json!(call.data);
                }
                entry
            }).collect::<Vec<_>>(),
        }],
        // "What would happen", not "may this account afford it": in the 4337
        // model the Safe pays no gas, so requiring funds and a nonce would
        // refuse simulations that are going to succeed.
        "validation": false,
        "traceTransfers": true,
        "returnFullTransactions": false,
    });

    let result =
        crate::executor::pool::call(chain_id, "eth_simulateV1", json!([payload, "latest"])).ok()?;
    // The pool hands back the WHOLE JSON-RPC envelope — `chain_id_of` and
    // every other caller unwrap `result` themselves, and a parser that forgot
    // to would read a perfectly good answer as "this node cannot simulate".
    // (It did, for one commit: the endpoint answered and the sheet degraded.)
    if result.get("error").is_some_and(|error| !error.is_null()) {
        return None;
    }
    let blocks = result.get("result").and_then(Value::as_array)?;
    if blocks.is_empty() {
        return None;
    }

    let mut logs: Vec<&Value> = Vec::new();
    for block in blocks {
        let Some(calls) = block.get("calls").and_then(Value::as_array) else {
            continue;
        };
        for call in calls {
            // A per-call revert is an ANSWER (the transaction would fail), not
            // a reason to degrade — but its logs did not happen, so they are
            // not netted.
            if !succeeded(call) {
                continue;
            }
            if let Some(entries) = call.get("logs").and_then(Value::as_array) {
                logs.extend(entries);
            }
        }
    }
    Some(derive_deltas(&logs, from))
}

/// A per-call result counts as success only when it says so — some nodes omit
/// `status`, and there the absence of an error is the best signal available.
fn succeeded(call: &Value) -> bool {
    match call.get("status") {
        Some(Value::String(s)) => s == "0x1",
        Some(Value::Number(n)) => n.as_u64() == Some(1),
        _ => call.get("error").is_none_or(Value::is_null),
    }
}

/// Net every `Transfer` that touches `user` into one signed delta per asset.
fn derive_deltas(logs: &[&Value], user: &str) -> Vec<TrustAssetDelta> {
    let user = user.to_lowercase();
    // Insertion order, so the sheet lists assets the way the chain reported
    // them rather than in a hash's order.
    let mut order: Vec<String> = Vec::new();
    let mut totals: std::collections::HashMap<String, i128> = std::collections::HashMap::new();

    for log in logs {
        let Some(topics) = log.get("topics").and_then(Value::as_array) else {
            continue;
        };
        if topics.len() != 3 {
            continue;
        }
        if !topics[0]
            .as_str()
            .is_some_and(|topic| topic.eq_ignore_ascii_case(TRANSFER_TOPIC))
        {
            continue;
        }
        let from = topic_address(&topics[1]);
        let to = topic_address(&topics[2]);
        if from != user && to != user {
            continue;
        }
        let Some(value) = first_word(log.get("data").and_then(Value::as_str).unwrap_or("")) else {
            continue;
        };
        if value == 0 {
            continue;
        }
        let address = log
            .get("address")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_lowercase();
        if address.is_empty() {
            continue;
        }
        let key = if address == NATIVE_SENTINEL {
            "native".to_owned()
        } else {
            address
        };
        if !totals.contains_key(&key) {
            order.push(key.clone());
        }
        let entry = totals.entry(key).or_insert(0);
        // A self-transfer (from == to == user) nets to zero, which is the
        // truth about it.
        if to == user {
            *entry = entry.saturating_add(value);
        }
        if from == user {
            *entry = entry.saturating_sub(value);
        }
    }

    order
        .into_iter()
        .filter_map(|key| {
            let delta = totals.get(&key).copied().unwrap_or(0);
            if delta == 0 {
                return None;
            }
            Some(TrustAssetDelta {
                kind: if key == "native" {
                    TrustDeltaKind::Native
                } else {
                    TrustDeltaKind::Erc20
                },
                token: (key != "native").then_some(key),
                delta: delta.to_string(),
            })
        })
        .collect()
}

/// The last 20 bytes of a 32-byte topic, lowercased.
fn topic_address(topic: &Value) -> String {
    let raw = topic.as_str().unwrap_or_default();
    let hex = raw.strip_prefix("0x").unwrap_or(raw);
    if hex.len() < 40 {
        return String::new();
    }
    format!("0x{}", &hex[hex.len() - 40..]).to_lowercase()
}

/// The first 32-byte word of a log's data, as an integer.
///
/// Saturating at `i128::MAX` rather than refusing: a value that large is not a
/// real balance change, and the judgment above cares about the direction and
/// the digits, not about arithmetic on absurd numbers.
fn first_word(data: &str) -> Option<i128> {
    let hex = data.strip_prefix("0x").unwrap_or(data);
    if hex.len() < 64 {
        return None;
    }
    let word = &hex[..64];
    let trimmed = word.trim_start_matches('0');
    if trimmed.is_empty() {
        return Some(0);
    }
    if trimmed.len() > 31 {
        return Some(i128::MAX);
    }
    i128::from_str_radix(trimmed, 16).ok()
}

/// `value` as the hex quantity `eth_simulateV1` wants. The wire carries wei as
/// a DECIMAL string; sending it unconverted is how a 1 becomes a 0x1 that is
/// really one wei and a 10 becomes sixteen.
fn hex_value(decimal: &str) -> String {
    let trimmed = decimal.trim();
    if trimmed.is_empty() {
        return "0x0".to_owned();
    }
    trimmed
        .parse::<u128>()
        .map_or_else(|_| "0x0".to_owned(), |value| format!("{value:#x}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
    const TOKEN: &str = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83";

    fn topic(address: &str) -> String {
        format!("0x{:0>64}", address.trim_start_matches("0x"))
    }

    fn log(address: &str, from: &str, to: &str, value: u128) -> Value {
        json!({
            "address": address,
            "topics": [TRANSFER_TOPIC, topic(from), topic(to)],
            "data": format!("0x{value:064x}"),
        })
    }

    /// A swap: one token out, another in, netted per asset and signed.
    #[test]
    fn a_swap_nets_to_one_line_per_asset() {
        let other = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
        let logs = vec![
            log(TOKEN, ME, "0xdex", 8_450_000_000),
            log(other, "0xdex", ME, 2_100_000),
        ];
        let refs: Vec<&Value> = logs.iter().collect();
        let deltas = derive_deltas(&refs, ME);

        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0].token.as_deref(), Some(TOKEN));
        assert_eq!(deltas[0].delta, "-8450000000", "what went out is negative");
        assert_eq!(deltas[1].token.as_deref(), Some(other));
        assert_eq!(deltas[1].delta, "2100000");
    }

    /// Native moves arrive as synthetic transfers from the sentinel, so one
    /// parser covers both kinds.
    #[test]
    fn a_native_move_is_the_same_parser() {
        let logs = vec![log(NATIVE_SENTINEL, ME, "0xdead", 10_000_000_000_000_000)];
        let refs: Vec<&Value> = logs.iter().collect();
        let deltas = derive_deltas(&refs, ME);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].kind, TrustDeltaKind::Native);
        assert!(deltas[0].token.is_none());
        assert_eq!(deltas[0].delta, "-10000000000000000");
    }

    /// Transfers between other people are not this wallet's business, and a
    /// self-transfer nets to nothing — reporting either would be a line about
    /// money that did not move.
    #[test]
    fn other_peoples_transfers_and_self_transfers_produce_nothing() {
        let logs = vec![log(TOKEN, "0xaaa", "0xbbb", 500), log(TOKEN, ME, ME, 900)];
        let refs: Vec<&Value> = logs.iter().collect();
        assert!(derive_deltas(&refs, ME).is_empty());
    }

    /// A reverted call's logs did not happen.
    #[test]
    fn a_reverted_call_contributes_nothing() {
        assert!(!succeeded(&json!({"status": "0x0"})));
        assert!(succeeded(&json!({"status": "0x1"})));
        assert!(succeeded(&json!({"status": 1})));
        // Nodes that omit status: absence of an error is the best signal.
        assert!(succeeded(&json!({})));
        assert!(!succeeded(&json!({"error": {"message": "reverted"}})));
    }

    /// The real thing, against Gnosis: what would a 0.001 xDAI transfer from
    /// the golden Safe move?
    ///
    /// `#[ignore]` because it needs the network — the same rule the balance
    /// tests follow. Run it with
    /// `cargo test executor::sim::tests::live -- --ignored --nocapture`.
    #[test]
    #[ignore = "reads a real chain"]
    fn live_a_native_transfer_simulates_to_its_own_delta() {
        crate::executor::storage::tests::with_temp_state("sim-live", || {
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let calls = vec![FeeCall {
                to: "0x000000000000000000000000000000000000dEaD".to_owned(),
                value: "1000000000000000".to_owned(),
                data: "0x".to_owned(),
            }];
            match simulate(GOLDEN, &calls, 100) {
                Some(deltas) => {
                    println!("deltas: {deltas:?}");
                    let native = deltas
                        .iter()
                        .find(|delta| delta.kind == TrustDeltaKind::Native)
                        .unwrap_or_else(|| unreachable!("the coin moved"));
                    assert_eq!(native.delta, "-1000000000000000");
                }
                None => println!("the endpoint does not implement eth_simulateV1 — degraded"),
            }
        });
    }

    /// Wei crosses the wire as a decimal string and `eth_simulateV1` wants a
    /// hex quantity. Sending it unconverted turns 10 wei into sixteen.
    #[test]
    fn the_value_is_converted_to_a_hex_quantity() {
        assert_eq!(hex_value("0"), "0x0");
        assert_eq!(hex_value("10"), "0xa");
        assert_eq!(hex_value("1000000000000000000"), "0xde0b6b3a7640000");
        assert_eq!(hex_value(""), "0x0");
        assert_eq!(hex_value("not a number"), "0x0");
    }
}
