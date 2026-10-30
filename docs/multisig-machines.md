# Multisig machines — the API the shells drive

2026-10-31 · W9. For the native lanes (desktop, Android, iOS) and the web.

**The rule (owner, binding):** one core for the multisig's business rules and
logic on every platform. A shell keeps only its UI and its I/O: it dispatches
events, performs the operations a machine asks for, routes notices between
machines, and draws views. Anything that decides — who signs how, whether a
transaction can run, which rows Activity gets, what a link means, when to look
again — is a machine's, or one of the pure rules below. The web
(`app-web/vela-wallet/src/lib/multisig/`) is the reference shell:
`shell/handler.ts` (the operation executor), `shell/live.ts` (the app's
pipelines), `shell/machines.svelte.ts` (boot, facts, the notice router).

Sources: `rust/crates/vela-core/src/app/multisig_*.rs`. Wire types are
generated (`node rust/scripts/gen-core-types.mjs multisig` →
`app-web/vela-wallet/src/lib/multisig/generated/`, 127 files); the names
below are the TS names. Every JSON field is the Rust field (snake_case) unless
the type says `camelCase`.

## 1. The protocol

Five machines, each an object with the same four calls — wasm
(`vela-core-wasm/src/multisig_machines.rs`) and uniffi
(`vela-core-uniffi/src/multisig_bridge.rs`) expose the same JSON surface:

```text
new()                         a machine in its initial state
dispatch(event_json)          -> { view, effects: [{ id, operation }], cancelled_effect_ids: [] }
resolve_effect(id, result)    -> the same shape
view()                        -> the current view
```

| Machine | uniffi / wasm class | Event | View |
| --- | --- | --- | --- |
| Accounts | `MultisigAccountsCore` | `MultisigAccountsEvent` | `MultisigAccountsView` |
| Proposals | `MultisigProposalsCore` | `MultisigProposalsEvent` | `MultisigProposalsView` |
| Members | `MultisigMembersCore` | `MultisigMembersEvent` | `MultisigMembersView` |
| Outside page | `MultisigOutsidePageCore` | `OutsidePageEvent` | `OutsidePageView` |
| Outside session | `MultisigOutsideSessionCore` | `OutsideSessionEvent` | `OutsideSessionView` |

Effect ids only grow; an answer to an id the machine no longer waits for
changes nothing (no error). `Render` effects are dropped: the view comes back
with every call. Each operation gets exactly ONE `MultisigShellResult`.

### 1.1 Operations (`MultisigOperation`, tag `type`)

| Operation | Fields | Answers |
| --- | --- | --- |
| `http` | method, url, body?, timeout_ms | `http_answered {status, retry_after?, body}` / `http_unreachable {detail}` — never retried by the shell |
| `rpc` | chain_id, method, params, bound_ms? | `rpc_result {result}` / `rpc_error {code?, message}` / `rpc_unanswered {detail}` (also when `bound_ms` passes) |
| `load_store` | keys[] | `store_loaded {entries: {key: string or null}}` / `store_failed` |
| `write_store` | key, value? (null removes) | `store_written` / `store_failed` |
| `wait` | ms | `waited` (a wait the machine stopped waiting for may be dropped) |
| `now` / `random` | — / bytes | `now {now_ms}` / `random {hex}` |
| `passkey` | owner, chain_id, challenge, purpose (`approve {multisig, safe_tx_hash, name?, tx}` or `record {multisig}`) | `asserted {credential_id, authenticator_data, client_data_json, signature}` / `page_approved {approval}` (the trusted signing page) / `passkey_cancelled` / `passkey_failed {detail}` |
| `ask_risk` | chain_id, safe, findings[] | `risk_answered {signs}` — a shell that cannot ask answers `false` |
| `send_calls` | chain_id, from, calls[], fee?, purpose? | `calls_sent {user_op_hash, tx_hash?}` / `calls_failed {detail, maybe_sent}` / `calls_cancelled` |
| `quote_calls` | chain_id, from, calls[] | `quoted {quote}` / `quote_failed {detail}` |
| `simulate` / `recipient_risk` / `user_op_status` | see the type | `simulated {reply?}` / `recipient_facts {is_contract?, first_time?}` / `user_op_status {status?, tx_hash?}` |
| `peer_open` / `peer_send` / `peer_receive` / `peer_close` | PeerConnect rooms | `peer_opened {room, invite?, pairing?}` / `ack` / `peer_message {message}` or `peer_closed {reason}` / `ack` |
| `wallet_request` | method, params (EIP-1193, the injected wallet) | `wallet_answered {result}` / `wallet_refused {code?, message}` |
| `notice` | notice: `MultisigNotice` | `ack` — the shell routes it (1.2) |

A failure the shell can only describe is answered with the operation's
failure shape (`operationFailure` in `shell/handler.ts` is the table).

### 1.2 Notices (`MultisigNotice`, tag `kind`) — the one router

| Notice | From | Route to |
| --- | --- | --- |
| `account {address, update}` — `use_chain`, `findings_on`, `recorded_name`, `name_members` | proposals, members | accounts `apply {address, update}` |
| `activity_rows {rows}` | proposals | the shell's transaction store, after wording each planned row (2.2) |
| `mailbox_took {safe}` | proposals, outside page | whoever shows that safe: proposals `refresh {fresh: true}` |
| `activated {address?}` | accounts | the shell (which account is shown) |
| `open_proposal {address, chain_id, safe_tx_hash, preview?}` | accounts (a link) | the shell opens the sheet; proposals `open_linked` |
| `outside_sign {request}` | proposals | start an outside session; its outcome back as proposals `outside_answered` |

### 1.3 Storage keys (the machine owns the record, the shell stores it)

Any key-value store; the names are the web's `localStorage` keys.

| Key | Owner |
| --- | --- |
| `vela.multisigAccounts`, `vela.activeMultisig`, `vela.multisigPendingProofs`, `vela.multisigDiscovered` | accounts |
| `vela.multisigPendingLink` (W9: a link waiting for a network this device lacks, one day) | accounts |
| `vela.multisigCreateDraft` (W9: the create form, until created or discarded) | accounts |
| `vela.multisigInviteShared` (W9: the invite was handed on, per multisig) | accounts |
| `vela.multisigKeptLink` (a link opened before any Vela account, one day) | accounts (also written by the web's welcome page, which loads no core) |
| `vela.multisigDeployed` | shared (`Chain`) |
| `vela.multisigOutbox`, `vela.multisigWaiting`, `vela.multisigExecutions`, `vela.multisigLastActor`, `vela.multisigExecuting`, `vela.multisig.executed` | proposals |
| `vela.connect.unsent.v1`, `vela.connect.sessions.v1` | outside page |
| `vela.multisig.externalWallets.v1` | outside session |

### 1.4 Timers the shell drives (the machine says when; the shell only ticks)

| Event | When |
| --- | --- |
| accounts `tick` | every 60 s, on `online`, focus, page shown |
| accounts `check_records` | once a session |
| proposals `tick` | every `view.pace_ms` while the multisig is shown and the page visible (12 s; 4 s while a sheet holds it) |
| proposals `refresh` | page shown again, back online |
| proposals `others_tick` | every 60 s while a page is visible |
| proposals `preview` | an open sheet asks every few seconds per executor; the machine decides whether the last look is stale (W9) |
| outside page `refresh` / session `mailbox_tick` | 12 s / 8 s |

## 2. The machines

Boot order (the web's `MultisigMachines.boot`): start accounts, members and
proposals over one executor; feed them the same facts (this device's Vela
accounts as `MultisigLocalAccount`, every network it has, the space's
networks, the service URL); route every notice through one router (1.2);
when the accounts machine says `loaded`, attach it to the screens. Facts
that move (another tab, a network added, another service, an account signed
in) are sent again as their events — never decided in the shell.

### 2.1 Accounts (`multisig_accounts.rs`, `multisig_accounts_link.rs`)

Every multisig on this device, its record, create / add / import, links,
member kinds, and who can act for it per network (§3.5).

- **Facts:** `start {service_url, accounts, networks}`, `local_accounts`,
  `networks` (also re-opens a link waiting for its network), `space
  {chain_ids}`, `reload` (store changed elsewhere), `service {url}`.
- **Background:** `tick` (records not landed), `check_records`,
  `recheck_record {address}`, `recheck` (who can act, per network),
  `discover {address, chain_ids}`.
- **§3.5 on demand (W9):** `check_on {address, chain_id}` — that network judged
  once a session, also one it is not used on yet; an unanswered read is no
  verdict and is asked again. Read it from the row: `checked_on`,
  `checking_on`, `account.findings` (that chain's). A sheet never re-reads the
  chain itself.
- **Create:** `new_draft` (a fresh salt; a kept draft stays), `draft {owners,
  threshold, name}`, `check_networks {chain_ids}`, `create {choice}` (one
  passkey); W9 `keep_draft {draft}` / `discard_draft` — the form is kept
  (`create.kept`, stored) until created or discarded; `create.other_space`
  names this device's networks of the other space (r7-9).
- **Add / find / import:** `resolve_ens`, `add`, `find`, `keep_found`,
  `keep_invited`, `parse_export`, `import`.
- **Records:** `share_again`, `check_backup`, `save_to_ethereum`, `plan_rename`
  (W9: `RenameBlocked::pending` + `lands_on` — a record on its way is said as
  such, r7-10; ask again when `shared` or `record_waiting` changes).
- **List:** `activate`, `remove`, `set_label`, `name_members`, `set_first_run`,
  W9 `invite_shared {address}` (row `invite_shared`: the home's first step
  ticks), `apply {address, update}` (a notice from another machine).
- **Links:** `open_link {hash, take_kept}`, `keep_link {hash}`, `close_link`,
  W9 `drop_link` (the person closed the missing-network sheet). A link waiting
  for a network is stored (`vela.multisigPendingLink`) and survives a reload.
- **Kinds:** `read_kinds {members}`, `pick_kind {address, kind?}`.

View `MultisigAccountsView`: `loaded`, `accounts: AccountRow[]` (`account`,
`name`, `shared`, `record_waiting`, `local_members`, `high_risk_on`,
`refused_on`, `checked_on`, `checking_on`, `invite_shared`), `active`,
`create` (`phase`, `draft_address`, `draft_error`, `address`, `refused`,
`error`, `networks`, `other_space`, `kept`), `add`, `import`, `backup`,
`share_again`, `share_again_failure`, `rename`, `service_down`, `link`
(`LinkStand`: broken, invite, missingNetwork, opening, member, preview,
failed, kept), `member_kinds`, `known_members`.

### 2.2 Proposals (`multisig_proposals.rs`, `multisig_proposals_flows.rs`, `multisig_presence.rs`, `multisig_gather.rs`, `multisig_dapp.rs`)

A multisig's transactions in flight: read (mailbox + chain, nothing trusted),
reviewed (the one review every signer signs from, §3.6), proposed, approved,
executed, watched.

- **Facts / reading:** `start`, `follow {account?}`, `account`,
  `local_accounts`, `networks`, `known`, `service`, `tick`, `refresh {fresh}`,
  `retry`, `online`, `watch` / `unwatch` (a sheet holds the faster pace),
  `accounts` + `others_tick` (presence of the multisigs not shown).
- **Review and propose:** `review {chain_id, safe_tx_hash, blind}`, `draft
  {chain_id, proposal}` (`transfers`, `rename`, `member_change`, `calls`,
  `cancel`), `confirm_draft {as_member?}`, `withdraw`.
- **Approve and execute:** `approve`, `preview {chain_id, safe_tx_hash,
  executor}`, `execute`, `send_again`.
- **W9 — the look is the machine's:** `preview` is a request, not an order:
  the machine looks only when the executor's last look is stale
  (`PREVIEW_FRESH_MS` 15 s; unpriced or unchecked 5 s), never two at once, and
  caps a look at `PREVIEW_CAP_MS` (15 s, then `unchecked`). On every `tick` it
  looks at each ready transaction by each executor here (`HOME_FRESH_MS`
  60 s; a look that stops it — would fail, nobody can pay — again at 15 s, so
  a funded executor is noticed on the next poll).
- **Presence, timeline, links, outside wallets, dApps:** `timeline`,
  `open_linked` / `close_linked`, `approve_outside`, `outside_answered`,
  `indexed_members`, `read_signers`, `dapp_request`, `dapp_leave`,
  `dapp_settled`, `calls_status`.

View `MultisigProposalsView` — draw these, never re-derive them:

- `items`, `chains`, `missing`, `unreadable`, `history_unreadable`,
  `offline`, `stale`, `unreachable`, `cached_at`, `read_at`, `loading`,
  `pace_ms`;
- `standings` per `chainId:hash` — the row's one word: `unsent`,
  `executing`, `queued`, `ready`, `blocked`, `yourTurn`, `progress`. W9: a
  blocked one carries `block` = `fails` with `why` (`short` with token, have,
  need; or `reverts` with reason), or `nobodyPays` — the home says “Can't run
  now — the multisig holds 0.28 USDC, and this needs 0.5 USDC”;
- `waiting_note` — the group's one note (offline, unreachable,
  chainUnreachable, lastKnown, stale, historyUnreadable, checking — W9: only
  with nothing shown); a Retry goes with unreachable, chainUnreachable and
  historyUnreadable;
- `previews` per `chainId:hash:executor`, `reviews` per `chainId:hash` (the
  one `MultisigReview`), `actions` per `propose`, `approve:hash` or
  `execute:hash` (`ActionState`), `executions`, `frozen`, `timelines`,
  `outbox`, `draft`, `last_actor`, `presence`, `others_waiting`, `dapp`,
  `calls_status`, `linked`;
- `signers` per `chainId:hash` (asked with `read_signers`) — W9 (r7-8), each
  member still to approve said ONCE: `rows` (kind here, vela, wallet, unsure,
  contract, or null while read), `in_vela` (“Approves in Vela on their own
  device” — only a member something shows is a Vela account), `outside` (the
  outside-wallet link: wallet and unsure, never a Vela member), `either`
  (nothing says yet: its own line, both ways offered), `contract`, `enough`;
- W9 `members_anywhere` — the followed multisig's members on some network.

Notice `activity_rows`: each executed row carries `rows`, the core's plan
(W9, `execution_rows`): one `send` row (to, token, amount) per transfer, else
one `call` row (to, sends, calls, contract) for the proposal. Amounts are
base units, decimal; the shell only words them (token symbol and decimals,
the proposal's title) and keeps them as its Activity rows.

### 2.3 Members (`multisig_members.rs`)

The multisig shown in Settings: its members per network, the change being
composed, the plan per network. Events `show` (account, chain_ids, local,
known), `refresh`, `local_accounts`, `contacts`, `waiting`, `compose`,
`step`, `draft`, `entry`, `read_entry`, `pick`, `entry_name`, `entry_kind`,
`proposing`. It never proposes: the shell sends each network's
`plan.networks[i].proposal` to proposals `draft`. View `MultisigMembersView`:
`address`, `original`, `networks`, `on_device`, `read_only`, `rules`,
`rule_chip`, `none_deployed`, `rows`, `base`, `recovery`, `compose`,
`stepper`, `entry`, `change`, `plan`, `can_propose`, `replace_preview`,
`groups`.

### 2.4 Outside page (`multisig_outside.rs`, `mod page`)

The `/connect` page an outside owner opens in a browser with its own wallet
(the wallet side of WalletPair). Events `start`, `wallet`, `connect_wallet`,
`link`, `refresh`, `approve` (account, form?), `approve_on_chain`,
`estimate_fee`, `balance`, `execute`, `reject`, `send_again`, `cancel`,
`resume_all`, `join`, `answer_connect`, `answer_request`, `end`,
`peer_state`, `paired`, `relay`. View `OutsidePageView`: `link`, `load`,
`tx` (`OutsideTxView`; its `review` is the same `MultisigReview` a Vela
member signs from — draw it, never decode again: the Bybit rule), `step`,
`actions`, `fee`, `balance`, `accounts`, `wallet_chain_id`, `unsent`,
`sessions`, `activity`, `request`, `connect_prompt`, `join_failed`, `relay`,
`wallet_failure`, `request_genesis`, `request_enough`, and the W9 additions
in section 4.

### 2.5 Outside session (`multisig_outside.rs`, `mod session`)

Vela's side of a live session asking an outside member's wallet to sign.
Events `start` (request, service_url, relay?, origin?), `peer_state`,
`paired`, `relay`, `mailbox_tick`, `ask_again`, `stop_waiting`,
`another_wallet`, `close`. View `OutsideSessionView`: `open`, `resumed`,
`invite`, `members`, `tx`, `phase`, `since`, `away`, `relay`, `accounts`,
`chain`, `wallet`, `paired_account`, `failure`, `outcome` (approved, sent or
cancelled — sent back as proposals `outside_answered`).

## 3. The pure rules (`multisig_rules`, W9)

Rules a screen asks one at a time, outside any machine, through the link
door — uniffi `multisig_link_call(method, args_json)`, wasm
`multisigLinkCall` — as `area.method` with one JSON object of arguments
(built with the `crux` feature, as the machines are):

- `calls.readable` (action): the calls the clear reading reads — a call, or
  a batch that is more than transfers — else null.
- `calls.reviewed` (action, tx): the calls the one review reads — every call
  and transfer; a lone transfer as the SafeTx's own call — else null.
- `names.label`, `names.kind`, `names.short`: a member's label (this
  device's account name, the name a multisig keeps, the passkey index's
  name, the short EIP-55 address) and kind.
- `networks.likely`, `networks.firstDefault`, `networks.lastUsed`,
  `networks.recent`, `networks.held`: the order networks are offered in, and
  create's default network.
- `relay.*`: the PeerConnect relay's token schedule (which relay needs a
  token, which token to use, refusals, the back-off, the status).

The link codec itself (proposal, invite and connect links, row ids, units) is
the same door; `multisig_link.rs` lists its methods.
