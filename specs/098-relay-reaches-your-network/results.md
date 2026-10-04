# 098 — Results

Branch `098-relay-reaches-your-network`, from `main` @ `698734b41`; PR #405. The relay half is
vela-relay PR #15 (branch of the same name, from `main` @ `8f4ca52`). Nothing deployed,
nothing released, no phones touched.

## Commits

| Commit | What |
|---|---|
| `b3d9cdfd5` | docs(098): the design |
| `0419e8536` `7bd851bf0` `cfa167fd1` | web, desktop, iOS + Android: `x-vela-rpc-url` on every relay request (FR-001) |
| `36a968b42` | core: the can't-reach stop; the funding stop re-probes every 10 s and moves on by itself (FR-004, FR-008) |
| `6edc503e5` | i18n residency move — approved by the owner 2026-10-03 (below) |
| `3105fe7e3` `9edeafe23` `39c527d7d` `d98763bc8` | web, desktop, Android, iOS: both stops drawn where the core opens them |
| `ae8eb386c` | four shells: "the relay is sent this address" where an RPC or provider key is set (FR-002) |
| `3fabc535d` | getvela.app: privacy policy; networks-and-fees, 15 languages (FR-003, FR-009) |
| `3ae36642e` | phones + desktop: the funding stop gives the address with a copy button (FR-008) |
| `d22fea7ee` | docs: the QR claim corrected, self-hosting a private chain (15 languages), takeover 03/07 |
| (this commit) | spec as built, results |

vela-relay: `df68828` (404 vs 503, host rule in the core, the opt-in) and `7fea898`
(`docs/rpc.md`, README, settings docs).

## Tests, and what fails without them (FR-010)

| Where | Tests | Mutation that fails one |
|---|---|---|
| core `tests/app_send.rs` | 8 new, 4 updated; 203 pass; workspace 2521 | mutated when written (`36a968b42`); each mutation failed a test |
| web `e2e/send-relay-empty.e2e.ts` | 3 (empty float → sheet; 404 → can't-reach sheet; balance line + watching) | mutated when written; failed |
| web unit | `rpc-pool-executor.test.ts`, `bundler-service-relay-rpc.test.ts` | — |
| desktop | `the_treasury_stop_can_be_left` (address copied), `a_relay_that_cannot_serve_the_chain_says_so_and_whose_it_is`, the two header tests; 927 pass | — |
| Android | `SendLiveTest` 46 (form + confirm address), `RelayRpcHeaderTest`, `CoreWireDriftTest` 74 | the stop removed from `form()` → the form test fails |
| iOS | `RelayStopTests` 5, `RelayRpcHeaderTests` 3 | — |
| relay core | `rpc_host` 5, `chain_directory` 2, `treasury` 3 new; 204 pass | drop the refused-RPC rule → `a_refused_wallet_rpc_and_a_silent_directory_is_no_rpc` fails |
| relay docker | 7 new against a local node/directory; 92 pass | drop the not-listed early return → `a_chain_the_directory_does_not_list_is_not_read_at_all` fails |

The relay's Cloudflare crate has no native tests by design (wasm-only); its rule is the
core's, and `cargo clippy -p vela-relay-cf --target wasm32-unknown-unknown` is clean apart
from warnings already on `main`.

## Found while building

- **The phones never showed the treasury address** — on `main` as on this branch. The stop
  said "fund this network's relayer" and not where. Fixed (`3ae36642e`).
- **The docker relay never refused `[::1]`**, and **the Cloudflare relay accepted any
  `http(s)` header URL** — one rule now, in the relay's core.
- **A chain the directory does not list would have read `200`** once the wallet started
  sending its RPC, and failed after signing. The relay asks the directory first.
- **31337 (Anvil) would have stayed `503`**: the directory gives the id to a dead public
  testnet. A refused wallet RPC plus silent directory endpoints is now `no_rpc`.
- **CI**: the §5.1 desktop commit was not `cargo fmt` clean (PR #405's first desktop run);
  fixed in `3ae36642e`.
- `componentsUi.treasuryBootstrap.networkLine` is in the corpus and no shell uses it: the web
  names the network in the sheet's header, the others on the form. Remove it at the next
  corpus regeneration, not worth one of its own.

## What remains for the owner

1. ~~The residency proposal~~ — **approved** 2026-10-03: 「i18n 体积预算上调 统一」. Budget
   141,800 → 144,400, reduction claim 86% → 85.8%.
2. **vela-relay PR #15**: review, merge, deploy both shells. Until then the live relay still
   answers `503` for 1337, 31337 and 123456789, and the wallet carries on through them as
   before — the funding stop works today; the can't-reach stop waits for the deploy. After
   it: `curl -s -w ' %{http_code}\n' <relay>/v1/treasury/123456789` → `not_listed … 404`;
   `…/1337` → `no_rpc … 404`.
3. ~~FR-007~~ — **ruled** 2026-10-03: a chain the directory does not list is not supported
   (「如果目录里没有的，可以不支持」).
