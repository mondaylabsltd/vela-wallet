# Known bugs

## BUG-1 (✅ FIXED 2026-07-06, fund-path) — a send that needs gas-account funding silently did nothing

**Discovered:** 2026-07-06, while device-testing `eth_sendTransaction` from the Safari
extension on Gnosis (fixture Safe "Parallel One").

**Symptom:** on the signing sheet for an `eth_sendTransaction` (or `wallet_sendCalls`),
tapping **授权 / Approve** produces **no visible response at all** — no spinner, no error,
no modal. The transaction never submits. Reproduced by the founder with a real tap
(so it is NOT a test/automation artifact).

**Root cause (confirmed from Metro logs):**
1. Vela's gas model gives each Safe a **dedicated gas-account EOA per chain**
   (`bundler-service.ts` header) that pays the UserOp's gas, separate from the Safe's
   own balance. The bundler normally auto-sponsors new users from its treasury; if that
   is unavailable the gas account is empty.
2. On approve, `approveRequest` (`src/models/dapp-connection.tsx`) runs a proactive
   `checkBundlerFunding` pre-check. With the gas account empty it returns "deposit
   needed", so approveRequest does `setFundingNeeded(funding); return;` — i.e. it
   ABORTS the send and asks the UI to show a top-up prompt.
   ```
   [BundlerFunding] deposit=0x…154d03 balance=0 status=INSUFFICIENT_BALANCE
   [BundlerFunding] threshold=… spendable=0 sufficient=false
   ```
3. **The top-up prompt never appears.** `BundlerFundingModal`
   (`src/components/ui/BundlerFundingModal.tsx`) is built on `AppModal`, which is a
   **native iOS `<Modal presentationStyle="pageSheet">`** (`src/components/ui/AppModal.tsx`).
   The signing sheet is ALSO an `AppModal`. `BundlerFundingModal` is rendered as a
   sibling **stacked on top of** the signing sheet's `AppModal`
   (`src/components/SigningRequestModal.tsx:700`). iOS does not present a second native
   modal over an already-presented one (this app has hit modal-over-modal issues before,
   see [[project_native_readiness_audit]]) → the funding modal renders **invisibly behind
   the sheet**. So the user taps 授权 and sees nothing; the send is silently blocked.

**Why it matters:** this is not fixture-specific. ANY real user whose gas account isn't
auto-sponsored (treasury empty / not eligible) will tap Approve on a send and get NOTHING
— a fund-path dead end with no feedback. (Separately worth checking on the bundler side
why Gnosis auto-sponsorship didn't fund the gas account.)

**Secondary bug (same path):** the `checkBundlerFunding` pre-check in `approveRequest`
runs for up to 15s BEFORE `setIsSigning(true)`, so even on the happy path tapping 授权
gives no feedback (no spinner) during that window — it "looks dead" for up to 15s.

**Fix (shipped 2026-07-06) — the single-modal content swap:**
- **Primary (invisible modal):** the funding UI body was extracted into a wrapper-less
  `BundlerFundingView` (`src/components/ui/BundlerFundingModal.tsx`); the standalone
  `BundlerFundingModal` (still used by the Send screen) is now a thin `<AppModal>` around
  it. `SigningRequestModal` no longer renders a *second* `AppModal` — it renders **one**
  sheet whose CONTENT swaps: `fundingNeeded ? <BundlerFundingView/> : <SigningSheet/>`.
  One native modal is only ever presented, so the funding UI is always visible + tappable.
  Swipe-to-dismiss over the funding view routes to `handleFundingCancel` (matches its 取消).
- **Secondary (no feedback):** in `approveRequest` the `setIsSigning(true)` /
  `setSignError(null)` / `setPendingOpHash(null)` were moved **above** the ≤15s gas
  pre-check, so Approve shows the spinner instantly; the funding-needed branch does
  `setIsSigning(false)` before handing off to the in-sheet funding view.
- **Hygiene:** `handleIncoming` clears `fundingNeeded` on every new signing request (so a
  stale funding view can't hijack the next sheet via the content-swap); `rejectRequest` /
  `dismissRequest` also clear it. `handleFundingComplete` now clears the bundler cache for
  the **request's** chain (`incomingRequest?.__chainId ?? chainIdRef.current`) so an
  extension sign on a non-global chain doesn't loop the funding prompt.
- **Dev harness:** `vela.forceFunding(chain|'all')` (`src/services/dev/fault-injection.ts`)
  makes `checkBundlerFunding` report "deposit needed" so the funding UX is reproducible
  without draining a gas account (dev-only; always false in prod).

**Verification:** typecheck clean; full jest 1058/1058; 4-lens adversarial review (fund-safety
/ react-lifecycle / ios-modal / send-regression) found **no defect introduced by the change**.
**Device confirmation still recommended** (the native-modal visibility is a device-only
property): on the DEV build, trigger a dApp/extension `eth_sendTransaction` whose gas account
is empty (or the natural repro from discovery) → the funding sheet must now appear **visible
and interactive in place of** the sign sheet, and tapping Approve must show a spinner
immediately. Old behavior = silent dead tap.

**Parallel-space test env (免费激活 can't work there):** the parallel space uses the REAL
bundler, but its fixture Safes were seeded into storage (never through the create flow), so
their fixed-key P-256 pubkeys were never uploaded to the bundler's index → free sponsorship
is always denied (`no_passkey_registered` → "免费激活需要通行密钥。"). **Founder decision
2026-07-06:** keep sponsorship real; **self-fund the fixture gas EOAs manually** (like
Parallel One's gas account `0xb32a3965c4823ea426de52c7e869dd0cfe154d03` funded 0.01 xDAI on
Gnosis). So in the parallel space the funding view now **skips the "choose" step and leads
with self-fund** (`BundlerFundingView` gates the initial `step` on `__VELA_PARALLEL__`) —
fund the gas-account EOA shown, the balance poll detects it, Continue → the sign proceeds.
(Prod is unchanged: choose step → real sponsorship → denial falls back to self-fund.)

**Files:** `src/components/ui/BundlerFundingModal.tsx` (BundlerFundingView split + test-env
self-fund lead), `src/components/SigningRequestModal.tsx` (single-modal content swap),
`src/models/dapp-connection.tsx` (approveRequest ordering + funding hygiene + chain fix),
`src/services/dev/fault-injection.ts` + `src/services/bundler-service.ts` (forceFunding seam).

---

## BUG-2 (✅ FIXED 2026-07-06, fund-path) — a reject/swipe during an in-flight approve still submitted the tx

**Surfaced:** 2026-07-06 by the adversarial review of the BUG-1 fix (pre-existing; BUG-1
didn't introduce it). `approveRequest` captured `base = incomingRequest` and never
re-validated after its awaits, and `AppModal` swipe-dismiss is ungated by `isSigning` (unlike
the Reject button). A swipe → `rejectRequest` (4001) while the in-flight approve kept going →
a "rejected" tx that still broadcasts **plus** a contradictory success response for the same id.
Two distinct windows: the ≤15s gas **pre-check** (before any auth) and the **submit** window
(after the passkey resolves, before `onSubmitted` — the bundler round-trip).

**Fix (shipped) — two complementary guards, both windows closed on all platforms:**
- **Pre-check window** — `signCancelledRef`: `rejectRequest` sets it; `approveRequest` resets
  it at entry and, right after the pre-check `await` (before `handleDAppRequest`), **aborts
  if set**. So a reject before auth truthfully 4001s and never submits.
- **Submit window** — a reactive `isSubmitting` flag: set true right before the passkey/submit
  (the tx is now committed once authenticated), reset in `finally`; `SigningRequestModal`'s
  `onClose` routes to `dismissRequest` (not reject) when `signError || pendingOpHash ||
  isSubmitting`. So a swipe during submit **dismisses** — the op proceeds and its real result
  is delivered; no 4001, no contradiction. (`pendingOpHash` covers the same span from submit
  onward — doubly covered.)
- **Web parity** — `AppModal`'s web `DragHandle` created its `PanResponder` once, capturing a
  stale `onClose`; now reads the latest via `onCloseRef` (the pattern AndroidSheet already used),
  so a web drag-dismiss during submit also routes to dismiss, not the stale reject.

**Verified:** typecheck + jest 1058/1058; a follow-up adversarial verify confirmed CLOSED on
iOS/Android (no stuck state, pre-check reject still a real reject, funding flow intact) and the
web `DragHandle` gap is the one it flagged — now also fixed. **Files:** `dapp-connection.tsx`
(signCancelledRef + isSubmitting + approveInFlightRef), `SigningRequestModal.tsx` (onClose
routing), `components/ui/AppModal.tsx` (web DragHandle onCloseRef).

## BUG-3 (✅ FIXED 2026-07-06, fund-path polish) — plain-tap approve buttons had no debounce

**Surfaced:** 2026-07-06 by the adversarial review (pre-existing + systemic). The ordinary
Approve `VelaButton` and the funded **继续/Continue** `Pressable` had no synchronous re-entry
guard — `disabled`/`loading` ride async `isSigning`, which hasn't flipped on a same-tick second
tap — so a double-tap fired two concurrent `approveRequest` calls → two passkey prompts.
(Capped by ERC-4337 nonce enforcement: at most one UserOp lands; worst case was a contradictory
dApp response, not lost funds.)

**Fix (shipped):** a synchronous re-entrancy lock at the single submit path — `approveInFlightRef`
in `approveRequest`: acquired at entry (a second concurrent call returns immediately), released
on **every** exit (the funding hand-off, the cancel-abort, and the `finally`). This covers ALL
approve entry points (Approve button, funding Continue, any future one) at the point that
matters — the submit function — rather than per-button. Verified: typecheck + jest 1058/1058;
the adversarial deadlock lens found no path that strands the lock (`serializeAssetSim` can't
throw; the pre-check is `try`-wrapped; all returns release). **File:** `dapp-connection.tsx`.
