# N05 · In-Memory Metrics & Failure Ring Buffer (No Telemetry Backend)

| | |
|---|---|
| **Epic** | N — Settings, Self-Host & Diagnostics |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A03 |
| **Related** | N04, N03, F08 |

## 1. Summary

Vela keeps **in-memory `metrics.ts` counters** and a **25-entry failure ring buffer** that attach —
**sanitized** (keys/signatures/calldata stripped) — to a one-click bug report (N04). There is **no
telemetry backend**: the data lives only in memory on the device and never leaves unless the user files
a report.

## 2. Background & context

Debugging without telemetry (A03) requires local diagnostics the user can choose to share. A bounded
ring buffer captures recent failures for context without unbounded memory or any background upload,
keeping privacy intact while making bug reports actionable.

## 3. Users & stories

- As a **user filing a bug**, I want recent failure context attached, so that the report is useful.
- As a **privacy-conscious user**, I want diagnostics kept on-device and sanitized, so that nothing leaks.

## 4. Functional requirements

- **FR-1** — Maintain in-memory counters (`metrics.ts`) for key operations/failures.
- **FR-2** — Maintain a **25-entry ring buffer** of recent failures (bounded; oldest evicted).
- **FR-3** — **Sanitize** entries — strip keys, signatures, and calldata — before they can be included in a report.
- **FR-4** — Attach the sanitized metrics + failure context to a bug report only on explicit user action (N04).
- **FR-5** — **No telemetry backend**; data never leaves the device otherwise (A03).

## 5. Non-functional requirements

- **NFR-1** — Bounded memory (25 entries) — no growth over a session.
- **NFR-2** — Sanitization is applied before any egress path.

## 6. UX / flow notes

Invisible until a bug report (N04) is filed; the fault harness (N03) exercises the counters/buffer during resilience testing.

## 7. Acceptance criteria

- [ ] **AC-1** — Recent failures populate the 25-entry ring buffer (oldest evicted beyond 25).
- [ ] **AC-2** — Sanitization removes keys/signatures/calldata from entries.
- [ ] **AC-3** — No metrics are transmitted without an explicit bug report.

## 8. Out of scope / non-goals

- The report transport — **N04**; fault injection — **N03**.

## 9. Dependencies, risks & open questions

- **Risk:** a new failure path logging sensitive data — sanitization must cover it; audit on change.
- **Open question:** None.

## 10. Source anchors

- `src/services/metrics.ts` — counters + 25-entry sanitized ring buffer.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clue 95.
