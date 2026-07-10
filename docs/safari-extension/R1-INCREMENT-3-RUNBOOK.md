# R1 Spike — Increment 3: gesture launch + deep link + request hand-off (FACT-1 / R3)

**Goal:** prove (a) a content-script button tap **synchronously launches** `velawallet://sign?rid=<uuid>` (FACT-1 — the crux of the sign hop), (b) Expo Router routes it to a new `src/app/sign.tsx`, and (c) the app **reads the signing request** the extension handed off via the App Group (by rid). NO signing, NO return-to-page (that's Inc 4).

> **✅ Prepared & verified by tooling 2026-07-05 (no device):** 5 edits applied (content.js "R1 sign" button + synchronous launch, background.js `writeSignRequest` relay, handler `writeSignRequest` branch, new `sign.tsx`, `_layout.tsx` Stack.Screen). Critic's reliability fix applied (background returns `true`, not `false`, so the worker survives until the native write dispatches). esbuild rebuilt (`content.js` 4.1kb, `background.js` 1.1kb — new code present); `tsc` 0 errors; `os_log` uses the Inc-2-fixed signature. **Swift not compiled here — on-device build is your step.** No App Group re-registration (same group as Inc 1/2).

## No prebuild needed

All Inc-3 changes are either in the `targets/safari/` **synchronized folder** (the edited `SafariWebExtensionHandler.swift` + the rebuilt `assets/`) or **JS served by Metro** (`sign.tsx`, `_layout.tsx`). None change the native project structure, so `expo run:ios` rebuilds and picks them up without a fresh prebuild.

## Build + run

```bash
cd /Volumes/data/production/vela-wallet
node packages/safari-extension/build.mjs   # REQUIRED — the extension serves targets/safari/assets/ (esbuild bundles), NOT src/. (Already run once.)
npx expo run:ios --device                  # rebuilds native (synchronized-folder Swift + assets) + Metro serves sign.tsx
```

## Step A — deep link ALONE (HARD GATE: proves routing + param, no extension/gesture)

1. On the device, open **Notes**, paste `velawallet://sign?rid=test`, tap the link (most reliable). (Safari address bar + Go also works.)
2. iOS may show a first-time **"Open in \"Vela Wallet\"?"** prompt → **Open**.
3. The app opens `sign.tsx`. **Expected:** `rid: test`, `status: request not found / expired` (bogus rid → no file → poll times out ~3 s). **This alone proves the deep-link routing + `rid` delivery.**
4. Cold-start variant: force-quit the app first, then tap the link.

> If `rid` shows `(none)`, the `velawallet://sign` host-vs-path parse needs a `linking.getStateFromPath` shim (Expo Router usually normalizes host→first segment, but this is the one unverified assumption — that's why Step A is a gate, not a formality). Report it and I'll add the shim.

## Step B — full gesture hand-off (proves FACT-1 launch + write + read)

1. In Safari open any `https://` page (content script injects on load). Confirm two floating buttons bottom-right: **R1 echo** (orange) and **R1 sign** (black, above it).
2. Tap **R1 sign**. (Optional, via Web Inspector page console: `Vela R1 sign -> launching app <rid>`.)
3. iOS launches Vela to `sign.tsx?rid=<rid>`.
4. **Expected:** `status: request received`, then `personal_sign from <origin>` and `params: [...]` — the exact payload the extension wrote to `sign-req-<rid>.json`.
5. If the app doesn't foreground within ~1.5 s (not installed / Cancel tapped), the button relabels to **Tap to open Vela** — tap again (fresh gesture; never auto-retry).

## ✅ Increment 3 PASS = all three

> **VERIFIED ON DEVICE 2026-07-05 — all three green.** Step A (`velawallet://sign?rid=test`) → `rid: test` / `request not found`. Step B (tap R1 sign → "Open in Vela?" banner → Open) → `sign.tsx` showed `request received` + `personal_sign from <origin>`. FACT-1 launch + deep-link routing + App Group hand-off confirmed. **R3: custom scheme shows the "Open in Vela?" banner (one extra tap) — Universal Links will make it one-tap-direct (parallel polish).** Proceeding to Increment 4.

- **(a)** Tapping **R1 sign** launches the app to `sign.tsx` → **FACT-1 gesture launch confirmed** (the critical S1 risk).
- **(b)** `sign.tsx` shows the correct `rid` from the deep link → param delivery.
- **(c)** `sign.tsx` displays `method`/`origin`/`params` from the extension-written `sign-req-<rid>.json` → App-Group request hand-off.

Isolation if one fails: (a) = scheme/launch (FACT-1 / gesture) · (b) = router/param (Step A catches it) · (c) = write path or poll race (bump `timeoutMs` in sign.tsx; confirm same App Group).

## 👀 R3 observation to report (decides the sign UX)

On the FIRST launch of Step B, note **which happens**:
- **Direct open** (app just opens), or
- **"Open in \"Vela Wallet\"?" system banner** requiring one tap.

This determines whether the real sign hop is one-tap or needs the banner affordance. Tell me what you see.
