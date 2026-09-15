# Requirements checklist — 053

- [ ] Every FR has at least one test or one device step that would fail if it
      regressed.
- [ ] FR-002: a test compares the bundled provider bytes with the web tree's.
- [ ] FR-004: a test compares the Swift routing sets with `protocol.js`.
- [ ] FR-009: `grep` finds exactly one call site building a user operation for
      a dApp request, and it is `UserOpSpine`.
- [ ] FR-011: a test drives an unknown method and asserts 4900 and that no
      endpoint was asked.
- [ ] FR-014: no `if` in any of the six executors decides meaning.
- [ ] FR-016: a test asserts the dispatched origin comes from the web view's
      URL and not from the envelope, including when the envelope lies.
- [ ] FR-017: one test proves calldata == SafeOp preimage == WebAuthn
      challenge.
- [ ] FR-018: results.md marks every SC device-verified / test-only / owed.
- [ ] Drift: one `CoreWireDriftTests` case per new machine, all six.
- [ ] The three invariant diffs are empty at closeout.
