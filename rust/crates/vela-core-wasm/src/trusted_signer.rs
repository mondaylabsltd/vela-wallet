//! What the web wallet needs of the signing page: the REGISTRY's relying party
//! (spec 075), and — since 2026-10-29 — the one hand-off it makes, a multisig
//! member's approval (`trustedSignerApprovalRequest` / `…Accept`, the page's
//! check `trustedSignerPageTarget` / `…Admit`, `multisigApprovalRoute`).
//!
//! The web wallet opens no signing page for a transaction, a message or a key
//! ceremony (owner, 2026-09-23 — "web 就不支持可信签名器好了"); a multisig
//! member's approval is the exception (the owner's Bybit lesson).
//! But a wallet whose keys were minted on a page can be published or read from
//! here, and a member minted on a page proves under THAT page's domain. Getting
//! the relying party wrong is how both phones hit 「可信签名器的回复与这笔请求
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
// The one hand-off the web makes: a member whose account reviews and signs on
// a page approves there (`trusted_signer::approval`). The web checks the page
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

/// Where a multisig member's approval goes on the web
/// (`trusted_signer::approval::approval_route`): `{type: 'in_vela'}`,
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
