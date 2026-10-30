//! Small rules every shell asks at the edges of a screen: when the launch
//! animation plays, and the ENS nodes a name lookup starts from. The uniffi
//! twins are `vela-core-uniffi/src/shell_rules_bridge.rs`.
//!
//! The web itself decides the launch animation before the core can load (the
//! pre-paint block in `app.html`, and Welcome loads no wasm), so its copies
//! are held to the core's vectors by `constants.test.ts`; this export is for
//! a page that has the core aboard.

use wasm_bindgen::prelude::*;

use crate::err;

/// `stored_last_played_ms`: `undefined` when nothing (readable) is stored.
#[wasm_bindgen(js_name = launchAnimationDue)]
pub fn launch_animation_due(
    stored_last_played_ms: Option<f64>,
    now_ms: f64,
    disabled: bool,
) -> bool {
    vela_core::launch::launch_animation_due(stored_last_played_ms, now_ms, disabled)
}

#[wasm_bindgen(js_name = launchAnimationStorageKey)]
pub fn launch_animation_storage_key() -> String {
    vela_core::launch::LAUNCH_ANIMATION_STORAGE_KEY.to_owned()
}

/// EIP-137 namehash, `0x`-prefixed lower-case hex.
#[wasm_bindgen(js_name = ensNamehash)]
pub fn ens_namehash(name: &str) -> String {
    vela_core::ens::ens_namehash(name)
}

/// The reverse node of `address`; rejects with `{ code, message }` when it
/// is not one.
#[wasm_bindgen(js_name = ensReverseNode)]
pub fn ens_reverse_node(address: &str) -> Result<String, JsValue> {
    vela_core::ens::ens_reverse_node(address).map_err(err)
}
