//! Which network a multisig is likely used on, for the two places a person
//! picks one — the add sheet when nothing says (r4b-8, r4D-4), and the
//! create sheet’s first network (r4a-14): what a link named, then the
//! network the wallet is on now (its network filter, r5reg-19), then the
//! networks the multisigs here were last used on, then where this device
//! last transacted, then where the account shown holds funds, then the
//! wallet’s own order. Only an order to offer them in: the pick stays the
//! person’s, and nothing here is evidence of where a multisig is used (that
//! is `multisig::multisig_networks`).
//!
//! The shell gathers the hints (its balances, its records, its multisigs)
//! and asks here: `networks.likely`, `networks.firstDefault`,
//! `networks.lastUsed`, `networks.recent`, `networks.held` — or reads the
//! accounts machine's `used_networks` / `recent_networks`, which it keeps
//! from its own list and this device's history ([`recent_of_history`]).
//! `networks.name` is the core's catalog name for a network this device may
//! not have ([`name`]).

use serde::Deserialize;
use serde_json::Value;

use super::{arg, to_json};
use crate::CoreError;

/// What says which network is likely (each list in its own order).
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct Hints {
    /// Networks a link named, first first.
    pub named: Vec<u64>,
    /// r5reg-19: the network the wallet is on now (its network filter).
    pub here: Vec<u64>,
    /// Networks the multisigs on this device were last used on, latest first.
    pub used: Vec<u64>,
    /// r5reg-19: networks this device’s own transactions were on, newest first.
    pub recent: Vec<u64>,
    /// Networks the account shown holds funds on, most first.
    pub held: Vec<u64>,
}

/// Where a network stands in every list: the wallet’s own order.
#[cfg(feature = "crux")]
fn rank(chain_id: u64) -> u64 {
    u32::try_from(chain_id).map_or(u64::MAX, crate::app::network_admin::builtin_rank)
}

/// Without the app’s table, the order the networks were given in.
#[cfg(not(feature = "crux"))]
fn rank(_chain_id: u64) -> u64 {
    0
}

/// `networks`, the likely ones first (each hint in its own order), the rest
/// in the wallet’s.
pub fn likely(networks: &[u64], hints: &Hints) -> Vec<u64> {
    let ranked: Vec<u64> = [
        &hints.named,
        &hints.here,
        &hints.used,
        &hints.recent,
        &hints.held,
    ]
    .into_iter()
    .flatten()
    .copied()
    .collect();
    let at = |id: u64| ranked.iter().position(|r| *r == id).unwrap_or(ranked.len());
    let mut out = networks.to_vec();
    out.sort_by_key(|id| (at(*id), rank(*id)));
    out
}

/// The create sheet’s first network (r4a-14): the one the wallet is on now
/// (r5reg-19), else the one the multisigs here were last used on, else the
/// one this device last transacted on, else where the account shown holds
/// the most, among `networks` — never one picked for the person when
/// nothing says (it was Sepolia by accident): `None` then, and the person
/// picks.
pub fn first_default(networks: &[u64], hints: &Hints) -> Option<u64> {
    [&hints.here, &hints.used, &hints.recent, &hints.held]
        .into_iter()
        .flatten()
        .copied()
        .find(|id| networks.contains(id))
}

/// A multisig here, as the order reads it.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct UsedAccount {
    pub chain_ids: Vec<u64>,
    pub added_ms: f64,
}

/// The networks the multisigs here were first used on, the newest first.
pub fn last_used(accounts: &[UsedAccount]) -> Vec<u64> {
    let mut sorted: Vec<&UsedAccount> = accounts.iter().collect();
    sorted.sort_by(|a, b| b.added_ms.total_cmp(&a.added_ms));
    sorted
        .iter()
        .filter_map(|a| a.chain_ids.first().copied())
        .collect()
}

/// One of this device’s transactions, as the order reads it.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Record {
    pub chain_id: Value,
    #[serde(default)]
    pub timestamp: f64,
}

/// r5reg-19: the networks `records` (this device’s transaction history, any
/// of its accounts) were on, the newest first — where this device was used.
pub fn recent(records: &[Record]) -> Vec<u64> {
    let mut sorted: Vec<&Record> = records.iter().collect();
    sorted.sort_by(|a, b| b.timestamp.total_cmp(&a.timestamp));
    let mut out: Vec<u64> = Vec::new();
    for r in sorted {
        if let Some(id) = r.chain_id.as_u64() {
            if !out.contains(&id) {
                out.push(id);
            }
        }
    }
    out
}

/// r5reg-19: [`recent`] of the history as the store holds it
/// (`vela.transactionHistory`, a JSON list of records): what does not read
/// is no evidence, and a store that does not read says nothing.
pub fn recent_of_history(raw: Option<&str>) -> Vec<u64> {
    let rows: Vec<Value> = raw
        .and_then(|r| serde_json::from_str(r).ok())
        .unwrap_or_default();
    let records: Vec<Record> = rows
        .into_iter()
        .filter_map(|r| serde_json::from_value(r).ok())
        .collect();
    recent(&records)
}

/// The catalog's name for `chain_id` — a network Vela ships, whether or not
/// this device has it now (hidden, or never added): "On Base (chain 8453)"
/// for a link or an invite naming a network the device lacks. `None` for a
/// chain the catalog does not know; a shell may still ask the chain index.
#[cfg(feature = "crux")]
pub fn name(chain_id: u64) -> Option<&'static str> {
    u32::try_from(chain_id)
        .ok()
        .and_then(crate::app::network_admin::builtin_display_name)
}

/// Without the app's table, no name.
#[cfg(not(feature = "crux"))]
pub fn name(_chain_id: u64) -> Option<&'static str> {
    None
}

/// A token the account shown holds (the balance core’s view).
#[derive(Clone, Debug, Deserialize)]
pub struct Token {
    pub chain_id: u64,
    pub balance: String,
    #[serde(default)]
    pub price_usd: Option<f64>,
    #[serde(default)]
    pub spam: bool,
}

/// The networks `tokens` hold anything on, the most valuable first; a token
/// with no price counts after the priced ones (a testnet’s coins have none).
pub fn held(tokens: &[Token]) -> Vec<u64> {
    let mut worth: Vec<(u64, f64)> = Vec::new();
    for t in tokens {
        let amount = t.balance.trim().parse::<f64>().unwrap_or(f64::NAN);
        // A spam token’s balance is no one’s funds.
        if t.spam || amount.is_nan() || amount <= 0.0 {
            continue;
        }
        let value = amount * t.price_usd.unwrap_or(0.0);
        match worth.iter_mut().find(|(id, _)| *id == t.chain_id) {
            Some((_, w)) => *w += value,
            None => worth.push((t.chain_id, value)),
        }
    }
    worth.sort_by(|a, b| b.1.total_cmp(&a.1));
    worth.into_iter().map(|(id, _)| id).collect()
}

/// `networks.<name>`, its arguments one JSON object, its answer JSON.
pub fn call(name: &str, args: &Value) -> Result<String, CoreError> {
    let hints = || -> Result<Hints, CoreError> {
        Ok(arg::<Option<Hints>>(args, "hints")?.unwrap_or_default())
    };
    match name {
        "likely" => to_json(&likely(&arg::<Vec<u64>>(args, "networks")?, &hints()?)),
        "firstDefault" => to_json(&first_default(
            &arg::<Vec<u64>>(args, "networks")?,
            &hints()?,
        )),
        "lastUsed" => to_json(&last_used(&arg::<Vec<UsedAccount>>(args, "accounts")?)),
        "recent" => to_json(&recent(&arg::<Vec<Record>>(args, "records")?)),
        "held" => to_json(&held(&arg::<Vec<Token>>(args, "tokens")?)),
        "name" => to_json(&self::name(arg::<u64>(args, "chainId")?)),
        other => Err(CoreError::Multisig(format!(
            "unknown networks method: {other}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! `likely-networks.test.ts`, case for case (r4D-4, r4a-14, r5reg-19).
    use super::*;
    use serde_json::json;

    /// These cases’ networks, in the wallet’s order.
    fn all() -> Vec<u64> {
        let mut ids = vec![1, 100, 11_155_111, 5_042_002, 8453];
        ids.sort_by_key(|id| rank(*id));
        ids
    }

    fn hints(v: Value) -> Hints {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn what_the_link_named_first_then_used_then_funds_then_the_wallet_order() {
        let order = likely(
            &all(),
            &hints(json!({ "named": [8453], "used": [5_042_002], "held": [100] })),
        );
        assert_eq!(&order[..3], &[8453, 5_042_002, 100]);
        let rest: Vec<u64> = all()
            .into_iter()
            .filter(|id| ![8453, 5_042_002, 100].contains(id))
            .collect();
        assert_eq!(&order[3..], rest.as_slice(), "the rest in the wallet order");
    }

    #[test]
    fn the_multisigs_here_newest_first_each_by_its_first_network() {
        let accounts: Vec<UsedAccount> = serde_json::from_value(json!([
            { "chainIds": [100, 1], "addedMs": 1 },
            { "chainIds": [5_042_002], "addedMs": 5 },
            { "chainIds": [], "addedMs": 9 }
        ]))
        .unwrap();
        assert_eq!(last_used(&accounts), vec![5_042_002, 100]);
    }

    #[test]
    fn where_funds_are_the_most_valuable_first_spam_and_empties_aside() {
        let tokens: Vec<Token> = serde_json::from_value(json!([
            { "chain_id": 1, "balance": "0", "price_usd": 3000 },
            { "chain_id": 100, "balance": "2", "price_usd": 1 },
            { "chain_id": 8453, "balance": "1", "price_usd": 3000 },
            { "chain_id": 5_042_002, "balance": "9", "price_usd": null },
            { "chain_id": 11_155_111, "balance": "5", "price_usd": 1, "spam": true }
        ]))
        .unwrap();
        assert_eq!(held(&tokens), vec![8453, 100, 5_042_002]);
    }

    #[test]
    fn a_first_network_only_when_something_says_which() {
        let all = all();
        let first = |h: Value| first_default(&all, &hints(h));
        assert_eq!(first(json!({ "used": [], "held": [] })), None);
        assert_eq!(
            first(json!({ "used": [5_042_002], "held": [100] })),
            Some(5_042_002)
        );
        assert_eq!(first(json!({ "used": [999], "held": [100] })), Some(100));
        assert_eq!(
            first(json!({ "named": [8453] })),
            None,
            "a link network is offered first, never picked for a new one"
        );
        assert_eq!(
            first(json!({ "here": [5_042_002], "used": [100] })),
            Some(5_042_002)
        );
        assert_eq!(
            first(json!({ "recent": [5_042_002], "held": [100] })),
            Some(5_042_002)
        );
    }

    #[test]
    fn the_network_the_wallet_is_on_and_where_this_device_transacted_lead() {
        let order = |h: Value| likely(&all(), &hints(h));
        assert_eq!(order(json!({ "here": [5_042_002] }))[0], 5_042_002);
        assert_eq!(
            &order(json!({ "recent": [5_042_002], "held": [100] }))[..2],
            &[5_042_002, 100]
        );
        // Where the wallet is beats where its multisigs were; a link beats both.
        assert_eq!(
            &order(json!({ "named": [8453], "here": [5_042_002], "used": [100] }))[..3],
            &[8453, 5_042_002, 100]
        );
    }

    #[test]
    fn where_this_device_transacted_newest_first_each_network_once() {
        let records: Vec<Record> = serde_json::from_value(json!([
            { "chainId": 100, "timestamp": 1 },
            { "chainId": 5_042_002, "timestamp": 9 },
            { "chainId": 100, "timestamp": 5 },
            { "chainId": 8453, "timestamp": 3 }
        ]))
        .unwrap();
        assert_eq!(recent(&records), vec![5_042_002, 100, 8453]);
    }

    #[test]
    fn asked_through_the_door() {
        let ask = |method: &str, args: Value| {
            crate::multisig_rules::call_json(method, &args)
                .unwrap()
                .unwrap()
        };
        assert_eq!(
            ask(
                "networks.likely",
                json!({ "networks": [1, 100], "hints": { "held": [100] } })
            ),
            "[100,1]"
        );
        assert_eq!(
            ask("networks.firstDefault", json!({ "networks": [1, 100] })),
            "null"
        );
        // The catalog's name, for a network a device may not have.
        assert_eq!(ask("networks.name", json!({ "chainId": 8453 })), "\"Base\"");
        assert_eq!(ask("networks.name", json!({ "chainId": 999_999 })), "null");
    }

    /// The history as the store holds it: a row that does not read is no
    /// evidence, a store that does not read says nothing.
    #[test]
    fn recent_reads_the_stored_history() {
        let raw = json!([
            { "chainId": 100, "timestamp": 1, "hash": "0x1" },
            { "chainId": "8453", "timestamp": 9 },
            { "timestamp": 12 },
            { "chainId": 10, "timestamp": 5 }
        ])
        .to_string();
        assert_eq!(recent_of_history(Some(&raw)), vec![10, 100]);
        assert!(recent_of_history(Some("{oops")).is_empty());
        assert!(recent_of_history(None).is_empty());
    }
}
