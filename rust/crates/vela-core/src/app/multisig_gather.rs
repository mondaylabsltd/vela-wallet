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
use super::multisig_review_state as review_state;
use super::multisig_shell::{ChainMembers, Io, MultisigOperation, MultisigShellResult, RpcFail};
use crate::multisig::{self, Genesis, SafeTx};
use crate::multisig_review::{
    self as mr, Gate, GateInput, Known, KnownKind, MultisigReview, ReadsInput, RecipientFacts,
    ReviewInput, SignRule,
};

/// How long one read of a review is waited for (`REVIEW_READ_MS`,
/// `bounded.ts`): past it, that read said nothing.
pub const REVIEW_READ_MS: u32 = 8_000;
/// r14-13: how long the core waits for any one question of a review — a
/// read whose bound ([`REVIEW_READ_MS`]) a shell did not keep, the shell's
/// simulation, a recipient's facts — before it takes it as unanswered. Above
/// the read's own bound (a pool's answer at 8 s and a hedge are not cut);
/// the same on every shell, so a review lands the same way at the same time
/// everywhere, whichever shell left a question open (Android's simulation
/// once held the review to its 40 s bound while the web's ended at 26 s).
pub const REVIEW_ASK_MS: u32 = REVIEW_READ_MS + 2_000;
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
    within(
        io,
        io.rpc_within(chain_id, method, params, Some(REVIEW_READ_MS)),
    )
    .await
    .map_or_else(|| json!({ "unreachable": true }), envelope)
}

/// `fut`, or `None` once [`REVIEW_ASK_MS`] has passed without its answer.
async fn within<Ev: Send + 'static, T>(
    io: &Io<Ev>,
    fut: impl std::future::Future<Output = T>,
) -> Option<T> {
    let answer = Box::pin(fut);
    let cap = Box::pin(io.wait(u64::from(REVIEW_ASK_MS)));
    match future::select(answer, cap).await {
        future::Either::Left((out, _)) => Some(out),
        future::Either::Right(_) => None,
    }
}

/// What a review's reads gathered: the review's input, and the answer of
/// each read the core made of the network ([`envelope`]s: the multisig's
/// own code, the code rounds, the state trace and probe) — what
/// [`ReviewState::settled`](super::multisig_review_state::ReviewState::settled)
/// lands the review by (r14-13).
#[derive(Clone, Debug)]
pub struct Gathered {
    pub input: ReviewInput,
    pub answers: Vec<Value>,
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
/// then together the multisig's own code (r14-13: the network's state the
/// review is read against — whatever the checks do, it says whether the
/// network answered at all), the state-diff trace and probe, the signing
/// sheet's simulation of its calls from the Safe, and each recipient's
/// facts. Every question is bounded ([`REVIEW_ASK_MS`]).
pub async fn gather_review<Ev: Send + 'static>(io: &Io<Ev>, subject: &ReviewSubject) -> Gathered {
    let chain_id = subject.chain_id;
    let mut answers: Vec<Value> = Vec::new();
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
            let code = e.get("result").and_then(Value::as_str).map(str::to_owned);
            (a.clone(), code, e)
        }))
        .await;
        for (a, code, e) in codes {
            base.codes.insert(a, code);
            answers.push(e);
        }
        reads = mr::review_reads(&base).unwrap_or_default();
    }
    let sim_asked = !reads.sim_calls.is_empty();
    let simulate = || async {
        if reads.sim_calls.is_empty() {
            return None;
        }
        match within(
            io,
            io.ask(MultisigOperation::Simulate {
                chain_id,
                from: subject.safe.clone(),
                calls: reads.sim_calls.clone(),
                state_overrides: reads.sim_state_overrides.clone(),
            }),
        )
        .await
        {
            Some(MultisigShellResult::Simulated { reply }) => reply,
            _ => None,
        }
    };
    // r14-13: the multisig's own code — the network's state this review is
    // read against, asked beside the checks (no time added).
    let anchor = bounded(io, chain_id, "eth_getCode", json!([subject.safe, "latest"]));
    let ask = |r: Option<mr::RpcRequest>| async move {
        match r {
            Some(r) => Some(bounded(io, chain_id, &r.method, r.params).await),
            None => None,
        }
    };
    let state = async { futures::join!(ask(reads.trace.clone()), ask(reads.probe.clone())) };
    let facts = future::join_all(reads.recipients.iter().map(|address| async move {
        match within(
            io,
            io.ask(MultisigOperation::RecipientRisk {
                chain_id,
                address: address.clone(),
            }),
        )
        .await
        {
            Some(MultisigShellResult::RecipientFacts {
                is_contract,
                first_time,
            }) => RecipientFacts {
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
    let (anchor, (mut trace, mut probe), mut sim_reply, recipient_facts, tokens) =
        futures::join!(anchor, state, simulate(), facts, tokens);
    let mut known = subject.known.clone();
    known.extend(members_known(
        &subject.members.owners,
        &subject.member_names,
    ));
    let token_facts: Vec<TokenFacts> = tokens.into_values().collect();
    let input =
        |trace: &Option<Value>, probe: &Option<Value>, sim_reply: &Option<String>| ReviewInput {
            reads: base.clone(),
            sim_reply: sim_reply.clone(),
            trace: trace.clone(),
            probe: probe.clone(),
            recipient_facts: recipient_facts.clone(),
            known: known.clone(),
            blind: false,
            token_facts: token_facts.clone(),
            origin: subject.origin.clone(),
        };
    // r15-12 / r16-3: a network coming back answers its multisig's own
    // state (the pool's outage probe goes first) while the checks asked
    // beside it are still turned away as silent — for 1–5 s on Arc, whose
    // nodes also answer a state trace with "method not found". Landed as is,
    // the review would be D1 ("… anyway", signable) until it is read again:
    // a review on its way from failed to read passed through "anyway". So
    // "unchecked" is published only once the checks are final for this read
    // (`checks_final`): the silent ones are asked again, paced, until one
    // answers or the re-ask reaches its own limit (`RECHECK_MS`) — the
    // review stays Reading (nothing signable) meanwhile.
    let started = io.now().await;
    loop {
        let review = review_of(&input(&trace, &probe, &sim_reply))
            .and_then(|r| serde_json::to_value(r).ok());
        let checks = check_envelopes(trace.as_ref(), probe.as_ref(), sim_asked, &sim_reply);
        let rechecked = io.now().await - started;
        if review_state::checks_final(review.as_ref(), &anchor, &checks, rechecked) {
            break;
        }
        let left = f64::from(review_state::RECHECK_MS) - rechecked;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let left_ms = left.max(0.0) as u64;
        io.wait(u64::from(review_state::RECHECK_PAUSE_MS).min(left_ms))
            .await;
        let silent = |e: &Option<Value>| e.as_ref().is_some_and(|e| !answered(e));
        let (t_again, p_again) = (silent(&trace), silent(&probe));
        let s_again = sim_asked && sim_silent(&sim_reply);
        let again = async {
            let ask_if = |again: bool, r: Option<mr::RpcRequest>| async move {
                if again {
                    Some(ask(r).await)
                } else {
                    None
                }
            };
            futures::join!(
                ask_if(t_again, reads.trace.clone()),
                ask_if(p_again, reads.probe.clone()),
                async {
                    if s_again {
                        Some(simulate().await)
                    } else {
                        None
                    }
                }
            )
        };
        // Within what is left of the re-ask's own limit: past it, what is
        // still silent is final.
        let left = f64::from(review_state::RECHECK_MS) - (io.now().await - started);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let cap = Box::pin(io.wait(left.max(0.0) as u64));
        let landed = future::select(Box::pin(again), cap).await;
        match landed {
            future::Either::Left(((t, p, s), _)) => {
                if let Some(t) = t {
                    trace = t;
                }
                if let Some(p) = p {
                    probe = p;
                }
                if let Some(s) = s {
                    sim_reply = s;
                }
            }
            future::Either::Right(_) => break,
        }
    }
    answers.push(anchor);
    answers.extend(trace.iter().chain(probe.iter()).cloned());
    Gathered {
        input: input(&trace, &probe, &sim_reply),
        answers,
    }
}

/// Whether a read's envelope is the network's answer (`{result}` or
/// `{error}`), not silence (`{unreachable}`).
fn answered(envelope: &Value) -> bool {
    envelope.get("result").is_some() || envelope.get("error").is_some()
}

/// Whether the shell's simulation came back as the network's silence: its
/// reply says `{"unreachable": true}` (every node failed). No reply at all
/// (a shell that does not simulate, or a question cut at its bound) is
/// final for that check — only a network's silence is asked again.
fn sim_silent(reply: &Option<String>) -> bool {
    reply.as_deref().is_some_and(|r| {
        serde_json::from_str::<Value>(r)
            .ok()
            .is_some_and(|v| v.get("unreachable").is_some())
    })
}

/// r16-3: the envelopes of the safety checks a review asked — the state
/// trace, the probe, the simulation (`{unreachable}` for its network's
/// silence, else `{result}`) — for
/// [`checks_final`](review_state::checks_final).
fn check_envelopes(
    trace: Option<&Value>,
    probe: Option<&Value>,
    sim_asked: bool,
    sim_reply: &Option<String>,
) -> Vec<Value> {
    let mut checks: Vec<Value> = trace.into_iter().chain(probe).cloned().collect();
    if sim_asked {
        checks.push(if sim_silent(sim_reply) {
            json!({ "unreachable": true })
        } else {
            json!({ "result": sim_reply })
        });
    }
    checks
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
