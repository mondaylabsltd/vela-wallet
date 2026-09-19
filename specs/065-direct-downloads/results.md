# 065 — results (2026-09-19)

What was built, how each requirement was checked, and — separately — what was not checked.

## Where things are

| | |
|---|---|
| file ↔ platform table, system detection (pure) | `app-web/getvela.app/src/lib/downloads/platforms.ts` |
| latest Release, R2 mirror, fallbacks (deps injected) | `app-web/getvela.app/src/lib/server/downloads.ts` |
| the same, wired to a real Workers request | `app-web/getvela.app/src/lib/server/downloads-edge.ts` |
| `GET /download/<platform>` · `GET /api/downloads` | `src/routes/download/[platform]/` · `src/routes/api/downloads/` |
| the page | `src/routes/[[locale=locale]]/get-started/+page.svelte` |
| `release-attach.sh` | `scripts/release-attach.sh` |

**A design fact the spec did not know:** `/get-started` is prerendered. It cannot render the
Release's file list at build time and must not pretend to, so: the HTML ships the state that
is true knowing nothing (no column highlighted, all twelve downloads plain links to
`/download/<platform>`), and a browser then learns the system and the list. Without
JavaScript, or with GitHub unreachable, every row still works — the Worker decides per click.

## Requirements

| | How it was checked |
|---|---|
| FR-001 | unit: missing platform → `302 /get-started?unavailable=…#downloads`, in the reader's locale if they came from a localized page (same-origin referrer only). Live, local Worker: `/download/android-apk` → that redirect |
| FR-002 | unit: fresh list not re-fetched; stale list served when GitHub is down, including from a new isolate (edge cache); nothing remembered → 503, page keeps plain links |
| FR-002a | **end to end against a real local R2** (`wrangler dev`, v0.9.3's extension, 22 469 822 B): first request streamed to the client and filled R2; second request served from R2; SHA-256 of both = the published `65ad8ef6…`; `Range: bytes=100-199` → `206`, bytes identical. Separately probed R2 itself: a `put` whose bytes do not match the given `sha256` is **refused and nothing is kept** |
| FR-003 | **Redesigned the same day** after the founder's review (「我们的不好看…要让用户很轻松没有压力地找到自己要的东西」, with VS Code's download page as the model): the first version was one button over an eleven-row text list, which had to be *read*. Now three columns — Windows · Linux · Mac — each a picture, a big button that downloads the build that cannot be wrong (x64 / universal, sharpened to the exact chip when the browser says it), and small chips for the other builds. The visitor's column is highlighted “Your system”; nothing is hidden behind a disclosure. Checked in a browser: light/dark, en/zh, real signals on an Apple-silicon Mac → `macos-arm64`, spoofed Windows → Windows column, `windows-x64` |
| FR-004 | 19 detection/matching tests (incl. the structured platform hint outranking a disagreeing UA string, and a Linux that names no distribution still getting its *column*), no browser: one per row of A2, plus unknown / phone / iPad-as-Mac → nothing |
| FR-005 | 14 translations written and stamped; `i18n:status`: 0 stale, `getStarted` translated in all |
| FR-006 | each refusal run for real — un-notarized `.dmg`; a `.deb` (unverifiable kind); unsigned `.exe`; a `.dmg` named for 0.9.4; a published file without `--replace`; an `.apk`. And the pass: the three real 0.9.3 images, `--replace --dry-run`, producing checksums identical to the published `SHA256SUMS-macos` |
| FR-007 | `release-macos-local.sh --upload` now calls `release-attach.sh`; the exact delegated call was dry-run from `dist/macos` |

Mutation check: with the checksum comparison removed from `serveDownload`, the eviction
test fails — it is not green by construction.

## Found while testing, worth knowing

The test machine reaches GitHub slowly without a proxy — the condition A5 exists for. The
first (mirror-filling) download ran at GitHub's speed and the client gave up at 300 s with
16 of 22 MB. **R2 still ended up with the complete, correct file**: the fill does not depend
on the person staying. A truncated object was never kept.

So the first downloader of each file after a release gets GitHub's speed; everyone after
gets R2's. A warm-up request per file after releasing removes even that (spec §A5 calls it a
courtesy; it is not automated here).

## Not checked

- **Production R2 and the production edge cache** — bucket `vela-wallet-installer` exists and
  is bound, but nothing has been served from it until this deploys. Locally it is Miniflare's R2, which enforced `sha256` the way the docs say the real one does.
- **`waitUntil` after a client disconnect in production**: Workers gives background work a
  limited time once the response ends. If a fill is cut short R2 refuses it (wrong hash) and
  the next request fills again — safe, but not observed in production.
- **A signed `.exe` passing** `release-attach.sh`: there is no Authenticode certificate (by
  ruling) and `osslsigncode` is not installed here. The refusal side was exercised.
- **"Right name, wrong commit"** for a `.dmg`: needs a notarized image of another commit,
  which does not exist. The commit check's pass side ran on all three real images.
- Translations are `drafted`, not reviewed (zh included for the new strings). The Windows
  and Chrome button names quoted in them follow each language's UI as recalled, not as
  looked up on a machine in that language.
