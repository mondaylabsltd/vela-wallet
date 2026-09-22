# Architecture and developer notes

The [README](../README.md) is the short version. This is the rest: why the
wallet is one Rust core with four native shells, what lives where, the shared
tooling, how each app is built and distributed, and the developer-level detail
behind the service endpoints, the fee and name resolution. User-facing
explanations live at [getvela.app/docs](https://getvela.app/docs).

- [Architecture](#architecture) — [why we left Expo](#why-we-left-expo), [the core](#what-lives-in-the-core), [the shells](#the-shells), [one source of truth](#one-source-of-truth-for-everything-shared), [where it stands](#where-it-stands)
- [Tooling](#tooling) — the `scripts/` package
- [Distribution](#distribution) — release channels, and phone apps you build yourself
- [Platform notes](#platform-notes) — web, desktop, app icons
- [Build for Web](#build-for-web-cloudflare-workers)
- [Service endpoints](#service-endpoints) · [Fee model](#fee-model) · [Recipient identity resolution](#recipient-identity-resolution)

## Architecture

### Why we left Expo

Two structural problems that no amount of care inside a React Native codebase could fix:

- **The code that must not be wrong existed several times over.** Keccak-256 was hand-rolled twice — TypeScript and a parallel Swift copy — alongside SHA-256, a dynamic ABI decoder and P-256 curve math on BigInt. Counterfactual Safe address derivation was maintained in three places, including byte-matched constants in the bundler repo. A divergence in any copy silently loses funds or makes the signing sheet lie about what is being approved. ([rust/README.md](../rust/README.md))
- **The rules that guard your money lived inside React components.** Rules bought with incidents — a passkey must *prove* it can sign before anything persists, a cancelled verification must resume from the signature instead of minting a second passkey — sat in `useState` cells and mutable refs, untestable without a browser. One send controller alone held ~40 state cells whose ordering was maintained by comments and discipline. ([specs/011](../specs/011-crux-onboarding-state/spec.md), [specs/016](../specs/016-crux-wallet-state/spec.md))

So the computation and the rules moved down into one Rust crate, and each platform got a shell that renders it with that platform's own UI toolkit — not a cross-platform runtime pretending to be four.

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
          │  ┌────────────────────────────────────────┐  │
          │  │ Crux state machines — business rules   │  │
          │  │ send · sign_request · fee_policy ·     │  │
          │  │ rpc_pool · contacts · balance · …      │  │
          │  ├────────────────────────────────────────┤  │
          │  │ primitives · abi · eip712 · safe ·     │  │
          │  │ webauthn · identicon · i18n (15 locs)  │  │
          │  └────────────────────────────────────────┘  │
          │  pure, deterministic — no I/O, no network    │
          └──────────────────────────────────────────────┘
```

### What lives in the core

- **Deterministic computation, zero I/O**: hex/base64url/quantity, keccak256, sha256, EIP-55, CREATE2, runtime calldata decoding, `eth_signTypedData_v4` digests, counterfactual Safe and splitter addresses, WebAuthn COSE/DER handling and two-assertion public-key recovery, account identicons, and the i18n engine with all 15 locale catalogs compiled in.
- **Business state as [Crux](https://github.com/redbadger/crux) machines**: the core owns every decision. A shell translates input into Events, executes the effects the core asks for (passkey ceremony, storage, RPC), hands the results back, and renders the ViewModel it gets. Rules become testable without a browser, a device, or a network.
- **No hand-rolled primitives, ever**: hashing, curve math, ABI coding and CBOR come from alloy-core, sha2, p256/ecdsa and ciborium/coset, at pinned versions.

One implementation reaches four surfaces: UniFFI generates the Swift and Kotlin bindings, `vela-core-wasm` produces the committed web artifact in `rust/pkg-web`, and the desktop client depends on the crate directly.

### The shells

| Platform | Directory | Stack | Run it |
| --- | --- | --- | --- |
| iOS | [app-ios/VelaWallet](../app-ios/VelaWallet) | SwiftUI + VelaCoreKit (SPM package wrapping the xcframework) | `./rust/scripts/build-ios-xcframework.sh`, then open `VelaWallet.xcodeproj`, ⌘R |
| Android | [app-android/vela-wallet](../app-android/vela-wallet) | Kotlin + Jetpack Compose + the UniFFI Kotlin bindings | generate the bindings (see [.github/workflows/ci.yml](../.github/workflows/ci.yml) `android`), then `./gradlew :app:installDebug` |
| Web | [app-web/vela-wallet](../app-web/vela-wallet/README.md) | SvelteKit 2 / Svelte 5 on Cloudflare Workers; also builds the Chrome extension (`pnpm build:extension`; the release zip is cut with every release, by pushing `release/v<version>` → [release.yml](../.github/workflows/release.yml)) | `pnpm install && pnpm dev` |
| Desktop | [app-desktop/vela-wallet](../app-desktop/vela-wallet/README.md) | Rust + [gpui](https://github.com/zed-industries/zed) | `cargo run` |

### One source of truth for everything shared

The four shells are only worth having if they cannot drift apart. Each shared asset has exactly one origin, a generator, and a CI gate that regenerates it and fails on a non-empty diff:

| Shared asset | Source of truth | Generated into |
| --- | --- | --- |
| Translations (15 locales) | `rust/crates/vela-core/i18n/locales/` | compiled-in Rust catalogs and `assets/i18n/<locale>.json`, which iOS, Android and the web shell all read (`npm --prefix scripts run gen:i18n`) |
| Wire types (Crux events, operations, views) | the Rust enums in `rust/crates/vela-core/src/app/` | `app-web/vela-wallet/src/lib/{onboarding,session,core}/generated` (`npm --prefix scripts run gen:core-types`) |
| The web wasm artifact | `rust/crates/vela-core-wasm` | `rust/pkg-web/` (the JS glue) + `assets/wasm/vela_core_bg.<fingerprint>.wasm` (`npm --prefix scripts run build:wasm`) |
| Design tokens | [docs/design-tokens.json](design-tokens.json) (Penpot DTCG export) | `tokens.css` / `tokens.ts` for web, `Tokens.swift` for iOS — literals are test-banned in product UI |
| Behavior | the conformance corpus in `rust/crates/vela-core/tests/vectors/` — the crypto/ABI/Safe/WebAuthn vectors are frozen goldens; the identicon and i18n vectors regenerate from the pinned `identicons-esm` and `i18next` packages | replayed through Rust, the Kotlin bindings, the Swift bindings and the shipped web artifact |
| App icons | [docs/design/icon/](design/icon/) | every platform's icon set (see [App icons](#app-icons)) |

Two parity suites compare the Rust ports against the npm packages the corpus was extracted from: the full 17,115 locale/key cross-product plus 50,000 fuzzed option bundles for i18n, and every address literal in the repo plus 200,000 random seeds for identicons. Keeping those two packages installed is what the npm package in `scripts/` is for.

### Where it stands

- **Web**: the SvelteKit shell is the production web wallet build, deployed as the Cloudflare Worker `vela-wallet-web` (see [Build for Web](#build-for-web-cloudflare-workers) for which build the hostname currently serves).
- **Desktop**: wired to the core end to end — onboarding, wallet, send, contacts, message signing, scanning, simulation (specs 030–038).
- **iOS and Android**: wired to the core end to end (specs 040–049 for Android, 050–058 for iOS) and tested on real devices, including the in-app dApp browser. Not yet published in the App Store or Google Play.
- **Retired**: the React Native / Expo app (spec 039). It could not run on a phone after PR #168 made the core facade wasm-only, and no store build of it ever shipped.

Feature specs, plans and delivery reports live in [specs/](../specs/), numbered in the order they landed.

## Tooling

The repository root carries no npm state. The generators and gates every shell depends on are an npm package in [scripts/](../scripts/package.json); run them from the root with `--prefix`, or `cd scripts` first:

```bash
npm ci --prefix scripts                       # four packages: the two npm oracles + @noble/* for scripts/onchain
npm --prefix scripts run gen:i18n             # corpus → Rust catalogs + assets/i18n (commit together with the corpus edit)
npm --prefix scripts run gen:core-types       # Rust enums → app-web's generated/ wire types
npm --prefix scripts run build:wasm           # vela-core → rust/pkg-web + assets/wasm/vela_core_bg.<hash>.wasm
npm --prefix scripts run verify:i18n          # parity: Rust i18n vs the pinned i18next
npm --prefix scripts run verify:identicon     # parity: Rust identicons vs the pinned identicons-esm
npm --prefix scripts run check:expo-residue   # the Expo tree stays retired (fails CI on a dead command in any doc)
```

`i18next` and `identicons-esm` are the oracles the conformance corpus is replayed against; `@noble/curves` and `@noble/hashes` are resolved from `scripts/node_modules` by `scripts/onchain/`. Do not "clean up" those four.

## Distribution

| | Packages | Stores | Build it yourself |
| --- | --- | --- | --- |
| **Web** | nothing to install — [wallet.getvela.app](https://wallet.getvela.app/) | — | [Build for Web](#build-for-web-cloudflare-workers) |
| **Desktop** — macOS, Windows, Linux | [GitHub Releases](https://github.com/mondaylabsltd/vela-wallet/releases), free; every package attached there is meant to install as downloaded | when they accept us — a store listing is paid, and buys one-click install and updates, never access | [app-desktop/vela-wallet](../app-desktop/vela-wallet/README.md) |
| **Browser extension** | [GitHub Releases](https://github.com/mondaylabsltd/vela-wallet/releases), free, loaded by hand | Chrome Web Store, when listed | [extension/](../app-web/vela-wallet/extension/README.md) |
| **iOS, Android** | none — the phone apps are **not** published on GitHub | the App Store and Google Play (paid) | yes; read the note below first |

The rule behind the table ([spec 063](../specs/063-release-channels/spec.md)): nothing is attached to a release unless a person with no developer tools can install it and reach a wallet. A macOS `.dmg` is attached only when that build was signed with our Developer ID and notarized by Apple; the Windows installer is not code-signed, so SmartScreen asks once (**More info → Run anyway**).

### Provenance — where a package came from

A `SHA256SUMS` file says two files are identical; it cannot say who made either of them, and it sits on the same page as the download. So every file `release.yml` attaches is also **attested** ([spec 081](../specs/081-audit-product-gaps/spec.md), FR-012) in the `publish` job — the one place in the whole release where every artifact exists together. The job asks for `id-token: write` and `attestations: write`, and `actions/attest-build-provenance@v2` signs a statement naming the commit, the workflow and the run. It is deliberately not in the packaging workflows: a reusable workflow cannot exceed its caller's permissions, and those workflows also run on pull requests, where there is nothing to attest. `clearsigning-package.yml` attests its zip the same way, in its own publishing job.

```bash
gh attestation verify vela-wallet_0.9.4_amd64.deb --repo mondaylabsltd/vela-wallet
gh attestation verify VelaWallet-Setup-0.9.4-x64.exe --repo mondaylabsltd/vela-wallet \
  --signer-workflow mondaylabsltd/vela-wallet/.github/workflows/release.yml
```

**macOS is the exception, and is documented as one.** The images are built, signed, notarized and attached by hand on the founder's Mac (spec 063 §3a), so no workflow can honestly claim to have built them; Apple's notarization is their proof, and `xcrun stapler validate` and `spctl -a -t open --context context:primary-signature -v` are how you ask for it. `macos-attest.yml` (`workflow_dispatch`, a tag) then downloads the images **as published**, repeats both checks, and attests those exact bytes — so a `.dmg` verifies like everything else, with `--signer-workflow …/macos-attest.yml`.

**Attestation is not code signing.** It says the file is the one this repository published; it is not a certificate the operating system knows, so the Windows SmartScreen prompt stays exactly as described above.

### A phone app you built yourself

It is the same wallet, with one door closed. "This device" — the phone's own passkey, through Credential Manager on Android or the system passkey sheet on iOS — only works in a build signed with *our* key: the operating system checks the app's signing identity against `getvela.app` before it lets the app use a `getvela.app` passkey, and your signature is not ours.

The two other ways in do not ask the operating system, and work in any build:

- **Scan with another phone.** The wallet shows a QR code; a phone that already holds your passkey scans it and approves. The whole exchange (hybrid / caBLE) runs in `vela-core`.
- **A security key.** USB on Android; on iOS a key that speaks FIDO over its smart-card interface (YubiKey firmware 5.8 or later). The CTAP conversation is `vela-core`'s too.

These are the same escape hatches the store apps fall back on if `getvela.app` is ever unreachable — which is what makes "you can always build it yourself" a true sentence rather than a polite one.

## Platform notes

Capabilities differ per shell and are documented where they were built: each shell's README, and the spec that wired the capability (for example passkey methods per platform in [specs/019](../specs/019-onboarding-live-wiring/spec.md)). The shells share the core's rules; what differs is the platform API each one drives.

### Web Notes

- **Passkey rpId**: `getvela.app` on `getvela.app` and its subdomains, in the extension, and during SSR, consistent with the native shells. **Any other host uses its own hostname** — local dev and preview hosts, and a self-hosted deployment on another domain, get their own passkeys and therefore a different wallet (`relyingPartyId()` in `src/lib/onboarding/core/passkey.ts`). The parallel space (`/[locale]/parallel`) is how a preview is exercised with a fixed key set instead.
- **Local storage**: IndexedDB for wallet records, `localStorage` for the onboarding keys and the endpoint override. No cross-device sync — accounts are stored in the browser only; the passkey and the public-key index rebuild them elsewhere.
- **dApps**: the same code builds a Chrome extension that injects the provider into pages (`app-web/vela-wallet/extension/`); the plain web app has no in-page dApp transport. The extension's `host_permissions` for `https://getvela.app/*` are what let it use `getvela.app` passkeys — the same wallet — and they are also the path to an existing wallet if the domain is ever gone.

### Desktop

The desktop client is a **separate native application** — Rust on
[gpui](https://github.com/zed-industries/zed), not a web view — in
[app-desktop/vela-wallet](../app-desktop/vela-wallet). It shares the `vela-core`
crate, the design sources and the fonts under `assets/fonts/` with the other
shells.

Installable packages are built from that directory, for x64 and ARM64:

| Platform | Package | Command |
| --- | --- | --- |
| Windows 10/11 | Inno Setup installer | `./scripts/build-windows-installer.ps1` |
| macOS 11+ | `.app` bundle | `./scripts/build-macos-app.sh` |
| Fedora, RHEL, openSUSE | `.rpm` | `./scripts/build-linux-packages.sh --formats rpm` |
| Debian, Ubuntu | `.deb` | `./scripts/build-linux-packages.sh --formats deb` |
| Any Linux, sandboxed | Flatpak bundle | `./scripts/build-flatpak.sh` |

Setup, system dependencies and release steps are in
[app-desktop/vela-wallet/README.md](../app-desktop/vela-wallet/README.md).

### App icons

Every icon in the repository — both native projects, the desktop
packages and this site's favicons — is rendered from one vector source,
[docs/design/icon/](design/icon/). Nothing is hand-exported, so the platforms cannot
drift apart:

```bash
./scripts/gen-app-icons.sh                                   # app-ios, app-android, getvela.app
app-desktop/vela-wallet/scripts/generate-desktop-icons.sh    # Linux hicolor, Windows .ico, macOS .iconset
```

Both scripts commit their output and encode the per-platform rules that
otherwise fail silently — iOS rejecting alpha, Android's 66.7% safe zone,
Windows resolving the icon by resource id. The details are in the desktop
README under [Icons](../app-desktop/vela-wallet/README.md#icons).

## Build for Web (Cloudflare Workers)

The web build comes from [app-web/vela-wallet](../app-web/vela-wallet/README.md) — the SvelteKit shell, deployed as the Cloudflare Worker `vela-wallet-web`:

```bash
cd app-web/vela-wallet
pnpm install
pnpm build      # tokens drift check + worker types + prerender all 15 locales
pnpm preview    # wrangler dev of the built worker on :4173
```

`pnpm build` runs the vela-core wasm i18n engine in Node to prerender each `/{locale}` page, so no translation runtime and no wasm reach the deployed Worker. Cloudflare builds the same command from the repo; CI runs it too, so a broken build fails the PR rather than the deploy.

**`wallet.getvela.app` is served by this Worker** since 2026-09-11 (the founder moved the hostname off the retired Expo build's Cloudflare Pages deployment the same day the tree was deleted; verified: `/` answers a 307 to the visitor's locale, `/en/wallet` is the SvelteKit page). The old Expo paths — `/onboarding`, `/pay`, `/web-request` — no longer exist; the wallet's URL space is `/{locale}/…`. The deployment runbook ([docs/project-takeover/05](project-takeover/05-deployment-runbook.md)) has the smoke test and the rollback (the Pages project's last deployment can be re-attached).

## Service endpoints

The step-by-step guide for users is **[getvela.app/docs/self-hosting](https://getvela.app/docs/self-hosting)**; this section is the developer summary.

The wallet uses four Vela-operated services by default. Each can be replaced in **Settings → Advanced → Service Endpoints** (desktop: Settings → Service Endpoints). Defaults are in `rust/crates/vela-core/src/app/network_admin.rs`.

| Service | Default | Repository | `/api/health` `service` |
| --- | --- | --- | --- |
| **Relay** (ERC-4337 bundler with in-band settlement) | `https://vela-relay-cf.getvela.app` (+ `/{chainId}`) | [mondaylabsltd/vela-relay](https://github.com/mondaylabsltd/vela-relay) (MIT) | `vela-relay` |
| **Public-key index** (registers wallets in the on-chain registry, answers lookups) | `https://p256-index-v2.getvela.app` | [mondaylabsltd/p256-index](https://github.com/mondaylabsltd/p256-index) (MIT) | `webauthn-p256-publickey-registry` |
| **Chain data** (networks, tokens, logos, ERC-7730 descriptors) | `https://ethereum-data.getvela.app` | [atshelchin/ethereum-data](https://github.com/atshelchin/ethereum-data) (MIT) | `ethereum-data` |
| **Exchange rates** (USD-based) | `https://vela-currency.getvela.app/v2/rates?base=USD` | [mondaylabsltd/vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) | checked by response shape |

The health check drives a badge; it does **not** gate the save (`network_admin.rs`: "Health badges never gate saves"). Any Frankfurter-compatible USD-based rate source works for exchange rates — keep `?base=USD` (see [docs/fiat-price.md](fiat-price.md)).

"Empty means the default" is one rule, `NetServiceEndpoints::effective` in `network_admin.rs`, and each shell reads the endpoint at the moment it uses it rather than caching one at start-up (spec 081 FR-002 — that cache was how a configured public-key index could be ignored for the life of a process). `P256_INDEX_DOMAIN_REGISTRY` is in p256-index's `.env.example`; without it the server hands out challenges the contract rejects.

## Fee model

Transactions are ERC-4337 UserOperations submitted by a relay. The relay is paid **in band**: the operation declares zero EntryPoint fees and carries a transfer from the Safe to the relay's settlement address inside the MultiSend the passkey signs, so the user pays exactly the quoted amount and the relay rejects anything else.

- **Amount** — `max(3 × padded gas limits × max(C, R), floor)` on standard networks (`INBAND_MARKUP` in `rust/crates/vela-core/src/app/fee_policy.rs`, shared by all shells): C is the wallet's own gas-price reading, R the relay's price for the chosen speed tier (default *fast*); the padded limits are the simulated verification and call gas ×1.5 with floors (300k / 2M verification for deployed / undeployed accounts). On Tempo (pathUSD gas) the multiple is 2. The minimum is about $0.01. A relay quote above 3 × C is refused. The padding puts the result above the operation's actual on-chain cost; the relay keeps the difference. The exact amount is on the confirm screen before the user signs.
- **Recipient** — whichever relay the wallet is set to (Vela's by default). Any vela-relay deployment works; a self-hosted relay reads chain metadata from its own directory with `VELA_RELAY_CHAIN_DIRECTORY_URL` (vela-relay v0.9.6 and later).
- **Fee asset** — the native coin, or a USD stablecoin from the chain's `stables` list when the relay can price the native coin; pathUSD on Tempo. No paymaster.
- **No per-wallet deposit.** The old "gas relayer account" activation deposit no longer exists. When a relay's own treasury on a chain is empty, the send flow says so; on Vela-run networks the user can report it or voluntarily contribute (non-refundable, and it does not pay for the transaction).
- **Max send** keeps back the fee amount for native-coin sends.

User-facing explanation: [getvela.app/docs/networks-and-fees](https://getvela.app/docs/networks-and-fees).

## Recipient Identity Resolution

The wallet names an address the user has entered (reverse lookup only — typing a name does not resolve an address). Order:

1. **Vela registry** — the address's first owner on the shared WebAuthn signer → the public-key index → the registry contract on Gnosis, then Ethereum (`rust/crates/vela-core/src/registry_lookup.rs`).
2. **Name services, read on-chain**: `.bnb` (BSC), `.arb` (Arbitrum), `.g` (Gravity), Basename (Base, ENSIP-19) and ENS (mainnet), in that priority.

The name-service table lives in each shell (web `recipient-identity.ts`, desktop `executor/identity.rs`, iOS `RecipientIdentity.swift`, Android `IdentityResolver.kt`); only positive results are cached.

A reverse record is a claim, not proof — anyone can point their own record at any string — so a name from step 2 is shown only when it **resolves forward to the same address**. The check is a transcript machine in the core (`app/name_verify.rs`, the shape spec 067 established): resolver → `addr`, with an ENSIP-10 wildcard walk for Basenames, one case-insensitive comparison, and the proved name handed back so what is drawn is what was checked. It fails closed — an unanswered call, a CCIP revert or a timeout shows the bare address, because failing open would let whoever poisons a record choose the moment. Step 1 is not put to it: a Vela wallet name is a label the chain gives for that address, not a record pointing at it.
