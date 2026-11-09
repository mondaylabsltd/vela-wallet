#!/usr/bin/env bash
#
# The trusted-signer page suites (app-web/trusted-signer/HANDOVER.md, "先跑一遍"),
# one after another, each into its own log — the list the handover keeps,
# with the runtime it names (the browser suites need bun: node's fetch refuses
# the page server's self-signed certificate).
#
#   CHROME_BIN=<Chrome for Testing> scripts/ci/trusted-signer-suites.sh <out-dir> [suite …]
#
# SB defaults to <out-dir>/sb and is filled by trusted-signer-sb.sh. Every
# suite runs even after one fails; the exit code is 1 when any did. A summary
# goes to <out-dir>/summary.md (and to $GITHUB_STEP_SUMMARY in Actions).
#
# Suites that need something the machine lacks say so and count as SKIPPED,
# never as passed: gen-fonts --check needs Python's fontTools.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PAGE="$ROOT/app-web/trusted-signer"
OUT="${1:?usage: trusted-signer-suites.sh <out-dir> [suite …]}"
shift
mkdir -p "$OUT"
OUT="$(cd "$OUT" && pwd)"
export SB="${SB:-$OUT/sb}"
"$ROOT/scripts/ci/trusted-signer-sb.sh" "$SB" >/dev/null
: "${CHROME_BIN:?set CHROME_BIN to a Chrome for Testing binary}"
[[ -x "$CHROME_BIN" ]] || { echo "CHROME_BIN is not executable: $CHROME_BIN" >&2; exit 2; }

# runtime  script                 args
SUITES=(
  "bun  answer-test.mjs"
  "bun  safeop-test.mjs"
  "bun  identicon-test.mjs"
  "bun  hostile-test.mjs"
  "bun  channels-test.mjs"
  "bun  ceremony-test.mjs"
  "bun  confirm-test.mjs"
  "node key-route-test.mjs"
  "node locales-test.mjs"
  "bun  gen-catalog.mjs --check"
  "node gen-fonts.mjs --check"
  "bun  single-file-test.mjs"
  "node sw-pin-test.mjs"
  "bun  desktop-demo.mjs --auto"
  "bun  takeover-test.mjs"
  "node multisig-test.mjs"
  "bun  unlimited-line-test.mjs"
  "node origin-line-test.mjs"
  "node plain-send-test.mjs"
  "bun  plain-send-test.mjs"
  "node claims-test.mjs"
  "node provenance-test.mjs"
  "bun  provenance-test.mjs"
)

# A hung suite must not hold the job for its whole timeout: 15 minutes each
# (GNU timeout on Linux, gtimeout from coreutils on a Mac, else none).
TIMEOUT=()
if command -v timeout >/dev/null; then TIMEOUT=(timeout 900)
elif command -v gtimeout >/dev/null; then TIMEOUT=(gtimeout 900); fi

pass=0 fail=0 skip=0
rows=()
cd "$PAGE" || exit 2
for entry in "${SUITES[@]}"; do
  read -r runtime script args <<<"$entry"
  if [[ $# -gt 0 ]]; then
    hit=0
    for w in "$@"; do [[ "$script" == "$w" || "$script" == "$w.mjs" ]] && hit=1; done
    [[ $hit == 1 ]] || continue
  fi
  name="${script%.mjs}-${runtime}"
  log="$OUT/$name.log"
  if [[ "$script" == gen-fonts.mjs ]] && ! python3 -c 'import fontTools' 2>/dev/null; then
    echo "SKIP $name: needs Python's fontTools (pip install fonttools)" | tee "$log"
    rows+=("| \`$runtime samples/$script $args\` | SKIPPED (no fontTools) | |")
    skip=$((skip + 1))
    continue
  fi
  t0=$SECONDS
  # shellcheck disable=SC2086
  ${TIMEOUT[@]+"${TIMEOUT[@]}"} "$runtime" "samples/$script" $args >"$log" 2>&1
  status=$?
  took=$((SECONDS - t0))
  # The last "n/m checks passed"-shaped line the suite printed, if any.
  tally="$(grep -Eo '[0-9]+/[0-9]+ ?(checks passed|passed|项通过)' "$log" | tail -1 || true)"
  if [[ $status == 0 ]]; then
    pass=$((pass + 1))
    echo "ok   $name (${took}s) $tally"
    rows+=("| \`$runtime samples/$script $args\` | ok | $tally |")
  else
    fail=$((fail + 1))
    echo "FAIL $name (exit $status, ${took}s) $tally — $log"
    tail -20 "$log" | sed 's/^/     /'
    rows+=("| \`$runtime samples/$script $args\` | **FAILED** (exit $status) | $tally |")
  fi
done

{
  echo "### Trusted-signer page suites"
  echo
  echo "$pass passed, $fail failed, $skip skipped."
  echo
  echo "| Suite | Result | Tally |"
  echo "|---|---|---|"
  printf '%s\n' "${rows[@]}"
} >"$OUT/summary.md"
[[ -n "${GITHUB_STEP_SUMMARY:-}" ]] && cat "$OUT/summary.md" >>"$GITHUB_STEP_SUMMARY"
echo "trusted-signer suites: $pass passed, $fail failed, $skip skipped (logs: $OUT)"
[[ $fail == 0 ]]
