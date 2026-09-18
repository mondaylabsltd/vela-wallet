# 063 results

What was verified, how, and — as plainly — what was not. 2026-09-18.

## Verified

| What | How | Result |
|---|---|---|
| Phones no longer publish (T001–T003) | `gh workflow run` on the branch, real builds — runs [35330090542](https://github.com/mondaylabsltd/vela-wallet/actions/runs/35330090542) (Android), [35330098084](https://github.com/mondaylabsltd/vela-wallet/actions/runs/35330098084) (iOS) | Two jobs each, no `release` job exists; APK+AAB and ipa+xcarchive built and kept as artifacts with their checksums; no release created |
| The gate, negative path (T016, T017, T019) | `gh workflow run "Desktop macOS packages"` on the branch — run [35330082788](https://github.com/mondaylabsltd/vela-wallet/actions/runs/35330082788) | `publishable=false`; keychain import, Gatekeeper validation and keychain cleanup **skipped**; all three architectures built ad-hoc and slice-verified; `Attach packages to the release` **skipped**; ran in the auto-created, unprotected, secret-less `build-check` environment; `release` untouched |
| Distribution signing (T010, T012–T014) | local `--distribution` run against a real `aarch64` release build with the real Developer ID identity | `flags=0x10000(runtime)`, secure `Timestamp=`, `Authority=Developer ID Application`, `com.apple.security.device.camera => true` in force, `NSCameraUsageDescription` present, image signed, `spctl` → *rejected — Unnotarized Developer ID* (the right answer: everything but Apple's ticket) |
| Profile validation (T018) | the workflow's checks run verbatim against the real profiles on the founder's Mac | development profile → refused as a development profile; another app's Developer ID profile → refused by App ID; the certificate-in-profile loop matched both real Developer ID certificates |
| Notary JSON parsing | `plutil -extract status raw` on a sample `notarytool` JSON | `Accepted` |
| Entitlements XML is strict-parser clean | `xmllint --noout`; a scan for `--` inside comments (one was written, caught, removed before commit) | clean |
| Extension package excludes the notes (T033) | `pnpm build:extension` | no `.md` in `extension/dist` |
| Download page (T040–T042) | `bun run i18n:status` before/after stamping; `vitest --run`; `svelte-check`; `prettier --check` | STALE ×15 before (correct: English changed), 0 after; 479 tests pass; 0 errors; formatted. Only `getStarted`'s fingerprint moved in each locale |
| Packaging scripts | `shellcheck -S style` over every script (CI's shellcheck flagged an `A && B \|\| C` the local default severity let through; fixed, and checked at the strictest level since) | clean |
| PR checks | #243 | all green, including the four `Validate …` metadata jobs and `Build the extension` |

## Not verified — and why

| What | Why not | Who closes it |
|---|---|---|
| **Notarization itself** (`notarytool submit`, stapling, the final `spctl` pass) | needs notary credentials (N3) | founder, `quickstart.md` §4 — one local command |
| **A launch of the signed bundle**, and "This device" inside it | needs a Developer ID provisioning profile for this App ID, which does not exist yet (N1). The rehearsal used the development profile to exercise the embed step only; such a bundle is not launchable | founder, §4 |
| **The camera in a packaged app** | `NSCameraUsageDescription` and the camera entitlement are present and in force in the signature, but nobody has pressed "Scan" in a packaged, hardened-runtime build. The claim that the missing key was a process kill rests on Apple's documented TCC behaviour, not on a reproduction | founder, §4 step 4 |
| **The gate, positive path** (sign → notarize → attach three dmgs) | credentials are reachable only from a `desktop-v*` tag, by design | the first tag after N4 |
| **`--notes-file` + `--generate-notes` in a real release** | only a tag creates a release. `gh`'s help documents the prepend behaviour (research R8) | the next `desktop-v*` / `extension-v*` tag |
| **Translations by a native reader** | fourteen locales are `drafted` by policy; zh is `reviewed` and two of its strings changed | founder for zh (T044) |

## Findings beyond the brief

1. **The packaged macOS app would have been killed on "Scan".** No `NSCameraUsageDescription`; unseen because the scanner had only run under `cargo run`, where the terminal is the responsible process. The third time this month a path that only a published package walks hid a defect — the ledger's 门外路径.
2. **The public stores do not show the apps.** Play 404 ×7 regions, App Store lookup empty (research R7). The page keeps "coming soon" until the founder says otherwise (N5).
3. **The Apple certificates are issued to a person.** If the enrollment is individual, Guideline 3.1.5(i) keeps wallet apps off the App Store and the Mac App Store regardless of anything technical (N6).
4. **Production `assetlinks.json` trusts a debug keystore** whose password is `android` and which lives on a laptop (N7). Any holder of that file can build an app that `getvela.app` vouches for.
5. **Two same-named Developer ID certificates** in the founder's keychain make signing *by name* fail as ambiguous on that Mac; everything here uses the SHA-1.
