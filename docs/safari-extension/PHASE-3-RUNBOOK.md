# Phase 3 runbook — polish (world:MAIN · toolbar popup · Universal Links)

**Status:** implemented 2026-07-06, JS-verified green (11 unit + 21 Playwright). Two
items carry a device/infra follow-up (called out below). Built on the Phase-B core
(both signature methods device-verified). All changes are ADDITIVE — the proven
Phase-A/B sign path is unchanged and remains the default.

## 1. `world:"MAIN"` injection — strict-CSP dApps

**Problem:** the provider was injected only via content.js's `<script src=runtime.getURL('inpage.js')>`.
A dApp with a strict `script-src` CSP (no extension origin) BLOCKS that tag → no
`window.ethereum` → the dApp can't see Vela at all.

**Fix:** register `inpage.js` as a **`world:"MAIN"` content script** (Safari 18+) —
extension-injected MAIN-world scripts are immune to the page CSP. The old `<script>`
tag stays as the **older-Safari fallback**, now gated so it doesn't fire (and log a
CSP error) once the MAIN-world path installed.

Correctness is guarded three ways (all test-locked, `test/world-guard.spec.ts`):
- `inpage.js` sets a **shared-DOM marker** (`data-vela-inpage`) on install; content.js
  skips its fallback `<script>` when the marker is present.
- `inpage.js` **bails if it detects extension APIs** (`browser/chrome.runtime.id`) —
  i.e. an older Safari that honored the entry but ignored `world` and ran it ISOLATED
  (where `window.ethereum` would be invisible). It bails WITHOUT marking, so content.js
  still fires the MAIN-world fallback. → no iOS<18 regression.
- `inpage.js`'s existing `__velaProviderInstalled` guard makes any double-inject a no-op.

Files: `manifest.json` (2nd content_scripts entry, `world:"MAIN"`), `inpage.js`
(world-guard + marker), `content.js` (marker-gated fallback).

## 2. Toolbar popup (`popup.html` + `popup.js`)

Replaces the R1 stub with a real status surface (`test/popup.spec.ts`):
- active Vela account (name / address / chain) from the app-written cache,
- current-site connection state (已连接/未连接 + chain) with a one-tap 断开,
- every other granted site, each with 断开,
- 打开 Vela hand-off (`tabs.create('velawallet://')`).

Read-only + revoke only; no keys, no signing. Background gains a `status` message
(read-only snapshot, no grant side effects); `revoke` already existed. Popup enumerates
grants from `storage.local` directly. Needs `tabs` permission (added) to read the active
origin; degrades to account-only if unavailable. Light + dark verified.

## 3. Universal Links — one-tap sign launch (ATTESTATION-DRIVEN, auto)

**★ DEVICE-VERIFIED END-TO-END (2026-07-06, ABC iPhone 11).** The full loop is proven on
hardware: AASA `applinks` deployed to getvela.app + dev-mode entitlement + iPhone
"Associated Domains Development" ON → association resolves (bypassing the lagging Apple
CDN); the app attests on a `/sign` UL open; a real dApp sign then launches **one-tap via
the UL with NO "Open in Vela?" banner**, and after signing **the dApp's Safari tab
survives (un-reloaded) and the signature returns to the dApp**. The fund-safety machinery
was also exercised live: an earlier automated run tripped the self-heal (`UL_BROKEN`), and
a fresh re-attestation cleanly out-dated the veto (`attestedAt > brokenAt`) to re-enable
the UL — no manual clear. Headless sign regression on the same build: `personal_sign` +
`eth_signTypedData_v4` both 4/4.

Removes the "Open in Vela?" scheme banner by launching the sign hand-off via
`https://getvela.app/sign?rid=…` instead of `velawallet://sign?rid=…`.

**Why a Universal Link is dangerous here:** it is a real top-level navigation. If the
AASA association FAILS (file not hosted, app not installed, CDN stale), iOS navigates the
dApp's tab to the web URL — **destroying the tab and its pending sign promise** (a
fund-safety regression). The scheme fails in place and the tab + focus-poll survive. So
the UL can NEVER be chosen speculatively — and you can't "try UL, fall back," because a
failed UL destroys your JS before it can react.

**The design — the app attests, the extension reads (no compile-time flag):**
- The app records `ulVerifiedAt` (a timestamp) the FIRST time it is opened via a
  `https://getvela.app/sign` UL — which PROVES the association resolves on this device
  (`<AccountFileWriter/>` observes `Linking` → `markUniversalLinkVerified()`; the flag
  is device-level + persisted; `writeAccountCache` re-reads it on every write and emits
  both `ulVerified` (TTL-checked boolean) and the raw `ulVerifiedAt`).
- The extension reads them off the connect response and passes to
  `signLaunchUrl(rid, useUL)` — UL only when attested, else the proven scheme. So the
  sign path is ALWAYS safe: scheme until attested, then it auto-upgrades. No rebuild.
- **Bootstrap (chicken-and-egg):** the toolbar popup shows a **测试一键签名** probe that
  opens `https://getvela.app/sign?rid=ul-selftest` in a NEW tab (non-destructive — worst
  case a getvela.app page in a throwaway tab, never a dApp tab). Opening it launches the
  app via the UL → the app attests → the badge flips to **✓ 一键签名已启用**. `app/sign.tsx`
  special-cases the `ul-selftest` rid (a confirmation screen; runs no sign).

**Hardening (from the 2026-07-06 adversarial review — a UL can break AFTER attestation,
e.g. the user picks "Open in Safari" for getvela.app; attestation must be perishable):**
- **Perishable (TTL):** attestation expires after 14 days; every successful getvela.app/sign
  UL open refreshes it, so active users never lapse, but a broken UL ages out → reverts to
  scheme. Backstop only.
- **Self-heal (primary):** a failed UL NAVIGATES the dApp tab to getvela.app — where
  content.js also runs. It stamps `UL_PENDING` before a UL launch; if it then loads on
  getvela.app with that stamp fresh, the UL is broken → it writes `UL_BROKEN` (a
  timestamp), which vetoes the UL at every launch site. Heals after ONE bad sign.
- **Race-free re-enable:** the veto holds only while `UL_BROKEN.ts > ulVerifiedAt`. A
  successful re-probe re-attests with a fresher `ulVerifiedAt` that out-dates the veto —
  no optimistic clearing, no window where a still-broken UL looks enabled.
- **Always-reachable reset:** the popup shows **重新测试** even when verified, so a device
  that later breaks is never stuck; the badge flips to a "跳转失败 → 重新测试" prompt when
  vetoed. (Contract locked by unit + popup specs, incl. the veto-vs-reattest ordering.)

**Shipped now (all the client code):** `signLaunchUrl(rid, ulVerified)` +
`universalLinkSelfTestUrl()` (protocol.js); `ulVerified` on the account cache
(app-group-account-sync.ts) + the `<AccountFileWriter/>` UL observer; connect/status
carry `ulVerified` (background.js); content.js captures it per-rid; the popup probe +
badge; the `applinks:getvela.app` entitlement (`plugins/with-native-modules.js`,
env-gated dev-mode). Contract test-locked (unit + popup specs).

**Remaining to actually light it up on device:**
1. **Host the AASA.** Served DYNAMICALLY by a SvelteKit endpoint in this repo —
   `getvela.app/src/routes/.well-known/apple-app-site-association/+server.ts`
   (→ Cloudflare Worker `getvela`), today `webcredentials` only. Add an `applinks` block
   to the SAME `json({...})` (keep `webcredentials`), then `cd getvela.app && bun run deploy`.
   Verify: `curl -s https://getvela.app/.well-known/apple-app-site-association` shows both
   blocks, `200`, `application/json`, no redirect.
   ```ts
   return json({
     applinks: { details: [{ appIDs: ['F9W689P9NE.app.getvela.VelaWallet'], components: [{ '/': '/sign' }] }] },
     webcredentials: { apps: ['F9W689P9NE.app.getvela.VelaWallet'] },
   });
   ```
2. **Native rebuild** so the `applinks` entitlement is installed. Debug is fine and
   preferred for iteration (`npx expo run:ios --device <UDID>`) — the entitlement is
   emitted at prebuild regardless of configuration, and Debug is REQUIRED for the
   dev-mode bypass below. (`--configuration Release` is only needed for the fully
   automated `run_matrix.py`.) Confirm it landed: `codesign -d --entitlements :- <VelaWallet.app>`
   lists `applinks:getvela.app`.
   - **Fast-iterate past Apple's CDN:** iOS ≥14 fetches the AASA from
     `app-site-association.cdn-apple.com/a/v1/getvela.app` (can lag hours). Prebuild with
     `VELA_AASA_DEV_MODE=1` (emits `applinks:getvela.app?mode=developer`) + iPhone Settings ›
     Developer › Associated Domains Development ON → swcd fetches getvela.app directly.
     Never ship a distribution build built with that env set.
3. **Device-verify** (app routing already confirmed: expo-router maps
   `https://getvela.app/sign?rid=…` → `app/sign.tsx` by path, rid intact, NO linking
   config — do NOT add a `prefixes` list, expo-router ignores it). Two checks:
   - **Isolation:** tap `https://getvela.app/sign?rid=test` from Notes (NOT typed in the
     address bar — UL won't fire). Vela opens, no banner, `sign.tsx` shows `rid: test`. If
     Safari loads the web page → not resolving; fix AASA/CDN/entitlement first.
   - **Bootstrap + real flow:** open the toolbar popup → tap **测试一键签名** → the app
     opens via UL and the badge flips to ✓. Then run `e2e/safari` against the parallel
     space (fixture Safes, never real funds) — the launch now emits the UL; assert on each
     sign that the dApp's Safari tab SURVIVES (a navigated-away tab = lost sign; if it ever
     happens, the association regressed — the app just stops attesting and it falls back to
     the scheme on the next cache write).

## Verification

```bash
cd packages/safari-extension
npm run test:unit       # 11 — incl. signLaunchUrl scheme-default contract
npm run test:provider   # 21 — provider + integration + popup + world-guard
```

**Device follow-up (needs a native rebuild — the extension bundle is packaged, not
Metro-served):** rebuild + reinstall, then re-run `e2e/safari/check_real_sign.py`
(both signature methods) to confirm world:MAIN + the launch-helper didn't regress the
sign path, open the toolbar popup, and — only after the AASA is live — device-verify UL.
