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
