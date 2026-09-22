# vela-relay-server

The Clear Signer's cross-device relay (`vela-relay/1`) as a native server. It pairs
two WebSockets per room and forwards their frames, and it never sees their contents
in the clear (the ends encrypt end to end). The contract is
[`specs/075-clear-signer-channel/contracts/relay.md`](../../../specs/075-clear-signer-channel/contracts/relay.md).
The room rules are the shared [`vela-relay`](../vela-relay) crate, which the
Cloudflare Worker ([`vela-relay-worker`](../vela-relay-worker)) runs too.

```
GET /v1/rooms/{room}?role=requester|signer    WebSocket upgrade
GET /healthz                                   200 "ok"
```

## Docker

The build context is the `rust/` workspace:

```sh
cd rust
docker build -f crates/vela-relay-server/Dockerfile -t vela-relay .
docker run --rm -p 8787:8787 vela-relay
```

The image is a static musl binary on `gcr.io/distroless/static-debian12:nonroot`
(about 3.5 MB). It has no shell, so to probe health use HTTP `GET /healthz` from
outside it. `docker stop` is handled (SIGTERM), not waited out.

## Without Docker

```sh
cd rust
cargo run --release -p vela-relay-server          # 0.0.0.0:8787
PORT=9000 cargo run --release -p vela-relay-server
```

## Running it for real

- **TLS is not built in.** The server speaks plain `ws://`. Put it behind a proxy
  that terminates TLS (Caddy, nginx, a Cloudflare Tunnel, a load balancer) so the
  wallet and the page reach it as `wss://`. The proxy must pass WebSocket
  upgrades and keep idle connections at least 150 s (the relay's own idle limit
  is 120 s; it pings every 30 s).
- **One process holds all rooms in memory.** Both ends of a room must reach the
  same process, so don't load-balance across replicas without sticky routing on
  the room path. A restart drops open rooms (the ends reconnect and pair again).
- **Logs** are room counts and lifetimes only: never room ids, never frame
  contents.

## Limits (from `vela-relay`)

| | |
|---|---|
| Frame | 256 KiB, then close 1009 |
| Room life | 10 min from its first connection, then close 4408 `expired` (both ends) |
| Idle end | 120 s without a text or binary frame, then close 4408 `idle` |
| Taken role | the newcomer is closed 4409 `role taken`; the incumbent stays |
| Ping | every 30 s |

The host adds three limits of its own. It gives up on an end that hasn't taken
a queued frame in 10 s (closed 4408, so one stuck socket can't stall its room).
It allows 10 s to send request headers. It never buffers more than 512 KiB of
one message, so anything larger is refused 1009 before it is read in full.

## Tests

```sh
cd rust
cargo test -p vela-relay -p vela-relay-server
node crates/vela-relay/tests/conformance.mjs http://127.0.0.1:8787          # ~5 s
node crates/vela-relay/tests/conformance.mjs http://127.0.0.1:8787 --slow   # + the time limits, ~11 min
```
