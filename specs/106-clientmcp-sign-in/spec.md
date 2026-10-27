# 106 — Connect the AI agent with clientmcp.dev

**Status:** approved by the owner (2026-10-27). Phases 1–6 built (2026-10-27): clientmcp.dev on staging; the core, the web, the desktop, iOS and Android, automated suites green. Phase 7 (devices) waits on the phones.

## The problem

To let an AI agent use the wallet today, a person must find a ClientMCP bridge, get a client token for it, and paste both into Settings → AI agent: a bridge address that ends in `/connect`, and a long token. The owner's verdict was "现在感觉很硬核，普通用户很难学会用" ("it feels hardcore; ordinary people can hardly learn it").

clientmcp.dev now issues tokens itself (ClientMCP `docs/self-serve.md`). A person signs in with GitHub. The site creates a client for this installation of the app and hands the app its client token through an OAuth authorization-code redirect with PKCE (RFC 6749, RFC 7636, RFC 8252). The site's side is built and running on staging. This spec is the wallet's side.

## Owner decisions (2026-10-27)

| | Decision |
|---|---|
| D1 | Getting a token is one button: "Connect with clientmcp.dev". The person signs in with GitHub, and the wallet has its bridge address and client token. Nothing is pasted. |
| D2 | The return address on iOS and Android is the private-use scheme `app.getvela.wallet:/clientmcp` (RFC 8252 §7.1), as registered on clientmcp.dev. |
| D3 | The web wallet returns to `https://wallet.getvela.app/clientmcp`. The browser extension build comes later. |
| D4 | The desktop returns to a loopback address, `http://127.0.0.1:<port>/clientmcp` (RFC 8252 §7.3): a one-time listener on a random port. No URL scheme is registered and the installers do not change. |
| D5 | Tokens already issued keep working, and a person's own bridge stays possible: the address and token fields remain, under "Use your own bridge". |
| D6 | The MCP server address for the person's AI agent can be copied from the wallet itself (owner, 2026-10-27: "这样才方便"). clientmcp.dev hands it over with the client token (`agent_url`). The wallet keeps it as it keeps the token, and shows it masked until Show, with Copy, and Share on the phones. |

## Design

### The exchange, once, in the core

The protocol lives in one place, `vela_core::clientmcp_link`. It is pure: no network and no clock. Each shell passes in random bytes and does its own HTTP. It is exported through uniffi to the phones, called directly by the desktop, and exported through wasm to the web.

- `begin(site, redirect_uri, label, installation, random: [u8; 64]) -> Pending`
  - `Pending { authorize_url, state, verifier, redirect_uri }`.
  - The verifier is 32 random bytes in base64url (43 characters). The challenge is its S256 hash. The state is another 32 random bytes.
  - `authorize_url` is `<site>/connect?app=vela&redirect_uri=…&state=…&code_challenge=…&code_challenge_method=S256&label=…&installation=…`.
- `returned(pending, url) -> Result<Code, LinkError>`
  - Reads the redirect: `code` and `state`, or `error`.
  - Refuses a state that is not the pending one, and a URL that is not the pending redirect address. On the loopback the port is the pending one.
- `exchange(pending, code) -> Request { url, body }`
  - `POST <site>/api/token` with a JSON body `{grant_type:"authorization_code", code, code_verifier, redirect_uri}`.
- `accept(status, body) -> Result<Linked, LinkError>`
  - `Linked { bridge_url, client_token, client_id, login, expires_at }`.
  - Refuses an answer whose bridge address is not `wss://` (or `ws://`); the `configure_agent` it is applied through checks the rest, as for a typed address.
  - The site's `error` / `error_description` becomes a `LinkError` the person can read.

The installation ID is 16 random bytes in base64url, made once and kept in `vela.agent.installation`. It is not a secret. Signing in again from the same installation gives it a new client token, and its old one stops: this is reconnecting, not a second client. The label is the device's kind ("iPhone", "Android phone", "Mac", "Windows PC", "Linux PC", "Web browser"). It is what clientmcp.dev lists, and the person can rename it there.

The site is `https://clientmcp.dev`. A development build may name another site (staging) through its build configuration, never through a setting.

### Saving the result

What comes back is applied as the person's own `configure_agent { enabled: true, bridge_url, client_token }` intent, the same path the fields use. It is person-only: an agent never starts, finishes or undoes a link. Beside it, `vela.agent` keeps `linked: { site, login, client_id }`, not a secret, so the panel can show whose it is. A token typed by hand clears `linked`.

The token is stored where it is stored today: the Keychain on iOS, the Keystore on Android, localStorage on the web, `wallet.json` (0600) on the desktop.

### What the person sees (Settings → AI agent)

- **Not connected:** a short explanation, the primary button **Connect with clientmcp.dev**, and below it a quiet "Use your own bridge", which unfolds the address and token fields.
- **While signing in:** the button reads "Waiting for clientmcp.dev…" with Cancel. A sign-in left unfinished expires after 10 minutes.
- **Connected through clientmcp.dev:**
  - "Connected as @login" and the status ("Connected", "Offline · retrying", …).
  - **For your AI agent** (D6): the MCP server address, masked until Show, with Copy and, on the phones, Share. A hint says it gives an agent use of this wallet, so it is to be kept private. Reconnecting replaces only the client token, so this address stays the same and agents already set up keep working. Deleting the client on clientmcp.dev retires it.
  - The allow switch, as today.
  - **Connect your AI agent →**, which opens `<site>/account`, with each agent's own steps (Claude, ChatGPT, Codex and the rest).
  - **Reconnect** (sign in again, which gives a new token) and **Disconnect** (forgets the token here; the client stays on clientmcp.dev until deleted there).
- **Errors** say what happened in plain words: the person cancelled, the site refused (with its reason), no network, or the code expired. Each comes with "Try again".

### Per shell

- **iOS:** `ASWebAuthenticationSession` with `callbackURLScheme: "app.getvela.wallet"`.
  - The session receives the redirect itself, so nothing is added to Info.plist.
  - It is not ephemeral, so a GitHub sign-in in Safari is reused.
  - The exchange uses `URLSession`.
- **Android:**
  - A Custom Tab, using the Trusted Signer's hand-built helper (`TrustedSignerTab`).
  - `MainActivity` gains an intent filter for the scheme `app.getvela.wallet`, routed in `routeDeepLink`.
  - The pending sign-in is kept in preferences (10 minutes), so it survives the process being killed while the browser is in front.
  - The exchange uses the app's HTTP client.
- **Desktop:**
  - Binds `127.0.0.1:0`, opens the browser with `cx.open_url`, and takes exactly one `GET /clientmcp?…` request.
  - Answers that request with a small page, "You can close this tab and return to Vela".
  - Closes the listener on the first request, on Cancel, or after 10 minutes.
  - The exchange goes through the existing HTTP stack, proxy included.
- **Web:**
  - Pending state is kept in `sessionStorage`; the tab navigates to clientmcp.dev and comes back.
  - A new prerendered page `/clientmcp` (outside `[locale]`) reads `code` and `state` in the browser, exchanges them, applies the intent, and replaces itself with `/{locale}/settings`, with the AI agent panel open.
  - The exchange is a cross-origin `fetch`, which needs CORS on clientmcp.dev (below).

### On clientmcp.dev (ClientMCP repo)

- **The return addresses Vela may use:**
  - `app.getvela.wallet:/clientmcp` (registered already);
  - `https://wallet.getvela.app/clientmcp`;
  - the loopback `http://127.0.0.1:<any port>/clientmcp`, where the port may vary (RFC 8252 §7.3) and nothing else may.
- **CORS on `/api/token`, for the web wallet:**
  - `OPTIONS` and `POST` answer `Access-Control-Allow-Origin` only for the web wallet's origins: `https://wallet.getvela.app`, and loopback origins on staging only.
  - The headers allow `content-type` and no credentials.
- **The token response:** `{client_token, bridge_url, mcp_url, agent_url, client_id, login, expires_at}`. `agent_url` (D6) is the agent role's current credential in the path form, the same address the account page copies. The core accepts it only as `https://` (or loopback `http://` for tests).

## Phases

| Phase | What | Where |
|---|---|---|
| 1 | Return addresses (web, loopback) and CORS for `/api/token`, with tests; staging deploy | ClientMCP site |
| 2 | `clientmcp_link` in vela-core with tests and vectors; uniffi and wasm exports; strings in 15 locales | Rust |
| 3 | Web: the panel, `/clientmcp`, the exchange; e2e against a local ClientMCP site | web |
| 4 | Desktop: the panel and the loopback listener | desktop |
| 5 | iOS: the panel and `ASWebAuthenticationSession` | iOS |
| 6 | Android: the panel, the Custom Tab and the intent filter | Android |
| 7 | Run on the phones' test wallets against staging (iPhone 15 Pro, S22), and on desktop and web; then production once clientmcp.dev's site is deployed | all |

## Implementation notes (2026-10-27)

- **Core:** `vela_core::clientmcp_link` (`begin`, `returned`, `exchange`, `accept`, `installation_id`, `loopback_redirect`, `account_url`) and `json`, its JSON face. Exported as `clientmcpLink*` to wasm (`vela-core-wasm/src/clientmcp.rs`) and uniffi (`vela-core-uniffi/src/clientmcp_bridge.rs`). Tests include RFC 7636 appendix B. 16 new `settings.agent.*` strings in 15 locales; gen-i18n's pin is 1968; the SC-005 budget held without a change.
- **Stored:** `vela.agent` gains `linked: { site, login, clientId }` on every shell; a token from anywhere else clears it. `vela.agent.installation` holds the installation ID. The agent's address (D6) is kept where the client token is: `agentUrl` in the record on the web and the desktop, the Keychain (account `agent-url`) on iOS, and the Keystore-sealed `vela.agentUrl` on Android. It is cleared with `linked`. There are 7 more strings, and gen-i18n's pin is 1975.
- **Web:** `$lib/agent/link.svelte.ts`; the `/clientmcp` Worker route 303s to `/{locale}/settings#clientmcp?…`, so the code stays out of server logs, and the page drops it from the address bar at once. `VITE_CLIENTMCP_SITE` names another site in a development build. e2e: `e2e/agent-link.e2e.ts` plays clientmcp.dev (4 tests). Also checked by hand against a local clientmcp.dev (fake GitHub, real site and bridge): connected as @alice, status Connected.
- **Desktop:** `src/agent/link.rs`: a non-blocking listener on `127.0.0.1:0`, polled until the return, Cancel or 10 minutes; a favicon request gets a 404 and it keeps waiting. The exchange goes through `proxy::with_routes`, with no status treated as an error. `VELA_CLIENTMCP_SITE` at build time names another site. Tests cover the whole trip against a played site.
- **iOS:** `Features/Agent/AgentLinkSession.swift` (`ASWebAuthenticationSession`, not ephemeral), `AgentStore.connectWithClientMCP()`. Info.plist's `VelaClientMCPSite` = `$(VELA_CLIENTMCP_SITE)`.
- **Android:** `feature/agent/ClientMcpLink.kt` opens the sign-in in the Trusted Signer's Custom Tab helper. `MainActivity` routes the `app.getvela.wallet` scheme, and the manifest gains its intent filter. The pending sign-in is kept in the store. `-PvelaClientmcpSite=` sets `BuildConfig.CLIENTMCP_SITE`.

## Open

- The browser extension build (`chrome-extension://` origin), later (D3).
- Whether "Connect your AI agent" should later show the MCP address in the wallet itself. For now it lives on clientmcp.dev, so there is one place to copy it from.
- The i18n SC-005 budget may need raising for the new strings. That is the owner's call.
