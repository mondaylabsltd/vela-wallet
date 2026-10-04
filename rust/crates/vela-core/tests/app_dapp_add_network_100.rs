//! Spec 100 — a page asks Vela to add a network (`wallet_addEthereumChain`).
//!
//! One test per rule, driven the way a shell drives it: `dapp_browser` routes
//! the page's request and answers it; `network_admin` checks the chain the way
//! Settings' wizard does and adds it the way Settings does; the shell only
//! carries `forward_to_add_network` → `dapp_add_requested` and
//! `dapp_add_settled` → `add_network_answered` between them (the last tests
//! here do exactly that, both machines together).

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation as DOp, DbrShellResult as DRes, Event as DEvent,
};
use vela_core::app::dapp_record::{DbrLayer, DbrOutcome, DbrReason, DbrRequestClass};
use vela_core::app::dapp_rpc::{
    add_chain_ask, add_outcome_error, usable_rpc_url, DappAddOutcome, DappChainAsk,
};
use vela_core::app::network_admin::{
    Event as NEvent, NetCustomNetwork, NetDappAddPhase, NetOperation as NOp, NetProviderKeys,
    NetRawChainData, NetShellResult as NRes, NetStoredEndpoints, NetWizardPhase, NetworkAdmin,
    REQUIRED_CONTRACTS,
};

type Browser = DomainDriver<DappBrowser>;
type Admin = DomainDriver<NetworkAdmin>;

const T0: f64 = 1_754_700_000_000.0;
const DAPP: &str = "https://dapp.example";
const A1: &str = "0x1111111111111111111111111111111111111111";
/// A chain no build of Vela carries.
const NEW_CHAIN: u32 = 7777;
const CATALOG_RPC: &str = "https://rpc.catalog.example";
const SITE_RPC: &str = "https://rpc.site.example";
const NOW_ISO: &str = "2026-10-04T12:00:00.000Z";

// ---------------------------------------------------------------------------
// The browser
// ---------------------------------------------------------------------------

fn browser() -> Browser {
    let mut sut = Browser::new();
    assert_eq!(sut.dispatch(DEvent::Start), vec![DOp::ListSites]);
    sut.dispatch(DEvent::NetworksChanged {
        chain_ids: vec![1, 100, 8453],
    });
    sut.dispatch(DEvent::AccountsUpdated {
        addresses: Some(vec![A1.to_owned()]),
    });
    sut.dispatch(DEvent::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
    });
    sut.resolve(DRes::SitesListed { sites: Vec::new() });
    sut.dispatch(page("t1", json!({"t":"hello","doc":"d1"})));
    sut
}

fn page(tab: &str, message: Value) -> DEvent {
    DEvent::PageMessage {
        tab: tab.to_owned(),
        frame_origin: DAPP.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
        now_ms: T0,
    }
}

fn ask(sut: &mut Browser, id: &str, method: &str, params: Value) -> Vec<DOp> {
    sut.dispatch(page(
        "t1",
        json!({"t":"req","doc":"d1","id":id,"method":method,"params":params}),
    ))
}

fn add(sut: &mut Browser, id: &str, params: Value) -> Vec<DOp> {
    ask(sut, id, "wallet_addEthereumChain", params)
}

/// What a typical dApp sends (wagmi's shape).
fn full_params(chain_id: u32) -> Value {
    json!([{
        "chainId": format!("0x{chain_id:x}"),
        "chainName": "Testland",
        "nativeCurrency": { "name": "Test", "symbol": "TST", "decimals": 18 },
        "rpcUrls": [SITE_RPC],
        "blockExplorerUrls": ["https://scan.site.example"],
    }])
}

fn answers(ops: &[DOp]) -> Vec<Value> {
    ops.iter()
        .filter_map(|op| match op {
            DOp::Deliver { message_json, .. } => serde_json::from_str::<Value>(message_json).ok(),
            _ => None,
        })
        .collect()
}

fn the_answer(ops: &[DOp]) -> Value {
    let found: Vec<Value> = answers(ops)
        .into_iter()
        .filter(|m| m["dir"] == "res")
        .collect();
    assert_eq!(found.len(), 1, "exactly one answer in {ops:?}");
    found.into_iter().next().unwrap()
}

fn chain_changed(ops: &[DOp]) -> Vec<Value> {
    answers(ops)
        .into_iter()
        .filter(|m| m["dir"] == "evt" && m["event"] == "chainChanged")
        .map(|m| m["data"].clone())
        .collect()
}

fn forwarded(ops: &[DOp]) -> Option<DappChainAsk> {
    ops.iter().find_map(|op| match op {
        DOp::ForwardToAddNetwork { ask, .. } => Some(ask.clone()),
        _ => None,
    })
}

fn answered(sut: &mut Browser, id: &str, outcome: DappAddOutcome) -> Vec<DOp> {
    sut.dispatch(DEvent::AddNetworkAnswered {
        tab: "t1".to_owned(),
        id: id.to_owned(),
        outcome,
        now_ms: T0 + 4_000.0,
    })
}

/// The tab's record row for request `id` (the inspector carries them).
fn row(sut: &mut Browser, id: &str) -> vela_core::app::dapp_record::DbrRequestRow {
    sut.dispatch(DEvent::InspectorOpened {
        tab: "t1".to_owned(),
    });
    let rows = sut.view().inspector.expect("inspected").rows;
    rows.into_iter()
        .rev()
        .find(|row| row.id == id)
        .expect("a row for the request")
}

// ---------------------------------------------------------------------------
// The sheet's machine
// ---------------------------------------------------------------------------

fn admin_with(customs: Vec<NetCustomNetwork>) -> Admin {
    let mut sut = Admin::new();
    assert_eq!(sut.dispatch(NEvent::Started), vec![NOp::ReadStore]);
    sut.resolve(NRes::StoreLoaded {
        custom_networks: customs,
        network_configs: vec![],
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
    });
    sut
}

fn admin() -> Admin {
    admin_with(Vec::new())
}

fn site_ask(chain_id: u32, rpc_urls: &[&str]) -> DappChainAsk {
    DappChainAsk {
        chain_id,
        chain_name: Some("Site Chain".to_owned()),
        native_symbol: Some("SITE".to_owned()),
        rpc_urls: rpc_urls.iter().map(|url| (*url).to_owned()).collect(),
        refused_rpc_urls: 0,
        explorer_url: Some("https://scan.site.example".to_owned()),
    }
}

fn requested(sut: &mut Admin, ask: DappChainAsk) -> Vec<NOp> {
    sut.dispatch(NEvent::DappAddRequested {
        tab: "t1".to_owned(),
        id: "a1".to_owned(),
        origin: DAPP.to_owned(),
        ask,
    })
}

fn catalog_entry() -> NetRawChainData {
    NetRawChainData {
        chain_id: Some(NEW_CHAIN),
        name: Some("Catalog Chain".to_owned()),
        short_name: Some("cat".to_owned()),
        native_currency_name: Some("Catalog Coin".to_owned()),
        native_currency_symbol: Some("CAT".to_owned()),
        native_currency_decimals: Some(18),
        rpc: vec![CATALOG_RPC.to_owned()],
        explorers: vec!["https://scan.catalog.example".to_owned()],
        testnet: false,
    }
}

fn catalog(sut: &mut Admin, data: Option<NetRawChainData>) -> Vec<NOp> {
    sut.resolve_matching(
        |op| {
            matches!(
                op,
                NOp::FetchChainInfo {
                    chain_id: NEW_CHAIN
                }
            )
        },
        NRes::ChainInfo {
            chain_id: NEW_CHAIN,
            data,
        },
    )
}

fn probed(sut: &mut Admin, url: &str, reported: Option<u32>) -> Vec<NOp> {
    let wanted = url.to_owned();
    sut.resolve_matching(
        move |op| matches!(op, NOp::ProbeRpc { url } if *url == wanted),
        NRes::Probed {
            url: url.to_owned(),
            reported_chain_id: reported,
            latency_ms: 30.0,
        },
    )
}

/// Every contract answered (deployed or not) and the P256 call.
fn contracts(sut: &mut Admin, url: &str, deployed: bool) -> Vec<NOp> {
    for (_, address, _) in REQUIRED_CONTRACTS {
        let wanted = (*address).to_owned();
        sut.resolve_matching(
            move |op| matches!(op, NOp::RpcGetCode { address, .. } if *address == wanted),
            NRes::Code {
                url: url.to_owned(),
                address: (*address).to_owned(),
                code: deployed.then(|| "0x6080604052".to_owned()),
            },
        );
    }
    sut.resolve_matching(
        |op| matches!(op, NOp::RpcCallP256 { .. }),
        NRes::P256Call {
            url: url.to_owned(),
            result: Some(format!("0x{}1", "0".repeat(63))),
        },
    )
}

fn settled(ops: &[NOp]) -> Option<DappAddOutcome> {
    ops.iter().find_map(|op| match op {
        NOp::DappAddSettled { outcome, .. } => Some(outcome.clone()),
        _ => None,
    })
}

fn writes(ops: &[NOp]) -> Option<Vec<NetCustomNetwork>> {
    ops.iter().find_map(|op| match op {
        NOp::WriteCustomNetworks { networks } => Some(networks.clone()),
        _ => None,
    })
}

/// A dApp add of NEW_CHAIN driven to `ready` through the catalog.
fn ready_from_catalog() -> Admin {
    let mut sut = admin();
    let ops = requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    assert_eq!(
        ops,
        vec![NOp::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
    let ops = catalog(&mut sut, Some(catalog_entry()));
    assert_eq!(
        ops,
        vec![NOp::ProbeRpc {
            url: CATALOG_RPC.to_owned()
        }],
        "the catalog's RPC — never the page's"
    );
    let ops = probed(&mut sut, CATALOG_RPC, Some(NEW_CHAIN));
    assert_eq!(ops.len(), REQUIRED_CONTRACTS.len() + 1);
    contracts(&mut sut, CATALOG_RPC, true);
    assert_eq!(
        sut.view().dapp_add.expect("the sheet").phase,
        NetDappAddPhase::Ready
    );
    sut
}

// ===========================================================================
// The browser: routing, answers, the record
// ===========================================================================

#[test]
fn a_chain_the_wallet_has_is_still_a_switch() {
    let mut sut = browser();
    let ops = add(&mut sut, "1", full_params(8453));
    assert!(forwarded(&ops).is_none(), "no sheet for a chain Vela has");
    assert_eq!(the_answer(&ops)["result"], Value::Null);
    assert_eq!(chain_changed(&ops), vec![json!("0x2105")]);
    assert_eq!(row(&mut sut, "1").class, DbrRequestClass::Local);
}

#[test]
fn an_unknown_chain_opens_the_sheet_and_waits_for_it() {
    let mut sut = browser();
    let ops = add(&mut sut, "1", full_params(NEW_CHAIN));
    let ask = forwarded(&ops).expect("forwarded to the sheet");
    assert_eq!(ask.chain_id, NEW_CHAIN);
    assert_eq!(ask.chain_name.as_deref(), Some("Testland"));
    assert_eq!(ask.native_symbol.as_deref(), Some("TST"));
    assert_eq!(ask.rpc_urls, vec![SITE_RPC.to_owned()]);
    assert_eq!(
        ask.explorer_url.as_deref(),
        Some("https://scan.site.example")
    );
    assert!(
        answers(&ops).iter().all(|m| m["dir"] != "res"),
        "the page waits for the person"
    );
    let view = sut.view();
    let adding = view.adding_network.expect("the browser knows the sheet");
    assert_eq!((adding.tab.as_str(), adding.id.as_str()), ("t1", "1"));
    assert!(view.tabs[0].busy, "the asking tab is never suspended");
    let row = row(&mut sut, "1");
    assert_eq!(row.class, DbrRequestClass::Consent);
    assert_eq!(row.outcome, DbrOutcome::Open);
}

#[test]
fn a_second_request_while_the_sheet_is_open_is_32002() {
    let mut sut = browser();
    add(&mut sut, "1", full_params(NEW_CHAIN));
    let ops = add(&mut sut, "2", full_params(NEW_CHAIN + 1));
    assert!(forwarded(&ops).is_none());
    assert_eq!(the_answer(&ops)["error"]["code"], json!(-32002));
    let second = row(&mut sut, "2");
    assert_eq!(second.code, Some(-32002));
    assert_eq!(second.layer, Some(DbrLayer::Wallet));
    assert_eq!(second.reason, Some(DbrReason::ConsentBusy));
    // The first is untouched.
    assert_eq!(row(&mut sut, "1").outcome, DbrOutcome::Open);
}

#[test]
fn added_switches_the_site_fires_chain_changed_and_answers_null() {
    let mut sut = browser();
    add(&mut sut, "1", full_params(NEW_CHAIN));
    let ops = answered(
        &mut sut,
        "1",
        DappAddOutcome::Added {
            chain_id: NEW_CHAIN,
        },
    );
    assert!(ops.contains(&DOp::WriteSiteChain {
        origin: DAPP.to_owned(),
        chain_id: NEW_CHAIN,
    }));
    assert_eq!(chain_changed(&ops), vec![json!("0x1e61")]);
    assert_eq!(the_answer(&ops)["result"], Value::Null);
    assert!(sut.view().adding_network.is_none());
    let chain = the_answer(&ask(&mut sut, "2", "eth_chainId", json!([])));
    assert_eq!(chain["result"], json!("0x1e61"));
    // And it is one of the wallet's networks now: a switch back and forth
    // needs no sheet.
    ask(
        &mut sut,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x1"}]),
    );
    let back = ask(
        &mut sut,
        "4",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x1e61"}]),
    );
    assert_eq!(the_answer(&back)["result"], Value::Null);
    let row = row(&mut sut, "1");
    assert_eq!(row.outcome, DbrOutcome::Answered);
    assert_eq!(row.ended_ms, Some(T0 + 4_000.0));
}

#[test]
fn declined_is_4001_and_nothing_changes() {
    let mut sut = browser();
    add(&mut sut, "1", full_params(NEW_CHAIN));
    let ops = answered(&mut sut, "1", DappAddOutcome::Declined);
    assert_eq!(the_answer(&ops)["error"]["code"], json!(4001));
    assert!(chain_changed(&ops).is_empty());
    assert!(!ops
        .iter()
        .any(|op| matches!(op, DOp::WriteSiteChain { .. })));
    let row = row(&mut sut, "1");
    assert_eq!(row.layer, Some(DbrLayer::Sheet));
    assert_eq!(row.reason, Some(DbrReason::RejectedByPerson));
    let chain = the_answer(&ask(&mut sut, "2", "eth_chainId", json!([])));
    assert_eq!(chain["result"], json!("0x1"), "still where it was");
}

#[test]
fn not_compatible_and_bad_rpc_answer_their_own_codes() {
    let mut sut = browser();
    add(&mut sut, "1", full_params(NEW_CHAIN));
    let ops = answered(&mut sut, "1", DappAddOutcome::NotCompatible);
    let answer = the_answer(&ops);
    assert_eq!(answer["error"]["code"], json!(4902));
    assert!(answer["error"]["message"]
        .as_str()
        .unwrap()
        .contains("not compatible with Vela Wallet"));
    let row1 = row(&mut sut, "1");
    assert_eq!(
        (row1.layer, row1.reason),
        (Some(DbrLayer::Wallet), Some(DbrReason::NotCompatible))
    );
    assert_eq!(
        DbrReason::NotCompatible.key(),
        "addToken.errorNotCompatible"
    );

    add(&mut sut, "2", full_params(NEW_CHAIN));
    let ops = answered(&mut sut, "2", DappAddOutcome::BadRpc);
    assert_eq!(the_answer(&ops)["error"]["code"], json!(-32602));
    let row2 = row(&mut sut, "2");
    assert_eq!(
        (row2.layer, row2.reason),
        (Some(DbrLayer::Wallet), Some(DbrReason::BadRpc))
    );
    // Each is worth the status line's attention; a decline is not.
    assert!(sut.view().tabs[0].last_failure.is_some());
}

#[test]
fn the_page_leaving_closes_the_sheet_and_a_late_answer_is_ignored() {
    let mut sut = browser();
    add(&mut sut, "1", full_params(NEW_CHAIN));
    let ops = sut.dispatch(page("t1", json!({"t":"hello","doc":"d2"})));
    assert!(ops.contains(&DOp::CancelAddNetwork {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
    }));
    assert_eq!(the_answer(&ops)["error"]["code"], json!(4900));
    assert!(sut.view().adding_network.is_none());
    let late = answered(
        &mut sut,
        "1",
        DappAddOutcome::Added {
            chain_id: NEW_CHAIN,
        },
    );
    assert!(answers(&late).is_empty(), "nothing reaches the new page");
    assert!(!late
        .iter()
        .any(|op| matches!(op, DOp::WriteSiteChain { .. })));
}

#[test]
fn a_malformed_add_is_32602_before_any_sheet() {
    let mut sut = browser();
    let six = json!([{"chainId":"0x1e61","nativeCurrency":{"name":"x","symbol":"X","decimals":6}}]);
    let ops = add(&mut sut, "1", six);
    assert!(forwarded(&ops).is_none());
    assert_eq!(the_answer(&ops)["error"]["code"], json!(-32602));
    let ops = add(
        &mut sut,
        "2",
        json!([{"chainId":"0x1e61","rpcUrls":"https://x"}]),
    );
    assert_eq!(the_answer(&ops)["error"]["code"], json!(-32602));
    let ops = add(&mut sut, "3", json!([{"chainId":"nope"}]));
    assert_eq!(the_answer(&ops)["error"]["code"], json!(-32602));
    assert_eq!(row(&mut sut, "1").reason, Some(DbrReason::BadParams));
}

// ===========================================================================
// Reading the page's ask (pure)
// ===========================================================================

#[test]
fn a_page_rpc_must_be_https_or_an_http_address_the_debug_rule_allows() {
    assert!(usable_rpc_url("https://rpc.example", false));
    assert!(usable_rpc_url("https://rpc.example:8443/v1?key=abc", false));
    assert!(!usable_rpc_url("http://rpc.example", false), "public http");
    assert!(
        !usable_rpc_url("http://rpc.example", true),
        "public http, debug or not"
    );
    assert!(usable_rpc_url("http://127.0.0.1:8545", false), "loopback");
    assert!(!usable_rpc_url("http://192.168.1.20:8545", false));
    assert!(
        usable_rpc_url("http://192.168.1.20:8545", true),
        "LAN, debug mode"
    );
    assert!(!usable_rpc_url("wss://rpc.example", false));
    assert!(!usable_rpc_url("file:///etc/passwd", true));
    assert!(!usable_rpc_url("https://user:pass@rpc.example", false));
    assert!(!usable_rpc_url("https://rpc .example", false));

    let ask = add_chain_ask(
        &json!([{
            "chainId": 7777,
            "chainName": "  Spoof\u{202E}ed\u{0007} chain  ",
            "rpcUrls": ["http://rpc.example", "https://a.example", "https://a.example",
                         "https://b.example", "https://c.example", "https://d.example",
                         "https://e.example"],
            "blockExplorerUrls": ["http://scan.example", "https://scan.example"],
        }]),
        false,
    )
    .expect("a usable ask");
    assert_eq!(ask.chain_name.as_deref(), Some("Spoofed chain"));
    assert_eq!(ask.refused_rpc_urls, 1);
    assert_eq!(ask.rpc_urls.len(), 4, "the first four usable, deduped");
    assert_eq!(ask.rpc_urls[0], "https://a.example");
    assert_eq!(ask.explorer_url.as_deref(), Some("https://scan.example"));
    assert_eq!(ask.native_symbol, None);
}

#[test]
fn every_outcome_has_one_answer_for_every_client() {
    assert_eq!(
        add_outcome_error(&DappAddOutcome::Added { chain_id: 1 }, 1),
        None
    );
    let code = |outcome: DappAddOutcome| add_outcome_error(&outcome, NEW_CHAIN).unwrap().0;
    assert_eq!(code(DappAddOutcome::Declined), 4001);
    assert_eq!(code(DappAddOutcome::NotCompatible), 4902);
    assert_eq!(code(DappAddOutcome::BadRpc), -32602);
    assert_eq!(code(DappAddOutcome::Busy), -32002);
}

// ===========================================================================
// The sheet: the catalog first, the page's RPC only when it must
// ===========================================================================

#[test]
fn the_catalogs_name_coin_and_rpc_win_over_the_pages() {
    let sut = ready_from_catalog();
    let view = sut.view().dapp_add.expect("the sheet");
    assert_eq!(view.host, "dapp.example");
    assert_eq!(view.name, "Catalog Chain");
    assert_eq!(view.native_symbol, "CAT");
    assert_eq!(view.rpc_host.as_deref(), Some("rpc.catalog.example"));
    assert_eq!(view.explorer_host.as_deref(), Some("scan.catalog.example"));
    assert!(!view.from_site);
    assert!(view.can_add);
    assert!(view.compat.expect("the check").multi_key_ready);
}

#[test]
fn an_unknown_chain_uses_the_pages_rpc_only_if_it_answers_for_the_chain() {
    let mut sut = admin();
    requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    let ops = catalog(&mut sut, None);
    assert_eq!(
        ops,
        vec![NOp::ProbeRpc {
            url: SITE_RPC.to_owned()
        }]
    );
    let sheet = sut.view().dapp_add.expect("the sheet");
    assert!(sheet.from_site);
    assert_eq!(sheet.name, "Site Chain");
    assert_eq!(sheet.phase, NetDappAddPhase::Checking);
    // It answers for another chain: proof, not silence.
    assert!(probed(&mut sut, SITE_RPC, Some(1)).is_empty());
    let sheet = sut.view().dapp_add.expect("the sheet");
    assert_eq!(sheet.phase, NetDappAddPhase::WrongRpc);
    assert_eq!(sheet.reported_chain_id, Some(1));
    assert!(!sheet.can_add);
    let ops = sut.dispatch(NEvent::DappAddDeclined);
    assert_eq!(settled(&ops), Some(DappAddOutcome::BadRpc));
    assert!(writes(&ops).is_none());
}

#[test]
fn a_pages_rpc_that_names_the_chain_is_checked_like_any_other() {
    let mut sut = admin();
    requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    catalog(&mut sut, None);
    let ops = probed(&mut sut, SITE_RPC, Some(NEW_CHAIN));
    assert_eq!(
        ops.len(),
        REQUIRED_CONTRACTS.len() + 1,
        "the same contracts"
    );
    contracts(&mut sut, SITE_RPC, true);
    let ops = sut.dispatch(NEvent::DappAddApproved {
        now_iso: NOW_ISO.to_owned(),
    });
    let saved = writes(&ops).expect("saved");
    let network = saved.iter().find(|n| n.chain_id == NEW_CHAIN).unwrap();
    assert_eq!(network.rpc_url, SITE_RPC);
    assert_eq!(network.display_name, "Site Chain");
    assert_eq!(network.native_symbol, "SITE");
    assert_eq!(
        settled(&ops),
        Some(DappAddOutcome::Added {
            chain_id: NEW_CHAIN
        })
    );
}

#[test]
fn no_usable_rpc_from_the_page_ends_the_check() {
    let mut sut = admin();
    let mut ask = site_ask(NEW_CHAIN, &[]);
    ask.refused_rpc_urls = 1;
    requested(&mut sut, ask);
    assert!(catalog(&mut sut, None).is_empty(), "nothing to probe");
    assert_eq!(sut.view().dapp_add.unwrap().phase, NetDappAddPhase::NoRpc);
    let ops = sut.dispatch(NEvent::DappAddDeclined);
    assert_eq!(settled(&ops), Some(DappAddOutcome::BadRpc));
}

#[test]
fn an_incompatible_chain_is_refused_and_nothing_is_added() {
    let mut sut = admin();
    requested(&mut sut, site_ask(NEW_CHAIN, &[]));
    catalog(&mut sut, Some(catalog_entry()));
    probed(&mut sut, CATALOG_RPC, Some(NEW_CHAIN));
    contracts(&mut sut, CATALOG_RPC, false);
    let sheet = sut.view().dapp_add.unwrap();
    assert_eq!(sheet.phase, NetDappAddPhase::NotCompatible);
    assert!(!sheet.can_add);
    // "Add" cannot act on it.
    let ops = sut.dispatch(NEvent::DappAddApproved {
        now_iso: NOW_ISO.to_owned(),
    });
    assert!(ops.is_empty());
    let ops = sut.dispatch(NEvent::DappAddDeclined);
    assert_eq!(settled(&ops), Some(DappAddOutcome::NotCompatible));
    assert!(writes(&ops).is_none());
    assert!(!sut.view().networks.iter().any(|n| n.chain_id == NEW_CHAIN));
}

#[test]
fn approve_saves_through_the_settings_path_and_settles_added() {
    let mut sut = ready_from_catalog();
    let ops = sut.dispatch(NEvent::DappAddApproved {
        now_iso: NOW_ISO.to_owned(),
    });
    let saved = writes(&ops).expect("vela.customNetworks written");
    let network = saved.iter().find(|n| n.chain_id == NEW_CHAIN).unwrap();
    assert_eq!(network.id, format!("custom-{NEW_CHAIN}"));
    assert_eq!(network.display_name, "Catalog Chain");
    assert_eq!(network.rpc_url, CATALOG_RPC);
    assert_eq!(network.added_at_iso, NOW_ISO);
    assert_eq!(
        settled(&ops),
        Some(DappAddOutcome::Added {
            chain_id: NEW_CHAIN
        })
    );
    let view = sut.view();
    assert!(view.dapp_add.is_none());
    assert_eq!(view.last_added_chain_id, Some(NEW_CHAIN));
    assert!(view
        .networks
        .iter()
        .any(|n| n.chain_id == NEW_CHAIN && n.is_custom));
}

#[test]
fn declining_a_compatible_chain_settles_declined() {
    let mut sut = ready_from_catalog();
    let ops = sut.dispatch(NEvent::DappAddDeclined);
    assert_eq!(settled(&ops), Some(DappAddOutcome::Declined));
    assert!(writes(&ops).is_none());
    assert!(sut.view().dapp_add.is_none());
}

#[test]
fn no_answer_is_unable_to_verify_and_retry_runs_it_again() {
    let mut sut = admin();
    requested(&mut sut, site_ask(NEW_CHAIN, &[]));
    catalog(&mut sut, Some(catalog_entry()));
    assert!(probed(&mut sut, CATALOG_RPC, None).is_empty());
    assert_eq!(
        sut.view().dapp_add.unwrap().phase,
        NetDappAddPhase::CheckFailed
    );
    let ops = sut.dispatch(NEvent::DappAddRetried);
    assert_eq!(
        ops,
        vec![NOp::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
    assert_eq!(
        sut.view().dapp_add.unwrap().phase,
        NetDappAddPhase::Checking
    );
    // Closing it then is the person's decline.
    let ops = sut.dispatch(NEvent::DappAddDeclined);
    assert_eq!(settled(&ops), Some(DappAddOutcome::Declined));
}

#[test]
fn a_chain_the_wallet_already_has_settles_added_without_a_sheet() {
    let mut sut = admin_with(vec![NetCustomNetwork {
        id: format!("custom-{NEW_CHAIN}"),
        display_name: "Mine".to_owned(),
        chain_id: NEW_CHAIN,
        icon_label: "M".to_owned(),
        icon_color: "#888888".to_owned(),
        icon_bg: "#F0F0F0".to_owned(),
        logo_url: String::new(),
        is_l2: false,
        rpc_url: CATALOG_RPC.to_owned(),
        explorer_url: String::new(),
        bundler_url: String::new(),
        native_symbol: "M".to_owned(),
        added_at_iso: NOW_ISO.to_owned(),
    }]);
    let ops = requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    assert_eq!(
        settled(&ops),
        Some(DappAddOutcome::Added {
            chain_id: NEW_CHAIN
        })
    );
    assert!(sut.view().dapp_add.is_none());
}

#[test]
fn the_settings_wizard_is_left_as_it_was() {
    let mut sut = admin();
    sut.dispatch(NEvent::ChainSelected {
        chain_id: 4242,
        keep_custom_rpc: false,
    });
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Resolving);
    requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    catalog(&mut sut, Some(catalog_entry()));
    let view = sut.view();
    assert_eq!(view.wizard.phase, NetWizardPhase::Resolving, "untouched");
    assert_eq!(view.dapp_add.unwrap().name, "Catalog Chain");
    // The wizard's own answer still lands in the wizard.
    let ops = sut.resolve_matching(
        |op| matches!(op, NOp::FetchChainInfo { chain_id: 4242 }),
        NRes::ChainInfo {
            chain_id: 4242,
            data: None,
        },
    );
    assert!(ops.is_empty());
    assert_eq!(sut.view().wizard.phase, NetWizardPhase::Error);
    assert_eq!(
        sut.view().dapp_add.unwrap().phase,
        NetDappAddPhase::Checking
    );
}

#[test]
fn a_second_client_is_told_busy_and_a_cancel_settles_nothing() {
    let mut sut = admin();
    requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    let ops = sut.dispatch(NEvent::DappAddRequested {
        tab: "t2".to_owned(),
        id: "b".to_owned(),
        origin: DAPP.to_owned(),
        ask: site_ask(NEW_CHAIN, &[SITE_RPC]),
    });
    assert_eq!(settled(&ops), Some(DappAddOutcome::Busy));
    let ops = sut.dispatch(NEvent::DappAddCancelled {
        tab: "t1".to_owned(),
        id: "a1".to_owned(),
    });
    assert!(settled(&ops).is_none());
    assert!(sut.view().dapp_add.is_none());
    // The catalog's late answer finds nothing to repaint.
    assert!(catalog(&mut sut, Some(catalog_entry())).is_empty());
}

#[test]
fn a_request_before_the_ledger_is_read_begins_once_it_is() {
    let mut sut = Admin::new();
    assert_eq!(sut.dispatch(NEvent::Started), vec![NOp::ReadStore]);
    let ops = requested(&mut sut, site_ask(NEW_CHAIN, &[SITE_RPC]));
    assert!(ops.is_empty(), "no dedup without the ledger");
    assert_eq!(
        sut.view().dapp_add.unwrap().phase,
        NetDappAddPhase::Checking
    );
    let ops = sut.resolve(NRes::StoreLoaded {
        custom_networks: vec![],
        network_configs: vec![],
        endpoints: NetStoredEndpoints::default(),
        provider_keys: NetProviderKeys::default(),
    });
    assert_eq!(
        ops,
        vec![NOp::FetchChainInfo {
            chain_id: NEW_CHAIN
        }]
    );
}

// ===========================================================================
// Both machines, the way a shell carries one to the other
// ===========================================================================

/// Carry `forward_to_add_network` into the sheet's machine.
fn carry_forward(ops: &[DOp], admin: &mut Admin) -> Vec<NOp> {
    let (tab, id, origin, ask) = ops
        .iter()
        .find_map(|op| match op {
            DOp::ForwardToAddNetwork {
                tab,
                id,
                origin,
                ask,
            } => Some((tab.clone(), id.clone(), origin.clone(), ask.clone())),
            _ => None,
        })
        .expect("forwarded");
    admin.dispatch(NEvent::DappAddRequested {
        tab,
        id,
        origin,
        ask,
    })
}

/// Carry `dapp_add_settled` back to the browser.
fn carry_back(ops: &[NOp], browser: &mut Browser) -> Vec<DOp> {
    let (tab, id, outcome) = ops
        .iter()
        .find_map(|op| match op {
            NOp::DappAddSettled { tab, id, outcome } => {
                Some((tab.clone(), id.clone(), outcome.clone()))
            }
            _ => None,
        })
        .expect("settled");
    browser.dispatch(DEvent::AddNetworkAnswered {
        tab,
        id,
        outcome,
        now_ms: T0 + 9_000.0,
    })
}

#[test]
fn end_to_end_approve_adds_switches_and_answers_null() {
    let mut page_side = browser();
    let mut sheet = admin();
    let ops = add(&mut page_side, "1", full_params(NEW_CHAIN));
    carry_forward(&ops, &mut sheet);
    catalog(&mut sheet, Some(catalog_entry()));
    probed(&mut sheet, CATALOG_RPC, Some(NEW_CHAIN));
    contracts(&mut sheet, CATALOG_RPC, true);
    let ops = sheet.dispatch(NEvent::DappAddApproved {
        now_iso: NOW_ISO.to_owned(),
    });
    assert!(writes(&ops).is_some(), "persisted the Settings way");
    let ops = carry_back(&ops, &mut page_side);
    assert_eq!(chain_changed(&ops), vec![json!("0x1e61")]);
    assert_eq!(the_answer(&ops)["result"], Value::Null);
    assert_eq!(row(&mut page_side, "1").outcome, DbrOutcome::Answered);
}

#[test]
fn end_to_end_an_incompatible_chain_answers_4902_and_adds_nothing() {
    let mut page_side = browser();
    let mut sheet = admin();
    let ops = add(&mut page_side, "1", full_params(NEW_CHAIN));
    carry_forward(&ops, &mut sheet);
    catalog(&mut sheet, Some(catalog_entry()));
    probed(&mut sheet, CATALOG_RPC, Some(NEW_CHAIN));
    contracts(&mut sheet, CATALOG_RPC, false);
    let ops = sheet.dispatch(NEvent::DappAddDeclined);
    assert!(writes(&ops).is_none());
    let ops = carry_back(&ops, &mut page_side);
    assert_eq!(the_answer(&ops)["error"]["code"], json!(4902));
    assert!(chain_changed(&ops).is_empty());
    let row = row(&mut page_side, "1");
    assert_eq!(row.reason, Some(DbrReason::NotCompatible));
}
