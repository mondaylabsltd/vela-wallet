//! The name a Vela user registered for a wallet, found by its ADDRESS — the
//! contract, written once (issue 191).
//!
//! ## Why this is three hops and not one
//!
//! The v1 passkey index answered `?walletRef=<address>`. The v2 index — the
//! default since multi-passkey wallets — does not, and cannot: an address is
//! `f(all founding keys)`, no single entry owns one, and the query answers
//! `400 publicKey, entryId, unitId or groupPublicKey is required`. Four shells
//! each carried their own copy of the v1 question, and for two specs the
//! registry step of every identity waterfall silently never answered.
//!
//! The chain holds the missing link. A Vela Safe's founding key is configured
//! on `SafeWebAuthnSharedSigner` ([`safe::WEBAUTHN_SIGNER`]), which keeps
//! `(x, y)` in the SAFE's own storage and hands them to anyone who asks:
//!
//! ```text
//! 1. sharedSigner.getConfiguration(address)   → the founding public key
//! 2. GET /api/query?publicKey=04‖x‖y          → the units that key founded
//! 3. GET /api/query?unitId=N                  → frozen metadata; the unit whose
//!                                               `address` is the one asked
//!                                               about names it (key_names[0],
//!                                               the field `login` recovers a
//!                                               wallet's name from)
//! ```
//!
//! Hops 2 and 3 are put to the index service first — one HTTP call, a cache of
//! the registry contract — and, when it does not answer, to the contract
//! itself: on Gnosis, then on Ethereum, where a person may have backed the
//! record up (spec 062) so that it outlives both our server and Gnosis.
//!
//! Not spoofable by a stranger: hop 1 is the chain's answer about THIS address,
//! and a unit only lists keys that signed their own membership. Whoever can put
//! a name there holds the wallet's founding key — it is their name to give.
//!
//! ## The shells own the transport; the core owns the contract
//!
//! [`step`] is a pure function of the transcript so far. A shell calls it with
//! the answers it has, performs the requests it is handed — an `eth_call`
//! through its RPC pool, a GET against its configured index — appends what came
//! back, and calls again, until [`LookupStep::Done`]. Which chain is asked
//! first, which unit is believed, what a malformed blob means and how long a
//! verdict may be remembered are decided here, so four clients cannot ask four
//! different questions or trust four different answers (the rule
//! `passkey::directory_lookup_url` already states).
//!
//! No state crosses the boundary but the transcript, so the binding is one
//! function on every platform and a shell cannot get the sequencing wrong.

use serde::{Deserialize, Serialize};

use crate::primitives;
use crate::registry_backup;
use crate::registry_metadata::RegistryMetadata;
use crate::safe;

/// `getConfiguration(address)` → `(uint256 x, uint256 y, uint176 verifiers)`.
const GET_CONFIGURATION: &str = "getConfiguration(address)";

/// Asked first, and alone: the registry's own chain, and the cheapest place
/// most Vela wallets have transacted. When it answers, nobody else is asked.
pub const HOME_CHAIN: u32 = 100;

/// Asked together when the home chain has no configuration. The address is the
/// same everywhere, but the configuration exists only where the Safe has been
/// DEPLOYED. Short on purpose: every entry is a call spent on every address
/// that is not a Vela wallet. Order is the tie-break when two chains answer.
pub const OTHER_CHAINS: [u32; 6] = [8453, 56, 42161, 10, 137, 1];

/// A key founds few wallets; past this the newest are the ones worth a call.
const MAX_UNITS: usize = 8;

/// One thing the shell must fetch. `id` is opaque to the shell: it comes back
/// on the [`LookupAnswer`] and nothing else is read from it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookupRequest {
    /// `eth_call({to, data}, "latest")` on `chain_id`. The answer's body is the
    /// raw `result` hex.
    EthCall {
        id: String,
        chain_id: u32,
        to: String,
        data: String,
    },
    /// `GET <configured index origin><path>`. The answer's body is the raw
    /// response text.
    IndexGet { id: String, path: String },
}

/// How a request ended. The distinction is the whole caching rule: `NotFound`
/// is the other side SAYING no; `Failed` is nobody saying anything.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LookupOutcome {
    /// An answer arrived; `body` carries it.
    Ok,
    /// The index answered 404. (An `eth_call` never uses this.)
    NotFound,
    /// Unreachable, timed out, an RPC error, any other HTTP status.
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LookupAnswer {
    pub id: String,
    pub outcome: LookupOutcome,
    pub body: Option<String>,
}

/// How long a shell may keep the verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LookupRemember {
    /// A founding key, once configured, and a unit's metadata, once
    /// registered, do not change.
    Forever,
    /// A miss is only true for now — a Safe is counterfactual until its first
    /// operation, a name can be registered later. [`MISS_TTL_MS`].
    Briefly,
    /// Somebody did not answer. That is not a verdict; ask again next time.
    No,
}

/// How long [`LookupRemember::Briefly`] lasts: long enough that a book full of
/// plain EOAs does not re-ask seven chains on every visit, short enough that a
/// new name shows up the same day.
pub const MISS_TTL_MS: f64 = 6.0 * 60.0 * 60.0 * 1000.0;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletName {
    /// The wallet's name as its owner registered it.
    pub name: String,
    /// The founding public key, uncompressed (`04‖x‖y`), lowercase bare hex.
    pub public_key: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LookupStep {
    /// Fetch all of these (together, if the shell can), append the answers to
    /// the transcript, and call [`step`] again.
    Ask { requests: Vec<LookupRequest> },
    Done {
        found: Option<WalletName>,
        remember: LookupRemember,
    },
}

fn done(found: Option<WalletName>, remember: LookupRemember) -> LookupStep {
    LookupStep::Done { found, remember }
}

fn is_askable(address: &str) -> bool {
    let Some(hex) = address.strip_prefix("0x") else {
        return false;
    };
    hex.len() == 40
        && hex.bytes().all(|b| b.is_ascii_hexdigit())
        // The zero address is a mint/burn counterparty, not a wallet.
        && hex.bytes().any(|b| b != b'0')
}

fn configuration_request(address_lower: &str, chain_id: u32) -> LookupRequest {
    let selector = primitives::function_selector(GET_CONFIGURATION)
        .map(|s| primitives::to_hex(&s, true))
        // The signature is a constant; this cannot fail, and a wrong selector
        // would only ever read as "not configured".
        .unwrap_or_default();
    LookupRequest::EthCall {
        id: format!("cfg:{chain_id}"),
        chain_id,
        to: safe::WEBAUTHN_SIGNER.to_owned(),
        data: format!("{selector}{:0>64}", &address_lower[2..]),
    }
}

/// What one chain said: a key, a definite "not a configured Vela Safe here",
/// or nothing.
enum Configured {
    Key(String),
    NotHere,
    Unknown,
}

fn read_configuration(answer: &LookupAnswer) -> Configured {
    if answer.outcome != LookupOutcome::Ok {
        return Configured::Unknown;
    }
    let Some(body) = answer.body.as_deref() else {
        return Configured::Unknown;
    };
    let hex = body.strip_prefix("0x").unwrap_or(body);
    if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Configured::Unknown;
    }
    // No contract at the signer's address on this chain answers `0x`.
    if hex.len() < 128 {
        return Configured::NotHere;
    }
    let (x, y) = (&hex[..64], &hex[64..128]);
    // An unconfigured account — every EOA, every other contract — reads zero.
    if x.bytes().all(|b| b == b'0') || y.bytes().all(|b| b == b'0') {
        return Configured::NotHere;
    }
    Configured::Key(format!("04{x}{y}").to_lowercase())
}

#[derive(Deserialize)]
struct KeyProfile {
    #[serde(default)]
    groups: Option<KeyGroups>,
}

#[derive(Deserialize)]
struct KeyGroups {
    #[serde(default, rename = "unitIds")]
    unit_ids: Vec<serde_json::Value>,
}

#[derive(Deserialize)]
struct UnitResponse {
    unit: UnitMeta,
}

#[derive(Deserialize)]
struct UnitMeta {
    metadata: String,
}

/// Ethereum: where a backed-up record also lives (`registry_backup`).
const BACKUP_CHAIN: u32 = 1;

/// Who can say which units a key founded, in the order they are asked.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    /// The index service — one HTTP call, a cache of the contract.
    Index,
    /// The registry contract itself, on this chain.
    Chain(u32),
}

const SOURCES: [Source; 3] = [
    Source::Index,
    Source::Chain(HOME_CHAIN),
    Source::Chain(BACKUP_CHAIN),
];

enum UnitSays {
    Named(String),
    NotThisWallet,
    Silent,
}

impl Source {
    fn id(self, base: &str) -> String {
        match self {
            Source::Index => base.to_owned(),
            Source::Chain(chain) => format!("{base}@{chain}"),
        }
    }

    fn key_request(self, id: String, public_key: &str) -> LookupRequest {
        match self {
            Source::Index => LookupRequest::IndexGet {
                id,
                path: format!("/api/query?publicKey={public_key}"),
            },
            Source::Chain(chain_id) => {
                use alloy_dyn_abi::DynSolValue;
                use alloy_primitives::U256;
                let key = registry_backup::public_key_bytes(public_key).unwrap_or_default();
                let data = registry_backup::call_data(
                    registry_backup::SIG_GROUPS_OF_KEY,
                    &[
                        DynSolValue::Bytes(key),
                        DynSolValue::Uint(U256::ZERO, 256),
                        DynSolValue::Uint(U256::from(MAX_UNITS), 256),
                        DynSolValue::Bool(true), // newest first, as the index lists them
                    ],
                )
                .unwrap_or_default();
                LookupRequest::EthCall {
                    id,
                    chain_id,
                    to: registry_backup::REGISTRY.to_owned(),
                    data,
                }
            }
        }
    }

    fn unit_request(self, id: String, unit_id: u64) -> LookupRequest {
        match self {
            Source::Index => LookupRequest::IndexGet {
                id,
                path: format!("/api/query?unitId={unit_id}&pageSize=1"),
            },
            Source::Chain(chain_id) => {
                use alloy_dyn_abi::DynSolValue;
                use alloy_primitives::U256;
                let data = registry_backup::call_data(
                    registry_backup::SIG_GET_UNIT,
                    &[DynSolValue::Uint(U256::from(unit_id), 256)],
                )
                .unwrap_or_default();
                LookupRequest::EthCall {
                    id,
                    chain_id,
                    to: registry_backup::REGISTRY.to_owned(),
                    data,
                }
            }
        }
    }

    /// The units a key founded, newest first; `None` = this source did not
    /// answer (or answered something unreadable) and the next one is asked.
    fn unit_ids(self, answer: &LookupAnswer) -> Option<Vec<u64>> {
        match self {
            Source::Index => match answer.outcome {
                LookupOutcome::Failed => None,
                // An unregistered key IS an answer: it founded nothing.
                LookupOutcome::NotFound => Some(Vec::new()),
                LookupOutcome::Ok => {
                    let parsed: KeyProfile = serde_json::from_str(answer.body.as_deref()?).ok()?;
                    Some(
                        parsed
                            .groups
                            .map(|g| g.unit_ids)
                            .unwrap_or_default()
                            .iter()
                            .filter_map(serde_json::Value::as_u64)
                            .collect(),
                    )
                }
            },
            Source::Chain(_) => {
                let bytes = registry_backup::returned(answer)?;
                // No registry at the address on this chain answers `0x`.
                if bytes.is_empty() {
                    return None;
                }
                let values = registry_backup::decode("(uint256,uint256[])", &bytes)?;
                Some(
                    values
                        .get(1)?
                        .as_array()?
                        .iter()
                        .filter_map(|id| id.as_uint().and_then(|(n, _)| u64::try_from(n).ok()))
                        .collect(),
                )
            }
        }
    }

    fn name_in(self, answer: &LookupAnswer, address_lower: &str) -> UnitSays {
        match self {
            Source::Index => match answer.outcome {
                LookupOutcome::Failed => UnitSays::Silent,
                LookupOutcome::NotFound => UnitSays::NotThisWallet,
                LookupOutcome::Ok => answer
                    .body
                    .as_deref()
                    .and_then(|body| name_in_unit(body, address_lower))
                    .map_or(UnitSays::NotThisWallet, UnitSays::Named),
            },
            Source::Chain(_) => {
                let Some(bytes) = registry_backup::returned(answer).filter(|b| !b.is_empty())
                else {
                    return UnitSays::Silent;
                };
                let unit =
                    registry_backup::decode(&format!("({})", registry_backup::UNIT_TUPLE), &bytes)
                        .and_then(|values| registry_backup::unit_from(values.first()?));
                let Some(unit) = unit else {
                    return UnitSays::Silent;
                };
                let hex = primitives::to_hex(&unit.metadata, false);
                name_in_metadata(&hex, address_lower)
                    .map_or(UnitSays::NotThisWallet, UnitSays::Named)
            }
        }
    }
}

/// The name a metadata blob gives `address_lower`, if it is about that address
/// and carries a usable name. A blob that does not decode names nobody.
fn name_in_metadata(metadata_hex: &str, address_lower: &str) -> Option<String> {
    // `decode_hex` is strict UTF-8 by construction (`serde_json::from_slice`):
    // a name is drawn beside somebody's money, and a lenient decoder invents
    // one (issue 200).
    let metadata = RegistryMetadata::decode_hex(metadata_hex).ok()?;
    if metadata.address.to_lowercase() != address_lower {
        return None;
    }
    let name = metadata.key_names.first()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The name this unit gives `address_lower`, if it is about that address and
/// carries a usable name. A blob that does not decode names nobody.
fn name_in_unit(body: &str, address_lower: &str) -> Option<String> {
    let unit: UnitResponse = serde_json::from_str(body).ok()?;
    name_in_metadata(&unit.unit.metadata, address_lower)
}

/// The next thing to do, given everything learned so far.
///
/// `answers` is the whole transcript, in any order; an id answered twice reads
/// its first answer. Unknown ids are ignored.
#[must_use]
pub fn step(address: &str, answers: &[LookupAnswer]) -> LookupStep {
    if !is_askable(address) {
        return done(None, LookupRemember::No);
    }
    let address = address.to_lowercase();
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    // --- Hop 1: the founding key, from the chain --------------------------
    let Some(home) = answer(&format!("cfg:{HOME_CHAIN}")) else {
        return LookupStep::Ask {
            requests: vec![configuration_request(&address, HOME_CHAIN)],
        };
    };
    let mut every_chain_answered = true;
    let public_key = match read_configuration(home) {
        Configured::Key(key) => key,
        home_said => {
            if matches!(home_said, Configured::Unknown) {
                every_chain_answered = false;
            }
            let missing: Vec<LookupRequest> = OTHER_CHAINS
                .iter()
                .filter(|chain| answer(&format!("cfg:{chain}")).is_none())
                .map(|chain| configuration_request(&address, *chain))
                .collect();
            if !missing.is_empty() {
                return LookupStep::Ask { requests: missing };
            }
            let mut found = None;
            for chain in OTHER_CHAINS {
                match answer(&format!("cfg:{chain}")).map(read_configuration) {
                    Some(Configured::Key(key)) if found.is_none() => found = Some(key),
                    Some(Configured::Unknown) | None => every_chain_answered = false,
                    _ => {}
                }
            }
            match found {
                Some(key) => key,
                // "Not a Vela wallet" is only known when every chain said so.
                None if every_chain_answered => return done(None, LookupRemember::Briefly),
                None => return done(None, LookupRemember::No),
            }
        }
    };

    // --- Hops 2 and 3, from the first source that answers -------------------
    //
    // The index service is a CACHE of the registry contract: one HTTP call
    // where the chain takes an `eth_call`, and nothing more. When it does not
    // answer, the same two questions are put to the contract itself — on
    // Gnosis, where the record lives, and then on Ethereum, where a person may
    // have backed it up (spec 062) precisely so that a name, and a wallet,
    // outlive both our server and Gnosis. Unit ids are per DEPLOYMENT (unit 10
    // on Gnosis is unit 0 on Ethereum), so once a source has listed a key's
    // units, those units are asked of that same source and no other.
    'sources: for source in SOURCES {
        let key_id = source.id("key");
        let Some(profile) = answer(&key_id) else {
            return LookupStep::Ask {
                requests: vec![source.key_request(key_id, &public_key)],
            };
        };
        let Some(listed) = source.unit_ids(profile) else {
            continue 'sources; // silent, or something this build cannot read
        };
        // Newest first, one at a time: a wallet registered twice is called by
        // its latest name, and the usual case costs one request.
        for unit_id in listed.iter().copied().take(MAX_UNITS) {
            let id = source.id(&format!("unit:{unit_id}"));
            let Some(unit) = answer(&id) else {
                return LookupStep::Ask {
                    requests: vec![source.unit_request(id, unit_id)],
                };
            };
            match source.name_in(unit, &address) {
                UnitSays::Silent => continue 'sources,
                UnitSays::NotThisWallet => {}
                UnitSays::Named(name) => {
                    return done(
                        Some(WalletName { name, public_key }),
                        LookupRemember::Forever,
                    );
                }
            }
        }
        // This source answered everything and named nobody. Ethereum only
        // holds the wallets somebody backed up, so its "nobody" is not a
        // verdict about a wallet that simply was not.
        return done(
            None,
            if source == Source::Chain(BACKUP_CHAIN) {
                LookupRemember::No
            } else {
                LookupRemember::Briefly
            },
        );
    }
    // Nobody answered.
    done(None, LookupRemember::No)
}

/// [`step`] over JSON, for the bindings: `answers_json` is a `LookupAnswer[]`,
/// the return a `LookupStep`. A transcript that does not parse is read as
/// empty — the lookup starts over rather than failing.
#[must_use]
pub fn step_json(address: &str, answers_json: &str) -> String {
    let answers: Vec<LookupAnswer> = serde_json::from_str(answers_json).unwrap_or_default();
    serde_json::to_string(&step(address, &answers))
        .unwrap_or_else(|_| r#"{"type":"done","found":null,"remember":"no"}"#.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const OTHER: &str = "0x306BceFC18cAc6D2ECFb9D18d0b0A495a6C89aD8";

    fn x() -> String {
        "19".repeat(32)
    }
    fn y() -> String {
        "fe".repeat(32)
    }
    fn key() -> String {
        format!("04{}{}", x(), y())
    }
    fn configured() -> String {
        format!("0x{}{}{:0>64}", x(), y(), "100")
    }
    fn zero() -> String {
        format!("0x{}", "0".repeat(192))
    }

    fn ok(id: &str, body: &str) -> LookupAnswer {
        LookupAnswer {
            id: id.to_owned(),
            outcome: LookupOutcome::Ok,
            body: Some(body.to_owned()),
        }
    }
    fn failed(id: &str) -> LookupAnswer {
        LookupAnswer {
            id: id.to_owned(),
            outcome: LookupOutcome::Failed,
            body: None,
        }
    }
    fn not_found(id: &str) -> LookupAnswer {
        LookupAnswer {
            id: id.to_owned(),
            outcome: LookupOutcome::NotFound,
            body: None,
        }
    }

    fn unit_body(address: &str, names: &[&str]) -> String {
        let metadata = RegistryMetadata {
            version: 1,
            address: address.to_owned(),
            wallet_version: "safe-1.4.1".to_owned(),
            key_names: names.iter().map(|n| (*n).to_owned()).collect(),
            created_at_iso: "2026-08-21T00:00:00Z".to_owned(),
        };
        let hex = metadata.encode_hex().unwrap_or_default();
        format!(
            r#"{{"unit":{{"unitId":1,"metadata":"{hex}"}},"members":{{"total":0,"items":[]}}}}"#
        )
    }

    fn ids(step: &LookupStep) -> Vec<String> {
        match step {
            LookupStep::Ask { requests } => requests
                .iter()
                .map(|r| match r {
                    LookupRequest::EthCall { id, .. } | LookupRequest::IndexGet { id, .. } => {
                        id.clone()
                    }
                })
                .collect(),
            LookupStep::Done { .. } => vec![],
        }
    }

    #[test]
    fn the_first_question_is_the_home_chains_configuration() {
        let LookupStep::Ask { requests } = step(SAFE, &[]) else {
            unreachable!("an empty transcript must ask");
        };
        assert_eq!(
            requests,
            vec![LookupRequest::EthCall {
                id: "cfg:100".to_owned(),
                chain_id: 100,
                to: safe::WEBAUTHN_SIGNER.to_owned(),
                // Verified against the deployed signer on Gnosis (issue 191).
                data: format!(
                    "0xc44b11f7000000000000000000000000{}",
                    &SAFE.to_lowercase()[2..]
                ),
            }]
        );
    }

    #[test]
    fn chain_then_key_then_the_unit_that_names_this_address() {
        let mut transcript = vec![ok("cfg:100", &configured())];
        let LookupStep::Ask { requests } = step(SAFE, &transcript) else {
            unreachable!()
        };
        assert_eq!(
            requests,
            vec![LookupRequest::IndexGet {
                id: "key".to_owned(),
                path: format!("/api/query?publicKey={}", key()),
            }]
        );

        transcript.push(ok(
            "key",
            r#"{"entry":{},"groups":{"total":3,"unitIds":[12,10,8]}}"#,
        ));
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["unit:12"]);

        // The same key founded another wallet; its name is not this one's.
        transcript.push(ok("unit:12", &unit_body(OTHER, &["GateDemo", "Key 2"])));
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["unit:10"]);

        transcript.push(ok("unit:10", &unit_body(SAFE, &["Interleave", "Key 2"])));
        assert_eq!(
            step(SAFE, &transcript),
            LookupStep::Done {
                found: Some(WalletName {
                    name: "Interleave".to_owned(),
                    public_key: key(),
                }),
                remember: LookupRemember::Forever,
            },
            "the newest unit about this address names it, and unit 8 is never fetched"
        );
    }

    #[test]
    fn a_safe_deployed_elsewhere_is_found_on_the_second_tier() {
        let mut transcript = vec![ok("cfg:100", &zero())];
        assert_eq!(
            ids(&step(SAFE, &transcript)),
            vec![
                "cfg:8453",
                "cfg:56",
                "cfg:42161",
                "cfg:10",
                "cfg:137",
                "cfg:1"
            ],
            "the rest are asked together"
        );
        for chain in OTHER_CHAINS {
            let body = if chain == 42161 { configured() } else { zero() };
            transcript.push(ok(&format!("cfg:{chain}"), &body));
        }
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["key"]);
    }

    #[test]
    fn a_plain_address_is_a_miss_remembered_briefly_and_never_reaches_the_index() {
        let mut transcript = vec![ok("cfg:100", &zero())];
        for chain in OTHER_CHAINS {
            // `0x` is what a chain without the signer contract answers.
            transcript.push(ok(&format!("cfg:{chain}"), "0x"));
        }
        assert_eq!(step(SAFE, &transcript), done(None, LookupRemember::Briefly));
    }

    #[test]
    fn a_chain_that_did_not_answer_is_not_a_verdict() {
        let mut transcript = vec![failed("cfg:100")];
        for chain in OTHER_CHAINS {
            transcript.push(ok(&format!("cfg:{chain}"), &zero()));
        }
        assert_eq!(step(SAFE, &transcript), done(None, LookupRemember::No));

        // …but a key found elsewhere still wins over the silence.
        let mut transcript = vec![failed("cfg:100"), ok("cfg:8453", &configured())];
        for chain in [56, 42161, 10, 137, 1] {
            transcript.push(failed(&format!("cfg:{chain}")));
        }
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["key"]);
    }

    // --- The contract as the index's understudy (spec 062) ------------------

    fn chain_groups(id: &str, unit_ids: &[u64]) -> LookupAnswer {
        use alloy_dyn_abi::DynSolValue;
        use alloy_primitives::U256;
        let array = unit_ids
            .iter()
            .map(|n| DynSolValue::Uint(U256::from(*n), 256))
            .collect();
        let bytes = DynSolValue::Tuple(vec![
            DynSolValue::Uint(U256::from(unit_ids.len()), 256),
            DynSolValue::Array(array),
        ])
        .abi_encode_params();
        ok(id, &primitives::to_hex(&bytes, true))
    }

    fn chain_unit(id: &str, address: &str, name: &str) -> LookupAnswer {
        use alloy_dyn_abi::DynSolValue;
        use alloy_primitives::{FixedBytes, U256};
        let metadata = RegistryMetadata {
            version: 1,
            address: address.to_owned(),
            wallet_version: "safe-1.4.1".to_owned(),
            key_names: vec![name.to_owned()],
            created_at_iso: "2026-08-21T00:00:00Z".to_owned(),
        }
        .encode_hex()
        .unwrap_or_default();
        let unit = DynSolValue::Tuple(vec![
            DynSolValue::String("getvela.app".to_owned()),
            DynSolValue::Bytes(primitives::from_hex(&metadata).unwrap_or_default()),
            DynSolValue::Bytes(vec![4; 65]),
            DynSolValue::FixedBytes(FixedBytes::<32>::ZERO, 32),
            DynSolValue::Uint(U256::from(1), 32),
            DynSolValue::Uint(U256::from(1), 256),
        ]);
        let bytes = DynSolValue::Tuple(vec![unit]).abi_encode_params();
        ok(id, &primitives::to_hex(&bytes, true))
    }

    #[test]
    fn an_index_that_is_failing_sends_the_question_to_the_contract_on_gnosis() {
        let mut transcript = vec![ok("cfg:100", &configured()), failed("key")];
        let LookupStep::Ask { requests } = step(SAFE, &transcript) else {
            unreachable!("a silent index is not the end")
        };
        let LookupRequest::EthCall {
            id,
            chain_id,
            to,
            data,
        } = &requests[0]
        else {
            unreachable!("the understudy is the contract")
        };
        assert_eq!((id.as_str(), *chain_id), ("key@100", 100));
        assert_eq!(to, registry_backup::REGISTRY);
        assert!(data.starts_with("0xcc7aae8e"), "getGroupsOfKey: {data}");

        transcript.push(chain_groups("key@100", &[10]));
        // Gnosis listed the units, so Gnosis is asked about them — never the
        // index, whose id space happens to coincide, and never Ethereum.
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["unit:10@100"]);
        transcript.push(chain_unit("unit:10@100", SAFE, "Interleave"));
        assert_eq!(
            step(SAFE, &transcript),
            done(
                Some(WalletName {
                    name: "Interleave".to_owned(),
                    public_key: key(),
                }),
                LookupRemember::Forever
            )
        );
    }

    #[test]
    fn with_gnosis_silent_too_a_backed_up_wallet_is_named_from_ethereum() {
        let mut transcript = vec![
            ok("cfg:8453", &configured()), // deployed on Base; Gnosis is unreachable
            failed("cfg:100"),
            failed("key"),
            failed("key@100"),
        ];
        for chain in [56, 42161, 10, 137, 1] {
            transcript.push(failed(&format!("cfg:{chain}")));
        }
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["key@1"]);
        // On Ethereum the same record is unit 0, not unit 10.
        transcript.push(chain_groups("key@1", &[0]));
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["unit:0@1"]);
        transcript.push(chain_unit("unit:0@1", SAFE, "Interleave"));
        let LookupStep::Done { found, remember } = step(SAFE, &transcript) else {
            unreachable!()
        };
        assert_eq!(found.map(|f| f.name).as_deref(), Some("Interleave"));
        assert_eq!(remember, LookupRemember::Forever);
    }

    #[test]
    fn ethereum_naming_nobody_is_not_a_verdict_and_total_silence_is_not_either() {
        // Ethereum only holds what somebody backed up.
        let silent_until_ethereum = |last: LookupAnswer| {
            vec![
                ok("cfg:100", &configured()),
                failed("key"),
                failed("key@100"),
                last,
            ]
        };
        assert_eq!(
            step(SAFE, &silent_until_ethereum(chain_groups("key@1", &[]))),
            done(None, LookupRemember::No)
        );
        assert_eq!(
            step(SAFE, &silent_until_ethereum(failed("key@1"))),
            done(None, LookupRemember::No)
        );
        // `0x` = no registry at that address on the chain: silence, not "none".
        assert_eq!(
            step(SAFE, &silent_until_ethereum(ok("key@1", "0x"))),
            done(None, LookupRemember::No)
        );
        // Gnosis answering "this key founded nothing" IS a verdict.
        assert_eq!(
            step(
                SAFE,
                &[
                    ok("cfg:100", &configured()),
                    failed("key"),
                    chain_groups("key@100", &[])
                ]
            ),
            done(None, LookupRemember::Briefly)
        );
    }

    #[test]
    fn a_unit_the_index_drops_mid_walk_restarts_on_the_contract() {
        let transcript = vec![
            ok("cfg:100", &configured()),
            ok("key", r#"{"groups":{"unitIds":[3]}}"#),
            failed("unit:3"),
        ];
        assert_eq!(ids(&step(SAFE, &transcript)), vec!["key@100"]);
        // An unregistered key is still an answer, and an unreadable body is not.
        let cfg = ok("cfg:100", &configured());
        assert_eq!(
            step(SAFE, &[cfg.clone(), not_found("key")]),
            done(None, LookupRemember::Briefly)
        );
        assert_eq!(
            ids(&step(SAFE, &[cfg, ok("key", "<html>gateway</html>")])),
            vec!["key@100"]
        );
    }

    #[test]
    fn no_unit_for_this_address_a_blank_name_or_a_garbled_blob_names_nobody() {
        let transcript = vec![
            ok("cfg:100", &configured()),
            ok("key", r#"{"groups":{"unitIds":[1,2,3,4]}}"#),
            ok("unit:1", &unit_body(OTHER, &["Somebody else"])),
            ok("unit:2", &unit_body(SAFE, &["   "])),
            ok("unit:3", r#"{"unit":{"metadata":"fffe"}}"#),
            not_found("unit:4"),
        ];
        assert_eq!(step(SAFE, &transcript), done(None, LookupRemember::Briefly));
    }

    #[test]
    fn unit_ids_are_capped_and_non_numbers_skipped() {
        let listed = r#"{"groups":{"unitIds":["x",9,8,7,6,5,4,3,2,1]}}"#;
        let mut transcript = vec![ok("cfg:100", &configured()), ok("key", listed)];
        let mut asked = Vec::new();
        while let LookupStep::Ask { requests } = step(SAFE, &transcript) {
            for id in ids(&LookupStep::Ask { requests }) {
                asked.push(id.clone());
                transcript.push(not_found(&id));
            }
        }
        assert_eq!(asked.len(), MAX_UNITS);
        assert_eq!(asked[0], "unit:9");
    }

    #[test]
    fn malformed_and_zero_addresses_cost_nothing() {
        for address in ["nope", "0x1234", &format!("0x{}", "0".repeat(40))] {
            assert_eq!(step(address, &[]), done(None, LookupRemember::No));
        }
    }

    #[test]
    fn the_json_door_speaks_the_same_contract() {
        let first = step_json(SAFE, "[]");
        assert!(first.contains(r#""type":"ask""#), "{first}");
        assert!(first.contains(r#""type":"eth_call""#), "{first}");
        assert!(first.contains(r#""chain_id":100"#), "{first}");

        let answers = serde_json::to_string(&vec![
            ok("cfg:100", &configured()),
            ok("key", r#"{"groups":{"unitIds":[10]}}"#),
            ok("unit:10", &unit_body(SAFE, &["Interleave"])),
        ])
        .unwrap_or_default();
        assert_eq!(
            step_json(SAFE, &answers),
            format!(
                r#"{{"type":"done","found":{{"name":"Interleave","public_key":"{}"}},"remember":"forever"}}"#,
                key()
            )
        );
        // A transcript that does not parse starts over; it does not throw.
        assert!(step_json(SAFE, "not json").contains(r#""type":"ask""#));
    }
}
