//! The caBLE / hybrid transport — "sign in with your phone" — and nothing else.
//!
//! Everything about the PROTOCOL — the QR payload, the BLE advert decrypt, the
//! Noise handshake, the CTAP-over-Noise framing — lives in [`vela_core::cable`],
//! where the other clients reach it too. What lives here is the part made of
//! platform: scanning the Bluetooth radio for the responder's proximity advert,
//! and opening the WebSocket tunnel. A connected [`WebSocketCablePort`] is a
//! [`CablePort`]; wrapping it with [`CableInitiator::establish`] yields a
//! [`vela_core::ctap::ceremony::Cable`] the shared ceremony drives, exactly like
//! the USB cable.
//!
//! ## Why the WebSocket is blocking and the scan is async
//!
//! This shell runs a ceremony on a blocking background thread (see `usb.rs`), so
//! the tunnel is a blocking [`tungstenite`] socket — no runtime for the socket
//! itself. The BLE scan is one-shot and `btleplug` is async-only, so a small
//! current-thread [`tokio`] runtime drives JUST the scan: the advert bytes come
//! back and the runtime is done, before the blocking socket is ever opened.

use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use btleplug::api::{Central, CentralEvent, Manager as _, ScanFilter};
use btleplug::platform::Manager;
use futures::StreamExt;
use tungstenite::client::IntoClientRequest;
use tungstenite::http::HeaderValue;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use uuid::Uuid;

use vela_core::cable::conn::CablePort;
use vela_core::cable::crypto as cable_crypto;
use vela_core::cable::session::CableInitiator;
use vela_core::ctap::ceremony::{Cable, TouchAnnouncer};
use vela_core::ctap::hid_cable::PortError;

#[cfg(target_os = "macos")]
mod l2cap;
#[cfg(target_os = "linux")]
mod l2cap_linux;

/// What can go wrong standing a hybrid connection up, BEFORE the `Cable` exists.
/// Once the handshake runs, failures are the core's `CableError`.
#[derive(Debug)]
pub enum HybridError {
    /// The scan window elapsed with no matching advert — the phone never showed,
    /// or its Bluetooth is off, or it scanned a different QR.
    NoAdvert,
    /// The Bluetooth adapter itself is missing or unavailable.
    Bluetooth(String),
    /// The advert decrypted but is malformed, or names an unknown tunnel domain.
    BadAdvert,
    /// The WebSocket tunnel would not open (connect, TLS, or handshake).
    Tunnel(String),
    /// The Noise handshake over the tunnel failed.
    Handshake(String),
}

impl std::fmt::Display for HybridError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdvert => write!(f, "no phone answered the QR within the scan window"),
            Self::Bluetooth(detail) => write!(f, "Bluetooth is unavailable: {detail}"),
            Self::BadAdvert => write!(f, "the phone's advertisement was malformed"),
            Self::Tunnel(detail) => write!(f, "the relay tunnel would not open: {detail}"),
            Self::Handshake(detail) => write!(f, "the encrypted channel failed: {detail}"),
        }
    }
}

/// The two 16-bit BLE service-data UUIDs a caBLE responder advertises under
/// (CTAP 2.2 used `0xFFF9`; 2.3 moved to `0xFDE2`). A scanner watches both.
const SERVICE_UUIDS: [Uuid; 2] = [
    Uuid::from_u128(0x0000_fff9_0000_1000_8000_00805f9b34fb),
    Uuid::from_u128(0x0000_fde2_0000_1000_8000_00805f9b34fb),
];

/// Can this desktop connect a Bluetooth LE L2CAP connection-oriented channel —
/// the CTAP 2.3 local data channel, which needs no tunnel server?
///
/// macOS can, through `CBL2CAPChannel` (see [`l2cap`]); Linux can, through a
/// BlueZ `AF_BLUETOOTH`/`BTPROTO_L2CAP` socket (see [`l2cap_linux`]). Windows
/// cannot — and the gap is the OS's, verified, not this module's laziness:
/// user mode gets RFCOMM sockets (classic Bluetooth) and GATT (WinRT), but LE
/// CoC — the exact channel CTAP 2.3 rides on — has no user-mode surface; it
/// takes a KMDF kernel profile driver speaking the L2CAP DDI. Even Microsoft's
/// own cross-device passkey flow requires internet on both devices (tunnel
/// data, BLE for proximity only). The one real route — our own HCI/L2CAP stack
/// on a WinUSB-attached dongle — is the Phase-11 self-owned-stack project, not
/// a patch.
///
/// It is read twice, and both readings matter. The QR payload uses it to decide
/// whether to OFFER the BLE channel: offering one that cannot be connected
/// invites an authenticator to pick it and strands the ceremony. And
/// [`establish_hybrid`] uses it as the guard on the L2CAP branch, so an
/// authenticator that advertises a PSM anyway lands on the tunnel rather than
/// on an error.
pub const BLE_CHANNEL_SUPPORTED: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// The WebSocket subprotocol the tunnel server requires.
const CABLE_SUBPROTOCOL: &str = "fido.cable";

/// How long to scan before giving up. A person has to pick up their phone,
/// unlock it, and approve the prompt that scanning the QR raised.
const SCAN_TIMEOUT: Duration = Duration::from_secs(90);

/// A read blocks this long before the tunnel is declared dead — long enough to
/// cover the person approving on their phone (the CTAP user-presence budget and
/// then some), short enough that a dropped tunnel does not hang forever.
const TUNNEL_READ_TIMEOUT: Duration = Duration::from_secs(130);

/// How long to wait for the TCP connection to the tunnel server. Bounded so an
/// unreachable tunnel — e.g. a network that cannot reach that server at all —
/// fails cleanly instead of hanging the sign-in forever. (This is exactly the
/// failure an Android phone hits when its assigned tunnel is Google's
/// `cable.ua5v.com` and the network cannot reach it — the BLE-only channel is
/// the answer there, not a longer wait.)
const TUNNEL_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The CTAP 2.2 WebSocket tunnel, as the core's [`CablePort`]: one binary frame
/// per message.
pub struct WebSocketCablePort {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
    /// The phone (or the relay) sent a CLOSE. Every later write is refused
    /// here rather than pushed into a socket that is already gone — the
    /// goodbye frame the client writes on its way out was landing exactly
    /// there (spec 038 finding 18).
    closed_by_peer: bool,
}

/// The one sentence a closed tunnel produces, shared with the ceremony's
/// failure mapping so the sheet can name what happened without matching prose.
pub const TUNNEL_CLOSED: &str = "the tunnel closed";

impl WebSocketCablePort {
    /// Open the tunnel at `url` (`wss://…/cable/connect/<routing>/<tunnel>`),
    /// negotiating the `fido.cable` subprotocol, with a bounded TCP-connect wait.
    pub fn connect(url: &str) -> Result<Self, HybridError> {
        let mut request = url
            .into_client_request()
            .map_err(|error| HybridError::Tunnel(error.to_string()))?;
        request.headers_mut().insert(
            "Sec-WebSocket-Protocol",
            HeaderValue::from_static(CABLE_SUBPROTOCOL),
        );

        // Connect the TCP socket ourselves — `tungstenite::connect` has no
        // deadline and, worse, dials a RAW socket that ignores the app's proxy.
        // The tunnel server may be reachable only through that proxy (Google's
        // cable.ua5v.com behind the GFW), so it is dialed through the proxy when
        // one is configured, resolving the host at the proxy. Then the connected
        // socket goes to `client_tls` for the TLS + WebSocket handshakes.
        let host = url
            .strip_prefix("wss://")
            .and_then(|rest| rest.split('/').next())
            .filter(|host| !host.is_empty())
            .ok_or_else(|| HybridError::Tunnel(format!("not a wss:// URL: {url}")))?;

        let stream = match resolve_proxy() {
            Some(proxy) => {
                log(&format!("dialing {host}:443 through {proxy}"));
                proxy.connect(host, 443)?
            }
            None => {
                log(&format!(
                    "dialing {host}:443 directly (no proxy configured)"
                ));
                let address = (host, 443u16)
                    .to_socket_addrs()
                    .map_err(|error| {
                        HybridError::Tunnel(format!("cannot resolve {host}: {error}"))
                    })?
                    .next()
                    .ok_or_else(|| HybridError::Tunnel(format!("no address for {host}")))?;
                TcpStream::connect_timeout(&address, TUNNEL_CONNECT_TIMEOUT)
                    .map_err(|error| HybridError::Tunnel(format!("cannot reach {host}: {error}")))?
            }
        };
        // A bounded read so a phone that never answers frees the thread instead
        // of blocking it until the process exits.
        let _ = stream.set_read_timeout(Some(TUNNEL_READ_TIMEOUT));

        log("socket connected; TLS + WebSocket handshake…");
        let (ws, _response) = tungstenite::client_tls(request, stream)
            .map_err(|error| HybridError::Tunnel(error.to_string()))?;
        log("WebSocket tunnel established (fido.cable)");

        Ok(Self {
            ws,
            closed_by_peer: false,
        })
    }
}

impl CablePort for WebSocketCablePort {
    fn write_frame(&mut self, frame: &[u8]) -> Result<(), PortError> {
        if self.closed_by_peer {
            log(&format!(
                "→ tunnel frame ({} bytes) refused: peer closed",
                frame.len()
            ));
            return Err(PortError::Io(TUNNEL_CLOSED.to_owned()));
        }
        log(&format!("→ tunnel frame ({} bytes)", frame.len()));
        self.ws
            .send(Message::Binary(frame.to_vec().into()))
            .map_err(|error| PortError::Io(error.to_string()))
    }

    fn read_frame(&mut self) -> Result<Vec<u8>, PortError> {
        loop {
            match self.ws.read() {
                Ok(Message::Binary(bytes)) => {
                    log(&format!("← tunnel frame ({} bytes)", bytes.len()));
                    return Ok(bytes.to_vec());
                }
                Ok(Message::Close(frame)) => {
                    log(&format!("← tunnel CLOSE {frame:?}"));
                    self.closed_by_peer = true;
                    return Err(PortError::Io(TUNNEL_CLOSED.to_owned()));
                }
                // The relay keeps the pair alive with pings and kills BOTH legs
                // when one stops answering — and the phone-side selector can
                // hold the tunnel idle for a long time. tungstenite queues the
                // pong automatically but only writes it on the NEXT read/write,
                // so it is flushed here, immediately, and logged so a keepalive
                // failure is visible instead of a mystery ~30s hangup.
                Ok(Message::Ping(payload)) => {
                    match self.ws.flush() {
                        Ok(()) => log(&format!("← ping ({} bytes) → pong flushed", payload.len())),
                        Err(error) => log(&format!("← ping, but pong flush FAILED: {error}")),
                    }
                    continue;
                }
                Ok(Message::Pong(_)) => {
                    log("← pong");
                    continue;
                }
                // Text frames are tunnel housekeeping, not caBLE frames.
                Ok(_) => continue,
                Err(error) => {
                    log(&format!(
                        "← tunnel read error (peer/relay dropped?): {error}"
                    ));
                    return Err(PortError::Io(error.to_string()));
                }
            }
        }
    }

    fn channel(&self) -> &str {
        "WebSocket"
    }
}

/// Run the whole hybrid handshake: scan for the phone this QR named, open the
/// tunnel it points at, and run the Noise handshake — returning the [`Cable`]
/// the ceremony drives.
///
/// [`Cable`]: vela_core::ctap::ceremony::Cable
///
/// The QR must already be on screen (the caller shows [`CableInitiator::qr_payload`]);
/// scanning it is what makes the phone start advertising, so nothing here can
/// happen until it is.
pub fn establish_hybrid(
    session: &CableInitiator,
    product: String,
    ephemeral_seed: &[u8],
    on_touch: Option<TouchAnnouncer>,
) -> Result<Box<dyn Cable>, HybridError> {
    // 1. Find the authenticator by its BLE proximity advert.
    log("scanning for the phone's Bluetooth advert…");
    let hit = scan_for_advert(session.eid_key())?;
    log("advert found; decrypted");

    // 2. The advert chooses the channel: a PSM means the CTAP 2.3 local BLE
    //    channel (direct L2CAP, no tunnel — the GFW-proof path); no PSM means
    //    the CTAP 2.2 WebSocket tunnel.
    #[cfg(target_os = "macos")]
    if let Some(psm) = hit.psm {
        log(&format!(
            "advert offers the BLE channel (PSM {psm}); connecting L2CAP CoC — no tunnel"
        ));
        let port = l2cap::L2capCablePort::connect(&hit.peripheral, psm)?;
        log("L2CAP channel open; starting Noise handshake");
        let cable = session
            .establish(port, &hit.plaintext, ephemeral_seed, product, on_touch)
            .map_err(|error| HybridError::Handshake(format!("{error:?}")))?;
        log("handshake complete; channel is up (BLE)");
        return Ok(Box::new(cable));
    }

    #[cfg(target_os = "linux")]
    if let Some(psm) = hit.psm {
        match hit.address {
            Some((address, random)) => {
                log(&format!(
                    "advert offers the BLE channel (PSM {psm}); connecting L2CAP CoC — no tunnel"
                ));
                // A refused or timed-out CoC is not the end of the ceremony: the
                // advert plaintext still carries the routing id and tunnel
                // domain, so a dual-channel phone is reachable on the tunnel
                // even though it named a PSM. Only a BLE-ONLY authenticator is
                // genuinely out of reach here, and it will simply not be at the
                // other end of the tunnel — an honest failure one step later,
                // rather than one that also strands every phone that offered
                // both. Bounded by `CONNECT_TIMEOUT`, so the fall-through is
                // reached in seconds rather than on the kernel's own schedule.
                match l2cap_linux::L2capCablePort::connect(address, random, psm) {
                    Ok(port) => {
                        log("L2CAP channel open; starting Noise handshake");
                        let cable = session
                            .establish(port, &hit.plaintext, ephemeral_seed, product, on_touch)
                            .map_err(|error| HybridError::Handshake(format!("{error:?}")))?;
                        log("handshake complete; channel is up (BLE)");
                        return Ok(Box::new(cable));
                    }
                    Err(error) => log(&format!(
                        "the BLE channel would not open ({error}); using the tunnel instead"
                    )),
                }
            }
            // The scan matched but BlueZ would not say the device's LE address —
            // fall through to the tunnel, which needs no addressing.
            None => log(&format!(
                "advert offers the BLE channel (PSM {psm}) but the device's LE address \
                 is unavailable; using the tunnel instead"
            )),
        }
    }

    // A PSM on a desktop with no L2CAP is not a dead end: the 16-byte advert
    // plaintext always carries the routing id and tunnel domain, so the CTAP
    // 2.2 tunnel is still derivable and an authenticator that offers BOTH
    // channels will meet us there. Only a BLE-ONLY authenticator is genuinely
    // out of reach — and it should never have got this far, because the QR
    // this desktop showed did not offer the BLE channel (`BLE_CHANNEL_SUPPORTED`
    // gates `qr_payload`), so it would have declined the QR outright. Falling
    // through beats the hard error this used to raise, which also caught every
    // dual-channel phone.
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    if let Some(psm) = hit.psm {
        log(&format!(
            "advert offers the BLE channel (PSM {psm}), but this desktop has no \
             L2CAP CoC; using the tunnel instead"
        ));
    }

    let url = session
        .connect_url(&hit.plaintext)
        .ok_or(HybridError::BadAdvert)?;
    log(&format!("opening tunnel: {url}"));
    let port = WebSocketCablePort::connect(&url)?;
    log("tunnel open; starting Noise handshake");
    let cable = session
        .establish(port, &hit.plaintext, ephemeral_seed, product, on_touch)
        .map_err(|error| HybridError::Handshake(format!("{error:?}")))?;
    log("handshake complete; channel is up (WebSocket)");
    Ok(Box::new(cable))
}

/// One diagnostics line to stderr. The hybrid path has no `Host` in reach, and
/// its failures happen off-screen (a scan, a socket) where the login machine's
/// one generic "sign-in failed" body cannot say what broke — so the detail goes
/// here, visible when the app is launched from a terminal or Console.app.
fn log(line: &str) {
    eprintln!("[vela-cable] {line}");
}

/// Lowercase hex for a byte slice, for the diagnostic logs.
fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Proxy — the tunnel is reached through the same proxy the rest of the app uses
// ---------------------------------------------------------------------------

/// A configured forward proxy to dial the tunnel through.
struct ProxyEndpoint {
    socks: bool,
    host: String,
    port: u16,
}

impl std::fmt::Display for ProxyEndpoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}://{}:{}",
            if self.socks { "socks5" } else { "http" },
            self.host,
            self.port
        )
    }
}

impl ProxyEndpoint {
    /// Open a TCP connection to `host:port` THROUGH the proxy, resolving `host`
    /// at the proxy (so a name the local resolver cannot reach still connects).
    fn connect(&self, host: &str, port: u16) -> Result<TcpStream, HybridError> {
        if self.socks {
            socks5_connect(&self.host, self.port, host, port)
        } else {
            http_connect(&self.host, self.port, host, port)
        }
    }
}

/// A SOCKS5 CONNECT tunnel to `host:port` through the proxy, with the host
/// resolved AT the proxy (socks5h). Hand-rolled rather than via a crate so every
/// step carries a timeout — a proxy that stalls must fail, not hang the sign-in
/// — and so the proxy's own reply code becomes a sentence a person can act on.
fn socks5_connect(
    proxy_host: &str,
    proxy_port: u16,
    host: &str,
    port: u16,
) -> Result<TcpStream, HybridError> {
    use std::io::{Read, Write};

    let host_bytes = host.as_bytes();
    if host_bytes.len() > 255 {
        return Err(HybridError::Tunnel(
            "tunnel host name too long for SOCKS5".to_owned(),
        ));
    }

    let address = (proxy_host, proxy_port)
        .to_socket_addrs()
        .map_err(|error| {
            HybridError::Tunnel(format!("cannot resolve proxy {proxy_host}: {error}"))
        })?
        .next()
        .ok_or_else(|| HybridError::Tunnel(format!("no address for proxy {proxy_host}")))?;
    let mut stream =
        TcpStream::connect_timeout(&address, TUNNEL_CONNECT_TIMEOUT).map_err(|error| {
            HybridError::Tunnel(format!("cannot reach proxy {proxy_host}: {error}"))
        })?;
    // The handshake must not outlast the connect budget; the long read timeout is
    // restored once the tunnel is up (for the person approving on their phone).
    let _ = stream.set_read_timeout(Some(TUNNEL_CONNECT_TIMEOUT));
    let _ = stream.set_write_timeout(Some(TUNNEL_CONNECT_TIMEOUT));

    let socks_err = |detail: String| HybridError::Tunnel(format!("SOCKS5 proxy: {detail}"));

    // Greeting: version 5, one method, "no authentication".
    stream
        .write_all(&[0x05, 0x01, 0x00])
        .map_err(|error| socks_err(format!("greeting failed: {error}")))?;
    let mut method = [0u8; 2];
    stream
        .read_exact(&mut method)
        .map_err(|error| socks_err(format!("no greeting reply: {error}")))?;
    if method[0] != 0x05 || method[1] != 0x00 {
        return Err(socks_err(format!(
            "proxy would not accept unauthenticated SOCKS5 (reply {method:02x?})"
        )));
    }

    // CONNECT to the domain (ATYP 3), the proxy resolving it.
    let mut request = vec![0x05, 0x01, 0x00, 0x03];
    #[allow(clippy::cast_possible_truncation)]
    request.push(host_bytes.len() as u8);
    request.extend_from_slice(host_bytes);
    request.extend_from_slice(&port.to_be_bytes());
    stream
        .write_all(&request)
        .map_err(|error| socks_err(format!("CONNECT failed: {error}")))?;

    let mut head = [0u8; 4];
    stream
        .read_exact(&mut head)
        .map_err(|error| socks_err(format!("no CONNECT reply: {error}")))?;
    if head[0] != 0x05 {
        return Err(socks_err(format!("bad reply version {:#x}", head[0])));
    }
    if head[1] != 0x00 {
        return Err(socks_err(format!(
            "the proxy could not reach {host}:{port} ({})",
            socks_reply_reason(head[1])
        )));
    }
    // Drain the bound address the reply carries, per its address type.
    match head[3] {
        0x01 => drain(&mut stream, 4 + 2).map_err(socks_err)?,
        0x04 => drain(&mut stream, 16 + 2).map_err(socks_err)?,
        0x03 => {
            let mut len = [0u8; 1];
            stream
                .read_exact(&mut len)
                .map_err(|error| socks_err(format!("truncated reply: {error}")))?;
            drain(&mut stream, len[0] as usize + 2).map_err(socks_err)?;
        }
        other => return Err(socks_err(format!("unknown reply address type {other:#x}"))),
    }

    let _ = stream.set_read_timeout(Some(TUNNEL_READ_TIMEOUT));
    Ok(stream)
}

/// Read and discard `n` bytes (a SOCKS reply's bound address).
fn drain(stream: &mut TcpStream, n: usize) -> Result<(), String> {
    use std::io::Read;
    let mut buffer = vec![0u8; n];
    stream
        .read_exact(&mut buffer)
        .map_err(|error| format!("truncated reply: {error}"))
}

/// A SOCKS5 reply code as a short reason.
fn socks_reply_reason(code: u8) -> &'static str {
    match code {
        0x01 => "general proxy failure",
        0x02 => "connection not allowed by the proxy",
        0x03 => "network unreachable",
        0x04 => "host unreachable",
        0x05 => "connection refused",
        0x06 => "TTL expired",
        0x07 => "command not supported",
        0x08 => "address type not supported",
        _ => "unknown SOCKS error",
    }
}

/// An HTTP `CONNECT` tunnel through a forward proxy.
fn http_connect(
    proxy_host: &str,
    proxy_port: u16,
    host: &str,
    port: u16,
) -> Result<TcpStream, HybridError> {
    use std::io::{Read, Write};

    let address = (proxy_host, proxy_port)
        .to_socket_addrs()
        .map_err(|error| {
            HybridError::Tunnel(format!("cannot resolve proxy {proxy_host}: {error}"))
        })?
        .next()
        .ok_or_else(|| HybridError::Tunnel(format!("no address for proxy {proxy_host}")))?;
    let mut stream =
        TcpStream::connect_timeout(&address, TUNNEL_CONNECT_TIMEOUT).map_err(|error| {
            HybridError::Tunnel(format!("cannot reach proxy {proxy_host}: {error}"))
        })?;
    stream
        .write_all(
            format!("CONNECT {host}:{port} HTTP/1.1\r\nHost: {host}:{port}\r\nProxy-Connection: keep-alive\r\n\r\n")
                .as_bytes(),
        )
        .map_err(|error| HybridError::Tunnel(format!("proxy CONNECT write failed: {error}")))?;

    let mut header = Vec::new();
    let mut byte = [0u8; 1];
    while !header.ends_with(b"\r\n\r\n") {
        let read = stream
            .read(&mut byte)
            .map_err(|error| HybridError::Tunnel(format!("proxy CONNECT read failed: {error}")))?;
        if read == 0 || header.len() > 8192 {
            return Err(HybridError::Tunnel(
                "proxy closed during CONNECT".to_owned(),
            ));
        }
        header.push(byte[0]);
    }
    let status = String::from_utf8_lossy(&header);
    if !status.starts_with("HTTP/1.1 200") && !status.starts_with("HTTP/1.0 200") {
        let first = status.lines().next().unwrap_or("").to_owned();
        return Err(HybridError::Tunnel(format!(
            "proxy refused CONNECT: {first}"
        )));
    }
    Ok(stream)
}

/// The forward proxy to dial the tunnel through, or `None` for a direct
/// connection.
///
/// Spec 038: asks `executor::proxy` — the app's ONE proxy decision (system
/// setting → environment → direct, re-derived on failure) — rather than
/// reading the environment and `scutil` on its own, which was a second
/// decision that could disagree with the first. Auth is dropped: the local
/// proxies this meets do not use it, and carrying it wrong is worse than not
/// carrying it.
fn resolve_proxy() -> Option<ProxyEndpoint> {
    let proxy = crate::executor::proxy::system_proxy()?;
    let socks = matches!(
        proxy.protocol(),
        ureq::ProxyProtocol::Socks5
            | ureq::ProxyProtocol::Socks5h
            | ureq::ProxyProtocol::Socks4
            | ureq::ProxyProtocol::Socks4A
    );
    Some(ProxyEndpoint {
        socks,
        host: proxy.host().to_owned(),
        port: proxy.port(),
    })
}

/// Scan the Bluetooth radio for a proximity advert that decrypts under this
/// session's EID key, and return its 16-byte plaintext. Blocks (on a private
/// current-thread runtime) up to [`SCAN_TIMEOUT`].
/// The matched advert: the decrypted 16-byte EID, the L2CAP PSM if the
/// authenticator offered the BLE channel, and the peripheral it came from (as a
/// UUID string, for a CoreBluetooth L2CAP connect).
struct AdvertHit {
    plaintext: [u8; 16],
    psm: Option<u16>,
    /// Read only by the CoreBluetooth L2CAP connect, so on the desktops that
    /// address peripherals differently it is recorded and never looked at. Kept
    /// rather than cfg'd away: the scan that fills it is shared, and a field
    /// that exists everywhere is one less thing for the platform builds to
    /// diverge on.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    peripheral: String,
    /// The peripheral's LE address in btleplug's big-endian display order, and
    /// whether it lives in the RANDOM address namespace — a BlueZ L2CAP socket
    /// addresses the device by exactly this pair, and the right MAC with the
    /// wrong namespace just times out. Linux-only: macOS refuses to expose MACs
    /// (CoreBluetooth speaks UUIDs) and no other desktop connects L2CAP.
    #[cfg(target_os = "linux")]
    address: Option<([u8; 6], bool)>,
}

fn scan_for_advert(eid_key: Vec<u8>) -> Result<AdvertHit, HybridError> {
    // BOTH drivers, and the I/O one is not optional. On Linux `btleplug` reaches
    // `bluetoothd` over D-Bus, and `dbus-tokio` registers that socket with the
    // runtime's I/O driver: without `enable_io` its connection task panics with
    // "A Tokio 1.x context was found, but IO is disabled", and because the panic
    // lands on a spawned task rather than here, the failure SURFACES ~25s later
    // as `org.freedesktop.DBus.Error.Timeout` from the first adapter call — a
    // Bluetooth error that names D-Bus and nothing else. macOS and Windows never
    // noticed: CoreBluetooth answers through a delegate and WinRT through its own
    // async, so neither touches the reactor and a time-only runtime was enough.
    // The scan is the common ancestor of both transports, so on Linux this one
    // missing line took the BLE channel and the WebSocket tunnel down together.
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .enable_time()
        .build()
        .map_err(|error| HybridError::Bluetooth(error.to_string()))?;

    runtime.block_on(async move {
        match tokio::time::timeout(SCAN_TIMEOUT, scan_loop(&eid_key)).await {
            Ok(result) => result,
            Err(_elapsed) => Err(HybridError::NoAdvert),
        }
    })
}

/// The async half of the scan: watch service-data adverts, trial-decrypting the
/// first 20 bytes of each under the two caBLE service UUIDs.
async fn scan_loop(eid_key: &[u8]) -> Result<AdvertHit, HybridError> {
    let manager = Manager::new()
        .await
        .map_err(|error| HybridError::Bluetooth(error.to_string()))?;
    let adapter = manager
        .adapters()
        .await
        .map_err(|error| HybridError::Bluetooth(error.to_string()))?
        .into_iter()
        .next()
        .ok_or_else(|| HybridError::Bluetooth("no Bluetooth adapter".to_owned()))?;
    log("Bluetooth adapter opened");

    let mut events = adapter
        .events()
        .await
        .map_err(|error| HybridError::Bluetooth(error.to_string()))?;
    adapter
        .start_scan(ScanFilter::default())
        .await
        .map_err(|error| HybridError::Bluetooth(error.to_string()))?;
    log("scan started");

    // Distinct caBLE candidates that did NOT match this QR — logged once each,
    // not once per advertisement (a phone re-broadcasts several times a second).
    // Their presence means a caBLE authenticator is advertising for a DIFFERENT
    // QR, which is the tell-tale of a stale scan: a previous attempt still live
    // on the phone, or the wrong QR scanned.
    let mut unmatched: std::collections::HashSet<Vec<u8>> = std::collections::HashSet::new();
    let mut hinted = false;

    while let Some(event) = events.next().await {
        // The caBLE advert is service data under 0xFFF9 / 0xFDE2.
        let CentralEvent::ServiceDataAdvertisement { id, service_data } = event else {
            continue;
        };
        for uuid in SERVICE_UUIDS {
            let Some(data) = service_data.get(&uuid) else {
                continue;
            };
            if data.len() < 20 {
                continue;
            }
            if let Some(plaintext) = cable_crypto::try_decrypt_advert(&data[0..20], eid_key) {
                // Bytes past the first 20 are the CTAP 2.3 BLE suffix — a CBOR
                // map whose key 1 is the L2CAP PSM for the local Bluetooth data
                // channel. Its presence is what says "this authenticator offers
                // the direct BLE channel"; absence means WebSocket-only (GMS).
                let suffix = &data[20..];
                let psm = cable_crypto::parse_advert_psm(suffix);
                log(&format!(
                    "matched this QR on {id}; suffix={} PSM={psm:?}",
                    if suffix.is_empty() {
                        "(none)".to_owned()
                    } else {
                        hex(suffix)
                    }
                ));
                // BlueZ's L2CAP socket needs the device's LE (address,
                // namespace) pair; btleplug has both on the peripheral's
                // properties. Best-effort: a lookup failure downgrades to the
                // tunnel rather than failing the scan that just succeeded.
                #[cfg(target_os = "linux")]
                let address = {
                    use btleplug::api::Peripheral as _;
                    match adapter.peripheral(&id).await {
                        Ok(peripheral) => {
                            peripheral
                                .properties()
                                .await
                                .ok()
                                .flatten()
                                .map(|properties| {
                                    let random = matches!(
                                        properties.address_type,
                                        Some(btleplug::api::AddressType::Random)
                                    );
                                    (properties.address.into_inner(), random)
                                })
                        }
                        Err(_) => None,
                    }
                };
                let _ = adapter.stop_scan().await;
                return Ok(AdvertHit {
                    plaintext,
                    psm,
                    peripheral: id.to_string(),
                    #[cfg(target_os = "linux")]
                    address,
                });
            }
            // A caBLE advert that is not ours. Log each distinct one once.
            if unmatched.insert(data.to_vec()) {
                log(&format!(
                    "caBLE advert for a DIFFERENT QR ({} bytes): {}",
                    data.len(),
                    hex(data)
                ));
                if !hinted {
                    hinted = true;
                    log("→ a phone is advertising for another QR. Make sure it is \
                         scanning the QR currently on screen — dismiss any earlier \
                         sign-in attempt on the phone and scan the fresh code.");
                }
            }
        }
    }

    Err(HybridError::NoAdvert)
}
