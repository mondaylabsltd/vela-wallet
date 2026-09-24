//! Spec 075: the Trusted Signer as a passkey route — the ceremonies besides
//! signing (`contracts/clear-signer-channel.md` §1.3–1.4).
//!
//! The create and sign-in machines speak to a shell in passkey operations
//! (`RegisterPasskey`, `AuthenticatePasskey`, `SignProof`, `SignMemberProof`).
//! With `method = trusted_signer` the shell hands the operation here, sends the
//! request this builds to the signer page, and hands the page's answer back
//! to [`verify`] — whose result it reports exactly as a platform ceremony's
//! (`PasskeyRegistered`, `PasskeyAuthenticated`, `ProofSigned`,
//! `MemberProofSigned`).
//!
//! The page signs only challenges it derived itself; this side checks that it
//! did:
//! - a sign-in challenge is `vela-signin-…`;
//! - a proof is `vela-verify-…` / `vela-recover-…`;
//! - none of those is 32 bytes a Safe could take for an operation hash;
//! - a member proof's is exactly the one the wallet fetched from the registry
//!   for the same inputs.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};

use super::ws::origin_of;
use super::TrustedSignerError;
use crate::app::shell::{ProofPurpose, ShellOperation};
use crate::app::{Assertion, Registration};
use crate::types::ClientDataKind;
use crate::webauthn::validate_client_data;

/// The page's `intent.method` for each ceremony.
pub const CREATE: &str = "vela_createPasskey";
pub const SIGN_IN: &str = "vela_signIn";
pub const PROOF: &str = "vela_proof";
pub const MEMBER_PROOF: &str = "vela_memberProof";

/// The four passkey operations the Trusted Signer runs, with only the fields a
/// ceremony needs. It reads the operation's own wire JSON (anything else in
/// it is ignored), so a shell passes the operation it already holds — and the
/// web's wasm does not link a reader for every machine operation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ceremony {
    RegisterPasskey {
        name: String,
        #[serde(default)]
        exclude_credential_ids: Vec<String>,
    },
    AuthenticatePasskey {},
    SignProof {
        credential_id: String,
        purpose: ProofPurpose,
    },
    SignMemberProof {
        credential_id: String,
        public_key_hex: String,
        #[serde(default)]
        attestation_hex: String,
        group_public_key_hex: String,
    },
}

impl Ceremony {
    /// The ceremony a machine operation asks for, or `None` for any other.
    #[must_use]
    pub fn of(operation: &ShellOperation) -> Option<Self> {
        Some(match operation {
            ShellOperation::RegisterPasskey {
                name,
                exclude_credential_ids,
                ..
            } => Self::RegisterPasskey {
                name: name.clone(),
                exclude_credential_ids: exclude_credential_ids.clone(),
            },
            ShellOperation::AuthenticatePasskey { .. } => Self::AuthenticatePasskey {},
            ShellOperation::SignProof {
                credential_id,
                purpose,
                ..
            } => Self::SignProof {
                credential_id: credential_id.clone(),
                purpose: *purpose,
            },
            ShellOperation::SignMemberProof {
                credential_id,
                public_key_hex,
                attestation_hex,
                group_public_key_hex,
                ..
            } => Self::SignMemberProof {
                credential_id: credential_id.clone(),
                public_key_hex: public_key_hex.clone(),
                attestation_hex: attestation_hex.clone(),
                group_public_key_hex: group_public_key_hex.clone(),
            },
            _ => return None,
        })
    }

    /// Read from an operation's wire JSON; `None` for any other operation.
    #[must_use]
    pub fn from_json(operation_json: &str) -> Option<Self> {
        serde_json::from_str(operation_json).ok()
    }
}

fn hex_to_b64url(hex: &str) -> String {
    crate::primitives::from_hex(hex.trim_start_matches("0x"))
        .map(|bytes| URL_SAFE_NO_PAD.encode(bytes))
        .unwrap_or_default()
}

const fn purpose_name(purpose: ProofPurpose) -> &'static str {
    match purpose {
        ProofPurpose::Verify => "verify",
        ProofPurpose::RecoverFirst => "recover_first",
        ProofPurpose::RecoverSecond => "recover_second",
    }
}

/// Which registry deployment a member proof is bound to.
///
/// The member challenge is
/// `keccak256(abi.encode(chainId, contract, rpId, publicKey, binding))`, so
/// these two facts are all the page needs to compute it — and computing it is
/// what it already does, refusing anything that does not match.
///
/// **Why the wallet supplies them rather than the page fetching them.** The
/// page used to read them from the registry's `/api/health`, which the
/// published page cannot do at all: `default-src 'none'` is inside its hashed
/// bytes (spec 076), so a page whose hash a wallet accepts can reach no
/// network. Measured, and then measured again the hard way — creating a wallet
/// failed at its second step with 「注册表没有应答」 (owner, 2026-09-24).
///
/// Handing them over is also stronger than fetching them. The page never signs
/// a challenge it was given; it signs the one it derived from what is on
/// screen. So a requester that lies about either fact gets a different
/// challenge — and the wallet, which fetched the real one, refuses the answer
/// (`verify`'s `expected_member_challenge`). The registry stops being a party
/// this step has to trust.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegistryDeployment {
    pub chain_id: u64,
    /// The `domainRegistry` contract, `0x`-prefixed.
    pub contract: String,
}

/// The page request for a ceremony. `registry` is the registry service the
/// wallet uses, named so the card can show it. `deployment` is required by a
/// member proof and ignored by every other ceremony.
#[must_use]
pub fn request(
    ceremony: &Ceremony,
    id: &str,
    wallet_name: &str,
    registry: &str,
    deployment: Option<&RegistryDeployment>,
) -> Value {
    let (method, params) = match ceremony {
        Ceremony::RegisterPasskey {
            name,
            exclude_credential_ids,
        } => (
            CREATE,
            json!({
                "name": name,
                "excludeCredentialIds": exclude_credential_ids
                    .iter()
                    .map(|id| hex_to_b64url(id))
                    .filter(|id| !id.is_empty())
                    .collect::<Vec<_>>(),
            }),
        ),
        Ceremony::AuthenticatePasskey {} => (SIGN_IN, json!({})),
        Ceremony::SignProof {
            credential_id,
            purpose,
        } => (
            PROOF,
            json!({
                "credentialId": hex_to_b64url(credential_id),
                "purpose": purpose_name(*purpose),
            }),
        ),
        Ceremony::SignMemberProof {
            credential_id,
            public_key_hex,
            attestation_hex,
            group_public_key_hex,
        } => (
            MEMBER_PROOF,
            json!({
                "credentialId": hex_to_b64url(credential_id),
                "publicKey": public_key_hex,
                "attestation": attestation_hex,
                "groupPublicKey": group_public_key_hex,
                "registry": registry,
                // Absent when the wallet could not learn the deployment. The
                // page then has nothing to compute the challenge from and says
                // so, rather than signing something it could not check.
                "chainId": deployment.map(|d| d.chain_id),
                "registryContract": deployment.map(|d| d.contract.clone()),
            }),
        ),
    };
    json!({
        "id": id,
        "intent": { "method": method, "params": [params], "origin": "" },
        "context": { "walletName": wallet_name },
    })
}

/// What a verified answer is, in the machines' own types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Registration(Registration),
    Assertion(Assertion),
}

fn field<'a>(object: &'a Value, name: &str) -> Result<&'a str, TrustedSignerError> {
    object
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| TrustedSignerError::Malformed(format!("missing {name}")))
}

fn hex_bytes(object: &Value, name: &str) -> Result<Vec<u8>, TrustedSignerError> {
    crate::primitives::from_hex(field(object, name)?)
        .map_err(|_| TrustedSignerError::Malformed(format!("{name} is not hex")))
}

fn credential_hex(object: &Value) -> Result<String, TrustedSignerError> {
    URL_SAFE_NO_PAD
        .decode(field(object, "credentialId")?.trim_end_matches('='))
        .map(|bytes| crate::primitives::to_hex(&bytes, false).to_ascii_lowercase())
        .map_err(|_| TrustedSignerError::Malformed("credentialId is not base64url".into()))
}

fn client_data(json: &[u8]) -> Result<Value, TrustedSignerError> {
    serde_json::from_slice(json)
        .map_err(|_| TrustedSignerError::Malformed("clientDataJSON is not JSON".into()))
}

/// The origin the authenticator saw — signed, unlike any `origin` field the
/// page writes next to it — must be the page the wallet opened.
fn same_origin(client: &Value, signer_origin: &str) -> Result<(), TrustedSignerError> {
    let seen = client
        .get("origin")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if origin_of(seen).is_empty() || origin_of(seen) != origin_of(signer_origin) {
        return Err(TrustedSignerError::Malformed(format!(
            "the answer came from {seen}, not from the Trusted Signer page"
        )));
    }
    Ok(())
}

fn challenge_bytes(client: &Value) -> Result<Vec<u8>, TrustedSignerError> {
    let text = client
        .get("challenge")
        .and_then(Value::as_str)
        .unwrap_or_default();
    URL_SAFE_NO_PAD
        .decode(text.trim_end_matches('='))
        .map_err(|_| TrustedSignerError::WrongChallenge)
}

/// A challenge the page derived for `prefix`: that prefix, and never the 32
/// bytes an operation hash is.
fn derived(challenge: &[u8], prefix: &str) -> bool {
    challenge.len() != 32
        && challenge.starts_with(prefix.as_bytes())
        && challenge.len() > prefix.len()
}

fn verify_registration(answer: &Value, signer_origin: &str) -> Result<Answer, TrustedSignerError> {
    let registration = answer
        .get("registration")
        .ok_or_else(|| TrustedSignerError::Malformed("missing registration".into()))?;
    let client_data_json = hex_bytes(registration, "clientDataJSON")?;
    validate_client_data(ClientDataKind::Create, &client_data_json, &[])
        .map_err(|error| TrustedSignerError::Malformed(error.to_string()))?;
    same_origin(&client_data(&client_data_json)?, signer_origin)?;
    let attestation_object_hex = field(registration, "attestationObject")?.to_ascii_lowercase();
    crate::app::public_key_hex_from_attestation(&attestation_object_hex)
        .map_err(|error| TrustedSignerError::Malformed(error.to_string()))?;
    Ok(Answer::Registration(Registration {
        credential_id: credential_hex(registration)?,
        attestation_object_hex,
        client_data_json_hex: crate::primitives::to_hex(&client_data_json, false),
        authenticator_attachment: field(registration, "authenticatorAttachment")
            .unwrap_or_default()
            .to_owned(),
        transports: field(registration, "transports")
            .unwrap_or_default()
            .to_owned(),
        signer_origin: Some(origin_of(signer_origin)).filter(|o| !o.is_empty()),
    }))
}

/// Accept the page's answer to `ceremony` only if it is the ceremony that was
/// asked for, from `signer_origin`, over a challenge the page was allowed to
/// sign — and for a member proof, over `expected_member_challenge` (the one
/// the wallet fetched from the registry for the same inputs).
///
/// # Errors
/// Every [`TrustedSignerError`] but `WrongToken`, which is the channel's to
/// report. A page that answered `error` is `Declined` (the person) or
/// `Refused` (its rules).
pub fn verify(
    ceremony: &Ceremony,
    answer: &Value,
    signer_origin: &str,
    expected_member_challenge: Option<&[u8]>,
) -> Result<Answer, TrustedSignerError> {
    if answer.get("t").and_then(Value::as_str) == Some("error") {
        let code = answer
            .get("code")
            .and_then(Value::as_str)
            .unwrap_or_default();
        return Err(super::refusal(code));
    }
    let named = match ceremony {
        Ceremony::RegisterPasskey { .. } => return verify_registration(answer, signer_origin),
        Ceremony::AuthenticatePasskey {} => None,
        Ceremony::SignProof { credential_id, .. }
        | Ceremony::SignMemberProof { credential_id, .. } => Some(credential_id),
    };

    let assertion = answer
        .get("assertion")
        .ok_or_else(|| TrustedSignerError::Malformed("missing assertion".into()))?;
    let authenticator_data = hex_bytes(assertion, "authenticatorData")?;
    let client_data_json = hex_bytes(assertion, "clientDataJSON")?;
    validate_client_data(ClientDataKind::Get, &client_data_json, &authenticator_data).map_err(
        |error| {
            if error.to_string().contains("User Verification") {
                TrustedSignerError::NotVerified
            } else {
                TrustedSignerError::Malformed(error.to_string())
            }
        },
    )?;
    let client = client_data(&client_data_json)?;
    same_origin(&client, signer_origin)?;
    let challenge = challenge_bytes(&client)?;
    let allowed = match ceremony {
        Ceremony::AuthenticatePasskey {} => derived(&challenge, "vela-signin-"),
        Ceremony::SignProof {
            purpose: ProofPurpose::Verify,
            ..
        } => derived(&challenge, "vela-verify-"),
        Ceremony::SignProof { .. } => derived(&challenge, "vela-recover-"),
        Ceremony::SignMemberProof { .. } => expected_member_challenge == Some(challenge.as_slice()),
        Ceremony::RegisterPasskey { .. } => false,
    };
    if !allowed {
        return Err(TrustedSignerError::WrongChallenge);
    }
    let credential_id = credential_hex(assertion)?;
    if let Some(named) = named {
        if !named
            .trim_start_matches("0x")
            .eq_ignore_ascii_case(&credential_id)
        {
            return Err(TrustedSignerError::ForeignKey);
        }
    }
    let user_id_hex = assertion
        .get("userHandle")
        .and_then(Value::as_str)
        .filter(|h| !h.is_empty())
        .map(str::to_ascii_lowercase);
    Ok(Answer::Assertion(Assertion {
        credential_id,
        signature_der_hex: field(assertion, "signatureDer")?.to_ascii_lowercase(),
        authenticator_data_hex: crate::primitives::to_hex(&authenticator_data, false),
        client_data_json_hex: crate::primitives::to_hex(&client_data_json, false),
        user_id_hex,
        authenticator_attachment: field(assertion, "authenticatorAttachment")
            .unwrap_or_default()
            .to_owned(),
        signer_origin: Some(origin_of(signer_origin)).filter(|o| !o.is_empty()),
    }))
}
