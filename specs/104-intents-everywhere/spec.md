# 104 — The agent on every Vela: the intents' judgements in Rust, a native ClientMCP client

**Status:** draft · 2026-10-21
**Follows:** spec 103 (web). The owner asked whether the web agent is ready for iOS, Android and desktop. It is, in design. Three pieces of groundwork come first, so that each shell does not port the same logic again.

## Owner decisions (2026-10-21)

| # | Question | Decision |
|---|---|---|
| E1 | What of the intents moves into Rust | **Only the pure judgements** (part A below). The orchestration stays in each shell (part B), as decided for 103 (D6): a stateful intents core was judged over-abstraction. |

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
| 5 | Desktop: the agent (part B in Rust, on gpui) | Desktop coverage ledger; live agent run |
| 6 | QR pairing | Pair desktop and web in one step |
| 7 | iOS and Android: part B, the approval sheet, the bar, foreground-only | Each platform's coverage ledger; live agent run |

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

## Open

- Whether `World` crosses uniffi as JSON (simplest; the views already serialize) or as uniffi records. Start with JSON.
