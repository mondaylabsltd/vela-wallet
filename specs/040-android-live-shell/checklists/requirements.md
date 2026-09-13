# Specification Quality Checklist: Android Live Shell — Settings & Contacts on the Core

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-05
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
      — *Deviation, deliberate and bounded*: the spec names `vela-core`, its
      machine files and `uniffi` because the feature's entire premise is a
      verified inventory of what already exists in this repository. Naming them
      is evidence, not design: FR-001…FR-019 stay behaviour-level and pick no
      Kotlin library, storage engine or serialization scheme — those are
      plan-level decisions.
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
      — the three user stories and all ten success criteria read as things a
      person can see happen on a phone.
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
      — SC-006 names an ABI because a binary-size budget cannot be stated
      without one; it measures an outcome, not a technique.
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
      — in-scope: three storage-only machines. Out-of-scope, named: 041's read
      path, 042's money path, the camera, visual changes, the unmerged
      navigation repair.
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- The premise correction in **Why** is load-bearing and was verified against
  `origin/main` (file paths, line counts, `impl App for` sites) rather than
  recalled. The prior program memory asserted the same thing; the check was run
  anyway, because the most expensive mistake in the sibling desktop feature was
  an unverified assertion about what the repository contained.
- FR-002 / SC-006 (bridge size) is the one requirement that can fail *before*
  any code is written. It is deliberately the first task in the plan.
