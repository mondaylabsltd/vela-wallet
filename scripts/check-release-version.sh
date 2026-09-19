#!/usr/bin/env bash
# May this be the next Vela version? (spec 066 — calendar versioning)
#
#   scripts/check-release-version.sh 26.9.1
#
# Vela's version is YY.M.REVISION — Year · Month · Revision, NOT major/minor/patch:
#   26.9.0   the first release of September 2026
#   26.9.1   the second
#   26.10.0  the first of October
#
# release.yml's gate runs this before anything is compiled. It refuses, in words:
#   1. a shape that is not YY.M.R — including a zero-padded month. `26.09.0` is
#      not a style choice: cargo rejects it ("invalid leading zero"), Chrome's
#      manifest forbids it, and rpm/dpkg read it as the SAME version as 26.9.0;
#   2. a month that is not now. The version's one promise is that it tells a
#      person how old their wallet is; a release cut in October called 26.9.x
#      breaks it. "Now" is generous — any timezone on Earth — so nobody is
#      refused for releasing on the 1st from Asia or the 30th from Hawaii;
#   3. a skipped or repeated revision: .0 first, then each next one in order.
#
# The month is when the release is CUT, not which line of code it fixes: a fix
# for 26.9.2 made in October is 26.10.0. There are no maintenance branches.
#
# For tests: NOW=<epoch seconds> and TAGS="v26.9.0 v26.9.1" replace the clock
# and `git tag`.
set -euo pipefail

version="${1:-}"
die() { echo "::error::$*" >&2; exit 1; }

[[ "$version" =~ ^([1-9][0-9])\.([1-9]|1[0-2])\.(0|[1-9][0-9]*)$ ]] || die \
  "'$version' is not a Vela version. It is YY.M.REVISION — e.g. 26.9.0 — with no leading zeros (26.09.0 is rejected by cargo and by Chrome) and no suffix."
yy="${BASH_REMATCH[1]}"; mm="${BASH_REMATCH[2]}"; rev="${BASH_REMATCH[3]}"

now="${NOW:-$(date -u +%s)}"
# Earliest and latest local dates on Earth right now: UTC-12 and UTC+14.
month_at() { # epoch -> "YY.M"
  if date -u -d "@$1" +%y.%-m 2>/dev/null; then return; fi
  date -u -r "$1" +%y.%-m                      # BSD date (macOS)
}
early="$(month_at $((now - 12*3600)))"; late="$(month_at $((now + 14*3600)))"
if [[ "$yy.$mm" != "$early" && "$yy.$mm" != "$late" ]]; then
  die "$version says $(printf '20%s-%02d' "$yy" "$mm"), and today is ${late/./ month } (YY M). The version is the month the release is cut: this one is $late.0 or later."
fi

tags="${TAGS-$(git tag -l 'v[0-9]*')}"
has() { grep -qxF "v$1" <<<"$(tr ' ' '\n' <<<"$tags")"; }
if (( rev > 0 )) && ! has "$yy.$mm.$((rev - 1))"; then
  die "$version skips a revision: v$yy.$mm.$((rev - 1)) has not been released. Revisions count up from .0 within a month."
fi
next=$((rev + 1))
if has "$yy.$mm.$next"; then
  die "$version is older than v$yy.$mm.$next, which is already released. The next revision this month comes after the latest one."
fi

if (( rev == 0 )); then words="first release"; else words="revision $rev"; fi
echo "$version — $(printf '20%s-%02d' "$yy" "$mm"), $words"
