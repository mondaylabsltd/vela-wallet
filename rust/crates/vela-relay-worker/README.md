# vela-relay-worker

The Clear Signer's cross-device relay (`vela-relay/1`) as a Cloudflare Worker. The
contract is
[`specs/075-clear-signer-channel/contracts/relay.md`](../../../specs/075-clear-signer-channel/contracts/relay.md),
and the rules are the shared [`vela-relay`](../vela-relay) crate, which the native
server ([`vela-relay-server`](../vela-relay-server)) runs too.

- The **Worker** answers `GET /healthz` and refuses bad room URLs with HTTP 400
  before any upgrade. It hands each room URL to that room's Durable Object.
- The **Durable Object** `RelayRoom` is one per room (`idFromName(room id)`). It
  holds the room's two sockets on the WebSocket **hibernation** API, so an idle
  room costs no memory. What it must remember travels in each socket's
  attachment: the connection id, role, room creation time and last frame time.
  Every event rebuilds the `Room` from those, applies the shared rules and
  carries out their actions. The room's life and idle clocks are one alarm, and
  an empty room deletes it. Nothing else is stored.
- The hibernation API answers pings itself, so this host sends none.

This crate builds only for `wasm32-unknown-unknown`, so it is its own Cargo
workspace (the `rust/` workspace excludes it) and has its own `Cargo.lock`.

## Build and run locally

`worker-build` (0.8.x, matching the `worker` crate) and `wrangler` are needed.

```sh
cd rust/crates/vela-relay-worker
worker-build --release                    # → build/index.js + build/index_bg.wasm
wrangler dev --local --port 8788          # Miniflare; runs the build command itself
node ../vela-relay/tests/conformance.mjs http://127.0.0.1:8788
node ../vela-relay/tests/conformance.mjs http://127.0.0.1:8788 --slow   # + time limits, ~11 min
```

## Deploy (not done from this repo yet)

`wrangler deploy` from this directory creates the `vela-relay` Worker with the
`RelayRoom` class (migration `v1`, SQLite-backed). To serve the default relay
URL `wss://relay.getvela.app`, add a custom domain for the Worker. The
Worker needs no secrets and no other bindings.

## Runtime behaviour worth knowing

These were measured on workerd 1.20260430 (wrangler 4.87) and 1.20260722
(wrangler 4.115), locally. They have not been checked on Cloudflare's edge.

- **A close the room decides on its own clock** (idle, expired) sends its close
  frame at once, and the peer's `left` goes out at once. But the runtime tears
  down that TCP connection only when the object next goes idle, about 10 s
  after the room's last event. That can be up to the room's end if the other
  end keeps talking. A close issued while handling that same socket's own
  frame (1009) tears down at once. An end that acts on the close frame loses
  nothing. An end that waits for the teardown waits: browsers have their own
  timeout for it (Chromium's is a few seconds), and Node's `WebSocket` has
  none, which is why the conformance script's slow checks allow 15 s of
  slack.
- **A refused connection (4409)** is accepted outside the hibernation set and
  closed at once. A hibernatable socket closed before its 101 has gone out
  never delivers its close frame. The runtime logs
  `Uncaught Error: Network connection lost.` when such a socket's client goes
  away. The refusal still arrives as 4409.
- `compatibility_date` is 2026-04-07 for `web_socket_auto_reply_to_close`: the
  runtime answers a client's close frame on a hibernated socket itself.
