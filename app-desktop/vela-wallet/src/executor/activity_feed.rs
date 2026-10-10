//! The only place the `activity_feed` machine touches the outside world.
//!
//! Eight operations: the local transaction store, receipt discovery, a delete,
//! a counterparty lookup, the toast timer, a haptic the desktop does not have —
//! and, since PR 3, the two halves of the repair of a receipt's time: reading
//! the time of the block that holds its transaction, and rewriting the one
//! stored record with it.
//!
//! ## A receipt's time is its block's time
//!
//! A `receive` row this shell writes comes from `token_trust`'s feed, whose
//! every time is a block's own (its invariant ⑨), so the row is stored marked
//! `timeVerified: true`. A row stored before the mark existed may hold the
//! clock of the day it was scanned (three receipts of 2026-09-29 stood under
//! "Today" on 2026-10-10); the core finds those by the missing mark, asks for
//! the block's time ([`receive_time`]) and has the row rewritten
//! ([`write_receive_time`]). Which rows, how many a round and when to ask
//! again are the core's; this file reads, and writes what it is told.
//!
//! ## `day_start_ms` is the shell's, and it is not a formatting detail
//!
//! The core's words: "LOCAL-midnight epoch ms for `timestamp` — computed by the
//! shell, which owns the device timezone." `vela-core` deliberately ships **no
//! timezone database**, so the day boundary is the one thing it cannot derive.
//!
//! Getting it wrong is visible rather than subtle: a feed grouped by UTC day
//! files a 20:00 transaction in Tokyo under *tomorrow*, and one at 19:00 in New
//! York under *today* when it should be yesterday — wrong for part of every day
//! for everybody who is not in Greenwich.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui::App;
use serde_json::{Value, json};

use vela_core::app::activity_feed::{
    ActivityFeed, Event, FeedOperation, FeedShellResult, FeedTxKind, FeedTxRecord, FeedTxStatus,
};
use vela_core::app::token_trust::TrustSimJudgment;

use crate::executor::{identity, storage};
use crate::resident::{self, Answer, Machine};
use crate::session;

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";

/// One stored row → one feed record.
///
/// Coercion, never policy: a row that will not parse is skipped rather than
/// failing the load, exactly as the contacts and network ledgers do. A feed that
/// refuses to render because one legacy row is odd is worse than a feed missing
/// that row.
pub(crate) fn to_record(row: &Value) -> Option<FeedTxRecord> {
    let text = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let optional = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    };

    let id = row.get("id").and_then(Value::as_str)?.to_owned();
    // Stored in SECONDS, and the core says so. Multiplying in the wrong place
    // puts every record in 1970.
    let timestamp = row.get("timestamp").and_then(Value::as_f64).unwrap_or(0.0);

    Some(FeedTxRecord {
        id,
        user_op_hash: text("userOpHash"),
        tx_hash: text("txHash"),
        from: text("from"),
        to: text("to"),
        to_name: optional("toName"),
        value: text("value"),
        symbol: text("symbol"),
        decimals: row
            .get("decimals")
            .and_then(Value::as_u64)
            .and_then(|d| u32::try_from(d).ok())
            .unwrap_or(18),
        logo_urls: row.get("logoUrls").and_then(Value::as_array).map(|urls| {
            urls.iter()
                .filter_map(|url| url.as_str().map(str::to_owned))
                .collect()
        }),
        chain_id: row
            .get("chainId")
            .and_then(Value::as_u64)
            .and_then(|id| u32::try_from(id).ok())
            .unwrap_or(0),
        timestamp,
        day_start_ms: crate::executor::day_start_ms(timestamp * 1000.0),
        status: match row.get("status").and_then(Value::as_str) {
            Some("pending") => FeedTxStatus::Pending,
            Some("failed") => FeedTxStatus::Failed,
            _ => FeedTxStatus::Confirmed,
        },
        // Absent means a record older than the field, and the core reads that
        // as `send` — its own `t.type ?? 'send'`. `None` is the honest report;
        // substituting `Send` here would hide a legacy row from the core's own
        // rule about legacy rows.
        kind: row
            .get("type")
            .and_then(Value::as_str)
            .and_then(|kind| match kind {
                "send" => Some(FeedTxKind::Send),
                "receive" => Some(FeedTxKind::Receive),
                "dapp_tx" => Some(FeedTxKind::DappTx),
                "sign_message" => Some(FeedTxKind::SignMessage),
                "sign_typed_data" => Some(FeedTxKind::SignTypedData),
                "connect" => Some(FeedTxKind::Connect),
                _ => None,
            }),
        usd: optional("usd"),
        // A dApp interaction's origin and recorded intent — a transaction's
        // or, since spec 093, a signature's — which the core turns into the
        // row's title (083 H2). Both are what `persist_record` in the sign
        // executor writes, in the bytes every client writes them.
        //
        // The site is read from `dappUrl`, the origin the request came from;
        // `dappOrigin` is a dApp's self-declared name on the clients that
        // pass one (083 H2 review). A row this shell wrote before `dappUrl`
        // existed has only `dappOrigin`, and on this shell that IS the origin:
        // the desktop has never handed the core a dApp identity
        // (`RequestArrived { dapp: None }`), so the core stored the origin
        // there.
        dapp_url: optional("dappUrl").or_else(|| optional("dappOrigin")),
        intent: optional("intent"),
        // What the sheet's own simulation showed when the person approved
        // (083 F1) — the row's figure. Written by the sign executor from the
        // approve alone; a row without it draws as it always has.
        balance_changes: row.get("assetChanges").and_then(stored_changes),
        calldata: calldata_of(row),
        // What the call does, for a dApp's record (spec 082 RJ16, G52): the
        // first call's `data` from the request as it was stored. Who the
        // row names — a token transfer's recipient or the contract — is the
        // core's reading of it.
        call_data: first_call_data(row),
        // What the request was (spec 093): the core's own summary, stored
        // verbatim at approve time and handed back untouched. One that will
        // not read as the core's shape is no summary — the core then reads
        // the row by its kind, as it reads a record from before 093.
        summary: row
            .get("dappSummary")
            .and_then(|summary| serde_json::from_value(summary.clone()).ok()),
        // How its operation ended (spec 097), as the tracker's patch wrote
        // it: handed back untouched; one that will not read is none.
        settlement: row
            .get("settlement")
            .and_then(|settlement| serde_json::from_value(settlement.clone()).ok()),
        // PR 3: the stored `timeVerified` mark — this row's time is its
        // block's own. Absent stays absent: that is what marks a row written
        // before the mark existed, and the core repairs those.
        time_verified: row.get("timeVerified").and_then(Value::as_bool),
    })
}

/// The first call's `data` in a stored dApp request (`signedRequest`, the
/// params array as `sign_request::persist_record` kept it): the call of
/// `eth_sendTransaction`, or the first of a `wallet_sendCalls` batch. `None`
/// for any other row.
fn first_call_data(row: &Value) -> Option<String> {
    if row.get("type").and_then(Value::as_str) != Some("dapp_tx") {
        return None;
    }
    let request = row.get("signedRequest").and_then(Value::as_str)?;
    let Ok(params) = serde_json::from_str::<Value>(request) else {
        return clipped_call_data(request);
    };
    let first = params.get(0)?;
    let call = first
        .get("calls")
        .and_then(|calls| calls.get(0))
        .unwrap_or(first);
    call.get("data").and_then(Value::as_str).map(str::to_owned)
}

/// A request over the 8 KB the store keeps is clipped and no longer parses
/// (`requestTruncated`): its first `data` is read off the text — the hex that
/// survived the clip. A prefix is enough for the core to tell a contract call
/// from a token transfer, whose whole calldata is 68 bytes.
fn clipped_call_data(request: &str) -> Option<String> {
    let after = &request[request.find("\"data\"")? + "\"data\"".len()..];
    let value = after
        .trim_start()
        .strip_prefix(':')?
        .trim_start()
        .strip_prefix('"')?;
    let hex: String = value
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect();
    (hex.len() > 2 && hex.starts_with("0x")).then_some(hex)
}

/// The balance changes a row kept (083 F1), from its `assetChanges` — the
/// web's `StoredAssetSim` shape, which the sign executor writes too. Each
/// line goes back to the judgment it was drawn from: the native coin; a token
/// with its symbol and decimals, in the set the wallet trusts only when the
/// line says `trusted: true` (the web writes no such word, so its lines are
/// drawn but never the row's figure); and anything marked unverified or
/// missing either — the safe reading — an unverified token, which carries no
/// figure. A line of any other kind is dropped.
fn stored_changes(stored: &Value) -> Option<Vec<TrustSimJudgment>> {
    let lines = stored.get("changes")?.as_array()?;
    Some(
        lines
            .iter()
            .filter_map(|line| {
                let delta = line.get("delta").and_then(Value::as_str)?.to_owned();
                match line.get("kind").and_then(Value::as_str)? {
                    "native" => Some(TrustSimJudgment::Native { delta }),
                    "erc20" => {
                        let token = line.get("token").and_then(Value::as_str).map(str::to_owned);
                        let symbol = line
                            .get("symbol")
                            .and_then(Value::as_str)
                            .filter(|symbol| !symbol.trim().is_empty());
                        let decimals = line
                            .get("decimals")
                            .and_then(Value::as_u64)
                            .and_then(|decimals| u32::try_from(decimals).ok());
                        let unverified = line.get("unverified").and_then(Value::as_bool);
                        let trusted = line.get("trusted").and_then(Value::as_bool) == Some(true);
                        Some(match (token, symbol, decimals, unverified) {
                            (Some(token), Some(symbol), Some(decimals), None | Some(false)) => {
                                TrustSimJudgment::Erc20Trusted {
                                    token,
                                    delta,
                                    symbol: symbol.to_owned(),
                                    decimals,
                                    in_trusted_set: trusted,
                                }
                            }
                            (token, ..) => TrustSimJudgment::Erc20Unverified { token, delta },
                        })
                    }
                    _ => None,
                }
            })
            .collect(),
    )
}

/// Whether a dApp's transaction carried calldata (083 F3): calldata makes
/// the row's `to` the contract it called, not a recipient.
///
/// The row's own `calldata`, when it has one — the sign executor decides it
/// from the whole final request, by the reading the submit path uses. Only a
/// row older than that field is read off the request stored beside it
/// (`signedRequest`: this shell keeps the params array as text, the web
/// `{ method, params }`), and then by what each METHOD submits: a single
/// transaction its own `data`, a batch its `calls` — never a `calls` a page
/// wrote beside a single transaction, which nothing sends (083 F3 review).
/// This shell's text names no method; its batch rows are the ones stored
/// with no `to`, which is all a batch writes there. `None` when there is no
/// request to read — none stored, clipped (`requestTruncated`), not JSON —
/// and for every other kind of row.
fn calldata_of(row: &Value) -> Option<bool> {
    if row.get("type").and_then(Value::as_str) != Some("dapp_tx") {
        return None;
    }
    if let Some(recorded) = row.get("calldata").and_then(Value::as_bool) {
        return Some(recorded);
    }
    if row.get("requestTruncated").and_then(Value::as_bool) == Some(true) {
        return None;
    }
    let (params, batch) = match row.get("signedRequest")? {
        Value::String(text) => (
            serde_json::from_str::<Value>(text).ok()?,
            row.get("to")
                .and_then(Value::as_str)
                .is_none_or(|to| to.trim().is_empty()),
        ),
        Value::Object(request) => (
            request.get("params")?.clone(),
            request.get("method").and_then(Value::as_str) == Some("wallet_sendCalls"),
        ),
        _ => return None,
    };
    let first = params.get(0).filter(|first| first.is_object())?;
    let carries = |call: &Value| {
        call.get("data")
            .and_then(Value::as_str)
            .is_some_and(crate::executor::sign_request::carries_calldata)
    };
    if batch {
        // A batch's calls are what it submits.
        Some(first.get("calls")?.as_array()?.iter().any(carries))
    } else {
        Some(carries(first))
    }
}

/// Discover incoming transfers and persist the ones `token_trust` admitted.
///
/// Answers the count of GENUINELY NEW records, which is what the core turns
/// into a celebration. Re-persisting a receipt the store already has must count
/// as zero, or every poll would congratulate somebody on the same payment: the
/// scan window overlaps between polls on purpose, and de-duping is the shell's
/// half of that bargain.
///
/// Any failure answers 0 — the TS `catch { return 0 }`. A scan that could not
/// run found nothing, which is a true statement, and it is not a reason to
/// show an error over a feed that is otherwise correct.
fn sync_received(address: &str) -> u32 {
    // One scan at a time. Ticks are 10 s apart and a twelve-chain discovery on
    // a bad network can outlive that; two of them overlapping would read the
    // same store, both find the same receipt missing, and both write it —
    // which is two rows and two celebrations for one payment. A scan that
    // could not run found nothing, which is the answer this function already
    // gives for every other way it can fail to run.
    if SCANNING.swap(true, Ordering::SeqCst) {
        return 0;
    }
    let added = scan(address);
    SCANNING.store(false, Ordering::SeqCst);
    added
}

static SCANNING: AtomicBool = AtomicBool::new(false);

fn scan(address: &str) -> u32 {
    persist_incoming(address, &crate::executor::token_trust::poll(address))
}

/// Store the transfers the scan admitted that the store does not hold yet,
/// and answer how many that was.
fn persist_incoming(
    address: &str,
    incoming: &[vela_core::app::token_trust::TrustIncomingView],
) -> u32 {
    if incoming.is_empty() {
        return 0;
    }

    // One read-modify-write under the store's lock (spec 082 RJ1 review):
    // this runs on a worker while a dApp's record, the Send and the tracker
    // write the same list, and a stale copy written back loses their rows.
    let mut added = 0u32;
    let written = storage::update_list(TX_KEY, |rows| {
        let mut known: std::collections::BTreeSet<String> = rows
            .iter()
            .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
            .collect();
        for transfer in incoming {
            if known.contains(&transfer.id) {
                continue;
            }
            // A non-native token whose metadata would not resolve is SKIPPED,
            // not stored at a guessed scale. The web says why: an 18-decimal
            // fallback on a 6-decimal token stores a misleading "+0 tokens",
            // and the transfer stays in the scan window to be retried once
            // metadata resolves. Genuine spam with no readable symbol never
            // reaches the feed.
            let (Some(symbol), Some(decimals)) = (
                transfer
                    .symbol
                    .clone()
                    .or_else(|| transfer.is_native.then(|| native_symbol(transfer.chain_id))),
                transfer.decimals.or(transfer.is_native.then_some(18)),
            ) else {
                continue;
            };
            let Some(row) = incoming_row(transfer, address, &symbol, decimals) else {
                continue;
            };
            known.insert(transfer.id.clone());
            rows.push(row);
            added += 1;
        }
        added > 0
    });
    if written.is_err() {
        // The store refused. Reporting new records that are not on disk would
        // celebrate a payment the next launch has never heard of.
        return 0;
    }
    added
}

/// One admitted transfer as a stored row, in the bytes every client reads.
fn incoming_row(
    transfer: &vela_core::app::token_trust::TrustIncomingView,
    address: &str,
    symbol: &str,
    decimals: u32,
) -> Option<Value> {
    // The core hands over the RAW on-chain amount; the store holds the human
    // one, exactly as `BalanceToken.balance` does and for the same reason —
    // `tx_usd_value` parses it as a decimal.
    let value = crate::executor::abi::format_raw_balance(&transfer.value, decimals);
    Some(json!({
        "id": transfer.id,
        "txHash": transfer.tx_hash,
        "userOpHash": "",
        "from": transfer.from,
        "to": address,
        "value": value,
        "symbol": symbol,
        "decimals": decimals,
        "chainId": transfer.chain_id,
        "timestamp": transfer.timestamp_sec,
        // PR 3: the feed's time is the transfer's own block's, by
        // construction (`token_trust` invariant ⑨ — a transfer whose block
        // could not be read never reaches the feed). Marked, so the core's
        // repair of older rows leaves this one alone.
        "timeVerified": true,
        "status": "confirmed",
        "type": "receive",
        // No `usd`. The ingest valuation needs a live price this path does not
        // hold, and the core RE-DERIVES the value on read (`tx_usd_value`),
        // including the ≈$1 stablecoin fallback. Writing "$0.00" would store a
        // claim; writing nothing lets the rule that owns it decide.
    }))
}

/// The time of the block that holds transaction `tx_hash` on `chain_id`, in
/// Unix seconds (`FeedOperation::ReadReceiveTime`, PR 3): the transaction's
/// receipt names its block, and that block's header carries the time.
///
/// `None` whenever either read gave no usable answer — no receipt (a node
/// that has pruned it, a hash it never saw), an RPC error, a block that did
/// not come back, a `timestamp` that is no number. Never a clock's time and
/// never a guess: the record then keeps the time it has, and when to ask
/// again is the core's. One attempt, no retry here.
///
/// `rpc` is the routed call — in the app,
/// [`crate::executor::token_trust::rpc`], the pool the scan itself reads
/// through.
fn receive_time(
    chain_id: u32,
    tx_hash: &str,
    rpc: impl Fn(u32, &str, Value) -> Option<Value>,
) -> Option<f64> {
    let receipt = rpc(chain_id, "eth_getTransactionReceipt", json!([tx_hash]))?;
    let block = receipt
        .get("blockNumber")
        .and_then(Value::as_str)?
        .to_owned();
    let header = rpc(chain_id, "eth_getBlockByNumber", json!([block, false]))?;
    crate::executor::token_trust::header_timestamp_sec(&header)
}

/// Rewrite ONE stored record's time (`FeedOperation::WriteReceiveTime`,
/// PR 3): its `timestamp` becomes `timestamp_sec` — a block's own time — and
/// it is marked `timeVerified: true`. Nothing else of it changes, and nothing
/// of any other record. Answers whether it was written: `false` when no
/// record has that id, or the store refused.
///
/// Under the store's lock, like the scan's merge, the delete, the tracker's
/// patches and a dApp's record: one read-modify-write, so none of them writes
/// a stale copy back over this.
fn write_receive_time(id: &str, timestamp_sec: f64) -> bool {
    storage::update_list(TX_KEY, |rows| {
        let Some(row) = rows
            .iter_mut()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(id))
            .and_then(Value::as_object_mut)
        else {
            return false;
        };
        row.insert("timestamp".to_owned(), json!(timestamp_sec));
        row.insert("timeVerified".to_owned(), json!(true));
        true
    })
    .unwrap_or(false)
}

/// A chain's own coin, for a native transfer the core did not name.
fn native_symbol(chain_id: u32) -> String {
    vela_core::app::network_admin::BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(
            || "tokens".to_owned(),
            |chain| chain.native_symbol.to_owned(),
        )
}

/// One record's stored request (spec 093), for its detail's "Technical
/// details" — read from the store by record id, and only when that section is
/// opened. This shell keeps the params array as text (`signedRequest`, as the
/// core cut it); a row the web wrote keeps `{ method, params }`, whose
/// `params` is the same thing. `None` when the record kept none.
pub(crate) fn stored_request(id: &str) -> Option<String> {
    let Ok(Some(Value::Array(rows))) = storage::read_value(TX_KEY) else {
        return None;
    };
    let row = rows
        .iter()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(id))?;
    let text = match row.get("signedRequest")? {
        Value::String(text) => text.clone(),
        Value::Null => return None,
        Value::Object(request) => request.get("params")?.to_string(),
        other => other.to_string(),
    };
    (!text.trim().is_empty()).then_some(text)
}

/// Every stored row, as the core reads it. Crate-visible so the sign
/// executor's test can prove the row it writes is the one Activity draws
/// (083 H2).
pub(crate) fn read_records() -> Vec<FeedTxRecord> {
    let Ok(Some(Value::Array(rows))) = storage::read_value(TX_KEY) else {
        return Vec::new();
    };
    rows.iter().filter_map(to_record).collect()
}

impl Machine for ActivityFeed {
    const LABEL: &'static str = "activity_feed";

    fn boot_event(cx: &App) -> Event {
        Event::AccountSwitched {
            address: session::view(cx).address,
        }
    }

    fn perform(operation: &FeedOperation) -> Answer<FeedShellResult, Self::Event> {
        match operation {
            FeedOperation::ReadTxStore { read_id, .. } => {
                Answer::Now(FeedShellResult::StoreLoaded {
                    records: read_records(),
                    now_ms: crate::executor::now_ms(),
                    // Echoed, and the core explains why at length: it is what
                    // binds a celebration to the read that earned it.
                    read_id: *read_id,
                })
            }

            // Live since 031: the whole `syncReceivedTransfers` pipeline —
            // `token_trust` runs the discovery and rules on admission, and this
            // persists what it admitted into the same store the send path will
            // write to.
            FeedOperation::ScanIncomingTransfers { address, .. } => {
                let address = address.clone();
                Answer::Blocking(Box::new(move || FeedShellResult::SyncCompleted {
                    new_count: sync_received(&address),
                }))
            }

            FeedOperation::DeleteTxRecord { id } => {
                let id = id.clone();
                let removed = storage::update_list(TX_KEY, |rows| {
                    let before = rows.len();
                    rows.retain(|row| row.get("id").and_then(Value::as_str) != Some(&id));
                    rows.len() != before
                })
                .unwrap_or(false);
                Answer::Now(if removed {
                    FeedShellResult::DeleteCommitted { id }
                } else {
                    // A delete that removed nothing is a FAILED delete, not a
                    // quiet success: the row is still on the screen, and the
                    // core needs to know it is still there.
                    FeedShellResult::DeleteFailed { id }
                })
            }

            FeedOperation::ResolveRecipientIdentity { addr } => {
                let addr = addr.clone();
                // The whole waterfall, in `executor::identity`: own accounts
                // first (local, no network), then the cache, the passkey index
                // and the name services. It is the SAME function
                // `contacts::resolve_identity` calls — the core describes one
                // lookup and two machines ask for it, so there is one.
                Answer::Blocking(Box::new(move || FeedShellResult::AliasResolved {
                    name: identity::resolve(&addr).map(|identity| identity.name),
                    addr,
                }))
            }

            FeedOperation::Timer { ms, generation } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                FeedShellResult::ToastExpired {
                    generation: *generation,
                },
            ),

            // A desktop has no haptics. Answered rather than skipped — the
            // celebration still runs, it just has one fewer sense.
            FeedOperation::Haptic => Answer::Now(FeedShellResult::HapticPlayed),

            // PR 3: the time of the block that holds a receipt's transaction
            // — two reads through the pool the scan uses, off the window's
            // thread. `None` is an honest "not read"; the core asks again
            // when it chooses to.
            FeedOperation::ReadReceiveTime {
                id,
                chain_id,
                tx_hash,
            } => {
                let (id, chain_id, tx_hash) = (id.clone(), *chain_id, tx_hash.clone());
                Answer::Blocking(Box::new(move || FeedShellResult::ReceiveTimeRead {
                    timestamp_sec: receive_time(chain_id, &tx_hash, |chain, method, params| {
                        crate::executor::token_trust::rpc(chain, method, params).ok()
                    }),
                    id,
                }))
            }

            // PR 3: that one record, rewritten with its block's time. On this
            // thread like the delete: a local write under the store's lock.
            FeedOperation::WriteReceiveTime { id, timestamp_sec } => {
                Answer::Now(FeedShellResult::ReceiveTimeWritten {
                    ok: write_receive_time(id, *timestamp_sec),
                    id: id.clone(),
                })
            }
        }
    }
}

static TICKING: AtomicBool = AtomicBool::new(false);

/// The cadence the core does NOT own.
///
/// Its words: "`FocusTick`/`LiveTick` cadence (focus + 30s auto-refresh, 10s
/// while the Activity tab is visible) stays in the shell: which tab is visible
/// is render-domain state the core never sees." The web's home runs the 10 s
/// `LiveTick` while its tab is visible (078 W-09); here that is the wallet
/// section showing ([`set_visible`]) in a window that is on screen — shown
/// and not minimized, asked of the system at each beat, since a minimized
/// window is not re-rendered and could not say so itself. Otherwise every
/// third beat — the 30 s auto-refresh. A scan that outlives a beat is not
/// doubled: `sync_received` runs one at a time.
const TICK: Duration = Duration::from_secs(10);
/// Beats between two auto-refreshes while the Activity is out of sight (30 s).
const HIDDEN_EVERY: u32 = 3;

/// The wallet section — the Activity — is what the window is showing.
static VISIBLE: AtomicBool = AtomicBool::new(false);
/// The wallet window's native handle, for asking whether it is on screen.
static WINDOW: std::sync::atomic::AtomicIsize = std::sync::atomic::AtomicIsize::new(0);

/// What the page says, each frame: whether the Activity is the section
/// showing, and which native window it is in (0 where there is none to ask).
pub fn set_visible(visible: bool, window: isize) {
    VISIBLE.store(visible, Ordering::Relaxed);
    WINDOW.store(window, Ordering::Relaxed);
}

/// The Activity can be seen right now — the web's
/// `document.visibilityState === 'visible'`.
fn activity_on_screen() -> bool {
    VISIBLE.load(Ordering::Relaxed) && window_on_screen(WINDOW.load(Ordering::Relaxed))
}

/// Shown and not minimized. Where the system cannot be asked, shown.
fn window_on_screen(window: isize) -> bool {
    #[cfg(windows)]
    if window != 0 {
        use windows_sys::Win32::UI::WindowsAndMessaging::{IsIconic, IsWindowVisible};
        let hwnd = window as windows_sys::Win32::Foundation::HWND;
        // SAFETY: two read-only queries on a handle; a stale one answers 0.
        return unsafe { IsWindowVisible(hwnd) != 0 && IsIconic(hwnd) == 0 };
    }
    let _ = window;
    true
}

/// Boot the feed and keep it re-reading for the life of the process.
///
/// **This is the celebration's only way in.** Without it the machine performs
/// its boot pipeline once and never again, and the one path that arms a toast
/// — a sync that persisted a genuinely new receipt, AFTER the first pass —
/// cannot be reached even in principle: the first pass is spent at boot, and no
/// second pass ever happens. A pending send would also stay pending on screen
/// until the next launch, because the re-read that notices the tracker's patch
/// is the same re-read.
///
/// Called by the wallet page on sign-in, beside the tracker's own; a second
/// call is a no-op.
pub fn start_ticks(cx: &mut App) {
    let _ = resident::resident::<ActivityFeed>(cx);
    if TICKING.swap(true, Ordering::SeqCst) {
        return;
    }
    cx.spawn(async move |cx| {
        let mut beat: u32 = 0;
        loop {
            cx.background_executor().timer(TICK).await;
            beat = beat.wrapping_add(1);
            let event = if activity_on_screen() {
                Event::LiveTick
            } else if beat.is_multiple_of(HIDDEN_EVERY) {
                Event::FocusTick
            } else {
                continue;
            };
            // Re-fetched each tick, for the reason the tracker's loop states:
            // a sign-out drops every resident, and the next sign-in's feed is
            // the one that must get the ticks.
            let feed = cx.update(|cx| resident::resident::<ActivityFeed>(cx));
            feed.update(cx, |resident, cx| resident.dispatch(event, cx));
        }
    })
    .detach();
}

/// The window came back, or something changed the store under it.
///
/// `FocusTick` is the core's own "look again now" — the same event the 30 s
/// loop sends, which is why there is one function and not two policies.
pub fn focus_tick(cx: &mut App) {
    resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
        resident.dispatch(Event::FocusTick, cx);
    });
}

/// The tracker converged `resolved_count` pending submissions.
///
/// The core re-reads the store and — pointedly — **never celebrates** for this
/// one: a send of your own that finally confirmed is not money arriving. Wiring
/// it is what takes a just-confirmed transfer off "pending" the moment it
/// lands, rather than at the next 30 s tick.
pub fn reconciled(resolved_count: u32, cx: &mut App) {
    if resolved_count == 0 {
        return;
    }
    resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
        resident.dispatch(Event::ReconcileCompleted { resolved_count }, cx);
    });
}

/// Tell the feed whose contact page is open (spec 093) — `None` once it
/// closes. The core then lists what passed between the account and that
/// address (`FeedView::contact_rows`); the page draws those rows and filters
/// nothing itself.
pub fn contact_changed(address: Option<String>, cx: &mut App) {
    resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
        resident.dispatch(Event::ContactFilterChanged { address }, cx);
    });
}

/// Tell the feed what the balance hero is doing about privacy.
///
/// The core suppresses the toast while balances are hidden (invariant ④), and
/// it can only do that if somebody tells it — the flag lives in
/// `balance_dashboard` and the feed has no way to read another machine. Nobody
/// told it until this cut, which means the celebration would have printed the
/// number the hero beside it was masking.
pub fn privacy_changed(hidden: bool, cx: &mut App) {
    resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
        resident.dispatch(Event::PrivacyChanged { hidden }, cx);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn seed(rows: Value) {
        if storage::write_value(TX_KEY, rows).is_err() {
            unreachable!("could not seed the tx store");
        }
    }

    /// Spec 082 RJ16 (G52): a request over the 8 KB the store keeps is
    /// clipped (`requestTruncated`) and no longer parses — a big multicall,
    /// a marketplace order. Its first call's `data` is still read off the
    /// text, so the contract is named as the contract, not as 接收方 (who got
    /// the money); a clip with no `data` in it says nothing.
    #[test]
    fn a_clipped_request_still_names_its_contract() {
        let call = format!("0x5ae401dc{}", "ab".repeat(6_000));
        let whole = json!([{ "to": "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83", "data": call }])
            .to_string();
        let clipped = &whole[..8 * 1024];
        assert!(serde_json::from_str::<Value>(clipped).is_err());
        let row = |request: &str, truncated: bool| {
            json!({
                "id": "dapp-1-tx", "userOpHash": "0xop", "txHash": "", "from": "0xme",
                "to": "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83", "value": "0x0",
                "symbol": "xDAI", "decimals": 18, "chainId": 100, "timestamp": 1,
                "status": "pending", "type": "dapp_tx", "signedRequest": request,
                "requestTruncated": truncated,
            })
        };
        let data = to_record(&row(clipped, true))
            .and_then(|record| record.call_data)
            .unwrap_or_default();
        assert!(
            data.starts_with("0x5ae401dc"),
            "{}",
            &data[..data.len().min(20)]
        );
        assert!(call.starts_with(&data));
        let spaced = r#"[{"to":"0xDD", "data" :  "0xa9059cbb00"#;
        assert_eq!(
            to_record(&row(spaced, true)).and_then(|record| record.call_data),
            Some("0xa9059cbb00".to_owned())
        );
        assert_eq!(
            to_record(&row(r#"[{"to":"0xDD","value":"0x"#, true))
                .and_then(|record| record.call_data),
            None
        );
    }

    /// A discovered receipt becomes a stored row, and the same one twice does
    /// not become two.
    #[test]
    fn an_admitted_transfer_persists_once_and_is_not_celebrated_again() {
        use vela_core::app::token_trust::TrustIncomingView;

        storage::tests::with_temp_state("feed-ingest", || {
            const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let transfer = TrustIncomingView {
                id: "100-0xdead-0".to_owned(),
                chain_id: 100,
                token: Some("0xDDAf".to_owned()),
                is_native: false,
                from: "0xAbCd".to_owned(),
                // Raw base units — six decimals, so 1.5 USDC.
                value: "1500000".to_owned(),
                tx_hash: "0xdead".to_owned(),
                block_number: 1.0,
                log_index: 0,
                timestamp_sec: 1_788_500_000.0,
                symbol: Some("USDC".to_owned()),
                decimals: Some(6),
            };
            let row = incoming_row(&transfer, ME, "USDC", 6)
                .unwrap_or_else(|| unreachable!("a complete transfer"));

            // The stored amount is HUMAN, not base units: `tx_usd_value`
            // parses it as a decimal, so 1500000 here would be a $1.5M receipt.
            assert_eq!(row.get("value").and_then(Value::as_str), Some("1.5"));
            assert_eq!(row.get("type").and_then(Value::as_str), Some("receive"));
            assert_eq!(row.get("to").and_then(Value::as_str), Some(ME));
            assert!(row.get("usd").is_none(), "the core re-derives the value");

            // And the record round-trips through the store reader into the
            // shape the core reads.
            if storage::write_value(TX_KEY, Value::Array(vec![row])).is_err() {
                unreachable!("could not seed");
            }
            let records = read_records();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].kind, Some(FeedTxKind::Receive));
            assert_eq!(records[0].symbol, "USDC");
            assert_eq!(records[0].decimals, 6);
            // The core's own valuation of what we wrote: a stablecoin with no
            // stored price is worth its amount, not zero.
            let usd = vela_core::app::activity_feed::tx_usd_value(&records[0]);
            assert!((usd - 1.5).abs() < 1e-9, "{usd}");
        });
    }

    const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    /// 2026-10-10T08:00:00Z: the scan's clock, stored as the receipts' time.
    const SCANNED_AT: i64 = 1_791_619_200;
    /// 2026-09-29T09:41:35Z: the first receipt's own block.
    const BLOCK_TIME: f64 = 1_790_674_895.0;

    /// One of the three receipts of the evidence: 0.001 xDAI, really
    /// received on 2026-09-29 (Gnosis), stored with the clock of the day the
    /// scan found it — 2026-10-10 — and no mark.
    fn stale_receipt(n: u32) -> Value {
        json!({
            "id": format!("100-0xfeed{n}-{n}"),
            "txHash": format!("0xfeed{n}"),
            "userOpHash": "",
            "from": "0xAbCd000000000000000000000000000000000001",
            "to": ME,
            "value": "0.001",
            "symbol": "xDAI",
            "decimals": 18,
            "chainId": 100,
            "timestamp": SCANNED_AT + i64::from(n),
            "status": "confirmed",
            "type": "receive",
            // Fields this build writes nowhere: kept as they are.
            "logoUrls": ["https://example.org/xdai.png"],
            "toName": "",
            "note": { "kept": [1, 2.5, "three", null] },
        })
    }

    fn stored_rows() -> Vec<Value> {
        match storage::read_value(TX_KEY) {
            Ok(Some(Value::Array(rows))) => rows,
            other => unreachable!("no tx store: {other:?}"),
        }
    }

    fn store_file() -> (String, Option<u64>) {
        let path = storage::path().unwrap_or_else(|e| unreachable!("{e:?}"));
        let bytes = std::fs::read_to_string(&path).unwrap_or_else(|e| unreachable!("{e}"));
        // The store writes beside the file and renames over it, so a write
        // of any kind is a new file — which bytes alone would not show.
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::MetadataExt as _;
            std::fs::metadata(&path).ok().map(|meta| meta.ino())
        };
        #[cfg(not(unix))]
        let file = None;
        (bytes, file)
    }

    fn written(id: &str, timestamp_sec: f64) -> bool {
        match ActivityFeed::perform(&FeedOperation::WriteReceiveTime {
            id: id.to_owned(),
            timestamp_sec,
        }) {
            Answer::Now(FeedShellResult::ReceiveTimeWritten { id: answered, ok }) => {
                assert_eq!(
                    answered, id,
                    "the answer names the record it was asked about"
                );
                ok
            }
            _ => unreachable!("the rewrite is a local write, answered at once"),
        }
    }

    fn loaded(read_id: u32) -> Vec<FeedTxRecord> {
        match ActivityFeed::perform(&FeedOperation::ReadTxStore {
            address: ME.to_owned(),
            read_id,
        }) {
            Answer::Now(FeedShellResult::StoreLoaded {
                records,
                read_id: echoed,
                ..
            }) => {
                assert_eq!(echoed, read_id);
                records
            }
            _ => unreachable!("the store read is answered at once"),
        }
    }

    /// PR 3 fix A, the store's half (`write_receive_time`). A stored receipt
    /// with the scan's clock for a time and no mark is rewritten with its
    /// block's time and `timeVerified: true` — that record, those two
    /// fields. Every other field of it is byte for byte what it was, the
    /// records beside it are untouched, and the next store read hands the
    /// core `time_verified: true` and the block's day. An id no record has
    /// answers `ok: false` and writes nothing at all.
    #[test]
    fn a_receipts_time_is_rewritten_in_place_and_nothing_else_is() {
        storage::tests::with_temp_state("feed-receive-time-write", || {
            let sent = json!({
                "id": "s1", "userOpHash": "0xop", "txHash": "0xsent", "from": ME,
                "to": "0xyou", "value": "5", "symbol": "xDAI", "decimals": 18,
                "chainId": 100, "timestamp": SCANNED_AT, "status": "confirmed",
                "type": "send", "usd": "$5.00"
            });
            seed(json!([sent, stale_receipt(1), stale_receipt(2)]));
            let before = stored_rows();
            let text = |value: &Value| value.to_string();

            // Before: no mark, so the core reads it as a record to repair.
            let unmarked = loaded(1);
            assert_eq!(unmarked[1].time_verified, None);
            assert_eq!(unmarked[1].kind, Some(FeedTxKind::Receive));
            #[allow(clippy::cast_precision_loss, reason = "seconds")]
            let scanned = (SCANNED_AT + 1) as f64;
            assert!((unmarked[1].timestamp - scanned).abs() < f64::EPSILON);

            // An id nobody has: `ok: false`, and the file is the same file.
            let untouched = store_file();
            assert!(!written("100-0xnobody-0", BLOCK_TIME));
            assert_eq!(store_file(), untouched, "nothing was written");

            // The record itself.
            assert!(written("100-0xfeed1-1", BLOCK_TIME));
            let after = stored_rows();
            assert_eq!(after.len(), 3);
            assert_eq!(text(&after[0]), text(&before[0]), "the send is untouched");
            assert_eq!(text(&after[2]), text(&before[2]), "the other receipt too");
            let (was, now) = (
                before[1].as_object().unwrap_or_else(|| unreachable!()),
                after[1].as_object().unwrap_or_else(|| unreachable!()),
            );
            assert_eq!(now.get("timestamp"), Some(&json!(BLOCK_TIME)));
            assert_eq!(now.get("timeVerified"), Some(&json!(true)));
            assert_eq!(now.len(), was.len() + 1, "one field added: the mark");
            for (field, value) in was {
                if field != "timestamp" {
                    assert_eq!(
                        now.get(field).map(text),
                        Some(text(value)),
                        "{field} is what it was"
                    );
                }
            }

            // …and the core's next read of it.
            let records = loaded(2);
            assert_eq!(records[1].id, "100-0xfeed1-1");
            assert_eq!(records[1].time_verified, Some(true));
            assert!((records[1].timestamp - BLOCK_TIME).abs() < f64::EPSILON);
            assert_eq!(
                records[1].day_start_ms,
                crate::executor::day_start_ms(BLOCK_TIME * 1000.0),
                "it stands under its block's day"
            );
            assert_ne!(records[1].day_start_ms, unmarked[1].day_start_ms);
            // Absent stays absent: the other receipt is still one to repair.
            assert_eq!(records[2].time_verified, None);
            assert_eq!(records[0].time_verified, None);

            // Written again with the same time: still that one record.
            assert!(written("100-0xfeed1-1", BLOCK_TIME));
            assert_eq!(text(&stored_rows()[1]), text(&after[1]));
        });
    }

    /// A stored mark that is not `true` is handed over as it is — the core
    /// reads `false` like no mark, and a junk value is no mark.
    #[test]
    fn the_stored_mark_is_mapped_and_never_invented() {
        let row = |mark: Value| {
            let mut row = stale_receipt(1);
            row["timeVerified"] = mark;
            to_record(&row).and_then(|record| record.time_verified)
        };
        assert_eq!(row(json!(true)), Some(true));
        assert_eq!(row(json!(false)), Some(false));
        assert_eq!(row(json!("true")), None);
        assert_eq!(row(Value::Null), None);
        assert_eq!(
            to_record(&stale_receipt(1)).and_then(|record| record.time_verified),
            None
        );
    }

    /// A node, scripted: what it answers each method with, and what it was
    /// asked.
    struct Node {
        receipt: Option<Value>,
        block: Option<Value>,
        asked: std::cell::RefCell<Vec<(u32, String, Value)>>,
    }

    impl Node {
        fn new(receipt: Option<Value>, block: Option<Value>) -> Self {
            Self {
                receipt,
                block,
                asked: std::cell::RefCell::default(),
            }
        }

        fn rpc(&self, chain_id: u32, method: &str, params: Value) -> Option<Value> {
            self.asked
                .borrow_mut()
                .push((chain_id, method.to_owned(), params));
            match method {
                "eth_getTransactionReceipt" => self.receipt.clone(),
                "eth_getBlockByNumber" => self.block.clone(),
                other => unreachable!("the repair asked for {other}"),
            }
        }

        fn time_of(&self, tx_hash: &str) -> Option<f64> {
            receive_time(100, tx_hash, |chain, method, params| {
                self.rpc(chain, method, params)
            })
        }
    }

    /// PR 3 fix A, the chain's half (`receive_time`), over a scripted node.
    /// The receipt names the block and the block's header carries the time:
    /// that, in seconds, is the answer. No receipt, a receipt with no block,
    /// a block that did not come back, a header with no readable time — each
    /// is `None`, asked once and not again, and never this machine's clock.
    #[test]
    fn a_receipts_time_is_read_from_its_block_or_not_at_all() {
        let receipt = || json!({ "transactionHash": "0xfeed1", "blockNumber": "0x2b1d3f7" });
        // 0x6ab8f1cf = 1_790_505_423.
        let block = || json!({ "number": "0x2b1d3f7", "timestamp": "0x6ab8f1cf" });

        let node = Node::new(Some(receipt()), Some(block()));
        assert_eq!(node.time_of("0xfeed1"), Some(1_790_505_423.0));
        assert_eq!(
            *node.asked.borrow(),
            vec![
                (
                    100,
                    "eth_getTransactionReceipt".to_owned(),
                    json!(["0xfeed1"])
                ),
                (
                    100,
                    "eth_getBlockByNumber".to_owned(),
                    json!(["0x2b1d3f7", false])
                ),
            ],
            "the receipt, then the block it names — headers only"
        );

        let now_s = crate::executor::now_ms() / 1000.0;
        let unread: [(&str, Option<Value>, Option<Value>, usize); 8] = [
            ("the receipt read failed", None, Some(block()), 1),
            ("no such receipt", Some(Value::Null), Some(block()), 1),
            (
                "a receipt with no block yet",
                Some(json!({ "transactionHash": "0xfeed1", "blockNumber": null })),
                Some(block()),
                1,
            ),
            (
                "a block number that is no string",
                Some(json!({ "blockNumber": 45_208_567 })),
                Some(block()),
                1,
            ),
            ("the block read failed", Some(receipt()), None, 2),
            ("no such block", Some(receipt()), Some(Value::Null), 2),
            (
                "a header with no time",
                Some(receipt()),
                Some(json!({ "number": "0x2b1d3f7" })),
                2,
            ),
            (
                "a time that is no number",
                Some(receipt()),
                Some(json!({ "timestamp": "yesterday" })),
                2,
            ),
        ];
        for (why, receipt, block, reads) in unread {
            let node = Node::new(receipt, block);
            let answer = node.time_of("0xfeed1");
            assert_eq!(answer, None, "{why}");
            assert_eq!(node.asked.borrow().len(), reads, "{why}: asked once each");
        }
        // The only number this ever answers is the header's own: with the
        // header's time a decade away from the clock, so is the answer.
        let old = Node::new(Some(receipt()), Some(json!({ "timestamp": "0x55d4a80" })));
        let answer = old.time_of("0xfeed1");
        assert_eq!(answer, Some(90_000_000.0));
        assert!(answer.is_some_and(|sec| (sec - now_s).abs() > 300_000_000.0));
    }

    /// The operation itself (`FeedOperation::ReadReceiveTime`): blocking
    /// work, through the pool the scan reads through — so with that chain's
    /// pool out of reach (faulted for this thread) the answer is `null` for
    /// the record asked about, at once, and never a time.
    #[test]
    fn the_receive_time_read_answers_null_when_the_pool_has_no_answer() {
        let operation = FeedOperation::ReadReceiveTime {
            id: "100-0xfeed1-1".to_owned(),
            chain_id: 100,
            tx_hash: "0xfeed1".to_owned(),
        };
        let Answer::Blocking(work) = ActivityFeed::perform(&operation) else {
            unreachable!("two network reads never run on the window's thread");
        };
        let answer = crate::executor::pool::with_fault(Some(&[100]), work);
        assert_eq!(
            answer,
            FeedShellResult::ReceiveTimeRead {
                id: "100-0xfeed1-1".to_owned(),
                timestamp_sec: None,
            }
        );
        assert_eq!(
            serde_json::to_value(&answer).ok(),
            Some(json!({
                "type": "receive_time_read",
                "id": "100-0xfeed1-1",
                "timestamp_sec": null,
            }))
        );
    }

    /// PR 3 fix A, the ingest's half: a receipt stored from `token_trust`'s
    /// feed is stored marked — its time is its block's by construction — and
    /// reads back to the core as `time_verified: true`, so the repair never
    /// asks about a row this build wrote. Stored once, counted once.
    #[test]
    fn a_receipt_from_the_trust_feed_is_stored_time_verified() {
        use vela_core::app::token_trust::TrustIncomingView;

        storage::tests::with_temp_state("feed-ingest-verified", || {
            let incoming = [
                TrustIncomingView {
                    id: "100-0xfeed1-1".to_owned(),
                    chain_id: 100,
                    token: None,
                    is_native: true,
                    from: "0xAbCd000000000000000000000000000000000001".to_owned(),
                    value: "1000000000000000".to_owned(),
                    tx_hash: "0xfeed1".to_owned(),
                    block_number: 45_208_567.0,
                    log_index: 1,
                    timestamp_sec: BLOCK_TIME,
                    symbol: None,
                    decimals: None,
                },
                TrustIncomingView {
                    id: "100-0xdead-0".to_owned(),
                    chain_id: 100,
                    token: Some("0xDDAf".to_owned()),
                    is_native: false,
                    from: "0xAbCd".to_owned(),
                    value: "1500000".to_owned(),
                    tx_hash: "0xdead".to_owned(),
                    block_number: 45_208_570.0,
                    log_index: 0,
                    timestamp_sec: BLOCK_TIME + 15.0,
                    symbol: Some("USDC".to_owned()),
                    decimals: Some(6),
                },
            ];
            // An older row beside them, with no mark: left as it is.
            seed(json!([stale_receipt(2)]));

            assert_eq!(persist_incoming(ME, &incoming), 2);
            let rows = stored_rows();
            assert_eq!(rows.len(), 3);
            assert_eq!(
                rows[0].get("timeVerified"),
                None,
                "an older row is not marked here"
            );
            for (row, transfer) in rows[1..].iter().zip(&incoming) {
                assert_eq!(row["id"], json!(transfer.id));
                assert_eq!(row["type"], json!("receive"));
                assert_eq!(row["timeVerified"], json!(true));
                assert_eq!(row["timestamp"], json!(transfer.timestamp_sec));
            }
            assert_eq!(rows[1]["value"], json!("0.001"));
            assert_eq!(rows[1]["symbol"], json!("xDAI"));

            let records = loaded(1);
            assert_eq!(records[0].time_verified, None);
            assert_eq!(records[1].time_verified, Some(true));
            assert_eq!(records[2].time_verified, Some(true));
            assert!((records[1].timestamp - BLOCK_TIME).abs() < f64::EPSILON);

            // The scan's windows overlap: the same two again add nothing.
            assert_eq!(persist_incoming(ME, &incoming), 0);
            assert_eq!(stored_rows().len(), 3);
        });
    }

    /// PR 3 fix A, the whole loop on this shell's wiring: the REAL feed core
    /// over the real store, with only the chain scripted. Three receipts
    /// stored under the day they were scanned: the core asks for each one's
    /// block time, this executor reads it and rewrites the record, the core
    /// reads the store once more — and every receipt stands under its own
    /// day, marked, with nothing else in the store changed. A second tick
    /// asks about none of them again.
    #[test]
    fn stale_receipts_are_put_under_their_own_day_by_the_cores_repair() {
        use crate::core_host::{CoreHost, Pending};

        /// Perform everything the core asks for until it asks no more —
        /// through this executor, but for the scan (a network poll: found
        /// nothing) and the chain (`node`). Returns the operations, in order.
        fn settle(
            host: &mut CoreHost<ActivityFeed>,
            mut pending: Vec<Pending<FeedOperation>>,
            node: &dyn Fn(u32, &str, Value) -> Option<Value>,
        ) -> Vec<FeedOperation> {
            let mut performed = Vec::new();
            while !pending.is_empty() {
                let next = pending.remove(0);
                performed.push(next.operation.clone());
                let result = match &next.operation {
                    FeedOperation::ScanIncomingTransfers { .. } => {
                        FeedShellResult::SyncCompleted { new_count: 0 }
                    }
                    FeedOperation::ResolveRecipientIdentity { addr } => {
                        FeedShellResult::AliasResolved {
                            addr: addr.clone(),
                            name: None,
                        }
                    }
                    FeedOperation::ReadReceiveTime {
                        id,
                        chain_id,
                        tx_hash,
                    } => FeedShellResult::ReceiveTimeRead {
                        id: id.clone(),
                        timestamp_sec: receive_time(*chain_id, tx_hash, node),
                    },
                    FeedOperation::Timer { .. } => continue,
                    operation => match ActivityFeed::perform(operation) {
                        Answer::Now(result) => result,
                        _ => unreachable!("{operation:?} is answered at once"),
                    },
                };
                pending.extend(host.resolve(next.id, result));
            }
            performed
        }

        storage::tests::with_temp_state("feed-receive-time-loop", || {
            let sent = json!({
                "id": "s1", "userOpHash": "0xop", "txHash": "0xsent", "from": ME,
                "to": "0xyou", "value": "5", "symbol": "xDAI", "decimals": 18,
                "chainId": 100, "timestamp": SCANNED_AT - 60, "status": "confirmed",
                "type": "send"
            });
            seed(json!([
                stale_receipt(1),
                sent,
                stale_receipt(2),
                stale_receipt(3)
            ]));
            let before = stored_rows();

            // Gnosis, 2026-09-29: block 0x2b1d3f0 + n holds receipt n, five
            // seconds a block.
            let node = |chain_id: u32, method: &str, params: Value| -> Option<Value> {
                assert_eq!(chain_id, 100);
                let first = params.get(0).and_then(Value::as_str).unwrap_or_default();
                match method {
                    "eth_getTransactionReceipt" => {
                        let n = u64::from_str_radix(first.trim_start_matches("0xfeed"), 16).ok()?;
                        Some(json!({ "blockNumber": format!("0x{:x}", 0x2b1_d3f0 + n) }))
                    }
                    "eth_getBlockByNumber" => {
                        let block = u64::from_str_radix(first.trim_start_matches("0x"), 16).ok()?;
                        #[allow(clippy::cast_possible_truncation, reason = "a block time")]
                        let time = BLOCK_TIME as u64 + (block - 0x2b1_d3f0) * 5;
                        Some(json!({ "timestamp": format!("0x{time:x}") }))
                    }
                    other => unreachable!("the repair asked for {other}"),
                }
            };

            let mut host = CoreHost::<ActivityFeed>::new();
            let pending = host.dispatch(Event::AccountSwitched {
                address: ME.to_owned(),
            });
            let performed = settle(&mut host, pending, &node);

            let reads: Vec<&str> = performed
                .iter()
                .filter_map(|operation| match operation {
                    FeedOperation::ReadReceiveTime { id, .. } => Some(id.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                reads,
                ["100-0xfeed3-3", "100-0xfeed2-2", "100-0xfeed1-1"],
                "each unmarked receipt, newest first; the send is not asked about"
            );
            let writes = performed
                .iter()
                .filter(|operation| matches!(operation, FeedOperation::WriteReceiveTime { .. }))
                .count();
            assert_eq!(writes, 3);
            let store_reads = performed
                .iter()
                .filter(|operation| matches!(operation, FeedOperation::ReadTxStore { .. }))
                .count();
            assert_eq!(
                store_reads, 2,
                "the first read, and one more after the round"
            );

            // The store: each receipt at its block's time and marked, every
            // other field as it was; the send untouched.
            let after = stored_rows();
            assert_eq!(after[1].to_string(), before[1].to_string());
            for (index, n) in [(0usize, 1.0f64), (2, 2.0), (3, 3.0)] {
                let mut expected = before[index].clone();
                expected["timestamp"] = json!(BLOCK_TIME + n * 5.0);
                expected["timeVerified"] = json!(true);
                assert_eq!(after[index], expected);
            }

            // What the core now holds is what Activity draws: every receipt
            // on 2026-09-29, none on the day it was scanned.
            let scanned_day = crate::executor::day_start_ms(
                1000.0 * {
                    #[allow(clippy::cast_precision_loss, reason = "seconds")]
                    {
                        SCANNED_AT as f64
                    }
                },
            );
            let own_day = crate::executor::day_start_ms(BLOCK_TIME * 1000.0);
            assert_ne!(scanned_day, own_day);
            let receipts: Vec<FeedTxRecord> = read_records()
                .into_iter()
                .filter(|record| record.kind == Some(FeedTxKind::Receive))
                .collect();
            assert_eq!(receipts.len(), 3);
            for receipt in &receipts {
                assert_eq!(receipt.time_verified, Some(true));
                assert_eq!(receipt.day_start_ms, own_day, "{}", receipt.id);
            }

            // The next tick: a store read and a scan, and no question about
            // a record already put right.
            let pending = host.dispatch(Event::FocusTick);
            let again = settle(&mut host, pending, &node);
            assert!(
                !again.iter().any(|operation| matches!(
                    operation,
                    FeedOperation::ReadReceiveTime { .. } | FeedOperation::WriteReceiveTime { .. }
                )),
                "{again:?}"
            );
            assert_eq!(stored_rows(), after);
        });
    }

    /// A scan that arrives while one is already running answers zero rather
    /// than joining in.
    ///
    /// Both would read the same store, both would find the same receipt
    /// missing from it, and both would write it back — two rows for one
    /// payment, and two celebrations. Nothing else in the pipeline would
    /// notice: the core counts what the shell reports, and the shell would be
    /// reporting the truth twice.
    #[test]
    fn a_second_scan_does_not_run_beside_the_first() {
        // Standing in for the in-flight one. Never touches the network: the
        // guard is the first thing `sync_received` does, before the poll.
        SCANNING.store(true, Ordering::SeqCst);
        let answered = sync_received("0x88cCA0EeDbF2C4426110bbFc998F048689266894");
        SCANNING.store(false, Ordering::SeqCst);
        assert_eq!(answered, 0, "a scan that could not run found nothing");
    }

    /// A record another client wrote reads back, camelCase and all.
    #[test]
    fn a_stored_row_maps_to_a_feed_record() {
        storage::tests::with_temp_state("feed-map", || {
            seed(json!([{
                "id": "tx1", "userOpHash": "0xuop", "txHash": "0xhash",
                "from": "0xme", "to": "0xyou", "toName": "Ada",
                "value": "1000000000000000000", "symbol": "xDAI", "decimals": 18,
                "chainId": 100, "timestamp": 1_756_000_000, "status": "confirmed",
                "type": "send", "usd": "$1.00"
            }]));
            let records = read_records();
            assert_eq!(records.len(), 1);
            let r = &records[0];
            assert_eq!(r.id, "tx1");
            assert_eq!(r.to_name.as_deref(), Some("Ada"));
            assert_eq!(r.chain_id, 100);
            assert_eq!(r.status, FeedTxStatus::Confirmed);
            assert_eq!(r.kind, Some(FeedTxKind::Send));
            assert_eq!(r.usd.as_deref(), Some("$1.00"));
        });
    }

    /// A dApp row's origin is `dappUrl` when it has one — never the name in
    /// `dappOrigin` beside it. A row written before `dappUrl` existed carries
    /// only `dappOrigin`, which on this shell is always the origin, so it
    /// still names its site (083 H2 review).
    #[test]
    fn a_dapp_row_reads_its_origin_from_dapp_url_first() {
        storage::tests::with_temp_state("feed-dapp-url", || {
            seed(json!([
                {
                    "id": "new", "timestamp": 1_756_000_000, "chainId": 100,
                    "type": "dapp_tx", "dappOrigin": "Uniswap",
                    "dappUrl": "https://app.uniswap.org", "intent": "Swap"
                },
                {
                    "id": "old", "timestamp": 1_755_000_000, "chainId": 100,
                    "type": "dapp_tx", "dappOrigin": "http://127.0.0.1:5173"
                }
            ]));
            let records = read_records();
            assert_eq!(
                records[0].dapp_url.as_deref(),
                Some("https://app.uniswap.org")
            );
            assert_eq!(records[0].intent.as_deref(), Some("Swap"));
            assert_eq!(
                records[1].dapp_url.as_deref(),
                Some("http://127.0.0.1:5173")
            );
            assert_eq!(records[1].intent, None);
        });
    }

    /// Spec 093: a dApp row's `dappSummary` — a transaction's or a
    /// signature's — goes back to the core exactly as it was stored, and one
    /// that is not the core's shape is no summary rather than a guess (the
    /// core then reads the row by its kind). A signature's origin is mapped
    /// like a transaction's.
    #[test]
    fn a_dapp_rows_summary_reads_back_verbatim_or_not_at_all() {
        use vela_core::app::dapp_activity::{DappAction, DappSummary};
        storage::tests::with_temp_state("feed-dapp-summary", || {
            seed(json!([
                {
                    "id": "permit", "timestamp": 1_759_100_000, "chainId": 1,
                    "type": "sign_typed_data", "txHash": "",
                    "dappUrl": "https://app.uniswap.org",
                    "dappSummary": {
                        "action": "permit",
                        "contract": "0x000000000022d473030f116ddee9f6b43ac78ba3",
                        "spender": "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad",
                        "token": "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48",
                        "symbol": "USDC", "decimals": 6, "unlimited": true,
                        "primary_type": "PermitSingle"
                    }
                },
                {
                    "id": "odd", "timestamp": 1_759_000_000, "chainId": 1,
                    "type": "sign_message", "dappUrl": "https://app.uniswap.org",
                    "dappSummary": { "action": "teleport" }
                },
                { "id": "old", "timestamp": 1_758_000_000, "chainId": 1, "type": "dapp_tx" }
            ]));
            let records = read_records();
            assert_eq!(
                records[0].summary,
                Some(DappSummary {
                    action: DappAction::Permit,
                    contract: Some("0x000000000022d473030f116ddee9f6b43ac78ba3".to_owned()),
                    spender: Some("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad".to_owned()),
                    token: Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_owned()),
                    symbol: Some("USDC".to_owned()),
                    decimals: Some(6),
                    unlimited: true,
                    primary_type: Some("PermitSingle".to_owned()),
                    ..DappSummary::default()
                })
            );
            assert_eq!(records[0].kind, Some(FeedTxKind::SignTypedData));
            assert_eq!(
                records[0].dapp_url.as_deref(),
                Some("https://app.uniswap.org")
            );
            assert_eq!(records[1].summary, None, "not the core's shape");
            assert_eq!(records[2].summary, None, "a record from before 093");
        });
    }

    /// Spec 097: the tracker's settlement, kept beside the status by the
    /// patch, reads back untouched — and one this build cannot read is none.
    #[test]
    fn a_dapp_rows_settlement_reads_back_verbatim_or_not_at_all() {
        use vela_core::app::tx_tracker::{TrackFailure, TrackMove, TrackSettlement};
        storage::tests::with_temp_state("feed-dapp-settlement", || {
            seed(json!([
                {
                    "id": "borrow", "timestamp": 1_759_100_000, "chainId": 56,
                    "type": "dapp_tx", "status": "confirmed",
                    "settlement": { "moved": [
                        { "token": "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d",
                          "delta": "300000000000000000" },
                        { "token": null, "delta": "1499036349071560" }
                    ] }
                },
                {
                    "id": "refused", "timestamp": 1_759_000_000, "chainId": 56,
                    "type": "dapp_tx", "status": "failed",
                    "settlement": { "failure": "refused" }
                },
                {
                    "id": "odd", "timestamp": 1_758_000_000, "chainId": 56,
                    "type": "dapp_tx", "settlement": { "failure": "exploded" }
                }
            ]));
            let records = read_records();
            assert_eq!(
                records[0].settlement,
                Some(TrackSettlement {
                    moved: Some(vec![
                        TrackMove {
                            token: Some("0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d".to_owned()),
                            delta: "300000000000000000".to_owned(),
                        },
                        TrackMove {
                            token: None,
                            delta: "1499036349071560".to_owned(),
                        },
                    ]),
                    failure: None,
                })
            );
            assert_eq!(
                records[1].settlement.as_ref().and_then(|s| s.failure),
                Some(TrackFailure::Refused)
            );
            assert_eq!(records[2].settlement, None, "not the core's shape");
        });
    }

    /// Spec 093: the technical section reads a record's stored request by
    /// its id — this shell's text as it was kept, the params of a row the web
    /// wrote — and nothing for a record that kept none or is gone.
    #[test]
    fn a_records_stored_request_is_read_by_its_id() {
        storage::tests::with_temp_state("feed-stored-request", || {
            seed(json!([
                { "id": "desk", "timestamp": 1, "chainId": 1, "type": "sign_message",
                  "signedRequest": r#"["0x48656c6c6f","0xme"]"# },
                { "id": "web", "timestamp": 1, "chainId": 1, "type": "dapp_tx",
                  "signedRequest": { "method": "eth_sendTransaction", "params": [{ "to": "0xr" }] } },
                { "id": "none", "timestamp": 1, "chainId": 1, "type": "sign_message",
                  "signedRequest": "" },
                { "id": "old", "timestamp": 1, "chainId": 1, "type": "dapp_tx" }
            ]));
            assert_eq!(
                stored_request("desk").as_deref(),
                Some(r#"["0x48656c6c6f","0xme"]"#)
            );
            assert_eq!(stored_request("web").as_deref(), Some(r#"[{"to":"0xr"}]"#));
            assert_eq!(stored_request("none"), None);
            assert_eq!(stored_request("old"), None);
            assert_eq!(stored_request("gone"), None);
        });
    }

    /// 083 F1: a row's `assetChanges` — this shell's or the web's, the same
    /// bytes — goes back to the judgments the sheet drew. A token is in the
    /// trusted set only when its line says so. A token marked unverified, or
    /// missing its symbol or decimals, reads as unverified (no figure), never
    /// as a trusted amount; a line of no known kind, or with no delta, is
    /// dropped; a row without the field has none.
    #[test]
    fn a_rows_balance_changes_read_back_as_the_sheets_judgments() {
        use vela_core::app::token_trust::TrustSimJudgment as J;
        storage::tests::with_temp_state("feed-asset-changes", || {
            seed(json!([
                {
                    "id": "swap", "timestamp": 1_759_100_000, "chainId": 8453,
                    "type": "dapp_tx",
                    "assetChanges": { "ok": true, "engine": "rpc", "changes": [
                        { "kind": "erc20", "token": "0xusdc", "delta": "-100000",
                          "symbol": "USDC", "decimals": 6, "trusted": true },
                        { "kind": "native", "delta": "37000000000000",
                          "symbol": "ETH", "decimals": 18 },
                        { "kind": "erc20", "token": "0xweb", "delta": "-5",
                          "symbol": "WEB", "decimals": 6 },
                        { "kind": "erc20", "token": "0xbad", "delta": "9000",
                          "symbol": "FREE", "decimals": 18, "unverified": true },
                        { "kind": "erc20", "token": "0xodd", "delta": "5", "symbol": "ODD" },
                        { "kind": "erc721", "delta": "1" },
                        { "kind": "native" }
                    ] }
                },
                { "id": "old", "timestamp": 1_759_000_000, "chainId": 8453, "type": "dapp_tx" }
            ]));
            let records = read_records();
            assert_eq!(
                records[0].balance_changes,
                Some(vec![
                    J::Erc20Trusted {
                        token: "0xusdc".to_owned(),
                        delta: "-100000".to_owned(),
                        symbol: "USDC".to_owned(),
                        decimals: 6,
                        in_trusted_set: true,
                    },
                    J::Native {
                        delta: "37000000000000".to_owned(),
                    },
                    // No `trusted` word (the web writes none): drawn, never
                    // the row's figure.
                    J::Erc20Trusted {
                        token: "0xweb".to_owned(),
                        delta: "-5".to_owned(),
                        symbol: "WEB".to_owned(),
                        decimals: 6,
                        in_trusted_set: false,
                    },
                    J::Erc20Unverified {
                        token: Some("0xbad".to_owned()),
                        delta: "9000".to_owned(),
                    },
                    J::Erc20Unverified {
                        token: Some("0xodd".to_owned()),
                        delta: "5".to_owned(),
                    },
                ])
            );
            assert_eq!(records[1].balance_changes, None);
        });
    }

    /// 083 F3: whether a dApp's transaction carried calldata.
    ///
    /// The row's own `calldata` first — the sign executor decided it from the
    /// whole request — so a request clipped at 8 KB still says. Only an older
    /// row is read off its stored request, and by what each method submits:
    /// a single transaction's own `data` (this shell's text, stored beside its
    /// `to`; the web's object, by its `method`), a batch's `calls` (this
    /// shell's rows stored with no `to`). A `calls` a page wrote beside a
    /// single transaction decides nothing either way (083 F3 review). A
    /// clipped or missing request says nothing, and neither does any other
    /// kind of row.
    #[test]
    fn a_dapp_row_says_whether_it_called_a_contract() {
        storage::tests::with_temp_state("feed-calldata", || {
            let row = |id: &str, kind: &str, to: &str, request: Value, truncated: bool| {
                json!({
                    "id": id, "timestamp": 1_759_100_000, "chainId": 8453, "type": kind,
                    "to": to, "signedRequest": request, "requestTruncated": truncated
                })
            };
            let mut recorded_call = row(
                "recorded",
                "dapp_tx",
                "0xr",
                json!(r#"[{"to":"0xr","da"#),
                true,
            );
            recorded_call["calldata"] = json!(true);
            let mut recorded_send = row(
                "recorded-send",
                "dapp_tx",
                "0xb",
                json!(r#"[{"to":"0xb","value":"0x1","data":"0xabcd"}]"#),
                false,
            );
            // The executor's verdict stands over a stored request that
            // disagrees with it (it read the whole one).
            recorded_send["calldata"] = json!(false);
            seed(json!([
                recorded_call,
                recorded_send,
                row("swap", "dapp_tx", "0xr", json!(r#"[{"to":"0xr","data":"0x3593564c"}]"#), false),
                row("send", "dapp_tx", "0xb", json!(r#"[{"to":"0xb","value":"0x1","data":"0x"}]"#), false),
                row("bare", "dapp_tx", "0xb", json!(r#"[{"to":"0xb","value":"0x1"}]"#), false),
                row(
                    "web",
                    "dapp_tx",
                    "0xr",
                    json!({ "method": "eth_sendTransaction", "params": [{ "to": "0xr", "data": "0xabcd" }] }),
                    false
                ),
                row(
                    "batch",
                    "dapp_tx",
                    "",
                    json!(r#"[{"calls":[{"to":"0xb","value":"0x1"},{"to":"0xr","data":"0x095ea7b3"}]}]"#),
                    false
                ),
                row(
                    "web-batch",
                    "dapp_tx",
                    "",
                    json!({ "method": "wallet_sendCalls", "params": [{ "calls": [{ "to": "0xr", "data": "0xabcd" }] }] }),
                    false
                ),
                // A plain 1 ETH send with a `calls` the page added: the send
                // is what went out, and its recipient stays one.
                row(
                    "smuggled",
                    "dapp_tx",
                    "0xeoa",
                    json!(r#"[{"to":"0xeoa","value":"0xde0b6b3a7640000","calls":[{"data":"0x01"}]}]"#),
                    false
                ),
                // …and the reverse: a router call with an empty `calls`.
                row(
                    "emptied",
                    "dapp_tx",
                    "0xr",
                    json!(r#"[{"to":"0xr","data":"0x3593","calls":[]}]"#),
                    false
                ),
                row(
                    "web-smuggled",
                    "dapp_tx",
                    "0xeoa",
                    json!({ "method": "eth_sendTransaction", "params": [{ "to": "0xeoa", "calls": [{ "data": "0x01" }] }] }),
                    false
                ),
                row("clipped", "dapp_tx", "0xr", json!(r#"[{"to":"0xr","data":"0x35"#), true),
                row("junk", "dapp_tx", "0xr", json!("not json"), false),
                row("message", "sign_message", "", json!(r#"[{"data":"0xabcd"}]"#), false),
                { "id": "none", "timestamp": 1_759_100_000, "chainId": 8453, "type": "dapp_tx" }
            ]));
            let calldata: Vec<(String, Option<bool>)> = read_records()
                .into_iter()
                .map(|record| (record.id, record.calldata))
                .collect();
            assert_eq!(
                calldata,
                vec![
                    ("recorded".to_owned(), Some(true)),
                    ("recorded-send".to_owned(), Some(false)),
                    ("swap".to_owned(), Some(true)),
                    ("send".to_owned(), Some(false)),
                    ("bare".to_owned(), Some(false)),
                    ("web".to_owned(), Some(true)),
                    ("batch".to_owned(), Some(true)),
                    ("web-batch".to_owned(), Some(true)),
                    ("smuggled".to_owned(), Some(false)),
                    ("emptied".to_owned(), Some(true)),
                    ("web-smuggled".to_owned(), Some(false)),
                    ("clipped".to_owned(), None),
                    ("junk".to_owned(), None),
                    ("message".to_owned(), None),
                    ("none".to_owned(), None),
                ]
            );
        });
    }

    /// A row with no `type` is a record older than the field. The core reads
    /// `None` as `send` by its own rule — so the shell must report `None` and
    /// not decide on its behalf.
    #[test]
    fn a_legacy_row_reports_no_kind_rather_than_guessing() {
        storage::tests::with_temp_state("feed-legacy", || {
            seed(json!([{ "id": "old", "timestamp": 1_700_000_000, "chainId": 1 }]));
            let records = read_records();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].kind, None, "the core owns the legacy default");
            assert_eq!(
                records[0].decimals, 18,
                "an absent decimals is the usual 18"
            );
        });
    }

    /// One unparseable row must not cost the feed.
    #[test]
    fn a_row_without_an_id_is_skipped_not_fatal() {
        storage::tests::with_temp_state("feed-junk", || {
            seed(json!([
                { "timestamp": 1 },
                { "id": "good", "timestamp": 1_756_000_000, "chainId": 100 }
            ]));
            let records = read_records();
            assert_eq!(records.len(), 1);
            assert_eq!(records[0].id, "good");
        });
    }

    /// Local midnight, not UTC midnight, and the two differ for most people.
    #[test]
    fn the_day_boundary_is_local() {
        let noon_utc = 1_756_040_000_000.0;
        let start = crate::executor::day_start_ms(noon_utc);
        const DAY_MS: f64 = 86_400_000.0;

        assert!(start <= noon_utc, "a day starts before the moment in it");
        assert!(
            noon_utc - start < DAY_MS,
            "and no more than a day before it"
        );
        // Two instants in the same local day share a boundary; one a day apart
        // does not. That holds in every timezone, which is what makes it a
        // usable assertion on a machine whose zone the test does not know.
        assert_eq!(
            crate::executor::day_start_ms(noon_utc + 3_600_000.0),
            start,
            "an hour later is the same local day"
        );
        assert_ne!(
            crate::executor::day_start_ms(noon_utc + DAY_MS),
            start,
            "a day later is not"
        );
    }

    /// Deleting a row that is not there is a FAILURE, not a quiet success: the
    /// row is still on the person's screen and the core has to know.
    #[test]
    fn deleting_a_missing_record_reports_failure() {
        storage::tests::with_temp_state("feed-delete", || {
            seed(json!([{ "id": "tx1", "timestamp": 1, "chainId": 1 }]));

            match ActivityFeed::perform(&FeedOperation::DeleteTxRecord {
                id: "tx1".to_owned(),
            }) {
                Answer::Now(FeedShellResult::DeleteCommitted { id }) => assert_eq!(id, "tx1"),
                other => unreachable!(
                    "the delete should have committed: {other:?}",
                    other = match other {
                        Answer::Now(result) => format!("{result:?}"),
                        _ => "non-local".to_owned(),
                    }
                ),
            }
            assert!(read_records().is_empty());

            match ActivityFeed::perform(&FeedOperation::DeleteTxRecord {
                id: "gone".to_owned(),
            }) {
                Answer::Now(FeedShellResult::DeleteFailed { id }) => assert_eq!(id, "gone"),
                _ => unreachable!("deleting nothing must report failure"),
            }
        });
    }
}
