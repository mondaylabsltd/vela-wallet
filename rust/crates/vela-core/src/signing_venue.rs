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
//!   lives, so the browser goes straight to it instead of asking again; and
//!   the person is told which key they will confirm with ([`KeyLabel`]).
//! - **R7** [`signing_route`] — the ONE rule every shell follows to send a
//!   signature somewhere: per account plan and [`SignatureKind`], Vela's own
//!   sheet, the account's page, or nowhere and why. The same answer on every
//!   surface — the apps, the web wallet and the Chrome extension (owner,
//!   2026-10-30: the page is usable everywhere, superseding the 2026-09-23
//!   "the web opens no signing page" and its 2026-10-29 multisig-only
//!   exception, spec 102 plan P2-11 / D-16). A shell that cannot carry a
//!   kind to the page refuses; it never falls back to its own sheet.
//! - **R8** [`SigningSurface`] — the one input R3 and R7 take about WHERE the
//!   shell runs: a surface whose own pages cannot use `getvela.app` passkeys
//!   (the Safari Web Extension — WebKit refuses the RP ID in an extension
//!   page, measured on iOS 26) sends every signature and every key ceremony
//!   to a page ([`signing_route_on`], [`ceremony_page_on`]). Every other
//!   surface answers exactly as R3 and R7 always did.
//!
//! What this does NOT claim: the domain is shared (D2), so a page as venue
//! assures *what you see is what you sign* against a tampered app interface;
//! it does not stop a fully compromised wallet on the same passkey domain
//! (`getvela.app`) from asking the passkey for a signature itself.
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
    /// RETIRED 2026-10-30: the web wallet once opened no signing page for a
    /// transaction or a message (owner, 2026-09-23), and a custom-domain
    /// account was refused there with this. The owner then made the page
    /// usable everywhere ([`signing_route`]), so no rule in this build gives
    /// it. Kept on the wire because shells built before still decode and
    /// word it (`settings.venue.blockedWeb`).
    NotOnWeb,
}

impl VenueBlock {
    /// The corpus key of the reason line: `{{domain}}` for
    /// [`Self::AppCannotReach`], `{{pageDomain}}` and `{{domain}}` for
    /// [`Self::PageOnOtherDomain`], nothing for [`Self::NotOnWeb`].
    #[must_use]
    pub const fn key(&self) -> &'static str {
        match self {
            Self::AppCannotReach { .. } => "settings.venue.blockedApp",
            Self::PageOnOtherDomain { .. } => "settings.venue.blockedPage",
            Self::NotOnWeb => "settings.venue.blockedWeb",
        }
    }

    /// The values [`Self::key`]'s line takes, by the corpus's variable names:
    /// `domain` and, for a page on another domain, `pageDomain`. With
    /// [`Self::key`] this is the whole sentence, so no shell restates which
    /// fact fills which placeholder.
    #[must_use]
    pub fn vars(&self) -> Vec<(&'static str, String)> {
        match self {
            Self::AppCannotReach { domain } => vec![("domain", domain.clone())],
            Self::PageOnOtherDomain {
                page_domain,
                domain,
            } => vec![
                ("pageDomain", page_domain.clone()),
                ("domain", domain.clone()),
            ],
            Self::NotOnWeb => Vec::new(),
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
    /// Versions of THIS page the person trusted on this device ("Trust this
    /// version?", spec 076 FR-009): lowercase sha256 hex. What a shell passes
    /// as `trusted` when it checks this page — and only this page: trusting a
    /// self-hoster's build vouches for that deployment, not for every page
    /// that might serve the same bytes. Never set for the official page (the
    /// official page is never stored, and nothing is ever asked about it).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trusted: Vec<String>,
}

impl SigningPage {
    /// A saved page at `url` (normalised by the caller) with no trust yet.
    #[must_use]
    pub fn new(url: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            name: name.into(),
            trusted: Vec::new(),
        }
    }
}

/// The versions the person trusted for the page at `url` on this device —
/// what a shell passes as `trusted` to `launch::target` and `launch::admit`
/// when it checks that page. Empty for the official page and for a page that
/// is not saved.
#[must_use]
pub fn trusted_versions(saved: &[SigningPage], url: &str) -> Vec<String> {
    if trusted_signer::integrity::is_official(url) {
        return Vec::new();
    }
    saved
        .iter()
        .find(|page| same_page(&page.url, url))
        .map(|page| page.trusted.clone())
        .unwrap_or_default()
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

/// What is being signed, as far as the venue rule cares (R4, R7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SignatureKind {
    /// One call as a user operation: a site's `eth_sendTransaction`.
    UserOperation,
    /// Several calls in one user operation: a site's `wallet_sendCalls`
    /// (EIP-5792), the wallet's own send (no site asked: its calls are handed
    /// over as a batch, [`trusted_signer::own_send_params`]), and a multisig
    /// executor's own operation ([`MULTISIG_EXECUTE_METHOD`]).
    Batch,
    /// `personal_sign`.
    PersonalSign,
    /// `eth_signTypedData_v4` (and its v3 / v1 spellings).
    TypedData,
    /// A multisig member's approval of a SafeTx (`vela_multisigApprove`,
    /// [`trusted_signer::approval`]).
    MultisigApproval,
    /// Create, add a key, sign in, a member or recovery proof — a challenge
    /// that is random or derived, with nothing to preview.
    KeyCeremony,
}

/// The method a multisig executor's own operation is asked under: the
/// executor's account calls `execTransaction` on the multisig.
pub const MULTISIG_EXECUTE_METHOD: &str = "vela_multisigExecute";

/// The method a member's owner proof is asked under when a multisig is
/// created: the genesis service's sponsor challenge for the multisig's
/// address — derived, moving nothing, with nothing to preview. A ceremony
/// (R3): in Vela for a `getvela.app` account.
pub const MULTISIG_CREATE_METHOD: &str = "vela_multisigCreate";

impl SignatureKind {
    /// The kind of a request by its JSON-RPC method — `""` is the wallet's own
    /// send. `None` for a method no venue signs: `eth_sign` (a bare digest,
    /// blind by construction, refused everywhere) and anything unknown.
    #[must_use]
    pub fn of_method(method: &str) -> Option<Self> {
        match method {
            "eth_sendTransaction" => Some(Self::UserOperation),
            "" | "wallet_sendCalls" | MULTISIG_EXECUTE_METHOD => Some(Self::Batch),
            "personal_sign" => Some(Self::PersonalSign),
            "eth_signTypedData_v4" | "eth_signTypedData_v3" | "eth_signTypedData" => {
                Some(Self::TypedData)
            }
            trusted_signer::approval::METHOD => Some(Self::MultisigApproval),
            MULTISIG_CREATE_METHOD => Some(Self::KeyCeremony),
            _ => None,
        }
    }
}

/// R4: where a signature of `kind` goes for an account on `domain` whose venue
/// is `venue`. Transactions, batches, messages and multisig approvals follow
/// the venue; a key ceremony runs in the app for a `getvela.app` account and
/// on the account's page otherwise (R3).
#[must_use]
pub fn venue_for(kind: SignatureKind, domain: &str, venue: &SigningVenue) -> SigningVenue {
    match kind {
        SignatureKind::UserOperation
        | SignatureKind::Batch
        | SignatureKind::PersonalSign
        | SignatureKind::TypedData
        | SignatureKind::MultisigApproval => venue.clone(),
        SignatureKind::KeyCeremony if is_app_domain(domain) => SigningVenue::InVela,
        SignatureKind::KeyCeremony => venue.clone(),
    }
}

/// Where one signature goes (R7). Its JSON is `{type: 'in_vela' | 'page' |
/// 'blocked', url?, block?}`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SigningRoute {
    /// Vela's own sheet and the account's own passkey prompt.
    InVela,
    /// The account's trusted signing page, at its base address: the shell
    /// checks it (R6), opens it, hands it the request, and accepts only an
    /// answer the core verifies ([`trusted_signer::accept_signature`],
    /// [`trusted_signer::approval::accept`]).
    Page { url: String },
    /// Nothing on this device can sign for the account, and why.
    Blocked { block: VenueBlock },
}

/// R7 — where a signature of `kind` by an account with `plan` goes, on every
/// surface alike: the apps, the web wallet and the Chrome extension (owner,
/// 2026-10-30).
///
/// - a blocked plan (a custom-domain account whose page is unknown here) →
///   blocked, with the plan's reason, whatever the kind;
/// - otherwise the venue [`venue_for`] gives. For a transaction, a batch, a
///   message or a multisig approval that is the venue the person chose
///   (already checked against R1 by the account's reader), so a person who
///   chose the page reviews every one of them there, independently of the
///   wallet's code (the owner's Bybit lesson); a key ceremony follows R3.
#[must_use]
pub fn signing_route(plan: &SigningPlan, kind: SignatureKind) -> SigningRoute {
    if let Some(block) = &plan.blocked {
        return SigningRoute::Blocked {
            block: block.clone(),
        };
    }
    match venue_for(kind, &plan.domain, &plan.venue) {
        SigningVenue::InVela => SigningRoute::InVela,
        SigningVenue::Page { url } => SigningRoute::Page { url },
    }
}

/// [`signing_route`] for a request's JSON-RPC `method` (`""`: the wallet's
/// own send) — `None` for a method no venue signs
/// ([`SignatureKind::of_method`]), which the shell refuses.
#[must_use]
pub fn signing_route_for_method(plan: &SigningPlan, method: &str) -> Option<SigningRoute> {
    signing_route_for_method_on(plan, method, SigningSurface::Standard)
}

// ---------------------------------------------------------------------------
// R8 — the surface
// ---------------------------------------------------------------------------

/// Where the shell asking runs, as far as passkeys decide (R8). Its JSON is
/// `"standard"` | `"safari_extension"`.
///
/// Not a list of platforms: the one fact that changes a route is whether the
/// shell's OWN page can run a `getvela.app` passkey ceremony. Every surface
/// Vela ships can, except one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SigningSurface {
    /// The apps (bound to `getvela.app` by their associated domain), the web
    /// wallet (`wallet.getvela.app`, `getvela.app`) and the Chrome extension
    /// (its host permission): Vela's own sheet reaches `getvela.app` keys.
    #[default]
    Standard,
    /// The Safari Web Extension (iOS and macOS). Its pages are
    /// `safari-web-extension://<UUID>`, and WebKit refuses them a
    /// `getvela.app` passkey (`SecurityError: The requested RPID did not
    /// match the origin`, measured on iOS 26.2) — for creating a key as for
    /// using one. So nothing can be signed, and no key made or used, in
    /// Vela's own sheet there: every signature and every ceremony goes to a
    /// page, which is a real `https` origin of the key's domain.
    SafariExtension,
}

impl SigningSurface {
    /// Can this surface's own pages run a `getvela.app` passkey ceremony?
    #[must_use]
    pub const fn passkeys_in_wallet(self) -> bool {
        matches!(self, Self::Standard)
    }

    /// The surface a shell names by its wire word (`"standard"`,
    /// `"safari_extension"`); anything else — an absent word, an older shell
    /// — is [`Self::Standard`], the answer every surface had before R8.
    #[must_use]
    pub fn of_name(name: Option<&str>) -> Self {
        match name.map(str::trim) {
            Some("safari_extension") => Self::SafariExtension,
            _ => Self::Standard,
        }
    }
}

/// The page that runs what a surface without its own passkeys cannot (R8),
/// for an account whose venue is `venue`: the venue itself when it is a page
/// (whatever its domain — R1 already checked that it reaches the account's
/// keys), else the official page, which reaches every `getvela.app` key.
fn page_for(venue: &SigningVenue) -> String {
    venue
        .page_url()
        .and_then(|url| trusted_signer::signer_url(url).ok())
        .unwrap_or_else(|| trusted_signer::DEFAULT_SIGNER_URL.to_owned())
}

/// R7 on `surface` (R8). On a surface whose pages run `getvela.app` passkeys
/// this is [`signing_route`], unchanged. On one that cannot (the Safari Web
/// Extension), Vela's own sheet is never the answer — for any kind, a key
/// ceremony included: what R7 would sign in Vela goes to the account's page,
/// or, for an account that signs in Vela, to the official page (its keys are
/// `getvela.app` keys, which the official page reaches). A blocked plan stays
/// blocked, and a page stays that page.
#[must_use]
pub fn signing_route_on(
    plan: &SigningPlan,
    kind: SignatureKind,
    surface: SigningSurface,
) -> SigningRoute {
    match signing_route(plan, kind) {
        SigningRoute::InVela if !surface.passkeys_in_wallet() => SigningRoute::Page {
            url: page_for(&plan.venue),
        },
        route => route,
    }
}

/// [`signing_route_for_method`] on `surface` (R8).
#[must_use]
pub fn signing_route_for_method_on(
    plan: &SigningPlan,
    method: &str,
    surface: SigningSurface,
) -> Option<SigningRoute> {
    SignatureKind::of_method(method).map(|kind| signing_route_on(plan, kind, surface))
}

/// R3 on `surface` (R8): the page a key ceremony runs on when the person's
/// chosen page is `page`. On a surface whose pages run `getvela.app`
/// passkeys this is [`ceremony_page`], unchanged — the app for `getvela.app`,
/// the page for a custom domain. On one that cannot (the Safari Web
/// Extension) every ceremony runs on a page: the chosen one when it is one a
/// browser would sign on, else the official page — so creating a wallet,
/// adding a key, signing in and every proof there mint and use `getvela.app`
/// keys on `sign.getvela.app`, the same keys the apps use.
#[must_use]
pub fn ceremony_page_on(page: Option<&str>, surface: SigningSurface) -> Option<String> {
    if surface.passkeys_in_wallet() {
        return ceremony_page(page);
    }
    Some(
        page.and_then(|page| trusted_signer::signer_url(page).ok())
            .unwrap_or_else(|| trusted_signer::DEFAULT_SIGNER_URL.to_owned()),
    )
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

/// The corpus key of a key place's title — "This device", "Phone or tablet",
/// "USB security key" — by its wire name. The same keys the choosers' rows
/// carry (`app::method_words`); anything else reads as this device, the place
/// a key with no record of where it lives was always looked for.
#[must_use]
pub fn place_title_key(method: &str) -> &'static str {
    match method {
        "hybrid" => "onboarding.create.methodHybridTitle",
        "security_key" => "onboarding.create.methodSecurityKeyTitle",
        _ => "onboarding.create.methodPlatformTitle",
    }
}

/// The name of the key a person confirms with — the hand-off card's
/// 「确认方式 | 这台设备」 / "Confirm with | This device" row, and wherever else
/// a signature names its key (spec 102). A row, as the signing page draws
/// it, not a sentence: no locale has to inflect a place's title inside one.
///
/// The key's own label when the person gave it one that is not the wallet's
/// name; otherwise the place it lives. A founding key is labelled with the
/// wallet's name, and the card already has a "Signing account" row that says
/// it — "Confirm with | Savings" under "Signing account | Savings" names nothing
/// new, where "Confirm with | Phone or tablet" tells the person which device to
/// reach for.
///
/// The shell draws the row's label from `label_key` and its value from
/// `name` when it is set, else the translation of `place_key`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct KeyLabel {
    /// The key's own label, drawn as it is. `None` when it has none, or its
    /// label is the wallet's name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The corpus key of the key's place — always set, so a shell has a
    /// caption (and an icon) even when it draws `name`.
    pub place_key: String,
    /// The corpus key of the row's label: [`CONFIRM_WITH_LABEL`] ("Confirm
    /// with") for a key that confirms a signature, a sign-in or a proof;
    /// [`NEW_KEY_ON_LABEL`] ("New key on") for a key a ceremony is making.
    /// The signing page's own `field.confirmWith` / `field.keyOn`.
    #[serde(default = "confirm_with_label")]
    pub label_key: String,
}

/// "Confirm with" — the key row's label for a key that confirms something.
pub const CONFIRM_WITH_LABEL: &str = "componentsUi.signing.confirmWithLabel";
/// "New key on" — the key row's label while a ceremony makes the key.
pub const NEW_KEY_ON_LABEL: &str = "componentsUi.signing.newKeyOnLabel";

fn confirm_with_label() -> String {
    CONFIRM_WITH_LABEL.to_owned()
}

impl Default for KeyLabel {
    fn default() -> Self {
        Self::of(None, "", "platform")
    }
}

impl KeyLabel {
    /// The label of a key named `key_name` (none, or empty, for a key with no
    /// label) that lives on `method`, in a wallet named `wallet_name`.
    #[must_use]
    pub fn of(key_name: Option<&str>, wallet_name: &str, method: &str) -> Self {
        let wallet = wallet_name.trim().to_lowercase();
        let name = key_name
            .map(str::trim)
            .filter(|name| !name.is_empty() && name.to_lowercase() != wallet)
            .map(str::to_owned);
        Self {
            name,
            place_key: place_title_key(method).to_owned(),
            label_key: confirm_with_label(),
        }
    }

    /// The label of a key a ceremony makes or uses on `method` (spec 102
    /// integration): its place — a ceremony knows no key name worth drawing
    /// (a new key carries the wallet's) — under "New key on" while it is
    /// being made (`making`), "Confirm with" otherwise.
    #[must_use]
    pub fn of_ceremony(method: &str, making: bool) -> Self {
        Self {
            name: None,
            place_key: place_title_key(method).to_owned(),
            label_key: if making {
                NEW_KEY_ON_LABEL
            } else {
                CONFIRM_WITH_LABEL
            }
            .to_owned(),
        }
    }

    /// The words, given the shell's translator for `place_key`.
    #[must_use]
    pub fn text(&self, translate: impl FnOnce(&str) -> String) -> String {
        match &self.name {
            Some(name) => name.clone(),
            None => translate(&self.place_key),
        }
    }
}

/// How an account signs on this device, in one answer: its signing domain,
/// the venue its transactions and messages go to (already checked against R1),
/// the key route (R5), and the name of that key as the person reads it.
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
    /// The key row — "Confirm with | {key}" — the key's name as the person
    /// reads it ([`KeyLabel`]): its own label when that is not the wallet's name, else
    /// its place. Always set: a record with no sign-in key names the place of
    /// its first key.
    #[serde(default)]
    pub key_label: KeyLabel,
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
            SigningPage::new("https://sign.example.com".to_owned(), "Mine".to_owned()),
            SigningPage::new("https://sign.example.com/backup/".to_owned(), String::new()),
            // The official page saved again, and a duplicate: listed once.
            SigningPage::new("https://SIGN.getvela.app".to_owned(), "dup".to_owned()),
            SigningPage::new("https://sign.example.com/".to_owned(), "dup".to_owned()),
            // Not a page at all: never offered.
            SigningPage::new("ftp://x".to_owned(), String::new()),
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
            SignatureKind::Batch,
            SignatureKind::PersonalSign,
            SignatureKind::TypedData,
            SignatureKind::MultisigApproval,
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

    /// R7: one rule, per plan and kind, on every surface (owner, 2026-10-30).
    #[test]
    fn every_kind_follows_the_chosen_venue_and_never_a_blocked_one() {
        let plan = |domain: &str, venue: SigningVenue, blocked: Option<VenueBlock>| SigningPlan {
            domain: domain.to_owned(),
            venue,
            blocked,
            key: None,
            key_label: KeyLabel::default(),
        };
        let signing = [
            SignatureKind::UserOperation,
            SignatureKind::Batch,
            SignatureKind::PersonalSign,
            SignatureKind::TypedData,
            SignatureKind::MultisigApproval,
        ];
        let official_url = trusted_signer::DEFAULT_SIGNER_URL.to_owned();
        for kind in signing {
            // A getvela.app account that chose the official page: the page.
            assert_eq!(
                signing_route(&plan(APP_DOMAIN, SigningVenue::official(), None), kind),
                SigningRoute::Page {
                    url: official_url.clone()
                },
                "{kind:?}"
            );
            // In Vela: Vela's sheet.
            assert_eq!(
                signing_route(&plan(APP_DOMAIN, SigningVenue::InVela, None), kind),
                SigningRoute::InVela,
                "{kind:?}"
            );
            // A custom-domain account: its own page, the only place its keys
            // answer — on the web and in the extension too, now.
            assert_eq!(
                signing_route(&plan("sign.example.com", page(OWN), None), kind),
                SigningRoute::Page {
                    url: OWN.to_owned()
                },
                "{kind:?}"
            );
            // A plan nothing here can carry out: blocked, with its reason.
            let block = VenueBlock::AppCannotReach {
                domain: "sign.example.com".to_owned(),
            };
            assert_eq!(
                signing_route(
                    &plan(
                        "sign.example.com",
                        SigningVenue::InVela,
                        Some(block.clone())
                    ),
                    kind
                ),
                SigningRoute::Blocked { block },
                "{kind:?}"
            );
        }
        // A ceremony still follows R3: in the app for getvela.app.
        assert_eq!(
            signing_route(
                &plan(APP_DOMAIN, SigningVenue::official(), None),
                SignatureKind::KeyCeremony
            ),
            SigningRoute::InVela
        );
        assert_eq!(
            signing_route(
                &plan("sign.example.com", page(OWN), None),
                SignatureKind::KeyCeremony
            ),
            SigningRoute::Page {
                url: OWN.to_owned()
            }
        );
    }

    /// R8: on the Safari Web Extension nothing is signed in Vela — every kind,
    /// a ceremony included, goes to a page; every other surface is R7 as it was.
    #[test]
    fn a_surface_without_its_own_passkeys_sends_everything_to_a_page() {
        let plan = |domain: &str, venue: SigningVenue, blocked: Option<VenueBlock>| SigningPlan {
            domain: domain.to_owned(),
            venue,
            blocked,
            key: None,
            key_label: KeyLabel::default(),
        };
        let every = [
            SignatureKind::UserOperation,
            SignatureKind::Batch,
            SignatureKind::PersonalSign,
            SignatureKind::TypedData,
            SignatureKind::MultisigApproval,
            SignatureKind::KeyCeremony,
        ];
        let official = trusted_signer::DEFAULT_SIGNER_URL.to_owned();
        let staging = page("https://staging.getvela.app/");
        let block = VenueBlock::AppCannotReach {
            domain: "sign.example.com".to_owned(),
        };
        for kind in every {
            for (plan, safari) in [
                // In Vela → the official page, which reaches getvela.app keys.
                (
                    plan(APP_DOMAIN, SigningVenue::InVela, None),
                    SigningRoute::Page {
                        url: official.clone(),
                    },
                ),
                // The official page → the official page (a ceremony too).
                (
                    plan(APP_DOMAIN, SigningVenue::official(), None),
                    SigningRoute::Page {
                        url: official.clone(),
                    },
                ),
                // Another getvela.app page → that page.
                (
                    plan(APP_DOMAIN, staging.clone(), None),
                    SigningRoute::Page {
                        url: "https://staging.getvela.app/".to_owned(),
                    },
                ),
                // A custom domain → its page, as everywhere.
                (
                    plan("sign.example.com", page(OWN), None),
                    SigningRoute::Page {
                        url: OWN.to_owned(),
                    },
                ),
                // Blocked stays blocked.
                (
                    plan(
                        "sign.example.com",
                        SigningVenue::InVela,
                        Some(block.clone()),
                    ),
                    SigningRoute::Blocked {
                        block: block.clone(),
                    },
                ),
            ] {
                assert_eq!(
                    signing_route_on(&plan, kind, SigningSurface::SafariExtension),
                    safari,
                    "{kind:?} {plan:?}"
                );
                assert_eq!(
                    signing_route_on(&plan, kind, SigningSurface::Standard),
                    signing_route(&plan, kind),
                    "{kind:?} {plan:?}: every other surface is unchanged"
                );
            }
        }
        // By method, `eth_sign` is still nobody's.
        let app = plan(APP_DOMAIN, SigningVenue::InVela, None);
        assert_eq!(
            signing_route_for_method_on(&app, "eth_sign", SigningSurface::SafariExtension),
            None
        );
        assert_eq!(
            signing_route_for_method_on(
                &app,
                MULTISIG_CREATE_METHOD,
                SigningSurface::SafariExtension
            ),
            Some(SigningRoute::Page { url: official })
        );
    }

    /// R3 on a surface (R8): the Safari Web Extension runs every ceremony on a
    /// page — the chosen one, else the official page.
    #[test]
    fn a_ceremony_on_a_surface_without_its_own_passkeys_runs_on_a_page() {
        let safari = SigningSurface::SafariExtension;
        assert_eq!(
            ceremony_page_on(None, safari).as_deref(),
            Some("https://sign.getvela.app/")
        );
        assert_eq!(
            ceremony_page_on(Some("https://sign.getvela.app"), safari).as_deref(),
            Some("https://sign.getvela.app/")
        );
        assert_eq!(
            ceremony_page_on(Some("sign.example.com"), safari).as_deref(),
            Some(OWN)
        );
        assert_eq!(
            ceremony_page_on(Some("http://localhost:8140"), safari).as_deref(),
            Some("http://localhost:8140/")
        );
        // Not a page at all: the official one, never Vela's own sheet.
        assert_eq!(
            ceremony_page_on(Some("http://example.com"), safari).as_deref(),
            Some("https://sign.getvela.app/")
        );
        for page in [
            None,
            Some("https://sign.getvela.app/"),
            Some("sign.example.com"),
            Some("http://example.com"),
        ] {
            assert_eq!(
                ceremony_page_on(page, SigningSurface::Standard),
                ceremony_page(page),
                "{page:?}"
            );
        }
    }

    #[test]
    fn a_surface_is_named_on_the_wire() {
        assert_eq!(
            serde_json::to_value(SigningSurface::SafariExtension).unwrap_or_default(),
            serde_json::json!("safari_extension")
        );
        assert_eq!(SigningSurface::default(), SigningSurface::Standard);
        assert_eq!(
            SigningSurface::of_name(Some("safari_extension")),
            SigningSurface::SafariExtension
        );
        for other in [
            None,
            Some(""),
            Some("standard"),
            Some("chrome"),
            Some("SAFARI"),
        ] {
            assert_eq!(
                SigningSurface::of_name(other),
                SigningSurface::Standard,
                "{other:?}"
            );
        }
        assert!(SigningSurface::Standard.passkeys_in_wallet());
        assert!(!SigningSurface::SafariExtension.passkeys_in_wallet());
    }

    /// Every request a shell signs has a kind; `eth_sign` and the unknown none.
    #[test]
    fn a_method_names_its_kind() {
        let cases = [
            ("eth_sendTransaction", Some(SignatureKind::UserOperation)),
            ("wallet_sendCalls", Some(SignatureKind::Batch)),
            ("", Some(SignatureKind::Batch)),
            (MULTISIG_EXECUTE_METHOD, Some(SignatureKind::Batch)),
            ("personal_sign", Some(SignatureKind::PersonalSign)),
            ("eth_signTypedData_v4", Some(SignatureKind::TypedData)),
            ("eth_signTypedData_v3", Some(SignatureKind::TypedData)),
            ("eth_signTypedData", Some(SignatureKind::TypedData)),
            (
                "vela_multisigApprove",
                Some(SignatureKind::MultisigApproval),
            ),
            (MULTISIG_CREATE_METHOD, Some(SignatureKind::KeyCeremony)),
            ("eth_sign", None),
            ("eth_signTransaction", None),
            ("vela_somethingElse", None),
        ];
        for (method, kind) in cases {
            assert_eq!(SignatureKind::of_method(method), kind, "{method}");
        }
        let official = SigningPlan {
            domain: APP_DOMAIN.to_owned(),
            venue: SigningVenue::official(),
            blocked: None,
            key: None,
            key_label: KeyLabel::default(),
        };
        assert_eq!(signing_route_for_method(&official, "eth_sign"), None);
        // A multisig's owner proof is a ceremony: in Vela for a getvela.app
        // account, whatever its venue (found on Arc, round 6: creating a
        // multisig with the page as venue was refused).
        assert_eq!(
            signing_route_for_method(&official, MULTISIG_CREATE_METHOD),
            Some(SigningRoute::InVela)
        );
        assert!(matches!(
            signing_route_for_method(&official, "personal_sign"),
            Some(SigningRoute::Page { .. })
        ));
        assert_eq!(
            serde_json::to_value(SigningRoute::Page {
                url: OWN.to_owned()
            })
            .unwrap_or_default(),
            serde_json::json!({"type": "page", "url": OWN})
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
