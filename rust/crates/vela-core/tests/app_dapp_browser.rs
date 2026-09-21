//! The in-app browser machine (spec 070) — one test per rule, driven exactly
//! the way a shell drives it: page messages in, operations out.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation as Op, DbrShellResult as Res, DbrStoredSite, Event, DEFAULT_CHAIN_ID,
    READS_IN_FLIGHT,
};
use vela_core::app::dapp_permissions::DpermGrant;
use vela_core::app::dapp_rpc::{self, classify, Route};
use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

type Sut = DomainDriver<DappBrowser>;

const T0: f64 = 1_754_700_000_000.0;
const DAPP: &str = "https://dapp.example";
const OTHER: &str = "https://other.example";
const A1: &str = "0x1111111111111111111111111111111111111111";
const A2: &str = "0x2222222222222222222222222222222222222222";
const A3: &str = "0x3333333333333333333333333333333333333333";

fn grant(origin: &str, address: &str, chain_id: u32) -> DpermGrant {
    DpermGrant {
        origin: origin.to_owned(),
        address: address.to_owned(),
        chain_id,
        granted_at_ms: T0,
    }
}

/// Started, sites listed, two accounts, A1 active, chains 1/100/8453.
fn booted(sites: Vec<DbrStoredSite>) -> Sut {
    let mut sut = Sut::new();
    assert_eq!(sut.dispatch(Event::Start), vec![Op::ListSites]);
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 100, 8453],
    });
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(vec![A1.to_owned(), A2.to_owned()]),
    });
    sut.dispatch(Event::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
    });
    sut.resolve(Res::SitesListed { sites });
    sut
}

fn fresh() -> Sut {
    booted(Vec::new())
}

fn connected(origin: &str) -> Sut {
    booted(vec![DbrStoredSite {
        origin: origin.to_owned(),
        grant: Some(grant(origin, A1, 100)),
        chain_id: None,
    }])
}

fn page(tab: &str, origin: &str, message: Value) -> Event {
    Event::PageMessage {
        tab: tab.to_owned(),
        frame_origin: origin.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
    }
}

fn hello(sut: &mut Sut, tab: &str, doc: &str, origin: &str) -> Vec<Op> {
    sut.dispatch(page(tab, origin, json!({"t":"hello","doc":doc})))
}

fn ask(
    sut: &mut Sut,
    tab: &str,
    doc: &str,
    origin: &str,
    id: &str,
    method: &str,
    params: Value,
) -> Vec<Op> {
    sut.dispatch(page(
        tab,
        origin,
        json!({"t":"req","doc":doc,"id":id,"method":method,"params":params}),
    ))
}

/// Every `Deliver` among `ops`, as `(tab, message)`.
fn delivered(ops: &[Op]) -> Vec<(String, Value)> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Deliver {
                tab,
                doc,
                message_json,
            } => {
                let value: Value = serde_json::from_str(message_json).unwrap();
                assert_eq!(value["doc"], json!(doc), "a delivery names its document");
                Some((tab.clone(), value))
            }
            _ => None,
        })
        .collect()
}

fn only_answer(ops: &[Op]) -> Value {
    let answers: Vec<Value> = delivered(ops)
        .into_iter()
        .filter(|(_, m)| m["dir"] == "res")
        .map(|(_, m)| m)
        .collect();
    assert_eq!(answers.len(), 1, "exactly one answer in {ops:?}");
    answers.into_iter().next().unwrap()
}

fn error_code(message: &Value) -> i64 {
    message["error"]["code"].as_i64().expect("an error answer")
}

fn events(ops: &[Op]) -> Vec<(String, String, Value)> {
    delivered(ops)
        .into_iter()
        .filter(|(_, m)| m["dir"] == "evt")
        .map(|(tab, m)| {
            (
                tab,
                m["event"].as_str().unwrap().to_owned(),
                m["data"].clone(),
            )
        })
        .collect()
}

fn forwarded(ops: &[Op]) -> Option<Op> {
    ops.iter()
        .find(|op| matches!(op, Op::ForwardToSigning { .. }))
        .cloned()
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

#[test]
fn the_table_matches_the_extension_protocol_file() {
    let protocol = include_str!("../../../../app-web/vela-wallet/extension/lib/protocol.js");
    let list = |name: &str| -> Vec<String> {
        let start = protocol
            .find(name)
            .unwrap_or_else(|| panic!("{name} in protocol.js"));
        let open = start + protocol[start..].find(['[', '(']).unwrap();
        let close = open + protocol[open..].find([']', ')']).unwrap();
        protocol[open..close]
            .split('\'')
            .enumerate()
            .filter(|(i, _)| i % 2 == 1)
            .map(|(_, s)| s.to_owned())
            .collect()
    };
    assert_eq!(
        list("READ_ONLY_RPC_METHODS = "),
        dapp_rpc::READ_ONLY_RPC_METHODS
    );
    assert_eq!(
        list("BUNDLER_METHODS = new Set("),
        dapp_rpc::BUNDLER_METHODS
    );
    let proxy = list("READ_PROXY_METHODS = new Set(");
    // `...READ_ONLY_RPC_METHODS, ...BUNDLER_METHODS,` then the extras.
    assert_eq!(proxy, dapp_rpc::EXTRA_READ_METHODS);
}

#[test]
fn the_provider_constants_match_the_extension_protocol_file() {
    let protocol = include_str!("../../../../app-web/vela-wallet/extension/lib/protocol.js");
    let provider = dapp_rpc::PROVIDER_JS;
    for pinned in [
        "CHANNEL = 'vela-1193'",
        "RDNS = 'app.getvela'",
        "WALLET_NAME = 'Vela Wallet'",
    ] {
        assert!(protocol.contains(pinned), "protocol.js: {pinned}");
        assert!(provider.contains(pinned), "provider: {pinned}");
    }
    for (name, code) in [
        ("USER_REJECTED", "4001"),
        ("UNAUTHORIZED", "4100"),
        ("UNSUPPORTED_METHOD", "4200"),
        ("UNKNOWN_PENDING", "4900"),
        ("CHAIN_NOT_ADDED", "4902"),
        ("INVALID_PARAMS", "-32602"),
        ("INTERNAL", "-32603"),
    ] {
        let line = format!("{name}: {code}");
        assert!(protocol.contains(&line), "protocol.js: {line}");
        assert!(provider.contains(&line), "provider: {line}");
    }
    assert_eq!(dapp_rpc::CHANNEL, "vela-1193");
}

#[test]
fn every_route() {
    assert_eq!(classify("eth_requestAccounts"), Route::Connect);
    assert_eq!(classify("wallet_requestPermissions"), Route::Connect);
    assert_eq!(classify("eth_accounts"), Route::Accounts);
    assert_eq!(classify("eth_coinbase"), Route::Coinbase);
    assert_eq!(classify("wallet_getPermissions"), Route::Permissions);
    assert_eq!(
        classify("wallet_revokePermissions"),
        Route::RevokePermissions
    );
    assert_eq!(classify("eth_chainId"), Route::ChainId);
    assert_eq!(classify("net_version"), Route::NetVersion);
    assert_eq!(classify("wallet_switchEthereumChain"), Route::SwitchChain);
    assert_eq!(classify("wallet_addEthereumChain"), Route::AddChain);
    assert_eq!(classify("wallet_watchAsset"), Route::WatchAsset);
    for method in [
        "eth_sendTransaction",
        "wallet_sendCalls",
        "personal_sign",
        "eth_signTypedData_v4",
    ] {
        assert_eq!(classify(method), Route::Sign, "{method}");
    }
    assert_eq!(classify("eth_call"), Route::Read { bundler: false });
    assert_eq!(
        classify("web3_clientVersion"),
        Route::Read { bundler: false }
    );
    assert_eq!(
        classify("eth_sendUserOperation"),
        Route::Read { bundler: true }
    );
    for method in [
        "eth_sign",
        "eth_signTransaction",
        "personal_ecRecover",
        "debug_traceCall",
        "",
    ] {
        assert_eq!(classify(method), Route::Unsupported, "{method}");
    }
}

#[test]
fn signing_words_live_in_the_core() {
    assert_eq!(
        dapp_rpc::sign_error_message(SignErrorKind::UserRejected),
        "User rejected the request"
    );
    assert_eq!(
        dapp_rpc::sign_error_message(SignErrorKind::UnlimitedApproval),
        "Unlimited approvals are disabled"
    );
}

// ---------------------------------------------------------------------------
// State answers
// ---------------------------------------------------------------------------

#[test]
fn a_new_site_starts_on_ethereum_and_sees_no_accounts() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x1"));
    assert_eq!(DEFAULT_CHAIN_ID, 1);
    let version = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "net_version",
        json!([]),
    ));
    assert_eq!(version["result"], json!("1"));
    let accounts = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(accounts["result"], json!([]));
    let coinbase = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "4",
        "eth_coinbase",
        json!([]),
    ));
    assert_eq!(coinbase["result"], Value::Null);
    let perms = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "5",
        "wallet_getPermissions",
        json!([]),
    ));
    assert_eq!(perms["result"], json!([]));
}

#[test]
fn a_connected_site_starts_on_the_chain_it_connected_on() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x64"));
    let accounts = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(accounts["result"], json!([A1]));
    let coinbase = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "eth_coinbase",
        json!([]),
    ));
    assert_eq!(coinbase["result"], json!(A1));
    let perms = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "4",
        "wallet_getPermissions",
        json!([]),
    ));
    assert_eq!(
        perms["result"],
        json!([{"parentCapability":"eth_accounts"}])
    );
}

#[test]
fn a_stored_chain_wins_over_the_grants_chain() {
    let mut sut = booted(vec![DbrStoredSite {
        origin: DAPP.to_owned(),
        grant: Some(grant(DAPP, A1, 100)),
        chain_id: Some(8453),
    }]);
    hello(&mut sut, "t1", "d1", DAPP);
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x2105"));
}

#[test]
fn watch_asset_is_false_and_unknowns_are_4200() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let watch = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_watchAsset",
        json!([{}]),
    ));
    assert_eq!(watch["result"], json!(false));
    let sign = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "eth_sign",
        json!([A1, "0x00"]),
    ));
    assert_eq!(error_code(&sign), 4200);
    let odd = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "eth_signTransaction",
        json!([]),
    ));
    assert_eq!(error_code(&odd), 4200);
    assert_eq!(
        odd["error"]["message"],
        json!("Vela does not support eth_signTransaction")
    );
}

// ---------------------------------------------------------------------------
// Connect
// ---------------------------------------------------------------------------

#[test]
fn connect_opens_one_sheet_and_approval_answers_every_merged_request() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    assert!(delivered(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([])
    ))
    .is_empty());
    assert!(delivered(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "wallet_requestPermissions",
        json!([{}])
    ))
    .is_empty());
    let consent = sut.view().consent.expect("consent sheet");
    assert_eq!(consent.origin, DAPP);
    assert_eq!(
        consent.methods,
        vec!["eth_requestAccounts", "wallet_requestPermissions"]
    );
    assert_eq!(consent.address.as_deref(), Some(A1));
    assert_eq!(consent.chain_id, 1);

    let ops = sut.dispatch(Event::ConsentApproved { now_ms: T0 + 5.0 });
    assert_eq!(
        ops[0],
        Op::WriteGrant {
            grant: DpermGrant {
                granted_at_ms: T0 + 5.0,
                ..grant(DAPP, A1, 1)
            }
        }
    );
    assert_eq!(
        ops[1],
        Op::SaveConnectionRecord {
            address: A1.to_owned(),
            chain_id: 1,
            origin: DAPP.to_owned()
        }
    );
    let answers: Vec<Value> = delivered(&ops)
        .into_iter()
        .filter(|(_, m)| m["dir"] == "res")
        .map(|(_, m)| m)
        .collect();
    assert_eq!(answers[0]["result"], json!([A1]));
    assert_eq!(
        answers[1]["result"],
        json!([{"parentCapability":"eth_accounts"}])
    );
    let evts = events(&ops);
    assert_eq!(
        evts[0],
        ("t1".to_owned(), "accountsChanged".to_owned(), json!([A1]))
    );
    assert_eq!(
        evts[1],
        ("t1".to_owned(), "chainChanged".to_owned(), json!("0x1"))
    );
    assert!(sut.view().consent.is_none());
    assert_eq!(sut.view().sites.len(), 1);
}

#[test]
fn a_granted_site_is_answered_without_a_sheet() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    ));
    assert_eq!(answer["result"], json!([A1]));
    assert!(sut.view().consent.is_none());
}

#[test]
fn rejecting_the_sheet_is_4001() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    );
    let answer = only_answer(&sut.dispatch(Event::ConsentRejected));
    assert_eq!(error_code(&answer), 4001);
    assert!(sut.view().sites.is_empty());
}

#[test]
fn a_second_origin_colliding_with_the_sheet_is_4001() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", OTHER);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    );
    let busy = only_answer(&ask(
        &mut sut,
        "t2",
        "d2",
        OTHER,
        "1",
        "eth_requestAccounts",
        json!([]),
    ));
    assert_eq!(error_code(&busy), 4001);
    assert_eq!(sut.view().consent.unwrap().origin, DAPP);
}

#[test]
fn connect_with_no_account_is_4001() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.resolve(Res::SitesListed { sites: Vec::new() });
    hello(&mut sut, "t1", "d1", DAPP);
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    ));
    assert_eq!(error_code(&answer), 4001);
}

#[test]
fn a_late_approve_after_the_page_left_grants_nothing() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    );
    let ops = hello(&mut sut, "t1", "d2", OTHER);
    assert_eq!(error_code(&only_answer(&ops)), 4900);
    assert!(sut.view().consent.is_none());
    assert!(sut
        .dispatch(Event::ConsentApproved { now_ms: T0 })
        .is_empty());
    assert!(sut.view().sites.is_empty());
}

#[test]
fn approval_announces_to_every_tab_of_that_origin_and_no_other() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", DAPP);
    hello(&mut sut, "t3", "d3", OTHER);
    ask(
        &mut sut,
        "t2",
        "d2",
        DAPP,
        "1",
        "eth_requestAccounts",
        json!([]),
    );
    let ops = sut.dispatch(Event::ConsentApproved { now_ms: T0 });
    let told: Vec<String> = events(&ops)
        .into_iter()
        .filter(|(_, name, _)| name == "accountsChanged")
        .map(|(tab, _, _)| tab)
        .collect();
    assert_eq!(told, vec!["t1", "t2"]);
}

// ---------------------------------------------------------------------------
// Chains
// ---------------------------------------------------------------------------

#[test]
fn switching_is_per_origin_persisted_and_announced_to_that_origin_only() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", OTHER);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x2105"}]),
    );
    assert!(ops.contains(&Op::WriteSiteChain {
        origin: DAPP.to_owned(),
        chain_id: 8453
    }));
    assert_eq!(only_answer(&ops)["result"], Value::Null);
    assert_eq!(
        events(&ops),
        vec![("t1".to_owned(), "chainChanged".to_owned(), json!("0x2105"))]
    );
    let other = only_answer(&ask(
        &mut sut,
        "t2",
        "d2",
        OTHER,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(other["result"], json!("0x1"));
    let mine = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(mine["result"], json!("0x2105"));
}

#[test]
fn switching_to_the_same_chain_writes_once_and_says_nothing() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x2105"}]),
    );
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "wallet_switchEthereumChain",
        json!([{"chainId":8453}]),
    );
    assert!(!ops.iter().any(|op| matches!(op, Op::WriteSiteChain { .. })));
    assert!(events(&ops).is_empty());
}

#[test]
fn add_chain_for_a_known_chain_is_a_switch() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_addEthereumChain",
        json!([{"chainId":"0x2105","chainName":"Base"}]),
    );
    assert_eq!(only_answer(&ops)["result"], Value::Null);
    assert_eq!(events(&ops)[0].1, "chainChanged");
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x2105"));
}

#[test]
fn unknown_chains_are_4902_and_bad_params_are_32602() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let switch = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x89"}]),
    ));
    assert_eq!(error_code(&switch), 4902);
    let add = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "wallet_addEthereumChain",
        json!([{"chainId":"0x89"}]),
    ));
    assert_eq!(error_code(&add), 4902);
    assert_eq!(
        add["error"]["message"],
        json!("Add chain 137 in Vela's network settings first")
    );
    let bad = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"nope"}]),
    ));
    assert_eq!(error_code(&bad), -32602);
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "4",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x64"), "nothing changed");
}

#[test]
fn a_person_can_pick_a_sites_network() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = sut.dispatch(Event::SiteChainPicked {
        origin: DAPP.to_owned(),
        chain_id: 1,
    });
    assert!(ops.contains(&Op::WriteSiteChain {
        origin: DAPP.to_owned(),
        chain_id: 1
    }));
    assert_eq!(
        events(&ops),
        vec![("t1".to_owned(), "chainChanged".to_owned(), json!("0x1"))]
    );
    let unknown = sut.dispatch(Event::SiteChainPicked {
        origin: DAPP.to_owned(),
        chain_id: 137,
    });
    assert!(unknown.is_empty(), "only a chain the wallet has");
    assert_eq!(sut.view().tabs[0].chain_id, 1);
}

// ---------------------------------------------------------------------------
// Accounts follow, revoke
// ---------------------------------------------------------------------------

#[test]
fn every_grant_follows_the_active_account() {
    let mut sut = booted(vec![
        DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(grant(DAPP, A1, 100)),
            chain_id: None,
        },
        DbrStoredSite {
            origin: OTHER.to_owned(),
            grant: Some(grant(OTHER, A1, 1)),
            chain_id: None,
        },
    ]);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = sut.dispatch(Event::AccountSwitched {
        address: A2.to_owned(),
        now_ms: T0 + 9.0,
    });
    let writes: Vec<&Op> = ops
        .iter()
        .filter(|op| matches!(op, Op::WriteGrant { .. }))
        .collect();
    assert_eq!(writes.len(), 2);
    assert!(writes.contains(&&Op::WriteGrant {
        grant: DpermGrant {
            address: A2.to_owned(),
            granted_at_ms: T0 + 9.0,
            ..grant(DAPP, A1, 100)
        }
    }));
    assert_eq!(
        events(&ops),
        vec![("t1".to_owned(), "accountsChanged".to_owned(), json!([A2]))]
    );
    let accounts = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(accounts["result"], json!([A2]));
}

#[test]
fn a_grant_for_an_account_the_wallet_no_longer_has_is_dropped() {
    let mut sut = booted(vec![DbrStoredSite {
        origin: DAPP.to_owned(),
        grant: Some(grant(DAPP, A3, 100)),
        chain_id: None,
    }]);
    // A3 is not one of the wallet's accounts: dropped at the list, not re-pinned.
    assert!(sut.view().sites.is_empty());
    hello(&mut sut, "t1", "d1", DAPP);
    let accounts = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(accounts["result"], json!([]));
}

#[test]
fn a_cold_account_read_never_logs_a_site_out() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(Event::AccountsUpdated { addresses: None });
    sut.resolve(Res::SitesListed {
        sites: vec![DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(grant(DAPP, A3, 100)),
            chain_id: None,
        }],
    });
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(Vec::new()),
    });
    assert_eq!(sut.view().sites.len(), 1);
    hello(&mut sut, "t1", "d1", DAPP);
    let accounts = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(accounts["result"], json!([A3]));
}

#[test]
fn revoking_tells_every_open_tab_of_that_site() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", DAPP);
    let ops = sut.dispatch(Event::RevokeRequested {
        origin: DAPP.to_owned(),
    });
    assert!(ops.contains(&Op::RemoveGrant {
        origin: DAPP.to_owned()
    }));
    let names: Vec<(String, String)> = events(&ops).into_iter().map(|(t, n, _)| (t, n)).collect();
    assert_eq!(
        names,
        vec![
            ("t1".to_owned(), "accountsChanged".to_owned()),
            ("t2".to_owned(), "accountsChanged".to_owned()),
            ("t1".to_owned(), "disconnect".to_owned()),
            ("t2".to_owned(), "disconnect".to_owned()),
        ]
    );
    assert!(sut.view().sites.is_empty());
    assert!(sut
        .dispatch(Event::RevokeRequested {
            origin: DAPP.to_owned()
        })
        .is_empty());
}

#[test]
fn revoke_all_clears_every_grant_but_keeps_chains() {
    let mut sut = booted(vec![
        DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(grant(DAPP, A1, 100)),
            chain_id: Some(8453),
        },
        DbrStoredSite {
            origin: OTHER.to_owned(),
            grant: Some(grant(OTHER, A1, 1)),
            chain_id: None,
        },
    ]);
    let ops = sut.dispatch(Event::RevokeAll);
    assert_eq!(
        ops.iter()
            .filter(|op| matches!(op, Op::RemoveGrant { .. }))
            .count(),
        2
    );
    assert!(sut.view().sites.is_empty());
    hello(&mut sut, "t1", "d1", DAPP);
    let chain = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(chain["result"], json!("0x2105"));
}

#[test]
fn a_site_can_disconnect_itself() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_revokePermissions",
        json!([{"eth_accounts":{}}]),
    );
    assert!(ops.contains(&Op::RemoveGrant {
        origin: DAPP.to_owned()
    }));
    assert_eq!(only_answer(&ops)["result"], Value::Null);
}

// ---------------------------------------------------------------------------
// Signing
// ---------------------------------------------------------------------------

#[test]
fn signing_needs_a_grant() {
    let mut sut = fresh();
    hello(&mut sut, "t1", "d1", DAPP);
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x68656c6c6f", A1]),
    ));
    assert_eq!(error_code(&answer), 4100);
}

#[test]
fn signing_on_public_http_is_refused() {
    let site = "http://dapp.example";
    let mut sut = connected(site);
    hello(&mut sut, "t1", "d1", site);
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        site,
        "1",
        "personal_sign",
        json!(["0x00", A1]),
    ));
    assert_eq!(error_code(&answer), 4100);
    let local = "http://192.168.1.4:8137";
    let mut sut = connected(local);
    hello(&mut sut, "t1", "d1", local);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        local,
        "1",
        "personal_sign",
        json!(["0x00", A1]),
    );
    assert!(forwarded(&ops).is_some(), "a LAN test dApp may sign");
}

#[test]
fn a_signature_is_forwarded_with_the_sites_chain_and_granted_address() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "7",
        "eth_sendTransaction",
        json!([{"to":A2,"value":"0x1"}]),
    );
    assert_eq!(
        forwarded(&ops),
        Some(Op::ForwardToSigning {
            tab: "t1".to_owned(),
            id: "7".to_owned(),
            method: "eth_sendTransaction".to_owned(),
            params_json: json!([{"to":A2,"value":"0x1"}]).to_string(),
            origin: DAPP.to_owned(),
            chain_id: 100,
            granted_address: A1.to_owned(),
        })
    );
    assert_eq!(sut.view().signing.unwrap().id, "7");
    let ops = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "7".to_owned(),
        payload: SignResponsePayload::Ok {
            result: Some("0xabc".to_owned()),
        },
        user_op_hash: None,
    });
    assert_eq!(only_answer(&ops)["result"], json!("0xabc"));
    assert!(sut.view().signing.is_none());
}

#[test]
fn a_signing_refusal_carries_the_cores_words() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x00", A1]),
    );
    let ops = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Err {
            code: 4001,
            kind: SignErrorKind::UserRejected,
            message: None,
        },
        user_op_hash: None,
    });
    let answer = only_answer(&ops);
    assert_eq!(error_code(&answer), 4001);
    assert_eq!(
        answer["error"]["message"],
        json!("User rejected the request")
    );
}

#[test]
fn a_second_signature_waits_in_line_and_opens_when_the_first_is_answered() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    assert!(forwarded(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x01", A1])
    ))
    .is_some());
    let second = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x02", A1]),
    );
    assert!(forwarded(&second).is_none());
    assert!(delivered(&second).is_empty(), "neither dropped nor refused");
    assert_eq!(sut.view().queued_signing, 1);
    let ops = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Ok {
            result: Some("0x11".to_owned()),
        },
        user_op_hash: None,
    });
    assert_eq!(only_answer(&ops)["id"], json!("1"));
    match forwarded(&ops) {
        Some(Op::ForwardToSigning { id, .. }) => assert_eq!(id, "2"),
        other => panic!("the queued request opens next: {other:?}"),
    }
}

#[test]
fn an_answer_is_delivered_exactly_once() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x01", A1]),
    );
    let answered = SignResponsePayload::Ok {
        result: Some("0x11".to_owned()),
    };
    sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: answered.clone(),
        user_op_hash: None,
    });
    let again = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: answered,
        user_op_hash: None,
    });
    assert!(delivered(&again).is_empty());
}

#[test]
fn a_navigation_mid_signature_settles_4900_and_closes_the_sheet() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_sendTransaction",
        json!([{"to":A2}]),
    );
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x02", A1]),
    );
    sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: "https://dapp.example/next".to_owned(),
    });
    let ops = hello(&mut sut, "t1", "d2", DAPP);
    let settled: Vec<i64> = delivered(&ops).iter().map(|(_, m)| error_code(m)).collect();
    assert_eq!(settled, vec![4900, 4900]);
    assert!(ops.contains(&Op::CancelSigning {
        tab: "t1".to_owned(),
        id: "1".to_owned()
    }));
    assert!(
        forwarded(&ops).is_none(),
        "the queued one died with its page"
    );
    assert!(sut.view().signing.is_none());
    assert_eq!(sut.view().queued_signing, 0);
    // The pipeline answers late: nobody hears it.
    let late = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Ok {
            result: Some("0x11".to_owned()),
        },
        user_op_hash: None,
    });
    assert!(delivered(&late).is_empty());
}

#[test]
fn closing_another_tab_moves_the_signing_line_on() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x01", A1]),
    );
    ask(
        &mut sut,
        "t2",
        "d2",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x02", A1]),
    );
    let ops = sut.dispatch(Event::TabClosed {
        tab: "t1".to_owned(),
    });
    assert!(ops.contains(&Op::CancelSigning {
        tab: "t1".to_owned(),
        id: "1".to_owned()
    }));
    match forwarded(&ops) {
        Some(Op::ForwardToSigning { tab, .. }) => assert_eq!(tab, "t2"),
        other => panic!("t2's request opens: {other:?}"),
    }
    assert!(
        delivered(&ops).is_empty(),
        "nothing is posted into a closed tab"
    );
}

#[test]
fn a_revoke_while_queued_refuses_the_waiting_signature() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x01", A1]),
    );
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x02", A1]),
    );
    sut.dispatch(Event::RevokeRequested {
        origin: DAPP.to_owned(),
    });
    let ops = sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Err {
            code: 4001,
            kind: SignErrorKind::UserRejected,
            message: None,
        },
        user_op_hash: None,
    });
    let answers: Vec<i64> = delivered(&ops)
        .iter()
        .filter(|(_, m)| m["dir"] == "res")
        .map(|(_, m)| error_code(m))
        .collect();
    assert_eq!(answers, vec![4001, 4100]);
    assert!(forwarded(&ops).is_none());
}

// ---------------------------------------------------------------------------
// Documents
// ---------------------------------------------------------------------------

#[test]
fn the_new_documents_warm_up_survives_a_late_navigation_callback() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    // Android: the new document's hello and first request beat onPageStarted.
    hello(&mut sut, "t1", "d2", DAPP);
    let read = ask(
        &mut sut,
        "t1",
        "d2",
        DAPP,
        "1",
        "eth_blockNumber",
        json!([]),
    );
    assert!(read.iter().any(|op| matches!(op, Op::Read { .. })));
    let nav = sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: "https://dapp.example/".to_owned(),
    });
    assert!(
        delivered(&nav).is_empty(),
        "navigation_started settles nothing"
    );
    sut.dispatch(page("t1", DAPP, json!({"t":"hello","doc":"d2"})));
    let done = sut.dispatch(Event::LoadFinished {
        tab: "t1".to_owned(),
        url: "https://dapp.example/".to_owned(),
    });
    assert!(delivered(&done).is_empty());
    let answer = sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered {
            body_json: Some(r#"{"result":"0x10"}"#.to_owned()),
        },
    );
    assert_eq!(only_answer(&answer)["result"], json!("0x10"));
}

#[test]
fn a_load_that_ends_without_a_hello_settles_the_old_document() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_sendTransaction",
        json!([{"to":A2}]),
    );
    sut.dispatch(Event::NavigationStarted {
        tab: "t1".to_owned(),
        url: "https://broken.example/".to_owned(),
    });
    let ops = sut.dispatch(Event::LoadFinished {
        tab: "t1".to_owned(),
        url: "https://broken.example/".to_owned(),
    });
    assert_eq!(error_code(&only_answer(&ops)), 4900);
    assert!(ops.contains(&Op::CancelSigning {
        tab: "t1".to_owned(),
        id: "1".to_owned()
    }));
    assert_eq!(
        sut.view().tabs[0].origin.as_deref(),
        Some("https://broken.example")
    );
}

#[test]
fn a_renderer_crash_settles_and_marks_the_tab() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x01", A1]),
    );
    let ops = sut.dispatch(Event::RendererGone {
        tab: "t1".to_owned(),
    });
    assert!(ops.contains(&Op::CancelSigning {
        tab: "t1".to_owned(),
        id: "1".to_owned()
    }));
    assert!(
        delivered(&ops).is_empty(),
        "a dead renderer is posted nothing"
    );
    assert!(sut.view().tabs[0].crashed);
    hello(&mut sut, "t1", "d2", DAPP);
    assert!(!sut.view().tabs[0].crashed);
}

#[test]
fn a_stale_document_is_never_answered_and_never_steals_the_tab() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t1", "d2", DAPP);
    let stale = ask(&mut sut, "t1", "d1", DAPP, "9", "eth_accounts", json!([]));
    assert!(stale.is_empty());
    let current = only_answer(&ask(
        &mut sut,
        "t1",
        "d2",
        DAPP,
        "1",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(current["doc"], json!("d2"));
}

#[test]
fn a_back_forward_cache_restore_makes_the_old_document_current_again() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t1", "d2", DAPP);
    hello(&mut sut, "t1", "d1", DAPP); // pageshow { persisted }
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_accounts",
        json!([]),
    ));
    assert_eq!(answer["doc"], json!("d1"));
}

#[test]
fn a_lost_hello_is_recovered_from_the_first_request() {
    let mut sut = connected(DAPP);
    let answer = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_chainId",
        json!([]),
    ));
    assert_eq!(answer["result"], json!("0x64"));
}

#[test]
fn messages_before_the_sites_are_listed_are_held_not_refused() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(Event::NetworksChanged {
        chain_ids: vec![1, 100],
    });
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(vec![A1.to_owned()]),
    });
    sut.dispatch(Event::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
    });
    hello(&mut sut, "t1", "d1", DAPP);
    assert!(ask(&mut sut, "t1", "d1", DAPP, "1", "eth_accounts", json!([])).is_empty());
    let ops = sut.resolve(Res::SitesListed {
        sites: vec![DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(grant(DAPP, A1, 100)),
            chain_id: None,
        }],
    });
    assert_eq!(only_answer(&ops)["result"], json!([A1]));
}

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

#[test]
fn answers_go_to_the_tab_that_asked() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", DAPP);
    let ops = ask(&mut sut, "t2", "d2", DAPP, "1", "eth_accounts", json!([]));
    assert_eq!(delivered(&ops)[0].0, "t2");
}

#[test]
fn a_navigation_in_one_tab_leaves_another_tabs_requests_alone() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", DAPP);
    ask(
        &mut sut,
        "t2",
        "d2",
        DAPP,
        "1",
        "eth_blockNumber",
        json!([]),
    );
    let ops = hello(&mut sut, "t1", "d3", OTHER);
    assert!(delivered(&ops).is_empty());
    let answer = sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered {
            body_json: Some(r#"{"result":"0x1"}"#.to_owned()),
        },
    );
    assert_eq!(delivered(&answer)[0].0, "t2");
}

#[test]
fn tab_views_tell_the_truth_about_the_lock_and_the_connection() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    hello(&mut sut, "t2", "d2", "http://insecure.example");
    let view = sut.view();
    let t1 = view.tabs.iter().find(|t| t.tab == "t1").unwrap();
    assert!(t1.secure);
    assert_eq!(t1.connected_address.as_deref(), Some(A1));
    assert_eq!(t1.chain_id, 100);
    let t2 = view.tabs.iter().find(|t| t.tab == "t2").unwrap();
    assert!(!t2.secure);
    assert_eq!(t2.connected_address, None);
}

#[test]
fn a_closed_tab_is_forgotten() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    sut.dispatch(Event::TabClosed {
        tab: "t1".to_owned(),
    });
    assert!(sut.view().tabs.is_empty());
}

// ---------------------------------------------------------------------------
// Frames and malformed input
// ---------------------------------------------------------------------------

#[test]
fn a_subframe_can_never_speak_as_the_top_page() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = sut.dispatch(Event::PageMessage {
        tab: "t1".to_owned(),
        frame_origin: "https://ads.example".to_owned(),
        is_main_frame: false,
        message_json: json!({"t":"req","doc":"d1","id":"x","method":"eth_sendTransaction","params":[{"to":A2}]}).to_string(),
    });
    assert!(ops.is_empty());
    assert!(sut.view().signing.is_none());
}

#[test]
fn a_frame_origin_that_does_not_match_its_document_is_ignored() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(&mut sut, "t1", "d1", OTHER, "1", "eth_accounts", json!([]));
    assert!(ops.is_empty());
}

#[test]
fn non_web_origins_are_ignored() {
    let mut sut = connected(DAPP);
    let ops = sut.dispatch(Event::PageMessage {
        tab: "t1".to_owned(),
        frame_origin: "file:///etc/passwd".to_owned(),
        is_main_frame: true,
        message_json: json!({"t":"hello","doc":"d1"}).to_string(),
    });
    assert!(ops.is_empty());
    assert!(sut.view().tabs.is_empty());
}

#[test]
fn malformed_requests_with_an_id_are_told_and_without_one_are_dropped() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let object_params = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_call",
        json!({"to":A2}),
    ));
    assert_eq!(error_code(&object_params), -32602);
    let no_method =
        only_answer(&sut.dispatch(page("t1", DAPP, json!({"t":"req","doc":"d1","id":"2"}))));
    assert_eq!(error_code(&no_method), -32600);
    let unserialisable = only_answer(&sut.dispatch(page(
        "t1",
        DAPP,
        json!({"t":"req","doc":"d1","id":"3","method":"eth_call","bad":true}),
    )));
    assert_eq!(error_code(&unserialisable), -32602);
    assert!(sut
        .dispatch(page(
            "t1",
            DAPP,
            json!({"t":"req","doc":"d1","method":"eth_call"})
        ))
        .is_empty());
    assert!(sut.dispatch(page("t1", DAPP, json!("hello"))).is_empty());
}

#[test]
fn a_duplicate_open_id_is_refused() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_blockNumber",
        json!([]),
    );
    let dup = only_answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_blockNumber",
        json!([]),
    ));
    assert_eq!(error_code(&dup), -32602);
}

// ---------------------------------------------------------------------------
// Reads
// ---------------------------------------------------------------------------

#[test]
fn a_read_goes_to_the_sites_chain_and_comes_back() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_getBalance",
        json!([A1, "latest"]),
    );
    assert!(ops.contains(&Op::Read {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        chain_id: 100,
        method: "eth_getBalance".to_owned(),
        params_json: json!([A1, "latest"]).to_string(),
        bundler: false,
    }));
    let answer = sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered {
            body_json: Some(r#"{"jsonrpc":"2.0","id":1,"result":"0xde0b6b3a7640000"}"#.to_owned()),
        },
    );
    assert_eq!(only_answer(&answer)["result"], json!("0xde0b6b3a7640000"));
}

#[test]
fn a_node_error_is_passed_through_and_silence_is_32603() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(&mut sut, "t1", "d1", DAPP, "1", "eth_call", json!([{}]));
    ask(&mut sut, "t1", "d1", DAPP, "2", "eth_call", json!([{}]));
    let revert = sut.resolve_matching(
        |op| matches!(op, Op::Read { id, .. } if id == "1"),
        Res::ReadAnswered {
            body_json: Some(
                r#"{"error":{"code":3,"message":"execution reverted","data":"0x08c379a0"}}"#
                    .to_owned(),
            ),
        },
    );
    let revert = only_answer(&revert);
    assert_eq!(
        revert["error"],
        json!({"code":3,"message":"execution reverted","data":"0x08c379a0"})
    );
    let silent = sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered { body_json: None },
    );
    assert_eq!(error_code(&only_answer(&silent)), -32603);
}

#[test]
fn bundler_reads_are_marked() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_getUserOperationReceipt",
        json!(["0xab"]),
    );
    assert!(ops
        .iter()
        .any(|op| matches!(op, Op::Read { bundler: true, .. })));
}

#[test]
fn reads_are_bounded_per_tab() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    for i in 0..READS_IN_FLIGHT {
        let ops = ask(
            &mut sut,
            "t1",
            "d1",
            DAPP,
            &format!("r{i}"),
            "eth_blockNumber",
            json!([]),
        );
        assert!(ops.iter().any(|op| matches!(op, Op::Read { .. })));
    }
    let queued = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "q",
        "eth_blockNumber",
        json!([]),
    );
    assert!(
        !queued.iter().any(|op| matches!(op, Op::Read { .. })),
        "waits for a slot"
    );
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::Read { id, .. } if id == "r0"),
        Res::ReadAnswered {
            body_json: Some(r#"{"result":"0x1"}"#.to_owned()),
        },
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::Read { id, .. } if id == "q")),
        "the queued read takes the slot"
    );
}

#[test]
fn a_page_can_look_up_the_receipt_of_the_user_operation_it_was_answered_with() {
    let op_hash = format!("0x{}", "ab".repeat(32));
    let tx_hash = format!("0x{}", "cd".repeat(32));
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_sendTransaction",
        json!([{"to":A2}]),
    );
    sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Ok {
            result: Some(op_hash.clone()),
        },
        user_op_hash: Some(op_hash.clone()),
    });
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "eth_getTransactionReceipt",
        json!([op_hash.to_uppercase().replace("0X", "0x")]),
    );
    assert!(ops.contains(&Op::ResolveUserOp {
        chain_id: 100,
        user_op_hash: op_hash.clone()
    }));
    // Pending: `null`, as a node says of a pending transaction.
    let pending = sut.resolve_matching(
        |op| matches!(op, Op::ResolveUserOp { .. }),
        Res::UserOpResolved { tx_hash: None },
    );
    assert_eq!(only_answer(&pending)["result"], Value::Null);
    // Landed: the node is asked for the real transaction's receipt.
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "eth_getTransactionReceipt",
        json!([op_hash]),
    );
    let landed = sut.resolve_matching(
        |op| matches!(op, Op::ResolveUserOp { .. }),
        Res::UserOpResolved {
            tx_hash: Some(tx_hash.clone()),
        },
    );
    assert!(landed.contains(&Op::Read {
        tab: "t1".to_owned(),
        id: "3".to_owned(),
        chain_id: 100,
        method: "eth_getTransactionReceipt".to_owned(),
        params_json: json!([tx_hash]).to_string(),
        bundler: false,
    }));
    // An ordinary hash is an ordinary read.
    let plain = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "4",
        "eth_getTransactionReceipt",
        json!([format!("0x{}", "ef".repeat(32))]),
    );
    assert!(plain.iter().any(|op| matches!(op, Op::Read { .. })));
}

#[test]
fn a_read_answered_after_its_page_left_is_dropped() {
    let mut sut = connected(DAPP);
    hello(&mut sut, "t1", "d1", DAPP);
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "eth_blockNumber",
        json!([]),
    );
    hello(&mut sut, "t1", "d2", DAPP);
    let late = sut.resolve_matching(
        |op| matches!(op, Op::Read { .. }),
        Res::ReadAnswered {
            body_json: Some(r#"{"result":"0x1"}"#.to_owned()),
        },
    );
    assert!(delivered(&late).is_empty());
}

// ---------------------------------------------------------------------------
// Settings' list
// ---------------------------------------------------------------------------

#[test]
fn the_sites_list_is_newest_first_with_each_sites_chain() {
    let mut sut = booted(vec![
        DbrStoredSite {
            origin: DAPP.to_owned(),
            grant: Some(grant(DAPP, A1, 100)),
            chain_id: Some(8453),
        },
        DbrStoredSite {
            origin: OTHER.to_owned(),
            grant: Some(DpermGrant {
                granted_at_ms: T0 + 1.0,
                ..grant(OTHER, A1, 1)
            }),
            chain_id: None,
        },
        DbrStoredSite {
            origin: "https://chain-only.example".to_owned(),
            grant: None,
            chain_id: Some(100),
        },
    ]);
    let sites = sut.view().sites;
    assert_eq!(sites.len(), 2);
    assert_eq!(sites[0].origin, OTHER);
    assert_eq!(sites[1].origin, DAPP);
    assert_eq!(sites[1].chain_id, 8453);
    assert!(sut.view().ready);
    let _ = &mut sut;
}

#[test]
fn a_grant_filed_under_the_wrong_origin_is_ignored() {
    let sut = booted(vec![DbrStoredSite {
        origin: DAPP.to_owned(),
        grant: Some(grant(OTHER, A1, 100)),
        chain_id: None,
    }]);
    assert!(sut.view().sites.is_empty());
}
