# Data model: 102 — Where you review and sign

**Spec**: [spec.md](spec.md) · **Plan**: [plan.md](plan.md) · **Research**: [research.md](research.md)

Everything below is `vela-core` (Phase 1). Shells draw it; none of them decides any of it.
Paths are relative to `rust/crates/vela-core/src/`.

## 1. The two axes and the one property

| | Type | Where it lives | Values |
|---|---|---|---|
| **Key place** | `app::KeyMethod` | `Account.sign_in_key.method`, every ceremony op's `method` | `platform` · `hybrid` · `security_key` — three, no fourth |
| **Signing venue** | `signing_venue::SigningVenue` | `Account.signing_venue` (per account, this device) | `{"type":"in_vela"}` · `{"type":"page","url":"https://sign.getvela.app/"}` |
| **Signing domain** | `String` (an RP ID) | `Account.signing_domain` | `getvela.app` · the self-hosted page's host |

`KeyMethod::TrustedSigner` no longer exists. The string `trusted_signer` is read in exactly one
place — the account reader's private `LegacyMethod` — and written nowhere.

## 2. Core types (new)

### `signing_venue.rs` (no `crux` feature needed; the bindings use it directly)

```rust
pub const APP_DOMAIN: &str = "getvela.app";

#[serde(tag = "type", rename_all = "snake_case")]
pub enum SigningVenue { #[default] InVela, Page { url: String } }   // url normalised by trusted_signer::signer_url

pub fn domain_of_page(url) -> String     // host, or getvela.app for *.getvela.app; "" for a non-address
pub fn is_app_domain(domain) -> bool
pub fn locked_to_page(domain) -> bool    // R2: !is_app_domain

#[serde(tag = "type", rename_all = "snake_case")]
pub enum VenueBlock {                    // R1's reasons; .key() → the corpus line
    AppCannotReach { domain },                      // settings.venue.blockedApp   {{domain}}
    PageOnOtherDomain { page_domain, domain },      // settings.venue.blockedPage  {{pageDomain}} {{domain}}
    NotOnWeb,                                       // settings.venue.blockedWeb   (core round: the web's only)
}
pub fn reachability(domain, &SigningVenue) -> Result<(), VenueBlock>             // R1

pub struct SigningPage { url: String, name: String, trusted: Vec<String> }       // a saved page; `trusted`: its own trusted versions (core round)
pub fn trusted_versions(saved, url) -> Vec<String>          // a page's trusted list; empty for the official page
pub struct VenueChoice { venue, name, domain, official, active, blocked: Option<VenueBlock> }
pub fn venue_choices(domain, active: &SigningVenue, saved: &[SigningPage]) -> Vec<VenueChoice>   // R1 + R2
pub fn venue_choices_on_web(domain, active, saved) -> Vec<VenueChoice>   // + NotOnWeb on every page row R1 lets through
pub fn default_venue(domain, pages) -> Option<SigningVenue>   // in_vela for getvela.app; else the first page on the domain

pub fn ceremony_page(page: Option<&str>) -> Option<String>    // R3: Some only for a page on a custom domain
pub enum SignatureKind { UserOperation, PersonalSign, TypedData, KeyCeremony }
pub fn venue_for(kind, domain, &SigningVenue) -> SigningVenue // R4

pub struct KeyRoute { credential_id, method, transports, hints: Vec<String> }    // R5
pub fn hints_of(method) -> &[&str]   // platform → client-device · hybrid → hybrid · security_key → security-key

pub fn place_title_key(method) -> &'static str              // "This device" / "Phone or tablet" / "USB security key"
pub struct KeyLabel { name: Option<String>, place_key: String }   // "Confirm with {key}": name ?? t(place_key)

pub struct SigningPlan { domain, venue, blocked: Option<VenueBlock>, key: Option<KeyRoute>, key_label: KeyLabel }
impl SigningPlan { fn on_web(self) -> Self }                // page venue on getvela.app → in Vela; custom domain → blocked NotOnWeb
```

`venue_choices` order: Vela's sheet, the official page, the saved pages (deduplicated, invalid
ones dropped), then the account's active page if it is not saved. Every row carries its `domain`
and, when it cannot reach the account's keys, the `VenueBlock` drawn under the disabled row.

### `trusted_signer/launch.rs` — R6

```rust
pub const MAX_CHECK_AGE_MS: u64 = 24 * 60 * 60 * 1000;

pub struct Target { base, version, url, proposed }        // private fields; made only by target()
pub fn target(base, index: Option<&[String]>, trusted, blocked) -> Result<Target, NoVersion>

pub enum Admission { Admitted(CheckedPage), Refused { target, verdict: Verdict } }
pub fn admit(&Target, observed: Option<&str>, failure: CheckFailure,
             trusted, blocked, verification_off, checked_at_ms) -> Admission

pub struct CheckedPage { target, verdict, checked_at_ms }  // private; made only by admit()
impl CheckedPage {
    fn url_launch(&self, request, callback, token, lang, now_ms) -> Result<String, LaunchRefused>   // `&lang=` (core round)
    fn ws_launch(&self, port, token, lang, now_ms) -> Result<String, LaunchRefused>
    fn is_fresh(now_ms) / refresh_due(last_attempt, now_ms) / line(now_ms) / target() / verdict() / checked_at_ms()
}
impl Admission { fn is_fresh(now) / refresh_due(last_attempt, now) / version_to_trust() -> Option<&str> / line() / page() }

// Core round — freshness, the background refresh, the fetch, the time
pub const REFRESH_AFTER_MS: u64 = MAX_CHECK_AGE_MS / 2;    // 12 h
pub const REFRESH_POLL_MS: u64 = 60 * 60 * 1000;           // ask hourly (plus start / foreground)
pub const RETRY_AFTER_MS: u64 = 10 * 60 * 1000;            // rest after an attempt that could not complete
pub const CHECK_HEADERS: &[(&str, &str)] = &[("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")];
pub fn refresh_due(checked_at_ms: Option<u64>, last_attempt_ms: Option<u64>, now_ms) -> bool
pub fn is_fresh_at(checked_at_ms, now_ms) -> bool
pub fn keep_or_replace(previous: Option<Admission>, next: Admission, now_ms) -> Admission   // an offline refresh keeps a fresh admission
pub fn keeps_previous(previous: Option<&Admission>, next: &Admission, now_ms) -> bool
pub fn line_while_checking(previous: Option<&Admission>, now_ms) -> IntegrityLine          // fresh previous' line, else Checking
pub fn checked_time(at, now, utc_offset_min, date_format, time_format, language) -> String // {{time}}: clock today, else date + time

pub enum IntegrityState { Checking, Matches, TrustedHere, Unchecked, Mismatch, Blocked,
                          AskToTrust, CouldNotCheck, NoVersion, AllBlocked }
pub struct IntegrityLine { state, version /* 8 hex */, checked_at_ms, key, opens }
```

**The rule.** `Target::url()` is the one string a shell fetches to check AND the one a launch
opens: `https://sign.getvela.app/b/<sha256>/sign` on the official host (Cloudflare Pages serves it
without the `sign.html` redirect a phone opening offline could not follow), `<base>/b/<sha256>/sign.html`
on anybody else's `dist/`. `launch::target` picks the version: with the deployment's index, the
first of this build's `BUILD_ALLOWED` then the person's trusted hashes that the index lists and that
is not blocked; with no index, `LAUNCH` on the official host (deployed by definition), else the first
unblocked candidate. A custom host's index may *propose* a version nobody knows (`proposed_by_index`)
— checking it ends only in `AskToTrust`; the official host's index never chooses.

`admit` = `integrity::decide`, plus: the bytes at `/b/<version>/` must BE `version` (another
accepted version served there is `Refused`). A failed, missing (`NotChecked`) or mismatched check is
`Admission::Refused` — `integrity::ENFORCE` is now `true` (the switch stays, as the kill switch). The
only functions in the crate that write a launch URL are `CheckedPage::{url_launch, ws_launch}`; the
old free `trusted_signer::{url_launch, ws_launch}` are gone. A check vouches for 24 h, then the line
reads "checking" and a launch is refused until the page is checked again.

## 3. The account record (`app/mod.rs`)

```rust
pub struct Account {
    id, name, address, public_key_hex, created_at_iso, keys: Vec<AccountKey>,
    pub sign_in_key: Option<SignInKey>,      // was `signed_in_with`
    pub signing_domain: String,              // new
    pub signing_venue: SigningVenue,         // new
}
pub struct SignInKey { credential_id, method: KeyMethod /* 3 places */, transports }  // no signer_origin
pub struct AccountKey { credential_id, public_key_hex, name, transports, signer_origin: Option<String> }

impl Account {
    fn key_route(&self) -> Option<KeyRoute>         // R5; replaces sign_in_route()
    fn signing_plan(&self) -> SigningPlan           // venue + domain + key route; R4's input
    fn choose_venue(&mut self, SigningVenue) -> Result<(), VenueBlock>   // R1-guarded
}
pub fn signing_plan_json(account_json) -> Option<String>   // replaces sign_in_route_json
```

### Written shape (`vela.accounts[i]`)

```json
{
  "id": "…", "name": "Savings", "address": "0x…", "public_key_hex": "04…",
  "created_at_iso": "2026-09-30T10:00:00.000Z",
  "keys": [{ "credential_id": "a1b2c3d4", "public_key_hex": "04…", "name": "Savings",
             "transports": "hybrid,internal", "signer_origin": "https://sign.getvela.app" }],
  "sign_in_key":   { "credential_id": "a1b2c3d4", "method": "platform", "transports": "hybrid,internal" },
  "signing_domain": "getvela.app",
  "signing_venue":  { "type": "page", "url": "https://sign.getvela.app/" },
  "signed_in_with": { "credential_id": "a1b2c3d4", "method": "platform", "transports": "hybrid,internal" }
}
```

`signed_in_with` is a **compatibility copy for builds before 102**, written by `Serialize` only:

- a `getvela.app` account gets it with the key's PLACE — an older build signs natively with a key it
  can reach (usable; it does not know venues, so it does not open the page);
- a custom-domain account gets **none** — an older build reading none follows the first key's
  `signer_origin` to the page ("auto" route), and every key of such an account carries it
  (`stamp_page_origin`, applied by the reader and by both machines). Writing a place there would send
  an older build to a native ceremony that cannot reach the key.

`trusted_signer` is never written. `AccountKey.signer_origin` keeps being written: it is what older
builds route by.

### Reading (`impl Deserialize for Account`)

Every field is read loosely: an unreadable `sign_in_key`, `signed_in_with` or `signing_venue` costs
only itself, never the account list. Then:

1. `sign_in_key` = the stored one; if absent, migrated from `signed_in_with`. If both are present and
   name different credentials, the legacy copy wins (an older build signed in again after this build
   wrote the record).
2. `signing_domain` = the stored one, else migrated (table below).
3. `signing_venue` = the stored one **if it reaches the domain's keys (R1)**, else the migrated one,
   else `default_venue`. A record never comes out of the reader pointing at a venue that cannot sign
   for it, unless nothing can (a custom domain with no page known) — then `signing_plan().blocked`
   says so and the shell signs nothing.
4. `stamp_page_origin`: a custom-domain account's keys without a `signer_origin` get the page's.

## 4. Migration of today's records

| Today's record (≤ 0.9.7) | `signing_domain` | `signing_venue` | `sign_in_key.method` | Fixture |
|---|---|---|---|---|
| `signed_in_with.method = trusted_signer`, origin `https://sign.getvela.app` (or any `*.getvela.app`) | `getvela.app` | the official page | from the key's stored transports (`reported_method`, default `platform`); transports = the key's | `official-page-account.json` |
| `trusted_signer`, origin custom (`http://localhost:8140`) | its host (`localhost`) | that origin's page, **locked** | from the key's stored transports | `custom-origin-account.json` |
| `trusted_signer`, origin empty or absent, no key origin | `getvela.app` | the official page (spec: "empty or official → official") | from the key's stored transports | `empty-signer-origin-account.json` |
| `platform` / `hybrid` / `security_key`, no key origin | `getvela.app` | in Vela | unchanged | `app-only-account.json` |
| place sign-in, keys behind a custom origin | that host | its page (locked) | unchanged | (reader test) |
| no `signed_in_with`; first key behind a page | that page's domain | that page | none — "as it always did" | `before-sign-in-key-page.json` |
| no `signed_in_with`, no origins (incl. the retired camelCase client) | `getvela.app` | in Vela | none | `before-sign-in-key-app.json` |

Fixtures: `rust/crates/vela-core/tests/fixtures/spec102/`; tests: `tests/signing_venue_102.rs`
(each row, the written-back record, and what a ≤ 0.9.7 build would route — restated as
`old_build_route` and asserted never to send a custom-domain account native).

Nothing changes in the registry or on-chain.

## 5. Device storage

| Key | Owner | Shape | Notes |
|---|---|---|---|
| `vela.accounts` | session / create / login machines | `Account[]` (§3) | survives as before; sign-out clears it, so a venue is per sign-in (see plan, decision D-3) |
| `vela.signingPages` | **new** `app::signing_pages` | `SigningPage[]` = `[{ "url": "https://…/", "name": "…" }]` | official page never stored; survives sign-out (`vela.` prefix); no storage-catalog row (a preference, like `vela.feeTier`) |
| `vela.trustedSignerUrl` | — | string | read once by `SigningPages` on its first `Stored`, imported as a saved page when usable, then **removed** (`WritePages{remove_legacy_url:true}`) |
| `vela.signerPage.trusted` / `.blocked` | shells (unchanged) | hash lists | FR-009 / FR-010, per device |
| `vela.signerPage.version` | desktop | — | no longer read or written (the checked page is held in memory, 24 h) |

## 6. Machines

### `app::signing_pages` (replaces `app::sign_pref`)

```text
Ops:     ReadStored | WritePages { pages: SigningPage[], remove_legacy_url?: bool }
Results: Stored { pages_json?: string, legacy_url?: string } | Written
Events:  Refresh | PageAdded { url, name } | PageRenamed { url, name } | PageRemoved { url }
         | VersionTrusted { url, version }                 // core round: "Trust this version"
View:    { pages: SigningPageRow[] /* official first */, saved: SigningPage[],
           add_error?: "invalid" | "insecure" | "duplicate", loaded }
SigningPageRow { url, name, domain, official, trusted: string[] }
```

`VersionTrusted` stores a sha256 on that page (saving it first when it was not saved); it is refused
for the official page (any `sign.getvela.app` address), for anything that is not a sha256, and before
the list was read. The stored shape gains `trusted` (omitted when empty).

Edits are refused until the list was read (a write would replace pages it never saw). Removing a
page changes no account's venue (`venue_choices` still lists an active page that is not saved).

### `app::session`

`Event::SigningVenueChosen { address, venue }` — by address (invariant ⑨); refused, nothing written,
when R1 says the venue cannot reach the account's keys; otherwise the in-memory account changes and
`SaveAccount` is issued (best effort, like every session write).

### `app::create_wallet`

| Before | After |
|---|---|
| `Event::SignerPageChanged { url }` (Settings' page) | `Event::SigningPageChosen { url: Option<String> }` — "Use my own signing page", only before the first key |
| `add_methods` could be 0–4 (TrustedSigner, narrowed by the first key's domain) | always the three places |
| `CreateView.{key_relying_party, key_signer_origin, add_blocked: AddBlocked}` | `CreateView.{signing_domain, signing_page, can_choose_page}`; `AddBlocked` deleted |
| `RegisterPasskey { name, exclude_credential_ids, method }` | + `page: Option<String>` (R3) |
| `SignMemberProof { …, signer_origin }` | `…, page` |
| `RegistryPublish { … }`, members with `signer_origin` | + `page`; members lose `signer_origin` |
| account: `signed_in_with { method: trusted_signer }` possible | `sign_in_key { method: place }`, `signing_domain`, `signing_venue` (in Vela; the chosen page if one; locked on a custom domain) |

### `app::login`

`Event::SignIn { method, page: Option<String> }` (`page` defaults to none). An address no browser
would sign on starts nothing. `AuthenticatePasskey { method, page }`, the recovery
`SignProof { …, page }` and the re-publish `RegistryPublish { …, page }` carry R3's page. A new
account is saved with the chosen page's domain and venue; a known one keeps its venue unless the
sign-in ran on a chosen page that reaches its keys.

### Ceremony requests (`trusted_signer/ceremony.rs`)

`Ceremony::{RegisterPasskey, AuthenticatePasskey, SignProof, SignMemberProof}` now read `method` (and
`transports` for the proofs) from the op; the page request's params gain `place`, `hints` and, for
the proofs, `transports` — R5 for the ceremonies a custom-domain wallet runs on its page.

### Signature request (`trusted_signer::request`)

`RequestInput.key_route: Option<&KeyRoute>`. With it, `context.allowCredentials` is that one
credential and `context.keyRoute = { credentialId (b64url), place, transports: [..], hints: [..] }`.
Without it, as before.

## 7. Words (`app/method_words.rs`, i18n)

- `KeyMethod::words` has three arms; `method_words("trusted_signer", …)` → `None`.
- `VenueRow { InVela, Page, OwnPage }` + `venue_row_words` / `venue_words(_json)`:
  `settings.venue.{inVela,inVelaBody}`, `settings.venue.{page,pageBody}`,
  `onboarding.create.{ownPageTitle,ownPageBody}`.
- `VenueBlock::key()`, `IntegrityState::key()` name the reason and integrity lines.
- Core round: `VenueRow::OwnPage` → `VenueRow::SigningPage` (`"signing_page"`; `venue_words` still
  reads `"own_page"`), keys `onboarding.create.{signingPageTitle,signingPageBody}`;
  `Ceremony::title_key()` → `componentsUi.signing.ceremony{Create,SignIn,Confirm}`;
  `KeyLabel.place_key` = the place rows' title keys.
- Keys rows (`wallet_keys::WalletKeyRow.method`) are the key's reported place, never
  `trusted_signer`; `WalletKeyRow.signer_origin` and `DeviceKey.signer_origin` are gone.

Corpus (15 locales) — new: `settings.venue.*` (8), `settings.signing.{pageAdd,pageDuplicate,keysOn}`,
`componentsUi.signing.{handoffTitle,handoffKey}`, `componentsUi.signing.integrity.*` (10),
`onboarding.create.{ownPageTitle,ownPageBody}`. Reworded: `settings.signing.{title,subtitle}` (now
"Signing pages" — the old ones promised a per-signature choice), `componentsUi.signing.trustedSigner
{Waiting,WaitingHint,Closed,Refused,Mismatch,Timeout}` (no "Trusted Signer"; the stale loopback
sentence is gone). Retired: `componentsUi.signing.{trustedSignerTitle,trustedSignerBody,
trustedSignerDoneTab,signWith}`, `settings.signing.{pageTitle,pageSubtitle,pageForeign,pageReset}`,
`onboarding.create.{methodBlockedHint,methodBlockedSigner}`, and three dead duplicates
(`onboarding.create.{alertNotDiscoverableTitle,alertNotDiscoverableBody,verifyStuckHint}`).
1905 paths = 1804 leaf + 101 branch.

Core round (D6 and the shells' gaps): new `settings.venue.blockedWeb`,
`settings.signing.{pageSelfHosted,pageTrust,pageRename,pageRemove,pageName}`,
`componentsUi.signing.{ceremonyCreate,ceremonySignIn,ceremonyConfirm}`; renamed
`onboarding.create.{ownPageTitle,ownPageBody}` → `{signingPageTitle,signingPageBody}` ("Use a trusted
signing page"); reworded `settings.venue.{inVela,page}`, `settings.signing.{pageOfficial,pageAdd}`,
`componentsUi.signing.handoffTitle`; retired `home.rescanNativeNote` (dead since spec 004).
1913 paths = 1812 leaf + 101 branch.

## 8. The send and sign cores: a venue that cannot be used here (core round)

| Core | Was | Now |
|---|---|---|
| `sign_request` | a shell refusing for the venue reported `Failed { message }` (English) | `SignSubmitOutcome::VenueBlocked { block }` → `SignErrorKind::VenueBlocked` (-32603, not retryable), `SignErrorNotice.venue_block`; record reason `SignerUnavailable` |
| `send` | `SendSubmitFailure::Other` → `send.txErrorGeneric` | `SendSubmitFailure::VenueBlocked { block }` → `SendTxErrorKey::VenueBlocked`, `SendView.tx_venue_block` |
| `sign_confirm` | — | `handoff_fee(FeeView?, FeeSpeedView?) → HandoffFee { fee, tier?, tier_key? }` (the hand-off card's row) |

## 9. Page addresses (`trusted_signer::signer_url`, core round)

Full-width forms are folded (U+FF01–FF5E → ASCII, U+3000 → space, `。`/`｡` → `.`), then only
`https` (or `http` on loopback), an ASCII host (LDH labels ≤ 63, ≤ 253 total; `xn--` IDNs; IPv4;
`[IPv6]`), and a port 1–65535 pass; a default port is dropped, user info refused, a non-ASCII path
percent-encoded. `http：／／localhost：8140` → `http://localhost:8140/`; `https://例子.中国/` → invalid.
