# Data model — spec 079

Three pieces of state each client holds per tab or per request. Their *rules* are the core's
(contracts/core-rules.md); the state lives in the shell because it is about a platform view.

## Load attempt (per tab)

| Field | Meaning |
|---|---|
| `requestedUrl` | what the person or the page asked to load |
| `phase` | `idle` → `requested` → `committed` → `finished`, or `failed(class, reasonKey)` |
| `retrying` | an automatic or manual retry is running (the panel stays, says so) |
| `attempt` | automatic attempts made for the current failure (resets on a new navigation or a tap) |
| `progress` | 0–100, shown from `requested` |

Transitions:

```
idle ──load/reload/link──► requested ──commit──► committed ──finish──► finished
                               │                     │
                               └──failure────────────┴──► failed(class) ──timer/Retry──► requested (retrying)
failed(class) ──new navigation──► requested (attempt = 0)
any ──tab leaves the front──► timers paused (state kept)
```

Rules: `failed` is shown as the Vela panel, never the engine's page; `requested` shows progress
and keeps the committed host in the address bar; a visit is recorded only through
`visit_to_record` at `finished`.

## Signing status (per forwarded request)

| State | Source | Shown as |
|---|---|---|
| `form` | request open, not approved | the request + fee + slide (or the trusted-signer button) |
| `signing` | `SignView.is_signing` | status: spinner, "签名中…" |
| `signed` (message) | answer sent | tick, "已签名", closes on its own |
| `submitting` | `SignView.is_submitting` | spinner, "正在提交…" |
| `waiting` | `pending_op_hash` set, entry `outcome = Landing` | clock + ring against the chain's usual time, "已提交 — 等待链上确认" |
| `stillConfirming` | entry `outcome = StillConfirming` | clock, the still-confirming sentence, closable without rejecting |
| `landed` | tracker confirmed / core cleared the sheet with a hash | tick, short hash + explorer link, closes on its own |
| `failed` | `SignView.error` or a terminal failed status | cross + plain reason |
| `unknown` | entry `outcome = Unknown` | the unknown sentence (explorer) |

Close control: `form` → reject once (4001); every later state → close only, no answer.

## Chain notice (per tab in front)

| Field | Meaning |
|---|---|
| `chainId` | the core's chain for the tab's origin |
| `shown` | `chainId ∈ failed_chains ∧ chainId ∉ rate_limited_chains` |

Retry re-asks one read on that chain through the pool (`eth_blockNumber`); an answer clears the
set and the notice with it.
