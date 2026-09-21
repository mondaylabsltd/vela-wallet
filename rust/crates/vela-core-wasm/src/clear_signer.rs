//! The Clear Signer for the web wallet (spec 071): the request the page
//! receives, the verdict on what it answers, and the loopback WebSocket the
//! phones speak — exported so the web builds and judges nothing itself, and
//! so the page's suite can talk to the very connection the phones run.
//!
//! JSON in, JSON out: the web holds these as JSON already.

use serde_json::{json, Value};
use vela_core::clear_signer::{self, ws, ClearSignerError, RequestInput, SignerUrlError};
use vela_core::user_op::WalletKey;
use wasm_bindgen::prelude::*;

fn internal(message: String) -> JsValue {
    super::err(vela_core::CoreError::Internal(message))
}

#[wasm_bindgen(js_name = clearSignerDefaultUrl)]
pub fn clear_signer_default_url() -> String {
    clear_signer::DEFAULT_SIGNER_URL.to_owned()
}

/// The address normalised, or throws `"invalid"` / `"insecure"`.
#[wasm_bindgen(js_name = clearSignerUrl)]
pub fn clear_signer_url(input: &str) -> Result<String, JsValue> {
    clear_signer::signer_url(input).map_err(|error| {
        JsValue::from_str(match error {
            SignerUrlError::Invalid => "invalid",
            SignerUrlError::Insecure => "insecure",
        })
    })
}

#[wasm_bindgen(js_name = clearSignerUsesWalletPasskeys)]
pub fn clear_signer_uses_wallet_passkeys(url: &str) -> bool {
    clear_signer::uses_wallet_passkeys(url)
}

/// `{method, params, origin, chainId, chainName?, nativeSymbol?, account,
/// accountName?, credentialIdsHex, userOp?, calls?}` → the page's `{intent,
/// context}`. As the phones' `clear_signer_request`: `calls` are the
/// operation's legs before its fee leg (so the fee leg is `calls.length`),
/// and an empty `method` — the wallet's own send — makes them the intent.
#[wasm_bindgen(js_name = clearSignerRequest)]
pub fn clear_signer_request(input_json: &str) -> Result<String, JsValue> {
    let input: Value =
        serde_json::from_str(input_json).map_err(|e| internal(format!("input: {e}")))?;
    let text = |name: &str| input.get(name).and_then(Value::as_str);
    let op = match input.get("userOp") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            clear_signer::user_op_from_json(value)
                .ok_or_else(|| internal("input: userOp".to_owned()))?,
        ),
    };
    let ids: Vec<String> = input
        .get("credentialIdsHex")
        .and_then(Value::as_array)
        .map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default();
    let calls = input
        .get("calls")
        .and_then(Value::as_array)
        .map(|calls| {
            calls
                .iter()
                .map(|call| {
                    let field =
                        |name: &str| call.get(name).and_then(Value::as_str).unwrap_or_default();
                    vela_core::user_op::to_multi_send_call(
                        field("to"),
                        field("value"),
                        field("data"),
                    )
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()
        .map_err(super::err)?
        .unwrap_or_default();
    let built = clear_signer::request(&RequestInput {
        method: text("method").unwrap_or_default(),
        params: input
            .get("params")
            .cloned()
            .unwrap_or(Value::Array(Vec::new())),
        origin: text("origin").unwrap_or_default(),
        chain_id: input
            .get("chainId")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        chain_name: text("chainName"),
        native_symbol: text("nativeSymbol"),
        account: text("account").unwrap_or_default(),
        account_name: text("accountName"),
        credential_ids_hex: &ids,
        user_op: op.as_ref(),
        calls: &calls,
    });
    Ok(built.to_string())
}

fn keys_of(keys_json: &str) -> Result<Vec<WalletKey>, JsValue> {
    let keys: Vec<Value> =
        serde_json::from_str(keys_json).map_err(|e| internal(format!("keys: {e}")))?;
    Ok(keys
        .iter()
        .map(|key| WalletKey {
            credential_id: key
                .get("credentialId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
            public_key_hex: key
                .get("publicKeyHex")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned(),
        })
        .collect())
}

fn verdict(result: Result<Value, ClearSignerError>, digest: &[u8], keys: &[WalletKey]) -> Value {
    let refused = |error: ClearSignerError| {
        let detail = match &error {
            ClearSignerError::Refused(code) => code.clone(),
            ClearSignerError::Malformed(detail) => detail.clone(),
            _ => String::new(),
        };
        json!({ "refused": { "code": error.code(), "detail": detail } })
    };
    match result.and_then(|result| clear_signer::verify(&result, digest, keys)) {
        Ok(verified) => json!({
            "accepted": {
                "credentialIdHex": verified.credential_id_hex,
                "signatureDer": vela_core::primitives::to_hex(&verified.signature_der, true),
                "authenticatorData": vela_core::primitives::to_hex(&verified.authenticator_data, true),
                "clientDataJSON": vela_core::primitives::to_hex(&verified.client_data_json, true),
            }
        }),
        Err(error) => refused(error),
    }
}

/// The page's answer judged against `digest` and the account's keys
/// (`[{credentialId, publicKeyHex}]`): `{accepted: {credentialIdHex,
/// signatureDer, authenticatorData, clientDataJSON}}` or `{refused: {code,
/// detail}}`.
#[wasm_bindgen(js_name = clearSignerVerify)]
pub fn clear_signer_verify(
    result_json: &str,
    digest: &[u8],
    keys_json: &str,
) -> Result<String, JsValue> {
    let keys = keys_of(keys_json)?;
    let result =
        serde_json::from_str(result_json).map_err(|e| ClearSignerError::Malformed(e.to_string()));
    Ok(verdict(result, digest, &keys).to_string())
}

/// One loopback WebSocket connection, exactly as the phones run it.
#[wasm_bindgen(js_name = ClearSignerWs)]
pub struct ClearSignerWs {
    inner: ws::Connection,
    digest: Vec<u8>,
    keys: Vec<WalletKey>,
}

#[wasm_bindgen(js_class = ClearSignerWs)]
impl ClearSignerWs {
    #[wasm_bindgen(constructor)]
    pub fn new(
        signer_url: &str,
        token: &str,
        id: &str,
        request_json: &str,
        digest: &[u8],
        keys_json: &str,
    ) -> Result<ClearSignerWs, JsValue> {
        let request: Value =
            serde_json::from_str(request_json).map_err(|e| internal(format!("request: {e}")))?;
        Ok(Self {
            inner: ws::Connection::new(signer_url, token, id, &request),
            digest: digest.to_vec(),
            keys: keys_of(keys_json)?,
        })
    }

    /// `{write: number[], close, outcome?}` — `outcome` as [`clear_signer_verify`].
    pub fn feed(&mut self, bytes: &[u8]) -> String {
        let step = self.inner.feed(bytes);
        let mut out = json!({ "write": step.write, "close": step.close });
        if let Some(outcome) = step.outcome {
            out["outcome"] = verdict(outcome, &self.digest, &self.keys);
        }
        out.to_string()
    }

    /// The socket closed: `"declined"` when a page that had the request went
    /// away, `""` when it never proved itself.
    pub fn closed(&mut self) -> String {
        self.inner
            .closed()
            .map(|error| error.code().to_owned())
            .unwrap_or_default()
    }
}
