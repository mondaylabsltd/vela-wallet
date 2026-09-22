---
title: "Pulse wallet is shutting down: move your funds before August 21"
description: "Pulse offers no key export, so every asset needs an on-chain transfer out before the app goes dark. A practical checklist, honest destination options, and three questions for picking your next wallet."
date: 2026-08-08
author: Shelchin
tags: [security, shutdowns, passkeys]
---

> **Corrections, 22 September 2026.** An entry-level Ledger (Nano S Plus) cost $59 in Ledger's US store in August 2026, not "around $44". And Family wallet is winding down (it keeps working until April 2027 and then moves users to account.aave.com), so calling it "shut down" overstated it. The deadline for Pulse has passed; the checklist and the three questions at the end still apply to any wallet that closes.

Pulse posted a [shutdown notice](https://x.com/PulseSocialFi/status/2084884180132114531) (pinned on their
X account). Three things in it matter: the service is being discontinued for
financial reasons; after August 21, 2026 you will no longer be able to access
your wallet through the app; their support team is still answering withdrawal
questions until then.

Quick disclosure before anything else: you're reading this on the blog of a
passkey wallet, so we have obvious skin in this game. For that reason, this
post recommends other people's products and doesn't pitch ours. I keep notes
on wallet shutdowns, and Pulse's case has one detail I think people are
underestimating, so I cleaned those notes up into this post. Every claim has a
link; if I got something wrong, tell me and I'll fix it.

## Why this shutdown is different from most

When [Leap Wallet shut down](https://www.theblock.co/post/396325/cosmos-ecosystems-leap-wallet-is-shutting-down)
in May, the official guidance was: export your seed phrase, import it into
Keplr or MetaMask, done. Same addresses, same assets, nothing moves on-chain.
That's how seed phrase wallets die: the account is the phrase, every wallet
app understands it, so a shutdown is close to a non-event for users.

Pulse is a passkey wallet with no seed phrase. Its
[notice](https://x.com/PulseSocialFi/status/2084884180132114531) says only "transfer all assets to
another wallet that you control", and as of August 8 neither the announcement
nor the [website](https://pulse.social/) mentions any way to export keys. If
that doesn't change, every asset you hold needs an actual on-chain transfer
out, and the only interface for making those transfers is the app that stops
working on August 21.

The precedent for this architecture is not reassuring. When Coinbase retired
the Web3 wallet built into its main app last year (MPC-based, also no
exportable seed),
[Forbes covered](https://www.forbes.com/sites/chrisgroshong/2025/10/09/coinbase-users-struggling-to-access-assets-the-web3-wallet-sunset-saga/)
users who missed the migration window, got stuck in the official migration
tool, and in some cases ended up with discounted compensation instead of
their assets.

So the entire advice of this post compresses to one sentence: don't let this
run to the last few days.

## The checklist

1. Open Pulse and write down every asset: token, amount, and which network
   it's on. If you can't tell the network, ask support rather than guessing.
2. Pick a destination wallet (next section) and get a receiving address.
3. Send a small test amount first, say 5 USDC. Confirm it arrives, then move
   the rest.
4. Check the network on every transfer. USDT exists on many chains, and
   transfers sent over the wrong one are usually unrecoverable.
5. When done, reconcile the balances in your new wallet against the list from
   step 1.

Leave buffer time. Support will be overloaded in the final days, and chains
occasionally congest.

## Where to go

There's no single right answer, so here are the options with their costs, and
you can match them to your situation.

A hardware wallet. Entry-level Ledger is around $44, Trezor Safe 3 is $59.
Makes sense for larger amounts. The cost: you're learning the seed phrase
routine, and you're waiting for shipping, which is another reason not to
procrastinate.

A mainstream software wallet (MetaMask, Rabby, Trust, etc.). Free, usable in
minutes. The cost: you're back to writing down and guarding 12 words, which
is probably the thing you picked Pulse to avoid.

An exchange. For small stablecoin balances, if you genuinely don't want to
deal with any of this, it's an honest option. Just be clear that you're
trading "the company shut down" risk for "someone else holds my money" risk,
not eliminating risk.

Another passkey wallet. The category is bigger than Pulse. Before picking one
(including ours — this is the part where you should trust us least), I'd
figure out one thing: if this company also shuts down, can my account still
be reached from software they don't control? That question deserves its own
post, so I wrote [one](/blog/passkeys-are-not-the-problem) that argues
entirely from public cases.

## While you're at it: how to pick a wallet

By my notes, Pulse is the sixth consumer wallet to shut down this year, after
[Family](https://avara.xyz/blog/the-future-of-family-wallet),
[Magic Eden Wallet](https://help.magiceden.io/en/articles/13885539-magic-eden-wallet-deprecation-overview),
[Leap](https://www.theblock.co/post/396325/cosmos-ecosystems-leap-wallet-is-shutting-down),
[HaHa](https://www.cryptopolitan.com/2026-year-of-crypto-shutdowns/), and
[Ctrl](https://cointelegraph.com/news/ctrl-wallet-shutdown-security-exploit).
The list will probably keep growing, so next time you choose a wallet, three
questions are worth asking first:

If this company disappears, what specific software do I use to get my assets
back? It only counts as an answer if you can name it ("import my seed into
MetaMask" counts).

Is the account a standard that multiple apps understand, or a custom
implementation only this one app recognizes?

Is the backend open source? Open source doesn't mean the product is good. It
means the product can outlive the company.

That's all. The deadline is August 21.
