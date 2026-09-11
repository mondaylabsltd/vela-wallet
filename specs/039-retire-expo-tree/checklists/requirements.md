# Specification Quality Checklist: Retiring the Expo tree

**Purpose**: Validate specification completeness and quality before planning
**Created**: 2026-09-11
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] Focused on what a person meets (the repo they clone, the doc they
      read, the URL they open) and what would break
- [x] Every claim about today is quoted from the code, the docs, or a live
      probe (`curl` of `wallet.getvela.app`), not recalled — the register's
      rows each carry a `path:line` on both sides
- [x] All sections completed; the register is filled, not a placeholder
- [x] The founder's rulings of 2026-09-11 are applied, not paraphrased:
      delete first; the hostname may stay; no compatibility with the Expo
      web build's data

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain — the two package questions
      are stated as defaults the founder can reverse in one line
- [x] Requirements are testable — each SC names what would fail and where
      the evidence lands (`results.md`)
- [x] Success criteria are measurable (`git ls-files`, grep script, gate
      red/green proof runs, a fresh-clone walk of the local-dev doc)
- [x] Edge cases identified: a shared-floor path with a consumer nobody
      grepped; a doc scrubbed of the word but not the meaning; the Pages
      project's next build failing after the merge; a link of ours still
      naming an Expo-only path
- [x] Scope is bounded: the deletion and the docs are the deliverable; the
      owed table is recorded, not built, unless the founder says so; native
      shells untouched
- [x] Dependencies and assumptions identified (frozen Pages deployment,
      analytics script, dev harnesses, package defaults, takeover docs,
      frozen goldens)

## Feature Readiness

- [x] Every functional requirement (FR-391…FR-400) maps to at least one SC
- [x] Journeys covered: the developer cloning the repo, the reviewer reading
      the docs, the visitor on the frozen hostname, the founder running the
      move checklist
- [x] The things a unit test cannot see are called out: the shared-floor
      consumers must be RUN (SC-392); the fresh-clone walk (SC-395)
- [x] `/speckit-plan` run 2026-09-11 (plan.md, research.md D1–D14,
      data-model.md, contracts/expo-residue-check.md, quickstart.md)
- [x] `/speckit-tasks` run 2026-09-11 (52 tasks, four story groups)
- [x] `/speckit-implement` run 2026-09-11 — T001–T049 done; T050 opened as
      draft PR #193 pending the `android` and `ios` CI jobs; T051–T052 done

## Notes

- WalletPair and the remote-inject bridge are not open rulings: the founder
  ruled them out for the web in spec 027 ("不用支持 walletpair 以及 remote
  inject 因为它们不成熟"); the register cites that ruling.
- One correction to the spec's first draft was made during implementation
  and recorded in place: `assets/images/icon.png` and `favicon.png` were the
  icon script's Expo OUTPUTS, not its inputs, and were deleted with the tree.
- The one cost of "delete first" that the spec insists on writing down: the
  Pages deployment freezes at the last Expo commit until the hostname moves.
