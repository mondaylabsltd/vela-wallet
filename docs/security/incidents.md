# Security incidents (internal)

Events in Vela's own development or operations that touched a production system in a way they
should not have, but put no user's keys or funds at risk. These are not vulnerabilities in the
released wallet; those are in [`advisories/`](advisories/). We keep this record so the
process failures are as visible as the code ones.

## 2026-11-07: a test build may have written a record to the production key index

**What happened.** During Round 17 of the multisig acceptance testing, an iOS simulator build with
dev fixtures was creating a throwaway passkey account. At 2026-11-06 20:25:12 UTC
(2026-11-07 04:25 UTC+8), it connected to the **production** key index,
`p256-index-v2.getvela.app`. The app was at the step "Writing the key index". The build's
`passkeyIndexURL` had not been pointed at a local stand-in. The tester's helper blocked the host
31 seconds later. About 90 seconds after that, the app returned to "Add passkeys" with the key
still awaiting "Confirm".

**Status: unconfirmed.** The registration probably did not complete, but we cannot rule out that
it did. The key index writes registrations to its on-chain registry contract. A completed
registration would be a permanent public record of a test-only public key. That key controls no
funds, and the record cannot be deleted.

**Impact.** No user data was read or changed, and no funds were involved. At worst there is one
extra, meaningless record in a public, append-only index.

**Cause.** Nothing in a test build stopped it from writing to production by default. The rule
"no production writes during testing" lived in a tester's checklist, not in the build. This was
the second such event. On 2026-11-01, real-device pass 1's Android build had recorded a multisig
genesis in the production multisig service. That one service was fixed in `ab5aad2a2`.

**Fix.** Merge `0e38b44434d67b1bf5d0d58f9959aab932409bc8` (2026-11-07, "Merge no-prod-writes",
[`docs/multisig-machines.md` §28](../multisig-machines.md#28-test-builds-never-write-to-production)):

- one core source of every production service default (`vela_core::service_defaults`);
- every **test build** on every platform refuses a write to a production host before the request
  leaves the device. Test builds are dev-fixtures, debug builds, the device-test builds, the
  web's dev server and the extension's development package;
- only `VELA_ALLOW_PRODUCTION_WRITES=1`, which is off by default, lets a test build write to
  production. Release builds are never refused;
- the local test harness starts its own key index;
- the rule is pinned by a shared vector (`rust/crates/vela-core/tests/vectors/rules/production-writes.json`) and by each
  shell's guard tests.

**Open.** The owner should check the production index for a record created around 2026-11-06
20:25 UTC by the Round 17 test account.
