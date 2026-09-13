# Implementation Plan: Android Founder Pass and Login Recovery

**Branch**: `048-android-founder-pass-login` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)
**Input**: Feature specification from `/specs/048-android-founder-pass-login/spec.md`

## Summary

Two things, in this order. First, the cross-shell login bug: the web address
now serves the SvelteKit shell over storage the retired client wrote, and the
core's `Account` cannot read the old spelling; the login and session effect
loops swallow the refusal, so the page hangs after the passkey signs. The fix
is tolerance in the core (serde aliases), a one-time rewrite on the web, and a
contract that a refused shell answer becomes the machine's own failure on
every shell — plus tests on all three. Second, the phone's dead affordances:
one clipboard helper, list rows that carry their id, a real code on the share
image, the contacts dock and group management, the identicon viewer hosted
once, the chain and class filters, explorer links, the hero's two taps, the
scanner flip, the settings fields — and a real text-size slider under one
written haptics policy. Every phase ends on the Xiaomi.

## Technical Context

**Language/Version**: Kotlin 2.x + Jetpack Compose (Android shell, minSdk 29);
Rust 2021 (vela-core, serde, crux); TypeScript/Svelte 5 (web shell).
**Primary Dependencies**: uniffi JSON bridge (`CoreHost`/`JsonShell`), CameraX
+ ZXing (scanner), OkHttp via `VelaHttp` (SC-107), Compose Material 3 sheets;
web: the wasm bridge + `effect-loop.ts`, vitest, Playwright.
**Storage**: Android Preferences DataStore (`VelaStore`), browser
`localStorage` (`vela.accounts`, `vela.activeAccountIndex`,
`vela.pendingUploads`), MediaStore for the saved image.
**Testing**: JUnit + the real core (`:app:testDebugUnitTest`, incl.
`CoreWireDriftTest`, `NoStrayHttpClientTest`), `cargo test -p vela-core
--features i18n-all,crux`, vitest, Playwright (isolated `--output`, own port
per the concurrent-sessions rule), the scripted Xiaomi pass (`ui.py`).
**Target Platform**: Android 10+ (the Xiaomi `9d5f42fb` in the parallel space),
the web at phone width, the core for all four shells.
**Project Type**: mobile shell + shared core + web shell.
**Performance Goals**: a tap answers within one frame (~16 ms) with the sheet
or the state change; the login message within 2 s.
**Constraints**: one core (events + view fields only, no new machines);
views may be subsets, operations/results exhaustive; no stray HTTP client;
no `ACCESS_NETWORK_STATE`; the founder's rules (logos, no bottom-sheet abuse,
button feedback: press = deformation + haptic).
**Scale/Scope**: 40 audit rows across ~30 Android files, 6 core lines, ~8 web
files, ~10 new tests.

## Constitution Check

`.specify/memory/constitution.md` is the unfilled template; the standing rules
live in the repo's agent rules and the memory of earlier specs. Gates applied:

- One implementation: the class-filter rule and the share-card composition are
  ported from the web's TypeScript verbatim, not reinvented (`sendTokenClass`,
  `share-image.ts`). The chain filter stays shell render state on both.
- Core changes are additive and backward compatible: serde aliases read the
  old spelling; nothing new is written; no view/event shape changes, so no
  ts-rs / uniffi regeneration — but the web wasm is rebuilt.
- Never silent: a refused shell answer is turned into the machine's failure on
  the web and on Android; the desktop's silent skip is recorded, not changed.
- Device verification for every phase (the founder's standing rule).
- SC-107: the clipboard helper and the gallery save touch no network.

## Project Structure

### Documentation (this feature)

```text
specs/048-android-founder-pass-login/
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── login-recovery.md      # old→new field aliases, refused-answer contract
│   └── android-affordances.md # every dead control → its action (the audit's 40 rows)
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
rust/crates/vela-core/src/app/mod.rs          # Account/AccountKey/PendingUpload* serde aliases
rust/crates/vela-core/tests/app_login.rs      # old-shape fixture logs in
rust/crates/vela-core/tests/app_session.rs    # old-shape fixture restores
app-web/vela-wallet/src/lib/onboarding/core/storage.ts     # normaliser + one-time rewrite
app-web/vela-wallet/src/lib/services/accounts.ts           # keys tolerated absent
app-web/vela-wallet/src/lib/core/effect-loop.ts            # refused answer → toFailure, onError required
app-web/vela-wallet/src/lib/onboarding/sessions.ts, session/session.svelte.ts, routes/[locale]/+page.svelte  # visible failure
app-web/vela-wallet/src/lib/onboarding/core/storage.test.ts, core/effect-loop.test.ts, e2e/*.spec.ts

app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── core/crux/CoreDriver.kt                      # escaped failure answered to the machine
├── core/platform/Clipboard.kt, Haptics.kt (VelaHaptic), Gallery.kt (MediaStore save)
├── core/designsystem/components/VelaSlider.kt   # the text-size slider
├── feature/onboarding/core/SessionController.kt # onFault
├── feature/flows/FlowHost.kt, FlowNav.kt, FlowLive.kt, FlowModels.kt, FlowScreens.kt, ShareCardArtwork.kt, ShareCardCapture.kt, components/*
├── feature/send/SendLive.kt, core/SendController.kt (class filter, group seed, preselect)
├── feature/scan/* (CameraScanner.kt lens, LiveScanSurface.kt flip, ScanSurface.kt look)
├── feature/contacts/* (dock, 群发转账, group new/rename/delete, swipe, 全部, export choice)
├── feature/settings/* (slider, editable overrides, links, feedback box, home rows)
├── feature/wallet/* (hero tap, status line rescue, identicon host)
└── navigation/VelaNavHost.kt                    # identicon viewer host, contacts action branches, chain filter state
docs/design-system/haptics.md (or the existing design doc) # the policy
```

**Structure Decision**: existing layout; no new modules. New files are three
platform helpers (Clipboard, Gallery, VelaHaptic), one slider component, the
contracts docs and the tests.

## Phases

0. **Login recovery** (core → web → Android, tests at each layer, wasm rebuild,
   web e2e with an old-shaped seed, Android with an old-shaped store).
1. **P1 dead taps**: clipboard helper everywhere; list rows carry ids and
   push no phantom step; share image = real code + web composition + gallery
   save; contacts dock, 群发转账, group new/rename/delete; identicon viewer
   hosted once at the NavHost and opened from every artwork.
2. **P2**: class + chain filters (SD1/A1/T1 + chain sheet), explorer links
   (R2/A2/T2), token detail 转账/收款/rows, hero tap-to-hide + status rescue,
   scanner flip, recipient picker 扫码/group rows + per-row pick, add-token
   native tab, settings fields/links, contact 全部/swipe/export choice, group
   menu.
3. **Slider + haptics**: `VelaSlider` for text size, `VelaHaptic` with the
   policy doc, applied to the listed controls, log line per haptic.
4. **Visual parity**: scanner brackets/status line, settings home rows on the
   live route, feedback box.
5. **Closeout**: scripted device pass over all 40 rows + the 9 founder items,
   rulers re-run, results.md with the after-state table, memory.

## Complexity Tracking

None: no new machines, no new modules; the identicon viewer and the chain
filter are hoisted to the NavHost because a per-screen copy would be the
fourth duplicate of the same sheet.
