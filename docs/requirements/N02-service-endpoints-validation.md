# N02 · Service Endpoints Configuration & On-Entry Validation

| | |
|---|---|
| **Epic** | N — Settings, Self-Host & Diagnostics |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A06 |
| **Related** | N01, B08, E05, F07, F03 |

## 1. Summary

Vela is **self-hostable end-to-end**: users configure custom endpoints in **Settings → Advanced →
Service Endpoints**. Each candidate is **validated before acceptance** — HTTPS only, reachable within
10s, and `/api/health` returns the correct `service` id + `status: "ok"`. Services: **Chain Data Index**
(`atshelchin/ethereum-data`), **Passkey Index** (B08), **Bundler** (`atshelchin/vela-bundler`, F07), plus
**FX = Frankfurter** (E05).

## 2. Background & context

"Self-host everything" is a uniquely defensible claim (competitors can't match it). Validation on entry
prevents silently pointing the wallet at a broken/hostile endpoint, and the health contract makes a
candidate prove itself before it's trusted.

## 3. Users & stories

- As a **self-hoster**, I want to point Vela at my own services, so that I depend on no one.
- As a **user**, I want a bad endpoint rejected before it's used, so that I don't break the wallet.

## 4. Functional requirements

- **FR-1** — Let users set custom endpoints for: Chain Data Index, Passkey Index (B08), Bundler (F07), FX (E05).
- **FR-2** — Validate a candidate before accepting: **HTTPS only**, reachable within **10s**, `/api/health` returns the correct `service` id + `status: "ok"`.
- **FR-3** — Persist accepted endpoints (A06); a config change refetches endpoint-keyed caches (E05/A06).
- **FR-4** — Reject and explain an invalid/unreachable/mismatched-service endpoint; keep the previous value.
- **FR-5** — All services are **MIT-licensed and self-hostable** (A01).

## 5. Non-functional requirements

- **NFR-1** — Validation is bounded (10s) and non-blocking to the rest of the app.
- **NFR-2** — HTTPS-only; never accept plaintext endpoints.

## 6. UX / flow notes

`RpcProvidersModal` / Service Endpoints screen: enter URL → validate → accept/reject with a specific reason. RPC provider entries feed the pool as the top tier (F03).

## 7. Acceptance criteria

- [ ] **AC-1** — A healthy self-hosted endpoint (correct `service`/`status`) is accepted.
- [ ] **AC-2** — A non-HTTPS or unreachable or wrong-`service` endpoint is rejected with a reason.
- [ ] **AC-3** — Swapping an endpoint refetches its dependent caches (E05).

## 8. Out of scope / non-goals

- The backend service implementations (separate repos); pool scoring — **F03**.

## 9. Dependencies, risks & open questions

- **Risk:** health-contract drift across service repos — keep `/api/health` shape in sync.
- **Open question:** None.

## 10. Source anchors

- `src/screens/settings/RpcProvidersModal.tsx`, `src/services/network-checker.ts`, `src/services/rpc-providers.ts`.
- `README.md` (Self-Deploy Service Endpoints); `docs/CONTENT-SOURCE-100-CLUES.md` — clues 11, 20.
