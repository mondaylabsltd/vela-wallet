//! The bridge connection's own thread.
//!
//! `clientmcp-client` is async on tokio, and this shell runs on gpui's
//! executors with no tokio reactor of its own (the BLE scan builds one and
//! drops it). So the connection gets one thread, alive for the process, with a
//! current-thread runtime blocking on nothing: the client's tasks run there,
//! and its handlers hand each call over to gpui (`agent::calls`).
//!
//! The socket is dialled the way the rest of the app reaches the network —
//! through the proxy the system names for the bridge (`ctap::cable::dial`, the
//! tunnel's own dialler), resolved at the proxy — and only then handed to the
//! client for TLS and the WebSocket handshake.

use std::io;
use std::sync::OnceLock;

use clientmcp_client::{Client, Close, Options, Status};

/// The runtime's handle, or `None` if it could not be built (logged once).
fn handle() -> Option<tokio::runtime::Handle> {
    static HANDLE: OnceLock<Option<tokio::runtime::Handle>> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("vela-agent".into())
                .spawn(move || {
                    let runtime = tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build();
                    match runtime {
                        Ok(runtime) => {
                            let _ = tx.send(Some(runtime.handle().clone()));
                            runtime.block_on(std::future::pending::<()>());
                        }
                        Err(error) => {
                            crate::diag::vlog!("agent", "no runtime for the bridge: {error}");
                            let _ = tx.send(None);
                        }
                    }
                });
            match spawned {
                Ok(_) => rx.recv().ok().flatten(),
                Err(error) => {
                    crate::diag::vlog!("agent", "no thread for the bridge: {error}");
                    None
                }
            }
        })
        .clone()
}

/// A TCP connection to the bridge's `host:port`, through the system's proxy
/// for it. Blocking work (a PAC lookup, a CONNECT), so on the blocking pool.
async fn dial(host: String, port: u16, secure: bool) -> io::Result<tokio::net::TcpStream> {
    let stream = tokio::task::spawn_blocking(move || {
        let scheme = if secure { "https" } else { "http" };
        crate::ctap::cable::dial(&format!("{scheme}://{host}:{port}/"), &host, port, |line| {
            crate::diag::vlog!("agent", "{line}");
        })
    })
    .await
    .map_err(io::Error::other)?
    .map_err(|error| io::Error::other(error.to_string()))?;
    stream.set_nonblocking(true)?;
    tokio::net::TcpStream::from_std(stream)
}

/// One connection to a bridge, until [`Connection::close`].
pub struct Connection {
    client: Client,
}

impl Connection {
    /// Dials `url` with `token`, with the tools `expose` puts on the client,
    /// and keeps reconnecting. `on_status` hears every change, on the
    /// connection's thread. `None` when this process has no runtime to run
    /// it on.
    pub fn open(
        url: &str,
        token: &str,
        instructions: &str,
        on_status: impl Fn(Status, Option<&Close>) + Send + Sync + 'static,
        expose: impl FnOnce(&Client),
    ) -> Option<Connection> {
        let handle = handle()?;
        let secure = url.trim_start().starts_with("wss:");
        let options = Options::new(url, token)
            .instructions(instructions)
            .on_status(on_status)
            .dial(move |host, port| dial(host, port, secure));
        let client = Client::new(options);
        // Registered with the first connection, not announced as a change.
        expose(&client);
        let connecting = client.clone();
        handle.spawn(async move {
            // The status says how it went; a close for good ends the task.
            let _ = connecting.connect().await;
        });
        Some(Connection { client })
    }

    /// Hangs up for good; calls in flight are cancelled.
    pub fn close(&self) {
        self.client.close();
    }
}
