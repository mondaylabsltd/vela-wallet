# Vela Wallet

A self-custodial smart wallet for EVM networks, built with React Native and Expo.

Vela Wallet uses ERC-4337 account abstraction with WebAuthn (passkey) authentication — no seed phrases, no private keys to manage.`npm run build:web`

Runs on **iOS**, **Android**, and **Web** from a single codebase.

## Features

- **Passkey authentication** — Sign transactions with Face ID, Touch ID, or fingerprint. No seed phrases or private key management.
- **Smart contract wallet** — Built on [Safe](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1) with ERC-4337 account abstraction. Your wallet is a Safe smart account.
- **12 EVM networks** — Ethereum, BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche, Gnosis, Unichain, Tempo, Monad, World Chain. Custom networks supported.
- **Multi-chain portfolio** — Balances and USD prices across all chains in one view. Native tokens, stablecoins, wrapped assets, and custom ERC-20s.
- **On-chain pricing** — DEX quotes (Uniswap V3, PancakeSwap, Aerodrome) with Chainlink oracle fallback. No third-party price API dependency.
- **Deposit detection** — Real-time balance monitoring with haptic notification when incoming transfers land.
- **DApp Connect** — Pair with the Vela browser extension over Bluetooth to sign transactions from your desktop.
- **Cross-device recovery** — Cloud-synced passkey backup via iCloud (iOS) or Google BlockStore (Android).
- **Fully self-hostable** — All three backend services (chain data, passkey index, bundler) are open source and can be self-deployed.

## Architecture

```
┌─────────────────────────────────────────────┐
│  React Native + Expo Router                 │
│  (iOS / Android / Web)                      │
├─────────────────────────────────────────────┤
│  Native Modules                             │
│  ┌──────────┐ ┌──────────┐ ┌─────────────┐ │
│  │ Passkey  │ │ CloudSync│ │ BLE Connect │ │
│  │ WebAuthn │ │ iCloud / │ │ DApp Pairing│ │
│  │ P-256    │ │ BlockStore│ │             │ │
│  └──────────┘ └──────────┘ └─────────────┘ │
├─────────────────────────────────────────────┤
│  Services                                   │
│  ┌──────────────────┐ ┌──────────────────┐  │
│  │ RPC Pool         │ │ Safe Transaction │  │
│  │ Multi-source     │ │ ERC-4337 UserOp  │  │
│  │ Auto-failover    │ │ WebAuthn signing │  │
│  │ Latency scoring  │ │ Bundler submit   │  │
│  └──────────────────┘ └──────────────────┘  │
│  ┌──────────────────┐ ┌──────────────────┐  │
│  │ Wallet API       │ │ Price Service    │  │
│  │ Multicall3 batch │ │ DEX quotes       │  │
│  │ Progressive load │ │ Chainlink oracle │  │
│  └──────────────────┘ └──────────────────┘  │
├─────────────────────────────────────────────┤
│  EVM Networks (12 chains)                   │
│  ETH · BNB · POL · ARB · OP · BASE          │
│  AVAX · GNO · UNI · TEMPO · MON · WLD       │
└─────────────────────────────────────────────┘
```

## Get Started

1. Install dependencies

   ```bash
   npm install
   ```
2. Start the app

   ```bash
   # iOS / Android
   npx expo start

   # Web
   npx expo start --web
   ```

## Platform Support


| Feature            | iOS                      | Android                     | Web                         |
| -------------------- | -------------------------- | ----------------------------- | ----------------------------- |
| Passkey (WebAuthn) | Native (ASAuthorization) | Native (Credential Manager) | `navigator.credentials` API |
| Cloud Sync         | iCloud Key-Value Store   | Google Play BlockStore      | IndexedDB (local only)      |
| QR Scanner         | expo-camera              | expo-camera                 | `getUserMedia` + jsQR       |
| Haptic Feedback    | expo-haptics             | expo-haptics                | No-op                       |
| Clipboard          | expo-clipboard           | expo-clipboard              | `navigator.clipboard`       |
| In-App Browser     | expo-web-browser         | expo-web-browser            | `window.open`               |
| BLE (DApp Connect) | VelaBLE native module    | VelaBLE native module       | Not supported (v1)          |
| Animated Balance   | Reanimated worklet       | Reanimated worklet          | Plain text (no animation)   |

### Web Notes

- **Passkey rpId**: Uses the registrable domain (e.g. `getvela.app`) so passkeys work across subdomains and are consistent with native.
- **Cloud Sync**: Web uses IndexedDB for local persistence. No cross-device sync — accounts are stored in the browser only.
- **DApp Connect**: BLE connection is not available on web. This is planned for a future release.
- **Native APIs**: All platform-specific APIs (Alert, Clipboard, Haptics, AppState, Linking) are abstracted via `src/services/platform.ts`.

## Build for Web (Cloudflare Pages)

1. Build the static web bundle

   ```bash
   npm run build:web
   ```

   Output goes to `dist/`.
2. Deploy with Wrangler CLI

   ```bash
   npx wrangler pages deploy dist --project-name vela-wallet
   ```

   Or connect your GitHub repo in the [Cloudflare Dashboard](https://dash.cloudflare.com) → Pages → Create a project:


   | Setting              | Value                 |
   | ---------------------- | ----------------------- |
   | Build command        | `npm run build:web`   |
   | Output directory     | `dist`                |
   | Environment variable | `NODE_VERSION` = `20` |

## Self-Deploy Service Endpoints

Vela Wallet relies on four backend endpoints. Default instances are provided, but you can deploy your own for full self-custody.

Configure custom endpoints in **Settings > Advanced > Service Endpoints**.


| Service                  | Description                                       | Repository                                                                                                                      |
| -------------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| **Chain Data Index**     | Network info, token data, chain logos             | [atshelchin/ethereum-data](https://github.com/atshelchin/ethereum-data)                                                         |
| **Passkey Index**        | Public key storage for cross-device recovery      | [atshelchin/webauthnp256-publickey-index.biubiu.tools](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools) |
| **Bundler Service**      | ERC-4337 transaction bundler                      | [atshelchin/vela-bundler](https://github.com/atshelchin/vela-bundler)                                                           |
| **Exchange-Rate Source** | USD-based fiat rates that drive the currency list | [Frankfurter](https://frankfurter.dev) (FOSS, self-hostable via Docker)                                                         |

The first three are Vela services that each expose a `/api/health` endpoint. The wallet validates three checks before accepting a custom endpoint for them:

1. **HTTPS** — only secure connections accepted
2. **Reachable** — server responds within 10 seconds
3. **Valid response** — `/api/health` returns the correct `service` identifier and `status: "ok"`

The **Exchange-Rate Source** is any USD-based FX API — the default is Frankfurter's public instance, `https://api.frankfurter.dev/v2/rates?base=USD`. It's validated by returning a parseable USD-based rate set (not `/api/health`). Frankfurter is open source and self-hostable with Docker — see [frankfurter.dev](https://frankfurter.dev). Pin the base to USD (`?base=USD`), or every conversion is silently wrong. For the response shapes Vela accepts, the Chainlink fallback, and a porting guide, see [docs/fiat-price.md](docs/fiat-price.md).

## Gas & Fee Model

Vela Wallet uses ERC-4337 account abstraction, so transactions are relayed by a **bundler** instead of being submitted directly by the user. This means:

### How Gas Fees Work

Each transaction incurs a gas fee that is deducted from **your Safe wallet's native token balance** (ETH, BNB, etc.). The fee consists of:

- **On-chain gas cost** — The actual cost to execute the transaction on the blockchain.
- **Relayer service fee** — A ~60% markup over the on-chain gas price (`maxFeePerGas = gasPrice × 1.6`). This covers the bundler's operating costs.

The total estimated fee is shown on the confirmation screen with a full breakdown: on-chain gas price, UserOp gas price, gas limit, fee in native tokens, and fee in USD.

### Gas Relayer Account

Before your first transaction on a network, you need to fund a **dedicated gas relayer account** (bundler EOA). This is a one-time setup:

- The deposit amount is based on the actual transaction gas requirement.
- The deposit is **non-refundable** — it serves as the relayer's initial operating balance.
- The relayer address **may change** due to service upgrades, requiring a new deposit.
- After the initial deposit, the relayer is self-sustaining: it earns back gas costs from each transaction via EntryPoint refunds.

### Max Send

When sending the maximum amount of a native token (ETH, BNB, etc.), the wallet automatically reserves enough for the transaction's gas fee (EntryPoint prefund). This prevents "insufficient balance" failures.

## WebAuthn Proxy Extension (Domain Recovery / Dev Passkeys)

If the production domain (`getvela.app`) becomes unavailable, passkeys bound to it will stop working on the new hosting domain because WebAuthn ties credentials to the rpId (relying party ID). The included Chrome extension solves this by proxying WebAuthn calls through the extension's own origin, which has `host_permissions` for `getvela.app`.

This also enables local development and preview deployments to authenticate with production passkeys.

### How rpId is resolved


| Environment                                           | Without extension | With extension |
| ------------------------------------------------------- | ------------------- | ---------------- |
| `getvela.app` / `*.getvela.app`                       | `getvela.app`     | `getvela.app`  |
| `localhost` / `127.0.0.1`                             | `localhost`       | `getvela.app`  |
| Preview domains (`*.pages.dev`, `*.vercel.app`, etc.) | current hostname  | `getvela.app`  |

Without the extension, each environment uses its own rpId and maintains independent passkeys. With the extension installed, all environments share the `getvela.app` rpId and the same set of passkeys.

### Supported preview domains

`pages.dev`, `workers.dev`, `github.io`, `vercel.app`, `netlify.app`, `deno.dev`, `fly.dev`, `railway.app`, `render.com`, `surge.sh`, `ngrok-free.app`, `trycloudflare.com`

### Setup

1. Open `chrome://extensions/` and enable **Developer mode**.
2. Click **Load unpacked** and select the `chrome-ext-webauthn-proxy/` directory.
3. Grant the requested permissions when prompted.
4. Navigate to your dev/preview URL — the extension activates automatically.

When a page calls `navigator.credentials.create()` or `.get()` with a non-matching rpId, the extension intercepts the call, opens a small popup window, and performs the WebAuthn ceremony with `rpId: "getvela.app"`. The system authenticator prompt (Touch ID / Windows Hello) appears as usual, and the result is passed back to the page.

### How it works

```
Page JS (any domain)
  │  navigator.credentials.create/get intercepted
  ▼
inject.js (MAIN world, document_start)
  │  serialize options, window.postMessage
  ▼
bridge.js (ISOLATED world, has chrome.runtime API)
  │  chrome.runtime.sendMessage
  ▼
background.js (service worker)
  │  chrome.windows.create → opens popup
  ▼
webauthn.html/js (extension origin, has host_permissions)
  │  navigator.credentials.create/get({ rpId: "getvela.app" })
  │  → System authenticator prompt (Touch ID / Windows Hello)
  ▼
Result flows back: webauthn.js → background → bridge → inject → page
```

### Important notes

- The `clientDataJSON.origin` in the WebAuthn response will be `chrome-extension://<id>`, not the page origin. Your relying party server must accept this origin when validating credentials created through the extension.
- The extension sets `window.__VELA_WEBAUTHN_PROXY_RPID__` in the page context. The app's `getRelyingPartyId()` reads this global to ensure public key uploads and server queries use the same rpId as the WebAuthn call.
- This extension is for development and disaster recovery only. Do not publish it to the Chrome Web Store.

## Recipient Identity Resolution

When sending tokens, the wallet resolves recipient addresses to human-readable names for verification. Resolution queries run in parallel across multiple name services, returning the first match by priority:


| Priority | Service       | Chain            | Registry            | Pattern          |
| ---------- | --------------- | ------------------ | --------------------- | ------------------ |
| 1        | Passkey Index | —               | Vela API            | walletRef lookup |
| 2        | .bnb          | BSC (56)         | `0x08CEd32a...`     | Standard ENS     |
| 3        | .arb          | Arbitrum (42161) | `0x4a067EE5...`     | Standard ENS     |
| 4        | .g            | Gravity (1625)   | `0x5dC881dd...`     | Standard ENS     |
| 5        | Basename      | Base (8453)      | `0xb9470442...`     | ENSIP-19         |
| 6        | ENS           | Mainnet (1)      | `0x00000000000C...` | Standard ENS     |

- **Standard ENS**: `namehash(addr.addr.reverse)` → `registry.resolver(node)` → `resolver.name(node)`
- **ENSIP-19** (Basenames): `reverseRegistrar.node(addr)` → chain-specific reverse node → same flow
- Only positive results are cached (AsyncStorage, 24h TTL)
- No third-party API dependencies — all queries use direct on-chain RPC calls

To add a new name service, add an entry to `NAME_SERVICES` in `src/services/recipient-identity.ts`.

## Security Model

- **No private key access** — Signing uses WebAuthn P-256 keys managed by your OS (iCloud Keychain / Google Password Manager). Vela Wallet never has access to the private key.
- **Safe smart account** — Your wallet is a Safe proxy contract, audited and battle-tested with billions in TVL.
- **On-device only** — Transaction construction, signing, and signature verification all happen locally. The bundler only receives the signed UserOperation.
- **Passkey-scoped** — Each wallet is bound to a passkey credential. Transactions require biometric verification (Face ID / fingerprint) every time.

## License

MIT
