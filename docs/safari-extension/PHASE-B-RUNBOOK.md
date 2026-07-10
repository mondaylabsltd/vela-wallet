# Phase B runbook — the real sign behind the transport

**Status:** signature methods **device-verified on the real path** (2026-07-06). The
Phase-A fake-sign is gone for `personal_sign` and `eth_signTypedData_v4`: both now
run the whole production pipeline (clear-signing UI → `Passkey.sign` → EIP-1271
contract signature) and resolve the dApp promise with a **real** signature — headless
(no Face ID), via the parallel-space fixed-key signer. `eth_sendTransaction` /
`wallet_sendCalls` are code-complete on the same method-agnostic path but not yet
device-verified (need a funded fixture Safe + a live bundler on the granted chain).

## What shipped (Phase B)

| File | Role |
|---|---|
| `src/services/extension-bridge-transport.ts` | The third `DAppTransport`. One-shot per `rid`: `connect()` reads `sign-req-<rid>.json` + emits the request (renders the global sheet); `sendResponse` writes the FROZEN `sign-result-<rid>.json`. Fund-safety contract: only `submitted`\|`rejected` reach disk; non-4001 errors write NOTHING (page → recoverable 4900). 7 unit tests. |
| `src/models/dapp-connection.tsx` | Two-slot provider. `signTransportRef` (transient) next to `transportRef` (durable); `beginExtensionSign()` installs the ext transport WITHOUT `disconnectCurrent()` (a live WalletPair session survives). Every response routes to the request's OWNING transport (per-request `__transport`, never a shared ref — F2), with per-request chain (`__chainId` — F4) and identity (`__dapp` — F3). |
| `src/app/sign.tsx` | Headless controller for `velawallet://sign?rid`. Waits signing-ready → builds the transport → `beginExtensionSign` → `connect()`; observes `disconnected` → "return to Safari". Never signs itself. |
| `src/components/SigningRequestModal.tsx` | 2-line change: sign + display against `incomingRequest.__chainId ?? chainId` and `__dapp ?? dappInfo` (F3/F4). |
| `src/models/types.ts` | `__transport` / `__chainId` / `__dapp` on the incoming-request shape. |

The §4 record-ordering fix is folded into `approveRequest`: the durable, app-owned
history record is persisted **before** `sendResponse` for every method, so the
extension's result file never lands before Vela Activity has the record.

## Device verification (`e2e/safari/check_real_sign.py`)

Runs headless in the **parallel space** (fixed-key P-256 signer → no Face ID). Now
parametrized by method:

```bash
# serve the test dApp on the LAN (extension injects on All Websites; a fresh origin
# just re-shows the in-Safari connect sheet, which the harness clicks through)
cd packages/safari-extension/testdapp && python3 -m http.server 8791 --bind 0.0.0.0

# arm the parallel space on the device first (velawallet://parallel — needs dev_unlocked=1)
cd e2e/safari
export VELA_TEST_URL="http://<mac-lan-ip>:8791/index.html"
VELA_SIGN_METHOD=personal_sign        ./venv/bin/python check_real_sign.py   # 4/4 PASS
VELA_SIGN_METHOD=eth_signTypedData_v4 ./venv/bin/python check_real_sign.py   # 4/4 PASS
```

The 4 steps: provider injected → connect (fixture Safe) → real SigningSheet approve
control found + tapped → dApp promise resolves with a REAL EIP-1271 signature
(`0x…`, NOT the `0xFA` fake, NOT a `4001` false-decline).

**Verified 2026-07-06, ABC iPhone 11 (DEV build off the running Metro):**
- `personal_sign` → 4/4 (earlier session; `signsheet.png`).
- `eth_signTypedData_v4` → **4/4**. Signature decoded = Safe contract-signature
  (owner `0x94a4f6af…` + dynamic WebAuthn assertion whose `clientDataJSON` tail is
  `{"origin":"https://getvela.app","crossOrigin":false}`) — proof it's the real
  passkey/EIP-1271 path, not a stub. Clear-signing sheet (`signsheet-eth_signTypedData_v4.png`)
  shows the decoded EIP-712 fields + the correct ERC-7730 "unknown descriptor" caution.

**Harness note:** the Safari→app scheme-launch banner is flaky under Appium (WebKit
suppresses it for synthetic gestures), so the launch is driven by `mobile: deepLink`
(`velawallet://sign?rid`) — a Phase-A concern already at 5/5, swapped only so the
Phase-B sign path runs unattended. The fixture Safe used is `Parallel One`
(`0xD400866e00B055B20752a826CD5C89b811de130b`).

## Remaining (Phase B)

1. **`eth_sendTransaction`** — ✅ VALIDATED end-to-end on Gnosis (2026-07-06). Extension
   send: approve → fixed-key sign → build UserOp (Safe deploy + xDAI transfer) → bundler
   `eth_sendUserOperation` OK → mined. On-chain: Parallel One deployed, Parallel Two +0.002
   xDAI over two 0.001 sends. Two blockers hit (both OUTSIDE the wallet send logic): the
   per-Safe **gas-account EOA** (`0xb32a3965…154d03`) was empty → `checkBundlerFunding`
   aborted the send, silently (masked by the invisible-funding-modal bug — `docs/KNOWN-BUGS.md`
   BUG-1); and a bundler-side Gnosis simulation "can't route" error (fixed bundler-side).
   Diagnosed by restarting Metro with stdout capture (`npx expo start > metro.log`) and reading
   the app's `[BundlerFunding]/[UserOp]/[Bundler]` JS logs — JS console.log goes to Metro, NOT
   device syslog. **`wall­et_sendCalls`** (batch) is the same `handleSendCalls` path, not yet
   separately device-tested. Harness caveat: with UL attestation set, the synthetic sheet-CTA
   click emits the real UL → navigates the dApp tab away → the tx-hash callback never returns,
   so check_real_sign reports 3/4 even though the send mined — the CHAIN (balances) is the verdict.
2. **Fund-safety matrix on the real path** — re-run `run_matrix.py`'s rows against the
   real sign (kill / evict / reject / concurrent) instead of the R1 fake. Rows 1/8
   (happy/reject) + concurrent are runnable now on the signature methods (no funds);
   kill/evict during a send need #1.
3. **Concurrent-session proof** — the two-slot raison d'être: a live WalletPair session +
   an extension sign at once, neither answering the other. ✅ **PROVEN (headless, 2026-07-06)**
   at the fund-safety-critical level — the response ROUTING isolation (F2/F3/F4). The
   provider now routes every per-request decision through one seam
   (`src/models/dapp-request-routing.ts`: `responseTransport` / `requestChainId` /
   `requestDApp`), and `src/__tests__/concurrent-session.test.ts` drives the REAL
   `ExtensionBridgeTransport` against a live-WalletPair stand-in to prove: an extension
   signature is answered on the extension's own result file and NEVER over the WalletPair
   socket (no leak — F2); a WalletPair reply never lands in the extension's file or settles
   its rid (no mis-settle — F2); each carries its own chain (F4) + dApp identity (F3); and
   the extension transport settles only on its own response. 6/6 green; jest 1075 total.
   The pure-node jest env can't render `DAppConnectionProvider`, so this proves the routing
   logic (the actual F2 fund-risk) deterministically; the remaining "beginExtensionSign never
   disconnects a live WalletPair" invariant is inspection- + adversarial-review-verified. A
   FULLY-LIVE on-device run (real WalletPair WebSocket + real App-Group extension sign at
   once) would need a new WalletPair-pairing Appium harness — a follow-up, not blocking.
