# J03 · Asymmetric Trust Model (Received vs Sent Confidence)

| | |
|---|---|
| **Epic** | J — Simulation & Safety Guards |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | J02 |
| **Related** | J01, I08, D05 |

## 1. Summary

Simulation logs are **unauthenticated**, so a hostile contract can emit a fake `Transfer(_, you, big)`
and spoof a green "+1,000,000 USDC." Vela renders a **received** amount with confidence **only** if the
token is curated/known, the chain's stable/wrapped, or one you already hold; otherwise it degrades to
**"unverified"** (direction + caution, no attacker-controlled amount). An **outflow can't be
understated**, so **sent** amounts always render. This asymmetry is the standout security idea in
simulation.

## 2. Background & context

Trusting simulated *received* amounts is dangerous because attackers control the logs. Trusting
*sent* amounts is safe because understating an outflow doesn't help an attacker. Encoding this asymmetry
prevents "you'll receive a fortune" bait while still showing what you're giving up.

## 3. Users & stories

- As a **user**, I don't want a spoofed "you'll receive X" to trick me, so that a hostile contract can't fake a gain.
- As a **user**, I always want to see what I'm sending, so that outflows are never hidden or understated.

## 4. Functional requirements

- **FR-1** — Render a **received** amount confidently **only** if: the token is curated/known (D04), a chain stable/wrapped, or one the user already holds.
- **FR-2** — Otherwise degrade received to **"unverified"**: show direction + caution, **not** the attacker-controlled amount.
- **FR-3** — Always render **sent** amounts (outflows can't be understated to the attacker's benefit).
- **FR-4** — Feed this confidence into risk scoring (I08) so unverified receipts don't read as "safe."

## 5. Non-functional requirements

- **NFR-1** — The trust rule is explicit and testable (harness scenarios).
- **NFR-2** — Consistent with deposit detection's allowlist stance (D05).

## 6. UX / flow notes

In `BalanceChangePreview` (J02), unverified receipts show a caution marker and hide the raw amount; sent amounts show normally.

## 7. Acceptance criteria

- [ ] **AC-1** — A fake `Transfer` of an unknown token shows "unverified," not a confident "+big."
- [ ] **AC-2** — A known-token receipt shows the amount confidently.
- [ ] **AC-3** — Sent amounts always render.

## 8. Out of scope / non-goals

- Preview rendering — **J02**; risk math — **I08**.

## 9. Dependencies, risks & open questions

- **Risk:** an attacker using a *known* token they don't actually send — outflow/side reconciliation and revert checks (J01/J04) bound this.
- **Open question:** None.

## 10. Source anchors

- `src/services/tx-simulation.ts:257` — received-confidence rule.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 24.
