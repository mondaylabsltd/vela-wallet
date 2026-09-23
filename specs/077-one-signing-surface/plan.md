# Implementation Plan: 077 — one signing surface

**Spec**: [spec.md](spec.md) | **Status**: not started

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| A | The panel opens the wallet; a pending request raises the sheet over it | the extension's package test; a panel test that the wallet is still drawn |
| B | The receipt after a dApp transaction — the send's own `StatusHero`, driven by the same tracker | web unit tests; the existing answering tests unchanged |
| C | The Ethereum backup inherits it with no code of its own | its existing tests, plus one that it draws the same component |
| D | The dedicated window (`?rid=`) keeps today's ending | the tests that pin it |

Order: A, then B; C falls out of B; D is a guard on both.

## Risks

- **The answering invariant.** The biggest. A receipt that lives after the
  answer must never be able to swallow, delay or duplicate one. B is built as a
  layer strictly above a settled request, and every answering test stays.
- **"One request is one page load."** The panel reloads per request today — a
  fresh core, a fresh fee session, nothing carried over (026's one-owner rule).
  If the panel becomes the wallet, that reload is gone and the fee session's
  ownership has to be re-argued, not assumed.
- **A wallet under the sheet is a wallet that can be driven.** Tapping through
  to the wallet while a request is pending must not be possible, or a person can
  start a send while a dApp waits on them.
