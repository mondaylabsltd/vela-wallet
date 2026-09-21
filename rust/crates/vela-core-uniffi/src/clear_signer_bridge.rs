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
    let (method, params) = if input.method.is_empty() {
        let calls = inner_calls(&input.calls)?;
        (
            "wallet_sendCalls",
            clear_signer::own_send_params(u64::from(input.chain_id), &input.account, &calls),
        )
    } else {
        let params = serde_json::from_str(&input.params_json)
            .map_err(|e| CoreError::Internal(format!("params_json: {e}")))?;
        (input.method.as_str(), params)
    };
    // The wallet appends the fee leg after the calls, on every chain.
    let fee_leg_index = (op.is_some() && !input.calls.is_empty()).then_some(input.calls.len());
    let built = clear_signer::request(&RequestInput {
        method,
        params,
        origin: &input.origin,
        chain_id: u64::from(input.chain_id),
        chain_name: input.chain_name.as_deref(),
        native_symbol: input.native_symbol.as_deref(),
        account: &input.account,
        account_name: input.account_name.as_deref(),
        credential_ids_hex: &input.credential_ids_hex,
        user_op: op.as_ref(),
        fee_leg_index,
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
    digest: Vec<u8>,
    keys: Vec<WalletKeyRecord>,
}

/// What to do after bytes arrived.
#[derive(Debug, Clone, uniffi::Record)]
pub struct ClearSignerStep {
    pub write: Vec<u8>,
    pub close: bool,
    pub outcome: Option<ClearSignerOutcome>,
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
            digest,
            keys,
        }))
    }

    pub fn feed(&self, bytes: Vec<u8>) -> ClearSignerStep {
        let step = lock(&self.inner).feed(&bytes);
        ClearSignerStep {
            write: step.write,
            close: step.close,
            outcome: step.outcome.map(|outcome| match outcome {
                Ok(result) => judge(&result, &self.digest, &self.keys),
                Err(error) => ClearSignerOutcome::Refused {
                    refusal: error.into(),
                },
            }),
        }
    }

    /// The socket closed under the connection: `Declined` when a page that
    /// had the request went away, `None` when it never proved itself.
    pub fn closed(&self) -> Option<ClearSignerRefusal> {
        lock(&self.inner).closed().map(Into::into)
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}
