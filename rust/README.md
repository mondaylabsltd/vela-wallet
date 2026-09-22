# vela-core — the wallet's shared computation core

Pure, deterministic, correctness-critical computation for Vela Wallet: parsing,
encoding, hashing, big-integer math, data assembly and validation. No I/O, no
network, no UI, no randomness. One implementation, four surfaces.

Feature spec and design docs: [`specs/001-rust-core-bindings/`](../specs/001-rust-core-bindings/).
[`contracts/core-api.md`](../specs/001-rust-core-bindings/contracts/core-api.md)
is the authoritative API surface and the list of intentional behavior changes.

## Why this exists

Before this crate, the wallet hand-rolled Keccak-256 (twice — TypeScript *and* a
parallel Swift copy), SHA-256, a dynamic ABI decoder, and P-256 elliptic-curve
math on BigInt. Counterfactual Safe address derivation was maintained in three
places, including byte-matched constants in the bundler repo. A bug in any of
them silently loses funds or makes the signing sheet lie about what is being
approved. This crate is one implementation of that logic, wrapping audited
libraries, shared to every platform.

## Layout

```
crates/vela-core          pure logic, zero FFI dependencies
  primitives              hex/base64url/quantity, keccak256, sha256, EIP-55, CREATE2
  abi                     runtime calldata decode → recursive AbiValue
  eip712                  eth_signTypedData_v4 digest (+ declared-domain guard)
  safe                    counterfactual Safe & splitter address assembly
  webauthn                COSE extract, DER→low-s, client data, 2-assertion recovery
  identicon               account avatars — exact port of identicons-esm@1.0.1
  identicon_features      GENERATED artwork table (84 SVG fragments)
crates/vela-core-uniffi   uniffi 0.32 shell → Kotlin (Android) + Swift (iOS)
crates/vela-core-wasm     wasm-bindgen shell → the web app
crates/vela-uniffi-bindgen  build-time ONLY — the `uniffi-bindgen` generator
pkg-web/                  GENERATED, committed — the shipped web artifact
harness/{kotlin,swift}    corpus replay through the generated bindings
scripts/                  build, verify, smoke, bench
```

**Never hand-roll a primitive.** Hashing, curve math, ABI coding and CBOR come
from alloy-core, sha2, p256/ecdsa and ciborium/coset. Exact pins live in the
workspace `Cargo.toml`; the reasoning behind each is in
[`research.md`](../specs/001-rust-core-bindings/research.md).

## Commands

| What | Command |
|---|---|
| Test the crate | `cd rust && cargo test --workspace --features vela-core/i18n-all` |
| Lint | `cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings` |
| Regenerate conformance vectors | `npm --prefix scripts run dump:vectors` (from the repo root; the tooling package is `scripts/`) |
| Regenerate the identicon artwork table | `npm --prefix scripts run gen:identicon-features` |
| Regenerate the i18n tables + `assets/i18n` | `npm --prefix scripts run gen:i18n` |
| Report the i18n corpus defect register | `npm --prefix scripts run lint:i18n` |
| i18n residency (SC-005) | `cargo test -p vela-core --features i18n-all --test i18n_residency -- --nocapture` |
| i18n budget (SC-007) | `cargo test -p vela-core --features i18n-all --release --test i18n_bench -- --nocapture` |
| Identicon parity vs the pinned `identicons-esm` | `npm --prefix scripts run verify:identicon` |
| i18n parity vs the pinned `i18next` | `npm --prefix scripts run verify:i18n` |
| Build the web artifact | `npm --prefix scripts run build:wasm` |
| Verify the shipped web artifact | `npm --prefix scripts run verify:wasm` |
| Kotlin bindings conformance | `rust/scripts/smoke-kotlin.sh` |
| Swift bindings conformance (macOS) | `rust/scripts/smoke-swift.sh` |
| Benchmark the web artifact | `npm --prefix scripts run bench:core` |

CI runs all of these (`rust` job on ubuntu, `rust-macos` for Swift).

## The conformance corpus

`crates/vela-core/tests/vectors/*.json` is the contract. Each case was extracted
by `scripts/dump-vectors/` from the **production TypeScript** — the behavior that
actually shipped — so a red test means a byte divergence from what users' wallets
do today, not a failed unit test. The same corpus is replayed through all four
surfaces: the crate (`cargo test`), the shipped wasm (`verify-web.mjs`), and the
Kotlin and Swift bindings (`smoke-*.sh`). All four must agree.

The identity vector — `compute_safe_address` → `0x762EdA60D3B68755c271D608644650278f88329F`,
cross-referenced with the iOS and Android test suites — is a **release blocker**:
existing users' wallet addresses must never change.

The identicon suites work the same way but have a different oracle: the pinned
`identicons-esm@1.0.1` package rather than this repo's TypeScript
(specs/003-rust-identicon). Their release-blocker rule is the mirror image of the
address one — **existing users' avatars must never change**, because the avatar is
how a user recognises an account at a glance. Two files back it: `identicon.json`
(1,499 curated cases, including all 84 artworks pinned by full text) and
`identicon-bulk.json` (20,000 hashes). A further 200,000-seed differential run
lives in `npm run verify:identicon`, which regenerates rather than commits.

Regenerate the corpus only by re-running the dump against the TypeScript oracle,
and review the resulting diff like code. A changed expectation means either a
TypeScript bug was fixed (document it) or the oracle drifted (investigate).

## Intentional divergences

The core is stricter than the code it replaces in 20 enumerated places — junk
that used to be silently coerced now errors, and four cases where the legacy
TypeScript was simply wrong are fixed. Every one is listed in
[`contracts/core-api.md`](../specs/001-rust-core-bindings/contracts/core-api.md)
with a marked vector, or a Rust-side unit test where no oracle vector is possible
because the old output was garbage. **An un-enumerated behavior change is a bug.**

## Platform notes

- **Web** loads the module synchronously from a base64 payload (`initSync`).
  The shape was chosen for the Expo web build's bundler (Metro could neither
  bundle wasm as ESM nor parse `import.meta`, and Cloudflare Pages dropped
  `node_modules` asset paths — spec 001 research D7); that app is gone (spec
  039), but the SvelteKit shell's build-time engine and `sync-wasm.mjs`, the
  corpus replay (`verify-web.mjs`) and `scripts/onchain/` all read the
  artifact in this form, so it stays. The build script still strips
  `import.meta` and fails loudly if the wasm-bindgen glue ever changes shape.
  `pkg-web/` is committed on purpose; CI fails if it drifts from a fresh
  build — and `--check` fingerprints every Rust file, so a core edit means a
  rebuild.
- **The build remaps `$CARGO_HOME`.** Registry panic-location strings otherwise
  embed the builder's home directory, which both leaks into production and makes
  the artifact impossible to reproduce on another machine.
- **Every shell links this crate.** iOS and Android through the UniFFI
  bindings (with the `crux` feature on since spec 019), the web shell through
  the wasm artifact, the desktop as a crate dependency. The TypeScript
  implementations the corpus was extracted from were deleted in PR #168, and
  the React Native / Expo app that hosted them in spec 039 — there is no
  second implementation anywhere.

## Bumping uniffi

Bindings are checksum-coupled to the uniffi version: Kotlin, Swift and any
consumer must regenerate together, or calls fail at runtime. Bump the workspace
pin, regenerate, and run both smoke harnesses before committing.

### The generator is not part of what ships

`uniffi/cli` links the whole `uniffi_bindgen` generator — plus `uniffi_udl`,
weedle, nom, clap, goblin, `cargo_metadata`, rustix, tempfile, askama and toml
— into whatever crate enables it. It used to be enabled on `vela-core-uniffi`,
the crate that *is* the shipped iOS static library, which measured
169,581,752 bytes and 554 objects on `aarch64-apple-ios` against 122,825,640
bytes and 516 objects without it. Spec 081 FR-014 moved the binary into its own
crate:

```bash
cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate \
  --library target/release/libvela_core_uniffi.dylib --language swift --out-dir bindings/swift
```

`generate --library` reads the interface metadata back out of the **built**
library, so one generator serves `vela-core-uniffi` and
`vela-dev-fixtures-uniffi` and depends on neither. Two rules keep it out:
never re-enable `uniffi/cli` on a library crate, and always build the shipped
library with `-p vela-core-uniffi` — cargo unifies features across the packages
of a single invocation, so a workspace-wide build would fold `cli` back in.
`vela-uniffi-bindgen` is therefore in `members` but not in `default-members`,
and `.github/workflows/ios-package.yml` runs an `nm -u` gate on the archived
app binary as the backstop.

Be accurate about what the split changed, because FR-014's research was not:
`nm -u` over the archived arm64 `.app` is **byte-identical** before and after,
since `ld` was already dead-stripping the unreachable generator. The binary's
`stat`/`fstat`/`lstat` imports — Apple's *FileTimestamp* required-reason API —
come from Rust's own `std` codegen unit, which a static archive links whole,
and they are declared in `app-ios/VelaWallet/VelaWallet/PrivacyInfo.xcprivacy`
for that reason. What the split buys is that 46 MB of build-time-only code and
eleven crates of supply chain are no longer *inside* the artifact a wallet
ships, held out of users' hands by a linker optimisation.


## i18n / L10n

`vela_core::i18n` reproduces `i18next@26.3.1` byte-for-byte; `vela_core::l10n`
reproduced the Expo app's `locale-format.ts` (deleted with that app) and corrects
the currency layer it never handled. Spec: `specs/004-rust-i18n/`.

**The corpus under `rust/crates/vela-core/i18n/locales/` is the only hand-edited
file set.** Everything downstream is generated and CI fails on drift:

| Generated | From |
|---|---|
| `src/i18n/paths.rs` | the shared 1,205-path table |
| `src/i18n_catalogs/*.rs` | one compiled-in value blob per locale |
| `src/l10n/datetime_data.rs` | day periods + weekday names, from ICU |
| `assets/i18n/<lng>.json` | the runtime on-demand asset the web route fetches |
| `../../src/i18n/resources.ts` | what the React Native app imports |
| `tests/vectors/i18n-*.json` | 18,975 conformance cases, from the real JS package |

Edit a string, run `npm run gen:i18n`, and commit the regeneration **with** the
corpus change — a corpus edit without it leaves the app rendering the old string
while the Rust suite renders the new one, and both would be green.

Locales are per-locale cargo features and the default set is **zero**: all 15
compiled in measure 1,315,023 wasm bytes against a 1,000,000 ceiling, and even one
costs more over the wire compiled in than fetched as JSON. Web fetches
`/i18n/<lng>.json` on demand; desktop and native may compile in what they ship.


## `vela-core/src/app/` — portable business state machines

Everything else in `vela-core` is a *kernel*: a pure function the app calls
(`compute_safe_address`, `identicon_svg`, `t`). The `app` module is different in
kind — it holds **state machines** that own what a flow decides, built on
[Crux](https://redbadger.github.io/crux/):

```text
Event ─► update(Model) ─► Model' + Command<Effect>
                                      │
ViewModel = view(Model')              └─► the shell performs I/O and answers
```

Two rules make it worth having, and both are enforceable:

1. **The core declares effects; it never performs them.** No network, no
   storage, no clock, no randomness. Wall-clock instants arrive as fields on the
   results the shell returns. That is what makes every rule testable in
   milliseconds with no browser (`npm run test:core`).
2. **It is feature-gated and off by default,** so a consumer that runs no
   machine pays for none. `crux_core`, `futures` and their trees stay out of its
   dependency graph entirely.

   Until spec 019 this rule had a second half — `vela-core-uniffi` must *never*
   link the framework, "because web is the one runtime that can execute it,
   Hermes has no WebAssembly":

   ```bash
   cargo tree -p vela-core-uniffi | grep -c crux   # was: must be 0
   ```

   That reason was about the **Expo React Native client**, whose JavaScript
   engine genuinely could not load wasm. It stopped describing reality when
   `app-ios` and `app-android` became native Swift and Kotlin: they have no
   Hermes, no wasm, and reach Rust only through uniffi. Keeping the rule would
   not have protected anything — it would have obliged the two native clients to
   re-implement the onboarding rules by hand, the exact duplication this crate
   exists to prevent.

   Since 019 the four clients run the same machines by four routes: the web and
   the browser extension through `vela-core-wasm`, the desktop app by linking
   `vela-core` directly, and iOS and Android through
   `vela-core-uniffi/src/onboarding_bridge.rs` — which is the wasm bridge's
   semantics, not a second implementation of them.

   What replaced the prohibition is a **measurement**: enabling `crux` on the
   uniffi crate costs +785,864 stripped bytes on arm64-v8a (2,369,528 →
   3,155,392). The figure lives in
   `specs/019-onboarding-live-wiring/results.md`, and moving it materially is a
   decision to take again rather than a diff to wave through. What is still an
   invariant:

   ```bash
   cargo tree -p vela-core-wasm | grep -c crux     # must be > 0
   ```

The TypeScript mirrors of the wire types are generated, committed and gated:
`npm run gen:onboarding-types` (add `-- --check` for the drift gate).

Design, contracts and the rule-to-test map: `specs/011-crux-onboarding-state/`,
superseded for the shell surface by
`specs/019-onboarding-live-wiring/contracts/shell-operations.md`.
