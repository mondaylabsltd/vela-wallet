# Multisig fork harness

Run the web wallet's multisig flows against real contracts: a Gnosis fork, the
genesis service (`safe-genesis-registry`'s docker shell) and a local relay.

| Piece | Where | What |
| --- | --- | --- |
| anvil | `127.0.0.1:8545` | Gnosis fork, `--hardfork osaka` (the P-256 precompile at `0x100` that passkey signatures need) |
| genesis service | `127.0.0.1:8787` | `genesis-server`: the registry at `0xdA88…fb89`, sponsored records, the proposal mailbox |
| relay | `127.0.0.1:8790` | `relay.py`: the ERC-4337 methods and REST endpoints the wallet asks the Vela relay; bundles each operation with `EntryPoint.handleOps` |

The relay pays from a fresh key, not anvil's defaults: on Gnosis those
addresses carry EIP-7702 delegations, and a fee paid to one reverts the whole
bundle (`AA91`).

## Run

```shell
scripts/onchain/multisig-fork/up.sh          # GENESIS_REPO=… if not ../../production3/safe-genesis-registry
cd app-web/vela-wallet
pnpm dev --host 127.0.0.1 --port 5173 &
npx playwright test -c playwright.onchain.config.ts
scripts/onchain/multisig-fork/down.sh
```

`e2e/onchain/` drives the parallel space (committed throwaway passkeys) through:

- one device, 2-of-3: create, fund, propose, approve and execute;
- two devices, 2-of-2: one proposes, the other opens the link, approves and
  executes; the first finds the execution on chain and shows it;
- the desktop layout.

Screenshots land in `e2e/onchain/.shots/`.

To use the harness by hand, point a browser profile at it:

```js
localStorage['vela.genesisServiceURL'] = 'http://127.0.0.1:8787'
localStorage['vela.serviceEndpoints'] = '{"bundlerServiceURL":"http://127.0.0.1:8790"}'
// IndexedDB vela/kv 'vela.networkConfig' = '[{"chainId":100,"rpcURL":"http://127.0.0.1:8545"}]'
```

The service's sponsor allows 5 owner-proved records per owner per day; a run
past that still works (the mailbox serves the genesis until the record
lands), and `down.sh && up.sh` resets it.
