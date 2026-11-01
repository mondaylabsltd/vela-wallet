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

| Machine         | uniffi / wasm class          | Event                    | View                    |
| --------------- | ---------------------------- | ------------------------ | ----------------------- |
| Accounts        | `MultisigAccountsCore`       | `MultisigAccountsEvent`  | `MultisigAccountsView`  |
| Proposals       | `MultisigProposalsCore`      | `MultisigProposalsEvent` | `MultisigProposalsView` |
| Members         | `MultisigMembersCore`        | `MultisigMembersEvent`   | `MultisigMembersView`   |
| Outside page    | `MultisigOutsidePageCore`    | `OutsidePageEvent`       | `OutsidePageView`       |
| Outside session | `MultisigOutsideSessionCore` | `OutsideSessionEvent`    | `OutsideSessionView`    |

Effect ids only grow; an answer to an id the machine no longer waits for
changes nothing (no error). `Render` effects are dropped: the view comes back
with every call. Each operation gets exactly ONE `MultisigShellResult`.

### 1.1 Operations (`MultisigOperation`, tag `type`)

| Operation                                                 | Fields                                                                                                     | Answers                                                                                                                                                                               |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `http`                                                    | method, url, body?, timeout_ms                                                                             | `http_answered {status, retry_after?, body}` / `http_unreachable {detail}` — never retried by the shell                                                                               |
| `rpc`                                                     | chain_id, method, params, bound_ms?                                                                        | `rpc_result {result}` / `rpc_error {code?, message}` / `rpc_unanswered {detail}` (also when `bound_ms` passes)                                                                        |
| `load_store`                                              | keys[]                                                                                                     | `store_loaded {entries: {key: string or null}}` / `store_failed`                                                                                                                      |
| `write_store`                                             | key, value? (null removes)                                                                                 | `store_written` / `store_failed`                                                                                                                                                      |
| `wait`                                                    | ms                                                                                                         | `waited` (a wait the machine stopped waiting for may be dropped)                                                                                                                      |
| `now` / `random`                                          | — / bytes                                                                                                  | `now {now_ms}` / `random {hex}`                                                                                                                                                       |
| `passkey`                                                 | owner, chain_id, challenge, purpose (`approve {multisig, safe_tx_hash, name?, tx}` or `record {multisig}`) | `asserted {credential_id, authenticator_data, client_data_json, signature}` / `page_approved {approval}` (the trusted signing page) / `passkey_cancelled` / `passkey_failed {detail}` |
| `ask_risk`                                                | chain_id, safe, findings[]                                                                                 | `risk_answered {signs}` — a shell that cannot ask answers `false`                                                                                                                     |
| `send_calls`                                              | chain_id, from, calls[], fee?, purpose?                                                                    | `calls_sent {user_op_hash, tx_hash?}` / `calls_failed {detail, maybe_sent}` / `calls_cancelled`                                                                                       |
| `quote_calls`                                             | chain_id, from, calls[]                                                                                    | `quoted {quote}` / `quote_failed {detail}`                                                                                                                                            |
| `simulate` / `recipient_risk` / `user_op_status`          | see the type                                                                                               | `simulated {reply?}` / `recipient_facts {is_contract?, first_time?}` / `user_op_status {status?, tx_hash?}`                                                                           |
| `peer_open` / `peer_send` / `peer_receive` / `peer_close` | PeerConnect rooms                                                                                          | `peer_opened {room, invite?, pairing?}` / `ack` / `peer_message {message}` or `peer_closed {reason}` / `ack`                                                                          |
| `wallet_request`                                          | method, params (EIP-1193, the injected wallet)                                                             | `wallet_answered {result}` / `wallet_refused {code?, message}`                                                                                                                        |
| `notice`                                                  | notice: `MultisigNotice`                                                                                   | `ack` — the shell routes it (1.2)                                                                                                                                                     |

A failure the shell can only describe is answered with the operation's
failure shape (`operationFailure` in `shell/handler.ts` is the table).

### 1.2 Notices (`MultisigNotice`, tag `kind`) — the one router

| Notice                                                                                    | From                    | Route to                                                                   |
| ----------------------------------------------------------------------------------------- | ----------------------- | -------------------------------------------------------------------------- |
| `account {address, update}` — `use_chain`, `findings_on`, `recorded_name`, `name_members` | proposals, members      | accounts `apply {address, update}`                                         |
| `activity_rows {rows}`                                                                    | proposals               | the shell's transaction store, after wording each planned row (2.2)        |
| `mailbox_took {safe}`                                                                     | proposals, outside page | whoever shows that safe: proposals `refresh {fresh: true}`                 |
| `activated {address?}`                                                                    | accounts                | the shell (which account is shown)                                         |
| `open_proposal {address, chain_id, safe_tx_hash, preview?}`                               | accounts (a link)       | the shell opens the sheet; proposals `open_linked`                         |
| `outside_sign {request}`                                                                  | proposals               | start an outside session; its outcome back as proposals `outside_answered` |

### 1.3 Storage keys (the machine owns the record, the shell stores it)

Any key-value store; the names are the web's `localStorage` keys.

| Key                                                                                                                                                    | Owner                                                                  |
| ------------------------------------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| `vela.multisigAccounts`, `vela.activeMultisig`, `vela.multisigPendingProofs`, `vela.multisigDiscovered`                                                | accounts                                                               |
| `vela.multisigPendingLink` (W9: a link waiting for a network this device lacks, one day — a proposal link, or (core-ms-2) an invite)                   | accounts                                                               |
| `vela.transactionHistory` (core-ms-2: read only, the `recent_networks` hint)                                                                           | the wallet (read by accounts)                                          |
| `vela.multisigCreateDraft` (W9: the create form, until created or discarded)                                                                           | accounts                                                               |
| `vela.multisigInviteShared` (W9: the invite was handed on, per multisig)                                                                               | accounts                                                               |
| `vela.multisigKeptLink` (a link opened before any Vela account, one day)                                                                               | accounts (also written by the web's welcome page, which loads no core) |
| `vela.multisigDeployed`                                                                                                                                | shared (`Chain`)                                                       |
| `vela.multisigOutbox`, `vela.multisigWaiting`, `vela.multisigExecutions`, `vela.multisigLastActor`, `vela.multisigExecuting`, `vela.multisig.executed` | proposals                                                              |
| `vela.connect.unsent.v1`, `vela.connect.sessions.v1`                                                                                                   | outside page                                                           |
| `vela.multisig.externalWallets.v1`                                                                                                                     | outside session                                                        |

### 1.4 Timers the shell drives (the machine says when; the shell only ticks)

| Event                                           | When                                                                                                       |
| ----------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| accounts `tick`                                 | every 60 s, on `online`, focus, page shown                                                                 |
| accounts `check_records`                        | once a session                                                                                             |
| proposals `tick`                                | every `view.pace_ms` while the multisig is shown and the page visible (12 s; 4 s while a sheet holds it)   |
| proposals `refresh`                             | page shown again, back online                                                                              |
| proposals `others_tick`                         | every 60 s while a page is visible                                                                         |
| proposals `preview`                             | an open sheet asks every few seconds per executor; the machine decides whether the last look is stale (W9) |
| outside page `refresh` / session `mailbox_tick` | 12 s / 8 s                                                                                                 |

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
  create's default network; core-ms-2 `networks.name {chainId}`: the core
  catalog's name for a network this device may not have (§6).
- `relay.*`: the PeerConnect relay's token schedule (which relay needs a
  token, which token to use, refusals, the back-off, the status).

The link codec itself (proposal, invite and connect links, row ids, units) is
the same door; `multisig_link.rs` lists its methods.

## 4. The outside page after W9 (r7-11, r4C-12)

- `OutsideTxView.title` (`OutsideTxTitle`): what the transaction is, as
  facts the shell words — `transfer` (to, amount in base units, `token`
  facts: symbol, decimals, read by the core), `batch` (total, transfersOnly,
  each transfer), `cancel`, `addOwner`, `removeOwner`, `swapOwner`,
  `changeThreshold`, `rename`, `call` (to, value), `unsupported` (refusal).
  `OutsideTxView.name` is the multisig's name. The page draws the title, then
  `tx.review` (the same `MultisigReview` as the member's sheet: blocks, the
  member change rule “Approvals: 2 of 3 → 2 of 4” from
  `review.memberChanges`, hashes, the advice) — never a reading of its own.
- `OutsidePageView.requestReview`: the review of the transaction Vela asks
  this page to sign (a paired request), drawn the same way.
- `OutsidePageView.rows` (`OutsideActivityRow`: id, at, `label`, `outcome`,
  bad): the list on the page, each row decided by the core — `label`
  (`OutsideRowLabel`: approve or approveTo with the title facts, reject,
  execute — an `execTransaction` is matched to its SafeTx in the core —
  signTx, sendTx, switchChain, other) and `outcome` (`OutsideRowOutcome`:
  waiting, inWallet, done, signedHash, sent, switched, withdrawn, declined,
  walletDeclined, failed with its problem, notSent). The shell maps each kind
  to its words, nothing else.

## 5. What W9 moved into the core (for a native lane catching up)

| Was the web's                                                                                    | Now                                                                                                         |
| ------------------------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------- |
| `preflight.svelte.ts` (looks, their pace and cap, “Can't run now”)                               | proposals: `preview` paced by the machine, `tick` looks at ready ones, `standings` carry the failure detail |
| Activity's group note and row words                                                              | proposals `waiting_note`, `standings`                                                                       |
| `presence.othersWaiting`                                                                         | proposals `others_waiting`                                                                                  |
| `membersAnywhere`                                                                                | proposals `members_anywhere`                                                                                |
| the sheet's split of members (Vela / outside / unsure / contract)                                | proposals `signers[…]` `in_vela`, `outside`, `either`, `contract`, `enough`                                 |
| `records.ts` (which rows an execution writes)                                                    | `activity_rows` rows' `rows` plan (`execution_rows`)                                                        |
| `recheck.ts`, `judge.ts`, `authority.ts` (the sheet's own chain read)                            | accounts `check_on`, row `checked_on` / `checking_on` / findings                                            |
| `execute.ts`, `service.ts` requests and outage, `owners.ts` signing                              | proposals (execution), `multisig_shell::Service` / `Outage`, the `passkey` operation                        |
| `names.svelte.ts`, `likely-networks.ts`, `relay.ts` decisions, `readableCalls` / `reviewedCalls` | `names.*`, `networks.*`, `relay.*`, `calls.*` through the link door                                         |
| `TxReview.svelte`'s own decoding; the connect page's row labels                                  | `OutsideTxView.title` + `review`, `OutsidePageView.rows`, `requestReview`                                   |
| a link waiting for a network (memory only, lost on reload)                                       | accounts, stored as `vela.multisigPendingLink`; `drop_link`                                                 |
| the create form (lost when the sheet closed)                                                     | accounts `keep_draft` / `discard_draft`, `create.kept`, `create.other_space`                                |

What stays in a shell: drawing; its words (i18n keys from the corpus);
performing operations (HTTP, RPC, storage, passkeys, rooms, the injected
wallet); token metadata for wording an amount; the timers of 1.4; and the
router of 1.2.

## 6. core-ms-2 — rules the desktop lane found still in the web's TypeScript

All additive (new events, operations and fields with defaults) except
`relay::on_state`'s Rust return type. Wire names are the JSON ones.

### 6.1 Accounts

| Was the shell's                                                                                | Now                                                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| the create sheet's starting threshold (`toThreshold`), its note and risk, the stepper's bounds | `create.threshold` (`MultisigThresholdView` `{value, members, note: solo / all / one / spare {count}, risky, can_lower, can_raise}`), `None` before a draft names members. Event `step_threshold {up}`. The sensible start (all of two, else all but one) unless the person picked for these members; a kept draft's threshold is the pick again. Send `value` as `draft`'s / `keep_draft`'s / `create`'s threshold. |
| applying an invite after Add (`useChain` per invite network, `keep_invited`)                   | The machine keeps the invite it opened: view `invite` (`MultisigInviteView` `{link, on_device, missing, missing_name}`). `add {address, network: null}` of the invite's address looks it up on its first network here, uses it on every invite network this device has, and keeps the invite's member labels. Send only the person's pick as `network`. Closing the invite's add sheet: `drop_link`.                 |
| an invite waiting for a network (a `keep_link` record, reopened on the next wallet load)       | An invite naming only networks this device lacks waits in `vela.multisigPendingLink` (the proposal link's record, a day): a reload restores it quietly (`invite.missing` set, no stand), `networks` with one of them says `link: invite` again. Add network needs nothing from the shell.                                                                                                                            |
| the `recent` / `used` hints (`loadTransactions` + `networks.recent`, `networks.lastUsed`)      | view `recent_networks` (read from `vela.transactionHistory` on `start`, `reload`, `new_draft`) and `used_networks`. Pass them as `hints.recent` / `hints.used` to `networks.likely` / `networks.firstDefault`.                                                                                                                                                                                                       |
| a network's name the device lacks (the chain index only)                                       | `networks.name {chainId}` through the link door (the core's catalog, `BUILTIN_CHAINS`), and `invite.missing_name`; `null` → the shell may still ask the chain index.                                                                                                                                                                                                                                                 |

`vela.transactionHistory` joins the accounts machine's `load_store` keys
(read only).

### 6.2 Proposals

| Was the shell's                                                                                       | Now                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ----------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| "Approve and execute": execute, and on a stop before signing approve instead (`stoppedBeforeSigning`) | view `approve_execute` per `chainId:hash` (lowercase): the members here whose one press approves and executes it — short of one approval, theirs reaches the threshold, their look at these approvals does not say they cannot (priced and payable; or, Round 5 S2, unpriced or unchecked — the execution looks again first). A look still out, failing, unpayable, refused or spent offers nothing. Event `approve_execute {chain_id, safe_tx_hash, executor}`: `actions["execute:<hash>"]`; stopped before any signature, `actions["approve:<hash>"]` follows (the approval given and kept, r4-11). |
| a site's transaction ready on its proposer's approval (`executeNow`, propose then execute)            | `draft.executeNow`; event `propose_execute {as_member}` — `actions["execute:<hash>"]` is `working` from the moment `propose` is `done`, and removed again if it is not ready.                                                                                                                                                                                                                                                                                                                                                                                                                         |
| the header chip (`currentRules`, `ruleChip`)                                                          | view `rule_chip` (`MultisigRuleChip`, as the members machine's) for the followed multisig.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| the waiting line's parts and the home's banner (`waitingWords`, `presence.of(first)`, `setUp`)        | view `waiting_parts` per multisig (`[{kind: needsYou / ready / waitingPayer, count}]`, in order, nonzero only), `banner` (`{parts, first}` for the followed multisig, only when there is one to open), `set_up` (deployed on a network read).                                                                                                                                                                                                                                                                                                                                                         |
| `calls_status` answers kept for ever                                                                  | event `calls_status_taken {id}` once the site was answered; at most `CALLS_STATUS_KEPT` (16) kept, the oldest going first.                                                                                                                                                                                                                                                                                                                                                                                                                                                                            |

### 6.3 The in-app browser machine (`dapp_browser`)

- Event `multisig_answered {tab, id, answer: MultisigDappAnswer}` in place of
  `signing_answered` for a request forwarded from a multisig: delivered as
  the proposals machine formed it — an error keeps its `data` (the proposal
  a "proposed, waiting" `-32000` carries).
- A `wallet_sendCalls` answered that way is a multisig batch:
  `wallet_getCallsStatus` of it is an existing `read` operation with
  `bundler: true` and `method: "wallet_getCallsStatus"`
  (`MULTISIG_CALLS_STATUS_METHOD`, never a bundler's method) on the batch's
  chain, `params: [batch]` — no new operation, so a shell that has not
  adopted it answers nothing new. Ask the proposals machine `calls_status
  {id, chain_id, calls_id: batch}`, answer `read_answered` with its
  `calls_status[id]` as `body_json` (a `DappAnswer` is a JSON-RPC body:
  `{result}` or `{error}`, delivered as it is, `data` kept), then send it
  `calls_status_taken {id}`. (The desktop still answers with
  `signing_answered` and reads its batches itself, unchanged.)

### 6.4 Elsewhere

- `relay::on_state` returns `OnState {status, accepted}` in Rust (the
  `relay.onState` JSON is unchanged): `accepted` (on the relay) is when the
  connection forgets its refusals.
- The uniffi multisig objects have `skip_unchanged_views(include_volatile)`
  (the wasm ones had it); the web's multisig machines opt in.
- `home.multisig.settings.unreadableOn` ("Couldn't read its members on
  {{network}}."), on-demand multisig namespace, all 15 locales: a network
  whose chain did not answer (`NetworkState::Unreadable`) in Settings.

## 7. Round 8 — the review reads every call itself (r8-1, r8-2, r8-4, r8-5)

All additive (new fields with defaults, new enum variants) except
`RpcFail::Error`'s Rust shape.

### 7.1 `MultisigReview.reading` — draw it, never decode

The one review (`reviews[...]`, `draft.review`, the outside page's
`tx.review` / `requestReview`) carries what each call does, read by the core
from the SafeTx's bytes alone (`multisig_review::reading`) with token facts
the core read itself (the registry — now with Arc Testnet's USDC, 6 decimals
— else the contract's `symbol()` / `decimals()`, bounded; a token that did
not answer stays unknown). No shell runs `clear_signing` / `approval_guard`
over a multisig call any more (the web's `call-review.svelte.ts` is deleted).

```text
reading: { calls: Call[], nested, unreadable, unlimited, blind, decimalsUnknown: string[] }
Call:    { path: number[], to, value: Amount, intent: Intent }   // path [] lone call, [2,1] leg 1 of leg 2's batch
Amount:  { raw, decimals: number|null, plain: string|null, unlimited }
Intent (tag kind): send {token, to, amount} | sendFrom {token, from, to, amount}
       | approve {token, spender, grant: approve|increase|decrease|permit|daiPermit|permit2, amount, expires}
       | approveAll {collection, operator, granted} | batch {runner, calls} | unreadable
       | members {action} | rename {name} | call {selector}
token:   OutsideTokenFacts {token (null = the coin), symbol, decimals, known}
```

Rules: decimals are never assumed (`decimals: null` → the raw integer and
`reading.decimalsUnknown`, no friendly amount); unlimited is the guard's line
(2^200 / Permit2 2^152, DAI `allowed`, `setApprovalForAll(true)`); a batch
inside a batch is read leg by leg to `MAX_NESTING` (3), deeper → `unreadable`.

New `concerns` (deduped by the core — say each once, r8-14):
`unlimitedApproval {token, spender}`, `nestedBatch`, `toOwnToken {token}`
(danger → `sign.anyway`), `unreadableBatch` (refused → `sign.allowed` false),
`decimalsUnknown {token}` (caution); `blind` is raised by the core itself for
a call it cannot read (a shell's `blind` can only add to it).

Words: `home.multisig.review.reading.*` (intent titles, row labels,
`unlimited`, `decimalsUnknown {{raw}}`, one line per concern); a leg is
`review.callNumber` with `n` = the path joined by `.`. Reference drawing:
`app-web/vela-wallet/src/lib/multisig/core/reading-blocks.ts`.

Parity vectors: `rust/crates/vela-core/tests/vectors/multisig/reading.json`,
replayed through the link door `review.reading {chainId, safe, tx,
tokenFacts?}` → `{reading, concerns, anyway, refused}`. `review.token
{chainId, token}` gives the registry's facts of a token.

### 7.2 Elsewhere

| Was | Now |
| --- | --- |
| `rpc_error {code, message}` | + `data?` — pass the node's `error.data`: the state probe's answer is a revert's payload (r8-4; Arc has no `debug_traceCall`). |
| titles worded with a shell's own token read (18 decimals on a failure) | proposals view `tokens` per `chainId:token` (registry, else the contract; once read never unsaid) (r8-2). |
| `OutboxEntry {why, retry_at}` | + `sending` (`why: "sending"`): on its way — say `proposal.outboxSending`, never "Not sent yet", no Send again (r8-16). |
| `OutsideExecutionFee {wei, count, partial}` | + `max_wei` (the most it can take); say `external.executeFeeMax` and, with `count > 1`, `external.executeConfirms` before the first prompt (r8-15). |
| `AccountRow.local_members` = founding members here | its members as the chains last said (`AccountUpdate::MembersNow` from proposals → accounts `apply`; stored `vela.multisigMembersNow`) (r8-7). |
| an execution found later dated when found | dated by its block's time (r8-17). |
| a dApp site's default chain 1 everywhere | Ethereum only when the space has it, else the space's first network (r8-21). |

## 8. Round 9 — one name, one reason, one balance

All additive (new fields with defaults, a new enum variant, new functions).

| Was the shell's                                                                                      | Now                                                                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a refusal said as `review.blocked` ("would change who controls…") whatever the reason (r9-9)         | `MultisigReview.refused` `{why: delegatecall / tooDeep / refund / stateChange / other, key}` — draw the corpus key `key` as the ONE refusal line (`home.multisig.review.refusedWhy.*`, or `review.blocked` for the state guard's own refusal), never `proposal.refused` with the core's English `refusal`.                                                             |
| "Checked… changes nothing about who controls" beside "Vela won't sign this" (r9-9)                   | `state.verdict.kind = withheld` (`StateVerdict::Withheld`): refused for another reason, nothing is said in the guard's place (nor its could-not-check).                                                                                                                                                                                                                  |
| "Only members can approve, and none is signed in on this device" whenever `can_approve` is empty     | `ProposalItem.members_here` (every member of it on this device, whatever the transaction): say `proposal.notMember` only when it is empty.                                                                                                                                                                                                                                |
| each shell picking a member's name (r9-12, r8-25, r4D-5)                                             | accounts view `member_labels` `{lowercase address: label}` — every member of every multisig here and this device's accounts, by `names::label` (kept, passkey index, own name, short address). `index_wanted`: the addresses to look up in the passkey index (this device's own too); answer with event `index_names {answers: {address: {vela, name}}}` (`MultisigIndexAnswer`). |
| an invite built from the shell's own known-map (Android's carried no labels)                         | `AccountRow.invite_members` (`InvitedMember[]`: founding members, then members now; a name only where one is known, and the kind): link door `addLink {origin, locale, multisig, extra: {chainIds, members}}`.                                                                                                                                                          |
| "Require 1 approvals", "it needs 1 approvals too"                                                    | plural families `home.multisig.action.changeThreshold_*` and `home.multisig.proposal.rejectHint_*` (`{{count}}`); the plain keys stay for a shell not moved yet.                                                                                                                                                                                                        |
| Arc USDC said twice in Balance changes (the coin and its ERC-20 interface) (r9-2)                     | the review's `simulation` drops the interface's line when the coin's says it (`sim_outcome::one_line_per_balance`, `native_alias_token`): nothing for a shell to do.                                                                                                                                                                                                       |
| a link to a used number: "executed or cancelled" (r8-24)                                             | `OutsideTxView.done` (`OutsideDone`: `executed {txHash}` / `cancelled`; `null` while open or unread) — words `home.multisig.external.{doneExecuted, doneCancelled}`.                                                                                                                                                                                                      |
| a new site on the space's first network although the account lives elsewhere (extension)            | `network_space::site_start_chain_for(space, last_used, account_chains)` (wasm `siteStartChainFor`, uniffi `site_start_chain_for`): after the network last used, the first network the account holds something on or used. The extension catalog carries `accountChains`.                                                                                                |

Names are hints, never authority (r9-12 (c), decided by best practice and
the core principle): a name travels as the member's own passkey-index name
(public, written with its founding key, the same on every device), as the
invite's labels, and as names a member change carries — not through the
mailbox. A label field on a mailbox proposal would be unauthenticated text
that anyone allowed to post (strangers may post up to 32 proposals per Safe)
could attach to a member's address, shown beside approvals; the service is
never a trust premise. The gap it would close — a Vela account the index
cannot find by address because it is deployed only off the index's chains —
is the index lookup's to close (hop 1 from the member's verified passkey
approval's `ownerGenesis`), a follow-up.
