//! The reading every platform draws (r8-1, r8-2, r8-5), and the shared
//! vectors (`tests/vectors/multisig/reading.json`) a native shell's parity
//! test replays through `review.reading`.

use super::*;
use crate::multisig::{self as ms, SafeTx};

const SAFE: &str = "0x0e96ddc491878645dB7E3a47d2a6Ccbc94f17f32";
const ARC: u64 = 5_042_002;
const ARC_USDC: &str = "0x3600000000000000000000000000000000000000";
const ODD: &str = "0x00000000000000000000000000000000000b0b0b";
const SPENDER: &str = "0x5555555555555555555555555555555555555555";
const TO: &str = "0x4444444444444444444444444444444444444444";
const ZERO: &str = "0x0000000000000000000000000000000000000000";

fn tx(to: &str, value: &str, data: &str, operation: u8) -> SafeTx {
    SafeTx {
        to: to.into(),
        value: value.into(),
        data: data.into(),
        operation,
        safe_tx_gas: "0x0".into(),
        base_gas: "0x0".into(),
        gas_price: "0x0".into(),
        gas_token: ZERO.into(),
        refund_receiver: ZERO.into(),
        nonce: "0x0".into(),
    }
}

fn read_tx(chain: u64, t: &SafeTx, facts: &[TokenFacts]) -> Reading {
    let review = ms::review(chain, SAFE, t).unwrap();
    let tokens = facts
        .iter()
        .map(|f| (f.token.clone().unwrap(), f.clone()))
        .collect();
    read(chain, t, &review, &tokens)
}

fn vectors() -> serde_json::Value {
    let path = format!(
        "{}/tests/vectors/multisig/reading.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_reading_vectors_hold() {
    let mut v = vectors();
    if std::env::var("VELA_WRITE_READING_VECTORS").is_ok() {
        for case in v["cases"].as_array_mut().unwrap() {
            let got = crate::multisig_rules::call_json("review.reading", &case["args"])
                .unwrap()
                .unwrap();
            case["expect"] = serde_json::from_str(&got).unwrap();
        }
        let path = format!(
            "{}/tests/vectors/multisig/reading.json",
            env!("CARGO_MANIFEST_DIR")
        );
        std::fs::write(path, serde_json::to_string_pretty(&v).unwrap() + "\n").unwrap();
    }
    let cases = v["cases"].as_array().unwrap();
    assert!(cases.len() >= 10);
    for case in cases {
        let name = case["name"].as_str().unwrap();
        let got = crate::multisig_rules::call_json("review.reading", &case["args"])
            .unwrap()
            .unwrap();
        let got: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got, case["expect"], "{name}");
    }
}

#[test]
fn arc_usdc_is_known_with_six_decimals() {
    let data = format!("0xa9059cbb{:0>64}{:064x}", &TO[2..], 10_000u64);
    let r = read_tx(ARC, &tx(ARC_USDC, "0", &data, 0), &[]);
    let Intent::Send { token, amount, .. } = &r.calls[0].intent else {
        panic!("{r:?}")
    };
    assert_eq!(token.symbol.as_deref(), Some("USDC"));
    assert_eq!(amount.plain.as_deref(), Some("0.01"));
    assert!(r.decimals_unknown.is_empty());
}

#[test]
fn unknown_decimals_are_never_assumed() {
    let data = format!("0xa9059cbb{:0>64}{:064x}", &TO[2..], 10_000u64);
    let r = read_tx(ARC, &tx(ODD, "0", &data, 0), &[]);
    let Intent::Send { amount, .. } = &r.calls[0].intent else {
        panic!()
    };
    assert_eq!(amount.raw, "10000");
    assert_eq!(amount.decimals, None);
    assert_eq!(amount.plain, None);
    assert_eq!(r.decimals_unknown, vec![ODD.to_owned()]);
    assert!(concerns(&r).contains(&Concern::DecimalsUnknown { token: ODD.into() }));
}

#[test]
fn an_unlimited_approve_is_a_danger_with_or_without_token_facts() {
    let data = format!("0x095ea7b3{:0>64}{}", &SPENDER[2..], "f".repeat(64));
    for facts in [
        vec![],
        vec![TokenFacts {
            token: Some(ODD.into()),
            symbol: Some("ODD".into()),
            decimals: Some(6),
            known: false,
        }],
    ] {
        let r = read_tx(ARC, &tx(ODD, "0", &data, 0), &facts);
        assert!(r.unlimited);
        let Intent::Approve { amount, grant, .. } = &r.calls[0].intent else {
            panic!()
        };
        assert!(amount.unlimited);
        assert_eq!(*grant, Grant::Approve);
        assert!(concerns(&r)
            .iter()
            .any(|c| matches!(c, Concern::UnlimitedApproval { .. })));
    }
}

#[test]
fn a_batch_nested_too_deep_is_not_read() {
    // multiSend(multiSend(multiSend(multiSend(transfer)))) called, four deep.
    let transfer = format!("a9059cbb{:0>64}{:064x}", &TO[2..], 1u64);
    let mut inner = transfer;
    for _ in 0..4 {
        let leg = format!(
            "00{}{:064x}{:064x}{}",
            &ms::MULTI_SEND_CALL_ONLY[2..],
            0,
            inner.len() / 2,
            inner
        );
        let pad = (64 - (leg.len() % 64)) % 64;
        inner = format!(
            "8d80ff0a{:064x}{:064x}{}{}",
            0x20,
            leg.len() / 2,
            leg,
            "0".repeat(pad)
        );
    }
    let r = read_tx(ARC, &tx(ARC_USDC, "0", &format!("0x{inner}"), 0), &[]);
    assert!(r.unreadable, "{r:?}");
    assert!(concerns(&r).contains(&Concern::UnreadableBatch));
}

#[test]
fn plain_units_is_exact() {
    assert_eq!(plain_units(U256::from(10_000u64), 6), "0.01");
    assert_eq!(plain_units(U256::from(1_500_000u64), 6), "1.5");
    assert_eq!(plain_units(U256::from(7u64), 0), "7");
    assert_eq!(plain_units(U256::from(1u64), 18), "0.000000000000000001");
}
