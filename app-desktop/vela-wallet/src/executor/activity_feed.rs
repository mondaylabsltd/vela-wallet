//! The only place the `activity_feed` machine touches the outside world.
//!
//! Six operations: the local transaction store, receipt discovery, a delete, a
//! counterparty lookup, the toast timer, and a haptic the desktop does not have.
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
fn to_record(row: &Value) -> Option<FeedTxRecord> {
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
    })
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
    // One scan at a time. Ticks are 30 s apart and a twelve-chain discovery on
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
    let incoming = crate::executor::token_trust::poll(address);
    if incoming.is_empty() {
        return 0;
    }

    let mut rows = match storage::read_value(TX_KEY) {
        Ok(Some(Value::Array(rows))) => rows,
        _ => Vec::new(),
    };
    let mut known: std::collections::BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();

    let mut added = 0u32;
    for transfer in &incoming {
        if known.contains(&transfer.id) {
            continue;
        }
        // A non-native token whose metadata would not resolve is SKIPPED, not
        // stored at a guessed scale. The web says why: an 18-decimal fallback
        // on a 6-decimal token stores a misleading "+0 tokens", and the
        // transfer stays in the scan window to be retried once metadata
        // resolves. Genuine spam with no readable symbol never reaches the feed.
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

    if added == 0 {
        return 0;
    }
    if storage::write_value(TX_KEY, Value::Array(rows)).is_err() {
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
        "status": "confirmed",
        "type": "receive",
        // No `usd`. The ingest valuation needs a live price this path does not
        // hold, and the core RE-DERIVES the value on read (`tx_usd_value`),
        // including the ≈$1 stablecoin fallback. Writing "$0.00" would store a
        // claim; writing nothing lets the rule that owns it decide.
    }))
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

fn read_records() -> Vec<FeedTxRecord> {
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
                let removed = match storage::read_value(TX_KEY) {
                    Ok(Some(Value::Array(rows))) => {
                        let before = rows.len();
                        let kept: Vec<Value> = rows
                            .into_iter()
                            .filter(|row| row.get("id").and_then(Value::as_str) != Some(&id))
                            .collect();
                        let removed = kept.len() != before;
                        storage::write_value(TX_KEY, Value::Array(kept)).is_ok() && removed
                    }
                    _ => false,
                };
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
        }
    }
}

static TICKING: AtomicBool = AtomicBool::new(false);

/// The cadence the core does NOT own.
///
/// Its words: "`FocusTick`/`LiveTick` cadence (focus + 30s auto-refresh, 10s
/// while the Activity tab is visible) stays in the shell: which tab is visible
/// is render-domain state the core never sees." A desktop window has one
/// screen, always mounted, and no tab that comes and goes — so it takes the
/// slower of the two and takes it always.
///
/// Thirty seconds is also the honest ceiling for what a tick costs here: each
/// one is a receipt discovery across every chain the person holds on, not a
/// local read.
const TICK: Duration = Duration::from_secs(30);

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
        loop {
            cx.background_executor().timer(TICK).await;
            // Re-fetched each tick, for the reason the tracker's loop states:
            // a sign-out drops every resident, and the next sign-in's feed is
            // the one that must get the ticks.
            let feed = cx.update(|cx| resident::resident::<ActivityFeed>(cx));
            feed.update(cx, |resident, cx| resident.dispatch(Event::FocusTick, cx));
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
