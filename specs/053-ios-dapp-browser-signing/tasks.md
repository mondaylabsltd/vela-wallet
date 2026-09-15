# Tasks — 053 iOS dApp Browser and Signing

**Read this first if you are picking the branch up cold.** It is written to be
enough on its own: the commands, what is done, what is next, and the traps.

Branch `053-ios-dapp-browser-signing`, stacked on `052-ios-money-wiring`, in
the worktree `/Volumes/data/production/vela-wallet-ios`.

The commands are in [quickstart.md](./quickstart.md). The six phases are in
[plan.md](./plan.md). The 28 operations are in
[contracts/shell-operations.md](./contracts/shell-operations.md). The channel
is [contracts/page-envelope.md](./contracts/page-envelope.md).

---

## Done

### Phase 0 — the papers

[spec.md](./spec.md) (FR-001…FR-018, SC-001…SC-014, six user stories),
[plan.md](./plan.md), [research.md](./research.md) (D0–D15 plus the ten
inherited Android device defects), the two contracts,
[quickstart.md](./quickstart.md), [checklists/requirements.md](./checklists/requirements.md).

Baselines in [results.md](./results.md).

---

### Phase 0 code ✅ `31cee508`

Nine wire files, six executors, the routing table, the provider bundle. Two
findings: Android's "an untold machine refuses every chain" is wider than the
truth, and a parity test passed against a set it had never read.

### Phase 1 — the engine ✅ `83ea6f6b`

探索 runs a real page and the page finds the wallet. The defect: a page that
loaded, ran, and was **invisible**, because the screen took its view from the
fixture.

### Phase 2 — memory ✅ `a5c239c7`

Favourites, the visit and the tab survive a force-quit — and the tab comes back
**with its page**, which the test had to learn is what a browser does.

### Phase 3 — connect ✅ `6db89403`

Consent, the instant answer for a granted origin, the pool-backed reads, and
`eth_sign` refused 4900. The defect: sheets carried their **contents** and so
showed what was true when they opened. `ExploreSheetKind` makes that
unspellable.

### Phase 4 — **the gate** ✅ `1e39ea2d`

Dust left the golden Safe because a page asked: 0.50067 → 0.48967 xDAI.

### Phase 5 — the guard and the signature ✅ `660440f5`

An unlimited approval is stopped and capped; `personal_sign` verifies on-chain
as `0x1626ba7e`. Four defects, and one of them was a **product** fix: an inert
slide that told assistive technology it was enabled.

---

## 053 is closed. Next is 054.

`specs/054-ios-send-contacts-parity/` — split and sweep, the payroll importer,
and contacts I/O. **Draw C7/C8/C9 first** (founder ruling 2026-09-14).

**What 054 inherits, and must not re-derive:**

- `SigningController` is the shape for a per-request host: four machines born
  with a request and dying with it. 054's batch importer is app-resident
  instead, and the difference is deliberate.
- `DocumentPorts` does not exist yet. 054 writes it (research D5 of the
  program plan) and 056 reuses it for the share sheet.
- The device-harness lessons are in `BrowserAcceptanceTests`: drag a slide
  rather than tapping it, tap the **hittable** match inside a `ViewThatFits`,
  and read the wallet's own surface before the page's.

## The traps, inherited and new

Everything in [research.md](./research.md)'s last section is a device-found
Android defect that will recur here. The iOS-specific ones:

1. **`WKUserScript` defaults to an isolated content world.** A
   `window.ethereum` defined in `.defaultClient` is invisible to the page.
   Both scripts, the message handler and every `evaluateJavaScript` must name
   `.page`.
2. **`WKScriptMessage.webView?.url` is the origin source**, read on the main
   thread at message time. Not a navigation callback (it has not fired yet),
   not `frameInfo.securityOrigin` (correct, but spelled differently from
   `dappOriginOf`, and a grant keyed differently is a grant that does not
   apply).
3. **`app.webViews` in XCUITest sees only painted text.** The test page prints
   `#verdict` lines for this reason.
4. **`file://` has no origin** under `dappOriginOf`, so the harness is served
   over loopback.
5. **A `UIViewRepresentable` that outlives its container leaks the web view
   onto another screen.** FR-013.
7. **`ViewThatFits` duplicates every candidate layout in the accessibility
   tree.** `firstMatch` can be a measured, unrendered copy: it exists, reports
   itself enabled, and a tap on it goes nowhere. Take the first `isHittable`.
8. **`tap()` does not invoke an `accessibilityAction`.** It synthesises a touch
   at the element's centre. A slide-to-confirm must be dragged.
9. **`allowsHitTesting(false)` is invisible to assistive technology.** Use
   `.disabled()` as well, or a shut control announces itself as available.
10. **A UI test's resources go stale where its code does not.**
   `test-without-building` can leave an old bundled page inside the runner.
11. Everything 052 learned still applies: `CoreStore.onFault` is silent without
   a logger; an event before `boot()` is dropped; a `var onRefresh: (() async
   -> Void)?` stored in a `View` segfaults AttributeGraph; one automation
   session at a time.

---

## Deliberately not in this cut

The camera scanner and the simulation block (055). `eth_sign` depth and the
SIWE phishing panel's copy (055) — here `eth_sign` is refused. Split, sweep,
the payroll importer, contacts I/O (054). `dapp_session` and `ext_cache` (both
ruled out). `dapp_permissions::PopupRequest` — an extension-window shape iOS
does not have; the arm stays undispatched and the reason is recorded.
