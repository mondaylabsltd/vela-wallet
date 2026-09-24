# Feature Specification: Close the audit's product gaps

**Feature Branch**: `081-audit-product-gaps`

**Created**: 2026-09-22

**Status**: Draft

**Input**: User description: "Fix the 14 product gaps that the spec-080 audit found and the public docs now disclose, so the 'you actually own it / run it yourself' promise and the security claims hold for the technical, self-hosting buyer. Every fix must be verified to work (tests, and real devices where it touches a native app) and checked for side effects." (full list in [audit-report.md](../080-site-content-accuracy/audit-report.md) "Not fixed here", and restated as FR-001…FR-020 below)

## Context

Spec 080 rewrote getvela.app for the reader who self-hosts and builds from source, and made every page tell the truth — including a list of gaps between what the product promises and what it does. That reader is the paying customer. For them, a disclosed gap is still a reason not to buy. This feature closes the gaps so the disclosures can be removed, not reworded.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — A dApp cannot take over my wallet with one signature (Priority: P1)

A dApp asks the wallet to sign a transaction whose target is the wallet's own account and whose effect is to change who controls it: add or swap an owner, change the threshold, enable a module, set a guard or fallback handler. Today Vela decodes such a request but lets the user sign it, and one signature hands over the account. After this feature, Vela refuses it on every app, whether it arrives alone or inside a batch, and tells the user why.

**Why this priority**: It is the only gap that can lose a user's entire balance in one tap, and the docs currently tell users to protect themselves by hand.

**Independent Test**: Send each kind of request from a test dApp to each app; confirm none can be signed and each shows the explanation; confirm ordinary transactions to other contracts are unaffected.

**Acceptance Scenarios**:

1. **Given** a connected dApp, **When** it requests a call from the wallet to itself that changes owners, threshold, modules, guard or fallback handler, **Then** the app shows the request as blocked with a plain explanation and offers no way to sign it.
2. **Given** a batch that contains one such call among ordinary calls, **When** the dApp submits it, **Then** the whole batch is blocked and the explanation names the call that caused it.
3. **Given** a dApp request to any other contract, or a self-call that does not change control (for example a plain transfer to one's own address), **When** it is submitted, **Then** it behaves exactly as before.
4. **Given** a typed-data signature request that would authorise such a change on the wallet's own account, **When** the dApp asks for it, **Then** it is blocked the same way.

---

### User Story 2 — The services I configure are the services the app uses (Priority: P1)

A self-hoster runs their own relay, public-key index, chain data and exchange rates, and points each app at them. Today the iPhone app cannot save the setting at all, the web app ignores a custom index when creating a wallet or signing in, the desktop app ignores it in a session that starts signed in, and Android still asks Vela's index when naming addresses. The service repositories' own setup files are incomplete. After this feature, every path in every app uses the configured services, and a self-hoster can bring up each service from its repository's instructions.

**Why this priority**: Running it yourself is the product's headline promise and the reason the buyer pays.

**Independent Test**: Point each app at non-default endpoints (a local stub that records requests), exercise create, sign-in, send, name lookup and rates, and confirm no request reaches a Vela default host.

**Acceptance Scenarios**:

1. **Given** the iPhone app, **When** the user edits any of the four service endpoints and saves, **Then** the value persists across restarts, is used by the app, and "Reset to defaults" restores Vela's.
2. **Given** a custom public-key index in any app, **When** the user creates a wallet, signs in on a new device, starts the app already signed in, or views a name for an address, **Then** only the custom index is contacted.
3. **Given** a fresh clone of the public-key index or the relay repository, **When** a self-hoster follows its example configuration and builds its container from source, **Then** registration and relaying work without undocumented settings.
4. **Given** the desktop or web app's settings, **When** the user taps "Self-hosting guide", **Then** the getvela.app self-hosting guide opens.

---

### User Story 3 — What the wallet tells me is true (Priority: P1)

The wallet makes claims on screen: that a transaction's description is "verified", that a network is ready, that an address belongs to a name. It also sends data to services. Today fetched descriptions are labelled verified without being authenticated, the network check passes chains where only the first of several keys can sign, reverse-resolved names are shown without checking they resolve back, the user's private RPC address (which can contain a provider key) is sent to the relay for no reason, and the app still says "12+ networks" when 24 are built in. After this feature, each claim is true.

**Why this priority**: The buyer checks claims; one false label undermines every other one.

**Independent Test**: For each claim, construct the case where it used to be false and confirm the app now says something true.

**Acceptance Scenarios**:

1. **Given** a transaction decoded from a descriptor fetched over the network and not authenticated, **When** the confirm screen shows it, **Then** it is not labelled "verified"; descriptors shipped inside the app (or otherwise authenticated) may be.
2. **Given** a chain that has the contracts for a single-key wallet but lacks Safe's signer factory, **When** the user adds it, **Then** the check reports what is missing and does not mark the network ready for multi-key wallets; contracts wallets never use are no longer required.
3. **Given** an address whose reverse record names "alice.eth" but "alice.eth" resolves to a different address, **When** the address is shown, **Then** the name is not shown for it.
4. **Given** a user whose first-choice RPC endpoint contains an API key, **When** the app talks to the relay, **Then** that endpoint is not sent.
5. **Given** any app or the extension listing, **When** it states the number of networks, **Then** it says 24 (or no number), in every language.

---

### User Story 4 — I can verify what I install (Priority: P2)

A technical user downloads a release. Today the only integrity check is a checksum file uploaded next to the packages in the same release, which proves nothing if the release is compromised; the iPhone app also lacks the privacy manifest the App Store requires. After this feature, every release artifact carries provenance that can be checked independently of the release page, the verification steps are documented, and the iPhone app declares its data use accurately.

**Why this priority**: Important to the buyer's trust and a hard blocker for the App Store, but no funds are at risk today.

**Independent Test**: Cut a release candidate, verify each artifact with the documented command, tamper with one and confirm verification fails; validate the iPhone archive's privacy report.

**Acceptance Scenarios**:

1. **Given** a published release, **When** a user runs the documented verification for any artifact, **Then** it confirms the artifact was built by the project's release workflow from a stated commit; a modified artifact fails.
2. **Given** the iPhone app archive, **When** its privacy report is generated, **Then** every required-reason API it uses is declared with an accurate reason and no undeclared tracking is present.
3. **Given** the Windows installer, **When** the release notes describe it, **Then** they state plainly that it is not code-signed and why.

---

### User Story 5 — Nothing runs behind my back, and what looks like it works does (Priority: P2)

The website still carries five server routes nothing calls, which forward wallet addresses to third parties; the web app's "Send feedback" button does nothing; the desktop "Erase this device" does nothing and the iPhone erase leaves browser data behind. After this feature, the dead routes are gone, feedback is delivered, and erase erases.

**Why this priority**: Privacy hygiene and broken buttons; small individually, noticed by exactly the users who read code.

**Independent Test**: Request each removed route (expect not-found); send feedback from the web app and see it arrive; erase each app and inspect local storage for residue.

**Acceptance Scenarios**:

1. **Given** the website, **When** anyone requests one of the five removed routes, **Then** it no longer exists, and nothing in any app or page called it.
2. **Given** the web app's feedback form, **When** the user sends a report, **Then** it is delivered to the project's issue tracker, or the user is handed a prefilled report to submit themselves if delivery is unavailable.
3. **Given** the desktop or iPhone app with a wallet, contacts, settings and dApp-browser data, **When** the user confirms "Erase this device", **Then** no wallet record, setting, contact, cache or browser data remains, and the app starts as new; passkeys stored with the user's passkey provider are untouched and the user is told so.

---

### Edge Cases

- A self-call that is harmless (a zero-value call with empty data to one's own address, or reading a view function) must not be blocked; only calls that change control are.
- Nested batches: a MultiSend inside a MultiSend, or a delegatecall to MultiSend carrying a self-call, is still caught.
- The guard must not break Vela's own transactions, which delegatecall MultiSend and pay the relay in the same batch.
- A custom index that is unreachable must fail visibly; the app must not silently fall back to Vela's index on paths the user configured.
- Existing users who configured endpoints on the iPhone app before (the page saved nothing) keep defaults — no migration surprise.
- Names: a forward lookup that times out shows no name rather than the unverified one; caching must not keep a name after forward verification fails.
- Descriptor label: a descriptor built into the app and also served by the network keeps "verified" only when the served copy matches the built-in one.
- Network check: networks already added before this change keep working for single-key wallets; the user is told if multi-key wallets cannot sign there.
- Erase on desktop while a dApp browser tab is open; erase interrupted by a crash must not leave half-erased state that looks like a wallet.
- Feedback delivery must not send wallet secrets, full RPC URLs with keys, or more than the user sees in the report.

## Requirements *(mandatory)*

### Functional Requirements

**Self-hosting**

- **FR-001** (item 1): The iPhone app's Service Endpoints page MUST show the current values, save edits, validate them the way the other apps do, persist across restarts, and restore defaults — driven by the same shared rules as the other apps.
- **FR-002** (item 2): Every path that contacts the public-key index — wallet creation, sign-in and recovery, sessions that start signed in, and name lookups — MUST use the index the user configured, on web, desktop, Android and iPhone.
- **FR-003** (item 3): The public-key index repository's example configuration MUST include every setting required for registration to succeed, and the index and relay repositories' source container builds MUST build from a clean clone.
- **FR-004** (item 4): The desktop and web "Self-hosting guide" entries MUST open https://getvela.app/docs/self-hosting.

**Security and truthful claims**

- **FR-005** (item 5): The wallet MUST refuse to sign any dApp-originated request — transaction, batch, or typed-data authorisation — whose effect includes a call from the account to itself that changes owners, threshold, modules, guard, module guard or fallback handler, including when nested in batches; the refusal MUST be decided once in the shared core and shown with an explanation in every app.
- **FR-006** (item 5): Vela's own transactions (including its batched fee payment) MUST continue to work unchanged.
- **FR-007** (item 6): The apps MUST NOT send the user's RPC endpoint addresses to the relay.
- **FR-008** (item 7): A transaction description MUST be labelled "verified" only when its descriptor is authenticated (shipped with the app, or matching the shipped copy); fetched, unauthenticated descriptors MUST carry an accurate lesser label.
- **FR-009** (item 8): The network admission check (apps and chain-setup page) MUST include Safe's passkey signer factory and its singleton, MUST report multi-key readiness separately from single-key readiness, and MUST stop requiring contracts the wallet never uses.
- **FR-010** (item 14): A name obtained by reverse lookup MUST be shown only if resolving that name forward returns the same address, on every app; a failed or timed-out forward lookup shows no name.
- **FR-011** (item 14): App copy and the extension listing MUST NOT understate the built-in network count; every locale MUST agree with the 24 built-in networks or state no number.

**Releases and store readiness**

- **FR-012** (item 9): Every release artifact MUST carry provenance or a signature verifiable independently of the release page, with a documented verification command; tampering MUST be detectable.
- **FR-013** (item 9): Release documentation MUST state that the Windows installer is not code-signed and what that means for the user.
- **FR-014** (item 10): The iPhone app MUST ship a privacy manifest declaring every required-reason API it uses with an accurate reason, and its data-collection declarations MUST match the privacy policy.

**Hygiene**

- **FR-015** (item 11): The five dormant website routes MUST be removed after proving no app, page or service calls them.
- **FR-016** (item 12): The web app's feedback "Send" MUST deliver the report (via the website's bug-report endpoint) or fall back to a prefilled report the user submits; the report MUST contain only what the user sees and approves.
- **FR-017** (item 13): "Erase this device" MUST remove all local wallet records, settings, contacts, caches and in-app browser data on desktop and iPhone, and MUST say that passkeys held by the passkey provider are not removed.

**Verification and side effects**

- **FR-018**: Every fix MUST have an automated test that fails before the change and passes after, where the behaviour is testable without a device.
- **FR-019**: Every fix that touches a native app MUST be exercised on a real device (Android: Xiaomi 9d5f42fb; iPhone 11) before it is called done, with the evidence recorded.
- **FR-020**: When a gap is closed, every public page and document that disclosed it (site docs in all 15 locales, README, docs/ARCHITECTURE.md, claim ledger) MUST be updated in the same change, and no page may claim a fix that has not shipped.

### Key Entities

- **Self-call guard verdict**: the core's decision on a request — allowed, or blocked with the offending call and a reason; consumed by every app's signing screen.
- **Service endpoint settings**: the four user-configurable service addresses per app, with validation state and defaults.
- **Descriptor provenance**: where a transaction description came from — built in, fetched and matching built-in, or fetched and unauthenticated — which decides its label.
- **Network readiness**: per network, readiness for single-key and for multi-key wallets, with the missing pieces listed.
- **Verified name**: an address-to-name association shown only after the forward check succeeds.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the control-changing self-call cases in the test matrix (each function alone, in a batch, nested, and as a typed-data authorisation) are blocked on all four apps; 0 regressions in the existing transaction test suites and in a real send on each native app.
- **SC-002**: With every endpoint pointed at a request-recording stub, a full create → sign-in → send → name-lookup session on each app makes 0 requests to Vela's default service hosts.
- **SC-003**: A self-hoster following only each service repository's README and example configuration brings up the index and the relay from source with 0 undocumented steps.
- **SC-004**: 0 on-screen claims in the audited set (verified label, network ready, name shown, network count) are false in the test cases built to break them.
- **SC-005**: 100% of artifacts in a release candidate verify with the documented command, and a single modified byte makes verification fail.
- **SC-006**: The iPhone archive's privacy report lists every required-reason API used, with 0 undeclared.
- **SC-007**: The five removed routes return not-found; a feedback report sent from the web app arrives; after erase, a scan of local storage on desktop and iPhone finds 0 wallet, contact, setting or browser-data residue.
- **SC-008**: Every gap closed here is removed from the site's "known gaps" disclosures in all 15 locales, and no disclosure remains for a gap that still exists or is removed for one that does.

## Assumptions

- **Self-calls are blocked outright, with no override.** Vela never builds such calls itself, keys are fixed at creation by design, and a user who truly needs to change modules has no Vela flow that requires it. If the founder wants an expert override later, it is a separate feature.
- **"Verified" is kept only for authenticated descriptors** rather than adding a signing scheme for the descriptor server now; a signed descriptor index can come later and would upgrade fetched descriptors to "verified".
- **Release provenance uses keyless, workflow-bound attestations** (no long-lived signing key to manage); Windows Authenticode needs a purchased certificate and is out of scope beyond documenting it.
- **Feedback delivery** uses the existing website bug-report endpoint with its server-side token; where the endpoint is not configured, the prefilled-report fallback is the delivered behaviour.
- **Erase does not remove passkeys** — they belong to the user's passkey provider and no app can delete them.
- **Vela registry wallet names** (labels from the on-chain registry) are not name-service records and are not subject to forward verification; they stay labelled as wallet names.
- Work in the relay and index repositories ships as separate PRs in those repositories; this spec tracks them.
- Real-device verification uses the connected Xiaomi (Android) and iPhone 11; the S22 remains the founder's real account and is never used to sign.
