# Vela Safari Web Extension — Architecture

**Goal:** After a user installs Vela Wallet on iOS, every EIP-1193 / EIP-6963 dApp opened in **iOS Safari** receives an injected `window.ethereum` provider, exactly like a desktop MetaMask extension. Connecting and reading happen silently in Safari; **only signing hops once to the native Vela app** (where the passkey lives), then returns.

**Status:** Proposal / architecture. No code written yet.
**Scope:** **iOS only.** Web wallet (`wallet.getvela.app`) and Android are explicitly out of scope for this feature. macOS Safari is a possible free side-effect but not a target.

> **⚠️ v2 — HARDENED.** An adversarial iOS-reliability review surfaced three verified facts that force structural changes. **§4 (steps ②/④/⑦), §5, and §6 below are partially SUPERSEDED — treat [§12 Hardening & Design Addendum](#12--hardening--design-addendum) as authoritative** wherever they conflict. Governing invariant: *a submitted transaction is never silently lost or duplicated, and an ambiguous outcome never renders as a clean decline.*

---

## 0. Decisions (locked)

1. **iOS-only.** Design against iOS Safari Web Extension reality; ignore web/Android portability here.
2. **Signing hops to the native app; everything else does not.** The passkey (`rpId=getvela.app`, Secure Enclave, non-extractable) can only be exercised in the native app (associated domain `webcredentials:getvela.app`) — verified. So the signature ceremony must happen there. Connect / read / state do not need the key, so they stay in Safari.
3. **Minimize hops.** Connect (`eth_requestAccounts`), all read RPC, `eth_chainId`/`eth_accounts`, chain switches → answered in-Safari, **zero app-switch**. Only `personal_sign` / `eth_signTypedData*` / `eth_sendTransaction` / `wallet_sendCalls` hop.
4. **The extension holds ZERO key material.** No seed, no private key, no `credentialId` for signing. It relays and caches public state only. Compromising the extension leaks browsing metadata, never funds.
5. **Reuse, don't rebuild.** Signing reuses the native app's existing pipeline verbatim via a new `DAppTransport` implementation (see §5). No new signing UI, no new bundler code, no new passkey code on the native side.
6. **`wallet.getvela.app` is NOT the iOS signer.** The native app is always installed on iOS (it is what delivers the extension), always logged in, always warm — the most reliable and available signing surface. The web wallet's in-Safari tab is fragile on iOS (aggressive tab eviction, ITP 7-day storage purge, cold bundle load) and is reserved for desktop/cross-device, not this feature.

---

## 1. Components: new vs reused

```
NEW (this project)                                    REUSED (already in the app)
─────────────────────────────────────────            ──────────────────────────────────────
packages/safari-extension/  (esbuild web bundle)      src/services/dapp-transport.ts   (DAppTransport iface)
  inpage.ts      EIP-1193 + EIP-6963 provider         src/models/dapp-connection.tsx   (DAppConnectionProvider)
  content.ts     relay + Shadow-DOM sheet UI          src/hooks/use-dapp-signing.ts    (isSigningMethod,
  background.ts  read/write router, native bridge       handleDAppRequest, handleReadOnlyRPC,
  sheet/*        connect sheet + sign hand-off UI        INSTANT_READONLY_METHODS)
  manifest.json  MV3                                   src/components/SigningRequestModal.tsx (the ONE sign UI)
                                                       src/modules/passkey (native Passkey.sign)
targets/safari/  (Apple extension target)             src/services/safe-transaction.ts (SafeOp hash)
  SafariWebExtensionHandler.swift  App Group I/O       vela-relay.getvela.app           (UserOp submit)
  Info.plist / entitlements                            src/services/approval-guard.ts   (never-unlimited)
                                                       simulateAssetChanges             (asset preview)
src/services/extension-bridge-transport.ts  NEW        LocalTransaction persistence     (activity replay)
src/app/sign.tsx  (deep-link route)          NEW
plugins/  App Group + applinks entitlements  EDIT
```

The load-bearing reuse: **`ExtensionBridgeTransport implements DAppTransport`** plugs the extension into the *exact same* `DAppConnectionProvider` that today serves WalletPair and remote-inject sessions. The native app cannot tell an extension-originated request from a QR-paired one; both flow through `request(id, method, params, origin)` → `SigningRequestModal`.

---

## 2. The injection (page side)

### 2.1 Chain

```
iOS Safari tab (dApp page)
┌─ MAIN world ───────────────┐   ┌─ ISOLATED world ─────────────────────────┐
│ inpage.js                  │   │ content.js                                │
│  window.ethereum (1193)    │◀─▶│  • relays request/response (postMessage)  │
│  EIP-6963 announce         │pM │  • hosts the Shadow-DOM sheet UI          │
└────────────────────────────┘   │  • browser.runtime.* to background        │
   injected via                  └───────────────────┬───────────────────────┘
   <script src=getURL('inpage.js')>                   │ runtime.sendMessage
   at document_start                                   ▼
                                          background.js (MV3 non-persistent event page)
                                            • READ  → answer directly (Vela RPC)
                                            • STATE → answer from App Group cache
                                            • SIGN  → native bridge → app hop
```

### 2.2 MAIN-world injection technique (iOS floor)

Content script (`run_at: document_start`, ISOLATED) injects a `<script src=runtime.getURL('inpage.js')>` element; `inpage.js` is in `web_accessible_resources`. Works **iOS 15+**. Do **not** use manifest `world:"MAIN"` (iOS 18+ only) as the primary path.

```js
// content.js
const s = document.createElement('script');
s.src = browser.runtime.getURL('inpage.js');
s.async = false;
s.onload = () => s.remove();
(document.head || document.documentElement).prepend(s);
```

- **MV3, floor = iOS 15.4.** Background is a **non-persistent event page** (`"background": { "scripts": ["background.js"], "persistent": false }`), not a pure service worker (iOS 17.4+ evicts SWs unreliably). Treat background as **stateless** — reload account/chain from App Group on every wake.
- **Per-site grant:** nothing injects until the user enables the extension for the site (Safari "Aa" → Vela → Allow). Surface this once in onboarding.
- **CSP caveat:** the injected `<script>` is subject to the page's CSP; strict-CSP dApps (Uniswap-class) may block it on iOS 15–17. Track block rate (§9). iOS-18-only `world:"MAIN"` is a future progressive enhancement for these.

### 2.3 The provider (`inpage.ts`)

Hand-rolled ~300-line EIP-1193 provider (not `@metamask/providers`). Transport = `window.postMessage` to content script, keyed by a page-local `rpcId`.

```ts
interface VelaInpageProvider {
  request({ method, params }): Promise<unknown>;
  on(event, cb): this;          // accountsChanged | chainChanged | connect | disconnect
  removeListener(event, cb): this;
  isVela: true;
}
```

EIP-6963: freeze `{ uuid: <per-session uuidv4>, name: "Vela Wallet", icon: <data-uri ≥96²>, rdns: "app.getvela" }`, dispatch `eip6963:announceProvider` eagerly + on `eip6963:requestProvider`. Also set `window.ethereum` defensively for legacy dApps (don't clobber a pre-existing provider).

### 2.4 Read/write classification (mirror the app's own split)

The background reuses the **same method classification the app already ships** so behavior is identical across QR-paired and extension sessions:

| Class | Methods | Answered by | Hop? |
|---|---|---|---|
| **Sign (write)** | `eth_sendTransaction`, `wallet_sendCalls`, `personal_sign`, `eth_sign`, `eth_signTypedData[_v3/_v4]` | native app (passkey) — see §4 | **1 hop** |
| **Connect** | `eth_requestAccounts` | in-Safari sheet (§3) | no |
| **State** | `eth_accounts`, `eth_chainId`, `net_version`, `wallet_getPermissions` | background ← App Group cache | no |
| **Chain** | `wallet_switchEthereumChain`, `wallet_addEthereumChain`, `wallet_watchAsset` | background (updates cache, emits `chainChanged`); add-chain may show a light in-Safari confirm | no |
| **Read** | `eth_call`, `eth_getBalance`, `eth_estimateGas`, `eth_getLogs`, receipts, … (the app's `INSTANT_READONLY_METHODS` / `handleReadOnlyRPC` allowlist) | background → Vela RPC endpoints | no |

Background read-proxy hits the same endpoints the app uses (`vela-relay.getvela.app/<chainId>` for bundler methods; per-network `rpcURL` for node methods). It applies the **same read-only allowlist** as `walletpair-transport.ts` — never proxy a method outside it.

---

## 3. Connect flow — in-Safari, zero hop (the beautiful sheet)

`eth_requestAccounts` from an origin **with no stored grant**:

```
dApp calls eth_requestAccounts
  → inpage → content → background
  → background: no permission for origin?  → tell content to render CONNECT SHEET
  → content injects Shadow-DOM bottom sheet:
        "biubiu.tools 想连接        Ethereum ▾
         ✓ 查看余额与活动
         ✓ 请求交易批准
         ✗ 未经允许不会动用资金
         [ 取消 ]            [ 确认 ]      "
     (address + accounts + chainId read from App Group cache; NO passkey, NO app)
  → user taps 确认
  → background writes permission grant to App Group:
        { origin, address, chainId, grantedAt }
  → inpage resolves eth_requestAccounts → [address]
  → provider emits 'connect' { chainId } and 'accountsChanged' [address]
```

- Subsequent `eth_requestAccounts` / `eth_accounts` from a **granted** origin resolve **instantly from cache**, no sheet, no hop. Connect is a one-time sheet per site.
- The granted site becomes a first-class **Connection** in Vela's model (persisted so it shows in the app's Connections surface with its activity feed).
- **Empty-state:** if App Group has no account (app never opened / not logged in), the sheet shows a single CTA "打开 Vela 登录" that opens `velawallet://` once. Rare; after first login the address is always cached.
- **Account/chain switch mid-session:** the app writes new `{address, chainId}` to App Group on change; the content script re-reads on `visibilitychange→visible` and emits `accountsChanged`/`chainChanged` (iOS has no app→extension push, so propagation is pull-on-focus).

---

## 4. Sign flow — one clean app round-trip

> **⚠️ Steps ②/④/⑦ superseded by §12.1.** `rid` is minted in `inpage` (not the background); the launch is a **synchronous tap** on the hand-off sheet's `<a href="velawallet://sign?rid=">` (user-activation cannot survive the async hops — FACT-1); the return path does **not** go through the background service worker (it is evicted and does not reliably wake — FACT-3). Read §12.1–12.3 as authoritative.

Signing reuses the native pipeline through `ExtensionBridgeTransport` (§5). The hard part is the **response channel** (a deep-link has none) and the **user-gesture** requirement for launching the app. Solved with an **App Group mailbox keyed by `requestId`** + **pull-on-focus** return.

```
① dApp "Confirm" click → inpage.request({method:'eth_sendTransaction', params})
     inpage stores pending Promise by rpcId, postMessage → content → background
② background classifies as SIGN. Generates requestId (uuidv4).
     sendNativeMessage({ type:'enqueueSign', requestId, method, params, origin, chainId })
③ SafariWebExtensionHandler.beginRequest (extension process):
     writes App Group mailbox entry:
       vela.ext.sign.<requestId> = { method, params, origin, chainId, status:'pending', createdAt }
     returns { ok:true } to background
④ background tells content: "openApp(requestId)".
     content opens  velawallet://sign?rid=<requestId>   ← MUST be inside the click's user gesture
     (iOS may show a one-tap "Open in Vela" affordance; custom scheme opens more directly than applinks)
⑤ Native app launches → route src/app/sign.tsx, reads ?rid
     → activates ExtensionBridgeTransport(rid): reads mailbox entry
     → emits DAppTransport 'request' (rid, method, params, origin)
     → EXISTING DAppConnectionProvider renders <SigningRequestModal>:
           clear-signing · simulateAssetChanges · gas/funding · approval-guard(no-unlimited)  [ALL REUSED]
     → user approves → Passkey.sign(safeOpHash) → UserOp → vela-bundler → userOpHash
     → onSubmitted: buildSigningRecord → LocalTransaction(status:'pending', dappOrigin)  [persist-at-submit]
     → transport.sendResponse(rid, userOpHash):
           writes mailbox  vela.ext.sign.<requestId> = { status:'submitted', result:userOpHash }
⑥ App tries to return to Safari (openURL back / or user taps Safari). Safari tab is kept alive by iOS.
⑦ Content script on visibilitychange→visible:
     runtime.sendMessage → background → sendNativeMessage({type:'poll', requestId})
     → handler reads mailbox → returns { status, result | error }
     → background → content → inpage resolves the rpcId Promise with userOpHash
     dApp sees the tx hash. Poll with backoff; timeout ~3 min → reject 4001.
```

**Reject / cancel:** `SigningRequestModal` reject → `sendResponse(rid, undefined, {code:4001})` → mailbox `{status:'rejected'}` → inpage rejects.

**requestId rules:** fresh uuidv4, **single-use** (consumed on read), **TTL** (reject > 5 min), **origin-pinned** (app displays and binds to the origin recorded at enqueue; a poll for a mismatched origin fails). The app never trusts a client-supplied result — only what its own passkey produced.

**Gesture risk (the #1 on-device unknown):** step ①→④ crosses async hops (postMessage, runtime.sendMessage, native message) before the URL is opened, which can drop the user-activation iOS requires to launch the app without a blocked-popup. Mitigations to validate (§9): pre-generate `requestId` in `inpage` and open `velawallet://sign?rid=` from the content script **synchronously** on the same gesture while the payload write to the mailbox races (the app waits/polls for the payload to appear by `rid`); accept a one-tap "Open in Vela" banner as the fallback.

---

## 5. Native integration: `ExtensionBridgeTransport implements DAppTransport`

> **⚠️ Superseded by §12.2.** `DAppConnectionProvider` has **no** "transient session" API — it holds a single `transportRef` (dapp-connection.tsx:219) and every connect calls `disconnectCurrent()` (:372/:397), which would kill a live WalletPair session. A **two-slot `signTransportRef` + public `beginExtensionSign()`** is required, and `ConnectionType` (:84) needs `'extension'`. App Group I/O is **net-new native work** — `expo-file-system` cannot reach the container. See §12.2.

The elegant reuse. `src/services/dapp-transport.ts` already defines:

```ts
interface DAppTransport {
  connect(): Promise<void>;
  sendResponse(id, result?, error?): void;
  pushWalletInfo(info): void;
  on('request', (id, method, params, origin) => void): () => void;
  // connected | disconnected | reconnecting | error
}
```

`ExtensionBridgeTransport` is a **third implementation** next to `RemoteInjectTransport` (SSE) and `WalletPairTransport` (WS):

- **Backing store:** the App Group container (not a relay — same device). A tiny native module (or the existing `expo-file-system` over the shared container) reads/writes `vela.ext.sign.<rid>`.
- **`connect()`** (called by `sign.tsx` with a `rid`): read the mailbox entry, then `emit('request', rid, method, params, origin)`. It is effectively **one-shot** per launch (one rid), unlike the persistent relay transports.
- **`sendResponse(rid, result, error)`:** write the result/error back to `vela.ext.sign.<rid>` and mark consumed.
- **`pushWalletInfo` / events:** no-op or write current `{address, chainId}` to the shared account-state key (this is also what keeps the extension's connect cache fresh — see §6).

Wiring: `src/app/sign.tsx` (new Expo Router route) reads `rid`, constructs an `ExtensionBridgeTransport`, and hands it to `DAppConnectionProvider` as a transient session. `DAppConnectionProvider.handleIncoming` already does `if (isSigningMethod(method)) …` → `handleDAppRequest(...)` → `SigningRequestModal`. **No change to the signing pipeline.** The only new native surface area is the transport (~150 LOC), the route (~80 LOC), and a shared-container read/write helper.

---

## 6. State sharing (App Group `group.app.getvela.wallet`)

> **⚠️ Superseded by §12.1.2 / §12.4.** The mailbox is an **immutable-file** container signalled by **Darwin notifications** — NOT `UserDefaults` (stale cross-process reads; KVO fails on keys with periods) and NOT `expo-file-system`. Result TTL is **decoupled** from request TTL (results persist for hours, until delivered-and-acked; reconcile from `LocalTransaction` if gone). See §12.

| Key | Writer | Reader | Contents |
|---|---|---|---|
| `vela.ext.account` | native app (on login / account / chain switch) | extension background | `{ address, chainId, accounts:[{name,address}] }` — **public** |
| `vela.ext.perms` | extension background (on connect approve) + app (on disconnect) | both | `{ [origin]: { address, chainId, grantedAt } }` |
| `vela.ext.sign.<rid>` | extension handler (enqueue) → native app (result) | both | one signing request + its result; TTL 5 min, single-use |

**Nothing sensitive crosses.** No `credentialId` for signing, no key material — the shared container is readable on jailbroken devices, so it carries only public address + chainId + permission grants + unsigned request payloads. Keys stay behind the passkey/Secure Enclave in the app.

---

## 7. UI surfaces (design)

Rendered as **content-script-injected Shadow-DOM** bottom sheets (closed shadow root, CSS-isolated from the dApp, styled with Vela tokens: hairline dividers, accent = "moving money / submit", `VelaButton` as the single CTA). Reliable on iOS (no remote-iframe quirks).

1. **Connect sheet** — §3. The real UI. Origin favicon + name, account row (name + truncated address + "Change"), network pill, the ✓/✗ permission list, Cancel / Confirm. Matches the Nightly reference the founder shared. Fully in-Safari.
2. **Sign hand-off sheet** — minimal. Origin, decoded intent preview (method + human summary, reusing decode logic — **preview only, no passkey**), single CTA **"在 Vela 中确认 →"** that triggers the app hop (§4 step ④), plus Cancel. This is the "prompt the user to go to the app" surface the founder anticipated. Keep it tight and premium; the authoritative clear-signing sheet is the native one.
3. **Toolbar popup (optional, later)** — status: connected origin, active account, switch account, disconnect. Low priority; static build (no signing, no keys).

Design language must match the app (see `docs/DESIGN-LANGUAGE.md`); the connect sheet is Vela's first impression inside the browser and is held to the same bar as the native `SigningRequestModal`.

---

## 8. Packaging (Expo, iOS-only)

`ios/` is gitignored + prebuild-generated ("config plugin or it doesn't persist"). The existing `plugins/with-native-modules.js` already uses `withEntitlementsPlist` (adds `webcredentials:getvela.app`), `withDangerousMod` (copies Swift into the Xcode project), and `withXcodeProject` — the exact toolkit needed.

- **Target creation:** **`@bacons/apple-targets`** (`type:"safari"` → `com.apple.Safari.web-extension`), files in `targets/safari/`, **primary**. Fallback if its alpha `@bacons/xcode` breaks on the SDK: extend `with-native-modules.js` with a `withSafariExtension` that adds the target + build phases via `withXcodeProject` (the team already hand-rolls pbxproj manipulation).
- **Web assets:** build `packages/safari-extension/` with **esbuild** (independent of Metro), `outdir → targets/safari/assets/`. Wire into prebuild via EAS `prebuildCommand` (`esbuild && expo prebuild`) or a `withDangerousMod`. Static popup build — no Metro/localhost HMR (extension CSP blocks it).
- **Entitlements (config-plugin-persisted):**
  - App Group `group.app.getvela.wallet` on **both** the app (extend `withIOSEntitlements` → `com.apple.security.application-groups`) and the extension target (mirror in `expo-target.config.js` — `@bacons/apple-targets` does **not** auto-add it for `safari`).
  - **MVP deep-link = existing `velawallet://`** (already registered in `app.json`; zero server change). Add `applinks:getvela.app` (+ an `applinks` block to the AASA at `getvela.app/.well-known/apple-app-site-association`, which today is `webcredentials`-only — do not disturb that block) in Phase 3 for a banner-free open.
- **Bundle id:** `app.getvela.VelaWallet.safari` (child of `app.getvela.VelaWallet`). New provisioning profile + EAS credentials; App Group capability enabled on both App IDs.
- Adding a target changes the native fingerprint → new `runtimeVersion` (policy `fingerprint`) — expected.

---

## 9. Security model

- **Extension holds zero keys.** Preserved *because* signing can't happen in the extension. Compromise leaks browsing/connection metadata, not funds.
- **Origin integrity:** every request carries the true page `origin` observed by the content script (the page can't forge what the content script reads). No RPC is served for an origin without a stored grant except the connect sheet itself.
- **Never blind-sign / never-unlimited:** signing routes through the existing `SigningRequestModal` + `approval-guard` + `simulateAssetChanges`; the extension adds **no** fast-path that bypasses the sheet. Must estimate the real tx (never blind-submit an unestimated large op).
- **requestId:** single-use, TTL 5 min, origin-pinned via the authenticated App-Group rendezvous — closes the "deep-link has no response channel / replayable rid" gap.
- **Per-site OS gate:** Safari's per-site extension access is an extra layer — nothing runs on a site the user hasn't allowed.
- **Anti-spoof:** stable `rdns=app.getvela` + App Store distribution (only real Vela ships this provider). The native sheet shows the dApp `origin` prominently; users approve against origin, not extension claims.

---

## 10. Phased plan

- **Phase 0 — Skeleton (3–5 d).** `@bacons/apple-targets` safari target + App Group + esbuild pipeline; empty `inpage`/`content`/`background`; `SafariWebExtensionHandler` App-Group echo; static popup. Prove prebuild reproducibility + on-device install + per-site grant. *Validates packaging risk.*
- **Phase 1 — Connect + read, zero hop (1–1.5 wk).** EIP-1193 `request` transport; read-RPC proxy; `eth_accounts`/`eth_chainId` from cache; the **connect Shadow-DOM sheet**; permission store in App Group; EIP-6963 announce. No signing yet. *Delivers the Nightly-parity connect experience.*
- **Phase 2 — Sign via app hop (1.5–2 wk).** `enqueueSign` → `velawallet://sign` → `src/app/sign.tsx` + `ExtensionBridgeTransport` → reuse `SigningRequestModal` → bundler → mailbox return → focus-poll resolve. `personal_sign` first, then `eth_signTypedData_v4`, then `eth_sendTransaction`/`wallet_sendCalls`. persist-at-submit + Connections activity. *Validates the gesture/deep-link/return round-trip — the core risk.*
- **Phase 3 — Polish (1 wk).** `applinks` banner-free open; `wallet_switchEthereumChain` + `chainChanged`/`accountsChanged` focus-poll; toolbar popup; iOS-18 `world:"MAIN"` progressive enhancement for CSP-strict dApps; the sign hand-off sheet preview decode.

**Total ≈ 4–6 weeks.** New code is small; most of Phase 2 is reuse.

---

## 11. On-device validation (must-test unknowns)

1. **Gesture → app launch** (§4): does `velawallet://sign` open the app from the content script after the async hops, or is user-activation lost / a banner shown? Test the pre-generated-rid synchronous-open mitigation. **Highest risk.**
2. **CSP block rate** (§2.2): what % of top dApps block the `<script>` injection on iOS 15–17? Decides urgency of the iOS-18 `world:"MAIN"` path.
3. **Background event-page wake** (§2.2): does read RPC / poll survive iOS 17.4+ background eviction reliably?
4. **Return-to-Safari focus poll** (§4 ⑦): does `visibilitychange` reliably resolve the inpage promise after the round-trip, including app-killed-mid-flight?
5. **`sendNativeMessage` completion** reliability (intermittent non-firing reported) → fallback to `connectNative` port.
6. **App Group container access from Expo/RN** (§5): confirm read/write to the shared container from the app process and the extension process with the chosen helper.

---

## 12.  Hardening & Design Addendum (v2 — authoritative)

Governing invariant, as an engineering contract: **a submitted transaction is never silently lost or duplicated, and an ambiguous outcome never renders as a clean decline.** Three *verified* iOS facts force structural changes (not tuning):

- **FACT-1 (gesture).** The app launch must originate in the **tapped page window's synchronous handler**. `postMessage` / `runtime.sendMessage` / `sendNativeMessage` deliver control to contexts that carry **no** user activation — so the background/native side can *never* launch the app, and no amount of speed rescues a post-hop launch. *(Verified: WebKit User Activation API; activation neither survives the async gap nor propagates across the message boundary.)*
- **FACT-2 (App Group).** Both processes *can* share the App Group, but `UserDefaults(suiteName:)` is the wrong primitive (cfprefsd per-process cache → stale reads; no cross-process notification; KVO silently fails on keys containing periods — ours do). `expo-file-system` cannot reach the container at all. *(Verified: Safari Web Extensions are NOT in the privacy sandbox that blocks shared-container writes, so the container is available — but only immutable-file + Darwin-notification IPC is reliable.)*
- **FACT-3 (background eviction).** An MV3 non-persistent background evicted after ~30–45 s **does not reliably wake** on iOS 17.4–18.6.2; `runtime.sendMessage` returns `undefined` synchronously to a dead worker with **no error**, and iOS 18.4.1/18.5 also silently drop `onMessage`. **This refutes the entire §4⑦ "focus-poll through the background service worker" return path** — the single most dangerous assumption in the v1 doc. *(Device-only bug; does not reproduce in Simulator/Chrome/Firefox/macOS Safari — an easy-to-miss landmine.)*

### 12.1  Stability hardening

**12.1.1 The launch — one synchronous gesture, one rid authority.**
- **`rid` is minted once, in `inpage` (MAIN world), via `crypto.randomUUID()`**, before the hand-off sheet's launch anchor renders. The native handler and `sign.tsx` use the supplied rid **verbatim**, never minting their own (deletes v1 §4②'s "background generates requestId" — dual generation is a 100%-failure contradiction).
- **Launch = a real tap on the hand-off sheet's CTA**, rendered `<a href="velawallet://sign?rid=…">` (or a synchronous `location.href =` in the tap handler). Do **not** chain the dApp's own "Confirm" click (dApps `await` fee/nonce work between click and `eth_sendTransaction`, so its activation is already dead; the content script is reached only via async `postMessage`). The sheet tap is a fresh page-window activation; content scripts share it.
- **Zero `await` between tap-handler entry and navigation.** The mailbox payload write **races** the launch (fire-and-forget `sendNativeMessage` after/parallel to `location.href`); the app polls for the payload by rid on arrival. Enforce with a test asserting no `await` on the launch path.
- **Universal Links primary, custom scheme fallback** — pull `applinks:getvela.app` forward to **Phase 2** (domain-verified, unhijackable; a leaked rid stays inert — payload is behind the App Group, rid is single-use + origin-pinned). ULs still require a real anchor tap; the sheet provides it.
- **Detect no-launch:** if `visibilitychange` doesn't fire within ~1.5 s of the tap, show an explicit "Tap to open Vela" affordance (a fresh gesture-bound anchor). Budget copy + telemetry for a first-run "Open in Vela?" system interstitial.

**12.1.2 The return channel — bypass the JS background entirely (the core rewrite).**
1. **Vela Activity is the source of truth, not the dApp response.** The instant the bundler accepts, persist `LocalTransaction{status:'pending', dappOrigin, userOpHash}` (persist-at-submit) — this makes every silent-loss symptom survivable: the user always sees the real outcome in Vela.
2. **App writes the result as an immutable, atomically-written file** `sign-result-<rid>.json` into the App Group **file** container (temp-write + `rename`; never a mutated shared file, never `UserDefaults`), then posts a **payload-less Darwin notification** (`CFNotificationCenterGetDarwinNotifyCenter`) = "scan the mailbox". Set `NSFileProtection` = **`completeUntilFirstUserAuthentication`** (survives lock after first unlock; not `complete`, which is unreadable if the phone locks mid-flow).
3. **The return poll reaches native through the background — there is no bypass.** ⚠️ **R2 RESOLVED (Inc 2 research, 2026-07-05) — the "content-script → handler directly" idea is REFUTED.** On Safari iOS, `browser.runtime.sendNativeMessage` is **background-only**; a content script **cannot** call native directly, and `connectNative` ports **die with the evictable MV3 worker**. So every native round-trip is gated by the evictable background — the return path **cannot** avoid it.
   - **Corrected design direction:** keep the result **authoritative in native / Vela Activity** (persist-at-submit); make each JS↔native hop a **short atomic `sendNativeMessage` round-trip with a client-side timeout**; treat a dropped background as an *expected* path whose guaranteed recovery is the user reopening Safari → the content script re-relays through a freshly-woken background → reads the App-Group result the native side already persisted. The native side owns all wait/poll state; the JS never long-polls. Full return-path redesign is a later increment (Inc 4+).
4. **Treat a synchronous `undefined` / non-firing completion as possible silent failure, never "pending".** Timeout every native call (~2 s), retry idempotently (poll is a pure read); after N failures surface an explicit "Return to Vela to check" affordance. Never a silent poll that can hang forever.
5. **Idempotent, lazy single-use.** The result file is deleted only *after* the inpage promise resolves and the page acks. Enforce TTL + single-use **on read** (non-persistent background can't run reliable timers — no timer GC). Dedupe double-delivery per `rpcId` in the provider.

**12.1.3 Error semantics — never a decline-shaped ambiguity.**
- **Timeout/unknown → a distinct non-4001 code** (`4900 "pending/unknown — check Vela Activity"`). Reserve `4001` for an *explicit* reject in `SigningRequestModal`. dApps treat `4001` as "declined, safe to retry" → a stuck-but-submitted tx becomes a double-spend. Highest fund-safety leverage in the doc.
- **Order events on return:** deliver a pending sign result **before** any `chainChanged`/`accountsChanged` in the same visibility cycle — a spurious `chainChanged` triggers dApp `location.reload()`, orphaning the completed sign.

**12.1.4 TTL decoupling.**

| Item | TTL | Rule |
|---|---|---|
| Request payload `vela.ext.sign.<rid>` | 5 min | Reject enqueue if older; app rejects a stale-request poll |
| **Result** `sign-result-<rid>.json` | **hours, until delivered-and-acked** | Never discard a known-good `submitted{userOpHash}` on a short timer; reconcile from `LocalTransaction` if the file is gone |

The `submitted` write happens atomically with / immediately before the bundler-accept UI transition, so the crash window excludes the result write.

**12.1.5 Cross-tab & cold-start correctness.**
- **Pending-rid queue lives in the App Group (persistent), never in background memory.** After a reload the page's original promise is gone; the content script persists the pending rid in `browser.storage.local` and on every load re-arms a fresh awaiter that re-subscribes by rid and polls. Reconstruct, don't "correlate across."
- **App-side queue:** the deep-link handler **enqueues, never replaces**; `sign.tsx` drains *all* pending entries and `SigningRequestModal` serializes them. A second-tab sign mid-modal is queued, not dropped.
- **Cold-start guard:** queue the deep link until the app is signing-ready (logged in, account loaded); never render `sign.tsx` against an unready store.
- **Concurrent-transport collision:** verify whether the provider can hold a WalletPair session *and* an extension sign at once (see §12.2); if not, serialize through the same queue.

**12.1.6 Account/permission freshness at sign time.**
- Write `vela.ext.account` **on every app foreground/launch**, not only on change (a user who installed while already logged in otherwise has an empty extension cache).
- On every `visibilitychange→visible`, re-read the App Group **fresh** before answering `eth_accounts`/`eth_chainId`.
- `sign.tsx` re-validates the origin against current `vela.ext.perms` and reconciles the origin's granted address vs the active account; mismatch → prompt to switch or reject, never silently sign from the wrong account.

### 12.2  Integration corrections (`src/models/dapp-connection.tsx`)

**Mental model (read this first).** The extension IS "a new protocol" — but only at the **signing layer**. It is a third `DAppTransport` (next to `RemoteInjectTransport` and `WalletPairTransport`) that carries **signing requests only**. Do **NOT** model the extension as a full WalletPair-style client that routes *every* request (connect/read/sign) to the native app as the "wallet peer" — that re-introduces an app-hop on connect and on every read, destroying the in-Safari experience and adding a cloud dependency. The correct shape is a **hybrid**: connect/read/state are answered locally in the extension (zero hop, §3); only signing is forwarded over `ExtensionBridgeTransport`. Implement the app's own `DAppTransport` interface **directly** (like `RemoteInjectTransport` does) — do not route through WalletPair's cross-device session/handshake protocol, which is too heavy for a same-device per-signature hop. What is reused is the **abstraction** (`DAppTransport` + the whole `DAppConnectionProvider` → `SigningRequestModal` pipeline); what is genuinely new is the **same-device transport mechanism** (App Group files + native messaging + deep link) — that cannot be borrowed from WalletPair's WS relay.

**Confirmed reuse (real):** the *signing pipeline* is genuinely transport-agnostic — `handleIncoming` branches on `isSigningMethod` and only sets `incomingRequest` (:248–270); `<SigningRequestModal>` reads only `incomingRequest`/`dappInfo`/`chainId` with an origin fallback; `handleDAppRequest` calls `enforceNoUnlimited` regardless of source; `sendResponse` has **no** connected-socket precondition (:549/:636/:646), so a mailbox-write response is valid. Clear-signing + `simulateAssetChanges` + approval-guard are source-independent.

**Corrections (v1 overstated reuse):**

| v1 claim | Reality | Fix |
|---|---|---|
| §5 "hands it to `DAppConnectionProvider` as a transient session" | **No such API.** Single `transportRef` (:219); every connect calls `disconnectCurrent()` (:372/:397) which destroys the current transport; public surface is only `connectToBridge`/`connectToWalletPair`/`confirmFingerprint` (:110–114). Attaching this way **kills a live WalletPair session.** | Add `'extension'` to `ConnectionType` (:84). Add public **`beginExtensionSign(transport)`** installing into a **separate `signTransportRef`** (not `transportRef`, no `disconnectCurrent()`). Route `sendResponse` to the transport that owns the rid. Replicate the identity guard (`transportRef.current === transport`, :433) so teardown never clobbers a concurrent session. |
| §5 interface sketch | Omits mandatory `name`, `connected`, `disconnect()`, `fetchDAppInfo()` (dapp-transport.ts:28–54). | `ExtensionBridgeTransport` implements all four; reports `connected:true` and does **not** emit `'disconnected'` until `sendResponse` completes (early `'disconnected'` nulls the ref and clears `incomingRequest` at :329–335, losing the response channel). |
| §5 "existing `expo-file-system` over the shared container" | **Wrong.** `expo-file-system` exposes only the app sandbox (`Paths.cache`); can't reach `containerURL(forSecurityApplicationGroupIdentifier:)`. The `appGroupKey` in `with-native-modules.js:336` is an Xcode PBXGroup, not an App Group. No `application-groups` entitlement today. | **Net-new native work:** a shared native module/plugin calling `containerURL(forSecurityApplicationGroupIdentifier:)`, entitlement on **both** targets, immutable-file mailbox + Darwin notifications. Re-scope §5's "~150 LOC" upward. |
| §4 deep-link routing "trivial reuse" | **Zero inbound deep-link handling exists** (no `getInitialURL`/`Linking.addEventListener('url')`/`useURL` in `src`; only outbound `Linking.openURL`). `_layout.tsx` `<Stack>` doesn't declare `sign`. | New surface: inbound linking (cold + warm), a signing-ready queue (§12.1.5), declare/configure the `sign` route. |

**Corrected plan:** (1) two-slot provider — persistent `transportRef` + transient `signTransportRef` + rid-keyed queue; (2) `beginExtensionSign`; (3) full-interface `ExtensionBridgeTransport`, stays `connected` until `sendResponse`; (4) new native App Group module (shared app + extension); (5) inbound deep-link + signing-ready queue → `sign.tsx`.

### 12.3  UX spec (finalized) — the sign hand-off's four states

Connect sheet ships as designed (§7.1; zero hop, low risk). The sign hand-off is **one mounted sheet, four states**; the launch fires from **this sheet's CTA tap** (§12.1.1), never chained off the dApp's Confirm.

```
STATE A — INTENT                                   STATE C — WAITING (the make-or-break moment)
┌───────────────────────────────────────┐        ┌───────────────────────────────────────┐
│ 🅑  biubiu.tools                        │        │                ◜  ◝                    │
│     请求一笔交易签名                      │        │                ◟  ◞   breathing ring   │
│ ────────────────────────────────────── │        │                       (NOT a spinner)  │
│ 发送                                     │        │           在 Vela 中完成签名            │
│ 0.05 ETH  →  0x9c4A…F012                │        │        完成后请返回此页面               │  ← honest: no
│ 在 Ethereum · 预计 gas ~$0.02            │        │ ────────────────────────────────────  │    auto-return
│ ────────────────────────────────────── │        │            [ 返回 Vela ]               │  ← re-fires rid
│ [ 取消 ]   [ 在 Vela 中确认  → ]         │        │            [ 取消这笔请求 ]             │  ← writes reject
└───────────────────────────────────────┘        └───────────────────────────────────────┘
   CTA = <a href=velawallet://sign?rid>              after ~6s dead poll → "在 Vela 的活动中查看这笔交易"
```

- **Copy correction:** `完成后自动返回这里` → **`完成后请返回此页面`**. iOS cannot programmatically focus the original tab (`openURL` opens a *new* tab) — never promise an auto-return we can't keep.
- **Self-heal:** on `visibilitychange→visible`, poll the result **through the native handler / `connectNative` port** (§12.1.2), not the background. `submitted`→success; `rejected`→rejected; `pending`→stay.
- **Breathing ring, never a spinner** — nothing happens *on this page*; a spinner lies. **"返回 Vela"** re-opens the same rid (consumed only on *result* read → safe; `sign.tsx` de-dupes). **Dead-worker floor:** after ~6 s of failed polls, show `在 Vela 的活动中查看这笔交易` (Activity is truth) — never hang the ring. **Timeout** ~3 min → State D with the **non-4001** pending code.
- **State D — RESOLVED:** Success (`✓` in `--success` ring, `userOpHash`+copy, auto-dismiss 1.6 s, success haptic); Rejected (muted `✕`, `--fg-muted`, no red, no haptic, resolves 4001); Timeout/Error (`--warning` amber, "未能完成 · 这次没能收到 Vela 的确认。你的资金没有变动。", Retry mints a **fresh** rid). **Color grammar:** `--error` red reserved for actual money-loss danger; ambiguous/recoverable = amber; user-chosen = neutral. No beta/apology banners.
- **Choreography:** `在 Vela 中确认 →` (medium haptic) → warm native `SigningRequestModal` (shows `origin` for anti-spoof) → post-approve `已发送 · 正在返回 Safari` (thin accent hairline L→R ~900 ms) → back in Safari the **identical** `✓`-in-`--success` animates in, so the two success glyphs across the app boundary read as **one continuous confirmation**. Where return is blocked, soften to `已发送 · 返回 Safari 查看` and stay.

### 12.4  dApp compatibility rules (`inpage.ts` + router) — maximize "works on ~all dApps"

- **EIP-6963 eager:** register the `eip6963:requestProvider` listener **first**, announce at eval **and** on every request; frozen `info`; the announced `provider` is the *same object reference* as `window.ethereum`; `rdns:"app.getvela"`; per-page-load `uuid`; data-URI icon.
- After setting `window.ethereum`, `dispatchEvent(new Event('ethereum#initialized'))` (resolves `@metamask/detect-provider` instantly, not its 3 s timeout). Set `window.ethereum` **only if absent**, always **configurable**, honor `providers[]`.
- **Legacy shims:** implement `send`/`sendAsync`/`enable` → `request` (missing these breaks web3.js / ethers ≤v4). Back synchronous legacy props (`selectedAddress`, `chainId` hex, `networkVersion` decimal, `isConnected()`) from an inpage session cache.
- **Param correctness (forward verbatim; native detects by shape):** `personal_sign` = `[message, address]` (detect address by `0x`+40hex, not position); `eth_signTypedData_v4` = `[address, typedData]` (**address first** — opposite; `typedData` string-or-object) — the #1 hand-rolled-provider bug. `eth_accounts` → `[]` for ungranted/locked (**never prompt/error**); only `eth_requestAccounts`/`wallet_requestPermissions` open the connect sheet. `eth_chainId` = minimal lowercase hex; `net_version` = decimal. `wallet_switchEthereumChain` unknown chain → reject `{code:4902}`; success → resolve `null` + emit `chainChanged`. EIP-5792 `wallet_sendCalls`: implement the full family (`getCapabilities`/`getCallsStatus`/`showCallsStatus`) at one pinned spec version.
- **Events under pull-on-focus:** **never** emit `accountsChanged([])` on a cold-cache miss (would log the user out of every open dApp) — on miss reload from App Group, else stay silent. Emit `chainChanged` synchronously when `wallet_switchEthereumChain` resolves; dedupe. Reconcile `{chainId,address}` on **every** response, not only on focus.
- **Read allowlist:** audit `INSTANT_READONLY_METHODS` against viem's full read surface (fee + tx-build methods) — a missing one throws *before any sign*. Reads idempotent + timed + retried.
- **Security:** reject `eth_sign` outright; display top-frame + requesting-frame origin; flag cross-origin iframe signs; do **not** globally spoof `isMetaMask` (keep `isVela:true` + clean 6963).
- **Honest ceiling:** (1) strict-CSP flagships (Uniswap/Aave/dYdX) block `<script>` injection on iOS 15–17 — only real fix is iOS-18 `world:"MAIN"`; (2) WalletConnect-only-on-mobile dApps never touch `window.ethereum` — WalletPair/WalletConnect is the answer there, not injection.

### 12.5  Revised risk register + GO/NO-GO

| # | On-device validation (physical iPhones, iOS 17.4→18.6+, **not** Simulator) | Sev |
|---|---|---|
| **R1** | **Sign return channel:** does `submitted{userOpHash}` reliably reach the resolving inpage promise on return despite SW death? Matrix: app-killed-mid-flight, phone-locked-during-passkey, 45 s+ eviction, second-tab concurrent sign. | **S1 (highest)** |
| R2 | Can the **content script call `sendNativeMessage` directly**? Else does a `connectNative` port opened at load survive the round-trip? | S1 |
| R3 | **Gesture→launch** from the sheet anchor, pre-generated rid, zero awaits; first-run interstitial. | S1 |
| R4 | **App Group bidirectional I/O** from both processes: immutable-file + Darwin notif + cold-launch read; `NSFileProtection` while locked / before first unlock. | S1 |
| R5 | `sendNativeMessage` non-firing + timeout/retry/`connectNative` fallback; dead-worker detection surfaces recovery, never a hang. | S1 |
| R6 | CSP `<script>` block rate on Uniswap/Aave/dYdX; iOS-18 `world:"MAIN"` viability. | S2 |
| R7 | Provider two-slot refactor: WalletPair session survives a concurrent extension sign (or is cleanly serialized). | S2 |
| R8 | Inbound deep-link cold/warm start into `sign.tsx` with signing-ready queue. | S2 |

**Highest risk: R1** — where funds get lost/duplicated and where a verified fact refutes the v1 design.

**GO / NO-GO GATE for Phase 2 ship** (fund-safety, physical devices): after a forced eviction/kill/lock matrix of **≥100 signing round-trips** across iOS 17.4–18.6+: **(a)** every submitted tx appears in Vela Activity (zero silent loss); **(b)** no dApp ever receives a `4001`/decline-shaped error for a tx that submitted (zero false-decline); **(c)** every unresolved return is *visibly recoverable* (ring never hangs; Activity fallback always reachable); **(d)** no double-submission across the concurrent/reload matrix. If any of (a)–(d) fails, **Phase 2 does not ship — the sign path stays behind WalletPair.**

### 12.6  Revised phases & effort

v1's "≈4–6 weeks, mostly reuse" **under-budgeted** three net-new bodies of work: the App Group native module (FACT-2), the provider two-slot refactor, and inbound deep-link handling.

- **Phase 0 — Skeleton + native substrate (1–1.5 wk):** Safari target + App Group entitlement on **both** targets + esbuild; **build the shared App Group native module now** (immutable-file mailbox + Darwin notif + `NSFileProtection`); bidirectional echo under concurrency; **CSP block-rate canary** (Uniswap/Aave/dYdX).
- **Phase 1 — Connect + read, zero hop (1–1.5 wk):** as v1 + full defensive provider surface (§12.4).
- **Phase 2 — Sign via app hop (2.5–3.5 wk, was 1.5–2):** provider two-slot + `beginExtensionSign`; full `ExtensionBridgeTransport`; inbound deep-link + signing-ready queue; **Universal Links pulled forward**; native-handler/`connectNative` return path; pending-rid queue; TTL decoupling; non-4001 semantics; persist-at-submit + Activity reconciliation. Order: `personal_sign` → `eth_signTypedData_v4` → `eth_sendTransaction`/`wallet_sendCalls`. **Gated by R1 GO/NO-GO.**
- **Phase 3 — Polish (1 wk):** toolbar popup; iOS-18 `world:"MAIN"` (promoted to required for CSP-strict flagships if R6 is high); decode-preview refinements.

**Revised total ≈ 6–9 weeks.** The signing *pipeline* reuse is real and saves weeks; the *plumbing around it* is where v1 under-budgeted.

### 12.7  Cross-cutting principles (fold into §0)

1. **Vela Activity is the source of truth, not the dApp response** — persist-at-submit; the return channel is at-most-once with unavoidable loss.
2. **Ambiguity never looks like a decline** — one distinct pending code, never `4001` on timeout.
3. **Single rid authority (inpage), launched by a real tap on the hand-off sheet** — fixes dual-generation + gesture-loss together.
4. **The App Group is persistence, never a live transport** — immutable atomic files + Darwin notifications; never `UserDefaults` as IPC, never `expo-file-system`.
5. **The return poll bypasses the evictable JS background** — via the native handler / warm `connectNative` port, with dead-worker detection and an Activity fallback that never hangs.
