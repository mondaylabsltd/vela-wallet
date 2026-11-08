# The pinned builder of the web core (§38, review 1-9; docs/security/verify.md).
#
# Every tool that touches rust/pkg-web and assets/wasm/vela_core_bg.*.wasm is
# named here by an exact version and, where it is a download, by its sha256:
#
#   rustc / cargo     1.97.1, the official image by digest (rust-toolchain.toml)
#   wasm32 target     the same toolchain's, installed below
#   wasm-pack         0.15.0, built from crates.io with --locked
#   wasm-bindgen-cli  0.2.126 (= wasm-bindgen in rust/Cargo.lock), --locked;
#                     wasm-pack takes the one on PATH when its version matches
#   wasm-opt          binaryen version_117 (what wasm-pack 0.15.0 would fetch,
#                     unverified, at build time) — on PATH, so wasm-pack uses it
#   node              22.22.3 (runs rust/scripts/build-web.mjs)
#
# Nothing is fetched while the module builds except crates.io sources pinned
# by Cargo.lock's checksums. Build it, then run the build on a clean export of
# the commit: rust/scripts/reproduce-web.sh does both and compares the bytes
# with what is committed.

# The image by its index digest. BASE may name a mirror of the SAME digest
# (`VELA_BASE_IMAGE` in reproduce-web.sh) where Docker Hub is out of reach;
# the digest is what makes it the same image.
ARG BASE=rust:1.97.1-bookworm@sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97
FROM ${BASE}

ARG TARGETARCH
ARG NODE_VERSION=22.22.3
ARG NODE_SHA256_X64=2e5d13569282d016861fae7c8f935e741693c269101a5bebcf761a5376d1f99f
ARG NODE_SHA256_ARM64=1c4a9933a5e45bc88f54f70b5f91232c127ec49f1a5989d23fb85824c7adf9b7
ARG BINARYEN_VERSION=117
ARG BINARYEN_SHA256_X64=3dc677006555b355ea2da5e82602065a161d5e83eaefd3f759afa00b96e83212
ARG BINARYEN_SHA256_ARM64=ad560204426015a815faa45693c83bef7d58677d38a39422c272a30ba4b6da2a
# Where the two tarballs come from. The sha256 above is what makes them the
# right ones, so a mirror (or a file server on the host, where a network cannot
# reach the origin) is as good: reproduce-web.sh passes VELA_NODE_DIST /
# VELA_BINARYEN_DIST through.
ARG NODE_DIST=https://nodejs.org/dist
ARG BINARYEN_DIST=https://github.com/WebAssembly/binaryen/releases/download

RUN rustup target add wasm32-unknown-unknown \
 && rustup component add rustfmt clippy

RUN set -eu; \
    case "$TARGETARCH" in \
      amd64) node=x64;   node_sha=$NODE_SHA256_X64;   bin=x86_64-linux;  bin_sha=$BINARYEN_SHA256_X64 ;; \
      arm64) node=arm64; node_sha=$NODE_SHA256_ARM64; bin=aarch64-linux; bin_sha=$BINARYEN_SHA256_ARM64 ;; \
      *) echo "unsupported TARGETARCH=$TARGETARCH" >&2; exit 1 ;; \
    esac; \
    curl -fsSLo /tmp/node.tar.xz "$NODE_DIST/v$NODE_VERSION/node-v$NODE_VERSION-linux-$node.tar.xz"; \
    echo "$node_sha  /tmp/node.tar.xz" | sha256sum -c -; \
    tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1; \
    curl -fsSLo /tmp/binaryen.tgz "$BINARYEN_DIST/version_$BINARYEN_VERSION/binaryen-version_$BINARYEN_VERSION-$bin.tar.gz"; \
    echo "$bin_sha  /tmp/binaryen.tgz" | sha256sum -c -; \
    tar -xzf /tmp/binaryen.tgz -C /opt; \
    ln -s "/opt/binaryen-version_$BINARYEN_VERSION/bin/wasm-opt" /usr/local/bin/wasm-opt; \
    rm /tmp/node.tar.xz /tmp/binaryen.tgz; \
    node --version; wasm-opt --version

RUN cargo install --locked wasm-pack --version 0.15.0 \
 && cargo install --locked wasm-bindgen-cli --version 0.2.126 \
 && rm -rf /usr/local/cargo/registry /usr/local/cargo/git \
 && wasm-pack --version && wasm-bindgen --version

# The checkout is mounted here; build-web.mjs maps this path (and the cargo
# home) out of the module, so where it lives on the host does not matter.
WORKDIR /vela
