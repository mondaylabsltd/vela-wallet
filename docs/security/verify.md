# How to verify Vela's web core and signing page yourself

Two things Vela ships to browsers are built bytes committed to this repository, not code a
browser compiles from source:

- **the web core**: `assets/wasm/vela_core_bg.<source>.wasm` and its glue `rust/pkg-web/`, the
  Rust core (`rust/crates/vela-core`) compiled to WebAssembly. The web wallet, the Chrome
  extension and the Safari extension run it, and it makes every decision the native apps'
  core makes;
- **the signing page**: `app-web/trusted-signer/dist/b/<sha256>/sign.html`, one file with its
  own `default-src 'none'` CSP, published at `https://sign.getvela.app/b/<sha256>/sign`.

This page says how to rebuild both from a commit and check them against what is committed and
what is served, what each check proves, and what it does not. Each step here is also a CI job
(`.github/workflows/ci.yml`: `web-core-reproducible`, `signing-page`, `supply-chain`), so
"the committed bytes are what this commit builds" is checked on every pull request. The
decisions behind this are in [`hardening.md`](hardening.md), Round 22.

You need `git`, Docker (or Podman) for the web core, and Bun 1.4.2 or Node 22 for the page.
Nothing here needs an account, a key or a Vela service.

---

## 1. The web core

### Rebuild it and compare

```sh
git clone https://github.com/mondaylabsltd/vela-wallet && cd vela-wallet
git checkout <commit or tag>
rust/scripts/reproduce-web.sh            # or: rust/scripts/reproduce-web.sh <commit>
```

The script exports the commit with `git archive` (no build output, no untracked file) and builds
the image `rust/scripts/web-build.Dockerfile` describes. It then runs
`node rust/scripts/build-web.mjs` inside that image and compares every output byte for byte with
what the commit carries. It prints one line per file, with its sha256:

```
  same     assets/wasm/vela_core_bg.<source>.wasm <sha256>
  same     rust/pkg-web/vela_core.js <sha256>
  …
reproduce-web: the committed web core is exactly what <commit> builds
```

The exit status is 0 only if every file is identical.

**What is pinned.** Every tool is named by an exact version, and every download by its sha256:

| Tool | Version | Pinned by |
| --- | --- | --- |
| rustc, cargo, std | 1.97.1 | `rust-toolchain.toml`; the base image `rust:1.97.1-bookworm` by its index digest |
| crates | `rust/Cargo.lock` | crates.io checksums (`--locked`) |
| wasm-pack | 0.15.0 | built from crates.io with `--locked` |
| wasm-bindgen CLI | 0.2.126 | equal to `wasm-bindgen` in `Cargo.lock`, built with `--locked` |
| wasm-opt | binaryen `version_117` | the release tarball's sha256 |
| node | 22.22.3 | the release tarball's sha256 |

**What was made reproducible** (`rust/scripts/build-web.mjs`, Round 22):

- Every path of the builder's machine is mapped out of the module (`--remap-path-prefix`).
  Before, the shipped module embedded `/Users/<developer>/.rustup/toolchains/…` in 27
  panic-location strings, because a toolchain with the `rust-src` component writes the
  standard library's paths as local paths. Those now read `/rustc/<commit>/library/…`, as they
  do on a machine without `rust-src`. The registry is `/cargo/registry/…`, and this checkout is
  `/vela`.
- The `producers` custom section is dropped. It records which tools touched the module, and
  wasm-bindgen's entry depends on how that CLI was installed (`0.2.126`, or `0.2.126 (<hash>)`
  for a downloaded binary). Nothing reads it at run time. This section lists the toolchain
  instead.

**Measured** (Round 22, on the developer machine):
- Two clean builds on macOS arm64 were byte-identical: the same tree each time, a fresh target
  directory each time, and the sha256 of each module was `43e1933b…` for that tree. No path of
  the build machine remains in the module.
- **Linux was not measured here.** The local Docker VM (2 CPUs, 3.8 GB) killed rustc on
  `vela-core` under `linux/amd64` emulation (out of memory), and the other container runtime
  had no network.
- So it is not yet shown that the CI container's bytes equal a macOS build's; the CI job is
  what checks it. If they differ, the job fails and keeps what it built. The fix is then to
  commit the container's build, so the shipped module *is* the CI-built, attested one.

On a Mac, Docker runs the `linux/amd64` image under emulation: it is slow, and it needs
several GB of memory for `vela-core`'s LTO. If Docker Hub is out of reach, set `VELA_BASE_IMAGE` to a
mirror of the same digest, for example
`VELA_BASE_IMAGE=m.daocloud.io/docker.io/library/rust:1.97.1-bookworm@sha256:0e2bcaef…`. The
script refuses any other digest. If Docker Desktop injects a proxy that its builds cannot use,
`VELA_BUILD_ARGS="--build-arg HTTPS_PROXY= --build-arg HTTP_PROXY="` blanks it, and
`VELA_BINARYEN_DIST` / `VELA_NODE_DIST` point the two tarballs at a mirror (their sha256 is
pinned).

### Check what wallet.getvela.app serves

The module's name is in `rust/pkg-web/vela_core_wasm_url.js` (for example
`/vela_core_bg.ced9d9bf6e0c.wasm`). Fetch it and hash it:

```sh
name=$(sed -n "s/.*WASM_URL = '\/\(.*\)';/\1/p" rust/pkg-web/vela_core_wasm_url.js)
curl -s --compressed "https://wallet.getvela.app/$name" | shasum -a 256
shasum -a 256 "assets/wasm/$name"
```

The two must match. `--compressed` decodes the brotli transfer encoding, so you hash the module
itself.

### Check that GitHub built it (attestation)

On every push to `main`, the `web-core-reproducible` job attests the module it built in the
container (GitHub build provenance, Sigstore-signed, bound to the workflow run and commit):

```sh
gh attestation verify "assets/wasm/$name" --repo mondaylabsltd/vela-wallet
```

An attestation shows that this repository's CI built these exact bytes from the named commit.
It does not show that the source is benign. Reading the source is still your job; the
reproducible build only removes the question "is the binary the source?".

### What this does not prove

- That the deployed Worker serves this module to every visitor. A server can serve one visitor
  different bytes. The check above shows what it served *you*.
- The rest of the web wallet's JavaScript is built by Vite on Cloudflare's builder from
  `app-web/vela-wallet`. That build is not covered by this check: it is reproducible in
  principle (`pnpm install --frozen-lockfile && pnpm build`), but nothing here compares it
  byte for byte. The decisions the core makes are in the module above. The JavaScript renders
  them and carries them out.

---

## 2. The signing page

### Rebuild it and compare

```sh
cd app-web/trusted-signer
bun samples/build-single.mjs --check      # or: node samples/build-single.mjs --check
```

`--check` rebuilds the page from `src/` (concatenation only: no minifier, no timestamp, no
environment) and fails if any of these hold:

- the page it builds is not committed at `dist/b/<its sha256>/sign.html`;
- a published version does not hash to its own path;
- `dist/index.json`, `dist/_headers` or `dist/sw.js` is not what this build writes;
- two builds of the same sources disagree.

Bun and Node produce the same bytes. CI runs both on every pull request (the `signing-page`
job).

### Check what sign.getvela.app serves

```sh
h=<a version>     # e.g. the first entry of BUILD_ALLOWED in
                  # rust/crates/vela-core/src/trusted_signer/integrity.rs
curl -s "https://sign.getvela.app/b/$h/sign" | shasum -a 256     # must print $h
```

The page's version is its sha256. A wallet accepts only the versions its own build lists
(`BUILD_ALLOWED`, newest first), so the list in the app's source is the trust root. No
signature or index is needed: `dist/index.json` only says which versions the host still serves,
and a wallet ignores any version it does not already know.

### How each wallet makes sure the page it opens is the page it checked

| Where | How the page is opened | What stops a host swapping it after the check |
| --- | --- | --- |
| Web wallet (`wallet.getvela.app`), official page | **from the bytes it hashed**, as a `blob:` URL (`CheckedPage::opens_from_bytes`) | the window never asks the host |
| Web wallet, a self-hosted page; Chrome and Safari extensions | at the checked address | the page's Service Worker pin, after the first visit |
| iOS, Android, desktop | at the checked address, in the system browser | the Service Worker pin, after the first visit; a check at most 10 minutes old |

The Service Worker (`dist/sw.js`, registered by the page at `b/<h>/`) serves each version only
as bytes whose sha256 is the version in its address. It pins the version it was first opened
as. You can watch it work: `node samples/sw-pin-test.mjs` (`CHROME_BIN` set to a Chrome for
Testing) serves the page from a host that later swaps it.

### What the page guarantees, and what it does not

This depends on the account (see `hardening.md`, Round 22, §3):

- **A `getvela.app` account** (made in the apps or on the official page). The page gives
  *display assurance*: it decodes the request with its own code, from bytes that match a
  published version, and signs only the digest it computed itself. A wallet whose interface is
  wrong or tampered with cannot make the page show one thing and sign another. **It is not an
  authority boundary.** The wallet's own origin, the extension and the apps can all ask the
  same passkey for a signature directly. A fully compromised wallet does not need the page.
- **An account on its own domain** (made on a self-hosted page). Only pages on that domain can
  use its keys. The wallet cannot sign without the page. The page is the boundary, as long as
  the page's host is honest, or the Service Worker has pinned it.

---

## 3. The vendored PeerConnect client

`app-web/vela-wallet/src/lib/vendor/peerconnect/` is PeerConnect's built JavaScript client
(multisig pairing). `VENDOR.json` records its source commit, toolchain and file hashes.

```sh
node scripts/check-peerconnect-vendor.mjs             # the hashes
node scripts/check-peerconnect-vendor.mjs --rebuild   # clone the commit, build, compare byte for byte
```

## 4. Dependencies

```sh
cargo deny --manifest-path rust/Cargo.toml --config deny.toml check
cargo deny --manifest-path app-desktop/vela-wallet/Cargo.toml --config deny.toml check
(cd app-web/vela-wallet && pnpm audit --prod --audit-level high && pnpm audit --audit-level critical)
(cd app-web/getvela.app && bun audit --prod --audit-level=high)
```

Every advisory, licence or source this tree allows is in `deny.toml`, each with its reason and
the way out.

## 5. Native releases

Release files (APK/AAB, IPA, DMG, MSI, Linux packages, the extension zips) carry GitHub build
provenance: `gh attestation verify <file> --repo mondaylabsltd/vela-wallet` (see
[`docs/RELEASING.md`](../RELEASING.md)). Those builds are attested, not yet reproducible
byte for byte. The native apps link the same Rust core as the web, built from the same
`rust/Cargo.lock`.
