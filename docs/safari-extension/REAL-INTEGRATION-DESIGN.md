# Real Safari-extension sign path — integration design (for review, pre-implementation)

**Status:** FUND-CRITICAL. Design reviewed 2026-07-06; verdict **FIX-FIRST** (see §Critic). Do not implement until F1–F4 are resolved and the founder approves the re-scoped plan (§Plan).

**Prereq context:** the R1 spike (Increments 1–4) is **conditional GO** (see `R1-INCREMENT-4-RUNBOOK.md`) — the sign RETURN CHANNEL (app writes `sign-result-<rid>.json` → Swift handler → `content.js` focus-poll → page) is proven on device, incl. survival of app-kill + background-worker eviction. Automation harness: `e2e/safari/`.

---

## ⚠️ The re-scope finding (Critic F1 — read this first)

**The spike proved the return CHANNEL, not the EIP-1193 provider.** Today `targets/safari/assets/inpage.js` is 8 lines (`window.__velaR1={injected:true}`) — **no `window.ethereum`, no provider, no request/response bridge.** The R1 "sign" is triggered by an injected **stub button** with a hardcoded `personal_sign` payload (`content.js` `onSignTap`), and the outcome is a **status pill** read from `browser.storage.local` — NOT a JSON-RPC result returned to a dApp. The frozen result schema `{rid,status,userOpHash,ts}` has no signature field and nothing resolves a dApp promise.

**Therefore "real integration" = TWO halves:**
- **Phase A — the real EIP-1193/6963 provider** (inpage `window.ethereum` + content/background request↔response correlation). This is the dApp-facing surface that replaces the stub button + pill. Standard extension work, intentionally deferred by the spike to de-risk the return channel first.
- **Phase B — the real sign behind the transport** (this design, below, with F2/F3/F4 fixed). A and B interlock: provider (A) makes the real request → transport → real sign (B) → result → transport → content → provider resolves the dApp's promise.

The design below is **solid for Phase B** but assumed Phase A was "frozen/done" — it isn't built.

---

## The elegant core (verified correct)

The global `<SigningRequestModal>` (`_layout.tsx:112`, sibling of `<Stack>`) is **already headless**: it renders whenever `incomingRequest !== null` and reads `activeAccount` itself (`SigningRequestModal.tsx:668-675`). So the ENTIRE signing UI + clear-signing + `enforceNoUnlimited` (`use-dapp-signing.ts:322`) + asset-sim (`SigningRequestModal.tsx:252/265/309`) + gas card + `BundlerFundingModal` (`:701`) appear **for free** the moment `incomingRequest` is set. `incomingRequest` is set by exactly one path: `handleIncoming` firing on a transport's `'request'` event (`dapp-connection.tsx:350→270`).

→ Integration = give the extension its own `DAppTransport` whose `connect()` emits `'request'` (renders the real sheet) and whose `sendResponse()` writes the frozen result file. Everything downstream is unchanged.

Two structural blockers (both real, verified):
1. **Clobber** — every connect entry (`connectToBridge:372`, `connectToWalletPair:397`, `disconnectBridge:465`) calls `disconnectCurrent()` (`:360`) which `.disconnect()`s the live transport → would kill a live WalletPair session. Solved by a **second slot + `beginExtensionSign` that never calls `disconnectCurrent`**.
2. **Mis-routing** — responses hard-code `transportRef.current?.sendResponse(...)` (`:549/:636/:646/:675/:260/:281/:287/:295/:305-309`). With two transports live, an extension response would go over the WalletPair socket. Solved by routing responses to **the transport that owns the request** (see F2 — must be per-request binding, NOT a shared ref).

---

## §1 `ExtensionBridgeTransport` (new: `src/services/extension-bridge-transport.ts`)

Implements the real `DAppTransport` (`dapp-transport.ts:27-54`). One-shot per rid: one request in, one response out, then settles.

- `constructor(rid)`; `name='Safari Extension'`.
- `connect()`: polls `sign-req-<rid>.json` (150ms/3s, same as `sign.tsx:38`) → parse → `_connected=true` → emit `'connected'` then `'request'(rid,method,params,origin)`.
- `sendResponse(id,result,error)`: **sync void** (interface) → fire-and-forget async `writeResult` (mirrors `RemoteInjectTransport` `dapp-transport.ts:159-168`), `_settled` guard for idempotency, then `_connected=false` + emit `'disconnected'` **only after the write completes**.
- `writeResult`: **frozen contract — only `submitted`|`rejected` reach disk.** `error.code===4001` → `{status:'rejected',userOpHash:'0x'}`; any **non-4001** error → **write NOTHING** (page falls to CHECK_VELA/4900, recoverable — preserves invariant b); success → `{status:'submitted',userOpHash:<string>}`.
- `pushWalletInfo`: no-op (no live channel). `fetchDAppInfo`: returns `{name:hostname(origin),url:origin}` (but see F3 — must actually be wired into per-request identity). `disconnect()`: `_connected=false`, no emit. `on/emit`: listener map. `reconnect?` omitted (one-shot).
- **Stays-connected-until-sendResponse rule:** `connected` true from `connect()` through the file write; early `'disconnected'` would clear `incomingRequest` (`dapp-connection.tsx:333`) mid-sign. Prevented two ways: (a) never emit before write; (b) `beginExtensionSign` does NOT subscribe the sign transport to `wireTransport`'s `disconnected` handler.

## §2 Two-slot `DAppConnectionProvider` change (`dapp-connection.tsx`)

- **A.** Add `signTransportRef` (transient) next to `transportRef` (durable). **[F2: do NOT add a single shared `requestTransportRef` — bind the owning transport per-request instead: capture in the `on('request')` closure and carry it on the `incomingRequest` object, e.g. `setIncomingRequest({...req,__transport})`.]**
- **B.** Stamp the originating transport on every inbound request (wrap `handleIncoming` in both `wireTransport:350` and `beginExtensionSign`).
- **C.** Route every response call-site (`:549/:636/:646/:675/:260/:281/:287/:295/:305-309`) to the **request's** transport (per-request, per F2), NOT `transportRef`. Lifecycle call-sites (`pushWalletInfo/switchChain/reconnect` `:238/:321/:686/:483/:742`) stay on `transportRef`.
- **D.** New public `beginExtensionSign(transport)`: registers `request` + `error` + an identity-guarded slot-null on `disconnected` that **never clears `incomingRequest`**; sets `signTransportRef.current=transport`; **no `disconnectCurrent()`, no `transportRef` write.** Expose via context interface (`:141`), default (`:169`), memo value+deps (`:768/:776`).
- **E.** Identity-guard replicates `confirmFingerprint:433` (`x.current === transport`).
- **F.** `disconnectCurrent:360` untouched; `beginExtensionSign` must never be reachable from a `disconnectCurrent` path.
- **[F3 fix]** Per-request dApp identity (name/url/origin) so the sheet, history `recordOrigin`, and SIWE guard (`:416→:1160`) use the extension origin, NOT a concurrent WP session's `dappInfo`.

## §3 `sign.tsx` rewrite — headless controller

Stops signing. Becomes: deep-link → **wait signing-ready** (`ready = !state.isLoading && state.hasWallet && !!activeAccount`) → `new ExtensionBridgeTransport(rid)` → `beginExtensionSign(t)` → `t.connect()` (sets `incomingRequest` → global sheet renders) → observe `t.on('disconnected')` → flip UI to "Return to Safari" (`Linking.openURL(origin)`). Delete `submitFakeSign`/`fakeUserOpHash`/`Sign(fake)`/`dev:reject` buttons; `readSignRequest` moves into the transport's `connect()`. Persist-at-submit preserved via the real `approveRequest`→`onSubmitted`→`saveTransaction` (`dapp-connection.tsx:538-547`).

**Method order:** (1) `personal_sign` — `handlePersonalSign:170` → EIP-191 → `computeSafeMessageHash:962` → `Passkey.sign` → `buildContractSignature` → EIP-1271 sig hex (no bundler); (2) `eth_signTypedData_v4`; (3) `eth_sendTransaction`/`wallet_sendCalls` — full `sendUserOp` (`safe-transaction.ts:459`) + funding pre-check + `onSubmitted`→`waitForReceipt`.

## §4 Preserve the proven R1 return path

**Zero changes** to `SafariWebExtensionHandler.swift` (`pollSignResult:41-63`) or `content.js` (poll `status`+`userOpHash`). Only difference on disk = a **real** hash/sig in `userOpHash`. Status→file mapping: success→`submitted`; reject(4001)→`rejected`; passkey-cancel/funding-cancel(-32603)/post-submit-fail→**no file**→4900 (recoverable). **Invariant-(a) reorder to flag:** for signatures the record saves at `:564-574` **after** `sendResponse:549` — move `saveTransaction` **above** `sendResponse` so durable-record precedes the result file for every method (~3 lines, low fund-impact but makes (a) literal).

---

## Critic findings (FIX-FIRST)

- **F1 (BLOCKER, scope):** spike proves the status-pill return channel, NOT a real dApp. No `window.ethereum` provider exists → real dApps get no result. Real integration REQUIRES building the provider (inpage/content) — the "frozen" files. → **Phase A.**
- **F2 (BLOCKER):** single shared `requestTransportRef` misroutes under concurrency — a WP read resolving after an extension request → WP result written to `sign-result-<wpId>.json` + trips `_settled` (swallows the real sign result); or the **real signature delivered over the WP socket to the wrong origin (leak)**. → bind owning transport **per-request**, never a shared ref. (Also §2C missed `:287`.)
- **F3 (BLOCKER, concurrent):** single `dappInfo` → extension sheet shows the WP dApp's identity, history mis-attributed, SIWE guard compares wrong origin. → per-request dApp identity.
- **F4:** cold-launch `personal_sign`/typed-data with no embedded chainId sign against default chain `1` → invalid on a Safe on chain X≠1. `SignRequest` has no chainId. → decide chain source before shipping method #1.
- **F5 (minor):** `sign.tsx` is itself a `presentation:'modal'` screen and the global `SigningRequestModal` renders on top → new iOS z-order/focus/dismiss scenario to verify. The §4 reorder is correctly identified.
- **Verified correct:** two-slot clobber analysis; `beginExtensionSign` avoiding `disconnectCurrent` + not subscribing to `wireTransport`'s `disconnected`; 4001-only mapping; persist-at-submit ordering for `eth_sendTransaction`; `AppGroup.writeFile` off-iOS throw handled by `void…finally`; interface conformance.

---

## Re-scoped plan (approved direction 2026-07-06: docs → Phase A → Phase B)

**Phase A — real EIP-1193/6963 provider** (makes dApps actually work; replaces stub+pill):
- `inpage.js`: real `window.ethereum` (EIP-1193 `request/on/removeListener`) + EIP-6963 announce (`rdns:app.getvela`), per-`rid`/`rpcId` request↔response correlation via `window.postMessage` ↔ content script.
- `content.js`: relay page↔background; deliver the sign result back to the inpage provider to **resolve the dApp's promise** (replacing the status pill). Read/state methods answered locally; sign methods routed to the app (existing proven launch+return).
- Keep the proven launch (`velawallet://sign?rid`) + return (poll `sign-result-<rid>`) plumbing — just drive it from the real provider instead of the stub button, and surface the result to the page provider.
- Re-validate with `e2e/safari` against a real test dApp (wagmi/viem) that calls `window.ethereum.request`.

**Phase B — real sign behind the transport** (this design, F2/F3/F4 fixed):
1. `ExtensionBridgeTransport` + unit-test the `writeResult` mapping.
2. Provider two-slot (§2 with **per-request** transport + identity binding — F2/F3). Verify existing WalletPair connect/sign/reject regress clean.
3. `sign.tsx` → `personal_sign` only; decide chainId source (F4); harness rows 1+8.
4. §4 signature reorder; harness row 3 (kill).
5. `eth_signTypedData_v4`; rows 1/3/5.
6. `eth_sendTransaction`/`wallet_sendCalls` (bundler+funding); full matrix.
7. Concurrent-session proof: live WP session + extension sign, neither answers the other (the two-slot raison d'être).

**Harness prereq for unattended runs:** a fixed-key/parallel-space P-256 signer so `Passkey.sign` resolves without Face ID; retarget `run_matrix.py` labels from `Sign (fake)`/`dev: reject` to the real SigningSheet Approve/Reject.

**Files:** new `src/services/extension-bridge-transport.ts`; edit `dapp-connection.tsx` (slots + `beginExtensionSign` + per-request routing/identity + context); rewrite `sign.tsx`; ~3-line `approveRequest` reorder; Phase A rewrites `inpage.js`/`content.js`. Frozen: `SafariWebExtensionHandler.swift`, `SigningRequestModal.tsx`, `use-dapp-signing.ts`, `safe-transaction.ts`, `app-group/index.ts`.
