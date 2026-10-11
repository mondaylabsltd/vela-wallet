//! A dev space has its own sites (the 2026-10-11 phone walk).
//!
//! The parallel space is the real app with a test wallet's keys. On a phone
//! it shares the device's store with the person's own wallet, and going in
//! is an account switch — so every site the person had connected was
//! re-pinned to the test wallet on the way in, and dropped when the test
//! wallet's record was removed on the way out. The owner's own connection
//! went with them, every walk.
//!
//! The rule: entering or leaving a dev space never touches a grant of the
//! person's accounts. The space has its own set; the person's comes back
//! exactly. These tests walk it the way a shell drives it — events in,
//! operations out — against a store that only ever changes by the
//! operations the machine emits, and compare that store byte for byte.

#![cfg(feature = "crux")]

mod support;

use std::collections::BTreeMap;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::dapp_browser::{
    DappBrowser, DbrOperation as Op, DbrShellResult as Res, DbrStoredSite, Event,
};
use vela_core::app::dapp_permissions::{origin_lock, DpermGrant, OriginLock};
use vela_core::app::dapp_record::DbrReason;

type Sut = DomainDriver<DappBrowser>;

const T0: f64 = 1_754_700_000_000.0;
const DAPP: &str = "https://dapp.example";
const OTHER: &str = "https://other.example";
/// The on-device test dApp: connected in the person's wallet AND in the
/// space, which is what every device walk does.
const LOCAL: &str = "http://127.0.0.1:8137";
/// The person's two accounts.
const A1: &str = "0x1111111111111111111111111111111111111111";
const A2: &str = "0x2222222222222222222222222222222222222222";
/// The space's test wallet.
const FIXTURE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

/// The site store as a shell keeps it: `vela.perm.<origin>` and
/// `vela.chain.<origin>`. It changes only by the machine's own operations.
#[derive(Clone, Default, PartialEq, Debug)]
struct Store {
    kv: BTreeMap<String, String>,
    /// Every operation that would have written to it, in order.
    writes: Vec<Op>,
}

impl Store {
    fn with(grants: &[DpermGrant], chains: &[(&str, u32)]) -> Self {
        let mut store = Self::default();
        for grant in grants {
            store.kv.insert(
                format!("vela.perm.{}", grant.origin),
                serde_json::to_string(grant).unwrap(),
            );
        }
        for (origin, chain) in chains {
            store
                .kv
                .insert(format!("vela.chain.{origin}"), chain.to_string());
        }
        store
    }

    fn apply(&mut self, ops: &[Op]) {
        for op in ops {
            match op {
                Op::WriteGrant { grant } => {
                    self.kv.insert(
                        format!("vela.perm.{}", grant.origin),
                        serde_json::to_string(grant).unwrap(),
                    );
                }
                Op::RemoveGrant { origin } => {
                    self.kv.remove(&format!("vela.perm.{origin}"));
                }
                Op::WriteSiteChain { origin, chain_id } => {
                    self.kv
                        .insert(format!("vela.chain.{origin}"), chain_id.to_string());
                }
                _ => continue,
            }
            self.writes.push(op.clone());
        }
    }

    /// Every byte of it, in key order.
    fn bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.kv).unwrap()
    }

    fn sites(&self) -> Vec<DbrStoredSite> {
        let mut sites: BTreeMap<String, DbrStoredSite> = BTreeMap::new();
        for (key, value) in &self.kv {
            if let Some(origin) = key.strip_prefix("vela.perm.") {
                sites
                    .entry(origin.to_owned())
                    .or_insert_with(|| blank(origin))
                    .grant = serde_json::from_str(value).ok();
            } else if let Some(origin) = key.strip_prefix("vela.chain.") {
                sites
                    .entry(origin.to_owned())
                    .or_insert_with(|| blank(origin))
                    .chain_id = value.parse().ok();
            }
        }
        sites.into_values().collect()
    }
}

fn blank(origin: &str) -> DbrStoredSite {
    DbrStoredSite {
        origin: origin.to_owned(),
        grant: None,
        chain_id: None,
    }
}

fn grant(origin: &str, address: &str, chain_id: u32) -> DpermGrant {
    DpermGrant {
        origin: origin.to_owned(),
        address: address.to_owned(),
        chain_id,
        granted_at_ms: T0,
    }
}

/// The person's wallet as the owner's phone had it: three connected sites,
/// one of them on a network it switched itself to.
fn owners_store() -> Store {
    Store::with(
        &[
            grant(DAPP, A1, 100),
            grant(OTHER, A1, 1),
            grant(LOCAL, A1, 100),
        ],
        &[(OTHER, 8453)],
    )
}

/// A browser and its store, with the events a shell sends.
struct Walk {
    sut: Sut,
    store: Store,
    /// Every operation the machine emitted, in order.
    ops: Vec<Op>,
}

impl Walk {
    /// Started; the store listed; the person's two accounts, A1 in front.
    fn booted(store: Store) -> Self {
        let mut walk = Self::cold(store);
        walk.accounts(&[A1, A2], A1, false);
        walk.list();
        walk
    }

    /// Started, nothing answered yet.
    fn cold(store: Store) -> Self {
        let mut sut = Sut::new();
        assert_eq!(sut.dispatch(Event::Start), vec![Op::ListSites]);
        sut.dispatch(Event::NetworksChanged {
            chain_ids: vec![1, 100, 8453],
        });
        Self {
            sut,
            store,
            ops: Vec::new(),
        }
    }

    /// The store answers the list.
    fn list(&mut self) {
        let sites = self.store.sites();
        let ops = self
            .sut
            .resolve_matching(|op| *op == Op::ListSites, Res::SitesListed { sites });
        self.took(ops);
    }

    fn took(&mut self, ops: Vec<Op>) -> Vec<Op> {
        self.store.apply(&ops);
        self.ops.extend(ops.iter().cloned());
        ops
    }

    fn send(&mut self, event: Event) -> Vec<Op> {
        let ops = self.sut.dispatch(event);
        self.took(ops)
    }

    /// What every shell sends when the session changes: the addresses, then
    /// the one in front.
    fn accounts(&mut self, addresses: &[&str], active: &str, space: bool) -> Vec<Op> {
        let mut ops = self.updated(addresses);
        ops.extend(self.switched(active, space));
        ops
    }

    fn updated(&mut self, addresses: &[&str]) -> Vec<Op> {
        self.send(Event::AccountsUpdated {
            addresses: Some(addresses.iter().map(|a| (*a).to_owned()).collect()),
        })
    }

    fn switched(&mut self, active: &str, space: bool) -> Vec<Op> {
        self.send(Event::AccountSwitched {
            address: active.to_owned(),
            now_ms: T0 + 60_000.0,
            space,
        })
    }

    /// Into the space, the way Android and iOS do it: the test wallet's
    /// record is appended and made active.
    fn enter(&mut self) -> Vec<Op> {
        self.accounts(&[A1, A2, FIXTURE], FIXTURE, true)
    }

    /// Out of it: the record is removed and the person is back on A1.
    fn leave(&mut self) -> Vec<Op> {
        self.accounts(&[A1, A2], A1, false)
    }

    fn page(&mut self, tab: &str, origin: &str, message: Value) -> Vec<Op> {
        self.send(Event::PageMessage {
            tab: tab.to_owned(),
            frame_origin: origin.to_owned(),
            is_main_frame: true,
            message_json: message.to_string(),
            now_ms: T0,
        })
    }

    fn hello(&mut self, tab: &str, doc: &str, origin: &str) -> Vec<Op> {
        self.page(tab, origin, json!({"t":"hello","doc":doc}))
    }

    fn ask(
        &mut self,
        tab: &str,
        doc: &str,
        origin: &str,
        id: &str,
        method: &str,
        params: Value,
    ) -> Vec<Op> {
        self.page(
            tab,
            origin,
            json!({"t":"req","doc":doc,"id":id,"method":method,"params":params}),
        )
    }

    /// The page asks to connect and the person presses Connect.
    fn connect(&mut self, tab: &str, doc: &str, origin: &str, id: &str) -> Vec<Op> {
        let asked = self.ask(tab, doc, origin, id, "eth_requestAccounts", json!([]));
        assert!(answers(&asked).is_empty(), "a connect waits for the person");
        assert_eq!(
            self.sut.view().consent.map(|consent| consent.origin),
            Some(origin.to_owned())
        );
        self.send(Event::ConsentApproved { now_ms: T0 + 5.0 })
    }

    /// `eth_accounts` as the page reads it.
    fn accounts_of(&mut self, tab: &str, doc: &str, origin: &str, id: &str) -> Value {
        let ops = self.ask(tab, doc, origin, id, "eth_accounts", json!([]));
        let mut answers = answers(&ops);
        assert_eq!(answers.len(), 1, "one answer in {ops:?}");
        answers.remove(0)["result"].clone()
    }

    /// The view's sites, as bytes.
    fn sites_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(&self.sut.view().sites).unwrap()
    }

    fn connected(&self, tab: &str) -> Option<String> {
        self.sut
            .view()
            .tabs
            .into_iter()
            .find(|view| view.tab == tab)
            .and_then(|view| view.connected_address)
    }
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

fn answers(ops: &[Op]) -> Vec<Value> {
    delivered(ops)
        .into_iter()
        .filter(|(_, message)| message["dir"] == "res")
        .map(|(_, message)| message)
        .collect()
}

/// `(tab, event, data)` for every page event among `ops`.
fn events(ops: &[Op]) -> Vec<(String, String, Value)> {
    delivered(ops)
        .into_iter()
        .filter(|(_, message)| message["dir"] == "evt")
        .map(|(tab, message)| {
            (
                tab,
                message["event"].as_str().unwrap().to_owned(),
                message["data"].clone(),
            )
        })
        .collect()
}

fn is_store_write(op: &Op) -> bool {
    matches!(
        op,
        Op::WriteGrant { .. } | Op::RemoveGrant { .. } | Op::WriteSiteChain { .. }
    )
}

// ---------------------------------------------------------------------------
// The walk the brief asks for: enter → connect inside → leave
// ---------------------------------------------------------------------------

/// Enter, connect a site inside (the SAME site the person has connected, and
/// a new one), switch a site's network inside, leave: the store is byte for
/// byte what it was, the list Settings draws is byte for byte what it was,
/// and nothing was written to the store at any point in between.
#[test]
fn a_round_trip_through_a_dev_space_leaves_the_persons_sites_byte_for_byte() {
    let mut walk = Walk::booted(owners_store());
    let store_before = walk.store.bytes();
    let sites_before = walk.sites_bytes();
    assert!(walk.store.writes.is_empty(), "booting wrote nothing");

    walk.enter();

    // Inside: the test dApp the person has connected is connected again, to
    // the test wallet; a site the person never used is connected too; and a
    // site switches network.
    walk.hello("t1", "d1", LOCAL);
    let connect = walk.connect("t1", "d1", LOCAL, "1");
    assert_eq!(
        answers(&connect)[0]["result"],
        json!([FIXTURE]),
        "inside, a site is given the space's account"
    );
    walk.hello("t2", "d2", "https://new.example");
    walk.connect("t2", "d2", "https://new.example", "1");
    walk.ask(
        "t1",
        "d1",
        LOCAL,
        "2",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x2105"}]),
    );
    assert_eq!(walk.accounts_of("t1", "d1", LOCAL, "3"), json!([FIXTURE]));

    walk.leave();

    assert_eq!(
        walk.store.writes,
        Vec::<Op>::new(),
        "not one write to the site store, from the way in to the way out"
    );
    assert_eq!(walk.store.bytes(), store_before, "the store, byte for byte");
    assert_eq!(
        walk.sites_bytes(),
        sites_before,
        "Settings' list, byte for byte"
    );
    // And the person's site answers for the person's account again, on the
    // network it was on.
    assert_eq!(walk.accounts_of("t1", "d1", LOCAL, "4"), json!([A1]));
    assert_eq!(walk.connected("t1").as_deref(), Some(A1));
    let chain = walk.ask("t1", "d1", LOCAL, "5", "eth_chainId", json!([]));
    assert_eq!(answers(&chain)[0]["result"], json!("0x64"));
    // The site connected only inside is not connected outside.
    assert_eq!(
        walk.accounts_of("t2", "d2", "https://new.example", "6"),
        json!([])
    );
}

/// The two events of a session change are separate, and a shell may deliver
/// them in either order — the old bug WAS an order (the shorter account list
/// arrived while the grants were still pinned to the test wallet). Every
/// order, on the way in and on the way out, ends in the same store.
#[test]
fn every_order_of_the_two_account_events_keeps_the_persons_sites() {
    for (enter_addresses_first, leave_addresses_first) in
        [(true, true), (true, false), (false, true), (false, false)]
    {
        let mut walk = Walk::booted(owners_store());
        let before = walk.store.bytes();

        if enter_addresses_first {
            walk.updated(&[A1, A2, FIXTURE]);
            walk.switched(FIXTURE, true);
        } else {
            walk.switched(FIXTURE, true);
            walk.updated(&[A1, A2, FIXTURE]);
        }
        walk.hello("t1", "d1", LOCAL);
        walk.connect("t1", "d1", LOCAL, "1");
        if leave_addresses_first {
            walk.updated(&[A1, A2]);
            walk.switched(A1, false);
        } else {
            walk.switched(A1, false);
            walk.updated(&[A1, A2]);
        }

        let order = format!("enter {enter_addresses_first}, leave {leave_addresses_first}");
        assert_eq!(walk.store.writes, Vec::<Op>::new(), "{order}");
        assert_eq!(walk.store.bytes(), before, "{order}");
        assert_eq!(
            walk.accounts_of("t1", "d1", LOCAL, "9"),
            json!([A1]),
            "{order}"
        );
    }
}

/// A launch INSIDE the space (the flag survives a relaunch): the account
/// event can arrive before the store has been read. What the store holds is
/// still the person's, and is set aside the moment it arrives.
#[test]
fn a_launch_inside_a_dev_space_sets_the_persons_sites_aside_as_they_load() {
    for listed_first in [false, true] {
        let mut walk = Walk::cold(owners_store());
        let before = walk.store.bytes();
        if listed_first {
            walk.list();
            walk.accounts(&[A1, A2, FIXTURE], FIXTURE, true);
        } else {
            walk.accounts(&[A1, A2, FIXTURE], FIXTURE, true);
            walk.list();
        }
        assert!(
            walk.sut.view().sites.is_empty(),
            "listed first {listed_first}: inside, the person's sites are not the space's"
        );
        walk.hello("t1", "d1", DAPP);
        assert_eq!(walk.accounts_of("t1", "d1", DAPP, "1"), json!([]));
        walk.connect("t1", "d1", DAPP, "2");
        walk.leave();
        assert_eq!(
            walk.store.writes,
            Vec::<Op>::new(),
            "listed first {listed_first}"
        );
        assert_eq!(walk.store.bytes(), before, "listed first {listed_first}");
        assert_eq!(walk.sut.view().sites.len(), 3);
        assert_eq!(walk.accounts_of("t1", "d1", DAPP, "3"), json!([A1]));
    }
}

/// The exact failure, pinned: with the person's sites pinned to A1, the old
/// machine wrote three re-pins on the way in and three removals on the way
/// out. Now there is not a single store operation in the whole visit — and
/// the same two events WITHOUT the space flag still do what an account
/// switch has always done, so the flag is what the rule hangs on.
#[test]
fn entering_and_leaving_emit_no_store_operation_at_all() {
    let mut walk = Walk::booted(owners_store());
    let ops: Vec<Op> = walk.enter().into_iter().chain(walk.leave()).collect();
    assert!(
        !ops.iter().any(is_store_write),
        "a visit writes nothing: {ops:?}"
    );

    // The person switching to an account of their own is not a visit: every
    // site follows it, as before.
    let mut walk = Walk::booted(owners_store());
    let ops = walk.accounts(&[A1, A2], A2, false);
    let repinned: Vec<&str> = ops
        .iter()
        .filter_map(|op| match op {
            Op::WriteGrant { grant } if grant.address == A2 => Some(grant.origin.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(repinned, vec![LOCAL, DAPP, OTHER]);
}

// ---------------------------------------------------------------------------
// What a page hears
// ---------------------------------------------------------------------------

/// A page open across the way in hears that it is no longer connected — in
/// the space it is not — and hears its own account back on the way out. A
/// page of a site that was never connected hears nothing either way.
#[test]
fn an_open_page_hears_exactly_what_changed_for_it() {
    let mut walk = Walk::booted(owners_store());
    walk.hello("t1", "d1", DAPP);
    walk.hello("t2", "d2", "https://never.example");
    assert_eq!(walk.connected("t1").as_deref(), Some(A1));

    let entered = walk.enter();
    assert_eq!(
        events(&entered),
        vec![("t1".to_owned(), "accountsChanged".to_owned(), json!([]))],
        "the connected page is told it is not connected here; nobody else is told anything"
    );
    assert_eq!(walk.connected("t1"), None);
    assert_eq!(walk.accounts_of("t1", "d1", DAPP, "1"), json!([]));

    let left = walk.leave();
    assert_eq!(
        events(&left),
        vec![("t1".to_owned(), "accountsChanged".to_owned(), json!([A1]))],
        "and is given its own account back"
    );
    assert_eq!(walk.connected("t1").as_deref(), Some(A1));
}

/// A site connected inside is told, on the way out, whatever is now true for
/// it: the person's account where the person had connected it, nothing where
/// they had not.
#[test]
fn a_site_connected_inside_hears_the_persons_truth_on_the_way_out() {
    let mut walk = Walk::booted(owners_store());
    walk.enter();
    walk.hello("t1", "d1", LOCAL);
    walk.connect("t1", "d1", LOCAL, "1");
    walk.hello("t2", "d2", "https://new.example");
    walk.connect("t2", "d2", "https://new.example", "1");

    let left = walk.leave();
    let mut heard: Vec<(String, Value)> = events(&left)
        .into_iter()
        .filter(|(_, event, _)| event == "accountsChanged")
        .map(|(tab, _, data)| (tab, data))
        .collect();
    heard.sort_by(|a, b| a.0.cmp(&b.0));
    // The last word each page hears is the truth outside the space.
    let last = |tab: &str| {
        heard
            .iter()
            .rev()
            .find(|(t, _)| t == tab)
            .map(|(_, data)| data.clone())
    };
    assert_eq!(last("t1"), Some(json!([A1])));
    assert_eq!(last("t2"), Some(json!([])));
}

/// A site keeps the network it was on when the space is entered — where a
/// page is is not a grant — and a switch made inside stays inside.
#[test]
fn a_site_keeps_its_network_on_the_way_in_and_gets_its_own_back_on_the_way_out() {
    let mut walk = Walk::booted(owners_store());
    walk.hello("t1", "d1", OTHER);
    let chain = |walk: &mut Walk, id: &str| {
        let ops = walk.ask("t1", "d1", OTHER, id, "eth_chainId", json!([]));
        answers(&ops)[0]["result"].clone()
    };
    assert_eq!(chain(&mut walk, "1"), json!("0x2105"));

    let entered = walk.enter();
    assert!(
        !events(&entered)
            .iter()
            .any(|(_, event, _)| event == "chainChanged"),
        "entering moves no page to another network"
    );
    assert_eq!(chain(&mut walk, "2"), json!("0x2105"));

    walk.ask(
        "t1",
        "d1",
        OTHER,
        "3",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x64"}]),
    );
    assert_eq!(chain(&mut walk, "4"), json!("0x64"));

    let left = walk.leave();
    assert!(
        events(&left).contains(&("t1".to_owned(), "chainChanged".to_owned(), json!("0x2105"))),
        "the page is told it is back on the person's network: {left:?}"
    );
    assert_eq!(chain(&mut walk, "5"), json!("0x2105"));
    assert_eq!(
        walk.store.kv.get(&format!("vela.chain.{OTHER}")),
        Some(&"8453".to_owned())
    );
}

// ---------------------------------------------------------------------------
// Around the visit
// ---------------------------------------------------------------------------

/// Coming back to a DIFFERENT account of the person's is the person changing
/// account: their sites follow it, as they always have. The space is not a
/// way around `followActiveAccount`; it is only not a part of it.
#[test]
fn leaving_to_another_of_the_persons_accounts_is_an_ordinary_switch() {
    let mut walk = Walk::booted(owners_store());
    walk.enter();
    let ops = walk.accounts(&[A1, A2], A2, false);
    let written: Vec<(&str, &str)> = ops
        .iter()
        .filter_map(|op| match op {
            Op::WriteGrant { grant } => Some((grant.origin.as_str(), grant.address.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(written, vec![(LOCAL, A2), (DAPP, A2), (OTHER, A2)]);
    assert!(!ops.iter().any(|op| matches!(op, Op::RemoveGrant { .. })));
}

/// Inside the space, disconnecting a site disconnects the space's grant and
/// leaves the person's — in the store and on the way out.
#[test]
fn a_disconnect_inside_is_the_spaces_own() {
    let mut walk = Walk::booted(owners_store());
    let before = walk.store.bytes();
    walk.enter();
    walk.hello("t1", "d1", LOCAL);
    walk.connect("t1", "d1", LOCAL, "1");
    walk.send(Event::RevokeRequested {
        origin: LOCAL.to_owned(),
    });
    assert_eq!(walk.accounts_of("t1", "d1", LOCAL, "2"), json!([]));
    walk.leave();
    assert_eq!(walk.store.bytes(), before);
    assert_eq!(walk.accounts_of("t1", "d1", LOCAL, "3"), json!([A1]));
}

/// "Remove every dApp connection" (Settings → Storage) is the person's own
/// words about the device, wherever they say them: from inside a space it
/// takes the sites set aside too, in the store, and they do not come back.
#[test]
fn erasing_every_connection_inside_a_space_erases_the_persons_too() {
    let mut walk = Walk::booted(owners_store());
    walk.enter();
    walk.hello("t1", "d1", LOCAL);
    walk.connect("t1", "d1", LOCAL, "1");
    let ops = walk.send(Event::RevokeAll);
    let mut removed: Vec<&str> = ops
        .iter()
        .filter_map(|op| match op {
            Op::RemoveGrant { origin } => Some(origin.as_str()),
            _ => None,
        })
        .collect();
    removed.sort_unstable();
    assert_eq!(removed, vec![LOCAL, DAPP, OTHER]);
    walk.leave();
    assert!(walk.sut.view().sites.is_empty());
    assert!(!walk
        .store
        .kv
        .keys()
        .any(|key| key.starts_with("vela.perm.")));
}

/// The test wallet's record leaving the device (the way out) is not what
/// drops the person's sites any more; and a second visit starts with a clean
/// space — what was connected inside the first is gone.
#[test]
fn a_second_visit_starts_clean() {
    let mut walk = Walk::booted(owners_store());
    walk.enter();
    walk.hello("t1", "d1", "https://new.example");
    walk.connect("t1", "d1", "https://new.example", "1");
    walk.leave();
    walk.enter();
    assert_eq!(
        walk.accounts_of("t1", "d1", "https://new.example", "2"),
        json!([]),
        "what the first visit connected ended with it"
    );
    assert!(walk.sut.view().sites.is_empty());
    walk.leave();
    assert_eq!(walk.store.bytes(), owners_store().bytes());
}

// ---------------------------------------------------------------------------
// "This site is not connected" is a statement about now
// ---------------------------------------------------------------------------

/// A page signs before it connects: 4100, and the tab's status says the site
/// is not connected. Then it connects — and the tab must stop saying so (the
/// phone walk: the banner stayed for as long as the tab did).
#[test]
fn a_not_connected_trouble_ends_when_the_site_connects() {
    let mut walk = Walk::booted(Store::default());
    walk.hello("t1", "d1", DAPP);
    walk.ask("t1", "d1", DAPP, "1", "personal_sign", json!(["0x00", A1]));
    let tab = |walk: &Walk| walk.sut.view().tabs.into_iter().next().unwrap();
    let before = tab(&walk);
    assert_eq!(
        before.last_failure.as_ref().map(|note| note.reason),
        Some(DbrReason::NotConnected)
    );
    assert_eq!(before.failed_recent, 1);

    walk.connect("t1", "d1", DAPP, "2");
    let after = tab(&walk);
    assert_eq!(after.connected_address.as_deref(), Some(A1));
    assert_eq!(
        after.last_failure, None,
        "connected: nothing is wrong with it"
    );
    assert_eq!(after.failed_recent, 0);

    // The record keeps the row — it happened — and a disconnect makes the
    // statement true again, so it is said again.
    walk.send(Event::RevokeRequested {
        origin: DAPP.to_owned(),
    });
    assert_eq!(
        tab(&walk).last_failure.map(|note| note.reason),
        Some(DbrReason::NotConnected)
    );
}

/// Only that trouble ends: another failure of the same tab is still its
/// latest trouble after the site connects.
#[test]
fn connecting_clears_no_other_trouble() {
    let mut walk = Walk::booted(Store::default());
    walk.hello("t1", "d1", DAPP);
    walk.ask(
        "t1",
        "d1",
        DAPP,
        "1",
        "wallet_switchEthereumChain",
        json!([{"chainId":"0x1234"}]),
    );
    walk.ask("t1", "d1", DAPP, "2", "personal_sign", json!(["0x00", A1]));
    walk.connect("t1", "d1", DAPP, "3");
    let tab = walk.sut.view().tabs.into_iter().next().unwrap();
    assert_eq!(
        tab.last_failure.map(|note| note.reason),
        Some(DbrReason::UnknownChain)
    );
    assert_eq!(tab.failed_recent, 1);
}

// ---------------------------------------------------------------------------
// The address bar's mark
// ---------------------------------------------------------------------------

/// A padlock says "encrypted", so it is drawn for https and nothing else: a
/// page on this device over plain http gets a mark of its own, and a public
/// http page the warning it always had.
#[test]
fn a_padlock_is_for_https_only() {
    for (origin, lock) in [
        ("https://app.uniswap.org", OriginLock::Secure),
        ("https://127.0.0.1:8443", OriginLock::Secure),
        ("http://127.0.0.1:8137", OriginLock::Local),
        ("http://localhost:5173", OriginLock::Local),
        ("http://[::1]:8080", OriginLock::Local),
        ("http://192.168.1.20:8137", OriginLock::Local),
        ("http://printer.local", OriginLock::Local),
        ("http://example.com", OriginLock::Insecure),
        ("http://127.0.0.1.evil.com", OriginLock::Insecure),
        ("not a url", OriginLock::Insecure),
        ("", OriginLock::Insecure),
    ] {
        assert_eq!(origin_lock(origin), lock, "{origin}");
    }
}

/// The tab's view and the consent sheet's both carry it, so no shell works
/// it out from a URL.
#[test]
fn the_tab_and_the_consent_sheet_carry_the_mark() {
    let mut walk = Walk::booted(Store::default());
    walk.hello("t1", "d1", LOCAL);
    walk.hello("t2", "d2", DAPP);
    let tabs = walk.sut.view().tabs;
    let lock = |tab: &str| tabs.iter().find(|view| view.tab == tab).unwrap().lock;
    assert_eq!(lock("t1"), OriginLock::Local);
    assert_eq!(lock("t2"), OriginLock::Secure);
    // `secure` keeps its meaning: safe to use the wallet from.
    assert!(tabs.iter().all(|view| view.secure));

    walk.ask("t1", "d1", LOCAL, "1", "eth_requestAccounts", json!([]));
    assert_eq!(
        walk.sut.view().consent.map(|consent| consent.lock),
        Some(OriginLock::Local)
    );
    // On the wire it is one lower-case word.
    assert_eq!(
        serde_json::to_value(OriginLock::Local).unwrap(),
        json!("local")
    );
}
