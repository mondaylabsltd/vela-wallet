//! CTAP2 commands, as bytes.
//!
//! One request encoder and one response decoder per command the wallet needs.
//! Nothing here talks to a device: a request is a `Vec<u8>` for a shell to put
//! on a wire, and a response is whatever came back off one.
//!
//! ## Canonical encoding is not a style choice
//!
//! CTAP2 requires the canonical CBOR encoding form (CTAP 2.1 §6): map keys
//! sorted by encoded length and then bytewise, definite lengths, no indefinite
//! streams. An authenticator is entitled to reject anything else, and — worse —
//! the `pinUvAuthParam` for a request is an HMAC over the request's own bytes,
//! so two encodings of "the same" map authenticate differently. Every map here
//! is built with integer keys inserted in ascending order, which is canonical
//! for the 1..=9 range CTAP uses, and the property test in `ctap_commands.rs`
//! holds that line.
//!
//! ## The shape the rest of the wallet expects
//!
//! `make_credential` hands back the pieces of an attestation object, and
//! [`attestation_object`] assembles them into exactly the CBOR blob a WebAuthn
//! `create()` would have produced — because that blob is what
//! [`crate::app::public_key_hex_from_attestation`] and
//! [`crate::webauthn::extract_attestation`] already know how to read. A CTAP
//! path that invented its own shape would need every one of those rewritten,
//! and would let the two paths disagree about a key.

use ciborium::Value;
use coset::CborSerializable as _;

use crate::error::CoreError;
use crate::types::P256PublicKey;
use crate::webauthn::p256_from_cose_key;

use super::pin_uv::Protocol;

/// The command byte that precedes the CBOR payload in a CTAPHID `CBOR` message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    MakeCredential = 0x01,
    GetAssertion = 0x02,
    GetInfo = 0x04,
    ClientPin = 0x06,
    GetNextAssertion = 0x08,
    Selection = 0x0b,
}

/// The authenticator's verdict, which is the first byte of every response.
///
/// Only the values a wallet acts on differently are named. Everything else
/// keeps its number: inventing a friendlier label for an error nobody has a
/// branch for loses the one detail a bug report needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Success,
    /// The person declined at the authenticator, or the ceremony timed out
    /// waiting for them (`CTAP2_ERR_OPERATION_DENIED`, `CTAP2_ERR_USER_ACTION_TIMEOUT`,
    /// `CTAP2_ERR_KEEPALIVE_CANCEL`).
    Cancelled,
    /// Every credential in `excludeList` is already on this authenticator
    /// (`CTAP2_ERR_CREDENTIAL_EXCLUDED`) — the founding-set guard doing its job.
    CredentialExcluded,
    /// No credential on this authenticator matches (`CTAP2_ERR_NO_CREDENTIALS`).
    NoCredentials,
    /// The PIN given was refused, or one is required and was not sent
    /// (`CTAP2_ERR_PIN_INVALID` 0x31, `CTAP2_ERR_PUAT_REQUIRED` 0x36). One
    /// attempt was spent; the caller may ask again.
    PinRequired,
    /// The authenticator has no PIN configured (`CTAP2_ERR_PIN_NOT_SET` 0x35),
    /// or refused an option this request needs (`CTAP2_ERR_UNSUPPORTED_OPTION`
    /// 0x2b — what a key with neither a PIN nor a biometric answers when a
    /// request asks for user verification). Distinct from
    /// [`Self::PinRequired`] because asking for a PIN again cannot help: the
    /// key has to be enrolled with the vendor's tool first.
    PinNotSet,
    /// Locked until the key is power-cycled or reset
    /// (`CTAP2_ERR_PIN_BLOCKED` 0x32, `CTAP2_ERR_PIN_AUTH_BLOCKED` 0x34).
    PinBlocked,
    /// The authenticator's own user verification did not succeed — the finger
    /// did not match (`CTAP2_ERR_UV_INVALID` 0x3f), or it has run out of
    /// attempts and only the PIN is left (`CTAP2_ERR_UV_BLOCKED` 0x3c). Both
    /// are a reason to OFFER the PIN, not to fail the ceremony.
    UvFailed,
    /// The authenticator's discoverable-credential storage is full
    /// (`CTAP2_ERR_KEY_STORE_FULL` 0x28). A Vela founding key must be
    /// discoverable, so there is no room to mint one until some are deleted —
    /// which a key accumulates fast under repeated testing (each create leaves
    /// a resident credential, and the key holds only 25–100).
    KeyStoreFull,
    /// Anything else, with its number intact.
    Other(u8),
}

impl Status {
    /// The numbers are CTAP 2.1 §6.3, and the three PIN groups are easy to
    /// cross — 0x32/0x33/0x34 are `PIN_BLOCKED` / `PIN_AUTH_INVALID` /
    /// `PIN_AUTH_BLOCKED`, which are three different sentences. Crossing them
    /// tells a person their key is bricked when their PIN was merely wrong, or
    /// offers a PIN retry to a key that will refuse every one until it is
    /// unplugged. `ctap_commands.rs` pins every byte in this match.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => Self::Success,
            // Checked against libfido2's `fido/err.h`, not recalled: until
            // 2026-09-19 this table had 0x19 as Cancelled and 0x21 as
            // CredentialExcluded, and no entry for 0x2d at all — and the test
            // beside it asserted the same numbers from the same memory. So a
            // key already in the founding set read as "you cancelled", and a
            // ceremony the person really did cancel (the client sends CTAPHID
            // CANCEL, the key answers 0x2d) read as a failed sign-in.
            0x19 => Self::CredentialExcluded, // CTAP2_ERR_CREDENTIAL_EXCLUDED
            0x27 | 0x2d | 0x2f => Self::Cancelled, // OPERATION_DENIED, KEEPALIVE_CANCEL, USER_ACTION_TIMEOUT
            // 0x21 is CTAP2_ERR_PROCESSING — nobody's decision; it keeps its number.
            0x28 => Self::KeyStoreFull,
            0x2b => Self::PinNotSet,
            0x2e => Self::NoCredentials,
            0x31 | 0x36 => Self::PinRequired,
            0x32 | 0x34 => Self::PinBlocked,
            0x35 => Self::PinNotSet,
            0x3c | 0x3f => Self::UvFailed,
            // 0x33 `PIN_AUTH_INVALID` deliberately falls through: it means the
            // pinUvAuthParam did not verify, which is a CLIENT fault (a wrong
            // shared secret, a wrong protocol number), not something a person
            // can fix by typing more carefully. It keeps its number so a bug
            // report says which one it was.
            other => Self::Other(other),
        }
    }

    pub fn is_success(self) -> bool {
        matches!(self, Self::Success)
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

/// One entry of `excludeList` / `allowList`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CredentialDescriptor {
    pub id: Vec<u8>,
}

/// What `authenticatorMakeCredential` needs.
///
/// `client_data_hash`, not client data: the authenticator signs a hash it never
/// interprets, which is what lets the caller decide what the ceremony is
/// *about* without the device having an opinion.
#[derive(Clone, Debug)]
pub struct MakeCredential {
    pub client_data_hash: Vec<u8>,
    pub rp_id: String,
    pub rp_name: String,
    pub user_id: Vec<u8>,
    pub user_name: String,
    pub user_display_name: String,
    /// Credentials this authenticator must refuse to duplicate. A wallet's
    /// already-founding keys go here so a provider cannot silently replace one.
    pub exclude: Vec<CredentialDescriptor>,
    /// Discoverable credential. Always true for a wallet: a key that cannot be
    /// found at sign-in is a key that cannot open the wallet.
    pub resident_key: bool,
    pub user_verification: bool,
    pub pin_uv_auth: Option<PinUvAuth>,
}

/// What `authenticatorGetAssertion` needs.
#[derive(Clone, Debug)]
pub struct GetAssertion {
    pub rp_id: String,
    pub client_data_hash: Vec<u8>,
    /// Empty means "any discoverable credential for this RP" — the
    /// "who are you?" ceremony sign-in starts with.
    pub allow: Vec<CredentialDescriptor>,
    /// Ask the person to touch the key.
    ///
    /// `false` is the SILENT probe: "do you hold this credential?", answered
    /// without lighting the key up. It is how a client with a credential id
    /// already in hand finds WHICH of several plugged-in keys holds it, instead
    /// of making all of them blink and asking a person to disambiguate
    /// something the client could have worked out itself.
    pub user_presence: bool,
    pub user_verification: bool,
    pub pin_uv_auth: Option<PinUvAuth>,
}

/// A `pinUvAuthToken` HMAC and the protocol that produced it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PinUvAuth {
    pub protocol: u8,
    pub param: Vec<u8>,
}

/// ES256, and only ES256.
///
/// The wallet's on-chain verifier is the RIP-7212 P-256 precompile and
/// two-signature recovery is ECDSA math, so an RSA credential can never become
/// a working wallet. Offering it would mint an orphan key in someone's
/// authenticator and fail later, somewhere less legible.
const ALG_ES256: i64 = -7;

fn map(entries: Vec<(i64, Value)>) -> Value {
    // Ascending integer keys ARE canonical order for the 1..=9 range CTAP uses
    // (equal encoded length, then bytewise). Callers below insert in order; the
    // sort makes that a property of this function rather than of every caller.
    let mut entries = entries;
    entries.sort_by_key(|(key, _)| *key);
    Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::Integer(key.into()), value))
            .collect(),
    )
}

fn credential_list(items: &[CredentialDescriptor]) -> Value {
    Value::Array(
        items
            .iter()
            .map(|item| {
                // "id" BEFORE "type": CTAP2 canonical CBOR orders text keys by
                // encoded length first. GMS's caBLE responder enforces it and
                // answers INVALID_CTAP to the reverse (device-found 2026-08-28:
                // recovery's second getAssertion — the first with an allowList —
                // died on it).
                Value::Map(vec![
                    (Value::Text("id".to_string()), Value::Bytes(item.id.clone())),
                    (
                        Value::Text("type".to_string()),
                        Value::Text("public-key".to_string()),
                    ),
                ])
            })
            .collect(),
    )
}

fn encode_value(command: Command, value: Option<Value>) -> Result<Vec<u8>, CoreError> {
    let mut out = Vec::new();
    out.push(command as u8);
    if let Some(value) = value {
        ciborium::ser::into_writer(&value, &mut out)
            .map_err(|error| CoreError::InvalidCbor(error.to_string()))?;
    }
    Ok(out)
}

impl MakeCredential {
    /// The request's bytes: the command byte, then canonical CBOR.
    pub fn encode(&self) -> Result<Vec<u8>, CoreError> {
        let mut entries = vec![
            (0x01, Value::Bytes(self.client_data_hash.clone())),
            (
                0x02,
                Value::Map(vec![
                    (
                        Value::Text("id".to_string()),
                        Value::Text(self.rp_id.clone()),
                    ),
                    (
                        Value::Text("name".to_string()),
                        Value::Text(self.rp_name.clone()),
                    ),
                ]),
            ),
            (
                0x03,
                Value::Map(vec![
                    (
                        Value::Text("id".to_string()),
                        Value::Bytes(self.user_id.clone()),
                    ),
                    (
                        Value::Text("name".to_string()),
                        Value::Text(self.user_name.clone()),
                    ),
                    (
                        Value::Text("displayName".to_string()),
                        Value::Text(self.user_display_name.clone()),
                    ),
                ]),
            ),
            (
                0x04,
                Value::Array(vec![Value::Map(vec![
                    (
                        Value::Text("alg".to_string()),
                        Value::Integer(ALG_ES256.into()),
                    ),
                    (
                        Value::Text("type".to_string()),
                        Value::Text("public-key".to_string()),
                    ),
                ])]),
            ),
        ];

        if !self.exclude.is_empty() {
            entries.push((0x05, credential_list(&self.exclude)));
        }

        let mut options = Vec::new();
        if self.resident_key {
            options.push((Value::Text("rk".to_string()), Value::Bool(true)));
        }
        if self.user_verification {
            options.push((Value::Text("uv".to_string()), Value::Bool(true)));
        }
        if !options.is_empty() {
            entries.push((0x07, Value::Map(options)));
        }

        if let Some(auth) = &self.pin_uv_auth {
            entries.push((0x08, Value::Bytes(auth.param.clone())));
            entries.push((0x09, Value::Integer(i64::from(auth.protocol).into())));
        }

        encode_value(Command::MakeCredential, Some(map(entries)))
    }
}

impl GetAssertion {
    pub fn encode(&self) -> Result<Vec<u8>, CoreError> {
        let mut entries = vec![
            (0x01, Value::Text(self.rp_id.clone())),
            (0x02, Value::Bytes(self.client_data_hash.clone())),
        ];

        if !self.allow.is_empty() {
            entries.push((0x03, credential_list(&self.allow)));
        }

        let mut options = Vec::new();
        if !self.user_presence {
            // `up` is emitted ONLY for the silent probe, explicitly false. A
            // normal request omits it: CTAP2 defaults `up` to true, so leaving
            // it out is the same request — and it is what real authenticators
            // expect. A redundant explicit `up:true` alongside `uv:true` is
            // rejected by some (GMS over caBLE drops the tunnel on it), and the
            // founder's proven demo never sends it.
            options.push((Value::Text("up".to_string()), Value::Bool(false)));
        }
        if self.user_verification {
            options.push((Value::Text("uv".to_string()), Value::Bool(true)));
        }
        if !options.is_empty() {
            entries.push((0x05, Value::Map(options)));
        }

        if let Some(auth) = &self.pin_uv_auth {
            entries.push((0x06, Value::Bytes(auth.param.clone())));
            entries.push((0x07, Value::Integer(i64::from(auth.protocol).into())));
        }

        encode_value(Command::GetAssertion, Some(map(entries)))
    }
}

/// Which `authenticatorClientPIN` job a request is asking for.
///
/// Only the four a wallet performs. Setting or changing a PIN is deliberately
/// absent: enrolling a security key is the authenticator vendor's flow, and a
/// wallet that offered to set a PIN would be taking custody of a credential it
/// has no way to recover.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum ClientPinSubcommand {
    /// How many PIN attempts remain before the key locks itself.
    GetPinRetries = 0x01,
    /// The authenticator's ephemeral public key for this session.
    GetKeyAgreement = 0x02,
    /// CTAP 2.0's token request: no permissions, no rpId.
    GetPinToken = 0x05,
    /// The token, obtained by the authenticator verifying the user ITSELF —
    /// a fingerprint on the key, not a PIN typed on the host. Preferred
    /// whenever `getInfo` reports built-in UV as configured: the person
    /// already chose a key with a sensor, and asking them to type instead is
    /// asking them to use the fallback.
    GetPinUvAuthTokenUsingUvWithPermissions = 0x06,
    /// How many built-in-UV attempts remain before it locks out and the PIN is
    /// the only way left.
    GetUvRetries = 0x07,
    /// CTAP 2.1's PIN request: the token is scoped to the operations and the RP
    /// named here.
    GetPinUvAuthTokenUsingPinWithPermissions = 0x09,
}

/// What a `pinUvAuthToken` is allowed to do (CTAP 2.1 §6.5.5.7).
///
/// A token minted for `mc | ga` on `getvela.app` cannot be replayed into a
/// credential-management command or against another relying party. CTAP 2.0's
/// `getPinToken` had no such scoping — which is why 2.1 added this and why the
/// wallet asks for the narrow token whenever the authenticator supports it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Permissions(u8);

impl Permissions {
    pub const MAKE_CREDENTIAL: Self = Self(0x01);
    pub const GET_ASSERTION: Self = Self(0x02);

    pub fn bits(self) -> u8 {
        self.0
    }
}

impl core::ops::BitOr for Permissions {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// One `authenticatorClientPIN` request.
///
/// The fields an individual subcommand does not use stay `None`; CTAP2 reads a
/// request by key, so sending a key the subcommand has no meaning for is a
/// protocol error rather than a harmless extra.
#[derive(Clone, Debug)]
pub struct ClientPin {
    pub protocol: Protocol,
    pub subcommand: ClientPinSubcommand,
    /// The PLATFORM's ephemeral public key. Minted by the shell — this crate
    /// has no randomness — and sent so the authenticator can complete the
    /// same ECDH from its side.
    pub key_agreement: Option<P256PublicKey>,
    /// `pinHashEnc`: the left 16 bytes of SHA-256(PIN), encrypted under the
    /// shared secret. The PIN itself never crosses the wire.
    pub pin_hash_enc: Option<Vec<u8>>,
    pub permissions: Option<Permissions>,
    /// The relying party the token is scoped to. `None` leaves the token
    /// unscoped, which an authenticator may refuse for `ga` permission.
    pub rp_id: Option<String>,
}

impl ClientPin {
    /// The `getKeyAgreement` request that opens every PIN session.
    pub fn key_agreement(protocol: Protocol) -> Self {
        Self {
            protocol,
            subcommand: ClientPinSubcommand::GetKeyAgreement,
            key_agreement: None,
            pin_hash_enc: None,
            permissions: None,
            rp_id: None,
        }
    }

    /// How many attempts are left. Asked BEFORE a PIN is offered, so a wallet
    /// can warn instead of spending the second-to-last try.
    pub fn pin_retries(protocol: Protocol) -> Self {
        Self {
            protocol,
            subcommand: ClientPinSubcommand::GetPinRetries,
            key_agreement: None,
            pin_hash_enc: None,
            permissions: None,
            rp_id: None,
        }
    }

    /// The token, verified by the authenticator's own sensor.
    ///
    /// Carries no `pinHashEnc`: nothing is typed, and there is no PIN in this
    /// exchange at all. The key prompts for a finger and answers with the same
    /// encrypted token the PIN path produces, so everything downstream is
    /// identical.
    ///
    /// Only defined for CTAP 2.1's permissions subcommand — an authenticator
    /// with built-in UV but no `pinUvAuthToken` support verifies inline
    /// instead, through the request's own `uv` option, and needs no token.
    pub fn uv_token(
        protocol: Protocol,
        platform_key: P256PublicKey,
        permissions: Permissions,
        rp_id: Option<String>,
    ) -> Self {
        Self {
            protocol,
            subcommand: ClientPinSubcommand::GetPinUvAuthTokenUsingUvWithPermissions,
            key_agreement: Some(platform_key),
            pin_hash_enc: None,
            permissions: Some(permissions),
            rp_id,
        }
    }

    /// How many built-in-UV attempts are left.
    pub fn uv_retries(protocol: Protocol) -> Self {
        Self {
            protocol,
            subcommand: ClientPinSubcommand::GetUvRetries,
            key_agreement: None,
            pin_hash_enc: None,
            permissions: None,
            rp_id: None,
        }
    }

    /// The token request. `permissions` selects the CTAP 2.1 subcommand; a
    /// `None` falls back to CTAP 2.0's unscoped `getPinToken`, which is all an
    /// older authenticator understands.
    pub fn pin_token(
        protocol: Protocol,
        platform_key: P256PublicKey,
        pin_hash_enc: Vec<u8>,
        permissions: Option<Permissions>,
        rp_id: Option<String>,
    ) -> Self {
        Self {
            protocol,
            subcommand: match permissions {
                Some(_) => ClientPinSubcommand::GetPinUvAuthTokenUsingPinWithPermissions,
                None => ClientPinSubcommand::GetPinToken,
            },
            key_agreement: Some(platform_key),
            pin_hash_enc: Some(pin_hash_enc),
            permissions,
            // rpId is only defined for the permissions subcommand; carrying it
            // into 2.0's getPinToken would be a key that command cannot read.
            rp_id: permissions.and(rp_id),
        }
    }

    pub fn encode(&self) -> Result<Vec<u8>, CoreError> {
        let mut entries = vec![
            (
                0x01,
                Value::Integer(i64::from(self.protocol.number()).into()),
            ),
            (
                0x02,
                Value::Integer(i64::from(self.subcommand as u8).into()),
            ),
        ];
        if let Some(key) = &self.key_agreement {
            entries.push((0x03, cose_key_value(key)?));
        }
        if let Some(pin_hash_enc) = &self.pin_hash_enc {
            entries.push((0x06, Value::Bytes(pin_hash_enc.clone())));
        }
        if let Some(permissions) = self.permissions {
            entries.push((0x09, Value::Integer(i64::from(permissions.bits()).into())));
        }
        if let Some(rp_id) = &self.rp_id {
            entries.push((0x0a, Value::Text(rp_id.clone())));
        }
        encode_value(Command::ClientPin, Some(map(entries)))
    }
}

/// A COSE_Key for the platform's ephemeral ECDH key, as CBOR.
///
/// Built with `coset`, not by hand. The wallet has exactly one COSE encoder and
/// one COSE decoder, and this is the encoder; a second one written here could
/// disagree with the one that reads attested credential data, and the two would
/// only ever be compared by an authenticator refusing a token.
///
/// `alg` is ECDH-ES+HKDF-256 (-25) because CTAP 2.1 §6.5.6 says so, not because
/// anything derives keys with it: PIN/UV protocol Two runs its own HKDF over
/// the shared X, and protocol One a bare SHA-256. Authenticators do check.
fn cose_key_value(key: &P256PublicKey) -> Result<Value, CoreError> {
    let cose = coset::CoseKeyBuilder::new_ec2_pub_key(
        coset::iana::EllipticCurve::P_256,
        key.x.clone(),
        key.y.clone(),
    )
    .algorithm(coset::iana::Algorithm::ECDH_ES_HKDF_256)
    .build();
    let bytes = cose
        .to_vec()
        .map_err(|error| CoreError::InvalidCoseKey(format!("COSE encode: {error}")))?;
    ciborium::de::from_reader(bytes.as_slice())
        .map_err(|error| CoreError::InvalidCbor(format!("COSE re-read: {error}")))
}

/// `authenticatorGetInfo` takes no arguments — the command byte is the request.
pub fn get_info_request() -> Result<Vec<u8>, CoreError> {
    encode_value(Command::GetInfo, None)
}

/// `authenticatorSelection`: "is this the key the person touched?", used to tell
/// two plugged-in authenticators apart. No arguments.
pub fn selection_request() -> Result<Vec<u8>, CoreError> {
    encode_value(Command::Selection, None)
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

/// A response's verdict and its CBOR body, separated.
pub fn split_response(payload: &[u8]) -> Result<(Status, &[u8]), CoreError> {
    let (status, body) = payload
        .split_first()
        .ok_or_else(|| CoreError::InvalidCbor("empty CTAP response".to_string()))?;
    Ok((Status::from_byte(*status), body))
}

fn parse_map(body: &[u8]) -> Result<Vec<(Value, Value)>, CoreError> {
    if body.is_empty() {
        return Ok(Vec::new());
    }
    let value: Value = ciborium::de::from_reader(body)
        .map_err(|error| CoreError::InvalidCbor(error.to_string()))?;
    match value {
        Value::Map(entries) => Ok(entries),
        _ => Err(CoreError::InvalidCbor(
            "CTAP response is not a map".to_string(),
        )),
    }
}

fn take(entries: &[(Value, Value)], key: i64) -> Option<&Value> {
    entries.iter().find_map(|(k, v)| match k {
        Value::Integer(i) if i128::from(*i) == i128::from(key) => Some(v),
        _ => None,
    })
}

fn bytes_at(entries: &[(Value, Value)], key: i64, what: &str) -> Result<Vec<u8>, CoreError> {
    match take(entries, key) {
        Some(Value::Bytes(bytes)) => Ok(bytes.clone()),
        _ => Err(CoreError::InvalidCbor(format!(
            "CTAP response is missing {what}"
        ))),
    }
}

/// What `authenticatorMakeCredential` answered.
///
/// `PartialEq` but not `Eq`: `att_stmt` is an untouched `ciborium::Value`,
/// which can hold a float. Parsing it into something narrower to win a trait
/// would mean deciding what an attestation statement is allowed to contain,
/// and that is the authenticator's business, not ours.
#[derive(Clone, Debug, PartialEq)]
pub struct MakeCredentialResponse {
    pub fmt: String,
    pub auth_data: Vec<u8>,
    /// The attestation statement, still CBOR. Kept whole rather than parsed:
    /// its shape depends on `fmt`, and the wallet only ever re-encodes it.
    pub att_stmt: Value,
}

pub fn parse_make_credential(body: &[u8]) -> Result<MakeCredentialResponse, CoreError> {
    let entries = parse_map(body)?;
    let fmt = match take(&entries, 0x01) {
        Some(Value::Text(text)) => text.clone(),
        _ => {
            return Err(CoreError::InvalidCbor(
                "makeCredential response is missing fmt".to_string(),
            ))
        }
    };
    let auth_data = bytes_at(&entries, 0x02, "authData")?;
    let att_stmt = take(&entries, 0x03)
        .cloned()
        .unwrap_or_else(|| Value::Map(Vec::new()));
    Ok(MakeCredentialResponse {
        fmt,
        auth_data,
        att_stmt,
    })
}

/// The CBOR blob a WebAuthn `create()` would have returned.
///
/// This is the join: everything downstream — public-key extraction, the
/// versioned attestation, the backup-state flag the second-key gate reads —
/// already parses this shape, so a CTAP-minted key and a browser-minted key
/// become indistinguishable the moment they leave this function.
pub fn attestation_object(response: &MakeCredentialResponse) -> Result<Vec<u8>, CoreError> {
    // Key order is the WebAuthn convention, and it is what the existing
    // parsers walk: they match on names, not positions, but a blob that reads
    // the same in both paths is one less thing that can differ.
    let value = Value::Map(vec![
        (
            Value::Text("fmt".to_string()),
            Value::Text(response.fmt.clone()),
        ),
        (
            Value::Text("attStmt".to_string()),
            response.att_stmt.clone(),
        ),
        (
            Value::Text("authData".to_string()),
            Value::Bytes(response.auth_data.clone()),
        ),
    ]);
    let mut out = Vec::new();
    ciborium::ser::into_writer(&value, &mut out)
        .map_err(|error| CoreError::InvalidCbor(error.to_string()))?;
    Ok(out)
}

/// What `authenticatorGetAssertion` answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GetAssertionResponse {
    /// The credential the authenticator chose. Absent when the request pinned
    /// exactly one, which is why it is optional here and not in the caller.
    pub credential_id: Option<Vec<u8>>,
    pub auth_data: Vec<u8>,
    /// DER, exactly as the authenticator produced it. Normalisation (including
    /// low-S) belongs to `webauthn.rs`, which already does it for the browser
    /// path — doing it twice, differently, is how two paths disagree.
    pub signature_der: Vec<u8>,
    /// The user handle, when the authenticator volunteered one. This is where a
    /// wallet's name survives a device wipe.
    pub user_id: Option<Vec<u8>>,
    /// How many discoverable credentials the authenticator holds for this
    /// relying party (`numberOfCredentials`, key 0x05).
    ///
    /// Present ONLY on the first response to a request with an empty allow
    /// list, and absent when there is exactly one. A client that ignores it
    /// silently signs in as whichever credential the key happened to return
    /// first — which, on a key holding two of a person's wallets, means one of
    /// them becomes unreachable. The rest are fetched with
    /// [`Command::GetNextAssertion`].
    pub number_of_credentials: Option<u32>,
}

pub fn parse_get_assertion(body: &[u8]) -> Result<GetAssertionResponse, CoreError> {
    let entries = parse_map(body)?;
    let credential_id = match take(&entries, 0x01) {
        Some(Value::Map(fields)) => fields.iter().find_map(|(k, v)| match (k, v) {
            (Value::Text(name), Value::Bytes(bytes)) if name == "id" => Some(bytes.clone()),
            _ => None,
        }),
        _ => None,
    };
    let user_id = match take(&entries, 0x04) {
        Some(Value::Map(fields)) => fields.iter().find_map(|(k, v)| match (k, v) {
            (Value::Text(name), Value::Bytes(bytes)) if name == "id" => Some(bytes.clone()),
            _ => None,
        }),
        _ => None,
    };
    let number_of_credentials = match take(&entries, 0x05) {
        Some(Value::Integer(count)) => u32::try_from(i128::from(*count)).ok(),
        _ => None,
    };
    Ok(GetAssertionResponse {
        credential_id,
        auth_data: bytes_at(&entries, 0x02, "authData")?,
        signature_der: bytes_at(&entries, 0x03, "signature")?,
        user_id,
        number_of_credentials,
    })
}

/// `authenticatorGetNextAssertion` — the next credential of a set the first
/// `getAssertion` reported. No arguments: the authenticator is walking a list
/// it already built, over the client-data hash it already has.
///
/// Valid only immediately after a `getAssertion` that reported
/// `numberOfCredentials` above one, and only until the authenticator's own
/// timer runs out. It costs NO second touch: user presence was collected once,
/// for the set.
pub fn get_next_assertion_request() -> Result<Vec<u8>, CoreError> {
    encode_value(Command::GetNextAssertion, None)
}

/// What `authenticatorClientPIN` answered.
///
/// One struct for four subcommands, because the authenticator answers all four
/// with the same map and simply leaves out what does not apply. A caller reads
/// the field its request was about; a `None` there means the authenticator did
/// not answer the question that was asked, which is a protocol fault worth
/// surfacing rather than defaulting through.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ClientPinResponse {
    /// The AUTHENTICATOR's ephemeral public key for this session.
    pub key_agreement: Option<P256PublicKey>,
    /// The `pinUvAuthToken`, still encrypted under the shared secret.
    pub pin_uv_auth_token: Option<Vec<u8>>,
    /// PIN attempts left before the key locks itself. Zero means locked until
    /// the authenticator is reset — and a reset destroys every credential on
    /// it, including this wallet's founding key.
    pub pin_retries: Option<u32>,
    /// The key must be unplugged and reinserted before another PIN attempt.
    pub power_cycle_state: bool,
    pub uv_retries: Option<u32>,
}

pub fn parse_client_pin(body: &[u8]) -> Result<ClientPinResponse, CoreError> {
    let entries = parse_map(body)?;
    let mut response = ClientPinResponse::default();

    if let Some(value) = take(&entries, 0x01) {
        // The key arrives as a nested CBOR map. It is re-serialized and handed
        // to the one COSE decoder this wallet has — the same one that reads a
        // credential's public key out of attested credential data — rather than
        // being walked here. Its on-curve check is the load-bearing part: an
        // off-curve "public key" is how a tampered response gets a client to
        // compute a shared secret with structure the attacker chose.
        let mut bytes = Vec::new();
        ciborium::ser::into_writer(value, &mut bytes)
            .map_err(|error| CoreError::InvalidCbor(error.to_string()))?;
        let key = coset::CoseKey::from_slice(&bytes)
            .map_err(|error| CoreError::InvalidCoseKey(format!("keyAgreement: {error}")))?;
        response.key_agreement = Some(p256_from_cose_key(&key)?);
    }
    if let Some(Value::Bytes(bytes)) = take(&entries, 0x02) {
        response.pin_uv_auth_token = Some(bytes.clone());
    }
    if let Some(Value::Integer(count)) = take(&entries, 0x03) {
        response.pin_retries = u32::try_from(i128::from(*count)).ok();
    }
    if let Some(Value::Bool(flag)) = take(&entries, 0x04) {
        response.power_cycle_state = *flag;
    }
    if let Some(Value::Integer(count)) = take(&entries, 0x05) {
        response.uv_retries = u32::try_from(i128::from(*count)).ok();
    }

    Ok(response)
}

/// The parts of `authenticatorGetInfo` a wallet acts on.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthenticatorInfo {
    pub versions: Vec<String>,
    /// 16 bytes identifying the authenticator MODEL, not the key.
    pub aaguid: Vec<u8>,
    /// The authenticator can store discoverable credentials.
    pub resident_key: bool,
    /// A PIN or biometric is already configured.
    pub client_pin_set: bool,
    /// The authenticator can verify the user itself (biometrics).
    pub user_verification: bool,
    /// CTAP 2.1's `getPinUvAuthTokenUsingPinWithPermissions` is available, so a
    /// token can be scoped to `mc | ga` on one relying party instead of being
    /// the unscoped 2.0 token that authenticates anything.
    pub pin_uv_auth_token: bool,
    pub pin_protocols: Vec<u8>,
}

pub fn parse_get_info(body: &[u8]) -> Result<AuthenticatorInfo, CoreError> {
    let entries = parse_map(body)?;
    let mut info = AuthenticatorInfo::default();

    if let Some(Value::Array(items)) = take(&entries, 0x01) {
        info.versions = items
            .iter()
            .filter_map(|item| match item {
                Value::Text(text) => Some(text.clone()),
                _ => None,
            })
            .collect();
    }
    if let Some(Value::Bytes(bytes)) = take(&entries, 0x03) {
        info.aaguid = bytes.clone();
    }
    if let Some(Value::Map(options)) = take(&entries, 0x04) {
        for (key, value) in options {
            let (Value::Text(name), Value::Bool(flag)) = (key, value) else {
                continue;
            };
            match name.as_str() {
                "rk" => info.resident_key = *flag,
                // `clientPin` absent means "no PIN support"; present-and-false
                // means "supported, not set". The wallet only cares whether it
                // must ask for one.
                "clientPin" => info.client_pin_set = *flag,
                "uv" => info.user_verification = *flag,
                "pinUvAuthToken" => info.pin_uv_auth_token = *flag,
                _ => {}
            }
        }
    }
    if let Some(Value::Array(items)) = take(&entries, 0x06) {
        info.pin_protocols = items
            .iter()
            .filter_map(|item| match item {
                Value::Integer(i) => u8::try_from(i128::from(*i)).ok(),
                _ => None,
            })
            .collect();
    }

    Ok(info)
}
