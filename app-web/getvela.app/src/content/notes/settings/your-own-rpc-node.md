---
title: "Using your own RPC node for a network"
description: "Enter its address in Settings → Networks, and Vela reads that network through your node first. The relay never submits transactions through it."
facts:
  - "Where | Settings → Networks → RPC URL"
  - "Rank | First, ahead of provider keys"
  - "Refused only if | The node reports another chain ID"
  - "Relay submits through it | Never"
checked: "2026-10-09"
commit: "8e70878dd"
sources:
  - rust/crates/vela-core/src/app/network_admin.rs#L42-L55
  - rust/crates/vela-core/src/app/network_admin.rs#L2944-L2967
  - rust/crates/vela-core/src/app/rpc_pool.rs#L194-L200
  - rust/crates/vela-core/src/app/rpc_pool.rs#L222-L250
  - rust/crates/vela-core/src/app/rpc_pool.rs#L1599-L1633
  - app-web/vela-wallet/src/lib/services/rpc-pool-endpoints.ts#L93-L129
  - rust/crates/vela-core/i18n/locales/en.json#L134
  - rust/crates/vela-core/i18n/locales/en/settingsModals.json#L6-L14
  - rust/crates/vela-core/i18n/locales/en/assets.json#L19-L22
  - specs/098-relay-reaches-your-network/spec.md#L23-L32
  - specs/098-relay-reaches-your-network/spec.md#L153-L181
  - https://github.com/mondaylabsltd/vela-relay/blob/main/docs/rpc.md
  - app-web/getvela.app/src/routes/privacy/+page.svelte#L109-L122
docs: networks-and-fees
related:
  - settings/rpc-provider-keys
draft: true
---

Vela reads each network through a pool of RPC endpoints. The address you enter ranks above nodes
built from your [RPC provider keys](/notes/settings/rpc-provider-keys) and Vela's built-in nodes. If
your node stops answering, Vela moves on to the next endpoint and tries yours again after a pause
that grows from 30 seconds to five minutes.

## When is it saved?

Vela saves the address as soon as you leave the field. It first asks the node for its chain ID. If
the node reports a different chain, the field shows "Not saved" and the previous endpoint stays in
use. A node that doesn't answer is saved anyway, because Vela refuses an address only when the node
proves it serves another chain.

If every endpoint for a network fails, Vela offers "Fix RPC" for that network, which saves to the
same field.

## What does the relay get?

With every request, the wallet sends the relay an RPC address from its pool for that network,
including any API key in it. It isn't always your node: for some requests it's whichever endpoint
answered a quick chain ID check fastest. The relay reads the network through that address first,
for the fee quote, gas prices and gas estimates. It doesn't write the key anywhere, and its logs keep
only the host.

To submit your transaction, the relay uses only its own endpoints, because a node it doesn't
control could misreport a nonce or a receipt and make it send again.

Vela's relay uses the address only if it's a public `https` one. It refuses `localhost` and private
network addresses, so a node on your own machine serves the wallet but not the relay. To keep your
API key from the relay, use an address without one, or [run your own relay](/docs/self-hosting#relay).
