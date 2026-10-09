//! What the web wallet still needs of the signing page (spec 075): nothing of
//! the channel, only the REGISTRY's relying party.
//!
//! The web wallet does not open a signing page at all (owner, 2026-09-23 —
//! "web 就不支持可信签名器好了"): no page, no ceremonies, no loopback socket.
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
