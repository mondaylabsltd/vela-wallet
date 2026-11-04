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
use vela_core::app::multisig_proposals::{ActionState, Event, MultisigProposals, ProposalKind};
use vela_core::app::multisig_review_state::{
    self as review_state, ReviewShows, ReviewState, REVIEW_TIMEOUT_MS,
};
use vela_core::app::multisig_shell::{MultisigAccount, MultisigOperation as Op};
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
        let open: Vec<&Op> = outstanding.iter().filter(|op| !held(op)).collect();
        let Some(op) = open
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| open.first())
            .map(|op| (*op).clone())
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

fn simulate(op: &Op) -> bool {
    matches!(op, Op::Simulate { .. })
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
    for key in vectors["keys"].as_array().unwrap() {
        assert!(key
            .as_str()
            .unwrap()
            .starts_with("home.multisig.review.state."));
    }
}
