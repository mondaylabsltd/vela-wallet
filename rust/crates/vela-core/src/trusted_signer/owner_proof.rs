//! A multisig member's owner proof on the trusted signing page
//! (`docs/multisig-machines.md` §16).
//!
//! When a multisig is created — or its record shared again — one member's
//! passkey signs the genesis service's sponsor challenge for the multisig's
//! address ([`crate::multisig::sponsor_challenge`]): the proof the service
//! asks before it records the multisig, its members and its rule, where every
//! member finds it. It moves nothing and approves nothing; it is still a
//! signature, and a member who reviews and signs on a page — or a surface
//! that has no passkeys of its own (the Safari extension, R8) — makes it
//! there, Vela-account grade: the page is handed the multisig's GENESIS, not
//! a challenge, and reads everything it shows from it.
//!
//! - [`params`] — the request's one parameter, as the core builds it for every
//!   shell (the passkey purpose's `params`): the multisig, the member, the
//!   genesis, the networks it is created for.
//! - [`read`] — what the page shows, read from those params alone: the
//!   multisig's address DERIVED from the genesis (CREATE2 on Safe's factory),
//!   its members and threshold read from the setup — which must be exactly a
//!   multisig as Vela creates one ([`crate::multisig::vela_multisig_of`]) —
//!   the member's place among them, and the challenge, derived from the
//!   address. The address the app names is only ever compared with the
//!   derived one. The page (`lib/multisig.js` `ownerProof`) is transcribed
//!   from this and cross-checked against the wasm and
//!   `tests/vectors/multisig/owner-proof.json`, case by case.
//! - The answer is accepted as every page signature is
//!   ([`super::accept_signature`]): [`super::digest_of`] reads the request it
//!   built by [`read`] and holds the answer to that challenge — never to one a
//!   shell or the page supplied.
//!
//! The networks and the name are the app's word (the challenge covers the
//! address alone, and the address the members and the rule): the page draws
//! them as such.
//!
//! Pure: no I/O, no clock.

use serde::Serialize;
use serde_json::{json, Value};

use crate::multisig::{self, Genesis};

/// The page's method for an owner proof.
pub const METHOD: &str = crate::signing_venue::MULTISIG_CREATE_METHOD;

/// The most networks one proof names (no shell creates a multisig on more).
pub const MAX_CHAINS: usize = 32;

/// The longest setup the page reads, in bytes: twenty members' setup is
/// under a kilobyte.
pub const MAX_INITIALIZER_BYTES: usize = 2048;

/// The largest number JavaScript holds exactly (`Number.MAX_SAFE_INTEGER`):
/// a chain id past it is one the page cannot read as the core does.
const MAX_SAFE_INTEGER: u64 = (1 << 53) - 1;

/// The request's params for `member`'s proof for the multisig at `multisig`,
/// created from `genesis` for `chain_ids`.
#[must_use]
pub fn params(multisig: &str, member: &str, genesis: &Genesis, chain_ids: &[u64]) -> Value {
    json!([{
        "multisig": multisig,
        "member": member,
        "genesis": genesis,
        "chainIds": chain_ids,
    }])
}

/// What the page shows for an owner proof, read from its params.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// The multisig's address, derived from the genesis (EIP-55).
    pub multisig: String,
    /// The member who proves (EIP-55).
    pub member: String,
    /// Its members, in the setup's order (EIP-55).
    pub owners: Vec<String>,
    /// The approvals it needs.
    pub threshold: u64,
    /// The member's place among them, from one.
    pub place: usize,
    /// The record's name — the app's word, and only a name a multisig can
    /// have ([`multisig::valid_name`]).
    pub name: Option<String>,
    /// The networks it is created for — the app's word.
    pub chain_ids: Vec<u64>,
    /// What the member's passkey signs: the sponsor challenge for the
    /// DERIVED address (`0x` hex).
    pub challenge: String,
}

/// Why the page refuses an owner proof. Its JSON (and [`Self::key`]) is the
/// page's own refusal key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Refusal {
    /// Not the shape this page reads exactly.
    #[serde(rename = "refuse.msOwnerShape")]
    Shape,
    /// Not a multisig as Vela creates one: another factory or singleton, a
    /// setup hook, a module, a payment, members it cannot have.
    #[serde(rename = "refuse.msOwnerGenesis")]
    NotVela,
    /// The app names one address; the genesis makes another.
    #[serde(rename = "refuse.msOwnerAddress")]
    AddressMismatch,
    /// The member is not one of the multisig's members.
    #[serde(rename = "refuse.msOwnerNotMember")]
    NotMember,
}

impl Refusal {
    /// The page's refusal key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::Shape => "refuse.msOwnerShape",
            Self::NotVela => "refuse.msOwnerGenesis",
            Self::AddressMismatch => "refuse.msOwnerAddress",
            Self::NotMember => "refuse.msOwnerNotMember",
        }
    }
}

/// `0x` and forty hex digits — the page's `ADDRESS`.
fn address(value: Option<&Value>) -> Option<&str> {
    let text = value?.as_str()?;
    let body = text.strip_prefix("0x")?;
    (body.len() == 40 && body.bytes().all(|b| b.is_ascii_hexdigit())).then_some(text)
}

/// `0x` and whole bytes — the page's `HEX_BYTES` — within `max` bytes.
fn hex_bytes(value: Option<&Value>, max: usize) -> Option<&str> {
    let text = value?.as_str()?;
    let body = text.strip_prefix("0x")?;
    (body.len() % 2 == 0 && body.len() / 2 <= max && body.bytes().all(|b| b.is_ascii_hexdigit()))
        .then_some(text)
}

/// A JSON number that is a whole chain id JavaScript holds exactly (`1.0`
/// reads as 1 there, so here too).
fn chain_id(value: &Value) -> Option<u64> {
    let n = value.as_number()?;
    let id = match n.as_u64() {
        Some(id) => id,
        None => {
            let f = n.as_f64()?;
            if f.fract() != 0.0 || f < 1.0 || f > MAX_SAFE_INTEGER as f64 {
                return None;
            }
            // Whole and within 2^53: exact.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                f as u64
            }
        }
    };
    (1..=MAX_SAFE_INTEGER).contains(&id).then_some(id)
}

/// A salt nonce: `0x` hex (`0x` alone is zero) or decimal digits, within
/// 256 bits — the core's quantity, without the spaces it forgives (JavaScript
/// and Rust do not agree on what a space is).
fn quantity(value: Option<&Value>) -> Option<&str> {
    let text = value?.as_str()?;
    let clean = text;
    let ok = match clean.strip_prefix("0x") {
        Some(hex) => {
            hex.bytes().all(|b| b.is_ascii_hexdigit())
                && (hex.is_empty() || alloy_primitives::U256::from_str_radix(hex, 16).is_ok())
        }
        None => {
            !clean.is_empty()
                && clean.bytes().all(|b| b.is_ascii_digit())
                && alloy_primitives::U256::from_str_radix(clean, 10).is_ok()
        }
    };
    ok.then_some(text)
}

/// The genesis in `p`, if every field is one the page reads exactly.
fn genesis_of(p: &serde_json::Map<String, Value>) -> Option<Genesis> {
    let g = p.get("genesis")?.as_object()?;
    let name = match g.get("name") {
        None | Some(Value::Null) => None,
        Some(Value::String(name)) => Some(name.clone()),
        Some(_) => return None,
    };
    Some(Genesis {
        factory: address(g.get("factory"))?.to_owned(),
        singleton: address(g.get("singleton"))?.to_owned(),
        salt_nonce: quantity(g.get("saltNonce"))?.to_owned(),
        initializer: hex_bytes(g.get("initializer"), MAX_INITIALIZER_BYTES)?.to_owned(),
        name,
    })
}

/// The networks in `p`: one to [`MAX_CHAINS`], distinct, in the order given.
fn chains_of(p: &serde_json::Map<String, Value>) -> Option<Vec<u64>> {
    let list = p.get("chainIds")?.as_array()?;
    if list.is_empty() || list.len() > MAX_CHAINS {
        return None;
    }
    let ids = list.iter().map(chain_id).collect::<Option<Vec<_>>>()?;
    let distinct = ids.iter().enumerate().all(|(i, id)| !ids[..i].contains(id));
    distinct.then_some(ids)
}

/// What the page shows for an owner proof whose request params are `params`
/// — or why it refuses. Checked in this order, as the page checks it: the
/// shape; a multisig as Vela creates one; the address the app names is the
/// one the genesis makes; the member is one of its members.
///
/// # Errors
///
/// [`Refusal`]: the page's reason, by its key.
pub fn read(params: &Value) -> Result<Reading, Refusal> {
    let p = match params.as_array().map(Vec::as_slice) {
        Some([only]) => only.as_object().ok_or(Refusal::Shape)?,
        _ => return Err(Refusal::Shape),
    };
    let named = address(p.get("multisig")).ok_or(Refusal::Shape)?;
    let member = address(p.get("member")).ok_or(Refusal::Shape)?;
    let chain_ids = chains_of(p).ok_or(Refusal::Shape)?;
    let genesis = genesis_of(p).ok_or(Refusal::Shape)?;
    let made = multisig::vela_multisig_of(&genesis).map_err(|_| Refusal::NotVela)?;
    if !made.address.eq_ignore_ascii_case(named) {
        return Err(Refusal::AddressMismatch);
    }
    let place = made
        .owners
        .iter()
        .position(|owner| owner.eq_ignore_ascii_case(member))
        .ok_or(Refusal::NotMember)?;
    let challenge = multisig::sponsor_challenge(&made.address).map_err(|_| Refusal::Shape)?;
    Ok(Reading {
        member: made.owners[place].clone(),
        multisig: made.address,
        owners: made.owners,
        threshold: made.threshold,
        place: place + 1,
        name: genesis.name.filter(|name| multisig::valid_name(name)),
        chain_ids,
        challenge,
    })
}

/// [`read`] as JSON — `{reading}` or `{refusal}` (the page's key): the
/// vectors' and the wasm's shape (`multisigCall('ownerProofRead')`).
#[must_use]
pub fn read_json(params: &Value) -> Value {
    match read(params) {
        Ok(reading) => json!({ "reading": reading }),
        Err(refusal) => json!({ "refusal": refusal.key() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multisig::MultisigGenesis;

    const A: &str = "0x8F1e0BC5D9aA3f6e2b7c4D1A9E8F3b2C5a6D7E01";
    const B: &str = "0x2B5AD5c4795c026514f8317c7a215E218DcCD6cF";
    const C: &str = "0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69";

    fn made() -> MultisigGenesis {
        multisig::multisig_genesis(&[A.into(), B.into(), C.into()], 2, "Team fund", "0x75bcd15")
            .unwrap_or_else(|e| unreachable!("{e}"))
    }

    fn proof(ms: &MultisigGenesis, member: &str) -> Value {
        params(&ms.address, member, &ms.genesis, &[5_042_002, 8453])
    }

    #[test]
    fn a_multisig_vela_creates_reads_whole() {
        let ms = made();
        let reading =
            read(&proof(&ms, &B.to_ascii_lowercase())).unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(reading.multisig, ms.address);
        assert_eq!(reading.member, B);
        assert_eq!(reading.owners, vec![A, B, C]);
        assert_eq!(reading.threshold, 2);
        assert_eq!(reading.place, 2);
        assert_eq!(reading.name.as_deref(), Some("Team fund"));
        assert_eq!(reading.chain_ids, vec![5_042_002, 8453]);
        assert_eq!(
            reading.challenge,
            multisig::sponsor_challenge(&ms.address).unwrap_or_default()
        );
    }

    #[test]
    fn what_the_page_cannot_vouch_for_is_refused() {
        let ms = made();
        // Another address than the genesis makes.
        let mut other = proof(&ms, A);
        other[0]["multisig"] = json!(C);
        assert_eq!(read(&other), Err(Refusal::AddressMismatch));
        // A member it does not have.
        assert_eq!(
            read(&proof(&ms, "0x0000000000000000000000000000000000000abc")),
            Err(Refusal::NotMember)
        );
        // Another singleton (the address follows it, so the genesis is not Vela's).
        let mut foreign = ms.clone();
        foreign.genesis.singleton = "0x41675C099F32341bf84BFc5382aF534df5C7461a".into();
        assert_eq!(read(&proof(&foreign, A)), Err(Refusal::NotVela));
        // A setup with a byte changed past the members (the fallback handler).
        let mut hooked = ms.clone();
        let init = hooked.genesis.initializer.clone();
        hooked.genesis.initializer = init.replacen("fd0732dc", "fd0732dd", 1);
        assert_ne!(hooked.genesis.initializer, init);
        assert_eq!(read(&proof(&hooked, A)), Err(Refusal::NotVela));
        // Shapes.
        for bad in [
            json!([]),
            json!([{}]),
            json!({"multisig": ms.address}),
            {
                let mut v = proof(&ms, A);
                v[0]["chainIds"] = json!([]);
                v
            },
            {
                let mut v = proof(&ms, A);
                v[0]["chainIds"] = json!([1, 1]);
                v
            },
            {
                let mut v = proof(&ms, A);
                v[0]["chainIds"] = json!([1.5]);
                v
            },
            {
                let mut v = proof(&ms, A);
                v[0]["genesis"]["initializer"] = json!("abc");
                v
            },
            {
                let mut v = proof(&ms, A);
                v[0]["genesis"]["name"] = json!(7);
                v
            },
        ] {
            assert_eq!(read(&bad), Err(Refusal::Shape), "{bad}");
        }
        // A whole number written as a float is the number (as in JavaScript).
        let mut floats = proof(&ms, A);
        floats[0]["chainIds"] = json!([8453.0]);
        assert_eq!(read(&floats).map(|r| r.chain_ids), Ok(vec![8453]));
    }

    /// The shared vectors (`tests/vectors/multisig/owner-proof.json`): the
    /// page's `ownerProof` answers each case the same (samples/multisig-test.mjs).
    #[test]
    fn the_vectors_read_as_the_page_reads_them() {
        let file: Value = serde_json::from_str(include_str!(
            "../../tests/vectors/multisig/owner-proof.json"
        ))
        .unwrap_or_else(|e| unreachable!("{e}"));
        let cases = file["cases"].as_array().cloned().unwrap_or_default();
        assert!(cases.len() >= 30, "{} cases", cases.len());
        let mut refusals = std::collections::BTreeSet::new();
        for case in &cases {
            let got = read_json(&case["params"]);
            assert_eq!(got, case["expect"], "{}", case["name"]);
            if let Some(key) = got["refusal"].as_str() {
                refusals.insert(key.to_owned());
            }
        }
        // Every refusal is exercised.
        assert_eq!(refusals.len(), 4, "{refusals:?}");
    }

    /// What the page signs is the core's to check: an answer over the
    /// challenge the request's GENESIS makes, by the member's key.
    #[test]
    fn an_answer_is_held_to_the_challenge_the_genesis_makes() {
        use base64::engine::general_purpose::URL_SAFE_NO_PAD;
        use base64::Engine as _;
        use p256::ecdsa::signature::hazmat::PrehashSigner as _;
        use p256::ecdsa::{Signature, SigningKey};
        use sha2::{Digest as _, Sha256};

        let signing = SigningKey::from_slice(&[7; 32]).unwrap_or_else(|_| unreachable!());
        let point = signing.verifying_key().to_sec1_point(false);
        let key = crate::user_op::WalletKey {
            credential_id: "0707c0ffee".into(),
            public_key_hex: crate::primitives::to_hex(point.as_bytes(), false),
        };
        let (member, _) = multisig::vela_account_genesis(std::slice::from_ref(&key))
            .unwrap_or_else(|_| unreachable!());
        let ms = multisig::multisig_genesis(&[A.into(), member.clone()], 1, "Pair", "0x9")
            .unwrap_or_else(|e| unreachable!("{e}"));
        let request = |account: &str, params: Value| {
            crate::trusted_signer::request(&crate::trusted_signer::RequestInput {
                method: METHOD,
                params,
                chain_id: 8453,
                account,
                credential_ids_hex: std::slice::from_ref(&key.credential_id),
                ..Default::default()
            })
        };
        let answer = |challenge: &[u8]| {
            let mut auth = Sha256::digest(b"getvela.app").to_vec();
            auth.push(0x05);
            auth.extend_from_slice(&[0, 0, 0, 1]);
            let client = format!(
                "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",\"origin\":\"https://sign.getvela.app\",\"crossOrigin\":false}}",
                URL_SAFE_NO_PAD.encode(challenge)
            );
            let prehash = crate::webauthn::webauthn_signing_hash(&auth, client.as_bytes());
            let signature: Signature = signing
                .sign_prehash(&prehash)
                .unwrap_or_else(|_| unreachable!());
            json!({
                "credentialId": URL_SAFE_NO_PAD.encode([7, 7, 0xc0, 0xff, 0xee]),
                "signature": crate::primitives::to_hex(&signature.to_bytes(), false),
                "authenticatorData": crate::primitives::to_hex(&auth, false),
                "clientDataJSON": crate::primitives::to_hex(client.as_bytes(), false),
            })
        };
        let keys = std::slice::from_ref(&key);
        let challenge = crate::primitives::from_hex(
            &multisig::sponsor_challenge(&ms.address).unwrap_or_default(),
        )
        .unwrap_or_default();
        let good = request(&member, params(&ms.address, &member, &ms.genesis, &[8453]));
        assert_eq!(
            crate::trusted_signer::digest_of(&good).as_deref(),
            Ok(challenge.as_slice())
        );
        assert!(crate::trusted_signer::accept_signature(
            &good,
            &answer(&challenge),
            keys,
            &challenge
        )
        .is_ok());
        // The shell about to sign another challenge: refused before the answer.
        let other =
            crate::primitives::from_hex(&multisig::sponsor_challenge(A).unwrap_or_default())
                .unwrap_or_default();
        assert_eq!(
            crate::trusted_signer::accept_signature(&good, &answer(&other), keys, &other),
            Err(crate::trusted_signer::TrustedSignerError::WrongChallenge)
        );
        // An answer over another challenge than the genesis makes.
        assert_eq!(
            crate::trusted_signer::accept_signature(&good, &answer(&other), keys, &challenge),
            Err(crate::trusted_signer::TrustedSignerError::WrongChallenge)
        );
        // A request that names another address than its genesis makes, or
        // another account than the member, has no bytes to hold an answer to.
        let mut lying = params(&ms.address, &member, &ms.genesis, &[8453]);
        lying[0]["multisig"] = json!(A);
        assert!(matches!(
            crate::trusted_signer::digest_of(&request(&member, lying)),
            Err(crate::trusted_signer::TrustedSignerError::Malformed(_))
        ));
        assert!(matches!(
            crate::trusted_signer::digest_of(&request(
                A,
                params(&ms.address, &member, &ms.genesis, &[8453])
            )),
            Err(crate::trusted_signer::TrustedSignerError::Malformed(_))
        ));
    }

    #[test]
    fn a_name_a_multisig_cannot_have_is_not_said() {
        let mut ms = made();
        ms.genesis.name = Some("Team\u{202E}dnuf".into());
        assert_eq!(read(&proof(&ms, A)).map(|r| r.name), Ok(None));
    }
}
