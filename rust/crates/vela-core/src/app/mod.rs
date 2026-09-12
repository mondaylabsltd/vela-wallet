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
pub mod clear_signing;
pub mod contacts;
pub mod contacts_initials;
pub mod contacts_io;
pub mod create_wallet;
pub mod dapp_permissions;
pub mod dapp_session;
pub mod display_currency;
pub mod explore_sites;
pub mod ext_cache;
pub mod fee_policy;
pub mod login;
pub mod manage_tokens;
pub mod money;
pub mod network_admin;
pub mod payment_request;
pub mod receive_watch;
pub mod rpc_pool;
pub mod send;
pub mod session;
pub mod shell;
pub mod sign_request;
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
}

/// One passkey of a wallet, in canonical founding order. `keys[0]` is the
/// pinned key that signs through the shared `WEBAUTHN_SIGNER`; every later key
/// signs through its own counterfactual signer proxy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// The persisted wallet. Serialises 1:1 to `StoredAccount`.
///
/// The scalar `id`/`public_key_hex` fields are the legacy single-key shape and
/// stay authoritative for `keys[0]`: a multi-key account writes them as copies
/// of its first key, and a legacy record simply has no `keys` at all. Only
/// [`Account::key_hexes`] / [`Account::matches_credential`] may interpret this
/// duality — everything else asks them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct Account {
    pub id: String,
    pub name: String,
    pub address: String,
    pub public_key_hex: String,
    pub created_at_iso: String,
    /// Full founding key set. Empty ⇒ legacy single-key account (the scalar
    /// fields are the whole story). `#[serde(default)]` lets records written
    /// before this field existed deserialize unchanged.
    #[serde(default)]
    pub keys: Vec<AccountKey>,
}

impl Account {
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
}

/// One draft key inside a multi-member [`PendingUpload`], founding order.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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
}

/// A key set that still owes the index server a successful publish. Written
/// *before* the first upload attempt so an interrupted creation is retried on a
/// later launch.
///
/// The scalar fields mirror `members[0]` (legacy single-key records have no
/// `members` and the scalars are the whole story). A record with
/// `members.len() > 1` must never be retried silently — replaying the publish
/// takes one passkey prompt per member.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// How the person chose to mint a founding key.
///
/// This is the **choice**, not the report. `CreateKeyRow` also carries
/// `authenticator_attachment` / `transports` / `aaguid`, which are what the
/// authenticator said about *itself* — and the two can legitimately disagree
/// (a "this device" choice that resolves to a cross-platform authenticator).
/// The ceremony follows the choice; the row's provider line shows the report.
/// Neither is inferred from the other.
///
/// [`KeyMethod::Hybrid`] exists before anything can execute it. A later feature
/// adds the transport, not a core type, and until then a shell can render the
/// method as present-and-explained rather than absent — which is what the
/// design draws.
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
    NotSupportedCreate,
    NotSupportedLogin,
    NotDiscoverable,
    IncompatibleCreate,
    IncompatibleLogin,
    CreateFailed { detail: String },
    RecoverOffer,
    RecoverFailed,
    SignInFailed { detail: String },
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

    #[test]
    fn name_budget_counts_utf8_bytes_not_characters() {
        assert!(name_fits_user_handle("Ann"));
        assert!(name_fits_user_handle("九个汉字刚好合适")); // 8 × 3 = 24 bytes
        assert!(!name_fits_user_handle("十个汉字就超过了预算")); // 10 × 3 = 30 bytes
    }
}
