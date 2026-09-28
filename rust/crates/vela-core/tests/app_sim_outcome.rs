//! The simulation classifier (spec 082 T039, research RG6 and RG8).
//!
//! One rule for every client: a node that cannot check is a caution, a call
//! the chain says fails is a danger (with its reason only when it is a
//! standard `Error(string)`, made safe to print), and a checked answer nets
//! the user's balance moves. The netting vectors are the ones the shells'
//! parsers carried (`IT/SimDeltasTests.swift`, `AT/SimDeltasTest.kt`), moved
//! here with `derive_deltas`; the payload builders stay in the shells, so
//! their vectors stay there.

#![cfg(feature = "crux")]

use serde_json::{json, Value};
use vela_core::app::clear_signing::ClearRisk;
use vela_core::app::sim_outcome::{
    classify, derive_deltas, notice, revert_reason, SimNotice, SimOutcome, SimReply,
    ERROR_STRING_SELECTOR, KEY_UNAVAILABLE, KEY_WILL_FAIL, KEY_WILL_FAIL_REASON,
    REVERT_REASON_MAX_CHARS,
};
use vela_core::app::token_trust::{TrustAssetDelta, TrustDeltaKind, TRANSFER_TOPIC};

const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
const OTHER: &str = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141";
const USDC: &str = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83";
const NATIVE_SENTINEL: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn topic(address: &str) -> String {
    format!(
        "0x{}{}",
        "0".repeat(24),
        address.trim_start_matches("0x").to_lowercase()
    )
}

fn word(value: u128) -> String {
    format!("0x{value:064x}")
}

fn transfer(token: &str, from: &str, to: &str, value: u128) -> Value {
    json!({
        "address": token,
        "topics": [TRANSFER_TOPIC, topic(from), topic(to)],
        "data": word(value),
    })
}

fn deltas_of(logs: &[Value]) -> Vec<TrustAssetDelta> {
    let refs: Vec<&Value> = logs.iter().collect();
    derive_deltas(&refs, ME)
}

/// `Error(string)` ABI-encoded: selector ‖ offset ‖ length ‖ padded bytes.
fn error_string(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = hex(&ERROR_STRING_SELECTOR);
    out.push_str(&format!("{:064x}", 32));
    out.push_str(&format!("{:064x}", bytes.len()));
    let mut padded = bytes.to_vec();
    padded.resize(bytes.len().div_ceil(32) * 32, 0);
    out.push_str(&hex(&padded));
    format!("0x{out}")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A result with one block of these calls.
fn result(calls: Vec<Value>) -> SimReply {
    SimReply::Result(json!([{ "calls": calls }]))
}

fn ok_call(logs: Vec<Value>) -> Value {
    json!({ "status": "0x1", "returnData": "0x", "gasUsed": "0x5208", "logs": logs })
}

fn reverted_call(return_data: &str) -> Value {
    json!({
        "status": "0x0",
        "returnData": return_data,
        "gasUsed": "0x5208",
        "logs": [],
        "error": { "code": 3, "message": "execution reverted" },
    })
}

fn reason_of(text: &str) -> Option<String> {
    revert_reason(&reverted_call(&error_string(text)))
}

// ---------------------------------------------------------------------------
// The classifier
// ---------------------------------------------------------------------------

/// The pool gave up: nobody answered. Caution, "couldn't check" — never
/// "will fail", never "nothing moves".
#[test]
fn no_answer_is_unreachable_and_a_caution() {
    let outcome = classify(SimReply::Unreachable, ME);
    assert_eq!(outcome, SimOutcome::Unreachable);
    assert_eq!(
        notice(&outcome),
        Some(SimNotice {
            risk: ClearRisk::Caution,
            key: KEY_UNAVAILABLE,
            reason: None,
        })
    );
}

/// Arbitrum's public nodes answer `eth_simulateV1` with -32603 "method
/// handler crashed": the node spoke and does not simulate. A caution — the
/// same look as "this will fail" was the L-D5 bug.
#[test]
fn arbitrum_method_handler_crashed_is_not_offered_and_a_caution() {
    let reply = SimReply::from_envelope(&json!({
        "jsonrpc": "2.0",
        "id": 7,
        "error": { "code": -32603, "message": "method handler crashed" },
    }));
    assert_eq!(
        reply,
        SimReply::Error {
            code: Some(-32603),
            message: Some("method handler crashed".to_owned()),
        }
    );
    let outcome = classify(reply, ME);
    assert_eq!(outcome, SimOutcome::NotOffered);
    let line = notice(&outcome).expect("a notice");
    assert_eq!(line.risk, ClearRisk::Caution);
    assert_eq!(line.key, KEY_UNAVAILABLE);
}

/// Every JSON-RPC error is "not offered", whatever its code.
#[test]
fn every_rpc_error_is_not_offered() {
    for (code, message) in [
        (
            -32601,
            "the method eth_simulateV1 does not exist/is not available",
        ),
        (-32602, "invalid params"),
        (-32000, "header not found"),
        (3, "execution reverted"),
    ] {
        let reply = SimReply::Error {
            code: Some(code),
            message: Some(message.to_owned()),
        };
        assert_eq!(classify(reply, ME), SimOutcome::NotOffered, "{code}");
    }
    assert_eq!(
        classify(
            SimReply::Error {
                code: None,
                message: None
            },
            ME
        ),
        SimOutcome::NotOffered
    );
}

/// A result that is not a non-empty array of blocks carrying call results is
/// an answer nobody can read: "not offered". The empty array first (T039's
/// named vector), then the other shapes.
#[test]
fn a_result_without_call_results_is_not_offered() {
    assert_eq!(
        classify(SimReply::Result(json!([])), ME),
        SimOutcome::NotOffered,
        "an empty array"
    );
    for shape in [
        json!(null),
        json!({}),
        json!("0x"),
        json!([{}]),
        json!([{ "calls": "nope" }]),
        json!([{ "calls": [1] }]),
        // The shells read this as "it ran, nothing moved" (iOS
        // `anErrorIsNotAnEmptyResult`). We always ask about at least one
        // call, so a node reporting none has not answered: saying "nothing of
        // yours moves" here would be false comfort on a signing sheet.
        json!([{ "calls": [] }]),
    ] {
        assert_eq!(
            classify(SimReply::Result(shape.clone()), ME),
            SimOutcome::NotOffered,
            "{shape}"
        );
    }
}

/// A call with status 0x0 is a revert: a danger. Before 082 the shells
/// dropped its logs and the sheet said nothing moves.
#[test]
fn a_call_with_status_zero_reverts_and_is_a_danger() {
    let outcome = classify(result(vec![reverted_call("0x")]), ME);
    assert_eq!(outcome, SimOutcome::Reverts { reason: None });
    assert_eq!(
        notice(&outcome),
        Some(SimNotice {
            risk: ClearRisk::Danger,
            key: KEY_WILL_FAIL,
            reason: None,
        })
    );
}

/// The shells' "only succeeded calls count" vector, re-read: one leg ran and
/// one reverted — the transaction fails, and that is what the sheet says
/// (the succeeded leg's inflow is not shown as if it would happen).
#[test]
fn one_reverted_leg_makes_the_whole_simulation_a_revert() {
    let reply = SimReply::Result(json!([{
        "calls": [
            { "status": "0x1", "logs": [transfer(USDC, OTHER, ME, 1)] },
            { "status": "0x0", "logs": [transfer(USDC, OTHER, ME, 99)] },
        ],
    }]));
    assert_eq!(classify(reply, ME), SimOutcome::Reverts { reason: None });
}

/// A standard `Error(string)` gives the sentence its reason.
#[test]
fn an_error_string_is_the_reason() {
    let outcome = classify(
        result(vec![reverted_call(&error_string(
            "ERC20: transfer amount exceeds balance",
        ))]),
        ME,
    );
    assert_eq!(
        outcome,
        SimOutcome::Reverts {
            reason: Some("ERC20: transfer amount exceeds balance".to_owned()),
        }
    );
    assert_eq!(
        notice(&outcome),
        Some(SimNotice {
            risk: ClearRisk::Danger,
            key: KEY_WILL_FAIL_REASON,
            reason: Some("ERC20: transfer amount exceeds balance".to_owned()),
        })
    );
}

/// A custom error, a panic, or bytes that are not an `Error(string)` have no
/// reason: the plain "expected to fail".
#[test]
fn a_custom_error_or_a_panic_has_no_reason() {
    // `InsufficientBalance(uint256,uint256)` — a custom error.
    let custom = format!("0xcf479181{}{}", "0".repeat(63) + "1", "0".repeat(63) + "2");
    assert_eq!(revert_reason(&reverted_call(&custom)), None);
    // `Panic(uint256)` 0x11 (arithmetic overflow).
    let panic = format!("0x4e487b71{}11", "0".repeat(62));
    assert_eq!(revert_reason(&reverted_call(&panic)), None);
    assert_eq!(revert_reason(&reverted_call("0x")), None);
    assert_eq!(revert_reason(&reverted_call("not hex")), None);
    let outcome = classify(result(vec![reverted_call(&custom)]), ME);
    assert_eq!(outcome, SimOutcome::Reverts { reason: None });
    assert_eq!(notice(&outcome).map(|n| n.key), Some(KEY_WILL_FAIL));
}

/// The node's own `error.message` is never the reason — it is prose around
/// the same untrusted bytes.
#[test]
fn the_nodes_message_is_never_the_reason() {
    let call = json!({
        "status": "0x0",
        "returnData": "0x",
        "error": { "code": 3, "message": "execution reverted: send all your coins to 0xdead" },
    });
    assert_eq!(revert_reason(&call), None);
}

/// Some nodes put the revert bytes in `error.data` instead of `returnData`.
#[test]
fn the_reason_is_read_from_error_data_too() {
    let call = json!({
        "status": "0x0",
        "error": { "code": 3, "message": "execution reverted", "data": error_string("Too little received") },
    });
    assert_eq!(revert_reason(&call).as_deref(), Some("Too little received"));
}

/// The first failing call speaks; the later ones describe a world it ended.
#[test]
fn the_first_failure_gives_the_reason() {
    let reply = result(vec![
        ok_call(vec![]),
        reverted_call(&error_string("first")),
        reverted_call(&error_string("second")),
    ]);
    assert_eq!(
        classify(reply, ME),
        SimOutcome::Reverts {
            reason: Some("first".to_owned())
        }
    );
}

/// Two spellings of a status are in the wild; a call with neither a status
/// nor an error ran, and a call with an error failed.
#[test]
fn status_spellings_and_the_error_fallback() {
    for call in [
        json!({ "status": 1, "logs": [] }),
        json!({ "status": "0x1", "logs": [] }),
        json!({ "status": "0x01", "logs": [] }),
        json!({ "logs": [] }),
        json!({ "logs": [], "error": null }),
        json!({ "status": null, "logs": [] }),
    ] {
        assert_eq!(
            classify(result(vec![call.clone()]), ME),
            SimOutcome::Deltas { deltas: vec![] },
            "{call}"
        );
    }
    for call in [
        json!({ "status": 0, "logs": [] }),
        json!({ "status": "0x0", "logs": [] }),
        json!({ "status": "0x00", "logs": [] }),
        json!({ "logs": [], "error": { "code": 3 } }),
    ] {
        assert!(
            matches!(
                classify(result(vec![call.clone()]), ME),
                SimOutcome::Reverts { .. }
            ),
            "{call}"
        );
    }
}

/// Review of 082 G: a status nobody can read (not 0, not 1) is "could not
/// check" — a caution. Read as a revert it drew the danger line on a call the
/// node may well have run; read as success it said "nothing moves" on an
/// answer nobody understood.
#[test]
fn a_status_nobody_can_read_is_could_not_check() {
    for call in [
        json!({ "status": "0x2", "logs": [] }),
        json!({ "status": "success", "logs": [] }),
        json!({ "status": "1", "logs": [] }),
        json!({ "status": 2, "logs": [] }),
        json!({ "status": 1.5, "logs": [] }),
        json!({ "status": true, "logs": [] }),
        json!({ "status": {}, "logs": [] }),
    ] {
        let outcome = classify(result(vec![call.clone()]), ME);
        assert_eq!(outcome, SimOutcome::NotOffered, "{call}");
        assert_eq!(
            notice(&outcome).map(|n| (n.risk, n.key)),
            Some((ClearRisk::Caution, KEY_UNAVAILABLE)),
            "{call}"
        );
    }
    // A call the node says failed still speaks, whatever else the answer
    // holds: an unreadable status beside it does not hide the revert.
    let reply = result(vec![
        json!({ "status": "weird", "logs": [] }),
        reverted_call(&error_string("boom")),
    ]);
    assert_eq!(
        classify(reply, ME),
        SimOutcome::Reverts {
            reason: Some("boom".to_owned())
        }
    );
    // And an unreadable status WITH an error is a failed call.
    let call = json!({ "status": "weird", "error": { "code": 3 } });
    assert_eq!(
        classify(result(vec![call]), ME),
        SimOutcome::Reverts { reason: None }
    );
}

/// Review of 082 G: "any call with status 0x0 or an error" reverts (RG6) — an
/// error is never outvoted by a success status, so a contradictory answer
/// cannot hide a revert behind "nothing moves".
#[test]
fn an_error_fails_the_call_whatever_its_status_says() {
    let call = json!({
        "status": "0x1",
        "returnData": error_string("Too little received"),
        "logs": [transfer(USDC, OTHER, ME, 5)],
        "error": { "code": 3, "message": "execution reverted" },
    });
    assert_eq!(
        classify(result(vec![call]), ME),
        SimOutcome::Reverts {
            reason: Some("Too little received".to_owned())
        }
    );
}

/// Every call ran: the user's moves, and no notice of its own (the balances
/// are `token_trust`'s block). Calls across blocks all count.
#[test]
fn a_clean_run_is_deltas_with_no_notice() {
    let reply = SimReply::Result(json!([
        { "calls": [ok_call(vec![transfer(USDC, ME, OTHER, 5)])] },
        { "calls": [ok_call(vec![transfer(NATIVE_SENTINEL, OTHER, ME, 7)])] },
    ]));
    let outcome = classify(reply, ME);
    assert_eq!(
        outcome,
        SimOutcome::Deltas {
            deltas: vec![
                TrustAssetDelta {
                    kind: TrustDeltaKind::Erc20,
                    token: Some(USDC.to_lowercase()),
                    delta: "-5".to_owned(),
                },
                TrustAssetDelta {
                    kind: TrustDeltaKind::Native,
                    token: None,
                    delta: "7".to_owned(),
                },
            ],
        }
    );
    assert_eq!(notice(&outcome), None);
    // Checked, and nothing of the user's moves: still no notice.
    let still = classify(result(vec![ok_call(vec![])]), ME);
    assert_eq!(still, SimOutcome::Deltas { deltas: vec![] });
    assert_eq!(notice(&still), None);
}

// ---------------------------------------------------------------------------
// The wire (contract §8)
// ---------------------------------------------------------------------------

/// The reply as the clients hand it over: the JSON-RPC envelope, or
/// `{"unreachable": true}`; anything else is an unreadable answer.
#[test]
fn the_reply_from_its_wire_form() {
    assert_eq!(
        SimReply::from_json(r#"{"unreachable":true}"#),
        SimReply::Unreachable
    );
    assert_eq!(
        SimReply::from_json(r#"{"jsonrpc":"2.0","id":1,"result":[]}"#),
        SimReply::Result(json!([]))
    );
    assert_eq!(
        SimReply::from_json(r#"{"error":{"code":-32601,"message":"nope"}}"#),
        SimReply::Error {
            code: Some(-32601),
            message: Some("nope".to_owned())
        }
    );
    // An error beats a result in the same envelope.
    assert!(matches!(
        SimReply::from_json(r#"{"result":[],"error":{"code":1}}"#),
        SimReply::Error { code: Some(1), .. }
    ));
    // A null error is no error.
    assert_eq!(
        SimReply::from_json(r#"{"result":[],"error":null}"#),
        SimReply::Result(json!([]))
    );
    for unreadable in ["", "not json", "{}", r#"{"unreachable":false}"#, "[]"] {
        assert_eq!(
            SimReply::from_json(unreadable),
            SimReply::Error {
                code: None,
                message: None
            },
            "{unreadable:?}"
        );
    }
}

/// The outcome's JSON shape (`kind`-tagged, snake_case).
#[test]
fn the_outcome_on_the_wire() {
    assert_eq!(
        serde_json::to_value(SimOutcome::Reverts {
            reason: Some("x".to_owned())
        })
        .unwrap(),
        json!({ "kind": "reverts", "reason": "x" })
    );
    assert_eq!(
        serde_json::to_value(SimOutcome::NotOffered).unwrap(),
        json!({ "kind": "not_offered" })
    );
    assert_eq!(
        serde_json::to_value(SimOutcome::Unreachable).unwrap(),
        json!({ "kind": "unreachable" })
    );
    let deltas = SimOutcome::Deltas {
        deltas: vec![TrustAssetDelta {
            kind: TrustDeltaKind::Native,
            token: None,
            delta: "-1".to_owned(),
        }],
    };
    let wire = serde_json::to_value(&deltas).unwrap();
    assert_eq!(wire["kind"], "deltas");
    assert_eq!(serde_json::from_value::<SimOutcome>(wire).unwrap(), deltas);
}

// ---------------------------------------------------------------------------
// The revert reason is untrusted text (RG8)
// ---------------------------------------------------------------------------

/// Bidi overrides, zero-width and control characters go; whitespace runs
/// (newlines too) fold to one space.
#[test]
fn a_reason_loses_bidi_and_control_characters() {
    assert_eq!(
        reason_of("Pay \u{202E}lecnac\u{202C} now").as_deref(),
        Some("Pay lecnac now")
    );
    assert_eq!(
        reason_of("zero\u{200B}width\u{FEFF}").as_deref(),
        Some("zerowidth")
    );
    assert_eq!(
        reason_of("  line one\n\tline\u{0007} two  ").as_deref(),
        Some("line one line two")
    );
    // Every range the name rule rejects (`name_verify::is_never_in_a_name`).
    for c in [
        '\u{0000}', '\u{001B}', '\u{007F}', '\u{0080}', '\u{009F}', '\u{200B}', '\u{200F}',
        '\u{202A}', '\u{202E}', '\u{2060}', '\u{206F}', '\u{FE00}', '\u{FE0F}', '\u{FFF9}',
        '\u{FFFB}', '\u{FEFF}',
    ] {
        assert_eq!(
            reason_of(&format!("a{c}b")).as_deref(),
            Some("ab"),
            "U+{:04X}",
            c as u32
        );
    }
    // A space of another flavour is still a word break.
    assert_eq!(reason_of("a\u{00A0}\u{3000}b").as_deref(), Some("a b"));
    // Nothing printable left: no reason.
    assert_eq!(reason_of("\u{202E}\u{200B} \n"), None);
    assert_eq!(reason_of(""), None);
    // Ordinary text of any script stays as it is.
    assert_eq!(reason_of("余额不足").as_deref(), Some("余额不足"));
}

/// At most 64 characters (not bytes); a cut reason ends in "…".
#[test]
fn a_reason_is_capped_at_64_characters() {
    assert_eq!(REVERT_REASON_MAX_CHARS, 64);
    let exact = "x".repeat(64);
    assert_eq!(reason_of(&exact).as_deref(), Some(exact.as_str()));

    let long = "y".repeat(200);
    let cut = reason_of(&long).expect("a reason");
    assert_eq!(cut.chars().count(), 64);
    assert!(cut.ends_with('…'));
    assert_eq!(cut, format!("{}…", "y".repeat(63)));

    // Characters, not bytes: 70 three-byte characters cut to 64 characters.
    let wide = "界".repeat(70);
    let cut = reason_of(&wide).expect("a reason");
    assert_eq!(cut.chars().count(), 64);

    // The cut never leaves a dangling space before the ellipsis.
    let spaced = format!("{} tail of the message", "z".repeat(62));
    assert_eq!(reason_of(&spaced), Some(format!("{}…", "z".repeat(62))));
}

/// Every bound of the ABI is checked: a length past the data, an offset past
/// it, invalid UTF-8 — no reason, never a panic.
#[test]
fn a_malformed_error_string_is_no_reason() {
    let selector = hex(&ERROR_STRING_SELECTOR);
    let too_long = format!(
        "0x{selector}{:064x}{:064x}{}",
        32,
        1_000,
        hex(b"short").to_owned() + &"0".repeat(54)
    );
    assert_eq!(revert_reason(&reverted_call(&too_long)), None);
    let bad_offset = format!("0x{selector}{:064x}{:064x}", 4_096, 5);
    assert_eq!(revert_reason(&reverted_call(&bad_offset)), None);
    let huge_offset = format!("0x{selector}{}{:064x}", "f".repeat(64), 5);
    assert_eq!(revert_reason(&reverted_call(&huge_offset)), None);
    let invalid_utf8 = format!("0x{selector}{:064x}{:064x}ff{}", 32, 1, "0".repeat(62));
    assert_eq!(revert_reason(&reverted_call(&invalid_utf8)), None);
    assert_eq!(
        revert_reason(&reverted_call(&format!("0x{selector}"))),
        None
    );
    assert_eq!(revert_reason(&json!({})), None);
}

/// Review of 082 G: the corpus engine expands `$t(key)` in a rendered
/// sentence AFTER `{{reason}}` is filled in, so a reason carrying `$t(` would
/// print a wallet sentence of the contract's choosing inside the danger line
/// (RG8: the text is the contract's, drawn on a signing sheet). Such a reason
/// is no reason — the danger stays, in the plain words.
#[test]
fn a_reason_that_would_name_a_wallet_sentence_is_no_reason() {
    for text in [
        "$t(componentsUi.signing.simUnavailableWarning)",
        "ok $t(send.title) tail",
        // No closing parenthesis: a locale's template may supply one.
        "$t(componentsUi.signing.simUnavailableWarning",
        // Characters the sanitiser drops must not assemble one either.
        "$\u{200B}t(componentsUi.signing.simUnavailableWarning)",
        "$\u{202E}t\u{FEFF}(x)",
    ] {
        assert_eq!(reason_of(text), None, "{text:?}");
        let outcome = classify(result(vec![reverted_call(&error_string(text))]), ME);
        assert_eq!(outcome, SimOutcome::Reverts { reason: None }, "{text:?}");
        assert_eq!(
            notice(&outcome).map(|n| (n.risk, n.key)),
            Some((ClearRisk::Danger, KEY_WILL_FAIL)),
            "{text:?}"
        );
    }
    // A dollar sign, a `t` and a parenthesis apart are ordinary text.
    assert_eq!(
        reason_of("min $5 (t) per call").as_deref(),
        Some("min $5 (t) per call")
    );
    assert_eq!(reason_of("$ t(x)").as_deref(), Some("$ t(x)"));
}

// ---------------------------------------------------------------------------
// The netting (moved from the desktop; vectors from iOS and Android)
// ---------------------------------------------------------------------------

/// iOS `transfersNetPerTokenInFirstSeenOrder`.
#[test]
fn transfers_net_per_token_in_first_seen_order() {
    let deltas = deltas_of(&[
        transfer(USDC, ME, USDC, 30),
        transfer(USDC, USDC, ME, 10),
        transfer(NATIVE_SENTINEL, USDC, ME, 7),
    ]);
    assert_eq!(
        deltas,
        vec![
            TrustAssetDelta {
                kind: TrustDeltaKind::Erc20,
                token: Some(USDC.to_lowercase()),
                delta: "-20".to_owned(),
            },
            // The sentinel is the NATIVE coin, not a contract.
            TrustAssetDelta {
                kind: TrustDeltaKind::Native,
                token: None,
                delta: "7".to_owned(),
            },
        ]
    );
}

/// Android `deltas net per token, the sentinel is the native coin, zero and
/// strangers drop`.
#[test]
fn deltas_net_and_zero_and_strangers_drop() {
    let deltas = deltas_of(&[
        transfer(NATIVE_SENTINEL, ME, OTHER, 1_000),
        transfer(USDC, OTHER, ME, 12_000_000),
        transfer(USDC, ME, OTHER, 2_000_000),
        transfer(USDC, OTHER, "0x1111111111111111111111111111111111111111", 5),
        transfer(USDC, ME, ME, 7),
        transfer(USDC, OTHER, ME, 0),
    ]);
    assert_eq!(deltas.len(), 2);
    assert_eq!(deltas[0].kind, TrustDeltaKind::Native);
    assert_eq!(deltas[0].delta, "-1000");
    assert_eq!(deltas[1].kind, TrustDeltaKind::Erc20);
    assert_eq!(
        deltas[1].token.as_deref(),
        Some(USDC.to_lowercase().as_str())
    );
    assert_eq!(deltas[1].delta, "10000000");
}

/// iOS `aMoveThatCancelsIsNotAMove`: in and out of the same token is how a
/// swap looks like a theft.
#[test]
fn a_move_that_cancels_is_not_a_move() {
    assert!(deltas_of(&[transfer(USDC, ME, USDC, 12), transfer(USDC, USDC, ME, 12)]).is_empty());
}

/// iOS `transfersThatMissTheWalletAreIgnored`.
#[test]
fn transfers_that_miss_the_wallet_are_ignored() {
    let other = "0x1111111111111111111111111111111111111111";
    assert!(deltas_of(&[transfer(USDC, other, USDC, 5)]).is_empty());
}

/// iOS `onlyWellFormedTransfersCount`: not a `Transfer`, too few topics, no
/// address, no data word, a topic that is not an address.
#[test]
fn only_well_formed_transfers_count() {
    let logs = [
        json!({ "address": USDC, "topics": ["0xdeadbeef", topic(ME), topic(USDC)], "data": word(5) }),
        json!({ "address": USDC, "topics": [TRANSFER_TOPIC, topic(ME)], "data": word(5) }),
        json!({ "address": "", "topics": [TRANSFER_TOPIC, topic(USDC), topic(ME)], "data": word(5) }),
        json!({ "address": USDC, "topics": [TRANSFER_TOPIC, topic(USDC), topic(ME)], "data": "0x" }),
        json!({ "address": USDC, "topics": [TRANSFER_TOPIC, "0x00", topic(ME)], "data": word(5) }),
        json!({ "address": USDC, "topics": [TRANSFER_TOPIC, topic(USDC), topic(ME)], "data": format!("0x{}", "z".repeat(64)) }),
        json!("not a log"),
    ];
    assert!(deltas_of(&logs).is_empty());
    // An empty user owns nothing — a malformed topic must not match it.
    let refs: Vec<&Value> = logs.iter().collect();
    assert!(derive_deltas(&refs, "").is_empty());
}

/// The topic's case does not matter, nor the user's.
#[test]
fn case_does_not_matter() {
    let mut log = transfer(USDC, OTHER, ME, 3);
    log["topics"][0] = json!(TRANSFER_TOPIC.to_uppercase().replacen("0X", "0x", 1));
    let refs = vec![&log];
    assert_eq!(
        derive_deltas(&refs, &ME.to_uppercase().replacen("0X", "0x", 1))[0].delta,
        "3"
    );
}

/// iOS `hexWordsDecode`: only the first 32-byte word is the value.
#[test]
fn only_the_first_word_is_the_value() {
    let mut log = transfer(USDC, OTHER, ME, 0);
    log["data"] = json!(format!("0x{}ff{}", "0".repeat(62), "f".repeat(64)));
    assert_eq!(deltas_of(&[log])[0].delta, "255");
    let mut one_eth = transfer(USDC, OTHER, ME, 0);
    one_eth["data"] = json!("0x0000000000000000000000000000000000000000000000000de0b6b3a7640000");
    assert_eq!(deltas_of(&[one_eth])[0].delta, "1000000000000000000");
}

/// iOS `arithmeticSurvivesAWordSizedValue`: exact past 128 bits (the desktop
/// copy saturated at `i128`).
#[test]
fn a_word_sized_value_is_exact() {
    let max = format!("0x{}", "f".repeat(64));
    let mut incoming = transfer(USDC, OTHER, ME, 0);
    incoming["data"] = json!(max);
    assert_eq!(
        deltas_of(&[incoming.clone()])[0].delta,
        "115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
    let mut outgoing = transfer(USDC, ME, OTHER, 0);
    outgoing["data"] = json!(max);
    assert_eq!(
        deltas_of(&[outgoing.clone()])[0].delta,
        "-115792089237316195423570985008687907853269984665640564039457584007913129639935"
    );
    assert!(deltas_of(&[incoming, outgoing]).is_empty(), "nets to zero");
}

// ---------------------------------------------------------------------------
// The words
// ---------------------------------------------------------------------------

/// Every key a notice hands a shell is in the corpus, and the reason key
/// interpolates `{{reason}}`.
#[cfg(feature = "i18n-en")]
#[test]
fn the_notice_keys_are_in_the_corpus() {
    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [KEY_UNAVAILABLE, KEY_WILL_FAIL, KEY_WILL_FAIL_REASON] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
    let template = i18n.t(KEY_WILL_FAIL_REASON, &opts).expect("the sentence");
    assert!(template.contains("{{reason}}"), "{template}");
}

/// Review of 082 G, end to end: whatever a reverting contract writes, the
/// drawn line never contains a wallet sentence it did not ask for.
#[cfg(feature = "i18n-en")]
#[test]
fn a_revert_reason_cannot_draw_a_wallet_sentence() {
    use vela_core::i18n::{Options, Var};
    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let unavailable = i18n
        .t(KEY_UNAVAILABLE, &Options::default())
        .expect("the caution sentence");
    let outcome = classify(
        result(vec![reverted_call(&error_string(&format!(
            "$t({KEY_UNAVAILABLE})"
        )))]),
        ME,
    );
    let line = notice(&outcome).expect("a notice");
    let drawn = match line.reason.as_deref() {
        Some(reason) => {
            let vars = [("reason", Var::Str(reason))];
            i18n.t(
                line.key,
                &Options {
                    vars: &vars,
                    ..Options::default()
                },
            )
        }
        None => i18n.t(line.key, &Options::default()),
    }
    .expect("the drawn line");
    assert!(!drawn.contains(&unavailable), "{drawn}");
    assert_eq!(line.risk, ClearRisk::Danger);
}
