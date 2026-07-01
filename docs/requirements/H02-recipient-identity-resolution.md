# H02 · Recipient Identity Resolution (Name Services)

| | |
|---|---|
| **Epic** | H — Send & Receive |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | F03 |
| **Related** | H01, H03, C03, D08 |

## 1. Summary

Vela resolves a recipient address to a **human name** across name services **in parallel**, taking the
**first match by priority**: Vela Passkey Index → `.bnb` (BSC) → `.arb` (Arbitrum) → `.g` (Gravity) →
Basename (Base, ENSIP-19) → ENS (mainnet). All via **direct RPC** (no third-party name API); only
**positive results are cached (24h)**.

## 2. Background & context

Names make recipients legible and reduce wrong-address risk, but they're not proof of identity (H03
still applies). Resolving on-chain keeps it privacy-preserving (A03) and consistent with "no third-party
API." Priority ordering picks the most Vela-relevant name first.

## 3. Users & stories

- As a **user**, I want to see a name for an address, so that I recognize who I'm paying.
- As a **user**, I want name resolution without a naming API tracking me, so that it fits Vela's privacy stance.

## 4. Functional requirements

- **FR-1** — Query name services **in parallel**; return the **first match by priority**: Vela Passkey Index → `.bnb` → `.arb` → `.g` → Basename → ENS.
- **FR-2** — Resolve via **direct RPC** over the pool (F03); no third-party name API.
- **FR-3** — Cache **only positive** results for **24h**; negatives are not cached (re-resolve later).
- **FR-4** — Surface the resolved name in send (H01), activity (D08), and signing (I01) — never as proof of identity (risk checks H03 still run).

## 5. Non-functional requirements

- **NFR-1** — Unreachable services degrade to "no name," never a false name.
- **NFR-2** — Parallel resolution is bounded so it doesn't stall the send UI.

## 6. UX / flow notes

Names appear next to addresses with the source implied by priority. The Vela Passkey Index name comes from C03. A name never suppresses a risk banner (H03).

## 7. Acceptance criteria

- [ ] **AC-1** — An address with a Basename and an ENS name resolves to the higher-priority one.
- [ ] **AC-2** — A positive resolution is cached 24h; a negative is re-queried.
- [ ] **AC-3** — With all services unreachable, no name is shown (no fabrication).

## 8. Out of scope / non-goals

- Risk checks — **H03**; on-chain name publish — **C03**.

## 9. Dependencies, risks & open questions

- **Risk:** name spoofing/impersonation — mitigated by treating names as hints, not identity (H03).
- **Open question:** additional name services as chains are added.

## 10. Source anchors

- `src/services/recipient-identity.ts` — parallel priority resolution + 24h positive cache.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 26.
