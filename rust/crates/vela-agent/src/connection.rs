//! The bridge connection's own thread.
//!
//! `clientmcp-client` is async on tokio, and the shells run on their own
//! executors (gpui, the main actor, the main looper). So the connection gets
//! one thread, alive for the process, with a current-thread runtime blocking
//! on nothing: the client's tasks run there, and the tools' handlers hand each
//! call to whatever answers it (`tools::Forward`).

use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::{Arc, OnceLock};

use clientmcp_client::{Client, Close, Options, Status};

/// Opens the TCP connection to the bridge's host and port — through the
/// shell's proxy route, say. Blocking work; run on the runtime's blocking pool.
pub type Dial = Arc<dyn Fn(String, u16) -> io::Result<std::net::TcpStream> + Send + Sync>;

/// The runtime's handle, or `None` if it could not be built.
#[must_use]
pub fn handle() -> Option<tokio::runtime::Handle> {
    static HANDLE: OnceLock<Option<tokio::runtime::Handle>> = OnceLock::new();
    HANDLE
        .get_or_init(|| {
            let (tx, rx) = std::sync::mpsc::channel();
            let spawned = std::thread::Builder::new()
                .name("vela-agent".into())
                .spawn(move || {
                    match tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                    {
                        Ok(runtime) => {
                            let _ = tx.send(Some(runtime.handle().clone()));
                            runtime.block_on(std::future::pending::<()>());
                        }
                        Err(_) => {
                            let _ = tx.send(None);
                        }
                    }
                });
            spawned.ok().and_then(|_| rx.recv().ok().flatten())
        })
        .clone()
}

fn dialled(
    dial: Dial,
    host: String,
    port: u16,
) -> Pin<Box<dyn Future<Output = io::Result<tokio::net::TcpStream>> + Send>> {
    Box::pin(async move {
        let stream = tokio::task::spawn_blocking(move || dial(host, port))
            .await
            .map_err(io::Error::other)??;
        stream.set_nonblocking(true)?;
        tokio::net::TcpStream::from_std(stream)
    })
}

/// One connection to a bridge, until [`Connection::close`].
pub struct Connection {
    client: Client,
}

impl Connection {
    /// Dials `url` with `token`, with the tools `expose` puts on the client,
    /// and keeps reconnecting. `on_status` hears every change, on the
    /// connection's thread. `dial`, when given, opens the socket (a proxy
    /// route); otherwise a direct connection. `None` when this process has no
    /// runtime to run it on.
    pub fn open(
        url: &str,
        token: &str,
        instructions: &str,
        on_status: impl Fn(Status, Option<&Close>) + Send + Sync + 'static,
        dial: Option<Dial>,
        expose: impl FnOnce(&Client),
    ) -> Option<Connection> {
        let handle = handle()?;
        let mut options = Options::new(url, token)
            .instructions(instructions)
            .on_status(on_status);
        if let Some(dial) = dial {
            options = options.dial(move |host, port| dialled(Arc::clone(&dial), host, port));
        }
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
