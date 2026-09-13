//! Multicall3 encoding for the native read path (spec 051 research D1).
//!
//! `aggregate3((address,bool,bytes)[])` is a dynamic array of tuples with a
//! dynamic member — head/tail offsets nested two deep. The bridge's existing
//! `abi_encode_address` / `_uint256` / `_bytes32` cannot express that, and a
//! hand-rolled encoder is correct right up until the first tuple with two
//! dynamic members. `alloy-dyn-abi` already knows how.
//!
//! **Why this lives in `vela-core-uniffi` and not in `vela-core`**: the wasm
//! crate does not link this one, so the web bundle pays nothing — and a change
//! under `vela-core` would force a `rust/pkg-web` rebuild and commit while
//! another session holds the web client. That is a scheduling reason, and it is
//! recorded as a consolidation debt rather than dressed up as a design: desktop
//! has its own copy in `executor/abi.rs`, and one shared home in `vela-core` is
//! the better end state.

use alloy_dyn_abi::{DynSolType, DynSolValue};
use alloy_primitives::Address;
use std::str::FromStr;

use crate::CoreError;

/// One call inside an `aggregate3` batch.
#[derive(uniffi::Record)]
pub struct Multicall3Call {
    /// `0x`-prefixed contract address.
    pub target: String,
    /// Let the batch continue when this one reverts.
    ///
    /// The balance reader sets it: a token that reverts on `balanceOf` — a
    /// proxy mid-upgrade, a contract that is not really an ERC-20 — must not
    /// cost the other eleven their answer.
    pub allow_failure: bool,
    pub call_data: Vec<u8>,
}

/// One result out of an `aggregate3` batch, in the order the calls went in.
#[derive(uniffi::Record)]
pub struct Multicall3Result {
    pub success: bool,
    pub return_data: Vec<u8>,
}

/// Build `aggregate3` calldata.
#[uniffi::export]
pub fn multicall3_encode_aggregate3(calls: Vec<Multicall3Call>) -> Result<Vec<u8>, CoreError> {
    let tuples = calls
        .into_iter()
        .map(|call| {
            let target = Address::from_str(&call.target)
                .map_err(|error| CoreError::Internal(format!("bad target: {error}")))?;
            Ok(DynSolValue::Tuple(vec![
                DynSolValue::Address(target),
                DynSolValue::Bool(call.allow_failure),
                DynSolValue::Bytes(call.call_data),
            ]))
        })
        .collect::<Result<Vec<_>, CoreError>>()?;

    // Derived from the signature rather than pasted: a typo is then a parse
    // error here instead of a batch that reverts on-chain.
    let mut out = vela_core::primitives::function_selector("aggregate3((address,bool,bytes)[])")?;
    out.extend_from_slice(&DynSolValue::Array(tuples).abi_encode_params());
    Ok(out)
}

/// Decode what `aggregate3` returned: `(bool success, bytes returnData)[]`.
///
/// A failed entry **keeps its slot**. The caller matches results to calls by
/// index, and compacting the failures away here would silently misalign every
/// token after the first revert — which is the kind of defect that shows up as
/// one person's balance appearing under another token's name.
#[uniffi::export]
pub fn multicall3_decode_aggregate3(data: Vec<u8>) -> Result<Vec<Multicall3Result>, CoreError> {
    let kind = DynSolType::Array(Box::new(DynSolType::Tuple(vec![
        DynSolType::Bool,
        DynSolType::Bytes,
    ])));
    let decoded = kind
        .abi_decode_params(&data)
        .map_err(|error| CoreError::Internal(format!("aggregate3 return: {error}")))?;

    let DynSolValue::Array(entries) = decoded else {
        return Err(CoreError::Internal(
            "aggregate3 return was not an array".to_string(),
        ));
    };

    entries
        .into_iter()
        .map(|entry| match entry {
            DynSolValue::Tuple(fields) => match fields.as_slice() {
                [DynSolValue::Bool(success), DynSolValue::Bytes(bytes)] => Ok(Multicall3Result {
                    success: *success,
                    return_data: bytes.clone(),
                }),
                _ => Err(CoreError::Internal("aggregate3 entry shape".to_string())),
            },
            _ => Err(CoreError::Internal(
                "aggregate3 entry was not a tuple".to_string(),
            )),
        })
        .collect()
}

/// `balanceOf(address)` calldata — what an `aggregate3` batch is almost always
/// made of.
#[uniffi::export]
pub fn erc20_encode_balance_of(owner_hex: String) -> Result<Vec<u8>, CoreError> {
    let mut out = vela_core::primitives::function_selector("balanceOf(address)")?;
    out.extend_from_slice(&vela_core::primitives::abi_encode_address(&owner_hex)?);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical Multicall3 selector, so a signature typo cannot pass.
    #[test]
    fn aggregate3_uses_the_canonical_selector() {
        let data = multicall3_encode_aggregate3(vec![]).unwrap();
        assert_eq!(&data[..4], &[0x82, 0xad, 0x56, 0xcb]);
    }

    /// `balanceOf(address)` is `0x70a08231`, and the argument is left-padded.
    #[test]
    fn balance_of_encodes_selector_and_padded_owner() {
        let data =
            erc20_encode_balance_of("0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_string())
                .unwrap();
        assert_eq!(&data[..4], &[0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(data.len(), 36);
        assert_eq!(&data[4..16], &[0u8; 12]);
    }

    /// A batch survives its own round trip, and a **failed** entry keeps its
    /// slot — the property the caller's index-matching depends on.
    #[test]
    fn a_failed_entry_keeps_its_slot() {
        let encoded = DynSolValue::Array(vec![
            DynSolValue::Tuple(vec![
                DynSolValue::Bool(true),
                DynSolValue::Bytes(vec![1, 2, 3]),
            ]),
            DynSolValue::Tuple(vec![DynSolValue::Bool(false), DynSolValue::Bytes(vec![])]),
            DynSolValue::Tuple(vec![DynSolValue::Bool(true), DynSolValue::Bytes(vec![9])]),
        ])
        .abi_encode_params();

        let out = multicall3_decode_aggregate3(encoded).unwrap();
        assert_eq!(out.len(), 3);
        assert!(!out[1].success);
        assert_eq!(out[2].return_data, vec![9]);
    }

    #[test]
    fn a_bad_target_is_refused_rather_than_encoded() {
        let result = multicall3_encode_aggregate3(vec![Multicall3Call {
            target: "not an address".to_string(),
            allow_failure: true,
            call_data: vec![],
        }]);
        assert!(result.is_err());
    }
}
