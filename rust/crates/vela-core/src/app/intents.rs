//! The wallet's intents (spec 103): what a person or an AI agent can ask the
//! wallet to do, and the rules every shell applies before doing it.
//!
//! The UI and an agent are two parallel ways of asking. Both name the same
//! typed [`Action`], tagged with who asks ([`Actor`]). This module is the part
//! every shell must agree on, as pure functions:
//!
//! - the catalog — each action, how much it matters ([`Risk`]), and whether
//!   only the person may ask for it;
//! - [`validate`] — the input's own shape, before anything is looked up;
//! - [`authorize`] — whether the asker may: a person always may; an agent
//!   never touches its own connection, and anything [`Risk::Manage`] waits
//!   for the person's approval.
//!
//! Doing it is the shell's: it holds the machines that own each rule, sends
//! them the same events its UI sends, and answers `completed` only once it
//! has SEEN the change in their views. State that already lives in those
//! machines is not copied here.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::contacts::is_address;
use super::fee_policy::FeeTier;
use super::fee_tier_pref;
use super::network_admin::{NetEndpointField, NetOverrideField, NetProviderId};
use super::send::BATCH_MAX_RECIPIENTS;
use crate::{i18n, prefs, storage_catalog};

/// The settings pages a [`Screen::Settings`] may name (each shell lays out
/// the ones it has).
pub const SETTINGS_PAGES: [&str; 14] = [
    "account",
    "appearance",
    "localization",
    "networks",
    "rpc-providers",
    "endpoints",
    "storage",
    "fee-speed",
    "feedback",
    "community",
    "about",
    "agent",
    "signing-venue",
    "signing-pages",
];

// ---------------------------------------------------------------------------
// Who asks, and how much it matters
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentActor"))]
pub enum Actor {
    /// The person, through the wallet's own UI.
    Person,
    /// An AI agent, through a ClientMCP bridge.
    Agent,
}

/// How much an action matters — the app's own scale, which an agent sees for
/// every action it may ask for (ClientMCP SPEC §9.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentRisk"))]
pub enum Risk {
    /// Changes nothing.
    Observe,
    /// Ordinary, reversible use: moving around, preferences, the address book.
    Operate,
    /// Security-relevant or hard to undo. An agent's request waits for the
    /// person's approval in the wallet.
    Manage,
    /// Moves funds or signs. An agent prepares; signing stays the person's.
    Transact,
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// A place in the wallet to show. Each shell lays it out its own way.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentScreen"))]
pub enum Screen {
    Home,
    Contacts,
    Contact {
        address: String,
    },
    Group {
        id: String,
    },
    /// `page`: one of [`SETTINGS_PAGES`]; absent = Settings itself.
    Settings {
        page: Option<String>,
    },
    /// A held token's page. `token`: the contract, absent for the chain's coin.
    Asset {
        chain_id: u32,
        token: Option<String>,
    },
    /// One record of the activity, by its id (`recent_transactions[].id`).
    Activity {
        id: String,
    },
    /// Where to receive: the networks, or one network's code.
    Receive {
        chain_id: Option<u32>,
    },
}

/// One recipient of a transfer to several.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentRecipient"))]
pub struct Recipient {
    pub to: String,
    /// A dot-decimal in the token's units.
    pub amount: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentPreference"))]
pub enum Preference {
    /// One of [`prefs::THEMES`].
    Theme(String),
    /// A supported locale tag, or `auto`.
    Language(String),
    /// One of [`prefs::TEXT_SCALE_LEVELS`]' names.
    TextScale(String),
    NumberFormat(String),
    DateFormat(String),
    TimeFormat(String),
    /// An ISO 4217 code.
    Currency(String),
    FeeSpeed(FeeTier),
    /// Share anonymous usage statistics.
    Analytics(bool),
}

/// Everything a person — or an agent — can ask the wallet to do, as the
/// wallet's own capabilities rather than its controls.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentAction"))]
pub enum Action {
    Show {
        screen: Screen,
    },
    SwitchAccount {
        address: String,
    },
    SetPreference {
        preference: Preference,
    },
    /// Add a contact, or rename one.
    SaveContact {
        address: String,
        name: String,
    },
    DeleteContact {
        address: String,
    },
    /// Create a group (`id` absent), or rename one.
    SaveGroup {
        id: Option<String>,
        name: String,
    },
    DeleteGroup {
        id: String,
    },
    /// A group's whole membership.
    SetGroupMembers {
        id: String,
        members: Vec<String>,
    },
    /// Every group a contact is in.
    SetContactGroups {
        address: String,
        group_ids: Vec<String>,
    },
    /// Take one wallet off this device (its passkey is untouched).
    RemoveWallet {
        address: String,
    },
    /// Sign out of this device. The session's own sheet asks first — for an
    /// agent, that sheet is the approval.
    SignOut,
    /// Erase everything this wallet keeps on this device.
    EraseDevice,
    /// Clear one storage item ([`storage_catalog::ITEMS`]).
    ClearStorageItem {
        item: String,
    },
    ClearCaches,
    /// The agent's own connection. The person's alone.
    ConfigureAgent {
        enabled: Option<bool>,
        bridge_url: Option<String>,
        client_token: Option<String>,
    },
    /// Whether an agent may submit a transaction for the person's passkey.
    /// The person's alone.
    SetAgentGrant {
        submit_transactions: bool,
    },
    /// A transfer, drafted into the wallet's own review — the screen a person
    /// reaches by filling the form. Nothing moves until it is submitted.
    /// `token`: the contract, absent for the chain's own coin. `amount`: a
    /// dot-decimal in the token's units, or `max`.
    PrepareTransfer {
        chain_id: u32,
        token: Option<String>,
        to: String,
        amount: String,
    },
    /// Press confirm on a prepared transfer's review: the person's passkey
    /// signs it. An agent may only with the person's grant.
    SubmitTransfer {
        operation: String,
    },
    /// Leave a prepared transfer's review without sending it.
    CancelTransfer {
        operation: String,
    },
    /// One token to several recipients, drafted into the review — the form's
    /// own several-recipients mode. Nothing moves until it is submitted.
    PrepareBatchTransfer {
        chain_id: u32,
        token: Option<String>,
        recipients: Vec<Recipient>,
    },
    /// Add a token to the wallet's list, by its contract on a chain.
    AddToken {
        chain_id: u32,
        address: String,
    },
    /// Delete one record from this device's activity history.
    DeleteActivity {
        id: String,
    },
    /// Hide or show amounts on every money surface.
    SetBalancePrivacy {
        hidden: bool,
    },
    /// Read the balances again now.
    RefreshBalances,
    SetContactFavorite {
        address: String,
        favorite: bool,
    },
    /// Add a network by its chain id, as Settings → Add network does: the
    /// wallet looks it up and checks it can serve it. `rpc_url`: an RPC to try
    /// first (it is raced with the known ones; the fastest that answers wins).
    AddNetwork {
        chain_id: u32,
        rpc_url: Option<String>,
    },
    /// One network's RPC or explorer. An RPC that reports another chain is
    /// refused; an empty one goes back to the default.
    SetNetworkEndpoint {
        chain_id: u32,
        field: NetOverrideField,
        url: String,
    },
    /// Remove a network the person added (the built-in ones stay).
    RemoveNetwork {
        chain_id: u32,
    },
    /// An RPC provider's API key; empty removes it.
    SetProviderKey {
        provider: NetProviderId,
        key: String,
    },
    /// One of the services the wallet relies on.
    SetServiceEndpoint {
        field: NetEndpointField,
        url: String,
    },
    ResetServiceEndpoints,
}

/// One entry of the catalog: what the action is for, as an agent reads it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentCatalogEntry"))]
pub struct CatalogEntry {
    /// The action's `type`.
    pub action: String,
    pub risk: Risk,
    /// Only the person may ask for it.
    pub person_only: bool,
    pub description: String,
    /// JSON Schema for the action's fields (all but `type`), as an agent's
    /// tool takes them.
    #[cfg_attr(feature = "bindings", ts(type = "Record<string, unknown>"))]
    pub input_schema: serde_json::Value,
}

struct Entry {
    action: &'static str,
    risk: Risk,
    person_only: bool,
    description: &'static str,
}

const fn entry(action: &'static str, risk: Risk, description: &'static str) -> Entry {
    Entry {
        action,
        risk,
        person_only: false,
        description,
    }
}

const fn person_only(action: &'static str, description: &'static str) -> Entry {
    Entry {
        action,
        risk: Risk::Manage,
        person_only: true,
        description,
    }
}

/// Every action, in one place: name, risk, and who may ask for it. The risk of
/// an [`Action`] value is read from here, so a scale and its catalog cannot
/// disagree.
const CATALOG: [Entry; 31] = [
    entry(
        "show",
        Risk::Operate,
        "Show a place in the wallet to the person.",
    ),
    entry(
        "switch_account",
        Risk::Operate,
        "Make another wallet on this device the active one.",
    ),
    entry(
        "set_preference",
        Risk::Operate,
        "Change a display or fee preference. Sharing usage statistics is a manage-level change.",
    ),
    entry(
        "save_contact",
        Risk::Operate,
        "Add a contact, or rename one.",
    ),
    entry("delete_contact", Risk::Manage, "Delete a contact."),
    entry(
        "save_group",
        Risk::Operate,
        "Create a contact group, or rename one.",
    ),
    entry(
        "delete_group",
        Risk::Manage,
        "Delete a contact group (its contacts stay).",
    ),
    entry(
        "set_group_members",
        Risk::Operate,
        "Set which saved contacts are in a group.",
    ),
    entry(
        "set_contact_groups",
        Risk::Operate,
        "Set which groups a contact is in.",
    ),
    entry(
        "remove_wallet",
        Risk::Manage,
        "Take one wallet off this device. Its passkey is not touched.",
    ),
    entry(
        "sign_out",
        Risk::Manage,
        "Sign out of this device. The person confirms on the wallet's own sheet.",
    ),
    entry(
        "erase_device",
        Risk::Manage,
        "Erase everything this wallet keeps on this device.",
    ),
    entry(
        "clear_storage_item",
        Risk::Manage,
        "Clear one kind of data this wallet keeps on this device.",
    ),
    entry(
        "clear_caches",
        Risk::Manage,
        "Clear every cache this wallet keeps on this device.",
    ),
    person_only(
        "configure_agent",
        "The AI agent's connection: on or off, its bridge and its token.",
    ),
    person_only(
        "set_agent_grant",
        "Whether an AI agent may submit transactions for the person's passkey.",
    ),
    entry(
        "prepare_transfer",
        Risk::Transact,
        "Draft a transfer into the wallet's own review screen, for the person to see. Nothing is sent: it answers `accepted` with an operation to follow (vela_wait). If an earlier transfer's receipt is still showing, that transfer has already gone out: drafting closes its receipt first (its own Done), says which one (`closed_receipt`), and goes on following it. A send still short of its receipt answers `busy`.",
    ),
    entry(
        "submit_transfer",
        Risk::Transact,
        "Press confirm on a prepared transfer: the person's passkey signs it. Only when the person has allowed it in Settings; otherwise they confirm it themselves. Once the person dismissed the passkey prompt for a transfer, only they can confirm that one (`denied`). On one network, a transfer is confirmed only once the previous one there is final: until then this answers `not_ready`.",
    ),
    entry(
        "cancel_transfer",
        Risk::Operate,
        "Leave a prepared transfer's review without sending it.",
    ),
    entry(
        "prepare_batch_transfer",
        Risk::Transact,
        "Draft one token to several recipients (each with its own amount) into the wallet's review. Nothing is sent: it answers `accepted` with an operation; submit or cancel it like a single transfer. An earlier transfer's receipt still showing is closed first, as for prepare_transfer (`closed_receipt`).",
    ),
    entry(
        "add_token",
        Risk::Operate,
        "Add a token to the wallet's list by its contract address on a chain.",
    ),
    entry(
        "delete_activity",
        Risk::Manage,
        "Delete one record from this device's activity history, by its record id — or a batch's operation hash, which deletes every record of that batch. Only the wallet's own records go; the chain keeps its own.",
    ),
    entry(
        "set_balance_privacy",
        Risk::Operate,
        "Hide or show amounts on every money surface.",
    ),
    entry("refresh_balances", Risk::Operate, "Read the balances again now."),
    entry(
        "set_contact_favorite",
        Risk::Operate,
        "Mark a contact as a favorite, or not.",
    ),
    entry(
        "add_network",
        Risk::Manage,
        "Add a network by chain id, as Settings → Add network does (the wallet checks it can serve it). An rpc_url is raced with the known RPCs; the fastest that answers is kept.",
    ),
    entry(
        "set_network_endpoint",
        Risk::Manage,
        "Change one network's RPC or explorer URL. An RPC reporting another chain is refused; empty restores the default.",
    ),
    entry(
        "remove_network",
        Risk::Manage,
        "Remove a network the person added.",
    ),
    entry(
        "set_provider_key",
        Risk::Manage,
        "Set an RPC provider's API key (alchemy, drpc or ankr); empty removes it.",
    ),
    entry(
        "set_service_endpoint",
        Risk::Manage,
        "Point one of the wallet's services (ethereum_data, passkey_index, bundler_service, fiat_rates) at another URL; empty restores its default.",
    ),
    entry(
        "reset_service_endpoints",
        Risk::Manage,
        "Put every service back on its default URL.",
    ),
];

fn catalog_entry(kind: &str) -> Option<&'static Entry> {
    CATALOG.iter().find(|entry| entry.action == kind)
}

/// The catalog, as the shells and an agent read it.
#[must_use]
pub fn catalog() -> Vec<CatalogEntry> {
    CATALOG
        .iter()
        .map(|entry| CatalogEntry {
            action: entry.action.to_owned(),
            risk: entry.risk,
            person_only: entry.person_only,
            description: entry.description.to_owned(),
            input_schema: input_schema(entry.action),
        })
        .collect()
}

fn object(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "properties": properties,
        "required": required,
        "additionalProperties": false
    })
}

fn url_schema() -> serde_json::Value {
    serde_json::json!({ "type": "string", "pattern": "^https?://\\S+$" })
}

fn address_schema(description: &str) -> serde_json::Value {
    serde_json::json!({
        "type": "string",
        "pattern": "^0x[0-9a-fA-F]{40}$",
        "description": description
    })
}

fn choice(kind: &str, value: serde_json::Value) -> serde_json::Value {
    object(
        serde_json::json!({ "kind": { "const": kind }, "value": value }),
        &["kind", "value"],
    )
}

fn strings(values: &[&str]) -> serde_json::Value {
    serde_json::json!({ "type": "string", "enum": values })
}

/// The shapes every shell's agent tools take — from the same sets the checks
/// use, so a tool never offers a value [`validate`] refuses.
fn input_schema(kind: &str) -> serde_json::Value {
    use serde_json::json;
    let address = address_schema("An Ethereum address.");
    let group_id = json!({ "type": "string", "description": "A group's id." });
    match kind {
        "show" => {
            let screen = |name: &str, extra: serde_json::Value, required: &[&str]| {
                let mut properties = json!({ "type": { "const": name } });
                if let (Some(into), Some(from)) = (properties.as_object_mut(), extra.as_object()) {
                    into.extend(from.clone());
                }
                let mut all = vec!["type"];
                all.extend(required);
                object(properties, &all)
            };
            object(
                json!({ "screen": { "oneOf": [
                    screen("home", json!({}), &[]),
                    screen("contacts", json!({}), &[]),
                    screen("contact", json!({ "address": address }), &["address"]),
                    screen("group", json!({ "id": group_id }), &["id"]),
                    screen("settings", json!({ "page": {
                        "oneOf": [strings(&SETTINGS_PAGES), { "type": "null" }]
                    } }), &[]),
                    screen("asset", json!({
                        "chain_id": { "type": "integer", "minimum": 1 },
                        "token": { "oneOf": [address_schema("The token contract."), { "type": "null" }] }
                    }), &["chain_id"]),
                    screen("activity", json!({ "id": { "type": "string" } }), &["id"]),
                    screen("receive", json!({
                        "chain_id": { "oneOf": [{ "type": "integer", "minimum": 1 }, { "type": "null" }] }
                    }), &[]),
                ] } }),
                &["screen"],
            )
        }
        "set_preference" => {
            let mut languages = vec![prefs::AUTO_LANGUAGE];
            languages.extend(i18n::resolve::SUPPORTED);
            let levels: Vec<&str> = prefs::TEXT_SCALE_LEVELS
                .iter()
                .map(|(name, _)| *name)
                .collect();
            object(
                json!({ "preference": { "oneOf": [
                    choice("theme", strings(&prefs::THEMES)),
                    choice("language", strings(&languages)),
                    choice("text_scale", strings(&levels)),
                    choice("number_format", strings(&prefs::NUMBER_FORMATS)),
                    choice("date_format", strings(&prefs::DATE_FORMATS)),
                    choice("time_format", strings(&prefs::TIME_FORMATS)),
                    choice("currency", json!({ "type": "string", "pattern": "^[A-Z]{3}$" })),
                    choice("fee_speed", strings(&["fast", "standard", "slow"])),
                    choice("analytics", json!({ "type": "boolean" })),
                ] } }),
                &["preference"],
            )
        }
        "switch_account" | "remove_wallet" | "delete_contact" => {
            object(json!({ "address": address }), &["address"])
        }
        "save_contact" => object(
            json!({ "address": address, "name": { "type": "string", "minLength": 1 } }),
            &["address", "name"],
        ),
        "save_group" => object(
            json!({
                "id": { "type": ["string", "null"], "description": "Absent or null: a new group." },
                "name": { "type": "string", "minLength": 1 }
            }),
            &["name"],
        ),
        "delete_group" => object(json!({ "id": group_id }), &["id"]),
        "set_group_members" => object(
            json!({ "id": group_id, "members": { "type": "array", "items": address } }),
            &["id", "members"],
        ),
        "set_contact_groups" => object(
            json!({ "address": address, "group_ids": { "type": "array", "items": group_id } }),
            &["address", "group_ids"],
        ),
        "clear_storage_item" => {
            let ids: Vec<&str> = storage_catalog::ITEMS.iter().map(|item| item.id).collect();
            object(json!({ "item": strings(&ids) }), &["item"])
        }
        "configure_agent" => object(
            json!({
                "enabled": { "type": ["boolean", "null"] },
                "bridge_url": { "type": ["string", "null"] },
                "client_token": { "type": ["string", "null"] }
            }),
            &[],
        ),
        "set_agent_grant" => object(
            json!({ "submit_transactions": { "type": "boolean" } }),
            &["submit_transactions"],
        ),
        "prepare_transfer" => object(
            json!({
                "chain_id": { "type": "integer", "minimum": 1 },
                "token": {
                    "oneOf": [address_schema("The token contract."), { "type": "null" }],
                    "description": "Absent or null: the chain's own coin."
                },
                "to": address_schema("The recipient's address."),
                "amount": {
                    "type": "string",
                    "pattern": AMOUNT_PATTERN,
                    "description": "Dot-decimal in the token's units, or \"max\"."
                }
            }),
            &["chain_id", "to", "amount"],
        ),
        "submit_transfer" | "cancel_transfer" => object(
            json!({ "operation": { "type": "string", "description": "From prepare_transfer." } }),
            &["operation"],
        ),
        "prepare_batch_transfer" => object(
            json!({
                "chain_id": { "type": "integer", "minimum": 1 },
                "token": {
                    "oneOf": [address_schema("The token contract."), { "type": "null" }],
                    "description": "Absent or null: the chain's own coin."
                },
                "recipients": {
                    "type": "array",
                    "minItems": 1,
                    "maxItems": MAX_RECIPIENTS,
                    "items": object(json!({
                        "to": address_schema("The recipient's address."),
                        "amount": { "type": "string", "pattern": DECIMAL_PATTERN }
                    }), &["to", "amount"])
                }
            }),
            &["chain_id", "recipients"],
        ),
        "add_token" => object(
            json!({
                "chain_id": { "type": "integer", "minimum": 1 },
                "address": address_schema("The token contract.")
            }),
            &["chain_id", "address"],
        ),
        "delete_activity" => object(
            json!({ "id": { "type": "string", "description": "recent_transactions[].id" } }),
            &["id"],
        ),
        "set_balance_privacy" => object(json!({ "hidden": { "type": "boolean" } }), &["hidden"]),
        "add_network" => object(
            json!({
                "chain_id": { "type": "integer", "minimum": 1 },
                "rpc_url": { "oneOf": [url_schema(), { "type": "null" }] }
            }),
            &["chain_id"],
        ),
        "set_network_endpoint" => object(
            json!({
                "chain_id": { "type": "integer", "minimum": 1 },
                "field": strings(&["rpc", "explorer"]),
                "url": { "oneOf": [url_schema(), { "const": "" }] }
            }),
            &["chain_id", "field", "url"],
        ),
        "remove_network" => object(
            json!({ "chain_id": { "type": "integer", "minimum": 1 } }),
            &["chain_id"],
        ),
        "set_provider_key" => object(
            json!({
                "provider": strings(&["alchemy", "drpc", "ankr"]),
                "key": { "type": "string" }
            }),
            &["provider", "key"],
        ),
        "set_service_endpoint" => object(
            json!({
                "field": strings(&["ethereum_data", "passkey_index", "bundler_service", "fiat_rates"]),
                "url": url_schema()
            }),
            &["field", "url"],
        ),
        "set_contact_favorite" => object(
            json!({ "address": address, "favorite": { "type": "boolean" } }),
            &["address", "favorite"],
        ),
        _ => object(json!({}), &[]),
    }
}

impl Action {
    /// The action's `type`, as it is written on the wire.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Show { .. } => "show",
            Self::SwitchAccount { .. } => "switch_account",
            Self::SetPreference { .. } => "set_preference",
            Self::SaveContact { .. } => "save_contact",
            Self::DeleteContact { .. } => "delete_contact",
            Self::SaveGroup { .. } => "save_group",
            Self::DeleteGroup { .. } => "delete_group",
            Self::SetGroupMembers { .. } => "set_group_members",
            Self::SetContactGroups { .. } => "set_contact_groups",
            Self::RemoveWallet { .. } => "remove_wallet",
            Self::SignOut => "sign_out",
            Self::EraseDevice => "erase_device",
            Self::ClearStorageItem { .. } => "clear_storage_item",
            Self::ClearCaches => "clear_caches",
            Self::ConfigureAgent { .. } => "configure_agent",
            Self::SetAgentGrant { .. } => "set_agent_grant",
            Self::PrepareTransfer { .. } => "prepare_transfer",
            Self::SubmitTransfer { .. } => "submit_transfer",
            Self::CancelTransfer { .. } => "cancel_transfer",
            Self::PrepareBatchTransfer { .. } => "prepare_batch_transfer",
            Self::AddToken { .. } => "add_token",
            Self::DeleteActivity { .. } => "delete_activity",
            Self::SetBalancePrivacy { .. } => "set_balance_privacy",
            Self::RefreshBalances => "refresh_balances",
            Self::SetContactFavorite { .. } => "set_contact_favorite",
            Self::AddNetwork { .. } => "add_network",
            Self::SetNetworkEndpoint { .. } => "set_network_endpoint",
            Self::RemoveNetwork { .. } => "remove_network",
            Self::SetProviderKey { .. } => "set_provider_key",
            Self::SetServiceEndpoint { .. } => "set_service_endpoint",
            Self::ResetServiceEndpoints => "reset_service_endpoints",
        }
    }

    #[must_use]
    pub fn risk(&self) -> Risk {
        // Usage statistics leave the device: not an ordinary preference.
        if let Self::SetPreference {
            preference: Preference::Analytics(_),
        } = self
        {
            return Risk::Manage;
        }
        catalog_entry(self.kind()).map_or(Risk::Manage, |entry| entry.risk)
    }

    #[must_use]
    pub fn person_only(&self) -> bool {
        catalog_entry(self.kind()).is_some_and(|entry| entry.person_only)
    }
}

// ---------------------------------------------------------------------------
// Operations: what a transfer goes through after it is prepared
// ---------------------------------------------------------------------------

/// Where a transfer stands — the same words on every shell, read from the
/// machines that own each part (the send, the passkey, the chain tracker).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentOperationState"))]
pub enum OperationState {
    /// On the review screen, waiting for confirm.
    AwaitingConfirmation,
    /// Confirmed; the passkey prompt is up for the person.
    AwaitingPasskey,
    /// Signed; on its way to the relay.
    Submitting,
    /// The relay took it; not yet on chain.
    Submitted,
    Confirmed,
    Failed,
    /// Left without sending.
    Cancelled,
}

impl OperationState {
    /// Nothing more will happen to it.
    #[must_use]
    pub fn is_final(self) -> bool {
        matches!(self, Self::Confirmed | Self::Failed | Self::Cancelled)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentOperation"))]
pub struct Operation {
    pub id: String,
    pub state: OperationState,
    pub chain_id: Option<u32>,
    /// Known once signed.
    pub user_op_hash: Option<String>,
    /// Known once on chain.
    pub tx_hash: Option<String>,
    /// Why it failed or was cancelled, in the owner's words.
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// Refusals
// ---------------------------------------------------------------------------

/// Why a call ended without its change — the same codes on every shell, for
/// an agent to act on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentReasonCode"))]
pub enum ReasonCode {
    /// The input is malformed or out of range.
    InvalidInput,
    /// What it names does not exist.
    NotFound,
    /// The wallet has not loaded what this needs yet.
    NotReady,
    /// Only the person may do this.
    PersonOnly,
    /// The AI agent is switched off.
    AgentOff,
    /// The person said no.
    Denied,
    /// Nobody answered the approval in time.
    ApprovalTimedOut,
    /// The person stopped the agent.
    Stopped,
    /// The machine or service that owns the rule refused it.
    Refused,
    /// The change was asked for but not seen in time.
    NotObserved,
    /// The person has not allowed an agent to do this; they do it themselves.
    NotGranted,
    /// Something of the person's is under way where this would go (an open
    /// send); finish or close it first.
    Busy,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentReason"))]
pub struct Reason {
    pub code: ReasonCode,
    pub detail: String,
}

fn reason(code: ReasonCode, detail: impl Into<String>) -> Reason {
    Reason {
        code,
        detail: detail.into(),
    }
}

fn invalid(detail: impl Into<String>) -> Result<(), Reason> {
    Err(reason(ReasonCode::InvalidInput, detail))
}

// ---------------------------------------------------------------------------
// Validation: the input's own shape
// ---------------------------------------------------------------------------

fn address(value: &str) -> Result<(), Reason> {
    if is_address(value.trim()) {
        Ok(())
    } else {
        invalid(format!(
            "not an address: {value} (0x followed by 40 hex digits)"
        ))
    }
}

fn named(value: &str, what: &str) -> Result<(), Reason> {
    if value.trim().is_empty() {
        invalid(format!("{what} must not be empty"))
    } else {
        Ok(())
    }
}

fn one_of(value: &str, allowed: &[&str], what: &str) -> Result<(), Reason> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        invalid(format!("{what} must be one of: {}", allowed.join(", ")))
    }
}

fn preference(preference: &Preference) -> Result<(), Reason> {
    match preference {
        Preference::Theme(value) => one_of(value, &prefs::THEMES, "theme"),
        Preference::Language(value) => {
            let mut allowed = vec![prefs::AUTO_LANGUAGE];
            allowed.extend(i18n::resolve::SUPPORTED);
            one_of(value, &allowed, "language")
        }
        Preference::TextScale(value) => {
            let levels: Vec<&str> = prefs::TEXT_SCALE_LEVELS
                .iter()
                .map(|(name, _)| *name)
                .collect();
            one_of(value, &levels, "text_scale")
        }
        Preference::NumberFormat(value) => one_of(value, &prefs::NUMBER_FORMATS, "number_format"),
        Preference::DateFormat(value) => one_of(value, &prefs::DATE_FORMATS, "date_format"),
        Preference::TimeFormat(value) => one_of(value, &prefs::TIME_FORMATS, "time_format"),
        Preference::Currency(code) => {
            if code.len() == 3 && code.bytes().all(|byte| byte.is_ascii_uppercase()) {
                Ok(())
            } else {
                invalid("currency must be an ISO 4217 code, e.g. USD")
            }
        }
        Preference::FeeSpeed(tier) => {
            if fee_tier_pref::OFFERED.contains(tier) {
                Ok(())
            } else {
                invalid("fee_speed must be one of: fast, standard, slow")
            }
        }
        Preference::Analytics(_) => Ok(()),
    }
}

/// The action's own shape — what can be known without looking anything up.
/// Whether what it names exists is the shell's to check, against the views
/// it holds.
///
/// # Errors
///
/// [`ReasonCode::InvalidInput`], naming what is wrong.
pub fn validate(action: &Action) -> Result<(), Reason> {
    match action {
        Action::Show { screen } => match screen {
            Screen::Home | Screen::Contacts | Screen::Receive { chain_id: None } => Ok(()),
            Screen::Receive {
                chain_id: Some(chain),
            } => chain_named(*chain),
            Screen::Asset { chain_id, token } => {
                chain_named(*chain_id)?;
                token.as_deref().map_or(Ok(()), address)
            }
            Screen::Activity { id } => named(id, "activity id"),
            Screen::Contact { address: value } => address(value),
            Screen::Group { id } => named(id, "group id"),
            Screen::Settings { page } => match page {
                Some(page) if !SETTINGS_PAGES.contains(&page.as_str()) => {
                    invalid(format!("no settings page is called {page}"))
                }
                _ => Ok(()),
            },
        },
        Action::SwitchAccount { address: value }
        | Action::RemoveWallet { address: value }
        | Action::DeleteContact { address: value } => address(value),
        Action::SetPreference { preference: value } => preference(value),
        Action::SaveContact {
            address: value,
            name,
        } => address(value).and_then(|()| named(name, "a contact's name")),
        Action::SaveGroup { id, name } => {
            if let Some(id) = id {
                named(id, "group id")?;
            }
            named(name, "a group's name")
        }
        Action::DeleteGroup { id } => named(id, "group id"),
        Action::SetGroupMembers { id, members } => {
            named(id, "group id")?;
            members.iter().try_for_each(|member| address(member))
        }
        Action::SetContactGroups {
            address: value,
            group_ids,
        } => {
            address(value)?;
            group_ids.iter().try_for_each(|id| named(id, "group id"))
        }
        Action::ClearStorageItem { item } => {
            let ids: Vec<&str> = storage_catalog::ITEMS
                .iter()
                .map(|known| known.id)
                .collect();
            one_of(item, &ids, "item")
        }
        Action::PrepareTransfer {
            chain_id,
            token,
            to,
            amount,
        } => {
            if *chain_id == 0 {
                return invalid("chain_id must name a chain");
            }
            if let Some(token) = token {
                address(token)?;
            }
            address(to)?;
            transfer_amount(amount)
        }
        Action::SubmitTransfer { operation } | Action::CancelTransfer { operation } => {
            named(operation, "operation")
        }
        Action::PrepareBatchTransfer {
            chain_id,
            token,
            recipients,
        } => {
            chain_named(*chain_id)?;
            if let Some(token) = token {
                address(token)?;
            }
            if recipients.is_empty() || recipients.len() > MAX_RECIPIENTS {
                return invalid(format!("recipients: 1 to {MAX_RECIPIENTS}"));
            }
            recipients.iter().try_for_each(|recipient| {
                address(&recipient.to)?;
                if recipient.amount.trim() == "max" {
                    return invalid("each recipient needs an amount; \"max\" is for one recipient");
                }
                transfer_amount(&recipient.amount)
            })
        }
        Action::AddToken {
            chain_id,
            address: contract,
        } => chain_named(*chain_id).and_then(|()| address(contract)),
        Action::DeleteActivity { id } => named(id, "activity id"),
        Action::SetContactFavorite { address: value, .. } => address(value),
        Action::SetBalancePrivacy { .. } | Action::RefreshBalances => Ok(()),
        Action::AddNetwork { chain_id, rpc_url } => {
            chain_named(*chain_id)?;
            rpc_url.as_deref().map_or(Ok(()), web_url)
        }
        Action::SetNetworkEndpoint { chain_id, url, .. } => {
            chain_named(*chain_id)?;
            if url.trim().is_empty() {
                Ok(())
            } else {
                web_url(url)
            }
        }
        Action::RemoveNetwork { chain_id } => chain_named(*chain_id),
        Action::SetProviderKey { key, .. } => {
            if key.chars().any(char::is_whitespace) && !key.trim().is_empty() {
                invalid("a key has no spaces in it")
            } else {
                Ok(())
            }
        }
        // Empty is the field cleared: the service's default again.
        Action::SetServiceEndpoint { url, .. } => {
            if url.trim().is_empty() {
                Ok(())
            } else {
                web_url(url)
            }
        }
        Action::ResetServiceEndpoints => Ok(()),
        Action::SignOut
        | Action::EraseDevice
        | Action::ClearCaches
        | Action::ConfigureAgent { .. }
        | Action::SetAgentGrant { .. } => Ok(()),
    }
}

/// The most recipients one transfer may have — the send's own cap.
pub const MAX_RECIPIENTS: usize = BATCH_MAX_RECIPIENTS;

/// A dot-decimal amount (no `max`).
pub const DECIMAL_PATTERN: &str = r"^([0-9]+(\.[0-9]+)?|\.[0-9]+)$";

/// An http(s) URL with something after the scheme.
fn web_url(url: &str) -> Result<(), Reason> {
    let url = url.trim();
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
        .unwrap_or_default();
    if rest.is_empty() || rest.chars().any(char::is_whitespace) {
        invalid(format!("not an http(s) URL: {url}"))
    } else {
        Ok(())
    }
}

fn chain_named(chain_id: u32) -> Result<(), Reason> {
    if chain_id == 0 {
        invalid("chain_id must name a chain")
    } else {
        Ok(())
    }
}

/// What a transfer's `amount` may be: `max`, or a dot-decimal above zero.
pub const AMOUNT_PATTERN: &str = r"^(max|[0-9]+(\.[0-9]+)?|\.[0-9]+)$";

fn transfer_amount(amount: &str) -> Result<(), Reason> {
    let amount = amount.trim();
    if amount == "max" {
        return Ok(());
    }
    let (whole, fraction) = amount.split_once('.').unwrap_or((amount, ""));
    let digits = |part: &str| part.bytes().all(|byte| byte.is_ascii_digit());
    let well_formed = !(whole.is_empty() && fraction.is_empty())
        && digits(whole)
        && digits(fraction)
        && !(amount.contains('.') && fraction.is_empty());
    if !well_formed {
        return invalid("amount must be a dot-decimal in the token's units, or \"max\"");
    }
    if amount.bytes().all(|byte| byte == b'0' || byte == b'.') {
        return invalid("amount must be above zero");
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Authorization: who may
// ---------------------------------------------------------------------------

/// What the person has allowed an agent, as Settings → AI agent holds it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentGrants"))]
pub struct Grants {
    /// The AI agent is switched on.
    pub agent_enabled: bool,
    /// An agent may submit a transaction for the person's passkey.
    pub submit_transactions: bool,
}

/// Who approves a request before it is done.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentApprover"))]
pub enum Approver {
    /// The wallet's own approval: the request, shown to the person to allow
    /// or deny.
    Wallet,
    /// The confirmation of the machine that owns the rule — the session's
    /// sign-out sheet, which carries the unsent-data warning a generic
    /// approval would not.
    Owner,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentDecision"))]
pub enum Decision {
    /// Do it.
    Allow,
    /// Do it once the person approves it.
    Approve { by: Approver },
    /// Do not.
    Deny { reason: Reason },
}

/// Whether `actor` may ask for `action`.
///
/// A person always may: their UI has already asked them anything worth
/// asking. An agent may not touch its own connection, may not act while it
/// is switched off, and waits for the person on anything manage-level.
#[must_use]
pub fn authorize(actor: Actor, action: &Action, grants: &Grants) -> Decision {
    if actor == Actor::Person {
        return Decision::Allow;
    }
    if action.person_only() {
        return Decision::Deny {
            reason: reason(ReasonCode::PersonOnly, "only the person may change this"),
        };
    }
    if !grants.agent_enabled {
        return Decision::Deny {
            reason: reason(ReasonCode::AgentOff, "the AI agent is switched off"),
        };
    }
    match action {
        // A draft moves nothing: the review it lands on is where the person
        // decides, as for one they filled in themselves.
        Action::PrepareTransfer { .. } | Action::PrepareBatchTransfer { .. } => {
            return Decision::Allow
        }
        // Pressing confirm is the person's, unless they allowed the agent —
        // and even then their passkey signs, or nothing does.
        Action::SubmitTransfer { .. } => {
            return if grants.submit_transactions {
                Decision::Allow
            } else {
                Decision::Deny {
                    reason: reason(
                        ReasonCode::NotGranted,
                        "the person confirms this transfer themselves on the review screen",
                    ),
                }
            };
        }
        _ => {}
    }
    match action.risk() {
        Risk::Observe | Risk::Operate => Decision::Allow,
        Risk::Manage | Risk::Transact => Decision::Approve {
            by: if matches!(action, Action::SignOut) {
                Approver::Owner
            } else {
                Approver::Wallet
            },
        },
    }
}

/// [`authorize`] and [`validate`] together, for a shell holding the action as
/// JSON: what it is, how much it matters, and what to do.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "IntentCheck"))]
pub struct Check {
    /// The action's `type`; absent when the JSON is no action.
    pub kind: Option<String>,
    pub risk: Option<Risk>,
    pub decision: Decision,
}

/// Check one request. Refusals of WHO come before refusals of WHAT: an agent
/// asking for the person's own setting is told that, however it asked.
#[must_use]
pub fn check(actor: Actor, action: &Action, grants: &Grants) -> Check {
    let decision = match authorize(actor, action, grants) {
        deny @ Decision::Deny { .. } => deny,
        allowed => match validate(action) {
            Ok(()) => allowed,
            Err(reason) => Decision::Deny { reason },
        },
    };
    Check {
        kind: Some(action.kind().to_owned()),
        risk: Some(action.risk()),
        decision,
    }
}

/// [`check`] for an action that arrives as JSON (an agent wrote it): one that
/// does not parse is refused, saying why.
#[must_use]
pub fn check_json(actor: Actor, action_json: &str, grants: &Grants) -> Check {
    match serde_json::from_str::<Action>(action_json) {
        Ok(action) => check(actor, &action, grants),
        Err(error) => Check {
            kind: None,
            risk: None,
            decision: Decision::Deny {
                reason: reason(ReasonCode::InvalidInput, format!("not an action: {error}")),
            },
        },
    }
}
