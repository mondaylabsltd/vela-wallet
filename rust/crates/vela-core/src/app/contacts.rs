//! Machine — the address book (spec `016-crux-wallet-state`, contacts P2).
//!
//! ```text
//! AccountSwitched ─► ReadStore + LoadSendHistory ─► ledger {saved, tombstones, groups, history}
//!        Save/Delete/Toggle/Group*/Import ─► mutate ledger ─► WriteStore (best effort)
//!        view() ─► merged book = saved ⊕ history-derived (tombstone-suppressed)
//! ```
//!
//! A contact is a *recipient* you've sent to, or one you save by hand — never a
//! contract you merely called. The auto-suggestion source is deliberately
//! narrow: only `type: 'send'` transfers, whose `to` is the actual recipient;
//! dApp contract calls (`dapp_tx`) are excluded so routers/tokens/dApps never
//! pollute the trust signal (`src/services/contacts.ts:1-16` and `:294`).
//!
//! Ported faithfully from `src/services/contacts.ts`, `contact-io.ts`
//! (import half), `recipient-risk.ts` and `recipient-identity.ts` (cache
//! policy). The TS `_writeChain` write lock (contacts.ts:71-81) has no
//! equivalent here: the core is single-threaded by construction, so every
//! mutation is atomic. The TS `clearContactsCache()` implicit invalidation
//! becomes the explicit [`Event::AccountSwitched`] — miss it and a previous
//! account's history would keep feeding suggestions, which is exactly the bug
//! the event exists to prevent (inventory.md integration notes).
//!
//! The shell owns storage, RPC, the file picker/saver and every word on
//! screen; the core owns the merge, the tombstones, the group ledger, the
//! import policy, the file FORMAT (`contacts_io.rs`, since spec 028 — four
//! shells read each other's backups) and the trust semantics (green check =
//! saved ∧ favorite, `RecipientTrust.tsx:5-8`).

use std::collections::{BTreeMap, BTreeSet};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

pub use super::contacts_initials::initial_of;
use super::contacts_initials::{section_letter, section_rank};
use super::contacts_io;
pub use super::contacts_io::ContactImportFailure;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// EOA / smart-contract account / unknown — lazily filled by classification.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactKind {
    Eoa,
    Account,
    #[default]
    Unknown,
}

/// `manual` = saved/named by the user; `auto` = a live suggestion from history.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactSource {
    Manual,
    Auto,
}

/// One address-book entry. Serialises 1:1 to the TS `Contact` shape the shell
/// persists under `vela.contacts` (timestamps are epoch ms as `f64` — no u64
/// crosses the wire).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Contact {
    /// Lowercased address — the canonical key.
    pub address: String,
    /// User-given name. Wins over `resolved_name` for display.
    pub name: Option<String>,
    /// Cached identity name (ENS / Basename / passkey), for display + search.
    pub resolved_name: Option<String>,
    pub resolved_source: Option<String>,
    pub kind: ContactKind,
    pub favorite: bool,
    pub note: Option<String>,
    /// Count of `send` txs to this address (recency/sort signal).
    pub tx_count: u32,
    /// Epoch ms of the most recent send.
    pub last_used_ms: f64,
    pub first_seen_ms: f64,
    pub source: ContactSource,
}

/// A named set of contacts (e.g. "Payroll") — a first-class registry object,
/// so it can be empty, renamed and reordered independently of its members.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactGroup {
    /// `grp_{n}` — deterministically generated (see [`next_group_id`]), never
    /// clock- or random-derived, so it survives a cold reload (invariant ⑥).
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    /// Lowercased member addresses, in display order.
    pub members: Vec<String>,
}

/// address → epoch ms it was deleted. A history-derived suggestion is
/// suppressed unless the user has transacted with it *since* the deletion.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactTombstone {
    pub address: String,
    pub dismissed_at_ms: f64,
}

/// The `LocalTransaction.type` union, as the shell maps history rows in.
/// `None` on [`ContactHistoryTx::kind`] is a legacy untyped record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactTxKind {
    Send,
    Receive,
    DappTx,
    SignMessage,
    SignTypedData,
    Connect,
}

/// One row of local send history — only the fields the book reads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactHistoryTx {
    /// `None` = a legacy record written before `type` existed. Ported
    /// verbatim: legacy rows do NOT become suggestions (`t.type !== 'send'`,
    /// contacts.ts:294) but DO count as prior interaction
    /// (`t.type === undefined`, recipient-risk.ts:64).
    pub kind: Option<ContactTxKind>,
    pub to: Option<String>,
    /// Resolved identity name captured at send time (`toName`).
    pub to_name: Option<String>,
    /// `t.timestamp ?? 0` — absent stamps derive at epoch 0, verbatim
    /// (contacts.ts:297).
    pub timestamp_ms: Option<f64>,
}

/// A resolved recipient identity (ENS / Basename / passkey).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactIdentity {
    pub name: String,
    /// Source label ("passkey" / "ENS" / ".bnb" …) — the shell owns the words.
    pub source: String,
}

/// `SaveContactInput`. `Some` overwrites the stored field, `None` keeps it —
/// the serde-visible reading of the TS object spread (an absent key preserves;
/// callers that clear a field send an empty string, which every TS read site
/// already treats as missing).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactSaveInput {
    pub address: String,
    pub name: Option<String>,
    pub note: Option<String>,
    pub favorite: Option<bool>,
    pub kind: Option<ContactKind>,
    pub resolved_name: Option<String>,
    pub resolved_source: Option<String>,
}

/// `SaveGroupInput` — omit `id` to create; pass an existing id to update in
/// place. When updating, `members: None` leaves the membership untouched.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactGroupInput {
    pub id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub members: Option<Vec<String>>,
}

/// One parsed row of an import file (`ExportedContact`). Parsing (JSON/CSV
/// detection, quoting, header mapping) stays in the shell; the core receives
/// the normalized rows and applies the import *policy*.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactImportEntry {
    pub address: String,
    pub name: Option<String>,
    pub note: Option<String>,
    pub favorite: Option<bool>,
}

/// A group in a backup — members are addresses (not ids), so import maps by
/// name (`ExportedGroup`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactImportGroup {
    pub name: String,
    pub color: Option<String>,
    pub members: Vec<String>,
}

/// The import outcome the shell shows ("2 added, 1 already existed").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactImportReport {
    /// New addresses saved.
    pub added: u32,
    /// Rows skipped because the address already exists (existing-wins) or
    /// repeats in the file.
    pub skipped: u32,
    /// Rows dropped for a malformed address — counted, never stored
    /// (invariant ⑤).
    pub invalid: u32,
    /// New groups created from the import.
    pub groups_created: u32,
}

/// The two file formats the book travels in (`contacts_io.rs`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactFileFormat {
    Json,
    Csv,
}

/// What an export covers: the whole saved book, or one group with its members.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactExportScope {
    All,
    Group { id: String },
}

/// A file the core has written and the shell hands to the person — a download
/// on the web, a save panel on the desktop, a share sheet on a phone. One-shot:
/// it sits in the view until [`Event::ExportTaken`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactExportFile {
    pub filename: String,
    pub mime: String,
    pub content: String,
    /// How many contacts the file carries — the shell's confirmation line.
    pub contacts: u32,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Storage keys (`vela.contacts`,
/// `vela.contacts.dismissed`, `vela.contactGroups`), RPC transports and the
/// name-service waterfall all live behind these sentences.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactOperation {
    /// Read all three stores at once. Unreadable/corrupt answers as empty,
    /// exactly as the TS loaders' `catch { [] }` does.
    ReadStore,
    /// Best-effort persist (the shell swallows storage errors, matching
    /// today's `persist*`; the in-memory ledger stays authoritative).
    WriteContacts {
        contacts: Vec<Contact>,
    },
    WriteDismissed {
        tombstones: Vec<ContactTombstone>,
    },
    WriteGroups {
        groups: Vec<ContactGroup>,
    },
    /// The current account's local transaction history.
    LoadSendHistory,
    /// Best-effort identity lookup (passkey index → name services).
    ResolveIdentity {
        address: String,
    },
    /// `eth_getCode` for the recipient. The shell returns the raw code so the
    /// core owns both projections (contact kind + risk badge).
    ClassifyRecipient {
        chain_id: u32,
        address: String,
    },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ContactShellResult {
    StoreLoaded {
        contacts: Vec<Contact>,
        tombstones: Vec<ContactTombstone>,
        groups: Vec<ContactGroup>,
    },
    HistoryLoaded {
        txs: Vec<ContactHistoryTx>,
    },
    /// `loadTransactions` threw — derivation yields no suggestions
    /// (contacts.ts:286-290).
    HistoryFailed,
    /// A best-effort write acknowledged. Never changes state.
    Written,
    /// `None` = no identity anywhere, or the lookup failed. Only `Some` is
    /// ever cached (recipient-identity.ts:232-267, invariant ⑦).
    IdentityResolved {
        address: String,
        identity: Option<ContactIdentity>,
    },
    /// `code: None` = RPC error/unreachable — unknown, NOT a verdict, and
    /// never cached (recipient-risk.ts:53-55; contacts.ts:437-442).
    RecipientClassified {
        chain_id: u32,
        address: String,
        code: Option<String>,
    },
}

impl Operation for ContactOperation {
    type Output = ContactShellResult;
}

#[effect]
pub enum ContactEffect {
    Render(RenderOperation),
    Shell(ContactOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ContactEvent"))]
pub enum Event {
    /// Session start AND account switch — the event-driven replacement for the
    /// implicit `clearContactsCache()`. Clears the whole ledger (including
    /// identity/classification caches) and reloads, so a previous account's
    /// history can never bleed into the new book (integration note: miss this
    /// event and the books cross accounts).
    AccountSwitched {
        my_address: Option<String>,
    },
    /// The local tx store changed (a send landed) — refresh the derivation
    /// source. TS re-read history on every `getAllContacts` call; the core is
    /// told instead.
    HistoryChanged,
    /// Create or update a saved contact (idempotent on address). `now_ms`
    /// rides on the event (the 016 now-from-shell pattern): the core has no
    /// clock, the shell stamps dispatch time.
    Save {
        input: ContactSaveInput,
        now_ms: f64,
    },
    /// Delete a saved contact: tombstone it and cascade it out of every group.
    Delete {
        address: String,
        now_ms: f64,
    },
    /// Flip a saved contact's star — or promote an unsaved suggestion to a
    /// starred saved contact (contacts.ts:238-245).
    ToggleFavorite {
        address: String,
        now_ms: f64,
    },
    GroupSave {
        input: ContactGroupInput,
    },
    GroupDelete {
        id: String,
    },
    SetGroupMembers {
        id: String,
        members: Vec<String>,
    },
    /// An import file, already parsed by the shell. The core applies the
    /// existing-wins policy and reports counts (contact-io.ts:203-245).
    ImportParsed {
        contacts: Vec<ContactImportEntry>,
        groups: Vec<ContactImportGroup>,
        now_ms: f64,
    },
    /// A recipient came on screen (Send entry, confirm row, trust line) —
    /// resolve identity + classification for it and project the trust view.
    InspectRecipient {
        chain_id: u32,
        address: String,
    },
    /// An import FILE as the person picked it — its text and its name. The
    /// core sniffs JSON from CSV, parses (refusing a bad file before any write,
    /// D50), applies the existing-wins policy, and — with `into_group` — seats
    /// every valid row in that group ("导入到本组"). The outcome is
    /// `ContactsView::last_import` or `import_failure`, until acknowledged.
    ImportFile {
        content: String,
        filename: Option<String>,
        into_group: Option<String>,
        now_ms: f64,
    },
    /// The report (or refusal) was shown; the view's line clears.
    ImportAcknowledged,
    /// Write the book — or one group — into a file for the shell to hand
    /// over. `exported_at_iso` is the shell's clock (the 016 now-from-shell
    /// pattern): it stamps the backup and dates the filename.
    ExportRequested {
        scope: ContactExportScope,
        format: ContactFileFormat,
        exported_at_iso: String,
    },
    /// The shell handed the file over; the one-shot leaves the view.
    ExportTaken,
    /// Seat more members in a group — a union, normalised, never a duplicate
    /// (the "添加成员" picker's answer; `SetGroupMembers` replaces instead).
    AddGroupMembers {
        id: String,
        members: Vec<String>,
    },
    RemoveGroupMember {
        id: String,
        address: String,
    },
    /// Which groups hold one contact — the "移入分组" picker's whole answer:
    /// the contact joins every listed group and leaves every other.
    SetContactGroups {
        address: String,
        group_ids: Vec<String>,
    },
    /// Internal: an effect resolved. `attempt` is captured when the request
    /// is made; a result carrying an older attempt belongs to a previous
    /// account's session and is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: ContactShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// What one successful `eth_getCode` pinned down. Both projections of the
/// same bytes: `kind` for the address book (contacts.ts:438 — no EIP-7702
/// carve-out, ported verbatim: a delegated EOA reads as a smart account
/// there), `is_contract` for the risk badge (recipient-risk.ts:42-51 — WITH
/// the carve-out: a delegated EOA is a wallet, never badged "Contract").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct CodeVerdict {
    kind: ContactKind,
    is_contract: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Inspected {
    chain_id: u32,
    address: String,
}

#[derive(Default)]
pub struct Model {
    /// The three stores have been read. Mutations before that are dropped —
    /// the TS equivalent was every mutator `await`ing its lazy load first, so
    /// no operation could ever act on an unloaded book.
    loaded: bool,
    my_address: Option<String>,
    saved: Vec<Contact>,
    tombstones: Vec<ContactTombstone>,
    groups: Vec<ContactGroup>,
    history: Vec<ContactHistoryTx>,
    /// Positive-only identity cache (recipient-identity.ts caches only
    /// resolutions; a `None` is re-asked next time — invariant ⑦).
    identities: BTreeMap<String, ContactIdentity>,
    /// Keyed `chain_id:addr` (recipient-risk.ts:26) — the stricter of the two
    /// TS caches; contacts.ts's chain-agnostic `kindCache` is superseded.
    verdicts: BTreeMap<(u32, String), CodeVerdict>,
    /// In-flight dedupe — the core-side replacement for the TS hook's
    /// module-level inflight merge.
    inflight_identity: BTreeSet<String>,
    inflight_classify: BTreeSet<(u32, String)>,
    inspected: Option<Inspected>,
    last_import: Option<ContactImportReport>,
    import_failure: Option<ContactImportFailure>,
    export: Option<ContactExportFile>,
    /// Bumped on every account switch; stale results are dropped by it.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// A group with its members resolved to contacts, in membership order. A
/// member without a saved contact is synthesised as a minimal `auto` entry so
/// send-to-group never silently drops a payee (contacts.ts:396-410,
/// invariant ③).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactGroupView {
    pub id: String,
    pub name: String,
    pub color: Option<String>,
    pub members: Vec<Contact>,
}

/// The trust line for the recipient currently on screen.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactRecipientView {
    /// Lowercased.
    pub address: String,
    pub saved: bool,
    /// Saved **and** starred — the only state that earns the green check
    /// (`RecipientTrust.tsx:5-8`); a poisoned look-alike address is never a
    /// starred contact.
    pub verified: bool,
    /// Contact name → contact resolved name → live identity → `None` (the
    /// shell falls back to a short address / generic label).
    pub display_name: Option<String>,
    /// The live identity, for the source tag ("Vela User" / "ENS").
    pub identity: Option<ContactIdentity>,
    pub kind: ContactKind,
    /// `Some(true)` = bytecode present; `Some(false)` = EOA (including an
    /// EIP-7702 delegated EOA); `None` = unknown/unreachable — never a false
    /// alarm (invariant ⑦).
    pub is_contract: Option<bool>,
    /// No prior outgoing send/dapp-tx to this address in local history — the
    /// address-poisoning tell (recipient-risk.ts:59-69).
    pub first_interaction: bool,
}

/// One letter of the A–Z directory (spec 028 US5 addendum): the addresses
/// of every contact in the book that files under `letter`, in the BOOK's
/// order — favourites first, then most-recent — so inside one letter the
/// person you pay most is still first. Sections run A–Z with `#` last.
///
/// Sections cover the WHOLE book. A shell that narrows the list by a search
/// box (the core has no list-search event; `matches_query` serves the
/// recipient picker) looks its surviving rows up here and drops the letters
/// that come out empty — an empty letter header is the shell's to avoid.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactSection {
    /// `"A"`..`"Z"`, or `"#"`.
    pub letter: String,
    /// Lowercased addresses, keys into `ContactsView::contacts`.
    pub addresses: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ContactsView {
    pub loaded: bool,
    /// The unified book: saved ⊕ history-derived, tombstone-suppressed,
    /// sorted favourites-first then most-recent.
    pub contacts: Vec<Contact>,
    /// The same book as an A–Z directory, by the core's one initial rule
    /// (`contacts_initials.rs`: 阿豪 → A, 妈妈 → M, an address → `#`).
    pub sections: Vec<ContactSection>,
    pub groups: Vec<ContactGroupView>,
    pub last_import: Option<ContactImportReport>,
    /// Why the last `ImportFile` was refused — before anything was written.
    /// Mutually exclusive with `last_import`; both clear on `ImportAcknowledged`.
    pub import_failure: Option<ContactImportFailure>,
    /// The file an `ExportRequested` produced, until the shell takes it.
    pub export: Option<ContactExportFile>,
    pub recipient: Option<ContactRecipientView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Contacts;

impl App for Contacts {
    type Event = Event;
    type Model = Model;
    type ViewModel = ContactsView;
    type Effect = ContactEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<ContactEffect, Event> {
        match event {
            Event::AccountSwitched { my_address } => {
                // Everything goes: saved/tombstones/groups reload from storage,
                // history/identity/classification are per-account state that
                // must never survive a switch.
                let attempt = model.attempt + 1;
                *model = Model {
                    attempt,
                    my_address: my_address.map(|a| a.to_lowercase()),
                    ..Model::default()
                };
                requests(
                    model,
                    vec![
                        ContactOperation::ReadStore,
                        ContactOperation::LoadSendHistory,
                    ],
                )
            }
            Event::HistoryChanged => requests(model, vec![ContactOperation::LoadSendHistory]),
            Event::Save { input, now_ms } => {
                if !model.loaded {
                    return Command::done();
                }
                let outcome = apply_save(model, input, now_ms);
                let mut ops = vec![ContactOperation::WriteContacts {
                    contacts: model.saved.clone(),
                }];
                if outcome.tombstone_cleared {
                    ops.push(ContactOperation::WriteDismissed {
                        tombstones: model.tombstones.clone(),
                    });
                }
                requests(model, ops)
            }
            Event::Delete { address, now_ms } => {
                if !model.loaded {
                    return Command::done();
                }
                let addr = address.to_lowercase();
                model.saved.retain(|c| c.address != addr);
                upsert_tombstone(&mut model.tombstones, &addr, now_ms);
                let mut ops = vec![
                    ContactOperation::WriteContacts {
                        contacts: model.saved.clone(),
                    },
                    ContactOperation::WriteDismissed {
                        tombstones: model.tombstones.clone(),
                    },
                ];
                // Cascade: drop the address from every group so a member never
                // dangles (contacts.ts:219-226, invariant ③). Groups are only
                // written when one actually held the address, as today.
                if model.groups.iter().any(|g| g.members.contains(&addr)) {
                    for group in &mut model.groups {
                        group.members.retain(|m| *m != addr);
                    }
                    ops.push(ContactOperation::WriteGroups {
                        groups: model.groups.clone(),
                    });
                }
                requests(model, ops)
            }
            Event::ToggleFavorite { address, now_ms } => {
                if !model.loaded {
                    return Command::done();
                }
                let addr = address.to_lowercase();
                let existing = model.saved.iter_mut().find(|c| c.address == addr);
                match existing {
                    // A saved contact flips in place (an `updateContact` patch:
                    // no reorder, no tombstone change — contacts.ts:229-244).
                    Some(contact) => {
                        contact.favorite = !contact.favorite;
                        requests(
                            model,
                            vec![ContactOperation::WriteContacts {
                                contacts: model.saved.clone(),
                            }],
                        )
                    }
                    // An unsaved suggestion is promoted to a starred saved
                    // contact via the full save path.
                    None => {
                        let outcome = apply_save(
                            model,
                            ContactSaveInput {
                                address: addr,
                                name: None,
                                note: None,
                                favorite: Some(true),
                                kind: None,
                                resolved_name: None,
                                resolved_source: None,
                            },
                            now_ms,
                        );
                        let mut ops = vec![ContactOperation::WriteContacts {
                            contacts: model.saved.clone(),
                        }];
                        if outcome.tombstone_cleared {
                            ops.push(ContactOperation::WriteDismissed {
                                tombstones: model.tombstones.clone(),
                            });
                        }
                        requests(model, ops)
                    }
                }
            }
            Event::GroupSave { input } => {
                if !model.loaded {
                    return Command::done();
                }
                apply_group_save(&mut model.groups, input);
                requests(
                    model,
                    vec![ContactOperation::WriteGroups {
                        groups: model.groups.clone(),
                    }],
                )
            }
            Event::GroupDelete { id } => {
                if !model.loaded {
                    return Command::done();
                }
                // Deleting a group never touches the contacts (contacts.ts:362-366).
                model.groups.retain(|g| g.id != id);
                requests(
                    model,
                    vec![ContactOperation::WriteGroups {
                        groups: model.groups.clone(),
                    }],
                )
            }
            Event::SetGroupMembers { id, members } => {
                if !model.loaded {
                    return Command::done();
                }
                let members = normalize_members(&members);
                for group in &mut model.groups {
                    if group.id == id {
                        group.members = members.clone();
                    }
                }
                requests(
                    model,
                    vec![ContactOperation::WriteGroups {
                        groups: model.groups.clone(),
                    }],
                )
            }
            Event::ImportParsed {
                contacts,
                groups,
                now_ms,
            } => {
                if !model.loaded {
                    return Command::done();
                }
                model.import_failure = None;
                let ops = apply_import(model, contacts, groups, now_ms);
                if ops.is_empty() {
                    // Nothing was added and no group changed — no writes, as
                    // today (the report still renders).
                    return render();
                }
                requests(model, ops)
            }
            Event::ImportFile {
                content,
                filename,
                into_group,
                now_ms,
            } => {
                if !model.loaded {
                    return Command::done();
                }
                match apply_import_file(
                    model,
                    &content,
                    filename.as_deref(),
                    into_group.as_deref(),
                    now_ms,
                ) {
                    Ok(ops) if !ops.is_empty() => requests(model, ops),
                    // Nothing written: a refusal (said in `import_failure`) or
                    // an import with nothing new (said in `last_import`).
                    _ => render(),
                }
            }
            Event::ImportAcknowledged => {
                model.last_import = None;
                model.import_failure = None;
                render()
            }
            Event::ExportRequested {
                scope,
                format,
                exported_at_iso,
            } => {
                if !model.loaded {
                    return Command::done();
                }
                model.export = export_book(model, &scope, format, &exported_at_iso);
                render()
            }
            Event::ExportTaken => {
                model.export = None;
                render()
            }
            Event::AddGroupMembers { id, members } => {
                if !model.loaded {
                    return Command::done();
                }
                let joining = normalize_members(&members);
                let changed = match model.groups.iter_mut().find(|g| g.id == id) {
                    Some(group) => union_members(&mut group.members, joining),
                    None => false,
                };
                write_groups_if(model, changed)
            }
            Event::RemoveGroupMember { id, address } => {
                if !model.loaded {
                    return Command::done();
                }
                let addr = address.to_lowercase();
                let changed = match model.groups.iter_mut().find(|g| g.id == id) {
                    Some(group) => {
                        let before = group.members.len();
                        group.members.retain(|m| *m != addr);
                        group.members.len() != before
                    }
                    None => false,
                };
                write_groups_if(model, changed)
            }
            Event::SetContactGroups { address, group_ids } => {
                if !model.loaded {
                    return Command::done();
                }
                let changed = set_contact_groups(&mut model.groups, &address, &group_ids);
                write_groups_if(model, changed)
            }
            Event::InspectRecipient { chain_id, address } => {
                let addr = address.to_lowercase();
                model.inspected = Some(Inspected {
                    chain_id,
                    address: addr.clone(),
                });
                // Malformed → no lookups, `is_contract: None` + not-first
                // shape (recipient-risk.ts:76-78); the zero address is a
                // mint/burn counterparty with no identity and would 404 the
                // passkey index (recipient-identity.ts:233-236).
                if !is_address(&addr) {
                    return render();
                }
                let mut ops = Vec::new();
                let class_key = (chain_id, addr.clone());
                if !model.verdicts.contains_key(&class_key)
                    && !model.inflight_classify.contains(&class_key)
                {
                    model.inflight_classify.insert(class_key);
                    ops.push(ContactOperation::ClassifyRecipient {
                        chain_id,
                        address: addr.clone(),
                    });
                }
                if !is_zero_address(&addr)
                    && !model.identities.contains_key(&addr)
                    && !model.inflight_identity.contains(&addr)
                {
                    model.inflight_identity.insert(addr.clone());
                    ops.push(ContactOperation::ResolveIdentity { address: addr });
                }
                if ops.is_empty() {
                    return render();
                }
                requests(model, ops)
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A previous account's read/lookup landing after a switch.
                    // Dropping it IS the "never cross account books" rule.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> ContactsView {
        let merged = merge_contacts(
            &model.saved,
            &model.tombstones,
            &model.history,
            model.my_address.as_deref(),
        );
        let contacts = sort_contacts(merged);
        ContactsView {
            loaded: model.loaded,
            sections: section_contacts(&contacts),
            contacts,
            groups: model
                .groups
                .iter()
                .map(|group| ContactGroupView {
                    id: group.id.clone(),
                    name: group.name.clone(),
                    color: group.color.clone(),
                    members: group
                        .members
                        .iter()
                        .map(|addr| resolve_member(&model.saved, addr))
                        .collect(),
                })
                .collect(),
            last_import: model.last_import,
            import_failure: model.import_failure,
            export: model.export.clone(),
            recipient: model
                .inspected
                .as_ref()
                .map(|target| project_recipient(model, target)),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: ContactShellResult) -> Command<ContactEffect, Event> {
    match result {
        ContactShellResult::StoreLoaded {
            contacts,
            tombstones,
            groups,
        } => {
            model.saved = contacts;
            model.tombstones = tombstones;
            model.groups = groups;
            model.loaded = true;
            render()
        }
        ContactShellResult::HistoryLoaded { txs } => {
            model.history = txs;
            render()
        }
        ContactShellResult::HistoryFailed => {
            // `loadTransactions` threw → no suggestions (contacts.ts:286-290).
            model.history = Vec::new();
            render()
        }
        ContactShellResult::IdentityResolved { address, identity } => {
            let addr = address.to_lowercase();
            model.inflight_identity.remove(&addr);
            let Some(identity) = identity else {
                // Negative/failed lookups are never cached — a later inspect
                // asks again (recipient-identity.ts caches positives only).
                return Command::done();
            };
            model.identities.insert(addr.clone(), identity.clone());
            // Write-back: a saved-but-unnamed contact adopts the resolved name
            // so the picker and future renders show it (RecipientTrust.tsx:78-84).
            let written = match model.saved.iter_mut().find(|c| c.address == addr) {
                Some(contact) if contact_display_name(contact).is_empty() => {
                    contact.resolved_name = Some(identity.name);
                    contact.resolved_source = Some(identity.source);
                    true
                }
                _ => false,
            };
            if written {
                requests(
                    model,
                    vec![ContactOperation::WriteContacts {
                        contacts: model.saved.clone(),
                    }],
                )
            } else {
                render()
            }
        }
        ContactShellResult::RecipientClassified {
            chain_id,
            address,
            code,
        } => {
            let addr = address.to_lowercase();
            model.inflight_classify.remove(&(chain_id, addr.clone()));
            match code {
                // RPC unreachable/errored — unknown, not a verdict; never
                // cached, so the next inspect retries (invariant ⑦).
                None => Command::done(),
                Some(code) => {
                    model
                        .verdicts
                        .insert((chain_id, addr), classify_code(&code));
                    render()
                }
            }
        }
        // A best-effort write acknowledged. Never changes state.
        ContactShellResult::Written => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Mutations
// ---------------------------------------------------------------------------

struct SaveOutcome {
    tombstone_cleared: bool,
}

/// `saveContact` (contacts.ts:183-209): merge-or-create keyed on the
/// lowercased address, move to the head, clear any deletion tombstone.
fn apply_save(model: &mut Model, input: ContactSaveInput, now_ms: f64) -> SaveOutcome {
    let addr = input.address.to_lowercase();
    let merged = match model.saved.iter().find(|c| c.address == addr) {
        Some(existing) => {
            let mut contact = existing.clone();
            if let Some(name) = input.name {
                contact.name = Some(name);
            }
            if let Some(note) = input.note {
                contact.note = Some(note);
            }
            if let Some(favorite) = input.favorite {
                contact.favorite = favorite;
            }
            if let Some(kind) = input.kind {
                contact.kind = kind;
            }
            if let Some(resolved_name) = input.resolved_name {
                contact.resolved_name = Some(resolved_name);
            }
            if let Some(resolved_source) = input.resolved_source {
                contact.resolved_source = Some(resolved_source);
            }
            contact.address = addr.clone();
            contact.source = ContactSource::Manual;
            contact
        }
        None => Contact {
            address: addr.clone(),
            name: input.name,
            resolved_name: input.resolved_name,
            resolved_source: input.resolved_source,
            kind: input.kind.unwrap_or_default(),
            favorite: input.favorite.unwrap_or(false),
            note: input.note,
            tx_count: 0,
            last_used_ms: now_ms,
            first_seen_ms: now_ms,
            source: ContactSource::Manual,
        },
    };
    model.saved.retain(|c| c.address != addr);
    model.saved.insert(0, merged);
    // Re-adding an address clears any prior deletion tombstone.
    let before = model.tombstones.len();
    model.tombstones.retain(|t| t.address != addr);
    SaveOutcome {
        tombstone_cleared: model.tombstones.len() != before,
    }
}

fn upsert_tombstone(tombstones: &mut Vec<ContactTombstone>, addr: &str, now_ms: f64) {
    match tombstones.iter_mut().find(|t| t.address == addr) {
        Some(entry) => entry.dismissed_at_ms = now_ms,
        None => tombstones.push(ContactTombstone {
            address: addr.to_owned(),
            dismissed_at_ms: now_ms,
        }),
    }
}

/// `saveGroup` (contacts.ts:337-360).
fn apply_group_save(groups: &mut Vec<ContactGroup>, input: ContactGroupInput) {
    let existing_index = input
        .id
        .as_ref()
        .and_then(|id| groups.iter().position(|g| g.id == *id));
    match existing_index {
        Some(index) => {
            let Some(group) = groups.get_mut(index) else {
                return;
            };
            // `input.name.trim() || existing.name` — a blank rename keeps the
            // old name.
            let trimmed = input.name.trim();
            if !trimmed.is_empty() {
                group.name = trimmed.to_owned();
            }
            if let Some(color) = input.color {
                group.color = Some(color);
            }
            if let Some(members) = input.members {
                group.members = normalize_members(&members);
            }
        }
        None => {
            let id = input.id.unwrap_or_else(|| next_group_id(groups));
            groups.push(ContactGroup {
                id,
                name: input.name.trim().to_owned(),
                color: input.color,
                members: normalize_members(&input.members.unwrap_or_default()),
            });
        }
    }
}

/// `importContacts` (contact-io.ts:203-245) — existing-wins. Returns the
/// write operations actually needed; sets `model.last_import`.
fn apply_import(
    model: &mut Model,
    contacts: Vec<ContactImportEntry>,
    groups: Vec<ContactImportGroup>,
    now_ms: f64,
) -> Vec<ContactOperation> {
    let mut report = ContactImportReport::default();
    let mut newly_added: BTreeSet<String> = BTreeSet::new();
    let mut tombstone_cleared = false;

    for entry in contacts {
        if !is_address(&entry.address) {
            report.invalid += 1;
            continue;
        }
        let addr = entry.address.to_lowercase();
        // Existing-wins: never overwrite a local contact; a duplicate within
        // the file is added once (invariant ⑤).
        if newly_added.contains(&addr) || model.saved.iter().any(|c| c.address == addr) {
            report.skipped += 1;
            continue;
        }
        let outcome = apply_save(
            model,
            ContactSaveInput {
                address: addr.clone(),
                name: entry.name,
                note: entry.note,
                favorite: entry.favorite,
                kind: None,
                resolved_name: None,
                resolved_source: None,
            },
            now_ms,
        );
        tombstone_cleared |= outcome.tombstone_cleared;
        newly_added.insert(addr);
        report.added += 1;
    }

    let mut groups_changed = false;
    if !groups.is_empty() && !newly_added.is_empty() {
        // Keyed by the file's group name, lowercased UNtrimmed — ported
        // verbatim from contact-io.ts:227-241 (a created group's stored name
        // is trimmed, but lookups use the raw file name).
        let mut by_name: BTreeMap<String, String> = model
            .groups
            .iter()
            .map(|g| (g.name.to_lowercase(), g.id.clone()))
            .collect();
        for group in groups {
            let members_to_add: Vec<String> = group
                .members
                .iter()
                .map(|m| m.to_lowercase())
                .filter(|m| newly_added.contains(m))
                .collect();
            if members_to_add.is_empty() {
                // Nothing new — leave existing groups alone (existing
                // contacts' memberships are never altered).
                continue;
            }
            match by_name.get(&group.name.to_lowercase()) {
                Some(id) => {
                    if let Some(target) = model.groups.iter_mut().find(|g| g.id == *id) {
                        for member in members_to_add {
                            if !target.members.contains(&member) {
                                target.members.push(member);
                            }
                        }
                        groups_changed = true;
                    }
                }
                None => {
                    let key = group.name.to_lowercase();
                    let id = next_group_id(&model.groups);
                    model.groups.push(ContactGroup {
                        id: id.clone(),
                        name: group.name.trim().to_owned(),
                        color: group.color,
                        members: normalize_members(&members_to_add),
                    });
                    by_name.insert(key, id);
                    report.groups_created += 1;
                    groups_changed = true;
                }
            }
        }
    }

    model.last_import = Some(report);

    let mut ops = Vec::new();
    if report.added > 0 {
        ops.push(ContactOperation::WriteContacts {
            contacts: model.saved.clone(),
        });
    }
    if tombstone_cleared {
        ops.push(ContactOperation::WriteDismissed {
            tombstones: model.tombstones.clone(),
        });
    }
    if groups_changed {
        ops.push(ContactOperation::WriteGroups {
            groups: model.groups.clone(),
        });
    }
    ops
}

/// [`Event::ImportFile`]: parse (refusing a bad file before any write, D50),
/// apply the existing-wins policy, then — for "import into this group" — seat
/// every VALID row in that group, whether it was just added or already saved.
/// The person named the group; membership is the one thing an existing entry
/// does not get to veto (its name, note and star still win, as always).
///
/// `Err` carries the refusal already recorded on the model; `Ok` the writes.
fn apply_import_file(
    model: &mut Model,
    content: &str,
    filename: Option<&str>,
    into_group: Option<&str>,
    now_ms: f64,
) -> Result<Vec<ContactOperation>, ContactImportFailure> {
    model.last_import = None;
    model.import_failure = None;
    if let Some(id) = into_group {
        if !model.groups.iter().any(|g| g.id == id) {
            model.import_failure = Some(ContactImportFailure::UnknownGroup);
            return Err(ContactImportFailure::UnknownGroup);
        }
    }
    let parsed = match contacts_io::parse(content, filename) {
        Ok(parsed) => parsed,
        Err(failure) => {
            model.import_failure = Some(failure);
            return Err(failure);
        }
    };
    let joining: Vec<String> = parsed
        .contacts
        .iter()
        .filter(|entry| is_address(&entry.address))
        .map(|entry| entry.address.to_lowercase())
        .collect();
    let mut ops = apply_import(model, parsed.contacts, parsed.groups, now_ms);
    if let Some(id) = into_group {
        let seated = match model.groups.iter_mut().find(|g| g.id == id) {
            Some(group) => union_members(&mut group.members, normalize_members(&joining)),
            None => false,
        };
        if seated {
            // The snapshot `apply_import` took predates the seating.
            ops.retain(|op| !matches!(op, ContactOperation::WriteGroups { .. }));
            ops.push(ContactOperation::WriteGroups {
                groups: model.groups.clone(),
            });
        }
    }
    Ok(ops)
}

/// [`Event::ExportRequested`]: the saved book, or one group resolved to its
/// members (an unsaved member is still a payee — it travels as its address,
/// invariant ③). A group that no longer exists exports nothing.
fn export_book(
    model: &Model,
    scope: &ContactExportScope,
    format: ContactFileFormat,
    exported_at_iso: &str,
) -> Option<ContactExportFile> {
    let (contacts, groups, group_name): (Vec<Contact>, Vec<ContactGroup>, Option<String>) =
        match scope {
            ContactExportScope::All => (model.saved.clone(), model.groups.clone(), None),
            ContactExportScope::Group { id } => {
                let group = model.groups.iter().find(|g| g.id == *id)?;
                let members = group
                    .members
                    .iter()
                    .map(|addr| resolve_member(&model.saved, addr))
                    .collect();
                (members, vec![group.clone()], Some(group.name.clone()))
            }
        };
    let content = match format {
        ContactFileFormat::Json => contacts_io::to_json(&contacts, &groups, exported_at_iso),
        ContactFileFormat::Csv => contacts_io::to_csv(&contacts, &groups),
    };
    Some(ContactExportFile {
        filename: contacts_io::export_filename(group_name.as_deref(), format, exported_at_iso),
        mime: contacts_io::mime_for(format).to_owned(),
        content,
        contacts: u32::try_from(contacts.len()).unwrap_or(u32::MAX),
    })
}

/// Append every `incoming` member not already seated. `incoming` is expected
/// normalised (lowercased, valid, de-duplicated). Answers whether anything changed.
fn union_members(target: &mut Vec<String>, incoming: Vec<String>) -> bool {
    let mut changed = false;
    for member in incoming {
        if !target.contains(&member) {
            target.push(member);
            changed = true;
        }
    }
    changed
}

/// [`Event::SetContactGroups`]: one contact's whole membership at once. A
/// malformed address changes nothing. Answers whether anything changed.
fn set_contact_groups(groups: &mut [ContactGroup], address: &str, group_ids: &[String]) -> bool {
    if !is_address(address) {
        return false;
    }
    let addr = address.to_lowercase();
    let mut changed = false;
    for group in groups.iter_mut() {
        let wanted = group_ids.contains(&group.id);
        let seated = group.members.contains(&addr);
        if wanted && !seated {
            group.members.push(addr.clone());
            changed = true;
        } else if !wanted && seated {
            group.members.retain(|m| *m != addr);
            changed = true;
        }
    }
    changed
}

/// A group edit that changed nothing writes nothing — only renders.
fn write_groups_if(model: &Model, changed: bool) -> Command<ContactEffect, Event> {
    if changed {
        requests(
            model,
            vec![ContactOperation::WriteGroups {
                groups: model.groups.clone(),
            }],
        )
    } else {
        render()
    }
}

// ---------------------------------------------------------------------------
// Pure derivation — the unified book (contacts.ts:258-317)
// ---------------------------------------------------------------------------

/// Saved contacts merged with live suggestions from send history. Saved
/// entries win on identity/name; recency and count are refreshed from history
/// so a saved contact still sorts by recent use. A tombstoned suggestion is
/// suppressed unless it has been used since deletion (invariant ②).
fn merge_contacts(
    saved: &[Contact],
    tombstones: &[ContactTombstone],
    history: &[ContactHistoryTx],
    my_address: Option<&str>,
) -> Vec<Contact> {
    let mut merged: Vec<Contact> = saved.to_vec();
    for auto in derive_from_history(history, my_address) {
        match merged.iter_mut().find(|c| c.address == auto.address) {
            Some(existing) => {
                existing.tx_count = existing.tx_count.max(auto.tx_count);
                existing.last_used_ms = existing.last_used_ms.max(auto.last_used_ms);
                if is_blank(&existing.resolved_name) && !is_blank(&auto.resolved_name) {
                    existing.resolved_name = auto.resolved_name;
                }
            }
            None => {
                // Suppress a deleted recipient unless it's been used since
                // deletion (`lastUsed <= dismissedAt` keeps it buried).
                if let Some(dismissed_at) = tombstones
                    .iter()
                    .find(|t| t.address == auto.address)
                    .map(|t| t.dismissed_at_ms)
                {
                    if auto.last_used_ms <= dismissed_at {
                        continue;
                    }
                }
                merged.push(auto);
            }
        }
    }
    merged
}

/// Recipients from `send` history, one per address, with counts + recency
/// (contacts.ts:283-317). ONLY `type: 'send'` rows — never dApp contract
/// calls, never receives, and (verbatim) never legacy untyped rows — so a
/// router/token address can never become a trusted-looking suggestion
/// (invariant ①).
fn derive_from_history(history: &[ContactHistoryTx], my_address: Option<&str>) -> Vec<Contact> {
    let me = my_address.map(str::to_lowercase);
    let mut out: Vec<Contact> = Vec::new();
    for tx in history {
        if tx.kind != Some(ContactTxKind::Send) {
            continue;
        }
        let Some(to) = tx.to.as_ref().map(|t| t.to_lowercase()) else {
            continue;
        };
        if to.is_empty() || !is_address(&to) || Some(&to) == me.as_ref() {
            continue;
        }
        let ts = tx.timestamp_ms.unwrap_or(0.0);
        match out.iter_mut().find(|c| c.address == to) {
            Some(existing) => {
                existing.tx_count += 1;
                existing.last_used_ms = existing.last_used_ms.max(ts);
                existing.first_seen_ms = existing.first_seen_ms.min(ts);
                if is_blank(&existing.resolved_name) && !is_blank(&tx.to_name) {
                    existing.resolved_name = tx.to_name.clone();
                }
            }
            None => out.push(Contact {
                address: to,
                name: None,
                resolved_name: tx.to_name.clone(),
                resolved_source: None,
                kind: ContactKind::Unknown,
                favorite: false,
                note: None,
                tx_count: 1,
                last_used_ms: ts,
                first_seen_ms: ts,
                source: ContactSource::Auto,
            }),
        }
    }
    out
}

/// `getGroupMembers`' resolution rule: a saved contact carries its name/kind,
/// an unsaved member becomes a minimal `auto` contact — never dropped.
fn resolve_member(saved: &[Contact], addr: &str) -> Contact {
    saved
        .iter()
        .find(|c| c.address == addr)
        .cloned()
        .unwrap_or(Contact {
            address: addr.to_owned(),
            name: None,
            resolved_name: None,
            resolved_source: None,
            kind: ContactKind::Unknown,
            favorite: false,
            note: None,
            tx_count: 0,
            last_used_ms: 0.0,
            first_seen_ms: 0.0,
            source: ContactSource::Auto,
        })
}

fn project_recipient(model: &Model, target: &Inspected) -> ContactRecipientView {
    let addr = &target.address;
    let valid = is_address(addr);
    let saved = if valid {
        model.saved.iter().find(|c| c.address == *addr)
    } else {
        None
    };
    let identity = model.identities.get(addr);
    let verdict = model.verdicts.get(&(target.chain_id, addr.clone()));
    let saved_display = saved
        .map(contact_display_name)
        .filter(|name| !name.is_empty());
    ContactRecipientView {
        address: addr.clone(),
        saved: saved.is_some(),
        verified: saved.is_some_and(|c| c.favorite),
        display_name: saved_display.or_else(|| identity.map(|id| id.name.clone())),
        identity: identity.cloned(),
        kind: verdict
            .map(|v| v.kind)
            .or_else(|| saved.map(|c| c.kind))
            .unwrap_or_default(),
        is_contract: verdict.map(|v| v.is_contract),
        first_interaction: valid && !has_prior_interaction(&model.history, addr),
    }
}

/// Any prior *outgoing* tx to this address — send, dApp tx, or a legacy
/// untyped record (recipient-risk.ts:59-69, verbatim including the legacy
/// clause that derivation deliberately lacks).
fn has_prior_interaction(history: &[ContactHistoryTx], addr: &str) -> bool {
    history.iter().any(|tx| {
        tx.to
            .as_deref()
            .is_some_and(|to| to.eq_ignore_ascii_case(addr))
            && matches!(
                tx.kind,
                Some(ContactTxKind::Send) | Some(ContactTxKind::DappTx) | None
            )
    })
}

// ---------------------------------------------------------------------------
// Pure helpers (pub where the TS original was an exported pure function)
// ---------------------------------------------------------------------------

/// `ADDRESS_RE` — `^0x[0-9a-fA-F]{40}$` (`src/models/types.ts:383`).
pub fn is_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 42
        && bytes[0] == b'0'
        && bytes[1] == b'x'
        && bytes[2..].iter().all(u8::is_ascii_hexdigit)
}

/// `^0x0{40}$` — the mint/burn counterparty (recipient-identity.ts:236).
fn is_zero_address(s: &str) -> bool {
    let bytes = s.as_bytes();
    bytes.len() == 42
        && bytes[0] == b'0'
        && bytes[1] == b'x'
        && bytes[2..].iter().all(|b| *b == b'0')
}

/// EIP-7702 delegation designator: `0xef0100 ++ implAddr`, exactly 23 bytes
/// (`^0xef0100[0-9a-fA-F]{40}$` case-insensitive, recipient-risk.ts:46). A
/// delegated EOA is a person's wallet with smart-account features — Vela's
/// own accounts delegate — and must NOT be badged "Contract" (invariant ⑦).
fn is_eip7702_delegation(code: &str) -> bool {
    let bytes = code.as_bytes();
    bytes.len() == 48
        && bytes[..8].eq_ignore_ascii_case(b"0xef0100")
        && bytes[8..].iter().all(u8::is_ascii_hexdigit)
}

/// Both projections of one `eth_getCode` answer, each ported verbatim from
/// its own TS source:
/// - `kind` — contacts.ts:438, which has NO EIP-7702 branch: a delegated EOA
///   classifies as a smart `account` in the address book (it IS a smart
///   account; ported verbatim, see inventory open questions).
/// - `is_contract` — recipient-risk.ts:42-51, WITH the carve-out: the risk
///   badge never calls a delegated EOA a contract.
fn classify_code(code: &str) -> CodeVerdict {
    let has_code = code != "0x" && code.len() > 2;
    CodeVerdict {
        kind: if has_code {
            ContactKind::Account
        } else {
            ContactKind::Eoa
        },
        is_contract: if is_eip7702_delegation(code) {
            false
        } else {
            has_code
        },
    }
}

/// Lowercase, drop invalid, de-dupe (first wins) — the canonical member shape
/// (contacts.ts:143-155, invariant ③).
fn normalize_members(addrs: &[String]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for addr in addrs {
        if !is_address(addr) {
            continue;
        }
        let low = addr.to_lowercase();
        if seen.contains(&low) {
            continue;
        }
        seen.insert(low.clone());
        out.push(low);
    }
    out
}

/// Next stable, collision-free group id: one past the largest numeric suffix.
/// Deterministic — no clock, no randomness — so it survives a cold reload and
/// a process restart (contacts.ts:157-165, invariant ⑥).
fn next_group_id(groups: &[ContactGroup]) -> String {
    let mut max: i64 = 0;
    for group in groups {
        let tail = group.id.strip_prefix("grp_").unwrap_or(&group.id);
        if let Some(n) = js_parse_int(tail) {
            if n > max {
                max = n;
            }
        }
    }
    format!("grp_{}", max + 1)
}

/// `parseInt(s, 10)`-alike: optional sign, longest leading digit run, `None`
/// where JS gives `NaN` (overflow also answers `None`; group counters never
/// get near it).
fn js_parse_int(s: &str) -> Option<i64> {
    let t = s.trim_start();
    let (negative, digits) = match t.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, t.strip_prefix('+').unwrap_or(t)),
    };
    let end = digits
        .bytes()
        .position(|b| !b.is_ascii_digit())
        .unwrap_or(digits.len());
    let run = digits.get(..end).unwrap_or("");
    if run.is_empty() {
        return None;
    }
    let value = run.parse::<i64>().ok()?;
    Some(if negative { -value } else { value })
}

fn is_blank(value: &Option<String>) -> bool {
    value.as_deref().is_none_or(str::is_empty)
}

/// User name → resolved identity → `""` (caller falls back to a short
/// address). Empty strings count as missing, exactly as TS falsiness does.
pub fn contact_display_name(contact: &Contact) -> String {
    contact
        .name
        .as_deref()
        .filter(|n| !n.is_empty())
        .or_else(|| contact.resolved_name.as_deref().filter(|n| !n.is_empty()))
        .unwrap_or("")
        .to_owned()
}

/// Sort: favourites first, then most-recently-used, then name/address.
/// The TS name tie-break is `localeCompare`; the core uses a deterministic
/// case-insensitive-then-exact comparison — locale collation is presentation
/// and would drag ICU into the wasm (only ties on equal star AND equal
/// `last_used_ms` can tell the difference).
pub fn sort_contacts(mut list: Vec<Contact>) -> Vec<Contact> {
    list.sort_by(|a, b| {
        b.favorite
            .cmp(&a.favorite)
            .then_with(|| {
                b.last_used_ms
                    .partial_cmp(&a.last_used_ms)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .then_with(|| {
                let (name_a, name_b) = (contact_display_name(a), contact_display_name(b));
                name_a
                    .to_lowercase()
                    .cmp(&name_b.to_lowercase())
                    .then_with(|| name_a.cmp(&name_b))
            })
            .then_with(|| a.address.cmp(&b.address))
    });
    list
}

/// The A–Z directory over an already-sorted book: each contact files under
/// the initial of what the person calls it (name → resolved name → nothing,
/// which is `#`); letters A–Z then `#`; the book's order inside a letter.
pub fn section_contacts(sorted: &[Contact]) -> Vec<ContactSection> {
    let mut sections: Vec<ContactSection> = Vec::new();
    for contact in sorted {
        let letter = section_letter(&contact_display_name(contact));
        match sections
            .iter_mut()
            .find(|s| s.letter.len() == 1 && s.letter.starts_with(letter))
        {
            Some(section) => section.addresses.push(contact.address.clone()),
            None => sections.push(ContactSection {
                letter: letter.to_string(),
                addresses: vec![contact.address.clone()],
            }),
        }
    }
    sections.sort_by_key(|s| section_rank(s.letter.chars().next().unwrap_or('#')));
    sections
}

/// Match a contact against a search query (name or address),
/// contacts.ts:461-469.
pub fn matches_query(contact: &Contact, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    contact.address.contains(&q)
        || contact
            .name
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains(&q))
        || contact
            .resolved_name
            .as_deref()
            .is_some_and(|n| n.to_lowercase().contains(&q))
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must match the current attempt, plus a
/// render.
fn requests(model: &Model, ops: Vec<ContactOperation>) -> Command<ContactEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<ContactEffect, Event>> = ops
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for ContactEffect {
    type Op = ContactOperation;
    fn into_shell(self) -> Option<crux_core::Request<ContactOperation>> {
        match self {
            ContactEffect::Render(_) => None,
            ContactEffect::Shell(request) => Some(request),
        }
    }
}
