//! The Trusted Signer (spec 071): what a clear-signing page receives, what it
//! answers, and the channels between — one implementation for every wallet.
//!
//! The page (`app-web/trusted-signer`) is the fourth way to sign. The three
//! others answer *where the passkey is*; this one answers *where the person
//! checks what they sign*: a separate, zero-dependency page that decodes the
//! request from the operation's own bytes, derives the digest itself, runs
//! the passkey ceremony in the browser and returns the assertion. The wallet
//! then trusts nothing it did not check:
//!
//! - [`request`] — the page's `{intent, context}`, built from what the shell
//!   already holds when it would sign: the method and params, the ASSEMBLED
//!   user operation (the digest covers that, not the site's call), the fee
//!   leg's index, the account and its credential ids.
//! - [`verify`] — the page's answer, accepted only when the client data is a
//!   `webauthn.get` over exactly the digest the WALLET computed, the user was
//!   verified, the credential is one of this wallet's, and the P-256 signature
//!   verifies under that credential's key. It returns the DER signature the
//!   existing Safe envelope takes, so nothing downstream changes.
//! - The channels: [`url_launch`] + [`parse_callback`] (URL fragment in,
//!   loopback redirect or beacon out — the desktop), [`ws_launch`] + [`ws`]
//!   (a WebSocket on the phone's own loopback, served byte for byte by this
//!   crate — the phones), per `app-web/trusted-signer/PROTOCOL.md`.
//!
//! Pure: no I/O, no clock, no randomness — the one-time token comes from the
//! shell.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::signature::hazmat::PrehashVerifier as _;
use p256::ecdsa::{Signature, VerifyingKey};
use serde_json::{json, Map, Value};

use crate::types::ClientDataKind;
use crate::user_op::{UserOperation, WalletKey};
use crate::webauthn::{validate_client_data, webauthn_signing_hash};

#[cfg(feature = "crux")]
pub mod ceremony;
/// Spec 076: whether the page may be opened at all — one decision, every shell.
pub mod integrity;
pub mod ws;

/// The official signer page (spec 071 [D]) — the host PROTOCOL.md names.
/// A person may point Settings at their own deployment.
pub const DEFAULT_SIGNER_URL: &str = "https://sign.getvela.app/";

/// Where the page sends a signature back to (spec 076, owner 2026-09-23:
/// 「我们在 android ios desktop(win linux mac) 都使用 custom schema」).
///
/// `velawallet://` is already registered by both phones (`CFBundleURLSchemes`,
/// the Android intent filter) for `pay` and `open`; this is a third shape on
/// the same door, so nothing new has to be claimed.
///
/// **Why a scheme and not the loopback callback it replaces.** Two measured
/// reasons, not a preference:
///
/// - the published page carries `default-src` `none` INSIDE its hashed bytes,
///   and a `WebSocket` from inside it cannot reach anything — measured, the
///   server saw no byte. A NAVIGATION is not governed by that CSP, also
///   measured: the page assigns `location.href` to a custom scheme with no
///   violation, and survives when nothing handles it.
/// - a phone app is suspended the moment the browser tab covers it, so it
///   cannot catch a navigation to `127.0.0.1`. The OS can still hand it a
///   scheme.
///
/// **What it does NOT give.** A custom scheme is not exclusive: any app may
/// register `velawallet://`, and which one wins is a collision on Android and
/// undefined on iOS. So this is a TRANSPORT and never an authorisation:
///
/// - integrity survives interception — [`verify`] accepts only a `webauthn.get`
///   over exactly the digest this wallet computed, by a credential it holds, so
///   a forged callback cannot be accepted;
/// - what interception costs is availability (the wallet waits) and
///   confidentiality (the interceptor learns the assertion and could front-run
///   submitting the person's own operation). Worth saying out loud rather than
///   implying the scheme is a boundary.
///
/// The one-time token travels with it and is checked by [`parse_callback`].
pub const CALLBACK_URL: &str = "velawallet://sign-result";

/// The "Sign with" value that routes a request to the Trusted Signer, next to
/// `platform` | `hybrid` | `security_key` (`wallet_keys::SIGN_METHODS`).
pub const METHOD: &str = "trusted_signer";

/// Why a signer page address cannot be used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignerUrlError {
    /// Not a URL at all.
    Invalid,
    /// Neither https nor a loopback http page: the browser will not run a
    /// passkey ceremony there (WebAuthn needs a secure context).
    Insecure,
}

/// A signer page address the person typed, normalised (trimmed, a scheme
/// added to a bare host, a trailing `/` on a bare origin) — or why it cannot
/// be used. https anywhere; http only on this device's own loopback.
pub fn signer_url(input: &str) -> Result<String, SignerUrlError> {
    let text = input.trim();
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return Err(SignerUrlError::Invalid);
    }
    let text = if text.contains("://") {
        text.to_owned()
    } else {
        format!("https://{text}")
    };
    let (scheme, rest) = text.split_once("://").ok_or(SignerUrlError::Invalid)?;
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host_of(authority);
    if host.is_empty() || authority.contains('@') {
        return Err(SignerUrlError::Invalid);
    }
    match scheme.to_ascii_lowercase().as_str() {
        "https" => {}
        "http" if is_loopback(&host) => {}
        "http" => return Err(SignerUrlError::Insecure),
        _ => return Err(SignerUrlError::Invalid),
    }
    let path = &rest[authority.len()..];
    let path = if path.is_empty() { "/" } else { path };
    Ok(format!(
        "{}://{}{}",
        scheme.to_ascii_lowercase(),
        authority.to_ascii_lowercase(),
        path
    ))
}

/// Whether a page at `url` can reach this wallet's passkeys. They were made
/// for `getvela.app`, and a browser only lets a page use a passkey made for
/// its own domain or a parent of it — a copy deployed anywhere else can show
/// a request but cannot sign it (Settings says so).
pub fn uses_wallet_passkeys(url: &str) -> bool {
    let Some((scheme, rest)) = url.trim().split_once("://") else {
        return false;
    };
    let host = host_of(rest.split(['/', '?', '#']).next().unwrap_or_default());
    scheme.eq_ignore_ascii_case("https")
        && (host == "getvela.app" || host.ends_with(".getvela.app"))
}

/// The relying party a key behind `signer_origin` belongs to — the rpId the
/// REGISTRY must be asked for that key's member challenge.
///
/// A page can only ever mint a passkey for its own domain (the browser
/// enforces it, and that rule is what makes a passkey belong to a site at
/// all). So a key created on `https://sign.getvela.app` is a `getvela.app`
/// key, and one created on a self-hosted page at `http://localhost:8140` is a
/// `localhost` key — whatever rpId the WALLET uses for its own ceremonies.
///
/// Asking the registry under the wallet's own rpId instead produced a
/// challenge the page could not have derived, and the wallet then refused its
/// own key's proof: "the Trusted Signer's answer does not match this request"
/// (found on the phone, 2026-09-22, and again over BLE the next day). The
/// page is right and the wallet was asking the wrong question.
///
/// `None` when the key lives on an authenticator this device reaches itself:
/// then the wallet's own rpId is the answer, as it always was.
#[must_use]
pub fn registry_rp_id(signer_origin: Option<&str>) -> Option<String> {
    let origin = signer_origin?.trim();
    if origin.is_empty() {
        return None;
    }
    let rest = origin.split_once("://").map_or(origin, |(_, rest)| rest);
    let host = host_of(rest.split(['/', '?', '#']).next().unwrap_or_default());
    if host.is_empty() {
        return None;
    }
    // The official page is a subdomain of the wallet's own relying party, and
    // a passkey made there belongs to the parent — `signer.js` resolves it the
    // same way, and the two must agree or nothing matches.
    if host == "getvela.app" || host.ends_with(".getvela.app") {
        return Some("getvela.app".to_owned());
    }
    Some(host)
}

/// The relying party a whole registry unit belongs to — or why the key set
/// cannot be published at all.
///
/// The contract stores ONE `rpId` per unit (`Unit { rpId, … }`, and it is
/// inside the `contentHash` the group proof signs), and each member's proof
/// carries `sha256(rpId)` inside authenticator data its OWN authenticator
/// produced. A key that belongs to another relying party therefore cannot
/// produce a proof this unit will accept — not by convention, by WebAuthn.
///
/// So a wallet's keys must share one relying party (ruling, 2026-09-23). A set
/// that does not is refused HERE, loudly, rather than published as a unit
/// nobody can ever prove membership of.
///
/// `member_origins` is each member's `signer_origin` in founding order — a key
/// on an authenticator this device reaches itself has none, and belongs to the
/// wallet's own relying party.
pub fn registry_unit_rp_id(
    member_origins: &[Option<String>],
    wallet_rp_id: &str,
) -> Result<String, Vec<String>> {
    let mut seen: Vec<String> = Vec::new();
    for origin in member_origins {
        let rp = registry_rp_id(origin.as_deref()).unwrap_or_else(|| wallet_rp_id.to_owned());
        if !seen.contains(&rp) {
            seen.push(rp);
        }
    }
    match seen.len() {
        0 => Ok(wallet_rp_id.to_owned()),
        1 => Ok(seen.remove(0)),
        _ => Err(seen),
    }
}

fn host_of(authority: &str) -> String {
    let authority = authority.to_ascii_lowercase();
    if let Some(v6) = authority.strip_prefix('[') {
        return v6
            .split(']')
            .next()
            .map(|h| format!("[{h}]"))
            .unwrap_or_default();
    }
    authority.split(':').next().unwrap_or_default().to_owned()
}

fn is_loopback(host: &str) -> bool {
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "[::1]"
        || host.starts_with("127.")
}

/// Why an answer from the page was not accepted, or what the page said.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TrustedSignerError {
    /// The answer is not the shape the protocol defines.
    #[error("the Trusted Signer's answer is malformed: {0}")]
    Malformed(String),
    /// Signed, but not over the digest this wallet computed — never submitted.
    #[error("the Trusted Signer signed something other than this request")]
    WrongChallenge,
    /// The user was not verified (no biometric / PIN).
    #[error("the passkey did not verify the user")]
    NotVerified,
    /// The key that answered is not one of this wallet's.
    #[error("the key that signed does not belong to this wallet")]
    ForeignKey,
    /// The signature does not verify under the wallet's key.
    #[error("the signature does not verify")]
    BadSignature,
    /// The page's rules refused the request (unlimited approval, operation
    /// mismatch, a method it cannot derive a digest for). Not the person.
    #[error("the Trusted Signer refused this request ({0})")]
    Refused(String),
    /// The person closed the page or declined.
    #[error("the Trusted Signer was closed without signing")]
    Declined,
    /// The answer came back with another request's token.
    #[error("the answer belongs to another request")]
    WrongToken,
}

impl TrustedSignerError {
    /// A stable name for the refusal, for shells that carry it as text (the
    /// web, logs): `declined`, `refused`, `wrong_challenge`, `foreign_key`,
    /// `bad_signature`, `not_verified`, `wrong_token`, `malformed`.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Malformed(_) => "malformed",
            Self::WrongChallenge => "wrong_challenge",
            Self::NotVerified => "not_verified",
            Self::ForeignKey => "foreign_key",
            Self::BadSignature => "bad_signature",
            Self::Refused(_) => "refused",
            Self::Declined => "declined",
            Self::WrongToken => "wrong_token",
        }
    }
}

/// What the shell hands over to build a request.
#[derive(Clone, Debug, Default)]
pub struct RequestInput<'a> {
    /// `eth_sendTransaction`, `wallet_sendCalls`, `personal_sign`,
    /// `eth_signTypedData_v4`, … — a site's own. EMPTY for the wallet's own
    /// send, which no site asked for: the intent is then `wallet_sendCalls`
    /// of [`Self::calls`] ([`own_send_params`]).
    pub method: &'a str,
    /// The JSON-RPC params, verbatim (unused when `method` is empty).
    pub params: Value,
    /// Who asked: the site's origin; empty for the wallet's own send.
    pub origin: &'a str,
    pub chain_id: u64,
    pub chain_name: Option<&'a str>,
    pub native_symbol: Option<&'a str>,
    /// The Safe the signature is for.
    pub account: &'a str,
    /// The account's name — it points the person at a passkey.
    pub account_name: Option<&'a str>,
    /// The account's credential ids, hex (every shell stores them so).
    pub credential_ids_hex: &'a [String],
    /// The assembled operation, for transactions. The digest covers THIS.
    pub user_op: Option<&'a UserOperation>,
    /// For a transaction, the calls the operation carries BEFORE its fee leg.
    /// The wallet appends the fee last on every chain, so the page is told
    /// the fee is leg `calls.len()`.
    pub calls: &'a [crate::user_op::MultiSendCall],
}

/// The page's `{intent, context}` (PROTOCOL.md §4, §8.1).
pub fn request(input: &RequestInput<'_>) -> Value {
    let mut context = Map::new();
    context.insert("chainId".into(), json!(input.chain_id));
    if let Some(name) = input.chain_name.filter(|n| !n.is_empty()) {
        context.insert("chainName".into(), json!(name));
    }
    if let Some(symbol) = input.native_symbol.filter(|s| !s.is_empty()) {
        context.insert("nativeSymbol".into(), json!(symbol));
    }
    context.insert("account".into(), json!(input.account));
    if let Some(name) = input.account_name.filter(|n| !n.is_empty()) {
        let letter: String = name
            .chars()
            .next()
            .map(|c| c.to_uppercase().collect())
            .unwrap_or_default();
        context.insert("signer".into(), json!({ "name": name, "letter": letter }));
    }
    let allow: Vec<Value> = input
        .credential_ids_hex
        .iter()
        .filter_map(|id| crate::primitives::from_hex(id).ok())
        .map(|bytes| json!(URL_SAFE_NO_PAD.encode(bytes)))
        .collect();
    if !allow.is_empty() {
        context.insert("allowCredentials".into(), Value::Array(allow));
    }
    if let Some(op) = input.user_op {
        let mut operation = Map::new();
        operation.insert("userOp".into(), user_op_json(op));
        if !input.calls.is_empty() {
            operation.insert("feeLegIndex".into(), json!(input.calls.len()));
        }
        operation.insert("entryPoint".into(), json!(crate::safe::ENTRY_POINT));
        operation.insert("module".into(), json!(crate::safe::SAFE_4337_MODULE));
        context.insert("operation".into(), Value::Object(operation));
    }
    let (method, params) = if input.method.is_empty() {
        (
            "wallet_sendCalls",
            own_send_params(input.chain_id, input.account, input.calls),
        )
    } else {
        (input.method, input.params.clone())
    };
    json!({
        "intent": { "method": method, "params": params, "origin": input.origin },
        "context": Value::Object(context),
    })
}

/// The intent for the wallet's OWN send, which no site asked for: the calls
/// the operation carries before its fee leg, as `wallet_sendCalls` (EIP-5792)
/// — the shape the page checks an operation's legs against. Its origin is
/// empty, which the page shows as the wallet itself.
pub fn own_send_params(
    chain_id: u64,
    account: &str,
    calls: &[crate::user_op::MultiSendCall],
) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|call| {
            let value = call
                .value_hex
                .trim_start_matches("0x")
                .trim_start_matches("0X");
            json!({
                "to": call.to,
                "value": format!("0x{}", if value.is_empty() { "0" } else { value }),
                "data": crate::primitives::to_hex(&call.data, true),
            })
        })
        .collect();
    json!([{
        "version": "1.0",
        "chainId": format!("0x{chain_id:x}"),
        "from": account,
        "calls": calls,
    }])
}

/// The operation in the page's field names (`lib/safeop.js`), gas figures as
/// decimal strings (`BigInt` reads them), bytes as `0x` hex.
fn user_op_json(op: &UserOperation) -> Value {
    json!({
        "sender": op.sender,
        "nonce": op.nonce,
        "initCode": crate::primitives::to_hex(&op.init_code, true),
        "callData": crate::primitives::to_hex(&op.call_data, true),
        "verificationGasLimit": op.verification_gas_limit.to_string(),
        "callGasLimit": op.call_gas_limit.to_string(),
        "preVerificationGas": op.pre_verification_gas.to_string(),
        "maxFeePerGas": op.max_fee_per_gas.to_string(),
        "maxPriorityFeePerGas": op.max_priority_fee_per_gas.to_string(),
        "paymasterAndData": crate::primitives::to_hex(&op.paymaster_and_data, true),
    })
}

/// The inverse of the request's `userOp`: an operation in the page's field
/// names (gas as decimal or `0x` strings, or numbers), for shells that hold
/// the operation as JSON (the web).
pub fn user_op_from_json(value: &Value) -> Option<UserOperation> {
    let text = |name: &str| value.get(name).and_then(Value::as_str);
    let bytes = |name: &str| match text(name) {
        None | Some("") | Some("0x") => Some(Vec::new()),
        Some(hex) => crate::primitives::from_hex(hex).ok(),
    };
    let gas = |name: &str| -> Option<u128> {
        match value.get(name)? {
            Value::Number(n) => n.as_u64().map(u128::from),
            Value::String(s) => match s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
                Some(hex) => u128::from_str_radix(if hex.is_empty() { "0" } else { hex }, 16).ok(),
                None => s.parse().ok(),
            },
            _ => None,
        }
    };
    Some(UserOperation {
        sender: text("sender")?.to_owned(),
        nonce: text("nonce")?.to_owned(),
        init_code: bytes("initCode")?,
        call_data: bytes("callData")?,
        verification_gas_limit: gas("verificationGasLimit")?,
        call_gas_limit: gas("callGasLimit")?,
        pre_verification_gas: gas("preVerificationGas")?,
        max_fee_per_gas: gas("maxFeePerGas")?,
        max_priority_fee_per_gas: gas("maxPriorityFeePerGas")?,
        paymaster_and_data: bytes("paymasterAndData")?,
        signature: Vec::new(),
    })
}

/// An assertion the wallet accepted, in the shapes the Safe envelope takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verified {
    /// Hex, no `0x` — how every shell stores credential ids.
    pub credential_id_hex: String,
    pub signature_der: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub client_data_json: Vec<u8>,
}

/// Accept the page's result (`{credentialId, signature, authenticatorData,
/// clientDataJSON}`: base64url id, raw `r‖s` hex, hex bytes) only if it is a
/// user-verified `webauthn.get` over exactly `digest`, by one of `keys`, with
/// a signature that verifies under that key.
pub fn verify(
    result: &Value,
    digest: &[u8],
    keys: &[WalletKey],
) -> Result<Verified, TrustedSignerError> {
    let field = |name: &str| {
        result
            .get(name)
            .and_then(Value::as_str)
            .ok_or_else(|| TrustedSignerError::Malformed(format!("missing {name}")))
    };
    let hex_field = |name: &str| {
        field(name).and_then(|text| {
            crate::primitives::from_hex(text)
                .map_err(|_| TrustedSignerError::Malformed(format!("{name} is not hex")))
        })
    };
    let credential = URL_SAFE_NO_PAD
        .decode(field("credentialId")?.trim_end_matches('='))
        .map_err(|_| TrustedSignerError::Malformed("credentialId is not base64url".into()))?;
    let raw_signature = hex_field("signature")?;
    let authenticator_data = hex_field("authenticatorData")?;
    let client_data_json = hex_field("clientDataJSON")?;

    // The Safe verifier's own byte rules, then the one it cannot check: that
    // the challenge IS the digest this wallet computed.
    validate_client_data(ClientDataKind::Get, &client_data_json, &authenticator_data).map_err(
        |error| {
            if error.to_string().contains("User Verification") {
                TrustedSignerError::NotVerified
            } else {
                TrustedSignerError::Malformed(error.to_string())
            }
        },
    )?;
    let client: Value = serde_json::from_slice(&client_data_json)
        .map_err(|_| TrustedSignerError::Malformed("clientDataJSON is not JSON".into()))?;
    let challenge = client
        .get("challenge")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if challenge != URL_SAFE_NO_PAD.encode(digest) {
        return Err(TrustedSignerError::WrongChallenge);
    }

    let credential_hex = crate::primitives::to_hex(&credential, false).to_ascii_lowercase();
    let key = keys
        .iter()
        .find(|key| {
            key.credential_id
                .trim_start_matches("0x")
                .eq_ignore_ascii_case(&credential_hex)
        })
        .ok_or(TrustedSignerError::ForeignKey)?;
    let point = crate::primitives::from_hex(&key.public_key_hex)
        .map_err(|_| TrustedSignerError::Malformed("wallet key is not hex".into()))?;
    let verifying = VerifyingKey::from_sec1_bytes(&point)
        .map_err(|_| TrustedSignerError::Malformed("wallet key is not a P-256 point".into()))?;
    if raw_signature.len() != 64 {
        return Err(TrustedSignerError::Malformed("signature is not r‖s".into()));
    }
    let signature =
        Signature::from_slice(&raw_signature).map_err(|_| TrustedSignerError::BadSignature)?;
    let signature = signature.normalize_s();
    let prehash = webauthn_signing_hash(&authenticator_data, &client_data_json);
    verifying
        .verify_prehash(&prehash, &signature)
        .map_err(|_| TrustedSignerError::BadSignature)?;
    Ok(Verified {
        credential_id_hex: credential_hex,
        signature_der: signature.to_der().as_bytes().to_vec(),
        authenticator_data,
        client_data_json,
    })
}

// ---------------------------------------------------------------------------
// Channels
// ---------------------------------------------------------------------------

/// `<base>sign.html?<query>` — the base may or may not end in `/`, or already
/// name `sign.html`.
fn sign_page(base: &str, query: &str) -> String {
    let trimmed = base.trim();
    let page = if trimmed.ends_with(".html") {
        trimmed.to_owned()
    } else if trimmed.ends_with('/') {
        format!("{trimmed}sign.html")
    } else {
        format!("{trimmed}/sign.html")
    };
    format!("{page}?{query}")
}

/// URL fragment + loopback callback (PROTOCOL.md §7.2): the request deflated
/// (`z=1`) and base64url'd into the fragment, which never reaches a server
/// and which the page wipes from history at once.
pub fn url_launch(base: &str, request: &Value, callback: &str, token: &str) -> String {
    let deflated = miniz_oxide::deflate::compress_to_vec(request.to_string().as_bytes(), 9);
    format!(
        "{}#i={}&cb={}&t={}&z=1",
        sign_page(base, "ch=url"),
        URL_SAFE_NO_PAD.encode(deflated),
        URL_SAFE_NO_PAD.encode(callback.as_bytes()),
        percent(token),
    )
}

/// The path the loopback callback is served on — `url_launch`'s `callback`
/// is `http://127.0.0.1:<port>` + this.
pub const CALLBACK_PATH: &str = "/vela";

/// The query of a loopback callback request — `GET /vela?t=…&result=…` (the
/// page navigating to it) or `POST /vela?t=…&error=…` (its beacon while the
/// tab closes) — or `None` for anything else a browser asks a listener for
/// (`/favicon.ico`).
pub fn callback_query(request_head: &str) -> Option<&str> {
    let mut parts = request_head.lines().next()?.split(' ');
    let method = parts.next()?;
    if method != "GET" && method != "POST" {
        return None;
    }
    let target = parts.next()?;
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    (path == CALLBACK_PATH).then_some(query)
}

/// The one-time token a [`CALLBACK_URL`] carries, or `None` when the URL is
/// not one of this wallet's callbacks at all.
///
/// **Why this is the core's and not each shell's.** Every shell receives the
/// callback somewhere different — `onNewIntent`, `onOpenURL`, an `argv` scan,
/// an `on_open_urls` handler — and each of them needs the same answer before
/// it can do anything: *which pending request is this for?* Written three
/// times it is three chances to accept `velawallet://sign-resultXXX`, or to
/// read a `t=` out of some other link's query. Written once it is one rule
/// with one set of tests, and a shell that gets `None` knows to drop the URL
/// in silence.
///
/// This deliberately does NOT say the callback is good: the token may belong
/// to no attempt, and the answer still has to survive [`parse_callback`] and
/// then [`verify`].
#[must_use]
pub fn callback_token(url: &str) -> Option<String> {
    callback_of(url)?
        .split('&')
        .find_map(|pair| pair.strip_prefix("t="))
        .map(unpercent)
        .filter(|token| !token.is_empty())
}

/// The query of a [`CALLBACK_URL`], for [`parse_callback`] — `None` when the
/// URL is not a callback.
///
/// The page navigates to `velawallet://sign-result?…`, but Windows hands the
/// app `velawallet://sign-result/?…`: it gives the host an empty path, as it
/// does for any URL with an authority. Measured on Windows 11 with the
/// published page, where the token matched and the answer was dropped for the
/// slash alone. That one slash is the only other spelling accepted.
#[must_use]
pub fn callback_of(url: &str) -> Option<&str> {
    let rest = url.strip_prefix(CALLBACK_URL)?;
    rest.strip_prefix('?').or_else(|| rest.strip_prefix("/?"))
}

/// What the callback carried: `?t=<token>&result=<b64url json>` or
/// `?t=<token>&error=<code>`. The token must be this request's.
pub fn parse_callback(query: &str, token: &str) -> Result<Value, TrustedSignerError> {
    let mut got_token = None;
    let mut result = None;
    let mut error = None;
    for pair in query.trim_start_matches('?').split('&') {
        let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
        let value = unpercent(value);
        match key {
            "t" => got_token = Some(value),
            "result" => result = Some(value),
            "error" => error = Some(value),
            _ => {}
        }
    }
    if got_token.as_deref() != Some(token) {
        return Err(TrustedSignerError::WrongToken);
    }
    if let Some(code) = error {
        return Err(refusal(&code));
    }
    let encoded = result.ok_or_else(|| TrustedSignerError::Malformed("no result".into()))?;
    let bytes = URL_SAFE_NO_PAD
        .decode(encoded.trim_end_matches('='))
        .map_err(|_| TrustedSignerError::Malformed("result is not base64url".into()))?;
    serde_json::from_slice(&bytes)
        .map_err(|_| TrustedSignerError::Malformed("result is not JSON".into()))
}

/// The page, told to connect to the wallet's loopback WebSocket (spec 071):
/// `sign.html?ch=ws#p=<port>&t=<token>`.
pub fn ws_launch(base: &str, port: u16, token: &str) -> String {
    format!("{}#p={port}&t={}", sign_page(base, "ch=ws"), percent(token))
}

/// `user_rejected` (the person) vs everything else (the page's rules).
pub(crate) fn refusal(code: &str) -> TrustedSignerError {
    match code {
        "user_rejected" | "" => TrustedSignerError::Declined,
        other => TrustedSignerError::Refused(other.to_owned()),
    }
}

fn percent(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn unpercent(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let decoded = std::str::from_utf8(&bytes[i + 1..i + 3])
                    .ok()
                    .and_then(|hex| u8::from_str_radix(hex, 16).ok());
                match decoded {
                    Some(byte) => {
                        out.push(byte);
                        i += 3;
                    }
                    None => {
                        out.push(b'%');
                        i += 1;
                    }
                }
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            other => {
                out.push(other);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {

    /// The custom-scheme callback goes out in the launch URL and comes back
    /// through `parse_callback` — one round trip, so the two halves cannot
    /// drift apart while each looks fine on its own.
    #[test]
    fn a_custom_scheme_callback_survives_the_round_trip() {
        let request = json!({ "intent": { "kind": "sign" } });
        let launch = url_launch("https://sign.getvela.app/", &request, CALLBACK_URL, "tok-1");
        // The callback rides in the FRAGMENT, base64url, never in a query: a
        // query is sent to the server and logged there, and the whole point is
        // that the server never sees what is being signed.
        assert!(launch.contains("#i="), "{launch}");
        assert!(
            !launch.contains("?i="),
            "the payload must not be in the query"
        );
        let encoded = URL_SAFE_NO_PAD.encode(CALLBACK_URL.as_bytes());
        assert!(launch.contains(&format!("cb={encoded}")), "{launch}");

        // What the OS hands the app back, as the page would build it.
        let answer = json!({ "signature": "0x30" });
        let payload = URL_SAFE_NO_PAD.encode(answer.to_string().as_bytes());
        let query = format!("t=tok-1&result={payload}");
        assert_eq!(parse_callback(&query, "tok-1").ok(), Some(answer));

        // Another app that registered the same scheme cannot replay it: the
        // token is this attempt's.
        assert!(matches!(
            parse_callback(&query, "tok-2"),
            Err(TrustedSignerError::WrongToken)
        ));
    }

    /// Which pending request a callback belongs to — the question every shell
    /// asks first, answered in one place so three shells cannot each answer it
    /// slightly differently.
    #[test]
    fn a_callback_names_the_attempt_it_belongs_to_and_nothing_else_does() {
        let callback = format!("{CALLBACK_URL}?t=tok-1&result=e30");
        assert_eq!(callback_token(&callback).as_deref(), Some("tok-1"));
        assert_eq!(callback_of(&callback), Some("t=tok-1&result=e30"));

        // The token is percent-decoded, because that is how it was written.
        assert_eq!(
            callback_token(&format!("{CALLBACK_URL}?t=a%2Fb&error=x")).as_deref(),
            Some("a/b")
        );

        // Order does not matter: `t` is found wherever the page put it.
        assert_eq!(
            callback_token(&format!("{CALLBACK_URL}?error=user_rejected&t=tok-9")).as_deref(),
            Some("tok-9")
        );

        // What Windows actually hands the app: the same URL with an empty path.
        let windows = format!("{CALLBACK_URL}/?t=tok-1&result=e30");
        assert_eq!(callback_token(&windows).as_deref(), Some("tok-1"));
        assert_eq!(callback_of(&windows), Some("t=tok-1&result=e30"));

        // Everything a shell must drop in SILENCE. The prefix check is on the
        // whole URL, so a host that merely starts the same way is not ours —
        // which is the one a naive `starts_with` gets wrong.
        for other in [
            "velawallet://pay?to=0x1&t=tok-1",
            "velawallet://sign-results?t=tok-1",
            "velawallet://sign-result",
            "velawallet://sign-result/",
            "velawallet://sign-result/x?t=tok-1",
            "velawallet://sign-result//?t=tok-1",
            "https://sign.getvela.app/?t=tok-1",
            "",
        ] {
            assert_eq!(callback_token(other), None, "{other} is not a callback");
            assert_eq!(callback_of(other), None, "{other} is not a callback");
        }

        // A callback with no token names no attempt, so it reaches none.
        assert_eq!(callback_token(&format!("{CALLBACK_URL}?result=e30")), None);
        assert_eq!(callback_token(&format!("{CALLBACK_URL}?t=")), None);
    }
    use super::*;

    /// The rpId the registry must be asked for a key that lives behind a page:
    /// the page's own domain, because that is the only one it could have
    /// minted the key for.
    #[test]
    fn a_key_behind_a_page_belongs_to_that_pages_domain() {
        assert_eq!(
            registry_rp_id(None),
            None,
            "an ordinary key: the wallet's own"
        );
        assert_eq!(registry_rp_id(Some("")), None);
        assert_eq!(
            registry_rp_id(Some("https://sign.getvela.app")).as_deref(),
            Some("getvela.app"),
            "the official page is a subdomain; signer.js resolves it to the parent too"
        );
        assert_eq!(
            registry_rp_id(Some("https://getvela.app/sign.html")).as_deref(),
            Some("getvela.app")
        );
        assert_eq!(
            registry_rp_id(Some("http://localhost:8140")).as_deref(),
            Some("localhost"),
            "a self-hosted page: its own host, the port dropped"
        );
        assert_eq!(
            registry_rp_id(Some("https://sign.example.com/x?y#z")).as_deref(),
            Some("sign.example.com"),
            "not a getvela.app suffix: no promotion to a parent"
        );
        assert_eq!(
            registry_rp_id(Some("https://notgetvela.app")).as_deref(),
            Some("notgetvela.app"),
            "a lookalike is not a subdomain"
        );
    }

    /// A unit carries one relying party, and every member's proof is signed
    /// under its own. A set that disagrees cannot be proved, so it is refused
    /// before it is written rather than after (ruling, 2026-09-23).
    #[test]
    fn a_unit_takes_one_relying_party_or_none() {
        let app = "getvela.app";
        // Keys on authenticators this device reaches: the wallet's own.
        assert_eq!(registry_unit_rp_id(&[None, None], app).as_deref(), Ok(app));
        assert_eq!(registry_unit_rp_id(&[], app).as_deref(), Ok(app));
        // The official page resolves to the same relying party, so a page key
        // and a platform key CAN share a unit — which is what makes the
        // shipped flow work.
        assert_eq!(
            registry_unit_rp_id(&[Some("https://sign.getvela.app".to_owned()), None], app)
                .as_deref(),
            Ok(app)
        );
        // A self-hosted page: the whole set is that domain's.
        assert_eq!(
            registry_unit_rp_id(
                &[
                    Some("http://localhost:8140".to_owned()),
                    Some("http://localhost:8140/sign.html".to_owned())
                ],
                app
            )
            .as_deref(),
            Ok("localhost")
        );
        // Mixed: refused, and it says which relying parties it found.
        let mixed = registry_unit_rp_id(&[Some("http://localhost:8140".to_owned()), None], app);
        assert_eq!(mixed, Err(vec!["localhost".to_owned(), app.to_owned()]));
    }
}
