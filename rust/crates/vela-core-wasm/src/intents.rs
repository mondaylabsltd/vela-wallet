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

// --- The judgements over the wallet's state (spec 104, part A) ------------

use serde_json::Value;
use vela_core::app::intent_rules::{
    self, Baseline, Ceremony, Examined, Progress, ReviewInput, SendFacts, TrackFacts, Verdict,
    World,
};
use vela_core::app::intents::{Action, Reason, ReasonCode};

fn action_of(json: &str) -> Option<Action> {
    serde_json::from_str(json).ok()
}

fn world_of(json: &str) -> World {
    serde_json::from_str(json).unwrap_or_default()
}

fn to_json(value: &impl serde::Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

/// Does what the action names exist, and is it already so? An action this
/// build cannot read is refused, never waved through.
#[wasm_bindgen(js_name = intentExamine)]
#[must_use]
pub fn intent_examine(action_json: &str, world_json: &str) -> String {
    let examined = action_of(action_json).map_or_else(
        || Examined::Refused {
            reason: Reason {
                code: ReasonCode::InvalidInput,
                detail: "not an action this wallet knows".to_owned(),
            },
        },
        |action| intent_rules::examine(&action, &world_of(world_json)),
    );
    to_json(&examined)
}

/// What was true when the call began.
#[wasm_bindgen(js_name = intentBaseline)]
#[must_use]
pub fn intent_baseline(action_json: &str, world_json: &str) -> String {
    let baseline = action_of(action_json)
        .map(|action| intent_rules::baseline(&action, &world_of(world_json)))
        .unwrap_or_default();
    to_json(&baseline)
}

/// Has the change held? Answers `{ verdict, baseline }`: the baseline as the
/// call has moved it (the sign-out sheet coming up is part of the call).
#[wasm_bindgen(js_name = intentVerdict)]
#[must_use]
pub fn intent_verdict(action_json: &str, world_json: &str, baseline_json: &str) -> String {
    let mut baseline = serde_json::from_str::<Baseline>(baseline_json).unwrap_or_default();
    let verdict = action_of(action_json).map_or(Verdict::NotYet, |action| {
        intent_rules::verdict(&action, &world_of(world_json), &mut baseline)
    });
    to_json(&serde_json::json!({ "verdict": verdict, "baseline": baseline }))
}

/// The views a judgement of the action reads.
#[wasm_bindgen(js_name = intentWorldNeeds)]
#[must_use]
pub fn intent_world_needs(action_json: &str) -> String {
    let needs: &[&str] = action_of(action_json).map_or(&[], |a| intent_rules::world_needs(&a));
    to_json(&needs)
}

/// Where a transfer stands now. `send_json` / `ceremony_json` may be `null`.
#[wasm_bindgen(js_name = intentOperationProgress)]
#[must_use]
pub fn intent_operation_progress(
    progress_json: &str,
    send_json: &str,
    ceremony_json: &str,
    tracker_json: &str,
) -> String {
    let Ok(progress) = serde_json::from_str::<Progress>(progress_json) else {
        return progress_json.to_owned();
    };
    let send = serde_json::from_str::<Option<SendFacts>>(send_json).unwrap_or(None);
    let ceremony = serde_json::from_str::<Option<Ceremony>>(ceremony_json).unwrap_or(None);
    let tracker = serde_json::from_str::<Vec<TrackFacts>>(tracker_json).unwrap_or_default();
    to_json(&intent_rules::operation_progress(
        progress,
        send.as_ref(),
        ceremony.as_ref(),
        &tracker,
    ))
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
    let approvals = serde_json::from_str::<Value>(approvals_json).unwrap_or(Value::Null);
    let activity = serde_json::from_str::<Value>(activity_json).unwrap_or(Value::Null);
    to_json(&intent_rules::agent_state(
        &world_of(world_json),
        page,
        &approvals,
        &activity,
    ))
}

/// `vela_review`: what the person is being asked to sign.
#[wasm_bindgen(js_name = intentReview)]
#[must_use]
pub fn intent_review(input_json: &str) -> String {
    let input = serde_json::from_str::<ReviewInput>(input_json).unwrap_or_default();
    to_json(&intent_rules::review(&input))
}
