//! Rules of the display currency, one test per rule.
//!
//! The machine's whole job is refusing bad pairings: a stored code must never
//! render at the rate-1 fallback, a seed must never persist without a real
//! rate, and a user's explicit pick must never lose to an async seed.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::display_currency::{
    CurrencyOperation as Op, CurrencyShellResult as Res, DisplayCurrency, Event,
};

type Sut = DomainDriver<DisplayCurrency>;

fn rate(code: &str, rate: Option<f64>) -> Res {
    Res::RateResolved {
        code: code.to_owned(),
        rate,
    }
}

fn stored(code: Option<&str>) -> Res {
    Res::StoredCode {
        code: code.map(str::to_owned),
    }
}

fn device(code: Option<&str>) -> Res {
    Res::DeviceCurrency {
        code: code.map(str::to_owned),
    }
}

// ---------------------------------------------------------------------------
// The atomic pair
// ---------------------------------------------------------------------------

/// FR-005 — before anything resolves, the view is the USD/1 placeholder, not
/// a half-loaded pairing.
#[test]
fn initial_view_is_usd_at_rate_one_and_uncommitted() {
    let sut = Sut::new();
    let view = sut.view();
    assert_eq!(view.code, "USD");
    assert_eq!(view.rate, Some(1.0));
    assert!(!view.committed);
}

/// FR-005 — the incident this machine exists for: a stored JPY must not render
/// while the rate is still the 1.0 default (¥12 instead of ¥1,860).
#[test]
fn stored_code_never_surfaces_before_its_rate() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("JPY")));

    let view = sut.view();
    assert_eq!(view.code, "USD", "JPY may not show until its rate arrives");
    assert!(!view.committed);
    // … but it is named as the money on its way, so no figure is drawn in
    // the placeholder's dollars meanwhile (the 102 device run).
    assert_eq!(view.pending.as_deref(), Some("JPY"));

    sut.resolve(rate("JPY", Some(155.0)));
    let view = sut.view();
    assert_eq!((view.code.as_str(), view.rate), ("JPY", Some(155.0)));
    assert!(view.committed);
    assert_eq!(view.pending, None);
}

/// Nothing is pending before the preference is read, nor while a first
/// launch prices its region guess — that is not the person's choice yet,
/// and it may still end on USD. A later re-read keeps the committed pair
/// on screen and names nothing pending (no flicker on focus).
#[test]
fn only_a_stored_choice_is_named_pending() {
    let mut sut = Sut::new();
    assert_eq!(sut.view().pending, None);
    sut.dispatch(Event::Refresh);
    assert_eq!(sut.view().pending, None, "still reading the preference");
    sut.resolve(stored(None));
    sut.resolve(device(Some("CNY")));
    let view = sut.view();
    assert!(!view.committed);
    assert_eq!(view.pending, None, "the seed is a guess, not a choice");

    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("CNY")));
    sut.resolve(rate("CNY", Some(7.1)));
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("CNY")));
    let view = sut.view();
    assert!(view.committed, "the pair on screen stays");
    assert_eq!(view.pending, None);
}

/// An unpriceable DISPLAY currency commits as unpriceable — `rate: None`, not
/// rate 1.
///
/// This used to be `rate.unwrap_or(1.0)`: the `getRate` display convenience,
/// baked into the one pair every surface reads. It is a fine convenience for a
/// balance card (print the USD figure rather than a blank) and a catastrophe
/// for the send screen, which divides a fiat-denominated amount by exactly
/// this number — "5000" typed as CNY became 5000 USDT, ~7x, with the confirm
/// slider armed. The shell may still degrade its FORMATTING; the core no
/// longer hands it a multiplier it does not have.
///
/// Mutation proof: restore `rate.unwrap_or(1.0)` and this asserts `Some(1.0)`,
/// and `unpriceable_display_currency_refuses_to_convert` in `app_send.rs` pays
/// out 5000 tokens for a 5000 CNY line.
#[test]
fn unpriceable_display_currency_commits_none_not_one() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("EUR")));
    sut.resolve(rate("EUR", None));

    let view = sut.view();
    assert_eq!(
        (view.code.as_str(), view.rate),
        ("EUR", None),
        "EUR still shows; it is simply not priced"
    );
    assert!(view.committed);
}

/// A source answering 0 (or a negative, or NaN) has priced nothing — the same
/// refusal as an outright failure, because once only a multiplier survives
/// they are indistinguishable from a claim that the rate is 1.
#[test]
fn a_non_positive_answer_is_not_a_rate() {
    for answer in [0.0, -7.17, f64::NAN] {
        let mut sut = Sut::new();
        sut.dispatch(Event::Refresh);
        sut.resolve(stored(Some("CNY")));
        sut.resolve(rate("CNY", Some(answer)));
        assert_eq!(sut.view().rate, None, "answer {answer} is not a rate");
    }
}

// ---------------------------------------------------------------------------
// The first-launch seed
// ---------------------------------------------------------------------------

/// FR-006 — the happy seed: region VND, rate resolves, choice persisted.
#[test]
fn seed_persists_only_after_a_real_rate() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);

    let ops = sut.resolve(stored(None));
    assert_eq!(ops, vec![Op::ReadDeviceCurrency]);

    let ops = sut.resolve(device(Some("VND")));
    assert_eq!(
        ops,
        vec![Op::ResolveRate {
            code: "VND".to_owned()
        }]
    );

    // Rate resolved — but storage is re-read before persisting (the user may
    // have picked something during the fetch).
    let ops = sut.resolve(rate("VND", Some(26_000.0)));
    assert_eq!(ops, vec![Op::ReadStoredCode]);

    let ops = sut.resolve(stored(None));
    assert_eq!(
        ops,
        vec![Op::WriteStoredCode {
            code: "VND".to_owned()
        }],
        "the seed persists exactly once, after the rate"
    );
    let view = sut.view();
    assert_eq!((view.code.as_str(), view.rate), ("VND", Some(26_000.0)));
}

/// FR-006/FR-007 — unpriceable is NOT rate 1: no rate, no seed, key stays
/// absent (₫78 instead of ₫2,000,000 would be strictly worse than USD).
#[test]
fn unpriceable_seed_stays_usd_and_persists_nothing() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(Some("VND")));

    let ops = sut.resolve(rate("VND", None));
    assert!(ops.is_empty(), "no write, no further ops");
    let view = sut.view();
    assert_eq!((view.code.as_str(), view.rate), ("USD", Some(1.0)));
    assert!(
        view.committed,
        "USD/1 is a real commitment, not the placeholder"
    );
}

/// A USD region, a missing region, and a malformed code all mean "no seed".
#[test]
fn non_candidate_regions_commit_usd() {
    for code in [None, Some("USD"), Some("usd "), Some("EURO"), Some("¥¥¥")] {
        let mut sut = Sut::new();
        sut.dispatch(Event::Refresh);
        sut.resolve(stored(None));
        let ops = sut.resolve(device(code));
        // "usd" upper-cases to USD (not a seed); the rest fail the ISO shape.
        assert!(
            ops.is_empty(),
            "{code:?} must not trigger a rate resolution"
        );
        assert_eq!(sut.view().code, "USD");
    }
}

/// FR-006 — the race the re-read exists for: the user picked EUR while the
/// seed's VND rate was still in flight. Their choice wins; VND is never
/// persisted.
#[test]
fn user_choice_made_during_seed_fetch_wins() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(Some("VND")));
    sut.resolve(rate("VND", Some(26_000.0)));

    // The re-read finds EUR — persisted by the Settings screen meanwhile.
    let ops = sut.resolve(stored(Some("EUR")));
    assert_eq!(
        ops,
        vec![Op::ResolveRate {
            code: "EUR".to_owned()
        }],
        "no WriteStoredCode for the seed candidate"
    );
    sut.resolve(rate("EUR", Some(0.85)));
    assert_eq!(sut.view().code, "EUR");
}

/// The seed runs once per session — a later focus with the key still absent
/// (seed declined) settles on USD without re-probing the device.
#[test]
fn seed_is_single_flight_per_session() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(None)); // no region → USD

    sut.dispatch(Event::Refresh);
    let ops = sut.resolve(stored(None));
    assert!(
        ops.is_empty(),
        "the device is not probed again this session"
    );
    assert_eq!(sut.view().code, "USD");
}

// ---------------------------------------------------------------------------
// The explicit pick
// ---------------------------------------------------------------------------

/// An explicit pick persists immediately and prices the pick.
#[test]
fn user_choice_writes_and_prices() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::UserChose {
        code: "KRW".to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteStoredCode {
                code: "KRW".to_owned()
            },
            Op::ResolveRate {
                code: "KRW".to_owned()
            },
        ]
    );
    sut.resolve(Res::CodeWritten);
    sut.resolve(rate("KRW", Some(1_390.0)));
    let view = sut.view();
    assert_eq!((view.code.as_str(), view.rate), ("KRW", Some(1_390.0)));
}

/// FR-006 — the attempt fence: a seed's rate that arrives AFTER an explicit
/// pick is a stale result and changes nothing.
#[test]
fn stale_seed_rate_after_user_choice_is_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(Some("VND"))); // seed's ResolveRate now outstanding

    // The user picks EUR while VND's rate is still in flight.
    sut.dispatch(Event::UserChose {
        code: "EUR".to_owned(),
    });

    // The seed's answer limps home late — resolve the OLDEST outstanding op
    // (the VND rate request issued before the pick).
    let ops = sut.resolve(rate("VND", Some(26_000.0)));
    assert!(ops.is_empty(), "the stale seed result must be inert");
    assert_ne!(sut.view().code, "VND");
}

/// While the pick's rate is in flight the PREVIOUS pair keeps rendering — no
/// flicker through USD, no EUR-at-JPY-rate frame.
#[test]
fn previous_pair_holds_while_a_new_pick_resolves() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("JPY")));
    sut.resolve(rate("JPY", Some(155.0)));

    sut.dispatch(Event::UserChose {
        code: "EUR".to_owned(),
    });
    let view = sut.view();
    assert_eq!(
        (view.code.as_str(), view.rate),
        ("JPY", Some(155.0)),
        "the old pair renders until the new one commits atomically"
    );
}

// ---------------------------------------------------------------------------
// Focus refresh
// ---------------------------------------------------------------------------

/// A focus refresh while anything is in flight coalesces — it must not
/// cancel a live seed and strand the first launch on USD.
#[test]
fn refresh_coalesces_while_in_flight() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(Some("VND"))); // ResolveRate in flight

    let ops = sut.dispatch(Event::Refresh);
    assert!(ops.is_empty(), "no second pipeline while one is running");

    // The seed still concludes.
    sut.resolve(rate("VND", Some(26_000.0)));
    sut.resolve(stored(None));
    assert_eq!(sut.view().code, "VND");
}

/// A later focus re-reads the preference — a currency changed in Settings on
/// another screen shows up everywhere.
#[test]
fn refresh_picks_up_a_changed_preference() {
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("JPY")));
    sut.resolve(rate("JPY", Some(155.0)));

    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("EUR")));
    sut.resolve(rate("EUR", Some(0.85)));
    assert_eq!(sut.view().code, "EUR");
}

// ---------------------------------------------------------------------------
// The withhold rule (PR 3 notes 9 and 27)
// ---------------------------------------------------------------------------

/// No fiat figure is drawn before the currency commits — so the wait has to
/// end. Every way a launch can go ends in a commit, whatever the sources
/// answer: a figure withheld is a figure delayed, never one lost.
#[test]
fn every_launch_ends_in_a_committed_currency() {
    // A stored choice, priced.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    assert!(!sut.view().committed, "nothing may be drawn yet");
    sut.resolve(stored(Some("CNY")));
    assert!(!sut.view().committed, "the rate is still out");
    sut.resolve(rate("CNY", Some(7.1)));
    assert!(sut.view().committed);

    // A stored choice no source can price: committed with no rate — the
    // figure then formats as dollars, honestly labelled.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(Some("VND")));
    sut.resolve(rate("VND", None));
    let view = sut.view();
    assert!(view.committed);
    assert_eq!((view.code.as_str(), view.rate), ("VND", None));

    // No choice, a region with nothing to seed: dollars.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(None));
    assert!(sut.view().committed);

    // No choice, a region whose currency cannot be priced now: dollars.
    let mut sut = Sut::new();
    sut.dispatch(Event::Refresh);
    sut.resolve(stored(None));
    sut.resolve(device(Some("CNY")));
    assert!(!sut.view().committed, "the seed's rate is still out");
    sut.resolve(rate("CNY", None));
    let view = sut.view();
    assert!(view.committed);
    assert_eq!(view.code, "USD");
}

/// The rule names its surfaces once, here: a shell's list of the places it
/// draws fiat is held to this one, so none is skipped again.
#[test]
fn the_withhold_rule_lists_every_fiat_surface_once() {
    use vela_core::app::display_currency::FIAT_SURFACES;
    let unique: std::collections::BTreeSet<&str> = FIAT_SURFACES.into_iter().collect();
    assert_eq!(unique.len(), FIAT_SURFACES.len(), "a surface twice");
    for surface in [
        "send_form",
        "signing_sheet",
        "balance_detail",
        "token_detail",
    ] {
        assert!(
            unique.contains(surface),
            "{surface}: the surfaces the first pass of the rule missed"
        );
    }
}
