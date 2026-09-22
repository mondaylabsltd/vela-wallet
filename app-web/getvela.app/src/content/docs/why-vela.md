---
title: Why we built Vela
description: The long version — where you are supposed to keep twelve words, what passkeys changed, what we could not accept in the wallets we were already using, and the trade-off we chose instead.
---

<script>
	import Callout from '$lib/components/Callout.svelte';
</script>

# Why we built Vela

We didn't set out to build another wallet. We started with a question we could
never answer cleanly:

> Where are you supposed to keep twelve words?

## The honest answer is a screenshot

Put them in Notes and you're one stolen phone away from trouble. Write them on
paper, and now you're thinking about fire, water, moving apartments, roommates,
trash bags, and whether future-you will remember where "the safe place" was.

The honest answer, for a lot of people, is a screenshot in the camera roll.
Everyone knows it's wrong. They do it anyway — because the "right" answer is too
hard to live with.

A seed phrase is a secret that has to survive decades of ordinary life without
ever being copied, photographed, typed into the wrong box, or read aloud to
somebody helpful on the phone. That is not a hard problem for a careful person.
It is a hard problem for a person.

## Then passkeys changed what a wallet could feel like

We used [Base Account](https://account.base.app) every day, and signing with
Face ID felt obvious in a way seed phrases never did — less like handling
hazardous material, more like using the rest of the internet.

But the more we used it, the more we hit edges we couldn't ignore:

- a **recovery key generated in a browser** that you just had to trust,
- **no custom networks**,
- **no way to host it ourselves**,
- and the quiet problem that was the biggest one: **if the service disappeared,
  the wallet disappeared with it.**

So we built the version we wanted to depend on.

## What Vela actually is

Vela is **a passkey wallet you can fully own.**

Your passkey stays where your device already protects it — iCloud Keychain,
Google Password Manager, or a hardware security key you hold. When you sign a
transaction, Vela asks your device to sign it; your device signs and sends back
just the signature. Vela never sees the key itself.

Most wallets still have a dangerous moment, even if it's brief: words on a
screen, a seed phrase in memory, a recovery key sitting in a browser tab. Vela is
designed so that moment never exists.

<Callout type="info" title="Not a promise — an architecture">
We can't access your keys. Not "we promise not to" — there is no code path in
Vela that could; WebAuthn doesn't allow it. The wallet is a
<a href="/docs/account-contract">Safe smart account</a> operated by a signature
your device produces and we only ever receive. What the app you sign with does
decide is <em>what</em> your key is asked to sign — which is why the
<a href="/docs/whitepaper">threat model</a> spends so long on it.
</Callout>

We made Vela **open source** so you can check that for yourself, and
**self-hostable** so that an existing wallet keeps working without our company's
servers — with one limit, the domain your passkeys belong to, which the
[self-hosting guide](/docs/self-hosting) explains along with the ways around it.
And we built on unmodified
[Safe contracts](https://github.com/safe-fndn/safe-smart-account/tree/release/v1.4.1)
because the boring, battle-tested path is the right one when people's money is
involved — the same contracts already securing billions on-chain.

## The trade-off we chose

There's still a trade-off, and it would be dishonest to bury it.

With Vela, your Apple or Google account matters, because that's where a synced
passkey lives. Lose that account, or delete the passkey, and there's no seed
phrase, no support reset, no back door.

But every self-custodial wallet asks you to choose which risk you'd rather live
with. A seed phrase can be copied, screenshotted, phished, or typed into the
wrong site at 1 a.m. A passkey is different: there are no words to reveal, no
secret to paste, and no fake site that can trick you into handing it over. Your
browser offers it only to pages on the real domain.

And the choice isn't binary. A wallet can be created with **up to seven
signers**, any one of which can sign on its own — passkeys on different devices,
a nearby phone you scan, or a USB/NFC security key. If you'd rather your wallet
didn't depend on a platform account at all, you can use hardware security keys
only — two of them, since a wallet can't rest on a single key that syncs nowhere.
The one condition is timing: your address is derived from the full set of keys,
so they're chosen when you create the wallet.

<Callout type="warning" title="What this does not buy you">
Extra signers are a way back in, not a second lock. Because any single key can
sign, adding a hardware key protects you against <em>losing</em> access — it does
not stop somebody who has already taken over one of your keys. That's the honest
shape of 1-of-n.
</Callout>

## That's why Vela exists

A wallet with no seed phrase to hide, no recovery key to trust, and no company
you have to hope will stay around forever.

If you want to check the claims rather than take them: the
[whitepaper](/docs/whitepaper) has the architecture,
[Audits & known issues](/docs/security-audits) has every contract we depend on
and what has and hasn't been audited, and all of the code is
[on GitHub](https://github.com/mondaylabsltd/vela-wallet).

Next: [install Vela](/docs/install).
