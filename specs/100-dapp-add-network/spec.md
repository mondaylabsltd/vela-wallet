# Feature Specification: A dApp can ask Vela to add a network

**Feature Branch**: `100-dapp-add-network`
**Created**: 2026-10-04
**Status**: Draft
**Input**: Owner, 2026-10-04 — Vela should implement adding a network from a dApp
(`wallet_addEthereumChain`) for compatibility with most dApps: a dApp that switches to a chain the
wallet lacks should keep working.

## The decision being changed

Spec 070 (`specs/070-dapp-browser-core/spec.md`, edge cases, decision **[D]**) ruled:
`wallet_addEthereumChain` for a chain the wallet **has** is a switch; for an **unknown** chain it is
4902 with the words "networks are added in Vela's Settings". Every client answered that way: the
core `dapp_browser` (desktop, iOS, Android in-app browsers, `dapp_browser.rs` `Route::AddChain`) and
the extension's worker (`background.js` `switchChain`).

So the common dApp flow — `wallet_switchEthereumChain` → 4902 → `wallet_addEthereumChain` with the
chain's parameters → expect the wallet to be on it — dead-ends at the second step, and wagmi reports
it as "user rejected". This spec replaces **[D]**: the person is asked, on Vela's own sheet, and the
network is checked and added exactly as Settings checks and adds it.

## Where it stands (measured 2026-10-04, `main` @ `90bcb02b5`)

- `dapp_rpc::classify("wallet_addEthereumChain")` → `Route::AddChain`; `dapp_browser` answers it
  with the switch path (`dapp_browser.rs:1504-1534`): known chain → `set_site_chain` + `null`;
  unknown → 4902 "Add chain N in Vela's network settings first", record `wallet / unknown_chain`.
- Settings' add-network wizard is the core machine `network_admin`: `ChainSelected` → dedup gate →
  the chain catalog (`/chains/eip155-N.json` from the Ethereum-data service) → the fastest-RPC race
  (`eth_chainId`) → `eth_getCode` of the twelve `REQUIRED_CONTRACTS` and the P256 probe on the winner
  → `Checked` → `AddConfirmed` → `WriteCustomNetworks`. Its words: "Checking compatibility...",
  "Compatible" / "Incompatible", "Unable to verify — RPC request failed", the incompatible hint and
  "Open Chain Setup Tool".
- `network_admin` is one app-wide instance on every client (desktop resident, Android
  `SettingsController`, iOS `SettingsStore`, web `networkAdmin`); its wizard state is shared, so a
  second user of the wizard state would clobber Settings.
- The extension's worker answers addChain itself and the page's bridge (`content.js`) treats it as
  a one-shot read: no surface can be shown for it today.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A dApp switches to a chain Vela lacks, and the person adds it (Priority: P1)

A person opens a dApp on Gnosis in Vela's browser. Vela does not have Gnosis. The dApp asks to
switch, hears "unrecognized chain", and asks to add it. Vela shows a sheet naming the site and the
network — name, chain id, coin, RPC host, explorer — checks that Vela's contracts are on it, and
offers **Add Network**. The person approves; the network appears in Settings and the network list,
the site is on Gnosis, and the dApp continues.

**Independent Test**: from a test page, call `wallet_addEthereumChain` for a chain in Vela's
catalog that the wallet lacks; approve; the page's request resolves `null`, `chainChanged` arrives
with the chain, `eth_chainId` answers it, and Settings lists the network.

**Acceptance Scenarios**:

1. **Given** a chain the wallet already has, **When** a page asks to add it, **Then** it is a
   switch, as before (no sheet).
2. **Given** a chain the wallet lacks, **When** a page asks to add it, **Then** a sheet names the
   requesting site and shows what is being added, and the request waits for the person.
3. **Given** that sheet, compatible, **When** the person approves, **Then** the network is saved
   through the same core path as Settings' add, the site moves to it, `chainChanged` fires, and the
   request answers `null`.
4. **Given** that sheet, **When** the person declines or closes it, **Then** the request answers
   4001 and nothing changes.

---

### User Story 2 — Vela never trusts a page's RPC or names more than its own (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a chain Vela's catalog knows, **Then** the sheet shows the catalog's name and coin
   and the network is saved with the catalog's vetted RPC; the page's `rpcUrls`, name and symbol are
   not used.
2. **Given** a chain the catalog does not know, **Then** only the page's `https://` RPC URLs are
   tried (`http://` only where the debug-mode rule offers http pages the wallet), each must answer
   `eth_chainId` with the requested id, and the sheet says the name and coin are the site's.
3. **Given** a page RPC that answers with another chain id, **Then** the sheet says so ("That RPC
   serves a different network (chain A, expected B)"), nothing can be added, and the request answers
   -32602.
4. **Given** no usable page RPC (none given, or none https), **Then** the sheet says the site gave
   no usable RPC and the request answers -32602.

---

### User Story 3 — An incompatible chain is refused in Settings' words (Priority: P1)

**Acceptance Scenarios**:

1. **Given** a chain missing any of Vela's Safe / ERC-4337 contracts or the P256 precompile,
   **Then** the sheet shows Settings' verdict ("Incompatible", the hint, "Open Chain Setup Tool"),
   Add is not offered, nothing is added, and closing the sheet answers the page 4902.
2. **Given** a chain whose RPCs do not answer, **Then** the sheet says "Unable to verify — RPC
   request failed" with Retry; closing it is the person's decline (4001).

---

### User Story 4 — One sheet at a time, and the record says how it ended (Priority: P2)

**Acceptance Scenarios**:

1. **Given** an add sheet open, **When** any page asks to add another network, **Then** that
   request answers -32002 at once.
2. **Given** the requesting page navigates or its tab closes, **Then** the sheet closes and the
   request ends 4900 "navigated away", as every other request.
3. **Given** any add request, **Then** the tab's request record (spec 099) shows it with its class
   (`consent` when the sheet opened, `local` for a switch), outcome, and on failure its layer and
   reason (`sheet / rejected_by_person`, `wallet / not_compatible`, `wallet / bad_rpc`,
   `wallet / consent_busy`, `wallet / bad_params`, `browser / navigated_away`), with one log line.

### Edge Cases

- The network is added meanwhile elsewhere (Settings, another tab's add): the sheet's machine finds
  it in its ledger and answers "added" without asking; the site switches.
- A Settings wizard in progress while a page asks: untouched — the page's add is separate state.
- The catalog cannot be reached: the request falls to the page's words and RPC — labelled as the
  site's on the sheet (research R2, known limit).
- `nativeCurrency.decimals` other than 18: -32602 before any sheet (Vela counts native coins in 18
  decimals).
- `rpcUrls` absent or empty: not refused up front (the catalog may know the chain); ends as "no
  usable RPC" only when the catalog does not.
- The extension (web): the same sheet in its request window / side panel; the worker keeps "known
  chain → switch" and "-32002 while one is open"; the surface runs the core's check.

## Requirements *(mandatory)*

- **FR-001**: `wallet_addEthereumChain` for a chain in the wallet's networks is a switch
  (unchanged).
- **FR-002**: For a chain not in the wallet's networks, the core reads the page's
  `AddEthereumChainParameter` (`dapp_rpc::add_chain_ask`), refuses a malformed one with -32602, and
  otherwise opens Vela's add-network sheet for that tab and request; the request stays open.
- **FR-003**: One add sheet at a time across all tabs: a second add request while one is open
  answers -32002 ("A request to add a network is already open").
- **FR-004**: The sheet's machine is `network_admin`, in state of its own beside Settings' wizard.
  It runs the wizard's compatibility check — the same step functions, not a copy.
- **FR-005**: Catalog first: when Vela's chain catalog knows the chain, its name, coin, explorer and
  RPCs are used; the page's are not. Otherwise the page's: RPC URLs must pass
  `dapp_rpc::usable_rpc_url` (https; http only where `offers_wallet` lets http pages have the
  wallet) and, on this path, an RPC counts only when it answers `eth_chainId` with the requested id.
- **FR-006**: Verdicts on the sheet: checking · ready · not compatible · unable to verify (Retry) ·
  the page's RPC serves another chain · no usable RPC. Only `ready` offers Add.
- **FR-007**: Approve saves through the wizard's own save (`build_custom_network` +
  `save_custom_network`: `vela.customNetworks`, `last_added_chain_id`), the browser adds the chain to
  its list, moves the site to it (`WriteSiteChain`), emits `chainChanged` to the origin's tabs and
  answers `null`.
- **FR-008**: Answers by outcome, one table for every client (`dapp_rpc::add_outcome_error`):
  added → `null`; declined / closed before a verdict / closed after "unable to verify" → 4001;
  not compatible → 4902; bad RPC → -32602; busy → -32002. The page leaving → 4900.
- **FR-009**: The request record names each ending's layer and reason (US4 AS3).
- **FR-010**: Each shell draws the sheet with its existing consent-sheet components and only
  carries the two machines' messages: `forward_to_add_network` → `dapp_add_requested`,
  `dapp_add_settled` → `add_network_answered`, `cancel_add_network` → `dapp_add_cancelled`.
- **FR-011**: Words: Settings' own wherever they fit; new corpus keys in all 15 locales; the
  i18n residency budget is not raised.

## Success Criteria *(mandatory)*

- **SC-001**: A page's add for a catalog chain the wallet lacks, approved, resolves `null` with
  `chainChanged` and the network in Settings — on desktop, iOS, Android and the extension.
- **SC-002**: The page's RPC is never probed for a chain the catalog knows (a core test asserts the
  probe list).
- **SC-003**: An incompatible chain adds nothing and answers 4902; a decline answers 4001; a second
  request answers -32002 — one core test each.
- **SC-004**: `ja` + `en` residency stays within `SC005_BUDGET` (144,400) with no budget change.

## Assumptions

- "Vela's own chain catalog" is the chain registry Settings' search and wizard read
  (`FetchChainInfo` → `{ethereum-data}/chains/eip155-N.json`).
- The switch after an add is wanted (wagmi treats an add that leaves the page on the old chain as
  "user rejected switch after adding network"), although EIP-3085 does not require it.
- The web wallet has no in-app browser; its dApps reach Vela through the extension.
