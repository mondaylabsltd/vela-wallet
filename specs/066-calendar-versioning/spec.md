# 066 — Calendar versioning: a Vela version says when, not how much

**Status**: RULED and BUILT 2026-09-19. Takes effect when the 0.9 line ends (0.9.4 was released under it on 2026-09-21, §4); the first calendar release is `YY.M.0` for the month it is cut.
**Origin**: founder, 2026-09-19, after 0.9.0 – 0.9.3.
**Touches**: `release.yml`'s gate, one script and its test, the runbook. No shell's code.

> Vela uses calendar versioning: **YY.M.REVISION**. `26.9.0` is the first September 2026 release.

## 1. The rule

Three numbers, and their names are **Year · Month · Revision** — *not* major · minor · patch.

| Version | Means |
|---|---|
| `26.9.0` | September 2026 · first release |
| `26.9.1` | September 2026 · revision 1 |
| `26.10.0` | October 2026 · first release |
| `27.1.0` | January 2027 · first release |

- The **month is the month the release is cut**, not the line of code it fixes. A fix for
  26.9.2 made in October is `26.10.0`. There are no maintenance branches (`released` only
  moves forward, spec 064), so there is nothing else it could be.
- The **revision starts at 0** and counts up by one. `.0` reads as "the first one", which is
  what it is.
- Nothing about a version number says how big the change is. That is what release notes are
  for. Nobody has to decide whether something "deserves" a major version, ever again.

## 2. Why this fits a wallet

Vela ships continuously; there is no "Vela 2". `1.7.3` tells a person nothing. `26.3.4`, seen
six months later, tells them — and tells us, in a bug report or a screenshot of About — that
**this wallet is half a year old**. For software that holds money, *how stale is this client*
is the most useful thing a version can say, and it is the one thing SemVer cannot.

About already shows `version (commit)` in every shell (spec 064 §3); it becomes
`26.9.0 (8366943)`: when, and exactly what.

## 3. `26.9.0`, not `26.09.0` — checked, not preferred

Zero-padding the month sorts prettily and **cannot be used**:

| Consumer | `26.09.0` | Checked how |
|---|---|---|
| cargo (desktop's `Cargo.toml`) | **rejected**: `invalid leading zero in minor version number` | ran it, 2026-09-19 |
| Chrome / Edge extension manifest | **rejected**: integers "must not start with 0" | Chrome's manifest `version` documentation |
| rpm, dpkg | accepted — and compared as *equal to* `26.9.0` | their numeric segment comparison |

The last row is the worst one: two spellings of one version, where a checksum list, a tag
and a package manager could each pick a different one. So: no leading zeros, anywhere, and
the gate refuses them rather than normalizing them.

Everything else takes `26.9.0` as it is: iOS `CFBundleShortVersionString` (three integers),
Android `versionName` (free text), Inno Setup, Flatpak, MSIX (as `26.9.0.0`; each part must
be ≤ 65535), git tags, GitHub Releases, and spec 065's download page, which matches file
names by shape (`[\d.]+`) and never by a particular version.

## 4. What enforces it

`scripts/check-release-version.sh <version>`, run by `release.yml`'s gate before anything is
compiled, for a version that does not have its tag yet. It refuses, in words:

1. **a shape that is not `YY.M.R`** — a padded month or revision, a suffix, two or four
   segments, a four-digit year, month 13, `1.0.0`;
2. **a month that is not now.** "Now" is generous on purpose: the month it is *anywhere on
   Earth* (UTC−12 … UTC+14). Releasing `26.10.0` on the morning of October 1st in Shanghai —
   still September 30th in UTC — is right, and must not be refused by a server's clock;
3. **a skipped or stale revision**: `.1` needs `.0` to exist; `.0` is refused once `.1` does.

**One exception, ruled 2026-09-21** (「坚持发 0.9.4」): the 0.9 line may *finish*. `0.9.N` passes
when it is the next revision after the last `v0.9.*` tag — and only while no calendar version
has been released. Once `v26.*` exists, 0.x is below every version anyone holds and is refused.
Four cases in the test cover it (next, skip, go back, after CalVer began).

A re-run of a release that already has its tag skips this check — otherwise a run retried
after midnight on the last day of a month would start failing for no reason the person
could fix.

`scripts/check-release-version.test.sh` holds twenty cases, one per sentence above
(including the two timezone edges and the year turning). The gate runs the tests *before*
it runs the script: a rule nobody exercised is a rumour.

Unchanged from spec 064: the four declared versions must all equal the branch's version,
and a version is never re-released from a different commit.

## 5. Things this changes that are easy to miss

- **This is a one-way door on the stores.** App Store and Play require a version higher
  than the last one submitted; after `26.9.0` there is no going back to `1.0.0`. Nothing has
  been submitted yet, which is exactly why now is the time.
- **"Pre-release" no longer comes from the number.** `release.yml` marks `0.*` as a GitHub
  pre-release (founder's ruling, 2026-09-17: "not the final 1.0"). A CalVer release has no
  1.0 to be short of, so `26.9.0` is published as a full release — GitHub's *Latest*. That
  line stays in the workflow for the record; it simply never matches again. If a build must
  be marked as not-for-everyone, that needs a new way to say so; none is defined here.
- **Build numbers are a different thing and are still `1`**: Android `versionCode` and iOS
  `CURRENT_PROJECT_VERSION` must rise with every store upload regardless of what the
  marketing version says. Not this spec; it becomes urgent at the first store submission.
- **`0.9.3 → 26.9.0` is an upgrade** to every comparator above (26 > 0), so the people on
  0.9.x are offered it normally.
- Only the **apps'** version is calendar-based. `vela-core` and the other Rust crates are
  libraries with real API contracts; they keep SemVer.
