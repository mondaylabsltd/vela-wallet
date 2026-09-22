//! Spec 075: the relay/BLE session, pinned for BOTH implementations — this
//! crate's `clear_signer::secure` and the page's `lib/transport/secure.js`
//! read the same `tests/clear-signer/secure-session.json`.
//!
//! Regenerate with `VELA_WRITE_VECTORS=1 cargo test -p vela-core --test secure_session`;
//! the page's sample test then must still pass unchanged.

use serde_json::{json, Value};
use vela_core::clear_signer::secure::{key_fingerprint, Handshake, Label, Role, Tail};

const PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/clear-signer/secure-session.json"
);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn case(label: Label, name: &str, seeds: (u8, u8, u8, u8)) -> Value {
    let signer_secret = [seeds.0; 32];
    let requester_secret = [seeds.1; 32];
    let signer_nonce = [seeds.2; 16];
    let requester_nonce = [seeds.3; 16];
    let signer = Handshake::new(&signer_secret, signer_nonce, Role::Signer).unwrap();
    let requester = Handshake::new(&requester_secret, requester_nonce, Role::Requester).unwrap();
    let signer_hello = signer.hello(None);
    let requester_hello = requester.hello(Some("vela-test/1"));
    let rk = key_fingerprint(requester.public_key());
    let signer_pk = hex(signer.public_key());
    let requester_pk = hex(requester.public_key());
    let mut s = signer.complete(&requester_hello, label, Some(&rk)).unwrap();
    let mut r = requester.complete(&signer_hello, label, None).unwrap();
    assert_eq!(s.code(), r.code());

    let tail = |i: u8| match label {
        Label::Relay => Tail::Counter,
        Label::Ble => Tail::MsgId(i),
    };
    let script: [(&str, &str); 4] = [
        (
            "requester",
            r#"{"v":1,"t":"intent","n":1,"id":"e6f3","intent":{"method":"vela_signIn","params":[{}],"origin":""}}"#,
        ),
        ("signer", r#"{"v":1,"t":"result","n":2,"id":"e6f3"}"#),
        ("requester", r#"{"v":1,"t":"intent","n":3,"id":"a1b2"}"#),
        ("signer", r#"{"v":1,"t":"bye","n":4,"reason":"done"}"#),
    ];
    let messages: Vec<Value> = script
        .iter()
        .enumerate()
        .map(|(i, (from, text))| {
            let msg_id = u8::try_from(i).unwrap() + 7;
            let sealed = if *from == "signer" {
                s.seal(text.as_bytes(), tail(msg_id))
            } else {
                r.seal(text.as_bytes(), tail(msg_id))
            };
            let opened = if *from == "signer" {
                r.open(&sealed, tail(msg_id)).unwrap()
            } else {
                s.open(&sealed, tail(msg_id)).unwrap()
            };
            assert_eq!(opened, text.as_bytes());
            let mut m = json!({ "from": from, "plaintext": text, "sealedHex": hex(&sealed) });
            if label == Label::Ble {
                m["msgId"] = json!(msg_id);
            }
            m
        })
        .collect();

    json!({
        "name": name,
        "label": label.as_str(),
        "signer": { "secretHex": hex(&signer_secret), "nonceHex": hex(&signer_nonce), "publicKeyHex": signer_pk, "hello": signer_hello },
        "requester": { "secretHex": hex(&requester_secret), "nonceHex": hex(&requester_nonce), "publicKeyHex": requester_pk, "hello": requester_hello, "app": "vela-test/1" },
        "rk": rk,
        "code": s.code(),
        "messages": messages,
    })
}

fn vectors() -> Value {
    json!({
        "about": "Spec 075: the Clear Signer's end-to-end session (relay.md §2, PROTOCOL.md §3). Hellos compare as JSON objects, not text.",
        "cases": [
            case(Label::Relay, "relay-a", (0x11, 0x22, 0xa1, 0xb2)),
            case(Label::Relay, "relay-b", (0x5a, 0x6b, 0x01, 0xfe)),
            case(Label::Ble, "ble-a", (0x11, 0x22, 0xa1, 0xb2)),
        ],
    })
}

#[test]
fn the_vectors_are_current() {
    let fresh = serde_json::to_string_pretty(&vectors()).unwrap() + "\n";
    if std::env::var("VELA_WRITE_VECTORS").is_ok() {
        std::fs::write(PATH, &fresh).unwrap();
    }
    let committed = std::fs::read_to_string(PATH).unwrap_or_default();
    assert_eq!(
        committed, fresh,
        "run with VELA_WRITE_VECTORS=1 to regenerate {PATH}"
    );
}
