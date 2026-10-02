//! Spec 082, the merge of the four core groups: where one group's output is
//! another group's input, the two meet here.
//!
//! Each group proved its own rules in its own suite, against fixtures it
//! built by hand for the other side. These tests drive the real machines end
//! to end instead, with the shell's part (store a record, apply a patch,
//! re-read the store) done the way every shell does it:
//!
//! - a lost submit reply (A: `sign_request`, `tx_tracker`) is one pending
//!   Activity row (G: `activity_feed`, RG4) until the tracker ends it —
//!   failed when the relay never had it, confirmed when the chain shows it;
//! - the record vocabularies the shells copy through unchanged line up;
//! - the browser bar (EF) and the Activity row (G) name a site the same way;
//! - the plain-send card (C) and the Activity row (G) state the same amount;
//! - every corpus key these modules hand a shell is in all 15 locales.

#![cfg(feature = "crux")]

mod support;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use support::DomainDriver;
use vela_core::app::activity_feed::{
    self, dapp_site, native_amount, ActivityFeed, FeedItem, FeedOperation as FeedOp, FeedRow,
    FeedShellResult as FeedRes, FeedTxKind, FeedTxRecord, FeedTxStatus,
};
use vela_core::app::browser_load::{self, address_bar, LoadFailureClass};
use vela_core::app::clear_signing::{plain_send_of, ClearLocale};
use vela_core::app::sign_request::{
    ending_of, ending_state, not_confirmed_detail, reverted_detail, Event as SignEvent,
    SignAccountRef, SignApproveOpts, SignEndingState, SignErrorKind, SignOperation as SignOp,
    SignRecord, SignRecordKind, SignRecordStatus, SignRequest, SignResponsePayload,
    SignShellResult as SignRes, SignSubmitOutcome,
};
use vela_core::app::sim_outcome;
use vela_core::app::tx_tracker::{
    Event as TrackEvent, TrackLifecycle, TrackOperation as TrackOp, TrackOutcome,
    TrackRecordStatus, TrackShellResult as TrackRes, TrackStatus, TxTracker,
};
use vela_core::primitives::checksum_address;
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::USER_OPERATION_EVENT_TOPIC;

const NOW: f64 = 1_700_000_000_000.0;
/// The signing account, as the sign machine is given it (lower case).
const ME: &str = "0x8f3cf7ad23cd3cadbd9735aff958023239c6a063";
const ORIGIN: &str = "https://app.example:8443";
const TO: &str = "0x3333333333333333333333333333333333333333";
/// 0.001 of the native coin.
const VALUE: &str = "0x38d7ea4c68000";
/// The op hash the wallet computed itself when the relay's reply was lost.
const LOCAL_OP: &str = "0x5538ce6978ab8924cce04d68c11cded19c36a8773e94656be6ada6315477ba1f";
/// The head read before the first submit POST.
const SUBMIT_BLOCK: u64 = 48_479_100;
/// The bundle tx the op's `UserOperationEvent` names.
const EVENT_TX: &str = "0xc6f3544fc4e3ac769e92c92ab4804cd3f38ffb8d607ba07b5103a59710094dc4";

// ---------------------------------------------------------------------------
// The shell's part
// ---------------------------------------------------------------------------

/// A value re-read as another module's type through its wire form — what a
/// shell does when it stores one module's string and hands it to another.
fn rewire<T: Serialize, U: DeserializeOwned>(value: &T) -> U {
    let json = serde_json::to_value(value).expect("serialises");
    serde_json::from_value(json).expect("the other module reads the same word")
}

/// The stored record, as every shell's `buildSigningRecord` writes a
/// transaction and the feed reads it back (`FeedTxRecord`).
fn stored(record: &SignRecord) -> FeedTxRecord {
    let params: Value = serde_json::from_str(&record.params_json).expect("params");
    let tx = &params[0];
    let timestamp = (record.now_ms / 1_000.0).floor();
    FeedTxRecord {
        id: record.record_id.clone(),
        user_op_hash: record.user_op_hash.clone(),
        tx_hash: record.result.clone(),
        from: record.from.clone(),
        to: tx["to"].as_str().unwrap_or_default().to_owned(),
        to_name: Some("Recipient".to_owned()),
        value: tx["value"].as_str().unwrap_or("0x0").to_owned(),
        symbol: "xDAI".to_owned(),
        decimals: 18,
        logo_urls: None,
        chain_id: record.chain_id,
        timestamp,
        day_start_ms: (timestamp / 86_400.0).floor() * 86_400_000.0,
        status: rewire(&record.status),
        kind: Some(rewire(&record.kind)),
        usd: None,
        dapp_url: Some(record.dapp_url.clone()),
        intent: record.intent.clone(),
        balance_changes: record.balance_changes.clone(),
        calldata: None,
        call_data: tx["data"].as_str().map(str::to_owned),
        summary: record.summary.clone(),
    }
}

/// The tracker's patch, applied in place to the stored records it names.
fn apply(records: &mut [FeedTxRecord], ops: &[TrackOp]) {
    for op in ops {
        if let TrackOp::UpdateTxRecords { ids, patch } = op {
            for record in records.iter_mut().filter(|r| ids.contains(&r.id)) {
                record.status = rewire(&patch.status);
                if let Some(tx_hash) = &patch.tx_hash {
                    record.tx_hash = tx_hash.clone();
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// The machines
// ---------------------------------------------------------------------------

/// A dApp's plain send approved, passkey done, and the relay's reply lost:
/// the sign machine hands over the op under the LOCAL hash. Returns the
/// machine and the pending record it asked the shell to store.
fn lost_reply() -> (DomainDriver<SignRequest>, SignRecord) {
    let mut sign = approved();
    let ops = sign.dispatch(SignEvent::OpSubmitted {
        id: "req-1".to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: NOW + 40_000.0,
        maybe_sent: true,
        submit_block: Some(SUBMIT_BLOCK),
    });
    let [SignOp::PersistRecord { record }] = ops.as_slice() else {
        unreachable!("one pending record: {ops:?}")
    };
    let record = record.clone();
    assert!(sign.resolve(SignRes::RecordPersisted).is_empty());
    (sign, record)
}

/// A dApp's plain send, approved and past the pre-check: the submit is out.
fn approved() -> DomainDriver<SignRequest> {
    let mut sign = DomainDriver::<SignRequest>::new();
    sign.dispatch(SignEvent::NetworksChanged {
        chain_ids: vec![100],
    });
    sign.dispatch(SignEvent::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: ME.to_owned(),
            credential_id: "cred-0".to_owned(),
        }],
        active_index: 0,
    });
    sign.dispatch(SignEvent::RequestArrived {
        id: "req-1".to_owned(),
        method: "eth_sendTransaction".to_owned(),
        params_json: format!(r#"[{{"to":"{TO}","data":"0x","value":"{VALUE}"}}]"#),
        origin: ORIGIN.to_owned(),
        transport_id: "wp-1".to_owned(),
        dedicated_transport: false,
        per_request_chain: None,
        dapp: None,
        granted_address: None,
        requested_address: None,
        request_ts_ms: None,
        now_ms: NOW,
    });
    sign.dispatch(SignEvent::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    let ops = sign.resolve(SignRes::PreCheck { funding: None });
    assert!(
        matches!(ops.as_slice(), [SignOp::SignAndSubmit { .. }]),
        "{ops:?}"
    );
    sign
}

/// The page's one answer after the shell's receipt wait ran out: the op hash.
fn answered(sign: &mut DomainDriver<SignRequest>) -> SignResponsePayload {
    let ops = sign.resolve(SignRes::Submit {
        outcome: SignSubmitOutcome::ReceiptPending {
            user_op_hash: LOCAL_OP.to_owned(),
        },
        now_ms: NOW + 160_000.0,
    });
    match ops.as_slice() {
        [SignOp::SendResponse { payload, .. }] => payload.clone(),
        other => unreachable!("one answer: {other:?}"),
    }
}

/// The shell feeds the hand-off to the tracker (the flag and the head with
/// it) and answers the first receipt poll `pending`.
fn tracking(sign: &DomainDriver<SignRequest>) -> DomainDriver<TxTracker> {
    let handoff = sign.view().tracker_handoff.expect("the hand-off");
    let mut tracker = DomainDriver::<TxTracker>::new();
    tracker.dispatch(TrackEvent::Submitted {
        user_op_hash: handoff.user_op_hash,
        record_ids: handoff.record_ids,
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    });
    tracker.resolve(TrackRes::Clock { now_ms: NOW });
    tracker.resolve(TrackRes::ReceiptPending {
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: NOW + 300.0,
    });
    tracker
}

fn tick(tracker: &mut DomainDriver<TxTracker>, now_ms: f64) -> Vec<TrackOp> {
    tracker.dispatch(TrackEvent::Tick);
    tracker.resolve_matching(|op| matches!(op, TrackOp::Now), TrackRes::Clock { now_ms })
}

fn is_find(op: &TrackOp) -> bool {
    matches!(op, TrackOp::FindOpEvent { .. })
}

fn op_event(now_ms: f64, logs: Option<&str>, head: u64) -> TrackRes {
    TrackRes::OpEvent {
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms,
        logs_json: logs.map(str::to_owned),
        error_json: None,
        head_block: Some(head),
    }
}

/// One status-poll round at `at`: the chain read to `head` with no event,
/// every receipt poll `pending`, every status poll `not_found`. Returns what
/// the answers asked for besides more reading.
fn not_found_round(tracker: &mut DomainDriver<TxTracker>, at: f64, head: u64) -> Vec<TrackOp> {
    tick(tracker, at);
    while tracker.outstanding().iter().any(is_find) {
        tracker.resolve_matching(is_find, op_event(at + 100.0, Some("[]"), head));
    }
    let mut asked = Vec::new();
    while let Some(op) = tracker
        .outstanding()
        .into_iter()
        .find(|op| matches!(op, TrackOp::PollReceipt { .. } | TrackOp::PollStatus { .. }))
    {
        let answer = match op {
            TrackOp::PollReceipt { .. } => TrackRes::ReceiptPending {
                user_op_hash: LOCAL_OP.to_owned(),
                now_ms: at + 200.0,
            },
            _ => TrackRes::Status {
                user_op_hash: LOCAL_OP.to_owned(),
                status: TrackLifecycle::NotFound,
                stage: None,
                now_ms: at + 200.0,
                tx_hash: None,
            },
        };
        asked.extend(tracker.resolve_matching(|o| *o == op, answer));
    }
    asked.retain(|op| {
        !matches!(
            op,
            TrackOp::FindOpEvent { .. } | TrackOp::PollReceipt { .. } | TrackOp::PollStatus { .. }
        )
    });
    asked
}

/// The op's own `UserOperationEvent` with `success = true`.
fn found_event() -> String {
    let data = format!("0x{:064x}{:064x}{:064x}{:064x}", 22, 1, 0x359a5, 0x359a5);
    serde_json::json!([{
        "address": ENTRY_POINT.to_lowercase(),
        "topics": [
            USER_OPERATION_EVENT_TOPIC,
            LOCAL_OP,
            format!("0x000000000000000000000000{}", &ME[2..]),
            "0x0000000000000000000000000000000000000000000000000000000000000000"
        ],
        "data": data,
        "blockNumber": "0x2e3bb1c",
        "transactionHash": EVENT_TX,
        "removed": false
    }])
    .to_string()
}

/// The Activity feed for this account, loaded from `records` (the account
/// spelled EIP-55, as the wallet now names it everywhere, RG10).
fn feed(records: Vec<FeedTxRecord>) -> DomainDriver<ActivityFeed> {
    let mut feed = DomainDriver::<ActivityFeed>::new();
    feed.dispatch(activity_feed::Event::AccountSwitched {
        address: checksum_address(ME).expect("an address"),
    });
    reload(&mut feed, records);
    feed.resolve_matching(
        |op| matches!(op, FeedOp::ScanIncomingTransfers { .. }),
        FeedRes::SyncCompleted { new_count: 0 },
    );
    feed
}

/// Answer the outstanding store read with `records`.
fn reload(feed: &mut DomainDriver<ActivityFeed>, records: Vec<FeedTxRecord>) {
    let read_id = feed
        .outstanding()
        .iter()
        .find_map(|op| match op {
            FeedOp::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a store read");
    feed.resolve_matching(
        |op| matches!(op, FeedOp::ReadTxStore { .. }),
        FeedRes::StoreLoaded {
            records,
            now_ms: NOW + 1_000.0,
            read_id,
        },
    );
}

/// The tracker wrote records: the shell says so, and the feed re-reads (RG3).
fn reconciled(feed: &mut DomainDriver<ActivityFeed>, records: Vec<FeedTxRecord>) {
    let ops = feed.dispatch(activity_feed::Event::ReconcileCompleted { resolved_count: 1 });
    assert!(
        ops.iter()
            .any(|op| matches!(op, FeedOp::ReadTxStore { .. })),
        "{ops:?}"
    );
    reload(feed, records);
}

fn rows(feed: &DomainDriver<ActivityFeed>) -> Vec<FeedItem> {
    feed.view()
        .rows
        .into_iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item),
            FeedRow::Header { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// A lost reply, from the sheet to Activity (A → G, RG4, RA3, RA4, ruling 8)
// ---------------------------------------------------------------------------

/// The relay's reply was lost and the relay never had the op. The whole way
/// through there is ONE row for it: pending under the local hash with the
/// site that asked — never failed on time, never a second row — until the
/// tracker's NotSent patches that same record failed. The sheet's ending
/// follows the same entry: "may have been sent", then "not sent".
#[test]
fn a_lost_reply_the_relay_never_had_is_one_row_pending_then_failed() {
    let (mut sign, record) = lost_reply();
    assert_eq!(record.status, SignRecordStatus::Pending);
    assert_eq!(record.user_op_hash, LOCAL_OP);
    assert!(record.maybe_sent);
    assert_eq!(record.dapp_origin, ORIGIN);

    let mut records = vec![stored(&record)];
    let mut activity = feed(records.clone());
    let row = rows(&activity).pop().expect("the row");
    assert_eq!(rows(&activity).len(), 1);
    assert_eq!(row.id, record.record_id);
    assert_eq!(row.kind, FeedTxKind::DappTx);
    assert_eq!(row.status, FeedTxStatus::Pending);
    assert_eq!(row.site.as_deref(), Some("app.example:8443"));
    assert_eq!(row.tx_hash, None, "no on-chain hash is claimed");
    assert_eq!(row.value.as_deref(), Some("0.001"));
    assert_eq!(activity.view().transactions[0].user_op_hash, LOCAL_OP);

    let payload = answered(&mut sign);
    assert_eq!(
        payload,
        SignResponsePayload::Err {
            code: -32603,
            kind: SignErrorKind::SubmitFailed,
            message: Some(not_confirmed_detail(LOCAL_OP)),
        },
        "the page's one answer: not confirmed yet — never the op hash (083, owner ruling 2026-10-01)"
    );
    let ending = ending_of("eth_sendTransaction", &payload, Some(LOCAL_OP)).expect("an ending");

    let mut tracker = tracking(&sign);
    assert_eq!(tracker.view().entries[0].outcome, TrackOutcome::MaybeSent);
    assert!(matches!(
        ending_state(&ending, tracker.view().entries.first()),
        SignEndingState::Following {
            outcome: TrackOutcome::MaybeSent,
            ..
        }
    ));

    // Inside the grace, and the first answer past it: still in doubt.
    for (at, head) in [(30_000.0, 6), (61_000.0, 12)] {
        let asked = not_found_round(&mut tracker, NOW + at, SUBMIT_BLOCK + head);
        assert!(asked.is_empty(), "{asked:?}");
        apply(&mut records, &asked);
        reconciled(&mut activity, records.clone());
        assert_eq!(rows(&activity)[0].status, FeedTxStatus::Pending);
    }

    // The second past the grace, the chain read to its head: never sent.
    let asked = not_found_round(&mut tracker, NOW + 73_000.0, SUBMIT_BLOCK + 14);
    assert!(
        asked.iter().any(|op| matches!(
            op,
            TrackOp::UpdateTxRecords { ids, patch }
                if *ids == vec![record.record_id.clone()]
                    && patch.status == TrackRecordStatus::Failed
        )),
        "the same record, failed: {asked:?}"
    );
    assert_eq!(tracker.view().entries[0].status, TrackStatus::NotSent);
    apply(&mut records, &asked);
    reconciled(&mut activity, records.clone());

    let after = rows(&activity);
    assert_eq!(after.len(), 1, "still one row");
    assert_eq!(after[0].id, record.record_id);
    assert_eq!(after[0].status, FeedTxStatus::Failed);
    assert_eq!(after[0].site.as_deref(), Some("app.example:8443"));
    assert_eq!(
        ending_state(&ending, tracker.view().entries.first()),
        SignEndingState::NotSent
    );
}

/// The relay's reply was lost but the op landed: the chain's own event
/// (ruling 8) confirms it with the bundle's tx hash, and the one row turns
/// confirmed with that hash — the hash the sheet's ending names too.
#[test]
fn a_lost_reply_found_on_chain_is_one_row_confirmed_with_the_event_tx() {
    let (mut sign, record) = lost_reply();
    let mut records = vec![stored(&record)];
    let mut activity = feed(records.clone());
    let ending =
        ending_of("eth_sendTransaction", &answered(&mut sign), Some(LOCAL_OP)).expect("an ending");

    let mut tracker = tracking(&sign);
    let ops = tick(&mut tracker, NOW + 12_400.0);
    assert!(ops.iter().any(is_find), "{ops:?}");
    // The first read asks for the head only; the window then starts at the
    // head read before the submit.
    let ops = tracker.resolve_matching(is_find, op_event(NOW + 12_500.0, None, SUBMIT_BLOCK + 40));
    assert!(
        ops.iter().any(|op| matches!(
            op,
            TrackOp::FindOpEvent {
                from_block: Some(SUBMIT_BLOCK),
                ..
            }
        )),
        "{ops:?}"
    );
    let asked = tracker.resolve_matching(
        is_find,
        op_event(NOW + 12_600.0, Some(&found_event()), SUBMIT_BLOCK + 40),
    );
    assert!(
        asked
            .iter()
            .any(|op| matches!(op, TrackOp::HoldingsMoved { .. })),
        "{asked:?}"
    );
    apply(&mut records, &asked);
    reconciled(&mut activity, records.clone());

    let after = rows(&activity);
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].status, FeedTxStatus::Confirmed);
    assert_eq!(after[0].tx_hash.as_deref(), Some(EVENT_TX));
    assert_eq!(
        ending_state(&ending, tracker.view().entries.first()),
        SignEndingState::Confirmed {
            tx_hash: EVENT_TX.to_owned()
        }
    );
}

/// Feed the sign machine's current hand-off to the tracker, as every shell
/// does once per value.
fn feed_handoff(
    sign: &DomainDriver<SignRequest>,
    tracker: &mut DomainDriver<TxTracker>,
) -> Vec<TrackOp> {
    let handoff = sign.view().tracker_handoff.expect("the hand-off");
    tracker.dispatch(TrackEvent::Submitted {
        user_op_hash: handoff.user_op_hash,
        record_ids: handoff.record_ids,
        chain_id: handoff.chain_id,
        maybe_sent: handoff.maybe_sent,
        submit_block: handoff.submit_block,
        admitted: handoff.admitted,
    })
}

/// Forward the tracker's entry for the in-flight op to the sign machine, as
/// every shell does whenever it changes (RJ4).
fn forward(
    sign: &mut DomainDriver<SignRequest>,
    tracker: &DomainDriver<TxTracker>,
    now_ms: f64,
) -> Vec<SignOp> {
    let entry = tracker.view().entries[0].clone();
    sign.dispatch(SignEvent::OpTracked {
        user_op_hash: entry.user_op_hash,
        status: entry.status,
        tx_hash: entry.tx_hash,
        now_ms,
    })
}

/// A dApp's plain send through the write-ahead (RJ1), accepted by the relay:
/// both hand-offs fed to the tracker, the first receipt poll `pending`.
fn accepted_through_the_write_ahead() -> (DomainDriver<SignRequest>, DomainDriver<TxTracker>) {
    let mut sign = approved();
    let ops = sign.dispatch(SignEvent::OpSigned {
        id: "req-1".to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        submit_block: Some(SUBMIT_BLOCK),
        now_ms: NOW + 3_000.0,
    });
    assert!(
        matches!(ops.as_slice(), [SignOp::PersistRecord { .. }]),
        "{ops:?}"
    );
    let mut tracker = DomainDriver::<TxTracker>::new();
    feed_handoff(&sign, &mut tracker);
    tracker.resolve(TrackRes::Clock {
        now_ms: NOW + 3_000.0,
    });
    tracker.resolve(TrackRes::ReceiptPending {
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: NOW + 3_300.0,
    });
    let ops = sign.resolve_matching(
        |op| matches!(op, SignOp::PersistRecord { .. }),
        SignRes::RecordPersisted,
    );
    assert!(
        matches!(ops.as_slice(), [SignOp::ClearToPost { .. }]),
        "{ops:?}"
    );
    sign.resolve_matching(
        |op| matches!(op, SignOp::ClearToPost { .. }),
        SignRes::Responded,
    );
    sign.dispatch(SignEvent::OpSubmitted {
        id: "req-1".to_owned(),
        user_op_hash: LOCAL_OP.to_owned(),
        now_ms: NOW + 4_000.0,
        maybe_sent: false,
        submit_block: Some(SUBMIT_BLOCK),
    });
    let ops = feed_handoff(&sign, &mut tracker);
    assert_eq!(ops, vec![TrackOp::Now], "the admitted hand-off: {ops:?}");
    tracker.resolve(TrackRes::Clock {
        now_ms: NOW + 4_000.0,
    });
    (sign, tracker)
}

/// An on-chain revert inside the answer window (ruling 9) — a swap whose
/// slippage check failed between the relay's simulation and the block, or a
/// Safe call reverting inside the op. The relay marks it `rejected` and
/// names the bundle tx; its status can answer before its receipt does. The
/// page's one answer is the tx hash, never "the network refused this
/// transaction; nothing was sent" for an op that is on chain and spent gas.
#[test]
fn an_on_chain_revert_the_relay_calls_rejected_is_answered_the_revert() {
    let (mut sign, mut tracker) = accepted_through_the_write_ahead();
    let ops = tick(&mut tracker, NOW + 15_400.0);
    assert!(
        ops.iter()
            .any(|op| matches!(op, TrackOp::PollStatus { .. })),
        "{ops:?}"
    );
    tracker.resolve_matching(
        |op| matches!(op, TrackOp::PollStatus { .. }),
        TrackRes::Status {
            user_op_hash: LOCAL_OP.to_owned(),
            status: TrackLifecycle::Rejected,
            stage: None,
            now_ms: NOW + 15_600.0,
            tx_hash: Some(EVENT_TX.to_owned()),
        },
    );
    let early = forward(&mut sign, &tracker, NOW + 15_700.0);
    assert!(
        early
            .iter()
            .all(|op| !matches!(op, SignOp::SendResponse { .. })),
        "no answer before the chain has spoken: {early:?}"
    );
    // The bundle tx's receipt: the op's own event, `success = false`.
    let data = format!("0x{:064x}{:064x}{:064x}{:064x}", 22, 0, 0x359a5, 0x359a5);
    let receipt = serde_json::json!({
        "transactionHash": EVENT_TX,
        "status": "0x1",
        "logs": [{
            "address": ENTRY_POINT.to_lowercase(),
            "topics": [
                USER_OPERATION_EVENT_TOPIC,
                LOCAL_OP,
                format!("0x000000000000000000000000{}", &ME[2..]),
                "0x0000000000000000000000000000000000000000000000000000000000000000"
            ],
            "data": data,
            "transactionHash": EVENT_TX,
        }],
    })
    .to_string();
    let asked = tracker.resolve_matching(
        |op| matches!(op, TrackOp::TxReceipt { .. }),
        TrackRes::TxReceipt {
            user_op_hash: LOCAL_OP.to_owned(),
            now_ms: NOW + 16_000.0,
            receipt_json: Some(receipt),
        },
    );
    assert!(
        asked
            .iter()
            .any(|op| matches!(op, TrackOp::HoldingsMoved { .. })),
        "gas was spent: {asked:?}"
    );
    let ops = forward(&mut sign, &tracker, NOW + 16_100.0);
    let answers: Vec<&SignResponsePayload> = ops
        .iter()
        .filter_map(|op| match op {
            SignOp::SendResponse { payload, .. } => Some(payload),
            _ => None,
        })
        .collect();
    assert_eq!(
        answers,
        vec![&SignResponsePayload::Err {
            code: -32603,
            kind: SignErrorKind::SubmitFailed,
            message: Some(reverted_detail(EVENT_TX)),
        }],
        "the revert, naming its transaction — never the hash a site reads as done (083)"
    );
    let ending = ending_of("eth_sendTransaction", answers[0], Some(LOCAL_OP)).expect("an ending");
    assert_eq!(
        ending_state(&ending, tracker.view().entries.first()),
        SignEndingState::Reverted {
            tx_hash: EVENT_TX.to_owned()
        }
    );
}

// ---------------------------------------------------------------------------
// The words the shells copy through unchanged
// ---------------------------------------------------------------------------

/// A shell stores the sign machine's kind and status and the tracker's patch
/// status as strings, and hands them to the feed: every one of them is a word
/// the feed reads, meaning the same thing.
#[test]
fn record_words_read_the_same_in_every_module() {
    let kinds = [
        (SignRecordKind::DappTx, FeedTxKind::DappTx),
        (SignRecordKind::SignTypedData, FeedTxKind::SignTypedData),
        (SignRecordKind::SignMessage, FeedTxKind::SignMessage),
    ];
    for (sign, feed) in kinds {
        assert_eq!(rewire::<_, FeedTxKind>(&sign), feed);
    }
    let statuses = [
        (SignRecordStatus::Pending, FeedTxStatus::Pending),
        (SignRecordStatus::Confirmed, FeedTxStatus::Confirmed),
    ];
    for (sign, feed) in statuses {
        assert_eq!(rewire::<_, FeedTxStatus>(&sign), feed);
    }
    let patches = [
        (TrackRecordStatus::Confirmed, FeedTxStatus::Confirmed),
        (TrackRecordStatus::Failed, FeedTxStatus::Failed),
    ];
    for (track, feed) in patches {
        assert_eq!(rewire::<_, FeedTxStatus>(&track), feed);
    }
}

// ---------------------------------------------------------------------------
// One site, one amount, wherever it is shown (EF ↔ G, C ↔ G)
// ---------------------------------------------------------------------------

/// The browser bar and the Activity row name a site with the same
/// `host[:port]`: lower case, a non-default port kept, a default one dropped.
#[test]
fn the_bar_and_the_activity_row_name_a_site_alike() {
    for origin in [
        "https://app.uniswap.org",
        "https://App.Uniswap.ORG:443",
        "http://127.0.0.1:8137",
        "http://localhost:80",
        "https://app.example:8443",
        "http://192.168.1.20:3000",
    ] {
        let bar = address_bar(Some(&format!("{origin}/swap?x=1")), None, None);
        assert_eq!(dapp_site(origin), Some(bar.host), "{origin}");
    }
}

/// A dApp's plain send: the card on the sheet and the row in Activity state
/// the same exact amount of the native coin (the card adds the locale's
/// grouping marks; the row's number is the same one). A zero send has a card
/// that says "0" and a row with no amount.
#[test]
fn the_plain_send_card_and_the_activity_row_state_one_amount() {
    let locale = ClearLocale::default();
    for (value, amount) in [
        ("0x38d7ea4c68000", "0.001"),
        ("0x1", "0.000000000000000001"),
        ("0xde0b6b3a7640000", "1"),
        ("0x1056e0f39c37a5c9b8000", "1234567.891"),
    ] {
        let card = plain_send_of(Some(TO), Some(value), &locale).expect("a card");
        assert_eq!(card.amount.replace(',', ""), amount, "{value}");
        assert_eq!(native_amount(value).as_deref(), Some(amount), "{value}");
    }
    let card = plain_send_of(Some(TO), Some("0x0"), &locale).expect("a card");
    assert!(card.no_value);
    assert_eq!(native_amount("0x0"), None);
}

// ---------------------------------------------------------------------------
// Every key handed to a shell is in the corpus (EF, G ↔ I18N)
// ---------------------------------------------------------------------------

/// Every locale's catalog as `resources.ts` builds it: the flat file, then
/// each namespace file spread over it.
fn catalogs() -> Vec<(String, Value)> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/locales");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(root).expect("the locales") {
        let dir = entry.expect("an entry").path();
        if !dir.is_dir() {
            continue;
        }
        let locale = dir
            .file_name()
            .and_then(|name| name.to_str())
            .expect("a locale")
            .to_owned();
        let mut merged = serde_json::Map::new();
        let flat = dir.with_extension("json");
        let mut files = vec![flat];
        let mut spaces: Vec<_> = std::fs::read_dir(&dir)
            .expect("the namespaces")
            .map(|entry| entry.expect("a namespace").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
            .collect();
        spaces.sort();
        files.extend(spaces);
        for file in files {
            let text = std::fs::read_to_string(&file).expect("a catalog file");
            let Value::Object(tree) = serde_json::from_str(&text).expect("json") else {
                unreachable!("{} is an object", file.display())
            };
            merged.extend(tree);
        }
        out.push((locale, Value::Object(merged)));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

/// The keys `browser_load`, `sim_outcome` and `activity_feed` hand a shell
/// to draw — each a sentence in every one of the 15 locales, not an echo.
#[test]
fn every_key_the_082_core_hands_a_shell_is_in_all_15_locales() {
    let mut keys: Vec<String> = [
        LoadFailureClass::Offline,
        LoadFailureClass::Timeout,
        LoadFailureClass::NotFound,
        LoadFailureClass::Refused,
        LoadFailureClass::Certificate,
        LoadFailureClass::Other,
        LoadFailureClass::Proxy,
    ]
    .into_iter()
    .map(|class| browser_load::reason_key(class).to_owned())
    .collect();
    keys.push(browser_load::stalled().reason_key);
    keys.extend(
        [
            sim_outcome::KEY_WILL_FAIL_REASON,
            sim_outcome::KEY_WILL_FAIL,
            sim_outcome::KEY_UNAVAILABLE,
            activity_feed::HISTORY_EMPTY_ALL,
            activity_feed::HISTORY_EMPTY_FILTERED,
            activity_feed::HOME_EMPTY_ALL,
            activity_feed::HOME_EMPTY_FILTERED,
        ]
        .map(str::to_owned),
    );

    let catalogs = catalogs();
    assert_eq!(catalogs.len(), 15, "the 15 locales");
    for (locale, catalog) in &catalogs {
        for key in &keys {
            let leaf = key
                .split('.')
                .try_fold(catalog, |node, segment| node.get(segment));
            let sentence = leaf.and_then(Value::as_str).unwrap_or_default();
            assert!(!sentence.trim().is_empty(), "{locale}: {key}");
        }
    }
    let reason = catalogs
        .iter()
        .map(|(_, catalog)| {
            sim_outcome::KEY_WILL_FAIL_REASON
                .split('.')
                .try_fold(catalog, |node, segment| node.get(segment))
                .and_then(Value::as_str)
                .unwrap_or_default()
        })
        .filter(|sentence| sentence.contains("{{reason}}"))
        .count();
    assert_eq!(reason, 15, "every locale fills the revert reason in");
}
