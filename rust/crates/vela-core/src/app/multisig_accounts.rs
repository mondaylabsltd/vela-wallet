//! Machine — the multisigs on this device (F0.4): create, add by address,
//! ENS name or invite, import from Safe{Wallet}, keep their records shared,
//! check who can act for them, back them up to Ethereum, remove them.
//!
//! A port of the web's `accounts.ts`, `store.svelte.ts`, `recheck.ts`,
//! `backup.ts`, `safe-service.ts` and `ens-name.ts`, rule for rule
//! (docs/multisig-vault-design.md §3.1, §3.3–§3.5, §4.1, §4.3, §11.1):
//!
//! - creating deploys nothing; the account and the creator's proof are kept
//!   the moment the passkey answers (r4a-13), before the service is asked,
//!   and a record that has not landed is asked for again
//!   ([`Event::Tick`] — the shell sends it every [`RECORD_RETRY_MS`], when
//!   back online and when the page is shown again);
//! - adding needs only the address: the service's genesis is used only if
//!   the core derives the address from it, and who controls it is the
//!   chain's word where it is deployed, the counterfactual rule's where it
//!   is not (§3.5): refused is not added, high risk only on the person's tap;
//! - its networks are the core's rule (`multisig_networks`), never the chain
//!   its record is on;
//! - every stored multisig is checked again once a session per network
//!   ([`Event::Recheck`]); the chain is the authority, and a network that
//!   does not answer changes nothing.
//!
//! The list lives in the store under the web's own keys
//! (`vela.multisigAccounts`, `vela.activeMultisig`,
//! `vela.multisigPendingProofs`, `vela.multisigDiscovered`), in the web's
//! own JSON. Other machines change a stored multisig through the shell
//! ([`Event::Apply`] with a [`AccountUpdate`] they sent as a notice).

use std::collections::{BTreeMap, BTreeSet};

use crux_core::{render::render, App, Command};
use futures::future::{self, Either};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_shell::{
    self as shell, keys, local_owners, lock, same_address, AccountUpdate, Chain, GenesisAnswer,
    ImportAnswer, Io, LocalAccount, MemberInfo, MultisigAccount, MultisigEffect, MultisigOperation,
    MultisigShellResult, Notice, OwnerProof, PasskeyPurpose, Service, ServiceFail, Shared,
};
use crate::multisig::{self, ExportedSafe, Genesis};

/// The default service's home chain (Gnosis): where a record goes when the
/// service in use names none.
pub const REGISTRY_HOME_CHAIN_ID: u64 = 100;
/// How often a record that has not landed is asked about again.
pub const RECORD_RETRY_MS: f64 = 60_000.0;
/// How long a queued record may take to land before its absence means anything.
pub const LANDING_MS: f64 = 10.0 * 60_000.0;
/// How long one network's read of an address is waited for when looking a multisig up.
pub const LOOKUP_READ_MS: u32 = 4_000;
/// A multisig's networks are looked for at most this often on one device.
pub const DISCOVER_EVERY_MS: f64 = 60.0 * 60.0 * 1000.0;
/// How many per-network mailbox reads are out at once (an older service).
const MAILBOX_READS_AT_ONCE: usize = 3;
/// Ethereum, where a backup of the record goes.
pub const ETHEREUM: u64 = 1;
const GET_THRESHOLD_CALL: &str = "0xe75235b8";
const IS_RECORDED: &str = "0xba340d1c";

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// What the person chose for a new multisig.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigCreateChoice"))]
pub struct CreateChoice {
    pub owners: Vec<String>,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub threshold: u64,
    pub name: String,
    /// The network it is first used on, kept here as its first network.
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    /// The Vela account on this device whose passkey proves it.
    pub creator: String,
    /// What the creator knows of the members (kept here, carried by its invite).
    #[serde(default)]
    pub members: BTreeMap<String, MemberInfo>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAccountsEvent"))]
pub enum Event {
    /// The screen came up: the store is read, then the background jobs run.
    Start {
        service_url: String,
        accounts: Vec<LocalAccount>,
        /// The networks this device has (built in, or added).
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        networks: Vec<u64>,
    },
    /// The session's Vela accounts changed.
    LocalAccounts {
        accounts: Vec<LocalAccount>,
    },
    /// The device's networks changed (one added, one removed).
    Networks {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        networks: Vec<u64>,
    },
    /// The networks of the space in front (mainnets or testnets): where an
    /// address is looked up (`addMultisig` reads the space's networks; the
    /// device's other networks still count as ones it has). Never sent: the
    /// lookup reads every network the device has.
    Space {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
    },
    /// The stored list changed in another document (a tab, the side panel):
    /// read again — the stored keys only, nothing asked of anyone.
    Reload,
    /// Another service is in use (Settings → Service Endpoints).
    Service {
        url: String,
    },
    /// Every [`RECORD_RETRY_MS`], online, focus, shown again
    /// (`followUnrecorded`): ask again about records that have not landed.
    Tick,
    /// Once a session: what each shared record is now (its name, and whether
    /// the service still has it).
    CheckRecords,
    /// A record was renamed here: look at it again now.
    RecheckRecord {
        address: String,
    },
    /// Once a session, and when a multisig or this device gains a network:
    /// who can act for each stored multisig on each of its networks.
    Recheck,
    /// Look for networks another member deployed it on (once a session).
    Discover {
        address: String,
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
    },
    /// A new draft on the create sheet: a fresh salt.
    NewDraft,
    /// The draft's owners, rule and name changed: its address is shown.
    Draft {
        owners: Vec<String>,
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        threshold: u64,
        name: String,
    },
    /// Which of these networks hold the Safe contracts a new multisig needs.
    CheckNetworks {
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
    },
    /// Create the draft (one passkey prompt).
    Create {
        choice: CreateChoice,
    },
    /// Resolve an ENS name typed as an address or a member.
    ResolveEns {
        name: String,
    },
    /// Add a multisig by its address (`addMultisig`); `network`: the one a
    /// link names, or the person picked when asked.
    Add {
        address: String,
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        network: Option<u64>,
        #[serde(default)]
        accept_risks: bool,
    },
    /// Read a multisig by its address and keep nothing (`findMultisig`, ms-21).
    Find {
        address: String,
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        network: Option<u64>,
        /// The networks to look on (a proposal link: its own one); absent,
        /// the space's.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number[] | null"))]
        chain_ids: Option<Vec<u64>>,
        /// How long before a record not there yet is asked for once more.
        #[serde(default)]
        #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
        wait_ms: Option<u64>,
    },
    /// Keep the one [`Event::Find`] read (a tap; not shown).
    KeepFound {
        #[serde(default)]
        accept_risks: bool,
    },
    /// Keep what an invite says of a multisig's members, as labels.
    KeepInvited {
        address: String,
        #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
        chain_ids: Vec<u64>,
        members: Vec<InvitedMember>,
    },
    /// A Safe{Wallet} export, picked by the person (read here only).
    ParseExport {
        json: String,
    },
    /// Import one of the export's Safes.
    Import {
        address: String,
        #[serde(default)]
        accept_risks: bool,
    },
    /// An owner's passkey shares it again (`shareAgain`).
    ShareAgain {
        address: String,
        owner: String,
    },
    /// Whether its record is on Ethereum.
    CheckBackup {
        address: String,
    },
    /// `from` (a Vela account here) writes its record on Ethereum.
    SaveToEthereum {
        address: String,
        from: String,
    },
    /// Where its record can be renamed, and whether it is deployed there.
    PlanRename {
        address: String,
    },
    Activate {
        address: Option<String>,
    },
    Remove {
        address: String,
    },
    SetLabel {
        address: String,
        label: Option<String>,
    },
    NameMembers {
        address: String,
        members: BTreeMap<String, MemberInfo>,
        #[serde(default)]
        typed: bool,
    },
    SetFirstRun {
        address: String,
        on: bool,
    },
    /// A change another machine made (its notice, routed by the shell).
    Apply {
        address: String,
        update: AccountUpdate,
    },
    #[serde(skip)]
    Changed,
}

/// A member an invite names (`InvitedMember`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigInvitedMember"))]
pub struct InvitedMember {
    #[serde(default)]
    pub address: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

// ---------------------------------------------------------------------------
// Outcomes the screens draw
// ---------------------------------------------------------------------------

/// Why a multisig was not added as it stands (`AddOutcome`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAddOutcome"))]
pub enum AddOutcome {
    Added {
        address: String,
    },
    NotFound,
    Unavailable,
    Unsupported,
    NotMultisig,
    /// Nothing says which network it is on: the person is asked.
    ChooseNetwork,
    /// No record, and a network did not answer in time: "couldn't check".
    Unchecked,
    /// Not deployed anywhere, and its genesis is refused.
    Refused {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    /// The chain did not answer for the Safe contracts it needs.
    Unverified {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
    },
    /// Deployed, and something besides its members can act for it: added
    /// only on the person's tap.
    HighRisk {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
}

/// What importing one exported Safe came to (`ImportOutcome`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigImportOutcome"))]
pub enum ImportOutcome {
    Imported,
    NotFound,
    Unavailable,
    Unsupported,
    NotDeployed,
    Refused {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    HighRisk {
        #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
        findings: Vec<Value>,
    },
    Unverified {
        #[cfg_attr(feature = "bindings", ts(type = "number"))]
        chain_id: u64,
    },
}

/// An ENS name, read on Ethereum (`EnsAnswer`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigEnsAnswer"))]
pub enum EnsAnswer {
    Found { name: String, address: String },
    None { name: String },
    Unavailable { name: String },
    Unsupported { name: String },
}

/// Whether a record is on Ethereum (`BackupState`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigBackupState"))]
pub enum BackupState {
    Checking,
    BackedUp,
    NotBackedUp,
    Unavailable,
    CouldNotCheck,
    /// Its `record` was sent from this device.
    Sent,
    Failed,
}

/// Where a phase of a flow stands.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigPhase"))]
pub enum Phase {
    #[default]
    Idle,
    Working,
    Done,
    Cancelled,
    Failed,
}

/// The create sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigCreateView"))]
pub struct CreateView {
    pub phase: Phase,
    /// The draft's address (its salt is fixed per draft); `None` while it
    /// cannot be made (`draft_error` says why).
    pub draft_address: Option<String>,
    pub draft_error: Option<String>,
    /// The created multisig, once kept here.
    pub address: Option<String>,
    /// The service refused its record (kept here all the same).
    pub refused: Option<String>,
    pub error: Option<String>,
    /// The Safe contracts per network asked about (`present` / `missing` / `unknown`).
    #[cfg_attr(feature = "bindings", ts(type = "Record<string, unknown>"))]
    pub networks: BTreeMap<u64, Value>,
}

/// The add sheet (and a link's read without keeping).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAddView"))]
pub struct AddView {
    pub phase: Phase,
    pub address: Option<String>,
    pub outcome: Option<AddOutcome>,
    /// What [`Event::Find`] read, not kept.
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub found: Option<MultisigAccount>,
    pub ens: Option<EnsAnswer>,
}

/// The import sheet.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigImportView"))]
pub struct ImportView {
    #[cfg_attr(feature = "bindings", ts(type = "unknown[]"))]
    pub safes: Vec<Value>,
    pub error: Option<String>,
    pub busy: Option<String>,
    pub outcomes: BTreeMap<String, ImportOutcome>,
}

/// Where a rename of the record runs (`MultisigSettings`): the index chain
/// holding the record that this device has; only offered deployed there.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigRenamePlan"))]
pub struct RenamePlan {
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub chain_id: Option<u64>,
    pub deployed: Option<bool>,
    /// r4a-12: why there is no chain — each said as itself.
    #[serde(default)]
    pub blocked: Option<RenameBlocked>,
}

/// Why a record cannot be renamed from here (`renameBlocked`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigRenameBlocked"))]
pub enum RenameBlocked {
    /// Its record is not saved yet: nothing to rename.
    Unrecorded,
    /// The service could not be asked.
    Unreachable,
    /// Held only on chains this device lacks.
    Elsewhere,
}

/// One stored multisig, as the switcher and Settings draw it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAccountRow"))]
pub struct AccountRow {
    #[cfg_attr(feature = "bindings", ts(type = "unknown"))]
    pub account: MultisigAccount,
    /// Its record's name when Vela shows it (`validName`), else its label.
    pub name: Option<String>,
    pub shared: bool,
    /// A creator's proof is kept here: its record is on its way (r4a-12).
    pub record_waiting: bool,
    /// The founding members that are Vela accounts on this device.
    pub local_members: Vec<String>,
    /// Networks where something besides its members can act for it.
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub high_risk_on: Vec<u64>,
    /// Networks where its genesis is refused: read-only there.
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub refused_on: Vec<u64>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigAccountsView"))]
pub struct MultisigAccountsView {
    pub loaded: bool,
    pub accounts: Vec<AccountRow>,
    pub active: Option<String>,
    pub create: CreateView,
    pub add: AddView,
    pub import: ImportView,
    pub backup: BTreeMap<String, BackupState>,
    pub share_again: BTreeMap<String, Phase>,
    /// Why sharing again failed when the passkey did (its `detail`); absent
    /// for one the service does not hold yet.
    #[serde(default)]
    pub share_again_failure: BTreeMap<String, String>,
    pub rename: BTreeMap<String, RenamePlan>,
    /// The service is away (the outage), as far as this machine has seen.
    pub service_down: bool,
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct State {
    loaded: bool,
    accounts: Vec<MultisigAccount>,
    active: Option<String>,
    proofs: BTreeMap<String, OwnerProof>,
    discovered_kept: BTreeMap<String, f64>,
    local: Vec<LocalAccount>,
    networks: Vec<u64>,
    space: Option<Vec<u64>>,
    service: Service,
    chain: Chain,
    // This session's.
    asked_without_proof: BTreeSet<String>,
    recording: BTreeSet<String>,
    retrying: bool,
    checked: BTreeSet<String>,
    discovered: BTreeSet<String>,
    verdicts: BTreeMap<String, Vec<Value>>,
    salt: Option<String>,
    draft: Option<(Vec<String>, u64, String)>,
    found: Option<(MultisigAccount, Vec<Value>)>,
    exported: Vec<ExportedSafe>,
    // What the screens draw.
    create: CreateView,
    add: AddView,
    import: ImportView,
    backup: BTreeMap<String, BackupState>,
    share_again: BTreeMap<String, Phase>,
    share_again_failure: BTreeMap<String, String>,
    rename: BTreeMap<String, RenamePlan>,
}

impl State {
    /// Where an address is looked up: the space in front, else every network here.
    fn lookup_chains(&self) -> Vec<u64> {
        self.space.clone().unwrap_or_else(|| self.networks.clone())
    }

    fn find(&self, address: &str) -> Option<&MultisigAccount> {
        self.accounts
            .iter()
            .find(|a| same_address(&a.address, address))
    }

    fn find_mut(&mut self, address: &str) -> Option<&mut MultisigAccount> {
        self.accounts
            .iter_mut()
            .find(|a| same_address(&a.address, address))
    }

    fn has_network(&self, chain_id: u64) -> bool {
        self.networks.contains(&chain_id)
    }

    /// `multisig.add`: the genesis must derive the address.
    fn add(
        &mut self,
        address: &str,
        genesis: &Genesis,
        recorded: bool,
        chain_ids: &[u64],
        now: f64,
    ) -> Result<MultisigAccount, String> {
        let derived = multisig::genesis_address(genesis).map_err(|e| e.to_string())?;
        if !same_address(&derived, address) {
            return Err("the genesis does not make this address".to_owned());
        }
        if self.find(address).is_some() {
            if recorded {
                self.mark_recorded(address);
            }
            for id in chain_ids {
                self.use_chain(address, *id);
            }
            return self.find(address).cloned().ok_or_else(|| "gone".to_owned());
        }
        let (owners, threshold) = multisig::genesis_members(genesis).map_err(|e| e.to_string())?;
        let mut ids: Vec<u64> = Vec::new();
        for id in chain_ids {
            if !ids.contains(id) {
                ids.push(*id);
            }
        }
        let row = MultisigAccount {
            address: address.to_owned(),
            genesis: genesis.clone(),
            owners,
            threshold,
            recorded,
            shared: None,
            chain_ids: ids,
            label: None,
            members: None,
            first_run: None,
            added_ms: now,
            findings: None,
        };
        self.accounts.push(row.clone());
        Ok(row)
    }

    fn set_label(&mut self, address: &str, label: Option<&str>) {
        let clean = label
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_owned);
        if let Some(a) = self.find_mut(address) {
            a.label = clean;
        }
    }

    /// `nameMembers`: what this device holds stays, unless `typed`.
    fn name_members(
        &mut self,
        address: &str,
        known: &BTreeMap<String, MemberInfo>,
        typed: bool,
    ) -> bool {
        let Some(row) = self.find_mut(address) else {
            return false;
        };
        let mut members = row.members.clone().unwrap_or_default();
        let mut changed = false;
        for (member, info) in known {
            let key = member.to_ascii_lowercase();
            let was = members.get(&key).cloned().unwrap_or_default();
            let given = info
                .name
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
                .map(str::to_owned);
            let name = if typed {
                given.clone().or(was.name.clone())
            } else {
                was.name.clone().or(given.clone())
            };
            let kind = if typed {
                info.kind.clone().or(was.kind.clone())
            } else {
                was.kind.clone().or(info.kind.clone())
            };
            let mine = (typed && given.is_some()) || was.typed == Some(true);
            if name == was.name && kind == was.kind && mine == (was.typed == Some(true)) {
                continue;
            }
            let typed_flag = (mine && name.is_some()).then_some(true);
            members.insert(
                key,
                MemberInfo {
                    name,
                    kind,
                    typed: typed_flag,
                },
            );
            changed = true;
        }
        if changed {
            row.members = Some(members);
        }
        changed
    }

    fn set_first_run(&mut self, address: &str, on: bool) {
        if let Some(a) = self.find_mut(address) {
            if a.first_run.unwrap_or(false) != on {
                a.first_run = on.then_some(true);
            }
        }
    }

    fn use_chain(&mut self, address: &str, chain_id: u64) -> bool {
        match self.find_mut(address) {
            Some(a) if !a.chain_ids.contains(&chain_id) => {
                a.chain_ids.push(chain_id);
                true
            }
            _ => false,
        }
    }

    fn mark_recorded(&mut self, address: &str) {
        if let Some(a) = self.find_mut(address) {
            a.recorded = true;
            a.shared = Some(true);
        }
    }

    fn mark_unshared(&mut self, address: &str) {
        if let Some(a) = self.find_mut(address) {
            if a.recorded || a.shared == Some(true) {
                a.recorded = false;
                a.shared = Some(false);
            }
        }
    }

    fn mark_shared(&mut self, address: &str) {
        if let Some(a) = self.find_mut(address) {
            if a.shared != Some(true) {
                a.shared = Some(true);
            }
        }
    }

    fn set_recorded_name(&mut self, address: &str, name: &str) {
        if let Some(a) = self.find_mut(address) {
            if a.genesis.name.as_deref() != Some(name) {
                a.genesis.name = Some(name.to_owned());
            }
        }
    }

    fn set_findings(&mut self, address: &str, findings: Vec<Value>) {
        if let Some(a) = self.find_mut(address) {
            a.findings = Some(findings);
        }
    }

    /// `setFindingsOn`: that network's findings replaced; written only on a change.
    fn set_findings_on(&mut self, address: &str, chain_id: u64, found: &[Value]) -> bool {
        let Some(a) = self.find_mut(address) else {
            return false;
        };
        let was = a.findings.clone().unwrap_or_default();
        let mut next: Vec<Value> = was
            .iter()
            .filter(|f| shell::finding_chain(f) != Some(chain_id))
            .cloned()
            .collect();
        next.extend(found.iter().cloned());
        if next == was {
            return false;
        }
        a.findings = Some(next);
        true
    }

    fn remove(&mut self, address: &str) {
        self.accounts.retain(|a| !same_address(&a.address, address));
        if self
            .active
            .as_deref()
            .is_some_and(|a| same_address(a, address))
        {
            self.active = None;
        }
    }

    fn accounts_json(&self) -> String {
        serde_json::to_string(&self.accounts).unwrap_or_else(|_| "[]".to_owned())
    }

    fn proofs_json(&self) -> String {
        serde_json::to_string(&self.proofs).unwrap_or_else(|_| "{}".to_owned())
    }
}

#[derive(Default)]
pub struct Model {
    state: Shared<State>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct MultisigAccounts;

type Cmd = Command<MultisigEffect, Event>;

/// Run `flow` as an async command over the model's state, rendering as it ends.
fn flow<F, Fut>(model: &Model, f: F) -> Cmd
where
    F: FnOnce(Io<Event>, Shared<State>) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let state = model.state.clone();
    Command::all([
        render(),
        Command::new(move |ctx| async move {
            let io = Io::new(ctx);
            f(io.clone(), state).await;
            io.emit(Event::Changed);
        }),
    ])
}

impl App for MultisigAccounts {
    type Event = Event;
    type Model = Model;
    type ViewModel = MultisigAccountsView;
    type Effect = MultisigEffect;

    fn update(&self, event: Event, model: &mut Model) -> Cmd {
        match event {
            Event::Changed => render(),
            Event::Start {
                service_url,
                accounts,
                networks,
            } => {
                {
                    let mut st = lock(&model.state);
                    st.service.set_url(&service_url);
                    st.local = accounts;
                    st.networks = networks;
                }
                flow(model, |io, st| async move { start(&io, &st).await })
            }
            Event::LocalAccounts { accounts } => {
                lock(&model.state).local = accounts;
                render()
            }
            Event::Networks { networks } => {
                lock(&model.state).networks = networks;
                flow(
                    model,
                    |io, st| async move { recheck_stored(&io, &st).await },
                )
            }
            Event::Space { chain_ids } => {
                lock(&model.state).space = Some(chain_ids);
                render()
            }
            Event::Reload => flow(model, |io, st| async move { start(&io, &st).await }),
            Event::Service { url } => {
                lock(&model.state).service.set_url(&url);
                render()
            }
            Event::Tick => flow(
                model,
                |io, st| async move { follow_unrecorded(&io, &st).await },
            ),
            Event::CheckRecords => {
                flow(model, |io, st| async move { check_records(&io, &st).await })
            }
            Event::RecheckRecord { address } => flow(model, move |io, st| async move {
                lock(&st).checked.remove(&address.to_ascii_lowercase());
                check_records(&io, &st).await;
            }),
            Event::Recheck => flow(
                model,
                |io, st| async move { recheck_stored(&io, &st).await },
            ),
            Event::Discover { address, chain_ids } => flow(model, move |io, st| async move {
                discover_chains(&io, &st, &address, &chain_ids).await;
            }),
            Event::NewDraft => {
                // A fresh sheet at once: what it asks next (its networks, its
                // draft) is never wiped by the salt answering after it.
                {
                    let mut s = lock(&model.state);
                    s.salt = None;
                    s.draft = None;
                    s.create = CreateView::default();
                }
                flow(model, |io, st| async move {
                    let salt = io.random_hex(32).await.map(|h| hex_prefixed(&h));
                    let mut s = lock(&st);
                    s.salt = salt;
                    redraft(&mut s);
                })
            }
            Event::Draft {
                owners,
                threshold,
                name,
            } => {
                let mut s = lock(&model.state);
                s.draft = Some((owners, threshold, name));
                redraft(&mut s);
                render()
            }
            Event::CheckNetworks { chain_ids } => flow(model, move |io, st| async move {
                let chain = lock(&st).chain.clone();
                let verdicts =
                    future::join_all(chain_ids.iter().map(|id| chain.safe_contracts_on(&io, *id)))
                        .await;
                let mut s = lock(&st);
                for (id, v) in chain_ids.iter().zip(verdicts) {
                    s.create
                        .networks
                        .insert(*id, serde_json::to_value(v).unwrap_or(Value::Null));
                }
            }),
            Event::Create { choice } => {
                {
                    let mut s = lock(&model.state);
                    s.create.phase = Phase::Working;
                    s.create.error = None;
                    s.create.refused = None;
                }
                flow(model, move |io, st| async move {
                    create(&io, &st, choice).await
                })
            }
            Event::ResolveEns { name } => {
                // The last name's answer goes at once: a retry is never answered by it.
                lock(&model.state).add.ens = None;
                flow(model, move |io, st| async move {
                    let answer = resolve_ens(&io, &name).await;
                    lock(&st).add.ens = Some(answer);
                })
            }
            Event::Add {
                address,
                network,
                accept_risks,
            } => {
                {
                    let mut s = lock(&model.state);
                    s.add.phase = Phase::Working;
                    s.add.address = Some(address.clone());
                    s.add.outcome = None;
                }
                flow(model, move |io, st| async move {
                    let outcome = add_multisig(&io, &st, &address, network, accept_risks).await;
                    let mut s = lock(&st);
                    s.add.phase = Phase::Done;
                    s.add.outcome = Some(outcome);
                })
            }
            Event::Find {
                address,
                network,
                chain_ids,
                wait_ms,
            } => {
                {
                    let mut s = lock(&model.state);
                    s.add.phase = Phase::Working;
                    s.add.address = Some(address.clone());
                    s.add.outcome = None;
                    s.add.found = None;
                    s.found = None;
                }
                flow(model, move |io, st| async move {
                    let chain_ids = chain_ids.unwrap_or_else(|| lock(&st).lookup_chains());
                    let wait_ms = wait_ms.unwrap_or(2000);
                    let found =
                        find_multisig(&io, &st, &address, &chain_ids, network, 1, wait_ms).await;
                    let mut s = lock(&st);
                    s.add.phase = Phase::Done;
                    match found {
                        Found::Account(account, findings) => {
                            s.add.found = Some((*account).clone());
                            s.add.outcome = Some(if findings.is_empty() {
                                AddOutcome::Added {
                                    address: account.address.clone(),
                                }
                            } else {
                                AddOutcome::HighRisk {
                                    findings: findings.clone(),
                                }
                            });
                            s.found = Some((*account, findings));
                        }
                        Found::No(outcome) => s.add.outcome = Some(outcome),
                    }
                })
            }
            Event::KeepFound { accept_risks } => flow(model, move |io, st| async move {
                let found = lock(&st).found.clone();
                if let Some((account, findings)) = found {
                    if findings.is_empty() || accept_risks {
                        keep_multisig(&io, &st, &account, findings).await;
                    }
                }
            }),
            Event::KeepInvited {
                address,
                chain_ids,
                members,
            } => flow(model, move |io, st| async move {
                keep_invited(&io, &st, &address, &chain_ids, &members).await;
            }),
            Event::ParseExport { json } => {
                let mut s = lock(&model.state);
                match multisig::parse_safe_export(&json) {
                    Ok(safes) => {
                        s.import = ImportView {
                            safes: safes
                                .iter()
                                .map(|x| serde_json::to_value(x).unwrap_or(Value::Null))
                                .collect(),
                            ..ImportView::default()
                        };
                        s.exported = safes;
                    }
                    Err(e) => {
                        s.import = ImportView {
                            error: Some(e.to_string()),
                            ..ImportView::default()
                        };
                        s.exported.clear();
                    }
                }
                render()
            }
            Event::Import {
                address,
                accept_risks,
            } => {
                let safe = {
                    let mut s = lock(&model.state);
                    s.import.busy = Some(address.clone());
                    s.exported
                        .iter()
                        .find(|x| same_address(&x.address, &address))
                        .cloned()
                };
                flow(model, move |io, st| async move {
                    let outcome = match safe {
                        Some(safe) => import_exported(&io, &st, &safe, accept_risks).await,
                        None => ImportOutcome::NotFound,
                    };
                    let mut s = lock(&st);
                    s.import.busy = None;
                    s.import
                        .outcomes
                        .insert(address.to_ascii_lowercase(), outcome);
                })
            }
            Event::ShareAgain { address, owner } => {
                {
                    let mut s = lock(&model.state);
                    let key = address.to_ascii_lowercase();
                    s.share_again.insert(key.clone(), Phase::Working);
                    s.share_again_failure.remove(&key);
                }
                flow(model, move |io, st| async move {
                    let (phase, failure) = share_again(&io, &st, &address, &owner).await;
                    let mut s = lock(&st);
                    let key = address.to_ascii_lowercase();
                    s.share_again.insert(key.clone(), phase);
                    if let Some(failure) = failure {
                        s.share_again_failure.insert(key, failure);
                    }
                })
            }
            Event::CheckBackup { address } => {
                lock(&model.state)
                    .backup
                    .insert(address.to_ascii_lowercase(), BackupState::Checking);
                flow(model, move |io, st| async move {
                    let state = check_backup(&io, &address).await;
                    lock(&st).backup.insert(address.to_ascii_lowercase(), state);
                })
            }
            Event::SaveToEthereum { address, from } => flow(model, move |io, st| async move {
                let genesis = lock(&st).find(&address).map(|a| a.genesis.clone());
                let Some(genesis) = genesis else { return };
                let state = start_backup(&io, &genesis, &from).await;
                lock(&st).backup.insert(address.to_ascii_lowercase(), state);
            }),
            Event::PlanRename { address } => flow(model, move |io, st| async move {
                let plan = plan_rename(&io, &st, &address).await;
                lock(&st).rename.insert(address.to_ascii_lowercase(), plan);
            }),
            Event::Activate { address } => flow(model, move |io, st| async move {
                activate(&io, &st, address).await;
            }),
            Event::Remove { address } => flow(model, move |io, st| async move {
                let was_active = {
                    let mut s = lock(&st);
                    let active = s.active.clone();
                    s.remove(&address);
                    active.is_some() && s.active.is_none()
                };
                save(&io, &st).await;
                if was_active {
                    activate(&io, &st, None).await;
                }
            }),
            Event::SetLabel { address, label } => {
                lock(&model.state).set_label(&address, label.as_deref());
                flow(model, |io, st| async move { save(&io, &st).await })
            }
            Event::NameMembers {
                address,
                members,
                typed,
            } => {
                let changed = lock(&model.state).name_members(&address, &members, typed);
                if !changed {
                    return render();
                }
                flow(model, |io, st| async move { save(&io, &st).await })
            }
            Event::SetFirstRun { address, on } => {
                lock(&model.state).set_first_run(&address, on);
                flow(model, |io, st| async move { save(&io, &st).await })
            }
            Event::Apply { address, update } => {
                let changed = {
                    let mut s = lock(&model.state);
                    match &update {
                        AccountUpdate::UseChain { chain_id } => s.use_chain(&address, *chain_id),
                        AccountUpdate::FindingsOn { chain_id, findings } => {
                            let key = verdict_key(&address, *chain_id);
                            s.verdicts.insert(key, findings.clone());
                            s.set_findings_on(&address, *chain_id, findings)
                        }
                        AccountUpdate::RecordedName { name } => {
                            s.set_recorded_name(&address, name);
                            true
                        }
                        AccountUpdate::NameMembers { members, typed } => {
                            s.name_members(&address, members, *typed)
                        }
                    }
                };
                if !changed {
                    return render();
                }
                flow(model, |io, st| async move { save(&io, &st).await })
            }
        }
    }

    fn view(&self, model: &Model) -> MultisigAccountsView {
        let s = lock(&model.state);
        let service_down = lock(&s.service.outage).down;
        MultisigAccountsView {
            loaded: s.loaded,
            accounts: s
                .accounts
                .iter()
                .map(|a| {
                    let findings = a.findings.clone().unwrap_or_default();
                    let on = |from: &str| {
                        let mut ids: Vec<u64> = Vec::new();
                        for f in &findings {
                            if f.get("from").and_then(Value::as_str) == Some(from) {
                                if let Some(id) = shell::finding_chain(f) {
                                    if !ids.contains(&id) {
                                        ids.push(id);
                                    }
                                }
                            }
                        }
                        ids
                    };
                    AccountRow {
                        name: a
                            .genesis
                            .name
                            .clone()
                            .filter(|n| multisig::valid_name(n))
                            .or_else(|| a.label.clone()),
                        shared: a.is_shared(),
                        record_waiting: s.proofs.contains_key(&a.address.to_ascii_lowercase()),
                        local_members: local_owners(&s.local, &a.owners)
                            .into_iter()
                            .map(|o| o.address)
                            .collect(),
                        high_risk_on: on("chain"),
                        refused_on: on("genesis"),
                        account: a.clone(),
                    }
                })
                .collect(),
            active: s.active.clone(),
            create: s.create.clone(),
            add: s.add.clone(),
            import: s.import.clone(),
            backup: s.backup.clone(),
            share_again: s.share_again.clone(),
            share_again_failure: s.share_again_failure.clone(),
            rename: s.rename.clone(),
            service_down,
        }
    }
}

// ---------------------------------------------------------------------------
// Store
// ---------------------------------------------------------------------------

fn hex_prefixed(hex: &str) -> String {
    if hex.starts_with("0x") {
        hex.to_owned()
    } else {
        format!("0x{hex}")
    }
}

/// Read the store (`multisig.boot`): rows without an address or genesis are dropped.
fn read_accounts(raw: Option<&str>) -> Vec<MultisigAccount> {
    let Some(rows) = raw.and_then(|r| serde_json::from_str::<Vec<Value>>(r).ok()) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter(|r| r.get("address").is_some_and(Value::is_string) && r.get("genesis").is_some())
        .filter_map(|r| serde_json::from_value(r).ok())
        .collect()
}

async fn start(io: &Io<Event>, st: &Shared<State>) {
    let loaded = io
        .load(&[
            keys::ACCOUNTS,
            keys::ACTIVE,
            keys::PENDING_PROOFS,
            keys::DISCOVERED,
            keys::DEPLOYED,
        ])
        .await;
    let get = |k: &str| loaded.get(k).cloned().flatten();
    {
        let mut s = lock(st);
        s.accounts = read_accounts(get(keys::ACCOUNTS).as_deref());
        s.active = get(keys::ACTIVE).filter(|a| !a.is_empty());
        s.proofs = get(keys::PENDING_PROOFS)
            .and_then(|r| serde_json::from_str(&r).ok())
            .unwrap_or_default();
        s.discovered_kept = get(keys::DISCOVERED)
            .and_then(|r| serde_json::from_str(&r).ok())
            .unwrap_or_default();
        s.chain.seed_deployed(get(keys::DEPLOYED).as_deref());
        s.loaded = true;
    }
    io.emit(Event::Changed);
}

async fn save(io: &Io<Event>, st: &Shared<State>) {
    // Never the list over one not read yet: a change before the store
    // answered would write an empty list over every stored multisig.
    let json = {
        let s = lock(st);
        if !s.loaded {
            return;
        }
        s.accounts_json()
    };
    io.store(keys::ACCOUNTS, Some(json)).await;
    io.emit(Event::Changed);
}

async fn save_proofs(io: &Io<Event>, st: &Shared<State>) {
    let json = lock(st).proofs_json();
    io.store(keys::PENDING_PROOFS, Some(json)).await;
}

async fn activate(io: &Io<Event>, st: &Shared<State>, address: Option<String>) {
    lock(st).active.clone_from(&address);
    io.store(keys::ACTIVE, address.clone()).await;
    io.notice(Notice::Activated { address }).await;
}

fn redraft(s: &mut State) {
    let Some((owners, threshold, name)) = s.draft.clone() else {
        return;
    };
    let Some(salt) = s.salt.clone() else {
        return;
    };
    match multisig::multisig_genesis(&owners, threshold, &name, &salt) {
        Ok(draft) => {
            s.create.draft_address = Some(draft.address);
            s.create.draft_error = None;
        }
        Err(e) => {
            s.create.draft_address = None;
            s.create.draft_error = Some(e.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// Create and record (`createMultisig`, `retryUnrecorded`, `shareAgain`)
// ---------------------------------------------------------------------------

/// Where every record the service writes goes: its home chain, as it says.
async fn record_home_chain(io: &Io<Event>, service: &Service) -> u64 {
    service
        .table(io)
        .await
        .and_then(|t| t.home_chain_id)
        .unwrap_or(REGISTRY_HOME_CHAIN_ID)
}

/// Why a record was not asked for: the service (away, busy — asked again
/// later), or something else (said on the created screen).
enum RecordFail {
    Service,
    Other(String),
}

/// Record `genesis` through the service on the index chain; `Ok(true)` once
/// the registry holds it (or will).
async fn record(
    io: &Io<Event>,
    service: &Service,
    address: &str,
    genesis: &Genesis,
    proof: Option<&OwnerProof>,
) -> Result<bool, RecordFail> {
    let named = multisig::named_genesis(genesis).map_err(|e| RecordFail::Other(e.to_string()))?;
    let mut req = json!({
        "chainId": record_home_chain(io, service).await,
        "safe": address,
        "genesis": named,
    });
    if let Some(proof) = proof {
        req["ownerProof"] = serde_json::to_value(proof).unwrap_or(Value::Null);
    }
    let answer = service
        .import_genesis(io, req)
        .await
        .map_err(|_| RecordFail::Service)?;
    Ok(matches!(
        answer,
        ImportAnswer::Recorded { .. } | ImportAnswer::Queued { .. }
    ))
}

/// `owner` asks the service to record `multisig`: one passkey prompt.
/// `None`: the person closed it.
async fn sign_owner_proof(
    io: &Io<Event>,
    owner: &LocalAccount,
    multisig_address: &str,
    chain_id: u64,
) -> Result<Option<OwnerProof>, String> {
    let challenge = multisig::sponsor_challenge(multisig_address).map_err(|e| e.to_string())?;
    let answer = io
        .ask(MultisigOperation::Passkey {
            owner: owner.address.clone(),
            chain_id,
            challenge,
            purpose: PasskeyPurpose::Record {
                multisig: multisig_address.to_owned(),
            },
        })
        .await;
    match answer {
        MultisigShellResult::Asserted {
            authenticator_data,
            client_data_json,
            signature,
            ..
        } => {
            let client = crate::primitives::from_hex(&client_data_json)
                .ok()
                .and_then(|b| String::from_utf8(b).ok())
                .ok_or_else(|| "the passkey's client data is not text".to_owned())?;
            let (_, owner_genesis) =
                multisig::vela_account_genesis(&owner.wallet_keys()).map_err(|e| e.to_string())?;
            Ok(Some(OwnerProof {
                owner: owner.address.clone(),
                owner_genesis,
                authenticator_data: hex_prefixed(&authenticator_data),
                client_data_json: client,
                signature: hex_prefixed(&signature),
            }))
        }
        MultisigShellResult::PasskeyCancelled => Ok(None),
        MultisigShellResult::PasskeyFailed { detail } => Err(detail),
        other => Err(format!("unexpected answer {other:?}")),
    }
}

async fn create(io: &Io<Event>, st: &Shared<State>, choice: CreateChoice) {
    let fail = |st: &Shared<State>, error: String| {
        let mut s = lock(st);
        s.create.phase = Phase::Failed;
        s.create.error = Some(error);
    };
    let (salt, creator, service) = {
        let s = lock(st);
        (
            s.salt.clone(),
            s.local
                .iter()
                .find(|a| same_address(&a.address, &choice.creator))
                .cloned(),
            s.service.clone(),
        )
    };
    let Some(salt) = salt else {
        return fail(st, "no draft".to_owned());
    };
    let Some(creator) = creator else {
        return fail(st, "the creator is not on this device".to_owned());
    };
    let draft =
        match multisig::multisig_genesis(&choice.owners, choice.threshold, &choice.name, &salt) {
            Ok(d) => d,
            Err(e) => return fail(st, e.to_string()),
        };
    let proof = match sign_owner_proof(io, &creator, &draft.address, choice.chain_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            lock(st).create.phase = Phase::Cancelled;
            return;
        }
        Err(e) => return fail(st, e),
    };
    let key = draft.address.to_ascii_lowercase();
    let now = io.now().await;
    let kept = {
        let mut s = lock(st);
        s.recording.insert(key.clone());
        s.proofs.insert(key.clone(), proof.clone());
        match s.add(
            &draft.address,
            &draft.genesis,
            false,
            &[choice.chain_id],
            now,
        ) {
            Ok(kept) => {
                if !choice.members.is_empty() {
                    s.name_members(&kept.address, &choice.members, false);
                }
                s.set_first_run(&kept.address, true);
                s.create.address = Some(kept.address.clone());
                Ok(kept)
            }
            Err(e) => Err(e),
        }
    };
    let kept = match kept {
        Ok(k) => k,
        Err(e) => {
            lock(st).recording.remove(&key);
            return fail(st, e);
        }
    };
    // Kept FIRST (r4a-13): the proof and the account, before the service.
    save_proofs(io, st).await;
    save(io, st).await;
    activate(io, st, Some(kept.address.clone())).await;
    io.emit(Event::Changed);
    let mut refused = None;
    let recorded = match record(io, &service, &draft.address, &draft.genesis, Some(&proof)).await {
        Ok(r) => r,
        Err(RecordFail::Service) => false,
        Err(RecordFail::Other(e)) => {
            refused = Some(e);
            false
        }
    };
    if recorded {
        let mut s = lock(st);
        s.mark_recorded(&draft.address);
        s.proofs.remove(&key);
    } else {
        match service.fetch_genesis(io, &draft.address, false).await {
            Ok(a) if a.found => lock(st).mark_shared(&draft.address),
            Ok(_) => {}
            Err(_) => {}
        }
    }
    save_proofs(io, st).await;
    save(io, st).await;
    let mut s = lock(st);
    s.recording.remove(&key);
    s.create.phase = Phase::Done;
    s.create.refused = refused;
}

/// `followUnrecorded`'s round: one retry at a time, only while one has not landed.
async fn follow_unrecorded(io: &Io<Event>, st: &Shared<State>) {
    {
        let mut s = lock(st);
        if s.retrying || !s.accounts.iter().any(|a| !a.recorded) {
            return;
        }
        s.retrying = true;
    }
    retry_unrecorded(io, st).await;
    lock(st).retrying = false;
}

/// Ask again for every multisig the registry has not confirmed. Quiet on failure.
pub async fn retry_unrecorded(io: &Io<Event>, st: &Shared<State>) {
    let service = lock(st).service.clone();
    let now = io.now().await;
    if lock(&service.outage).blocked(now) {
        return;
    }
    let unrecorded: Vec<MultisigAccount> = lock(st)
        .accounts
        .iter()
        .filter(|a| !a.recorded)
        .cloned()
        .collect();
    for account in unrecorded {
        let key = account.address.to_ascii_lowercase();
        if lock(st).recording.contains(&key) {
            continue;
        }
        let Ok(known) = service.fetch_genesis(io, &account.address, true).await else {
            continue;
        };
        if known.found && known.pending != Some(true) {
            lock(st).mark_recorded(&account.address);
            continue;
        }
        if known.found {
            lock(st).mark_shared(&account.address);
        }
        let proof = lock(st).proofs.get(&key).cloned();
        if proof.is_none() {
            let mut s = lock(st);
            if s.asked_without_proof.contains(&key) {
                continue;
            }
            s.asked_without_proof.insert(key.clone());
        }
        if let Ok(true) = record(
            io,
            &service,
            &account.address,
            &account.genesis,
            proof.as_ref(),
        )
        .await
        {
            lock(st).mark_shared(&account.address);
        }
    }
    {
        let mut s = lock(st);
        let recorded: Vec<String> = s
            .accounts
            .iter()
            .filter(|a| a.recorded)
            .map(|a| a.address.to_ascii_lowercase())
            .collect();
        for k in recorded {
            s.proofs.remove(&k);
        }
    }
    save_proofs(io, st).await;
    save(io, st).await;
}

async fn share_again(
    io: &Io<Event>,
    st: &Shared<State>,
    address: &str,
    owner: &str,
) -> (Phase, Option<String>) {
    let (account, owner, service) = {
        let s = lock(st);
        (
            s.find(address).cloned(),
            s.local
                .iter()
                .find(|a| same_address(&a.address, owner))
                .cloned(),
            s.service.clone(),
        )
    };
    let (Some(account), Some(owner)) = (account, owner) else {
        return (Phase::Failed, None);
    };
    let chain = account
        .chain_ids
        .first()
        .copied()
        .unwrap_or(REGISTRY_HOME_CHAIN_ID);
    let proof = match sign_owner_proof(io, &owner, &account.address, chain).await {
        Ok(Some(p)) => p,
        Ok(None) => return (Phase::Cancelled, None),
        Err(detail) => return (Phase::Failed, Some(detail)),
    };
    let recorded = record(
        io,
        &service,
        &account.address,
        &account.genesis,
        Some(&proof),
    )
    .await
    .unwrap_or(false);
    if !recorded {
        lock(st)
            .proofs
            .insert(account.address.to_ascii_lowercase(), proof);
        save_proofs(io, st).await;
    }
    let shared = recorded
        || service
            .fetch_genesis(io, &account.address, false)
            .await
            .is_ok_and(|a| a.found);
    if shared {
        lock(st).mark_shared(&account.address);
        save(io, st).await;
        (Phase::Done, None)
    } else {
        (Phase::Failed, None)
    }
}

// ---------------------------------------------------------------------------
// Records now (`checkRecords`)
// ---------------------------------------------------------------------------

/// The name `address`'s record holds now on `chain_id`, read with this
/// wallet's RPC; `None`: that chain cannot be read here; `Some(None)`: no record.
async fn recorded_name_on_chain(
    io: &Io<Event>,
    st: &Shared<State>,
    address: &str,
    chain_id: u64,
) -> Option<Option<String>> {
    if !lock(st).has_network(chain_id) {
        return None;
    }
    let data = multisig::genesis_of_call(address).ok()?;
    let ret = io
        .eth_call(chain_id, multisig::GENESIS_REGISTRY, &data)
        .await
        .ok()?;
    let record = multisig::read_genesis_of(address, &ret).ok()?;
    Some(record.and_then(|r| r.genesis.name))
}

async fn check_records(io: &Io<Event>, st: &Shared<State>) {
    let service = lock(st).service.clone();
    let now = io.now().await;
    if lock(&service.outage).blocked(now) {
        return;
    }
    let shared: Vec<MultisigAccount> = lock(st)
        .accounts
        .iter()
        .filter(|a| a.recorded || a.shared == Some(true))
        .cloned()
        .collect();
    let mut changed = false;
    for account in shared {
        let key = account.address.to_ascii_lowercase();
        if !lock(st).checked.insert(key.clone()) {
            continue;
        }
        let known: GenesisAnswer = match service.fetch_genesis(io, &account.address, true).await {
            Ok(k) => k,
            Err(_) => {
                lock(st).checked.remove(&key);
                continue;
            }
        };
        if !known.found {
            let now = io.now().await;
            let mut s = lock(st);
            if now - account.added_ms > LANDING_MS {
                s.mark_unshared(&account.address);
                changed = true;
            } else {
                s.checked.remove(&key);
            }
            continue;
        }
        let Some(genesis) = known
            .genesis
            .as_ref()
            .filter(|_| known.pending != Some(true))
        else {
            continue;
        };
        let derived = multisig::genesis_address(genesis).ok();
        if !derived.is_some_and(|d| same_address(&d, &account.address)) {
            continue;
        }
        let from = known
            .read_from
            .or_else(|| known.recorded_on.as_ref().and_then(|r| r.first().copied()));
        let on_chain = match from {
            Some(id) => recorded_name_on_chain(io, st, &account.address, id).await,
            None => None,
        };
        let name = match on_chain {
            None => genesis.name.clone(),
            Some(n) => n,
        };
        if let Some(name) = name {
            if !name.is_empty()
                && Some(&name) != account.genesis.name.as_ref()
                && multisig::valid_name(&name)
            {
                lock(st).set_recorded_name(&account.address, &name);
                changed = true;
            }
        }
    }
    if changed {
        save(io, st).await;
    }
}

// ---------------------------------------------------------------------------
// Adding by address (`addMultisig`, `findMultisig`, `vetAuthority`)
// ---------------------------------------------------------------------------

enum Found {
    Account(Box<MultisigAccount>, Vec<Value>),
    No(AddOutcome),
}

enum Vetted {
    Ok(Vec<Value>),
    No(AddOutcome),
}

async fn vet_authority(
    io: &Io<Event>,
    address: &str,
    genesis: &Genesis,
    deployed: &[u64],
    used: &[u64],
) -> Vetted {
    let mut findings = Vec::new();
    for chain_id in deployed {
        let check = shell::deployed_on(io, *chain_id, address).await;
        if check.unread {
            return Vetted::No(AddOutcome::Unavailable);
        }
        findings.extend(
            check
                .risks
                .iter()
                .map(|r| shell::chain_finding(*chain_id, r)),
        );
    }
    if !deployed.is_empty() {
        return Vetted::Ok(findings);
    }
    for chain_id in used {
        match shell::counterfactual_on(io, *chain_id, address, genesis).await {
            Some(multisig::Authority::Refused { reasons }) => {
                return Vetted::No(AddOutcome::Refused {
                    findings: reasons
                        .iter()
                        .map(|r| shell::genesis_finding(*chain_id, r))
                        .collect(),
                })
            }
            Some(multisig::Authority::Unknown { .. }) | None => {
                return Vetted::No(AddOutcome::Unverified {
                    chain_id: *chain_id,
                })
            }
            Some(multisig::Authority::Verified { .. }) => {}
        }
    }
    Vetted::Ok(findings)
}

/// Where `address` has code, and the networks that did not say (bounded).
async fn deployed_chains(io: &Io<Event>, address: &str, chain_ids: &[u64]) -> (Vec<u64>, Vec<u64>) {
    let reads = chain_ids.iter().map(|id| async move {
        (
            *id,
            Chain::has_code_within(io, *id, address, Some(LOOKUP_READ_MS))
                .await
                .ok(),
        )
    });
    let results = future::join_all(reads).await;
    (
        results
            .iter()
            .filter(|(_, has)| *has == Some(true))
            .map(|(id, _)| *id)
            .collect(),
        results
            .iter()
            .filter(|(_, has)| has.is_none())
            .map(|(id, _)| *id)
            .collect(),
    )
}

/// `address` has sent a transaction of its own on one of `chain_ids`.
async fn sent_from(io: &Io<Event>, address: &str, chain_ids: &[u64]) -> bool {
    let reads = chain_ids.iter().map(|id| async move {
        io.rpc_within(
            *id,
            "eth_getTransactionCount",
            json!([address, "latest"]),
            Some(LOOKUP_READ_MS),
        )
        .await
        .ok()
        .and_then(|r| r.as_str().and_then(shell::quantity))
        .unwrap_or(0)
    });
    future::join_all(reads).await.into_iter().any(|n| n > 0)
}

/// The contract at `address` answers as a Safe (`getThreshold` ≥ 1).
async fn is_safe(io: &Io<Event>, chain_id: u64, address: &str) -> bool {
    match io
        .rpc(
            chain_id,
            "eth_call",
            json!([{ "to": address, "data": GET_THRESHOLD_CALL }, "latest"]),
        )
        .await
    {
        Ok(result) => result
            .as_str()
            .is_some_and(|r| r.len() == 66 && shell::quantity(r).is_some_and(|t| t > 0)),
        // A node's error is no answer; a revert is no Safe.
        Err(shell::RpcFail::Error { message, .. }) => {
            !message.to_ascii_lowercase().contains("revert")
        }
        Err(shell::RpcFail::Unanswered(_)) => true,
    }
}

/// Which of `chain_ids` the mailbox holds proposals of `address` for (an older service).
async fn proposal_chains(
    io: &Io<Event>,
    service: &Service,
    address: &str,
    chain_ids: &[u64],
) -> Vec<u64> {
    let mut found = Vec::new();
    for batch in chain_ids.chunks(MAILBOX_READS_AT_ONCE) {
        let reads = batch.iter().map(|id| async move {
            match service.fetch_proposals(io, *id, address, true).await {
                Ok(Some(a)) if !a.proposals.is_empty() => Some(*id),
                _ => None,
            }
        });
        found.extend(future::join_all(reads).await.into_iter().flatten());
    }
    found
}

async fn wait_for_task(io: &Io<Event>, service: &Service, id: &str) {
    for _ in 0..30 {
        io.wait(2000).await;
        if let Some(status) = service.fetch_task(io, id).await {
            if status == "done" || status == "failed" {
                return;
            }
        }
    }
}

/// The multisig at `address`, read and checked — kept nowhere (`findMultisig`).
async fn find_multisig(
    io: &Io<Event>,
    st: &Shared<State>,
    address: &str,
    chain_ids: &[u64],
    network: Option<u64>,
    waits: u32,
    wait_ms: u64,
) -> Found {
    let service = lock(st).service.clone();
    let on_chain = {
        let (a, c) = (address.to_owned(), chain_ids.to_vec());
        let io = io.clone();
        async move {
            let ((deployed, unread), sent) =
                futures::join!(deployed_chains(&io, &a, &c), sent_from(&io, &a, &c));
            (deployed, unread, sent)
        }
        .boxed()
        .shared()
    };
    let mut answer = match service.fetch_genesis(io, address, false).await {
        Ok(a) => a,
        Err(_) => return Found::No(AddOutcome::Unavailable),
    };
    if !answer.found {
        let again = {
            let (service, io, a) = (service.clone(), io.clone(), address.to_owned());
            async move {
                let mut latest: Result<GenesisAnswer, ServiceFail> = Ok(GenesisAnswer::default());
                for _ in 0..waits {
                    io.wait(wait_ms).await;
                    latest = service.fetch_genesis(&io, &a, false).await;
                    if latest.as_ref().is_ok_and(|l| l.found) {
                        break;
                    }
                }
                latest
            }
            .boxed()
        };
        let (deployed, sent, again_rest, again_done) =
            match future::select(on_chain.clone(), again).await {
                Either::Left(((deployed, _, sent), rest)) => (deployed, sent, Some(rest), None),
                Either::Right((latest, oc)) => {
                    let (deployed, _, sent) = oc.await;
                    (deployed, sent, None, Some(latest))
                }
            };
        if let Some(first) = deployed.first() {
            if !is_safe(io, *first, address).await {
                return Found::No(AddOutcome::NotMultisig);
            }
        }
        if deployed.is_empty() && sent {
            return Found::No(AddOutcome::NotMultisig);
        }
        if deployed.is_empty() {
            let latest = match (again_done, again_rest) {
                (Some(l), _) => l,
                (None, Some(rest)) => rest.await,
                (None, None) => Ok(GenesisAnswer::default()),
            };
            match latest {
                Ok(l) => answer = l,
                Err(_) => return Found::No(AddOutcome::Unavailable),
            }
        }
    }
    if !answer.found {
        let (deployed, unread, _) = on_chain.clone().await;
        let served = service.served_chains(io).await;
        let deployed_on = deployed
            .iter()
            .find(|id| served.as_ref().is_some_and(|s| s.contains(id)))
            .or(deployed.first())
            .copied();
        let Some(deployed_on) = deployed_on else {
            return Found::No(if unread.is_empty() {
                AddOutcome::NotFound
            } else {
                AddOutcome::Unchecked
            });
        };
        match service
            .import_genesis(io, json!({ "chainId": deployed_on, "safe": address }))
            .await
        {
            Ok(ImportAnswer::Queued { task_id }) => wait_for_task(io, &service, &task_id).await,
            Ok(ImportAnswer::Refused { .. }) => return Found::No(AddOutcome::NotMultisig),
            Ok(_) => {}
            Err(_) => return Found::No(AddOutcome::Unavailable),
        }
        answer = match service.fetch_genesis(io, address, false).await {
            Ok(a) => a,
            Err(_) => return Found::No(AddOutcome::Unavailable),
        };
        if !answer.found || answer.genesis.is_none() {
            return Found::No(AddOutcome::NotFound);
        }
    }
    let Some(genesis) = answer.genesis.clone() else {
        return Found::No(AddOutcome::NotFound);
    };
    let Ok(derived) = multisig::genesis_address(&genesis) else {
        return Found::No(AddOutcome::Unsupported);
    };
    if !same_address(&derived, address) {
        return Found::No(AddOutcome::NotFound);
    }
    let (used, _, _) = on_chain.await;
    let all = service.fetch_all_proposals(io, address, false).await;
    let proposed: Vec<u64> = match &all {
        Ok(Some(all)) => {
            let mut ids: Vec<u64> = Vec::new();
            for p in &all.proposals {
                if !ids.contains(&p.chain_id) {
                    ids.push(p.chain_id);
                }
            }
            ids
        }
        Ok(None) if used.is_empty() && network.is_none() => {
            proposal_chains(io, &service, address, chain_ids).await
        }
        _ => Vec::new(),
    };
    let have = {
        let s = lock(st);
        let mut have: Vec<u64> = chain_ids.to_vec();
        for id in proposed.iter().chain(network.iter()) {
            if s.has_network(*id) && !have.contains(id) {
                have.push(*id);
            }
        }
        have
    };
    let where_ = multisig::multisig_networks(&multisig::NetworkEvidence {
        deployed: used.clone(),
        proposals: proposed,
        named: network,
        kept: Vec::new(),
        have: Some(have),
    });
    if where_.is_empty() {
        return Found::No(AddOutcome::ChooseNetwork);
    }
    let findings = match vet_authority(io, address, &genesis, &used, &where_).await {
        Vetted::Ok(f) => f,
        Vetted::No(outcome) => return Found::No(outcome),
    };
    let Ok((owners, threshold)) = multisig::genesis_members(&genesis) else {
        return Found::No(AddOutcome::Unsupported);
    };
    let account = MultisigAccount {
        address: address.to_owned(),
        genesis,
        owners,
        threshold,
        recorded: answer.pending != Some(true),
        shared: Some(true),
        chain_ids: where_,
        label: None,
        members: None,
        first_run: None,
        added_ms: io.now().await,
        findings: None,
    };
    Found::Account(Box::new(account), findings)
}

/// Keep what [`find_multisig`] read, in the list (not shown).
async fn keep_multisig(
    io: &Io<Event>,
    st: &Shared<State>,
    found: &MultisigAccount,
    findings: Vec<Value>,
) -> Option<MultisigAccount> {
    let now = io.now().await;
    let kept = {
        let mut s = lock(st);
        let kept = s
            .add(
                &found.address,
                &found.genesis,
                found.recorded,
                &found.chain_ids,
                now,
            )
            .ok()?;
        s.mark_shared(&kept.address);
        if !findings.is_empty() {
            s.set_findings(&kept.address, findings);
        }
        s.find(&kept.address).cloned()
    };
    save(io, st).await;
    kept
}

/// Add a multisig by address, and show it (`addMultisig`).
async fn add_multisig(
    io: &Io<Event>,
    st: &Shared<State>,
    address: &str,
    network: Option<u64>,
    accept_risks: bool,
) -> AddOutcome {
    let existing = lock(st).find(address).map(|a| a.address.clone());
    if let Some(existing) = existing {
        activate(io, st, Some(existing.clone())).await;
        return AddOutcome::Added { address: existing };
    }
    let chain_ids = lock(st).lookup_chains();
    let kept = match find_multisig(io, st, address, &chain_ids, network, 1, 2000).await {
        Found::Account(account, findings) if findings.is_empty() => {
            keep_multisig(io, st, &account, Vec::new()).await
        }
        Found::Account(account, findings) if accept_risks => {
            keep_multisig(io, st, &account, findings).await
        }
        Found::Account(_, findings) => return AddOutcome::HighRisk { findings },
        Found::No(outcome) => return outcome,
    };
    let Some(kept) = kept else {
        return AddOutcome::NotFound;
    };
    activate(io, st, Some(kept.address.clone())).await;
    AddOutcome::Added {
        address: kept.address,
    }
}

/// `keepInvited`: a label lands only on an address that is a member.
async fn keep_invited(
    io: &Io<Event>,
    st: &Shared<State>,
    address: &str,
    link_chains: &[u64],
    members: &[InvitedMember],
) {
    let (account, chain) = {
        let s = lock(st);
        (s.find(address).cloned(), s.chain.clone())
    };
    let Some(account) = account else { return };
    if members.is_empty() {
        return;
    }
    let info = |m: &InvitedMember| MemberInfo {
        name: m.name.clone().filter(|n| !n.is_empty()),
        kind: m.kind.clone(),
        typed: None,
    };
    let addressed: Vec<&InvitedMember> = members.iter().filter(|m| m.address.is_some()).collect();
    let mut changed = false;
    if addressed.is_empty() {
        if members.len() != account.owners.len() {
            return;
        }
        let named: BTreeMap<String, MemberInfo> = account
            .owners
            .iter()
            .zip(members)
            .map(|(o, m)| (o.clone(), info(m)))
            .collect();
        changed |= lock(st).name_members(&account.address, &named, false);
    } else {
        let is_founding = |m: &InvitedMember| {
            m.address
                .as_deref()
                .is_some_and(|a| account.owners.iter().any(|o| same_address(o, a)))
        };
        let founding: BTreeMap<String, MemberInfo> = addressed
            .iter()
            .filter(|m| is_founding(m))
            .filter_map(|m| Some((m.address.clone()?, info(m))))
            .collect();
        if !founding.is_empty() {
            changed |= lock(st).name_members(&account.address, &founding, false);
        }
        let others: Vec<&&InvitedMember> = addressed.iter().filter(|m| !is_founding(m)).collect();
        if !others.is_empty() {
            let ids: Vec<u64> = link_chains
                .iter()
                .filter(|id| account.chain_ids.contains(id))
                .copied()
                .collect();
            let reads = ids.iter().map(|id| {
                let chain = chain.clone();
                let account = account.clone();
                async move {
                    chain
                        .read_members(io, *id, &account.address, &account.genesis)
                        .await
                        .map(|m| m.owners)
                        .unwrap_or_default()
                }
            });
            let now: Vec<String> = future::join_all(reads)
                .await
                .into_iter()
                .flatten()
                .collect();
            let kept: BTreeMap<String, MemberInfo> = others
                .iter()
                .filter(|m| {
                    m.address
                        .as_deref()
                        .is_some_and(|a| now.iter().any(|o| same_address(o, a)))
                })
                .filter_map(|m| Some((m.address.clone()?, info(m))))
                .collect();
            if !kept.is_empty() {
                changed |= lock(st).name_members(&account.address, &kept, false);
            }
        }
    }
    if changed {
        save(io, st).await;
    }
}

// ---------------------------------------------------------------------------
// Networks another member made it used on (`discoverChains`)
// ---------------------------------------------------------------------------

async fn discover_chains(io: &Io<Event>, st: &Shared<State>, address: &str, chain_ids: &[u64]) {
    let account = {
        let mut s = lock(st);
        let key = address.to_ascii_lowercase();
        if !s.discovered.insert(key) {
            return;
        }
        s.find(address).cloned()
    };
    let Some(account) = account else { return };
    let mut unseen: Vec<u64> = chain_ids
        .iter()
        .filter(|id| !account.chain_ids.contains(id))
        .copied()
        .collect();
    if unseen.is_empty() {
        return;
    }
    let now = io.now().await;
    unseen.sort_unstable();
    let space = format!(
        "{}:{}",
        account.address.to_ascii_lowercase(),
        unseen
            .iter()
            .map(u64::to_string)
            .collect::<Vec<_>>()
            .join(",")
    );
    if lock(st)
        .discovered_kept
        .get(&space)
        .is_some_and(|at| now - at < DISCOVER_EVERY_MS)
    {
        return;
    }
    let (deployed, _) = deployed_chains(io, &account.address, &unseen).await;
    let kept_json = {
        let mut s = lock(st);
        s.discovered_kept.insert(space, now);
        s.discovered_kept
            .retain(|_, at| now - *at < DISCOVER_EVERY_MS);
        serde_json::to_string(&s.discovered_kept).unwrap_or_default()
    };
    io.store(keys::DISCOVERED, Some(kept_json)).await;
    let where_ = multisig::multisig_networks(&multisig::NetworkEvidence {
        deployed,
        kept: account.chain_ids.clone(),
        ..multisig::NetworkEvidence::default()
    });
    let mut changed = false;
    {
        let mut s = lock(st);
        for id in where_ {
            changed |= s.use_chain(&account.address, id);
        }
    }
    if changed {
        save(io, st).await;
    }
}

// ---------------------------------------------------------------------------
// Checked again (`recheck.ts`)
// ---------------------------------------------------------------------------

fn verdict_key(address: &str, chain_id: u64) -> String {
    format!("{}:{chain_id}", address.to_ascii_lowercase())
}

/// Every multisig here, on each of its networks this device has, not
/// checked this session — one multisig at a time.
async fn recheck_stored(io: &Io<Event>, st: &Shared<State>) {
    let accounts = lock(st).accounts.clone();
    for account in accounts {
        let due: Vec<u64> = {
            let s = lock(st);
            account
                .chain_ids
                .iter()
                .filter(|id| {
                    s.has_network(**id)
                        && !s
                            .verdicts
                            .contains_key(&verdict_key(&account.address, **id))
                })
                .copied()
                .collect()
        };
        let checks = due.iter().map(|id| {
            let account = account.clone();
            async move {
                (
                    *id,
                    shell::judge(io, *id, &account.address, &account.genesis).await,
                )
            }
        });
        let results = future::join_all(checks).await;
        let mut changed = false;
        {
            let mut s = lock(st);
            for (id, found) in results {
                let Some(found) = found else { continue };
                s.verdicts
                    .insert(verdict_key(&account.address, id), found.clone());
                changed |= s.set_findings_on(&account.address, id, &found);
            }
        }
        if changed {
            save(io, st).await;
        }
    }
}

// ---------------------------------------------------------------------------
// Importing from Safe{Wallet} (`importExported`, `safe-service.ts`)
// ---------------------------------------------------------------------------

/// The Safe transaction service's short name per chain.
pub fn tx_service_prefix(chain_id: u64) -> Option<&'static str> {
    Some(match chain_id {
        1 => "eth",
        10 => "oeth",
        50 => "xdc",
        56 => "bnb",
        100 => "gno",
        130 => "unichain",
        137 => "pol",
        143 => "monad",
        146 => "sonic",
        204 => "opbnb",
        324 => "zksync",
        480 => "wc",
        5000 => "mantle",
        8453 => "base",
        9745 => "plasma",
        10200 => "chi",
        42161 => "arb1",
        42220 => "celo",
        43111 => "hemi",
        43114 => "avax",
        57073 => "ink",
        59144 => "linea",
        80094 => "berachain",
        84532 => "basesep",
        534352 => "scr",
        747474 => "katana",
        11_155_111 => "sep",
        _ => return None,
    })
}

enum Creation {
    Genesis(Genesis),
    Unsupported,
}

/// The genesis a Safe was created with, from the chain's transaction
/// service (`creationGenesis`). `Err`: it could not be read.
async fn creation_genesis(
    io: &Io<Event>,
    chain_id: u64,
    safe: &str,
) -> Result<Option<Creation>, String> {
    let Some(prefix) = tx_service_prefix(chain_id) else {
        return Ok(None);
    };
    let url = format!(
        "https://api.safe.global/tx-service/{prefix}/api/v1/safes/{}/creation/",
        shell::checksummed(safe)
    );
    let (status, body) = match io.http("GET", url, None, 15_000).await {
        shell::HttpAnswer::Answered { status, body, .. } => (status, body),
        shell::HttpAnswer::Unreachable(d) => return Err(d),
    };
    if status == 404 || status == 422 {
        return Ok(None);
    }
    if !(200..300).contains(&status) {
        return Err(format!("transaction service: HTTP {status}"));
    }
    let c: Value = serde_json::from_str(&body).map_err(|e| e.to_string())?;
    let s = |k: &str| c.get(k).and_then(Value::as_str).map(str::to_owned);
    let salt = c.get("saltNonce").and_then(|v| match v {
        Value::String(s) => shell::quantity(s),
        Value::Number(n) => n.as_u64().map(u128::from),
        _ => None,
    });
    let raw_salt = c.get("saltNonce").filter(|v| !v.is_null());
    if let (Some(factory), Some(single), Some(setup), Some(_)) = (
        s("factoryAddress"),
        s("masterCopy"),
        s("setupData"),
        raw_salt,
    ) {
        if setup != "0x" {
            // A salt beyond u128 is read from its decimal digits by the core.
            let salt_hex = match salt {
                Some(v) => format!("0x{v:x}"),
                None => {
                    match decimal_to_hex(c.get("saltNonce").and_then(Value::as_str).unwrap_or("")) {
                        Some(h) => h,
                        None => return Ok(None),
                    }
                }
            };
            let genesis = Genesis {
                factory,
                singleton: single,
                salt_nonce: salt_hex,
                initializer: setup,
                name: None,
            };
            return Ok(match multisig::genesis_address(&genesis) {
                Ok(a) if same_address(&a, safe) => Some(Creation::Genesis(genesis)),
                Ok(_) => None,
                Err(_) => Some(Creation::Unsupported),
            });
        }
    }
    let Some(tx_hash) = s("transactionHash") else {
        return Ok(None);
    };
    let tx = io
        .rpc(chain_id, "eth_getTransactionByHash", json!([tx_hash]))
        .await
        .ok();
    let Some(input) = tx
        .as_ref()
        .and_then(|t| t.get("input"))
        .and_then(Value::as_str)
    else {
        return Ok(None);
    };
    if let Ok(Some(genesis)) = multisig::genesis_from_calldata(input, safe) {
        return Ok(Some(Creation::Genesis(genesis)));
    }
    Ok(s("factoryAddress").map(|_| Creation::Unsupported))
}

/// A decimal salt of any size as `0x` hex (`BigInt(s).toString(16)`).
fn decimal_to_hex(s: &str) -> Option<String> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    alloy_primitives::U256::from_str_radix(s, 10)
        .ok()
        .map(|v| format!("0x{v:x}"))
}

async fn import_exported(
    io: &Io<Event>,
    st: &Shared<State>,
    safe: &ExportedSafe,
    accept_risks: bool,
) -> ImportOutcome {
    let chain_ids: Vec<u64> = safe.chains.iter().map(|c| c.chain_id).collect();
    let (existing, service) = {
        let s = lock(st);
        (s.find(&safe.address).cloned(), s.service.clone())
    };
    if let Some(existing) = existing {
        {
            let mut s = lock(st);
            for id in &chain_ids {
                s.use_chain(&existing.address, *id);
            }
            if existing.label.is_none() {
                if let Some(name) = &safe.name {
                    s.set_label(&existing.address, Some(name));
                }
            }
        }
        save(io, st).await;
        return ImportOutcome::Imported;
    }
    let mut genesis: Option<Genesis> = None;
    let mut recorded = false;
    let mut unreachable = 0;
    let mut asked = 1;
    match service.fetch_genesis(io, &safe.address, false).await {
        Ok(known) if known.found && known.genesis.is_some() => {
            genesis = known.genesis;
            recorded = known.pending != Some(true);
        }
        Ok(_) => {}
        Err(_) => unreachable += 1,
    }
    if genesis.is_none() {
        for c in &safe.chains {
            if c.deployed {
                continue;
            }
            let Some(props) = &c.props else { continue };
            if let Ok(g) = multisig::safe_wallet_props_genesis(props, &safe.address) {
                genesis = Some(g);
                break;
            }
        }
    }
    let mut unsupported = false;
    if genesis.is_none() {
        for c in safe
            .chains
            .iter()
            .filter(|c| c.deployed && tx_service_prefix(c.chain_id).is_some())
        {
            asked += 1;
            match creation_genesis(io, c.chain_id, &safe.address).await {
                Ok(Some(Creation::Genesis(g))) => {
                    genesis = Some(g);
                    break;
                }
                Ok(Some(Creation::Unsupported)) => unsupported = true,
                Ok(None) => {}
                Err(_) => unreachable += 1,
            }
        }
    }
    let Some(genesis) = genesis else {
        if unsupported {
            return ImportOutcome::Unsupported;
        }
        if asked > 0 && unreachable == asked {
            return ImportOutcome::Unavailable;
        }
        let reads = safe
            .chains
            .iter()
            .map(|c| async move { Chain::has_code(io, c.chain_id, &safe.address).await.ok() });
        let deployed = future::join_all(reads).await;
        if deployed.contains(&Some(false)) && !deployed.contains(&Some(true)) {
            return ImportOutcome::NotDeployed;
        }
        return ImportOutcome::NotFound;
    };
    match multisig::genesis_address(&genesis) {
        Ok(a) if same_address(&a, &safe.address) => {}
        Ok(_) => return ImportOutcome::NotFound,
        Err(_) => return ImportOutcome::Unsupported,
    }
    let readable: Vec<u64> = {
        let s = lock(st);
        chain_ids
            .iter()
            .filter(|id| s.has_network(**id))
            .copied()
            .collect()
    };
    if readable.is_empty() {
        return ImportOutcome::Unverified {
            chain_id: chain_ids.first().copied().unwrap_or(0),
        };
    }
    let live_reads = readable.iter().map(|id| async move {
        Chain::has_code(io, *id, &safe.address)
            .await
            .unwrap_or(false)
            .then_some(*id)
    });
    let live: Vec<u64> = future::join_all(live_reads)
        .await
        .into_iter()
        .flatten()
        .collect();
    let findings = match vet_authority(io, &safe.address, &genesis, &live, &readable).await {
        Vetted::Ok(f) => f,
        Vetted::No(AddOutcome::Refused { findings }) => return ImportOutcome::Refused { findings },
        Vetted::No(AddOutcome::Unverified { chain_id }) => {
            return ImportOutcome::Unverified { chain_id }
        }
        Vetted::No(_) => return ImportOutcome::Unavailable,
    };
    if !findings.is_empty() && !accept_risks {
        return ImportOutcome::HighRisk { findings };
    }
    let now = io.now().await;
    let added = {
        let mut s = lock(st);
        match s.add(&safe.address, &genesis, recorded, &chain_ids, now) {
            Ok(a) => {
                if !findings.is_empty() {
                    s.set_findings(&a.address, findings);
                }
                if let Some(name) = &safe.name {
                    s.set_label(&a.address, Some(name));
                }
                if recorded {
                    s.mark_shared(&a.address);
                }
                true
            }
            Err(_) => false,
        }
    };
    if !added {
        return ImportOutcome::NotFound;
    }
    save(io, st).await;
    if !recorded {
        // The service, if it is there, records it for other devices (best effort).
        let listed: Vec<u64> = safe
            .chains
            .iter()
            .filter(|c| c.deployed)
            .map(|c| c.chain_id)
            .collect();
        if !listed.is_empty() {
            let served = service.served_chains(io).await;
            let read = listed
                .iter()
                .find(|id| served.as_ref().is_some_and(|s| s.contains(id)));
            let _ = match read {
                None => record(io, &service, &safe.address, &genesis, None)
                    .await
                    .map(|_| ())
                    .map_err(|_| ()),
                Some(id) => service
                    .import_genesis(io, json!({ "chainId": id, "safe": safe.address }))
                    .await
                    .map(|_| ())
                    .map_err(|_| ()),
            };
        }
    }
    ImportOutcome::Imported
}

// ---------------------------------------------------------------------------
// Ethereum backup (`backup.ts`) and the rename's chain
// ---------------------------------------------------------------------------

async fn check_backup(io: &Io<Event>, address: &str) -> BackupState {
    match Chain::has_code(io, ETHEREUM, multisig::GENESIS_REGISTRY).await {
        Ok(true) => {}
        Ok(false) => return BackupState::Unavailable,
        Err(_) => return BackupState::CouldNotCheck,
    }
    let data = format!(
        "{IS_RECORDED}{:0>64}",
        address.trim_start_matches("0x").to_ascii_lowercase()
    );
    match io
        .eth_call(ETHEREUM, multisig::GENESIS_REGISTRY, &data)
        .await
    {
        Ok(r) => {
            if shell::quantity(&r) == Some(1) {
                BackupState::BackedUp
            } else {
                BackupState::NotBackedUp
            }
        }
        Err(_) => BackupState::CouldNotCheck,
    }
}

async fn start_backup(io: &Io<Event>, genesis: &Genesis, from: &str) -> BackupState {
    let Ok(call) = multisig::record_call(genesis) else {
        return BackupState::Failed;
    };
    match io
        .ask(MultisigOperation::SendCalls {
            chain_id: ETHEREUM,
            from: from.to_owned(),
            calls: vec![call],
            fee: None,
        })
        .await
    {
        MultisigShellResult::CallsSent { .. } => BackupState::Sent,
        MultisigShellResult::CallsCancelled => BackupState::NotBackedUp,
        _ => BackupState::Failed,
    }
}

/// The rename runs on the registry chain that holds the record and that
/// this device has (the service's home chain first); offered only where
/// the multisig is deployed (§3.3, owner 2026-10-29: a rename never deploys).
/// A record still on its way, or none, names no chain (`unrecorded`); one
/// held only on chains this device lacks is `elsewhere`; a service that
/// could not be asked is `unreachable` (r4a-12) — each said as itself.
async fn plan_rename(io: &Io<Event>, st: &Shared<State>, address: &str) -> RenamePlan {
    let service = lock(st).service.clone();
    let blocked = |why| RenamePlan {
        chain_id: None,
        deployed: None,
        blocked: Some(why),
    };
    let Ok(known) = service.fetch_genesis(io, address, false).await else {
        return blocked(RenameBlocked::Unreachable);
    };
    let home = record_home_chain(io, &service).await;
    let recorded_on: Vec<u64> = if known.found && known.pending != Some(true) {
        known.recorded_on.clone().unwrap_or_default()
    } else {
        Vec::new()
    };
    let held: Vec<u64> = {
        let s = lock(st);
        recorded_on
            .iter()
            .filter(|id| s.has_network(**id))
            .copied()
            .collect()
    };
    let chain_id = if held.contains(&home) {
        Some(home)
    } else {
        held.first().copied()
    };
    let Some(chain_id) = chain_id else {
        return blocked(if recorded_on.is_empty() {
            RenameBlocked::Unrecorded
        } else {
            RenameBlocked::Elsewhere
        });
    };
    let deployed = Chain::has_code(io, chain_id, address).await.ok();
    RenamePlan {
        chain_id: Some(chain_id),
        deployed,
        blocked: None,
    }
}

// ---------------------------------------------------------------------------
// ENS (`ens-name.ts`)
// ---------------------------------------------------------------------------

const ENS_REGISTRY: &str = "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e";
const RESOLVER: &str = "0x0178b8bf";
const ADDR: &str = "0x3b3b57de";

/// Whether `text` is meant as an ENS name (ends in `.eth`).
pub fn looks_like_ens(text: &str) -> bool {
    let t = text.trim();
    t.len() > 4
        && t.to_ascii_lowercase().ends_with(".eth")
        && !t.chars().any(|c| c.is_whitespace() || c == '/')
}

/// Letters, digits and hyphens, dot-separated, ending in `.eth`.
fn plain_name(text: &str) -> bool {
    let Some(labels) = text.strip_suffix(".eth") else {
        return false;
    };
    !labels.is_empty()
        && labels.split('.').all(|l| {
            !l.is_empty()
                && l.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
                && !l.starts_with('-')
                && !l.ends_with('-')
        })
}

/// ENS namehash.
pub fn namehash(name: &str) -> String {
    let mut node = [0u8; 32];
    for label in name.rsplit('.') {
        let lh = crate::primitives::keccak256(label.as_bytes());
        let mut buf = node.to_vec();
        buf.extend_from_slice(&lh);
        node.copy_from_slice(&crate::primitives::keccak256(&buf));
    }
    crate::primitives::to_hex(&node, true)
}

fn ens_word(result: &str) -> Option<String> {
    let h = result.strip_prefix("0x")?;
    (h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())).then(|| format!("0x{}", &h[24..]))
}

async fn resolve_ens(io: &Io<Event>, typed: &str) -> EnsAnswer {
    let name = typed.trim().to_ascii_lowercase();
    if !plain_name(&name) {
        return EnsAnswer::Unsupported { name };
    }
    let node = namehash(&name);
    let node = node.trim_start_matches("0x");
    const ZERO: &str = "0x0000000000000000000000000000000000000000";
    let resolver = match io
        .eth_call(ETHEREUM, ENS_REGISTRY, &format!("{RESOLVER}{node}"))
        .await
    {
        Ok(r) => ens_word(&r),
        Err(_) => None,
    };
    let Some(resolver) = resolver else {
        return EnsAnswer::Unavailable { name };
    };
    if resolver == ZERO {
        return EnsAnswer::None { name };
    }
    match io
        .eth_call(ETHEREUM, &resolver, &format!("{ADDR}{node}"))
        .await
    {
        Err(shell::RpcFail::Error { message, .. }) => {
            if message.to_ascii_lowercase().contains("revert") {
                EnsAnswer::None { name }
            } else {
                EnsAnswer::Unavailable { name }
            }
        }
        Err(shell::RpcFail::Unanswered(_)) => EnsAnswer::Unavailable { name },
        Ok(r) => match ens_word(&r) {
            None => EnsAnswer::Unavailable { name },
            Some(a) if a == ZERO => EnsAnswer::None { name },
            Some(a) => EnsAnswer::Found {
                name,
                address: shell::checksummed(&a),
            },
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn ens_is_told_from_an_address() {
        assert!(looks_like_ens("vitalik.eth"));
        assert!(looks_like_ens(" Treasury.ETH "));
        assert!(!looks_like_ens(
            "0x0000000000000000000000000000000000000001"
        ));
        assert!(!looks_like_ens("a b.eth"));
        assert!(plain_name("pay.vitalik.eth"));
        assert!(
            !plain_name("vіtalik.eth"),
            "a lookalike is not normalised here"
        );
        assert!(!plain_name("-a.eth"));
    }

    #[test]
    fn namehash_is_ens_own() {
        assert_eq!(
            namehash("eth"),
            "0x93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert_eq!(
            namehash("vitalik.eth"),
            "0xee6c4522aab0003e8d14cd40a6af439055fd2577951148c14b6cea9a53475835"
        );
    }

    #[test]
    fn members_named_keep_this_devices_word() {
        let mut s = State::default();
        let g = multisig::multisig_genesis(
            &["0x00000000000000000000000000000000000000aa".to_owned()],
            1,
            "Quiet Harbor",
            "0x01",
        )
        .unwrap();
        s.add(&g.address, &g.genesis, false, &[100, 100], 1.0)
            .unwrap();
        assert_eq!(s.accounts[0].chain_ids, vec![100]);
        let m = |name: &str, kind: &str| MemberInfo {
            name: Some(name.to_owned()),
            kind: Some(kind.to_owned()),
            typed: None,
        };
        let a = "0x00000000000000000000000000000000000000AA".to_owned();
        assert!(s.name_members(
            &g.address,
            &BTreeMap::from([(a.clone(), m("Ana", "passkey"))]),
            false
        ));
        // A link's label does not replace what is kept.
        assert!(!s.name_members(
            &g.address,
            &BTreeMap::from([(a.clone(), m("Eve", "wallet"))]),
            false
        ));
        // The person's typed name does, and is marked typed.
        assert!(s.name_members(
            &g.address,
            &BTreeMap::from([(a.clone(), m("Ann", "wallet"))]),
            true
        ));
        let kept = &s.accounts[0].members.as_ref().unwrap()[&a.to_ascii_lowercase()];
        assert_eq!(kept.name.as_deref(), Some("Ann"));
        assert_eq!(kept.kind.as_deref(), Some("wallet"));
        assert_eq!(kept.typed, Some(true));
        // A genesis that is not this address is never kept.
        assert!(s
            .add(
                "0x0000000000000000000000000000000000000001",
                &g.genesis,
                false,
                &[],
                1.0
            )
            .is_err());
    }

    #[test]
    fn findings_on_a_network_replace_only_that_networks() {
        let mut s = State::default();
        let g = multisig::multisig_genesis(
            &["0x00000000000000000000000000000000000000aa".to_owned()],
            1,
            "Q",
            "0x02",
        )
        .unwrap();
        s.add(&g.address, &g.genesis, false, &[1, 2], 1.0).unwrap();
        let f1 =
            json!({"from": "chain", "chainId": 1, "risk": {"kind": "module", "address": "0x1"}});
        let f2 = json!({"from": "genesis", "chainId": 2, "reason": {"kind": "notSetup"}});
        assert!(s.set_findings_on(&g.address, 1, std::slice::from_ref(&f1)));
        assert!(s.set_findings_on(&g.address, 2, std::slice::from_ref(&f2)));
        assert!(
            !s.set_findings_on(&g.address, 2, std::slice::from_ref(&f2)),
            "unchanged: not written"
        );
        assert!(
            s.set_findings_on(&g.address, 1, &[]),
            "the chain is the authority: a risk gone goes"
        );
        assert_eq!(s.accounts[0].findings.as_ref().unwrap(), &vec![f2]);
    }
}
