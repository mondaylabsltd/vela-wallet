//! The parallel space's fixed keyset — the one thing in it that is not real.
//!
//! Every Vela client has a test environment that is the real app with a single
//! substitution: where a passkey would sign, THIS fixed set of P-256 keypairs
//! signs instead. Everything downstream is genuine — the derived Safe
//! addresses, the WebAuthn assertions the on-chain verifier accepts, the
//! relay, the chains. The Expo and web clients carry the keyset as
//! `passkey-fixture.ts`; this is the same keyset for the tiers whose only
//! language is Rust. Same scalars, same credential ids, same derived
//! addresses — a wallet the web created in its parallel space is the wallet
//! the desktop signs for in its own.
//!
//! ## Why it lives here and not in a shell
//!
//! Every desktop signing path ends at an authenticator that cannot be handed a
//! private key: a USB key over CTAP2, a phone over caBLE, the platform vault,
//! `webauthn.dll`. A shell that wanted a software signer would have to
//! re-implement the WebAuthn assembly this crate already owns
//! (`webauthn_signing_hash`, the client-data layout, the attestation object
//! `extract_attestation_public_key` walks) — and a second copy of that
//! assembly is a chance for the fixture's bytes to drift from a real
//! authenticator's, which is precisely what a fixture must never do. So the
//! signer sits beside the kernels it uses, and a shell only chooses WHEN to
//! call it.
//!
//! ## Why it is a feature, and off by default
//!
//! ⚠️ These private keys are throwaway TEST keys, committed on purpose. They
//! never guard real funds; their addresses must never receive real money
//! beyond test dust. `dev-fixtures` is DEFAULT OFF so a production binary
//! carries neither the scalars nor any path that could reach a software
//! signer: `vela-core-uniffi` and `vela-core-wasm` do not enable it, and a
//! shell that does must gate the runtime switch separately and show a badge
//! whenever the space is active.
//!
//! ## What is frozen
//!
//! - The three scalars and their credential ids (`vela-fixture-01..03`).
//! - The three single-key Safe addresses and the multi-key Safe address —
//!   golden-locked in the tests below; funded on Gnosis for live sweeps.
//! - The registration's authenticator flags byte, [`REGISTRATION_FLAGS`]
//!   (UP|UV|AT, no BE/BS): the fixtures' registry entries on Gnosis carry
//!   exactly this attestation and entries are immutable. A side effect worth
//!   knowing: no BE/BS means the fixtures read as DEVICE-BOUND, so a
//!   single-key parallel-space creation trips the second-key gate — which
//!   doubles as the live demo of that gate.

use p256::ecdsa::{signature::hazmat::PrehashSigner, Signature, SigningKey};

use crate::error::CoreError;
use crate::primitives;
use crate::safe::{compute_safe_address, compute_safe_address_multi, parse_public_key};
use crate::types::P256PublicKey;
use crate::webauthn::webauthn_signing_hash;

/// The relying party every Vela passkey is bound to, fixture or real.
pub const RP_ID: &str = "getvela.app";
/// The origin the fixture's clientDataJSON claims — the same one every real
/// Vela ceremony claims, so the JSON is indistinguishable in shape.
pub const ORIGIN: &str = "https://getvela.app";
/// What the fixture reports as `authenticatorAttachment`. The web fixture
/// says `platform`; a desktop consumer records the same so a key minted in
/// either space reads alike.
pub const AUTHENTICATOR_ATTACHMENT: &str = "platform";
/// The `getTransports()` list the fixture registration reports.
pub const TRANSPORTS: &str = "internal";
/// The registration's authenticator-data flags: UP (0x01) | UV (0x04) |
/// AT (0x40). FROZEN — see the module note.
pub const REGISTRATION_FLAGS: u8 = 0x45;
/// The assertion's flags: UP | UV. Safe's verifier requires UV.
const ASSERTION_FLAGS: u8 = 0x05;

struct Seed {
    name: &'static str,
    /// A 32-byte scalar below the curve order.
    private_key_hex: &'static str,
}

/// The keyset. Add rows here to grow it — nothing else changes.
const SEED: &[Seed] = &[
    Seed {
        name: "Parallel One",
        private_key_hex: "d80133c59ce0943689a9c1ff6006242c27b19412439fbc88f94feb5ca1e802d5",
    },
    Seed {
        name: "Parallel Two",
        private_key_hex: "6e1ebe95f2f14d70b193aedbfe87c3d495943c19fb04a81c163cf92ae384c59f",
    },
    Seed {
        name: "Parallel Three",
        private_key_hex: "e66f17e63e4b6e1a6c8a31086d86bcb3172816bec70a5221576c1e2a2ae1f336",
    },
];

/// One fixture identity, derived from its seed row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureAccount {
    /// Position in the keyset; the web's `FIXTURE_ACCOUNTS[index]`.
    pub index: usize,
    /// Hex of the ASCII `vela-fixture-0N`, no `0x`, lowercase — the stable
    /// credential id AND the wallet `account.id`, exactly as the web spells it.
    pub credential_id_hex: String,
    pub name: &'static str,
    /// The scalar, hex, no `0x`. Test material; see the module note.
    pub private_key_hex: &'static str,
    /// Uncompressed P-256 point `04 ‖ x ‖ y`, hex, no `0x`, lowercase.
    pub public_key_hex: String,
    /// The single-key counterfactual Safe, EIP-55 checksummed.
    pub address: String,
}

/// A WebAuthn assertion signed by a fixture key. Hex fields carry no `0x`,
/// mirroring `app::Assertion` / the web's `PasskeyAssertionResult`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureAssertion {
    pub credential_id_hex: String,
    /// DER `ECDSA-Sig-Value`, low-S — what an authenticator emits.
    pub signature_der_hex: String,
    /// `rpIdHash(32) ‖ flags(1) ‖ signCount(4)`.
    pub authenticator_data_hex: String,
    /// The raw JSON bytes, byte-exact as signed.
    pub client_data_json_hex: String,
}

/// A WebAuthn registration minted for a fixture key: a `fmt: none` attestation
/// object whose attested credential data carries the account's public key as
/// a COSE key, in the layout [`crate::webauthn::extract_attestation_public_key`]
/// walks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixtureRegistration {
    pub credential_id_hex: String,
    pub attestation_object_hex: String,
    pub client_data_json_hex: String,
}

fn signing_key(private_key_hex: &str) -> Result<SigningKey, CoreError> {
    let scalar = primitives::from_hex(private_key_hex)?;
    SigningKey::from_slice(&scalar).map_err(|error| {
        CoreError::Internal(format!("fixture seed is not a P-256 scalar: {error}"))
    })
}

fn public_key(account: &FixtureAccount) -> Result<P256PublicKey, CoreError> {
    parse_public_key(&account.public_key_hex)
}

fn derive(index: usize, seed: &Seed) -> Result<FixtureAccount, CoreError> {
    let signing = signing_key(seed.private_key_hex)?;
    let public_key_hex = primitives::to_hex(
        signing.verifying_key().to_sec1_point(false).as_bytes(),
        false,
    );
    let key = parse_public_key(&public_key_hex)?;
    let address = compute_safe_address(&key.x, &key.y)?.address;
    Ok(FixtureAccount {
        index,
        credential_id_hex: primitives::to_hex(
            format!("vela-fixture-0{}", index + 1).as_bytes(),
            false,
        ),
        name: seed.name,
        private_key_hex: seed.private_key_hex,
        public_key_hex,
        address,
    })
}

/// How many fixture identities the keyset holds.
pub fn count() -> usize {
    SEED.len()
}

/// Every fixture identity, in keyset order.
pub fn accounts() -> Result<Vec<FixtureAccount>, CoreError> {
    SEED.iter()
        .enumerate()
        .map(|(index, seed)| derive(index, seed))
        .collect()
}

/// One fixture identity by position.
pub fn account(index: usize) -> Result<FixtureAccount, CoreError> {
    let seed = SEED.get(index).ok_or_else(|| {
        CoreError::Internal(format!(
            "fixture index {index} out of range (the keyset holds {})",
            SEED.len()
        ))
    })?;
    derive(index, seed)
}

/// The fixture a credential id names, tolerating a `0x` prefix and any case.
/// `Ok(None)` is a normal answer: the id belongs to a real passkey.
pub fn by_credential_id(id: &str) -> Result<Option<FixtureAccount>, CoreError> {
    let wanted = id.strip_prefix("0x").unwrap_or(id).to_ascii_lowercase();
    for (index, seed) in SEED.iter().enumerate() {
        let candidate =
            primitives::to_hex(format!("vela-fixture-0{}", index + 1).as_bytes(), false);
        if candidate == wanted {
            return derive(index, seed).map(Some);
        }
    }
    Ok(None)
}

/// The multi-key fixture wallet: ALL fixture keys founding one Safe, fixture
/// #1 pinned as `keys[0]`. Its address is what a parallel-space multi-key
/// creation must derive and what a key-#2/#3 login must reconstruct.
pub fn multi_address() -> Result<String, CoreError> {
    let keys = accounts()?
        .iter()
        .map(public_key)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(compute_safe_address_multi(&keys)?.address)
}

/// Which fixture answers a ceremony, given the allow list the machine offered.
///
/// A real provider shows a picker; the fixture decides by rule so a test is
/// deterministic. `preferred` (the web's `vela.parallel.signWith(n)`) wins
/// whenever the list allows it — or whenever the list is EMPTY, which is the
/// discoverable "who are you?" ceremony where any fixture is a valid answer.
/// Otherwise the first allowed fixture answers, and an allow list that names
/// no fixture at all falls back to fixture #1: a machine that asked for a real
/// credential gets the same answer the web's parallel space gives.
pub fn resolve_signer(
    allow_credential_ids: &[String],
    preferred: Option<usize>,
) -> Result<FixtureAccount, CoreError> {
    let allowed: Vec<FixtureAccount> = allow_credential_ids
        .iter()
        .filter_map(|id| by_credential_id(id).transpose())
        .collect::<Result<_, _>>()?;
    if let Some(index) = preferred {
        let wanted = account(index)?;
        if allow_credential_ids.is_empty() || allowed.iter().any(|a| a.index == wanted.index) {
            return Ok(wanted);
        }
    }
    match allowed.into_iter().next() {
        Some(first) => Ok(first),
        None => account(0),
    }
}

/// `rpIdHash(32) ‖ flags(1) ‖ signCount(4)`, sign count zero like the web.
fn authenticator_data(rp_id: &str, flags: u8) -> Vec<u8> {
    let mut out = primitives::sha256(rp_id.as_bytes());
    out.push(flags);
    out.extend_from_slice(&[0, 0, 0, 0]);
    out
}

/// A genuine WebAuthn assertion over `challenge`, signed with `account`'s
/// key: `ECDSA_P256(sha256(authenticatorData ‖ sha256(clientDataJSON)))`,
/// low-S, DER. Byte-for-byte what an authenticator would emit for this key
/// and challenge, so `validate_client_data` accepts it, `der_signature_to_raw_low_s`
/// parses it, and Safe's on-chain verifier validates it against the
/// account's public key.
pub fn build_assertion(
    account: &FixtureAccount,
    challenge: &[u8],
    rp_id: &str,
    origin: &str,
) -> Result<FixtureAssertion, CoreError> {
    let signing = signing_key(account.private_key_hex)?;
    let client_data_json = format!(
        "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",\"origin\":\"{}\",\"crossOrigin\":false}}",
        primitives::to_base64url(challenge),
        origin
    );
    let authenticator_data = authenticator_data(rp_id, ASSERTION_FLAGS);
    let digest = webauthn_signing_hash(&authenticator_data, client_data_json.as_bytes());
    let signature: Signature = signing
        .sign_prehash(&digest)
        .map_err(|error| CoreError::Internal(format!("fixture signing failed: {error}")))?;
    // Low-S is what the verifier wants; RFC 6979 does not promise it.
    let der = signature.normalize_s().to_der();
    Ok(FixtureAssertion {
        credential_id_hex: account.credential_id_hex.clone(),
        signature_der_hex: primitives::to_hex(der.as_bytes(), false),
        authenticator_data_hex: primitives::to_hex(&authenticator_data, false),
        client_data_json_hex: primitives::to_hex(client_data_json.as_bytes(), false),
    })
}

/// CBOR definite-length text string (short form; every key here is < 24 bytes).
fn cbor_text(s: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 1);
    out.push(0x60 | s.len() as u8);
    out.extend_from_slice(s.as_bytes());
    out
}

/// CBOR definite-length byte string, sized for the fixture's authData.
fn cbor_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(bytes.len() + 3);
    match bytes.len() {
        n if n < 24 => out.push(0x40 | n as u8),
        n if n < 256 => out.extend_from_slice(&[0x58, n as u8]),
        n => out.extend_from_slice(&[0x59, (n >> 8) as u8, n as u8]),
    }
    out.extend_from_slice(bytes);
    out
}

/// A registration for `account`: `{"fmt":"none","attStmt":{},"authData":…}`
/// whose authData carries the credential id and the public key as a COSE_Key
/// `{1:2, 3:-7, -1:1, -2:x, -3:y}`. The layout, and the frozen flags byte,
/// match the web fixture exactly.
pub fn build_registration(
    account: &FixtureAccount,
    rp_id: &str,
    origin: &str,
) -> Result<FixtureRegistration, CoreError> {
    let key = public_key(account)?;
    let credential_id = primitives::from_hex(&account.credential_id_hex)?;

    let mut cose = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20];
    cose.extend_from_slice(&key.x);
    cose.extend_from_slice(&[0x22, 0x58, 0x20]);
    cose.extend_from_slice(&key.y);

    let mut auth_data = authenticator_data(rp_id, REGISTRATION_FLAGS);
    auth_data.extend_from_slice(&[0u8; 16]); // AAGUID: none
    auth_data.extend_from_slice(&[(credential_id.len() >> 8) as u8, credential_id.len() as u8]);
    auth_data.extend_from_slice(&credential_id);
    auth_data.extend_from_slice(&cose);

    let mut attestation = vec![0xa3];
    attestation.extend(cbor_text("fmt"));
    attestation.extend(cbor_text("none"));
    attestation.extend(cbor_text("attStmt"));
    attestation.push(0xa0);
    attestation.extend(cbor_text("authData"));
    attestation.extend(cbor_bytes(&auth_data));

    let client_data_json = format!(
        "{{\"type\":\"webauthn.create\",\"challenge\":\"{}\",\"origin\":\"{}\",\"crossOrigin\":false}}",
        primitives::to_base64url(&[0u8; 32]),
        origin
    );
    Ok(FixtureRegistration {
        credential_id_hex: account.credential_id_hex.clone(),
        attestation_object_hex: primitives::to_hex(&attestation, false),
        client_data_json_hex: primitives::to_hex(client_data_json.as_bytes(), false),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ClientDataKind;
    use crate::webauthn::{
        attested_credential_id, der_signature_to_raw_low_s, extract_attestation_public_key,
        validate_client_data,
    };
    use p256::ecdsa::signature::hazmat::PrehashVerifier as _;
    use p256::ecdsa::VerifyingKey;

    fn ok<T>(result: Result<T, CoreError>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Golden lock: these are the addresses funded on-chain. A change to the
    /// keyset or the derivation must be a conscious edit here (and a
    /// re-fund), never silent. Same vector as the web's
    /// `passkey-fixture.test.ts`.
    #[test]
    fn the_three_single_key_addresses_are_frozen() {
        let addresses: Vec<String> = ok(accounts()).into_iter().map(|a| a.address).collect();
        assert_eq!(
            addresses,
            [
                "0xD400866e00B055B20752a826CD5C89b811de130b",
                "0x031d7D57c99CAF891e1C250554691Fd12D84772b",
                "0x58cd0ce6A27099220543b31710d7860d75Ba1d3d",
            ]
        );
    }

    /// The golden multi-key Safe — the one the read-wiring live sweeps read.
    #[test]
    fn the_multi_key_address_is_frozen() {
        assert_eq!(
            ok(multi_address()),
            "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        );
        for account in ok(accounts()) {
            assert_ne!(
                account.address.to_lowercase(),
                ok(multi_address()).to_lowercase()
            );
        }
    }

    #[test]
    fn credential_ids_are_the_ascii_names_in_hex() {
        let ids: Vec<String> = ok(accounts())
            .into_iter()
            .map(|a| a.credential_id_hex)
            .collect();
        assert_eq!(ids[0], primitives::to_hex(b"vela-fixture-01", false));
        assert_eq!(ids[1], primitives::to_hex(b"vela-fixture-02", false));
        assert_eq!(ids[2], primitives::to_hex(b"vela-fixture-03", false));
        assert_eq!(count(), 3);
    }

    #[test]
    fn every_account_is_distinct() {
        let accounts = ok(accounts());
        for (i, a) in accounts.iter().enumerate() {
            assert_eq!(a.index, i);
            assert!(a.public_key_hex.starts_with("04"));
            assert_eq!(a.public_key_hex.len(), 130);
            for b in &accounts[i + 1..] {
                assert_ne!(a.address, b.address);
                assert_ne!(a.public_key_hex, b.public_key_hex);
                assert_ne!(a.credential_id_hex, b.credential_id_hex);
            }
        }
    }

    #[test]
    fn a_credential_id_resolves_with_or_without_prefix_and_case() {
        let first = ok(account(0));
        assert_eq!(
            ok(by_credential_id(&first.credential_id_hex)),
            Some(first.clone())
        );
        let shouted = format!("0x{}", first.credential_id_hex.to_uppercase());
        assert_eq!(ok(by_credential_id(&shouted)), Some(first));
        assert_eq!(ok(by_credential_id("deadbeef")), None);
        assert_eq!(ok(by_credential_id("")), None);
        assert!(account(3).is_err());
    }

    #[test]
    fn the_signer_rule_matches_the_web() {
        let ids: Vec<String> = ok(accounts())
            .into_iter()
            .map(|a| a.credential_id_hex)
            .collect();
        // An allow list: the first fixture it names answers.
        assert_eq!(ok(resolve_signer(&ids[1..], None)).index, 1);
        // Preferred wins when allowed …
        assert_eq!(ok(resolve_signer(&ids, Some(2))).index, 2);
        // … and yields to the list when not.
        assert_eq!(ok(resolve_signer(&ids[..1], Some(2))).index, 0);
        // A list naming only real credentials falls back to fixture #1.
        assert_eq!(ok(resolve_signer(&["cafe".to_owned()], None)).index, 0);
        // The discoverable ceremony: fixture #1, or the preferred one.
        assert_eq!(ok(resolve_signer(&[], None)).index, 0);
        assert_eq!(ok(resolve_signer(&[], Some(1))).index, 1);
    }

    /// Recompute `sha256(authData ‖ sha256(clientDataJSON))` and verify the
    /// signature against the account's own public key — the same math as
    /// Safe's verifier.
    #[test]
    fn an_assertion_signs_the_exact_digest_the_verifier_checks() {
        let challenge = [0xcd_u8; 32];
        for account in ok(accounts()) {
            let assertion = ok(build_assertion(&account, &challenge, RP_ID, ORIGIN));
            assert_eq!(assertion.credential_id_hex, account.credential_id_hex);

            let auth_data = ok(primitives::from_hex(&assertion.authenticator_data_hex));
            let client_data = ok(primitives::from_hex(&assertion.client_data_json_hex));
            ok(validate_client_data(
                ClientDataKind::Get,
                &client_data,
                &auth_data,
            ));

            let der = ok(primitives::from_hex(&assertion.signature_der_hex));
            let raw = ok(der_signature_to_raw_low_s(&der));
            assert_eq!(raw.len(), 64);
            let signature = Signature::from_der(&der).unwrap_or_else(|e| unreachable!("{e}"));
            assert_eq!(
                signature.normalize_s(),
                signature,
                "the fixture emits low-S, as the verifier requires"
            );

            let point = ok(primitives::from_hex(&account.public_key_hex));
            let verifying =
                VerifyingKey::from_sec1_bytes(&point).unwrap_or_else(|e| unreachable!("{e}"));
            let digest = webauthn_signing_hash(&auth_data, &client_data);
            ok(verifying
                .verify_prehash(&digest, &signature)
                .map_err(|e| CoreError::Internal(e.to_string())));

            // The layout a real authenticator produces.
            assert_eq!(auth_data.len(), 37);
            assert_eq!(&auth_data[..32], &primitives::sha256(RP_ID.as_bytes())[..]);
            assert_eq!(auth_data[32], 0x05);
            let json = String::from_utf8(client_data).unwrap_or_else(|e| unreachable!("{e}"));
            assert!(json.starts_with("{\"type\":\"webauthn.get\",\"challenge\":\""));
            assert!(json.contains(&primitives::to_base64url(&challenge)));
            assert!(json.contains("\"origin\":\"https://getvela.app\""));
        }
    }

    #[test]
    fn different_challenges_give_different_signatures_and_the_same_key() {
        let account = ok(account(1));
        let a = ok(build_assertion(&account, &[1_u8; 32], RP_ID, ORIGIN));
        let b = ok(build_assertion(&account, &[2_u8; 32], RP_ID, ORIGIN));
        assert_ne!(a.signature_der_hex, b.signature_der_hex);
        assert_eq!(a.authenticator_data_hex, b.authenticator_data_hex);
    }

    #[test]
    fn a_registration_round_trips_through_the_parser() {
        for account in ok(accounts()) {
            let registration = ok(build_registration(&account, RP_ID, ORIGIN));
            let attestation = ok(primitives::from_hex(&registration.attestation_object_hex));
            let parsed = ok(extract_attestation_public_key(&attestation));
            let expected = ok(parse_public_key(&account.public_key_hex));
            assert_eq!(parsed, expected);

            // authData sits after the 3-key map header; the parser found the
            // key, now check the id and the frozen flags the same way a
            // shell would.
            let auth_data_start = attestation.len() - (32 + 1 + 4 + 16 + 2 + 15 + 77);
            let auth_data = &attestation[auth_data_start..];
            assert_eq!(
                ok(attested_credential_id(auth_data)),
                ok(primitives::from_hex(&account.credential_id_hex))
            );
            assert_eq!(
                auth_data[32], REGISTRATION_FLAGS,
                "the attestation flags are frozen"
            );

            let client_data = ok(primitives::from_hex(&registration.client_data_json_hex));
            ok(validate_client_data(
                ClientDataKind::Create,
                &client_data,
                &[],
            ));
        }
    }
}
