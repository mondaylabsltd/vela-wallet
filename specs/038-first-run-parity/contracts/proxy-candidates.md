# Contract — how the desktop gets out of the machine

`executor/proxy.rs` is the only HTTP agent factory. It answers with the agent
for the CURRENT candidate; a transport failure advances the candidate.

## Candidates, in order

| # | Source | macOS | Linux | Windows |
| --- | --- | --- | --- | --- |
| 1 | **System** | `scutil --proxy`: HTTPS → HTTP → SOCKS (as `socks5h`), `*Enable=1` and port ≠ 0; `ExceptionsList` → no_proxy | GNOME `gsettings` (exists) | WinINET via ureq (exists) |
| 2 | **Environment** | `ALL_PROXY` / `HTTPS_PROXY` / `HTTP_PROXY` via `Proxy::try_from_env()`; SOCKS5 forced to resolve remotely (exists) | same | same |
| 3 | **Direct** | no proxy | same | same |

A candidate that yields no proxy is skipped (e.g. no system proxy → env).
Duplicates collapse (env naming the same host:port as system is one candidate).

## Advancing

- A request whose failure `classify()`s as `network: true` marks the current
  candidate failed and the same request is retried ONCE on the next candidate.
- After the last candidate fails the error is reported as `transport: Local`
  ("this machine could not get out"), never as the service being down.
- Any success resets to candidate 1 on the next derivation.
- The derivation is not cached for the process: it is recomputed after any
  failure and at most once per `PROXY_REDERIVE_AFTER` (60 s) otherwise, so a
  proxy that comes back or a setting that changes is honoured without a
  restart.

## What is reported to the core

| Outcome | `RegistryError` | Core view |
| --- | --- | --- |
| Server answered (any status) | `network: false` | as today |
| A route existed, the service did not answer (timeout/refused at the far end through a working proxy) | `network: true, transport: Remote` | `endpoint_unreachable` (existing) |
| Every candidate refused locally | `network: true, transport: Local` | `transport_failed` (new) — the shell says this machine could not get out |

Loopback targets (`127.0.0.1`, `localhost`) always go direct (existing rule).
