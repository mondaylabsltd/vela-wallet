# Vela Wallet

[![CI](https://github.com/mondaylabsltd/vela-wallet/actions/workflows/ci.yml/badge.svg)](https://github.com/mondaylabsltd/vela-wallet/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**An Ethereum wallet you actually own.** Every piece of it — the apps and the
services behind them — is MIT-licensed, builds from public repositories,
and runs on your own machines.

- Your account is an **unmodified Safe v1.4.1** on **ERC-4337** (EntryPoint
  v0.7), signed by **passkeys or FIDO2 security keys** and verified on-chain as
  P-256 by the precompile at `0x100` (EIP-7951; RIP-7212 on rollups). No seed phrase, no Vela contract in the funds path, no admin
  role for us.
- **One Rust core, four native apps**: SwiftUI on iOS, Jetpack Compose on
  Android, SvelteKit on the web (and the browser extension), gpui on desktop.
  The same `vela-core` makes every decision on all four.
- **Self-hostable end to end**: the relay, the public-key index, chain data and
  exchange rates are separate repositories, and the apps can be pointed at yours.

[Website](https://getvela.app) · [Docs](https://getvela.app/docs) ·
[Self-hosting guide](https://getvela.app/docs/self-hosting) ·
[Whitepaper](https://getvela.app/docs/whitepaper) ·
[Architecture](docs/ARCHITECTURE.md) ·
[Releases](https://github.com/mondaylabsltd/vela-wallet/releases)

## What's worth reading here

- **The core makes the decisions.** Every rule that guards money — send, fees,
  signing requests, RPC failover, contacts — is a [Crux](https://github.com/redbadger/crux)
  state machine in [`rust/crates/vela-core`](rust/README.md). The apps render
  what the core returns and run the effects it asks for, so the rules are tested
  without a browser, a device or a network.
- **No hand-rolled crypto.** Hashing, curve maths, ABI and CBOR come from
  alloy-core, sha2, p256/ecdsa and ciborium/coset at pinned versions. A frozen
  conformance corpus is replayed through Rust, the Kotlin and Swift bindings and
  the shipped wasm.
- **CTAP in the core.** `vela-core` talks to a phone over hybrid (QR) transport
  and to USB security keys directly, so an app you compile yourself can still
  sign for a wallet created at getvela.app.
- **Fees are signed, not trusted.** The relay is paid in band: a transfer inside
  the MultiSend your key signs. You see the amount before signing; the relay
  can delay or refuse, not change it. No paymaster.
  [How the fee is set](https://getvela.app/docs/networks-and-fees#fee).
- **Any EVM chain with the P-256 precompile** (EIP-7951 / RIP-7212). [Chain setup](https://getvela.app/chain-setup)
  deploys the missing Safe and ERC-4337 contracts that anyone can deploy.
- **Clear signing** from ERC-7730 descriptors, with an explicit blind-signing
  warning for anything that can't be decoded.
- **A dApp cannot take over the account.** A request for a call from your Safe
  to itself — `addOwnerWithThreshold`, `enableModule`, `setGuard` and the rest
  of that family — is refused, inside a batch or a `MultiSend` as well, as is
  any leg carrying a `delegatecall` and a `SafeTx` typed-data signature. The
  rule is in the core (`self_call_guard.rs`), so every client has it.

## Architecture

```
┌────────────────┬────────────────┬────────────────┬────────────────┐
│  iOS           │  Android       │  Web           │  Desktop       │
│  SwiftUI       │  Compose       │  SvelteKit     │  gpui (Rust)   │
│  app-ios/      │  app-android/  │  app-web/      │  app-desktop/  │
└───────┬────────┴───────┬────────┴───────┬────────┴───────┬────────┘
        │ UniFFI/Swift   │ UniFFI/Kotlin  │ wasm-bindgen   │ crate dep
        └────────────────┴───────┬────────┴────────────────┘
                                 ▼
          ┌──────────────────────────────────────────────┐
          │  vela-core  (rust/crates/vela-core)          │
          │  Crux state machines — business rules        │
          │  primitives · abi · eip712 · safe ·          │
          │  webauthn · ctap · i18n (15 locales)         │
          │  pure, deterministic — no I/O, no network    │
          └──────────────────────────────────────────────┘
```

Why it's built this way (and why the React Native app it started as was
retired), how shared assets are generated and gated, and how each app is built
and shipped: **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

## Repository map

| Path | What | Start with |
| --- | --- | --- |
| [`rust/`](rust/README.md) | `vela-core`: state machines, crypto, ABI, WebAuthn/CTAP, i18n; UniFFI and wasm bindings | `cargo test --workspace --features vela-core/i18n-all` in `rust/` |
| [`app-desktop/vela-wallet`](app-desktop/vela-wallet/README.md) | Desktop app on gpui (macOS, Windows, Linux) | `cargo run` |
| [`app-web/vela-wallet`](app-web/vela-wallet/README.md) | Web wallet and the Chrome extension | `pnpm install && pnpm dev` |
| [`app-ios/VelaWallet`](app-ios/VelaWallet) | iOS app, SwiftUI | `./rust/scripts/build-ios-xcframework.sh`, then Xcode — and `./rust/scripts/check-ios-core-fresh.sh` before any device test, because `xcodebuild` never rebuilds the Rust |
| [`app-android/vela-wallet`](app-android/vela-wallet) | Android app, Jetpack Compose | generate the bindings ([ci.yml](.github/workflows/ci.yml) `android`), then `./gradlew :app:installDebug` |
| [`app-web/getvela.app`](app-web/getvela.app) | The website and the user docs | `bun install && bun run dev` |
| [`app-web/clearsigning`](app-web/clearsigning/README.md) | Standalone signing page, zero build | `python3 -m http.server`, or open `index.html` |
| [`scripts/`](scripts/package.json) | Generators and CI gates the apps share | [Tooling](docs/ARCHITECTURE.md#tooling) |
| [`specs/`](specs/) | Every feature's spec, plan and delivery report, numbered in order | the latest one |

The quickest look at the whole thing is the desktop app or the web wallet;
each app's README lists its prerequisites.

## Run the whole stack

| Service | Repository | Does |
| --- | --- | --- |
| Relay | [vela-relay](https://github.com/mondaylabsltd/vela-relay) | Submits your UserOperations and collects the fee you signed |
| Public-key index | [p256-index](https://github.com/mondaylabsltd/p256-index) | Writes a new wallet's keys to the ownerless registry contract; answers lookups |
| Chain data | [ethereum-data](https://github.com/atshelchin/ethereum-data) | Networks, tokens, logos, ERC-7730 descriptors |
| Exchange rates | [vela-currency](https://github.com/mondaylabsltd/vela-currency) | ECB rates, Frankfurter-compatible |

All MIT, on Docker or Cloudflare Workers. Point an app at them in **Settings →
Service Endpoints** (not yet on iOS). Run your own relay and the fee your wallet signs goes to
your own treasury. The one thing you can't replace is the passkeys' domain,
`getvela.app`: the [self-hosting guide](https://getvela.app/docs/self-hosting)
walks through every service and the ways around that.

## Security model

- Keys are WebAuthn P-256 credentials held by your passkey provider or security
  key; the apps only ever receive signatures.
- One to seven keys, chosen at creation and fixed after that; any one can sign.
- Contracts are Safe v1.4.1, Safe's 4337 module and Safe's passkey signers,
  unmodified. Vela holds no key and no role on your account.
- The code you run could present a malicious transaction, and a passkey prompt
  shows the domain, not the transaction — see the threat model in the
  [whitepaper](https://getvela.app/docs/whitepaper).
- The Safe contracts, modules and EntryPoint are audited. Vela's own code is
  not, and no audit is scheduled: [audits & known issues](https://getvela.app/docs/security-audits).

## Status

Alpha, holding real funds on 24 mainnets — start with small amounts.
The web wallet runs at [wallet.getvela.app](https://wallet.getvela.app); the
desktop apps and the Chrome extension ship from
[Releases](https://github.com/mondaylabsltd/vela-wallet/releases). The iOS and
Android apps are wired end to end and tested on real devices. They are headed
for the App Store and Google Play as a one-time purchase, and you can build
them yourself for free.

## Verify what you downloaded

A checksum only says that two files are the same file. Ask instead where the
file came from: every package attached to a
[Release](https://github.com/mondaylabsltd/vela-wallet/releases) carries a build
provenance attestation — signed by the workflow run that produced it, recording
the commit it was built from — which GitHub keeps and anyone can check with the
[GitHub CLI](https://cli.github.com) (`gh auth login` once; the check is free for
a public repository):

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet

# Stricter: also insist it came from the release workflow and nowhere else.
gh attestation verify VelaWallet-Setup-0.9.4-x64.exe --repo mondaylabsltd/vela-wallet \
  --signer-workflow mondaylabsltd/vela-wallet/.github/workflows/release.yml
```

The `SHA256SUMS` files stay on each release for anyone without `gh`
(`sha256sum -c`); the one CI writes is attested along with the packages it
lists, while `SHA256SUMS-macos` is written on the Mac that signs the images.

**macOS** images are signed with our Developer ID and notarized by Apple on a
Mac, by hand — nothing that can sign as us is stored on GitHub — so their
provenance is Apple's, and the attestation is added afterwards from the
published bytes (the `macOS provenance` workflow, so `--signer-workflow` for a
`.dmg` is `.github/workflows/macos-attest.yml`). macOS checks them itself when
you open one; to ask it out loud:

```bash
xcrun stapler validate VelaWallet-0.9.4-macos-arm64.dmg
spctl -a -t open --context context:primary-signature -v VelaWallet-0.9.4-macos-arm64.dmg
```

**Windows** — the installer is not code-signed, and an attestation does not
change that: SmartScreen says "Windows protected your PC" once (**More info →
Run anyway**). That prompt is permanent. We decided against buying a
code-signing certificate: it removes a warning without making the file any more
genuine, and what actually tells you the file is ours is the attestation above,
which you can check yourself and we cannot forge.

Attestations begin with the first release built after this landed; earlier
packages have their checksums only.

## Contributing

Vela Wallet relies on four backend endpoints. Default instances are provided, but you can deploy your own for full self-custody.

Configure custom endpoints in **Settings > Advanced > Service Endpoints**.


| Service                  | Description                                       | Repository                                                                                                                      |
| -------------------------- | --------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- |
| **Chain Data Index**     | Network info, token data, chain logos             | [atshelchin/ethereum-data](https://github.com/atshelchin/ethereum-data)                                                         |
| **Passkey Index**        | Public key storage for cross-device recovery      | [atshelchin/webauthnp256-publickey-index.biubiu.tools](https://github.com/atshelchin/webauthnp256-publickey-index.biubiu.tools) |
| **Bundler Service**      | ERC-4337 transaction bundler                      | [mondaylabsltd/vela-relay](https://github.com/mondaylabsltd/vela-relay)                                                           |
| **Exchange-Rate Source** | USD-based fiat rates that drive the currency list | [mondaylabsltd/vela-currency](https://github.com/mondaylabsltd/vela-currency) (Frankfurter-compatible, ECB daily rates)              |

The first three are Vela services that each expose a `/api/health` endpoint. The wallet validates three checks before accepting a custom endpoint for them:

1. **HTTPS** — only secure connections accepted
2. **Reachable** — server responds within 10 seconds
3. **Valid response** — `/api/health` returns the correct `service` identifier and `status: "ok"`

The **Exchange-Rate Source** is any USD-based FX API — the default is `https://vela-currency.getvela.app/v2/rates?base=USD`, served by [mondaylabsltd/vela-currency](https://github.com/mondaylabsltd/vela-currency), Vela's small Frankfurter-compatible service (ECB daily reference rates, dual-runtime Deno / Cloudflare Workers). It's validated by returning a parseable USD-based rate set (not `/api/health`). A self-hosted [Frankfurter](https://frankfurter.dev) instance works as a drop-in alternative. Pin the base to USD (`?base=USD`), or every conversion is silently wrong. For the response shapes Vela accepts, the Chainlink fallback, and a porting guide, see [docs/fiat-price.md](docs/fiat-price.md).

A fifth endpoint is **optional**, and only for the Clear Signer: signing a transaction on
a *second* device pairs the two through a WebSocket **tunnel**, which forwards sealed
frames and can read none of them. It lives in its own repository, `vela-tunnel` (a Rust
room-rules crate plus two hosts — a distroless Docker image and a Cloudflare Worker), and
the wallet reaches it by URL under **Settings > Sign with > Tunnel**. Anyone can run
their own; the protocol is [specs/075-clear-signer-channel/contracts/tunnel.md](specs/075-clear-signer-channel/contracts/tunnel.md).
Signing on the same device uses Bluetooth or the device's own loopback and needs no
tunnel at all.

## Gas & Fee Model

Vela Wallet uses ERC-4337 account abstraction, so transactions are relayed by a **bundler** instead of being submitted directly by the user. This means:

### How Gas Fees Work

Each transaction incurs a gas fee deducted from **your Safe wallet** — in the network's native token by default, or in a supported stablecoin where the bundler offers ERC-20 settlement (Tempo has no native coin, so gas there is always settled in USD stablecoins). The fee consists of:

- **On-chain gas cost** — The actual cost to execute the transaction on the blockchain.
- **Relayer service fee** — The total charge is a fixed multiple of the raw on-chain cost: currently **3× on standard networks** (`INBAND_MARKUP` in the web shell's `safe-transaction.ts`, mirrored by the desktop executor) and **2× on Tempo**, with minimums of 0.00001 native units or $0.01 in stablecoins. The bundler is the price authority for the gas price itself. The margin pays the relayer that fronts the gas and runs the infrastructure.

The confirmation screen shows a single quoted total in the fee asset and in fiat. The quoted amount and its recipient are part of the signed payload, so the relayer is paid exactly what was shown.

### Gas Relayer Account

Before your first transaction on a network, you need to fund a **dedicated gas relayer account** (bundler EOA). This is a one-time setup:

- The deposit amount is based on the actual transaction gas requirement.
- The deposit is **non-refundable** — it serves as the relayer's initial operating balance.
- The relayer address **may change** due to service upgrades, requiring a new deposit.
- After the initial deposit, the relayer is self-sustaining: it earns back gas costs from each transaction via EntryPoint refunds.

### Max Send

When sending the maximum amount of a native token (ETH, BNB, etc.), the wallet automatically reserves enough for the transaction's gas fee (EntryPoint prefund). This prevents "insufficient balance" failures.

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
- Only positive results are cached (locally, 24h TTL)
- No third-party API dependencies — all queries use direct on-chain RPC calls

The registry is `NAME_SERVICES` in the web shell's `recipient-identity.ts` (`app-web/vela-wallet/src/lib/services/`) and, for the desktop, in `app-desktop/vela-wallet/src/executor/identity.rs` — add a service to both.

## Security Model

- **No private key access** — Signing uses WebAuthn P-256 keys managed by your OS (iCloud Keychain / Google Password Manager) or by a security key. Vela Wallet never has access to the private key.
- **Safe smart account** — Your wallet is a Safe proxy contract, audited and battle-tested with billions in TVL.
- **On-device only** — Transaction construction, signing, and signature verification all happen locally. The bundler only receives the signed UserOperation.
- **Passkey-scoped** — Each wallet is bound to its founding passkeys. Transactions require the authenticator's own verification (Face ID / fingerprint / PIN) every time.

Every change lands as a numbered spec in [`specs/`](specs/) (spec, plan, tasks,
results); read a recent one before proposing something larger than a fix.
[Bug reports](https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml)
and translation fixes are welcome. If this is the wallet you'd want to exist,
a star helps other developers find it.

## License

MIT — see [LICENSE](LICENSE).
