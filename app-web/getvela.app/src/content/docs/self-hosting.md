---
title: Self-hosting guide
description: "Everything Vela runs for you, what each piece does, and how to replace it with your own — the relay, the public-key index, chain data, exchange rates and the apps — plus the one thing you cannot replace and how to live without getvela.app."
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Self-hosting guide

Your money is in a Safe contract on-chain, controlled by your keys. Nothing Vela
runs can move it. What Vela does run is the machinery that makes the wallet
convenient: a relay that submits your transactions, an index that helps a new
device find your wallet, a directory of chain data, an exchange-rate feed, and
the apps themselves.

This page lists every one of those pieces, what breaks without it, and how to
run your own. It also covers the one piece you cannot replace — the domain your
passkeys belong to — and what to do if getvela.app goes away.

<Callout type="info" title="Who this page is for">
You should be comfortable with a terminal, Docker or Cloudflare Workers, and
funding an address on a chain. Nothing here is needed to use Vela day to day.
</Callout>

## The map

| Piece | What it does | Vela's default | Can you replace it? | Without it |
| --- | --- | --- | --- | --- |
| **Relay** | Receives your signed operation, pays the gas, submits it, collects the fee you signed | `vela-relay-cf.getvela.app` | Yes — run [vela-relay](#relay) and point the wallet at it | You can't send |
| **Public-key index** | Registers a new wallet's keys on-chain; answers "which wallet is this key part of?" | `p256-index-v2.getvela.app` | Yes — run [p256-index](#index) | New wallets can't be created; sign-in falls back to reading the chain |
| **Registry contract** | The permanent public record of each wallet's keys | `0x94fD…1EA9` on Gnosis | Not needed — nobody owns it; the wallet reads it directly | — |
| **Chain data** | Network details, token lists, logos, clear-signing descriptors | `ethereum-data.getvela.app` | Yes — run [ethereum-data](#chain-data) | No token lists or logos; fewer transactions decoded; adding networks fails |
| **Exchange rates** | Fiat values in your display currency | `vela-currency.getvela.app` | Yes — run [vela-currency](#exchange-rates) or any Frankfurter-compatible source | The apps fall back to on-chain Chainlink rates where they can (desktop shows USD) |
| **RPC nodes** | Reading balances, simulating transactions | Public endpoints per network | Yes — per network, in Settings → Networks | Vela fails over between endpoints |
| **The apps** | The wallet itself | wallet.getvela.app, release builds | Yes — [build them](#web-app) | — |
| **getvela.app** | The domain your passkeys belong to | — | **No** — see [below](#if-getvela-app-disappears) | — |

A few third-party services are also contacted and are not Vela's: the public
function-selector databases (sourcify, openchain, 4byte) used as a last resort
when decoding a transaction, the authenticator directory that names your
security key model, and Apple's and Google's tunnel servers when you sign with
a phone by scanning a QR code.

## The one thing you cannot replace: the passkey's domain

<span id="if-getvela-app-disappears"></span>

A passkey belongs to the website it was created for. Vela's keys are created for
`getvela.app`. Browsers offer them only to pages on getvela.app or its subdomains
(or to origins getvela.app declares as related), and a phone's built-in passkeys
work only in apps getvela.app vouches for. Outside the browser the rule is looser:
Chrome lets an extension with permission for getvela.app use them, and a program
on your computer can ask a security key or a phone for a getvela.app signature
directly — which is how self-built apps work, and why software you run matters.
Two things follow.

**A copy of the web wallet on your own domain is a different wallet.** Served
from `wallet.example.com`, the same code creates passkeys for
`wallet.example.com` — new keys, and therefore a new address. It cannot sign for a wallet created at
wallet.getvela.app. That copy is still useful: for a wallet you create there,
or to run the whole stack yourself from scratch.

**For an existing wallet, these still work if getvela.app is offline or gone:**

| Way in | Keys it can use | Where to get it |
| --- | --- | --- |
| The **Vela browser extension** (Chromium browsers: Chrome, Edge, Brave) | Any key the browser can reach: this device's passkey, a USB security key (NFC where the computer supports it), a phone by QR | A release zip from [GitHub](https://github.com/mondaylabsltd/vela-wallet/releases), or [build it](#web-app) |
| A **desktop or phone app you build yourself** | A phone by QR, and USB security keys | [Build it](#web-app) |
| The **store and notarized desktop apps** | A phone by QR and security keys always; "this device" passkeys only while the operating system can still verify the app against getvela.app | GitHub releases (stores later) |

The extension can use `getvela.app` keys because Chrome lets an extension with
permission for a site use that site's passkeys. The browser checks that
permission locally; we have measured it working, though not yet with the domain
actually offline. A self-built app can use a phone
or security key because Vela talks to them directly; the phone's own passkey
("this device") needs the app to be signed by Vela, and yours isn't.

The [signing page](/docs/clear-signing-self-host) is not a way in on its own:
it signs requests that another program sends it, and no Vela app sends them yet.

<Callout type="warning" title="Whoever controls the domain can ask for a signature">
Any page served from getvela.app or one of its subdomains — or by whoever
controls the domain in future — can ask your keys for a signature, and the system
prompt shows "getvela.app", not the transaction. This is how passkeys work
everywhere. Vela's website forbids its own pages from using passkeys for this
reason. It is also why the extension and self-built apps matter: they carry their
own code, although by default they still fetch descriptors and use services
under getvela.app.
</Callout>

## Point the wallet at your services

Each app has four fields under **Settings → Advanced → Service Endpoints**
(on desktop, **Settings → Service Endpoints**):
chain data, passkey index, Vela relay and fiat rates. Each field shows Vela's
default until you change it; **Reset to Defaults** restores all four. For the
relay, index and chain data, the wallet calls `/api/health` and shows a badge,
green only when the endpoint names the right service and reports
`status: "ok"`. It saves what you type either way — wait for green.

| Service | `service` in `/api/health` |
| --- | --- |
| Relay | `vela-relay` |
| Public-key index | `webauthn-p256-publickey-registry` |
| Chain data | `ethereum-data` |
| Exchange rates | not checked by name — must return a USD-based rate list |

All four apps honour all four fields, and a changed endpoint takes effect on
the next call rather than the next launch: every path — creating a wallet,
signing in, looking up a name for an address — reads the endpoint at the moment
it uses it. On iOS the passkey index can also be changed on the sign-in screen
when the default is unreachable.

(Until September 2026 there were four exceptions to that, the worst of them an
iOS page that showed placeholder values and saved nothing. They are fixed.)

## Run your own relay

<span id="relay"></span>

The relay is [vela-relay](https://github.com/mondaylabsltd/vela-relay) (Rust,
MIT). One deployment serves every chain: the wallet calls
`https://your-relay/<chainId>`. It must be vela-relay — the wallet asks for a
fee quote with a Vela-specific method that generic ERC-4337 bundlers don't
implement.

**What you need**

- Either Docker plus a Redis and an [Iggy](https://iggy.apache.org) server you
  already run, or a Cloudflare account on **Workers Paid** with Node.js and a Rust
  toolchain (with the `wasm32-unknown-unknown` target) on your machine.
- An `OPERATOR_SECRET` (hex, at least 32 bytes). It derives one treasury address
  and a pool of relayer addresses, the same on every chain. Keep it secret: it
  controls the relay's funds.
- Gas on every chain you want to serve: send the chain's coin (pathUSD on Tempo)
  to your treasury address. The treasury tops up the relayers.

**Docker**

```sh
git clone https://github.com/mondaylabsltd/vela-relay
cd vela-relay
cp .env.example .env
# in .env: VELA_RELAY_IGGY_URL, VELA_RELAY_REDIS_URL, OPERATOR_SECRET,
# VELA_RELAY_CHAIN_DIRECTORY_URL if you run your own chain data,
# and VELA_RELAY_IMAGE set to a release image you trust (see docs/docker.md)
docker compose pull relay
docker compose up -d --no-build
curl --fail http://127.0.0.1:4567/readyz
```

Either works: the published image is quickest, and `docker compose up --build`
builds the same thing from the source you can read. Without Docker,
`cargo run --release --bin vela-relay` runs it directly.

**Cloudflare Workers**

```sh
cd vela-relay/vela-relay-cf
npx wrangler queues create vela-relay-ops
npx wrangler queues create vela-relay-dlq
npx wrangler secret put OPERATOR_SECRET
# own chain data: add "VELA_RELAY_CHAIN_DIRECTORY_URL" under "vars" in wrangler.jsonc
npx wrangler deploy
```

**Check it**

```sh
curl https://your-relay/api/health        # {"service":"vela-relay","status":"ok",…}
curl https://your-relay/v1/treasury/100   # your treasury address on Gnosis, and whether it needs gas
```

Then put `https://your-relay` in the **Vela relay** field.

**Know this**

- The fee the wallet pays goes to your treasury. The wallet computes it the same
  way whichever relay you use (see [networks & fees](/docs/networks-and-fees)).
- A custom network you added before changing the relay keeps the relay address
  it was added with.
- The relay reads each chain's details and the stablecoins it accepts from a
  chain directory: `ethereum-data.getvela.app` unless you set
  `VELA_RELAY_CHAIN_DIRECTORY_URL` to [your own](#chain-data). It needs vela-relay v0.9.6 or later; older builds always read Vela's copy.

## Run your own public-key index

<span id="index"></span>

The index is [p256-index](https://github.com/mondaylabsltd/p256-index) (Rust, MIT).
When a wallet is created, it checks every key's proof, then writes the group to
the **registry contract** on Gnosis and pays the gas. Keep using the existing
registry at `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9`: it has no owner, any
funded address can write to it, and every Vela app reads it directly. A
registry of your own would be invisible to them.

**What you need**

- Docker with Redis and Iggy (the server), or a Cloudflare account (the Worker
  version, whose own README notes its on-chain write has not been tested end to
  end yet).
- A Gnosis private key with xDAI. Registering a wallet costs about 1.1M gas with
  one key and about 3.6M with seven.
- These settings:

```dotenv
P256_INDEX_IGGY_URL=iggy+tcp://user:password@iggy.example:5100
P256_INDEX_REDIS_URL=redis://redis.example:6379/0
P256_INDEX_CONTRACT_ADDRESS=0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9
P256_INDEX_DOMAIN_REGISTRY=0x5266DfF591B9F9EecfEdb8E7EfEf6c687854edaf
PRIVATE_KEY=0x…
```

`P256_INDEX_DOMAIN_REGISTRY` is the one people miss: without it the server
hands out challenges the contract rejects, and every registration fails. It is
in `.env.example`, and it must match the deployed contract's own
`DOMAIN_REGISTRY` — from registry VERSION 12 the challenge domain is fixed at
deployment, so it does not move when the contract is redeployed.

**Run and check**

```sh
git clone https://github.com/mondaylabsltd/p256-index
cd p256-index
cargo run --release -p p256-index-server
curl https://your-index/api/health   # "service":"webauthn-p256-publickey-registry","status":"ok"
```

The server listens on plain HTTP (port 11256 by default); put a TLS proxy in
front of it, since the wallet only accepts `https://` endpoints. `docker build
-f p256-index-server/Dockerfile .` works from a clean clone too; copy
`.env.example` to `p256-index-server/.env` first if you use Compose.

**If no index answers at all**, existing wallets still work: on sign-in the app
reads the registry contract on Gnosis (then Ethereum) through your RPC nodes. A
wallet with a single key can even be rebuilt from two signatures with no
registry involved. Creating a new wallet does need an index, because something
has to pay for the registration.

## Run your own chain data

<span id="chain-data"></span>

Chain data is [ethereum-data](https://github.com/atshelchin/ethereum-data)
(MIT): static JSON and images for about 2,600 networks and their tokens, plus
the ERC-7730 descriptors Vela uses to explain transactions.

```sh
docker run -d --name ethereum-data -p 3000:3000 --restart unless-stopped \
  ghcr.io/atshelchin/ethereum-data:latest
curl http://localhost:3000/api/health   # "service":"ethereum-data","status":"ok"
```

Its README also covers building from source and deploying to Cloudflare. Serve
it over HTTPS and put the address in the **Chain data** field.

The relay reads these files too, including a Vela-specific field (the `stables`
list decides which stablecoins can pay fees). Point it at your copy with
`VELA_RELAY_CHAIN_DIRECTORY_URL=https://your-chain-data`; it caches each
network's entry for an hour.

## Run your own exchange rates

<span id="exchange-rates"></span>

[vela-currency](https://github.com/mondaylabsltd/vela-currency) (MIT) republishes
the European Central Bank's daily rates. It needs no keys.

```sh
docker run -d -p 8080:8080 -v rates-data:/data ghcr.io/mondaylabsltd/vela-currency:latest
curl "http://localhost:8080/v2/rates?base=USD"
```

Put `https://your-host/v2/rates?base=USD` in the **Fiat rates** field. Any
Frankfurter-compatible service works too. Keep `?base=USD`: every conversion
assumes it.

## Build the apps yourself

<span id="web-app"></span>

All the apps are in [one repository](https://github.com/mondaylabsltd/vela-wallet)
(MIT). The README lists each app's build steps; the short version:

| App | Build | Signs for your existing getvela.app wallet? |
| --- | --- | --- |
| Browser extension | `cd app-web/vela-wallet && pnpm install && pnpm build:extension`, then load `extension/dist` unpacked at `chrome://extensions` | Yes, with any key |
| Web wallet | `cd app-web/vela-wallet && pnpm install && pnpm build`; deploys as a Cloudflare Worker | No — on your domain it is a different wallet (see above) |
| Desktop | `cd app-desktop/vela-wallet && cargo run` (packaging scripts in its README) | Yes, with a phone by QR or a USB security key |
| Android | Generate the core bindings, then `./gradlew :app:installDebug` | Yes, with a phone by QR or a USB security key |
| iOS | `./rust/scripts/build-ios-xcframework.sh`, then build in Xcode with your own team | Yes, with a phone by QR or a USB-C / Lightning YubiKey (firmware 5.8 or later) |

A self-built app's "this device" passkey won't work for getvela.app wallets:
Apple and Google only let apps signed by Vela use `getvela.app` passkeys.

## Add a network Vela doesn't ship

Vela runs on any EVM chain that has the P-256 precompile and the standard
contracts it checks for. [Chain setup](/chain-setup) tells you what a chain is
missing and deploys what anyone can deploy; [networks & fees](/docs/networks-and-fees)
explains the requirements. The check includes the two contracts a wallet with
more than one key needs, and marks them as such — a chain without them still
runs a one-key wallet.

## What still points at Vela after all this

If you replace everything above, these remain:

- **The authenticator directory** that names security-key models — cosmetic;
  the apps fall back to a generic name.
- **getvela.app's association files**, which the store apps need for "this
  device" passkeys. A phone or security key doesn't need them.

And these are not Vela's: public selector databases, Apple's and Google's
phone-sign-in tunnels, and whichever RPC providers you choose.

Next: [the signing page you can run yourself](/docs/clear-signing-self-host).
