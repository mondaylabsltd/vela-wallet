//! r10 follow-ups — the in-app browser and the two network spaces (spec 105).
//!
//! - A page's switch (or add) to a network of the OTHER space is refused
//!   naming that space, in the sentence the extension worker says
//!   (`network_space::switch_refusal_message`, the shared
//!   `site-start-chain.json` `switches`), and the status line says it in the
//!   person's words — never "not in Vela's networks".
//! - A space switch moves every site whose network is outside the new space
//!   to that space's start network (the owner's spec-105 line: test value is
//!   never mistaken for real): the page hears `chainChanged`, the new chain is
//!   stored, and the site's next request that opens a sheet carries one line
//!   that says so.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation as Op, DbrShellResult as Res, DbrStoredSite, Event,
};
use vela_core::app::dapp_permissions::DpermGrant;
use vela_core::app::network_space::{NetworkSpace, SPACE_MOVED_KEY};
use vela_core::app::sign_request::SignResponsePayload;

type Sut = DomainDriver<DappBrowser>;

const T0: f64 = 1_754_700_000_000.0;
const DAPP: &str = "https://dapp.example";
const FAUCET: &str = "https://faucet.example";
const A1: &str = "0x1111111111111111111111111111111111111111";

const MAINNET: [u32; 3] = [1, 100, 8453];
const TESTNET: [u32; 3] = [11_155_111, 84_532, 5_042_002];

fn known() -> Vec<u32> {
    MAINNET.iter().chain(TESTNET.iter()).copied().collect()
}

fn space(space: NetworkSpace) -> Event {
    Event::SpaceNetworksChanged {
        space,
        chain_ids: match space {
            NetworkSpace::Mainnet => MAINNET.to_vec(),
            NetworkSpace::Testnet => TESTNET.to_vec(),
        },
        known_chain_ids: known(),
    }
}

fn grant(origin: &str, chain_id: u32) -> DbrStoredSite {
    DbrStoredSite {
        origin: origin.to_owned(),
        grant: Some(DpermGrant {
            origin: origin.to_owned(),
            address: A1.to_owned(),
            chain_id,
            granted_at_ms: T0,
        }),
        chain_id: None,
    }
}

/// Started in the mainnet space, A1 active, these sites stored.
fn booted(sites: Vec<DbrStoredSite>) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(space(NetworkSpace::Mainnet));
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(vec![A1.to_owned()]),
    });
    sut.dispatch(Event::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
        space: false,
    });
    sut.resolve(Res::SitesListed {
        sites,
        multisig_batches_json: None,
    });
    sut
}

fn page(tab: &str, origin: &str, message: Value) -> Event {
    Event::PageMessage {
        tab: tab.to_owned(),
        frame_origin: origin.to_owned(),
        is_main_frame: true,
        message_json: message.to_string(),
        now_ms: 0.0,
    }
}

fn hello(sut: &mut Sut, tab: &str, doc: &str, origin: &str) {
    sut.dispatch(page(tab, origin, json!({"t":"hello","doc":doc})));
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

fn delivered(ops: &[Op]) -> Vec<(String, Value)> {
    ops.iter()
        .filter_map(|op| match op {
            Op::Deliver {
                tab, message_json, ..
            } => Some((tab.clone(), serde_json::from_str(message_json).unwrap())),
            _ => None,
        })
        .collect()
}

fn answer(ops: &[Op]) -> Value {
    let answers: Vec<Value> = delivered(ops)
        .into_iter()
        .filter(|(_, m)| m["dir"] == "res")
        .map(|(_, m)| m)
        .collect();
    assert_eq!(answers.len(), 1, "exactly one answer in {ops:?}");
    answers.into_iter().next().unwrap()
}

fn chain_changed(ops: &[Op]) -> Vec<(String, Value)> {
    delivered(ops)
        .into_iter()
        .filter(|(_, m)| m["dir"] == "evt" && m["event"] == "chainChanged")
        .map(|(tab, m)| (tab, m["data"].clone()))
        .collect()
}

fn chain_id(sut: &mut Sut, tab: &str, doc: &str, origin: &str, id: &str) -> Value {
    answer(&ask(sut, tab, doc, origin, id, "eth_chainId", json!([])))["result"].clone()
}

// ---------------------------------------------------------------------------
// The refusal
// ---------------------------------------------------------------------------

/// r10 (Safari, the in-app browsers): Arc Testnet asked for from the mainnet
/// space is the testnet space's network, said as that — the extension's
/// sentence word for word — and the status line names the space.
#[test]
fn a_switch_to_the_other_space_is_refused_naming_it() {
    let mut sut = booted(vec![grant(DAPP, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    let refused = answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x4cef52"}]),
    ));
    assert_eq!(refused["error"]["code"], json!(4902));
    assert_eq!(
        refused["error"]["message"],
        json!(
            "Chain 5042002 is one of Vela's testnet networks; Vela is showing its mainnet \
             networks. Switch to the testnet space in Vela to use it."
        )
    );
    let tab = sut.view().tabs.into_iter().find(|t| t.tab == "t1").unwrap();
    let note = tab.last_failure.expect("the status line");
    assert_eq!(
        note.key,
        "componentsUi.browserStatus.reason.otherSpaceTestnet"
    );
    // The site stays where it was.
    assert_eq!(chain_id(&mut sut, "t1", "d1", DAPP, "2"), json!("0x1"));

    // A chain Vela does not have at all is still "not in Vela's networks".
    let unknown = answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x67932"}]),
    ));
    assert_eq!(
        unknown["error"]["message"],
        json!("Chain 424242 is not in Vela's networks")
    );
    let tab = sut.view().tabs.into_iter().find(|t| t.tab == "t1").unwrap();
    assert_eq!(
        tab.last_failure.unwrap().key,
        "componentsUi.browserStatus.reason.unknownChain"
    );
}

/// An add of a network Vela has in the other space never reaches the add
/// sheet (it would have put a mainnet-space site on a testnet): refused the
/// same way. An add of one Vela lacks still opens the sheet (spec 100).
#[test]
fn an_add_of_the_other_spaces_network_is_refused_too() {
    let mut sut = booted(vec![grant(DAPP, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_addEthereumChain",
        json!([{"chainId":"0xaa36a7","chainName":"Sepolia","rpcUrls":["https://rpc.sepolia.org"]}]),
    );
    assert!(!ops
        .iter()
        .any(|op| matches!(op, Op::ForwardToAddNetwork { .. })));
    let refused = answer(&ops);
    assert_eq!(refused["error"]["code"], json!(4902));
    assert!(refused["error"]["message"]
        .as_str()
        .unwrap()
        .contains("one of Vela's testnet networks"));

    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "wallet_addEthereumChain",
        json!([{"chainId":"0x89"}]),
    );
    assert!(ops
        .iter()
        .any(|op| matches!(op, Op::ForwardToAddNetwork { ask, .. } if ask.chain_id == 137)));
}

/// A shell that has not said its space keeps today's answers.
#[test]
fn without_the_space_the_refusal_is_as_before() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(Event::NetworksChanged {
        chain_ids: MAINNET.to_vec(),
    });
    sut.dispatch(Event::AccountsUpdated {
        addresses: Some(vec![A1.to_owned()]),
    });
    sut.dispatch(Event::AccountSwitched {
        address: A1.to_owned(),
        now_ms: T0,
        space: false,
    });
    sut.resolve(Res::SitesListed {
        sites: vec![grant(DAPP, 1)],
        multisig_batches_json: None,
    });
    hello(&mut sut, "t1", "d1", DAPP);
    let refused = answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x4cef52"}]),
    ));
    assert_eq!(
        refused["error"]["message"],
        json!("Chain 5042002 is not in Vela's networks")
    );
}

// ---------------------------------------------------------------------------
// The space switch
// ---------------------------------------------------------------------------

/// The owner's spec-105 line: a mainnet-space site never stays on a testnet
/// when the person switches to the testnet space — and back. The page hears
/// `chainChanged`, the chain is stored, a site already in the space stays.
#[test]
fn a_space_switch_moves_the_sites_outside_it() {
    let mut sut = booted(vec![grant(DAPP, 8453), grant(FAUCET, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    assert_eq!(chain_id(&mut sut, "t1", "d1", DAPP, "1"), json!("0x2105"));

    let ops = sut.dispatch(space(NetworkSpace::Testnet));
    // Both sites were mainnet's: both move to the testnet space's start (no
    // network used there yet, nothing held: its first).
    let stored: Vec<(String, u32)> = ops
        .iter()
        .filter_map(|op| match op {
            Op::WriteSiteChain { origin, chain_id } => Some((origin.clone(), *chain_id)),
            _ => None,
        })
        .collect();
    assert_eq!(
        stored,
        vec![
            (DAPP.to_owned(), 11_155_111),
            (FAUCET.to_owned(), 11_155_111)
        ]
    );
    // The open page hears it, once.
    assert_eq!(
        chain_changed(&ops),
        vec![("t1".to_owned(), json!("0xaa36a7"))]
    );
    assert_eq!(chain_id(&mut sut, "t1", "d1", DAPP, "2"), json!("0xaa36a7"));

    // The page puts itself on Arc: the network last used in the testnet
    // space. Back to mainnet and to testnet again: Arc is where it lands.
    let ok = answer(&ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x4cef52"}]),
    ));
    assert_eq!(ok["result"], Value::Null);
    let ops = sut.dispatch(space(NetworkSpace::Mainnet));
    assert_eq!(chain_changed(&ops), vec![("t1".to_owned(), json!("0x1"))]);
    let ops = sut.dispatch(space(NetworkSpace::Testnet));
    assert_eq!(
        chain_changed(&ops),
        vec![("t1".to_owned(), json!("0x4cef52"))]
    );

    // The same space said again (a network added, a list re-sent): nothing
    // moves.
    let ops = sut.dispatch(space(NetworkSpace::Testnet));
    assert!(chain_changed(&ops).is_empty(), "{ops:?}");
    assert!(!ops.iter().any(|op| matches!(op, Op::WriteSiteChain { .. })));
}

/// The moved site's next request that opens a sheet says it once — the
/// signing sheet here; a read says nothing and leaves it standing.
#[test]
fn a_moved_sites_next_request_carries_one_line() {
    let mut sut = booted(vec![grant(DAPP, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    sut.dispatch(space(NetworkSpace::Testnet));
    // A read opens no sheet.
    chain_id(&mut sut, "t1", "d1", DAPP, "1");
    let ops = ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x68656c6c6f", A1]),
    );
    assert!(ops
        .iter()
        .any(|op| matches!(op, Op::ForwardToSigning { .. })));
    let note = sut
        .view()
        .signing
        .and_then(|s| s.space_note)
        .expect("the line");
    assert_eq!(note.chain_id, 11_155_111);
    assert_eq!(note.key, SPACE_MOVED_KEY);
    let wire = serde_json::to_value(sut.view()).unwrap();
    assert_eq!(wire["signing"]["space_note"]["key"], json!(SPACE_MOVED_KEY));
}

/// The line is said once: the request after it carries none.
#[test]
fn the_line_is_said_once() {
    let mut sut = booted(vec![grant(DAPP, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    sut.dispatch(space(NetworkSpace::Testnet));
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "personal_sign",
        json!(["0x00", A1]),
    );
    assert!(sut.view().signing.unwrap().space_note.is_some());
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x01", A1]),
    );
    assert_eq!(sut.view().queued_signing, 1);
    // The first is answered; the second, next on the sheet, says nothing.
    sut.dispatch(Event::SigningAnswered {
        tab: "t1".to_owned(),
        id: "1".to_owned(),
        payload: SignResponsePayload::Ok {
            result: Some("0xabc".into()),
        },
        user_op_hash: None,
        now_ms: 0.0,
    });
    let next = sut.view().signing.expect("the second");
    assert_eq!(next.id, "2");
    assert!(next.space_note.is_none());
    let wire = serde_json::to_value(sut.view()).unwrap();
    assert!(
        wire["signing"].get("space_note").is_none(),
        "absent on the wire"
    );
}

/// The page choosing a network itself after the move is its own choice:
/// nothing is said of the move any more.
#[test]
fn a_pages_own_switch_says_the_move_is_over() {
    let mut sut = booted(vec![grant(DAPP, 1)]);
    hello(&mut sut, "t1", "d1", DAPP);
    sut.dispatch(space(NetworkSpace::Testnet));
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x14a34"}]),
    );
    ask(
        &mut sut,
        "t1",
        "d1",
        DAPP,
        "2",
        "personal_sign",
        json!(["0x00", A1]),
    );
    assert!(sut.view().signing.unwrap().space_note.is_none());
}

/// A space switch that comes before the stored sites were read moves them
/// once they are.
#[test]
fn a_space_switch_before_the_sites_are_read_still_moves_them() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Start);
    sut.dispatch(space(NetworkSpace::Mainnet));
    sut.dispatch(space(NetworkSpace::Testnet));
    let ops = sut.resolve(Res::SitesListed {
        sites: vec![grant(DAPP, 8453)],
        multisig_batches_json: None,
    });
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::WriteSiteChain { origin, chain_id } if origin == DAPP && *chain_id == 11_155_111
    )));
}

/// A page on a site with no network of its own follows the space's start —
/// its answer moved, so it is told.
#[test]
fn a_page_with_no_network_of_its_own_hears_the_space_move() {
    let mut sut = booted(Vec::new());
    hello(&mut sut, "t1", "d1", DAPP);
    assert_eq!(chain_id(&mut sut, "t1", "d1", DAPP, "1"), json!("0x1"));
    let ops = sut.dispatch(space(NetworkSpace::Testnet));
    assert_eq!(
        chain_changed(&ops),
        vec![("t1".to_owned(), json!("0xaa36a7"))]
    );
    // Nothing is stored for it: it was never given a network.
    assert!(!ops.iter().any(|op| matches!(op, Op::WriteSiteChain { .. })));
}
