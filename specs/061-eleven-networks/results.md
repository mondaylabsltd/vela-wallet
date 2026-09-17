# Results — eleven networks

## Shipped

- Core: 11 rows in `BUILTIN_CHAINS` (24 total), verified Alchemy/dRPC slugs (none for XRPL EVM), `USDT0` peg, Stable's native alias, Orbit/OP adders.
- Shells: web `chains.ts` + `rpc-providers.ts`; iOS `ChainCatalog.swift` (rows + slugs); Android `SendLive.kt` name→symbol; desktop needs nothing (its DEX table falls through to the directory).
- Chain data: Stable's `USDT0` mirror removed from `stables`; redeployed.
- Relay: Plume in the Alchemy map; Stable pegged; redeployed.
- README: 13 → 24.

## Verified

| Gate | Result |
|---|---|
| `cargo test -p vela-core --features crux,i18n-all` | ✅ 38 suites |
| Desktop `cargo test` | ✅ 395 |
| Web `vitest` | ✅ 995 pass; the same 3 pre-existing failures as before this work |
| Web `svelte-check` | ✅ 0 errors |
| Cross-shell parity gates ×4 | ✅ |
| iOS build, Android unit tests | see the notes below |

One test needed a new fixture, not a new assertion: `networks.svelte.test.ts` used **Celo** as its example of a *custom* network. Celo is built in now, so the fixture is Linea.

## Still to do (operations, not code)

1. Fund the relay treasury on each of the eleven, in that chain's coin. Until then the wallet's out-of-gas sheet says so and points at the operator.
2. A real send on Mantle before it is announced — its gas metering is the one shape in this set the wallet has not been observed against.
