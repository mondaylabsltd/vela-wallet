#!/usr/bin/env bash
# The chain MultisigHermeticUITests run their multisig on (LocalMultisigWorld):
# a local anvil — no fork, no public network — as Gnosis: chain id 100, Osaka
# (the P-256 precompile passkeys need), a block every 5 s as Gnosis makes them.
# The world resets it and places the contracts at the start of every test.
#
#   app-ios/scripts/ui-anvil.sh            start it (port 8571), print the export
#   app-ios/scripts/ui-anvil.sh stop       stop it
#
# Then run the UI tests with TEST_RUNNER_VELA_UI_ANVIL_URL set, as printed.
set -euo pipefail
PORT="${VELA_UI_ANVIL_PORT:-8571}"
PIDFILE="${TMPDIR:-/tmp}/vela-ui-anvil-$PORT.pid"

if [ "${1:-}" = stop ]; then
  [ -f "$PIDFILE" ] && kill "$(cat "$PIDFILE")" 2>/dev/null || true
  rm -f "$PIDFILE"
  exit 0
fi

command -v anvil >/dev/null || { echo "ui-anvil: anvil not found — install Foundry (https://getfoundry.sh)" >&2; exit 1; }
anvil --port "$PORT" --chain-id 100 --hardfork osaka --block-time 5 --silent \
  >"${TMPDIR:-/tmp}/vela-ui-anvil-$PORT.log" 2>&1 &
echo $! >"$PIDFILE"
URL="http://127.0.0.1:$PORT"
for _ in $(seq 1 30); do
  if curl -s -X POST -H 'content-type: application/json' \
       --data '{"jsonrpc":"2.0","id":1,"method":"eth_chainId","params":[]}' "$URL" | grep -q '"0x64"'; then
    echo "export TEST_RUNNER_VELA_UI_ANVIL_URL=$URL"
    exit 0
  fi
  sleep 1
done
echo "ui-anvil: anvil did not answer on $URL" >&2
exit 1
