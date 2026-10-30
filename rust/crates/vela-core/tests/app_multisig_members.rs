//! The members machine (`multisig_members`), driven against a simulated
//! shell — the web's `member-changes`, `members`, `member-entry`,
//! `member-reading` and `rule-words` suites, and the logic of
//! `MemberChanges.svelte.test.ts` and `MultisigSettings.svelte.test.ts`'s
//! "original and current members", rule for rule. Pure vectors:
//! `tests/vectors/multisig/members.json`.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use std::collections::BTreeMap;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_members::{
    change_key, changes_members, current_rules, deployed_members, kept_member, looks_like_name,
    member_change_groups, member_problem, member_state_of, members_anywhere, name_service,
    plan_member_change, progress_line, rule_chip, Compose, EntryNote, Event, MemberContact,
    MemberKind, MemberPlan, MemberProblem, MemberRule, MultisigMembers, NetworkMembers,
    NetworkState, PlanNetwork, Recovery, RuleChip, WaitingProposal, WaitingStatus,
};
use vela_core::app::multisig_proposals::ProposalKind;
use vela_core::app::multisig_shell::{
    AccountUpdate, ChainMembers, MemberInfo, MultisigAccount, MultisigOperation as Op,
    MultisigShellResult as Res, Notice,
};
use vela_core::multisig::{self, Action, CallArg, MemberChange, MemberRefusal, MemberVerdict};
use world::{fixture, genesis, settle_reads_first, stored_row, DeployedSafe, World};

type Sut = DomainDriver<MultisigMembers>;

const A: &str = "0x1111111111111111111111111111111111111111";
const B: &str = "0x2222222222222222222222222222222222222222";
const C: &str = "0x3333333333333333333333333333333333333333";
const D: &str = "0x4444444444444444444444444444444444444444";
const ZERO: &str = "0x0000000000000000000000000000000000000000";
const ARC: u64 = 5_042_002;
const BASE: u64 = 8453;
const GNOSIS: u64 = 100;
const OPTIMISM: u64 = 10;

/// The member change a network's proposal carries (always a member change).
fn change_of(n: &PlanNetwork) -> Option<MemberChange> {
    match &n.proposal {
        None => None,
        Some(ProposalKind::MemberChange { change }) => Some(change.clone()),
        Some(other) => panic!("not a member change: {other:?}"),
    }
}

fn account_of(g: &multisig::MultisigGenesis) -> MultisigAccount {
    serde_json::from_value(stored_row(g, &[], true)).unwrap()
}

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle_reads_first(sut, world, 5_000);
    assert!(answered < 5_000, "the machine never settled");
}

fn show_event(world: &World, account: &MultisigAccount, chain_ids: &[u64]) -> Event {
    Event::Show {
        account: account.clone(),
        chain_ids: chain_ids.to_vec(),
        local: world.locals.clone(),
        known: Vec::new(),
    }
}

fn shown(world: &mut World, account: &MultisigAccount, chain_ids: &[u64]) -> Sut {
    let mut sut = Sut::new();
    let event = show_event(world, account, chain_ids);
    run(&mut sut, world, event);
    sut
}

/// Arc and Base deployed with the same 2 of 3 (A, B, C); Gnosis not deployed.
fn two_networks() -> (World, MultisigAccount) {
    let g = genesis(&[A, B, C], 2, "0x01");
    let mut world = World::default();
    world.deploy(ARC, &g.address, DeployedSafe::clean(&[A, B, C], 2));
    world.deploy(BASE, &g.address, DeployedSafe::clean(&[A, B, C], 2));
    (world, account_of(&g))
}

fn plan_of(sut: &Sut) -> MemberPlan {
    sut.view().plan.expect("a plan")
}

fn on(plan: &MemberPlan, chain: u64) -> PlanNetwork {
    plan.networks
        .iter()
        .find(|n| n.chain_id == chain)
        .cloned()
        .unwrap_or_else(|| panic!("no plan for {chain}: {plan:?}"))
}

/// "2 of 3 → 2 of 2": the rule before and after, where the core built it.
fn rules_of(n: &PlanNetwork) -> ((u64, usize), (u64, usize)) {
    match &n.verdict {
        MemberVerdict::Ready { before, after, .. } => (
            (before.threshold, before.owners.len()),
            (after.threshold, after.owners.len()),
        ),
        MemberVerdict::Refused { refusal } => panic!("refused on {}: {refusal:?}", n.chain_id),
    }
}

fn refusal_of(n: &PlanNetwork) -> MemberRefusal {
    match &n.verdict {
        MemberVerdict::Refused { refusal } => refusal.clone(),
        MemberVerdict::Ready { .. } => panic!("built on {}", n.chain_id),
    }
}

fn data_of(n: &PlanNetwork) -> String {
    match &n.verdict {
        MemberVerdict::Ready { tx, .. } => tx.data.to_ascii_lowercase(),
        MemberVerdict::Refused { refusal } => panic!("refused: {refusal:?}"),
    }
}

fn waiting(chain_id: u64, action: Action, approved: u64, status: WaitingStatus) -> WaitingProposal {
    WaitingProposal {
        chain_id,
        safe_tx_hash: format!("0x{chain_id}"),
        nonce: "3".to_owned(),
        action: Some(action),
        approved,
        threshold: 2,
        status,
    }
}

fn add_d() -> Action {
    Action::AddOwner {
        owner: D.to_owned(),
        threshold: "2".to_owned(),
    }
}

fn word(hex: &str) -> String {
    format!("{:0>64}", hex.trim_start_matches("0x").to_ascii_lowercase())
}

fn chain_name(id: u64) -> String {
    match id {
        ARC => "Arc".to_owned(),
        BASE => "Base".to_owned(),
        GNOSIS => "Gnosis".to_owned(),
        other => other.to_string(),
    }
}

// =====================================================================================
// member-changes.test.ts — one action, one proposal per deployed network
// =====================================================================================

/// Arc 2 of 3 (A, B, C); Base already changed: 2 of 3 (D, A, B); Gnosis not deployed.
fn changed_networks() -> (World, MultisigAccount) {
    let g = genesis(&[A, B, C], 2, "0x02");
    let mut world = World::default();
    world.deploy(ARC, &g.address, DeployedSafe::clean(&[A, B, C], 2));
    world.deploy(BASE, &g.address, DeployedSafe::clean(&[D, A, B], 2));
    (world, account_of(&g))
}

#[test]
fn each_networks_change_is_built_against_that_networks_own_list() {
    let (mut world, account) = changed_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    let change = MemberChange::Remove {
        owner: B.to_owned(),
        threshold: None,
    };
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            change: Some(change.clone()),
        },
    );
    let plan = plan_of(&sut);
    assert_eq!(plan.ready, vec![ARC, BASE]);
    assert_eq!(plan.undeployed, vec![GNOSIS]);
    let (arc, base) = (on(&plan, ARC), on(&plan, BASE));
    // removeOwner(prevOwner, owner, threshold): B follows A on Arc …
    assert_eq!(
        data_of(&arc),
        format!("0xf8dc5dd9{}{}{}", word(A), word(B), word("2"))
    );
    // … and on Base too, but A follows D there: the previous owner is each list's.
    assert!(data_of(&base).starts_with(&format!("0xf8dc5dd9{}", word(A))));
    assert_eq!(rules_of(&arc), ((2, 3), (2, 2)));
    // The review every member reads it with says what it is.
    let MemberVerdict::Ready { tx, .. } = &arc.verdict else {
        unreachable!()
    };
    let read = multisig::review(ARC, &account.address, tx).unwrap();
    assert!(matches!(read.action, Some(Action::RemoveOwner { .. })));
    // What the proposals machine is handed, per network: the change itself.
    assert_eq!(change_of(&arc), Some(change.clone()));
    assert_eq!(change_of(&base), Some(change));
    assert!(sut.view().can_propose);
}

#[test]
fn a_network_refuses_in_its_own_terms_and_proposes_only_where_it_can() {
    let (mut world, account) = changed_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    let draft = |sut: &mut Sut, world: &mut World, change: MemberChange| {
        run(
            sut,
            world,
            Event::Draft {
                change: Some(change),
            },
        );
        plan_of(sut)
    };
    // C is a member on Arc, not on Base (replaced there already).
    let plan = draft(
        &mut sut,
        &mut world,
        MemberChange::Replace {
            old_owner: C.to_owned(),
            new_owner: D.to_owned(),
        },
    );
    assert_eq!(plan.ready, vec![ARC]);
    assert!(matches!(
        refusal_of(&on(&plan, BASE)),
        MemberRefusal::NotMember { .. }
    ));
    assert_eq!(
        change_of(&on(&plan, BASE)),
        None,
        "nothing to propose there"
    );
    let plan = draft(
        &mut sut,
        &mut world,
        MemberChange::Add {
            owner: account.address.clone(),
            threshold: None,
        },
    );
    assert_eq!(
        plan.networks.iter().map(refusal_of).collect::<Vec<_>>(),
        vec![MemberRefusal::TheMultisig, MemberRefusal::TheMultisig]
    );
    let plan = draft(
        &mut sut,
        &mut world,
        MemberChange::Threshold { threshold: 4 },
    );
    assert_eq!(
        refusal_of(&on(&plan, ARC)),
        MemberRefusal::ThresholdTooHigh {
            threshold: 4,
            owners: 3
        }
    );
    let plan = draft(
        &mut sut,
        &mut world,
        MemberChange::Add {
            owner: ZERO.to_owned(),
            threshold: None,
        },
    );
    assert!(plan.ready.is_empty());
    assert!(!sut.view().can_propose);
}

#[test]
fn a_second_member_change_is_refused_while_one_waits_on_that_network() {
    let (mut world, account) = changed_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE]);
    run(
        &mut sut,
        &mut world,
        Event::Waiting {
            proposals: vec![waiting(
                ARC,
                Action::ChangeThreshold {
                    threshold: "3".to_owned(),
                },
                0,
                WaitingStatus::Pending,
            )],
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            change: Some(MemberChange::Add {
                owner: C.to_owned(),
                threshold: None,
            }),
        },
    );
    let plan = plan_of(&sut);
    assert_eq!(refusal_of(&on(&plan, ARC)), MemberRefusal::ChangeWaiting);
    // Base has nothing waiting, and C is not a member there.
    assert_eq!(plan.ready, vec![BASE]);
    // A transfer waiting is no member change; one executed or voided waits no more.
    let arc = ChainMembers {
        deployed: true,
        owners: vec![A.to_owned(), B.to_owned(), C.to_owned()],
        threshold: 2,
        nonce: 3,
    };
    let transfer = Action::Transfer {
        token: None,
        to: D.to_owned(),
        amount: "1".to_owned(),
    };
    assert!(
        !member_state_of(&arc, &[waiting(ARC, transfer, 0, WaitingStatus::Pending)]).change_waiting
    );
    assert!(
        !member_state_of(&arc, &[waiting(ARC, add_d(), 0, WaitingStatus::Done)]).change_waiting
    );
    assert!(
        member_state_of(&arc, &[waiting(ARC, add_d(), 0, WaitingStatus::Queued)]).change_waiting
    );
}

#[test]
fn nothing_is_offered_where_it_is_not_deployed() {
    let g = genesis(&[A, B, C], 2, "0x03");
    let account = account_of(&g);
    let mut world = World::default();
    let mut sut = shown(&mut world, &account, &[GNOSIS]);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            change: Some(MemberChange::Threshold { threshold: 3 }),
        },
    );
    let plan = plan_of(&sut);
    assert!(plan.networks.is_empty());
    assert!(plan.ready.is_empty());
    assert_eq!(plan.undeployed, vec![GNOSIS]);
    // MemberChanges: "offers nothing when it is deployed nowhere".
    let view = sut.view();
    assert!(view.none_deployed);
    assert!(view.rows.is_empty());
    assert!(!view.can_propose);
    assert_eq!(view.networks[0].state, NetworkState::Undeployed);
}

#[test]
fn every_deployed_networks_members_once_the_first_networks_order_first() {
    let (mut world, account) = changed_networks();
    let sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    let rows: Vec<String> = sut.view().rows.into_iter().map(|r| r.address).collect();
    assert_eq!(rows, vec![A, B, C, D]);
}

#[test]
fn the_changes_waiting_read_as_one_change_per_action() {
    let item = |chain, action, approved| waiting(chain, action, approved, WaitingStatus::Pending);
    let upper = Action::AddOwner {
        owner: D.to_ascii_uppercase().replace("0X", "0x"),
        threshold: "2".to_owned(),
    };
    let groups = member_change_groups(
        &[
            item(ARC, add_d(), 1),
            item(BASE, upper, 0),
            item(
                ARC,
                Action::Transfer {
                    token: None,
                    to: D.to_owned(),
                    amount: "5".to_owned(),
                },
                0,
            ),
            waiting(GNOSIS, add_d(), 2, WaitingStatus::Done),
        ],
        &[],
    );
    assert_eq!(groups.len(), 1);
    assert_eq!(progress_line(&groups[0], chain_name), "Arc 1/2 · Base 0/2");
    // The networks in the wallet's order, whichever was proposed first.
    let ordered = member_change_groups(
        &[item(BASE, add_d(), 0), item(ARC, add_d(), 1)],
        &[ARC, BASE],
    );
    assert_eq!(
        ordered[0]
            .parts
            .iter()
            .map(|p| p.chain_id)
            .collect::<Vec<_>>(),
        vec![ARC, BASE]
    );
    assert_eq!(
        change_key(Some(&Action::ChangeThreshold {
            threshold: "3".to_owned()
        }))
        .as_deref(),
        Some("threshold:3")
    );
    assert_eq!(
        change_key(Some(&Action::Transfer {
            token: None,
            to: D.to_owned(),
            amount: "1".to_owned()
        })),
        None
    );
    let batch: Action = serde_json::from_value(json!({
        "kind": "batch",
        "calls": [{"to": A, "value": "0", "data": "0x", "action": add_d()}]
    }))
    .unwrap();
    assert!(changes_members(Some(&batch)));
}

// =====================================================================================
// MemberChanges.svelte.test.ts — changing the members (Settings), the logic
// =====================================================================================

#[test]
fn removes_a_member_with_one_proposal_per_deployed_network_none_where_it_is_not() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    // The rows: every deployed network's members, each with Replace and Remove.
    assert_eq!(sut.view().rows.len(), 3);
    run(
        &mut sut,
        &mut world,
        Event::Compose {
            compose: Some(Compose::Remove {
                owner: A.to_owned(),
            }),
        },
    );
    // 2 of 3 minus one: the rule still fits (2 of 2) on each network.
    let plan = plan_of(&sut);
    assert_eq!(rules_of(&on(&plan, ARC)), ((2, 3), (2, 2)));
    assert_eq!(rules_of(&on(&plan, BASE)), ((2, 3), (2, 2)));
    assert_eq!(plan.undeployed, vec![GNOSIS]);
    // Propose: one request per network, the change as composed (no threshold picked).
    let requests: Vec<(u64, MemberChange)> = plan
        .networks
        .iter()
        .filter_map(|n| change_of(n).map(|c| (n.chain_id, c)))
        .collect();
    let remove_a = MemberChange::Remove {
        owner: A.to_owned(),
        threshold: None,
    };
    assert_eq!(requests, vec![(ARC, remove_a.clone()), (BASE, remove_a)]);
    // A removal names no new member: nothing is kept on this device.
    run(&mut sut, &mut world, Event::Proposing);
    assert!(world.notices.is_empty());
}

#[test]
fn removing_below_the_approvals_needed_is_refused_and_lowering_them_makes_it_possible() {
    let g = genesis(&[A, B, C], 3, "0x04");
    let account = account_of(&g);
    let mut world = World::default();
    world.deploy(ARC, &g.address, DeployedSafe::clean(&[A, B, C], 3));
    let mut sut = shown(&mut world, &account, &[ARC]);
    run(
        &mut sut,
        &mut world,
        Event::Compose {
            compose: Some(Compose::Remove {
                owner: B.to_owned(),
            }),
        },
    );
    assert_eq!(
        refusal_of(&on(&plan_of(&sut), ARC)),
        MemberRefusal::BelowThreshold {
            owners: 2,
            threshold: 3
        }
    );
    assert!(!sut.view().can_propose);
    // Untouched, the stepper reads the rule as it stands: 3 of 2.
    let stepper = sut.view().stepper.unwrap();
    assert_eq!((stepper.value, stepper.n), (3, 2));
    // The stepper under it: one fewer approval.
    run(&mut sut, &mut world, Event::Step { by: -1 });
    assert_eq!(rules_of(&on(&plan_of(&sut), ARC)), ((3, 3), (2, 2)));
    assert_eq!(
        sut.view().change,
        Some(MemberChange::Remove {
            owner: B.to_owned(),
            threshold: Some(2)
        })
    );
    assert_eq!(plan_of(&sut).ready, vec![ARC]);
}

#[test]
fn replaces_a_member_the_new_account_shown_in_full_and_its_name_kept_here() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    run(
        &mut sut,
        &mut world,
        Event::Compose {
            compose: Some(Compose::Replace {
                owner: C.to_owned(),
            }),
        },
    );
    assert!(sut.view().stepper.is_none(), "a replace keeps the rule");
    // The multisig itself is no member: refused for every network.
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: account.address.clone(),
        },
    );
    run(&mut sut, &mut world, Event::ReadEntry);
    assert_eq!(
        refusal_of(&on(&plan_of(&sut), ARC)),
        MemberRefusal::TheMultisig
    );
    run(&mut sut, &mut world, Event::Entry { text: D.to_owned() });
    assert!(sut.view().plan.is_none(), "a new text is no entry yet");
    run(&mut sut, &mut world, Event::ReadEntry);
    assert_eq!(sut.view().entry.entry.unwrap().address, D);
    run(
        &mut sut,
        &mut world,
        Event::EntryName {
            name: "Dana".to_owned(),
        },
    );
    // Round 5: the preview names who replaces whom, by the name just typed.
    let preview = sut.view().replace_preview.unwrap();
    assert_eq!(preview.old.address, C);
    assert_eq!(preview.new_address, D);
    assert_eq!(preview.new_name.as_deref(), Some("Dana"));
    let plan = plan_of(&sut);
    assert_eq!(plan.ready, vec![ARC, BASE]);
    assert_eq!(
        change_of(&on(&plan, BASE)),
        Some(MemberChange::Replace {
            old_owner: C.to_owned(),
            new_owner: D.to_owned()
        })
    );
    run(&mut sut, &mut world, Event::Proposing);
    assert_eq!(
        world.notices,
        vec![Notice::Account {
            address: account.address.clone(),
            update: AccountUpdate::NameMembers {
                members: BTreeMap::from([(
                    D.to_owned(),
                    MemberInfo {
                        name: Some("Dana".to_owned()),
                        kind: Some("wallet".to_owned()),
                        typed: None,
                    }
                )]),
                typed: true,
            },
        }]
    );
}

#[test]
fn the_approvals_stepper_changes_the_rule_and_a_network_that_refuses_says_so() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    assert_eq!(
        sut.view().recovery,
        Some(Recovery::Replaceable { m: 2, n: 3 })
    );
    run(&mut sut, &mut world, Event::Step { by: 1 });
    let view = sut.view();
    assert_eq!(view.compose, Some(Compose::Threshold));
    let s = view.stepper.unwrap();
    assert_eq!((s.value, s.n, s.can_more), (3, 3, false));
    assert_eq!(rules_of(&on(&plan_of(&sut), ARC)), ((2, 3), (3, 3)));
    // Past the members: still 3.
    run(&mut sut, &mut world, Event::Step { by: 1 });
    assert_eq!(
        sut.view().change,
        Some(MemberChange::Threshold { threshold: 3 })
    );
    // Base has a member change waiting: it refuses, Arc proposes.
    run(
        &mut sut,
        &mut world,
        Event::Waiting {
            proposals: vec![waiting(BASE, add_d(), 0, WaitingStatus::Pending)],
        },
    );
    let plan = plan_of(&sut);
    assert_eq!(refusal_of(&on(&plan, BASE)), MemberRefusal::ChangeWaiting);
    assert_eq!(plan.ready, vec![ARC]);
    // Every member must approve: a lost passkey cannot be replaced.
    let g = genesis(&[A, B], 2, "0x05");
    let both = account_of(&g);
    let mut world = World::default();
    world.deploy(ARC, &g.address, DeployedSafe::clean(&[A, B], 2));
    let sut = shown(&mut world, &both, &[ARC]);
    assert_eq!(sut.view().recovery, Some(Recovery::EveryMember));
}

#[test]
fn adds_a_member_picked_from_the_contacts_its_address_shown_its_name_and_kind_kept() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    run(
        &mut sut,
        &mut world,
        Event::Contacts {
            contacts: vec![
                MemberContact {
                    address: D.to_owned(),
                    name: Some("Dana".to_owned()),
                    resolved_name: None,
                    kind: Some("eoa".to_owned()),
                },
                MemberContact {
                    address: "0x5555555555555555555555555555555555555555".to_owned(),
                    name: Some("Eve".to_owned()),
                    resolved_name: None,
                    kind: Some("eoa".to_owned()),
                },
            ],
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Compose {
            compose: Some(Compose::Add),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "dan".to_owned(),
        },
    );
    // A name searched is no wrong address: no error while it matches a contact.
    run(&mut sut, &mut world, Event::ReadEntry);
    let view = sut.view();
    assert_eq!(view.entry.note, None);
    assert_eq!(view.entry.suggestions.len(), 1);
    assert_eq!(view.entry.suggestions[0].address, D);
    run(
        &mut sut,
        &mut world,
        Event::Pick {
            address: D.to_owned(),
        },
    );
    let view = sut.view();
    assert_eq!(view.entry.entry.as_ref().unwrap().address, D);
    assert_eq!(view.entry.name, "Dana");
    assert_eq!(view.entry.kind, MemberKind::Wallet);
    assert!(view.entry.suggestions.is_empty(), "picked: no more offers");
    assert_eq!(rules_of(&on(&plan_of(&sut), ARC)), ((2, 3), (2, 4)));
    assert_eq!(
        change_of(&on(&plan_of(&sut), ARC)),
        Some(MemberChange::Add {
            owner: D.to_owned(),
            threshold: None
        })
    );
    run(&mut sut, &mut world, Event::Proposing);
    let Notice::Account {
        update: AccountUpdate::NameMembers { members, typed },
        ..
    } = &world.notices[0]
    else {
        panic!("{:?}", world.notices)
    };
    assert!(*typed);
    assert_eq!(
        members[D],
        MemberInfo {
            name: Some("Dana".to_owned()),
            kind: Some("wallet".to_owned()),
            typed: None
        }
    );
}

#[test]
fn a_change_waiting_on_several_networks_shows_as_one_with_each_ones_progress() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC, BASE, GNOSIS]);
    run(
        &mut sut,
        &mut world,
        Event::Waiting {
            proposals: vec![
                waiting(BASE, add_d(), 0, WaitingStatus::Pending),
                waiting(ARC, add_d(), 1, WaitingStatus::Pending),
            ],
        },
    );
    let groups = sut.view().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!(progress_line(&groups[0], chain_name), "Arc 1/2 · Base 0/2");
    assert_eq!(groups[0].action, add_d());
    // Each network's part opens its own proposal.
    assert_eq!(groups[0].parts[1].safe_tx_hash, format!("0x{BASE}"));
}

// =====================================================================================
// MultisigSettings.svelte.test.ts — original and current members
// =====================================================================================

#[test]
fn each_deployed_network_is_shown_beside_the_original_members_marked_added_and_removed() {
    let mut world = World::default();
    world.locals.push(fixture(0));
    let me = world.locals[0].address.clone();
    let other = "0x00000000000000000000000000000000000000bb";
    let new = "0x00000000000000000000000000000000000000cc";
    let g = genesis(&[&me, other], 2, "0x06");
    let account = account_of(&g);
    let mut gnosis = DeployedSafe::clean(&[&me, new], 1);
    gnosis.nonce = 3;
    world.deploy(GNOSIS, &g.address, gnosis);
    world.deploy(OPTIMISM, &g.address, DeployedSafe::clean(&[&me, other], 2));
    let sut = shown(&mut world, &account, &[GNOSIS, BASE, OPTIMISM]);
    let view = sut.view();
    assert_eq!(view.original.rule.unwrap(), MemberRule::of(2, 2));
    assert_eq!(view.original.members.len(), 2);
    assert!(view.original.members[0].on_device, "this device's account");
    assert_eq!(view.original.members[0].kind.as_deref(), Some("passkey"));
    assert_eq!(
        view.original.members[0].name.as_deref(),
        Some(world.locals[0].name.as_str())
    );
    // Gnosis: OTHER swapped for NEW, one approval now.
    let NetworkState::Deployed {
        rule,
        members,
        removed,
        same,
        threshold_was,
        threshold_is,
        nonce,
    } = &view.networks[0].state
    else {
        panic!("{:?}", view.networks[0])
    };
    assert_eq!(*rule, MemberRule::of(1, 2));
    assert_eq!(members.iter().filter(|m| m.added).count(), 1);
    assert!(members
        .iter()
        .find(|m| m.added)
        .unwrap()
        .address
        .eq_ignore_ascii_case(new));
    assert_eq!(removed.len(), 1);
    assert!(removed[0].address.eq_ignore_ascii_case(other));
    assert!(!same);
    assert_eq!((*threshold_was, *threshold_is), (2, 1));
    assert_eq!(nonce, "3");
    // Base, not deployed: starts with the original members.
    assert_eq!(view.networks[1].state, NetworkState::Undeployed);
    // Optimism: the same as the original members.
    assert!(matches!(
        view.networks[2].state,
        NetworkState::Deployed { same: true, .. }
    ));
    assert!(!view.read_only);
    assert_eq!(view.on_device.len(), 1);
    // The rule differs by network (r4D-7).
    assert_eq!(
        view.rule_chip,
        Some(RuleChip::Differs {
            rules: vec![MemberRule::of(1, 2), MemberRule::of(2, 2)]
        })
    );
}

#[test]
fn the_original_members_alone_before_any_network_is_read() {
    let g = genesis(&[A, B], 2, "0x07");
    let account = account_of(&g);
    let world = World::default();
    let mut sut = Sut::new();
    sut.dispatch(show_event(&world, &account, &[GNOSIS, BASE]));
    let view = sut.view();
    assert!(view
        .networks
        .iter()
        .all(|n| n.state == NetworkState::Reading));
    assert_eq!(view.original.members.len(), 2);
    assert_eq!(view.rules, vec![MemberRule::of(2, 2)]);
    assert!(!view.none_deployed, "nothing is said before it is read");
    // None of its members is on this device: read-only.
    assert!(view.read_only);
}

// =====================================================================================
// What the chain does not say is never "the same"
// =====================================================================================

#[test]
fn a_network_that_does_not_answer_is_unreadable_never_the_same() {
    let (mut world, account) = two_networks();
    world.silent.insert(BASE);
    let mut sut = shown(&mut world, &account, &[ARC, BASE]);
    let view = sut.view();
    assert!(matches!(
        view.networks[1].state,
        NetworkState::Unreadable { .. }
    ));
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            change: Some(MemberChange::Threshold { threshold: 3 }),
        },
    );
    let plan = plan_of(&sut);
    assert_eq!(plan.ready, vec![ARC]);
    assert_eq!(plan.unreadable, vec![BASE]);
    assert!(plan.networks.iter().all(|n| n.chain_id != BASE));
    // It comes back: read again, it is said.
    world.silent.clear();
    run(&mut sut, &mut world, Event::Refresh);
    assert!(matches!(
        sut.view().networks[1].state,
        NetworkState::Deployed { same: true, .. }
    ));
}

#[test]
fn a_read_that_never_answers_is_bounded() {
    let (mut world, account) = two_networks();
    let mut sut = Sut::new();
    sut.dispatch(show_event(&world, &account, &[ARC, BASE]));
    // Every read is answered but Base's: it hangs.
    for _ in 0..200 {
        let next = sut.outstanding().into_iter().find(|op| match op {
            Op::Wait { .. } => false,
            Op::Rpc { chain_id, .. } => *chain_id != BASE,
            _ => true,
        });
        let Some(op) = next else { break };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    assert!(matches!(
        sut.view().networks[0].state,
        NetworkState::Deployed { .. }
    ));
    assert_eq!(sut.view().networks[1].state, NetworkState::Reading);
    // Its bound runs out (Arc's wait, then Base's): unreadable, not "the same".
    while let Some(op) = sut
        .outstanding()
        .into_iter()
        .find(|op| matches!(op, Op::Wait { .. }))
    {
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    let NetworkState::Unreadable { detail } = sut.view().networks[1].state.clone() else {
        panic!("{:?}", sut.view().networks[1])
    };
    assert!(detail.contains("8000"), "{detail}");
}

#[test]
fn members_changed_on_chain_are_read_again() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC]);
    {
        let safe = world.safe_mut(ARC, &account.address);
        safe.owners = vec![D.to_owned(), A.to_owned(), B.to_owned(), C.to_owned()];
        safe.nonce = 1;
    }
    run(&mut sut, &mut world, Event::Refresh);
    let NetworkState::Deployed { members, .. } = &sut.view().networks[0].state else {
        panic!()
    };
    assert_eq!(members.len(), 4);
    assert!(members[0].added);
}

#[test]
fn another_multisig_shown_starts_afresh() {
    let (mut world, account) = two_networks();
    let mut sut = shown(&mut world, &account, &[ARC]);
    run(
        &mut sut,
        &mut world,
        Event::Compose {
            compose: Some(Compose::Threshold),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Waiting {
            proposals: vec![waiting(ARC, add_d(), 0, WaitingStatus::Pending)],
        },
    );
    let g = genesis(&[A, B], 1, "0x08");
    let other = account_of(&g);
    let event = show_event(&world, &other, &[GNOSIS]);
    run(&mut sut, &mut world, event);
    let view = sut.view();
    assert_eq!(view.address.as_deref(), Some(other.address.as_str()));
    assert_eq!(view.compose, None);
    assert!(view.groups.is_empty());
    assert_eq!(view.networks.len(), 1);
}

// =====================================================================================
// member-entry.test.ts / members.test.ts — who joins
// =====================================================================================

const RESOLVER: &str = "0x231b0ee14048e9dccd1d247744d114a4eb5e8e63";
const THEIRS: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

/// ENS answers, as the web test's script: the registry names `RESOLVER`,
/// which answers `address` (or the zero address).
fn ens(world: &mut World, address: Option<&'static str>) {
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        if method != "eth_call" {
            return None;
        }
        let to = params[0]["to"].as_str()?.to_ascii_lowercase();
        let data = params[0]["data"].as_str()?.to_ascii_lowercase();
        let data = data.trim_start_matches("0x");
        let ok = |a: &str| Res::RpcResult {
            result: json!(format!("0x{}", word(a))),
        };
        if data.starts_with("0178b8bf") {
            return Some(ok(RESOLVER));
        }
        if to == RESOLVER && data.starts_with("3b3b57de") {
            return Some(ok(address.unwrap_or(ZERO)));
        }
        None
    }));
}

fn entering(world: &mut World) -> Sut {
    let (w, account) = two_networks();
    *world = w;
    let mut sut = shown(world, &account, &[ARC]);
    run(
        &mut sut,
        world,
        Event::Compose {
            compose: Some(Compose::Add),
        },
    );
    sut
}

#[test]
fn a_name_resolves_to_the_address_its_resolver_gives_on_ethereum_or_base() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    ens(&mut world, Some(THEIRS));
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "Vitalik.eth".to_owned(),
        },
    );
    world.rpc_log.clear();
    sut.dispatch(Event::ReadEntry);
    assert!(sut.view().entry.resolving);
    settle_reads_first(&mut sut, &mut world, 100);
    let view = sut.view();
    assert!(!view.entry.resolving);
    let entry = view.entry.entry.unwrap();
    assert_eq!(entry.address, THEIRS, "shown in full beside the name");
    assert_eq!(entry.name.as_deref(), Some("vitalik.eth"));
    assert_eq!(view.entry.name, "vitalik.eth", "the name it was found by");
    assert_eq!(world.rpc_log[0].0, 1, "ENS on Ethereum");
    assert!(view.plan.is_some());
    world.rpc_log.clear();
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "bob.base.eth".to_owned(),
        },
    );
    run(&mut sut, &mut world, Event::ReadEntry);
    assert_eq!(world.rpc_log[0].0, BASE, "a Basename on Base");
}

#[test]
fn a_name_says_not_found_or_unavailable_never_an_address() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    ens(&mut world, None);
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "nobody.eth".to_owned(),
        },
    );
    run(&mut sut, &mut world, Event::ReadEntry);
    let view = sut.view();
    assert_eq!(
        view.entry.note,
        Some(EntryNote::NameNotFound {
            name: "nobody.eth".to_owned()
        })
    );
    assert!(view.entry.entry.is_none());
    assert!(view.plan.is_none());
    world.rpc_hooks.clear();
    world.silent.insert(1);
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "vitalik.eth".to_owned(),
        },
    );
    run(&mut sut, &mut world, Event::ReadEntry);
    assert_eq!(
        sut.view().entry.note,
        Some(EntryNote::NameUnavailable {
            name: "vitalik.eth".to_owned()
        })
    );
}

#[test]
fn a_late_answer_for_older_text_is_dropped() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    ens(&mut world, Some(THEIRS));
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "vitalik.eth".to_owned(),
        },
    );
    sut.dispatch(Event::ReadEntry);
    // The person types on before the name answers.
    sut.dispatch(Event::Entry {
        text: "0x12".to_owned(),
    });
    settle_reads_first(&mut sut, &mut world, 100);
    let view = sut.view();
    assert!(view.entry.entry.is_none());
    assert!(!view.entry.resolving);
    assert_eq!(view.entry.typed, "0x12");
}

#[test]
fn an_address_typed_is_refused_with_its_reason_where_it_is_added() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    let read = |sut: &mut Sut, world: &mut World, text: &str| {
        run(
            sut,
            world,
            Event::Entry {
                text: text.to_owned(),
            },
        );
        run(sut, world, Event::ReadEntry);
        sut.view().entry.note
    };
    assert_eq!(read(&mut sut, &mut world, ZERO), Some(EntryNote::NoOne));
    assert_eq!(
        read(
            &mut sut,
            &mut world,
            "0x0000000000000000000000000000000000000001"
        ),
        Some(EntryNote::NoOne)
    );
    assert_eq!(
        read(&mut sut, &mut world, "0x12"),
        Some(EntryNote::InvalidAddress)
    );
    assert_eq!(
        read(
            &mut sut,
            &mut world,
            "0xD9c79Cc5C3DE9858E8767BB702DB5FfE51a20432"
        ),
        Some(EntryNote::InvalidChecksum)
    );
    assert_eq!(
        read(
            &mut sut,
            &mut world,
            " 0x2222222222222222222222222222222222222222 "
        ),
        None
    );
    // Already a member: the core says so, network by network.
    assert_eq!(read(&mut sut, &mut world, A), None);
    assert!(matches!(
        refusal_of(&on(&plan_of(&sut), ARC)),
        MemberRefusal::AlreadyMember { .. }
    ));
}

#[test]
fn this_devices_accounts_are_offered_and_join_as_vela_accounts() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    let mine = fixture(1);
    run(
        &mut sut,
        &mut world,
        Event::LocalAccounts {
            local: vec![mine.clone()],
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: mine.address[..6].to_owned(),
        },
    );
    let offered = sut.view().entry.suggestions;
    assert_eq!(offered.len(), 1);
    assert_eq!(offered[0].kind, MemberKind::Vela);
    // Typed in full, it reads as this device's account: a Vela account.
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: mine.address.clone(),
        },
    );
    run(&mut sut, &mut world, Event::ReadEntry);
    assert_eq!(sut.view().entry.kind, MemberKind::Vela);
    // The person's pick wins.
    run(
        &mut sut,
        &mut world,
        Event::EntryKind {
            kind: Some(MemberKind::Wallet),
        },
    );
    assert_eq!(sut.view().entry.kind, MemberKind::Wallet);
}

// =====================================================================================
// names.svelte.ts (keptMember), rule-words.test.ts
// =====================================================================================

#[test]
fn members_are_labelled_from_this_device_never_as_authority() {
    let g = genesis(&[A, B], 2, "0x09");
    let mut account = account_of(&g);
    account.members = Some(BTreeMap::from([(
        A.to_owned(),
        MemberInfo {
            name: Some("Ana".to_owned()),
            kind: Some("wallet".to_owned()),
            typed: None,
        },
    )]));
    let mut another = account_of(&genesis(&[B], 1, "0x0a"));
    another.members = Some(BTreeMap::from([(
        B.to_owned(),
        MemberInfo {
            name: Some("Bo".to_owned()),
            kind: None,
            typed: None,
        },
    )]));
    // The first multisig that names it.
    assert_eq!(
        kept_member(&[account.clone(), another.clone()], B)
            .name
            .as_deref(),
        Some("Bo")
    );
    let mut world = World::default();
    let mut sut = Sut::new();
    sut.dispatch(Event::Show {
        account: account.clone(),
        chain_ids: vec![GNOSIS],
        local: Vec::new(),
        known: vec![another],
    });
    settle_reads_first(&mut sut, &mut world, 100);
    let rows = sut.view().original.members;
    assert_eq!(rows[0].name.as_deref(), Some("Ana"));
    assert_eq!(rows[0].kind.as_deref(), Some("wallet"));
    assert_eq!(rows[1].name.as_deref(), Some("Bo"));
}

#[test]
fn addresses_read_in_one_case_however_they_came() {
    let mut world = World::default();
    let mut sut = entering(&mut world);
    let checksummed = "0xd9c79Cc5C3DE9858E8767BB702DB5FfE51a20432";
    run(
        &mut sut,
        &mut world,
        Event::Contacts {
            contacts: vec![MemberContact {
                address: checksummed.to_ascii_lowercase(),
                name: Some("Lower".to_owned()),
                resolved_name: None,
                kind: None,
            }],
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::Entry {
            text: "low".to_owned(),
        },
    );
    assert_eq!(sut.view().entry.suggestions[0].address, checksummed);
}

#[test]
fn the_rule_is_a_requirement_and_one_member_reads_as_one() {
    let original = MemberRule::of(2, 3);
    assert!(!original.solo);
    assert!(MemberRule::of(1, 1).solo, "never \"1 of 1 members\"");
    assert_eq!(
        rule_chip(&current_rules(original, &[])),
        Some(RuleChip::Needs { rule: original })
    );
    let read = ChainMembers {
        deployed: true,
        owners: vec![A.to_owned(), B.to_owned()],
        threshold: 2,
        nonce: 0,
    };
    assert!(matches!(
        rule_chip(&current_rules(original, &[&read])),
        Some(RuleChip::Needs { .. })
    ));
}

fn catalog(locale: &str) -> Value {
    let path = format!(
        "{}/i18n/locales/{locale}/home.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str::<Value>(&raw).unwrap()["home"]["multisig"].clone()
}

fn locales() -> Vec<String> {
    let dir = format!("{}/i18n/locales", env!("CARGO_MANIFEST_DIR"));
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .unwrap()
        .filter_map(|e| {
            let e = e.ok()?;
            e.file_type()
                .ok()?
                .is_dir()
                .then(|| e.file_name().to_string_lossy().into_owned())
        })
        .collect();
    out.sort();
    out
}

fn fill(template: &str, rule: MemberRule) -> String {
    template
        .replace("{{m}}", &rule.m.to_string())
        .replace("{{n}}", &rule.n.to_string())
}

/// The words a rule is said with (`ruleTitle` / `ruleChipText`).
fn rule_words(m: &Value, rule: MemberRule) -> (String, String) {
    let p = &m["panel"];
    let s = |v: &Value| v.as_str().unwrap().to_owned();
    if rule.solo {
        (s(&p["titleSolo"]), s(&p["chipSolo"]))
    } else {
        (fill(&s(&p["title"]), rule), fill(&s(&p["chip"]), rule))
    }
}

#[test]
fn every_locale_fills_every_rule_word_and_one_member_reads_as_one() {
    let all = locales();
    assert!(all.len() >= 15, "{all:?}");
    let en = catalog("en");
    assert_eq!(
        rule_words(&en, MemberRule::of(2, 3)).0,
        "Needs 2 of 3 members"
    );
    assert_eq!(rule_words(&en, MemberRule::of(2, 4)).1, "Needs 2 of 4");
    assert_eq!(rule_words(&en, MemberRule::of(1, 1)).1, "One member");
    assert!(!rule_words(&en, MemberRule::of(1, 1)).0.contains("1 of 1"));
    for zh in ["zh", "zh-TW"] {
        assert_eq!(
            rule_words(&catalog(zh), MemberRule::of(2, 4)).1,
            "需 4 位中的 2 位"
        );
    }
    for locale in &all {
        let m = catalog(locale);
        let (title, chip) = rule_words(&m, MemberRule::of(2, 3));
        let (solo_title, solo_chip) = rule_words(&m, MemberRule::of(1, 1));
        for text in [&title, &chip, &solo_title, &solo_chip] {
            assert!(!text.trim().is_empty(), "{locale}");
            assert!(!text.contains("{{"), "{locale}: {text}");
        }
        assert!(chip.contains('2') && chip.contains('3'), "{locale}: {chip}");
        // The chip is no longer the bare "2 of 3" the stepper shows.
        let bare = fill(
            m["create"]["thresholdValue"].as_str().unwrap(),
            MemberRule::of(2, 3),
        );
        assert_ne!(chip, bare, "{locale}");
        for key in [
            ("proposal", "withdraw"),
            ("create", "createdBodySolo"),
            ("create", "sharedSolo"),
            ("presence", "ruleDiffers"),
        ] {
            assert!(
                !m[key.0][key.1].as_str().unwrap_or("").trim().is_empty(),
                "{locale}: {key:?}"
            );
        }
        // The refusals each have their sentence, every one the core gives.
        for kind in [
            "notAnAddress",
            "noOne",
            "theMultisig",
            "alreadyMember",
            "notMember",
            "lastMember",
            "thresholdTooHigh",
            "thresholdZero",
            "belowThreshold",
            "unchanged",
            "tooMany",
            "changeWaiting",
        ] {
            assert!(
                !m["members"]["refuse"][kind]
                    .as_str()
                    .unwrap_or("")
                    .trim()
                    .is_empty(),
                "{locale}: refuse.{kind}"
            );
        }
    }
}

#[test]
fn the_reviews_words_are_plain_words() {
    for locale in locales().iter().filter(|l| *l != "en") {
        let m = catalog(locale);
        for (group, key) in [
            ("review", "sourceProbe"),
            ("review", "writeGuard"),
            ("review", "writeModuleGuard"),
            ("review", "writeHandler"),
            ("authority", "handler"),
            ("authority", "riskGuard"),
            ("authority", "riskHandler"),
        ] {
            let text = m[group][key].as_str().unwrap().to_lowercase();
            let words: Vec<&str> = text.split(|c: char| !c.is_alphanumeric()).collect();
            assert!(
                !words
                    .iter()
                    .any(|w| matches!(*w, "guard" | "guards" | "handler")),
                "{locale}: {group}.{key}: {text}"
            );
        }
    }
}

// =====================================================================================
// The pure vectors
// =====================================================================================

/// `expected` within `actual` (vitest's `toMatchObject`): objects by the keys
/// expected, arrays element for element, addresses whatever their case.
fn matches(actual: &Value, expected: &Value) -> bool {
    match (actual, expected) {
        (Value::Object(a), Value::Object(e)) => e
            .iter()
            .all(|(k, v)| a.get(k).map_or(v.is_null(), |x| matches(x, v))),
        (Value::Array(a), Value::Array(e)) => {
            a.len() == e.len() && a.iter().zip(e).all(|(x, y)| matches(x, y))
        }
        (Value::String(a), Value::String(e)) if a.len() == 42 && a.starts_with("0x") => {
            a.eq_ignore_ascii_case(e)
        }
        (a, e) => a == e,
    }
}

fn members_of(v: &Value) -> ChainMembers {
    serde_json::from_value(v.clone()).unwrap()
}

fn strings(v: &Value) -> Vec<String> {
    serde_json::from_value(v.clone()).unwrap()
}

fn replay(input: &Value, expect: &Value) -> Result<(), String> {
    let f = input["fn"].as_str().unwrap();
    let got: Value = match f {
        "memberProblem" => json!(member_problem(
            input["typed"].as_str().unwrap(),
            &strings(&input["owners"])
        )),
        "validName" => json!(multisig::valid_name(input["name"].as_str().unwrap())),
        "looksLikeName" => json!(looks_like_name(input["typed"].as_str().unwrap())),
        "nameService" => json!(name_service(input["name"].as_str().unwrap()).0),
        "plan" => {
            let chains: Vec<NetworkMembers> =
                serde_json::from_value(input["chains"].clone()).unwrap();
            let change: MemberChange = serde_json::from_value(input["change"].clone()).unwrap();
            let multisig = input["multisig"].as_str().unwrap();
            let plan = plan_member_change(multisig, &chains, &change);
            let mut expect = expect.clone();
            if let Some(nets) = expect.get_mut("networks").and_then(Value::as_array_mut) {
                for (i, n) in nets.iter_mut().enumerate() {
                    if let Some(prefix) = n.as_object_mut().and_then(|o| o.remove("dataPrefix")) {
                        let data = data_of(&plan.networks[i]);
                        if !data.starts_with(prefix.as_str().unwrap()) {
                            return Err(format!("network {i}: {data} does not start {prefix}"));
                        }
                    }
                }
            }
            if let Some(kind) = expect.as_object_mut().and_then(|o| o.remove("reviewed")) {
                let MemberVerdict::Ready { tx, .. } = &plan.networks[0].verdict else {
                    return Err("not built".to_owned());
                };
                let read = multisig::review(chains[0].chain_id, multisig, tx).unwrap();
                let action = serde_json::to_value(read.action).unwrap();
                if action["kind"] != kind {
                    return Err(format!("reviewed as {action}"));
                }
            }
            let got = serde_json::to_value(&plan).unwrap();
            return if matches(&got, &expect) {
                Ok(())
            } else {
                Err(format!("got {got}"))
            };
        }
        "memberState" => {
            let items: Vec<WaitingProposal> =
                serde_json::from_value(input["items"].clone()).unwrap();
            serde_json::to_value(member_state_of(&members_of(&input["members"]), &items)).unwrap()
        }
        "deployedMembers" => {
            let chains: Vec<NetworkMembers> =
                serde_json::from_value(input["chains"].clone()).unwrap();
            let read: Vec<&ChainMembers> = chains.iter().map(|c| &c.members).collect();
            json!(deployed_members(&read))
        }
        "groups" => {
            let items: Vec<WaitingProposal> =
                serde_json::from_value(input["items"].clone()).unwrap();
            let order: Vec<u64> = serde_json::from_value(input["order"].clone()).unwrap();
            let names = input["names"].clone();
            let groups = member_change_groups(&items, &order);
            let name = |id: u64| {
                names[id.to_string()]
                    .as_str()
                    .map_or_else(|| id.to_string(), str::to_owned)
            };
            json!({
                "count": groups.len(),
                "progress": groups.iter().map(|g| progress_line(g, name)).collect::<Vec<_>>(),
                "chains": groups.iter().map(|g| g.parts.iter().map(|p| p.chain_id).collect::<Vec<_>>()).collect::<Vec<_>>(),
            })
        }
        "changeKey" => {
            let action: Action = serde_json::from_value(input["action"].clone()).unwrap();
            json!(change_key(Some(&action)))
        }
        "changesMembers" => {
            let action: Option<Action> = serde_json::from_value(input["action"].clone()).unwrap();
            json!(changes_members(action.as_ref()))
        }
        "reading" => {
            let calls: Vec<CallArg> = serde_json::from_value(input["calls"].clone()).unwrap();
            let multisig = input["multisig"].as_str().unwrap();
            let tx = multisig::calls_tx(&calls, "0x0").unwrap();
            let read = multisig::review(input["chainId"].as_u64().unwrap(), multisig, &tx).unwrap();
            let action = read
                .action
                .ok_or_else(|| format!("not signed: {:?}", read.refusal))?;
            let changes = vela_core::multisig_reading::member_changes(
                &action,
                &strings(&input["owners"]),
                input["threshold"].as_u64().unwrap(),
            )
            .unwrap();
            serde_json::to_value(changes).unwrap()
        }
        "drift" => serde_json::to_value(
            multisig::member_drift(
                &strings(&input["original"]),
                input["threshold"].as_u64().unwrap(),
                &strings(&input["current"]),
                input["currentThreshold"].as_u64().unwrap(),
            )
            .unwrap(),
        )
        .unwrap(),
        "rules" => {
            let owners = strings(&input["owners"]);
            let original = MemberRule::of(input["threshold"].as_u64().unwrap(), owners.len());
            let chains: Vec<ChainMembers> =
                serde_json::from_value(input["chains"].clone()).unwrap();
            let read: Vec<&ChainMembers> = chains.iter().collect();
            let rules = current_rules(original, &read);
            json!({ "rules": rules, "chip": rule_chip(&rules) })
        }
        "anywhere" => {
            let chains: Vec<ChainMembers> =
                serde_json::from_value(input["chains"].clone()).unwrap();
            let read: Vec<&ChainMembers> = chains.iter().collect();
            json!(members_anywhere(&strings(&input["owners"]), &read))
        }
        other => return Err(format!("no such function {other}")),
    };
    if matches(&got, expect) {
        Ok(())
    } else {
        Err(format!("got {got}"))
    }
}

#[test]
fn the_member_vectors_hold() {
    let path = format!(
        "{}/tests/vectors/multisig/members.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let vectors: Vec<Value> =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(vectors.len() >= 50, "{}", vectors.len());
    let failed: Vec<String> = vectors
        .iter()
        .filter_map(|v| {
            replay(&v["input"], &v["expect"])
                .err()
                .map(|e| format!("{}: {e}", v["name"].as_str().unwrap()))
        })
        .collect();
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}

#[test]
fn member_problems_are_said_in_the_webs_words() {
    assert_eq!(
        serde_json::to_value(MemberProblem::InvalidAddress).unwrap(),
        json!("invalidAddress")
    );
    assert_eq!(
        member_problem(D, &[D.to_owned()]),
        Some(MemberProblem::Duplicate)
    );
}

// =====================================================================================
// One name for the change: the members view's per-network proposal is the
// proposals machine's `Draft`, dispatched as it is.
// =====================================================================================

#[test]
fn a_networks_proposal_is_dispatched_as_it_is_into_the_proposals_machines_draft() {
    use vela_core::app::multisig_proposals::{self as proposals, MultisigProposals};
    let here = fixture(0);
    let g = genesis(&[&here.address, B, C], 2, "0x0d");
    let mut world = World {
        locals: vec![here.clone()],
        ..World::default()
    };
    world.deploy(
        GNOSIS,
        &g.address,
        DeployedSafe::clean(&[&here.address, B, C], 2),
    );
    let mut account = account_of(&g);
    account.chain_ids = vec![GNOSIS];
    let mut members = shown(&mut world, &account, &[GNOSIS]);
    let change = MemberChange::Add {
        owner: D.to_owned(),
        threshold: Some(2),
    };
    run(
        &mut members,
        &mut world,
        Event::Draft {
            change: Some(change.clone()),
        },
    );
    let network = on(&plan_of(&members), GNOSIS);
    // The shell's whole job: `{ type: "draft", chain_id, proposal }`, both
    // fields taken from the row as they are.
    let wire = serde_json::to_value(&network).unwrap();
    let draft: proposals::Event = serde_json::from_value(json!({
        "type": "draft",
        "chain_id": wire["chain_id"],
        "proposal": wire["proposal"],
    }))
    .unwrap();
    assert!(matches!(
        &draft,
        proposals::Event::Draft {
            chain_id: GNOSIS,
            proposal: ProposalKind::MemberChange { change: c }
        } if *c == change
    ));

    let mut sut: DomainDriver<MultisigProposals> = DomainDriver::new();
    sut.dispatch(proposals::Event::Start {
        service_url: world.service_url.clone(),
        accounts: world.locals.clone(),
        networks: vec![GNOSIS],
        known: Vec::new(),
    });
    settle_reads_first(&mut sut, &mut world, 5_000);
    sut.dispatch(proposals::Event::Follow {
        account: Some(account.clone()),
    });
    settle_reads_first(&mut sut, &mut world, 5_000);
    sut.dispatch(draft);
    settle_reads_first(&mut sut, &mut world, 5_000);
    let shown = sut
        .view()
        .draft
        .expect("the draft the members view asked for");
    assert!(
        matches!(
            &shown.item.review.action,
            Some(Action::AddOwner { owner, .. }) if owner.eq_ignore_ascii_case(D)
        ),
        "{:?}",
        shown.item.review.action
    );
    // The transaction the proposals machine built is the one the members view showed.
    let MemberVerdict::Ready { tx, .. } = &network.verdict else {
        unreachable!()
    };
    assert_eq!(
        shown.item.tx.data.to_ascii_lowercase(),
        tx.data.to_ascii_lowercase()
    );
}
