# 100 — Data model

Everything here is core (`vela-core`) state or wire. Shells hold engines, carry messages between the
two machines and draw. New wire fields are `#[serde(default)]`; new enum variants land in every
shell's mirror in the same commit (Android's typed enums refuse a whole view on an unknown name,
iOS's `String` enums throw).

## `dapp_rpc` (pure)

| Item | Shape | Rule |
|---|---|---|
| `DappChainAsk` | `{ chain_id: u32, chain_name: Option<String>, native_symbol: Option<String>, rpc_urls: Vec<String>, refused_rpc_urls: u32, explorer_url: Option<String> }` | What `add_chain_ask` read from `[AddEthereumChainParameter]`: texts cleaned (≤ 64 chars, no control / bidi-override), `rpc_urls` the first four usable (deduped), `explorer_url` the first https one |
| `add_chain_ask(params, debug_mode)` | `Result<DappChainAsk, &'static str>` | `Err` = -32602 words: no `chainId`; `nativeCurrency` not an object; `decimals` ≠ 18; `rpcUrls` / `blockExplorerUrls` not lists of strings |
| `usable_rpc_url(url, debug_mode)` | `bool` | https, or http where `offers_wallet` allows; no whitespace, no userinfo, ≤ 512 chars |
| `url_host(url)` | `Option<String>` | host[:port] for a sheet line |
| `DappAddOutcome` | `added { chain_id } · declined · not_compatible · bad_rpc · busy` (tag `type`) | how the sheet ended |
| `add_outcome_error(outcome, chain_id)` | `Option<(i64, String)>` | `None` = `null`; 4001 / 4902 / -32602 / -32002 (research R4) |
| `ADD_NOT_COMPATIBLE` = 4902, `ADD_BUSY` = -32002 | consts | |

## `dapp_browser`

| Item | Change |
|---|---|
| `Model.adding: Option<AddJob { tab, doc, id, origin, chain_id }>` | the one add request on the sheet |
| `OpenKind::AddNetwork` | the tab is `busy` while it is open (099 R2: never suspended) |
| `DbrOperation::ForwardToAddNetwork { tab, id, origin, ask: DappChainAsk }` | open the sheet; answered once by `AddNetworkAnswered` |
| `DbrOperation::CancelAddNetwork { tab, id }` | the page left; close without answering |
| `Event::AddNetworkAnswered { tab, id, outcome: DappAddOutcome, now_ms }` | the sheet's outcome |
| `DbrView.adding_network: Option<DbrSigningView>` | `{ tab, id }` of the open add request (`#[serde(default)]`) |

Routing (`Route::AddChain`): no `chainId` → -32602 · chain in `chains` → switch · `add_chain_ask`
refuses → -32602 · `adding` is some → -32002 · else forward. The row is re-classed `consent` when the
sheet opens.

## `dapp_record`

| Item | Change |
|---|---|
| `DbrReason::NotCompatible` | key `addToken.errorNotCompatible` (Settings' line) |
| `DbrReason::BadRpc` | key `componentsUi.browserStatus.reason.badRpc` |
| `DbrReason::ConsentBusy` | now also the -32002 of a second add; line generalised |
| `DbrRequestClass::Consent` | also an add that opened its sheet |

| Ending | Layer / reason | Code |
|---|---|---|
| added | — (answered) | `null` |
| declined | `sheet / rejected_by_person` | 4001 |
| not compatible | `wallet / not_compatible` | 4902 |
| bad RPC | `wallet / bad_rpc` | -32602 |
| second add | `wallet / consent_busy` | -32002 |
| malformed | `wallet / bad_params` | -32602 |
| page left | `browser / navigated_away` | 4900 |

## `network_admin`

| Item | Shape |
|---|---|
| `Model.dapp_add: Option<DappAdd>` | `{ tab, id, origin, ask, info: Option<NetChainInfo>, from_site, rpc_url, check: WizardPhase, phase: NetDappAddPhase, compat, reported_other }` — beside `wizard`, never in it |
| `Model.dapp_gen` | the generation the page's check answers carry |
| `Event::DappAddRequested { tab, id, origin, ask }` | begin (after the store loads); a second, other request → `Busy` |
| `Event::DappAddApproved { now_iso }` | only from `ready`; `build_custom_network` + `save_custom_network`, then settle `added` |
| `Event::DappAddDeclined` | settle by phase: `not_compatible` → `not_compatible`; `wrong_rpc`/`no_rpc` → `bad_rpc`; else `declined` |
| `Event::DappAddRetried` | only from `check_failed`: the catalog again |
| `Event::DappAddCancelled { tab, id }` | drop, settle nothing |
| `NetOperation::DappAddSettled { tab, id, outcome }` | answer `Written` |
| `NetView.dapp_add: Option<NetDappAddView>` (`#[serde(default)]`) | the sheet |
| `NetDappAddView` | `{ tab, id, origin, host, chain_id, name, native_symbol, rpc_host, explorer_host, from_site, phase, reported_chain_id, compat, can_add }` |
| `NetDappAddPhase` | `checking · ready · not_compatible · check_failed · wrong_rpc · no_rpc` |

```text
requested ─► has it? ── yes ─► settle added
                │ no
                ▼
          FetchChainInfo ──── known ─► catalog RPCs ─┐
                │ unknown                            │
                ▼                                    ▼
        page's usable RPCs ── none ─► no_rpc     probes (must answer the chain id)
                │                                    │ none answered: another id seen & from site → wrong_rpc
                └───────────────► probes ────────────┤ else → check_failed (Retry)
                                                     ▼
                                       getCode × 12 + P256 → ready | not_compatible
```

The check's step functions (`probe_candidates`, `begin_probes`, `probed_step`, `code_step`,
`p256_step`, `contracts_verdict`, `unverified`) are shared with the wizard.

## Corpus

| Key | en |
|---|---|
| `connect.browser.addLead` (new) | {{host}} asks to add a network |
| `connect.browser.addFromSite` (new) | Not in Vela’s network list — the name and coin are the site’s. |
| `componentsUi.browserStatus.reason.badRpc` (new) | The site gave no usable RPC for this network |
| `componentsUi.browserStatus.reason.consentBusy` (changed) | Another request is open |
| `onboarding.login.alertNotFound{Title,Body}` (removed) | read by no client |
