//! The Trusted Signer's request, verification and channels (spec 071), with a
//! real P-256 key standing in for the passkey.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::signature::hazmat::PrehashSigner as _;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{json, Value};
use vela_core::primitives::{sha256, to_hex};
use vela_core::trusted_signer::integrity::{CheckFailure, LAUNCH};
use vela_core::trusted_signer::launch::{admit, target, CheckedPage};
use vela_core::trusted_signer::ws::{intent_message, parse_message, Connection, Message};
use vela_core::trusted_signer::{
    self, parse_callback, request, verify, RequestInput, TrustedSignerError,
};

/// The official page at its launch version, checked and admitted — the only
/// way a launch URL is built (spec 102 R6).
fn checked_official() -> CheckedPage {
    let page = target("https://sign.getvela.app", None, &[], &[]).expect("a version");
    admit(
        &page,
        Some(LAUNCH),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        1_000,
    )
    .page()
    .cloned()
    .expect("the shipped page is admitted")
}
use vela_core::user_op::{UserOperation, WalletKey};
use vela_core::webauthn::webauthn_signing_hash;

const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

fn key(seed: u8) -> SigningKey {
    SigningKey::from_slice(&[seed; 32]).expect("a valid scalar")
}

fn wallet_key(signing: &SigningKey, credential: &[u8]) -> WalletKey {
    WalletKey {
        credential_id: to_hex(credential, false),
        public_key_hex: to_hex(
            signing.verifying_key().to_sec1_point(false).as_bytes(),
            false,
        ),
    }
}

/// What the page returns for `digest`, signed by `signing` as `credential`.
fn answer(signing: &SigningKey, credential: &[u8], digest: &[u8], flags: u8) -> Value {
    let mut authenticator_data = sha256(b"getvela.app");
    authenticator_data.push(flags);
    authenticator_data.extend_from_slice(&[0, 0, 0, 7]);
    let client = format!(
        r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://sign.getvela.app","crossOrigin":false}}"#,
        URL_SAFE_NO_PAD.encode(digest)
    );
    let prehash = webauthn_signing_hash(&authenticator_data, client.as_bytes());
    let signature: Signature = signing.sign_prehash(&prehash).expect("signs");
    json!({
        "credentialId": URL_SAFE_NO_PAD.encode(credential),
        "signature": to_hex(&signature.to_bytes(), true),
        "authenticatorData": to_hex(&authenticator_data, true),
        "clientDataJSON": to_hex(client.as_bytes(), true),
        "userVerified": flags & 0x04 != 0,
    })
}

const UV: u8 = 0x05;

#[test]
fn a_verified_answer_over_this_digest_by_this_wallet_is_accepted() {
    let signing = key(7);
    let digest = [0xab_u8; 32];
    let credential = [0x11_u8, 0x22, 0x33];
    let keys = vec![
        wallet_key(&key(9), &[0x99]),
        wallet_key(&signing, &credential),
    ];
    let verified =
        verify(&answer(&signing, &credential, &digest, UV), &digest, &keys).expect("accepted");
    assert_eq!(verified.credential_id_hex, "112233");
    assert_eq!(
        verified.signature_der[0], 0x30,
        "DER, as the Safe envelope takes"
    );
    assert!(verified
        .client_data_json
        .starts_with(br#"{"type":"webauthn.get""#));
}

#[test]
fn a_signature_over_another_digest_is_refused() {
    let signing = key(7);
    let keys = vec![wallet_key(&signing, &[1])];
    let result = verify(&answer(&signing, &[1], &[0xcd; 32], UV), &[0xab; 32], &keys);
    assert_eq!(result, Err(TrustedSignerError::WrongChallenge));
}

#[test]
fn a_key_that_is_not_this_wallets_is_refused() {
    let signing = key(7);
    let keys = vec![wallet_key(&key(8), &[2])];
    let result = verify(&answer(&signing, &[1], &[0xab; 32], UV), &[0xab; 32], &keys);
    assert_eq!(result, Err(TrustedSignerError::ForeignKey));
}

#[test]
fn a_known_credential_signed_by_another_key_is_refused() {
    // The id matches, the key does not: the page claimed a credential it does
    // not hold.
    let keys = vec![wallet_key(&key(8), &[1])];
    let result = verify(&answer(&key(7), &[1], &[0xab; 32], UV), &[0xab; 32], &keys);
    assert_eq!(result, Err(TrustedSignerError::BadSignature));
}

#[test]
fn a_tampered_signature_is_refused() {
    let signing = key(7);
    let keys = vec![wallet_key(&signing, &[1])];
    let mut tampered = answer(&signing, &[1], &[0xab; 32], UV);
    let sig = tampered["signature"].as_str().unwrap().to_owned();
    let flipped = format!(
        "{}{}",
        &sig[..sig.len() - 2],
        if sig.ends_with("00") { "01" } else { "00" }
    );
    tampered["signature"] = json!(flipped);
    assert_eq!(
        verify(&tampered, &[0xab; 32], &keys),
        Err(TrustedSignerError::BadSignature)
    );
}

#[test]
fn an_unverified_user_is_refused() {
    let signing = key(7);
    let keys = vec![wallet_key(&signing, &[1])];
    let result = verify(
        &answer(&signing, &[1], &[0xab; 32], 0x01),
        &[0xab; 32],
        &keys,
    );
    assert_eq!(result, Err(TrustedSignerError::NotVerified));
}

#[test]
fn a_malformed_answer_is_refused_not_trusted() {
    let keys = vec![wallet_key(&key(7), &[1])];
    assert!(matches!(
        verify(&json!({}), &[0; 32], &keys),
        Err(TrustedSignerError::Malformed(_))
    ));
    let mut bad = answer(&key(7), &[1], &[0; 32], UV);
    bad["signature"] = json!("0xnothex");
    assert!(matches!(
        verify(&bad, &[0; 32], &keys),
        Err(TrustedSignerError::Malformed(_))
    ));
}

fn sample_op() -> UserOperation {
    UserOperation {
        sender: SAFE.to_owned(),
        nonce: "0x7".to_owned(),
        init_code: vec![],
        call_data: vec![0x7b, 0xb3, 0x74, 0x28],
        verification_gas_limit: 300_000,
        call_gas_limit: 200_000,
        pre_verification_gas: 110_000,
        max_fee_per_gas: 1_500_000_007,
        max_priority_fee_per_gas: 1_500_000_000,
        paymaster_and_data: vec![],
        signature: vec![],
    }
}

#[test]
fn the_request_carries_the_operation_and_the_wallets_keys() {
    let op = sample_op();
    let ids = vec!["112233".to_owned(), "0xaabb".to_owned()];
    let built = request(&RequestInput {
        method: "eth_sendTransaction",
        params: json!([{ "to": SAFE, "value": "0x1" }]),
        origin: "https://app.uniswap.org",
        chain_id: 100,
        chain_name: Some("Gnosis"),
        native_symbol: Some("xDAI"),
        account: SAFE,
        account_name: Some("savings"),
        credential_ids_hex: &ids,
        key_route: None,
        user_op: Some(&op),
        calls: &[vela_core::user_op::MultiSendCall {
            to: SAFE.into(),
            value_hex: "0x1".into(),
            data: vec![],
        }],
        origin_seen_by_browser: false,
    });
    assert_eq!(built["intent"]["method"], "eth_sendTransaction");
    assert_eq!(built["intent"]["origin"], "https://app.uniswap.org");
    let context = &built["context"];
    assert_eq!(context["chainId"], 100);
    assert_eq!(context["account"], SAFE);
    assert_eq!(
        context["signer"],
        json!({ "name": "savings", "letter": "S" })
    );
    assert_eq!(context["allowCredentials"], json!(["ESIz", "qrs"]));
    let operation = &context["operation"];
    assert_eq!(operation["feeLegIndex"], 1);
    assert_eq!(operation["userOp"]["callData"], "0x7bb37428");
    assert_eq!(operation["userOp"]["maxFeePerGas"], "1500000007");
    assert_eq!(operation["entryPoint"], vela_core::safe::ENTRY_POINT);
    // What the web holds as JSON reads back as the same operation.
    assert_eq!(
        trusted_signer::user_op_from_json(&operation["userOp"]),
        Some(op)
    );
    let mut hex_gas = operation["userOp"].clone();
    hex_gas["callGasLimit"] = json!("0x30d40");
    assert_eq!(
        trusted_signer::user_op_from_json(&hex_gas).map(|o| o.call_gas_limit),
        Some(200_000)
    );
}

/// Spec 102 R5: with the account's key route the page is offered that one key
/// — its transports and the hints for where it lives — so the browser goes
/// straight to it instead of asking where the passkey is.
#[test]
fn the_request_names_the_key_to_use_and_where_it_lives() {
    let ids = vec!["112233".to_owned(), "aabb".to_owned()];
    let route = vela_core::signing_venue::KeyRoute::new("aabb", "hybrid", "hybrid,internal");
    let built = request(&RequestInput {
        method: "personal_sign",
        params: json!(["0x68656c6c6f", SAFE]),
        origin: "https://app.example",
        chain_id: 1,
        account: SAFE,
        credential_ids_hex: &ids,
        key_route: Some(&route),
        ..Default::default()
    });
    let context = &built["context"];
    assert_eq!(
        context["allowCredentials"],
        json!(["qrs"]),
        "only the key this device signs with"
    );
    assert_eq!(
        context["keyRoute"],
        json!({
            "credentialId": "qrs",
            "place": "hybrid",
            "transports": ["hybrid", "internal"],
            "hints": ["hybrid"],
        })
    );
    // No route: every key, as before, and no `keyRoute`.
    let built = request(&RequestInput {
        method: "personal_sign",
        params: json!(["0x68656c6c6f", SAFE]),
        chain_id: 1,
        account: SAFE,
        credential_ids_hex: &ids,
        ..Default::default()
    });
    assert_eq!(built["context"]["allowCredentials"], json!(["ESIz", "qrs"]));
    assert!(built["context"].get("keyRoute").is_none());
}

#[test]
fn a_message_request_names_no_operation() {
    let built = request(&RequestInput {
        method: "personal_sign",
        params: json!(["0x68656c6c6f", SAFE]),
        origin: "https://app.example",
        chain_id: 1,
        account: SAFE,
        ..Default::default()
    });
    assert!(built["context"].get("operation").is_none());
    assert!(built["context"].get("allowCredentials").is_none());
}

#[test]
fn the_url_channel_round_trips() {
    let built = json!({ "intent": { "method": "personal_sign" }, "context": { "chainId": 1 } });
    let url = checked_official()
        .url_launch(&built, "http://127.0.0.1:51234/vela", "tok en", "", 2_000)
        .unwrap();
    // The official host opens the version that was checked, content-addressed.
    let pinned = format!("https://sign.getvela.app/b/{LAUNCH}/sign?ch=url#i=");
    assert!(url.starts_with(&pinned), "{url}");
    let fragment = url.split_once('#').unwrap().1;
    let mut parts = fragment.split('&').map(|p| p.split_once('=').unwrap());
    let (_, i) = parts.next().unwrap();
    let inflated =
        miniz_oxide::inflate::decompress_to_vec(&URL_SAFE_NO_PAD.decode(i).unwrap()).unwrap();
    assert_eq!(serde_json::from_slice::<Value>(&inflated).unwrap(), built);
    assert!(fragment.contains("&t=tok%20en&z=1"));

    let result = json!({ "credentialId": "AQ" });
    let query = format!(
        "t=tok%20en&result={}",
        URL_SAFE_NO_PAD.encode(result.to_string())
    );
    assert_eq!(parse_callback(&query, "tok en"), Ok(result));
    assert_eq!(
        parse_callback("t=other&result=e30", "tok en"),
        Err(TrustedSignerError::WrongToken)
    );
    assert_eq!(
        parse_callback("t=tok%20en&error=user_rejected", "tok en"),
        Err(TrustedSignerError::Declined)
    );
    assert_eq!(
        parse_callback("t=tok%20en&error=refused", "tok en"),
        Err(TrustedSignerError::Refused("refused".into()))
    );
}

#[test]
fn the_websocket_channel_speaks_the_protocol() {
    assert_eq!(
        checked_official()
            .ws_launch(51_234, "abc", "", 2_000)
            .unwrap(),
        format!("https://sign.getvela.app/b/{LAUNCH}/sign?ch=ws#p=51234&t=abc")
    );
    let built = json!({ "intent": { "method": "personal_sign" }, "context": { "chainId": 1 } });
    let intent: Value = serde_json::from_str(&intent_message("r1", &built)).unwrap();
    assert_eq!(intent["t"], "intent");
    assert_eq!(intent["id"], "r1");
    assert_eq!(intent["intent"]["method"], "personal_sign");
    assert_eq!(
        parse_message(r#"{"v":1,"t":"hello","token":"abc"}"#),
        Message::Hello {
            token: "abc".into()
        }
    );
    assert_eq!(
        parse_message(r#"{"v":1,"t":"result","id":"r1","result":{"credentialId":"AQ"}}"#),
        Message::Result {
            id: "r1".into(),
            result: json!({ "credentialId": "AQ" })
        }
    );
    assert_eq!(
        parse_message(r#"{"v":1,"t":"error","id":"r1","code":"user_rejected"}"#),
        Message::Error {
            id: "r1".into(),
            error: TrustedSignerError::Declined
        }
    );
    assert_eq!(parse_message("nonsense"), Message::Ignored);
    assert_eq!(
        trusted_signer::DEFAULT_SIGNER_URL,
        "https://sign.getvela.app/"
    );
}

// --- the loopback WebSocket, byte for byte ----------------------------------

fn upgrade(origin: &str) -> Vec<u8> {
    format!(
        "GET / HTTP/1.1\r\nHost: 127.0.0.1:51234\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
         Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\nOrigin: {origin}\r\n\r\n"
    )
    .into_bytes()
}

/// A masked client frame, as a browser sends it.
fn client_frame(opcode: u8, fin: bool, payload: &[u8]) -> Vec<u8> {
    let mask = [0x37, 0xfa, 0x21, 0x3d];
    let mut out = vec![if fin { 0x80 } else { 0 } | opcode];
    match payload.len() {
        n if n < 126 => out.push(0x80 | n as u8),
        n => {
            out.push(0x80 | 126);
            out.extend_from_slice(&(n as u16).to_be_bytes());
        }
    }
    out.extend_from_slice(&mask);
    out.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
    out
}

fn text(message: &str) -> Vec<u8> {
    client_frame(0x1, true, message.as_bytes())
}

/// The text of the server frames in `bytes` (after any HTTP head).
fn server_texts(bytes: &[u8]) -> Vec<String> {
    let mut at = bytes
        .windows(4)
        .position(|w| w == b"\r\n\r\n")
        .map_or(0, |i| i + 4);
    if !bytes.starts_with(b"HTTP/") {
        at = 0;
    }
    let mut out = Vec::new();
    while at + 2 <= bytes.len() {
        let opcode = bytes[at] & 0x0f;
        let (len, head) = match bytes[at + 1] {
            126 => (
                u16::from_be_bytes([bytes[at + 2], bytes[at + 3]]) as usize,
                4,
            ),
            n => (n as usize, 2),
        };
        let payload = &bytes[at + head..at + head + len];
        if opcode == 0x1 {
            out.push(String::from_utf8(payload.to_vec()).unwrap());
        }
        at += head + len;
    }
    out
}

fn connection() -> Connection {
    let built = json!({ "intent": { "method": "personal_sign", "params": ["0x68", SAFE] }, "context": { "chainId": 100 } });
    Connection::new("https://sign.getvela.app/", "tok", "r1", &built)
}

#[test]
fn a_page_that_proves_itself_gets_the_intent_and_its_answer_is_the_outcome() {
    let mut c = connection();
    // The head may arrive in pieces.
    let head = upgrade("https://sign.getvela.app");
    let first = c.feed(&head[..20]);
    assert!(first.write.is_empty() && !first.close);
    let open = c.feed(&head[20..]);
    let response = String::from_utf8_lossy(&open.write);
    assert!(response.starts_with("HTTP/1.1 101"), "{response}");
    assert!(response.contains("Sec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo="));

    let hello = c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let sent = server_texts(&hello.write);
    assert_eq!(sent.len(), 1);
    let intent: Value = serde_json::from_str(&sent[0]).unwrap();
    assert_eq!(intent["t"], "intent");
    assert_eq!(intent["id"], "r1");
    assert_eq!(intent["intent"]["method"], "personal_sign");
    assert_eq!(intent["context"]["chainId"], 100);
    assert!(hello.outcome.is_none());

    // An answer for another request is not this one's.
    let stray = c.feed(&text(r#"{"v":1,"t":"result","id":"r0","result":{"x":1}}"#));
    assert!(stray.outcome.is_none() && !stray.close);

    // Fragmented, as a browser may send a large message.
    let answer = r#"{"v":1,"t":"result","id":"r1","result":{"credentialId":"AQ"}}"#.as_bytes();
    let mut bytes = client_frame(0x1, false, &answer[..10]);
    bytes.extend(client_frame(0x9, true, b"ping"));
    bytes.extend(client_frame(0x0, true, &answer[10..]));
    let done = c.feed(&bytes);
    assert_eq!(done.outcome, Some(Ok(json!({ "credentialId": "AQ" }))));
    // Spec 075: the session stays open for the shell's next request.
    assert!(!done.close);
    assert_eq!(
        done.write[0], 0x8a,
        "the ping was answered with a pong first"
    );
    let end = c.end();
    let bye: Value = serde_json::from_str(&server_texts(&end.write)[0]).unwrap();
    assert_eq!(bye["t"], "bye");
    assert!(end.close);
    assert_eq!(
        c.closed(),
        None,
        "an answered conversation has nothing more to say"
    );
}

/// Spec 075: one page visit carries a create and its member proof (or a
/// recovery's two signatures) — the shell sends the next request on the same
/// connection once the last one is answered.
#[test]
fn a_session_carries_the_next_request_on_the_same_connection() {
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    // Too early: the first request has no answer yet.
    let early = c.send("r2", &json!({ "intent": { "method": "vela_proof" } }));
    assert!(early.write.is_empty());

    let created = c.feed(&text(
        r#"{"v":1,"t":"result","id":"r1","registration":{"credentialId":"AQ"}}"#,
    ));
    let Some(Ok(answer)) = created.outcome else {
        panic!("{created:?}")
    };
    assert_eq!(
        answer["registration"]["credentialId"], "AQ",
        "a ceremony's answer reaches the verifier whole"
    );

    let next = c.send(
        "r2",
        &json!({ "intent": { "method": "vela_memberProof" }, "context": {} }),
    );
    let sent: Value = serde_json::from_str(&server_texts(&next.write)[0]).unwrap();
    assert_eq!(
        (sent["t"].as_str(), sent["id"].as_str()),
        (Some("intent"), Some("r2"))
    );
    // The old id is spent.
    let stale = c.feed(&text(r#"{"v":1,"t":"result","id":"r1","assertion":{}}"#));
    assert!(stale.outcome.is_none());
    let proved = c.feed(&text(
        r#"{"v":1,"t":"result","id":"r2","assertion":{"credentialId":"AQ"}}"#,
    ));
    assert!(matches!(proved.outcome, Some(Ok(_))));

    // The page closing between requests ends the session with no outcome.
    assert_eq!(c.closed(), None);
}

#[test]
fn another_origin_is_refused_before_it_can_say_anything() {
    let mut c = connection();
    let step = c.feed(&upgrade("https://evil.example"));
    assert!(String::from_utf8_lossy(&step.write).starts_with("HTTP/1.1 403"));
    assert!(step.close);
    assert_eq!(step.outcome, None, "a stranger does not end the request");
    let mut c = connection();
    let no_origin = c.feed(b"GET / HTTP/1.1\r\nUpgrade: websocket\r\nSec-WebSocket-Key: a\r\n\r\n");
    assert!(no_origin.close && no_origin.outcome.is_none());
}

#[test]
fn a_wrong_token_is_closed_without_ending_the_request() {
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    let step = c.feed(&text(r#"{"v":1,"t":"hello","token":"spent"}"#));
    assert!(
        server_texts(&step.write).is_empty(),
        "no intent for a stranger"
    );
    assert!(step.close);
    assert_eq!(step.outcome, None);
    assert_eq!(c.closed(), None);
}

#[test]
fn a_page_that_goes_away_after_the_intent_was_closed_without_signing() {
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    assert_eq!(c.closed(), Some(TrustedSignerError::Declined));

    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let close = c.feed(&client_frame(0x8, true, &1001u16.to_be_bytes()));
    assert_eq!(close.outcome, Some(Err(TrustedSignerError::Declined)));
    assert!(close.close);
}

#[test]
fn the_pages_refusal_is_the_outcome() {
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let step = c.feed(&text(
        r#"{"v":1,"t":"error","id":"r1","code":"unlimited_approval"}"#,
    ));
    assert_eq!(
        step.outcome,
        Some(Err(TrustedSignerError::Refused(
            "unlimited_approval".into()
        )))
    );
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let step = c.feed(&text(
        r#"{"v":1,"t":"error","id":"r1","code":"user_rejected"}"#,
    ));
    assert_eq!(step.outcome, Some(Err(TrustedSignerError::Declined)));
}

#[test]
fn an_unmasked_frame_is_a_protocol_fault() {
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let step = c.feed(&[0x81, 0x02, b'h', b'i']);
    assert!(step.close);
    assert!(matches!(
        step.outcome,
        Some(Err(TrustedSignerError::Malformed(_)))
    ));
}

#[test]
fn a_signer_address_is_https_or_this_devices_own_loopback() {
    use vela_core::trusted_signer::{signer_url, SignerUrlError};
    assert_eq!(
        signer_url(" sign.getvela.app ").as_deref(),
        Ok("https://sign.getvela.app/")
    );
    assert_eq!(
        signer_url("https://Me.Example/cs/").as_deref(),
        Ok("https://me.example/cs/")
    );
    assert_eq!(
        signer_url("http://localhost:8140").as_deref(),
        Ok("http://localhost:8140/")
    );
    // D9: one loopback origin — a passkey cannot live on an IP address.
    assert_eq!(
        signer_url("http://127.0.0.1:8140/sign.html").as_deref(),
        Ok("http://localhost:8140/sign.html")
    );
    assert_eq!(
        signer_url("http://[::1]:8140/").as_deref(),
        Ok("http://localhost:8140/")
    );
    assert_eq!(
        signer_url("http://192.168.1.4:8140/"),
        Err(SignerUrlError::Insecure)
    );
    assert_eq!(
        signer_url("http://sign.example/"),
        Err(SignerUrlError::Insecure)
    );
    assert_eq!(signer_url("ftp://x/"), Err(SignerUrlError::Invalid));
    assert_eq!(
        signer_url("https://user@evil.example/"),
        Err(SignerUrlError::Invalid)
    );
    assert_eq!(signer_url(""), Err(SignerUrlError::Invalid));
    assert_eq!(signer_url("https:// spaced"), Err(SignerUrlError::Invalid));
}

#[test]
fn the_loopback_callback_is_found_in_the_browsers_request() {
    use vela_core::trusted_signer::callback_query;
    assert_eq!(
        callback_query("GET /vela?t=tok&result=e30 HTTP/1.1\r\nHost: 127.0.0.1:5\r\n\r\n"),
        Some("t=tok&result=e30")
    );
    assert_eq!(
        callback_query("POST /vela?t=tok&error=user_rejected HTTP/1.1\r\n"),
        Some("t=tok&error=user_rejected")
    );
    assert_eq!(callback_query("GET /favicon.ico HTTP/1.1\r\n"), None);
    assert_eq!(callback_query("PUT /vela?t=x HTTP/1.1\r\n"), None);
    assert_eq!(callback_query(""), None);
}

#[test]
fn the_wallets_own_send_is_sent_as_its_calls_with_the_fee_leg_after_them() {
    let op = sample_op();
    let calls = [vela_core::user_op::MultiSendCall {
        to: "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141".into(),
        value_hex: "0x38d7ea4c68000".into(),
        data: vec![],
    }];
    let built = request(&RequestInput {
        method: "",
        origin: "",
        chain_id: 100,
        account: SAFE,
        user_op: Some(&op),
        calls: &calls,
        ..Default::default()
    });
    assert_eq!(built["intent"]["method"], "wallet_sendCalls");
    assert_eq!(
        built["intent"]["origin"], "",
        "the page draws an empty origin as the wallet itself"
    );
    let batch = &built["intent"]["params"][0];
    assert_eq!(batch["chainId"], "0x64");
    assert_eq!(batch["from"], SAFE);
    assert_eq!(batch["calls"][0]["value"], "0x38d7ea4c68000");
    assert_eq!(batch["calls"][0]["data"], "0x");
    assert_eq!(built["context"]["operation"]["feeLegIndex"], 1);
}

/// D1 (real-device pass 2): a multisig executor's operation, as the native
/// shells and the desktop hand it over (`vela_multisigExecute` and the send
/// purpose's fields), is told to the page the way the web tells it — the
/// number, the core's safeTxHash there, the name, and "… anyway" when the
/// one review asks for it (neither safety check read, or a danger): the page
/// says "Execute anyway". Params that do not read add nothing.
#[test]
fn an_executors_page_hears_the_number_and_the_reviews_anyway_from_every_shell() {
    let tx = json!({
        "to": "0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69", "value": "0x1", "data": "0x",
        "operation": 0, "safeTxGas": "0x0", "baseGas": "0x0", "gasPrice": "0x0",
        "gasToken": "0x0000000000000000000000000000000000000000",
        "refundReceiver": "0x0000000000000000000000000000000000000000", "nonce": "0x4"
    });
    let ask = |params: Value| {
        request(&RequestInput {
            method: vela_core::signing_venue::MULTISIG_EXECUTE_METHOD,
            params,
            origin: "",
            chain_id: 100,
            account: SAFE,
            ..Default::default()
        })
    };
    let anyway = ask(json!({
        "tx": tx, "multisig": SAFE, "name": "Team fund", "threshold": 2,
        "owners": [SAFE], "anyway": true
    }));
    let about = &anyway["context"]["multisig"];
    assert_eq!(about["anyway"], true);
    assert_eq!(about["nonce"], "0x4");
    assert_eq!(about["name"], "Team fund");
    let st: vela_core::multisig::SafeTx = serde_json::from_value(tx.clone()).unwrap();
    assert_eq!(
        about["safeTxHash"],
        vela_core::multisig::safe_tx_hash(100, SAFE, &st).unwrap()
    );
    let plain = ask(json!({"tx": tx, "multisig": SAFE, "threshold": 2}));
    assert!(plain["context"]["multisig"].get("anyway").is_none());
    // A shell that hands over nothing readable: nothing said.
    assert!(ask(json!({})).pointer("/context/multisig").is_none());
    assert!(ask(json!({"multisig": SAFE}))
        .pointer("/context/multisig")
        .is_none());
}

/// Spec 079: a request from the wallet's own browser names the site as the
/// browser saw it; one relayed from elsewhere carries no `dapp` (the page keeps
/// its caution).
#[test]
fn a_browser_seen_origin_is_named_to_the_page() {
    use vela_core::trusted_signer::{request, RequestInput};
    let input = |seen: bool| RequestInput {
        method: "personal_sign",
        params: serde_json::json!(["0x48", "0x88cCA0EeDbF2C4426110bbFc998F048689266894"]),
        origin: "https://app.uniswap.org",
        chain_id: 100,
        chain_name: Some("Gnosis"),
        native_symbol: Some("xDAI"),
        account: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
        account_name: None,
        credential_ids_hex: &[],
        key_route: None,
        user_op: None,
        calls: &[],
        origin_seen_by_browser: seen,
    };
    let seen = request(&input(true));
    assert_eq!(
        seen["context"]["dapp"],
        serde_json::json!({ "name": "app.uniswap.org", "origin": "https://app.uniswap.org", "source": "vela_browser" })
    );
    let relayed = request(&input(false));
    assert!(relayed["context"].get("dapp").is_none());
}

/// Spec 079: the version the phones open is one this build accepts — and,
/// spec 102 R6, it is opened only as a checked page, at the address that was
/// checked.
#[test]
fn the_launch_pin_is_allowed_and_opened_where_it_was_checked() {
    use vela_core::trusted_signer::integrity::BUILD_ALLOWED;
    assert!(BUILD_ALLOWED.contains(&LAUNCH));
    let page = checked_official();
    let built = json!({ "intent": { "method": "personal_sign" }, "context": { "chainId": 1 } });
    let url = page
        .url_launch(&built, "velawallet://sign-result", "t", "", 2_000)
        .unwrap();
    assert!(
        url.starts_with(&format!("{}?ch=url#", page.target().url())),
        "{url}"
    );
}

// ---------------------------------------------------------------------------
// The page, everywhere (owner, 2026-10-30): the answer is accepted only over
// the bytes the core computed from the request it built.
// ---------------------------------------------------------------------------

/// A request of `method` as the core builds it for the page.
fn signature_request(method: &str, params: Value, op: Option<&UserOperation>) -> Value {
    request(&RequestInput {
        method,
        params,
        origin: if method.is_empty() {
            ""
        } else {
            "https://app.example"
        },
        chain_id: 5_042_002,
        account: SAFE,
        user_op: op,
        calls: &[vela_core::user_op::MultiSendCall {
            to: SAFE.into(),
            value_hex: "0x1".into(),
            data: vec![],
        }],
        ..Default::default()
    })
}

const TYPED: &str = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"chainId","type":"uint256"}],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Ether Mail","chainId":5042002},"message":{"contents":"Hello"}}"#;

#[test]
fn the_core_computes_what_the_page_must_have_signed() {
    let op = sample_op();
    let safe_op = vela_core::user_op::calculate_safe_op_hash(&op, 5_042_002).expect("hashes");
    for method in ["eth_sendTransaction", "wallet_sendCalls", ""] {
        let built = signature_request(method, json!([{ "to": SAFE, "value": "0x1" }]), Some(&op));
        assert_eq!(
            trusted_signer::digest_of(&built).as_deref(),
            Ok(safe_op.as_slice()),
            "{method:?}"
        );
    }
    // A message: the request's own hash in the account's SafeMessage.
    let personal = signature_request("personal_sign", json!(["0x68656c6c6f", SAFE]), None);
    let original =
        vela_core::sign_message::original_hash("personal_sign", r#"["0x68656c6c6f","x"]"#)
            .expect("a hash");
    let wrapped =
        vela_core::user_op::compute_safe_message_hash(&original, 5_042_002, SAFE).expect("wraps");
    assert_eq!(trusted_signer::digest_of(&personal), Ok(wrapped));
    let typed = signature_request("eth_signTypedData_v4", json!([SAFE, TYPED]), None);
    let original = vela_core::eip712::hash_typed_data(TYPED).expect("hashes");
    let wrapped =
        vela_core::user_op::compute_safe_message_hash(&original, 5_042_002, SAFE).expect("wraps");
    assert_eq!(trusted_signer::digest_of(&typed), Ok(wrapped));

    // What no venue signs, and a request missing what its kind needs.
    let eth_sign = signature_request("eth_sign", json!([SAFE, "0xabcd"]), None);
    assert!(matches!(
        trusted_signer::digest_of(&eth_sign),
        Err(TrustedSignerError::Refused(_))
    ));
    let no_op = signature_request("eth_sendTransaction", json!([{ "to": SAFE }]), None);
    assert!(matches!(
        trusted_signer::digest_of(&no_op),
        Err(TrustedSignerError::Malformed(_))
    ));
}

#[test]
fn the_pages_answer_is_accepted_only_over_the_cores_bytes_by_the_accounts_key() {
    let signing = key(21);
    let credential = [0x21_u8, 0x21];
    let keys = vec![wallet_key(&signing, &credential)];
    let op = sample_op();
    let built = signature_request("wallet_sendCalls", json!([{ "calls": [] }]), Some(&op));
    let digest = trusted_signer::digest_of(&built).expect("a digest");

    let good = answer(&signing, &credential, &digest, UV);
    let verified =
        trusted_signer::accept_signature(&built, &good, &keys, &digest).expect("accepted");
    assert_eq!(verified.credential_id_hex, "2121");

    // The shell is about to submit other bytes than the request describes.
    assert_eq!(
        trusted_signer::accept_signature(&built, &good, &keys, &[0xab; 32]),
        Err(TrustedSignerError::WrongChallenge)
    );
    // An answer over different bytes (another operation).
    let mut other = op.clone();
    other.nonce = "0x8".into();
    let other_digest =
        vela_core::user_op::calculate_safe_op_hash(&other, 5_042_002).expect("hashes");
    assert_eq!(
        trusted_signer::accept_signature(
            &built,
            &answer(&signing, &credential, &other_digest, UV),
            &keys,
            &digest
        ),
        Err(TrustedSignerError::WrongChallenge)
    );
    // An answer by a key that is not the account's.
    let stranger = key(22);
    assert_eq!(
        trusted_signer::accept_signature(
            &built,
            &answer(&stranger, &[0x22], &digest, UV),
            &keys,
            &digest
        ),
        Err(TrustedSignerError::ForeignKey)
    );
    // The account's key id, another key's signature.
    assert_eq!(
        trusted_signer::accept_signature(
            &built,
            &answer(&stranger, &credential, &digest, UV),
            &keys,
            &digest
        ),
        Err(TrustedSignerError::BadSignature)
    );
    // Not user-verified.
    assert_eq!(
        trusted_signer::accept_signature(
            &built,
            &answer(&signing, &credential, &digest, 0x01),
            &keys,
            &digest
        ),
        Err(TrustedSignerError::NotVerified)
    );

    // A message: the same rule over the SafeMessage bytes.
    let message = signature_request("personal_sign", json!(["hello", SAFE]), None);
    let bytes = trusted_signer::digest_of(&message).expect("a digest");
    assert!(trusted_signer::accept_signature(
        &message,
        &answer(&signing, &credential, &bytes, UV),
        &keys,
        &bytes
    )
    .is_ok());
    // Signed over the bare EIP-191 hash (a page that skipped the wrap).
    let bare =
        vela_core::sign_message::original_hash("personal_sign", r#"["hello"]"#).expect("a hash");
    assert_eq!(
        trusted_signer::accept_signature(
            &message,
            &answer(&signing, &credential, &bare, UV),
            &keys,
            &bytes
        ),
        Err(TrustedSignerError::WrongChallenge)
    );
}
