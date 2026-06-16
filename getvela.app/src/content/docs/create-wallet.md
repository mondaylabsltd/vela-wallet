---
title: Create your wallet
description: Create a self-custodial Vela wallet in under a minute using a passkey — no seed phrase.
---

# Create your wallet

Creating a wallet takes about a minute and a single biometric prompt.

## Steps

1. Open Vela and choose **Create wallet**.
2. Pick a **name** for the wallet. This is stored alongside your passkey's public
   key so you can recognize the account when you sign in on another device. It is
   public, so don't use anything you'd rather keep private.
3. When prompted, **authenticate with your passkey** — Face ID, Touch ID, or your
   device's equivalent. This creates the signing key inside your device's secure
   hardware.
4. That's it. You'll land on your wallet, ready to receive tokens.

There is no seed phrase step, because there is no seed phrase. The key that
controls your wallet is the passkey you just created.

## What just happened

- A key pair was generated in your device's secure element.
- The **public** key (and the name you chose) was published to Vela's passkey
  index so the account can be found again on a new device. The
  [recovery doc](/docs/recovery) covers this in detail.
- The **private** key never left your device and never will. Not even your OS
  shows it to you; you only ever *use* it, with your face or fingerprint.

## Next steps

- [Receive your first tokens](/docs/send-and-receive)
- [Understand networks and fees](/docs/networks-and-fees)
- [Read how passkeys keep this safe](/docs/passkeys)
