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
| `unreached_chains` after one call's first pass failed on transport at every endpoint | after `UNREACHED_AFTER_ENDPOINTS` = 3 DIFFERENT endpoints failed on transport (a hang, a refusal) with no answer of any kind from the chain between them — from any calls; a pool smaller than three needs all of them. An answer (a result, an error, a 429) empties the count. RF1 stays. |
| every read of an out-of-reach chain swept every endpoint, three passes | the circuit breaker's half-open probe: ONE read at a time probes the chain, on at most `OUTAGE_PROBE_ENDPOINTS` = 2 endpoints (best score first; cooldowns rotate them) in a single pass; any other read of it meanwhile fails at once with nothing on the wire (`failed`, not delivered). The read that found the outage stops after a probe's worth. A usable answer clears the chain as before. Writes (anything not a hedged read) are never refused this way. |
| `pool_unreached` read again, forced, and named the network when that read failed (+18 s) | it names it AT ONCE (the network joins `failed_chain_ids`: no "Live" over it) — the pool's word is a read's own: several nodes said nothing, nothing answered since — and the forced read owed confirms it or unsays it (`pool_said`; a cache's answer does neither). |

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
session — named < 35 s after the first hung request, < 25 hung requests).

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
| (desktop) rows written in the background, feed not told | written, then `activity_feed::focus_tick` |

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
- **D9 (dev harness).** A passkey's relying party is a domain, never an IP
  address: the local trusted page on `127.0.0.1` showed the browser's raw
  `SecurityError`, and its passkeys would not be `localhost`'s. One loopback
  origin — `localhost`, same port: the core's `signer_url`, the app's
  `loopbackPage` and the Safari `dev-seed.js` all canonicalise `127.x` and
  `[::1]` to it.
