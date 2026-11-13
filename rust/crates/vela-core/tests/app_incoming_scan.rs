//! r9-17 / r8-13: the incoming-funds scan as one core rule.
//!
//! The shared vectors (`vectors/incoming-scan.json`: facts → networks,
//! head + coverage → windows, presence → cadence) replayed through the pure
//! rules and their JSON doors; then the machines that run them —
//! `token_trust`'s planned scan (`ScanRequested`: the networks, the live
//! window, the bounded backfill, the coverage handed back) and
//! `activity_feed`'s cadence (`PresenceChanged` / `ScanDue` / `Interacting`,
//! the shared `cadence_steps`) and its one scan per account at a time.

#![cfg(feature = "crux")]

mod support;

use serde_json::Value;
use support::DomainDriver;
use vela_core::app::activity_feed::{
    ActivityFeed, Event as FeedEvent, FeedOperation as FeedOp, FeedPresence, FeedPresenceChange,
    FeedShellResult as FeedRes,
};
use vela_core::app::incoming_scan::{
    self, next_scan_ms, scan_plan, scan_plan_json, scan_windows_json, IncomingScanPlan,
    IncomingScanWindows, TrustAccountNetwork, TrustBlockSpan, TrustScanCursor, TrustScanFacts,
};
use vela_core::app::token_trust::{
    address_topic, Event, TokenTrust, TrustLogsOutcome, TrustOperation as Op, TrustRawLog,
    TrustShellResult as Res, NATIVE_LOG_ADDRESSES, SAFE_RECEIVED_TOPIC, TIMESTAMP_BLOCK_CAP,
    TRANSFER_TOPIC,
};

// ===========================================================================
// The shared vectors
// ===========================================================================

fn vectors() -> Value {
    let path = format!(
        "{}/tests/vectors/incoming-scan.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the vectors")).expect("JSON")
}

#[test]
fn the_vectors_state_the_cores_constants() {
    let v = vectors();
    let c = &v["constants"];
    let n = |k: &str| c[k].as_u64().unwrap_or_else(|| panic!("constant {k}"));
    assert_eq!(
        c["cursor_key_prefix"].as_str(),
        Some(incoming_scan::CURSOR_KEY_PREFIX)
    );
    assert_eq!(n("live_scan_blocks"), incoming_scan::LIVE_SCAN_BLOCKS);
    assert_eq!(
        n("backfill_lookback_secs"),
        incoming_scan::BACKFILL_LOOKBACK_SECS
    );
    assert_eq!(n("backfill_max_blocks"), incoming_scan::BACKFILL_MAX_BLOCKS);
    assert_eq!(
        n("backfill_chunk_blocks"),
        incoming_scan::BACKFILL_CHUNK_BLOCKS
    );
    assert_eq!(
        n("backfill_chunks_per_poll"),
        incoming_scan::BACKFILL_CHUNKS_PER_POLL as u64
    );
    let steps: Vec<u32> = c["cadence_steps_ms"]
        .as_array()
        .expect("cadence_steps_ms")
        .iter()
        .map(|s| s.as_u64().expect("a step") as u32)
        .collect();
    assert_eq!(steps, incoming_scan::CADENCE_STEPS_MS);
    assert_eq!(
        n("cadence_hidden_ms"),
        u64::from(incoming_scan::CADENCE_HIDDEN_MS)
    );
    assert_eq!(
        n("cadence_interacting_max_ms") as f64,
        incoming_scan::CADENCE_INTERACTING_MAX_MS
    );
    assert_eq!(
        n("cadence_scan_lost_ms") as f64,
        incoming_scan::CADENCE_SCAN_LOST_MS
    );
}

#[test]
fn every_plan_vector_holds_through_the_rule_and_its_door() {
    let v = vectors();
    let plans = v["plans"].as_array().expect("plans");
    assert!(plans.len() >= 8);
    for case in plans {
        let name = case["name"].as_str().unwrap_or("?");
        let facts: TrustScanFacts = serde_json::from_value(case["facts"].clone()).expect("facts");
        let expect: IncomingScanPlan =
            serde_json::from_value(case["expect"].clone()).expect("expect");
        assert_eq!(scan_plan(&facts), expect, "{name}");
        let door: IncomingScanPlan =
            serde_json::from_str(&scan_plan_json(&case["facts"].to_string())).expect("door");
        assert_eq!(door, expect, "{name} (door)");
    }
}

#[test]
fn every_window_vector_holds_through_the_door() {
    let v = vectors();
    let windows = v["windows"].as_array().expect("windows");
    assert!(windows.len() >= 8);
    for case in windows {
        let name = case["name"].as_str().unwrap_or("?");
        let expect: IncomingScanWindows =
            serde_json::from_value(case["expect"].clone()).expect("expect");
        let got: IncomingScanWindows =
            serde_json::from_str(&scan_windows_json(&case["ask"].to_string())).expect("door");
        assert_eq!(got, expect, "{name}");
        // Every read is inside [floor, head], none overlaps the live window
        // or another, and none is longer than the chunk.
        let chunk = case["ask"]["chunk"]
            .as_u64()
            .unwrap_or(incoming_scan::BACKFILL_CHUNK_BLOCKS);
        let (live_from, _) = got.live.blocks().expect("live");
        let mut below = live_from;
        for span in &got.backfill {
            let (from, to) = span.blocks().expect("span");
            assert!(from as f64 >= got.floor, "{name}: below the floor");
            assert!(to < below, "{name}: not newest first / overlapping");
            assert!(to - from < chunk, "{name}: longer than the chunk");
            below = from;
        }
    }
}

#[test]
fn every_cadence_vector_holds() {
    let v = vectors();
    for case in v["cadence"].as_array().expect("cadence") {
        let name = case["name"].as_str().unwrap_or("?");
        let got = next_scan_ms(
            case["visible"].as_bool().expect("visible"),
            case["quiet_scans"].as_u64().expect("quiet_scans") as u32,
        );
        assert_eq!(
            u64::from(got),
            case["expect_ms"].as_u64().expect("ms"),
            "{name}"
        );
    }
}

// ===========================================================================
// token_trust: the planned scan
// ===========================================================================

type Trust = DomainDriver<TokenTrust>;

/// A multisig, mixed case as a shell might hand it.
const SAFE: &str = "0x5AfE00000000000000000000000000000000aBcD";
const SAFE_LC: &str = "0x5afe00000000000000000000000000000000abcd";
const PAYER: &str = "0xd9c79c0000000000000000000000000000000001";
const ARC: u32 = 5_042_002;
const HEAD: u64 = 31_000_000;
const TESTNET: [u32; 8] = [
    11_155_111, 84_532, 421_614, 11_155_420, 97, 80_002, 10_200, ARC,
];

fn multisig_facts(deploy_block: Option<f64>) -> TrustScanFacts {
    TrustScanFacts {
        held_chain_ids: vec![],
        account_networks: vec![TrustAccountNetwork {
            chain_id: ARC,
            deploy_block,
        }],
        space_chain_ids: TESTNET.to_vec(),
        added_chain_ids: vec![],
    }
}

fn hex(n: u64) -> String {
    format!("0x{n:x}")
}

/// A native coin paid to the multisig, as its own `SafeReceived` log.
fn safe_received(block: u64, value: u128) -> TrustRawLog {
    TrustRawLog {
        address: SAFE_LC.to_owned(),
        topics: vec![SAFE_RECEIVED_TOPIC.to_owned(), address_topic(PAYER)],
        data: format!("0x{value:064x}"),
        transaction_hash: format!("0x{block:064x}"),
        block_number: Some(hex(block)),
        log_index: Some("0x0".to_owned()),
    }
}

/// An EIP-7708 native `Transfer` to the multisig.
fn native_transfer(block: u64, idx: u32) -> TrustRawLog {
    TrustRawLog {
        address: NATIVE_LOG_ADDRESSES[0].to_owned(),
        topics: vec![
            TRANSFER_TOPIC.to_owned(),
            address_topic(PAYER),
            address_topic(SAFE),
        ],
        data: format!("0x{:064x}", 7_u128),
        transaction_hash: format!("0x{block:060x}{idx:04x}"),
        block_number: Some(hex(block)),
        log_index: Some(hex(u64::from(idx))),
    }
}

fn logs(chain_id: u32, outcome: TrustLogsOutcome) -> Res {
    Res::Logs {
        address: SAFE_LC.to_owned(),
        chain_id,
        outcome,
    }
}

fn received(chain_id: u32, outcome: TrustLogsOutcome) -> Res {
    Res::SafeReceivedLogs {
        address: SAFE_LC.to_owned(),
        chain_id,
        outcome,
    }
}

fn ok(logs: Vec<TrustRawLog>) -> TrustLogsOutcome {
    TrustLogsOutcome::Ok { logs }
}

/// The span a transfer-logs read asks for.
fn span_of(op: &Op) -> (u64, u64) {
    match op {
        Op::RpcGetLogs {
            from_block,
            to_block,
            ..
        } => (
            u64::from_str_radix(&from_block[2..], 16).expect("from"),
            u64::from_str_radix(&to_block[2..], 16).expect("to"),
        ),
        other => panic!("expected a transfer-logs read, got {other:?}"),
    }
}

/// Start a planned scan of the multisig and answer up to its head on Arc;
/// returns the live window's read.
fn start(sut: &mut Trust, facts: TrustScanFacts, cursors: Vec<TrustScanCursor>) -> Op {
    let ops = sut.dispatch(Event::ScanRequested {
        address: SAFE.to_owned(),
        facts,
        cursors,
    });
    assert_eq!(ops, vec![Op::ReadCustomTokens]);
    let ops = sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    assert_eq!(
        ops,
        vec![Op::RpcBlockNumber {
            address: SAFE_LC.to_owned(),
            chain_id: ARC
        }],
        "r9-17: a multisig holding nothing scans the network it lives on — only it"
    );
    let ops = sut.resolve(Res::BlockNumber {
        address: SAFE_LC.to_owned(),
        chain_id: ARC,
        block_hex: Some(hex(HEAD)),
    });
    assert_eq!(ops.len(), 1);
    ops[0].clone()
}

/// Answer one window (transfer logs, then `SafeReceived`); returns what the
/// scan asks next.
fn window(sut: &mut Trust, transfers: Vec<TrustRawLog>, own: Vec<TrustRawLog>) -> Vec<Op> {
    let ops = sut.resolve(logs(ARC, ok(transfers)));
    assert!(
        matches!(ops.as_slice(), [Op::RpcGetSafeReceivedLogs { .. }]),
        "{ops:?}"
    );
    sut.resolve(received(ARC, ok(own)))
}

fn covered(sut: &Trust) -> Vec<(u64, u64)> {
    sut.view()
        .cursors
        .iter()
        .find(|c| c.chain_id == ARC)
        .map(|c| {
            c.covered
                .iter()
                .filter_map(TrustBlockSpan::blocks)
                .collect()
        })
        .unwrap_or_default()
}

/// r9-17: the receipt that arrived before the scan first ran — "Received
/// +0.6" on a multisig funded before this device scanned it — is found by
/// the backfill, with its block's own time, and the coverage is handed back.
#[test]
fn a_multisig_receipt_from_before_the_first_scan_is_backfilled() {
    let mut sut = Trust::new();
    let live = start(&mut sut, multisig_facts(None), vec![]);
    assert_eq!(span_of(&live), (HEAD - 100, HEAD), "the live window first");

    // The live window: nothing. The backfill's first read follows, newest
    // first, at the chunk.
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (HEAD - 10_100, HEAD - 101));
    // The payment, 5 000 blocks before the head — long out of the live
    // window — is in it, as the Safe's own log.
    let paid = HEAD - 5_000;
    let ops = window(&mut sut, vec![], vec![safe_received(paid, 600_000)]);
    assert_eq!(span_of(&ops[0]), (HEAD - 20_100, HEAD - 10_101));
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (HEAD - 30_100, HEAD - 20_101));
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (HEAD - 40_100, HEAD - 30_101));
    // Four reads a poll: then the timestamps of what was found.
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(
        ops,
        vec![Op::RpcGetBlockByNumber {
            address: SAFE_LC.to_owned(),
            chain_id: ARC,
            block: hex(paid),
        }]
    );
    sut.resolve(Res::BlockTimestamp {
        address: SAFE_LC.to_owned(),
        chain_id: ARC,
        block_number: paid as f64,
        timestamp_sec: Some(1_790_000_000.0),
    });
    let view = sut.view();
    assert!(!view.scanning);
    assert_eq!(view.incoming.len(), 1);
    assert_eq!(view.incoming[0].value, "600000");
    assert_eq!(view.incoming[0].timestamp_sec, 1_790_000_000.0);
    // Covered: the live window and the four reads below it, one span.
    assert_eq!(covered(&sut), vec![(HEAD - 40_100, HEAD)]);
}

/// A restart hands the kept coverage back: the next poll reads only what is
/// still open, and never a span twice.
#[test]
fn a_restart_resumes_below_the_kept_coverage() {
    let mut sut = Trust::new();
    let kept = TrustScanCursor {
        chain_id: ARC,
        covered: vec![TrustBlockSpan::of(HEAD - 300_000, HEAD - 50)],
    };
    let live = start(&mut sut, multisig_facts(None), vec![kept]);
    assert_eq!(span_of(&live), (HEAD - 100, HEAD));
    let ops = window(&mut sut, vec![], vec![]);
    // Floor: a week of 2 s blocks (302 400) below the head; open: the two
    // thousand-odd blocks between it and the kept span.
    let floor = HEAD - 302_400;
    assert_eq!(span_of(&ops[0]), (floor, HEAD - 300_001));
    let ops = window(&mut sut, vec![], vec![]);
    assert!(ops.is_empty(), "nothing else is open: {ops:?}");
    assert!(!sut.view().scanning);
    assert_eq!(covered(&sut), vec![(floor, HEAD)]);
}

/// Never below the deploy block: nothing can have reached the multisig
/// before it existed.
#[test]
fn the_backfill_stops_at_the_deploy_block() {
    let mut sut = Trust::new();
    let deployed = HEAD - 3_000;
    start(&mut sut, multisig_facts(Some(deployed as f64)), vec![]);
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (deployed, HEAD - 101));
    let ops = window(&mut sut, vec![], vec![]);
    assert!(ops.is_empty());
    assert_eq!(covered(&sut), vec![(deployed, HEAD)]);
}

/// An endpoint that caps the span: the read's span stays open, the scan
/// shrinks its reads to the cap, and the next poll reads it at that size.
#[test]
fn a_capped_backfill_read_stays_open_and_shrinks_to_the_cap() {
    let mut sut = Trust::new();
    start(&mut sut, multisig_facts(None), vec![]);
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (HEAD - 10_100, HEAD - 101));
    let ops = sut.resolve(logs(ARC, TrustLogsOutcome::RangeCapped { cap: 2_000 }));
    assert!(ops.is_empty(), "the poll ends with what it has: {ops:?}");
    assert!(!sut.view().scanning);
    assert_eq!(covered(&sut), vec![(HEAD - 100, HEAD)]);

    // The next poll (the head moved 10 blocks): reads of 2 000 blocks, from
    // just below what is covered.
    sut.dispatch(Event::ScanRequested {
        address: SAFE.to_owned(),
        facts: multisig_facts(None),
        cursors: vec![],
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(Res::BlockNumber {
        address: SAFE_LC.to_owned(),
        chain_id: ARC,
        block_hex: Some(hex(HEAD + 10)),
    });
    let ops = window(&mut sut, vec![], vec![]);
    assert_eq!(span_of(&ops[0]), (HEAD - 2_100, HEAD - 101));
}

/// A span whose native receipts could not be read is not covered: a later
/// poll reads it again (the transfers it did find still land).
#[test]
fn a_window_without_its_native_receipts_stays_open() {
    let mut sut = Trust::new();
    start(&mut sut, multisig_facts(None), vec![]);
    let ops = sut.resolve(logs(ARC, ok(vec![])));
    assert!(matches!(
        ops.as_slice(),
        [Op::RpcGetSafeReceivedLogs { .. }]
    ));
    let ops = sut.resolve(received(ARC, TrustLogsOutcome::Failed));
    assert_eq!(span_of(&ops[0]), (HEAD - 10_100, HEAD - 101));
    assert!(covered(&sut).is_empty(), "the live window is not covered");
}

/// One poll finds at most what it can time: past that, the backfill waits
/// for the next poll instead of finding transfers it would drop.
#[test]
fn the_backfill_pauses_once_a_poll_has_found_what_it_can_time() {
    let mut sut = Trust::new();
    start(&mut sut, multisig_facts(None), vec![]);
    let many: Vec<TrustRawLog> = (0..TIMESTAMP_BLOCK_CAP as u64)
        .map(|i| native_transfer(HEAD - 50 + i, 0))
        .collect();
    let ops = window(&mut sut, many, vec![]);
    assert!(
        ops.iter()
            .all(|op| matches!(op, Op::RpcGetBlockByNumber { .. })),
        "straight to the timestamps: {ops:?}"
    );
    assert_eq!(ops.len(), TIMESTAMP_BLOCK_CAP);
}

/// A shell not moved yet: `HeldChainsSnapshot` + `PollRequested` reads the
/// live window only and its view carries no cursors — on the wire, nothing.
#[test]
fn the_legacy_poll_is_the_live_window_alone() {
    let mut sut = Trust::new();
    sut.dispatch(Event::HeldChainsSnapshot {
        address: SAFE.to_owned(),
        chain_ids: vec![ARC],
    });
    sut.dispatch(Event::PollRequested {
        address: SAFE.to_owned(),
    });
    sut.resolve(Res::CustomTokens {
        tokens: Some(vec![]),
    });
    sut.resolve(Res::BlockNumber {
        address: SAFE_LC.to_owned(),
        chain_id: ARC,
        block_hex: Some(hex(HEAD)),
    });
    let ops = window(&mut sut, vec![], vec![]);
    assert!(ops.is_empty(), "no backfill: {ops:?}");
    let view = sut.view();
    assert!(view.cursors.is_empty());
    let wire = serde_json::to_value(&view).expect("view");
    assert!(wire.get("cursors").is_none(), "absent on the wire: {wire}");
}

/// The coverage is the account's: another account starts from nothing.
#[test]
fn coverage_belongs_to_its_account() {
    let mut sut = Trust::new();
    start(&mut sut, multisig_facts(None), vec![]);
    window(&mut sut, vec![], vec![]);
    assert!(!covered(&sut).is_empty());
    sut.dispatch(Event::ScanRequested {
        address: PAYER.to_owned(),
        facts: TrustScanFacts::default(),
        cursors: vec![],
    });
    assert!(sut.view().cursors.is_empty());
}

// ===========================================================================
// activity_feed: one scan per account, and the core's cadence
// ===========================================================================

type Feed = DomainDriver<ActivityFeed>;

const ME: &str = "0xA11ce";
const T0: f64 = 1_790_000_000_000.0;

fn is_scan(op: &FeedOp) -> bool {
    matches!(op, FeedOp::ScanIncomingTransfers { .. })
}

fn presence(visible: bool, since: f64, now: f64) -> FeedPresence {
    FeedPresence {
        visible,
        since_interaction_ms: since,
        now_ms: now,
    }
}

/// Switched to `ME`, the first read answered; the scan stays out.
fn feed() -> Feed {
    let mut sut = Feed::new();
    let ops = sut.dispatch(FeedEvent::AccountSwitched {
        address: ME.to_owned(),
    });
    assert_eq!(ops.iter().filter(|op| is_scan(op)).count(), 1);
    sut
}

/// r9-17 (desktop's one global flag): one scan of an account at a time — a
/// tick meanwhile only re-reads — and a switch never waits on the previous
/// account's scan.
#[test]
fn one_scan_per_account_at_a_time() {
    let mut sut = feed();
    let ops = sut.dispatch(FeedEvent::FocusTick);
    assert!(!ops.iter().any(is_scan), "{ops:?}");
    assert!(ops
        .iter()
        .any(|op| matches!(op, FeedOp::ReadTxStore { .. })));

    // The first account's scan is still out; the new account does not wait.
    let ops = sut.dispatch(FeedEvent::AccountSwitched {
        address: "0xB0b".to_owned(),
    });
    assert!(
        ops.iter().any(is_scan),
        "the new account scans now: {ops:?}"
    );

    // The first account's late answer is dropped with its attempt: the new
    // account's scan is still the one out.
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    let ops = sut.dispatch(FeedEvent::LiveTick);
    assert!(!ops.iter().any(is_scan), "{ops:?}");

    // Its own answer: the next tick scans again.
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    let ops = sut.dispatch(FeedEvent::LiveTick);
    assert!(ops.iter().any(is_scan), "{ops:?}");
}

/// A shell that reports no presence keeps its own ticks: no `next_scan`.
#[test]
fn no_presence_no_core_cadence() {
    let sut = feed();
    assert_eq!(sut.view().next_scan, None);
    let wire = serde_json::to_value(sut.view()).expect("view");
    assert!(wire.get("next_scan").is_none());
}

/// r10: the shared cadence steps (`cadence_steps`) through the machine —
/// the back-off, the moments a person looks, one scan at a time, the quiet
/// period while the person interacts, and one redraw per scan.
#[test]
fn every_cadence_step_vector_holds_through_the_machine() {
    let v = vectors();
    let scenarios = v["cadence_steps"].as_array().expect("cadence_steps");
    assert!(scenarios.len() >= 8);
    for scenario in scenarios {
        let name = scenario["name"].as_str().unwrap_or("?");
        let mut sut = Feed::new();
        for (i, step) in scenario["steps"]
            .as_array()
            .expect("steps")
            .iter()
            .enumerate()
        {
            let what = step["do"].as_str().expect("do");
            let at = T0 + step["at"].as_f64().expect("at") * 1_000.0;
            let shown = step["visible"].as_bool().unwrap_or(true);
            let here = presence(shown, 0.0, at);
            let before = sut.view().next_scan;
            let renders = sut.renders();
            let ops = match what {
                "account_switched" => sut.dispatch(FeedEvent::AccountSwitched {
                    address: ME.to_owned(),
                }),
                "scan_due" | "scan_due_stale" => {
                    let seq = before.expect("a timer").seq;
                    let seq = if what == "scan_due" {
                        seq
                    } else {
                        seq.wrapping_sub(1)
                    };
                    sut.dispatch(FeedEvent::ScanDue {
                        seq,
                        presence: here,
                    })
                }
                "interacting_on" | "interacting_off" => sut.dispatch(FeedEvent::Interacting {
                    on: what == "interacting_on",
                    presence: here,
                }),
                "scan_answer" => sut.resolve_matching(
                    is_scan,
                    FeedRes::SyncCompleted {
                        new_count: step["new_count"].as_u64().expect("new_count") as u32,
                        unread_chains: vec![],
                    },
                ),
                "store_answer" => {
                    let mut ops = Vec::new();
                    while sut
                        .outstanding()
                        .iter()
                        .any(|op| matches!(op, FeedOp::ReadTxStore { .. }))
                    {
                        let read_id = sut
                            .outstanding()
                            .iter()
                            .find_map(|op| match op {
                                FeedOp::ReadTxStore { read_id, .. } => Some(*read_id),
                                _ => None,
                            })
                            .expect("a read");
                        ops.extend(sut.resolve_matching(
                            |op| matches!(op, FeedOp::ReadTxStore { .. }),
                            FeedRes::StoreLoaded {
                                records: Vec::new(),
                                now_ms: at,
                                read_id,
                            },
                        ));
                    }
                    ops
                }
                presence_change => {
                    let change = presence_change
                        .strip_prefix("presence:")
                        .unwrap_or_else(|| panic!("{name} #{i}: unknown step {what}"));
                    let change: FeedPresenceChange =
                        serde_json::from_value(Value::String(change.to_owned())).expect("change");
                    sut.dispatch(FeedEvent::PresenceChanged {
                        presence: here,
                        change,
                    })
                }
            };
            let expect = &step["expect"];
            let step_name = format!("{name} — #{i} {what} at {}", step["at"]);
            assert_eq!(
                ops.iter().any(is_scan),
                expect["scan"].as_bool().expect("scan"),
                "{step_name}: scan? {ops:?}"
            );
            assert_eq!(
                ops.iter()
                    .any(|op| matches!(op, FeedOp::ReadTxStore { .. })),
                expect["read"].as_bool().expect("read"),
                "{step_name}: read? {ops:?}"
            );
            assert_eq!(
                (sut.renders() - renders) as u64,
                expect["renders"].as_u64().expect("renders"),
                "{step_name}: renders"
            );
            let after = sut.view().next_scan;
            let armed = match (before, after) {
                (Some(b), Some(a)) if a.seq == b.seq => None,
                (_, a) => a.map(|a| u64::from(a.after_ms)),
            };
            assert_eq!(armed, expect["armed"].as_u64(), "{step_name}: armed");
            // The machine renders every change (`RENDERS_EVERY_CHANGE`): the
            // shell's view is never behind.
            if sut.rendered_view().is_some() {
                assert_eq!(sut.rendered_view(), Some(&sut.view()), "{step_name}: drawn");
            }
        }
    }
}

/// r10: a shell that never said `started` follows no core cadence: the
/// quiet-period event changes nothing for it.
#[test]
fn interacting_without_the_cadence_is_nothing() {
    let mut sut = feed();
    let ops = sut.dispatch(FeedEvent::Interacting {
        on: true,
        presence: presence(true, 0.0, T0),
    });
    assert!(ops.is_empty());
    assert_eq!(sut.view().next_scan, None);
}

/// r10: a scan its shell never answered stops holding the next one after
/// five minutes — the person's next moment scans again.
#[test]
fn a_scan_never_answered_is_let_go() {
    let mut sut = feed();
    sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0),
        change: FeedPresenceChange::Started,
    });
    // The hand-off's scan is never answered.
    let ops = sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0 + 60_000.0),
        change: FeedPresenceChange::Refreshed,
    });
    assert!(!ops.iter().any(is_scan), "one is still out: {ops:?}");
    let ops = sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0 + 300_000.0),
        change: FeedPresenceChange::Refreshed,
    });
    assert!(ops.iter().any(is_scan), "let go after 5 min: {ops:?}");
}

/// r10: the account hand-off starts the back-off afresh — what was held for
/// the previous account is spent.
#[test]
fn a_new_account_starts_the_back_off_again() {
    let mut sut = feed();
    sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0),
        change: FeedPresenceChange::Started,
    });
    for k in 0..4 {
        sut.resolve_matching(
            is_scan,
            FeedRes::SyncCompleted {
                new_count: 0,
                unread_chains: vec![],
            },
        );
        let seq = sut.view().next_scan.expect("armed").seq;
        sut.dispatch(FeedEvent::ScanDue {
            seq,
            presence: presence(true, 0.0, T0 + f64::from(k) * 1_000.0),
        });
    }
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    assert_eq!(sut.view().next_scan.map(|n| n.after_ms), Some(120_000));
    sut.dispatch(FeedEvent::AccountSwitched {
        address: "0xB0b".to_owned(),
    });
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    assert_eq!(sut.view().next_scan.map(|n| n.after_ms), Some(15_000));
}

/// r15-1: under the cadence, "No activity yet" may be said (`settled`) once
/// the account's first scan is back — drawn by a render the cadence makes
/// anyway (a scan asked for again while one was out runs once more, and ITS
/// answer settles it), never a redraw of its own and never a view moved
/// without a render.
#[test]
fn the_first_scan_back_settles_the_empty_claim_on_a_render_the_cadence_makes() {
    let mut sut = feed();
    sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0),
        change: FeedPresenceChange::Started,
    });
    let read_id = sut
        .outstanding()
        .iter()
        .find_map(|op| match op {
            FeedOp::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a read");
    sut.resolve_matching(
        |op| matches!(op, FeedOp::ReadTxStore { .. }),
        FeedRes::StoreLoaded {
            records: Vec::new(),
            now_ms: T0,
            read_id,
        },
    );
    assert!(!sut.view().settled, "read, not scanned yet");
    // A send while the first scan is out: one more scan when it lands.
    sut.dispatch(FeedEvent::PresenceChanged {
        presence: presence(true, 0.0, T0 + 2_000.0),
        change: FeedPresenceChange::Sent,
    });
    let renders = sut.renders();
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    assert_eq!(
        sut.renders(),
        renders,
        "the scan that starts another draws nothing"
    );
    assert!(
        !sut.view().settled,
        "and the view has not moved without a render"
    );
    sut.resolve_matching(
        is_scan,
        FeedRes::SyncCompleted {
            new_count: 0,
            unread_chains: vec![],
        },
    );
    assert!(sut.view().settled, "the second answer settles it");
    assert_eq!(sut.rendered_view(), Some(&sut.view()), "drawn");
}
