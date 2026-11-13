# Multisig fork harness

Run the web wallet's multisig flows against real contracts: a Gnosis fork, the
genesis service (`safe-genesis-registry`'s docker shell) and a local relay.

| Piece | Where | What |
| --- | --- | --- |
| anvil | `127.0.0.1:8545` | Gnosis fork, `--hardfork osaka` (the P-256 precompile at `0x100` that passkey signatures need) |
| genesis service | `127.0.0.1:8787` | `genesis-server`: the registry (v4, deployed on the fork at start at `0xF412…945d`, the address the wallet knows — up.sh stops if the build lands elsewhere), sponsored records, the proposal mailbox |
| second anvil | `127.0.0.1:8546` | the same Gnosis state answering as Base (8453): a network the wallet has and the service is NOT configured to read |
| relay | `127.0.0.1:8790` | `relay.py`: the ERC-4337 methods and REST endpoints the wallet asks the Vela relay; bundles each operation with `EntryPoint.handleOps` |
| PeerConnect relay | `127.0.0.1:8792` | an outside owner's wallet page and Vela meet through it |
| key index | `127.0.0.1:8793` | `../key-index/server.mjs`: the p256-index v2 protocol, in memory — a run's throwaway accounts are recorded HERE, never in the production index (§28) |

Every service the wallet writes to is local: a test build never writes to
production (docs/multisig-machines.md §28 — and the test builds refuse to).
`.state/harness.env` names them all in the launch inputs the shells read
(`VELA_SERVICE_ENDPOINTS` with `passkeyIndexURL`, `genesisServiceURL` and
`bundlerServiceURL`; `VELA_RPC_OVERRIDES`; `VELA_PEERCONNECT_RELAY`):
`. .state/harness.env` before launching iOS (simctl / XCUITest env) or the
desktop; Android takes `$(cat .state/android-launch-args)` on its
`am start` line.

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

Give the web suite and vela-core's `multisig_fork` test a harness each
(`down.sh && up.sh` between them). The core test sends from the fixture
accounts' addresses (`anvil_impersonateAccount`), which gives them a nonce,
and the fork then refuses to create their counterfactual accounts (AA13
"initCode failed": CREATE2 onto an address with a nonce) — every web test
that deploys one fails for the harness's reason, not the wallet's. And the
web suite spends the sponsor's five records per owner a day, after which the
core test's create is not recorded.

A second harness beside a running one takes other ports — up.sh and the
Playwright harness read the same names (`FORK_PORT`, `UNREAD_PORT`,
`GENESIS_PORT`, `RELAY_PORT`, `PEERCONNECT_PORT`, `KEY_INDEX_PORT`), with its own `STATE`;
`PEERCONNECT_BIN` runs a relay already built instead of building one:

```shell
export FORK_PORT=8645 UNREAD_PORT=8646 GENESIS_PORT=8887 RELAY_PORT=8890 PEERCONNECT_PORT=8892 KEY_INDEX_PORT=8893
STATE=/tmp/harness-2 scripts/onchain/multisig-fork/up.sh
VELA_ONCHAIN_URL=http://127.0.0.1:5291 npx playwright test -c playwright.onchain.config.ts
STATE=/tmp/harness-2 scripts/onchain/multisig-fork/down.sh
```

`e2e/onchain/` drives the parallel space (committed throwaway passkeys) through:

- one device, 2-of-3: create, fund, propose, approve and execute;
- two devices, 2-of-2: one proposes, the other opens the link, approves and
  executes; the first finds the execution on chain and shows it;
- the desktop layout;
- a network the service does not read (the second fork, as Base): propose,
  approve and execute there;
- an outside owner signing through the connect page, and Safes imported from
  a Safe{Wallet} export (one deployed, one not) signed by two outside wallets;
- registry v4: a multisig renames its record on chain — a proposal of its
  own, approved and executed (deploying it on Gnosis) — and the registry and
  the wallet show the new name;
- a multisig the service holds no record of (as one recorded in an earlier
  registry is): said as not shared, and shared again on a member's passkey.

Screenshots land in `e2e/onchain/.shots/`.

To use the harness by hand, point a browser profile at it:

```js
localStorage['vela.genesisServiceURL'] = 'http://127.0.0.1:8787'
localStorage['vela.serviceEndpoints'] = '{"bundlerServiceURL":"http://127.0.0.1:8790","passkeyIndexURL":"http://127.0.0.1:8793"}'
// IndexedDB vela/kv 'vela.networkConfig' =
//   '[{"chainId":100,"rpcURL":"http://127.0.0.1:8545"},{"chainId":8453,"rpcURL":"http://127.0.0.1:8546"}]'
```

The service's sponsor allows 5 owner-proved records per owner per day; a run
past that still works (the mailbox serves the genesis until the record
lands), and `down.sh && up.sh` resets it.
