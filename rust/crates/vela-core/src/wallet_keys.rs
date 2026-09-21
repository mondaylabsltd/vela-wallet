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
    /// Only [`sign_route`] reads it; the keys view matches by public key.
    #[serde(default, alias = "credentialId")]
    pub credential_id: String,
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
    /// Backed up to a sync fabric. `None` when nobody can vouch for an answer —
    /// only the device answered, or its attestation blob is unreadable — and a
    /// badge nobody can vouch for is not drawn (issue #207).
    pub synced: Option<bool>,
    pub aaguid: String,
    /// "Apple Passwords", "1Password" … empty when the catalog cannot name it.
    pub provider_name: String,
    /// `platform` | `hybrid` | `security_key`.
    pub method: String,
    /// Uncompressed `04‖x‖y`, lowercase bare hex.
    pub public_key_hex: String,
    /// The WebAuthn credential id, base64url as authenticators and the registry
    /// explorer print it. Empty when only the device answered.
    pub credential_id: String,
    /// The registry's 20-byte attestation summary, `0x`-hex. Empty when only
    /// the device answered.
    pub attestation_hex: String,
    /// The authenticator verified the person at registration (UV). `None` when
    /// nobody can vouch for it.
    pub user_verified: Option<bool>,
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

/// What kind of authenticator the hints describe.
///
/// The rules moved to [`passkey::reported_method_name`] (issue #207) so that the
/// row's icon, its caption and this field cannot disagree about the same key.
/// A report of nothing still reads as `platform` here: this view's rows always
/// carry a method, and "a passkey on this device" is what every client drew
/// before any of these fields existed.
fn method_of(attachment: &str, transports: &str) -> &'static str {
    passkey::reported_method_name(attachment, transports).unwrap_or("platform")
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
            credential_id: String::new(),
            attestation_hex: String::new(),
            user_verified: None,
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
        let attestation = item["attestation"].as_str().unwrap_or_default();
        let (aaguid, _) = passkey::attestation_signals(attestation);
        // "Backed up?" as a question that CAN go unanswered (issue #207). The
        // badge this feeds is a display, not a gate: `attestation_signals`
        // answers the same question with `true` for a blob it cannot read,
        // because the second-key gate must fail open rather than dead-end an
        // honest provider — but a row that cannot tell "synced" from "nobody
        // said" would show a green tick nobody verified. `user_verified` below
        // has always asked this way; `synced` now matches it, and the shells
        // already draw nothing for `None`.
        let synced = passkey::attestation_backed_up(attestation);
        let credential_id = item["credentialId"]
            .as_str()
            .and_then(|hex| primitives::from_hex(hex).ok())
            .map(|bytes| primitives::to_base64url(&bytes))
            .unwrap_or_default();
        let remembered = device
            .iter()
            .find(|key| normal_key(&key.public_key_hex).as_deref() == Some(&public_key_hex));
        // The person's OWN label first (founder, 2026-09-17: a name somebody
        // chose outranks every name somebody else recorded). The registry's
        // metadata is what the key was called on the day it was registered —
        // often a default — and only speaks when the device has nothing.
        let name = remembered
            .map(|key| key.name.clone())
            .filter(|name| !name.is_empty())
            .or_else(|| names.get(index).filter(|name| !name.is_empty()).cloned())
            .unwrap_or_default();
        rows.push(WalletKeyRow {
            name,
            authenticator_attachment: attachment.to_owned(),
            transports: transports.to_owned(),
            confirmed: true,
            synced,
            provider_name: passkey::provider_name(&aaguid)
                .unwrap_or_default()
                .to_owned(),
            aaguid,
            method: method_of(attachment, transports).to_owned(),
            public_key_hex,
            credential_id,
            attestation_hex: if attestation.is_empty() {
                String::new()
            } else {
                format!("0x{}", attestation.trim_start_matches("0x"))
            },
            user_verified: passkey::attestation_user_verified(attestation),
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

// ---------------------------------------------------------------------------
// "Sign with" — which key a ceremony is pinned to, and how it is reached
// ---------------------------------------------------------------------------

/// Every "Sign with" value, in the order every shell lists them: `auto` (do
/// what the wallet always did), the three places a passkey can be, and the
/// Clear Signer (spec 071) — a separate page that checks the request and runs
/// the ceremony itself, which [`sign_route`] does not route.
pub const SIGN_METHODS: [&str; 5] = ["auto", "platform", "hybrid", "security_key", "clear_signer"];

/// Where one signing ceremony goes: the credential it is pinned to, the
/// transports the request carries, and the method the shell routes by.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignRoute {
    pub credential_id: String,
    pub transports: String,
    /// `platform` | `hybrid` | `security_key`.
    pub method: String,
}

/// The person chose HOW to sign this one request (founder, 2026-09-19: creating
/// and signing in let a person say where their passkey is; signing silently
/// took the first key's stored route). `None` for `auto` — and for anything
/// this build does not know — which means "do what you always did".
///
/// A native ceremony is PINNED to one credential, so the choice also picks the
/// key: the first founding key whose own stored transports describe that
/// method. A wallet whose first key is an Apple passkey and whose third is a
/// YubiKey, asked for "security key", must pin the YubiKey — pinning the first
/// would ask a security key for a credential it does not hold. When no key
/// says it is of that kind (a synced passkey approved from a phone is
/// registered as `internal`), the first key is pinned and the method's own
/// transports make it reachable.
#[must_use]
pub fn sign_route(device: &[DeviceKey], method: &str) -> Option<SignRoute> {
    let transports = match method {
        "platform" => "internal",
        "hybrid" => "hybrid,internal",
        "security_key" => "usb,nfc,ble",
        _ => return None,
    };
    let usable = |key: &&DeviceKey| !key.credential_id.is_empty();
    let pinned = device
        .iter()
        .filter(usable)
        .find(|key| method_of("", &key.transports) == method)
        .or_else(|| device.iter().find(usable))?;
    Some(SignRoute {
        credential_id: pinned.credential_id.clone(),
        transports: transports.to_owned(),
        method: method.to_owned(),
    })
}

/// The JSON door: `null` for `auto`, an unknown method, or a wallet with no
/// usable credential.
#[must_use]
pub fn sign_route_json(device_keys_json: &str, method: &str) -> Option<String> {
    let device: Vec<DeviceKey> = serde_json::from_str(device_keys_json).ok()?;
    serde_json::to_string(&sign_route(&device, method)?).ok()
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
            credential_id: "golden-key-0".to_owned(),
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
        // The person's own label outranks the registry's; where the device has
        // none, the registry's metadata speaks.
        assert_eq!(keys[0].name, "Parallel Multi");
        assert!(keys.iter().all(|key| !key.name.is_empty()), "{keys:?}");
        // The explorer's facts, for the details a row opens onto.
        assert!(keys.iter().all(|key| !key.credential_id.is_empty()));
        assert!(keys.iter().all(|key| key.attestation_hex.len() == 42));
        assert!(keys.iter().all(|key| key.user_verified.is_some()));
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

    fn held(credential_id: &str, transports: &str) -> DeviceKey {
        DeviceKey {
            credential_id: credential_id.to_owned(),
            transports: transports.to_owned(),
            ..DeviceKey::default()
        }
    }

    #[test]
    fn sign_with_pins_the_key_of_the_chosen_kind() {
        let keys = [
            held("apple", "hybrid,internal"),
            held("chrome", "internal"),
            held("yubikey", "nfc,usb"),
        ];
        // The YubiKey, not the first key: a security key cannot answer for a
        // credential it does not hold.
        let route = sign_route(&keys, "security_key").unwrap_or_else(|| unreachable!());
        assert_eq!(
            (route.credential_id.as_str(), route.transports.as_str()),
            ("yubikey", "usb,nfc,ble")
        );
        let route = sign_route(&keys, "platform").unwrap_or_else(|| unreachable!());
        assert_eq!(route.credential_id, "apple");
        // Nobody registered as hybrid-only: the first key, made reachable over a QR code.
        let route = sign_route(&keys, "hybrid").unwrap_or_else(|| unreachable!());
        assert_eq!(
            (route.credential_id.as_str(), route.transports.as_str()),
            ("apple", "hybrid,internal")
        );
    }

    #[test]
    fn auto_and_the_unknown_change_nothing() {
        let keys = [held("apple", "internal")];
        assert_eq!(sign_route(&keys, "auto"), None);
        assert_eq!(sign_route(&keys, "telepathy"), None);
        // No credential to pin: nothing to route.
        assert_eq!(sign_route(&[held("", "internal")], "platform"), None);
        assert_eq!(sign_route_json("not json", "platform"), None);
        assert!(sign_route_json(
            r#"[{"credentialId":"aa","transports":"usb"}]"#,
            "security_key"
        )
        .is_some_and(|json| json.contains(r#""credential_id":"aa""#)));
    }

    /// A blob this build cannot read is not a "Cloud-synced" badge (issue #207).
    ///
    /// The GATE — "does this wallet need a second key?" — reads
    /// `attestation_signals`, which says `true` there so an honest provider
    /// that omits attested-credential data is never dead-ended. The VIEW asks
    /// the question that can go unanswered, so the row draws no badge at all
    /// rather than a green tick nobody verified.
    #[test]
    fn a_registry_attestation_nobody_can_read_badges_nothing() {
        let public_key = recorded()["publicKey"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let attested = |flags: &str| format!("0x01{}{flags}0000", "00".repeat(16));
        let synced_of = |attestation: &str| {
            let body = serde_json::json!({
                "members": { "items": [{
                    "publicKey": public_key,
                    "authenticatorAttachment": "platform",
                    "transports": "internal",
                    "attestation": attestation,
                    "credentialId": "0xaabb",
                }]}
            })
            .to_string();
            registry_rows(&body, &[]).unwrap_or_else(|| unreachable!())[0].synced
        };
        // BS (bit 4) set, then clear: a readable blob still answers.
        assert_eq!(synced_of(&attested("10")), Some(true));
        assert_eq!(synced_of(&attested("00")), Some(false));
        // Absent, and the wrong length: nobody said.
        assert_eq!(synced_of(""), None);
        assert_eq!(synced_of("0xdead"), None);
        // …while the gate's own reading of those two keeps failing open.
        assert!(passkey::attestation_signals("").1);
        assert!(passkey::attestation_signals("0xdead").1);
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
