---
title: "The Trusted Signer vs signing in the app"
nav: "Trusted Signer vs the app"
description: "With in-app signing, the app shows the transaction and asks your key. With the Trusted Signer, a separate page checks the transaction first."
facts:
  - "In the app | The app checks and signs"
  - "Trusted Signer | A separate page checks first"
  - "Available in | Desktop, iPhone and Android apps"
  - "Costs | An extra step on every signature"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/trusted_signer.rs#L14-L19
  - rust/crates/vela-core/src/trusted_signer.rs#L469-L476
  - rust/crates/vela-core/src/trusted_signer/integrity.rs#L136-L147
  - specs/075-clear-signer-channel/spec.md#L57-L61
  - specs/080-site-content-accuracy/claim-ledger.md#L23
  - specs/080-site-content-accuracy/claim-ledger.md#L28
  - specs/080-site-content-accuracy/claim-ledger.md#L33
docs: clear-signing-self-host
related:
  - keys/four-ways-to-sign-in
  - signing/no-reject-button
draft: true
---

When you sign in the app, the app decodes the request, calculates the data to be signed, and asks
your key for a signature. The passkey prompt shows only "getvela.app", not the transaction, so you
rely on the app to show you the transaction correctly. The [Bybit attack](/docs/bybit-attack) used
this kind of gap: the interface showed one transaction while the signers signed another.

## How the Trusted Signer works

You still start in the app, where you can set an approval limit or a speed. The app then opens a page
at sign.getvela.app, which:

- decodes the transaction and draws the identicons
- calculates the data to be signed
- refuses requests that would hand control of the wallet to someone else
- asks your browser for the passkey signature

The app accepts the result only if it's a signature from one of the wallet's keys over exactly the
data the app expected. The page has no network access, and each version is published at its own
address.

## What it protects against

The Trusted Signer protects against a wallet app, or an update to it, that shows you something
different from what it signs. It doesn't protect against:

- malware that controls your computer or browser
- a harmful request that's displayed correctly. An unlimited approval is shown in red, but you can
  still sign it.
- a replaced signing page. The desktop app compares the page with the versions it ships with, but
  only logs a mismatch. The phone apps don't check yet.

The app can still reach your getvela.app passkeys without the page. If you use the Trusted Signer
and a passkey prompt appears without the page, don't approve it.

## Where you can use it

The Trusted Signer is available in the desktop, iPhone and Android apps, but not in the web wallet or
the browser extension. Each signature takes an extra step to the page and back, and approval limits
and speed have to be set in the app before the page opens.
