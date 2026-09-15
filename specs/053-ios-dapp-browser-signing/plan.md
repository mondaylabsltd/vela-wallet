# Implementation Plan — 053 iOS dApp Browser and Signing

**Branch**: `053-ios-dapp-browser-signing`, stacked on `052-ios-money-wiring`.
**Spec**: [spec.md](./spec.md) · **Research**: [research.md](./research.md)

## Summary

Six machines, 28 shell operations, two live builders, one platform engine. The
screens are drawn (E1–E7, cs1–cs33, the connection panel, the allowance editor,
the slide); what is missing is everything between them and the core.

The one genuinely new thing is a `WKWebView` running somebody else's
JavaScript. Three rules follow and are requirements, not care: the origin is
never the page's claim (FR-016), routing is an allowlist (FR-011), and
displayed equals signed (FR-017).

## Constitution Check

`.specify/memory/constitution.md` is still the empty template, so the gates are
the repository's own rules (`docs/agent-rules/AI-CODING-RULES.md`) plus this
program's three invariants.

| Gate | How this cut satisfies it |
|---|---|
| **Core decides, shell performs** | Every executor answers facts. No business `if`. The routing table is the one exception and it is a *port*, checked by a parity test against the shared script. |
| **Zero lines under `rust/crates/vela-core/src/app/`** | Verified at closeout by `git diff` against the branch point. |
| **Zero corpus delta** | `explore.*`, `connect.*`, `clearSigning.*` and the two `componentsUi.signing*` families already exist. |
| **Zero `vela-core-uniffi` change** | Research D0: every export is already committed. A bindings diff at closeout is a signal, not a success. |
| **Zero lines in the four sibling clients** | The provider is **read** from `app-web`, never edited. |
| **Executor contract (four rules)** | One `perform` + one `neutralAnswer` per executor, exhaustive switch, loud unknown tag, one drift test per machine. |
| **Device verification** | FR-018. Each phase ends on the iPhone; results.md marks every criterion. |

## Project Structure

```
app-ios/VelaWallet/VelaWallet/
  Features/Explore/
    Core/BrowserEngine.swift        NEW  one WKWebView + its navigation state
    Core/ProviderBridge.swift       NEW  script assembly, the handler, delivery
    Core/DappRpc.swift              NEW  the allowlist
    Core/RequestRouter.swift        NEW  state / switch / ack / read / unsupported
    Core/BrowserExecutor.swift      NEW  dapp_permissions' 8 operations + envelopes
    Core/BrowserController.swift    NEW  the owner: 3 cores, the engines, routing
    Core/DpermWire.swift            NEW
    Core/ExploreWire.swift          NEW
    Core/BhistWire.swift            NEW
    Core/ExploreExecutor.swift      NEW  explore_sites + browser_history
    ExploreLive.swift               NEW  ExploreView+BhistView+DpermView → the drawn model
    ExploreScreen.swift             EDIT the .browsing arm's one view
  Features/Signing/
    Core/SignWire.swift             NEW
    Core/ClearWire.swift            NEW
    Core/GuardWire.swift            NEW
    Core/SignExecutor.swift         NEW  7 operations
    Core/ClearExecutor.swift        NEW  5 operations
    Core/GuardExecutor.swift        NEW  3 operations
    Core/SigningController.swift    NEW  the per-request host
    SigningLive.swift               NEW  SignView+ClearView+GuardView+FeeView → SigningModel
    SigningFee.swift                NEW  the fee session for a dApp request
  Components/Explore/
    BrowserWebView.swift            NEW  UIViewRepresentable over the engine
app-ios/scripts/
  bundle-provider.sh                NEW
  provider-{input,output}.xcfilelist NEW
app-ios/VelaWallet/VelaWalletUITests/Resources/testdapp/  NEW
```

## Phases

| # | What | Ends when |
|---|---|---|
| 0 | baselines, the nine wire files, the drift tests, the provider bundle + parity test | the app builds and 9 new drift tests pass; no screen has changed |
| 1 | the engine: `BrowserEngine`, `ProviderBridge`, `BrowserWebView`, the `.browsing` arm | a real page loads on the iPhone and reports the announcement |
| 2 | memory: `ExploreExecutor`, `BhistExecutor`, `ExploreLive`, the tab strip, the groups | favourites/groups/tabs/recents survive a force-quit on the phone |
| 3 | connect: `BrowserExecutor`, `DappRpc`, `RequestRouter`, `BrowserController`, the consent surface | the test dApp connects, reads a block through the pool, switches chain |
| 4 | **sign**: `SignExecutor`, `ClearExecutor`, `SigningController`, `SigningLive` | **dust leaves the golden Safe because a page asked**, and the page prints a tx hash |
| 5 | the guard, the message signature, the connection list | unlimited blocked and edited; `isValidSignature` answers `0x1626ba7e` |
| 6 | closeout: the device pass, the SC table, the baselines | every SC marked device-verified or test-only |

Phase 4 is the gate. Nothing in phase 5 begins until SC-005 is met.

## Complexity Tracking

| Thing | Why it is not simpler |
|---|---|
| Two controllers instead of one | The permissions/browser state is app-resident (a pending request must survive a sheet); the signing trio is per-request and must die with it, or a stale fee session prices the next request. |
| A routing table duplicated in Swift | The alternative is a fifth implementation drifting silently. The parity test makes the duplicate honest. |
| The provider assembled at runtime | The bytes must be *identical* to the web tree's for the drift test to mean anything, and the web tree's are ES modules. Stripping two keywords at load is the smallest transform that keeps the bytes comparable. |
