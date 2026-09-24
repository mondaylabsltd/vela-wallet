//! What this device holds for Vela, row by row, and the three ways it is
//! given back: one row's Clear, "Clear all caches", and "Erase this device"
//! (spec 072).
//!
//! Which row a key belongs to, which keys are caches and which an erase
//! removes are the core's (`vela_core::storage_catalog`), written once for
//! every shell. This file does only what a shell has to: list the store,
//! measure it, delete, and — for the erase — look again afterwards, because
//! telling somebody their device is clean while their history is still on it
//! is the one outcome an erase cannot have.
//!
//! The desktop has ONE store — the JSON document behind [`storage`] — so the
//! scan is one listing. The dApp browser's own website data belongs to the
//! sites it loaded, not to Vela, and is not a `vela.` key.

use std::collections::HashMap;

use vela_core::storage_catalog::{
    StorageGroup, group_of_key, is_cache_key, is_erasable_key, is_ours, item_of_key, records_in,
};

use crate::executor::storage::{self, StorageError};

/// One row's share: its bytes, and the count its meta line prints (records,
/// contacts, items or sites — which one is the row's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub bytes: u64,
    pub count: usize,
}

/// The whole store, measured.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub total_bytes: u64,
    /// Every key of ours — the headline's "N records in total", as the web
    /// counts it.
    pub key_count: usize,
    user_bytes: u64,
    cache_bytes: u64,
    session_bytes: u64,
    items: HashMap<&'static str, Usage>,
}

impl Report {
    /// A drawn row's share, by the catalog's id. Zero for a row with nothing
    /// in it, which is a true statement rather than a missing one.
    #[must_use]
    pub fn item(&self, id: &str) -> Usage {
        self.items.get(id).copied().unwrap_or_default()
    }

    /// A group's bytes — the bar's three segments.
    #[must_use]
    pub fn group(&self, group: StorageGroup) -> u64 {
        match group {
            StorageGroup::User => self.user_bytes,
            StorageGroup::Cache => self.cache_bytes,
            StorageGroup::Sessions => self.session_bytes,
        }
    }
}

/// What a row counts in one of its keys.
fn count_for(id: &str, key: &str, raw: &str) -> usize {
    match id {
        // "N contacts" is people: the groups and the dismissed suggestions
        // live in the same row but are not contacts.
        "contacts" if key != storage::KEY_CONTACTS => 0,
        "transactions" | "contacts" | "custom" | "browsing" => records_in(raw).unwrap_or(1),
        // One grant per site; the request and chain keys are not sites.
        "dapps" => usize::from(key.starts_with("vela.perm.")),
        _ => 1,
    }
}

/// Measure a store's entries (`key → raw value`). Pure.
#[must_use]
pub fn measure_entries(entries: &[(String, String)]) -> Report {
    let mut report = Report::default();
    for (key, raw) in entries.iter().filter(|(key, _)| is_ours(key)) {
        let bytes = (key.len() + raw.len()) as u64;
        report.total_bytes += bytes;
        report.key_count += 1;
        match group_of_key(key) {
            StorageGroup::User => report.user_bytes += bytes,
            StorageGroup::Cache => report.cache_bytes += bytes,
            StorageGroup::Sessions => report.session_bytes += bytes,
        }
        if let Some(item) = item_of_key(key) {
            let usage = report.items.entry(item.id).or_default();
            usage.bytes += bytes;
            usage.count += count_for(item.id, key, raw);
        }
    }
    report
}

/// This device, measured now.
#[must_use]
pub fn measure() -> Report {
    measure_entries(&storage::raw_entries().unwrap_or_default())
}

fn keys_where(rule: impl Fn(&str) -> bool) -> Result<Vec<String>, StorageError> {
    Ok(storage::raw_entries()?
        .into_iter()
        .map(|(key, _)| key)
        .filter(|key| rule(key))
        .collect())
}

/// One row's Clear: every key the catalog files under it, and nothing else.
/// The dApp row is not cleared here — a grant is the browser machine's to
/// revoke, which also tells the open pages.
pub fn clear_item(id: &str) -> Result<Vec<String>, StorageError> {
    let doomed = keys_where(|key| item_of_key(key).is_some_and(|item| item.id == id))?;
    storage::remove_values(&doomed)?;
    Ok(doomed)
}

/// "Clear all caches": exactly the cache group's keys. Your data, your
/// preferences — `vela.balanceHidden` among them — and the sessions are
/// untouched by construction.
pub fn clear_caches() -> Result<Vec<String>, StorageError> {
    let doomed = keys_where(is_cache_key)?;
    storage::remove_values(&doomed)?;
    Ok(doomed)
}

/// The keys an erase would remove, out of a listing. Pure.
#[must_use]
pub fn erasable(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    keys.into_iter()
        .filter(|key| is_erasable_key(key))
        .collect()
}

/// The erase ran and something survived it.
///
/// A distinct type because the caller must NOT treat this as erased: the
/// person stays where they are and is told the erase did not finish.
#[derive(Debug, PartialEq, Eq)]
pub struct EraseIncomplete {
    /// The keys still there — never logged with their values. Empty when the
    /// store could not even be listed, which is not a clean store either.
    pub remaining: Vec<String>,
}

/// "Erase this device": delete every erasable key — found by scanning, never
/// by a list of what to delete — then look again.
///
/// `Ok` carries what was removed; `Err` what survived. The delete's own
/// failure is not the verdict, the second look is: a write that half-landed
/// is only visible there.
pub fn erase() -> Result<Vec<String>, EraseIncomplete> {
    let listed =
        || storage::raw_entries().map(|entries| erasable(entries.into_iter().map(|(key, _)| key)));
    let doomed = listed().map_err(|_| EraseIncomplete {
        remaining: Vec::new(),
    })?;
    if let Err(error) = storage::remove_values(&doomed) {
        eprintln!("[vela-wallet] erase: the delete failed ({error}); verifying");
    }
    match listed() {
        Ok(remaining) if remaining.is_empty() => Ok(doomed),
        Ok(remaining) => Err(EraseIncomplete { remaining }),
        Err(_) => Err(EraseIncomplete { remaining: doomed }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage::tests::with_temp_state;
    use serde_json::{Value, json};

    fn seed(pairs: &[(&str, Value)]) {
        for (key, value) in pairs {
            if storage::write_value(key, value.clone()).is_err() {
                unreachable!("could not seed {key}");
            }
        }
    }

    fn keys() -> Vec<String> {
        let mut keys: Vec<String> = storage::raw_entries()
            .unwrap_or_default()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        keys.sort();
        keys
    }

    /// Every key of ours goes — the accounts, the preferences, the grants, a
    /// key nobody has written yet — except the unconfirmed public keys, and a
    /// key that is not ours is not ours to judge. Then the second look finds
    /// nothing, which is what `Ok` means.
    #[test]
    fn erase_scans_deletes_keeps_the_outbox_and_verifies() {
        with_temp_state("erase-scan", || {
            seed(&[
                ("vela.accounts", json!([{ "id": "a" }])),
                ("vela.activeAccountIndex", json!(0)),
                ("vela.theme", json!("dark")),
                ("vela.perm.https://app.example", json!({ "address": "0x1" })),
                ("recipient_id:0xabc", json!("{}")),
                ("vela.some-key-added-next-year", json!(1)),
                ("vela.pendingUploads", json!([{ "id": "cred0" }])),
                ("theme_preference", json!("dark")),
            ]);
            let mut removed = erase().unwrap_or_else(|error| unreachable!("{error:?}"));
            removed.sort();
            assert_eq!(
                removed,
                [
                    "recipient_id:0xabc",
                    "vela.accounts",
                    "vela.activeAccountIndex",
                    "vela.perm.https://app.example",
                    "vela.some-key-added-next-year",
                    "vela.theme",
                ]
            );
            assert_eq!(keys(), ["theme_preference", "vela.pendingUploads"]);
            assert!(
                erasable(keys()).is_empty(),
                "nothing erasable survived the erase"
            );
        });
    }

    /// The keep-list and the scan, without a store.
    #[test]
    fn the_erase_rule_is_the_namespace_minus_the_keep_list() {
        let listed = [
            "vela.contacts",
            "vela.pendingUploads",
            "recipient_id:1",
            "unrelated",
        ]
        .map(str::to_owned);
        assert_eq!(erasable(listed), ["vela.contacts", "recipient_id:1"]);
    }

    /// "Clear all caches" clears caches: the hidden-balance switch is a
    /// preference and survives it, and so does everything that is your data.
    #[test]
    fn clearing_caches_keeps_the_hidden_balance_and_your_data() {
        with_temp_state("clear-caches", || {
            seed(&[
                ("vela.balanceCache", json!({ "0x1": [] })),
                ("vela.fiatRates.v1", json!([])),
                ("vela.tokenMeta.100:0xabc", json!({})),
                ("vela.balanceHidden", json!("1")),
                ("vela.contacts", json!([{ "address": "0x2" }])),
                ("vela.perm.https://app.example", json!({})),
            ]);
            let mut cleared = clear_caches().unwrap_or_else(|error| unreachable!("{error}"));
            cleared.sort();
            assert_eq!(
                cleared,
                [
                    "vela.balanceCache",
                    "vela.fiatRates.v1",
                    "vela.tokenMeta.100:0xabc"
                ]
            );
            assert_eq!(
                keys(),
                [
                    "vela.balanceHidden",
                    "vela.contacts",
                    "vela.perm.https://app.example"
                ]
            );
        });
    }

    /// A row's Clear takes its own keys. "Custom tokens and networks" does
    /// not take the RPC provider keys or the service endpoints with it.
    #[test]
    fn clearing_a_row_takes_only_that_rows_keys() {
        with_temp_state("clear-row", || {
            seed(&[
                ("vela.customTokens", json!([{ "address": "0x1" }])),
                ("vela.customNetworks", json!([{ "chainId": 7 }])),
                ("vela.networkConfig", json!([{ "chainId": 1 }])),
                ("vela.rpcProviders", json!({ "alchemy": "key" })),
                (
                    "vela.serviceEndpoints",
                    json!({ "fiatRatesURL": "https://x" }),
                ),
                ("vela.transactionHistory", json!([{ "id": "t" }])),
            ]);
            let mut cleared = clear_item("custom").unwrap_or_else(|error| unreachable!("{error}"));
            cleared.sort();
            assert_eq!(
                cleared,
                [
                    "vela.customNetworks",
                    "vela.customTokens",
                    "vela.networkConfig"
                ]
            );
            assert_eq!(
                keys(),
                [
                    "vela.rpcProviders",
                    "vela.serviceEndpoints",
                    "vela.transactionHistory"
                ]
            );
        });
    }

    /// The page's numbers come from the entries: each key lands in its row and
    /// group, counts are the row's own unit, and the groups add up to the
    /// total the headline states.
    #[test]
    fn a_measurement_files_every_key_in_its_row_and_group() {
        let entries: Vec<(String, String)> = [
            ("vela.transactionHistory", r#"[{"a":1},{"a":2},{"a":3}]"#),
            ("vela.contacts", r#"[{"a":1},{"a":2}]"#),
            ("vela.contactGroups", r#"[{"g":1}]"#),
            ("vela.customTokens", r#"[{"t":1}]"#),
            ("vela.customNetworks", r#"[{"n":1}]"#),
            ("vela.balanceCache", r#"{"x":1}"#),
            ("vela.balanceHidden", "1"),
            ("vela.perm.https://a.example", "{}"),
            ("vela.perm.https://b.example", "{}"),
            ("vela.req.https://a.example", "{}"),
            ("vela.accounts", "[]"),
            ("someone.else", "ignored"),
        ]
        .into_iter()
        .map(|(key, raw)| (key.to_owned(), raw.to_owned()))
        .collect();
        let report = measure_entries(&entries);

        assert_eq!(report.key_count, 11, "every key of ours, none of theirs");
        assert_eq!(report.item("transactions").count, 3);
        assert_eq!(report.item("contacts").count, 2, "people, not groups");
        assert_eq!(report.item("custom").count, 2);
        assert_eq!(report.item("dapps").count, 2, "sites, not request keys");
        assert_eq!(report.item("scan"), Usage::default());
        assert_eq!(
            report.group(StorageGroup::User)
                + report.group(StorageGroup::Cache)
                + report.group(StorageGroup::Sessions),
            report.total_bytes
        );
        let cache = ("vela.balanceCache".len() + r#"{"x":1}"#.len()) as u64;
        assert_eq!(
            report.group(StorageGroup::Cache),
            cache,
            "the hidden-balance switch is not a cache"
        );
    }
}
