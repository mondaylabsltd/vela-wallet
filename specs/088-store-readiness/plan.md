# Implementation Plan: 088 — Store submission readiness

**Branch**: `088-store-readiness` | **Date**: 2026-10-01 | **Spec**: [spec.md](spec.md)
**Input**: [audit.md](audit.md) §4 A and B (code and docs), with §4 C turned into
[owner-checklist.md](owner-checklist.md). §4 D waits for production.

## Summary

The owner uploads Android to a Play testing track and iOS to TestFlight on
2026-10-02. This plan closes the audit's code blockers: release signing, 16 KB
pages, debug extras in release, outside links into the provider-injected
browser, the Android 10/11 tracker failure, and the iOS archive/export check.
It also makes the in-app and public deletion text true and fixes the store
documents. Rules live in vela-core; the shells only draw.

## Technical context

- **Core**: Rust `vela-core` (Crux machines) → UniFFI (Kotlin, Swift) and wasm-bindgen (web).
- **Android**: Kotlin/Compose, AGP 9.3, minSdk 29 / target 36, JNA → UniFFI.
- **iOS**: SwiftUI, Xcode 26.3, iOS 17.4+, VelaCoreKit xcframework.
- **Site**: SvelteKit on Cloudflare, `app-web/getvela.app`.
- **Constraints**:
  - No real phones; emulators and simulators only.
  - No uploads and no deploys.
  - The ja + en i18n residency is capped at 138,800 bytes, and another branch has taken it to 138,798.

## Principles check

| Principle | How this plan keeps it |
|---|---|
| Rules decided once in vela-core | Three new rules, each in the core: who gets a provider (`is_secure_context`), which messages are read (`dapp_browser::page_message`), and which outside links may open and what host to show (`external_page_host`). |
| Shells only draw | Android and iOS call `dappExternalPageHost`, hold the page, and draw one sheet each. |
| Parity | Every shell that has a defect gets the fix. The provider and message rules sit in the core, so they reach Android, iOS, desktop and web together. `velawallet://open` exists only on Android and iOS, and both are fixed. |
| Stable experience in an unstable environment | No new notification. Android 10/11 polls as plain work. A link that is refused fails silently, as every unhandled link does. |
| Copy: state safety positively, no scare banners, no audit claim | The erase note says what stays; the listing copy follows the claim ledger. |
| Less duplication | No new strings: the confirm reuses `receive.pay.open` and `common.cancel`. One sheet per shell. |

## Design decisions

1. **Provider only in secure contexts** (FR-004). The decision lives in two places, and both read the same rule:
   - **Script.** The core's `provider_script` returns early unless `window.isSecureContext`.
   - **Core gate.** The core drops every page message whose frame origin is not a secure context: https, or http on exact loopback.

   Android's `addWebMessageListener` origin rules have no pattern for "any https origin" (androidx docs: only a plain host, `*.host` or an IP literal), so the listener stays `"*"` and the core gate is the real one.

   This reverses spec 070's "a LAN test dApp may sign" for in-app browsers. The owner's rule for 088 is https plus loopback for development.
2. **Outside links ask first** (FR-004). The tokenisers (`PayLink`) are unchanged. `askToOpen(url)` asks the core for the host and holds the page; a sheet shows the host and the URL. Nothing loads until "Open in Vela Wallet". The sheet is hosted outside the navigation graph, so a link can arrive on any screen.
3. **Debug launch extras** (FR-003). A pure `LaunchExtras` object decides which extras a build reads; release reads none of the device-pass extras. It is JVM-tested.
4. **Android tracker** (FR-005). Expedited only on API 31+. The alternative, a `getForegroundInfo()` foreground-service notification, was rejected: it needs new strings (no budget) and a visible notification for a two-minute poll.
5. **Signing** (FR-001). Signing values come from env vars or a gitignored `keystore.properties`. A checked task fails the release when the key is missing. Unsigned output only with `-PvelaUnsignedRelease`, which CI passes.
6. **Build numbers** (FR-012). Both build numbers are `git rev-list --count HEAD`, overridable. CI checks out full history.
7. **16 KB** (FR-002). JNA 5.17.0 and an explicit `max-page-size=16384` for the 64-bit Rust targets.
8. **iOS** (FR-006 to FR-009).
   - Archive and export dry run.
   - Entitlements: `smartcard` is dropped by Xcode, so it was kept and documented.
   - Export-compliance comment made true.
   - Bluetooth string narrowed to the one remaining use.
   - `NSLocalNetworkUsageDescription` added, for user-directed LAN pages and self-hosted endpoints.
   - iPhone locked to portrait.
9. **Deletion** (FR-010).
   - The erase sheet's `keeps` note names what stays: relay up to 14 days, index up to 30 days, the permanent on-chain record.
   - en and ja `desc`/`loses` are trimmed so the corpus is 40 bytes smaller.
   - The site gets `/delete` and `/support`.

## Files

| Area | Files |
|---|---|
| Core | `rust/crates/vela-core/src/app/{dapp_permissions,dapp_browser,dapp_rpc}.rs`, `rust/crates/vela-core-uniffi/src/lib.rs`, `rust/crates/vela-core/tests/{app_dapp_browser,app_dapp_permissions}.rs` |
| i18n | `rust/crates/vela-core/i18n/locales/*.json` (15), regenerated `src/i18n_catalogs/`, `assets/i18n/`, `tests/vectors/i18n-exhaustive.json` |
| Android | `app/build.gradle.kts`, `gradle/libs.versions.toml`, `.gitignore`, `MainActivity.kt`, `LaunchExtras.kt`, `feature/browser/{ExternalPageSheet.kt,core/BrowserController.kt}`, `navigation/VelaNavHost.kt`, `core/i18n/I18nKeys.kt`, `feature/wallet/core/TrackerWork.kt`, `res/values/themes.xml`, tests |
| iOS | `Info.plist`, `VelaWallet.entitlements`, `project.pbxproj`, `App/RootView.swift`, `Features/Explore/{ExternalPageSheet.swift,Core/BrowserController.swift}`, `Features/Onboarding/I18nKeys.swift`, tests, regenerated `vela_core_uniffi.swift` |
| Build | `rust/scripts/build-android.sh`, `.github/workflows/{android,ios}-package.yml`, `rust/pkg-web`, `assets/wasm` |
| Site | `app-web/getvela.app/src/routes/{delete,support,privacy}`, footer, URL list |
| Docs | `docs/NATIVE-LAUNCH-CHECKLIST.md`, `docs/store-submission/{privacy-and-review,store-listing-copy}.md`, this folder |
