//! r13-3: the step BEFORE a review exists — a draft being built (send's
//! "Propose and approve", a site's request), a proposal link being opened —
//! has the review's discipline: bounded by `OPEN_TIMEOUT_MS`, and when it
//! fails it names the read that failed (the network, the multisig service,
//! or both), with Try again. A hung network is never said as the service's
//! waiting transactions, and nothing spins without a bound.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::multisig_proposals::flows::LinkedStand;
use vela_core::app::multisig_proposals::{ActionState, Event, MultisigProposals, ProposalKind};
use vela_core::app::multisig_review_state::{
    self as review_state, OPEN_TIMEOUT_MS, REVIEW_TIMEOUT_MS,
};
use vela_core::app::multisig_shell::{MultisigAccount, MultisigOperation as Op};
use vela_core::multisig::{self, Transfer};
use world::{fixture, genesis, settle_reads_first, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

const B: &str = "0x00000000000000000000000000000000000000bb";

fn start(world: &World) -> Event {
    Event::Start {
        service_url: SERVICE.to_owned(),
        accounts: world.locals.clone(),
        networks: vec![100, 8453],
        known: Vec::new(),
    }
}

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle_reads_first(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// As [`run`], never answering what `held` names (a node or a service that
/// hangs: never answered, not refused); a wait only when nothing else is
/// outstanding — so a bound runs out. The waits each answered are returned.
fn run_holding(
    sut: &mut Sut,
    world: &mut World,
    event: Event,
    held: impl Fn(&Op) -> bool,
) -> Vec<u32> {
    sut.dispatch(event);
    let mut waits = Vec::new();
    for _ in 0..20_000 {
        let outstanding = sut.outstanding();
        let open: Vec<&Op> = outstanding.iter().filter(|op| !held(op)).collect();
        let Some(op) = open
            .iter()
            .find(|op| !matches!(op, Op::Wait { .. }))
            .or_else(|| open.first())
            .map(|op| (*op).clone())
        else {
            return waits;
        };
        if let Op::Wait { ms } = op {
            waits.push(ms);
        }
        let res = world.answer(&op);
        sut.resolve_matching(|o| *o == op, res);
    }
    panic!("the machine never settled");
}

fn chain_100(op: &Op) -> bool {
    matches!(op, Op::Rpc { chain_id: 100, .. })
}

fn service(op: &Op) -> bool {
    matches!(op, Op::Http { .. })
}

fn account() -> MultisigAccount {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let g = genesis(&refs, 2, "0x31");
    serde_json::from_value(world::stored_row(&g, &[100], true)).unwrap()
}

/// A device with member 0, following the 2-of-3, everything read once.
fn setup() -> (World, Sut, MultisigAccount) {
    let account = account();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let mut sut = Sut::new();
    let ev = start(&world);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    (world, sut, account)
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

fn unread(key: &str) -> ActionState {
    ActionState::Unread {
        reason_key: key.to_owned(),
    }
}

/// r13-3 (web send): the network hangs before the draft exists — the step
/// fails at `OPEN_TIMEOUT_MS` naming the NETWORK (was: 40 s, then "Couldn't
/// read this multisig's waiting transactions").
#[test]
fn a_draft_whose_network_hangs_fails_in_time_naming_the_network() {
    let (mut world, mut sut, _) = setup();
    let waits = run_holding(&mut sut, &mut world, transfer(), chain_100);
    assert!(waits.contains(&OPEN_TIMEOUT_MS), "bounded: {waits:?}");
    assert!(
        !waits.contains(&REVIEW_TIMEOUT_MS),
        "no review was ever asked"
    );
    let view = sut.view();
    assert!(view.draft.is_none());
    assert_eq!(
        view.actions["propose"],
        unread(review_state::UNREAD_CHAIN_KEY)
    );
}

/// r13-3: a network that refuses (answers nothing at once) is the network's
/// too — never the service's.
#[test]
fn a_draft_whose_network_refuses_names_the_network() {
    let (mut world, mut sut, _) = setup();
    world.silent.insert(100);
    run(&mut sut, &mut world, transfer());
    assert_eq!(
        sut.view().actions["propose"],
        unread(review_state::UNREAD_CHAIN_KEY)
    );
}

/// r13-3: the multisig service hangs, the network answers: said as the
/// service's (the words `mailboxUnreadable` always had), in time.
#[test]
fn a_draft_whose_service_hangs_names_the_service_in_time() {
    let (mut world, mut sut, _) = setup();
    let waits = run_holding(&mut sut, &mut world, transfer(), service);
    assert!(waits.contains(&OPEN_TIMEOUT_MS), "bounded: {waits:?}");
    assert_eq!(
        sut.view().actions["propose"],
        ActionState::MailboxUnreadable
    );
}

/// r13-3: neither answers — said as both (most likely this device's
/// connection), in time.
#[test]
fn a_draft_whose_network_and_service_hang_names_both() {
    let (mut world, mut sut, _) = setup();
    run_holding(&mut sut, &mut world, transfer(), |op| {
        chain_100(op) || service(op)
    });
    assert_eq!(
        sut.view().actions["propose"],
        unread(review_state::UNREAD_BOTH_KEY)
    );
}

/// r13-3: Try again (propose again) once the network answers: the draft is
/// built and its review read.
#[test]
fn try_again_after_the_network_returns_builds_the_draft() {
    let (mut world, mut sut, _) = setup();
    run_holding(&mut sut, &mut world, transfer(), chain_100);
    assert!(sut.view().draft.is_none());
    run(&mut sut, &mut world, transfer());
    let view = sut.view();
    let draft = view.draft.expect("built");
    assert!(draft.review_state.is_ready(), "{:?}", draft.review_state);
    assert!(!view.actions.contains_key("propose"));
}

/// r13-3: a site's request (the extension's `eth_sendTransaction`) is the
/// same draft: bounded, the network named — not an empty sheet for 48 s.
#[test]
fn a_sites_request_whose_network_hangs_is_said_in_time() {
    let (mut world, mut sut, account) = setup();
    let request = Event::DappRequest {
        id: "r1".to_owned(),
        account: Box::new(account.clone()),
        method: "eth_sendTransaction".to_owned(),
        params: json!([{ "from": account.address, "to": B, "value": "0x1" }]),
        origin: "https://app.example".to_owned(),
        chain_id: 100,
        limit_ms: Some(300_000.0),
    };
    let waits = run_holding(&mut sut, &mut world, request, chain_100);
    assert!(waits.contains(&OPEN_TIMEOUT_MS), "bounded: {waits:?}");
    assert_eq!(
        sut.view().actions["propose"],
        unread(review_state::UNREAD_CHAIN_KEY)
    );
}

// -- a proposal link ---------------------------------------------------------

fn stored_proposal(account: &MultisigAccount, tx: &multisig::SafeTx) -> Value {
    json!({
        "chainId": 100, "safe": account.address,
        "safeTxHash": multisig::safe_tx_hash(100, &account.address, tx).unwrap(),
        "safeTx": tx, "proposer": "", "approvals": [], "createdMs": 1, "updatedMs": 1,
    })
}

/// r13-3 (iOS cold link): the app opens on a proposal link and the network
/// hangs — "Opening the transaction…" ends at `OPEN_TIMEOUT_MS` with the read
/// still out named and Try again; Try again, the network back, opens it.
#[test]
fn a_link_whose_network_hangs_is_said_in_time_with_try_again() {
    let account = account();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(stored_proposal(&account, &tx));
    let mut sut = Sut::new();
    let ev = start(&world);
    run(&mut sut, &mut world, ev);
    // Cold: the multisig followed and its link opened at once, the network
    // hanging from the start.
    sut.dispatch(Event::Follow {
        account: Some(account.clone()),
    });
    let waits = run_holding(
        &mut sut,
        &mut world,
        Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: hash.clone(),
            preview: None,
        },
        chain_100,
    );
    assert!(waits.contains(&OPEN_TIMEOUT_MS), "bounded: {waits:?}");
    let linked = sut.view().linked.expect("the link");
    assert_eq!(linked.stand, LinkedStand::Failed);
    assert_eq!(
        linked.reason_key.as_deref(),
        Some(review_state::LINK_UNREAD_CHAIN_KEY)
    );
    assert!(linked.retry);

    // Try again — the network answers now.
    run(&mut sut, &mut world, Event::Retry);
    let linked = sut.view().linked.expect("the link");
    assert_eq!(linked.stand, LinkedStand::Found, "{linked:?}");
    assert!(linked.reason_key.is_none() && !linked.retry);
}

/// r13-3: a link read in time never says a reason.
#[test]
fn a_link_read_in_time_says_no_reason() {
    let account = account();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(stored_proposal(&account, &tx));
    let mut sut = Sut::new();
    let ev = start(&world);
    run(&mut sut, &mut world, ev);
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
    );
    run(
        &mut sut,
        &mut world,
        Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: hash,
            preview: None,
        },
    );
    let linked = sut.view().linked.expect("the link");
    assert_eq!(linked.stand, LinkedStand::Found);
    assert!(linked.reason_key.is_none() && !linked.retry);
}

/// Follow-ups after Round 13 (§18, item 1): a link whose multisig service
/// did not answer (`notFound`) or whose network did not (`unreadable`) is a
/// read that failed, not an answer — it names the read and offers Try again
/// (`retry`), like `failed`; Try again, the read back, opens it. Only
/// `notInMailbox` (read, and not there) is an answer, and offers none.
#[test]
fn a_link_whose_service_or_network_did_not_answer_offers_try_again() {
    let account = account();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    for (down, stand, key) in [
        (
            "service",
            LinkedStand::NotFound,
            review_state::LINK_UNREAD_SERVICE_KEY,
        ),
        (
            "network",
            LinkedStand::Unreadable,
            review_state::LINK_UNREAD_CHAIN_KEY,
        ),
    ] {
        let mut world = World {
            locals: vec![fixture(0)],
            ..World::default()
        };
        world.mailbox.push(stored_proposal(&account, &tx));
        let mut sut = Sut::new();
        let ev = start(&world);
        run(&mut sut, &mut world, ev);
        if down == "service" {
            world.service_down = true;
        } else {
            world.silent.insert(100);
        }
        run(
            &mut sut,
            &mut world,
            Event::Follow {
                account: Some(account.clone()),
            },
        );
        run(
            &mut sut,
            &mut world,
            Event::OpenLinked {
                chain_id: 100,
                safe_tx_hash: hash.clone(),
                preview: None,
            },
        );
        let linked = sut.view().linked.expect("the link");
        assert_eq!(linked.stand, stand, "{down}: {linked:?}");
        assert_eq!(linked.reason_key.as_deref(), Some(key), "{down}");
        assert!(linked.retry, "{down}: a failed read offers Try again");

        // Try again — the read answers now.
        world.service_down = false;
        world.silent.clear();
        run(&mut sut, &mut world, Event::Retry);
        let linked = sut.view().linked.expect("the link");
        assert_eq!(linked.stand, LinkedStand::Found, "{down}: {linked:?}");
        assert!(linked.reason_key.is_none() && !linked.retry, "{down}");
    }
}

/// §18 item 1: a link read and not in the mailbox is an answer — said as
/// that, no Try again (it is read again on every tick).
#[test]
fn a_link_not_in_the_mailbox_offers_no_try_again() {
    let (mut world, mut sut, _account) = setup();
    run(
        &mut sut,
        &mut world,
        Event::OpenLinked {
            chain_id: 100,
            safe_tx_hash: format!("0x{}", "cd".repeat(32)),
            preview: None,
        },
    );
    let linked = sut.view().linked.expect("the link");
    assert_eq!(linked.stand, LinkedStand::NotInMailbox, "{linked:?}");
    assert!(linked.reason_key.is_none() && !linked.retry);
}

// -- r13-7: a new multisig's first history read ---------------------------------

const RAN_TX: &str = "0xefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefefef";

/// A device that has just added the 2-of-3, whose transaction #0 already ran
/// (on another member's device): the mailbox has it, the chain's number is
/// 1, its `ExecutionSuccess` is in the logs. Started, not yet followed.
fn executed_elsewhere() -> (World, Sut, MultisigAccount) {
    let account = account();
    let tx = multisig::cancel_tx(&account.address, "0x0").unwrap();
    let hash = multisig::safe_tx_hash(100, &account.address, &tx).unwrap();
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    world.mailbox.push(stored_proposal(&account, &tx));
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let mut after =
        world::DeployedSafe::clean(&owners.iter().map(String::as_str).collect::<Vec<_>>(), 2);
    after.nonce = 1;
    world.deploy(100, &account.address, after);
    world.receipts.insert(
        RAN_TX.to_owned(),
        json!({"logs": [{"address": account.address, "topics": [
            vela_core::app::multisig_proposals::EXECUTION_SUCCESS, hash
        ], "data": "0x"}]}),
    );
    let mut sut = Sut::new();
    let ev = start(&world);
    run(&mut sut, &mut world, ev);
    (world, sut, account)
}

fn get_logs(op: &Op) -> bool {
    matches!(op, Op::Rpc { method, .. } if method == "eth_getLogs")
}

fn ran_row_handed(world: &World) -> bool {
    world.notices.iter().any(|n| {
        matches!(n, vela_core::app::multisig_shell::Notice::ActivityRows { rows }
            if rows.iter().any(|r| r.to_string().to_ascii_lowercase().contains(&RAN_TX[2..])))
    })
}

/// r13-7: while a new multisig's history is first read, it is `reading`
/// (the home says so — never "No activity yet", never an error); read, the
/// executed transaction is handed to Activity and the history is `read`.
#[test]
fn a_new_multisigs_history_is_reading_until_its_first_read_ends() {
    use vela_core::app::multisig_proposals::HistoryStand;
    let (mut world, mut sut, account) = executed_elsewhere();
    run_holding(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account.clone()),
        },
        get_logs,
    );
    let view = sut.view();
    assert_eq!(view.history, HistoryStand::Reading);
    assert!(
        view.history_unreadable.is_empty(),
        "never an error while read"
    );
    settle_reads_first(&mut sut, &mut world, 20_000);
    let view = sut.view();
    assert_eq!(view.history, HistoryStand::Read);
    assert!(ran_row_handed(&world), "the executed send is in Activity");
}

/// r13-7: a node that does not answer one window of the first read is asked
/// again before the history is said unread (a freshly booted pool, the side
/// panel's).
#[test]
fn a_window_unanswered_once_is_asked_again_before_the_history_is_unread() {
    use vela_core::app::multisig_proposals::HistoryStand;
    use vela_core::app::multisig_shell::MultisigShellResult as Res;
    let (mut world, mut sut, account) = executed_elsewhere();
    let mut missed = 0;
    world.rpc_hooks.push(Box::new(move |_, method, _| {
        (method == "eth_getLogs" && missed < 1).then(|| {
            missed += 1;
            Res::RpcUnanswered {
                detail: "timed out".to_owned(),
            }
        })
    }));
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account),
        },
    );
    let view = sut.view();
    assert_eq!(view.history, HistoryStand::Read);
    assert!(view.history_unreadable.is_empty());
    assert!(ran_row_handed(&world));
}

/// r13-7: a node's "limited to a 10,000 range" is a 10 000-block window (the
/// pool's one reading), never 10 blocks — which ran a new multisig's first
/// read out of windows.
#[test]
fn a_range_limit_written_with_a_comma_narrows_to_its_width() {
    use vela_core::app::multisig_proposals::HistoryStand;
    use vela_core::app::multisig_shell::MultisigShellResult as Res;
    let (mut world, mut sut, account) = executed_elsewhere();
    // Head far past the proposal's day; a window wider than 10 000 blocks
    // is refused.
    world.rpc_hooks.push(Box::new(|_, method, params| {
        if method == "eth_getBlockByNumber" && params[0] == "latest" {
            return Some(Res::RpcResult {
                result: json!({"number": "0x7a120", "timestamp": "0x7fffffff"}),
            });
        }
        if method != "eth_getLogs" {
            return None;
        }
        let q = |k: &str| {
            u64::from_str_radix(params[0][k].as_str().unwrap().trim_start_matches("0x"), 16)
                .unwrap()
        };
        (q("toBlock") - q("fromBlock") + 1 > 10_000).then(|| Res::RpcError {
            code: Some(-32_000),
            message: "eth_getLogs is limited to a 10,000 range".to_owned(),
            data: None,
        })
    }));
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account),
        },
    );
    let view = sut.view();
    assert_eq!(
        view.history,
        HistoryStand::Read,
        "{:?}",
        view.history_unreadable
    );
}

/// r13-7: a first read that cannot be done is `unread` (said where, with
/// Retry — not "No activity yet"); Retry once the network answers reads it.
#[test]
fn an_unread_first_history_is_read_on_retry() {
    use vela_core::app::multisig_proposals::HistoryStand;
    use vela_core::app::multisig_shell::MultisigShellResult as Res;
    let (mut world, mut sut, account) = executed_elsewhere();
    world.rpc_hooks.push(Box::new(|_, method, _| {
        (method == "eth_getLogs").then(|| Res::RpcUnanswered {
            detail: "timed out".to_owned(),
        })
    }));
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(account),
        },
    );
    let view = sut.view();
    assert_eq!(view.history, HistoryStand::Unread);
    assert_eq!(view.history_unreadable, vec![100]);
    world.rpc_hooks.clear();
    run(&mut sut, &mut world, Event::Retry);
    let view = sut.view();
    assert_eq!(view.history, HistoryStand::Read);
    assert!(ran_row_handed(&world));
}
