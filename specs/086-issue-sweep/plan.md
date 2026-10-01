# Implementation Plan: 086 — Issue sweep

**Branch**: `086-issue-sweep` (docs) · each fix on `fix/issue-<N>-<desc>` | **Date**: 2026-10-01 | **Spec**: [spec.md](spec.md)

## Summary

Sixteen tester issues, triaged against `main` @ `67e2d193d`. Each is root-caused from code, fixed at the
root (in vela-core when the decision is a rule), and fixed on every shell that has the same defect.
Each lands on its own branch and PR (`Fixes #N`). Issues on the same screen may share one PR.

## Technical Context

- **Core**: Rust `rust/crates/vela-core` (Crux machines). It is shared through UniFFI (Kotlin, Swift) and
  wasm-bindgen (web and extension). The MV3 extension worker uses JS mirrors pinned by tests.
- **Shells**: Android `app-android/vela-wallet` (Kotlin/Compose), iOS `app-ios/VelaWallet` (SwiftUI),
  web + extension `app-web/vela-wallet` (SvelteKit), desktop `app-desktop/vela-wallet` (gpui).
- **Testing**:
  - core: `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` + clippy.
  - Android: `testDebugUnitTest`.
  - iOS: VelaWalletTests on a cloned simulator.
  - web: vitest + svelte-check.
  - desktop: `cargo test`.
- **Devices**: Xiaomi alioth (Android, `9d5f42fb`) and iPhone 11 (iOS 26.5.2). Both are reserved for the
  lead; agents use simulators.
- **Constraints**:
  - The i18n ja+en budget is near its cap (138,800).
  - No signer-page deploys.
  - No real funds.
  - Never change a system proxy.

## Constitution Check

The project constitution file is an unfilled template. The binding principles are the owner's:

1. A stable experience in an unstable environment.
2. Rules are decided once in the core; shells only draw.
3. A clear, simple, maintainable architecture with less duplication.
4. Every fix is device-verified on Android and iOS.

Each cluster below names how it meets 2 and 4. **Gate: PASS** (no violations planned).

## Delivery clusters (parallel, disjoint worktrees)

| Cluster | Issues | Worktree | Shells to check |
|---|---|---|---|
| A — scan to send | #312, #326 (web), #332 (Android) | `vela-wallet-082` | all four. The rule "which asset a scanned code offers" goes in the core (FR-004). |
| B — Explore + Assets | #330, #329, #328 (Android) | `vela-wallet-tdsig` | iOS, web, desktop. Favorite naming uses the core LoadWatch `named_url`. |
| C — Android screens | #331, #322, #321, #314 | `vela-wallet-082-core-h` | iOS, web, desktop |
| D — Web contacts | #310 + #334 (one PR), #333 | `vela-wallet-082-core-a` | desktop, iOS, Android. Import decoding is decided in the core. |
| E — Extension | #315, #317 | `vela-wallet-082-core-c` | in-app dApp browsers (account follow) |
| F — iOS trusted signer | #318 | `vela-wallet-082-core-ef` (lead) | iPhone + simulators |

Agents commit locally. The lead reviews each diff, verifies it on a device, pushes, and opens the PR.

## Research

See [research.md](research.md): one entry per issue with the confirmed root cause (file:line on `main`),
the decision, and the alternatives considered.

## Verification

- Per issue: the reported steps are replayed on the affected platform before and after the fix, with
  screenshots. Automated tests cover the failure, the correct behaviour and the edges.
- Per branch: the full suites of every shell the branch touched.
- The program's device pass (spec 087) runs on an integration build of `main` plus every fix branch.
