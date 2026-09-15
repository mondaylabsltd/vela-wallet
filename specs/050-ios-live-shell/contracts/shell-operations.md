# Contract — What iOS Answers for Every Operation

Twenty-seven operations across three machines. **Every one is answered exactly once**
(FR-002). Nothing throws out of an executor: a failure is the core's own result
variant, chosen so the machine's next decision is the one it would make if the shell
had genuinely observed that outcome.

Operations and results are internally tagged, `snake_case`:

```jsonc
// operation, from the core
{ "type": "write_contacts", "contacts": [ … ] }
// result, back from the shell
{ "type": "written" }
```

**An unrecognised `type` must still answer.** Swift cannot check a JSON tag for
exhaustiveness the way Rust checks a `match`, so a machine that grows an operation
would otherwise hang forever with no diagnostic. The executor's `default:` branch
logs loudly and answers the operation's failure-shaped variant where one exists,
falling back to the machine's most neutral result.

Legend: **L** live in 050 · **C** fail-closed, `// live in 051` or `052`.

---

## `contacts` — 7 operations

| | Operation | Answer |
|---|---|---|
| L | `read_store` | `store_loaded { contacts, tombstones, groups }` from `vela.contacts`, `vela.contacts.dismissed` (object→list pivot), `vela.contactGroups`. Corrupt or absent → the empty triple. |
| L | `write_contacts { contacts }` | `written`. Best-effort; a storage failure is still `written`, matching the core's note that "the in-memory ledger stays authoritative". |
| L | `write_dismissed { tombstones }` | `written`. List→object pivot on the way out. |
| L | `write_groups { groups }` | `written`. |
| C | `load_send_history` | `history_failed` — a **modelled** failure, not an empty list. An empty history would tell the core "this person has never sent to anyone", which is a claim; `history_failed` says "I could not look", which is the truth until 052 has a transaction store. |
| C | `resolve_identity { address }` | `identity_resolved { address, identity: null }` — unknown, never invented. |
| C | `classify_recipient { chain_id, address }` | `recipient_classified { chain_id, address, code: null }`. `null` is *unknown*; the core's invariant ⑦ is that unknown is never a false alarm, so this must not be `""` (which would mean "definitely an EOA"). |

## `display_currency` — 4 operations

| | Operation | Answer |
|---|---|---|
| L | `read_stored_code` | `stored_code { code }` from `vela.displayCurrency`. **Absent → `null`**, which the core reads as "never chose" — not as USD. |
| L | `write_stored_code { code }` | `code_written`. |
| L | `read_device_currency` | `device_currency { code }` from `Locale.current.currency?.identifier`. `null` for a regionless locale. This was a carried debt on desktop (a region→ISO-4217 table); iOS gets it from the platform. |
| C | `resolve_rate { code }` | `rate_resolved { code, rate: null }`. **`null` is not `1`** (FR-008). The screen renders the core's degraded presentation. |

## `network_admin` — 16 operations

| | Operation | Answer |
|---|---|---|
| L | `read_store` | `store_loaded { custom_networks, network_configs, endpoints, provider_keys }`. `endpoints` is the **partial** blob — absent fields stay absent so the core applies its own defaults merge. Read through `AccountStore` (data-model §5). |
| L | `write_custom_networks { networks }` | `written` |
| L | `write_network_configs { configs }` | `written` |
| L | `write_service_endpoints { endpoints }` | `written`. Through `AccountStore`, merging rather than replacing — the two-writer contract. |
| L | `write_rpc_providers { keys }` | `written`. A cleared key is **removed**, not stored empty. |
| L | `start_search_debounce { ms }` | `debounce_elapsed` after `Task.sleep(ms)`. Cancellation is the driver's: a superseded debounce is cancelled and **not** answered (CoreDriver property 3). |
| L | `fetch_search_index` | `search_index { chains }`. Network failure → `search_index { chains: [] }` — the core then shows its own "no results" rather than a lie. |
| L | `fetch_chain_info { chain_id }` | `chain_info { chain_id, data }`; failure → `data: null`. |
| L | `probe_rpc { url }` | `probed { url, reported_chain_id, latency_ms }` via `eth_chainId`. Failure or garbage → `reported_chain_id: null` with the measured latency. **This is the operation the add gate depends on**: `add_confirmed` refuses a candidate whose compatibility was never verified, so a probe-less settings screen could never add a network. |
| L | `probe_reachable { url }` | `reachable { url, ok, latency_ms }` |
| L | `rpc_get_code { url, address }` | `code { url, address, code }`; failure → `code: null` (unknown, per invariant ⑦). |
| L | `rpc_call_p256 { url }` | `p256_call { url, result }` — `eth_call` against the RIP-7212 precompile `0x…0100` with the known-good fixture calldata; failure → `result: null`. |
| L | `fetch_service_health { field, base_url }` | `service_health { field, body, latency_ms }` with `body` one of `failed` / `http_error { status }` / `identity { service, status }`. |
| C | `fetch_fiat_rates { url }` | `fiat_rates { body: { type: "failed" }, latency_ms: 0 }` until 051 owns the price path. |
| C | `invalidate_pools { chain_id }` | `invalidated` — an acknowledged no-op; there is no pool until 051. |
| C | `clear_bundler_cache { chain_id }` | `bundler_cache_cleared` — acknowledged no-op until 052. |

---

## Why the probes travel with this executor

They look like they belong to 051's RPC pool and they do not. FR-007 draws the line at
*"its own self-contained HTTP call whose answer gates a user-visible outcome"*, and
`probe_rpc` is exactly that: the core refuses to add a chain it never verified, so
without the probe the wizard is decoration. The pool exists to **route** repeated
reads for a person's own money — a different job, with a ban map and a race. Web
revised its own D1 to the same conclusion and shipped it.

## The failure contract, restated

1. Nothing propagates outward. An executor that throws leaves the core waiting.
2. A failure answer is chosen for **what it makes the core do next**, which is why
   `history_failed` beats an empty list and `code: null` beats `""`.
3. A cancelled effect is **not** answered — the core abandoned it, and a late answer
   would push stale state into a machine that moved on.
