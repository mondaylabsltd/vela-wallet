# 065 — Direct downloads: the site hands a person the right file, and one command attaches a signed package

**Status**: BUILT 2026-09-19, against v0.9.3 (the first spec-064 release) — see
[results.md](results.md). One step waits on the founder: creating the R2 bucket (§A5); until
then every download is the GitHub redirect, which is the designed fallback, not a fault.
**Origin**: founder, 2026-09-19.
**Shells**: the download page on getvela.app (a Cloudflare Worker), and one script.

> 「在 https://getvela.app/get-started 这个页面，每个下载不要跳转到 github release，这样用户还要选择
> 自己下载什么，根本不懂。用一个更加好的方式来下载，这样用户更加直观清晰的选择要下载的，直接下载，
> 不用跳转到 github」
>
> 「github action 希望能支持我上传 … 各个版本的签名包，我可以不用全部上传，按需传我要上传的」
>
> 「这件事放到 065，不要忘记做了」

## 0. What was agreed

1. **The download page stops sending people to GitHub** (Part A). Approved as proposed:
   「其他按你推荐的来」.
2. **A signed package is attached with one local command, one file at a time** (Part B).
3. **Phones stay out of it**: 「android 和 ios 包不用传到 github release」. Their cards keep
   pointing at the stores (spec 063). Uploading signed phone packages means uploading to
   Play / App Store, which is a different tool and not this spec.
4. **Downloads are served from R2, not from GitHub** (§A5) — decided 2026-09-19:
   「要的，放到 r2 下载更快」.

## 1. Why

Today "Download from GitHub Releases" drops a person on a page listing fourteen files —
`vela-wallet_0.9.2_arm64.deb`, `VelaWallet-0.9.2-macos-universal.dmg`,
`app.getvela.VelaWallet-aarch64.flatpak` … — and asks them to know which one is theirs. Most
people do not know whether their laptop is `x86_64` or `aarch64`, and should not have to.
The page already knows: the browser tells it.

## Part A — the download page

**A1. It detects the system and says so.** One primary button, in words:
*Download for macOS (Apple silicon)*. Clicking it starts the download. The person never sees
a GitHub page. Detection is a convenience and must be allowed to be wrong: the chosen
platform is named in the button, and everything else is one click away.

**A2. "Other platforms", written for people**, each a direct download:

| Shown as | File |
|---|---|
| macOS — Apple silicon (M1 and later) | `…-macos-arm64.dmg` |
| macOS — Intel | `…-macos-x86_64.dmg` |
| macOS — not sure | `…-macos-universal.dmg` |
| Windows — most PCs | `…-Setup-…-x64.exe` |
| Windows — ARM (Snapdragon, Surface Pro X) | `…-Setup-…-arm64.exe` |
| Linux — Ubuntu, Debian, Mint (.deb) | `…_amd64.deb` / `…_arm64.deb` |
| Linux — Fedora, openSUSE (.rpm) | `….x86_64.rpm` / `….aarch64.rpm` |
| Linux — any distribution (Flatpak) | `…-x86_64.flatpak` / `…-aarch64.flatpak` |
| Browser extension (Chrome, Edge, Brave) | `vela-wallet-extension-….zip`, with its three install steps shown beside it — a zip a person does not know how to load is not a download |

**A3. A stable entry per platform**: `GET /download/<platform>` on the Worker (e.g.
`/download/macos-arm64`) finds that file in the **latest** Release and redirects to it. The
page never hard-codes a version: a new release moves every button by itself. "Latest"
includes pre-releases — every release so far is one.

**A4. Only what exists is offered.** The macOS images arrive after the founder signs them
locally (spec 063 §3a), minutes to hours after the Release is created. Until they are
there the macOS rows say *coming shortly* — never a link that 404s. The same rule covers
any file a release happens not to carry.
→ The Worker reads the Release's asset list (GitHub's API), cached at the edge for a few
minutes; the page renders from that list, not from a list of what *ought* to exist.

**A5. The bytes come from R2** (founder, 2026-09-19: 「要的，放到 r2 下载更快」).
`github.com` release downloads are often slow or unreachable in mainland China, so the file
a person receives is served by Cloudflare from an R2 bucket, not redirected to GitHub.

- **GitHub Releases stays the source of truth.** R2 is a mirror of it, never a second place
  to publish: a file is in R2 because it is on the Release, and only then.
- **Filled on first request, then kept.** `/download/<platform>` looks in R2 under
  `vX.Y.Z/<file>`; on a miss the Worker fetches the asset from the Release, streams it to
  the person *and* into R2, and every later request is R2 alone. No upload step for the
  founder, and nothing to forget — `release-attach.sh` (Part B) does not have to know R2
  exists. (A warm-up request per file right after a release is a cheap courtesy to the
  first reader, not a requirement.)
- **What is served is what was published.** The mirror is keyed by version and file name,
  and checked against the Release's own `SHA256SUMS*` before it is kept: a truncated fetch
  must not become the file everyone downloads from then on. A `--replace` in Part B has to
  be able to evict the old object.
- **If R2 or the Worker path fails, fall back to the GitHub redirect** — a slow download
  beats none.
- **Costs, said plainly**: R2 has no egress fee; storage is ~150 MB per release (fourteen
  desktop files + the extension), so years of releases fit in a few GB. Old versions can
  be pruned by a lifecycle rule; the latest few are what people download.
- The bucket and its binding live in the site's `wrangler` config; **creating the bucket
  is the founder's** (or done with their say-so) — it is an account resource.

**A6. Words**: sixteen locales, through spec 059's process (translate, then stamp).

**A7. What the page must never do**: offer a phone package; link to a file that is not
there; name a version it read from anywhere but the Release.

## Part B — `release-attach.sh`

A GitHub Action cannot receive a file from a laptop; signing happens on the founder's Mac by
ruling (spec 063 §3a), so attaching is a local act. One command, any subset of files:

```bash
./scripts/release-attach.sh v0.9.3 ~/Downloads/VelaWallet-Setup-0.9.3-x64.exe
./scripts/release-attach.sh v0.9.3 dist/macos/*.dmg
```

Before anything is uploaded, per file, in words when it refuses:

- **it is what it claims** — a `.dmg` is notarized and stapled (`stapler validate`, `spctl`);
  an `.exe` carries a valid Authenticode signature; anything else is refused by name rather
  than guessed at;
- **it belongs to this release** — its name carries the tag's version, and where the format
  allows it, the binary carries the tag's commit (spec 064 §3);
- **it does not replace a published file** unless told to (`--replace`), because that
  changes a checksum under people who already downloaded;
- **no phone package** (`.apk`, `.aab`, `.ipa`) — refused, citing §0.3.

Then it uploads, and updates the checksum file that belongs to that platform.
`release-macos-local.sh` becomes "build, then call this" rather than a second uploader.

This keeps spec 063's rule — *nothing on a Release that a person cannot install* — true for
files attached by hand, which is exactly where it is easiest to break.

## Requirements

- **FR-001** `/download/<platform>`: latest release → that platform's asset → redirect; a
  platform with no asset answers with the page (and the *coming shortly* state), not a 404.
- **FR-002** The asset list is read from GitHub, cached briefly, and survives GitHub being
  slow or down by serving the last good list.
- **FR-002a** The bytes are served from R2 (A5): fill-on-miss from the Release, verified
  against the Release's checksums before being kept, GitHub redirect as the fallback.
- **FR-003** The page: detected primary button; "other platforms" per A2; A4's states.
- **FR-004** Detection is testable without a browser (a pure function of UA / client hints),
  and has a test per row of A2 plus "unknown → show the list, pick nothing".
- **FR-005** Sixteen locales.
- **FR-006** `release-attach.sh` with the four refusals of Part B, each exercised.
- **FR-007** `release-macos-local.sh` delegates its upload to it.

## Depends on

Spec 064's first release: one Release per version, tagged `vX.Y.Z`, with every desktop and
extension file on it. Against today's per-shell releases (`desktop-v*`, `extension-v*`) A3
would have to look in two places.
