# peerconnect

Two peers, one room, end-to-end encryption and reliable delivery over an untrusted, stateless WebSocket relay.

One side creates a room and hands the other an invite (a QR code, a link). From then on the two can reconnect whenever they like, from wherever they are, and messages sent while one of them is away arrive when it comes back. The relay forwards ciphertext between the two seats of a room: it cannot read, forge or replay messages, and it stores nothing.

```toml
[dependencies]
peerconnect = { version = "0.1", features = ["client"] }
```

```rust
use peerconnect::{client, Event, Room, State};

let room = Room::join(&invite.parse()?);
save(&room.to_string());               // before connecting: it holds the identity the inviter pins
let mut conn = client::connect(room);
conn.send(b"hello").await?;            // queued until the inviter is reachable
while let Some(event) = conn.next().await {
    match event {
        Event::Message(data) => println!("{}", String::from_utf8_lossy(&data)),
        Event::Paired => save(&conn.room().to_string()),
        Event::State(State::Closed(_)) => break,
        _ => {}
    }
}
```

Without the `client` feature the crate does no I/O: `Peer` is a state machine that any WebSocket library and event loop can drive.

- Website, guide and live demo: <https://peerconnect.dev>
- Specification: <https://peerconnect.dev/spec>
- Relay server: [`peerconnect-relay`](https://crates.io/crates/peerconnect-relay) (`cargo install peerconnect-relay`)
- JavaScript SDK: [`peerconnect` on npm](https://www.npmjs.com/package/peerconnect)
- Source: <https://github.com/atshelchin/peerconnect>

The protocol is a v1 draft and has not been externally audited.

Licensed under the [MIT license](https://github.com/atshelchin/peerconnect/blob/main/LICENSE).
