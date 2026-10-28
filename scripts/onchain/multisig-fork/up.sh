#!/bin/sh
# The multisig fork harness: a Gnosis fork, the genesis service and a local
# relay, wired so the web wallet's multisig flows run against real contracts.
#
#   GENESIS_REPO   path to safe-genesis-registry (default ../../../../production3/safe-genesis-registry)
#   STATE          where logs and the service's database go (default ./.state)
#
# A second fork answers as Base (8453) on :8546: Gnosis's state under another
# chain id, which the genesis service is NOT configured to read — a network the
# wallet has and the service does not, where the mailbox judges by genesis.
#
# Then point the browser at it (the dev console, or the Playwright harness):
#   localStorage['vela.genesisServiceURL'] = 'http://127.0.0.1:8787'
#   localStorage['vela.serviceEndpoints']  = '{"bundlerServiceURL":"http://127.0.0.1:8790"}'
#   IndexedDB vela/kv 'vela.networkConfig' = '[{"chainId":100,"rpcURL":"http://127.0.0.1:8545"},
#                                              {"chainId":8453,"rpcURL":"http://127.0.0.1:8546"}]'
set -eu
here=$(cd "$(dirname "$0")" && pwd)
GENESIS_REPO=${GENESIS_REPO:-"$here/../../../../../production3/safe-genesis-registry"}
STATE=${STATE:-"$here/.state"}
RPC=http://127.0.0.1:8545
UNREAD=http://127.0.0.1:8546
mkdir -p "$STATE"

# The P-256 precompile (0x100) that passkey signatures need exists from Osaka.
anvil --fork-url "${FORK_URL:-https://rpc.gnosischain.com}" --port 8545 --hardfork osaka --silent \
  >"$STATE/anvil.log" 2>&1 &
echo $! >"$STATE/anvil.pid"
anvil --fork-url "${FORK_URL:-https://rpc.gnosischain.com}" --port 8546 --chain-id 8453 --hardfork osaka --silent \
  >"$STATE/anvil-unread.log" 2>&1 &
echo $! >"$STATE/anvil-unread.pid"
for r in $RPC $UNREAD; do
  i=0; until cast block-number --rpc-url $r >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 60 ] && { echo "anvil did not start ($r)"; exit 1; }; sleep 1; done
done

fund() { for r in $RPC $UNREAD; do cast rpc anvil_setBalance "$1" 0x3635C9ADC5DEA00000 --rpc-url $r >/dev/null; done; }  # 1000 of the native coin
# The service's sponsor (anvil #0), the relay's bundler (relay.py), and the parallel
# space's fixture accounts (the committed throwaway keys, src/lib/dev/passkey-fixture.ts).
for a in 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 0x57AB0996DC98fbE8Ad58681793Da0312Ef76c404 \
         0xD400866e00B055B20752a826CD5C89b811de130b 0x031d7D57c99CAF891e1C250554691Fd12D84772b \
         0x58cd0ce6A27099220543b31710d7860d75Ba1d3d 0x88cCA0EeDbF2C4426110bbFc998F048689266894; do
  fund "$a"
done

(cd "$GENESIS_REPO" && cargo build -q -p genesis-server)
rm -f "$STATE"/genesis.sqlite*
REGISTRY_ADDRESS=0xdA88357C2eB85e7e0515B1122a28befd6Dc8fb89 HOME_CHAIN_ID=100 \
CHAINS='[{"chainId":100,"name":"Gnosis (fork)","rpcs":["http://127.0.0.1:8545"]}]' \
PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
MAX_FEE_WEI=100000000000 LISTEN_ADDR=127.0.0.1:8787 DATABASE_PATH="$STATE/genesis.sqlite" \
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost \
  "$GENESIS_REPO/target/debug/genesis-server" >"$STATE/genesis.log" 2>&1 &
echo $! >"$STATE/genesis.pid"

RPCS="{\"100\":\"$RPC\",\"8453\":\"$UNREAD\"}" RELAY_PORT=8790 NO_PROXY=127.0.0.1 no_proxy=127.0.0.1 \
  python3 "$here/relay.py" >"$STATE/relay.log" 2>&1 &
echo $! >"$STATE/relay.pid"

i=0; until curl -s --noproxy '*' http://127.0.0.1:8787/api/health >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 60 ] && { echo "genesis service did not start"; exit 1; }; sleep 1; done
echo "fork $RPC · unread fork (8453) $UNREAD · genesis http://127.0.0.1:8787 · relay http://127.0.0.1:8790 · logs $STATE"
