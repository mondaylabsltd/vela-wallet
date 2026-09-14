# Shell operations — 053

The 28 operations six machines ask for, what each iOS executor answers, and
what it answers when it cannot find out.

**The four rules** (`specs/024-web-live-shell/contracts/shell-operations.md`):

1. Every operation is answered **exactly once**.
2. An expected failure uses the machine's own failure variant. Executors do
   **not** throw.
3. No business `if`. "Could I read it" is the executor's; "what does that mean"
   is the core's.
4. The `switch` is exhaustive and an unknown tag answers **loudly** — a
   `neutralAnswer` twin gives every arm a safe answer, and the drift test walks
   the whole list.

---

## `explore_sites` — 2 operations · `ExploreExecutor`

Store key `vela.explore`, one JSON document.

| Operation | Answer | Cannot find out |
|---|---|---|
| `read_explore` | `loaded { doc }` | `loaded { doc: null }` — an empty start page, never a failure to dismiss |
| `write_explore { doc }` | `written` | `written` — best effort; the mirror is authoritative |

---

## `browser_history` — 3 operations · `BhistExecutor`

Store key `vela.browserHistory`, a JSON array.

| Operation | Answer | Cannot find out |
|---|---|---|
| `read_history` | `loaded { entries }` | `loaded { entries: [] }` — the TS `catch { [] }`, ported |
| `write_history { entries }` | `written` | `written` |
| `remove_history` | `written`, after **deleting the key** | `written` |

`remove_history` removes the key rather than writing `[]`. That is
`browser-history.ts:99-105` verbatim and the other three clients do the same.

**The machine has no `ready` flag.** A visit dispatched before
`read_history` has answered is dropped, silently. The executor therefore
signals when the load lands and the controller records no visit before it —
Android's phase 0 finding, which iOS inherits unchanged because it is a
property of the core, not of Android.

---

## `dapp_permissions` — 8 operations · `BrowserExecutor`

| Operation | Answer | Cannot find out |
|---|---|---|
| `read_grant { origin }` | `grant_read { origin, grant }` from `vela.perm.<origin>` | `grant_read { origin, grant: null }` |
| `write_grant { grant }` | `ack` | `ack` |
| `remove_grant { origin }` | `ack`, after writing **the empty string** | `ack` |
| `respond { id, payload }` | `ack`, after delivering the envelope | `ack` |
| `emit_event { event }` | `ack`, to the **current** tab | `ack` |
| `settle_forwarded { code, reason }` | `ack`, after erroring every still-open forwarded id | `ack` |
| `save_connection_record { address, chain_id, origin }` | `ack`, after writing the `connect` row | `ack` |
| `forward_to_signing { id, method, params_json, origin }` | `ack`, after handing it to the router | `ack` |

`remove_grant` writes `""`, it does not delete. Whatever reads it must treat an
empty string as **absent**, or a revoked origin comes back granted.

### The reject-message table

Nine reasons, nine sentences, from the shell — the core owns which reason.

`unauthorized_frame` · `no_account_available` · `consent_busy` ·
`insecure_origin` · `user_rejected` · `navigated_away` · `browser_closed` ·
`not_connected` · `stale_authorized_address`

---

## `sign_request` — 7 operations · `SignExecutor`

| Operation | Answer | Cannot find out |
|---|---|---|
| `send_response { transport_id, id, payload }` | `responded` | `responded` |
| `check_bundler_funding { … }` | `pre_check { funding: null }` | same |
| `attempt_sponsorship { … }` | `sponsorship { denied, reason: null }` | same |
| `sign_and_submit { … }` | `submit { outcome, now_ms }` via `UserOpSpine` | `submit { failed }` |
| `persist_record { record }` | `record_persisted`, after the row is on disk | `record_persisted` |
| `update_record { record_id, close }` | `record_updated` | `record_updated` |
| `switch_active_account { index }` | `account_switched` | `account_switched` |

**`check_bundler_funding` answers `null` on purpose.** The core's own doc says
a timed-out or errored pre-check is not a refusal; the submit's underfunded
answer is the authority. Desktop and Android answer the same.

**`attempt_sponsorship` answers `denied` with no reason.** This shell reaches no
sponsorship path, and `funded` would be a claim that somebody else paid.

**`sign_and_submit` reports twice.** The accepted user-op hash mid-flight
through `OpSubmitted`, so the durable record precedes anything the dApp could
poll; then the final outcome. The page is answered with a **transaction** hash
when the receipt arrives inside the wait, and the user-op hash when it does not.

---

## `clear_signing` — 5 operations · `ClearExecutor`

| Operation | Answer | Cannot find out |
|---|---|---|
| `http_get { path }` | `descriptor_fetched { path, json }` — body text on 200 | `descriptor_fetched { path, json: null }` |
| `rpc_eth_call { chain_id, to, data, probe }` | `rpc_answer { probe, chain_id, to, result, rpc_error }` | the same with `result: null, rpc_error: false` |
| `selector_db_lookup { selector }` | `selector_candidates { sigs }` | `selector_candidates { sigs: [] }` |
| `timer { ms, token }` | `timed_out { token }` after `ms` | `timed_out { token }` at once |
| `now` | `clock { now_ms }` | the same |

`rpc_error: true` means **the node answered with an error object** — a revert.
`result: null` with `rpc_error: false` means nobody answered. The core reads
those as different facts and an executor that flattens them makes a contract
that reverts look like a chain that is down.

Selector lookup is openchain first, then 4byte, cached in the executor.

---

## `approval_guard` — 3 operations · `GuardExecutor`

| Operation | Answer | Cannot find out |
|---|---|---|
| `read_token_metadata { chain_id, tokens }` | `meta_resolved { metas }` — one Multicall3 `aggregate3` | `meta_resolved { metas: null }` |
| `read_erc20_allowance { … }` | `allowance_read { allowance }` decimal string | `allowance_read { allowance: null }` |
| `read_erc20_balance { … }` | `balance_read { balance }` decimal string | `balance_read { balance: null }` |

`metas: null` (the whole read failed) and a token **missing from** `Some(list)`
are different: the core's fallbacks differ. An empty `tokens` list answers
`Some([])`, not a failure.

---

## The router is not an executor

`DappRpc` and `RequestRouter` sit **outside** this table. They serve
`forward_to_signing`'s non-signing siblings — the reads, the state answers, the
chain switch — which the permissions machine routes to the shell rather than
deciding itself. Their contract is the allowlist, and the allowlist is checked
against `extension/lib/protocol.js` by a test that fails when they disagree.

| Route | Who answers | Shape |
|---|---|---|
| `sign` | `sign_request` | forwarded |
| `state` | the shell, from the dperm view's chain | `eth_chainId` **hex**, `net_version` **decimal** |
| `switch` | the settings machine's chain rows | `-32602` unparseable · `4902` unknown · else switch and `result: null` |
| `ack` | nobody | `result: null`, nothing changed |
| `read` | `RpcPool`, node or bundler | the node's `result`, or its error verbatim |
| `unsupported` | the shell | `4900` — **not** 4001; the person did not decline |

`eth_sign` is refused **before** the signing test that would otherwise catch it.
It is policy, not a missing feature.
