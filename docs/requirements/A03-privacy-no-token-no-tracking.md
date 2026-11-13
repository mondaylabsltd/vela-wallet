# A03 · Privacy, No-Token & No-Tracking Principles

> **Superseded (R23): the code contradicts FR-3 and §1 in part. The R21 analytics text (opt-in, off by default, no transaction-outcome event) matches the code. What does not: servers persist more than "the passkey public key + account name" — the multisig genesis service and its mailbox store multisig geneses, proposals and approvals (unauthenticated, CORS `*`), and the relay keeps submitted operations for a limited time; and §1's "the only device permission is Camera" contradicts FR-1's own list.**
>
> **What is true:** `app-web/vela-wallet/src/lib/analytics/consent.svelte.ts`, `app-web/vela-wallet/src/lib/analytics/catalog.ts`; `docs/security/hardening.md` §3, §10; `docs/SECURITY-MODEL.md` §11 (privacy); getvela.app's privacy page (`app-web/getvela.app/src/routes/privacy/+page.svelte`). Checked against the code at commit `fa6083c87` (R23, 2026-11-09). This document is kept as history; it does not describe the product.

| | |
|---|---|
| **Epic** | A — Product Foundations & Cross-Cutting |
| **Status** | ⚠️ Superseded (R23, 2026-11-09): see the note above |
| **Owner** | Shelchin |
| **Depends on** | A01 |
| **Related** | A02, B08, N05, N04 |

## 1. Summary

Vela collects **no accounts, no email, no KYC, and no tracking**. The apps ship with **zero
analytics/crash/tracking SDK**; the only device permission is Camera (for QR). The sole data stored
server-side is the passkey public key + chosen account name — and that is *published on-chain on
Gnosis by design* (B08). There is no token and never will be. These are hard product constraints, not
preferences.

**The one exception, opt-in (R21, §35, decided 2026-11-08).** The web wallet and the browser
extension can send anonymous usage statistics (screen paths with identifiers masked, named moments
such as "wallet created", coarse properties) to Rybbit — **only if the person switches it on**
(Settings → Advanced / Privacy, "Share anonymous usage statistics"). Off by default; a browser that
never chose is off (before R21 the default was on, and nothing was stored for it). **No event tells how
a transaction ended** — `send_submitted`, `send_confirmed`, `send_failed`, `backup_keys_*` and a
site's transaction request are not counted — and a site's approved request (a signature the site may
take on chain itself) is counted bare, with no kind and no network (final review, 2026-11-12). No event
carries an address, an amount or a hash. Every hit still has a time and the IP every web request
carries. The native apps carry none.

## 2. Background & context

A privacy wallet that phones home contradicts its own thesis. Diagnostics therefore stay on-device
(N05) unless the user explicitly files a bug report (N04). "No token, ever" is a deliberate
anti-scam / anti-speculation signal that counter-positions against airdrop-bait wallets.

## 3. Users & stories

- As a **privacy-conscious user**, I want no identity, tracking, or telemetry, so that using the wallet leaks nothing about me.
- As a **cautious user**, I want assurance there's no token to buy or farm, so that I know Vela isn't a speculation vehicle.

## 4. Functional requirements

- **FR-1** — App bundle contains **no analytics/crash/tracking dependency**; `NSPrivacyTracking=false`. The web wallet's own usage statistics are not an SDK (the wallet's code POSTs a masked payload) and are opt-in: off until the person switches them on, never sent from a browser that has not (`analytics/consent.svelte.ts`). Permissions are limited to what a declared feature needs: camera (QR), photo-library add (save a receive code), vibrate and notifications on Android, and Bluetooth for the hybrid/caBLE passkey transport. None of them feeds a tracker.
- **FR-2** — No account, email, phone, or KYC is ever collected or required.
- **FR-3** — Servers persist only the passkey public key + account name (on-chain by design, B08); nothing else about the user.
- **FR-4** — Diagnostics are in-memory only (N05); **no data leaves the device** unless the user files a report (N04) or, on the web, switches usage statistics on.
- **FR-5** — **No token / airdrop / farming** anywhere in the product or roadmap.
- **FR-6** — No usage event tells how a transaction ended, nor that a site asked for one (`analytics/catalog.ts` `TX_OUTCOME_EVENTS`, `TX_REQUEST_KINDS`, held by `payload.test.ts`).

## 5. Non-functional requirements

- **NFR-1** — Verifiable: the "no tracking" claim is checkable from each shell's dependency manifest (`app-web/vela-wallet/package.json`, `rust/Cargo.toml`, the Gradle and SPM files) and the native config, all open source.
- **NFR-2** — The marketing site's analytics are cookieless (Rybbit). The web wallet and extension use the same tool only when the person opts in; the native apps have none.

## 6. UX / flow notes

No consent prompt interrupts anyone: the native apps have nothing to consent to, and the web wallet's usage statistics are a switch in Settings that starts off and says what it sends ("Off unless you turn it on…"). Bug report (N04) explicitly states what sanitized data it will send before sending.

## 7. Acceptance criteria

- [ ] **AC-1** — A dependency scan finds no analytics/crash/telemetry SDK in the app.
- [ ] **AC-2** — Every permission in the native manifests traces to a shipped feature, and none to a tracker; `NSPrivacyTracking=false`.
- [ ] **AC-3** — With no bug report filed, the app makes no request to any first-party telemetry endpoint; the web wallet sends no usage statistics from a browser where the person has not switched them on (`consent.svelte.test.ts`).
- [ ] **AC-4** — No usage event tells a transaction's outcome, and a site's approved request carries neither its kind nor its network (`payload.test.ts` "transaction outcomes").

## 8. Out of scope / non-goals

- The bug-report data contract — see **N04**; the metrics buffer — see **N05**.

## 9. Dependencies, risks & open questions

- **Risk:** a transitive dependency introduces telemetry — needs a periodic dependency audit (O02).
- **Open question:** None.

## 10. Source anchors

- `package.json` — absence of analytics SDKs.
- `docs/store-submission/privacy-and-review.md` — permissions & privacy posture.
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 8, 30.
