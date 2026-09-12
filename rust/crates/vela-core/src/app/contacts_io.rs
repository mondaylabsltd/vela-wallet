//! The address book as a FILE — how it is written out and how one is read
//! back in (spec 028 US5, FR-408).
//!
//! Lifted into the core from the desktop shell's `executor/contact_io.rs`
//! (031), itself a port of Expo's `src/services/contact-io.ts` @ `c513c4c6`.
//! It moves here because four shells now speak this format — web, desktop,
//! Android, iOS — and a backup written on any one of them must open on every
//! other. A rule about which column is the address, or whether `"name": ""`
//! means "no name", cannot be allowed to drift per platform: the moment it
//! does, somebody's export stops importing on their other device.
//!
//! What lives here is FORMAT: JSON-vs-CSV sniffing, quoting, line endings,
//! which column is which. What it yields is already-parsed rows; the import
//! POLICY — existing-wins, the counts, which groups are created — is
//! `contacts.rs`'s `apply_import`, and none of it is re-decided here.
//!
//! ## The CSV heuristics are not tidiness
//!
//! A foreign file rarely spells the column `address` — `wallet`, `Public
//! Address` and `Recipient` are all common — and the version this ports from
//! records what happened when an unrecognised header fell back to column 0: if
//! column 0 held the NAME, every row failed the address test, every row was
//! dropped silently, and the import reported "0 added, 0 already existed".
//! Nothing imported, nothing explained, nothing to try differently. So when the
//! header does not say where the address is, **the data does** — and a file
//! that plainly held rows and yielded no address at all is REFUSED with a
//! reason, never "succeeded" with zero of everything (D50: refuse before any
//! write).

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::contacts::{
    is_address, Contact, ContactFileFormat, ContactGroup, ContactImportEntry, ContactImportGroup,
};

/// The backup document's version. Not ours to bump alone: every client reads
/// these bytes.
pub const BACKUP_VERSION: u64 = 1;

/// What a file yielded, before the core rules on any of it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ParsedContactsFile {
    pub contacts: Vec<ContactImportEntry>,
    pub groups: Vec<ContactImportGroup>,
}

/// Why an import file was refused — before anything was written (D50).
///
/// Each variant is something a person can act on differently: pick a JSON or
/// CSV file, add an address column, choose a file that is not empty. A refusal
/// is distinct from an empty parse on purpose: "0 added" is indistinguishable
/// from an empty address book, and the file's mistake would be erased before
/// anyone saw it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactImportFailure {
    /// Named `.json` (or shaped like it) and not parseable as a JSON object.
    MalformedJson,
    /// A CSV that plainly held contact rows and yielded no address anywhere.
    NoAddressColumn,
    /// No rows at all.
    Empty,
    /// "Import into this group" named a group that no longer exists.
    UnknownGroup,
}

// ---------------------------------------------------------------------------
// Serialize
// ---------------------------------------------------------------------------

/// The JSON backup, pretty-printed as every client writes it:
/// `{version, exportedAt, contacts: [{address, name?, note?, favorite?}],
/// groups: [{name, color?, members: [address]}]}`. Group members are
/// ADDRESSES, not ids, so an import maps them by name (ids are per-device).
#[must_use]
pub fn to_json(contacts: &[Contact], groups: &[ContactGroup], exported_at: &str) -> String {
    let backup = json!({
        "version": BACKUP_VERSION,
        "exportedAt": exported_at,
        "contacts": contacts.iter().map(exported_contact).collect::<Vec<_>>(),
        "groups": groups
            .iter()
            .map(|group| {
                let mut object = Map::new();
                object.insert("name".to_owned(), json!(group.name));
                if let Some(color) = group.color.as_ref().filter(|c| !c.is_empty()) {
                    object.insert("color".to_owned(), json!(color));
                }
                object.insert("members".to_owned(), json!(group.members));
                Value::Object(object)
            })
            .collect::<Vec<_>>(),
    });
    serde_json::to_string_pretty(&backup).unwrap_or_else(|_| "{}".to_owned())
}

/// An absent field is OMITTED, not written as null or "". A backup that says
/// `"name": ""` re-imports a contact whose name is the empty string.
fn exported_contact(contact: &Contact) -> Value {
    let mut object = Map::new();
    object.insert("address".to_owned(), json!(contact.address));
    if let Some(name) = contact.name.as_ref().filter(|n| !n.is_empty()) {
        object.insert("name".to_owned(), json!(name));
    }
    if let Some(note) = contact.note.as_ref().filter(|n| !n.is_empty()) {
        object.insert("note".to_owned(), json!(note));
    }
    if contact.favorite {
        object.insert("favorite".to_owned(), json!(true));
    }
    Value::Object(object)
}

/// The CSV backup: `address,name,note,favorite,groups`, groups `;`-joined per
/// row. `\n` line endings, quoted only where a cell needs it.
#[must_use]
pub fn to_csv(contacts: &[Contact], groups: &[ContactGroup]) -> String {
    let mut lines = vec!["address,name,note,favorite,groups".to_owned()];
    for contact in contacts {
        let memberships: Vec<&str> = groups
            .iter()
            .filter(|group| group.members.contains(&contact.address))
            .map(|group| group.name.as_str())
            .collect();
        lines.push(
            [
                contact.address.clone(),
                contact.name.clone().unwrap_or_default(),
                contact.note.clone().unwrap_or_default(),
                if contact.favorite { "true" } else { "" }.to_owned(),
                memberships.join(";"),
            ]
            .iter()
            .map(|cell| csv_cell(cell))
            .collect::<Vec<_>>()
            .join(","),
        );
    }
    lines.join("\n")
}

/// Quote only when the cell needs it — a comma, a quote or a newline.
fn csv_cell(value: &str) -> String {
    if value.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", value.replace('"', "\"\""))
    } else {
        value.to_owned()
    }
}

/// The MIME type a shell hands the file out under.
#[must_use]
pub fn mime_for(format: ContactFileFormat) -> &'static str {
    match format {
        ContactFileFormat::Json => "application/json",
        ContactFileFormat::Csv => "text/csv",
    }
}

/// `vela-contacts[-<group>][-<yyyy-mm-dd>].<ext>`.
///
/// The date is read off the ISO stamp the shell supplied (the core has no
/// clock); a stamp that does not start `yyyy-mm-dd` contributes nothing rather
/// than a garbled suffix. A group's name is slugged to what every filesystem
/// accepts — alphanumerics of any script survive (家人 stays 家人), everything
/// else becomes one dash — and a name that slugs to nothing is called `group`.
#[must_use]
pub fn export_filename(
    group_name: Option<&str>,
    format: ContactFileFormat,
    exported_at_iso: &str,
) -> String {
    let mut name = String::from("vela-contacts");
    if let Some(group) = group_name {
        let slug = slug(group);
        name.push('-');
        name.push_str(if slug.is_empty() { "group" } else { &slug });
    }
    if let Some(date) = iso_date_prefix(exported_at_iso) {
        name.push('-');
        name.push_str(date);
    }
    name.push('.');
    name.push_str(match format {
        ContactFileFormat::Json => "json",
        ContactFileFormat::Csv => "csv",
    });
    name
}

fn slug(text: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for ch in text.trim().chars() {
        if ch.is_alphanumeric() {
            out.extend(ch.to_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
        if out.chars().count() >= 32 {
            break;
        }
    }
    out.trim_end_matches('-').to_owned()
}

fn iso_date_prefix(iso: &str) -> Option<&str> {
    let head = iso.get(..10)?;
    let bytes = head.as_bytes();
    let digits_at = |range: std::ops::Range<usize>| bytes[range].iter().all(u8::is_ascii_digit);
    (digits_at(0..4) && bytes[4] == b'-' && digits_at(5..7) && bytes[7] == b'-' && digits_at(8..10))
        .then_some(head)
}

// ---------------------------------------------------------------------------
// Parse
// ---------------------------------------------------------------------------

/// JSON or CSV, detected by extension then by shape.
pub fn parse(
    content: &str,
    filename: Option<&str>,
) -> Result<ParsedContactsFile, ContactImportFailure> {
    // A BOM in front of `{` is still JSON, and a BOM in front of a header is
    // still a header.
    let trimmed = content.trim_start_matches('\u{feff}').trim();
    let looks_json = filename.is_some_and(|name| name.to_lowercase().ends_with(".json"))
        || trimmed.starts_with('{');
    if looks_json {
        parse_json(trimmed)
    } else {
        parse_csv(trimmed)
    }
}

/// `{contacts: [...], groups: [...]}` — both optional (an object with neither
/// is an empty backup, as it always was). Anything that is not a JSON object is
/// refused: a person who picked the wrong file must hear so, not "0 added".
fn parse_json(text: &str) -> Result<ParsedContactsFile, ContactImportFailure> {
    let data =
        serde_json::from_str::<Value>(text).map_err(|_| ContactImportFailure::MalformedJson)?;
    if !data.is_object() {
        return Err(ContactImportFailure::MalformedJson);
    }
    let contacts = data
        .get("contacts")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(imported_contact).collect())
        .unwrap_or_default();
    let groups = data
        .get("groups")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some(ContactImportGroup {
                        name: item.get("name").and_then(Value::as_str)?.to_owned(),
                        color: item.get("color").and_then(Value::as_str).map(str::to_owned),
                        members: item
                            .get("members")
                            .and_then(Value::as_array)
                            .map(|members| {
                                members
                                    .iter()
                                    .filter_map(|m| m.as_str().map(str::to_owned))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(ParsedContactsFile { contacts, groups })
}

fn imported_contact(value: &Value) -> Option<ContactImportEntry> {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    Some(ContactImportEntry {
        address: value.get("address").and_then(Value::as_str)?.to_owned(),
        name: text("name"),
        note: text("note"),
        // `true` or the string "true" — a CSV round-tripped through a
        // spreadsheet comes back as the second.
        favorite: match value.get("favorite") {
            Some(Value::Bool(true)) => Some(true),
            Some(Value::String(text)) if text == "true" => Some(true),
            _ => None,
        },
    })
}

/// Split one CSV line, honouring quotes and doubled quotes
/// (`recipient-table.ts::splitCsvLine`, comma delimiter).
#[must_use]
pub fn split_csv_line(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = line.chars().peekable();
    while let Some(ch) = chars.next() {
        if in_quotes {
            if ch == '"' {
                if chars.peek() == Some(&'"') {
                    cur.push('"');
                    chars.next();
                } else {
                    in_quotes = false;
                }
            } else {
                cur.push(ch);
            }
        } else if ch == '"' {
            in_quotes = true;
        } else if ch == ',' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(ch);
        }
    }
    out.push(cur);
    out
}

/// Which column holds the address: the header's word if it says so, else the
/// first column that actually contains one.
fn address_column(header: Option<&[String]>, rows: &[Vec<String>]) -> Option<usize> {
    if let Some(header) = header {
        if let Some(index) = header.iter().position(|h| h.to_lowercase() == "address") {
            return Some(index);
        }
    }
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    (0..width).find(|i| {
        rows.iter()
            .any(|row| row.get(*i).is_some_and(|cell| is_address(cell)))
    })
}

struct Columns {
    address: usize,
    name: Option<usize>,
    note: Option<usize>,
    favorite: Option<usize>,
    groups: Option<usize>,
}

fn index_columns(header: Option<&[String]>, address: usize, named_address: bool) -> Columns {
    let find = |header: &[String], word: &str| header.iter().position(|h| h.to_lowercase() == word);
    // The first column that is NOT the address one — the de-facto label.
    let first_other = usize::from(address == 0);

    match header {
        // The file speaks our vocabulary: take every column it names and infer
        // nothing beyond them.
        Some(header) if named_address => Columns {
            address,
            name: find(header, "name"),
            note: find(header, "note"),
            favorite: find(header, "favorite"),
            groups: find(header, "groups"),
        },
        // A foreign header (`label,wallet`): its words told us nothing, so keep
        // only what is unambiguous.
        Some(header) => Columns {
            address,
            name: find(header, "name").or(Some(first_other)),
            note: None,
            favorite: None,
            groups: None,
        },
        // Headerless in our own export order — but only when the address sits
        // where that order puts it. Anywhere else and the file has told us
        // nothing about the rest.
        None if address == 0 => Columns {
            address,
            name: Some(1),
            note: Some(2),
            favorite: Some(3),
            groups: Some(4),
        },
        None => Columns {
            address,
            name: Some(first_other),
            note: None,
            favorite: None,
            groups: None,
        },
    }
}

fn parse_csv(text: &str) -> Result<ParsedContactsFile, ContactImportFailure> {
    let lines: Vec<&str> = text
        .split(['\n', '\r'])
        .filter(|line| !line.trim().is_empty())
        .collect();
    let Some(first_line) = lines.first() else {
        return Err(ContactImportFailure::Empty);
    };
    let first: Vec<String> = split_csv_line(first_line)
        .into_iter()
        .map(|cell| cell.trim().to_owned())
        .collect();
    // A first row containing an address is DATA, not a header.
    let has_header = !first.iter().any(|cell| is_address(cell));
    let rows: Vec<Vec<String>> = lines
        .iter()
        .skip(usize::from(has_header))
        .map(|line| {
            split_csv_line(line)
                .into_iter()
                .map(|cell| cell.trim().to_owned())
                .collect()
        })
        .collect();

    let header = has_header.then_some(first.as_slice());
    let named_address = header.is_some_and(|h| h.iter().any(|c| c.to_lowercase() == "address"));
    let columns = index_columns(
        header,
        address_column(header, &rows).unwrap_or(0),
        named_address,
    );

    let mut contacts = Vec::new();
    let mut group_map: Vec<(String, Vec<String>)> = Vec::new();
    let mut attempted = 0u32;
    let mut valid = 0u32;
    for cells in &rows {
        let cell = |index: Option<usize>| {
            index
                .and_then(|i| cells.get(i))
                .map(String::as_str)
                .filter(|s| !s.is_empty())
        };
        let Some(address) = cell(Some(columns.address)) else {
            // Nothing where the address goes is structure — a blank line or a
            // separator — not a contact anyone tried to import.
            continue;
        };
        let address = address.to_owned();
        attempted += 1;

        // A malformed row is carried through, NOT dropped: "is this an address"
        // is `apply_import`'s question and it counts the answer. Swallowing bad
        // rows here made `invalid` structurally zero on this path.
        contacts.push(ContactImportEntry {
            address: address.clone(),
            name: cell(columns.name).map(str::to_owned),
            note: cell(columns.note).map(str::to_owned),
            favorite: cell(columns.favorite)
                .filter(|value| matches!(value.to_lowercase().as_str(), "true" | "1" | "yes"))
                .map(|_| true),
        });
        if !is_address(&address) {
            continue;
        }
        valid += 1;

        if let Some(names) = cell(columns.groups) {
            for name in names.split(';').map(str::trim).filter(|n| !n.is_empty()) {
                let lower = address.to_lowercase();
                match group_map.iter_mut().find(|(existing, _)| existing == name) {
                    Some((_, members)) => members.push(lower),
                    None => group_map.push((name.to_owned(), vec![lower])),
                }
            }
        }
    }

    // Rows that plainly meant to be contacts, and not one address among them.
    if attempted > 0 && valid == 0 {
        return Err(ContactImportFailure::NoAddressColumn);
    }
    if attempted == 0 {
        return Err(ContactImportFailure::Empty);
    }
    Ok(ParsedContactsFile {
        contacts,
        groups: group_map
            .into_iter()
            .map(|(name, members)| ContactImportGroup {
                name,
                color: None,
                members,
            })
            .collect(),
    })
}
