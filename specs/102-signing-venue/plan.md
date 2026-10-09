# Implementation Plan: 102 — Where you review and sign: in Vela, or on a page you trust

**Branch**: `102-signing-venue` | **Date**: 2026-10-09 | **Spec**: [spec.md](spec.md)
**Design**: [research.md](research.md) · [data-model.md](data-model.md)

## Summary

The Trusted Signer stops being a fourth key method and becomes what the code always said it was:
**where a person reviews and signs**. Every account has a key PLACE (this device / phone or tablet /
USB key — three, no fourth), a signing VENUE chosen per account on this device (in Vela, or a saved
trusted page), and a signing DOMAIN (the RP ID its keys live under). All rules are in `vela-core`
(Phase 1, this branch): the model and the migration of today's records; R1/R2 reachability with
reasons; R3 ceremony routing; R4 the venue for transactions and messages; R5 the key route the page
is told; R6 one rule for the page that is checked AND opened, enforced. The shells (Phase 2) and the
page (Phase 3) draw and carry it.

## Technical context

- **Core** (`rust/crates/vela-core`): new `signing_venue.rs` (R1–R5, `SigningPlan`), new
  `trusted_signer/launch.rs` (R6), new `app/signing_pages.rs` (replaces `app/sign_pref.rs`); changed
  `app/mod.rs` (Account, SignInKey, KeyMethod, the migration reader and the compatibility writer),
  `app/{create_wallet,login,session,shell,method_words}.rs`, `wallet_keys.rs`, `trusted_signer.rs`,
  `trusted_signer/{ceremony,integrity}.rs` (`ENFORCE = true`; `BUILD_ALLOWED`/`LAUNCH` untouched).
- **Bindings**: UniFFI (`vela-core-uniffi`: `signing_plan`, `signing_venue_choices`,
  `signing_page_domain`, `venue_words`, `SignerPageTarget`, `signer_page_admit` →
  `SignerPageAdmission`, `SigningPagesCore`; Swift bindings regenerated and committed), wasm
  (`signingPlan`, `signingVenueChoices`, `signingVenueBlock`, `signingPageDomain`, `venueWords`,
  `SigningPagesCore`; `rust/pkg-web` + `assets/wasm` rebuilt), TS (`gen-core-types`: three mirrors).
- **Desktop**: the integrity checker (`executor/signer_integrity.rs`) is ported onto the rule in this
  phase (see R6 below); the rest of the desktop is Phase 2 and does not compile until then.
- **Constraints**: i18n residency under `SC005_BUDGET` 145,400 without a budget move; old builds keep
  WORKING on new records ([[vela-alpha-no-backcompat]]); never write `trusted_signer`; never touch
  `app-web/trusted-signer` or `integrity.rs` `BUILD_ALLOWED`/`LAUNCH` here (Phase 0 is another branch).

## The rules, and where each lives

| Rule | Core | Test |
|---|---|---|
| R1 reachability, with reasons | `signing_venue::reachability`, `VenueBlock::key` | `signing_venue::tests::every_venue_against_every_domain` (5 venues × 3 domains), `app_session::a_venue_that_cannot_reach_the_keys_is_refused` |
| R2 locked to its domain; several pages, one active | `venue_choices`, `locked_to_page`, `default_venue`, `Account::choose_venue` | `the_choices_an_account_has`, `an_active_page_that_is_not_saved_is_still_listed`, `a_custom_origin_account_migrates_locked_to_its_page` |
| R3 ceremonies in the app for `getvela.app`, on the page otherwise | `ceremony_page`; ops' `page` field (create/login) | `a_ceremony_runs_on_a_page_only_for_a_custom_domain`, `app_create_wallet::{a_custom_page_mints_and_confirms_every_key, the_official_page_mints_in_the_app_and_becomes_the_venue}`, `app_login::{a_custom_page_sign_in_is_saved_on_its_domain, signing_in_on_the_official_page_runs_in_the_app_and_keeps_the_page}` |
| R4 venue for transactions and messages | `venue_for`, `Account::signing_plan` | `the_venue_applies_to_transactions_and_messages`, `the_plan_is_venue_domain_and_key` |
| R5 key route (credential + transports + hints) | `KeyRoute`, `Account::key_route`, `trusted_signer::request` (`keyRoute`, narrowed `allowCredentials`), ceremony `place`/`hints` | `the_request_names_the_key_to_use_and_where_it_lives`, `a_ceremony_tells_the_page_where_the_key_lives`, uniffi `the_request_carries_the_key_route` |
| R6 one URL checked and opened; failed/missing check refuses | `launch::{target, admit, CheckedPage}`, `ENFORCE = true` | `signing_venue_102::{the_launch_url_is_the_checked_version, a_failed_or_missing_check_opens_nothing, bytes_that_are_not_the_version_named_are_refused, a_stale_check_opens_nothing_until_it_is_run_again, the_desktops_checker_works_through_the_rule, …}`, uniffi `a_phone_opens_only_the_version_it_checked` |
| Migration | `Account`'s reader (`migrate`, `settle_venue`), writer (`signed_in_with` copy) | `signing_venue_102::*migrates*` against `tests/fixtures/spec102/` (official-page, custom-origin, app-only, empty signer_origin, two pre-sign-in-key records) incl. what a ≤ 0.9.7 build routes on the rewritten record |
| No fourth method | `KeyMethod` (3), `method_words`, `wallet_keys` | `there_is_no_fourth_row`, `there_is_no_page_route_any_more`, `a_row_is_captioned_by_where_its_key_lives` |

### R6, and the desktop's checker

`launch::target(base, index, trusted, blocked)` → the version and the ONE url; shells fetch exactly
`target.url()`, hash it (`integrity::hash_page`), `launch::admit(...)`. Only an admitted
`CheckedPage` builds a launch URL (`url_launch`/`ws_launch`); the free `trusted_signer::url_launch`
and `ws_launch` are gone, so "desktop signatures open the core's fixed LAUNCH path, not the checked
one" (research §3) can no longer be written. The desktop's checker keeps its shape — fetch the
index, pick, fetch by hash, hash, rule — but each step is now the rule's: `check(base)` runs
`launch::target` + `fetch_and_hash(target.url())` + `launch::admit` and leaves the admitted page in
memory; `checked_page(base, now)` replaces `open_url(base)` (which fell back to the unchecked bare
address); `line(base, now)` is the hand-off card's integrity line. An index that cannot be fetched is
logged and the launch version asked for directly (it no longer short-circuits to "could not
check"). Ported in `app-desktop/vela-wallet/src/executor/signer_integrity.rs`; compiles clean in
isolation (the crate does not until Phase 2); its unit tests run once the desktop compiles. The same
flow, against the committed `dist/` bytes, is the core test `the_desktops_checker_works_through_the_rule`.

## API changes each shell must make (Phase 2)

Wire summary every shell shares:

| Was | Now |
|---|---|
| `sign_in_route(account) → {credential_id, transports, method, signer_origin?}` | `signing_plan(account) → {domain, venue, blocked?, key?: {credential_id, method, transports, hints}}` |
| `sign_route(keys, "auto")` → page route for page keys | `sign_route` answers places only (`auto` → none); the venue is the plan's |
| `KeyMethod` `trusted_signer` | gone: events with it no longer deserialize; ops never carry it |
| op `method == trusted_signer` ⇒ run the ceremony on a page | op `page: Some(url)` ⇒ run it on that page (R3); `method` is the place, pass it as hints |
| `SignProof/SignMemberProof.signer_origin` | `.page`; also new on `RegisterPasskey`, `AuthenticatePasskey`, `RegistryPublish`; `RegistryPublishMember.signer_origin` gone — the unit's rpId is `registry_rp_id(RegistryPublish.page)` ?? the wallet's own |
| `CreateWalletEvent::signer_page_changed {url}` (sent from Settings' page) | `signing_page_chosen {url?}` — only from "Use my own signing page", before the first key |
| `CreateView.{key_relying_party, key_signer_origin, add_blocked}` | `.{signing_domain, signing_page?, can_choose_page}`; `add_methods` is always the three |
| `LoginEvent::sign_in {method}` | `sign_in {method, page?}` |
| — | `SessionEvent::signing_venue_chosen {address, venue}` |
| `SignPrefCore` (`read_stored`/`write_signer_url`, `vela.trustedSignerUrl`) | `SigningPagesCore` (`read_stored` → `stored {pages_json?, legacy_url?}`; `write_pages {pages, remove_legacy_url?}`; `vela.signingPages`) |
| `trusted_signer_url_launch(base, req, token)` / `ws_launch` | `SignerPageTarget::choose(base, index?, trusted, blocked)` → fetch `url()` → `signer_page_admit(target, hash?, failure, trusted, blocked, off, checked_at_ms)` → `SignerPageAdmission.{opens, line(now), verdict, url_launch(req, token, now), ws_launch}` |
| `TrustedSignerInput` | + `key_route_json` (the plan's `key`, passed through) |
| `key_method_words("trusted_signer", …)` | `None`; `venue_words("in_vela" \| "page" \| "own_page")` for the venue rows and the chooser entry |
| `wallet_keys_step` rows `method: "trusted_signer"`, `signer_origin` | `method` is the place; no `signer_origin`; show the account's `signing_domain` for a custom-domain account |
| `signer_page_enforced()` false | true |
| `Account.signed_in_with` | `sign_in_key` + `signing_domain` + `signing_venue` (the record still carries a `signed_in_with` copy for older builds — never read it) |

### Desktop — `app-desktop/vela-wallet/src` (Rust; ~120 compile errors in ~20 files today)

1. **Ceremony routing** — `executor/mod.rs:132,161,232,268` and `executor/registry.rs:1033,1138`:
   branch on `op.page.is_some()`, not `method == TrustedSigner`; the page is `op.page` itself (drop
   the Settings fallback in `executor/trusted_signer.rs:956-978` `signer_page_for` and
   `:1088-1092` `member_proof`); member-challenge rpId `registry_rp_id(op.page)` (`executor/mod.rs:210-213`);
   unit rpId `registry_rp_id(RegistryPublish.page)` (`executor/registry.rs:995-998`). Open the page
   only via `signer_integrity::checked_page(page, now)` (check first when `None`).
2. **Signature routing** — replace `route_of`/`Route::TrustedSigner`/`page_to_follow`/`page_key`
   (`executor/send.rs:113-230`), `SignContext::new` (`executor/sign_request.rs:187-249`) and
   `follow_sign_in` (callers `wallet/money.rs:296`, `wallet/signing_host.rs:588`) with
   `account.signing_plan()`: `venue = page` → the hand-off card and `CheckedPage::url_launch`;
   `in_vela` → native with `plan.key` (credential + transports); no `key` → `first_key_route` as
   today; `blocked` → refuse with `VenueBlock::key()`. `wallet/page.rs:16892` (`signs_on_page`) and
   `:17551-17557` (the open-signer button) read the plan.
3. **Request** — `executor/trusted_signer.rs:818` `RequestInput { key_route: plan.key.as_ref(), .. }`;
   `:717` `SchemeLine::ask` launches through the `CheckedPage`.
4. **Integrity** — `prime_in_background` (`executor/trusted_signer.rs:590`) checks every page in use
   (accounts' venues + saved pages), not only the Settings URL; `signer_integrity::line` on the card;
   `trusted_signer_e2e.rs` (ignored) builds its launch from a `CheckedPage`.
5. **Settings** — replace `executor/sign_pref.rs` and `wallet/page.rs:11305-11457` (`settings_signing`,
   the URL field, `signer_page_draft/focus` at `:1021,:1028`) with Settings → Signing pages
   (`SigningPages` machine; `storage.rs` `KEY_SIGNING_PAGES = "vela.signingPages"`, keep
   `KEY_TRUSTED_SIGNER_URL` for the one-time import) and the account's "Where you review and sign"
   (`venue_choices` + `SessionEvent::SigningVenueChosen`). `settings/mod.rs:361-370` (`nav_signing`
   used `trustedSignerTitle`/`Body`, `settings.signing.page*`).
6. **Choosers** — `hardware.rs:160-175` drop TrustedSigner from `CREATE_ROUTES`/`SIGNIN_ROUTES`; add
   the "Use my own signing page" entry (`venue_words("own_page")`, picks a saved page, shows its
   domain + integrity line) → `CreateEvent::SigningPageChosen` (`onboarding.rs:322` replaces
   `SignerPageChanged`) / `LoginEvent::SignIn { page }` (`onboarding.rs:339`);
   `onboarding_flow.rs:949,1025-1044` (`add_blocked` captions gone; show `signing_domain`),
   `:166` provider-line arm, `passkey_icons.rs:52` Eye arm, `hardware.rs:273`/`onboarding_flow.rs:974`
   element ids, `gallery.rs:79-250` fixtures, `loc.rs:503-504` (`methodBlocked*`).
7. **Keys list** — `wallet/page.rs:9512-9547` (DeviceKey without `signer_origin`; `key_route()` for
   the "signs here" credential), `:9963-9993,:20184-20212` (no page holder/detail row; show the
   domain for a custom-domain account).
8. **Cards' titles** — `signing/trusted_signer.rs:43,131` used `trustedSignerTitle` → `handoffTitle`;
   waiting/closed/refused/mismatch/timeout keys are unchanged (values reworded).
9. **Test literals** — `Account { … }` gains `sign_in_key/signing_domain/signing_venue`
   (`session.rs:254`, `wallet/money.rs:1789,2538`, `wallet/signing_host.rs:3339`,
   `executor/{send.rs:817, sign_request.rs:1165, identity.rs:482, user_op.rs:1390, storage.rs:996}`,
   `wallet/page.rs:20756`); `SignInKey` has no `signer_origin`; `Assertion.signer_origin` unchanged.

### Android — `app-android/vela-wallet/app/src/main/java/app/getvela/wallet` (Kotlin, UniFFI)

1. `feature/onboarding/core/CoreViews.kt:71-88` — `KeyMethod` loses `TrustedSigner` (`of()` throws on
   unknown: keep it so); `:135-139,160-163,199-208` read `signing_domain`/`signing_page`/
   `can_choose_page` instead of `add_blocked`.
2. Choosers — `flow/SignInMethodSheet.kt:76-80`, `flow/KeysScreen.kt:393-481` (and `:289-291` Eye,
   `:465-475` `methodBlocked*`): three rows + the "Use my own signing page" entry;
   `OnboardingViewModel.kt:368-375` stops sending `signer_page_changed` from the raw store key,
   sends `signing_page_chosen` from the entry; `:449-450` `sign_in {method, page}`.
3. `feature/onboarding/core/OnboardingExecutor.kt:126-253, 346-441, 527-603` — route by op `page`
   (not `method`), rpId from `page`, publish members by `RegistryPublish.page` (today it looks the
   page up in the stored record, `signerOriginOf`); `PasskeyExecutor.kt:815-818` drop the arm;
   `RegistryClient.kt:557-569` `PublishMember.signerOrigin` → unit rpId from the op.
4. Signatures — `feature/send/core/UserOpSpine.kt:77-146` (`routeFor`/`routeOf`): `signingPlan`
   instead of `signInRoute`/`signRoute(AUTO)`; page venue → hand-off card; pass
   `key_route_json = plan.key`; `feature/signing/core/SigningController.kt:537-542` reads the plan
   (one decision, not two). `SendController.kt:765-814` (`pagesOf`, `keyRoutesJson` with
   `signer_origin`) go.
5. **Integrity (new on phones)** — `trustedsigner/TrustedSignerScheme.kt:89` launches through
   `SignerPageAdmission.urlLaunch`; add the fetch: `SignerPageTarget.choose(page, index?, trusted,
   blocked)` → GET `url()` (plain HTTPS, no WebView) → `signerPageHash` → `signerPageAdmit`; check
   when the card opens if none is fresh; draw `line(now)`; Open enabled only when `opens()`.
   `TrustedSignerChannel.kt:284-290` (drops a self-hosted sub-path) goes with it: the page is the
   plan's venue URL.
6. Settings — `feature/settings/core/SignPrefWire.kt`, `SignPrefExecutor.kt`, `SettingsController.kt:204-230`,
   `SettingsLive.kt:87-122`, `SettingsFixtures.kt:245-249`, `SettingsModels.kt:199-206`,
   `SettingsScreen.kt:355,1494-1495,2367`, `core/crux/CoreBridge.kt:12,154`,
   `navigation/VelaNavHost.kt:2121-2416`, `VelaWalletApplication.kt:356,453,834` →
   `SigningPagesCore` (+ `vela.signingPages`; `VelaStore.kt:76` key kept for the import) and the
   account's venue setting (`signingVenueChoices` + `SessionEvent.signing_venue_chosen`).
7. Keys list — `feature/settings/SettingsLive.kt:1001-1028`, `core/WalletKeys.kt:36-143`,
   `VelaWalletApplication.kt:622-648`: no page holder/detail, `credential_id` from `plan.key`.
8. Strings — `VelaWalletApplication.kt:361-364`, `SigningLive.kt:114-123,326`: keys unchanged
   (reworded); `I18nKeys.kt:133-134` `methodBlocked*` and `FlowCopy.kt:100-120`
   (`trustedSignerTitle`/`TRUSTED_SIGNER_BODY`) retired. Hard-coded English at
   `UserOpSpine.kt:134,144,272`, `OnboardingExecutor.kt:547-602` → corpus.
9. Dead code to delete rather than migrate: `TrustedSignerEnvelope.kt`.
10. Tests — `FlowFixturesTest.kt:160-179`, `KeyMethodCopyTest.kt:67-82`, `RegistryBackupTest.kt:146`,
    `TrustedSignerRouteTest.kt`, `TrustedSignerChannelTest.kt`, `CoreWireDriftTest.kt:878-882`,
    `SettingsLiveTest.kt:789-807`, testDebug `SignInKeySigningTest.kt`, `TrustedSignerCeremonyTest.kt`,
    `DappSignMachineTest.kt`, `SendMachineTest.kt` (`signed_in_with` fixtures → `sign_in_key`).

### iOS — `app-ios/VelaWallet/VelaWallet` (Swift, UniFFI; `vela_core_uniffi.swift` already regenerated)

1. `Features/Onboarding/Core/CoreViews.swift:59-64` — `KeyMethod` loses `.trustedSigner`
   (`CaseIterable` feeds both choosers); `:102-137` decode `signing_domain`/`signing_page`/
   `can_choose_page` (iOS decode failures are swallowed — `OnboardingModel.swift:313` — so a wrong
   field silently freezes the create screen: check it).
2. Choosers — `WelcomeScreen.swift:86-133`, `CreatePanel.swift:415-499` (+ `:297-302, :474-485`
   `methodBlocked*`): three rows + "Use my own signing page"; `OnboardingModel.swift:324` sends
   `signing_page_chosen` from the entry (not `signer_page_changed` at `startCreate`), `:394`
   `sign_in {method, page}`; `FlowCopy.swift:104-131`, `UsbCeremonyPrompts.swift:327-338` drop the
   fourth arm.
3. `Features/Onboarding/Core/OnboardingExecutor.swift:118-215, 315-375, 458-521` — route by op
   `page`; rpId from `page`; publish by `RegistryPublish.page` (today it builds the string
   "trusted_signer" per member, `:517`); `RegistryClient.swift:64-77`.
4. Signatures — `Core/UserOpSpine.swift:73-96, 148, 572-685` and `Features/Send/SendExecutor.swift:768-837`:
   `signingPlan` instead of `signInRoute`/`signRoute("auto")`; `SignRouteWire` → plan decode;
   `key_route_json`; `Features/Signing/Core/SigningController.swift:385-390` reads the plan.
5. **Integrity (new on phones)** — `Features/Signing/TrustedSigner/TrustedSignerChannel.swift:232-287`
   and `TrustedSigner.swift:194-246`: fetch + `SignerPageTarget`/`signerPageAdmit` as Android;
   launch via `SignerPageAdmission.urlLaunch`; `TrustedSigner.swift:246` (sub-path drop) goes.
6. Settings — `Features/Settings/SignPrefExecutor.swift`, `SettingsWire.swift:353-390`,
   `SettingsStore.swift:28-176`, `SettingsLive.swift:676-716`, `SettingsModels.swift:52,195-205,686`,
   `SettingsScreen.swift:564,602`, `SettingsSheet.swift:106-108,287-297`, `SettingsFixtures.swift:196-197,595`,
   `App/RootView.swift:367-376,3580-3584,3689-3690`, `Core/VelaStore.swift:79` → `SigningPagesCore`
   + venue setting. (`scripts/check-event-payloads.mjs` already flags `SignPrefExecutor.swift:33,43`.)
7. Keys list — `SettingsLive.swift:486-533`, `Core/WalletKeys.swift:45-217`.
8. Strings — `I18nKeys.swift:86-87,194-220` (retired keys; the 16 `trustedSignerWhere…` constants
   are already dead), `TrustedSigner.swift:137-140`, `TrustedSignerSheets.swift:57-64`,
   `SigningLive.swift:110-113,321`; hard-coded English `UserOpSpine.swift:577-608`,
   `OnboardingExecutor.swift:465` → corpus.
9. Tests — `TrustedSignerRouteTests`, `SignInRouteTests`, `TrustedSignerTests` (+ the
   `recordJson(signedInWith:)` callers in 8 files), `FlowFixturesTests`, `RegistryBackupTests`,
   `TrustedSignerOneSlideTests`, `EraseDeviceTests`, UI tests `TrustedSignerChooserDeviceTests`,
   `TrustedSignerDeviceTests`.

### Web — `app-web/vela-wallet` (SvelteKit + wasm; never opens a page)

1. Generated types are regenerated in this phase (`src/lib/{onboarding,session,core}/generated`):
   `KeyMethod` has three members; `SignPref*`/`AddBlocked` are gone; `SigningPages*`, `SigningPlan`,
   `SigningVenue`, `VenueChoice`, `VenueBlock`, `KeyRoute`, `IntegrityLine` are new.
2. `trusted_signer` handling to delete: `ui/onboarding/v2/{AddMethodPicker.svelte:52-60,
   KeysScreen.svelte:106-116, DoneScreen.svelte:105-110, PasskeyProviderMark.svelte:69-75}`,
   `onboarding/passkey-icons.ts:181-207`, `onboarding/core/copy.ts:129-167`,
   `settings/live.ts:1549-1649`, `signing/sign-challenge.ts:49-102` (no more throw-on-page: a
   `getvela.app` account whose venue is a page signs natively on the web, which has no hand-off),
   `analytics/methods.ts`, `onboarding/v2-fixtures.ts:79-220`.
3. Wasm calls — `core/kernels.ts:653-755`: `signInRoute` → `signingPlan` (`SignInRoute` type →
   `SigningPlan`); drop `trustedSignerUsesWalletPasskeys` (unused); `services/wallet-keys.ts:15-124`
   (`WalletKeyRow` hand-written type loses `signer_origin`, `deviceKeys` stops sending it).
4. `SignPrefCore` → `SigningPagesCore` (`core/client.ts:77,116`, `settings/core/sign-pref*.ts`,
   `routes/[locale]/settings/+page.svelte:50,173,399-406`); `ui/onboarding/v2/CreateFlow.svelte:28,127-137`
   stops sending `signer_page_changed`.
5. Onboarding executor — `onboarding/core/executor.ts:61-173` (`sign_member_proof`/`registry_publish`
   rpId from op `page`), `publish.ts:125-135`; `routes/[locale]/+page.svelte:153-210` `sign_in`.
6. Account handling — `onboarding/core/storage.ts:99-149` (`normaliseAccount`) passes the whole
   record through (it rebuilt `signed_in_with`/`signer_origin` by hand); `services/accounts.ts:14-30`.
7. Strings — `i18n/messages.ts:148-155`, `settings/messages.ts:94-112,555-564`,
   `i18n/engine.server.ts:427-440` (retired keys).
8. Tests — `copy.test.ts`, `add-method-picker.svelte.test.ts`, `passkey-route.test.ts:58-60`,
   `sign-challenge.test.ts`, `ethereum-backup-row.test.ts`, `wallet-keys.test.ts`, `storage.test.ts`,
   `sign-pref.test.ts`, `e2e/welcome-layout.e2e.ts:123-141` (already expects four rows against three).
9. Docs (getvela.app) describing the Trusted Signer as a key method or a Settings field:
   `clear-signing-self-host.md`, `self-hosting.md`, `security-audits.md`, `bybit-attack.md`,
   `whitepaper.md` (en, zh, and the other 13 locales).

## Phase 2 tasks — each shell

- [ ] P2-01 [all] Build against the new API (list above); delete the fourth key method everywhere.
- [ ] P2-02 [all] Create / sign-in choosers: three places + "Use my own signing page" (picks a saved
  page; shows its domain and integrity line; `signing_page_chosen` / `sign_in {page}`).
- [ ] P2-03 [desktop, android, ios] Ceremonies route by the op's `page` (R3); rpIds from it.
- [ ] P2-04 [desktop, android, ios] Signatures route by `signing_plan` (R4); `key_route_json` (R5).
- [ ] P2-05 [desktop, android, ios] The hand-off card (D4): `handoffTitle`, "Confirm with {{key}}"
  (the key's name, or its place's title), the integrity line, Open (enabled iff `opens`);
  waiting / closed / refused / mismatch / timeout as today (reworded strings).
- [ ] P2-06 [android, ios] The integrity check on the phones (T034): fetch `SignerPageTarget.url()`
  over plain HTTPS, hash, admit; re-check when stale; launch only through the admission.
- [ ] P2-07 [desktop] Wire the ported checker: `checked_page` for every launch, check every page in
  use at start and when stale, `line()` on the card.
- [ ] P2-08 [all] Settings → Signing pages (`SigningPagesCore`; add / rename / remove; official first;
  each row's domain and integrity line); the 071 URL field removed. **Web (Phase 2 report): NOT
  wired** — the board exists in the gallery, no route reaches it; see P2b-W3.
- [ ] P2-09 [all] Account settings → "Where you review and sign" (`signing_venue_choices`; disabled
  rows with `VenueBlock` reasons; custom-domain accounts locked; `signing_venue_chosen`). **Web
  (Phase 2 report): READ-ONLY** — it draws the account's venue and rows but sends no
  `signing_venue_chosen`; by design now (the web opens no page, so every page row is disabled with
  `settings.venue.blockedWeb`, P2b-W2).
- [ ] P2-10 [all] Keys list: captions by place, never "Trusted Signer"; a custom-domain account
  shows its domain (`settings.signing.keysOn`).
- [ ] P2-11 [web] The web opens no page (owner, 2026-09-23): a page venue on a `getvela.app` account
  signs natively there; a custom-domain account cannot sign on the web — say why with
  `signingVenueBlock(domain, {"type":"in_vela"})`.
- [ ] P2-12 [all] Tests per list above; `scripts/check-event-payloads.mjs` green; iOS/Android
  hard-coded English moved to the corpus; getvela.app docs updated.

## Phase 3 tasks — the page (`app-web/trusted-signer`, after Phase 0's fix lands)

- [ ] P3-01 Read `context.keyRoute` (`credentialId`, `place`, `transports`, `hints`): `get()` with that
  one credential, its transports, and `hints` — no generic chooser.
- [ ] P3-02 Ceremonies: read `place`/`hints` (create: `authenticatorAttachment` + `hints`;
  proofs: `transports` + `hints`).
- [ ] P3-03 Intent-first layout (the page is the authority; the app shows only the hand-off card).
- [ ] P3-04 15 languages (from the corpus, compiled into the hashed bytes), the zh member-challenge
  text corrected (it is computed locally).
- [ ] P3-05 Rebuild reproducibly → new hash at the FRONT of `BUILD_ALLOWED`; deploy `dist/`; move
  `LAUNCH` only after the deploy (the index keeps an undeployed hash from being chosen).

## Gates (run on this branch)

| Gate | Result |
|---|---|
| `cargo fmt --all --check` (rust) | clean |
| `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings` | clean |
| `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,796 passed, 0 failed, 2 ignored |
| `cargo build --target wasm32-unknown-unknown -p vela-core-wasm` | ok |
| `node rust/scripts/gen-core-types.mjs --check` / `gen-onboarding-types.mjs --check` | current |
| `node rust/scripts/build-web.mjs --check` | current (wasm 4,799,932 B) |
| `node rust/scripts/verify-web.mjs` | 51,391 cases green |
| `./rust/scripts/smoke-kotlin.sh` | 51,346 cases green + onboarding bridge |
| `./rust/scripts/build-ios-xcframework.sh` + `smoke-swift.sh` | bindings regenerated and committed; 51,346 cases green |
| `node scripts/gen-i18n.mjs` (pins 1905 = 1804 + 101), `npm --prefix scripts run dump:vectors`, `lint-i18n-corpus`, `verify-i18n-parity` | regenerated; no new defects; 77,060 comparisons, 0 divergences |
| `tests/i18n_residency.rs` | ja + en **144,269 B** runtime-JSON route (was 144,803), 139,979 B compiled route (was 140,545), budget 145,400 unmoved; reduction 85.9 % |
| `app-desktop/vela-wallet/scripts/check-windows.sh` | ok |
| `scripts/check-event-payloads.mjs` | 2 mismatches — iOS `SignPrefExecutor.swift:33,43`, Phase 2 by design |
| desktop `cargo check` / Android / iOS / web builds | not run to green — Phase 2 (the shells do not compile against the new API) |

## Decisions taken in Phase 1 — for the owner

- **D-1 Older builds.** `trusted_signer` is never written. A `getvela.app` account's record carries a
  `signed_in_with` copy naming the key's PLACE, so a ≤ 0.9.7 build signs it natively — usable, but
  in Vela even when the account's venue is the page. A custom-domain account carries no copy, so an
  older build follows the keys' `signer_origin` to the page ("keep writing signer_origin").
- **D-2 Empty `signer_origin`** on a Trusted Signer sign-in migrates to the OFFICIAL page (as the
  spec says), although ≤ 0.9.7 meant "the page Settings names" — which could have been custom.
- **D-3 Defaults and sign-out.** A new account (and any sign-in in the app) starts **in Vela**; the
  venue lives in the account record, so sign-out forgets it and a later sign-in starts in Vela again
  (a custom-domain account is re-locked to the page it signed in on). Saved pages survive sign-out.
- **D-4 A check vouches for 24 h** (`MAX_CHECK_AGE_MS`); after that the page is checked again before
  it opens, and the card says "checking".
- **D-5 `ENFORCE` is on now** in the core (it bites once a shell wires the admission). Safe across
  Phase 0's new hash: a version is only chosen from what the deployed index lists, or `LAUNCH`.
- **D-6 The official page opens `/b/<sha>/sign`, a custom one `/b/<sha>/sign.html`** — and that same
  string is fetched for the check. The bytes at `/b/<sha>/` must BE that version (stricter than 076:
  another accepted version served there is refused).
- **D-7 A custom page's index may propose an unknown version**, which can only end in "trust this
  version on this device?" — how a self-hoster's own build gets trusted. Never on the official page.
- **D-8 "Use my own signing page" with a `getvela.app` page** runs the ceremonies in the app (R3) and
  makes that page the new account's venue.
- **D-9 The key route narrows the page's `allowCredentials` to the sign-in key.**
- **D-10 Naming** (open item): EN "signing page" / "trusted page", zh 签名页 / 可信签名页,
  "Where you review and sign" / 在哪里预览并签名. The corpus no longer says "Trusted Signer".
- **D-11 Residency.** Room came from the retired fourth-method/URL-field strings and three dead
  duplicates in `onboarding.create` (`alertNotDiscoverable{Title,Body}`, `verifyStuckHint` — read by
  no client); the budget did not move.
- **D-12 Removing a saved page changes no account** (its venue still names it, and the venue list
  still shows it as active).

## Core round after Phase 2 (2026-10-09) — the gaps the shells hit

The shells' Phase 2 reports (desktop, web, Android, iOS) and the page found seventeen things the core
did not yet say. Each is now a core rule with its bindings and tests; the shells adopt them in
Phase 2b (below). Tests: `tests/signing_venue_102b.rs` (21), plus `app_signing_pages`,
`app_sign_request::a_blocked_venue_is_said_and_not_retried`,
`app_send::a_blocked_venue_is_said_with_its_reason`, `method_words::a_key_label_names_its_place_…`,
and the bridges' `trusted_signer_bridge::tests::{freshness_and_the_background_refresh,
trust_time_and_titles_across_the_boundary}`, `tests_102_core_round::*`.

| # | Gap | Rule (core) | UniFFI | wasm |
|---|---|---|---|---|
| 1 | Trust an unknown version of a self-hosted page | `SigningPagesEvent::VersionTrusted {url, version}` stores it on THAT page (`SigningPage.trusted`, `SigningPageRow.trusted`); refused for the official page, a non-sha256, an unread list; an unsaved page is saved by it. The trusted list a check takes is the page's (`signing_venue::trusted_versions`); `launch::{target, admit}` ignore `trusted` for the official page. Question = the `integrity.askTrust` line; button = `settings.signing.pageTrust` | `SignerPageAdmission.version_to_trust()`, `signing_page_trusted(saved_json, url)`; the event rides `SigningPagesCore` | event rides `SigningPagesCore` |
| 2 | `checked {{time}}` | `launch::checked_time(at, now, utc_offset_min, date_format, time_format, language)`: the clock time in the person's format when the check ran today, else date + time | `signer_integrity_time(…)` | `signerIntegrityTime(…)` |
| 3, 11, 17 | "Confirm with {key}" | `KeyLabel {name?, place_key}` — the sign-in key's own label when it is not the wallet's name (trimmed, any case), else its place's title; a record with no sign-in key names its first key's reported place. `Account::key_label()`, carried as `SigningPlan.key_label` | in `signing_plan` JSON | in `signingPlan` JSON |
| 4, 13 | Stale checks: "checking", background refresh, freshness | `launch::{REFRESH_AFTER_MS (12 h), REFRESH_POLL_MS (1 h), RETRY_AFTER_MS (10 min), refresh_due, is_fresh_at, keep_or_replace, keeps_previous, line_while_checking}`, `Admission::{is_fresh, refresh_due}`, `CheckedPage::refresh_due` | `SignerPageAdmission.{is_fresh, checked_at_ms, refresh_due}`, `signer_page_keep_or_replace`, `signer_integrity_line_while_checking`, `signer_page_refresh_due`, `signer_page_refresh_schedule` | `signerCheckFresh`, `signerCheckRefreshDue` |
| 5 | Hand-off card fee + speed row | `sign_confirm::handoff_fee(FeeView?, FeeSpeedView?) → HandoffFee {fee, tier?, tier_key?}` — the card reads the SAME `FeeView` (session in force) and `FeeSpeedView` the sheet drives; no row unless the fee is settled for the speed in force; no control on the card; Open = `sign_confirm_state.enabled && line.opens` | `handoff_fee_row(fee_json?, speed_json?)` | `handoffFeeRow` |
| 6 | `lang=` on the launch | `CheckedPage::url_launch(request, callback, token, lang, now)` / `ws_launch(port, token, lang, now)` put `&lang=<tag>` in the query (a malformed tag is left out); the page resolves it by the apps' rule | `SignerPageAdmission.url_launch(request_json, token, lang, now_ms)` / `ws_launch(port, token, lang, now_ms)` — **breaking** | — |
| 7 | The web's reason; a translated sign-time refusal | `VenueBlock::NotOnWeb` (`settings.venue.blockedWeb`); `venue_choices_on_web` (page rows R1 does not block get it); `SigningPlan::on_web` (page venue on `getvela.app` → in Vela; custom domain → `blocked: not_on_web`). `SignSubmitOutcome::VenueBlocked {block}` → `SignErrorKind::VenueBlocked` + `SignErrorNotice.venue_block` (not retryable; page hears -32603 "This account cannot sign here"; record `SignerUnavailable`). `SendSubmitFailure::VenueBlocked {block}` → `SendTxErrorKey::VenueBlocked` + `SendView.tx_venue_block` | via JSON | `signingPlan(account, "web")`, `signingVenueChoices(…, "web")` |
| 9 | D6 naming | values reworded in 15 locales; `onboarding.create.{ownPageTitle,ownPageBody}` RENAMED `{signingPageTitle,signingPageBody}` (the entry lists Vela's official page too — "own page" misnamed it); `VenueRow::SigningPage` (`"signing_page"`; `"own_page"` still reads) | `venue_words("signing_page")` | `venueWords("signing_page")` |
| 10 | Signing pages' own words | `settings.signing.{pageTrust, pageRename, pageRemove, pageName, pageSelfHosted}` | — | — |
| 12 | A ceremony's own title | `Ceremony::title_key()`: create a key → `componentsUi.signing.ceremonyCreate`; sign in + recovery proofs → `ceremonySignIn`; a new key's proof, a member proof → `ceremonyConfirm` | `trusted_signer_ceremony_title_key(operation_json)` | — (the web runs no page ceremony) |
| 14 | The check fetches what a browser gets | `launch::CHECK_HEADERS` — a navigation `Accept: text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8`; documented beside the rule (Cloudflare's beacon was injected into `text/html` answers only; the page's `_headers` now say `no-transform`) | `signer_page_check_headers() → [SignerHttpHeader {name, value}]` | `signerPageCheckHeaders()` (JSON pairs) |
| 15 | Full-width input stored as an address | `trusted_signer::signer_url` folds the full-width forms a CJK keyboard types (U+FF01–FF5E, U+3000, `。`/`｡`), then accepts only https (or http on loopback), an ASCII host (LDH labels, `xn--` IDNs, IPv4, `[IPv6]`), a port 1–65535; drops a default port; percent-encodes a non-ASCII path; refuses user info. Refusals are `invalid` / `insecure` (`settings.signing.pageInvalid` / `pageInsecure`) | (all URL inputs go through it) | (same) |
| 16 | Stale doc | `trusted_signer_ceremony_request`'s doc: an op whose `page` is set, `method` = the key's place | — | — |

### Decisions taken in this round — for the owner

- **D-13 `{{time}}` is a moment, not "2 min ago".** Every locale's integrity line already says
  "checked AT {{time}}" (「检查于 {{time}}」, 「{{time}} に確認」, "{{time}} kontrol edildi"), so a
  clock time reads right in all fifteen with no new words, never goes stale on an open card, and is
  what all four shells had reached for on their own. The relative words the corpus has ("2m",
  「2分钟前」) are an activity list's and read wrongly there. A check can be up to a day old, so a
  check from before today carries its date ("10/08/2026, 14:32") — "14:32" alone could be read as
  later than now.
- **D-14 Background refresh: half a day.** A page in use (each account's page venue + every saved
  page) is re-checked when its check is older than 12 h — on start, on every return to the
  foreground, and hourly while the app runs; an attempt that could not complete rests 10 min. An
  offline refresh keeps a still-fresh admission; a completed one (a mismatch included) replaces it.
  So Open almost never waits; when it does (a stale check), the card reads "checking" with Open off.
- **D-15 Trust is per page and per device.** "Trust this version" stores the version on that saved
  page (`vela.signingPages`), not in the device-wide `vela.signerPage.trusted` list (no longer read
  or written by 102 shells — a self-hoster answers the question again, once per page). The official
  page can never be trusted into opening something else: the core ignores `trusted` for it even when
  a shell passes it.
- **D-16 The web.** Page rows are shown, disabled, with "Signing pages open from the Vela apps, not
  the web." R1's reason wins where both hold. A `getvela.app` account whose venue is a page signs in
  Vela on the web (P2-11, unchanged); a custom-domain account is refused there with the same line.
- **D-17 Key label.** "Confirm with YubiKey 5C" for a key the person named; "Confirm with Phone or
  tablet" when the key carries the wallet's name (the card's "Signing account" row already says it).
- **D-18 The hand-off card's Open is the sheet's confirm** (`sign_confirm_state`) AND the integrity
  line's `opens` — the fee must be settled for the speed in force before the page opens, because the
  page signs the operation the app assembled, fee leg included. No fee control on the card.
- **D-19 D6, the venue rows.** Besides the owner's wording for the page row ("Review and sign on a
  trusted signing page"), "In Vela" became "Review and sign in Vela" / 「在 Vela 里预览并签名」 so the
  two rows of one choice read alike. The official page's NAME is now "Vela's official signing page"
  (`settings.signing.pageOfficial`, which the shells draw as the row's name — the web also used it as
  a tag, which it no longer needs).
- **D-20 Addresses in other scripts are refused, not converted.** A self-hosted page on an IDN must be
  added in its `xn--` form: the passkey's relying party is the host the BROWSER computes, and a second
  home-made IDNA conversion is how the two would disagree. The full-width fold is the part of NFKC an
  address can contain; the core carries no normalisation tables (wasm size).
- **D-21 Residency.** The new lines (+9 leaves, net) fit under SC-005 without a budget move by
  retiring `home.rescanNativeNote` — the longest `home.rescan*` line, with zero call sites since
  spec 004's research (git grep, every 102 worktree). The rest of `home.rescan*` is equally dead and
  left for its own clean-up.

## Phase 2b — each shell adopts the core round

Wire summary (what changes for every shell):

| Was | Now |
|---|---|
| shell formats `{{time}}` itself (desktop `clock`, Android `sameDay`+`Formats`, iOS `Formats.time`) | `signer_integrity_time` / `signerIntegrityTime` / `launch::checked_time` |
| key name read from the account record (`keys[].name` vs `account.name`) | `SigningPlan.key_label` → `name ?? t(place_key)` |
| `url_launch(request_json, token, now_ms)`, `ws_launch(port, token, now_ms)` | `+ lang` before `now_ms` (the app's language tag) |
| fetch the target with the HTTP client's default `Accept` | send `signer_page_check_headers()` / `CHECK_HEADERS` |
| trusted = device-wide `vela.signerPage.trusted` | trusted = the page's own list (`signing_page_trusted(saved_json, url)` / `SigningPageRow.trusted`) |
| AskToTrust: no action (or borrowed "Confirm") | button `settings.signing.pageTrust` → `version_trusted {url, version: admission.version_to_trust()}`, then check again |
| stale check re-runs at Open with no state | draw `signer_integrity_line_while_checking(previous, now)`; refresh in the background by `signer_page_refresh_due` + `signer_page_refresh_schedule`; keep the result through `signer_page_keep_or_replace` |
| ceremony on a page titled `handoffTitle` | `trusted_signer_ceremony_title_key(op_json)` |
| borrowed `onboarding.create.confirmKeyBtn` / `explore.rename` / `onboarding.create.removeKeyBtn` / `contacts.nameLabel` on Signing pages | `settings.signing.{pageTrust, pageRename, pageRemove, pageName}` |
| `onboarding.create.{ownPageTitle,ownPageBody}`, `venue_words("own_page")` | `onboarding.create.{signingPageTitle,signingPageBody}`, `venue_words("signing_page")` |
| a self-hosted row named by its host | `settings.signing.pageSelfHosted` ("Self-hosted · {{domain}}") unless the person named it |
| a sign-time venue refusal in English | `SignSubmitOutcome::venue_blocked {block}` / `SendSubmitFailure::venue_blocked {block}`; draw `VenueBlock::key()` (`blockedApp {{domain}}`, `blockedPage {{pageDomain}} {{domain}}`, `blockedWeb`) |
| `VenueBlock` had two variants | three: `not_on_web` (no vars) — exhaustive matches/unions must handle it |

### Desktop — `app-desktop/vela-wallet/src` (Rust, links `vela-core`)

- [ ] P2b-D1 `executor/trusted_signer.rs:785` `page.url_launch(…, lang, now)` and `trusted_signer_e2e.rs`
  `ws_launch(port, token, lang, now)` — `lang` = the app's language (`loc.language()`).
- [ ] P2b-D2 `signing/integrity.rs:73-99` `text()`/`clock()` → `launch::checked_time(at, now, offset,
  date, time, lang)` with `format_prefs::current()`'s date and time words.
- [ ] P2b-D3 `executor/signer_integrity.rs`: `fetch_and_hash` (`:160`) and the index fetch (`:101`) send
  `launch::CHECK_HEADERS`; `trusted` per page (`signing_venue::trusted_versions(saved, base)`);
  `record` keeps a fresh admission through an offline refresh (`launch::keep_or_replace` /
  `keeps_previous`); `line` while running → `launch::line_while_checking`; `wants_check` →
  `launch::refresh_due(checked_at, last_attempt, now)`; poll every `REFRESH_POLL_MS` and on focus
  (`prime_in_background`).
- [ ] P2b-D4 Trust: on an `AskToTrust` line (card, Settings → Signing pages, the choosers' picker) a
  `settings.signing.pageTrust` button → `SigningPagesEvent::VersionTrusted { url,
  version: admission.version_to_trust() }`, then `forget_refusal` + check.
- [ ] P2b-D5 `executor/send.rs:160-169` (`key_name`) → `plan.key_label` (`KeyLabel::text(|k| loc.t(k))`);
  `gallery.rs:476-514`, `signing/trusted_signer.rs:516-571` fixtures follow.
- [ ] P2b-D6 Hand-off card: `sign_confirm::handoff_fee(Some(&fee_view), speed_view.as_ref())` as one
  quiet row under the key line; Open enabled iff `confirm_state_of(…).enabled && line.opens`.
- [ ] P2b-D7 Ceremony on a page: title `Ceremony::of(op)?.title_key()` instead of `handoffTitle`
  (`signing/trusted_signer.rs:339,436`).
- [ ] P2b-D8 D6: `loc.rs:504-505` and `signing/pages.rs:209-210` → `onboarding.create.signingPage{Title,Body}`
  (or `venue_words("signing_page")`); self-hosted rows `settings.signing.pageSelfHosted`; Signing
  pages' rename/remove/name → `settings.signing.page{Rename,Remove,Name}`.
- [ ] P2b-D9 A venue refusal at sign time → `SignSubmitOutcome::VenueBlocked{block}` /
  `SendSubmitFailure::VenueBlocked{block}`; draw `SignErrorNotice.venue_block` / `SendView.tx_venue_block`.

### Android — `app-android/vela-wallet/app/src/main/java/app/getvela/wallet`

- [ ] P2b-A1 `feature/signing/trustedsigner/TrustedSignerScheme.kt:104` `admission.urlLaunch(request, token,
  lang, nowMs)`.
- [ ] P2b-A2 `SignerPageChecks.kt:237-256` (`words`, `sameDay`) → `signerIntegrityTime(checkedAtMs, now,
  offsetMinutes, dateFormat, timeFormat, language)`.
- [ ] P2b-A3 `SignerPageChecks.kt:263-290`: request headers from `signerPageCheckHeaders()` (replacing the
  hand-set `Accept`); `trusted` from `signingPageTrusted(savedJson, url)`; keep through
  `signerPageKeepOrReplace`; draw `signerIntegrityLineWhileChecking` while a check runs; refresh by
  `signerPageRefreshDue` / `signerPageRefreshSchedule()` on start, `ON_RESUME`, hourly.
- [ ] P2b-A4 Trust: `feature/settings/SettingsLive.kt:161` `trust` → `settings.signing.pageTrust`; send
  `version_trusted {url, version}` from `admission.versionToTrust()`; same button on the hand-off
  card's AskToTrust line.
- [ ] P2b-A5 `SettingsLive.kt:151-152` (+ remove) → `settings.signing.page{Name,Rename,Remove}`;
  `core/i18n/I18nKeys.kt:124-125,430` borrowings retire from this screen.
- [ ] P2b-A6 `feature/signing/trustedsigner/SigningVenue.kt:80` (`keyName`) and
  `feature/send/core/UserOpSpine.kt:96-145` → `plan.key_label`.
- [ ] P2b-A7 Hand-off card fee row: `handoffFeeRow(feeJson, speedJson)`; Open = `signConfirmState`
  enabled && `line.opens`.
- [ ] P2b-A8 `feature/signing/SigningLive.kt:102`: a ceremony's title is
  `trustedSignerCeremonyTitleKey(opJson)`.
- [ ] P2b-A9 D6: `feature/onboarding/flow/OwnPage.kt:75,96` `venueWords("signing_page")`; self-hosted rows
  `settings.signing.pageSelfHosted`; `FlowFixtures.kt:52`, `FlowCopy.kt:87` doc.
- [ ] P2b-A10 Venue refusals (hard-coded English in `UserOpSpine.kt`) → the `venue_blocked` outcome /
  failure.

### iOS — `app-ios/VelaWallet/VelaWallet` (`vela_core_uniffi.swift` regenerated in this round)

- [ ] P2b-I1 `Features/Signing/TrustedSigner/TrustedSignerChannel.swift:217`
  `admission.urlLaunch(requestJson:token:lang:nowMs:)`.
- [ ] P2b-I2 `SignerPageChecks.swift:308-320` (`text`) → `signerIntegrityTime(…)`.
- [ ] P2b-I3 `SignerPageChecks.swift`: fetch with `signerPageCheckHeaders()`; per-page `trusted`
  (`signingPageTrusted`); `signerPageKeepOrReplace`; `signerIntegrityLineWhileChecking`; refresh by
  `signerPageRefreshDue` on `scenePhase == .active` and hourly.
- [ ] P2b-I4 `Features/Settings/SigningSettings.swift:161-164` → `settings.signing.page{Rename,Remove,Trust,Name}`;
  trust sends `version_trusted`.
- [ ] P2b-I5 `Core/SigningVenue.swift:170-209` (`keyName` from the record), `App/RootView.swift:3820`,
  `Core/UserOpSpine.swift:561`, `Features/Signing/Core/SigningController.swift:183` → `plan.key_label`.
- [ ] P2b-I6 `Features/Signing/TrustedSigner/HandoffCard.swift:141` ceremony titles; fee row via
  `handoffFeeRow`; Open = confirm gate && `opens`.
- [ ] P2b-I7 D6: `Features/Onboarding/I18nKeys.swift:87-88,221` → `signingPage{Title,Body}`;
  `SigningPagePicker.swift:109,301` `venueWords(row: "signing_page")`; UI test
  `SigningVenueChooserDeviceTests.swift:33` (「使用我自己的签名页」 → 「使用可信签名页」); unit tests
  `TrustedSignerRouteTests.swift:365`, `FlowFixturesTests.swift:164`.
- [ ] P2b-I8 Venue refusals → `venue_blocked`.

### Web — `app-web/vela-wallet/src` (opens no page)

- [ ] P2b-W1 `lib/signing/sign-challenge.ts:74,108`: `signingPlan(record, 'web')` (`lib/core/kernels.ts:668`
  gains the `surface` argument); a `blocked` plan refuses with `VenueBlockedError` AND reports
  `{type:'venue_blocked', block}` to the sign/send core, so the sheet draws the reason
  (`SignErrorNotice.venue_block` / `SendView.tx_venue_block`) in the person's language.
- [ ] P2b-W2 P2-09 (read-only, by design): `lib/settings/venue.ts:101` `signingVenueChoices(…, 'web')`;
  `venueBlockText` (`:41`) handles `not_on_web` (`settings.venue.blockedWeb`, no vars) — the TS union
  grew; `messages.ts` / `i18n/engine.server.ts` carry `settings.venue.blockedWeb`.
- [ ] P2b-W3 P2-08 (not wired): recommended to stay so — nothing on the web opens a saved page. For
  the same reason the web's choosers gain nothing from "Use a trusted signing page": a self-hosted
  page's ceremonies run only on that page (R3), which the web cannot open, and the official page's
  run in the app and sign in Vela on the web anyway (P2-11). Recommended: drop the entry on the web
  (coordinator's call — P2-02 listed it for every shell). If it stays, a self-hosted row is drawn
  disabled with `settings.venue.blockedWeb` (a row whose `signingPageDomain(url)` is not
  `getvela.app`).
- [ ] P2b-W4 D6: `lib/core/kernels.ts:706` `venueWords` type `'signing_page'`; tests
  `ui/onboarding/v2/add-method-picker.svelte.test.ts:74,82-83` → `signingPage{Title,Body}`;
  `settings.signing.pageOfficial` is now the official row's NAME (`venue.ts:195` `officialTag` drops).
- [ ] P2b-W5 The gallery boards that draw integrity lines and the hand-off card use
  `signerIntegrityTime` and `handoffFeeRow`.

### The page — `app-web/trusted-signer`

- [ ] P2b-P1 Already reads `?lang=` (`src/sign.js:27`); nothing to change — the apps now send it.

### Gates for this round

| Gate | Result |
|---|---|
| `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --features vela-core/dev-fixtures -- -D warnings`, `cargo clippy -p vela-core-wasm --target wasm32-unknown-unknown -- -D warnings` | clean |
| `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,820 passed, 0 failed, 2 ignored |
| `node rust/scripts/gen-core-types.mjs --check` / `gen-onboarding-types.mjs --check` | current (`HandoffFee`, `KeyLabel` new) |
| `node rust/scripts/build-web.mjs --check` | current (wasm 4,840,468 B, ceiling 8,000,000) |
| `node rust/scripts/verify-web.mjs` | 51,511 cases green |
| `./rust/scripts/smoke-kotlin.sh` | 51,466 cases green + onboarding bridge |
| `./rust/scripts/build-ios-xcframework.sh` + `smoke-swift.sh` | bindings regenerated and committed; 51,466 cases green |
| `node scripts/gen-i18n.mjs` (pins 1913 = 1812 + 101), `dump:vectors`, `lint-i18n-corpus`, `verify-i18n-parity` | regenerated; no new defects; 77,180 comparisons, 0 divergences |
| `tests/i18n_residency.rs` | ja + en **144,550 B** runtime-JSON route (was 144,269), 140,242 B compiled (was 139,979), budget 145,400 unmoved; reduction 85.84 % (≥ 85.8) |
| `app-desktop/vela-wallet/scripts/check-windows.sh` | ok |
