# 062 — Wallet keys: see them, replace one, back them up

**Status**: draft for the founder's rulings (§8). Nothing in §3–§5 is built.
**Origin**: founder, 2026-09-17, on the back of issue 191 (where the wallet learned to
read a Safe's founding key from the chain).
**Shells**: web, Android, iOS, desktop — every rule in `vela-core`, written once.

> 「我希望在设置中能看到这个地址的公钥有哪些，有两个版本：创建版本（所有网络都一样），
> 以及最新版本（不同网络可能不一样，需要到 safe account 实时查询出 signer 以及公钥）……
> 支持对某个网络的 signer 进行更换，操作交互要简单友好，小白就能上手……
> 支持把某个钱包的创建版本的 p256 数据迁移到以太坊主网。」

## 1. Why this matters

A Vela address is `f(founding keys)`. That is why it is the same on every network and
why it can be rebuilt from nothing but a passkey — and it is also why, today, a lost or
stolen key is permanent: `signers.md` tells people to "plan the key set at creation"
because the wallet exposes no way to change it afterwards. For a wallet people are asked
to pay for and keep money in, "you can never change a lock" is the largest remaining
hole in the safety story. Three things close it:

1. **See** — which keys control this address, as founded and as each network has it now.
2. **Replace** — swap one key for another on a network, without moving the money.
3. **Back up** — put the founding record somewhere that outlives us and Gnosis.

## 2. What is true today (verified 2026-09-17, not assumed)

| Fact | Where / how verified |
|---|---|
| Every wallet is 1-of-N, N ≤ 7. `owners[0]` is the shared `SafeWebAuthnSharedSigner` `0x94a4…55c2`, configured with key 0 inside the Safe's own storage; keys 1..N are `SafeWebAuthnSignerProxy` owners. | `safe.rs:173-232`, `:252` |
| The founding key is readable per chain: `sharedSigner.getConfiguration(safe)`. | live, Gnosis; shipped in `registry_lookup.rs` (#191) |
| A signer proxy's key is readable from its 236-byte runtime: `verifiers @0x01`, `y @0x27`, `x @0x4d`, `singleton @0x82`. | live on `0x88cC…6894`: owners `0x039a…d7c1`, `0xe1aa…06f9` decode to exactly the two extra members the registry lists for that wallet |
| Nothing in the repo reads owners, swaps owners, or encodes any owner-management call. | grep: `swapOwner`, `getOwners`, `addOwnerWithThreshold` → 0 hits |
| A Safe self-call goes through the existing UserOp path unchanged; only approval-shaped calldata is inspected. | `user_op.rs:1250-1254`, `approval_guard.rs` |
| The signature names its signer: the owner address is the `r` word. Which owner that is comes from `Account.keys` — the FOUNDING set. | `user_op.rs:304-335`, `:468-486` |
| The registry keeps every accepted write's calldata on-chain: `registerPayloadOf(unitId)` returned 4,324 bytes for a real 3-key wallet. The signed domain is frozen at `(100, 0x5266…edaf)`, so those bytes verify on any same-domain deployment **with no passkey ceremony**. | p256-index `WebAuthnP256PublicKeyRegistry.sol:116-128, 255-264`; live `eth_call` |
| Replaying a payload that is already there reverts with a recognisable error. | live, Gnosis: `0x10eafb1a…` |
| The registry is **not** deployed on Ethereum (`eth_getCode` = `0x`). `P256VERIFY` at `0x100` **is** live there. | live, mainnet |
| The registry has a `refer` write: one passkey pointing at an existing group — its "add a device later" table. Discovery data, not authority. | p256-index contract `:618`, README |
| No settings screen on any shell lists an account's keys. `AccountKey` stores no AAGUID / attestation / sync flag; the index returns them, the client drops them. | `registry.ts:188-193`, `mod.rs:133-146` |

## 3. Part A — "Keys" in Settings

One new row on every shell: **Settings → Keys** (密钥). Two sections.

### A1. Founding keys — 创建时的钥匙 · the same on every network

From the registry unit that names this address (the #191 walk already finds it), one card
per member, in founding order:

- provider mark + model name (the vendored AAGUID catalog: "YubiKey · Security Key Series
  with NFC"); the person's own key name above it
- badges from the 20-byte attestation: **Synced / This device only** (BS flag),
  **User-verified** (UV flag); attachment + transports as one quiet line
- public key and credential id behind a disclosure, each with Copy
- the group: unit id, `safe-1.4.1`, registered date, relying party

This is the registry explorer's card, in the wallet. Offline or index unreachable: the
locally stored `Account.keys` with a line saying the details could not be loaded.

### A2. Keys on each network — 各网络上现在的钥匙

One row per network where the Safe is deployed, read live:

`getOwners()` · `getThreshold()` · `getConfiguration(safe)` for owner 0 · `eth_getCode` for
each proxy owner.

Each network row resolves to ONE of four states, and says it in words:

| State | Meaning | Row says |
|---|---|---|
| **Same as founded** | live owners == founding set | ✓ Same as created |
| **Changed** | differs | "1 key replaced" — opens the diff |
| **Not set up yet** | no code at the address | "Not used on this network yet — it will start with the founding keys" |
| **Could not check** | RPC silent | "Could not check" + retry — never shown as "same" |

A key read from the chain is believed only if re-deriving its proxy address
(`compute_webauthn_signer_address`) gives the owner address it was read from. An owner that
is neither the shared signer nor a recognisable proxy is shown as **Unknown signer** with
its address — never hidden: an unexpected owner is exactly what this screen exists to catch.

`threshold != 1` is shown as a warning state; this wallet never sets it.

## 4. Part B — Replace a key on one network

### B1. The flow, as the person meets it

From a key's card on a network: **Replace this key**.

1. **Why** — one screen, two choices: *I lost it / it may be stolen* · *I'm moving to a new
   device*. (Only changes the wording of the last step.)
2. **Create the new key** — the existing add-a-key picker (this device / phone / security
   key), unchanged.
3. **Review** — one sentence, a before/after of the two key cards, the network, the fee.
   "On **Base**, **MacBook Touch ID** will stop working and **YubiKey 5C** will start."
4. **Confirm** — signed with any key that still works on that network. If the only key the
   person holds here is the one being replaced, that is fine: it signs its own replacement.
5. **Done** — the network row flips to *Changed*; an offer: "Do the same on your other
   networks" (a checklist of the deployed ones, one confirmation each — see B3).

### B2. What is sent

One UserOp on that network, one MultiSend:

- key 1..N: `factory.createSigner(x, y, 0x100)` (CALL; skipped if the proxy has code) then
  `safe.swapOwner(prevOwner, oldProxy, newProxy)` (self-CALL)
- key 0: `sharedSigner.configure((x, y, 0x100))` as a **DELEGATECALL** leg — the shared
  signer stays owner 0, its configuration in the Safe's storage changes

`prevOwner` comes from the live `getOwners()` order read in the same session, and the core
refuses to build the call if the live owner list no longer matches what the review showed.

Then, best-effort and off the money path: `registry.refer(groupPublicKey, metadata, member)`
so the new key can FIND this wallet at login. The registry calls this discovery data, not
authority — it is never what makes the key an owner.

### B3. The three things this must get right

1. **Signing must follow the chain, not the founding record.** `signer_address_for` reads
   `Account.keys`. After a replacement the new key is in no founding set, so the account
   gains a per-network signer map (`chain_id → owners`), refreshed from the chain, and
   signing on chain X consults it. Without this a replaced key cannot sign and the feature
   is a trap.
2. **Login with only the new key.** Today login rebuilds the address from a unit the key
   FOUNDED. A referred key founded nothing. Login gains one rung: no founded unit for this
   key → its references → the group → the address (the group's address is still
   `f(founding keys)`, which the unit carries).
3. **The cache rule from #191 changes.** `registry_lookup` keeps a hit "for good" because a
   founding key "does not change". After key 0 is replaced, `getConfiguration` returns the
   NEW key and the walk finds no unit for it. The walk must fall back from "units founded"
   to "references" as well.

### B4. What the person must be told, in the flow, once

- **A replacement is per network.** The old key still works on every network where it has
  not been replaced.
- **A network you have never used starts with the FOUNDING keys.** The address is
  `f(founding keys)`; the first operation there deploys the Safe with them. If a founding
  key is *stolen* (not merely lost), whoever holds it can deploy and spend on any network
  this wallet has not been set up on. For "lost" this is harmless. For "stolen" the honest
  advice is: replace on every network in use **and** stop receiving on networks you have
  not set up — or move to a new wallet. The flow says this when the reason is *stolen*.
- The founding record never changes. Settings keeps showing both.

## 5. Part C — Back up the founding record to Ethereum

**What it is**: the wallet's founding registration (keys + proofs + metadata), copied into
the same registry contract on Ethereum. After it, the wallet can be rebuilt from Ethereum
alone — no Vela server, no Gnosis.

**Why it needs no signing**: the registry stored the original calldata and froze its
signature domain, so the bytes that were valid on Gnosis are valid on Ethereum. Anyone can
submit them; whoever submits pays.

Settings → Keys → **Back up to Ethereum**. One row, three states, found without any server:
`eth_call` the payload against the Ethereum registry — it would succeed → *Not backed up*;
it reverts as a duplicate → *Backed up ✓*; anything else → *Could not check*.

Flow: a sentence on what it is and that it is optional → the fee from `eth_estimateGas`
(never a constant; ~$2–3 at 0.23 gwei for a 3-key wallet, 2026-09-17) → confirm → one
UserOp from the person's Safe on Ethereum whose single call is `registry ← payload`.
References (`referPayloadOf`) replay after the register, in order.

**Restore**: the #191 walk and login learn a second source — the same three hops against
the Ethereum registry by `eth_call` when the index is unreachable. This is what makes the
backup worth having, and it is in scope.

**Blocked on one thing outside this repo**: the registry is not deployed on Ethereum. It is
a one-time CREATE2 deployment (same bytecode, same constructor `(100, 0x5266…edaf)`, same
salt ⇒ the same address `0x94fD…1EA9`), by anyone, with no owner. It is the operator's to
do; `p256-index/docs/mainnet-self-backup.md` §3 is the runbook. Part C ships dark until the
address has code.

## 6. Out of scope

- Adding an 8th key, removing a key without replacing it, or raising the threshold.
  1-of-N stays; "replace" keeps N constant, which keeps every rule about N true.
- Replacing on a network the Safe is not deployed on (there is nothing to change yet).
- Server-paid Ethereum gas, dual-writing, Ethereum-first reads (the server design's own
  non-goals).
- Any change to `encode_setup_data`. One byte there moves every multi-key address.

## 7. Success criteria

- **SC-621** Settings → Keys lists the founding keys with model, badges and copyable key,
  on all four shells, and says so when the index could not be reached.
- **SC-622** For `0x88cC…6894` the Gnosis row reads *Same as created* and lists three keys
  whose public keys equal the registry's; a network with no code reads *Not set up yet*; a
  silenced RPC reads *Could not check*, never *Same*.
- **SC-623** A key whose re-derived proxy address does not match is shown as *Unknown
  signer*, never as a named key.
- **SC-624** (parallel space, Gnosis, real funds) a proxy key is replaced in one operation;
  afterwards the OLD key's signature is rejected and the NEW key's operation lands.
- **SC-625** the same for key 0 (`configure`).
- **SC-626** the review refuses to proceed when the live owners changed since it was drawn.
- **SC-627** a fresh device holding only the NEW key signs in and lands on the same address.
- **SC-628** the *stolen* path shows the undeployed-network warning; the *new device* path
  does not.
- **SC-629** (after deployment) a wallet backed up to Ethereum reads *Backed up ✓* from a
  cold start with the index unreachable, and signs in from Ethereum alone.
- **SC-630** each of the above on a real Android phone and a real iPhone, recorded
  separately from "tests pass" (the founder's standing rule).

## 8. Rulings needed before Part B is written

1. **Replace only, N constant?** Recommended. Add/remove changes what "1-of-N" means per
   network and doubles the states Settings must explain.
2. **The undeployed-network caveat (B4).** It is a property of the address scheme, not a
   bug we can fix in this spec. Is telling the person, on the *stolen* path, acceptable —
   or should *stolen* route to "move to a new wallet" instead of "replace"?
3. **"Do the same on other networks"**: offer it as a checklist after the first
   replacement (recommended), or make replacement all-networks-at-once from the start?
4. **Who deploys the registry to Ethereum, and when?** Part C waits on it.
5. **`signers.md` and `B09`** say owners cannot be changed / one passkey per wallet. Both
   get rewritten when Part B ships — confirm the public wording changes with it.
