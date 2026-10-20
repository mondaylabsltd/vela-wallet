//! The wallet's intents for the web (spec 103): the catalog, and the check
//! every request passes before the shell does it — JSON in, JSON out, the
//! rules themselves in `vela_core::app::intents`.

use vela_core::app::intents::{self, Actor, Grants};
use wasm_bindgen::prelude::*;

/// Every action, with how much it matters and who may ask for it.
#[wasm_bindgen(js_name = intentCatalog)]
#[must_use]
pub fn intent_catalog() -> String {
    serde_json::to_string(&intents::catalog()).unwrap_or_else(|_| "[]".to_owned())
}

/// Whether `actor` (`"person"` or `"agent"`) may ask for the action, and
/// whether its input is well formed. An actor or grants this build cannot
/// read is treated as an agent with nothing granted — the narrowest reading.
#[wasm_bindgen(js_name = intentCheck)]
#[must_use]
pub fn intent_check(actor: &str, action_json: &str, grants_json: &str) -> String {
    let actor = serde_json::from_value::<Actor>(serde_json::Value::String(actor.to_owned()))
        .unwrap_or(Actor::Agent);
    let grants = serde_json::from_str::<Grants>(grants_json).unwrap_or_default();
    let check = intents::check_json(actor, action_json, &grants);
    serde_json::to_string(&check).unwrap_or_else(|_| "null".to_owned())
}
