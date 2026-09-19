//! Resolving a passkey's wallets through three layers — the index for speed,
//! the chain for truth, Ethereum for survival (spec 062 plan §6, built as 064).
//!
//! Signing in asks two questions: *which groups does this key belong to?* and
//! *who are this group's members?* — and derives the wallet's address from the
//! answer. Three parties can answer:
//!
//! 1. the **index service**: one round trip, everything in it, and OURS — which
//!    is exactly why it must not be the thing a person's address depends on;
//! 2. the **registry contract on Gnosis**, where the record lives;
//! 3. the same contract on **Ethereum**, where a person may have backed it up.
//!
//! Until this module the layers were fallbacks on SILENCE, written four times
//! (once per shell): an index that answered was believed. An index that is
//! wrong — compromised, buggy, or pointed at by somebody else's DNS — could
//! hand back a member set that is not the registered one, and the wallet would
//! derive, show and receive funds at an address its owner does not solely
//! control. The passkey assertion does not help: the person's key really is in
//! the forged set.
//!
//! The contract makes a better arrangement possible. A group's identity is its
//! **group public key** (never the per-deployment `unitId`), and its
//! `contentHash` is a pure function of `(rpId, metadata, groupPublicKey,
//! members)` — computable offline. So an index answer is not trusted, it is
//! **checked**: the hash is recomputed here from what the index said and
//! compared with what `getUnitByGroupKey` says on chain. One `eth_call`. Equal:
//! the index told the truth, and cannot have told anything else. Different: the
//! index is discarded and the chain is read in full.
//!
//! What "cannot verify" means, precisely: the index accepts a registration
//! before it lands on chain, so a group the chain does not know YET is normal
//! for a wallet created a minute ago. That is `verified_by: none` — the person
//! still signs in (refusing would make the wallet depend on RPC health), and
//! nothing may call it verified. Only a group the chain DOES hold, with a
//! different hash, proves the index wrong.
//!
//! Same shape as every other walk here: a pure function of a transcript. A
//! shell performs the requests it is handed and asks again.

use alloy_dyn_abi::DynSolValue;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::primitives;
use crate::registry_backup::{call_data, decode, returned, REGISTRY, UNIT_TUPLE};
use crate::registry_chain::{self, READ_CHAINS};
use crate::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};

const SIG_UNIT_BY_GROUP_KEY: &str = "getUnitByGroupKey(bytes)";
/// A wallet's founding set is at most seven keys; the index is asked for the
/// whole of it in one page.
const MAX_MEMBERS: u64 = 7;

/// Who answered. Unit ids are per DEPLOYMENT, so whoever lists a key's units is
/// who must be asked about them: the shell hands this back, opaque, with each
/// unit question.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "chain_id")]
pub enum Source {
    Index,
    Chain(u32),
}

impl Source {
    /// `index` | `chain:100` — the opaque token a shell keeps between questions.
    #[must_use]
    pub fn token(self) -> String {
        match self {
            Self::Index => "index".to_owned(),
            Self::Chain(chain_id) => format!("chain:{chain_id}"),
        }
    }

    /// Anything unreadable is the index: that is where a fresh question starts.
    #[must_use]
    pub fn parse(token: &str) -> Self {
        token
            .strip_prefix("chain:")
            .and_then(|id| id.parse().ok())
            .map_or(Self::Index, Self::Chain)
    }
}

/// Which chain PROVED the answer. `None` = the index answered and no chain
/// could confirm or deny it (unreachable, or the group has not landed yet).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerifiedBy {
    Gnosis,
    Ethereum,
    None,
}

fn verifier(chain_id: u32) -> VerifiedBy {
    if chain_id == READ_CHAINS[0] {
        VerifiedBy::Gnosis
    } else {
        VerifiedBy::Ethereum
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResolveStep {
    Ask {
        requests: Vec<LookupRequest>,
    },
    Done {
        /// The answer in the INDEX's JSON shape, whoever gave it — every guard a
        /// shell already has runs unchanged. `None` = nobody answered.
        body: Option<String>,
        /// The token to hand back with this key's unit questions.
        source: String,
        verified_by: VerifiedBy,
        /// The index answered, and the chain proved it wrong. The body is the
        /// chain's. Worth a log line: it means our own service lied or broke.
        index_discarded: bool,
    },
}

fn done(
    body: Option<String>,
    source: Source,
    verified_by: VerifiedBy,
    index_discarded: bool,
) -> ResolveStep {
    ResolveStep::Done {
        body,
        source: source.token(),
        verified_by,
        index_discarded,
    }
}

fn nobody(source: Source) -> ResolveStep {
    done(None, source, VerifiedBy::None, false)
}

fn eth_call(id: String, chain_id: u32, call: &registry_chain::ChainCall) -> LookupRequest {
    LookupRequest::EthCall {
        id,
        chain_id,
        to: call.to.clone(),
        data: call.data.clone(),
    }
}

/// Percent-encode the one thing that goes in a query string here: a hex key.
/// Hex needs no escaping; anything else is refused by the callers' own parsing.
fn query_safe(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|c| c.is_ascii_hexdigit() || c == 'x')
}

// ---------------------------------------------------------------------------
// contentHash — the group's frozen founding digest, recomputed offline
// ---------------------------------------------------------------------------

fn hex_bytes(value: &str) -> Option<Vec<u8>> {
    primitives::from_hex(value.trim_start_matches("0x")).ok()
}

/// `contentHashFor(rpId, metadata, groupPublicKey, members)` exactly as the
/// contract computes it:
/// `keccak(abi.encode(rpId, metadata, groupPublicKey, [keccak(abi.encode(pk, attestation))…]))`.
///
/// From an index-shaped unit body. `None` when the body does not carry what the
/// hash is made of — which is itself a reason not to believe it.
#[must_use]
pub fn content_hash_of(unit_body: &Value) -> Option<[u8; 32]> {
    let unit = &unit_body["unit"];
    let members = unit_body["members"]["items"].as_array()?;
    // A PARTIAL page hashes to something else, and should: the founding set is
    // all of its members or it is a different wallet.
    let total = unit_body["members"]["total"].as_u64()?;
    if members.len() as u64 != total {
        return None;
    }
    let mut member_hashes = Vec::with_capacity(members.len());
    for member in members {
        let encoded = DynSolValue::Tuple(vec![
            DynSolValue::Bytes(hex_bytes(member["publicKey"].as_str()?)?),
            DynSolValue::Bytes(hex_bytes(member["attestation"].as_str()?)?),
        ])
        .abi_encode_params();
        let hash: [u8; 32] = primitives::keccak256(&encoded).try_into().ok()?;
        member_hashes.push(DynSolValue::FixedBytes(hash.into(), 32));
    }
    let encoded = DynSolValue::Tuple(vec![
        DynSolValue::String(unit["rpId"].as_str()?.to_owned()),
        DynSolValue::Bytes(hex_bytes(unit["metadata"].as_str()?)?),
        DynSolValue::Bytes(hex_bytes(unit["groupPublicKey"].as_str()?)?),
        DynSolValue::Array(member_hashes),
    ])
    .abi_encode_params();
    primitives::keccak256(&encoded).try_into().ok()
}

/// What `getUnitByGroupKey` said: does the chain hold this group, under which
/// of ITS unit ids, with which `contentHash`.
struct OnChain {
    exists: bool,
    unit_id: u64,
    content_hash: [u8; 32],
}

fn on_chain(answer: &LookupAnswer) -> Option<OnChain> {
    let values = decode(&format!("(bool,uint256,{UNIT_TUPLE})"), &returned(answer)?)?;
    let unit = values.get(2)?.as_tuple()?;
    let (hash, _) = unit.get(3)?.as_fixed_bytes()?;
    Some(OnChain {
        exists: values.first()?.as_bool()?,
        unit_id: values
            .get(1)?
            .as_uint()
            .and_then(|(n, _)| u64::try_from(n).ok())?,
        content_hash: hash.try_into().ok()?,
    })
}

// ---------------------------------------------------------------------------
// Question 1 — which groups does this key belong to?
// ---------------------------------------------------------------------------

fn unit_ids(body: &Value) -> usize {
    body["groups"]["unitIds"].as_array().map_or(0, Vec::len)
}

/// `public_key_hex` → the index's `?publicKey=` body, from whoever can give it.
///
/// The index first. If it is silent, refuses, or says the key founded nothing,
/// the chain is asked before a person is told their wallet does not exist: an
/// index that is merely BEHIND must not read as "no such wallet". A chain that
/// knows the key founded nothing is believed only when it is the record's home;
/// Ethereum holds what somebody backed up, and an empty answer there is not a
/// verdict.
#[must_use]
pub fn key_step(public_key_hex: &str, answers: &[LookupAnswer]) -> ResolveStep {
    let key = public_key_hex.trim_start_matches("0x").to_lowercase();
    if !query_safe(&key) {
        return nobody(Source::Index);
    }
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    let Some(index) = answer("index") else {
        return ResolveStep::Ask {
            requests: vec![LookupRequest::IndexGet {
                id: "index".to_owned(),
                path: format!("/api/query?publicKey={key}"),
            }],
        };
    };
    let index_body = (index.outcome == LookupOutcome::Ok)
        .then_some(index.body.as_deref())
        .flatten()
        .filter(|body| serde_json::from_str::<Value>(body).is_ok());
    let listed = index_body.filter(|body| {
        serde_json::from_str::<Value>(body).is_ok_and(|parsed| unit_ids(&parsed) > 0)
    });
    if let Some(body) = listed {
        return done(
            Some(body.to_owned()),
            Source::Index,
            VerifiedBy::None,
            false,
        );
    }

    // Silent, refusing, or "founded nothing": the contract is asked.
    let Some(calls) = registry_chain::key_status_calls(&key) else {
        return done(
            index_body.map(str::to_owned),
            Source::Index,
            VerifiedBy::None,
            false,
        );
    };
    for chain_id in READ_CHAINS {
        let ids = [format!("key@{chain_id}:0"), format!("key@{chain_id}:1")];
        let (Some(entry), Some(groups)) = (answer(&ids[0]), answer(&ids[1])) else {
            return ResolveStep::Ask {
                requests: ids
                    .iter()
                    .zip(&calls)
                    .map(|(id, call)| eth_call(id.clone(), chain_id, call))
                    .collect(),
            };
        };
        let body = (entry.outcome == LookupOutcome::Ok && groups.outcome == LookupOutcome::Ok)
            .then(|| entry.body.as_deref().zip(groups.body.as_deref()))
            .flatten()
            .and_then(|(entry, groups)| registry_chain::key_status_json(entry, groups));
        let Some(body) = body else { continue };
        let founded = serde_json::from_str::<Value>(&body).map_or(0, |parsed| unit_ids(&parsed));
        if chain_id != READ_CHAINS[0] && founded == 0 {
            continue;
        }
        return done(
            Some(body),
            Source::Chain(chain_id),
            verifier(chain_id),
            false,
        );
    }
    // No chain could add anything: what the index said stands, such as it was.
    done(
        index_body.map(str::to_owned),
        Source::Index,
        VerifiedBy::None,
        false,
    )
}

// ---------------------------------------------------------------------------
// Question 2 — who are this group's members?
// ---------------------------------------------------------------------------

/// The whole unit from ONE chain, under that chain's own unit id.
fn from_chain(
    chain_id: u32,
    unit_id: u64,
    answers: &[LookupAnswer],
    index_discarded: bool,
) -> ResolveStep {
    let source = Source::Chain(chain_id);
    let Some(calls) = registry_chain::unit_calls(unit_id) else {
        return nobody(source);
    };
    let ids = [
        format!("unit:{unit_id}@{chain_id}:0"),
        format!("unit:{unit_id}@{chain_id}:1"),
    ];
    let answer = |id: &str| answers.iter().find(|a| a.id == id);
    let (Some(unit), Some(members)) = (answer(&ids[0]), answer(&ids[1])) else {
        return ResolveStep::Ask {
            requests: ids
                .iter()
                .zip(&calls)
                .map(|(id, call)| eth_call(id.clone(), chain_id, call))
                .collect(),
        };
    };
    let body = (unit.outcome == LookupOutcome::Ok && members.outcome == LookupOutcome::Ok)
        .then(|| unit.body.as_deref().zip(members.body.as_deref()))
        .flatten()
        .and_then(|(unit, members)| registry_chain::unit_json(unit_id, unit, members));
    match body {
        Some(body) => done(Some(body), source, verifier(chain_id), index_discarded),
        None => done(None, source, VerifiedBy::None, index_discarded),
    }
}

/// `unit_id`, as listed by `source` → the index's `?unitId=` body, PROVED where
/// a chain can prove it.
#[must_use]
pub fn unit_step(unit_id: u64, source: &str, answers: &[LookupAnswer]) -> ResolveStep {
    let source = Source::parse(source);
    // A chain listed this unit: its ids are that chain's, and only it is asked.
    if let Source::Chain(chain_id) = source {
        return from_chain(chain_id, unit_id, answers, false);
    }
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    let Some(index) = answer("index") else {
        return ResolveStep::Ask {
            requests: vec![LookupRequest::IndexGet {
                id: "index".to_owned(),
                path: format!("/api/query?unitId={unit_id}&pageSize={MAX_MEMBERS}&order=asc"),
            }],
        };
    };
    let parsed = (index.outcome == LookupOutcome::Ok)
        .then_some(index.body.as_deref())
        .flatten()
        .and_then(|body| Some((body, serde_json::from_str::<Value>(body).ok()?)));
    let Some((body, parsed)) = parsed else {
        // The index listed this unit and then went away, or refused. Its ids
        // are Gnosis's, so Gnosis is who can still answer.
        return from_chain(READ_CHAINS[0], unit_id, answers, false);
    };

    // The index answered. It is not believed; it is CHECKED.
    let claimed = content_hash_of(&parsed);
    let group_key = parsed["unit"]["groupPublicKey"]
        .as_str()
        .and_then(hex_bytes);
    let (Some(claimed), Some(group_key)) = (claimed, group_key) else {
        // A body the hash cannot be made from is not an answer worth having.
        return from_chain(READ_CHAINS[0], unit_id, answers, true);
    };
    let Some(data) = call_data(SIG_UNIT_BY_GROUP_KEY, &[DynSolValue::Bytes(group_key)]) else {
        return done(
            Some(body.to_owned()),
            Source::Index,
            VerifiedBy::None,
            false,
        );
    };
    for chain_id in READ_CHAINS {
        let id = format!("verify@{chain_id}");
        let Some(said) = answer(&id) else {
            return ResolveStep::Ask {
                requests: vec![LookupRequest::EthCall {
                    id,
                    chain_id,
                    to: REGISTRY.to_owned(),
                    data,
                }],
            };
        };
        // Silent, or something this build cannot read: the next chain may know.
        let Some(chain) = on_chain(said) else {
            continue;
        };
        if !chain.exists {
            // Not there YET (the index accepts a registration before it lands),
            // or never backed up to Ethereum. Not a verdict either way.
            continue;
        }
        if chain.content_hash == claimed {
            return done(
                Some(body.to_owned()),
                Source::Index,
                verifier(chain_id),
                false,
            );
        }
        // The chain holds this group and it is NOT what the index described.
        // The index is discarded; the chain is read in full, under its own id.
        return from_chain(chain_id, chain.unit_id, answers, true);
    }
    // Nobody could confirm or deny. The person still signs in.
    done(
        Some(body.to_owned()),
        Source::Index,
        VerifiedBy::None,
        false,
    )
}

// ---------------------------------------------------------------------------
// JSON doors, for the bindings
// ---------------------------------------------------------------------------

fn answers_of(json: &str) -> Vec<LookupAnswer> {
    serde_json::from_str(json).unwrap_or_default()
}

fn to_json(step: &ResolveStep) -> String {
    serde_json::to_string(step).unwrap_or_else(|_| {
        r#"{"type":"done","body":null,"source":"index","verified_by":"none","index_discarded":false}"#
            .to_owned()
    })
}

#[must_use]
pub fn key_step_json(public_key_hex: &str, answers_json: &str) -> String {
    to_json(&key_step(public_key_hex, &answers_of(answers_json)))
}

#[must_use]
pub fn unit_step_json(unit_id: u64, source: &str, answers_json: &str) -> String {
    to_json(&unit_step(unit_id, source, &answers_of(answers_json)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Web's verbatim recording of the contract on both chains, keyed by chain
    /// then by calldata — plus, since 064, the two `getUnitByGroupKey` answers.
    fn recorded() -> Value {
        serde_json::from_str(include_str!(
            "../../../../app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json"
        ))
        .unwrap_or_default()
    }

    fn key() -> String {
        recorded()["publicKey"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }

    /// The Gnosis unit 10 in the index's shape — built from the CHAIN's own
    /// bytes, so it is what an honest index returns.
    fn honest_unit() -> Value {
        let fixture = recorded();
        let calls = registry_chain::unit_calls(10).unwrap_or_default();
        let at = |i: usize| {
            fixture["answers"]["100"][&calls[i].data]
                .as_str()
                .unwrap_or_default()
                .to_owned()
        };
        serde_json::from_str(&registry_chain::unit_json(10, &at(0), &at(1)).unwrap_or_default())
            .unwrap_or_default()
    }

    /// Drive a walk: the index says `index`, the recorded chains minus `down`.
    fn run(
        mut step: impl FnMut(&[LookupAnswer]) -> ResolveStep,
        index: Option<&str>,
        down: &[u32],
    ) -> (ResolveStep, Vec<String>) {
        let fixture = recorded();
        let mut answers = Vec::new();
        let mut asked = Vec::new();
        for _ in 0..32 {
            match step(&answers) {
                ResolveStep::Ask { requests } => {
                    for request in requests {
                        let (id, body) = match request {
                            LookupRequest::IndexGet { id, .. } => (id, index.map(str::to_owned)),
                            LookupRequest::EthCall {
                                id, chain_id, data, ..
                            } => {
                                let body = (!down.contains(&chain_id))
                                    .then(|| {
                                        fixture["answers"][chain_id.to_string()][&data].as_str()
                                    })
                                    .flatten()
                                    .map(str::to_owned);
                                (id, body)
                            }
                        };
                        asked.push(id.clone());
                        answers.push(LookupAnswer {
                            id,
                            outcome: if body.is_some() {
                                LookupOutcome::Ok
                            } else {
                                LookupOutcome::Failed
                            },
                            body,
                        });
                    }
                }
                done => return (done, asked),
            }
        }
        unreachable!("the walk never finished")
    }

    fn parts(step: ResolveStep) -> (Option<Value>, String, VerifiedBy, bool) {
        let ResolveStep::Done {
            body,
            source,
            verified_by,
            index_discarded,
        } = step
        else {
            unreachable!()
        };
        (
            body.and_then(|b| serde_json::from_str(&b).ok()),
            source,
            verified_by,
            index_discarded,
        )
    }

    #[test]
    fn the_hash_is_the_contracts_own() {
        // Recomputed offline from the members, it equals what `getUnit` stores.
        let fixture = recorded();
        let calls = registry_chain::unit_calls(10).unwrap_or_default();
        let unit_hex = fixture["answers"]["100"][&calls[0].data]
            .as_str()
            .unwrap_or_default();
        let stored = decode(
            &format!("({UNIT_TUPLE})"),
            &hex_bytes(unit_hex).unwrap_or_default(),
        )
        .and_then(|values| {
            values
                .first()?
                .as_tuple()?
                .get(3)?
                .as_fixed_bytes()
                .map(|(h, _)| h.to_vec())
        })
        .unwrap_or_default();
        assert_eq!(
            content_hash_of(&honest_unit()).map(|h| h.to_vec()),
            Some(stored)
        );
    }

    #[test]
    fn an_honest_index_is_proved_by_one_call_to_gnosis() {
        let honest = honest_unit().to_string();
        let (done, asked) = run(|a| unit_step(10, "index", a), Some(&honest), &[]);
        let (body, source, verified_by, discarded) = parts(done);
        assert_eq!(
            (source.as_str(), verified_by, discarded),
            ("index", VerifiedBy::Gnosis, false)
        );
        assert_eq!(body, Some(honest_unit()));
        // The index, then ONE eth_call. Ethereum is never asked.
        assert_eq!(asked, ["index", "verify@100"]);
    }

    #[test]
    fn a_forged_member_is_caught_and_the_chain_is_read_instead() {
        // The attack this exists for: one member's key swapped for somebody
        // else's. Everything else about the body is genuine.
        let mut forged = honest_unit();
        let theirs = forged["members"]["items"][1]["publicKey"].clone();
        forged["members"]["items"][2]["publicKey"] = theirs;
        let (done, asked) = run(
            |a| unit_step(10, "index", a),
            Some(&forged.to_string()),
            &[],
        );
        let (body, source, verified_by, discarded) = parts(done);
        assert!(discarded, "the index was believed");
        assert_eq!(
            (source.as_str(), verified_by),
            ("chain:100", VerifiedBy::Gnosis)
        );
        assert_eq!(
            body,
            Some(honest_unit()),
            "the chain's founding set, not the index's"
        );
        assert!(asked.iter().any(|id| id.starts_with("unit:10@100")));
    }

    #[test]
    fn gnosis_silent_the_ethereum_backup_proves_it() {
        let honest = honest_unit().to_string();
        let (done, asked) = run(|a| unit_step(10, "index", a), Some(&honest), &[100]);
        let (_, source, verified_by, _) = parts(done);
        assert_eq!(
            (source.as_str(), verified_by),
            ("index", VerifiedBy::Ethereum)
        );
        assert_eq!(asked, ["index", "verify@100", "verify@1"]);
    }

    #[test]
    fn no_chain_reachable_still_signs_in_and_is_never_called_verified() {
        let honest = honest_unit().to_string();
        let (done, _) = run(|a| unit_step(10, "index", a), Some(&honest), &[100, 1]);
        let (body, _, verified_by, discarded) = parts(done);
        assert_eq!((verified_by, discarded), (VerifiedBy::None, false));
        assert!(body.is_some());
    }

    #[test]
    fn a_silent_index_is_answered_by_gnosis_under_the_same_id() {
        let (done, _) = run(|a| unit_step(10, "index", a), None, &[]);
        let (body, source, verified_by, discarded) = parts(done);
        assert_eq!(
            (source.as_str(), verified_by, discarded),
            ("chain:100", VerifiedBy::Gnosis, false)
        );
        assert_eq!(body, Some(honest_unit()));
    }

    #[test]
    fn a_chain_listing_is_continued_on_that_chain_and_nowhere_else() {
        // Gnosis unit 10 is Ethereum unit 0: the token is what keeps them apart.
        let (done, asked) = run(
            |a| unit_step(0, "chain:1", a),
            Some("must not be asked"),
            &[],
        );
        let (body, source, verified_by, _) = parts(done);
        assert_eq!(
            (source.as_str(), verified_by),
            ("chain:1", VerifiedBy::Ethereum)
        );
        assert_eq!(
            body.map(|b| b["members"]["total"].clone()),
            Some(Value::from(3))
        );
        assert!(
            asked
                .iter()
                .all(|id| id.ends_with("@1:0") || id.ends_with("@1:1")),
            "{asked:?}"
        );
    }

    #[test]
    fn the_key_question_index_first_then_the_contract() {
        // An index that knows the key: believed for the LISTING (the units it
        // names are each proved when they are fetched).
        let listing = r#"{"entry":{},"groups":{"total":1,"unitIds":[10]}}"#;
        let (done, asked) = run(|a| key_step(&key(), a), Some(listing), &[]);
        let (_, source, _, _) = parts(done);
        assert_eq!((source.as_str(), asked.len()), ("index", 1));

        // Silent → Gnosis, whose ids come back with its token.
        let (done, _) = run(|a| key_step(&key(), a), None, &[]);
        let (body, source, verified_by, _) = parts(done);
        assert_eq!(
            (source.as_str(), verified_by),
            ("chain:100", VerifiedBy::Gnosis)
        );
        assert_eq!(
            body.map(|b| b["groups"]["unitIds"].clone()),
            Some(serde_json::json!([12, 10, 8]))
        );

        // Silent and Gnosis silent → Ethereum's backup.
        let (done, _) = run(|a| key_step(&key(), a), None, &[100]);
        let (body, source, _, _) = parts(done);
        assert_eq!(source, "chain:1");
        assert_eq!(
            body.map(|b| b["groups"]["unitIds"].clone()),
            Some(serde_json::json!([0]))
        );

        // Nobody at all.
        let (done, _) = run(|a| key_step(&key(), a), None, &[100, 1]);
        assert_eq!(parts(done).0, None);
    }

    #[test]
    fn an_index_that_says_no_such_wallet_is_checked_before_a_person_is_told_so() {
        // Behind, or lying by omission: the chain knows three units.
        let nothing = r#"{"entry":null,"groups":{"total":0,"unitIds":[]}}"#;
        let (done, _) = run(|a| key_step(&key(), a), Some(nothing), &[]);
        let (body, source, _, _) = parts(done);
        assert_eq!(source, "chain:100");
        assert_eq!(
            body.map(|b| b["groups"]["unitIds"].clone()),
            Some(serde_json::json!([12, 10, 8]))
        );
        // …and with no chain to ask, what the index said stands.
        let (done, _) = run(|a| key_step(&key(), a), Some(nothing), &[100, 1]);
        let (body, source, _, _) = parts(done);
        assert_eq!(source, "index");
        assert_eq!(
            body.map(|b| b["groups"]["total"].clone()),
            Some(Value::from(0))
        );
    }

    #[test]
    fn the_token_round_trips_and_garbage_starts_at_the_index() {
        assert_eq!(
            Source::parse(&Source::Chain(100).token()),
            Source::Chain(100)
        );
        assert_eq!(Source::parse(&Source::Index.token()), Source::Index);
        assert_eq!(Source::parse("chain:nope"), Source::Index);
        assert!(key_step_json("not a key", "[]").contains(r#""body":null"#));
        assert!(unit_step_json(10, "index", "not json").contains(r#""type":"ask""#));
    }
}
