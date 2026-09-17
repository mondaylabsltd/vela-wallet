//! Backing a wallet's founding record up to Ethereum — the contract, written
//! once (spec 062 §5a).
//!
//! ## What is being backed up, and why it needs no signing
//!
//! A Vela address is `f(founding keys)`, and the passkey registry on Gnosis is
//! where those keys are written down: lose that record and a passkey can no
//! longer find its wallet. The registry was built to be copied. It stores the
//! verbatim calldata of every write it accepted (`registerPayloadOf`), and the
//! domain those signatures cover is frozen at construction rather than read
//! from `block.chainid` — so the bytes that were valid on Gnosis are valid on
//! any same-domain deployment, on any chain, **without the person touching a
//! passkey again**. Anyone may submit them; being the sender grants nothing.
//!
//! ## Server-free by construction
//!
//! Every read here is an `eth_call` against the registry contract — the same
//! address on both chains. The index service is never asked: a backup that
//! needed our server in order to work would not be a backup from our server.
//!
//! ```text
//! 1. Ethereum  VERSION()                          no code → Unavailable
//! 2. Gnosis    getGroupsOfKey(foundingKey,0,8,↓)  the units this key founded
//! 3. Gnosis    getUnit(id), newest first          the one whose metadata names
//!                                                 THIS address → groupPublicKey
//! 4. Ethereum  getUnitByGroupKey(groupPublicKey)  exists → BackedUp
//! 5. Gnosis    registerPayloadOf(id)              the bytes → NotBackedUp + call
//! ```
//!
//! The founding key comes from the account record — a wallet knows its own
//! key. Step 4 asks the contract whether the group is there instead of
//! simulating a replay and reading the revert: the same answer, without
//! depending on how an RPC happens to report a custom error.
//!
//! The shell's half is the one `registry_lookup` already defines: perform the
//! requests, append the answers, ask again ([`LookupRequest`], [`LookupAnswer`]).

use alloy_dyn_abi::{DynSolType, DynSolValue, JsonAbiExt as _};
use alloy_json_abi::Function;
use alloy_primitives::U256;
use serde::{Deserialize, Serialize};

use crate::primitives;
use crate::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};
use crate::registry_metadata::RegistryMetadata;

/// `WebAuthnP256PublicKeyRegistry` V13. A CREATE2 deployment: the same
/// bytecode, constructor `(100, 0x5266…edaf)` and salt give this address on
/// every chain, which is what lets a client hardcode one.
pub const REGISTRY: &str = "0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9";

/// Where the record lives (Gnosis) and where the backup goes (Ethereum).
pub const SOURCE_CHAIN: u32 = 100;
pub const TARGET_CHAIN: u32 = 1;

/// A key founds few wallets; the newest are the ones worth a call.
const MAX_UNITS: u64 = 8;

const SIG_VERSION: &str = "VERSION()";
const SIG_GROUPS_OF_KEY: &str = "getGroupsOfKey(bytes,uint256,uint256,bool)";
const SIG_GET_UNIT: &str = "getUnit(uint256)";
const SIG_UNIT_BY_GROUP_KEY: &str = "getUnitByGroupKey(bytes)";
const SIG_REGISTER_PAYLOAD: &str = "registerPayloadOf(uint256)";

/// `Unit { rpId, metadata, groupPublicKey, contentHash, memberCount, createdAt }`.
const UNIT_TUPLE: &str = "(string,bytes,bytes,bytes32,uint32,uint256)";

const PROOF: &str = "(bytes,string,uint256,uint256,uint256,uint256)";

/// The registry's one creating write. A payload that is not exactly this call
/// is not offered to anybody.
fn register_signature() -> String {
    format!("register(string,bytes,bytes,{PROOF},(bytes,bytes,bytes,bytes,bytes,{PROOF})[])")
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackupState {
    /// The registry is not deployed on Ethereum. The row is not drawn.
    Unavailable,
    /// Gnosis has no unit naming this address for this key — a v1-era wallet,
    /// or a key that is not this wallet's founding key.
    NotRegistered,
    /// Ethereum already holds this wallet's group.
    BackedUp,
    /// It does not; `call` is the one transaction that would put it there.
    NotBackedUp,
    /// Somebody did not answer, or answered something this build cannot trust.
    /// Never drawn as either verdict.
    CouldNotCheck,
}

/// The one call that performs the backup, on [`TARGET_CHAIN`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupCall {
    pub chain_id: u32,
    pub to: String,
    /// Decimal wei — always `"0"`; spelled out so a shell builds a `FeeCall`
    /// without inventing a field.
    pub value: String,
    /// The Gnosis registration's verbatim calldata, `0x`-hex.
    pub data: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BackupStep {
    Ask {
        requests: Vec<LookupRequest>,
    },
    Done {
        state: BackupState,
        /// Present exactly when `state` is `NotBackedUp`.
        call: Option<BackupCall>,
        /// The group's unit id on Gnosis, once known — for a receipt line.
        unit_id: Option<u64>,
    },
}

fn done(state: BackupState) -> BackupStep {
    BackupStep::Done {
        state,
        call: None,
        unit_id: None,
    }
}

// ---------------------------------------------------------------------------
// Encoding the five questions
// ---------------------------------------------------------------------------

fn call_data(signature: &str, args: &[DynSolValue]) -> Option<String> {
    let mut data = primitives::function_selector(signature).ok()?;
    data.extend(DynSolValue::Tuple(args.to_vec()).abi_encode_params());
    Some(primitives::to_hex(&data, true))
}

fn ask(id: &str, chain_id: u32, signature: &str, args: &[DynSolValue]) -> Option<BackupStep> {
    Some(BackupStep::Ask {
        requests: vec![LookupRequest::EthCall {
            id: id.to_owned(),
            chain_id,
            to: REGISTRY.to_owned(),
            data: call_data(signature, args)?,
        }],
    })
}

// ---------------------------------------------------------------------------
// Reading the answers
// ---------------------------------------------------------------------------

/// The returned bytes of an answered call. `None` = nobody answered (or not
/// hex) — which is never a verdict.
fn returned(answer: &LookupAnswer) -> Option<Vec<u8>> {
    if answer.outcome != LookupOutcome::Ok {
        return None;
    }
    primitives::from_hex(answer.body.as_deref()?).ok()
}

fn decode(types: &str, data: &[u8]) -> Option<Vec<DynSolValue>> {
    match DynSolType::parse(types)
        .ok()?
        .abi_decode_params(data)
        .ok()?
    {
        DynSolValue::Tuple(values) => Some(values),
        single => Some(vec![single]),
    }
}

struct Unit {
    metadata: Vec<u8>,
    group_public_key: Vec<u8>,
}

fn unit_from(value: &DynSolValue) -> Option<Unit> {
    let fields = value.as_tuple()?;
    Some(Unit {
        metadata: fields.get(1)?.as_bytes()?.to_vec(),
        group_public_key: fields.get(2)?.as_bytes()?.to_vec(),
    })
}

/// Is this unit's metadata about `address_lower`? A blob that does not decode
/// is about nobody.
fn names(unit: &Unit, address_lower: &str) -> bool {
    RegistryMetadata::decode_hex(&primitives::to_hex(&unit.metadata, false))
        .is_ok_and(|metadata| metadata.address.to_lowercase() == address_lower)
}

/// The payload is exactly `register(...)` for THIS unit. The Ethereum contract
/// re-verifies every signature anyway; this is so a person is never shown a
/// fee for bytes that are about something else.
fn payload_is_this_unit(payload: &[u8], unit: &Unit) -> bool {
    let Ok(register) = Function::parse(&register_signature()) else {
        return false;
    };
    if payload.len() < 4 || payload[..4] != register.selector()[..] {
        return false;
    }
    let Ok(inputs) = register.abi_decode_input(&payload[4..]) else {
        return false;
    };
    inputs.get(1).and_then(DynSolValue::as_bytes) == Some(unit.metadata.as_slice())
        && inputs.get(2).and_then(DynSolValue::as_bytes) == Some(unit.group_public_key.as_slice())
}

fn public_key_bytes(public_key_hex: &str) -> Option<Vec<u8>> {
    let bytes = primitives::from_hex(public_key_hex).ok()?;
    match bytes.len() {
        65 if bytes[0] == 0x04 => Some(bytes),
        // The account record's older shape: bare x‖y.
        64 => Some([&[0x04][..], &bytes].concat()),
        _ => None,
    }
}

fn is_address(address: &str) -> bool {
    address
        .strip_prefix("0x")
        .is_some_and(|hex| hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
}

// ---------------------------------------------------------------------------
// The walk
// ---------------------------------------------------------------------------

/// The next thing to do, given everything learned so far.
///
/// `address` is the wallet; `founding_public_key_hex` its FIRST founding key
/// (`Account.keys[0]`, or the legacy `public_key_hex`), uncompressed. `answers`
/// is the whole transcript, in any order.
#[must_use]
pub fn step(address: &str, founding_public_key_hex: &str, answers: &[LookupAnswer]) -> BackupStep {
    let could_not = || done(BackupState::CouldNotCheck);
    let Some(public_key) = public_key_bytes(founding_public_key_hex) else {
        return could_not();
    };
    if !is_address(address) {
        return could_not();
    }
    let address = address.to_lowercase();
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    // 1. Is there anywhere to back up TO?
    let Some(target) = answer("target") else {
        return ask("target", TARGET_CHAIN, SIG_VERSION, &[]).unwrap_or_else(could_not);
    };
    match returned(target) {
        None => return could_not(),
        // No contract at the address answers `0x`.
        Some(bytes) if bytes.is_empty() => return done(BackupState::Unavailable),
        Some(_) => {}
    }

    // 2. The units this key founded, newest first.
    let Some(groups) = answer("groups") else {
        let args = [
            DynSolValue::Bytes(public_key),
            DynSolValue::Uint(U256::ZERO, 256),
            DynSolValue::Uint(U256::from(MAX_UNITS), 256),
            DynSolValue::Bool(true),
        ];
        return ask("groups", SOURCE_CHAIN, SIG_GROUPS_OF_KEY, &args).unwrap_or_else(could_not);
    };
    let Some(unit_ids) = returned(groups)
        .and_then(|bytes| decode("(uint256,uint256[])", &bytes))
        .and_then(|values| {
            values.get(1)?.as_array().map(|ids| {
                ids.iter()
                    .filter_map(|id| id.as_uint().and_then(|(n, _)| u64::try_from(n).ok()))
                    .collect::<Vec<u64>>()
            })
        })
    else {
        return could_not();
    };

    // 3. The one that names THIS address.
    let mut found: Option<(u64, Unit)> = None;
    for unit_id in unit_ids {
        let id = format!("unit:{unit_id}");
        let Some(got) = answer(&id) else {
            let args = [DynSolValue::Uint(U256::from(unit_id), 256)];
            return ask(&id, SOURCE_CHAIN, SIG_GET_UNIT, &args).unwrap_or_else(could_not);
        };
        let Some(unit) = returned(got)
            .and_then(|bytes| decode(&format!("({UNIT_TUPLE})"), &bytes))
            .and_then(|values| unit_from(values.first()?))
        else {
            return could_not();
        };
        if names(&unit, &address) {
            found = Some((unit_id, unit));
            break;
        }
    }
    let Some((unit_id, unit)) = found else {
        return done(BackupState::NotRegistered);
    };

    // 4. Is it on Ethereum already?
    let Some(mirrored) = answer("mirrored") else {
        let args = [DynSolValue::Bytes(unit.group_public_key.clone())];
        return ask("mirrored", TARGET_CHAIN, SIG_UNIT_BY_GROUP_KEY, &args)
            .unwrap_or_else(could_not);
    };
    let Some(exists) = returned(mirrored)
        .and_then(|bytes| decode(&format!("(bool,uint256,{UNIT_TUPLE})"), &bytes))
        .and_then(|values| values.first()?.as_bool())
    else {
        return could_not();
    };
    if exists {
        return BackupStep::Done {
            state: BackupState::BackedUp,
            call: None,
            unit_id: Some(unit_id),
        };
    }

    // 5. The bytes that would put it there.
    let Some(payload) = answer("payload") else {
        let args = [DynSolValue::Uint(U256::from(unit_id), 256)];
        return ask("payload", SOURCE_CHAIN, SIG_REGISTER_PAYLOAD, &args).unwrap_or_else(could_not);
    };
    let Some(payload) = returned(payload)
        .and_then(|bytes| decode("(bytes)", &bytes))
        .and_then(|values| values.first()?.as_bytes().map(<[u8]>::to_vec))
        .filter(|payload| payload_is_this_unit(payload, &unit))
    else {
        // Includes a unit migrated from before V13 stored payloads (empty
        // bytes): there is nothing safe to offer.
        return could_not();
    };
    BackupStep::Done {
        state: BackupState::NotBackedUp,
        call: Some(BackupCall {
            chain_id: TARGET_CHAIN,
            to: REGISTRY.to_owned(),
            value: "0".to_owned(),
            data: primitives::to_hex(&payload, true),
        }),
        unit_id: Some(unit_id),
    }
}

/// [`step`] over JSON, for the bindings: `answers_json` is a `LookupAnswer[]`,
/// the return a `BackupStep`. A transcript that does not parse is read as
/// empty — the walk starts over rather than failing.
#[must_use]
pub fn step_json(address: &str, founding_public_key_hex: &str, answers_json: &str) -> String {
    let answers: Vec<LookupAnswer> = serde_json::from_str(answers_json).unwrap_or_default();
    serde_json::to_string(&step(address, founding_public_key_hex, &answers)).unwrap_or_else(|_| {
        r#"{"type":"done","state":"could_not_check","call":null,"unit_id":null}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::FixedBytes;

    const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const OTHER: &str = "0x306BceFC18cAc6D2ECFb9D18d0b0A495a6C89aD8";

    fn founding_key() -> String {
        format!("04{}{}", "19".repeat(32), "fe".repeat(32))
    }
    fn group_key(tag: u8) -> Vec<u8> {
        [vec![0x04], vec![tag; 64]].concat()
    }
    fn metadata(address: &str) -> Vec<u8> {
        let hex = RegistryMetadata {
            version: 1,
            address: address.to_owned(),
            wallet_version: "safe-1.4.1".to_owned(),
            key_names: vec!["Interleave".to_owned()],
            created_at_iso: "2026-08-21T00:00:00Z".to_owned(),
        }
        .encode_hex()
        .unwrap_or_default();
        primitives::from_hex(&hex).unwrap_or_default()
    }
    fn unit_value(address: &str, tag: u8) -> DynSolValue {
        DynSolValue::Tuple(vec![
            DynSolValue::String("getvela.app".to_owned()),
            DynSolValue::Bytes(metadata(address)),
            DynSolValue::Bytes(group_key(tag)),
            DynSolValue::FixedBytes(FixedBytes::<32>::ZERO, 32),
            DynSolValue::Uint(U256::from(3), 32),
            DynSolValue::Uint(U256::from(1_787_833_395u64), 256),
        ])
    }
    fn hex_of(values: Vec<DynSolValue>) -> String {
        primitives::to_hex(&DynSolValue::Tuple(values).abi_encode_params(), true)
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

    fn deployed() -> LookupAnswer {
        ok(
            "target",
            &hex_of(vec![DynSolValue::Uint(U256::from(13), 256)]),
        )
    }
    fn groups(ids: &[u64]) -> LookupAnswer {
        let array = ids
            .iter()
            .map(|id| DynSolValue::Uint(U256::from(*id), 256))
            .collect();
        ok(
            "groups",
            &hex_of(vec![
                DynSolValue::Uint(U256::from(ids.len()), 256),
                DynSolValue::Array(array),
            ]),
        )
    }
    fn unit(id: u64, address: &str, tag: u8) -> LookupAnswer {
        ok(
            &format!("unit:{id}"),
            &hex_of(vec![unit_value(address, tag)]),
        )
    }
    fn mirrored(exists: bool) -> LookupAnswer {
        ok(
            "mirrored",
            &hex_of(vec![
                DynSolValue::Bool(exists),
                DynSolValue::Uint(U256::ZERO, 256),
                unit_value(if exists { SAFE } else { "" }, 7),
            ]),
        )
    }
    fn proof() -> DynSolValue {
        DynSolValue::Tuple(vec![
            DynSolValue::Bytes(vec![1; 37]),
            DynSolValue::String("{}".to_owned()),
            DynSolValue::Uint(U256::from(23), 256),
            DynSolValue::Uint(U256::from(1), 256),
            DynSolValue::Uint(U256::from(5), 256),
            DynSolValue::Uint(U256::from(6), 256),
        ])
    }
    /// A `register(...)` calldata for (`address`, group `tag`).
    fn register_payload(address: &str, tag: u8) -> Vec<u8> {
        let member = DynSolValue::Tuple(vec![
            DynSolValue::Bytes(group_key(9)),
            DynSolValue::Bytes(vec![1; 20]),
            DynSolValue::Bytes(vec![2; 16]),
            DynSolValue::Bytes(b"platform".to_vec()),
            DynSolValue::Bytes(b"internal".to_vec()),
            proof(),
        ]);
        let args = DynSolValue::Tuple(vec![
            DynSolValue::String("getvela.app".to_owned()),
            DynSolValue::Bytes(metadata(address)),
            DynSolValue::Bytes(group_key(tag)),
            proof(),
            DynSolValue::Array(vec![member]),
        ]);
        let mut data = primitives::function_selector(&register_signature()).unwrap_or_default();
        data.extend(args.abi_encode_params());
        data
    }
    fn payload(bytes: Vec<u8>) -> LookupAnswer {
        ok("payload", &hex_of(vec![DynSolValue::Bytes(bytes)]))
    }

    fn asked(step: &BackupStep) -> (String, u32, String) {
        let BackupStep::Ask { requests } = step else {
            unreachable!("expected a question, got {step:?}");
        };
        let LookupRequest::EthCall {
            id,
            chain_id,
            to,
            data,
        } = &requests[0]
        else {
            unreachable!("the backup only ever makes eth_calls");
        };
        assert_eq!(to, REGISTRY, "every question goes to the registry");
        (id.clone(), *chain_id, data[..10].to_owned())
    }

    #[test]
    fn the_selectors_are_the_deployed_contracts() {
        // Verified against the live Gnosis registry, 2026-09-17 (spec 062 §2).
        for (signature, selector) in [
            (SIG_GROUPS_OF_KEY, "0xcc7aae8e"),
            (SIG_GET_UNIT, "0x1655bfbe"),
            (SIG_REGISTER_PAYLOAD, "0x19178efe"),
            (register_signature().as_str(), "0xcd438f9b"),
        ] {
            let got = primitives::function_selector(signature).unwrap_or_default();
            assert_eq!(primitives::to_hex(&got, true), selector, "{signature}");
        }
    }

    #[test]
    fn the_walk_asks_five_questions_in_order_and_offers_the_gnosis_bytes() {
        let key = founding_key();
        let mut transcript = Vec::new();
        assert_eq!(
            asked(&step(SAFE, &key, &transcript)),
            ("target".into(), 1, "0x".to_owned() + "ffa1ad74")
        );

        transcript.push(deployed());
        assert_eq!(asked(&step(SAFE, &key, &transcript)).0, "groups");
        assert_eq!(asked(&step(SAFE, &key, &transcript)).1, 100);

        transcript.push(groups(&[12, 10, 8]));
        assert_eq!(asked(&step(SAFE, &key, &transcript)).0, "unit:12");
        // The same key founded another wallet; that one is not being backed up.
        transcript.push(unit(12, OTHER, 1));
        assert_eq!(asked(&step(SAFE, &key, &transcript)).0, "unit:10");
        transcript.push(unit(10, SAFE, 2));

        // Unit 8 is never fetched: the newest unit naming this address wins.
        let (id, chain, _) = asked(&step(SAFE, &key, &transcript));
        assert_eq!((id.as_str(), chain), ("mirrored", 1));
        transcript.push(mirrored(false));

        let (id, chain, _) = asked(&step(SAFE, &key, &transcript));
        assert_eq!((id.as_str(), chain), ("payload", 100));
        let bytes = register_payload(SAFE, 2);
        transcript.push(payload(bytes.clone()));

        assert_eq!(
            step(SAFE, &key, &transcript),
            BackupStep::Done {
                state: BackupState::NotBackedUp,
                call: Some(BackupCall {
                    chain_id: 1,
                    to: REGISTRY.to_owned(),
                    value: "0".to_owned(),
                    data: primitives::to_hex(&bytes, true),
                }),
                unit_id: Some(10),
            }
        );
    }

    #[test]
    fn a_registry_that_is_not_on_ethereum_is_unavailable_not_an_error() {
        assert_eq!(
            step(SAFE, &founding_key(), &[ok("target", "0x")]),
            done(BackupState::Unavailable)
        );
    }

    #[test]
    fn a_group_ethereum_already_holds_is_backed_up_and_no_payload_is_fetched() {
        let transcript = [deployed(), groups(&[10]), unit(10, SAFE, 2), mirrored(true)];
        assert_eq!(
            step(SAFE, &founding_key(), &transcript),
            BackupStep::Done {
                state: BackupState::BackedUp,
                call: None,
                unit_id: Some(10),
            }
        );
    }

    #[test]
    fn a_key_that_founded_nothing_for_this_address_is_not_registered() {
        let key = founding_key();
        assert_eq!(
            step(SAFE, &key, &[deployed(), groups(&[])]),
            done(BackupState::NotRegistered)
        );
        assert_eq!(
            step(SAFE, &key, &[deployed(), groups(&[12]), unit(12, OTHER, 1)]),
            done(BackupState::NotRegistered)
        );
    }

    #[test]
    fn a_payload_about_something_else_is_never_offered() {
        let head = vec![
            deployed(),
            groups(&[10]),
            unit(10, SAFE, 2),
            mirrored(false),
        ];
        for wrong in [
            register_payload(OTHER, 2),   // another wallet's metadata
            register_payload(SAFE, 3),    // another group key
            vec![0xde, 0xad, 0xbe, 0xef], // not a register call at all
            Vec::new(),                   // a unit from before payloads were stored
        ] {
            let mut transcript = head.clone();
            transcript.push(payload(wrong));
            assert_eq!(
                step(SAFE, &founding_key(), &transcript),
                done(BackupState::CouldNotCheck)
            );
        }
    }

    #[test]
    fn silence_anywhere_is_could_not_check_never_a_verdict() {
        let key = founding_key();
        let full = [
            deployed(),
            groups(&[10]),
            unit(10, SAFE, 2),
            mirrored(false),
        ];
        for cut in 0..full.len() {
            let mut transcript = full[..cut].to_vec();
            let silent = match cut {
                0 => "target",
                1 => "groups",
                2 => "unit:10",
                _ => "mirrored",
            };
            transcript.push(failed(silent));
            assert_eq!(
                step(SAFE, &key, &transcript),
                done(BackupState::CouldNotCheck),
                "a silent `{silent}` must not read as backed up OR as not"
            );
        }
        let mut transcript = full.to_vec();
        transcript.push(failed("payload"));
        assert_eq!(
            step(SAFE, &key, &transcript),
            done(BackupState::CouldNotCheck)
        );
        // Garbage that is hex but not the ABI shape, too.
        assert_eq!(
            step(SAFE, &key, &[deployed(), ok("groups", "0x1234")]),
            done(BackupState::CouldNotCheck)
        );
    }

    #[test]
    fn the_legacy_bare_key_and_a_malformed_one() {
        let bare = format!("{}{}", "19".repeat(32), "fe".repeat(32));
        assert_eq!(asked(&step(SAFE, &bare, &[])).0, "target");
        assert_eq!(step(SAFE, "04abcd", &[]), done(BackupState::CouldNotCheck));
        assert_eq!(
            step("nope", &founding_key(), &[]),
            done(BackupState::CouldNotCheck)
        );
    }

    #[test]
    fn the_json_door_speaks_the_same_contract() {
        let first = step_json(SAFE, &founding_key(), "[]");
        assert!(
            first.contains(r#""type":"ask""#) && first.contains(r#""chain_id":1"#),
            "{first}"
        );
        let answers = serde_json::to_string(&vec![ok("target", "0x")]).unwrap_or_default();
        assert_eq!(
            step_json(SAFE, &founding_key(), &answers),
            r#"{"type":"done","state":"unavailable","call":null,"unit_id":null}"#
        );
    }
}
