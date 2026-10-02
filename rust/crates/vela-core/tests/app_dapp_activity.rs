//! Spec 093 — what a dApp interaction was, as Activity states it.
//!
//! The summary is read ONCE, at approve time, from the request's full final
//! params by the sheet's own readers; the record keeps it beside a clipped
//! copy of the request, and the feed words the row from it. These tests pin
//! the reading (`dapp_activity::summarize`), the record the signing machine
//! writes, and the row the feed makes of that record.

#![cfg(feature = "crux")]

mod support;

use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::activity_feed::{
    ActivityFeed, Event as FeedEvent, FeedFact, FeedLine, FeedOperation as FeedOp, FeedRow,
    FeedShellResult as FeedRes, FeedTxRecord,
};
use vela_core::app::approval_guard::GuardTokenMetaView;
use vela_core::app::clear_signing::ClearTerm;
use vela_core::app::dapp_activity::{
    stored_request, summarize, DappAction, DappSummary, STORED_REQUEST_MAX_BYTES,
};
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignOperation as Op, SignRecord, SignRecordKind,
    SignRequest, SignShellResult as Res, SignSubmitOutcome,
};

const ACCT: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ROUTER: &str = "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad";
const PERMIT2: &str = "0x000000000022d473030f116ddee9f6b43ac78ba3";
const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const TOKEN: &str = "0x2222222222222222222222222222222222222222";
const UNISWAP: &str = "https://app.uniswap.org";
const MAX_U160: &str = "1461501637330902918203684832716283019655932542975";

fn word(hexish: &str) -> String {
    format!("{:0>64}", hexish.trim_start_matches("0x"))
}

fn approve_data(spender: &str, amount_hex: &str) -> String {
    format!("0x095ea7b3{}{}", word(spender), word(amount_hex))
}

fn permit_single(amount: &str, expiration: u64) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "PermitSingle": [
                {"name": "details", "type": "PermitDetails"},
                {"name": "spender", "type": "address"},
                {"name": "sigDeadline", "type": "uint256"}
            ],
            "PermitDetails": [
                {"name": "token", "type": "address"},
                {"name": "amount", "type": "uint160"},
                {"name": "expiration", "type": "uint48"},
                {"name": "nonce", "type": "uint48"}
            ]
        },
        "primaryType": "PermitSingle",
        "domain": {"name": "Permit2", "chainId": 1, "verifyingContract": PERMIT2},
        "message": {
            "details": {"token": USDC, "amount": amount, "expiration": expiration, "nonce": 0},
            "spender": ROUTER,
            "sigDeadline": 1_767_225_600u64
        }
    })
}

fn erc2612(value: &str, deadline: u64) -> Value {
    json!({
        "types": {
            "EIP712Domain": [
                {"name": "name", "type": "string"},
                {"name": "chainId", "type": "uint256"},
                {"name": "verifyingContract", "type": "address"}
            ],
            "Permit": [
                {"name": "owner", "type": "address"},
                {"name": "spender", "type": "address"},
                {"name": "value", "type": "uint256"},
                {"name": "nonce", "type": "uint256"},
                {"name": "deadline", "type": "uint256"}
            ]
        },
        "primaryType": "Permit",
        "domain": {"name": "Token", "chainId": 1, "verifyingContract": TOKEN},
        "message": {
            "owner": ACCT, "spender": ROUTER, "value": value, "nonce": 0, "deadline": deadline
        }
    })
}

fn typed_params(document: &Value) -> Value {
    json!([ACCT, document.to_string()])
}

fn hex_text(text: &str) -> String {
    format!("0x{}", hex_of(text.as_bytes()))
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn siwe(domain: &str) -> String {
    format!(
        "{domain} wants you to sign in with your Ethereum account:\n{ACCT}\n\nSign in to the app.\n\nURI: https://{domain}\nVersion: 1\nChain ID: 1\nNonce: 32891756\nIssued At: 2026-10-02T00:00:00Z"
    )
}

fn resolved(symbol: &str, decimals: u32) -> GuardTokenMetaView {
    GuardTokenMetaView {
        symbol: symbol.to_owned(),
        decimals,
        verified: true,
        loading: false,
    }
}

// ---------------------------------------------------------------------------
// The reading
// ---------------------------------------------------------------------------

/// Permit2's unlimited permit: who, which token (named by the built-in table
/// when the sheet had nothing), no limit, and no date — the `2^48-1`
/// expiration is "never".
#[test]
fn an_unlimited_permit2_permit_is_read_whole() {
    let params = typed_params(&permit_single(MAX_U160, (1 << 48) - 1));
    let summary = summarize("eth_signTypedData_v4", &params, 1, UNISWAP, None);
    assert_eq!(
        summary,
        DappSummary {
            action: DappAction::Permit,
            contract: Some(PERMIT2.to_owned()),
            spender: Some(ROUTER.to_owned()),
            token: Some(USDC.to_owned()),
            symbol: Some("USDC".to_owned()),
            decimals: Some(6),
            unlimited: true,
            primary_type: Some("PermitSingle".to_owned()),
            ..DappSummary::default()
        }
    );
}

/// An ERC-2612 permit for a finite amount: the amount, the deadline, and the
/// token as the approval surface resolved it — only once it resolved.
#[test]
fn a_limited_erc2612_permit_keeps_its_amount_and_the_sheets_token() {
    let params = typed_params(&erc2612("100000000", 1_767_225_600));
    let summary = summarize(
        "eth_signTypedData_v4",
        &params,
        1,
        UNISWAP,
        Some(&resolved("TKN", 6)),
    );
    assert_eq!(summary.action, DappAction::Permit);
    assert_eq!(summary.amount.as_deref(), Some("100000000"));
    assert!(!summary.unlimited);
    assert_eq!(
        (summary.symbol.as_deref(), summary.decimals),
        (Some("TKN"), Some(6))
    );
    assert_eq!(summary.expires_at, Some(1_767_225_600.0));
    assert_eq!(summary.contract.as_deref(), Some(TOKEN));

    let unresolved = GuardTokenMetaView {
        loading: true,
        ..resolved("…", 18)
    };
    let summary = summarize(
        "eth_signTypedData_v4",
        &params,
        1,
        UNISWAP,
        Some(&unresolved),
    );
    assert_eq!(
        (summary.symbol, summary.decimals),
        (None, None),
        "a token the sheet was still reading is not named"
    );
}

/// Typed data that grants nothing is structured data: its type and contract.
#[test]
fn typed_data_that_grants_nothing_is_structured_data() {
    let mail = json!({
        "types": {
            "EIP712Domain": [{"name": "chainId", "type": "uint256"}],
            "Mail": [{"name": "contents", "type": "string"}]
        },
        "primaryType": "Mail",
        "domain": {"chainId": 1},
        "message": {"contents": "hello"}
    });
    let summary = summarize(
        "eth_signTypedData_v4",
        &typed_params(&mail),
        1,
        UNISWAP,
        None,
    );
    assert_eq!(summary.action, DappAction::TypedData);
    assert_eq!(summary.primary_type.as_deref(), Some("Mail"));
    assert_eq!(summary.spender, None);
}

/// A Sign-In with Ethereum message is a sign-in only for the site that
/// asked; the same message from another site is a message — the row never
/// says "Sign in on evil.example" for somebody else's domain.
#[test]
fn a_sign_in_must_match_the_site() {
    let params = json!([hex_text(&siwe("app.uniswap.org")), ACCT]);
    let matched = summarize("personal_sign", &params, 1, UNISWAP, None);
    assert_eq!(matched.action, DappAction::SignIn);
    assert_eq!(matched.signin_domain.as_deref(), Some("app.uniswap.org"));

    let mismatched = summarize("personal_sign", &params, 1, "https://evil.example", None);
    assert_eq!(mismatched.action, DappAction::Message);
    assert_eq!(mismatched.signin_domain, None);

    let plain = summarize(
        "personal_sign",
        &json!([hex_text("hello"), ACCT]),
        1,
        UNISWAP,
        None,
    );
    assert_eq!(plain.action, DappAction::Message);
    assert_eq!(
        summarize("eth_sign", &json!([ACCT, "0x12"]), 1, UNISWAP, None).action,
        DappAction::BlindSign
    );
}

/// On-chain approvals: unlimited, a revoke, and a decrease — which lowers an
/// allowance and grants nothing, so it is a call.
#[test]
fn on_chain_approvals_grant_revoke_or_are_calls() {
    let tx = |data: String| json!([{ "to": USDC, "data": data, "value": "0x0" }]);
    let unlimited = summarize(
        "eth_sendTransaction",
        &tx(approve_data(ROUTER, &"f".repeat(64))),
        1,
        UNISWAP,
        None,
    );
    assert_eq!(unlimited.action, DappAction::Approve);
    assert!(unlimited.unlimited && !unlimited.revoke);
    assert_eq!(unlimited.amount, None);
    assert_eq!(unlimited.contract.as_deref(), Some(USDC));
    assert_eq!(unlimited.spender.as_deref(), Some(ROUTER));
    assert_eq!(unlimited.symbol.as_deref(), Some("USDC"));

    let revoke = summarize(
        "eth_sendTransaction",
        &tx(approve_data(ROUTER, "0")),
        1,
        UNISWAP,
        None,
    );
    assert_eq!(revoke.action, DappAction::Approve);
    assert!(revoke.revoke);

    let decrease = format!("0xa457c2d7{}{}", word(ROUTER), word("10"));
    let call = summarize("eth_sendTransaction", &tx(decrease), 1, UNISWAP, None);
    assert_eq!(call.action, DappAction::Call);
    assert_eq!(call.calls, 1);
}

/// A batch: how many calls, where (the first call to a protocol the wallet
/// knows), and the allowance a call granted. One call is that call.
#[test]
fn a_batch_names_its_calls_its_place_and_its_grant() {
    let params = json!([{
        "version": "2.0.0",
        "chainId": "0x1",
        "calls": [
            { "to": USDC, "data": approve_data(ROUTER, &"f".repeat(64)) },
            { "to": ROUTER, "data": "0x3593564c", "value": "0x0" },
        ]
    }]);
    let summary = summarize("wallet_sendCalls", &params, 1, UNISWAP, None);
    assert_eq!(summary.action, DappAction::Batch);
    assert_eq!(summary.calls, 2);
    assert_eq!(summary.contract.as_deref(), Some(ROUTER));
    assert_eq!(summary.spender.as_deref(), Some(ROUTER));
    assert!(summary.unlimited);

    let one = json!([{ "calls": [{ "to": ROUTER, "data": "0x3593564c" }] }]);
    let summary = summarize("wallet_sendCalls", &one, 1, UNISWAP, None);
    assert_eq!((summary.action, summary.calls), (DappAction::Call, 1));
}

// ---------------------------------------------------------------------------
// The record the signing machine writes
// ---------------------------------------------------------------------------

type Sign = DomainDriver<SignRequest>;

fn signer() -> Sign {
    let mut sut = Sign::new();
    sut.dispatch(Event::NetworksChanged { chain_ids: vec![1] });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: ACCT.to_owned(),
            credential_id: "cred".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

fn arrive(sut: &mut Sign, method: &str, params: &Value) {
    sut.dispatch(Event::RequestArrived {
        id: "req-1".to_owned(),
        method: method.to_owned(),
        params_json: params.to_string(),
        origin: UNISWAP.to_owned(),
        transport_id: "wp".to_owned(),
        dedicated_transport: false,
        per_request_chain: None,
        dapp: Some(vela_core::app::sign_request::SignDappIdentity {
            name: "Totally Uniswap".to_owned(),
            url: Some(UNISWAP.to_owned()),
        }),
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: 1_000.0,
    });
}

/// A signature's record: approved, signed, persisted — what the disk gets.
fn signature_record(method: &str, params: &Value, opts: SignApproveOpts) -> SignRecord {
    let mut sut = signer();
    arrive(&mut sut, method, params);
    sut.dispatch(Event::ApproveTapped { opts });
    let ops = sut.resolve(Res::Submit {
        outcome: SignSubmitOutcome::Succeeded {
            result: format!("0x{}", "5a".repeat(65)),
        },
        now_ms: 2_000.0,
    });
    match ops.as_slice() {
        [Op::PersistRecord { record }] => record.clone(),
        other => panic!("expected the record first: {other:?}"),
    }
}

/// The summary is read from the WHOLE request, even one far past what the
/// record keeps: a permit carrying 20 KB of extra text is still read as the
/// permit it is, while the stored copy stays within 8 KB and says it was cut.
/// The signature itself is the page's, never the disk's.
#[test]
fn a_record_reads_the_whole_request_and_keeps_a_clipped_copy() {
    let mut document = permit_single(MAX_U160, (1 << 48) - 1);
    document["domain"]["name"] = json!(format!("Permit2{}", " ".repeat(20_000)));
    let params = typed_params(&document);
    assert!(params.to_string().len() > 2 * STORED_REQUEST_MAX_BYTES);

    let record = signature_record(
        "eth_signTypedData_v4",
        &params,
        SignApproveOpts {
            token_meta: Some(resolved("USDC", 6)),
            ..SignApproveOpts::default()
        },
    );
    assert_eq!(record.kind, SignRecordKind::SignTypedData);
    assert!(
        record.result.is_empty(),
        "the signature stays with the page"
    );
    let summary = record.summary.expect("a summary");
    assert_eq!(summary.action, DappAction::Permit);
    assert!(summary.unlimited);
    assert_eq!(summary.spender.as_deref(), Some(ROUTER));
    assert!(record.request_truncated);
    assert!(record.stored_request.len() <= STORED_REQUEST_MAX_BYTES);
    assert_eq!(
        record.params_json,
        params.to_string(),
        "the request itself is whole"
    );
}

/// A request past the old phones' 4 KB but within 8 KB is kept verbatim.
#[test]
fn a_request_within_eight_kilobytes_is_kept_whole() {
    let message = hex_text(&"x".repeat(2_500));
    let params = json!([message, ACCT]);
    assert!(params.to_string().len() > 4_096);
    let record = signature_record("personal_sign", &params, SignApproveOpts::default());
    assert!(!record.request_truncated);
    assert_eq!(record.stored_request, params.to_string());
    assert!(!stored_request(&record.stored_request).1);
}

/// A sign-in is matched against the origin the request came from — not the
/// name the dApp gave itself.
#[test]
fn the_record_matches_a_sign_in_to_the_origin() {
    let params = json!([hex_text(&siwe("app.uniswap.org")), ACCT]);
    let record = signature_record("personal_sign", &params, SignApproveOpts::default());
    assert_eq!(record.dapp_url, UNISWAP);
    let summary = record.summary.expect("a summary");
    assert_eq!(summary.action, DappAction::SignIn);
}

/// A transaction's pending record carries the summary too — from the FINAL
/// params: the cap the person chose, not the site's unlimited ask.
#[test]
fn a_transaction_record_summarizes_the_final_params() {
    let asked =
        json!([{ "to": USDC, "data": approve_data(ROUTER, &"f".repeat(64)), "value": "0x0" }]);
    let capped = json!([{ "to": USDC, "data": approve_data(ROUTER, "5f5e100"), "value": "0x0" }]);
    let mut sut = signer();
    arrive(&mut sut, "eth_sendTransaction", &asked);
    sut.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts {
            params_override_json: Some(capped.to_string()),
            token_meta: Some(resolved("USDC", 6)),
            ..SignApproveOpts::default()
        },
    });
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "req-1".to_owned(),
        user_op_hash: "0xop".to_owned(),
        now_ms: 5_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    let record = ops
        .iter()
        .find_map(|op| match op {
            Op::PersistRecord { record } => Some(record.clone()),
            _ => None,
        })
        .expect("the pending record");
    let summary = record.summary.expect("a summary");
    assert_eq!(summary.action, DappAction::Approve);
    assert!(!summary.unlimited);
    assert_eq!(summary.amount.as_deref(), Some("100000000"));
    assert_eq!(record.stored_request, capped.to_string());
}

// ---------------------------------------------------------------------------
// The row the feed makes of that record
// ---------------------------------------------------------------------------

fn rewire<T: Serialize, U: DeserializeOwned>(value: &T) -> U {
    serde_json::from_value(serde_json::to_value(value).expect("serialises"))
        .expect("the other module reads the same word")
}

/// The record as a shell stores it and maps it back: the summary verbatim.
fn stored(record: &SignRecord) -> FeedTxRecord {
    let timestamp = (record.now_ms / 1_000.0).floor();
    serde_json::from_value(json!({
        "id": record.record_id,
        "user_op_hash": record.user_op_hash,
        "tx_hash": record.result,
        "from": record.from,
        "to": "",
        "to_name": null,
        "value": "0",
        "symbol": "",
        "decimals": 0,
        "logo_urls": null,
        "chain_id": record.chain_id,
        "timestamp": timestamp,
        "day_start_ms": 0.0,
        "status": rewire::<_, Value>(&record.status),
        "kind": rewire::<_, Value>(&record.kind),
        "usd": null,
        "dapp_url": record.dapp_url,
        "summary": record.summary,
    }))
    .expect("a feed record")
}

fn rows_of(records: Vec<FeedTxRecord>) -> Vec<vela_core::app::activity_feed::FeedItem> {
    let mut sut = DomainDriver::<ActivityFeed>::new();
    sut.dispatch(FeedEvent::AccountSwitched {
        address: ACCT.to_owned(),
    });
    let read_id = sut
        .outstanding()
        .iter()
        .find_map(|op| match op {
            FeedOp::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a read");
    sut.resolve(FeedRes::StoreLoaded {
        records,
        now_ms: 10_000.0,
        read_id,
    });
    sut.view()
        .rows
        .into_iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item),
            FeedRow::Header { .. } => None,
        })
        .collect()
}

/// End to end: a permit signed on app.uniswap.org reads "Spending permit on
/// Uniswap", unlimited, the site on the second line — and a sign-in "Sign in
/// on app.uniswap.org", the network alone beneath it.
#[test]
fn a_signed_permit_and_sign_in_read_as_rows() {
    let permit = signature_record(
        "eth_signTypedData_v4",
        &typed_params(&permit_single(MAX_U160, (1 << 48) - 1)),
        SignApproveOpts::default(),
    );
    let signin = signature_record(
        "personal_sign",
        &json!([hex_text(&siwe("app.uniswap.org")), ACCT]),
        SignApproveOpts::default(),
    );
    let mut later = stored(&signin);
    later.timestamp += 60.0;
    let rows = rows_of(vec![stored(&permit), later]);
    assert_eq!(rows.len(), 2);

    let signin = rows[0].dapp.clone().expect("a dApp row");
    assert_eq!(signin.intent_term, Some(ClearTerm::SignInIntent));
    assert_eq!(signin.place.as_deref(), Some("app.uniswap.org"));
    assert_eq!(rows[0].subtitle, vec![FeedLine::Network { chain_id: 1 }]);

    let permit = rows[1].dapp.clone().expect("a dApp row");
    assert_eq!(permit.intent_term, Some(ClearTerm::PermitIntent));
    assert_eq!(permit.place.as_deref(), Some("Uniswap"));
    assert!(permit.allowance.as_ref().is_some_and(|a| a.unlimited));
    assert!(permit.off_chain);
    assert_eq!(
        rows[1].subtitle,
        vec![
            FeedLine::Site {
                site: "app.uniswap.org".to_owned()
            },
            FeedLine::Network { chain_id: 1 },
        ]
    );
    assert!(permit
        .facts
        .iter()
        .any(|fact| matches!(fact, FeedFact::Spender { .. })));
    assert!(permit.facts.len() <= 6);
}
