//! `vela-relay-server` — the Clear Signer relay as a native server.
//!
//! Listens on `0.0.0.0:$PORT` (default 8787), plain HTTP: TLS (`wss://`) is the
//! job of whatever sits in front of it.

use std::net::SocketAddr;
use std::process::ExitCode;

use tokio::net::TcpListener;

const DEFAULT_PORT: u16 = 8787;

#[tokio::main]
async fn main() -> ExitCode {
    let port = match std::env::var("PORT") {
        Err(_) => DEFAULT_PORT,
        Ok(value) => match value.parse::<u16>() {
            Ok(port) => port,
            Err(_) => {
                eprintln!("vela-relay: PORT must be a port number, got {value:?}");
                return ExitCode::from(2);
            }
        },
    };
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = match TcpListener::bind(addr).await {
        Ok(listener) => listener,
        Err(err) => {
            eprintln!("vela-relay: cannot listen on {addr}: {err}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("vela-relay: {} listening on {addr}", vela_relay::PROTOCOL);
    tokio::select! {
        () = vela_relay_server::serve(listener) => {}
        () = shutdown() => eprintln!("vela-relay: shutting down"),
    }
    ExitCode::SUCCESS
}

/// Ctrl-C, or SIGTERM from `docker stop` — the binary is PID 1 in its image, and
/// PID 1 gets no default SIGTERM handler.
async fn shutdown() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut term) => {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    _ = term.recv() => {}
                }
            }
            Err(_) => {
                let _ = tokio::signal::ctrl_c().await;
            }
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
