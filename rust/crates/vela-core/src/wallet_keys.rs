//! Which passkeys control a wallet — the keys view in Settings (spec 062).
//!
//! A Vela address is `f(all founding keys)`, and ANY one of them can sign. A
//! person who is offered "back up your keys" is owed the sight of them first:
//! how many there are, what each is called, and which vault is holding it.
//!
//! The account record on the device knows the keys' names and nothing about
//! where they live. The registry knows the rest — each member's attestation
//! carries the authenticator model (AAGUID) and whether the credential is
//! synced — and the registry's metadata carries the names as the owner wrote
//! them. So the rows come from the CONTRACT, read directly (Gnosis, then the
//! Ethereum backup; never our index server), and fall back to what the device
//! remembers when no chain answers. A fallback row says less; it never says
//! something else.
//!
//! Same shape as the other walks: a pure function of a transcript. A shell
//! performs the `eth_call`s it is handed and asks again.
//!
//! ```text
//! 1. chain  getGroupsOfKey(key[0])      the units this key founded
//! 2. chain  getUnit(id), newest first   the one whose metadata names THIS address
//! 3. chain  getGroupMembers(id)         the founding set, in founding order
//! ```

use alloy_dyn_abi::DynSolValue;
use alloy_primitives::U256;
use serde::{Deserialize, Serialize};

use crate::registry_backup::{
    call_data, decode, is_address, names, public_key_bytes, returned, unit_from, MAX_UNITS,
    REGISTRY, SIG_GET_UNIT, SIG_GROUPS_OF_KEY, UNIT_TUPLE,
};
use crate::registry_chain::{self, READ_CHAINS};
use crate::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};
use crate::registry_metadata::RegistryMetadata;
use crate::{passkey, primitives};

/// One key as the device's account record holds it. Both spellings are read:
/// the retired client wrote camelCase at the same web origin.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct DeviceKey {
    #[serde(default, alias = "publicKeyHex")]
    pub public_key_hex: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub transports: String,
}

/// Where the rows came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeysSource {
    /// The registry contract answered: every field is filled.
    Registry,
    /// No chain answered. Names and transports only, from the account record.
    Device,
    /// A chain DID answer, and holds no record naming this address — a v1-era
    /// wallet, or one created before its registration landed. The same rows as
    /// [`Self::Device`], but nothing was unreachable and no shell may say so.
    NotRegistered,
}

/// One row of the keys view. The first eight fields are `CreateKeyRow`'s, in
/// its spelling, so the shells draw it with the component the create flow
/// already has.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletKeyRow {
    /// The owner's label; EMPTY when nobody recorded one — the shell then says
    /// "Key n".
    pub name: String,
    pub authenticator_attachment: String,
    pub transports: String,
    /// Always `true`: a key that is in the founding set confirmed its membership.
    pub confirmed: bool,
    /// Backed up to a sync fabric. `None` when only the device answered — a
    /// badge nobody can vouch for is not drawn.
    pub synced: Option<bool>,
    pub aaguid: String,
    /// "Apple Passwords", "1Password" … empty when the catalog cannot name it.
    pub provider_name: String,
    /// `platform` | `hybrid` | `security_key`.
    pub method: String,
    /// Uncompressed `04‖x‖y`, lowercase bare hex.
    pub public_key_hex: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum KeysStep {
    Ask {
        requests: Vec<LookupRequest>,
    },
    Done {
        source: KeysSource,
        /// The chain whose registry answered; `None` for [`KeysSource::Device`].
        chain_id: Option<u32>,
        keys: Vec<WalletKeyRow>,
    },
}

/// What kind of authenticator the hints describe. A removable transport wins:
/// a security key may also report `hybrid`, a phone never reports `usb`.
fn method_of(attachment: &str, transports: &str) -> &'static str {
    let has = |hint: &str| transports.split(',').any(|t| t.trim() == hint);
    if has("usb") || has("nfc") || has("ble") {
        "security_key"
    } else if has("hybrid") && (attachment == "cross-platform" || !has("internal")) {
        "hybrid"
    } else {
        "platform"
    }
}

fn normal_key(public_key_hex: &str) -> Option<String> {
    Some(primitives::to_hex(
        &public_key_bytes(public_key_hex)?,
        false,
    ))
}

fn device_rows(device: &[DeviceKey]) -> Vec<WalletKeyRow> {
    device
        .iter()
        .map(|key| WalletKeyRow {
            name: key.name.clone(),
            authenticator_attachment: String::new(),
            transports: key.transports.clone(),
            confirmed: true,
            synced: None,
            aaguid: String::new(),
            provider_name: String::new(),
            method: method_of("", &key.transports).to_owned(),
            public_key_hex: normal_key(&key.public_key_hex).unwrap_or_default(),
        })
        .collect()
}

fn from_device(device: &[DeviceKey]) -> KeysStep {
    device_done(KeysSource::Device, device)
}

fn device_done(source: KeysSource, device: &[DeviceKey]) -> KeysStep {
    KeysStep::Done {
        source,
        chain_id: None,
        keys: device_rows(device),
    }
}

fn eth_call(id: String, chain_id: u32, data: String) -> LookupRequest {
    LookupRequest::EthCall {
        id,
        chain_id,
        to: REGISTRY.to_owned(),
        data,
    }
}

/// Rows from the unit body `registry_chain::unit_json` produced, named from the
/// unit's own metadata and, where that is silent, from the device.
fn registry_rows(unit_body: &str, device: &[DeviceKey]) -> Option<Vec<WalletKeyRow>> {
    let body: serde_json::Value = serde_json::from_str(unit_body).ok()?;
    let names = body["unit"]["metadata"]
        .as_str()
        .and_then(|hex| RegistryMetadata::decode_hex(hex).ok())
        .map(|metadata| metadata.key_names)
        .unwrap_or_default();
    let items = body["members"]["items"].as_array()?;
    let mut rows = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let public_key_hex = normal_key(item["publicKey"].as_str()?)?;
        let attachment = item["authenticatorAttachment"].as_str().unwrap_or_default();
        let transports = item["transports"].as_str().unwrap_or_default();
        let (aaguid, synced) =
            passkey::attestation_signals(item["attestation"].as_str().unwrap_or_default());
        let remembered = device
            .iter()
            .find(|key| normal_key(&key.public_key_hex).as_deref() == Some(&public_key_hex));
        let name = names
            .get(index)
            .filter(|name| !name.is_empty())
            .cloned()
            .or_else(|| remembered.map(|key| key.name.clone()))
            .unwrap_or_default();
        rows.push(WalletKeyRow {
            name,
            authenticator_attachment: attachment.to_owned(),
            transports: transports.to_owned(),
            confirmed: true,
            synced: Some(synced),
            provider_name: passkey::provider_name(&aaguid)
                .unwrap_or_default()
                .to_owned(),
            aaguid,
            method: method_of(attachment, transports).to_owned(),
            public_key_hex,
        });
    }
    Some(rows)
}

/// The next thing to do, given everything learned so far.
///
/// `device` is the account record's key list in founding order (a legacy
/// single-key record is a list of one). `answers` is the whole transcript.
#[must_use]
pub fn step(address: &str, device: &[DeviceKey], answers: &[LookupAnswer]) -> KeysStep {
    let Some(first) = device
        .first()
        .and_then(|key| public_key_bytes(&key.public_key_hex))
    else {
        return from_device(device);
    };
    if !is_address(address) {
        return from_device(device);
    }
    let address = address.to_lowercase();
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    // Did any registry give a complete answer that simply has no unit for this
    // address? That is "not registered", which is not "unreachable".
    let mut answered_without_it = false;

    'chains: for chain_id in READ_CHAINS {
        // 1. The units this key founded, newest first.
        let groups_id = format!("groups@{chain_id}");
        let Some(groups) = answer(&groups_id) else {
            let args = [
                DynSolValue::Bytes(first.clone()),
                DynSolValue::Uint(U256::ZERO, 256),
                DynSolValue::Uint(U256::from(MAX_UNITS), 256),
                DynSolValue::Bool(true),
            ];
            let Some(data) = call_data(SIG_GROUPS_OF_KEY, &args) else {
                return from_device(device);
            };
            return KeysStep::Ask {
                requests: vec![eth_call(groups_id, chain_id, data)],
            };
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
            // Silent, or no registry here: the next chain may know.
            continue;
        };

        // 2. The one that names THIS address.
        for unit_id in unit_ids {
            let unit_key = format!("unit:{unit_id}@{chain_id}");
            let Some(got) = answer(&unit_key) else {
                let args = [DynSolValue::Uint(U256::from(unit_id), 256)];
                let Some(data) = call_data(SIG_GET_UNIT, &args) else {
                    return from_device(device);
                };
                return KeysStep::Ask {
                    requests: vec![eth_call(unit_key, chain_id, data)],
                };
            };
            let Some(unit) = returned(got)
                .and_then(|bytes| decode(&format!("({UNIT_TUPLE})"), &bytes))
                .and_then(|values| unit_from(values.first()?))
            else {
                continue 'chains;
            };
            if !names(&unit, &address) {
                continue;
            }

            // 3. Its founding set, in founding order.
            let members_key = format!("members:{unit_id}@{chain_id}");
            let Some(members) = answer(&members_key) else {
                let Some(call) = registry_chain::unit_calls(unit_id).and_then(|mut c| c.pop())
                else {
                    return from_device(device);
                };
                return KeysStep::Ask {
                    requests: vec![eth_call(members_key, chain_id, call.data)],
                };
            };
            let rows = (members.outcome == LookupOutcome::Ok)
                .then(|| got.body.as_deref().zip(members.body.as_deref()))
                .flatten()
                .and_then(|(unit_hex, members_hex)| {
                    registry_chain::unit_json(unit_id, unit_hex, members_hex)
                })
                .and_then(|body| registry_rows(&body, device));
            match rows {
                Some(keys) if !keys.is_empty() => {
                    return KeysStep::Done {
                        source: KeysSource::Registry,
                        chain_id: Some(chain_id),
                        keys,
                    };
                }
                _ => continue 'chains,
            }
        }
        // This chain answered every question and holds no unit for the address:
        // on Gnosis that is a v1-era wallet. The next chain is still worth asking.
        answered_without_it = true;
    }
    if answered_without_it {
        device_done(KeysSource::NotRegistered, device)
    } else {
        from_device(device)
    }
}

/// The JSON door the bindings use. `device_keys_json` is the account record's
/// `keys` array (or a one-element array built from the legacy scalars);
/// anything unreadable is an empty list, and the answer is then an empty view.
#[must_use]
pub fn step_json(address: &str, device_keys_json: &str, answers_json: &str) -> String {
    let device: Vec<DeviceKey> = serde_json::from_str(device_keys_json).unwrap_or_default();
    let answers: Vec<LookupAnswer> = serde_json::from_str(answers_json).unwrap_or_default();
    serde_json::to_string(&step(address, &device, &answers)).unwrap_or_else(|_| {
        r#"{"type":"done","source":"device","chain_id":null,"keys":[]}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    /// Web's verbatim recording of the contract on both chains, keyed by chain
    /// then by calldata — the same bytes the four shells' sign-in tests read.
    fn recorded() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../../../app-web/vela-wallet/src/lib/onboarding/core/__fixtures__/registry-chain.json"
        ))
        .unwrap_or_default()
    }

    fn device() -> Vec<DeviceKey> {
        vec![DeviceKey {
            public_key_hex: recorded()["publicKey"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            name: "Parallel Multi".to_owned(),
            transports: "internal".to_owned(),
        }]
    }

    /// Drive the walk against the recording, with some chains "down".
    fn run(down: &[u32]) -> (KeysStep, Vec<u32>) {
        let fixture = recorded();
        let mut answers = Vec::new();
        let mut asked = Vec::new();
        for _ in 0..32 {
            match step(SAFE, &device(), &answers) {
                KeysStep::Ask { requests } => {
                    for request in requests {
                        let LookupRequest::EthCall {
                            id, chain_id, data, ..
                        } = request
                        else {
                            unreachable!()
                        };
                        asked.push(chain_id);
                        let body = (!down.contains(&chain_id))
                            .then(|| fixture["answers"][chain_id.to_string()][&data].as_str())
                            .flatten();
                        answers.push(LookupAnswer {
                            id,
                            outcome: if body.is_some() {
                                LookupOutcome::Ok
                            } else {
                                LookupOutcome::Failed
                            },
                            body: body.map(str::to_owned),
                        });
                    }
                }
                done => return (done, asked),
            }
        }
        unreachable!("the walk never finished")
    }

    #[test]
    fn the_golden_wallets_three_keys_come_from_gnosis_in_founding_order() {
        let (done, asked) = run(&[]);
        let KeysStep::Done {
            source,
            chain_id,
            keys,
        } = done
        else {
            unreachable!()
        };
        assert_eq!((source, chain_id), (KeysSource::Registry, Some(100)));
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0].public_key_hex, device()[0].public_key_hex);
        assert!(keys.iter().all(|key| key.confirmed && key.synced.is_some()));
        assert!(keys.iter().all(|key| key.public_key_hex.len() == 130));
        // Every row is called something: the registry's metadata carries the names.
        assert!(keys.iter().all(|key| !key.name.is_empty()), "{keys:?}");
        assert!(asked.iter().all(|chain| *chain == 100));
    }

    #[test]
    fn a_silent_gnosis_is_answered_by_the_ethereum_backup() {
        let (done, _) = run(&[100]);
        let KeysStep::Done {
            source,
            chain_id,
            keys,
        } = done
        else {
            unreachable!()
        };
        assert_eq!((source, chain_id), (KeysSource::Registry, Some(1)));
        assert_eq!(keys.len(), 3);
    }

    #[test]
    fn with_no_chain_the_device_says_what_it_knows_and_vouches_for_nothing() {
        let (done, _) = run(&[100, 1]);
        let KeysStep::Done {
            source,
            chain_id,
            keys,
        } = done
        else {
            unreachable!()
        };
        assert_eq!((source, chain_id), (KeysSource::Device, None));
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].name, "Parallel Multi");
        assert_eq!(keys[0].synced, None);
        assert_eq!(keys[0].provider_name, "");
    }

    #[test]
    fn a_registry_that_answers_with_nothing_is_not_an_unreachable_one() {
        // The same key, asked about an address none of its units names.
        let fixture = recorded();
        let other = "0xD40086000000000000000000000000000000130b";
        let mut answers = Vec::new();
        let done = loop {
            match step(other, &device(), &answers) {
                KeysStep::Ask { requests } => {
                    for request in requests {
                        let LookupRequest::EthCall {
                            id, chain_id, data, ..
                        } = request
                        else {
                            unreachable!()
                        };
                        let body = fixture["answers"][chain_id.to_string()][&data].as_str();
                        answers.push(LookupAnswer {
                            id,
                            outcome: if body.is_some() {
                                LookupOutcome::Ok
                            } else {
                                LookupOutcome::Failed
                            },
                            body: body.map(str::to_owned),
                        });
                    }
                }
                done => break done,
            }
        };
        let KeysStep::Done { source, keys, .. } = done else {
            unreachable!()
        };
        assert_eq!(source, KeysSource::NotRegistered);
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].synced, None);
    }

    #[test]
    fn a_record_without_a_readable_key_asks_nobody() {
        let none = step(SAFE, &[], &[]);
        assert!(
            matches!(none, KeysStep::Done { source: KeysSource::Device, ref keys, .. } if keys.is_empty())
        );
        let junk = [DeviceKey {
            public_key_hex: "zz".into(),
            ..DeviceKey::default()
        }];
        assert!(matches!(
            step(SAFE, &junk, &[]),
            KeysStep::Done {
                source: KeysSource::Device,
                ..
            }
        ));
    }

    #[test]
    fn the_method_is_read_from_the_hints() {
        assert_eq!(method_of("platform", "internal"), "platform");
        assert_eq!(method_of("platform", "hybrid,internal"), "platform");
        assert_eq!(method_of("cross-platform", "hybrid,internal"), "hybrid");
        assert_eq!(method_of("cross-platform", "usb,nfc"), "security_key");
        assert_eq!(method_of("", "usb"), "security_key");
        assert_eq!(method_of("", ""), "platform");
    }

    #[test]
    fn the_json_door_round_trips_and_survives_garbage() {
        let first = step_json(
            SAFE,
            &serde_json::json!([{ "publicKeyHex": device()[0].public_key_hex }]).to_string(),
            "[]",
        );
        assert!(first.contains(r#""type":"ask""#) && first.contains("groups@100"));
        let garbage = step_json(SAFE, "not json", "not json");
        assert!(garbage.contains(r#""source":"device""#));
    }
}
