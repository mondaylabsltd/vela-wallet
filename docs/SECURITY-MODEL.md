# Vela Wallet: security model

This is the one document a reviewer should read first. It says what Vela protects, whom it
trusts for what, what each defence guarantees and what it does not, where every override is,
and what is known to be open. Every claim cites the code or a test. Each citation is checked
mechanically on every pull request (`scripts/ci/check-doc-claims.mjs`): a path or symbol named
here that does not exist fails CI. That check proves the citation exists. It does not prove the
code does what the sentence says; that is the reader's job, and the reason the citations exist.

**Verified against commit `fa6083c87`** (branch `feat/multisig-account`, 2026-11-09). Where this
document and the code disagree, the code is right and this document has a bug. Please
[report it](../SECURITY.md).

---

## Contents

1. [Scope and status](#1-scope-and-status)
2. [System overview](#2-system-overview)
3. [Assets](#3-assets)
4. [Trust assumptions](#4-trust-assumptions)
5. [Adversaries and outcomes](#5-adversaries-and-outcomes)
6. [Defences: guarantees and non-guarantees](#6-defences-guarantees-and-non-guarantees)
7. [Fail-open and "… anyway" inventory](#7-fail-open-and--anyway-inventory)
8. [Keys and accounts](#8-keys-and-accounts)
9. [The multisig model](#9-the-multisig-model)
10. [Network and RPC](#10-network-and-rpc)
11. [Privacy](#11-privacy)
12. [Build and release verifiability](#12-build-and-release-verifiability)
13. [Known gaps and accepted risks](#13-known-gaps-and-accepted-risks)
14. [Disclosure and advisories](#14-disclosure-and-advisories)
15. [Document changelog](#15-document-changelog)

---

## 1. Scope and status

**What this covers.** The code in this repository at the commit above: the shared Rust core
(`rust/crates/vela-core`), the shells that render it (web wallet and Chrome extension in
`app-web/vela-wallet`, Safari extension, iOS, Android, desktop), the signing page
(`app-web/trusted-signer`), and the website's security-relevant parts (`app-web/getvela.app`:
the app-association files, headers, analytics). It describes the services Vela runs (relay, key
index, chain data, rates, the multisig genesis service and mailbox) only as the wallet sees
them. Their code is in other repositories (see [SECURITY.md](../SECURITY.md#scope)), and this
document makes no claim about their internals.

**Release status.**

| | State at this commit |
| --- | --- |
| Latest release | v0.9.8 (`main`). It predates every Round 21 and Round 22 change below: no SafeOp expiry, no `settlementGas` cap, no two-provider corroboration, no hold-to-confirm, no parent-Safe guard, no ERC-6492. A Vela account that owns another Safe is exposed to [VELA-2026-002](security/advisories/VELA-2026-002-parent-safe-authority.md) in v0.9.8. |
| This branch | 1,483 commits ahead of `main`, including the multisig feature. **Not released.** |
| Multisig services | The genesis service's and mailbox's Round 22 changes are uncommitted in their own repository and not deployed ([hardening §10](security/hardening.md#10-the-mailbox-cannot-stall-or-phish-a-multisig-review-1-13-wallet-side)); `docs/multisig-vault-design.md` still lists deploying the service and mailbox, and a small mainnet run with real passkeys, as pending. No mainnet end-to-end multisig run is on record. |
| Signing page | The newest pages are built and committed here; deploying them is an owner go-live step, and `integrity::LAUNCH` (the version the phones open) is still an older page that is to move after that deployment. See [§6.7](#67-the-signing-page) and the go-live lists in [hardening.md](security/hardening.md#go-live-checklist-the-owner). |
| Audits | Vela's own code is not audited, and no audit is scheduled. The contracts it uses are; see [Audits & known issues](https://getvela.app/docs/security-audits). |

**Where the detail is.** Every decision summarised here, with its reasons and the proofs run, is
in [`docs/security/hardening.md`](security/hardening.md) (Rounds 21 and 22). The multisig
machines are in [`docs/multisig-machines.md`](multisig-machines.md) (§28–§38 for the security
rounds). The review that prompted Rounds 21–23 was an internal review of 2026-11-07; its items
are cited below as "review 1-6" and so on.

---

## 2. System overview

One Rust core makes every decision; each platform's shell renders what the core returns and
performs the I/O the core asks for (`docs/ARCHITECTURE.md`). The trust boundaries:

```text
                         ┌──────────────────────── the person's device ────────────────────────┐
                         │                                                                      │
  dApp (web page,        │   Shell: web / extension / iOS / Android / desktop                   │
  in-app browser) ──────▶│     │ renders views, runs effects (passkey, storage, HTTP)           │
  EIP-1193 / 6963        │     ▼                                                                │
                         │   vela-core (Rust; wasm on the web, UniFFI on phones, crate on       │
                         │   desktop): clear signing, guards, fee rules, simulation verdicts,   │
                         │   multisig rules, the SafeOp hash it lets a key sign                 │
                         │     │                                   │                            │
                         │     │ challenge (a digest the core       │ hand-off (optional venue)  │
                         │     │ computed and checked)              ▼                            │
                         │     ▼                               Signing page (sign.getvela.app,  │
                         │   Authenticator ◀──────────────────  or opened from hashed bytes;    │
                         │   (platform passkey, phone over      one file, CSP default-src none) │
                         │    hybrid/caBLE, USB/NFC key)                                        │
                         └───────┬──────────────────────┬────────────────────────┬──────────────┘
                                 │ signed UserOperation  │ reads                  │ lookups / records
                                 ▼                       ▼                        ▼
  RP ID "getvela.app" ─┐   Relay (bundler,         RPC pool (public nodes,   Key index ─▶ registry
  app associations,    │   in-band fee)            the person's node,         contract (Gnosis,
  DNS, TLS             │      │                    two providers per          Ethereum backup)
  (the domain)         │      ▼                    verdict read)             Chain data (tokens,
                       │   EntryPoint v0.7 ─▶ Safe v1.4.1 ─▶ Safe4337Module   descriptors), rates,
                       │   passkey module ─▶ P-256 precompile 0x100           genesis service +
                       │                                                       mailbox, PeerConnect
                       └─ decides which apps and pages may use the passkeys
```

What crosses each boundary, and what checks it:

| Boundary | Crosses | Checked by |
| --- | --- | --- |
| dApp → shell | a JSON-RPC request | the core's request readers and guards ([§6](#6-defences-guarantees-and-non-guarantees)); nothing a dApp writes is a field the core trusts to choose a path (`docs/multisig-machines.md` §31) |
| core → authenticator | a WebAuthn challenge | the core computes the digest itself and refuses to hash an operation that differs from the reviewed one (`signed_review::reviewed_safe_op_hash`, `signed_review::dapp_safe_op_hash`) |
| shell → signing page | a request; the page re-derives the digest | the page decodes with its own code and signs only its own digest; the core accepts only the digest it computed ([§6.7](#67-the-signing-page)) |
| device → relay | a signed UserOperation | the signature: the relay cannot change it. It can delay, refuse or time it ([§5](#5-adversaries-and-outcomes)) |
| device → RPC | reads | two independent providers for every read a verdict stands on ([§6.4](#64-two-provider-simulation-and-the-state-diff-guard)) |
| device → key index | lookups, registrations | checked against the registry contract on chain ([§6.12](#612-key-index-verification)) |
| device → chain data | token lists, ERC-7730 descriptors | a fetched descriptor is never "verified" and never grades green ([§6.1](#61-clear-signing-and-provenance)) |
| the domain → OS / browser | which apps and pages may use `getvela.app` passkeys | the association files ([§4](#4-trust-assumptions)); the domain itself is trusted |

---

## 3. Assets

| Asset | Where it lives | Lost if… |
| --- | --- | --- |
| Funds in a Vela account | a Safe v1.4.1 on each chain, counterfactual until its first transaction there | any one founding key signs a transfer (1-of-N) |
| Multisig funds and authority | a Safe owned by Vela accounts, EOAs or contracts, M-of-N | M owners' approvals are obtained, or one owner meets the threshold alone (1-of-N) |
| The passkeys (P-256 private keys) | the person's authenticators; synced passkeys also in the Apple / Google / password-manager account | never leave the authenticator; a signature is obtained by asking the authenticator |
| The RP ID `getvela.app` and its domain | DNS, the registrar, TLS, `/.well-known/` files | whoever controls it can serve pages and app associations that ask any `getvela.app` passkey for a signature |
| Account discovery data | the key index and the registry contract (public keys, credential ids, names, the address) | it is public by design; integrity matters, secrecy does not ([§6.12](#612-key-index-verification)) |
| Privacy: address ↔ IP ↔ identity | every service request carries an IP; the relay sees operations | [§11](#11-privacy) |

---

## 4. Trust assumptions

One row per component. "Trusted for" is what the component can break if it is wrong or
hostile. "Not trusted for" is what the wallet checks itself.

| Component | Trusted for | Not trusted for |
| --- | --- | --- |
| **Contracts, per chain**: Safe v1.4.1 (`safe::SAFE_SINGLETON`, `safe::SAFE_PROXY_FACTORY`), Safe4337Module v0.3.0 (`safe::SAFE_4337_MODULE`), Safe passkey module v0.2.1 (`safe::WEBAUTHN_SIGNER`, `safe::WEBAUTHN_SIGNER_FACTORY`), EntryPoint v0.7 (`safe::ENTRY_POINT`), MultiSend, Multicall3 | executing exactly what was signed, verifying the passkey signature | — (they are unmodified and audited upstream; Vela holds no role on them) |
| **The P-256 precompile at `0x100`** (EIP-7951 / RIP-7212) | verifying every passkey signature; there is no fallback verifier | presence: a network is only added after a real signature check against it (`network_admin::P256_PRECOMPILE`) |
| **Multisig contracts**: CompatibilityFallbackHandler, MultiSendCallOnly (`multisig::MULTI_SEND_CALL_ONLY`), SafeToL2Setup, SafeGenesisRegistry v4 | a multisig's setup and execution; the registry records a multisig's genesis and name | the registry is an index, not authority: authority is read from the chain ([§9](#9-the-multisig-model)). **SafeGenesisRegistry is not audited.** |
| **The key index's registry contract** (Gnosis, with an Ethereum backup copy) | a durable public record of which keys found which address | it holds no funds; the wallet recomputes addresses from it |
| **Authenticators and sync providers** (iCloud Keychain, Google Password Manager, password managers, security keys) | keeping the private key secret; performing user verification on every signature | the content of the signature: the OS prompt names the RP ID only, never the transaction |
| **The domain `getvela.app`** (registrar, DNS, TLS) | deciding which pages and apps can use `getvela.app` passkeys | — (it is a single point of control; [§5](#5-adversaries-and-outcomes), [§8](#8-keys-and-accounts)) |
| **App associations** (`app-web/getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`, `app-web/getvela.app/src/routes/.well-known/apple-app-site-association/+server.ts`) | which native apps may use `getvela.app` passkeys. **Exactly two identities**: Android `app.getvela.wallet` with release-key SHA-256 `A3:8E:36:FE:5A:99:AE:30:73:F7:91:4C:3D:43:58:32:AE:BD:D9:7D:C1:A4:DF:EF:9E:4B:3A:6E:10:31:9D:53` (`get_login_creds` and `handle_all_urls`), and iOS `F9W689P9NE.app.getvela.VelaWallet` (`webcredentials`; app links for `/sign`, `/pay`, `/pay/`). No `/.well-known/webauthn` related-origins file is served. | pinned by `assetlinks.test.ts` and `association.test.ts`; test builds use their own RP ID (`service_defaults::relying_party_id`, [hardening §6](security/hardening.md#6-test-builds-have-their-own-passkey-domain-review-0-1-follow-up)). When Play App Signing is enabled its certificate is to be added, and nothing else. |
| **The app code you run** (the web wallet as served, an extension or app build) | everything it renders and every request it asks the authenticator to sign | — (this is the Bybit class: [§5](#5-adversaries-and-outcomes), [§6.7](#67-the-signing-page), [§12](#12-build-and-release-verifiability)) |
| **The Chrome extension's permissions** (`app-web/vela-wallet/extension/manifest.json`) | it holds `https://getvela.app/*` (so it can use `getvela.app` passkeys) and `*://*/*` (to inject the provider) | — |
| **RPC nodes** | liveness; balances and history shown (one node's word) | a signing verdict: simulation, state diff, code and authority reads need two independent providers, or the person's own node ([§6.4](#64-two-provider-simulation-and-the-state-diff-guard)) |
| **Relay** (vela-relay, `service_defaults::BUNDLER_SERVICE_URL`) | inclusion and timing within the operation's validity window; quoting the gas price | the operation's content, the fee amount and recipient (signed); `settlementGas` above the cap ([§6.11](#611-the-settlementgas-cap)); a per-gas price above 3× the wallet's own reading |
| **Key index** (`service_defaults::PASSKEY_INDEX_URL`) | availability of sign-in on a new device | which keys co-own an address: checked against the registry on chain, and an unchecked multi-key answer shows no receive address ([§6.12](#612-key-index-verification)) |
| **Chain data** (`service_defaults::ETHEREUM_DATA_URL`) | token lists, logos, ERC-7730 descriptors as served | grading a call green or "verified" ([§6.1](#61-clear-signing-and-provenance)) |
| **Rates** (`service_defaults::FIAT_RATES_URL`) | the fiat figure shown, and the token amount a fiat-denominated entry becomes | — (a wrong rate changes how much a fiat amount sends; the token amount is shown before signing) |
| **Genesis service and mailbox** (`service_defaults::GENESIS_SERVICE_URL`) | availability of a multisig's genesis record and proposal list | authority, hashes, approvals: recomputed and re-verified on the device ([§9](#9-the-multisig-model)) |
| **PeerConnect relay** (`service_defaults::PEERCONNECT_RELAY_URL`) and its token endpoint, with PeerConnect's trial-token fallback (`service_defaults::PEERCONNECT_TRIAL_URL`) | carrying an outside owner's wallet session for a multisig | the signatures it carries: checked by the core (ecrecover) before use |
| **Passkey model directory** (`service_defaults::AAGUID_DIRECTORY_URL`) | naming an authenticator model | — (display only) |
| **Analytics host** `tj.appsdata.org` (Rybbit) | — | the web wallet and extension send nothing unless the person turns statistics on (`app-web/vela-wallet/src/lib/analytics/consent.svelte.ts`); the website getvela.app loads Rybbit's script on every page except `/chain-setup` (`app-web/getvela.app/src/routes/+layout.svelte`). [§11](#11-privacy) |

---

## 5. Adversaries and outcomes

| Adversary | Can | Cannot | What stops it | What does not |
| --- | --- | --- | --- | --- |
| **Malicious dApp** (connected) | ask for any transaction, batch or signature; show a misleading page | sign without the person; take the account over through a self-call, a delegatecall leg, a `SafeTx`, or the account's authority over a Safe it owns | clear signing; the self-call and parent-Safe guards ([§6.2](#62-the-self-call-and-parent-safe-guards)); the approval guard; two-provider simulation; WYSIWYS binding; SIWE binding | a transaction the person reads and approves that is harmful in substance (a legitimate-looking swap to a bad pool, an approval to a drainer the person accepts) |
| **Phishing website** (not on `getvela.app`) | imitate Vela | use `getvela.app` passkeys: browsers offer them only to `getvela.app` and its subdomains | WebAuthn's RP ID rule; the website's own pages deny themselves passkeys (`app-web/getvela.app/src/lib/security-headers.ts`); only `wallet.getvela.app` and `sign.getvela.app` keep them | a person who types a seed-like secret or approves something on a site that uses its own keys |
| **Compromised wallet frontend or CDN** (the served web wallet, an extension update) | show anything; ask the `getvela.app` passkey for any signature, directly | — | for a page-only or custom-domain account, the page is the authority ([§6.7](#67-the-signing-page)); otherwise only build verification after the fact ([§12](#12-build-and-release-verifiability)) | **this is not stopped for `getvela.app` accounts.** The signing page gives display assurance, not authority (`signing_venue.rs`, "What this does NOT claim") |
| **Compromised Vela service** — relay | delay, refuse, reorder or time an operation inside its 30-minute window; set the per-gas price, up to 3× the wallet's own reading | change what was signed; bill gas above the cap; land an operation after `validUntil` | the signature; `user_op::SAFE_OP_LIFETIME_SECS`; `fee_policy::settlement_gas_cap` | censorship (switch relays or run your own; there is no self-submit button); a swap landed at the worst moment within its slippage and window |
| — key index | refuse sign-in; return a wrong group | make the wallet show an attacker-co-owned address to receive at | `registry_resolve::identity_of`, `app::receive_standing`, `registry_resolve::confirm_step` | a person who signs with an unconfirmed account (it can sign; it cannot receive) |
| — chain data | serve wrong token lists, logos, descriptors | make a descriptor "verified" or a call green | `ClearProvenance::is_verified`; the provenance floor in `project_result` | a wrong but plausible description of a call, graded Normal or worse, which the person believes |
| — rates | misstate fiat values | change a token amount after it is shown | the token amount on the confirm screen | a person who enters a fiat amount and does not read the token figure |
| — genesis service / mailbox | drop, delay or invent proposals; link owners to Safes (privacy) | make a member sign a hash it did not compute; count a non-member's approval; stall the nonce with far-ahead posts | `read_proposal` recomputes and re-verifies; `next_nonce_of`; `from_non_member` ([§6.14](#614-the-mailbox-rules)) | liveness: a dropped proposal is lost to the members who did not see it |
| **Malicious RPC** (one provider) | lie about balances, history, simulation, state diff, code, owners | give a plain (non-"anyway") verdict alone | two independent providers per verdict read (`app::corroborate`); chain-id probe | balances and history (one node's word); two colluding providers, or one backend behind two brands (provider independence is a domain heuristic) |
| **Malicious co-signer** (multisig) | propose anything; withhold approval | make another member sign a hash its own device did not compute; execute without M approvals | per-device review; the state-diff guard; safe-shape rules | social engineering of the other members |
| **Mailbox spammer** (stranger) | post proposals for any Safe | stall the nonce; appear as "needs your approval" | `from_non_member`; server `nonceTooFar` (not yet deployed) | the posts themselves are stored and readable by anyone |
| **Malware or a rogue native app** on the device | ask an authenticator for a `getvela.app` signature over CTAP (it writes its own `clientDataJSON` origin, `ctap::ceremony::client_data_json`); a browser extension with `getvela.app` permission can use the passkeys | — | nothing in Vela. App associations limit which apps the OS lets use platform passkeys; they do not gate CTAP | **not in scope to stop**: software you run is trusted (SECURITY.md "Out of scope") |
| **Device theft** | hold the device | sign without user verification | the authenticator's user verification on every signature | a thief who passes it (knows the PIN): then move funds ([§8](#8-keys-and-accounts)) |
| **Passkey-sync account takeover** (Apple / Google account) | obtain the synced passkey | — | — | **nothing in Vela stops this.** A synced key is as strong as the account behind it; a security key does not sync |
| **Domain, registrar or DNS takeover** | serve pages and association files that use every `getvela.app` passkey | — | — | **not stopped.** Domain controls (registry lock, DNSSEC, CAA, HSTS preload) are not documented here ([§13](#13-known-gaps-and-accepted-risks)) |
| **Supply chain** (dependencies, vendored code, CI, the committed wasm) | ship code into a build | change the committed web core without CI noticing | cargo-deny, JS audits, Dependabot, the PeerConnect rebuild check, the byte-for-byte web-core rebuild ([§12](#12-build-and-release-verifiability)) | a malicious dependency that passes audit; native builds are attested, not reproducible |
| **AI agent / prompt injection** (spec 103; off unless the person turns it on) | read reviews, draft transfers, give an opinion in its own card | confirm a transfer; change what is signed, a button or a risk | `rust/crates/vela-core/src/app/intents.rs` (an agent's transfer only asks; the person confirms); `rust/crates/vela-core/src/app/agent_review.rs` (requester words reach the agent as untrusted data) | a person persuaded by the agent's opinion |

---

## 6. Defences: guarantees and non-guarantees

Each defence names the rule (the core owns it; every shell renders it), what it guarantees,
what it does not, and the tests or vectors that pin it. Vectors under `tests/vectors/` are
replayed by the core and by every shell through the real core.

### 6.1 Clear signing and provenance

- **Rule.** `rust/crates/vela-core/src/app/clear_signing.rs`: calls and EIP-712 messages are
  decoded with ERC-7730 descriptors (built in, fetched from chain data, or a token-standard
  shape), then a public selector database as a last resort. Each reading records where its bytes
  came from (`ClearProvenance`). Only `BuiltIn` and `PinnedMatch` are "verified"
  (`ClearProvenance::is_verified`), and every other reading is floored at Normal risk by
  `project_result`, so only the app's own words can grade a call green
  ([hardening §14](security/hardening.md#14-only-the-apps-own-words-grade-a-call-green-review-1-4)).
  Typed-data requests are read as exactly one document; a request with more than one is refused
  (`rust/crates/vela-core/src/typed_data_request.rs`, the VELA-2026-001 fix).
- **Guarantees.** The sheet shows Vela's own decoding of the bytes that will be signed, with the
  provenance said. A compromised descriptor service cannot make a call "verified" or green.
- **Does not guarantee.** That a Fetched or SelectorDb description is true. That an EIP-712
  message is harmless: typed data can authorise asset movement (Permit, Permit2, orders), and
  the guards in §6.2–§6.3 apply to it. Recipient facts ("is it a contract", "first time") and the
  `no_code` probe are one node's word; they only add cautions.
- **Tests.** `tests/app_clear_signing.rs`; `tests/typed_data_wysiwys.rs`;
  `r22_signing_truth.rs` (`a_fetched_descriptor_saying_claim_is_never_graded_safe`).

### 6.2 The self-call and parent-Safe guards

- **Rule.** `self_call_guard::detect_self_call` refuses, with no override, a dApp request for a
  call from the account to itself (owner, threshold, module, guard, module-guard,
  fallback-handler, `setup`, `execTransaction`, `execTransactionFromModule`), any `delegatecall`
  leg, and a `SafeTx` typed-data signature, recursing through `multiSend` and `execTransaction`
  to depth 4 (`SelfCallFunction`). Empty-calldata self-calls are allowed (the fee leg and the
  estimator use them). `parent_safe_guard::verdict` extends this to **every Safe the account
  owns**: `execTransaction`, `approveHash` (`parent_safe_guard::APPROVE_HASH`), the admin list, and
  `SafeTx` / `SafeMessage` typed data under that Safe's domain. "Owns" is the device's multisig
  list, or an `isOwner(account)` read (`SignOperation::ReadSafeOwner`) that must be corroborated
  by two providers; `true`, an unreadable answer or a disagreement refuses. The submit step
  checks the final params again (`parent_safe_guard::enforce`). The same self-call rule holds at
  a connected multisig's own address (`multisig_dapp::self_call_refusal`, SEC-PARENT-2).
- **Guarantees.** A connected site cannot obtain a signature that changes who controls the
  account or a Safe it owns, through any of these shapes, on any shell or the signing page.
- **Does not guarantee.** Protection against a shape outside the list (a future Safe version's
  new admin function, a module's own admin calls on a Safe that has one). An `isOwner` read for a
  Safe on another chain is made on the request's chain only. The signing page cannot read the
  chain, so it refuses these shapes on **any** Safe but the account, which is stricter.
- **Tests.** Vectors `tests/vectors/rules/parent-safe-guard.json` (39 cases),
  `tests/vectors/rules/multisig-self-call.json` (13); `tests/app_self_call_guard.rs`,
  `tests/app_parent_safe_guard.rs`; fork `tests/multisig_fork.rs`
  (`parent_safe_takeover_is_refused_while_the_multisigs_own_flow_runs`); the page's
  `samples/takeover-test.mjs`; web `e2e/parent-safe-guard.e2e.ts`.

### 6.3 The approval guard

- **Rule.** `rust/crates/vela-core/src/app/approval_guard.rs` detects approval-granting shapes
  from raw calldata and typed data (approve, increaseAllowance, `setApprovalForAll`, ERC-2612
  and Permit2 permits, Permit2 witness transfers), including approvals carried inside
  `multicall`, Multicall3 and `multiSend` up to four carriers deep
  (`approval_guard::calldata_approvals`). An amount of 2^128 or more is "unlimited"
  (`approval_guard::UNLIMITED_CAP_256`); one of at least 100× a read, non-zero balance is said in
  the danger tone (`approval_guard::FAR_ABOVE_BALANCE_TIMES`). An unbounded amount is kept as the
  dApp asked unless the person caps it, and the submit guard lets it through only with consent
  for that one approval, in that one leg (`SignApproveOpts.unlimited_approved_legs`).
- **Guarantees.** No unbounded approval is signed without the sheet having shown it as such, and
  consent for one does not cover another.
- **Does not guarantee.** That a bounded approval is safe. Signed permits cannot be capped (they
  are signed as asked or refused). Batch legs are judged by the 2^128 line only (no per-leg
  balance). There is no allowance list or revoke screen (KNOWN-BUGS R22-PLAN-1). The signing page
  reads `PermitBatch`, `PermitBatchTransferFrom` and `PermitBatchWitnessTransferFrom` as generic
  typed data.
- **Tests.** `tests/app_approval_guard.rs`, `tests/app_sign_request.rs` (per-leg waiver),
  `tests/app_batch_id_097.rs`; the page's `samples/unlimited-line-test.mjs`.

### 6.4 Two-provider simulation and the state-diff guard

- **Rule.** A read a verdict stands on is a corroborated read: the RPC pool asks a second
  endpoint from another provider (`app::corroborate::provider_of`, `corroborate::independent`; at
  most `rpc_pool::WITNESS_MAX_ASKS` extra asks; never hedged), after each endpoint has reported
  the chain's id. The core compares meaning, not bytes: simulation within 2 %
  (`sim_outcome::SIM_AGREE_BPS`, showing the worse figure), the state diff by its verdict, code,
  storage and `eth_call` exactly (`corroborate::same_answer`). This covers the sign sheet's
  `eth_simulateV1`, every read of the multisig review, the gate before every multisig signature
  (`authorize_signing`), and the parent-Safe `isOwner` read. With "Only use my node", the
  person's node is trusted alone, and the sheet says so (`corroborate::own_node`). The
  **state-diff guard** (`multisig_review::StateVerdict`, `debug_traceCall` prestate diff and the
  SafeStateProbe) applies to multisig reviews: a change to who controls the multisig that the
  transaction does not declare refuses signing with no override (`StateVerdict::Undeclared`); a
  refusal from either node stands alone.
- **Guarantees.** One lying or compromised node cannot produce a plain verdict: alone it gives
  "unconfirmed" ("… anyway"); beside an honest node it gives "disagreed". A lie can make Vela
  refuse; it cannot make a multisig member change look declared.
- **Does not guarantee.** Independence beyond a registrable-domain heuristic (two brands reselling
  one backend count as two). Two colluding providers. The state diff for a **Vela account's own**
  transactions: there is none; those rest on simulation and the guards. Ordinary display reads
  (balances, history) are one node's word.
- **Tests.** Vector `tests/vectors/rules/signing-truth.json` (13 simulation envelopes, 8 `isOwner`
  readings, 12 provider URLs); `tests/r22_signing_truth.rs`; fork `tests/signing_truth_fork.rs`
  (a real drain behind a lying proxy, `tests/fork_proxy`) and `tests/multisig_fork.rs`
  (`a_lying_node_never_gives_a_member_change_a_plain_review`).

### 6.5 The review door

- **Rule.** A multisig review that is loading, failed, or not read cannot be signed: the door is
  shut until the review lands (`multisig_review_state::door_with`), and the core enforces it again
  at the action (`docs/multisig-machines.md` §12, §14). A review whose state diff or simulation is
  unread is "… anyway"; one refused for its content (safe-shape rules, undeclared control change)
  cannot be signed at all.
- **Guarantees.** Nothing is signed on a review that was not read.
- **Does not guarantee.** That a read review is correct: it inherits the limits of §6.1 and §6.4.
- **Tests.** Vector `tests/vectors/multisig/review-state.json` (with `hold_ms` and `refuseBlind`
  cases); `tests/app_multisig_review_state.rs`; web `e2e/multisig-review-recovery.e2e.ts`.

### 6.6 Hold-to-confirm and "Refuse blind signing"

- **Rule.** `sign_confirm::posture` makes the confirm "… anyway" whenever anything Vela checks
  could not be read ([§7](#7-fail-open-and--anyway-inventory)). Every "… anyway" and every danger
  is held for `sign_confirm::HOLD_TO_CONFIRM_MS` (1.5 s); with assistive technology, a first
  activation arms and a second within `sign_confirm::CONFIRM_AGAIN_WINDOW_MS` (5 s) confirms. The
  "Refuse blind signing" setting (`prefs::keys::REFUSE_BLIND_SIGNING`, **off by default**) turns
  every "… anyway" into a refusal (`ConfirmBlock::BlindRefused`).
- **Guarantees.** No unread check is confirmed by a tap, a double tap or a press made while the
  sheet is arriving. With the setting on, nothing unread is signed at all.
- **Does not guarantee.** That a person reads a held warning. The default is off on purpose
  ([hardening §13](security/hardening.md#13-every-check-vela-could-not-read-is--anyway-review-1-3)):
  networks with one provider or no `eth_simulateV1` would otherwise refuse ordinary transactions.
  The signing page draws "… anyway" as the app's word and has no hold of its own. Hold and the
  assistive path have not been walked by a person on real hardware.
- **Tests.** `tests/r22_signing_truth.rs` (posture for every simulation kind, refuse-blind);
  `review-state.json`; Android `HoldToConfirmDeviceTest`.

### 6.7 The signing page

The signing page (`app-web/trusted-signer`) is one file with its own `default-src 'none'` CSP
inside its bytes. It decodes a request with its own code, refuses the takeover shapes of §6.2,
and signs only a digest it computed; the core accepts only the digest **it** computed
(`rust/crates/vela-core/src/trusted_signer.rs`).

**Per account type** (`signing_venue.rs`; [hardening R22-3](security/hardening.md#r22-3-the-signing-page-check-then-open-review-0-5)):

| Account | Guarantees | Does not guarantee |
| --- | --- | --- |
| **`getvela.app`** (made in the apps or on the official page) | **display assurance**: a wrong or tampered wallet interface cannot make the page show one thing and sign another | **authority.** The web wallet's origin, the extensions and the apps can call the same passkey directly. A fully compromised wallet does not need the page. On the web the wallet's own code runs the check, so a compromised wallet can also open a page of its choosing. |
| **Its own domain** (made on a self-hosted page) | display assurance **and authority**: only pages on that domain can use its keys | an honest page host on the first visit (trust on first use); the host chooses which `sw.js` it serves |

**Integrity of the bytes that run:**

- **The trust root is the build.** `integrity::BUILD_ALLOWED` lists the page hashes a build
  accepts; an unknown hash on the official host is refused (`integrity::ENFORCE`); the check is
  in the core for every shell (`rust/crates/vela-core/src/trusted_signer/integrity.rs`).
- **From bytes (the web wallet).** For the official page, the web wallet opens the bytes it
  hashed as a `blob:` URL, never the address (`integrity::FROM_BYTES`,
  `launch::CheckedPage::opens_from_bytes`; `app-web/vela-wallet/src/lib/signing/page-handoff.svelte.ts`).
  The host is not asked again, so it cannot swap the page after the check.
- **The Service Worker pin** (`app-web/trusted-signer/src/sw.js`), where bytes cannot be opened
  (extensions, a self-hosted page, the natives' system browser): after the first visit, `b/<h>/`
  is served only as bytes hashing to `h`. First visit is trust on first use; the browser updates
  the worker from the host; a browser that keeps no worker is unpinned.
- **Freshness.** Opening needs a check at most `launch::MAX_CHECK_AGE_MS` (10 minutes) old; check
  requests carry a navigation's headers (`launch::CHECK_HEADERS`), though TLS fingerprint, user
  agent and timing still differ.
- **Tests.** `tests/signing_venue_102.rs`; web `e2e/page-from-bytes.e2e.ts`; the page's
  `samples/sw-pin-test.mjs` (10/10); `samples/build-single.mjs --check` in CI (`signing-page`).
- **Not done.** A page-signed send and a multisig approval on a phone, end to end. The page-scoped
  RP ID that would make the page an authority boundary for every account is an owner decision,
  not built ([§13](#13-known-gaps-and-accepted-risks)).

### 6.8 Integrity pinning (build trust roots)

What a build pins rather than fetches: the signing page's hashes (`integrity::BUILD_ALLOWED`,
§6.7); built-in descriptors (§6.1); the contract addresses that make an account's address
(`rust/crates/vela-core/src/safe.rs`, frozen by the identity vector in `tests/vectors/safe.json`);
the production service defaults (`rust/crates/vela-core/src/service_defaults.rs`); the app
associations (§4). Pinned by tests, not by a signature: whoever can change the source and ship a
build can change any of them.

### 6.9 Fee binding and the press freeze

- **Rule.** The fee is in band: a transfer to the relay inside the MultiSend the passkey signs,
  with zero EntryPoint fees and no paymaster (`docs/ARCHITECTURE.md` "Fee model"). The amount is
  `max(settlementGas × inBandFeePerGas[tier], floor)` where the relay bills gas used, and
  `3 × gas limits × max(wallet reading, relay price)` only against a relay that publishes neither
  figure (`fee_policy::IN_BAND_PRICE`, the `Charge` variants). A wallet send is bound review →
  hash (`signed_review::reviewed_safe_op_hash`); a dApp's request is bound to the review frozen at
  the confirm (`signed_review::DappReview`, `signed_review::dapp_safe_op_hash`,
  `signed_review::dapp_message_hash`). From the press to its signature the figures pressed are
  frozen; a quote that moved under the press signs nothing and says so (`send::press_frozen`,
  `docs/multisig-machines.md` §34). Fees round up at the shown places (`fee_policy::fee_figure`);
  Max sends exactly what it shows (`send::max_shown_exact`).
- **Guarantees.** The amount, recipient and fee signed are the ones shown; nothing is signed on a
  figure nobody saw. A shell cannot hash an operation whose calls differ from the reviewed ones.
- **Does not guarantee.** That the fee is close to the on-chain cost: the README records one swap
  charged about 14× its gas. The signing sheet's approve reads the fee view at the handler, not
  the painted frame (a one-frame window, no incident).
- **Tests.** Vector `tests/vectors/rules/send-fee-cover.json`; `tests/app_send_fee_cover.rs`
  (`the_figures_signed_are_the_figures_pressed_or_nothing_is_signed`); `r22_signing_truth.rs`
  (`attest`); a test per shell that a drifted operation prompts no passkey.

### 6.10 validUntil: every SafeOp expires

- **Rule.** `validAfter = 0`, `validUntil = now + 30 min` (`user_op::SAFE_OP_LIFETIME_SECS`,
  `user_op::validity_window`, `user_op::stamp_validity`). `user_op::signable_safe_op_hash` refuses
  an operation with `validUntil = 0`, so a path that forgot to stamp signs nothing. The EntryPoint
  enforces the window (AA22), read as "expired, nothing was sent" (`user_op::is_expired_rejection`).
- **Guarantees.** A relay cannot land a signed operation more than 30 minutes after it was signed.
- **Does not guarantee.** Anything inside the window: a relay can still choose when within it. A
  device clock more than 30 minutes behind the chain expires every operation (said honestly, not
  diagnosed).
- **Tests.** Fork `rust/crates/vela-core/tests/safe_op_window_fork.rs` (4/4: the module's
  `getOperationHash` equals the core's for four windows; expiry and not-yet-due both AA22).

### 6.11 The settlementGas cap

- **Rule.** `fee_policy::settlement_gas_cap`: a published bill is refused
  (`FeeFailure::SettlementGasTooHigh`) above the lower of the signed gas limits and 125 % of the
  wallet's own gas model + 50,000 (`fee_policy::SETTLEMENT_GAS_FACTOR_PERCENT`,
  `fee_policy::SETTLEMENT_GAS_MARGIN`, `fee_policy::static_limits_for`).
- **Guarantees.** A relay cannot bill gas the operation can never use, nor inflate its bill with
  its limits where the wallet has its own model.
- **Does not guarantee.** A bound on a contract call the wallet could not measure (only the
  signed limits bound it), nor on a relay older than CONTRACT v2 (`Charge::Marked`, priced on the
  relay's limits).
- **Tests.** Vector `tests/vectors/rules/settlement-gas-cap.json` (16 cases).

### 6.12 Key-index verification

- **Rule.** Sign-in checks the index's group against the registry contract on Gnosis, then the
  Ethereum backup (`rust/crates/vela-core/src/registry_resolve.rs`, `VerifiedBy`). The signing
  member is matched by one of the two public keys the assertion itself recovers
  (`webauthn::recover_candidates`), not by credential id. A multi-key group nobody on chain
  vouched for is saved unconfirmed (`registry_resolve::identity_of`), and an unconfirmed or
  refuted account shows no address, QR, share, payment request or deposit watch
  (`app::receive_standing`) until `registry_resolve::confirm_step`, reading the chain alone,
  confirms or refutes it.
- **Guarantees.** A lying index cannot make the wallet show an address it co-owns as the person's
  receive address.
- **Does not guarantee.** An unconfirmed account can still sign, and could be added as a multisig
  member (its address would be the one only the index vouched for). An older build reading an
  unconfirmed record ignores the field.
- **Tests.** Vector `rust/crates/vela-core/tests/vectors/rules/lying-index.json` (8 cases);
  `tests/app_login.rs`; fork `tests/lying_index_fork.rs`.

### 6.13 ERC-6492 and SIWE

- **Rule.** A message signature from an account with no code on the request's chain (or whose code
  could not be read) is wrapped in ERC-6492's envelope with the account's own factory call
  (`user_op::message_signature`, `user_op::erc6492_wrap`, which refuses keys that do not make the
  account). An EIP-4361 sign-in is refused, never "anyway", unless its domain is the requesting
  origin's host (and port when named), its URI is on that host and its chain id is the request's
  chain (`clear_signing::sign_in_refusal`).
- **Guarantees.** Counterfactual accounts' signatures verify with 6492-aware verifiers; a sign-in
  for another site or another chain is never signed.
- **Does not guarantee.** Verification by a plain EIP-1271 caller that does not know ERC-6492,
  for an undeployed account.
- **Tests.** `tests/erc6492.rs`; fork `tests/erc6492_fork.rs` with
  `scripts/onchain/erc6492/verify.mjs` (viem universal validation, 21/21); `tests/app_sign_request.rs`.

### 6.14 The mailbox rules

- **Rule.** Everything read from the mailbox is recomputed (`multisig_proposals::read_proposal`:
  the safeTxHash, the review, each approval's signature, current membership). The next nonce
  comes only from open proposals with at least one counting member approval
  (`multisig_proposals::next_nonce_of`). A proposal no member vouches for is `from_non_member`: out
  of the default list and every count, behind "Show proposals from non-members" with a warning.
  Contract members must be bound to their own genesis (`multisig::counterfactual_authority`,
  `Refusal::OwnerNotBound`).
- **Guarantees.** The mailbox cannot make a member sign a hash its device did not compute, cannot
  stall the nonce with a stranger's far-ahead post, and cannot present a stranger's post as
  "needs your approval".
- **Does not guarantee.** Liveness or privacy: the mailbox needs no authentication, serves CORS
  `*`, and links accounts, Safes and pending transactions ([§11](#11-privacy)). A contract member
  that exists only on a network neither the multisig nor the registry uses is not seen.
- **Tests.** `tests/app_multisig_proposals.rs`; `multisig.rs`
  (`a_contract_member_must_be_bound_to_its_own_genesis`); fork `tests/multisig_fork.rs`
  (`a_strangers_far_proposal_does_not_stall_the_multisig`).

### 6.15 The production-write guard (development builds only)

- **Rule.** In a test build (`BuildFacts::is_test_build`), a write to a production host is refused
  before it leaves the device (`service_defaults::write_verdict`; reads in
  `service_defaults::READ_RPC_METHODS` are allowed). Only `VELA_ALLOW_PRODUCTION_WRITES=1` lets a
  test build write. Release builds never install the guard.
- **Guarantees.** Protects production services from test builds. **It protects users from
  nothing**, and is listed here only because it is a security-process control
  ([incidents.md](security/incidents.md)).
- **Tests.** Vector `rust/crates/vela-core/tests/vectors/rules/production-writes.json`;
  `tests/production_writes.rs`; each shell's guard tests.

---

## 7. Fail-open and "… anyway" inventory

Every place a person can sign something Vela could not fully check. Each is gated by "Refuse
blind signing" unless marked otherwise.

| Override | When it appears | What gates it | Why it exists |
| --- | --- | --- | --- |
| **Sign anyway: blind call or message** | a call or message no descriptor reads (`ClearSigningView`) | danger tone, 1.5 s hold; refused with Refuse blind signing | some legitimate contracts have no descriptor |
| **Sign anyway: unread batch leg** | one leg of `wallet_sendCalls` nobody reads (`ClearSigningView::sign_anyway`) | same | as above, per leg |
| **… anyway: simulation not read** | `eth_simulateV1` did not come back, timed out, is not offered, only one provider answered, or two disagreed (`sim_outcome::SimOutcome`) | caution or danger, held; "Check again"; the own-node hint | a single-provider pool or a network without `eth_simulateV1` would otherwise block ordinary use |
| **… anyway: simulation says it reverts** | the simulation reverts | danger, held | a simulation is one block's view on one or two nodes, not the execution; the decision (review 1-3) was "a revert blocks unless the person goes through … anyway" |
| **… anyway: multisig state diff or simulation unread** | `Concern::StateUnread` or `Concern::Unsimulated` in a multisig review | held; door `blindRefused` with the setting on | networks without `debug_traceCall`, or one provider |
| **eth_sign** | `eth_sign` (signed as EIP-191 wrapped in the account's `SafeMessage`, `rust/crates/vela-core/src/sign_message.rs`) | blind, held; the signing page refuses `eth_sign` | dApp compatibility |
| **Unlimited approval kept** | an unbounded amount the person does not cap | red line, cap one chip away; consent per approval per leg. **Not** gated by Refuse blind signing (it is read, not blind) | Permit2 and EIP-5792 bundles revert if the approve leg is re-encoded |
| **Multisig account risk** | a deployed multisig has a module, guard, unknown singleton or unknown fallback handler (`multisig::Risk`, `Gate::Risk`) | a confirmation before **every** signature (`MultisigOperation::AskRisk`). Not gated by Refuse blind signing. **The design document said "stop signing"; the code asks** (`docs/multisig-vault-design.md` §5.2 note) | members may knowingly use a module; the chain read itself is corroborated |
| **Unknown signing-page hash on a custom address** | a self-hosted page whose bytes this build does not list | asked once; the person may trust it (`integrity.rs`, rule 4) | self-hosting must be possible |
| **Kill switch** | `integrity::ENFORCE = false` opens a refused page with its refusal drawn | a source change and a release | recovery from a self-inflicted outage |
| **Test-build production writes** | `VELA_ALLOW_PRODUCTION_WRITES=1` | test builds only; off by default | real-device passes that must write |

Where Vela fails **closed** with no override: the self-call and parent-Safe shapes (§6.2); a
sign-in that does not bind (§6.13); an undeclared control change in a multisig (§6.4); the
multisig safe-shape rules (§9); an operation with no expiry (§6.10); a fee or calldata that drifted
from the review (§6.9); an unknown page hash on the official host (§6.7); a refuted account's
receive address (§6.12).

---

## 8. Keys and accounts

- **The account.** A Safe v1.4.1 proxy with Safe4337Module v0.3.0 as module and fallback handler,
  owned by 1–7 passkey signers (`safe::MAX_MULTI_KEYS`), threshold 1. Its address is CREATE2 over
  the setup data, which includes every founding key, so it is the same on every chain and exists
  before anything is deployed (`safe.rs` `compute_safe_address_multi`).
- **1-of-N, fixed keys.** Any founding key signs alone. Keys cannot be added, removed or replaced:
  a change on one chain would make the account differ from the address that still stands for the
  founding set on every chain where it is not yet deployed.
- **Compromise means migrating.** A key that may be in someone else's hands cannot be removed. The
  only remedy is to move funds to a new account, on every chain where they are.
- **Recovery.** Sign in with any one founding key (a synced passkey, a phone over hybrid, a
  security key). The wallet finds the account through the key index, or directly from the
  registry contract, and checks the group against the chain (§6.12). A single-key account can be
  rebuilt on the device from two signatures (`webauthn::recover_candidates`). There is no seed,
  no guardian and no social recovery: if every founding key is lost, the account is lost.
- **Index down.** Sign-in falls back to the registry contract on Gnosis, then Ethereum. With no
  chain answering, a multi-key account is saved unconfirmed and cannot receive until checked.
- **Index lying.** §6.12: a forged member is caught by its public key; an unchecked multi-key
  group shows no receive address; the chain-only confirm step refutes a group that does not make
  the address.
- **Domain loss.** The RP ID cannot move. Existing accounts keep working through the Chrome
  extension (which can use `getvela.app` passkeys by permission) and through self-built apps that
  reach a phone or a security key over CTAP; platform passkeys in the store apps work only while
  the OS can still verify the app against the domain (`app-web/getvela.app/src/content/docs/self-hosting.md`).
  A new owner of the domain could serve pages and association files that use every
  `getvela.app` passkey.
- **Not supported.** EIP-7702 delegation of a Vela account; migration to EntryPoint v0.8/v0.9 or
  Safe 1.5 for existing accounts (their address is fixed to the setup data).

---

## 9. The multisig model

**Not released.** A Vela multisig is a Safe v1.4.1 (SafeL2, CompatibilityFallbackHandler, no
guard, owners in the order given) whose owners are Vela accounts, EOAs or contracts, M-of-N.

- **Genesis.** Created counterfactually; the genesis service records it (owner-proved), and the
  SafeGenesisRegistry v4 contract can hold the record and the name. The registry is an index, not
  the multisig's chain or authority. Contract members must be bound to their own genesis
  (§6.14).
- **Authority comes from the chain.** Members, threshold, nonce, modules, guard, handler and
  singleton are read on chain, corroborated by two providers (`multisig::deployed_check`,
  `authorize_signing`); for an undeployed multisig, from its verified genesis.
- **What a member signs.** Its own account's `SafeMessage` over the multisig's safeTxHash, which
  the core recomputes (`multisig::approval_from_assertion`). This is the very shape §6.2 refuses
  to a dApp; approvals enter only from the multisig's own flows, decided by the path in the core,
  not by a field.
- **Safe shapes only** (`multisig::review`): `operation = 0` except a delegatecall to v1.4.1
  MultiSendCallOnly (`multisig::MULTI_SEND_CALL_ONLY`) whose legs are all calls;
  `safeTxGas = baseGas = gasPrice = 0` and no gas token or refund receiver; calls to the multisig
  itself only for the member changes and a cancel; never `enableModule`, `setGuard`,
  `setFallbackHandler`. A transaction proposed elsewhere outside these shapes is decoded and shown
  as "Vela won't sign this".
- **Review.** Every member's device decodes, simulates and state-diffs the transaction itself
  (§6.4, §6.5) and shows the domain, message and safeTxHash for an independent hardware-wallet
  check.
- **Execution.** Any local Vela account sends an ordinary UserOperation calling `execTransaction`
  (`v = 1` for itself), paying the in-band fee shown (§6.9).
- **Outside owners.** An EOA owner signs the SafeTx EIP-712 in its own wallet over PeerConnect /
  WalletPair (Vela's `/connect` page is the wallet side); the core ecrecovers every signature
  before it is posted.
- **Sites.** A site connected to a multisig gets proposals, not signatures; the multisig signs no
  messages for sites (4200). The self-call rule holds at the multisig's own address
  (`multisig_dapp::self_call_refusal`).

**Known limits.** The mailbox is a liveness and privacy dependency (§6.14, §11). Proposal links
name a transaction (`multisig_link::proposal_link`); they do not carry it, so without the mailbox
members cannot see each other's proposals. Not built: push notifications, Safe Transaction
Service sync, batch on-chain approval, reject votes, soft expiry, proposers. An abandoned
proposal keeps its nonce until it is cancelled. A Vela passkey member's approval for a Safe
v1.5.0 multisig is not supported. The multisig account check asks rather than refuses (§7). An
unconfirmed Vela account can be made a member (§6.12). The full list is the
`docs/multisig-vault-design.md` §5.2 / §5.3.2 notes.

---

## 10. Network and RPC

- **The pool.** Each network is read through a pool of endpoints: built-in, public fallbacks, and
  the person's own endpoints and provider keys (`rust/crates/vela-core/src/app/rpc_pool.rs`). It
  scores, fails over, bans and self-heals. Endpoint URLs are filtered by
  `network_admin::is_allowed_endpoint_url`.
- **Verdict reads** need two independent providers (§6.4). Every source of a corroborated read
  must have reported the chain's id first; a node that names another chain is excluded from every
  read of that chain.
- **"Only use my node"** pins the pool to the person's node, trusted alone, and says so.
- **Adding a network** checks the twelve contracts an account needs and probes the P-256
  precompile with a real signature (`network_admin::P256_PRECOMPILE`).
- **What the relay learns.** The RPC URL the wallet uses for a chain, including any API key in it,
  is sent to the relay with each request so it can reach the same network.
- **dApp reads** are proxied to the node for an allow-listed method set
  (`rust/crates/vela-core/src/app/dapp_rpc.rs`); `eth_sendRawTransaction` is among them and is
  forwarded, not refused ([§13](#13-known-gaps-and-accepted-risks)).

---

## 11. Privacy

| Party | Sees | Retention | Opt-out |
| --- | --- | --- | --- |
| Registry contract (public, on chain) | each key's public key and credential id, the authenticator model, the wallet name and key labels, the address, the signed registration | permanent | none once written; a wallet need not be registered to work on one device |
| Key index | the record before it submits it; the addresses you look up names for; your IP | the service's | use your own index |
| Relay | your address, the operations you submit, the RPC URL (with any key), your IP | operations kept for a limited time to retry and diagnose | run your own relay |
| RPC providers | every read, your IP | theirs | "Only use my node" |
| Chain data, rates, passkey directory | what you ask (tokens, descriptors, rates, a model id), your IP | theirs | self-host chain data and rates |
| Selector databases (sourcify, openchain, 4byte) | a function selector, as a last resort | theirs | — |
| Genesis service and mailbox | multisig owners, threshold, name; proposals and approvals; **no authentication, CORS `*`**: anyone who knows a Safe's address can read its pending proposals | the service's | — (not yet self-hostable through Settings for every flow) |
| PeerConnect relay | an encrypted session between Vela and an outside owner's wallet; tokens from `getvela.app/api/peerconnect-token`, or PeerConnect's trial token as a fallback | theirs | — |
| Bug report service | what you type and attach; filed as a **public** GitHub issue | until deleted | do not send one |
| Rybbit analytics (`tj.appsdata.org`) | web wallet and extension: **only if turned on** — screens and named moments with coarse details, never an address, amount or transaction outcome (`app-web/vela-wallet/src/lib/analytics/catalog.ts#TX_OUTCOME_EVENTS`, `payload.test.ts`); the website: page views and some clicks, on every page but `/chain-setup` | Rybbit's | off by default in the wallet; the native apps send none |
| Apple / Google / your password manager | your synced passkeys; Apple's and Google's tunnel servers carry a hybrid (QR) ceremony | theirs | use a security key |

The authoritative list for users is the privacy page
(`app-web/getvela.app/src/routes/privacy/+page.svelte`). Requirement A03 is superseded where it
says servers keep only a public key and a name ([A03](requirements/A03-privacy-no-token-no-tracking.md)).

---

## 12. Build and release verifiability

How to check each artifact yourself is [`docs/security/verify.md`](security/verify.md), with a
one-page public summary at [`docs/security/review-pack.md`](security/review-pack.md#how-to-verify-vela-yourself).
In short:

- **Web core** (the wasm the web wallet and extensions run): rebuilt byte for byte in a pinned
  container by `rust/scripts/reproduce-web.sh` (`rust/scripts/web-build.Dockerfile`); CI job
  `web-core-reproducible` compares it on every PR and attests it on `main`. Measured
  byte-identical on two clean macOS builds; **Linux not yet measured** (the first CI run is the
  cross-host proof).
- **Signing page**: `app-web/trusted-signer/samples/build-single.mjs --check`, blocking in CI under
  Bun and Node.
- **The rest of the web wallet's JavaScript** is built by Vite on Cloudflare and is **not**
  compared byte for byte.
- **Native releases** carry GitHub build-provenance attestations (macOS: Apple notarization, then
  an attestation of the published bytes). They are attested, **not reproducible**.
- **Supply chain**: `deny.toml` (cargo-deny over both lockfiles), JS audits, Dependabot
  (`.github/dependabot.yml`), the PeerConnect vendor check
  (`scripts/check-peerconnect-vendor.mjs`, `app-web/vela-wallet/src/lib/vendor/peerconnect/VENDOR.json`).
  No CodeQL, secret scanning or SBOM.

---

## 13. Known gaps and accepted risks

Each item is open at the commit above. "Accepted" means decided and recorded; the others are work.

| # | Gap | Status | Where |
| --- | --- | --- | --- |
| G1 | **Provider independence is a registrable-domain heuristic.** Two brands on one backend count as two; it is no stronger than the pool's list. | accepted, documented | `corroborate::provider_of`; [hardening §12](security/hardening.md#12-one-rpc-never-decides-what-the-signer-believes-review-1-2-sec-parent-3) |
| G2 | **`getvela.app` accounts: the signing page is display assurance, not authority.** The page-scoped RP ID (keys only the page can use) is the fix. | owner decision, not built | `signing_venue.rs`; [hardening R22-3](security/hardening.md#owner-decision-a-page-scoped-rp-id-the-real-fix-not-built-this-round) |
| G3 | **The page-only passkey domain is not built** (same item, as a product: an opt-in "page-only signing" account on a dedicated registrable domain). | owner decision | as G2 |
| G4 | **First-party key backup simulation is one node's word**: the wallet's own key-record backup gets no exemption and is "… anyway", held, when its simulation is uncorroborated. Honest; a candidate for a first-party rule. | open | [hardening "Left open (Round 22, signing truth)"](security/hardening.md#left-open-round-22-signing-truth) |
| G5 | **Linux reproducibility of the web wasm is unmeasured.** If CI's container bytes differ from the committed macOS build, the container's build is to be committed and served. | owner item (read the first CI run) | [hardening R22-2](security/hardening.md#r22-2-reproducible-builds-review-1-9); [verify.md](security/verify.md) |
| G6 | **GPL-3.0 `zlog` / `ztracing` / `ztracing_macro` are linked into the desktop** through the pinned gpui. A release blocker for the desktop until the pin moves past zed `ac5af8b9` (relicensed Apache-2.0), or the risk is accepted for one release. | owner item | `deny.toml`; [hardening R22-1](security/hardening.md#r22-1-dependency-gates-review-1-10) |
| G7 | **SafeGenesisRegistry v4 is not audited** (internal adversarial review only). The multisig rules do not take its word for authority, but it records geneses and names. | accepted for an unreleased feature | [Audits & known issues](https://getvela.app/docs/security-audits) |
| G8 | The mailbox server's Round 22 changes (`nonceTooFar`, stranger allowance, open-first listing) are uncommitted in their repository; CI's nightly fork job still pins `mailbox-any-signer`. | owner item | [hardening §10](security/hardening.md#10-the-mailbox-cannot-stall-or-phish-a-multisig-review-1-13-wallet-side) |
| G9 | An unconfirmed account can sign and can be made a multisig member; iOS can re-save a confirmed account as unconfirmed (fail-safe); an older build drops `identity`. | open (R23 candidate) | [hardening "Round 22: left open"](security/hardening.md#round-22-left-open) |
| G10 | No allowance list or revoke screen. | planned | KNOWN-BUGS R22-PLAN-1 |
| G11 | The signing page reads `PermitBatch`, `PermitBatchTransferFrom` and `PermitBatchWitnessTransferFrom` as generic typed data; batch legs are judged by 2^128 alone. | open | [hardening "Round 22: left open"](security/hardening.md#round-22-left-open) |
| G12 | A relay older than CONTRACT v2 is priced on its own limits, not capped against the wallet's model. | open | [hardening §2](security/hardening.md#2-the-relays-settlementgas-is-capped-review-1-6) |
| G13 | No self-submit path: censorship by the relay is answered only by switching relays or running one. | open | [§5](#5-adversaries-and-outcomes) |
| G14 | CTAP security keys keep `getvela.app` in every build; the key index's recorded RP for a test build's keys follows each shell's own reading. | open (test builds only) | [hardening §6](security/hardening.md#6-test-builds-have-their-own-passkey-domain-review-0-1-follow-up) |
| G15 | Other `*.getvela.app` hosts (relay, index, ethereum-data, safe-genesis, currency) must send `publickey-credentials-*=()` and the anti-framing pair; they are in other repositories. Domain controls (registry lock, DNSSEC, CAA, HSTS preload) are not documented. | owner item | [hardening §4](security/hardening.md#4-origin-hardening-review-1-1) |
| G16 | The signing page's newest versions are not deployed, and `integrity::LAUNCH` must move after deployment; until then a phone's page-route transaction is refused `WrongChallenge` (fail-closed). | owner item | [hardening go-live](security/hardening.md#go-live-checklist-the-owner) |
| G17 | `eth_sendRawTransaction` from a dApp is forwarded to the node, not refused (it carries a transaction someone else signed; Vela signs nothing). | open, P2 | `rust/crates/vela-core/src/app/dapp_rpc.rs` |
| G18 | No CodeQL, secret scanning or SBOM. The extension ships as a hand-loaded zip, so fixes do not auto-update. | open | [§12](#12-build-and-release-verifiability) |
| G19 | No EIP-7702 stance, no migration to newer EntryPoint or Safe versions for existing accounts; the audits page names EntryPoint v0.7's interception issue. | open | [Audits & known issues](https://getvela.app/docs/security-audits) |
| G20 | An AI agent may ask to switch statistics on (a `Manage` intent the person approves). | open | [hardening §3](security/hardening.md#3-usage-statistics-are-opt-in-review-1-8) |
| G21 | The phones' page path has no end-to-end harness; hold-to-confirm and the assistive path have not been walked on real hardware. | open | [hardening R22-3](security/hardening.md#r22-3-the-signing-page-check-then-open-review-0-5) |

Open bugs in the tracker are in [`docs/KNOWN-BUGS.md`](KNOWN-BUGS.md); the security-relevant ones
are SEC-PARENT-1, SEC-PARENT-2 and SEC-PARENT-3 (all fixed on this branch) and R22-PLAN-1.

---

## 14. Disclosure and advisories

How to report, response targets, safe harbour and scope: [`SECURITY.md`](../SECURITY.md) (GitHub
private vulnerability reporting first; `security@getvela.app` is listed but not confirmed; no PGP
key yet; **no bug bounty**). `security.txt` is served by both sites
(`app-web/getvela.app/src/routes/.well-known/security.txt/+server.ts`).

| ID | Issue | Fixed in | Published |
| --- | --- | --- | --- |
| [VELA-2026-001](security/advisories/VELA-2026-001-typed-data-preview-mismatch.md) | A typed-data request could preview one EIP-712 message and sign another | v0.9.6 | not yet |
| [VELA-2026-002](security/advisories/VELA-2026-002-parent-safe-authority.md) | A site connected to a Vela account could act for a Safe that account owns | `64da454d3`, not released | not yet; after the release |
| [VELA-2026-003](security/advisories/VELA-2026-003-passkey-app-association.md) | getvela.app's app-association files let development and demo apps request users' passkeys | site `0562c17bb`, deployed 2026-11-07 | not yet |

Internal incidents that put no user at risk: [`docs/security/incidents.md`](security/incidents.md).

---

## 15. Document changelog

| Date | Commit verified | Change |
| --- | --- | --- |
| 2026-11-09 | `fa6083c87` | First version (Round 23), following the outline of the 2026-11-07 internal review. Supersedes the whitepaper's "Threats considered" as the reference threat model. |
