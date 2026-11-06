//! §12: every reviewed thing has an explicit review state — `loading`,
//! `ready`, `failed {reason_key}` — and nothing is signed unless it is
//! `ready` (the Bybit rule). The proposals machine's drafts and proposals,
//! bounded by `REVIEW_TIMEOUT_MS`, read again on `review_again`; the core
//! refuses to sign on anything else whatever a shell lets the person press;
//! and the shared door vectors (`tests/vectors/multisig/review-state.json`)
//! every shell replays.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::Value;
use support::DomainDriver;
use vela_core::app::multisig_gather::REVIEW_ASK_MS;
use vela_core::app::multisig_proposals::{ActionState, Event, MultisigProposals, ProposalKind};
use vela_core::app::multisig_review_state::{
    self as review_state, ReviewShows, ReviewState, REVIEW_TIMEOUT_MS,
};
use vela_core::app::multisig_shell::{
    MultisigAccount, MultisigOperation as Op, PasskeyPurpose, SendPurpose,
};
use vela_core::multisig::Transfer;
use world::{fixture, genesis, settle_reads_first, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

const B: &str = "0x00000000000000000000000000000000000000bb";

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle_reads_first(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// As [`run`], never answering what `held` names (a shell that does not
/// answer a simulation): every other operation first, a wait only when
/// nothing else is outstanding — so the review's bound runs out.
fn run_holding(sut: &mut Sut, world: &mut World, event: Event, held: impl Fn(&Op) -> bool) {
    sut.dispatch(event);
    for _ in 0..20_000 {
        let outstanding = sut.outstanding();
        let open: Vec<Op> = outstanding.into_iter().filter(|op| !held(op)).collect();
        let Some(op) = open
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| world::soonest_wait(&open))
            .cloned()
        else {
            return;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    panic!("the machine never settled");
}

/// As [`run`], answering nothing but the reads that come before the review
/// (the chain, the mailbox): the review is still being read.
fn dispatch_only(sut: &mut Sut, event: Event) {
    sut.dispatch(event);
}

fn is_simulate(op: &Op) -> bool {
    matches!(op, Op::Simulate { .. })
}

/// A simulation never answered, and the core's own bound on that one
/// question (`REVIEW_ASK_MS`, r14-13) never run out either: what is left to
/// end the review is its `REVIEW_TIMEOUT_MS`.
fn simulate(op: &Op) -> bool {
    is_simulate(op) || matches!(op, Op::Wait { ms } if *ms == REVIEW_ASK_MS)
}

/// A 2-of-3 of the three fixture accounts, used on 100; `here`: which of
/// them are on this device.
fn setup(here: &[usize]) -> (World, Sut, MultisigAccount) {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = genesis(&refs, 2, "0x31");
    let account: MultisigAccount =
        serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap();
    let mut world = World {
        locals: here.iter().map(|i| fixture(*i)).collect(),
        ..World::default()
    };
    let sut = follow(&mut world, &account);
    (world, sut, account)
}

/// A machine on this world's device, following `account`.
fn follow(world: &mut World, account: &MultisigAccount) -> Sut {
    let mut sut = Sut::new();
    run(
        &mut sut,
        world,
        Event::Start {
            service_url: SERVICE.to_owned(),
            accounts: world.locals.clone(),
            networks: vec![100, 8453],
            known: Vec::new(),
        },
    );
    run(
        &mut sut,
        world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    sut
}

fn transfer() -> Event {
    Event::Draft {
        chain_id: 100,
        proposal: ProposalKind::Transfers {
            transfers: vec![Transfer {
                token: None,
                to: B.to_owned(),
                amount: "1".to_owned(),
            }],
        },
    }
}

fn draft_state(sut: &Sut) -> ReviewState {
    sut.view().draft.expect("a draft").review_state
}

fn door_of_draft(sut: &Sut) -> review_state::SignDoor {
    let d = sut.view().draft.expect("a draft");
    review_state::door(Some(&d.review_state), d.review.as_ref())
}

// -- drafts ------------------------------------------------------------------------------

/// The draft is drawn first and its review a moment later: until then it is
/// `loading`, and the door is shut — never "allowed" by default.
#[test]
fn a_draft_is_loading_until_its_review_is_read_and_only_then_signable() {
    let (mut world, mut sut, _) = setup(&[0]);
    // The chain and the mailbox answered, the review's own reads not yet.
    run_holding(&mut sut, &mut world, transfer(), |op| {
        simulate(op) || matches!(op, Op::Wait { ms } if *ms == REVIEW_TIMEOUT_MS)
    });
    let draft = sut.view().draft.expect("drawn first");
    assert_eq!(draft.review_state, ReviewState::Loading);
    assert!(draft.review.is_none());
    let door = door_of_draft(&sut);
    assert_eq!(door.shows, ReviewShows::Loading);
    assert!(!door.enabled, "nothing can be signed while it is read");
    // Pressed anyway (a shell that let it through): the core signs nothing.
    run_holding(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
        simulate,
    );
    assert!(world.passkeys.is_empty(), "no passkey on a review not read");
    // The simulation answers: read, and signable.
    let (mut world2, mut sut2, _) = setup(&[0]);
    run(&mut sut2, &mut world2, transfer());
    assert_eq!(draft_state(&sut2), ReviewState::Ready);
    assert!(door_of_draft(&sut2).enabled);
    run(
        &mut sut2,
        &mut world2,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(matches!(
        sut2.view().actions["propose"],
        ActionState::Done { .. }
    ));
    assert_eq!(world2.passkeys.len(), 1);
}

/// A review that does not come back in `REVIEW_TIMEOUT_MS` is `failed` with
/// "couldn't read it in time" — not loading for ever, not allowed. Nothing is
/// signed; "Try again" (`review_again`) reads it again and it can be signed.
#[test]
fn a_draft_review_that_times_out_fails_blocks_signing_and_recovers_on_try_again() {
    let (mut world, mut sut, _) = setup(&[0]);
    run_holding(&mut sut, &mut world, transfer(), simulate);
    assert_eq!(
        draft_state(&sut),
        ReviewState::failed(review_state::REASON_TIMED_OUT)
    );
    let door = door_of_draft(&sut);
    assert_eq!(door.shows, ReviewShows::Failed);
    assert!(!door.enabled && door.retry);
    assert_eq!(
        door.reason_key.as_deref(),
        Some("home.multisig.review.state.timedOut")
    );
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert_eq!(
        sut.view().actions["propose"],
        ActionState::SigningUnchecked,
        "the core refuses a review that was not read"
    );
    assert!(world.passkeys.is_empty());
    // Try again: loading at once.
    sut.drop_matching(simulate);
    dispatch_only(
        &mut sut,
        Event::ReviewAgain {
            chain_id: None,
            safe_tx_hash: None,
        },
    );
    assert_eq!(draft_state(&sut), ReviewState::Loading);
    settle_reads_first(&mut sut, &mut world, 20_000);
    assert_eq!(draft_state(&sut), ReviewState::Ready);
    assert!(door_of_draft(&sut).enabled);
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(matches!(
        sut.view().actions["propose"],
        ActionState::Done { .. }
    ));
    assert_eq!(world.passkeys.len(), 1);
}

/// Every kind of draft carries the state: a rename, a member change, a
/// rejection's cancellation, a site's calls.
#[test]
fn every_kind_of_draft_carries_its_review_state() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::Rename {
                name: "Calm Harbor".to_owned(),
            },
        },
    );
    assert_eq!(draft_state(&sut), ReviewState::Ready);
    run(&mut sut, &mut world, Event::Withdraw);
    // A rejection: the cancellation at a number, reviewed like any draft —
    // and shut while its review is read.
    run_holding(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::Cancel {
                nonce: "0".to_owned(),
            },
        },
        |op| simulate(op) || matches!(op, Op::Wait { ms } if *ms == REVIEW_TIMEOUT_MS),
    );
    let _ = account;
    if let Some(d) = sut.view().draft {
        // A cancellation moves nothing (no simulation to hold): read at once
        // or still being read — never anything else.
        assert!(
            matches!(d.review_state, ReviewState::Loading | ReviewState::Ready),
            "{:?}",
            d.review_state
        );
        let door = review_state::door(Some(&d.review_state), d.review.as_ref());
        assert!(
            !door.enabled || d.review_state.is_ready(),
            "open only when ready"
        );
    }
}

// -- proposals ---------------------------------------------------------------------------

/// Another member's device: the proposal is listed, its review not asked
/// yet — approving signs nothing until it is read; `review` makes it
/// `loading` at once, then `ready`, and only then does approve sign.
#[test]
fn a_proposal_is_approved_only_once_its_review_is_ready() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, transfer());
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let ActionState::Done { safe_tx_hash, .. } = sut.view().actions["propose"].clone() else {
        panic!("proposed");
    };
    // Member 1's own device.
    world.locals = vec![fixture(1)];
    world.passkeys.clear();
    let mut other = follow(&mut world, &account);
    let key = format!("100:{safe_tx_hash}");
    assert!(
        !other.view().review_states.contains_key(&key),
        "not asked yet"
    );
    let approve = Event::Approve {
        chain_id: 100,
        safe_tx_hash: safe_tx_hash.clone(),
        owner: fixture(1).address,
    };
    run(&mut other, &mut world, approve.clone());
    assert_eq!(
        other.view().actions[&format!("approve:{safe_tx_hash}")],
        ActionState::SigningUnchecked
    );
    assert!(world.passkeys.is_empty(), "no passkey on an unread review");
    dispatch_only(
        &mut other,
        Event::Review {
            chain_id: 100,
            safe_tx_hash: safe_tx_hash.clone(),
            blind: false,
        },
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Loading);
    settle_reads_first(&mut other, &mut world, 20_000);
    let view = other.view();
    assert_eq!(view.review_states[&key], ReviewState::Ready);
    assert!(review_state::door(view.review_states.get(&key), view.reviews.get(&key)).enabled);
    run(&mut other, &mut world, approve);
    assert!(matches!(
        other.view().actions[&format!("approve:{safe_tx_hash}")],
        ActionState::Done { .. }
    ));
    assert_eq!(world.passkeys.len(), 1);
}

/// A proposal's review that times out is `failed`; a shell asking again
/// while it is read never pushes the bound back (one read at a time); a
/// review read earlier is not unsaid by a later read that fails; and a
/// proposal no longer listed is `failed {gone}`, never loading for ever.
#[test]
fn a_proposal_review_is_bounded_kept_once_read_and_gone_when_unlisted() {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, transfer());
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let ActionState::Done { safe_tx_hash, .. } = sut.view().actions["propose"].clone() else {
        panic!("proposed");
    };
    world.locals = vec![fixture(1)];
    let mut other = follow(&mut world, &account);
    let key = format!("100:{safe_tx_hash}");
    let review = Event::Review {
        chain_id: 100,
        safe_tx_hash: safe_tx_hash.clone(),
        blind: false,
    };
    // Asked, then asked again while it is read: one bound only.
    other.dispatch(review.clone());
    other.dispatch(review.clone());
    let bounds = other
        .outstanding()
        .iter()
        .filter(|op| matches!(op, Op::Wait { ms } if *ms == REVIEW_TIMEOUT_MS))
        .count();
    assert_eq!(
        bounds, 1,
        "one read at a time: the bound is never pushed back"
    );
    run_holding(
        &mut other,
        &mut world,
        Event::Refresh { fresh: false },
        simulate,
    );
    assert_eq!(
        other.view().review_states[&key],
        ReviewState::failed(review_state::REASON_TIMED_OUT)
    );
    other.drop_matching(simulate);
    // Try again: read.
    run(
        &mut other,
        &mut world,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(safe_tx_hash.clone()),
        },
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    // Read again quietly (a sheet's refresh) and it times out: still ready.
    run_holding(&mut other, &mut world, review, simulate);
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    other.drop_matching(simulate);
    // One no longer listed.
    let gone = "0x".to_owned() + &"ab".repeat(32);
    run(
        &mut other,
        &mut world,
        Event::Review {
            chain_id: 100,
            safe_tx_hash: gone.clone(),
            blind: false,
        },
    );
    assert_eq!(
        other.view().review_states[&format!("100:{gone}")],
        ReviewState::failed(review_state::REASON_GONE)
    );
}

/// What a draft proposed from a read review carries into its proposal: the
/// review it was proposed from, `ready` — its execution signs from it.
#[test]
fn a_proposed_draft_keeps_its_ready_review() {
    let (mut world, mut sut, _) = setup(&[0, 1]);
    run(&mut sut, &mut world, transfer());
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let ActionState::Done { safe_tx_hash, .. } = sut.view().actions["propose"].clone() else {
        panic!("proposed");
    };
    let key = format!("100:{safe_tx_hash}");
    assert_eq!(sut.view().review_states[&key], ReviewState::Ready);
    assert!(sut.view().reviews[&key].is_object());
}

// -- D1: neither safety check read (real-device pass 2) -----------------------------------

/// A slow network: every read the state guard makes (`debug_traceCall`, the
/// Safe's `simulateAndRevert` probe) gives up, and so does the simulation.
fn checks_unread(world: &mut World) {
    let probe = format!(
        "0x{}",
        alloy_primitives::hex::encode(
            &alloy_primitives::keccak256(b"simulateAndRevert(address,bytes)")[..4]
        )
    );
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        (method == "debug_traceCall" || probing).then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "timed out".to_owned(),
            }
        })
    }));
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
}

/// As [`run`], keeping what each passkey was asked for (the page's words).
fn run_purposes(sut: &mut Sut, world: &mut World, event: Event) -> Vec<PasskeyPurpose> {
    sut.dispatch(event);
    let mut purposes = Vec::new();
    for _ in 0..20_000 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| world::soonest_wait(&outstanding))
            .cloned()
        else {
            return purposes;
        };
        if let Op::Passkey { purpose, .. } = &op {
            purposes.push(purpose.clone());
        }
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    panic!("the machine never settled");
}

fn approve_anyway(purposes: &[PasskeyPurpose]) -> bool {
    !purposes.is_empty()
        && purposes
            .iter()
            .all(|p| matches!(p, PasskeyPurpose::Approve { anyway: true, .. }))
}

/// Real-device pass 2, D1: each read of a review gives up at its bound on a
/// slow network, so at ~36 s the review came back with neither the state
/// guard nor the simulation read — "ready", and signed in one tap (the test
/// hand approved and executed so). It stays readable, but the core's sign
/// rule asks for "… anyway": the door says it (danger line, Try again), and
/// every passkey — "Propose and approve", "Approve", and the one press of
/// "Approve and execute" — is asked as "… anyway", the words the trusted
/// page draws.
#[test]
fn a_review_neither_check_read_is_ready_but_every_press_signs_only_anyway() {
    let (mut world, mut sut, account) = setup(&[0]);
    checks_unread(&mut world);
    run(&mut sut, &mut world, transfer());
    let draft = sut.view().draft.expect("a draft");
    assert_eq!(
        draft.review_state,
        ReviewState::Ready,
        "readable, not failed"
    );
    let door = review_state::door(Some(&draft.review_state), draft.review.as_ref());
    assert!(door.enabled && door.anyway && door.retry);
    assert_eq!(
        door.reason_key.as_deref(),
        Some(review_state::UNCHECKED_KEY)
    );
    // Propose and approve: "… anyway".
    let purposes = run_purposes(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(approve_anyway(&purposes), "{purposes:?}");
    let ActionState::Done { safe_tx_hash, .. } = sut.view().actions["propose"].clone() else {
        panic!("proposed: {:?}", sut.view().actions["propose"]);
    };
    // Another member's device: its review is unchecked too; its Try again
    // reads it again (loading at once), and it stays "… anyway".
    world.locals = vec![fixture(1)];
    world.passkeys.clear();
    let mut other = follow(&mut world, &account);
    let key = format!("100:{safe_tx_hash}");
    run(
        &mut other,
        &mut world,
        Event::Review {
            chain_id: 100,
            safe_tx_hash: safe_tx_hash.clone(),
            blind: false,
        },
    );
    let view = other.view();
    assert_eq!(view.review_states[&key], ReviewState::Ready);
    let door = review_state::door(view.review_states.get(&key), view.reviews.get(&key));
    assert!(door.enabled && door.anyway && door.retry);
    dispatch_only(
        &mut other,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(safe_tx_hash.clone()),
        },
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Loading);
    settle_reads_first(&mut other, &mut world, 20_000);
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    // r13-2: ready again right after Try again, the checks still unread —
    // never one plain tap in that window either.
    let view = other.view();
    let door = review_state::door(view.review_states.get(&key), view.reviews.get(&key));
    assert!(door.enabled && door.anyway && door.retry);
    // The one press: approve and execute — the executor's own operation
    // (its approval as `msg.sender`) is asked "… anyway" too: its page says
    // "Execute anyway". r16-4: the press is on the fee its look drew.
    run(
        &mut other,
        &mut world,
        Event::Preview {
            chain_id: 100,
            safe_tx_hash: safe_tx_hash.clone(),
            executor: fixture(1).address,
        },
    );
    let look = other.view().previews[&format!(
        "100:{}:{}",
        safe_tx_hash.to_ascii_lowercase(),
        fixture(1).address.to_ascii_lowercase()
    )]
    .clone();
    let shown = look["fee"]["amount"]
        .as_str()
        .and_then(vela_core::app::multisig_shell::quantity)
        .map(|f| f.to_string());
    assert!(shown.is_some(), "{look}");
    world.purposes.clear();
    run(
        &mut other,
        &mut world,
        Event::ApproveExecute {
            chain_id: 100,
            safe_tx_hash: safe_tx_hash.clone(),
            executor: fixture(1).address,
            shown_fee: shown,
        },
    );
    assert!(matches!(
        other.view().actions[&format!("execute:{safe_tx_hash}")],
        ActionState::Done { .. }
    ));
    assert!(
        matches!(
            world.purposes.as_slice(),
            [Some(SendPurpose::Execute { anyway: true, .. })]
        ),
        "{:?}",
        world.purposes
    );
}

/// Round 13 (r13-2): a node that REFUSES (not hangs) answers every read at
/// once with an error, and the review was `ready` in 2–4 s with both checks
/// unread and a plain Approve (MS-B #9, a change of threshold). Whatever left
/// both unread — a hang, a refusal, the reads right after Try again — the
/// sign rule asks for "… anyway".
#[test]
fn a_refused_node_reads_neither_check_and_signs_only_anyway() {
    let (mut world, mut sut, _) = setup(&[0]);
    let probe = format!(
        "0x{}",
        alloy_primitives::hex::encode(
            &alloy_primitives::keccak256(b"simulateAndRevert(address,bytes)")[..4]
        )
    );
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        (method == "debug_traceCall" || probing).then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcError {
                code: Some(-32603),
                message: "connect ECONNREFUSED 127.0.0.1:8545".to_owned(),
                data: None,
            }
        })
    }));
    world.sim_reply = Some(
        r#"{"error":{"code":-32603,"message":"connect ECONNREFUSED 127.0.0.1:8545"}}"#.to_owned(),
    );
    run(&mut sut, &mut world, transfer());
    let draft = sut.view().draft.expect("a draft");
    assert_eq!(
        draft.review_state,
        ReviewState::Ready,
        "ready at once, not failed"
    );
    let door = review_state::door(Some(&draft.review_state), draft.review.as_ref());
    assert!(door.enabled && door.anyway && door.retry, "{door:?}");
    assert_eq!(
        door.reason_key.as_deref(),
        Some(review_state::UNCHECKED_KEY)
    );
    let purposes = run_purposes(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(approve_anyway(&purposes), "{purposes:?}");
}

/// D1: one of the two checks read keeps today's caution — readable, one
/// plain press, no danger line, a plain passkey.
#[test]
fn one_check_unread_alone_signs_plainly() {
    let (mut world, mut sut, _) = setup(&[0]);
    // This world's state guard cannot read the Safe's state; the simulation
    // answers: one check of two.
    world.sim_reply = Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned());
    run(&mut sut, &mut world, transfer());
    let draft = sut.view().draft.expect("a draft");
    assert_eq!(draft.review_state, ReviewState::Ready);
    let review = draft.review.clone().expect("a review");
    assert_eq!(review["state"]["verdict"]["kind"], "unavailable");
    assert_eq!(review["simulation"]["kind"], "deltas");
    let door = review_state::door(Some(&draft.review_state), draft.review.as_ref());
    assert!(door.enabled && !door.retry && door.reason_key.is_none());
    let purposes = run_purposes(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    assert!(!purposes.is_empty());
    assert!(
        purposes
            .iter()
            .all(|p| matches!(p, PasskeyPurpose::Approve { anyway: false, .. })),
        "{purposes:?}"
    );
}

// -- r14-13: a network that answered nothing is never "… anyway" ---------------------------

/// A proposal of `setup`'s multisig, proposed while everything answered,
/// and member 1's own device following it: (world, that device, key, hash).
fn proposed_on_another_device() -> (World, Sut, String, String) {
    let (mut world, mut sut, account) = setup(&[0]);
    run(&mut sut, &mut world, transfer());
    run(
        &mut sut,
        &mut world,
        Event::ConfirmDraft { as_member: None },
    );
    let ActionState::Done { safe_tx_hash, .. } = sut.view().actions["propose"].clone() else {
        panic!("proposed");
    };
    world.locals = vec![fixture(1)];
    world.passkeys.clear();
    let other = follow(&mut world, &account);
    let key = format!("100:{safe_tx_hash}");
    (world, other, key, safe_tx_hash)
}

fn review(hash: &str) -> Event {
    Event::Review {
        chain_id: 100,
        safe_tx_hash: hash.to_owned(),
        blind: false,
    }
}

fn door_at(sut: &Sut, key: &str) -> review_state::SignDoor {
    let view = sut.view();
    review_state::door(view.review_states.get(key), view.reviews.get(key))
}

/// A read of the multisig's network (any method).
fn chain_read(op: &Op) -> bool {
    matches!(op, Op::Rpc { chain_id: 100, .. })
}

/// Round 14 (r14-13): with every node of the network refusing, the review's
/// reads all came back unanswered — not the multisig's own state, not a
/// check — and the web drew it `ready` with "… anyway" while Android said it
/// could not be read. One rule now: a review its network answered nothing
/// of is `failed {chainUnread}` — never signable, not even "… anyway" —
/// with Try again, which reads it again once the network answers.
#[test]
fn a_review_its_network_answered_nothing_of_fails_never_anyway() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    world.silent.insert(100);
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    world.recipients.insert(B.to_owned(), (None, None));
    run(&mut other, &mut world, review(&hash));
    assert_eq!(
        other.view().review_states[&key],
        ReviewState::failed(review_state::REASON_CHAIN_UNREAD)
    );
    let door = door_at(&other, &key);
    assert_eq!(door.shows, ReviewShows::Failed);
    assert!(!door.enabled && !door.anyway && door.retry, "{door:?}");
    assert_eq!(
        door.reason_key.as_deref(),
        Some("home.multisig.review.state.chainUnread")
    );
    // Pressed anyway (a shell that let it through): nothing is signed.
    run(
        &mut other,
        &mut world,
        Event::Approve {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            owner: fixture(1).address,
        },
    );
    assert!(world.passkeys.is_empty(), "no passkey on a review not read");
    // The network answers again (its simulation too): Try again reads it,
    // plain.
    world.silent.clear();
    world.sim_reply = Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned());
    world.recipients.clear();
    run(
        &mut other,
        &mut world,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(hash.clone()),
        },
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    let door = door_at(&other, &key);
    assert!(door.enabled && !door.anyway && door.reason_key.is_none());
}

/// Round 15 (r15-13): a failed review read itself again — "Reading" at
/// 35 s, failed at 45 s — because a shell's timer asked `review` every 30 s
/// and the core turned any review not ready back to loading. §12: a failed
/// review is said, with Try again, and never retried behind the person's
/// back: `review` leaves it failed (no read is made); `review_again` — the
/// person's Try again — reads it.
#[test]
fn a_failed_review_is_read_again_only_on_the_persons_try_again() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    world.silent.insert(100);
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    run(&mut other, &mut world, review(&hash));
    let failed = ReviewState::failed(review_state::REASON_CHAIN_UNREAD);
    assert_eq!(other.view().review_states[&key], failed);
    // The network is back; the shell's timer (or a reopened sheet) asks.
    world.silent.clear();
    world.sim_reply = Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned());
    let asked = world.rpc_log.len();
    other.dispatch(review(&hash));
    assert_eq!(
        other.view().review_states[&key],
        failed,
        "never back to Reading on its own"
    );
    run(&mut other, &mut world, review(&hash));
    assert_eq!(other.view().review_states[&key], failed);
    assert_eq!(world.rpc_log.len(), asked, "nothing was read again");
    // The person's Try again reads it.
    run(
        &mut other,
        &mut world,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(hash.clone()),
        },
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
}

/// Round 15 (r15-12): right after an outage lifted, a review's Try again
/// landed for ~2 s as D1 — drawn, enabled, "… anyway" — before it was read
/// in full: the network's first answer (the multisig's own state, through
/// the pool's outage probe) came back while the checks asked beside it were
/// still turned away. A review never passes through a signable "anyway" on
/// its way from failed to read: checks that all came back as silence while
/// the network answered are asked once more within the same run.
#[test]
fn a_review_coming_back_from_an_outage_never_lands_anyway_on_the_way() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    world.silent.insert(100);
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    run(&mut other, &mut world, review(&hash));
    assert!(matches!(
        other.view().review_states[&key],
        ReviewState::Failed { .. }
    ));
    // The outage lifts: the network answers, but the first trace and probe
    // (and the first simulation) are still turned away as silent.
    world.silent.clear();
    let probe = format!(
        "0x{}",
        alloy_primitives::hex::encode(
            &alloy_primitives::keccak256(b"simulateAndRevert(address,bytes)")[..4]
        )
    );
    let turned_away = std::sync::Arc::new(std::sync::Mutex::new(0_u32));
    let count = turned_away.clone();
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        let mut n = count.lock().unwrap();
        ((method == "debug_traceCall" || probing) && *n < 2).then(|| {
            *n += 1;
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "timed out".to_owned(),
            }
        })
    }));
    world
        .sim_replies
        .push_back(Some(r#"{"unreachable":true}"#.to_owned()));
    world.sim_reply = Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned());
    run(
        &mut other,
        &mut world,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(hash.clone()),
        },
    );
    assert_eq!(
        *turned_away.lock().unwrap(),
        2,
        "both checks were turned away once"
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    let door = door_at(&other, &key);
    assert!(
        door.enabled && !door.anyway && door.reason_key.is_none(),
        "read in full, never anyway: {door:?}"
    );
}

/// As [`run`], watching the review under `key` after every answer: the
/// door must never stand enabled on "… anyway" before the read lands —
/// what a person sees between the press of Try again and the review.
/// Returns the clock when the review landed.
fn run_never_anyway_on_the_way(sut: &mut Sut, world: &mut World, event: Event, key: &str) -> f64 {
    sut.dispatch(event);
    for _ in 0..20_000 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| world::soonest_wait(&outstanding))
            .cloned()
        else {
            return world.now;
        };
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
        let view = sut.view();
        if view.review_states.get(key) == Some(&ReviewState::Loading) {
            let door = door_at(sut, key);
            assert!(!door.enabled, "nothing signable while it is read: {door:?}");
        }
    }
    panic!("the machine never settled");
}

fn probe_selector() -> String {
    format!(
        "0x{}",
        alloy_primitives::hex::encode(
            &alloy_primitives::keccak256(b"simulateAndRevert(address,bytes)")[..4]
        )
    )
}

/// Round 16 (r16-3): r15-12b was not fixed on Arc. Its nodes ANSWER a
/// state trace — with "method not found" — so the r15-12 re-ask (made only
/// when every check came back as silence) never ran: the network had just
/// come back, the probe and the simulation were still turned away, and the
/// review landed "unchecked" with an enabled "Approve anyway" for 1–5 s
/// until the next read. The rule now: a review that would land unchecked
/// while one of its checks is still silent asks those checks again, paced,
/// until they answer or the re-ask's own limit (`RECHECK_MS`) is reached;
/// only then is "unchecked" final. Failed → Reading → ready, every sign
/// button off on the way.
#[test]
fn a_review_whose_trace_is_refused_waits_for_its_silent_checks_before_unchecked() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    world.silent.insert(100);
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    run(&mut other, &mut world, review(&hash));
    assert!(matches!(
        other.view().review_states[&key],
        ReviewState::Failed { .. }
    ));
    // The outage lifts. The trace is refused by every node (Arc: "method
    // not found" — the world's default answer); the probe and the
    // simulation are turned away as silent for the first two asks each
    // (the read and one re-ask made at once), then answered.
    world.silent.clear();
    let probe = probe_selector();
    let turned_away = std::sync::Arc::new(std::sync::Mutex::new(0_u32));
    let count = turned_away.clone();
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        let mut n = count.lock().unwrap();
        (probing && *n < 2).then(|| {
            *n += 1;
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "connection refused".to_owned(),
            }
        })
    }));
    for _ in 0..2 {
        world
            .sim_replies
            .push_back(Some(r#"{"unreachable":true}"#.to_owned()));
    }
    world.sim_reply = Some(r#"{"result":[{"calls":[{"status":"0x1","logs":[]}]}]}"#.to_owned());
    run_never_anyway_on_the_way(
        &mut other,
        &mut world,
        Event::ReviewAgain {
            chain_id: Some(100),
            safe_tx_hash: Some(hash.clone()),
        },
        &key,
    );
    assert_eq!(
        *turned_away.lock().unwrap(),
        2,
        "the probe was turned away twice"
    );
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    let door = door_at(&other, &key);
    assert!(
        door.enabled && !door.anyway && door.reason_key.is_none(),
        "read in full, never anyway: {door:?}"
    );
}

/// r16-3, the other side: a network that answers its multisig's state but
/// whose checks stay silent (a node that never traces or simulates) is still
/// D1 — "… anyway" — but only once the re-ask has run to its own limit:
/// the review stays Reading (nothing signable) until then.
#[test]
fn checks_that_stay_silent_land_unchecked_only_at_the_reasks_own_limit() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    let probe = probe_selector();
    let asked = std::sync::Arc::new(std::sync::Mutex::new(0_u32));
    let count = asked.clone();
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        probing.then(|| {
            *count.lock().unwrap() += 1;
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "connection refused".to_owned(),
            }
        })
    }));
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    let pressed = world.now;
    let landed = run_never_anyway_on_the_way(&mut other, &mut world, review(&hash), &key);
    assert_eq!(other.view().review_states[&key], ReviewState::Ready);
    let door = door_at(&other, &key);
    assert!(door.enabled && door.anyway && door.retry, "{door:?}");
    assert!(
        landed - pressed >= f64::from(review_state::RECHECK_MS),
        "unchecked only once the re-ask ran to its limit ({} ms)",
        landed - pressed
    );
    assert!(
        *asked.lock().unwrap() > 2,
        "the silent probe was asked again, paced"
    );
}

/// r14-13, a HUNG network (no answer at all, on any shell): every read —
/// and a simulation the shell never ends — is ended by the core at
/// `REVIEW_ASK_MS`, so the review lands `failed {chainUnread}` well before
/// the review's own 40 s bound: the same answer, at the same time, on every
/// shell (Android's simulation held its review to "couldn't read it in
/// time" at ~50 s while the web's said "… anyway" at 26 s).
#[test]
fn a_hung_network_fails_the_review_by_the_cores_own_bound_on_every_question() {
    let (mut world, mut other, key, hash) = proposed_on_another_device();
    run_holding(&mut other, &mut world, review(&hash), |op| {
        chain_read(op)
            || is_simulate(op)
            || matches!(op, Op::RecipientRisk { .. })
            || matches!(op, Op::Wait { ms } if *ms == REVIEW_TIMEOUT_MS)
    });
    assert_eq!(
        other.view().review_states[&key],
        ReviewState::failed(review_state::REASON_CHAIN_UNREAD),
        "not timed out: every question was bounded"
    );
    assert!(!door_at(&other, &key).enabled);
}

/// r14-13: the network answered (the multisig's own state), and only the
/// two safety checks are unread — refused at once, a change of threshold
/// that names no recipient (MS-B #0): still drawn, "… anyway" (D1).
#[test]
fn only_the_two_checks_unread_stays_unchecked_anyway() {
    let (mut world, mut sut, _) = setup(&[0]);
    let probe = format!(
        "0x{}",
        alloy_primitives::hex::encode(
            &alloy_primitives::keccak256(b"simulateAndRevert(address,bytes)")[..4]
        )
    );
    world.rpc_hooks.push(Box::new(move |_, method, params| {
        let probing = method == "eth_call"
            && params[0]["data"]
                .as_str()
                .is_some_and(|d| d.to_ascii_lowercase().starts_with(&probe));
        (method == "debug_traceCall" || probing).then(|| {
            vela_core::app::multisig_shell::MultisigShellResult::RpcUnanswered {
                detail: "connection refused".to_owned(),
            }
        })
    }));
    world.sim_reply = Some(r#"{"unreachable":true}"#.to_owned());
    run(
        &mut sut,
        &mut world,
        Event::Draft {
            chain_id: 100,
            proposal: ProposalKind::MemberChange {
                change: vela_core::multisig::MemberChange::Threshold { threshold: 1 },
            },
        },
    );
    let draft = sut.view().draft.expect("a draft");
    assert_eq!(
        draft.review_state,
        ReviewState::Ready,
        "the network answered"
    );
    let door = review_state::door(Some(&draft.review_state), draft.review.as_ref());
    assert!(door.enabled && door.anyway && door.retry, "{door:?}");
    assert_eq!(
        door.reason_key.as_deref(),
        Some(review_state::UNCHECKED_KEY)
    );
}

/// r14-13: a simulation the shell never answers does not hold the review
/// to its 40 s bound: the core ends that one question, and the review lands
/// on what the network did answer (one check read: plain).
#[test]
fn a_simulation_a_shell_never_ends_is_ended_by_the_core() {
    let (mut world, mut sut, _) = setup(&[0]);
    run_holding(&mut sut, &mut world, transfer(), |op| {
        is_simulate(op) || matches!(op, Op::Wait { ms } if *ms == REVIEW_TIMEOUT_MS)
    });
    assert_eq!(draft_state(&sut), ReviewState::Ready, "not timed out");
    assert!(door_of_draft(&sut).enabled);
}

// -- the shared vectors ------------------------------------------------------------------

/// `tests/vectors/multisig/review-state.json`: the door, replayed through
/// the link door every shell calls (`review.door`).
#[test]
fn the_door_vectors_hold_through_the_link_door() {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/vectors/multisig/review-state.json"
    ))
    .unwrap();
    let vectors: Value = serde_json::from_str(&raw).unwrap();
    let cases = vectors["door"].as_array().unwrap();
    assert!(cases.len() >= 8);
    for case in cases {
        let got = vela_core::multisig_link::call_json("review.door", &case["args"].to_string())
            .unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got, case["expect"], "{}", case["name"]);
    }
    assert_eq!(
        vectors["timeout_ms"].as_u64(),
        Some(u64::from(REVIEW_TIMEOUT_MS))
    );
    // §18 (b): a sheet opened without a link, its transaction not listed.
    let missing = vectors["missing"].as_array().unwrap();
    assert!(missing.len() >= 5);
    for case in missing {
        let got = vela_core::multisig_link::call_json("review.missing", &case["args"].to_string())
            .unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got, case["expect"], "{}", case["name"]);
    }
    // r14-13: where a gathered review lands, through the same door.
    let settle = vectors["settle"].as_array().unwrap();
    assert!(settle.len() >= 5);
    for case in settle {
        let got = vela_core::multisig_link::call_json("review.settle", &case["args"].to_string())
            .unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got, case["expect"], "{}", case["name"]);
    }
    // r16-3: whether a review's checks are final for this read, through
    // the same door.
    let recheck = vectors["recheck"].as_array().unwrap();
    assert!(recheck.len() >= 6);
    for case in recheck {
        let got =
            vela_core::multisig_link::call_json("review.checksFinal", &case["args"].to_string())
                .unwrap_or_else(|e| panic!("{}: {e:?}", case["name"]));
        let got: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(got, case["expect"], "{}", case["name"]);
    }
    for key in vectors["keys"].as_array().unwrap() {
        assert!(key
            .as_str()
            .unwrap()
            .starts_with("home.multisig.review.state."));
    }
}
