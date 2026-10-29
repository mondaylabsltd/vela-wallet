#!/usr/bin/env bash
# Rebuild SafeStateProbe's runtime code into src/multisig_probe_code.hex (what the core
# puts at a fresh address in the eth_call state override). Needs Foundry (forge).
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
mkdir -p "$work/src"
cp "$here/SafeStateProbe.sol" "$work/src/"
cd "$work"
forge build --use 0.8.26 --optimize --optimizer-runs 200 --evm-version paris --no-metadata -q src/SafeStateProbe.sol
code=$(jq -r '.deployedBytecode.object' out/SafeStateProbe.sol/SafeStateProbe.json)
printf '%s\n' "$code" >"$here/../src/multisig_probe_code.hex"
echo "SafeStateProbe runtime: $(((${#code} - 2) / 2)) bytes"
