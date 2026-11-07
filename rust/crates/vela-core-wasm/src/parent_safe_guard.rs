//! §31: the parent-Safe guard's pure rule (`vela_core::app::parent_safe_guard`)
//! for the web's vector replay (`verify-web`) and any page with the core
//! aboard. The uniffi twin is `vela-core-uniffi/src/parent_safe_guard_bridge.rs`.
//! The machines (`SignRequestCore`, the multisig proposals) apply it
//! themselves; nothing in a shell decides from this.

use wasm_bindgen::prelude::*;

/// `parent_safe_guard::check` over JSON: a `CheckInput` in, a
/// `ParentVerdict` out (`{ verdict: "clear" | "refuse" | "read", … }`), or
/// `{ "error": … }` for an input that does not read.
#[wasm_bindgen(js_name = parentSafeCheck)]
#[must_use]
pub fn parent_safe_check(input_json: &str) -> String {
    vela_core::app::parent_safe_guard::check_json(input_json)
        .unwrap_or_else(|error| serde_json::json!({ "error": error }).to_string())
}
