# `@vela-wallet/sdk`

EIP-1193 provider for connecting an HTTPS dApp to the Vela Web Wallet. It opens
`https://wallet.getvela.app/web-request` for account consent and every signing
request; no native app or browser extension is required.

> **Status (2026-09-11).** The wallet-side page this SDK opens lived in the
> React Native / Expo app, retired in spec 039. The SDK and its protocol are
> kept; the `/web-request` surface is owed to `app-web/vela-wallet`
> (spec 039 Part B, "Owed before the hostname moves") and, until it lands,
> is served by the frozen Expo build on that hostname.

## Usage

```ts
import { createVelaWalletSDK } from '@vela-wallet/sdk';

const vela = createVelaWalletSDK({
  appName: 'Example dApp',
  appUrl: window.location.origin,
  appLogoUrl: `${window.location.origin}/icon.png`,
});

const provider = vela.getProvider();
const [address] = await provider.request({ method: 'eth_requestAccounts' }) as string[];

const signature = await provider.request({
  method: 'personal_sign',
  params: ['0x48656c6c6f', address],
});
```

During local development, set `walletUrl` to the locally served wallet route:

```ts
createVelaWalletSDK({
  appName: 'Local dApp',
  walletUrl: 'http://localhost:8081/web-request',
});
```

Call popup-opening methods directly from a click/tap handler. Browsers may block
them after unrelated asynchronous work. A dApp that sends
`Cross-Origin-Opener-Policy` must use `same-origin-allow-popups` (or omit COOP),
because `same-origin` severs the secure opener channel.

## Supported wallet methods

- `eth_requestAccounts`, `eth_accounts`
- `eth_sendTransaction`
- `personal_sign`, `eth_sign`, `eth_signTypedData*`
- `wallet_sendCalls`, `wallet_getCallsStatus`
- `wallet_switchEthereumChain`
- read-only JSON-RPC methods supported by the Vela wallet RPC router

The returned account is a Safe/ERC-4337 smart account. Message signatures use
EIP-1271 and must not be verified with EOA-only `ecrecover` logic.
