//! What this machine actually holds, filed by row.
//!
//! The fourth copy of a mapping the other three shells have had since spec 047
//! — Android's `feature/settings/core/DeviceStorage.kt`, iOS's
//! `DeviceStorage.swift`, the web's `device-storage.ts`. Desktop drew the
//! storage page from a fixture: "1.0 MB · 200 records" beside a 清除 offering
//! to free an amount nobody measured, and none of the 清除 was even clickable.
//!
//! Two rules carried across, both about honesty:
//!
//! 1. **Filed by key, not by guess.** [`item_of`] is the same mapping on every
//!    client, so "browsing data" means the same keys everywhere and clearing it
//!    removes exactly those. A key that belongs to no row — the account
//!    records, the preferences — is counted under no row, because counting it
//!    under one that can be cleared would misdescribe what clearing does.
//! 2. **Records are counted only where they exist.** A value that is not a list
//!    has no record count, and the row then shows a size alone rather than an
//!    invented "1".

use serde_json::Value;

/// Which group a row sits in, which is also what its 清除 means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Group {
    /// Made by this person. Clearing loses something.
    User,
    /// Re-fetchable. Clearing costs a round trip.
    Cache,
    /// A site's access to this wallet.
    Sessions,
}

/// The drawn rows, in the order the page draws them. Ids are shared with the
/// other shells; the labels are the corpus's.
pub const ITEMS: [(&str, Group); 8] = [
    ("transactions", Group::User),
    ("contacts", Group::User),
    ("custom", Group::User),
    ("browsing", Group::User),
    ("balances", Group::Cache),
    ("rates", Group::Cache),
    ("scan", Group::Cache),
    ("dapps", Group::Sessions),
];

/// Which drawn row a key belongs to; `None` = nobody's row.
#[must_use]
pub fn item_of(key: &str) -> Option<&'static str> {
    match key {
        "vela.transactionHistory" => Some("transactions"),
        "vela.contactGroups" => Some("contacts"),
        "vela.customTokens" | "vela.customNetworks" => Some("custom"),
        "vela.browserHistory" | "vela.explore" | "vela.bhist" => Some("browsing"),
        "vela.balanceCache" | "vela.balanceHidden" => Some("balances"),
        "vela.rpc.banned" => Some("scan"),
        _ if key.starts_with("vela.contacts") => Some("contacts"),
        _ if key.starts_with("vela.fiatRates")
            || key.starts_with("vela.fxRates")
            || key.starts_with("vela.fiatFeedAddrs") =>
        {
            Some("rates")
        }
        _ if key.starts_with("vela.receiveWatch") || key.starts_with("vela.trust") => Some("scan"),
        _ if key.starts_with("vela.perm.") => Some("dapps"),
        _ => None,
    }
}

/// One row, measured.
#[derive(Clone, Debug, PartialEq)]
pub struct ItemReport {
    pub id: &'static str,
    pub group: Group,
    pub keys: Vec<String>,
    pub bytes: usize,
    /// `None` when the value is not a list — see rule 2.
    pub records: Option<u32>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub items: Vec<ItemReport>,
    pub total_bytes: usize,
    pub total_records: u32,
}

impl Report {
    #[must_use]
    pub fn bytes_of(&self, group: Group) -> usize {
        self.items
            .iter()
            .filter(|item| item.group == group)
            .map(|item| item.bytes)
            .sum()
    }

    #[must_use]
    pub fn item(&self, id: &str) -> Option<&ItemReport> {
        self.items.iter().find(|item| item.id == id)
    }
}

/// Measure the document this machine holds.
#[must_use]
pub fn measure() -> Report {
    report_of(&super::storage::read_all_public())
}

/// The measuring, over a map somebody already has — which is what makes it
/// testable without touching the real document.
#[must_use]
pub fn report_of(map: &serde_json::Map<String, Value>) -> Report {
    let items: Vec<ItemReport> = ITEMS
        .iter()
        .map(|&(id, group)| {
            let own: Vec<(&String, &Value)> = map
                .iter()
                .filter(|(key, _)| item_of(key) == Some(id))
                .collect();
            let bytes = own
                .iter()
                .map(|(_, value)| serde_json::to_string(value).map_or(0, |text| text.len()))
                .sum();
            let counted: Vec<u32> = own
                .iter()
                .filter_map(|(_, value)| records_in(value))
                .collect();
            ItemReport {
                id,
                group,
                keys: own.iter().map(|(key, _)| (*key).clone()).collect(),
                bytes,
                records: (!counted.is_empty()).then(|| counted.iter().sum()),
            }
        })
        .collect();
    Report {
        total_bytes: items.iter().map(|item| item.bytes).sum(),
        total_records: items.iter().filter_map(|item| item.records).sum(),
        items,
    }
}

/// A list's length, or a list-shaped member of an object; `None` when the value
/// is not a list at all.
#[must_use]
pub fn records_in(value: &Value) -> Option<u32> {
    if let Value::Array(items) = value {
        return u32::try_from(items.len()).ok();
    }
    let object = value.as_object()?;
    for name in ["items", "contacts", "entries", "records", "grants"] {
        if let Some(Value::Array(items)) = object.get(name) {
            return u32::try_from(items.len()).ok();
        }
    }
    None
}

/// Remove exactly one row's keys, and say which went.
pub fn clear(id: &str) -> Vec<String> {
    let keys: Vec<String> = super::storage::read_all_public()
        .keys()
        .filter(|key| item_of(key) == Some(id))
        .cloned()
        .collect();
    for key in &keys {
        let _ = super::storage::remove_value(key);
    }
    keys
}

/// Every cache row at once — the 清除全部缓存 link.
pub fn clear_caches() -> Vec<String> {
    ITEMS
        .iter()
        .filter(|(_, group)| *group == Group::Cache)
        .flat_map(|(id, _)| clear(id))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The mapping is a contract with the other three shells: the same key has
    /// to land in the same row everywhere, or "browsing data" means one thing
    /// on a phone and another on a desktop.
    #[test]
    fn keys_are_filed_the_way_the_other_shells_file_them() {
        assert_eq!(item_of("vela.transactionHistory"), Some("transactions"));
        assert_eq!(item_of("vela.contacts"), Some("contacts"));
        assert_eq!(item_of("vela.contacts.dismissed"), Some("contacts"));
        assert_eq!(item_of("vela.contactGroups"), Some("contacts"));
        assert_eq!(item_of("vela.customTokens"), Some("custom"));
        assert_eq!(item_of("vela.customNetworks"), Some("custom"));
        assert_eq!(item_of("vela.browserHistory"), Some("browsing"));
        assert_eq!(item_of("vela.balanceCache"), Some("balances"));
        assert_eq!(item_of("vela.fiatRates.USD"), Some("rates"));
        assert_eq!(item_of("vela.rpc.banned"), Some("scan"));
        assert_eq!(item_of("vela.perm.https://app.uniswap.org"), Some("dapps"));

        // Nobody's row, and deliberately so: these are not offered for
        // deletion here, so counting them under something that can be cleared
        // would be a lie about what clearing does.
        assert_eq!(item_of("vela.accounts"), None);
        assert_eq!(item_of("vela.activeAccountIndex"), None);
        assert_eq!(item_of("vela.textScale"), None);
        assert_eq!(item_of("vela.serviceEndpoints"), None);
    }

    #[test]
    fn a_value_that_is_not_a_list_has_no_record_count() {
        let map = serde_json::json!({
            "vela.transactionHistory": [{"id": "a"}, {"id": "b"}],
            "vela.balanceCache": {"total": "1.0"},
            "vela.contacts": {"contacts": [{"address": "0x1"}]},
            "vela.accounts": [{"address": "0x9"}, {"address": "0x8"}],
        })
        .as_object()
        .cloned()
        .unwrap_or_default();
        let report = report_of(&map);

        let txs = report.item("transactions").expect("row exists");
        assert_eq!(txs.records, Some(2));
        let balances = report.item("balances").expect("row exists");
        assert_eq!(
            balances.records, None,
            "an object with no list member is a size, not a count"
        );
        assert!(balances.bytes > 0, "but it still has a size");
        let contacts = report.item("contacts").expect("row exists");
        assert_eq!(contacts.records, Some(1), "counted through the list member");

        // `vela.accounts` is two records and belongs to no row, so it is in
        // neither the totals nor any row.
        assert_eq!(report.total_records, 3);
        assert!(
            report
                .items
                .iter()
                .all(|item| !item.keys.contains(&"vela.accounts".to_owned())),
            "the account records are nobody's row"
        );
    }

    #[test]
    fn the_cache_group_is_the_three_rows_it_says_it_is() {
        let cache: Vec<&str> = ITEMS
            .iter()
            .filter(|(_, group)| *group == Group::Cache)
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(cache, ["balances", "rates", "scan"]);
        assert_eq!(ITEMS.len(), 8, "the page draws eight rows");
    }

    #[test]
    fn json_is_measured_the_way_it_is_stored() {
        let map = json!({"vela.explore": {"favorites": []}})
            .as_object()
            .cloned()
            .unwrap_or_default();
        let report = report_of(&map);
        let browsing = report.item("browsing").expect("row exists");
        assert_eq!(browsing.bytes, r#"{"favorites":[]}"#.len());
    }
}
