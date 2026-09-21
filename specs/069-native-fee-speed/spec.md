# Spec 069 — The speed control on every shell, decided once

**One repo** (`vela-wallet`: the Rust core + 4 shells). Builds on spec 068,
which shipped the refreshable fee and the speed control on the web and the
tier-aware relay. Owner decisions are marked **[R]**; everything else is
derived from source read on 2026-09-21.

## Why

Spec 068 put the fee refresh, the three-speed picker and the stored default
speed on the web. The three native shells (desktop, Android, iOS) still
hard-code `fast` at every quote, never name a tier on the wire, draw no stale
line and have no refresh control, and their Settings have no default speed.
**[R]** "网络费切换以及速度选择等功能 … 需要在 desktop / android / ios 中都接入 …
还有设置中的默认速度设置也要一并做掉."

The web's speed rules — which tier is in force, the free upgrade (#686 A), the
one-speed statement (#686 B), the gas bid as a range (#684/#685), which extra
quotes to keep alive — were written in TypeScript (`speed-choice.ts`,
`gas-price.ts`, `free-speed.ts`, the wallet route's effects). Porting them
three times would be four copies of the subtlest code on the send screen.
**[R]** Move the logic into the core first; the web then drives the core too,
and the native shells only draw and wire.

## What moves into the core

### `fee_speed` — a new machine (`vela_core::app::fee_speed`)

The speed control of one send surface. It DECIDES; the shell keeps the fee
sessions (bridge objects a machine cannot hold) and DOES.

| Event | Meaning |
|---|---|
| `configure {preferred, number}` | the stored default (`fee_tier_pref` view) and the number preset |
| `reset` | a send starts or ends: forget the pick, the upgrade, the fold, every quote |
| `stage_changed {on_form}` | a free upgrade is only decided on the form |
| `toggle` / `pick {tier}` | the folded control; a pick is one-shot and folds it |
| `quotes_changed {chain_id, in_force, previews}` | every fee session's `{busy, fee}` |

The view carries `tier` (in force — the session must price it and the submit
names it), `previews` (other tiers to keep priced), `open`, `picked`, `free`,
`free_note`, `single`, `gas_price_line`, and one option per offered tier with
its own settled `fee`, `measuring` ("…" vs "—") and formatted `gas_price`.

It asks the shell for nothing (an empty operation type), because the shell's
half is one reconcile rule, identical on every platform:

1. The session in force not pricing `tier` → **promote** the preview for that
   tier when it holds a settled quote of the same operation (the price tapped
   is the price paid, #681); otherwise re-price the session in force at `tier`
   once it is not measuring.
2. Keep a preview session for each tier in `previews`, pricing the same
   operation, re-priced whenever the session in force is; drop the rest.
3. After any session's view changes, dispatch `quotes_changed`.

The gas-bid formatter (one unit and one precision over the whole set, `~`
between the ends, `gwei`/`wei` untranslated) moved with it, over the core's own
number presets; `NumberPreset` gained its wire names (`comma_dot`, …).

The web's unit tests moved with the code, vector for vector
(`rust/crates/vela-core/tests/app_fee_speed.rs`, 57 tests).

### `fee_tier_pref` — exported over uniffi

It existed (spec 068) but only the wasm bridge exported it. Both machines are
now `bridge_object!` lines, so iOS and Android drive the same code as the web.

## The shells

Every shell does the same five things.

1. **Default speed in Settings.** A row in 高级 (between 服务端点 and 存储, as
   on the web) opening a sheet of the three speeds, each with its description
   line. Stored at `vela.feeTier` (survives sign-out; not a cache), read and
   validated by `fee_tier_pref`.
2. **The send form's speed control**, under the fee row: folded by default
   (label + the tier in force + chevron), the free note under it when
   `free_note`; open, the "this transaction only" line and three options
   (name, own fee, gas bid, description, tick), or the one-speed statement.
3. **The refresh control and the stale line** on the fee row. Refresh drops
   the shell's cached gas signals (#212) before it re-quotes, so it is a new
   measurement; the stale line ("这个数字有点旧了") reserves its height, is not
   shown while measuring nor over a figure of another tier.
4. **The tier on the wire.** The quote is priced at the tier in force and the
   submission names `quoted_fee.tier` as `eth_sendUserOperation`'s third
   parameter (never `rapid`); the relay reads it (spec 068 relay work,
   deployed). The bundler quote's `maxPriorityFeePerGas` is read so the gas
   bid can be published.
5. **The confirm restates the speed** only for a pick or a free upgrade (the
   latter with its reason).

### The dApp signing sheet [decided here, revised 2026-09-21]

A Settings row that says "default transaction speed" and is ignored by every
dApp transaction would be a setting that lies — and a sheet that prices at a
speed nobody can see or change would be half of one. So the dApp signing sheet
gets **the same speed control as the send form, on all four clients**: it
opens at the stored default (`fast` for everybody who never chose), folds
under the fee, offers the three speeds with their own fees and gas bids, takes
the free upgrade, and a tap is one-shot.

It is the same code, not a copy. Each client moved the send form's fee
sessions and reconcile step into one unit both surfaces own — web
`SpeedControl`, Android `SpeedControl`, iOS `FeeStore`, desktop
`wallet/speed_control.rs` — and the sheet's options are drawn by the send
form's own builder with the sheet's fee line.

Two rules the sheet adds, because nothing else guards them there:

1. **The slide never signs the speed just left.** Between a tap and that
   speed's own figure landing, the core's `confirm_fee_ready` is still true on
   the previous speed's quote. The row reads "estimating" and the slide stays
   shut until the fee in hand is the tier in force (issue 681's rule, applied
   to the gate as well as the row).
2. **Every sheet signs what it shows.** The approve carries the displayed
   quote — amount, recipient and `tier` (`SignQuotedFee`). The web sheet
   approved with `quoted_fee: null` until now and re-priced on submit, so a
   speed picked there could not have reached the relay; it now signs its
   displayed in-band quote like the native sheets.

### The tier on the wire, from one place

`SendQuotedFee` (built by the `send` core) and `SignQuotedFee` (carried by
the approve) now hold `tier`, taken from the same estimate as the amount and
filtered to a name the relay accepts (`fee_speed::wire_tier` — never
`rapid`). Every shell's submit path reads it there; the web's `feeTier()`
port is gone.

### Per shell

| | desktop (GPUI) | Android (Compose) | iOS (SwiftUI) |
|---|---|---|---|
| fee_tier_pref | resident `Machine` + `vela.feeTier` in `wallet.json` | `FeeTierPrefCore` + DataStore key | `FeeTierPrefCore` + `VelaStore.Key` |
| fee_speed + sessions | `wallet/speed_control.rs`, owned by `SendHost` and `SigningHost` | `SpeedControl`, owned by `SendController` and `SigningController` | `FeeStore`, owned by the send flow and `SigningController` |
| preview sessions | extra `CoreHost<FeePolicy>` per tier | extra `FeePolicyCore` hosts | extra `FeePolicyCore` stores |
| gas-signal cache (#212) | add 15 s cache + invalidate | add 15 s cache + invalidate | add 15 s cache + invalidate |
| relay tip field | read already | read `maxPriorityFeePerGas` | read `maxPriorityFeePerGas` |
| wire tier | `send_user_op` 3rd param | `sendUserOp` 3rd param | `sendUserOp` 3rd param |
| fee wire | — | add `effective_gas_price`, `max_gas_price` | add `tier`, `effective_gas_price`, `max_gas_price` |

Also fixed on the way, because the picker depends on it: the fee stamp that
decides when the send machine hears a new quote must include the tier (the web
fixed this as `feeKey`, #686) — on a floor-clamped chain two tiers charge the
same wei, and a charge-only stamp kept the old tier's estimate in the send
machine.

## Verification

- Core: `cargo test -p vela-core --features crux --test app_fee_speed` (+ the
  existing fee suites).
- Web: unit + the 068 e2e (`e2e/fee-speed.e2e.ts`) unchanged in behaviour.
- Desktop: `cargo test` in `app-desktop/vela-wallet`.
- Android: `:app:testDebugUnitTest`, then on the Redmi K40 (`9d5f42fb`): the
  control folded/open, a pick, the Settings row persisting across relaunch.
- iOS: `xcodebuild test`, then on the connected iPhone likewise.

## Sequencing

Core → web (proves the machine against the shipped behaviour) → desktop →
Android → iOS, one commit per shell. The relay already honours a named tier
(spec 068), so no shell here can name a tier the relay ignores.

## Follow-ups (not in 069)

- The relay's quoted tip must be the tip it signs. The quote read the
  `eth_feeHistory` median while the executor signs `eth_maxPriorityFeePerGas`,
  so on Polygon "Standard" showed ~353–599 gwei and landed at 282–536. Fixed in
  vela-relay on `fix/quote-tip-is-the-signed-tip` (both paths read one
  resolver); not yet deployed.
- The desktop signing sheet does not re-ask a stale quote (Android and iOS
  do); the slide stays open on it until submit re-checks.
- Per-chain default speeds (spec 068 rule 3).
