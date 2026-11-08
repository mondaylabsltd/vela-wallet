//! Resolving a passkey's wallets through three layers — the index for speed,
//! the chain for truth, Ethereum for survival (spec 062 plan §6, built as 067).
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
//! §37 (review 1-7): that `verified_by: none` is no longer a wallet like any
//! other. One key is the person's own whoever answered (the address derives
//! from it alone), but a MULTI-key group the chain has not confirmed is only
//! the index's word for who else holds a key — a lying index can hand back
//! the person's real credential beside an attacker's keys, and the wallet
//! would show an address the attacker co-owns. So such an account is saved
//! [`Identity::Unconfirmed`]: the person is in (refusing would make the
//! wallet depend on RPC health), it can sign, but it shows no address to
//! receive at and takes no deposit until [`confirm_step`] — chain only, no
//! index — finds the group on chain and its members make this very address.
//! A chain that holds the key's group and it makes another address proves
//! the index wrong: [`Identity::Refuted`].
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

#[cfg(feature = "bindings")]
use ts_rs::TS;

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
///
/// Also what a shell hands the sign-in machine with a key's status
/// (`ShellResult::RegistryKeyStatus`): only a Gnosis verdict may say a key has
/// no record. The default is `None`, so a shell that does not pass it fails
/// closed — "nobody vouched for this", never "this key is unknown".
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum VerifiedBy {
    Gnosis,
    Ethereum,
    #[default]
    None,
}

/// §37 (review 1-7): what the chain has said about the group an account's
/// address was derived from. Persisted on the account (`Account::identity`);
/// absent reads [`Self::Confirmed`] — every record before §37, a wallet made
/// on this device (its keys are its own), a one-key wallet (its address
/// derives from the key that just signed), a group a chain proved.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum Identity {
    /// Nothing left to confirm.
    #[default]
    Confirmed,
    /// A multi-key group only the index vouched for: no receive address, no
    /// deposit, "not yet confirmed on-chain" — checked again
    /// ([`confirm_step`]) before first use.
    Unconfirmed,
    /// The chain holds this key's group, and it makes another address: the
    /// index lied (or broke). Never received at; the person signs in again.
    Refuted,
}

impl Identity {
    #[must_use]
    pub fn is_confirmed(&self) -> bool {
        *self == Self::Confirmed
    }
}

/// §37: the identity a sign-in gives the account it rebuilt from a group of
/// `members` keys that `verified_by` vouched for.
#[must_use]
pub fn identity_of(members: usize, verified_by: VerifiedBy) -> Identity {
    if members > 1 && verified_by == VerifiedBy::None {
        Identity::Unconfirmed
    } else {
        Identity::Confirmed
    }
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
// §37 — confirming an unconfirmed account, from the chain alone
// ---------------------------------------------------------------------------

/// A step of [`confirm_step`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfirmStep {
    /// Make these `eth_call`s and ask again with the answers.
    Ask { requests: Vec<LookupRequest> },
    /// What the chain said. `Unconfirmed` (no chain answered, or none holds
    /// the group yet) changes nothing; the others are saved on the account.
    Done {
        identity: Identity,
        verified_by: VerifiedBy,
    },
}

/// `keys` (uncompressed P-256 hex), in some order, make `address`: each tried
/// as the pinned first key — the only degree of freedom the multi-key
/// derivation has (it orders the rest itself).
fn keys_make(keys: &[String], address: &str) -> bool {
    let derives = |ordered: &[String]| {
        ordered
            .iter()
            .map(|hex| crate::safe::parse_public_key(hex))
            .collect::<Result<Vec<_>, _>>()
            .and_then(|keys| crate::safe::compute_safe_address_multi(&keys))
            .is_ok_and(|info| info.address.eq_ignore_ascii_case(address.trim()))
    };
    (0..keys.len()).any(|pin| {
        let mut ordered = vec![keys[pin].clone()];
        ordered.extend(
            keys.iter()
                .enumerate()
                .filter(|(i, _)| *i != pin)
                .map(|(_, k)| k.clone()),
        );
        derives(&ordered)
    })
}

/// §37 (review 1-7): re-check, before first use, the account at `address`
/// that `public_key_hex` (one of its keys) signs for — on the chain only,
/// never the index: which group does the registry say this key founded, and
/// do that group's members make `address`?
///
/// Gnosis first, then the Ethereum backup. Yes → [`Identity::Confirmed`].
/// The chain holds the key's group and its members make another address →
/// [`Identity::Refuted`]. Neither chain answers, or neither holds a group for
/// the key yet (the index takes a registration before it lands) →
/// [`Identity::Unconfirmed`], asked again later.
#[must_use]
pub fn confirm_step(public_key_hex: &str, address: &str, answers: &[LookupAnswer]) -> ConfirmStep {
    let unconfirmed = ConfirmStep::Done {
        identity: Identity::Unconfirmed,
        verified_by: VerifiedBy::None,
    };
    let key = public_key_hex.trim_start_matches("0x").to_lowercase();
    if !query_safe(&key) {
        return unconfirmed;
    }
    let Some(calls) = registry_chain::key_status_calls(&key) else {
        return unconfirmed;
    };
    let answer = |id: &str| answers.iter().find(|a| a.id == id);
    fn ok(a: &LookupAnswer) -> Option<&str> {
        (a.outcome == LookupOutcome::Ok)
            .then_some(a.body.as_deref())
            .flatten()
    }
    for chain_id in READ_CHAINS {
        let ids = [format!("key@{chain_id}:0"), format!("key@{chain_id}:1")];
        let (Some(entry), Some(groups)) = (answer(&ids[0]), answer(&ids[1])) else {
            return ConfirmStep::Ask {
                requests: ids
                    .iter()
                    .zip(&calls)
                    .map(|(id, call)| eth_call(id.clone(), chain_id, call))
                    .collect(),
            };
        };
        let listed = ok(entry)
            .zip(ok(groups))
            .and_then(|(entry, groups)| registry_chain::key_status_json(entry, groups))
            .and_then(|body| serde_json::from_str::<Value>(&body).ok());
        let unit_ids: Vec<u64> = listed
            .and_then(|body| {
                Some(
                    body["groups"]["unitIds"]
                        .as_array()?
                        .iter()
                        .filter_map(Value::as_u64)
                        .collect(),
                )
            })
            .unwrap_or_default();
        if unit_ids.is_empty() {
            // Silent, or no group for this key here (yet).
            continue;
        }
        // Every group this key founded here — a key may found more than one,
        // and each needs every member's passkey to be registered at all, so
        // ANY of them making this address is the person's own wallet.
        let mut asks = Vec::new();
        let mut read: Vec<Option<Vec<String>>> = Vec::new();
        for unit_id in &unit_ids {
            let Some(unit_calls) = registry_chain::unit_calls(*unit_id) else {
                read.push(None);
                continue;
            };
            let ids = [
                format!("unit:{unit_id}@{chain_id}:0"),
                format!("unit:{unit_id}@{chain_id}:1"),
            ];
            let (Some(unit), Some(members)) = (answer(&ids[0]), answer(&ids[1])) else {
                asks.extend(
                    ids.iter()
                        .zip(&unit_calls)
                        .map(|(id, call)| eth_call(id.clone(), chain_id, call)),
                );
                continue;
            };
            read.push(
                ok(unit)
                    .zip(ok(members))
                    .and_then(|(unit, members)| registry_chain::unit_json(*unit_id, unit, members))
                    .and_then(|body| serde_json::from_str::<Value>(&body).ok())
                    .and_then(|body| {
                        body["members"]["items"]
                            .as_array()?
                            .iter()
                            .map(|m| m["publicKey"].as_str().map(str::to_owned))
                            .collect::<Option<Vec<String>>>()
                    }),
            );
        }
        if !asks.is_empty() {
            return ConfirmStep::Ask { requests: asks };
        }
        if read
            .iter()
            .flatten()
            .any(|keys| !keys.is_empty() && keys_make(keys, address))
        {
            return ConfirmStep::Done {
                identity: Identity::Confirmed,
                verified_by: verifier(chain_id),
            };
        }
        if read.iter().all(Option::is_some) {
            // Every group this key founded here was read, and none makes
            // this address: the chain proves the index's group wrong.
            return ConfirmStep::Done {
                identity: Identity::Refuted,
                verified_by: verifier(chain_id),
            };
        }
        // A group could not be read: no verdict from this chain.
    }
    unconfirmed
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

/// [`confirm_step`] over JSON: `{type: 'ask', requests}` |
/// `{type: 'done', identity, verified_by}`.
#[must_use]
pub fn confirm_step_json(public_key_hex: &str, address: &str, answers_json: &str) -> String {
    serde_json::to_string(&confirm_step(
        public_key_hex,
        address,
        &answers_of(answers_json),
    ))
    .unwrap_or_else(|_| {
        r#"{"type":"done","identity":"unconfirmed","verified_by":"none"}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Web's verbatim recording of the contract on both chains, keyed by chain
    /// then by calldata — plus, since 067, the two `getUnitByGroupKey` answers.
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

    /// An attacker's genuine P-256 point (the test suite's second fixture key).
    const ATTACKER: &str = "041111111111111111111111111111111111111111111111111111111111111114d8dd738ca691a327dd14c119194a3d96dbb93d2cb9edab12387669ec973cb024";

    fn unit_keys(body: &Value) -> Vec<String> {
        body["members"]["items"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .filter_map(|m| m["publicKey"].as_str().map(str::to_owned))
            .collect()
    }

    fn address_of(keys: &[String]) -> String {
        let parsed: Vec<_> = keys
            .iter()
            .filter_map(|k| crate::safe::parse_public_key(k).ok())
            .collect();
        crate::safe::compute_safe_address_multi(&parsed)
            .map(|i| i.address)
            .unwrap_or_default()
    }

    fn confirmed(step: ConfirmStep) -> (Identity, VerifiedBy) {
        let ConfirmStep::Done {
            identity,
            verified_by,
        } = step
        else {
            unreachable!()
        };
        (identity, verified_by)
    }

    /// Drive a confirm walk against the recorded chains minus `down`.
    fn confirm_run(address: &str, down: &[u32]) -> ConfirmStep {
        let fixture = recorded();
        let mut answers: Vec<LookupAnswer> = Vec::new();
        for _ in 0..16 {
            match confirm_step(&key(), address, &answers) {
                ConfirmStep::Ask { requests } => {
                    for request in requests {
                        let LookupRequest::EthCall {
                            id, chain_id, data, ..
                        } = request
                        else {
                            unreachable!("the confirm walk never asks the index");
                        };
                        let body = (!down.contains(&chain_id))
                            .then(|| fixture["answers"][chain_id.to_string()][&data].as_str())
                            .flatten()
                            .map(str::to_owned);
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
                done => return done,
            }
        }
        unreachable!("the confirm walk never finished")
    }

    /// §37: an index that told the truth is confirmed by the chain alone —
    /// a group this key founded makes the very address.
    #[test]
    fn an_honest_group_is_confirmed_on_chain() {
        let address = address_of(&unit_keys(&honest_unit()));
        assert_eq!(
            confirmed(confirm_run(&address, &[])),
            (Identity::Confirmed, VerifiedBy::Gnosis)
        );
        assert_eq!(
            confirmed(confirm_run(&address, &[100])),
            (Identity::Confirmed, VerifiedBy::Ethereum),
            "Gnosis silent: the Ethereum backup confirms it"
        );
    }

    /// §37 (review 1-7), the lying-index fixture: the person's real key and
    /// credential, beside an attacker's key — the address it makes is one
    /// the attacker co-owns. The chain holds this key's groups, none makes
    /// it: refuted. With no chain to ask, it stays unconfirmed — never
    /// confirmed.
    #[test]
    fn a_lying_index_group_is_refuted_by_the_chain() {
        let lie = address_of(&[key(), ATTACKER.to_owned()]);
        let (identity, _) = confirmed(confirm_run(&lie, &[]));
        assert_eq!(identity, Identity::Refuted);
        assert_eq!(
            confirmed(confirm_run(&lie, &[100, 1])),
            (Identity::Unconfirmed, VerifiedBy::None)
        );
        // …and at sign-in it was never verified: a multi-key group nobody
        // but the index vouched for is saved unconfirmed.
        let mut forged = honest_unit();
        forged["members"]["items"][1]["publicKey"] = Value::from(ATTACKER);
        let (_, _, verified_by, _) = parts(
            run(
                |a| unit_step(10, "index", a),
                Some(&forged.to_string()),
                &[100, 1],
            )
            .0,
        );
        assert_eq!(identity_of(3, verified_by), Identity::Unconfirmed);
        assert_eq!(identity_of(1, verified_by), Identity::Confirmed);
        assert_eq!(identity_of(3, VerifiedBy::Gnosis), Identity::Confirmed);
    }

    /// §37: the lying-index vector (`tests/vectors/rules/lying-index.json`)
    /// every shell replays through its own resolver loop against the recorded
    /// chains (`registry-chain.json`): what the unit step vouches for, the
    /// identity a sign-in saves, and what the confirm step finds. Rebuilt
    /// with `VELA_WRITE_VECTORS=1`; otherwise it must equal what the core
    /// says.
    #[test]
    fn the_lying_index_vector_is_the_cores_answer() {
        let honest = honest_unit();
        let keys = unit_keys(&honest);
        let mut forged = honest.clone();
        forged["members"]["items"][1]["publicKey"] = Value::from(ATTACKER);
        let lie = address_of(&[key(), ATTACKER.to_owned()]);
        let unit_case = |name: &str, body: &Value, down: &[u32]| {
            let (body_out, source, verified_by, discarded) =
                parts(run(|a| unit_step(10, "index", a), Some(&body.to_string()), down).0);
            let members = body_out.as_ref().map_or(0, |b| unit_keys(b).len());
            serde_json::json!({
                "name": name, "kind": "unit", "unitId": 10, "source": "index",
                "index": body, "down": down,
                "expect": {
                    "verified_by": verified_by, "source": source, "index_discarded": discarded,
                    "members": members,
                    "identity": identity_of(members, verified_by),
                },
            })
        };
        let confirm_case = |name: &str, address: &str, down: &[u32]| {
            let (identity, verified_by) = confirmed(confirm_run(address, down));
            serde_json::json!({
                "name": name, "kind": "confirm", "address": address, "down": down,
                "expect": { "identity": identity, "verified_by": verified_by },
            })
        };
        let cases = vec![
            unit_case("an honest index, Gnosis proves it", &honest, &[]),
            unit_case(
                "a forged member is caught; the chain's members are used",
                &forged,
                &[],
            ),
            unit_case(
                "a forged member, no chain answers: unconfirmed, never confirmed",
                &forged,
                &[100, 1],
            ),
            unit_case(
                "an honest index, no chain answers: still unconfirmed",
                &honest,
                &[100, 1],
            ),
            confirm_case(
                "the honest group is confirmed on Gnosis",
                &address_of(&keys),
                &[],
            ),
            confirm_case(
                "the honest group, Gnosis silent: Ethereum's backup confirms",
                &address_of(&keys),
                &[100],
            ),
            confirm_case(
                "the lying index's address (the person's key + an attacker's) is refuted",
                &lie,
                &[],
            ),
            confirm_case(
                "the lying index's address, no chain answers: unconfirmed",
                &lie,
                &[100, 1],
            ),
        ];
        let vector = serde_json::json!({
            "about": "§37 (review 1-7): a key index that lies. `publicKey` is the recorded golden key; every eth_call is answered from app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json (`answers[chainId][calldata]`), minus the chains in `down` (answered as failed); an IndexGet is answered with `index`. kind `unit`: drive `registry_resolve_unit_step(unitId, source, …)` to done — `expect.verified_by` is what the shell passes on in RegistryUnit, `expect.identity` what a sign-in saves for a group of `expect.members` keys (`unconfirmed`: no receive address). kind `confirm`: drive `registry_resolve_confirm_step(publicKey, address, …)` to done.",
            "attacker": ATTACKER,
            "publicKey": key(),
            "cases": cases,
        });
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/vectors/rules/lying-index.json"
        );
        if std::env::var("VELA_WRITE_VECTORS").is_ok() {
            let text = serde_json::to_string_pretty(&vector).unwrap_or_default() + "\n";
            assert!(std::fs::write(path, text).is_ok());
        }
        let on_disk: Value = std::fs::read_to_string(path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        assert_eq!(
            on_disk, vector,
            "the vector is the core's answer (VELA_WRITE_VECTORS=1 rebuilds it)"
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
