//! Round 15 (r15-11, r15-12): the proposals machine's list reads — what is
//! said of a multisig while its list is not read yet, and which read owns
//! the list when a newer one is asked while another is still out.
//!
//! - r15-11a: a cold load onto a link flashed "isn't shown here" — the
//!   machine said nothing was loading for a multisig not read yet.
//! - r15-11b: rapid links all ended at "Couldn't reach the multisig
//!   service" — a kept list (`mailbox: false`, "not read yet") was said as
//!   the service's failure, and the newly followed multisig's read was
//!   dropped behind the previous one's.
//! - r15-12a: Try again right after an outage kept "couldn't check" for
//!   40 s+ — the press joined (was dropped behind) a read hung in the outage.

#![cfg(all(feature = "crux", feature = "dev-fixtures"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod support;
#[path = "multisig_world/mod.rs"]
mod world;

use support::DomainDriver;
use vela_core::app::multisig_proposals::{Event, MultisigProposals};
use vela_core::app::multisig_review_state::{self as review_state, MISSING_OPENING_KEY};
use vela_core::app::multisig_shell::MultisigAccount;
use world::{fixture, genesis, settle_reads_first, World, SERVICE};

type Sut = DomainDriver<MultisigProposals>;

fn run(sut: &mut Sut, world: &mut World, event: Event) {
    sut.dispatch(event);
    let answered = settle_reads_first(sut, world, 20_000);
    assert!(answered < 20_000, "the machine never settled");
}

/// Two 2-of-3 multisigs of the fixture accounts on 100, both followed once
/// (their lists kept, as an earlier page leaves them), the second in front.
fn two_multisigs() -> (World, Sut, MultisigAccount, MultisigAccount) {
    let owners: Vec<String> = (0..3).map(|i| fixture(i).address).collect();
    let refs: Vec<&str> = owners.iter().map(String::as_str).collect();
    let row = |salt: &str| -> MultisigAccount {
        serde_json::from_value(world::stored_row(&genesis(&refs, 2, salt), &[100], true)).unwrap()
    };
    let (a, b) = (row("0x31"), row("0x32"));
    let mut world = World {
        locals: vec![fixture(0)],
        ..World::default()
    };
    let mut sut = Sut::new();
    let accounts = world.locals.clone();
    run(
        &mut sut,
        &mut world,
        Event::Start {
            service_url: SERVICE.to_owned(),
            accounts,
            networks: vec![100],
            known: Vec::new(),
        },
    );
    for account in [&a, &b] {
        run(
            &mut sut,
            &mut world,
            Event::Follow {
                account: Some(account.clone()),
            },
        );
    }
    (world, sut, a, b)
}

fn followed(sut: &Sut) -> String {
    sut.view().followed.unwrap().to_ascii_lowercase()
}

fn mailbox_read(sut: &Sut) -> Option<bool> {
    sut.view()
        .chains
        .iter()
        .find(|c| c.chain_id == 100)
        .map(|c| c.mailbox)
}

/// What a sheet opened on a transaction not in the list says now (the
/// core's `missing_said`, as every shell asks it).
fn missing_key(sut: &Sut) -> String {
    let v = sut.view();
    let unreadable = v.unreadable.contains(&100);
    review_state::missing_said(v.loading, unreadable, mailbox_read(sut)).key
}

/// r15-11: following a multisig again draws its kept list at once, but
/// says nothing of it ("not here", "couldn't reach the service") until a
/// read of it lands: it is `loading` — "Opening…".
#[test]
fn a_multisig_followed_again_is_loading_until_its_own_read_lands() {
    let (mut world, mut sut, a, _) = two_multisigs();
    sut.dispatch(Event::Follow {
        account: Some(a.clone()),
    });
    assert_eq!(followed(&sut), a.address.to_ascii_lowercase());
    assert_eq!(
        mailbox_read(&sut),
        Some(false),
        "its kept list, not read yet"
    );
    assert!(sut.view().loading, "not read yet is loading");
    assert_eq!(missing_key(&sut), MISSING_OPENING_KEY);
    let answered = settle_reads_first(&mut sut, &mut world, 20_000);
    assert!(answered < 20_000);
    assert!(!sut.view().loading);
    assert_eq!(mailbox_read(&sut), Some(true));
}

/// r15-11b: a multisig followed while another's read is still out (a link
/// opened right after another) is read at once — never dropped behind the
/// read out, never said "couldn't reach the service" for a read that was
/// never made; the older read's late answer is not the list's.
#[test]
fn a_newly_followed_multisig_is_read_at_once_whatever_read_is_still_out() {
    let (mut world, mut sut, a, _) = two_multisigs();
    // The second multisig's read goes out — and never comes back.
    sut.dispatch(Event::Tick);
    assert!(!sut.outstanding().is_empty(), "a read is out");
    sut.drop_matching(|_| true);
    // A link to a transaction of the first.
    run(
        &mut sut,
        &mut world,
        Event::Follow {
            account: Some(a.clone()),
        },
    );
    assert_eq!(followed(&sut), a.address.to_ascii_lowercase());
    assert!(!sut.view().loading, "its read landed");
    assert_eq!(mailbox_read(&sut), Some(true), "read now, not a kept list");
    assert_ne!(missing_key(&sut), review_state::MISSING_SERVICE_KEY);
}

/// r15-12a: Try again right after an outage is a read begun at the press —
/// not one started during the outage and still hung — and the sheet is
/// loading until it lands, never the failed read's words again meanwhile.
#[test]
fn try_again_reads_now_never_behind_a_read_hung_in_the_outage() {
    let (mut world, mut sut, _, _) = two_multisigs();
    // The service was down: the list is unread.
    world.service_down = true;
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(mailbox_read(&sut), Some(false));
    assert_eq!(missing_key(&sut), review_state::MISSING_SERVICE_KEY);
    // A tick's read goes out during the outage and hangs.
    sut.dispatch(Event::Tick);
    sut.drop_matching(|_| true);
    // The outage lifts; the person presses Try again.
    world.service_down = false;
    sut.dispatch(Event::Retry);
    assert!(sut.view().loading, "reading again, said as that");
    assert_eq!(missing_key(&sut), MISSING_OPENING_KEY);
    let answered = settle_reads_first(&mut sut, &mut world, 20_000);
    assert!(answered < 20_000);
    assert!(!sut.view().loading);
    assert_eq!(mailbox_read(&sut), Some(true), "read at the press");
}

/// Round 16 (r16-16): a `429` from the multisig service was never said —
/// the home sat silent over its kept list for 45 s, while a refused service
/// was said at 4 s. A rate-limited service is said as busy, with Retry; its
/// background reads back off (they ask nothing until its wait — the
/// service's Retry-After, else a growing step — has passed); the person's
/// Retry asks at once; and an answer that is not a `429` ends it.
#[test]
fn a_rate_limited_service_is_said_busy_backs_off_and_retry_asks_now() {
    use vela_core::app::multisig_presence::WaitingNote;
    let (mut world, mut sut, _, _) = two_multisigs();
    world.service_rate_limited = true;
    run(&mut sut, &mut world, Event::Tick);
    assert_eq!(mailbox_read(&sut), Some(false));
    assert_eq!(
        sut.view().waiting_note,
        Some(WaitingNote::Busy),
        "said busy, never silent"
    );
    assert!(WaitingNote::Busy.retry(), "with Retry");
    // Backing off: a tick inside the service's wait asks the service nothing.
    let asked = world.http_log.len();
    run(&mut sut, &mut world, Event::Tick);
    let service_asks = world.http_log[asked..]
        .iter()
        .filter(|l| l.contains(SERVICE))
        .count();
    assert_eq!(service_asks, 0, "no read while it said to wait");
    assert_eq!(sut.view().waiting_note, Some(WaitingNote::Busy));
    // The limit lifts; the person's Retry reads at once.
    world.service_rate_limited = false;
    run(&mut sut, &mut world, Event::Retry);
    assert_eq!(mailbox_read(&sut), Some(true));
    assert_eq!(sut.view().waiting_note, None);
}
