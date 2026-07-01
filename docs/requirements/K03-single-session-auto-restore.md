# K03 · Single-Session Model & Auto-Restore

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K01 |
| **Related** | K02, K08, C05, L01 |

## 1. Summary

Vela maintains **exactly one dApp connection at a time** — a deliberate simplification that shrinks
attack and cognitive surface (A01). Sessions **auto-restore on mount** via an ordered strategy
(remote-inject first, then WalletPair, then a signed snapshot), so a reload or app relaunch reconnects
the user's single session rather than dropping it.

## 2. Background & context

Multiple simultaneous connections multiply risk and confusion. One session keeps the model legible:
"you are connected to this dApp." Auto-restore makes that session durable across the app lifecycle,
consistent with pending-op persistence (L01).

## 3. Users & stories

- As a **user**, I want one clear active connection, so that I always know what I'm connected to.
- As a **user**, I want my session to survive a reload, so that I don't re-pair constantly.

## 4. Functional requirements

- **FR-1** — Enforce a **single** active dApp session; connecting a new dApp replaces the current one.
- **FR-2** — Auto-restore on mount using the ordered strategy: remote-inject → WalletPair → signed snapshot.
- **FR-3** — Scope the session to the active account (C05); switching accounts affects the connection.
- **FR-4** — Surface the single connection compactly on home (D01) and fully in the Connections panel (L04-adjacent).

## 5. Non-functional requirements

- **NFR-1** — Restore is bounded by resilience deadlines (K08) so it never hangs.
- **NFR-2** — The signed snapshot restore is integrity-checked.

## 6. UX / flow notes

Connected card shows the dApp + E2E badge (K02). Replacing a session is explicit. Pending dApp ops persist independently (L01).

## 7. Acceptance criteria

- [ ] **AC-1** — Connecting a second dApp replaces the first (single session).
- [ ] **AC-2** — Reloading the app auto-restores the active session.
- [ ] **AC-3** — Switching the active account re-scopes/ends the session appropriately.

## 8. Out of scope / non-goals

- Pairing/fingerprint — **K01/K02**; resilience — **K08**; pending-op persistence — **L01**.

## 9. Dependencies, risks & open questions

- **Risk:** account switch mid-session — must scope cleanly (C05).
- **Open question:** whether to ever support >1 session (currently a firm non-goal).

## 10. Source anchors

- `src/models/dapp-connection.tsx` — single-session + auto-restore strategy.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 90.
