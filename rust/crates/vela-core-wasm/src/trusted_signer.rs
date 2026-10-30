//! What the web wallet and the Chrome extension need of the signing page: the
//! REGISTRY's relying party (spec 075), and — since the owner made the page
//! usable everywhere (2026-10-30, superseding 2026-09-23's "web 就不支持可信签名器
//! 好了" and its multisig-only exception) — every hand-off an app makes: a
//! transaction, a batch, the wallet's own send, a message
//! (`trustedSignerSignatureRequest` / `…Accept`) and a multisig member's
//! approval (`trustedSignerApprovalRequest` / `…Accept`), on a page checked as
//! the apps check it (`trustedSignerPageTarget` / `…Admit`); where each goes is
//! `signingRoute`.
//!
//! A wallet whose keys were minted on a page is published or read from here,
//! and a member minted on a page proves under THAT page's domain. Getting the
//! relying party wrong is how both phones hit 「可信签名器的回复与这笔请求
//! 不符」, so the rule stays where every shell can reach it. Spec 102: whether
//! a page can reach an account's keys at all is `signingVenueBlock`.

use vela_core::trusted_signer;
use wasm_bindgen::prelude::*;

/// The relying party a key minted behind `signerOrigin` belongs to, or `null`
/// for a key this wallet's own authenticators made (spec 075).
///
/// A key made on a Trusted Signer page is signed under THAT page's domain, so a
/// challenge fetched under the wallet's own would never match the answer —
/// 「可信签名器的回复与这笔请求不符」, which is how both phones found this.
#[wasm_bindgen(js_name = trustedSignerRegistryRpId)]
pub fn trusted_signer_registry_rp_id(signer_origin: Option<String>) -> Option<String> {
    trusted_signer::registry_rp_id(signer_origin.as_deref())
}

/// The ONE relying party a unit is filed under, or an error naming the parties
/// found when its members do not agree (ruling, 2026-09-23).
///
/// The registry stores a single `rpId` per unit and every member proves under
/// its own, so a mixed set is refused here rather than written and never
/// provable.
/// `member_origins_json` is a JSON array of each member's signer origin, with
/// `null` for a key the wallet's own authenticators made — wasm-bindgen has no
/// `Vec<Option<String>>`, and the phones pass the same shape.
#[wasm_bindgen(js_name = trustedSignerUnitRpId)]
pub fn trusted_signer_unit_rp_id(
    member_origins_json: &str,
    wallet_rp_id: &str,
) -> Result<String, JsValue> {
    let origins: Vec<Option<String>> = serde_json::from_str(member_origins_json)
        .map_err(|error| JsValue::from_str(&error.to_string()))?;
    trusted_signer::registry_unit_rp_id(&origins, wallet_rp_id)
        .map_err(|found| JsValue::from_str(&found.join(", ")))
}

/// The corpus key of the title a key ceremony on a self-hosted page is drawn
/// under while it waits (spec 102) — `componentsUi.signing.ceremonyCreate`,
/// `.ceremonySignIn` or `.ceremonyConfirm` — or `null` for an operation that is
/// not a ceremony. The web runs none; its gallery draws the apps' card.
#[wasm_bindgen(js_name = trustedSignerCeremonyTitleKey)]
#[must_use]
pub fn trusted_signer_ceremony_title_key(operation_json: &str) -> Option<String> {
    trusted_signer::ceremony::Ceremony::from_json(operation_json)
        .map(|ceremony| ceremony.title_key().to_owned())
}

/// The key row of a ceremony's card, as JSON `KeyLabel`
/// (`{place_key, label_key}`): "New key on | This device" while a key is made,
/// "Confirm with | USB security key" when one signs in or proves — the page's
/// own row. `null` for an operation that is not a ceremony.
#[wasm_bindgen(js_name = trustedSignerCeremonyKeyLabel)]
#[must_use]
pub fn trusted_signer_ceremony_key_label(operation_json: &str) -> Option<String> {
    trusted_signer::ceremony::Ceremony::from_json(operation_json)
        .and_then(|ceremony| serde_json::to_string(&ceremony.key_label()).ok())
}

// ---------------------------------------------------------------------------
// A multisig member's approval on the page (2026-10-29)
//
// A member whose account reviews and signs on a page approves there
// (`trusted_signer::approval`). The web checks the page
// (`launch::target` + `launch::admit`, as every app does), opens the checked
// version, hands the request over `postMessage`, and accepts the answer only
// as the core accepts it.
// ---------------------------------------------------------------------------

use vela_core::trusted_signer::{approval, integrity, launch};

fn js(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// What `trustedSignerApprovalRequest` reads (the web's names).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalRequestArgs {
    chain_id: u64,
    #[serde(default)]
    chain_name: Option<String>,
    #[serde(default)]
    native_symbol: Option<String>,
    multisig: String,
    #[serde(default)]
    multisig_name: Option<String>,
    tx: vela_core::multisig::SafeTx,
    member: String,
    #[serde(default)]
    member_name: Option<String>,
    #[serde(default)]
    credential_ids_hex: Vec<String>,
    #[serde(default)]
    key_route: Option<vela_core::signing_venue::KeyRoute>,
}

/// The page's `{intent, context}` for a member's approval of a SafeTx
/// (`trusted_signer::approval::request`): the method, the raw SafeTx, the
/// core's safeTxHash, the member, its keys and route, the multisig's name.
#[wasm_bindgen(js_name = trustedSignerApprovalRequest)]
pub fn trusted_signer_approval_request(args_json: &str) -> Result<String, JsValue> {
    let a: ApprovalRequestArgs = serde_json::from_str(args_json).map_err(js)?;
    let request = approval::request(&approval::ApprovalInput {
        chain_id: a.chain_id,
        chain_name: a.chain_name.as_deref(),
        native_symbol: a.native_symbol.as_deref(),
        multisig: &a.multisig,
        multisig_name: a.multisig_name.as_deref(),
        tx: &a.tx,
        member: &a.member,
        member_name: a.member_name.as_deref(),
        credential_ids_hex: &a.credential_ids_hex,
        key_route: a.key_route.as_ref(),
    })
    .map_err(js)?;
    Ok(request.to_string())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct FoundingKey {
    credential_id: String,
    public_key_hex: String,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApprovalAcceptArgs {
    result: serde_json::Value,
    chain_id: u64,
    member: String,
    safe_tx_hash: String,
    keys: Vec<FoundingKey>,
}

/// The page's answer as the member's approval (`PasskeyApproval`, JSON), or
/// an error whose message is the refusal's code (`wrong_challenge`,
/// `foreign_key`, `bad_signature`, `not_verified`, `malformed`) — accepted
/// only as `trusted_signer::approval::accept` accepts it.
#[wasm_bindgen(js_name = trustedSignerApprovalAccept)]
pub fn trusted_signer_approval_accept(args_json: &str) -> Result<String, JsValue> {
    let a: ApprovalAcceptArgs = serde_json::from_str(args_json).map_err(js)?;
    let keys: Vec<vela_core::user_op::WalletKey> = a
        .keys
        .into_iter()
        .map(|k| vela_core::user_op::WalletKey {
            credential_id: k.credential_id,
            public_key_hex: k.public_key_hex,
        })
        .collect();
    let approval = approval::accept(&a.result, a.chain_id, &a.member, &a.safe_tx_hash, &keys)
        .map_err(|error| JsValue::from_str(error.code()))?;
    serde_json::to_string(&approval).map_err(js)
}

// ---------------------------------------------------------------------------
// A transaction, a batch, the wallet's own send or a message on the page
// (2026-10-30)
// ---------------------------------------------------------------------------

/// One call of an operation, as the web's `OperationToSign.calls` carries it:
/// `value` in base units (decimal or `0x` hex), `data` `0x` hex.
#[derive(serde::Deserialize)]
struct CallArgs {
    to: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    data: String,
}

/// What `trustedSignerSignatureRequest` reads (the web's names).
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureRequestArgs {
    /// `""` for the wallet's own send.
    #[serde(default)]
    method: String,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    origin: String,
    chain_id: u64,
    #[serde(default)]
    chain_name: Option<String>,
    #[serde(default)]
    native_symbol: Option<String>,
    account: String,
    #[serde(default)]
    account_name: Option<String>,
    #[serde(default)]
    credential_ids_hex: Vec<String>,
    #[serde(default)]
    key_route: Option<vela_core::signing_venue::KeyRoute>,
    /// The assembled operation in the page's field names, for a transaction.
    #[serde(default)]
    user_op: Option<serde_json::Value>,
    /// The operation's calls before its fee leg.
    #[serde(default)]
    calls: Vec<CallArgs>,
    #[serde(default)]
    origin_seen_by_browser: bool,
}

fn hex_quantity(value: &str) -> String {
    let value = value.trim();
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return format!("0x{}", if hex.is_empty() { "0" } else { hex });
    }
    match value.parse::<u128>() {
        Ok(n) => format!("0x{n:x}"),
        Err(_) => "0x0".to_owned(),
    }
}

/// The page's `{intent, context}` for a transaction, a batch, the wallet's own
/// send (`method: ""`) or a message (`trusted_signer::request`): the method and
/// params, the ASSEMBLED operation (the SafeOp hash covers that, not the
/// site's call) with its fee leg's index, the account, its key and where it
/// lives. An error for an operation that does not read.
#[wasm_bindgen(js_name = trustedSignerSignatureRequest)]
pub fn trusted_signer_signature_request(args_json: &str) -> Result<String, JsValue> {
    let a: SignatureRequestArgs = serde_json::from_str(args_json).map_err(js)?;
    let op = match &a.user_op {
        Some(value) => Some(
            trusted_signer::user_op_from_json(value)
                .ok_or_else(|| JsValue::from_str("the operation does not read"))?,
        ),
        None => None,
    };
    let calls = a
        .calls
        .iter()
        .map(|call| {
            Ok(vela_core::user_op::MultiSendCall {
                to: call.to.clone(),
                value_hex: hex_quantity(&call.value),
                data: match call.data.trim() {
                    "" | "0x" => Vec::new(),
                    hex => vela_core::primitives::from_hex(hex).map_err(js)?,
                },
            })
        })
        .collect::<Result<Vec<_>, JsValue>>()?;
    // A site's operation is signed as the site asked it, under its own method;
    // the wallet's own send (and a multisig executor's operation, which no
    // site asked for either) as its calls.
    let method = if a.method == vela_core::signing_venue::MULTISIG_EXECUTE_METHOD {
        ""
    } else {
        a.method.as_str()
    };
    // A multisig executor's operation: what its bytes do not carry — the
    // number the transaction runs at, the safeTxHash there, the multisig's
    // name and threshold — for the page to check and draw as the app's
    // (`approval::execution_context`, round 7). Params that name a multisig
    // must read; ones from a shell that names none add nothing.
    let execution = if a.method == vela_core::signing_venue::MULTISIG_EXECUTE_METHOD
        && a.params.get("multisig").is_some()
    {
        let p: ExecutionParams = serde_json::from_value(a.params.clone()).map_err(js)?;
        Some(
            trusted_signer::approval::execution_context(
                a.chain_id,
                &p.multisig,
                &p.tx,
                p.name.as_deref(),
                p.threshold,
            )
            .map_err(js)?,
        )
    } else {
        None
    };
    let mut request = trusted_signer::request(&trusted_signer::RequestInput {
        method,
        params: a.params.clone(),
        origin: &a.origin,
        chain_id: a.chain_id,
        chain_name: a.chain_name.as_deref(),
        native_symbol: a.native_symbol.as_deref(),
        account: &a.account,
        account_name: a.account_name.as_deref(),
        credential_ids_hex: &a.credential_ids_hex,
        key_route: a.key_route.as_ref(),
        user_op: op.as_ref(),
        calls: &calls,
        origin_seen_by_browser: a.origin_seen_by_browser,
    });
    if let (Some(about), Some(context)) = (
        execution,
        request.get_mut("context").and_then(|c| c.as_object_mut()),
    ) {
        context.insert("multisig".into(), about);
    }
    Ok(request.to_string())
}

/// A multisig executor's params (`vela_multisigExecute`): the SafeTx it
/// executes, on which multisig, and what this device knows of it.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionParams {
    tx: vela_core::multisig::SafeTx,
    multisig: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    threshold: Option<u64>,
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct SignatureAcceptArgs {
    /// The `{intent, context}` the page was handed.
    request: serde_json::Value,
    /// The page's answer.
    result: serde_json::Value,
    /// The account's founding keys.
    keys: Vec<FoundingKey>,
    /// The bytes the shell is about to sign with the answer, hex.
    digest_hex: String,
}

/// The page's answer to a signature request, as the shell's passkey
/// assertion (`{credentialId, signatureHex, authenticatorDataHex,
/// clientDataJSONHex}`: hex, the signature DER as WebAuthn gives it) — or an
/// error whose message is the refusal's code (`wrong_challenge`,
/// `foreign_key`, `bad_signature`, `not_verified`, `refused`, `malformed`).
/// Accepted only as `trusted_signer::accept_signature` accepts it: over
/// exactly the bytes the core computes from the request (which must be the
/// bytes the shell signs, `digestHex`), by one of the account's keys,
/// user-verified, under the Safe verifier's client-data rules.
#[wasm_bindgen(js_name = trustedSignerSignatureAccept)]
pub fn trusted_signer_signature_accept(args_json: &str) -> Result<String, JsValue> {
    let a: SignatureAcceptArgs = serde_json::from_str(args_json).map_err(js)?;
    let keys: Vec<vela_core::user_op::WalletKey> = a
        .keys
        .into_iter()
        .map(|k| vela_core::user_op::WalletKey {
            credential_id: k.credential_id,
            public_key_hex: k.public_key_hex,
        })
        .collect();
    let digest = vela_core::primitives::from_hex(&a.digest_hex)
        .map_err(|_| JsValue::from_str("malformed"))?;
    let verified = trusted_signer::accept_signature(&a.request, &a.result, &keys, &digest)
        .map_err(|error| JsValue::from_str(error.code()))?;
    let hex = |bytes: &[u8]| vela_core::primitives::to_hex(bytes, false);
    Ok(serde_json::json!({
        "credentialId": verified.credential_id_hex,
        "signatureHex": hex(&verified.signature_der),
        "authenticatorDataHex": hex(&verified.authenticator_data),
        "clientDataJSONHex": hex(&verified.client_data_json),
    })
    .to_string())
}

/// Where a multisig member's approval goes — `signingRoute` for
/// `vela_multisigApprove` (`trusted_signer::approval::approval_route`): `{type: 'in_vela'}`,
/// `{type: 'page', url}` or `{type: 'blocked', block}` — `null` for a record
/// this build cannot read. `account_json` is the member's record as stored.
#[wasm_bindgen(js_name = multisigApprovalRoute)]
#[must_use]
pub fn multisig_approval_route(account_json: &str) -> Option<String> {
    approval::approval_route_json(account_json)
}

/// The versions a deployment's `index.json` lists — `None` for a body that is
/// not `{"versions": [...]}`.
fn index_versions(index_json: Option<&str>) -> Option<Vec<String>> {
    let value: serde_json::Value = serde_json::from_str(index_json?).ok()?;
    value
        .get("versions")?
        .as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

fn trusted_list(trusted_json: &str) -> Vec<String> {
    serde_json::from_str(trusted_json).unwrap_or_default()
}

/// R6 for the web: which version of the page at `base` to check and open
/// (`launch::target`) — `{url, version, shortVersion}`, or an error naming
/// why there is none (`nothing_published` | `all_blocked`). `index_json` is
/// the body of the deployment's `index.json`, `null` when it could not be
/// read; `trusted_json` the versions the person trusted for this page.
#[wasm_bindgen(js_name = trustedSignerPageTarget)]
pub fn trusted_signer_page_target(
    base: &str,
    index_json: Option<String>,
    trusted_json: &str,
) -> Result<String, JsValue> {
    let index = index_versions(index_json.as_deref());
    let target = launch::target(base, index.as_deref(), &trusted_list(trusted_json), &[]).map_err(
        |why| {
            JsValue::from_str(match why {
                integrity::NoVersion::NothingPublishedThisWalletKnows => "nothing_published",
                integrity::NoVersion::EverythingUsableIsBlocked => "all_blocked",
            })
        },
    )?;
    Ok(serde_json::json!({
        "url": target.url(),
        "version": target.version(),
        "shortVersion": target.short_version(),
    })
    .to_string())
}

/// R6's ruling for the web on the bytes `trustedSignerPageTarget`'s URL
/// served (`launch::admit`): `{line, launch, versionToTrust}` — the integrity
/// line (`IntegrityLine`), the address to open with the request handed over
/// by `postMessage` (`CheckedPage::post_launch`; `null` unless the page was
/// admitted and the check is fresh), and the version a self-hosted page asks
/// to be trusted. `observed` is the sha256 of the body (`null` when it could
/// not be fetched); `now_ms` the shell's clock.
#[wasm_bindgen(js_name = trustedSignerPageAdmit)]
#[must_use]
pub fn trusted_signer_page_admit(
    base: &str,
    index_json: Option<String>,
    trusted_json: &str,
    observed: Option<String>,
    now_ms: f64,
    lang: &str,
) -> String {
    let index = index_versions(index_json.as_deref());
    let trusted = trusted_list(trusted_json);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let now = if now_ms.is_finite() && now_ms > 0.0 {
        now_ms as u64
    } else {
        0
    };
    let target = match launch::target(base, index.as_deref(), &trusted, &[]) {
        Ok(target) => target,
        Err(why) => {
            return serde_json::json!({
                "line": launch::IntegrityLine::no_version(why),
                "launch": null,
                "versionToTrust": null,
            })
            .to_string()
        }
    };
    let failure = integrity::CheckFailure::Unreachable;
    let admission = launch::admit(
        &target,
        observed.as_deref(),
        failure,
        &trusted,
        &[],
        false,
        now,
    );
    let launch_url = admission
        .page()
        .and_then(|page| page.post_launch(lang, now).ok());
    serde_json::json!({
        "line": admission.line(),
        "launch": launch_url,
        "versionToTrust": admission.version_to_trust(),
    })
    .to_string()
}
