//! The Clear Signer relay's rules — `vela-relay/1`, with no I/O.
//!
//! The relay pairs two WebSockets in a room — one `requester` (the wallet), one
//! `signer` (the page) — and forwards their frames verbatim. It is blind: the two
//! ends run their own ECDH + HKDF + AES-GCM session inside the room, so nothing the
//! relay carries after the hellos is readable to it
//! (`specs/075-clear-signer-channel/contracts/relay.md`).
//!
//! Two hosts run these rules: `vela-relay-server` (native, tokio, Docker) and
//! `vela-relay-worker` (a Cloudflare Worker, one Durable Object per room). Everything
//! they must agree on lives here so they cannot drift apart:
//!
//! - the URL rules — [`target`], [`is_valid_room_id`], [`Role`];
//! - the room state machine — [`Room`], whose event methods return the [`Action`]s a
//!   host carries out;
//! - the limits and close codes — [`MAX_FRAME_BYTES`], [`ROOM_LIFETIME_MS`],
//!   [`IDLE_TIMEOUT_MS`], [`PING_INTERVAL_MS`], [`Close`];
//! - the relay's own frames — [`Control`].
//!
//! Time is injected: every event takes `now` in milliseconds on whatever clock the
//! host has (a monotonic one natively, `Date.now()` in the Worker). That is also what
//! lets the ten-minute and two-minute limits be tested without waiting.

#![forbid(unsafe_code)]

mod room;
mod route;

pub use room::{Action, ConnId, Occupant, Room};
pub use route::{is_valid_room_id, target, Role, Target, HEALTHZ_PATH, ROOMS_PREFIX, ROOM_ID_LEN};

/// The protocol label: the relay's path version and the ends' HKDF/AAD label.
pub const PROTOCOL: &str = "vela-relay/1";

/// Milliseconds on the host's clock.
pub type Millis = u64;

/// The largest frame the relay forwards, in bytes (256 KiB). A frame over it closes
/// its sender with [`Close::TooBig`].
pub const MAX_FRAME_BYTES: usize = 256 * 1024;

/// How long a room lives from its first connection (10 minutes). At that point
/// both ends are closed with [`Close::Expired`].
pub const ROOM_LIFETIME_MS: Millis = 10 * 60 * 1000;

/// How long an end may go without sending a frame (120 s) before it is closed
/// with [`Close::Idle`]. Only the end's own text and binary frames count — pongs
/// are answered by the end's WebSocket stack, not by the end, so counting them
/// would make the limit unenforceable wherever the host pings.
pub const IDLE_TIMEOUT_MS: Millis = 120 * 1000;

/// How often a host pings each end, where its platform lets it (30 s). The
/// Worker's hibernation API answers pings itself and cannot send them.
pub const PING_INTERVAL_MS: Millis = 30 * 1000;

/// Close code: normal closure.
pub const CLOSE_NORMAL: u16 = 1000;
/// Close code: a frame over [`MAX_FRAME_BYTES`].
pub const CLOSE_TOO_BIG: u16 = 1009;
/// Close code: a bad room id or role noticed after the upgrade.
pub const CLOSE_BAD_REQUEST: u16 = 4400;
/// Close code: the room expired, or the end went idle.
pub const CLOSE_EXPIRED: u16 = 4408;
/// Close code: the role is already taken in this room.
pub const CLOSE_ROLE_TAKEN: u16 = 4409;

/// Why the relay closes an end: a close code plus its reason text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Close {
    /// 1000.
    Normal,
    /// 1009 — a frame over [`MAX_FRAME_BYTES`].
    TooBig,
    /// 4400 — a bad room id or role, for a host that can only refuse after the upgrade.
    BadRequest,
    /// 4408 `expired` — the room reached [`ROOM_LIFETIME_MS`].
    Expired,
    /// 4408 `idle` — the end sent nothing for [`IDLE_TIMEOUT_MS`].
    Idle,
    /// 4409 `role taken` — a second connection in a role the room already holds.
    RoleTaken,
}

impl Close {
    /// The WebSocket close code.
    pub const fn code(self) -> u16 {
        match self {
            Close::Normal => CLOSE_NORMAL,
            Close::TooBig => CLOSE_TOO_BIG,
            Close::BadRequest => CLOSE_BAD_REQUEST,
            Close::Expired | Close::Idle => CLOSE_EXPIRED,
            Close::RoleTaken => CLOSE_ROLE_TAKEN,
        }
    }

    /// The close frame's reason text (ASCII, well under the 123-byte limit).
    pub const fn reason(self) -> &'static str {
        match self {
            Close::Normal => "",
            Close::TooBig => "too big",
            Close::BadRequest => "bad request",
            Close::Expired => "expired",
            Close::Idle => "idle",
            Close::RoleTaken => "role taken",
        }
    }
}

/// The relay's own frames. They are text frames whose JSON has a `"relay"` key;
/// an end's own JSON never has one, so an end can tell them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Control {
    /// Both roles are present: the ends may start their handshake.
    Joined,
    /// The other end left; the room waits for a reconnect in its role.
    Left,
}

impl Control {
    /// The exact text frame the relay sends.
    pub const fn text(self) -> &'static str {
        match self {
            Control::Joined => r#"{"v":1,"relay":"joined"}"#,
            Control::Left => r#"{"v":1,"relay":"left"}"#,
        }
    }

    /// Recognises a relay frame by its exact text — the relay only ever sends
    /// these two byte strings.
    pub fn from_text(text: &str) -> Option<Self> {
        [Control::Joined, Control::Left]
            .into_iter()
            .find(|c| c.text() == text)
    }
}

/// Whether a frame of `len` bytes is within [`MAX_FRAME_BYTES`]. A host whose
/// WebSocket library needs a size cap up front can use this and the constant; the
/// verdict that closes an end is [`Room::on_frame`]'s.
pub const fn frame_fits(len: usize) -> bool {
    len <= MAX_FRAME_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_the_contract_numbers() {
        assert_eq!(MAX_FRAME_BYTES, 262_144);
        assert_eq!(ROOM_LIFETIME_MS, 600_000);
        assert_eq!(IDLE_TIMEOUT_MS, 120_000);
        assert_eq!(PING_INTERVAL_MS, 30_000);
        assert_eq!(PROTOCOL, "vela-relay/1");
    }

    #[test]
    fn close_codes_and_reasons() {
        let table = [
            (Close::Normal, 1000, ""),
            (Close::TooBig, 1009, "too big"),
            (Close::BadRequest, 4400, "bad request"),
            (Close::Expired, 4408, "expired"),
            (Close::Idle, 4408, "idle"),
            (Close::RoleTaken, 4409, "role taken"),
        ];
        for (close, code, reason) in table {
            assert_eq!(close.code(), code, "{close:?}");
            assert_eq!(close.reason(), reason, "{close:?}");
            // A close frame's reason must fit in 123 bytes.
            assert!(close.reason().len() <= 123);
        }
    }

    #[test]
    fn control_frames_are_exact_bytes() {
        assert_eq!(Control::Joined.text(), "{\"v\":1,\"relay\":\"joined\"}");
        assert_eq!(Control::Left.text(), "{\"v\":1,\"relay\":\"left\"}");
    }

    #[test]
    fn control_frames_are_recognised_only_verbatim() {
        assert_eq!(
            Control::from_text(Control::Joined.text()),
            Some(Control::Joined)
        );
        assert_eq!(
            Control::from_text(Control::Left.text()),
            Some(Control::Left)
        );
        for other in [
            "",
            "{\"v\":1,\"t\":\"hello\",\"role\":\"signer\"}",
            "{ \"v\":1,\"relay\":\"joined\"}",
            "{\"v\":1,\"relay\":\"gone\"}",
            "{\"v\":2,\"relay\":\"joined\"}",
        ] {
            assert_eq!(Control::from_text(other), None, "{other}");
        }
    }

    #[test]
    fn frame_fits_up_to_and_including_the_limit() {
        assert!(frame_fits(0));
        assert!(frame_fits(MAX_FRAME_BYTES));
        assert!(!frame_fits(MAX_FRAME_BYTES + 1));
    }
}
