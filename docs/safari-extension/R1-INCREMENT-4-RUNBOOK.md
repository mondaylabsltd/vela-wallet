# R1 Spike — Increment 4: return path + fake sign  ·  **R1 S1 GO/NO-GO GATE**

**Goal:** prove the full sign round-trip and its 4 fund-safety invariants on a physical device. page taps R1 sign → app launches (Inc 3) → app **fake-signs** (no passkey/bundler) → writes `sign-result-<rid>.json` + persists → user returns to Safari → content.js **focus-polls** the result via the (evictable) background → the page **displays** it. **This increment decides whether the sign path ships in front of WalletPair, or the return path gets redesigned.**

> **✅ Prepared & verified by tooling 2026-07-05 (no device):** applied + fixed both critic findings — (1) **`build.mjs` re-run** so the shipped bundle actually contains Inc-4 (verified: `assets/manifest.json` has `"storage"`, `assets/content.js` has `pollSignResult`/`VELA4`/`storage`, `assets/background.js` has `pollSignResult`); (2) **reject-race fixed** — `sign.tsx` now uses explicit **Sign (fake)** / **dev: reject** buttons with a one-shot `signOnceRef` guard (mutually exclusive, no last-writer-wins race). `tsc` 0 errors; Swift `pollSignResult` branch verified static (readJSON by exact rid, os_log signature, completeRequest on all paths). **Swift not compiled here.** No App Group re-registration.

> **✅ R1 CONDITIONAL GO — 2026-07-05 (founder accepted).** Verified by a fully-automated Appium+WDA harness on a physical iPhone 11 (iOS 26.5, Release build). **4/5 rows PASS incl. the hardest fund-safety cases:** happy=✓submitted; reject=✕4001 (b); **kill-after-sign=✓submitted (a, app death); >65s-evict/FACT-3=✓submitted (a, background-worker death).** Only reload-mid-flight (Row 7) failed — the reloaded page's `storage.local` re-arm doesn't re-show the verdict; NOT a fund-safety violation (result persists + is retrievable, proven by kill/evict), logged as a fixable secondary gap. **A real product bug was found+fixed: the `signOnceRef` guard blocked re-signing (sign.tsx modal reused across rids) — now reset per-rid.** Next: (1) fix reload re-arm; (2) Universal Links (one-tap, no banner); (3) real integration (wire SigningRequestModal+passkey+bundler behind the proven transport, §12).

## The 4 invariants (why this increment exists)
- **(a) Never silently lose** — the outcome is authoritative on disk in TWO places written at submit (AsyncStorage `vela.tx.pending.<rid>` FIRST, then the atomic App-Group `sign-result-<rid>.json`). The page getting it is best-effort on top; if the tab never returns, Vela still has it.
- **(b) Never false-decline** — `4001` is emitted on EXACTLY one path (native `status:'rejected'`). Dead-worker `undefined`, timeout, relay throw, `{found:false}` all → **UNKNOWN → 4900 "check Vela"**, which never settles as a decline.
- **(c) Never hang** — every poll is `Promise.race`-bounded (1.2 s × 4 ≈ 6 s → "check Vela"), and re-polls on next focus/load. Every state renders a visible banner.
- **(d) No double** — fresh UUID per rid; `resolveOnce` guard (synchronous, before any await) + `storage.local` key removal.

---

## Build + run (build.mjs is MANDATORY — the bundle is NOT served by Metro)

```bash
cd /Volumes/data/production/vela-wallet
node packages/safari-extension/build.mjs        # REQUIRED — extension ships targets/safari/assets/ (esbuild), NOT src/. (Already run + verified.)
# sanity: the built bundle must contain Inc-4:
grep -q '"storage"' targets/safari/assets/manifest.json && grep -q pollSignResult targets/safari/assets/content.js && echo "bundle OK" || echo "STALE BUNDLE — rerun build.mjs"
npx expo run:ios --device                        # rebuilds native (edited SafariWebExtensionHandler.swift in the synchronized folder)
```

## Consoles (attach all four; `VELA4 … t=<epoch_ms>` totally-orders a round-trip across them)
- **CT** (content script) → Safari **Web Inspector** → the page console
- **BG** (background) → Safari Web Inspector → the **service-worker** context
- **APP** → **Xcode console** (or Metro terminal) — the RN app process
- **NAT** (extension Swift) → **Console.app**, filter the extension process

## Eyeball artifacts (authoritative outcome, independent of the UI)
- In the page console: `await browser.storage.local.get(null)` → every rid's mirror (`state`, `userOpHash`, `resolveCode`).
- The App Group `sign-result-<rid>.json` + the AsyncStorage `vela.tx.pending.<rid>` row = the durable truth.
- **Hard ordering guard:** on every run `APP persisted` must log a **smaller `t`** than `APP result-written`. Reversed once → **NO-GO** (the persist crash-window is open).

## The flow per run (note: app-side now needs one tap)
Tap **R1 sign** in Safari → "Open in Vela?" banner → **Open** → app opens `sign.tsx` (`request received`) → **tap "Sign (fake)"** (or **"dev: reject"** for Row 8) → `Signed — return to Safari` → switch back to Safari → the page's **R1 sign** area updates to `✓ submitted 0x…` (or re-polls to "check Vela").

---

## Adversarial matrix (~10 runs, physical device, iOS 17.4→18.6+)

| # | Scenario | Steps | Expected robust outcome | PASS check |
|---|----------|-------|-------------------------|-----------|
| 1 | **Happy path (cold)** | Force-quit Vela → R1 sign → Open → tap Sign (fake) → return to Safari | Page shows ✓ + hash; `vela.tx.pending` exists | mirror `state=RESOLVED`, `userOpHash` == result file |
| 2 | **Happy path (warm)** | Same, Vela already backgrounded | Same as #1 | Same |
| 3 | **Kill app right after sign** | Wait for `APP result-written` in Xcode → force-quit Vela → return to Safari | Return wakes a fresh worker, native reads the file, page resolves | `NAT result-read found=true` → RESOLVED. If kill beat the write: page → CHECK_VELA, `vela.tx.pending` still present. **Never blank, never 4001** |
| 4 | **Lock during fake-sign** | On sign screen tap Sign (fake), lock phone ~10 s → unlock → return | File readable post-unlock (first-user-auth protection) | RESOLVED (or CHECK_VELA if lock interrupted the write; LocalTx intact); no unreadable-file error in NAT; no 4001 |
| 5 | **>45 s evict → return** | Complete fake-sign → leave Vela foregrounded idle ≥60 s (evicts the MV3 worker) → back to Safari (target iOS 18.4.1/18.5) | First poll may hit FACT-3 `undefined`; retry re-relays and resolves | `BG native-undefined` ≥1× **then** `native-reply found=true`; `CT poll-attempt n≥2`; RESOLVED (or CHECK_VELA, **never silent**) |
| 6 | **Two tabs concurrent** | R1 sign in Tab A (don't return) → R1 sign in Tab B → Sign both in Vela → return A then B | Two distinct rids; each tab resolves to ITS OWN hash; no cross-delivery | two `vela.sign.*` entries, each RESOLVED with its own file's hash; `ridA≠ridB` |
| 7 | **Reload mid-flight** | While `sign.tsx` signing, reload the Safari tab → let sign finish → return | Realm wiped; content boot **re-arms from `storage.local`**; poll resolves same rid | `CT re-armed source=storage.local` after reload → RESOLVED/CHECK_VELA for the same rid; result not orphaned |
| 8 | **Explicit reject (the (b) control)** | R1 sign → in Vela tap **dev: reject** → return | Muted rejected state, resolves **4001** | page rejected; mirror `resolveCode=4001`; result file `status=rejected`. **The ONLY row where 4001 is correct** |
| 9 | **Tab discard** | R1 sign → open many tabs/apps to force Safari to discard the dApp tab → Sign → return (Safari reloads it) | Same as reload: fresh content script re-arms from persistent storage | `CT re-armed` for the original rid → RESOLVED/CHECK_VELA; rid survived in `storage.local` |
| 10 | **Regression on floor OS** | Repeat #1 on **iOS 17.4** | Stable | (a),(b),(c),(d) hold |

## GO / NO-GO decision rule
- **GO** (ship sign path in front of WalletPair) **only if:** all 10 runs hold **(a) never-lose** and **(b) never-false-decline** with **zero** violations, and **(c)** never-hang + **(d)** no-double hold on every run. (a) and (b) are fund-safety — **no tolerance**: a single violation on any run = **NO-GO** (sign path stays behind WalletPair; return path redesigned).
- Any **(c)/(d)** violation = NO-GO, but a fixable-design signal (tell me what failed and I iterate).

## Known spike limitations (NOT matrix-blocking; hardening later)
- **#3** — `submitFakeSign` has no timeout around the AsyncStorage/App-Group writes; if a native write ever *stalled* the app-side would sit on `signing…`. Fake writes are fast+reliable, so not exercised; real app needs a durable-write timeout + a fault-injection row.
- **#4** — a `4900`/TIMEOUT rid is never GC'd from `storage.local` (re-polls each focus, bounded — not a hang). Unbounded key growth; add a terminal sweep/TTL when hardening.
- **R3 (Inc 3):** the launch still shows the "Open in Vela?" custom-scheme banner (one extra tap). Universal Links remove it — parallel polish, independent of this gate.

---

**After you run it:** tell me the per-row pass/fail (or just "all 10 green"). All (a)/(b) green across the matrix ⇒ **GO** — I mark R1 cleared and we move from spike to the real integration (wire the actual `SigningRequestModal` + passkey + bundler behind this proven transport, per ARCHITECTURE §12). Any (a)/(b) failure ⇒ we redesign the return path before shipping the sign hop.
