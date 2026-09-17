# Implementation Plan: Arc Network Integration

**Branch**: `060-arc-network-integration` | **Date**: 2026-09-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/060-arc-network-integration/spec.md`

## Summary

Add Circle's Arc (chain `5042`) as the thirteenth built-in network, and relocate the two self-hosted services it depends on to Cloudflare under `getvela.app`.

Reconnaissance (see [research.md](./research.md)) established that Arc needs **nothing** from the signing path: all eleven required contracts and the RIP-7212 precompile are live at canonical addresses on mainnet and testnet, so Safe addresses, passkey verification and recovery work as they do everywhere else. The real work is three narrow rules plus a service move:

1. **A pegged native price.** Arc's gas coin is USDC. There is no DEX and no Chainlink feed to quote it, so it must be pegged at $1 — and the peg belongs in the core, once, replacing the `"USD" ⇒ $1` literal currently copied into four shells. With a price in hand, Arc automatically inherits the same $0.01 fee floor every other network already applies; no Arc-specific fee code exists anywhere in this plan.
2. **A per-chain minimum gas price.** Arc *silently discards* any transaction under 20 Gwei. The wallet's 5 Gwei static fallback would produce exactly that, so a floor must be data the fee machine consults, not a branch.
3. **One coin, one row.** Native USDC and its 6-decimal ERC-20 interface at `0x3600…0000` are one balance; the chain data must not list the mirror as a token, and the add-token path must refuse it.
4. **The service move.** `ethereum-data.getvela.app` and `vela-relay-cf.getvela.app` become the built-in endpoints for every chain, with stored user overrides untouched.

## Technical Context

**Language/Version**: Rust 1.x (shared core `vela-core`, desktop shell, relay), TypeScript 5 / SvelteKit 2 (web shell), Swift 5.9+ / SwiftUI (iOS), Kotlin / Jetpack Compose (Android), TypeScript + Bun (ethereum-data build scripts)

**Primary Dependencies**: Crux state machines, alloy-core, UniFFI (Swift/Kotlin bindings), wasm-bindgen (web), Cloudflare Workers + `worker-build` (both services), Durable Objects / KV / Queues (relay)

**Storage**: unchanged — device storage for preferences and overrides; relay Durable Objects for lanes, records, treasury; KV for loss-harmless caches

**Testing**: `cargo test` (core, desktop, relay), `vitest` (web), XCTest (iOS), JUnit/Robolectric (Android), plus the cross-language parity corpora already in `rust/crates/vela-core/tests` and the shells' parity tests

**Target Platform**: iOS 17+, Android 10+, evergreen browsers + browser extension, macOS/Windows/Linux desktop; two Cloudflare Workers (paid plan)

**Project Type**: multi-shell wallet over one shared Rust core, plus two self-hosted backend services in sibling repositories

**Performance Goals**: no regression — Arc's ~0.5 s blocks should surface as a ~2 s expected inclusion; balance reads stay within the existing multicall budget (one extra chain, no extra round trips per chain)

**Constraints**: money rules live in the core and are written once (the reason the Rust core exists); no change may alter fee amounts or balances on the twelve existing chains; stored endpoint overrides are sacred; the old relay host must keep answering during rollout

**Scale/Scope**: 3 repositories, 4 shells + 1 core, 2 Cloudflare deployments, 2 new chain-data files, ~13 code edit sites for the endpoint move

### Repositories touched

| Repo | Path | Role in this feature |
|---|---|---|
| vela-wallet | `/Volumes/data/production/agent-3/vela-wallet` | chain table, peg rule, gas floor, single-balance rule, endpoint defaults |
| ethereum-data | `/Volumes/data/production/shelchin-workspace/data/ethereum-data` | Arc chain files + logo; Cloudflare deploy at `ethereum-data.getvela.app` |
| vela-relay | `/Volumes/data/production/vela-relay` | chain-directory host change; Arc serving; Cloudflare deploy at `vela-relay-cf.getvela.app` |

## Constitution Check

*GATE: must pass before Phase 0 research, re-checked after Phase 1 design.*

`.specify/memory/constitution.md` is still the unfilled Spec Kit template, so it imposes no project-specific gates. The de-facto standard is [docs/agent-rules/AI-CODING-RULES.md](../../docs/agent-rules/AI-CODING-RULES.md), evaluated below.

| Rule | Assessment |
|---|---|
| Understand first, then plan, then code | Satisfied: every claim in research.md is either a live probe or a cited doc; no design decision rests on assumption. |
| Keep changes small, one problem per PR | **Tension.** This feature is three separable problems (chain support / service relocation / a shared peg rule). Mitigated by sequencing them as independently shippable slices (see Phase 2 split below) rather than one commit. |
| Follow existing patterns, no speculative abstraction | Satisfied: the chain entry copies Tempo's and Gnosis's shape; the peg and gas floor follow the existing "rule in core, data per chain" pattern rather than introducing a new one. |
| Risk level | **High.** Money rules (fee floor, gas price), production endpoint changes, and a new network. Requires test evidence and a rollback plan — both specified below. |
| Verification order: typecheck → lint → unit → integration → e2e → build | Adopted verbatim in `quickstart.md`. |
| Security red lines | No secrets read or written by code in this feature. Cloudflare secrets are set out of band via `wrangler secret put`; none appears in a config file or a commit. |

**Rollback plan**: the endpoint move is a constant change — reverting the commit restores `awesometools.dev` / `vela-relay.getvela.app`, and both old hosts stay live. The Arc chain entry is additive: removing the chain-table row removes the network. The peg and gas-floor rules are guarded by parity tests that pin the twelve existing chains to their current answers, so a regression there fails the build rather than reaching a person's money.

**Post-Phase-1 re-check**: no new violations. The one tension (change size) is answered by the slice ordering, not waived.

## Project Structure

### Documentation (this feature)

```text
specs/060-arc-network-integration/
├── plan.md              # This file
├── research.md          # Phase 0 — live probe results and decisions
├── data-model.md        # Phase 1 — the entities this feature adds or changes
├── quickstart.md        # Phase 1 — how to verify the whole thing end to end
├── contracts/
│   ├── chain-data-arc.md        # the chain-data file contract for Arc
│   └── core-exports.md          # new core surface: peg + minimum gas price
└── tasks.md             # Phase 2 output (/speckit-tasks — not created here)
```

### Source code (the three repositories)

```text
vela-wallet/
├── rust/crates/vela-core/src/app/
│   ├── network_admin.rs        # BUILTIN_CHAINS +Arc; provider slugs; DEFAULT_* endpoint constants
│   ├── fee_policy.rs           # per-chain minimum gas price, applied to the derived gas price
│   └── balance_dashboard.rs    # pegged_native_usd() beside choose_native_price; native-alias rule
├── rust/crates/vela-core-wasm/src/lib.rs      # export the peg to the web shell
├── rust/crates/vela-core-uniffi/src/lib.rs    # export the peg to iOS and Android
├── rust/crates/vela-core/tests/ # parity + regression corpora
├── app-web/vela-wallet/src/lib/services/
│   ├── chains.ts, networks.ts, endpoints.ts, chain-tokens.ts, price-service.ts, rpc-providers.ts
├── app-ios/VelaWallet/VelaWallet/
│   ├── Core/ChainCatalog.swift, Core/Prices.swift
│   └── Features/Settings/NetworkAdminExecutor.swift
├── app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
│   ├── feature/wallet/core/ChainData.kt, feature/wallet/core/ChainlinkPrices.kt
│   └── VelaWalletApplication.kt
└── app-desktop/vela-wallet/src/
    ├── executor/chain_tokens.rs, executor/chainlink.rs, executor/relay.rs, executor/pool.rs
    └── settings/fixtures.rs

ethereum-data/
├── chains/eip155-5042.json        # NEW — Arc mainnet
├── chains/eip155-5042002.json     # NEW — Arc testnet
├── chainlogos/eip155-5042.png     # NEW — Arc logo
└── wrangler.toml                  # custom domain: ethereum-data.getvela.app

vela-relay/
├── src/utils/rpc.rs                    # chain-directory host
├── vela-relay-cf/src/arms/market.rs    # chain-directory host
└── vela-relay-cf/wrangler.jsonc        # custom domain: vela-relay-cf.getvela.app
```

**Structure Decision**: no new project layout. Every change lands in a directory that already owns that concern — the peg lands in `app::balance_dashboard`, the module that already owns native-coin pricing and already exports `choose_native_price` through both binding crates, because its whole purpose is to exist exactly once.

## Implementation phases

### Slice A — Services relocate (independently shippable)

Deploy ethereum-data and vela-relay to Cloudflare at the new hostnames, with the Arc chain files already included, then flip the wallet's built-in defaults. Verifiable without Arc: the twelve existing chains must behave identically. Ships first because Slice B's chain data is served from it.

### Slice B — Arc becomes a network

Chain table entries across core and four shells, provider slugs, logo, inclusion time. At the end of this slice Arc reads balances and appears everywhere — but its fee is not yet trustworthy, so this slice is not released on its own.

### Slice C — The money rules

The peg (core + four shell call sites), the per-chain minimum gas price in the fee machine, and the single-balance rule. This is the slice that makes Arc *safe*, and it carries the parity tests that pin the other twelve chains.

### Slice D — Failure legibility

Issuer-blocklist and destructed-account revert explanations; timestamp tie-breaking by block number.

Slices B+C+D are one release; Slice A may precede it.

## Complexity Tracking

| Violation | Why needed | Simpler alternative rejected because |
|---|---|---|
| Feature spans three repositories in one spec | Arc cannot read a balance until its chain data is served, and the requester scoped the Cloudflare move into this work | Splitting the service move into its own spec would leave Slice B blocked on an unspecified dependency, and the endpoint constants would be edited twice |
| A new core rule exported through both binding crates for a two-entry table | The rule it replaces is currently duplicated in four languages; adding a second symbol to four copies is how pricing rules diverge | Adding `USDC` to each shell's existing literal was measured against this and rejected in research R4. The rule lands in `app::balance_dashboard` beside `choose_native_price` — the module that already owns native pricing — rather than in a new module of its own |
