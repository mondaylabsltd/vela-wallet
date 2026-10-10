//! Spec 097 part B — Activity says what happened, with the figures the chain
//! proved.
//!
//! Every fixture under `tests/fixtures/dapp097/` is real: `req-*.json` is the
//! request a dApp sent the extension in the second real-money pass (BNB
//! Chain, the golden Safe `0x88cC…6894`, 2026-10-02), and `receipt-*.json` is
//! the bundle transaction it became, read from `bsc-dataseed.bnbchain.org`
//! and trimmed to the fields a receipt reader uses.
//!
//! The chain these tests walk is the one a shell runs: the signing machine
//! approves the real request and writes its record; the tracker reads the
//! real receipt and patches the record; the feed draws the row. Pinned:
//! - N5: a landed call shows what its OWN receipt proves moved — a borrow's
//!   "+0.3 USDC", a USDC→BNB swap's both legs — never the sheet's guess; a
//!   "Received" folded into the row still shows its "+".
//! - N4: a failed row says why.
//! - N7: titles are capitalised, places are protocols (Permit2 approvals
//!   take their owner's), a signed `Order` reads as a swap, and an unknown
//!   price is no "≈ $0.00".
//! - N8: the contract the sheet named is named the same in Activity.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::activity_feed::{
    ActivityFeed, Event as FeedEvent, FeedDappChange, FeedDirection, FeedFact, FeedItem,
    FeedOperation as FeedOp, FeedRow, FeedShellResult as FeedRes, FeedTxKind, FeedTxRecord,
    FeedTxStatus,
};
use vela_core::app::clear_signing::ClearTerm;
use vela_core::app::dapp_activity::{
    summarize, with_approve_facts, DappReading, DappSummary, DappToken,
};
use vela_core::app::sign_request::{
    Event, SignAccountRef, SignApproveOpts, SignOperation as Op, SignQuotedFee, SignRecord,
    SignRequest, SignShellResult as Res, SignSubmitOutcome,
};
use vela_core::app::token_trust::{TrustReceiptLog, TrustSimJudgment};
use vela_core::app::tx_tracker::{
    proven_moves, Event as TrackEvent, TrackFailure, TrackLifecycle, TrackMove,
    TrackOperation as TOp, TrackRecordPatch, TrackRecordStatus, TrackSettlement,
    TrackShellResult as TRes, TxTracker, SAFE_RECEIVED_TOPIC,
};

const SAFE: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
const USDC: &str = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d";
const USDT: &str = "0x55d398326f99059ff775485246999027b3197955";
const A_BNB_WBNB: &str = "0x9b00a09492a626678e5a3009982191586c444df9";
const DEBT_USDC: &str = "0xcdbbed5606d9c5c98eeedd67933991dc17f0c68d";
const AAVE_POOL: &str = "0x6807dc923806fe8fd134338eabca509979a7e0cb";
const AAVE_GATEWAY: &str = "0x0c2c95b24529664fe55d4437d7a31175cfe6c4f7";
const NATIVE_ORDER_FACTORY: &str = "0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01";
const CURVE_ROUTER: &str = "0xa72c85c258a81761433b4e8da60505fe3dd551cc";
const BSC: u32 = 56;
const NOW: f64 = 1_790_958_668_000.0;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/dapp097/{name}.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

/// A receipt's logs as the tracker is handed them.
fn logs_of(receipt: &Value) -> Vec<TrustReceiptLog> {
    receipt["logs"]
        .as_array()
        .expect("logs")
        .iter()
        .map(|log| TrustReceiptLog {
            address: text(&log["address"]),
            topics: log["topics"]
                .as_array()
                .expect("topics")
                .iter()
                .map(text)
                .collect(),
            data: text(&log["data"]),
        })
        .collect()
}

/// The op hash the receipt's own `UserOperationEvent` names.
fn op_hash_of(receipt: &Value) -> String {
    receipt["logs"]
        .as_array()
        .expect("logs")
        .iter()
        .find(|log| text(&log["topics"][0]).starts_with("0x49628fd1"))
        .map(|log| text(&log["topics"][1]))
        .expect("the op's event")
}

fn moved(token: Option<&str>, delta: &str) -> TrackMove {
    TrackMove {
        token: token.map(str::to_owned),
        delta: delta.to_owned(),
    }
}

// ---------------------------------------------------------------------------
// What each receipt proves (the tracker's reader)
// ---------------------------------------------------------------------------

/// Every landed operation of the pass, read from its own receipt: the Safe's
/// net movement per coin. The coin arriving is the Safe's own `SafeReceived`;
/// the coin the call sent is no log's, so a native-in swap and a native-out
/// one differ exactly as the chain recorded them.
#[test]
fn the_receipt_proves_what_each_operation_of_the_pass_moved() {
    let read = |name: &str| {
        let receipt = fixture(&format!("receipt-{name}"));
        proven_moves(&logs_of(&receipt), &op_hash_of(&receipt)).expect("the op's event")
    };
    assert_eq!(
        read("aave-borrow"),
        vec![
            moved(Some(DEBT_USDC), "300000000000000001"),
            moved(Some(USDC), "300000000000000000"),
        ],
        "a borrow brings the debt token and the USDC in"
    );
    assert_eq!(
        read("aave-repay"),
        vec![
            moved(Some(DEBT_USDC), "-300000000000000001"),
            moved(Some(USDC), "-300000036827585322"),
        ]
    );
    assert_eq!(
        read("pcs-usdc-bnb"),
        vec![
            moved(Some(USDC), "-1160000000000000000"),
            moved(None, "1499036349071560"),
        ],
        "USDC out, BNB in — the BNB is the Safe's own SafeReceived"
    );
    assert_eq!(
        read("uni-usdc-bnb"),
        vec![
            moved(Some(USDC), "-1160000000000000000"),
            moved(None, "1496447240777689"),
        ]
    );
    assert_eq!(
        read("aave-withdraw"),
        vec![
            moved(Some(A_BNB_WBNB), "-2999999999999999"),
            moved(None, "3000000001373923"),
        ],
        "the interest minted first nets into the aToken burnt"
    );
    assert_eq!(
        read("curve-swap"),
        vec![
            moved(Some(USDC), "-100000000000000000"),
            moved(Some(USDT), "100012293602944751"),
        ],
        "the router's own hops are not the Safe's"
    );
    assert_eq!(
        read("pcs-bnb-usdc"),
        vec![moved(Some(USDC), "2319944398956697958")],
        "the BNB it sent is the call's value, not a log"
    );
    assert_eq!(
        read("aave-supply"),
        vec![moved(Some(A_BNB_WBNB), "2999999999999999")]
    );
    assert!(
        read("1inch-create-order").is_empty(),
        "an order moved nothing but its value"
    );
}

/// No event for the op, no reading — and a neighbour's op in the same bundle
/// is never ours.
#[test]
fn a_receipt_without_the_ops_event_proves_nothing() {
    let receipt = fixture("receipt-aave-borrow");
    let logs = logs_of(&receipt);
    let other = format!("0x{}", "ab".repeat(32));
    assert_eq!(proven_moves(&logs, &other), None);
    let without_event: Vec<TrustReceiptLog> = logs
        .iter()
        .filter(|log| !log.topics[0].starts_with("0x49628fd1"))
        .cloned()
        .collect();
    assert_eq!(proven_moves(&without_event, &op_hash_of(&receipt)), None);

    // Two ops in one bundle: the second's transfers are not the first's.
    let curve = fixture("receipt-curve-swap");
    let mut bundle = logs.clone();
    bundle.extend(
        logs_of(&curve)
            .into_iter()
            .filter(|log| !log.topics[0].starts_with("0xbb47ee3e")),
    );
    assert_eq!(
        proven_moves(&bundle, &op_hash_of(&receipt)),
        Some(vec![
            moved(Some(DEBT_USDC), "300000000000000001"),
            moved(Some(USDC), "300000000000000000"),
        ])
    );
    assert_eq!(
        proven_moves(&bundle, &op_hash_of(&curve)),
        Some(vec![
            moved(Some(USDC), "-100000000000000000"),
            moved(Some(USDT), "100012293602944751"),
        ])
    );
}

/// The `SafeReceived` topic is the core's own keccak of the Safe event, and
/// only the account itself can write it: another contract's look-alike is
/// not the coin arriving.
#[test]
fn only_the_account_itself_proves_the_coin_arrived() {
    let digest = vela_core::primitives::keccak256(b"SafeReceived(address,uint256)");
    assert_eq!(
        vela_core::primitives::to_hex(&digest, true),
        SAFE_RECEIVED_TOPIC
    );
    let receipt = fixture("receipt-pcs-usdc-bnb");
    let mut logs = logs_of(&receipt);
    for log in &mut logs {
        if log.topics[0].eq_ignore_ascii_case(SAFE_RECEIVED_TOPIC) {
            log.address = "0x1111111111111111111111111111111111111111".to_owned();
        }
    }
    assert_eq!(
        proven_moves(&logs, &op_hash_of(&receipt)),
        Some(vec![moved(Some(USDC), "-1160000000000000000")])
    );
}

// ---------------------------------------------------------------------------
// The chain a shell runs: approve → record → receipt → patch → row
// ---------------------------------------------------------------------------

type Sign = DomainDriver<SignRequest>;

fn signer() -> Sign {
    let mut sut = Sign::new();
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![BSC],
    });
    sut.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: SAFE.to_owned(),
            credential_id: "cred".to_owned(),
        }],
        active_index: 0,
    });
    sut
}

/// The record the signing machine writes for a real request.
fn record_of(name: &str, opts: SignApproveOpts, op_hash: &str) -> SignRecord {
    let request = fixture(&format!("req-{name}"));
    let method = text(&request["method"]);
    let mut sut = signer();
    sut.dispatch(Event::RequestArrived {
        id: "req-1".to_owned(),
        method: method.clone(),
        params_json: request["params"].to_string(),
        origin: text(&request["origin"]),
        transport_id: "ext".to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(BSC),
        dapp: None,
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
        first_party: false,
    });
    let ops = sut.dispatch(Event::ApproveTapped { opts });
    let persisted = |ops: &[Op]| {
        ops.iter().find_map(|op| match op {
            Op::PersistRecord { record } => Some(record.clone()),
            _ => None,
        })
    };
    if method.starts_with("eth_signTypedData") {
        let ops = sut.resolve(Res::Submit {
            outcome: SignSubmitOutcome::Succeeded {
                result: format!("0x{}", "5a".repeat(65)),
            },
            now_ms: NOW + 1_000.0,
        });
        return persisted(&ops).expect("the signature's record");
    }
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::CheckBundlerFunding { .. })),
        "{name}: {ops:?}"
    );
    sut.resolve(Res::PreCheck { funding: None });
    let ops = sut.dispatch(Event::OpSubmitted {
        id: "req-1".to_owned(),
        user_op_hash: op_hash.to_owned(),
        now_ms: NOW + 1_000.0,
        maybe_sent: false,
        submit_block: None,
    });
    persisted(&ops).expect("the pending record")
}

type Track = DomainDriver<TxTracker>;

fn tracking(record: &SignRecord) -> Track {
    let mut sut = Track::new();
    sut.dispatch(TrackEvent::Submitted {
        sender: None,
        user_op_hash: record.user_op_hash.clone(),
        record_ids: vec![record.record_id.clone()],
        chain_id: BSC,
        maybe_sent: false,
        submit_block: None,
        admitted: true,
    });
    sut.resolve_matching(|op| matches!(op, TOp::Now), TRes::Clock { now_ms: NOW });
    sut
}

fn patch_in(ops: &[TOp]) -> TrackRecordPatch {
    ops.iter()
        .find_map(|op| match op {
            TOp::UpdateTxRecords { patch, .. } => Some(patch.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no patch: {ops:?}"))
}

/// The relay's receipt for the op, with its logs — the path every shell's
/// receipt poll takes.
fn landed(record: &SignRecord, receipt: &Value) -> TrackRecordPatch {
    let mut sut = tracking(record);
    let ops = sut.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::ReceiptWithLogs {
            user_op_hash: record.user_op_hash.clone(),
            tx_hash: text(&receipt["transactionHash"]),
            now_ms: NOW + 4_000.0,
            logs: logs_of(receipt),
        },
    );
    patch_in(&ops)
}

/// The record as a shell stores it and maps it back, the patch applied.
fn stored(record: &SignRecord, patch: Option<&TrackRecordPatch>) -> FeedTxRecord {
    let params: Value = serde_json::from_str(&record.params_json).expect("params");
    let tx = &params[0];
    let timestamp = (record.now_ms / 1_000.0).floor();
    let mut row: FeedTxRecord = serde_json::from_value(json!({
        "id": record.record_id,
        "user_op_hash": record.user_op_hash,
        "tx_hash": "",
        "from": record.from,
        "to": tx.get("to").and_then(Value::as_str).unwrap_or_default(),
        "to_name": null,
        "value": tx.get("value").and_then(Value::as_str).unwrap_or("0x0"),
        "symbol": "BNB",
        "decimals": 18,
        "logo_urls": null,
        "chain_id": record.chain_id,
        "timestamp": timestamp,
        "day_start_ms": 0.0,
        "status": serde_json::to_value(record.status).expect("status"),
        "kind": serde_json::to_value(record.kind).expect("kind"),
        "usd": null,
        "dapp_url": record.dapp_url,
        "intent": record.intent,
        "balance_changes": record.balance_changes,
        "call_data": tx.get("data").and_then(Value::as_str),
        "summary": record.summary,
    }))
    .expect("a feed record");
    if let Some(patch) = patch {
        row.status = match patch.status {
            TrackRecordStatus::Confirmed => FeedTxStatus::Confirmed,
            TrackRecordStatus::Failed => FeedTxStatus::Failed,
        };
        if let Some(tx_hash) = &patch.tx_hash {
            row.tx_hash.clone_from(tx_hash);
        }
        row.settlement.clone_from(&patch.settlement);
    }
    row
}

/// The "Received" record the shell's scan writes for a stablecoin that came
/// in — the chain's stables are watched.
fn scanned(id: &str, tx_hash: &str, value: &str, symbol: &str) -> FeedTxRecord {
    serde_json::from_value(json!({
        "id": id,
        "user_op_hash": "",
        "tx_hash": tx_hash,
        "from": AAVE_POOL,
        "to": SAFE,
        "to_name": null,
        "value": value,
        "symbol": symbol,
        "decimals": 18,
        "logo_urls": null,
        "chain_id": BSC,
        "timestamp": NOW / 1_000.0 + 10.0,
        "day_start_ms": 0.0,
        "status": "confirmed",
        "kind": "receive",
        "usd": null,
    }))
    .expect("a receive record")
}

fn rows_of(records: Vec<FeedTxRecord>) -> Vec<FeedItem> {
    let mut sut = DomainDriver::<ActivityFeed>::new();
    sut.dispatch(FeedEvent::AccountSwitched {
        address: SAFE.to_owned(),
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
        now_ms: NOW + 60_000.0,
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

fn line(direction: FeedDirection, value: &str, symbol: &str) -> FeedDappChange {
    FeedDappChange {
        direction,
        verified: true,
        symbol: symbol.to_owned(),
        value: Some(value.to_owned()),
        decimals: Some(18),
        exact: true,
    }
}

fn unverified(direction: FeedDirection) -> FeedDappChange {
    FeedDappChange {
        direction,
        verified: false,
        symbol: String::new(),
        value: None,
        decimals: None,
        exact: true,
    }
}

/// What the sheet's simulation resolved for a token, as the approve carries
/// it (every shell records these since 097).
fn judged(token: &str, delta: &str, symbol: &str, trusted: bool) -> TrustSimJudgment {
    TrustSimJudgment::Erc20Trusted {
        token: token.to_owned(),
        delta: delta.to_owned(),
        symbol: symbol.to_owned(),
        decimals: 18,
        in_trusted_set: trusted,
    }
}

/// N5, the borrow: no coin left, so the row had nothing to draw and the 0.3
/// USDC it brought in was folded away. Now the receipt proves the USDC in:
/// the row reads "+0.3 USDC" on its own, the scan's "Received" stays folded
/// into it, and the debt token — a coin the wallet cannot vouch for — is a
/// detail line with no figure.
#[test]
fn an_aave_borrow_reads_what_it_brought_in() {
    let receipt = fixture("receipt-aave-borrow");
    let record = record_of(
        "aave-borrow",
        SignApproveOpts {
            intent: Some("Borrow".to_owned()),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let patch = landed(&record, &receipt);
    assert_eq!(patch.status, TrackRecordStatus::Confirmed);
    let tx_hash = text(&receipt["transactionHash"]);
    let rows = rows_of(vec![
        stored(&record, Some(&patch)),
        scanned("rx-borrow", &tx_hash, "0.3", "USDC"),
    ]);
    assert_eq!(rows.len(), 1, "the scan's Received folds into the row");
    let row = &rows[0];
    assert_eq!(row.kind, FeedTxKind::DappTx);
    let dapp = row.dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.intent_term, Some(ClearTerm::IntentBorrow));
    assert_eq!(dapp.place.as_deref(), Some("Aave"));
    assert_eq!(row.value, None, "nothing left the account");
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.3", "USDC")),
        "the figure the receipt proved, drawn on its own"
    );
    assert!(!dapp.estimated);
    assert_eq!(
        dapp.changes,
        vec![
            unverified(FeedDirection::In),
            line(FeedDirection::In, "0.3", "USDC")
        ]
    );
    assert!(dapp.facts.contains(&FeedFact::BalanceChanges));
}

/// N5, the swaps that send no coin: USDC out and BNB in, both proven — on
/// PancakeSwap's three-call batch and Uniswap's single call alike. The USDC
/// is named by what the sheet resolved for it; the BNB is the chain's coin.
#[test]
fn a_swap_that_sends_no_coin_reads_both_legs() {
    for (name, back) in [
        ("pcs-usdc-bnb", "0.00149903634907156"),
        ("uni-usdc-bnb", "0.001496447240777689"),
    ] {
        let receipt = fixture(&format!("receipt-{name}"));
        let record = record_of(
            name,
            SignApproveOpts {
                intent: Some("Swap".to_owned()),
                balance_changes: Some(vec![judged(USDC, "-1160000000000000000", "USDC", true)]),
                ..SignApproveOpts::default()
            },
            &op_hash_of(&receipt),
        );
        let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
        let row = &rows[0];
        let dapp = row.dapp.as_ref().expect("a dApp row");
        assert_eq!(
            (row.value.as_deref(), row.symbol.as_str()),
            (Some("1.16"), "USDC"),
            "{name}"
        );
        assert_eq!(row.direction, FeedDirection::Out);
        assert!(!dapp.estimated, "{name}: proven, not expected");
        assert_eq!(
            dapp.received,
            Some(line(FeedDirection::In, back, "BNB")),
            "{name}"
        );
        assert!(row.priced, "{name}: USDC at face value");
        assert_eq!(dapp.intent_term, Some(ClearTerm::IntentSwap));
    }
}

/// N5, Curve and the Aave withdraw. Curve's USDT is named by the scan's own
/// "Received" in that transaction (same amount); the withdraw's aToken is a
/// token nothing recorded a name for, so its line has no figure and the row
/// leads with the BNB that came back.
#[test]
fn curve_and_a_withdraw_read_what_the_chain_recorded() {
    let receipt = fixture("receipt-curve-swap");
    let record = record_of(
        "curve-swap",
        SignApproveOpts {
            balance_changes: Some(vec![judged(USDC, "-100000000000000000", "USDC", true)]),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![
        stored(&record, Some(&landed(&record, &receipt))),
        scanned(
            "rx-curve",
            &text(&receipt["transactionHash"]),
            "0.100012293602944751",
            "USDT",
        ),
    ]);
    assert_eq!(rows.len(), 1);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(rows[0].value.as_deref(), Some("0.1"));
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.100012293602944751", "USDT"))
    );

    let receipt = fixture("receipt-aave-withdraw");
    let record = record_of(
        "aave-withdraw",
        SignApproveOpts::default(),
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(rows[0].value, None);
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.003000000001373923", "BNB"))
    );
    assert_eq!(
        dapp.changes[0],
        unverified(FeedDirection::Out),
        "the aToken left, unnamed"
    );
    assert_eq!(dapp.place.as_deref(), Some("Aave"));
    // The sheet had named the aToken (held, so trusted): it leads.
    let record = record_of(
        "aave-withdraw",
        SignApproveOpts {
            balance_changes: Some(vec![judged(
                A_BNB_WBNB,
                "-3000000000000000000",
                "aBnbWBNB",
                true,
            )]),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (Some("0.002999999999999999"), "aBnbWBNB"),
        "the receipt's figure, not the sheet's"
    );
}

/// A coin sent with the call is proven by the record's own value, netted
/// with any of it that came back — the supply reads "−0.003 BNB" and its
/// aToken, folded in from the scan.
#[test]
fn the_coin_a_call_sends_is_its_own_value() {
    let receipt = fixture("receipt-aave-supply");
    let record = record_of(
        "aave-supply",
        SignApproveOpts::default(),
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![
        stored(&record, Some(&landed(&record, &receipt))),
        scanned(
            "rx-supply",
            &text(&receipt["transactionHash"]),
            "0.002999999999999999",
            "aBnbWBNB",
        ),
    ]);
    assert_eq!(rows.len(), 1);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (Some("0.003"), "BNB")
    );
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.002999999999999999", "aBnbWBNB"))
    );
    assert!(!rows[0].priced, "BNB has no price here: no fiat, not $0");
}

/// A network fee paid in a token is a `Transfer` in the same receipt — the
/// wallet's, not the dApp's: the row says the 1.16 USDC the swap took.
#[test]
fn a_fee_paid_in_a_token_is_not_the_dapps_figure() {
    let receipt = fixture("receipt-pcs-usdc-bnb");
    let relay = "0x7970ac76595c7a29357b7c683f4eed75f0bbd291";
    let fee = 20_000_000_000_000_000_u128; // 0.02 USDC
    let mut receipt_with_fee = receipt.clone();
    let logs = receipt_with_fee["logs"].as_array_mut().expect("logs");
    let at = logs
        .iter()
        .position(|log| text(&log["topics"][0]).starts_with("0x6895c136"))
        .expect("the Safe's ExecutionFromModuleSuccess");
    let topic = |address: &str| format!("0x{:0>64}", address.trim_start_matches("0x"));
    logs.insert(
        at,
        json!({
            "address": USDC,
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                topic(SAFE),
                topic(relay)
            ],
            "data": format!("0x{fee:064x}"),
        }),
    );
    let opts = SignApproveOpts {
        gas_fee_token: Some(USDC.to_owned()),
        quoted_fee: Some(SignQuotedFee {
            amount: fee.to_string(),
            recipient: relay.to_owned(),
            tier: None,
        }),
        balance_changes: Some(vec![judged(USDC, "-1160000000000000000", "USDC", true)]),
        ..SignApproveOpts::default()
    };
    let record = record_of("pcs-usdc-bnb", opts, &op_hash_of(&receipt));
    let summary = record.summary.clone().expect("a summary");
    assert_eq!(summary.fee_token.as_deref(), Some(USDC));
    assert_eq!(summary.fee_amount, Some(fee.to_string()));
    let patch = landed(&record, &receipt_with_fee);
    let proven = patch
        .settlement
        .as_ref()
        .and_then(|s| s.moved.clone())
        .expect("proven");
    assert_eq!(
        proven[0],
        moved(Some(USDC), "-1180000000000000000"),
        "the receipt itself holds both"
    );
    let rows = rows_of(vec![stored(&record, Some(&patch))]);
    assert_eq!(rows[0].value.as_deref(), Some("1.16"));
}

/// N5 stays honest where nothing was proven: a pending row keeps the sheet's
/// expectation ("≈"), a confirmation the chain's event alone found claims
/// nothing new, and a failed one moved nothing.
#[test]
fn only_a_read_receipt_replaces_the_sheets_expectation() {
    let receipt = fixture("receipt-pcs-usdc-bnb");
    let opts = SignApproveOpts {
        balance_changes: Some(vec![
            judged(USDC, "-1160000000000000000", "USDC", true),
            TrustSimJudgment::Native {
                delta: "1490000000000000".to_owned(),
            },
        ]),
        ..SignApproveOpts::default()
    };
    let record = record_of("pcs-usdc-bnb", opts, &op_hash_of(&receipt));
    let rows = rows_of(vec![stored(&record, None)]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(rows[0].value.as_deref(), Some("1.16"));
    assert!(dapp.estimated, "pending: the sheet's expectation, marked ≈");

    // The relay's receipt without logs: confirmed, nothing claimed.
    let mut sut = tracking(&record);
    let ops = sut.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::Receipt {
            user_op_hash: record.user_op_hash.clone(),
            tx_hash: text(&receipt["transactionHash"]),
            now_ms: NOW + 4_000.0,
        },
    );
    let patch = patch_in(&ops);
    assert_eq!(patch.settlement, None);
    let rows = rows_of(vec![stored(&record, Some(&patch))]);
    assert!(rows[0].dapp.as_ref().is_some_and(|d| d.estimated));

    // The bundle's receipt read by its tx hash proves the same as the relay's.
    let mut sut = tracking(&record);
    sut.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::ReceiptPending {
            user_op_hash: record.user_op_hash.clone(),
            now_ms: NOW + 300.0,
        },
    );
    sut.dispatch(TrackEvent::Tick);
    sut.resolve_matching(
        |op| matches!(op, TOp::Now),
        TRes::Clock {
            now_ms: NOW + 12_400.0,
        },
    );
    sut.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::ReceiptPending {
            user_op_hash: record.user_op_hash.clone(),
            now_ms: NOW + 12_700.0,
        },
    );
    let ops = sut.resolve_matching(
        |op| matches!(op, TOp::PollStatus { .. }),
        TRes::Status {
            rejection_reason: None,
            user_op_hash: record.user_op_hash.clone(),
            status: TrackLifecycle::Included,
            stage: None,
            now_ms: NOW + 13_100.0,
            tx_hash: Some(text(&receipt["transactionHash"])),
        },
    );
    assert!(
        ops.iter().any(|op| matches!(op, TOp::TxReceipt { .. })),
        "{ops:?}"
    );
    let ops = sut.resolve_matching(
        |op| matches!(op, TOp::TxReceipt { .. }),
        TRes::TxReceipt {
            user_op_hash: record.user_op_hash.clone(),
            now_ms: NOW + 13_500.0,
            receipt_json: Some(receipt.to_string()),
        },
    );
    assert_eq!(
        patch_in(&ops).settlement,
        landed(&record, &receipt).settlement
    );
}

/// N4: the deliberate failure of the pass — an Aave withdraw of USDC the
/// Safe never supplied. The relay refused it after "Submitted"; the record
/// keeps why, and the row says it.
#[test]
fn a_failed_row_says_why() {
    let op = format!("0x{}", "4319f0f7".repeat(8));
    let record = record_of("aave-withdraw-failed", SignApproveOpts::default(), &op);
    let mut sut = tracking(&record);
    // The relay's word, after a receipt poll that found nothing yet.
    sut.resolve_matching(
        |op| matches!(op, TOp::PollReceipt { .. }),
        TRes::ReceiptPending {
            user_op_hash: op.clone(),
            now_ms: NOW + 3_000.0,
        },
    );
    let mut patch = None;
    for _ in 0..50 {
        let pending = sut.outstanding();
        let Some(next) = pending.first() else {
            sut.dispatch(TrackEvent::Tick);
            continue;
        };
        let ops = sut.resolve(match next {
            TOp::Now => TRes::Clock {
                now_ms: NOW + 13_000.0,
            },
            TOp::PollReceipt { .. } => TRes::ReceiptPending {
                user_op_hash: op.clone(),
                now_ms: NOW + 13_000.0,
            },
            TOp::PollStatus { .. } => TRes::Status {
                rejection_reason: None,
                user_op_hash: op.clone(),
                status: TrackLifecycle::Rejected,
                stage: None,
                now_ms: NOW + 13_100.0,
                tx_hash: None,
            },
            other => panic!("unexpected {other:?}"),
        });
        if ops
            .iter()
            .any(|op| matches!(op, TOp::UpdateTxRecords { .. }))
        {
            patch = Some(patch_in(&ops));
            break;
        }
    }
    let patch = patch.expect("the refusal closed the record");
    assert_eq!(
        patch.settlement,
        Some(TrackSettlement {
            moved: None,
            failure: Some(TrackFailure::Refused),
        })
    );
    let rows = rows_of(vec![stored(&record, Some(&patch))]);
    let row = &rows[0];
    assert_eq!(row.status, FeedTxStatus::Failed);
    let dapp = row.dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.failure, Some(TrackFailure::Refused));
    assert_eq!(dapp.place.as_deref(), Some("Aave"));
    assert_eq!(row.value, None);

    // A confirmed row says no failure, and an older failed one none it lacks.
    let mut older = stored(&record, Some(&patch));
    older.settlement = None;
    let rows = rows_of(vec![older]);
    assert_eq!(rows[0].dapp.as_ref().and_then(|d| d.failure), None);
}

/// N7 + N8, the 1inch native order: "create order" as the sheet's
/// descriptor-service reading wrote it is a title ("Create order"); the
/// contract the reading named — NativeOrderFactory, owned by 1inch — names
/// the place and the detail's contract, never the raw address. The 0.003 BNB
/// it sent has no known price: no fiat figure, not "≈ $0.00".
#[test]
fn the_contract_the_sheet_named_is_named_in_activity() {
    let receipt = fixture("receipt-1inch-create-order");
    let opts = SignApproveOpts {
        intent: Some("create order".to_owned()),
        reading: Some(DappReading {
            address: Some(NATIVE_ORDER_FACTORY.to_owned()),
            name: Some("NativeOrderFactory".to_owned()),
            owner: Some("1inch".to_owned()),
            tokens: Vec::new(),
        }),
        ..SignApproveOpts::default()
    };
    let record = record_of("1inch-create-order", opts, &op_hash_of(&receipt));
    let summary = record.summary.clone().expect("a summary");
    assert_eq!(summary.contract_name.as_deref(), Some("NativeOrderFactory"));
    assert_eq!(summary.owner.as_deref(), Some("1inch"));
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    let row = &rows[0];
    let dapp = row.dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.intent.as_deref(), Some("Create order"));
    assert_eq!(dapp.intent_term, None);
    assert_eq!(dapp.place.as_deref(), Some("1inch"));
    assert!(dapp.facts.contains(&FeedFact::Contract {
        address: NATIVE_ORDER_FACTORY.to_owned(),
        name: Some("NativeOrderFactory".to_owned()),
    }));
    assert_eq!(
        (row.value.as_deref(), row.symbol.as_str()),
        (Some("0.003"), "BNB")
    );
    assert!(!row.priced, "unknown is not zero");
}

/// N7, places: an approval of Permit2 takes the protocol that owns that
/// Permit2 (Uniswap's), not the site's host; a call to a contract the
/// reading named takes its owner (Curve, once the sheet names the router);
/// and a signed 1inch `Order` is a swap on 1inch, not "Sign structured data".
#[test]
fn places_are_protocols_and_an_order_is_a_swap() {
    // The person kept the site's unlimited ask (the sheet's consent gate).
    let approve = record_of(
        "uni-approve",
        SignApproveOpts {
            unlimited_approved: true,
            ..SignApproveOpts::default()
        },
        &format!("0x{}", "01".repeat(32)),
    );
    let order = record_of(
        "1inch-order",
        SignApproveOpts {
            intent: Some("1inch Order".to_owned()),
            ..SignApproveOpts::default()
        },
        "",
    );
    let curve = record_of(
        "curve-swap",
        SignApproveOpts {
            reading: Some(DappReading {
                address: Some(CURVE_ROUTER.to_owned()),
                name: Some("Curve Router".to_owned()),
                owner: Some("Curve".to_owned()),
                tokens: Vec::new(),
            }),
            ..SignApproveOpts::default()
        },
        &format!("0x{}", "02".repeat(32)),
    );
    let mut records = vec![stored(&approve, None), stored(&order, None)];
    let mut later = stored(&curve, None);
    later.timestamp += 30.0;
    later.id = format!("{}-curve", later.id);
    records.push(later);
    let rows = rows_of(records);
    let place = |row: &FeedItem| row.dapp.as_ref().and_then(|d| d.place.clone());
    let curve_row = &rows[0];
    assert_eq!(place(curve_row).as_deref(), Some("Curve"));
    assert!(curve_row
        .dapp
        .as_ref()
        .is_some_and(|d| d.facts.contains(&FeedFact::Contract {
            address: CURVE_ROUTER.to_owned(),
            name: Some("Curve Router".to_owned()),
        })));
    let approve_row = rows
        .iter()
        .find(|row| {
            row.dapp
                .as_ref()
                .is_some_and(|d| d.intent_term == Some(ClearTerm::IntentApprove))
        })
        .expect("the approval");
    assert_eq!(place(approve_row).as_deref(), Some("Uniswap"));
    let order_row = rows
        .iter()
        .find(|row| row.kind == FeedTxKind::SignTypedData)
        .expect("the order");
    let dapp = order_row.dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.intent_term, Some(ClearTerm::IntentSwap));
    assert_eq!(dapp.place.as_deref(), Some("1inch"));
    assert!(dapp.off_chain);
}

/// The facts the approve adds to a summary: a reading's name only for the
/// contract it read, and a fee only in a token, only on a transaction.
#[test]
fn the_approve_adds_only_what_belongs_to_the_request() {
    let base = DappSummary {
        contract: Some(AAVE_POOL.to_owned()),
        calls: 1,
        ..DappSummary::default()
    };
    let gateway = DappReading {
        address: Some(AAVE_GATEWAY.to_owned()),
        name: Some("Gateway".to_owned()),
        owner: Some("Aave".to_owned()),
        tokens: vec![DappToken {
            address: USDC.to_uppercase().replace("0X", "0x"),
            symbol: "USDC".to_owned(),
            decimals: 18,
        }],
    };
    let named = with_approve_facts(base.clone(), Some(&gateway), None, None);
    assert_eq!(
        (named.contract_name, named.owner),
        (None, None),
        "read for another contract"
    );
    assert_eq!(
        named.tokens,
        vec![DappToken {
            address: USDC.to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 18,
        }],
        "the coins it named are the request's whatever contract was read"
    );
    let pool = DappReading {
        address: Some(AAVE_POOL.to_uppercase().replace("0X", "0x")),
        ..gateway
    };
    let named = with_approve_facts(base.clone(), Some(&pool), Some(USDC), Some("5"));
    assert_eq!(named.contract_name.as_deref(), Some("Gateway"));
    assert_eq!(named.fee_token.as_deref(), Some(USDC));
    assert_eq!(named.fee_amount.as_deref(), Some("5"));
    assert_eq!(
        with_approve_facts(base.clone(), None, Some("not a token"), Some("5")).fee_token,
        None
    );
    assert_eq!(
        with_approve_facts(base.clone(), None, Some(USDC), Some("0x5")).fee_amount,
        None
    );
    let signature = DappSummary {
        action: vela_core::app::dapp_activity::DappAction::TypedData,
        ..base
    };
    assert_eq!(
        with_approve_facts(signature, None, Some(USDC), Some("5")).fee_token,
        None,
        "a signature pays no fee"
    );
}

/// A row from before 097 reads as it did: a stored record with none of the
/// new fields loads, and an unreadable settlement is no settlement.
#[test]
fn records_from_before_097_still_read() {
    let receipt = fixture("receipt-aave-borrow");
    let record = record_of(
        "aave-borrow",
        SignApproveOpts::default(),
        &op_hash_of(&receipt),
    );
    let mut value = serde_json::to_value(stored(&record, None)).expect("json");
    value["settlement"] = json!({"moved": "not a list"});
    value["status"] = json!("confirmed");
    let row: FeedTxRecord = serde_json::from_value(value).expect("lenient");
    assert_eq!(row.settlement, None);
    let rows = rows_of(vec![row]);
    assert_eq!(rows[0].value, None);
    assert_eq!(rows[0].dapp.as_ref().and_then(|d| d.received.clone()), None);
}

/// N8, the sheet's side: a finished reading hands its contract's name and
/// owner to the record — here the build's own Aave descriptor for the pool
/// the borrow called. The shell copies it to the approve and decides nothing.
#[test]
fn the_sheets_reading_names_the_contract_for_the_record() {
    use vela_core::app::clear_signing::{
        ClearLocale, ClearOperation as COp, ClearProbe, ClearShellResult as CRes, ClearSigning,
        ClearSurface, Event as ClearEvent,
    };
    let request = fixture("req-aave-borrow");
    let tx = &request["params"][0];
    let mut sut = DomainDriver::<ClearSigning>::new();
    sut.dispatch(ClearEvent::ResolveTransaction {
        to: Some(text(&tx["to"])),
        data: Some(text(&tx["data"])),
        value: None,
        chain_id: BSC,
        locale: ClearLocale::default(),
    });
    for _ in 0..200 {
        let Some(op) = sut.outstanding().first().cloned() else {
            break;
        };
        sut.resolve(match op {
            COp::Now => CRes::Clock { now_ms: NOW },
            COp::HttpGet { path } => CRes::DescriptorFetched { path, json: None },
            // The chain answers for USDC on BNB Chain: 18 decimals, "USDC".
            COp::RpcEthCall {
                chain_id,
                to,
                probe,
                ..
            } => {
                let usdc = chain_id == BSC && to.eq_ignore_ascii_case(USDC);
                let result = match probe {
                    ClearProbe::Decimals if usdc => Some(format!("0x{:064x}", 18)),
                    ClearProbe::Symbol if usdc => {
                        Some(format!("0x{:064x}{:064x}{:0<64}", 32, 4, "55534443"))
                    }
                    _ => None,
                };
                CRes::RpcAnswer {
                    probe,
                    chain_id,
                    to,
                    rpc_error: result.is_none(),
                    result,
                }
            }
            COp::SelectorDbLookup { .. } => CRes::SelectorCandidates { sigs: Vec::new() },
            COp::Timer { token, .. } => CRes::TimedOut { token },
        });
    }
    let view = sut.view();
    assert_eq!(view.surface, ClearSurface::ClearSign);
    assert_eq!(
        view.record_reading,
        Some(DappReading {
            address: Some(AAVE_POOL.to_owned()),
            name: Some("Aave V3 Pool".to_owned()),
            owner: Some("Aave".to_owned()),
            tokens: vec![DappToken {
                address: USDC.to_owned(),
                symbol: "USDC".to_owned(),
                decimals: 18,
            }],
        })
    );
}

/// N5 on a sheet with no simulation (the web's): the coins the sheet's
/// READING showed name what the receipt proves, in the detail. A name is no
/// trust: a coin the wallet does not know never leads the row, and one
/// arriving carries no figure — the site's own contract could have answered
/// any symbol. BNB Chain's USDC and USDT are the wallet's registry coins
/// there (097 D), so they are trusted whatever the reading says: before
/// 097 D the web's USDC → BNB swap read only "+0.0015 BNB", and Curve's
/// USDT arrived with no figure.
#[test]
fn the_sheets_reading_names_the_coins_without_a_simulation() {
    let token = |address: &str, symbol: &str| DappToken {
        address: address.to_owned(),
        symbol: symbol.to_owned(),
        decimals: 18,
    };
    let receipt = fixture("receipt-uni-usdc-bnb");
    let record = record_of(
        "uni-usdc-bnb",
        SignApproveOpts {
            reading: Some(DappReading {
                tokens: vec![token(USDC, "USDC")],
                ..DappReading::default()
            }),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.changes[0], line(FeedDirection::Out, "1.16", "USDC"));
    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (Some("1.16"), "USDC"),
        "the registry's USDC leads the row"
    );
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.001496447240777689", "BNB"))
    );

    let receipt = fixture("receipt-curve-swap");
    let record = record_of(
        "curve-swap",
        SignApproveOpts {
            reading: Some(DappReading {
                tokens: vec![token(USDC, "USDC"), token(USDT, "USDT")],
                ..DappReading::default()
            }),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(dapp.changes[0], line(FeedDirection::Out, "0.1", "USDC"));
    assert_eq!(
        dapp.received,
        Some(line(FeedDirection::In, "0.100012293602944751", "USDT"))
    );

    // Aave's aToken is no registry coin: named by the reading, it arrives
    // with no figure.
    let receipt = fixture("receipt-aave-supply");
    let record = record_of(
        "aave-supply",
        SignApproveOpts {
            reading: Some(DappReading {
                tokens: vec![token(A_BNB_WBNB, "aBnbWBNB")],
                ..DappReading::default()
            }),
            ..SignApproveOpts::default()
        },
        &op_hash_of(&receipt),
    );
    let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(
        (rows[0].value.as_deref(), rows[0].symbol.as_str()),
        (Some("0.003"), "BNB")
    );
    assert_eq!(dapp.changes[1], unverified(FeedDirection::In));
    assert_eq!(dapp.received, None, "named by the reading alone: no figure");
}

/// 097 D: the registry alone names a swap's coins — a record with neither
/// the sheet's judgments nor its reading (a client that recorded nothing)
/// still leads both USDC → BNB swaps with the 1.16 USDC they took.
#[test]
fn the_registry_names_the_coins_of_a_record_that_named_none() {
    for (name, back) in [
        ("pcs-usdc-bnb", "0.00149903634907156"),
        ("uni-usdc-bnb", "0.001496447240777689"),
    ] {
        let receipt = fixture(&format!("receipt-{name}"));
        let record = record_of(name, SignApproveOpts::default(), &op_hash_of(&receipt));
        let rows = rows_of(vec![stored(&record, Some(&landed(&record, &receipt)))]);
        let dapp = rows[0].dapp.as_ref().expect("a dApp row");
        assert_eq!(
            (rows[0].value.as_deref(), rows[0].symbol.as_str()),
            (Some("1.16"), "USDC"),
            "{name}"
        );
        assert_eq!(rows[0].direction, FeedDirection::Out, "{name}");
        assert_eq!(
            dapp.received,
            Some(line(FeedDirection::In, back, "BNB")),
            "{name}"
        );
        assert!(rows[0].priced, "{name}: USDC at face value");
    }
}

/// 097 D: the registry names a token on the record's own chain only. The
/// same receipt filed under Ethereum moves bytes that are no coin the wallet
/// knows there: the USDC line has no name and leads nothing.
#[test]
fn a_registry_coin_is_named_on_its_own_chain_only() {
    let receipt = fixture("receipt-uni-usdc-bnb");
    let record = record_of(
        "uni-usdc-bnb",
        SignApproveOpts::default(),
        &op_hash_of(&receipt),
    );
    let mut on_ethereum = stored(&record, Some(&landed(&record, &receipt)));
    on_ethereum.chain_id = 1;
    let rows = rows_of(vec![on_ethereum]);
    let dapp = rows[0].dapp.as_ref().expect("a dApp row");
    assert_eq!(rows[0].value, None);
    assert_eq!(dapp.changes[0], unverified(FeedDirection::Out));
}

/// 097 D: an allowance's token is named by the registry of the request's
/// chain — BNB Chain's USDC at its 18 decimals there; Ethereum's USDC
/// address on BNB Chain is nobody's word, so no name and no width.
#[test]
fn an_allowance_names_its_token_on_its_own_chain() {
    let approve = |token: &str| {
        json!([{
            "to": token,
            "data": format!("0x095ea7b3{:0>64}{:0>64}", &CURVE_ROUTER[2..], "11b340"),
            "value": "0x0",
        }])
    };
    let usdc_ethereum = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    let on = |chain_id: u32, token: &str| {
        let summary = summarize(
            "eth_sendTransaction",
            &approve(token),
            chain_id,
            "https://www.curve.finance",
            None,
        );
        (summary.symbol, summary.decimals)
    };
    assert_eq!(on(BSC, USDC), (Some("USDC".to_owned()), Some(18)));
    assert_eq!(on(BSC, usdc_ethereum), (None, None));
    assert_eq!(on(1, usdc_ethereum), (Some("USDC".to_owned()), Some(6)));
}
