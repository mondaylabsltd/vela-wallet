//! Where a person reviews and signs (spec 102): in Vela, or on a page they
//! trust.
//!
//! The Trusted Signer used to be drawn as a fourth place a key lives, beside
//! this device, a phone and a USB key. It never was one: the same three places
//! exist on the page too, and the page is where a person *checks what they
//! sign* (`trusted_signer.rs`, from the start). So an account now has two
//! independent things, and one property:
//!
//! - **where its key lives** — this device, a phone or tablet, a USB key
//!   ([`crate::app::KeyMethod`], three values and no fourth);
//! - **its signing venue** ([`SigningVenue`]) — Vela's own sheet, or a saved
//!   page. Chosen per account on this device and changeable at any time
//!   without touching a key (owner, D1);
//! - **its signing domain** — the RP ID its keys live under. `getvela.app` for
//!   every account made in the apps or on the official page; the person's own
//!   domain for one made on a self-hosted page (D3).
//!
//! The rules below decide; the shells draw.
//!
//! - **R1** [`reachability`] — a venue can be used only if it can use the
//!   account's keys: Vela's sheet ⇔ the domain is `getvela.app`; a page ⇔ the
//!   page's domain is the account's. An unreachable venue comes back with the
//!   reason a person reads ([`VenueBlock`]).
//! - **R2** [`venue_choices`] — every venue an account could pick, each
//!   reachable or not and why; a custom-domain account is locked to pages on
//!   its domain, and when several share it the person picks one.
//! - **R3** [`ceremony_page`] — a key ceremony (create, add a key, sign in, a
//!   proof) runs in the app for a `getvela.app` account, and on its page for a
//!   custom-domain one: only that page can mint or use those keys.
//! - **R4** [`venue_for`] — the venue applies to transactions and messages;
//!   a ceremony follows R3.
//! - **R5** [`KeyRoute`] — the page is told which key to use and where it
//!   lives, so the browser goes straight to it instead of asking again.
//!
//! What this does NOT claim: the domain is shared (D2), so a page as venue
//! assures *what you see is what you sign*; it does not stop a fully
//! compromised app from asking a native passkey for a signature.
//!
//! Pure: no I/O, no clock.

use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::trusted_signer;

/// The signing domain of every account made in the apps or on the official
/// page (D2): the RP ID the apps are bound to, and the parent the official
/// page's keys fold to.
pub const APP_DOMAIN: &str = "getvela.app";

/// Where an account's transactions and messages are previewed and signed on
/// this device.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SigningVenue {
    /// Vela's own signing sheet.
    #[default]
    InVela,
    /// A trusted signing page, by its base address as
    /// [`trusted_signer::signer_url`] normalises it (`https://sign.getvela.app/`).
    Page { url: String },
}

impl SigningVenue {
    /// The official page.
    #[must_use]
    pub fn official() -> Self {
        Self::Page {
            url: trusted_signer::DEFAULT_SIGNER_URL.to_owned(),
        }
    }

    /// A page venue for `url`, normalised — `None` for an address no browser
    /// would run a passkey ceremony on.
    #[must_use]
    pub fn page(url: &str) -> Option<Self> {
        trusted_signer::signer_url(url)
            .ok()
            .map(|url| Self::Page { url })
    }

    /// The page's address, for a page venue.
    #[must_use]
    pub fn page_url(&self) -> Option<&str> {
        match self {
            Self::InVela => None,
            Self::Page { url } => Some(url),
        }
    }

    /// The domain whose keys this venue can use.
    #[must_use]
    pub fn domain(&self) -> String {
        match self {
            Self::InVela => APP_DOMAIN.to_owned(),
            Self::Page { url } => domain_of_page(url),
        }
    }

    /// Two venues are the same place: both Vela's sheet, or pages at the same
    /// normalised address.
    #[must_use]
    pub fn same_as(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::InVela, Self::InVela) => true,
            (Self::Page { url: a }, Self::Page { url: b }) => same_page(a, b),
            _ => false,
        }
    }
}

/// The RP ID a page at `url` mints and uses keys for — the page's own host,
/// except that every `*.getvela.app` page folds to [`APP_DOMAIN`] (the page's
/// `signer.js` folds it the same way, and the two must agree or nothing
/// matches). Empty for something that is not an address.
#[must_use]
pub fn domain_of_page(url: &str) -> String {
    trusted_signer::registry_rp_id(Some(url)).unwrap_or_default()
}

/// Is this the apps' own signing domain?
#[must_use]
pub fn is_app_domain(domain: &str) -> bool {
    domain.trim().eq_ignore_ascii_case(APP_DOMAIN)
}

/// Is an account on this domain locked to a page (R2)? Only `getvela.app`
/// keys are reachable from Vela's own sheet.
#[must_use]
pub fn locked_to_page(domain: &str) -> bool {
    !is_app_domain(domain)
}

fn same_page(a: &str, b: &str) -> bool {
    let normal = |url: &str| trusted_signer::signer_url(url).unwrap_or_else(|_| url.to_owned());
    normal(a) == normal(b)
}

// ---------------------------------------------------------------------------
// R1 — reachability
// ---------------------------------------------------------------------------

/// Why a venue cannot be used for an account (R1). Carries the two facts the
/// sentence needs; the words are the shells' ([`VenueBlock::key`]).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum VenueBlock {
    /// Vela's own sheet signs with `getvela.app` keys only, and this account's
    /// keys live on `domain` — a passkey made for another site cannot be used
    /// by an app that is not bound to it.
    AppCannotReach { domain: String },
    /// The page lives on `page_domain` and the account's keys on `domain`: a
    /// browser lets a page use only the passkeys of its own site.
    PageOnOtherDomain { page_domain: String, domain: String },
}

impl VenueBlock {
    /// The corpus key of the reason line. `{{domain}}` always, and for a page
    /// `{{pageDomain}}` too.
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::AppCannotReach { .. } => "settings.venue.blockedApp",
            Self::PageOnOtherDomain { .. } => "settings.venue.blockedPage",
        }
    }
}

/// R1: can `venue` use the keys of an account whose signing domain is
/// `domain`?
///
/// # Errors
///
/// The [`VenueBlock`] a person is shown beside the disabled choice.
pub fn reachability(domain: &str, venue: &SigningVenue) -> Result<(), VenueBlock> {
    let domain = domain.trim().to_ascii_lowercase();
    match venue {
        SigningVenue::InVela if is_app_domain(&domain) => Ok(()),
        SigningVenue::InVela => Err(VenueBlock::AppCannotReach { domain }),
        SigningVenue::Page { url } => {
            let page_domain = trusted_signer::signer_url(url)
                .map(|url| domain_of_page(&url))
                .unwrap_or_default();
            if !page_domain.is_empty() && page_domain == domain {
                Ok(())
            } else {
                Err(VenueBlock::PageOnOtherDomain {
                    page_domain,
                    domain,
                })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// R2 — the choices an account has
// ---------------------------------------------------------------------------

/// One saved signing page (Settings → Signing pages). The official page is
/// never stored: it is always first, and cannot be removed or renamed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SigningPage {
    /// The base address, normalised by [`trusted_signer::signer_url`].
    pub url: String,
    /// The person's label; empty ⇒ the shell names it by its host.
    #[serde(default)]
    pub name: String,
}

/// One row of "Where you review and sign" (R1, R2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct VenueChoice {
    pub venue: SigningVenue,
    /// A saved page's label; empty for Vela's sheet, the official page and an
    /// unnamed page — the shell names those.
    pub name: String,
    /// The domain whose keys this choice can use.
    pub domain: String,
    /// The official page.
    pub official: bool,
    /// This account's venue now.
    pub active: bool,
    /// Why this choice cannot be used for the account; `None` when it can.
    /// A blocked row is drawn disabled, with this reason under it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<VenueBlock>,
}

/// R1 + R2: every venue an account on `domain` could pick, in the order they
/// are drawn — Vela's sheet, the official page, then the saved pages — each
/// reachable or blocked with its reason, and the account's `active` venue
/// marked. An active page that is not (or no longer) saved is listed last,
/// so the screen never hides where the account signs today.
#[must_use]
pub fn venue_choices(
    domain: &str,
    active: &SigningVenue,
    saved: &[SigningPage],
) -> Vec<VenueChoice> {
    let official = SigningVenue::official();
    let mut rows: Vec<(SigningVenue, String, bool)> = vec![
        (SigningVenue::InVela, String::new(), false),
        (official.clone(), String::new(), true),
    ];
    for page in saved {
        let Some(venue) = SigningVenue::page(&page.url) else {
            continue;
        };
        if rows.iter().any(|(listed, _, _)| listed.same_as(&venue)) {
            continue;
        }
        rows.push((venue, page.name.trim().to_owned(), false));
    }
    if !rows.iter().any(|(listed, _, _)| listed.same_as(active)) {
        rows.push((active.clone(), String::new(), false));
    }
    rows.into_iter()
        .map(|(venue, name, official)| VenueChoice {
            domain: venue.domain(),
            active: venue.same_as(active),
            blocked: reachability(domain, &venue).err(),
            venue,
            name,
            official,
        })
        .collect()
}

/// The venue an account falls back to when none was chosen, or when the one
/// stored cannot reach its keys: Vela's sheet for a `getvela.app` account; for
/// a custom-domain one, the first of `pages` on that domain — `None` when no
/// page there is known, and then nothing on this device can sign for it.
#[must_use]
pub fn default_venue<'a>(
    domain: &str,
    pages: impl IntoIterator<Item = &'a str>,
) -> Option<SigningVenue> {
    if is_app_domain(domain) {
        return Some(SigningVenue::InVela);
    }
    pages
        .into_iter()
        .filter_map(SigningVenue::page)
        .find(|venue| reachability(domain, venue).is_ok())
}

// ---------------------------------------------------------------------------
// R3 / R4 — which venue a request goes to
// ---------------------------------------------------------------------------

/// R3: the page a key ceremony runs on, when the person's chosen page is
/// `page` — `Some` only for a page on a custom domain, because only that page
/// can mint or use its keys. A `getvela.app` page (the official one included)
/// gives `None`: the ceremony runs in the app, where the challenge is random
/// or derived and there is nothing to preview.
#[must_use]
pub fn ceremony_page(page: Option<&str>) -> Option<String> {
    let url = trusted_signer::signer_url(page?).ok()?;
    let domain = domain_of_page(&url);
    (!domain.is_empty() && !is_app_domain(&domain)).then_some(url)
}

/// What is being signed, as far as the venue rule cares (R4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignatureKind {
    /// A user operation: the wallet's own send, or a site's transaction.
    UserOperation,
    /// `personal_sign`.
    PersonalSign,
    /// `eth_signTypedData_v4`.
    TypedData,
    /// Create, add a key, sign in, a member or recovery proof — a challenge
    /// that is random or derived, with nothing to preview.
    KeyCeremony,
}

/// R4: where a signature of `kind` goes for an account on `domain` whose venue
/// is `venue`. Transactions and messages follow the venue; a key ceremony runs
/// in the app for a `getvela.app` account and on the account's page otherwise
/// (R3).
#[must_use]
pub fn venue_for(kind: SignatureKind, domain: &str, venue: &SigningVenue) -> SigningVenue {
    match kind {
        SignatureKind::UserOperation | SignatureKind::PersonalSign | SignatureKind::TypedData => {
            venue.clone()
        }
        SignatureKind::KeyCeremony if is_app_domain(domain) => SigningVenue::InVela,
        SignatureKind::KeyCeremony => venue.clone(),
    }
}

// ---------------------------------------------------------------------------
// R5 — the key route
// ---------------------------------------------------------------------------

/// Which key a signature is pinned to and where it lives (R5) — what the
/// page is told so the browser goes straight to that key, and what a native
/// ceremony is pinned with.
///
/// Without it the page passes credential ids alone and the browser draws its
/// generic "where is your passkey?" chooser — this device / a phone / a
/// security key — which is the question the person already answered when
/// they created the account or signed in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct KeyRoute {
    /// Hex, no `0x` — how every shell stores credential ids.
    pub credential_id: String,
    /// Where the key lives: `platform` | `hybrid` | `security_key`.
    pub method: String,
    /// The transports an `allowCredentials` entry carries, comma joined.
    pub transports: String,
    /// WebAuthn L3 `hints` for the place: `client-device`, `hybrid`,
    /// `security-key`.
    #[serde(default)]
    pub hints: Vec<String>,
}

impl KeyRoute {
    /// A route to `credential_id` on `method`, its hints derived.
    #[must_use]
    pub fn new(credential_id: &str, method: &str, transports: &str) -> Self {
        Self {
            credential_id: credential_id.to_owned(),
            method: method.to_owned(),
            transports: transports.to_owned(),
            hints: hints_of(method)
                .iter()
                .map(|hint| (*hint).to_owned())
                .collect(),
        }
    }
}

/// The WebAuthn L3 `hints` for a key place. Empty for anything else.
#[must_use]
pub fn hints_of(method: &str) -> &'static [&'static str] {
    match method {
        "platform" => &["client-device"],
        "hybrid" => &["hybrid"],
        "security_key" => &["security-key"],
        _ => &[],
    }
}

/// How an account signs on this device, in one answer: its signing domain,
/// the venue its transactions and messages go to (already checked against R1),
/// and the key route (R5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SigningPlan {
    /// The RP ID the account's keys live under.
    pub domain: String,
    /// Where transactions and messages are previewed and signed.
    pub venue: SigningVenue,
    /// Set when nothing on this device can reach the account's keys — a
    /// custom-domain account whose page is unknown here. The shell signs
    /// nothing and shows this reason. `None` in every ordinary case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked: Option<VenueBlock>,
    /// The key this device signs with, and where it lives. `None` for a record
    /// written before the sign-in key was kept, which signs as it always did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeyRoute>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWN: &str = "https://sign.example.com/";

    fn page(url: &str) -> SigningVenue {
        SigningVenue::page(url).unwrap_or_else(|| unreachable!("{url} is a page"))
    }

    #[test]
    fn the_official_page_and_its_parent_are_the_apps_domain() {
        assert_eq!(domain_of_page("https://sign.getvela.app/"), APP_DOMAIN);
        assert_eq!(domain_of_page("https://getvela.app/sign.html"), APP_DOMAIN);
        assert_eq!(domain_of_page(OWN), "sign.example.com");
        assert_eq!(domain_of_page("http://localhost:8140/x/"), "localhost");
        assert_eq!(domain_of_page("https://notgetvela.app/"), "notgetvela.app");
        assert_eq!(domain_of_page(""), "");
        assert_eq!(SigningVenue::InVela.domain(), APP_DOMAIN);
        assert_eq!(SigningVenue::official().domain(), APP_DOMAIN);
    }

    /// R1 for every venue/domain pair: Vela's sheet reaches only `getvela.app`
    /// keys, and a page only its own domain's.
    #[test]
    fn every_venue_against_every_domain() {
        let venues = [
            SigningVenue::InVela,
            SigningVenue::official(),
            page("https://staging.getvela.app/"),
            page(OWN),
            page("http://localhost:8140/"),
        ];
        let domains = [APP_DOMAIN, "sign.example.com", "localhost"];
        // rows: venue; columns: domain; `true` = reachable.
        let expected = [
            [true, false, false],
            [true, false, false],
            [true, false, false],
            [false, true, false],
            [false, false, true],
        ];
        for (venue, row) in venues.iter().zip(expected) {
            for (domain, reachable) in domains.iter().zip(row) {
                assert_eq!(
                    reachability(domain, venue).is_ok(),
                    reachable,
                    "{venue:?} on {domain}"
                );
            }
        }
        // And the reasons name both sides.
        assert_eq!(
            reachability("sign.example.com", &SigningVenue::InVela),
            Err(VenueBlock::AppCannotReach {
                domain: "sign.example.com".to_owned()
            })
        );
        assert_eq!(
            reachability(APP_DOMAIN, &page(OWN)),
            Err(VenueBlock::PageOnOtherDomain {
                page_domain: "sign.example.com".to_owned(),
                domain: APP_DOMAIN.to_owned()
            })
        );
        // Case does not decide anything; a lookalike is another domain.
        assert!(reachability("GetVela.App", &SigningVenue::InVela).is_ok());
        assert!(reachability(APP_DOMAIN, &page("https://getvela.app.evil.test/")).is_err());
        // Something that is not a page reaches nothing.
        let junk = SigningVenue::Page {
            url: "not a url".to_owned(),
        };
        assert!(matches!(
            reachability(APP_DOMAIN, &junk),
            Err(VenueBlock::PageOnOtherDomain { ref page_domain, .. }) if page_domain.is_empty()
        ));
    }

    #[test]
    fn a_reason_names_its_line() {
        assert_eq!(
            VenueBlock::AppCannotReach {
                domain: String::new()
            }
            .key(),
            "settings.venue.blockedApp"
        );
        assert_eq!(
            VenueBlock::PageOnOtherDomain {
                page_domain: String::new(),
                domain: String::new()
            }
            .key(),
            "settings.venue.blockedPage"
        );
    }

    /// R2: a `getvela.app` account may pick Vela or any `getvela.app` page; a
    /// custom-domain one only pages on its domain — and when two share it, it
    /// picks one.
    #[test]
    fn the_choices_an_account_has() {
        let saved = [
            SigningPage {
                url: "https://sign.example.com".to_owned(),
                name: "Mine".to_owned(),
            },
            SigningPage {
                url: "https://sign.example.com/backup/".to_owned(),
                name: String::new(),
            },
            // The official page saved again, and a duplicate: listed once.
            SigningPage {
                url: "https://SIGN.getvela.app".to_owned(),
                name: "dup".to_owned(),
            },
            SigningPage {
                url: "https://sign.example.com/".to_owned(),
                name: "dup".to_owned(),
            },
            // Not a page at all: never offered.
            SigningPage {
                url: "ftp://x".to_owned(),
                name: String::new(),
            },
        ];
        let app = venue_choices(APP_DOMAIN, &SigningVenue::InVela, &saved);
        let shape: Vec<(bool, bool, bool)> = app
            .iter()
            .map(|row| (row.official, row.active, row.blocked.is_none()))
            .collect();
        assert_eq!(
            shape,
            [
                (false, true, true),   // In Vela, active
                (true, false, true),   // the official page
                (false, false, false), // Mine — another domain
                (false, false, false), // backup — another domain
            ]
        );
        assert_eq!(app[2].name, "Mine");
        assert_eq!(app[2].domain, "sign.example.com");

        let custom = venue_choices(
            "sign.example.com",
            &page("https://sign.example.com/"),
            &saved,
        );
        let reachable: Vec<bool> = custom.iter().map(|row| row.blocked.is_none()).collect();
        assert_eq!(reachable, [false, false, true, true]);
        assert!(custom[2].active && !custom[3].active);
        assert!(matches!(
            custom[0].blocked,
            Some(VenueBlock::AppCannotReach { .. })
        ));
    }

    /// The page an account signs on today is shown even when it is not saved —
    /// removing a saved page changes no account's venue.
    #[test]
    fn an_active_page_that_is_not_saved_is_still_listed() {
        let rows = venue_choices("localhost", &page("http://localhost:8140"), &[]);
        assert_eq!(rows.len(), 3);
        assert!(rows[2].active && rows[2].blocked.is_none());
        assert_eq!(rows[2].venue, page("http://localhost:8140/"));
    }

    #[test]
    fn the_fallback_venue() {
        assert_eq!(
            default_venue(APP_DOMAIN, ["https://x.example/"]),
            Some(SigningVenue::InVela)
        );
        assert_eq!(
            default_venue("sign.example.com", ["http://localhost:1/", OWN]),
            Some(page(OWN))
        );
        assert_eq!(
            default_venue("sign.example.com", ["http://localhost:1/"]),
            None
        );
    }

    /// R3: ceremonies run in the app for `getvela.app`, on the page otherwise.
    #[test]
    fn a_ceremony_runs_on_a_page_only_for_a_custom_domain() {
        assert_eq!(ceremony_page(None), None);
        assert_eq!(ceremony_page(Some("https://sign.getvela.app/")), None);
        assert_eq!(ceremony_page(Some("https://other.getvela.app")), None);
        assert_eq!(
            ceremony_page(Some("sign.example.com")).as_deref(),
            Some(OWN)
        );
        assert_eq!(
            ceremony_page(Some("http://localhost:8140")).as_deref(),
            Some("http://localhost:8140/")
        );
        assert_eq!(ceremony_page(Some("http://example.com")), None, "insecure");
    }

    /// R4: transactions and messages follow the venue; ceremonies follow R3.
    #[test]
    fn the_venue_applies_to_transactions_and_messages() {
        let official = SigningVenue::official();
        for kind in [
            SignatureKind::UserOperation,
            SignatureKind::PersonalSign,
            SignatureKind::TypedData,
        ] {
            assert_eq!(venue_for(kind, APP_DOMAIN, &official), official);
            assert_eq!(
                venue_for(kind, APP_DOMAIN, &SigningVenue::InVela),
                SigningVenue::InVela
            );
        }
        assert_eq!(
            venue_for(SignatureKind::KeyCeremony, APP_DOMAIN, &official),
            SigningVenue::InVela
        );
        assert_eq!(
            venue_for(SignatureKind::KeyCeremony, "sign.example.com", &page(OWN)),
            page(OWN)
        );
    }

    #[test]
    fn a_route_names_its_place_for_the_browser() {
        let route = KeyRoute::new("ab", "security_key", "usb,nfc,ble");
        assert_eq!(route.hints, ["security-key"]);
        assert_eq!(
            KeyRoute::new("ab", "platform", "internal").hints,
            ["client-device"]
        );
        assert_eq!(KeyRoute::new("ab", "hybrid", "hybrid").hints, ["hybrid"]);
        assert!(KeyRoute::new("ab", "trusted_signer", "").hints.is_empty());
    }

    #[test]
    fn a_venue_round_trips_on_the_wire() {
        let json = serde_json::to_string(&page(OWN)).unwrap_or_default();
        assert_eq!(json, r#"{"type":"page","url":"https://sign.example.com/"}"#);
        assert_eq!(
            serde_json::to_string(&SigningVenue::InVela).unwrap_or_default(),
            r#"{"type":"in_vela"}"#
        );
        let back: SigningVenue =
            serde_json::from_str(&json).unwrap_or_else(|_| unreachable!("reads"));
        assert_eq!(back, page(OWN));
    }
}
