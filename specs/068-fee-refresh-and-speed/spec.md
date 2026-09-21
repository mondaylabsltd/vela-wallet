# Spec 068 — The fee is something you can refresh, and a speed you can choose

**Two repos.** `vela-wallet` (4 shells + the Rust core) and `vela-relay`.
Owner decisions are recorded inline as **[R]**; everything else is derived from
source read on 2026-09-20.

## Why

The fee row is the one number on the send screen that moves on its own, and
today the person can neither re-read it nor influence it.

- **It goes stale silently.** Gas moves; the quote has a 30 s TTL that is
  advisory and unenforced, and `fee_policy`'s `stale` flag has no consumer in
  any shell. A person on the confirm screen is spending their drift budget on
  think-time with nothing on screen saying so.
- **Speed is not offered, and cannot be.** Every shell hard-codes the `fast`
  tier (`fee-quote.svelte.ts:36`, iOS `FeeStore.swift:127` / `SendStore.swift:255`
  / `SendExecutor.swift:441` / `SigningController.swift:309`, Android
  `RelayClient.kt:366`). The relay reports three tiers
  (`user_operation_gas_price.rs`) but its executor never reads one — it computes
  `quoted_fee_per_gas = 2 × base + tip` itself
  (`execution.rs:1389`) and `decide_settlement` can only reprice **down**.
  So **"cheaper" only happens as an uncontrolled side effect** (pay less → the
  repricing valve lowers the cap → risks `FloorUnfundable`), and **"faster" is
  impossible at any price.**

## Money and speed are two different levers — do not conflate them

**This is the trap this spec exists to avoid.** The relay's *reported* tiers and
its *submit cap* are different quantities:

- the **reported tier** (`pimlico_getUserOperationGasPrice`) is ~`1.2×base`; it
  is what the CLIENT prices against (`3 × gas × max(C, R)`) — it decides **what
  the person pays**;
- the **submit cap** (`quoted_fee_per_gas` = `2×base + tip`,
  `execution.rs:1389`) is inclusion headroom — it decides **how fast the
  transaction lands**. The chain only ever charges `base + tip`, so a higher cap
  costs nothing in calm markets and only binds during a spike.

Measured 2026-09-20, Ethereum `base = 0.0535 gwei`: reported `fast` = 0.0938,
while the submit cap is `2×base` = **0.1070**. Base: reported `fast` 0.0084 vs
cap 0.0100. **The reported "fast" price sits BELOW today's submit cap on every
EIP-1559 chain.** So "submit at the requested tier's reported price" would make
超快 *slower than today* — the exact inversion the owner flagged. (On BSC the two
coincide only because its `baseFeePerGas` is 0 and the whole price is the tip;
BSC hides this bug.)

Therefore **the tier must drive BOTH levers**: the reported row the client pays
against, *and* a base-fee multiplier for the relay's submit cap. A tier that
moved only the price would mean the person pays less while the relay still
submits at the old cap — the relay silently subsidising "economy" out of its
1.4× markup, which is neither sustainable nor honest.

## The contract [R]

The client sends a **tier NAME**, never an absolute wei value: the relay
translates it with its own oracle **at submit time**, so a quote that went stale
between signing and inclusion cannot mis-set the price.

```
absent  ⇒ exactly today's behaviour — the relay keeps its own pace (2×base + tip)
present ⇒ cap = clamp( tier_multiplier[requested] × base + tip,
                       inclusion_floor,
                       reimbursement / markup )
```

| tier | cap | note |
|---|---|---|
| `slow` 较慢 | **1.5×base** + tip | the inclusion floor — genuinely cheaper *and* genuinely lower priority |
| `standard` 标准 | **2.0×base** + tip | today's behaviour |
| `fast` 超快 (default) | **3.0×base** + tip | real spike survival — this is what 加速/抢跑 buys |

**Every tier is funded by what the person already pays.** The client pays
`3 × gas × R`, the relay requires `1.4 × gas × cap`, so the highest fundable cap
is `2.14 × R`. Measured on Ethereum: slow funds up to 3.13×base, standard 3.44,
fast 3.76 — all above the caps above, so **no tier needs the relay to subsidise
it**, and none should be repriced down in normal conditions. A test must pin
this headroom per tier, because it is the property that keeps the feature honest.

- **Additive and backwards compatible.** An omitted tier changes nothing, so
  every shipped client keeps working and the change can go out relay-first.
- **Floor.** Never below `inclusion_floor_fee_per_gas` (default 15_000 bps =
  1.5×base). This is the owner's stated requirement: a price the chain will not
  include is a rejection, not a saving.
- **Ceiling.** Never above what the signed reimbursement funds at the 1.4×
  settlement markup — the relay must not run at a loss because someone asked for
  `fast`.
- Both clamps are silent and safe: the op still submits, at the nearest fundable
  includable price. Only the existing `FloorUnfundable` path refuses.

### `rapid` is a dead fourth tier — leave it, never offer it

`vela-core`'s `FeeTier` has four (`slow | standard | rapid | fast`) while the
relay reports three. `rapid` has a multiplier (`fee_policy.rs:317`, ×1.5), a
translation in all 15 locales, and a wire mapping in Android
(`RelayClient.kt:365`) and desktop (`relay.rs:437`) — but **nothing anywhere
constructs it**. It is already dead.

Ruling: keep the variant, never offer it in the picker, never put it on the
wire. Deleting it would touch the generated enum, 15 locale files and the
path-count pins for no user-visible benefit — the same reasoning that kept
`keyDeviceOnlyBadge`'s name in #207. Document it as deliberately unoffered so
the next reader does not "finish" it.

## Client behaviour

**Refresh.** The fee row gets an explicit refresh affordance, and the `stale`
flag finally gets a consumer. The plumbing already exists from #212:
`invalidateFeeSignals(chainId)` is exported and already called by
`FeeQuote.requote()` before it dispatches, so a refresh is genuinely a fresh
measurement and not the 15 s cache. What is missing is the control and the
"this figure is from a while ago" state.

**Speed.** [R] Three tiers, **`fast` stays the default**, and **the control
stays folded away** unless the person opens it.

The option NAME is the speed; what that speed BUYS is a short description line
**under** each option. (Amended 2026-09-21 by the owner, who looked at the
shipped Settings → 交易速度 dropdown: the heading asks about SPEED, so every
option has to BE a speed. `slow` had been named 经济便宜 / "Economy", which
mixes a speed scale with a value judgement and reads as a joke. Moving the
advantage to its own line keeps the reason the cheap tier is attractive without
making the scale incoherent.)

| wire | zh | en | zh description | en description |
|---|---|---|---|---|
| `fast` | 超快（默认） | Fast | 网络繁忙时也最先确认 | First to confirm, even when the network is busy |
| `standard` | 标准 | Standard | 日常转账的平衡选择 | Balanced for everyday transfers |
| `slow` | 较慢 | Slow | 手续费最低，适合不着急时 | Lowest fee, if you can wait |

Never in gwei, on either line. When the control is open each option also shows
its own fee, so the trade is visible at the moment of choosing.

### A stored default, and a one-off override [R]

A per-transaction picker alone is the wrong shape for a standing preference: it
would make **the person who cares most about cost do the most work**, tapping
the fee down on every single send. Urgency is per-transaction; thrift is not.

So the tier is chosen in two places, with two different lifetimes:

| | nature | where |
|---|---|---|
| **preference** | stable ("I always want economy") | **Settings**, stored once |
| **urgency** | one-off ("this one must land now") | the send screen's folded control |

Rules:
1. **The factory default stays `fast`**, so a fresh install behaves exactly as
   today. The setting only changes what *this person* chose.
2. **A per-transaction override is one-shot and never rewrites the preference.**
   Bump one send to `fast` and the next send is back at your default. A setting
   that silently drifts is a setting nobody can trust — this is where wallets
   usually get it wrong.
3. **Global, not per-chain, to start.** Per-chain thrift (economy on Polygon,
   fast on Ethereum) is a real want, but it fails the owner's "friendly, clear,
   simple" bar. Record as a follow-up.
4. The folded control on the send screen shows **your** default, not a global
   one, so the two surfaces never disagree.

Paved road: `display_currency.rs` is already a committed-preference core machine
of exactly this shape — follow it rather than inventing a new pattern. Storage
sits under the `vela.` prefix and **survives sign-out** (per the standing ruling
that sign-out clears only `vela.accounts` + `vela.activeAccountIndex`): a fee
preference belongs to the person and the device, not to the account.

**The factory default is deliberately unchanged.** Every shell hard-codes `fast` today,
so keeping it means this feature **cannot regress an existing send**: someone
who never opens the control gets exactly what they get now. Only a deliberate
tap makes a send cheaper and slower. (For scale: measured on BSC 2026-09-20 the
tiers sit within ~9% — fast 0.0632 / standard 0.0579 / slow 0.0526 gwei — so on
BSC the saving is small; on a congested chain it is not.)

## Work

**Relay** (`/Volumes/data/production/vela-relay`)
1. Accept an optional tier on `eth_sendUserOperation`; validate against the
   three names; `deny_unknown_fields` is in force, so this is a wire change.
2. Carry it: admission record → Redis/Iggy envelope → the executor's context.
3. At `execution.rs:1389`, when present, take `quoted_fee_per_gas` from the
   tier table instead of the default, then clamp as above.
4. Tests: each tier submits at its price; absent behaves byte-identically to
   today; a tier under the floor is raised, not refused; a tier the
   reimbursement cannot fund is capped; an unknown name is refused.

**Client**
5. Stop hard-coding `fast`: thread the chosen tier from the send/signing surface
   through `fee_policy` (already parameterised — `ctx.tier`) to the submission.
6. The picker + the refresh control, on web first, then iOS / Android / desktop.
7. i18n: **the tier names already exist** — `send.gasTier.{slow,standard,rapid,fast}`
   are in `paths.rs` and translated in all 15 locales (originally en `Slow /
   Standard / Rapid / Fast`, zh `慢速 / 标准 / 极速 / 快速`). So the tier wording
   is a **value-only** edit with **no pin bump**: `fast` → 超快 / Fast, `slow` →
   较慢 / Slow, `standard` unchanged. The DESCRIPTIONS are new paths and did
   need the pin bumped in `scripts/gen-i18n.mjs` — three flat leaves,
   `send.gasTierHint{Fast,Standard,Slow}`, chosen over a branch so only the
   leaf count moves and so `rapid`, the dead tier nothing offers, acquires no
   description to translate. Likewise the refresh control, the disclosure that
   opens the picker and the stale-quote line.
   (`slow` briefly shipped as 经济便宜 / Economy; see the amendment above.)

## Sequencing

Relay first (it is additive and invisible until a client asks), then web, then
the three native shells. Do not ship a client that names a tier before the relay
that honours it — a named tier would be silently ignored, which is the class of
"the screen says one thing, the chain does another" this whole batch is about.
