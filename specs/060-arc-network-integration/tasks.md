# Tasks: Arc Network Integration

**Input**: Design documents from `specs/060-arc-network-integration/`

**Prerequisites**: [plan.md](./plan.md), [spec.md](./spec.md), [research.md](./research.md), [data-model.md](./data-model.md), [contracts/](./contracts/)

**Tests**: included. The project's coding rules classify this work as **High risk** (money rules + production endpoints), which requires test evidence.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel — different files, no dependency on another unfinished task
- **[Story]**: the user story the task serves

## Path conventions

Three repositories, referred to by short name:

- `wallet/` → `/Volumes/data/production/agent-3/vela-wallet`
- `data/` → `/Volumes/data/production/shelchin-workspace/data/ethereum-data`
- `relay/` → `/Volumes/data/production/vela-relay`

---

## Phase 1: Setup

- [x] T001 Confirm `wrangler whoami` authenticates against the Cloudflare account owning `getvela.app`, and that the account is on a Workers **paid** plan (both services require it — `data/wrangler.toml` documents the 20k static-asset limit; `relay/vela-relay-cf/wrangler.jsonc` documents the CPU ceiling).
- [x] T002 [P] Record the pre-change fee/balance baseline for the twelve existing chains: run `cargo test -p vela-core fee_policy` and save the current parity corpus output as the regression reference for T034.

---

## Phase 2: Foundational — Arc chain data (blocks every story)

**⚠️ Nothing downstream works until Arc's chain file is being served.**

- [x] T003 [P] Create `data/chains/eip155-5042.json` exactly per [contracts/chain-data-arc.md](./contracts/chain-data-arc.md) — empty `stables`, no `dex`, no `wrappedNativeToken`, `nativeCurrency` USDC/18, `features:[RIP7212]`, `contracts.safeProxyFactory`.
- [x] T004 [P] Create `data/chains/eip155-5042002.json` for Arc testnet with the testnet RPCs, faucet and explorer.
- [x] T005 [P] Add `data/chainlogos/eip155-5042.png` (WebP bytes under a `.png` name, matching the corpus).
- [x] T006 Run `bun run build` in `data/` to regenerate `index/` and `erc7730/` so the new chains are searchable, and verify `jq` output matches the contract.
- [x] T007 Assert the two new files never mention `0x3600000000000000000000000000000000000000` and never list EURC or USYC under `stables` — the two mistakes that would double a balance or price euros as dollars.

**Checkpoint**: chain files exist locally and are correct.

---

## Phase 3: User Story 4 — Services move house (Priority: P1)

**Goal**: chain data at `ethereum-data.getvela.app`, relay at `vela-relay-cf.getvela.app`, twelve existing chains unaffected.

**Independent Test**: a clean install pointed at the new defaults reads balances and sends on Ethereum, Base, Gnosis and Tempo with no reference to Arc.

### Deploy

- [x] T008 [US4] Bind the custom domain `ethereum-data.getvela.app` in `data/wrangler.toml` (`routes = [{ pattern = "ethereum-data.getvela.app", custom_domain = true }]`) and deploy with `wrangler deploy`.
- [x] T009 [US4] Verify the deployed service: `/chains/eip155-5042.json` → 5042, `/chains/eip155-1.json` → 1, `/chainlogos/eip155-5042.png` → 200, `/api/health` → `{"service":"ethereum-data"}`.
- [x] T010 [US4] Bind `vela-relay-cf.getvela.app` in `relay/vela-relay-cf/wrangler.jsonc` and deploy; set required secrets out of band via `wrangler secret put` (never in a config file).
- [x] T011 [US4] Verify `https://vela-relay-cf.getvela.app` health reports service identity `vela-relay`, which [network_admin.rs `SERVICE_IDENTITY`](../../rust/crates/vela-core/src/app/network_admin.rs#L166) requires.

### Wallet defaults

- [x] T012 [US4] `wallet/rust/crates/vela-core/src/app/network_admin.rs`: `DEFAULT_ETHEREUM_DATA_URL` → `https://ethereum-data.getvela.app`; `BUNDLER_BASE` and `DEFAULT_BUNDLER_SERVICE_URL` → `https://vela-relay-cf.getvela.app`.
- [x] T013 [P] [US4] Web: `services/endpoints.ts`, `services/networks.ts` (logo base + bundler base), `settings/fixtures.ts`, `services/chain-tokens.ts` (doc comment).
- [x] T014 [P] [US4] iOS: `Features/Settings/NetworkAdminExecutor.swift`, `Features/Settings/SettingsFixtures.swift`, `VelaWalletTests/NetworkAdminLiveTests.swift`.
- [x] T015 [P] [US4] Android: `VelaWalletApplication.kt`, `feature/settings/SettingsFixtures.kt`.
- [x] T016 [P] [US4] Desktop: `settings/fixtures.rs`, `executor/relay.rs` (`BUILTIN_BASE`), `executor/pool.rs`.

### Relay directory host

- [x] T017 [P] [US4] `relay/src/utils/rpc.rs` and `relay/vela-relay-cf/src/arms/market.rs`: chain-directory base → `https://ethereum-data.getvela.app`; update `relay/src/app/rpc/handlers/README.md` to match.

### Tests

- [x] T018 [US4] Test: a stored endpoint override survives the default change untouched (the upgrade path in `network_admin`).
- [x] T019 [US4] Test: service-identity acceptance is unchanged for both relocated endpoints.

**Checkpoint**: both services answer at their new hostnames; existing chains behave as before.

---

## Phase 4: User Story 1 — Arc is a network (Priority: P1) 🎯 MVP

**Goal**: Arc appears, reads balances, and can send.

**Independent Test**: on Arc testnet, switch network, read balance, send, see it on the explorer.

- [x] T020 [US1] `wallet/rust/crates/vela-core/src/app/network_admin.rs`: add the Arc row to `BUILTIN_CHAINS` (`arc` / "Arc" / 5042 / `USDC` / `https://rpc.mainnet.arc.io` / `https://explorer.arc.io` / `typical_inclusion_s: 2`) and the provider slugs (Alchemy `arc-mainnet`, dRPC `arc`, Ankr none) per [data-model.md](./data-model.md).
- [x] T021 [P] [US1] Web: add Arc to `services/chains.ts` (`isL2: false`, `apiNetworkId: 'arc-mainnet'`, icon label/colour) and to `services/rpc-providers.ts`.
- [x] T022 [P] [US1] iOS: add Arc to `Core/ChainCatalog.swift`.
- [x] T023 [P] [US1] Android: add Arc to `feature/wallet/core/ChainData.kt` (and any sibling chain list it feeds).
- [x] T024 [P] [US1] Desktop: add Arc where `executor/chain_tokens.rs` and the settings fixtures enumerate chains; confirm `builtin_dex(5042)` is `None`.
- [x] T025 [US1] Confirm Arc is excluded from the Arbitrum and OP-stack static gas adders in `fee_policy.rs` (it is an L1) and from `is_tempo_chain`.
- [x] T026 [US1] Test: `BUILTIN_CHAINS` contains 13 chains, Arc's fields match the contract, and `5042002` is **not** built in.
- [x] T027 [P] [US1] Cross-shell parity: run `scripts/check-ios-android-parity.mjs` and `scripts/check-android-event-parity.mjs`; fix drift.
- [x] T028 [US1] Update `wallet/README.md` — "12 EVM networks" becomes 13 and names Arc.

**Checkpoint**: Arc is visible and readable on all four shells.

---

## Phase 5: User Story 2 — The fee reads like every other network (Priority: P1)

**Goal**: $1 peg → the universal $0.01 floor; 20 Gwei chain floor honoured even on the fallback path.

**Independent Test**: quote an Arc send twice — once with `eth_gasPrice` reachable, once forced to fail — and confirm both are ≥ 20 Gwei and ≥ $0.01.

### The peg

- [x] T029 [US2] Add `pegged_native_usd(symbol) -> Option<f64>` to `wallet/rust/crates/vela-core/src/app/balance_dashboard.rs` per [contracts/core-exports.md](./contracts/core-exports.md), beside `choose_native_price`.
- [x] T030 [US2] Export it: `vela-core-wasm` as `peggedNativeUsd`, `vela-core-uniffi` as `pegged_native_usd`.
- [x] T031 [P] [US2] Replace the four hard-coded pegs with a call to the core: web `services/price-service.ts`, iOS `Core/Prices.swift`, Android `feature/wallet/core/ChainlinkPrices.kt`, desktop `executor/chainlink.rs`.
- [x] T032 [US2] Test: `USD`/`usd`/`USDC`/`usdc` → 1.0; `ETH`/`XDAI`/empty → none. Test: Tempo's existing behaviour is byte-identical to before.

### The gas floor

- [x] T033 [US2] Add `min_gas_price_wei(chain_id) -> u128` to `wallet/rust/crates/vela-core/src/app/fee_policy.rs` (Arc 20 Gwei, all else 0) and apply it to the derived gas price **and** the `FALLBACK_GAS_PRICE_WEI` path, before any margin.
- [x] T034 [US2] Test (regression, the important one): every one of the twelve existing chains produces gas prices and fee amounts bit-identical to the T002 baseline.
- [x] T035 [US2] Test: on Arc, a failed `eth_gasPrice` read still yields `maxFeePerGas >= 20 Gwei`.
- [x] T036 [US2] Test: an Arc fee quote is never below $0.01 and equals the Gnosis floor for equivalent gas — the requester's "same as other networks" requirement, asserted rather than assumed.

### Relay side

- [x] T037 [US2] Confirm the relay quotes Arc through the ordinary native path (`is_tempo == false` for 5042) and that its admission floor works against an 18-decimal native coin.

**Checkpoint**: Arc fees are honest and Arc operations are accepted by the chain.

---

## Phase 6: User Story 3 — One coin, one row (Priority: P2)

**Goal**: native USDC and its ERC-20 mirror are one balance everywhere.

**Independent Test**: portfolio total equals the on-chain balance, not twice it; add-token refuses the mirror.

- [x] T038 [US3] Add `native_alias_token(chain_id) -> Option<&'static str>` to the core per [contracts/core-exports.md](./contracts/core-exports.md).
- [x] T039 [US3] Wire the refusal into the add-token path with a message naming the native coin ("this is Arc's native coin, already shown"), not a generic validation error.
- [x] T040 [P] [US3] Add the refusal string to all 15 locale catalogs and run `scripts/verify-i18n-parity.mjs`.
- [x] T041 [US3] Test: on 5042, the mirror address is refused; on chain 1 the same address is an ordinary unknown token.
- [x] T042 [US3] Test: an Arc holding appears exactly once in the token list and once in the portfolio total.

---

## Phase 7: Polish — failure legibility and docs

- [~] T043 [P] **Not done, deliberately** (see results.md) — explain issuer-blocklist and destructed-account reverts in the failure path rather than surfacing a generic estimation error (spec FR-017); add the strings to all locales.
- [~] T044 [P] **Not done, deliberately** (see results.md) — ensure anything ordering by block timestamp breaks ties by block number — Arc's sub-second blocks share timestamps (research R7).
- [x] T045 [P] Update `wallet/docs/` where the network list or service endpoints are documented.
- [x] T046 Run the full verification ladder from [quickstart.md](./quickstart.md): typecheck → lint → unit → integration → e2e → build, across core, four shells and the relay.
- [ ] T047 Execute the quickstart §6 testnet end-to-end script on at least one shell, and §7 non-regression on four existing chains.
- [ ] T048 *(real funds, last, once)* Quickstart §8 mainnet smoke test.

---

---

## Phase 8: Found in the browser (parallel space, Arc testnet)

Added after the end-to-end run; each of these was a real defect the earlier verification could not see. See [results.md](./results.md).

- [x] T049 Permissive CORS + preflight on the Cloudflare relay (`vela-relay-cf/src/shell.rs`) — without it the web wallet cannot reach the relay on ANY chain.
- [x] T050 `GET /v1/treasury` and `GET /v1/treasury/{chain_id}` on the Cloudflare relay, with the pure floor/quantity rules moved to `vela_relay_core::treasury` and the docker shell switched to them.
- [x] T051 Rebuild `rust/pkg-web` so the browser runs the core that carries the native-alias rule.
- [x] T052 Treat `native_alias` as a settled probe in the wallet route's re-probe guard, or the refusal re-probes forever.
- [x] T053 Port `GET /v1/account/{chain}/{safe}` to the Cloudflare shell, with its grammar and verdict moved into `vela_relay_core::account` and the docker shell switched to them.
- [x] T054 Arc testnet send completed end to end: 2.25 USDC + 0.166496 fee, Safe deployed in the same operation, ~10 s.
- [x] T055 The out-of-gas sheet says who can fix it — `is_builtin_chain` → `operator_served`, report-first on networks Vela ships, self-funding first on networks the person added. Copy in 15 locales; both paths tested; web, iOS, Android and desktop wired.
- [x] T056 Suggested top-up is `max(shortfall, 0.01)` native coin — a floor-sized ask starts nothing.
- [ ] T057 Give up loudly: surface a dead-lettered operation instead of "waiting for confirmation" forever.
- [ ] T058 Fund the Arc MAINNET treasury, then repeat the smoke test with real funds.

---

## Dependencies

```text
T001,T002 ─► T003..T007 ─► T008..T011 ─► T012..T019 ──┐
                                                       ├─► T020..T028 (US1)
                                                       └─► T029..T037 (US2) ─► T038..T042 (US3) ─► T043..T048
```

- Phase 2 blocks everything: no chain file, no Arc.
- US4's deploy (T008–T011) precedes the wallet default flip (T012–T016), or a clean install points at a host that is not yet serving.
- US1 and US2 can proceed in parallel once US4 lands; US2's parity test (T034) must pass before any release.
- US3 is independent of US2 but reads the same chain data.

## Parallel opportunities

- T003/T004/T005 — three separate files.
- T013–T016 — four shells, four files, same one-line change.
- T021–T024 — four shells, the chain row.
- T031 — four shells, the peg call site (after T030 lands the export).
- T043/T044/T045 — unrelated polish.

## Story completion criteria

| Story | Done when |
|---|---|
| US4 | both hostnames serve; twelve chains unchanged; overrides preserved |
| US1 | Arc visible and readable on all four shells; testnet send lands |
| US2 | Arc fee ≥ $0.01, `maxFeePerGas` ≥ 20 Gwei on both paths, twelve-chain parity green |
| US3 | one USDC row, correct total, mirror refused |
