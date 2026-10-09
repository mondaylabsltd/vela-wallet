---
title: "Why a network without the P-256 precompile can't run Vela"
nav: "No P-256 precompile, no Vela"
description: "Passkey signatures are verified by the P-256 precompile at 0x100, and that address is part of every Vela address. A chain without it can't run Vela."
facts:
  - "Verifier address | 0x100"
  - "On Ethereum | EIP-7951, since Fusaka (December 2025)"
  - "On rollups | RIP-7212, same interface"
  - "Fallback verifier | None"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - specs/080-site-content-accuracy/claim-ledger.md#L16
  - rust/crates/vela-core/i18n/locales/en/about.json#L10
  - rust/crates/vela-core/src/safe.rs#L44-L53
  - rust/crates/vela-core/src/safe.rs#L172-L205
  - rust/crates/vela-core/src/app/network_admin.rs#L173-L184
  - rust/crates/vela-core/src/app/network_admin.rs#L2229-L2236
  - rust/crates/vela-core/src/app/network_admin.rs#L2293-L2314
  - rust/crates/vela-core/src/app/network_admin.rs#L2350-L2356
  - app-web/getvela.app/src/lib/chain-setup/verdict.ts#L1-L14
  - app-web/getvela.app/src/lib/chain-setup/rpc.ts#L100-L123
  - specs/060-arc-network-integration/research.md#L5-L27
  - app-web/vela-wallet/src/lib/settings/live.ts#L265-L272
  - app-web/vela-wallet/src/lib/settings/live.ts#L331-L345
  - app-web/getvela.app/src/lib/i18n/messages/en.ts#L807-L811
docs: networks-and-fees
related:
  - networks/arc-gas-floor
draft: true
---

Every Vela transaction carries a WebAuthn signature from one of your keys, made with the P-256
curve. On-chain passkey verification uses the P-256 precompile at `0x100`. It's EIP-7951 on
Ethereum, live since the Fusaka upgrade in December 2025, and RIP-7212 on rollups. The interface is
the same.

There's no fallback verifier, and `0x100` is part of every address. Your wallet's setup data names
`0x100` as the verifier for each of its keys, and your address is computed from that setup data. A
verifier at any other address would give every wallet a different address.

So nothing deployed later can fix a chain that lacks the precompile. The precompile isn't something
anyone can deploy, and only the chain's own operators can add it.

## What you see

- When you add such a network in the wallet, the check marks it "Incompatible", with the "P-256
  precompile" row failed.
- The [chain setup page](/chain-setup) says "Vela cannot run here" and offers no deployment steps,
  because none of them would lead to a working wallet.

## How Vela checks for it

A native precompile has no code, so finding no code at `0x100` doesn't mean it's missing. On Arc,
on 17 September 2026, `eth_getCode` at `0x100` returned `0x` while the precompile worked.

The wallet first sends `0x100` a signature it knows is valid, and expects the answer 1. If that
fails, it accepts contract code at `0x100` instead, for a chain that ships the verifier as a
contract there. The chain setup page runs the same two checks.
