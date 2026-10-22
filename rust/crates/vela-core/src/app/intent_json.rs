//! The intents for every shell, JSON in and JSON out (spec 103, spec 104):
//! the catalog, the check, and the judgements over the wallet's state.
//!
//! The web (wasm), Android and iOS (uniffi) and the desktop call these same
//! functions, so a shell never decides how to read a request, and what an
//! unreadable one means is decided once — always the narrowest reading: an
//! action this build cannot read is refused, a verdict on one is "not yet",
//! an unknown actor is an agent with nothing granted.

use serde::Serialize;
use serde_json::Value;

use super::intent_rules::{
    self, Baseline, Ceremony, Examined, Progress, ReviewInput, SendFacts, TrackFacts, Verdict,
    World,
};
use super::intents::{self, Action, Actor, Grants, Reason, ReasonCode};

fn to_json(value: &impl Serialize) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_owned())
}

fn action_of(json: &str) -> Option<Action> {
    serde_json::from_str(json).ok()
}

fn world_of(json: &str) -> World {
    serde_json::from_str(json).unwrap_or_default()
}

/// Every action, with how much it matters and who may ask for it.
#[must_use]
pub fn catalog() -> String {
    to_json(&intents::catalog())
}

/// Whether `actor` (`"person"` or `"agent"`) may ask for the action, and
/// whether its input is well formed.
#[must_use]
pub fn check(actor: &str, action_json: &str, grants_json: &str) -> String {
    let actor =
        serde_json::from_value::<Actor>(Value::String(actor.to_owned())).unwrap_or(Actor::Agent);
    let grants = serde_json::from_str::<Grants>(grants_json).unwrap_or_default();
    to_json(&intents::check_json(actor, action_json, &grants))
}

/// Where the action's change is seen (a screen), or `null`.
#[must_use]
pub fn shows(action_json: &str) -> String {
    to_json(&action_of(action_json).and_then(|action| intents::shows(&action)))
}

/// What an agent's request changes, for its approval (a string), or `null`.
#[must_use]
pub fn approval_detail(action_json: &str) -> String {
    to_json(&action_of(action_json).and_then(|action| intent_rules::approval_detail(&action)))
}

/// Does what the action names exist, and is it already so?
#[must_use]
pub fn examine(action_json: &str, world_json: &str) -> String {
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
#[must_use]
pub fn baseline(action_json: &str, world_json: &str) -> String {
    let baseline = action_of(action_json)
        .map(|action| intent_rules::baseline(&action, &world_of(world_json)))
        .unwrap_or_default();
    to_json(&baseline)
}

/// Has the change held? Answers `{ verdict, baseline }`: the baseline as the
/// call has moved it (the sign-out sheet coming up is part of the call).
#[must_use]
pub fn verdict(action_json: &str, world_json: &str, baseline_json: &str) -> String {
    let mut baseline = serde_json::from_str::<Baseline>(baseline_json).unwrap_or_default();
    let verdict = action_of(action_json).map_or(Verdict::NotYet, |action| {
        intent_rules::verdict(&action, &world_of(world_json), &mut baseline)
    });
    to_json(&serde_json::json!({ "verdict": verdict, "baseline": baseline }))
}

/// The views a judgement of the action reads.
#[must_use]
pub fn world_needs(action_json: &str) -> String {
    let needs: &[&str] = action_of(action_json).map_or(&[], |a| intent_rules::world_needs(&a));
    to_json(&needs)
}

/// Where a transfer stands now. `send_json` / `ceremony_json` may be `null`;
/// a progress this build cannot read is handed back as it came.
#[must_use]
pub fn operation_progress(
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
#[must_use]
pub fn agent_state(
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
#[must_use]
pub fn review(input_json: &str) -> String {
    let input = serde_json::from_str::<ReviewInput>(input_json).unwrap_or_default();
    to_json(&intent_rules::review(&input))
}
