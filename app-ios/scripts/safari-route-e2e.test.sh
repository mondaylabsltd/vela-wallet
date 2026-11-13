#!/usr/bin/env bash
# safari-route-e2e.sh's environment hand-off, exercised with xcodebuild and
# xcrun stubbed (nothing is built, no simulator is touched):
#   app-ios/scripts/safari-route-e2e.test.sh
# r15-16: MS_NAME reaches the test runner as TEST_RUNNER_VELA_MS_NAME, and an
# unset MS_NAME passes nothing (the test's own default name stands).
set -uo pipefail
here=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/bin"
cat >"$work/bin/xcodebuild" <<'EOF'
#!/usr/bin/env bash
env | grep '^TEST_RUNNER_VELA_' | sort >"$STUB_ENV_OUT"
echo "Test Case '-[stub]' passed (0.001 seconds)."
EOF
printf '#!/usr/bin/env bash\nexit 0\n' >"$work/bin/xcrun"
chmod +x "$work/bin/xcodebuild" "$work/bin/xcrun"

fails=0
run() { # name expect-line|"-" [MS_NAME]
  local name="$1" want="$2" out="$work/$1.env" code
  (
    export PATH="$work/bin:$PATH" STUB_ENV_OUT="$out" UDID=stub DD="$work/dd" SHOTS="$work/shots/$name" \
      NOBUILD=1 FORK_PORT=1 UNREAD_PORT=2 GENESIS_PORT=3 RELAY_PORT=4 PEERCONNECT_PORT=5 \
      PAGE_PORT=6 INDEX_PORT=7 DAPP_PORT=8
    unset MS_NAME
    [ $# -ge 3 ] && export MS_NAME="$3"
    "$here/safari-route-e2e.sh" test07_createMultisigFromTheHeader >"$work/$name.log" 2>&1
  )
  code=$?
  if [ $code -ne 0 ]; then
    printf '  FAIL  %s: exit %s\n' "$name" "$code"; sed 's/^/        /' "$work/$name.log"; fails=1; return
  fi
  if [ "$want" = "-" ]; then
    if grep -q '^TEST_RUNNER_VELA_MS_NAME=' "$out"; then
      printf '  FAIL  %s: passed a name nobody gave\n' "$name"; fails=1; return
    fi
  elif ! grep -qx "$want" "$out"; then
    printf '  FAIL  %s: wanted %s in\n' "$name" "$want"; sed 's/^/        /' "$out"; fails=1; return
  fi
  printf '  ok    %s\n' "$name"
}
run no-name -
run a-name 'TEST_RUNNER_VELA_MS_NAME=R15 Team' 'R15 Team'
run plain 'TEST_RUNNER_VELA_SAFARI_LOCALE=en'
exit $fails
