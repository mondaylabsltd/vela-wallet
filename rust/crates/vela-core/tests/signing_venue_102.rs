//! Spec 102 — where a person reviews and signs. One test per rule, against
//! records as builds before 102 wrote them (`tests/fixtures/spec102/`).
//!
//! - **Migration**: every old record reads into the new model per the spec's
//!   table, and the record this build writes back keeps an older build
//!   signing (it never writes the retired `trusted_signer` route).
//! - **R1–R5** through the account: the venue reaches the keys, the key route
//!   names the key's place.
//! - **R6**: the URL opened is the version checked, and a failed, missing or
//!   mismatched check opens nothing.

#![cfg(feature = "crux")]

use serde_json::Value;
use vela_core::app::{Account, KeyMethod};
use vela_core::signing_venue::{SigningVenue, VenueBlock, APP_DOMAIN};
use vela_core::trusted_signer::integrity::{
    self, hash_page, CheckFailure, NoVersion, Verdict, BUILD_ALLOWED, LAUNCH,
};
use vela_core::trusted_signer::launch::{
    admit, target, Admission, IntegrityLine, IntegrityState, LaunchRefused, MAX_CHECK_AGE_MS,
};

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

fn page(url: &str) -> SigningVenue {
    SigningVenue::page(url).unwrap_or_else(|| unreachable!("{url}"))
}

// ---------------------------------------------------------------------------
// What a build before 102 does with a record — ≤ 0.9.7's routing, restated.
// ---------------------------------------------------------------------------

/// Where an older build sends a signature for `record`.
#[derive(Debug, PartialEq, Eq)]
enum OldRoute {
    /// Natively, pinned to this credential over this route.
    Native { credential: String, method: String },
    /// On the page at this origin (empty: the page Settings names).
    Page { origin: String },
    /// No route at all: "as it always did" — the first key's own transports.
    AsItAlwaysDid,
}

/// ≤ 0.9.7: `Account::sign_in_route` from `signed_in_with` (dropped whole when
/// it does not read: `credential_id` and one of four `method`s required), else
/// `wallet_keys::sign_route(keys, "auto")` — the first key with a credential
/// goes to its page when it names one.
fn old_build_route(record: &Value) -> OldRoute {
    let keys = record["keys"].as_array().cloned().unwrap_or_default();
    let holds = |credential: &str| {
        record["id"] == credential || keys.iter().any(|key| key["credential_id"] == credential)
    };
    let origin_of = |credential: &str| {
        keys.iter()
            .find(|key| key["credential_id"] == credential)
            .and_then(|key| key["signer_origin"].as_str())
            .unwrap_or_default()
            .to_owned()
    };
    let sign_in = &record["signed_in_with"];
    if let (Some(credential), Some(method)) = (
        sign_in["credential_id"].as_str(),
        sign_in["method"].as_str(),
    ) {
        let known = ["platform", "hybrid", "security_key", "trusted_signer"];
        if known.contains(&method) && !credential.is_empty() && holds(credential) {
            return if method == "trusted_signer" {
                OldRoute::Page {
                    origin: sign_in["signer_origin"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| origin_of(credential)),
                }
            } else {
                OldRoute::Native {
                    credential: credential.to_owned(),
                    method: method.to_owned(),
                }
            };
        }
    }
    match keys
        .iter()
        .find(|key| {
            key["credential_id"]
                .as_str()
                .is_some_and(|id| !id.is_empty())
        })
        .and_then(|key| key["signer_origin"].as_str())
        .filter(|origin| !origin.is_empty())
    {
        Some(origin) => OldRoute::Page {
            origin: origin.to_owned(),
        },
        None => OldRoute::AsItAlwaysDid,
    }
}

/// What this build writes back for `account`.
fn written(account: &Account) -> Value {
    serde_json::to_value(account).unwrap_or_default()
}

/// Every record this build writes: no retired route, the new fields present,
/// and still a record an older reader takes (its required fields there).
fn assert_written_well(account: &Account) -> Value {
    let value = written(account);
    let text = value.to_string();
    assert!(
        !text.contains("trusted_signer"),
        "the fourth route is never written: {text}"
    );
    for field in [
        "id",
        "address",
        "public_key_hex",
        "created_at_iso",
        "signing_domain",
    ] {
        assert!(value.get(field).is_some(), "{field} missing: {text}");
    }
    assert_eq!(read(&value), *account, "reads back as it was");
    value
}

// ---------------------------------------------------------------------------
// Migration — one test per row of data-model.md's table
// ---------------------------------------------------------------------------

/// An account that signed on the official page: the apps' domain, the official
/// page as its venue, the key's place read from its stored transports.
#[test]
fn an_official_page_account_migrates_to_the_official_venue() {
    let old = fixture("official-page-account.json");
    assert_eq!(
        old_build_route(&old),
        OldRoute::Page {
            origin: "https://sign.getvela.app".to_owned()
        },
        "what 0.9.7 did with it"
    );
    let account = read(&old);
    assert_eq!(account.signing_domain, APP_DOMAIN);
    assert_eq!(account.signing_venue, SigningVenue::official());
    let key = account
        .sign_in_key
        .clone()
        .unwrap_or_else(|| unreachable!());
    assert_eq!(
        key.method,
        KeyMethod::Platform,
        "`hybrid,internal`: this device"
    );
    assert_eq!(
        key.transports, "hybrid,internal",
        "the key's own transports"
    );

    let plan = account.signing_plan();
    assert_eq!(plan.blocked, None);
    let route = plan.key.unwrap_or_else(|| unreachable!());
    assert_eq!(route.credential_id, "a1b2c3d4");
    assert_eq!(route.method, "platform");
    assert_eq!(route.transports, "internal,hybrid");
    assert_eq!(route.hints, ["client-device"]);

    // Written back: the page's origin stays on the key, and an older build
    // reads a native route to a key it can reach — usable, if not on the page.
    let new = assert_written_well(&account);
    assert_eq!(new["keys"][0]["signer_origin"], "https://sign.getvela.app");
    assert_eq!(
        old_build_route(&new),
        OldRoute::Native {
            credential: "a1b2c3d4".to_owned(),
            method: "platform".to_owned()
        }
    );
}

/// A custom-origin account: its host is the domain, its page the venue —
/// locked, Vela's sheet unreachable — and the page is what an older build
/// still opens after this build rewrites the record.
#[test]
fn a_custom_origin_account_migrates_locked_to_its_page() {
    let old = fixture("custom-origin-account.json");
    let account = read(&old);
    assert_eq!(account.signing_domain, "localhost");
    assert_eq!(account.signing_venue, page("http://localhost:8140/"));
    assert_eq!(
        account.sign_in_key.as_ref().map(|key| key.method),
        Some(KeyMethod::SecurityKey),
        "`usb,nfc`: a USB key, wherever the page ran"
    );
    // Every key now names the page — the second had none on disk.
    assert!(account
        .keys
        .iter()
        .all(|key| key.signer_origin.as_deref() == Some("http://localhost:8140")));
    // R1/R2: Vela's sheet cannot reach these keys; another domain's page
    // neither.
    let mut changed = account.clone();
    assert_eq!(
        changed.choose_venue(SigningVenue::InVela),
        Err(VenueBlock::AppCannotReach {
            domain: "localhost".to_owned()
        })
    );
    assert!(changed.choose_venue(SigningVenue::official()).is_err());
    assert_eq!(changed, account, "nothing changed");

    let new = assert_written_well(&account);
    assert!(
        new.get("signed_in_with").is_none(),
        "no native route to keys an app cannot reach"
    );
    assert_eq!(
        old_build_route(&new),
        OldRoute::Page {
            origin: "http://localhost:8140".to_owned()
        },
        "an older build still signs on the page"
    );
}

/// An account that never used a page: Vela's sheet, its sign-in key and place
/// exactly as they were.
#[test]
fn an_app_only_account_migrates_to_vela_unchanged() {
    let old = fixture("app-only-account.json");
    let account = read(&old);
    assert_eq!(account.signing_domain, APP_DOMAIN);
    assert_eq!(account.signing_venue, SigningVenue::InVela);
    let route = account.key_route().unwrap_or_else(|| unreachable!());
    assert_eq!(
        (
            route.credential_id.as_str(),
            route.method.as_str(),
            route.transports.as_str()
        ),
        ("5ec0de02", "security_key", "usb,nfc,ble,hybrid")
    );
    let new = assert_written_well(&account);
    assert_eq!(
        old_build_route(&new),
        old_build_route(&old),
        "an older build sees no change"
    );
}

/// A page sign-in whose page was never recorded meant "the page Settings
/// names"; it migrates to the official page (spec 102, Migration).
#[test]
fn an_empty_signer_origin_migrates_to_the_official_page() {
    let old = fixture("empty-signer-origin-account.json");
    let account = read(&old);
    assert_eq!(account.signing_domain, APP_DOMAIN);
    assert_eq!(account.signing_venue, SigningVenue::official());
    assert_eq!(
        account
            .sign_in_key
            .as_ref()
            .map(|key| (key.method, key.transports.as_str())),
        Some((KeyMethod::Platform, "internal"))
    );
    let new = assert_written_well(&account);
    assert!(matches!(old_build_route(&new), OldRoute::Native { .. }));
}

/// A record from before the sign-in key was kept took the first key's page
/// when it named one — and still does, as its venue.
#[test]
fn a_record_from_before_the_sign_in_key_keeps_its_page() {
    let account = read(&fixture("before-sign-in-key-page.json"));
    assert_eq!(account.signing_venue, SigningVenue::official());
    assert_eq!(account.signing_domain, APP_DOMAIN);
    assert_eq!(account.key_route(), None, "signs as it always did");
    let new = assert_written_well(&account);
    assert!(new.get("signed_in_with").is_none());
    assert_eq!(
        old_build_route(&new),
        OldRoute::Page {
            origin: "https://sign.getvela.app".to_owned()
        }
    );

    // The retired client's camelCase record, no keys, no page: Vela.
    let account = read(&fixture("before-sign-in-key-app.json"));
    assert_eq!(account.signing_venue, SigningVenue::InVela);
    assert_eq!(account.key_route(), None);
    assert_written_well(&account);
}

/// Reading is idempotent: a migrated record written and read again is the
/// same account, and every fixture's plan reaches its keys.
#[test]
fn every_migrated_record_is_stable_and_signable() {
    for name in [
        "official-page-account.json",
        "custom-origin-account.json",
        "app-only-account.json",
        "empty-signer-origin-account.json",
        "before-sign-in-key-page.json",
        "before-sign-in-key-app.json",
    ] {
        let once = read(&fixture(name));
        let twice = read(&written(&once));
        assert_eq!(once, twice, "{name}");
        assert_eq!(written(&once), written(&twice), "{name}");
        assert_eq!(once.signing_plan().blocked, None, "{name}");
    }
}

/// A custom-domain account whose page this device does not know — its domain
/// stored, no key naming a page — cannot sign anywhere here, and says so
/// rather than opening Vela's sheet on keys it cannot reach.
#[test]
fn a_custom_domain_with_no_page_known_is_blocked_not_misrouted() {
    let mut record = fixture("app-only-account.json");
    record["signing_domain"] = "sign.example.com".into();
    let plan = read(&record).signing_plan();
    assert_eq!(
        plan.blocked,
        Some(VenueBlock::AppCannotReach {
            domain: "sign.example.com".to_owned()
        })
    );
}

// ---------------------------------------------------------------------------
// R6 — the URL opened is the version checked
// ---------------------------------------------------------------------------

const OFFICIAL: &str = "https://sign.getvela.app/";
const UNKNOWN: &str = "ff11223344556677889900aabbccddeeff00112233445566778899aabbccddee";

fn listed(hashes: &[&str]) -> Vec<String> {
    hashes.iter().map(|hash| (*hash).to_owned()).collect()
}

/// The invariant: the address a check fetches and the address a launch opens
/// are one value, and it names the version that was checked — for the
/// official page and for anybody's deployment, with or without an index.
#[test]
fn the_launch_url_is_the_checked_version() {
    let request = serde_json::json!({ "intent": { "method": "personal_sign" } });
    for (base, index) in [
        (OFFICIAL, None),
        (
            OFFICIAL,
            Some(listed(&[BUILD_ALLOWED[1], BUILD_ALLOWED[2]])),
        ),
        ("http://localhost:8140", Some(listed(&[BUILD_ALLOWED[0]]))),
        ("https://sign.example.com/signer/", None),
    ] {
        let chosen = target(base, index.as_deref(), &[], &[]).unwrap_or_else(|why| {
            unreachable!("{base}: {why:?}");
        });
        assert!(
            chosen.url().contains(&format!("/b/{}/", chosen.version())),
            "{base}"
        );
        let admitted = admit(
            &chosen,
            Some(chosen.version()),
            CheckFailure::NotChecked,
            &[],
            &[],
            false,
            1_000,
        );
        let checked = admitted.page().unwrap_or_else(|| unreachable!("{base}"));
        let url = checked
            .url_launch(&request, "velawallet://sign-result", "t", "", 1_000)
            .unwrap_or_else(|_| unreachable!());
        assert!(
            url.starts_with(&format!("{}?ch=url#", chosen.url())),
            "{base}: opened {url}, checked {}",
            chosen.url()
        );
        assert_eq!(checked.target(), &chosen);
    }
}

/// Without an index the official page opens its deployed launch version —
/// spec 079's offline pin, now checked before it opens.
#[test]
fn no_index_asks_the_official_page_for_its_launch_version() {
    let chosen = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|_| unreachable!());
    assert_eq!(chosen.version(), LAUNCH);
    assert_eq!(
        chosen.url(),
        format!("https://sign.getvela.app/b/{LAUNCH}/sign")
    );
    // A custom deployment is a plain static host: its file is `sign.html`.
    let custom =
        target("https://sign.example.com/x", None, &[], &[]).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        custom.url(),
        format!(
            "https://sign.example.com/x/b/{}/sign.html",
            BUILD_ALLOWED[0]
        )
    );
}

/// The desktop's checker, through the rule: read the index, take the target,
/// fetch exactly its URL, hash the bytes, admit. Here with the committed
/// bytes of the launch version, as the official host serves them.
#[test]
fn the_desktops_checker_works_through_the_rule() {
    let available = listed(BUILD_ALLOWED);
    let chosen = target(OFFICIAL, Some(&available), &[], &[]).unwrap_or_else(|_| unreachable!());
    assert_eq!(
        chosen.version(),
        BUILD_ALLOWED[0],
        "the newest the index lists"
    );
    let bytes = std::fs::read(format!(
        "{}/../../../app-web/trusted-signer/dist/b/{}/sign.html",
        env!("CARGO_MANIFEST_DIR"),
        chosen.version()
    ))
    .unwrap_or_default();
    let admitted = admit(
        &chosen,
        Some(&hash_page(&bytes)),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        5_000,
    );
    let line = admitted.line();
    assert_eq!(line.state, IntegrityState::Matches);
    assert_eq!(line.version, &chosen.version()[..8]);
    assert_eq!(line.checked_at_ms, Some(5_000));
    assert!(line.opens);
}

/// FR-006: a check that did not complete — unreachable, unreadable, or never
/// run — opens nothing, and the card says so.
#[test]
fn a_failed_or_missing_check_opens_nothing() {
    let chosen = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|_| unreachable!());
    for failure in [
        CheckFailure::Unreachable,
        CheckFailure::NoCachedBytes,
        CheckFailure::NotChecked,
    ] {
        let admitted = admit(&chosen, None, failure, &[], &[], false, 1);
        assert_eq!(admitted.page(), None, "{failure:?}");
        assert!(matches!(
            admitted,
            Admission::Refused {
                verdict: Verdict::CouldNotCheck(_),
                ..
            }
        ));
        let line = admitted.line();
        assert_eq!(line.state, IntegrityState::CouldNotCheck);
        assert!(!line.opens);
    }
}

/// The bytes at `/b/<version>/` must BE that version. Another accepted
/// version served there is still refused: the person would be told they see
/// one version and get another.
#[test]
fn bytes_that_are_not_the_version_named_are_refused() {
    let chosen = target(OFFICIAL, Some(&listed(&[BUILD_ALLOWED[1]])), &[], &[])
        .unwrap_or_else(|_| unreachable!());
    let admitted = admit(
        &chosen,
        Some(BUILD_ALLOWED[2]),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        1,
    );
    match &admitted {
        Admission::Refused {
            verdict: Verdict::Refused { actual, expected },
            ..
        } => {
            assert_eq!(actual, BUILD_ALLOWED[2]);
            assert_eq!(expected, &vec![BUILD_ALLOWED[1].to_owned()]);
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    assert_eq!(admitted.line().state, IntegrityState::Mismatch);
}

/// Unknown bytes on the official page: refused, nobody to ask. A blocked
/// version: refused, whatever else vouches for it.
#[test]
fn unknown_and_blocked_versions_are_refused() {
    let chosen = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|_| unreachable!());
    let unknown = admit(
        &chosen,
        Some(UNKNOWN),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        1,
    );
    assert_eq!(unknown.line().state, IntegrityState::Mismatch);
    assert_eq!(unknown.page(), None);

    let blocked = vec![LAUNCH.to_owned()];
    let denied = admit(
        &chosen,
        Some(LAUNCH),
        CheckFailure::NotChecked,
        &[],
        &blocked,
        false,
        1,
    );
    assert_eq!(denied.line().state, IntegrityState::Blocked);
    assert_eq!(denied.page(), None);
}

/// The official page never takes a version on its index's word. A custom
/// page's own build is proposed by its index — and can only end in asking
/// the person; once trusted, it opens as "trusted on this device".
#[test]
fn a_custom_build_is_asked_about_never_opened_on_the_indexs_word() {
    let only_unknown = listed(&[UNKNOWN]);
    assert_eq!(
        target(OFFICIAL, Some(&only_unknown), &[], &[]),
        Err(NoVersion::NothingPublishedThisWalletKnows)
    );
    let base = "https://sign.example.com/";
    let proposed = target(base, Some(&only_unknown), &[], &[]).unwrap_or_else(|_| unreachable!());
    assert!(proposed.proposed_by_index());
    let asked = admit(
        &proposed,
        Some(UNKNOWN),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        1,
    );
    assert_eq!(asked.line().state, IntegrityState::AskToTrust);
    assert_eq!(asked.page(), None);

    let trusted = vec![UNKNOWN.to_owned()];
    let chosen =
        target(base, Some(&only_unknown), &trusted, &[]).unwrap_or_else(|_| unreachable!());
    assert!(!chosen.proposed_by_index());
    let admitted = admit(
        &chosen,
        Some(UNKNOWN),
        CheckFailure::NotChecked,
        &trusted,
        &[],
        false,
        7,
    );
    assert_eq!(admitted.line().state, IntegrityState::TrustedHere);
    assert!(admitted.page().is_some());

    // Blocked, it is not even proposed.
    assert_eq!(
        target(base, Some(&only_unknown), &[], &trusted),
        Err(NoVersion::NothingPublishedThisWalletKnows)
    );
}

/// With checking turned off for a custom page, it opens — and the line says
/// it was not checked. The official page has no such switch.
#[test]
fn checking_off_opens_a_custom_page_unchecked_and_never_the_official_one() {
    let custom = target(
        "https://sign.example.com",
        Some(&listed(&[UNKNOWN])),
        &[],
        &[],
    )
    .unwrap_or_else(|_| unreachable!());
    let admitted = admit(
        &custom,
        Some(UNKNOWN),
        CheckFailure::NotChecked,
        &[],
        &[],
        true,
        1,
    );
    assert_eq!(admitted.line().state, IntegrityState::Unchecked);
    assert!(admitted.page().is_some());

    let official = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|_| unreachable!());
    let refused = admit(
        &official,
        Some(UNKNOWN),
        CheckFailure::NotChecked,
        &[],
        &[],
        true,
        1,
    );
    assert_eq!(refused.page(), None);
}

/// A check vouches for a day. After that the page is checked again before
/// it opens, and the card says "checking" rather than an old "checked".
#[test]
fn a_stale_check_opens_nothing_until_it_is_run_again() {
    let chosen = target(OFFICIAL, None, &[], &[]).unwrap_or_else(|_| unreachable!());
    let admitted = admit(
        &chosen,
        Some(LAUNCH),
        CheckFailure::NotChecked,
        &[],
        &[],
        false,
        10,
    );
    let checked = admitted.page().unwrap_or_else(|| unreachable!());
    let request = serde_json::json!({});
    let late = 10 + MAX_CHECK_AGE_MS + 1;
    assert_eq!(
        checked.url_launch(&request, "velawallet://sign-result", "t", "", late),
        Err(LaunchRefused::Stale)
    );
    assert_eq!(
        checked.ws_launch(1, "t", "", late),
        Err(LaunchRefused::Stale)
    );
    assert_eq!(checked.line(late), IntegrityLine::checking());
    assert!(checked
        .url_launch(
            &request,
            "velawallet://sign-result",
            "t",
            "",
            10 + MAX_CHECK_AGE_MS
        )
        .is_ok());
}

/// Nothing to ask for, and why: nothing this wallet knows is published, or
/// everything that would do is blocked here.
#[test]
fn no_version_to_ask_for_is_said_as_such() {
    let all_blocked = listed(BUILD_ALLOWED);
    assert_eq!(
        target(OFFICIAL, None, &[], &all_blocked),
        Err(NoVersion::EverythingUsableIsBlocked)
    );
    assert_eq!(
        IntegrityLine::no_version(NoVersion::EverythingUsableIsBlocked).state,
        IntegrityState::AllBlocked
    );
    assert_eq!(
        IntegrityLine::no_version(NoVersion::NothingPublishedThisWalletKnows).state,
        IntegrityState::NoVersion
    );
}

#[test]
#[allow(clippy::assertions_on_constants)]
fn enforcement_is_on() {
    assert!(integrity::ENFORCE);
}

/// Every integrity line and every reason a venue is blocked is a string in
/// all 15 languages, so no shell draws a bare key.
#[test]
fn every_line_is_in_every_language() {
    let mut keys: Vec<&str> = [
        IntegrityState::Checking,
        IntegrityState::Matches,
        IntegrityState::TrustedHere,
        IntegrityState::Unchecked,
        IntegrityState::Mismatch,
        IntegrityState::Blocked,
        IntegrityState::AskToTrust,
        IntegrityState::CouldNotCheck,
        IntegrityState::NoVersion,
        IntegrityState::AllBlocked,
    ]
    .iter()
    .map(|state| state.key())
    .collect();
    keys.push(
        VenueBlock::AppCannotReach {
            domain: String::new(),
        }
        .key(),
    );
    keys.push(
        VenueBlock::PageOnOtherDomain {
            page_domain: String::new(),
            domain: String::new(),
        }
        .key(),
    );
    let root = format!("{}/i18n/locales", env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0;
    for entry in std::fs::read_dir(&root).unwrap_or_else(|e| unreachable!("{e}")) {
        let dir = entry.unwrap_or_else(|e| unreachable!("{e}")).path();
        if !dir.is_dir() {
            continue;
        }
        let lang = dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_owned();
        let mut merged = serde_json::Map::new();
        let top: Value = serde_json::from_str(
            &std::fs::read_to_string(format!("{root}/{lang}.json")).unwrap_or_default(),
        )
        .unwrap_or_default();
        merged.extend(top.as_object().cloned().unwrap_or_default());
        for file in std::fs::read_dir(&dir).unwrap_or_else(|e| unreachable!("{e}")) {
            let text = std::fs::read_to_string(file.unwrap_or_else(|e| unreachable!("{e}")).path())
                .unwrap_or_default();
            let value: Value = serde_json::from_str(&text).unwrap_or_default();
            merged.extend(value.as_object().cloned().unwrap_or_default());
        }
        let merged = Value::Object(merged);
        for key in &keys {
            let node = key.split('.').fold(&merged, |node, part| &node[part]);
            assert!(
                node.as_str().is_some_and(|text| !text.trim().is_empty()),
                "{lang}: {key} is missing"
            );
        }
        checked += 1;
    }
    assert_eq!(checked, 15);
}
