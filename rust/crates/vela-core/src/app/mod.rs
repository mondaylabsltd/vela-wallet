//! Portable onboarding state machines (spec `011-crux-onboarding-state`).
//!
//! # The boundary
//!
//! Everything in this module **decides**; nothing in it **does**. A machine
//! receives an [`Event`](create_wallet::Event), updates its `Model`, and returns
//! `Command`s that either re-render or declare a [`shell::Operation`] — "please
//! register a passkey", "please store this account". The platform shell performs
//! the operation and returns a [`shell::ShellResult`]. There is no network, no
//! storage, no clock and no randomness here, which is exactly what makes the
//! rules testable without a browser and portable to SwiftUI/Compose/GPUI later.
//!
//! # Why the rules live here at all
//!
//! Each one was bought by an incident and used to live inside a React component:
//! prove a passkey can sign before persisting anything (issue #1), resume a
//! cancelled verification instead of minting a second passkey, save locally only
//! after the index server confirms the key, treat a missing index record as
//! recoverable rather than fatal, and heal the index in the background so
//! reaching a funded wallet never blocks on a server (issue #89).
//!
//! # Layout
//!
//! - [`shell`] — the operation/result vocabulary both machines speak
//! - [`create_wallet`] — machine A: register → prove → derive → sync → save
//! - [`login`] — machine B: authenticate → resolve → recover → enter
//!
//! Compiled only with `--features crux`; the default build of this crate — the
//! one the iOS and Android bindings link — does not contain it.

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::primitives;
use crate::safe;
use crate::types::{ClientDataKind, WebAuthnAssertion};
use crate::webauthn;

pub mod activity_feed;
pub mod approval_guard;
pub mod balance_dashboard;
pub mod batch_import;
pub mod browser_history;
pub mod browser_load;
pub mod browser_tabs;
pub mod clear_signing;
pub mod contacts;
pub mod contacts_initials;
pub mod contacts_io;
pub mod create_wallet;
pub mod dapp_activity;
pub mod dapp_browser;
pub mod dapp_permissions;
pub mod dapp_record;
pub mod dapp_rpc;
pub mod dapp_session;
pub mod display_currency;
pub mod explore_sites;
pub mod ext_cache;
pub mod fee_policy;
pub mod fee_speed;
pub mod fee_tier_pref;
pub mod login;
pub mod manage_tokens;
pub mod method_words;
pub mod money;
pub mod name_verify;
pub mod net_health;
pub mod network_admin;
pub mod payment_request;
pub mod privacy;
pub mod receive_watch;
pub mod remote_mark;
pub mod rpc_pool;
pub mod self_call_guard;
pub mod send;
pub mod session;
pub mod shell;
pub mod sign_confirm;
pub mod sign_request;
pub mod signing_pages;
pub mod sim_outcome;
pub mod token_registry;
pub mod token_trust;
pub mod tx_tracker;

/// Implemented by every per-domain effect enum (spec 016) so product-agnostic
/// plumbing — the wasm bridge, the test driver — can split shell requests
/// from renders without knowing the domain. Three lines per machine; the
/// bridge and driver are written once.
pub trait SplitEffect {
    type Op: crux_core::capability::Operation;
    fn into_shell(self) -> Option<crux_core::Request<Self::Op>>;
}

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// WebAuthn caps `user.id` at 64 bytes; `encodeUserID` spends 37 of them on
/// `'\0' + uuid`, leaving 27 for the UTF-8 name. Mirrors
/// `MAX_USER_NAME_BYTES` in `src/modules/passkey/index.ts` — validated here so
/// a too-long (typically CJK) name fails on the form instead of deep inside the
/// ceremony with "User handle exceeds 64 bytes".
pub const MAX_USER_NAME_BYTES: usize = 64 - 37;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// A completed `navigator.credentials.create()`. Hex everywhere, matching the
/// existing `PasskeyRegistrationResult` contract.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Registration {
    pub credential_id: String,
    pub attestation_object_hex: String,
    pub client_data_json_hex: String,
    /// PublicKeyCredential response hints (not in authData, not signed):
    /// the `authenticatorAttachment` token ("platform" / "cross-platform",
    /// or empty) and the `getTransports()` list joined with commas
    /// (e.g. "hybrid,internal", or empty). Stored on the entry for display.
    #[serde(default)]
    pub authenticator_attachment: String,
    #[serde(default)]
    pub transports: String,
    /// The signing page's origin when the ceremony ran on one (a custom-domain
    /// wallet, R3) — the shell reports it only after the answer was checked to
    /// come from that origin. `None` for a ceremony in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_origin: Option<String>,
}

/// A completed `navigator.credentials.get()`. Mirrors `PasskeyAssertionResult`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Assertion {
    pub credential_id: String,
    pub signature_der_hex: String,
    pub authenticator_data_hex: String,
    pub client_data_json_hex: String,
    pub user_id_hex: Option<String>,
    /// The `authenticatorAttachment` token on the assertion's
    /// PublicKeyCredential ("platform" / "cross-platform", or empty). Unlike
    /// the attestation and transports — which live only in a create()
    /// response — this IS exposed on an assertion, so a recovered/logged-in
    /// key can still record it. Store-only display; never signed.
    #[serde(default)]
    pub authenticator_attachment: String,
    /// The signing page's origin when the ceremony ran on one (a sign-in on a
    /// custom-domain wallet's page, R3). `None` for a ceremony in the app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_origin: Option<String>,
}

/// One passkey of a wallet, in canonical founding order. `keys[0]` is the
/// pinned key that signs through the shared `WEBAUTHN_SIGNER`; every later key
/// signs through its own counterfactual signer proxy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct AccountKey {
    pub credential_id: String,
    /// Uncompressed P-256 point, `04‖x‖y` hex.
    pub public_key_hex: String,
    /// Per-key label; `keys[0].name` is the wallet name itself.
    pub name: String,
    /// WHERE this credential lives, as its authenticator reported at
    /// registration, comma joined (`hybrid,internal`, `usb,nfc`). Empty for
    /// records written before this field existed, and for authenticators that
    /// reported nothing — a `get()` then falls back to letting the platform
    /// guess, which is what this field exists to stop.
    #[serde(default)]
    pub transports: String,
    /// The origin of the signing page this key was minted or found on (spec
    /// 075). Spec 102: no longer how this build routes anything — the
    /// account's [`Account::signing_domain`] and [`Account::signing_venue`]
    /// are — but still WRITTEN, because it is what an older build reads to
    /// open the page (its "auto" route follows the first key's page), and a
    /// custom-domain account must stay signable there. Every key of a
    /// custom-domain account carries its page's origin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_origin: Option<String>,
}

/// The persisted wallet. Serialises 1:1 to `StoredAccount`.
///
/// The scalar `id`/`public_key_hex` fields are the legacy single-key shape and
/// stay authoritative for `keys[0]`: a multi-key account writes them as copies
/// of its first key, and a legacy record simply has no `keys` at all. Only
/// [`Account::key_hexes`] / [`Account::matches_credential`] may interpret this
/// duality — everything else asks them.
///
/// Spec 048: the retired client wrote these records in camelCase at the same
/// web origin the current shell now serves (`publicKeyHex`, `createdAt`,
/// `keys[].credentialId`). The hand-written reader below accepts that
/// spelling; only snake_case is ever written, and the web rewrites an old list
/// once it has read it.
///
/// Spec 102 adds where the account reviews and signs ([`Self::signing_venue`])
/// and the domain its keys live under ([`Self::signing_domain`]). A record
/// from before has neither, and the reader derives both from what it does
/// have — see [`Account`]'s `Deserialize` and `specs/102-signing-venue/
/// data-model.md` for the table. Records are written with both, plus what an
/// older build needs to keep signing (see the `Serialize` impl).
#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Account {
    pub id: String,
    pub name: String,
    pub address: String,
    pub public_key_hex: String,
    pub created_at_iso: String,
    /// Full founding key set. Empty ⇒ legacy single-key account (the scalar
    /// fields are the whole story). Records written before this field existed
    /// deserialize unchanged.
    pub keys: Vec<AccountKey>,
    /// The key this device signs with — see [`SignInKey`]. `None` on records
    /// written before it existed, which sign as they always did.
    ///
    /// Spec 102: written under this name; the `signed_in_with` of earlier
    /// builds is read (and migrated) but never written by this one — except as
    /// the compatibility copy older builds read, see `Serialize`.
    #[cfg_attr(feature = "bindings", ts(optional = nullable))]
    pub sign_in_key: Option<SignInKey>,
    /// Spec 102: the RP ID this account's keys live under — `getvela.app` for
    /// every account made in the apps or on the official page, the person's
    /// own domain for one made on a self-hosted page.
    pub signing_domain: String,
    /// Spec 102 (D1): where this account's transactions and messages are
    /// previewed and signed on THIS device. Always one that can reach the
    /// account's keys (R1) — the reader replaces one that cannot.
    pub signing_venue: crate::signing_venue::SigningVenue,
}

/// The JSON door to [`Account::signing_plan`] for the shells that hold the
/// record as JSON (web over wasm, the phones over UniFFI): the stored account
/// in, how it signs out — `None` for a record this build cannot read.
#[must_use]
pub fn signing_plan_json(account_json: &str) -> Option<String> {
    let account: Account = serde_json::from_str(account_json).ok()?;
    serde_json::to_string(&account.signing_plan()).ok()
}

/// [`signing_plan_json`] as the web wallet carries it out
/// ([`crate::signing_venue::SigningPlan::on_web`]): the web opens no page, so
/// a `getvela.app` account signs in Vela there and a custom-domain account is
/// `blocked` with the web's reason.
#[must_use]
pub fn signing_plan_on_web_json(account_json: &str) -> Option<String> {
    let account: Account = serde_json::from_str(account_json).ok()?;
    serde_json::to_string(&account.signing_plan().on_web()).ok()
}

/// Which of the wallet's keys this device signs with, and where that key
/// lives: the one the person created the wallet with, or last signed in with,
/// HERE.
///
/// Founder, 2026-09-26: a person says where their passkey is when they create
/// or sign in, and never again — every later signature reuses that answer. The
/// sign-in just proved this key answers over this route on this device, so a
/// "Sign with" choice per signature was asking a question already answered.
/// There is no switching: a key that stops answering is replaced by signing
/// out and signing in with another, which records that one instead.
///
/// Spec 102: `method` is where the key LIVES — one of the three places. Where
/// the person reviews and signs is the account's venue, not the key's.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignInKey {
    pub credential_id: String,
    /// The place that reached it — the person's choice at sign-in, not what
    /// the key reported at registration: a synced passkey minted on a phone
    /// (`internal`) is reached from a desktop by scanning a code.
    pub method: KeyMethod,
    /// Where the key actually answered from: the sign-in assertion's
    /// attachment (`internal`, or `usb,nfc,ble,hybrid` for "not this
    /// device"), or the transports the key reported when it was made. A
    /// system sheet may answer a choice from somewhere else — "This device"
    /// picked, a phone scanned — so the route names both, and a later
    /// signature can reach the key wherever the sign-in found it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub transports: String,
}

impl Account {
    /// R5: the key this device signs with and where it lives — the key the
    /// account was created or signed in with, over the place that reached it
    /// ([`SignInKey`]). `None` when the record predates that, or names a key
    /// this wallet does not hold: the shell then signs as it always did.
    #[must_use]
    pub fn key_route(&self) -> Option<crate::signing_venue::KeyRoute> {
        let key = self.sign_in_key.as_ref()?;
        if key.credential_id.is_empty() || !self.matches_credential(&key.credential_id) {
            return None;
        }
        let method = key.method.name();
        let routed = crate::wallet_keys::transports_of_method(method)?;
        Some(crate::signing_venue::KeyRoute::new(
            &key.credential_id,
            method,
            &crate::wallet_keys::joined_transports(routed, &key.transports),
        ))
    }

    /// How this account signs on this device: its signing domain, the venue
    /// its transactions and messages go to (R4), and its key route (R5).
    ///
    /// The venue the reader produced always reaches the keys when anything
    /// can; [`crate::signing_venue::SigningPlan::blocked`] is set only when
    /// nothing on this device can — and then the shell signs nothing.
    #[must_use]
    pub fn signing_plan(&self) -> crate::signing_venue::SigningPlan {
        crate::signing_venue::SigningPlan {
            domain: self.signing_domain.clone(),
            blocked: crate::signing_venue::reachability(&self.signing_domain, &self.signing_venue)
                .err(),
            venue: self.signing_venue.clone(),
            key: self.key_route(),
            key_label: self.key_label(),
        }
    }

    /// "Confirm with | {key}" — the key this device signs with, as the person
    /// reads it ([`crate::signing_venue::KeyLabel`]): its own label when that
    /// is not the wallet's name, else the place it lives (the place the
    /// sign-in reached it on). A record with no sign-in key names its first
    /// key the same way, its place read from what that key reported.
    #[must_use]
    pub fn key_label(&self) -> crate::signing_venue::KeyLabel {
        let (key, method) = match self
            .sign_in_key
            .as_ref()
            .filter(|key| self.matches_credential(&key.credential_id))
        {
            Some(sign_in) => (
                self.keys
                    .iter()
                    .find(|key| key.credential_id == sign_in.credential_id),
                sign_in.method,
            ),
            None => {
                let first = self.keys.first();
                let method = first
                    .and_then(|key| crate::passkey::reported_method("", &key.transports))
                    .unwrap_or_default();
                (first, method)
            }
        };
        crate::signing_venue::KeyLabel::of(
            key.map(|key| key.name.as_str()),
            &self.name,
            method.name(),
        )
    }

    /// Choose where this account reviews and signs (D1) — refused, and nothing
    /// changed, when the venue cannot reach the account's keys (R1).
    ///
    /// # Errors
    ///
    /// Why that venue cannot be used for this account.
    pub fn choose_venue(
        &mut self,
        venue: crate::signing_venue::SigningVenue,
    ) -> Result<(), crate::signing_venue::VenueBlock> {
        let venue = match venue {
            crate::signing_venue::SigningVenue::Page { url } => {
                crate::signing_venue::SigningVenue::page(&url)
                    .unwrap_or(crate::signing_venue::SigningVenue::Page { url })
            }
            in_vela => in_vela,
        };
        crate::signing_venue::reachability(&self.signing_domain, &venue)?;
        self.signing_venue = venue;
        Ok(())
    }

    /// The full key set in founding order; a legacy account projects its
    /// scalar field as the sole key.
    pub(crate) fn key_hexes(&self) -> Vec<String> {
        if self.keys.is_empty() {
            vec![self.public_key_hex.clone()]
        } else {
            self.keys.iter().map(|k| k.public_key_hex.clone()).collect()
        }
    }

    /// Does this credential belong to the wallet — as the legacy sole key or
    /// as any founding member?
    pub(crate) fn matches_credential(&self, credential_id: &str) -> bool {
        self.id == credential_id || self.keys.iter().any(|k| k.credential_id == credential_id)
    }

    /// The origin every key of a custom-domain account carries ([`AccountKey::
    /// signer_origin`]) — its page's — so an older build, which opens the page
    /// the first key names, keeps signing for it. Keys of a `getvela.app`
    /// account are left as they are.
    pub(crate) fn stamp_page_origin(&mut self) {
        if !crate::signing_venue::locked_to_page(&self.signing_domain) {
            return;
        }
        let Some(url) = self.signing_venue.page_url() else {
            return;
        };
        let origin = crate::trusted_signer::ws::origin_of(url);
        if origin.is_empty() {
            return;
        }
        for key in &mut self.keys {
            if key.signer_origin.as_deref().is_none_or(str::is_empty) {
                key.signer_origin = Some(origin.clone());
            }
        }
    }
}

/// How an account signs, settled for a new record by the machine that makes
/// it: the domain its keys live under and the venue it starts with. A page
/// the person chose (`page`) is the venue — locked to it when it is on a
/// custom domain (R2) — and otherwise Vela's own sheet.
pub(crate) fn new_signing(page: Option<&str>) -> (String, crate::signing_venue::SigningVenue) {
    use crate::signing_venue::{domain_of_page, SigningVenue, APP_DOMAIN};
    match page.and_then(SigningVenue::page) {
        Some(venue) => {
            let domain = venue
                .page_url()
                .map(domain_of_page)
                .filter(|domain| !domain.is_empty())
                .unwrap_or_else(|| APP_DOMAIN.to_owned());
            (domain, venue)
        }
        None => (APP_DOMAIN.to_owned(), SigningVenue::InVela),
    }
}

/// One draft key inside a multi-member [`PendingUpload`], founding order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PendingUploadMember {
    pub credential_id: String,
    pub name: String,
    pub public_key_hex: String,
    pub attestation_object_hex: String,
    #[serde(default)]
    pub authenticator_attachment: String,
    #[serde(default)]
    pub transports: String,
    /// See [`AccountKey::signer_origin`] — kept for the same older builds,
    /// which retry an interrupted publish from this record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signer_origin: Option<String>,
}

/// A key set that still owes the index server a successful publish. Written
/// *before* the first upload attempt so an interrupted creation is retried on a
/// later launch.
///
/// The scalar fields mirror `members[0]` (legacy single-key records have no
/// `members` and the scalars are the whole story). A record with
/// `members.len() > 1` must never be retried silently — replaying the publish
/// takes one passkey prompt per member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct PendingUpload {
    pub id: String,
    pub name: String,
    pub public_key_hex: String,
    pub attestation_object_hex: String,
    pub created_at_iso: String,
    /// Browser-reported display hints captured at creation, preserved so an
    /// interrupted publish retried on a later launch keeps them (they are not
    /// recoverable from the attestation object).
    #[serde(default)]
    pub authenticator_attachment: String,
    #[serde(default)]
    pub transports: String,
    /// Full founding key set. Empty ⇒ legacy single-key record.
    #[serde(default)]
    pub members: Vec<PendingUploadMember>,
    /// Issue 409: the registry task a ONE-key wallet's publish was accepted
    /// under. Present ⇒ the registry holds the registration, signed and
    /// queued, and this record is waiting only for the landing to be
    /// confirmed — which the session's landing watch does by reading that
    /// task: a read, no passkey. Absent on every multi-key record (those are
    /// entered only after the landing, and the record goes with it) and on any
    /// record whose publish was never accepted.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_id: Option<String>,
}

impl PendingUpload {
    /// Is this record waiting ONLY for its registry landing to be confirmed —
    /// a one-key wallet whose publish the registry already accepted? Those are
    /// the records a read can settle (issue 409). Everything else still owes
    /// the registry a publish, and a publish takes a passkey.
    #[must_use]
    pub fn awaits_landing(&self) -> Option<&str> {
        if self.members.len() > 1 {
            return None;
        }
        self.task_id.as_deref().filter(|task| !task.is_empty())
    }
}

/// One record of the pending-upload outbox as a reader finds it on disk
/// (issue 409). `None` when this build cannot read it.
///
/// The outbox is the shells' JSON, written by every build that ever ran on
/// the device — and by tests that seed it with whatever shape they need. One
/// record that will not parse must cost only itself: the landing watch that
/// reads the outbox would otherwise refuse the whole answer, and with it every
/// record it could have settled.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct OutboxRecord(pub Option<PendingUpload>);

impl<'de> Deserialize<'de> for OutboxRecord {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = serde_json::Value::deserialize(d)?;
        Ok(Self(serde_json::from_value(raw).ok()))
    }
}

// ---------------------------------------------------------------------------
// Spec 048: the retired client's spelling
//
// The retired client wrote these records in camelCase at the same web origin
// the current shell now serves (`publicKeyHex`, `createdAt`,
// `keys[].credentialId`). Each reader below accepts both spellings and the
// writers above emit snake_case only. Written by hand rather than with
// `#[serde(alias)]`, which ts-rs cannot parse and reports as a warning that
// CI's clippy gate turns into an error.
// ---------------------------------------------------------------------------

fn either<E: serde::de::Error>(
    snake: Option<String>,
    camel: Option<String>,
    field: &'static str,
) -> Result<String, E> {
    snake.or(camel).ok_or_else(|| E::missing_field(field))
}

impl<'de> Deserialize<'de> for AccountKey {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            credential_id: Option<String>,
            #[serde(rename = "credentialId")]
            credential_id_camel: Option<String>,
            public_key_hex: Option<String>,
            #[serde(rename = "publicKeyHex")]
            public_key_hex_camel: Option<String>,
            #[serde(default)]
            name: String,
            #[serde(default)]
            transports: String,
            #[serde(default)]
            signer_origin: Option<String>,
        }
        let w = Wire::deserialize(d)?;
        Ok(AccountKey {
            credential_id: either(w.credential_id, w.credential_id_camel, "credential_id")?,
            public_key_hex: either(w.public_key_hex, w.public_key_hex_camel, "public_key_hex")?,
            name: w.name,
            transports: w.transports,
            signer_origin: w.signer_origin.filter(|o| !o.is_empty()),
        })
    }
}

impl<'de> Deserialize<'de> for Account {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            id: String,
            #[serde(default)]
            name: String,
            address: String,
            public_key_hex: Option<String>,
            #[serde(rename = "publicKeyHex")]
            public_key_hex_camel: Option<String>,
            created_at_iso: Option<String>,
            #[serde(rename = "createdAt")]
            created_at_camel: Option<String>,
            #[serde(default)]
            keys: Vec<AccountKey>,
            /// Spec 102's sign-in key. Read loosely, like every field below: a
            /// value a newer build added must cost only itself (the account
            /// signs as it always did), never the whole account list.
            #[serde(default)]
            sign_in_key: Option<serde_json::Value>,
            /// The sign-in key as builds before spec 102 wrote it — a route,
            /// whose `method` could be `trusted_signer`.
            #[serde(default)]
            signed_in_with: Option<serde_json::Value>,
            #[serde(default)]
            signing_domain: Option<String>,
            #[serde(default)]
            signing_venue: Option<serde_json::Value>,
        }
        let w = Wire::deserialize(d)?;
        let legacy = w
            .signed_in_with
            .and_then(|raw| serde_json::from_value::<LegacySignIn>(raw).ok());
        let current = w
            .sign_in_key
            .and_then(|raw| serde_json::from_value::<SignInKey>(raw).ok());
        let mut account = Account {
            id: w.id,
            name: w.name,
            address: w.address,
            public_key_hex: either(w.public_key_hex, w.public_key_hex_camel, "public_key_hex")?,
            created_at_iso: either(w.created_at_iso, w.created_at_camel, "created_at_iso")?,
            keys: w.keys,
            sign_in_key: None,
            signing_domain: String::new(),
            signing_venue: crate::signing_venue::SigningVenue::InVela,
        };
        let stored = w
            .signing_venue
            .and_then(|raw| serde_json::from_value::<crate::signing_venue::SigningVenue>(raw).ok());
        let migrated = migrate(&account, legacy.as_ref());
        account.sign_in_key = match (current, migrated.sign_in_key) {
            // Both: this build wrote the record and its compatibility copy
            // together. An older build that signed in again since rewrote the
            // copy alone — then the copy is the newer fact.
            (Some(current), Some(copy)) if copy.credential_id != current.credential_id => {
                Some(copy)
            }
            (Some(current), _) => Some(current),
            (None, copy) => copy,
        };
        account.signing_domain = w
            .signing_domain
            .map(|domain| domain.trim().to_ascii_lowercase())
            .filter(|domain| !domain.is_empty())
            .unwrap_or(migrated.domain);
        account.signing_venue = settle_venue(&account, stored, migrated.venue);
        account.stamp_page_origin();
        Ok(account)
    }
}

// ---------------------------------------------------------------------------
// Spec 102: what builds before it wrote, and what they still read
//
// The data-model table (`specs/102-signing-venue/data-model.md`, "Migration")
// is the specification of `migrate`; each row has a test below.
// ---------------------------------------------------------------------------

/// The sign-in key as builds before spec 102 wrote it (`signed_in_with`): a
/// ROUTE, whose method could be the Trusted Signer — "sign on the page this key
/// lives behind". Only ever read.
#[derive(Deserialize)]
struct LegacySignIn {
    credential_id: String,
    method: LegacyMethod,
    #[serde(default)]
    transports: String,
    #[serde(default)]
    signer_origin: Option<String>,
}

/// The four "Sign with" routes of builds before spec 102. The fourth is read
/// here and nowhere else: it names a venue, not a place a key lives, and no
/// record this build writes carries it.
#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum LegacyMethod {
    Platform,
    Hybrid,
    SecurityKey,
    TrustedSigner,
}

/// What a record from before spec 102 says about how it signs.
struct Migrated {
    sign_in_key: Option<SignInKey>,
    domain: String,
    venue: crate::signing_venue::SigningVenue,
}

/// A page origin from an old record, empty read as absent.
fn origin_of_record(origin: Option<&String>) -> Option<&str> {
    origin
        .map(|origin| origin.trim())
        .filter(|origin| !origin.is_empty())
}

/// The migration rule (spec 102, "Migration"):
///
/// - a Trusted Signer sign-in whose page is empty or official → domain
///   `getvela.app`, venue the official page, the key place read from the key's
///   stored transports;
/// - one on a custom origin → domain that host, venue locked to that page;
/// - any other sign-in → the key place it names, venue Vela's sheet (or, on a
///   custom domain, its page);
/// - no sign-in key at all → the route builds before it always took: the first
///   key's page if it lives behind one, else Vela's sheet.
fn migrate(account: &Account, legacy: Option<&LegacySignIn>) -> Migrated {
    use crate::signing_venue::{SigningVenue, APP_DOMAIN};
    let held = |credential_id: &str| {
        account
            .keys
            .iter()
            .find(|key| key.credential_id == credential_id)
    };
    // The page this record's signatures went to, if they went to one.
    let page_origin: Option<String> = match legacy {
        Some(sign_in) if sign_in.method == LegacyMethod::TrustedSigner => Some(
            origin_of_record(sign_in.signer_origin.as_ref())
                .or_else(|| {
                    held(&sign_in.credential_id)
                        .and_then(|key| origin_of_record(key.signer_origin.as_ref()))
                })
                // Empty: "the person's page from Settings" — the official one
                // (spec 102, Migration).
                .unwrap_or(crate::trusted_signer::DEFAULT_SIGNER_URL)
                .to_owned(),
        ),
        Some(_) => None,
        None => account
            .keys
            .iter()
            .find(|key| !key.credential_id.is_empty())
            .and_then(|key| origin_of_record(key.signer_origin.as_ref()))
            .map(str::to_owned),
    };
    // The domain: the page's, else any key's page (a custom-domain wallet's
    // keys all share one), else the apps'.
    let domain = page_origin
        .as_deref()
        .and_then(|origin| crate::trusted_signer::registry_rp_id(Some(origin)))
        .or_else(|| {
            account
                .keys
                .iter()
                .filter_map(|key| origin_of_record(key.signer_origin.as_ref()))
                .find_map(|origin| crate::trusted_signer::registry_rp_id(Some(origin)))
        })
        .unwrap_or_else(|| APP_DOMAIN.to_owned());
    let venue = page_origin
        .as_deref()
        .and_then(SigningVenue::page)
        .filter(|venue| crate::signing_venue::reachability(&domain, venue).is_ok())
        .or_else(|| {
            crate::signing_venue::default_venue(
                &domain,
                account
                    .keys
                    .iter()
                    .filter_map(|key| origin_of_record(key.signer_origin.as_ref())),
            )
        })
        .unwrap_or(SigningVenue::InVela);
    let sign_in_key = legacy.map(|sign_in| {
        let stored = held(&sign_in.credential_id)
            .map(|key| key.transports.as_str())
            .unwrap_or_default();
        let method = match sign_in.method {
            LegacyMethod::Platform => KeyMethod::Platform,
            LegacyMethod::Hybrid => KeyMethod::Hybrid,
            LegacyMethod::SecurityKey => KeyMethod::SecurityKey,
            // The page was where the person checked what they signed; the key
            // itself lived where its authenticator said it did.
            LegacyMethod::TrustedSigner => {
                crate::passkey::reported_method("", stored).unwrap_or_default()
            }
        };
        SignInKey {
            credential_id: sign_in.credential_id.clone(),
            method,
            // A page sign-in recorded no transports; the key route then comes
            // from the key's own (spec 102, Migration).
            transports: if sign_in.transports.is_empty()
                && sign_in.method == LegacyMethod::TrustedSigner
            {
                crate::passkey::allowlist_transports(stored)
            } else {
                sign_in.transports.clone()
            },
        }
    });
    Migrated {
        sign_in_key,
        domain,
        venue,
    }
}

/// The venue a record is read with: the stored one when it can reach the
/// account's keys (R1); otherwise the migrated one, then the domain's default.
/// A record never comes out of the reader pointing at a venue that cannot
/// sign for it, unless nothing can — a custom domain with no page known —
/// and then [`Account::signing_plan`] says so.
fn settle_venue(
    account: &Account,
    stored: Option<crate::signing_venue::SigningVenue>,
    migrated: crate::signing_venue::SigningVenue,
) -> crate::signing_venue::SigningVenue {
    use crate::signing_venue::reachability;
    let domain = &account.signing_domain;
    stored
        .into_iter()
        .chain(std::iter::once(migrated.clone()))
        .find(|venue| reachability(domain, venue).is_ok())
        .or_else(|| {
            crate::signing_venue::default_venue(
                domain,
                account
                    .keys
                    .iter()
                    .filter_map(|key| origin_of_record(key.signer_origin.as_ref())),
            )
        })
        .unwrap_or(migrated)
}

impl Serialize for Account {
    /// Every field, and one more for builds before spec 102: `signed_in_with`,
    /// the sign-in key in the shape they read — written only for a
    /// `getvela.app` account, whose keys an older build can reach natively.
    ///
    /// A custom-domain account gets none on purpose. An older build reading
    /// one would sign natively with a key the app cannot reach; reading none,
    /// it follows the first key's page — which every key of such an account
    /// names ([`AccountKey::signer_origin`]) — and keeps working. The fourth
    /// route (`trusted_signer`) is never written.
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct OlderBuildsCopy<'a> {
            credential_id: &'a str,
            method: KeyMethod,
            #[serde(skip_serializing_if = "str::is_empty")]
            transports: &'a str,
        }
        #[derive(Serialize)]
        struct Wire<'a> {
            id: &'a str,
            name: &'a str,
            address: &'a str,
            public_key_hex: &'a str,
            created_at_iso: &'a str,
            keys: &'a [AccountKey],
            #[serde(skip_serializing_if = "Option::is_none")]
            sign_in_key: Option<&'a SignInKey>,
            signing_domain: &'a str,
            signing_venue: &'a crate::signing_venue::SigningVenue,
            #[serde(skip_serializing_if = "Option::is_none")]
            signed_in_with: Option<OlderBuildsCopy<'a>>,
        }
        let copy = self
            .sign_in_key
            .as_ref()
            .filter(|_| !crate::signing_venue::locked_to_page(&self.signing_domain))
            .map(|key| OlderBuildsCopy {
                credential_id: &key.credential_id,
                method: key.method,
                transports: &key.transports,
            });
        Wire {
            id: &self.id,
            name: &self.name,
            address: &self.address,
            public_key_hex: &self.public_key_hex,
            created_at_iso: &self.created_at_iso,
            keys: &self.keys,
            sign_in_key: self.sign_in_key.as_ref(),
            signing_domain: &self.signing_domain,
            signing_venue: &self.signing_venue,
            signed_in_with: copy,
        }
        .serialize(s)
    }
}

impl<'de> Deserialize<'de> for PendingUploadMember {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            credential_id: Option<String>,
            #[serde(rename = "credentialId")]
            credential_id_camel: Option<String>,
            #[serde(default)]
            name: String,
            public_key_hex: Option<String>,
            #[serde(rename = "publicKeyHex")]
            public_key_hex_camel: Option<String>,
            attestation_object_hex: Option<String>,
            #[serde(rename = "attestationObjectHex")]
            attestation_object_hex_camel: Option<String>,
            #[serde(default)]
            authenticator_attachment: String,
            #[serde(default, rename = "authenticatorAttachment")]
            authenticator_attachment_camel: String,
            #[serde(default)]
            transports: String,
            #[serde(default)]
            signer_origin: Option<String>,
        }
        let w = Wire::deserialize(d)?;
        Ok(PendingUploadMember {
            credential_id: either(w.credential_id, w.credential_id_camel, "credential_id")?,
            name: w.name,
            public_key_hex: either(w.public_key_hex, w.public_key_hex_camel, "public_key_hex")?,
            attestation_object_hex: either(
                w.attestation_object_hex,
                w.attestation_object_hex_camel,
                "attestation_object_hex",
            )?,
            authenticator_attachment: if w.authenticator_attachment.is_empty() {
                w.authenticator_attachment_camel
            } else {
                w.authenticator_attachment
            },
            transports: w.transports,
            signer_origin: w.signer_origin.filter(|o| !o.is_empty()),
        })
    }
}

impl<'de> Deserialize<'de> for PendingUpload {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Wire {
            id: String,
            #[serde(default)]
            name: String,
            public_key_hex: Option<String>,
            #[serde(rename = "publicKeyHex")]
            public_key_hex_camel: Option<String>,
            attestation_object_hex: Option<String>,
            #[serde(rename = "attestationObjectHex")]
            attestation_object_hex_camel: Option<String>,
            created_at_iso: Option<String>,
            #[serde(rename = "createdAt")]
            created_at_camel: Option<String>,
            #[serde(default)]
            authenticator_attachment: String,
            #[serde(default, rename = "authenticatorAttachment")]
            authenticator_attachment_camel: String,
            #[serde(default)]
            transports: String,
            #[serde(default)]
            members: Vec<PendingUploadMember>,
            #[serde(default)]
            task_id: Option<String>,
        }
        let w = Wire::deserialize(d)?;
        Ok(PendingUpload {
            id: w.id,
            name: w.name,
            public_key_hex: either(w.public_key_hex, w.public_key_hex_camel, "public_key_hex")?,
            attestation_object_hex: either(
                w.attestation_object_hex,
                w.attestation_object_hex_camel,
                "attestation_object_hex",
            )?,
            created_at_iso: either(w.created_at_iso, w.created_at_camel, "created_at_iso")?,
            authenticator_attachment: if w.authenticator_attachment.is_empty() {
                w.authenticator_attachment_camel
            } else {
                w.authenticator_attachment
            },
            transports: w.transports,
            members: w.members,
            task_id: w.task_id.filter(|task| !task.is_empty()),
        })
    }
}

/// One member passkey to include in a possession-proven registry publish, in
/// canonical founding order. The executor signs each with its credential.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RegistryPublishMember {
    pub credential_id: String,
    /// Uncompressed P-256 point, `04‖x‖y` hex.
    pub public_key_hex: String,
    /// Empty, or 20 versioned attestation bytes (hex).
    pub attestation_hex: String,
    /// Browser-reported display hints (not signed): the
    /// `authenticatorAttachment` token and the comma-joined transports list.
    #[serde(default)]
    pub authenticator_attachment: String,
    #[serde(default)]
    pub transports: String,
    /// The possession proof collected AT CREATION (interleaved flow). Absent
    /// on the login re-publish, whose executor signs the member live.
    #[serde(default)]
    pub proof: Option<crate::registry_proof::RegistryProof>,
}

/// One founding member of a registry group (Unit), as fetched back from the
/// index. The mirror of [`RegistryPublishMember`] on the read side; ascending
/// fetch order IS the canonical founding order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RegistryUnitMember {
    pub credential_id: String,
    /// Uncompressed P-256 point, `04‖x‖y` hex.
    pub public_key_hex: String,
    /// Browser-reported display hints recorded at registration.
    #[serde(default)]
    pub authenticator_attachment: String,
    #[serde(default)]
    pub transports: String,
}

/// Where a key lives — how the person chose to mint a founding key, or which
/// key they signed in with.
///
/// This is the **choice**, not the report. `CreateKeyRow` also carries
/// `authenticator_attachment` / `transports` / `aaguid`, which are what the
/// authenticator said about *itself* — and the two can legitimately disagree
/// (a "this device" choice that resolves to a cross-platform authenticator).
/// The ceremony follows the choice; the row's provider line shows the report.
/// Neither is inferred from the other.
///
/// **Three places, and no fourth** (spec 102). The Trusted Signer was a fourth
/// value here from spec 075 until 102: a page is not a place a key lives — the
/// same three places exist on the page too — but where a person reviews and
/// signs, which is the account's [`crate::signing_venue::SigningVenue`]. A
/// record that still says `trusted_signer` is read by the account reader
/// (`LegacyMethod`) and mapped; nothing writes it, and nothing else accepts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum KeyMethod {
    /// The authenticator built into the device the app is running on.
    #[default]
    Platform,
    /// A nearby device, reached by scanning a code.
    Hybrid,
    /// A removable authenticator — a USB/NFC security key.
    SecurityKey,
}

impl KeyMethod {
    /// Every place, in the order the choosers draw them.
    pub const ALL: [Self; 3] = [Self::Platform, Self::Hybrid, Self::SecurityKey];

    /// The wire name — `platform` | `hybrid` | `security_key`.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Platform => "platform",
            Self::Hybrid => "hybrid",
            Self::SecurityKey => "security_key",
        }
    }
}

/// How a ceremony failed. The **shell** reports the raw platform error; the
/// classification is the core's, so both machines branch on the same vocabulary.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum FailureKind {
    /// The user dismissed the OS sheet. Never an error state, never an alert.
    Cancelled,
    NotSupported,
    /// A device-local credential that would never appear at sign-in (issue #1).
    NotDiscoverable,
    Other,
}

/// The transient line under the create form. Semantic — the shell owns the
/// words, so 14 locales stay out of the wasm.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum StatusKey {
    SettingUpIdentity,
    VerifyingIdentity,
    ExtractingKey,
    ComputingAddress,
    SyncingKey,
    SetupCancelled,
    VerifyCancelled,
}

/// A question or notice for the user. One variant per existing `showAlert` call
/// site; `RecoverOffer` is the only one whose answer changes the flow.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum PromptKind {
    /// This device cannot run the ceremony. `security_key`: the route was a
    /// security key (issue #450), so the sheet talks about the KEY — "no
    /// biometric authentication" sent an iPad owner holding a YubiKey off to
    /// look for Face ID. `#[serde(default)]`: a reader that predates it reads
    /// `false`.
    NotSupportedCreate {
        #[serde(default)]
        security_key: bool,
    },
    /// As [`PromptKind::NotSupportedCreate`], at sign-in.
    NotSupportedLogin {
        #[serde(default)]
        security_key: bool,
    },
    NotDiscoverable,
    IncompatibleCreate,
    IncompatibleLogin,
    CreateFailed {
        detail: String,
        /// The link to the other device failed, not the authenticator
        /// ([`crate::cable::conn::is_link_failure`]): the sheet says to scan
        /// again (issue #446). `#[serde(default)]`: a reader that predates it
        /// reads `false`.
        #[serde(default)]
        phone_link: bool,
    },
    /// Only on Gnosis's verdict that the passkey has no record: the rebuild
    /// makes a ONE-key wallet, and nobody else's "no record" can rule out a
    /// multi-key one.
    RecoverOffer,
    RecoverFailed,
    /// Sign-in could not find out whether this passkey has a wallet record:
    /// a lookup failed, or nobody but the index vouched for its answer.
    /// Nothing was saved. Confirmable — "Try again" asks again from the
    /// signature already made (no new passkey prompt); "Cancel" ends it.
    /// `local`: every failed lookup never left this device, so the sheet says
    /// "check your connection" rather than blame the registry.
    RegistryUnreachable {
        #[serde(default)]
        local: bool,
    },
    SignInFailed {
        detail: String,
        /// As [`PromptKind::CreateFailed`]'s.
        #[serde(default)]
        phone_link: bool,
    },
}

impl PromptKind {
    /// A failed create, classified: the detail is the platform's (or the
    /// core's own) words, kept for the technical details.
    #[must_use]
    pub fn create_failed(detail: String) -> Self {
        let phone_link = crate::cable::conn::is_link_failure(&detail);
        Self::CreateFailed { detail, phone_link }
    }

    /// A failed sign-in, classified as [`PromptKind::create_failed`].
    #[must_use]
    pub fn sign_in_failed(detail: String) -> Self {
        let phone_link = crate::cable::conn::is_link_failure(&detail);
        Self::SignInFailed { detail, phone_link }
    }

    /// [`PromptKind::create_failed`] for a CEREMONY run with `method`. A
    /// phone (hybrid) ceremony's failure is the link's by definition — the
    /// person's Face ID or fingerprint is on the OTHER device — whatever the
    /// platform's words: "No phone answered the code" timed out on Android
    /// and still said to set up Face ID here (issue #446).
    #[must_use]
    pub fn create_failed_via(detail: String, method: KeyMethod) -> Self {
        let phone_link =
            method == KeyMethod::Hybrid || crate::cable::conn::is_link_failure(&detail);
        Self::CreateFailed { detail, phone_link }
    }

    /// A create ceremony `method` could not run here (issue #450).
    #[must_use]
    pub fn not_supported_create(method: KeyMethod) -> Self {
        Self::NotSupportedCreate {
            security_key: method == KeyMethod::SecurityKey,
        }
    }

    /// A sign-in ceremony `method` could not run here (issue #450).
    #[must_use]
    pub fn not_supported_login(method: KeyMethod) -> Self {
        Self::NotSupportedLogin {
            security_key: method == KeyMethod::SecurityKey,
        }
    }

    /// [`PromptKind::sign_in_failed`] for a ceremony run with `method`, as
    /// [`PromptKind::create_failed_via`].
    #[must_use]
    pub fn sign_in_failed_via(detail: String, method: KeyMethod) -> Self {
        let phone_link =
            method == KeyMethod::Hybrid || crate::cable::conn::is_link_failure(&detail);
        Self::SignInFailed { detail, phone_link }
    }
}

// ---------------------------------------------------------------------------
// Pure helpers shared by both machines
// ---------------------------------------------------------------------------

/// Uncompressed SEC1 public key (`04 ‖ x ‖ y`) from an attestation object.
pub(crate) fn public_key_hex_from_attestation(
    attestation_object_hex: &str,
) -> Result<String, CoreError> {
    let bytes = primitives::from_hex(attestation_object_hex)?;
    let key = webauthn::extract_attestation_public_key(&bytes)?;
    Ok(format!(
        "04{}{}",
        primitives::to_hex(&key.x, false),
        primitives::to_hex(&key.y, false)
    ))
}

/// The counterfactual Safe address for a public key — the wallet's identity.
pub(crate) fn address_from_public_key_hex(public_key_hex: &str) -> Result<String, CoreError> {
    let key = safe::parse_public_key(public_key_hex)?;
    Ok(safe::compute_safe_address(&key.x, &key.y)?.address)
}

/// The counterfactual Safe address for a founding key set. Byte-identical to
/// [`address_from_public_key_hex`] for a single key (the release-gated
/// `compute_safe_address_multi` N=1 equivalence); `keys[0]` is the pinned
/// shared-signer key, the rest are canonically ordered inside.
pub(crate) fn address_from_public_key_hexes(hexes: &[String]) -> Result<String, CoreError> {
    let keys = hexes
        .iter()
        .map(|hex| safe::parse_public_key(hex))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(safe::compute_safe_address_multi(&keys)?.address)
}

impl Assertion {
    /// Decode into the byte-level assertion the crypto kernels take.
    pub(crate) fn to_core(&self) -> Result<WebAuthnAssertion, CoreError> {
        Ok(WebAuthnAssertion {
            authenticator_data: primitives::from_hex(&self.authenticator_data_hex)?,
            client_data_json: primitives::from_hex(&self.client_data_json_hex)?,
            signature_der: primitives::from_hex(&self.signature_der_hex)?,
        })
    }

    /// Byte-level acceptance check against the Safe on-chain verifier. A
    /// provider that fails this can produce signatures the wallet's contracts
    /// will never accept, so the flow must stop before anything is persisted.
    pub(crate) fn is_safe_compatible(&self) -> bool {
        let (Ok(client_data), Ok(auth_data)) = (
            primitives::from_hex(&self.client_data_json_hex),
            primitives::from_hex(&self.authenticator_data_hex),
        ) else {
            return false;
        };
        webauthn::validate_client_data(ClientDataKind::Get, &client_data, &auth_data).is_ok()
    }

    /// The wallet name carried in the credential's user handle, or `None`.
    ///
    /// `user.id` is the UTF-8 bytes of `name\0uuid` on every platform. Anything
    /// that is not exactly that shape is rejected rather than guessed at: a
    /// foreign credential's random handle must never become an account name (and
    /// from there reach the public key index), and a Latin-1 read of UTF-8 bytes
    /// turned every non-ASCII name into mojibake. Mirrors
    /// `decodeUserNameFromHandle` in `src/modules/passkey/index.ts`.
    pub(crate) fn user_name(&self) -> Option<String> {
        let hex = self.user_id_hex.as_deref()?;
        let bytes = primitives::from_hex(hex).ok()?;
        let text = String::from_utf8(bytes).ok()?;
        let (name, uuid) = text.split_once('\0')?;
        if !is_uuid_v4_shape(uuid) {
            return None;
        }
        if !valid_display_name(name) {
            return None;
        }
        Some(name.to_owned())
    }
}

/// A displayable wallet name: non-empty, ≤64 UTF-16 units, no control
/// characters or U+FFFD — the same bar `user_name()` holds the handle to,
/// shared so a server-recovered name cannot smuggle in what a handle cannot.
pub(crate) fn valid_display_name(name: &str) -> bool {
    !name.is_empty() && name.encode_utf16().count() <= 64 && !has_unprintable(name)
}

/// `8-4-4-4-12` hex, case-insensitive: the web encoder emits lowercase but
/// iOS `UUID().uuidString` is UPPERCASE — a name must never be lost because
/// the uuid tail's case differs.
fn is_uuid_v4_shape(candidate: &str) -> bool {
    const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
    let mut parts = candidate.split('-');
    for len in GROUPS {
        let Some(part) = parts.next() else {
            return false;
        };
        if part.len() != len || !part.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
    }
    parts.next().is_none()
}

/// C0/C1 control characters and U+FFFD — the `UNPRINTABLE_RE` guard. U+FFFD is
/// included because a lenient UTF-8 decoder substitutes it instead of failing,
/// which would smuggle a garbled name through an otherwise strict check.
fn has_unprintable(name: &str) -> bool {
    name.chars()
        .any(|c| c <= '\u{1f}' || ('\u{7f}'..='\u{9f}').contains(&c) || c == '\u{fffd}')
}

/// Does this name fit the WebAuthn user-handle budget?
pub(crate) fn name_fits_user_handle(name: &str) -> bool {
    name.len() <= MAX_USER_NAME_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn handle(text: &str) -> Assertion {
        Assertion {
            credential_id: "cred".to_owned(),
            signature_der_hex: String::new(),
            authenticator_data_hex: String::new(),
            client_data_json_hex: String::new(),
            user_id_hex: Some(primitives::to_hex(text.as_bytes(), false)),
            authenticator_attachment: String::new(),
            signer_origin: None,
        }
    }

    const UUID: &str = "0f8fad5b-d9cb-469f-a165-70867728950e";

    #[test]
    fn user_name_decodes_utf8_names_without_mojibake() {
        assert_eq!(
            handle(&format!("看看书\0{UUID}")).user_name().as_deref(),
            Some("看看书")
        );
    }

    #[test]
    fn user_name_rejects_a_foreign_handle() {
        // No separator, no uuid — a credential this app did not mint.
        assert_eq!(handle("some-random-opaque-handle").user_name(), None);
        // Separator present but the tail is not a uuid.
        assert_eq!(handle("Ann\0not-a-uuid").user_name(), None);
    }

    #[test]
    fn user_name_rejects_unprintable_and_empty_names() {
        assert_eq!(handle(&format!("\u{7}bad\0{UUID}")).user_name(), None);
        assert_eq!(handle(&format!("\0{UUID}")).user_name(), None);
    }

    fn two_keys(sign_in_key: Option<SignInKey>) -> Account {
        let key = |id: &str, transports: &str| AccountKey {
            credential_id: id.to_owned(),
            public_key_hex: "04ab".to_owned(),
            name: id.to_owned(),
            transports: transports.to_owned(),
            signer_origin: None,
        };
        Account {
            id: "first".to_owned(),
            name: "Ann".to_owned(),
            address: "0x2222222222222222222222222222222222222222".to_owned(),
            public_key_hex: "04ab".to_owned(),
            created_at_iso: "2026-09-26T00:00:00.000Z".to_owned(),
            keys: vec![key("first", "internal"), key("second", "usb,nfc")],
            sign_in_key,
            signing_domain: crate::signing_venue::APP_DOMAIN.to_owned(),
            signing_venue: crate::signing_venue::SigningVenue::InVela,
        }
    }

    fn signed_in(credential_id: &str, method: KeyMethod) -> Option<SignInKey> {
        Some(SignInKey {
            credential_id: credential_id.to_owned(),
            method,
            transports: String::new(),
        })
    }

    fn read(json: &serde_json::Value) -> Account {
        serde_json::from_value(json.clone()).unwrap_or_else(|e| unreachable!("reads: {e}"))
    }

    /// Founder, 2026-09-26: signing reuses the sign-in's key AND place — not
    /// the first key, and not where that key was registered. R5: the route
    /// carries the place's hints for a page to pass on.
    #[test]
    fn signatures_go_to_the_sign_in_key_over_its_route() {
        let account = two_keys(signed_in("second", KeyMethod::SecurityKey));
        let route = account.key_route().unwrap_or_else(|| unreachable!());
        assert_eq!(route.credential_id, "second", "not keys[0]");
        assert_eq!(route.method, "security_key");
        assert_eq!(route.transports, "usb,nfc,ble");
        assert_eq!(route.hints, ["security-key"]);

        // A synced passkey registered as `internal`, reached here by a scan.
        let account = two_keys(signed_in("first", KeyMethod::Hybrid));
        let route = account.key_route().unwrap_or_else(|| unreachable!());
        assert_eq!(
            (route.method.as_str(), route.transports.as_str()),
            ("hybrid", "hybrid,internal")
        );
        assert_eq!(route.hints, ["hybrid"]);
    }

    /// "This device" was picked, and the system sheet was answered by a phone
    /// or a key on the desk (a cross-platform attachment). The route names the
    /// choice AND where the key was found, so the next signature is not aimed
    /// only at an authenticator that does not hold it.
    #[test]
    fn the_route_also_names_where_the_sign_in_found_the_key() {
        let mut account = two_keys(signed_in("first", KeyMethod::Platform));
        if let Some(key) = account.sign_in_key.as_mut() {
            key.transports = "usb,nfc,ble,hybrid".to_owned();
        }
        let route = account.key_route().unwrap_or_else(|| unreachable!());
        assert_eq!(route.method, "platform");
        assert_eq!(route.transports, "internal,usb,nfc,ble,hybrid");

        // Found where it was chosen: nothing added, nothing repeated.
        if let Some(key) = account.sign_in_key.as_mut() {
            key.transports = "internal".to_owned();
        }
        let route = account.key_route().unwrap_or_else(|| unreachable!());
        assert_eq!(route.transports, "internal");
    }

    /// No record, or one naming a key this wallet does not hold: the shell
    /// signs as it always did.
    #[test]
    fn no_usable_sign_in_key_means_no_route() {
        assert_eq!(two_keys(None).key_route(), None);
        assert_eq!(
            two_keys(signed_in("stranger", KeyMethod::Platform)).key_route(),
            None
        );
        assert_eq!(
            two_keys(signed_in("", KeyMethod::Platform)).key_route(),
            None
        );
    }

    /// The plan answers venue and key together, and a `getvela.app` account
    /// is never blocked.
    #[test]
    fn the_plan_is_venue_domain_and_key() {
        let mut account = two_keys(signed_in("second", KeyMethod::SecurityKey));
        let plan = account.signing_plan();
        assert_eq!(plan.domain, "getvela.app");
        assert_eq!(plan.venue, crate::signing_venue::SigningVenue::InVela);
        assert_eq!(plan.blocked, None);
        assert_eq!(plan.key, account.key_route());

        // Choosing the official page: allowed, nothing else changes.
        assert_eq!(
            account.choose_venue(crate::signing_venue::SigningVenue::official()),
            Ok(())
        );
        assert_eq!(
            account.signing_plan().venue,
            crate::signing_venue::SigningVenue::official()
        );
        assert_eq!(account.signing_plan().key, plan.key);

        // A page on another domain: refused, and the venue stands (R1).
        let other = crate::signing_venue::SigningVenue::Page {
            url: "https://sign.example.com".to_owned(),
        };
        assert!(matches!(
            account.choose_venue(other),
            Err(crate::signing_venue::VenueBlock::PageOnOtherDomain { .. })
        ));
        assert_eq!(
            account.signing_venue,
            crate::signing_venue::SigningVenue::official()
        );
    }

    /// A record this build wrote reads back as it was — and carries, beside
    /// it, the copy an older build reads.
    #[test]
    fn a_record_round_trips_with_its_copy_for_older_builds() {
        let mut account = two_keys(signed_in("second", KeyMethod::Hybrid));
        account.signing_venue = crate::signing_venue::SigningVenue::official();
        let value = serde_json::to_value(&account).unwrap_or_default();
        assert_eq!(read(&value), account);
        assert_eq!(value["signing_domain"], "getvela.app");
        assert_eq!(value["signing_venue"]["type"], "page");
        // The copy: the sign-in key in the shape a build before 102 reads.
        assert_eq!(value["signed_in_with"]["credential_id"], "second");
        assert_eq!(value["signed_in_with"]["method"], "hybrid");
        let text = value.to_string();
        assert!(!text.contains("trusted_signer"), "{text}");

        // A sign-in key a newer build added: the account still reads.
        let mut newer = value.clone();
        newer["sign_in_key"]["method"] = "telepathy".into();
        newer["signed_in_with"]["method"] = "telepathy".into();
        let back = read(&newer);
        assert_eq!(back.sign_in_key, None);
        assert_eq!(back.keys, account.keys);

        // Records with no sign-in key write none, and no copy.
        let json = serde_json::to_string(&two_keys(None)).unwrap_or_default();
        assert!(
            !json.contains("sign_in_key") && !json.contains("signed_in_with"),
            "{json}"
        );
    }

    /// An older build that signed in again rewrote the copy alone: the copy is
    /// then the newer fact.
    #[test]
    fn a_copy_an_older_build_rewrote_wins() {
        let account = two_keys(signed_in("first", KeyMethod::Platform));
        let mut value = serde_json::to_value(&account).unwrap_or_default();
        value["signed_in_with"] =
            serde_json::json!({ "credential_id": "second", "method": "security_key" });
        let back = read(&value);
        assert_eq!(
            back.sign_in_key,
            signed_in("second", KeyMethod::SecurityKey)
        );
    }

    /// A stored venue that cannot reach the keys is never read back (R1): the
    /// reader falls to one that can.
    #[test]
    fn a_stored_venue_that_cannot_reach_the_keys_is_replaced() {
        let account = two_keys(None);
        let mut value = serde_json::to_value(&account).unwrap_or_default();
        value["signing_venue"] =
            serde_json::json!({ "type": "page", "url": "https://sign.example.com/" });
        assert_eq!(
            read(&value).signing_venue,
            crate::signing_venue::SigningVenue::InVela
        );
        // Garbage costs only the venue.
        value["signing_venue"] = serde_json::json!(42);
        assert_eq!(
            read(&value).signing_venue,
            crate::signing_venue::SigningVenue::InVela
        );
    }

    /// A custom-domain account: no copy for older builds (they would sign
    /// natively with keys they cannot reach), and every key names the page —
    /// which is what an older build follows.
    #[test]
    fn a_custom_domain_account_keeps_older_builds_on_its_page() {
        let mut account = two_keys(signed_in("first", KeyMethod::Platform));
        account.signing_domain = "sign.example.com".to_owned();
        account.signing_venue = crate::signing_venue::SigningVenue::Page {
            url: "https://sign.example.com/".to_owned(),
        };
        account.stamp_page_origin();
        let value = serde_json::to_value(&account).unwrap_or_default();
        assert!(value.get("signed_in_with").is_none(), "{value}");
        for key in value["keys"].as_array().into_iter().flatten() {
            assert_eq!(key["signer_origin"], "https://sign.example.com");
        }
        let back = read(&value);
        assert_eq!(back, account);
        assert!(back.signing_plan().blocked.is_none());
    }

    /// The door the web and the phones call: the stored record in, the plan
    /// out, and `null` rather than a guess for anything it cannot read.
    #[test]
    fn the_json_door_answers_like_the_account() {
        let account = two_keys(signed_in("second", KeyMethod::Hybrid));
        let json = serde_json::to_string(&account).unwrap_or_default();
        let plan: crate::signing_venue::SigningPlan =
            serde_json::from_str(&signing_plan_json(&json).unwrap_or_default())
                .unwrap_or_else(|_| unreachable!());
        assert_eq!(plan, account.signing_plan());
        assert_eq!(signing_plan_json("not json"), None);
    }

    #[test]
    fn a_new_record_starts_in_vela_or_on_the_page_the_person_chose() {
        use crate::signing_venue::SigningVenue;
        assert_eq!(
            new_signing(None),
            ("getvela.app".to_owned(), SigningVenue::InVela)
        );
        assert_eq!(
            new_signing(Some("https://sign.getvela.app")),
            ("getvela.app".to_owned(), SigningVenue::official())
        );
        assert_eq!(
            new_signing(Some("http://localhost:8140")),
            (
                "localhost".to_owned(),
                SigningVenue::Page {
                    url: "http://localhost:8140/".to_owned()
                }
            )
        );
        // Not a page: nothing to lock to.
        assert_eq!(
            new_signing(Some("ftp://nope")),
            ("getvela.app".to_owned(), SigningVenue::InVela)
        );
    }

    #[test]
    fn name_budget_counts_utf8_bytes_not_characters() {
        assert!(name_fits_user_handle("Ann"));
        assert!(name_fits_user_handle("九个汉字刚好合适")); // 8 × 3 = 24 bytes
        assert!(!name_fits_user_handle("十个汉字就超过了预算")); // 10 × 3 = 30 bytes
    }
}
