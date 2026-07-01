# N03 · Fault-Injection Harness (`vela.*` Console)

| | |
|---|---|
| **Epic** | N — Settings, Self-Host & Diagnostics |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A04 |
| **Related** | F08, N05, O01 |

## 1. Summary

A built-in **fault-injection harness** exposes `vela.*` web-console commands to **simulate failure
conditions** — RPC down, RPC slow, null price, rate-limit (`vela.rateLimitRpc`) — so failure-state UX
can be validated deterministically. It's a development/testing tool with **no telemetry backend**:
nothing leaves the device.

## 2. Background & context

Resilience (F04/F08) is only trustworthy if its failure states are exercised. Rather than waiting for
real outages, the harness injects faults on demand, letting the founder verify that cached balances,
calm rate-limit UX (F08), and error states behave correctly.

## 3. Users & stories

- As a **developer/founder**, I want to simulate RPC failures on demand, so that I can validate failure-state UX.
- As a **reviewer**, I want reproducible fault scenarios, so that resilience isn't just theoretical.

## 4. Functional requirements

- **FR-1** — Expose `vela.*` console commands to simulate: RPC **down**, RPC **slow**, **null price**, **rate-limit** (`vela.rateLimitRpc`).
- **FR-2** — Injected faults drive the real UI states (F08 cached-balance/rate-limit; E01 null-price "—").
- **FR-3** — The harness is a dev tool; **no data leaves the device** (A03).
- **FR-4** — Faults are toggleable/clearable to restore normal behavior.

## 5. Non-functional requirements

- **NFR-1** — Harness must not ship as an attack surface in production builds (dev/console-gated).
- **NFR-2** — Faults are isolated and reversible.

## 6. UX / flow notes

Driven from the web console; effects appear in the live app (home F08, prices E01). Complements the metrics buffer (N05) attached to bug reports (N04).

## 7. Acceptance criteria

- [ ] **AC-1** — `vela.rateLimitRpc` produces the calm cached-balance UX (F08), not an error banner.
- [ ] **AC-2** — A simulated null price renders "—", not "$0" (E01).
- [ ] **AC-3** — Clearing faults restores normal behavior.

## 8. Out of scope / non-goals

- Rate-limit UX itself — **F08**; metrics buffer — **N05**; parallel test env — **O01**.

## 9. Dependencies, risks & open questions

- **Risk:** harness exposure in production — keep it dev/console-gated.
- **Open question:** None.

## 10. Source anchors

- `src/services/dev/` (fault-injection), `src/services/metrics.ts`.
- memory `project_fault_injection_harness`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 95.
