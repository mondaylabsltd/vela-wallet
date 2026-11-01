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

/// r9-17: the networks an account's incoming scan reads, and where its
/// native balance is watched (`incoming_scan::scan_plan`): `TrustScanFacts`
/// JSON in, `IncomingScanPlan` JSON out. The uniffi twin is
/// `shell_rules_bridge.rs`'s `incoming_scan_plan`.
#[wasm_bindgen(js_name = incomingScanPlan)]
pub fn incoming_scan_plan(facts_json: &str) -> String {
    vela_core::app::incoming_scan::scan_plan_json(facts_json)
}

/// r9-17: one poll's reads on one network (`incoming_scan::scan_windows`):
/// `IncomingScanWindowsAsk` JSON in, `IncomingScanWindows` JSON out.
#[wasm_bindgen(js_name = incomingScanWindows)]
pub fn incoming_scan_windows(ask_json: &str) -> String {
    vela_core::app::incoming_scan::scan_windows_json(ask_json)
}

/// r9-17: the wait before the next incoming scan
/// (`incoming_scan::next_scan_ms`).
#[wasm_bindgen(js_name = incomingNextScanMs)]
pub fn incoming_next_scan_ms(visible: bool, since_interaction_ms: f64) -> u32 {
    vela_core::app::incoming_scan::next_scan_ms(visible, since_interaction_ms)
}
