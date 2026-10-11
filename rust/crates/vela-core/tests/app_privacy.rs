//! Balance privacy (spec 016 invariant ⑧, `app::privacy`): what the core
//! withholds while hidden, and the shared fixture every shell replays.
//!
//! `tests/fixtures/privacy-hidden.json` is a REAL balance and feed — driven
//! through the two machines below, not written by hand — once shown and once
//! hidden, with figures whose digits appear nowhere else (addresses are
//! letters, hashes repeat `ab`). Every shell feeds both through every surface
//! builder it has and asserts:
//!
//! 1. shown: each `forbidden` digit run appears somewhere (the test is not
//!    vacuous);
//! 2. hidden: no output of a `masked_surfaces` builder contains any of them,
//!    and every masked figure draws as `mask` (`balance_mask` on the hero);
//! 3. hidden: the `visible_surfaces` (Send, the signing sheet, Receive) still
//!    draw their figures;
//! 4. each feed row's figure masks exactly when `figure_maskable` says so.
//!
//! Regenerate after a deliberate change with
//! `VELA_WRITE_FIXTURES=1 cargo test -p vela-core --features crux --test app_privacy`
//! and review the diff: every shell's privacy test reads this file.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::activity_feed::{
    ActivityFeed, Event as FeedEvent, FeedOperation as FeedOp, FeedRow, FeedShellResult as FeedRes,
    FeedTxKind, FeedTxRecord, FeedTxStatus, FeedView,
};
use vela_core::app::balance_dashboard::{
    BalanceCacheEntry, BalanceDashboard, BalanceShellResult as Res, BalanceToken, BalanceView,
    Event,
};
use vela_core::app::dapp_activity::{DappAction, DappSummary};
use vela_core::app::privacy::{figure_maskable, MoneySurface, BALANCE_MASK, MASK};
use vela_core::app::token_trust::TrustSimJudgment;

const ADDR_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ADDR_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const PAYER: &str = "0xcccccccccccccccccccccccccccccccccccccccc";
const PAYEE: &str = "0xdddddddddddddddddddddddddddddddddddddddd";
const SPENDER: &str = "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
/// The split's second recipient (Bea, `PAYEE`, is its first).
const SECOND: &str = "0xfafafafafafafafafafafafafafafafafafafafa";
const USDC_MAINNET: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const NOW: f64 = 1_700_000_000_000.0;
const T0: f64 = 1_700_000_000_000.0;

/// The digit runs of every figure in the fixture. None of them occurs in an
/// address, a hash, an id, a chain id or a date of the fixture.
const FORBIDDEN: [&str; 13] = [
    "376", // USDC held: 376.54321, worth $376.54
    "418", // xDAI held: 418.25, worth $418.25
    "157", // BNB on the unreachable network: last seen worth $157
    "951", // the total: $951.79
    "263", // the other account's switcher row: $263.17
    "289", // received: 289.5 USDT (the toast's figure too)
    "163", // sent: 163.25 USDC
    "237", // a swap's outflow: 237.5 USDC
    "352", // a capped permit: 352 USDC
    "128", // the swap's expected return: 128.75 xDAI
    "214", // a split's first share: 214.5 USDC, to Bea
    "469", // its second share: 469.25 USDC
    "683", // the split's total: 683.75 USDC
];

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/privacy-hidden.json")
}

// ---------------------------------------------------------------------------
// The balance
// ---------------------------------------------------------------------------

fn token(
    chain_id: u32,
    symbol: &str,
    balance: &str,
    price: f64,
    contract: Option<&str>,
) -> BalanceToken {
    BalanceToken {
        chain_id,
        symbol: symbol.to_owned(),
        name: symbol.to_owned(),
        balance: balance.to_owned(),
        decimals: if contract.is_some() { 6 } else { 18 },
        token_address: contract.map(str::to_owned),
        price_usd: Some(price),
        spam: false,
    }
}

fn held() -> Vec<BalanceToken> {
    vec![
        token(1, "USDC", "376.54321", 1.0, Some(USDC_MAINNET)),
        token(100, "xDAI", "418.25", 1.0, None),
    ]
}

/// Account A: read once holding BNB on BNB Chain too, then that network went
/// quiet; the switcher was opened over A and B. `hidden` toggles privacy last.
fn balance(hidden: bool) -> BalanceView {
    let mut sut: DomainDriver<BalanceDashboard> = DomainDriver::new();
    sut.dispatch(Event::AccountChanged {
        address: ADDR_A.to_owned(),
    });
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: None,
    });
    let mut first = held();
    first.push(token(56, "BNB", "0.5", 314.0, None));
    sut.resolve(Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: first,
        failed_chain_ids: vec![],
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![1, 100, 56],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW,
    });
    sut.resolve(Res::BalanceCacheWritten);
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: held(),
        failed_chain_ids: vec![56],
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![1, 100, 56],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW + 1_000.0,
    });
    sut.dispatch(Event::SwitcherOpened {
        addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()],
    });
    sut.resolve(Res::BalanceCacheWritten);
    sut.resolve(Res::CachedBalancesLoaded {
        balances: vec![BalanceCacheEntry {
            address: ADDR_B.to_owned(),
            usd: 263.17,
        }],
    });
    // Both refreshes fail (best effort): every row keeps its cached figure.
    for address in [ADDR_A, ADDR_B] {
        sut.resolve(Res::AccountAssetsFetched {
            address: address.to_owned(),
            tokens: None,
        });
    }
    if hidden {
        sut.dispatch(Event::PrivacyToggled);
    }
    sut.view()
}

// ---------------------------------------------------------------------------
// The feed
// ---------------------------------------------------------------------------

/// Each record its own transaction (letters only, so no digit run hides in
/// a hash): records sharing one fold into one row.
fn record(id: &str, kind: FeedTxKind, ts: f64) -> FeedTxRecord {
    let pair = match id {
        "sent" => "ab",
        "swap" => "ac",
        "received" => "ad",
        "split-a" | "split-b" => "af",
        _ => "ae",
    };
    FeedTxRecord {
        id: id.to_owned(),
        user_op_hash: String::new(),
        tx_hash: format!("0x{}", pair.repeat(32)),
        from: ADDR_A.to_owned(),
        to: PAYEE.to_owned(),
        to_name: Some("Bea".to_owned()),
        value: "0".to_owned(),
        symbol: "USDC".to_owned(),
        decimals: 6,
        logo_urls: None,
        chain_id: 1,
        timestamp: ts,
        day_start_ms: (ts / 86_400.0).floor() * 86_400_000.0,
        status: FeedTxStatus::Confirmed,
        kind: Some(kind),
        usd: None,
        dapp_url: None,
        intent: None,
        balance_changes: None,
        calldata: None,
        call_data: None,
        summary: None,
        settlement: None,
        // Not on the wire when absent, so the fixture the shells share
        // (`privacy-hidden.json`) is byte for byte what it was.
        time_verified: None,
    }
}

fn records() -> Vec<FeedTxRecord> {
    let ts = T0 / 1000.0 - 86_400.0;
    let mut sent = record("sent", FeedTxKind::Send, ts);
    sent.value = "163.25".to_owned();
    sent.usd = Some("163.25".to_owned());

    let mut swap = record("swap", FeedTxKind::DappTx, ts - 600.0);
    swap.to = SPENDER.to_owned();
    swap.to_name = None;
    swap.value = "0x0".to_owned();
    swap.symbol = "xDAI".to_owned();
    swap.decimals = 18;
    swap.chain_id = 100;
    swap.user_op_hash = format!("0x{}", "cd".repeat(32));
    swap.dapp_url = Some("https://swap.example".to_owned());
    swap.calldata = Some(true);
    swap.balance_changes = Some(vec![
        TrustSimJudgment::Erc20Trusted {
            token: "0x2a22f9c3b484c3629090feed35f17ff8f88f76f0".to_owned(),
            delta: "-237500000".to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 6,
            in_trusted_set: true,
        },
        TrustSimJudgment::Native {
            delta: "128750000000000000000".to_owned(),
        },
    ]);

    let mut capped = record("permit", FeedTxKind::SignTypedData, ts - 1_200.0);
    capped.to = String::new();
    capped.to_name = None;
    capped.symbol = String::new();
    capped.decimals = 0;
    capped.tx_hash = String::new();
    capped.dapp_url = Some("https://swap.example".to_owned());
    capped.summary = Some(DappSummary {
        action: DappAction::Permit,
        contract: Some(USDC_MAINNET.to_owned()),
        spender: Some(SPENDER.to_owned()),
        token: Some(USDC_MAINNET.to_owned()),
        symbol: Some("USDC".to_owned()),
        decimals: Some(6),
        amount: Some("352000000".to_owned()),
        primary_type: Some("Permit".to_owned()),
        ..DappSummary::default()
    });

    let mut unlimited = capped.clone();
    unlimited.id = "permit-unlimited".to_owned();
    unlimited.timestamp = ts - 1_800.0;
    unlimited.summary = Some(DappSummary {
        amount: None,
        unlimited: true,
        ..capped.summary.clone().unwrap_or_default()
    });

    let mut signature = record("signature", FeedTxKind::SignMessage, ts - 2_400.0);
    signature.to = String::new();
    signature.to_name = None;
    signature.symbol = String::new();
    signature.tx_hash = String::new();
    signature.dapp_url = Some("https://swap.example".to_owned());

    // One send to two people (a split): two records sharing an operation
    // fold into one row. Its total is one figure and each recipient's share
    // is another — a hidden split that masked the total and still listed who
    // got how much leaked exactly what the mask was for (the web did).
    let mut share_a = record("split-a", FeedTxKind::Send, ts - 300.0);
    share_a.user_op_hash = format!("0x{}", "ef".repeat(32));
    share_a.value = "214.5".to_owned();
    share_a.usd = Some("214.5".to_owned());
    let mut share_b = share_a.clone();
    share_b.id = "split-b".to_owned();
    share_b.to = SECOND.to_owned();
    share_b.to_name = None;
    share_b.value = "469.25".to_owned();
    share_b.usd = Some("469.25".to_owned());

    vec![sent, share_a, share_b, swap, capped, unlimited, signature]
}

fn received() -> FeedTxRecord {
    let mut r = record("received", FeedTxKind::Receive, T0 / 1000.0 - 60.0);
    r.from = PAYER.to_owned();
    r.to = ADDR_A.to_owned();
    r.to_name = None;
    r.value = "289.5".to_owned();
    r.symbol = "USDT".to_owned();
    r.usd = Some("289.5".to_owned());
    r
}

/// Booted with the history, then a payment arrives on the next pass (the
/// toast), and Bea's page is open.
fn feed(hidden: bool) -> FeedView {
    let mut sut: DomainDriver<ActivityFeed> = DomainDriver::new();
    sut.dispatch(FeedEvent::AccountSwitched {
        address: ADDR_A.to_owned(),
    });
    let read_id = |sut: &DomainDriver<ActivityFeed>| {
        sut.outstanding()
            .into_iter()
            .find_map(|op| match op {
                FeedOp::ReadTxStore { read_id, .. } => Some(read_id),
                _ => None,
            })
            .unwrap_or_default()
    };
    let id = read_id(&sut);
    sut.resolve(FeedRes::StoreLoaded {
        records: records(),
        now_ms: T0,
        read_id: id,
    });
    sut.resolve(FeedRes::SyncCompleted { new_count: 0 });
    drain(&mut sut);
    sut.dispatch(FeedEvent::FocusTick);
    let id = read_id(&sut);
    sut.resolve(FeedRes::StoreLoaded {
        records: records(),
        now_ms: T0 + 1_000.0,
        read_id: id,
    });
    sut.resolve(FeedRes::SyncCompleted { new_count: 1 });
    let id = read_id(&sut);
    let mut all = vec![received()];
    all.extend(records());
    sut.resolve(FeedRes::StoreLoaded {
        records: all,
        now_ms: T0 + 2_000.0,
        read_id: id,
    });
    drain(&mut sut);
    sut.dispatch(FeedEvent::ContactFilterChanged {
        address: Some(PAYEE.to_owned()),
    });
    if hidden {
        sut.dispatch(FeedEvent::PrivacyChanged { hidden: true });
    }
    sut.view()
}

/// Answer the haptic, the timer and any alias lookups, so nothing is pending.
fn drain(sut: &mut DomainDriver<ActivityFeed>) {
    while let Some(op) = sut.outstanding().first().cloned() {
        match op {
            FeedOp::ResolveRecipientIdentity { addr } => {
                sut.resolve(FeedRes::AliasResolved { addr, name: None });
            }
            FeedOp::Haptic => {
                sut.resolve(FeedRes::HapticPlayed);
            }
            _ => break,
        }
    }
}

// ---------------------------------------------------------------------------
// The fixture
// ---------------------------------------------------------------------------

fn surfaces(list: &[MoneySurface]) -> Value {
    serde_json::to_value(list).unwrap_or_default()
}

fn fixture() -> Value {
    let feed_hidden = feed(true);
    let figure_maskable: serde_json::Map<String, Value> = feed_hidden
        .rows
        .iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some((item.id.clone(), Value::Bool(item.figure_maskable))),
            FeedRow::Header { .. } => None,
        })
        .collect();
    json!({
        "suite": "privacy-hidden",
        "about": "A real balance and feed, driven through vela-core's balance_dashboard and activity_feed (tests/app_privacy.rs), shown and hidden. Replay both through every surface builder: shown, every `forbidden` run appears; hidden, no masked surface's output contains one and every masked figure is `mask` (`balance_mask` on the home total); the visible surfaces keep their figures; a row's figure masks exactly when `figure_maskable` says so. The feed carries a SPLIT (one send to two people, the row whose `batch` is set): its total and each recipient's share are figures too, in the row and in its detail. Regenerate: VELA_WRITE_FIXTURES=1 cargo test -p vela-core --features crux --test app_privacy",
        "mask": MASK,
        "balance_mask": BALANCE_MASK,
        "masked_surfaces": surfaces(&MoneySurface::MASKED),
        "visible_surfaces": surfaces(&MoneySurface::VISIBLE),
        "forbidden": FORBIDDEN,
        "figure_maskable": figure_maskable,
        "shown": { "balance": balance(false), "feed": feed(false) },
        "hidden": { "balance": balance(true), "feed": feed_hidden },
    })
}

fn pretty(value: &Value) -> String {
    let mut out = serde_json::to_string_pretty(value).unwrap_or_default();
    out.push('\n');
    out
}

/// The committed fixture is what the core produces today.
#[test]
fn the_shared_fixture_is_current() {
    let want = pretty(&fixture());
    if std::env::var_os("VELA_WRITE_FIXTURES").is_some() {
        std::fs::write(fixture_path(), &want).unwrap();
    }
    let have = std::fs::read_to_string(fixture_path()).unwrap_or_default();
    assert!(
        have == want,
        "tests/fixtures/privacy-hidden.json is stale — regenerate with \
         VELA_WRITE_FIXTURES=1 cargo test -p vela-core --features crux --test app_privacy"
    );
}

/// Not vacuous: shown, every forbidden run is in the views the shells draw.
#[test]
fn every_forbidden_figure_is_there_when_shown() {
    let shown = format!(
        "{}{}",
        serde_json::to_string(&balance(false)).unwrap_or_default(),
        serde_json::to_string(&feed(false)).unwrap_or_default()
    );
    for run in FORBIDDEN {
        assert!(shown.contains(run), "{run} is drawn nowhere when shown");
    }
}

/// Hidden, the core itself sends none of them where it withholds: the
/// total, the cached total, the switcher, the unreachable list, the toast.
#[test]
fn hidden_the_core_withholds_every_total_and_the_toast() {
    let view = balance(true);
    assert!(view.hidden && view.switcher.hidden && view.switcher.open);
    assert_eq!(view.display_total_usd, None);
    assert_eq!(view.cached_total_usd, None);
    assert!(view.switcher.balances.is_empty());
    assert!(view
        .unreachable_networks
        .iter()
        .all(|row| row.last_seen_usd.is_none()));
    let withheld = serde_json::to_string(&(
        view.display_total_usd,
        view.cached_total_usd,
        &view.switcher,
        &view.unreachable_networks,
    ))
    .unwrap_or_default();
    for run in FORBIDDEN {
        assert!(
            !withheld.contains(run),
            "{run} leaves the core while hidden"
        );
    }
    let feed = feed(true);
    assert!(feed.hidden);
    assert!(feed.toast.is_none());
    // …and shown, those same places do carry the figures.
    let shown = balance(false);
    assert!(shown.cached_total_usd.is_some());
    assert!(!shown.switcher.balances.is_empty());
    assert!(feed_with_toast());
}

fn feed_with_toast() -> bool {
    feed(false).toast.is_some()
}

/// Each row's `figure_maskable` is the rule's, and covers every kind of row
/// the fixture has.
#[test]
fn the_fixture_covers_every_kind_of_row_figure() {
    let feed = feed(true);
    let mut kinds = Vec::new();
    for row in &feed.rows {
        if let FeedRow::Item { item } = row {
            assert_eq!(item.figure_maskable, figure_maskable(item), "{}", item.id);
            kinds.push((item.id.as_str().to_owned(), item.figure_maskable));
        }
    }
    let ids: Vec<&str> = kinds.iter().map(|(id, _)| id.as_str()).collect();
    for id in [
        "received",
        "sent",
        "swap",
        "permit",
        "permit-unlimited",
        "signature",
    ] {
        assert!(ids.contains(&id), "{id} is missing from {ids:?}");
    }
    let maskable = |id: &str| kinds.iter().find(|(k, _)| k == id).map(|(_, m)| *m);
    assert_eq!(
        maskable("permit"),
        Some(true),
        "a capped allowance is money"
    );
    assert_eq!(maskable("permit-unlimited"), Some(false), "a risk to see");
    assert_eq!(maskable("signature"), Some(false), "no figure to hide");
    assert!(!feed.contact_rows.is_empty(), "Bea's page has rows");

    // The split: one row, masked, with each recipient's share inside it —
    // on the feed and on the home's cut. Every shell's fixture test has to
    // walk the row AND its detail, where the shares are listed. (A contact's
    // page lists what has that one counterparty; a split has none.)
    let split = |rows: &[FeedRow]| {
        rows.iter().find_map(|row| match row {
            FeedRow::Item { item } => item.batch.clone().map(|batch| (item.clone(), batch)),
            FeedRow::Header { .. } => None,
        })
    };
    let (item, batch) = split(&feed.rows).unwrap_or_else(|| unreachable!("a split row"));
    assert!(item.figure_maskable, "a split's total is money");
    assert_eq!(item.value.as_deref(), Some("683.75"));
    assert_eq!(batch.count, 2);
    let shares: Vec<(&str, &str)> = batch
        .transfers
        .iter()
        .map(|line| (line.to.as_str(), line.value.as_str()))
        .collect();
    assert_eq!(shares, [(PAYEE, "214.5"), (SECOND, "469.25")]);
    assert!(split(&feed.home_rows).is_some(), "the home draws it too");
}

/// No figure of the fixture hides in something that is not a figure: an
/// address, a hash, an id, a timestamp. A shell's "hidden: no forbidden run
/// in the output" would fail on a row that leaked nothing.
#[test]
fn no_forbidden_run_hides_outside_a_figure() {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(map) => {
                for key in [
                    "value",
                    "usd_value",
                    "balance",
                    "total_usd",
                    "display_total_usd",
                    "cached_total_usd",
                    "last_seen_usd",
                    "usd",
                    "amount",
                    "price_usd",
                    "balances",
                    "toast",
                    "subtitle",
                    "dapp",
                    "transactions",
                ] {
                    map.remove(key);
                }
                for child in map.values_mut() {
                    strip(child);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut rest = json!({ "balance": balance(false), "feed": feed(false) });
    strip(&mut rest);
    let rest = rest.to_string();
    for run in FORBIDDEN {
        assert!(
            !rest.contains(run),
            "{run} also appears outside a figure — pick another number"
        );
    }
}

/// The surface table is total: every surface is either masked or visible.
#[test]
fn every_surface_is_masked_or_visible_never_both() {
    for surface in MoneySurface::MASKED {
        assert!(surface.masks(true) && !surface.masks(false), "{surface:?}");
    }
    for surface in MoneySurface::VISIBLE {
        assert!(!surface.masks(true), "{surface:?}");
        assert!(!MoneySurface::MASKED.contains(&surface));
    }
}
