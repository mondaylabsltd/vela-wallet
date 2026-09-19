# 064 — Release from the release branch: one push cuts every package at one commit, and every shell says which

**Status**: design RULED 2026-09-19 (§0); being built.
**Origin**: founder, 2026-09-19, after 0.9.0–0.9.2 were tagged on `main`.
**Shells**: web wallet, browser extension, desktop (macOS · Windows · Linux), Android, iOS;
five packaging workflows and one new one; one Cloudflare setting only the founder can change.

> 「我们会发布 release/v0.9.2 这种分支，我希望这种分支发布的时候，就是构建 github release，这样可以
> 保证写入到 app 中的 git commit id 和 release/v0.9.2 分支发布时是一致的……因为现在在 main 分支
> 打 tag 这种方式产生了不一致。我希望客户端展示的版本号和 git commit id 确实和这个 git branch 以及
> commit id 对应的上。包括 app-web app-desktop app-android app-ios web extension 都应该要能做到」

## 0. The founder's rulings (2026-09-19)

1. **Pushing `release/vX.Y.Z` IS the release** (「做法选 B」). Not a tag on `main` afterwards:
   the workflow builds the branch's head commit, and creates the tag and the GitHub Release
   *at that commit*.
2. **The web wallet follows the release too**, through a fixed pointer branch that Cloudflare
   deploys from (「方式 1」). Its name is **`released`** (「那就叫 released 吧」) — `release`
   is impossible: git stores branches as paths, so `release` cannot exist beside
   `release/v0.9.3` (`cannot lock ref 'refs/heads/release'`, tried).
3. From spec 063 and unchanged: phones are never attached to a GitHub Release; macOS images
   are signed on the founder's Mac and attached by hand; nothing that can sign is on GitHub.

## 1. What is wrong today — two layers, and the second is worse than the first

**Layer 1, the one that was reported.** `release/v0.9.2` has head **Y**. It is merged into
`main`, which makes a merge commit **X**, and the tag goes on X. Every package is built
from X. Someone holding the app, reading its commit, and looking for it on the release
branch finds nothing.

**Layer 2, found while looking.** Most shells do not show a real commit at all:

| Shell | What "About" shows today | Where it comes from |
|---|---|---|
| Android | the real short commit | `git rev-parse --short HEAD` at Gradle configuration time |
| iOS | `build 1` — no commit | `VELA_GIT_COMMIT` must be passed to `xcodebuild`; `ios-package.yml` never passes it |
| Desktop | **`6ab8f`** | a constant in `settings/fixtures.rs:483` — the design mock's. The comment says so |
| Web, extension | **`v1.0.0 (6ab8f)`** | constants in `settings/fixtures.ts:168-169` — *both* halves are the mock's. 0.9.2 shipped saying 1.0.0 |

One shell of six tells the truth. So this is first a spec about every shell reading its
version and commit from the build, and only then about which commit that is.

## 2. The release, as one push

```
git switch -c release/v0.9.3 main      # bump the four versions here, commit
git push origin release/v0.9.3         # ← this is the release
```

`release.yml` runs on `push` to `release/v*`, in this order:

1. **Gate** (seconds, before anything is compiled):
   - the branch name is `release/vX.Y.Z`, digits only (spec 063's runbook says why a suffix
     breaks three packagers);
   - **all four declared versions equal X.Y.Z** — `Cargo.toml`, `extension/manifest.json`,
     `build.gradle.kts` `versionName`, `MARKETING_VERSION` — one check, where there used to be
     one per workflow per tag;
   - **the version is not already released from a different commit.** If tag `vX.Y.Z` exists
     and points elsewhere, the run FAILS and says so: published packages are not replaced
     under the people who downloaded them. A fix is a new version. (Same commit → the run
     continues and re-attaches, so a failed run can simply be re-run.)
2. **Build**, every shell from the same `GITHUB_SHA`, by *calling* the existing packaging
   workflows (`workflow_call`) — they stay the single definition of how each package is made.
3. **Publish**: one tag `vX.Y.Z` and one GitHub Release at that commit, holding desktop
   (Windows, Linux; macOS if this run signed it — it does not, by ruling) and the extension,
   opened by the install notes. Phone packages stay workflow artifacts (spec 063).
4. **Point `released` at the commit** — fast-forward only
   (`git push origin $SHA:refs/heads/released`). Cloudflare, watching `released`, builds
   and deploys the web wallet from exactly that commit.

Then, by hand and unchanged from spec 063 except for the tag's name:
`./scripts/release-macos-local.sh vX.Y.Z --upload`.

The per-shell tags (`desktop-v*`, `extension-v*`, `android-v*`, `ios-v*`) stop being
triggers. One way to release; `workflow_dispatch` stays on every packaging workflow as the
build check it has been since 0.9.0.

**Why one release instead of one per shell.** They were separate because they were
separately tagged. With one commit and one version there is one thing to point at — and
the rule "push release tags one at a time, or nothing triggers" stops existing.

## 3. Every shell says which build it is

One contract, five implementations:

> The version is the shell's own declared version. The commit is `VELA_GIT_COMMIT` when the
> build sets it (CI always does, from `GITHUB_SHA`; Cloudflare's build from
> `WORKERS_CI_COMMIT_SHA`), otherwise `git rev-parse --short HEAD`, otherwise the word
> `unknown` — never a plausible-looking constant. Seven characters on screen.

| Shell | Change |
|---|---|
| Web, extension | `vite` defines the version (from `extension/manifest.json`, the version the gate already checks) and the commit; `fixtures.ts` stops owning either. The mock's `1.0.0 (6ab8f)` stays where it belongs — in the design gallery |
| Desktop | `build.rs` emits `VELA_GIT_COMMIT`; `about_version(live)` reads it, as it already reads `CARGO_PKG_VERSION`. Flatpak builds in a sandbox from a git source, so the env route must be proven there, not assumed |
| iOS | `ios-package.yml` passes `VELA_GIT_COMMIT` — the build setting has been waiting for it since spec 058 |
| Android | already right; honours `VELA_GIT_COMMIT` too, so a container build without a usable `.git` still says the truth |

**FR-C1** A test per shell fails if "About" can show the mock's constants in a live build.

## 4. The web wallet and `released`

Checked against Cloudflare's docs (Workers Builds, updated 2026-08-28), not recalled: the
production branch is **one fixed branch from a dropdown** — no pattern — and every push to
it builds and deploys; the build sees `WORKERS_CI_COMMIT_SHA` and `WORKERS_CI_BRANCH`.

- `vela-wallet-web`'s production branch becomes `released`. **Only the founder can do
  this** (dashboard → the Worker → Settings → Build → Branch control), and it has to happen
  *after* `released` exists — §6 N1.
- A consequence to agree to, not discover: **merging to `main` no longer deploys the web
  wallet.** It ships when a release does. `main` builds become previews if non-production
  builds are switched on (a checkbox, all-or-nothing on Workers Builds).
- The marketing site (`getvela`) keeps deploying from `main`: it is not a versioned
  client, and it serves `assetlinks.json` and the API, which must not wait for a release.
- `released` only ever moves forward. A release cut from a branch that does not contain
  the previous release is refused by git, which is the right answer.

## 5. Requirements

- **FR-001** `release.yml`: gate → build (reusable workflows) → publish → move `released` (§2).
- **FR-002** The gate's three refusals, each in words a person can act on.
- **FR-003** The five packaging workflows gain `workflow_call`, lose their tag triggers and
  their own `release` jobs; `workflow_dispatch` and the PR metadata checks stay.
- **FR-004** Every build job exports `VELA_GIT_COMMIT` from the commit it checked out.
- **FR-005** §3, per shell, with FR-C1's test.
- **FR-006** `release-macos-local.sh` takes `vX.Y.Z`, and refuses unless `HEAD` is that tag's
  commit — as it does today for `desktop-v*`.
- **FR-007** The runbook and the two READMEs describe this release, and the three traps of
  spec 063 that it retires are marked retired rather than deleted.
- **FR-008** The first release under this spec is its own acceptance test (§7).

**Not in this spec**: store submissions; Android signing; the `released`-branch protection
rules (worth having; a repository setting for the founder).

## 6. What waits on the founder

| # | What | When |
|---|---|---|
| N1 | Cloudflare → `vela-wallet-web` → Settings → Build → Branch control → production branch **`released`** | after the first release has created `released`; until then the web wallet keeps deploying from `main`, exactly as today |
| ~~N2~~ | **AGREED 2026-09-19** (「接受」): `main` no longer deploys the web wallet; it ships with a release (§4) | — |
| N3 | Branch protection for `released` (no direct pushes except the workflow; no force-push) | any time |

## 7. Acceptance — the first release cut this way

For `release/vX.Y.Z` at commit **Y**, all of these say X.Y.Z and the first seven characters
of **Y**: the desktop apps' About on Windows, Linux and (after the local step) macOS; the
extension's About; the Android and iOS builds' About (from their artifacts); and — after
N1 — `wallet.getvela.app`'s About. `git branch --contains` of that commit lists
`release/vX.Y.Z` and `released`. Nothing about it requires looking at `main`.
