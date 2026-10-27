# 105 — Mainnet and testnet spaces

**Status:** design approved by the owner (2026-10-27); implemented on all four shells (2026-10-27), automated suites green. Not yet run on the phones' test wallets.

## The problem

Mainnets and testnets are mixed together. The fiat total counts testnet coins: Arc Network Testnet's native USDC is priced at $1, because `pegged_native_usd` prices a native coin by its symbol alone. Every list and picker (assets, send, receive, activity, dApps) offers both kinds side by side. Nothing tells a person whether what they see has real value.

What the code has today:
- `BUILTIN_CHAINS` holds 24 networks, every one a mainnet, with no testnet field.
- Every testnet a person sees is one they added. The add-network wizard reads the chain registry's `testnet` flag (`NetChainInfo.is_testnet`), but `NetCustomNetwork` does not keep it, so nothing knows afterwards.
- The shells each build the list of chains the balances read: Android reads the core's network rows; iOS, the web and the desktop keep their own copies.

## Owner decisions (2026-10-27)

- **Two spaces.** The **mainnet space** holds networks whose assets have real value. The **testnet space** holds testnets and developer networks with nothing of real value.
- **One device-wide switch**, shared by every wallet. A device starts in the mainnet space.
- **Always visible.** Every screen says which space is in front. The testnet space is unmistakable.
- **Each space offers only its own networks**: balances and the total, assets, activity, receive, send's tokens and networks, adding tokens, network settings, and the chains a dApp can use.
- **A network added by the person:** a network the chain registry calls a testnet goes to the testnet space. Anything the registry does not say goes to the testnet space by default. The person chooses when adding, and can move it later.
- **The AI agent** may ask to switch spaces; the person approves it, like switching wallets. It reads only the current space.
- **Common public testnets are built in** (owner, 2026-10-27), in the testnet space, so a developer does not have to add them. A testnet is built in only where this wallet works on it: the Safe and WebAuthn contracts it needs are deployed there and the relay serves it. Candidates: Ethereum Sepolia, Base Sepolia, Arbitrum Sepolia, OP Sepolia, Polygon Amoy, BNB Smart Chain Testnet, Arc Network Testnet, Tempo Moderato. Each is checked the way the add-network wizard checks a network before it goes in.

## Design

### The core decides; the shells follow

- **Classification.** Built-in chains gain `testnet: bool`; all are `false` today. A testnet built-in added later (Sepolia, say) is marked so in the table. The `NetCustomNetwork` record gains `isTestnet` (camelCase, the shared `vela.customNetworks` record), written from the wizard's choice. `NetNetworkRow` exposes `is_testnet`.
- **The space** is a preference, `vela.networkSpace` (`mainnet` | `testnet`). It is read and validated in the core like the fee speed (`fee_tier_pref.rs`); anything unreadable is `mainnet`. The storage catalog classes it as a user setting, so clearing caches keeps it.
- **One function picks a space's chains:** `space_chain_ids(space, networks)`. Every shell's chain list goes through it, so the four cannot disagree.
  - The balance read gets only the current space's chains, so the total counts only that space.
  - So do the send and receive chain lists, manage tokens, the signing machine's `supported_chains`, and the dApp browser's `NetworksChanged`.
  - The activity feed shows only the current space's records. They stay stored; switching back shows them again.
- **No value on a testnet.** Nothing on a testnet is priced: `pegged_native_usd` and the registry stablecoins' $1 peg apply only to mainnet chains. In the testnet space, amounts show without a fiat value, and the total reads as "test assets, no real value" instead of a fiat figure.

### What the person sees

- **Mainnet space:** a small "Mainnet" label beside the account name in the home header. Tapping it opens the switch.
- **Testnet space:** a persistent amber bar at the top of every screen, "Testnet space · nothing here has real value", drawn the way the parallel space's badge is placed (iOS a top safe-area inset, Android over every route, the web a fixed bar, the desktop above the page). The header label reads "Testnet".
- **The switch** lives in the header label and in Settings → Networks. Switching to the testnet space says what it means once; switching back needs no confirmation.
- **Add network:** the wizard shows the registry's verdict as a choice, "Testnet — no real value" or "Mainnet", preselected to testnet when the registry does not say. Settings → Networks lists each space's networks under its own heading and offers "Move to the other space" for a network the person added.
- **dApps:** a site that asks to switch to (or add) a chain of the other space is answered as for an unsupported chain, with a line saying it is a testnet or a mainnet chain and where to switch spaces. A site's remembered chain from the other space falls back to the space's first chain.
- **Send:** a recipient address is not tied to a space, so nothing changes there. The token and network pickers simply show the current space.

### The AI agent

- **A new action,** `set_network_space { space }`. It is manage-level: the person's Allow, the approval naming the space.
- **`vela_state`** says `network_space`, and its networks and balances are the current space's.
- **`prepare_transfer`** on a chain of the other space is refused, naming the space it belongs to.

### Networks added before this

A custom network stored without `isTestnet` is classified once, on first launch. The shells already fetch the chain registry entry for a network, and the registry's verdict is used when it answers. Otherwise the network goes to the testnet space, per the owner's default. The person can move it.

The risk: someone who added a real mainnet by hand, and whose registry lookup fails, finds it in the testnet space after the update. The fix is one tap, "Move to the other space".

## Phases

| # | What | Exit |
|---|---|---|
| 1 | Core: classification, the preference, `space_chain_ids`, no value on testnets, the agent's action and state | Rust tests; vectors for the shells |
| 2 | Web: the indicator, the switch, filtered lists, the wizard choice, dApps | unit + e2e |
| 3 | Desktop | tests + a manual pass |
| 4 | iOS, Android | tests + a run on the phones' test wallets |

## Implementation notes (2026-10-27)

- Built in: Sepolia, Base Sepolia, Arbitrum Sepolia, OP Sepolia, Polygon Amoy, BNB Smart Chain Testnet, Avalanche Fuji, Arc Testnet and Tempo Moderato, all in `BUILTIN_CHAINS` with `testnet: true`. The relay's float is empty on every one except Arc Testnet. The owner funds the relay address before sends work there.
- The activity feed filters in the core (`SpaceChainsChanged`), so all four shells draw the same records.
- Moving a network between spaces is the person's alone, with no intent and no agent action. An agent that could move a testnet into the mainnet space could make test coins read as money. All four coverage ledgers place it as `none`.
- The balance cache (`vela.balanceCache`) is read and written only in the mainnet space. The token cache is keyed per space.
- Tests that used Sepolia as "a chain the wallet does not have" now use Holesky (17000).

## Open

- The wording of the bar and the labels: en and zh are reviewed; the other catalog languages are machine-quality.
- Networks added before this are not classified by the chain registry on first launch. They read as testnet, and one tap moves them.
- A balance read in flight while the space switches can, when it settles, write that space's total to the saved cache (Android and desktop guard on the space at write time; the window is one read).
- The account switcher's per-account totals still count fiat in the testnet space on the desktop.
- A device run on the phones' test wallets (phase 4 exit).
