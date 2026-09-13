# Implementation Plan: Android Read Wiring

**Branch**: `041-android-read-wiring` | **Date**: 2026-09-05 | **Spec**: [spec.md](./spec.md)

## Summary

Land one HTTP/JSON-RPC transport driven by the `rpc_pool` machine, then let the
six read-path machines waiting behind it speak: balances, activity, custom
tokens, token trust, deposit watching and payment requests. Fill in the
eighteen arms 040 left fail-closed, and close the add-network criterion 040
recorded as impossible.

The road built in 040 does not change. A machine is still: one
`bridge_object!` line, one `…Wire.kt`, one executor, one drift-test entry, one
controller method per intent. What is new is that executors can now *ask a
chain a question* — through the pool, and only through the pool.

## Technical Context

**Language/Version**: Kotlin 2.2.10, Compose, AGP 9.3.1, minSdk 29.

**Primary Dependencies**: existing — uniffi, kotlinx.serialization (040),
DataStore, **okhttp** (already present for caBLE; now the pool's transport,
research D1). No new dependency.

**Storage**: `VelaStore` from 040, plus the ban map under the key the other
clients use.

**Testing**: JVM unit tests over the real machines through JNA (040's pattern),
with a fake transport for determinism; instrumented tests for anything needing
real storage across a process death; a device pass for the balance and activity
criteria, which cannot be asserted anywhere else.

**Constraints**: **no component but the pool may issue a chain request**
(FR-101/SC-107). Bridge size measured before the work lands (SC-108). No
business rule in Kotlin (FR-113).

**Scale/Scope**: 7 machines added (Android goes from 6 to 13 of 24); ~1,700
lines of sibling TypeScript as the port reference; 18 fail-closed arms to
upgrade; the wallet home, assets, activity and receive surfaces.

## Constitution Check

Same gates as 040 (the repository's constitution file is an unfilled
template; the normative rules are the sibling specs'). One addition specific to
this feature:

| Gate | How this plan meets it |
| --- | --- |
| Rules live in Rust | FR-113; the pool especially — scoring, bans, verdicts are `rpc_pool.rs` |
| Every operation answered exactly once | one `when` arm per operation, exhaustive over the sealed hierarchy |
| Executors never throw for expected failure | transport failures become the machine's own outcome variants |
| A Rust wire change is a compile error | sealed `when` + `CoreWireDriftTest` |
| **One transport** (new) | the facade is the only holder of an HTTP client; a test asserts no executor gains one |

## Phase plan

Each phase ends green and is a commit.

**Phase 0 — Measure (GATE).** Build the 13-machine bridge, strip, record
against 040's three data points. *(Running before any Kotlin.)*

**Phase 1 — The transport and the pool.** `RpcWire.kt`, `RpcPoolExecutor`
(seven operations: config, post, probe, jitter, backoff, persist, conclude),
the okhttp client, and the facade that owns "the fetch and the promise". The
endpoint seed comes from `network_admin`'s view (research D4). Proof: a fake
transport drives a real `RpcPoolCore` through a failing endpoint and the ban
lands in storage.

**Phase 2 — The ten network arms.** `NetworkAdminExecutor`'s fail-closed arms
become real through the pool: chain info, RPC and reachability probes,
`eth_getCode`, the P-256 precompile call, service health, fiat rates, and the
two no-ops that now act on a real pool. The settings health tiles light up.

**Phase 3 — The rate.** `display_currency`'s `resolve_rate`, which is one
operation and re-proves the road; the currency total starts converting.

**Phase 4 — Balances.** `balance_dashboard`, `manage_tokens`, `token_trust`;
`WalletLive.kt` builds the home's hero, network filter and asset rows. The two
inherited traps (research D6) are checked explicitly, with tests.

**Phase 5 — Activity.** `activity_feed`; the home feed, the activity screens,
and the contact-detail block 040 left empty.

**Phase 6 — Receive.** `receive_watch` and `payment_request`; the receive
flow's deposit watching and EIP-681 handling.

**Phase 7 — Contacts backfills.** Identity resolution, recipient
classification, send history.

**Phase 8 — Add a network.** FR-111: the wizard works now that the chain index
is reachable — the criterion 040 recorded as impossible.

**Phase 9 — Closeout.** `grep -rn 'live in 041'` → zero; device pass for
SC-101…SC-106; results with verdicts, measurements and debts for 042.

## Complexity Tracking

One new subsystem (the transport), and it is confined to one file behind one
facade. No new dependency, no new module, no change to the 040 road. The
alternative — letting each executor fetch for itself — would be simpler to
write and would discard every routing rule the core owns, which is the entire
reason the pool machine exists.
