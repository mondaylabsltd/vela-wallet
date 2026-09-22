# Contract — the Clear Signer relay (`vela-relay/1`)

The cross-device WebSocket channel (owner, 2026-09-22: "跨端时，websocket 蓝牙是核心通道";
"wss 中继需要支持 docker 和 cloudflare worker，采用 rust 去写"). It supersedes the line
"没有服务器。没有中继。" in `app-web/clearsigning/PROTOCOL.md`; everything else there stands.

The relay is **dumb and blind**: it pairs two sockets and forwards their frames. It never
sees a key, a signature or an intent in the clear — the two ends run the same ECDH +
HKDF + AES-GCM session the BLE channel runs (PROTOCOL.md §3), under their own label.

## 1. Rooms

```
GET /v1/rooms/{room}?role=requester|signer      (WebSocket upgrade)
GET /healthz                                     → 200 "ok"
```

- `room` — 22 characters of base64url (128 random bits), chosen by the requester (the
  wallet). Anything else → HTTP 400 before the upgrade.
- A room is created by its first connection and holds **at most one requester and one
  signer**. A second connection in a taken role is accepted and closed at once with
  **4409** `role taken`; the incumbent is untouched.
- When both roles are present the relay sends each of them the text frame
  `{"v":1,"relay":"joined"}`. When one leaves, the other receives
  `{"v":1,"relay":"left"}` and the room waits for a reconnect in that role.
- **No buffering.** A frame sent while the peer is absent is dropped. Ends start their
  handshake only after `joined`.
- **Forwarding.** Every frame from one end (text or binary) reaches the other end
  byte-for-byte, in order. The relay's own frames are text frames whose JSON has a
  `"relay"` key. An end's own JSON never has one (§3).
- **Limits** (each a constant in the shared crate):
  - A frame over **256 KiB** → close **1009**.
  - A room lives at most **10 minutes** from creation → close **4408** `expired`, both ends.
  - An end with no frame for **120 s** → close **4408**.
  - A room left empty is forgotten at once.
- Pings: the host sends a WebSocket ping every 30 s where its platform lets it (the
  Worker's hibernation API answers pings itself).
- Close codes: 1000 normal, 1009 too big, 4400 bad request (bad room id or role after
  the upgrade), 4408 expired/idle, 4409 role taken.
- CORS / Origin: any origin may connect — the security is end-to-end (§2), and the page
  is served from many origins (the official one, and people's own deployments).
- No accounts, no persistence, no logs of frame contents. Hosts may log room lifetimes
  and counts.

## 2. The session inside a room (both ends, not the relay)

Exactly PROTOCOL.md §3 with the label `vela-relay/1` in place of `vela-ble/1`, and no
BLE framing (a WebSocket message is already whole):

1. After `joined`, the **signer** (the page) sends a text frame
   `{"v":1,"t":"hello","role":"signer","pk":"<b64url 65-byte raw P-256>","nonce":"<b64url 16 bytes>"}`.
2. The **requester** (the wallet) answers with
   `{"v":1,"t":"hello","role":"requester","pk":…,"nonce":…,"app":"vela-ios/0.9.4"}`.
3. Both derive:
   ```
   shared = ECDH(own private key, peer public key)
   salt   = nonce(signer) ‖ nonce(requester)
   key    = HKDF-SHA256(shared, salt, "vela-relay/1 key",  32)
   code   = HKDF-SHA256(shared, salt, "vela-relay/1 code",  4)   → BE u32 mod 1e6, 6 digits
   ```
4. **Pairing check.** The requester's pairing link carries
   `rk = b64url(SHA-256(requester pk)[0..16])`. The page refuses a requester hello whose key
   does not hash to `rk`, which stops a relay (or anybody who guessed the room) from
   standing in for the wallet. The **6-digit code** is then shown on both screens and the
   person confirms it on the wallet before the wallet sends anything. That stops a stand-in
   for the page (a stolen link), which matters most for a create: a substituted page would
   hand the wallet somebody else's key.
5. Every later message is a **binary** frame `IV(12) ‖ AES-GCM(ciphertext‖tag)`:
   - IV = `"C2P."` or `"P2C."` ‖ 8-byte big-endian counter, per direction, starting at 1,
     never reused.
   - c2p = signer → requester, p2c = requester → signer (the BLE names: the page is the
     central).
   - AAD = `"vela-relay/1|" ‖ direction ‖ "|" ‖ counter-as-decimal`.
   - The plaintext is the UTF-8 JSON of PROTOCOL.md §4 (`intent` / `result` / `error` /
     `bye`, each with its monotonically increasing `n`). A message whose `n` does not
     increase is dropped.

## 3. The pairing link

```
<signer page URL>#relay=<wss URL, percent-encoded>&room=<room>&rk=<rk>&v=1
```

The wallet shows it as a QR code and as a copyable link. The page reads it from its
fragment (never sent to any server) and connects as `signer`. The default relay is
`wss://relay.getvela.app`; the wallet's Settings let a person name their own (same rule
as the Clear Signer page: https/wss only, loopback allowed).

## 4. Hosts

One Rust crate holds the room rules; two thin hosts run it:

| Crate | Target | Runs as |
|---|---|---|
| `rust/crates/vela-relay` | any (no I/O) | room state machine, limits, close codes, frame classification |
| `rust/crates/vela-relay-server` | native (tokio) | a binary + `Dockerfile` (distroless/static), `PORT` env, `/healthz` |
| `rust/crates/vela-relay-worker` | `wasm32-unknown-unknown` via `worker-build` | a Cloudflare Worker whose Durable Object (one per room) holds the two sockets on the hibernation API; `wrangler.toml` |

Both hosts pass the same conformance script (`rust/crates/vela-relay/tests/conformance.mjs`,
two WebSocket clients against a URL): pairing, forwarding both kinds of frame, 4409,
1009, `left` + reconnect, and the ids refused.
