# Backlog — reports that belong to no open spec

A spec's `tasks.md` holds what that spec found. This holds what the owner
reported while a *different* spec was open, so it survives the spec that was
current when it was said. Each entry names the evidence, not just the symptom.

## B-1 · A scanned address locks the asset (owner, 2026-09-23)

**Reported:** 「扫码到一个地址后无法切换发送资产，只能发送里面的默认代币」 — after
scanning a QR to get a recipient, the send screen will only send the token it
arrived with.

**What the code does.** A scan with a prefilled recipient skips the token
picker entirely and takes the top of the list:

```rust
// send.rs, the prefilled-recipient branch
if let Some(first) = model.tokens.first().cloned() {
    // Quick-send from scan: highest-value token, prefilled recipient.
    model.selected_token = Some(first);
    model.recipient = prefilled;
    model.step = SendStep::EnterDetails;
```

The way back to the picker exists — `Event::Back` from `EnterDetails` sets
`step = SelectToken` — but it **clears the recipient on the way**:

```rust
model.selected_token = None;
model.amount = DenominatedAmount::token("");
model.recipient.clear();      // ← the scanned address is gone
```

So changing the asset costs the person the scan: they go back, pick another
token, and have to scan again. From the screen it reads as "the asset cannot be
changed", which is what was reported.

**Decided (owner, 2026-09-23): a hand-off is not discarded.** When the
recipient came from outside — a scan, a contact, a payment request — going back
to the picker keeps it, so changing the asset costs nothing. The person never
chose that token; only the wallet did, from the balance. `Back` still means
"start over" when the person typed the recipient themselves, because there the
hand-off is theirs to redo.

Affects every shell, because the step machine is the core's.

## B-2 … B-6 · The Chrome extension's signing surface (owner, 2026-09-23)

**Status 2026-09-23: B-2, B-3, B-4, B-6 fixed and driven in a real browser;
B-7 fixed structurally; B-5 deferred by the owner.** Spec
[077](077-one-signing-surface/spec.md) — the reports were right that they were
one thing, and `tasks.md` there carries the evidence per item.

Five reports from using the extension against Uniswap. They are listed together
because the first four may share one cause and the fifth almost certainly does.

**B-2 · the wallet disappears when a request arrives.** ✅ **Fixed** (077
FR-001). Wanted: the side panel keeps the wallet UI and the signing sheet rises
over it, as on Android and iOS. What happened: `extension/panel.js` navigated
the panel to the request page — a page whose whole job is the one request — so
the wallet was not under it; it was replaced.

The panel now opens `wallet.html?panel` and the request rises over it.
*Measured: the consent card as a sheet with "Parallel One · $5.44 · Receive Send
Scan Activity" behind it, then the signing sheet, then the receipt, the wallet
visible throughout.* Two things had to come with it: a request arriving while
the panel is already open (`sidePanel.open` does not reload a standing panel),
and a guard so the wallet under a pending request can be read but not driven.

**B-3 · no speed switch when signing a dApp transaction.** ✅ **Fixed.**
**B-4 · a Uniswap swap on Polygon shows no network fee.** ✅ **Fixed**
(commit c2b0c5e4). It was the first candidate below, with a cause neither guess
named: `BigInt('0x')` THROWS, and `'0x'` is what several dApp libraries send for
"no ether with this call". The throw took the calls, and with them the fee AND
the speed control — one `catch`, two things gone, silently. `fee-calls.ts` now
reads `'0x'` as zero and refuses only what is genuinely unreadable.
*Measured on the panel: "Network fee 0.01 xDAI · ≈$0.01 | Speed Fast".*

What I checked before guessing: the web's `SigningHost.svelte` DOES build a
`SpeedControl` over the sheet's `FeeQuote`, and `SigningSheet` → `FeeRow` draws
a `FeeSpeedRow`. So "the web never wired the speed control" is FALSE, and the
simple story — "the dApp path is not the send path" — does not survive contact
with the code.

Two candidates remain, both to be measured:
- `SigningHost.callsOf()` returns `null` unless every call has a non-empty `to`,
  and `null` means no quote is even asked for. A swap should pass that.
- the quote is asked for and comes back refused, and the sheet then draws
  nothing rather than saying why.

**B-5 · Arbitrum fails outright.** ⏸ **Deferred by the owner**
(「B-5 · Arbitrum 直接失败 先忽略吧」). Not a missing network: chain 42161 is a
built-in (`network_admin.rs:234`), with an Alchemy slug and an explorer.
**Blocked on the error text** — "直接失败" could be the bundler refusing the
chain, a Safe not deployed there, or the in-band fee path. The screen's own
sentence would name it.

**B-6 · a dApp signature has no on-chain waiting animation**, and
**B-7 · Settings' "save the public key to Ethereum mainnet" feels unlike a
transfer.** Both are the same shape: a surface that submits a transaction
without the send flow's waiting-and-landing treatment.

✅ **B-6 fixed** (077 FR-002). The landing belongs to the SHEET, so every
surface that mounts it gets one. *Measured: submitted at t+5s with the operation
hash, the ring filling on the chain's own clock, confirmed at t+24s with the
transaction hash and an explorer link, Done returning to the wallet.*

✅ **B-7 fixed structurally, not yet watched.** The backup posts into the same
seam and so inherits the landing; a test now forbids any surface mounting the
sheet without a receipt, which is exactly how B-7 happened. It signs on Ethereum
mainnet and this wallet has no mainnet gas, so nobody has watched one confirm —
see 077's tasks.md, "Not verified".

**Owner's direction:** 「我觉得签名管线应该统一一下吧」 — one signing pipeline,
not three.
