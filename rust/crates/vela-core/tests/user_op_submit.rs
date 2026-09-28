//! Rules of `user_op::submit_step` (spec 082 T012, RA1): one test per rule.
//!
//! The question every rule answers is the one G21 got wrong on all three
//! clients: after a submit that did not come back cleanly, may the person be
//! told "not sent — try again"? Only when nothing can have left the device, or
//! the relay refused an operation no earlier POST can have delivered.

use vela_core::user_op::{
    submit_step, RelayRejection, SubmitReply, SubmitStep, SubmitVerdict, NOT_SENT_DAPP_DETAIL,
    SUBMIT_MAX_RETRIES, SUBMIT_RETRY_DELAY_MS,
};

const LOCAL: &str = "0x5538ce6978ab8924cce04d68c11cded19c36a8773e94656be6ada6315477ba1f";
const RELAY: &str = "0xa035b480c649967d6713388a017a91c8aea0c9732ed7f4ab51caba8a99199135";

fn error(message: &str) -> SubmitReply {
    SubmitReply::Error(serde_json::json!({ "code": -32000, "message": message }).to_string())
}

fn done(verdict: SubmitVerdict) -> SubmitStep {
    SubmitStep::Done(verdict)
}

fn maybe_sent() -> SubmitStep {
    done(SubmitVerdict::MaybeSent {
        user_op_hash: LOCAL.to_owned(),
    })
}

#[test]
fn the_retry_constants_are_the_old_loops() {
    assert_eq!(SUBMIT_MAX_RETRIES, 3);
    assert_eq!(SUBMIT_RETRY_DELAY_MS, 3_000);
    assert_eq!(NOT_SENT_DAPP_DETAIL, "relay unreachable; nothing was sent");
}

/// Rule 1: a result is the relay holding the operation — its hash wins.
#[test]
fn a_result_is_accepted_with_the_relay_s_hash() {
    for maybe_delivered in [false, true] {
        assert_eq!(
            submit_step(
                &SubmitReply::Hash(RELAY.to_owned()),
                0,
                maybe_delivered,
                LOCAL
            ),
            done(SubmitVerdict::Accepted {
                user_op_hash: RELAY.to_owned()
            })
        );
    }
}

/// A result nobody can read is still an answer from a relay that took the
/// operation: never "not sent".
#[test]
fn a_result_that_is_not_a_hash_may_have_been_sent() {
    for junk in ["", "0x", "null", "0x1234"] {
        assert_eq!(
            submit_step(&SubmitReply::Hash(junk.to_owned()), 0, false, LOCAL),
            maybe_sent(),
            "{junk}"
        );
    }
}

/// Rule 2: the `[existingHash:…]` marker names the operation the relay already
/// holds — accepted under that hash, whatever the transport saw.
#[test]
fn an_existing_hash_marker_is_accepted_with_that_hash() {
    let reply = error(&format!(
        "operation already pending for this account [existingHash:{RELAY}]"
    ));
    for maybe_delivered in [false, true] {
        assert_eq!(
            submit_step(&reply, 1, maybe_delivered, LOCAL),
            done(SubmitVerdict::Accepted {
                user_op_hash: RELAY.to_owned()
            })
        );
    }
    // In `data` rather than `message`: the raw JSON is read first.
    let in_data = SubmitReply::Error(
        serde_json::json!({ "code": -32000, "message": "busy", "data": format!("[existingHash:{RELAY}]") })
            .to_string(),
    );
    assert_eq!(
        submit_step(&in_data, 0, false, LOCAL),
        done(SubmitVerdict::Accepted {
            user_op_hash: RELAY.to_owned()
        })
    );
}

/// Rule 3: "currently processing" re-POSTs the identical operation after 3 s
/// until the third retry, then stops being a reason to wait.
#[test]
fn a_busy_relay_is_retried_until_the_third_retry() {
    let busy = error("Another operation for this account is currently processing");
    for attempt in 0..SUBMIT_MAX_RETRIES {
        assert_eq!(
            submit_step(&busy, attempt, false, LOCAL),
            SubmitStep::RetryAfter {
                delay_ms: SUBMIT_RETRY_DELAY_MS
            },
            "attempt {attempt}"
        );
    }
    assert_eq!(
        submit_step(&busy, 2, false, LOCAL),
        SubmitStep::RetryAfter { delay_ms: 3_000 }
    );
    assert!(matches!(
        submit_step(&busy, 3, false, LOCAL),
        SubmitStep::Done(SubmitVerdict::NotSent { rejection: Some(_) })
    ));
    let later = error("Retry later");
    assert_eq!(
        submit_step(&later, 0, true, LOCAL),
        SubmitStep::RetryAfter { delay_ms: 3_000 }
    );
    // Out of retries after a lost reply: may have been sent.
    assert_eq!(submit_step(&later, 3, true, LOCAL), maybe_sent());
}

/// Rule 4: an error after a POST that may have been delivered proves nothing
/// about that POST — may have been sent, under the local hash.
#[test]
fn an_error_after_a_possible_delivery_may_have_been_sent() {
    assert_eq!(
        submit_step(&error("gas relayer is unavailable"), 1, true, LOCAL),
        maybe_sent()
    );
    assert_eq!(
        submit_step(&SubmitReply::NoAnswer, 0, true, LOCAL),
        maybe_sent()
    );
}

/// Rule 4, G21 exactly: attempt 0's reply was lost (the op landed), attempt 1
/// is refused AA25 because attempt 0 consumed the nonce. "Not sent — try
/// again" here is the double payment.
#[test]
fn aa25_after_a_lost_reply_may_have_been_sent() {
    let aa25 = error("AA25 invalid account nonce");
    // The sticky OR over both POSTs: attempt 0 timed out, attempt 1 got JSON.
    let attempt_0_timed_out = true;
    let attempt_1_answered_json = false;
    let maybe_delivered = attempt_0_timed_out || attempt_1_answered_json;
    assert_eq!(submit_step(&aa25, 1, maybe_delivered, LOCAL), maybe_sent());
}

/// Rule 5: a refusal when no POST can have delivered the operation is a
/// definite "not sent", carrying the relay's reason.
#[test]
fn an_error_with_nothing_delivered_is_not_sent_with_the_rejection() {
    assert_eq!(
        submit_step(
            &error("The gas relayer is unavailable right now."),
            0,
            false,
            LOCAL
        ),
        done(SubmitVerdict::NotSent {
            rejection: Some(RelayRejection::RelayerUnavailable)
        })
    );
    assert_eq!(
        submit_step(
            &error("dedicated bundler gas account is short"),
            0,
            false,
            LOCAL
        ),
        done(SubmitVerdict::NotSent {
            rejection: Some(RelayRejection::BundlerUnderfunded)
        })
    );
    assert_eq!(
        submit_step(&error("AA25 invalid account nonce"), 0, false, LOCAL),
        done(SubmitVerdict::NotSent {
            rejection: Some(RelayRejection::Other(
                "Transaction nonce mismatch. Please try again.".to_owned()
            ))
        })
    );
}

/// Rule 6: the pool gave up and nothing left the device (DNS, refused, TLS,
/// proxy CONNECT failure): definitely not sent, no relay words to quote.
#[test]
fn exhaustion_with_nothing_delivered_is_not_sent_with_no_rejection() {
    assert_eq!(
        submit_step(&SubmitReply::NoAnswer, 0, false, LOCAL),
        done(SubmitVerdict::NotSent { rejection: None })
    );
}

/// The verdict crosses the wasm/UniFFI JSON boundary; its shape is pinned.
#[test]
fn the_wire_shape_is_stable() {
    let json = |step: &SubmitStep| serde_json::to_value(step).unwrap_or_default();
    assert_eq!(
        json(&maybe_sent()),
        serde_json::json!({ "done": { "type": "maybe_sent", "user_op_hash": LOCAL } })
    );
    assert_eq!(
        json(&done(SubmitVerdict::NotSent { rejection: None })),
        serde_json::json!({ "done": { "type": "not_sent", "rejection": null } })
    );
    assert_eq!(
        json(&SubmitStep::RetryAfter { delay_ms: 3_000 }),
        serde_json::json!({ "retry_after": { "delay_ms": 3000 } })
    );
    let reply: SubmitReply =
        serde_json::from_str(r#"{"error":"{\"message\":\"x\"}"}"#).unwrap_or(SubmitReply::NoAnswer);
    assert_eq!(reply, SubmitReply::Error(r#"{"message":"x"}"#.to_owned()));
    let none: SubmitReply =
        serde_json::from_str(r#""no_answer""#).unwrap_or(SubmitReply::Hash(String::new()));
    assert_eq!(none, SubmitReply::NoAnswer);
}
