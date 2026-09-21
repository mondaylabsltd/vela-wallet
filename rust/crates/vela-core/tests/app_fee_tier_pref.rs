//! Rules of the stored transaction speed (spec 068), one test per rule.
//!
//! The machine's whole job is refusing to invent a preference: an unreadable
//! or unrecognised key must land on the factory `fast` — today's behaviour for
//! every shell — never on some other tier, and never on the dead `rapid`.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::fee_policy::FeeTier;
use vela_core::app::fee_tier_pref::{
    parse_stored, tier_name, Event, FeeTierPref, FeeTierPrefOperation as Op,
    FeeTierPrefShellResult as Res, FACTORY_DEFAULT, OFFERED,
};

type Sut = DomainDriver<FeeTierPref>;

fn stored(raw: Option<&str>) -> Res {
    Res::StoredTier {
        raw: raw.map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// The factory default
// ---------------------------------------------------------------------------

/// Before anything is read, the view is `fast` — what every shell hard-coded
/// before this machine existed — and it says out loud that nobody chose it.
#[test]
fn initial_view_is_the_factory_default_and_uncommitted() {
    let sut = Sut::new();
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Fast);
    assert_eq!(FACTORY_DEFAULT, FeeTier::Fast);
    assert!(!view.committed);
}

/// A fresh install: the key is absent, so the send screen starts exactly where
/// it started yesterday.
#[test]
fn an_absent_key_is_the_factory_default() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::Refresh);
    assert_eq!(ops, vec![Op::ReadStoredTier]);
    sut.resolve(stored(None));

    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Fast);
    assert!(!view.committed, "nothing was chosen, so nothing is claimed");
}

/// A stored preference is read back and committed.
#[test]
fn a_stored_name_is_read_back() {
    for tier in OFFERED {
        let mut sut = Sut::new();
        sut.dispatch(Event::Refresh);
        sut.resolve(stored(Some(tier_name(tier))));
        let view = sut.view();
        assert_eq!(view.tier, tier, "{} round-trips", tier_name(tier));
        assert!(view.committed);
    }
}

/// A key holding something this build does not offer is NOT a tier. It reads
/// as "never chose" — the factory default — and is not rewritten, because
/// rewriting would let an older build silently erase a newer build's choice.
#[test]
fn an_unknown_name_reads_as_unset_and_is_not_rewritten() {
    for raw in ["rapid", "turbo", "", "FAST", "fast "] {
        let mut sut = Sut::new();
        sut.dispatch(Event::Refresh);
        let ops = sut.resolve(stored(Some(raw)));
        let view = sut.view();
        if raw == "fast " {
            // Surrounding whitespace is not a different preference.
            assert_eq!(view.tier, FeeTier::Fast);
            assert!(view.committed, "{raw:?} is `fast` with a stray space");
            continue;
        }
        assert_eq!(view.tier, FACTORY_DEFAULT, "{raw:?} is not a tier");
        assert!(!view.committed, "{raw:?} must not claim a choice");
        assert!(
            !ops.iter()
                .any(|op| matches!(op, Op::WriteStoredTier { .. })),
            "{raw:?} must not be written back"
        );
    }
}

/// `rapid` is the dead fourth variant: it still exists in `FeeTier`, and this
/// machine must never offer it or accept it from storage (spec 068).
#[test]
fn rapid_is_never_offered() {
    assert_eq!(OFFERED, [FeeTier::Fast, FeeTier::Standard, FeeTier::Slow]);
    assert!(!OFFERED.contains(&FeeTier::Rapid));
    assert_eq!(parse_stored("rapid"), None);
    assert_eq!(Sut::new().view().offered, OFFERED.to_vec());
}

// ---------------------------------------------------------------------------
// Choosing
// ---------------------------------------------------------------------------

/// A pick shows immediately and persists under the wire name the relay reads.
#[test]
fn a_pick_commits_and_persists_the_wire_name() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::UserChose {
        tier: FeeTier::Slow,
    });
    assert_eq!(
        ops,
        vec![Op::WriteStoredTier {
            tier: "slow".to_owned()
        }],
        "the stored token is the one `eth_sendUserOperation` accepts"
    );
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Slow, "the row tapped is the row shown");
    assert!(view.committed);
}

/// A write that never lands does not undo what is on screen — the preference
/// is best effort, exactly as `display_currency`'s is.
#[test]
fn an_acknowledged_write_changes_nothing() {
    let mut sut = Sut::new();
    sut.dispatch(Event::UserChose {
        tier: FeeTier::Standard,
    });
    sut.resolve(Res::TierWritten);
    assert_eq!(sut.view().tier, FeeTier::Standard);
    assert!(sut.view().committed);
}

/// The race this machine's `attempt` exists for: a read that was already in
/// flight when the person picked must not land on top of their pick.
#[test]
fn a_slow_read_never_overwrites_a_fresh_pick() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.dispatch(Event::UserChose {
        tier: FeeTier::Slow,
    });
    // The read from before the pick finally answers with the old value.
    sut.resolve_matching(|op| matches!(op, Op::ReadStoredTier), stored(Some("fast")));

    assert_eq!(
        sut.view().tier,
        FeeTier::Slow,
        "their choice wins over a read that predates it"
    );
}

/// A focus refresh while the first read is in flight is coalesced — it must
/// not cancel the read that is about to commit.
#[test]
fn a_second_refresh_does_not_supersede_the_first() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    let ops = sut.dispatch(Event::Refresh);
    assert!(ops.is_empty(), "the read in flight commits anyway");
    assert_eq!(sut.outstanding(), vec![Op::ReadStoredTier]);

    sut.resolve(stored(Some("standard")));
    assert_eq!(sut.view().tier, FeeTier::Standard);
}
