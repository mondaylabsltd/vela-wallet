# Vela Wallet

[![CI](https://github.com/mondaylabsltd/vela-wallet/actions/workflows/ci.yml/badge.svg)](https://github.com/mondaylabsltd/vela-wallet/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**An Ethereum wallet you actually own.** Every piece of it — the apps and the
services behind them — is MIT-licensed, builds from public repositories,
and runs on your own machines.

- Your account is an **unmodified Safe v1.4.1** on **ERC-4337** (EntryPoint
  v0.7), signed by **passkeys or FIDO2 security keys** and verified on-chain as
  P-256 (RIP-7212). No seed phrase, no Vela contract in the funds path, no admin
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
- **Any EVM chain with RIP-7212.** [Chain setup](https://getvela.app/chain-setup)
  deploys the missing Safe and ERC-4337 contracts that anyone can deploy.
- **Clear signing** from ERC-7730 descriptors, with an explicit blind-signing
  warning for anything that can't be decoded.

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
| [`app-ios/VelaWallet`](app-ios/VelaWallet) | iOS app, SwiftUI | `./rust/scripts/build-ios-xcframework.sh`, then Xcode |
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

## Contributing

Every change lands as a numbered spec in [`specs/`](specs/) (spec, plan, tasks,
results); read a recent one before proposing something larger than a fix.
[Bug reports](https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml)
and translation fixes are welcome. If this is the wallet you'd want to exist,
a star helps other developers find it.

## License

MIT — see [LICENSE](LICENSE).
