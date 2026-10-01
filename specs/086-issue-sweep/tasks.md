# Tasks: 086 — Issue sweep

Format: `- [ ] T### [P] [US#] description (files) — test`. `[P]` = a disjoint worktree; the clusters run in parallel.

## Phase 1 — Setup
- [x] T001 Spec, checklist, plan and research skeleton on `086-issue-sweep` (`specs/086-issue-sweep/`).
- [x] T002 Reuse the merged 082/085 worktrees with warm builds, and copy the native artifacts into `core-h` and `core-ef`.

## Phase 2 — US1 Scan to send (cluster A)
- [ ] T010 [P] [US1] #312: an asset on the code's network; a core rule for which asset a scan offers — core tests + web vitest.
- [ ] T011 [P] [US1] #326: the token card on Send opens the picker and keeps the recipient — web.
- [ ] T012 [P] [US1] #332: Android scan shows the recipient first — Android unit; device.
- [ ] T013 [US1] Parity on iOS/desktop for T010–T012 — iOS/desktop tests.

## Phase 3 — US2 Contacts (cluster D)
- [ ] T020 [P] [US2] #334 + #310: Edit beside the name; Edit/Delete with the content — web screenshots at 1440 and 390.
- [ ] T021 [P] [US2] #333: decide import encoding in the core (BOM / UTF-8 / UTF-16 / legacy) — fixtures per encoding on every shell.

## Phase 4 — US3 Android everyday screens (clusters B, C)
- [ ] T030 [P] [US3] #328: the asset sheet reopens after × — Android unit; device, 10 times.
- [ ] T031 [P] [US3] #330: Manage groups is reachable with every group hidden — Android + iOS.
- [ ] T032 [P] [US3] #329: a favorite is never named after an error page (core naming) — core + Android + iOS.
- [ ] T033 [P] [US3] #331: separate the remove control from the amount field — Android + iOS.
- [ ] T034 [P] [US3] #322: Sign Out affordance — Android + iOS.
- [ ] T035 [P] [US3] #321: Receive fits one screen — Android + iOS screenshots.

## Phase 5 — US4 (cluster C)
- [ ] T040 [P] [US4] #314: hierarchy of the backup-keys confirmation — Android + iOS.

## Phase 6 — US5 Extension (cluster E)
- [ ] T050 [P] [US5] #315: dApps get only the signed-in account; accountsChanged on switch — unit + Chrome for Testing.
- [ ] T051 [P] [US5] #317: the signing sheet follows the extension language — unit + Chrome for Testing.

## Phase 7 — US6 iOS Trusted Signer (cluster F)
- [x] T060 [US6] #318: reproduce on the iPhone (page opens, passkey prompt) and on simulators (full round trip); root cause = v0.9.4 predates the URL channel.
- [x] T061 [US6] #318: `TrustedSignerRoundTripDeviceTests` (opt-in), PR #341.

## Phase 8 — Delivery
- [ ] T070 Review every agent branch's diff; device-verify on the Xiaomi and the iPhone; push; open a PR per issue (`Fixes #N`).
- [ ] T071 `results.md`: per issue, the PR, root cause, tests, device evidence and residual risks.
