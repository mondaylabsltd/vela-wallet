# Multisig machines — the API the shells drive

2026-10-31 · W9 (§9: round 10, 2026-11-01; §12: the review state, 2026-11-05; §13: round 12, 2026-11-05). For the native lanes (desktop, Android, iOS) and the web.

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

| Was                                                                    | Now                                                                                                                                                 |
| ---------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `rpc_error {code, message}`                                            | + `data?` — pass the node's `error.data`: the state probe's answer is a revert's payload (r8-4; Arc has no `debug_traceCall`).                      |
| titles worded with a shell's own token read (18 decimals on a failure) | proposals view `tokens` per `chainId:token` (registry, else the contract; once read never unsaid) (r8-2).                                           |
| `OutboxEntry {why, retry_at}`                                          | + `sending` (`why: "sending"`): on its way — say `proposal.outboxSending`, never "Not sent yet", no Send again (r8-16).                             |
| `OutsideExecutionFee {wei, count, partial}`                            | + `max_wei` (the most it can take); say `external.executeFeeMax` and, with `count > 1`, `external.executeConfirms` before the first prompt (r8-15). |
| `AccountRow.local_members` = founding members here                     | its members as the chains last said (`AccountUpdate::MembersNow` from proposals → accounts `apply`; stored `vela.multisigMembersNow`) (r8-7).       |
| an execution found later dated when found                              | dated by its block's time (r8-17).                                                                                                                  |
| a dApp site's default chain 1 everywhere                               | Ethereum only when the space has it, else the space's first network (r8-21).                                                                        |

## 8. Round 9 — one name, one reason, one balance

All additive (new fields with defaults, a new enum variant, new functions).

| Was the shell's                                                                                  | Now                                                                                                                                                                                                                                                                                                                                                                               |
| ------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a refusal said as `review.blocked` ("would change who controls…") whatever the reason (r9-9)     | `MultisigReview.refused` `{why: delegatecall / tooDeep / refund / stateChange / other, key}` — draw the corpus key `key` as the ONE refusal line (`home.multisig.review.refusedWhy.*`, or `review.blocked` for the state guard's own refusal), never `proposal.refused` with the core's English `refusal`.                                                                        |
| "Checked… changes nothing about who controls" beside "Vela won't sign this" (r9-9)               | `state.verdict.kind = withheld` (`StateVerdict::Withheld`): refused for another reason, nothing is said in the guard's place (nor its could-not-check).                                                                                                                                                                                                                           |
| "No asset changes" under "Vela won't sign this" for a delegatecall or a batch too deep           | `simulation.kind = withheld` (`multisig_review::sim_withheld`): refused for a reason other than `stateChange`, no deltas, no notice, no `no_change_key`, and no `unsimulated` concern — draw NOTHING in the simulation's place (a shell's no-change fallback must not fire). The signing sheet's `SimVerdict` never has this kind.                                                |
| "Only members can approve, and none is signed in on this device" whenever `can_approve` is empty | `ProposalItem.members_here` (every member of it on this device, whatever the transaction): say `proposal.notMember` only when it is empty.                                                                                                                                                                                                                                        |
| each shell picking a member's name (r9-12, r8-25, r4D-5)                                         | accounts view `member_labels` `{lowercase address: label}` — every member of every multisig here and this device's accounts, by `names::label` (kept, passkey index, own name, short address). `index_wanted`: the addresses to look up in the passkey index (this device's own too); answer with event `index_names {answers: {address: {vela, name}}}` (`MultisigIndexAnswer`). |
| an invite built from the shell's own known-map (Android's carried no labels)                     | `AccountRow.invite_members` (`InvitedMember[]`: founding members, then members now; a name only where one is known, and the kind): link door `addLink {origin, locale, multisig, extra: {chainIds, members}}`.                                                                                                                                                                    |
| "Require 1 approvals", "it needs 1 approvals too"                                                | plural families `home.multisig.action.changeThreshold_*` and `home.multisig.proposal.rejectHint_*` (`{{count}}`); the plain keys stay for a shell not moved yet.                                                                                                                                                                                                                  |
| Arc USDC said twice in Balance changes (the coin and its ERC-20 interface) (r9-2)                | the review's `simulation` drops the interface's line when the coin's says it (`sim_outcome::one_line_per_balance`, `native_alias_token`): nothing for a shell to do.                                                                                                                                                                                                              |
| a link to a used number: "executed or cancelled" (r8-24)                                         | `OutsideTxView.done` (`OutsideDone`: `executed {txHash}` / `cancelled`; `null` while open or unread) — words `home.multisig.external.{doneExecuted, doneCancelled}`.                                                                                                                                                                                                              |
| a new site on the space's first network although the account lives elsewhere (extension)         | `network_space::site_start_chain_for(space, last_used, account_chains)` (wasm `siteStartChainFor`, uniffi `site_start_chain_for`): after the network last used, the first network the account holds something on or used. The extension catalog carries `accountChains`.                                                                                                          |

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

## 9. Round 10 — the rules no shell can drift from

All additive (new events, operations, fields with serde defaults, new
functions) except `names::label`'s order and the Rust struct literals of
`MultisigOperation::Simulate`, `BalanceShellResult::FetchSettled`,
`PasskeyPurpose::Approve` and `SendPurpose::Execute` (one more field each).
Each rule has a shared vector the native shells replay.

### 9.1 Names (r9-12, r8-25, r4D-5) — one name per address, everywhere

| Was                                                                                                 | Now                                                                                                                                                                                                                                                                                                                                                                              |
| --------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| sheets drew `member_labels`, headers / Activity / Settings drew this device's own name              | EVERY place an address is named — header, switcher, Activity, Settings, sheets, invites — draws accounts view `member_labels[lowercase address]`, this device's own accounts included; the local name is only the fallback while the machine has none. Answer `index_wanted` (it names this device's own accounts too) with `index_names`.                                       |
| `names.label`: kept → index → own → short                                                           | index (the account's own published name, the same answer on every device) → kept (a multisig's label: an invite's, or typed when the member was added) → own (local) → short. Decided by best practice + the core principle: only the published name can be the same on a device that never took the invite. Vectors: `tests/vectors/multisig/names.json` through the link door. |
| login named an account after the signing key's user handle                                          | the published `key_names[0]` first (`login::reconstruct_account`).                                                                                                                                                                                                                                                                                                               |
| fixtures: the multi-key wallet 0x88cC… named "Parallel One" (Android, iOS) / "Parallel Multi" (web) | "Interleave" — what the passkey index publishes for it — `dev_fixtures::MULTI_NAME`, uniffi `fixture_multi_name()`. Each key keeps its own name (Parallel One/Two/Three); two addresses never share one.                                                                                                                                                                         |

There is no rename of a Vela account on any shell today (the name is set
and published at creation); a rename, when one is added, publishes through
the registry (`RegistryPublish` metadata `key_names`) — never local only.

### 9.2 The rule chip (r9-14)

Proposals tells accounts `AccountUpdate::RuleNow {chip: MultisigRuleChip}`
with `MembersNow` (the rule in force per network, `current_rules`); accounts
keeps it per multisig (`vela.multisigRulesNow`) and serves
`AccountRow.rule` (`MultisigRowRule {chip, from_chain, last_known}`). Draw
`rule.chip` in the switcher, the header and Settings for a multisig NOT
followed (the followed one keeps proposals `rule_chip`); with `from_chain &&
last_known` add `home.multisig.presence.ruleLastKnown`. Route `rule_now` to
accounts `apply` like `members_now`.

### 9.3 The balance stamp (r10-1, r9-3)

`BalanceView.last_refreshed_at_ms` moves only when every network the shown
account lives on that the round asked answered (`balance_dashboard::
stamp_moves`): the networks it holds something on (read or
`holdings_restored`) plus NEW event `lives_on {address, chain_ids}` (a
multisig's networks; send it before or with `account_changed`). An account
living on none of the asked networks is stamped by any answer; nothing
answering never stamps. An account or space switch clears the stamp; it
never moves back. NEW `FetchSettled.read_at_ms` (optional): a token-cache
hit says when its rows were read — never "now". Shells pass `read_chain_ids`
(every chain asked) and draw the field as it is (`null`: no "Updated").
Vectors: `tests/vectors/balance-stamp.json` (machine steps).

r11 (desktop S4): for an account said to live somewhere (`lives_on`), the
home names only those networks (and ones it held something on) in
`unreachable_networks`, and `balance_partial` / `notice` ("Some balances
are still updating") count only their failures: "Can't reach OP Sepolia"
said nothing about a multisig used only on Arc Testnet. A Vela account
(nothing said) keeps every network. The floor and the retries are
unchanged. Shells draw the fields as they are.

`ProposalsView.unreachable` ("The multisig service can't be reached") is
true only when the service itself missed a mailbox read on a network whose
chain this device could read; a dead chain is `chainUnreachable`.

### 9.4 Incoming funds (r9-17, r8-13) — `incoming_scan`

The scan's networks, window, backfill and cadence are the core's. Shells
run `ScanIncomingTransfers` through token_trust `scan_requested {address,
facts: {held_chain_ids, account_networks: [{chain_id, deploy_block}],
space_chain_ids, added_chain_ids}, cursors}` (push `registry_tokens_snapshot`
for every chain of `incoming_scan_plan(facts).scan_chain_ids` first), keep
`TrustView.cursors` under `vela.scanCursors.<address>`, and drive the
cadence with activity_feed `presence_changed` / `scan_due {seq}` (one timer
per `FeedView.next_scan.seq`). Backfill below the live 100 blocks: up to a
week, at most 400,000 blocks, 10,000-block reads, 4 per poll, never below a
known deploy block. uniffi `incoming_scan_plan`, `incoming_scan_windows`,
`incoming_next_scan_ms` (wasm `incomingScanPlan`, …). Vectors:
`tests/vectors/incoming-scan.json`.

**The cadence after round 10 (a follow-up).** Adopting the round-5 numbers
(10 s while the person was at the page) took a real Redmi K40's 200-row
history scroll from ~1–2 % janky frames to 5.63 %: the scan ran most
exactly while someone scrolled, and every scan re-read the store and
redrew the list twice. Decided by best practice (back off when nothing
arrives, scan when a person looks, never in a gesture's way):

| Was                                                          | Now                                                                                                                                                                                                                                                                              |
| ------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 10 s touched lately, 30 s idle, 60 s hidden                  | a back-off by scans in a row that found nothing: 15 s → 30 s → 60 s → 2 min (`CADENCE_STEPS_MS`); 2 min hidden. Something arrived: 15 s again. `incoming_next_scan_ms(visible, quiet_scans)` — the second argument changed (was `since_interaction_ms`).                         |
| at once on shown / online                                    | at once on shown, online, and NEW `presence_changed` changes `refreshed` (a pull-to-refresh: store read + scan) and `sent` (the person sent from this account: the scan alone); each starts the back-off again. `interacted` is accepted and no longer brings a scan closer.     |
| a touch brought the next scan to 10 s                        | NEW event `interacting {on, presence}`: say `on` when a scroll / fling / drag starts and `off` when it has settled. A scan that comes due meanwhile is held and runs at `off`; a scan that lands meanwhile draws its next timer at `off`. An `on` older than 60 s holds nothing. |
| each due scan: store read + scan + render, then render again | a due scan is the scan alone — no store read, nothing drawn — and its landing renders ONCE, carrying the next timer (and, when something arrived, the new rows through the one re-read). Records other machines change still re-read through `reconcile_completed`.              |
| a timer armed at every tick                                  | `next_scan` is absent until the hand-off's scan lands; the timer is armed when a scan lands. A scan asked for while one is out runs once more when it lands (never two at once); a scan its shell never answered stops counting after 5 min.                                     |

Shell work: report `interacting {on}` from the list's scroll state (Android
`LazyListState.isScrollInProgress`, iOS `ScrollPhase`/`isTracking ||
isDecelerating`, the web's `scroll`/`wheel`/`touchmove` with a short
settle), `refreshed` from pull-to-refresh, and `sent` after a send is
handed off. A shell that sends none of them still gets the back-off and the
one-redraw scan. Vectors: `cadence` (the rule) and `cadence_steps` (the
machine, step by step) in `tests/vectors/incoming-scan.json`.

### 9.5 The review and the outside page

| Was                                                                                       | Now                                                                                                                                                                                                                         |
| ----------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| a lone batch read as `review.callNumber` with `n` = `""` ("第 笔调用") (r10-10)           | `reading::Call.leg` (`"2.1"`, `null` for the lone call: draw NO number). Never join `path`. Lists inside a sentence use the platform's locale list formatter.                                                               |
| deep4 / delegatecall refused, yet "expected to fail" (r10-6)                              | a review refused for what it is withholds a revert too (`StateVerdict::Withheld`, no `Concern::Reverts`), and proposals never looks at it (no preview, no `blocked: fails`).                                                |
| an undeployed multisig simulated from a bare address: "expected to fail" (real-device S2) | `MultisigOperation::Simulate.state_overrides` (optional JSON): put it as it is into `eth_simulateV1`'s `blockStateCalls[0].stateOverrides` — the multisig placed as its deployment leaves it (`Reads.sim_state_overrides`). |
| "Couldn't read its members" until a reload                                                | the members machine reads an unread network again itself (`MEMBERS_REREAD_MS`, `wait` operations).                                                                                                                          |
| /connect: the core's English `refusal` beside the translated reason (r10-5)               | draw `review.refused.key`'s words once.                                                                                                                                                                                     |
| the signing page could not say a member change before → after                             | `PasskeyPurpose::Approve {.., owners, threshold}`, `SendPurpose::Execute {.., owners}`; pass them to `TrustedSignerApprovalInput` (optional). Page 797489e6 (first in `BUILD_ALLOWED`; `LAUNCH` unchanged).                 |

### 9.6 The web and the extension (no native change)

- The extension's start chain: `vela.ext.accountChains` (per signed-in
  account, written by the wallet page only, never emptied by a page that has
  not read holdings); the catalog's `chains` hold both spaces' networks,
  `order` the space in front's. Spec 105's line, kept (coordinator): a new
  site starts only on a network of the space in front (the network last used
  there, else one the account holds or used there, else the space's default),
  and a page's switch reaches only that space's networks
  (`network_space::site_may_switch_to`, the worker's `switchRefusal`): a
  network of the other space is 4902 naming that space, one Vela lacks
  "not in Vela's networks". Moving between spaces is the person's own choice
  in Vela. Vectors `tests/vectors/site-start-chain.json` (`cases`,
  `switches`; Rust and the worker's JS twin).
- Welcome reads a multisig link with the core before it says it waits
  (`onboarding.welcomeWeb.linkBroken`).

### 9.7 Outside the multisig machines (every shell drives the same core)

- **Amount + fee (r10-3).** The send rules weigh amount + fee against ONE
  balance on every confirm path — native fee, in-band fee, Arc's USDC whose
  coin and ERC-20 interface are one balance (`manage_tokens::same_balance`,
  `rescale_units`; `fee_policy::fee_in_units_of`), Max's reserve — and the
  dApp sheet's fee check subtracts the request's own amount. New
  `FeeOptionView.short_with_amount` (words `send.warnInsufficientForGas`).
  Vectors `tests/vectors/rules/send-fee-cover.json`. The r10 revert itself
  came from the harness relay reporting a fixed balance; the fee machine
  still takes the relay's balance figure (a follow-up: cross-check with the
  wallet's own read).
- **Passkey errors (real-device S3).** `app::passkey_error::words(platform,
domain, code, message) -> {kind, key, failure}` (uniffi
  `passkey_error_words`, wasm `passkeyErrorWords`): a corpus key
  (`onboarding.common.passkeyError.*`, `phoneLinkFailed`) for the platform's
  error facts, never the system's English. Onboarding's `passkey_failed`
  takes optional `error {platform, domain, code, message}`; the core fills
  `PromptKind::{CreateFailed, SignInFailed}.words_key`. Vectors
  `tests/vectors/rules/passkey-errors.json`.

### 9.8 Shell notes from the native fixers (round 10)

- **Android** (branch of the Android r10 fixer, merged by the coordinator):
  the sheet title from `review.reading` (r10-8); an executed sheet draws its
  frozen facts and asks the timeline once more when done (r10-7);
  `waiting_note` drawn as it is; the "Updated" stamp hidden while
  `checking_key` is set; lists through `ListFormatter`
  (`MultisigWords.list`); `recipient_facts.first_time = false` for this
  device's own accounts and multisigs; the notification ask marked on its
  answer; WorkManager rounds on an INTERNET (not VALIDATED) network request,
  plus an expedited round on leaving. r10-6: the shell also drops
  `reverts` / `unsimulated` beside a withheld simulation and
  `unreadableBatch` beside `refused.why = tooDeep` (one red box) — the core
  no longer emits them (§9.5), both are compatible. Its new on-demand keys
  (`home.multisig.notify.*`, `home.multisig.create.close*`) move the
  multisig pin; this branch's own +1 (`presence.ruleLastKnown`, 665) is to
  be added on merge.
- **Desktop**: the deep-batch refusal said once (`reading::
concern_blocks_beside`); `SendPick.loading`; network status beside the
  stamp; `TokensPartial` only with a held balance.

## 10. Round 10 follow-ups — one rule, at the source

All additive (new events, view fields with serde defaults, new functions)
except two signatures: uniffi/wasm `incoming_next_scan_ms(visible,
quiet_scans: u32)` (was `since_interaction_ms: f64`) and, Rust only, the
desktop's own `wallet_keys` / `device_key_rows` / `more_networks` (one more
argument each).

| #   | Was                                                                                                                          | Now                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| --- | ---------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | the incoming scan every 10 s while the person was at the page; two renders per scan (5.63 % jank on a Redmi K40)             | §9.4's table: a back-off 15 s → 30 s → 60 s → 2 min, at once on shown / online / `refreshed` / `sent`, held during `interacting {on, presence}`, one render per scan. Vectors `incoming-scan.json` `cadence`, `cadence_steps`.                                                                                                                                                                                                                                                                                                                                                    |
| 2   | `unreadableBatch` listed beside `refused.why = tooDeep`; the desktop (and Android) filtered it                               | the review leaves that concern out once it has decided the tone (`multisig_review`); the desktop's `concern_blocks_beside` filter is gone. Android's own filter is now a no-op and can go.                                                                                                                                                                                                                                                                                                                                                                                        |
| 3   | the in-app browser: "Chain 5042002 is not in Vela's networks" for the other space's network; an add of one reached the sheet | NEW event `space_networks_changed {space, chain_ids, known_chain_ids}` (send it in place of `networks_changed`): the other space's network is 4902 with the extension's sentence (`network_space::switch_refusal_message`, en of `connect.browser.switchRefusal.*`), the status line `componentsUi.browserStatus.reason.otherSpace{Testnet,Mainnet}` (in `last_failure.key`; the reason stays `unknown_chain`), and an add of one is refused the same way.                                                                                                                        |
| 4   | a site kept its network across a space switch                                                                                | on a space switch every site outside the new space moves to its start (`network_space::sites_moved_by_space_switch`; the worker's `spaceSwitchMoves`); pages hear `chainChanged`; the next request that opens a sheet carries `DbrConsentView.space_note` / `DbrSigningView.space_note` `{chain_id, key: "connect.browser.spaceMoved"}` — hand it to the sign machine as NEW `space_noted {id, chain_id}`, and draw `SignRequestView.space_note` under the requester ("{{network}}" = that chain's name). Vectors `site-start-chain.json` `switches[].message`, `space_switches`. |
| 5   | a key row could take the account's name ("Interleave" over "Parallel One")                                                   | `wallet_keys::step_for_account`; `step_json`'s device argument may be `{"account_name", "keys"}` — hand it the record's keys with their own names (not `walletName` for key 0) and the account's name. A label that is the account's name, or the registry's `key_names[0]` (the wallet's published name), labels no key ("Key n").                                                                                                                                                                                                                                               |
| 5b  | a key with no name of its own was drawn blank on the web (the e2e's "E2E Wallet" became "")                                  | never blank: every `WalletKeyRow` is called by its own `name`, else by NEW `label_key` (`settingsModals.keys.keyN`, "Key {{n}}") with NEW `label_n` (its place in the founding order, from 1) — the same before, during and after the walk. Not the authenticator (that is the row's caption, said once and only once the registry vouches for it), not where the key was made (nothing records it honestly: issue 207, spec 102). Vectors `wallet-keys/labels.json`.                                                                                                        |
| 6   | the desktop joined lists in sentences with ", "                                                                              | `vela_core::l10n::format_list(locale, items)` — CLDR's "and" list from ICU (gen-i18n Stage 5b, `l10n/list_data.rs`); the desktop's `Loc::list`. iOS/Android keep the system formatter, the web `Intl.ListFormat`.                                                                                                                                                                                                                                                                                                                                                                 |
| 7   | Android: a title-only "Failed to load tokens." over the in-app browser (the core's alert, a minute after Send was left)      | a late answer after the flow closed or NEW `left` raises nothing; the failed list is `SendView.tokens_failed_key` (`send.tokensUnavailable`) with "Try again" (`common.tryAgain`) → NEW `retry_tokens`. The `ShowAlert {load_tokens_failed}` still goes for a shell not moved; one that draws the state skips it.                                                                                                                                                                                                                                                                 |

Shell work these leave (no shell can drift; each is a draw or a send):

- **Feed** (all): `interacting {on}` from the list's scroll state,
  `refreshed` from pull-to-refresh, `sent` after a hand-off. Android's
  `FeedWire` declares them (wire parity); nothing sends them yet.
- **In-app browser** (Android, iOS): `space_networks_changed` in place of
  `networks_changed`; `space_note` on the consent and signing sheets (via
  `space_noted`).
- **Keys** (Android, iOS): the record's own key names and
  `{account_name, keys}` to `wallet_keys_step`; an unnamed row says
  `label_key` with `{{n}}` = `label_n` (#5b) — never blank, also while the
  registry is asked.
- **Send** (Android lane): send `left` when Send is left; draw
  `tokens_failed_key` in the picker; draw the send alert only on Send.

## 11. Round 11 — one fee, one verdict, one name

All additive (new functions, new view fields with serde defaults, a new
enum) except one behaviour of the fee machine (r11-8, below) and the Rust
struct literals of `ProposalItem`, `OutsideItem`, `OutsideTxShown` and
`MultisigReview` (one more field each).

### 11.1 The fee (r11-8)

The same plain 0.01 USDC send from a deployed account, through the same
relay, was priced and charged 0.045 on Android, 0.1092 on iOS, 0.1168 on the
desktop and 0.1224 on the web. Read from the bundles on Arc Testnet (the
relay answers without `settlementGas`, so the charge is `3 × gas × basis`):
iOS, the desktop and the web all priced the relay's limits (500,000 +
1,000,000 + 100,000 = 1,600,000) at Arc's `eth_gasPrice` of the moment
(22.75 / 25 / 25.5 gwei), so those three are the same rule. Android's relay
estimate never came back from the emulator. Its shell said `simulation_failed`,
the core priced the static model (300,000 + 200,000 + 100,000 = 600,000 →
0.045), and the submit then measured again and signed the relay's 1,600,000.
That fee was under-provisioned against the limits it carried, and the gas it
was priced on was not the gas signed.

| Was | Now |
| --- | --- |
| `FeeGasOutcome::SimulationFailed` on a small operation → the static model | no simulation, no fee: `FeeFailure::EstimateFailed`, asked again on the schedule (3 s, 6 s, 8 s…) |
| each shell deciding "refused" vs "failed" (iOS: refused → `simulation_failed`, unreachable → `context_unavailable`; Android: both → `simulation_failed`; the desktop's own `refuses()`) | `fee_policy::estimate_gas_outcome(error_json)` (uniffi `fee_estimate_gas_outcome(error_json?) -> "refused" \| "simulation_failed"`, wasm `feeEstimateGasOutcome`): `refused` only on the relay's own "simulation failed" / "execution reverted" / "reverted during simulation", never for an `AA..` code |
| `Refused` on a small operation: the static fee, then the submit measured again | the static fee AND `prepared` = the static limits (L2 adder in `preVerificationGas`), so the submit signs exactly what was priced |

Shell work: send the core's outcome for every failed estimate; sign
`prepared` whenever the fee carries it (every shell already does).
Vector `tests/vectors/fee/plain-send.json`: wire answers → the shell results
every shell must produce → the machine's fee. Replayed by
`tests/app_fee_parity.rs` and the web's `fee-plain-send-parity.test.ts`.

### 11.2 Sent or not sent (r11-9)

| Was | Now |
| --- | --- |
| each shell reading its own exceptions into `not_connected` / `network` (a refused connect in 4–6 ms said "It may have been sent… don't send it again" on Android) | `rpc_pool::TransportFailure` — the error CLASS the shell passes — and `transport_outcome_of(class)` (uniffi `transport_outcome_of(failure) -> JSON`, wasm `transportOutcomeOf`): `offline`, `dns`, `connect_refused`, `connect_timeout`, `tls`, `refused`, `write_failed` → `not_connected` (not sent); `after_write`, `unknown` → `network`; `read_timeout`, `timeout` → `timeout` (may have been sent) |
| one-press approve-and-execute with a look that could not price or check it: an `execute:` state `stopped {unchecked}` beside the approval, drawn as "This would fail if executed now: it couldn't be checked…" next to "Couldn't check whether it would run — you can still approve" | the approval is the one verdict. Nothing is recorded for the execution, and the look's own line (`proposal.feeUnchecked` / `feeUnpriced`) is the one note |

Draw `ActionState::Stopped` by its preview's kind: `fails` →
`proposal.wouldFail {reason}`, `unpriced` → `proposal.feeUnpriced`,
`unchecked` → `proposal.feeUnchecked`. Draw it in place of the fee line,
never beside it, and never `wouldFail` for a look that did not check.
Vector `tests/vectors/rules/transport-failures.json` (each class, its
outcome, `may_have_delivered`, and the submit verdict; per-platform guidance
in `platform`). Android: tell `write_failed` from `after_write` with
OkHttp's `EventListener` (`connectFailed` / `requestBodyEnd`). The web's
`fetch` names only `offline` and its own `timeout`; anything else is
`unknown`.

### 11.3 Names (r11-6)

| Was | Now |
| --- | --- |
| short addresses 6+6 on the web and Android, 4+4 on iOS and the desktop | `names.short` (`multisig_rules::names::short_address`) is THE short form: EIP-55, `0x` + 6, `…`, 6 (`0x88cCA0…266894`). Six and six because a 4+4 look-alike is a 2^32 search for an address-poisoning sender and 6+6 is 2^48 |
| "Add 0x88cCA0…266894" (web Activity, outside page), "Add 0x88cC…6894" (desktop Activity), "Add Interleave (…)" elsewhere | `names.who {address, label}` (`names::who`): "Interleave (0x88cCA0…266894)", or the short form alone when the label is none. Every title that names a member — sheets, Activity rows, the outside page, notifications — uses it with `label` = `member_labels[lowercase]`. The web's Activity draws a member change with the names as they stand, never the first drawing. The outside page asks the passkey index itself (`names.label` with the published name only) |

A name shown to others is only ever the member's published one (the
passkey index), a kept label, or the short form — never this device's
local-only name presented as the member's own.

### 11.4 S4 — counts, signs, headings, numbers

| Was | Now |
| --- | --- |
| "9 need your approval" counting deep4 and the delegatecall, and "Ask members to approve" on them | a transaction Vela refuses for what it is (`multisig_review::refused_for_what_it_is`: a delegatecall, a refund, a batch too deep) has empty `can_approve` and `can_execute` at the source. Every count, "Your turn" and the ask drop it. Hide the ask when `can_approve` is empty and `review.refused` is set |
| "−0.01" on the web, "-0.01" on the desktop Activity and the Android / iOS sheets | `l10n::signed_amount_text(amount, outgoing)` (uniffi `signed_amount_text`, wasm `signedAmountText`) for a written amount; `format_signed_token_amount` from base units. U+2212 only |
| "SIMULATION RESULT" (Android) vs "Balance changes" | `MultisigReview.moves_title_key` (`componentsUi.signing.balanceChangesTitle`; `null` when the simulation is withheld) |
| "It can run once…": every earlier number (web), the previous one (natives), "#0" (outside page) | `ProposalItem.waits_for`, `OutsideItem.waits_for`, `OutsideTxShown.waits_for`: every number from the multisig's nonce up to its own, at most 20 (`multisig_proposals::waits_for`). One → `proposal.queuedNote {nonce}`, more → `proposal.queuedNoteMany {nonces}` (the platform's list of `#n`). The outside page now says the wallet's sentence |

### 11.4b What waits names its transactions (r11-1, from the Android lane)

`multisig_presence::Waiting` gains `needs_you_keys` / `ready_keys`
(`<chainId>:<safeTxHash lowercase>`, from the same lists the counts come
from — so a refused transaction is in neither) and `read_at` (the followed
list's read time, an others tick's start, `None` for a list kept from an
earlier page). A notifier keys "already said" by transaction, and a
background round speaks only from reads that began after it did. iOS's
notifier keys by count the same way and can adopt it.

### 11.5 The web and the extensions (no native change)

- r11-4: the proposal sheet draws `review.refused.key` whenever it is set,
  whatever `action` is, and asks nobody to approve a transaction nobody can
  sign.
- r11-5: the switcher's (and Settings') multisig rows draw `AccountRow.rule`
  with "Last known".
- Add member prefills Name with the address's published name (index), never
  a local or kept one, and never over what the person typed.
- r11-2 (Chrome extension): a request the side panel took whose sheet could
  not be built (a read before the sheet threw) is answered `-32603` "nothing
  was signed — try again" instead of staying owed behind the plain wallet.
- Safari Web Extension: the development package hands signatures to the
  `dev-seed.html` `page=` origin (loopback with a port only); the release
  package always names `https://sign.getvela.app`.

## 12. The review's state — nothing signed on a review not read

The core drew a draft first and filled its review in a moment later; until
it came, iOS read the empty review as "allowed" (`?? true`), so "Propose and
approve" could be pressed before anyone saw a review. On an error the review
stayed empty too: no shell could tell "still loading" from "failed". The
owner's rule decides it (decided by the coordinator): no blind signing — a
review that was not read is a blind signature.

All additive (new view fields with serde defaults, new events, a new
module) except the Rust struct literals of `DraftView`, `OutsideTxView` and
`TxShown` (one more field each).

### 12.1 The state (`multisig_review_state`)

```text
ReviewState (tag kind): loading | ready | failed {reason_key}
```

| Where                                                            | Field                                                                         |
| ---------------------------------------------------------------- | ----------------------------------------------------------------------------- |
| every draft — send, member change, rename, reject's cancellation, a site's request | proposals `draft.reviewState`                               |
| every proposal a member approves or executes                     | proposals `review_states["<chainId>:<safeTxHash>"]` (beside `reviews`; missing = not asked yet = `loading`) |
| the outside page's link transaction / Vela's request             | `tx.reviewState` / `requestReview.reviewState`                               |

- `loading` is set the moment a review is asked (`draft`, `review`, a link
  opened) and is **bounded by the core**: `REVIEW_TIMEOUT_MS` = 40 s (above
  the reads' own bounds — three code rounds and the state reads at
  `REVIEW_READ_MS` each — so only a read a shell never answers is cut). Past
  it: `failed {home.multisig.review.state.timedOut}`. It can never spin for
  ever, and it never becomes "allowed" by default.
- `failed` reasons: `timedOut`, `unreadable` (the core could not read the
  transaction or gather its review), `gone` (the proposal is no longer in
  the list it was opened from).
- One read at a time per review: a shell asking `review` again while one is
  read never pushes the bound back. A quiet re-read (`review` on a `ready`
  one) keeps the review drawn and does not unsay it if it fails — the gate
  reads the chain again before any passkey anyway.
- A draft proposed from a `ready` review hands that review to its proposal
  (`review_states[...] = ready`): its execution signs from it.

### 12.2 Events

- proposals `review_again {chain_id?, safe_tx_hash?}` — both: that
  proposal's review; neither: the draft's. `loading` at once, then `ready`
  or `failed`; it supersedes a read under way.
- outside page `review_again` — the link's transaction's review (when not
  `ready`) and every request whose review is missing, read again.

### 12.3 The door — the only thing that enables a button that signs

`multisig_review_state::door(state, review) -> SignDoor {shows: loading /
review / failed, enabled, anyway, reason_key, retry}` (link door `review.door
{state?, review?}`; uniffi `multisig_link_call`, wasm `multisigLinkCall`).
`enabled` only when the state is `ready`, a review is there, and its
`sign.allowed` is true; `anyway` → the existing "… anyway" step. A missing
state, a missing review, a missing sign rule: shut.

Shells: every sign / propose / approve / approve-and-execute / execute /
"sign in wallet" button is enabled only by `door.enabled`; `loading` draws
`home.multisig.review.state.loading` ("Reading the transaction…") where the
review goes, the button disabled; `failed` draws the reason's words,
`.blocked` ("Nothing can be signed until it's read.") and `.tryAgain`,
which sends `review_again`. A failed review is not retried behind the
person's back. No shell may treat an absent review as allowed.

### 12.4 The core's own backstop

Whatever a shell lets the person press: `confirm_draft` / `propose_execute`
on a draft not `ready`, and `approve` / `execute` / `approve_execute` on a
proposal not `ready`, sign nothing (`ActionState::SigningUnchecked`, no
passkey asked). The outside page offers no approve / execute / reject
(`OutsideTxView::open` requires `ready`) and hands no SafeTx request to the
wallet (`answer_request` with `go` is ignored) unless the door is open.

Words: `home.multisig.review.state.{loading, blocked, timedOut, unreadable,
gone, tryAgain}` (on-demand multisig namespace, all 15 locales; pin 682).
Vectors: `tests/vectors/multisig/review-state.json` (`door` cases, the
timeout, the keys) — replayed by Rust (`app_multisig_review_state.rs`), the
web (`core/review-door.test.ts`), the desktop, Android and iOS. Machine
tests: `tests/app_multisig_review_state.rs`, and the outside page's in
`tests/app_multisig_outside.rs`.

## 13. Round 12 — amounts said, outages named, one title, one fee row

All additive (a new event, new view fields with serde defaults, new link-door
rules, a new module) except two wire shapes: the multisig shortfall's `have` /
`need` (strings → objects, r12-1, deliberately so no shell can scale them by a
guess) and the Rust struct literal of `RpcPoolView` (one more field). Each
rule has a shared vector the native shells replay.

### 13.1 Amounts carry how they are said (r12-1)

| Was | Now |
| --- | --- |
| a shortfall `why = {kind: "short", token: <address>, have: "<base units>", need: "<base units>"}`; Android / iOS read `why.token.decimals ?? 18` (the token is an address string), so Arc Testnet's 0.01 USDC (6-decimal interface `0x3600…`) read "needs <0.000001" | `have` and `need` are `MultisigAmount` (`multisig_rules::amount::Amount`): `{base_units, token, said}`, `said` = `{kind: "known", text, number, symbol, decimals}` (draw `text`, "0.01 USDC") or `{kind: "decimals_unknown", key, raw}` (draw the corpus key `home.multisig.review.reading.decimalsUnknown` with `{{raw}}`). Decimals and symbol: the registry first, else the token contract's own answer (read by the look), never 18. `token` stays the address (lowercase), or null for the network's coin. |
| each shell scaling base units with its own formatter | the core's text (`format_units`). Door `amount.of {base_units, chain_id, token?, symbol?, decimals?}` for any amount a shell has in base units. |

Where: `previews[...].why`, `actions["execute:…"]` `stopped.preview.why`,
`standings[...]` `blocked.fails.why` (the home's "Can't run now — …"). Shell
work: draw `said`; delete any `?: 18` / `?? 18` for these. Vector
`tests/vectors/multisig/amounts.json` (through the door; Rust
`multisig_rules::amount::tests`). Generated TS `MultisigAmount`,
`MultisigAmountSaid`.

### 13.2 A network that does not answer is never a confident zero (r12-2, r12-3)

**r12-2, found.** The web's balance machine cannot say "Live · listening for
payments" while any network is in `failed_chain_ids` (`live_key` needs
`!partial`), so the round said every network answered — and for the web it
had. On the round's host, `rpc.soneium.org`, `mainnet.unichain.org` and
`forno.celo.org` hang for a DIRECT connection, but answer in under a second
through the macOS system proxy, which the browser (and the iOS simulator)
use. The Android emulator went through the harness proxy and the desktop
through `VELA_DEV_PROXY`, both direct out: they could not reach the three and
said so. Reproduced on this host (curl `--noproxy` hangs 8 s; Playwright
Chromium answers in 0.7 s). The cold rule held; the web's warm path did not
(below), and that is the same "Live over a silent network" shape.

**r12-3, found.** After one complete round, nothing the web reads is forced:
the 10-minute poll, focus and account switch are non-forced, and the shell's
token cache (five minutes) answered them with "every network answered" (no
`onFailedChains` on a hit) — the core then cleared `failed_chain_ids`, reset
the recovery reads and kept "Live". A partial round was also stored under the
previous complete round's time, so the cache stayed "fresh" over a network
that had just failed. The pool knew (the incoming scan and the native-balance
watch read through it and recorded the network unreached); nothing told the
balance machine. Android had no token cache, so every read reached the
network.

| Was | Now |
| --- | --- |
| a cache-answered settle (`read_at_ms` set) replaced `failed_chain_ids` with its own `[]` | it never clears a network the machine knows it could not read, nor a doubted one (`suspect`): those stay, the round stays partial, the recovery reads stay armed — no "Live" over them. |
| the pool's failures stayed in the pool | NEW `RpcPoolView.out_of_reach_chains` (the chain notice's own set: `failed_chains` ∪ `unreached_chains` − `rate_limited_chains`) and NEW balance event `pool_unreached {chain_ids}` — send it whenever that set changes. A network newly in it that the shown account is read on (asked by its last round; for an account said to live somewhere, one of its networks) and that the home does not say yet is read again AT ONCE, forced past the shell's cache. A round already out, or an app away, owes it: the read runs when that round settles, or at the next focus (forced). Before the account's first round settles, nothing extra is read. |
| (web) a partial round cached under the previous complete round's time | `fetchedAt: 0` — a partial round is never fresh. |

Shell work (iOS, desktop, Android): subscribe to the pool's view and send
`pool_unreached {chain_ids: view.out_of_reach_chains}` to the balance
machine on every change (the web: `rpc-pool.ts` `onPoolOutOfReach` →
`balance-resident.ts`). A shell with a token cache of its own must answer a
`fetch_tokens {force: true}` from the network. Vectors
`tests/vectors/balance-pool-unreached.json` (machine steps: `event`,
`answer`, `expect {fetch: {force} | null, unreachable: [ids], live}`;
Rust `the_pool_unreached_vectors_hold`). Web e2e
`e2e/home-unanswered.e2e.ts`: nodes that hang (never fulfilled, not a 500) —
three from the start (said, never Live), and Ethereum going silent in a warm
session (named without a reload; red without the fix).

### 13.3 One title for a member change (r12-8)

| Was | Now |
| --- | --- |
| Activity froze an executed member change's title when the row was written — the language and names of that moment ("Add 0xD40086…de130b" on a zh page) — while the sheet said "Add Dana Local (…)" | link-door rule `names.change {action, labels}` → `{key, params, count?}` or null: `home.multisig.change.{add, remove, replace}` with each member as `names.who(address, labels[lowercase])`, or `home.multisig.action.changeThreshold` with `count` (say its plural form). `labels` = the accounts machine's `member_labels` (a sheet: the name typed for the member there, else that). Fill it in the page's language when the row is DRAWN. |

Shell work: the Activity row of an executed member change keeps the action
(the web: `LocalTransaction.multisigChange`) and is worded by this rule when
it is drawn (the web re-titles stored rows when the page's words or
`member_labels` change: `rewordMemberRows`); the sheet's title uses the same
rule. Vector `tests/vectors/multisig/member-titles.json` (Rust
`names::tests::the_member_title_vectors_hold`).

### 13.4 S4 — one note, one fee row

| Was | Now |
| --- | --- |
| proposal #0: "expected to fail" (the review's `Concern::Reverts`) beside "This would fail if executed now: …" (the look's `fails`) | `ProposalsView.reviews` drops the review's `reverts` concern for a proposal whose look (`previews`, or `actions` `stopped`) says `fails`, and serves a reverting `simulation` withheld (`kind: "withheld"`, no `notice_key`, `movesTitleKey` null — the shape every shell already draws as nothing, r9-9): the look's line, with its reason, is the one note (`multisig_proposals::one_note_per_failure`). Nothing for a shell to do; a shell guard that hid the simulation's notice beside a failing look can go. |
| confirm fee "~0.12" (Android, iOS) vs "0.12" (web, desktop) | `fee.row {quoted}` (link door; `fee_policy::confirm_fee_row`) → `{label_key, amount_prefix}`: the amount is the fee signed and charged (r11-8), drawn bare after `amount_prefix` (empty — never "~"); `label_key` `send.estFeeLabel`, or `send.feeTokenEstimate` when `quoted` is false. Vector `tests/vectors/fee/confirm-row.json`. |

### 13.5 The web, the extension, the page (no native change)

- **The extension's new-site network (the third time).** The stored
  `vela.ext.accountChains` was right and late: an account funded on Arc
  Testnet seconds before a connect had no page that had read its holdings
  since; the side panel booted at the click, resolved the start chain once
  before its own balance round ended (Sepolia), and the grant kept it. Now
  the panel reads the account's holdings first (bounded, 8 s) and stores
  where it lives (one helper, `accountLivesOn`), and a new site's consent
  follows the stored inputs (`watchStartChain`) until Connect — the request
  window too. `e2e/extension-site-start.e2e.ts` reproduces the round
  headless (late funds, a slow node, the real side panel; red without the
  fix) and runs in `ci.yml`'s `web` job on every pull request
  (`docs/ci/test-loops.md`).
- **The signing page (r12-7).** Every short address is `names.short`'s form:
  EIP-55, `0x` + 6, `…`, 6 (was 4+4 in whatever case it was handed,
  "0x3e9b…692d"). Page **987ccbcc** (first in `BUILD_ALLOWED`; `LAUNCH`
  unchanged; deploy `dist/` first).
- S4: "Open #n" and the fee's Retry in neutral link ink (the danger red is
  Reject's); a space before every "·" (Svelte 5 drops whitespace at the start
  of a tag: "R12 Alice· On this device", "e56405· Needs 2 of 4"); the outside
  page no longer prints a changed member's full address twice (the party
  list skips the members `MemberChangeRule` draws).

### 13.6 Two onchain regressions, fixed (coordinator, round 12)

| Was | Now |
| --- | --- |
| /connect: Vela's request for a multisig deployed nowhere and recorded with no service (imported from a Safe{Wallet} export) sat on "Reading the transaction…", then "Couldn't read this transaction" — the review-state merge rightly shut "Sign with …" until the review is read, and the page could never learn the members (before it, it was signed on no review) | NEW `OutsideSessionRequest.genesis` (optional): Vela sends the multisig's genesis beside the signing request (the WalletPair request object's `genesis` member). The page keeps it only when the core derives the address from it (`multisig_outside::genesis_for`; `OutsideSafeTxRequest.genesis`) and reviews from it. A forged one is not believed. A shell that rebuilds the session request field by field must carry `genesis` (the web's external sheet did not). |
| a request for the link's own transaction was read a second time | it takes the link's ready review at once; a request's whole read is bounded by `REVIEW_TIMEOUT_MS` (ready, or failed with its reason) |
| "Approve and execute" stuck on "Executing…" in the onchain suite | the harness, not the wallet: the core fork test gives the fixture accounts a nonce (`anvil_impersonateAccount`), after which their counterfactual accounts cannot be created on that fork (AA13). Each suite now gets a harness of its own (`nightly.yml`, the harness README). |

Tests: `tests/app_multisig_outside.rs` (`a_request_for_a_multisig_nobody_records_is_reviewed_from_the_genesis_vela_sends`, `a_genesis_that_is_not_the_multisigs_is_not_believed`, `the_session_sends_the_genesis_beside_its_request`, `a_request_for_the_links_own_transaction_takes_the_links_ready_review`), `tests/app_multisig_outside_propose.rs`; the 14 onchain tests on a fresh harness.

## 14. Real-device pass 2 — nothing checked is never one tap; a snapshot is its fetch's

Decided by the coordinator under the owner's clear-signing rule (no blind
signing, the Bybit lesson; a state-diff guard before signing; honest states).

### 14.1 D1 (S2) — neither safety check read signs only "… anyway"

**Found.** On a slow network each read gives up at its bound (8 s), so at
~36 s a review came back with the state-diff guard `unavailable` AND no
simulation — `ready`, a caution, and one tap. The test hand approved and
executed in that state. Round 13 (r13-2) found the same from another
cause: a node that REFUSES (not hangs) answers every read at once with an
error, and the review was `ready` in 2–4 s with a plain Approve — and the
same in the reads right after Try again. The rule reads the review's two
verdicts, not their cause: a hang, a refusal, a timeout or the window after
Try again are all "both unread" (vectors `checks` replay each cause's real
answers; `a_refused_node_reads_neither_check_and_signs_only_anyway`).

| Was | Now |
| --- | --- |
| guard unread + simulation unread: two cautions (`stateUnread`, `unsimulated`), `sign {allowed: true, anyway: false}` | NEW concern `unchecked` (Danger) beside them: `tone: danger`, `sign.anyway: true` (`multisig_review::unchecked_of`). Still `ready` — the review stays readable. |
| one of the two unread | unchanged, now explicit and tested: a caution, one plain press |
| the door said nothing over a `ready` review | `review.door` on a `ready` review with the `unchecked` concern: `{shows: review, enabled, anyway: true, reason_key: "home.multisig.review.state.unchecked", retry: true}` — `anyway` whatever the sign rule handed says. |
| outside page `review_again` re-read only reviews not `ready` | also the link's (and every request's) `ready` review that is `unchecked` |
| `SendPurpose::Execute` carried no "anyway": the executor's page said a plain "Execute" | `SendPurpose::Execute.anyway` (serde default) from the gate's sign rule; `approval::execution_context(…, anyway)` → `context.multisig.anyway`; the core's `trusted_signer::request` now builds `context.multisig` for `vela_multisigExecute` from the shell's params for every shell (`approval::execution_of_params`; the natives and the desktop had none — no number, no hash on their executor's card). |

Shell work (done on all four): over a `ready` review whose door has a
`reason_key`, draw that key as a DANGER line above the review with Try again
(`review_again`; testids `review-unchecked` / `multisig.proposal.reviewUnchecked`
/ `multisig.review.unchecked`); every sign press follows `door.anyway`
("Approve anyway", "Approve and execute anyway", "Execute anyway", "Propose
anyway"); executor params carry `anyway`. Found on the way: the web's and
iOS's "Approve" alone beside "Approve and execute" stayed plain under an
"anyway" review — it now says "Approve anyway" in danger too (the desktop
and Android already did). Words: `home.multisig.review.state.
unchecked` ("Vela couldn't check what this transaction changes."; pin 684).

The page **954b9013** (front of `BUILD_ALLOWED`; LAUNCH unchanged; deploy
`dist/` first): an executor's card whose `context.multisig.anyway` is true says
"Execute anyway" in the danger fill with `warn.msAppAnywayExec` (15 locales);
"anyway" never opens a refusal. The approval's "Approve anyway" (r9-10) already
followed `PasskeyPurpose.anyway`, which the gate now sets for D1 too.

Vectors: `tests/vectors/multisig/review-state.json` — five more `door` cases
(every shell replays them) and `checks` (guard × simulation; Rust replays
them through `multisig_review`). Machine tests: `app_multisig_review_state.rs`
(`a_review_neither_check_read_is_ready_but_every_press_signs_only_anyway`,
`one_check_unread_alone_signs_plainly`), `trusted_signer.rs`
(`an_executors_page_hears_the_number_and_the_reviews_anyway_from_every_shell`).

### 14.2 D3 (S2) — a balance snapshot paints only the fetch it came from

**Found (iOS shell; the core trusted it).** The balance executor's mid-fetch
snapshots were labelled with the account in FRONT when each landed
(`WalletStore.scopedTo`), not the account the fetch read. At launch the home
opens the session's account first (the member — the dev switch
`VELA_PARALLEL_ACCOUNT` put it in front) and the remembered multisig a beat
later, so the member's in-flight snapshots reached the core as the
multisig's: drawn ("$2.4M", "Updated 24m"), kept as its rows
(`HoldingsCache`, keyed correctly by address — it stored what the view held)
and restored on the next launch; Send and the pay link's Send offered the same
figures. The dev switch exposed it; any account switch during a fetch could.

| Was | Now |
| --- | --- |
| `chain_assets_arrived {address, tokens}`: the address alone, a shell's word | `BalanceOperation::FetchTokens.generation` (the account-and-space generation, bumped by every account or space switch) and `chain_assets_arrived {address, generation, tokens}` — REQUIRED: a snapshot whose generation is not the current one is dropped, whatever address it carries; addresses compare without case. |
| iOS: `chainAssetsArrived(tokens)` labelled with `scopedTo` | the executor hands each snapshot its fetch's own address and generation |

Every shell echoes the fetch's generation (web `balance-executor.ts`, desktop
`executor/balance_dashboard.rs`, Android `BalanceExecutor.kt`, iOS
`BalanceExecutor.swift`). The caches checked: every holdings cache
(`vela.holdingsCache`) is keyed `address#space` on all four; the cached total
by address. Send's figures are the balance machine's (one source), each
shell's answer gated on the balance view's address.

Tests: `app_balance_dashboard.rs`
(`another_accounts_snapshot_never_paints_the_account_in_front_whatever_its_label`,
`a_relaunch_and_a_space_switch_keep_each_accounts_rows_its_own`); iOS
`BalanceBleedTests` (the dev switch, the switch, the mislabelled snapshot,
Send's figures, a relaunch); Android `BalanceMachineTest`.

### 14.3 Smaller

- **D8** — the fee line under a shut Approve (the review being read) said
  "you can still approve": `home.multisig.proposal.feeUncheckedHeld`
  ("Couldn't check whether it would run — Execute checks again.", pin 685)
  wherever the door is not open, on all four shells.
- **D5** — a passkey creation the core's deadline ended (Google unreachable)
  is said: `create_wallet` takes a `cancelled` failure whose facts classify as
  `TimedOut` as a failure with "The passkey prompt timed out. Try again.",
  never the quiet "setup cancelled". Android races the platform call against
  `passkeySignTimeoutMs` (`withinPasskeyDeadline`) — it ends at the deadline
  even when the provider ignores its cancellation — and reports it as
  `TYPE_TIMEOUT_ERROR`.
- **D10** — the Android `-PvelaDeviceTest` build is labelled "Vela Test"
  (manifest placeholder `velaAppLabel`).
- **D6** — measured, not changed. On the simulator (PerfTests
  `testColdStartToCachedHome`, Release, n=5 each) the home's
  "named-account" gate (5fb15bffd) costs nothing measurable on a single
  account's cold start: 0.645 s with it, 0.620 s without (the 0.831 s first
  run came right after the seed launch). With one account the balance view
  names the shown account from its first frame (logged frame by frame). The
  device's 0.51 s was taken with a multisig shown at launch: the home then
  waits for the multisig machine to name it, and drawing before that is
  exactly the member's figure under the multisig's name (S3, D3). The gate
  stays.

## 15. Round 13 web/core fixes — a hung read is bounded and named; a home claims nothing of a space it does not live in

Decided under the owner's rules (honest, recoverable states; nothing spins
without a bound; a failure names its true cause, with Try again; one core).
All additive (new events, view fields with serde defaults, enum variants)
except the Rust struct literals of `LinkedView` (two more fields) and the
behaviour of the RPC pool during an outage (below).

### 15.1 r13-4 (S3) — a network whose nodes hang is out of reach after three of them

**Found.** A warm session on Mainnet with every node of Soneium, Unichain
and Celo hung (taking the connection, saying nothing): nothing read them for
326 s (the account held nothing there; the scan plan does not include them;
the shell's token cache answered), then 187 s and ~100 hung requests across
~25 endpoints before "Can't reach 3 networks". The cause was the pool: a
network was `unreached` only after ONE call had swept its whole list (RF1),
and a call walks its endpoints one at a time, 8 s each (1.5 s hedge) — 25
endpoints, three passes, every call; a hang was never kept from the wire.
The balance machine then read again (`pool_unreached`) and waited its own
18 s deadline.

| Was | Now |
| --- | --- |
| `unreached_chains` after one call's first pass failed on transport at every endpoint | after `UNREACHED_AFTER_ENDPOINTS` = 3 DIFFERENT endpoints failed on transport (a hang, a refusal) with no answer of any kind from the chain between them — from any calls; a pool smaller than three needs all of them — AND at least `UNREACHED_MIN_SILENCE_MS` = 5 s since the first of them (refusals land in milliseconds: three refused public nodes before the one that answers are no outage — the e2e caught "Can't reach Ethereum" flickering for a frame while Ethereum answered; hung nodes take 8 s each, so the bound costs a hang nothing). An answer (a result, an error, a 429) empties the count. RF1 (a whole pass refused) stays, at once. |
| every read of an out-of-reach chain swept every endpoint, three passes | for a SILENT chain (the silence rule; RF1 alone keeps the whole sweep — a node that 500s one method still answers the next read), the circuit breaker's half-open probe: a read explores the chain in a single pass, over the nodes not yet silent in this outage — each hung node is asked once, never again until the chain answers (a node that answers, later in the list, is still found: a 401 primary and refused public nodes before a working one); once every node is silent it probes `OUTAGE_PROBE_ENDPOINTS` = 2 of them: the most trusted source first (the person's own node — a cooldown's score must not keep the node that comes back from being asked), then the oldest failure, so successive probes go round them all. No read is failed without asking anyone (an earlier draft failed every read but one at once while a probe was out — the e2e caught a read failed that a node not yet asked would have answered). The read that found the outage makes no second pass. A usable answer clears the chain as before. Writes (anything not a hedged read) are never refused this way. |
| `pool_unreached` read again, forced, and named the network when that read failed (+18 s) | NEW `RpcPoolView.silent_chains` (the out-of-reach networks found by the silence rule above) and `pool_unreached {chain_ids, silent}` (serde default `[]`): a SILENT network is named AT ONCE (it joins `failed_chain_ids`: no "Live" over it) — several of its nodes said nothing at all for seconds, nothing answered since — and the forced read owed confirms it or unsays it (`pool_said`; a cache's answer does neither). Any other pool loss is only read again, as before: RF1 can come from one method a node refuses (a stub's 500), and naming that at once flickered "Can't reach Ethereum" for a frame while Ethereum answered (caught by an e2e probe). The web passes `silent`; the natives send none yet (read again: about 18 s more). |

Timeline now: a hung read at T; three silent endpoints by ~T + 17.5 s (8 + 1.5
+ 8); named then. The web shell already hands the core what it needs
(`timeout` vs `network` / `not_connected` outcomes, `latency_ms`, `now_ms`);
its endpoint skipper stays as a transport detail. The 326 s before the first
hung read is not a read that hung: nothing asked those networks (the
account holds nothing there, and their state was the cache's) — out of this
fix's scope; the 10-minute poll and the cache rules of §13.2 bound it.

Tests: `app_rpc_pool.rs` (`a_hung_chain_with_many_endpoints_is_unreached_after_three_silent_endpoints`,
`an_answer_between_silent_endpoints_starts_the_count_again`,
`an_unreached_chain_is_probed_by_one_read_on_two_endpoints`,
`a_write_during_an_outage_still_goes_out`; RF1's test now also says the read
that found it ends there); vector `balance-pool-unreached.json` (+ "named at
once, on the pool's word … unsaid when that read answers"); web e2e
`home-unanswered.e2e.ts` r13-4 (every node of Ethereum hangs in a warm
session — named < 35 s after the first hung request, < 25 hung requests);
`a_node_not_silent_yet_is_still_asked_during_an_outage`.

### 15.2 r13-3 (S3) — the step before a review has the review's discipline

**Found.** Before a draft (send's "Propose and approve", a site's
`eth_sendTransaction`) or a link's transaction exists, the reads it waits
for had no bound — the 40 s / 48 s were the pool giving up — and a chain
read that failed was said as the service's ("Couldn't read this multisig's
waiting transactions"): `draft()` folded `Err(ChainUnavailable)` and
`mailbox == false` into `MailboxUnreadable`. iOS's cold link sat on "Opening
the transaction…" (`LinkedStand::Reading`, no bound, no Try again).

| Where | Now |
| --- | --- |
| a draft (`draft`, `dapp_request`) | the members' read (the network) and the waiting list (the service) read side by side, bounded by `OPEN_TIMEOUT_MS` = 25 s (`multisig_review_state`; below the review's 40 s, above one hung node plus a hedge). Failed or still out at the bound, the read is named: `ActionState::Unread {reason_key}` — `home.multisig.proposal.unread.chain` (the network; `{{network}}`), `.both` (network and service: most likely the connection); the service alone keeps `MailboxUnreadable` (`proposal.mailboxDown`). A delegatecall's code read that fails is the network's, never "has no code". Try again = propose again (send's CTA, the site's sheet's Retry). |
| send | `SendProposeFailure::ChainDown` → `SendTxErrorKey::ChainUnreachable` ("Couldn't reach {{network}}, so nothing was signed or sent.") |
| a link's transaction (`open_linked`) | bounded by `OPEN_TIMEOUT_MS`: still reading then → `LinkedStand::Failed` with `LinkedView.reason_key` (`proposal.unread.linkChain` / `.linkService` / `.linkBoth`: the read still out, from the machine's own count of reads out, `ReadOut`) and `retry`. `unreadable` and `notFound` carry their key and `retry` too. The proposals `retry` starts the link's bound again (a preview's transaction is read again). The reads go on: it opens when they come. |
| the accounts machine's link open | its multisig lookup and member read bounded by the same deadline (past it: `AddOutcome::Unchecked`; the member read falls back to the founding members, as a failed read always did). |

Link door: `review.opening {chain, service, link?}` → the key (or null).
Words: `home.multisig.proposal.unread.{chain, both, linkChain, linkService,
linkBoth}` (15 locales; pin 690). Tests: `tests/app_multisig_opening.rs`
(the network hung / refused, the service hung, both, Try again, a site's
request, a link hung → failed → Try again → found); web
`multisig/core/opening.test.ts`.

Shells: web — the send's line (`chain_down`), `ProposalSheet` (the site's
draft and a link's `failed` with Try again, `proposal-open-failed`),
settings' rename and member changes; iOS — `actionFailure` (`unread`),
`proposeFromSend` (`chain_down`), the link's `failed` with Try again, the
site's sheet now says a failed draft with Retry and Decline (it spun for
ever); desktop — `ms_action_words` (`Unread`), the link's `Failed`, send's
`ChainDown`; Android — the words (`unread`, the link's `failed`), minimally
(another lane is on its shells; its send wiring still maps only
`mailbox_down`, so an unread draft there says the generic line: open).

### 15.3 r13-7 (S3) — a new multisig's first history read is honest while it runs

**Found.** The history (what it executed, found by `ExecutionSuccess`) was
read in a phase no view showed: nothing said "being read", so the wallet tab
said "No activity yet" while the side panel's read failed and said
"Couldn't read this multisig's history". The read failed on its first
window a node did not answer (a freshly booted pool — the side panel's) and
on a node's "limited to a 10,000 range", which this module read as 10 blocks
(the smallest number in it) and ran out of windows. Desktop wrote the rows
on a thread and never told its feed: the first load missed the executed
send until the window was focused again.

| Was | Now |
| --- | --- |
| no state for the history | `ProposalsView.history` = `reading` (its first read on this page is out) → `read` / `unread`; once read it stays read; `retry` reads an unread one again. |
| one unanswered window: unreadable | asked again twice (2 s, 4 s) first |
| the module's own range parser | the pool's one reading (`rpc_pool::get_logs_range_cap`, ruling 8) |
| (web) "No activity yet" whatever the history | Activity is `loading` while the history is `reading`; `unread`: its empty state says the panel's `historyUnreadable` line, never "No activity yet" |
| (desktop) rows written in the background, feed not told | written off the main thread, then the page's feed reads its store again (`activity_feed::focus_tick`) |

Tests: `app_multisig_opening.rs` (`a_new_multisigs_history_is_reading_until_its_first_read_ends`,
`a_window_unanswered_once_is_asked_again_before_the_history_is_unread`,
`a_range_limit_written_with_a_comma_narrows_to_its_width`,
`an_unread_first_history_is_read_on_retry`); web `live-elsewhere.test.ts`.
Open: iOS and Android draw their Activity from the balance and scan alone
(no `history` read yet) — the field is there for them.

### 15.4 r13-1 (S2) — a home claims nothing of a space its account does not live in

**Found.** The natives each had their own copy of the rule (r10-4, r11-3:
Android `OtherSpace`, iOS `WalletLive.livesElsewhere`, desktop
`network_space::lives_elsewhere`); the web had none, so an Arc-only multisig
with Mainnet in front read "$0.00 · Live · listening for payments", "No
activity yet", "Deposit your first asset".

Now the core owns it: `network_space::lives_elsewhere(chain_ids, here,
other, front) -> Option<Elsewhere {chain_ids, space, line_key,
heading_key, switch_key}>` — none of its networks in the space in front
(`here`: the person's lists), its networks of the other space; wasm
`networkLivesElsewhere`. Vector `tests/vectors/multisig/other-space.json`
(Rust `other_space_vectors.rs`, web `opening.test.ts`). The web home (and
the extension's, the same components) draws the note "Used on Arc Testnet ·
Testnet space" with the switch (`set_network_space`) at the top of the
multisig's card, the other space's heading in the figure's place, and no
status line, no refresh, no empty state (`WalletLiveInputs.elsewhere`).
Open: the natives keep their shell copies of the rule (they agree with the
vector's cases); moving them onto a uniffi export is the next native round.

### 15.5 Smaller

- **Safari extension, no wallet on a connect.** The request tab sat on
  "Preparing wallet…" while it booted the wasm core, read the network lists
  and published the snapshot — reads that need a wallet — before it said
  there was none. Now, in the window, a signed-out session is said at once
  (Create / I have a wallet / Cancel, `request-no-wallet`) — even before the
  request detail is read — and the request is read when somebody signs in.
  Chrome: the same window code (its side panel already went to Welcome).
- **r13-6 parity (the invite's network note, from the coordinator).** The
  web's invite preview said `add.networkMissing` with the proposal sheet's
  whole "On Arc Testnet (chain 5042002) — a network this device doesn't
  have" as its `{{network}}` — a doubled word and clause. Now the web (and
  the extension: the same sheet) and the desktop hand it the network's NAME
  (the invite's `missing_name`, else the chain's name), as Android now does
  and iOS did: one sentence in the core's words.
- **D9 (dev harness).** A passkey's relying party is a domain, never an IP
  address: the local trusted page on `127.0.0.1` showed the browser's raw
  `SecurityError`, and its passkeys would not be `localhost`'s. One loopback
  origin — `localhost`, same port: the core's `signer_url`, the app's
  `loopbackPage` and the Safari `dev-seed.js` all canonicalise `127.x` and
  `[::1]` to it.

## 16. Page: multisig owner proof

Creating a multisig (or sharing its record again) asks one member's passkey
for the genesis service's sponsor challenge,
`keccak256("safe-genesis-registry/sponsor/v1" ‖ multisig)`: the proof the
service needs before it records the multisig where every member finds it. Until
now the trusted page had no card for it (`vela_multisigCreate`), so every
caller that routes it to the page refused it. That covered the Safari Web
Extension, whose pages cannot use a `getvela.app` passkey (R8), so the Safari
wallet could not create a multisig at all.

**Decision.** The owner proof is a page signature like any other, read the way
a Vela account's signature is read. The page is never handed a challenge. It is
handed the multisig's **genesis** and reads everything it shows from it:

- **The request** (`trusted_signer::owner_proof::params`). The core builds it
  into the passkey purpose: `PasskeyPurpose::Record { multisig, params }`, with
  `params = [{multisig, member, genesis, chainIds}]`. Every shell forwards it
  as-is to the core's generic request (`trusted_signer::request`, method
  `vela_multisigCreate`). No shell builds or reads it.
- **The reading** (`owner_proof::read`, mirrored by the page's
  `lib/multisig.js` `ownerProof`, in the same order).
  1. The shape is checked.
  2. The genesis must be exactly a multisig as Vela creates one
     (`multisig::vela_multisig_of`): Safe's v1.4.1 factory, the L2 singleton,
     and a setup written again from the members and threshold it names, byte
     for byte. That rules out a hook, a module, a payment, and members a
     multisig cannot have.
  3. The address is derived by CREATE2. The address the app names is only
     compared with it (hard-danger refusal on a mismatch).
  4. The member must be one of the members, and must be the signing account.
  5. The challenge is derived from the derived address.
- **What the card shows.** The multisig (6+6 EIP-55 with its identicon; the
  name tagged as the app's), its networks (the page's own chain names, tagged
  "per the app": the challenge covers only the address), every member in setup
  order with this signer tagged "this account", "Needs N of M", and the
  signing account. The salt, setup, factory, singleton and challenge are in the
  details. The button is "Prove membership". The copy is in the page's own
  fifteen-locale catalogue, zh included.
- **Acceptance.** `trusted_signer::digest_of` handles the method through the
  same `read`, so `accept_signature` accepts only a user-verified answer, by the
  member's key, over the sponsor challenge for the address the genesis makes.
  That value must also be the challenge the machine is about to give the
  service.

**Why the page does not load the wasm.** The page is a zero-dependency,
content-addressed single file. Its `default-src 'none'` CSP is inside the
hashed bytes. It mirrors the core's rules in plain JS (as it already does for
`safeTxHash`, `review` and the executor's deploys), and those rules are
cross-checked case by case against the shipped wasm and the shared vectors
(`tests/vectors/multisig/owner-proof.json`: 66 cases, including r4-e's six
hostile geneses). The core is still the one source: the vectors are generated
from it, and `owner_proof.rs` asserts them.

**Every caller, the same way.** The core's route
(`signing_route_on(plan, KeyCeremony, surface)`) decides where the proof goes.
A `getvela.app` account proves in Vela on a standard surface; a custom-domain
account proves on its own page; on the Safari extension every proof goes to a
page.

| Caller | Before | Now |
| --- | --- | --- |
| Web wallet, Chrome extension, Safari package (`sign-challenge.ts`) | refused (`unsupported`) | `params` to the page through the shared handoff |
| iOS (`MultisigLive.passkey` → `UserOpSpine.signOwnerProof`) | refused (`not_opened`) | the spine's one `ceremony` (in Vela, or the page through `trustedSignerSignatureAccept`) |
| Android (`MultisigLive.passkey` → `UserOpSpine.signOwnerProof`) | refused (`not_opened`) | the same |
| Desktop (`multisig/live.rs` `prove_on_page`) | signed in Vela even when the keys live on a page | routed: blocked / its page (`trusted_signer::sign`) / in Vela |

Page **64363925** is first in `BUILD_ALLOWED`; `LAUNCH` is unchanged. It reaches
users only once `dist/` is deployed.

Tests:

- Core: `owner_proof.rs` (the vectors, `accept_signature` held to the challenge
  the genesis makes) and `tests/app_multisig_accounts.rs` (creating and sharing
  again carry params the page reads as this multisig and this challenge).
- Page: `multisig-test.mjs` §3 (vectors against the page and the wasm, the
  pipeline, the hostile cases, the answer rule) and `answer-test.mjs` G and S
  (the published bytes: the web wallet, the Safari relay, evil.example, a lying
  address).
- Web: `sign-challenge.test.ts` and `handler.test.ts`.

## 17. Round 13 native adoption

Round 13 (§15) moved five rules into the core and adopted them on the web.
This round adopts them on Android, iOS and the desktop. No shell keeps a copy
of a rule. Every shell draws the core's states and offers the same ways out.
Decisions follow best practice and the owner's rules: one core, honest
states, every failure recoverable.

| Item | Decision | Android | iOS | Desktop |
| --- | --- | --- | --- | --- |
| **r13-1, other space** | Export `network_lives_elsewhere(args_json) -> String` through uniffi (`shell_rules_bridge.rs`). It takes and returns the same JSON as the wasm `networkLivesElsewhere`, so the vector replays byte for byte. Each shell draws the note's line, heading, switch and switch target from the answer's keys and `space`. | `OtherSpace.livesElsewhere(…, front)` returns `Elsewhere?` from the core. The filtering copy is deleted. | `WalletLive.livesElsewhere(…, front)` returns `Elsewhere?` from the core. The copy is deleted. The card takes `Elsewhere`. | `network_space::lives_elsewhere` calls the core's function directly, over the person's lists of both spaces. The copy is deleted. |
| **r13-4, silent networks** | Send the pool view's `silent_chains` beside `out_of_reach_chains` as `pool_unreached {chain_ids, silent}`, on a change to either set (the web's `onPoolOutOfReach`). The natives already sent what the silence rule reads: `timeout` / `network` outcomes and `now_ms`. | `RpcPoolView.silent_chains` is decoded (nothing filled it before). `WalletController.poolUnreachedOf(view)` builds the event. | `RpcPoolViewWire.silentChains`. `onOutOfReach(chainIds, silent)` fires on either change. `WalletStore.poolUnreachedEvent`. | `pool::PoolLoss {chain_ids, silent}` is kept by the pool thread and dispatched by the 1 s look. |
| **r13-7, history** | Follow the web exactly. The state applies only while the proposals machine follows the shown multisig, and only over no rows. `reading` shows Activity as loading. `unread` shows `panel.historyUnreadable` (naming the first unread network) with **Try again** (`common.tryAgain`), which sends the proposals machine's `retry`. `read` leaves Activity as it is. Unlike the web, the natives put Try again on the Activity empty state itself rather than only in the card's note. That is the honest place, because it is where "No activity yet" used to be. A view from an older core reads as `read`. | `WalletLive.withMultisigHistory`, `EmptyStateModel.retry`, `home.activity.retry` | `WalletLive.withMultisigHistory`, `SectionEmptyModel.retry`, `home.activity.retry` | `with_multisig_history`, `HomeActivity::Unread`, `home-activity-retry` |
| **r13-3, Try again** | A link's `failed` stand and a site's draft whose reads failed get Try again on every shell, matching iOS and the web. The send's `chain_down` already had it on every shell (`retry_after_error`). | Link `failed`: the note plus `multisig.proposal.openRetry` (`retry`), now that `MsLinkedView.retry` is read. The site's sheet no longer spins for ever: it shows the failure with Retry (`MultisigHub.dappRetry` re-sends the kept `dapp_request`) and Decline. | already done | Link `Failed`: `ms-proposal-open-retry` (`link_offers_try_again`). The site's sheet and send already had it. |
| **Device-neutral notice** | New `onboarding.storage.deviceUnreadable{Title,Body}` in all 15 locales (pin 2287). The body names no browser copy to reset. | does not show the notice (an unreadable list lands on onboarding) | does not show the notice | DESKTOP-1's welcome notice and switcher line use the new pair (`UNREADABLE_TITLE` / `UNREADABLE_BODY`). |

Tests:

- **Core:** the uniffi crate's `the_other_space_vectors_hold_through_the_export`.
- **Android (JVM):**
  - `OtherSpaceVectorsTest` (the vector)
  - `RpcPoolMachineTest` (three hung nodes become silent and are handed on as silent; one refused node is not)
  - `MultisigHistoryHomeTest`
  - `MultisigViewsTest` (link `retry`, `unread` words)
  - `MultisigShellTest` (a site's failed draft is asked again)
- **iOS:**
  - `OtherSpaceVectorsTests`
  - `R13NativeAdoptionTests` (the pool view, the pool's hand-off, `balance-pool-unreached.json` through the shell's event, the history states)
- **Desktop:**
  - `the_pool_unreached_vectors_hold_through_the_pools_loss`
  - `the_pools_loss_carries_its_silent_networks`
  - `a_multisigs_history_decides_its_empty_activity`
  - `link_offers_try_again` (in `a_sheet_with_no_transaction_says_what_is_true`)
  - `the_unreadable_notice_names_no_browser`
  - the other-space test, now holding the core's keys

## 18. Follow-ups after Round 13

Four loose ends from §17. Each was decided by best practice and the owner's
rules (one core, full parity, honest and recoverable states) and adopted on
every shell.

| Item | Decision | Why |
| --- | --- | --- |
| **1. Link Try again** | The core's flag stands. `LinkedView.retry` is true for `failed`, `unreadable` and `notFound`, and every shell offers Try again (the proposals machine's `retry`) for every stand the core marks `retry`. Each shell says it in the core's words for the read (`reason_key`: `proposal.unread.linkChain` / `linkService`). | `notFound` is not a definitive answer: it is the multisig service's silence (`ChainState.mailbox` false, the mailbox not read). The definitive answer is `notInMailbox` (read, and not there), which carries no retry and is read again on every tick. So `notFound` is retryable. The old `proposal.notFound` line ("hasn't been shared with the members yet") was untrue for a read that failed, and no shell says it for a link any more. |
| **2. Unreadable account list** | One rule in the core, `app::stored_accounts` (UniFFI `stored_accounts_read/upsert/remove`, wasm `storedAccountsRead/Upsert/Remove`). The read returns the records the core reads, as stored, and counts the rest (positions and sizes, never contents). A value that is not a list counts as one unreadable record. Writes keep every unreadable record where it was, byte for byte. A value that is not JSON at all is kept as one record holding its text as a JSON string, so every byte can be recovered. An upsert replaces by `id`, as the desktop does. Android, iOS and the web read and write only through it. Each says it: the welcome shows `onboarding.storage.deviceUnreadable{Title,Body}` (the web keeps its browser-worded prompt), and the switcher shows a line under the list. The parallel-space seed refuses. Signing in with the passkey is the way back: it writes the account beside what is kept. | The desktop's DESKTOP-1 behaviour, without a copy per shell. Android threw on a list it could not read, so one bad record hid all the others and "Sign in again" failed. iOS read it as empty and the next save wrote `[account]` over it. The web dropped such records on every save and in its respelling rewrite (removed: the core reads both spellings). The scan avoids serde_json's `raw_value` feature, which grew the wasm by 138 KB. |
| **3. History Try again** | Try again for an unread multisig history lives on Activity's empty state on every shell. The web now draws it there too (`EmptyState` `action`, the page's `MULTISIG_RETRY`). One sentence and one control per failure: while Activity says the history could not be read, the waiting group or card does not say it again. Over rows, Activity does not say it, so the group's note and its Retry still do. Any other note is unaffected. | The natives drew the line and Try again on the empty state **and** the card's note with Retry, so the failure was said twice with two controls. The web hid its empty state behind the waiting group, whose note was the only control. Activity is where "No activity yet" would have stood, so the failure belongs there. |
| **4. WEB-CLEAR-LOCALE-1** | Already fixed by spec 097 E (`14429c8ac`). The web and the extension (one sheet) hand the core `toClearLocale(resolvedFormatKeys())`, and the device's offset is sampled per request. iOS (`SigningController.defaultLocale`) and the desktop (`signing_host::clear_locale`) already used the person's presets and the device's offset. KNOWN-BUGS is marked fixed. | Nothing left to change. `sheet-locale.test.ts` covers it. |

| Item | Web | Android | iOS | Desktop |
| --- | --- | --- | --- | --- |
| 1 | `ProposalSheet` `linkFailure` follows `linked.retry` | `LinkedStands.offersTryAgain` / `failure` | `MsLinkedView.offersTryAgain` / `failure(_:)` | `link_offers_try_again`, `no_item_lines` |
| 2 | `storage.ts` `readStoredAccounts`, `saveAccount`, `removeAccount`, `storedUnreadable` → `AccountsSheetBody` line | `AccountStore` (core), `AccountStore.unreadable`, `WelcomeScreen` notice, `AccountsSheetModel.unreadable` | `AccountStore` (core), `StoredAccountsWatch`, `WelcomeScreen` notice, `AccountsSheetModel.unreadable` | unchanged (DESKTOP-1, the same rule over JSON values) |
| 3 | `activitySaysHistory`, `waitingRows.group(…, historySaid)`, `EmptyState` action | `WalletLive.historySaidInActivity`, `cardNote` | `WalletLive.historySaidInActivity`, `MultisigHomeCard.cardNote` | `history_said_in_activity`, `card_note` |

Tests:

- **Core:**
  - `a_link_whose_service_or_network_did_not_answer_offers_try_again`
  - `a_link_not_in_the_mailbox_offers_no_try_again`
  - `stored_accounts` unit tests
  - the UniFFI `the_stored_accounts_exports_keep_what_they_cannot_read`
- **Web:**
  - vitest "a link whose read did not come back"
  - `storage.test.ts` "a stored record this browser cannot read"
  - the switcher line in `fiat-withheld.test.ts`
  - `live-elsewhere.test.ts` and `activity.test.ts` (item 3)
  - e2e `login-old-shape` (item 2) and `multisig-world` "an unread history"
- **Android:**
  - `MultisigViewsTest.every_link_the_core_says_retryable_offers_try_again`
  - `StoredAccountsUnreadableTest`
  - `MultisigHistoryHomeTest.an_unread_history_is_said_once_on_activitys_empty_state`
- **iOS:** `R13FollowupsTests` (items 1–3)
- **Desktop:**
  - `a_sheet_with_no_transaction_says_what_is_true`
  - `a_multisigs_history_decides_its_empty_activity`
  - `an_unread_history_is_said_once`

Left open:

- The desktop keeps its DESKTOP-1 copy of the item 2 rule over the JSON values in `wallet.json`. It is the same rule, but it does not yet call `stored_accounts`.
- A sheet opened without a link whose multisig's mailbox was not read still says `proposal.notFound` on the web and the desktop. Only links were in scope here.

## 19. Round 14 fixes (core, web, Safari, desktop, iOS)

Decided under the owner's rules (one core, full parity, honest and
recoverable states, Vela-account-grade clear signing, no blind signing).
Each item was written test first.

### 19.1 r14-13 + parity — a review its network answered nothing of is failed, never "… anyway"

**Found.** With every Arc node hung, the web's review ended at 26 s in the
D1 state (`ready`, `unchecked`, "… anyway") while Android went Reading →
"Couldn't read it in time" (~50 s) → Try again. Two causes: the core had no
rule telling "the transaction was not read against its chain at all" from
"only the two safety checks are unread", and the core waited for the
shell's simulation and recipient facts without a bound of its own (the web's
pool ended them at 8 s; Android's simulation held the whole review to the
40 s `REVIEW_TIMEOUT_MS`).

| Was | Now |
| --- | --- |
| `gather_review` asked the code rounds, trace, probe, simulation, facts and tokens; nothing said whether the network answered any of it | it also reads the multisig's own code (`eth_getCode(safe)`, beside the checks: no time added) and hands back every read's envelope (`Gathered {input, answers}`: the anchor, the code rounds, the trace, the probe) |
| `ReviewState::of(review)`: any review gathered is `ready` | `ReviewState::settled(review, answers)`: `ready` only when the network answered at least one read (`{result}` or `{error}` — a node saying "method not found" answered); none answered → `failed {home.multisig.review.state.chainUnread}` ("Couldn't reach {{network}} to read this transaction.") — never signable, not even "… anyway", Try again (`review_again`). Only when the chain answered and the trace / probe / simulation alone are unread is the review `unchecked` (D1, unchanged). |
| the shell's `Simulate` and `RecipientRisk` (and a read whose `bound_ms` a shell ignores) were waited for until the review's 40 s bound | every question of a review is ended by the core at `REVIEW_ASK_MS` = `REVIEW_READ_MS` + 2 s = 10 s (`multisig_gather::within`): the same answer at the same time on every shell — a full hang lands `failed {chainUnread}` in ~10–18 s everywhere |
| the outside page drew whatever was gathered | `review_now` returns `(review, state)` through the same `settled`; a review that is not `ready` is not drawn |

Why the anchor read: a change of threshold names no recipient and no token,
so with the checks refused the only reads were the checks themselves — the
rule could not tell "Arc is down" from "Arc refuses `debug_traceCall`". The
multisig's own code is the network state the review is read against, and it
is cheap. Shell facts (recipient facts, the simulation) are not counted as
the chain's answer: a shell may answer them from a cache.

Words: `home.multisig.review.state.chainUnread` (15 locales; on-demand pin
691). Every shell fills `{{network}}` (web `reasonWords(m, key, network)`,
`ReviewStateNote network`; desktop `sheet::failed_reason_words`; iOS
`MsReviewPending.network`; Android `MsReviewDoor(network =)`, minimally — the
Android lane adopts the rest). Vectors: `review-state.json` `settle` (six
cases: hung or refused end to end, the anchor answered with both checks
unread, "method not found", every read answered, no review, nothing asked),
replayed through the link door `review.settle {review?, answers}` by Rust and
the web; `keys` + `chainUnread`. Machine tests (`app_multisig_review_state.rs`):
`a_review_its_network_answered_nothing_of_fails_never_anyway`,
`a_hung_network_fails_the_review_by_the_cores_own_bound_on_every_question`,
`only_the_two_checks_unread_stays_unchecked_anyway`,
`a_simulation_a_shell_never_ends_is_ended_by_the_core`.

### 19.2 r14-4 — a setup that could not be checked is never said as a finding

**Found.** With Arc refused, the desktop said "Vela won't use this multisig
Its setup could give control of its funds to someone other than its members…"
of a multisig Vela created, under an enabled "Approve anyway". The deploy
gate's read failed (`Authority::Unknown`), and the execution look handed every
non-verified authority on as `refused`; the desktop drew every `refused` as
the refusal title and body (a period missing between them). iOS and Android
drew every `refused` as "couldn't check" — honest for this case, untrue for
a setup read and refused.

Now the core tells them apart: `Preview::Unverified` (`{"kind":
"unverified", "authority": {"kind": "unknown", …}}`) for a setup whose Safe
contracts went unread — looked at again at the unpriced pace, Execute waits,
"Check again" — and `Preview::Refused` only for a setup read and refused. The
execute gate stops with the same distinction. Every shell: `unverified` →
`authority.deployUnverified` (caution); `refused` → `authority.deployRefused`
and each reason once (danger): web `previewOf`, desktop
`findings::deploy_words` (fee line and the stopped line), iOS
`MsWords.deployRefused`, Android `MultisigWords.deployRefused` (minimal
adoption of the new kind). Approving stays possible: it deploys nothing.
Tests: `a_deploy_whose_safe_contracts_went_unread_is_unverified_never_refused`
(core), `an_unread_setup_is_never_said_as_a_finding` (desktop), the web's
`execution-words.test.ts`.

### 19.3 r14-7 — a read that failed is as unread as one still out

**Found (core).** A site's draft classifies what it could not read at
`OPEN_TIMEOUT_MS` by what is still OUT. The network's read had already come
back failed (its nodes found silent) while the service still hung, so the
draft said only the service's line. The flags now mean "unread": still out
or come back failed. Test:
`a_site_draft_whose_network_failed_first_and_service_hangs_names_both` (both
orders). No shell change: every shell already says `unread.both`.

### 19.4 §18 leftovers (b) and (c)

- **(b) A sheet opened without a link** whose transaction is not in the list
  now says what the core decides from the machine's facts
  (`multisig_review_state::missing_said(loading, unreadable, mailbox)`, link
  door `review.missing`): the service not read → `proposal.unread.linkService`
  with Try again (the proposals machine's `retry`); the network unread →
  `proposal.cantCheck` with Try again; read and absent → `notInMailbox`;
  still loading → `opening`; another network → `notHere`. Never
  `proposal.notFound` ("hasn't been shared yet") for a read that did not come
  back. This device being offline stays the web's to say first (a device fact,
  not a rule). Web `missingSaid` / `missingWords` in `ProposalSheet`, desktop
  `no_item_lines` + `missing_offers_try_again`, iOS `MsRules.missingSaid` in
  the sheet's no-item state (it spun on "Opening…" before). Vectors:
  `review-state.json` `missing` (Rust, web, iOS test).
- **(c) Failing invite links.** `LinkStand::Failed` carries `retry`, decided
  by the add sheet's own per-cause rule (`AddOutcome::offers_retry`: busy,
  service unavailable, a network not answering, the Safe contracts unread →
  retry; not found, not a multisig, refused, high risk → no retry — an answer
  that came back is not asked again behind the person's words). New event
  `retry_link` opens the same link again. Web: the add sheet opened from the
  link says Try again (`prefillRetry`); desktop: the add form's Try again
  (`offers_retry`); iOS and Android: Try again on the link's failed sheet.
  Test: `a_link_whose_lookup_did_not_come_back_is_retryable_and_opens_on_try_again`.

### 19.5 r14-1 (S2) — the Safari wallet tab, at phone width

- **Root cause of the "429 pt page in a 402 pt viewport".** Not a layout
  overflow: 429/402 = 934/874 = 16/15, the zoom iOS applies when a field
  with 15 px text takes focus, and keeps after blur. Plain web (WebKit,
  Chromium, iOS Safari) never overflowed at 320–430. **Decision:** the
  pre-paint script in `app.html` adds `maximum-scale=1` on iOS / iPadOS only
  (iPadOS: MacIntel with touch points) rather than a 16 px floor on every
  input — a global floor fights per-component sizes (the amount field), and
  iOS keeps pinch-zoom despite `maximum-scale` while Android would lose it.
  The extension pages share `app.html`. Tests: `viewport-zoom.test.ts`,
  e2e `narrow-layout.e2e.ts` (home with a Vela account and with a long-named
  multisig, the switcher, Settings and Switch account at 320/375/402/430 —
  no horizontal overflow; the iPhone cap; no cap elsewhere).
- **The header switcher is a real button.** WebKit turns a
  `<button aria-haspopup>` into a pop-up button, which iOS exposes without
  the button trait (the tester's "group"). **Decision:** a plain `<button>`
  named by the account, the name and chevron inside it; neither
  `aria-haspopup` nor `aria-expanded` (the switcher is a modal that makes
  the trigger inert; the WAI-ARIA dialog pattern needs neither).
  Test: `WalletHeader.svelte.test.ts` (`getByRole('button')`, both the name
  and the chevron open it).
- **Create / Add multisig from Switch account.** **Decision:** the phone
  Settings sheet's Switch account reuses the wide settings' Create multisig
  and Add multisig buttons (r4D-19), each closing the sheet first — the same
  flows, one sheet at a time. iOS Settings → Switch account now carries
  `MultisigSwitcherSection` too. Every surface: web / Chrome / Safari —
  header and Switch account; iOS — header and Settings; desktop — header,
  settings and the browser's switcher; Android — header and the browser's
  switcher, Settings → Switch account left to the Android lane (recipe:
  thread a `multisigSection` slot through `SettingsActions` →
  `SettingsScreen` → `AccountsSheetBody`).

### 19.6 Display

- **r14-11a — a short form never breaks.** Unicode line breaking allows a
  break after "…" (U+2026), and a CJK line takes it. **Decision:** shell
  no-wrap runs around each short form (web `address-runs.ts` /
  `AddressText.svelte` in activity rows, member changes, settings, create,
  the review; iOS `lineLimit(1)` on account rows; desktop
  `whitespace_nowrap` on the switcher and create rows), not word joiners in
  the core's `short_address` — that would change every golden test, text
  match and copied string. Test: `ActivityRow.svelte.test.ts` lays zh and en
  rows at 220–360 px.
- **r14-11b — the desktop header's name.** The network-space pill sat on the
  name's line and squeezed it. As on the web, the name line holds the name
  and chevron only; the pill follows the address. The two-line clamp stays
  (parity with the web's `WalletHeader`, 078 H-11: a name's end tells two
  accounts apart).
- **r14-12 — iOS "Create New Account"** is the neutral ink (`.ink`), not the
  accent (`MultisigR10FixTests.theAccountsSheetsCreateIsNeutralInk`).

### 19.7 r14-5 — a rate-limited add is said, with Try again

- **Root cause (desktop):** the sheet's own note of the add it sent was lost
  when the sheet was opened again while the core was still adding (the core
  reopens an invite's sheet when its network arrives; the page rebuilds) —
  the outcome landed on nothing. The sheet now waits for any add the core
  runs for the address in its field (`add_await`).
- **Core:** the accounts machine records networks that answered only "slow
  down"; it pauses once (`RATE_LIMIT_PAUSE_MS` = 3 s), asks again, then
  answers `AddOutcome::Busy {chain_id}` (`add.busy`, the network named).
  `AddView.retry` (`AddOutcome::offers_retry`: busy, unavailable, unchecked,
  unverified) → Add reads Try again on every shell.
- **Decisions:** the rate limit reaches the core as a detail marker
  (`RATE_LIMITED_DETAIL`, "rate-limited"; the core also reads "rate limit",
  "too many requests", "http 429"), not a new field (≈20 pattern sites);
  `Retry-After` is not honoured in the pool this round (its 429 outcome does
  not carry it; the pool already backs off over three passes and fails over
  between endpoints) — the add makes one bounded pause and then names it.

### 19.8 §18 (a) — the desktop's stored accounts

The desktop reads, upserts and removes its account list through the core's
`app::stored_accounts` over the stored value's text; its DESKTOP-1 copy is
deleted. Unreadable records are still counted, logged by position and size,
and kept through every write; the welcome notice and switcher line are
unchanged. Test: `the_account_list_is_read_and_written_by_the_cores_rule`.

### 19.9 r14-8 — the max-send hint is the confirm's own fee

**Root cause.** Two rules: the hint kept back the fee once, Max kept back
1.5× it (on the tester's 0.1196 USDC Max filled 0, so the hint's 0.008792
was typed by hand); Continue then re-quoted at 0.0984 and nothing kept the
typed figure current, stranding 0.012408.

- **One ceiling, the fee once** (`app/send.rs` `max_fee_reserve`): the
  hint's `max_transfer_amount`, the Max fill and the confirm's gate are one
  figure. The 1.5× was a drift allowance left from the TypeScript send code;
  the signature charges exactly the quoted fee (r11-8).
- **The hint and Max read the fee on screen** (`shown_fee`), the quote the
  confirm draws and signs, never another estimate.
- **A Max follows every quote on the confirm until the slide**
  (`hold_max_to_quote`: Continue's re-quote, the fee card's re-quotes, a
  signature called off), said by the existing `max_moved`; frozen from the
  slide (the review digest stops a moved figure being signed unseen). A
  hand-typed figure stays the person's; it is re-checked against the
  confirm's quote, and the hint states the new ceiling if it no longer fits.
- The natives already draw the core's `same_asset_fee_issue` and
  `max_moved` and fill Max through `TapMax`: no shell does its own max
  maths, so the rebuilt core fixes them too.

Tests: `app_send_fee_cover.rs`
(`the_hint_is_the_ceiling_max_fills_against_the_same_quote`,
`a_max_follows_the_confirms_quote_until_it_is_signed`, vector section
`ceiling` in `send-fee-cover.json` with the tester's figures), web
`live-send-ceiling.test.ts`. Open: a typed amount that still fits stays as
typed when the fee drops (Max is the way to send everything); the
multi-token sweep's 2× reserve is a separate rule, unchanged.

### 19.10 §16 end to end on Safari (r14-1's proof)

Proven on an iOS 26.2 simulator (iPhone 17 Pro, its own Face ID) against
local services only: the fork harness (Gnosis fork, the genesis service
from `sgr-mailbox`, the local relay) and the local signing page (dist,
64363925, front of `BUILD_ALLOWED`). `SafariRouteDeviceTests`
`test07_createMultisigFromTheHeader` (header switcher → Add account →
Create multisig) and `test08_createMultisigFromSettings` (Settings → Switch
account → Create multisig) each create a 1-of-1 multisig from the Safari
extension's wallet tab: the hand-off card, the page's "Prove multisig
membership" card (name, address, "this account", "Needs 1 of 1"), the
simulator passkey, "Multisig created" / "Shared", and the multisig in front
on the home and listed in the switcher; the genesis records reached the
local service and its registry on the fork. A clean run (erased simulator,
tests 01, 02, 07, 08) passed. Runner: `app-ios/scripts/safari-route-e2e.sh`
(loopback only; links are never opened with `simctl openurl` on an https
URL). Not covered: zh, the deployed page, a 1-of-2, anything on chain beyond
the counterfactual record. Found (minor, open): on Safari iOS 26 the tall
Create sheet opens with "Next" under Safari's floating bottom bar until the
sheet is scrolled once.

### 19.11 The Android lane's three hand-offs (§20 "left open")

- **The newest link open supersedes one in flight (core).** Link opens run
  side by side in the accounts machine, so an older open whose lookup was
  slower could land after a newer one, take its stand and switch the
  multisig shown behind its sheet. **Decision:** every open takes a number
  (`LinkState.seq`, bumped by `open`, `open_proposal`, Try again and a
  network added); after each wait an open that is no longer the newest stops
  — it says nothing, keeps nothing, switches nothing, opens nothing. Test:
  `the_newest_link_supersedes_an_open_still_in_flight`. Android's sheet guard
  stays as a shell detail; the account behind the sheet is now the core's
  on every shell.
- **The fee signed is the fee shown (core).** Execute re-quoted a look older
  than `PREVIEW_FRESH_MS` at the press and signed that quote (r14-9's 0.0972
  under "≈0.098172"). **Decision:** a new quote whose fee differs from the
  look on screen signs nothing: `ActionState::Failed {fee_moved_detail}`
  (`{kind: "fee_moved", amount, chainId, executor}` — the shape Android's
  `ShownFee` already wrote, so its words read the core's answer unchanged),
  the new look is kept, and the next press inside the window signs exactly
  it. A stop rather than a confirm step: the sheet already draws the new
  look, and one more press on what is shown is the honest confirm. Every
  shell says "That didn't go through: Network fee ≈ {new}, paid by {name}"
  with existing words (web `ExecutionFeeMovedError`, desktop
  `ShellFailure::FeeMoved`, iOS `failureText`, Android as before); the
  trusted page executes through the same core path. Test:
  `an_execution_never_signs_a_fee_other_than_the_one_shown`. Android's shell
  guard may now be retired by its lane; it agrees with the core.
- **iOS r14-10.** `CameraScanner.Refusal.actionLabelKey`: a camera refused
  for good says `onboarding.create.openSettings` on the button that opens
  Settings (the scan sheet and Explore's scanner), as Android does.

### 19.12 Left open

- Android Settings → Switch account still offers only Create / Sign in
  (recipe in 19.5); Android's add sheet relabels Add as Try again only once
  its lane reads `view.add.retry`.
- iOS composite subtitles that hold an address with other text
  (`MultisigSettingsSheet`, `MultisigAddSheet`, the home card's proposal
  title) and desktop `multisig_add.rs`' row: short forms there can still be
  wrapped after their "…". Android rows: reported to its lane.
- `ImportOutcome` has no busy mapping yet (a rate-limited import says
  "unavailable").
- The Safari Create sheet's "Next" under the floating bar (19.10).
- Desktop's fee-moved line names the executor by its short address (the
  failure words have no view of the member labels there).

## 20. Round 14 Android fixes

Round 14's Android findings (r14-2, r14-3, r14-6, r14-9, r14-10), fixed in
the Android shell. Each was decided by best practice and the owner's rules:
the core owns the rules and the shell draws what it says; states are honest
and recoverable; parity with the web and iOS. No core change was needed.
§12's review state, §14.1's "… anyway" and §15's bounds are the core's and
are followed as they are; the full-hang outcome (failed or unchecked) is the
core lane's, and the shell draws whichever the core says.

| Item | Cause | Decision |
| --- | --- | --- |
| **r14-2 (a) A link over an open sheet** | The layer opened the link, then set the Link sheet: for a multisig kept here the core's `open_proposal` could land first and be covered. Every sheet's close was "close whatever is open", so a dismissal landing after a newer sheet took its place (Back or a swipe just before a link, on a loaded device) closed the NEW sheet and sent `close_linked` for its link. An older link whose reads were slower could also open its transaction after a newer one. | The newest link wins. `MultisigHub.takeLink` records the transaction asked for (`latestAsked`), puts the Link sheet up, and only then opens the link; `showProposal` does the same for a tapped notification. The router drops an `open_proposal` that is not the newest ask. Every sheet closes only itself (`closeOf(s)`: `compareAndSet(s, null)`; the proposal, link and invite sheets send `close_linked` / `close_link` / `drop_link` only while they are still the sheet open). |
| **r14-2 (b) A cold start on link #3 restored #2** | Android hands a recreated activity its STARTING intent again: a configuration change, a killed process whose task comes back (the new link then arrives through `onNewIntent`), or a launch from Recents. `onCreate` acted on it every time, so the task's old link or notification re-opened #2 over #3. | `LaunchIntent.firstDelivery`: the starting intent's link, notification, shortcut, shared text and receipt are acted on only on a first creation (no saved state, not `FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY`). A review never resurrects as "Checked" (r14-6 below); a fresh process starts at Reading because the core's review state is not stored. |
| **r14-2 (c) A link during an outage** | The tested build predates §18 item 1: its `unreadable` stand drew `proposal.cantCheck` with no Try again. Nothing asked again when the network returned. | Try again is §18's (every stand the core marks `retry`). It also recovers by itself: when the pool lets a network go (it leaves `out_of_reach_chains` ∪ `silent_chains`) or the device comes back online (`MultisigReach.back`), `MultisigHub.networkBack` refreshes the shown multisig, sends the link's `retry` when its stand offers one on that network, and bumps `reachedAgain`, which re-asks an open sheet's looks. This is the person's own Try again, sent for them when the cause is gone. A failed review (§12) is still never retried behind their back. |
| **r14-3 "Offline" flicker** | The shell mapping, not a connectivity signal. The executor fed `NetHealth` the delivery verdict: a connect that timed out is `not_connected` for delivery, so each hung Arc node (the proxy's tunnel never opened) and a slow Optimism Sepolia node counted as misses from two sources. With nothing else read for 10 s, that made the device "Offline". The next answer from any chain put it back, so it alternated with "Can't reach Arc Testnet" every 8–16 s. | `NetHealthFeed.of(outcome, failure)`: the transport now reports its failure class (`RpcPostResult.failure`). A timeout of the connect, of the answer, or of the call says nothing either way, which is the core's `net_health` rule ("that one request timing out is not fed"). A refused or unroutable connect, a failed name lookup, a broken handshake or socket still count. So "Offline" means the device has no network; a hung network is that network's notice ("Can't reach Arc Testnet", stable), as on iOS and the web. Found on the way: only an answered call ends "Offline", and nothing read for minutes once the device was back, so the home now reads again every `NetHealth.OFFLINE_PROBE_MS` (10 s) while it says "Offline". |
| **r14-6 (a) A stale review on reopen** | `review` on a review already `ready` is the core's quiet re-read (§12.1): the old review stays drawn, with its buttons enabled, until the read lands. A sheet opened a minute later drew the old "Checked" for as long as the network hung. | `MultisigHub.reviewOnOpen`: when a sheet opens on a review that landed longer ago than the core's `REVIEW_FRESH_MS` (30 s; the hub notes when each review landed), it sends `review_again`. The review starts at Reading, every sign button is shut, and it waits for the core's `ready` or `failed`. A review that just landed (just proposed from its draft, just read) stands. |
| **r14-6 (b) The fee never re-quoted** | The r13 S4 timer asked an unpriced or unchecked look at most four times (16 s), then never again; "Check again" was drawn only under Execute, not beside the one-press "Approve and execute". | An unpriced or unchecked look is asked again every `LookAsk.SLOW_EVERY_MS` (20 s) after the first four, and at once on `reachedAgain`. A priced look still stands (r13 S4 holds). "Check again" (`proposal.feeRetry`) sits beside every fee line that could not be worked out or checked (`LookAsk.offersCheckAgain`), as on the web. |
| **r14-9 Fee mismatch** | A re-quote at confirm. The fee line is the machine's look, and a priced look stands (r13 S4). When the person pressed after its 15 s (`PREVIEW_FRESH_MS`), Execute took a new look before the passkey and signed that quote (0.0972), not the one on screen (≈0.098172). | The fee signed is the fee shown. At the press the sheet records the figure on screen (`ShownFee.pressed`). Before anything is signed, the executor compares it with the operation's `send_calls.fee` (`ShownFee.refusal`). If they are equal, it goes on. If they differ, nothing is signed: the machine hears `CallsFailed {fee_moved, maybe_sent: false}`, the sheet says "That didn't go through: Network fee ≈ {new}, paid by {name}" in existing words, and the fee line shows the new look. The next press, inside the window, signs exactly it. This is a shell guard at the one place a fee becomes a signature; it needs no new words and no core change. |
| **r14-10 Camera label** | The label was fixed while the action changed. | `CameraRefusal.label`: once the button opens this app's Settings page (a refusal for good), it says `onboarding.create.openSettings` ("Open Settings"). The scanner has no key of its own. The label is read again after each answer and each return to the front. iOS's scanner opens Settings but still says `componentsUi.scanner.grantPermission`, so iOS has the same gap (left open below). |

Tests:

- **JVM:** `OfflineIsTheDevicesTest` (r14-3); `R14AndroidFixesTest` (the slow look pace and Check again, `pricedFee`, `ShownFee`, the `fee_moved` words, `LaunchIntent`, `MultisigReach`); `CameraRefusalTest` (r14-10).
- **Device:**
  - `R14MultisigDeviceTest`:
    - `aLinkOverAnOpenSheetIsShownAndAnOlderOneOpenedLateNeverTakesItBack`
    - `aRecreatedActivityNeverOpensItsStartingLinkAgain`
    - `aReviewReadAWhileAgoStartsAtReadingWhenItsSheetOpensAgain`
    - `anExecutionIsSignedOnlyAtTheFeeOnScreenWhenPressed`
  - `ScanCameraRefusalTest`: the Settings label.
  - `MultisigWorld` gains `hang`, `skewMs`, `quoteAmount` and `sentFees`.
  - With the four fixes switched off, all four `R14MultisigDeviceTest` cases fail; with them on, they pass.
- **Live (own emulator behind a fault proxy):** Arc's node was hung in a warm testnet session while OP Sepolia's node timed out by itself. This is the tester's pair of networks. Nine connect timeouts were logged. The home said "Can't reach 2 networks right now" at 18 s and held it for 150 s, with no "Offline" and no `net offline`. With the device's Wi-Fi and data off, it said "Offline". When they came back, the line was gone within 5 s with no touch.

Left open:

- **Core (for the core lane):** the accounts machine runs link opens side by side. An older open's late `say(Member …)` can overwrite the newer link's stand, and its `activate` can switch the account shown. The shell's newest-wins guard covers the sheet, not the account behind it. Superseding the open in flight would be the core's rule.
- **Core:** Execute's silent re-quote is still the core's behaviour. A core rule ("a new look whose fee differs from the one shown stops with it") would give every shell what Android now does in its executor.
- **iOS:** r14-10's label (`grantPermission` over a button that opens Settings).

## 21. Round 14 leftovers

Closes §19.12 and §20's "left open". Decided by best practice under the
owner's rules (one core, full parity at paid-app polish, honest and
recoverable states); each item was written test first.

| Item | Decision |
| --- | --- |
| **Android Settings → Switch account** (r14-1 parity) | `SettingsActions.multisigSection(close)` threads the slot through `SettingsRoute` → `SettingsScreen` → `SettingsSheet`; the Accounts sheet draws `MultisigSwitchAccountSection` under the Vela accounts' Create / Sign in: every multisig here, Create multisig, Add multisig, each closing the Settings sheet first and opening the same flow the header opens (the multisig sheets draw over every route, `MultisigAppLayer`). Now every surface offers both: web/Chrome/Safari (header, Switch account), iOS (header, Settings), desktop (header, settings, browser), Android (header, Settings, browser). |
| **Android add sheet** (r14-5 adoption) | `MsAddView.retry` is decoded; Add reads Try again (`addPressLabel`) while the outcome said is one the core marks retryable, read from the machine's own view at the answer (never a lagging copy); the `busy` answer was already said in `add.busy`. A rate-limited network also leaves the lookup unable to say where the multisig is used, so the person is first asked the network (`chooseNetwork`), then told busy. |
| **Android fee guard** (r14-9) | The shell's `ShownFee` (pressed figure, executor refusal, `MultisigShellFailure.FeeMoved`) and `pricedFee` are deleted: the core refuses a fee other than the one shown (§19.11, `fee_moved`) before it ever asks `send_calls`, and the executor hands an execution to the send pipeline as asked. The words read the core's answer unchanged. `R14MultisigDeviceTest.anExecutionIsSignedOnlyAtTheFeeOnScreenWhenPressed` still proves the behaviour, now with no shell guard. |
| **Rate-limited import** (core + every shell) | `ImportOutcome::Busy {chain_id}`: an import whose network answered only "slow down" is waited out once (`RATE_LIMIT_PAUSE_MS`), then named — the add's rule. `ImportOutcome::offers_retry` (busy, unavailable, unverified) fills `ImportView.retry` (lowercase addresses); each such row offers Try again, which is the same `import` asked again. Said in `add.busy` with the network named on web (`AddMultisigSheet`; its in-progress marker is now `importing`, no longer colliding with the kind), iOS (`MultisigAddSheet`), desktop (`multisig_add.rs`) and Android (`importStatus`). No new copy. |
| **Short forms whole** (r14-11a) | A short form inside prose binds at draw time only — never the core's `short_address`, a copied string or what a reader hears. Android: `AddressBreaks.wholeShortForms` puts U+2060 either side of the "…" in every `MsText` (all multisig rows, subtitles, notes, chips), semantics keep the plain text. Desktop: gpui pieces cannot flow inline, so the CJK line-breaking layer (`ui::prose`) never breaks inside a short form; the add row, proposal headings and titles, member changes, approval rows, facts, timeline, settings headings and activity titles draw through it. iOS: `AddressText.bound` (U+2060 either side of the "…") through `Text(prose:)`, the plain text as the accessibility label; every multisig text draws through it (rows, subtitles, notes, the home card's proposal title, member lines), links and shared text stay plain, guarded by a source test. Word joiners rather than runs on iOS and Android: one `Text` keeps its line breaking and modifiers. |
| **Safari iOS 26 Create sheet** | The wallet/menu `BottomSheet`'s box is capped at the dynamic viewport (`top: 0; height: 100%; max-height: 100dvh`, taps to the scrim and sheet). A sheet no page frames sat in the initial containing block, which iOS Safari sizes to the LARGE viewport (toolbar collapsed), so its foot ("Next") stood under the floating toolbar until a scroll. `dvh` is what is visible with the toolbar shown; in a frame shorter than the window the cap never binds. Not chosen: `viewport-fit=cover` (every page would need its own insets, and it does nothing for the toolbar); a sticky footer on Create (its first step fits once the frame is right). The keyboard lift (`visualViewport`) and `env(safe-area-inset-bottom)` are unchanged. |
| **Desktop "fee moved" line** | Names the executor by the core's member label (`member_labels`, then `names.label`) — the fee line's own name — through a pure `failure_words(loc, detail, chain, name)`; `ms_failure_words` / `ms_action_words` take `cx`. |

Tests:

- **Core:** `a_rate_limited_import_is_named_busy_with_try_again`, `an_import_offers_try_again_only_for_a_read_that_did_not_come_back`.
- **Web:** `AddMultisigSheet.svelte.test.ts` ("a Safe whose import did not come back"), `BottomSheet.svelte.test.ts` ("its foot is on screen, whatever frames it"), `CreateMultisigSheet.svelte.test.ts` ("Next lies within the visible viewport, unscrolled and uncovered", 402×874).
- **Desktop:** `a_rate_limited_import_is_said_busy_and_its_row_offers_try_again`, `the_import_rows_short_form_is_never_broken`, `a_short_address_is_never_broken_inside_a_line`, `a_short_form_inside_a_line_is_one_unbreakable_run`, `a_fee_that_moved_names_its_executor_by_the_cores_member_label`.
- **iOS:** `MultisigR14LeftoversTests` (`theImportViewReadsWhichRowsOfferTryAgain`, `aBusyImportSaysTheNetworkIsBusyNeverUnavailable`, `aShortFormInProseIsBoundAndNothingElseIs`, `aBoundShortFormIsNeverWrappedAfterItsEllipsisAtAnyWidth` — CoreText at every width with a control, `everyMultisigTextDrawsShortFormsWhole`).
- **Android JVM:** `R14LeftoversTest` (the short-form binding, `view.add.retry` and `import.retry` decoded and said, a rate-limited add and import through the real machines, no shell copy of the fee rule, the core's `fee_moved` said with the executor's name).
- **Android device:** `R14LeftoversDeviceTest` (Settings → Switch account reaches Create and Add multisig, closing itself first; a rate-limited add reads Try again until it adds; a rate-limited import row's Try again imports; a zh subtitle's short form whole at every width 1 px at a time, with a control that splits without the joiners) and `R14MultisigDeviceTest` (the fee rule, now the core's alone).

Left open:

- Safari test07 / test08 were not re-run against the `dvh` fix: they need the §19.10 local stack (`multisig-fork/up.sh` forks Gnosis from an upstream RPC, the local signing page and index on loopback, the Safari extension build). The cause is reproduced and fixed in Chromium at 402×874; the simulator run is the remaining proof.
- iOS wallet-level surfaces outside `Features/Multisig/Views` that put a short address inside prose (activity to/from lines, signing rows) are not bound yet.
- Desktop: a short form drawn alone in a box narrower than itself can still wrap (none at shipped sizes).
- Web: should Create's first step ever outgrow the sheet (many accounts, large text), Next scrolls with it; pinning it would mean a footer slot in `multisig/ui/Sheet.svelte`.

## 22. Round 15 fixes (web, Safari, desktop, core)

Decided by best practice under the owner's rules (one core owns the rules,
full parity, honest and recoverable states: nothing claimed before it is
read, nothing done behind the person's back). Each item was written test
first; the native lane (r15-1, -5, -7, -8, -9, -14) is separate.

| Item | Cause | Decision |
| --- | --- | --- |
| **r15-3 Safari Create sheet's Next** | Not `dvh`: on the iOS 26.2 simulator the wallet tab's web view shows 62–776 pt of 874 (top bar 62, floating toolbar 98) and the §21 cap held — the sheet ended at 777. Create's first step is taller than 88% of 714 pt, so Next sat in the sheet's scroll, under its bottom edge. | Every step's action row is the sheet's **foot, outside the scroll** (`multisig/ui/Sheet.svelte` and `settings/ui/Dialog.svelte` gain a `footer` slot; `BottomSheet`'s wallet skin draws the hairline where the body goes on). A step that outgrows the sheet scrolls its body; its action never moves. Chosen over shrinking the step or a sticky element inside the scroll: the foot is the bottom sheet's existing contract (the signing sheet's confirm). Proved: `CreateMultisigSheet.svelte.test.ts` (402×714, 375×548: Next / Next / Create on screen and hit-testable at open) and `SafariRouteDeviceTests` test01/02/07 on an erased iOS 26.2 simulator (new assertion: Next whole above the toolbar at open — 700–752 pt). Harness: a fresh simulator's keyboard introduction is put away (`dismissKeyboardIntro`). |
| **r15-16 `MS_NAME`** | A word only an expansion produced is never an assignment: bash ran `TEST_RUNNER_VELA_MS_NAME=…` as the command (127). | The optional name rides as an `env` argument (`${extra[@]+"${extra[@]}"}`, bash 3.2 + `set -u`). `safari-route-e2e.test.sh` stubs `xcodebuild`/`xcrun`. |
| **r15-6 doubled "That didn't go through"** | Web only: the fee-moved reason carried the failed prefix and the failed line added it again. Desktop, iOS and Android compose it once. | A reason is the reason alone; the line that draws a failed phase adds the prefix once (the draft's line too). |
| **r15-13 a failed review re-read itself** | The web sheet asked `review` every 30 s whatever its state, and the core turned any review not `ready` back to `loading`. | **Core:** `review` on a `failed` review reads nothing — only `review_again` (the person's Try again) does. Every shell is covered, whatever its timers; the web's timers also skip a failed review. A reopened sheet shows the failure with Try again. |
| **r15-12b "… anyway" flashed on recovery** | Right after an outage the anchor (`eth_getCode(safe)`) answered through the pool's outage probe while the checks asked beside it were still turned away: the review landed D1 (enabled "… anyway") until the 5 s re-read. | **Core (`gather_review`):** when the network answered but every safety check came back as silence, the checks are asked once more within the same bounded run; only what is still unread then is said unread. D1 stays for a network that answers but will not trace or simulate. |
| **r15-11 / r15-12a list reads** | (a) A link's sheet opened on the multisig in front a moment ago; the machine said "not loading" for a multisig whose list was a kept copy (`mailbox: false` = not read yet) → "isn't shown here" / "Couldn't reach the multisig service". (b) A newly followed multisig's read, and a Try again, were dropped while another read was out (an old multisig's, or one hung in the outage). Aborted fetches and the link's `seq` were ruled out. | **Core (proposals machine):** a newly followed multisig while ANOTHER multisig's read is out, and the person's `retry`, start a read now (`read_gen`; the older read's answer is dropped when it lands); a read of the very multisig followed again (after a moment of none) is waited for, and `online` reads as before — superseding those broke the onchain suite's executions (found on the fork, fixed before merge). `loading` from such a Follow / Try again until a read of it lands. **Web:** a link's sheet is its multisig's (descriptor `address`), followed at once; a sheet whose multisig is not the one followed says "Opening…"; the sheet's Try again is the machine's `retry`. A 429 from the service is still worded as unreachable (not reproduced; left open below). |
| **r15-4 false "Deposit your first asset"** | Desktop: the first round after launch carried nothing over (`LAST_SETTLED` is in memory), so a refused Arc wrote Dana's rows out of `vela.holdingsCache`. Core: `empty_key` was allowed while some networks were unread. | **Core:** `empty_key` is withheld while any network said for the account is unread (failed, silent, rate limited) — the list says that network (`unreachable_networks`, the hero's words); a rate limit keeps it loading while the pool waits. **Desktop:** the first round carries a failed chain's rows from the disk; the Assets strip and panel draw the unread words (the web's `unreadAssets`). |
| **r15-15 "Checking…" at 50 s** | Desktop: the settle asked the pool thread (rate-limited and own-node chains) with no bound; a pool thread held by a refused network held the first settle. Core: `pool_unreached` ignored the pool until the first settle. | **Core:** a network the pool finds silent during the first read is named at once (`failed_chain_ids` + `pool_said`; `checking_key` gives way); the settle has the last word. Vector `balance-pool-unreached.json` gains the case. **Desktop:** the settle's pool questions are bounded (`SETTLE_ASK_MS` 1.5 s). |
| **r15-2 desktop Add + invite link** | The sheet only sent `OpenLink` and waited for a one-shot link stand read at render; with a multisig in front it never reached the sheet (not reproduced headless). | Add reads the link itself with the core's pure parsers (`broken_link`, `add_link_fault`, `parse_add_link`, `parse_proposal_link`): an invite fills its address and card at once (and is still opened in the core), a damaged link says its words at once, a proposal link spins under "Opening…"; a failed lookup keeps its stand and Try again sends `RetryLink`. |
| **r15-10 desktop full address** | Review rows drew full addresses as plain text; gpui broke at the overflowing character. | Rows draw them through `address_text` (22 + 20); `ui::prose` allows a break inside a full address only between its halves. |
| **r15-1 web adoption (§23)** | The core's `FeedView` now names its account and says when it has looked; the web drew whatever rows it held and said "No activity yet" once the balance had looked. | `feedFor(feed, shown)`: Activity (home, wide home) and History draw the feed only when `account` is the account shown (the balance's address), else loading; an empty list says "No activity yet" / "No transactions yet" only when `settled`. The extension's pages share these models. |
| **Fee moved (desktop, not concluded in R15)** | Desktop and web re-ask a priced look every 5 s; the core swaps a look older than 15 s. A press could sign a look that replaced the figure after it was painted, and the stop rarely fired. | **Core:** `Execute` / `ApproveExecute` carry `shown_fee` (base units): any look holding another figure stops with `fee_moved`, nothing signed; without it (older shells) the r14-9 rule stands. Web and desktop send it; the desktop scenario proves the stop and the line "That didn't go through: Network fee ≈ …, paid by Ana" (once). |

Tests: core `app_multisig_review_state.rs`
(`a_failed_review_is_read_again_only_on_the_persons_try_again`,
`a_review_coming_back_from_an_outage_never_lands_anyway_on_the_way`),
`app_multisig_proposals_reads.rs` (three cases),
`app_multisig_proposals.rs`
(`a_press_signs_only_the_fee_it_carries_even_on_a_fresh_look`),
`app_balance_dashboard.rs` (`an_unread_network_never_yields_an_empty_claim`,
`a_silent_network_during_the_first_read_is_named_never_checking`); web
`CreateMultisigSheet`, `ProposalSheet` (fee prefix once, Opening for a
multisig not read yet, `shown_fee` sent); desktop scenarios and unit tests
(link read at once, Try again → `retry_link`, full-address halves, first
round from disk, unread list words, settle budget, fee moved).

Left open:

- iOS and Android do not send `shown_fee` yet (optional; their presses keep
  the r14-9 rule) — for the native lane, beside r15-5. (Done: §24.)
- A 429 from the multisig service reads as "Couldn't reach the multisig
  service" (the r15-11b rapid-link run could also have met the service's
  rate limit; not reproduced here).
- r15-2 and r15-15's desktop causes were not reproduced live; the fixes make
  them unreachable (the sheet no longer depends on the one-shot stand; the
  settle no longer waits on the pool thread).
- Desktop: with only a rate-limited network and no rows the Assets strip
  stays loading, as the web's skeleton does.

### 22.1 Two web e2e reds after the merge

Both were timing-dependent: they passed alone and on a quiet machine, and
failed under load. Neither reached a live endpoint. Every request was
logged (`page.on('request' | 'requestfailed' | 'requestfinished')`), and
every off-origin one was refused by `denyOffOrigin`. A full run with every
page behind a proxy that does not exist had 505 green and two reds. Both
reds came from the load (the Rust suite was running beside it) and passed
alone behind the same proxy. The extension suites launch their own browser,
so the proxy did not cover them.

| Red | Cause | Decision |
| --- | --- | --- |
| `home-truth` :158 [webkit], three "Fix" rows | **Test hermeticity.** The list named Arbitrum and Gnosis ("Held nothing when last read": read, then lost) beside the faulted Ethereum. The stub answered the deposit watch's `eth_getBalance` and the Activity scan's `eth_getLogs` with HTTP 500. To the pool that is a transport failure, so it swept the network's shipped public nodes, which `denyOffOrigin` refuses (Arbitrum and Gnosis ship three each). That makes three silent endpoints on every tick. With five seconds between two such ticks and no other answer, the silence rule (r13-4) named the network, and r15-15 says it at once during the first read. The rule did what it says: the stub's network did fail two reads out of three. | `stubJsonRpc` answers those two background reads, when a suite does not, with a JSON-RPC "method not found". That is an answer, as a working node gives for a method it does not serve, and the callers read it as before (no reading). The silence rule is unchanged. |
| `fee-coin-switch` :97 [chromium], `click` timed out | **Core.** The ETH row of the fee sheet was disabled ("Insufficient ETH for gas fees"). A Max pressed during the form quote's 400 ms debounce made that quote price the transfer of the WHOLE balance. The fee machine weighs each coin against what the priced operation leaves, so on the next re-price (12 s on Ethereum) the chain's coin had nothing left. But a Max follows a fee-coin switch by holding the fee back, so the coin could pay. The test passed only when the pick beat the re-price. A 14 s pause before the pick reproduces it every time. | Under a Max, the form's quote is the Max's own transfer (`max_estimate_call`: one wei of the chain's coin, as the warm-up and the Max's estimate already are). Continue's pre-check still prices the exact amount. Test `a_form_quote_under_a_max_prices_the_maxs_transfer_not_the_whole_balance`. |

## 23. Round 15 native fixes

Round 15's Android and iOS findings (r15-1, r15-5, r15-7, r15-8, r15-9,
r15-14) and the iOS checks R15 left unconcluded. Decided by best practice
under the owner's rules: one core owns the rules, the shells draw it; never
another account's data; never act or fail silently; orange only for what
moves value or signs. Each item was written test first.

| Item | Cause | Decision |
| --- | --- | --- |
| **r15-1 (S3) Another account's activity** | The core's feed. `AccountSwitched` bumped the attempt (dropping late answers) but KEPT the previous account's folded rows and rendered them at once — "the previous feed keeps painting until the new read commits", a port of the TypeScript `addressRef` behaviour, pinned by a test. On Android the switch reaches the feed only after the wallet route's awaits (networks loaded, the space, the currency), so on a cold start whose link put a multisig in front the multisig's home drew the first account's "Received" rows for as long as that took. The view also never said whose rows it held, nor whether it had looked: Android drew "No activity yet" over any empty feed, before any read. | **Core:** a switch to another account empties the rows and records at once (the same account said again is not a switch). `FeedView` names the account its rows are for (`account`, lower-cased) and says when "No activity yet" may be claimed (`settled`: this account's store read AND its first incoming scan came back since it was switched to — the iOS/desktop R12 S4 rule, now the core's). The first scan back is drawn by a render the feed makes anyway (the cadence's armed timer, the re-read of what it found, or — a scan asked for again while it was out — the next scan's answer); a shell not following the cadence gets one render. The shared cadence vectors (one redraw per scan) are unchanged. Additive wire fields with serde defaults. **Shells:** the home's Activity and History draw the feed's rows only under the account they are for, and an empty list says "No activity yet" only when `settled`; otherwise the skeleton — the D3 rule for the balance, applied to Activity. Android `WalletLive.home(shownAccount =)` / `activityMode`, `FlowLive.history(shownAccount =)`; iOS `WalletLive.apply(shownAccount:)`, `FlowsLive.history(shownAccount:)`, `FeedViewWire.isFor`. Desktop: no shell change — its rows are the core's (now emptied on a switch) and its empty line already waits for the account's scan. Web: the core's emptying reaches it unchanged (its empty line waits for the balance's first look); adopting `account`/`settled` is the web lane's. |
| **r15-5 (S3) iOS: the first press after a fee move did nothing** | The core's `fee_moved` stop (§19.11) worked: nothing was signed and the action recorded `failed {fee_moved}` in the right words. iOS drew every action's failure among the notes under the sheet's TITLE — off screen at the top of a long sheet, while the person was at the bottom pressing. To them the press did nothing. | A press's failure is said beside the button pressed (`MsProposalModel.pressFailures(at:)`): Approve's and the one press's under Approve, Execute's under Execute; at the head only when no button is drawn for it (done, or nothing here can act). Said once, in the core's words, with one "That didn't go through:" (`failureText` composes `proposal.failed` over `proposal.feeLine`; the web's doubled prefix, r15-6, is the web lane's). The next press signs the new figure, as the core rules. Android already drew it beside the button. |
| **r15-7 (S4) Android: undeployed multisig's fee line** | The one-time setup lines (`setupLines`) were drawn only under Execute, never under the one press ("Approve and execute") the tester used. And a look whose Safe contracts went unread (`unverified`) or were read and refused is not offered as a one press (`approve_executors`), while `approveOnlyWhy` said only "would fail" and "cannot pay" — so nothing was said. | `MsProposalModel.executionLines(executor, failure)`: the fee line, then the setup lines — one function the one press and Execute both draw. `approveOnlyWhy` also says `unverified` ("Vela couldn't check the Safe contracts on {network}…", warning) and `refused` (danger), as iOS and the web do. |
| **r15-8 (S4) Android: "Already added" after Try again** | The row said "Already added" whenever the Safe was on this device, so a Safe this very import added (busy, then Try again) read as one added before. | The web's rule: "Already added" only for a Safe that was here before this import said anything of it (`importAlready(here, outcome) = here && outcome == null`); otherwise what the import came to ("Imported"). A Safe here is not pickable again, and "Import N" counts only Safes not here yet — a press that would import nothing is never offered (it was: "Import 1" over a Safe just imported, a silent no-op). The same fix on iOS (`MultisigAddSheet.importAlready`), which had the same rule. |
| **r15-9 (S4) iOS: orange in Switch account** | The Accounts sheet drew the selected Vela account's name and check in the accent; and Settings → Switch account (unlike the header switcher) kept the Vela account checked while a multisig was in front — two checks. | Choosing an account moves no value and signs nothing: name and check in neutral ink (`fgBase`). One check: while a multisig is shown no Vela account row wears it, in every switcher (`SettingsLive.withAccounts(multisigShown:)`). |
| **r15-14 (S4) iOS: the fee note twice** | With the one press offered and the transaction not ready, the note was drawn under the one press AND in Execute's place. | Said once: Execute's place draws it only when no one press is offered (`feeNoteUnderExecute`), as Android does. |

R15's unconcluded iOS checks, now covered by tests on the simulator:

- **r14-10 "Open Settings":** the label choice is one function
  (`CameraScanner.Refusal.actionLabel(loc)`) used by the scan sheet and
  Explore's scanner: "Open Settings" for a camera refused for good, no button
  for restricted, missing or busy cameras (the simulator has no camera, so the
  live path stays a device check).
- **Import busy with Try again:** a busy row offers Try again with the
  network named; Try again asks the same import; the landed import says
  "Imported".
- **Newest link wins:** through the real accounts machine and the iOS
  executor, an older link whose lookup is held while a newer one opens:
  the newer stand and account stay, the older lookup's landing switches,
  opens and says nothing (`MsSimWorld.holdGenesis`).

Tests:

- **Core:** `a_switched_feed_never_shows_the_previous_accounts_rows_and_says_when_it_has_looked`, `the_first_scan_back_settles_the_empty_claim_on_a_render_the_cadence_makes`; `switching_accounts_drops_in_flight_answers` now asserts the rows go with the switch; `privacy-hidden.json` regenerated (two new view fields).
- **Android JVM:** `R15NativeFixesTest` (Activity keyed to the account, not settled is loading, the multisig's unread history still said; the one press's setup lines and the unread setup said; Imported after Try again).
- **Android device:** `R15NativeDeviceTest` (an undeployed multisig's one press shows "Includes one-time setup on Gnosis:" and "deploying this multisig"; a rate-limited import's Try again lands as "Imported", never "Already added", with Import N shut). The unread-setup case stays JVM-only: the simulated world cannot leave the Safe contracts unread while the look's balance read answers (it says `unchecked` first).
- **iOS:** `MultisigR15NativeTests` (the same r15-1 rules on the home and History; press failures beside their buttons with one prefix; the fee note once; Imported; one neutral check, no accent pixel on the hosted Accounts sheet; the camera label; newest link wins) and `MultisigFlowTests.aFeeThatMovedStopsThePressSaidBesideItAndTheNextPressSignsTheNewFigure` (through the hub over the world: a stale look whose re-quote moved signs nothing, `fee_moved` is said beside the one press with one prefix, and the next press signs exactly the new figure; `MsSimWorld` gains `skewMs`, `quoteAmount`, `sentFees`, `holdGenesis`).

Left open:

- r15-1 on a device end to end (cold start, killed process, hung Arc, a second link) was not re-run; the rule is the core's and every shell's draw is unit-tested. Android still tells the feed about a switch only after the wallet route's awaits; the home now says "reading" for that time instead of another account's rows.
- Android History while not `settled` draws no skeleton of its own (an empty list, never "No transactions yet"); a skeleton there is polish for a later round.
- The web draws the core's emptied rows but not yet `account` / `settled` (its empty line waits for the balance's first look): the web lane's to adopt.
- The iOS fee-moved stop and the one-press setup line were proven in the simulated world, not on Arc with a moving relay.

## 24. Natives carry the fee shown

- **Android and iOS send `shown_fee`** (§19.11, §22) on every press that executes — Execute and the one press "Approve and execute" (`MsProposalModel.executeEvent`); a site's transaction executes through the same sheet. The draft's `propose_execute` draws no figure, so it carries none (as on web and desktop).
- **The figure** is `MsProposalModel.shownFee`: the priced look's `fee.amount` in the model the sheet painted (the press's own closure), only while drawn; no figure drawn, none carried (r14-9 holds).
- **A quote landing between paint and press** signs nothing: `fee_moved`, said once beside the pressed button in the core's words; the next press signs the new figure.
- **Tests:** Android `NativeShownFeeTest` (JVM, real machines) and `NativeShownFeeDeviceTest` (frame clock held while the look lands; the press is the button's own click); iOS `MultisigFlowTests` `theOnePressCarries…` / `executeCarries…` (through the hub).

## 25. Round 16 fixes (core, web, desktop)

Decided by best practice under the owner's rules (one core owns the rules
and every shell draws them; full parity; honest states — nothing claimed
before it is read, no raw error, never silent; a press's failure is said
beside the button pressed; no signable state appears transiently). Each item
was written test first; the Android/iOS findings r16-4…r16-15 are the native
lane's (`r16-fix-native`), and this lane's Android/iOS edits only adopt the
core changes below.

| Item | Cause | Decision |
| --- | --- | --- |
| **r16-3 (S3) "… anyway" on the way back from an outage** (r15-12b not fixed) | §22's re-ask ran only when EVERY safety check came back as silence. Arc's nodes answer a state trace with "method not found" — an answer — so after an outage lifted (its multisig's own code answered first) the probe and the simulation, still turned away for 1–5 s, were never asked again: the review landed `unchecked` with an enabled "Approve anyway" until the next read. | **Core rule** `review_state::checks_final(review, anchor, checks, rechecked_ms)`: "unchecked" is published only once the checks are final for this read — the review would not land unchecked anyway, or its multisig's own state went unanswered (it lands `failed {chainUnread}`), or no check is still silent (a refusal is the network's answer), or the re-ask reached its own limit (`RECHECK_MS` 10 s). Otherwise `gather_review` asks the silent checks again, paced (`RECHECK_PAUSE_MS` 1 s), each round bounded by what is left of the limit; the review stays Reading (every sign button off). Failed → Reading → ready on every shell, since every shell's review is the core's. A simulation is silent only when its reply says `{"unreachable": true}` (no reply at all — a shell that does not simulate, a question cut at its bound — is final). Cost: a network that answers its state but never its checks lands D1 up to 10 s later than before. Link door `review.checksFinal`; vectors `review-state.json` `recheck` (Rust and web replay). The simulated world's clock now answers the soonest wait first (a 1 s pause runs out before a 40 s bound). Android, iOS and desktop draw the core's review state and door unchanged. |
| **r16-1 / r16-7 fee-moved stop's figure** | The stop kept the figure of the moment it was said while the fee line kept re-quoting; the next press signed the fee line's. | **Core rule, chosen over freezing re-quotes:** the view serves an execution's `fee_moved` stop with its executor's look NOW (`stops_follow_looks`): a re-quoted figure moves the stop with it, and a look with no figure withdraws it (the fee line then says why nothing can run). Freezing was rejected: a frozen look ages past `PREVIEW_FRESH_MS`, and the next press would re-quote and stop again. Every shell draws `view.actions` as it is, so desktop, web, iOS and Android follow without a shell change; the desktop draws the stop beside the pressed button (below). |
| **r16-22 "Deposit your first asset" under a hung Arc** | Not a hung read counted as read: the round the account's switch got was the shell's TTL cache (`fetchTokens`, 5 min) — rows read earlier, no network asked now — and its "nothing" was taken as every network's while Arc's live read was still out (named only when the pool found Arc silent, ~18 s). | **Core rule:** an empty-state claim needs every network read *now*. The balance machine records whether a round of the shown account came from the networks (`read_live`, reset on a switch); `empty_key` and `live_key` need it; and a cache round that says the account holds nothing is read live at once (forced past the cache) — the claim, or the outage, is that read's to say. The stamp stays the cache's time (r10-1): it says when those rows were read, not "now" unless they were. |
| **r16-16 a 429 from the multisig service never said** | A `429` is an answer, so the service's outage stayed "up"; only `busy_until` held background reads, and nothing in the view said why the list was not read. | **Core:** the service's `Outage` records a read answered with its rate limit (`rate_limited`); background reads back off for the wait it named (capped at 5 min), else a growing step (`BUSY_STEPS_MS` 10/20/40/60 s); any other answer ends it; the person's Retry asks at once (it resets the outage). The waiting group says `WaitingNote::Busy` with Retry, after "unreachable" and before a network's own note: "Multisig service busy — waiting transactions will load shortly; retrying." (`home.multisig.service.busyNote`, 15 locales; on-demand pin 693). Web, iOS, Android and desktop word it. |
| **r16-21 web full addresses in batch rows** | The rows drew a full address as one wrapping text (`overflow-wrap: anywhere`). | `addressHalves` (the desktop's r15-10 rule, `ui::address::address_halves`): a full address or hash is two unbreakable halves, 22 + 20 for an address, with a break opportunity between them. `AddressText` draws it; `KeyValueRows` (every call-review row, "Run by") draws its values through it. |
| **r16-23 Safari zh test** | `toWalletHome` matched "钱包" by prefix, so the zh account "钱包E2E" (the header's switcher) was tapped; the created title was waited for in English with a one-shot zh check of words the product does not use ("多签已创建"). | The wallet tab is matched by its whole label (`tapControl(exact:)`); the created title is waited for in the run's own words (`Words.created`: "多签账户已创建" / "Multisig created"). A third cause showed on the run: zh's taller network step put Gnosis under the sheet's foot (r15-3's action row, outside the scroll), and its tap landed on the foot — a body row is now revealed above the foot too (`reveal(aboveFoot:)`). Proved on a fresh iOS 26.2 simulator against a fresh fork harness (97xx), the local page and index: zh test01, 02, 07 and 08 pass. (The runner enrols Face ID before the first test; a simulator not yet booted makes the first passkey ceremony fail — boot it first.) |
| **r16-1 (S3) desktop: the stop off screen** | Every Approve / Execute failure was drawn in the scrolling body under Approvals, off screen at the press, while the footer re-quoted. | A press's failure is drawn in the sheet's footer directly under the presses (at the footer's top when no press is drawn; only a done sheet, which has no footer, says it at its head) — once, one prefix, never in the body too — and worded from the view at every render (`sheet::press_failure_lines`), so its figure is the core's now. Tests: `a_press_failure_is_said_in_the_foot_beside_the_presses`, `a_press_failure_is_worded_from_the_view_at_each_render`; the fee-moved scenario now re-quotes while the stop is shown and the stop's figure moves with it. |
| **r16-2 (S3) desktop: "No activity yet" before the account is read** | Activity used a per-process "scanned" set, so an account scanned earlier said "No activity yet" at once after a switch; the empty claim ignored "Checking…" and unread networks; "Updated now" ignored unread networks. | The desktop adopts `FeedView.account` / `settled` (§23): rows only under the account they are for; "No activity yet" / "No transactions yet" only once settled, the first balance read over and every network read; a refused or silent network is named in the hero's words, a rate limit keeps the skeleton. "Updated …" is withheld while any network is unread (`all_networks_read`). The `SCANNED` set is gone. Tests: `activity_is_claimed_only_for_the_account_shown_and_only_when_read`, `updated_is_never_claimed_over_an_unread_network`, `an_empty_history_says_so_and_a_loading_one_waits`, and others. |
| **r16-17 desktop: damaged invite** | The first Add on a damaged link put the link's body in the field and said `add.badInvite`; a second Add read that text as an address ("Enter a valid address"); in the tester's run the first click went to window focus. | The form remembers the damaged text and its key (`field_damage`): a pasted damaged link is said on input, and every Add on it says the damage again, as does a sheet a damaged link opened. Test: `a_damaged_invite_is_said_as_damaged_on_paste_and_on_every_add`. |
| **r16-18 desktop: raw `Other("…")`** | `multisig/live.rs` sent `format!("{failure:?}")` as the failure's message, drawn raw. | `ShellFailure::ChainUnreachable`: a read that fails before the passkey on a network the pool finds failing says `proposal.reason.chainUnreachable` (the send column's rule); `RelayerUnavailable` says `not_sent`; any other unnamed failure or core diagnostic says `proposal.reason.unknown` ("did not check out" says `passkeyFailed`, as on the web). The Debug text goes to the log only; said beside the press (r16-1). Test: `an_execution_that_failed_is_said_in_the_cores_words_never_debug_text`. |
| **r16-19 desktop: "no longer in the list" at launch** | The sheet sent `review` before the list was read; the core landed it `failed {gone}`, and a failed review is read again only on Try again (r15-13). | The desktop asks the review and timeline only once the machine holds the transaction (`sheet::asks_review_now`); a sheet whose multisig is not yet followed says "Opening…" with no Try again (r15-11's rule, as on the web). **Core rule (this lane):** a proposal review asked before its multisig's list was read is not failed `gone` — it stays Reading and is read once the list lands (below). Tests: `the_review_is_asked_only_once_the_transaction_is_listed`, `a_sheet_whose_multisig_is_not_followed_yet_says_opening`. |
| **r16-20 desktop: Settings → Account beside a sheet** | At 1280 wide with a sheet open the settings column is 304 px; both account buttons shrank in one row, cutting their centred labels ("reate New Accoun"); the full address was squeezed between the name and the rule chip. | The buttons' row wraps (`account_button_row`), each at its label's width; a multisig's name and rule chip share a wrapping line (`name_and_rule_line`) and the full address sits below at the column's width. Tests: `the_account_buttons_wrap_and_each_fits_beside_a_sheet`, `the_multisig_heading_wraps_its_chip_never_squeezing_the_address`. |
| **r16-19 core: a review asked before its list is read** | `review_proposal` landed `failed {gone}` whenever `find_item` found nothing — also while the multisig's list was still being read; a failed review is read again only on Try again (r15-13), so any shell asking early met it. | A review asked while its list is unread (the first read out, or the mailbox on that network not read: `list_unread`) is not failed: it stays Reading (`review_waiting`). After every read of the list (`reviews_after_refresh`) it is read when the list holds its transaction, said gone when the list — read — does not, and waits on while the list is still unread. Test: `a_review_asked_before_the_list_is_read_waits_for_it_never_gone`. |

Tests:

- **Core:** `app_multisig_review_state.rs` (`a_review_whose_trace_is_refused_waits_for_its_silent_checks_before_unchecked` — the door watched after every answer: nothing signable while it is read; `checks_that_stay_silent_land_unchecked_only_at_the_reasks_own_limit`; `a_review_asked_before_the_list_is_read_waits_for_it_never_gone`; the `recheck` vectors through `review.checksFinal`), `app_multisig_proposals.rs` (`a_fee_moved_stop_names_the_figure_the_next_press_signs`, `a_fee_moved_stop_is_withdrawn_when_the_look_has_no_figure`), `app_multisig_proposals_reads.rs` (`a_rate_limited_service_is_said_busy_backs_off_and_retry_asks_now`), `app_balance_dashboard.rs` (`a_cache_that_says_nothing_never_claims_an_empty_wallet_until_read_live`). The r16-3 machine tests fail on the R15 core.
- **Web:** e2e `multisig-review-recovery.e2e.ts` — refused and hung, a DOM watcher on every mutation and frame from before Try again to an enabled Approve. Against the R15 core both runs fail ("Approve anyway" enabled at 19.5 s into the watch); against this core both pass, Reading seen on the way. The single-node world needs the app's own reads to find the node again (its pool cooldown) before the press, as a person does; the network then turns the checks away for 2.5 s. `review-door.test.ts` replays `recheck`; `activity.test.ts` (busy note, Retry); `address-runs.test.ts` and `KeyValueRows.svelte.test.ts` (halves at 220–400 px, each half one line).
- **Desktop:** see the rows above. **iOS:** `R13FollowupsTests.aBusyServiceIsSaidWithRetry`. **Android JVM:** `MultisigViewsTest` (busy note with Retry).

Left open:

- The sheet's own words for a transaction not in an unread list (`missing_said`) still say a rate-limited service as "Couldn't reach the multisig service"; the home's note says busy.
- From the desktop lane, core rules for a later round: `ActionState::Failed {detail}` carries English diagnostics that the web, iOS and Android match on or show (iOS `failureText` shows a non-JSON detail raw — r16-18's iOS twin); the activity scan's `SyncCompleted` does not say which networks it could not read, so `settled` can be true while a network is down (the desktop reads the balance view's unread networks instead); whether "Updated …" may be claimed is derived in each shell (an `updated` key in the balance view would make it one rule); there are no words for "Activity can't be read on {network}" (the desktop reuses the hero's).
- None of the desktop fixes was looked at in a running app (unit and scenario tests only); r16-20 wants a look at a 1280-wide window with a sheet open.
- iOS r16-7's second half (the fee line said "Couldn't work out the network fee" for ~2 s after the press) is the native lane's; the staleness half is the core's rule above.

## 26. Round 16 native fixes

Round 16's Android (r16-4, -5, -12, -13, -14, -15) and iOS (r16-6 … -11)
findings. Decided by best practice under the owner's rules: one core owns the
rules and the shells draw it; never act partly and say nothing; never fail
silently; never show a stale figure as current; one check for the one thing
in front; orange only for what moves value or signs. Each item was written
test first. The fee-moved stop's figure (r16-7's first half) is the core
lane's §25 rule (`stops_follow_looks`, merged from `r16-fix-core`): the
natives draw the core's detail and adopt it with no shell change.

| Item | Cause | Decision |
| --- | --- | --- |
| **r16-4 (S3) "Approve and execute" approved only, silently** | The core's one press (`approve_execute`): the press came 16 s after its look (priced 0.12, on screen and carried as `shown_fee`), so `execute` re-quoted — and Arc's 429s left the re-quote `unpriced`. r4-11 then approved alone ("the approval is still given and kept") and r11-9 made that stop *quiet* (no `execute:` state), so nothing was said; the next look priced it again and the sheet read "2/2" over a plain Execute. | **Core (minimal, `multisig_proposals.rs`):** the one press does both or nothing. Stopped before its signature (fee not worked out or checked, would fail, cannot pay), nothing is approved either, and the stop is kept on `execute:<hash>` — every shell already draws `stopped` by its look's kind beside the press ("Couldn't work out the network fee right now."). A one press over a look that priced nothing (no figure shown, none carried) never signs a figure priced only at the press: `fee_moved` with that figure, nothing signed (`execute_as(one_press)`). `keep_look` lifts a `stopped {unpriced \| unchecked}` once a look prices it, so a stop never outlives its reason. The proposer's own one press (`propose_execute`) is unchanged (it approves by proposing). The approval alone stays the person's own press: **Android** now offers "Approve" beside "Approve and execute" (the web's and iOS's second button, secondary style, `multisig.proposal.approveOnly`); the one press settles on its execution alone (it no longer waits ten minutes for an approval the core never gives); a fee stop's way on is **Check again** (a new look), once — the separate Check again under the press is not drawn beside it. **iOS** offers Check again under the one press for an unpriced/unchecked look or stop. "Approved — not executed: …" (the web's `partial` phase) can no longer arise from the one press; the words stay for any shell path that approves first. |
| **r16-5 (S3) Reject on an unread review failed silently** | Reject proposes a cancellation (§r9-13): its draft reads the chain, which was down, and `hub.propose` answered `unread` — logged, never drawn. | Say the core's answer **beside Reject** with Try again (`rejectFailureLine`: `unread` / `mailboxUnreadable` in the warning tone, refusals and failures in danger; the person's own passkey "cancel" says nothing). Reject stays enabled — the cancellation is its own proposal with its own review, and Try again asks it again. **iOS parity:** its reject failure was said among the notes under the sheet's title; it is now beside Reject with Try again (`MsProposalModel.rejectLine`), at the head only when no Reject is drawn. Web: its failed phase is already said beside the actions. |
| **r16-12 Two checks in Settings → Switch account** | Android's Settings built the accounts sheet with the session's active index; only the header passed "none" while a multisig was in front. | `SettingsLive.withAccounts(multisigShown =)` (iOS's r15-9 rule): the multisig's row wears the one check. |
| **r16-13 A stale balance shown as current** | Android's asset rows had no notion of a figure not read this round (the web marks `unreachable_networks` rows "Last seen …"); on a cold start with Arc hung the restored rows (`holdings_restored`, last session's) stood as current. | `WalletLive.staleChains`: a row is **last seen** when its network is in the core's `unreachable_networks` or rate-limited, and — every row — while the account's first read is still out (`checking_key`: the rows are last session's, in the core's own words). Mainnet: "Last seen $0.83" in the worth's place (the web's row); the testnet space has no worth line, so the amount itself says it: "Last seen 0.827013 USDC". Drawn in the quieter ink. Home and the Assets list. |
| **r16-14 The shut button's label flipped; the note was untrue** | While the review is unread the timer keeps taking looks that come and go with the same outage; the label followed `approve_execute`, and the look's line ("Couldn't check whether it would run — …") and Check again sat under a button nothing could press, beside the door's own Try again. | While the door is shut (`saysExecution` = a press can sign) the button reads **Approve**, steady, and nothing about executing is drawn — no fee line, setup, Check again or "why not the one press": the door's reason ("Couldn't reach Arc Testnet to read this transaction." with Try again) is the one note. |
| **r16-15 A gap under the setup's head** | Each setup line was its own note in the sheet's column (its section spacing), so the head ending in ":" stood apart from its item. | `ExecutionLines`: the fee line, then the setup as ONE block — head and items at the block's line spacing, items marked "·". |
| **r16-6 iOS: the other space named the testnets** | iOS told the accounts machine the space only at boot (`networksChanged` had no caller); after a switch to Testnet the core still held Mainnet's networks as "the space", so `create.other_space` was the testnets. | `MultisigHub.spaceChanged(chains)`, sent with the home's space networks on every change (after boot). The line is the core's over the space in front. |
| **r16-7 iOS: the stop's stale figure; the 2 s "couldn't work out" flash** | The figure: the core lane's §25 (`stops_follow_looks`) — the stop follows the look now; iOS draws the core's detail unchanged. The flash: the multisig's fee session supersedes an older quote with `nil` when a newer one is asked; the press's own re-quote beside the sheet's timer made one of them "failed" → `unpriced` for ~2 s. | `MsQuoteLine`: the multisig's quotes run one at a time (FIFO), identical asks share one answer — no quote is ever superseded into "failed". |
| **r16-8 iOS: "1/2 approved" while executing** | The chip is the core's standing, which turns `executing` only once the flight is recorded; during the press's passkey and send the mailbox still counts 1 (the one press carries the last approval itself). | While this device's Execute or one press is out (`execute:` working) or the core follows its flight, the chip says "Executing…". Android parity, same rule. |
| **r16-9 iOS: the keyboard covered Next and could not be put away** | Next sat at the end of the scrolling body; nothing but an interactive scroll dismissed the keyboard. | The create sheet's action row is the sheet's **foot** (`MsSheet(footer:)`, `safeAreaInset(edge: .bottom)` — the web's r15-3 contract), riding above the keyboard; every `MsSheet` gets a keyboard toolbar "Done", a tap on empty space and any drag of the body put the keyboard away, and Return does (`MsField` `.submitLabel(.done)`). |
| **r16-10 iOS: totals disagreed; testnet priced in CNY** | The switcher's total summed every account with an unknown figure as 0 ("Total CN¥0.00" beside "CN¥2.99", read a moment apart), and the testnet space was given worth there though its home says none. | One rule for both switchers (one builder): the total is the sum of the balance core's figure for **every** account, said only when each is known (`SettingsLive.accountsTotal`); the testnet space shows no figure on a row and no total. |
| **r16-11 iOS: another account's home flashed at launch** | The multisig in front is the accounts machine's, read from its store after the hub boots; until then the home drew the Vela account's. | `LaunchFront`: when a multisig was in front at last run (`vela.activeMultisig`, read synchronously at launch) the home holds the page's own ground until the hub has read it, at most 2 s. The faded welcome screen at 3 s was not reproduced. |

Tests: core `a_one_press_whose_fee_is_unknown_at_the_press_signs_nothing_and_says_why`,
`a_one_press_over_no_figure_signs_nothing_until_a_figure_is_shown`,
`approve_and_execute_stopped_before_signing_approves_nothing` (replaces the
r4-11 test); the D1 "… anyway" test now presses on its look's figure. Android
JVM `R16NativeFixesTest`; device `R16NativeDeviceTest` (the r16-4 press over a
refused re-quote: no passkey, no approval, the stop and one Check again, then
lifted; the setup block's gap measured); four older device tests that
approved by pressing the one press over a refused quote (r4-11) now press
Approve beside it. iOS `MultisigR16NativeTests` (the create sheet's Next whole
on the SE's screen at open, hosted; the testnet accounts sheet with no worth);
`MultisigFlowTests`' reject case reads the line beside Reject.

Left open:

- **For the core lane:** a plain Execute over a look that priced nothing still
  signs a figure priced at the press (only the one press got the r16-4 rule);
  the same `one_press` guard would cover it.
- The web's `onExecute` keeps its r4-11 branch (`approved-not-executed`); it is
  now unreachable from the one press — the web lane may drop it. Desktop's
  comment on r4-11 likewise.
- r16-11's faded welcome screen (once, not reproduced).

## 27. Round 16 leftovers

The open lists of §25 and §26, decided by best practice under the owner's
rules (one core owns the rules and every shell draws them; full parity;
honest states — never raw error text, never a figure signed that nobody was
shown). Each item was written test first.

| Item | Decision |
| --- | --- |
| **1. A plain Execute over no figure** | The one press's guard (§26, `execute_as`) is now every press on the sheet's: `execute_as(…, pressed)` is true for Execute and "Approve and execute". A press whose look priced nothing (no figure drawn, none carried) never signs a figure priced only at the press — `fee_moved` with that figure, nothing signed; the next press is on it. Not pressed: only the proposer's own one press after proposing (`propose_execute`), whose draft said its fee (`draft.executeNow`). Core and fork-suite test helpers now press on the figure their look drew, as every shell does (§24). Test: `a_plain_execute_over_no_figure_signs_nothing_until_a_figure_is_shown` (fails on the R16 core: the press signed). |
| **2. Dead "approved — not executed"** | The one press never approves alone (r16-4), so the web's `partial` phase, its `approved-not-executed` line and the corpus words `home.multisig.proposal.approvedNotExecuted` (15 locales; multisig pin 693 → 692) are removed; no other press reaches them (Approve is its own press, its failure said as any). The desktop's r4-11 comment beside the press is corrected. The web test fake now follows the core (both or nothing), and its test says nothing is approved and the stop is said beside the press. |
| **3. The sheet under a 429** | One answer, the core's busy (§25 r16-16): `ProposalsView.busy` (the service's last read answered its rate limit); `review.missing {…, busy}` says `home.multisig.service.busyNote` with Try again when the list is unread because the service is busy (a network that did not answer is still said first; a list read and lacking it stays "not in the mailbox"); a link's `NotFound` carries the same key as its `reason_key`. Every shell words that key; none says "Couldn't reach the multisig service" for a 429. Vectors `review-state.json` `missing` (busy cases). |
| **4. Failures as kinds** | New core module `multisig_failure`: a failed action's detail is JSON with a stable `kind`. The core's own failures are `passkey_check` (what was signed does not check out) or `unknown` with a stable `reason` code (for a log); every failure a view hands a shell passes `served`, so plain text (a shell's English, `describe(error)`) becomes `{"kind":"unknown"}` with its text dropped — in the proposals view's actions and the accounts view's create / share-again failures. `review.failure {detail}` → `{kind, key}`: the catalog key for every kind whose words need no more than the network's name, `null` for the kinds worded from their own facts (`fee_moved`, `cannot_pay`, `venue_blocked`). The shells map kinds (web `detailError` / `failureWords`, iOS and Android `failureText`, desktop through `failure_said`) and never read or show a detail's text or `message`; a passkey's refinement stays a catalog key. Vectors `review-state.json` `failure`, replayed by Rust, web, iOS and Android. The same rule on the texts beside them, on every shell: a refused draft whose reason is not a member change's JSON says `review.refusedWhy.other`; a create's `draft_error` says `proposal.reason.unknown`; a record the service refused says `settings.shareAgainNote`; the outside signing relay's line names the relay's refusal, a pass not got, or the relay's host — else "Vela went offline — reconnecting…" — and a room not opened says `proposal.reason.unknown`. |
| **5. Activity's unread networks** | The trust machine says which networks of the account its scan could not read (`TrustView.unread_chains`: a head that did not answer, the live window's logs refused or capped twice; a backfill read that fails does not count; a network read again leaves it; a network the poll no longer reads, or an account switch, empties it). The shell hands it to the feed with the scan's answer (`SyncCompleted.unread_chains`, default empty). `FeedView.unread_chains` (within the chain filter) and `unread_key`: `home.activityUnreadOne` ("Activity can't be read on {{network}} right now.") or `home.activityUnreadMany` (`{{n}}`), 15 locales (+2 compiled leaves: 2289 paths). `settled` — "No activity yet" may be said — now also waits for every network. Every shell draws the line over the rows when there are rows, and in the empty state's place (never the skeleton, never "No activity yet") when there are none, on the home and in History. |
| **6. "Updated …"** | One core answer: `BalanceView.updated_at_ms` is the last round's stamp only once the account's first read has ended and every network said for it was read — none refused, silent (the pool's word) or rate limited, nothing failed inside Vela, not "nothing read at all". Every shell draws "Updated <ago>" from it alone; otherwise the control is named for what it does ("Refresh balance"). `last_refreshed_at_ms` keeps its other uses (a settle's mark, the widget). **The issue-462 e2e (`home-truth.e2e.ts:292`, webkit 1 in 3 on the full gates, firefox on the core lane):** busy and its words are one value — `aria-busy` and the shown label both follow `spinning` (`view.refreshing` or the press's 650 ms hold) on one element — and a probe sampling both every 50 ms on chromium, firefox and webkit never saw "busy" beside "Updated" (busy + "Updating…" until 650–700 ms, then at rest + "Updated now"). The stubbed read answers at once, so "Updating…" stood only for the hold; an `aria-busy` assertion met late in that window passed, and the label assertion after it met the control at rest. The spec now holds the read open until it has looked (the route waits on the test), reads busy and the label in ONE evaluate, checks they still agree a second later (past the hold, while the read is out), then releases the read. 5× per browser: 15/15. |

Tests: core `app_multisig_proposals.rs` (`a_plain_execute_over_no_figure_…`,
`failures_are_stable_kinds_never_english_text`), `app_multisig_proposals_reads.rs`
(`a_rate_limited_service_is_said_busy_on_the_sheet_too`),
`app_multisig_review_state.rs` (the `missing` and `failure` vectors through the
link door), `app_token_trust.rs` (`the_scan_names_the_networks_it_could_not_read`),
`app_activity_feed.rs` (`a_network_the_scan_could_not_read_is_said_and_never_claimed_empty`,
`the_activity_unread_keys_are_in_the_corpus`), `app_balance_dashboard.rs`
(`updated_is_one_core_answer_never_claimed_over_an_unread_network`). Web:
`review-door.test.ts` (vectors, busy words), `failure.test.ts`,
`live-activity-unread.test.ts`, `live.test.ts` (Updated), `feed-executor.test.ts`,
`token-trust-resident.test.ts`, `ProposalSheet.svelte.test.ts` (busy on the sheet,
the one press approves nothing). Android, iOS and desktop: their own unit tests
for each item (see the commits).

## 28. Test builds never write to production

Twice a test build wrote to a production service by default: real-device
pass 1's Android build recorded a genesis in the production multisig service
(fixed for that one service in ab5aad2a2), and Round 17's iOS dev-fixtures
simulator build, creating a throwaway passkey account, wrote it to the
production key index (`https://p256-index-v2.getvela.app`, "Writing the key
index") because nobody had pointed `passkeyIndexURL` at a local stand-in. The
owner's rule is absolute — no production writes during testing, local
stand-ins only — and a tester's checklist did not keep it. The build does now.

**One source of service defaults.** `vela_core::service_defaults` holds every
production service default: the key index, ethereum-data, the relay (bundler
and sponsor), fiat rates, the multisig service and its mailbox, the passkey
model directory, PeerConnect's relay, token and trial, the bug report and the
signing page. The names the core used before (`network_admin::DEFAULT_*`,
`multisig_shell::DEFAULT_GENESIS_SERVICE_URL`, `multisig_outside::relay::*`,
`trusted_signer::DEFAULT_SIGNER_URL`, `passkey::AAGUID_DIRECTORY_ORIGIN`) are
aliases of it. The shells keep no copy: Android and iOS read uniffi
`service_defaults()` (`CoreServiceDefaults`), the desktop reads the core by
path, and the web imports `serviceDefaults.ts`, generated from the core by
`generate_wallet_state_bindings` and drift-gated by `gen-core-types --check`
(`pnpm check`) — the web needs its defaults before the wasm can load.

**The rule** (`service_defaults::write_verdict`, uniffi
`production_write_refusal`, wasm `productionWriteVerdict`):

| | |
| --- | --- |
| Test build (`BuildFacts::is_test_build`) | dev-fixtures; Android's `devicetest` flavour (`app.getvela.wallet.devicetest`) or iOS's "Vela Test" build (`….devicetest`); any debug build (Android `BuildConfig.DEBUG`, iOS `DEBUG`, Rust `debug_assertions`); the web's dev server or the extension's development package (`extension/dist`, marked by `vela-test-build.json`, which its release and store packages are derived without). |
| Production host | `getvela.app` and every host under it, and every default's host (`relay.peerconnect.dev`, `peerconnect.dev`, the passkey directory's). A look-alike (`notgetvela.app`, `getvela.app.evil.test`) is not. |
| Write | Any method but GET, HEAD and OPTIONS — unless a POST is JSON-RPC (one call or a batch) asking only `READ_RPC_METHODS` (an estimate, a receipt, a gas quote). A submit, a sponsor request, a registration, a challenge, a token, a bug report, a REST body, an unknown method: a write. Allowlisted, so it fails closed. |
| Verdict | In a test build without the opt-in, a write to a production host is refused before it leaves the device; everything else is allowed. A release build is never refused. |
| Said | `common.testBuild.productionWriteRefused` (15 locales): "This test build won't write to the live service; point it at a local one." The refused request fails as never sent (the transports' `refused` / not-connected class, never "may have been sent"); the key index's clients say it as an answer, not as the network. Logged on every shell (`guard`). |
| Opt-in | `VELA_ALLOW_PRODUCTION_WRITES=1` — off by default. Android: `-PvelaAllowProductionWrites` (build) or the debug launch extra `vela.allowProductionWrites=1`; iOS and desktop: the launch environment; web: the variable when the dev server starts or the package is built, or `localStorage['vela.dev.allowProductionWrites'] = '1'`. A real-device pass keeps its production reads without it. |

**Where each shell enforces it.** Android: `ProductionWriteGuard`, an okhttp
interceptor on `VelaHttp.client` (every derived client inherits it) in a test
build only, and `RegistryClient`'s own connection asks it before it writes.
iOS: `ProductionWriteGuard`, a URL protocol registered with the loading system
and first in every `URLSession.vela(_:)` session — it takes a POST to a
production host, reads its body, asks the core, and fails it or sends it on;
it is in `Dev/`, excluded from Release (`EXCLUDED_SOURCE_FILE_NAMES`). Desktop:
a ureq middleware on every agent `proxy` builds (`production_guard.rs`), only
when `debug_assertions` or `dev-fixtures`; `proxy::walk` reports the refusal as
`local` (not sent). Web: `production-guard.ts` wraps `fetch` from the root
layout — only in the dev server or the extension build, and only active when
the extension's marker says it is the development package.

**Release builds are unaffected**, by construction and by test: the guard is
installed only in a test build (iOS: not even compiled into Release; desktop:
`guard_for(false, _)` is `None`; Android: `ProductionWriteGuard.installed` is
the core's `is_test_build`; web: not installed outside the dev server and the
extension, and the release packages carry no marker), and each shell's test
sends a release build's write to a production host name mapped to a local
stand-in and sees it arrive. `check-release-apk.sh`, `check-ios-store-binary.sh`
and `check-store-binary.sh` hold the release binaries to it.

**The harness.** `scripts/onchain/multisig-fork/up.sh` starts a local stand-in
for every service the wallet writes to — the genesis service and mailbox, the
relay, PeerConnect's relay, and now a key index
(`scripts/onchain/key-index/server.mjs`: the p256-index v2 protocol as the
clients use it, in memory, with the registry's honest member challenge) — and
writes `.state/harness.env` (`VELA_SERVICE_ENDPOINTS` with `passkeyIndexURL`,
`genesisServiceURL`, `bundlerServiceURL`; `VELA_RPC_OVERRIDES`;
`VELA_PEERCONNECT_RELAY`) and `.state/android-launch-args` for the shells. The
web's onchain harness (`e2e/onchain/harness.ts`) points `passkeyIndexURL` at it.

Vector: `rust/crates/vela-core/tests/vectors/rules/production-writes.json`
(defaults, build facts, opt-in values, and 30 requests), replayed by
`tests/production_writes.rs`, the uniffi bridge's test, Android
`ProductionWriteGuardTest`, iOS `ProductionWriteGuardTests`, the desktop's
`production_guard` tests and the web's `production-guard.test.ts` — each of
which also proves a test build's write refused before it leaves and a release
build's write delivered, with the production names answered on this machine.

## 29. Round 17 fixes (core, web, desktop)

Decided by best practice under the owner's rules (one core owns the rules and
every shell draws them; full parity; honest states — never claim emptiness or
freshness while a network is unread, no signable state appears transiently, a
press's failure is said beside the button in specific words). Each item was
written test first. The Android/iOS drawing items (r17-5 … r17-7, r17-12
Android, r17-14 adoption, r17-15 … r17-20) are the native lane's
(`r17-fix-native`); this lane's native edits only adopt the core where the
build needs it.

| Item | Cause | Decision |
| --- | --- | --- |
| **r17-1 (S3) "… anyway" right after a refused outage lifts** (r16-3 again, web and extension; desktop r17-8 once) | Not the core's re-ask (§25), which ran; the web shell's endpoint skipper (home-17, `createEndpointSkipper`) answered reads for a node that had just failed on transport with `not_connected`, without the wire. Pressed at once, the review's code read of the multisig's singleton was answered so: its state checks (trace, probe) were never BUILT, no check was "silent", the simulation answered (Arc does not simulate) — so `checks_final` took "unchecked" as final and the door opened "… anyway". | Two core rules. **(a) A local refusal is not the network's answer:** a state check that was never asked because the code it is built from went unanswered counts as a silent check (`multisig_gather`: the silent code reads are read again, the checks built and asked, within the same `RECHECK_MS` bound); only the network's own answer, or the bound, makes "unchecked" final. **(b) The person's ask is always sent:** `Io::person()` — the proposals machine's Try again (`review_again`, both kinds), its `retry`, and every press (Approve, Execute, "Approve and execute"), and the outside page's Try again — marks every read `MultisigOperation::Rpc.person`; the shell hands it to its pool as `rpc_pool::Event::CallRequested.person`, which lifts the chain's failure cooldowns, sweeps the WHOLE pool even while the chain is out of reach (never the outage's two-node probe), and marks each POST `RpcOperation::JsonRpcPost.must_send`. The web skipper never answers a `must_send` POST locally. Tests: core `a_check_never_asked_because_its_code_went_unanswered_is_not_final` (fails on the R16 gather: the review landed after two code reads, no trace or probe), `a_persons_own_ask_is_always_sent_past_the_outage_and_its_cooldowns`; web e2e `multisig-review-recovery.e2e.ts` — the DOM watcher run pressed at the instant a refused outage lifts, on the web and in the extension. |
| **r17-10 one Try again after a lift said "Couldn't reach Arc" again** | The same skipper: its window (1 s doubling to 30 s) still held the node 3–25 s after the lift; only a second press, after the window, read the network. | Rule (b) above: a person-initiated ask is always actually sent. The e2e asserts one press recovers — no "Couldn't reach Gnosis", no Try again left — without waiting for the app's own reads. |
| **r17-2 the no-figure stop kept 0.3022 while the fee line said 0.2325** | Not the core: `stops_follow_looks` serves the stop of a plain Execute over no figure (§27 item 1) with its look now (core test `a_no_figure_stop_follows_the_requote_and_the_next_press_signs_it`, passes). The web sheet drew the words it had computed at the press (`phase.reason`), never the machine's action again. | A press's failure on the web is the machine's action (`execute:<hash>` / `approve:<hash>`): its words are read from the view at every render (`failedReason`), so the stop follows the re-quote and goes when the core withdraws it — as desktop (r16-1), iOS and Android already draw `view.actions`. Test: `ProposalSheet.svelte.test.ts` "a fee stop follows the look the next press signs". |
| **r17-3 / r17-4 / r17-9 "No activity yet", a raw chain id, "Updated now" while unread** | (1) The feed's unread set was the scan's alone, never filtered by the space: Arc's leaked into Mainnet ("5042002" where the shell had no name). (2) `settled` was per account but not per space: a scan asked in the other space settled the new one. (3) The scan reads only where the account holds or lives (an empty testnet account: six networks, Arc is the eighth), so with Arc hung or 429 only the BALANCE knew Arc was unread — "No activity yet" was claimed over it. (4) `updated_at_ms` accepted a round the shell's cache answered (no network asked now). | **Core rules.** "No activity yet" is a claim about every network of the space, per account and per space: `FeedView.unread_chains` = the scan's unread ∪ the balance's unread networks of THIS account, filtered to the space in front (and the chain filter); `settled` also needs the balance's first read of this account over (`activity_feed::Event::BalanceRead {account, read, unread_chain_ids}`, the shell copies `BalanceView.networks_read` / `unread_chain_ids` — failed, silent or rate limited: a failure or a 429 is never a read); a space switch (`SpaceChainsChanged` to another set) unsettles the feed and scans at once, and a scan asked in the other space settles nothing (`space_epoch`). `FeedView.unread_network` names the one network (the built-in table's name); a shell never says a raw chain id — its own name, else the many-networks words. `BalanceView.updated_at_ms` needs a round the networks answered since the switch (`read_live`). Web and desktop hand the balance on (`BalanceRead`) and draw the core's `settled` / `unread_chains` alone (the desktop's own r16-2 reading of the balance view is gone). Tests: `no_activity_is_claimed_per_space_and_account_only_over_networks_really_read`, `updated_and_the_networks_read_are_claimed_only_from_a_live_read`; web `live-activity-unread.test.ts` (no raw id). |
| **r17-11 the To address broke anywhere beside "First time sending here"** | `PartyRow` drew the whole address (and an unnamed party's name, which is that address) with `overflow-wrap: anywhere` in a row shared with the chip. | `PartyRow` and `FactRow` draw values through `AddressText` (r16-21's halves, 22 + 20, each unbreakable); the chip wraps under the halves when both do not fit. Test: `PartyRow.svelte.test.ts` at 300–430 px. |
| **r17-12 (web) "Something went wrong" for a fee not priced at the press, or a refused relay** | The web's `sendCalls` classified only the passkey's and `UserOpNotSentError`; a relay that refused its estimate (`EstimateGasError`) or a pool failure before the passkey went out as `unknown`. | The desktop's r16-18 rule on the web: stopped before the passkey was asked, a read on a network the pool finds failing (`chainFailing`: failed or out of reach) is `chain_unreachable`; the relay refusing or not reached is `not_sent`; a failure inside the app stays `unknown`. The sheet words each kind from the catalog (`review.failure`). Test: `live-send-failure.test.ts`. Android parity is the native lane's. |
| **r17-13 / r17-14 fiat in the testnet space** | Desktop summed "Total $0.00" for the testnet space; Android/iOS priced testnet Send amounts and fees in CNY. The rule (spec 105) lived in each shell. | **One core rule:** `network_space::shows_fiat(space)` (and `priced_in`, `shows_fiat_str`): the testnet space shows no fiat anywhere — no total, no worth on a row, no fiat beside a Send amount or a fee. The Send machine applies it itself: `SendDisplayContext.space` (default mainnet); in the testnet space no token is priced on the screen whatever the shell handed in (no fiat line, no ⇄ into fiat). Web passes the space; desktop applies it to its totals and fiat lines (below); Android/iOS adopt it (native lane). Tests: `the_testnet_space_prices_nothing_on_send`, `the_testnet_space_shows_no_fiat_anywhere`. |
| **r17-21 wording drift** | "Check again" (the one press) vs "Retry" (plain Execute); "Retry" on the home's busy note vs "Try again" on the sheet. | **One wording per action, in the catalog:** re-asking a read that failed — the busy or unreachable service, an unread review, a failed list or history, a Safe-contracts check — is **"Try again"** (`home.multisig.service.retry` now carries `home.multisig.review.state.tryAgain`'s words in every locale: en, es-MX and pt-BR changed; the others already matched); a new look of an execution — a fee or check stop, whether of the one press or a plain Execute — is **"Check again"** (`home.multisig.proposal.feeRetry`). For the native lane: Android's plain Execute stop must draw `proposal.feeRetry` (not `service.retry`); iOS likewise. |
| **Outside page: no Reject on deep4 / delegate** | `OutsideTxView::can_reject` required `open()` — the gate of the transaction itself — so a transaction Vela refuses to sign (`Blocked`) could not be rejected there, where the member's sheet offers Reject. | Parity: Reject is offered on a read review whose gate is Open, Risk or **Blocked** (a rejection is its own transaction, the same number's cancellation, reviewed and gated on its own before the wallet is asked); never while the review is unread, never for a multisig whose own setup Vela refuses (`Refused`), never once done. The connect page draws `canReject` as before. The trusted signing page is unchanged (no rebuild, `BUILD_ALLOWED` untouched). Test: `a_transaction_the_core_does_not_sign_is_blocked` now rejects it. |
| **r17-8 (desktop, once) "仍然批准" enabled under a refused Arc** | Not reproduced; the desktop's sign buttons already open only through the core's door. | Covered by r17-1's two core rules (a check never asked is not final; the press's and Try again's reads are always sent — the desktop threads `MultisigOperation::Rpc.person` into its pool's `CallRequested.person`). Desktop test: a kept "… anyway" review opens nothing while the review is loading, failed or not asked, and a shut press is never labelled "… anyway". |
| **Desktop Settings: "couldn't check the Safe contracts" until Check again** | A `CheckOn` asked while Arc was unread landed no verdict; nothing asked it again but the person. | The desktop asks every unverified network again by itself: at once when the pool no longer lists it out of reach (`pool::out_of_reach_now`), else on a back-off of 3 s doubling to 30 s while Settings is open. The row's button is the person's Try again (`CheckOn.person = true`, a new core field: its reads always sent); the automatic re-asks say `false`. Its words are "Try again" (r17-21). Tests: `an_unverified_network_is_asked_again_once_it_answers`, `the_recheck_back_off_is_bounded`; core `a_network_is_checked_on_demand_once_a_session` now also checks the person's flag. |

**For the native lane (Android, iOS) — adopt:**

- Pass `MultisigOperation::Rpc.person` into the pool's `CallRequested.person`
  (a person's Try again / press is always sent; the natives have no local skip
  window, but the outage's two-node probe and the cooldown order apply).
- Dispatch `activity_feed::Event::BalanceRead { account, read:
  BalanceView.networks_read, unread_chain_ids: BalanceView.unread_chain_ids }`
  to the feed whenever the balance view moves; draw the unread line with
  `FeedView.unread_network`, else the shell's own name, never a raw chain id
  (no name: `home.activityUnreadMany`). This is r17-3 / r17-4 on Android and
  iOS, and r17-5 (the line over existing rows) draws from the same fields.
- Send: set `SendDisplayContext.space`; and apply `network_space::shows_fiat`
  to every fiat the shell draws itself (fee lines, totals, rows) — r17-14.
- Wording (r17-21): a stop's way on under the one press OR a plain Execute is
  `home.multisig.proposal.feeRetry` ("Check again"); every re-ask of a failed
  read (home busy note, sheet, Settings backup check, unverified network) is
  "Try again" (`home.multisig.service.retry` now says it).
- `CheckOn.person` for the person's Try again on an unverified network.
- r17-12 on Android: the desktop/web kinds for a stop before the passkey
  (`chain_unreachable` on a failing network, `not_sent` for the relay).

Tests (this lane): core `app_rpc_pool.rs`
(`a_persons_own_ask_is_always_sent_past_the_outage_and_its_cooldowns`),
`app_multisig_review_state.rs`
(`a_check_never_asked_because_its_code_went_unanswered_is_not_final`),
`app_multisig_proposals.rs`
(`a_no_figure_stop_follows_the_requote_and_the_next_press_signs_it`),
`app_activity_feed.rs`
(`no_activity_is_claimed_per_space_and_account_only_over_networks_really_read`),
`app_balance_dashboard.rs`
(`updated_and_the_networks_read_are_claimed_only_from_a_live_read`),
`app_send.rs` (`the_testnet_space_prices_nothing_on_send`), `network_space`
(`the_testnet_space_shows_no_fiat_anywhere`), `app_multisig_outside.rs` (the
blocked transaction rejected), `app_multisig_accounts.rs` (CheckOn's person);
web `rpc-pool-executor.test.ts` (`must_send` past the skipper),
`live-send-failure.test.ts`, `ProposalSheet.svelte.test.ts` (r17-2),
`PartyRow.svelte.test.ts` (r17-11), `live-activity-unread.test.ts` (r17-4),
e2e `multisig-review-recovery.e2e.ts` (pressed at the lift, web and
extension); desktop as above.

Left open:

- One unread network nobody can name reads "Activity can't be read on 1
  networks" (the corpus has no singular for an unnamed one); built-in networks
  and the person's own are always named.
- The onchain web suite (fresh harness, 97xx ports, every `*.getvela.app`
  host resolved to nowhere): 11 passed, 1 flaky (the rename), 2 failed in the
  suite's order ("two devices…": "Another transaction at #0 ran first"; "an
  outside owner…": no outside link) — both pass alone on a fresh harness
  (2/2). The core's `multisig_fork` test passes on its own harness. The
  ordering failure is not diagnosed further here.
- The web skipper still answers a background read locally during its window
  (that is its purpose, home-17); only the person's asks pass it. Moving the
  skip window into the core pool (one rule for every shell) is a later
  refactor.

## 30. Round 17 native fixes

Round 17's Android (r17-12, -15, -16) and iOS (r17-6, -7, -17 … -20)
findings, and the shell side of the core lane's r17-5, r17-14 and r17-21.
Decided by best practice under the owner's rules: one core owns the rules and
the shells draw it; never a raw key or English error text; never a wrong
screen flashed; a status chip never contradicts the body. Each item was
written test first.

| Item | Cause | Decision |
| --- | --- | --- |
| **r17-6 (S3) iOS: "1/2 approved" over "Executing…"** (r16-8 not fixed) | r16-8 said "Executing…" only while `!isDone`. The mailbox marks the proposal `done` as soon as its nonce is used, ~1.2 s before the flight's receipt is judged; the body drew the execution verdict `waiting` ("Executing…") while the chip fell back to the standing ("1/2 approved"). | The chip says "Executing…" whenever the body would: this device's press is out, the core follows its flight, **or the flight's verdict is `waiting`** — checked before the done branch. The body no longer draws its own "Executing…" under the chip (said once). Android parity: the same rule before its `isDone` branch (its chip was empty there). |
| **r17-7 (S3) iOS: Welcome flashed at launch** (r16-11 not fixed) | `signedInOrWelcome` drew the wallet for `allowedRoute == .wallet` and **Welcome for everything else — `loading` included**. The session's first view is `booting` (`loading`) until the account store is read; the weekly launch lockup covered that frame, every other cold start drew Welcome (live buttons) for 0.3–1 s. | `RootScreen.of(route)`: `wallet` → the wallet, `onboarding` (the store read and empty) → Welcome, `loading` → the page's own ground (nothing of any screen). Launch-sequence tests drive the real session machine over a real store and sample every route it publishes: with an account the root never draws Welcome (ground → wallet); with none, ground → Welcome. |
| **r17-17 (S4) iOS: raw text on a damaged link** | The link stand sheet drew the core's `text` (what the damaged link held, "5042002.0x2746…") as the sentence, and a failed lookup's words were `add.<kind>` for any kind — `add.refused`, `add.highRisk`, `add.unverified` are not in the corpus, so their keys were drawn. | A damaged link says `proposal.badLink` (an invite: `add.badChecksum` / `add.badInvite`), never what it held. Every `AddOutcome` kind is mapped (`refused` → `authority.refusedTitle`, `highRisk` → `authority.riskTitle`, `unverified` → `authority.unverified`). **Guard:** `Loc` records every key it answers with itself (the FR-005 fallback, DEBUG); tests read it, and a test scans every literal key in the multisig views against the corpus and fails on any key built from a value outside a closed list. Android: the same mapping and a JVM missing-key guard. |
| **r17-18 (S4) iOS: a shut "Approve and execute" on an unread review** (r16-14's twin) | iOS drew the one press, its fee line ("Couldn't check whether it would run — Execute checks again.") and Check again whatever the door said. | Android's §26 rule (`saysExecution` = the door is open): while the review is unread the button reads **Approve**, held still; no fee line, setup, Check again or "why not the one press"; the review's reason with Try again is the one note. Execute's fee and setup lines follow the same rule. |
| **r17-19 (S4) a refused create record said as "can't reach the service"** | **Core.** `record()` read the service's 422 refusal and its error answers as `Ok(false)`, so `create.refused` stayed `None` and every shell drew "The multisig service can't be reached right now". | **Core (minimal):** an answer that did not record it (`Refused`, `Error`) is `create.refused = "refused"` — a stable word, never the service's text; a service that did not answer is unchanged (`None`). Web, desktop and iOS already say `settings.shareAgainNote` for `refused`; Android adopts it. Tests: `created_and_refused_by_the_service_stays_kept` (now asserts it), `an_error_answer_to_the_record_is_said_as_refused_never_unreachable` (both fail on the R16 core). |
| **r17-20 (S4) iOS: four smaller items** | (a) The catalog's count carries the separator before the total ("1 account · "); with no total (testnet, a figure not known) it dangled. (b) The create sheet's foot sat above the keyboard's own bar (the floating "Done"), whose ground is transparent: body fields showed between Next and the keys. (c) The open path's Try again sent `retry` and drew nothing until the machine's next read changed the line (~3 s). (d) The member challenge to the passkey index had only idle timeouts (15 s between bytes); an interrupted write held ~90 s, then the core's `key_kept` sheet was drawn as the generic error with the index's raw detail — or not noticed. | (a) `SettingsLive.accountsSummary`: the count alone drops its trailing separator. (b) The foot's ground runs down behind the keyboard's bar (`ignoresSafeArea(.all, edges: .bottom)`): attached, no gap. (c) `MsRetryButton`: busy the moment it is pressed, until what is said changes (or 15 s) — the open path's three Try agains and the link's. (d) The challenge is bounded by the clock (`OnboardingExecutor.bounded`, 20 s; past it, the index's failure); `PromptKind.key_kept` is read and says `keyKeptTitle` with `keyKeptBodyIndex` / `keyKeptBodyNetwork` — the key is kept and its row's Confirm is the Try again. |
| **r17-14 (S4) Testnet Send priced in CNY** | Send's surfaces and the signing sheet drew fiat from the display currency whatever the space. | iOS: `Display.inSpace(testnet:)` withholds every fiat figure in the testnet space (Send's form, confirm and fee, the signing sheet's fee), and the send machine is told no rate (`sendContext(testnet:)`), so it prices nothing and ⇄ never enters fiat; told again when the space changes. Android: `Money.withoutValue()` in the testnet space — no fiat line and no room for one, Send's rows without worth, no ⇄ fiat toggle, the signing sheet's money too. A core rule (the space prices nothing) is the core lane's; the shells' rule then becomes its adoption. |
| **r17-21 wording** | "Retry" (`service.retry`) on the home's busy note and Settings' backup check beside "Try again" on the sheet; the core lane's §29 was not available. | Every retry of a failed read says **Try again** (`review.state.tryAgain`); **Check again** (`proposal.feeRetry`) only re-prices a fee. If the core lane's §29 decides otherwise, its words win and the shells follow its keys. |
| **r17-5 iOS: Activity's unread line over rows** | The shell already draws the core's `FeedView.unread_key` over rows on the home and in History (round 16 leftover 5; covered by `R16LeftoversTests`). The missing line traced to the scan's unread set, which the core lane is reworking per space (r17-3/r17-4); not reproduced in the simulated world. | No shell change; the iOS tests keep the line over rows on both surfaces. Left open below for a device re-run on the core lane's rule. |
| **r17-15 (S4) Android: zh listed 20 Mainnet networks, en 24** | Not the locale. Stable, MegaETH, Plume and XRPL EVM are shown only on the person's word (`shown_networks`, owner decision 2026-10-29). Android settled `vela.shownNetworks` only when its network store was read at start (`NetEvent.Started`) and only once `vela.heldChainIds` existed — written after the first balance round. A fresh device's first run read the store before any round: no record, so every network was shown (24); the record settled on the NEXT launch (20). The tester's zh run came after a relaunch. iOS settles at its first network-store load, so its list is the same on every launch and in every locale (no iOS change). | Android settles in the session the holdings are first read: `WalletController` notes `heldChainIds` for the first time → `settings.startNetworks()`, the core re-reads and settles. The list is the core's record, identical in every locale (JVM test over every shipped locale). |
| **r17-16 (S4) Android: a stale failure beside 执行; a refused approval never worded** | (1) The one press's stop stayed recorded on `execute:<hash>` after the person approved alone. (2) Android matched the outbox entry by `endsWith(hash)`, but the core's key is `chainId:safe:hash:owner` — never found, so only "Send again" (重新发送) showed. | (1) **Core (minimal):** a successful Approve drops a settled failure or stop on `execute:<hash>` (one working or done is left alone) — the mirror of the one press dropping `approve:`; every shell follows. Android also keeps a press-order guard (a failure older than a later press that went through is not drawn), now a no-op guard. Test `an_approval_that_went_through_ends_the_one_press_stop_before_it` (fails on the R16 core). (2) The entry is matched by the core's key; an unsent approval is always worded (its state, else `outboxNotSent`); Send again only when nothing is on its way and no retry is due (the iOS and web rule). |
| **r17-12 (S4) Android: "Something went wrong, so nothing was sent."** | The multisig shell mapped every submit refusal it did not name to `Other`, so a relay that refused, or a fee the press could not price, fell to the generic line. | The submit refusal is mapped by kind as on desktop: `NotSent` / `RelayerUnavailable` → `not_sent`; a chain read nobody answered (or on a network the pool finds failing) before the passkey → `chain_unreachable {chainId}`; the spine's gas estimate the relay did not answer is `RelayerUnavailable` (Send's own flow then says its relay words). Never the generic line for a named cause. |
| **r17-4 (shell guard, Android)** | The home named the feed's unread network from the space's own list, so another space's chain (the core lane's leak) printed as "5042002". | A network is named from every network the wallet knows (`allNetworks`), never a raw id; the leak itself is the core lane's. |

Tests:

- **Core:** `app_multisig_accounts.rs` (`created_and_refused_by_the_service_stays_kept` now asserts `refused`, `an_error_answer_to_the_record_is_said_as_refused_never_unreachable`), `app_multisig_proposals.rs` (`an_approval_that_went_through_ends_the_one_press_stop_before_it`); each fails on the R16 core.
- **iOS:** `MultisigR17NativeTests` (16: the chip over a done-but-waiting flight; the root's screen per route and two launch sequences through the real session machine; the damaged link in en/zh, every `AddOutcome` kind, the missing-key guard and the corpus scan; the shut door; the refused record; the count alone; the foot; the busy Try again; the bounded index and the kept key; testnet Send; one word per action; hosted shots of the link sheet and the switcher). `SwitcherCountTests`, `FiatWithholdTests` and `MultisigR16NativeTests` pinned the dangling separator and now expect the count alone.
- **Android:** `R17NativeFixesTest` (JVM; five of its cases fail with the old rules restored, the rest assert new functions) and `R17NativeDeviceTest` (device: the relay that never took it, a stale one-press failure, a kept approval, testnet Send).

Left open:

- r17-5 was not reproduced: the shells draw the core's unread line over rows (tested); whether the scan's unread set names a refused Arc within 100 s is the core lane's per-space rework (r17-3/r17-4). Wants a device re-run on its rule.
- r17-20 (d): only the creation-time member challenge is bounded; the publish's task poll keeps its own 120 s budget (the index accepted the task; its failure is the sync-failed screen, which is said). RegistryClient was left to the `no-prod-writes` lane.
- `home.balanceDetailRetry` ("Retry now", the hero's stalled sync) is a read's retry outside the multisig surfaces; aligning it with "Try again" is for the core lane's §29 wording table, on every shell at once.
- A refused create record keeps its proof for the background retry, so Settings says "not shared" with the share-again note, as before.

## 31. Parent-Safe guard

Reported 2026-09-29 (disclosure package, `APPENDIX-PARENT.md`); fixed here
(P0, treated as S1). The self-call guard (spec 081, `self_call_guard.rs`)
refused the Safe-admin selectors only when the call's `to` was the signing
account itself. A Vela account is also an owner of every Vela multisig it is a
member of — its **parent** Safes — and a connected site could make the account
act for one of those without ever pointing at the account:

- **(a)** `parent.execTransaction(… swapOwner / addOwnerWithThreshold /
  changeThreshold / enableModule / setGuard / setFallbackHandler …, sig {r =
  account, v = 1})` — the "approved by `msg.sender`" signature. For a 1-of-N
  parent: an immediate takeover (proved on the fork, below).
- **(b)** `parent.approveHash(h)` — not listed anywhere: one counting approval.
- **(c)** EIP-712 `SafeMessage` (or `SafeTx`) under the parent's domain
  (`verifyingContract` = the parent). The wallet wraps it in the account's own
  `SafeMessage`, which is exactly a nested EIP-1271 approval the parent counts
  as this owner's. The sheet offered only a blind "Sign anyway".

The appendix's own PoC — a zero-amount Permit2 message at `params[0]` and the
parent's `SafeTx` at `params[1]` — is already refused -32602 by the
one-document reader (WYSIWYS audit 2026-10-01); the parent's `SafeTx` alone is
now refused in the parent's words.

**Decision (one core rule, every surface).** A request that reaches the dApp
signing machine (`sign_request`), or a site's request to a multisig
(proposals `DappRequest`), is refused — with no "anyway" — when any of these
targets **a Safe this account owns**: `execTransaction` (any inner call),
`approveHash`, any owner / threshold / module / guard / fallback-handler /
setup call (the self-call list), any of those inside a MultiSend /
MultiSendCallOnly or `execTransaction` payload (recursively, depth 4), or typed
data with `primaryType` `SafeTx` / `SafeMessage` whose domain's
`verifyingContract` is that Safe (every document of the request is read).

"A Safe this account owns" is:

1. any multisig on this device's list (`Event::MultisigsChanged`, every
   multisig any local account is a member of — no site has a reason to drive
   one, whoever its members are), refused at arrival with no read; or
2. any other target with one of those shapes: the core asks
   `SignOperation::ReadSafeOwner` — `eth_call isOwner(account)` on the
   target, on the request's chain (the typed data's own chain for a
   signature) — and keeps the confirm shut (`ConfirmBlock::Reading`) until it
   is answered. `true` refuses (`owner`). A clean `false`, an empty answer (no
   code) or a revert (no `isOwner`: not a Safe) clears it. Anything else — no
   node answered, a non-revert error, an answer that is not one ABI `bool` —
   refuses "couldn't check" (`unchecked`), never allows.

The submit chokepoint (`proceed_submit`) checks the FINAL params again
(`parent_safe_guard::enforce`): a target on the list, a read that said owner or
could not be read, or a target nobody read (a rewrite after arrival) refuses.

**Vela's own multisig flows are not dApp requests.** Approve, execute,
propose and the owner proof run in the multisig machines: the approval is a
`passkey` operation with purpose `approve` (the page's
`vela_multisigApprove`), the execution a `send_calls` with purpose `execute`
(the page's `vela_multisigExecute`, read by `lib/multisig.js` `execution`),
the proof a `passkey` with purpose `record` (`vela_multisigCreate`). None of
them enters `sign_request` or the multisig `DappRequest`, so none passes this
rule; the distinction is the path a request enters by, decided in the core,
never a field a site can write. The wallet's own first-party requests (the
passkey-registry backup, the Ethereum record call) are held to the rule all
the same — they never target a Safe, so nothing of Vela's is refused.

**Words.** `componentsUi.signing.parentSafeBlocked` — "This site asked your
account to change or approve something for a multisig you're a member of
({{function}} on {{safe}}). Vela never lets a site do that — use the
multisig's own screen. Nothing was signed." — and `parentSafeUnchecked` for a
read that could not be made; fifteen locales, zh included. Title:
`selfCallBlockedTitle`. The site is answered `-32603`
(`SignErrorKind::ParentSafeBlocked`, the call and the Safe in the detail) when
the person closes the sheet — never 4001. A multisig's site request is held
`HeldStage::Refused` with `multisig_dapp::parent_refusal` (`data.reason =
"parent_safe"`, `data.key` the sentence, `data.why`, `data.safe`,
`data.function`).

**Wire** (`sign_request`, generated TS in `src/lib/core/generated`):

| | |
| --- | --- |
| Event | `multisigs_changed {addresses}` — at boot beside `accounts_changed`, and whenever the list changes |
| Operation | `read_safe_owner {id, chain_id, safe, account, data}` — `eth_call [{to: safe, data}, "latest"]` on `chain_id` through the shell's RPC pool, bounded; the shell decides nothing |
| Result | `safe_owner_read {id, chain_id, safe, answer}`, `answer` = `returned {data}` / `rpc_error {code, message}` / `unreachable {detail}` |
| View | `SignBlockedView.parent` (the Safe) and `.reason` (`member` / `owner` / `unchecked`); `confirm_block` `reading` while a read is out, `refused` once refused |
| Error kind | `parent_safe_blocked` (-32603) |

**The page.** `resolve.js` refuses the same shapes independently
(`refuse.parentSafe` with the function, `refuse.parentSafeMessage`), in every
leg of the operation the app assembled. It cannot read the chain (no
`connect-src`) and does not take the app's word for which Safes are the
account's, so it refuses them on ANY Safe but the account — stricter than the
core, which clears a Safe the account does not own. Vela's own multisig
execution (no origin, read whole by `lib/multisig.js`) is skipped exactly as
the takeover rule skips it. Page **2c649b76** is first in `BUILD_ALLOWED`;
`LAUNCH` is unchanged; it reaches users once `dist/` is deployed.

**Every surface.**

| Surface | Site entry points | Ownership read | List fed | Words |
| --- | --- | --- | --- | --- |
| Core | `sign_request` (arrival + submit), proposals `DappRequest` (its own `rpc`) | — | — | catalog keys |
| Web wallet, Chrome extension, Safari package | the web wallet's requester, `DappRequestHost` (panel, request window); the Safari package is the same code | `sign-executor.ts`, `poolRpcCall`, 8 s | `signRequest.syncMultisigs()` at boot, on change, before each request | `live.ts`; a multisig's refusal by `dapp/multisig-refusal.ts` |
| Android | in-app browser → a Vela account (`SigningController`) or a multisig (proposals); the Ethereum backup shares the controller | `SignExecutor.kt`, the RPC pool, 15 s backstop | `VelaWalletApplication.multisigAddresses`, after `accounts_changed` and on change | `SigningLive.kt`, `MultisigWords.dappRefusal` |
| iOS | in-app browser → a Vela account (`SigningController.Incoming(browser:)`) or a multisig (`MultisigHub.dappRequest`); the Safari Web Extension is the web package | `SignExecutor.swift`, the RPC pool, 16 s cap | `MultisigHub.multisigAddresses`, after `accounts_changed`; `RootView` on change | `SigningLive.swift`, `MultisigDappSheet` |
| Desktop | in-app browser → a Vela account (`signing_host`) or a multisig (proposals) | `executor/sign_request.rs`, `pool::call_within`, 8 s | with the accounts; again on change while a sheet is up | `signing/mod.rs`, `wallet/page/multisig_dapp.rs` |
| Signing page | every request handed to it | cannot read: refuses the shapes on any Safe | — | `refuse.parentSafe`, `refuse.parentSafeMessage` |

WalletConnect: no shell takes a site's request over it (Android and iOS
refuse a `wc:` link as unsupported); PeerConnect carries only a multisig's
outside-owner session, Vela asking a wallet — never a site asking Vela.

**Tests.**

- Core: `tests/vectors/rules/parent-safe-guard.json` (39 cases: (a) and every
  admin selector, (b), (c) `SafeMessage` and `SafeTx`, the appendix's
  two-document PoC, MultiSend-nested and batch legs, unknown Safe with isOwner
  true / false / reverted / empty / unreadable / junk, the submit form, a
  read on the typed data's own chain, clear cases) replayed by
  `tests/app_parent_safe_guard.rs` (and the machine: refused at arrival, after
  the read, at submit; a late list; first-party requests held to it; the
  answer) and the uniffi bridge; `tests/app_multisig_dapp.rs` §31 (a
  multisig's site request: list, isOwner true, false, unreadable).
- Fork (`tests/multisig_fork.rs`
  `parent::parent_safe_takeover_is_refused_while_the_multisigs_own_flow_runs`):
  a 1-of-2 multisig P of A and B on the Gnosis fork; Vela proposes, A approves
  and executes a transfer (lands); the site's `execTransaction(swapOwner)` with
  `v = 1` sent from A's address hands P over (proved on a snapshot, reverted);
  through the sign machine the same request is refused — after the real isOwner
  read (owner), from the list (member), `approveHash`, `SafeMessage` — and a
  node that does not answer refuses `unchecked`.
- Page: `samples/takeover-test.mjs` replays the core vector (every case with a
  target refused, none without) plus direct cases; 67/67.
- Shells: web `signing/core/parent-safe-guard.test.ts`, `dapp/multisig-refusal.test.ts`,
  `multisig/shell/dapp-requests.test.ts`, the Safari package test and
  `e2e/parent-safe-guard.e2e.ts` (web wallet and Chrome extension), wasm
  `verify-web.mjs` (39/39); Android `ParentSafeGuardVectorTest`,
  `ParentSafeEntryPointsTest`, instrumented `ParentSafeGuardDeviceTest`; iOS
  `ParentSafeGuardTests` (vector, read mapping, the browser path end to end)
  and `MultisigFlowTests`; desktop the vector, the executor's read over a
  loopback node, the browser entry end to end and the multisig scenarios.

**Left open.** The page is stricter than the core: a site's `execTransaction` /
`approveHash` / admin call on a Safe the account does NOT own (which the core
clears after the read) is still refused there, because the page cannot read the
chain. A multisig connected to a site is held to the parent rule but not to
the self-call list for its own address (members review every proposal; out of
this lane). The ownership read trusts the RPC answer for an unknown Safe (the
list does not depend on it); a Safe on another chain is read on the request's
chain only.

## 32. Round 19 fixes (core, web, desktop, Safari)

The last fix round before the R20 release-candidate acceptance; scope frozen
to the Round 18 report's items below. Decided by best practice under the
owner's rules: one Rust core owns the rules and every shell renders them;
full parity; honest states — never sign a figure other than the one on screen
at the passkey, never draw a signable or "… anyway" frame the core will
refuse, never show raw debug text, never word a refusal as a network failure.
Each item was written test first. The native lane (`r19-fix-native`) owns
r18-1 … r18-4, r18-9 on Android/iOS, r18-18 and the Android/iOS adoption of
the rules below; this lane's native edits are what the build needs plus the
§28 refusal's mapping (item r18-8).

| Item | Cause | Decision |
| --- | --- | --- |
| **r18-5 (S3) the fee signed must be the fee on screen at signing** (iOS: 0.248201 at the press, re-quoted on screen to 0.203525 before Face ID, 0.248201775 charged) | Not the press: it signed its fresh look (0.248201775), as §19.11 / §24 rule. The shell's 5 s re-ask (and the home's poll) kept asking `Preview` while the press was out; past `PREVIEW_FRESH_MS` `look_if_stale` took a new look and the view served it — the line moved under the passkey to a figure nobody was signing. | **Core rule (`multisig_proposals`), chosen over §25's "stops follow looks" for the press itself:** from the press (Execute, "Approve and execute") to its signature, cancel or stop, the pressed executor's look is **frozen**: `State.pressing` holds its `preview_key`, `look_if_stale` takes no new look for it, and the view serves it with `"signing": true` (`pressed_looks`). Every shell draws the frozen figure as it already draws `previews`, and marks it `home.multisig.proposal.feeSigning` ("Signing at this fee — it stays fixed until you confirm or cancel.", 15 locales). The press's own re-quote (its look older than `PREVIEW_FRESH_MS` at the press) is the only new figure a press can meet; one other than the pressed figure stops with `fee_moved`, nothing signed, the line then the new figure, unmarked (§19.11, §27 item 1, unchanged). §25's reason for rejecting a freeze (a frozen look ages and the next press re-quotes) does not apply: the freeze lasts only while the press is out, and the press has already decided what it signs. Vector `tests/vectors/multisig/press-fee.json` (5 cases: Execute and the one press re-asked under the passkey, a cancel, the press's own re-quote moved and unmoved); machine test `the_fee_on_screen_at_the_passkey_is_the_fee_signed` (fails on the R18 core: the line said 512 while 256 was signed). |
| **r18-8 (S3) §28 refusals worded as network failures, and retried** | Every shell's guard failed the refused write with the transport's own shape (`http_unreachable`, `index_failed {network}`), so the core could not tell it from an outage: the genesis service's `request` retried it three times and marked the service away ("The multisig service can't be reached right now…"), the record was asked again on every tick, and the key index's challenge was said as "didn't answer" / "offline". | **A stable kind in the core.** The wire's transport failures carry `refused` (`MultisigShellResult::HttpUnreachable.refused`, onboarding `ShellResult::IndexFailed.refused`, `SessionShellResult::RegistryLandingUnconfirmed.refused`; optional, `true` only when the shell's §28 guard refused the request before it left). The genesis service answers `ServiceFail::Refused` at once — one ask, no retry, no outage — and remembers it for the session (`Service::write_refused`): the background never writes to it again (the unrecorded-record loop, the outbox), only the person's own press does. Said: a created multisig's `create.refused = "production_write_refused"` (`multisig_accounts::PRODUCTION_WRITE_REFUSED`); a share-again's failure `{"kind":"production_write_refused"}` → `FailureKind::ProductionWriteRefused`, key `common.testBuild.productionWriteRefused` (`multisig_failure`, vector `review-state.json` `failure`); an outbox entry `why = "production_write_refused"` (`multisig_proposals::OUTBOX_REFUSED`, `retry_at: null` — Send again only); the key index's member challenge keeps the key with `PromptKind::key_kept_refused` (`words_key` = the sentence, `local: false`); a refused publish's retry screen `CreateView.sync_error_key`; the session's pending publish is not retried by `SyncPending` once refused. The outside page's post stops `not_sent` at once. Never "didn't answer", "can't be reached" or "offline", never retried by itself. Tests: `a_record_the_test_build_refused_is_said_so_once_and_never_retried_by_itself` (one POST, no outage, no tick retry, Share again once in the test build's words), `a_challenge_the_test_build_refused_is_said_as_that_never_the_index`, `a_publish_the_test_build_refused_says_so_on_the_retry_screen`, `a_publish_the_test_build_refused_is_not_tried_again_by_itself`. |
| **r18-10 (S4) a SafeTx on an unlisted Safe the account owns refused in the generic self-call words** | `on_request_arrived` matched the parent verdict first only when it already refused (a listed multisig); for `Read` (an unlisted Safe, isOwner still to be asked) it fell through to `detect_self_call`, which refuses every `SafeTx` whatever its domain — at once, in `self_call_blocked`'s words, before the read that would have said "a multisig you're a member of". | The parent rule decides first: a `SafeTx` whose domain is a Safe being read is held (`Pending.deferred_block`) while the read is out (confirm shut, `Reading`); owned (or unreadable) → the parent's words (`parentSafeBlocked` / `parentSafeUnchecked`, -32603 `parent_safe_blocked`); not owned → still refused, in the self-call words (a SafeTx is never signed). Vector cases `r18-10: unknown Safe, SafeTx, …` (isOwner true, before the read); tests `a_safe_tx_for_an_unlisted_safe_this_account_owns_is_refused_in_the_parents_words`, `a_safe_tx_for_a_safe_this_account_does_not_own_is_still_refused`. |
| **r18-14 (S4) a plain Send with the relay refused: "Could not build a valid transaction estimate"** | `SendEstimateFailure::body_key` gave the generic sentence to everything but a chain read and an internal fault, though the fee row already names `QuoteUnavailable` / `EstimateFailed` as the relay's ("Couldn't get a price from the relay", "The relay couldn't test-run this transaction"). | Continue's alert says the relay's kind: `QuoteUnavailable` and `EstimateFailed` → `send.alertEstimateRelayBody` ("Vela's relay didn't answer, so nothing was sent. Try again in a moment.", 15 locales; `send::ESTIMATE_RELAY_BODY_KEY`). Every shell already words the alert through the core's `body_key`. Test: `every_fee_failure_passes_through_and_is_worded_by_its_cause`. |
| **r18-6 (S3, Safari Web Extension) a shut "Sign anyway" frame ~200 ms before the parent-Safe refusal** | Not Safari's own code (its package is the web's sheet): the web's `buildSigningModel` labelled the confirm from the clear-signing view alone (`clear.sign_anyway` for a blind `SafeMessage`, red), while the core said `confirm_block: reading` — the §31 isOwner read was out. Safari's slower read made the frame visible; the web wallet and the Chrome extension drew it too. | While `sign.confirm_block === 'reading'` the confirm is a shut **"Sign"** — no danger style, no "anyway" — with "Reading the request…" under it, as native iOS draws it (`confirmLabel(clear:)`). The core's view already said it; the shell now draws only that. Tests: `parent-safe-guard.test.ts` "while the read is out the confirm is a shut "Sign" — never "anyway", never red" (fails on R18: "Sign anyway"); e2e `parent-safe-guard.e2e.ts` frame watchers (MutationObserver + rAF, every frame from arrival to refusal, isOwner held 800 ms) on the web wallet and the Chrome extension's request window: no frame ever has a button matching /anyway/i, "Reading the request…" over a disabled "Sign" is seen. The Safari route UI suite was not run (it needs the simulator harness end to end; the package is the same code the e2e covers). |
| **r18-7 (S3, desktop) raw `Failed { rate_limited: false }` in Settings' members section** | `multisig/live.rs` `unanswered_detail` Debug-formatted the pool's error; the core keeps it as `NetworkState::Unreadable { detail }` (a diagnostic), and the desktop drew `detail` as prose. | The members section draws the core's words like the web and iOS: `home.multisig.settings.unreadableOn` with the network's name — never a detail. The pool's errors get fixed words (`PoolError: Display`); a page failure goes to the core as its wire word (it was Debug text, `"Unanswered"`, which the core's snake_case matcher read as a refusal). **Guard:** `debug_text_guard.rs` scans every non-test `format!` / `write!` / `writeln!` for a Debug placeholder; 20 allowlisted sites, each with its reason (log-only, a dedupe or cache key, a developer probe, a diagnostics field); a new unlisted site fails, and so does an allowlist entry that no longer matches. Test also `unreadable_members_are_said_in_the_cores_words_never_the_detail`. |
| **r18-9 (S4) fiat in the Testnet space** (web Send confirm, web dApp sheet, extension panel, Safari Send) | `network_space::shows_fiat` was applied by the Send machine to token worth and by the web's home/switcher, but every fee figure and every stablecoin's 1.0 price was formatted without it (`fee-line.ts` `feeMoney`, the signing sheet's fee and amount lines, asset/token/activity details, the unreachable and balance-detail sheets, the batch importer). | Every fiat the web draws passes the core's rule: wasm `networkSpaceShowsFiat` (`shows_fiat_str`), TS `network-space/fiat.ts` `showsFiat` / `pricedIn`; the signing host now passes the space into `SigningLiveInputs`. In the Testnet space no fee, amount, worth, total, "last seen" figure, fiat input or batch fiat tab is drawn. Test: `fiat-withheld.test.ts` "r18-9: every money surface in the testnet space names no currency — a stablecoin at 1, a relay fee priced" (each surface first proven to show fiat in Mainnet). Android and iOS are the native lane's. |
| **r18-13 (S3, desktop) "Approve and execute" under a refused Arc: no failure words** | Not a lost failure: the press stopped (`Stopped { unpriced \| unchecked }`, its look could not price or check it) and the desktop drew the stop with the fee line's own note ("Couldn't check whether it would run — you can still approve…") in the fee line's place — read as the fee note changing, not as the press failing. | The press's stop is said beside the press, as a failure: on a network the pool holds out of reach (`pool::out_of_reach_now`) `home.multisig.proposal.reason.chainUnreachable` ("Can't reach {{network}} right now, so nothing was sent."); otherwise "That didn't go through: {{reason}}" with `proposal.unpricedReason` / `uncheckedReason`. Check again stays its way on. (`out_of_reach_now`, not r16-18's `failed_chains ∪ unreached_chains`: those block on the pool thread and this is drawn every frame; a chain only rate-limited does not get the "can't reach" sentence.) Test: scenario `a_press_stopped_for_its_fee_says_why_beside_the_press` (one press and plain Execute, en and zh). |
| **r18-23 (S4, web, extension) the parent sentence's Safe in lowercase** | The core's `SignBlockedView.parent` is lowercase (an identity), and the web shortened it without a checksum (`$lib/wallet/identity` `shortenAddress`). | The web shows the EIP-55 form, shortened (`checksumAddress`), in the parent sentence and the multisig's refusal (`dapp/multisig-refusal.ts`), like iOS (`names.short`). The core's identity and the site's error detail stay lowercase. |

**Shell adoption of the core rules (this lane).**

| Rule | Web, Chrome extension, Safari package | Desktop | iOS / Android (minimal here) |
| --- | --- | --- | --- |
| r18-5 frozen line | `proposal.feeSigning` under the fee line while `signing` (`ProposalSheet.svelte`, `execution-words.ts`) | `fee_signing_words` under the fee line; scenario `the_fee_line_keeps_the_pressed_figure_and_says_so_while_the_press_is_out` | draw the frozen figure already (they draw `previews` as served); the mark is the native lane's |
| r18-8 refused | `isProductionWriteRefused` (by class or name — errors cross the extension's worker) → `refused: true` on the multisig shell, the key index and the session publish; the sentence on the created sheet, Share again, the outbox line, the kept-key sheet and the retry screen (which stops its own timer once refused) | the guard's refusal carried as `HttpFailure.refused` / `RegistryError` / session publish; the sentence on the created screen, Share again, the outbox line, the sync-failed screen; a kept key has its own outcome (`OutcomeKind::KeyKept`) with the core's `words_key` | `refused: true` from the multisig client, the key index and the session publish; the sentence on the kept-key sheet, the created multisig's refused record and the outbox line (`R19RefusedWriteTests`, `R19RefusedWriteTest`); `sync_error_key` is the native lane's |
| r18-14 relay words | `send.alertEstimateRelayBody` resolved (`flows/messages.ts`) | through `body_key` (test `a_relay_refused_continue_says_the_relays_sentence`) | through `body_key`; `CoreRoundTests` / `FeeFailureViewTest` updated |

**For the native lane (Android, iOS) — adopt:** the `feeSigning` mark under a fee line whose look is `signing` (r18-5); `CreateView.sync_error_key` on the retry screen; Share again's `production_write_refused` failure through `failure_said` (iOS already does); every fiat through `shows_fiat` (r18-9, theirs); the press-stop wording of r18-13 if their sheets also draw `Stopped { unpriced | unchecked }` only in the fee line's place.

Tests: core `app_multisig_proposals.rs` (`the_fee_on_screen_at_the_passkey_is_the_fee_signed`, vector `press-fee.json`), `app_multisig_accounts.rs`, `app_create_wallet.rs`, `app_session_landing_409.rs`, `app_parent_safe_guard.rs` (vector cases `r18-10: …`), `app_multisig_review_state.rs` (vector `review-state.json` `failure` gains `production_write_refused`), `app_send.rs`; web, desktop and native as named above.

Left open:

- A refused write is remembered per session (the genesis service, the session publish): a NEW launch's background asks once more, and the guard refuses it before it leaves — nothing is sent, nothing is said wrongly, but it is one automatic ask per launch.
- The Safari route UI suite (`safari-route-e2e.sh`) was not run for r18-6; the fix is in the shared web sheet, proven by the unit test and the web/extension frame watchers.
- The R18 S4s neither lane fixes are in docs/KNOWN-BUGS.md (R18-11, -12, -15, -16, -17, -19, -20, -21, -22), with §31's two open items (SEC-PARENT-2: the self-call rule for a connected multisig's own address; SEC-PARENT-3: an unlisted Safe's ownership answer comes from one RPC).

## 33. Round 19 native fixes

Round 18's Android and iOS findings that were still open (r18-1, -2, -3,
-4, -9, -16, -18) and the native adoption of the core lane's Round 19 rules
(r18-5, r18-8; §32). Decided by best practice
under the owner's rules: one core owns the rules and the shells draw them;
parity with web and desktop; honest states — never "No activity yet" while a
network is unread, never Welcome or another account flashed, a status chip
never contradicting the body. Each item was written test first.

| Item | Cause | Decision |
| --- | --- | --- |
| **r18-1 (S3) Android + iOS: "No activity yet" beside "Can't reach …"** (r17-3 not adopted) | Neither shell dispatched the core's `activity_feed` `BalanceRead` (§29), so `settled` rested on the scan alone — which reads only where the account holds or lives — and the balance's unread networks never reached the feed. | Both shells hand the balance view (the core's own, unfiltered for space; the feed filters) to the feed on every distinct word — Android `feedFollowsBalance` (the first word said synchronously, before any account switch can reach the feed, so a feed never settles on the scan alone), iOS `AppGraph.feedFollowsBalance` (said again after the feed core boots). The unread line names the one network from the shell's lists, else the core's `unread_network` (or catalog), else the many-networks words — never a raw chain id (R16's Android pin on the id is replaced). |
| **r18-2 (S3) iOS: no unread line over rows** (r17-5) | The same missing wiring: with Arc refused only the balance knew Arc was unread, the scan's set (Arc not scanned) stayed empty. | Item 1's wiring; the existing line over rows on the home and in History now has its data. Tested with rows and a refused network through the real cores. |
| **r18-3 (S3) Android: Welcome, then the other account's home, at a cold start** (iOS's r16-11 / r17-7 twin) | The NavHost's start destination is Welcome, drawn while the session is `Loading`; then the hub (booted only once the session says Wallet) had not read `vela.activeMultisig`, so the home drew the session's Vela account for ~0.7 s. | iOS's §30 rule on Android: `LaunchFront.screen` — `Loading` → the page's ground; `Onboarding` → Welcome; `Wallet` → the ground until the stored front is read (once, at launch, after a debug seed, before the session boots), the hub's accounts are loaded when a multisig was in front, and the wallet route is the only one the NavHost shows; bounded by 2 s from the session's Wallet. `LaunchGround` draws `bgBase` over the NavHost, takes every touch, says nothing, lifts once for good (saved across a recreate). |
| **r18-4 (S3) iOS / r18-16 (S4) Android: the chip against the body around an execution** | iOS: a mailbox refresh marks the proposal `done` before this device's flight is judged; the body fell to the done branch (the mailbox's one approver, no buttons) while the chip fell to the standing ("1/2 approved"). Android, the mirror: the chip held "Executing…" (press working / receipt waiting) over a body already "Executed", with "Which approvals counted … isn't recorded on this device" (the frozen facts not in yet). | ONE answer drives both (`chipSaysExecuting` / `executingHere`): false once the receipt is judged (`executed` / `executedByOther` / `replaced`) whatever the press's action still reports; else true while the flight is `waiting`, this device's `execute:` works, or the core follows the flight. While it holds: the chip says "Executing…", the approvals heading carries no count, every member is listed (Approved or nothing — no "Waiting"), no Executed box or facts; "isn't recorded" is said only of an execution this device did not follow. Both move on in the same frame. |
| **r18-9 (S4) Android + iOS: fiat in the Testnet space** | Each shell kept its own copy of the rule and applied it to Send and the signing sheet only: Android's Activity detail "≈ $0.30", iOS's hero "Total balance · CNY". | **One core call:** uniffi `network_space_shows_fiat(space)` (`network_space::shows_fiat_str`; the only core edit of this lane). Android: `NetworkSpace.showsFiat`; `WalletLive.Money.of` is the space's money (`Money.space`, said by the app on every change before the screens recompose) and `withoutValue` names no currency (empty label); the Send machine is told `SendDisplayContext.space`. iOS: `NetworkSpaceKind.showsFiat` feeds the hero (no code), the widget, the rows, the Assets list, the switcher, the token page, the Activity detail, the browser's holdings, Send (no rate told) and the signing fee. |
| **r18-18 (S4) iOS: a hung key index called "This device is offline"** | iOS's `RegistryClient.request` marked every transport failure — the 15 s idle timeout of a hung index included — as `network`, which the core says as "This device is offline"; the guard's refusal was `network: false`, said "index didn't answer". | Three causes, three sentences. iOS's `network` now means "never left this device" (`notConnectedToInternet`, `dataNotAllowed`, roaming, a call) → `keyKeptBodyNetwork`; a timeout, refused connection or bad answer is the index's → `keyKeptBodyIndex`; this build's guard → `refused: true` and the core's `words_key` `common.testBuild.productionWriteRefused`. Android parity: the kept key now has its own title and sentences (it drew the raw detail), and "offline" only when `NetHealth` says the device has none (the onboarding executor's `network` flag is unchanged — the core also reads it to offer another endpoint). |
| **Adoption (r18-5, r18-8; core §32)** | The core lane's rules: a pressed look is never re-quoted and is served `signing: true`; a write the guard refused is `refused` and has its own kind. | Both shells: the signing look's fee line stands and `home.multisig.proposal.feeSigning` is drawn under it. Each shell tells the core `refused: true` on multisig `http_unreachable`, onboarding `index_failed` and session `registry_landing_unconfirmed` when its guard refused (Android `Refusals.of` along the cause chain). The core's answers are drawn in `common.testBuild.productionWriteRefused`: a failed action's `production_write_refused` kind, an outbox entry's `why`, a create record's `create.refused`, the sync-failed screen's `sync_error_key` (in the message's place), the kept key's `words_key`. As §32 rules it, the core never retries by itself and the person's own press stays: Send again on the outbox entry (`retry_at: null`) and Retry upload on the sync-failed screen are kept on both shells. Merged with the core lane's own native refused wiring (`f97d51462`): one implementation each — Android reads the guard's refusal along the cause chain (`Refusals.of`), iOS through `TestBuildRefusal` (a no-op in Release); `refused` is sent only when true. |

Tests:

- **Core (uniffi):** `tests_r19_native::the_testnet_space_shows_no_fiat_by_the_cores_word`.
- **Android:** `R19NativeFixesTest` (JVM, 11): the real `balance_dashboard` over a pool whose OP Sepolia node never answers and the real `activity_feed`, through the wiring — no frame says "No activity yet" while the read is out or after it fails, the unread line in its place; every network read → "No activity yet"; the launch through the real session machine over a real store (multisig in front: ground → wallet; Vela account: ground → wallet; none: ground → Welcome); the chip and the body through press → mailbox done → receipt; every money surface in the testnet space with no currency symbol or code; the signing mark; refused writes and their words; three kept-key sentences. R16/R17 pins follow (no raw id; `inSpace`); `FeeFailureViewTest` follows the core lane's r18-14 relay words.
- **iOS:** `R19NativeTests`: the real balance and Activity cores with Arc's reads held open past a 3 s per-chain deadline (`anEmptyAccountUnderAHungNetworkIsNeverClaimedEmptyUntilEveryNetworkIsRead`), a feed booted later hearing the reads again, refused Arc with rows on the hosted home and in History; the testnet hero naming no currency and every money surface without fiat (Send and the signing fee through the display they are handed); the chip/body frames; the signing fee; the three index sentences; every refused write handed on and said. `MultisigR8FixTests`, `OnboardingExecutorTests` and `CoreRoundTests` follow the new rules (the "isn't recorded" rule, the offline flag, the core's r18-14 relay words).
- **Device:** five cold starts of the debug build with a multisig in front on a fresh emulator, recorded and OCR'd at 20 fps: 0 Welcome frames, 0 frames of the Vela account, the multisig's home directly after the ground (the hub's load lifted it every time, never the 2 s bound). The instrumented suite (197) passes on a clean install; `WalletLive.Money.space` follows the stored space, so a device whose stored space is the testnet draws no fiat in the mainnet-pricing device tests either (seen once with this lane's seeded emulator; a clean install is green).
- **Gates:** Android JVM 1884/0; instrumented 197 (193 + the 4 that the seeded data had broken, 10/10 rerun on a clean install); release APK check ok; `check-native-wire --strict` and `check-ios-wire-parity --strict` ok; iOS 2136 tests, 1 known issue; Rust workspace (`i18n-all,dev-fixtures,crux`) fmt, clippy `-D warnings` and 4192 tests; i18n gen (no change), lint, verify; the wasm rebuilt for the merged core (`build-web --check`, `verify:wasm`); web `pnpm check` and vitest; desktop `cargo test`; `check-scenarios`.

Left open:

- iOS r18-9: Send's form/confirm and the signing fee are checked through the display they are handed and a source check, not rendered live.
- iOS: the many-networks words read "1 networks" for one network nobody names (§29's known gap).
- A multisig press failing with `production_write_refused` is worded from the core's key; the Try again beside it was not audited on either shell.
- r18-1 on a device in the Testnet space was not re-run (the seeded space did not take on the emulator); a mainnet cold start showed the unread line ("Activity can't be read on World Chain right now.") in the empty state's place while World Chain was unread, then "No activity yet" once it answered.
