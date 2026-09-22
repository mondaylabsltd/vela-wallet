# Tasks — 081, close the audit's product gaps

Feature dir `specs/081-audit-product-gaps` · branch `081-audit-product-gaps` · worktree `/Volumes/data/production/vela-wallet-081`

Order follows [plan.md](plan.md): the three packages that change core wire types run one at a time (WP1 → WP2 → WP3 → WP4); everything else may run alongside. `[P]` marks tasks that touch different files and have no incomplete dependency.

## Phase 1: Setup

- [x] T001 Confirm the baseline is green before touching anything: run every standing gate in [quickstart.md](quickstart.md) and record failures that already exist in `specs/081-audit-product-gaps/results.md`
- [x] T002 [P] Note the two connected devices and their build commands in `specs/081-audit-product-gaps/results.md` (Xiaomi `9d5f42fb`, iPhone 11 `ABC`), per `reference_android_test_device` / `reference_ios_shell_traps`

## Phase 2: Foundational (blocking)

- [x] T003 Add payload-aware checking to `scripts/check-ios-android-parity.mjs` and `scripts/check-android-event-parity.mjs`: compare each dispatched event's field names against the generated `app-web/vela-wallet/src/lib/core/generated/*Event.ts`, not just the `"type"` string (this is the guard that would have caught FR-001)
- [x] T004 Run the new parity check and record every existing mismatch it finds in `specs/081-audit-product-gaps/results.md` (expected: the four iOS settings events in research §2)

## Phase 3: User Story 1 — a dApp cannot take over my wallet (P1) · WP1

- [x] T010 [US1] Create the pure kernel `rust/crates/vela-core/src/app/self_call_guard.rs`: `SelfCallFunction`, `SelfCallBlock`, `detect_self_call(method, params, safe)`, `enforce_no_self_call(...)`, MultiSend/`execTransaction` recursion with depth cap 4, per [contracts/self-call-guard.md](contracts/self-call-guard.md)
- [x] T011 [US1] Register the module and add `SignErrorKind::SelfCallBlocked` + `SignView.blocked` in `rust/crates/vela-core/src/app/sign_request.rs` (hooks at `on_request_arrived` and `proceed_submit`, beside `enforce_no_unlimited`)
- [x] T012 [US1] Block `primaryType == "SafeTx"` typed-data requests whose `domain.verifyingContract` is the signing Safe, in `sign_request.rs`
- [x] T013 [US1] Core tests in `rust/crates/vela-core/tests/app_self_call_guard.rs`: all 13 selectors alone, in a batch leg, nested in MultiSend, inside `execTransaction`, `params_override_json` smuggling, SafeTx typed data
- [x] T014 [US1] Negative tests in the same file: empty-data self-transfer, view selector to self, ordinary dApp call, and the in-band fee leg shape
- [x] T015 [US1] Corpus keys for the blocked sheet in `rust/crates/vela-core/i18n/locales/*/componentsUi.json` (15 locales) + the six-step i18n gate
- [x] T016 [US1] Regenerate bindings and wire types: `npm --prefix scripts run gen:core-types`, `build:wasm`, `sync:wasm`; update `SignWire.kt` and `SignWire.swift` for the new `SignErrorKind`
- [x] T017 [US1] Web: add the signing sheet's missing status/error surface and render `blocked` in `app-web/vela-wallet/src/lib/signing/live.ts` and its sheet component
- [x] T018 [P] [US1] Desktop: render `blocked` in `app-desktop/vela-wallet/src/signing/live.rs`
- [x] T019 [P] [US1] Android: render `blocked` in `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/feature/signing/SigningLive.kt`
- [x] T020 [P] [US1] iOS: render `blocked` in `app-ios/VelaWallet/VelaWallet/Features/Signing/SigningLive.swift`
- [x] T021 [US1] Test-dApp buttons (addOwner, enableModule, batch with a self-call leg, SafeTx typed data) in `app-web/vela-wallet/e2e/testdapp/index.html`, `app-android/.../dev/testdapp/index.html`, `app-ios/.../Resources/testdapp.html`
- [x] T022 [US1] Web e2e in `app-web/vela-wallet/e2e/extension-signing.e2e.ts`: each new button shows the blocked explanation and offers no confirm
- [x] T023 [US1] Device runs: Xiaomi and iPhone — blocked self-call from the test dApp, then a real send and a real dApp transaction still work; record in `results.md`

## Phase 4: User Story 3 — what the wallet tells me is true (P1) · WP2, WP3, WP4, WP6

- [x] T030 [US3] Add `ClearProvenance` to `ClearSignResult` and derive `verified` in `rust/crates/vela-core/src/app/clear_signing.rs`; thread it through `try_calldata`/`try_eip712` and the four builders
- [x] T031 [US3] Implement `PinnedMatch` (fetched descriptor equal to the built-in one) and move the ERC-2612 permit / Permit2 typed descriptors into the built-in table
- [x] T032 [US3] Core tests in `rust/crates/vela-core/tests/app_clear_signing.rs` for every provenance value, including the permit sheets staying verified
- [x] T033 [US3] Corpus key for the unauthenticated-descriptor label (15 locales) + i18n gate; regenerate bindings
- [x] T034 [P] [US3] Web labels in `app-web/vela-wallet/src/lib/signing/live.ts` (stop mapping every `!verified` to `selectorNotListed`)
- [x] T035 [P] [US3] Desktop labels in `app-desktop/vela-wallet/src/signing/live.rs` (stop labelling `partial` as verified ABI)
- [x] T036 [P] [US3] Android labels in `.../feature/signing/SigningLive.kt`; iOS labels in `.../Features/Signing/SigningLive.swift`
- [x] T040 [US3] Required-contract set + `multi_key_ready` in `rust/crates/vela-core/src/app/network_admin.rs` (add SafeWebAuthnSignerFactory and singleton, drop CompatibilityFallbackHandler)
- [x] T041 [US3] Core tests in `rust/crates/vela-core/tests/app_network_admin.rs` for the new set and the two readiness levels
- [x] T042 [US3] Site mirror `app-web/getvela.app/src/lib/chain-setup/required-contracts.ts` + `deployment-data.json` payloads + verdict split in `chain-setup/verdict.ts`, keeping the drift test green
- [x] T043 [P] [US3] Readiness rows per shell: web `lib/settings/live.ts`, desktop `src/settings/live.rs`, Android `SettingsLive.kt`, iOS `SettingsLive.swift` (iOS also gains its missing P-256 row) + corpus keys
- [x] T050 [US3] New core rule `rust/crates/vela-core/src/app/name_verify.rs` (transcript machine, per [contracts/verified-name.md](contracts/verified-name.md)) + tests
- [x] T051 [P] [US3] Web transport in `app-web/vela-wallet/src/lib/services/recipient-identity.ts`, with the cache version bump
- [x] T052 [P] [US3] Desktop transport in `app-desktop/vela-wallet/src/executor/identity.rs`
- [x] T053 [P] [US3] Android transport in `.../feature/contacts/core/IdentityResolver.kt`
- [x] T054 [P] [US3] iOS transport in `app-ios/VelaWallet/VelaWallet/Core/RecipientIdentity.swift`
- [x] T055 [US3] Network-count copy: `rust/crates/vela-core/i18n/locales/*/onboarding.json` (4 keys × 15 locales) + `app-web/vela-wallet/extension/manifest.json` + `app-desktop/vela-wallet/packaging/{app.getvela.VelaWallet.metainfo.xml,vela-wallet.spec,deb/control.in}`; six-step i18n gate
- [x] T056 [US3] Device runs: a name that fails forward verification shows no name on Xiaomi and iPhone; a normal ENS name still shows; record in `results.md`

## Phase 5: User Story 2 — the services I configure are the ones used (P1) · WP5, WP8

- [x] T060 [US2] iOS: add `withEndpoints` to `app-ios/VelaWallet/VelaWallet/Features/Settings/SettingsLive.swift` and project it in `App/RootView.swift`
- [x] T061 [US2] iOS: fix the event payloads in `Features/Settings/SettingsStore.swift` (`field`/`provider`/`chain_id`), and pass the RPC pool to `NetworkAdminExecutor` so `invalidate_pools` works
- [x] T062 [US2] iOS tests round-tripping `SettingsStore` events through the core in `app-ios/VelaWallet/VelaWalletTests/`
- [x] T063 [US2] Core: add `effective_passkey_index_url` beside `effective_ethereum_data_url` in `rust/crates/vela-core/src/app/network_admin.rs` + test
- [x] T064 [P] [US2] Web: read the index per call in `app-web/vela-wallet/src/lib/onboarding/core/registry.ts` (drop the module global)
- [x] T065 [P] [US2] Desktop: set the index at session boot in `src/main.rs`, and on save in `src/executor/network_admin.rs`
- [x] T066 [P] [US2] Android: read the index per lookup in `VelaWalletApplication.kt` / `IdentityResolver.kt`
- [x] T067 [P] [US2] iOS: re-apply the index at `startCreate()`/`signIn()` in `Features/Onboarding/OnboardingModel.swift`
- [x] T068 [US2] Remove `X-Rpc-Url` from every sender: web `rpc-pool-executor.ts`, `bundler-service.ts`; desktop `executor/pool.rs`, `executor/relay.rs`; Android `RpcPoolExecutor.kt`, `RelayClient.kt`; iOS `RelayClient.swift` (+ the stale comment in `Core/CoreHTTP.swift`)
- [x] T069 [P] [US2] Self-hosting guide links: `app-web/vela-wallet/src/lib/settings/ui/EndpointsPanel.svelte` and desktop `src/wallet/page.rs` (`cx.open_url`)
- [x] T070 [US2] Stub-host verification per [quickstart.md](quickstart.md) FR-002 on all four shells; device runs on Xiaomi and iPhone; record in `results.md`
- [x] T071 [US2] p256-index PR: `.env.example` (uncomment the required contract address, add `P256_INDEX_DOMAIN_REGISTRY` with its note), `README` + `readme.zh.md`, `p256-index-server/Dockerfile` (copy `p256-replay`), `docker-compose.yaml` optional env file; verify `docker build` from a clean clone
- [x] T072 [US2] vela-relay PR: `Dockerfile` (workspace members, `build.rs` + `build_info.rs`, `contracts/alto`, `--bin vela-relay`), `compose.yaml` env file; verify `docker build` from a clean clone

## Phase 6: User Story 4 — I can verify what I install (P2) · WP-release

- [x] T080 [US4] Add `id-token: write` + `attestations: write` and `actions/attest-build-provenance@v2` to the `publish` job in `.github/workflows/release.yml`; same for `clearsigning-package.yml`
- [x] T081 [US4] Add a `workflow_dispatch` attest job for hand-published macOS dmgs (re-verify staple + Gatekeeper, then attest) in `.github/workflows/`
- [x] T082 [US4] Document verification: README, `docs/ARCHITECTURE.md` (Distribution), site `install.md` + `get-started` page (15 locales), including that attestation does not remove SmartScreen and that the Windows installer is unsigned; fix the stale `SHA256SUMS-extension` note and the dead README anchor
- [x] T083 [US4] Split the `uniffi-bindgen` binary out of the shipped library crate (`rust/crates/vela-core-uniffi/Cargo.toml`) and re-measure the iOS binary's imported APIs
- [x] T084 [US4] Add `app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy` declaring exactly what survives T083 (UserDefaults CA92.1 at minimum) plus `NSPrivacyTracking=false` and the collected-data entry matching the privacy policy
- [x] T085 [US4] CI gate in `.github/workflows/ios-package.yml`: `plutil -lint`, bundle-root presence, and an `nm -u` check against the declared categories; generate the privacy report from an archive once and record it

## Phase 7: User Story 5 — nothing runs behind my back (P2) · WP7

- [x] T090 [US5] Re-prove no callers, then delete `app-web/getvela.app/src/routes/api/{wallet,transactions,nft,bundler,proxy}` and any now-dead helper in `src/lib/server/net.ts`; keep `og`, `downloads`, `bug-report`, `exchange-rate`
- [x] T091 [P] [US5] Update the stale takeover docs that describe the deleted routes (`docs/project-takeover/{01,02,05,06,08,11}`)
- [x] T092 [US5] Web feedback: new `app-web/vela-wallet/src/lib/services/bug-report.ts` (POST to the site endpoint, prefilled-issue fallback by field id), wire `onsend` in `SettingsHome.svelte`, make the row reachable, replace the fictional preview lines with real ones
- [x] T093 [US5] Test that the feedback payload contains no address, balance, endpoint URL or raw `vela.*` value
- [x] T094 [P] [US5] Fix Android's prefilled issue URLs to use field ids (`&what=`, `&environment=`) instead of `&body=`
- [x] T095 [US5] Desktop erase: handler on the danger card (`src/settings/components.rs`, `src/wallet/page.rs`), full sweep + `clear_all_browsing_data()` + verify + sign-out confirmed
- [x] T096 [US5] iOS erase: prefix sweep over `VelaStore.allKeys()`, `WKWebsiteDataStore` removal, logo `URLCache`, verify, sign-out **confirmed** (`App/RootView.swift`)
- [x] T097 [P] [US5] Web: wire the wide-layout erase card in `src/lib/settings/ui/SettingsDesktop.svelte`
- [x] T098 [P] [US5] Android erase: add the `settings` DataStore, crash prefs, WebView data, logs and caches to `feature/settings/core/DeviceStorage.kt`
- [ ] T099 [US5] Device runs: erase on Xiaomi and iPhone with a dApp browsed first; storage scan shows only the keep-list; record in `results.md`

## Phase 8: Docs sync and close-out · WP9

- [x] T100 Remove each closed gap's disclosure from the site docs in all 15 locales (`self-hosting.md`, `security-audits.md`, `bybit-attack.md`, `whitepaper.md`, `install.md`, `networks-and-fees.md` as applicable)
- [x] T101 [P] Update `README.md`, `docs/ARCHITECTURE.md`, `docs/CONTENT-SOURCE-100-CLUES.md`, and the 080 `claim-ledger.md` + `audit-report.md` ("Not fixed here" → fixed, with the PR)
- [x] T102 Run every standing gate plus the site link crawl and `i18n:status --gate`; fix what they catch
- [ ] T103 Write `specs/081-audit-product-gaps/results.md`: what changed, the verification table, device evidence, decisions made without the founder, and anything left open
- [ ] T104 Open the PRs (monorepo + p256-index + vela-relay), cross-linked, with the merge order stated

## Dependencies

- T003 → T004 → (T060–T062 benefit, but do not block)
- WP order for core wire changes: T010–T023 (US1) → T030–T036 → T040–T043 → T050–T056
- T071/T072 are independent repositories and may land first; T100 must not claim them before they merge
- T080–T085 independent; T083 before T084
- T090 before T091; T092 before T093

## Parallel opportunities

- Phase 3: T018–T020 (three shells) after T016
- Phase 4: T034–T036, T043, T051–T054
- Phase 5: T064–T067, T069, and the two repo PRs T071/T072
- Phase 7: T091, T094, T097, T098

## Implementation strategy

Ship US1 first and alone — it is the only gap that can cost a user everything, and it forces the wire-change discipline the rest of the feature depends on. Then US3 (truth), US2 (self-hosting), and finally US4/US5. Each package is a PR with its own tests, device evidence and doc updates, so a partial feature is always coherent and nothing claims a fix that has not shipped.
