//! On-device storage for the wallet's account list.
//!
//! One JSON file under the platform config directory, holding the SAME KEYS the
//! browser puts in `localStorage` and the Expo client puts in AsyncStorage:
//! `vela.accounts`, `vela.activeAccountIndex`, `vela.pendingUploads`,
//! `vela.serviceEndpoints`. Those names are not decoration — they are how a
//! record written by one client is still legible after it is copied to another,
//! and they are what the operations contract pins.
//!
//! ## The invariant every function here carries
//!
//! `Account` holds both the legacy scalar key fields and the full `keys` array,
//! and the core derives the address from **all** keys. A mapper that copies an
//! account field by field and drops `keys` does not merely lose data: it
//! silently "repairs" a multi-key account into a different, wrong, single-key
//! Safe on the next restore, at an address nothing can deploy. So nothing here
//! reshapes an account. Records go in and come out whole, as
//! `serde_json::Value` all the way to the core's own `serde` impls.

use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::{Map, Value, json};

use vela_core::app::{Account, PendingUpload};

/// The four keys, spelled exactly as every other client spells them.
pub const KEY_ACCOUNTS: &str = "vela.accounts";
pub const KEY_ACTIVE_INDEX: &str = "vela.activeAccountIndex";
pub const KEY_PENDING_UPLOADS: &str = "vela.pendingUploads";
pub const KEY_SERVICE_ENDPOINTS: &str = "vela.serviceEndpoints";

/// The keys the wallet-state machines own (spec 030). Same names, and the same
/// camelCase FIELD names inside, as the web and Expo clients — a record written
/// on one client has to stay legible on another, which is what makes copying a
/// wallet between machines work at all.
pub const KEY_CONTACTS: &str = "vela.contacts";
pub const KEY_CONTACTS_DISMISSED: &str = "vela.contacts.dismissed";
pub const KEY_CONTACT_GROUPS: &str = "vela.contactGroups";
pub const KEY_CUSTOM_NETWORKS: &str = "vela.customNetworks";
pub const KEY_NETWORK_CONFIG: &str = "vela.networkConfig";
pub const KEY_RPC_PROVIDERS: &str = "vela.rpcProviders";
pub const KEY_RPC_BANNED: &str = "vela.rpc.banned";
pub const KEY_DISPLAY_CURRENCY: &str = "vela.displayCurrency";

/// The storage failed in a way the core answers with `storage_failed`, never a
/// crash: a read-only home directory, a full disk, a file another process holds.
#[derive(Debug)]
pub struct StorageError(pub String);

impl std::fmt::Display for StorageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

type Result<T> = std::result::Result<T, StorageError>;

/// Serialises writes within this process. Two screens can be saving at once —
/// the session machine's migration write-back and an onboarding save — and the
/// file is rewritten whole, so an interleave would lose one of them.
static LOCK: Mutex<()> = Mutex::new(());

/// Where the file lives.
///
/// `dirs::config_dir()` rather than a hand-rolled `~/.vela`: each desktop has
/// its own convention (`~/Library/Application Support`, `%APPDATA%`,
/// `$XDG_CONFIG_HOME`) and a wallet that ignores it is a wallet the platform's
/// own backup and migration tools do not know about.
pub fn path() -> Result<PathBuf> {
    // `VELA_STATE_DIR` overrides it — the same env-switch family as
    // `VELA_THEME` / `VELA_LANG` / `VELA_GALLERY`. It exists so a test can run
    // against a temporary directory instead of the developer's real wallet,
    // which is not a hypothetical concern: every function in this file
    // REWRITES the document.
    if let Ok(dir) = std::env::var("VELA_STATE_DIR")
        && !dir.is_empty()
    {
        return Ok(PathBuf::from(dir).join("wallet.json"));
    }
    let base = dirs::config_dir()
        .ok_or_else(|| StorageError("this system has no configuration directory".to_owned()))?;
    Ok(base.join("VelaWallet").join("wallet.json"))
}

fn read_all() -> Result<Map<String, Value>> {
    let path = path()?;
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        // A file that is not there yet is an empty wallet, not a failure.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Map::new()),
        Err(error) => return Err(StorageError(format!("{}: {error}", path.display()))),
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(Value::Object(map)) => Ok(map),
        // Corrupt JSON reads as empty rather than throwing, exactly as the web
        // client does: a damaged file must not make the wallet permanently
        // unopenable, and every write below replaces the whole document.
        _ => Ok(Map::new()),
    }
}

fn write_all(map: Map<String, Value>) -> Result<()> {
    let path = path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| StorageError(format!("{}: {error}", parent.display())))?;
    }
    let body = serde_json::to_string_pretty(&Value::Object(map))
        .map_err(|error| StorageError(error.to_string()))?;

    // Write beside the target and rename over it. A crash halfway through a
    // direct write leaves a truncated account list, and an account list that
    // parses as EMPTY is indistinguishable from being signed out.
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, body)
        .map_err(|error| StorageError(format!("{}: {error}", temporary.display())))?;
    fs::rename(&temporary, &path)
        .map_err(|error| StorageError(format!("{}: {error}", path.display())))
}

fn read_list(key: &str) -> Result<Vec<Value>> {
    Ok(match read_all()?.get(key) {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    })
}

/// Read one key, whole and unreshaped.
///
/// `None` means absent; a corrupt document reads as absent rather than failing,
/// exactly as `read_all` decided for every other reader here.
pub fn read_value(key: &str) -> Result<Option<Value>> {
    Ok(read_all()?.get(key).cloned())
}

/// What this wallet is actually using on disk, and how many records it holds.
///
/// One JSON document, so the size is one `metadata` call and the record count
/// is the sum of the array-valued keys plus one for each scalar. The settings
/// screen said **2.4 MB / 216 records** to everybody; a person deciding whether
/// to clear a cache deserves their own number.
///
/// `(bytes, records)`. Zero for both when the file is not there yet, which is a
/// true statement about a wallet that has written nothing.
#[must_use]
pub fn usage() -> (u64, u32) {
    let Ok(path) = path() else {
        return (0, 0);
    };
    let bytes = fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);
    let records = read_all().map_or(0, |map| {
        map.values()
            .map(|value| match value {
                // An array key holds N records; anything else is one.
                Value::Array(items) => u32::try_from(items.len()).unwrap_or(u32::MAX),
                Value::Null => 0,
                _ => 1,
            })
            .sum()
    });
    (bytes, records)
}

/// Write one key, whole.
///
/// Codecs live in the executors, not here — which is what lets a new machine
/// arrive without touching this file (spec 030 SC-004).
pub fn write_value(key: &str, value: Value) -> Result<()> {
    write_key(key, value)
}

/// Merge fields into an object-valued key, leaving its siblings alone.
///
/// `vela.serviceEndpoints` has two independent writers — onboarding's registry
/// override and `network_admin`'s endpoint editor — and a whole-value write from
/// either erases the other's fields. See `save_registry_endpoint`.
pub fn merge_value(key: &str, fields: Map<String, Value>) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    let mut object = match map.get(key) {
        Some(Value::Object(existing)) => existing.clone(),
        _ => Map::new(),
    };
    for (field, value) in fields {
        if value.is_null() {
            object.remove(&field);
        } else {
            object.insert(field, value);
        }
    }
    map.insert(key.to_owned(), Value::Object(object));
    write_all(map)
}

/// Remove one key entirely.
///
/// Not `write_value(key, Null)`: a stored null is a record the usage count and
/// every future reader still have to step over, and a revoked permission
/// should leave nothing behind that says a site was ever here.
pub fn remove_value(key: &str) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    map.remove(key);
    write_all(map)
}

fn write_key(key: &str, value: Value) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    map.insert(key.to_owned(), value);
    write_all(map)
}

// ---------------------------------------------------------------------------
// Accounts
// ---------------------------------------------------------------------------

pub fn load_accounts() -> Result<Vec<Account>> {
    let mut accounts = Vec::new();
    for item in read_list(KEY_ACCOUNTS)? {
        // A record that will not deserialize is SKIPPED, not fatal. One
        // corrupt entry among five must not lock a person out of the other
        // four; the whole-list failure mode is what `AccountsUnavailable` is
        // for, and it is reserved for the file itself being unreadable.
        if let Ok(account) = serde_json::from_value::<Account>(item) {
            accounts.push(account);
        }
    }
    Ok(accounts)
}

/// Upsert by id. The whole record is written — see the invariant above.
pub fn save_account(account: &Account) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    let mut accounts = match map.get(KEY_ACCOUNTS) {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };
    let encoded = serde_json::to_value(account).map_err(|error| StorageError(error.to_string()))?;
    let at = accounts
        .iter()
        .position(|item| item.get("id") == encoded.get("id"));
    match at {
        Some(at) => accounts[at] = encoded,
        None => accounts.push(encoded),
    }
    map.insert(KEY_ACCOUNTS.to_owned(), Value::Array(accounts));
    write_all(map)
}

pub fn load_active_index() -> usize {
    // Missing, garbage and negative all read as 0. A negative index would make
    // the session render an empty address with a wallet present, which the core
    // forbids — so it fails closed here rather than at the wire.
    read_all()
        .ok()
        .and_then(|map| map.get(KEY_ACTIVE_INDEX).cloned())
        .and_then(|value| match value {
            Value::Number(number) => number.as_i64(),
            Value::String(text) => text.parse::<i64>().ok(),
            _ => None,
        })
        .filter(|index| *index > 0)
        .and_then(|index| usize::try_from(index).ok())
        .unwrap_or(0)
}

pub fn save_active_index(index: usize) -> Result<()> {
    write_key(KEY_ACTIVE_INDEX, json!(index))
}

// ---------------------------------------------------------------------------
// Pending uploads
// ---------------------------------------------------------------------------

pub fn has_pending_uploads() -> Result<bool> {
    Ok(!read_list(KEY_PENDING_UPLOADS)?.is_empty())
}

/// Keyed by `id`, which for a pending upload IS the credential id of its first
/// founding key.
pub fn save_pending_upload(record: &PendingUpload) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    let encoded = serde_json::to_value(record).map_err(|error| StorageError(error.to_string()))?;
    let mut pending = match map.get(KEY_PENDING_UPLOADS) {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };
    pending.retain(|item| item.get("id") != encoded.get("id"));
    pending.push(encoded);
    map.insert(KEY_PENDING_UPLOADS.to_owned(), Value::Array(pending));
    write_all(map)
}

pub fn remove_pending_upload(credential_id: &str) -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    let mut pending = match map.get(KEY_PENDING_UPLOADS) {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };
    pending.retain(|item| item.get("id").and_then(Value::as_str) != Some(credential_id));
    map.insert(KEY_PENDING_UPLOADS.to_owned(), Value::Array(pending));
    write_all(map)
}

// ---------------------------------------------------------------------------
// Endpoints
// ---------------------------------------------------------------------------

/// The saved passkey-index endpoint override, if any.
///
/// Reads `passkeyIndexURL` and falls back to the legacy desktop-only `registry`.
/// Those are the same fact under two names: the desktop's "registry" IS the
/// passkey index (`executor/registry.rs` defaults to `p256-index-v2.getvela.app`
/// and accepts the identity `webauthn-p256-publickey-index`), and every other
/// client — web's `services/endpoints.ts`, the Expo client — spells it
/// `passkeyIndexURL`. Until spec 030 the desktop wrote a name nothing else could
/// read, so a self-hosted index configured here was invisible everywhere else.
pub fn load_registry_endpoint() -> Option<String> {
    let endpoints = read_all().ok()?;
    let object = endpoints.get(KEY_SERVICE_ENDPOINTS)?;
    object
        .get("passkeyIndexURL")
        .or_else(|| object.get("registry"))?
        .as_str()
        .map(str::to_owned)
}

/// Save it under the cross-client name, and drop the legacy one in the same
/// write so the two cannot disagree on the next launch.
///
/// MERGES rather than replaces. `network_admin` writes the other three fields of
/// this key (`ethereumDataURL`, `bundlerServiceURL`, `fiatRatesURL`), and the
/// whole-value write this used to do would have erased them — and theirs would
/// have erased this.
/// A first-run / replay flag: epoch milliseconds, or `None` when never
/// written (spec 038). The two callers — the intro's "seen" and the launch
/// animation's "played" — use the SAME key names as the web (`vela.intro.seen`,
/// `vela.launch.played`) so the four shells share one vocabulary.
pub fn read_epoch_ms(key: &str) -> Option<f64> {
    read_value(key).ok().flatten().and_then(|value| value.as_f64())
}

/// Write such a flag. Best-effort in the same way the web's is: an unwritable
/// store means a second viewing, which is cosmetic; a front door that fails
/// because a decoration could not write is not.
pub fn write_epoch_ms(key: &str, now_ms: f64) {
    if let Err(error) = write_value(key, Value::from(now_ms)) {
        eprintln!("[vela-wallet] {key} could not be saved: {error}");
    }
}

pub fn save_registry_endpoint(url: &str) -> Result<()> {
    let mut fields = Map::new();
    fields.insert("passkeyIndexURL".to_owned(), json!(url));
    fields.insert("registry".to_owned(), Value::Null);
    merge_value(KEY_SERVICE_ENDPOINTS, fields)
}

// ---------------------------------------------------------------------------
// Sign-out
// ---------------------------------------------------------------------------

/// Forget which wallet this computer is signed into — the account list and the
/// active index, and NOTHING else.
///
/// The scope is the decision, not an implementation detail. Contacts, history,
/// custom tokens and networks, endpoints and preferences belong to the ACCOUNT
/// rather than to the session, and the account comes back intact because its
/// address derives from the passkey rather than from disk. The pending-upload
/// outbox is excluded for a second, independent reason: a record there is a
/// public key the registry never confirmed, and the next launch can still retry
/// it — but a deleted record can never be retried, and that credential becomes
/// unfindable at sign-in.
pub fn clear_signed_in_wallet() -> Result<()> {
    let Ok(_guard) = LOCK.lock() else {
        return Err(StorageError("the storage lock is poisoned".to_owned()));
    };
    let mut map = read_all()?;
    map.remove(KEY_ACCOUNTS);
    map.remove(KEY_ACTIVE_INDEX);
    write_all(map)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use vela_core::app::AccountKey;

    /// One temporary state directory per test.
    ///
    /// `VELA_STATE_DIR` is process-wide, so these tests are serialized behind
    /// one lock rather than run in parallel — the alternative is a shared
    /// document two tests rewrite at once, which is exactly the interleave the
    /// file lock in this module exists to prevent.
    /// Two writers share `vela.serviceEndpoints`, and until spec 030 either
    /// erased the other. Onboarding saves the passkey-index override;
    /// `network_admin` saves the other three service URLs. The whole-value write
    /// this replaced meant configuring a self-hosted index silently unset the
    /// data, bundler and fiat endpoints — and saving those silently unset the
    /// The storage panel's two figures, measured rather than asserted.
    #[test]
    fn usage_counts_records_and_measures_the_file() {
        tests::with_temp_state("storage-usage", || {
            // Nothing written yet: zero of both, which is true about a wallet
            // that has stored nothing — not a failure to measure.
            assert_eq!(usage(), (0, 0));

            if write_value("vela.accounts", serde_json::json!([{ "a": 1 }, { "a": 2 }])).is_err()
                || write_value("vela.balanceHidden", serde_json::json!("1")).is_err()
                || write_value("vela.empty", serde_json::json!([])).is_err()
            {
                unreachable!("could not seed");
            }
            let (bytes, records) = usage();
            // Two array entries plus one scalar. An empty array holds nothing
            // and counts as nothing.
            assert_eq!(records, 3);
            assert!(bytes > 0, "the file is on disk and has a size");
        });
    }

    /// index, sending the next launch back to the public default.
    #[test]
    fn saving_one_service_endpoint_leaves_its_siblings_alone() {
        with_temp_state("endpoints-merge", || {
            let mut others = Map::new();
            others.insert("ethereumDataURL".to_owned(), json!("https://data.example"));
            others.insert("fiatRatesURL".to_owned(), json!("https://rates.example"));
            if merge_value(KEY_SERVICE_ENDPOINTS, others).is_err() {
                unreachable!("could not seed the endpoints");
            }

            if save_registry_endpoint("https://idx.example").is_err() {
                unreachable!("could not save the index endpoint");
            }

            let stored = read_value(KEY_SERVICE_ENDPOINTS)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("the endpoints vanished"));
            assert_eq!(
                stored.get("ethereumDataURL").and_then(Value::as_str),
                Some("https://data.example"),
                "a sibling endpoint was erased"
            );
            assert_eq!(
                stored.get("fiatRatesURL").and_then(Value::as_str),
                Some("https://rates.example")
            );
            assert_eq!(
                stored.get("passkeyIndexURL").and_then(Value::as_str),
                Some("https://idx.example")
            );
        });
    }

    /// The desktop used to spell this field `registry`; every other client
    /// spells it `passkeyIndexURL`. A record written by an older desktop must
    /// still be read, and must converge on the shared name the moment anything
    /// writes it — so the two names cannot disagree on the next launch.
    #[test]
    fn a_legacy_registry_endpoint_is_read_and_then_converged() {
        with_temp_state("endpoints-legacy", || {
            let mut legacy = Map::new();
            legacy.insert("registry".to_owned(), json!("https://old.example"));
            if merge_value(KEY_SERVICE_ENDPOINTS, legacy).is_err() {
                unreachable!("could not seed the legacy record");
            }
            assert_eq!(
                load_registry_endpoint().as_deref(),
                Some("https://old.example"),
                "an older desktop's record became unreadable"
            );

            if save_registry_endpoint("https://new.example").is_err() {
                unreachable!("could not save");
            }
            let stored = read_value(KEY_SERVICE_ENDPOINTS)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("the endpoints vanished"));
            assert!(
                stored.get("registry").is_none(),
                "the legacy field survived, so two names can now disagree"
            );
            assert_eq!(
                load_registry_endpoint().as_deref(),
                Some("https://new.example")
            );
        });
    }

    /// Every key this cut's machines own round-trips whole, and reads as absent
    /// rather than failing when it has never been written.
    #[test]
    fn the_wallet_state_keys_round_trip_and_start_absent() {
        with_temp_state("wallet-state-keys", || {
            for key in [
                KEY_CUSTOM_NETWORKS,
                KEY_NETWORK_CONFIG,
                KEY_RPC_PROVIDERS,
                KEY_DISPLAY_CURRENCY,
            ] {
                assert!(
                    matches!(read_value(key), Ok(None)),
                    "{key} should start absent"
                );
            }

            let networks = json!([{ "id": "custom-100", "chainId": 100 }]);
            if write_value(KEY_CUSTOM_NETWORKS, networks.clone()).is_err() {
                unreachable!("could not write networks");
            }
            assert_eq!(
                read_value(KEY_CUSTOM_NETWORKS).ok().flatten(),
                Some(networks)
            );

            // A bare string, not an object — matching what the other clients store.
            if write_value(KEY_DISPLAY_CURRENCY, json!("JPY")).is_err() {
                unreachable!("could not write the currency");
            }
            assert_eq!(
                read_value(KEY_DISPLAY_CURRENCY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str),
                Some("JPY")
            );
        });
    }

    pub(crate) fn with_temp_state<T>(name: &str, body: impl FnOnce() -> T) -> T {
        static SERIAL: Mutex<()> = Mutex::new(());
        let Ok(_guard) = SERIAL.lock() else {
            unreachable!("the test lock is poisoned");
        };
        let dir = std::env::temp_dir().join(format!("vela-storage-test-{name}"));
        let _ = fs::remove_dir_all(&dir);
        if fs::create_dir_all(&dir).is_err() {
            unreachable!("could not create the temporary state directory");
        }
        // SAFETY: the lock above makes this the only thread touching the
        // variable for the duration of `body`.
        unsafe { std::env::set_var("VELA_STATE_DIR", &dir) };
        let out = body();
        let _ = fs::remove_dir_all(&dir);
        out
    }

    /// Put one un-confirmed public key in the outbox. Written as JSON rather
    /// than through `save_pending_upload` because the shape is what matters to
    /// the caller, not the record.
    pub(crate) fn write_pending_upload(credential_id: &str) {
        let Ok(path) = path() else {
            unreachable!("path");
        };
        let mut map = read_all().unwrap_or_default();
        map.insert(
            KEY_PENDING_UPLOADS.to_owned(),
            json!([{ "id": credential_id }]),
        );
        let Ok(body) = serde_json::to_string(&Value::Object(map)) else {
            unreachable!("serialize");
        };
        let _ = fs::write(&path, body);
    }

    fn account(id: &str, keys: usize) -> Account {
        Account {
            id: id.to_owned(),
            name: "Everyday wallet".to_owned(),
            address: "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33".to_owned(),
            public_key_hex: "04aa".to_owned(),
            created_at_iso: "2026-08-25T00:00:00.000Z".to_owned(),
            keys: (0..keys)
                .map(|index| AccountKey {
                    credential_id: format!("cred{index}"),
                    public_key_hex: format!("04{index:02}"),
                    name: format!("Key {}", index + 1),
                    transports: "usb".to_owned(),
                })
                .collect(),
        }
    }

    /// THE invariant. A multi-key account that comes back with fewer keys is a
    /// different, wrong, single-key Safe at an address nothing can deploy — and
    /// it fails silently, on the next launch, after the wallet was funded.
    #[test]
    fn a_multi_key_account_round_trips_with_every_key() {
        with_temp_state("multi-key", || {
            let saved = account("wallet-1", 3);
            if save_account(&saved).is_err() {
                unreachable!("save failed");
            }
            let loaded = match load_accounts() {
                Ok(accounts) => accounts,
                Err(error) => unreachable!("{error}"),
            };
            assert_eq!(loaded.len(), 1);
            assert_eq!(
                loaded[0].keys.len(),
                3,
                "keys were dropped on the round trip"
            );
            assert_eq!(loaded[0], saved);
        });
    }

    /// A legacy single-key record has NO `keys` array, and that emptiness is
    /// the fact — filling it in would invent a founding set.
    #[test]
    fn a_legacy_account_keeps_its_empty_key_list() {
        with_temp_state("legacy", || {
            let saved = account("wallet-legacy", 0);
            if save_account(&saved).is_err() {
                unreachable!("save failed");
            }
            match load_accounts() {
                Ok(accounts) => assert!(accounts[0].keys.is_empty()),
                Err(error) => unreachable!("{error}"),
            }
        });
    }

    /// Upsert by id, not append: saving the same wallet twice must not produce
    /// two accounts a switcher would show side by side.
    #[test]
    fn saving_the_same_account_twice_replaces_it() {
        with_temp_state("upsert", || {
            let mut saved = account("wallet-1", 2);
            let _ = save_account(&saved);
            saved.name = "Renamed".to_owned();
            let _ = save_account(&saved);
            match load_accounts() {
                Ok(accounts) => {
                    assert_eq!(accounts.len(), 1);
                    assert_eq!(accounts[0].name, "Renamed");
                }
                Err(error) => unreachable!("{error}"),
            }
        });
    }

    /// Corrupt JSON reads as an empty wallet rather than an error, exactly as
    /// the web client does: a damaged file must not make the wallet
    /// permanently unopenable, and every write replaces the whole document.
    #[test]
    fn a_corrupt_file_reads_as_empty_rather_than_failing() {
        with_temp_state("corrupt", || {
            let Ok(path) = path() else {
                unreachable!("path");
            };
            let _ = fs::write(&path, "{not json");
            match load_accounts() {
                Ok(accounts) => assert!(accounts.is_empty()),
                Err(error) => unreachable!("a corrupt file must not fail: {error}"),
            }
        });
    }

    /// Missing, garbage and negative all read as 0. A negative index would make
    /// the session render an empty address with a wallet present, which the
    /// core forbids.
    #[test]
    fn the_active_index_fails_closed() {
        with_temp_state("active-index", || {
            assert_eq!(load_active_index(), 0, "missing");
            let Ok(path) = path() else {
                unreachable!("path");
            };
            for raw in [
                r#"{"vela.activeAccountIndex": -3}"#,
                r#"{"vela.activeAccountIndex": "x"}"#,
            ] {
                let _ = fs::write(&path, raw);
                assert_eq!(load_active_index(), 0, "{raw}");
            }
            let _ = fs::write(&path, r#"{"vela.activeAccountIndex": 2}"#);
            assert_eq!(load_active_index(), 2);
        });
    }

    /// Sign-out drops the account list and the active index, and NOTHING else.
    /// The pending-upload outbox in particular survives: a record there is a
    /// public key the registry never confirmed, and a deleted record can never
    /// be retried — that credential becomes unfindable at sign-in.
    #[test]
    fn signing_out_leaves_the_pending_outbox_and_the_endpoint_alone() {
        with_temp_state("sign-out", || {
            let _ = save_account(&account("wallet-1", 2));
            let _ = save_active_index(1);
            let _ = save_registry_endpoint("https://example.invalid");
            let Ok(path) = path() else {
                unreachable!("path");
            };
            // Written directly: the point is the KEY surviving, and building a
            // whole `PendingUpload` would test serde rather than the scope.
            let Ok(raw) = fs::read_to_string(&path) else {
                unreachable!("read");
            };
            let Ok(Value::Object(mut map)) = serde_json::from_str::<Value>(&raw) else {
                unreachable!("parse");
            };
            map.insert(KEY_PENDING_UPLOADS.to_owned(), json!([{ "id": "cred0" }]));
            let Ok(body) = serde_json::to_string(&Value::Object(map)) else {
                unreachable!("serialize");
            };
            let _ = fs::write(&path, body);

            if clear_signed_in_wallet().is_err() {
                unreachable!("clear failed");
            }
            match load_accounts() {
                Ok(accounts) => assert!(accounts.is_empty()),
                Err(error) => unreachable!("{error}"),
            }
            assert_eq!(load_active_index(), 0);
            match has_pending_uploads() {
                Ok(pending) => assert!(pending, "the outbox must survive a sign-out"),
                Err(error) => unreachable!("{error}"),
            }
            assert_eq!(
                load_registry_endpoint().as_deref(),
                Some("https://example.invalid"),
                "the endpoint belongs to the machine, not to the session"
            );
        });
    }
}
