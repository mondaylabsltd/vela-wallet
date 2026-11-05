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
    AddOutcome, BackupState, CreateChoice, CreateDraft, EnsAnswer, Event, ImportOutcome,
    IndexAnswer, InvitedMember, KindAsk, MultisigAccounts, Phase, RenameBlocked, CREATE_DRAFT_KEY,
    INVITE_SHARED_KEY,
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
    let c_name = c.name.clone();
    run(&mut sut, &mut world, Event::Create { choice: c });
    let view = sut.view();
    assert_eq!(view.create.phase, Phase::Done, "{:?}", view.create);
    assert_eq!(world.passkeys.len(), 1, "one passkey prompt");
    let address = view.create.address.clone().unwrap();
    assert_eq!(address, drafted, "the address the review showed");
    // §16: the proof carries what a trusted page reads it from — the
    // genesis — and the page's reading of it is this multisig, this creator,
    // and exactly the challenge the passkey was asked for.
    let purpose = &world.passkey_purposes[0];
    assert_eq!(purpose["kind"], "record");
    let reading = vela_core::trusted_signer::owner_proof::read(&purpose["params"]).unwrap();
    assert_eq!(reading.multisig, address);
    assert!(reading.member.eq_ignore_ascii_case(&world.passkeys[0].0));
    assert_eq!(reading.challenge, world.passkeys[0].1);
    assert_eq!(reading.threshold, 2);
    assert_eq!(reading.name.as_deref(), Some(c_name.as_str()));
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

/// r14-5: every node of the network answering "slow down" (HTTP 429), as the
/// shell's pool reports it.
fn rate_limited(on: u64) -> world::RpcHook {
    Box::new(move |chain, _, _| {
        (chain == on).then(
            || vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: format!(
                    "{}: every node of chain {on} answered HTTP 429",
                    vela_core::app::multisig_shell::RATE_LIMITED_DETAIL
                ),
            },
        )
    })
}

/// r14-5: a network rate-limiting every read an add needs is waited out
/// once (a bounded pause), then named — busy, with Try again — and nothing
/// is added. Never a silent return, never "couldn't reach the service".
#[test]
fn a_network_rate_limiting_the_add_is_named_busy_after_one_pause() {
    let g = genesis(&[A], 1, "0x1c");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.deploy(8453, &g.address, DeployedSafe::clean(&[A], 1));
    world.rpc_hooks.push(rate_limited(8453));
    let mut sut = started(&mut world, vec![8453]);
    let before = world.now;
    run(&mut sut, &mut world, add(&g.address, Some(8453), false));
    let view = sut.view();
    assert_eq!(view.add.outcome, Some(AddOutcome::Busy { chain_id: 8453 }));
    assert!(view.add.retry, "Try again is offered");
    assert_eq!(view.add.phase, Phase::Done);
    assert!(world.stored_accounts().is_empty(), "nothing added");
    let lookups = world
        .rpc_log
        .iter()
        .filter(|(c, m, p)| {
            *c == 8453 && m == "eth_getCode" && p[0].as_str() == Some(g.address.as_str())
        })
        .count();
    assert_eq!(
        lookups, 2,
        "asked once more after the pause, not again and again"
    );
    assert!(
        world.now - before >= vela_core::app::multisig_accounts::RATE_LIMIT_PAUSE_MS as f64,
        "the second ask waited the pause"
    );
}

/// r14-5: a limit that lifts during the pause is no failure: the second ask
/// adds it, and the person never saw a thing go wrong.
#[test]
fn a_rate_limit_that_lifts_within_the_pause_adds() {
    let g = genesis(&[A], 1, "0x1d");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.deploy(8453, &g.address, DeployedSafe::clean(&[A], 1));
    // Limited until the first ask has failed: every read before the second
    // lookup of the address itself.
    let mut lookups = 0;
    let address = g.address.clone();
    let mut limit = rate_limited(8453);
    world.rpc_hooks.push(Box::new(move |chain, method, params| {
        if chain == 8453 && method == "eth_getCode" && params[0].as_str() == Some(address.as_str())
        {
            lookups += 1;
        }
        if lookups >= 2 {
            return None;
        }
        limit(chain, method, params)
    }));
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, add(&g.address, Some(8453), false));
    assert_eq!(
        sut.view().add.outcome,
        Some(AddOutcome::Added {
            address: g.address.clone()
        })
    );
    assert!(!sut.view().add.retry);
    assert_eq!(world.stored_accounts().len(), 1);
}

/// r14-5: a network that does not answer at all (no limit said) keeps its
/// own words — busy is only for the nodes that said "slow down".
#[test]
fn a_silent_network_is_not_called_busy() {
    let mut world = World::default();
    world.silent.insert(8453);
    let mut sut = started(&mut world, vec![100, 8453]);
    run(
        &mut sut,
        &mut world,
        add("0x00000000000000000000000000000000000000cf", None, false),
    );
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::Unchecked));
    assert!(
        sut.view().add.retry,
        "a read that did not come back is retried"
    );
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
            chain_ids: None,
            wait_ms: None,
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
    // §16: a page reads this multisig, this owner, its networks and the
    // challenge asked for from the params the proof carries.
    let reading =
        vela_core::trusted_signer::owner_proof::read(&world.passkey_purposes[0]["params"]).unwrap();
    assert_eq!(reading.multisig, g.address);
    assert_eq!(reading.challenge, world.passkeys[0].1);
    assert_eq!(reading.chain_ids, vec![8453]);
    assert_eq!(reading.owners.len(), 2);
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

/// r14 leftovers (§21): an import whose network answered only "slow down"
/// is waited out once, then said as busy — the network named, Try again
/// offered (`import.retry`) — never "unavailable", and nothing is kept.
#[test]
fn a_rate_limited_import_is_named_busy_with_try_again() {
    let g = genesis(&[A], 1, "0x24");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world.deploy(100, &g.address, DeployedSafe::clean(&[A], 1));
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&g.address, 100, true, None),
        },
    );
    world.rpc_hooks.push(rate_limited(100));
    let before = world.now;
    let key = g.address.to_ascii_lowercase();
    let import = Event::Import {
        address: g.address.clone(),
        accept_risks: false,
    };
    run(&mut sut, &mut world, import.clone());
    let view = sut.view();
    assert_eq!(
        view.import.outcomes[&key],
        ImportOutcome::Busy { chain_id: 100 }
    );
    assert_eq!(view.import.retry, vec![key.clone()], "Try again is offered");
    assert!(view.import.busy.is_none());
    assert!(world.stored_accounts().is_empty(), "nothing imported");
    assert!(
        world.now - before >= vela_core::app::multisig_accounts::RATE_LIMIT_PAUSE_MS as f64,
        "asked once more after the pause"
    );

    // The limit lifts: Try again (the same `import`) imports it.
    world.rpc_hooks.clear();
    run(&mut sut, &mut world, import);
    let view = sut.view();
    assert_eq!(view.import.outcomes[&key], ImportOutcome::Imported);
    assert!(view.import.retry.is_empty(), "nothing left to try again");
    assert_eq!(world.stored_accounts().len(), 1);
}

/// r14 leftovers: what an import comes to offers Try again exactly when a
/// read did not come back — the add sheet's rule, for every shell.
#[test]
fn an_import_offers_try_again_only_for_a_read_that_did_not_come_back() {
    for (outcome, retry) in [
        (ImportOutcome::Busy { chain_id: 1 }, true),
        (ImportOutcome::Unavailable, true),
        (ImportOutcome::Unverified { chain_id: 1 }, true),
        (ImportOutcome::Imported, false),
        (ImportOutcome::NotFound, false),
        (ImportOutcome::Unsupported, false),
        (ImportOutcome::NotDeployed, false),
        (ImportOutcome::Refused { findings: vec![] }, false),
        (ImportOutcome::HighRisk { findings: vec![] }, false),
    ] {
        assert_eq!(outcome.offers_retry(), retry, "{outcome:?}");
    }
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

/// r8-7: a member added by a member change is on this device: the members
/// on it are its members as the chains say now, kept across a reload.
#[test]
fn a_member_added_since_is_on_this_device() {
    let g = genesis(&[A], 1, "0x17");
    let added = fixture(0);
    let mut world = World {
        locals: vec![added.clone()],
        ..World::default()
    };
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    assert!(
        sut.view().accounts[0].local_members.is_empty(),
        "not a founding member"
    );
    run(
        &mut sut,
        &mut world,
        Event::Apply {
            address: g.address.clone(),
            update: AccountUpdate::MembersNow {
                members: vec![A.to_owned(), added.address.clone()],
            },
        },
    );
    assert_eq!(
        sut.view().accounts[0].local_members,
        vec![added.address.clone()]
    );
    let again = started(&mut world, vec![100]);
    assert_eq!(
        again.view().accounts[0].local_members,
        vec![added.address.clone()],
        "kept across a reload"
    );
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

// -- the web's facade (W8-M1) --------------------------------------------------------

/// A change asked for before the store answered writes nothing: the list is
/// never written over one not read yet (the web's facade queues until then).
#[test]
fn nothing_is_written_over_a_list_not_read_yet() {
    let g = genesis(&[A], 1, "0x30");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = Sut::new();
    sut.dispatch(Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: Vec::new(),
        networks: vec![100],
    });
    sut.dispatch(Event::SetLabel {
        address: g.address.clone(),
        label: Some("Early".to_owned()),
    });
    sut.dispatch(Event::Remove {
        address: "0x00000000000000000000000000000000000000ee".to_owned(),
    });
    settle(&mut sut, &mut world, 5_000);
    assert!(sut.view().loaded);
    assert_eq!(world.stored_accounts().len(), 1, "the stored list stands");
    assert_eq!(sut.view().accounts.len(), 1);
}

/// Another document changed the stored list: read again, nothing asked.
#[test]
fn another_documents_change_is_read_again() {
    let g = genesis(&[A], 1, "0x31");
    let h = genesis(&[B], 1, "0x32");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    assert_eq!(sut.view().accounts.len(), 1);
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true), stored_row(&h, &[100], true)]).to_string(),
    );
    world
        .store
        .insert(keys::ACTIVE.to_owned(), h.address.clone());
    let (http, rpc) = (world.http_log.len(), world.rpc_log.len());
    run(&mut sut, &mut world, Event::Reload);
    let view = sut.view();
    assert_eq!(view.accounts.len(), 2);
    assert_eq!(view.active.as_deref(), Some(h.address.as_str()));
    assert_eq!(world.http_log.len(), http, "nothing asked of the service");
    assert_eq!(world.rpc_log.len(), rpc, "nor of a chain");
}

/// An address is looked up on the space in front; the device's other
/// networks are not read for it.
#[test]
fn an_address_is_looked_up_on_the_space_in_front() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100, 11_155_111]);
    run(
        &mut sut,
        &mut world,
        Event::Space {
            chain_ids: vec![100],
        },
    );
    run(
        &mut sut,
        &mut world,
        add("0x00000000000000000000000000000000000000dd", None, false),
    );
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::NotFound));
    assert!(world.reads_on(100) > 0);
    assert_eq!(world.reads_on(11_155_111), 0, "the other space is not read");
}

/// A proposal link's lookup reads its own network only.
#[test]
fn a_find_reads_the_networks_it_names() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![100, 8453]);
    run(
        &mut sut,
        &mut world,
        Event::Find {
            address: "0x00000000000000000000000000000000000000dd".to_owned(),
            network: Some(8453),
            chain_ids: Some(vec![8453]),
            wait_ms: Some(2500),
        },
    );
    assert_eq!(sut.view().add.outcome, Some(AddOutcome::NotFound));
    assert_eq!(world.reads_on(100), 0);
    assert!(world.reads_on(8453) > 0);
}

/// r4a-12: why a record cannot be renamed here, each said as itself; one
/// held on a network here names it, with whether it is deployed there.
#[test]
fn a_rename_says_why_it_cannot_run() {
    let g = genesis(&[A], 1, "0x33");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], false)]).to_string(),
    );
    let plan = |sut: &Sut| sut.view().rename[&g.address.to_ascii_lowercase()].clone();
    let rename = || Event::PlanRename {
        address: g.address.clone(),
    };
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, rename());
    assert_eq!(plan(&sut).blocked, Some(RenameBlocked::Unrecorded));
    assert_eq!(plan(&sut).chain_id, None);

    world.record(&g.address, &g.genesis, true);
    run(&mut sut, &mut world, rename());
    assert_eq!(
        plan(&sut).blocked,
        Some(RenameBlocked::Pending),
        "r7-10: a record on its way is not one never saved"
    );
    assert_eq!(plan(&sut).chain_id, None, "and names no chain yet");
    assert_eq!(plan(&sut).lands_on, Some(100), "it lands on the home chain");

    world.record(&g.address, &g.genesis, false);
    run(&mut sut, &mut world, rename());
    assert_eq!(plan(&sut).blocked, Some(RenameBlocked::Elsewhere));

    world.service_down = true;
    run(&mut sut, &mut world, rename());
    assert_eq!(plan(&sut).blocked, Some(RenameBlocked::Unreachable));

    world.service_down = false;
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, rename());
    assert_eq!(plan(&sut).chain_id, Some(100));
    assert_eq!(plan(&sut).deployed, Some(false));
    assert_eq!(plan(&sut).blocked, None);
}

/// Sharing again: a passkey that failed says why; a service that does not
/// hold it says nothing more than that it failed.
#[test]
fn sharing_again_says_why_a_passkey_failed() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let g = genesis(&[&me, B], 2, "0x34");
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[8453], false)]).to_string(),
    );
    world.passkey_fails = Some("{\"kind\":\"page_declined\"}".to_owned());
    let mut sut = started(&mut world, vec![8453]);
    let key = g.address.to_ascii_lowercase();
    let share = || Event::ShareAgain {
        address: g.address.clone(),
        owner: me.clone(),
    };
    run(&mut sut, &mut world, share());
    assert_eq!(sut.view().share_again[&key], Phase::Failed);
    assert_eq!(
        sut.view().share_again_failure.get(&key).map(String::as_str),
        Some("{\"kind\":\"page_declined\"}")
    );

    world.passkey_fails = None;
    world.service_down = true;
    run(&mut sut, &mut world, share());
    assert_eq!(sut.view().share_again[&key], Phase::Failed);
    assert!(!sut.view().share_again_failure.contains_key(&key));
}

/// A new draft is a fresh sheet at once: what the sheet asked right after
/// (its networks) is not wiped when the salt answers.
#[test]
fn a_new_draft_keeps_what_was_asked_after_it() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let mut sut = started(&mut world, vec![100]);
    sut.dispatch(Event::NewDraft);
    sut.dispatch(Event::CheckNetworks {
        chain_ids: vec![100],
    });
    sut.dispatch(Event::Draft {
        owners: vec![me],
        threshold: 1,
        name: "Quiet Harbor".to_owned(),
    });
    settle(&mut sut, &mut world, 5_000);
    let view = sut.view();
    assert!(view.create.networks.contains_key(&100));
    assert!(view.create.draft_address.is_some());
}

/// r4D-5 (the web's `ens-name` suite): a name's address is read on Ethereum
/// — the registry's resolver, then its `addr` — and given back
/// checksummed; a resolver with no address for it points nowhere.
#[test]
fn an_ens_names_address_is_read_and_checksummed() {
    use std::cell::RefCell;
    use std::rc::Rc;
    use vela_core::app::multisig_shell::MultisigShellResult as Res;
    let resolver = format!(
        "0x{}{}",
        "0".repeat(24),
        "4976fb03c32e5b8cfe2b6ccb31c09ba78ebaba41"
    );
    let addr = Rc::new(RefCell::new(format!(
        "0x{}{}",
        "0".repeat(24),
        "d8da6bf26964af9d7eed9e03e53415d37aa96045"
    )));
    let calls = Rc::new(RefCell::new(Vec::<(u64, String)>::new()));
    let mut world = World::default();
    let mut sut = started(&mut world, vec![1]);
    {
        let (addr, calls) = (addr.clone(), calls.clone());
        world.rpc_hooks.push(Box::new(move |chain, method, params| {
            if method != "eth_call" {
                return None;
            }
            let to = params[0]["to"].as_str().unwrap_or("").to_owned();
            let data = params[0]["data"].as_str().unwrap_or("").to_owned();
            calls.borrow_mut().push((chain, to));
            Some(Res::RpcResult {
                result: json!(if data.starts_with("0x0178b8bf") {
                    resolver.clone()
                } else {
                    addr.borrow().clone()
                }),
            })
        }));
    }
    run(
        &mut sut,
        &mut world,
        Event::ResolveEns {
            name: "Vitalik.eth".to_owned(),
        },
    );
    assert_eq!(
        sut.view().add.ens,
        Some(EnsAnswer::Found {
            name: "vitalik.eth".to_owned(),
            address: "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045".to_owned(),
        })
    );
    let asked = calls.borrow().clone();
    assert_eq!(
        asked.iter().map(|(c, _)| *c).collect::<Vec<_>>(),
        vec![1, 1]
    );
    assert_eq!(asked[0].1, "0x00000000000C2E074eC69A0dFb2997BA6C7d2e1e");

    *addr.borrow_mut() = format!("0x{}", "0".repeat(64));
    run(
        &mut sut,
        &mut world,
        Event::ResolveEns {
            name: "nobody.eth".to_owned(),
        },
    );
    assert_eq!(
        sut.view().add.ens,
        Some(EnsAnswer::None {
            name: "nobody.eth".to_owned()
        })
    );
}

/// r4D-5 (the web's `invite` suite): one added since is kept once a network
/// the invite names says it is a member — a network that cannot be read
/// keeps only the founding members' — and a link from before r6 names them
/// by the genesis's order, only when the counts match.
#[test]
fn an_invites_labels_for_members_added_since_and_by_order() {
    const C: &str = "0x00000000000000000000000000000000000000cc";
    const STRANGER: &str = "0x00000000000000000000000000000000000000dd";
    let g = genesis(&[A, B], 2, "0x35");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    world.deploy(100, &g.address, DeployedSafe::clean(&[A, B, C], 2));
    let mut sut = started(&mut world, vec![100]);
    let m = |a: &str, n: &str| InvitedMember {
        address: Some(a.to_owned()),
        name: Some(n.to_owned()),
        kind: Some("passkey".to_owned()),
    };
    let invite = |members: Vec<InvitedMember>| Event::KeepInvited {
        address: g.address.clone(),
        chain_ids: vec![100],
        members,
    };
    world.silent.insert(100);
    run(&mut sut, &mut world, invite(vec![m(A, "Ana"), m(C, "Cy")]));
    let members = world.stored_accounts()[0]["members"].clone();
    assert_eq!(members[A]["name"], "Ana");
    assert!(members.get(C).is_none(), "the network did not say");

    world.silent.clear();
    run(
        &mut sut,
        &mut world,
        invite(vec![m(C, "Cy"), m(STRANGER, "Ana")]),
    );
    let members = world.stored_accounts()[0]["members"].clone();
    assert_eq!(members[C]["name"], "Cy");
    assert!(members.get(STRANGER).is_none(), "not a member anywhere");

    // Before r6: by the genesis's order, only when the counts match.
    let h = genesis(&[A, B], 2, "0x36");
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&h, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    let unaddressed = |name: Option<&str>, kind: Option<&str>| InvitedMember {
        address: None,
        name: name.map(str::to_owned),
        kind: kind.map(str::to_owned),
    };
    let keep = |members| Event::KeepInvited {
        address: h.address.clone(),
        chain_ids: vec![100],
        members,
    };
    run(
        &mut sut,
        &mut world,
        keep(vec![unaddressed(Some("Ana"), None)]),
    );
    assert!(world.stored_accounts()[0].get("members").is_none());
    run(
        &mut sut,
        &mut world,
        keep(vec![
            unaddressed(Some("Ana"), None),
            unaddressed(Some("Bo"), Some("wallet")),
        ]),
    );
    let members = world.stored_accounts()[0]["members"].clone();
    assert_eq!(members[A]["name"], "Ana");
    assert_eq!(members[B]["name"], "Bo");
    assert_eq!(members[B]["kind"], "wallet");
}

// -- ported from the web's suites (W8-M1) ---------------------------------------------------

/// accounts-records: a record's name is the chain's own record where this
/// device has the registry's network — a rename shows here, the service's
/// word only where the chain cannot be read.
#[test]
fn a_records_name_is_read_from_the_chain_where_it_can_be() {
    use vela_core::app::multisig_shell::MultisigShellResult as Res;
    let owners = [
        "0x1111111111111111111111111111111111111111".to_owned(),
        "0x2222222222222222222222222222222222222222".to_owned(),
    ];
    let ms = vela_core::multisig::multisig_genesis(&owners, 2, "Team fund", "0x7").unwrap();
    let ret = include_str!("vectors/multisig-genesis-of.hex")
        .trim()
        .to_owned();
    let mut stored = ms.genesis.clone();
    stored.name = Some("Old name".to_owned());
    let mut service = ms.genesis.clone();
    service.name = Some("Service word".to_owned());
    let mut world = World::default();
    world.record(&ms.address, &service, false);
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([{
            "address": ms.address, "genesis": stored, "owners": ms.owners,
            "threshold": ms.threshold, "recorded": true, "chainIds": [100], "addedMs": 1,
        }])
        .to_string(),
    );
    let registry = vela_core::multisig::GENESIS_REGISTRY.to_ascii_lowercase();
    world.rpc_hooks.push(Box::new(move |chain, method, params| {
        let to = params[0]["to"].as_str().unwrap_or("").to_ascii_lowercase();
        (chain == 100 && method == "eth_call" && to == registry).then(|| Res::RpcResult {
            result: json!(ret.clone()),
        })
    }));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::CheckRecords);
    assert_eq!(world.stored_accounts()[0]["genesis"]["name"], "Team fund");
}

/// accounts-records: a name that comes with a genesis that is not this
/// address is no record of it; a record queued moments ago is given the
/// time to land (not called unshared), and looked at again.
#[test]
fn a_record_for_another_address_or_still_landing_changes_nothing() {
    let g = genesis(&[A], 1, "0x37");
    let other = genesis(&[B], 1, "0x38");
    let mut world = World::default();
    let mut wrong = other.genesis.clone();
    wrong.name = Some("Treasury".to_owned());
    world.record(&g.address, &wrong, false);
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, Event::CheckRecords);
    assert_eq!(
        world.stored_accounts()[0]["genesis"]["name"],
        "Quiet Harbor"
    );

    // Added a moment ago, the service has none yet: still shared, asked again.
    let h = genesis(&[A], 1, "0x39");
    let mut row = stored_row(&h, &[100], true);
    row["addedMs"] = json!(world.now - 1_000.0);
    world
        .store
        .insert(keys::ACCOUNTS.to_owned(), json!([row]).to_string());
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::CheckRecords);
    assert_eq!(world.stored_accounts()[0]["recorded"], true);
    let asked = world.http_log.len();
    run(&mut sut, &mut world, Event::CheckRecords);
    assert!(world.http_log.len() > asked, "looked at again");
}

/// accounts-find: a record goes to the default service's home (Gnosis)
/// only when the service in use names none.
#[test]
fn a_record_goes_to_gnosis_when_the_service_names_no_home() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    world.home_chain = None;
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, Event::NewDraft);
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(world.imports[0]["chainId"], 100);

    let mut world = World::default();
    world.locals.push(fixture(0));
    world.home_chain = Some(5_042_002);
    let mut sut = started(&mut world, vec![8453]);
    run(&mut sut, &mut world, Event::NewDraft);
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(
        world.imports[0]["chainId"], 5_042_002,
        "the service's own home"
    );
}

/// accounts-import: a refused genesis is not imported and says why; one
/// deployed with a module only on the person's tap, keeping what was found.
#[test]
fn an_import_is_vetted_as_an_address_added_is() {
    let g = genesis(&[A], 1, "0x3a");
    let mut world = World::default();
    world.record(&g.address, &g.genesis, false);
    world
        .canonical_missing
        .insert((100, HANDLER_LOWER.to_owned()));
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&g.address, 100, true, None),
        },
    );
    let import = |accept_risks| Event::Import {
        address: g.address.clone(),
        accept_risks,
    };
    run(&mut sut, &mut world, import(false));
    let key = g.address.to_ascii_lowercase();
    let ImportOutcome::Refused { findings } = sut.view().import.outcomes[&key].clone() else {
        panic!("{:?}", sut.view().import.outcomes[&key]);
    };
    assert_eq!(findings[0]["from"], "genesis");
    assert!(world.stored_accounts().is_empty());

    let h = genesis(&[A], 1, "0x3b");
    let mut world = World::default();
    world.record(&h.address, &h.genesis, false);
    let mut safe = DeployedSafe::clean(&[A], 1);
    safe.modules = vec!["0x00000000000000000000000000000000000000ee".to_owned()];
    world.deploy(100, &h.address, safe);
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ParseExport {
            json: export_of(&h.address, 100, true, None),
        },
    );
    let import = |accept_risks| Event::Import {
        address: h.address.clone(),
        accept_risks,
    };
    run(&mut sut, &mut world, import(false));
    let key = h.address.to_ascii_lowercase();
    assert!(matches!(
        sut.view().import.outcomes[&key],
        ImportOutcome::HighRisk { .. }
    ));
    assert!(world.stored_accounts().is_empty(), "not without the tap");
    run(&mut sut, &mut world, import(true));
    assert_eq!(sut.view().import.outcomes[&key], ImportOutcome::Imported);
    assert_eq!(
        world.stored_accounts()[0]["findings"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
}

/// accounts-records: one with no proof kept here is asked about once a
/// session, not every minute; an owner's proof the service keeps, though
/// its sponsor will not pay now, is shared.
#[test]
fn sharing_without_a_proof_is_asked_once_a_session() {
    let g = genesis(&[A], 1, "0x3c");
    let mut world = World {
        import_status: (
            200,
            json!({"status": "resolved", "reason": "notDeployed", "recordCall": {}}),
        ),
        ..World::default()
    };
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], false)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(world.imports.len(), 1);
    assert!(world.imports[0].get("ownerProof").is_none());
    assert_eq!(world.imports[0]["genesis"]["name"], "Quiet Harbor", "named");
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(world.imports.len(), 1, "once a session");

    // The owner's passkey: not paid for now, yet the service hands it out.
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let h = genesis(&[&me], 1, "0x3d");
    world.import_status = (
        200,
        json!({"status": "resolved", "reason": "budget", "recordCall": {}}),
    );
    world.record(&h.address, &h.genesis, true);
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&h, &[100], false)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    run(
        &mut sut,
        &mut world,
        Event::ShareAgain {
            address: h.address.clone(),
            owner: me,
        },
    );
    assert_eq!(
        sut.view().share_again[&h.address.to_ascii_lowercase()],
        Phase::Done
    );
    assert_eq!(world.stored_accounts()[0]["shared"], true);
}

/// recheck: once a session — and a network it comes to be used on is
/// checked then (its verdict per network, never the multisig's at once).
#[test]
fn a_network_it_comes_to_be_used_on_is_checked_then() {
    let g = genesis(&[A], 1, "0x3e");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    world.deploy(100, &g.address, DeployedSafe::clean(&[A], 1));
    let mut sut = started(&mut world, vec![100, 8453]);
    run(&mut sut, &mut world, Event::Recheck);
    let on_100 = world.reads_on(100);
    assert!(on_100 > 0);
    assert_eq!(world.reads_on(8453), 0, "not one of its networks yet");
    run(&mut sut, &mut world, Event::Recheck);
    assert_eq!(world.reads_on(100), on_100, "once a session");
    run(
        &mut sut,
        &mut world,
        Event::Apply {
            address: g.address.clone(),
            update: AccountUpdate::UseChain { chain_id: 8453 },
        },
    );
    run(&mut sut, &mut world, Event::Recheck);
    assert!(world.reads_on(8453) > 0, "checked once it is used there");
    assert_eq!(world.reads_on(100), on_100);
    assert!(sut.view().accounts[0].high_risk_on.is_empty());
    assert!(sut.view().accounts[0].refused_on.is_empty());
}

// -- r7-10: a record on its way is said as such ------------------------------------

/// Created while the service was away: its proof is kept here and asked
/// again — renamed once it lands, never "not saved".
#[test]
fn a_record_asked_for_here_is_on_its_way_not_unrecorded() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    world.service_down = true;
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    let address = sut.view().create.address.clone().unwrap();
    world.service_down = false;
    run(
        &mut sut,
        &mut world,
        Event::PlanRename {
            address: address.clone(),
        },
    );
    let plan = sut.view().rename[&address.to_ascii_lowercase()].clone();
    assert_eq!(plan.blocked, Some(RenameBlocked::Pending));
    assert_eq!(plan.lands_on, Some(100));
}

// -- §3.5: one network checked on demand --------------------------------------------

/// `CheckOn`: one network read once a session — also one it is not used on
/// yet; a network that does not answer is no verdict and is asked again.
#[test]
fn a_network_is_checked_on_demand_once_a_session() {
    let g = genesis(&[A], 1, "0x61");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    world.silent.insert(8453);
    world
        .canonical_missing
        .insert((8453, HANDLER_LOWER.to_owned()));
    let mut sut = started(&mut world, vec![100, 8453]);
    let check = || Event::CheckOn {
        address: g.address.clone(),
        chain_id: 8453,
    };
    run(&mut sut, &mut world, check());
    let row = &sut.view().accounts[0];
    assert!(world.reads_on(8453) > 0, "read, though not used there yet");
    assert!(row.checked_on.is_empty(), "no answer is no verdict");
    assert!(row.checking_on.is_empty());
    world.silent.clear();
    run(&mut sut, &mut world, check());
    let row = sut.view().accounts[0].clone();
    assert_eq!(row.checked_on, vec![8453], "asked again, and answered");
    assert_eq!(row.refused_on, vec![8453], "the same verdict rule");
    assert_eq!(
        world.stored_accounts()[0]["chainIds"],
        json!([100]),
        "not added there"
    );
    let reads = world.reads_on(8453);
    run(&mut sut, &mut world, check());
    assert_eq!(world.reads_on(8453), reads, "checked once a session");
    // The re-check counts it as checked too.
    run(&mut sut, &mut world, Event::Recheck);
    assert_eq!(world.reads_on(8453), reads);
    assert!(sut.view().accounts[0].checked_on.contains(&100));
    // A network this device lacks is not read at all.
    let before = world.reads_on(42161);
    run(
        &mut sut,
        &mut world,
        Event::CheckOn {
            address: g.address.clone(),
            chain_id: 42161,
        },
    );
    assert_eq!(world.reads_on(42161), before);
}

// -- r7-9: the create sheet --------------------------------------------------------

fn draft(world: &World) -> CreateDraft {
    let me = world.locals[0].address.clone();
    CreateDraft {
        owners: vec![me, B.to_owned()],
        members: BTreeMap::from([(
            B.to_owned(),
            MemberInfo {
                name: Some("Bo".to_owned()),
                kind: None,
                typed: None,
            },
        )]),
        name: "Quiet Harbor".to_owned(),
        threshold: 2,
        chain_id: Some(8453),
    }
}

/// Leaving the create sheet loses nothing: its draft (members, their names
/// and picked kinds, name, rule, network) is the machine’s record until it
/// is created or discarded; a new sheet does not wipe it.
#[test]
fn the_create_draft_is_kept_until_created_or_discarded() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let mut sut = started(&mut world, vec![100, 8453]);
    let d = draft(&world);
    run(&mut sut, &mut world, Event::KeepDraft { draft: d });
    run(
        &mut sut,
        &mut world,
        Event::PickKind {
            address: B.to_owned(),
            kind: Some(MemberKind::Wallet),
        },
    );
    assert!(world.store.contains_key(CREATE_DRAFT_KEY));
    // The page reloads: the sheet opens on it, the pick the person made too.
    let mut sut = started(&mut world, vec![100, 8453]);
    let kept = sut.view().create.kept.clone().unwrap();
    assert_eq!(kept.name, "Quiet Harbor");
    assert_eq!(kept.chain_id, Some(8453));
    assert_eq!(kept.members[B].name.as_deref(), Some("Bo"));
    assert_eq!(kept.members[B].kind.as_deref(), Some("wallet"));
    assert_eq!(sut.view().member_kinds[B].kind, Some(MemberKind::Wallet));
    run(&mut sut, &mut world, Event::NewDraft);
    assert!(sut.view().create.kept.is_some(), "a new sheet keeps it");
    // Discarded: gone.
    run(&mut sut, &mut world, Event::DiscardDraft);
    assert!(sut.view().create.kept.is_none());
    assert!(!world.store.contains_key(CREATE_DRAFT_KEY));
    // A blank sheet keeps nothing.
    let me = world.locals[0].address.clone();
    let blank = CreateDraft {
        owners: vec![me],
        threshold: 1,
        ..CreateDraft::default()
    };
    run(&mut sut, &mut world, Event::KeepDraft { draft: blank });
    assert!(!world.store.contains_key(CREATE_DRAFT_KEY));
    // Kept again, then created: done with.
    let d = draft(&world);
    run(&mut sut, &mut world, Event::KeepDraft { draft: d });
    assert!(world.store.contains_key(CREATE_DRAFT_KEY));
    let c = choice(&world);
    run(&mut sut, &mut world, Event::Create { choice: c });
    assert_eq!(sut.view().create.phase, Phase::Done);
    assert!(sut.view().create.kept.is_none());
    assert!(!world.store.contains_key(CREATE_DRAFT_KEY));
}

/// r7-9: the create sheet offers the space in front; this device’s networks
/// of the other space are said, not hidden.
#[test]
fn the_other_spaces_networks_are_said() {
    let mut world = World::default();
    let mut sut = started(&mut world, vec![1, 100, 5_042_002, 11_155_111]);
    assert!(
        sut.view().create.other_space.is_empty(),
        "no space said yet"
    );
    run(
        &mut sut,
        &mut world,
        Event::Space {
            chain_ids: vec![1, 100],
        },
    );
    assert_eq!(sut.view().create.other_space, vec![5_042_002, 11_155_111]);
}

/// r7-8: what the person picked for each member lands in the stored row —
/// a pick alone, before any chain answered.
#[test]
fn a_created_multisig_keeps_the_kinds_picked() {
    const EVE: &str = "0x00000000000000000000000000000000000000ee";
    let mut world = World::default();
    world.locals.push(fixture(0));
    let mut sut = started(&mut world, vec![100]);
    let me = world.locals[0].address.clone();
    run(&mut sut, &mut world, Event::NewDraft);
    for (who, kind) in [(B, MemberKind::Vela), (EVE, MemberKind::Wallet)] {
        run(
            &mut sut,
            &mut world,
            Event::PickKind {
                address: who.to_owned(),
                kind: Some(kind),
            },
        );
    }
    let c = CreateChoice {
        owners: vec![me.clone(), B.to_owned(), EVE.to_owned()],
        threshold: 2,
        name: "Quiet Harbor".to_owned(),
        chain_id: 100,
        creator: me.clone(),
        members: BTreeMap::from([(
            B.to_owned(),
            MemberInfo {
                name: Some("Bob".to_owned()),
                kind: None,
                typed: None,
            },
        )]),
    };
    run(&mut sut, &mut world, Event::Create { choice: c });
    let members = &world.stored_accounts()[0]["members"];
    assert_eq!(members[me.to_ascii_lowercase()]["kind"], "passkey");
    assert_eq!(members[B]["kind"], "passkey");
    assert_eq!(members[B]["name"], "Bob");
    assert_eq!(members[EVE]["kind"], "wallet");
    let known = &sut.view().known_members;
    assert_eq!(known[EVE].kind.as_deref(), Some("wallet"));
    assert_eq!(known[B].name.as_deref(), Some("Bob"));
}

/// r4D-8: its invite handed on from here is kept (through a reload), said on
/// its row, and forgotten with it.
#[test]
fn an_invite_handed_on_is_kept_with_the_multisig() {
    let g = genesis(&[A], 1, "0x62");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    assert!(!sut.view().accounts[0].invite_shared);
    let shared = || Event::InviteShared {
        address: g.address.to_ascii_uppercase().replace("0X", "0x"),
    };
    run(&mut sut, &mut world, shared());
    assert!(sut.view().accounts[0].invite_shared);
    let sut2 = started(&mut world, vec![100]);
    assert!(
        sut2.view().accounts[0].invite_shared,
        "kept through a reload"
    );
    run(
        &mut sut,
        &mut world,
        Event::InviteShared {
            address: B.to_owned(),
        },
    );
    assert_eq!(
        world.store[INVITE_SHARED_KEY],
        json!([g.address.to_ascii_lowercase()]).to_string(),
        "only a multisig here"
    );
    run(
        &mut sut,
        &mut world,
        Event::Remove {
            address: g.address.clone(),
        },
    );
    assert_eq!(world.store[INVITE_SHARED_KEY], "[]");
}

// -- core-ms-2: what the shells decided -------------------------------------------------------

/// The threshold step starts sensible — all of two, else all but one — and
/// keeps the person's pick for as long as the members are the ones it was
/// picked for; a kept draft's threshold is the pick again.
#[test]
fn the_threshold_starts_sensible_and_keeps_the_persons_pick() {
    use vela_core::app::multisig_accounts::ThresholdNote;
    const C: &str = "0x00000000000000000000000000000000000000cc";
    const D: &str = "0x00000000000000000000000000000000000000dd";
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let mut sut = started(&mut world, vec![100]);
    run(&mut sut, &mut world, Event::NewDraft);
    assert!(sut.view().create.threshold.is_none(), "no members yet");
    let draft_of = |owners: &[&str], threshold: u64| Event::Draft {
        owners: owners.iter().map(|o| (*o).to_owned()).collect(),
        threshold,
        name: "Quiet Harbor".to_owned(),
    };
    run(&mut sut, &mut world, draft_of(&[&me], 1));
    let t = sut.view().create.threshold.clone().unwrap();
    assert_eq!((t.value, t.members, t.note), (1, 1, ThresholdNote::Solo));
    assert!(!t.risky && !t.can_lower && !t.can_raise);
    run(&mut sut, &mut world, draft_of(&[&me, B], 1));
    let t = sut.view().create.threshold.clone().unwrap();
    assert_eq!((t.value, t.note), (2, ThresholdNote::All), "all of two");
    assert!(t.risky);
    run(&mut sut, &mut world, draft_of(&[&me, B, C], 1));
    let t = sut.view().create.threshold.clone().unwrap();
    assert_eq!(
        (t.value, t.note),
        (2, ThresholdNote::Spare { count: 1 }),
        "all but one"
    );
    // The person's pick, clamped to the members.
    run(&mut sut, &mut world, Event::StepThreshold { up: true });
    run(&mut sut, &mut world, Event::StepThreshold { up: true });
    let t = sut.view().create.threshold.clone().unwrap();
    assert_eq!(
        (t.value, t.note, t.can_raise),
        (3, ThresholdNote::All, false)
    );
    // The draft's address is the pick's.
    let address = sut.view().create.draft_address.clone();
    run(&mut sut, &mut world, draft_of(&[&me, B, C], 3));
    assert_eq!(sut.view().create.draft_address, address);
    // Other members: the sensible start again; the same ones back: the pick.
    run(&mut sut, &mut world, draft_of(&[&me, B, C, D], 3));
    assert_eq!(sut.view().create.threshold.clone().unwrap().value, 3);
    for _ in 0..4 {
        run(&mut sut, &mut world, Event::StepThreshold { up: false });
    }
    let t = sut.view().create.threshold.clone().unwrap();
    assert_eq!(
        (t.value, t.note, t.can_lower),
        (1, ThresholdNote::One, false)
    );
    assert!(t.risky);
    run(&mut sut, &mut world, draft_of(&[&me, B, C], 1));
    assert_eq!(sut.view().create.threshold.clone().unwrap().value, 2);
    run(&mut sut, &mut world, draft_of(&[&me, B, C, D], 1));
    assert_eq!(sut.view().create.threshold.clone().unwrap().value, 1);
    // A kept draft (1 of its 2): its threshold again on a new sheet.
    let mut kept = draft(&world);
    kept.threshold = 1;
    run(&mut sut, &mut world, Event::KeepDraft { draft: kept });
    run(&mut sut, &mut world, Event::NewDraft);
    run(&mut sut, &mut world, draft_of(&[&me, B], 1));
    assert_eq!(sut.view().create.threshold.clone().unwrap().value, 1);
    // Started over: the sensible start.
    run(&mut sut, &mut world, Event::DiscardDraft);
    run(&mut sut, &mut world, Event::NewDraft);
    run(&mut sut, &mut world, draft_of(&[&me, B], 1));
    assert_eq!(sut.view().create.threshold.clone().unwrap().value, 2);
}

/// r5reg-19: where this device's transactions were, newest first, read from
/// the wallet's history — and the networks the multisigs here were first
/// used on: the hints the network order is asked with.
#[test]
fn the_network_hints_are_the_machines() {
    use vela_core::app::multisig_accounts::TRANSACTIONS_KEY;
    let mut world = World::default();
    world.locals.push(fixture(0));
    world.store.insert(
        TRANSACTIONS_KEY.to_owned(),
        json!([
            { "chainId": 100, "timestamp": 1 },
            { "chainId": 8453, "timestamp": 9 },
            { "chainId": 100, "timestamp": 5 }
        ])
        .to_string(),
    );
    let g = genesis(&[A], 1, "0x71");
    let h = genesis(&[A], 1, "0x72");
    let mut older = stored_row(&g, &[42161, 100], true);
    older["addedMs"] = json!(1.0);
    let mut newer = stored_row(&h, &[10], true);
    newer["addedMs"] = json!(2.0);
    world
        .store
        .insert(keys::ACCOUNTS.to_owned(), json!([older, newer]).to_string());
    let mut sut = started(&mut world, vec![100, 8453, 10, 42161]);
    assert_eq!(sut.view().recent_networks, vec![8453, 100]);
    assert_eq!(sut.view().used_networks, vec![10, 42161]);
    // A transaction since: read again when a create sheet opens.
    world.store.insert(
        TRANSACTIONS_KEY.to_owned(),
        json!([{ "chainId": 10, "timestamp": 20 }]).to_string(),
    );
    run(&mut sut, &mut world, Event::NewDraft);
    assert_eq!(sut.view().recent_networks, vec![10]);
}

/// r9-12 (r8-25, r4D-5): one name per member, the core's rule, drawn from
/// the view — what a multisig here keeps, else the passkey index's name,
/// else this device's own name for its account, else the short address —
/// and the invite carries every name known, with each kind.
#[test]
fn one_name_per_member_and_the_invite_carries_every_known_one() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let own_name = world.locals[0].name.clone();
    let g = genesis(&[&me, A, B], 2, "0x63");
    let mut row = stored_row(&g, &[100], true);
    row["members"] = json!({ B: { "name": "Bo", "kind": "wallet" } });
    world
        .store
        .insert(keys::ACCOUNTS.to_owned(), json!([row]).to_string());
    let mut sut = started(&mut world, vec![100]);
    let view = sut.view();
    let labels = &view.member_labels;
    assert_eq!(
        labels[&me.to_ascii_lowercase()],
        own_name,
        "this device's own name, last"
    );
    assert_eq!(labels[B], "Bo", "kept");
    assert_eq!(labels[A], "0x000000…0000AA", "nothing names it");
    // r11-6: this device's own name for its account is its own: an invite
    // (what other devices keep) never carries it — only a published or kept one.
    let mine = view.accounts[0]
        .invite_members
        .iter()
        .find(|m| {
            m.address
                .as_deref()
                .is_some_and(|x| x.eq_ignore_ascii_case(&me))
        })
        .cloned()
        .unwrap();
    assert_eq!(mine.name, None, "a local-only name stays on this device");
    let mut wanted = view.index_wanted.clone();
    wanted.sort();
    let mut want = vec![A.to_owned(), B.to_owned(), me.to_ascii_lowercase()];
    want.sort();
    assert_eq!(
        wanted, want,
        "this device's own too: its registered name is shared"
    );

    // The index answers: A registered "Interleave"; this device's account
    // registered its own name too, which every device shows before its nickname.
    run(
        &mut sut,
        &mut world,
        Event::IndexNames {
            answers: BTreeMap::from([
                (
                    A.to_owned(),
                    IndexAnswer {
                        vela: true,
                        name: Some("Interleave".into()),
                    },
                ),
                (
                    me.clone(),
                    IndexAnswer {
                        vela: true,
                        name: Some("Dana".into()),
                    },
                ),
                (
                    B.to_owned(),
                    IndexAnswer {
                        vela: false,
                        name: None,
                    },
                ),
            ]),
        },
    );
    let view = sut.view();
    assert_eq!(view.member_labels[A], "Interleave");
    assert_eq!(view.member_labels[&me.to_ascii_lowercase()], "Dana");
    assert_eq!(view.member_labels[B], "Bo", "kept before the index");
    assert!(view.index_wanted.is_empty());

    let invite = &view.accounts[0].invite_members;
    let by = |a: &str| {
        invite
            .iter()
            .find(|m| {
                m.address
                    .as_deref()
                    .is_some_and(|x| x.eq_ignore_ascii_case(a))
            })
            .cloned()
            .unwrap()
    };
    assert_eq!(invite.len(), 3);
    assert_eq!(by(&me).name.as_deref(), Some("Dana"));
    assert_eq!(by(A).name.as_deref(), Some("Interleave"));
    assert_eq!(by(B).name.as_deref(), Some("Bo"));
    let kind = |a: &str| serde_json::to_value(by(a).kind).unwrap();
    assert_eq!(kind(&me), json!("passkey"));
    assert_eq!(kind(A), json!("passkey"));
    assert_eq!(kind(B), json!("wallet"));
}

/// r9-14: the rule a chain read last said is kept per multisig and served on
/// its row, marked last known once it was not read this session — never the
/// founding "Needs 2 of 3" when the chain said 2 of 4.
#[test]
fn the_rule_the_chain_said_is_kept_on_the_row() {
    use vela_core::app::multisig_members::{MemberRule, RuleChip};
    let g = genesis(&[A], 1, "0x18");
    let mut world = World::default();
    world.store.insert(
        keys::ACCOUNTS.to_owned(),
        json!([stored_row(&g, &[100], true)]).to_string(),
    );
    let mut sut = started(&mut world, vec![100]);
    let row = sut.view().accounts[0].rule.clone();
    assert_eq!(
        row.chip,
        RuleChip::Needs {
            rule: MemberRule::of(1, 1)
        },
        "the founding rule before any read"
    );
    assert!(!row.from_chain);
    assert!(row.last_known);
    let now = RuleChip::Needs {
        rule: MemberRule::of(2, 4),
    };
    run(
        &mut sut,
        &mut world,
        Event::Apply {
            address: g.address.clone(),
            update: AccountUpdate::RuleNow { chip: now.clone() },
        },
    );
    let row = sut.view().accounts[0].rule.clone();
    assert_eq!(row.chip, now);
    assert!(row.from_chain);
    assert!(!row.last_known, "read this session");
    let again = started(&mut world, vec![100]);
    let row = again.view().accounts[0].rule.clone();
    assert_eq!(row.chip, now, "kept across a reload");
    assert!(row.from_chain);
    assert!(row.last_known, "not read since");
}
