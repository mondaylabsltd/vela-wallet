# PeerConnect JavaScript client (vendored)

The transport under WalletPair (`src/lib/walletpair/`): end-to-end encrypted
two-peer rooms through a relay that reads nothing and keeps nothing. Copied
unmodified from PeerConnect's built `js/` package, because PeerConnect is not
yet published to a registry.

| | |
| --- | --- |
| Source | PeerConnect, commit `f3148cb` (2026-10-28), `js/` as built that day |
| Protocol | PeerConnect v1 rooms |
| Verified | PeerConnect's own `js` tests (7/7) against `peerconnect-relay` from the same commit; two peers through `wss://relay.peerconnect.dev` |
| License | MIT (PeerConnect is MIT OR Apache-2.0) |

Files: `index.js`, `index.d.ts` (the API); `dist/peerconnect.js`,
`dist/peerconnect.d.ts` (wasm-bindgen bindings); `dist/module.js` (the
WebAssembly module, inlined as base64).

## Updating

In a PeerConnect checkout: `sh js/scripts/build.sh`, then `(cd js && npm test)`,
then copy `js/index.js js/index.d.ts` here and `js/dist/{peerconnect.js,peerconnect.d.ts,module.js}`
to `dist/`, and update the commit above.
