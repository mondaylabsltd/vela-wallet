//! Rules of the Activity feed, one test per invariant (inventory
//! `activity_feed` ① – ⑧ plus the celebration lifecycle and staleness), and
//! spec 082's rows the core decides: dApp transactions with their status and
//! site (RG1–RG4) and the empty-line keys (RG5).
//!
//! The fake clock is `now_ms` on each `StoreLoaded`; day keys are
//! shell-supplied `day_start_ms` values — the core never owns time or
//! timezone.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::activity_feed::{
    dapp_site, history_empty_key, home_empty_key, is_stable, native_amount, tx_usd_value,
    ActivityFeed, Event, FeedBatchKind, FeedCounterpartyRole, FeedDirection, FeedItem,
    FeedOperation as Op, FeedRow, FeedShellResult as Res, FeedTxKind, FeedTxRecord, FeedTxStatus,
    FeedView, HISTORY_EMPTY_ALL, HISTORY_EMPTY_FILTERED, HOME_EMPTY_ALL, HOME_EMPTY_FILTERED,
};

type Sut = DomainDriver<ActivityFeed>;

const ADDR: &str = "0xA11ceFeedAA";
const OTHER: &str = "0xSomebodyElse";
const T0: f64 = 1_754_700_000_000.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

/// A deterministic "local midnight" for a fixture timestamp — what the shell
/// would supply from the device timezone.
fn day_of(ts_seconds: f64) -> f64 {
    (ts_seconds / 86_400.0).floor() * 86_400_000.0
}

fn base(id: &str, ts: f64) -> FeedTxRecord {
    FeedTxRecord {
        id: id.to_owned(),
        user_op_hash: String::new(),
        tx_hash: format!("0xtx{id}"),
        from: String::new(),
        to: String::new(),
        to_name: None,
        value: "1".to_owned(),
        symbol: "USDC".to_owned(),
        decimals: 6,
        logo_urls: None,
        chain_id: 8453,
        timestamp: ts,
        day_start_ms: day_of(ts),
        status: FeedTxStatus::Confirmed,
        kind: None,
        usd: None,
        dapp_origin: None,
        call_data: None,
    }
}

/// An outgoing transfer from the active account. `to_name` is set so the
/// fixture generates no alias traffic unless a test wants it.
fn send(id: &str, uoh: &str, to: &str, value: &str, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::Send);
    r.user_op_hash = uoh.to_owned();
    r.from = ADDR.to_owned();
    r.to = to.to_owned();
    r.to_name = Some(format!("name-of-{to}"));
    r.value = value.to_owned();
    r
}

/// An incoming transfer to the active account.
fn recv(id: &str, from: &str, value: &str, symbol: &str, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::Receive);
    r.from = from.to_owned();
    r.to = ADDR.to_owned();
    r.value = value.to_owned();
    r.symbol = symbol.to_owned();
    r
}

/// A transaction a dApp asked for, as the shells store it
/// (`buildSigningRecord`): wei `value`, the native symbol, 18 decimals, the
/// asking site's origin.
fn dapp_tx(id: &str, origin: &str, to: &str, value: &str, ts: f64) -> FeedTxRecord {
    let mut r = base(id, ts);
    r.kind = Some(FeedTxKind::DappTx);
    r.from = ADDR.to_owned();
    r.to = to.to_owned();
    r.to_name = Some(format!("name-of-{to}"));
    r.value = value.to_owned();
    r.symbol = "xDAI".to_owned();
    r.decimals = 18;
    r.chain_id = 100;
    r.user_op_hash = format!("0xop{id}");
    r.dapp_origin = Some(origin.to_owned());
    r
}

/// A `StoreLoaded` echoing an explicit read id.
fn loaded_for(read_id: u32, records: Vec<FeedTxRecord>, now_ms: f64) -> Res {
    Res::StoreLoaded {
        records,
        now_ms,
        read_id,
    }
}

/// A `StoreLoaded` for the OLDEST outstanding read — what a shell answering in
/// order produces, and what every test here means unless it says otherwise.
fn loaded(sut: &Sut, records: Vec<FeedTxRecord>, now_ms: f64) -> Res {
    loaded_for(oldest_read_id(sut), records, now_ms)
}

/// The id the oldest outstanding `ReadTxStore` is waiting to be echoed.
fn oldest_read_id(sut: &Sut) -> u32 {
    sut.outstanding()
        .iter()
        .find_map(|op| match op {
            Op::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a ReadTxStore is outstanding")
}

/// Matches any `ReadTxStore` for the default address, whatever its id.
fn read_op_for(address: &str, read_id: u32) -> Op {
    Op::ReadTxStore {
        address: address.to_owned(),
        read_id,
    }
}

fn read_op() -> Op {
    read_op_for(ADDR, 0)
}

/// Zero every read id so an assertion can name the SHAPE of the requested
/// operations without pinning ids the core mints internally.
fn shapes(ops: Vec<Op>) -> Vec<Op> {
    ops.into_iter()
        .map(|op| match op {
            Op::ReadTxStore { address, .. } => Op::ReadTxStore {
                address,
                read_id: 0,
            },
            other => other,
        })
        .collect()
}

fn scan_op() -> Op {
    Op::ScanIncomingTransfers {
        address: ADDR.to_owned(),
    }
}

/// Answer every outstanding alias request (front of the queue) with "nothing
/// resolved" so later assertions see a clean queue.
fn drain_aliases(sut: &mut Sut) {
    while let Some(Op::ResolveRecipientIdentity { addr }) = sut.outstanding().first().cloned() {
        sut.resolve(Res::AliasResolved { addr, name: None });
    }
}

/// Switch to `ADDR`, commit the given records, run a no-op first sync pass
/// (so `initialized` is spent) and drain any alias traffic.
fn boot(records: Vec<FeedTxRecord>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    assert_eq!(
        shapes(ops),
        vec![read_op(), scan_op()],
        "tick = read + scan"
    );
    sut.resolve(loaded(&sut, records, T0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    drain_aliases(&mut sut);
    sut
}

/// The item payloads of the current rows, in order.
fn items(sut: &Sut) -> Vec<vela_core::app::activity_feed::FeedItem> {
    sut.view()
        .rows
        .into_iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item),
            FeedRow::Header { .. } => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// ① — one row per id; batch folding with id-dedupe first
// ---------------------------------------------------------------------------

/// A legacy same-id duplicate (a resubmitted single send) renders exactly one
/// row and is NOT mistaken for a batch (`activity.ts:436-451`).
#[test]
fn same_id_duplicate_renders_once_and_is_not_a_batch() {
    let dup = send("op1-0", "0xHASH", "0xCafe", "10", 100_000.0);
    let sut = boot(vec![dup.clone(), dup]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, "op1-0");
    assert!(rows[0].batch.is_none(), "a duplicate is not a batch of two");
    assert_eq!(rows[0].value.as_deref(), Some("10"));
}

/// Sibling lines sharing one userOpHash fold to ONE row carrying the
/// breakdown; split = one token, many recipients — the summed figure is a
/// decimal string the shell formats.
#[test]
fn batch_siblings_fold_to_one_split_row() {
    let mut a = send("h-0", "0xH", "0xAaa", "10", 100_000.0);
    let mut b = send("h-1", "0xH", "0xBbb", "20", 100_000.0);
    let mut c = send("h-2", "0xH", "0xCcc", "30", 100_000.0);
    a.usd = Some("$10.00".to_owned());
    b.usd = Some("$20.00".to_owned());
    c.usd = Some("$30.00".to_owned());
    let solo = recv("r-solo", "0xBob", "5", "USDT", 90_000.0);
    let mut sut = boot(vec![a, b, c, solo]);
    drain_aliases(&mut sut);

    let rows = items(&sut);
    assert_eq!(rows.len(), 2, "three siblings became one row");
    let batch_row = &rows[0];
    assert_eq!(batch_row.id, "0xH", "row id is the shared userOpHash");
    assert_eq!(batch_row.direction, FeedDirection::Out);
    assert_eq!(
        batch_row.value.as_deref(),
        Some("60"),
        "split sums one token"
    );
    assert_eq!(batch_row.symbol, "USDC");
    assert_eq!(batch_row.decimals, Some(6));
    assert!(batch_row.counterparty.is_none(), "no single recipient");
    assert!((batch_row.usd_value - 60.0).abs() < 1e-9);
    let batch = batch_row.batch.as_ref().expect("batch payload");
    assert_eq!(batch.kind, FeedBatchKind::Split);
    assert_eq!(batch.count, 3);
    assert_eq!(batch.ids, vec!["h-0", "h-1", "h-2"]);
    assert_eq!(batch.symbol.as_deref(), Some("USDC"));
    assert_eq!(batch.transfers.len(), 3);
}

/// Many tokens to one recipient = multi_select: no summable token figure
/// (`value` is None), the single recipient is the counterparty.
#[test]
fn multi_token_batch_is_multi_select() {
    let mut a = send("m-0", "0xM", "0xSameGuy", "10", 100_000.0);
    let mut b = send("m-1", "0xM", "0xSameGuy", "3", 100_000.0);
    a.symbol = "USDC".to_owned();
    b.symbol = "DAI".to_owned();
    let sut = boot(vec![a, b]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert!(row.value.is_none(), "mixed tokens cannot sum");
    assert_eq!(row.symbol, "", "token: b.symbol ?? ''");
    assert_eq!(row.decimals, None);
    assert_eq!(row.counterparty.as_deref(), Some("0xSameGuy"));
    let batch = row.batch.as_ref().expect("batch payload");
    assert_eq!(batch.kind, FeedBatchKind::MultiSelect);
    assert_eq!(batch.to.as_deref(), Some("0xSameGuy"));
    assert!(batch.symbol.is_none());
}

/// A resubmitted line INSIDE a real batch counts once toward the group size
/// and the sum (`ids.has(t.id)` before the group push).
#[test]
fn resubmitted_line_in_a_real_batch_counts_once() {
    let a = send("h-0", "0xH", "0xAaa", "10", 100_000.0);
    let b = send("h-1", "0xH", "0xBbb", "20", 100_000.0);
    let sut = boot(vec![a.clone(), b, a]);
    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let batch = rows[0].batch.as_ref().expect("batch payload");
    assert_eq!(batch.count, 2, "the duplicate line counted once");
    assert_eq!(rows[0].value.as_deref(), Some("30"));
}

/// Feed scoping: only this account's transfers and the transactions its
/// dApps asked for — signatures and connects excluded — and the raw
/// `transactions` view is scoped the same way (spec 082 RG1 rewrote this
/// test, which used to pin the dApp transaction's exclusion).
#[test]
fn feed_and_transactions_are_account_scoped() {
    let ours_send = send("s1", "0xU1", "0xCafe", "1", 300_000.0);
    let mut foreign_send = send("s2", "0xU2", "0xCafe", "1", 290_000.0);
    foreign_send.from = OTHER.to_owned();
    let ours_recv = recv("r1", "0xBob", "2", "USDT", 280_000.0);
    let mut foreign_recv = recv("r2", "0xBob", "2", "USDT", 270_000.0);
    foreign_recv.to = OTHER.to_owned();
    let dapp = dapp_tx("d1", "https://app.test", "0xRouter", "0x0", 260_000.0);
    let mut foreign_dapp = dapp_tx("d2", "https://app.test", "0xRouter", "0x0", 255_000.0);
    foreign_dapp.from = OTHER.to_owned();
    // Legacy untyped record ⇒ send (`t.type ?? 'send'`), matched case-insensitively.
    let mut legacy = base("l1", 250_000.0);
    legacy.from = ADDR.to_uppercase();
    legacy.to = "0xCafe".to_owned();
    legacy.to_name = Some("Ann".to_owned());
    let signed: Vec<FeedTxRecord> = [
        FeedTxKind::SignMessage,
        FeedTxKind::SignTypedData,
        FeedTxKind::Connect,
    ]
    .into_iter()
    .enumerate()
    .map(|(n, kind)| {
        let mut r = base(&format!("x{n}"), 240_000.0);
        r.kind = Some(kind);
        r.from = ADDR.to_owned();
        r.dapp_origin = Some("https://app.test".to_owned());
        r
    })
    .collect();

    let mut records = vec![
        ours_send,
        foreign_send,
        ours_recv,
        foreign_recv,
        dapp,
        foreign_dapp,
        legacy,
    ];
    records.extend(signed);
    let mut sut = boot(records);
    drain_aliases(&mut sut);

    let ids: Vec<String> = items(&sut).into_iter().map(|i| i.id).collect();
    assert_eq!(ids, vec!["s1", "r1", "d1", "l1"]);
    let tx_ids: Vec<String> = sut.view().transactions.into_iter().map(|t| t.id).collect();
    assert_eq!(tx_ids, vec!["s1", "r1", "d1", "l1"]);
}

// ---------------------------------------------------------------------------
// Spec 082 — rows the core decides (RG1–RG4)
// ---------------------------------------------------------------------------

/// A dApp transaction just submitted is a pending `DappTx` row, out from this
/// account, naming the site that asked (`host[:port]`, nothing else of the
/// origin) and the contract it called.
#[test]
fn a_pending_dapp_transaction_is_a_row_with_its_site() {
    let mut pending = dapp_tx(
        "dapp-1-tx",
        "http://127.0.0.1:8137",
        "0xRouter",
        "0x2386f26fc10000",
        100_000.0,
    );
    pending.status = FeedTxStatus::Pending;
    pending.tx_hash = String::new();
    let sut = boot(vec![pending]);

    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.id, "dapp-1-tx");
    assert_eq!(row.kind, FeedTxKind::DappTx);
    assert_eq!(row.status, FeedTxStatus::Pending);
    assert_eq!(row.site.as_deref(), Some("127.0.0.1:8137"));
    assert_eq!(row.direction, FeedDirection::Out);
    assert_eq!(row.counterparty.as_deref(), Some("0xRouter"));
    assert_eq!(row.chain_id, 100);
    assert_eq!(row.tx_hash, None, "no on-chain hash yet");
    // 0x2386f26fc10000 wei = 0.01 of the native coin.
    assert_eq!(row.value.as_deref(), Some("0.01"));
    assert_eq!(row.symbol, "xDAI");
    assert_eq!(row.decimals, Some(18));
    assert!(row.batch.is_none());
}

/// The row carries the record's status as the tracker left it: confirmed with
/// its hash, or failed.
#[test]
fn a_dapp_row_carries_confirmed_and_failed() {
    let confirmed = dapp_tx("c", "https://app.uniswap.org", "0xR", "0", 200_000.0);
    let mut failed = dapp_tx("f", "https://app.uniswap.org", "0xR", "0", 100_000.0);
    failed.status = FeedTxStatus::Failed;
    let sut = boot(vec![confirmed, failed]);

    let rows = items(&sut);
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].status, FeedTxStatus::Confirmed);
    assert_eq!(rows[0].tx_hash.as_deref(), Some("0xtxc"));
    assert_eq!(rows[1].status, FeedTxStatus::Failed);
    assert!(rows
        .iter()
        .all(|r| r.site.as_deref() == Some("app.uniswap.org")));
}

/// A submit whose reply was lost (MaybeSent) is persisted pending under the
/// LOCAL operation hash with no tx hash; it is a pending `DappTx` row like any
/// other until the tracker patches the record (RG4) — never failed on time,
/// never hidden.
#[test]
fn a_maybe_sent_op_under_its_local_hash_is_a_pending_row() {
    let local_hash = "0xc6f3544fc4e3ac769e92c92ab4804cd3f38ffb8d607ba07b5103a59710094dc4";
    let mut maybe_sent = dapp_tx(
        "dapp-9-tx",
        "https://app.test",
        "0xRouter",
        "0x0",
        100_000.0,
    );
    maybe_sent.user_op_hash = local_hash.to_owned();
    maybe_sent.tx_hash = String::new();
    maybe_sent.status = FeedTxStatus::Pending;
    let sut = boot(vec![maybe_sent]);

    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].kind, FeedTxKind::DappTx);
    assert_eq!(rows[0].status, FeedTxStatus::Pending);
    assert_eq!(rows[0].site.as_deref(), Some("app.test"));
    assert_eq!(rows[0].tx_hash, None);
    let tx = &sut.view().transactions[0];
    assert_eq!(
        tx.user_op_hash, local_hash,
        "the detail sheet keeps the hash"
    );
}

/// `personal_sign` (and every other signature) moves nothing: its record is
/// never a row, even with the dApp's origin on it.
#[test]
fn a_personal_sign_is_never_a_row() {
    let mut msg = base("dapp-5-msg", 100_000.0);
    msg.kind = Some(FeedTxKind::SignMessage);
    msg.from = ADDR.to_owned();
    msg.value = "0".to_owned();
    msg.symbol = String::new();
    msg.dapp_origin = Some("https://app.test".to_owned());
    let sut = boot(vec![msg]);
    assert!(items(&sut).is_empty());
    assert!(sut.view().transactions.is_empty());
}

/// A zero value is no amount (a contract call does not read as money), and
/// wei arrives as hex or decimal.
#[test]
fn a_dapp_row_shows_an_amount_only_for_native_value() {
    let zero = dapp_tx("z", "https://a.test", "0xR", "0x0", 300_000.0);
    let hex = dapp_tx("h", "https://a.test", "0xR", "0xde0b6b3a7640000", 200_000.0);
    let dec = dapp_tx(
        "d",
        "https://a.test",
        "0xR",
        "1500000000000000000",
        100_000.0,
    );
    let sut = boot(vec![zero, hex, dec]);
    let rows = items(&sut);
    assert_eq!(rows[0].value, None);
    assert_eq!(rows[0].symbol, "", "no amount, no symbol");
    assert_eq!(rows[0].decimals, None);
    assert_eq!(rows[0].usd_value, 0.0);
    assert_eq!(rows[1].value.as_deref(), Some("1"));
    assert_eq!(rows[1].symbol, "xDAI");
    assert_eq!(rows[2].value.as_deref(), Some("1.5"));
}

/// The wei parser: hex and decimal, zero and nonsense are no amount.
#[test]
fn native_amounts_from_wei() {
    assert_eq!(
        native_amount("0x1").as_deref(),
        Some("0.000000000000000001")
    );
    assert_eq!(native_amount("0X0DE0B6B3A7640000").as_deref(), Some("1"));
    assert_eq!(
        native_amount(" 250000000000000000 ").as_deref(),
        Some("0.25")
    );
    assert_eq!(native_amount("0x"), None);
    assert_eq!(native_amount("0x0"), None);
    assert_eq!(native_amount("0"), None);
    assert_eq!(native_amount(""), None);
    assert_eq!(native_amount("1.5"), None, "not wei");
    assert_eq!(native_amount("-1"), None);
    assert_eq!(native_amount("+1"), None);
    assert_eq!(native_amount("0xzz"), None);
}

/// The site is the origin's `host[:port]`: default ports dropped, the host
/// lower-cased, and nothing for an origin that is not http(s).
#[test]
fn the_site_is_host_and_port() {
    assert_eq!(
        dapp_site("https://App.Uniswap.org").as_deref(),
        Some("app.uniswap.org")
    );
    assert_eq!(
        dapp_site("http://127.0.0.1:8137").as_deref(),
        Some("127.0.0.1:8137")
    );
    assert_eq!(
        dapp_site("https://example.com:443").as_deref(),
        Some("example.com")
    );
    assert_eq!(
        dapp_site("https://example.com/path?q=1").as_deref(),
        Some("example.com")
    );
    assert_eq!(dapp_site(""), None);
    assert_eq!(dapp_site("chrome-extension://abc"), None);
    assert_eq!(dapp_site("not a url"), None);
}

/// Sends and receives say what they are, with their record's status; a folded
/// batch is a `Send` carrying its FIRST line's status.
#[test]
fn every_row_says_its_kind_and_status_and_a_batch_uses_its_first_line() {
    let mut a = send("h-0", "0xH", "0xAaa", "10", 300_000.0);
    let mut b = send("h-1", "0xH", "0xBbb", "20", 300_000.0);
    a.status = FeedTxStatus::Failed;
    b.status = FeedTxStatus::Confirmed;
    let mut single = send("s", "0xS", "0xCcc", "1", 200_000.0);
    single.status = FeedTxStatus::Pending;
    let incoming = recv("r", "0xBob", "5", "USDT", 100_000.0);
    let mut sut = boot(vec![a, b, single, incoming]);
    drain_aliases(&mut sut);

    let rows = items(&sut);
    assert_eq!(rows.len(), 3);
    assert!(rows[0].batch.is_some());
    assert_eq!(rows[0].kind, FeedTxKind::Send);
    assert_eq!(rows[0].status, FeedTxStatus::Failed, "the first line's");
    assert_eq!(rows[1].kind, FeedTxKind::Send);
    assert_eq!(rows[1].status, FeedTxStatus::Pending);
    assert_eq!(rows[2].kind, FeedTxKind::Receive);
    assert_eq!(rows[2].status, FeedTxStatus::Confirmed);
    assert!(
        rows.iter().all(|r| r.site.is_none()),
        "only a DappTx has a site"
    );

    // The other order: the first line decides either way.
    let mut a = send("g-0", "0xG", "0xAaa", "10", 300_000.0);
    let mut b = send("g-1", "0xG", "0xBbb", "20", 300_000.0);
    a.status = FeedTxStatus::Confirmed;
    b.status = FeedTxStatus::Failed;
    let sut = boot(vec![a, b]);
    assert_eq!(items(&sut)[0].status, FeedTxStatus::Confirmed);
}

/// Old shell JSON still decodes: a record without `dapp_origin`, and an item
/// or view from before 082 (a missing status claims nothing — `Pending` —
/// and a missing kind is the legacy `send`).
#[test]
fn json_from_before_082_still_decodes() {
    let record: FeedTxRecord = serde_json::from_value(serde_json::json!({
        "id": "a", "user_op_hash": "", "tx_hash": "", "from": ADDR, "to": "0xB",
        "to_name": null, "value": "1", "symbol": "USDC", "decimals": 6,
        "logo_urls": null, "chain_id": 8453, "timestamp": 1.0, "day_start_ms": 0.0,
        "status": "confirmed", "kind": "dapp_tx", "usd": null
    }))
    .expect("a record without dapp_origin");
    assert_eq!(record.dapp_origin, None);

    let item: FeedItem = serde_json::from_value(serde_json::json!({
        "id": "a", "direction": "out", "counterparty": "0xB", "alias": null,
        "value": "1", "symbol": "USDC", "decimals": 6, "usd_value": 1.0,
        "chain_id": 8453, "timestamp": 1.0, "day_start_ms": 0.0,
        "tx_hash": null, "batch": null
    }))
    .expect("an item without kind/status/site");
    assert_eq!(item.kind, FeedTxKind::Send);
    assert_eq!(item.status, FeedTxStatus::Pending);
    assert_eq!(item.site, None);

    let view: FeedView = serde_json::from_value(serde_json::json!({
        "rows": [], "transactions": [], "new_item_id": null, "toast": null
    }))
    .expect("a view without the empty keys");
    assert_eq!(view.history_empty_key, "");

    let wire = serde_json::to_value(
        &items(&boot(vec![dapp_tx(
            "w",
            "https://a.test",
            "0xR",
            "0x0",
            1.0,
        )]))[0],
    )
    .expect("serialises");
    assert_eq!(wire["kind"], "dapp_tx");
    assert_eq!(wire["status"], "confirmed");
    assert_eq!(wire["site"], "a.test");
}

// ---------------------------------------------------------------------------
// Spec 082 — the empty lines (RG5, L-D7)
// ---------------------------------------------------------------------------

/// All networks: "No transactions yet" / "No activity yet"; one network: the
/// "on this network" lines. The core chooses, from the chain filter.
#[test]
fn the_empty_keys_follow_the_chain_filter() {
    let mut sut = boot(Vec::new());
    let view = sut.view();
    assert_eq!(view.history_empty_key, "history.emptyTitle");
    assert_eq!(view.home_empty_key, "home.emptyNoActivity");

    sut.dispatch(Event::ChainFilterChanged {
        chain_id: Some(100),
    });
    let view = sut.view();
    assert_eq!(view.history_empty_key, "history.emptyFilter");
    assert_eq!(view.home_empty_key, "home.emptyNoActivityNetwork");

    sut.dispatch(Event::ChainFilterChanged { chain_id: None });
    assert_eq!(sut.view().history_empty_key, HISTORY_EMPTY_ALL);
    assert_eq!(sut.view().home_empty_key, HOME_EMPTY_ALL);

    // A core that was never given an account says the same.
    let fresh = Sut::new();
    assert_eq!(fresh.view().history_empty_key, HISTORY_EMPTY_ALL);

    assert_eq!(history_empty_key(Some(1)), HISTORY_EMPTY_FILTERED);
    assert_eq!(home_empty_key(Some(1)), HOME_EMPTY_FILTERED);
}

/// Every key the feed hands a shell is in the corpus.
#[cfg(feature = "i18n-en")]
#[test]
fn the_empty_keys_are_in_the_corpus() {
    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [
        HISTORY_EMPTY_ALL,
        HISTORY_EMPTY_FILTERED,
        HOME_EMPTY_ALL,
        HOME_EMPTY_FILTERED,
    ] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
}

// ---------------------------------------------------------------------------
// ② — cache paints first; a no-op sync never re-reads (no flicker)
// ---------------------------------------------------------------------------

#[test]
fn cached_feed_survives_a_noop_sync() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);
    let before = sut.view();
    assert_eq!(items(&sut).len(), 1);

    let ops = sut.dispatch(Event::FocusTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
    assert_eq!(sut.view(), before, "nothing blanks while requests are out");

    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert!(ops.is_empty(), "newCount = 0 must not re-read (no flicker)");
    assert_eq!(sut.view(), before);
}

#[test]
fn sync_with_new_receipts_rereads_the_store() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()], "something landed — read it");
}

#[test]
fn live_tick_runs_the_same_pipeline() {
    let mut sut = boot(vec![]);
    let ops = sut.dispatch(Event::LiveTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
}

// ---------------------------------------------------------------------------
// ③ — the first sync pass never celebrates the backlog
// ---------------------------------------------------------------------------

#[test]
fn first_sync_pass_never_celebrates_but_the_second_does() {
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    sut.resolve(loaded(&sut, vec![], T0));
    // First pass discovers a 2-receipt BACKLOG.
    let ops = sut.resolve(Res::SyncCompleted { new_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 1_000.0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbob".to_owned()
        }],
        "no haptic, no toast timer on the first pass"
    );
    assert!(sut.view().toast.is_none(), "history never celebrates");
    assert!(sut.view().new_item_id.is_none());
    sut.resolve(Res::AliasResolved {
        addr: "0xbob".to_owned(),
        name: None,
    });

    // Second pass: a genuinely-new in-session receipt.
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 2_000.0));
    let ops = sut.resolve(Res::SyncCompleted { new_count: 1 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let newer_send = send("s9", "0xU9", "0xCafe", "1", 300_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let ops = sut.resolve(loaded(&sut, vec![newer_send, r2, r1], T0 + 3_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ],
        "buzz + arm the toast countdown"
    );
    let view = sut.view();
    let toast = view.toast.expect("toast shows");
    assert_eq!(
        toast.item_id, "r2",
        "the newest INCOMING item, not the newest row"
    );
    assert_eq!(toast.value, "7");
    assert_eq!(toast.symbol, "USDT");
    assert_eq!(toast.deadline_ms, T0 + 3_000.0 + 2_800.0);
    assert_eq!(view.new_item_id.as_deref(), Some("r2"), "row glow");
}

/// tx_tracker convergence re-reads the store but NEVER celebrates — a flipped
/// pending→confirmed is not new money in.
#[test]
fn reconcile_rereads_without_celebrating() {
    let mut sut = boot(vec![]);
    assert!(sut
        .dispatch(Event::ReconcileCompleted { resolved_count: 0 })
        .is_empty());
    let ops = sut.dispatch(Event::ReconcileCompleted { resolved_count: 2 });
    assert_eq!(shapes(ops), vec![read_op()]);
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r1], T0 + 1_000.0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbob".to_owned()
        }],
        "no haptic / timer"
    );
    assert!(sut.view().toast.is_none());
    assert!(sut.view().new_item_id.is_none());
}

// ---------------------------------------------------------------------------
// ④ — privacy suppresses the toast (and only the toast)
// ---------------------------------------------------------------------------

#[test]
fn privacy_suppresses_the_toast_but_not_glow_or_haptic() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let mut sut = boot(vec![r1.clone()]);
    sut.dispatch(Event::PrivacyChanged { hidden: true });

    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![r1.clone()], T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 1 });
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let ops = sut.resolve(loaded(&sut, vec![r2, r1], T0 + 2_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ],
        "the haptic still fires while hidden (ported: only the toast render is gated)"
    );
    let view = sut.view();
    assert!(view.toast.is_none(), "a toast would leak the masked number");
    assert_eq!(
        view.new_item_id.as_deref(),
        Some("r2"),
        "glow is amount-free"
    );

    // Unhide within the toast window: the withheld state becomes visible.
    sut.dispatch(Event::PrivacyChanged { hidden: false });
    assert_eq!(sut.view().toast.expect("toast").item_id, "r2");
}

// ---------------------------------------------------------------------------
// Toast lifecycle
// ---------------------------------------------------------------------------

fn celebrated(records_before: Vec<FeedTxRecord>, new_record: FeedTxRecord) -> Sut {
    let mut sut = boot(records_before.clone());
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, records_before.clone(), T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 1 });
    let mut all = vec![new_record];
    all.extend(records_before);
    let ops = sut.resolve(loaded(&sut, all, T0 + 2_000.0));
    assert_eq!(
        ops,
        vec![
            Op::Haptic,
            Op::Timer {
                ms: 2_800,
                generation: 1
            }
        ]
    );
    sut.resolve(Res::HapticPlayed);
    sut
}

#[test]
fn toast_expiry_clears_the_toast_but_keeps_the_glow() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);
    assert!(sut.view().toast.is_some());

    sut.resolve(Res::ToastExpired { generation: 1 });
    let view = sut.view();
    assert!(view.toast.is_none(), "2.8s over — the toast goes");
    assert_eq!(
        view.new_item_id.as_deref(),
        Some("r2"),
        "the row glow outlives the toast (newItemId is not timer-cleared)"
    );
}

/// A timer echo bearing a stale generation (a superseded celebration's
/// countdown firing late) must never clear the current toast — the core twin
/// of `clearTimeout(toastTimer.current)`.
#[test]
fn stale_toast_timer_is_ignored() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);

    let ops = sut.resolve(Res::ToastExpired { generation: 42 });
    assert!(ops.is_empty(), "a stale echo is a no-op");
    assert!(sut.view().toast.is_some(), "the live toast survives");
}

// ---------------------------------------------------------------------------
// ⑤ — tombstones: a deleted row cannot be repainted by a concurrent reload
// ---------------------------------------------------------------------------

#[test]
fn deleted_row_is_not_resurrected_by_a_concurrent_reload() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    // A background reload is already in flight (it read storage BEFORE the
    // delete write lands)…
    sut.dispatch(Event::FocusTick);
    // …when the user deletes the row.
    let ops = sut.dispatch(Event::DeleteRequested {
        id: "s1".to_owned(),
    });
    assert_eq!(
        ops,
        vec![Op::DeleteTxRecord {
            id: "s1".to_owned()
        }]
    );
    assert!(items(&sut).is_empty(), "optimistic removal is instant");

    // The stale read commits — the tombstone filters the ghost.
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    assert!(items(&sut).is_empty(), "the ghost never repaints");
    sut.resolve(Res::SyncCompleted { new_count: 0 });

    // The write settles; post-delete reloads stay clean.
    assert!(sut
        .resolve(Res::DeleteCommitted {
            id: "s1".to_owned()
        })
        .is_empty());
    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![], T0 + 2_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert!(items(&sut).is_empty());
}

/// Ported verbatim: the TS `.finally()` drops the tombstone on FAILURE too,
/// so the next reload resurrects the row — honest, since the record really is
/// still in storage.
#[test]
fn failed_delete_lets_the_next_reload_resurrect_the_row() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    sut.dispatch(Event::DeleteRequested {
        id: "s1".to_owned(),
    });
    assert!(items(&sut).is_empty());
    sut.resolve(Res::DeleteFailed {
        id: "s1".to_owned(),
    });

    sut.dispatch(Event::FocusTick);
    sut.resolve(loaded(&sut, vec![s1], T0 + 1_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    assert_eq!(items(&sut).len(), 1, "the record still exists — show it");
}

// ---------------------------------------------------------------------------
// ⑥ — date headers interleave with items in feed order
// ---------------------------------------------------------------------------

#[test]
fn day_headers_interleave_in_feed_order() {
    // 200_000s and 190_000s share a "day"; 100_000s is an earlier one.
    let r_a = recv("a", "0xBob", "1", "USDT", 200_000.0);
    let r_b = recv("b", "0xBob", "2", "USDT", 190_000.0);
    let s_c = send("c", "0xU1", "0xCafe", "3", 100_000.0);
    let mut sut = boot(vec![s_c, r_a, r_b]); // store order ≠ feed order
    drain_aliases(&mut sut);

    let day_new = day_of(200_000.0);
    let day_old = day_of(100_000.0);
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 5);
    match &rows[0] {
        FeedRow::Header {
            id,
            day_start_ms,
            timestamp,
        } => {
            assert_eq!(id, &format!("day-{day_new}"));
            assert_eq!(*day_start_ms, day_new);
            assert_eq!(*timestamp, 200_000.0, "labelled from the day's first item");
        }
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[1], FeedRow::Item { item } if item.id == "a"));
    assert!(matches!(&rows[2], FeedRow::Item { item } if item.id == "b"));
    match &rows[3] {
        FeedRow::Header { day_start_ms, .. } => assert_eq!(*day_start_ms, day_old),
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[4], FeedRow::Item { item } if item.id == "c"));
}

#[test]
fn chain_filter_regroups_headers_over_the_filtered_list() {
    let mut r_a = recv("a", "0xBob", "1", "USDT", 200_000.0);
    r_a.chain_id = 1;
    let r_b = recv("b", "0xBob", "2", "USDT", 190_000.0); // 8453
    let mut s_c = send("c", "0xU1", "0xCafe", "3", 100_000.0);
    s_c.chain_id = 1;
    let mut sut = boot(vec![r_a, r_b, s_c]);
    drain_aliases(&mut sut);

    sut.dispatch(Event::ChainFilterChanged {
        chain_id: Some(8453),
    });
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 2, "one day, one item on 8453");
    match &rows[0] {
        FeedRow::Header { timestamp, .. } => assert_eq!(
            *timestamp, 190_000.0,
            "the header derives from the FILTERED day's first item"
        ),
        other => panic!("expected header, got {other:?}"),
    }
    assert!(matches!(&rows[1], FeedRow::Item { item } if item.id == "b"));

    // A day whose items are all filtered out emits no header at all.
    sut.dispatch(Event::ChainFilterChanged { chain_id: Some(1) });
    let rows = sut.view().rows;
    assert_eq!(rows.len(), 4); // header + a, header + c

    sut.dispatch(Event::ChainFilterChanged { chain_id: None });
    assert_eq!(sut.view().rows.len(), 5, "unfiltered again");
}

// ---------------------------------------------------------------------------
// ⑦ — alias memoisation: local name first, one network attempt ever
// ---------------------------------------------------------------------------

#[test]
fn stored_name_blocks_network_and_attempts_never_repeat() {
    let named = send("s1", "0xU1", "0xCafe", "1", 300_000.0); // to_name set
    let unnamed = recv("r1", "0xBeEf", "2", "USDT", 200_000.0);
    let unnamed_again = recv("r2", "0xbeef", "3", "USDT", 100_000.0); // same addr, other case

    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![named, unnamed, unnamed_again], T0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xbeef".to_owned()
        }],
        "one lowercased request; the stored-name row never asks"
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });

    sut.resolve(Res::AliasResolved {
        addr: "0xbeef".to_owned(),
        name: Some("Bob".to_owned()),
    });
    let rows = items(&sut);
    assert_eq!(rows[0].alias.as_deref(), Some("name-of-0xCafe"));
    assert_eq!(rows[1].alias.as_deref(), Some("Bob"));
    assert_eq!(rows[2].alias.as_deref(), Some("Bob"), "case-folded lookup");

    // A reload re-derives pending addresses — attempted ones never re-ask.
    sut.dispatch(Event::FocusTick);
    let r3 = recv("r3", "0xBEEF", "4", "USDT", 50_000.0);
    let ops = sut.resolve(loaded(
        &sut,
        vec![
            send("s1", "0xU1", "0xCafe", "1", 300_000.0),
            recv("r1", "0xBeEf", "2", "USDT", 200_000.0),
            r3,
        ],
        T0 + 1_000.0,
    ));
    assert!(
        ops.is_empty(),
        "0xbeef was attempted — never again this session"
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });
}

/// The resolved overlay wins over a stored name for display
/// (`aliasMap.get(...) ?? item.alias`, HomeScreen.tsx:244-250).
#[test]
fn resolved_name_wins_over_stored_for_display() {
    let with_stored = send("s1", "0xU1", "0xD00d", "1", 200_000.0);
    let mut bare = recv("r1", "0xd00d", "2", "USDT", 100_000.0);
    bare.symbol = "USDT".to_owned();

    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![with_stored, bare], T0));
    assert_eq!(
        ops,
        vec![Op::ResolveRecipientIdentity {
            addr: "0xd00d".to_owned()
        }]
    );
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    sut.resolve(Res::AliasResolved {
        addr: "0xd00d".to_owned(),
        name: Some("vitalik.eth".to_owned()),
    });
    let rows = items(&sut);
    assert_eq!(
        rows[0].alias.as_deref(),
        Some("vitalik.eth"),
        "the resolved name overrides the stored one"
    );
    assert_eq!(rows[1].alias.as_deref(), Some("vitalik.eth"));
}

/// An unresolved address is memoised as attempted too — `None` answers are
/// never retried (and an empty name is falsy, as in TS).
#[test]
fn unresolved_alias_never_retries() {
    let r1 = recv("r1", "0xGhost", "1", "USDT", 100_000.0);
    let mut sut = Sut::new();
    sut.dispatch(Event::AccountSwitched {
        address: ADDR.to_owned(),
    });
    let ops = sut.resolve(loaded(&sut, vec![r1.clone()], T0));
    assert_eq!(ops.len(), 1);
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    let ops = sut.resolve(Res::AliasResolved {
        addr: "0xghost".to_owned(),
        name: Some(String::new()),
    });
    assert!(
        ops.is_empty(),
        "empty name is falsy — not memoised as a name"
    );
    assert!(items(&sut)[0].alias.is_none());

    sut.dispatch(Event::FocusTick);
    let ops = sut.resolve(loaded(&sut, vec![r1], T0 + 1_000.0));
    assert!(ops.is_empty(), "no second network attempt");
    sut.resolve(Res::SyncCompleted { new_count: 0 });
}

// ---------------------------------------------------------------------------
// ⑧ — stablecoin face-value: never $0.00
// ---------------------------------------------------------------------------

#[test]
fn stablecoin_face_value_is_never_zero() {
    // No stored USD at all → face value.
    let r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    assert_eq!(tx_usd_value(&r), 45.5);

    // Stored "$0.00" (the legacy unknown marker) → face value, not zero.
    let mut r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    r.usd = Some("$0.00".to_owned());
    assert_eq!(tx_usd_value(&r), 45.5);

    // The on-chain Tether glyph folds: "USD₮0" is a stablecoin.
    let r = recv("r", "0xBob", "12", "USD₮0", 100_000.0);
    assert_eq!(tx_usd_value(&r), 12.0);

    // A real stored value wins over the face value ("$1,234.56" → 1234.56).
    let mut r = recv("r", "0xBob", "45.5", "USDT", 100_000.0);
    r.usd = Some("$1,234.56".to_owned());
    assert_eq!(tx_usd_value(&r), 1234.56);

    // Unpriced non-stablecoins honestly stay 0.
    let r = recv("r", "0xBob", "4840000", "SNDRA", 100_000.0);
    assert_eq!(tx_usd_value(&r), 0.0);

    // Empty value string → parseFloat('0') → no face value to show.
    let mut r = recv("r", "0xBob", "", "USDT", 100_000.0);
    r.usd = Some(String::new()); // falsy, like an absent field
    assert_eq!(tx_usd_value(&r), 0.0);

    assert!(is_stable("usdc.e"), "case-folded match");
    assert!(!is_stable("WETH"));
}

/// The valuation flows into the folded items (both directions).
#[test]
fn items_carry_the_stable_face_value() {
    let r1 = recv("r1", "0xBob", "45.5", "USDT", 200_000.0);
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0); // USDC, no usd stored
    let mut sut = boot(vec![r1, s1]);
    drain_aliases(&mut sut);
    let rows = items(&sut);
    assert_eq!(rows[0].usd_value, 45.5);
    assert_eq!(rows[1].usd_value, 10.0);
}

// ---------------------------------------------------------------------------
// Staleness — the addressRef guard
// ---------------------------------------------------------------------------

#[test]
fn switching_accounts_drops_in_flight_answers() {
    let s1 = send("s1", "0xU1", "0xCafe", "10", 100_000.0);
    let mut sut = boot(vec![s1.clone()]);

    // A load for the old account is in flight when the user switches.
    sut.dispatch(Event::FocusTick);
    let ops = sut.dispatch(Event::AccountSwitched {
        address: "0xNewAccount".to_owned(),
    });
    assert_eq!(
        shapes(ops),
        vec![
            Op::ReadTxStore {
                address: "0xNewAccount".to_owned(),
                read_id: 0
            },
            Op::ScanIncomingTransfers {
                address: "0xNewAccount".to_owned()
            }
        ]
    );
    let before = sut.view();
    assert_eq!(
        before.rows.len(),
        2,
        "the previous feed keeps painting until the new read commits"
    );

    // The OLD account's answers land late — and change nothing.
    let stale_extra = send("s2", "0xU2", "0xCafe", "99", 300_000.0);
    let ops = sut.resolve(loaded(&sut, vec![s1, stale_extra], T0 + 1_000.0));
    assert!(ops.is_empty());
    assert_eq!(sut.view(), before, "a stale commit never paints");
    let ops = sut.resolve(Res::SyncCompleted { new_count: 5 });
    assert!(ops.is_empty(), "a stale sync never re-reads");

    // The NEW account's read commits its own feed.
    let mut theirs = base("n1", 400_000.0);
    theirs.kind = Some(FeedTxKind::Send);
    theirs.from = "0xNewAccount".to_owned();
    theirs.to = "0xCafe".to_owned();
    theirs.to_name = Some("Ann".to_owned());
    theirs.user_op_hash = "0xN".to_owned();
    sut.resolve(loaded(&sut, vec![theirs], T0 + 2_000.0));
    sut.resolve(Res::SyncCompleted { new_count: 0 });
    let ids: Vec<String> = items(&sut).into_iter().map(|i| i.id).collect();
    assert_eq!(ids, vec!["n1"]);
}

#[test]
fn account_switch_clears_the_celebration() {
    let r1 = recv("r1", "0xBob", "5", "USDT", 100_000.0);
    let r2 = recv("r2", "0xBob", "7", "USDT", 200_000.0);
    let mut sut = celebrated(vec![r1], r2);
    assert!(sut.view().toast.is_some());

    sut.dispatch(Event::AccountSwitched {
        address: "0xNewAccount".to_owned(),
    });
    let view = sut.view();
    assert!(view.toast.is_none(), "setReceipt(null) on switch");
    assert!(view.new_item_id.is_none(), "setNewItemId(null) on switch");
}

// ---------------------------------------------------------------------------
// Inertness without an account
// ---------------------------------------------------------------------------

#[test]
fn empty_address_is_inert() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(Event::FocusTick).is_empty());
    assert!(sut.dispatch(Event::LiveTick).is_empty());
    assert!(sut
        .dispatch(Event::ReconcileCompleted { resolved_count: 3 })
        .is_empty());
    assert!(sut
        .dispatch(Event::AccountSwitched {
            address: String::new()
        })
        .is_empty());
    assert!(sut.view().rows.is_empty());
}

/// A celebration belongs to the read the sync asked for — not to whichever
/// read answers next.
///
/// A tick issues the store read and the incoming-transfer scan together. If the
/// scan answers first, the sync flags a celebration and asks for a FRESH read;
/// the earlier read is still in flight and its snapshot predates the receipt.
/// Before `read_id` correlation that stale answer consumed the flag, and the
/// real receipt landed with no toast, glow or haptic — a regression against the
/// TypeScript original, which bound the celebration to one `loadData` by
/// closure.
#[test]
fn a_stale_read_never_eats_the_celebration_the_sync_earned() {
    let old = send("tx-old", "uoh-old", "0xdead", "5", T0 / 1000.0);
    let mut sut = boot(vec![old.clone()]);

    // A tick: read + scan, both outstanding. Remember the tick's read id.
    let ops = sut.dispatch(Event::FocusTick);
    assert_eq!(shapes(ops), vec![read_op(), scan_op()]);
    let stale_read = oldest_read_id(&sut);

    // The scan answers FIRST: one receipt landed. That flags a celebration and
    // asks for a read of its own.
    sut.resolve_matching(
        |op| matches!(op, Op::ScanIncomingTransfers { .. }),
        Res::SyncCompleted { new_count: 1 },
    );
    let fresh_read = sut
        .outstanding()
        .iter()
        .filter_map(|op| match op {
            Op::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .find(|id| *id != stale_read)
        .expect("the sync asked for a read of its own");

    // The stale read finally answers — with the pre-receipt snapshot.
    sut.resolve_matching(
        |op| matches!(op, Op::ReadTxStore { read_id, .. } if *read_id == stale_read),
        loaded_for(stale_read, vec![old.clone()], T0),
    );
    drain_aliases(&mut sut);
    assert!(
        sut.view().toast.is_none(),
        "a snapshot that predates the receipt must not celebrate"
    );

    // The read the sync asked for arrives, carrying the receipt. THIS one
    // celebrates.
    let landed = recv("tx-in", "0xbeef", "9", "USDC", T0 / 1000.0 + 1.0);
    sut.resolve_matching(
        |op| matches!(op, Op::ReadTxStore { read_id, .. } if *read_id == fresh_read),
        loaded_for(fresh_read, vec![landed, old], T0),
    );
    drain_aliases(&mut sut);
    assert!(
        sut.view().toast.is_some(),
        "the receipt's own read must celebrate"
    );
}

// ---------------------------------------------------------------------------
// Spec 082 round 2 — what a dApp record's detail says (RJ16, G52)
// ---------------------------------------------------------------------------

/// The founder's address, as the DX-W3 run sent to it.
const FOUNDER: &str = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141";
/// USDC on Gnosis — the contract the DX-W3 call went to.
const GNOSIS_USDC: &str = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83";

/// `transfer(FOUNDER, 10^30)` — exactly 4 + 32 + 32 bytes.
fn transfer_to_founder() -> String {
    format!(
        "0xa9059cbb{:0>64}{:0>64}",
        FOUNDER.trim_start_matches("0x").to_lowercase(),
        "c9f2c9cd04674edea40000000"
    )
}

/// DX-W3: a relay-rejected USDC transfer. The detail named the token contract
/// as 接收方 and offered the explorer for an op that never reached the chain.
/// The recipient is the transfer's, checksummed; there is no tx hash to link.
#[test]
fn a_token_transfer_names_its_recipient_and_links_no_op_hash() {
    let op = "0x4d558afa2e899ead13277162dfa5b25fa7f9b1bc809de0b9f90f7713449439d7";
    let mut rejected = dapp_tx(
        "dapp-w3-tx",
        "http://127.0.0.1:8137",
        GNOSIS_USDC,
        "0x0",
        100_000.0,
    );
    rejected.call_data = Some(transfer_to_founder());
    rejected.status = FeedTxStatus::Failed;
    rejected.user_op_hash = op.to_owned();
    rejected.tx_hash = op.to_uppercase().replace("0X", "0x");
    rejected.to_name = None;
    let sut = boot(vec![rejected]);

    let rows = items(&sut);
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.counterparty.as_deref(), Some(FOUNDER));
    assert_eq!(row.counterparty_role, FeedCounterpartyRole::Recipient);
    assert_eq!(row.tx_hash, None, "an op hash is never an explorer link");
    assert_eq!(row.status, FeedTxStatus::Failed);
}

/// Any other call goes to a contract: the counterparty is the contract, said
/// as the contract ("interacting with"), not as a recipient.
#[test]
fn a_swap_call_names_the_contract() {
    let mut swap = dapp_tx(
        "dapp-s-tx",
        "https://app.uniswap.org",
        "0xRouter",
        "0x0",
        100_000.0,
    );
    swap.call_data = Some(format!("0x3593564c{}", "00".repeat(96)));
    let sut = boot(vec![swap]);
    let row = &items(&sut)[0];
    assert_eq!(row.counterparty.as_deref(), Some("0xRouter"));
    assert_eq!(row.counterparty_role, FeedCounterpartyRole::Contract);
    assert_eq!(
        row.tx_hash.as_deref(),
        Some("0xtxdapp-s-tx"),
        "a real tx hash stays"
    );

    // A transfer selector with the wrong length is not a transfer.
    let mut odd = dapp_tx("dapp-o-tx", "https://x.test", "0xToken", "0x0", 100_000.0);
    odd.call_data = Some(format!("{}00", transfer_to_founder()));
    let sut = boot(vec![odd]);
    let row = &items(&sut)[0];
    assert_eq!(row.counterparty.as_deref(), Some("0xToken"));
    assert_eq!(row.counterparty_role, FeedCounterpartyRole::Contract);
}

/// No calldata (a plain native send from a page, or a shell that does not
/// map it yet): `to`, as a recipient — today's row, unchanged.
#[test]
fn a_plain_send_is_unchanged() {
    let plain = dapp_tx(
        "dapp-p-tx",
        "https://x.test",
        "0xFriend",
        "0x2386f26fc10000",
        100_000.0,
    );
    let sut = boot(vec![plain]);
    let row = &items(&sut)[0];
    assert_eq!(row.counterparty.as_deref(), Some("0xFriend"));
    assert_eq!(row.counterparty_role, FeedCounterpartyRole::Recipient);
    assert_eq!(row.value.as_deref(), Some("0.01"));

    let mut empty = dapp_tx("dapp-e-tx", "https://x.test", "0xFriend", "0x0", 100_000.0);
    empty.call_data = Some("0x".to_owned());
    let sut = boot(vec![empty]);
    assert_eq!(
        items(&sut)[0].counterparty_role,
        FeedCounterpartyRole::Recipient,
        "empty calldata is no call"
    );
}

/// The wire: both fields default, so a record or row from before round 2
/// still reads.
#[test]
fn the_new_fields_default_on_the_wire() {
    let record = serde_json::to_value(base("x", 1.0)).unwrap_or_default();
    let mut old = record.clone();
    if let Some(map) = old.as_object_mut() {
        map.remove("call_data");
    }
    let back: FeedTxRecord = serde_json::from_value(old).expect("an old record reads");
    assert_eq!(back.call_data, None);
    assert_eq!(
        serde_json::to_value(FeedCounterpartyRole::Contract).unwrap_or_default(),
        serde_json::json!("contract")
    );
    assert_eq!(
        FeedCounterpartyRole::default(),
        FeedCounterpartyRole::Recipient
    );
}
