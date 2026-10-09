---
title: "RPC provider keys: Alchemy, dRPC and Ankr"
nav: "RPC provider keys"
description: "One API key from Alchemy, dRPC or Ankr covers every built-in network that provider serves. Vela uses it after any RPC URL you set for a network."
facts:
  - "Alchemy | 23 of 24 built-in networks"
  - "dRPC | 23 of 24 built-in networks"
  - "Ankr | 8 of 24 built-in networks"
  - "Stored | On the device, unencrypted"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/network_admin.rs#L76-L77
  - rust/crates/vela-core/src/app/network_admin.rs#L537-L637
  - rust/crates/vela-core/src/app/network_admin.rs#L770-L778
  - rust/crates/vela-core/src/app/network_admin.rs#L3251-L3366
  - rust/crates/vela-core/src/app/rpc_pool.rs#L222-L250
  - app-web/vela-wallet/src/lib/services/rpc-pool-endpoints.ts#L93-L129
  - app-web/vela-wallet/src/lib/services/rpc-providers.ts#L31-L38
  - rust/crates/vela-core/src/storage_catalog.rs#L65-L78
  - app-web/vela-wallet/src/lib/services/storage.ts#L1-L10
  - app-desktop/vela-wallet/src/executor/storage.rs#L100-L106
  - app-ios/VelaWallet/VelaWallet/Core/VelaStore.swift#L127-L129
  - app-android/vela-wallet/app/src/main/java/app/getvela/wallet/core/data/VelaStore.kt#L137-L138
  - rust/crates/vela-core/src/storage_catalog.rs#L120-L168
  - rust/crates/vela-core/i18n/locales/en/settingsModals.json#L22-L34
  - rust/crates/vela-core/i18n/locales/en.json#L150
docs: networks-and-fees
related:
  - settings/your-own-rpc-node
  - settings/erase-this-device
draft: true
---

Each of these providers issues one API key that works on every network it serves. You paste the
key in Settings → RPC Providers, and Vela builds that provider's endpoint for each built-in network
on its list. Alchemy and dRPC cover every built-in network except XRPL EVM. Ankr covers Ethereum,
BNB Chain, Polygon, Arbitrum, Optimism, Base, Avalanche and Gnosis. A key isn't used on a network
you added yourself.

## Where does it rank?

For each network, Vela tries endpoints in this order:

1. the RPC URL you set for that network
2. your provider keys
3. Vela's built-in and public nodes
4. nodes from the chain index

With more than one key, Alchemy goes first until Vela has measured which provider answers fastest.
If a provider fails on a network, Vela moves on to the next endpoint for it.

## Checking a key

The key is saved when you leave the field. Vela then tries the key on each of the provider's networks
and shows how many answered with the right chain ID. "Check key" runs the test again. To stop
using a provider, clear its field.

## Where the key goes

Vela stores the keys on the device without encryption, with your other settings. The key is part of
each endpoint address built from it, so it goes to the provider with every request. It also goes to
the relay whenever that endpoint is the address the wallet sends the relay, as [Using your own RPC
node](/notes/settings/your-own-rpc-node) explains.

"Erase This Device" deletes the keys. Clearing "Custom tokens and networks" in Device storage
doesn't, though it does remove the RPC URLs you set per network.
