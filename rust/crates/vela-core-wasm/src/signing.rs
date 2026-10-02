//! Where a signature goes (founder, 2026-09-26): the key the account was
//! created or signed in with — never a choice made per signature.

use wasm_bindgen::prelude::*;

/// The route an account's signatures take, as JSON
/// (`{credential_id, transports, method, signer_origin?}`), or `null` for a
/// record written before the account named its sign-in key — that one signs as
/// it always did. `account_json` is the stored account record. See
/// `vela_core::app::Account::sign_in_route`.
#[wasm_bindgen(js_name = signInRoute)]
pub fn sign_in_route(account_json: &str) -> Option<String> {
    vela_core::app::sign_in_route_json(account_json)
}

/// A key-method row's words as JSON (`{title_key, line_key, line_name}`), or
/// `null` for a wire name the core does not know (087 F01, F02). See
/// `vela_core::app::method_words`.
#[wasm_bindgen(js_name = keyMethodWords)]
#[must_use]
pub fn key_method_words(method: &str, chooser: &str, unlock: &str) -> Option<String> {
    vela_core::app::method_words::method_words_json(method, chooser, unlock)
}

/// A `FeeFailure` from the page: its wire name (`"quote_unavailable"`), or
/// for a failure that carries data its JSON text
/// (`{"chain_read":{"rate_limited":true}}`, spec 082 RJ13).
pub(crate) fn fee_failure_of(failure: &str) -> Option<vela_core::app::fee_policy::FeeFailure> {
    serde_json::from_value(serde_json::Value::String(failure.to_owned()))
        .ok()
        .or_else(|| serde_json::from_str(failure).ok())
}

/// The wait in ms before automatic fee re-quote `attempt` (1-based) after
/// `failure` (see [`fee_failure_of`]), or `undefined` when no retry can fix
/// it (spec 079 FR-008) — the extension's signing sheet re-asks on the same
/// schedule as every other client: 3 s, 6 s, then every 8 s (spec 082 RJ12).
#[wasm_bindgen(js_name = feeRequoteDelayMs)]
pub fn fee_requote_delay_ms(failure: &str, attempt: u32) -> Option<u32> {
    vela_core::app::fee_policy::requote_delay_ms(fee_failure_of(failure)?, attempt)
}

/// The bound on each automatic fee re-quote, in ms (spec 082 RJ12).
#[wasm_bindgen(js_name = feeRequoteTimeoutMs)]
#[must_use]
pub fn fee_requote_timeout_ms() -> u32 {
    vela_core::app::fee_policy::REQUOTE_TIMEOUT_MS
}

/// The corpus key of the reason line under a failed fee, or `undefined` for
/// none (spec 082 RJ13) — the one choice every shell used to make itself.
/// `explore.chainDown` takes `{{chain}}`. `failure` as for
/// [`fee_requote_delay_ms`].
#[wasm_bindgen(js_name = feeFailureReasonKey)]
#[must_use]
pub fn fee_failure_reason_key(failure: &str) -> Option<String> {
    vela_core::app::fee_policy::failure_reason_key(fee_failure_of(failure)?).map(str::to_owned)
}
