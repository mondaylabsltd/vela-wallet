//! Spec 099 — the layers of a dApp tab, as the browser machine records them:
//! every request a row naming the layer and reason that ended it, the page
//! and provider states, busy tabs, the read deadline, one log line per end,
//! and the inspector's report. One test per rule.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation as Op, DbrShellResult as Res, DbrStoredSite, DbrTabView, Event,
};
use vela_core::app::dapp_permissions::DpermGrant;
use vela_core::app::dapp_record::{
    DbrLayer, DbrOutcome, DbrPageState, DbrProviderState, DbrReadFailure, DbrReason,
    DbrRequestClass, READ_DEADLINE_MS, REQUEST_RECORD_CAP,
};
use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

type Sut = DomainDriver<DappBrowser>;

const T0: f64 = 1_754_700_000_000.0;
const DAPP: &str = "https://dapp.example";
const A1: &str = "0x1111111111111111111111111111111111111111";

fn booted(connected: bool) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 100, 8453],
    });
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(vec![A1.to_owned()]),
    });
    sut.dispatch(Event::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
    });
    let sites = if connected {
        vec![DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(DpermGrant {
                origin: DAPP.to_owned(),
                address: A1.to_owned(),
                chain_id: 100,
                granted_at_ms: T0,
            }),
            chain_id: None,
        }]
    } else {
        Vec::new()
    };
    sut.resolve(Res::SitesListed { sites });
    sut
}

/// A tab showing DAPP, its document `d1` said hello, its load finished.
fn on_page(connected: bool) -> Sut {
    let mut sut = booted(connected);
    sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: format!("{DAPP}/"),
        now_ms: T0,
    });
    sut.dispatch(page(json!({"t":"hello","doc":"d1"}), T0 + 10.0));
    sut.dispatch(Event::LoadFinished {
        tab: "t1".to_owned(),
        url: format!("{DAPP}/"),
        now_ms: T0 + 20.0,
    });
    sut
}

fn page(message: Value, now_ms: f64) -> Event {
    Event::PageMessage {
        tab: "t1".to_owned(),
        frame_origin: DAPP.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
        now_ms,
    }
}

fn ask(sut: &mut Sut, id: &str, method: &str, params: Value, now_ms: f64) -> Vec<Op> {
    sut.dispatch(page(
        json!({"t":"req","doc":"d1","id":id,"method":method,"params":params}),
        now_ms,
    ))
}

fn tab(sut: &Sut) -> DbrTabView {
    sut.view()
        .tabs
        .into_iter()
        .find(|tab| tab.tab == "t1")
        .expect("tab t1")
}

fn logs(ops: &[Op]) -> Vec<String> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Log { line } => Some(line.clone()),
            _ => None,
        })
        .collect()
}

fn inspected(sut: &mut Sut) -> vela_core::app::dapp_record::DbrInspectorView {
    sut.dispatch(Event::InspectorOpened {
        tab: "t1".to_owned(),
    });
    sut.view().inspector.expect("the inspected tab's record")
}

fn answer_read(
    sut: &mut Sut,
    body: Option<&str>,
    failure: Option<DbrReadFailure>,
    now: f64,
) -> Vec<Op> {
    sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered {
            body_json: body.map(str::to_owned),
            now_ms: now,
            failure,
        },
    )
}

/// A request the wallet answers itself: a row, class local, answered, timed.
#[test]
fn a_local_answer_is_a_timed_row() {
    let mut sut = on_page(false);
    let ops = ask(&mut sut, "1", "eth_chainId", json!([]), T0 + 100.0);
    let lines = logs(&ops);
    assert_eq!(lines.len(), 1, "one log line for the request's end");
    assert!(
        lines[0].starts_with("dapp tab=t1 req=1 method=eth_chainId class=local outcome=answered")
    );

    let record = inspected(&mut sut);
    let row = record.rows.last().unwrap();
    assert_eq!(row.class, DbrRequestClass::Local);
    assert_eq!(row.outcome, DbrOutcome::Answered);
    assert_eq!(row.started_ms, T0 + 100.0);
    assert_eq!(row.ended_ms, Some(T0 + 100.0));
    assert_eq!(row.layer, None);
}

/// Every read is sent with the deadline, and one that times out says so —
/// network layer, `timed_out`, an issue for the status entry.
#[test]
fn a_read_carries_the_deadline_and_a_timeout_names_the_network() {
    let mut sut = on_page(false);
    let ops = ask(&mut sut, "1", "eth_blockNumber", json!([]), T0 + 100.0);
    let deadline = ops.iter().find_map(|op| match op {
        Op::Read { deadline_ms, .. } => Some(*deadline_ms),
        _ => None,
    });
    assert_eq!(deadline, Some(READ_DEADLINE_MS));
    assert!(tab(&sut).busy, "a read in flight keeps the tab busy");
    assert_eq!(tab(&sut).open_requests, 1);

    let ops = answer_read(
        &mut sut,
        None,
        Some(DbrReadFailure::TimedOut),
        T0 + 30_100.0,
    );
    assert!(logs(&ops)[0].contains("layer=network reason=timed_out ms=30000"));
    let view = tab(&sut);
    assert!(!view.busy);
    assert_eq!(view.failed_recent, 1);
    let note = view.last_failure.expect("the status entry's line");
    assert_eq!(
        (note.layer, note.reason),
        (DbrLayer::Network, DbrReason::TimedOut)
    );
    assert_eq!(note.method, "eth_blockNumber");
}

/// No endpoint, a rate limit, an endpoint's own error: each is named.
#[test]
fn read_failures_are_told_apart() {
    let mut sut = on_page(false);
    ask(&mut sut, "1", "eth_call", json!([{}, "latest"]), T0);
    answer_read(&mut sut, None, None, T0 + 5.0);
    ask(&mut sut, "2", "eth_call", json!([{}, "latest"]), T0 + 10.0);
    answer_read(
        &mut sut,
        Some(r#"{"error":{"code":429,"message":"slow down"}}"#),
        None,
        T0 + 15.0,
    );
    ask(&mut sut, "3", "eth_call", json!([{}, "latest"]), T0 + 20.0);
    answer_read(
        &mut sut,
        Some(r#"{"error":{"code":3,"message":"execution reverted"}}"#),
        None,
        T0 + 25.0,
    );
    let rows = inspected(&mut sut).rows;
    let why: Vec<_> = rows
        .iter()
        .map(|row| (row.layer, row.reason, row.code))
        .collect();
    assert_eq!(
        why,
        vec![
            (
                Some(DbrLayer::Network),
                Some(DbrReason::NoEndpoint),
                Some(-32603)
            ),
            (
                Some(DbrLayer::Network),
                Some(DbrReason::RateLimited),
                Some(429)
            ),
            (
                Some(DbrLayer::Network),
                Some(DbrReason::EndpointError),
                Some(3)
            ),
        ]
    );
}

/// A bundler read is the relay's, and fails as the relay.
#[test]
fn a_relay_read_fails_as_the_relay() {
    let mut sut = on_page(false);
    ask(
        &mut sut,
        "1",
        "eth_getUserOperationReceipt",
        json!(["0xabc"]),
        T0,
    );
    answer_read(&mut sut, None, None, T0 + 5.0);
    let row = inspected(&mut sut).rows.pop().unwrap();
    assert_eq!(row.class, DbrRequestClass::RelayRead);
    assert_eq!(row.layer, Some(DbrLayer::Relay));
}

/// The wallet's own refusals name the wallet and the rule.
#[test]
fn the_wallets_refusals_name_the_rule() {
    let mut sut = on_page(false);
    ask(&mut sut, "1", "eth_sign", json!([A1, "0x00"]), T0);
    ask(&mut sut, "2", "personal_sign", json!(["0x00", A1]), T0);
    ask(
        &mut sut,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x1234"}]),
        T0,
    );
    let rows = inspected(&mut sut).rows;
    let why: Vec<_> = rows
        .iter()
        .map(|row| (row.layer, row.reason, row.code))
        .collect();
    assert_eq!(
        why,
        vec![
            (
                Some(DbrLayer::Wallet),
                Some(DbrReason::UnsupportedMethod),
                Some(4200)
            ),
            (
                Some(DbrLayer::Wallet),
                Some(DbrReason::NotConnected),
                Some(4100)
            ),
            (
                Some(DbrLayer::Wallet),
                Some(DbrReason::UnknownChain),
                Some(4902)
            ),
        ]
    );
}

fn signed(sut: &mut Sut, id: &str, at: f64, payload: SignResponsePayload) -> Vec<Op> {
    ask(sut, id, "personal_sign", json!(["0x00", A1]), at);
    sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: id.to_owned(),
        payload,
        user_op_hash: None,
        now_ms: at + 1_000.0,
    })
}

fn err(kind: SignErrorKind, message: Option<&str>) -> SignResponsePayload {
    SignResponsePayload::Err {
        code: if kind == SignErrorKind::UserRejected {
            4001
        } else {
            -32603
        },
        kind,
        message: message.map(str::to_owned),
    }
}

/// A signing request is busy until answered; how it ended names the sheet,
/// the relay or the signer — and a person's "no" is not an issue.
#[test]
fn signing_ends_name_sheet_relay_and_signer() {
    let mut sut = on_page(true);
    ask(&mut sut, "1", "personal_sign", json!(["0x00", A1]), T0);
    assert!(tab(&sut).busy, "a signing request keeps the tab busy");
    sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: err(SignErrorKind::UserRejected, None),
        user_op_hash: None,
        now_ms: T0 + 1_000.0,
    });
    assert!(!tab(&sut).busy);
    assert_eq!(tab(&sut).failed_recent, 0, "a person's no is not an issue");

    signed(
        &mut sut,
        "2",
        T0 + 10_000.0,
        err(
            SignErrorKind::SubmitFailed,
            Some(vela_core::user_op::REFUSED_DAPP_DETAIL),
        ),
    );
    signed(
        &mut sut,
        "3",
        T0 + 20_000.0,
        err(SignErrorKind::SignerUnavailable, Some("no app id")),
    );
    signed(
        &mut sut,
        "4",
        T0 + 30_000.0,
        SignResponsePayload::Ok {
            result: Some("0xsig".into()),
        },
    );

    let rows = inspected(&mut sut).rows;
    let why: Vec<_> = rows
        .iter()
        .map(|row| (row.outcome, row.layer, row.reason))
        .collect();
    assert_eq!(
        why,
        vec![
            (
                DbrOutcome::Failed,
                Some(DbrLayer::Sheet),
                Some(DbrReason::RejectedByPerson)
            ),
            (
                DbrOutcome::Failed,
                Some(DbrLayer::Relay),
                Some(DbrReason::RelayRefused)
            ),
            (
                DbrOutcome::Failed,
                Some(DbrLayer::Signer),
                Some(DbrReason::SignerUnavailable)
            ),
            (DbrOutcome::Answered, None, None),
        ]
    );
    assert_eq!(tab(&sut).failed_recent, 2);
    assert_eq!(rows[3].duration_ms(), Some(1_000.0));
}

/// The page leaving ends its open requests as the browser's, 4900 — a new
/// document is "navigated away" (not an issue), a crash is an issue.
#[test]
fn a_page_that_goes_ends_its_rows_as_the_browser() {
    let mut sut = on_page(false);
    ask(&mut sut, "1", "eth_blockNumber", json!([]), T0);
    let ops = sut.dispatch(page(json!({"t":"hello","doc":"d2"}), T0 + 50.0));
    assert!(
        logs(&ops)
            .iter()
            .any(|line| line.contains("req=1")
                && line.contains("layer=browser reason=navigated_away"))
    );
    assert_eq!(tab(&sut).failed_recent, 0);

    sut.dispatch(page(
        json!({"t":"req","doc":"d2","id":"9","method":"eth_blockNumber","params":[]}),
        T0 + 60.0,
    ));
    sut.dispatch(Event::RendererGone {
        tab: "t1".to_owned(),
        now_ms: T0 + 70.0,
    });
    let view = tab(&sut);
    assert_eq!(view.page, DbrPageState::Crashed);
    assert_eq!(
        view.last_failure.map(|note| note.reason),
        Some(DbrReason::PageCrashed)
    );
}

/// Closing the tab logs its open requests' ends before the rows go.
#[test]
fn closing_a_tab_logs_what_it_had_open() {
    let mut sut = on_page(false);
    ask(&mut sut, "1", "eth_blockNumber", json!([]), T0);
    let ops = sut.dispatch(Event::TabClosed {
        tab: "t1".to_owned(),
        now_ms: T0 + 10.0,
    });
    assert!(logs(&ops)
        .iter()
        .any(|line| line.contains("req=1") && line.contains("reason=tab_closed")));
    assert!(sut.view().tabs.is_empty());
}

/// The provider layer: pending while loading, offered on hello, not offered
/// on an insecure origin or when a load finishes with no hello.
#[test]
fn the_provider_state_says_whether_the_wallet_is_there() {
    let mut sut = booted(false);
    sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: format!("{DAPP}/"),
        now_ms: T0,
    });
    assert_eq!(tab(&sut).provider, DbrProviderState::Pending);
    assert_eq!(tab(&sut).page, DbrPageState::Loading);
    sut.dispatch(Event::LoadFinished {
        tab: "t1".to_owned(),
        url: format!("{DAPP}/"),
        now_ms: T0 + 10.0,
    });
    assert_eq!(
        tab(&sut).provider,
        DbrProviderState::NoHello,
        "the script did not run"
    );
    assert_eq!(tab(&sut).page, DbrPageState::Ready);

    sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: format!("{DAPP}/b"),
        now_ms: T0 + 20.0,
    });
    sut.dispatch(page(json!({"t":"hello","doc":"d1"}), T0 + 30.0));
    assert_eq!(tab(&sut).provider, DbrProviderState::Offered);

    let ops = sut.dispatch(Event::NavigationStarted {
        tab: "t2".to_owned(),
        url: "http://public.example/".to_owned(),
        now_ms: T0 + 40.0,
    });
    let t2 = sut
        .view()
        .tabs
        .into_iter()
        .find(|tab| tab.tab == "t2")
        .unwrap();
    assert_eq!(t2.provider, DbrProviderState::InsecureOrigin);
    assert!(logs(&ops).iter().any(|line| line
        == "dapp tab=t2 origin=http://public.example page=loading provider=insecure_origin"));
}

/// A state is logged once, when it changes — not on every event.
#[test]
fn tab_states_are_logged_on_change_only() {
    let mut sut = on_page(false);
    let ops = ask(&mut sut, "1", "eth_chainId", json!([]), T0 + 100.0);
    assert!(
        !logs(&ops).iter().any(|line| line.contains("page=")),
        "nothing changed on the page"
    );
}

/// The ring keeps the last REQUEST_RECORD_CAP rows.
#[test]
fn the_record_is_bounded() {
    let mut sut = on_page(false);
    for i in 0..(REQUEST_RECORD_CAP + 5) {
        ask(&mut sut, &i.to_string(), "eth_chainId", json!([]), T0);
    }
    let rows = inspected(&mut sut).rows;
    assert_eq!(rows.len(), REQUEST_RECORD_CAP);
    assert_eq!(rows[0].id, "5", "the oldest went first");
}

/// The full record reaches the view only for the inspected tab, and its
/// report carries methods and outcomes — never params or addresses.
#[test]
fn the_inspector_carries_the_record_and_nothing_private() {
    let mut sut = on_page(true);
    ask(
        &mut sut,
        "1",
        "eth_getBalance",
        json!([A1, "latest"]),
        T0 + 100.0,
    );
    answer_read(&mut sut, None, None, T0 + 105.0);
    assert!(sut.view().inspector.is_none(), "no record unless asked");

    let record = inspected(&mut sut);
    assert!(record.connected);
    assert_eq!(record.chain_id, 100);
    assert!(
        record
            .report
            .contains("eth_getBalance read failed -32603 network/no_endpoint 5ms"),
        "{}",
        record.report
    );
    assert!(record.report.contains("origin: https://dapp.example"));
    assert!(
        !record.report.to_lowercase().contains(&A1[2..]),
        "no address in the report"
    );

    sut.dispatch(Event::InspectorClosed);
    assert!(sut.view().inspector.is_none());
}

/// A duplicate id is answered without ending the open request's row.
#[test]
fn a_duplicate_id_does_not_end_the_open_row() {
    let mut sut = on_page(false);
    ask(&mut sut, "1", "eth_blockNumber", json!([]), T0);
    ask(&mut sut, "1", "eth_blockNumber", json!([]), T0 + 1.0);
    let rows = inspected(&mut sut).rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].outcome, DbrOutcome::Open);
}
