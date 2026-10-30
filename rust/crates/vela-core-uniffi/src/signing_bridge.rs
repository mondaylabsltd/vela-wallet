//! Where a signature goes and how its answer is taken, for Android and iOS —
//! the web's round 5/6 surface (`signingRoute`, `signingVenueBlock`,
//! `multisigApprovalRoute`, `trustedSignerSignatureAccept`,
//! `trustedSignerApprovalRequest` / `…Accept`) and the dApp request's answer
//! words (`signErrorWords`, `signNotConfirmedDetail`, `signAnswered`,
//! `signRequestTtlMs`), over UniFFI.
//!
//! Each function answers what its wasm twin answers, in the same JSON — the
//! shared vectors (`vela-core/tests/vectors/native-parity/exports.json`) are
//! replayed by `rust/scripts/verify-web.mjs` through the shipped wasm and by
//! this crate's tests, the Kotlin harness and the Swift harness through these.
//! Where an answer is a judgement the shell acts on (an accepted signature,
//! an approval) it is typed, as `trusted_signer_bridge`'s are.

use crate::trusted_signer_bridge::{TrustedSignerOutcome, TrustedSignerRefusal};
use crate::{CoreError, WalletKeyRecord, WebAuthnAssertion};

use vela_core::trusted_signer::{self, approval};

// ---------------------------------------------------------------------------
// Where a signature goes (spec 102, owner 2026-10-30: the page everywhere)
// ---------------------------------------------------------------------------

/// R7 — where a request of `method` by the stored account goes
/// (`signing_venue::signing_route`): `{"type":"in_vela"}`,
/// `{"type":"page","url":…}` or `{"type":"blocked","block":…}`. `method` is
/// the JSON-RPC method (`""` for the wallet's own send,
/// `vela_multisigExecute` for a multisig executor's operation,
/// `vela_multisigApprove` for a member's approval). `None` for a record this
/// build cannot read or a method no venue signs (`eth_sign`) — refuse it.
/// `account_json` is the stored account record, as for `signing_plan`.
#[uniffi::export]
#[must_use]
pub fn signing_route(account_json: String, method: String) -> Option<String> {
    vela_core::app::signing_route_json(&account_json, &method)
}

/// R1: why `venue_json` (a `SigningVenue`) cannot reach the keys of an
/// account on `domain`, as a `VenueBlock` JSON — or `None` when it can. A
/// venue that does not read reaches nothing: the JSON string `"invalid"`.
#[uniffi::export]
#[must_use]
pub fn signing_venue_block(domain: String, venue_json: String) -> Option<String> {
    let Ok(venue) = serde_json::from_str(&venue_json) else {
        return Some("\"invalid\"".to_owned());
    };
    vela_core::signing_venue::reachability(&domain, &venue)
        .err()
        .and_then(|block| serde_json::to_string(&block).ok())
}

/// Where a multisig member's approval goes — [`signing_route`] for
/// `vela_multisigApprove` (`trusted_signer::approval::approval_route`), as
/// the same JSON. `None` for a record this build cannot read.
/// `account_json` is the member's record as stored.
#[uniffi::export]
#[must_use]
pub fn multisig_approval_route(account_json: String) -> Option<String> {
    approval::approval_route_json(&account_json)
}

// ---------------------------------------------------------------------------
// The page's answer to a signature request, held to the request
// ---------------------------------------------------------------------------

fn wallet_keys(keys: &[WalletKeyRecord]) -> Vec<vela_core::user_op::WalletKey> {
    keys.iter()
        .map(|key| vela_core::user_op::WalletKey {
            credential_id: key.credential_id.clone(),
            public_key_hex: key.public_key_hex.clone(),
        })
        .collect()
}

fn malformed(detail: impl std::fmt::Display) -> TrustedSignerRefusal {
    TrustedSignerRefusal::Malformed {
        detail: detail.to_string(),
    }
}

/// The page's answer (`result_json`) to the request it was handed
/// (`request_json`, the `{intent, context}` of `trusted_signer_request`),
/// accepted only as `trusted_signer::accept_signature` accepts it: the bytes
/// the CORE computes from the request must be `digest` — the bytes the shell
/// is about to sign with the answer — and the answer a user-verified
/// assertion over them by one of `keys` (the account's founding keys). A
/// request that describes one thing while the shell signs another is
/// `WrongChallenge` before the answer is read.
///
/// Stricter than `trusted_signer_verify`, which takes the shell's
/// digest on trust; the web's `trustedSignerSignatureAccept`.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn trusted_signer_signature_accept(
    request_json: String,
    result_json: String,
    keys: Vec<WalletKeyRecord>,
    digest: Vec<u8>,
) -> TrustedSignerOutcome {
    let refused = |refusal| TrustedSignerOutcome::Refused { refusal };
    let request: serde_json::Value = match serde_json::from_str(&request_json) {
        Ok(value) => value,
        Err(error) => return refused(malformed(format!("request_json: {error}"))),
    };
    let result: serde_json::Value = match serde_json::from_str(&result_json) {
        Ok(value) => value,
        Err(error) => return refused(malformed(format!("result_json: {error}"))),
    };
    match trusted_signer::accept_signature(&request, &result, &wallet_keys(&keys), &digest) {
        Ok(verified) => TrustedSignerOutcome::Accepted {
            credential_id_hex: verified.credential_id_hex,
            assertion: WebAuthnAssertion {
                authenticator_data: verified.authenticator_data,
                client_data_json: verified.client_data_json,
                signature_der: verified.signature_der,
            },
        },
        Err(error) => refused(error.into()),
    }
}

// ---------------------------------------------------------------------------
// A multisig member's approval on the page (2026-10-29)
// ---------------------------------------------------------------------------

/// What the shell holds when a member would approve a SafeTx on the page —
/// the web's `trustedSignerApprovalRequest` arguments.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TrustedSignerApprovalInput {
    pub chain_id: u64,
    pub chain_name: Option<String>,
    pub native_symbol: Option<String>,
    /// The multisig (the Safe whose transaction this is).
    pub multisig: String,
    /// The multisig's name as this device knows it — drawn by the page as the
    /// app's word.
    pub multisig_name: Option<String>,
    /// The raw SafeTx as JSON (`{to, value, data, operation, safeTxGas,
    /// baseGas, gasPrice, gasToken, refundReceiver, nonce}`), as the core's
    /// `multisig::SafeTx` reads it.
    pub tx_json: String,
    /// The member approving: an account of this device that owns the multisig.
    pub member: String,
    pub member_name: Option<String>,
    /// The member's credential ids, hex — offered when there is no route.
    pub credential_ids_hex: Vec<String>,
    /// The member's key route as JSON (`signing_plan`'s `key`), as it came.
    pub key_route_json: Option<String>,
}

/// The page's `{intent, context}` for a member's approval of a SafeTx
/// (`trusted_signer::approval::request`), as JSON: the method, the raw
/// SafeTx, the core's safeTxHash, the member, its keys and route, the
/// multisig's name. An error for a SafeTx, route or address that does not read.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn trusted_signer_approval_request(
    input: TrustedSignerApprovalInput,
) -> Result<String, CoreError> {
    let tx: vela_core::multisig::SafeTx = serde_json::from_str(&input.tx_json)
        .map_err(|e| CoreError::Internal(format!("tx_json: {e}")))?;
    let key_route: Option<vela_core::signing_venue::KeyRoute> = input
        .key_route_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| CoreError::Internal(format!("key_route_json: {e}")))?;
    let request = approval::request(&approval::ApprovalInput {
        chain_id: input.chain_id,
        chain_name: input.chain_name.as_deref(),
        native_symbol: input.native_symbol.as_deref(),
        multisig: &input.multisig,
        multisig_name: input.multisig_name.as_deref(),
        tx: &tx,
        member: &input.member,
        member_name: input.member_name.as_deref(),
        credential_ids_hex: &input.credential_ids_hex,
        key_route: key_route.as_ref(),
    })?;
    Ok(request.to_string())
}

/// The page's answer to a member's approval, judged.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum TrustedSignerApprovalOutcome {
    /// The member's approval (`PasskeyApproval` JSON: `{owner, signature, …}`),
    /// checked the chain's way — what the genesis service and the chain take.
    Accepted {
        approval_json: String,
    },
    Refused {
        refusal: TrustedSignerRefusal,
    },
}

/// The page's answer (`result_json`) as `member`'s approval of
/// `safe_tx_hash` on `chain_id`, accepted only as
/// `trusted_signer::approval::accept` accepts it: a user-verified assertion
/// over exactly the approval bytes THIS wallet computes, by one of `keys`
/// (the member's founding keys), whose approval names `member` and verifies
/// the way the chain will.
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn trusted_signer_approval_accept(
    result_json: String,
    chain_id: u64,
    member: String,
    safe_tx_hash: String,
    keys: Vec<WalletKeyRecord>,
) -> TrustedSignerApprovalOutcome {
    let refused = |refusal| TrustedSignerApprovalOutcome::Refused { refusal };
    let result: serde_json::Value = match serde_json::from_str(&result_json) {
        Ok(value) => value,
        Err(error) => return refused(malformed(format!("result_json: {error}"))),
    };
    match approval::accept(
        &result,
        chain_id,
        &member,
        &safe_tx_hash,
        &wallet_keys(&keys),
    ) {
        Ok(approval) => match serde_json::to_string(&approval) {
            Ok(approval_json) => TrustedSignerApprovalOutcome::Accepted { approval_json },
            Err(error) => refused(malformed(error)),
        },
        Err(error) => refused(error.into()),
    }
}

// ---------------------------------------------------------------------------
// How a dApp request is answered (spec 082 RA8, RB2; spec 083)
// ---------------------------------------------------------------------------

/// The message a dApp reads when signing ended in the error the core named
/// `kind` (a `SignErrorKind`'s wire name, `"user_rejected"`), with the
/// notice's `detail` — `dapp_rpc::sign_error_words`. `None` for a kind the
/// core does not know. For the developer, not the person: EIP-1193 messages
/// are not UI.
#[uniffi::export]
#[must_use]
pub fn sign_error_words(kind: String, detail: Option<String>) -> Option<String> {
    let kind: vela_core::app::sign_request::SignErrorKind =
        serde_json::from_value(serde_json::Value::String(kind)).ok()?;
    Some(vela_core::app::dapp_rpc::sign_error_words(
        kind,
        detail.as_deref(),
    ))
}

/// The page's "not confirmed yet" for an operation that may still land —
/// `sign_request::not_confirmed_detail` (083).
#[uniffi::export]
#[must_use]
pub fn sign_not_confirmed_detail(user_op_hash: String) -> String {
    vela_core::app::sign_request::not_confirmed_detail(&user_op_hash)
}

/// The hash or signature an answer to a page names (`payload_json`, the
/// core's `SignResponsePayload` JSON) — a batch's id read out of EIP-5792
/// 2.0.0's `{ id }` (spec 097 G) — `SignResponsePayload::answered`. `None`
/// for an error, a `null` answer or JSON that does not read.
#[uniffi::export]
#[must_use]
pub fn sign_answered(payload_json: String) -> Option<String> {
    serde_json::from_str::<vela_core::app::sign_request::SignResponsePayload>(&payload_json)
        .ok()?
        .answered()
        .map(str::to_owned)
}

/// A sign request older than this (ms) is never signed —
/// `sign_request::EXTENSION_REQUEST_TTL_MS` (RB2).
#[uniffi::export]
#[must_use]
pub fn sign_request_ttl_ms() -> f64 {
    vela_core::app::sign_request::EXTENSION_REQUEST_TTL_MS
}
