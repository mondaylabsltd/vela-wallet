//! Two readings of a multisig transaction that every member must see said
//! the same way, so they are rules here rather than each shell's guess:
//!
//! - **What it changes about who controls the multisig**
//!   ([`member_changes`]). Safe's own owner-management functions —
//!   `addOwnerWithThreshold`, `removeOwner`, `swapOwner`, `changeThreshold` —
//!   called on the Safe itself, alone or inside a MultiSendCallOnly batch, as
//!   [`crate::multisig::review`] decoded them. Each change is read against
//!   the members it will meet — the network's members now, and for a batch
//!   every earlier change in it — as Safe applies it: who is added, removed or
//!   replaced, the rule before and after ("2 of 3 → 2 of 4"), and whether the
//!   Safe would accept it at all (adding a member twice, removing a
//!   non-member, a threshold above the members left all revert).
//! - **What an executed transaction's own receipt says**
//!   ([`execution_facts`]). Safe L2's `SafeMultiSigTransaction` event carries
//!   the signatures `execTransaction` checked and `(nonce, msg.sender,
//!   threshold)` as they were then. The approvals that counted are the owners
//!   of the first `threshold` signatures — Safe reads no more — and the
//!   executor is `msg.sender`. An executed transaction reads these frozen
//!   facts, never today's owners re-counted (an owner removed since did
//!   approve it; an executor that approved by sending it counts once).

use alloy_primitives::{keccak256, Address, B256, U256};
use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::multisig::{safe_tx_hash, Action, SafeTx};
use crate::primitives;

fn err(message: impl Into<String>) -> CoreError {
    CoreError::Multisig(message.into())
}

fn addr(s: &str) -> Result<Address, CoreError> {
    s.trim()
        .parse::<Address>()
        .map_err(|_| err(format!("not an address: {s}")))
}

// ---------------------------------------------------------------------------
// Member changes
// ---------------------------------------------------------------------------

/// "M of N": how many approvals a transaction needs, of how many members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rule {
    pub threshold: u64,
    pub owners: u64,
}

/// Which of Safe's four owner-management functions a change is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChangeKind {
    /// `addOwnerWithThreshold(owner, threshold)`.
    Add,
    /// `removeOwner(prev, owner, threshold)`.
    Remove,
    /// `swapOwner(prev, old, new)`: one member replaced by another.
    Replace,
    /// `changeThreshold(threshold)`.
    Threshold,
}

/// One change to the multisig's members or threshold, read against the
/// members it meets.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberChange {
    /// Its call's number in a batch (from 1); `None` when the transaction
    /// is this change alone.
    pub call: Option<u32>,
    pub kind: ChangeKind,
    /// The member it adds (an add, or a replace's new member), checksummed.
    pub added: Option<String>,
    /// The member it removes (a remove, or a replace's old member).
    pub removed: Option<String>,
    /// The rule it meets, and the rule it leaves.
    pub before: Rule,
    pub after: Rule,
    /// Whether Safe accepts it against the members it meets. `false`: the
    /// transaction would revert there — said, never drawn as a change that
    /// will happen.
    pub fits: bool,
}

/// Every member change a transaction makes, in order, with the rule the
/// transaction meets and the one it leaves.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemberChanges {
    pub changes: Vec<MemberChange>,
    pub before: Rule,
    pub after: Rule,
    /// Every change fits.
    pub fits: bool,
}

/// A threshold as an action carries it (decimal); one past `u64` is no
/// threshold any Safe accepts.
fn threshold_of(text: &str) -> Option<u64> {
    U256::from_str_radix(text.trim(), 10)
        .ok()
        .and_then(|v| u64::try_from(v).ok())
}

/// The member changes `action` makes to a multisig whose members are
/// `owners` with `threshold` — the network's members now, read from its
/// chain — or `None` when it changes none (a transfer, a call, a batch
/// without one). A batch's changes are read one after another, each meeting
/// the members the earlier ones left.
pub fn member_changes(
    action: &Action,
    owners: &[String],
    threshold: u64,
) -> Result<Option<MemberChanges>, CoreError> {
    let mut members = owners
        .iter()
        .map(|o| addr(o))
        .collect::<Result<Vec<_>, _>>()?;
    let mut rule = Rule {
        threshold,
        owners: members.len() as u64,
    };
    let start = rule;
    let legs: Vec<(Option<u32>, &Action)> = match action {
        Action::Batch { calls } => calls
            .iter()
            .enumerate()
            .map(|(i, c)| (Some(u32::try_from(i + 1).unwrap_or(u32::MAX)), &c.action))
            .collect(),
        one => vec![(None, one)],
    };
    let mut changes = Vec::new();
    for (call, leg) in legs {
        let before = rule;
        let has = |members: &[Address], a: Address| members.contains(&a);
        let (kind, added, removed, fits) = match leg {
            Action::AddOwner { owner, threshold } => {
                let owner = addr(owner)?;
                let t = threshold_of(threshold);
                let fits = !owner.is_zero() && !has(&members, owner);
                if fits {
                    members.push(owner);
                }
                let count = members.len() as u64;
                let fits = fits && t.is_some_and(|t| t >= 1 && t <= count);
                rule = Rule {
                    threshold: t.unwrap_or(rule.threshold),
                    owners: count,
                };
                (ChangeKind::Add, Some(owner), None, fits)
            }
            Action::RemoveOwner { owner, threshold } => {
                let owner = addr(owner)?;
                let t = threshold_of(threshold);
                let fits = has(&members, owner);
                members.retain(|m| *m != owner);
                let count = members.len() as u64;
                let fits = fits && t.is_some_and(|t| t >= 1 && t <= count);
                rule = Rule {
                    threshold: t.unwrap_or(rule.threshold),
                    owners: count,
                };
                (ChangeKind::Remove, None, Some(owner), fits)
            }
            Action::SwapOwner {
                old_owner,
                new_owner,
            } => {
                let (old, new) = (addr(old_owner)?, addr(new_owner)?);
                let fits = has(&members, old) && !has(&members, new) && !new.is_zero();
                if fits {
                    if let Some(slot) = members.iter_mut().find(|m| **m == old) {
                        *slot = new;
                    }
                }
                (ChangeKind::Replace, Some(new), Some(old), fits)
            }
            Action::ChangeThreshold { threshold } => {
                let t = threshold_of(threshold);
                let fits = t.is_some_and(|t| t >= 1 && t <= rule.owners);
                rule.threshold = t.unwrap_or(rule.threshold);
                (ChangeKind::Threshold, None, None, fits)
            }
            _ => continue,
        };
        changes.push(MemberChange {
            call,
            kind,
            added: added.map(|a| a.to_checksum(None)),
            removed: removed.map(|a| a.to_checksum(None)),
            before,
            after: rule,
            fits,
        });
    }
    if changes.is_empty() {
        return Ok(None);
    }
    let fits = changes.iter().all(|c| c.fits);
    Ok(Some(MemberChanges {
        changes,
        before: start,
        after: rule,
        fits,
    }))
}

// ---------------------------------------------------------------------------
// An executed transaction's frozen facts
// ---------------------------------------------------------------------------

/// `SafeMultiSigTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes,bytes)`
/// — Safe L2's record of every `execTransaction`, with the signatures it
/// checked and `abi.encode(nonce, msg.sender, threshold)`.
const SAFE_MULTISIG_TRANSACTION: &str =
    "SafeMultiSigTransaction(address,uint256,bytes,uint8,uint256,uint256,uint256,address,address,bytes,bytes)";
/// `ExecutionSuccess(bytes32 txHash, uint256 payment)`, the hash indexed.
const EXECUTION_SUCCESS: &str = "ExecutionSuccess(bytes32,uint256)";

/// One log of a transaction receipt, as the node gave it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

/// What a multisig transaction was when it was executed, from its own
/// receipt: who sent it, the threshold then, and the members whose approvals
/// counted (in the order Safe checked them; the executor among them when it
/// approved by sending it).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFacts {
    pub executor: String,
    pub threshold: u64,
    pub nonce: String,
    pub approvers: Vec<String>,
    /// The executor approved by sending it (`v = 1` from `msg.sender`).
    pub executor_approved: bool,
}

fn word(data: &[u8], index: usize) -> Option<&[u8]> {
    data.get(index * 32..index * 32 + 32)
}

fn word_u256(data: &[u8], index: usize) -> Option<U256> {
    word(data, index).map(U256::from_be_slice)
}

fn word_address(data: &[u8], index: usize) -> Option<Address> {
    let w = word(data, index)?;
    // A canonical address word: nothing in its upper twelve bytes.
    w[..12]
        .iter()
        .all(|b| *b == 0)
        .then(|| Address::from_slice(&w[12..]))
}

/// The `bytes` whose offset is head word `index`.
fn bytes_at(data: &[u8], index: usize) -> Option<&[u8]> {
    let offset: usize = word_u256(data, index)?.try_into().ok()?;
    let len: usize = U256::from_be_slice(data.get(offset..offset + 32)?)
        .try_into()
        .ok()?;
    data.get(offset + 32..offset.checked_add(32)?.checked_add(len)?)
}

fn topic(signature: &str) -> B256 {
    keccak256(signature.as_bytes())
}

fn same_topic(text: Option<&String>, want: B256) -> bool {
    text.and_then(|t| t.parse::<B256>().ok()) == Some(want)
}

/// The owner a 65-byte signature entry stands for, as Safe's
/// `checkNSignatures` reads it: a contract signature (`v = 0`) or an
/// approved hash / the sender (`v = 1`) name it in `r`; an `eth_sign`
/// signature (`v > 30`) recovers over the prefixed hash; any other is
/// `ecrecover` of the hash.
fn signer(entry: &[u8], hash: B256) -> Option<(Address, u8)> {
    let v = entry[64];
    match v {
        0 | 1 => {
            let r = &entry[..32];
            r[..12]
                .iter()
                .all(|b| *b == 0)
                .then(|| (Address::from_slice(&r[12..]), v))
        }
        v if v > 30 => {
            let mut prefixed = b"\x19Ethereum Signed Message:\n32".to_vec();
            prefixed.extend_from_slice(hash.as_slice());
            let mut sig = entry.to_vec();
            sig[64] = v - 4;
            crate::multisig::recover(keccak256(prefixed), &sig)
                .ok()
                .map(|a| (a, v))
        }
        _ => crate::multisig::recover(hash, entry).ok().map(|a| (a, v)),
    }
}

/// The frozen facts of `safe_tx_hash`'s execution by `safe` on `chain_id`,
/// read from the logs of the transaction that executed it, or `None` when
/// they do not say: no `ExecutionSuccess` for it there, or no Safe L2
/// `SafeMultiSigTransaction` whose transaction hashes to it (a non-L2 Safe
/// emits none). Only the logs the multisig itself emitted are read, and the
/// event's own fields must hash to `safe_tx_hash` — a log claiming another
/// transaction's signatures is not this one's.
pub fn execution_facts(
    chain_id: u64,
    safe: &str,
    safe_tx_hash_hex: &str,
    logs: &[ReceiptLog],
) -> Result<Option<ExecutionFacts>, CoreError> {
    let me = addr(safe)?;
    let want: B256 = safe_tx_hash_hex
        .trim()
        .parse()
        .map_err(|_| err("not a safeTxHash"))?;
    let mine = |log: &&ReceiptLog| addr(&log.address).is_ok_and(|a| a == me);
    let succeeded = logs.iter().filter(mine).any(|log| {
        same_topic(log.topics.first(), topic(EXECUTION_SUCCESS))
            && same_topic(log.topics.get(1), want)
    });
    if !succeeded {
        return Ok(None);
    }
    for log in logs.iter().filter(mine) {
        if !same_topic(log.topics.first(), topic(SAFE_MULTISIG_TRANSACTION)) {
            continue;
        }
        let Ok(data) = primitives::from_hex(&log.data) else {
            continue;
        };
        let read = || -> Option<ExecutionFacts> {
            let info = bytes_at(&data, 10)?;
            let nonce = word_u256(info, 0)?;
            let executor = word_address(info, 1)?;
            let threshold = u64::try_from(word_u256(info, 2)?).ok()?;
            let tx = SafeTx {
                to: word_address(&data, 0)?.to_checksum(None),
                value: format!("{:#x}", word_u256(&data, 1)?),
                data: primitives::to_hex(bytes_at(&data, 2)?, true),
                operation: u8::try_from(word_u256(&data, 3)?).ok()?,
                safe_tx_gas: format!("{:#x}", word_u256(&data, 4)?),
                base_gas: format!("{:#x}", word_u256(&data, 5)?),
                gas_price: format!("{:#x}", word_u256(&data, 6)?),
                gas_token: word_address(&data, 7)?.to_checksum(None),
                refund_receiver: word_address(&data, 8)?.to_checksum(None),
                nonce: format!("{nonce:#x}"),
            };
            let hash: B256 = safe_tx_hash(chain_id, safe, &tx).ok()?.parse().ok()?;
            if hash != want {
                return None;
            }
            let signatures = bytes_at(&data, 9)?;
            let count = usize::try_from(threshold).ok()?;
            let mut approvers = Vec::with_capacity(count);
            let mut executor_approved = false;
            for i in 0..count {
                let entry = signatures.get(i * 65..i * 65 + 65)?;
                let (owner, v) = signer(entry, want)?;
                if v == 1 && owner == executor {
                    executor_approved = true;
                }
                approvers.push(owner.to_checksum(None));
            }
            Some(ExecutionFacts {
                executor: executor.to_checksum(None),
                threshold,
                nonce: nonce.to_string(),
                approvers,
                executor_approved,
            })
        };
        if let Some(facts) = read() {
            return Ok(Some(facts));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::multisig::{calls_tx, review, CallArg};

    const SAFE: &str = "0x00000000000000000000000000000000000000Aa";
    const A: &str = "0x1111111111111111111111111111111111111111";
    const B: &str = "0x2222222222222222222222222222222222222222";
    const C: &str = "0x3333333333333333333333333333333333333333";
    const D: &str = "0x4444444444444444444444444444444444444444";
    const SENTINEL: &str = "0x0000000000000000000000000000000000000001";

    fn sel(signature: &str) -> Vec<u8> {
        keccak256(signature.as_bytes())[..4].to_vec()
    }

    fn w_addr(a: &str) -> Vec<u8> {
        let mut w = vec![0u8; 12];
        w.extend_from_slice(addr(a).unwrap().as_slice());
        w
    }

    fn w_num(n: u64) -> Vec<u8> {
        U256::from(n).to_be_bytes::<32>().to_vec()
    }

    fn hexed(bytes: Vec<u8>) -> String {
        primitives::to_hex(&bytes, true)
    }

    fn add(owner: &str, t: u64) -> String {
        hexed(
            [
                sel("addOwnerWithThreshold(address,uint256)"),
                w_addr(owner),
                w_num(t),
            ]
            .concat(),
        )
    }

    fn remove(prev: &str, owner: &str, t: u64) -> String {
        hexed(
            [
                sel("removeOwner(address,address,uint256)"),
                w_addr(prev),
                w_addr(owner),
                w_num(t),
            ]
            .concat(),
        )
    }

    fn swap(prev: &str, old: &str, new: &str) -> String {
        hexed(
            [
                sel("swapOwner(address,address,address)"),
                w_addr(prev),
                w_addr(old),
                w_addr(new),
            ]
            .concat(),
        )
    }

    fn threshold(t: u64) -> String {
        hexed([sel("changeThreshold(uint256)"), w_num(t)].concat())
    }

    /// The action `review` reads from calls into the multisig itself.
    fn action(datas: &[String]) -> Action {
        let calls: Vec<CallArg> = datas
            .iter()
            .map(|data| CallArg {
                to: SAFE.into(),
                value: String::new(),
                data: data.clone(),
            })
            .collect();
        let tx = calls_tx(&calls, "0x0").unwrap();
        review(100, SAFE, &tx).unwrap().action.expect("signed")
    }

    fn rule(threshold: u64, owners: u64) -> Rule {
        Rule { threshold, owners }
    }

    fn owners(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    fn checksum(a: &str) -> String {
        addr(a).unwrap().to_checksum(None)
    }

    #[test]
    fn adding_a_member_reads_who_and_the_rule_before_and_after() {
        let read = member_changes(&action(&[add(D, 2)]), &owners(&[A, B, C]), 2)
            .unwrap()
            .unwrap();
        assert_eq!(
            read.changes,
            vec![MemberChange {
                call: None,
                kind: ChangeKind::Add,
                added: Some(checksum(D)),
                removed: None,
                before: rule(2, 3),
                after: rule(2, 4),
                fits: true,
            }]
        );
        assert_eq!(
            (read.before, read.after, read.fits),
            (rule(2, 3), rule(2, 4), true)
        );
    }

    #[test]
    fn removing_replacing_and_rethresholding_read_the_same_way() {
        let members = owners(&[A, B, C]);
        let removed = member_changes(&action(&[remove(A, B, 1)]), &members, 2)
            .unwrap()
            .unwrap();
        let change = &removed.changes[0];
        assert_eq!(change.kind, ChangeKind::Remove);
        assert_eq!(change.removed.as_deref(), Some(checksum(B).as_str()));
        assert_eq!((change.before, change.after), (rule(2, 3), rule(1, 2)));
        assert!(change.fits);

        let swapped = member_changes(&action(&[swap(SENTINEL, A, D)]), &members, 2)
            .unwrap()
            .unwrap();
        let change = &swapped.changes[0];
        assert_eq!(change.kind, ChangeKind::Replace);
        assert_eq!(change.removed.as_deref(), Some(checksum(A).as_str()));
        assert_eq!(change.added.as_deref(), Some(checksum(D).as_str()));
        assert_eq!((change.before, change.after), (rule(2, 3), rule(2, 3)));

        let raised = member_changes(&action(&[threshold(3)]), &members, 2)
            .unwrap()
            .unwrap();
        assert_eq!(raised.changes[0].kind, ChangeKind::Threshold);
        assert_eq!((raised.before, raised.after), (rule(2, 3), rule(3, 3)));
    }

    /// Inside a batch each change meets what the ones before it left, and is
    /// numbered by its call.
    #[test]
    fn a_batch_reads_its_changes_in_order() {
        let read = member_changes(
            &action(&[add(D, 2), remove(D, A, 3), threshold(2)]),
            &owners(&[A, B, C]),
            2,
        )
        .unwrap()
        .unwrap();
        let steps: Vec<_> = read
            .changes
            .iter()
            .map(|c| (c.call, c.kind, c.before, c.after, c.fits))
            .collect();
        assert_eq!(
            steps,
            vec![
                (Some(1), ChangeKind::Add, rule(2, 3), rule(2, 4), true),
                (Some(2), ChangeKind::Remove, rule(2, 4), rule(3, 3), true),
                (Some(3), ChangeKind::Threshold, rule(3, 3), rule(2, 3), true),
            ]
        );
        assert_eq!((read.before, read.after), (rule(2, 3), rule(2, 3)));
    }

    /// A batch that also sends money keeps its member changes, numbered by
    /// their call; a transaction with none reads `None`.
    #[test]
    fn only_the_member_changes_of_a_batch_are_read() {
        let calls = vec![
            CallArg {
                to: B.into(),
                value: "0x1".into(),
                data: String::new(),
            },
            CallArg {
                to: SAFE.into(),
                value: String::new(),
                data: swap(SENTINEL, A, D),
            },
        ];
        let tx = calls_tx(&calls, "0x0").unwrap();
        let mixed = review(100, SAFE, &tx).unwrap().action.unwrap();
        let read = member_changes(&mixed, &owners(&[A, B]), 1)
            .unwrap()
            .unwrap();
        assert_eq!(read.changes.len(), 1);
        assert_eq!(read.changes[0].call, Some(2));

        let transfer = review(100, SAFE, &calls_tx(&calls[..1], "0x0").unwrap())
            .unwrap()
            .action
            .unwrap();
        assert_eq!(
            member_changes(&transfer, &owners(&[A, B]), 1).unwrap(),
            None
        );
    }

    /// What Safe would revert is said as not fitting, never drawn as a change
    /// that will happen.
    #[test]
    fn a_change_the_safe_would_refuse_does_not_fit() {
        let members = owners(&[A, B, C]);
        let cases = [
            add(A, 2),            // already a member
            add(D, 5),            // more approvals than members
            remove(A, D, 1),      // not a member
            remove(A, B, 3),      // more approvals than the two left
            swap(SENTINEL, D, A), // the old one is no member, the new one is
            threshold(0),
            threshold(4),
        ];
        for data in cases {
            let read = member_changes(&action(std::slice::from_ref(&data)), &members, 2)
                .unwrap()
                .unwrap();
            assert!(!read.fits, "{data}");
        }
    }

    // -- frozen execution facts --------------------------------------------

    fn tx(nonce: u64) -> SafeTx {
        SafeTx {
            to: B.into(),
            value: "0x5".into(),
            data: "0x".into(),
            operation: 0,
            safe_tx_gas: "0x0".into(),
            base_gas: "0x0".into(),
            gas_price: "0x0".into(),
            gas_token: Address::ZERO.to_checksum(None),
            refund_receiver: Address::ZERO.to_checksum(None),
            nonce: format!("{nonce:#x}"),
        }
    }

    fn tail(bytes: &[u8]) -> Vec<u8> {
        let mut out = w_num(bytes.len() as u64);
        out.extend_from_slice(bytes);
        out.extend(std::iter::repeat_n(0u8, (32 - bytes.len() % 32) % 32));
        out
    }

    /// The event's data, as Safe L2 encodes it.
    fn event_data(
        t: &SafeTx,
        signatures: &[u8],
        nonce: u64,
        sender: &str,
        threshold: u64,
    ) -> String {
        let info = [w_num(nonce), w_addr(sender), w_num(threshold)].concat();
        let data_tail = tail(&primitives::from_hex(&t.data).unwrap());
        let sig_tail = tail(signatures);
        let info_tail = tail(&info);
        let head_len = 11 * 32;
        let data_at = head_len;
        let sig_at = data_at + data_tail.len();
        let info_at = sig_at + sig_tail.len();
        let head = [
            w_addr(&t.to),
            w_num(5),
            w_num(data_at as u64),
            w_num(0),
            w_num(0),
            w_num(0),
            w_num(0),
            w_addr("0x0000000000000000000000000000000000000000"),
            w_addr("0x0000000000000000000000000000000000000000"),
            w_num(sig_at as u64),
            w_num(info_at as u64),
        ]
        .concat();
        hexed([head, data_tail, sig_tail, info_tail].concat())
    }

    /// `r = owner, s = 0, v` — a contract signature (0) or the sender's (1).
    fn named(owner: &str, v: u8) -> Vec<u8> {
        [w_addr(owner), vec![0u8; 32], vec![v]].concat()
    }

    fn logs(hash: &str, data: String) -> Vec<ReceiptLog> {
        let t = |s: &str| primitives::to_hex(topic(s).as_slice(), true);
        vec![
            ReceiptLog {
                address: SAFE.into(),
                topics: vec![t(SAFE_MULTISIG_TRANSACTION)],
                data,
            },
            ReceiptLog {
                address: SAFE.into(),
                topics: vec![t(EXECUTION_SUCCESS), hash.into()],
                data: hexed(w_num(0)),
            },
        ]
    }

    #[test]
    fn an_execution_reads_who_approved_then_and_who_sent_it() {
        let t = tx(7);
        let hash = safe_tx_hash(100, SAFE, &t).unwrap();
        // Two of two: A by contract signature, C by sending it. A third entry
        // past the threshold is not read (Safe does not read it either).
        let sigs = [named(A, 0), named(C, 1), named(B, 0)].concat();
        let facts = execution_facts(
            100,
            SAFE,
            &hash,
            &logs(&hash, event_data(&t, &sigs, 7, C, 2)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            facts,
            ExecutionFacts {
                executor: checksum(C),
                threshold: 2,
                nonce: "7".into(),
                approvers: vec![checksum(A), checksum(C)],
                executor_approved: true,
            }
        );
    }

    #[test]
    fn an_executor_that_is_no_approver_is_not_counted() {
        let t = tx(1);
        let hash = safe_tx_hash(100, SAFE, &t).unwrap();
        let sigs = [named(A, 0), named(B, 0)].concat();
        let facts = execution_facts(
            100,
            SAFE,
            &hash,
            &logs(&hash, event_data(&t, &sigs, 1, D, 2)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(facts.approvers, vec![checksum(A), checksum(B)]);
        assert_eq!(facts.executor, checksum(D));
        assert!(!facts.executor_approved);
    }

    /// Facts are only ever this transaction's: another one's log, another
    /// contract's log, or no `ExecutionSuccess` for it read as none.
    #[test]
    fn nothing_else_passes_for_its_facts() {
        let t = tx(3);
        let hash = safe_tx_hash(100, SAFE, &t).unwrap();
        let sigs = named(A, 0);
        // The event's own transaction is another (its nonce differs).
        let other = event_data(&tx(4), &sigs, 4, A, 1);
        assert_eq!(
            execution_facts(100, SAFE, &hash, &logs(&hash, other)).unwrap(),
            None
        );
        // Emitted by another contract.
        let mut foreign = logs(&hash, event_data(&t, &sigs, 3, A, 1));
        for log in &mut foreign {
            log.address = D.into();
        }
        assert_eq!(execution_facts(100, SAFE, &hash, &foreign).unwrap(), None);
        // No ExecutionSuccess for it (a non-L2 Safe, or a failed execution).
        let mut failed = logs(&hash, event_data(&t, &sigs, 3, A, 1));
        failed.pop();
        assert_eq!(execution_facts(100, SAFE, &hash, &failed).unwrap(), None);
    }

    #[test]
    fn a_plain_accounts_signature_recovers_its_owner() {
        use k256::ecdsa::SigningKey;
        let key = SigningKey::from_slice(&[7u8; 32]).unwrap();
        let point = key.verifying_key().to_sec1_point(false);
        let owner = Address::from_slice(&keccak256(&point.as_bytes()[1..])[12..]);
        let t = tx(2);
        let hash = safe_tx_hash(100, SAFE, &t).unwrap();
        let digest: B256 = hash.parse().unwrap();
        let (sig, rec) = key.sign_prehash_recoverable(digest.as_slice());
        let mut entry = sig.to_bytes().to_vec();
        entry.push(27 + rec.to_byte());
        let facts = execution_facts(
            100,
            SAFE,
            &hash,
            &logs(&hash, event_data(&t, &entry, 2, A, 1)),
        )
        .unwrap()
        .unwrap();
        assert_eq!(facts.approvers, vec![owner.to_checksum(None)]);
    }
}
