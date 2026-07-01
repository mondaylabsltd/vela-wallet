# K08 · Connection Resilience (Heartbeat, Reconnect, Deadlines)

| | |
|---|---|
| **Epic** | K — dApp Connect (WalletPair) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | K01 |
| **Related** | K02, K03, A04 |

## 1. Summary

WalletPair is tuned for mobile reality: a **WebSocket heartbeat pings every 25s** (under Cloudflare's
~30s idle timeout); on app-foreground after **≥20s** background it forces `session.reconnect()`; and
**bounded deadlines** (`CONFIRM_JOIN 30s`, `RECONNECT_MAX 60s`, provider `reconnectStuck 45s`) stop the
UI from hanging on a silent relay.

## 2. Background & context

Mobile apps background/foreground constantly and networks drop; a naive WebSocket connection silently
dies. Heartbeats keep it alive under the relay's idle timeout, foreground-triggered reconnects recover
fast, and deadlines ensure the UI never spins forever on a dead channel.

## 3. Users & stories

- As a **mobile user**, I want my dApp connection to survive backgrounding, so that I don't re-pair after switching apps.
- As a **user**, I don't want the UI to hang on a dead relay, so that failures surface promptly.

## 4. Functional requirements

- **FR-1** — Send a WebSocket heartbeat ping every **25s** (tuned under CF's ~30s idle timeout).
- **FR-2** — On app-foreground after **≥20s** in background (via the platform seam A04), force `session.reconnect()`.
- **FR-3** — Enforce bounded deadlines: `CONFIRM_JOIN 30s`, `RECONNECT_MAX 60s`, provider `reconnectStuck 45s`.
- **FR-4** — On a deadline breach, surface a clear disconnected/stuck state rather than an infinite spinner.

## 5. Non-functional requirements

- **NFR-1** — Reconnect logic is idempotent; it doesn't spawn duplicate sessions (single-session, K03).
- **NFR-2** — Timers respect app lifecycle (don't fire heartbeats while backgrounded needlessly).

## 6. UX / flow notes

Connection card reflects live/reconnecting/stuck states. Foreground reconnect is automatic; a stuck relay shows an actionable state, not a hang.

## 7. Acceptance criteria

- [ ] **AC-1** — A session survives a >25s idle period via heartbeats.
- [ ] **AC-2** — Foregrounding after ≥20s background triggers a reconnect.
- [ ] **AC-3** — A dead relay surfaces a stuck state within the deadline, not an infinite spinner.

## 8. Out of scope / non-goals

- Pairing/fingerprint — **K01/K02**; single-session/restore — **K03**.

## 9. Dependencies, risks & open questions

- **Risk:** relay/CF timeout changes could require re-tuning the 25s ping.
- **Open question:** None.

## 10. Source anchors

- `src/services/walletpair-transport.ts:513-587` — heartbeat, foreground reconnect, deadlines.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 89.
