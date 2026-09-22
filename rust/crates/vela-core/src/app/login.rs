//! Machine B — signing in with an existing passkey, and recovering when the
//! index server has never heard of it.
//!
//! ```text
//! Idle ─sign in─► support ─► authenticate ─► Safe-compat check
//!                                                  │
//!            local account ◄────── load accounts ──┘
//!                  │                     │ no local match
//!                  │                     ▼
//!                  │             query index ──record──► save ─► enter
//!                  │                     │ 404
//!                  │                     ▼
//!                  │            offer on-device recovery ─accept─► 2nd signature
//!                  ▼                                                    │
//!               enter ◄───────────────── save ◄─── rebuild public key ◄──┘
//!                                          └─► re-publish to the index (background)
//! ```
//!
//! The recovery branch is what keeps the index server a **cache** rather than a
//! single point of failure: two signatures from the same credential pin down
//! exactly one public key, and therefore exactly one Safe address, entirely
//! on-device. And because the wallet being recovered may already hold funds,
//! reaching it must never block on a server — the index is healed in the
//! background, after the user is already inside.

use crux_core::{command::AbortHandle, render::render, App, Command};
use serde::{Deserialize, Serialize};

use super::shell::{CompletionMode, Effect, ProofPurpose, ShellOperation, ShellResult};
use super::{
    address_from_public_key_hex, valid_display_name, Account, AccountKey, Assertion, FailureKind,
    KeyMethod, PromptKind, RegistryPublishMember,
};
use crate::error::CoreError;
use crate::primitives;
use crate::registry_metadata::{RegistryMetadata, REGISTRY_METADATA_VERSION};
use crate::webauthn;

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// Probes before the index server is declared unreachable, and the gap between
/// them — today's `for (i = 0; i < 3; i++)` with a 2 s sleep.
const HEALTH_PROBES: u8 = 3;
const HEALTH_PROBE_GAP_MS: u32 = 2000;

/// Fallback when a credential carries no name this app can trust.
const FALLBACK_ACCOUNT_NAME: &str = "Wallet";

/// The Safe deployment this wallet uses, recorded in the registry metadata.
const WALLET_VERSION: &str = "safe-1.4.1";

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "LoginEvent"))]
pub enum Event {
    /// Screen mounted — starts the reachability probe.
    Start,
    /// "I already have a wallet". `method` is which authenticator to sign in
    /// with — `Platform` (the default) lets the system choose; `SecurityKey`
    /// forces the app-owned CTAP path, so a wallet on a hardware key is
    /// reachable even when a platform passkey is also present.
    SignIn {
        #[serde(default)]
        method: KeyMethod,
    },
    /// Internal: a sign-in effect resolved, tagged with the attempt that asked.
    #[serde(skip)]
    ShellCompleted { attempt: u64, result: ShellResult },
    /// Internal: a health-probe effect resolved. Deliberately a separate
    /// channel — probing runs independently of sign-in, so it must not be
    /// discarded when a sign-in attempt supersedes.
    #[serde(skip)]
    HealthCompleted { result: ShellResult },
    /// Internal: the background index heal finished. Ignored by construction —
    /// the user is already in the wallet and nothing about it may change state.
    #[serde(skip)]
    HealIgnored,
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Idle,
    CheckingSupport,
    Authenticating,
    LoadingAccounts,
    /// One signature yields two candidate keys; we ask the registry which one
    /// it already knows before asking the user to sign a second time.
    MatchingKey,
    /// The matched key belongs to a registry group — fetching its founding
    /// members to reconstruct the (possibly multi-key) wallet.
    FetchingUnit,
    /// The resolved account's name fell to the fallback — asking the v1
    /// index for the display name a v1-era wallet stored server-side.
    ResolvingName,
    /// Waiting for the user to accept or decline on-device recovery.
    AwaitingConsent,
    /// Waiting for the second signature that pins down the public key.
    Recovering,
    /// Running the possession-proven publish before entering (option B).
    Publishing,
    Saving,
    Completing,
}

/// What follows a name resolution: entering directly (the account is
/// already registered) or the recovery publish.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum AfterName {
    #[default]
    Save,
    Publish,
}

#[derive(Clone, Debug, Default)]
pub struct Health {
    probes_done: u8,
    unreachable: bool,
    /// The LAST failed probe never left the machine (`IndexTransportFailed`).
    /// Read only once `unreachable` is set, so a single local blip inside an
    /// otherwise-remote failure does not change the sentence.
    local: bool,
}

#[derive(Default)]
pub struct Model {
    stage: Stage,
    /// The assertion this attempt authenticated with. In the recovery branch it
    /// is also the *first* of the two signatures.
    assertion: Option<Assertion>,
    /// Wall clock observed during authentication, used for `created_at`.
    observed_at: String,
    /// The account about to be persisted.
    pending: Option<Account>,
    /// Every account this device already stores, kept from the credential
    /// match so the ADDRESS match below can still see it. A wallet is its
    /// address, and the credential check alone cannot say "this wallet is
    /// already here" — see [`begin_save`].
    known: Vec<Account>,
    /// Candidate public keys from the first signature still to be checked
    /// against the registry (`04‖x‖y` hex).
    candidates: Vec<String>,
    /// The candidate currently being queried.
    querying: Option<String>,
    /// Where the flow continues once the name resolution answers.
    after_name: AfterName,
    /// The authenticator the person chose on the sign-in screen. Carried so the
    /// "who are you?" ceremony runs on the route they asked for.
    method: KeyMethod,
    attempt: u64,
    health: Health,
    abort: Option<AbortHandle>,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct LoginView {
    /// A sign-in is in flight — the welcome button shows its spinner.
    pub busy: bool,
    /// The index server did not answer its health probe. The screen surfaces
    /// the endpoint settings so the user can point the app somewhere reachable.
    pub endpoint_unreachable: bool,
    /// The probe could not get out of THIS MACHINE (spec 038): every route the
    /// shell tried refused locally. Implies `endpoint_unreachable`. The screen
    /// says "check your network" and does NOT offer the endpoint field — a
    /// different URL cannot fix a machine that cannot reach any of them.
    pub transport_failed: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Login;

impl App for Login {
    type Event = Event;
    type Model = Model;
    type ViewModel = LoginView;
    type Effect = Effect;

    fn update(&self, event: Event, model: &mut Model) -> Command<Effect, Event> {
        match event {
            Event::Start => {
                model.health = Health::default();
                Command::all([probe_health(), render()])
            }
            Event::SignIn { method } => sign_in(model, method),
            Event::HealthCompleted { result } => accept_health(model, result),
            Event::HealIgnored => Command::done(),
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done(); // belongs to a superseded attempt
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> LoginView {
        LoginView {
            busy: model.stage != Stage::Idle,
            endpoint_unreachable: model.health.unreachable,
            transport_failed: model.health.unreachable && model.health.local,
        }
    }
}

// ---------------------------------------------------------------------------
// Reachability probe
// ---------------------------------------------------------------------------

fn accept_health(model: &mut Model, result: ShellResult) -> Command<Effect, Event> {
    match result {
        ShellResult::IndexHealth { ok: true } => {
            // Reachable — stop probing and say nothing.
            model.health.probes_done = HEALTH_PROBES;
            Command::done()
        }
        ShellResult::IndexHealth { ok: false } => probe_failed(model, false),
        ShellResult::IndexTransportFailed => probe_failed(model, true),
        ShellResult::Waited => probe_health(),
        _ => Command::done(),
    }
}

/// One failed probe. `local` is what the shell said about THIS probe; it is
/// what the verdict reports only if this probe is the one that made the index
/// unreachable.
fn probe_failed(model: &mut Model, local: bool) -> Command<Effect, Event> {
    model.health.probes_done += 1;
    model.health.local = local;
    if model.health.probes_done >= HEALTH_PROBES {
        model.health.unreachable = true;
        render()
    } else {
        Command::request_from_shell(ShellOperation::Wait {
            ms: HEALTH_PROBE_GAP_MS,
        })
        .then_send(|result| Event::HealthCompleted { result })
    }
}

fn probe_health() -> Command<Effect, Event> {
    Command::request_from_shell(ShellOperation::ProbeIndexHealth)
        .then_send(|result| Event::HealthCompleted { result })
}

// ---------------------------------------------------------------------------
// Sign-in
// ---------------------------------------------------------------------------

fn sign_in(model: &mut Model, method: KeyMethod) -> Command<Effect, Event> {
    if model.stage != Stage::Idle {
        return Command::done(); // one ceremony at a time
    }
    model.attempt += 1;
    model.assertion = None;
    model.pending = None;
    model.candidates = Vec::new();
    model.querying = None;
    model.method = method;
    model.stage = Stage::CheckingSupport;
    request(model, ShellOperation::CheckPasskeySupport)
}

fn accept(model: &mut Model, result: ShellResult) -> Command<Effect, Event> {
    match (&model.stage, result) {
        // -- support probe ---------------------------------------------------
        (Stage::CheckingSupport, ShellResult::PasskeySupport { supported }) => {
            if supported {
                model.stage = Stage::Authenticating;
                request(
                    model,
                    ShellOperation::AuthenticatePasskey {
                        method: model.method,
                    },
                )
            } else {
                idle_with_prompt(model, PromptKind::NotSupportedLogin)
            }
        }

        // -- authentication --------------------------------------------------
        (Stage::Authenticating, ShellResult::PasskeyAuthenticated { assertion, now_iso }) => {
            // Before any resolution or persistence: can this provider's
            // signatures ever satisfy the Safe contracts?
            if !assertion.is_safe_compatible() {
                return idle_with_prompt(model, PromptKind::IncompatibleLogin);
            }
            model.assertion = Some(assertion);
            model.observed_at = now_iso;
            model.stage = Stage::LoadingAccounts;
            request(model, ShellOperation::LoadAccounts)
        }

        // -- resolution: local ------------------------------------------------
        (Stage::LoadingAccounts, ShellResult::AccountsLoaded { accounts }) => {
            let Some(assertion) = model.assertion.clone() else {
                return Command::done();
            };
            model.known = accounts.clone();
            match accounts
                .iter()
                .position(|account| account.matches_credential(&assertion.credential_id))
            {
                Some(active_index) => {
                    // Already known here: enter immediately, no server involved.
                    model.stage = Stage::Completing;
                    request(
                        model,
                        ShellOperation::CompleteOnboarding {
                            mode: CompletionMode::SetWallet {
                                accounts,
                                active_index,
                            },
                        },
                    )
                }
                None => {
                    // The registry cannot be looked up by credential id, but
                    // one signature already yields two candidate keys — one
                    // real, one with no holder. Whichever the registry knows
                    // is the real one, so the common case (already published)
                    // needs no second signature at all.
                    begin_candidate_match(model)
                }
            }
        }
        (Stage::LoadingAccounts, ShellResult::StorageFailed { message }) => {
            idle_with_prompt(model, PromptKind::SignInFailed { detail: message })
        }

        // -- recovery ----------------------------------------------------------
        (Stage::AwaitingConsent, ShellResult::PromptAnswered { accepted }) => {
            let Some(assertion) = model.assertion.clone() else {
                return Command::done();
            };
            if !accepted {
                model.stage = Stage::Idle;
                return render();
            }
            model.stage = Stage::Recovering;
            request(
                model,
                ShellOperation::SignProof {
                    // The same credential that just signed, so what it reported
                    // about itself a moment ago still describes it.
                    transports: transports_from_attachment(&assertion.authenticator_attachment),
                    credential_id: assertion.credential_id,
                    // Run the second signature on the SAME route as the first: a
                    // phone over caBLE stays on caBLE (a USB proof would look for a
                    // security key that was never plugged in, and no QR would show).
                    method: model.method,
                    purpose: ProofPurpose::RecoverSecond,
                    signer_origin: assertion.signer_origin.clone(),
                },
            )
        }
        (
            Stage::Recovering,
            ShellResult::ProofSigned {
                assertion: second, ..
            },
        ) => {
            let Some(first) = model.assertion.clone() else {
                return Command::done();
            };
            match recover_account(&first, &second, &model.observed_at) {
                // Both candidates were already checked against the registry and
                // neither was known, so the recovered key is unpublished: the
                // publish follows — and it FREEZES the name into the group
                // metadata, which is exactly why a fallback name is worth one
                // legacy lookup first.
                Some(account) => resolve_name_then(model, account, AfterName::Publish),
                // The two signatures did not pin down exactly one key (or the
                // bytes would not parse). Nothing is persisted on a guess.
                None => idle_with_prompt(model, PromptKind::RecoverFailed),
            }
        }
        (Stage::Recovering, ShellResult::PasskeyFailed { kind, .. }) => match kind {
            FailureKind::Cancelled => {
                model.stage = Stage::Idle;
                render()
            }
            _ => idle_with_prompt(model, PromptKind::RecoverFailed),
        },

        // -- one-signature match: which candidate does the registry know? -----
        // A candidate the registry already holds IS the real key — the false
        // candidate has no holder and can never be registered. Enter directly:
        // one signature, and no publish (it is already there).
        (
            Stage::MatchingKey,
            ShellResult::RegistryKeyStatus {
                registered: true,
                unit_ids,
            },
        ) => {
            // A key belonging to a group must be reconstructed from the FULL
            // founding set — deriving from this key alone would produce the
            // wrong address for any multi-key wallet. The lowest id is the
            // founding group (multi-unit membership is an explicit non-goal).
            match unit_ids.iter().min().copied() {
                Some(unit_id) => {
                    model.stage = Stage::FetchingUnit;
                    request(model, ShellOperation::RegistryQueryUnit { unit_id })
                }
                // A registered key predating groups: the historical
                // single-key resolution.
                None => match matched_account(model) {
                    Some(account) => resolve_name_then(model, account, AfterName::Save),
                    None => offer_recovery(model),
                },
            }
        }
        // This candidate is unknown; try the next, or fall back to a second
        // signature once both are exhausted.
        (
            Stage::MatchingKey,
            ShellResult::RegistryKeyStatus {
                registered: false, ..
            },
        ) => query_next_candidate(model),
        // The registry could not answer, so it cannot disambiguate the two
        // candidates — fall back to the second signature.
        (Stage::MatchingKey, ShellResult::IndexFailed { .. }) => offer_recovery(model),

        // -- group reconstruction ----------------------------------------------
        (
            Stage::FetchingUnit,
            ShellResult::RegistryUnit {
                metadata_hex,
                members,
            },
        ) => {
            let Some(assertion) = model.assertion.clone() else {
                return Command::done();
            };
            match reconstruct_account(&assertion, &metadata_hex, &members, &model.observed_at) {
                // The group is already on-chain — no publish, just enter.
                Some(account) => resolve_name_then(model, account, AfterName::Save),
                // The fetched members do not recompute to the recorded
                // address. Nothing is persisted on a guess.
                None => idle_with_prompt(
                    model,
                    PromptKind::SignInFailed {
                        detail: "registry group does not match its address".to_owned(),
                    },
                ),
            }
        }
        // The unit is known to exist but could not be read. A multi-key
        // wallet's address CANNOT be derived from one key, so entering with a
        // single-key guess would fund the wrong Safe — surface the failure.
        (Stage::FetchingUnit, ShellResult::IndexFailed { message, .. }) => {
            idle_with_prompt(model, PromptKind::SignInFailed { detail: message })
        }

        // -- name resolution (v1-era wallets) ----------------------------------
        (Stage::ResolvingName, ShellResult::LegacyName { name }) => {
            let Some(mut account) = model.pending.take() else {
                return Command::done();
            };
            if let Some(name) = name.filter(|name| valid_display_name(name)) {
                if let Some(first) = account.keys.iter_mut().find(|key| key.name == account.name) {
                    first.name = name.clone();
                }
                account.name = name;
            }
            match model.after_name {
                AfterName::Save => begin_save(model, account),
                AfterName::Publish => {
                    model.pending = Some(account);
                    begin_publish(model)
                }
            }
        }

        // -- publish before entering (option B) --------------------------------
        // Publish landed (or the identical group was already there): enter.
        (Stage::Publishing, ShellResult::RegistryPublished) => match model.pending.clone() {
            Some(account) => begin_save(model, account),
            None => Command::done(),
        },
        // Publish failed: the recovered wallet is still valid; enter anyway.
        (Stage::Publishing, ShellResult::IndexFailed { .. }) => match model.pending.clone() {
            Some(account) => begin_save(model, account),
            None => Command::done(),
        },

        // -- persistence and handover -----------------------------------------
        // Publishing to the registry already happened before this point
        // (option B), so save simply hands the wallet over.
        (Stage::Saving, ShellResult::AccountSaved) => {
            let Some(account) = model.pending.clone() else {
                return Command::done();
            };
            model.stage = Stage::Completing;
            request(
                model,
                ShellOperation::CompleteOnboarding {
                    mode: CompletionMode::AddAccount { account },
                },
            )
        }
        (Stage::Saving, ShellResult::StorageFailed { message }) => {
            idle_with_prompt(model, PromptKind::SignInFailed { detail: message })
        }
        (Stage::Completing, ShellResult::OnboardingCompleted) => Command::done(),

        // -- ceremony failures anywhere else ------------------------------------
        (_, ShellResult::PasskeyFailed { kind, message }) => match kind {
            // The user closed the sheet. Not an error, not an alert.
            FailureKind::Cancelled => {
                model.stage = Stage::Idle;
                render()
            }
            FailureKind::NotSupported => idle_with_prompt(model, PromptKind::NotSupportedLogin),
            _ => idle_with_prompt(
                model,
                PromptKind::SignInFailed {
                    detail: message.unwrap_or_default(),
                },
            ),
        },

        _ => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the account for a credential and a known public key.
/// Where a credential lives, inferred from what an ASSERTION reported.
///
/// Registration reports transports outright; an assertion does not — it reports
/// only `authenticatorAttachment`. So this is a hint, and it is deliberately a
/// WIDE one for a cross-platform credential: "not on this device, could be a
/// key or could be a phone" is the truth, and naming both routes lets the
/// platform offer both. Naming neither is what makes Android pick the security
/// key and strand somebody holding a phone.
fn transports_from_attachment(attachment: &str) -> String {
    match attachment {
        "platform" => "internal".to_owned(),
        "cross-platform" => "usb,nfc,ble,hybrid".to_owned(),
        _ => String::new(),
    }
}

fn account_from_key(assertion: &Assertion, public_key_hex: &str, now_iso: &str) -> Option<Account> {
    let address = address_from_public_key_hex(public_key_hex).ok()?;
    let name = account_name(assertion);
    Some(Account {
        id: assertion.credential_id.clone(),
        name: name.clone(),
        address,
        public_key_hex: public_key_hex.to_owned(),
        created_at_iso: now_iso.to_owned(),
        keys: vec![AccountKey {
            credential_id: assertion.credential_id.clone(),
            public_key_hex: public_key_hex.to_owned(),
            name,
            transports: transports_from_attachment(&assertion.authenticator_attachment),
            signer_origin: assertion.signer_origin.clone(),
        }],
    })
}

/// Rebuild the wallet from two signatures by the same credential.
fn recover_account(first: &Assertion, second: &Assertion, now_iso: &str) -> Option<Account> {
    let (a, b) = (first.to_core().ok()?, second.to_core().ok()?);
    let key = webauthn::recover_public_key_from_assertions(&a, &b).ok()??;
    let public_key_hex = format!(
        "04{}{}",
        primitives::to_hex(&key.x, false),
        primitives::to_hex(&key.y, false)
    );
    account_from_key(first, &public_key_hex, now_iso)
}

/// The name to show, from the credential's own user handle when the index has
/// none. Never a raw foreign handle — see `Assertion::user_name`.
fn account_name(assertion: &Assertion) -> String {
    assertion
        .user_name()
        .unwrap_or_else(|| FALLBACK_ACCOUNT_NAME.to_owned())
}

/// Continue with `account`, first recovering its display name from the v1
/// index when — and only when — the passkey handle yielded none (the name
/// fell to the fallback). A v2-era wallet, or any handle that decodes,
/// never makes this extra request.
fn resolve_name_then(
    model: &mut Model,
    account: Account,
    after: AfterName,
) -> Command<Effect, Event> {
    if account.name != FALLBACK_ACCOUNT_NAME {
        return match after {
            AfterName::Save => begin_save(model, account),
            AfterName::Publish => {
                model.pending = Some(account);
                begin_publish(model)
            }
        };
    }
    let credential_id = account.id.clone();
    model.pending = Some(account);
    model.after_name = after;
    model.stage = Stage::ResolvingName;
    request(model, ShellOperation::LookupLegacyName { credential_id })
}

/// Persist the recovered wallet and hand it over — unless this device already
/// has it, in which case just enter as that account.
///
/// **A wallet IS its address**, and the credential match above cannot see
/// that. The address derives from the wallet's WHOLE key set, so signing in
/// with a second passkey of a multi-key wallet — one the stored record does
/// not list, which is every record written before that key joined — finds no
/// credential match, recovers the wallet from the registry, and arrives here
/// with the SAME address under a different `id`. `SaveAccount` upserts by
/// `id`, so that used to append: one wallet, twice in the switcher, twice in
/// every account list, and both entries opening the same Safe. (The pair also
/// crashed the web's switcher, which keyed its rows by address — issue 214
/// follow-up, founder-reported 2026-09-16.)
///
/// So the existing record wins and stays exactly as it is: it may carry a
/// name the user chose, and its address — the only thing that decides which
/// Safe this is — is by definition the same. Entering through `SetWallet`
/// rather than `AddAccount` is what makes this "sign in", not "add".
fn begin_save(model: &mut Model, account: Account) -> Command<Effect, Event> {
    if let Some(active_index) = model
        .known
        .iter()
        .position(|existing| existing.address.eq_ignore_ascii_case(&account.address))
    {
        model.pending = None;
        model.stage = Stage::Completing;
        let accounts = model.known.clone();
        return request(
            model,
            ShellOperation::CompleteOnboarding {
                mode: CompletionMode::SetWallet {
                    accounts,
                    active_index,
                },
            },
        );
    }
    model.pending = Some(account.clone());
    model.stage = Stage::Saving;
    request(model, ShellOperation::SaveAccount { account })
}

/// Recover the two candidate keys from the first signature and start checking
/// them against the registry. If neither can even be recovered, fall back to
/// the two-signature path.
fn begin_candidate_match(model: &mut Model) -> Command<Effect, Event> {
    let candidates = model
        .assertion
        .as_ref()
        .and_then(|assertion| assertion.to_core().ok())
        .and_then(|core| webauthn::recover_candidates(&core).ok())
        .unwrap_or_default();
    if candidates.is_empty() {
        return offer_recovery(model);
    }
    model.candidates = candidates;
    query_next_candidate(model)
}

/// Query the next untried candidate against the registry; when both are
/// exhausted, offer the on-device (two-signature) recovery.
fn query_next_candidate(model: &mut Model) -> Command<Effect, Event> {
    if model.candidates.is_empty() {
        return offer_recovery(model);
    }
    let candidate = model.candidates.remove(0);
    model.querying = Some(candidate.clone());
    model.stage = Stage::MatchingKey;
    request(
        model,
        ShellOperation::RegistryQueryByPublicKey {
            public_key_hex: candidate,
        },
    )
}

/// The account for the candidate the registry just confirmed it knows.
fn matched_account(model: &Model) -> Option<Account> {
    let assertion = model.assertion.as_ref()?;
    let public_key_hex = model.querying.as_ref()?;
    account_from_key(assertion, public_key_hex, &model.observed_at)
}

/// Rebuild the wallet from its registry group.
///
/// Recomputation is the authority, the metadata blob the cross-check: the
/// address that is persisted MUST be derivable from the fetched members, or
/// nothing is. The fetch order is the canonical founding order, so the
/// straight derivation matches first try; if it does not (a server that
/// reordered members), every member is tried as the pinned `keys[0]` — the
/// only degree of freedom, since `compute_safe_address_multi` canonically
/// orders the rest internally.
fn reconstruct_account(
    assertion: &Assertion,
    metadata_hex: &str,
    members: &[super::RegistryUnitMember],
    now_iso: &str,
) -> Option<Account> {
    if members.is_empty()
        || !members
            .iter()
            .any(|member| member.credential_id == assertion.credential_id)
    {
        // A group that does not even contain the signing credential is not
        // this wallet, whatever the query said.
        return None;
    }
    let metadata = RegistryMetadata::decode_hex(metadata_hex).ok()?;

    let ordered = reorder_to_address(members, &metadata.address)?;

    let name = assertion
        .user_name()
        .or_else(|| metadata.key_names.first().cloned())
        .unwrap_or_else(|| FALLBACK_ACCOUNT_NAME.to_owned());
    let keys: Vec<AccountKey> = ordered
        .iter()
        .enumerate()
        .map(|(index, member)| AccountKey {
            credential_id: member.credential_id.clone(),
            public_key_hex: member.public_key_hex.clone(),
            name: metadata
                .key_names
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("Key {}", index + 1)),
            // The registry stores no transports, and the one key that just
            // signed is the only one this device saw. Empty is the honest
            // answer for the others.
            transports: if member.credential_id == assertion.credential_id {
                transports_from_attachment(&assertion.authenticator_attachment)
            } else {
                String::new()
            },
            // Spec 075: the key that answered through the Clear Signer lives
            // behind that page. The others are unknown here, as their
            // transports are.
            signer_origin: if member.credential_id == assertion.credential_id {
                assertion.signer_origin.clone()
            } else {
                None
            },
        })
        .collect();
    let first = keys.first()?;
    Some(Account {
        id: first.credential_id.clone(),
        name,
        address: metadata.address.clone(),
        public_key_hex: first.public_key_hex.clone(),
        created_at_iso: if metadata.created_at_iso.is_empty() {
            now_iso.to_owned()
        } else {
            metadata.created_at_iso.clone()
        },
        keys,
    })
}

/// The founding order whose derivation equals `address`, or `None`. Tries the
/// given order first, then each member as the pin.
fn reorder_to_address<'a>(
    members: &'a [super::RegistryUnitMember],
    address: &str,
) -> Option<Vec<&'a super::RegistryUnitMember>> {
    let derives_to = |ordered: &[&super::RegistryUnitMember]| -> bool {
        let hexes: Vec<String> = ordered
            .iter()
            .map(|member| member.public_key_hex.clone())
            .collect();
        super::address_from_public_key_hexes(&hexes)
            .map(|derived| derived.eq_ignore_ascii_case(address))
            .unwrap_or(false)
    };

    let as_fetched: Vec<&super::RegistryUnitMember> = members.iter().collect();
    if derives_to(&as_fetched) {
        return Some(as_fetched);
    }
    for pin in 0..members.len() {
        let mut ordered: Vec<&super::RegistryUnitMember> = Vec::with_capacity(members.len());
        ordered.push(&members[pin]);
        ordered.extend(
            members
                .iter()
                .enumerate()
                .filter_map(|(index, member)| (index != pin).then_some(member)),
        );
        if derives_to(&ordered) {
            return Some(ordered);
        }
    }
    None
}

/// Offer the two-signature recovery — the second signature disambiguates the
/// candidates the registry could not.
fn offer_recovery(model: &mut Model) -> Command<Effect, Event> {
    model.stage = Stage::AwaitingConsent;
    request(
        model,
        ShellOperation::Prompt {
            kind: PromptKind::RecoverOffer,
            confirmable: true,
        },
    )
}

/// Emit the possession-proven publish for the pending account. A metadata
/// encoding failure never blocks reaching the wallet.
fn begin_publish(model: &mut Model) -> Command<Effect, Event> {
    let Some(account) = model.pending.clone() else {
        return Command::done();
    };
    // The authenticatorAttachment IS exposed on the login assertion (unlike the
    // attestation object and transports), so carry it onto the published entry.
    let attachment = model
        .assertion
        .as_ref()
        .map(|assertion| assertion.authenticator_attachment.clone())
        .unwrap_or_default();
    match registry_publish_op(&account, &attachment, model.method) {
        Ok(operation) => {
            model.stage = Stage::Publishing;
            request(model, operation)
        }
        Err(_) => begin_save(model, account),
    }
}

/// Build the single-key registry publish operation for an account.
fn registry_publish_op(
    account: &Account,
    authenticator_attachment: &str,
    method: KeyMethod,
) -> Result<ShellOperation, CoreError> {
    let metadata = RegistryMetadata {
        version: REGISTRY_METADATA_VERSION,
        address: account.address.clone(),
        wallet_version: WALLET_VERSION.to_owned(),
        key_names: vec![account.name.clone()],
        created_at_iso: account.created_at_iso.clone(),
    };
    Ok(ShellOperation::RegistryPublish {
        metadata_hex: metadata.encode_hex()?,
        members: vec![RegistryPublishMember {
            credential_id: account.id.clone(),
            public_key_hex: account.public_key_hex.clone(),
            // A recovered key carries no create()-time attestation object and no
            // transports (getTransports() is create-only) — but its assertion
            // DOES expose authenticatorAttachment, so that one is preserved.
            attestation_hex: String::new(),
            authenticator_attachment: authenticator_attachment.to_owned(),
            transports: String::new(),
            // Legacy mode: the executor mints a fresh group key and signs
            // this member live (one prompt) — recovery has no creation-time
            // proof to replay.
            proof: None,
            // …and that live signature must reach the page this key lives
            // behind, if it lives behind one (spec 075).
            signer_origin: account
                .keys
                .iter()
                .find(|key| key.credential_id == account.id)
                .or_else(|| account.keys.first())
                .and_then(|key| key.signer_origin.clone()),
        }],
        group_seed_hex: String::new(),
        group_public_key_hex: String::new(),
        // The live possession signature runs on the same route the sign-in and
        // the recovery signature did.
        method,
    })
}

fn idle_with_prompt(model: &mut Model, prompt: PromptKind) -> Command<Effect, Event> {
    model.stage = Stage::Idle;
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

fn request(model: &mut Model, operation: ShellOperation) -> Command<Effect, Event> {
    let attempt = model.attempt;
    let command = Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result });
    model.abort = Some(command.abort_handle());
    Command::all([command, render()])
}
