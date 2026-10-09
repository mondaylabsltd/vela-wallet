---
title: "How Vela turns a transaction into words before you sign"
nav: "How a transaction is decoded"
description: "A transaction is a block of numbers. Before you sign, Vela works out what they mean and shows the action, amounts and addresses, or warns you when it can't."
facts:
  - "Description format | ERC-7730"
  - "Marked Verified | Built-in, or an identical fetched copy"
  - "If nothing fits | Explicit blind-signing warning"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L30
  - specs/080-site-content-accuracy/claim-ledger.md#L33
  - specs/080-site-content-accuracy/claim-ledger.md#L39
  - rust/crates/vela-core/src/app/clear_signing.rs#L1-L50
  - rust/crates/vela-core/src/app/clear_signing.rs#L157-L175
  - rust/crates/vela-core/src/app/clear_signing.rs#L730-L760
  - rust/crates/vela-core/src/app/clear_signing.rs#L2833-L2911
  - rust/crates/vela-core/src/app/clear_signing.rs#L2929-L3086
  - rust/crates/vela-core/src/app/clear_signing.rs#L3692-L3725
  - rust/crates/vela-core/src/app/clear_signing.rs#L4976-L4996
  - rust/crates/vela-core/src/app/self_call_guard.rs#L1-L24
  - rust/crates/vela-core/i18n/locales/en/componentsUi.json#L66-L114
  - app-web/getvela.app/src/content/docs/clear-signing.md#L28-L55
docs: clear-signing
related:
  - how-it-works/what-your-passkey-signs
  - interface/orange-means-money-moves
  - trusted-signer/trusted-signer-vs-in-app
order: 12
draft: true
---

A call to a contract is sent as calldata: four bytes that identify the function, followed by its
arguments as numbers. Signing that without being able to read it is called blind signing.

Before you sign, Vela matches the calldata against a description of the function and shows you the
action (Send, Approve, Swap), the amounts in the token's own units and the addresses involved. What
you see is what you sign: the confirm screen is decoded from the exact transaction that is signed.

## Where the description comes from

The descriptions use ERC-7730, a standard file format that says what each of a contract's functions
does and how to show its arguments. Vela tries these sources in order and uses the first that fits:

1. A descriptor built into the app, for common contracts such as the Uniswap routers, WETH and the
   Aave v3 pool.

2. A descriptor for that exact contract, fetched from Vela's chain-data server.

3. The shapes of the token standards, built into the app: ERC-20 tokens, and ERC-721 and ERC-1155
   NFTs. ERC-20 and ERC-721 share `approve` and `transferFrom`, so Vela asks the contract whether
   it's an NFT before choosing.

4. Descriptors for other standards, such as ERC-4626 vaults, fetched from the same server.

5. A public database of function selectors. Vela decodes the arguments generically and labels the
   result best effort.

If none of these fits, Vela shows an explicit blind-signing warning.

## What "Verified" means

Only a descriptor built into the app, or a fetched one identical to the built-in copy, is marked
Verified. Changing it would mean changing the app.

Fetched descriptors are not cryptographically authenticated. They're unsigned files from a server
address you can change. Vela still uses them, with a line saying the description came from Vela's
descriptor service and nothing authenticated it.

## Amounts and risk

Vela takes a token's decimals from its built-in token list or reads them from the token contract. If
neither works, it shows the amount as if the token had 18 decimals and marks it unverified.

Each result gets a risk level. Approvals and permits are marked caution, and an unlimited approval
is marked danger, in red. A partial decode, an unverified amount, an expired deadline or a
best-effort decode never shows a level lower than caution.

## What Vela refuses

A dApp can request a call from your Safe to itself, such as `enableModule`,
`addOwnerWithThreshold`, `setFallbackHandler` or `setGuard`. Any one, signed once, hands over the
account. Vela refuses them, inside a batch or a `MultiSend` too, along with any leg carrying a
`delegatecall` and any `SafeTx` typed-data signature. They're shown as refused and can't be signed.

The decoding runs in the same app that asks you to sign. A separate place to check and sign exists
only if you chose the Trusted Signer.
