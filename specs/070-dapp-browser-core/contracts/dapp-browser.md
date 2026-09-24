# Contract — `dapp_browser` machine + `dapp_rpc` + the injected script

Wire names are snake_case JSON (serde `tag = "type"`), like every machine.

## The page ⇄ shell wire (the bridge)

The script `provider_script(host)` installs, in the TOP frame only:

- the provider (`provider/inpage.js`, unchanged behavior: EIP-1193 + EIP-6963),
- the bridge, which mints `DOC` (a random id) and posts strings to the host:

```jsonc
{"t":"hello","doc":"<DOC>"}                                  // once, before any page script
{"t":"req","doc":"<DOC>","id":"<id>","method":"…","params":…}  // per request
```

- `window.__velaDeliver(json)` (non-writable): parses, drops unless
  `json.doc === DOC`, then `window.postMessage({ch:'vela-1193', dir, id|event, result|error|data}, location.origin)`.

`host` picks only the post function:

| host | post |
|---|---|
| `android` | `VelaHost.postMessage(s)` (a `WebMessageListener`) |
| `ios` | `window.webkit.messageHandlers.VelaHost.postMessage(s)` |
| `desktop` | `window.ipc.postMessage(s)` |

## Events (shell → core)

| Event | Fields | Notes |
|---|---|---|
| `start` | — | asks `list_sites` once |
| `networks_changed` | `chain_ids: [u32]` | chains a site may switch/add to |
| `accounts_updated` | `addresses: [String]?` | every wallet address; `null` = not known yet |
| `account_switched` | `address, now_ms` | every grant follows (re-pin + `accountsChanged`) |
| `page_message` | `tab, frame_origin, is_main_frame, message_json` | `frame_origin` = the platform's origin of the SENDING frame (Android `sourceOrigin`, iOS `frameInfo.securityOrigin`, desktop the webview URL) |
| `navigation_started` | `tab, url` | records a load began; settles nothing by itself |
| `load_finished` | `tab, url, failed` | settles the old document if no `hello` came since the load began |
| `tab_closed` | `tab` | settles + forgets the tab |
| `renderer_gone` | `tab` | settles; tab view `crashed = true` until the next `hello` |
| `consent_approved` | `now_ms` | |
| `consent_rejected` | — | |
| `site_chain_picked` | `origin, chain_id` | the person picked a network in the connection panel |
| `revoke_requested` | `origin` | |
| `revoke_all` | — | Settings → Storage → dApp connections |
| `signing_answered` | `tab, id, payload: SignResponsePayload, user_op_hash?` | the signing pipeline's answer, delivered exactly once |

## Operations (core → shell) and their results

| Operation | Result | Neutral answer on failure |
|---|---|---|
| `list_sites` | `sites_listed {sites: [{origin, grant?, chain_id?}]}` | empty list |
| `write_grant {grant}` / `remove_grant {origin}` / `write_site_chain {origin, chain_id}` | `ack` | `ack` |
| `deliver {tab, doc, message_json}` | `ack` | `ack` |
| `read {tab, id, chain_id, method, params_json, bundler}` | `read_answered {body_json?}` — the JSON-RPC body (`{"result":…}` or `{"error":{…}}`), `null` if nothing answered | `read_answered {null}` → -32603 |
| `resolve_user_op {chain_id, user_op_hash}` | `user_op_resolved {tx_hash?}` | `{null}` → the page gets `null` (pending) |
| `forward_to_signing {tab, id, method, params_json, origin, chain_id, granted_address?}` | `ack` | — (the shell answers later via `signing_answered`) |
| `cancel_signing {tab, id}` | `ack` | `ack` |
| `save_connection_record {address, chain_id, origin}` | `ack` | `ack` |

## View

```jsonc
{
  "ready": true,                       // sites listed
  "consent": {"tab","origin","methods":[…],"address","chain_id"} | null,
  "tabs": [{"tab","origin"?,"connected_address"?,"chain_id","secure","crashed"}],
  "sites": [{"origin","address","chain_id","granted_at_ms"}],   // Settings list
  "signing": {"tab","id"} | null,      // the request whose sheet should be up
  "queued_signing": 0
}
```

## `dapp_rpc` (pure)

- `classify(method) -> Route`: `connect` (eth_requestAccounts,
  wallet_requestPermissions) · `accounts` (eth_accounts) · `coinbase` ·
  `permissions` (wallet_getPermissions) · `revoke_permissions` ·
  `chain_id` · `net_version` · `switch_chain` · `add_chain` · `watch_asset` ·
  `sign` (eth_sendTransaction, wallet_sendCalls, personal_sign,
  eth_signTypedData, _v1, _v3, _v4) · `read {bundler}` (the allowlists,
  byte-equal to `protocol.js`) · `unsupported` (eth_sign and everything else).
- `chain_param(params_json) -> Option<u32>` — `[{chainId:"0x64"|"100"}]`.
- `validate(message_json) -> Result<Request, Invalid{id?, code}>`.
- `browser_input(text) -> String` — URL or DuckDuckGo search (research R6).
- `error_json(id, code, message)`, `result_json(id, value)`, `event_json(name, data)`.
