//! The accounts machine (`multisig_accounts`), driven against a simulated
//! shell — the web's `accounts-create`, `accounts-find`, `accounts-unknown`,
//! `accounts-records`, `accounts-import`, `accounts`, `recheck`, `chain` and
//! `ens-name` suites, rule for rule.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use std::collections::BTreeMap;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_accounts::{
    AddOutcome, BackupState, CreateChoice, EnsAnswer, Event, ImportOutcome, InvitedMember, KindAsk,
    MultisigAccounts, Phase,
};
use vela_core::app::multisig_shell::{keys, AccountUpdate, MemberInfo, Notice};
use vela_core::app::multisig_signers::{KindNote, MemberKind};
use world::{fixture, genesis, settle, stored_row, DeployedSafe, World, HANDLER_141, SERVICE, T0};

type Sut = DomainDriver<MultisigAccounts>;

const A: &str = "0x00000000000000000000000000000000000000aa";
const B: &str = "0x00000000000000000000000000000000000000bb";
const HANDLER_LOWER: &str = "0xfd0732dc9e303f09fcef3a7388ad10a83459ec99";

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle(sut, world, 5_000);
    assert!(answered < 5_000, "the machine never settled");
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
    assert!(sut.view().loaded);
    sut
}

fn add(address: &str, network: Option<u64>, accept_risks: bool) -> Event {
    Event::Add {
        address: address.to_owned(),
        network,
        accept_risks,
    }
}

fn choice(world: &World) -> CreateChoice {
    let me = world.locals[0].address.clone();
    CreateChoice {
        owners: vec![me.clone(), B.to_owned()],
        threshold: 2,
        name: "Quiet Harbor".to_owned(),
        chain_id: 8453,
        creator: me,
        members: BTreeMap::from([(
            B.to_owned(),
            MemberInfo {
                name: Some("Bo".to_owned()),
                kind: Some("wallet".to_owned()),
                typed: None,
            },
        )]),
    }
}

// -- the store ---------------------------------------------------------------------

#[test]
fn the_store_is_read_as_the_web_wrote_it() {
    let g = genesis(&[A, B], 2, "0x05");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true), {"address": "0x01"}, "junk"]).to_string(),
    );
    world
        .store
        .insert(keys::ACTIVE.to_owned(), g.address.clone());
    let sut = started(&mut world, vec![100]);
    let view = sut.view();
    assert_eq!(view.accounts.len(), 1, "a row without a genesis is dropped");
    assert_eq!(view.active.as_deref(), Some(g.address.as_str()));
    assert_eq!(view.accounts[0].name.as_deref(), Some("Quiet Harbor"));
    assert!(view.accounts[0].shared);
}

// -- creating (accounts-create) ------------------------------------------------------

/// Recorded: shared, shown, its proof no longer kept — on the index chain
/// (the service's home, 100) whatever network it is first used on.
#[test]
fn created_and_recorded_on_the_index_chain() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, Event::NewDraft);
    let c = choice(&world);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            owners: c.owners.clone(),
            threshold: 2,
            name: c.name.clone(),
        },
    );
    let drafted = sut.view().create.draft_address.clone().unwrap();
    run(&mut sut, &mut world, Event::Create { choice: c });
    let view = sut.view();
    assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
    assert_eq!(world.passkeys.len(), 1, "one passkey prompt");
    let address = view.create.address.clone().unwrap();
    assert_eq!(address, drafted, "the address the review showed");
    assert_eq!(view.active.as_deref(), Some(address.as_str()));
    assert!(world.notices.contains(&Notice::Activated {
        address: Some(address.clone())
    }));
    assert_eq!(world.imports.len(), 1);
    let import = &world.imports[0];
    assert_eq!(
        import["chainId"], 100,
        "the index chain, not the first network"
    );
    assert_eq!(import["genesis"]["name"], "Quiet Harbor", "sent named");
    assert_eq!(import["ownerProof"]["owner"], world.locals[0].address);
    assert!(import["ownerProof"]["clientDataJSON"]
        .as_str()
        .unwrap()
        .starts_with("{\"type\":\"webauthn.get\""));
    assert!(import["ownerProof"]["signature"]
        .as_str()
        .unwrap()
        .starts_with("0x"));
    let rows = world.stored_accounts();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["recorded"], true);
    assert_eq!(rows[0]["shared"], true);
    assert_eq!(rows[0]["chainIds"], json!([8453]));
    assert_eq!(rows[0]["firstRun"], true);
    assert_eq!(rows[0]["members"][B]["name"], "Bo");
    assert_eq!(world.store[keys::PENDING_PROOFS], "{}");
    assert!(!view.accounts[0].record_waiting);
}

/// A service that is down is no failure: kept, not shared, its proof
/// waiting — then the loop records it with that proof, never signing again.
#[test]
fn created_while_the_service_is_down_is_kept_and_recorded_later() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    world.service_down = true;
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    let view = sut.view();
    assert_eq!(view.create.phase, Phase::Done);
    assert_eq!(view.create.refused, None, "a service away is not a refusal");
    let rows = world.stored_accounts();
    assert_eq!(rows.len(), 1, "kept here first");
    assert_eq!(rows[0]["recorded"], false);
    assert!(rows[0].get("shared").is_none());
    assert!(view.accounts[0].record_waiting, "its proof is kept");
    let proofs: BTreeMap<String, Value> =
        serde_json::from_str(&world.store[keys::PENDING_PROOFS]).unwrap();
    assert_eq!(proofs.len(), 1);
    assert!(view.service_down);

    // Back, after its step: the loop records it with the kept proof.
    world.service_down = false;
    world.now += 400_000.0;
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(world.imports.len(), 1);
    assert!(
        world.imports[0].get("ownerProof").is_some(),
        "with the kept proof"
    );
    assert_eq!(world.passkeys.len(), 1, "never signed again");
    // Taken: shared now; recorded once a later read confirms it.
    assert_eq!(world.stored_accounts()[0]["shared"], true);
    assert_eq!(world.stored_accounts()[0]["recorded"], false);
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(world.imports.len(), 1, "confirmed, not recorded twice");
    assert_eq!(world.stored_accounts()[0]["recorded"], true);
    assert!(!sut.view().accounts[0].record_waiting);
    // Every record landed: the tick asks nothing.
    let asked = world.http_log.len();
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(world.http_log.len(), asked);
}

/// The service's word on a record it would not pay for: kept here, not shared.
#[test]
fn created_and_refused_by_the_service_stays_kept() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    world.import_status = (422, json!({"refusal": {"kind": "ownerNotMember"}}));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(sut.view().create.phase, Phase::Done);
    assert_eq!(world.stored_accounts().len(), 1);
    assert_eq!(world.stored_accounts()[0]["recorded"], false);
}

/// The person closes the passkey: nothing kept, nothing asked.
#[test]
fn a_cancelled_passkey_creates_nothing() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    world.cancel_passkey = true;
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(sut.view().create.phase, Phase::Cancelled);
    assert!(world.stored_accounts().is_empty());
    assert!(world.imports.is_empty());
}

/// r4a-14: one member that is a multisig itself — a 1 of 1 genesis is made.
#[test]
fn a_one_of_one_draft_has_an_address() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            owners: vec![A.to_owned()],
            threshold: 1,
            name: "Solo".to_owned(),
        },
    );
    assert!(sut.view().create.draft_address.is_some());
    // A rule that cannot be met says why.
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            owners: vec![A.to_owned()],
            threshold: 2,
            name: "Solo".to_owned(),
        },
    );
    assert!(sut.view().create.draft_error.is_some());
    assert!(sut.view().create.draft_address.is_none());
}

// -- adding by address (accounts-find, accounts-unknown) ------------------------------

/// Undeployed, its record found, the mailbox holding a proposal on 8453:
/// Verified there by Safe's own code, used there — never the record's chain.
#[test]
fn added_where_the_mailbox_says_verified_by_the_counterfactual_rule() {
    let g = genesis(&[A, B], 2, "0x11");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.mailbox.push(
        json!({"chainId": 8453, "safe": g.address, "safeTxHash": "0x01",
        "safeTx": cancel(&g.address), "approvals": []}),
    );
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    let view = sut.view();
    assert_eq!(
        view.add.outcome,
        Some(AddOutcome::Added {
            address: g.address.clone()
        })
    );
    assert_eq!(view.active.as_deref(), Some(g.address.as_str()));
    let rows = world.stored_accounts();
    assert_eq!(rows[0]["chainIds"], json!([8453]));
    assert_eq!(rows[0]["shared"], true);
    assert_eq!(
        world
            .http_log
            .iter()
            .filter(|l| l.contains("/api/proposals/"))
            .count(),
        1,
        "one cross-chain read, not one per network"
    );
}

/// Deployed on Arbitrum though its record is on Gnosis: Arbitrum. The chain
/// is asked, not the genesis.
#[test]
fn deployed_is_its_network_and_the_chain_is_asked() {
    let g = genesis(&[A], 1, "0x17");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.deploy(42161, &g.address, DeployedSafe::clean(&[A], 1));
    let mut sut = started(&mut world, vec![100, 42161]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    assert!(matches!(
        sut.view().add.outcome,
        Some(AddOutcome::Added { .. })
    ));
    assert_eq!(world.stored_accounts()[0]["chainIds"], json!([42161]));
    assert!(world
        .rpc_log
        .iter()
        .any(|(c, m, _)| *c == 42161 && m == "eth_getStorageAt"));
}

/// Deployed with a module: high risk — kept only on the person's tap, with
/// what was found, shown wherever it is.
#[test]
fn deployed_with_a_module_is_kept_only_on_the_persons_tap() {
    let g = genesis(&[A], 1, "0x18");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut safe = DeployedSafe::clean(&[A], 1);
    safe.modules = vec!["0x00000000000000000000000000000000000000ee".to_owned()];
    world.deploy(100, &g.address, safe);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    let Some(AddOutcome::HighRisk { findings }) = sut.view().add.outcome else {
        panic!("{:?}", sut.view().add.outcome);
    };
    assert_eq!(findings[0]["risk"]["kind"], "module");
    assert!(world.stored_accounts().is_empty(), "not without the tap");
    run(&mut sut, &mut world, add(&g.address, None, true));
    assert!(matches!(
        sut.view().add.outcome,
        Some(AddOutcome::Added { .. })
    ));
    let row = &sut.view().accounts[0];
    assert_eq!(row.high_risk_on, vec![100]);
    assert_eq!(
        world.stored_accounts()[0]["findings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// Nothing says which network (its record's chain is no home): asked. The
/// person's pick is then its network.
#[test]
fn nothing_naming_a_network_asks() {
    let g = genesis(&[A], 1, "0x12");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::ChooseNetwork));
    assert!(world.stored_accounts().is_empty(), "nothing kept");
    run(&mut sut, &mut world, add(&g.address, Some(8453), false));
    assert!(matches!(
        sut.view().add.outcome,
        Some(AddOutcome::Added { .. })
    ));
    assert_eq!(world.stored_accounts()[0]["chainIds"], json!([8453]));
}

/// The cross-chain list naming only networks this device lacks: asks.
#[test]
fn a_list_naming_only_networks_this_device_lacks_asks() {
    let g = genesis(&[A], 1, "0x19");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.mailbox.push(
        json!({"chainId": 10, "safe": g.address, "safeTxHash": "0x01",
        "safeTx": cancel(&g.address), "approvals": []}),
    );
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::ChooseNetwork));
}

/// A genesis the core refuses there (Safe's handler has no code on that
/// chain) is not added, and says why — no "add anyway".
#[test]
fn refused_by_the_counterfactual_rule_is_not_kept() {
    let g = genesis(&[A], 1, "0x13");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world
        .canonical_missing
        .insert((8453, HANDLER_LOWER.to_owned()));
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, add(&g.address, Some(8453), true));
    let Some(AddOutcome::Refused { findings }) = sut.view().add.outcome else {
        panic!("{:?}", sut.view().add.outcome);
    };
    assert_eq!(findings[0]["from"], "genesis");
    assert_eq!(findings[0]["chainId"], 8453);
    assert_eq!(findings[0]["reason"]["kind"], "missingCode");
    assert!(world.stored_accounts().is_empty());
}

/// The chain does not answer for the Safe contracts (but did for the
/// lookup): not verified, not added.
#[test]
fn a_chain_that_cannot_be_checked_yet_is_unverified() {
    let g = genesis(&[A], 1, "0x14");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let handler = HANDLER_141.to_ascii_lowercase();
    world.rpc_hooks.push(Box::new(move |chain, method, params| {
        (chain == 8453
            && method == "eth_getCode"
            && params[0].as_str().unwrap().eq_ignore_ascii_case(&handler))
        .then(
            || vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "timed out".to_owned(),
            },
        )
    }));
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, add(&g.address, Some(8453), false));
    assert_eq!(
        sut.view().add.outcome,
        Some(AddOutcome::Unverified { chain_id: 8453 })
    );
    assert!(world.stored_accounts().is_empty());
}

/// A deployed Safe whose state cannot be read is not added unchecked.
#[test]
fn a_deployed_safe_whose_state_cannot_be_read_is_unavailable() {
    let g = genesis(&[A], 1, "0x1a");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.deploy(100, &g.address, DeployedSafe::clean(&[A], 1));
    world.rpc_hooks.push(Box::new(|_, method, _| {
        (method == "eth_getStorageAt").then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "timed out".to_owned(),
            }
        })
    }));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, add(&g.address, None, false));
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::Unavailable));
}

/// r4b-13: an unknown address is "not found" within a few seconds — its
/// record asked for once more, not four times.
#[test]
fn an_unknown_address_is_not_found_after_one_more_ask() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        add("0x00000000000000000000000000000000000000cc", None, false),
    );
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::NotFound));
    let asks = world
        .http_log
        .iter()
        .filter(|l| l.contains("/api/genesis/"))
        .count();
    assert_eq!(asks, 2);
    assert!(world.now - T0 < 10_000.0, "within a few seconds");
}

/// r4b-13: a network that never answers is "couldn't check", never "none".
#[test]
fn an_unanswering_network_is_unchecked_never_none() {
    let mut world = World::default();
    world.silent.insert(8453);
    let mut sut = started(&mut world, vec![100, 8453]);
    run(
        &mut sut,
        &mut world,
        add("0x00000000000000000000000000000000000000cd", None, false),
    );
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::Unchecked));
}

/// An address that sends its own transactions is no undeployed Safe.
#[test]
fn an_account_that_sends_is_not_a_multisig() {
    let mut world = World::default();
    let who = "0x00000000000000000000000000000000000000ce";
    world.tx_counts.insert((100, who.to_owned()), 3);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, add(who, None, false));
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::NotMultisig));
}

/// ms-21: read by its address, kept nowhere; kept on a tap, not shown.
#[test]
fn found_keeps_nothing_until_the_tap_which_does_not_show_it() {
    let g = genesis(&[A], 1, "0x1b");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::Find {
            address: g.address.clone(),
            network: Some(100),
        },
    );
    assert!(sut.view().add.found.is_some());
    assert!(world.stored_accounts().is_empty());
    run(
        &mut sut,
        &mut world,
        Event::KeepFound {
            accept_risks: false,
        },
    );
    assert_eq!(world.stored_accounts().len(), 1);
    assert_eq!(sut.view().active, None, "not shown");
}

/// An invite's labels land only on members (r4D-5).
#[test]
fn an_invites_labels_land_only_on_members() {
    let g = genesis(&[A, B], 2, "0x1c");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    let m = |a: &str, n: &str| InvitedMember {
        address: Some(a.to_owned()),
        name: Some(n.to_owned()),
        kind: Some("passkey".to_owned()),
    };
    run(
        &mut sut,
        &mut world,
        Event::KeepInvited {
            address: g.address.clone(),
            chain_ids: vec![100],
            members: vec![
                m(A, "Ana"),
                m("0x00000000000000000000000000000000000000dd", "Mallory"),
            ],
        },
    );
    let members = &world.stored_accounts()[0]["members"];
    assert_eq!(members[A]["name"], "Ana");
    assert!(members
        .get("0x00000000000000000000000000000000000000dd")
        .is_none());
}

// -- where a record goes, and what it is now (accounts-records) --------------------------

/// A record the service no longer holds (an earlier registry) is not shared
/// — once it had the time to land.
#[test]
fn a_record_the_service_lacks_is_not_shared_after_its_landing_time() {
    let g = genesis(&[A], 1, "0x1d");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::CheckRecords);
    let row = &world.stored_accounts()[0];
    assert_eq!(row["recorded"], false);
    assert_eq!(row["shared"], false);
}

/// The service's name where the chain cannot be read here, and never one
/// Vela would not show.
#[test]
fn a_records_name_is_taken_only_when_vela_shows_it() {
    let g = genesis(&[A], 1, "0x1e");
    let mut world = World::default();
    let mut renamed = g.genesis.clone();
    renamed.name = Some("Treasury".to_owned());
    world.record(&g.address, &renamed, false);
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    // The registry's chain is not this device's: the service's word.
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, Event::CheckRecords);
    assert_eq!(world.stored_accounts()[0]["genesis"]["name"], "Treasury");

    let mut bad = g.genesis.clone();
    bad.name = Some(" spaced ".to_owned());
    world.record(&g.address, &bad, false);
    run(
        &mut sut,
        &mut world,
        Event::RecheckRecord {
            address: g.address.clone(),
        },
    );
    assert_eq!(world.stored_accounts()[0]["genesis"]["name"], "Treasury");
}

/// Sharing again: one proof, a named genesis, shared.
#[test]
fn an_owners_passkey_shares_it_again() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let g = genesis(&[&me, B], 2, "0x1f");
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[8453], false)]).to_string(),
    );
    let mut sut = started(&mut world, vec![8453]);
    run(
        &mut sut,
        &mut world,
        Event::ShareAgain {
            address: g.address.clone(),
            owner: me,
        },
    );
    assert_eq!(
        sut.view().share_again[&g.address.to_ascii_lowercase()],
        Phase::Done
    );
    assert_eq!(world.passkeys.len(), 1);
    assert_eq!(world.imports[0]["chainId"], 100);
    assert_eq!(world.imports[0]["genesis"]["name"], "Quiet Harbor");
    assert_eq!(world.stored_accounts()[0]["shared"], true);
}

// -- checked again (recheck) ---------------------------------------------------------

/// Once a session per network; a network that does not answer changes
/// nothing and is asked again; one this device lacks is not read.
#[test]
fn rechecked_once_a_session_and_again_where_unanswered() {
    let g = genesis(&[A], 1, "0x15");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100, 8453, 42161], true)]).to_string(),
    );
    world.silent.insert(8453);
    world
        .canonical_missing
        .insert((100, HANDLER_LOWER.to_owned()));
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, Event::Recheck);
    assert_eq!(
        world.reads_on(42161),
        0,
        "a network this device lacks is not read"
    );
    assert_eq!(sut.view().accounts[0].refused_on, vec![100]);
    let (on_100, on_8453) = (world.reads_on(100), world.reads_on(8453));

    world.silent.clear();
    run(&mut sut, &mut world, Event::Recheck);
    assert_eq!(world.reads_on(100), on_100, "checked once a session");
    assert!(world.reads_on(8453) > on_8453, "asked again");
    assert_eq!(sut.view().accounts[0].refused_on, vec![100]);
    assert_eq!(
        world.stored_accounts()[0]["findings"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "written to the stored row"
    );
}

/// Deployed with a module since: marked high risk; the chain is the
/// authority — a risk gone from it is no longer said.
#[test]
fn a_module_added_since_is_found_and_one_gone_is_not_said() {
    let g = genesis(&[A], 1, "0x20");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut safe = DeployedSafe::clean(&[A], 1);
    safe.modules = vec!["0x00000000000000000000000000000000000000ee".to_owned()];
    world.deploy(100, &g.address, safe);
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::Recheck);
    assert_eq!(sut.view().accounts[0].high_risk_on, vec![100]);

    // A new session (a fresh machine): the module is gone on chain.
    world.safe_mut(100, &g.address).modules.clear();
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::Recheck);
    assert!(sut.view().accounts[0].high_risk_on.is_empty());
}

// -- networks another member made it used on (discoverChains) -------------------------------

/// Found once a session where it is deployed; looked for once an hour on the device.
#[test]
fn networks_it_was_deployed_on_elsewhere_are_found_once() {
    let g = genesis(&[A], 1, "0x21");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    world.deploy(8453, &g.address, DeployedSafe::clean(&[A], 1));
    let mut sut = started(&mut world, vec![100, 8453, 10]);
    let discover = || Event::Discover {
        address: g.address.clone(),
        chain_ids: vec![100, 8453, 10],
    };
    run(&mut sut, &mut world, discover());
    assert_eq!(world.stored_accounts()[0]["chainIds"], json!([100, 8453]));
    assert!(world.store.contains_key(keys::DISCOVERED));
    assert!(
        !world.http_log.iter().any(|l| l.contains("/api/proposals")),
        "no mailbox read per network"
    );

    // A reload within the hour (a fresh machine, the same device): not read again.
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100, 8453, 10]);
    let reads = world.rpc_log.len();
    run(&mut sut, &mut world, discover());
    assert_eq!(world.rpc_log.len(), reads);
}

// -- importing from Safe{Wallet} (accounts-import) ----------------------------------------

fn export_of(address: &str, chain: u64, deployed: bool, props: Option<Value>) -> String {
    let mut data = json!({"addedSafes": {}, "undeployedSafes": {}, "addressBook": {}});
    if deployed {
        data["addedSafes"][chain.to_string()][address] =
            json!({"owners": [{"value": A}], "threshold": 1});
    } else {
        data["undeployedSafes"][chain.to_string()][address] = json!({"props": props});
    }
    data["addressBook"][chain.to_string()][address] = json!("Ops");
    json!({"version": "3.0", "data": data}).to_string()
}

/// A Verified genesis from the service is imported, with the address book's name.
#[test]
fn a_verified_safe_is_imported_with_its_label() {
    let g = genesis(&[A], 1, "0x22");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&g.address, 100, true, None),
        },
    );
    assert_eq!(sut.view().import.safes.len(), 1);
    // Listed as deployed, the chain shows it deployed.
    world.deploy(100, &g.address, DeployedSafe::clean(&[A], 1));
    run(
        &mut sut,
        &mut world,
        Event::Import {
            address: g.address.clone(),
            accept_risks: false,
        },
    );
    assert_eq!(
        sut.view().import.outcomes[&g.address.to_ascii_lowercase()],
        ImportOutcome::Imported
    );
    assert_eq!(world.stored_accounts()[0]["label"], "Ops");
}

/// On no network this wallet can read: not imported unchecked.
#[test]
fn a_safe_on_no_readable_network_is_unverified() {
    let g = genesis(&[A], 1, "0x23");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&g.address, 137, true, None),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Import {
            address: g.address.clone(),
            accept_risks: false,
        },
    );
    assert_eq!(
        sut.view().import.outcomes[&g.address.to_ascii_lowercase()],
        ImportOutcome::Unverified { chain_id: 137 }
    );
    assert!(world.stored_accounts().is_empty());
}

// -- the rest ---------------------------------------------------------------------------

/// Another machine's change is applied and written; removing drops it.
#[test]
fn another_machines_update_is_applied() {
    let g = genesis(&[A], 1, "0x16");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    world
        .store
        .insert(keys::ACTIVE.to_owned(), g.address.clone());
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::Apply {
            address: g.address.clone(),
            update: AccountUpdate::UseChain { chain_id: 8453 },
        },
    );
    assert_eq!(world.stored_accounts()[0]["chainIds"], json!([100, 8453]));
    run(
        &mut sut,
        &mut world,
        Event::Remove {
            address: g.address.clone(),
        },
    );
    assert!(world.stored_accounts().is_empty());
    assert_eq!(sut.view().active, None);
    assert!(!world.store.contains_key(keys::ACTIVE));
}

/// ens-name: a name with no resolver points nowhere; Ethereum not answering
/// is said as that; a lookalike is not looked up.
#[test]
fn ens_names_are_read_on_ethereum() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![1]);
    run(
        &mut sut,
        &mut world,
        Event::ResolveEns {
            name: "vіtalik.eth".to_owned(),
        },
    );
    assert!(matches!(
        sut.view().add.ens,
        Some(EnsAnswer::Unsupported { .. })
    ));
    world.rpc_hooks.push(Box::new(|_, method, _| {
        (method == "eth_call").then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcResult {
                result: json!(format!("0x{}", "0".repeat(64))),
            }
        })
    }));
    run(
        &mut sut,
        &mut world,
        Event::ResolveEns {
            name: "Nobody.eth".to_owned(),
        },
    );
    assert_eq!(
        sut.view().add.ens,
        Some(EnsAnswer::None {
            name: "nobody.eth".to_owned()
        })
    );
    world.rpc_hooks.clear();
    world.silent.insert(1);
    run(
        &mut sut,
        &mut world,
        Event::ResolveEns {
            name: "vitalik.eth".to_owned(),
        },
    );
    assert!(matches!(
        sut.view().add.ens,
        Some(EnsAnswer::Unavailable { .. })
    ));
}

/// The record's backup on Ethereum: checked, and written by a Vela account.
#[test]
fn the_backup_is_checked_and_sent() {
    let g = genesis(&[A], 1, "0x24");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![1, 100]);
    run(
        &mut sut,
        &mut world,
        Event::CheckBackup {
            address: g.address.clone(),
        },
    );
    // No registry on this Ethereum: unavailable.
    assert_eq!(
        sut.view().backup[&g.address.to_ascii_lowercase()],
        BackupState::Unavailable
    );
    run(
        &mut sut,
        &mut world,
        Event::SaveToEthereum {
            address: g.address.clone(),
            from: A.to_owned(),
        },
    );
    assert_eq!(world.sent.len(), 1);
    assert_eq!(world.sent[0].0, 1);
    assert_eq!(
        world.sent[0].2[0]["to"]
            .as_str()
            .unwrap()
            .to_ascii_lowercase(),
        vela_core::multisig::GENESIS_REGISTRY.to_ascii_lowercase()
    );
    // The shell is told what it is for: the signing sheet, never an execution.
    assert_eq!(
        world.purposes,
        vec![Some(vela_core::app::multisig_shell::SendPurpose::Record {
            multisig: g.address.clone()
        })]
    );
}

/// A voiding transaction at nonce 0: a proposal any mailbox could hold.
fn cancel(safe: &str) -> Value {
    serde_json::to_value(vela_core::multisig::cancel_tx(safe, "0x0").unwrap()).unwrap()
}

// -- r7: a member's kind, declared then verified (member-kind, signers) -----------------

const SAFE_MEMBER: &str = "0x00000000000000000000000000000000000000cc";
const VELA_ELSEWHERE: &str = "0x00000000000000000000000000000000000000dd";

fn ask(address: &str, chain_ids: &[u64]) -> KindAsk {
    KindAsk {
        address: address.to_owned(),
        chain_ids: chain_ids.to_vec(),
    }
}

/// The create sheet's members, read on the networks it may be used on: an
/// account here (found), another Safe (code, Vela's signer not configured:
/// a smart contract account), a Vela account deployed elsewhere (its signer
/// configured there, no index record needed), and a plain address ("Other
/// wallet" only hinted).
#[test]
fn the_create_sheets_members_are_found_contract_or_hinted_from_the_chain() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    world
        .codes
        .insert((8453, SAFE_MEMBER.to_owned()), "0x6080".to_owned());
    world.configured.insert((1, VELA_ELSEWHERE.to_owned()));
    let mut sut = started(&mut world, vec![100, 8453]);
    let me = world.locals[0].address.clone();
    run(
        &mut sut,
        &mut world,
        Event::ReadKinds {
            members: vec![
                ask(&me, &[100, 8453]),
                ask(SAFE_MEMBER, &[100, 8453]),
                ask(VELA_ELSEWHERE, &[100, 8453]),
                ask(B, &[100, 8453]),
            ],
        },
    );
    let kinds = sut.view().member_kinds;
    let of = |a: &str| kinds[&a.to_ascii_lowercase()];
    assert_eq!(of(&me).kind, Some(MemberKind::Vela));
    assert_eq!(of(&me).note, Some(KindNote::Known));
    assert_eq!(of(SAFE_MEMBER).kind, Some(MemberKind::Contract));
    assert_eq!(of(SAFE_MEMBER).note, Some(KindNote::Contract));
    assert!(of(SAFE_MEMBER).locked);
    assert_eq!(of(VELA_ELSEWHERE).kind, Some(MemberKind::Vela));
    assert_eq!(of(VELA_ELSEWHERE).note, Some(KindNote::Known));
    assert_eq!(of(B).kind, None);
    assert_eq!(of(B).suggested, Some(MemberKind::Wallet));
    assert_eq!(of(B).note, Some(KindNote::Likely));
    // An account here is never read.
    assert!(!world.rpc_log.iter().any(|(_, _, p)| p
        .to_string()
        .to_ascii_lowercase()
        .contains(&me[2..].to_ascii_lowercase())));
}

/// The person's pick stands where the chain has no finding, never over it;
/// creating keeps each member's kind as chosen — and one left unsaid keeps
/// nothing.
#[test]
fn the_persons_pick_is_kept_with_the_new_multisig_where_evidence_allows() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    world
        .codes
        .insert((8453, SAFE_MEMBER.to_owned()), "0x6080".to_owned());
    let mut sut = started(&mut world, vec![100, 8453]);
    let me = world.locals[0].address.clone();
    run(
        &mut sut,
        &mut world,
        Event::ReadKinds {
            members: vec![ask(SAFE_MEMBER, &[8453]), ask(B, &[8453]), ask(A, &[8453])],
        },
    );
    for (who, kind) in [(B, MemberKind::Wallet), (SAFE_MEMBER, MemberKind::Vela)] {
        run(
            &mut sut,
            &mut world,
            Event::PickKind {
                address: who.to_owned(),
                kind: Some(kind),
            },
        );
    }
    let kinds = sut.view().member_kinds;
    assert_eq!(kinds[B].kind, Some(MemberKind::Wallet), "the person's word");
    assert_eq!(kinds[B].note, None);
    assert_eq!(
        kinds[SAFE_MEMBER].kind,
        Some(MemberKind::Contract),
        "evidence beats the label"
    );
    run(&mut sut, &mut world, Event::NewDraft);
    let c = CreateChoice {
        owners: vec![
            me.clone(),
            SAFE_MEMBER.to_owned(),
            B.to_owned(),
            A.to_owned(),
        ],
        threshold: 2,
        name: "Quiet Harbor".to_owned(),
        chain_id: 8453,
        creator: me.clone(),
        members: BTreeMap::from([(
            B.to_owned(),
            MemberInfo {
                name: Some("Bo".to_owned()),
                kind: None,
                typed: None,
            },
        )]),
    };
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(sut.view().create.phase, Phase::Done);
    let members = &world.stored_accounts()[0]["members"];
    assert_eq!(members[me.to_ascii_lowercase()]["kind"], "passkey");
    assert_eq!(members[SAFE_MEMBER]["kind"], "contract");
    assert_eq!(members[B]["kind"], "wallet");
    assert_eq!(members[B]["name"], "Bo");
    assert!(
        members.get(A).is_none_or(|m| m.get("kind").is_none()),
        "unsaid keeps nothing: {members}"
    );
}

/// A Safe{Wallet} import keeps what each member was declared as (or Vela
/// found), as the person's word.
#[test]
fn an_import_keeps_its_members_kinds() {
    let g = genesis(&[A], 1, "0x22");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.configured.insert((100, A.to_owned()));
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&g.address, 100, true, None),
        },
    );
    world.deploy(100, &g.address, DeployedSafe::clean(&[A], 1));
    run(
        &mut sut,
        &mut world,
        Event::ReadKinds {
            members: vec![ask(A, &[100])],
        },
    );
    assert_eq!(
        sut.view().member_kinds[A].note,
        Some(KindNote::Known),
        "Vela's signer configured for it: a Vela account"
    );
    run(
        &mut sut,
        &mut world,
        Event::Import {
            address: g.address.clone(),
            accept_risks: false,
        },
    );
    assert_eq!(
        sut.view().import.outcomes[&g.address.to_ascii_lowercase()],
        ImportOutcome::Imported
    );
    let members = &world.stored_accounts()[0]["members"];
    assert_eq!(members[A]["kind"], "passkey", "{members}");
}
