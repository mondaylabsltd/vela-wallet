//! The Clear Signer's request, verification and channels (spec 071), with a
//! real P-256 key standing in for the passkey.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use p256::ecdsa::signature::hazmat::PrehashSigner as _;
use p256::ecdsa::{Signature, SigningKey};
use serde_json::{json, Value};
use vela_core::clear_signer::ws::{intent_message, parse_message, Connection, Message};
use vela_core::clear_signer::{
    self, parse_callback, request, url_launch, verify, ws_launch, ClearSignerError, RequestInput,
};
use vela_core::primitives::{sha256, to_hex};
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
    assert_eq!(result, Err(ClearSignerError::WrongChallenge));
}

#[test]
fn a_key_that_is_not_this_wallets_is_refused() {
    let signing = key(7);
    let keys = vec![wallet_key(&key(8), &[2])];
    let result = verify(&answer(&signing, &[1], &[0xab; 32], UV), &[0xab; 32], &keys);
    assert_eq!(result, Err(ClearSignerError::ForeignKey));
}

#[test]
fn a_known_credential_signed_by_another_key_is_refused() {
    // The id matches, the key does not: the page claimed a credential it does
    // not hold.
    let keys = vec![wallet_key(&key(8), &[1])];
    let result = verify(&answer(&key(7), &[1], &[0xab; 32], UV), &[0xab; 32], &keys);
    assert_eq!(result, Err(ClearSignerError::BadSignature));
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
        Err(ClearSignerError::BadSignature)
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
    assert_eq!(result, Err(ClearSignerError::NotVerified));
}

#[test]
fn a_malformed_answer_is_refused_not_trusted() {
    let keys = vec![wallet_key(&key(7), &[1])];
    assert!(matches!(
        verify(&json!({}), &[0; 32], &keys),
        Err(ClearSignerError::Malformed(_))
    ));
    let mut bad = answer(&key(7), &[1], &[0; 32], UV);
    bad["signature"] = json!("0xnothex");
    assert!(matches!(
        verify(&bad, &[0; 32], &keys),
        Err(ClearSignerError::Malformed(_))
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
        user_op: Some(&op),
        calls: &[vela_core::user_op::MultiSendCall {
            to: SAFE.into(),
            value_hex: "0x1".into(),
            data: vec![],
        }],
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
        clear_signer::user_op_from_json(&operation["userOp"]),
        Some(op)
    );
    let mut hex_gas = operation["userOp"].clone();
    hex_gas["callGasLimit"] = json!("0x30d40");
    assert_eq!(
        clear_signer::user_op_from_json(&hex_gas).map(|o| o.call_gas_limit),
        Some(200_000)
    );
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
    let url = url_launch(
        "https://sign.getvela.app",
        &built,
        "http://127.0.0.1:51234/vela",
        "tok en",
    );
    assert!(url.starts_with("https://sign.getvela.app/sign.html?ch=url#i="));
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
        Err(ClearSignerError::WrongToken)
    );
    assert_eq!(
        parse_callback("t=tok%20en&error=user_rejected", "tok en"),
        Err(ClearSignerError::Declined)
    );
    assert_eq!(
        parse_callback("t=tok%20en&error=refused", "tok en"),
        Err(ClearSignerError::Refused("refused".into()))
    );
}

#[test]
fn the_websocket_channel_speaks_the_protocol() {
    assert_eq!(
        ws_launch("https://sign.getvela.app/", 51_234, "abc"),
        "https://sign.getvela.app/sign.html?ch=ws#p=51234&t=abc"
    );
    assert_eq!(
        ws_launch("http://localhost:8099/sign.html", 1, "abc"),
        "http://localhost:8099/sign.html?ch=ws#p=1&t=abc"
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
            error: ClearSignerError::Declined
        }
    );
    assert_eq!(parse_message("nonsense"), Message::Ignored);
    assert_eq!(
        clear_signer::DEFAULT_SIGNER_URL,
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
    assert!(done.close);
    assert_eq!(
        done.write[0], 0x8a,
        "the ping was answered with a pong first"
    );
    assert_eq!(
        c.closed(),
        None,
        "an answered conversation has nothing more to say"
    );
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
    assert_eq!(c.closed(), Some(ClearSignerError::Declined));

    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let close = c.feed(&client_frame(0x8, true, &1001u16.to_be_bytes()));
    assert_eq!(close.outcome, Some(Err(ClearSignerError::Declined)));
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
        Some(Err(ClearSignerError::Refused("unlimited_approval".into())))
    );
    let mut c = connection();
    c.feed(&upgrade("https://sign.getvela.app"));
    c.feed(&text(r#"{"v":1,"t":"hello","token":"tok"}"#));
    let step = c.feed(&text(
        r#"{"v":1,"t":"error","id":"r1","code":"user_rejected"}"#,
    ));
    assert_eq!(step.outcome, Some(Err(ClearSignerError::Declined)));
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
        Some(Err(ClearSignerError::Malformed(_)))
    ));
}

#[test]
fn a_signer_address_is_https_or_this_devices_own_loopback() {
    use vela_core::clear_signer::{signer_url, uses_wallet_passkeys, SignerUrlError};
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
    assert_eq!(
        signer_url("http://127.0.0.1:8140/sign.html").as_deref(),
        Ok("http://127.0.0.1:8140/sign.html")
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

    assert!(uses_wallet_passkeys("https://sign.getvela.app/"));
    assert!(uses_wallet_passkeys("https://getvela.app/sign/"));
    assert!(!uses_wallet_passkeys("https://getvela.app.evil.example/"));
    assert!(!uses_wallet_passkeys("http://localhost:8140/"));
    assert!(!uses_wallet_passkeys("https://me.example/"));
}

#[test]
fn the_loopback_callback_is_found_in_the_browsers_request() {
    use vela_core::clear_signer::callback_query;
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
