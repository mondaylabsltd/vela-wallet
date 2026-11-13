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

<Callout type="info" title="Last verified November 2026">
Checked against the code at commit <code>fa6083c87</code> (the development
branch). The page's guarantees per kind of wallet are below; the full threat
model is <a href="https://github.com/mondaylabsltd/vela-wallet/blob/main/docs/SECURITY-MODEL.md">SECURITY-MODEL.md</a>.
</Callout>

## Where it runs

The official page is served from **sign.getvela.app**. Every Vela app can send
it a request: the web wallet, the Chrome and Safari extensions, and the desktop
(macOS, Windows, Linux), iPhone and Android apps. You check the request and sign
with your passkey on the page, and the page hands the signature back.

- **The web wallet** opens the official page **from the very bytes it checked**:
  it fetches the page, hashes it, and opens those bytes in a new window, so the
  host is never asked a second time and cannot swap the page after the check.
- **The extensions and the phone and desktop apps** open the page at its address
  (the apps in the system browser, where the app cannot reach in). The page
  installs a small service worker that, from the second visit on, serves each
  version only as bytes whose hash is that version. The first visit is trusted
  as it arrives, and a browser that keeps no service worker (a private window,
  cleared site data) is not pinned. The apps also re-check the page at the tap
  when their last check is more than 10 minutes old.

It is opt-in. You choose it as the way you sign when you create a wallet or sign
in, and from then on every signature for that wallet on that device goes through
it. It can also create the wallet's keys. On `sign.getvela.app` it uses the same
`getvela.app` passkeys as the apps.

<Callout type="info" title="Tested so far">
Recorded end-to-end runs against the published page: Android, and Windows 11
(creating a wallet and signing in). The web wallet's opening from the checked
bytes is signed end to end in an automated browser test (Chromium). The macOS,
Linux and iPhone apps use the same wiring; none of them has a recorded full run
yet, and a page-signed transaction on a phone has no automated test yet.
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
  the bytes that arrived or nothing — so an unlimited approval or permit (2^128
  or more, the same line as the apps; pages before November 2026 drew it at
  2^200, 2^152 for Permit2) is shown in red with that reason and can be signed as
  it stands; an on-chain cap is chosen on the wallet's own approval screen,
  before the request gets here. An approval for a whole NFT collection is
  refused. Permit2's witness transfers are read as the transfers they are;
  `PermitBatch` and the batch transfer forms are still shown as generic typed
  data.
- **It refuses what it cannot stand behind:** `eth_sign`, a method it does not
  know, a token sent to its own contract, an operation it cannot read, an
  operation with no expiry (every one it signs shows "Valid until"), and a
  sign-in whose challenge the requester supplied.
- **It refuses what would hand over your account,** by the rule the apps apply:
  a call from your account to one of its own owner, module, guard or fallback
  functions, inside a batch too; a `delegatecall`, except into Safe's MultiSend
  contract, which bundles an operation's calls; and a `SafeTx` signature. It also
  refuses a site's `execTransaction`, `approveHash` or owner-management call to
  **any** other Safe, and a `SafeMessage` signature under another Safe's domain:
  the page cannot read the chain to learn which Safes your account owns, so it
  refuses them all (the apps clear a Safe your account does not own). It checks
  every call in the operation the app assembled, not only the calls the site
  asked for.
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

## What it guarantees, and what it does not

That depends on which domain your wallet's keys belong to.

- **A wallet made in the apps or on the official page** (keys for
  `getvela.app`). The page gives **display assurance**: a wallet interface that
  is wrong or tampered with cannot make the page show you one thing and sign
  another, because the page decodes and computes the digest itself. It is **not
  an authority boundary**: the web wallet, the extensions and the apps can all
  ask the same passkey for a signature directly, so a fully compromised app does
  not need the page. On the web, the check of the page is the wallet's own code,
  so a compromised wallet could also open a page of its choosing. Settings says
  this where you choose the page.
- **A wallet made on a self-hosted page** (keys for that page's domain). Only
  pages on that domain can use its keys, so the apps cannot sign without the
  page: here the page is the boundary — provided its host is honest on the first
  visit, before the service worker pins it, since the host also chooses the
  service worker it serves.

A kind of wallet whose keys **only** the page can use, on a domain no app or
wallet origin can reach, would make the page the boundary for everyone. It is
designed but not built.

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

Every app fetches the published version it will open, hashes it and compares
it with the versions built into it, and **refuses to open** a page on
sign.getvela.app that doesn't match. A page on your own address whose version
the app doesn't know is shown to you once, with its hash, for you to trust or
not. The list built into the app is the trust root; the host's `index.json`
only says which versions it still serves.

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
