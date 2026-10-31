//! The relay token's rules for a shell's PeerConnect transport
//! ([`crate::app::multisig_outside::relay`]), asked one at a time: which
//! relay asks for a token, whether the one kept is still good, what a token
//! endpoint's answer means, whether a refused token is forgotten, how long a
//! connection waits after a refusal or after getting no token, and where its
//! way onto the relay stands. The transport keeps only the I/O — the socket,
//! the timers, the fetches, the storage — and asks here for every decision.
//!
//! The refusal count is the transport's to keep between questions: it hands
//! the state in and keeps the state handed back (`relay.refusals`).

use serde::Deserialize;
use serde_json::{json, Value};

use super::{arg, to_json};
use crate::app::multisig_outside::relay::{self, Before, KeptToken, Refusals, RelayStatus};
use crate::CoreError;

/// What a connection tells its refusal count.
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
enum RefusalsOn {
    /// The relay refused the token in use (`unauthorized`).
    Unauthorized,
    /// Before the next token is asked for: what to do first.
    BeforeToken,
    /// The person tapped Retry after a stop.
    Retried,
    /// A token was issued (`source`).
    Issued,
    /// On the relay: the refusals are forgotten.
    Accepted,
}

fn err(message: impl Into<String>) -> CoreError {
    CoreError::Multisig(message.into())
}

/// `relay.<name>`, its arguments one JSON object, its answer JSON.
pub fn call(name: &str, args: &Value) -> Result<String, CoreError> {
    match name {
        // Constants the transport schedules by.
        "constants" => to_json(&json!({
            "defaultRelay": relay::DEFAULT_RELAY,
            "tokenEndpoint": relay::TOKEN_ENDPOINT,
            "trialEndpoint": relay::TRIAL_ENDPOINT,
            "backoffMs": relay::BACKOFF_MS,
            "refusals": relay::REFUSALS,
            "unreachableAfterMs": relay::UNREACHABLE_AFTER_MS,
        })),
        "needsToken" => to_json(&relay::needs_token(&arg::<String>(args, "relay")?)),
        "usable" => {
            let kept = arg::<Option<KeptToken>>(args, "kept").unwrap_or(None);
            let now_s: f64 = arg(args, "nowS")?;
            let trial = arg::<Option<bool>>(args, "trial")?.unwrap_or(false);
            to_json(
                &relay::usable(kept.as_ref(), now_s, trial)
                    .map(|(token, source)| json!({ "token": token, "source": source })),
            )
        }
        "tokenAnswer" => {
            let status = arg::<Option<u16>>(args, "status")?;
            let body = arg::<Option<String>>(args, "body")?.unwrap_or_default();
            to_json(&match relay::token_answer(status, &body) {
                Ok(kept) => json!({ "ok": kept }),
                Err(said) => json!({ "err": said }),
            })
        }
        "noTokenDetail" => to_json(&relay::no_token_detail(
            &arg::<String>(args, "endpoint")?,
            arg::<Option<String>>(args, "site")?.as_deref(),
            &arg::<String>(args, "trial")?,
        )),
        "forgets" => {
            let kept = arg::<Option<KeptToken>>(args, "kept").unwrap_or(None);
            to_json(&relay::forgets(
                kept.as_ref(),
                arg::<Option<String>>(args, "refused")?.as_deref(),
            ))
        }
        "refusals" => {
            let mut state = arg::<Option<Refusals>>(args, "state")?.unwrap_or_default();
            let on: RefusalsOn = arg(args, "on")?;
            let before: Option<Before> = match on {
                RefusalsOn::Unauthorized => {
                    state.unauthorized();
                    None
                }
                RefusalsOn::BeforeToken => Some(state.before_token()),
                RefusalsOn::Retried => {
                    state.retried();
                    None
                }
                RefusalsOn::Issued => {
                    state.issued(arg(args, "source")?);
                    None
                }
                RefusalsOn::Accepted => {
                    state.accepted();
                    None
                }
            };
            to_json(&json!({ "state": state, "before": before }))
        }
        "noTokenStep" => to_json(&relay::no_token_step(arg(args, "tries")?)),
        "onState" => {
            let status: RelayStatus =
                arg::<Option<RelayStatus>>(args, "status")?.unwrap_or_default();
            let state: String = arg(args, "state")?;
            let offline_ms = arg::<Option<f64>>(args, "offlineMs")?.unwrap_or(0.0);
            let relay_url = arg::<Option<String>>(args, "relay")?
                .unwrap_or_else(|| relay::DEFAULT_RELAY.to_owned());
            to_json(&json!({
                "status": relay::on_state(&status, &state, offline_ms, &relay_url),
                // On the relay: its refusals are forgotten.
                "accepted": matches!(state.as_str(), "waiting" | "connected"),
            }))
        }
        "worst" => to_json(&relay::worst(&arg::<Vec<RelayStatus>>(args, "statuses")?)),
        other => Err(err(format!("unknown relay rule: {other}"))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn ask(name: &str, args: Value) -> Value {
        serde_json::from_str(&call(name, &args).unwrap()).unwrap()
    }

    #[test]
    fn every_rule_answers_through_the_door() {
        assert_eq!(
            ask(
                "needsToken",
                json!({ "relay": "wss://relay.peerconnect.dev/" })
            ),
            json!(true)
        );
        assert_eq!(
            ask("refusals", json!({ "on": "beforeToken" }))["before"],
            json!({ "kind": "go", "trial": false })
        );
        assert!(call("nope", &json!({})).is_err());
    }
}
