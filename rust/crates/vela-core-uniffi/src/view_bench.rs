//! Measures what one machine costs per answer on the uniffi path: the time
//! spent inside the bridge (update, view, serialization) and the bytes the
//! shell then has to parse, decode and draw. Two machines, the two the iOS
//! launch profile named: the activity feed with 200 rows and its name
//! lookups, and the RPC pool under a burst of calls.
//!
//! Not a gate: ignored by default, run in release:
//!
//!   cargo test --release -p vela-core-uniffi view_bench -- --ignored --nocapture
//!
//! Every scenario is deterministic (no clock, no randomness), so two builds
//! answer the same events with the same effects and the figures compare.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::onboarding_bridge::{ActivityFeedCore, RpcPoolCore};

const ADDR: &str = "0x00000000000000000000000000000000000000aa";
const NOW_MS: f64 = 1_760_000_000_000.0;

#[derive(Default)]
struct Tally {
    answers: u32,
    with_view: u32,
    bytes: usize,
    elapsed: Duration,
}

impl std::fmt::Display for Tally {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:>5} answers, {:>5} with a view, {:>9} bytes, {:>9.3} ms in the bridge",
            self.answers,
            self.with_view,
            self.bytes,
            self.elapsed.as_secs_f64() * 1000.0
        )
    }
}

/// One machine as the bench drives it.
struct Driven<'a> {
    dispatch: &'a dyn Fn(&str) -> String,
    resolve: &'a dyn Fn(u64, &str) -> String,
    answer: &'a dyn Fn(&Value) -> Option<Value>,
    queue: VecDeque<(u64, Value)>,
    tally: Tally,
}

impl Driven<'_> {
    fn take(&mut self, call: impl FnOnce() -> String) {
        let started = Instant::now();
        let text = call();
        self.tally.elapsed += started.elapsed();
        self.tally.answers += 1;
        self.tally.bytes += text.len();
        let answer: Value = serde_json::from_str(&text).expect("an answer is JSON");
        if answer.get("view").is_some_and(|view| !view.is_null()) {
            self.tally.with_view += 1;
        }
        for effect in answer["effects"].as_array().expect("effects") {
            let id = effect["id"].as_u64().expect("effect id");
            self.queue.push_back((id, effect["operation"].clone()));
        }
    }

    fn send(&mut self, event: &Value) {
        let dispatch = self.dispatch;
        let text = event.to_string();
        self.take(|| dispatch(&text));
    }

    /// Answer every operation, oldest first, until nothing is outstanding.
    fn drain(&mut self) {
        while let Some((id, operation)) = self.queue.pop_front() {
            let Some(result) = (self.answer)(&operation) else {
                continue;
            };
            let resolve = self.resolve;
            let text = result.to_string();
            self.take(|| resolve(id, &text));
        }
    }

    fn lap(&mut self) -> Tally {
        std::mem::take(&mut self.tally)
    }
}

fn day_of(ts_seconds: f64) -> f64 {
    (ts_seconds / 86_400.0).floor() * 86_400_000.0
}

/// 200 settled records, half in and half out, every counterparty different
/// and none of them named: 200 rows, 200 name lookups.
fn feed_records() -> Value {
    let mut records = Vec::new();
    for i in 0..200_u32 {
        let incoming = i % 2 == 0;
        let party = format!("0x{:040x}", 0x1000 + i);
        let ts = 1_759_000_000.0 - f64::from(i) * 21_600.0;
        records.push(json!({
            "id": format!("rec-{i}"),
            "user_op_hash": if incoming { String::new() } else { format!("0x{:064x}", i) },
            "tx_hash": format!("0x{:064x}", 0x10_000 + i),
            "from": if incoming { party.clone() } else { ADDR.to_owned() },
            "to": if incoming { ADDR.to_owned() } else { party.clone() },
            "to_name": null,
            "value": format!("{}", 1_000_000 + i),
            "symbol": "USDC",
            "decimals": 6,
            "logo_urls": null,
            "chain_id": 8453,
            "timestamp": ts,
            "day_start_ms": day_of(ts),
            "status": "confirmed",
            "kind": if incoming { "receive" } else { "send" },
            "usd": format!("{}.00", 1 + i % 7),
            "time_verified": true
        }));
    }
    Value::Array(records)
}

fn feed_answer(records: &Value) -> impl Fn(&Value) -> Option<Value> + '_ {
    move |op: &Value| {
        Some(match op["type"].as_str()? {
            "read_tx_store" => json!({
                "type": "store_loaded",
                "records": records,
                "now_ms": NOW_MS,
                "read_id": op["read_id"]
            }),
            "scan_incoming_transfers" => json!({ "type": "sync_completed", "new_count": 0 }),
            "resolve_recipient_identity" => {
                let addr = op["addr"].as_str()?;
                json!({
                    "type": "alias_resolved",
                    "addr": addr,
                    "name": format!("name-{}", &addr[addr.len() - 4..])
                })
            }
            "read_receive_time" => {
                json!({ "type": "receive_time_read", "id": op["id"], "timestamp_sec": null })
            }
            "write_receive_time" => {
                json!({ "type": "receive_time_written", "id": op["id"], "ok": true })
            }
            "timer" => json!({ "type": "toast_expired", "generation": op["generation"] }),
            "haptic" => json!({ "type": "haptic_played" }),
            "delete_tx_record" => json!({ "type": "delete_committed", "id": op["id"] }),
            other => panic!("the feed asked for {other}"),
        })
    }
}

/// The feed on launch: the account, its 200 records, one name answer at a
/// time; then ten live ticks over the same store (nothing changes).
fn bench_feed(opt_in: Option<&dyn Fn(&ActivityFeedCore)>) -> (Tally, Tally) {
    let core = ActivityFeedCore::new();
    if let Some(opt_in) = opt_in {
        opt_in(&core);
    }
    let records = feed_records();
    let answer = feed_answer(&records);
    let dispatch = |event: &str| core.dispatch(event.to_owned()).expect("dispatch");
    let resolve =
        |id: u64, result: &str| core.resolve_effect(id, result.to_owned()).expect("resolve");
    let mut driven = Driven {
        dispatch: &dispatch,
        resolve: &resolve,
        answer: &answer,
        queue: VecDeque::new(),
        tally: Tally::default(),
    };
    driven.send(&json!({ "type": "account_switched", "address": ADDR }));
    driven.drain();
    let launch = driven.lap();
    for _ in 0..10 {
        driven.send(&json!({ "type": "live_tick" }));
        driven.drain();
    }
    (launch, driven.lap())
}

fn pool_answer(op: &Value) -> Option<Value> {
    let endpoints: Vec<Value> = (0..6)
        .map(|i| json!({ "url": format!("https://rpc-{i}.example"), "source": "public" }))
        .collect();
    Some(match op["type"].as_str()? {
        "load_pool_config" => json!({
            "type": "pool_config",
            "chain_id": op["chain_id"],
            "rpc_endpoints": endpoints,
            "bundler_endpoints": [],
            "now_ms": NOW_MS
        }),
        "probe_chain_id" => json!({
            "type": "chain_id_probed",
            "chain_id": op["chain_id"],
            "url": op["url"],
            "reported": op["chain_id"],
            "latency_ms": 40.0,
            "now_ms": NOW_MS
        }),
        "json_rpc_post" => {
            // Every fifth call times out on its first endpoint and fails
            // over, so the queue of what is left to try moves as it does
            // on a busy network.
            let call: u32 = op["call_id"].as_str()?[1..].parse().ok()?;
            let first = op["url"].as_str()? == "https://rpc-0.example";
            let outcome = if call % 5 == 0 && first {
                json!({ "type": "timeout" })
            } else {
                json!({ "type": "response", "error": null })
            };
            json!({
                "type": "post_outcome",
                "call_id": op["call_id"],
                "url": op["url"],
                "outcome": outcome,
                "latency_ms": 35.0,
                "now_ms": NOW_MS
            })
        }
        "draw_jitter" => json!({ "type": "jitter", "call_id": op["call_id"], "value": 0.5 }),
        "start_backoff" => {
            json!({ "type": "backoff_elapsed", "call_id": op["call_id"], "now_ms": NOW_MS })
        }
        "persist_bans" => json!({ "type": "persisted" }),
        "conclude" => json!({ "type": "concluded" }),
        other => panic!("the pool asked for {other}"),
    })
}

/// 200 calls on one chain, twenty at a time.
fn bench_pool(opt_in: Option<&dyn Fn(&RpcPoolCore)>) -> Tally {
    let core = RpcPoolCore::new();
    if let Some(opt_in) = opt_in {
        opt_in(&core);
    }
    let dispatch = |event: &str| core.dispatch(event.to_owned()).expect("dispatch");
    let resolve =
        |id: u64, result: &str| core.resolve_effect(id, result.to_owned()).expect("resolve");
    let mut driven = Driven {
        dispatch: &dispatch,
        resolve: &resolve,
        answer: &pool_answer,
        queue: VecDeque::new(),
        tally: Tally::default(),
    };
    driven.send(&json!({ "type": "bans_loaded", "entries": [] }));
    for burst in 0..10_u32 {
        for i in 0..20_u32 {
            driven.send(&json!({
                "type": "call_requested",
                "call_id": format!("c{}", burst * 20 + i),
                "chain_id": 1,
                "kind": "rpc",
                "method": "eth_call",
                "now_ms": NOW_MS
            }));
        }
        driven.drain();
    }
    driven.lap()
}

type Laps = Vec<(&'static str, Tally)>;

fn report(label: &str, runs: usize, mut run: impl FnMut() -> Laps) {
    // One warm-up, then the median of `runs` by bridge time.
    run();
    let mut laps: Vec<Laps> = (0..runs).map(|_| run()).collect();
    let parts = laps[0].len();
    for part in 0..parts {
        laps.sort_by_key(|lap| lap[part].1.elapsed);
        let (name, tally) = &laps[runs / 2][part];
        println!("{label:<26} {name:<34} {tally}");
    }
}

#[test]
#[ignore = "a bench: run in release with --ignored --nocapture"]
fn view_bench() {
    report("activity feed (200 rows)", 9, || {
        let (launch, ticks) = bench_feed(None);
        vec![
            ("launch + 200 names", launch),
            ("10 unchanged ticks", ticks),
        ]
    });
    report("rpc pool (200 calls)", 9, || {
        vec![("200 calls", bench_pool(None))]
    });
    // The same, from a shell that opted in (`skip_unchanged_views`).
    report("feed, skipping unchanged", 9, || {
        let (launch, ticks) = bench_feed(Some(&|core: &ActivityFeedCore| {
            core.skip_unchanged_views(false).expect("opt in");
        }));
        vec![
            ("launch + 200 names", launch),
            ("10 unchanged ticks", ticks),
        ]
    });
    report("pool, skipping unchanged", 9, || {
        let quiet = bench_pool(Some(&|core: &RpcPoolCore| {
            core.skip_unchanged_views(false).expect("opt in");
        }));
        let hedging = bench_pool(Some(&|core: &RpcPoolCore| {
            core.skip_unchanged_views(true).expect("opt in");
        }));
        vec![("200 calls", quiet), ("200 calls, + pending_urls", hedging)]
    });
}
