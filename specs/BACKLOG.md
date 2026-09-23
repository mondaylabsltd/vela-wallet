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
