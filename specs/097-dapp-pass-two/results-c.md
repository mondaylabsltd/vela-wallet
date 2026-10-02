# 097 part C — a refusal after "Submitted" is shown, not swallowed (N4, sheet part)

Branch `097-refusal-after-submit` from `origin/main` cca03b562. Not pushed. Spec and findings live on
part A's branch (`specs/097-dapp-pass-two/spec.md`, User Story 3; `findings.md`, N4).

## Plan

**Root cause.** The pass's Aave withdraw (`fixtures/dapp097/aave-withdraw-refused.json`): the relay
took the op (`submit verdict=accepted hash=0x4319f0f743`, 16:45:48), the window's landing said
"Submitted", and the tracker's status poll then read `rejected` (16:45:59.726). `sign_request::on_op_tracked`
answered the page with the verdict at once (−32603 "the network refused this transaction; nothing was
sent", `req.answered outcome=error 16:45:59.727Z`), and the extension worker closes a request window
the moment its request is answered (`background.js` `answer` → `releaseWindow`). The landing would
have drawn the refusal (`dapp-receipt.ts` `refused`); the window was gone first.

096 part A set the rule for a failure *before* sending (F8): the answer is held while the sheet shows
the failure; the close answers it once; a new request answers the one it replaces; nobody looking →
answered at once. The tracker's post-submit verdict was the one failure path left outside it.

**Decisions (all in the core).**

1. A tracker `Rejected` or `NotSent` for the request on the sheet is **held** (`Pending::held`) — the
   page is answered **at the close**, consistent with 096 A. Not on the sheet (closed while it ran,
   superseded) → answered at once. The page gone (`TransportDropped`) → dropped: the browser already
   answered it 4900.
2. **No "Try again" after the submit.** `failure_retryable` additionally requires that the rid was
   never settled `Submitted` — invariant ⑧, "a settled rid never signs twice". A refusal is refused
   again (RJ3); a "not sent" after a POST is the page's to send again, not the sheet's to re-sign.
3. A global chain switch under a held failure answers that failure, not a 4001 (a 096 A gap the hold
   widened).
4. The extension window stays open because nothing is answered while the refusal shows; the worker is
   unchanged. A window closed from the OS while it shows answers once with the window's settlement
   (RJ2: "not confirmed yet" for a claimed submit) — the 096 A behaviour the spec keeps.

**Shells.** Web/extension: the landing over the sheet already draws `refused`; its Done must now be
the close the core answers on (`landingCloseAction` → `dismiss_tapped`), or the window's backstop
would answer the page 4900 → "not confirmed yet". Desktop, iOS, Android: their sheets already drew a
held failed receipt (096 A); checked through each shell's real controller.

## Tasks

- [x] T1 core: hold the tracker's refusal / not-sent in `on_op_tracked`; retry gated on ⑧; chain switch
  answers a held failure. Tests from the pass's request (fail on old code).
- [x] T2 web: `landingCloseAction` + `SigningHost.closeLanding`; `dapp-receipt.test.ts`,
  `SigningHost.svelte.test.ts`.
- [x] T3 extension: `background.test.ts` window-surface contract; e2e
  `extension-lifecycle.e2e.ts` "097 N4" (stub relay accepts, then status `rejected`).
- [x] T4 desktop: wiring test through `tracked_event` + `status::approved`.
- [x] T5 iOS: `SigningFollowsTrackerTests` (held, receipt drawn, Done answers once); `committed` no
  longer keeps a controller whose pipeline ended in a held failure.
- [x] T6 Android: `DappSignMachineTest` (held, closed only after Done answers once).
- [x] T7 regenerate wasm / pkg-web / TS mirror; suites; screenshots.

## Results

| Commit | What |
|--------|------|
| 1aaad342c | core: hold the post-submit refusal / not-sent; retry gated on ⑧; chain switch answers a held failure; regenerated wasm, pkg-web, TS mirror |
| 6a698b760 | desktop: wiring test (tracker → core → receipt → close) |
| 109e09d25 | Android: wiring test through the real controller |
| 4200367fc | web + extension: `landingCloseAction`, tests, worker contract, e2e |
| 055bf9e21 | iOS: `committed` after a held failure; tracker tests |

### Core (`rust/crates/vela-core/src/app/sign_request.rs`)

- `on_op_tracked`: `Rejected` / `NotSent` with the request on the sheet → `Pending::held`, `sign_error`
  (+ `sign_error_refused` for a refusal), `pending_op_hash` cleared, **no answer**. Off the sheet → answered
  at once (unchanged). The rid is settled `Submitted` either way (⑧: an arrival of it replays, never signs).
- `failure_retryable` = held ∧ not refused ∧ `SubmitFailed` ∧ **rid not settled `Submitted`**.
- `on_chain_switch`: a held failure is answered with its own words, not 4001.
- Answered exactly once: close (`DismissTapped` / `SwipeDismissed` / a `RejectTapped` routed to dismiss), a new
  request taking the sheet, a global chain switch; a dropped transport drops it unanswered (the browser's 4900).
- Tests (`tests/app_sign_request.rs`, fixture `tests/fixtures/dapp097/aave-withdraw-refused.json` = the
  pass's request): `n4_*` ×7 new; `a_rejection_the_tracker_learns_is_answered_refused_once`,
  `a_not_sent_verdict_the_tracker_proves_is_answered_not_sent_once`,
  `a_not_sent_verdict_never_answers_an_op_the_relay_accepted` updated. **7 fail on the old code.**

### Shells

- **Web / extension.** `dapp-receipt.ts` `landingCloseAction`; `SigningHost.closeLanding` dispatches
  `dismiss_tapped` when the core still shows the landing's request with a failure (else G37's hide). The
  worker is unchanged: it closes a window only on an answer, so the window stays while the refusal is held.
  `background.test.ts`: window-surface contract (stays until the refusal's answer; closes on it; one answer;
  an OS close answers once). e2e `extension-lifecycle.e2e.ts` "097 N4": packaged extension, request window,
  fixture key, stand-in relay that takes the op and then reports `rejected` — refusal visible, window open
  8 s past every timer, page unanswered; Done → one −32603 in the core's words → window closes, ledger empty.
  **Against an extension built from the old core it fails** (the refusal is never visible: the window closed).
- **Desktop.** Already drew the held failed receipt (096 F8; `status::approved`). Wiring test
  `the_tracker_s_verdict_answers_the_page_once` now: tracker `rejected` → no answer; receipt Failed +
  `refused`, Done, no retry; `SwipeDismissed` → one answer. Comment in `status.rs` updated.
- **iOS.** Sheet already drew it (`SigningLive.receipt`). `SigningController.committed` excludes a failure on
  the sheet, so a page gone under the held refusal does not leave a controller in `retiredSigning`.
  `aRejectedVerdictAnswersRefusedOnce` (held, receipt drawn, close answers once) and
  `aPageGoneUnderTheRefusalLeavesNothingCommitted`.
- **Android.** Sheet already drew it (`SigningLive.receipt`); `cancel()` already closes on a page gone.
  `DappSignMachineTest` "the tracker's rejection answers refused once": held, `closed` false, close answers once.
- **No shell lacks the surface**: all four (and the extension) have the dApp signing sheet.

### Activity (part B)

No field added. The tracker already closes the record `failed` with no `txHash` for a refusal / not sent
(a revert carries its `txHash`), which is all this part stores. If B needs "refused" vs "not sent" words on
the row, the minimal addition is a failure kind on the tracker's failed patch — not done here.

## Tests

- Core: `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` **2452 passed, 0 failed,
  1 ignored**; clippy `-D warnings` clean; fmt clean. `build-web --check`, `gen-onboarding-types --check` current.
- Web: vitest **175 files, 2578 passed, 5 skipped**; `pnpm check` 0 errors / 0 warnings; prettier + eslint clean
  on touched files; `pnpm build:extension` ok.
- Extension e2e (lifecycle, signing, connect, store-package, live-provider): **38/39 passed**; the one failure
  was `SC-304` (personal_sign) hitting the 3-min timeout under load — passes alone in 2.4 s (096 A saw the same
  class, a different test each run). "097 N4" passed 3/3 serially, and in the full run.
- Desktop: fmt ok; clippy: no new warnings (54 / 56 before and after); `cargo test` **910 passed, 0 failed,
  49 ignored**.
- Android: `testDebugUnitTest` **963 tests, 0 failures**.
- iOS: focused suites then full `VelaWalletTests` **1159 tests in 147 suites passed** (own cloned simulator,
  deleted after).
- CI scripts: native reachability ok; event payloads 0 mismatches; dead controls 0.

i18n: **0 bytes added** (no new strings: `componentsUi.signing.refused`, `componentsTx.receipt.statusFailed`,
`componentsTx.receipt.done`, `send.txErrorGeneric` reused). ja+en resident 137,675 B; runtime JSON 141,793 B
of the 141,800 budget.

## Screenshots

Session scratchpad `097-shots/`:

- `097c-ext-refusal-window.png` — the packaged extension's request window (420 × 760) after the stand-in relay
  refused the taken op (e2e, headless Chromium).
- `097c-ios-refusal.png` — the iOS signing sheet from the real controller after accept → `rejected`
  (simulator, rendered through the app's window scene by a throwaway test, not committed).

Desktop and Android have no screenshot harness for a live signing receipt (as in 096 A).

## Open questions

1. **A revert after "Submitted"** (`Dropped` with a tx, or the shell's `Reverted`) is still answered at once and
   the sheet cleared; in the extension the worker then closes the window over the landing's "reverted". Same
   swallow, different ending. Holding it too needs iOS/Android receipts to draw a held revert from the view
   (desktop already does via `reverted_transaction`). Not in N4; proposed as a follow-up.
2. The window closed from the OS while the refusal shows answers the RJ2 settlement ("not confirmed yet …"),
   not the refusal's words — 096 A's behaviour, kept as the spec says. The surface could post the held answer
   on `pagehide`.
3. While the refusal is held the shell's receipt wait keeps polling the relay until the close (or its ~120 s
   end) — harmless, a few extra reads.
4. Ruling to confirm: a "not sent" after the submit offers Done only (⑧), though its sentence
   (`send.txErrorGeneric`) says "please try again" — the page's retry, not the sheet's.
