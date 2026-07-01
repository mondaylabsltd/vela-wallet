# A03 · Privacy, No-Token & No-Tracking Principles

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | A01 |
| **Related** | A02, B08, N05, N04 |

## 1. Summary

Vela collects **no accounts, no email, no KYC, and no tracking**. The app ships with **zero
analytics/crash/tracking SDK**; the only device permission is Camera (for QR). The sole data stored
server-side is the passkey public key + chosen account name — and that is *published on-chain on
Gnosis by design* (B08). There is no token and never will be. These are hard product constraints, not
preferences.

## 2. Background & context

A privacy wallet that phones home contradicts its own thesis. Diagnostics therefore stay on-device
(N05) unless the user explicitly files a bug report (N04). "No token, ever" is a deliberate
anti-scam / anti-speculation signal that counter-positions against airdrop-bait wallets.

## 3. Users & stories

- As a **privacy-conscious user**, I want no identity, tracking, or telemetry, so that using the wallet leaks nothing about me.
- As a **cautious user**, I want assurance there's no token to buy or farm, so that I know Vela isn't a speculation vehicle.

## 4. Functional requirements

- **FR-1** — App bundle contains **no analytics/crash/tracking dependency**; `NSPrivacyTracking=false`; only the Camera permission is declared.
- **FR-2** — No account, email, phone, or KYC is ever collected or required.
- **FR-3** — Servers persist only the passkey public key + account name (on-chain by design, B08); nothing else about the user.
- **FR-4** — Diagnostics are in-memory only (N05); **no data leaves the device** unless the user files a report (N04).
- **FR-5** — **No token / airdrop / farming** anywhere in the product or roadmap.

## 5. Non-functional requirements

- **NFR-1** — Verifiable: the "no tracking" claim is checkable from `package.json` and native config (open source).
- **NFR-2** — The marketing site's analytics are cookieless/self-hosted (Rybbit) — separate from the app, which has none.

## 6. UX / flow notes

No consent/tracking prompts exist because there is nothing to consent to. Bug report (N04) explicitly states what sanitized data it will send before sending.

## 7. Acceptance criteria

- [ ] **AC-1** — A dependency scan finds no analytics/crash/telemetry SDK in the app.
- [ ] **AC-2** — Native manifests declare only Camera; `NSPrivacyTracking=false`.
- [ ] **AC-3** — With no bug report filed, the app makes no request to any first-party telemetry endpoint.

## 8. Out of scope / non-goals

- The bug-report data contract — see **N04**; the metrics buffer — see **N05**.

## 9. Dependencies, risks & open questions

- **Risk:** a transitive dependency introduces telemetry — needs a periodic dependency audit (O02).
- **Open question:** None.

## 10. Source anchors

- `package.json` — absence of analytics SDKs.
- `docs/store-submission/privacy-and-review.md` — permissions & privacy posture.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 8, 30.
