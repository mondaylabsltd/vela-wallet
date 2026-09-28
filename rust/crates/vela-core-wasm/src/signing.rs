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

/// The wait in ms before automatic fee re-quote `attempt` (1-based) after
/// `failure` (the `FeeFailure` wire name, e.g. `"quote_unavailable"`), or
/// `undefined` when no retry can fix it (spec 079 FR-008) — the extension's
/// signing sheet re-asks on the same schedule as every other client.
#[wasm_bindgen(js_name = feeRequoteDelayMs)]
pub fn fee_requote_delay_ms(failure: &str, attempt: u32) -> Option<u32> {
    use vela_core::app::fee_policy::{requote_delay_ms, FeeFailure};
    let failure: FeeFailure =
        serde_json::from_value(serde_json::Value::String(failure.to_owned())).ok()?;
    requote_delay_ms(failure, attempt)
}
