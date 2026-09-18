//! Signing in when the index service is gone — the registry contract read
//! directly, in the index's own words (spec 062).
//!
//! A passkey finds its wallet through two questions: *which units did this key
//! found?* and *who are that unit's members, and what does its metadata say?*
//! Every shell asks them of the index service, which is a cache of the registry
//! contract. When the index does not answer, sign-in stops — even though the
//! answers are sitting on Gnosis, and, for a person who backed their record up,
//! on Ethereum too. A backup nothing can read is not a backup.
//!
//! This module is the contract's side of those two questions:
//!
//! ```text
//! /api/query?publicKey=K   ⇔  hasEntry(K) + getGroupsOfKey(K, 0, 8, newest first)
//! /api/query?unitId=N      ⇔  getUnit(N)  + getGroupMembers(N, 0, 7, founding order)
//! ```
//!
//! and it answers in the **index's JSON shapes**, so a shell's existing
//! parsing, guards and error handling run unchanged on either source — the
//! fallback is a different transport, never a second implementation of login.
//!
//! Unit ids are per DEPLOYMENT: unit 10 on Gnosis is unit 0 on Ethereum. A
//! shell must ask about a key's units on the chain that listed them.

use alloy_dyn_abi::DynSolValue;
use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::primitives;
use crate::registry_backup::{
    self, call_data, decode, public_key_bytes, REGISTRY, SIG_GET_UNIT, SIG_GROUPS_OF_KEY,
    UNIT_TUPLE,
};

/// Where the record lives, then where a person may have copied it. The order
/// a shell tries them in once the index has not answered.
pub const READ_CHAINS: [u32; 2] = [registry_backup::SOURCE_CHAIN, registry_backup::TARGET_CHAIN];

/// A wallet's founding set is at most seven keys (`safe::MAX_MULTI_KEYS`); a
/// key founds few wallets and the newest are the ones worth a call.
const MAX_MEMBERS: u64 = 7;
const MAX_UNITS: u64 = 8;

const SIG_HAS_ENTRY: &str = "hasEntry(bytes)";
const SIG_GROUP_MEMBERS: &str = "getGroupMembers(uint256,uint256,uint256,bool)";
/// `Entry { publicKey, attestation, credentialId, authenticatorAttachment, transports, createdAt }`.
const ENTRY_TUPLE: &str = "(bytes,bytes,bytes,bytes,bytes,uint256)";

/// One `eth_call` against the registry. The shell performs it on whichever
/// [`READ_CHAINS`] entry it is trying and hands the raw `result` hex back.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainCall {
    pub to: String,
    pub data: String,
}

fn registry_call(signature: &str, args: &[DynSolValue]) -> Option<ChainCall> {
    Some(ChainCall {
        to: REGISTRY.to_owned(),
        data: call_data(signature, args)?,
    })
}

fn bytes_of(hex: &str) -> Option<Vec<u8>> {
    let bytes = primitives::from_hex(hex).ok()?;
    // No contract at the registry's address answers `0x`: that chain cannot
    // say anything, which is different from saying "none".
    (!bytes.is_empty()).then_some(bytes)
}

/// The two calls behind `?publicKey=`: `[hasEntry, getGroupsOfKey]`.
#[must_use]
pub fn key_status_calls(public_key_hex: &str) -> Option<Vec<ChainCall>> {
    let key = public_key_bytes(public_key_hex)?;
    Some(vec![
        registry_call(SIG_HAS_ENTRY, &[DynSolValue::Bytes(key.clone())])?,
        registry_call(
            SIG_GROUPS_OF_KEY,
            &[
                DynSolValue::Bytes(key),
                DynSolValue::Uint(U256::ZERO, 256),
                DynSolValue::Uint(U256::from(MAX_UNITS), 256),
                DynSolValue::Bool(true), // newest first, as the index lists them
            ],
        )?,
    ])
}

/// The index's `?publicKey=` body from the two results, or `None` when either
/// is not the contract's answer (the shell then tries the next chain).
#[must_use]
pub fn key_status_json(has_entry_hex: &str, groups_hex: &str) -> Option<String> {
    let registered = decode("(bool)", &bytes_of(has_entry_hex)?)?
        .first()?
        .as_bool()?;
    let groups = decode("(uint256,uint256[])", &bytes_of(groups_hex)?)?;
    let total = groups
        .first()?
        .as_uint()
        .and_then(|(n, _)| u64::try_from(n).ok())?;
    let unit_ids: Vec<u64> = groups
        .get(1)?
        .as_array()?
        .iter()
        .filter_map(|id| id.as_uint().and_then(|(n, _)| u64::try_from(n).ok()))
        .collect();
    Some(
        json!({
            // The index sends the entry's fields; every reader asks only
            // whether there IS one.
            "entry": if registered { json!({}) } else { serde_json::Value::Null },
            "groups": { "total": total, "unitIds": unit_ids },
        })
        .to_string(),
    )
}

/// The two calls behind `?unitId=`: `[getUnit, getGroupMembers]`, members in
/// ascending (founding) order — the order the Safe address derivation pins.
#[must_use]
pub fn unit_calls(unit_id: u64) -> Option<Vec<ChainCall>> {
    Some(vec![
        registry_call(SIG_GET_UNIT, &[DynSolValue::Uint(U256::from(unit_id), 256)])?,
        registry_call(
            SIG_GROUP_MEMBERS,
            &[
                DynSolValue::Uint(U256::from(unit_id), 256),
                DynSolValue::Uint(U256::ZERO, 256),
                DynSolValue::Uint(U256::from(MAX_MEMBERS), 256),
                DynSolValue::Bool(false),
            ],
        )?,
    ])
}

fn text(value: &DynSolValue) -> Option<String> {
    String::from_utf8(value.as_bytes()?.to_vec()).ok()
}

fn bare_hex(value: &DynSolValue) -> Option<String> {
    Some(primitives::to_hex(value.as_bytes()?, false))
}

/// The index's `?unitId=` body from the two results. `None` when either is not
/// the contract's answer, or a member cannot be read — a partial founding set
/// would rebuild a DIFFERENT, fundable, wrong address, so it is never offered.
#[must_use]
pub fn unit_json(unit_id: u64, unit_hex: &str, members_hex: &str) -> Option<String> {
    let unit = decode(&format!("({UNIT_TUPLE})"), &bytes_of(unit_hex)?)?;
    let unit = unit.first()?.as_tuple()?;
    let members = decode(
        &format!("(uint256,uint256[],{ENTRY_TUPLE}[])"),
        &bytes_of(members_hex)?,
    )?;
    let total = members
        .first()?
        .as_uint()
        .and_then(|(n, _)| u64::try_from(n).ok())?;
    let entry_ids = members.get(1)?.as_array()?;
    let entries = members.get(2)?.as_array()?;
    if entries.len() != entry_ids.len() || entries.len() as u64 != total {
        return None;
    }
    let mut items = Vec::with_capacity(entries.len());
    for (entry, entry_id) in entries.iter().zip(entry_ids) {
        let fields = entry.as_tuple()?;
        items.push(json!({
            "entryId": entry_id.as_uint().and_then(|(n, _)| u64::try_from(n).ok())?,
            "publicKey": bare_hex(fields.first()?)?,
            "attestation": bare_hex(fields.get(1)?)?,
            "credentialId": bare_hex(fields.get(2)?)?,
            "authenticatorAttachment": text(fields.get(3)?)?,
            "transports": text(fields.get(4)?)?,
        }));
    }
    Some(
        json!({
            "unit": {
                "unitId": unit_id,
                "rpId": unit.first()?.as_str()?,
                "metadata": bare_hex(unit.get(1)?)?,
                "groupPublicKey": bare_hex(unit.get(2)?)?,
                "memberCount": total,
            },
            "members": { "total": total, "items": items },
        })
        .to_string(),
    )
}

// ---------------------------------------------------------------------------
// JSON doors, for the bindings
// ---------------------------------------------------------------------------

/// `{"chains":[100,1],"calls":[{to,data},{to,data}]}` for a public key, or
/// `None` for a key this build cannot read.
#[must_use]
pub fn key_status_plan_json(public_key_hex: &str) -> Option<String> {
    Some(json!({ "chains": READ_CHAINS, "calls": key_status_calls(public_key_hex)? }).to_string())
}

/// The same for a unit.
#[must_use]
pub fn unit_plan_json(unit_id: u64) -> Option<String> {
    Some(json!({ "chains": READ_CHAINS, "calls": unit_calls(unit_id)? }).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::FixedBytes;

    fn hex_of(values: Vec<DynSolValue>) -> String {
        primitives::to_hex(&DynSolValue::Tuple(values).abi_encode_params(), true)
    }
    fn key() -> String {
        format!("04{}{}", "19".repeat(32), "fe".repeat(32))
    }
    fn entry(tag: u8, attachment: &str, transports: &str) -> DynSolValue {
        DynSolValue::Tuple(vec![
            DynSolValue::Bytes([vec![0x04], vec![tag; 64]].concat()),
            DynSolValue::Bytes(vec![1; 20]),
            DynSolValue::Bytes(vec![tag; 16]),
            DynSolValue::Bytes(attachment.as_bytes().to_vec()),
            DynSolValue::Bytes(transports.as_bytes().to_vec()),
            DynSolValue::Uint(U256::from(1), 256),
        ])
    }
    fn unit_hex() -> String {
        hex_of(vec![DynSolValue::Tuple(vec![
            DynSolValue::String("getvela.app".to_owned()),
            DynSolValue::Bytes(b"{\"version\":1}".to_vec()),
            DynSolValue::Bytes(vec![4; 65]),
            DynSolValue::FixedBytes(FixedBytes::<32>::ZERO, 32),
            DynSolValue::Uint(U256::from(2), 32),
            DynSolValue::Uint(U256::from(1), 256),
        ])])
    }
    fn members_hex(total: u64, entries: Vec<DynSolValue>) -> String {
        let ids = (0..entries.len() as u64)
            .map(|i| DynSolValue::Uint(U256::from(40 + i), 256))
            .collect();
        hex_of(vec![
            DynSolValue::Uint(U256::from(total), 256),
            DynSolValue::Array(ids),
            DynSolValue::Array(entries),
        ])
    }

    #[test]
    fn the_calls_are_the_deployed_contracts() {
        let calls = key_status_calls(&key()).unwrap_or_default();
        assert_eq!(calls.len(), 2);
        assert!(calls.iter().all(|call| call.to == REGISTRY));
        // Selectors checked against the live Gnosis registry (spec 062).
        assert!(
            calls[0].data.starts_with("0x2776d098"),
            "hasEntry: {}",
            calls[0].data
        );
        assert!(calls[1].data.starts_with("0xcc7aae8e"), "getGroupsOfKey");
        let calls = unit_calls(10).unwrap_or_default();
        assert!(calls[0].data.starts_with("0x1655bfbe"), "getUnit");
        assert!(
            calls[1].data.starts_with("0x00a54450"),
            "getGroupMembers: {}",
            calls[1].data
        );
        assert_eq!(key_status_calls("04abcd"), None);
    }

    #[test]
    fn a_key_status_is_the_indexs_own_shape() {
        let groups = hex_of(vec![
            DynSolValue::Uint(U256::from(3), 256),
            DynSolValue::Array(
                [12u64, 10, 8]
                    .iter()
                    .map(|n| DynSolValue::Uint(U256::from(*n), 256))
                    .collect(),
            ),
        ]);
        assert_eq!(
            key_status_json(&hex_of(vec![DynSolValue::Bool(true)]), &groups).as_deref(),
            Some(r#"{"entry":{},"groups":{"total":3,"unitIds":[12,10,8]}}"#)
        );
        let none = hex_of(vec![
            DynSolValue::Uint(U256::ZERO, 256),
            DynSolValue::Array(vec![]),
        ]);
        assert_eq!(
            key_status_json(&hex_of(vec![DynSolValue::Bool(false)]), &none).as_deref(),
            Some(r#"{"entry":null,"groups":{"total":0,"unitIds":[]}}"#)
        );
        // `0x` = no registry on that chain: not an answer.
        assert_eq!(key_status_json("0x", &none), None);
        assert_eq!(
            key_status_json(&hex_of(vec![DynSolValue::Bool(true)]), "0x1234"),
            None
        );
    }

    #[test]
    fn a_unit_carries_every_member_in_founding_order_or_nothing() {
        let body = unit_json(
            10,
            &unit_hex(),
            &members_hex(
                2,
                vec![
                    entry(0xaa, "platform", "internal"),
                    entry(0xbb, "cross-platform", "usb"),
                ],
            ),
        )
        .unwrap_or_default();
        let value: serde_json::Value = serde_json::from_str(&body).unwrap_or_default();
        assert_eq!(
            value["unit"]["metadata"],
            primitives::to_hex(b"{\"version\":1}", false)
        );
        assert_eq!(value["members"]["total"], 2);
        let items = value["members"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        assert_eq!(items[0]["publicKey"], format!("04{}", "aa".repeat(64)));
        assert_eq!(items[0]["credentialId"], "aa".repeat(16));
        assert_eq!(items[0]["authenticatorAttachment"], "platform");
        assert_eq!(items[1]["transports"], "usb");
        assert_eq!(items[1]["entryId"], 41);

        // A page that holds fewer members than the unit has would rebuild the
        // wrong address: refused, never degraded.
        assert_eq!(
            unit_json(
                10,
                &unit_hex(),
                &members_hex(3, vec![entry(0xaa, "platform", "internal")])
            ),
            None
        );
        assert_eq!(unit_json(10, "0x", &members_hex(0, vec![])), None);
    }

    #[test]
    fn the_plans_name_the_chains_in_the_order_they_are_tried() {
        let plan = key_status_plan_json(&key()).unwrap_or_default();
        assert!(
            plan.starts_with(r#"{"calls":["#) || plan.contains(r#""chains":[100,1]"#),
            "{plan}"
        );
        assert!(unit_plan_json(3)
            .unwrap_or_default()
            .contains(r#""chains":[100,1]"#));
    }
}
