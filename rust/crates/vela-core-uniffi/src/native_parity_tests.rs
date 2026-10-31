//! F0.2: the uniffi exports answer what the web's wasm exports answer.
//!
//! `vela-core/tests/vectors/native-parity/exports.json` holds, for every
//! export the native apps take over from the web, inputs as the uniffi
//! function takes them and the answer the SHIPPED wasm gave
//! (`rust/scripts/native-parity-wasm.mjs`; `verify-web.mjs` replays the file
//! against the wasm on every run). Here each case goes through the uniffi
//! function — or, for a rule the natives already had under another name, the
//! native route to it — and must land on the very same value. One `#[test]`
//! per export, so a drift names the export it is in.

use serde_json::{json, Value};

use crate::rules_bridge as rules;
use crate::signing_bridge as signing;
use crate::trusted_signer_bridge::{TrustedSignerOutcome, TrustedSignerRefusal};
use crate::{intents_bridge as intents, WalletKeyRecord};

const CORPUS: &str = include_str!("../../vela-core/tests/vectors/native-parity/exports.json");

fn corpus() -> Vec<Value> {
    let doc: Value = serde_json::from_str(CORPUS).expect("native-parity corpus reads");
    assert_eq!(doc["suite"], "native-parity");
    doc["cases"].as_array().expect("cases").clone()
}

fn s(input: &Value, key: &str) -> String {
    input[key]
        .as_str()
        .unwrap_or_else(|| panic!("input `{key}` is not a string: {input}"))
        .to_owned()
}

fn opt_s(input: &Value, key: &str) -> Option<String> {
    input[key].as_str().map(str::to_owned)
}

fn u32_of(input: &Value, key: &str) -> u32 {
    u32::try_from(
        input[key]
            .as_u64()
            .unwrap_or_else(|| panic!("input `{key}`")),
    )
    .expect("u32 input")
}

fn b(input: &Value, key: &str) -> bool {
    input[key]
        .as_bool()
        .unwrap_or_else(|| panic!("input `{key}`"))
}

fn strings(input: &Value, key: &str) -> Vec<String> {
    input[key]
        .as_array()
        .unwrap_or_else(|| panic!("input `{key}`"))
        .iter()
        .map(|v| v.as_str().expect("string item").to_owned())
        .collect()
}

fn keys(input: &Value) -> Vec<WalletKeyRecord> {
    input["keys"]
        .as_array()
        .expect("keys")
        .iter()
        .map(|k| WalletKeyRecord {
            credential_id: s(k, "credential_id"),
            public_key_hex: s(k, "public_key_hex"),
        })
        .collect()
}

/// JSON text as the wasm table reduces it: parsed, `""` kept as `""`.
fn json_text(text: &str) -> Value {
    if text.is_empty() {
        return Value::String(String::new());
    }
    serde_json::from_str(text).unwrap_or_else(|e| panic!("not JSON ({e}): {text}"))
}

fn opt_json(text: Option<String>) -> Value {
    text.map_or(Value::Null, |t| json_text(&t))
}

fn opt_text(text: Option<String>) -> Value {
    text.map_or(Value::Null, Value::String)
}

fn hex(bytes: &[u8]) -> String {
    vela_core::primitives::to_hex(bytes, false).to_ascii_lowercase()
}

fn code(refusal: &TrustedSignerRefusal) -> &'static str {
    match refusal {
        TrustedSignerRefusal::Declined => "declined",
        TrustedSignerRefusal::PageRefused { .. } => "refused",
        TrustedSignerRefusal::WrongChallenge => "wrong_challenge",
        TrustedSignerRefusal::ForeignKey => "foreign_key",
        TrustedSignerRefusal::BadSignature => "bad_signature",
        TrustedSignerRefusal::NotVerified => "not_verified",
        TrustedSignerRefusal::WrongToken => "wrong_token",
        TrustedSignerRefusal::Malformed { .. } => "malformed",
    }
}

/// One case through the uniffi surface, reduced to the vector's shape.
#[allow(clippy::too_many_lines)]
fn run(func: &str, i: &Value) -> Value {
    match func {
        // --- where a signature goes ---
        "signing_route" => opt_json(signing::signing_route(s(i, "account_json"), s(i, "method"))),
        "signing_venue_block" => opt_json(signing::signing_venue_block(
            s(i, "domain"),
            s(i, "venue_json"),
        )),
        "multisig_approval_route" => {
            opt_json(signing::multisig_approval_route(s(i, "account_json")))
        }

        // --- the trusted signing page ---
        // Already native: `trusted_signer_request` (typed input, no draft for a message).
        "trusted_signer_signature_request" => {
            let input = crate::trusted_signer_bridge::TrustedSignerInput {
                method: s(i, "method"),
                params_json: s(i, "params_json"),
                origin: s(i, "origin"),
                origin_seen_by_browser: b(i, "origin_seen_by_browser"),
                chain_id: u32_of(i, "chain_id"),
                chain_name: opt_s(i, "chain_name"),
                native_symbol: opt_s(i, "native_symbol"),
                account: s(i, "account"),
                account_name: opt_s(i, "account_name"),
                credential_ids_hex: strings(i, "credential_ids_hex"),
                key_route_json: opt_s(i, "key_route_json"),
                anyway: false,
                calls: Vec::new(),
            };
            json_text(
                &crate::trusted_signer_bridge::trusted_signer_request(input, None)
                    .expect("request builds"),
            )
        }
        "trusted_signer_signature_accept" => {
            let digest = vela_core::primitives::from_hex(&s(i, "digest_hex")).expect("digest hex");
            match signing::trusted_signer_signature_accept(
                s(i, "request_json"),
                s(i, "result_json"),
                keys(i),
                digest,
            ) {
                TrustedSignerOutcome::Accepted {
                    credential_id_hex,
                    assertion,
                } => json!({ "accepted": {
                    "credential_id_hex": credential_id_hex.trim_start_matches("0x").to_ascii_lowercase(),
                    "signature_der_hex": hex(&assertion.signature_der),
                    "authenticator_data_hex": hex(&assertion.authenticator_data),
                    "client_data_json_hex": hex(&assertion.client_data_json),
                } }),
                TrustedSignerOutcome::Refused { refusal } => json!({ "refused": code(&refusal) }),
            }
        }
        "trusted_signer_approval_request" => json_text(
            &signing::trusted_signer_approval_request(signing::TrustedSignerApprovalInput {
                chain_id: i["chain_id"].as_u64().expect("chain_id"),
                chain_name: opt_s(i, "chain_name"),
                native_symbol: opt_s(i, "native_symbol"),
                multisig: s(i, "multisig"),
                multisig_name: opt_s(i, "multisig_name"),
                tx_json: s(i, "tx_json"),
                member: s(i, "member"),
                member_name: opt_s(i, "member_name"),
                credential_ids_hex: strings(i, "credential_ids_hex"),
                key_route_json: opt_s(i, "key_route_json"),
            })
            .expect("approval request builds"),
        ),
        "trusted_signer_approval_accept" => match signing::trusted_signer_approval_accept(
            s(i, "result_json"),
            i["chain_id"].as_u64().expect("chain_id"),
            s(i, "member"),
            s(i, "safe_tx_hash"),
            keys(i),
        ) {
            signing::TrustedSignerApprovalOutcome::Accepted { approval_json } => {
                json!({ "accepted": json_text(&approval_json) })
            }
            signing::TrustedSignerApprovalOutcome::Refused { refusal } => {
                json!({ "refused": code(&refusal) })
            }
        },

        // --- how a dApp request is answered ---
        "sign_error_words" => opt_text(signing::sign_error_words(s(i, "kind"), opt_s(i, "detail"))),
        "sign_not_confirmed_detail" => {
            Value::String(signing::sign_not_confirmed_detail(s(i, "user_op_hash")))
        }
        "sign_answered" => opt_text(signing::sign_answered(s(i, "payload_json"))),
        "sign_request_ttl_ms" => json!(signing::sign_request_ttl_ms()),

        // --- the agent ---
        "intent_agent_reads" => json_text(&intents::intent_agent_reads(
            s(i, "world_json"),
            opt_s(i, "operation"),
        )),
        "intent_advice" => json_text(&intents::intent_advice(s(i, "input_json"))),
        "intent_advice_standing" => json_text(&intents::intent_advice_standing(s(i, "input_json"))),

        // --- the read path ---
        "rpc_own_only_pool" => json_text(&rules::rpc_own_only_pool(
            s(i, "seeds_json"),
            b(i, "own_only"),
        )),
        "rpc_hedge_after_ms" => json!(rules::rpc_hedge_after_ms()),
        "rpc_is_hedged_read" => json!(rules::rpc_is_hedged_read(s(i, "kind"), s(i, "method"))),
        "rpc_early_verdict" => json_text(&rules::rpc_early_verdict(
            s(i, "kind"),
            s(i, "method"),
            s(i, "url"),
            s(i, "outcome_json"),
        )),
        "rpc_read_timeout_ms" => json!(rules::rpc_read_timeout_ms()),
        "rpc_cooldown_ms" => json!(rules::rpc_cooldown_ms(u32_of(i, "consecutive_failures"))),

        // --- fees and gas ---
        "min_gas_price_wei" => Value::String(rules::min_gas_price_wei(u32_of(i, "chain_id"))),
        "fee_quote_deadline_ms" => json!(rules::fee_quote_deadline_ms()),
        // Already native: `network_typical_inclusion_s` (`None` where the web says 0).
        "typical_inclusion_seconds" => {
            json!(crate::network_typical_inclusion_s(u32_of(i, "chain_id")).unwrap_or(0))
        }
        // Already native, as the draft's steps: `user_op_floors` →
        // `user_op_apply_estimate` → the inner calls' floor (which the phones
        // raise through `user_op_raise_call_gas`, the same
        // `inner_calls_gas_floor` the web's `innerCallsGasFloor` reads).
        "user_op_gas_limits" => {
            let floors = crate::user_op_floors(
                u32_of(i, "chain_id"),
                b(i, "deployed"),
                u32_of(i, "sub_calls"),
            );
            let draft = crate::UserOpDraft {
                sender: "0x1111111111111111111111111111111111111111".into(),
                nonce: "0".into(),
                init_code: Vec::new(),
                call_data: Vec::new(),
                verification_gas_limit: "0".into(),
                call_gas_limit: "0".into(),
                pre_verification_gas: "0".into(),
                max_fee_per_gas: "0".into(),
                max_priority_fee_per_gas: "0".into(),
                paymaster_and_data: Vec::new(),
                signature: Vec::new(),
            };
            let applied = crate::user_op_apply_estimate(
                draft,
                s(i, "verification_gas_limit"),
                s(i, "call_gas_limit"),
                s(i, "pre_verification_gas"),
                opt_s(i, "settlement_gas"),
                floors,
            )
            .expect("estimate applies");
            let call: u128 = applied.call_gas_limit.parse().expect("call gas");
            let floor: u128 = opt_s(i, "inner_floor").map_or(0, |f| f.parse().expect("floor"));
            json!({
                "verificationGasLimit": applied.verification_gas_limit,
                "callGasLimit": call.max(floor).to_string(),
                "preVerificationGas": applied.pre_verification_gas,
            })
        }

        // --- dApp RPC ---
        "dapp_rpc_classify" => json_text(&rules::dapp_rpc_classify(s(i, "method"))),
        "dapp_rpc_capabilities" => json_text(&rules::dapp_rpc_capabilities(
            s(i, "params_json"),
            s(i, "granted_json"),
            s(i, "chains_json"),
        )),
        "dapp_rpc_calls_status" => json_text(&rules::dapp_rpc_calls_status(
            s(i, "id"),
            u32_of(i, "chain_id"),
            opt_s(i, "receipt_json"),
            opt_s(i, "relay_status_json"),
        )),

        // --- tokens, sites, names, currencies ---
        "token_provenance" => json_text(&rules::token_provenance(
            u32_of(i, "chain_id"),
            s(i, "token"),
            s(i, "symbol"),
            s(i, "native_symbol"),
        )),
        "site_start_chain" => json!(rules::site_start_chain(
            i["space_chain_ids"]
                .as_array()
                .expect("space_chain_ids")
                .iter()
                .map(|v| u32::try_from(v.as_u64().expect("chain id")).expect("u32"))
                .collect(),
            i["last_used"]
                .as_u64()
                .map(|v| u32::try_from(v).expect("u32")),
        )),
        "resolved_name_step" => json_text(&rules::resolved_name_step(
            u32_of(i, "chain_id"),
            s(i, "registry"),
            s(i, "name"),
            s(i, "answers_json"),
        )),
        "currency_fraction_digits" => json!(rules::currency_fraction_digits(s(i, "code"))),

        // --- core asks 1 ---
        "classify_relay_rejection" => match crate::classify_relay_rejection(s(i, "message")) {
            crate::RelayRejection::RelayerUnavailable => json!("relayer_unavailable"),
            crate::RelayRejection::BundlerUnderfunded => json!("bundler_underfunded"),
            crate::RelayRejection::NonceUsed => json!("nonce_used"),
            crate::RelayRejection::NonceHeld { user_op_hash } => {
                json!({ "nonce_held": { "user_op_hash": user_op_hash } })
            }
            crate::RelayRejection::Other { message } => json!({ "other": message }),
        },
        "is_localhost_http" => json!(rules::is_localhost_http(s(i, "url"))),
        "passkey_ceremony_end" => {
            use signing::PasskeyCeremonyEnd as E;
            json!(match signing::passkey_ceremony_end(
                s(i, "kind"),
                i["elapsed_ms"].as_f64().expect("elapsed_ms")
            ) {
                E::Cancelled => "cancelled",
                E::TimedOut => "timed_out",
                E::NotSupported => "not_supported",
                E::NotDiscoverable => "not_discoverable",
                E::Other => "other",
            })
        }
        "passkey_sign_timeout_ms" => json!(signing::passkey_sign_timeout_ms()),
        "rpc_node_standing" => json!(rules::rpc_node_standing(
            s(i, "view_json"),
            u32_of(i, "chain_id")
        )),
        "sim_outcome" => {
            let record = crate::sim_outcome(s(i, "user"), s(i, "reply_json"));
            json!({
                "kind": record.kind,
                "deltas": json_text(&record.deltas_json),
                "revert_reason": record.revert_reason,
                "notice_risk": record.notice_risk,
                "notice_key": record.notice_key,
                "no_change_key": record.no_change_key,
                "recheck": record.recheck,
            })
        }
        "builtin_network_display_name" => {
            json!(rules::builtin_network_display_name(u32_of(i, "chain_id")))
        }
        "builtin_network_ids" => json!(rules::builtin_network_ids()),
        other => panic!("no uniffi arm for `{other}` — add it to native_parity_tests.rs"),
    }
}

/// JSON values equal as data: numbers by value (the wasm's `30000` is the
/// uniffi `30000.0`), objects by key whatever their order.
fn same(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => x.as_f64() == y.as_f64(),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len() && x.iter().all(|(k, v)| y.get(k).is_some_and(|w| same(v, w)))
        }
        _ => a == b,
    }
}

fn replay(func: &str) {
    let cases: Vec<Value> = corpus().into_iter().filter(|c| c["fn"] == func).collect();
    assert!(!cases.is_empty(), "the corpus has no case for `{func}`");
    let mut failures = Vec::new();
    for case in &cases {
        let got = run(func, &case["input"]);
        let want = &case["expect"]["value"];
        if !same(&got, want) {
            failures.push(format!(
                "{}\n  wasm:   {want}\n  uniffi: {got}",
                case["name"]
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} `{func}` cases differ from the wasm:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

macro_rules! parity {
    ($($func:ident),* $(,)?) => {
        $(
            #[test]
            fn $func() {
                replay(stringify!($func));
            }
        )*

        /// Every function the corpus names has its test above — a case for
        /// an export nobody replays would be a silent hole.
        #[test]
        fn every_corpus_function_is_replayed() {
            let tested = [$(stringify!($func)),*];
            for case in corpus() {
                let func = case["fn"].as_str().expect("fn");
                assert!(tested.contains(&func), "`{func}` has cases but no parity test");
            }
        }
    };
}

parity!(
    signing_route,
    signing_venue_block,
    multisig_approval_route,
    trusted_signer_signature_request,
    trusted_signer_signature_accept,
    trusted_signer_approval_request,
    trusted_signer_approval_accept,
    sign_error_words,
    sign_not_confirmed_detail,
    sign_answered,
    sign_request_ttl_ms,
    intent_agent_reads,
    intent_advice,
    intent_advice_standing,
    rpc_own_only_pool,
    rpc_hedge_after_ms,
    rpc_is_hedged_read,
    rpc_early_verdict,
    rpc_read_timeout_ms,
    rpc_cooldown_ms,
    min_gas_price_wei,
    fee_quote_deadline_ms,
    typical_inclusion_seconds,
    user_op_gas_limits,
    dapp_rpc_classify,
    dapp_rpc_capabilities,
    dapp_rpc_calls_status,
    token_provenance,
    site_start_chain,
    resolved_name_step,
    currency_fraction_digits,
    classify_relay_rejection,
    is_localhost_http,
    passkey_ceremony_end,
    passkey_sign_timeout_ms,
    rpc_node_standing,
    sim_outcome,
    builtin_network_display_name,
    builtin_network_ids,
);

/// The refusals the corpus does not reach by a page's answer: input a shell
/// hands over that does not read is `Malformed`, never a panic or an accept.
#[test]
fn unreadable_page_answers_are_malformed() {
    let refused = |outcome: TrustedSignerOutcome| match outcome {
        TrustedSignerOutcome::Refused { refusal } => code(&refusal),
        TrustedSignerOutcome::Accepted { .. } => "accepted",
    };
    assert_eq!(
        refused(signing::trusted_signer_signature_accept(
            "{".into(),
            "{}".into(),
            Vec::new(),
            vec![0; 32]
        )),
        "malformed"
    );
    assert_eq!(
        refused(signing::trusted_signer_signature_accept(
            r#"{"intent":{"method":"personal_sign"},"context":{"chainId":100}}"#.into(),
            "not json".into(),
            Vec::new(),
            vec![0; 32]
        )),
        "malformed"
    );
    assert!(matches!(
        signing::trusted_signer_approval_accept(
            "[".into(),
            100,
            "0x1111111111111111111111111111111111111111".into(),
            format!("0x{}", "ab".repeat(32)),
            Vec::new()
        ),
        signing::TrustedSignerApprovalOutcome::Refused {
            refusal: TrustedSignerRefusal::Malformed { .. }
        }
    ));
    assert!(
        signing::trusted_signer_approval_request(signing::TrustedSignerApprovalInput {
            chain_id: 100,
            chain_name: None,
            native_symbol: None,
            multisig: "0x1111111111111111111111111111111111111111".into(),
            multisig_name: None,
            tx_json: "{}".into(),
            member: "0x1111111111111111111111111111111111111111".into(),
            member_name: None,
            credential_ids_hex: Vec::new(),
            key_route_json: None,
            anyway: false,
        })
        .is_err()
    );
}
