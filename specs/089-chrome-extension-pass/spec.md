# Feature Specification: The Chrome extension pass — smooth, secure, usable, good-looking in a dApp

**Feature Branch**: `089-chrome-extension-pass`
**Created**: 2026-10-01
**Status**: Pass done; nine fix branches ready for review (none pushed); open findings listed in
[results.md](results.md)
**Input**: Owner (2026-10-01): "chrome ext 也要检查，因为 chrome 扩展本质是 web vela wallet 然后可以 inject
到 dapp，要验证功能是否顺畅，安全，能用，美观" — the extension IS the web wallet, packaged as MV3 and
injected into dApps; verify it is smooth, secure, usable and good-looking. Principles: a stable
experience in an unstable environment; one clear, maintainable architecture with less duplication;
rules decided once in vela-core (the MV3 worker cannot run wasm, so it keeps JS twins pinned to the
core by tests).

Tested on a local integration of `origin/main` @ `67e2d193d` + PR #342 (#315, dApps see only the
signed-in account) + PR #343 (#317, every surface opens in the chosen language) — branch
`089-integration`, never pushed — in Chrome for Testing 151 with a throwaway profile, the
parallel-space fixture wallet, the repo's test dApp, hostile pages served on 127.0.0.1, a stand-in
chain and relay, and a per-browser chaos proxy. No real funds moved; no phone was used.

## User Scenarios & Testing

### User Story 1 — A dApp can use Vela without friction, on a good network and a bad one (Priority: P1)

A person installs the extension, signs in (parallel space), and uses dApps: discovery (EIP-6963 and
the legacy `window.ethereum`), connect, chain switching, every signing method, batches, the methods
Vela refuses, disconnect, several tabs, a worker restart mid-request, and a slow or dead network.

**Why this priority**: the extension is only worth installing if a dApp's ordinary calls just work.

**Independent Test**: the extension e2e suites plus the 089 functional matrix (results.md §US1).

**Acceptance Scenarios**:

1. **Given** a connected site, **When** it calls `eth_requestAccounts` or `wallet_requestPermissions`
   again (on every load, or a second Connect click), **Then** it is answered at once and no window or
   panel opens (today a request window flashes open and shut and steals focus).
2. **Given** `main`, **When** the extension e2e suite runs, **Then** it is green (today the three G35
   lifecycle cases are red: they expect the op hash, the code answers "not confirmed yet").
3. **Given** every endpoint of a chain is unreachable, **When** a dApp reads, **Then** it gets the
   chain by name in plain words, `eth_chainId`/`eth_accounts` stay instant, and reads recover by
   themselves when the network returns.
4. **Given** the worker is stopped mid-request, **When** it restarts, **Then** the request resumes
   and is answered once, with no Chrome error text (EX8, already covered).
5. **Given** the toolbar button and an open request window but no wallet tab, **When** it is pressed,
   **Then** the wallet opens in a tab of its own and the request is untouched (today the request
   window is navigated to the wallet and the request answered 4900).

### User Story 2 — A hostile page cannot trick the wallet or the person (Priority: P1)

**Why this priority**: the extension runs in every page the person opens.

**Independent Test**: hostile pages (forged channel messages, cross-origin iframes, deceptive long
hosts, IDN hosts, a public plain-http origin, a request flood), manifest review, the worker's
message checks; results.md §US2.

**Acceptance Scenarios**:

1. **Given** a granted site, **When** it asks `eth_sendTransaction` / `wallet_sendCalls` with a `from`
   that is another account, **Then** it is refused 4100 before any sheet (today the sheet offers to
   send it from the granted account).
2. **Given** a public plain-http origin, **When** it asks for a signature, **Then** it is refused 4100
   "Signing requires a secure origin", as every in-app browser refuses it (today the sheet opens).
3. **Given** a host longer than the sheet's header, **When** it asks for a signature, **Then** every
   character of the host is on screen (today "app.uniswap.org.se…" hides the registrable domain).
4. **Given** any web page, **When** it fetches `chrome-extension://<id>/…`, **Then** nothing is
   served (today `inpage.js` is web-accessible to every site); **and** the content script's own
   world cannot read the wallet's `storage.local` (today it can).
5. **Given** a forged origin, a replayed id, a cross-origin frame, **When** they reach the channel,
   **Then** the window names the true origin, answers once, and the frame reaches nothing (covered).

### User Story 3 — Every surface reads well at its real size, in every theme and language (Priority: P2)

**Why this priority**: the side panel is 360 px; the request window 420 × 760; ru is the longest
locale; a signing sheet that clips an address is a security defect, not only a cosmetic one.

**Independent Test**: screenshots of every surface (side panel via CDP, request window, wallet tab)
in en / zh / ru × light / dark, ru at the largest text size; results.md §US3.

**Acceptance Scenarios**:

1. **Given** the side panel, **When** a send or approval sheet shows, **Then** the full recipient /
   spender address is on screen (today its last characters are clipped).
2. **Given** the panel or ru, **When** the slide control shows, **Then** no letter of its label is
   under the knob (today they are).
3. **Given** ru, **When** the sheet's hero or intent line shows, **Then** it is in the app's sans
   (today Cyrillic falls back to Times).

### User Story 4 — The package is ready for the Chrome Web Store when the owner decides (Priority: P3)

**Independent Test**: research.md §CWS checklist.

**Acceptance Scenarios**:

1. **Given** the manifest, **When** a reviewer reads it, **Then** every permission and host
   permission has a written justification, and nothing is exposed that nothing uses.

### Edge Cases

- A connect from a granted site that arrives before the worker has read storage (cold start): it
  goes to a surface, as before, and the surface answers it the same way.
- A transaction that names no `from`, or the granted account in another case: forwarded, pinned to
  the grant.
- A `personal_sign` whose message is itself 20 bytes of hex: the account is the second param.
- Loopback, `.local` and private-LAN http origins: still allowed to sign (local dApps).

## Requirements

### Functional Requirements

- **FR-001** The extension e2e suite is green on `main` (the G35 cases assert the 083 answer).
- **FR-002** A connect from an origin already granted to the signed-in account is answered by the
  worker, from the rule `eth_accounts` answers with, and opens no surface; the rule is a twin of
  `decide_popup_request`'s connect branch, pinned by test.
- **FR-003** The request surface asks the core for the address a request names
  (`dapp_rpc::requested_address`) and for the origin rule (`is_insecure_public_origin`); it keeps no
  guess of its own.
- **FR-004** The signing sheet never truncates the host, never clips an address, never draws text
  under the slide knob, and never falls back to a serif.
- **FR-005** The toolbar reuses only a normal window's wallet tab.
- **FR-006** No extension file is web-accessible; nothing is externally connectable.
- **FR-007** research.md carries the Web Store checklist and a justification for each permission.
- **FR-008** `storage.local` is readable by the extension's own pages and worker only, never by a
  content script.

### Key Entities

- **The worker's mirrors** — `grantMirror` (grants + snapshot) beside `surfacePreference`: state the
  worker must read synchronously, inside the page's gesture.
- **`PopupRequest`** — the core's question for the request surface, now with `origin` and
  `params_json`.

## Success Criteria

- **SC-001** Extension e2e on `main` + the lifecycle fix: 35/35 (was 32/35).
- **SC-002** A connected site's repeat connect opens 0 windows (was 1 per call).
- **SC-003** A transaction from another account and a public-http signature: 0 sheets, 4100.
- **SC-004** At 360 px, en/zh/ru, light/dark: 0 clipped hosts, 0 clipped addresses, 0 letters under
  the knob, 0 serif Cyrillic in the sheet.
- **SC-005** No regression: web unit, svelte-check, every extension e2e, core workspace tests and
  clippy on the branch that changes the core.
