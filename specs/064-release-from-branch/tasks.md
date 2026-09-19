# 064 tasks

`[X]` done and verified as noted · `[~]` done, verification pending · `[ ]` open

## Phase 1 — every shell says which build it is (FR-004, FR-005)
- [X] T001 Web + extension: `vite` defines version (manifest) and commit; `fixtures.ts` stops owning them — test forced to `abcdef1` shows `0.9.2 (abcdef1)`
- [X] T002 Desktop: `build.rs` stamps `VELA_GIT_COMMIT`; About reads it when live — binary carries HEAD via the git fallback
- [X] T003 Android honours `VELA_GIT_COMMIT` — generates `abcdef1` with it, HEAD without
- [~] T004 iOS: the archive step passes it and reads the archive's Info.plist back — needs a runner
- [~] T005 Every build job asserts its package carries the commit (D6) — needs runners

## Phase 2 — the release is a push (FR-001..003, FR-006)
- [X] T010 Six packaging workflows: `workflow_call`, no tag triggers, no `release` jobs, dead tag-version steps removed
- [X] T011 `release.yml`: gate → build → publish → `released`
- [X] T012 Gate's version parsing rehearsed locally against the real tree (all four + Cargo.lock = 0.9.2)
- [~] T013 Gate refusals seen on real runners with throwaway branches
- [X] T014 `release-macos-local.sh` takes `vX.Y.Z`, stamps the tag's commit, asserts the binaries carry it
- [ ] T015 **The first real release** — spec §7 is the acceptance test

## Phase 3 — the record (FR-007)
- [X] T020 Runbook: the push-is-the-release procedure; traps 1 and 2 of spec 063 marked retired, trap 3 kept
- [X] T021 Stale `desktop-v*` / `extension-v*` references in workflows and READMEs

## Waits on the founder
- [X] **N2** agreed 2026-09-19: `main` no longer deploys the web wallet
- [ ] **N1** Cloudflare → `vela-wallet-web` → production branch `released` (after the first release creates it)
- [ ] **N3** branch protection for `released`
