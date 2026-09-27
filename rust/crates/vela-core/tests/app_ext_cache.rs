//! Rules of the Safari-extension account snapshot + UL attestation TTL, one
//! test per rule (inventory `ext_cache`, invariants ①–⑥).
//!
//! The fake clock is `now_ms` on the `AttestationRead` result — the core never
//! owns time. The App Group file, the network catalog and iOS itself are all
//! behind the operation vocabulary.

#![cfg(feature = "crux")]

mod support;

use std::collections::BTreeSet;

use support::DomainDriver;
use vela_core::app::ext_cache::{
    Event, ExtCache, ExtCacheOperation as Op, ExtCacheShellResult as Res, ExtSnapshot, ExtTheme,
    DEFAULT_EXT_CHAIN_ID, UL_TTL_MS,
};
use vela_core::app::Account;

type Sut = DomainDriver<ExtCache>;

const T0: f64 = 1_754_700_000_000.0;
const SIGN_UL: &str = "https://getvela.app/sign?rid=req-42";

fn acct(name: &str, address: &str) -> Account {
    Account {
        id: format!("id-{address}"),
        name: name.to_owned(),
        address: address.to_owned(),
        // A sensitive-adjacent field the snapshot must NEVER carry.
        public_key_hex: "04deadbeefcafebabe".to_owned(),
        created_at_iso: "2026-08-05T00:00:00.000Z".to_owned(),
        keys: Vec::new(),
        signed_in_with: None,
    }
}

fn ann() -> Account {
    acct("Ann", "0xaaaa000000000000000000000000000000000001")
}

fn bob() -> Account {
    acct("Bob", "0xbbbb000000000000000000000000000000000002")
}

/// Wallet state with `active` as the active account, loading resolved.
fn changed(active: Option<Account>, accounts: Vec<Account>) -> Event {
    Event::AccountsChanged {
        is_loading: false,
        has_wallet: active.is_some(),
        accounts,
        active,
        theme: "auto".to_owned(),
        locale: "en".to_owned(),
    }
}

fn loading() -> Event {
    Event::AccountsChanged {
        is_loading: true,
        has_wallet: false,
        accounts: Vec::new(),
        active: None,
        theme: "auto".to_owned(),
        locale: "en".to_owned(),
    }
}

fn attestation(ts: f64, now_ms: f64) -> Res {
    Res::AttestationRead { ts, now_ms }
}

/// The single `WriteSnapshot` out of `ops`, or panic.
fn written(ops: Vec<Op>) -> ExtSnapshot {
    match ops.as_slice() {
        [Op::WriteSnapshot { snapshot }] => snapshot.clone(),
        other => panic!("expected exactly one WriteSnapshot, got {other:?}"),
    }
}

/// Log Ann in and complete one write (never-attested), leaving the machine
/// idle. Returns the written snapshot.
fn logged_in(sut: &mut Sut) -> ExtSnapshot {
    let ops = sut.dispatch(changed(Some(ann()), vec![ann(), bob()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(0.0, T0)));
    assert!(sut.resolve(Res::SnapshotWritten).is_empty());
    snapshot
}

// ---------------------------------------------------------------------------
// Invariant ② — the loading gate
// ---------------------------------------------------------------------------

/// `AccountFileWriter.tsx:70` — the boot restore window is neither "logged
/// out" nor "logged in": clearing would permanently delete a logged-in user's
/// cache, so NOTHING is asked of the shell.
#[test]
fn loading_window_neither_writes_nor_clears() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(loading()).is_empty());

    // Even with a wallet visible, loading still holds everything.
    let ops = sut.dispatch(Event::AccountsChanged {
        is_loading: true,
        has_wallet: true,
        accounts: vec![ann()],
        active: Some(ann()),
        theme: "auto".to_owned(),
        locale: "en".to_owned(),
    });
    assert!(ops.is_empty());
}

/// A foreground before any reported state (or during restore) must not touch
/// the file — the model boots into the gate, exactly like the TS latest-ref
/// starting at `isLoading: true`.
#[test]
fn foreground_before_state_resolves_is_a_no_op() {
    let mut sut = Sut::new();
    assert!(sut.dispatch(Event::Foregrounded).is_empty());

    sut.dispatch(loading());
    assert!(sut.dispatch(Event::Foregrounded).is_empty());
}

/// Loading resolving into a logged-in wallet writes the snapshot.
#[test]
fn loading_resolved_logged_in_writes() {
    let mut sut = Sut::new();
    sut.dispatch(loading());
    let ops = sut.dispatch(changed(Some(ann()), vec![ann()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(0.0, T0)));
    assert_eq!(snapshot.address, ann().address);
    assert_eq!(snapshot.name, "Ann");
    assert_eq!(snapshot.updated_at_ms, T0);
}

/// Loading resolving into genuinely-no-wallet clears — the extension shows
/// empty-state.
#[test]
fn loading_resolved_logged_out_clears() {
    let mut sut = Sut::new();
    sut.dispatch(loading());
    let ops = sut.dispatch(changed(None, Vec::new()));
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
    assert!(sut.view().snapshot.is_none());
}

// ---------------------------------------------------------------------------
// Invariant ① — the public projection
// ---------------------------------------------------------------------------

/// The shared container is world-readable on jailbroken devices: each account
/// serializes to EXACTLY `{ name, address }` — the rich `Account`'s
/// `public_key_hex` / `id` / `created_at_iso` never survive the projection.
#[test]
fn snapshot_accounts_carry_exactly_name_and_address() {
    let mut sut = Sut::new();
    let snapshot = logged_in(&mut sut);

    assert_eq!(snapshot.accounts.len(), 2);
    let json = serde_json::to_value(&snapshot).expect("snapshot serializes");
    for entry in json["accounts"].as_array().expect("accounts array") {
        let keys: BTreeSet<&str> = entry
            .as_object()
            .expect("account object")
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(keys, BTreeSet::from(["name", "address"]));
    }

    let raw = serde_json::to_string(&snapshot).expect("snapshot serializes");
    for leak in ["public_key_hex", "deadbeef", "created_at_iso", "id-0x"] {
        assert!(!raw.contains(leak), "sensitive field leaked: {leak}");
    }
}

/// The snapshot's top-level shape is a closed allowlist — a new field must
/// consciously land on this list (and in the security review), never by
/// accident.
#[test]
fn snapshot_top_level_fields_are_the_public_allowlist() {
    let mut sut = Sut::new();
    let snapshot = logged_in(&mut sut);
    let json = serde_json::to_value(&snapshot).expect("snapshot serializes");
    let keys: BTreeSet<&str> = json
        .as_object()
        .expect("snapshot object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(
        keys,
        BTreeSet::from([
            "address",
            "name",
            "accounts",
            "chain_id",
            "updated_at_ms",
            "ul_verified",
            "ul_verified_at_ms",
            "theme",
            "locale",
        ])
    );
}

// ---------------------------------------------------------------------------
// Invariant ⑤ — the stable default chain
// ---------------------------------------------------------------------------

/// `DEFAULT_EXT_CHAIN_ID` — never the volatile dApp-bridge chainId. The event
/// vocabulary cannot even carry a chain id, so this is structural; the written
/// value is pinned here.
#[test]
fn chain_id_is_the_stable_default() {
    let mut sut = Sut::new();
    let snapshot = logged_in(&mut sut);
    assert_eq!(snapshot.chain_id, DEFAULT_EXT_CHAIN_ID);
    assert_eq!(DEFAULT_EXT_CHAIN_ID, 1, "Ethereum, as the service pins it");
}

// ---------------------------------------------------------------------------
// The logged-out gate (component `!hasWallet || !address` + service guard)
// ---------------------------------------------------------------------------

/// `hasWallet` without an active account is "logged out" for the cache.
#[test]
fn wallet_without_active_account_clears() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountsChanged {
        is_loading: false,
        has_wallet: true,
        accounts: vec![ann()],
        active: None,
        theme: "auto".to_owned(),
        locale: "en".to_owned(),
    });
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
}

/// An active account with an EMPTY address is the service's own guard
/// (`app-group-account-sync.ts:169`): clear, don't write a broken snapshot.
#[test]
fn empty_active_address_clears() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(changed(Some(acct("Ann", "")), vec![ann()]));
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
}

// ---------------------------------------------------------------------------
// Invariant ③ — the 14-day TTL, judged in one place
// ---------------------------------------------------------------------------

/// An attestation younger than 14 days is fresh: the UL hand-off may be used.
#[test]
fn attestation_within_ttl_is_verified() {
    let mut sut = Sut::new();
    sut.dispatch(changed(Some(ann()), vec![ann()]));
    let ts = T0 - (UL_TTL_MS - 1.0);
    let snapshot = written(sut.resolve(attestation(ts, T0)));
    assert!(snapshot.ul_verified);
    assert_eq!(snapshot.ul_verified_at_ms, ts);
    assert_eq!(sut.view().ul_verified_at_ms, ts);
}

/// At exactly 14 days the attestation has aged out (`< TTL`, not `<=`) — the
/// extension reverts to the always-safe scheme. The RAW timestamp still rides
/// the snapshot: the extension compares it against its self-heal veto, so
/// expiry must not zero it (ported verbatim).
#[test]
fn attestation_at_ttl_expires_but_raw_timestamp_survives() {
    let mut sut = Sut::new();
    sut.dispatch(changed(Some(ann()), vec![ann()]));
    let ts = T0 - UL_TTL_MS;
    let snapshot = written(sut.resolve(attestation(ts, T0)));
    assert!(!snapshot.ul_verified, "exactly-TTL is expired");
    assert_eq!(
        snapshot.ul_verified_at_ms, ts,
        "raw ts survives the verdict"
    );
}

/// A FUTURE timestamp is not fresh (`now - ts >= 0` guard): a rolled-back
/// clock falls back to the scheme rather than trusting a hand-off that might
/// hijack the dApp tab.
#[test]
fn future_attestation_is_not_verified() {
    let mut sut = Sut::new();
    sut.dispatch(changed(Some(ann()), vec![ann()]));
    let snapshot = written(sut.resolve(attestation(T0 + 1.0, T0)));
    assert!(!snapshot.ul_verified);
    assert_eq!(snapshot.ul_verified_at_ms, T0 + 1.0);
}

/// Never attested (0) and garbage timestamps both normalize to "never" —
/// `getUniversalLinkVerifiedAt`'s finite-and-positive guard.
#[test]
fn absent_or_garbage_attestation_reads_as_never() {
    let mut sut = Sut::new();
    sut.dispatch(changed(Some(ann()), vec![ann()]));
    let snapshot = written(sut.resolve(attestation(0.0, T0)));
    assert!(!snapshot.ul_verified);
    assert_eq!(snapshot.ul_verified_at_ms, 0.0);
    assert!(sut.resolve(Res::SnapshotWritten).is_empty());

    let ops = sut.dispatch(Event::Foregrounded);
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(-5.0, T0)));
    assert!(!snapshot.ul_verified);
    assert_eq!(snapshot.ul_verified_at_ms, 0.0, "negative normalizes to 0");
}

// ---------------------------------------------------------------------------
// Theme / locale projection
// ---------------------------------------------------------------------------

/// `buildAccountCache` theme guard is strict equality: exactly "light" or
/// "dark" pass, anything else — including case variants — is "auto".
#[test]
fn theme_normalizes_by_strict_equality() {
    for (raw, expected) in [
        ("light", ExtTheme::Light),
        ("dark", ExtTheme::Dark),
        ("auto", ExtTheme::Auto),
        ("Dark", ExtTheme::Auto),
        ("LIGHT", ExtTheme::Auto),
        ("", ExtTheme::Auto),
        ("solarized", ExtTheme::Auto),
    ] {
        let mut sut = Sut::new();
        let ops = sut.dispatch(Event::AccountsChanged {
            is_loading: false,
            has_wallet: true,
            accounts: vec![ann()],
            active: Some(ann()),
            theme: raw.to_owned(),
            locale: "zh".to_owned(),
        });
        assert_eq!(ops, vec![Op::ReadAttestation]);
        let snapshot = written(sut.resolve(attestation(0.0, T0)));
        assert_eq!(snapshot.theme, expected, "theme {raw:?}");
        assert_eq!(snapshot.locale, "zh");
    }
}

// ---------------------------------------------------------------------------
// Foreground re-sync (§12.1.6)
// ---------------------------------------------------------------------------

/// A user who installed the extension while already logged in gets the cache
/// on the next foreground — every foreground rewrites from the latest state.
#[test]
fn foreground_rewrites_with_latest_state() {
    let mut sut = Sut::new();
    logged_in(&mut sut);

    let ops = sut.dispatch(Event::Foregrounded);
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(0.0, T0 + 60_000.0)));
    assert_eq!(snapshot.address, ann().address);
    assert_eq!(snapshot.updated_at_ms, T0 + 60_000.0);
}

// ---------------------------------------------------------------------------
// Staleness — attempt supersession
// ---------------------------------------------------------------------------

/// An account switch while a write's attestation read is in flight supersedes
/// it: the stale read must not write the OLD account's snapshot over the new
/// decision.
#[test]
fn account_switch_supersedes_in_flight_write() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(changed(Some(ann()), vec![ann(), bob()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);

    let ops = sut.dispatch(changed(Some(bob()), vec![ann(), bob()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);

    // The OLD read answers first — dropped, no write.
    assert!(sut.resolve(attestation(0.0, T0)).is_empty());

    // The new one writes Bob.
    let snapshot = written(sut.resolve(attestation(0.0, T0 + 10.0)));
    assert_eq!(snapshot.address, bob().address);
    assert_eq!(snapshot.name, "Bob");
}

// ---------------------------------------------------------------------------
// Invariant ⑥ — logout clears, and wins every race
// ---------------------------------------------------------------------------

/// The session machine's hand-off: `SessionEnded` removes the file so the
/// extension stops serving account data the moment the session is over.
#[test]
fn session_ended_clears_the_cache() {
    let mut sut = Sut::new();
    logged_in(&mut sut);

    let ops = sut.dispatch(Event::SessionEnded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
    assert!(sut.view().snapshot.is_none());
    assert!(sut.resolve(Res::SnapshotRemoved).is_empty());

    // Foreground after logout clears again (idempotent), never writes.
    let ops = sut.dispatch(Event::Foregrounded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
}

/// The TS's latent interleaving — a write that started before logout landing
/// AFTER the clear and resurrecting the file — cannot happen here: logout
/// bumps the attempt, so the in-flight read's answer is dropped (documented
/// deviation; invariant ⑥ wins).
#[test]
fn logout_beats_an_in_flight_write() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(changed(Some(ann()), vec![ann()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);

    let ops = sut.dispatch(Event::SessionEnded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);

    // The pre-logout attestation read answers — no write may follow.
    assert!(sut.resolve(attestation(0.0, T0)).is_empty());
    assert!(sut.view().snapshot.is_none());
    assert!(sut.outstanding().iter().all(|op| op == &Op::RemoveSnapshot));
}

/// A logout arriving before any `AccountsChanged` still clears — the explicit
/// signal bypasses the loading gate (an affirmative logout is not the
/// ambiguous boot window).
#[test]
fn session_ended_clears_even_before_state_resolves() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::SessionEnded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);

    // And the machine now knows it is logged out: foreground clears again.
    let ops = sut.dispatch(Event::Foregrounded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
}

// ---------------------------------------------------------------------------
// Invariant ④ — the anchored Universal-Link guard
// ---------------------------------------------------------------------------

/// Exact apex host + `/sign` scope, case-insensitive — every accepted shape.
#[test]
fn sign_universal_links_are_recognized() {
    for url in [
        "https://getvela.app/sign",
        "https://getvela.app/sign?rid=abc",
        "https://getvela.app/sign/",
        "https://getvela.app/sign/extra",
        "https://getvela.app/sign#frag",
        "HTTPS://GETVELA.APP/SIGN?rid=abc",
    ] {
        let mut sut = Sut::new();
        let ops = sut.dispatch(Event::UniversalLinkOpened {
            url: url.to_owned(),
            now_ms: T0,
        });
        assert!(
            ops.iter()
                .any(|op| matches!(op, Op::PersistAttestation { .. })),
            "{url} must attest"
        );
    }
}

/// Spoofs and unrelated launches change NOTHING — no attestation, no sign
/// request. `evil-getvela.app` / `getvela.app.evil.com` / a path containing
/// the string / the wrong scheme / `/signup` all fail the anchor.
#[test]
fn spoofed_or_unrelated_urls_are_ignored() {
    for url in [
        "https://evil-getvela.app/sign?rid=x",
        "https://getvela.app.evil.com/sign?rid=x",
        "https://evil.com/https://getvela.app/sign",
        "https://getvela.app/signup?rid=x",
        "https://getvela.app/other/sign",
        "http://getvela.app/sign?rid=x",
        "velawallet://sign?rid=x",
        "https://getvela.app",
        "",
    ] {
        let mut sut = Sut::new();
        let ops = sut.dispatch(Event::UniversalLinkOpened {
            url: url.to_owned(),
            now_ms: T0,
        });
        assert!(ops.is_empty(), "{url:?} must be ignored, got {ops:?}");
    }
}

// ---------------------------------------------------------------------------
// The UL flow — sign hand-off + attestation refresh
// ---------------------------------------------------------------------------

/// A real rid drives the extension sign AND refreshes the attestation; the
/// re-sync then re-READS the flag (not trusting the persisted value) and
/// writes a verified snapshot.
#[test]
fn ul_with_rid_signs_attests_and_rewrites() {
    let mut sut = Sut::new();
    logged_in(&mut sut);

    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: SIGN_UL.to_owned(),
        now_ms: T0 + 1_000.0,
    });
    assert_eq!(
        ops,
        vec![
            Op::RequestExtensionSign {
                rid: "req-42".to_owned()
            },
            Op::PersistAttestation { ts: T0 + 1_000.0 },
        ]
    );

    assert!(sut.resolve(Res::SignRequested).is_empty());
    let ops = sut.resolve(Res::AttestationPersisted);
    assert_eq!(ops, vec![Op::ReadAttestation], "persist ack re-syncs");

    let snapshot = written(sut.resolve(attestation(T0 + 1_000.0, T0 + 1_100.0)));
    assert!(snapshot.ul_verified);
    assert_eq!(snapshot.ul_verified_at_ms, T0 + 1_000.0);
    assert_eq!(snapshot.address, ann().address);
}

/// `ul-selftest` is the attestation probe, not a real sign — it refreshes the
/// TTL but never touches the sign bus. Same for a missing or empty rid.
#[test]
fn selftest_and_missing_rid_attest_without_signing() {
    for url in [
        "https://getvela.app/sign?rid=ul-selftest",
        "https://getvela.app/sign",
        "https://getvela.app/sign?rid=",
        "https://getvela.app/sign?other=x",
    ] {
        let mut sut = Sut::new();
        logged_in(&mut sut);
        let ops = sut.dispatch(Event::UniversalLinkOpened {
            url: url.to_owned(),
            now_ms: T0 + 1_000.0,
        });
        assert_eq!(
            ops,
            vec![Op::PersistAttestation { ts: T0 + 1_000.0 }],
            "{url} must attest without a sign request"
        );
    }
}

/// The rid is form-decoded like `URLSearchParams` (`%hh` and `+`) before
/// riding the sign bus.
#[test]
fn rid_is_form_decoded() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: "https://getvela.app/sign?rid=a%20b%2Bc+d".to_owned(),
        now_ms: T0,
    });
    assert_eq!(
        ops.first(),
        Some(&Op::RequestExtensionSign {
            rid: "a b+c d".to_owned()
        })
    );
}

/// A UL while logged out still attests (device-level proof) and still drives
/// the sign bus (the TS handler does both before any login check) — but the
/// re-sync CLEARS, because there is no account to advertise.
#[test]
fn ul_while_logged_out_attests_then_clears() {
    let mut sut = Sut::new();
    sut.dispatch(changed(None, Vec::new()));
    assert!(sut.resolve(Res::SnapshotRemoved).is_empty());

    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: SIGN_UL.to_owned(),
        now_ms: T0,
    });
    assert_eq!(
        ops,
        vec![
            Op::RequestExtensionSign {
                rid: "req-42".to_owned()
            },
            Op::PersistAttestation { ts: T0 },
        ]
    );
    assert!(sut.resolve(Res::SignRequested).is_empty());
    let ops = sut.resolve(Res::AttestationPersisted);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
}

/// A cold-start UL before any wallet state: attest, but neither write nor
/// clear on the ack — the boot window still holds the file (invariant ②).
#[test]
fn cold_start_ul_attests_but_defers_the_file() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: "https://getvela.app/sign?rid=ul-selftest".to_owned(),
        now_ms: T0,
    });
    assert_eq!(ops, vec![Op::PersistAttestation { ts: T0 }]);
    assert!(sut.resolve(Res::AttestationPersisted).is_empty());
}

/// Wallet state arriving while the attestation persist is in flight defers to
/// the attested re-sync: one deterministic write with the LATEST inputs and
/// the fresh attestation (the TS pair of racing writes, collapsed).
#[test]
fn accounts_changed_during_attesting_rides_the_resync() {
    let mut sut = Sut::new();
    logged_in(&mut sut);

    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: "https://getvela.app/sign?rid=ul-selftest".to_owned(),
        now_ms: T0 + 1_000.0,
    });
    assert_eq!(ops, vec![Op::PersistAttestation { ts: T0 + 1_000.0 }]);

    // Switch to Bob mid-flight — no separate read is started.
    assert!(sut
        .dispatch(changed(Some(bob()), vec![ann(), bob()]))
        .is_empty());

    let ops = sut.resolve(Res::AttestationPersisted);
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(T0 + 1_000.0, T0 + 1_200.0)));
    assert_eq!(snapshot.address, bob().address, "latest inputs win");
    assert!(snapshot.ul_verified, "fresh attestation rides along");
}

/// A UL opening while a plain write's attestation read is in flight
/// supersedes it — the final file reflects the refreshed attestation, not the
/// pre-refresh read.
#[test]
fn ul_supersedes_an_in_flight_read() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(changed(Some(ann()), vec![ann()]));
    assert_eq!(ops, vec![Op::ReadAttestation]);

    let ops = sut.dispatch(Event::UniversalLinkOpened {
        url: "https://getvela.app/sign?rid=ul-selftest".to_owned(),
        now_ms: T0 + 500.0,
    });
    assert_eq!(ops, vec![Op::PersistAttestation { ts: T0 + 500.0 }]);

    // The superseded read answers with the STALE (never-attested) value —
    // dropped.
    assert!(sut.resolve(attestation(0.0, T0 + 600.0)).is_empty());

    let ops = sut.resolve(Res::AttestationPersisted);
    assert_eq!(ops, vec![Op::ReadAttestation]);
    let snapshot = written(sut.resolve(attestation(T0 + 500.0, T0 + 700.0)));
    assert!(snapshot.ul_verified);
}

// ---------------------------------------------------------------------------
// Ack hygiene
// ---------------------------------------------------------------------------

/// Best-effort acks are inert — a write/remove/sign acknowledgment never
/// changes state or issues follow-ups.
#[test]
fn acks_are_inert() {
    let mut sut = Sut::new();
    let before = logged_in(&mut sut); // resolves SnapshotWritten internally

    let ops = sut.dispatch(Event::SessionEnded);
    assert_eq!(ops, vec![Op::RemoveSnapshot]);
    assert!(sut.resolve(Res::SnapshotRemoved).is_empty());

    // And the view still reflects decisions, not acks.
    assert!(sut.view().snapshot.is_none());
    assert_eq!(before.address, ann().address);
}
