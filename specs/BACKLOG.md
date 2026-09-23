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

Five reports from using the extension against Uniswap. They are listed together
because the first four may share one cause and the fifth almost certainly does.

**B-2 · the wallet disappears when a request arrives.** Wanted: the side
panel keeps the wallet UI and the signing sheet rises over it, as on Android and
iOS. What happens: `extension/panel.js` navigates the panel to
`<locale>/request.html?rid=…` — a page whose whole job is the one request. The
wallet is not under it; it was replaced.

**B-3 · no speed switch when signing a dApp transaction.**
**B-4 · a Uniswap swap on Polygon shows no network fee.**

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

**B-5 · Arbitrum fails outright.** Not a missing network: chain 42161 is a
built-in (`network_admin.rs:234`), with an Alchemy slug and an explorer.
**Blocked on the error text** — "直接失败" could be the bundler refusing the
chain, a Safe not deployed there, or the in-band fee path. The screen's own
sentence would name it.

**B-6 · a dApp signature has no on-chain waiting animation**, and
**B-7 · Settings' "save the public key to Ethereum mainnet" feels unlike a
transfer.** Both are the same shape: a surface that submits a transaction
without the send flow's waiting-and-landing treatment.

**Owner's direction:** 「我觉得签名管线应该统一一下吧」 — one signing pipeline,
not three.
