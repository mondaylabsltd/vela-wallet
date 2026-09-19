# 067 — Results

| | core | web | desktop | Android | iOS |
|---|---|---|---|---|---|
| Shell's own ladder deleted; reads driven by `registry_resolve` | ✅ | ✅ | ✅ | ✅ | ✅ |
| Honest index proved with ONE Gnosis call | test | test | test + **live** | test | test |
| Forged member caught → chain's founding set | test | test | — (core's) | test | test |
| No chain reachable → index's word | test | test | — | test | test |
| Index gone → Gnosis; Gnosis gone → Ethereum under ITS ids | test | test | **live** | test | test |
| Nobody → the INDEX's own failure is reported | test | test | test | test | test |

## How each claim was checked

- **The hash is the contract's own.** `the_hash_is_the_contracts_own` computes
  `contentHashFor` offline over the recorded Gnosis unit and compares it with the
  `contentHash` the chain returned — byte-equal. Live, on desktop:
  `the_live_index_is_proved_by_gnosis` (`#[ignore]`) — the REAL index's unit 10 hashes
  to what REAL Gnosis stores.
- **One fixture, four shells.** `registry-chain.json` (verbatim `eth_call` results from
  both chains, now including the two `getUnitByGroupKey` answers) drives the web,
  Android and iOS tests through the real core; Android serves the index from a real
  local HTTP server, iOS through `URLProtocol`.
- **Android device** (Xiaomi): the NDK-built library with the new exports links and the
  app runs. **No registry sign-in was performed on a phone** — that takes the owner's
  passkey. Same for iOS (simulator suite only) and a web sign-in in a browser.

Suites: core green · desktop green · Android 550 · iOS 608 · web (see CI).

## Not verified

- A real passkey sign-in, any shell, after this change. The read path is the same
  client method as before and is covered by recorded bytes + one live desktop test.
- A forged index has only ever been simulated.
