//! Whether the signer page may be opened at all (spec 076, phase A).
//!
//! The Trusted Signer's whole claim is **you see what you sign**, and that rests
//! on the page being the page it is supposed to be. Spec 075 cut every
//! cross-device channel for this reason: a page on somebody else's device is
//! one this device never fetched, and what it never fetched it cannot check.
//! The remaining channel — the page in this device's own browser — is the only
//! one where the check is possible.
//!
//! This module is the DECISION, and nothing else. It does not fetch, hash,
//! open a WebView or touch the clock; the shells do that (076 phase C) and
//! bring the answer here. One decision for every wallet, so iOS, Android and
//! the desktop cannot come to different conclusions about the same bytes.
//!
//! The ordering, which is the whole of it:
//!
//! 1. the check did not complete → **nothing opens** (FR-006: a check that
//!    cannot complete is a check that failed);
//! 2. the hash is on this device's deny-list → **nothing opens**, whatever
//!    else says otherwise (FR-010: deny wins);
//! 3. the hash is one this build ships, or one this person trusted → open;
//! 4. an unknown hash on a CUSTOM address → ask the person once (FR-009);
//! 5. an unknown hash on the OFFICIAL address → refuse, with both hashes
//!    named. There is nothing to ask: nobody can vouch for bytes
//!    `sign.getvela.app` was not supposed to serve.
//!
//! **What this does not claim.** It catches a replaced build — a hijacked
//! bucket, a bad CDN config, a poisoned release — which is the realistic
//! attack and hits everyone at once. It does NOT catch a server that serves
//! bad bytes only to the real navigation or only to one victim; only the
//! page's own Service Worker (FR-008) answers that. So the words a shell puts
//! on [`Verdict::Refused`] are "this page does not match the published list",
//! never "a distribution attack was prevented" (spec 076, threat model).
//!
//! # Checking, not yet refusing
//!
//! [`BUILD_ALLOWED`] lists the published pages and the shells run this check
//! on every launch, but [`ENFORCE`] is still `false`: a verdict is recorded
//! and shown, and the page opens either way. The two are deliberately
//! separate, because the commit that lists a hash must not be the commit that
//! starts turning people away — see [`ENFORCE`] for why, and the tripwire test
//! that makes flipping it a deliberate act.
//!
//! The ordering of [`BUILD_ALLOWED`] is load-bearing: [`choose_version`] takes
//! the first entry the endpoint still serves, so the front of the list is what
//! a wallet opens. A new page goes to the front and the older ones stay, so a
//! wallet that shipped before the new page still has something to open.

/// The page hashes THIS BUILD accepts (FR-001), lowercase sha256 hex over the
/// page's whole bytes.
///
/// **It can shrink.** "一直累加" is the wrong invariant: a version found to be
/// compromised must be removable, or every client already shipped would accept
/// a replay of it for ever. This is *this build's accepted set* — usually
/// growing, occasionally losing an entry.
///
/// No chain is read. The trust root is the app binary, which the person
/// already trusts completely, and it works offline.
///
/// Empty until 076 phase B publishes the first content-addressed page. See the
/// module note: enforcing an empty set opens nothing.
pub const BUILD_ALLOWED: &[&str] = &[
    // NEWEST FIRST. `choose_version` walks this order and takes the first one
    // the endpoint still serves, so whatever stands at the front is what a
    // wallet opens when several versions are published.
    //
    // Every entry is built from `app-web/trusted-signer/src/` by
    // `bun samples/build-single.mjs`, reproducibly, under both Bun and Node;
    // the bytes are committed at
    // `app-web/trusted-signer/dist/b/<this hash>/sign.html`.

    // The create ceremony works over the custom scheme, and the card names
    // where the answer goes instead of borrowing an identity it cannot check.
    // Until this page, `walletRequester` vouched only for the socket channels —
    // so with those retired, creating a wallet through the page was refused
    // outright (owner, 2026-09-24: 「创建钱包时，滑动签名不可用」).
    "4563215b962d9b615c5dc93bb8121ae02aba6d6b3c20b62e9146d69f03b1e505",
    // The page renamed to Trusted Signer. Its own words changed, so its bytes
    // did, so its hash did — which is the discipline working rather than a
    // nuisance: the page cannot be edited without saying so here.
    "802b2fced1eb1fdcd4d935da15b62ac3d9ba13cfafdf6768a67d5e1296a3e0b6",
    // The first published page (076 phase B). Kept, not replaced: it is still
    // served, and a wallet that ships knowing only this hash has to go on
    // working after the newer page appears.
    "810db7c5dbb461897287ae6e8a8c863e9075779e96a07af06efff98fbd925178",
];

/// Whether a shell may REFUSE on this module's verdict, as opposed to merely
/// recording it.
///
/// Deliberately separate from [`BUILD_ALLOWED`] being non-empty. The two
/// answer different questions — "which bytes would we accept" and "may we stop
/// a person yet" — and tying them together means the commit that lists the
/// first hash also, silently, starts refusing every page that is not yet
/// published. That is a self-inflicted outage in the shape of a security
/// feature.
///
/// Flip this in the commit that publishes the page at the official address,
/// and not before. Until then the shells check, log and open.
pub const ENFORCE: bool = false;

/// The official page's host. A person may point Settings at their own
/// deployment; that address is "custom" here, and the rules differ (FR-009).
const OFFICIAL_HOST: &str = "sign.getvela.app";

/// Why the check could not be completed. Every one of these fails closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CheckFailure {
    /// The page could not be reached at all.
    Unreachable,
    /// The bytes could not be read back from the navigation's own cache.
    ///
    /// A server that sends `Cache-Control: no-store` produces exactly this,
    /// which is what a server defeating the check would do (FR-006, SC-002).
    NoCachedBytes,
    /// The check has not been run yet for this address.
    NotChecked,
}

/// What a shell should do about this page.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Open it: the bytes are a version this device decided on in advance.
    Open,
    /// Open it, and say plainly on the signing screen that these bytes were
    /// NOT checked (FR-009).
    ///
    /// Only ever reachable for a custom address whose owner turned
    /// verification off. The warning is **standing**, not a dialog dismissed
    /// once, because the state outlives the dismissal — which is why this is a
    /// separate verdict and not [`Verdict::Open`] with a flag a shell can
    /// forget to read.
    OpenUnverified,
    /// Never. This hash is on the person's deny-list (FR-010).
    Denied {
        /// The hash that is blocked, as it will be shown.
        hash: String,
    },
    /// The bytes are not a version this device accepts, and there is nobody to
    /// ask: it is the official address.
    Refused {
        /// What the page served.
        actual: String,
        /// What this build would have accepted. May be empty — see
        /// [`BUILD_ALLOWED`].
        expected: Vec<String>,
    },
    /// A custom address serving a version this device has not decided about.
    /// Ask the person once; yes adds it to their allow-list (FR-009).
    AskToTrust {
        /// The hash to show them. They cannot trust what they cannot see.
        actual: String,
    },
    /// The check did not complete, so nothing opens (FR-006).
    CouldNotCheck(CheckFailure),
    /// There was no version to even ask for. Not a failure of the check — the
    /// check never started — and the two reasons need different words.
    NoVersionToAsk(NoVersion),
}

/// Everything the decision is made from. All of it is already known to the
/// shell when it is about to open the page.
#[derive(Clone, Copy, Debug)]
pub struct Page<'a> {
    /// The address, as [`super::signer_url`] normalised it.
    pub url: &'a str,
    /// The hash the check produced, or `None` when it could not complete.
    pub observed: Option<&'a str>,
    /// Why, when `observed` is `None`.
    pub failure: CheckFailure,
    /// Hashes this person trusted on this device (FR-009). Per device: the
    /// `vela.` preferences do not sync, and the UI says so.
    pub trusted: &'a [String],
    /// Hashes this person blocked on this device (FR-010). Deny wins.
    pub blocked: &'a [String],
    /// Verification turned off. Honoured ONLY for a custom address: there is
    /// no legitimate reason to accept unverified bytes from the official page,
    /// and an off switch there is a social-engineering door ("just turn
    /// verification off to fix that error").
    pub verification_off: bool,
}

/// The hash of the bytes a page was served as, lowercase hex.
///
/// In the core so that every shell agrees on what "the page's hash" means:
/// sha256 over the response body exactly as received, with no normalisation —
/// no trimming, no re-encoding, no line-ending fixes. A page is its bytes, and
/// anything that "helpfully" tidies them would make the published hash
/// unreproducible.
///
/// The shells fetch, this hashes, [`decide`] rules.
#[must_use]
pub fn hash_page(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(bytes);
    digest.iter().fold(String::with_capacity(64), |mut out, b| {
        use core::fmt::Write as _;
        let _ = write!(out, "{b:02x}");
        out
    })
}

/// A sha256 hex string as this module compares them: lowercase, 64 hex
/// characters, or `None` for anything else.
///
/// Comparison is on the normalised form, so a hash a person pasted in
/// uppercase blocks the same version as one this build ships.
#[must_use]
pub fn normalize_hash(input: &str) -> Option<String> {
    let text = input.trim();
    if text.len() != 64 || !text.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some(text.to_ascii_lowercase())
}

/// Whether this address is the page Vela publishes, as opposed to one the
/// person deployed themselves.
///
/// Host only, and case-insensitively: a path, a port or a query does not make
/// `sign.getvela.app` somebody else's, and a subdomain of it is NOT it —
/// `sign.getvela.app.evil.test` must not inherit the official page's rules.
#[must_use]
pub fn is_official(url: &str) -> bool {
    let rest = url.trim().split_once("://").map_or(url.trim(), |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = host.split_once(':').map_or(host, |(h, _)| h);
    host.eq_ignore_ascii_case(OFFICIAL_HOST)
}

/// The content-addressed URL for a known version (FR-002).
///
/// `https://sign.getvela.app/b/<sha256>/sign.html` — the client asks for a
/// version it already knows, so there is no "which version is current" to get
/// wrong, a mismatch is attributable and reproducible by anyone, and old
/// clients keep working because the server keeps every published version.
///
/// It works for ANY address, not just the official one: content addressing is a
/// property of the DEPLOYMENT — the `dist/` directory laid out as an index plus
/// `b/<sha256>/sign.html` — and someone who deploys that directory on their own
/// host has the same layout and deserves the same check. Limiting it to one
/// hostname would mean the self-hoster's bytes were never fetched by hash, and
/// the path everyone else relies on would go untested on any machine but the
/// official one.
///
/// `None` only when the hash is not a hash. A deployment that serves a single
/// page and no `b/` directory is handled by the caller falling back to the
/// address as typed.
#[must_use]
pub fn content_addressed_url(url: &str, hash: &str) -> Option<String> {
    let hash = normalize_hash(hash)?;
    let base = url.trim().trim_end_matches('/');
    if base.is_empty() {
        return None;
    }
    Some(format!("{base}/b/{hash}/sign.html"))
}

/// Which published version to ask for, given what the endpoint still has.
///
/// The page is content-addressed (`/b/<sha256>/sign.html`), so a client asks
/// for a version it already knows — but it cannot know which of them the
/// server still keeps. An index at a well-known path says what is available
/// (owner, 2026-09-23: 「需要有一个索引不然的话,不知道端点支持哪些版」).
///
/// **The index narrows; it never chooses.** The candidates are this build's
/// set and then the person's own trusted hashes, IN THAT ORDER, and the first
/// one the endpoint still serves wins. So a lying index can do exactly two
/// things: hide versions (a refusal — loud, and fail-closed), or list versions
/// this wallet does not trust (ignored entirely). It cannot steer a person on
/// to a particular trusted version, and it certainly cannot introduce one.
/// That is why the index needs no authority of its own and is not signed.
///
/// `None` means the endpoint serves nothing this wallet accepts. That is
/// "update the wallet", not "you are under attack" — and with content
/// addressing it should not normally happen at all, because a published path
/// never goes away.
///
/// The index is an OPTIMISATION, not a dependency: a shell that cannot fetch
/// it may try its own preferred hash directly, since the URL is derivable from
/// the hash alone.
pub fn choose_version(
    available: &[String],
    trusted: &[String],
    blocked: &[String],
) -> Result<String, NoVersion> {
    let offered: Vec<String> = available.iter().filter_map(|h| normalize_hash(h)).collect();
    let candidates = || {
        BUILD_ALLOWED
            .iter()
            .filter_map(|h| normalize_hash(h))
            .chain(trusted.iter().filter_map(|h| normalize_hash(h)))
            .filter(|candidate| offered.iter().any(|offer| offer == candidate))
    };
    if let Some(usable) = candidates().find(|candidate| !contains(blocked, candidate)) {
        return Ok(usable);
    }
    // Nothing to ask for — and WHY decides what a person is told. "Could not
    // check" reads as a network fault and sends them debugging the wrong
    // thing; "you have blocked every version this wallet can use" is both true
    // and actionable. Found by driving the real chain over a LAN address.
    if candidates().next().is_some() {
        Err(NoVersion::EverythingUsableIsBlocked)
    } else {
        Err(NoVersion::NothingPublishedThisWalletKnows)
    }
}

/// Why there was no version to ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoVersion {
    /// The endpoint publishes nothing this wallet accepts. "Update the wallet",
    /// not "you are under attack" — and with content addressing it should not
    /// normally happen, because a published path never goes away.
    NothingPublishedThisWalletKnows,
    /// Every version that WOULD have been usable is on this device's deny-list.
    /// The person did that, and can undo it.
    EverythingUsableIsBlocked,
}

/// Whether the page may be opened, and what to say when it may not.
///
/// See the module documentation for the ordering; it is the whole design.
#[must_use]
pub fn decide(page: &Page<'_>) -> Verdict {
    // 1 · A check that cannot complete is a check that failed (FR-006). Before
    //     everything else, because an unreadable page is not a trusted one.
    let Some(observed) = page.observed.and_then(normalize_hash) else {
        return Verdict::CouldNotCheck(page.failure);
    };

    // 2 · Deny outranks everything — this build's set and the person's own
    //     allow-list included (FR-010). It is the answer to "version X was
    //     just found compromised" without waiting for an app release.
    if contains(page.blocked, &observed) {
        return Verdict::Denied { hash: observed };
    }

    // 3 · Decided in advance: by whoever shipped this build, or by this person
    //     on this device.
    if contains(BUILD_ALLOWED, &observed) || contains(page.trusted, &observed) {
        return Verdict::Open;
    }

    if is_official(page.url) {
        // 5 · Nobody can vouch for bytes the official page was not supposed to
        //     serve, so there is nothing to ask and no switch to turn off.
        //     `verification_off` is deliberately not read here.
        return Verdict::Refused {
            actual: observed,
            expected: BUILD_ALLOWED.iter().map(|h| (*h).to_owned()).collect(),
        };
    }

    // A custom address. The person chose it, so they are the one who can
    // vouch for it — either once, now, or standingly by turning the check off.
    if page.verification_off {
        Verdict::OpenUnverified
    } else {
        // 4 · Ask once. Yes adds it to their allow-list, and the invariant
        //     holds: the wallet only ever runs bytes whose hash was decided in
        //     advance. The self-hoster is protected too — their own server
        //     being compromised still changes the bytes.
        Verdict::AskToTrust { actual: observed }
    }
}

/// `true` when `list` holds `hash`, comparing normalised forms so a stored
/// entry in another case still matches.
fn contains<S: AsRef<str>>(list: &[S], hash: &str) -> bool {
    list.iter()
        .filter_map(|entry| normalize_hash(entry.as_ref()))
        .any(|entry| entry == hash)
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: &str = "aa11223344556677889900aabbccddeeff00112233445566778899aabbccddee";
    const B: &str = "bb11223344556677889900aabbccddeeff00112233445566778899aabbccddee";

    fn page<'a>(url: &'a str, observed: Option<&'a str>) -> Page<'a> {
        Page {
            url,
            observed,
            failure: CheckFailure::NotChecked,
            trusted: &[],
            blocked: &[],
            verification_off: false,
        }
    }

    #[test]
    fn a_page_is_its_bytes() {
        // The empty page, and a known vector — so a shell that "helpfully"
        // trims or re-encodes before handing bytes over is caught by the
        // number rather than by a reviewer.
        assert_eq!(
            hash_page(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            hash_page(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Whitespace is part of the page. A trailing newline is a different
        // page, and must hash differently.
        assert_ne!(hash_page(b"<html></html>"), hash_page(b"<html></html>\n"));
        // And what it produces is what `decide` compares.
        assert_eq!(normalize_hash(&hash_page(b"abc")), Some(hash_page(b"abc")));
    }

    #[test]
    fn a_check_that_did_not_complete_opens_nothing() {
        // FR-006. A server sending `Cache-Control: no-store` lands here, and
        // that is exactly what a server defeating this check would do.
        let mut p = page("https://sign.getvela.app/", None);
        p.failure = CheckFailure::NoCachedBytes;
        assert_eq!(
            decide(&p),
            Verdict::CouldNotCheck(CheckFailure::NoCachedBytes)
        );
    }

    #[test]
    fn an_unreadable_hash_is_not_a_hash() {
        // Whatever the shell hands over, only 64 hex characters are a hash.
        for junk in ["", "  ", "not-a-hash", "aa11", &A[..63], &format!("{A}ff")] {
            let p = page("https://sign.getvela.app/", Some(junk));
            assert!(matches!(decide(&p), Verdict::CouldNotCheck(_)), "{junk:?}");
        }
    }

    #[test]
    fn deny_outranks_this_build_and_the_person_alike() {
        // FR-010, the ordering that matters most: a version found compromised
        // stops opening TODAY, without waiting for an app release.
        let blocked = vec![A.to_owned()];
        let trusted = vec![A.to_owned()];
        let mut p = page("https://sign.getvela.app/", Some(A));
        p.blocked = &blocked;
        p.trusted = &trusted;
        assert_eq!(decide(&p), Verdict::Denied { hash: A.to_owned() });
    }

    #[test]
    fn a_blocked_hash_stays_blocked_on_a_custom_address_too() {
        let blocked = vec![A.to_owned()];
        let mut p = page("https://signer.example.test/", Some(A));
        p.blocked = &blocked;
        p.verification_off = true;
        assert!(matches!(decide(&p), Verdict::Denied { .. }));
    }

    #[test]
    fn a_version_this_person_trusted_opens() {
        let trusted = vec![A.to_owned()];
        let mut p = page("https://signer.example.test/", Some(A));
        p.trusted = &trusted;
        assert_eq!(decide(&p), Verdict::Open);
    }

    #[test]
    fn case_does_not_decide_anything() {
        // A hash pasted in uppercase blocks the same version as a lowercase one.
        let blocked = vec![A.to_ascii_uppercase()];
        let shouted = A.to_ascii_uppercase();
        let mut p = page("https://sign.getvela.app/", Some(&shouted));
        p.blocked = &blocked;
        assert!(matches!(decide(&p), Verdict::Denied { .. }));
    }

    #[test]
    fn the_official_page_serving_an_unknown_version_is_refused_with_both_hashes() {
        let p = page("https://sign.getvela.app/", Some(B));
        let verdict = decide(&p);
        assert!(matches!(verdict, Verdict::Refused { .. }), "{verdict:?}");
        if let Verdict::Refused { actual, expected } = verdict {
            assert_eq!(actual, B);
            // SC-001: the screen names what was expected. Empty until phase B
            // publishes, and the shell must say so rather than draw an empty
            // list as if it meant "anything".
            assert_eq!(expected.len(), BUILD_ALLOWED.len());
        }
    }

    #[test]
    fn the_official_page_has_no_off_switch() {
        // FR-009: an off switch on the official address is a social-engineering
        // door — "just turn verification off to fix that error".
        let mut p = page("https://sign.getvela.app/", Some(B));
        p.verification_off = true;
        assert!(matches!(decide(&p), Verdict::Refused { .. }));
    }

    #[test]
    fn a_custom_address_asks_once_rather_than_refusing() {
        // FR-009: the person chose it, so they are the one who can vouch for
        // it. They see the hash, because nobody can trust what they cannot see.
        let p = page("https://signer.example.test/", Some(B));
        assert_eq!(
            decide(&p),
            Verdict::AskToTrust {
                actual: B.to_owned()
            }
        );
    }

    #[test]
    fn a_custom_address_with_the_check_off_opens_but_says_so() {
        // Not `Open`: the verdict itself carries the standing warning, so a
        // shell cannot forget to draw it.
        let mut p = page("https://signer.example.test/", Some(B));
        p.verification_off = true;
        assert_eq!(decide(&p), Verdict::OpenUnverified);
    }

    #[test]
    fn the_official_host_is_that_host_and_not_a_lookalike() {
        assert!(is_official("https://sign.getvela.app/"));
        assert!(is_official("https://SIGN.GetVela.app/b/abc/sign.html"));
        assert!(is_official("https://sign.getvela.app:443/"));
        // The ones that must not inherit the official page's rules.
        assert!(!is_official("https://sign.getvela.app.evil.test/"));
        assert!(!is_official("https://evil.test/?x=sign.getvela.app"));
        assert!(!is_official("https://sign.getvela.app@evil.test/"));
        assert!(!is_official("http://127.0.0.1:8080/"));
    }

    #[test]
    fn a_deployment_is_addressed_by_hash_wherever_it_lives() {
        // FR-002: ask for a version you already know. The `dist/` layout is
        // what makes this possible, and it is the same layout wherever it is
        // copied — so a self-hoster gets the same check, and the path everyone
        // relies on is exercisable on a machine that is not the official one.
        assert_eq!(
            content_addressed_url("https://sign.getvela.app/", &A.to_ascii_uppercase()),
            Some(format!("https://sign.getvela.app/b/{A}/sign.html"))
        );
        assert_eq!(
            content_addressed_url("http://192.168.1.20:8080/", A),
            Some(format!("http://192.168.1.20:8080/b/{A}/sign.html"))
        );
        // A base with a path keeps it: a deployment can live in a subdirectory.
        assert_eq!(
            content_addressed_url("https://example.test/signer", A),
            Some(format!("https://example.test/signer/b/{A}/sign.html"))
        );
        // And a hash that is not a hash addresses nothing.
        assert_eq!(
            content_addressed_url("https://sign.getvela.app/", "nope"),
            None
        );
    }

    #[test]
    fn the_index_narrows_the_choice_and_never_makes_it() {
        // A version only the ENDPOINT knows about is not a version this wallet
        // will run. The index has no authority; it only says what is there.
        let available = vec![B.to_owned()];
        assert_eq!(
            choose_version(&available, &[], &[]),
            Err(NoVersion::NothingPublishedThisWalletKnows)
        );

        // One the person trusted, and the endpoint still has: that is the one.
        let trusted = vec![B.to_owned()];
        assert_eq!(choose_version(&available, &trusted, &[]), Ok(B.to_owned()));

        // Blocked outranks both, here as everywhere (FR-010).
        let blocked = vec![B.to_owned()];
        assert_eq!(
            choose_version(&available, &trusted, &blocked),
            Err(NoVersion::EverythingUsableIsBlocked)
        );
    }

    #[test]
    fn the_endpoint_cannot_steer_which_trusted_version_is_taken() {
        // The person trusts two, in their order; the index lists them in the
        // other order. The CLIENT's order decides.
        let trusted = vec![A.to_owned(), B.to_owned()];
        let available = vec![B.to_owned(), A.to_owned()];
        assert_eq!(choose_version(&available, &trusted, &[]), Ok(A.to_owned()));
    }

    #[test]
    fn an_endpoint_serving_nothing_this_wallet_knows_is_not_an_attack() {
        // It is "update the wallet". Content addressing is what should keep it
        // from happening: a published path never goes away.
        assert_eq!(
            choose_version(&[], &[A.to_owned()], &[]),
            Err(NoVersion::NothingPublishedThisWalletKnows)
        );
        assert_eq!(
            choose_version(&["not-a-hash".to_owned()], &[A.to_owned()], &[]),
            Err(NoVersion::NothingPublishedThisWalletKnows)
        );
    }

    #[test]
    fn the_newest_shipped_page_is_the_one_a_wallet_opens() {
        // The ordering of `BUILD_ALLOWED` is not decoration. An endpoint that
        // serves every version this build knows — which is what `dist/` now
        // holds — must yield the FRONT one, so that publishing a new page
        // actually moves people onto it instead of leaving them on whichever
        // hash happens to sort first.
        let all: Vec<String> = BUILD_ALLOWED.iter().map(|h| (*h).to_owned()).collect();
        let mut shuffled = all.clone();
        shuffled.reverse();
        for offered in [&all, &shuffled] {
            assert_eq!(
                choose_version(offered, &[], &[]).as_deref(),
                Ok(BUILD_ALLOWED[0]),
                "the endpoint's order must not decide which version is taken"
            );
        }
    }

    #[test]
    fn every_shipped_page_is_committed_at_its_own_path() {
        // A listed hash a person cannot actually fetch is a brick: `decide`
        // would refuse the only page the endpoint serves. The repository is
        // what the owner deploys, so the check is that `dist/` holds each
        // listed version, and that the bytes there hash to their own name.
        let dist = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../app-web/trusted-signer/dist/b");
        for hash in BUILD_ALLOWED {
            let path = dist.join(hash).join("sign.html");
            let bytes = std::fs::read(&path).unwrap_or_default();
            assert!(
                !bytes.is_empty(),
                "{} is listed but not published",
                path.display()
            );
            assert_eq!(
                hash_page(&bytes),
                *hash,
                "the bytes at {} do not hash to their own name",
                path.display()
            );
        }
    }

    #[test]
    fn every_shipped_hash_is_a_hash() {
        assert!(
            !BUILD_ALLOWED.is_empty(),
            "a build that accepts nothing opens nothing"
        );
        for hash in BUILD_ALLOWED {
            assert_eq!(
                normalize_hash(hash).as_deref(),
                Some(*hash),
                "{hash} is not 64 lowercase hex characters"
            );
        }
    }

    #[test]
    // The assertion IS on a constant, and that is the point: this is a tripwire
    // on `ENFORCE`, so that turning refusals on is a deliberate act with a
    // failing test attached rather than a one-character edit nobody reviews.
    #[allow(clippy::assertions_on_constants)]
    fn refusing_is_still_switched_off() {
        // The guard against a self-inflicted outage: listing a hash must not,
        // by itself, start refusing pages that are not published yet. Flip
        // `ENFORCE` in the commit that publishes, and change this test with it.
        assert!(
            !ENFORCE,
            "076: enforcement is on — the page must be published at the official \
             address first, and every listed hash reachable at its own path"
        );
    }
}
