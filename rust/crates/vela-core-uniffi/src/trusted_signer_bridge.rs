//! The trusted signing page over UniFFI (spec 071): the request a clear-signing
//! page receives, the verification of what it answers, and the loopback
//! WebSocket between the phone app and the page — so no shell builds, parses,
//! frames or trusts any of it on its own.
//!
//! Spec 102: a page is opened only through a [`SignerPageAdmission`] — the
//! check of the very version that is opened (R6) — and the request names the
//! key to use and where it lives (R5).

use crate::{
    inner_calls, op_of, CoreError, UserOpCall, UserOpDraft, WalletKeyRecord, WebAuthnAssertion,
};
use std::sync::{Arc, Mutex};

use vela_core::trusted_signer::{self, RequestInput, TrustedSignerError};

/// What the shell holds when it would sign.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TrustedSignerInput {
    /// The request's own method (`eth_sendTransaction`, `personal_sign`, …);
    /// EMPTY for the wallet's own send, whose intent is built from `calls`.
    pub method: String,
    /// Its JSON-RPC params, verbatim (ignored when `method` is empty).
    pub params_json: String,
    /// The requesting site's origin; empty for the wallet's own send.
    pub origin: String,
    /// Spec 079: the origin was observed by the wallet's own browser engine.
    #[uniffi(default = false)]
    pub origin_seen_by_browser: bool,
    pub chain_id: u32,
    pub chain_name: Option<String>,
    pub native_symbol: Option<String>,
    pub account: String,
    pub account_name: Option<String>,
    /// The account's credential ids, hex — offered only when there is no
    /// `key_route_json`.
    pub credential_ids_hex: Vec<String>,
    /// Spec 102 R5: the account's key route as JSON — the `key` of
    /// `signing_plan`'s answer, passed through as it came. With it the page
    /// offers that one key with its transports and hints.
    #[uniffi(default = None)]
    pub key_route_json: Option<String>,
    /// For a transaction: the calls the operation carries BEFORE its fee leg
    /// (the wallet appends the fee last, so its index is `calls.len()`). With
    /// an empty `method` — the wallet's own send, which no site asked for —
    /// they are also the intent (`wallet_sendCalls`). Empty for a message.
    pub calls: Vec<UserOpCall>,
}

/// The page's `{intent, context}` as JSON. `draft` is the ASSEMBLED
/// operation for a transaction (the digest covers it), `None` for a message.
#[uniffi::export]
pub fn trusted_signer_request(
    input: TrustedSignerInput,
    draft: Option<UserOpDraft>,
) -> Result<String, CoreError> {
    let op = draft.as_ref().map(op_of).transpose()?;
    let calls = inner_calls(&input.calls)?;
    // Only a site's own request carries params; the wallet's own send is
    // built by the core from its calls.
    let params = if input.method.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_str(&input.params_json)
            .map_err(|e| CoreError::Internal(format!("params_json: {e}")))?
    };
    let key_route: Option<vela_core::signing_venue::KeyRoute> = input
        .key_route_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| CoreError::Internal(format!("key_route_json: {e}")))?;
    let built = trusted_signer::request(&RequestInput {
        method: &input.method,
        params,
        origin: &input.origin,
        chain_id: u64::from(input.chain_id),
        chain_name: input.chain_name.as_deref(),
        native_symbol: input.native_symbol.as_deref(),
        account: &input.account,
        account_name: input.account_name.as_deref(),
        credential_ids_hex: &input.credential_ids_hex,
        key_route: key_route.as_ref(),
        user_op: op.as_ref(),
        calls: &calls,
        origin_seen_by_browser: input.origin_seen_by_browser,
    });
    Ok(built.to_string())
}

/// Why a Trusted Signer answer was not accepted — or what the page said.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum TrustedSignerRefusal {
    /// The person closed the page or declined.
    Declined,
    /// The page's own rules refused the request (`code` is the page's).
    PageRefused { code: String },
    /// Signed something other than this request's digest — never submitted.
    WrongChallenge,
    /// The key that answered is not one of this wallet's.
    ForeignKey,
    /// The signature does not verify under the wallet's key.
    BadSignature,
    /// The passkey did not verify the person.
    NotVerified,
    /// An answer for another request.
    WrongToken,
    /// Not the protocol's shape.
    Malformed { detail: String },
}

impl From<TrustedSignerError> for TrustedSignerRefusal {
    fn from(error: TrustedSignerError) -> Self {
        match error {
            TrustedSignerError::Declined => Self::Declined,
            TrustedSignerError::Refused(code) => Self::PageRefused { code },
            TrustedSignerError::WrongChallenge => Self::WrongChallenge,
            TrustedSignerError::ForeignKey => Self::ForeignKey,
            TrustedSignerError::BadSignature => Self::BadSignature,
            TrustedSignerError::NotVerified => Self::NotVerified,
            TrustedSignerError::WrongToken => Self::WrongToken,
            TrustedSignerError::Malformed(detail) => Self::Malformed { detail },
        }
    }
}

/// The page's answer, judged.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum TrustedSignerOutcome {
    /// Over this digest, by this wallet's key, user-verified: sign with it
    /// (`user_op_sign` / `eip1271_signature`) exactly as with any passkey.
    Accepted {
        credential_id_hex: String,
        assertion: WebAuthnAssertion,
    },
    Refused {
        refusal: TrustedSignerRefusal,
    },
}

/// Accept `result_json` only if it signs `digest` with one of `keys` — the
/// URL and postMessage channels' answer.
#[uniffi::export]
pub fn trusted_signer_verify(
    result_json: String,
    digest: Vec<u8>,
    keys: Vec<WalletKeyRecord>,
) -> TrustedSignerOutcome {
    match serde_json::from_str(&result_json) {
        Ok(result) => judge(&result, &digest, &keys),
        Err(error) => TrustedSignerOutcome::Refused {
            refusal: TrustedSignerRefusal::Malformed {
                detail: error.to_string(),
            },
        },
    }
}

fn judge(
    result: &serde_json::Value,
    digest: &[u8],
    keys: &[WalletKeyRecord],
) -> TrustedSignerOutcome {
    let keys: Vec<vela_core::user_op::WalletKey> = keys
        .iter()
        .map(|key| vela_core::user_op::WalletKey {
            credential_id: key.credential_id.clone(),
            public_key_hex: key.public_key_hex.clone(),
        })
        .collect();
    match trusted_signer::verify(result, digest, &keys) {
        Ok(verified) => TrustedSignerOutcome::Accepted {
            credential_id_hex: verified.credential_id_hex,
            assertion: WebAuthnAssertion {
                authenticator_data: verified.authenticator_data,
                client_data_json: verified.client_data_json,
                signature_der: verified.signature_der,
            },
        },
        Err(error) => TrustedSignerOutcome::Refused {
            refusal: error.into(),
        },
    }
}

/// The official page — always the first of the saved signing pages.
#[uniffi::export]
pub fn trusted_signer_default_url() -> String {
    trusted_signer::DEFAULT_SIGNER_URL.to_owned()
}

// --- The custom-scheme channel ---------------------------------------------
//
// The only channel the phones have (owner, 2026-09-23: 「回环 WebSocket 不做
// 呀，现在就是纯 custom schema」). The published page carries `default-src
// 'none'` inside its hashed bytes and cannot open a socket at all — measured,
// the listening server saw no byte — while a navigation to a custom scheme is
// not governed by that CSP, also measured. A suspended phone app could not
// have caught a loopback navigation anyway.

/// Where a page is told to send the answer: `velawallet://sign-result`.
#[uniffi::export]
#[must_use]
pub fn trusted_signer_callback_url() -> String {
    trusted_signer::CALLBACK_URL.to_owned()
}

/// Which pending request a `velawallet://…` the OS handed the app belongs to,
/// or `None` for every other URL — including the wallet's own `pay` and `open`
/// links.
///
/// A shell asks this FIRST, and `None` means: change nothing, say nothing.
#[uniffi::export]
#[must_use]
pub fn trusted_signer_callback_token(url: String) -> Option<String> {
    trusted_signer::callback_token(&url)
}

/// What a callback carried, once its token is known to be this attempt's.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum TrustedSignerCallback {
    /// The page answered. Still to be judged — by
    /// [`trusted_signer_verify`] for a signature, or
    /// [`trusted_signer_verify_ceremony`] for a ceremony.
    Answered { answer_json: String },
    /// The page refused, or the person did, or the callback is not this
    /// attempt's after all.
    Refused { refusal: TrustedSignerRefusal },
}

/// Read a whole `velawallet://sign-result?…` for the attempt `token` names.
///
/// The URL goes in whole rather than a query the shell carved out of it: the
/// carving is the part that is easy to get wrong, and it is already written
/// once in the core.
#[uniffi::export]
pub fn trusted_signer_parse_callback(url: String, token: String) -> TrustedSignerCallback {
    let Some(query) = trusted_signer::callback_of(&url) else {
        return TrustedSignerCallback::Refused {
            refusal: TrustedSignerRefusal::WrongToken,
        };
    };
    match trusted_signer::parse_callback(query, &token) {
        Ok(answer) => TrustedSignerCallback::Answered {
            answer_json: answer.to_string(),
        },
        Err(error) => TrustedSignerCallback::Refused {
            refusal: error.into(),
        },
    }
}

/// One TCP connection on the app's loopback listener, spoken to byte for
/// byte: the shell writes what [`TrustedSignerStep::write`] holds, closes when
/// told, and keeps listening until a step carries an outcome. The answer is
/// verified here, so an outcome is already the verdict.
#[derive(uniffi::Object)]
pub struct TrustedSignerConnection {
    inner: Mutex<trusted_signer::ws::Connection>,
    /// What the current request's answer is judged against (spec 075: a
    /// session carries several requests, each with its own judge).
    judge: Mutex<Judge>,
    signer_origin: String,
}

enum Judge {
    /// A signature over this digest by one of these keys (spec 071).
    Digest {
        digest: Vec<u8>,
        keys: Vec<WalletKeyRecord>,
    },
    /// A passkey ceremony for this machine operation (spec 075).
    Ceremony {
        operation_json: String,
        expected_member_challenge: Option<Vec<u8>>,
    },
}

/// What to do after bytes arrived.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TrustedSignerStep {
    pub write: Vec<u8>,
    pub close: bool,
    /// A signing request's verdict (spec 071).
    pub outcome: Option<TrustedSignerOutcome>,
    /// Spec 075: a ceremony's verdict — `None` for a signing request.
    pub ceremony: Option<TrustedSignerCeremonyOutcome>,
}

#[uniffi::export]
impl TrustedSignerConnection {
    /// `signer_url` is the page the app opened (its origin is the only one
    /// let in), `request_json` [`trusted_signer_request`]'s, `digest` what the
    /// passkey must sign, `keys` the account's.
    #[uniffi::constructor]
    pub fn new(
        signer_url: String,
        token: String,
        id: String,
        request_json: String,
        digest: Vec<u8>,
        keys: Vec<WalletKeyRecord>,
    ) -> Result<Arc<Self>, CoreError> {
        let request = serde_json::from_str(&request_json)
            .map_err(|e| CoreError::Internal(format!("request_json: {e}")))?;
        Ok(Arc::new(Self {
            inner: Mutex::new(trusted_signer::ws::Connection::new(
                &signer_url,
                &token,
                &id,
                &request,
            )),
            judge: Mutex::new(Judge::Digest { digest, keys }),
            signer_origin: trusted_signer::ws::origin_of(&signer_url),
        }))
    }

    /// Spec 075: a session that opens with a passkey ceremony
    /// (`trusted_signer_ceremony_request`'s request for `operation_json`).
    /// `expected_member_challenge` is the registry's challenge the wallet
    /// fetched itself, for a member proof.
    #[uniffi::constructor]
    pub fn new_ceremony(
        signer_url: String,
        token: String,
        id: String,
        request_json: String,
        operation_json: String,
        expected_member_challenge: Option<Vec<u8>>,
    ) -> Result<Arc<Self>, CoreError> {
        let request = serde_json::from_str(&request_json)
            .map_err(|e| CoreError::Internal(format!("request_json: {e}")))?;
        Ok(Arc::new(Self {
            inner: Mutex::new(trusted_signer::ws::Connection::new(
                &signer_url,
                &token,
                &id,
                &request,
            )),
            judge: Mutex::new(Judge::Ceremony {
                operation_json,
                expected_member_challenge,
            }),
            signer_origin: trusted_signer::ws::origin_of(&signer_url),
        }))
    }

    pub fn feed(&self, bytes: Vec<u8>) -> TrustedSignerStep {
        let step = lock(&self.inner).feed(&bytes);
        let mut out = TrustedSignerStep {
            write: step.write,
            close: step.close,
            outcome: None,
            ceremony: None,
        };
        if let Some(outcome) = step.outcome {
            match &*lock(&self.judge) {
                Judge::Digest { digest, keys } => {
                    out.outcome = Some(match outcome {
                        Ok(result) => judge(&result, digest, keys),
                        Err(error) => TrustedSignerOutcome::Refused {
                            refusal: error.into(),
                        },
                    });
                }
                Judge::Ceremony {
                    operation_json,
                    expected_member_challenge,
                } => {
                    out.ceremony = Some(match outcome {
                        Ok(answer) => ceremony_verdict(
                            operation_json,
                            &answer,
                            &self.signer_origin,
                            expected_member_challenge.as_deref(),
                        ),
                        Err(error) => TrustedSignerCeremonyOutcome::Refused {
                            refusal: error.into(),
                        },
                    });
                }
            }
        }
        out
    }

    /// Spec 075: the session's next request, a signature — once the last one
    /// has its answer (an empty step otherwise).
    pub fn send_signature(
        &self,
        id: String,
        request_json: String,
        digest: Vec<u8>,
        keys: Vec<WalletKeyRecord>,
    ) -> Result<TrustedSignerStep, CoreError> {
        let request = serde_json::from_str(&request_json)
            .map_err(|e| CoreError::Internal(format!("request_json: {e}")))?;
        let step = lock(&self.inner).send(&id, &request);
        if !step.write.is_empty() {
            *lock(&self.judge) = Judge::Digest { digest, keys };
        }
        Ok(bare(step))
    }

    /// Spec 075: the session's next request, a ceremony.
    pub fn send_ceremony(
        &self,
        id: String,
        request_json: String,
        operation_json: String,
        expected_member_challenge: Option<Vec<u8>>,
    ) -> Result<TrustedSignerStep, CoreError> {
        let request = serde_json::from_str(&request_json)
            .map_err(|e| CoreError::Internal(format!("request_json: {e}")))?;
        let step = lock(&self.inner).send(&id, &request);
        if !step.write.is_empty() {
            *lock(&self.judge) = Judge::Ceremony {
                operation_json,
                expected_member_challenge,
            };
        }
        Ok(bare(step))
    }

    /// Spec 075: the flow is done — `bye`, then close.
    pub fn end(&self) -> TrustedSignerStep {
        bare(lock(&self.inner).end())
    }

    /// The socket closed under the connection: `Declined` when a page that
    /// had the request went away, `None` when it never proved itself.
    pub fn closed(&self) -> Option<TrustedSignerRefusal> {
        lock(&self.inner).closed().map(Into::into)
    }
}

fn bare(step: trusted_signer::ws::Step) -> TrustedSignerStep {
    TrustedSignerStep {
        write: step.write,
        close: step.close,
        outcome: None,
        ceremony: None,
    }
}

// ---------------------------------------------------------------------------
// Spec 075: the Trusted Signer as a passkey route
// ---------------------------------------------------------------------------

/// A ceremony's answer, judged: the machine's own `Registration` /
/// `Assertion` as its wire JSON, ready to be reported in the shell result a
/// platform ceremony would have produced.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum TrustedSignerCeremonyOutcome {
    Registered { registration_json: String },
    Asserted { assertion_json: String },
    Refused { refusal: TrustedSignerRefusal },
}

/// The page request for a machine operation (`RegisterPasskey`,
/// `AuthenticatePasskey`, `SignProof`, `SignMemberProof` as their wire JSON)
/// with `method = trusted_signer`; `None` for anything else. `registry` is the
/// registry service the page fetches a member challenge from.
#[uniffi::export]
pub fn trusted_signer_ceremony_request(
    operation_json: String,
    id: String,
    wallet_name: String,
    registry: String,
    deployment: Option<SignerRegistryDeployment>,
) -> Option<String> {
    let ceremony = trusted_signer::ceremony::Ceremony::from_json(&operation_json)?;
    let deployment = deployment.map(|d| trusted_signer::ceremony::RegistryDeployment {
        chain_id: d.chain_id,
        contract: d.contract,
    });
    Some(
        trusted_signer::ceremony::request(
            &ceremony,
            &id,
            &wallet_name,
            &registry,
            deployment.as_ref(),
        )
        .to_string(),
    )
}

/// The registry deployment a member proof is bound to — the chain and the
/// `domainRegistry` contract, as `/api/health` names them.
///
/// The shell reads these once from its registry and hands them over, because
/// the page cannot: `default-src 'none'` is inside its hashed bytes (076), so a
/// page whose hash a wallet accepts reaches no network. The page computes the
/// challenge from them and refuses anything that does not match, so lying about
/// either fact produces a challenge the wallet did not ask for.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SignerRegistryDeployment {
    pub chain_id: u64,
    pub contract: String,
}

/// Judge a ceremony's answer that arrived by another channel (BLE).
#[uniffi::export]
pub fn trusted_signer_verify_ceremony(
    operation_json: String,
    answer_json: String,
    signer_origin: String,
    expected_member_challenge: Option<Vec<u8>>,
) -> TrustedSignerCeremonyOutcome {
    match serde_json::from_str::<serde_json::Value>(&answer_json) {
        Ok(answer) => ceremony_verdict(
            &operation_json,
            &answer,
            &signer_origin,
            expected_member_challenge.as_deref(),
        ),
        Err(error) => TrustedSignerCeremonyOutcome::Refused {
            refusal: TrustedSignerRefusal::Malformed {
                detail: error.to_string(),
            },
        },
    }
}

fn ceremony_verdict(
    operation_json: &str,
    answer: &serde_json::Value,
    signer_origin: &str,
    expected: Option<&[u8]>,
) -> TrustedSignerCeremonyOutcome {
    let Some(ceremony) = trusted_signer::ceremony::Ceremony::from_json(operation_json) else {
        return TrustedSignerCeremonyOutcome::Refused {
            refusal: TrustedSignerRefusal::Malformed {
                detail: "operation_json is not a Trusted Signer ceremony".to_owned(),
            },
        };
    };
    use trusted_signer::ceremony::Answer;
    let json = |value: Result<String, serde_json::Error>| value.unwrap_or_default();
    match trusted_signer::ceremony::verify(&ceremony, answer, signer_origin, expected) {
        Ok(Answer::Registration(registration)) => TrustedSignerCeremonyOutcome::Registered {
            registration_json: json(serde_json::to_string(&registration)),
        },
        Ok(Answer::Assertion(assertion)) => TrustedSignerCeremonyOutcome::Asserted {
            assertion_json: json(serde_json::to_string(&assertion)),
        },
        Err(error) => TrustedSignerCeremonyOutcome::Refused {
            refusal: error.into(),
        },
    }
}

// --- The registry's relying party ------------------------------------------

/// The rpId the registry must be asked for a key that lives behind a page —
/// the page's own domain, since that is the only one it could have minted the
/// key for. `None` when the key is on an authenticator this device reaches
/// itself: the wallet's own rpId, as before.
#[uniffi::export]
pub fn trusted_signer_registry_rp_id(signer_origin: Option<String>) -> Option<String> {
    trusted_signer::registry_rp_id(signer_origin.as_deref())
}

/// The relying party a whole unit belongs to, or the several it found — a
/// wallet's keys must share one, because the contract stores one `rpId` per
/// unit and every member proves membership under its own authenticator's.
/// A mixed set can never be proved, so it is refused before it is written.
#[uniffi::export]
pub fn trusted_signer_unit_rp_id(
    member_origins: Vec<Option<String>>,
    wallet_rp_id: String,
) -> Result<String, CoreError> {
    trusted_signer::registry_unit_rp_id(&member_origins, &wallet_rp_id).map_err(|found| {
        CoreError::Internal(format!(
            "these keys belong to different sites ({}), and a wallet's keys must share one",
            found.join(", ")
        ))
    })
}

/// A poisoned lock is a panic that already happened; the state behind it is a
/// connection's, not a wallet's, so the conversation carries on rather than
/// taking the app down with it.
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

// ---------------------------------------------------------------------------
// Spec 076: is the signer page the page it is supposed to be?
// ---------------------------------------------------------------------------
//
// The decision is `vela_core::trusted_signer::integrity`, and it is the same one
// the desktop asks — so the three shells cannot come to different conclusions
// about the same bytes. What a shell does is fetch: the index, then the version
// it chose, then hand the bytes here to be hashed and ruled on.

use vela_core::trusted_signer::integrity;

/// Why a check could not be completed. Every one of these opens nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignerCheckFailure {
    /// The page could not be reached: no connection, a timeout, a status that
    /// was not 200.
    Unreachable,
    /// It was reached and its bytes could not be read.
    NoBytes,
    /// It has not been checked yet.
    NotChecked,
}

impl From<SignerCheckFailure> for integrity::CheckFailure {
    fn from(value: SignerCheckFailure) -> Self {
        match value {
            SignerCheckFailure::Unreachable => Self::Unreachable,
            SignerCheckFailure::NoBytes => Self::NoCachedBytes,
            SignerCheckFailure::NotChecked => Self::NotChecked,
        }
    }
}

impl From<integrity::CheckFailure> for SignerCheckFailure {
    fn from(value: integrity::CheckFailure) -> Self {
        match value {
            integrity::CheckFailure::Unreachable => Self::Unreachable,
            integrity::CheckFailure::NoCachedBytes => Self::NoBytes,
            integrity::CheckFailure::NotChecked => Self::NotChecked,
        }
    }
}

/// Why there was no version to even ask for. The two need different words: one
/// sends a person to update the wallet, the other to their own block list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignerNoVersion {
    NothingPublishedThisWalletKnows,
    EverythingUsableIsBlocked,
}

impl From<integrity::NoVersion> for SignerNoVersion {
    fn from(value: integrity::NoVersion) -> Self {
        match value {
            integrity::NoVersion::NothingPublishedThisWalletKnows => {
                Self::NothingPublishedThisWalletKnows
            }
            integrity::NoVersion::EverythingUsableIsBlocked => Self::EverythingUsableIsBlocked,
        }
    }
}

/// What a shell should do about this page.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SignerPageVerdict {
    /// Open it.
    Open,
    /// Open it, and carry a STANDING warning that these bytes were not checked.
    /// Only reachable for a custom address whose owner turned verification off.
    OpenUnverified,
    /// Never: this hash is on the person's block list, which outranks
    /// everything including the build's own set.
    Denied { hash: String },
    /// Not a version this build knows, on the official address, where there is
    /// nobody to ask. The words are about hashes — never "an attack was
    /// prevented", which this check cannot tell from a wallet that is behind.
    Refused {
        actual: String,
        expected: Vec<String>,
    },
    /// A custom address serving a version this device has not decided about.
    AskToTrust { actual: String },
    /// The check did not complete. Nothing opens.
    CouldNotCheck { why: SignerCheckFailure },
    /// There was nothing to ask for; the check never started.
    NoVersionToAsk { why: SignerNoVersion },
}

impl From<integrity::Verdict> for SignerPageVerdict {
    fn from(value: integrity::Verdict) -> Self {
        match value {
            integrity::Verdict::Open => Self::Open,
            integrity::Verdict::OpenUnverified => Self::OpenUnverified,
            integrity::Verdict::Denied { hash } => Self::Denied { hash },
            integrity::Verdict::Refused { actual, expected } => Self::Refused { actual, expected },
            integrity::Verdict::AskToTrust { actual } => Self::AskToTrust { actual },
            integrity::Verdict::CouldNotCheck(why) => Self::CouldNotCheck { why: why.into() },
            integrity::Verdict::NoVersionToAsk(why) => Self::NoVersionToAsk { why: why.into() },
        }
    }
}

/// Everything the decision is made from. A shell already holds all of it by the
/// time it is about to open the page.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SignerPageCheck {
    /// The address Settings holds, normalised by `trusted_signer::signer_url`.
    pub url: String,
    /// The hash of the bytes that came back, or `None` when the fetch failed.
    pub observed_hash: Option<String>,
    /// Why, when `observed_hash` is `None`.
    pub failure: SignerCheckFailure,
    /// Hashes this person trusted ON THIS DEVICE. These never sync.
    pub trusted: Vec<String>,
    /// Hashes this person blocked on this device. Deny outranks everything.
    pub blocked: Vec<String>,
    /// Honoured only for a custom address; the core enforces that.
    pub verification_off: bool,
}

/// Which version to ask the endpoint for.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum SignerVersionChoice {
    Ask { hash: String },
    None { why: SignerNoVersion },
}

/// The hash of the bytes a page was served as — sha256, no normalisation.
///
/// A page IS its bytes: no trimming, no re-encoding, no line-ending fixes.
/// Anything that tidied them would make the published hash unreproducible.
#[uniffi::export]
#[must_use]
pub fn signer_page_hash(bytes: Vec<u8>) -> String {
    integrity::hash_page(&bytes)
}

/// Which published version to fetch, given what the endpoint says it has.
///
/// The index NARROWS the choice and never makes it: the order is this
/// client's, so a lying index can only hide versions (a loud refusal) or offer
/// ones this wallet does not trust (ignored).
#[uniffi::export]
#[must_use]
pub fn signer_page_choose_version(
    available: Vec<String>,
    trusted: Vec<String>,
    blocked: Vec<String>,
) -> SignerVersionChoice {
    match integrity::choose_version(&available, &trusted, &blocked) {
        Ok(hash) => SignerVersionChoice::Ask { hash },
        Err(why) => SignerVersionChoice::None { why: why.into() },
    }
}

/// Where a known version lives, for any deployment of `dist/`.
#[uniffi::export]
#[must_use]
pub fn signer_page_url(base: String, hash: String) -> Option<String> {
    integrity::content_addressed_url(&base, &hash)
}

/// Whether the page may be opened, and what to say when it may not.
#[uniffi::export]
#[must_use]
pub fn signer_page_decide(check: SignerPageCheck) -> SignerPageVerdict {
    integrity::decide(&integrity::Page {
        url: &check.url,
        observed: check.observed_hash.as_deref(),
        failure: check.failure.into(),
        trusted: &check.trusted,
        blocked: &check.blocked,
        verification_off: check.verification_off,
    })
    .into()
}

/// Whether a refused page stays closed. On since spec 102 (R6):
/// [`signer_page_admit`] refuses it, and nothing else can open a page.
///
/// Deliberately not "is the allow-set non-empty": listing the first hash must
/// not, by itself, start refusing pages that are not published yet.
#[uniffi::export]
#[must_use]
pub fn signer_page_enforced() -> bool {
    integrity::ENFORCE
}

/// The hashes this build accepts. Shown in Settings, so a person can see which
/// version is in force and block it.
#[uniffi::export]
#[must_use]
pub fn signer_page_allowed() -> Vec<String> {
    integrity::BUILD_ALLOWED
        .iter()
        .map(|hash| (*hash).to_owned())
        .collect()
}

// ---------------------------------------------------------------------------
// Spec 102 R6: the version that is checked is the version that is opened
// ---------------------------------------------------------------------------
//
// `vela_core::trusted_signer::launch` holds the rule. A shell asks it which
// page to check (`SignerPageTarget::choose`), fetches EXACTLY that URL, hashes
// the bytes, and asks it to rule (`signer_page_admit`). Only the admission can
// build a launch URL, so a shell cannot open a page it did not check, nor a
// version other than the one it checked.

use vela_core::trusted_signer::launch;

/// The page a wallet will fetch, check and open: one version of one
/// deployment, and the one URL that serves it. Made only by
/// [`SignerPageTarget::choose`].
#[derive(uniffi::Object)]
pub struct SignerPageTarget {
    chosen: Result<launch::Target, integrity::NoVersion>,
}

#[uniffi::export]
impl SignerPageTarget {
    /// R6: which version of the page at `base` to open, and therefore to
    /// check. `index` is the deployment's `index.json` list, `None` when it
    /// could not be read (the official page then opens its launch version).
    #[uniffi::constructor]
    pub fn choose(
        base: String,
        index: Option<Vec<String>>,
        trusted: Vec<String>,
        blocked: Vec<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            chosen: launch::target(&base, index.as_deref(), &trusted, &blocked),
        })
    }

    /// The URL to fetch — and the URL a launch opens. `None` when there is no
    /// version to ask for ([`Self::no_version`] says why).
    pub fn url(&self) -> Option<String> {
        self.chosen.as_ref().ok().map(|t| t.url().to_owned())
    }

    /// The version (sha256 hex) that URL must serve.
    pub fn version(&self) -> Option<String> {
        self.chosen.as_ref().ok().map(|t| t.version().to_owned())
    }

    /// A custom page's own build, named by its index: checking it can only end
    /// in asking the person.
    pub fn proposed_by_index(&self) -> bool {
        self.chosen
            .as_ref()
            .is_ok_and(launch::Target::proposed_by_index)
    }

    /// Why there was no version to ask for.
    pub fn no_version(&self) -> Option<SignerNoVersion> {
        self.chosen.as_ref().err().map(|why| (*why).into())
    }

    /// The line to draw while there is no check: "checking", or why there is
    /// nothing to check.
    pub fn line(&self) -> SignerIntegrityLine {
        match &self.chosen {
            Ok(_) => launch::IntegrityLine::checking(),
            Err(why) => launch::IntegrityLine::no_version(*why),
        }
        .into()
    }
}

/// What the hand-off card and Settings draw about a page (spec 102, D4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum SignerIntegrityState {
    Checking,
    Matches,
    TrustedHere,
    Unchecked,
    Mismatch,
    Blocked,
    AskToTrust,
    CouldNotCheck,
    NoVersion,
    AllBlocked,
}

impl From<launch::IntegrityState> for SignerIntegrityState {
    fn from(value: launch::IntegrityState) -> Self {
        use launch::IntegrityState as S;
        match value {
            S::Checking => Self::Checking,
            S::Matches => Self::Matches,
            S::TrustedHere => Self::TrustedHere,
            S::Unchecked => Self::Unchecked,
            S::Mismatch => Self::Mismatch,
            S::Blocked => Self::Blocked,
            S::AskToTrust => Self::AskToTrust,
            S::CouldNotCheck => Self::CouldNotCheck,
            S::NoVersion => Self::NoVersion,
            S::AllBlocked => Self::AllBlocked,
        }
    }
}

/// "Version 0ba8ee8c · matches Vela's published build list · checked …", or
/// why the page will not open: `key` is the corpus key, with `{{version}}`
/// (the first eight hex characters) and `{{time}}` (from `checked_at_ms`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SignerIntegrityLine {
    pub state: SignerIntegrityState,
    pub version: String,
    pub checked_at_ms: Option<u64>,
    pub key: String,
    /// The Open button is enabled only when `true`.
    pub opens: bool,
}

impl From<launch::IntegrityLine> for SignerIntegrityLine {
    fn from(line: launch::IntegrityLine) -> Self {
        Self {
            state: line.state.into(),
            version: line.version,
            checked_at_ms: line.checked_at_ms,
            key: line.key,
            opens: line.opens,
        }
    }
}

/// R6: the ruling on a target's bytes — and, when it admits the page, the only
/// way to open it.
#[derive(uniffi::Object)]
pub struct SignerPageAdmission {
    /// `Err`: the target had no version to ask for, and why.
    inner: Result<launch::Admission, integrity::NoVersion>,
}

/// Rule on the bytes `target`'s URL served: `observed_hash` is
/// [`signer_page_hash`] of exactly those bytes, `None` when the fetch failed
/// (`failure` says how). `checked_at_ms` is the shell's clock when they
/// arrived. A target with no version admits nothing.
#[uniffi::export]
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
pub fn signer_page_admit(
    target: Arc<SignerPageTarget>,
    observed_hash: Option<String>,
    failure: SignerCheckFailure,
    trusted: Vec<String>,
    blocked: Vec<String>,
    verification_off: bool,
    checked_at_ms: u64,
) -> Arc<SignerPageAdmission> {
    Arc::new(SignerPageAdmission {
        inner: target.chosen.as_ref().map_err(|why| *why).map(|chosen| {
            launch::admit(
                chosen,
                observed_hash.as_deref(),
                failure.into(),
                &trusted,
                &blocked,
                verification_off,
                checked_at_ms,
            )
        }),
    })
}

#[uniffi::export]
impl SignerPageAdmission {
    /// May the page be opened (now, or until the check goes stale)?
    pub fn opens(&self) -> bool {
        self.page().is_some()
    }

    /// The line to draw at `now_ms` — "checking" once the check is too old.
    pub fn line(&self, now_ms: u64) -> SignerIntegrityLine {
        match &self.inner {
            Ok(launch::Admission::Admitted(page)) => page.line(now_ms).into(),
            Ok(refused) => refused.line().into(),
            Err(why) => launch::IntegrityLine::no_version(*why).into(),
        }
    }

    /// The decision underneath, for the details a refusal opens onto (both
    /// hashes, say).
    pub fn verdict(&self) -> Option<SignerPageVerdict> {
        match self.inner.as_ref().ok()? {
            launch::Admission::Admitted(page) => Some(page.verdict().clone().into()),
            launch::Admission::Refused { verdict, .. } => Some(verdict.clone().into()),
        }
    }

    /// The checked page, told to answer over [`trusted_signer_callback_url`]:
    /// the request deflated into the URL's FRAGMENT, which no server sees.
    /// Refused when the page was not admitted, or the check is too old
    /// (check again, then open).
    pub fn url_launch(
        &self,
        request_json: String,
        token: String,
        now_ms: u64,
    ) -> Result<String, CoreError> {
        let page = self.page().ok_or_else(not_admitted)?;
        let request = serde_json::from_str(&request_json)
            .map_err(|e| CoreError::Internal(format!("request_json: {e}")))?;
        page.url_launch(&request, trusted_signer::CALLBACK_URL, &token, now_ms)
            .map_err(|e| CoreError::Internal(e.to_string()))
    }

    /// The checked page, told to connect to this app's loopback WebSocket.
    pub fn ws_launch(&self, port: u16, token: String, now_ms: u64) -> Result<String, CoreError> {
        let page = self.page().ok_or_else(not_admitted)?;
        page.ws_launch(port, &token, now_ms)
            .map_err(|e| CoreError::Internal(e.to_string()))
    }
}

impl SignerPageAdmission {
    fn page(&self) -> Option<&launch::CheckedPage> {
        self.inner.as_ref().ok().and_then(launch::Admission::page)
    }
}

fn not_admitted() -> CoreError {
    CoreError::Internal("the signing page did not pass its check, so it is not opened".to_owned())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const OFFICIAL: &str = "https://sign.getvela.app/";

    /// Spec 102 R6 across the boundary: a phone fetches the target's URL, and
    /// only an admission of exactly that version opens it — at that URL.
    #[test]
    fn a_phone_opens_only_the_version_it_checked() {
        let target = SignerPageTarget::choose(OFFICIAL.to_owned(), None, vec![], vec![]);
        let url = target.url().expect("the launch version");
        let version = target.version().expect("a version");
        assert!(url.contains(&version));
        assert_eq!(target.line().state, SignerIntegrityState::Checking);

        let admitted = signer_page_admit(
            target.clone(),
            Some(version.clone()),
            SignerCheckFailure::NotChecked,
            vec![],
            vec![],
            false,
            1_000,
        );
        assert!(admitted.opens());
        let line = admitted.line(2_000);
        assert_eq!(line.state, SignerIntegrityState::Matches);
        assert_eq!(line.checked_at_ms, Some(1_000));
        let launch = admitted
            .url_launch("{}".to_owned(), "t".to_owned(), 2_000)
            .expect("admitted");
        assert!(launch.starts_with(&format!("{url}?ch=url#")), "{launch}");
        assert!(launch.contains("&z=1"), "{launch}");
        // Too old: check again first.
        assert!(admitted
            .url_launch(
                "{}".to_owned(),
                "t".to_owned(),
                1_000 + launch::MAX_CHECK_AGE_MS + 1
            )
            .is_err());
    }

    /// A failed fetch, or bytes that are not the version named, open nothing.
    #[test]
    fn a_failed_or_mismatched_check_opens_nothing() {
        let target = SignerPageTarget::choose(OFFICIAL.to_owned(), None, vec![], vec![]);
        let failed = signer_page_admit(
            target.clone(),
            None,
            SignerCheckFailure::Unreachable,
            vec![],
            vec![],
            false,
            1,
        );
        assert!(!failed.opens());
        assert_eq!(failed.line(1).state, SignerIntegrityState::CouldNotCheck);
        assert!(failed
            .url_launch("{}".to_owned(), "t".to_owned(), 1)
            .is_err());

        let other = integrity::BUILD_ALLOWED
            .iter()
            .find(|hash| **hash != target.version().unwrap_or_default())
            .map(|hash| (*hash).to_owned());
        let mismatched = signer_page_admit(
            target,
            other,
            SignerCheckFailure::NotChecked,
            vec![],
            vec![],
            false,
            1,
        );
        assert!(!mismatched.opens());
        assert_eq!(mismatched.line(1).state, SignerIntegrityState::Mismatch);
        assert!(matches!(
            mismatched.verdict(),
            Some(SignerPageVerdict::Refused { .. })
        ));
    }

    /// Nothing to ask for: the reason survives to the card.
    #[test]
    fn no_version_says_why() {
        let blocked: Vec<String> = integrity::BUILD_ALLOWED
            .iter()
            .map(|hash| (*hash).to_owned())
            .collect();
        let target = SignerPageTarget::choose(OFFICIAL.to_owned(), None, vec![], blocked.clone());
        assert_eq!(target.url(), None);
        assert_eq!(
            target.no_version(),
            Some(SignerNoVersion::EverythingUsableIsBlocked)
        );
        let admitted = signer_page_admit(
            target,
            None,
            SignerCheckFailure::NotChecked,
            vec![],
            blocked,
            false,
            1,
        );
        assert_eq!(admitted.line(1).state, SignerIntegrityState::AllBlocked);
    }

    /// R5 across the boundary: the plan's key route, passed through as JSON.
    #[test]
    fn the_request_carries_the_key_route() {
        let route = vela_core::signing_venue::KeyRoute::new("aabb", "security_key", "usb,nfc,ble");
        let built = trusted_signer_request(
            TrustedSignerInput {
                method: "personal_sign".to_owned(),
                params_json: r#"["0x68", "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]"#.to_owned(),
                origin: String::new(),
                origin_seen_by_browser: false,
                chain_id: 1,
                chain_name: None,
                native_symbol: None,
                account: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                account_name: None,
                credential_ids_hex: vec!["1122".to_owned(), "aabb".to_owned()],
                key_route_json: Some(serde_json::to_string(&route).unwrap()),
                calls: vec![],
            },
            None,
        )
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&built).unwrap();
        assert_eq!(
            value["context"]["allowCredentials"],
            serde_json::json!(["qrs"])
        );
        assert_eq!(
            value["context"]["keyRoute"]["hints"],
            serde_json::json!(["security-key"])
        );
    }
}
