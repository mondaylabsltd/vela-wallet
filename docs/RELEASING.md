# Releasing Vela Wallet

> For whoever cuts the next release — a person or an agent. Claude Code reaches this through the
> owner's `vela-release` skill when the owner says 发布版本 / 发版 / release.

A release is **one push**: `git push origin release/vX.Y.Z`. `.github/workflows/release.yml` builds
every shell from that commit, creates the tag and the GitHub Release at it, and fast-forwards the
`released` branch, which is what Cloudflare deploys the web wallet (wallet.getvela.app) from. The
specs behind this: 063 (channels), 064 (release from the branch), 065 (direct downloads), 066
(calendar versions), 081 (provenance). Read `.github/workflows/release.yml` once before starting;
it is the source of truth if this file and it disagree.

## Never

- Never push a `release/v*` branch without the owner's explicit "go" **for this version**. The push
  publishes packages and deploys the production web wallet. Approval for one release is not
  approval for the next.
- Never create or push a tag by hand, and never tag main. The tag is made by `release.yml` at the
  release branch's head.
- Never attach Android or iOS packages to a GitHub Release. Phones ship through the stores only;
  their CI builds stay workflow artifacts.
- Never sign in CI or put signing material on GitHub. macOS is signed on the owner's Mac.
- Never revoke or delete the older "Developer ID Application" certificates in the owner's keychain.
  Revocation breaks everything ever signed with them.
- Never type the Apple app-specific password or edit the notary keychain profile. They are the
  owner's.
- Never re-release a version from a different commit, and never replace a published package. Once
  the tag exists, a fix is a new version.
- Never force-push `released`. It only moves forward.
- Windows stays unsigned by the owner's ruling (SmartScreen is explained in the notes). Don't buy or
  suggest a certificate as part of a release.

## 0. Agree three things with the owner first

1. **The version.** Run `scripts/check-release-version.sh <candidate>`; it says yes or why not.
   - **The 0.9 line**: 0.9.N+1 is allowed (next revision only, no skips) **until the first calendar
     version exists**. 0.x publishes as a pre-release.
   - **Calendar versions (spec 066)**: `YY.M.REVISION`, e.g. `26.10.0` for the first release of
     October 2026, then `26.10.1`. The month is when the release is cut, not when the code was
     written. Never zero-pad (`26.09.0` breaks cargo, Chrome and rpm). The first calendar release
     closes the 0.9 line for good and publishes as "Latest", not a pre-release. That switch is the
     owner's decision; ask, don't assume.
2. **What goes in.** Normally `origin/main` HEAD. List the open PRs (`gh pr list`) and ask whether
   any must land first.
3. **The go-ahead to push.** Show the version, the commit, the change summary and the release-note
   text, then ask. Do not push before the answer.

## 1. Pre-flight (read-only)

```bash
git fetch origin --tags
prev=$(gh release list --limit 1 --json tagName -q '.[0].tagName')    # e.g. v0.9.6
git merge-base --is-ancestor "$prev" origin/main && echo "main contains $prev"   # required: `released` only fast-forwards
gh run list --branch main --limit 5            # main's CI must be green on the commit you will release
git log --oneline "$prev"..origin/main         # what changed, for the notes and the owner's summary
scripts/check-release-version.sh X.Y.Z
```

If main's CI is red on that commit, stop and report. Don't release over a red main.

## 2. The release commit

Work in a clean worktree or checkout:

```bash
git switch -c release/vX.Y.Z origin/main
```

Bump the **four declared versions**, plus the lock. The gate checks exactly these:

| File | Field |
|---|---|
| `app-desktop/vela-wallet/Cargo.toml` | `[package] version = "X.Y.Z"` |
| `app-desktop/vela-wallet/Cargo.lock` | the `vela-wallet` entry: run `cargo update -p vela-wallet` in `app-desktop/vela-wallet` (the build is `--locked`) |
| `app-web/vela-wallet/extension/manifest.json` | `"version": "X.Y.Z"` (the extension and the web wallet) |
| `app-android/vela-wallet/app/build.gradle.kts` | `versionName = "X.Y.Z"` (`versionCode` is computed: commit count, or `-PvelaVersionCode`) |
| `app-ios/VelaWallet/VelaWallet.xcodeproj/project.pbxproj` | **every** `MARKETING_VERSION = X.Y.Z;` (all build configurations must agree) |

If main already carries the version (0.9.6 was bumped early, for the Chrome store package), only the
notes change. That is fine.

Add the user-facing notes, one plain paragraph about what a person notices (not commit titles), in
**both** Linux packaging files:

- `app-desktop/vela-wallet/packaging/app.getvela.VelaWallet.metainfo.xml`: a new `<release>` first in
  `<releases>`:
  `<release version="X.Y.Z" date="YYYY-MM-DD" type="development">` (`type="stable"` once off 0.x)
  `<description><p>…</p></description></release>`
- `app-desktop/vela-wallet/packaging/vela-wallet.spec`: a new first `%changelog` entry:
  `* Ddd Mmm DD YYYY Monday Labs <hello@getvela.app> - X.Y.Z-1` followed by `- …` lines wrapped at
  about 80 columns.

`packaging/release-notes.md` (desktop) and `extension/release-notes.md` are install instructions
shared by every release. Change them only when installing changed. GitHub adds the PR list
(`--generate-notes`).

Check locally what the gate will check, before pushing:

```bash
grep -m1 '^version' app-desktop/vela-wallet/Cargo.toml
node -p "require('./app-web/vela-wallet/extension/manifest.json').version"
grep -E 'versionName = ' app-android/vela-wallet/app/build.gradle.kts
grep -oE 'MARKETING_VERSION = [^;]+' app-ios/VelaWallet/VelaWallet.xcodeproj/project.pbxproj | sort -u   # exactly one line
awk '/^name = "vela-wallet"$/{getline; print; exit}' app-desktop/vela-wallet/Cargo.lock
scripts/check-release-version.sh X.Y.Z && scripts/check-release-version.test.sh >/dev/null && echo gate-ok
```

Commit as `release: X.Y.Z`, ending with the usual co-author line.

## 3. Push = release (only after the owner's go)

```bash
git push origin release/vX.Y.Z
gh run list --workflow release.yml --limit 1
gh run watch <run-id>
```

Jobs, in order: `gate` → `linux`, `windows`, `macos`, `extension`, `android`, `ios` (the six
packaging workflows, called) → `publish` (one tag and one Release, files attested) → `released`
(fast-forward, which deploys the web wallet).

If a job fails **before `publish`**, no tag exists yet. Fix it on the release branch with a new
commit and push again; the gate allows it. 0.9.5 is the example: a Windows-only exhaustive `match`
broke one installer, fixed by `ce55fb65b` on `release/v0.9.5`. Pull requests skip the packaging
jobs, so a packaging change should first be tried with `gh workflow run <workflow>.yml --ref
<branch>`. Container steps run `dash`, not `bash`.

If it fails **after** the tag exists, the version is spent. Fix it in a new version.

## 4. macOS, by hand on the owner's Mac

CI's macOS job has no credentials: it verifies the build and attaches nothing. On the owner's Mac,
from a clean checkout of the tag:

```bash
git fetch --tags && git checkout vX.Y.Z
cd app-desktop/vela-wallet
./scripts/release-macos-local.sh vX.Y.Z            # build + sign + notarize + verify, upload nothing
./scripts/release-macos-local.sh vX.Y.Z --upload   # …and attach the three .dmg + SHA256SUMS-macos via scripts/release-attach.sh
```

- **Signing:** the script picks the certificate by the provisioning profile's SHA-1, never by name.
  The Developer ID cert's SHA-1 starts `292A51F2`; the profile is
  `~/Downloads/VelaWallet_Developer_ID.provisionprofile`; the notary keychain profile is
  `vela-notary`.
- **Before running:** ask the owner. It runs for a long time and uses their keychain, and Apple may
  prompt them.
- **When the images are on the Release:**
  `gh workflow run macos-attest.yml -f tag=vX.Y.Z`. It checks notarization and Gatekeeper on the
  published bytes and attests them.

## 5. Chrome Web Store (owner uploads)

The store package is `vela-wallet-extension-X.Y.Z-chrome-web-store.zip`: the `chrome-web-store`
artifact of the release run's `extension` job (`gh run download <run-id> -n chrome-web-store`), or
built with `cd app-web/vela-wallet && pnpm package:extension`. **Not** the zip attached to the
Release (the `chrome-extension` artifact): that one keeps `key` for "Load unpacked". Run the
pre-flight checks P1–P6 in `docs/store-submission/chrome-web-store.md` and hand the zip to the
owner. The dashboard upload is theirs.

## 6. Phones

`android-package.yml` and `ios-package.yml` build and verify in the release run. Their artifacts
are never attached. A store upload is the owner's step.
- **Android:** `versionCode` is the commit count, or set with `-PvelaVersionCode=N`. It must be
  higher than every earlier upload.
- **iOS:** `CURRENT_PROJECT_VERSION` is still `1`. It must be raised before any App Store or
  TestFlight upload.

## 7. Verify, then report

```bash
gh release view vX.Y.Z    # rpm, deb, flatpak, Windows, the extension zip, SHA256SUMS (+ .dmg ×3 and SHA256SUMS-macos after step 4)
gh attestation verify <a downloaded file> --repo mondaylabsltd/vela-wallet
git ls-remote origin refs/heads/released    # = the release commit
curl -s https://getvela.app/api/downloads | head -c 300    # "version":"X.Y.Z"; the R2 mirror fills on first request
```

Also check that the web wallet's About page reads **X.Y.Z (<short sha>)** at wallet.getvela.app,
once Workers Builds has deployed `released`.

Report to the owner:
- the Release link;
- what is attached and what is still pending (macOS, the Chrome store, the phone stores);
- anything that failed and how it was fixed.

## 8. Back into main

```bash
gh pr create --base main --head release/vX.Y.Z --title "release: X.Y.Z back into main" --body "…"
```

The owner merges it with a merge commit, as with #402 for 0.9.6. The next release must contain this
one, or `released` refuses to move.

## Traps already met

- **0.9.0–0.9.2:** tagging main after merging meant the packages carried a commit that wasn't on the
  release branch, and five of six shells showed the design mock's commit. That's why the tag now
  comes from `release.yml`.
- **0.9.5:** code compiled only on Windows broke one of six installers. The release builds all six;
  a PR builds none.
- **The gate:** a stale `Cargo.lock` or one `MARKETING_VERSION` left behind fails it in seconds.
  Run the local checks in step 2.
- **Versions:** the month rule (spec 066) and no zero padding.
- **`released`:** it must fast-forward. Cut from a main that contains the previous release (step 1).
