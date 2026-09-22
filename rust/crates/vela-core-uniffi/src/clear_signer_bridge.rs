//! The Clear Signer over UniFFI (spec 071): the request a clear-signing page
//! receives, the verification of what it answers, and the loopback WebSocket
//! between the phone app and the page — so no shell builds, parses, frames or
//! trusts any of it on its own.

use crate::{
    inner_calls, op_of, CoreError, UserOpCall, UserOpDraft, WalletKeyRecord, WebAuthnAssertion,
};
use std::sync::{Arc, Mutex};

use vela_core::clear_signer::{self, ClearSignerError, RequestInput};

/// What the shell holds when it would sign.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ClearSignerInput {
    /// The request's own method (`eth_sendTransaction`, `personal_sign`, …);
    /// EMPTY for the wallet's own send, whose intent is built from `calls`.
    pub method: String,
    /// Its JSON-RPC params, verbatim (ignored when `method` is empty).
    pub params_json: String,
    /// The requesting site's origin; empty for the wallet's own send.
    pub origin: String,
    pub chain_id: u32,
    pub chain_name: Option<String>,
    pub native_symbol: Option<String>,
    pub account: String,
    pub account_name: Option<String>,
    /// The account's credential ids, hex.
    pub credential_ids_hex: Vec<String>,
    /// For a transaction: the calls the operation carries BEFORE its fee leg
    /// (the wallet appends the fee last, so its index is `calls.len()`). With
    /// an empty `method` — the wallet's own send, which no site asked for —
    /// they are also the intent (`wallet_sendCalls`). Empty for a message.
    pub calls: Vec<UserOpCall>,
}

/// The page's `{intent, context}` as JSON. `draft` is the ASSEMBLED
/// operation for a transaction (the digest covers it), `None` for a message.
#[uniffi::export]
pub fn clear_signer_request(
    input: ClearSignerInput,
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
    let built = clear_signer::request(&RequestInput {
        method: &input.method,
        params,
        origin: &input.origin,
        chain_id: u64::from(input.chain_id),
        chain_name: input.chain_name.as_deref(),
        native_symbol: input.native_symbol.as_deref(),
        account: &input.account,
        account_name: input.account_name.as_deref(),
        credential_ids_hex: &input.credential_ids_hex,
        user_op: op.as_ref(),
        calls: &calls,
    });
    Ok(built.to_string())
}

/// Why a Clear Signer answer was not accepted — or what the page said.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum ClearSignerRefusal {
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

impl From<ClearSignerError> for ClearSignerRefusal {
    fn from(error: ClearSignerError) -> Self {
        match error {
            ClearSignerError::Declined => Self::Declined,
            ClearSignerError::Refused(code) => Self::PageRefused { code },
            ClearSignerError::WrongChallenge => Self::WrongChallenge,
            ClearSignerError::ForeignKey => Self::ForeignKey,
            ClearSignerError::BadSignature => Self::BadSignature,
            ClearSignerError::NotVerified => Self::NotVerified,
            ClearSignerError::WrongToken => Self::WrongToken,
            ClearSignerError::Malformed(detail) => Self::Malformed { detail },
        }
    }
}

/// The page's answer, judged.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ClearSignerOutcome {
    /// Over this digest, by this wallet's key, user-verified: sign with it
    /// (`user_op_sign` / `eip1271_signature`) exactly as with any passkey.
    Accepted {
        credential_id_hex: String,
        assertion: WebAuthnAssertion,
    },
    Refused {
        refusal: ClearSignerRefusal,
    },
}

/// Accept `result_json` only if it signs `digest` with one of `keys` — the
/// URL and postMessage channels' answer.
#[uniffi::export]
pub fn clear_signer_verify(
    result_json: String,
    digest: Vec<u8>,
    keys: Vec<WalletKeyRecord>,
) -> ClearSignerOutcome {
    match serde_json::from_str(&result_json) {
        Ok(result) => judge(&result, &digest, &keys),
        Err(error) => ClearSignerOutcome::Refused {
            refusal: ClearSignerRefusal::Malformed {
                detail: error.to_string(),
            },
        },
    }
}

fn judge(
    result: &serde_json::Value,
    digest: &[u8],
    keys: &[WalletKeyRecord],
) -> ClearSignerOutcome {
    let keys: Vec<vela_core::user_op::WalletKey> = keys
        .iter()
        .map(|key| vela_core::user_op::WalletKey {
            credential_id: key.credential_id.clone(),
            public_key_hex: key.public_key_hex.clone(),
        })
        .collect();
    match clear_signer::verify(result, digest, &keys) {
        Ok(verified) => ClearSignerOutcome::Accepted {
            credential_id_hex: verified.credential_id_hex,
            assertion: WebAuthnAssertion {
                authenticator_data: verified.authenticator_data,
                client_data_json: verified.client_data_json,
                signature_der: verified.signature_der,
            },
        },
        Err(error) => ClearSignerOutcome::Refused {
            refusal: error.into(),
        },
    }
}

/// The official page, the setting's default.
#[uniffi::export]
pub fn clear_signer_default_url() -> String {
    clear_signer::DEFAULT_SIGNER_URL.to_owned()
}

/// The page, told to connect to this app's loopback WebSocket.
#[uniffi::export]
pub fn clear_signer_ws_launch(base: String, port: u16, token: String) -> String {
    clear_signer::ws_launch(&base, port, &token)
}

/// One TCP connection on the app's loopback listener, spoken to byte for
/// byte: the shell writes what [`ClearSignerStep::write`] holds, closes when
/// told, and keeps listening until a step carries an outcome. The answer is
/// verified here, so an outcome is already the verdict.
#[derive(uniffi::Object)]
pub struct ClearSignerConnection {
    inner: Mutex<clear_signer::ws::Connection>,
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
pub struct ClearSignerStep {
    pub write: Vec<u8>,
    pub close: bool,
    /// A signing request's verdict (spec 071).
    pub outcome: Option<ClearSignerOutcome>,
    /// Spec 075: a ceremony's verdict — `None` for a signing request.
    pub ceremony: Option<ClearSignerCeremonyOutcome>,
}

#[uniffi::export]
impl ClearSignerConnection {
    /// `signer_url` is the page the app opened (its origin is the only one
    /// let in), `request_json` [`clear_signer_request`]'s, `digest` what the
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
            inner: Mutex::new(clear_signer::ws::Connection::new(
                &signer_url,
                &token,
                &id,
                &request,
            )),
            judge: Mutex::new(Judge::Digest { digest, keys }),
            signer_origin: clear_signer::ws::origin_of(&signer_url),
        }))
    }

    /// Spec 075: a session that opens with a passkey ceremony
    /// (`clear_signer_ceremony_request`'s request for `operation_json`).
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
            inner: Mutex::new(clear_signer::ws::Connection::new(
                &signer_url,
                &token,
                &id,
                &request,
            )),
            judge: Mutex::new(Judge::Ceremony {
                operation_json,
                expected_member_challenge,
            }),
            signer_origin: clear_signer::ws::origin_of(&signer_url),
        }))
    }

    pub fn feed(&self, bytes: Vec<u8>) -> ClearSignerStep {
        let step = lock(&self.inner).feed(&bytes);
        let mut out = ClearSignerStep {
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
                        Err(error) => ClearSignerOutcome::Refused {
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
                        Err(error) => ClearSignerCeremonyOutcome::Refused {
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
    ) -> Result<ClearSignerStep, CoreError> {
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
    ) -> Result<ClearSignerStep, CoreError> {
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
    pub fn end(&self) -> ClearSignerStep {
        bare(lock(&self.inner).end())
    }

    /// The socket closed under the connection: `Declined` when a page that
    /// had the request went away, `None` when it never proved itself.
    pub fn closed(&self) -> Option<ClearSignerRefusal> {
        lock(&self.inner).closed().map(Into::into)
    }
}

fn bare(step: clear_signer::ws::Step) -> ClearSignerStep {
    ClearSignerStep {
        write: step.write,
        close: step.close,
        outcome: None,
        ceremony: None,
    }
}

// ---------------------------------------------------------------------------
// Spec 075: the Clear Signer as a passkey route
// ---------------------------------------------------------------------------

/// A ceremony's answer, judged: the machine's own `Registration` /
/// `Assertion` as its wire JSON, ready to be reported in the shell result a
/// platform ceremony would have produced.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum ClearSignerCeremonyOutcome {
    Registered { registration_json: String },
    Asserted { assertion_json: String },
    Refused { refusal: ClearSignerRefusal },
}

/// The page request for a machine operation (`RegisterPasskey`,
/// `AuthenticatePasskey`, `SignProof`, `SignMemberProof` as their wire JSON)
/// with `method = clear_signer`; `None` for anything else. `registry` is the
/// registry service the page fetches a member challenge from.
#[uniffi::export]
pub fn clear_signer_ceremony_request(
    operation_json: String,
    id: String,
    wallet_name: String,
    registry: String,
) -> Option<String> {
    let ceremony = clear_signer::ceremony::Ceremony::from_json(&operation_json)?;
    Some(clear_signer::ceremony::request(&ceremony, &id, &wallet_name, &registry).to_string())
}

/// Judge a ceremony's answer that arrived by another channel (the relay, BLE).
#[uniffi::export]
pub fn clear_signer_verify_ceremony(
    operation_json: String,
    answer_json: String,
    signer_origin: String,
    expected_member_challenge: Option<Vec<u8>>,
) -> ClearSignerCeremonyOutcome {
    match serde_json::from_str::<serde_json::Value>(&answer_json) {
        Ok(answer) => ceremony_verdict(
            &operation_json,
            &answer,
            &signer_origin,
            expected_member_challenge.as_deref(),
        ),
        Err(error) => ClearSignerCeremonyOutcome::Refused {
            refusal: ClearSignerRefusal::Malformed {
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
) -> ClearSignerCeremonyOutcome {
    let Some(ceremony) = clear_signer::ceremony::Ceremony::from_json(operation_json) else {
        return ClearSignerCeremonyOutcome::Refused {
            refusal: ClearSignerRefusal::Malformed {
                detail: "operation_json is not a Clear Signer ceremony".to_owned(),
            },
        };
    };
    use clear_signer::ceremony::Answer;
    let json = |value: Result<String, serde_json::Error>| value.unwrap_or_default();
    match clear_signer::ceremony::verify(&ceremony, answer, signer_origin, expected) {
        Ok(Answer::Registration(registration)) => ClearSignerCeremonyOutcome::Registered {
            registration_json: json(serde_json::to_string(&registration)),
        },
        Ok(Answer::Assertion(assertion)) => ClearSignerCeremonyOutcome::Asserted {
            assertion_json: json(serde_json::to_string(&assertion)),
        },
        Err(error) => ClearSignerCeremonyOutcome::Refused {
            refusal: error.into(),
        },
    }
}

/// The relay the wallet pairs through unless Settings name another.
#[uniffi::export]
pub fn clear_signer_default_relay() -> String {
    clear_signer::DEFAULT_RELAY_URL.to_owned()
}

/// A relay address the person typed, normalised — `None` when it cannot be
/// used (wss anywhere, ws only on loopback).
#[uniffi::export]
pub fn clear_signer_relay_url(input: String) -> Option<String> {
    clear_signer::relay_url(&input).ok()
}

/// A room id from 16 random bytes the shell drew.
#[uniffi::export]
pub fn clear_signer_relay_room(random: Vec<u8>) -> Option<String> {
    let bytes: [u8; 16] = random.try_into().ok()?;
    Some(clear_signer::relay_room(&bytes))
}

/// The socket the wallet opens: `<relay>/v1/rooms/<room>?role=requester`.
#[uniffi::export]
pub fn clear_signer_relay_room_url(relay: String, room: String) -> String {
    clear_signer::relay_room_url(&relay, &room, "requester")
}

/// The pairing link (QR + copy) for the page on another device.
#[uniffi::export]
pub fn clear_signer_relay_link(
    signer_url: String,
    relay: String,
    room: String,
    rk: String,
) -> String {
    clear_signer::relay_link(&signer_url, &relay, &room, &rk)
}

/// `rk`: the requester key's fingerprint the pairing link carries.
#[uniffi::export]
pub fn clear_signer_key_fingerprint(public_key: Vec<u8>) -> String {
    clear_signer::secure::key_fingerprint(&public_key)
}

/// The wallet's side of the end-to-end session (relay and BLE), before the
/// page's hello. The shell draws the 32 secret bytes and the 16-byte nonce.
#[derive(uniffi::Object)]
pub struct ClearSignerHandshake {
    inner: Mutex<Option<clear_signer::secure::Handshake>>,
    public_key: Vec<u8>,
}

#[uniffi::export]
impl ClearSignerHandshake {
    #[uniffi::constructor]
    pub fn new(secret: Vec<u8>, nonce: Vec<u8>) -> Result<Arc<Self>, CoreError> {
        let secret: [u8; 32] = secret
            .try_into()
            .map_err(|_| CoreError::Internal("secret must be 32 bytes".into()))?;
        let nonce: [u8; 16] = nonce
            .try_into()
            .map_err(|_| CoreError::Internal("nonce must be 16 bytes".into()))?;
        let handshake = clear_signer::secure::Handshake::new(
            &secret,
            nonce,
            clear_signer::secure::Role::Requester,
        )
        .map_err(|e| CoreError::Internal(e.to_string()))?;
        let public_key = handshake.public_key().to_vec();
        Ok(Arc::new(Self {
            inner: Mutex::new(Some(handshake)),
            public_key,
        }))
    }

    /// The 65-byte key — hash it into the pairing link's `rk`.
    pub fn public_key(&self) -> Vec<u8> {
        self.public_key.clone()
    }

    /// This side's hello text frame.
    pub fn hello(&self, app: Option<String>) -> String {
        lock(&self.inner)
            .as_ref()
            .map(|h| h.hello(app.as_deref()))
            .unwrap_or_default()
    }

    /// Finish with the page's hello. `relay` picks the label (`vela-relay/1`,
    /// else `vela-ble/1`). Once only.
    pub fn complete(
        &self,
        peer_hello: String,
        relay: bool,
    ) -> Result<Arc<ClearSignerSession>, CoreError> {
        let handshake = lock(&self.inner)
            .take()
            .ok_or_else(|| CoreError::Internal("the handshake is already complete".into()))?;
        let label = if relay {
            clear_signer::secure::Label::Relay
        } else {
            clear_signer::secure::Label::Ble
        };
        let session = handshake
            .complete(&peer_hello, label, None)
            .map_err(|e| CoreError::Internal(e.to_string()))?;
        Ok(Arc::new(ClearSignerSession {
            inner: Mutex::new(session),
        }))
    }
}

/// An established end-to-end session: the code to show, and sealing.
#[derive(uniffi::Object)]
pub struct ClearSignerSession {
    inner: Mutex<clear_signer::secure::Session>,
}

#[uniffi::export]
impl ClearSignerSession {
    /// The six digits the wallet shows beside the page's.
    pub fn code(&self) -> String {
        lock(&self.inner).code().to_owned()
    }

    /// Seal the next outgoing message. `msg_id` is BLE's frame message id;
    /// `None` on the relay (the AAD binds the counter).
    pub fn seal(&self, plaintext: Vec<u8>, msg_id: Option<u8>) -> Vec<u8> {
        lock(&self.inner).seal(&plaintext, tail(msg_id))
    }

    /// Open the page's next message; a replay, a reflection or a tampered
    /// frame is refused.
    pub fn open(&self, sealed: Vec<u8>, msg_id: Option<u8>) -> Result<Vec<u8>, CoreError> {
        lock(&self.inner)
            .open(&sealed, tail(msg_id))
            .map_err(|e| CoreError::Internal(e.to_string()))
    }
}

fn tail(msg_id: Option<u8>) -> clear_signer::secure::Tail {
    msg_id.map_or(
        clear_signer::secure::Tail::Counter,
        clear_signer::secure::Tail::MsgId,
    )
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
