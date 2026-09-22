# Implementation Plan: Close the audit's product gaps

**Branch**: `081-audit-product-gaps` | **Date**: 2026-09-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/081-audit-product-gaps/spec.md`; measured findings in [research.md](research.md)

## Summary

Fourteen disclosed gaps, closed where they belong: money rules in `vela-core`, transport and platform work in each shell, two small fixes in the service repositories, and one workflow change for release provenance. The work is sliced into nine packages that ship as separate PRs; the three that change core wire types are ordered first and sequentially, because each regenerates the TypeScript, Kotlin and Swift bindings that all four shells decode strictly.

## Technical Context

**Language/Version**: Rust 2024 (`vela-core`, desktop shell, service repos), TypeScript 5 + Svelte 5 (web wallet, site), Kotlin 2 + Jetpack Compose (Android), Swift 6 / SwiftUI (iOS), GitHub Actions YAML

**Primary Dependencies**: Crux state machines, UniFFI (Swift/Kotlin bindings), wasm-bindgen (`rust/pkg-web`), alloy-core, SvelteKit + adapter-cloudflare, gpui, wry (desktop webview), Lottie (iOS)

**Storage**: per shell — browser localStorage + IndexedDB (web), one JSON document (desktop), DataStore (Android), UserDefaults (iOS); no server-side user storage

**Testing**: `cargo test -p vela-core --features crux,i18n-all` and `tests/vectors/*` conformance replayed through Kotlin, Swift and the shipped wasm; `vitest` (web wallet, site); `cargo test` (desktop); JUnit + instrumented (Android); XCTest / Swift Testing (iOS); Playwright e2e with a test dApp (web); real devices per FR-019

**Target Platform**: web (Cloudflare Workers), Chrome extension, macOS/Windows/Linux desktop, iOS 17+, Android 10+ (minSdk 29)

**Project Type**: multi-shell monorepo — one shared Rust core, four native shells, one marketing/docs site, two separate service repositories

**Performance Goals**: no regression in signing-sheet time-to-first-render; the self-call guard is pure calldata inspection (no I/O); forward name verification adds at most one `eth_call` per name and must not block the address from being shown

**Constraints**: a blocked request must fail closed (never silently unsignable); wire-type changes ship atomically with all four shells' generated bindings; corpus edits follow the six-step i18n gate; `vela.pendingUploads` survives erase everywhere; no site copy enters the vela-core corpus

**Scale/Scope**: 14 gaps → 20 functional requirements, ~9 PRs across 3 repositories, 4 shells, 15 locales

## Constitution Check

`.specify/memory/constitution.md` in this repo is still the unfilled template, so the gates below come from the project's recorded rulings (the vela-relay constitution's core/shell split, which this repo follows in practice, plus founder decisions in `docs/` and memory):

| Gate | Status |
| --- | --- |
| Business rules live in the core, shells only render and execute effects | **PASS** — FR-005 (self-call guard), FR-008 (provenance), FR-009 (readiness), FR-010 (forward-verification rule) all land in `vela-core`; shells keep transport only |
| The core stays I/O-free and deterministic | **PASS** — the guard is pure calldata inspection; forward verification follows the existing `registry_lookup` transcript pattern (core decides, shell performs `eth_call`) |
| One implementation, four shells; no rule duplicated per platform | **PASS** — and the feature *removes* duplication (four copies of reverse-name logic gain one shared rule) |
| Site copy never enters the vela-core corpus | **PASS** — network-count copy is corpus (app), the site's own strings stay in the site catalogs |
| Device verification before "done" for native shells | **PASS** — FR-019, with the matrix in [quickstart.md](quickstart.md) |
| No claim shipped ahead of the fix | **PASS** — FR-020 ties each doc edit to the PR that closes its gap |

No violations to justify; Complexity Tracking is therefore omitted.

## Project Structure

### Documentation (this feature)

```text
specs/081-audit-product-gaps/
├── spec.md              # what and why (14 gaps → FR-001…FR-020)
├── plan.md              # this file
├── research.md          # Phase 0 — measured current behaviour, file:line, decisions
├── data-model.md        # Phase 1 — the five entities the fixes introduce
├── contracts/           # Phase 1 — core↔shell contracts that change
│   ├── self-call-guard.md
│   ├── descriptor-provenance.md
│   ├── network-readiness.md
│   ├── verified-name.md
│   └── erase-device.md
├── quickstart.md        # Phase 1 — how each fix is proved, incl. the device matrix
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source code (the parts this feature touches)

```text
rust/crates/vela-core/src/app/
├── self_call_guard.rs      # NEW — pure kernel (FR-005)
├── sign_request.rs         # two hooks + new SignErrorKind (FR-005/006)
├── clear_signing.rs        # ClearProvenance, honest `verified` (FR-008)
├── network_admin.rs        # required set, multi_key_ready, effective index URL (FR-001/002/009)
├── name_verify.rs          # NEW — forward-verification transcript rule (FR-010)
└── i18n/…, ../i18n/locales/*/…   # new keys + network-count copy (FR-005/009/011)

app-web/vela-wallet/        # web: error surface, index per call, feedback, erase card, X-Rpc-Url
app-desktop/vela-wallet/    # desktop: index at boot, guide link, erase handler, X-Rpc-Url
app-android/vela-wallet/    # Android: index per lookup, erase coverage, X-Rpc-Url
app-ios/VelaWallet/         # iOS: endpoints wiring, index re-apply, erase coverage, PrivacyInfo
app-web/getvela.app/        # site: chain-setup mirror, delete 5 routes, docs in 15 locales
.github/workflows/          # release provenance (FR-012)
```

**Structure Decision**: no new project or package; the only new files are two core modules, one iOS manifest, one web service module, and the spec's own documents.

## Work packages and order

Ordered by dependency, then by the spec's priorities. Each package is one PR with its own tests and doc updates.

| # | Package | Requirements | Depends on |
| --- | --- | --- | --- |
| WP1 | Self-call guard: core kernel + `sign_request` hooks + `SignErrorKind` + four shells' rendering (incl. **the web signing sheet's missing error surface**) + test-dApp buttons | FR-005, FR-006, FR-018/019 | — |
| WP2 | Descriptor provenance: `ClearProvenance`, honest `verified`, pinned typed descriptors, per-shell labels (fixing desktop's `verifiedAbiWarning` and web's `selectorNotListed` mislabels) | FR-008 | WP1 (wire churn) |
| WP3 | Network readiness: required-contract set (+ signer factory and singleton, − fallback handler), `multi_key_ready`, four shells + site chain-setup mirror and its drift test | FR-009 | WP2 (wire churn) |
| WP4 | Forward-verified names: core rule + four shells' transports, cache rule, tests | FR-010 | WP3 |
| WP5 | Service endpoints and the index: iOS page wiring + payload-parity guard in the parity scripts, per-call index reads in web/desktop/Android/iOS, `X-Rpc-Url` removal, self-hosting guide links | FR-001, FR-002, FR-004, FR-007 | — (parallel with WP1–WP4) |
| WP6 | Copy: network count in the corpus (six-step gate) + packaging metadata + extension manifest | FR-011 | — |
| WP7 | Hygiene: delete the five dormant site routes, wire web feedback (+ real preview lines, reachable row, prefill by field id), erase on desktop/iOS (+ web wide-layout card, Android's missed stores) | FR-015, FR-016, FR-017 | — |
| WP8 | Service repositories: p256-index `.env.example` (including the commented-out **required** contract address) + Dockerfile + compose; vela-relay Dockerfile + compose — separate PRs in those repos | FR-003 | — |
| WP9 | Docs sync: remove each closed gap's disclosure from the site (15 locales), README, `docs/ARCHITECTURE.md`, the 080 claim ledger and audit report | FR-020 | all |

**Sequencing rule**: WP1 → WP2 → WP3 → WP4 touch core wire types and are done one at a time, each regenerating bindings (`npm --prefix scripts run gen:core-types`, `build:wasm`) and shipping all four shells together. WP5–WP8 run in parallel with them because they touch shell transport, workflows and other repositories.

## Risks and how they are contained

1. **A blocked request that renders as nothing** (web has no error surface today) — WP1 adds the status block first, and the Playwright test asserts the visible explanation, not just the refusal.
2. **Strict wire decoding on iOS/Android** — new enum variants break the whole view if a shell lags; every wire change ships with the generated bindings in the same PR, and `cargo test -p vela-core` plus the conformance replay run before the shells build.
3. **Over-blocking** (FR-006) — the fee leg and the estimation placeholder target the Safe itself; the guard is scoped to dApp-originated params and allows empty-data self-calls, with regression tests on a real send per shell.
4. **Honest `verified` downgrades every permit sheet** — typed descriptors are fetched today; WP2 bundles the ones Vela relies on as built-ins before flipping the label, and measures what is left.
5. **Forward verification hides names users already see** — it is a visible behaviour change; caches are versioned so unverified entries are dropped, and the contacts/activity paths that persist names are checked (`contacts.rs:995`, `activity_feed.rs:229`).
6. **Corpus edits without `dump:vectors`** leave the conformance vector stale and fail CI with a misleading message — WP6 follows the six-step gate and the task list names each step.
7. **Deleting routes that something calls** — WP7 re-runs the proof (grep across all three repos) in the PR and keeps `og`, `downloads`, `bug-report`, `exchange-rate`.
8. **macOS artifacts cannot carry workflow provenance** (they are notarized and attached by hand) — documented as notarization-verified, with an opt-in attest job; FR-012 is satisfied per artifact type, not by pretending.
9. **Two writers on `vela.serviceEndpoints`** stay safe only because every shell merges — WP5 keeps the merge and adds a test.

## Phase status

- Phase 0 (research) — **done**, [research.md](research.md)
- Phase 1 (design and contracts) — **done**: [data-model.md](data-model.md), [contracts/](contracts/), [quickstart.md](quickstart.md)
- Phase 2 (tasks) — `/speckit-tasks`
