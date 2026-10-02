# Feature Specification: The Chrome extension, ready for its first Chrome Web Store submission

**Feature Branch**: `094-chrome-web-store`

**Created**: 2026-10-02

**Status**: Done on the branch (not pushed) — see [results.md](results.md)

**Input**: Lead (2026-10-02): make the Chrome extension ready for its first Chrome Web Store
submission (a test listing) tomorrow, with a beta that feels close to stable. Baseline:
[089](../089-chrome-extension-pass/spec.md). A read-only audit of `main` @ `ec033f231` found four
blockers (B1–B4), twelve should-dos (S1–S12) and three nice-to-haves; the lead's decisions are the
requirements below.

## User Scenarios & Testing

### User Story 1 — The package the store accepts, and the sheet the owner pastes (Priority: P1)

The owner builds one zip, uploads it, and fills the dashboard from one document: every permission
justified, the privacy form answered truthfully, the listing's text and images ready.

**Why this priority**: without it there is no submission tomorrow.

**Independent Test**: `pnpm package:extension`; unzip the `-chrome-web-store` zip and read its
manifest (no `key`, 0.9.6, Chrome 122, no Incognito), list its files (no parallel space, the wasm the
code names at the root); `src/lib/extension/package.test.ts`; `e2e/extension-store-package.e2e.ts`
loads exactly that package; `docs/store-submission/chrome-web-store.md` and its images.

**Acceptance Scenarios**:

1. **Given** a fresh checkout, **When** `pnpm build:extension` runs, **Then** the package carries the
   wasm its code names (B2) and a store package beside it without `key` or developer pages (B1).
2. **Given** the store package, **When** it is installed, **Then** Chrome assigns its id, the welcome
   opens, the core compiles, a site gets the provider, and the parallel space is not there.
3. **Given** the dashboard, **When** the owner pastes the sheet, **Then** every field has an answer
   grounded in code or the claim ledger, and the open judgement calls are marked.

### User Story 2 — A first run that does not stumble (Priority: P1)

A person installs, is welcomed, creates a wallet, connects a dApp — and the extension says plainly
what Chrome does not: tabs open before the install need a reload, limited site access breaks sites
and passkeys (with a one-click grant), and a request window with no wallet leads to one.

**Independent Test**: store-package e2e (welcome, install note, site access grant); signing e2e "connecting with no
wallet"; `background.test.ts` "a fresh install".

**Acceptance Scenarios**:

1. **Given** a fresh install with web pages open, **When** the worker starts, **Then** the welcome
   opens in a tab and asks for those tabs to be reloaded (S3).
2. **Given** site access withheld, **When** any extension page shows, **Then** it says so in plain
   words with an Allow button; a passkey ceremony Chrome refused reads the corpus sentence, not
   Chrome's `SecurityError` (S2).
3. **Given** no wallet, **When** a page asks to connect without a click, **Then** the window says
   "Create a wallet first", opens the welcome in a tab, and turns into the consent once a wallet
   exists (S5).
4. **Given** the extension, **When** the person erases the device, **Then** they land on the
   welcome's document, not `chrome-error://` (S4).

### User Story 3 — A dApp's ordinary calls just work (Priority: P2)

**Independent Test**: lifecycle e2e (a send that lands; a batch whose status is confirmed); connect
e2e (a burst = one window); core `app_dapp_browser`; `calls-status.test.ts`.

**Acceptance Scenarios**:

1. **Given** a batch sent with `wallet_sendCalls`, **When** the dApp asks `wallet_getCallsStatus` /
   `wallet_getCapabilities`, **Then** it gets EIP-5792 answers, in the extension and in every in-app
   browser (S6).
2. **Given** a page firing twelve connects with no click, **When** they arrive, **Then** one window
   opens, one decision answers them all (S7).

### User Story 4 — What is signed is said, offline included (Priority: P2)

**Acceptance Scenarios**:

1. **Given** an unlimited Permit2 permit, **When** the sheet shows, **Then** the danger sentence and
   the "can't be capped here" line show on every shell, with no cap editor (S8).
2. **Given** a network that hangs, **When** a fee is quoted, **Then** it fails within 15 s with
   words and a retry schedule instead of "Estimating…" forever (S9, fee half).

### Edge Cases

- A store package has no `key`: the e2e takes its id from the service worker.
- A window answered while another request of its site is queued: the worker navigates the window to
  the next doorway; the page's own close is only a backstop.
- A worker restarted with a queue and no window: the queue gets one.
- `wallet_getCallsStatus` while the bundler cannot be asked: pending (100), never an error.
- A refresh that hangs past the deadline keeps the quote on screen.

## Requirements

### Functional Requirements

- **FR-001 (B1)** One build writes the development package (keeps `key`, the parallel space), the
  GitHub-release package (keeps `key`, no developer pages) and the store package (no `key`, no
  developer pages); CI builds and uploads the two release zips; the package test covers all three.
- **FR-002 (B2)** The build runs the token check and the wasm copy itself; the package test fails if a
  wasm the built code names is not in the package.
- **FR-003 (B3)** `docs/store-submission/chrome-web-store.md` answers every dashboard field with
  evidence (single purpose, permissions, remote code, data use with reasons, certifications, policy
  URL, category, descriptions, reviewer instructions, trader note, review times).
- **FR-004 (B4)** Three 1280×800 screenshots of the real UI, the 440×280 tile, the 128×128 icon.
- **FR-005 (S1)** `minimum_chrome_version` 122; the site's install page says so in every locale.
- **FR-006 (S2)** Withheld host permissions are detected, explained, and granted back in one click.
- **FR-007 (S3)** A fresh install opens the welcome; tabs open before it are named for a reload
  (no `scripting` permission).
- **FR-008 (S4)** Erase in the extension lands on a document.
- **FR-009 (S5)** The request window with nobody signed in routes to create / sign in.
- **FR-010 (S6)** EIP-5792 status and capabilities: a core rule, every in-app browser, the worker's
  pinned twin.
- **FR-011 (S7)** One request window per site.
- **FR-012 (S8)** One core flag for the unlimited warning, permits included; every shell draws it.
- **FR-013 (S9)** A core bound on every fee quote; every shell answers its timer. Cached balance
  after an offline reload: see results (deferred).
- **FR-014 (S10)** The privacy policy states the forwarding for any site and the Limited Use
  statement.
- **FR-015 (S11)** A version the release rule allows, distinguishable from v0.9.5.
- **FR-016 (S12)** Every `e2e/extension-*.e2e.ts` run on the final build; new coverage for typed
  data v4, `wallet_sendCalls`, a successful `eth_sendTransaction`, connect with no wallet.
- **FR-017 (NICE)** No console line in pages; `incognito: not_allowed`; no parallel space in the
  store package.

## Success Criteria

- **SC-001** The store zip: no `key`, 0.9.6, Chrome 122, Incognito off, 0 developer files, the named
  wasm present.
- **SC-002** Every extension e2e green on the final build, the new ones included.
- **SC-003** Core, web, desktop, iOS and Android suites green (pre-existing failures named).
- **SC-004** The sheet and five images ready under `docs/store-submission/`.
