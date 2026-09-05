# Results — Android Live Shell (040)

Written as the work lands, not after. Each phase adds its own verdict; the
success criteria are verdicted at the end.

---

## Phase 0 — The size probe (T001–T004) ✅

Four builds, all `cargo ndk -t arm64-v8a --platform 29 build --release`
(`lto = true`, `opt-level = "z"`, `codegen-units = 1`), sizes taken after
`llvm-strip -s`.

| Build | uniffi objects | machines linked | stripped bytes | Δ vs A |
| --- | --- | --- | --- | --- |
| **A** baseline | 3 | 3 | 3,715,432 | — |
| **D** scaffolding probe | 6 (3 duplicates) | 3 | 3,733,192 | **+17,760** |
| **B** this slice | 6 | 6 | 4,786,088 | **+1,070,656** |
| **C** whole program | 24 | 24 | 7,890,696 | **+4,175,264** |

**Unit costs, measured:**

- one uniffi object: **5,920 bytes** (D − A, ÷3)
- one machine, first three added: **356,885 bytes** (B − A, ÷3)
- one machine, remaining eighteen: **172,478 bytes** (C − B, ÷18)

### The gate, and what happened at it

The rule was fixed before the builds ran (research D6): *if B − A exceeds
400 KB stripped, stop and price the multiplexed-bridge alternative.*

**B − A = 1,070,656 bytes. The gate tripped.** So the alternative was priced
with build D rather than with an argument: three extra uniffi objects wrapping
machines *already linked* cost 17,760 bytes between them. Bridge scaffolding is
**1.7%** of the cost the gate was aimed at. Collapsing 21 objects into one
multiplexer would save ~124 KB of 4.18 MB and would replace a compiler-checked
set of classes with a hand-written string switch.

**Verdict: the gate tripped, the alternative was measured, and it does not
exist.** The bytes are the machines' own code — the same 39,630 lines of rules
the web ships as wasm and the desktop links directly. Proceeding.

### The number the program carries

Wiring all 24 machines costs **+4,175,264 stripped bytes per ABI**. The app
ships an App Bundle with three ABI splits, so one person downloads one split:
**≈ +4.2 MB**. This slice spends 1.02 MB of it; specs 041 and 042 inherit
≈ 3.1 MB between them, and the marginal cost per machine is *falling*
(357 KB → 172 KB) as shared serde and crux code amortises.

If that ever needs to come down, the first lead to measure is
`panic = "abort"` in `[profile.release]` — a repository-wide change affecting
all clients, and therefore not this feature's call.

### Reproducing it

`/tmp` scripts are not artefacts; the method is in
[quickstart.md](./quickstart.md) §5, and the registration lists used for builds
B and C are the same ones `rust/crates/vela-core-wasm/src/wallet_state.rs`
already carries, with `bridge_class!` read as `bridge_object!`.

**Working tree left clean**: every probe restored
`rust/crates/vela-core-uniffi/src/onboarding_bridge.rs` on exit, verified with
`git status` after the run.

---

## Corrections to the record (found while planning, before any code)

1. **The premise of the request was wrong, and it matters.** Contacts,
   clear signing and the explorer do *not* need their logic written in
   `vela-core`: `contacts.rs` (1,385 lines), `clear_signing.rs` (4,841) and
   `browser_history.rs` (471) are complete Crux machines on `origin/main`.
   Verified by `impl App for` sites and line counts, not recalled. Retired in
   research D1 so specs 041 and 042 do not re-litigate it.

2. **The Contacts tab is inert on purpose, and this feature is why it stops
   being.** `VelaNavHost.kt:271-280` refuses to navigate there because the
   screen shows fixtures a signed-in person would read as real data. Meanwhile
   Settings' `通讯录` row *does* push to it (`:334`) — so the leak the comment
   was avoiding exists anyway, one tap further in. Recorded in research D11;
   closing it is part of US2, not a follow-up.

3. **Ten settings fields cannot be typed into.** `VelaUrlField` renders a value
   and takes no input; there is no `TextField` of any kind under
   `feature/settings/`. The desktop sibling mis-filed the same finding as
   "blocked on missing artwork" for four features running. It is not blocked —
   `VelaTextField` is in the design system already. Research D7, sequenced as
   Phase 4, *before* the write path that needs it.
