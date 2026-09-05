# Contract — i18n Keys

**Expected corpus delta: zero.**

Both screens were built against the corpus and resolve every string through `Loc`:
contacts in spec 018, settings in spec 023. Wiring them to the core changes **which**
strings are chosen, not **which keys exist** — the core returns codes and data, the
live builder resolves them through the same keys the fixture builder resolves.

## Why zero is achievable, and where it could fail

The fixture builders are the inventory. Every string a live builder needs already has
a call site in `ContactsFixtures.swift` or `SettingsFixtures.swift`, because the
fixtures were written to render every drawn state — including the refusal states
(ST10c 不兼容) and the empty states (C3).

The two places a gap could appear:

1. **A core refusal with no fixture.** `network_admin` models refusals the drawings
   may not all cover — a chain-mismatch, a duplicate chain id, an unreachable
   endpoint. Where the core's refusal code has no drawn counterpart, the builder
   maps to the nearest drawn refusal rather than inventing copy, and the mismatch is
   recorded in results.md.
2. **A plural or interpolated count.** The corpus has no ICU; counts are composed the
   way the existing fixtures compose them.

## The process if a key is genuinely needed

Adding one is **not** a one-line change. It is the 6-step corpus process:
the source catalogue, all 14 locales, the regenerated wasm bytes and the fingerprint —
and while any spec holds the corpus open, it is blocked outright.

FR-010 forbids a corpus delta in this feature. A discovered gap is therefore recorded
and routed, never patched in place, and never papered over with a literal string —
which the literal audit (`app-ios/scripts/audit-literals.mjs`) would fail anyway.

## Verification

`node app-ios/scripts/audit-literals.mjs` at every phase boundary, and
`git diff --stat -- rust/crates/vela-core/src/i18n_catalogs/` must be empty at
closeout.
