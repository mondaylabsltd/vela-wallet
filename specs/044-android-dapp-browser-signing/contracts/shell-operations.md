# Shell operations — who answers what (044)

Every operation is answered, never skipped; a shell failure answers the
executor's `neutralAnswer` (the 043 pattern).

## dapp_permissions (`BrowserExecutor`)

| Operation | Answer | Source |
| --- | --- | --- |
| `ReadGrant{origin}` | `GrantRead{grant?}` | `vela.perm.<origin>` |
| `WriteGrant{grant}` / `RemoveGrant{origin}` | `Ack` | store |
| `Respond{id, payload}` | `Ack` | `ProviderBridge.deliver` — `{dir:"res", id, result}` or `{…, error:{code, message}}` |
| `EmitEvent{event}` | `Ack` | `{dir:"evt", event, data}` to the page |
| `SettleForwarded{code, reason}` | `Ack` | closes the signing controller's open answer with the error |
| `SaveConnectionRecord{…}` | `Ack` | `vela.transactions` row `type: "connect"` |
| `ForwardToSigning{…}` | `Ack` | `DappRpc.route` → Sign: born `SigningController`; State/Switch/Ack/Read: answered by the shell; Unsupported: 4200 |

Events the shell dispatches: `ProviderRequest{id, method, params_json,
origin, is_main_frame}` (from the bridge), `NavigationStarted{url}` (every
document load; and once at birth with the current URL), `ConsentApproved
{now_ms}` / `ConsentRejected` (the sheet), `BrowserClosed`, `AccountsUpdated`
/ `AccountSwitched` (session), `ChainChanged` (settings / a granted
switch), `RevokeRequested{origin}` (the connections list), `ShellCompleted`.

## explore_sites (`ExploreExecutor`) / browser_history (`BhistExecutor`)

`ReadExplore` → `Loaded{doc}`; `WriteExplore{doc}` → `Written`;
`ReadHistory` → `Loaded{entries}`; `WriteHistory{entries}` → `Written`;
`RemoveHistory{origin}` → `Written`. Keys `vela.explore`, `vela.browserHistory`.

## sign_request (`SignExecutor`)

| Operation | Answer |
| --- | --- |
| `SendResponse{id, transport_id, payload}` | `Responded` — delivered to the tab that owns `transport_id` |
| `CheckBundlerFunding` | `PreCheck` — `RelayClient.probeTreasury` (043) |
| `AttemptSponsorship` | `Sponsorship` — off in the parallel space (043's rule) |
| `SignAndSubmit{…}` | `Submit` — `UserOpSpine` (043's order); `OpSubmitted{user_op_hash}` mid-flight; the receipt's tx hash at the end |
| `PersistRecord{record}` | `RecordPersisted` — `FeedExecutor.writeRecords` (`dapp_tx`) |
| `UpdateRecord{…}` | `RecordUpdated` — `FeedExecutor.patchRecords` |
| `SwitchActiveAccount{index}` | `AccountSwitched` — verified before and after (fail-closed) |

## clear_signing (`ClearExecutor`)

`HttpGet{path}` → `DescriptorFetched`; `RpcEthCall{chain_id,to,data}` →
`RpcAnswer`; `SelectorDbLookup{selector}` → `SelectorCandidates`;
`Timer{ms,token}` → `TimedOut`; `Now` → `Clock`.

## approval_guard (`GuardExecutor`)

`ReadTokenMetadata{chain_id,tokens}` → `MetaResolved`; `ReadErc20Allowance`
→ `AllowanceRead`; `ReadErc20Balance` → `BalanceRead` (pool `eth_call`s).
