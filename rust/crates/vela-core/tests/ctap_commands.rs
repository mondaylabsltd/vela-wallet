//! CTAP2 request encoding and response decoding.
//!
//! The tests that matter here are of two kinds: what an authenticator will
//! REFUSE (a non-canonical encoding, a missing exclusion list), and what the
//! rest of the wallet will MISREAD (an attestation object whose shape the
//! existing parsers do not recognise).

use ciborium::Value;
use serde_json::Value as Json;

use vela_core::ctap::commands::{
    attestation_object, get_info_request, parse_get_assertion, parse_get_info,
    parse_make_credential, split_response, ClientPin, ClientPinSubcommand, Command,
    CredentialDescriptor, GetAssertion, MakeCredential, Permissions, PinUvAuth, Status,
};
use vela_core::ctap::Protocol;
use vela_core::types::P256PublicKey;
use vela_core::webauthn;

const VECTORS: &str = include_str!("vectors/webauthn.json");

/// The real CTAP2 attestation object the crux suites register with.
fn real_attestation_object() -> Vec<u8> {
    let root: Json = match serde_json::from_str(VECTORS) {
        Ok(root) => root,
        Err(error) => unreachable!("webauthn vectors parse: {error}"),
    };
    let hex = root["cases"]
        .as_array()
        .and_then(|cases| {
            cases
                .iter()
                .find(|case| case["name"] == "extractPublicKey/real-key")
        })
        .and_then(|case| case["input"]["attestation_object"].as_str())
        .unwrap_or_else(|| unreachable!("attestation vector missing"));
    match vela_core::primitives::from_hex(hex.trim_start_matches("0x")) {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("attestation vector is not hex: {error}"),
    }
}

fn decode(bytes: &[u8]) -> Value {
    match ciborium::de::from_reader(bytes) {
        Ok(value) => value,
        Err(error) => unreachable!("our own encoding does not parse: {error}"),
    }
}

fn make_credential() -> MakeCredential {
    MakeCredential {
        client_data_hash: vec![0xaa; 32],
        rp_id: "getvela.app".to_owned(),
        rp_name: "Vela Wallet".to_owned(),
        user_id: b"Everyday wallet\0uuid".to_vec(),
        user_name: "Everyday wallet".to_owned(),
        user_display_name: "Everyday wallet".to_owned(),
        exclude: Vec::new(),
        resident_key: true,
        user_verification: true,
        pin_uv_auth: None,
    }
}

// ---------------------------------------------------------------------------
// Requests
// ---------------------------------------------------------------------------

#[test]
fn a_request_is_its_command_byte_then_cbor() {
    let bytes = match make_credential().encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(bytes[0], Command::MakeCredential as u8);
    assert!(matches!(decode(&bytes[1..]), Value::Map(_)));
}

/// CTAP2 requires the canonical encoding form, and an authenticator may refuse
/// anything else. It matters twice over: `pinUvAuthParam` is an HMAC over these
/// exact bytes, so two encodings of "the same" map authenticate differently.
#[test]
fn request_map_keys_are_in_canonical_order() {
    let mut request = make_credential();
    request.exclude = vec![CredentialDescriptor { id: vec![1, 2, 3] }];
    request.pin_uv_auth = Some(PinUvAuth {
        protocol: 2,
        param: vec![0x99; 32],
    });
    let bytes = match request.encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!("request is not a map");
    };
    let keys: Vec<i128> = entries
        .iter()
        .filter_map(|(key, _)| match key {
            Value::Integer(i) => Some(i128::from(*i)),
            _ => None,
        })
        .collect();
    let mut sorted = keys.clone();
    sorted.sort_unstable();
    assert_eq!(keys, sorted, "CTAP2 canonical CBOR sorts map keys");
    assert_eq!(keys, vec![1, 2, 3, 4, 5, 7, 8, 9]);
}

/// ES256 only, and nothing else offered.
///
/// The on-chain verifier is the RIP-7212 P-256 precompile and two-signature
/// recovery is ECDSA math, so an RSA credential can never become a working
/// wallet. Offering RS256 would mint an orphan key in someone's authenticator
/// and fail later, somewhere far less legible than here.
#[test]
fn only_es256_is_offered() {
    let bytes = match make_credential().encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!()
    };
    let params = entries
        .iter()
        .find_map(|(key, value)| match key {
            Value::Integer(i) if i128::from(*i) == 4 => Some(value),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("pubKeyCredParams missing"));
    let Value::Array(items) = params else {
        unreachable!("pubKeyCredParams is not an array")
    };
    assert_eq!(items.len(), 1, "exactly one algorithm is offered");
    let Value::Map(fields) = &items[0] else {
        unreachable!()
    };
    let alg = fields
        .iter()
        .find_map(|(key, value)| match (key, value) {
            (Value::Text(name), Value::Integer(i)) if name == "alg" => Some(i128::from(*i)),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("alg missing"));
    assert_eq!(alg, -7, "ES256");
}

/// The founding-set guard on the wire: without it a provider may silently
/// REPLACE a key the wallet's address depends on, and the address becomes one
/// nothing can deploy.
#[test]
fn exclude_list_carries_every_founding_credential() {
    let mut request = make_credential();
    request.exclude = vec![
        CredentialDescriptor { id: vec![0xaa; 16] },
        CredentialDescriptor { id: vec![0xbb; 16] },
    ];
    let bytes = match request.encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!()
    };
    let Some(Value::Array(items)) = entries.iter().find_map(|(key, value)| match key {
        Value::Integer(i) if i128::from(*i) == 5 => Some(value),
        _ => None,
    }) else {
        unreachable!("excludeList missing");
    };
    assert_eq!(items.len(), 2);
}

/// An empty exclusion list must be ABSENT, not empty: CTAP2 treats a present
/// zero-length list as a protocol error on some authenticators.
#[test]
fn an_empty_exclude_list_is_omitted_entirely() {
    let bytes = match make_credential().encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!()
    };
    assert!(
        !entries
            .iter()
            .any(|(key, _)| matches!(key, Value::Integer(i) if i128::from(*i) == 5)),
        "an empty excludeList must not be sent at all"
    );
}

/// A wallet key that is not discoverable cannot be found at sign-in, so the
/// wallet cannot be opened. `rk` is not optional here.
#[test]
fn a_wallet_key_is_always_requested_as_discoverable() {
    let bytes = match make_credential().encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!()
    };
    let Some(Value::Map(options)) = entries.iter().find_map(|(key, value)| match key {
        Value::Integer(i) if i128::from(*i) == 7 => Some(value),
        _ => None,
    }) else {
        unreachable!("options missing");
    };
    assert!(options.iter().any(|(key, value)| matches!(
        (key, value),
        (Value::Text(name), Value::Bool(true)) if name == "rk"
    )));
}

/// The "who are you?" ceremony: no allowList at all, so the authenticator
/// offers whatever discoverable credential it holds for this RP.
#[test]
fn a_discoverable_sign_in_sends_no_allow_list() {
    let request = GetAssertion {
        rp_id: "getvela.app".to_owned(),
        client_data_hash: vec![0xcc; 32],
        allow: Vec::new(),
        user_presence: true,
        user_verification: true,
        pin_uv_auth: None,
    };
    let bytes = match request.encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(bytes[0], Command::GetAssertion as u8);
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!()
    };
    assert!(
        !entries
            .iter()
            .any(|(key, _)| matches!(key, Value::Integer(i) if i128::from(*i) == 3)),
        "an empty allowList must not be sent at all"
    );
}

#[test]
fn get_info_is_a_bare_command_byte() {
    assert_eq!(get_info_request(), Ok(vec![Command::GetInfo as u8]));
}

// ---------------------------------------------------------------------------
// Responses
// ---------------------------------------------------------------------------

#[test]
fn a_response_splits_into_a_status_and_a_body() {
    let (status, body) = match split_response(&[0x00, 0xa0]) {
        Ok(pair) => pair,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(status, Status::Success);
    assert_eq!(body, &[0xa0]);

    assert!(
        split_response(&[]).is_err(),
        "an empty response is not a status"
    );
}

/// Only the codes a wallet branches on differently get a name. Everything else
/// keeps its number, because inventing a friendlier label for an error nobody
/// handles loses the one detail a bug report needs.
#[test]
fn status_codes_map_to_the_decisions_they_drive() {
    assert_eq!(Status::from_byte(0x00), Status::Success);
    // The numbers below are libfido2's `src/fido/err.h` (the CTAP reference
    // client), read on 2026-09-19 — NOT this crate's own table read back to
    // itself, which is what this test used to be and why it passed for months
    // with 0x19 and 0x21 swapped around and 0x2d missing:
    //   FIDO_ERR_CREDENTIAL_EXCLUDED 0x19   FIDO_ERR_PROCESSING        0x21
    //   FIDO_ERR_OPERATION_DENIED    0x27   FIDO_ERR_KEEPALIVE_CANCEL  0x2d
    //   FIDO_ERR_NO_CREDENTIALS      0x2e   FIDO_ERR_USER_ACTION_TIMEOUT 0x2f
    assert_eq!(Status::from_byte(0x19), Status::CredentialExcluded);
    assert_eq!(Status::from_byte(0x27), Status::Cancelled);
    // What a key answers to CTAPHID CANCEL — the person pressed Cancel here.
    assert_eq!(Status::from_byte(0x2d), Status::Cancelled);
    assert_eq!(Status::from_byte(0x2f), Status::Cancelled);
    // PROCESSING is nobody's decision: it keeps its number.
    assert_eq!(Status::from_byte(0x21), Status::Other(0x21));
    assert_eq!(Status::from_byte(0x2e), Status::NoCredentials);
    assert_eq!(Status::from_byte(0x7f), Status::Other(0x7f));
    assert!(Status::Success.is_success());
    assert!(!Status::Cancelled.is_success());
}

/// The five PIN codes, by number.
///
/// They sit next to each other and mean five different things, and a wallet
/// that crosses two of them says something false to a person holding a key:
/// "it is locked" when the PIN was merely mistyped, or "try again" to a key
/// that will refuse every attempt until it is unplugged. The numbers are
/// CTAP 2.1 §6.3 and this test is what pins them.
#[test]
fn the_five_pin_codes_are_five_different_sentences() {
    // 0x31 PIN_INVALID — wrong PIN, one attempt spent, ask again.
    assert_eq!(Status::from_byte(0x31), Status::PinRequired);
    // 0x36 PUAT_REQUIRED — a token is needed and none was sent.
    assert_eq!(Status::from_byte(0x36), Status::PinRequired);

    // 0x32 PIN_BLOCKED — out of attempts; only a reset clears it, and a reset
    // destroys every credential on the key.
    assert_eq!(Status::from_byte(0x32), Status::PinBlocked);
    // 0x34 PIN_AUTH_BLOCKED — power-cycle to clear.
    assert_eq!(Status::from_byte(0x34), Status::PinBlocked);

    // 0x35 PIN_NOT_SET — no PIN exists. Asking for one cannot help.
    assert_eq!(Status::from_byte(0x35), Status::PinNotSet);
    // 0x2b UNSUPPORTED_OPTION — what a key with neither a PIN nor a biometric
    // answers to a request that asks for user verification. Same instruction.
    assert_eq!(Status::from_byte(0x2b), Status::PinNotSet);

    // 0x33 PIN_AUTH_INVALID — the pinUvAuthParam did not verify. A CLIENT
    // fault, not a person's; it keeps its number so a bug report names it.
    assert_eq!(Status::from_byte(0x33), Status::Other(0x33));
}

/// THE integration point.
///
/// A real attestation object is taken apart into the three fields a CTAP2
/// `makeCredential` answers with, then reassembled by our own encoder — and the
/// parsers the browser path already uses must get the same public key and the
/// same versioned attestation out of the result.
///
/// If this drifts, a desktop-minted key and a browser-minted key derive
/// different addresses from the same authenticator, and the wallet is two
/// wallets.
#[test]
fn a_reassembled_attestation_object_is_indistinguishable_from_a_browser_one() {
    let original = real_attestation_object();

    // Take it apart the way a CTAP2 response would arrive: status byte, then a
    // map of {1: fmt, 2: authData, 3: attStmt}.
    let Value::Map(fields) = decode(&original) else {
        unreachable!("the vector is not a map")
    };
    let mut fmt = None;
    let mut auth_data = None;
    let mut att_stmt = None;
    for (key, value) in &fields {
        match (key, value) {
            (Value::Text(name), Value::Text(text)) if name == "fmt" => fmt = Some(text.clone()),
            (Value::Text(name), Value::Bytes(bytes)) if name == "authData" => {
                auth_data = Some(bytes.clone())
            }
            (Value::Text(name), other) if name == "attStmt" => att_stmt = Some(other.clone()),
            _ => {}
        }
    }
    let response_map = Value::Map(vec![
        (
            Value::Integer(1.into()),
            Value::Text(fmt.clone().unwrap_or_default()),
        ),
        (
            Value::Integer(2.into()),
            Value::Bytes(auth_data.clone().unwrap_or_default()),
        ),
        (
            Value::Integer(3.into()),
            att_stmt.clone().unwrap_or(Value::Map(Vec::new())),
        ),
    ]);
    let mut body = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&response_map, &mut body) {
        unreachable!("{error}");
    }

    let parsed = match parse_make_credential(&body) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(Some(parsed.fmt.clone()), fmt);
    assert_eq!(Some(parsed.auth_data.clone()), auth_data);

    let rebuilt = match attestation_object(&parsed) {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };

    let from_original = match webauthn::extract_attestation_public_key(&original) {
        Ok(key) => key,
        Err(error) => unreachable!("the vector must parse: {error:?}"),
    };
    let from_rebuilt = match webauthn::extract_attestation_public_key(&rebuilt) {
        Ok(key) => key,
        Err(error) => unreachable!("the rebuilt object must parse: {error:?}"),
    };
    assert_eq!(
        (from_original.x, from_original.y),
        (from_rebuilt.x, from_rebuilt.y),
        "a CTAP-minted key must derive the same address as a browser-minted one"
    );

    let hex_of = |bytes: &[u8]| -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() };
    assert_eq!(
        webauthn::extract_attestation(&hex_of(&original)),
        webauthn::extract_attestation(&hex_of(&rebuilt)),
        "the versioned attestation — and with it the backup-state flag the \
         second-key gate reads — must survive the round trip"
    );
}

#[test]
fn an_assertion_response_yields_its_signature_and_handle() {
    let body_map = Value::Map(vec![
        (
            Value::Integer(1.into()),
            Value::Map(vec![
                (
                    Value::Text("type".to_owned()),
                    Value::Text("public-key".to_owned()),
                ),
                (Value::Text("id".to_owned()), Value::Bytes(vec![0xde; 16])),
            ]),
        ),
        (Value::Integer(2.into()), Value::Bytes(vec![0x01; 37])),
        (Value::Integer(3.into()), Value::Bytes(vec![0x30, 0x44])),
        (
            Value::Integer(4.into()),
            Value::Map(vec![(
                Value::Text("id".to_owned()),
                Value::Bytes(b"Everyday wallet\0uuid".to_vec()),
            )]),
        ),
    ]);
    let mut body = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&body_map, &mut body) {
        unreachable!("{error}");
    }

    let parsed = match parse_get_assertion(&body) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(parsed.credential_id, Some(vec![0xde; 16]));
    assert_eq!(parsed.auth_data.len(), 37);
    assert_eq!(parsed.signature_der, vec![0x30, 0x44]);
    assert_eq!(parsed.user_id, Some(b"Everyday wallet\0uuid".to_vec()));
}

/// A response missing `authData` is not a partially-useful response — there is
/// nothing to verify — so it fails rather than yielding an empty vector.
#[test]
fn an_assertion_without_auth_data_is_an_error() {
    let mut body = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&Value::Map(vec![]), &mut body) {
        unreachable!("{error}");
    }
    assert!(parse_get_assertion(&body).is_err());
}

#[test]
fn get_info_reports_what_the_wallet_branches_on() {
    let body_map = Value::Map(vec![
        (
            Value::Integer(1.into()),
            Value::Array(vec![
                Value::Text("U2F_V2".to_owned()),
                Value::Text("FIDO_2_0".to_owned()),
            ]),
        ),
        (Value::Integer(3.into()), Value::Bytes(vec![0x2f; 16])),
        (
            Value::Integer(4.into()),
            Value::Map(vec![
                (Value::Text("rk".to_owned()), Value::Bool(true)),
                (Value::Text("clientPin".to_owned()), Value::Bool(true)),
                (Value::Text("uv".to_owned()), Value::Bool(false)),
            ]),
        ),
        (
            Value::Integer(6.into()),
            Value::Array(vec![Value::Integer(2.into()), Value::Integer(1.into())]),
        ),
    ]);
    let mut body = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&body_map, &mut body) {
        unreachable!("{error}");
    }

    let info = match parse_get_info(&body) {
        Ok(info) => info,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(info.versions, vec!["U2F_V2", "FIDO_2_0"]);
    assert_eq!(info.aaguid.len(), 16);
    assert!(
        info.resident_key,
        "a wallet cannot use an authenticator without rk"
    );
    assert!(
        info.client_pin_set,
        "a PIN is set — the ceremony needs a token"
    );
    assert!(!info.user_verification);
    assert_eq!(info.pin_protocols, vec![2, 1]);
}

// ---------------------------------------------------------------------------
// authenticatorClientPIN
// ---------------------------------------------------------------------------

/// The request that opens every PIN session, byte for byte.
///
/// Hand-computable, which is the point: `06` is the command, `a2` a two-entry
/// map, then `01 01` (pinUvAuthProtocol = 1) and `02 02` (subCommand =
/// getKeyAgreement). If this ever grows a third key, an authenticator that
/// reads the request strictly stops answering and the desktop client loses its
/// only way to reach a key with a PIN.
#[test]
fn get_key_agreement_is_six_bytes() {
    let bytes = match ClientPin::key_agreement(Protocol::One).encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(bytes, vec![0x06, 0xa2, 0x01, 0x01, 0x02, 0x02]);

    let two = match ClientPin::key_agreement(Protocol::Two).encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(two, vec![0x06, 0xa2, 0x01, 0x02, 0x02, 0x02]);
}

/// A real P-256 point — the curve's generator, from SEC 2 §2.4.2. Written out
/// rather than computed, so this fixture cannot drift with a p256 upgrade. Any
/// fabricated (x, y) is refused by the shared COSE decoder, which the
/// off-curve test below asserts directly.
fn platform_key() -> P256PublicKey {
    let hex = |text: &str| match vela_core::primitives::from_hex(text) {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("generator coordinate is not hex: {error}"),
    };
    P256PublicKey {
        x: hex("6b17d1f2e12c4247f8bce6e563a440f277037d812deb33a0f4a13945d898c296"),
        y: hex("4fe342e2fe1a7f9b8ee7eb4a7c0f9e162bce33576b315ececbb6406837bf51f5"),
    }
}

/// The permissions subcommand carries what CTAP 2.1 scopes a token by; the
/// 2.0 fallback carries neither, because `getPinToken` has no key for them.
#[test]
fn only_the_permissions_subcommand_scopes_the_token() {
    let scoped = ClientPin::pin_token(
        Protocol::Two,
        platform_key(),
        vec![0x11; 32],
        Some(Permissions::MAKE_CREDENTIAL | Permissions::GET_ASSERTION),
        Some("getvela.app".to_owned()),
    );
    assert_eq!(
        scoped.subcommand,
        ClientPinSubcommand::GetPinUvAuthTokenUsingPinWithPermissions
    );
    let map = match decode(
        &scoped
            .encode()
            .unwrap_or_else(|error| unreachable!("{error:?}"))[1..],
    ) {
        Value::Map(entries) => entries,
        other => unreachable!("clientPIN request is not a map: {other:?}"),
    };
    let at = |key: i64| {
        map.iter().find_map(|(k, v)| match k {
            Value::Integer(i) if i128::from(*i) == i128::from(key) => Some(v.clone()),
            _ => None,
        })
    };
    assert_eq!(at(0x09), Some(Value::Integer(3.into())), "mc | ga");
    assert_eq!(at(0x0a), Some(Value::Text("getvela.app".to_owned())));
    assert!(at(0x03).is_some(), "keyAgreement must travel with the hash");
    assert_eq!(at(0x06), Some(Value::Bytes(vec![0x11; 32])));

    // The same call with no permissions must fall back to CTAP 2.0's
    // unscoped getPinToken — and must NOT smuggle the rpId into it, which is
    // a key that subcommand does not define.
    let unscoped = ClientPin::pin_token(
        Protocol::Two,
        platform_key(),
        vec![0x11; 32],
        None,
        Some("getvela.app".to_owned()),
    );
    assert_eq!(unscoped.subcommand, ClientPinSubcommand::GetPinToken);
    assert_eq!(unscoped.rp_id, None);
    let bytes = unscoped
        .encode()
        .unwrap_or_else(|error| unreachable!("{error:?}"));
    let map = match decode(&bytes[1..]) {
        Value::Map(entries) => entries,
        other => unreachable!("{other:?}"),
    };
    assert!(
        !map.iter().any(|(k, _)| matches!(
            k,
            Value::Integer(i) if i128::from(*i) == 0x09 || i128::from(*i) == 0x0a
        )),
        "getPinToken must carry neither permissions nor rpId"
    );
}

/// The platform's key goes out as a COSE_Key the wallet's own decoder reads
/// back unchanged — the same decoder that reads a credential's public key out
/// of attested credential data. If these two ever diverge, the desktop mints
/// tokens no authenticator accepts and the failure surfaces as "wrong PIN".
#[test]
fn the_key_agreement_key_round_trips_through_the_shared_cose_decoder() {
    let key = platform_key();
    let request = ClientPin::pin_token(Protocol::Two, key.clone(), vec![0x22; 32], None, None);
    let bytes = match request.encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let map = match decode(&bytes[1..]) {
        Value::Map(entries) => entries,
        other => unreachable!("{other:?}"),
    };
    let cose = map
        .iter()
        .find_map(|(k, v)| match k {
            Value::Integer(i) if i128::from(*i) == 0x03 => Some(v.clone()),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("no keyAgreement in the request"));

    // alg MUST be ECDH-ES+HKDF-256 (-25). Authenticators check it, and nothing
    // in the wallet derives keys with it — protocol Two runs its own HKDF.
    let alg = match &cose {
        Value::Map(entries) => entries.iter().find_map(|(k, v)| match k {
            Value::Integer(i) if i128::from(*i) == 3 => Some(v.clone()),
            _ => None,
        }),
        other => unreachable!("keyAgreement is not a map: {other:?}"),
    };
    assert_eq!(alg, Some(Value::Integer((-25).into())));

    // And it decodes back to the same point through the ONE COSE decoder.
    let mut encoded = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&cose, &mut encoded) {
        unreachable!("{error}");
    }
    let response = client_pin_response(vec![(0x01, cose)]);
    let parsed = match vela_core::ctap::parse_client_pin(&response) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(parsed.key_agreement, Some(key));
}

fn client_pin_response(entries: Vec<(i64, Value)>) -> Vec<u8> {
    let map = Value::Map(
        entries
            .into_iter()
            .map(|(key, value)| (Value::Integer(key.into()), value))
            .collect(),
    );
    let mut body = Vec::new();
    if let Err(error) = ciborium::ser::into_writer(&map, &mut body) {
        unreachable!("{error}");
    }
    body
}

/// A keyAgreement key that is not on P-256 must be refused, not agreed with.
///
/// This is the one response in the PIN flow an attacker on the wire can
/// usefully forge: a client that completes ECDH against an off-curve point can
/// be walked into a shared secret with structure the forger chose. The check
/// lives in the shared COSE decoder, so this test is really asserting that the
/// CTAP path goes through it.
#[test]
fn an_off_curve_key_agreement_key_is_refused() {
    let bogus = Value::Map(vec![
        (Value::Integer(1.into()), Value::Integer(2.into())),
        (Value::Integer(3.into()), Value::Integer((-25).into())),
        (Value::Integer((-1).into()), Value::Integer(1.into())),
        (Value::Integer((-2).into()), Value::Bytes(vec![0x01; 32])),
        (Value::Integer((-3).into()), Value::Bytes(vec![0x02; 32])),
    ]);
    let response = client_pin_response(vec![(0x01, bogus)]);
    assert!(
        vela_core::ctap::parse_client_pin(&response).is_err(),
        "an off-curve keyAgreement key must not produce a shared secret"
    );
}

/// Retries and the power-cycle flag are what a wallet warns on before spending
/// an attempt. Zero retries means the key is locked until it is RESET, and a
/// reset destroys the wallet's founding credential.
#[test]
fn retries_and_power_cycle_are_read_back() {
    let response = client_pin_response(vec![
        (0x03, Value::Integer(2.into())),
        (0x04, Value::Bool(true)),
        (0x05, Value::Integer(5.into())),
    ]);
    let parsed = match vela_core::ctap::parse_client_pin(&response) {
        Ok(parsed) => parsed,
        Err(error) => unreachable!("{error:?}"),
    };
    assert_eq!(parsed.pin_retries, Some(2));
    assert!(parsed.power_cycle_state);
    assert_eq!(parsed.uv_retries, Some(5));
    assert_eq!(parsed.pin_uv_auth_token, None);
    assert_eq!(parsed.key_agreement, None);
}

/// The silent probe: "do you hold this credential?", asked without lighting
/// the key up.
///
/// `up: false` has to be EMITTED. CTAP2 defaults `up` to true when the key is
/// absent from the options map, so a probe that merely leaves it out is a
/// request that makes the authenticator blink — which is the opposite of a
/// silent probe, and with three keys plugged in means three keys blinking for
/// a question the client could have answered itself.
#[test]
fn a_silent_probe_says_up_false_out_loud() {
    let probe = GetAssertion {
        rp_id: "getvela.app".to_owned(),
        client_data_hash: vec![0xbb; 32],
        allow: vec![CredentialDescriptor { id: vec![1, 2, 3] }],
        user_presence: false,
        user_verification: false,
        pin_uv_auth: None,
    };
    let bytes = match probe.encode() {
        Ok(bytes) => bytes,
        Err(error) => unreachable!("{error:?}"),
    };
    let Value::Map(entries) = decode(&bytes[1..]) else {
        unreachable!("request is not a map");
    };
    let options = entries
        .iter()
        .find_map(|(k, v)| match k {
            Value::Integer(i) if i128::from(*i) == 0x05 => Some(v.clone()),
            _ => None,
        })
        .unwrap_or_else(|| unreachable!("no options map"));
    let Value::Map(options) = options else {
        unreachable!("options is not a map");
    };
    assert_eq!(
        options.iter().find_map(|(k, v)| match k {
            Value::Text(name) if name == "up" => Some(v.clone()),
            _ => None,
        }),
        Some(Value::Bool(false)),
        "a probe that omits `up` is a probe that makes the key blink"
    );
    assert!(
        !options
            .iter()
            .any(|(k, _)| matches!(k, Value::Text(name) if name == "uv")),
        "a probe asks for no verification either"
    );

    // And the allow list pins exactly the one credential being asked about,
    // so the answer is about IT and not about whatever else the key holds.
    let allow = entries.iter().find_map(|(k, v)| match k {
        Value::Integer(i) if i128::from(*i) == 0x03 => Some(v.clone()),
        _ => None,
    });
    let Some(Value::Array(allow)) = allow else {
        unreachable!("no allow list");
    };
    assert_eq!(allow.len(), 1);
}
