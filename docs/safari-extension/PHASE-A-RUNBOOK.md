# Phase A runbook — the real EIP-1193/6963 provider

**Status:** implemented 2026-07-06. Local verification green (10 `node --test` unit + 12 Playwright browser conformance). Adversarially reviewed (8 confirmed findings, all fixed + re-verified). Device validation via `e2e/safari` is the remaining human step.

### Review fixes folded in (2026-07-06)

- **Read proxy is an ALLOWLIST** (`protocol.js` `READ_PROXY_METHODS`), not a denylist — `eth_signTransaction` / arbitrary methods are refused (`4200`), never forwarded to a public RPC (no open-relay fail-open).
- **eth_chainId uses a STABLE default** (`DEFAULT_EXT_CHAIN_ID = 1`), decoupled from the volatile dApp-bridge chain; each origin picks/switches its own chain (per-origin, stored in the extension).
- **Account cache is never deleted during the boot LOADING window** (a slow AsyncStorage restore no longer nukes a logged-in user's cache).
- **Grants are re-validated** against the current account cache (a removed account is not returned) — but never dropped on a cold-cache miss.
- **Cross-tab safe:** only the tab that created a sign `rid` settles/clears it (`signMap` ownership), so a second same-origin tab can't strand the first's pending promise.
- **`connect` event ordering** fixed (a premature `connected` flip on the `eth_accounts` warm no longer suppresses `connect`) — locked by a regression test.
- proxyRpc abort timer now covers the response-body read; `send()` throws coded errors.

### Device-found fix (2026-07-06) — the sign hand-off launch

On-device testing (ABC iPhone 11) surfaced a real bug the local tests could not (Chromium can't exercise the `velawallet://` scheme): the sign CTA was an `<a href="velawallet://…">`, but its click handler (`onSignLaunch`) synchronously called `showSignWaiting()` which replaces the sheet's `innerHTML` — **detaching the anchor mid-click, which cancels the default scheme navigation** → the app never launches → `personal_sign` never resolves. Fixed by matching the R1-proven pattern: the CTA is now a **`<button>`** whose handler navigates **imperatively via `window.location.href`** (synchronous, keeps the tap's user activation), and the sheet swap to the waiting state is **deferred to a macrotask** so it can't cancel the in-flight navigation.

### Device verification status (2026-07-06, ABC iPhone 11)

**`check_provider.py` → 5/5 PASS** on the ABC iPhone 11 (button-CTA build): provider injected ✓, `eth_requestAccounts` → real address `0x14fB…eA5c` ✓, `eth_chainId` `0x1` ✓, `eth_getBalance` **real balance via the read-proxy** ✓, `personal_sign` → in-Safari sheet → app hop → return → **dApp promise resolves** (`0xFAc98…`, the Phase-A fake signature; no `4001` false-decline) ✓. The whole loop — EIP-6963 discovery → in-Safari connect → read-proxy → sign hand-off → return → resolve — is proven on hardware.

Harness note: `check_provider.py` waits for the app's "Signed" state before returning to Safari (else the single focus-poll races the App-Group result write). The upgrade-install (`expo run:ios --device --configuration Release`) preserved the extension grant — a full delete+reinstall would reset it (re-enable in Settings › Safari › Extensions).

Phase A replaces the R1 spike's stub (`window.__velaR1` + a hardcoded sign button + a status pill) with a **real `window.ethereum` provider** that dApps discover and drive. Connect / read / state are answered **in Safari, zero app hop**; signing is routed through the already-proven launch + focus-poll return (which still **fake-signs** in Phase A — Phase B swaps in the real `SigningRequestModal`/passkey/bundler).

## What shipped

| File | Role |
|---|---|
| `packages/safari-extension/src/lib/protocol.js` | Shared constants + pure helpers (method classification mirroring the app, error codes, param utils). Unit-tested. |
| `packages/safari-extension/src/inpage.js` | The EIP-1193 + EIP-6963 provider (MAIN world). Tagged `postMessage` transport keyed by `rpcId`; session cache backing sync props; legacy shims. |
| `packages/safari-extension/src/content.js` | Bridge (page ↔ background), the **connect sheet** + **sign hand-off sheet** (open Shadow DOM), and the return-poll rewired to **resolve the dApp promise**. |
| `packages/safari-extension/src/background.js` | Router: STATE from the app-written cache, READ proxied to RPC/bundler, CONNECT grants (storage.local); relays `writeSignRequest`/`pollSignResult` to native. |
| `packages/safari-extension/src/manifest.json` | Adds `host_permissions` so the background can proxy reads to RPC endpoints. |
| `targets/safari/SafariWebExtensionHandler.swift` | New `getAccount` case → reads `vela.ext.account.json`. |
| `src/services/app-group-account-sync.ts` + `src/components/AccountFileWriter.tsx` | App writes the **public** account cache (`vela.ext.account.json`) on account/chain change + every foreground. Mounted in `_layout.tsx`. |
| `packages/safari-extension/testdapp/index.html` | Vanilla EIP-6963 test dApp — the Playwright fixture and the device test page. |

## The account cache (what crosses the App Group)

`vela.ext.account.json`, written by the app, read by the extension:

```json
{
  "address": "0x…", "name": "Main",
  "accounts": [{ "name": "Main", "address": "0x…" }],
  "chainId": 1,
  "chains": { "1": { "name": "Ethereum", "rpcUrl": "https://…", "bundlerUrl": "https://vela-bundler.getvela.app/1" } },
  "updatedAt": 1751800000000
}
```

**Only public data** — address, display name, chainId, public RPC/bundler URLs. No `credentialId`, no `publicKeyHex`, no key material (the container is readable on jailbroken devices).

## Method routing (mirrors the app exactly)

- **Sign** (`personal_sign`, `eth_signTypedData*`, `eth_sendTransaction`, `wallet_sendCalls`) → app hop. `eth_sign` is **refused** (`4200`).
- **Connect** (`eth_requestAccounts`, `wallet_requestPermissions`) → in-Safari consent sheet; grant stored per-origin.
- **State** (`eth_accounts`, `eth_chainId`, `net_version`, `wallet_getPermissions`) → local; `eth_accounts` returns `[]` for an ungranted origin (never prompts).
- **Switch** (`wallet_switchEthereumChain`) → per-origin chain update + `chainChanged`; unknown chain → `4902`.
- **Read** (everything else non-signing) → proxied to `chains[chainId].rpcUrl` (or `.bundlerUrl` for the 5 ERC-4337 methods).

## Fund-safety in the sign path (invariants a–d)

- **Pre-launch cancel** (intent sheet "取消") → `4001` — truthful, nothing was sent.
- **Post-launch** the waiting sheet offers only **"关闭（在 Vela 中继续）"** — it never resolves `4001`, because the app may already have submitted. The focus-poll still delivers the real `submitted`/`rejected`, or settles **`4900`** (not `4001`) at the ~3-min ceiling.
- Timeout / dead-worker / unknown → **`4900`**, never `4001` (a `4001` tells a dApp "safe to retry" → double-spend).
- Single-resolve per `rid` (`resolvedRids`); durable `storage.local` mirror survives reload; result is authoritative on disk + in Vela Activity.

## Build

```bash
node packages/safari-extension/build.mjs          # esbuild → targets/safari/assets/
# then the usual prebuild + device build:
npx expo run:ios --device <UDID> --configuration Release
```
Re-run the esbuild step after every source change (and after `expo prebuild --clean`).

## Local verification (no device)

```bash
cd packages/safari-extension
npm run test:unit        # node --test — protocol classification/params/errors (10 tests)
npm run test:provider    # builds, then Playwright drives the REAL inpage.js in Chromium
                         # against a mock native bridge + the test dApp (11 tests)
```
The Playwright suite proves EIP-6963 discovery, request/response correlation, param handling (personal_sign `[msg,addr]` vs typed-data `[addr,data]`), sync props, events, legacy shims, `eth_sign` refusal, and response dedupe.

## Device verification (`e2e/safari`)

Serve the test dApp somewhere Safari can reach and the extension is granted on (e.g. host `testdapp/index.html`, or `python3 -m http.server` on the LAN), then:

```bash
export VELA_TEST_URL="http://<host>/index.html"   # the served test dApp
export VELA_UDID=…  VELA_TEAM=…
./venv/bin/python check_injection.py   # provider + EIP-6963 present?
./venv/bin/python check_provider.py    # connect → chainId → getBalance → personal_sign
```

`check_provider.py` notes:
- Connect confirm + dApp buttons are driven by synthetic clicks (no launch).
- The **sign CTA** needs a **real** coordinate tap (a synthetic click carries no user activation → iOS drops the scheme launch, FACT-1). Calibrate once: `export VELA_WEB_YOFFSET=<Safari toolbar height in points>`.
- The app must be **logged in** so `vela.ext.account.json` exists (else connect shows the empty-state "打开 Vela 登录").
- Phase A fake-signs, so `personal_sign` resolves with a fake `0xFA…` hash — that is expected; the point is the dApp promise **resolves** (and never with a `4001` false-decline).

## Phase A vs Phase B

Phase A = the dApp-facing provider works and resolves promises. The signature/hash is **fake** (`sign.tsx` still `submitFakeSign`). Phase B (`REAL-INTEGRATION-DESIGN.md`) replaces the fake with the real `ExtensionBridgeTransport` → `SigningRequestModal` → passkey → bundler, behind a two-slot `DAppConnectionProvider`, and delivers real signatures/tx hashes.
