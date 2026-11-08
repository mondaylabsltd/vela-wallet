//! §28: every production service default, from the core, and the rule that a
//! test build never writes to one (`vela_core::service_defaults`). The uniffi
//! twins are `vela-core-uniffi/src/service_defaults_bridge.rs`.
//!
//! The web reads its defaults before the core can load (Welcome loads no
//! wasm), so it imports `serviceDefaults.ts`, GENERATED from the core
//! (`generate_wallet_state_bindings`, drift-gated by `gen-core-types
//! --check`); these exports are the same answers for a page with the core
//! aboard, and the dev guard's door to the rule (`productionWriteVerdict`).

use wasm_bindgen::prelude::*;

/// `ServiceDefaults` as JSON (camelCase).
#[wasm_bindgen(js_name = serviceDefaults)]
#[must_use]
pub fn service_defaults() -> String {
    vela_core::service_defaults::service_defaults_json()
}

/// §35: the relying party the web wallet uses on `host`
/// (`service_defaults::web_relying_party_id`) — the rule `passkey.ts`
/// follows, replayed against the shared vector.
#[wasm_bindgen(js_name = webRelyingPartyId)]
#[must_use]
pub fn web_relying_party_id(host: &str, packaged_extension: bool) -> String {
    vela_core::service_defaults::web_relying_party_id(host, packaged_extension)
}

/// `service_defaults::relying_party_id` over JSON (the native rule, for the
/// shared vector's replay through the shipped wasm).
#[wasm_bindgen(js_name = relyingPartyJson)]
#[must_use]
pub fn relying_party_json(ask_json: &str) -> String {
    vela_core::service_defaults::relying_party_json(ask_json)
}

/// `service_defaults::write_verdict` over JSON: a `{ testBuild,
/// allowProductionWrites, method, url, body }` ask in, a `{ verdict:
/// "allowed" } | { verdict: "refused", host, key, message }` out.
#[wasm_bindgen(js_name = productionWriteVerdict)]
#[must_use]
pub fn production_write_verdict(ask_json: &str) -> String {
    vela_core::service_defaults::write_verdict_json(ask_json)
}
