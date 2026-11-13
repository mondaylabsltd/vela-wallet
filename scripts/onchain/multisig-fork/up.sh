#!/bin/sh
# The multisig fork harness: a Gnosis fork, the genesis service and a local
# relay, wired so the web wallet's multisig flows run against real contracts.
#
#   GENESIS_REPO   path to safe-genesis-registry (default <checkout>/../../production3/safe-genesis-registry)
#   PEERCONNECT_REPO  path to peerconnect (default <checkout>/../../production3/peerconnect): its relay carries
#                  an outside owner's session (ws://127.0.0.1:8792)
#                  <checkout> is the main checkout (git's common dir), so a git
#                  worktree of the wallet, however deep, finds the same repos.
#   PEERCONNECT_BIN   a peerconnect-relay already built: run it instead of building PEERCONNECT_REPO
#   STATE          where logs and the service's database go (default ./.state)
#   FORK_PORT UNREAD_PORT GENESIS_PORT RELAY_PORT PEERCONNECT_PORT KEY_INDEX_PORT
#                  the ports (default 8545 8546 8787 8790 8792 8793). A second
#                  harness beside a running one takes others; the Playwright
#                  harness reads the same names (e2e/onchain/harness.ts).
#
# EVERY service the wallet writes to has a local stand-in here — the genesis
# service and its mailbox, the relay (bundler and sponsor), PeerConnect's
# relay, and a key index (`../key-index/server.mjs`, in memory): a test build
# never writes to production (§28, docs/multisig-machines.md), and a run that
# creates an account creates it HERE. `$STATE/harness.env` names them all, in
# the launch inputs every shell reads (`VELA_SERVICE_ENDPOINTS`,
# `VELA_RPC_OVERRIDES`, `VELA_PEERCONNECT_RELAY` — iOS's DevLaunchInputs, the
# desktop's dev env; Android's `--es vela.serviceEndpoints …` from
# `$STATE/android-launch-args`): source it, and nothing points at production.
#
# A second fork answers as Base (8453) on :8546: Gnosis's state under another
# chain id, which the genesis service is NOT configured to read — a network the
# wallet has and the service does not, where the mailbox judges by genesis.
#
# The service's rate limits are set out of reach: a suite's writes all come
# from one address, and a limit hit there is the wallet's 429 path, unit-tested
# on its own.
#
# Then point the browser at it (the dev console, or the Playwright harness):
#   localStorage['vela.genesisServiceURL'] = 'http://127.0.0.1:8787'
#   localStorage['vela.serviceEndpoints']  = '{"bundlerServiceURL":"http://127.0.0.1:8790","passkeyIndexURL":"http://127.0.0.1:8793"}'
#   IndexedDB vela/kv 'vela.networkConfig' = '[{"chainId":100,"rpcURL":"http://127.0.0.1:8545"},
#                                              {"chainId":8453,"rpcURL":"http://127.0.0.1:8546"}]'
set -eu
here=$(cd "$(dirname "$0")" && pwd)
# The main checkout: git's common dir is its .git, from a worktree too (a
# worktree's own top level sits under .claude/worktrees/…). Without git, this
# script's own repository.
checkout=$(git -C "$here" rev-parse --path-format=absolute --git-common-dir 2>/dev/null || true)
case "$checkout" in
  */.git) checkout=${checkout%/.git} ;;
  *) checkout=$(git -C "$here" rev-parse --show-toplevel 2>/dev/null || (cd "$here/../../.." && pwd)) ;;
esac
GENESIS_REPO=${GENESIS_REPO:-"$checkout/../../production3/safe-genesis-registry"}
PEERCONNECT_REPO=${PEERCONNECT_REPO:-"$checkout/../../production3/peerconnect"}
STATE=${STATE:-"$here/.state"}
FORK_PORT=${FORK_PORT:-8545}
UNREAD_PORT=${UNREAD_PORT:-8546}
GENESIS_PORT=${GENESIS_PORT:-8787}
RELAY_PORT=${RELAY_PORT:-8790}
PEERCONNECT_PORT=${PEERCONNECT_PORT:-8792}
KEY_INDEX_PORT=${KEY_INDEX_PORT:-8793}
RPC=http://127.0.0.1:$FORK_PORT
UNREAD=http://127.0.0.1:$UNREAD_PORT
mkdir -p "$STATE"

# The P-256 precompile (0x100) that passkey signatures need exists from Osaka.
anvil --fork-url "${FORK_URL:-https://rpc.gnosischain.com}" --port "$FORK_PORT" --hardfork osaka --silent \
  >"$STATE/anvil.log" 2>&1 &
echo $! >"$STATE/anvil.pid"
anvil --fork-url "${FORK_URL:-https://rpc.gnosischain.com}" --port "$UNREAD_PORT" --chain-id 8453 --hardfork osaka --silent \
  >"$STATE/anvil-unread.log" 2>&1 &
echo $! >"$STATE/anvil-unread.pid"
for r in $RPC $UNREAD; do
  i=0; until cast block-number --rpc-url $r >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 60 ] && { echo "anvil did not start ($r)"; exit 1; }; sleep 1; done
done

# Warm the forks: the first read of a contract fetches it from the public RPC
# (seconds each), longer than the genesis service waits for an answer.
warming=""
for r in $RPC $UNREAD; do
  for a in 0xa6B71E26C5e0845f74c812102Ca7114b6A896AB2 0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67 \
           0x14F2982D601c9458F93bD70B218933A6f8165e7b 0x3E5c63644E683549055b9Be8653de26E0B4CD36E \
           0xd9Db270c1B5E3Bd161E8c8503c55cEABeE709552 0x29fcB43b46531BcA003ddC8FCB67FFE91900C762 \
           0x41675C099F32341bf84BFc5382aF534df5C7461a 0xfd0732Dc9E303f09fCEf3a7388Ad10A83459Ec99 \
           0xf48f2B2d2a534e402487b3ee7C18c33Aec0Fe5e4 0x9641d764fc13c8B624c04430C7356C1C7C8102e2 \
           0x0000000071727De22E5E9d8BAf0edAc6f37da032 0x75cf11467937ce3F2f357CE24ffc3DBF8fD5c226 \
           0x2dd68b007B46fBe91B9A7c3EDa5A7a1063cB5b47 0x94a4F6affBd8975951142c3999aEAB7ecee555c2 \
           0x38869bf66a61cF6bDB996A6aE40D5853Fd43B526 0xA42f6c68d9e0E13ac0910837356c0A0155413E8C; do
    cast code "$a" --rpc-url $r >/dev/null 2>&1 &
    warming="$warming $!"
  done
  for f in 0xa6B71E26C5e0845f74c812102Ca7114b6A896AB2 0x4e1DCf7AD4e460CfD30791CCC4F9c8a4f820ec67 0x14F2982D601c9458F93bD70B218933A6f8165e7b; do
    cast call "$f" "proxyCreationCode()(bytes)" --rpc-url $r >/dev/null 2>&1 &
    warming="$warming $!"
  done
done
# Only the warm-up: a bare `wait` would wait for the forks themselves.
for p in $warming; do wait "$p" 2>/dev/null; done

fund() { for r in $RPC $UNREAD; do cast rpc anvil_setBalance "$1" 0x3635C9ADC5DEA00000 --rpc-url $r >/dev/null; done; }  # 1000 of the native coin
# The service's sponsor (anvil #0), the relay's bundler (relay.py), and the parallel
# space's fixture accounts (the committed throwaway keys, src/lib/dev/passkey-fixture.ts).
for a in 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 0x57AB0996DC98fbE8Ad58681793Da0312Ef76c404 \
         0xD400866e00B055B20752a826CD5C89b811de130b 0x031d7D57c99CAF891e1C250554691Fd12D84772b \
         0x58cd0ce6A27099220543b31710d7860d75Ba1d3d 0x88cCA0EeDbF2C4426110bbFc998F048689266894; do
  fund "$a"
done

# The registry (v4: every record named, a Safe renames itself), through the
# deterministic deployer with the production salt, as on any chain: the
# address depends only on its code. It must be the address the wallet knows
# (vela_core::multisig::GENESIS_REGISTRY), or the run stops here.
WALLET_REGISTRY=0xF412388bF908E510066d3Be5c704bd9A327f945d
INITCODE=$(cd "$GENESIS_REPO/contracts" && forge inspect SafeGenesisRegistry bytecode)
SALT=$(cast keccak "safe-genesis-registry/v4")  # script/Deploy.s.sol's SALT
DEPLOYER=0x4e59b44847b379578588920cA78FbF26c0B4956C
REGISTRY=$(cast create2 --deployer $DEPLOYER --salt "$SALT" --init-code "$INITCODE" 2>/dev/null | tail -1 | awk '{print $NF}')
[ "$(echo "$REGISTRY" | tr 'A-F' 'a-f')" = "$(echo "$WALLET_REGISTRY" | tr 'A-F' 'a-f')" ] || {
  echo "this registry build lands at $REGISTRY, not the wallet's $WALLET_REGISTRY (another version?)"; exit 1; }
[ "$(cast code "$REGISTRY" --rpc-url $RPC)" != "0x" ] || cast send $DEPLOYER "$SALT${INITCODE#0x}" --rpc-url $RPC \
  --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 >/dev/null
[ "$(cast code "$REGISTRY" --rpc-url $RPC)" != "0x" ] || { echo "registry did not deploy"; exit 1; }

(cd "$GENESIS_REPO" && cargo build -q -p genesis-server)
rm -f "$STATE"/genesis.sqlite*
REGISTRY_ADDRESS=$REGISTRY HOME_CHAIN_ID=100 \
CHAINS='[{"chainId":100,"name":"Gnosis (fork)","rpcs":["'"$RPC"'"],"txService":"https://api.safe.global/tx-service/gno","registry":true}]' \
PRIVATE_KEY=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 \
MAX_FEE_WEI=100000000000 LISTEN_ADDR=127.0.0.1:$GENESIS_PORT DATABASE_PATH="$STATE/genesis.sqlite" \
READS_PER_MINUTE=100000 WRITES_PER_MINUTE_PER_IP=100000 WRITES_PER_MINUTE_PER_SAFE=100000 WRITES_PER_MINUTE_PER_SIGNER=100000 \
NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost \
  "$GENESIS_REPO/target/debug/genesis-server" >"$STATE/genesis.log" 2>&1 &
echo $! >"$STATE/genesis.pid"

# The PeerConnect relay an outside owner's wallet page and Vela meet through.
if [ -z "${PEERCONNECT_BIN:-}" ]; then
  (cd "$PEERCONNECT_REPO" && cargo build -q -p peerconnect-relay)
  PEERCONNECT_BIN="$PEERCONNECT_REPO/target/debug/peerconnect-relay"
fi
PEERCONNECT_LISTEN=127.0.0.1:$PEERCONNECT_PORT "$PEERCONNECT_BIN" >"$STATE/peerconnect.log" 2>&1 &
echo $! >"$STATE/peerconnect.pid"

RPCS="{\"100\":\"$RPC\",\"8453\":\"$UNREAD\"}" RELAY_PORT=$RELAY_PORT NO_PROXY=127.0.0.1 no_proxy=127.0.0.1 \
  python3 "$here/relay.py" >"$STATE/relay.log" 2>&1 &
echo $! >"$STATE/relay.pid"

# The key index: where a run's throwaway accounts are recorded (§28 — never
# the production index). In memory; `down.sh && up.sh` empties it.
[ -d "$here/../../node_modules/@noble/hashes" ] || npm --prefix "$here/../.." ci --silent >/dev/null
KEY_INDEX_PORT=$KEY_INDEX_PORT node "$here/../key-index/server.mjs" >"$STATE/key-index.log" 2>&1 &
echo $! >"$STATE/key-index.pid"
KEY_INDEX=http://127.0.0.1:$KEY_INDEX_PORT
i=0; until curl -s --noproxy '*' "$KEY_INDEX/api/health" >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 30 ] && { echo "the key index did not start"; exit 1; }; sleep 1; done

# Every service, local — what a shell is launched with (§28).
ENDPOINTS="{\"passkeyIndexURL\":\"$KEY_INDEX\",\"genesisServiceURL\":\"http://127.0.0.1:$GENESIS_PORT\",\"bundlerServiceURL\":\"http://127.0.0.1:$RELAY_PORT\"}"
RPC_OVERRIDES="{\"100\":\"$RPC\",\"8453\":\"$UNREAD\"}"
PEER=ws://127.0.0.1:$PEERCONNECT_PORT
cat >"$STATE/harness.env" <<EOF
# Written by scripts/onchain/multisig-fork/up.sh: every service local (§28).
VELA_SERVICE_ENDPOINTS='$ENDPOINTS'
VELA_RPC_OVERRIDES='$RPC_OVERRIDES'
VELA_PEERCONNECT_RELAY='$PEER'
export VELA_SERVICE_ENDPOINTS VELA_RPC_OVERRIDES VELA_PEERCONNECT_RELAY
EOF
printf "%s\n" "--es vela.serviceEndpoints '$ENDPOINTS' --es vela.rpcOverrides '$RPC_OVERRIDES' --es vela.peerconnectRelay $PEER" \
  >"$STATE/android-launch-args"

health=http://127.0.0.1:$GENESIS_PORT/api/health
i=0; until curl -s --noproxy '*' "$health" >/dev/null 2>&1; do i=$((i+1)); [ $i -gt 60 ] && { echo "genesis service did not start"; exit 1; }; sleep 1; done
# The service reads only its own version of the registry: said now, not at the first lookup.
curl -s --noproxy '*' "$health" | grep -q '"status":"ok"' || {
  echo "the genesis service does not read the registry at $REGISTRY:"; curl -s --noproxy '*' "$health"; exit 1; }
echo "registry $REGISTRY · peerconnect ws://127.0.0.1:$PEERCONNECT_PORT · fork $RPC · unread fork (8453) $UNREAD · genesis http://127.0.0.1:$GENESIS_PORT · relay http://127.0.0.1:$RELAY_PORT · key index $KEY_INDEX · logs $STATE"
echo "launch inputs: . $STATE/harness.env (iOS, desktop) · adb … am start … \$(cat $STATE/android-launch-args)"
