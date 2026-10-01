//! Chain vectors for `user_op::user_op_hash` (spec 082 T011, RA6 — the
//! phase-0 gate).
//!
//! Two operations that landed on Gnosis during the 082 device pass while the
//! relay's reply was lost (G21): the extension's dust send (nonce 22) and the
//! desktop's (the fixture Safe, nonce 41). The fixture holds each
//! `handleOps` transaction's calldata and the `UserOperationEvent` it emitted,
//! fetched once, read-only. The test decodes the calldata back into the
//! `UserOperation` a client would have built and asserts the local hash is the
//! EntryPoint's own — the event's `topics[1]`. If this ever fails, a client
//! would follow the wrong hash after a lost reply, so it is the one vector
//! that must never be regenerated from the code under test.

use alloy_dyn_abi::{DynSolType, DynSolValue};
use serde_json::Value;
use vela_core::primitives::{from_hex, to_hex};
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::{user_op_hash, UserOperation, USER_OPERATION_EVENT_TOPIC};

const FIXTURE: &str = include_str!("fixtures/userop-hash-gnosis.json");
/// `handleOps(PackedUserOperation[],address)`.
const HANDLE_OPS_SELECTOR: &str = "765e827f";
const PACKED_OPS: &str = "(address,uint256,bytes,bytes,bytes32,uint256,bytes32,bytes,bytes)[]";

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap_or_else(|e| unreachable!("fixture: {e}"))
}

fn text<'v>(value: &'v Value, key: &str) -> &'v str {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| unreachable!("fixture field `{key}`"))
}

/// The high and low `uint128` halves of a packed v0.7 word.
fn halves(word: &[u8]) -> (u128, u128) {
    let mut high = [0u8; 16];
    let mut low = [0u8; 16];
    high.copy_from_slice(&word[..16]);
    low.copy_from_slice(&word[16..32]);
    (u128::from_be_bytes(high), u128::from_be_bytes(low))
}

/// One decoded `PackedUserOperation`, back in the v0.6 shape the core builds.
fn unpack(tuple: &DynSolValue) -> UserOperation {
    let DynSolValue::Tuple(fields) = tuple else {
        unreachable!("not a tuple: {tuple:?}")
    };
    let [sender, nonce, init_code, call_data, account_gas_limits, pre_verification_gas, gas_fees, paymaster_and_data, signature] =
        fields.as_slice()
    else {
        unreachable!("PackedUserOperation has nine fields, got {}", fields.len())
    };
    let bytes = |value: &DynSolValue| match value {
        DynSolValue::Bytes(b) => b.clone(),
        other => unreachable!("not bytes: {other:?}"),
    };
    let word = |value: &DynSolValue| match value {
        DynSolValue::FixedBytes(w, 32) => w.to_vec(),
        other => unreachable!("not bytes32: {other:?}"),
    };
    let uint = |value: &DynSolValue| match value {
        DynSolValue::Uint(n, 256) => *n,
        other => unreachable!("not uint256: {other:?}"),
    };
    let DynSolValue::Address(sender) = sender else {
        unreachable!("sender is not an address")
    };
    let (verification_gas_limit, call_gas_limit) = halves(&word(account_gas_limits));
    let (max_priority_fee_per_gas, max_fee_per_gas) = halves(&word(gas_fees));
    // v0.7 packs `paymaster ‖ uint128 ‖ uint128 ‖ data`; the core's shape is
    // `paymaster ‖ data` with both gas limits sent as zero, so a vector with
    // non-zero limits could not have come from this wallet.
    let packed = bytes(paymaster_and_data);
    let paymaster_and_data = if packed.is_empty() {
        Vec::new()
    } else {
        assert!(packed.len() >= 52, "short paymasterAndData");
        assert!(
            packed[20..52].iter().all(|b| *b == 0),
            "paymaster gas limits the wallet never sends"
        );
        let mut unpacked = packed[..20].to_vec();
        unpacked.extend_from_slice(&packed[52..]);
        unpacked
    };
    UserOperation {
        sender: sender.to_checksum(None),
        nonce: format!("0x{:x}", uint(nonce)),
        init_code: bytes(init_code),
        call_data: bytes(call_data),
        verification_gas_limit,
        call_gas_limit,
        pre_verification_gas: u128::try_from(uint(pre_verification_gas))
            .unwrap_or_else(|e| unreachable!("{e}")),
        max_fee_per_gas,
        max_priority_fee_per_gas,
        paymaster_and_data,
        signature: bytes(signature),
    }
}

/// Every operation in one `handleOps` calldata.
fn decode_handle_ops(input: &str) -> Vec<UserOperation> {
    let calldata = from_hex(input).unwrap_or_else(|e| unreachable!("{e}"));
    assert_eq!(to_hex(&calldata[..4], false), HANDLE_OPS_SELECTOR);
    let params = DynSolType::Tuple(vec![
        PACKED_OPS
            .parse::<DynSolType>()
            .unwrap_or_else(|e| unreachable!("{e}")),
        DynSolType::Address,
    ]);
    let decoded = params
        .abi_decode_params(&calldata[4..])
        .unwrap_or_else(|e| unreachable!("handleOps: {e}"));
    let DynSolValue::Tuple(top) = decoded else {
        unreachable!("params are a tuple")
    };
    let Some(DynSolValue::Array(ops)) = top.first() else {
        unreachable!("the first param is the ops array")
    };
    ops.iter().map(unpack).collect()
}

#[test]
fn the_local_hash_is_the_entry_point_s_own_for_both_gnosis_operations() {
    let fixture = fixture();
    let chain_id = fixture
        .get("chain_id")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| unreachable!("chain_id"));
    assert_eq!(chain_id, 100);
    assert!(text(&fixture, "entry_point").eq_ignore_ascii_case(ENTRY_POINT));
    assert_eq!(
        text(&fixture, "user_operation_event_topic"),
        USER_OPERATION_EVENT_TOPIC
    );

    let vectors = fixture
        .get("vectors")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("vectors"));
    assert_eq!(vectors.len(), 2, "both G21 operations are pinned");
    for vector in vectors {
        let tx = text(vector, "tx_hash");
        assert!(text(vector, "to").eq_ignore_ascii_case(ENTRY_POINT), "{tx}");
        let event = vector
            .get("user_operation_event")
            .unwrap_or_else(|| unreachable!("{tx}: event"));
        let topics: Vec<&str> = event
            .get("topics")
            .and_then(Value::as_array)
            .unwrap_or_else(|| unreachable!("{tx}: topics"))
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert_eq!(topics[0], USER_OPERATION_EVENT_TOPIC, "{tx}");

        let ops = decode_handle_ops(text(vector, "input"));
        assert_eq!(ops.len(), 1, "{tx}: one operation per bundle");
        let op = &ops[0];
        // The event names the same sender the calldata carries.
        assert_eq!(
            format!("0x{:0>64}", op.sender[2..].to_lowercase()),
            topics[2],
            "{tx}"
        );
        let hash = user_op_hash(op, chain_id).unwrap_or_else(|e| unreachable!("{tx}: {e}"));
        assert_eq!(hash, topics[1], "{tx}: the EntryPoint's userOpHash");
    }
}

/// The nonces the device pass recorded (22 for the extension, 41 for the
/// desktop): the decoder reads the operations it is meant to.
#[test]
fn the_vectors_are_the_two_operations_the_device_pass_recorded() {
    let fixture = fixture();
    let vectors = fixture
        .get("vectors")
        .and_then(Value::as_array)
        .unwrap_or_else(|| unreachable!("vectors"));
    let nonces: Vec<(u64, String)> = vectors
        .iter()
        .map(|vector| {
            let ops = decode_handle_ops(text(vector, "input"));
            let block = vector
                .get("block_number")
                .and_then(Value::as_u64)
                .unwrap_or_else(|| unreachable!("block_number"));
            (block, ops[0].nonce.clone())
        })
        .collect();
    assert_eq!(
        nonces,
        vec![
            (48_478_729, "0x16".to_owned()),
            (48_479_132, "0x29".to_owned())
        ]
    );
}
