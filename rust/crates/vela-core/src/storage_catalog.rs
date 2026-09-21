//! What this device holds for Vela, row by row, and what "erase" removes
//! (spec 072) — the catalog every shell's storage page and erase read.
//!
//! Each shell kept its own copy and they had drifted: which row a key
//! belongs to (the phones filed `vela.balanceHidden`, a preference, as a
//! balance cache, so "clear all caches" un-hid balances), which keys the
//! "custom" row clears, and how an erase finds its keys (iOS walked a
//! hand-kept delete list, which is wrong by default: a key added next year is
//! never erased). The rules are the web's (`device-storage.ts`,
//! `erase-device.ts`), extended with the native shells' own keys.
//!
//! The shells keep what only they can do — enumerate their stores, delete,
//! verify, count bytes — and ask this module what each key IS.

/// The drawn groups of the storage page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageGroup {
    /// Cannot be recovered once cleared.
    User,
    /// Rebuilt on its own.
    Cache,
    /// Connections: clearing one disconnects a site.
    Sessions,
}

impl StorageGroup {
    #[must_use]
    pub fn id(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Cache => "cache",
            Self::Sessions => "sessions",
        }
    }
}

/// One drawn row: its id (the fixture ids every shell already uses), its
/// group, the exact keys it holds and the prefixes of the stores that write
/// one key per subject.
pub struct StorageItem {
    pub id: &'static str,
    pub group: StorageGroup,
    pub keys: &'static [&'static str],
    pub prefixes: &'static [&'static str],
}

/// Every row, in the order the page draws them.
pub const ITEMS: [StorageItem; 8] = [
    StorageItem {
        id: "transactions",
        group: StorageGroup::User,
        keys: &["vela.transactionHistory"],
        prefixes: &[],
    },
    StorageItem {
        id: "contacts",
        group: StorageGroup::User,
        keys: &[
            "vela.contacts",
            "vela.contactGroups",
            "vela.contacts.dismissed",
        ],
        prefixes: &[],
    },
    // Custom tokens and networks, and each network's own overrides. The RPC
    // provider keys and the service endpoints are NOT here: they have their
    // own pages (and "reset"), and a row that says "tokens and networks"
    // must not delete someone's API keys.
    StorageItem {
        id: "custom",
        group: StorageGroup::User,
        keys: &[
            "vela.customTokens",
            "vela.customNetworks",
            "vela.networkConfig",
        ],
        prefixes: &[],
    },
    StorageItem {
        id: "browsing",
        group: StorageGroup::User,
        keys: &["vela.browserHistory", "vela.explore", "vela.bhist"],
        prefixes: &[],
    },
    StorageItem {
        id: "balances",
        group: StorageGroup::Cache,
        keys: &["vela.balanceCache"],
        prefixes: &[],
    },
    StorageItem {
        id: "rates",
        group: StorageGroup::Cache,
        keys: &[],
        prefixes: &["vela.fiatRates", "vela.fxRates", "vela.fiatFeedAddrs"],
    },
    StorageItem {
        id: "scan",
        group: StorageGroup::Cache,
        keys: &["vela.rpc.banned", "vela.recipientIdentity"],
        prefixes: &[
            "vela.tokenMeta.",
            "recipient_id:",
            "vela.scan",
            "vela.receiveWatch",
        ],
    },
    StorageItem {
        id: "dapps",
        group: StorageGroup::Sessions,
        keys: &["vela.ext.cache"],
        prefixes: &["vela.perm.", "vela.chain.", "vela.req."],
    },
];

/// Every key the app writes is under this prefix — the erase is a scan of it.
pub const KEY_PREFIX: &str = "vela.";

/// The one `vela.` key an erase leaves behind: a passkey public key the index
/// service has never confirmed. Its retry needs no account list, but a
/// deleted record can never be retried — and that key could then never be
/// found at sign-in on any device (web `erase-device.ts`).
pub const ERASE_KEEP_KEYS: [&str; 1] = ["vela.pendingUploads"];

/// The drawn row a key belongs to; `None` is "your data nobody named" (the
/// accounts, the preferences), counted as user data and never swept by
/// "clear all caches".
#[must_use]
pub fn item_of_key(key: &str) -> Option<&'static StorageItem> {
    ITEMS
        .iter()
        .find(|item| item.keys.contains(&key))
        .or_else(|| {
            ITEMS
                .iter()
                .find(|item| item.prefixes.iter().any(|prefix| key.starts_with(prefix)))
        })
}

/// The group a key counts toward.
#[must_use]
pub fn group_of_key(key: &str) -> StorageGroup {
    item_of_key(key).map_or(StorageGroup::User, |item| item.group)
}

/// Would "clear all caches" remove this key? Exactly the cache group's.
#[must_use]
pub fn is_cache_key(key: &str) -> bool {
    group_of_key(key) == StorageGroup::Cache
}

/// Is this one of ours at all: the `vela.` namespace, plus the one
/// unprefixed cache.
#[must_use]
pub fn is_ours(key: &str) -> bool {
    key.starts_with(KEY_PREFIX) || key.starts_with("recipient_id:")
}

/// Would "erase this device" delete this key? Everything of ours but the
/// keep-list — found by scanning, never by a list of what to delete.
#[must_use]
pub fn is_erasable_key(key: &str) -> bool {
    is_ours(key) && !ERASE_KEEP_KEYS.contains(&key)
}

/// How many records a stored value holds, for the rows that count records:
/// a JSON array's length, or an object's `items` / `contacts` / `entries` /
/// `records` / `grants` array. `None` when the value is not a list.
#[must_use]
pub fn records_in(value: &str) -> Option<usize> {
    let parsed: serde_json::Value = serde_json::from_str(value.trim()).ok()?;
    match &parsed {
        serde_json::Value::Array(list) => Some(list.len()),
        serde_json::Value::Object(map) => ["items", "contacts", "entries", "records", "grants"]
            .iter()
            .find_map(|name| map.get(*name).and_then(serde_json::Value::as_array))
            .map(Vec::len),
        _ => None,
    }
}

/// A byte count as a number and a unit, in 1024s — what every shell's
/// storage page writes (Android had used 1000s). The shell formats the number
/// in the person's own number format.
#[must_use]
pub fn bytes_display(bytes: u64) -> (f64, &'static str) {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    (value, UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(key: &str) -> Option<&'static str> {
        item_of_key(key).map(|item| item.id)
    }

    #[test]
    fn every_shells_keys_land_in_the_same_row() {
        assert_eq!(row("vela.transactionHistory"), Some("transactions"));
        assert_eq!(row("vela.contacts.dismissed"), Some("contacts"));
        assert_eq!(row("vela.networkConfig"), Some("custom"));
        assert_eq!(row("vela.bhist"), Some("browsing"));
        assert_eq!(row("vela.fiatRates.v1"), Some("rates"));
        assert_eq!(row("vela.fiatFeedAddrs.v1"), Some("rates"));
        assert_eq!(row("vela.tokenMeta.100:0xabc"), Some("scan"));
        assert_eq!(row("recipient_id:0xabc"), Some("scan"));
        assert_eq!(row("vela.receiveWatch.0xabc"), Some("scan"));
        assert_eq!(row("vela.perm.https://app.uniswap.org"), Some("dapps"));
        assert_eq!(row("vela.chain.https://app.uniswap.org"), Some("dapps"));
    }

    #[test]
    fn preferences_and_the_wallet_are_nobodys_row() {
        for key in [
            "vela.accounts",
            "vela.activeAccountIndex",
            "vela.balanceHidden",
            "vela.theme",
            "vela.rpcProviders",
            "vela.serviceEndpoints",
            "vela.feeTier",
            "vela.signMethod",
        ] {
            assert_eq!(row(key), None, "{key}");
            assert_eq!(group_of_key(key), StorageGroup::User);
            assert!(!is_cache_key(key), "clear-all-caches must not touch {key}");
        }
    }

    #[test]
    fn erase_is_a_scan_of_the_namespace_minus_the_keep_list() {
        assert!(is_erasable_key("vela.theme"));
        assert!(is_erasable_key("vela.perm.https://x"));
        assert!(is_erasable_key("recipient_id:0xabc"));
        assert!(is_erasable_key("vela.some-key-added-next-year"));
        assert!(!is_erasable_key("vela.pendingUploads"));
        assert!(!is_erasable_key("theme_preference"));
    }

    #[test]
    fn records_and_bytes() {
        assert_eq!(records_in("[1,2,3]"), Some(3));
        assert_eq!(records_in(r#"{"contacts":[{},{}]}"#), Some(2));
        assert_eq!(records_in(r#""1""#), None);
        assert_eq!(bytes_display(512), (512.0, "B"));
        assert_eq!(bytes_display(1536), (1.5, "KB"));
        assert_eq!(bytes_display(3 * 1024 * 1024), (3.0, "MB"));
    }
}
