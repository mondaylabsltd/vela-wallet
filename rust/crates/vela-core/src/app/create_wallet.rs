//! Machine A — creating a wallet.
//!
//! ```text
//! Form ─submit─► CheckingSupport ─► Registering(key 1) ─► AddKeys
//!   ▲                                    ▲                  │ ▲
//!   │ cancel-at-register KEEPS drafts    └──add_key(≤7)─────┘ │
//!   │                                                finish_keys
//!   │                                                        ▼
//!   └────────────── SavingPending ─► Syncing{publish → remove pending}
//!                                                        ─► Saving ─► Created
//! ```
//!
//! The ordering is the product. The passkey must first produce a valid
//! signature (a provider can report `create()` success and still have stored
//! nothing — issue #1). A wallet is 1..=7 founding passkeys, all minted here:
//! the Safe address is a function of the FULL key set, so keys can never be
//! added later. The wallet is then published to the registry as a
//! possession-proven group *before* it is entered (option B): the same key set
//! that derives the address is the group's membership. For a multi-key wallet
//! that publish is load-bearing for recovery — a sibling device can only
//! reconstruct the address from the registry group — which is exactly why the
//! account is saved and enterable only after the publish has landed on-chain.
//! A publish failure surfaces the retry screen while the passkeys and their
//! drafts are kept; a single key additionally stays recoverable on-device
//! from two signatures.

use crux_core::{command::AbortHandle, render::render, App, Command};
use serde::{Deserialize, Serialize};

use super::shell::{CompletionMode, Effect, ShellOperation, ShellResult};
use super::{
    name_fits_user_handle, public_key_hex_from_attestation, Account, AccountKey, FailureKind,
    KeyMethod, PendingUpload, PromptKind, RegistryPublishMember, StatusKey,
};
use crate::error::CoreError;
use crate::registry_metadata::{RegistryMetadata, REGISTRY_METADATA_VERSION};
use crate::registry_proof::RegistryProof;

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// The acknowledgment checklist. Three rows, all required — the gate is a
/// business rule, not a UI decoration.
///
/// Each row is a FACT about where something ends up, and the three together are
/// the whole custody story a person is agreeing to:
///
/// * Row 0 — the public key and the wallet name are written into the on-chain
///   contract. Public, permanent, and not deletable later.
/// * Row 1 — the private key stays in the device's credential manager or on a
///   security key. Vela never sees it, and therefore cannot recover it.
/// * Row 2 — legal assent to the privacy policy and the terms.
///
/// It was four, then two, and is three. The four included two rows that said
/// true things about recovery and about a compromised provider account, and a
/// checklist people tick without reading records nothing — so those became
/// assurances beside the gate. Two turned out to be the wrong two: the pair
/// named where the PRIVATE key lives and what the user agrees to, and never
/// said that the public key and the chosen name go on-chain in the clear. That
/// is the one consequence of creating a wallet that cannot be undone
/// afterwards, so it is now a gate rather than something a person discovers.
///
/// The recovery assurance that used to sit between them is gone from this
/// screen: it described a benefit, and this list is only for the facts a person
/// is consenting to.
pub const ACK_COUNT: usize = 3;

/// The Safe deployment this wallet uses, recorded in the registry metadata.
const WALLET_VERSION: &str = "safe-1.4.1";

/// The reason some methods are missing, when some are.
fn blocked_reason(drafts: &[Draft], signer_page: &str) -> Option<AddBlocked> {
    let committed = committed_relying_party(drafts)?;
    if methods_for(drafts, signer_page).len() == 4 {
        return None;
    }
    let page_rp = crate::clear_signer::registry_rp_id(Some(signer_page))
        .unwrap_or_else(|| "getvela.app".to_owned());
    let page_differs = page_rp != committed;
    Some(AddBlocked {
        relying_party: committed,
        page: page_differs.then(|| {
            if signer_page.trim().is_empty() {
                crate::clear_signer::DEFAULT_SIGNER_URL.to_owned()
            } else {
                signer_page.to_owned()
            }
        }),
        page_relying_party: page_differs.then_some(page_rp),
    })
}

/// Why a key method is not on offer (spec 075). Carries the two facts the
/// sentence needs, so the words live in the shells' catalogues and the
/// judgement lives here.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct AddBlocked {
    /// The relying party every key in this wallet belongs to.
    pub relying_party: String,
    /// The Clear Signer page Settings names, when it is the thing that does
    /// not fit — a key minted there would belong to `page_relying_party`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
    /// That page's relying party, when it differs from the wallet's.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_relying_party: Option<String>,
}

/// The relying party this key set has committed to, or `None` while it is
/// empty (ruling, 2026-09-23).
///
/// A key minted on a Clear Signer page belongs to that page's domain; every
/// other route mints a key of the wallet's own relying party, and so does the
/// official page. The first key decides, because the registry stores ONE
/// `rpId` per unit and a member can only ever prove membership under its own.
fn committed_relying_party(drafts: &[Draft]) -> Option<String> {
    let first = drafts.first()?;
    Some(
        crate::clear_signer::registry_rp_id(first.signer_origin.as_deref())
            .unwrap_or_else(|| "getvela.app".to_owned()),
    )
}

/// Which methods may still mint a key for this set.
///
/// Everything, until the set belongs to somebody's own deployment — then only
/// the Clear Signer, because that page is the only thing that can mint another
/// key of that domain. Offering the rest would let a person mint a key that
/// can never join this wallet's unit, which is only discovered at the publish.
fn methods_for(drafts: &[Draft], signer_page: &str) -> Vec<KeyMethod> {
    const OWN: [KeyMethod; 3] = [
        KeyMethod::Platform,
        KeyMethod::Hybrid,
        KeyMethod::SecurityKey,
    ];
    // Where the Clear Signer would mint: the page Settings names. The official
    // one is a `getvela.app` page, so a key from it joins a set of the app's
    // own keys; somebody's own deployment mints for its own domain, and that
    // key can only ever join a set of ITS keys.
    let page_rp = crate::clear_signer::registry_rp_id(Some(signer_page))
        .unwrap_or_else(|| "getvela.app".to_owned());
    let mut allowed: Vec<KeyMethod> = Vec::new();
    match committed_relying_party(drafts) {
        // Nothing minted yet: any route may start the set, and whatever it
        // picks is what the rest must match.
        None => {
            allowed.extend(OWN);
            allowed.push(KeyMethod::ClearSigner);
        }
        Some(committed) => {
            if committed == "getvela.app" {
                allowed.extend(OWN);
            }
            // …and the page only when it would mint for the same party. This
            // is the case the owner hit on 2026-09-23: a set of `getvela.app`
            // keys, a signer page on `localhost`, and the Clear Signer still
            // offered — so a key was minted that nothing would accept, and the
            // wallet only said so at the publish.
            if page_rp == committed {
                allowed.push(KeyMethod::ClearSigner);
            }
        }
    }
    allowed
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "CreateWalletEvent"))]
pub enum Event {
    Start,
    NameChanged {
        name: String,
    },
    AckToggled {
        index: usize,
    },
    /// The primary button: "Create Wallet", or "Finish verification" when a
    /// draft is waiting.
    Submit,
    /// From the key list: mint one more founding passkey with this label
    /// (empty ⇒ "Key N") through the chosen kind of authenticator. Capped at
    /// `MAX_MULTI_KEYS`.
    AddKey {
        name: String,
        #[serde(default)]
        method: KeyMethod,
    },
    /// The Clear Signer page Settings names, so this machine can tell whether
    /// that route would mint a key this set can accept. Sent on entry and
    /// whenever the preference changes; empty means the official page.
    SignerPageChanged {
        url: String,
    },
    /// Drop a not-yet-published draft key. Index 0 (the wallet's pinned first
    /// key) is only removable via `StartOver`.
    RemoveKey {
        index: usize,
    },
    /// Relabel a draft key. Index 0's label is the wallet name itself.
    KeyNameChanged {
        index: usize,
        name: String,
    },
    /// Re-run a drafted key's creation-time membership confirmation — the
    /// recovery for a cancelled or failed `SignMemberProof`.
    ConfirmKey {
        index: usize,
    },
    /// Done adding keys — derive the address from the FULL set and publish.
    FinishKeys,
    /// Abandon the drafted passkeys and start from a clean form.
    StartOver,
    /// Re-run the index upload after it exhausted its retries.
    RetryUpload,
    EnterWallet,
    GoBack,
    /// Internal: an effect resolved. Never sent by a shell — `attempt` is
    /// captured by the core when the request is made, which is what makes a
    /// late result from an abandoned attempt identifiable.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: ShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// A registered passkey that has not yet proven it can sign. Its whole reason
/// for existing is that a cancelled verification must resume from the
/// *signature*, never mint a second passkey.
#[derive(Clone, Debug, Default)]
pub struct Draft {
    pub credential_id: String,
    pub attestation_object_hex: String,
    pub name: String,
    pub registered_at_iso: String,
    /// Browser-reported display hints from the create() credential.
    pub authenticator_attachment: String,
    pub transports: String,
    /// Which kind of authenticator the person asked for. Recorded so a later
    /// step can return to the SAME authenticator (the membership confirmation
    /// is a `get()` against the credential just minted). It is not what the row
    /// says: the row reads the hints above, through `CreateKeyRow::kind`
    /// (issue #207).
    pub method: KeyMethod,
    /// Extracted at registration (the create() response carries the COSE
    /// key), so a duplicate authenticator is caught the moment it appears.
    pub public_key_hex: String,
    /// The 20-byte versioned attestation, or empty.
    pub attestation_hex: String,
    /// The creation-time membership proof — collected right after the
    /// registration, one `get()` per key, interleaved. `None` until signed
    /// (or after a cancelled confirmation).
    pub proof: Option<RegistryProof>,
    /// Spec 075: the Clear Signer page the key was minted behind, if any —
    /// where its membership confirmation must be signed too.
    pub signer_origin: Option<String>,
}

/// One derived founding key, canonical founding order.
#[derive(Clone, Debug, Default)]
pub struct PreparedKey {
    pub credential_id: String,
    /// The per-key label; `keys[0].name` is the wallet name.
    pub name: String,
    pub public_key_hex: String,
    /// The full attestation object (hex) — kept for the pending record.
    pub attestation_object_hex: String,
    /// The 20-byte versioned attestation extracted from the attestation
    /// object, or empty when it could not be extracted. Bound into the
    /// registry member proof.
    pub attestation_hex: String,
    /// Browser-reported display hints captured from the create() credential.
    pub authenticator_attachment: String,
    pub transports: String,
    /// The creation-time membership proof.
    pub proof: Option<RegistryProof>,
    /// Spec 075: the Clear Signer page the key was minted behind, if any.
    pub signer_origin: Option<String>,
}

/// Derived from the drafts, not yet persisted anywhere. `keys[0]` is the
/// pinned shared-signer key; the address is a function of the FULL set.
#[derive(Clone, Debug, Default)]
pub struct Prepared {
    pub keys: Vec<PreparedKey>,
    pub address: String,
    pub created_at_iso: String,
}

impl Prepared {
    /// The first (pinned) key — the wallet's stable identity.
    fn first(&self) -> &PreparedKey {
        &self.keys[0]
    }

    fn account(&self) -> Account {
        let first = self.first();
        Account {
            id: first.credential_id.clone(),
            name: first.name.clone(),
            address: self.address.clone(),
            public_key_hex: first.public_key_hex.clone(),
            created_at_iso: self.created_at_iso.clone(),
            keys: self
                .keys
                .iter()
                .map(|key| AccountKey {
                    credential_id: key.credential_id.clone(),
                    public_key_hex: key.public_key_hex.clone(),
                    name: key.name.clone(),
                    transports: key.transports.clone(),
                    signer_origin: key.signer_origin.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SyncState {
    /// 1-based attempt number of the run in flight; 0 when idle.
    pub tries: u8,
    /// Why the last attempt failed — shown behind the technical-details
    /// disclosure and carried into the bug report.
    pub last_error: Option<String>,
    /// A failed `create` is not yet a failure: the query below decides. Kept so
    /// the original cause is what surfaces if confirmation also fails.
    pub create_error: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SyncStep {
    /// Running the possession-proven publish (option B: before entering).
    #[default]
    Publishing,
    /// Clearing the pending-upload record after a successful publish.
    RemovingPending,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Form,
    CheckingSupport,
    /// Minting the one-time software group key (shell randomness) — the
    /// anchor every member's creation-time proof binds to.
    GeneratingGroupKey,
    Registering,
    /// A freshly registered key is confirming its membership (one `get()`
    /// over the member-mode challenge) — interleaved, per key.
    SigningKey,
    /// The key list between registration and derivation: add, relabel or
    /// remove founding keys, then `FinishKeys` freezes the set.
    AddKeys,
    SavingPending,
    Syncing(SyncStep),
    Saving,
    SyncFailed,
    Created,
    Completing,
}

impl Stage {
    /// Is an operation in flight? Everything that is not a resting place.
    fn is_busy(&self) -> bool {
        !matches!(
            self,
            Stage::Form | Stage::AddKeys | Stage::SyncFailed | Stage::Created
        )
    }
}

#[derive(Default)]
pub struct Model {
    name: String,
    acks: [bool; ACK_COUNT],
    /// The Clear Signer page Settings names — empty means the official one.
    /// It decides whether that route can mint a key THIS set accepts, which
    /// depends on the page's domain rather than on the route (spec 075).
    signer_page: String,
    /// Founding-order draft keys; `drafts[0]` is the pinned first key.
    drafts: Vec<Draft>,
    /// The label of the registration currently in flight, claimed by the
    /// `PasskeyRegistered` result.
    registering_label: String,
    /// The method of that same in-flight registration, claimed alongside it.
    registering_method: KeyMethod,
    /// The one-time group key the shell minted for this run. Every member's
    /// creation-time proof binds to its public key; the seed closes the
    /// group at publish. Cleared by StartOver.
    group_seed_hex: Option<String>,
    group_public_key_hex: Option<String>,
    /// Which draft the in-flight `SignMemberProof` belongs to.
    signing_index: Option<usize>,
    prepared: Option<Prepared>,
    sync: SyncState,
    stage: Stage,
    status: Option<StatusKey>,
    /// Bumped on every user-initiated start. A result carrying a different
    /// attempt belongs to an abandoned run and is dropped.
    attempt: u64,
    abort: Option<AbortHandle>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum CreateStage {
    /// The form panel — also what shows while work is in flight, with `busy`
    /// driving the button spinner. Matches today's screen exactly.
    Form,
    /// The founding-key list: rows in `keys`, plus add/relabel/remove and the
    /// finish button. Also shown (busy) while an added key's registration is
    /// in flight.
    AddKeys,
    SyncFailed,
    Created,
}

/// One row of the founding-key list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct CreateKeyRow {
    /// The per-key label; row 0 carries the wallet name.
    pub name: String,
    /// Browser-reported display hints ("platform"/"cross-platform", comma-
    /// joined transports) — purely informational.
    pub authenticator_attachment: String,
    pub transports: String,
    /// The key confirmed its group membership at creation. A `false` row
    /// (cancelled confirmation) offers a per-row retry, and FinishKeys is
    /// gated on every row being `true`.
    pub confirmed: bool,
    /// Backed up to a sync fabric (authenticatorData BS flag). Unknown
    /// attestation reads as `true` — display and the second-key gate both
    /// fail open.
    pub synced: bool,
    /// Is [`Self::synced`] a FACT, or the benefit of the doubt? `false` when
    /// the attestation blob is unreadable and the `true` above is the gate
    /// failing open. The gate keeps failing open; a row with no answer draws no
    /// badge instead of a green "Cloud-synced" nobody verified (issue #207).
    pub synced_known: bool,
    /// The authenticator model's AAGUID as a canonical uuid, or empty when
    /// absent/all-zero. Shells pass it back to the core for the provider's
    /// mark (`passkey_provider_png` / `passkeyProviderIconDataUri`).
    pub aaguid: String,
    /// The vault holding this key, resolved from [`Self::aaguid`] against the
    /// vendored catalog: "Apple Passwords", "1Password", "Windows Hello".
    /// Empty when the catalog does not know the model — hardware keys and
    /// attestation-less registrations both land there — and the shells then
    /// say what they always said, from [`Self::kind`].
    ///
    /// Resolved HERE rather than in each shell so that four clients cannot
    /// disagree about who holds a key (and so the lookup stays offline: asking
    /// a directory service would tell it which vault holds a Vela wallet).
    pub provider_name: String,
    /// Which kind of authenticator the person chose for this key — the tap in
    /// the method picker, kept for ceremony ROUTING only (which authenticator a
    /// later confirmation must return to). It is not what the row says.
    pub method: KeyMethod,
    /// What this key IS, from the authenticator's own report — where it lives.
    /// Rows draw their icon AND their caption from this one field, so the two
    /// cannot disagree (issue #207: a hardware-fob icon beside "Passkey" beside
    /// "This device only", from three unrelated signals). Falls back to
    /// [`Self::method`] when the authenticator reported nothing at all.
    pub kind: KeyMethod,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SubmitLabel {
    Create,
    FinishVerify,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct CreateView {
    pub stage: CreateStage,
    pub name: String,
    pub name_editable: bool,
    pub name_too_long: bool,
    pub acks: Vec<bool>,
    pub can_submit: bool,
    pub submit_label: SubmitLabel,
    pub busy: bool,
    pub status: Option<StatusKey>,
    pub show_start_over: bool,
    /// Present only once the wallet is real. Showing an address earlier would
    /// invite funding a wallet that may never be reachable.
    pub address: Option<String>,
    pub sync_error_detail: Option<String>,
    pub can_go_back: bool,
    /// The drafted founding keys, founding order. Non-empty from the moment
    /// the first registration lands.
    pub keys: Vec<CreateKeyRow>,
    /// May one more key be added (below the cap, nothing in flight)?
    pub can_add_key: bool,
    /// Spec 075 (ruling, 2026-09-23): the relying party this key set has
    /// committed to, once its first key exists.
    ///
    /// The registry stores ONE `rpId` per unit and every member's proof
    /// carries `sha256(rpId)` from its own authenticator, so a set spread over
    /// two sites can never be proved. The first key therefore decides where
    /// the rest must come from. `None` before there is a key.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_relying_party: Option<String>,
    /// The page that relying party lives on, when it is a Clear Signer page —
    /// so a shell can say WHICH page the remaining keys must be minted on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_signer_origin: Option<String>,
    /// Which methods may still mint a key for this set. Every method while the
    /// set is empty; afterwards only those that would mint for the relying
    /// party it committed to — which for the Clear Signer depends on the page
    /// Settings names, not on the route.
    pub add_methods: Vec<KeyMethod>,
    /// Why the others are not offered, when some are missing. A sentence a
    /// person can act on: what this wallet's keys belong to, and what the
    /// route they just reached for would have made instead.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub add_blocked: Option<AddBlocked>,
    /// May the key set be frozen and published (≥1 key, nothing in flight)?
    pub can_finish: bool,
    /// The sole drafted key is NOT a synced passkey: one lost device would
    /// make the wallet unrecoverable, so finishing requires a second key.
    /// Drives the friendly hint under the key list.
    pub needs_second_key: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct CreateWallet;

impl App for CreateWallet {
    type Event = Event;
    type Model = Model;
    type ViewModel = CreateView;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Start => render(),
            Event::NameChanged { name } => {
                if model.stage.is_busy() || !model.drafts.is_empty() {
                    return Command::done(); // the input is not editable then
                }
                model.name = name;
                render()
            }
            Event::AckToggled { index } => {
                if model.stage.is_busy() || index >= ACK_COUNT {
                    return Command::done();
                }
                model.acks[index] = !model.acks[index];
                render()
            }
            Event::Submit => submit(model),
            Event::AddKey { name, method } => add_key(model, name, method),
            Event::SignerPageChanged { url } => {
                model.signer_page = url;
                render()
            }
            Event::RemoveKey { index } => remove_key(model, index),
            Event::KeyNameChanged { index, name } => key_name_changed(model, index, name),
            Event::ConfirmKey { index } => confirm_key(model, index),
            Event::FinishKeys => finish_keys(model),
            Event::StartOver => start_over(model),
            Event::RetryUpload => retry_upload(model),
            Event::EnterWallet => enter_wallet(model),
            Event::GoBack => {
                // The ONLY step this machine can return to is the form, and
                // only from the key list. From the form itself there is
                // nowhere to go but out of the flow, which is the host's to
                // do — `can_go_back` says so, and a host that asked anyway
                // used to get a re-render that changed nothing and a back
                // affordance that did nothing (device-found 2026-08-25).
                //
                // The drafts survive: the form re-entered with drafts is the
                // "finish verification" state, and its submit returns here.
                if model.stage != Stage::AddKeys {
                    return Command::done();
                }
                model.stage = Stage::Form;
                model.status = None;
                render()
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A result from an abandoned run. Dropping it is the whole
                    // reason `attempt` exists: without it, a late upload result
                    // could resurrect a draft the user already abandoned.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> CreateView {
        let trimmed = model.name.trim();
        let name_too_long = !name_fits_user_handle(trimmed);
        let busy = model.stage.is_busy();
        let has_draft = !model.drafts.is_empty();

        let stage = match model.stage {
            Stage::SyncFailed => CreateStage::SyncFailed,
            Stage::Created | Stage::Completing => CreateStage::Created,
            Stage::AddKeys => CreateStage::AddKeys,
            // Every registration and membership confirmation is launched from
            // the key list, so an in-flight one keeps it on screen — including
            // the first key's, whose list is still empty.
            Stage::Registering | Stage::SigningKey => CreateStage::AddKeys,
            _ => CreateStage::Form,
        };

        let at_key_list = model.stage == Stage::AddKeys;
        CreateView {
            stage,
            name: model.name.clone(),
            name_editable: !busy && !has_draft,
            name_too_long,
            acks: model.acks.to_vec(),
            can_submit: !busy
                && model.stage == Stage::Form
                && model.acks.iter().all(|checked| *checked)
                && (has_draft || (!trimmed.is_empty() && !name_too_long)),
            submit_label: if has_draft {
                SubmitLabel::FinishVerify
            } else {
                SubmitLabel::Create
            },
            busy,
            status: model.status,
            show_start_over: has_draft && !busy,
            address: match stage {
                CreateStage::Created => model.prepared.as_ref().map(|p| p.address.clone()),
                _ => None,
            },
            sync_error_detail: match stage {
                CreateStage::SyncFailed => model.sync.last_error.clone(),
                _ => None,
            },
            // Does the CORE have a step to go back to? Only the key list
            // does — everywhere else "back" means leaving the flow, which
            // the host owns because the core cannot see what contains it.
            can_go_back: model.stage == Stage::AddKeys,
            keys: model
                .drafts
                .iter()
                .map(|draft| {
                    let (aaguid, synced) = attestation_signals(&draft.attestation_hex);
                    let provider_name = crate::passkey::provider_name(&aaguid)
                        .unwrap_or_default()
                        .to_owned();
                    CreateKeyRow {
                        name: draft.name.clone(),
                        authenticator_attachment: draft.authenticator_attachment.clone(),
                        transports: draft.transports.clone(),
                        confirmed: draft.proof.is_some(),
                        synced,
                        synced_known: crate::passkey::attestation_backed_up(&draft.attestation_hex)
                            .is_some(),
                        aaguid,
                        provider_name,
                        // The page first, when the key lives behind one (spec
                        // 075): a page runs the ceremony in a browser and so
                        // reports `platform`, and drawing "a passkey on this
                        // device" would name the wrong side of the page. Then
                        // the report; the person's tap only when the
                        // authenticator said nothing about itself.
                        kind: if draft
                            .signer_origin
                            .as_deref()
                            .is_some_and(|origin| !origin.is_empty())
                        {
                            KeyMethod::ClearSigner
                        } else {
                            crate::passkey::reported_method(
                                &draft.authenticator_attachment,
                                &draft.transports,
                            )
                            .unwrap_or(draft.method)
                        },
                        method: draft.method,
                    }
                })
                .collect(),
            can_add_key: at_key_list && model.drafts.len() < crate::safe::MAX_MULTI_KEYS,
            key_relying_party: committed_relying_party(&model.drafts),
            key_signer_origin: model
                .drafts
                .first()
                .and_then(|draft| draft.signer_origin.clone())
                .filter(|origin| !origin.is_empty()),
            add_methods: methods_for(&model.drafts, &model.signer_page),
            add_blocked: blocked_reason(&model.drafts, &model.signer_page),
            can_finish: at_key_list
                && has_draft
                && model.drafts.iter().all(|draft| draft.proof.is_some())
                && !needs_second_key(&model.drafts),
            needs_second_key: needs_second_key(&model.drafts),
        }
    }
}

// ---------------------------------------------------------------------------
// User-initiated transitions
// ---------------------------------------------------------------------------

fn submit(model: &mut Model) -> Command<Effect, Event> {
    // Only from the form. `Created` and `SyncFailed` are also "not busy", and a
    // draft outlives a successful creation — so without this guard a stray
    // submit after the wallet exists would start a *second* ceremony for a
    // wallet that is already done. No screen offers that button today; the
    // machine simply must not depend on that staying true.
    if model.stage != Stage::Form || !model.acks.iter().all(|checked| *checked) {
        return Command::done();
    }

    if !model.drafts.is_empty() {
        // Resume: passkeys already exist (never re-registered). Return to the
        // key list — the publish's member proofs are the gets.
        model.stage = Stage::AddKeys;
        model.status = None;
        return render();
    }

    let trimmed = model.name.trim();
    if trimmed.is_empty() || !name_fits_user_handle(trimmed) {
        return Command::done();
    }

    model.attempt += 1;
    model.stage = Stage::CheckingSupport;
    model.status = None;
    request(model, ShellOperation::CheckPasskeySupport)
}

/// The passkey-provider display name for an added key: the wallet name plus
/// the key's label, degraded to the label alone when the composition would
/// not fit the WebAuthn user-handle budget. Key 1 always uses the wallet
/// name itself, keeping N=1 byte-identical to the single-key flow.
fn passkey_display_name(wallet: &str, label: &str) -> String {
    let composed = format!("{wallet} · {label}");
    if name_fits_user_handle(&composed) {
        composed
    } else {
        label.to_owned()
    }
}

fn add_key(model: &mut Model, label: String, method: KeyMethod) -> Command<Effect, Event> {
    if model.stage != Stage::AddKeys || model.drafts.len() >= crate::safe::MAX_MULTI_KEYS {
        return Command::done();
    }
    // A method this set cannot use is refused HERE, not at the publish
    // (ruling, 2026-09-23). The view already offers only `add_methods`, and a
    // shell that asks anyway would otherwise mint a passkey that can never
    // join this wallet's unit — the person would keep a key they cannot use
    // and learn about it minutes later, after the registry refused the set.
    if !methods_for(&model.drafts, &model.signer_page).contains(&method) {
        return Command::done();
    }
    // Key 1's label and provider display name ARE the wallet name (N=1 stays
    // byte-identical to the single-key flow); later keys compose "wallet ·
    // label" and fall back to the label alone when that would not fit.
    let first = model.drafts.is_empty();
    let label = label.trim().to_owned();
    let label = if first {
        model.name.trim().to_owned()
    } else if label.is_empty() {
        format!("Key {}", model.drafts.len() + 1)
    } else {
        label
    };
    if !name_fits_user_handle(&label) {
        return Command::done();
    }
    model.attempt += 1;
    model.registering_label = label.clone();
    model.registering_method = method;
    model.stage = Stage::Registering;
    model.status = Some(StatusKey::SettingUpIdentity);
    let name = if first {
        label.clone()
    } else {
        passkey_display_name(model.name.trim(), &label)
    };
    // The provider must refuse to reuse an already-founding authenticator
    // entry — a silent replacement would drop a key the address depends on.
    let exclude_credential_ids = model
        .drafts
        .iter()
        .map(|draft| draft.credential_id.clone())
        .collect();
    request(
        model,
        ShellOperation::RegisterPasskey {
            name,
            exclude_credential_ids,
            method,
        },
    )
}

fn remove_key(model: &mut Model, index: usize) -> Command<Effect, Event> {
    // Index 0 is the pinned first key — removable only via StartOver.
    if model.stage != Stage::AddKeys || index == 0 || index >= model.drafts.len() {
        return Command::done();
    }
    model.drafts.remove(index);
    render()
}

fn key_name_changed(model: &mut Model, index: usize, name: String) -> Command<Effect, Event> {
    // Row 0's label IS the wallet name; it is not editable here.
    if model.stage != Stage::AddKeys || index == 0 || index >= model.drafts.len() {
        return Command::done();
    }
    let label = name.trim().to_owned();
    if label.is_empty() || !name_fits_user_handle(&label) {
        return Command::done();
    }
    model.drafts[index].name = label;
    render()
}

/// The sync signals inside a 20-byte versioned attestation
/// (`version ‖ AAGUID(16) ‖ authenticatorData flags ‖ reserved(2)`):
/// the AAGUID as a canonical uuid (empty when absent or all-zero) and
/// whether the credential is backed up (BS, bit 4) — a "synced passkey".
///
/// An EMPTY or malformed attestation reads as synced: some authenticators
/// legitimately omit attested-credential data, and the second-key gate must
/// fail open there rather than dead-end an honest provider (same benefit of
/// the doubt `credProps.rk === undefined` gets in the passkey module).
fn attestation_signals(attestation_hex: &str) -> (String, bool) {
    crate::passkey::attestation_signals(attestation_hex)
}

impl Draft {
    /// Is this credential backed up to a sync fabric (BS flag)? Unknown
    /// attestation reads as synced — see [`attestation_signals`].
    fn synced(&self) -> bool {
        attestation_signals(&self.attestation_hex).1
    }
}

/// A single-key wallet whose only key is NOT synced is one lost device away
/// from being unrecoverable — such a set needs a second key before it can
/// freeze. Any second key (even device-bound) breaks the single point of
/// failure; a synced sole key never trips this.
fn needs_second_key(drafts: &[Draft]) -> bool {
    drafts.len() == 1 && !drafts[0].synced()
}

/// Kick off one draft's creation-time membership confirmation.
fn begin_sign_member(model: &mut Model, index: usize) -> Command<Effect, Event> {
    let (Some(group_public_key_hex), Some(draft)) = (
        model.group_public_key_hex.clone(),
        model.drafts.get(index).cloned(),
    ) else {
        return Command::done();
    };
    model.signing_index = Some(index);
    model.stage = Stage::SigningKey;
    model.status = Some(StatusKey::VerifyingIdentity);
    request(
        model,
        ShellOperation::SignMemberProof {
            credential_id: draft.credential_id,
            public_key_hex: draft.public_key_hex,
            attestation_hex: draft.attestation_hex,
            transports: crate::passkey::allowlist_transports(&draft.transports),
            // The route that minted this key confirms it. The draft recorded
            // the person's choice at registration precisely so a later step can
            // return to the same authenticator.
            method: draft.method,
            group_public_key_hex,
            signer_origin: draft.signer_origin,
        },
    )
}

/// Retry a drafted key's membership confirmation (its row's own recovery).
fn confirm_key(model: &mut Model, index: usize) -> Command<Effect, Event> {
    if model.stage != Stage::AddKeys || index >= model.drafts.len() {
        return Command::done();
    }
    if model.drafts[index].proof.is_some() {
        return Command::done(); // already confirmed — nothing to redo
    }
    model.attempt += 1;
    begin_sign_member(model, index)
}

/// Freeze the founding set: every key already confirmed its membership at
/// creation, so this derives the address from ALL of them and writes the
/// pending-sync record — all before a single byte of account state exists.
fn finish_keys(model: &mut Model) -> Command<Effect, Event> {
    if model.stage != Stage::AddKeys || model.drafts.is_empty() {
        return Command::done();
    }
    // Every founding key must carry its creation-time proof: the publish
    // replays them without a single prompt, so an unconfirmed draft would
    // surface as a server rejection instead of a clear per-row state.
    if model.drafts.iter().any(|draft| draft.proof.is_none()) {
        return Command::done();
    }
    if needs_second_key(&model.drafts) {
        return Command::done();
    }
    model.attempt += 1;

    let keys: Vec<PreparedKey> = model
        .drafts
        .clone()
        .into_iter()
        .map(|draft| PreparedKey {
            credential_id: draft.credential_id,
            name: draft.name,
            public_key_hex: draft.public_key_hex,
            attestation_object_hex: draft.attestation_object_hex,
            attestation_hex: draft.attestation_hex,
            authenticator_attachment: draft.authenticator_attachment,
            transports: crate::passkey::allowlist_transports(&draft.transports),
            proof: draft.proof,
            signer_origin: draft.signer_origin,
        })
        .collect();

    model.status = Some(StatusKey::ComputingAddress);
    let hexes: Vec<String> = keys.iter().map(|key| key.public_key_hex.clone()).collect();
    // Also rejects duplicate keys — two providers returning the same
    // credential material would otherwise mint an undeployable address.
    let address = match super::address_from_public_key_hexes(&hexes) {
        Ok(address) => address,
        Err(error) => {
            return fail_to_add_keys(
                model,
                PromptKind::CreateFailed {
                    detail: error.to_string(),
                },
            )
        }
    };

    // The wallet's creation moment is the first key's mint time — no clock
    // lives in the core.
    let created_at_iso = model.drafts[0].registered_at_iso.clone();
    let first = keys[0].clone();
    let members = keys
        .iter()
        .map(|key| super::PendingUploadMember {
            credential_id: key.credential_id.clone(),
            name: key.name.clone(),
            public_key_hex: key.public_key_hex.clone(),
            attestation_object_hex: key.attestation_object_hex.clone(),
            authenticator_attachment: key.authenticator_attachment.clone(),
            transports: key.transports.clone(),
            signer_origin: key.signer_origin.clone(),
        })
        .collect();
    model.prepared = Some(Prepared {
        keys,
        address,
        created_at_iso: created_at_iso.clone(),
    });
    model.stage = Stage::SavingPending;
    request(
        model,
        ShellOperation::SavePendingUpload {
            record: PendingUpload {
                id: first.credential_id,
                name: first.name,
                public_key_hex: first.public_key_hex,
                attestation_object_hex: first.attestation_object_hex,
                created_at_iso,
                authenticator_attachment: first.authenticator_attachment,
                transports: first.transports,
                members,
            },
        },
    )
}

fn start_over(model: &mut Model) -> Command<Effect, Event> {
    // Abandon the drafted passkeys. Nothing about them was persisted (no
    // account, and the pending upload only exists once the set is frozen), so
    // this is a clean reset; the orphaned authenticator entries are inert.
    model.attempt += 1;
    if let Some(handle) = model.abort.take() {
        handle.abort();
    }
    model.drafts.clear();
    model.registering_label = String::new();
    model.group_seed_hex = None;
    model.group_public_key_hex = None;
    model.signing_index = None;
    model.prepared = None;
    model.sync = SyncState::default();
    model.stage = Stage::Form;
    model.status = None;
    render()
}

fn retry_upload(model: &mut Model) -> Command<Effect, Event> {
    if model.stage != Stage::SyncFailed || model.prepared.is_none() {
        return Command::done();
    }
    // Resumes at the publish — never at registration. The passkey is already
    // proven; re-registering would mint a second one for the same wallet. The
    // publish re-prompts for the member signature, which an explicit retry
    // makes expected.
    model.attempt += 1;
    begin_publish(model)
}

fn enter_wallet(model: &mut Model) -> Command<Effect, Event> {
    if model.stage != Stage::Created {
        return Command::done();
    }
    let Some(prepared) = model.prepared.clone() else {
        return Command::done();
    };
    // Signing was proven during creation and the key is synced — entering is
    // now a state transition, not another ceremony.
    model.attempt += 1;
    model.stage = Stage::Completing;
    request(
        model,
        ShellOperation::CompleteOnboarding {
            mode: CompletionMode::AddAccount {
                account: prepared.account(),
            },
        },
    )
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: ShellResult) -> Command<Effect, Event> {
    match (&model.stage, result) {
        // -- support probe ---------------------------------------------------
        (Stage::CheckingSupport, ShellResult::PasskeySupport { supported }) => {
            if supported {
                // The group key comes FIRST: every member's creation-time
                // proof binds to its public key, so it must exist before the
                // first registration.
                model.stage = Stage::GeneratingGroupKey;
                model.status = Some(StatusKey::SettingUpIdentity);
                request(model, ShellOperation::GenerateGroupKey)
            } else {
                fail_to_form(model, PromptKind::NotSupportedCreate, None)
            }
        }

        // -- group key ---------------------------------------------------------
        (
            Stage::GeneratingGroupKey,
            ShellResult::GroupKeyGenerated {
                seed_hex,
                group_public_key_hex,
            },
        ) => {
            model.group_seed_hex = Some(seed_hex);
            model.group_public_key_hex = Some(group_public_key_hex);
            // Land on the (empty) key list instead of minting a first key
            // here. The first key used to be the one choice the shell made
            // instead of the person — `KeyMethod::default()`, i.e. the
            // platform authenticator — which on an OEM whose system sheet
            // cannot reach a security key locked hardware-key owners out of
            // creating a wallet at all (device-found on a Xiaomi, 2026-08-26).
            // Every founding key, the first included, is now minted from the
            // key screen through the same method choice.
            model.stage = Stage::AddKeys;
            model.status = None;
            render()
        }
        (Stage::GeneratingGroupKey, ShellResult::StorageFailed { message }) => {
            fail_to_form(model, PromptKind::CreateFailed { detail: message }, None)
        }

        // -- registration ----------------------------------------------------
        (
            Stage::Registering,
            ShellResult::PasskeyRegistered {
                registration,
                now_iso,
            },
        ) => {
            // Extract the public key NOW: a duplicate authenticator (same
            // credential material behind a different id) is caught the moment
            // it appears, not at FinishKeys.
            model.status = Some(StatusKey::ExtractingKey);
            let public_key_hex =
                match public_key_hex_from_attestation(&registration.attestation_object_hex) {
                    Ok(hex) => hex,
                    Err(error) => {
                        return fail_registration(
                            model,
                            PromptKind::CreateFailed {
                                detail: error.to_string(),
                            },
                        )
                    }
                };
            if model
                .drafts
                .iter()
                .any(|draft| draft.public_key_hex.eq_ignore_ascii_case(&public_key_hex))
            {
                return fail_registration(
                    model,
                    PromptKind::CreateFailed {
                        detail: "this authenticator already holds one of this wallet's keys"
                            .to_owned(),
                    },
                );
            }
            // The attestation is best-effort: a passkey with no
            // attested-credential data still registers.
            let attestation_hex =
                crate::webauthn::extract_attestation(&registration.attestation_object_hex)
                    .unwrap_or_default();
            model.drafts.push(Draft {
                credential_id: registration.credential_id,
                attestation_object_hex: registration.attestation_object_hex,
                name: model.registering_label.clone(),
                registered_at_iso: now_iso,
                authenticator_attachment: registration.authenticator_attachment,
                transports: registration.transports,
                method: model.registering_method,
                public_key_hex,
                attestation_hex,
                proof: None,
                signer_origin: registration.signer_origin,
            });
            // Interleaved: the key confirms its group membership right here,
            // while this authenticator is still "in hand" — the member
            // challenge binds only (groupPublicKey, own attestation), so it
            // exists before the rest of the set does. One create + one get
            // per key, back to back; the publish then needs NO prompts.
            begin_sign_member(model, model.drafts.len() - 1)
        }
        (Stage::Registering, ShellResult::PasskeyFailed { kind, message }) => match kind {
            FailureKind::Cancelled => {
                // Back to the key list with the existing drafts intact — for
                // the first key that list is empty, and returning THERE rather
                // than to the form is what lets the person try again with a
                // different method.
                model.stage = Stage::AddKeys;
                model.status = Some(StatusKey::SetupCancelled);
                render()
            }
            // The authenticator made a device-local credential: it would sign
            // fine here but never appear at sign-in or sync for recovery. Stop
            // now — nothing has been persisted (issue #1).
            FailureKind::NotDiscoverable => fail_registration(model, PromptKind::NotDiscoverable),
            FailureKind::NotSupported => fail_registration(model, PromptKind::NotSupportedCreate),
            FailureKind::Other => fail_registration(
                model,
                PromptKind::CreateFailed {
                    detail: message.unwrap_or_default(),
                },
            ),
        },

        // -- creation-time membership confirmation ----------------------------
        (Stage::SigningKey, ShellResult::MemberProofSigned { proof }) => {
            if let Some(index) = model.signing_index.take() {
                if let Some(draft) = model.drafts.get_mut(index) {
                    draft.proof = Some(proof);
                }
            }
            model.stage = Stage::AddKeys;
            model.status = None;
            render()
        }
        (Stage::SigningKey, ShellResult::PasskeyFailed { kind, message }) => {
            model.signing_index = None;
            match kind {
                // The key stays drafted, just unconfirmed — its row offers a
                // per-key retry, and FinishKeys is gated on every proof.
                FailureKind::Cancelled => {
                    model.stage = Stage::AddKeys;
                    model.status = Some(StatusKey::VerifyCancelled);
                    render()
                }
                _ => fail_to_add_keys(
                    model,
                    PromptKind::CreateFailed {
                        detail: message.unwrap_or_default(),
                    },
                ),
            }
        }
        // The member-mode challenge could not be fetched (index/network).
        (Stage::SigningKey, ShellResult::IndexFailed { message, .. }) => {
            model.signing_index = None;
            fail_to_add_keys(model, PromptKind::CreateFailed { detail: message })
        }

        // -- pending record --------------------------------------------------
        (Stage::SavingPending, ShellResult::PendingUploadSaved) => begin_publish(model),
        (Stage::SavingPending, ShellResult::StorageFailed { message }) => {
            fail_to_form(model, PromptKind::CreateFailed { detail: message }, None)
        }

        // -- registry publish (option B: publish before entering) ------------
        // Published (or the identical group was already on-chain) → clear the
        // pending record, then save and enter.
        (Stage::Syncing(SyncStep::Publishing), ShellResult::RegistryPublished) => {
            let Some(prepared) = model.prepared.clone() else {
                return Command::done();
            };
            model.stage = Stage::Syncing(SyncStep::RemovingPending);
            request(
                model,
                ShellOperation::RemovePendingUpload {
                    credential_id: prepared.first().credential_id.clone(),
                },
            )
        }
        // Publish failed. There is no silent retry — the publish needs a
        // passkey signature — so offer the retry screen. The pending record is
        // kept, and the key stays recoverable on-device regardless.
        (Stage::Syncing(SyncStep::Publishing), ShellResult::IndexFailed { message, .. }) => {
            model.sync.last_error = Some(message);
            model.stage = Stage::SyncFailed;
            model.status = None;
            render()
        }
        (Stage::Syncing(SyncStep::RemovingPending), ShellResult::PendingUploadRemoved) => {
            save_account(model)
        }
        (Stage::Syncing(SyncStep::RemovingPending), ShellResult::StorageFailed { .. }) => {
            save_account(model)
        }

        // -- local persistence ----------------------------------------------
        (Stage::Saving, ShellResult::AccountSaved) => {
            model.stage = Stage::Created;
            model.status = None;
            render()
        }
        (Stage::Saving, ShellResult::StorageFailed { message }) => {
            fail_to_form(model, PromptKind::CreateFailed { detail: message }, None)
        }

        // -- handover ---------------------------------------------------------
        (Stage::Completing, ShellResult::OnboardingCompleted) => Command::done(),

        // A prompt was dismissed, or a result arrived for a stage that no longer
        // expects it. Neither is an error, and neither may change state.
        _ => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Internal transitions
// ---------------------------------------------------------------------------

/// Run the possession-proven publish for the prepared wallet. A metadata
/// encoding failure surfaces on the retry screen rather than blocking.
fn begin_publish(model: &mut Model) -> Command<Effect, Event> {
    let Some(prepared) = model.prepared.clone() else {
        return Command::done();
    };
    model.sync = SyncState::default();
    model.status = Some(StatusKey::SyncingKey);
    let group_seed_hex = model.group_seed_hex.clone().unwrap_or_default();
    let group_public_key_hex = model.group_public_key_hex.clone().unwrap_or_default();
    match registry_publish_op(&prepared, group_seed_hex, group_public_key_hex) {
        Ok(operation) => {
            model.stage = Stage::Syncing(SyncStep::Publishing);
            request(model, operation)
        }
        Err(error) => {
            model.sync.last_error = Some(error.to_string());
            model.stage = Stage::SyncFailed;
            model.status = None;
            render()
        }
    }
}

/// Build the registry publish operation for a prepared wallet: one member per
/// founding key, `key_names` in the same canonical founding order, every
/// member carrying its creation-time proof — the executor closes the group
/// with the software key and registers, no prompts.
fn registry_publish_op(
    prepared: &Prepared,
    group_seed_hex: String,
    group_public_key_hex: String,
) -> Result<ShellOperation, CoreError> {
    let metadata = RegistryMetadata {
        version: REGISTRY_METADATA_VERSION,
        address: prepared.address.clone(),
        wallet_version: WALLET_VERSION.to_owned(),
        key_names: prepared.keys.iter().map(|key| key.name.clone()).collect(),
        created_at_iso: prepared.created_at_iso.clone(),
    };
    Ok(ShellOperation::RegistryPublish {
        metadata_hex: metadata.encode_hex()?,
        members: prepared
            .keys
            .iter()
            .map(|key| RegistryPublishMember {
                credential_id: key.credential_id.clone(),
                public_key_hex: key.public_key_hex.clone(),
                attestation_hex: key.attestation_hex.clone(),
                authenticator_attachment: key.authenticator_attachment.clone(),
                transports: key.transports.clone(),
                proof: key.proof.clone(),
                signer_origin: key.signer_origin.clone(),
            })
            .collect(),
        group_seed_hex,
        group_public_key_hex,
        // Every member above replays its creation-time proof, so no live
        // signature happens and no route is consulted; the default satisfies
        // the field.
        method: KeyMethod::default(),
    })
}

/// The only place an account is ever written — reachable only after the
/// possession-proven publish has landed (or degraded to the retry screen).
fn save_account(model: &mut Model) -> Command<Effect, Event> {
    let Some(prepared) = model.prepared.clone() else {
        return Command::done();
    };
    model.stage = Stage::Saving;
    request(
        model,
        ShellOperation::SaveAccount {
            account: prepared.account(),
        },
    )
}

/// A registration failure returns to wherever the drafts live: the key list
/// when founding keys already exist (they stay intact), the form otherwise.
fn fail_registration(model: &mut Model, prompt: PromptKind) -> Command<Effect, Event> {
    // Always back to the key list — even for the first key, whose list is
    // empty. The list is where the method choice lives, and a failed
    // authenticator is exactly when a person wants to pick a different one.
    fail_to_add_keys(model, prompt)
}

/// Return to the key list, telling the user why. The drafts are kept — the
/// minted passkeys are real and the user decides what to do with the set.
fn fail_to_add_keys(model: &mut Model, prompt: PromptKind) -> Command<Effect, Event> {
    model.stage = Stage::AddKeys;
    model.status = None;
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(ShellOperation::Prompt {
            kind: prompt,
            confirmable: false,
        })
        .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

/// Return to the form, optionally telling the user why. The draft is left
/// exactly as the caller set it: cleared for terminal outcomes, kept for
/// resumable ones.
fn fail_to_form(
    model: &mut Model,
    prompt: PromptKind,
    status: Option<StatusKey>,
) -> Command<Effect, Event> {
    model.stage = Stage::Form;
    model.status = status;
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(ShellOperation::Prompt {
            kind: prompt,
            confirmable: false,
        })
        .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

/// Issue one operation, remembering how to abandon it. `attempt` is captured
/// here, so the result can be matched against the run that asked for it.
fn request(model: &mut Model, operation: ShellOperation) -> Command<Effect, Event> {
    let attempt = model.attempt;
    let command = Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result });
    model.abort = Some(command.abort_handle());
    Command::all([command, render()])
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    /// The key list as the shells receive it, from drafts alone. The event
    /// path is covered in `tests/app_create_wallet.rs`; this reaches the three
    /// row fields directly because the shapes that matter here (a YubiKey, a
    /// phone reached by a code, an attestation nobody can read) are properties
    /// of the DRAFT, not of a ceremony.
    fn rows(drafts: Vec<Draft>) -> Vec<CreateKeyRow> {
        let model = Model {
            name: "Ann".to_owned(),
            stage: Stage::AddKeys,
            drafts,
            ..Model::default()
        };
        CreateWallet.view(&model).keys
    }

    fn draft(attachment: &str, transports: &str, method: KeyMethod) -> Draft {
        Draft {
            name: "Ann".to_owned(),
            authenticator_attachment: attachment.to_owned(),
            transports: transports.to_owned(),
            method,
            // A real 20-byte summary: version 1, an all-zero AAGUID, flags
            // 0x45 (no BS bit — device-bound), two reserved bytes.
            attestation_hex: "0100000000000000000000000000000000450000".to_owned(),
            ..Draft::default()
        }
    }

    /// Issue #207: the row must say what the key IS, whatever was tapped.
    ///
    /// The tap only chooses a ceremony — and on the web it does not even do
    /// that, because `navigator.credentials` shows its own picker. Somebody who
    /// taps "Phone or tablet" and then touches the YubiKey in the port must not
    /// be told they have a phone.
    #[test]
    fn a_key_that_reports_itself_a_security_key_says_so_whatever_was_tapped() {
        let rows = rows(vec![draft("cross-platform", "usb,nfc", KeyMethod::Hybrid)]);
        assert_eq!(rows[0].kind, KeyMethod::SecurityKey, "the report decides");
        assert_eq!(
            rows[0].method,
            KeyMethod::Hybrid,
            "the choice survives untouched — it routes the ceremony"
        );
    }

    /// The reporter's first row: a phone reached by scanning a code. Drawn as a
    /// hardware fob before this, because `cross-platform` alone decided it.
    #[test]
    fn a_phone_reached_by_a_code_is_a_phone() {
        let rows = rows(vec![draft(
            "cross-platform",
            "hybrid,internal",
            KeyMethod::Platform,
        )]);
        assert_eq!(rows[0].kind, KeyMethod::Hybrid);
    }

    /// Nothing reported at all: the person's tap is all there is.
    #[test]
    fn a_silent_authenticator_leaves_the_choice_standing() {
        let rows = rows(vec![draft("", "", KeyMethod::SecurityKey)]);
        assert_eq!(rows[0].kind, KeyMethod::SecurityKey);
    }

    /// A badge nobody can vouch for is not drawn — but the GATE still fails
    /// open, which is the part that keeps an honest provider out of a dead end.
    #[test]
    fn an_unreadable_attestation_is_synced_for_the_gate_and_unknown_for_the_badge() {
        let mut unreadable = draft("platform", "internal", KeyMethod::Platform);
        unreadable.attestation_hex = String::new();
        let model = Model {
            stage: Stage::AddKeys,
            drafts: vec![unreadable],
            ..Model::default()
        };
        let view = CreateWallet.view(&model);
        assert!(
            view.keys[0].synced,
            "the gate keeps its benefit of the doubt"
        );
        assert!(
            !view.keys[0].synced_known,
            "nobody verified it, so no badge"
        );
        assert!(
            !view.needs_second_key,
            "failing open is the whole point of reading it as synced"
        );

        // A readable one answers both.
        let readable = rows(vec![draft("platform", "internal", KeyMethod::Platform)]);
        assert!(readable[0].synced_known);
        assert!(!readable[0].synced, "flags 0x45 carries no BS bit");
    }
}
