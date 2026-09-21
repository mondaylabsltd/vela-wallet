#!/usr/bin/env bash
# Every rule of check-release-version.sh, exercised. Run by release.yml's gate
# before it trusts the script, and by hand: scripts/check-release-version.test.sh
set -uo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")"
fails=0
# 2026-09-19 12:00 UTC unless a case says otherwise.
SEP19=1789819200
t() { # expect(ok|no) now tags version
  local want="$1" now="$2" tags="$3" version="$4" got=ok
  NOW="$now" TAGS="$tags" ./check-release-version.sh "$version" >/dev/null 2>&1 || got=no
  if [[ "$got" == "$want" ]]; then printf '  ok    %-10s %s\n' "$version" "${5:-}";
  else printf '  FAIL  %-10s wanted %s, got %s — %s\n' "$version" "$want" "$got" "${5:-}"; fails=1; fi
}
t ok $SEP19 ""                   26.9.0   "first release of the month"
t ok $SEP19 "v26.9.0"            26.9.1   "next revision"
t ok $SEP19 "v0.9.3"             26.9.0   "the old 0.9.x tags do not get in the way"
t no $SEP19 ""                   26.09.0  "zero-padded month (cargo and Chrome reject it)"
t no $SEP19 ""                   2026.9.0 "four-digit year"
t no $SEP19 ""                   26.9     "two segments"
t no $SEP19 ""                   26.9.0-rc1 "suffix"
t no $SEP19 ""                   26.13.0  "no such month"
t no $SEP19 ""                   26.9.01  "zero-padded revision"
t no $SEP19 ""                   1.0.0    "SemVer is over"
t ok $SEP19 "v0.9.3"             0.9.4    "the 0.9 line may finish: next revision"
t ok $SEP19 "v0.9.3 v0.9.4"      0.9.5    "…and the one after"
t no $SEP19 "v0.9.3"             0.9.5    "…but not skip one"
t no $SEP19 "v0.9.3 v0.9.4"      0.9.3    "…or go back"
t no $SEP19 "v0.9.3 v26.9.0"     0.9.4    "once a calendar version exists, 0.9 is over"
t no $SEP19 "v0.9.3"             0.10.0   "the exception is 0.9.N only"
t no $SEP19 ""                   26.10.0  "next month, not yet"
t no $SEP19 ""                   26.8.0   "last month"
t no $SEP19 ""                   26.9.1   "skips .0"
t no $SEP19 "v26.9.0"            26.9.2   "skips .1"
t no $SEP19 "v26.9.0 v26.9.1"    26.9.0   "older than what is released"
# 2026-09-30 20:00 UTC: already October 1st in Asia — both months are "now".
SEP30=$((1790726400 + 20*3600))
t ok $SEP30 ""                   26.10.0  "Oct 1st in Shanghai while UTC says Sep 30"
t ok $SEP30 "v26.9.0"            26.9.1   "…and still September in UTC"
# 2026-10-01 06:00 UTC: still September 30th in Hawaii.
OCT1=$((1790812800 + 6*3600))
t ok $OCT1 "v26.9.0"             26.9.1   "Sep 30th in Honolulu while UTC says Oct 1"
t no $((OCT1 + 86400)) "v26.9.0" 26.9.1   "Oct 2nd everywhere: September is over"
# 2027-01-05
t ok 1799150400 ""               27.1.0   "a new year"
exit $fails
