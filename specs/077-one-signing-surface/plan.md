# Implementation Plan: 077 — one signing surface

**Spec**: [spec.md](spec.md) | **Tasks**: [tasks.md](tasks.md) | **Status**: A,
B and D done and driven in a real browser; C structural (see tasks.md's "Not
verified")

## Phases

| Phase | Deliverable | Gate |
|---|---|---|
| A | The panel opens the wallet; a pending request raises the sheet over it | the extension's package test; a panel test that the wallet is still drawn |
| B | The receipt after a dApp transaction — the send's own `StatusHero`, driven by the same tracker | web unit tests; the existing answering tests unchanged |
| C | The Ethereum backup inherits it with no code of its own | its existing tests, plus one that it draws the same component |
| D | The dedicated window (`?rid=`) keeps today's ending | the tests that pin it |

Order taken: B first (it is what the owner asked for twice), then A, then D.
C falls out of B.

## Risks, and what became of them

- **The answering invariant.** The biggest. A receipt that lives after the answer
  must never be able to swallow, delay or duplicate one. B is built as a layer
  strictly above a settled request, and every answering test stays.
  *Held: the answer goes out unconditionally in `sendResponse` before anything
  about the chain is read, and all 1535 web tests still pass.*
- **"One request is one page load."** The panel reloaded per request — a fresh
  core, a fresh fee session, nothing carried over (026's one-owner rule). If the
  panel becomes the wallet, that reload is gone and the fee session's ownership
  has to be re-argued, not assumed.
  *Re-argued, not assumed: `take()` refuses to start while a request is owed, so
  the page's one fee session is only ever asked about one operation — which is
  already how it works when a person signs two sends in a row. The reload is
  gone; the rule is kept by sequencing instead of by teardown (T014).*
- **A wallet under the sheet is a wallet that can be driven.** Tapping through
  to the wallet while a request is pending must not be possible, or a person can
  start a send while a dApp waits on them.
  *Closed by T015: an invisible guard under the sheet's scrim while a request is
  owed. Invisible on purpose — the wallet behind a request is something to READ,
  and dimming it would take that away too.*

## What driving it actually taught

Three things no test knew, each found by running the packaged extension:

1. `tracker_handoff` is `null` inside `sendResponse` — it lands on the next view.
2. `chrome.sidePanel.open` on an already-open panel does not reload the page, so
   a surface that asks for its request only at mount never sees the second one.
3. `background.js`'s `settle()` removes a dedicated window the moment the answer
   goes out, which is why the panel — with no window id — is the only surface a
   transaction can be watched on.
