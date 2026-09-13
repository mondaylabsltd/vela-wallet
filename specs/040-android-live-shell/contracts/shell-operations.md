# Contract — what the Android shell answers, per operation (040)

§0 **The rules** (unchanged from 019's contract, now normative for wallet-state
machines on Android):

1. Every operation is answered **exactly once** — a skipped operation leaves
   the core waiting forever.
2. An executor **never throws** for an expected failure; failures route through
   the machine's own failure variant. Classification stays in Rust.
3. Executors hold **no business `if`**. One operation ↔ one call.
4. The `when` over operations is **exhaustive on a sealed hierarchy with no
   `else`**, so a Rust wire change is a Kotlin compile error.
5. In 040 Android has **no network layer**: network-flavoured operations are
   answered *immediately* with the shapes the machine's failure twin defines,
   each marked `// live in 041`. Spec 041 upgrades those arms to real calls;
   nothing else changes.

The web executors are the port source and are named per operation below; where
Android answers differently, the difference is stated and justified.

---

## contacts (`ContactsCore`)

Port source: `app-web/vela-wallet/src/lib/contacts/core/contacts-executor.ts`.

| Operation | 040 Android behaviour | Answers |
| --- | --- | --- |
| `read_store` | read `vela.contacts`, `vela.contacts.dismissed`, `vela.contactGroups` from `VelaStore`; coerce defensively — unsalvageable reads as empty, never as a partial book | `store_loaded { contacts, tombstones, groups }` |
| `write_contacts` | write `vela.contacts`, best effort (a storage error still answers) | `written` |
| `write_dismissed` | write `vela.contacts.dismissed` (an `address → epoch ms` **map**, not a list) | `written` |
| `write_groups` | write `vela.contactGroups` | `written` |
| `load_send_history` | Android has no local transaction store yet — truthfully empty, `// live in 041` | `history_loaded { txs: [] }` |
| `resolve_identity` | fail-closed, no fetch, `// live in 041` | `identity_resolved { address, identity: null }` |
| `classify_recipient` | fail-closed, no fetch, `// live in 041` | `recipient_classified { chain_id, address, code: null }` |

`code: null` is **unknown, not a verdict** — the core decides what an unknown
recipient means, and this shell must not pre-empt it with a guess.

**Stored shape (compatibility contract, camelCase, unchanged across clients)**:
`address`, `name?`, `resolvedName?`, `resolvedSource?`, `kind`, `favorite?`,
`note?`, `txCount`, `lastUsed`, `firstSeen`, `source`.

---

## network_admin (`NetworkAdminCore`)

Port source: `app-web/vela-wallet/src/lib/settings/core/network-admin-executor.ts`.

| Operation | 040 Android behaviour | Answers |
| --- | --- | --- |
| `read_store` | read `vela.customNetworks`, `vela.networkConfig`, `vela.rpcProviders`, `vela.serviceEndpoints`; unreadable → "nothing configured" | `store_loaded { custom_networks, network_configs, endpoints, provider_keys }` |
| `write_custom_networks` | write `vela.customNetworks`, best effort | `written` |
| `write_network_configs` | write `vela.networkConfig`, best effort | `written` |
| `write_service_endpoints` | write `vela.serviceEndpoints` — the **complete merged record**, four camelCase strings, verbatim as the core sent it | `written` |
| `write_rpc_providers` | write `vela.rpcProviders`, best effort | `written` |
| `start_search_debounce` | a real `delay(ms)` on the executor's scope | `debounce_elapsed` |
| `fetch_search_index` | fail-closed, `// live in 041` | `search_index { chains: [] }` |
| `fetch_chain_info` | fail-closed, `// live in 041` | `chain_info { data: null }` |
| `probe_rpc` | fail-closed, `// live in 041` | `probed { reported_chain_id: null, latency_ms: 0 }` |
| `probe_reachable` | fail-closed, `// live in 041` | `reachable { ok: false, latency_ms: 0 }` |
| `rpc_get_code` | fail-closed, `// live in 041` | `code { code: null }` |
| `rpc_call_p256` | fail-closed, `// live in 041` | `p256_call { result: null }` |
| `fetch_service_health` | fail-closed, `// live in 041` | `service_health { body: failed, latency_ms: 0 }` |
| `fetch_fiat_rates` | fail-closed, `// live in 041` | `fiat_rates { body: failed, latency_ms: 0 }` |
| `invalidate_pools` | acknowledged no-op — Android has no pools until 041 | `invalidated` |
| `clear_bundler_cache` | acknowledged no-op | `bundler_cache_cleared` |

Consequences the **core's** rules define (not this shell): saving a custom
network is not gated on a successful probe; every health surface renders its
unknown state. FR-013 forbids dressing any of them as measured.

---

## display_currency (`DisplayCurrencyCore`)

Port source: `app-web/vela-wallet/src/lib/settings/core/currency-executor.ts`.

| Operation | 040 Android behaviour | Answers |
| --- | --- | --- |
| `read_stored_code` | read `vela.displayCurrency`; absent → `null` = "never chose" (never a default code) | `stored_code { code }` |
| `write_stored_code` | write `vela.displayCurrency`, best effort | `code_written` |
| `read_device_currency` | **REAL on Android** — `Currency.getInstance(primaryLocale).currencyCode`, guarded `^[A-Z]{3}$`; regionless locale (`en`) throws `IllegalArgumentException` → `null` | `device_currency { code }` |
| `resolve_rate` | fail-closed, `// live in 041` | `rate_resolved { code, rate: null }` |

Two traps this table is written to prevent:

- **`rate: null` is not `rate: 1`.** `display_currency.rs:50` says so
  explicitly, and the core's seeding decision depends on the difference.
  Answering `1` would make an unpriced currency look priced.
- **`read_device_currency` is where Android differs from web** (research D8).
  Web answers `null` because a browser has no region; Android has one, so
  answering `null` here would be a lie that costs a person the correct default
  currency.

---

## Supersession

For the three machines it covers, this contract is Android's first. Spec 019's
`contracts/shell-operations.md` (onboarding, session, passkey ceremonies)
remains in force unchanged; this document does not touch it.
