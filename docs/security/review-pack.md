# External review pack

A scope document for an auditor or a security researcher reviewing Vela Wallet. It lists what
there is, where the code that decides safety lives, how to build and test it, what is already
known to be open, and how to reach us. The threat model is
[`docs/SECURITY-MODEL.md`](../SECURITY-MODEL.md); read it first.

**Prepared against commit `fa6083c87`** (branch `feat/multisig-account`, 2026-11-09); its CI and test-corpus sections re-checked at `dff291256` with the final review's CI fixes (2026-11-12). The latest
release, v0.9.8 on `main`, predates the Round 21–22 hardening and the multisig feature
([SECURITY-MODEL §1](../SECURITY-MODEL.md#1-scope-and-status)). Every path and symbol below is
checked to exist by `scripts/ci/check-doc-claims.mjs` in CI.

---

## 1. Components and size

Lines are counted over tracked files with `git ls-files <scope> | xargs wc -l`, at the commit
above. "Core" is where every rule lives; the shells render it and perform I/O.

| Component | Language | Scope | Lines |
| --- | --- | --- | ---: |
| **Core** (`vela-core`): signing, clear signing, guards, fees, RPC pool, multisig rules, WebAuthn, CTAP | Rust | `rust/crates/vela-core/src` (includes inline unit tests) | 180,223 |
| Core integration tests and fork tests | Rust | `rust/crates/vela-core/tests` | 130,704 |
| Bindings: UniFFI (iOS, Android), wasm (web) | Rust | `rust/crates/vela-core-uniffi`, `rust/crates/vela-core-wasm` | 15,833 |
| AI agent runner (spec 103) | Rust | `rust/crates/vela-agent` | 3,653 |
| Desktop app (gpui) | Rust | `app-desktop/vela-wallet/src` | 227,735 |
| Windows passkey crate | Rust | `app-desktop/vela-passkey-win` | 724 |
| Web wallet and Chrome extension | TypeScript, Svelte | `app-web/vela-wallet/src`, excluding tests and generated code | 167,718 |
| Web wallet tests | TypeScript | `*.test.ts` / `*.spec.ts` under `app-web/vela-wallet/src` | 104,235 |
| Web generated wire types | TypeScript | `app-web/vela-wallet/src/lib/*/generated` | 13,746 |
| Extension background and content scripts | JavaScript | `app-web/vela-wallet/extension` | 5,324 |
| **Signing page** | JavaScript | `app-web/trusted-signer/src` (6,315 of it locale strings) | 15,161 |
| iOS app | Swift | `app-ios/VelaWallet/VelaWallet` | 114,899 |
| iOS tests (unit, UI) | Swift | `app-ios/VelaWallet/VelaWalletTests`, `app-ios/VelaWallet/VelaWalletUITests` | 88,043 |
| Android app | Kotlin | `app-android/vela-wallet/app/src/main` | 120,859 |
| Android tests (unit, instrumented) | Kotlin | `app-android/vela-wallet/app/src/test`, `app-android/vela-wallet/app/src/androidTest` | 65,035 |
| Website (association files, headers, analytics, docs) | TypeScript, Svelte, Markdown | `app-web/getvela.app` | — |

**Out of this repository, in scope of [SECURITY.md](../../SECURITY.md#scope):** the relay
(`vela-relay`), the key index and its registry contract (`p256-index`), the multisig genesis
service, mailbox and `SafeGenesisRegistry` (`safe-genesis-registry`), rates (`vela-currency`)
and chain data (`ethereum-data`). **Out of scope:** Safe, Safe4337Module, the Safe passkey
contracts, EntryPoint and Multicall3, used unmodified.

---

## 2. Critical paths

Where a bug costs funds or authority. Start here.

| Path | Entry points | What to try to break |
| --- | --- | --- |
| **Signing: what is hashed** | `user_op::signable_safe_op_hash`, `signed_review::reviewed_safe_op_hash`, `signed_review::dapp_safe_op_hash`, `signed_review::dapp_message_hash`, `rust/crates/vela-core/src/typed_data_request.rs`, `rust/crates/vela-core/src/sign_message.rs` | make a shell hash something other than the reviewed calls, fee leg, digest, chain or account; a typed-data request whose preview and digest differ (the VELA-2026-001 class) |
| **The dApp signing machine** | `rust/crates/vela-core/src/app/sign_request.rs` (arrival, `proceed_submit`), `sign_confirm::posture`, `sign_confirm::with_posture` | reach a plain confirm over an unread check; skip the submit re-check; get a "… anyway" past Refuse blind signing |
| **Guards** | `self_call_guard::detect_self_call`, `parent_safe_guard::verdict`, `parent_safe_guard::enforce`, `approval_guard::calldata_approvals`, `clear_signing::sign_in_refusal` | a takeover shape the recursion misses (depth, encoding, a carrier not listed; a target address in a non-canonical spelling is already known, SECURITY-MODEL G24); an `isOwner` answer that clears a Safe the account owns; an approval the detector misses |
| **Clear signing and provenance** | `rust/crates/vela-core/src/app/clear_signing.rs` (`ClearProvenance`, `project_result`) | a descriptor or selector reading that grades green, or reads as "verified", without being built in |
| **Simulation and corroboration** | `rust/crates/vela-core/src/app/corroborate.rs`, `sim_outcome::classify_envelope`, `rust/crates/vela-core/src/app/rpc_pool.rs` (witness phase, chain-id probe) | one lying node producing a plain verdict; two endpoints of one provider counted as independent; a witness answer reused across chains |
| **Fees** | `rust/crates/vela-core/src/app/fee_policy.rs` (`settlement_gas_cap`, `IN_BAND_PRICE`, `fee_figure`), `send::press_frozen`, `send::max_shown_exact` | a charge above what was shown; a relay bill above the cap; a quote change under the press that signs |
| **SafeOp validity** | `user_op::validity_window`, `user_op::stamp_validity`, `user_op::is_expired_rejection` | an operation signed with no expiry, or one a relay can land late |
| **Multisig authority** | `rust/crates/vela-core/src/multisig.rs` (`review`, `read_batch`, `deployed_check`, `counterfactual_authority`, `approval_from_assertion`), `rust/crates/vela-core/src/multisig_review.rs` (`StateVerdict`, `signing_gate`), `rust/crates/vela-core/src/app/multisig_proposals.rs` (`read_proposal`, `next_nonce_of`, `authorize_signing`), `multisig_review_state::door_with` | a member signing a safeTxHash its device did not recompute; an undeclared control change that reads as declared; a stranger's approval counted; the review door open on an unread review |
| **Key index** | `rust/crates/vela-core/src/registry_resolve.rs` (`identity_of`, `confirm_step`), `rust/crates/vela-core/src/app/login.rs`, `app::receive_standing`, `webauthn::recover_candidates` | a lying index that gets a co-owned address shown as a receive address |
| **The signing page** | `app-web/trusted-signer/src/lib/resolve.js`, `app-web/trusted-signer/src/lib/digest.js`, `app-web/trusted-signer/src/lib/safeop.js`, `app-web/trusted-signer/src/sw.js`; core side `rust/crates/vela-core/src/trusted_signer/integrity.rs`, `rust/crates/vela-core/src/trusted_signer/launch.rs`, `rust/crates/vela-core/src/signing_venue.rs`; web `app-web/vela-wallet/src/lib/signing/page-handoff.svelte.ts` | the page signing a digest it did not compute; the wallet opening bytes other than the hashed ones; the Service Worker serving bytes that do not hash to their path |
| **Passkey domain and app associations** | `app-web/getvela.app/src/routes/.well-known/assetlinks.json/+server.ts`, `app-web/getvela.app/src/routes/.well-known/apple-app-site-association/+server.ts`, `service_defaults::relying_party_id`, `service_defaults::web_relying_party_id` | an identity beyond the two release apps; a test build reaching `getvela.app` platform passkeys |
| **Web origin** | `app-web/vela-wallet/src/lib/security-headers.ts`, `app-web/vela-wallet/vite.config.ts` (CSP hashes), `app-web/vela-wallet/src/hooks.server.ts` | script execution on `wallet.getvela.app` (it holds the passkey ceremony for every `getvela.app` account) |

---

## 3. Threat model

[`docs/SECURITY-MODEL.md`](../SECURITY-MODEL.md): assets, trust assumptions, adversaries, each
defence's guarantee and non-guarantee, the "… anyway" inventory, and the known gaps. The
decisions and their proofs are in [`hardening.md`](hardening.md).

---

## 4. Build and run

Prerequisites, per component, are in each README. The short version:

```sh
# Core: unit, conformance corpus, proptests, machines (what CI runs)
cd rust && cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures,vela-core/crux

# Core fork tests (ignored by default): a local anvil fork of Gnosis, the genesis service,
# the relay and the key index as local stand-ins. Needs foundry and the safe-genesis-registry
# checkout (scripts/onchain/multisig-fork/README.md).
FORK_PORT=97xx scripts/onchain/multisig-fork/up.sh
VELA_MULTISIG_FORK=1 FORK_PORT=97xx cargo test -p vela-core --features crux,dev-fixtures \
  --test signing_truth_fork -- --ignored --nocapture        # also multisig_fork, lying_index_fork,
                                                             # safe_op_window_fork, erc6492_fork

# Web wallet (Node 22, pnpm)
cd app-web/vela-wallet && pnpm install --frozen-lockfile && pnpm check && pnpm test:unit -- --run && pnpm build

# Signing page: rebuild and compare with what is committed
cd app-web/trusted-signer && node samples/build-single.mjs --check

# The web core, byte for byte in the pinned container
rust/scripts/reproduce-web.sh

# Desktop
cd app-desktop/vela-wallet && cargo test
```

The fork harness uses local stand-ins for every service; nothing here needs a Vela account,
key or production service. **Please do not write to production services** (SECURITY.md, safe
harbour): key-index registrations and registry records are permanent.

CI (`.github/workflows/ci.yml`) runs on every pull request: the core on Linux and macOS, the
Kotlin and Swift binding corpora, the web unit suite and two e2e specs
(`extension-site-start.e2e.ts`, `security-headers.e2e.ts`), the website's unit suite (its
app-association files, security.txt and headers among them), the signing page's rebuild under
Bun and Node, the supply-chain gates, the web-core rebuild, and the claims check. Which of
these stop a merge is the repository's branch-protection setting, an owner action this
repository cannot show. The full web e2e suite, the signing-page suites and the fork suites run
nightly (`.github/workflows/nightly.yml`); nightly runs gate no merge.

---

## 5. Test and vector corpora

| Corpus | Where | Replayed by |
| --- | --- | --- |
| Conformance vectors (crypto, ABI, EIP-712, Safe addresses, WebAuthn, identicons, i18n) | `rust/crates/vela-core/tests/vectors/` (65 files) | Rust, the Kotlin and Swift bindings, the shipped wasm (`rust/crates/vela-core/tests/conformance.rs`) |
| Security rules | `rust/crates/vela-core/tests/vectors/rules/`: `parent-safe-guard.json`, `multisig-self-call.json`, `signing-truth.json`, `lying-index.json`, `settlement-gas-cap.json`, `send-fee-cover.json`, `rp-id.json`, `production-writes.json`, `passkey-errors.json`, `transport-failures.json` | the core and every shell through the real core |
| Multisig review | `rust/crates/vela-core/tests/vectors/multisig/`, `rust/crates/vela-core/tests/vectors/multisig-review/` | the core and every shell's review replay |
| Fork tests (real contracts on a local Gnosis fork) | `rust/crates/vela-core/tests/multisig_fork.rs`, `rust/crates/vela-core/tests/signing_truth_fork.rs`, `rust/crates/vela-core/tests/lying_index_fork.rs`, `rust/crates/vela-core/tests/safe_op_window_fork.rs`, `rust/crates/vela-core/tests/erc6492_fork.rs` (with `scripts/onchain/erc6492/verify.mjs`); the lying RPC is `rust/crates/vela-core/tests/fork_proxy` | nightly CI, all five in the `multisig-fork` job; the job skips itself while `vars.SGR_MAILBOX_REF` (the pinned genesis service the harness starts) is unset |
| Signing-page suites (hostile inputs, takeover shapes, digests, the SW pin) | `app-web/trusted-signer/samples/` (`hostile-test.mjs`, `takeover-test.mjs`, `safeop-test.mjs`, `sw-pin-test.mjs`, …) | `scripts/ci/trusted-signer-suites.sh` |
| Web end to end | `app-web/vela-wallet/e2e/` (`parent-safe-guard.e2e.ts`, `page-from-bytes.e2e.ts`, `security-headers.e2e.ts`, `e2e/onchain/` against the fork) | nightly (`security-headers.e2e.ts` and `extension-site-start.e2e.ts` also in CI `web`) |
| Cross-platform scenario manifest | `docs/scenarios/multisig.yaml` | `scripts/ci/check-scenarios.mjs` |

---

## 6. Known gaps

[SECURITY-MODEL §13](../SECURITY-MODEL.md#13-known-gaps-and-accepted-risks) lists every open item
with its status. A report that narrows one of them is welcome; a report that only restates one is
already known. Bugs in the tracker: [`docs/KNOWN-BUGS.md`](../KNOWN-BUGS.md). Past issues:
[`advisories/`](advisories/).

---

## 7. Contact

- **Report privately:** GitHub private vulnerability reporting,
  <https://github.com/mondaylabsltd/vela-wallet/security/advisories/new>.
- **Email:** `security@getvela.app` is listed in `security.txt` but **not yet confirmed**; use
  GitHub until it is.
- No PGP key yet; ask through either channel if you need encryption.
- Response targets, coordinated disclosure (90 days by default) and safe harbour:
  [`SECURITY.md`](../../SECURITY.md). There is **no bug bounty** at present.
- For a scoped engagement (an audit), open a private advisory asking for a contact; it reaches
  the maintainers (MONDAY LABS LTD).

---

## How to verify Vela yourself

One page, no account needed. Each step says what it proves and what it does not. The full
procedure is [`verify.md`](verify.md).

1. **The account is a standard Safe.** Your address is CREATE2 over Safe v1.4.1's factory and
   your keys' setup data. The contract addresses are constants in
   `rust/crates/vela-core/src/safe.rs` (Safe singleton, proxy factory, Safe4337Module v0.3.0,
   EntryPoint v0.7, the passkey module's signer and factory). Compare them with Safe's own
   deployments and read your Safe's owners, threshold, modules and fallback handler on any block
   explorer. *Proves:* Vela holds no role on your account. *Does not prove:* that the app asks you
   to sign what it shows.
2. **The web core is this source.** `rust/scripts/reproduce-web.sh <commit>` rebuilds the wasm in
   a pinned container and compares it byte for byte; `gh attestation verify` checks that CI built
   the module wallet.getvela.app serves; `curl` the served module and hash it ([verify.md
   §1](verify.md#1-the-web-core)). *Proves:* the bytes that make every decision are this commit's.
   *Does not prove:* that every visitor is served them, or anything about the rest of the web
   wallet's JavaScript, which is not compared byte for byte.
3. **The signing page is this source.** `node samples/build-single.mjs --check` in
   `app-web/trusted-signer`, then `curl -s https://sign.getvela.app/b/<h>/sign | shasum -a 256`
   must print `<h>`; the versions a build accepts are `integrity::BUILD_ALLOWED`. *Proves:* the
   page you are served is a published, rebuildable version. *Does not prove:* authority for a
   `getvela.app` account ([SECURITY-MODEL §6.7](../SECURITY-MODEL.md#67-the-signing-page)).
4. **The release you downloaded came from CI.** `gh attestation verify <file> --repo
   mondaylabsltd/vela-wallet` for every release file; macOS images also pass `xcrun stapler
   validate`. *Proves:* provenance. *Does not prove:* reproducibility; native builds are attested,
   not reproducible.
5. **Who may use your passkeys.** `curl https://getvela.app/.well-known/assetlinks.json` and
   `curl https://getvela.app/.well-known/apple-app-site-association` must list exactly the two
   release apps in [SECURITY-MODEL §4](../SECURITY-MODEL.md#4-trust-assumptions). *Proves:* which
   native apps the OS lets use platform passkeys. *Does not prove:* anything about CTAP clients or
   browser extensions, which these files do not gate.
6. **The defences exist.** Run the core tests and the vectors (§4 above), and the claims check
   `node scripts/ci/check-doc-claims.mjs`, which fails if a code location cited in the security
   documents names nothing. *Proves:* the cited rules and tests exist and pass. *Does not prove:*
   that they cover every attack; that is what a review is for.
