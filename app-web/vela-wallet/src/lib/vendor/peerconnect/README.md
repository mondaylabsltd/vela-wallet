# PeerConnect JavaScript client (vendored)

The transport under WalletPair (whose messages the core's
`multisig_outside` machines speak; `src/lib/multisig/shell/peer.ts` binds
this client for them): end-to-end encrypted
two-peer rooms through a relay that reads nothing and keeps nothing. Copied
from PeerConnect's built `js/` package, because PeerConnect is not yet
published to a registry — with one local addition, `Connection.reconnectNow()`
(r4C-15: when the network comes back, a connection waiting out its backoff,
up to 30 s, tries the relay at once). Keep it when updating, until PeerConnect
has its own.

| | |
| --- | --- |
| Source | PeerConnect `977ec1e2175e32d5827c8646d1a9961cab3f0607` (2026-10-28, "Relay tokens: required on demand, revocable, metered"), `js/` — https://github.com/atshelchin/peerconnect |
| Built with | rustc 1.97.1, `cargo build --locked -p peerconnect-wasm --target wasm32-unknown-unknown --profile wasm`, wasm-bindgen 0.2.129 (`--target web --weak-refs`), no wasm-opt, `js/scripts/inline.mjs`; this machine's paths mapped out of the module |
| Local change | `vela-local.patch`: `Connection.reconnectNow()` on `js/index.js` and `js/index.d.ts` |
| Protocol | PeerConnect v1 rooms |
| License | MIT (PeerConnect is MIT OR Apache-2.0) |

`VENDOR.json` records all of it, with the sha256 of every vendored file.

## Verifying it (§38, review 1-10)

```sh
node scripts/check-peerconnect-vendor.mjs            # the hashes (CI: app)
node scripts/check-peerconnect-vendor.mjs --rebuild  # rebuild from the commit, byte for byte (CI: supply-chain)
```

`--rebuild` clones the recorded commit into a temporary directory, builds it as
above, applies `vela-local.patch` to the upstream `index.js` / `index.d.ts` and
compares all five files. It needs rustup's 1.97.1 with `wasm32-unknown-unknown`
and `wasm-bindgen` 0.2.129 on PATH. `--rebuild <checkout>` builds from a local
clone instead of GitHub.

**Correction, 2026-11-08.** Until §38 this file said the source was commit
`f3148cb`. It was not: `index.js` carried the relay-token API of the next
commit, `977ec1e`. The module itself had been built from an uncommitted
working tree between the two (one panic line number differed), so it matched
no commit at all. It was rebuilt from `977ec1e` with the recipe above. The
only change is that line number and the builder's paths, now mapped out.

## Updating

Pick a PeerConnect commit, run its own `js` tests (`cd js && npm test`)
against `peerconnect-relay` from the same commit, then set `commit` (and
`toolchain` / `wasmBindgen` if they move) in `VENDOR.json`, refresh
`vela-local.patch` if `index.js` moved, and run:

```sh
node scripts/check-peerconnect-vendor.mjs --rebuild --write
node scripts/check-peerconnect-vendor.mjs --rebuild
```
