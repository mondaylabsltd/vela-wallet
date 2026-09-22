# syntax=docker/dockerfile:1
#
# The Clear Signer relay (vela-relay/1), native host — see README.md.
#
# The build context is the rust/ workspace (cargo needs the workspace manifests
# and the lockfile); Dockerfile.dockerignore next to this file keeps it small.
#
#   cd rust
#   docker build -f crates/vela-relay-server/Dockerfile -t vela-relay .
#   docker run --rm -p 8787:8787 vela-relay

# Build: a static musl binary, so the image needs no libc at all. The tag pins
# the same toolchain as rust/rust-toolchain.toml (kept out of the context, so
# rustup does not fetch that file's wasm target and components here).
FROM rust:1.97.1-alpine AS build
WORKDIR /src
COPY . .
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    cargo build --release --locked -p vela-relay-server \
    && cp target/release/vela-relay-server /vela-relay-server

# Run: distroless static — no shell, no package manager, a non-root user.
FROM gcr.io/distroless/static-debian12:nonroot
COPY --from=build /vela-relay-server /vela-relay-server
ENV PORT=8787
EXPOSE 8787
USER nonroot
ENTRYPOINT ["/vela-relay-server"]
