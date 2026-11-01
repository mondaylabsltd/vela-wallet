//! The one review's reads and the one gate (§3.5, §3.6), shared by every
//! machine that has a multisig transaction signed: the proposals machine (a
//! Vela member's signature) and the outside page (an outside owner's wallet).
//!
//! A port of the reads `proposal-review.svelte.ts` (`gatherReview`) and
//! `call-review.svelte.ts` make for the one `multisig_review` every member
//! reads, and of the gate `gate.svelte.ts` (`authorizeSigning`) puts before
//! every signature. One function each, so the connect page's review is the
//! one a Vela member gets — the core's reading, the three hashes, the
//! state-diff guard (trace and probe), the signing sheet's own simulation
//! ([`MultisigOperation::Simulate`]) and the facts of each recipient
//! ([`MultisigOperation::RecipientRisk`]) — and its gate decides the same
//! way on the same reads ([`decide`]).
//!
//! Nothing here keeps anything: a machine that caches a review (the
//! proposals machine signs from one gathered moments ago at the very same
//! members) does so itself.

use std::collections::BTreeMap;

use futures::future;
use serde_json::{json, Value};

use super::multisig_outside::reading::TokenFacts;
use super::multisig_shell::{ChainMembers, Io, MultisigOperation, MultisigShellResult, RpcFail};
use crate::multisig::{self, Genesis, SafeTx};
use crate::multisig_review::{
    self as mr, Gate, GateInput, Known, KnownKind, MultisigReview, ReadsInput, RecipientFacts,
    ReviewInput, SignRule,
};

/// How long one read of a review is waited for (`REVIEW_READ_MS`,
/// `bounded.ts`): past it, that read said nothing.
pub const REVIEW_READ_MS: u32 = 8_000;
/// `review_reads` asks for code at most this many rounds (a call naming a
/// contract that names another).
const CODE_ROUNDS: usize = 3;

/// What a review is of: a transaction of `safe` on `chain_id`, at the
/// members read there now.
#[derive(Clone, Debug)]
pub struct ReviewSubject {
    pub chain_id: u64,
    pub safe: String,
    pub tx: SafeTx,
    pub members: ChainMembers,
    /// Its genesis (for a multisig not deployed there; its name).
    pub genesis: Option<Genesis>,
    /// Who the person knows (contacts, own accounts, multisigs), for the
    /// recipients' roles and look-alikes. The members are added here.
    pub known: Vec<Known>,
    /// What this device calls each member (lowercase address → name).
    pub member_names: BTreeMap<String, String>,
    /// The site that asked, for a dApp's calls.
    pub origin: Option<String>,
    /// Tokens this device already read (lowercase address → facts): never
    /// read again, so a read that fails now cannot unsay one (r8-2).
    pub tokens: BTreeMap<String, TokenFacts>,
}

/// A JSON-RPC answer as the review takes it: `{result}`, `{error}`,
/// `{unreachable: true}`.
pub fn envelope(answer: Result<Value, RpcFail>) -> Value {
    match answer {
        Ok(result) => json!({ "result": result }),
        // r8-4: the error's `data` is the probe's answer (simulateAndRevert
        // always reverts) — kept, so the state-diff guard reads on a network
        // without a debug trace.
        Err(RpcFail::Error {
            code,
            message,
            data: Some(data),
        }) => {
            json!({ "error": { "code": code, "message": message, "data": data } })
        }
        Err(RpcFail::Error { code, message, .. }) => {
            json!({ "error": { "code": code, "message": message } })
        }
        Err(RpcFail::Unanswered(_)) => json!({ "unreachable": true }),
    }
}

async fn bounded<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_id: u64,
    method: &str,
    params: Value,
) -> Value {
    envelope(
        io.rpc_within(chain_id, method, params, Some(REVIEW_READ_MS))
            .await,
    )
}

/// The members as the review knows them: labelled with this device's name
/// for each, else their address.
pub fn members_known(owners: &[String], names: &BTreeMap<String, String>) -> Vec<Known> {
    owners
        .iter()
        .map(|m| Known {
            address: m.clone(),
            label: names
                .get(&m.to_ascii_lowercase())
                .cloned()
                .unwrap_or_else(|| m.clone()),
            kind: KnownKind::Member,
        })
        .collect()
}

/// Every read the one review of `subject` needs, made now: the code
/// `review_reads` names (each bounded, asked again for what that names),
/// then together the state-diff trace and probe, the signing sheet's
/// simulation of its calls from the Safe, and each recipient's facts.
pub async fn gather_review<Ev: Send + 'static>(
    io: &Io<Ev>,
    subject: &ReviewSubject,
) -> ReviewInput {
    let chain_id = subject.chain_id;
    let mut base = ReadsInput {
        chain_id,
        safe: subject.safe.clone(),
        tx: Some(subject.tx.clone()),
        owners: subject.members.owners.clone(),
        threshold: subject.members.threshold,
        deployed: subject.members.deployed,
        genesis: subject.genesis.clone(),
        ..ReadsInput::default()
    };
    let mut reads = mr::review_reads(&base).unwrap_or_default();
    for _ in 0..CODE_ROUNDS {
        let missing: Vec<String> = reads
            .need_code
            .iter()
            .filter(|a| !base.codes.contains_key(*a))
            .cloned()
            .collect();
        if missing.is_empty() {
            break;
        }
        let codes = future::join_all(missing.iter().map(|a| async move {
            let e = bounded(io, chain_id, "eth_getCode", json!([a, "latest"])).await;
            (
                a.clone(),
                e.get("result").and_then(Value::as_str).map(str::to_owned),
            )
        }))
        .await;
        base.codes.extend(codes);
        reads = mr::review_reads(&base).unwrap_or_default();
    }
    let simulate = async {
        if reads.sim_calls.is_empty() {
            return None;
        }
        match io
            .ask(MultisigOperation::Simulate {
                chain_id,
                from: subject.safe.clone(),
                calls: reads.sim_calls.clone(),
                state_overrides: reads.sim_state_overrides.clone(),
            })
            .await
        {
            MultisigShellResult::Simulated { reply } => reply,
            _ => None,
        }
    };
    let ask = |r: Option<mr::RpcRequest>| async move {
        match r {
            Some(r) => Some(bounded(io, chain_id, &r.method, r.params).await),
            None => None,
        }
    };
    let state = async { futures::join!(ask(reads.trace.clone()), ask(reads.probe.clone())) };
    let facts = future::join_all(reads.recipients.iter().map(|address| async move {
        match io
            .ask(MultisigOperation::RecipientRisk {
                chain_id,
                address: address.clone(),
            })
            .await
        {
            MultisigShellResult::RecipientFacts {
                is_contract,
                first_time,
            } => RecipientFacts {
                address: address.clone(),
                is_contract,
                first_time,
            },
            _ => RecipientFacts {
                address: address.clone(),
                is_contract: None,
                first_time: None,
            },
        }
    }));
    // The tokens the reading names that the registry does not (r8-2): each
    // contract's own symbol and decimals; one that does not answer stays
    // unknown — its amount is said raw, never as 18 decimals.
    let tokens = async {
        let review = multisig::review(chain_id, &subject.safe, &subject.tx).ok();
        let names: Vec<String> = review
            .map(|r| mr::reading::tokens_to_read(chain_id, &subject.tx, &r))
            .unwrap_or_default()
            .into_iter()
            .filter(|t| !subject.tokens.contains_key(t))
            .collect();
        let mut read =
            super::multisig_outside::reading::read_these(io, chain_id, names, Some(REVIEW_READ_MS))
                .await;
        for (k, v) in &subject.tokens {
            read.entry(k.clone()).or_insert_with(|| v.clone());
        }
        read
    };
    let ((trace, probe), sim_reply, recipient_facts, tokens) =
        futures::join!(state, simulate, facts, tokens);
    let mut known = subject.known.clone();
    known.extend(members_known(
        &subject.members.owners,
        &subject.member_names,
    ));
    ReviewInput {
        reads: base,
        sim_reply,
        trace,
        probe,
        recipient_facts,
        known,
        blind: false,
        token_facts: tokens.into_values().collect(),
        origin: subject.origin.clone(),
    }
}

/// The one review of what was read ([`mr::multisig_review`]); `None`: the
/// core cannot read the transaction at all.
pub fn review_of(input: &ReviewInput) -> Option<MultisigReview> {
    mr::multisig_review(input).ok()
}

/// The gate before a signature, from what the chain gave just now
/// (`findings`, §3.5 — `None`: it did not answer) and the review of the
/// transaction about to be signed (§3.6). One rule for every signer:
///
/// - no review, and the core does not read the transaction: blocked (never
///   signed, no override);
/// - no review of one it reads: unchecked — nothing is signed on the
///   decoding alone (a refused network is still said as refused);
/// - else the core's `signing_gate`: unread, refused network, refused
///   transaction, risk (asked every time), open.
pub fn decide(
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
    findings: Option<Vec<Value>>,
    review: Option<&MultisigReview>,
) -> Gate {
    let sign = review.map(|r| r.sign);
    let decodable =
        sign.is_some() || multisig::review(chain_id, safe, tx).is_ok_and(|r| r.action.is_some());
    decide_with(chain_id, findings, sign, decodable)
}

/// [`decide`] over the facts alone: the review's sign rule (`None`: no
/// review), and whether the core reads the transaction at all — the rule
/// the shared gate vectors (`tests/vectors/multisig/gate.json`) pin.
pub fn decide_with(
    chain_id: u64,
    findings: Option<Vec<Value>>,
    sign: Option<SignRule>,
    decodable: bool,
) -> Gate {
    let sign = sign.or_else(|| {
        (!decodable).then_some(SignRule {
            allowed: false,
            anyway: false,
        })
    });
    let gate = mr::signing_gate(&GateInput {
        chain_id,
        findings,
        sign,
    });
    match gate {
        Gate::Open | Gate::Risk { .. } if sign.is_none() => Gate::Unchecked,
        other => other,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn an_envelope_is_what_the_review_reads() {
        assert_eq!(envelope(Ok(json!("0x1"))), json!({"result": "0x1"}));
        assert_eq!(
            envelope(Err(RpcFail::Unanswered("timed out".into()))),
            json!({"unreachable": true})
        );
        assert_eq!(
            envelope(Err(RpcFail::Error {
                code: Some(3),
                message: "execution reverted".into(),
                data: None,
            })),
            json!({"error": {"code": 3, "message": "execution reverted"}})
        );
        // r8-4: the revert's payload reaches the probe's reader.
        assert_eq!(
            envelope(Err(RpcFail::Error {
                code: Some(3),
                message: "execution reverted".into(),
                data: Some(json!("0x0001")),
            })),
            json!({"error": {"code": 3, "message": "execution reverted", "data": "0x0001"}})
        );
    }

    #[test]
    fn members_are_known_by_this_devices_names() {
        let names = BTreeMap::from([("0xab".to_owned(), "Ana".to_owned())]);
        let known = members_known(&["0xAB".to_owned(), "0xcd".to_owned()], &names);
        assert_eq!(known[0].label, "Ana");
        assert_eq!(known[1].label, "0xcd");
        assert!(known.iter().all(|k| k.kind == KnownKind::Member));
    }
}
