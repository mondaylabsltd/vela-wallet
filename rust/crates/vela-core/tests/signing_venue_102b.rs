//! Spec 102, the core round after Phase 2 — the gaps the shells hit, one test
//! per rule.
//!
//! 1. Trusting a self-hosted page's own build: stored on that page, and the
//!    next check of it opens ("trusted on this device"); never the official
//!    page.
//! 2. `{{time}}` in "checked {{time}}": one formatter for every shell.
//! 3. / 11. / 17. "Confirm with {key}": the key's own label, or its place
//!    when the label is the wallet's name; carried by the signing plan.
//! 4. / 13. Freshness: "checking" while a stale check re-runs, the background
//!    refresh rule, and a refresh that could not complete keeping a good check.
//! 5. The hand-off card's fee + speed row.
//! 6. The launch URL carries the app's language.
//! 7. The web: page rows disabled with their reason, and its plan.
//! 12. A ceremony's own title.
//! 14. The check asks as a browser asks.
//! 15. A page address is what a keyboard typed, read as meant — or refused.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::fee_policy::FeeView;
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::sign_confirm::{handoff_fee, handoff_fee_json};
use vela_core::app::signing_pages::{
    Event, SigningPages, SigningPagesOperation as Op, SigningPagesShellResult as Res,
};
use vela_core::app::Account;
use vela_core::signing_venue::{
    trusted_versions, venue_choices_on_web, KeyLabel, SigningPage, SigningPlan, SigningVenue,
    VenueBlock, APP_DOMAIN,
};
use vela_core::trusted_signer::integrity::{CheckFailure, NoVersion, Verdict};
use vela_core::trusted_signer::launch::{
    admit, checked_time, keep_or_replace, line_while_checking, refresh_due, target, Admission,
    IntegrityState, CHECK_HEADERS, MAX_CHECK_AGE_MS, REFRESH_AFTER_MS, RETRY_AFTER_MS,
};
use vela_core::trusted_signer::{signer_url, SignerUrlError, DEFAULT_SIGNER_URL};

const OWN: &str = "https://sign.example.com/";
/// A build Vela never published — a self-hoster's own.
const OWN_BUILD: &str = "3f9a1c22aabbccddeeff00112233445566778899aabbccddeeff001122334455";
const DAY: u64 = 24 * 60 * 60 * 1000;

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/spec102/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| unreachable!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| unreachable!("{path}: {e}"))
}

fn read(record: &Value) -> Account {
    serde_json::from_value(record.clone()).unwrap_or_else(|e| unreachable!("reads: {e}"))
}

/// The check of `base` with `trusted`, its index listing `listed`, served
/// `observed`, at `at`.
fn check(base: &str, listed: &[&str], trusted: &[String], observed: &str, at: u64) -> Admission {
    let index: Vec<String> = listed.iter().map(|h| (*h).to_owned()).collect();
    let target = target(base, Some(&index), trusted, &[])
        .unwrap_or_else(|why| unreachable!("a version for {base}: {why:?}"));
    admit(
        &target,
        Some(observed),
        CheckFailure::NotChecked,
        trusted,
        &[],
        false,
        at,
    )
}

// ---------------------------------------------------------------------------
// 1 · Trust this version — per page, never the official one
// ---------------------------------------------------------------------------

type Pages = DomainDriver<SigningPages>;

fn pages(stored: Option<&str>) -> Pages {
    let mut sut = Pages::new();
    assert_eq!(sut.dispatch(Event::Refresh), vec![Op::ReadStored]);
    sut.resolve(Res::Stored {
        pages_json: stored.map(str::to_owned),
        legacy_url: None,
    });
    sut
}

/// The whole loop: a self-hosted build is asked about, the answer is stored
/// on that page, and the next check of it opens and says why.
#[test]
fn a_self_hosters_build_is_trusted_on_its_page_and_then_opens() {
    // The page's index lists only its own build: the check can only ask.
    let asked = check(OWN, &[OWN_BUILD], &[], OWN_BUILD, 1_000);
    assert_eq!(asked.line().state, IntegrityState::AskToTrust);
    assert!(!asked.line().opens);
    assert_eq!(asked.version_to_trust(), Some(OWN_BUILD));

    // "Trust this version": stored on that page.
    let mut sut = pages(Some(
        r#"[{"url":"https://sign.example.com/","name":"Mine"}]"#,
    ));
    let ops = sut.dispatch(Event::VersionTrusted {
        url: "sign.example.com".into(),
        version: OWN_BUILD.to_ascii_uppercase(),
    });
    let mut mine = SigningPage::new(OWN, "Mine");
    mine.trusted = vec![OWN_BUILD.to_owned()];
    assert_eq!(
        ops,
        vec![Op::WritePages {
            pages: vec![mine.clone()],
            remove_legacy_url: false,
        }]
    );
    let view = sut.view();
    assert_eq!(view.pages[1].trusted, [OWN_BUILD]);
    assert!(view.pages[0].trusted.is_empty(), "never the official page");
    assert_eq!(trusted_versions(&view.saved, OWN), [OWN_BUILD]);
    // Asked again: nothing to write.
    assert!(sut
        .dispatch(Event::VersionTrusted {
            url: OWN.into(),
            version: OWN_BUILD.into(),
        })
        .is_empty());

    // The next check of the page, with ITS trusted list: open, "trusted on
    // this device", and launched at the very version that was checked.
    let trusted = trusted_versions(&view.saved, OWN);
    let opened = check(OWN, &[OWN_BUILD], &trusted, OWN_BUILD, 2_000);
    let line = opened.line();
    assert_eq!(line.state, IntegrityState::TrustedHere);
    assert!(line.opens);
    assert_eq!(line.version, &OWN_BUILD[..8]);
    let launch = opened
        .page()
        .and_then(|page| {
            page.url_launch(&json!({}), "velawallet://sign-result", "t", "", 2_000)
                .ok()
        })
        .unwrap_or_default();
    assert!(
        launch.starts_with(&format!("{OWN}b/{OWN_BUILD}/sign.html?ch=url#")),
        "{launch}"
    );

    // Stored, and read back as stored.
    let json = serde_json::to_string(&view.saved).unwrap_or_default();
    assert_eq!(pages(Some(&json)).view().saved, vec![mine]);
}

/// A page that is not saved yet is saved by the answer — the person just
/// decided to trust it.
#[test]
fn trusting_a_page_that_is_not_saved_saves_it() {
    let mut sut = pages(None);
    let ops = sut.dispatch(Event::VersionTrusted {
        url: "http://localhost:8140".into(),
        version: OWN_BUILD.into(),
    });
    let mut local = SigningPage::new("http://localhost:8140/", "");
    local.trusted = vec![OWN_BUILD.to_owned()];
    assert_eq!(
        ops,
        vec![Op::WritePages {
            pages: vec![local],
            remove_legacy_url: false,
        }]
    );
}

/// Never the official page, never something that is not a version, never
/// before the list was read.
#[test]
fn the_official_page_and_non_versions_are_never_trusted() {
    let mut unread = Pages::new();
    assert!(unread
        .dispatch(Event::VersionTrusted {
            url: OWN.into(),
            version: OWN_BUILD.into(),
        })
        .is_empty());

    let mut sut = pages(None);
    for (url, version) in [
        (DEFAULT_SIGNER_URL, OWN_BUILD),
        ("https://SIGN.getvela.app/other/", OWN_BUILD),
        (OWN, "3f9a1c22"),
        (OWN, "not a hash"),
        ("ftp://x", OWN_BUILD),
    ] {
        assert!(
            sut.dispatch(Event::VersionTrusted {
                url: url.into(),
                version: version.into(),
            })
            .is_empty(),
            "{url} {version}"
        );
    }
    assert!(sut.view().saved.is_empty());
    assert!(trusted_versions(
        &[SigningPage::new(DEFAULT_SIGNER_URL, "")],
        DEFAULT_SIGNER_URL
    )
    .is_empty());
}

/// Even handed a "trusted" version for the official page, the rule ignores
/// it: nothing a person was talked into trusting opens in its name.
#[test]
fn trust_handed_to_the_official_page_is_ignored() {
    let talked_into = vec![OWN_BUILD.to_owned()];
    // The index lists only the stranger: no version this wallet knows.
    assert_eq!(
        target(DEFAULT_SIGNER_URL, Some(&talked_into), &talked_into, &[]).err(),
        Some(NoVersion::NothingPublishedThisWalletKnows)
    );
    // Served at the launch version's address: refused, not opened.
    let launch = target(DEFAULT_SIGNER_URL, None, &talked_into, &[])
        .unwrap_or_else(|why| unreachable!("{why:?}"));
    let served = admit(
        &launch,
        Some(OWN_BUILD),
        CheckFailure::NotChecked,
        &talked_into,
        &[],
        false,
        1,
    );
    assert!(served.page().is_none());
    assert_eq!(served.line().state, IntegrityState::Mismatch);
    assert_eq!(served.version_to_trust(), None);
}

// ---------------------------------------------------------------------------
// 2 · "checked {{time}}"
// ---------------------------------------------------------------------------

/// 2026-10-09 14:32:00 UTC.
const AT: u64 = 1_791_556_320_000;

#[test]
fn the_check_time_is_the_clock_today_and_the_date_before() {
    // Each moment is one unbreakable unit (a no-break space wherever the
    // format has a space, a word joiner inside a CJK day period); compared
    // here as written.
    let checked_time = |at, now, offset, date: &str, time: &str, lang: &str| {
        let moment = checked_time(at, now, offset, date, time, lang);
        assert!(!moment.contains(' '), "{moment:?} could break across lines");
        moment.replace('\u{a0}', " ").replace('\u{2060}', "")
    };
    let later = AT + 2 * 60 * 1000;
    assert_eq!(
        checked_time(AT, later, 0, "mdy_slash", "h24", "en"),
        "14:32"
    );
    assert_eq!(
        checked_time(AT, later, 0, "mdy_slash", "h12", "en"),
        "2:32 PM"
    );
    assert_eq!(checked_time(AT, later, 0, "iso", "h12", "ja"), "午後 2:32");
    // In UTC+8 it is 22:32 the same day.
    assert_eq!(checked_time(AT, later, 480, "iso", "h24", "zh"), "22:32");
    // In UTC+10 the check ran at 00:32 on the 10th; read at 00:40 that day.
    assert_eq!(
        checked_time(AT, AT + 8 * 60 * 1000, 600, "iso", "h24", "en"),
        "00:32"
    );
    // Yesterday's check, read today: "14:32" alone would read as a time still
    // to come, so the date says which day.
    let tomorrow = AT + 20 * 60 * 60 * 1000;
    assert_eq!(
        checked_time(AT, tomorrow, 0, "mdy_slash", "h24", "en"),
        "10/09/2026, 14:32"
    );
    assert_eq!(
        checked_time(AT, tomorrow, 0, "dmy_dot", "h12", "de"),
        "09.10.2026, 2:32 PM"
    );
    // `auto` and unknown words read as the defaults.
    assert_eq!(checked_time(AT, later, 0, "auto", "auto", "en"), "14:32");
}

/// The 102 device run: 「检查于 下午」 / 「10:07」 on two lines. The day
/// period and the clock are glued, and so are a date and its time.
#[test]
fn a_checked_time_never_breaks_inside() {
    let later = AT + 2 * 60 * 1000;
    let tomorrow = AT + 20 * 60 * 60 * 1000;
    for (now, language, clock) in [
        (later, "zh", "h12"),
        (later, "ja", "h12"),
        (later, "en", "h12"),
        (tomorrow, "en", "h24"),
        (tomorrow, "zh", "h12"),
    ] {
        let moment = checked_time(AT, now, 0, "ymd_slash", clock, language);
        assert!(!moment.contains(' '), "{language} {clock}: {moment:?}");
    }
    // A CJK day period is glued too: U+00A0 binds the space, not 下|午.
    assert_eq!(
        checked_time(AT, later, 0, "ymd_slash", "h12", "zh"),
        "下\u{2060}午\u{a0}2:32"
    );
}

/// Between two CJK characters a line may break — 「下午」 split after 「下」
/// on the web at some widths. Every such place inside a moment carries a
/// word joiner; a Latin-script moment needs none and gets none.
#[test]
fn a_checked_time_never_breaks_inside_a_cjk_day_period() {
    let later = AT + 2 * 60 * 1000;
    let tomorrow = AT + 20 * 60 * 60 * 1000;
    // A place a line could break: two neighbours, either of them a Han,
    // kana or Hangul character, with no joiner and no no-break space between.
    let cjk = |c: char| matches!(u32::from(c), 0x2E80..=0xA4CF | 0xAC00..=0xD7FF);
    let breakable = |moment: &str| {
        let chars: Vec<char> = moment.chars().collect();
        chars.windows(2).any(|pair| {
            let glue = |c: char| c == '\u{2060}' || c == '\u{a0}';
            !glue(pair[0]) && !glue(pair[1]) && (cjk(pair[0]) || cjk(pair[1]))
        })
    };
    for language in ["zh", "zh-TW", "zh-HK", "ja", "ko"] {
        for (now, clock) in [(later, "h12"), (tomorrow, "h12"), (tomorrow, "h24")] {
            let moment = checked_time(AT, now, 0, "ymd_slash", clock, language);
            assert!(!breakable(&moment), "{language} {clock}: {moment:?}");
            assert!(!moment.contains(' '), "{language} {clock}: {moment:?}");
        }
    }
    assert_eq!(
        checked_time(AT, later, 0, "ymd_slash", "h12", "ja"),
        "午\u{2060}後\u{a0}2:32"
    );
    // An alphabet: exactly as before, no joiner anywhere — "ÖS" and "CH"
    // are words, and nothing breaks inside a word.
    for (now, clock) in [(later, "h12"), (later, "h24"), (tomorrow, "h12")] {
        for language in [
            "en", "de", "fr", "es-MX", "pt-BR", "it", "id", "tr", "vi", "ru",
        ] {
            let moment = checked_time(AT, now, 0, "ymd_slash", clock, language);
            assert!(!moment.contains('\u{2060}'), "{language}: {moment:?}");
        }
    }
    assert_eq!(
        checked_time(AT, later, 0, "ymd_slash", "h12", "en"),
        "2:32\u{a0}PM"
    );
}

// ---------------------------------------------------------------------------
// 3 / 11 / 17 · "Confirm with {key}"
// ---------------------------------------------------------------------------

#[test]
fn the_key_is_named_by_its_own_label_when_it_has_one() {
    // The app-only fixture signs in with its second key, "YubiKey".
    let account = read(&fixture("app-only-account.json"));
    let label = account.key_label();
    assert_eq!(label.name.as_deref(), Some("YubiKey"));
    assert_eq!(label.place_key, "onboarding.create.methodSecurityKeyTitle");
    assert_eq!(account.signing_plan().key_label, label);
    assert_eq!(label.text(|key| format!("<{key}>")), "YubiKey");
}

/// The founding key carries the wallet's name, which the card's "Signing
/// account" row already says: the place it lives is named instead.
#[test]
fn a_key_named_after_the_wallet_is_named_by_its_place() {
    let mut record = fixture("app-only-account.json");
    record["signed_in_with"] = json!({
        "credential_id": "5ec0de01",
        "method": "hybrid",
        "transports": "hybrid"
    });
    let label = read(&record).key_label();
    assert_eq!(label.name, None);
    assert_eq!(label.place_key, "onboarding.create.methodHybridTitle");
    assert_eq!(
        label.text(|key| key.rsplit('.').next().unwrap_or_default().to_owned()),
        "methodHybridTitle"
    );
    // Case and spaces do not make it another name.
    assert_eq!(
        KeyLabel::of(Some("  spending "), "Spending", "platform").name,
        None
    );
    assert_eq!(KeyLabel::of(Some(""), "Spending", "platform").name, None);
    assert_eq!(
        KeyLabel::of(None, "Spending", "security_key").place_key,
        "onboarding.create.methodSecurityKeyTitle"
    );
}

/// A record from before the sign-in key names its first key the same way,
/// its place read from what that key reported.
#[test]
fn a_record_without_a_sign_in_key_names_its_first_key() {
    let account = read(&fixture("before-sign-in-key-app.json"));
    let plan = account.signing_plan();
    assert!(plan.key.is_none());
    assert!(!plan.key_label.place_key.is_empty());

    let mut record = fixture("app-only-account.json");
    record
        .as_object_mut()
        .map(|record| record.remove("signed_in_with"));
    let label = read(&record).key_label();
    assert_eq!(label.name, None, "the first key is named after the wallet");
    // `hybrid,internal`: a platform passkey that also syncs.
    assert_eq!(label.place_key, "onboarding.create.methodPlatformTitle");
}

/// The plan's JSON carries it, for the shells that hold JSON.
#[test]
fn the_plan_on_the_wire_names_the_key() {
    let record = fixture("app-only-account.json").to_string();
    let plan: Value = vela_core::app::signing_plan_json(&record)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    assert_eq!(
        plan["key_label"],
        json!({"name": "YubiKey", "place_key": "onboarding.create.methodSecurityKeyTitle",
               "label_key": "componentsUi.signing.confirmWithLabel"})
    );
}

/// The key is a row — "Confirm with | YubiKey" — not a sentence a locale has
/// to inflect a place's title into (integration polish b). A label written
/// before the row existed reads as "Confirm with".
#[test]
fn the_key_is_a_row_with_its_own_label() {
    assert_eq!(
        KeyLabel::of(Some("YubiKey"), "Savings", "security_key").label_key,
        "componentsUi.signing.confirmWithLabel"
    );
    let old: KeyLabel =
        serde_json::from_value(json!({"place_key": "onboarding.create.methodPlatformTitle"}))
            .unwrap_or_default();
    assert_eq!(old.label_key, "componentsUi.signing.confirmWithLabel");
}

// ---------------------------------------------------------------------------
// 4 / 13 · Freshness: checking, and the background refresh
// ---------------------------------------------------------------------------

fn official_check(at: u64) -> Admission {
    let launch =
        target(DEFAULT_SIGNER_URL, None, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
    let version = launch.version().to_owned();
    admit(
        &launch,
        Some(&version),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        at,
    )
}

fn unreachable_check(at: u64) -> Admission {
    let launch =
        target(DEFAULT_SIGNER_URL, None, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
    admit(
        &launch,
        None,
        CheckFailure::Unreachable,
        &[],
        &[],
        false,
        at,
    )
}

/// A day vouches; past it the page is refused until checked again, and while
/// that runs the line is "checking" — with Open off.
#[test]
fn a_stale_check_reads_checking_while_it_runs_again() {
    let checked = official_check(1_000);
    assert!(checked.is_fresh(1_000 + MAX_CHECK_AGE_MS));
    assert!(!checked.is_fresh(1_001 + MAX_CHECK_AGE_MS));
    assert!(!unreachable_check(1).is_fresh(1));

    // Re-running a fresh check (a background refresh): the good line stays.
    let during = line_while_checking(Some(&checked), 2_000);
    assert_eq!(during.state, IntegrityState::Matches);
    assert!(during.opens);
    // Re-running a stale one (at Open): "checking", Open off.
    let during = line_while_checking(Some(&checked), 2_000 + MAX_CHECK_AGE_MS);
    assert_eq!(during.state, IntegrityState::Checking);
    assert!(!during.opens);
    assert_eq!(during.key, "componentsUi.signing.integrity.checking");
    // Nothing before it, or a refusal: "checking".
    assert_eq!(line_while_checking(None, 1).state, IntegrityState::Checking);
    assert_eq!(
        line_while_checking(Some(&unreachable_check(1)), 1).state,
        IntegrityState::Checking
    );
}

/// The background refresh: a page in use is checked again once its check is
/// half a day old (or it has none), and attempts that cannot reach it are
/// spaced out.
#[test]
fn checks_are_refreshed_in_the_background_before_they_expire() {
    assert_eq!(REFRESH_AFTER_MS, MAX_CHECK_AGE_MS / 2);
    assert!(refresh_due(None, None, 5));
    assert!(
        !refresh_due(Some(0), None, REFRESH_AFTER_MS),
        "half a day old"
    );
    assert!(refresh_due(Some(0), None, REFRESH_AFTER_MS + 1));
    // An attempt just ran (and could not complete): wait.
    assert!(!refresh_due(None, Some(100), 100 + RETRY_AFTER_MS - 1));
    assert!(refresh_due(None, Some(100), 100 + RETRY_AFTER_MS));
    let checked = official_check(0);
    assert!(!checked.refresh_due(None, DAY / 4));
    assert!(checked.refresh_due(None, DAY * 3 / 4));
    assert!(checked
        .page()
        .is_some_and(|page| page.refresh_due(None, DAY * 3 / 4)));
    // Always before the check stops vouching.
    const { assert!(REFRESH_AFTER_MS + RETRY_AFTER_MS < MAX_CHECK_AGE_MS) };
}

/// A refresh that could not complete keeps a check that still vouches; one
/// that completed — a mismatch included — replaces it.
#[test]
fn a_refresh_that_could_not_complete_keeps_the_good_check() {
    let now = DAY / 2 + 1;
    let kept = keep_or_replace(Some(official_check(0)), unreachable_check(now), now);
    assert!(
        kept.is_fresh(now),
        "offline for a moment: the morning's check stands"
    );

    // Too old to stand: the failure is the news.
    let late = MAX_CHECK_AGE_MS + 1;
    let replaced = keep_or_replace(Some(official_check(0)), unreachable_check(late), late);
    assert_eq!(replaced.line().state, IntegrityState::CouldNotCheck);

    // A completed refresh that refuses replaces a fresh admission.
    let launch =
        target(DEFAULT_SIGNER_URL, None, &[], &[]).unwrap_or_else(|why| unreachable!("{why:?}"));
    let swapped = admit(
        &launch,
        Some(OWN_BUILD),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        now,
    );
    let replaced = keep_or_replace(Some(official_check(0)), swapped, now);
    assert_eq!(replaced.line().state, IntegrityState::Mismatch);
    assert!(matches!(
        replaced,
        Admission::Refused {
            verdict: Verdict::Refused { .. },
            ..
        }
    ));
}

// ---------------------------------------------------------------------------
// 5 · The hand-off card's fee + speed row
// ---------------------------------------------------------------------------

fn fee_view(busy: bool, ready: bool, tier: &str) -> FeeView {
    serde_json::from_value(json!({
        "busy": busy,
        "failed": null,
        "fee": {
            "chain_id": 8453, "total_wei": "21000000000000", "max_fee_per_gas": "1000000",
            "network_fee_per_gas": "1000000", "relayer_fee_per_gas": "0",
            "bundler_gas_price": "1000000", "in_band_gas_basis": "0",
            "effective_gas_price": null, "max_gas_price": null, "total_gas": "21000",
            "deployed": true, "tier": tier, "quoted": true,
            "fee_asset": {"type": "erc20", "token": "0xusdc", "decimals": 6, "amount": "20000", "symbol": "USDC"},
            "fee_recipient": null
        },
        "stale": false,
        "fee_token": "0xusdc",
        "options": [],
        "confirm_fee_ready": ready
    }))
    .unwrap_or_else(|e| unreachable!("fee view: {e}"))
}

fn speed_view(tier: &str, single: bool) -> FeeSpeedView {
    serde_json::from_value(json!({
        "tier": tier, "preferred": "standard", "previews": [], "open": false,
        "picked": false, "free": false, "free_note": false, "single": single,
        "gas_price_line": false, "options": []
    }))
    .unwrap_or_else(|e| unreachable!("speed view: {e}"))
}

#[test]
fn the_hand_off_card_keeps_the_fee_and_speed_that_were_chosen() {
    let row = handoff_fee(
        Some(&fee_view(false, true, "fast")),
        Some(&speed_view("fast", false)),
    )
    .unwrap_or_else(|| unreachable!("a settled fee has a row"));
    assert_eq!(row.fee.total_wei, "21000000000000");
    assert_eq!(row.tier_key.as_deref(), Some("send.gasTier.fast"));

    // One speed on this network, or no speed control: nothing to restate.
    let single = handoff_fee(
        Some(&fee_view(false, true, "standard")),
        Some(&speed_view("standard", true)),
    );
    assert_eq!(single.map(|row| row.tier), Some(None));
    let bare = handoff_fee(Some(&fee_view(false, true, "standard")), None);
    assert_eq!(bare.and_then(|row| row.tier_key), None);

    // No row for a fee nobody chose yet: measuring, another speed's figure,
    // not ready — and none for a message.
    assert!(handoff_fee(Some(&fee_view(true, true, "fast")), None).is_none());
    assert!(handoff_fee(
        Some(&fee_view(false, true, "standard")),
        Some(&speed_view("fast", false))
    )
    .is_none());
    assert!(handoff_fee(Some(&fee_view(false, false, "fast")), None).is_none());
    assert!(handoff_fee(None, None).is_none());

    // The JSON door the phones and the web use.
    let fee = serde_json::to_string(&fee_view(false, true, "slow")).unwrap_or_default();
    let speed = serde_json::to_string(&speed_view("slow", false)).unwrap_or_default();
    let wire: Value = handoff_fee_json(Some(&fee), Some(&speed))
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    assert_eq!(wire["tier"], "slow");
    assert_eq!(wire["tier_key"], "send.gasTier.slow");
    assert_eq!(handoff_fee_json(Some("{"), None), None);
}

// ---------------------------------------------------------------------------
// 6 · The launch URL carries the app's language
// ---------------------------------------------------------------------------

#[test]
fn the_page_is_opened_in_the_apps_language() {
    let checked = official_check(1_000);
    let page = checked.page().unwrap_or_else(|| unreachable!("admitted"));
    let url = page.target().url().to_owned();
    let launched = |lang: &str| {
        page.url_launch(&json!({}), "velawallet://sign-result", "t", lang, 1_000)
            .unwrap_or_default()
    };
    assert!(launched("zh-HK").starts_with(&format!("{url}?ch=url&lang=zh-HK#")));
    assert!(launched("pt-BR").contains("&lang=pt-BR#"));
    // None, or not a language tag: the page follows the browser.
    for lang in ["", "  ", "zh&evil=1", "../x", "1zh", "zh HK"] {
        assert!(
            launched(lang).starts_with(&format!("{url}?ch=url#")),
            "{lang:?}"
        );
    }
    assert_eq!(
        page.ws_launch(51_234, "t", "ja", 1_000).unwrap_or_default(),
        format!("{url}?ch=ws&lang=ja#p=51234&t=t")
    );
}

// ---------------------------------------------------------------------------
// 7 · The web opens no page
// ---------------------------------------------------------------------------

#[test]
fn the_web_shows_page_rows_disabled_with_its_reason() {
    let saved = [SigningPage::new(OWN, "Mine")];
    let rows = venue_choices_on_web(APP_DOMAIN, &SigningVenue::official(), &saved);
    let blocked: Vec<Option<VenueBlock>> = rows.iter().map(|row| row.blocked.clone()).collect();
    assert_eq!(
        blocked,
        [
            None,
            Some(VenueBlock::NotOnWeb),
            // R1's reason wins where both hold.
            Some(VenueBlock::PageOnOtherDomain {
                page_domain: "sign.example.com".to_owned(),
                domain: APP_DOMAIN.to_owned(),
            }),
        ]
    );
    assert_eq!(VenueBlock::NotOnWeb.key(), "settings.venue.blockedWeb");
    assert_eq!(
        serde_json::to_value(VenueBlock::NotOnWeb).unwrap_or_default(),
        json!({"type": "not_on_web"})
    );
}

/// A `getvela.app` account whose venue is a page signs in Vela on the web; a
/// custom-domain account cannot sign there, and its plan says why.
#[test]
fn the_webs_plan() {
    let official = read(&fixture("official-page-account.json")).signing_plan();
    assert_eq!(official.venue, SigningVenue::official());
    let web = official.clone().on_web();
    assert_eq!(web.venue, SigningVenue::InVela);
    assert_eq!(web.blocked, None);
    assert_eq!(web.key, official.key, "the same key, signed natively");

    let custom = read(&fixture("custom-origin-account.json")).signing_plan();
    assert_eq!(custom.blocked, None);
    let web = custom.clone().on_web();
    assert_eq!(web.blocked, Some(VenueBlock::NotOnWeb));
    assert_eq!(web.venue, custom.venue);

    // Through the JSON door the web uses.
    let record = fixture("custom-origin-account.json").to_string();
    let plan: SigningPlan = vela_core::app::signing_plan_on_web_json(&record)
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_else(|| unreachable!("the web's plan"));
    assert_eq!(plan.blocked, Some(VenueBlock::NotOnWeb));
}

// ---------------------------------------------------------------------------
// 12 · A ceremony's own title
// ---------------------------------------------------------------------------

#[test]
fn a_ceremony_on_its_page_has_its_own_title() {
    use vela_core::trusted_signer::ceremony::Ceremony;
    let title =
        |op: Value| Ceremony::from_json(&op.to_string()).map(|ceremony| ceremony.title_key());
    assert_eq!(
        title(json!({"type": "register_passkey", "name": "Mine", "page": OWN})),
        Some("componentsUi.signing.ceremonyCreate")
    );
    assert_eq!(
        title(json!({"type": "authenticate_passkey", "page": OWN})),
        Some("componentsUi.signing.ceremonySignIn")
    );
    for purpose in ["recover_first", "recover_second"] {
        assert_eq!(
            title(json!({"type": "sign_proof", "credential_id": "ab", "purpose": purpose})),
            Some("componentsUi.signing.ceremonySignIn")
        );
    }
    assert_eq!(
        title(json!({"type": "sign_proof", "credential_id": "ab", "purpose": "verify"})),
        Some("componentsUi.signing.ceremonyConfirm")
    );
    assert_eq!(
        title(json!({"type": "sign_member_proof", "credential_id": "ab",
                     "public_key_hex": "04", "group_public_key_hex": "04"})),
        Some("componentsUi.signing.ceremonyConfirm")
    );
    assert_eq!(title(json!({"type": "read_stored"})), None);
}

/// A ceremony card names its key as the page does (integration polish c):
/// "New key on | Phone or tablet" while a key is made, "Confirm with | USB
/// security key" when one signs in or proves — by place, never by a name.
#[test]
fn a_ceremony_names_its_key_as_the_page_does() {
    use vela_core::trusted_signer::ceremony::Ceremony;
    let label = |op: Value| {
        Ceremony::from_json(&op.to_string())
            .map(|ceremony| ceremony.key_label())
            .unwrap_or_else(|| unreachable!("a ceremony"))
    };
    let made = label(
        json!({"type": "register_passkey", "name": "Mine", "page": OWN,
                            "method": "hybrid"}),
    );
    assert_eq!(made.name, None, "a new key carries the wallet's name");
    assert_eq!(made.place_key, "onboarding.create.methodHybridTitle");
    assert_eq!(made.label_key, "componentsUi.signing.newKeyOnLabel");
    for op in [
        json!({"type": "authenticate_passkey", "page": OWN, "method": "security_key"}),
        json!({"type": "sign_proof", "credential_id": "ab", "purpose": "verify",
               "method": "security_key"}),
        json!({"type": "sign_member_proof", "credential_id": "ab", "public_key_hex": "04",
               "group_public_key_hex": "04", "method": "security_key"}),
    ] {
        let used = label(op);
        assert_eq!(used.place_key, "onboarding.create.methodSecurityKeyTitle");
        assert_eq!(used.label_key, "componentsUi.signing.confirmWithLabel");
    }
    // An operation from before the place was carried reads as this device.
    assert_eq!(
        label(json!({"type": "authenticate_passkey"})).place_key,
        "onboarding.create.methodPlatformTitle"
    );
}

/// A refusal's sentence is the key and its values, both from the core
/// (integration polish g): no shell decides which fact fills which name.
#[test]
fn a_venue_refusal_carries_its_values() {
    let page = VenueBlock::PageOnOtherDomain {
        page_domain: "sign.example.com".to_owned(),
        domain: APP_DOMAIN.to_owned(),
    };
    assert_eq!(
        page.vars(),
        vec![
            ("pageDomain", "sign.example.com".to_owned()),
            ("domain", APP_DOMAIN.to_owned())
        ]
    );
    assert_eq!(
        VenueBlock::AppCannotReach {
            domain: "x.example".to_owned()
        }
        .vars(),
        vec![("domain", "x.example".to_owned())]
    );
    assert!(VenueBlock::NotOnWeb.vars().is_empty());
}

// ---------------------------------------------------------------------------
// 14 · The check asks as a browser asks
// ---------------------------------------------------------------------------

#[test]
fn the_check_asks_for_the_page_as_a_browser_navigation_does() {
    let accept = CHECK_HEADERS
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("accept"))
        .map(|(_, value)| *value)
        .unwrap_or_default();
    assert!(
        accept.starts_with("text/html,application/xhtml+xml"),
        "{accept}"
    );
    assert!(accept.contains("*/*"), "and anything, as a browser does");
}

// ---------------------------------------------------------------------------
// 15 · A page address, read as meant — or refused
// ---------------------------------------------------------------------------

#[test]
fn what_a_chinese_keyboard_typed_is_read_as_meant() {
    // Found on Android: stored as `https://http：／／localhost：8140/`.
    assert_eq!(
        signer_url("http：／／localhost：8140").as_deref(),
        Ok("http://localhost:8140/")
    );
    assert_eq!(
        signer_url("ｈｔｔｐｓ：／／sign。example。com／ｐａｇｅ").as_deref(),
        Ok("https://sign.example.com/page")
    );
    assert_eq!(signer_url("sign.example.com\u{3000}").as_deref(), Ok(OWN));
}

#[test]
fn an_address_is_a_real_scheme_an_ascii_host_and_a_port() {
    for (typed, normal) in [
        ("HTTPS://Sign.Example.COM", OWN),
        ("https://sign.example.com:443", OWN),
        (
            "https://sign.example.com:8443/x",
            "https://sign.example.com:8443/x",
        ),
        ("http://localhost:80/", "http://localhost/"),
        ("http://[::1]:8140", "http://[::1]:8140/"),
        ("http://127.0.0.1:8140/s", "http://127.0.0.1:8140/s"),
        (
            "https://xn--fsqu00a.xn--fiqs8s/",
            "https://xn--fsqu00a.xn--fiqs8s/",
        ),
        (
            "https://sign.example.com/页",
            "https://sign.example.com/%E9%A1%B5",
        ),
    ] {
        assert_eq!(signer_url(typed).as_deref(), Ok(normal), "{typed}");
    }
    for typed in [
        "",
        "ftp://sign.example.com",
        "javascript://x",
        "https://例子.中国/",
        "https://sign.example.com:0",
        "https://sign.example.com:65536",
        "https://sign.example.com:",
        "https://sign.example.com:84a3",
        "https://user@sign.example.com",
        "https://-bad.example.com",
        "https://sign..example.com",
        "https://sign.example.com./",
        "https://sign_example.com",
        "https://sign.example.com/a b",
        "https://[zz]:1/",
        "https://sign.example.com/\u{7}",
    ] {
        assert_eq!(signer_url(typed), Err(SignerUrlError::Invalid), "{typed:?}");
    }
    assert_eq!(
        signer_url("http://sign.example.com"),
        Err(SignerUrlError::Insecure)
    );
}

// ---------------------------------------------------------------------------
// The words — every key this round names exists in all fifteen languages
// ---------------------------------------------------------------------------

/// The titles a page is opened under (D6): the hand-off card's and each
/// ceremony's. None of them owns the page.
const OWNS_NO_PAGE: [&str; 4] = [
    "componentsUi.signing.handoffTitle",
    "componentsUi.signing.ceremonyCreate",
    "componentsUi.signing.ceremonySignIn",
    "componentsUi.signing.ceremonyConfirm",
];

/// A possessive, per language, as it is written in a lowercased line padded
/// with a space at each end — whole words where the language has them, the
/// possessive suffix where it is one (Turkish).
fn possessives(lang: &str) -> &'static [&'static str] {
    match lang {
        "en" => &[" your ", " my ", " own "],
        "de" => &[" dein", " ihr ", " ihre", " mein"],
        "es-MX" => &[" tu ", " tus ", " su ", " sus ", " mi "],
        "fr" => &[" votre ", " vos ", " ta ", " ton ", " ma ", " mon "],
        "id" => &[" anda", "-mu ", " saya "],
        "it" => &[" tua ", " tuo ", " sua ", " suo ", " mia ", " mio "],
        "ja" => &["あなたの", "自分の", "私の"],
        "ko" => &["당신의", " 내 ", "나의"],
        "pt-BR" => &[" sua ", " seu ", " minha ", " meu "],
        "ru" => &[" сво", " ваш", " тво", " мой ", " моей ", " мою "],
        "tr" => &["ınız", "iniz", "unuz", "ünüz", "nız", "niz", " benim "],
        "vi" => &["của bạn", "của tôi"],
        "zh" | "zh-TW" => &["你的", "您的", "我的", "自己的"],
        "zh-HK" => &["你嘅", "您嘅", "我嘅", "自己嘅", "你的"],
        other => unreachable!("a sixteenth language: {other}"),
    }
}

#[test]
fn every_new_line_is_in_fifteen_languages() {
    let keys = [
        VenueBlock::NotOnWeb.key(),
        "settings.venue.inVela",
        "settings.venue.page",
        "settings.signing.pageOfficial",
        "settings.signing.pageAdd",
        "settings.signing.pageSelfHosted",
        "settings.signing.pageTrust",
        "settings.signing.pageRename",
        "settings.signing.pageRemove",
        "settings.signing.pageName",
        "componentsUi.signing.handoffTitle",
        "componentsUi.signing.confirmWithLabel",
        "componentsUi.signing.newKeyOnLabel",
        "componentsUi.signing.ceremonyCreate",
        "componentsUi.signing.ceremonySignIn",
        "componentsUi.signing.ceremonyConfirm",
        "onboarding.create.signingPageTitle",
        "onboarding.create.signingPageBody",
        "send.gasTier.slow",
        "send.gasTier.standard",
        "send.gasTier.fast",
    ];
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/i18n/locales");
    let mut languages = 0;
    for entry in std::fs::read_dir(root).unwrap_or_else(|e| unreachable!("{e}")) {
        let dir = entry.unwrap_or_else(|e| unreachable!("{e}")).path();
        if !dir.is_dir() {
            continue;
        }
        for key in keys {
            let mut parts = key.split('.');
            let namespace = parts.next().unwrap_or_default();
            let own = dir.join(format!("{namespace}.json"));
            let file = if own.exists() {
                own
            } else {
                dir.with_extension("json")
            };
            let text = std::fs::read_to_string(&file).unwrap_or_default();
            let mut node: Value = serde_json::from_str(&text).unwrap_or_default();
            node = node[namespace].clone();
            for part in parts {
                node = node[part].clone();
            }
            let line = node.as_str().unwrap_or_default();
            assert!(!line.trim().is_empty(), "{}: {key}", dir.display());
            if key.ends_with("pageSelfHosted") {
                assert!(line.contains("{{domain}}"), "{}: {key}", dir.display());
            }
            // D6: "my own" names only a page the person deployed.
            if key == "onboarding.create.signingPageTitle" {
                assert!(!line.to_lowercase().contains(" my own"), "{line}");
            }
            // D6: the hand-off card opens Vela's page as often as a
            // self-hosted one, so its title owns neither — nor does a
            // ceremony's ("Create a key on the signing page", never "your
            // key on your signing page"): the page is the one the account
            // signs on, Vela's or self-deployed.
            if OWNS_NO_PAGE.contains(&key) {
                let lang = dir
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default();
                let lower = format!(" {} ", line.to_lowercase());
                for possessive in possessives(lang) {
                    assert!(
                        !lower.contains(possessive),
                        "{lang}: {key} = {line:?} owns the page ({possessive:?})"
                    );
                }
            }
        }
        // The pre-D6 names are gone.
        let onboarding = std::fs::read_to_string(dir.join("onboarding.json")).unwrap_or_default();
        assert!(!onboarding.contains("ownPageTitle"), "{}", dir.display());
        languages += 1;
    }
    assert_eq!(languages, 15);
}
