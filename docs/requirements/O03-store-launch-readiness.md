# O03 · Store Launch Readiness (iOS / Android Submission)

| | |
|---|---|
| **Epic** | O — Ops, Testing, Store & Meta |
| **Status** | 🚧 In progress / 🔜 Next |
| **Owner** | Shelchin |
| **Depends on** | — |
| **Related** | A02, A03, O02 |

## 1. Summary

Vela runs on the web today; the **native iOS & Android apps** share the same code and are in real-device
testing ahead of an **App Store and Google Play** release. This requirement tracks the submission
readiness gap: signing/config, store assets & copy, and account/testing setup — all while preserving the
privacy (A03) and honesty (A02) posture in store metadata.

## 2. Background & context

Store launch is a distribution gate for the founder (weak-at-distribution context). The Apple org
account is done; remaining blockers are largely operational (Android signing, store assets/copy, Play
account-type/closed-testing) plus config landmines. Store copy must not violate the audit/alpha phrasing
(A02) or over-claim.

## 3. Users & stories

- As a **mobile user**, I want Vela on the App Store / Play, so that I can install it natively.
- As the **founder**, I want a clear submission checklist, so that launch isn't blocked by surprises.

## 4. Functional requirements

- **FR-1** — Native iOS/Android builds from the shared codebase (A04), validated on real devices.
- **FR-2** — Android release signing configured (debug-keystore gap resolved).
- **FR-3** — Store assets + listing copy prepared, consistent with A02 phrasing and A03 privacy (Camera-only permission, no tracking).
- **FR-4** — Play account type + closed-testing track set up; Apple org account (done) used for submission.
- **FR-5** — Resolve known config gaps/landmines before submission.

## 5. Non-functional requirements

- **NFR-1** — Store metadata preserves honesty/audit framing (A02) and privacy claims (A03).
- **NFR-2** — Web wallet remains free; mobile is a paid, region-priced download (business model, A01/clue 35).

## 6. UX / flow notes

Uses `docs/store-submission/` copy + privacy/review docs. No "beta" fear banners in-app (A02).

## 7. Acceptance criteria

- [ ] **AC-1** — Signed iOS and Android release builds are produced from the shared code.
- [ ] **AC-2** — Store listings (assets + copy) pass review with accurate privacy/permission declarations.
- [ ] **AC-3** — Closed-testing track is live ahead of public release.

## 8. Out of scope / non-goals

- App feature work (covered by other epics); engineering process — **O02**.

## 9. Dependencies, risks & open questions

- **Risk:** config landmines (signing, permissions) can block review — resolve pre-submission.
- **Open question:** exact regional pricing for the paid mobile download.

## 10. Source anchors

- `docs/store-submission/privacy-and-review.md`, `docs/store-submission/store-listing-copy.md`, `app.json`, `android/`, `ios/`.
- memory `project_store_launch_readiness`; `getvela.app/src/routes/roadmap/+page.svelte` (native apps 🔜); clue 35.
