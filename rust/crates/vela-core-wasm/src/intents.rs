//! The wallet's intents for the web (spec 103, spec 104): the catalog, the
//! check every request passes, and the judgements over the wallet's state.
//! JSON in, JSON out — `vela_core::app::intent_json`, the same functions
//! every shell calls.

use vela_core::app::intent_json;
use wasm_bindgen::prelude::*;

/// Every action, with how much it matters and who may ask for it.
#[wasm_bindgen(js_name = intentCatalog)]
#[must_use]
pub fn intent_catalog() -> String {
    intent_json::catalog()
}

/// Whether `actor` may ask for the action, and whether its input is well formed.
#[wasm_bindgen(js_name = intentCheck)]
#[must_use]
pub fn intent_check(actor: &str, action_json: &str, grants_json: &str) -> String {
    intent_json::check(actor, action_json, grants_json)
}

/// Where the action's change is seen: the screen to open once an agent's
/// request is done (a screen's JSON, or `null`).
#[wasm_bindgen(js_name = intentShows)]
#[must_use]
pub fn intent_shows(action_json: &str) -> String {
    intent_json::shows(action_json)
}

/// What an agent's request changes, for its approval: the network, URL or
/// record the action's name does not say (a JSON string, or `null`).
#[wasm_bindgen(js_name = intentApprovalDetail)]
#[must_use]
pub fn intent_approval_detail(action_json: &str) -> String {
    intent_json::approval_detail(action_json)
}

/// Does what the action names exist, and is it already so?
#[wasm_bindgen(js_name = intentExamine)]
#[must_use]
pub fn intent_examine(action_json: &str, world_json: &str) -> String {
    intent_json::examine(action_json, world_json)
}

/// What was true when the call began.
#[wasm_bindgen(js_name = intentBaseline)]
#[must_use]
pub fn intent_baseline(action_json: &str, world_json: &str) -> String {
    intent_json::baseline(action_json, world_json)
}

/// Has the change held? `{ verdict, baseline }`.
#[wasm_bindgen(js_name = intentVerdict)]
#[must_use]
pub fn intent_verdict(action_json: &str, world_json: &str, baseline_json: &str) -> String {
    intent_json::verdict(action_json, world_json, baseline_json)
}

/// The views a judgement of the action reads.
#[wasm_bindgen(js_name = intentWorldNeeds)]
#[must_use]
pub fn intent_world_needs(action_json: &str) -> String {
    intent_json::world_needs(action_json)
}

/// Where a transfer stands now.
#[wasm_bindgen(js_name = intentOperationProgress)]
#[must_use]
pub fn intent_operation_progress(
    progress_json: &str,
    send_json: &str,
    ceremony_json: &str,
    tracker_json: &str,
) -> String {
    intent_json::operation_progress(progress_json, send_json, ceremony_json, tracker_json)
}

/// `vela_state`: the wallet as an agent reads it.
#[wasm_bindgen(js_name = intentAgentState)]
#[must_use]
pub fn intent_agent_state(
    world_json: &str,
    page: &str,
    approvals_json: &str,
    activity_json: &str,
) -> String {
    intent_json::agent_state(world_json, page, approvals_json, activity_json)
}

/// `vela_review`: what the person is being asked to sign.
#[wasm_bindgen(js_name = intentReview)]
#[must_use]
pub fn intent_review(input_json: &str) -> String {
    intent_json::review(input_json)
}

/// What an agent's request changes, for its approval, with what the wallet
/// knows around it (`world_json`, as for `intentExamine`).
#[wasm_bindgen(js_name = intentApprovalDetailIn)]
#[must_use]
pub fn intent_approval_detail_in(action_json: &str, world_json: &str) -> String {
    intent_json::approval_detail_in(action_json, world_json)
}
