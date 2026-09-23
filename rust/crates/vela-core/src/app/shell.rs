//! The vocabulary both onboarding machines use to ask the outside world for
//! something, and the vocabulary the outside world answers in.
//!
//! Shared on purpose: six of these operations are used by both flows, so the web
//! shell implements **one** executor and the generated TypeScript has one union
//! instead of two overlapping ones.
//!
//! Nothing here performs the work. `RegisterPasskey` is a *sentence*, not a
//! ceremony; `SaveAccount` is a *request*, not a write. The shell owns `rpId`,
//! endpoint URLs, timeouts, challenge material and the clock — none of which
//! appear in this file, which is why no core test can depend on a domain, a
//! network condition or the time of day.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::render::RenderOperation;
use serde::{Deserialize, Serialize};

use super::{
    Account, Assertion, FailureKind, KeyMethod, PendingUpload, PromptKind, Registration,
    RegistryPublishMember, RegistryUnitMember,
};
use crate::registry_proof::RegistryProof;

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// Why a signature is being requested. The shell mints a fresh challenge per
/// request; the purpose only selects which challenge label it uses, preserving
/// today's `vela-verify-…` / `vela-recover-…` strings.
///
/// The "two recovery signatures must differ" invariant is **not** trust in the
/// shell: `recover_public_key_from_assertions` returns `None` unless the two
/// assertions pin down exactly one key, so a repeated challenge fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ProofPurpose {
    /// Prove a freshly registered passkey can actually sign (issue #1).
    Verify,
    /// First of the two signatures that rebuild a lost public key.
    RecoverFirst,
    /// Second of the two — over a different challenge.
    RecoverSecond,
}

/// How onboarding hands the wallet over to the app.
///
/// Two shapes because sign-in resolves differently: a locally known credential
/// restores the whole account list with the right one selected, everything else
/// appends the single account it just established.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum CompletionMode {
    SetWallet {
        accounts: Vec<Account>,
        active_index: usize,
    },
    AddAccount {
        account: Account,
    },
}

/// Everything below this line must be performed by a platform shell.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ShellOperation {
    /// Is a passkey authenticator available at all?
    CheckPasskeySupport,
    /// `navigator.credentials.create()` — mint a passkey for this name.
    RegisterPasskey {
        name: String,
        /// Credentials the authenticator must refuse to reuse
        /// (`excludeCredentials`): the wallet's already-registered founding
        /// keys, so a provider cannot silently replace one of them when the
        /// user adds another key. Empty for the first key.
        #[serde(default)]
        exclude_credential_ids: Vec<String>,
        /// Which authenticator the *person* asked for. Selects the ceremony —
        /// the platform authenticator, a nearby device, or a removable security
        /// key — and nothing else. What the authenticator turns out to report
        /// about itself comes back on the registration and is not constrained
        /// by this.
        #[serde(default)]
        method: KeyMethod,
    },
    /// `navigator.credentials.get()` against a known credential.
    SignProof {
        credential_id: String,
        /// What the authenticator reported about ITSELF at registration, comma
        /// joined (`hybrid,internal`, `usb,nfc`, …), or empty when unknown.
        /// See [`ShellOperation::SignMemberProof::transports`] — this is the
        /// same fact and the same reason.
        #[serde(default)]
        transports: String,
        /// Which authenticator route to sign over. A proof must reach the SAME
        /// key the first signature did: recovery's second signature runs on the
        /// route the person signed in with (a phone over caBLE, or a plugged-in
        /// security key), so the platform cannot silently answer it with a
        /// different credential. Defaults to the platform authenticator, the
        /// value a shell that never sets it would expect.
        #[serde(default)]
        method: KeyMethod,
        purpose: ProofPurpose,
        /// Spec 075: with `method = trusted_signer`, the page the key lives
        /// behind (the sign-in's own). `None` on every other route.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signer_origin: Option<String>,
    },
    /// Mint the one-time software group key for a wallet's registry group.
    /// All randomness lives in the shell; the seed never touches the core's
    /// serialized state beyond being echoed into the final publish.
    GenerateGroupKey,
    /// One founding passkey confirms its group membership AT CREATION: the
    /// member challenge binds only (groupPublicKey, own attestation) — the
    /// contract's `memberBindingFor` — so it exists before the rest of the
    /// set does. The executor fetches the member-mode challenge, runs the
    /// `get()`, and assembles the proof in the core.
    SignMemberProof {
        credential_id: String,
        /// Uncompressed P-256 point, `04‖x‖y` hex.
        public_key_hex: String,
        /// Empty, or 20 versioned attestation bytes (hex).
        attestation_hex: String,
        /// WHERE this credential lives, as the authenticator reported it at
        /// registration (`getTransports()`), comma joined: `hybrid,internal`,
        /// `usb,nfc`, `internal`. Empty when it reported nothing.
        ///
        /// **Load-bearing, not a hint.** A `get()` whose `allowCredentials`
        /// entry carries no transports leaves the platform to guess where to
        /// look, and Android's Credential Manager guesses "removable security
        /// key" — it drew "Connect your security key" for a passkey living in
        /// Apple Passwords on another phone, which is a dead end the person
        /// cannot answer (device-found 2026-08-26). With `hybrid` present the
        /// platform offers the other-device route instead, which is the one
        /// that can actually complete.
        #[serde(default)]
        transports: String,
        /// Which authenticator route to sign over — the same fact, and the same
        /// reason, as [`ShellOperation::SignProof::method`]. The confirmation
        /// must reach the key that was JUST minted, so it runs on the route
        /// that minted it: a key created on a phone over caBLE is confirmed on
        /// that phone, not on whatever is in the USB port.
        ///
        /// `transports` above says the same thing to a platform that ROUTES for
        /// itself (a browser, Android's Credential Manager). A shell that is its
        /// own CTAP client picks the transport itself and has nothing to read it
        /// from — which is why both fields ride along and neither replaces the
        /// other.
        #[serde(default)]
        method: KeyMethod,
        group_public_key_hex: String,
        /// Spec 075: with `method = trusted_signer`, the page the key was just
        /// minted behind — the membership is confirmed there. `None` on every
        /// other route.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        signer_origin: Option<String>,
    },
    /// The v1 index's display name for a credential — the only place a
    /// v1-era wallet's name survives (v1 stored it server-side; a handle
    /// that decodes carries its own). Best-effort and read-only.
    LookupLegacyName {
        credential_id: String,
    },
    /// `navigator.credentials.get()` with no credential hint — "who are you?".
    /// `method` is the person's choice on the sign-in screen: `Platform` lets
    /// the system sheet answer (a device passkey, or its own security-key/scan
    /// routes), `SecurityKey` forces the app-owned CTAP path — the only way to
    /// sign into a wallet on a security key when a platform passkey is also
    /// present and the system would otherwise use it silently.
    AuthenticatePasskey {
        #[serde(default)]
        method: KeyMethod,
    },
    /// Read every locally stored account.
    LoadAccounts,
    SaveAccount {
        account: Account,
    },
    SavePendingUpload {
        record: PendingUpload,
    },
    RemovePendingUpload {
        credential_id: String,
    },
    /// Publish the wallet's key set as one possession-proven registry group.
    /// With `group_seed_hex` set (the interleaved create flow), the members
    /// carry proofs collected at creation and the executor only closes the
    /// group (software group proof) and registers — no prompts. With it
    /// empty (the login re-publish), the executor runs the whole legacy
    /// mechanism: fresh group key, challenges, one `get()` per member.
    /// `metadata_hex` is the group's opaque blob, already encoded.
    RegistryPublish {
        metadata_hex: String,
        members: Vec<RegistryPublishMember>,
        #[serde(default)]
        group_seed_hex: String,
        #[serde(default)]
        group_public_key_hex: String,
        /// Which authenticator route a member with no replayable proof must sign
        /// its live possession proof over. It matters only on desktop, and only
        /// for the recovery re-publish (a phone credential signs over caBLE, not
        /// the USB path — which would find no key and show no QR); a create
        /// replays its creation-time proof and never signs live. Defaults to the
        /// platform authenticator, what a shell that never sets it expects.
        #[serde(default)]
        method: KeyMethod,
    },
    /// Is this public key already an entry in the registry? Lets a sign-in
    /// skip a redundant re-publish (and its extra signature).
    RegistryQueryByPublicKey {
        public_key_hex: String,
    },
    /// Fetch one registry group (Unit): its metadata blob and its founding
    /// members. The only way a sibling device can reconstruct a multi-key
    /// wallet's full key set — and with it the address.
    ///
    /// `u32`, not `u64`: the wire is JSON (see [`ShellOperation::Wait`]); the
    /// shell rejects an id past `2^32` as an index failure instead of
    /// truncating it.
    RegistryQueryUnit {
        unit_id: u32,
    },
    /// One health probe of the index server.
    ProbeIndexHealth,
    /// Wait, without the core owning a clock. Used for retry backoff.
    ///
    /// `u32`, not `u64`: the wire is JSON and `JSON.parse` yields a `number`, so
    /// a 64-bit type would generate a TypeScript `bigint` the shell never
    /// actually receives.
    Wait {
        ms: u32,
    },
    /// Ask (or tell) the user something. `confirmable` selects a two-button
    /// dialog whose answer is a business decision.
    Prompt {
        kind: PromptKind,
        confirmable: bool,
    },
    /// Hand the wallet to the app and leave onboarding.
    CompleteOnboarding {
        mode: CompletionMode,
    },
}

impl Operation for ShellOperation {
    type Output = ShellResult;
}

/// What the shell observed. Failures arrive as *variants*, never as exceptions:
/// the executor converts every rejected promise into the failure result that
/// belongs to the operation, which is what lets the core own classification
/// instead of pattern-matching error strings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum ShellResult {
    PasskeySupport {
        supported: bool,
    },
    /// `now_iso` is the wall clock the shell observed while doing the work —
    /// reported alongside the observation so the core stays a pure function of
    /// its inputs (no clock effect, no clock in tests).
    PasskeyRegistered {
        registration: Registration,
        now_iso: String,
    },
    ProofSigned {
        assertion: Assertion,
        now_iso: String,
    },
    /// The one-time software group key the shell just minted.
    GroupKeyGenerated {
        seed_hex: String,
        /// Uncompressed P-256 point, `04‖x‖y` hex (no `0x`).
        group_public_key_hex: String,
    },
    /// A founding passkey's possession proof, assembled from its
    /// creation-time `get()` over the member-mode challenge.
    MemberProofSigned {
        proof: RegistryProof,
    },
    /// The v1 record's display name, or None (absent record, offline, or a
    /// v2-era wallet). Never an error — a lost name degrades the label only.
    LegacyName {
        name: Option<String>,
    },
    PasskeyAuthenticated {
        assertion: Assertion,
        now_iso: String,
    },
    PasskeyFailed {
        kind: FailureKind,
        /// The platform's own words, forwarded verbatim for the
        /// "something went wrong" alert. Absent for classified failures, whose
        /// copy comes from the classification.
        message: Option<String>,
    },
    AccountsLoaded {
        accounts: Vec<Account>,
    },
    AccountSaved,
    PendingUploadSaved,
    PendingUploadRemoved,
    StorageFailed {
        message: String,
    },
    /// The registry publish landed on-chain (or the identical group was
    /// already there).
    RegistryPublished,
    /// The registry's answer to `RegistryQueryByPublicKey`.
    RegistryKeyStatus {
        registered: bool,
        /// The ids of the groups (Units) this key is a founding member of,
        /// ascending. Empty for a registered key predating groups.
        #[serde(default)]
        unit_ids: Vec<u32>,
    },
    /// The registry's answer to `RegistryQueryUnit`: the group's frozen
    /// metadata blob plus its founding members in canonical founding order
    /// (the ascending on-chain member order IS the founding order).
    RegistryUnit {
        metadata_hex: String,
        members: Vec<RegistryUnitMember>,
    },
    IndexFailed {
        message: String,
        /// True when the request never reached the server (transport failure or
        /// abort). Only the shell can tell that from a 4xx, so this one bit of
        /// classification is delegated.
        network: bool,
    },
    IndexHealth {
        ok: bool,
    },
    /// The health probe could not get OUT of the machine: every route the
    /// shell knows — system proxy, environment proxy, direct — refused before
    /// anything reached the index (spec 038 Part B). Distinct from
    /// `IndexHealth { ok: false }`, where a route existed and the service did
    /// not answer: the first is the person's machine, the second is our
    /// service, and the screen must never blame one for the other. Additive —
    /// a shell that cannot tell the two apart keeps sending `IndexHealth`.
    IndexTransportFailed,
    Waited,
    PromptAnswered {
        accepted: bool,
    },
    OnboardingCompleted,
}

/// Render, or ask the shell for something. Shared by both machines.
#[effect]
pub enum Effect {
    Render(RenderOperation),
    Shell(ShellOperation),
}

impl super::SplitEffect for Effect {
    type Op = ShellOperation;
    fn into_shell(self) -> Option<crux_core::Request<ShellOperation>> {
        match self {
            Effect::Render(_) => None,
            Effect::Shell(request) => Some(request),
        }
    }
}
