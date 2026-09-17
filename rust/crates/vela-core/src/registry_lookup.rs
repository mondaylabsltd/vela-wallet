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

/// The name this unit gives `address_lower`, if it is about that address and
/// carries a usable name. A blob that does not decode names nobody.
fn name_in_unit(body: &str, address_lower: &str) -> Option<String> {
    let unit: UnitResponse = serde_json::from_str(body).ok()?;
    // `decode_hex` is strict UTF-8 by construction (`serde_json::from_slice`):
    // a name is drawn beside somebody's money, and a lenient decoder invents
    // one (issue 200).
    let metadata = RegistryMetadata::decode_hex(&unit.unit.metadata).ok()?;
    if metadata.address.to_lowercase() != address_lower {
        return None;
    }
    let name = metadata.key_names.first()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
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

    // --- Hop 2: the units that key founded ---------------------------------
    let Some(profile) = answer("key") else {
        return LookupStep::Ask {
            requests: vec![LookupRequest::IndexGet {
                id: "key".to_owned(),
                path: format!("/api/query?publicKey={public_key}"),
            }],
        };
    };
    let unit_ids: Vec<u64> = match profile.outcome {
        LookupOutcome::Failed => return done(None, LookupRemember::No),
        LookupOutcome::NotFound => return done(None, LookupRemember::Briefly),
        LookupOutcome::Ok => {
            let Some(parsed) = profile
                .body
                .as_deref()
                .and_then(|body| serde_json::from_str::<KeyProfile>(body).ok())
            else {
                // The index answered something this build cannot read.
                return done(None, LookupRemember::No);
            };
            parsed
                .groups
                .map(|g| g.unit_ids)
                .unwrap_or_default()
                .iter()
                .filter_map(serde_json::Value::as_u64)
                .take(MAX_UNITS)
                .collect()
        }
    };

    // --- Hop 3: the unit that names THIS address ---------------------------
    // In the index's order — newest first — and one at a time, so a wallet
    // registered twice is called by its latest name and the usual case costs
    // one request.
    for unit_id in unit_ids {
        let id = format!("unit:{unit_id}");
        let Some(unit) = answer(&id) else {
            return LookupStep::Ask {
                requests: vec![LookupRequest::IndexGet {
                    id,
                    path: format!("/api/query?unitId={unit_id}&pageSize=1"),
                }],
            };
        };
        match unit.outcome {
            LookupOutcome::Failed => return done(None, LookupRemember::No),
            LookupOutcome::NotFound => {}
            LookupOutcome::Ok => {
                if let Some(name) = unit
                    .body
                    .as_deref()
                    .and_then(|body| name_in_unit(body, &address))
                {
                    return done(
                        Some(WalletName { name, public_key }),
                        LookupRemember::Forever,
                    );
                }
            }
        }
    }
    done(None, LookupRemember::Briefly)
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

    #[test]
    fn an_index_that_is_failing_is_not_a_miss() {
        let cfg = ok("cfg:100", &configured());
        assert_eq!(
            step(SAFE, &[cfg.clone(), failed("key")]),
            done(None, LookupRemember::No)
        );
        assert_eq!(
            step(SAFE, &[cfg.clone(), ok("key", "<html>gateway</html>")]),
            done(None, LookupRemember::No)
        );
        let listed = ok("key", r#"{"groups":{"unitIds":[3]}}"#);
        assert_eq!(
            step(SAFE, &[cfg.clone(), listed, failed("unit:3")]),
            done(None, LookupRemember::No)
        );
        // An unregistered key IS an answer.
        assert_eq!(
            step(SAFE, &[cfg, not_found("key")]),
            done(None, LookupRemember::Briefly)
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
