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

## BUG-4 (✅ FIXED 2026-07-06 — WalletPair) — pairing threw "crypto.getRandomValues must be defined"

**Discovered:** 2026-07-06, building the concurrent-session DEVICE harness
(`e2e/safari/check_concurrent.py` — the first time a REAL WalletPair session was
established on the device; WalletPair has ≈0 real dApp adoption, which is *why* the
Safari extension exists, so this path was never device-tested before).

**Symptom:** on the device (parallel space, DEV build), entering a valid pairing URI
on the Connect screen → **"连接失败 · crypto.getRandomValues must be defined"**.
`WalletPairTransport.prepare()` → `session.prepareJoin()` → X25519 keygen throws.
Reproduces on a FRESH app start (not stale state). The Node peer + the real relay work
fine — the failure is entirely app-side crypto.

**Root cause:** `@noble/hashes/crypto` captures `globalThis.crypto` **once at module
evaluation** (`@noble/hashes/utils.js`: `const crypto_1 = require('@noble/hashes/crypto')`,
then at call time `crypto_1.crypto.getRandomValues`). If that module evaluates BEFORE
`react-native-get-random-values` installs `globalThis.crypto`, noble holds `undefined`
forever. The pod IS linked (`ios/Podfile.lock` has `react-native-get-random-values 1.11.0`)
and `src/polyfills.ts` imports it as `_layout.tsx`'s first line — yet on device it still
loses the race (some module imports @noble before `_layout`'s polyfills line, or the native
RNG isn't installing `getRandomValues` at import). `polyfills.ts`'s own comment already warns
"Without these, WalletPair (Vela Connect) pairing throws on the first scan."

**Fix approach (needs a native rebuild to verify):** guarantee `react-native-get-random-values`
runs before ANY `@noble/*` module — e.g. a custom entry `index.js` (`import '@/polyfills';
import 'expo-router/entry';`) set as `package.json` `main`, instead of relying on `_layout`
import order; OR at the top of `polyfills.ts` assert `globalThis.crypto?.getRandomValues` is a
function after the import and install a fallback (e.g. `expo-crypto`) if not. Then re-run
`e2e/safari/check_concurrent.py` — the harness is ready and drives the full flow (parallel arm →
Connect screen → URI entry → connectToWalletPair → fingerprint → concurrent extension sign →
assert WP survives + no leak); it currently stops at this crypto throw.

**Fix (shipped + DEVICE-VERIFIED 2026-07-06):** a custom entry `index.js`
(`import './src/polyfills'; import 'expo-router/entry';`) set as `package.json` `main` — so
the RNG polyfill installs `globalThis.crypto` before ANY `@noble/*` module (and thus before
the module-load capture) unconditionally. `polyfills.ts` also fails loud now if
`crypto.getRandomValues` is still missing after the import. Verified on the device (ABC iPhone
11): after `main→index.js` + a Metro restart (`iOS Bundled index.js`), the polyfill "MISSING"
warning does NOT fire, and WalletPair pairing no longer throws — the fingerprint-verify screen
renders and Confirm proceeds (the crypto crash is gone). NO native rebuild needed — the entry
change is picked up by re-bundling.

**Note:** the two-slot ROUTING isolation (F2/F3/F4) is independently proven headless
(`src/__tests__/concurrent-session.test.ts`) and is unaffected by this.

## BUG-5 (relay flakiness — blocks the live concurrent proof) — WalletPair join lost when the relay drops an idle pre-pair connection

**Discovered:** 2026-07-06, right after fixing BUG-4, running `e2e/safari/check_concurrent.py`.
With BUG-4 fixed the wallet now joins the relay (app phase `waiting_accept`), but the session
never reaches `connected`: the dApp peer's own SDK disconnect log is
`{"side":"dapp","kind":"transport_close","code":1000,"reason":"closed","phase":"waiting","willReconnect":true}`
— the **CF-Worker relay CLOSES the peer's idle pre-pair WebSocket** (normal 1000), the peer
auto-reconnects, but the wallet's join arrives during that gap and is LOST → the wallet's
`waiting_accept` then times out as `peer_closed`. This is exactly the "the relay may silently
drop the join message (e.g. CF Worker hibernation), leaving both sides stuck in waiting_accept"
case that `src/services/walletpair-transport.ts`'s `confirmFingerprint` already anticipates.

**★ It is APP-SIDE, not the relay (isolated 2026-07-06):** a pure **Node↔Node** pairing —
`DAppSession` ↔ `WalletSession` from the SAME `walletpair-sdk`, both over the SAME real relay
`wss://relay.walletpair.org/v1` — **pairs in ~2s, fingerprints match, both reach `connected`**.
And a lone peer sits in `waiting` for 38s+ with NO disconnect. So the relay + SDK are fine; the
peer only drops **when the APP's wallet joins**. The one variable is the wallet-side WebSocket:
Node's `ws` vs React-Native's `WebSocket` (Hermes). Something about the app's RN WalletPair
connection makes the relay close the peer as the app joins (candidate causes: RN `WebSocket`
handshake/`?ch=` routing differences; a stale persisted `K_walletpairSession` reconnecting and
colliding; the relay treating the RN connection as a replacement on the channel). WalletPair's
≈0 real-dApp adoption is why this app-side breakage went unnoticed.

**Next step (fixable in THIS repo):** enable `setWalletpairDebugLogging(true)` +
`setDisconnectLogSink` on the wallet side (walletpair-transport.ts) + rebuild, and watch the
app's WS handshake / the exact frame that precedes the peer close; compare to the Node
`WalletSession` that pairs cleanly. Until then the device concurrent proof reaches **2/4**
(extension real-signature ✓, no-leak ✓; wp-connected / wp-survived blocked). The harness
(`check_concurrent.py` + `wp_peer.mjs`) is ready → 4/4 once the app's wallet can hold a session.
A workaround via peer keepalive is impossible — `DAppSession.ping()` is a no-op unless already
`connected` (dapp-session.ts:294), and the drop isn't idle-based anyway.
