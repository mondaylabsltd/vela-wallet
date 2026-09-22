//! The native host end to end, over real sockets: the parts of the contract that
//! take seconds, not minutes. (The ten-minute and two-minute limits are the
//! rules crate's unit tests; `vela-relay/tests/conformance.mjs` runs the same
//! checks against any host by URL.)

use std::net::SocketAddr;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::{Bytes, Error as WsError, Message};
use tokio_tungstenite::{client_async, WebSocketStream};
use vela_relay::{Control, MAX_FRAME_BYTES};

type Ws = WebSocketStream<TcpStream>;

const WAIT: Duration = Duration::from_secs(5);

async fn start() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(vela_relay_server::serve(listener));
    addr
}

async fn connect(addr: SocketAddr, room: &str, role: &str) -> Result<Ws, WsError> {
    let url = format!("ws://{addr}/v1/rooms/{room}?role={role}");
    let stream = TcpStream::connect(addr).await.unwrap();
    let (ws, _) = client_async(url.into_client_request().unwrap(), stream).await?;
    Ok(ws)
}

async fn next(ws: &mut Ws) -> Message {
    loop {
        match timeout(WAIT, ws.next())
            .await
            .expect("timed out")
            .expect("stream ended")
        {
            Ok(Message::Ping(_) | Message::Pong(_)) => continue,
            Ok(msg) => return msg,
            Err(err) => panic!("read failed: {err}"),
        }
    }
}

async fn joined(ws: &mut Ws) {
    assert_eq!(next(ws).await, Message::text(Control::Joined.text()));
}

async fn closed_with(ws: &mut Ws, code: u16) {
    match next(ws).await {
        Message::Close(Some(frame)) => assert_eq!(frame.code, CloseCode::from(code)),
        other => panic!("expected close {code}, got {other:?}"),
    }
}

/// Nothing arrives for a short while.
async fn quiet(ws: &mut Ws) {
    if let Ok(Some(Ok(msg))) = timeout(Duration::from_millis(300), ws.next()).await {
        panic!("expected nothing, got {msg:?}");
    }
}

async fn http_get(addr: SocketAddr, path: &str) -> String {
    let mut stream = TcpStream::connect(addr).await.unwrap();
    let request = format!("GET {path} HTTP/1.1\r\nHost: relay\r\nConnection: close\r\n\r\n");
    stream.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    timeout(WAIT, stream.read_to_string(&mut response))
        .await
        .unwrap()
        .unwrap();
    response
}

fn room(n: u8) -> String {
    format!("test-room-{n:02}-aaaaaaaaa")
}

#[tokio::test]
async fn healthz_answers_ok() {
    let addr = start().await;
    let response = http_get(addr, "/healthz").await;
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(response.ends_with("\r\n\r\nok"), "{response}");
}

#[tokio::test]
async fn bad_room_ids_and_roles_are_refused_before_the_upgrade() {
    let addr = start().await;
    let good = room(1);
    for (room, role) in [
        ("short", "signer"),
        ("test-room-01-aaaaaaaaaa", "signer"),
        ("test.room.01.aaaaaaaaa", "signer"),
        (good.as_str(), "admin"),
        (good.as_str(), ""),
    ] {
        match connect(addr, room, role).await {
            Err(WsError::Http(response)) => assert_eq!(response.status(), 400, "{room} {role}"),
            other => panic!("{room} {role}: expected 400, got {:?}", other.map(|_| ())),
        }
    }
    let response = http_get(addr, "/elsewhere").await;
    assert!(response.starts_with("HTTP/1.1 404"), "{response}");
}

#[tokio::test]
async fn pairs_forwards_both_ways_and_refuses_a_taken_role() {
    let addr = start().await;
    let id = room(2);
    let mut requester = connect(addr, &id, "requester").await.unwrap();
    // No buffering: this frame has nobody to go to.
    requester.send(Message::text("early")).await.unwrap();
    let mut signer = connect(addr, &id, "signer").await.unwrap();
    joined(&mut signer).await;
    joined(&mut requester).await;

    let hello = r#"{"v":1,"t":"hello","role":"signer","pk":"x","nonce":"y"} ✓"#;
    signer.send(Message::text(hello)).await.unwrap();
    assert_eq!(next(&mut requester).await, Message::text(hello));
    let sealed: Vec<u8> = (0..=255u8).cycle().take(4096).collect();
    requester
        .send(Message::binary(sealed.clone()))
        .await
        .unwrap();
    assert_eq!(next(&mut signer).await, Message::binary(sealed));
    let full = Bytes::from(vec![7u8; MAX_FRAME_BYTES]);
    signer.send(Message::Binary(full.clone())).await.unwrap();
    assert_eq!(next(&mut requester).await, Message::Binary(full));

    // A third connection in a taken role is closed 4409; the incumbent stays.
    let mut third = connect(addr, &id, "signer").await.unwrap();
    closed_with(&mut third, 4409).await;
    quiet(&mut requester).await;
    signer.send(Message::text("still here")).await.unwrap();
    assert_eq!(next(&mut requester).await, Message::text("still here"));
}

#[tokio::test]
async fn an_oversize_frame_closes_its_sender_1009_and_the_peer_hears_left() {
    let addr = start().await;
    let id = room(3);
    let mut requester = connect(addr, &id, "requester").await.unwrap();
    let mut signer = connect(addr, &id, "signer").await.unwrap();
    joined(&mut signer).await;
    joined(&mut requester).await;
    signer
        .send(Message::binary(vec![0u8; MAX_FRAME_BYTES + 1]))
        .await
        .unwrap();
    closed_with(&mut signer, 1009).await;
    assert_eq!(
        next(&mut requester).await,
        Message::text(Control::Left.text())
    );
}

#[tokio::test]
async fn a_leave_is_announced_and_the_role_can_reconnect() {
    let addr = start().await;
    let id = room(4);
    let mut requester = connect(addr, &id, "requester").await.unwrap();
    let mut signer = connect(addr, &id, "signer").await.unwrap();
    joined(&mut signer).await;
    joined(&mut requester).await;
    signer.close(None).await.unwrap();
    assert_eq!(
        next(&mut requester).await,
        Message::text(Control::Left.text())
    );

    let mut again = connect(addr, &id, "signer").await.unwrap();
    joined(&mut again).await;
    joined(&mut requester).await;
    again.send(Message::text("back")).await.unwrap();
    assert_eq!(next(&mut requester).await, Message::text("back"));
}
