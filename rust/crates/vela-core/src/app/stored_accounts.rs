//! The stored account list (`vela.accounts`), as every shell that keeps it
//! as text reads and writes it (§18 item 2; DESKTOP-1, ANDROID-8).
//!
//! One rule, here, for the web (wasm) and the phones (UniFFI): a record this
//! build cannot read is **said and kept, never dropped**.
//!
//! - **Read** ([`read_json`]): the records the core reads as an
//!   [`Account`](super::Account) — exactly as stored, for the session's
//!   `accounts_loaded` — and how many it cannot (with their positions and
//!   sizes, never their contents: a record holds addresses and public keys,
//!   which no log line carries). One bad record never costs the others. A
//!   value that is there but is not a list is ONE record this build cannot
//!   read — never an empty list, which the next upsert would write back
//!   without it (the mechanism of ANDROID-8).
//! - **Write** ([`upsert_json`], [`remove_json`]): every record this build
//!   cannot read stays where it was, its bytes as they were. A list shortened
//!   by what could not be read is never written. A value that is not JSON at
//!   all cannot sit in a list as it is, so it is kept as one record holding
//!   its text as a JSON string: every byte of it recoverable.
//! - **The way back**: signing in with the passkey writes the account's
//!   record again (an upsert, by `id`), so the person is never locked out by
//!   a record that cannot be read — and nothing they had is lost on the way.
//!
//! The desktop keeps its list as a JSON value inside `wallet.json`
//! (`executor::storage`) and reads and writes it through these same
//! functions, over that value's text (r14, §19).

use serde::de::IgnoredAny;
use serde::Serialize;
use serde_json::Value;

use super::Account;

/// Where the account list is stored, on every shell.
pub const KEY: &str = "vela.accounts";

/// One stored record: its text exactly as stored, and whether this build
/// reads it as an account.
struct Record {
    text: String,
    readable: bool,
}

/// A record this build cannot read: where it is and how big — never what.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct UnreadableAt {
    pub position: usize,
    pub bytes: usize,
}

/// What [`read_json`] answers.
#[derive(Debug)]
pub struct StoredRead {
    /// The records this build reads, exactly as stored (each one's text), in
    /// their order.
    pub accounts: Vec<String>,
    /// How many stored records this build cannot read (kept, and said).
    pub unreadable: usize,
    pub unreadable_at: Vec<UnreadableAt>,
}

/// What a write answers: the text to store, and the counts to log.
#[derive(Debug, Serialize)]
pub struct StoredWrite {
    /// The whole list to store under [`KEY`].
    pub stored: String,
    pub before: usize,
    pub after: usize,
    /// An upsert replaced the record with its `id` (else it was added).
    pub replaced: bool,
    /// Records this build cannot read, kept as they were.
    pub unreadable_kept: usize,
}

fn readable(text: &str) -> bool {
    serde_json::from_str::<Account>(text).is_ok()
}

fn record(text: String) -> Record {
    let readable = readable(&text);
    Record { text, readable }
}

/// The text of each element of `list`, a JSON array already known to be
/// valid — exactly as written (only the whitespace around each is dropped).
/// A scan rather than serde_json's `RawValue`, whose feature grows every
/// deserializer in the wasm.
fn elements(list: &str) -> Vec<&str> {
    let inner = list.trim();
    let inner = &inner[1..inner.len() - 1];
    let (mut out, mut depth, mut start) = (Vec::new(), 0usize, 0usize);
    let (mut in_string, mut escaped) = (false, false);
    for (i, b) in inner.bytes().enumerate() {
        if in_string {
            match (escaped, b) {
                (true, _) => escaped = false,
                (false, b'\\') => escaped = true,
                (false, b'"') => in_string = false,
                _ => {}
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' | b'{' => depth += 1,
            b']' | b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                out.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    let last = inner[start..].trim();
    if !last.is_empty() || !out.is_empty() {
        out.push(last);
    }
    out
}

/// The records stored, as stored. `None`, empty or `null`: none.
fn records(raw: Option<&str>) -> Vec<Record> {
    let Some(raw) = raw.filter(|r| !r.trim().is_empty()) else {
        return Vec::new();
    };
    if serde_json::from_str::<IgnoredAny>(raw).is_err() {
        // Not JSON at all: one record holding its text, every byte of it.
        return vec![Record {
            text: serde_json::to_string(raw).unwrap_or_else(|_| "null".to_owned()),
            readable: false,
        }];
    }
    match raw.trim() {
        // `null` is no list at all: nothing stored.
        "null" => Vec::new(),
        list if list.starts_with('[') => elements(list)
            .into_iter()
            .map(|item| record(item.to_owned()))
            .collect(),
        // A JSON value that is not a list: one record, as it is.
        value => vec![record(value.to_owned())],
    }
}

fn joined(records: &[Record]) -> String {
    let mut out = String::from("[");
    for (i, r) in records.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&r.text);
    }
    out.push(']');
    out
}

fn field(text: &str, name: &str) -> Option<String> {
    serde_json::from_str::<Value>(text)
        .ok()?
        .get(name)?
        .as_str()
        .map(str::to_owned)
}

/// Read the stored list: the records this build reads, as stored, and what
/// it cannot read, counted.
#[must_use]
pub fn read(raw: Option<&str>) -> StoredRead {
    let mut out = StoredRead {
        accounts: Vec::new(),
        unreadable: 0,
        unreadable_at: Vec::new(),
    };
    for (position, r) in records(raw).into_iter().enumerate() {
        if r.readable {
            out.accounts.push(r.text);
        } else {
            out.unreadable_at.push(UnreadableAt {
                position,
                bytes: r.text.len(),
            });
        }
    }
    out.unreadable = out.unreadable_at.len();
    out
}

/// Upsert `account_json` (a stored record, whole) by its `id`: the record
/// with that `id` is replaced where it stands, else it is added at the end.
/// Every other record — one this build cannot read too — stays as it was.
/// `None` when `account_json` is not a record with an `id`.
#[must_use]
pub fn upsert(raw: Option<&str>, account_json: &str) -> Option<StoredWrite> {
    let account = account_json.trim();
    let id = field(account, "id")?;
    let mut list = records(raw);
    let before = list.len();
    let at = list
        .iter()
        .position(|r| field(&r.text, "id").as_deref() == Some(id.as_str()));
    let fresh = record(account.to_owned());
    let replaced = at.is_some();
    match at {
        Some(at) => list[at] = fresh,
        None => list.push(fresh),
    }
    Some(StoredWrite {
        stored: joined(&list),
        before,
        after: list.len(),
        replaced,
        unreadable_kept: list.iter().filter(|r| !r.readable).count(),
    })
}

/// Which records [`remove`] takes.
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Remove {
    /// The record with this `id` (the parallel space's fixture).
    Id(String),
    /// Every record at this address, in any case (a row's identity is its
    /// address — session invariant ⑨). A record with no readable address is
    /// not it.
    Address(String),
}

/// Drop the records `by` names; every other — one this build cannot read
/// too — stays as it was. A record no longer there is a no-op.
#[must_use]
pub fn remove(raw: Option<&str>, by: &Remove) -> StoredWrite {
    let list = records(raw);
    let before = list.len();
    let kept: Vec<Record> = list
        .into_iter()
        .filter(|r| match by {
            Remove::Id(id) => field(&r.text, "id").as_deref() != Some(id.as_str()),
            Remove::Address(address) => {
                field(&r.text, "address").is_none_or(|a| !a.eq_ignore_ascii_case(address))
            }
        })
        .collect();
    StoredWrite {
        stored: joined(&kept),
        before,
        after: kept.len(),
        replaced: false,
        unreadable_kept: kept.iter().filter(|r| !r.readable).count(),
    }
}

// -- the JSON doors (wasm `storedAccounts*`, UniFFI `stored_accounts_*`) ------

/// [`read`] as JSON: `{accounts, unreadable, unreadable_at}` — each account
/// record's text as stored.
#[must_use]
pub fn read_json(raw: Option<&str>) -> String {
    let read = read(raw);
    let at = serde_json::to_string(&read.unreadable_at).unwrap_or_else(|_| "[]".to_owned());
    format!(
        r#"{{"accounts":[{}],"unreadable":{},"unreadable_at":{at}}}"#,
        read.accounts.join(","),
        read.unreadable
    )
}

/// [`upsert`] as JSON: `{stored, before, after, replaced, unreadable_kept}`,
/// or `null` for a record with no `id` (nothing to write).
#[must_use]
pub fn upsert_json(raw: Option<&str>, account_json: &str) -> String {
    upsert(raw, account_json)
        .and_then(|w| serde_json::to_string(&w).ok())
        .unwrap_or_else(|| "null".to_owned())
}

/// [`remove`] as JSON. `by_json`: `{"id": "…"}` or `{"address": "0x…"}`;
/// anything else is `null` (nothing to write).
#[must_use]
pub fn remove_json(raw: Option<&str>, by_json: &str) -> String {
    serde_json::from_str::<Remove>(by_json)
        .ok()
        .and_then(|by| serde_json::to_string(&remove(raw, &by)).ok())
        .unwrap_or_else(|| "null".to_owned())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    const GOOD: &str = r#"{"id":"cred-1","name":"Main","address":"0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32","public_key_hex":"04ab","created_at_iso":"2026-01-01T00:00:00Z","keys":[]}"#;
    const OTHER: &str = r#"{"id":"cred-2","name":"Two","address":"0x00000000000000000000000000000000000000Bb","public_key_hex":"04cd","created_at_iso":"2026-01-02T00:00:00Z","keys":[]}"#;
    /// A record this build cannot read: its key's id is a number.
    const BAD: &str = r#"{"id":"cred-9","address":"0x00000000000000000000000000000000000000Cc","keys":[{"credential_id":7}],"note":"a\/b 1.50"}"#;

    fn list(items: &[&str]) -> String {
        format!("[{}]", items.join(","))
    }

    fn texts(read: &StoredRead) -> Vec<&str> {
        read.accounts.iter().map(String::as_str).collect()
    }

    #[test]
    fn nothing_stored_is_no_record() {
        for raw in [None, Some(""), Some("  "), Some("null"), Some("[]")] {
            let r = read(raw);
            assert!(r.accounts.is_empty() && r.unreadable == 0, "{raw:?}");
        }
    }

    #[test]
    fn one_bad_record_costs_only_itself_and_is_counted_by_size_not_content() {
        let raw = list(&[GOOD, BAD, OTHER]);
        let r = read(Some(&raw));
        assert_eq!(texts(&r), vec![GOOD, OTHER]);
        assert_eq!(r.unreadable, 1);
        assert_eq!(
            r.unreadable_at,
            vec![UnreadableAt {
                position: 1,
                bytes: BAD.len()
            }]
        );
        let json = read_json(Some(&raw));
        assert!(
            !json.contains("cred-9")
                && !json.contains("0x00000000000000000000000000000000000000Cc")
        );
    }

    #[test]
    fn the_scan_splits_only_at_the_lists_own_commas() {
        let tricky = r#"{"id":"a,]}","keys":[{"n":"\\\"[,"}],"x":[1,{"y":2}]}"#;
        let raw = format!("[ {tricky} ,\n {GOOD} , 7,\"s\" ]");
        let split: Vec<String> = records(Some(&raw)).into_iter().map(|r| r.text).collect();
        assert_eq!(split, vec![tricky, GOOD, "7", "\"s\""]);
        let r = read(Some(&raw));
        assert_eq!((texts(&r), r.unreadable), (vec![GOOD], 3));
        let json: Value = serde_json::from_str(&read_json(Some(&raw))).unwrap();
        assert_eq!(json["accounts"][0]["id"], "cred-1");
        assert_eq!(json["unreadable"], 3);
    }

    #[test]
    fn a_value_that_is_not_a_list_is_one_unreadable_record() {
        for raw in [r#"{"a":1}"#, r#""a string""#, "7", "{not json"] {
            let r = read(Some(raw));
            assert!(r.accounts.is_empty(), "{raw}");
            assert_eq!(r.unreadable, 1, "{raw}");
        }
    }

    #[test]
    fn a_save_keeps_every_unreadable_record_byte_for_byte() {
        let raw = list(&[BAD, GOOD]);
        let fresh = OTHER;
        let w = upsert(Some(&raw), fresh).unwrap();
        assert_eq!(w.stored, list(&[BAD, GOOD, OTHER]));
        assert_eq!(
            (w.before, w.after, w.replaced, w.unreadable_kept),
            (2, 3, false, 1)
        );
        // The account signed in again replaces its own record where it stands.
        let renamed = GOOD.replace("Main", "Renamed");
        let w = upsert(Some(&w.stored), &renamed).unwrap();
        assert_eq!(w.stored, list(&[BAD, &renamed, OTHER]));
        assert!(w.replaced);
        assert!(w.stored.contains(BAD), "the bad record's bytes, as stored");
    }

    #[test]
    fn signing_in_over_a_value_that_is_no_list_keeps_it() {
        // The way back: the passkey's sign-in writes its record, and the value
        // this build could not read stays beside it.
        let w = upsert(Some(r#"{"a":1}"#), GOOD).unwrap();
        assert_eq!(w.stored, list(&[r#"{"a":1}"#, GOOD]));
        let r = read(Some(&w.stored));
        assert_eq!((texts(&r), r.unreadable), (vec![GOOD], 1));
        // Not JSON at all: kept as its text, every byte recoverable.
        let w = upsert(Some("{not json"), GOOD).unwrap();
        let kept: Vec<Value> = serde_json::from_str(&w.stored).unwrap();
        assert_eq!(kept[0], Value::String("{not json".to_owned()));
        assert_eq!(read(Some(&w.stored)).unreadable, 1);
    }

    #[test]
    fn a_removal_takes_only_what_it_names() {
        let raw = list(&[GOOD, BAD, OTHER]);
        let w = remove(
            Some(&raw),
            &Remove::Address("0x00000000000000000000000000000000000000BB".to_owned()),
        );
        assert_eq!(w.stored, list(&[GOOD, BAD]));
        assert_eq!((w.before, w.after, w.unreadable_kept), (3, 2, 1));
        let w = remove(Some(&w.stored), &Remove::Id("cred-1".to_owned()));
        assert_eq!(w.stored, list(&[BAD]));
        // Never shorter by what could not be read: nothing named, nothing gone.
        let w = remove(Some(&w.stored), &Remove::Id("cred-404".to_owned()));
        assert_eq!(w.stored, list(&[BAD]));
    }

    #[test]
    fn the_json_doors_answer_null_for_what_cannot_be_written() {
        assert_eq!(upsert_json(None, r#"{"name":"no id"}"#), "null");
        assert_eq!(upsert_json(None, "nope"), "null");
        assert_eq!(remove_json(None, r#"{"key":"x"}"#), "null");
        let w: Value = serde_json::from_str(&upsert_json(None, GOOD)).unwrap();
        assert_eq!(w["stored"], Value::String(list(&[GOOD])));
        let w: Value =
            serde_json::from_str(&remove_json(Some(&list(&[GOOD])), r#"{"id":"cred-1"}"#)).unwrap();
        assert_eq!(w["stored"], Value::String("[]".to_owned()));
    }
}
