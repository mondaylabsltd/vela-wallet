# Contract — what the web shell answers, per operation (024)

§0 **The rules** (unchanged from 019's contract, now normative for
wallet-state machines on web):
1. Every operation is answered exactly once — a skipped operation leaves the
   core waiting forever.
2. `execute` never rejects for an expected failure; rejections route through
   the machine's `…OperationFailure(effect)` twin into the variant that
   operation answers with. Classification stays in Rust.
3. Executors hold no business `if`. One operation ↔ one call.
4. Exhaustive `switch` with a `never` fallthrough, so a Rust wire change is a
   compile error.
5. In 024 the web has **no network layer**: network-flavoured operations are
   answered *immediately* with the same shapes the failure twin defines
   (research D1). Spec 025 upgrades those arms to real calls; nothing else
   changes.

The Expo failure twins are the authoritative shape reference
(`src/services/wallet-state-core/network-admin-executor.ts:528`,
`contacts-executor.ts:280`, `executors.ts:78`); the web executors are ports.

## contacts (`ContactsCore`)

| Operation | 024 web behaviour | Answers |
| --- | --- | --- |
| `read_store` | read `vela.contacts` + `.dismissed` + `vela.contactGroups` from KV; coerce defensively (unsalvageable → empty, as the TS `catch { [] }`) | `store_loaded` |
| `write_contacts` / `write_dismissed` / `write_groups` | KV write, best-effort (storage error still answers) | `written` |
| `load_send_history` | ~~no web tx store yet → truthfully empty~~ **028 US5**: `records.loadTransactions()` (the 025 `vela.transactionHistory` store) mapped row-for-row — `type`→`kind` (absent stays `null`), `to`, `toName`, seconds→ms; which rows COUNT stays `contacts.rs`'s | `history_loaded { txs }` |
| `resolve_identity` | fail-closed, no fetch | `identity_resolved { identity: null }` |
| `classify_recipient` | fail-closed, no fetch | `recipient_classified { code: null }` (unknown ≠ verdict) |

**028 US5 — the book as a file.** No operation was added. Files reach the
core as EVENTS (`import_file { content, filename, into_group, now_ms }`) and
leave it as a VIEW field (`export: ContactExportFile | null`, cleared by
`export_taken`): the shell's whole part is `pickTextFile` in and
`saveTextFile` out. Format (JSON/CSV sniffing, quoting, the address-column
heuristics) is `contacts_io.rs`'s; policy (existing-wins, the counts, seating
into a group) is `contacts.rs`'s. The refusals (`import_failure`) are the
core's too — a shell shows `importFailTitle/Body` and acknowledges.

## network_admin (`NetworkAdminCore`)

| Operation | 024 web behaviour | Answers |
| --- | --- | --- |
| `read_store` | KV reads; `serviceEndpoints` via the onboarding localStorage helper (D3a); unreadable → "nothing configured" | `store_loaded` |
| `write_custom_networks` / `write_network_configs` / `write_rpc_providers` | KV write, best-effort | `written` |
| `write_service_endpoints` | localStorage helper (D3a), best-effort | `written` |
| `start_search_debounce` | real `setTimeout` | `debounce_elapsed` |
| `fetch_search_index` | fail-closed | `search_index { chains: [] }` |
| `fetch_chain_info` | fail-closed | `chain_info { data: null }` |
| `probe_rpc` | fail-closed | `probed { reported_chain_id: null, latency_ms: 0 }` |
| `probe_reachable` | fail-closed | `reachable { ok: false, latency_ms: 0 }` |
| `rpc_get_code` | fail-closed | `code { code: null }` |
| `rpc_call_p256` | fail-closed | `p256_call { result: null }` |
| `fetch_service_health` | fail-closed | `service_health { body: { type: 'failed' }, latency_ms: 0 }` |
| `fetch_fiat_rates` | fail-closed | `fiat_rates { body: { type: 'failed' }, latency_ms: 0 }` |
| `invalidate_pools` | acknowledged no-op (no pools on web yet) | `invalidated` |
| `clear_bundler_cache` | acknowledged no-op | `bundler_cache_cleared` |

Consequences the core's verbatim-ported rules define (not this shell):
saving a custom network is not gated on probe success; health surfaces render
their unknown/unreachable states. The settings screen keeps its RPC-health
tiles fixture-fed in 024 regardless (spec FR-001).

## display_currency (`DisplayCurrencyCore`)

| Operation | 024 web behaviour | Answers |
| --- | --- | --- |
| `read_stored_code` | KV read | `stored_code { code }` (absent → null = "never chose") |
| `write_stored_code` | KV write, best-effort | its ack |
| `read_device_currency` | web has no device region by core rule | `null` |
| `resolve_rate` | fail-closed, no fetch | `rate: null` (≠ 1: formatting degrades to USD figure, conversion refuses) |

## Supersession

For the machines it covers, this contract supersedes nothing — it is their
first web contract. 019's `contracts/shell-operations.md` (onboarding/session)
remains in force unchanged.
