//! Spec 071/075: the BLE framing the page and the peripherals must agree on,
//! byte for byte (`app-web/clearsigning/PROTOCOL.md` §2).
//!
//! The page ([`lib/transport/ble.js`]) is the central side of every BLE
//! session and three shells run the peripheral side. These vectors are the
//! only thing standing between them and four dialects of the same header, so
//! they are read from BOTH sides: here, and by
//! `app-web/clearsigning/samples/ble-vectors-test.mjs`, which reassembles them
//! with the page's own code.
//!
//! Regenerate deliberately: `VELA_WRITE_VECTORS=1 cargo test -p vela-core
//! --test ble_frames`. A diff in `tests/clear-signer/ble-frames.json` is a
//! wire change and needs the page to change with it.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use serde_json::{json, Value};
use vela_core::clear_signer::ble::{Framer, Reassembler, FLAG_SEALED, MIN_CHUNK};

fn vectors_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/clear-signer/ble-frames.json")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).expect("hex"))
        .collect()
}

/// The cases, as (name, chunk, flags, msg_id, payload).
fn cases() -> Vec<(&'static str, usize, u8, u8, Vec<u8>)> {
    vec![
        ("empty-plaintext", 244, 0, 9, Vec::new()),
        (
            "one-frame",
            244,
            FLAG_SEALED,
            1,
            b"{\"v\":1,\"t\":\"hello\"}".to_vec(),
        ),
        ("exactly-one-chunk", 244, FLAG_SEALED, 2, vec![0x5a; 244]),
        ("one-byte-over", 244, FLAG_SEALED, 3, vec![0x5a; 245]),
        (
            "a-batch-intent",
            244,
            FLAG_SEALED,
            42,
            (0..2_500u32).map(|n| (n % 251) as u8).collect(),
        ),
        ("at-the-floor", MIN_CHUNK, FLAG_SEALED, 255, vec![0xa5; 97]),
    ]
}

fn framer_at(chunk: usize) -> Framer {
    let mut framer = Framer::new();
    while framer.chunk() > chunk {
        assert!(framer.halve(), "cannot reach {chunk}");
    }
    assert_eq!(framer.chunk(), chunk);
    framer
}

/// The vectors as this crate frames them today.
fn current() -> Value {
    let built: Vec<Value> = cases()
        .into_iter()
        .map(|(name, chunk, flags, msg_id, payload)| {
            let framer = framer_at(chunk);
            let frames = framer.frames(flags, msg_id, &payload);
            json!({
                "name": name,
                "chunk": chunk,
                "flags": flags,
                "msgId": msg_id,
                "payload": hex(&payload),
                "frames": frames.iter().map(|f| hex(f)).collect::<Vec<_>>(),
            })
        })
        .collect();
    json!({
        "note": "PROTOCOL.md §2 — flags, msgId, seq(BE), total(BE), payload",
        "cases": built,
    })
}

/// Rewrite the file when asked — from either test, since both read it and the
/// two run in whatever order the harness likes.
fn ensure_written() {
    if std::env::var("VELA_WRITE_VECTORS").is_ok() {
        std::fs::write(
            vectors_path(),
            format!("{}\n", serde_json::to_string_pretty(&current()).unwrap()),
        )
        .unwrap();
    }
}

#[test]
fn the_frames_are_what_the_page_reads() {
    ensure_written();
    let current = current();
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(vectors_path()).expect("vectors")).unwrap();
    assert_eq!(
        recorded, current,
        "the framing changed — the page frames it the other way until it is changed too"
    );
}

#[test]
fn every_vector_reassembles_back_to_its_payload() {
    ensure_written();
    let recorded: Value =
        serde_json::from_str(&std::fs::read_to_string(vectors_path()).expect("vectors")).unwrap();
    for case in recorded["cases"].as_array().expect("cases") {
        let mut reassembler = Reassembler::new();
        let frames = case["frames"].as_array().expect("frames");
        let mut whole = None;
        for frame in frames {
            whole = reassembler.accept(&unhex(frame.as_str().expect("hex")), 0);
        }
        let whole = whole.expect("the last frame completes it");
        let name = case["name"].as_str().unwrap_or_default();
        assert_eq!(hex(&whole.payload), case["payload"], "{name}");
        assert_eq!(
            u64::from(whole.msg_id),
            case["msgId"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            u64::from(whole.flags),
            case["flags"].as_u64().unwrap(),
            "{name}"
        );
        assert_eq!(
            reassembler.pending(),
            0,
            "{name}: nothing left half-arrived"
        );
    }
}
