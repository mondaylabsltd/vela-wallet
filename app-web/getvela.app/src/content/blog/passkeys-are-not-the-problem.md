---
title: Pulse users aren't trapped because of passkeys
description: "The wrong lesson from the Pulse shutdown is that seedless wallets don't work. Whether you can recover your assets depends on how many programs recognize your account, not on whether your key is twelve words or a passkey."
date: 2026-08-08
author: Shelchin
tags: [passkeys, security, design]
---

> **Corrections, 22 September 2026.** Two parts of this post overstated the case. Family did not tell users to export a phrase; it is winding down and moving users to Aave's account app. And in "group two", Dharma's and Loopring's own notices said users could still reach their contract accounts afterwards — through an open-source interface or by calling the contracts directly — so "only recognized by its own software" was too strong. Separately, a Safe owned by a passkey can be *read* by any Safe interface, but only software that can request a signature for that passkey's website can sign for it; for Vela, that means the tools listed in the [self-hosting guide](/docs/self-hosting#if-getvela-app-disappears). The framework — ask what else can reach your account — stands.

Pulse wallet [announced it's shutting down on August 21](https://x.com/PulseSocialFi/status/2084884180132114531),
with no key export path offered. Users have to move their assets out, transfer
by transfer, before the deadline. (If you still have funds in Pulse, the
[practical guide](/blog/pulse-wallet-is-shutting-down) comes first. This post
is the argument.)

The take you're going to see a lot in the coming days: "see, seedless wallets
don't work, seed phrases were right all along." It sounds reasonable, and it
blames the wrong variable. Since this take resurfaces every time a wallet
dies, it seems worth taking apart properly.

## Decompose "can I get my money back" into two questions

Whether you can recover your assets after a wallet's operator disappears
depends on two independent things:

(a) Do you hold the signing power? Private key, seed phrase, passkey: who can
use it.

(b) How many paths exist for exercising that signing power? Besides this
company's software, does any other program in the world recognize your
account?

Seed phrases, passkeys, and MPC are three different answers to (a). But
shutdown outcomes are determined almost entirely by (b).

## Where Pulse got stuck

From public materials, Pulse stores the passkey in the user's own iCloud
Keychain or Google Password Manager
([their website's description](https://pulse.social/)), out of the company's
reach. By that design, after August 21 the key most likely still sits with
the user.

The problem is that no other software recognizes the account that key
controls. The logic for reconstructing the account lives in a closed-source
app and a proprietary backend. App stops, (b) goes to zero.

The key isn't lost. The door is gone.

## Testing the framework against recent shutdowns

Sort the 2025–2026 wallet shutdowns by (b) and the outcomes line up almost
too neatly.

Group one, where (b) is guaranteed by an open standard (seed phrase wallets):
[Leap](https://www.theblock.co/post/396325/cosmos-ecosystems-leap-wallet-is-shutting-down),
[Ctrl](https://cointelegraph.com/news/ctrl-wallet-shutdown-security-exploit),
[Magic Eden Wallet](https://help.magiceden.io/en/articles/13885558-how-to-migrate-your-magic-eden-wallet-before-may-1st),
[Family](https://avara.xyz/blog/the-future-of-family-wallet),
[Reddit Vault](https://support.reddithelp.com/hc/en-us/articles/12525978622740-How-do-I-keep-my-Vault-safe).
The official guidance in every single case was the same sentence: export your
phrase or key, import it into any compatible wallet. Same address, nothing
moves on-chain. BIP39 is a standard implemented by hundreds of programs, and
any one of them dying doesn't matter.

Group two, where (b) was one proprietary path:

[Coinbase's in-app Web3 wallet](https://help.coinbase.com/en/coinbase/trading-and-funding/trade-on-dex/web3-wallet-sunset).
MPC, no exportable seed, official migration tool as the only way out.
[Forbes covered](https://www.forbes.com/sites/chrisgroshong/2025/10/09/coinbase-users-struggling-to-access-assets-the-web3-wallet-sunset-saga/)
users stuck after the window closed, some of whom ended up with discounted
compensation.

[Loopring wallet](https://medium.com/loopring-protocol/loopring-wallet-closure-announcement-287400ff621b).
Smart contract accounts; the closure announcement gave users a deadline to
transfer assets out to other wallets.

[Dharma](https://coinmarketcap.com/academy/article/dharma-to-shut-in-30-days-as-opensea-buys-company).
Smart contract accounts; when it shut down after the OpenSea acquisition in
2022, the company paid all network fees so users could evacuate within the
30-day window. When a company pays for your exit, it's because you can't
leave otherwise.

[Authereum](https://blockspot.io/wallet/authereum/), an earlier case from
2021: same story, withdraw or lose access.

Look at the composition of group two: MPC and contract accounts, big
companies and small teams, custodial-ish and self-custodial. The one thing
they share is that the account was only recognized by its own software.

## The counterexample: passkeys can live in architectures where (b) is healthy

There's no necessary connection between passkeys and "only one path". Safe's
contract accounts support passkeys as signers (see
[Safe's docs](https://docs.safe.global/)), and the same Safe account can be
reached from the official web app, several third-party interfaces, and
command-line tools. Multiple paths, standard contracts.

So a passkey account built on a standard contract that multiple programs
support, with open-source clients and backends anyone can re-deploy, keeps
(b) above one even after its operator shuts down. The path exists
technically. Pulse just didn't take it.

## The real weaknesses of passkeys

So this doesn't read as a defense brief: passkeys have two genuine downsides.
Both are real, and both are unrelated to this shutdown.

One, platform dependence. The key lives inside your Apple or Google account,
and losing that account can mean losing signing power. That moves the single
point of failure from "a piece of paper" to "a platform account"; it doesn't
remove it. Safe's own docs recommend against using a passkey as the only
signer.

Two, moving cost. Switching wallets with a contract account means on-chain
transfers, asset by asset, gas on every one. An EOA imports a phrase and
walks away. This is an inherent minus of the contract account category;
standardization softens it and doesn't eliminate it.

## Conclusion

When choosing a wallet, "does it have a seed phrase" is a bad question,
because it asks about (a). The good questions all ask about (b):

If this company disappears, which specific programs can I still use to reach
my assets? If you can't name them, the answer is none.

Is the account a standard that many programs recognize, or a custom
implementation only this app understands?

Are the clients and backend open source? Open source doesn't guarantee a good
product. It guarantees the product can outlive the company.

Pulse users are paying for the gap between innovation in experience and
standardization of exits. Passkeys didn't create that gap, and going back to
seed phrases won't close it. The biggest mess in group two came from
Coinbase, and it had nothing to do with passkeys.

---

Disclosure: this is the blog of a passkey wallet, so the bias is obvious — we
build one, and our design choices line up with the argument above. That is
exactly why every claim here rests on linked public cases rather than on
anything of ours. If you find an error, tell me.

Further reading: denkeni's
[analysis of the decline of smart contract wallets](https://denkeni.substack.com/p/falling-of-smart-contract-wallet),
starting from Blocto ending its EVM support.
