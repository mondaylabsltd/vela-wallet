//! A multisig link opened as ONE flow — the web's `linked.test.ts`,
//! `kept-link.test.ts` and `MultisigHost.answerLink` / `openProposal`: the
//! accounts machine reads the link, finds the multisig here or looks it up,
//! keeps and shows it only when a member is here, and tells the proposals
//! machine to open its proposal ([`Notice::OpenProposal`]); with none here,
//! its transaction is read only — nothing added, nothing switched.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::json;
use support::DomainDriver;
use vela_core::app::multisig_accounts::link::LinkStand;
use vela_core::app::multisig_accounts::{AddOutcome, Event, MultisigAccounts};
use vela_core::app::multisig_proposals::flows::LinkedStand;
use vela_core::app::multisig_proposals::{self as proposals, MultisigProposals};
use vela_core::app::multisig_shell::{keys, MultisigAccount, Notice};
use vela_core::multisig;
use vela_core::multisig_link::{AddLinkFault, BrokenLink, KEPT_LINK_KEY};
use world::{fixture, genesis, settle, stored_row, DeployedSafe, World, SERVICE};

type Sut = DomainDriver<MultisigAccounts>;
type Props = DomainDriver<MultisigProposals>;

const OUTSIDE: &str = "0x00000000000000000000000000000000000000bb";

fn run<A>(sut: &mut DomainDriver<A>, world: &mut World, event: A::Event)
where
    A: crux_core::App + Default,
    A::Model: Default,
    A::Effect: vela_core::app::SplitEffect<Op = vela_core::app::multisig_shell::MultisigOperation>,
{
    sut.dispatch(event);
    let answered = settle(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

fn started(world: &mut World, networks: Vec<u64>) -> Sut {
    let mut sut = Sut::new();
    run(
        &mut sut,
        world,
        Event::Start {
            service_url: SERVICE.to_owned(),
            accounts: world.locals.clone(),
            networks,
        },
    );
    sut
}

fn link(chain: u64, address: &str, hash: &str) -> String {
    format!("#ms={chain}.{address}.{hash}")
}

fn open(hash: &str) -> Event {
    Event::OpenLink {
        hash: hash.to_owned(),
        take_kept: false,
    }
}

const HASH: &str = "0xabababababababababababababababababababababababababababababababab";

fn opened(world: &World) -> Vec<Notice> {
    world
        .notices
        .iter()
        .filter(|n| matches!(n, Notice::OpenProposal { .. }))
        .cloned()
        .collect()
}

/// ms-22, r5reg-22: a damaged link is said at once, never ignored.
#[test]
fn a_damaged_link_is_said() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open("#ms=100.0x12"));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Broken {
            link: BrokenLink::Proposal,
            fault: None,
            ..
        })
    ));
    // An invite whose address's letters were changed on the way.
    let g = genesis(&[OUTSIDE], 1, "0x51");
    let swapped: String = g
        .address
        .chars()
        .map(|c| {
            if c.is_ascii_uppercase() {
                c.to_ascii_lowercase()
            } else if c.is_ascii_lowercase() && c != 'x' {
                c.to_ascii_uppercase()
            } else {
                c
            }
        })
        .collect();
    run(&mut sut, &mut world, open(&format!("#msa=100:{swapped}")));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Broken {
            link: BrokenLink::Add,
            fault: Some(AddLinkFault::Checksum),
            ..
        })
    ));
    assert!(opened(&world).is_empty());
}

/// r4D-4: an invite opens the add sheet; adding is the person's tap.
#[test]
fn an_invite_opens_the_add_sheet_and_adds_nothing() {
    let g = genesis(&[OUTSIDE], 1, "0x52");
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        open(&format!("#msa=100:{}", g.address)),
    );
    match sut.view().link {
        Some(LinkStand::Invite { link }) => assert_eq!(link.chain_ids, vec![100]),
        other => panic!("{other:?}"),
    }
    assert!(world.stored_accounts().is_empty());
}

/// A member is here (its founding members say so): shown, its proposal
/// opened — no lookup, no chain read waited for.
#[test]
fn a_member_here_opens_it_as_before() {
    let me = fixture(0).address;
    let g = genesis(&[&me, OUTSIDE], 2, "0x53");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100, 8453]);
    world.notices.clear();
    let http = world.http_log.len();
    run(&mut sut, &mut world, open(&link(8453, &g.address, HASH)));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Member {
            kept_before: true,
            ..
        })
    ));
    assert_eq!(world.http_log.len(), http, "nothing looked up");
    assert_eq!(sut.view().active.as_deref(), Some(g.address.as_str()));
    let at = world
        .notices
        .iter()
        .position(|n| matches!(n, Notice::Activated { .. }))
        .unwrap();
    let open = world
        .notices
        .iter()
        .position(|n| matches!(n, Notice::OpenProposal { preview: None, .. }))
        .unwrap();
    assert!(at < open, "shown first, then its proposal");
    // Used on the link's network now.
    assert!(world.stored_accounts()[0]["chainIds"]
        .as_array()
        .unwrap()
        .contains(&json!(8453)));
}

/// ms-21: none of its members here — its transaction read only; nothing
/// added, nothing switched.
#[test]
fn no_member_here_is_a_preview_that_keeps_nothing() {
    let g = genesis(&[OUTSIDE], 1, "0x54");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open(&link(100, &g.address, HASH)));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Preview { kept: false, .. })
    ));
    assert!(world.stored_accounts().is_empty());
    assert_eq!(sut.view().active, None);
    match &opened(&world)[..] {
        [Notice::OpenProposal {
            preview: Some(account),
            chain_id: 100,
            ..
        }] => assert_eq!(account.address, g.address),
        other => panic!("{other:?}"),
    }
}

/// A member added on chain since its creation counts: kept, shown, opened.
#[test]
fn a_member_added_on_chain_since_counts() {
    let me = fixture(0).address;
    let g = genesis(&[OUTSIDE], 1, "0x55");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.record(&g.address, &g.genesis, false);
    world.deploy(100, &g.address, DeployedSafe::clean(&[OUTSIDE, &me], 1));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open(&link(100, &g.address, HASH)));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Member {
            kept_before: false,
            ..
        })
    ));
    assert_eq!(world.stored_accounts().len(), 1);
    assert_eq!(sut.view().active.as_deref(), Some(g.address.as_str()));
}

/// Not found: said as that, nothing kept.
#[test]
fn not_found_is_said_and_keeps_nothing() {
    let g = genesis(&[OUTSIDE], 1, "0x56");
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open(&link(100, &g.address, HASH)));
    match sut.view().link {
        Some(LinkStand::Failed { reason, .. }) => {
            assert!(matches!(
                reason,
                AddOutcome::NotFound | AddOutcome::Unavailable | AddOutcome::Unchecked
            ));
        }
        other => panic!("{other:?}"),
    }
    assert!(world.stored_accounts().is_empty());
    assert!(opened(&world).is_empty());
}

/// §3.5: more than its members act for it — nothing read or kept; added
/// on the person's tap, its transaction opens then (ms-22).
#[test]
fn a_high_risk_one_opens_only_once_the_person_adds_it() {
    let me = fixture(0).address;
    let g = genesis(&[&me], 1, "0x57");
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.record(&g.address, &g.genesis, false);
    let mut safe = DeployedSafe::clean(&[&me], 1);
    safe.modules = vec!["0x00000000000000000000000000000000000000cc".to_owned()];
    world.deploy(100, &g.address, safe);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open(&link(100, &g.address, HASH)));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::Failed {
            reason: AddOutcome::HighRisk { .. },
            ..
        })
    ));
    assert!(world.stored_accounts().is_empty());
    assert!(opened(&world).is_empty());
    run(
        &mut sut,
        &mut world,
        Event::Add {
            address: g.address.clone(),
            network: Some(100),
            accept_risks: true,
        },
    );
    assert!(matches!(sut.view().link, Some(LinkStand::Member { .. })));
    assert_eq!(opened(&world).len(), 1);
}

/// ms-22: a network this device lacks is said; the link opens once it is added.
#[test]
fn a_missing_network_opens_the_link_once_added() {
    let g = genesis(&[OUTSIDE], 1, "0x58");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, open(&link(8453, &g.address, HASH)));
    assert!(matches!(
        sut.view().link,
        Some(LinkStand::MissingNetwork { .. })
    ));
    assert!(opened(&world).is_empty());
    run(
        &mut sut,
        &mut world,
        Event::Networks {
            networks: vec![100, 8453],
        },
    );
    assert!(matches!(sut.view().link, Some(LinkStand::Preview { .. })));
    assert_eq!(opened(&world).len(), 1);
}

/// r4b-9: a link opened with no wallet here is kept, and opened (once)
/// when there is one; a link in the address wins, the kept one waits.
#[test]
fn a_link_kept_through_sign_in_opens_once() {
    let g = genesis(&[OUTSIDE], 1, "0x59");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    let hash = link(100, &g.address, HASH);
    run(&mut sut, &mut world, Event::KeepLink { hash: hash.clone() });
    assert_eq!(sut.view().link, Some(LinkStand::Kept));
    assert!(world.store.contains_key(KEPT_LINK_KEY));
    // An invite in the address is answered; the kept link is kept again.
    run(
        &mut sut,
        &mut world,
        Event::OpenLink {
            hash: format!("#msa=100:{}", g.address),
            take_kept: true,
        },
    );
    assert!(matches!(sut.view().link, Some(LinkStand::Invite { .. })));
    assert!(world.store.contains_key(KEPT_LINK_KEY));
    run(
        &mut sut,
        &mut world,
        Event::OpenLink {
            hash: String::new(),
            take_kept: true,
        },
    );
    assert!(matches!(sut.view().link, Some(LinkStand::Preview { .. })));
    assert!(!world.store.contains_key(KEPT_LINK_KEY), "taken once");
}

// -- the proposals machine's side: the linked proposal's sheet ------------------------------

fn account_of(g: &multisig::MultisigGenesis) -> MultisigAccount {
    serde_json::from_value(stored_row(g, &[100], true)).unwrap()
}

/// A preview reads the link's transaction from the mailbox, re-hashed —
/// and says so when the mailbox does not hold it yet.
#[test]
fn a_preview_reads_the_transaction_and_says_when_it_is_not_there() {
    let g = genesis(&[OUTSIDE], 1, "0x5a");
    let account = account_of(&g);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let mut sut = Props::new();
    let ev = proposals::Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(&mut sut, &mut world, ev);
    let open = |hash: &str| proposals::Event::OpenLinked {
        chain_id: 100,
        safe_tx_hash: hash.to_owned(),
        preview: Some(Box::new(account.clone())),
    };
    run(&mut sut, &mut world, open(&hash));
    let linked = sut.view().linked.unwrap();
    assert!(linked.preview);
    assert_eq!(linked.stand, LinkedStand::NotInMailbox);
    assert_eq!(sut.view().followed, None, "nothing switched");
    assert_eq!(sut.view().pace_ms, proposals::OPEN_POLL_MS);

    world.mailbox.push(json!({
        "chainId": 100, "safe": account.address, "safeTxHash": hash, "safeTx": tx,
        "proposer": OUTSIDE, "approvals": [], "createdMs": 1, "updatedMs": 1,
    }));
    run(&mut sut, &mut world, proposals::Event::Tick);
    let linked = sut.view().linked.unwrap();
    assert_eq!(linked.stand, LinkedStand::Found);
    assert_eq!(linked.item.unwrap().safe_tx_hash, hash);
    assert!(
        sut.view().reviews.contains_key(&format!("100:{hash}")),
        "its review, as any sheet's"
    );
    // A copy that does not hash as the link says is not the link's.
    let other = format!("0x{}", "12".repeat(32));
    run(&mut sut, &mut world, open(&other));
    assert_eq!(sut.view().linked.unwrap().stand, LinkedStand::NotInMailbox);
    run(&mut sut, &mut world, proposals::Event::CloseLinked);
    assert!(sut.view().linked.is_none());
}

/// A member's link opens on the followed multisig's own list.
#[test]
fn a_members_link_opens_on_the_followed_list() {
    let me = fixture(0).address;
    let g = genesis(&[&me, OUTSIDE], 2, "0x5b");
    let account = account_of(&g);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(json!({
        "chainId": 100, "safe": account.address, "safeTxHash": hash, "safeTx": tx,
        "proposer": OUTSIDE, "approvals": [], "createdMs": 1, "updatedMs": 1,
    }));
    let mut sut = Props::new();
    let ev = proposals::Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        proposals::Event::Follow {
            account: Some(account.clone()),
        },
    );
    run(
        &mut sut,
        &mut world,
        proposals::Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            preview: None,
        },
    );
    let linked = sut.view().linked.unwrap();
    assert!(!linked.preview);
    assert_eq!(linked.stand, LinkedStand::Found);
    assert_eq!(linked.item.unwrap().can_approve, vec![me]);
}

/// ms-21: a preview's members still to approve are read as any sheet's —
/// how each signs, and who an outside wallet may sign for — from the link's
/// own list, never the followed multisig's.
#[test]
fn a_previews_members_say_how_they_sign() {
    let g = genesis(&[OUTSIDE], 1, "0x5c");
    let account = account_of(&g);
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(json!({
        "chainId": 100, "safe": account.address, "safeTxHash": hash, "safeTx": tx,
        "proposer": OUTSIDE, "approvals": [], "createdMs": 1, "updatedMs": 1,
    }));
    let mut sut = Props::new();
    let ev = proposals::Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100],
        known: Vec::new(),
    };
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        proposals::Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            preview: Some(Box::new(account.clone())),
        },
    );
    assert_eq!(sut.view().linked.unwrap().stand, LinkedStand::Found);
    run(
        &mut sut,
        &mut world,
        proposals::Event::ReadSigners {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
        },
    );
    let signers = sut.view().signers[&format!("100:{hash}")].clone();
    let owners: Vec<String> = signers
        .rows
        .iter()
        .map(|r| r.owner.to_lowercase())
        .collect();
    assert_eq!(owners, vec![OUTSIDE.to_owned()]);
    assert!(signers.rows[0].kind.is_some(), "how it signs is read");
    assert_eq!(signers.outside.len(), 1, "its own wallet may sign for it");
    assert_eq!(sut.view().followed, None, "nothing switched");
}
