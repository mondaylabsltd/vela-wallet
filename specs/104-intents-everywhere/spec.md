# 104 — The agent on every Vela: the intents' judgements in Rust, a native ClientMCP client

**Status:** draft · 2026-10-21
**Follows:** spec 103 (web). The owner asked whether the web agent is ready for iOS, Android and desktop. It is, in design. Three pieces of groundwork come first, so that each shell does not port the same logic again.

## Owner decisions (2026-10-21)

| # | Question | Decision |
|---|---|---|
| E1 | What of the intents moves into Rust | **Only the pure judgements** (part A below). The orchestration stays in each shell (part B), as decided for 103 (D6): a stateful intents core was judged over-abstraction. |
| E2 | Part B on the phones (phase 7, revising E1 for the native shells) | **One shared Rust agent core, `vela-agent`**: the runner, the transfers, the tools and the ClientMCP connection, moved out of the desktop and reached by the phones through uniffi. Each native shell keeps only its "do one thing" (its own stores' calls), its views, opening a screen, and its UI. The web keeps its TypeScript part B. |

## What moves, and what does not

**Part A — into `vela_core::app::intents` (about 620 lines of the web's TypeScript).** These are pure functions over views the core already produces (`SessionView`, `ContactsView`, `BalanceView`, `FeedView`, `NetView`, `CurrencyView`, `FeeTierPrefView`, `SendView`, `SignRequestView`, `TrackEntryView`, all of which already derive `Deserialize`).

| | Web today | What it decides |
|---|---|---|
| A1 | `rules.ts` `examine` | Does what an action names exist; is it already so (then nothing is asked or done). |
| A2 | `rules.ts` `baselineOf`, `verdict` | Has the change held: `completed` is said only once it is seen. |
| A3 | `transfers` `#fromSend` / `#fromChain` | Where a transfer stands, read from the send, the passkey's ceremony and the chain tracker (the 2026-10-21 "submitting before signing" fix lives here). |
| A4 | `state.ts` | `vela_state`: what an agent may see. The privacy rules live here: hidden balances stay hidden, a provider key is only "set or not". |
| A5 | `review.ts` | `vela_review`: what the person is asked to sign, with the note that none of it is an instruction to the agent. |

**Part B — stays in each shell (about 1,180 lines on the web).**
- `perform`: the shell's own resident calls.
- The event sequences that wait on views: networks, tokens, drafting a transfer.
- The call runner: approvals, timeouts, the bridge's cancel, Stop.
- Presenters, the approval sheet, the agent bar, and the ClientMCP connection.

## Design

- **`World`** (Rust, `Deserialize`) is what a shell sees, one optional view per resident:
  - `session`, `contacts`, `balance`, `feed`, `networks`, `currency`, `fee_tier`;
  - `operations`;
  - `preferences` (the seven shell-kept values);
  - `agent_enabled`.
- **The functions:**
  - `examine(action, world) → Examined`;
  - `baseline(action, world) → Baseline`;
  - `verdict(action, world, &mut baseline) → Verdict`;
  - `operation_state(operation, send, ceremony, tracker) → OperationState`;
  - `agent_state(world, extras) → JSON`;
  - `review(send, operation, signing, …) → JSON`.
- **`world_needs(action)`** names the views a judgement reads, so a shell polling `verdict` serializes those, not the whole wallet (the feed alone is large).
- **Exports:**
  - wasm `intentExamine` / `intentVerdict` / `intentAgentState` / `intentReview` / `intentOperationState` — the web drops its TypeScript copies and calls these;
  - uniffi the same, for iOS and Android;
  - the desktop calls them directly (it links `vela-core`).
- **Tests:** the web's judgement tests (`intents.test.ts`, the transfer mapping, `review.test.ts`, `state`) move to Rust. The web keeps its orchestration tests, now over the wasm judgements. The e2e is unchanged and must stay green.

## The native ClientMCP client

- **`clientmcp-client`** is a Rust crate implementing the `clientmcp.v1` client:
  - register and `registered`;
  - calls, results and `cancel` (a cancellation token per call);
  - ping and pong, reconnect with backoff;
  - the reserved outcome members.

  It runs the same conformance suite as the JS SDK.
- **The desktop** (gpui, already on tokio and tungstenite) uses it directly. iOS and Android use it through uniffi, or a thin native SDK if a platform needs one.

## Before going live

- ~~**The production bridge's cancel** (Cloudflare): an agent that hangs up in the first second of a call is never cancelled in the app.~~ Fixed in phase 4.
- **One-step pairing (103 phase 7).** Copying a bridge URL and a minted token is not usable on a phone. Pair by QR code: the app shows it, the agent's side scans or pastes it.
- **Mobile in the background.** iOS (and Android, less strictly) suspends the socket. The agent reaches the wallet only while it is in the foreground; waking it is out of v1, and the app says so.

## Phases

| # | What | Exit |
|---|---|---|
| 1 | Part A into Rust; wasm exports; the web calls them and drops its copies | **done** — `intent_rules` (14 Rust tests); the web's `rules`/`state`/`review`/transfer progress are doors to it; unit 3900, e2e 455, live run |
| 2 | uniffi exports for the intents | **done** — `intents_bridge` (the same JSON functions); Kotlin and Swift bindings generated; both harnesses replay the shared `intents` vectors |
| 3 | `clientmcp-client` (Rust) | **done** — conformance 34/34 with it as the application, against the native bridge and the Worker in workerd; 6 lifecycle tests of its own |
| 4 | Production cancel fix | **done** — deployed to `bridge.clientmcp.dev`; conformance there 33/34 with either application, the one skipped needing a 5 s call timeout; first-second hang-ups cancelled within 0.25 s |
| 5 | Desktop: the agent (part B in Rust, on gpui) | **done** — `src/agent/` and `wallet/page/agent.rs`; the desktop's coverage ledger (`agent::coverage`); a live run through `bridge.clientmcp.dev`, transfer included |
| 6 | QR pairing | **deferred** (owner, 2026-10-21): `bridge.clientmcp.dev` serves as the open trial bridge for now. The design proposed: pairing over the client's own connection (the bridge mints an agent token for the client it is), and optional open registration on a bridge that enables it |
| 7 | iOS and Android: part B, the approval sheet, the bar, foreground-only | **implemented, not production-ready** — `vela-agent` shared with the desktop; live runs through `bridge.clientmcp.dev` on the iOS simulator and an Android emulator; the passkey on a physical Android phone and a physical iPhone; the security review's fixes and the review bound to the signature in. Open before production: F3, the token on the web and the desktop — see "Mobile: what is not verified yet" |

## Phase 1 notes

- **The facts types.** `World` does not take the full view types. Its "facts" types carry the views' own field names, only the fields the judgements read, every one defaulted, unknown ones ignored. A shell passes its views unchanged; a test passes only what it means; a view gaining a field touches nothing here.
- **`world_needs`** keeps a polled verdict from serializing the whole wallet: deleting a record sends the feed, a contact change sends the address book, and so on.
- **Cost.** The wasm grew by 143 KB raw: +52 KB gzip, +38 KB brotli, about 3%. Most of it is the deserializers for the facts types (the projections written with `json!` or with typed structs measure the same). Reading the world as a generic JSON value instead would cut it, at the cost of the typed code. That is the owner's call.
- **Found while porting:** a cancelled transfer kept the agent's "waiting for the person to confirm" as its reason. It now says it was left without sending.

## Phase 3 notes

- **Where.** `crates/client` in the ClientMCP repository (`clientmcp-client`), beside the JS SDK, with the same behaviour:
  - online on `registered`;
  - ping every 25 s, the socket dropped after 10 s of silence;
  - jittered backoff up to 30 s;
  - stops for good on 1009, 4400, 4409, and on 4401 with a fixed token;
  - an oversized result becomes a tool error;
  - `completed` / `accepted` / `rejected` refuse the reserved members.
- **Cancelling is stricter than the JS SDK's.** When the agent cancels, or the connection ends, the handler's future is dropped. The work stops at its next await, instead of a signal the handler may ignore. The handler also gets a `CancellationToken`, for work it handed elsewhere.
- **The conformance suite runs it.** `CLIENTMCP_APP=rust tests/run-native.sh` (and `run-worker.sh`). An example app (`conformance_app`) does the protocol. The suite's handlers stay in JavaScript, reached over stdio. A dropped future reports back as a cancel, so the cancel tests check the Rust behaviour itself.
- **Its own tests**, against the native bridge, cover what the suite does not:
  - reconnecting after the bridge restarts;
  - a fresh token for each connection, with a 4401 retried;
  - a refused fixed token closing for good;
  - `connect` after `close`;
  - an agent hanging up in the first second dropping the handler;
  - `wss://` going through rustls.
- **Found:** with only the WebSocket crate's TLS feature, rustls had no crypto provider, so the first `wss://` connection would have panicked. The client now builds its own TLS config with *ring* and the platform's roots, and installs nothing process-wide.
- **The production cancel bug is not in workerd.** The first-second hang-up test passes against the Worker run locally. Phase 4 has to be checked on Cloudflare itself.
- **Not yet:** the uniffi wrapper for iOS and Android. It comes with phase 7, shaped by what the native shells need.

## Phase 4 notes

- **The cause.** On Cloudflare, an agent hanging up on a call's pending JSON response (its first second) ends the Worker's invocation without dropping anything. The bridge's only signal had been a dropped stream, so the application kept the call, and its approval sheet, until the 120 s call timeout. workerd does not do this, so local runs never showed it.
- **The fix** (`crates/worker`):
  - the `enable_request_signal` flag;
  - the Worker cancels on the agent request's abort signal;
  - the Durable Object also watches its own request's signal;
  - the object remembers, for 10 s, a Worker cancel that arrived before its call, and refuses that call.
- **Measured on production**, hanging up at fixed delays: before, hang-ups at 0.6–0.8 s were never cancelled; after, every hang-up whose call reached the app was cancelled within 0.25 s.
  - A staging copy on workers.dev, reached through this network's HTTP proxy, could not show the bug: the proxy keeps the connection to Cloudflare open until the next write.
- **Production now** runs version `c0b5e511`; the one before was `5bee0b6c` (rollback: `wrangler rollback`).
- **The suite** passes on production with the JS SDK and with the Rust client (TLS included). The only skip is the call-timeout case. The harness's https handshake case now works there too.

## Phase 5 notes

- **The client** is vendored (now `rust/vendor/clientmcp-client`, shared with the phones; the owner's choice over a git dependency) and runs on a thread of its own: the desktop has no tokio reactor. Its socket is dialled through the app's proxy route — the cable tunnel's own dialler, now shared (`ctap::cable::dial`) — for which the client gained a dial hook.
- **Part B in Rust** (`src/agent/`): the runner (`intents.rs`, generic over its wallet so its rules — approvals, Stop, a call given up, the observe — are tested without a window), `perform.rs`, `transfers.rs` (drafting on the send column, the passkey's ceremony counted in `SendHost::dispatch`), `world.rs`, and `tools.rs` (the same 32 tools as the web). The judgements are the core's, called directly.
- **The person's clicks go through the intents** (spec 103 D3): the same 21 actions as the web's `PERSON`, from the account switcher to the review's confirm. A commit that closes its dialog "on the core's word" (adding a network) waits for `completed`.
- **What the person sees**: Settings → AI agent (switch, bridge, masked token, the MCP endpoint, the last action); a capsule floating at the top of the window, as the web's (owner, 2026-10-21: a full-width strip ran into the window's buttons) — the link's state as a dot, what the agent did last, Stop; the approval sheet, above every other dialog, Esc being No.
- **What an agent did is seen where it is** (owner, 2026-10-21: "授权后还是停在首页没有变化"): once an agent's request completes, the screen its change lives on opens — `intents::shows(action)`, in the core so every shell opens the same one (`intentShows` for the web, `intent_shows` for the phones). The web opens it too (its runner's `reveal`, through the same presenters `show` uses).
- **The ledger** places 198 core events and 61 direct calls of the desktop's screens. Its `later` list, the gaps: several tokens to one address (sweep); paying the fee in another token, and one transfer's own speed; testing a provider key; answering a dApp's signing request (and limiting its approval); disconnecting one site, forgetting one site of the history, browsing in the in-app browser; signing in or creating a wallet. Not on the desktop at all: usage statistics (refused before anyone is asked) and a community page.
- **Live run** (2026-10-21, the parallel space's fixture wallet, through `bridge.clientmcp.dev`):
  - operate actions, approvals (Allow, Stop, a request withdrawn when the agent gave up), the person's own changes through the intents;
  - adding Arc Network Testnet;
  - a 0.1 USDC transfer drafted by the agent, read through `vela_review`, confirmed by the person on the review: `awaiting_passkey` → `submitted` → `confirmed`.

  Deny was not clicked in the live run (the runner's tests cover it). Found and fixed on the way: the agent was asked to approve a statistics switch the desktop does not have.
- **Not checked here:** the Linux and Windows builds (CI's jobs); the signing sheet's `vela_review` half on the desktop (no dApp request came up).

## Mobile: what is not verified yet

Phase 7 is implemented on both phones; that is not the same as ready to ship. The owner's order (2026-10-21), and where each stands:

1. **Coverage ledgers — implemented.** iOS `AgentCoverageTests`, Android `AgentCoverageTest`, as the desktop's `agent::coverage`: every call a screen makes into a store or controller is placed (intent, step, view, machine, none, later), the 21 commit points are checked to go through the intents, and only catalogued actions are named. Their `later` lists are the gaps an agent cannot reach yet: several tokens to one address (sweep), paying the fee in another token, one transfer's own speed, answering a dApp's signing request (and limiting its approval), browsing in the in-app browser, disconnecting one site, testing a provider key, signing in or creating a wallet, and (Android) the Ethereum key backup.
2. **The client token in secure storage — implemented, verified on a simulator and an emulator.**
   - iOS: the Keychain (`AgentTokenVault`, `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`). On an ad-hoc-signed simulator build the token is kept and read back, the `vela.agent` record holds `{enabled, url}` only, and the agent reconnects after a relaunch from the Keychain alone. An unsigned build — CI's — has no Keychain (`errSecMissingEntitlement`): there the Keychain test is reported disabled, and a token entered is kept for that run only.
   - Android: the Keystore (`KeystoreTokenVault`: an AES-GCM key inside the Keystore seals the token; only `vela.agentToken`, the sealed bytes, is stored). On the emulator the plain token an earlier build stored was moved and is gone from the app's store, and the agent reconnects after a cold restart.
   - Both: Erase This Device forgets it (the Keychain item; the sealed bytes and the Keystore key). Android now also switches the agent off and hangs up there: the app outlives the erase, and its connection had been dialled with that token.
   - The web and the desktop: see "The token on the web and the desktop" below.
3. **A person-confirmed transfer on chain — verified on both phones, with the parallel space's fixture keys.** The agent drafted 0.00001 BNB to a fixture Safe; the owner confirmed it on the review; it was signed, submitted and confirmed (BNB Chain, EntryPoint v0.7, `UserOperationEvent` success):
   - iOS simulator: tx `0x61fe46636c27dcad665745740dc4d2a98a0530999bb7541d0ab0df1538a043fe` (block 128933432);
   - Android emulator: tx `0x5fbe63113bf7a59b881662a5d661f8453023542b9bd23ab03cc3aa3138badde4` (block 128934102), followed through `vela_wait`: `awaiting_confirmation` → `submitting` → `submitted` → `confirmed`.
4. **A platform passkey — verified on a physical Android phone and a physical iPhone.** See "The passkey on a phone" below.
5. **A security review of the agent surface — done; its fixes in.** See "Security review (2026-10-21)" below.

6. **The review bound to what the passkey signs — implemented on every shell.** See "The review, bound to the signature" below.

Still open before production: the bridge noticing a phone that went silent (F3, ClientMCP); the token on the web (`localStorage`) and the desktop (`0600`, not yet the keychain). Still deferred: QR pairing (phase 6).

### The passkey on a phone

A Redmi K40 (Android, `9d5f42fb`), the owner's phone. The app on it was backed up and upgraded in place (owner's choice); the owner's wallet on it was neither used nor changed. A dedicated wallet, **Agent test** (`0x1100…45aC`), was created on it with the phone's own passkey and funded with a few cents of BNB from the parallel space's fixture wallet. `vela_state` was not called on the phone (it lists every account on the device). Each run: the agent drafted 0.00001 BNB to the fixture Safe through `bridge.clientmcp.dev`; its submit only asked; the owner acted on the phone; a watcher followed the operation through `vela_wait`.

| Run | What the owner did | What the agent read | On chain |
|---|---|---|---|
| T1 | Confirmed on the review, passed the passkey | followed to `confirmed` | `0x96636f986c1c3bfce45b23c5b606a1b3c2821364232d4b1a9c5da6cf35b89c9c` |
| T2 | Cancelled the system passkey sheet | back to `awaiting_confirmation`, "the person dismissed the passkey prompt"; nothing signed | — |
| (T2b) | Meant to cancel, signed instead | followed to `confirmed` like T1 | `0x4506569184463aa2a7b7023ab12b7c7a7fad2d1fdcabc09758c9fc8283e34ec9` |
| T3 | Went back from the review and left the send | **wrong: stayed `awaiting_confirmation`** — fixed (below); run again with the fix: `cancelled`, "left the review without sending" | — |
| T4 | Sent the app to the background on the review, came back, confirmed | the transfer followed to `confirmed` | `0xbf95f05c5662e410889b08b3b282e7e4393c34cc98afe843b921094d3d989a9e` |
| T5 | Airplane mode on the review: the send failed; network back, retried on the same review | the retry followed to `confirmed` | `0x3e3069cf317d5ee17caaf0d2ab1c3ff3d77d2424c7f414518cf8871d869b2c24` |

Every transaction above has `UserOperationEvent` success. **Found and fixed:**

- **Leaving the review did not end the transfer** (T3). The core's reading of the send (`intent_rules::from_send`) now says a send off its review with nothing sent is `cancelled`, "left the review without sending"; and a second review on the same send is a transfer of its own (`Transfers::adopt` passes over one that has ended), on every shell — `vela-agent` and the web's `transfers`.
- **A send that stopped on its review read as `failed`**, a final state, though the review offers to try again — so an agent would have drafted it anew while the person's retry went through. It is now `awaiting_confirmation`, with why it stopped.
- **The bridge does not notice a phone that went silent** (T5, F3): while the phone was in airplane mode the agent's calls hung for minutes. Not fixed — ClientMCP's, below.

**The iPhone** (2026-10-23): an iPhone 11 (iOS 26.5.2) of the owner's, with seven of the owner's wallets on Vela 0.9.8 (build 4677, a developer build). Before anything was installed, its app data was copied off and a rollback build of 4677 made from its own commit, signed for the phone (`~/VelaDeviceBackups/iphone11-F30282CB-20261022/`, with the steps to restore). It was then upgraded in place to a Release build of `e515261be` — Release because Debug links the parallel space's fixed keys — and the seven accounts read back unchanged, before and after the runs. A dedicated wallet, **agent test** (`0x61B6…6022`), was created on it with Face ID, and funded with 0.0001 + 0.00002 BNB from the fixture wallet on the emulator — itself the first transfer through the review binding (`0x41a0bb8f…`, `0x29bc2166…`). The agent ran on a bridge client of its own (`vela-iphone-test`); its token, passed once at launch, went to the Keychain (the record holds `{enabled, url}`).

| Run | What the owner did | What the agent read | On chain |
|---|---|---|---|
| I1 | Confirmed, Face ID (the wallet's first operation: it deployed the Safe) | `awaiting_confirmation` → `awaiting_passkey` → `submitted` → `confirmed` | `0x54ffca05e402d3e38cd1b3f910713076f662c2ce95f8ad1c1d6aee3507c6c96f` |
| I2 | Cancelled the passkey sheet | back to `awaiting_confirmation`, "the person dismissed the passkey prompt"; no hash, the balance unchanged | — |
| I3 | Went back from that review and left the send | `cancelled`, "left the review without sending" | — |
| I4 | Sent Vela to the background on the review, came back, confirmed | offline while in the background; then `awaiting_passkey` → `submitted` → `confirmed` | `0xca0bab439d5ede9716463042700a1b3675bdb628143550c57bda115633006db6` |
| I5 | Airplane mode on the review: the send failed; network back, retried on the same review | `awaiting_passkey` → `submitted` → `confirmed` | `0xc55979e39858e2e9957d8bbb967c8908c6a26907c9ff665dd07aca97f6f73fa1` |

Every one with `UserOperationEvent` success. Each confirm went through the review binding: the tap named its review, and the passkey's hash was computed only for the operation that was it — on a real passkey, end to end. Not exercised by hand: a swap under the tap (the unit tests cover it).

On the Android phone, T3 was run again with this round's build installed in place: Back from the agent's review read `cancelled` at once. The person's own second review on the same send could not be shown there — the test wallet's balance no longer covers the fee, so the wallet stops at Continue — and is covered by `vela-agent`'s and the web's tests. The same build connected only once the app was in front (S4). **The physical iPhone is not done**: the paired iPhone 11 has Vela 0.9.8 on it, and whether to back it up and upgrade it in place is the owner's to say.

### Security review (2026-10-21)

The question: can any agent action bypass the person's confirmation, trigger a passkey signing, or reach sensitive wallet data without the person's say?

**What holds** (read in the code, and covered by tests):

- **No agent action signs or raises a passkey.** `submit_transfer` from an agent only marks the transfer "waiting for the person to confirm it on the review" (`transfers::submit`); `SlideConfirm` is dispatched for the person alone. There is no tool, action or `perform` route to the passkey, to a dApp's signing request, or to a key.
- **The approval gate.** `authorize` in the core: the person always; `person_only` (the agent's own connection) never; nothing while the agent is off; manage- and transact-level actions only after the person's Allow; `sign_out` on the session's own sheet. An approval is withdrawn when the agent's call is given up, on Stop, and in the background.
- **What an agent reads.** Balances and amounts are hidden from it while the person hides them; spam is not read; a provider's key is read only as "set or not"; `vela_review` hands a dApp's words over as data.
- **The send.** An agent reaches it only through the send's own events, in the form's order; a send the person has open is `busy`.

**What was weak, and is now fixed:**

| # | Finding | Fix |
|---|---|---|
| S1 | **A review swapped under the person's finger.** `cancel_transfer` and a new draft need no approval, so an agent could replace the review the person was reading with another just before their tap; the passkey sheet after it shows no transaction. | A review an agent drafted takes the person's confirm only once it has been on screen 1.5 s (`REVIEW_SETTLE_MS`, `vela-agent` and the web); a tap before that is refused, "look it over, then confirm again". The person's own review is theirs at once. |
| S2 | **Hidden balances could be shown or read.** `set_balance_privacy { hidden: false }` was operate-level, so an agent could unhide them without asking; and `max` on the review is the balance itself. | Showing them is manage-level (the person's Allow, "Show balance"); `max` is refused while they are hidden (`examine`). Left: a draft refused for its amount still tells an agent the balance is below it — each probe opens the send in front of the person. |
| S3 | **Approvals that did not say what.** Changing an RPC, a service endpoint or a provider key was asked as "Change a service endpoint", with no URL. | `intent_rules::approval_detail` (`intentApprovalDetail` on every shell): the chain, the field, the URL whole — it is the person's own screen — and a key as its last four. |
| S4 | **Android, and iOS, dialled before the app was in front** (a process started for a broadcast or a background launch). | Not in front until the activity, or the scene, says so. |
| S5 | **URLs that carry keys went to the agent whole** (a provider's key in an RPC path, a key in a query, `user:pass@`). | `redact_url`: scheme and host, `/…` for the rest; the wallet's own defaults whole. |
| S6 | **Android's erase left the agent connected** with the token it forgot. | Off, and hung up. |
| S7 | **Stop did not stop at once.** A draft under way went on to its review; requests in the moment before the setting was saved were answered as "on". | `Agent::stop` (and the desktop's) switches it off at once; a draft checks between its steps (`SendDesk::halted`: stopped, off or in the background) and leaves nothing open. |
| S8 | **An approval could change under a tap.** Android's dialog was the same dialog with the next request's words. | One dialog or sheet per request, its Allow taking a tap only after 0.7 s (Android, iOS, the web; the desktop ignores an Allow click before then). |
| S9 | **Another wallet, reached without asking** (found on the iPhone run, 2026-10-23). `switch_account` was operate-level, and the active wallet is what an agent reads and drafts from: it could switch to any wallet on the device and read its balances and history. | Manage-level: the person's Allow, the approval naming the wallet ("name · address") on every shell. Their own switch is unchanged. On the phones (2026-10-27): the Redmi K40 — Allow switched, Deny answered `denied` and nothing moved; the iPhone 15 Pro — Allow switched (twice, each meant as Deny: see below). |

S1's delay is one of three links now; the others bind what the person saw to what is signed (below).

**Found on the phones, to fix:** on iOS the approval sheet's Allow is the large red button on top and Deny the one below it; asked to tap Deny, the owner tapped Allow twice. The safe answer should be the easy one.

**Left, low:** an agent-drafted review is not marked as the agent's on the review itself (the capsule says what the agent did last; on an Android flow, where it is compact, it shows only the link and Stop). Side-effect actions an agent may take without asking (show, refresh, save or favourite a contact) are visible and reversible.

### The review, bound to the signature (2026-10-22)

The owner's follow-up to S1: that an agent cannot reach the passkey does not mean the confirmation cannot be steered. A delay on the review is a heuristic; what the person confirmed must be bound to what the passkey signs.

**What an audit of the four shells found** (each assembles the operation itself, from the send core's `SubmitUserOp`, and has the passkey sign its SafeOp hash):

- Nothing compared the final operation with the review. The web's attest rebuilt the calldata from the same calls and the shell's own fee choice, so it could not catch that choice.
- The web and the desktop, finding the confirmed quote missing or unusable, fetched a new one and signed it — a fee nobody reviewed; on Tempo with no quote they signed a computed reimbursement. The phones refuse both.
- The send core signed whatever fee estimate it held when the confirm's own checks (the credential, the relay's float) finished: a re-quote landing between the tap and the signature was signed.
- Every confirm found "the review" when it was handled, not when it was drawn: a tap on one review handled after another took its place (an agent's cancel and redraft) confirmed the second.

**Three links, now:**

1. **The tap names the review it was drawn on.** On the review the send's view carries `review_digest`: a SHA-256 over exactly what the passkey would authorize — the account, the chain, every transfer (recipient, token, base units, in order) and the fee (`signed_review::SignedReview::digest`). Each shell's confirm reads it when the button is drawn (iOS a capture list, Android at composition, the desktop at render, the web from the view on screen) and sends it with the confirm (`submit_transfer { review }`, `slide_confirm { review }`). A confirm drawn for another review confirms nothing (`vela-agent` and the web refuse it with a reason; the send core refuses it again). The person's `submit_transfer` without one is refused.
2. **The confirm freezes what it confirmed.** At the confirm the send core freezes the review (`confirmed_review`), derived from the confirm screen's own figures (its lines, in base units, and the quoted fee) — not from the calls, so a builder that drifted from the screen is caught too. When the submit is built it is derived again; anything that moved — a re-quote that landed meanwhile — puts the review back in front of the person with its new figures, unsigned. `SubmitUserOp` hands the frozen review down.
3. **The hash the passkey signs exists only for that review.** `signed_review::reviewed_safe_op_hash` decodes the operation's calls out of its calldata — the exact inverse of the wallet's own encoders: one `executeUserOp` CALL, or `executeUserOp(MultiSend, …, DELEGATECALL)` whose packed calls are all CALLs — reads each as a native or ERC-20 transfer (anything else, an `approve` say, is refused), and compares sender, chain, every transfer in order and the fee leg with the review. Every shell's send computes its challenge through it (`userOpReviewedSafeOpHash` on the phones, `attestReviewedSafeOpHash` on the web, directly on the desktop); a dApp's request keeps the signing sheet as its review. A send without a review signs nothing. The fallbacks above now fail closed: a fee the review did not show is not signed.

**Tests:** the core's decoder and check (5: the wallet's own operations read; a drift in amount, recipient, token, fee, chain, sender, a leg more or less; a call not shown; shapes the wallet does not build); the send core (3: the review handed down is the one drawn, and only its operation hashes; a confirm drawn for another review; a re-quote after the confirm); `vela-agent` and the web's ledger (a confirm naming another review, or none); the web's attest; iOS's executor through its real spine (a review saying 999 where the operation pays 1000, and none at all: no ceremony). Not yet: the same on a physical device.

### The token on the web and the desktop

Reviewed; what a token gives: connecting as this wallet's application to its bridge — so whoever holds it can answer the person's agent as if it were the wallet (wrong balances, wrong addresses), or push the real wallet off. It signs nothing: the passkey is on the device.

- **Web:** `localStorage` (`vela.agent`, `{enabled, url, token}`), in the clear. Readable by any script on the wallet's origin and by anything that reads the browser profile. A non-extractable WebCrypto key in IndexedDB would keep it from the profile's files and from a copy of them, not from a script on the origin. Proposed: short-lived tokens the bridge renews over the connection (with QR pairing), and the same strict CSP the wallet already relies on.
- **Desktop:** `wallet.json` (with the RPC providers' keys), in the clear; until now created `0644`. macOS and Windows keep its folder private; a Linux `~/.config` is often readable by every account. **Fixed:** the document is written `0600` on Unix, an older copy's mode included (`storage::write_private`). Proposed: the OS keychain (Keychain, Credential Manager, Secret Service) for the token and the providers' keys.

### F3: a phone that went silent (ClientMCP)

**Fixed in ClientMCP, deployed** (`2c6b0b2`, version `ed0aabcd` on `bridge.clientmcp.dev`, 2026-10-27; the previous one `c0b5e511`; the suite there 33 + 1 skipped). On the phones, airplane mode during an agent's call: the Redmi K40's failed "disconnected" after 74 s, the iPhone 15 Pro's after 54 s, the next call "offline" at once (before: the whole call timeout, and more). The fix: the Worker now applies the protocol's idle rule (SPEC §3.2, `CLIENTMCP_CLIENT_IDLE`, 90 s by default, the native bridge's too) — a client is last heard from at the later of its last message and the platform's record of its last answered ping, and one silent past the limit is closed: on any agent request, and every two seconds while a call waits. `tests/run-idle.sh` (3 s idle, 20 s call timeout): a call to a silent client fails in 4 s on the Worker, 3 s on the native bridge. Below is the problem as found.

The Worker answers the client's pings at the edge (`setWebSocketAutoResponse`), so its Durable Object never learns that a client stopped pinging. A call to a phone that lost its network waits for the whole call timeout (120 s) and then fails. The fix belongs to ClientMCP's Worker: on a call, and while it waits, read `getWebSocketAutoResponseTimestamp` and fail the call — and close the socket — once the last pong is older than the client's ping interval and its grace (25 s + 10 s). worker-rs 0.8.7 does not expose that method; it needs a `wasm_bindgen` binding of its own. Then a production deploy.

## Phase 7 notes (iOS)

- **`vela-agent`** (`rust/crates/vela-agent`, E2): `intents` (the runner), `transfers` (drafting onto the shell's send, over `SendDesk`), `tools` (the 32 tools, the model's instructions, `vela_wait`), `connection` (the client on a thread of its own), `bridge_url`, and `shell` — the agent for a shell that answers from its own threads: a blocking `Shell` trait and an `Agent` that runs everything on the agent's thread. The desktop uses the first five directly; its own runner, tools and URL rules are gone.
- **uniffi** (`vela-core-uniffi/src/agent_bridge.rs`): `AgentShell` (what Swift and Kotlin implement: views as JSON, `perform`, `reveal`, `page`, `signing`, `changed`, the send's open/start/dispatch/close/alert, the account, holdings, tracker, network id) and `AgentCore` (configure, link, approvals, activity, answer, stop, `invoke_person`, ceremony, `current_review`).
- **The client's trust roots**: iOS and Android hand out no root store, so `rustls-native-certs` found none there and every `wss://` connect would have failed. The client now uses Mozilla's bundled roots on those two (`webpki-roots`) and the platform's elsewhere (ClientMCP `crates/client`, after `baa89b6`; the vendored copy follows it).
- **iOS** (`Features/Agent/`):
  - `AgentStore` (in `AppGraph`): the settings (`vela.agent`, as the web and the desktop store them), the link, the requests, the person's road in (`person`);
  - `AgentShellBridge`: hops to the main actor; the views are the machines' own JSON (`keepsJson` on the stores read);
  - `AgentPerform`: each action by the store calls iOS's own UI makes — the token sheet's search across networks, a network's bin leaving the balances be;
  - `AgentPage`: what only `RootView` can do (open a screen, open and close a send with its screens, erase, clear a store), registered by it;
  - the send opens on the agent's token and recipient (`SendStore.handIn`); the passkey's ceremony is heard from the send's ports.
- **The person's taps go through the intents**: the same 21 actions as the web's `PERSON`, the review's confirm included (as `submit_transfer` on the review's operation).
- **What the person sees**: Settings → Advanced → AI agent; the capsule, floating above the tab bar (the pages lay their tops out against the window, so at the top it covered the account's name); the approval sheet, a swipe down being No. The agent answers only in the foreground: in the background it hangs up, and what waited is withdrawn.
- **Tests**: `AgentTests` (7) drive the person's taps through the real runner and bridge; `vela-agent` 18 Rust tests (6 for `shell`, over a fake shell). The full unit suite passes (1678, one known issue of its own).
- **Live run** (2026-10-21, simulator, parallel space, `bridge.clientmcp.dev`): state; show; save a contact (its page opened); a BNB transfer drafted onto the review, read through `vela_review`, the agent's submit only asking, cancelled, followed by `vela_wait`; delete a group — Allow, Deny and an approval that timed out; background → offline, foreground → back. Not run live: the person's confirm on chain (the shared code's, confirmed on the desktop), Stop from the capsule (its runner path is tested).
- **Not yet**: iOS's coverage ledger; a token in the Keychain rather than `UserDefaults` (where every other `vela.*` value is).

## Phase 7 notes (Android)

- **The same core** through the same uniffi bridge: `feature/agent/` — `AgentStore` (in `AppContainer`: the settings in `vela.agent`, the link, the requests, the person's road in), `AgentShellBridge` (the controllers' intents and views are thread-safe, so most calls are answered on the agent's thread; the navigation host's and `perform` hop to the main thread), `AgentPerform` (each action by the controller calls Android's own UI makes), `AgentPage` (the navigation host's: the page in front, `show`, opening a send).
- **What the controllers now say** for it: each view as the core wrote it (`viewJson` and friends), the session's included; the send's raw dispatch, whether a send is open (`journeyOpen`, from `open` to `left`), and the passkey's ceremony. Erase and the storage clears moved from the Settings route into the container, so the person's and the agent's take one road.
- **Screens a route keeps** — an asset, a record, a network's code, a contact, a group — are left for that route to take (`AgentStore.detail`), as `pendingFlow` and `pendingSettingsPage` already worked; a Settings page asked for while Settings is up now recomposes it.
- **The 21 commit points** go through the intents, as on the other shells. Settings → Advanced → AI agent; the approval dialog; the capsule above the tab bar, and — on a wallet flow, whose foot is its button and the note above it on a phone's small screen — compact in the header's empty corner. Foreground only, from the container's activity count.
- **Tests**: `AgentTest` (5: the settings record, the bridge-URL rule, the requests and calls as the core writes them, words for every action in two languages, the Settings row); the JVM suite passes (1372).
- **Live run** (2026-10-21, a fresh Android 14 emulator, parallel space, `bridge.clientmcp.dev`): state; show; save a contact (its page opened); a transfer drafted onto the review, read, asked for, cancelled and followed; delete a group — Allow and Deny tapped; background → offline, foreground → back; Stop from the capsule (switched off, offline) and on again from Settings. Found and fixed on the way: the capsule covered Confirm & Send, then the note above it, on the review.

## Open

- Whether `World` crosses uniffi as JSON (simplest; the views already serialize) or as uniffi records. Start with JSON.
