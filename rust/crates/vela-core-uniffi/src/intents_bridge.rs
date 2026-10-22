//! The wallet's intents for Android and iOS (spec 103, spec 104): the
//! catalog, the check every request passes, and the judgements over the
//! wallet's state. JSON in, JSON out — `vela_core::app::intent_json`, the
//! same functions the web and the desktop call. The shell keeps the
//! orchestration (its residents' calls, approvals, waiting).

use vela_core::app::intent_json;

/// Every action, with how much it matters and who may ask for it.
#[uniffi::export]
#[must_use]
pub fn intent_catalog() -> String {
    intent_json::catalog()
}

/// Whether `actor` (`"person"` or `"agent"`) may ask for the action, and
/// whether its input is well formed.
#[uniffi::export]
#[must_use]
pub fn intent_check(actor: String, action_json: String, grants_json: String) -> String {
    intent_json::check(&actor, &action_json, &grants_json)
}

/// Where the action's change is seen: the screen to open once an agent's
/// request is done (a screen's JSON, or `null`).
#[uniffi::export]
#[must_use]
pub fn intent_shows(action_json: String) -> String {
    intent_json::shows(&action_json)
}

/// What an agent's request changes, for its approval: the network, URL or
/// record the action's name does not say (a JSON string, or `null`).
#[uniffi::export]
#[must_use]
pub fn intent_approval_detail(action_json: String) -> String {
    intent_json::approval_detail(&action_json)
}

/// Does what the action names exist, and is it already so?
#[uniffi::export]
#[must_use]
pub fn intent_examine(action_json: String, world_json: String) -> String {
    intent_json::examine(&action_json, &world_json)
}

/// What was true when the call began.
#[uniffi::export]
#[must_use]
pub fn intent_baseline(action_json: String, world_json: String) -> String {
    intent_json::baseline(&action_json, &world_json)
}

/// Has the change held? `{ verdict, baseline }`.
#[uniffi::export]
#[must_use]
pub fn intent_verdict(action_json: String, world_json: String, baseline_json: String) -> String {
    intent_json::verdict(&action_json, &world_json, &baseline_json)
}

/// The views a judgement of the action reads.
#[uniffi::export]
#[must_use]
pub fn intent_world_needs(action_json: String) -> String {
    intent_json::world_needs(&action_json)
}

/// Where a transfer stands now (`send_json` / `ceremony_json` may be `null`).
#[uniffi::export]
#[must_use]
pub fn intent_operation_progress(
    progress_json: String,
    send_json: String,
    ceremony_json: String,
    tracker_json: String,
) -> String {
    intent_json::operation_progress(&progress_json, &send_json, &ceremony_json, &tracker_json)
}

/// `vela_state`: the wallet as an agent reads it.
#[uniffi::export]
#[must_use]
pub fn intent_agent_state(
    world_json: String,
    page: String,
    approvals_json: String,
    activity_json: String,
) -> String {
    intent_json::agent_state(&world_json, &page, &approvals_json, &activity_json)
}

/// `vela_review`: what the person is being asked to sign.
#[uniffi::export]
#[must_use]
pub fn intent_review(input_json: String) -> String {
    intent_json::review(&input_json)
}
