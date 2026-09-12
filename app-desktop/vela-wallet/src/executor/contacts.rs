//! The only place the `contacts` machine touches the outside world.
//!
//! Seven operations: three ledgers it owns, and four questions it can only ask
//! of infrastructure this cut does not have yet.
//!
//! ## Four operations answer honestly-empty, and the distinction matters
//!
//! `load_send_history` has no local transaction store to read until spec 032, so
//! it answers `HistoryLoaded { txs: [] }` — a true statement about a store that
//! does not exist, not a failure. `resolve_identity` and `classify_recipient`
//! both need the RPC pool spec 031 brings, so they answer `None`.
//!
//! **`None` is "unknown", not a verdict.** The core says so in its own comments
//! and never caches it: an unreachable RPC must not become "this address is not
//! a contract", because the next screen would render a risk badge nobody
//! measured. Answering — rather than skipping — is what keeps the core from
//! waiting forever, and answering with the modelled *unknown* is what keeps the
//! answer from being a lie.
//!
//! ## The dismissed store is an object, and that is load-bearing
//!
//! `vela.contacts.dismissed` is `address → epoch ms`, not a list. A tombstone
//! suppresses a history-derived suggestion **unless the person has transacted
//! since the deletion**, so the timestamp is the whole mechanism. Written as a
//! list it would still round-trip through this file and quietly stop working.

use gpui::App;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use vela_core::app::contacts::{
    Contact, ContactGroup, ContactHistoryTx, ContactKind, ContactOperation, ContactShellResult,
    ContactSource, ContactTombstone, ContactTxKind, Contacts, Event,
};

use crate::executor::{identity, pool, storage};
use crate::resident::{Answer, Machine};
use crate::session;

// ---------------------------------------------------------------------------
// Stored shapes — camelCase, and optionals OMITTED rather than nulled
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct StoredContact {
    address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_source: Option<String>,
    kind: ContactKind,
    favorite: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    note: Option<String>,
    tx_count: u32,
    /// Stored as `lastUsed` / `firstSeen`; the core names the unit.
    last_used: f64,
    first_seen: f64,
    source: StoredSource,
}

/// `source` has no `Default` in the core, and an absent one means `manual` —
/// a record written before suggestions existed was saved by hand.
#[derive(Serialize, Deserialize, Default, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum StoredSource {
    #[default]
    Manual,
    Auto,
}

impl From<StoredSource> for ContactSource {
    fn from(s: StoredSource) -> Self {
        match s {
            StoredSource::Manual => ContactSource::Manual,
            StoredSource::Auto => ContactSource::Auto,
        }
    }
}

impl From<ContactSource> for StoredSource {
    fn from(s: ContactSource) -> Self {
        match s {
            ContactSource::Manual => StoredSource::Manual,
            ContactSource::Auto => StoredSource::Auto,
        }
    }
}

impl From<StoredContact> for Contact {
    fn from(s: StoredContact) -> Self {
        Self {
            // Lowercased on the way IN as well as out: the address is the
            // canonical key, and a record written with mixed case by an older
            // client must land in the same bucket as everything else.
            address: s.address.to_lowercase(),
            name: s.name.filter(|value| !value.is_empty()),
            resolved_name: s.resolved_name.filter(|value| !value.is_empty()),
            resolved_source: s.resolved_source.filter(|value| !value.is_empty()),
            kind: s.kind,
            favorite: s.favorite,
            note: s.note.filter(|value| !value.is_empty()),
            tx_count: s.tx_count,
            last_used_ms: s.last_used,
            first_seen_ms: s.first_seen,
            source: s.source.into(),
        }
    }
}

impl From<&Contact> for StoredContact {
    fn from(c: &Contact) -> Self {
        Self {
            address: c.address.clone(),
            name: c.name.clone(),
            resolved_name: c.resolved_name.clone(),
            resolved_source: c.resolved_source.clone(),
            kind: c.kind,
            favorite: c.favorite,
            note: c.note.clone(),
            tx_count: c.tx_count,
            last_used: c.last_used_ms,
            first_seen: c.first_seen_ms,
            source: c.source.into(),
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct StoredGroup {
    id: String,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    color: Option<String>,
    members: Vec<String>,
}

impl From<StoredGroup> for ContactGroup {
    fn from(s: StoredGroup) -> Self {
        Self {
            id: s.id,
            name: s.name,
            color: s.color.filter(|value| !value.is_empty()),
            members: s.members.iter().map(|m| m.to_lowercase()).collect(),
        }
    }
}

impl From<&ContactGroup> for StoredGroup {
    fn from(g: &ContactGroup) -> Self {
        Self {
            id: g.id.clone(),
            name: g.name.clone(),
            color: g.color.clone(),
            members: g.members.clone(),
        }
    }
}

fn read_list<S, T>(key: &str) -> Vec<T>
where
    S: for<'de> Deserialize<'de>,
    T: From<S>,
{
    let Ok(Some(Value::Array(items))) = storage::read_value(key) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<S>(item).ok())
        .map(T::from)
        .collect()
}

fn write_list<T, S: Serialize>(key: &str, items: &[T], to_stored: impl Fn(&T) -> S) {
    let encoded = Value::Array(
        items
            .iter()
            .map(|item| serde_json::to_value(to_stored(item)).unwrap_or(Value::Null))
            .collect(),
    );
    let _ = storage::write_value(key, encoded);
}

/// `address → epoch ms`. An object, not a list — see the module note.
fn read_tombstones() -> Vec<ContactTombstone> {
    let Ok(Some(Value::Object(map))) = storage::read_value(storage::KEY_CONTACTS_DISMISSED) else {
        return Vec::new();
    };
    map.into_iter()
        .filter_map(|(address, at)| {
            at.as_f64().map(|dismissed_at_ms| ContactTombstone {
                address: address.to_lowercase(),
                dismissed_at_ms,
            })
        })
        .collect()
}

fn write_tombstones(tombstones: &[ContactTombstone]) {
    let mut map = Map::new();
    for tombstone in tombstones {
        map.insert(
            tombstone.address.clone(),
            serde_json::Number::from_f64(tombstone.dismissed_at_ms)
                .map_or(Value::Null, Value::Number),
        );
    }
    let _ = storage::write_value(storage::KEY_CONTACTS_DISMISSED, Value::Object(map));
}

/// `vela.transactionHistory` — the shared local store, spelled once per reader.
const TX_KEY: &str = "vela.transactionHistory";

/// The send history behind the suggestion list, from the store the activity
/// feed writes.
///
/// One file, one shape, two readers. `activity_feed`'s mapper is the fuller one
/// (it needs amounts, status and day keys); this needs four fields, and reading
/// them here rather than borrowing that mapper keeps each machine's vocabulary
/// its own.
fn read_send_history() -> Vec<ContactHistoryTx> {
    let Ok(Some(Value::Array(rows))) = storage::read_value(TX_KEY) else {
        // A storage failure yields NO suggestions, which is the core's own
        // reading of `loadTransactions().catch(() => [])` — an empty book, not
        // a broken one.
        return Vec::new();
    };
    rows.iter()
        .map(|row| ContactHistoryTx {
            // Absent means a record older than the field. The core reads that
            // as "not a suggestion, but still prior interaction", and
            // substituting `Send` here would invent a recipient nobody sent to.
            kind: row
                .get("type")
                .and_then(Value::as_str)
                .and_then(|kind| match kind {
                    "send" => Some(ContactTxKind::Send),
                    "receive" => Some(ContactTxKind::Receive),
                    "dapp_tx" => Some(ContactTxKind::DappTx),
                    "sign_message" => Some(ContactTxKind::SignMessage),
                    "sign_typed_data" => Some(ContactTxKind::SignTypedData),
                    "connect" => Some(ContactTxKind::Connect),
                    _ => None,
                }),
            to: row
                .get("to")
                .and_then(Value::as_str)
                .filter(|to| !to.is_empty())
                .map(str::to_owned),
            to_name: row
                .get("toName")
                .and_then(Value::as_str)
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned),
            // Stored in SECONDS; the core wants milliseconds, and getting this
            // wrong dates every suggestion to 1970.
            timestamp_ms: row
                .get("timestamp")
                .and_then(Value::as_f64)
                .map(|seconds| seconds * 1000.0),
        })
        .collect()
}

impl Machine for Contacts {
    const LABEL: &'static str = "contacts";

    fn boot_event(cx: &App) -> Event {
        // Whose book this is, said explicitly. The core's own comment: miss
        // this and "a previous account's history would keep feeding
        // suggestions" — one person's address book bleeding into another's.
        let address = session::view(cx).address;
        Event::AccountSwitched {
            my_address: (!address.is_empty()).then_some(address),
        }
    }

    fn perform(operation: &ContactOperation) -> Answer<ContactShellResult, Self::Event> {
        match operation {
            ContactOperation::ReadStore => Answer::Now(ContactShellResult::StoreLoaded {
                contacts: read_list::<StoredContact, Contact>(storage::KEY_CONTACTS),
                tombstones: read_tombstones(),
                groups: read_list::<StoredGroup, ContactGroup>(storage::KEY_CONTACT_GROUPS),
            }),

            ContactOperation::WriteContacts { contacts } => {
                write_list(storage::KEY_CONTACTS, contacts, |c: &Contact| {
                    StoredContact::from(c)
                });
                Answer::Now(ContactShellResult::Written)
            }
            ContactOperation::WriteDismissed { tombstones } => {
                write_tombstones(tombstones);
                Answer::Now(ContactShellResult::Written)
            }
            ContactOperation::WriteGroups { groups } => {
                write_list(storage::KEY_CONTACT_GROUPS, groups, |g: &ContactGroup| {
                    StoredGroup::from(g)
                });
                Answer::Now(ContactShellResult::Written)
            }

            // Live since 031. The marker this replaced said "there is no local
            // transaction store yet" — which stopped being true the moment the
            // activity feed's receipt discovery started writing to
            // `vela.transactionHistory`. Left alone it would have kept the
            // suggestion list empty for a reason that had gone away.
            //
            // Every row is handed over, `receive` and `dapp_tx` included: which
            // of them may become a suggestion is the CORE's rule (only
            // `type: 'send'`, so a router or a token contract never pollutes the
            // trust signal) and its risk half counts prior interaction more
            // widely. Filtering here would answer one of those questions on the
            // core's behalf and get the other wrong.
            ContactOperation::LoadSendHistory => Answer::Now(ContactShellResult::HistoryLoaded {
                txs: read_send_history(),
            }),

            // Live since 031: the waterfall in `executor::identity`, shared with
            // `activity_feed`'s twin operation so one screen cannot learn a
            // name the other never does.
            ContactOperation::ResolveIdentity { address } => {
                let address = address.clone();
                Answer::Blocking(Box::new(move || {
                    let identity = identity::resolve(&address);
                    ContactShellResult::IdentityResolved { address, identity }
                }))
            }

            // Live since 031: `eth_getCode` through the pool, which is what
            // makes this answerable at all — the question is "what does this
            // chain say", and choosing which endpoint to ask is the pool's job.
            //
            // A failure still answers `None`, and `None` still means UNKNOWN
            // rather than "not a contract". The core never caches it, because
            // an unreachable RPC must not become a risk badge nobody measured.
            ContactOperation::ClassifyRecipient { chain_id, address } => {
                let (chain_id, address) = (*chain_id, address.clone());
                Answer::Blocking(Box::new(move || {
                    let code = pool::call(
                        chain_id,
                        "eth_getCode",
                        serde_json::json!([address, "latest"]),
                    )
                    .ok()
                    .and_then(|body| {
                        body.get("result")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    });
                    ContactShellResult::RecipientClassified {
                        chain_id,
                        address,
                        code,
                    }
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use serde_json::json;

    fn contact(address: &str, name: &str) -> Contact {
        Contact {
            address: address.to_owned(),
            name: Some(name.to_owned()),
            resolved_name: None,
            resolved_source: None,
            kind: ContactKind::Eoa,
            favorite: true,
            note: None,
            tx_count: 3,
            last_used_ms: 1_756_000_000_000.0,
            first_seen_ms: 1_700_000_000_000.0,
            source: ContactSource::Manual,
        }
    }

    /// The cross-client contract, on the RAW JSON. Optionals must be OMITTED,
    /// not written as null: `services/contacts.ts` produces a record without
    /// The suggestion source: every row handed over, with the CORE deciding
    /// which of them is a recipient.
    #[test]
    fn the_history_is_handed_over_whole_and_the_core_picks_the_recipients() {
        storage::tests::with_temp_state("contacts-history", || {
            let rows = serde_json::json!([
                {
                    "id": "a",
                    "type": "send",
                    "to": "0xAbC",
                    "toName": "Alice",
                    "timestamp": 1_788_500_000.0,
                },
                // A dApp call. The core drops it from suggestions — a router or
                // a token contract is not somebody you paid — but the SHELL
                // must not make that call, because the risk half counts prior
                // interaction more widely.
                { "id": "b", "type": "dapp_tx", "to": "0xRouter", "timestamp": 1.0 },
                // A receipt, written by the activity feed's discovery.
                { "id": "c", "type": "receive", "to": "0xMe", "timestamp": 2.0 },
                // A record older than the `type` field.
                { "id": "d", "to": "0xLegacy", "timestamp": 3.0 },
                // A blank name is not a name.
                { "id": "e", "type": "send", "to": "0xDef", "toName": "  " },
            ]);
            if storage::write_value(TX_KEY, rows).is_err() {
                unreachable!("could not seed");
            }

            let history = read_send_history();
            assert_eq!(history.len(), 5, "every row, not a filtered subset");
            assert_eq!(history[0].kind, Some(ContactTxKind::Send));
            assert_eq!(history[0].to.as_deref(), Some("0xAbC"));
            assert_eq!(history[0].to_name.as_deref(), Some("Alice"));
            // SECONDS on disk, milliseconds to the core. Getting this wrong
            // dates every suggestion to 1970.
            assert_eq!(history[0].timestamp_ms, Some(1_788_500_000_000.0));

            assert_eq!(history[1].kind, Some(ContactTxKind::DappTx));
            assert_eq!(history[2].kind, Some(ContactTxKind::Receive));
            // Absent `type` stays absent: the core reads it as "not a
            // suggestion, but still prior interaction".
            assert_eq!(history[3].kind, None);
            assert_eq!(history[4].to_name, None, "a blank name is not a name");

            // An unreadable store yields no suggestions, not a broken book.
            if storage::write_value(TX_KEY, serde_json::json!("nonsense")).is_err() {
                unreachable!("could not seed");
            }
            assert!(read_send_history().is_empty());
        });
    }

    /// the key, and a `null` there reads back as "explicitly cleared".
    #[test]
    fn a_stored_contact_matches_what_other_clients_write() {
        let stored = serde_json::to_value(StoredContact::from(&contact("0xabc", "Ada")))
            .unwrap_or_else(|_| unreachable!("a contact must serialize"));
        let object = stored
            .as_object()
            .unwrap_or_else(|| unreachable!("a contact is an object"));

        assert_eq!(object.get("address").and_then(Value::as_str), Some("0xabc"));
        assert_eq!(object.get("name").and_then(Value::as_str), Some("Ada"));
        assert_eq!(object.get("kind").and_then(Value::as_str), Some("eoa"));
        assert_eq!(object.get("txCount").and_then(Value::as_u64), Some(3));
        assert_eq!(
            object.get("lastUsed").and_then(Value::as_f64),
            Some(1_756_000_000_000.0)
        );
        assert_eq!(object.get("source").and_then(Value::as_str), Some("manual"));

        for absent in ["resolvedName", "resolvedSource", "note"] {
            assert!(
                !object.contains_key(absent),
                "`{absent}` must be omitted, not written as null"
            );
        }
        for rust_spelling in ["tx_count", "last_used_ms", "first_seen_ms", "resolved_name"] {
            assert!(
                !object.contains_key(rust_spelling),
                "`{rust_spelling}` leaked the Rust spelling"
            );
        }
    }

    /// The dismissed store is an OBJECT keyed by address. A list would
    /// round-trip through this file and silently stop suppressing suggestions,
    /// because the timestamp is what the suppression rule compares against.
    #[test]
    fn tombstones_persist_as_an_address_keyed_map() {
        storage::tests::with_temp_state("contacts-tombstones", || {
            write_tombstones(&[ContactTombstone {
                address: "0xabc".to_owned(),
                dismissed_at_ms: 1_756_000_000_000.0,
            }]);
            let raw = storage::read_value(storage::KEY_CONTACTS_DISMISSED)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("nothing was written"));
            assert!(raw.is_object(), "a list here would break suppression");
            assert_eq!(
                raw.get("0xabc").and_then(Value::as_f64),
                Some(1_756_000_000_000.0)
            );
            assert_eq!(read_tombstones().len(), 1);
        });
    }

    /// A record another client wrote reads back, including one with mixed-case
    /// address and no optional fields at all.
    #[test]
    fn a_record_written_by_another_client_reads_back() {
        storage::tests::with_temp_state("contacts-foreign", || {
            let foreign = json!([{
                "address": "0xAbCdEf0000000000000000000000000000000001",
                "kind": "account", "txCount": 2, "lastUsed": 1.0, "firstSeen": 0.5,
                "source": "auto"
            }]);
            if storage::write_value(storage::KEY_CONTACTS, foreign).is_err() {
                unreachable!("could not seed");
            }
            let contacts = read_list::<StoredContact, Contact>(storage::KEY_CONTACTS);
            assert_eq!(contacts.len(), 1);
            assert_eq!(
                contacts[0].address, "0xabcdef0000000000000000000000000000000001",
                "the canonical key is lowercase"
            );
            assert_eq!(contacts[0].kind, ContactKind::Account);
            assert_eq!(contacts[0].source, ContactSource::Auto);
            assert_eq!(contacts[0].name, None);
        });
    }

    /// The operation READS the store now.
    ///
    /// This asserted `txs.is_empty()` while the marker said "no transaction
    /// store yet", and it kept passing after one appeared — because it read
    /// whatever state directory the process happened to point at rather than a
    /// seeded one. A test that cannot tell an empty store from an unread one
    /// cannot notice the arm going live, which is exactly what it was there to
    /// watch.
    #[test]
    fn the_history_operation_reads_the_store_rather_than_answering_empty() {
        storage::tests::with_temp_state("contacts-history-op", || {
            // A genuinely empty store is still empty — that half was always
            // true and stays true.
            match Contacts::perform(&ContactOperation::LoadSendHistory) {
                Answer::Now(ContactShellResult::HistoryLoaded { txs }) => assert!(txs.is_empty()),
                _ => unreachable!("history is local"),
            }

            let rows = serde_json::json!([
                { "id": "a", "type": "send", "to": "0xAbC", "timestamp": 1.0 }
            ]);
            if storage::write_value(TX_KEY, rows).is_err() {
                unreachable!("could not seed");
            }
            match Contacts::perform(&ContactOperation::LoadSendHistory) {
                Answer::Now(ContactShellResult::HistoryLoaded { txs }) => {
                    assert_eq!(txs.len(), 1, "the store has a row and it must arrive");
                    assert_eq!(txs[0].to.as_deref(), Some("0xAbC"));
                }
                _ => unreachable!("history is local"),
            }
        });
    }

    /// Classification goes to the chain since 031, and the answer identifies
    /// what it classified.
    ///
    /// The golden Safe IS a contract, so a real run must come back with runtime
    /// code — and an EOA must come back with `0x`. Those are different facts and
    /// the core turns them into different badges; conflating them is how a
    /// wallet calls a person's own address a contract.
    #[test]
    #[ignore = "classifies two real addresses on Gnosis"]
    fn a_real_address_is_classified_from_the_chain() {
        let classify = |address: &str| {
            let Answer::Blocking(work) = Contacts::perform(&ContactOperation::ClassifyRecipient {
                chain_id: 100,
                address: address.to_owned(),
            }) else {
                unreachable!("classification is network work since 031");
            };
            match work() {
                ContactShellResult::RecipientClassified { code, chain_id, .. } => {
                    assert_eq!(chain_id, 100, "the answer must identify what it classified");
                    code
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        };

        let safe = classify("0x88cCA0EeDbF2C4426110bbFc998F048689266894")
            .unwrap_or_else(|| unreachable!("the chain did not answer for the Safe"));
        println!(
            "  Safe   -> {} bytes of code",
            safe.len().saturating_sub(2) / 2
        );
        assert!(safe.len() > 2, "a deployed Safe has runtime code");

        // The zero address is not a contract; `0x` is a VERDICT, and it is a
        // different answer from `None`.
        let eoa = classify("0x0000000000000000000000000000000000000001")
            .unwrap_or_else(|| unreachable!("the chain did not answer for the EOA"));
        println!("  non-contract -> {eoa:?}");
        assert_eq!(eoa, "0x", "an address with no code answers 0x, not None");
    }

    /// Saved, then gone after a relaunch — through the real loop.
    #[test]
    fn a_saved_contact_survives_a_relaunch() {
        storage::tests::with_temp_state("contacts-relaunch", || {
            write_list(
                storage::KEY_CONTACTS,
                &[contact("0xabc", "Ada")],
                |c: &Contact| StoredContact::from(c),
            );

            let mut host = CoreHost::<Contacts>::new();
            let mut pending = host.dispatch(Event::AccountSwitched {
                my_address: Some("0xme".to_owned()),
            });
            while let Some(next) = pending.pop() {
                match Contacts::perform(&next.operation) {
                    Answer::Now(result) => pending.extend(host.resolve(next.id, result)),
                    _ => continue,
                }
            }
            let view = format!("{:?}", host.view());
            assert!(
                view.contains("Ada"),
                "the stored contact did not reach the view"
            );
        });
    }
}
