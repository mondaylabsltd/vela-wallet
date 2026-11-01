//! r11-8 / r11-9: the shared vectors every shell's parity test replays —
//! `vectors/fee/plain-send.json` (the same plain send priced and charged the
//! same on every shell) and `vectors/rules/transport-failures.json` ("not
//! sent" versus "may have been sent", from the error class a shell passes).

#![cfg(feature = "crux")]

mod support;

use serde_json::Value;
use support::DomainDriver;
use vela_core::app::fee_policy::{
    estimate_gas_outcome, Event, FeeFailure, FeeGasOutcome, FeeOperation as Op, FeePolicy,
    FeeShellResult as Res,
};
use vela_core::app::rpc_pool::{
    may_have_delivered, transport_outcome_of, RpcTransportOutcome, TransportFailure,
};
use vela_core::user_op::{submit_step, SubmitReply, SubmitStep, SubmitVerdict};

const PLAIN_SEND: &str = include_str!("vectors/fee/plain-send.json");
const TRANSPORT: &str = include_str!("vectors/rules/transport-failures.json");

fn parse<T: serde::de::DeserializeOwned>(value: &Value) -> T {
    serde_json::from_value(value.clone()).unwrap_or_else(|e| panic!("{e}: {value}"))
}

fn is_deadline(op: &Op) -> bool {
    matches!(op, Op::StartDeadline { .. })
}

#[test]
fn the_same_plain_send_is_priced_one_way() {
    let root: Value = serde_json::from_str(PLAIN_SEND).expect("vector parses");
    let request: Event = parse(&root["request"]);
    let gas: Res = parse(&root["gas_price"]["shell_result"]);
    let bundler: Res = parse(&root["bundler_quote"]["shell_result"]);
    let quotes: Res = parse(&root["in_band_quotes"]["shell_result"]);
    for case in root["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().unwrap_or_default();
        let outcome: FeeGasOutcome = parse(&case["user_op_gas"]);
        // The relay's body means this outcome — the core's rule, not a
        // shell's (`estimate_gas_outcome`).
        let wire = &case["estimate_wire"];
        if wire.is_null() {
            assert_eq!(estimate_gas_outcome(None), outcome, "{name}");
        } else if let Some(error) = wire.get("error") {
            assert_eq!(
                estimate_gas_outcome(Some(&error.to_string())),
                outcome,
                "{name}: the error member"
            );
            assert_eq!(
                estimate_gas_outcome(Some(&wire.to_string())),
                outcome,
                "{name}: the whole body"
            );
        }

        let mut sut: DomainDriver<FeePolicy> = DomainDriver::new();
        sut.dispatch(request.clone());
        sut.drop_matching(is_deadline);
        sut.resolve_matching(|op| matches!(op, Op::FetchGasPrice { .. }), gas.clone());
        sut.resolve_matching(
            |op| matches!(op, Op::FetchBundlerQuote { .. }),
            bundler.clone(),
        );
        let ops = sut.resolve_matching(
            |op| matches!(op, Op::FetchInBandQuotes { .. }),
            quotes.clone(),
        );
        assert!(
            ops.iter()
                .any(|op| matches!(op, Op::EstimateUserOpGas { .. })),
            "{name}: {ops:?}"
        );
        sut.drop_matching(is_deadline);
        sut.resolve_matching(
            |op| matches!(op, Op::EstimateUserOpGas { .. }),
            Res::UserOpGas { outcome },
        );
        let view = sut.view();
        let expect = &case["expect"];
        let failed: Option<FeeFailure> = parse(&expect["failed"]);
        assert_eq!(view.failed, failed, "{name}");
        if expect.get("fee").is_some_and(Value::is_null) {
            assert!(view.fee.is_none(), "{name}: {:?}", view.fee);
            continue;
        }
        let fee = view.fee.unwrap_or_else(|| panic!("{name}: a fee"));
        assert_eq!(
            fee.total_gas,
            expect["total_gas"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            fee.in_band_gas_basis,
            expect["in_band_gas_basis"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            fee.total_wei,
            expect["total_wei"].as_str().unwrap(),
            "{name}"
        );
        let prepared = fee
            .prepared
            .unwrap_or_else(|| panic!("{name}: the limits it was priced on"));
        let want = &expect["prepared"];
        assert_eq!(
            prepared.verification_gas_limit,
            want["verification_gas_limit"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            prepared.call_gas_limit,
            want["call_gas_limit"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            prepared.pre_verification_gas,
            want["pre_verification_gas"].as_str().unwrap(),
            "{name}"
        );
        assert_eq!(
            prepared.settlement_gas.as_deref(),
            want["settlement_gas"].as_str(),
            "{name}"
        );
        // What is signed is what was priced: the prepared limits sum to the
        // gas the fee was charged on (a relay without `settlementGas`).
        let sum: u128 = [
            &prepared.verification_gas_limit,
            &prepared.call_gas_limit,
            &prepared.pre_verification_gas,
        ]
        .iter()
        .map(|g| g.parse::<u128>().unwrap())
        .sum();
        assert_eq!(sum.to_string(), fee.total_gas, "{name}");
    }
}

#[test]
fn an_error_class_decides_not_sent_or_maybe_sent() {
    let root: Value = serde_json::from_str(TRANSPORT).expect("vector parses");
    let cases = root["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 11, "every class is in the vector");
    for case in cases {
        let failure: TransportFailure = parse(&case["failure"]);
        let outcome: RpcTransportOutcome = parse(&case["outcome"]);
        assert_eq!(transport_outcome_of(failure), outcome, "{failure:?}");
        let delivered = may_have_delivered(&outcome);
        assert_eq!(
            delivered,
            case["may_have_delivered"].as_bool().unwrap(),
            "{failure:?}"
        );
        let verdict = match submit_step(&SubmitReply::NoAnswer, 0, delivered, "0xlocal") {
            SubmitStep::Done(SubmitVerdict::NotSent { rejection: None }) => "not_sent",
            SubmitStep::Done(SubmitVerdict::MaybeSent { .. }) => "maybe_sent",
            other => panic!("{failure:?}: {other:?}"),
        };
        assert_eq!(verdict, case["submit"].as_str().unwrap(), "{failure:?}");
    }
}
