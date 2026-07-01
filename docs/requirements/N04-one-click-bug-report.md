# N04 · One-Click Bug Report with Sanitized Diagnostics

| | |
|---|---|
| **Epic** | N — Settings, Self-Host & Diagnostics |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | N05 |
| **Related** | N01, A03, N03 |

## 1. Summary

An in-app **report button** posts to **`getvela.app/api/bug-report`** (a backend proxy holding the GitHub
PAT **server-side only**) with **sanitized** metrics + failed-chain context, falling back to a
**prefilled GitHub issue URL**. It's the only path by which any diagnostic data leaves the device, and
only when the user explicitly chooses to file a report (A03).

## 2. Background & context

Vela ships no telemetry (A03), so bug diagnosis needs a user-initiated channel. Sanitized metrics
(N05) give enough context to debug without leaking keys/signatures/calldata. The PAT stays server-side
so the app never embeds a secret; a URL fallback keeps reporting possible if the proxy is down.

## 3. Users & stories

- As a **user hitting a bug**, I want to report it in one tap with useful context, so that it gets fixed.
- As a **privacy-conscious user**, I want to see what's sent and control it, so that I trust the report path.

## 4. Functional requirements

- **FR-1** — A report button posts sanitized diagnostics (N05 metrics + failed-chain context) to `getvela.app/api/bug-report`.
- **FR-2** — The GitHub **PAT is server-side only** (proxy); the app never embeds it.
- **FR-3** — Fall back to a **prefilled GitHub issue URL** (bug.yml) if the proxy is unavailable.
- **FR-4** — Diagnostics are **sanitized** — strip keys/signatures/calldata (N05) before sending.
- **FR-5** — Nothing is sent unless the user files a report (A03); ideally show what will be sent.

## 5. Non-functional requirements

- **NFR-1** — No secret in the client; no data leaves the device except on explicit report.
- **NFR-2** — Robust fallback so reporting works even if the proxy is down.

## 6. UX / flow notes

`BugReportModal` invoked from the quiet Settings Feedback row (N01). Copy states what's included. Feedback stance: helpful, not naggy.

## 7. Acceptance criteria

- [ ] **AC-1** — Filing a report posts sanitized diagnostics via the proxy (no PAT in client).
- [ ] **AC-2** — With the proxy down, the prefilled GitHub URL fallback works.
- [ ] **AC-3** — No keys/signatures/calldata appear in the payload.

## 8. Out of scope / non-goals

- The metrics buffer contents — **N05**; the backend proxy implementation (server-side).

## 9. Dependencies, risks & open questions

- **Risk:** accidental inclusion of sensitive data — sanitization (N05) is the gate; test it.
- **Open question:** rate-limiting abusive reports at the proxy.

## 10. Source anchors

- `src/services/bug-report.ts`, `src/services/feedback.ts`, `src/components/ui/BugReportModal.tsx`.
- memory `project_one_click_bug_report`; `docs/CONTENT-SOURCE-100-CLUES.md` — clue 96.
