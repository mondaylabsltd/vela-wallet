# HTTPS Web Wallet integration

Vela can act as an external wallet for an HTTPS dApp without a native app or
browser extension. The dApp integrates `@vela-wallet/sdk`; account consent and
signing happen at `wallet.getvela.app` in a popup.

## Flow

```text
dApp (https://merchant.example)
  │ provider.request({ method, params })
  ▼
@vela-wallet/sdk opens wallet.getvela.app/web-request?session=<random>
  │ VELA_WEB_READY (window.postMessage)
  │ VELA_WEB_INIT + transferred MessagePort
  ▼
Vela verifies event.source + event.origin + session
  │ connect: per-origin account consent
  │ sign: existing clear-signing → passkey → Safe/ERC-4337 pipeline
  ▼
VELA_WEB_RESPONSE on the capability-bound MessagePort
  │
  ▼
dApp request promise resolves/rejects
```

## Security invariants

- The browser-provided `MessageEvent.origin` is the dApp identity. `appUrl` and
  other SDK metadata are presentation-only and never authorize a request.
- Production dApps must use HTTPS. `http://localhost` loopback origins are
  allowed only for development.
- A random popup session and RPC request ID bind every response. The wallet
  accepts one initializer and `WebPopupTransport` settles exactly once.
- The transferred `MessagePort` is the response capability. Request payloads and
  signatures are not placed in URLs.
- Grants are stored per exact origin. A signing request must match the granted
  account, and the wallet reconciles to that account before presenting approval.
- Every value-moving or signing operation uses the existing Vela confirmation,
  simulation, passkey, history and anti-double-submit path.

## Browser requirements and current boundary

Popup methods must be called from a user gesture. The dApp must not use
`Cross-Origin-Opener-Policy: same-origin`; use `same-origin-allow-popups` when a
COOP header is required.

This first transport targets browser popup/new-tab flows where `window.opener`
and transferable `MessagePort` are available. A future redirect/relay transport
is still needed for browsers that deliberately discard opener state and for
cross-device approval.

## Source map

- dApp SDK: `packages/vela-sdk/src/index.ts`
- shared protocol: `packages/vela-sdk/src/protocol.ts`
- wallet request route: `src/app/web-request.tsx`
- one-shot transport: `src/services/web-popup-transport.ts`
- existing signing pipeline: `src/models/dapp-connection.tsx`
