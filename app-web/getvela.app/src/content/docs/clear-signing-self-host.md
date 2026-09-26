---
title: Trusted Signer
description: A single-file page at sign.getvela.app that decodes a request and signs it with your passkey on its own — what it checks, which apps use it, and how to rebuild or run your own copy.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Trusted Signer

Vela decodes every transaction before you approve it, and that decoding is
honest work — but it is work done by the same app that built the transaction.
If the app or the way it reaches you is tampered with, it can show you one
thing and sign another. That is exactly what happened to
[Bybit](/docs/bybit-attack).

The Trusted Signer exists to split that in two: the app only hands over the
request, and the check and the signature happen on a separate page — one you
can read end to end, rebuild byte for byte, or run yourself.

## Where it runs

The official page is served from **sign.getvela.app**. The desktop (macOS,
Windows, Linux), iPhone and Android apps can send it a request: the app opens the
page in a browser tab with the request in the link, you check it and sign with
your passkey there, and the page hands the signature back to the app through a
`velawallet://` link. The web wallet can't use it.

It is opt-in. You choose it as the way you sign when you create a wallet or sign
in, and from then on every signature for that wallet on that device goes through
it. It can also create the wallet's keys. On `sign.getvela.app` it uses the same
`getvela.app` passkeys as the apps.

<Callout type="info" title="Tested so far">
Recorded end-to-end runs against the published page: Android, and Windows 11
(creating a wallet and signing in). The macOS, Linux and iPhone apps use the same
wiring; none of them has a recorded full run yet.
</Callout>

## What it does before it signs

- **It decodes the request itself.** What the call does, to whom, and for how
  much, from the calldata — including calls nested inside a batch.
- **It signs only a digest it computed.** EIP-191, EIP-712, SafeOp and
  SafeMessage digests are computed in the page, never taken from the requester;
  tests check the SafeOp and SafeMessage digests against `vela-core`, the code the
  wallet uses, and the app refuses a signature over any digest other than the one
  it computed itself.
- **It checks the transaction is the one that was requested.** The call the
  site asked for has to actually be inside the operation being signed, or the
  page refuses.
- **It says when an approval is unlimited.** It can't change an amount — it signs
  the bytes that arrived or nothing — so an unlimited approval or permit (2^128 or
  more on this page) is shown in red with that reason and can be signed as it
  stands; an on-chain cap is chosen on the wallet's own approval screen, before
  the request gets here. An approval for a whole NFT collection is refused.
- **It refuses what it cannot stand behind:** `eth_sign`, a method it does not
  know, a token sent to its own contract, an operation it cannot read, and a
  sign-in whose challenge the requester supplied.
- **It shows the account's address and an identicon computed on the page.**
  Recipients and contracts are never named from the request — only the page's own
  reviewed table can name a contract. The account's own name, which the app sends
  so you can pick the right passkey, is shown beside its address.
- **It asks for user verification** (your fingerprint, face or PIN) on every
  signature.

## What it deliberately does not have

- **No editors.** The request is fixed when it arrives: you sign it or you do
  not. A fee picker or an allowance editor would rewrite the calldata, which is
  the disease this page exists to prevent.
- **No network access.** The page is one file whose content security policy
  (`default-src 'none'`) is inside its own bytes, so it cannot fetch anything,
  open a connection or load an image. The only thing that leaves it is its
  answer, when it follows the callback link in the request (`velawallet://` when a
  Vela app asked). Token logos are drawn as letters.

## What the app checks in return

The app does not trust the page either. It accepts a signature only when the
signed challenge is the digest **the app computed**, user verification was
done, the key is one of your wallet's, and the P-256 signature verifies under
that key.

## Every published version, checkable

Each version is built from `app-web/trusted-signer/src/` into a single file,
reproducibly — Bun and Node produce the same bytes — and published at its own
address, `sign.getvela.app/b/<sha256>/sign.html`, next to every earlier version.
The list is at `sign.getvela.app/index.json`.

```sh
cd app-web/trusted-signer
node samples/build-single.mjs --check   # rebuilds a version listed in dist/
curl -sL https://sign.getvela.app/b/<sha256>/sign.html | shasum -a 256
```

When it starts, the desktop app fetches the published version it will open,
hashes it and compares it with the versions built into it. The verdict is only
logged, and a page that doesn't match is opened anyway. The phone apps don't
check yet.

## Run your own copy

Settings keeps the address of the page your apps open, so you can point it at
your own deployment: any HTTPS address, or `localhost` for testing. Build it with
`bun samples/build-single.mjs` (or `node`) and copy `dist/` to your host.

A copy on your own domain signs with passkeys created for **that** domain, not
with the `getvela.app` passkeys — so it is a way to create and use a wallet
whose keys live under your domain, not a way to sign for an existing
`getvela.app` wallet. All of a wallet's keys share one domain.

The code, and the scripts that build and check it, are in
`app-web/trusted-signer/`.
