# 100 — Core contract

What changes on the wire between `vela-core` and the shells. Types are in
[../data-model.md](../data-model.md). Every rule is decided once in the core; a shell that re-decides
one of them is a bug.

## The relay a shell performs

```text
dapp_browser ── forward_to_add_network { tab, id, origin, ask } ──► shell ──► network_admin: dapp_add_requested { tab, id, origin, ask }
network_admin ── dapp_add_settled { tab, id, outcome } ──────────► shell ──► dapp_browser: add_network_answered { tab, id, outcome, now_ms }   (answer `written`)
dapp_browser ── cancel_add_network { tab, id } ──────────────────► shell ──► network_admin: dapp_add_cancelled { tab, id }
```

A shell answers `forward_to_add_network` / `cancel_add_network` with `ack` at once (like
`forward_to_signing`), and `dapp_add_settled` with `written`. It never interprets `outcome`.

## `dapp_browser`

| Wire | JSON |
|---|---|
| op | `{"type":"forward_to_add_network","tab":"t1","id":"7","origin":"https://app.example","ask":{"chain_id":100,"chain_name":"Gnosis","native_symbol":"xDAI","rpc_urls":["https://rpc.gnosischain.com"],"refused_rpc_urls":0,"explorer_url":"https://gnosisscan.io"}}` |
| op | `{"type":"cancel_add_network","tab":"t1","id":"7"}` |
| event | `{"type":"add_network_answered","tab":"t1","id":"7","outcome":{"type":"added","chain_id":100},"now_ms":…}` |
| view | `DbrView.adding_network: {"tab":"t1","id":"7"} \| null` |

## `network_admin`

| Wire | JSON |
|---|---|
| event | `{"type":"dapp_add_requested","tab":…,"id":…,"origin":…,"ask":{…}}` |
| event | `{"type":"dapp_add_approved","now_iso":"2026-10-04T12:00:00.000Z"}` |
| event | `{"type":"dapp_add_declined"}` · `{"type":"dapp_add_retried"}` · `{"type":"dapp_add_cancelled","tab":…,"id":…}` |
| op | `{"type":"dapp_add_settled","tab":…,"id":…,"outcome":{"type":"declined"}}` → answer `{"type":"written"}` |
| view | `NetView.dapp_add: NetDappAddView \| null` |

`FetchChainInfo`, `ProbeRpc`, `RpcGetCode`, `RpcCallP256`, `WriteCustomNetworks` are executed as for
the wizard — the shell cannot tell whose they are.

## The sheet (every client)

From `NetView.dapp_add` (and only it):

| Part | Words (key) | Shown when |
|---|---|---|
| title | `settingsModals.addNetwork.modalTitle` | always |
| who asks | `connect.browser.addLead` with `host` (+ the site avatar where the consent sheet has one) | always |
| rows | `addToken.labelName` (`name`, or `addToken.chainId` while empty), `addToken.labelChainId`, `addToken.labelNativeToken` (`native_symbol`), `addToken.labelRpcUrl` (`rpc_host`), `addToken.labelExplorer` (`explorer_host`) | a row whose value is known |
| source | `connect.browser.addFromSite` | `from_site` |
| status | `settingsModals.addNetwork.checkingCompatibility` | `checking` |
| | `settingsModals.addNetwork.compatible` (+ `singleKeyOnly` when `!compat.multi_key_ready`) | `ready` |
| | `settingsModals.addNetwork.incompatible` + `incompatibleHint` + `openChainSetupTool` (the shell's chain-setup link) | `not_compatible` |
| | `settingsModals.addNetwork.unableToVerify` + `retry` → `dapp_add_retried` | `check_failed` |
| | `assets.rpcFixWrongChain` (`actual` = `reported_chain_id`, `expected` = `chain_id`) | `wrong_rpc` |
| | `componentsUi.browserStatus.reason.badRpc` | `no_rpc` |
| primary | `settingsModals.addNetwork.addNetworkBtn` → `dapp_add_approved` | `can_add` |
| dismiss | `connect.browser.cancel` (checking / ready / check_failed) or `common.done` (a verdict) → `dapp_add_declined` | always |

The sheet is modal like the consent sheet (not swiped away); closing it by any means sends
`dapp_add_declined` — the core decides what that answers.

## Free functions

- wasm: `dappAddChainAsk(paramsJson, debugMode) → {"ok": DappChainAsk} | {"error": words}`;
  `dappAddOutcomeError(outcomeJson, chainId) → null | {"code", "message"}` (the extension's window).
- UniFFI: none — the native shells reach both machines as JSON bridges already.

## As built (2026-10-04)

The code is the contract; nothing above changed while building it except as noted in
[../results.md](../results.md).
