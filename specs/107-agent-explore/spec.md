# 107 — The agent in Explore, and gates that see every screen

**Status:** approved by the owner (2026-10-27, "开始吧"). Phases 1–3, E8 and E9 built on all four shells (2026-10-27), automated suites green; phase 4 (backup, payment request, remove token) and phase 5 (devices) to do.

## The problem

The owner (2026-10-27): "explorer 页面完全不支持 clientmcp 控制呀，可能还有其他地方也缺少了" ("the Explorer page doesn't support ClientMCP control at all; other places may be missing too").

The owner's agent, connected to the Galaxy S22, reported the same thing. It has 33 tools, none for Explore: it cannot open Uniswap, type a URL or browse a dApp. For dApp requests it can only read the one on the sheet.

A capability a person has and the agent lacks is a defect (spec 104). These were not caught because the coverage gates do not see them. An audit on 2026-10-27 found:

- **No browsing intent on any shell.**
  - Nothing opens a site, moves back or forward, reloads, manages tabs, favorites or recent sites, or sets a site's network.
  - `show` cannot open Explore.
  - The one dApp-related action is `clear_storage_item {item:"dapps"}`, which disconnects every site.
- **dApp requests.** The agent can only read the open signing sheet (`vela_review`). It cannot reject a request, or bring one to the person's attention.
- **The gates miss whole areas:**
  - **iOS:** `ExploreScreen`'s `controller.*` calls are not seen at all, though spec 104 says iOS covers browsing.
  - **Web:** only `.svelte` files are scanned. That misses connect approve/reject, per-site disconnect (`revokeGrant`), the signing sheet's `reject_tapped` (dispatched through a variable), and the Ethereum backup.
  - **Desktop:** `webview::navigate` / back / forward / reload and `opener` are missed.
  - **Android:** Explore is placed, as `later`.
- **Other capabilities with no intent:**
  - disconnecting one site;
  - the Ethereum registry backup;
  - a payment request with an amount;
  - removing a custom token;
  - forgetting one history site.

## Proposed decisions (for the owner)

| | Proposal |
|---|---|
| E1 | **Browsing is the agent's, like navigation.** Opening a site, back, forward, reload, opening and closing tabs, favorites and clearing recent sites need no approval. They show the person a page and change nothing of value; a site still reaches nothing without the person's connect. |
| E2 | **Opening a site is checked like a person's typed URL.** The core's URL rules apply: https only, and the same blocklist and warnings. The approval sheet never shows the agent's words, only the host. |
| E3 | **What a site may do stays the person's.** Connecting a site (its address and chains), approving a signature, a transaction, an added network, or an unlimited allowance is only ever the person's tap on the wallet's own sheet, as for transfers (D5, spec 103). The agent never raises a passkey prompt. |
| E4 | **The agent may answer "no", and may ask.** It may reject a pending dApp request (no passkey, nothing signed). It may also bring a request to the person ("waiting for you", as `submit_transfer` does), which shows the sheet. |
| E5 | **Disconnecting a site and setting a site's network are manage-level:** the agent asks and the person approves in the wallet, like switching accounts. |
| E6 | **The gates see every screen.** Each shell's coverage test scans every view file and every call into a store, controller or browser. A person's action that is not placed fails the build, Explore included. |
| E7 | **The web wallet has no in-app browser** (by design; dApps reach it through the extension). There, `open_site` opens a new browser tab, and the rest is refused as "not on this device". |
| E8 | **(Owner, 2026-10-27) An agent uses only wallets the person allowed it,** and none by default. Settings → AI agent lists the device's wallets, each with an "Allow the agent" switch; the wallet in front when the person first connects is offered on. When the wallet in front is not allowed: the agent sees no balances, activity or contacts, and every action is refused (`wallet_not_allowed`) except `show`. `switch_account` to a wallet not allowed is refused outright, with nothing asked. |
| E9 | **(Owner, 2026-10-27) An approval says what it allows.** A site's network change shows the site's domain, the network it is on, the one it would move to, and the wallet it affects; a disconnect shows the domain and the wallet; a wallet switch shows the wallet it leaves and the one it goes to (`approval_detail_in`, read against the wallet). |

## Design

### New intents (`vela_core::app::intents`)

| Action | Level | What |
|---|---|---|
| `open_site { url, new_tab }` | routine | Explore, at the URL, in the current tab or a new one |
| `browse { to: back \| forward \| reload \| stop }` | routine | the current tab |
| `close_tab { id }` / `close_all_tabs` | routine | |
| `set_site_favorite { origin, favorite }` | routine | |
| `clear_recent_sites` / `forget_site { origin }` | routine | history |
| `disconnect_site { origin }` | manage | the site's grant goes |
| `set_site_network { origin, chain_id }` | manage | the network a connected site sees |
| `reject_dapp_request { id }` | routine | the sheet's reject: nothing is signed |
| `ask_person_dapp_request { id }` | routine | brings the sheet forward; only the person answers |
| `show { screen: explore }` | — | the start page |

`vela_state` gains `explore`: the tabs (URL, title, which is current), favorites, recent sites, connected sites with their networks, and the pending dApp request's kind and origin. Page content is never included. Everything a site supplies, such as titles, is data for the agent, not instructions (the `REVIEW_NOTE` rule).

### Also closing the other gaps

- `backup_registry { chain_id }` (manage): starts the Ethereum copy. The person confirms with their passkey on its sheet, as for transfers.
- `prepare_payment_request { chain_id, token, amount }`: the receive screen with an amount.
- `remove_token { chain_id, address }` (manage).

### Gates

The four coverage tests are widened per E6. A new `later` entry needs a reason the owner can read. The `later` list is reported with every spec that touches the agent.

## Implementation notes (2026-10-27)

- **Core:**
  - 11 actions plus `Screen::Explore` in `intents.rs`. `site_url` checks an address the way the browser does (https; http only on this device's own network) and `site_origin` checks an origin.
  - Rules in `intent_rules.rs`: `ExploreFacts` in `World`, `examine_explore`, `verdict_explore`, approval details.
  - `explore_facts(explore, dapp, history, back, forward, sheet)`: one mapping from the browser machines' own views for every shell with a browser. A request's id is `connect:<tab>`, `sign:<tab>:<id>` or `add_network:<tab>:<id>`, and `dapp_request_target` parses it back.
  - Exported to uniffi as `intentExploreFacts` and `intentDappRequestTarget`.
  - 11 action names in 15 locales; gen-i18n's pin is 1987.
  - The agent's instructions name Explore and say plainly that it cannot approve a dApp's request, or click or type in a page.
- **Desktop:** `WalletPage::agent_explore_facts` and `agent_explore` (`wallet/page/agent.rs`) drive the page's own `browser_go`, tab closing, residents and `BrowserHost`. A signature's reject is the signing host's `RejectTapped`; a network's is `DappAddDeclined`.
- **iOS:** `BrowserController` keeps its three views as JSON (`keepsJson`) for `agentFacts(sheet:)`. `AgentPerform` uses the controller's own calls. `AgentPage.rejectSigning` is the signing sheet's reject.
- **Android:** `BrowserController.agentFacts`, `forgetRecent`, `addFavorite` and `setSiteChain`; `AgentPerform` routes the actions and `VelaNavHost` shows Explore.
- **Web (E7):** `explore` is `{browser:false}`. `open_site` opens a new browser tab (`noopener`); the other actions are refused by the core.
- **Gates (E6):**
  - iOS scans `controller.*` (ExploreScreen) and `signing.reject`.
  - The desktop scans `webview::` and `opener::`.
  - The web adds five residents and a new test for the committing functions screens import from `$lib/dapp`, `$lib/backup` and `$lib/signing`.
  - Android's `browser.*` entries now name their intents.
- **Still `later`:**
  - switching to another open tab;
  - renaming a favorite (desktop);
  - answering or disconnecting extension sites on the web (E7);
  - the Ethereum backup (phase 4);
  - limiting a dApp's approval;
  - sending a report.

### E8 and E9 (2026-10-27)

- **Core:**
  - `World.agent_wallets` and `intent_rules::grants(world)`; `Grants` gains `wallets` and `wallet`.
  - `authorize` refuses `switch_account` to a wallet not allowed, and every action but `show` while the wallet in front is not allowed (`ReasonCode::WalletNotAllowed`).
  - `agent_state` gains `wallet_allowed`, `accounts[].agent_allowed` and `explore`. That last one was missing: `vela_state` never carried Explore until now. When the wallet in front is not allowed, nothing of it is read.
  - `configure_agent` gains `wallets` (person-only). The conformance vector was updated.
- **Shells:**
  - Each stores `wallets` in the `vela.agent` record; it is not a secret.
  - Each shell's views carry `agent_wallets`, and its check builds the grants from them.
  - Settings → AI agent has a switch per wallet, and a warning while none is on.
  - The first clientmcp.dev connection offers the wallet in front.
- **E9:** `approval_detail_in(action, world)`, exported as `intentApprovalDetailIn`, is used by every shell's approval:
  - `app.uniswap.org · Ethereum (1) → Base (8453) · Agent test (0x…)`;
  - a disconnect: the domain and the wallet;
  - a wallet switch: from → to.

## Not in this spec

- Clicking or typing inside a dApp's page ("swap USDC for ETH on Uniswap" end to end). That is automating a third-party page, which is out of scope. The agent can open the dApp and the person does the rest; or, later, the agent prepares the contract call itself through a reviewed transaction intent. That is a separate spec.

## Phases

| Phase | What |
|---|---|
| 1 | Core: the actions, rules, schemas and `explore` view; vela-agent tools; strings |
| 2 | The gates widened on all four shells, first, so they list everything that is missing |
| 3 | Desktop, iOS, Android: perform the browsing and dApp-request actions; web: E7 |
| 4 | The other gaps: backup, payment request, remove token |
| 5 | Device runs on the test wallets: open Uniswap, back, favorite, a dApp connect the agent brings to the person, a reject |
