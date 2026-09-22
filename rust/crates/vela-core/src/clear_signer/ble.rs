//! The BLE channel's framing (`app-web/clearsigning/PROTOCOL.md` §2).
//!
//! A BLE write carries a couple of hundred bytes and a batch intent carries a
//! few thousand, so every message is cut into frames behind a six-byte header:
//!
//! ```text
//! offset  len  meaning
//! 0       1    flags — bit0: 1 = sealed, 0 = plaintext (the handshake only)
//! 1       1    msgId — the same for every frame of one message, wrapping at 256
//! 2       2    seq   — frame number from 0, big-endian
//! 4       2    total — how many frames this message has, big-endian
//! 6       …    payload
//! ```
//!
//! This lives in the core because three shells run the peripheral side — an
//! Android `BluetoothGattServer`, an iOS `CBPeripheralManager`, a macOS one —
//! and the page ([`app-web/clearsigning/lib/transport/ble.js`]) is the central
//! side they all talk to. Three hand-written reassemblers would be three
//! chances to disagree about a wrapped `msgId`, a frame that arrives twice, or
//! a message that never finishes; the page's behaviour is the reference and
//! `tests/clear-signer/ble-frames.json` pins the two against each other.
//!
//! There is no clock here. A shell hands `now_ms` to [`Reassembler::accept`]
//! and calls [`Reassembler::sweep`] to drop what went stale, the same way every
//! other machine in this crate takes time as an argument.
//!
//! The `msgId` is not bookkeeping: it is sealed into the session's AAD
//! ([`super::secure::Tail::MsgId`]), so the id a message is framed under and
//! the id it is sealed under must be the same one — which is why
//! [`Framer::next_id`] hands it out before sealing rather than during framing.

use std::collections::BTreeMap;

/// The GATT service a wallet advertises (`76656c61` is `vela` in ASCII). The
/// advertisement must carry it: Chrome's device chooser filters on it.
pub const SERVICE_UUID: &str = "76656c61-0001-4000-8000-00805f9b34fb";

/// Central → peripheral: `write`, `writeWithoutResponse`.
pub const C2P_UUID: &str = "76656c61-0002-4000-8000-00805f9b34fb";

/// Peripheral → central: `notify`, `read`.
pub const P2C_UUID: &str = "76656c61-0003-4000-8000-00805f9b34fb";

/// The six-byte frame header.
pub const HEADER: usize = 6;

/// Payload bytes per frame, for the ATT MTU (247) every modern phone
/// negotiates. The page starts here too.
pub const DEFAULT_CHUNK: usize = 244;

/// The floor for [`Framer::halve`]: an authenticator on the default MTU of 23
/// still takes this much, and halving past it would only stall.
pub const MIN_CHUNK: usize = 20;

/// How long an incomplete message may wait for its missing frames.
pub const REASSEMBLY_TIMEOUT_MS: u64 = 10_000;

/// Bit 0 of `flags`: this message is sealed (everything after the handshake).
pub const FLAG_SEALED: u8 = 1;

/// A message that arrived whole.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub flags: u8,
    pub msg_id: u8,
    pub payload: Vec<u8>,
}

impl Message {
    /// Was this message sealed, or is it a handshake hello in the clear?
    #[must_use]
    pub fn sealed(&self) -> bool {
        self.flags & FLAG_SEALED != 0
    }
}

/// Cuts messages into frames, and remembers which `msgId` comes next.
#[derive(Clone, Debug)]
pub struct Framer {
    chunk: usize,
    last_id: u8,
}

impl Default for Framer {
    fn default() -> Self {
        Self::new()
    }
}

impl Framer {
    #[must_use]
    pub fn new() -> Self {
        Self {
            chunk: DEFAULT_CHUNK,
            last_id: 0,
        }
    }

    /// Payload bytes per frame, as it stands.
    #[must_use]
    pub fn chunk(&self) -> usize {
        self.chunk
    }

    /// The id the next message gets. Taken BEFORE sealing, because the session
    /// seals it into the AAD and the frames must carry the very same one.
    pub fn next_id(&mut self) -> u8 {
        self.last_id = self.last_id.wrapping_add(1);
        self.last_id
    }

    /// Size the chunk for a negotiated ATT MTU, and say what it became.
    ///
    /// A peripheral learns the MTU when the central subscribes, and this is
    /// the arithmetic every one of them would otherwise write for itself:
    /// `mtu - 3` is what an ATT value may carry (opcode + handle), and
    /// [`HEADER`] of that is ours, so the payload is `mtu - 3 - 6`.
    ///
    /// [`DEFAULT_CHUNK`] is deliberately NOT that number. It is what the page
    /// writes with, and a central's oversized write is split into a long write
    /// by the OS — but a peripheral's **notify** cannot be split: `updateValue`
    /// truncates at the link's maximum, and a truncated frame is a message
    /// that never completes (iOS, spec 075 T041, found this). So a peripheral
    /// calls this once the MTU is known rather than trusting the default.
    ///
    /// The result is clamped to [`MIN_CHUNK`]..=[`DEFAULT_CHUNK`]: an MTU
    /// smaller than the floor still sends, in frames the link will carry.
    pub fn fit_to_mtu(&mut self, mtu: usize) -> usize {
        self.fit_to_value_len(mtu.saturating_sub(3))
    }

    /// The same, for a platform that reports what a notification may CARRY
    /// rather than the MTU it was negotiated from.
    ///
    /// CoreBluetooth does: `CBCentral.maximumUpdateValueLength` is already
    /// `mtu - 3`, and two of the three peripherals are CoreBluetooth, so the
    /// conversion belongs here instead of in each of them. Passing that number
    /// to [`Self::fit_to_mtu`] would quietly cost three bytes per frame —
    /// nothing breaks, so nothing would ever find it (iOS, spec 075 T041,
    /// wrote the conversion by hand rather than risk exactly that).
    pub fn fit_to_value_len(&mut self, value_len: usize) -> usize {
        let room = value_len.saturating_sub(HEADER);
        self.chunk = room.clamp(MIN_CHUNK, DEFAULT_CHUNK);
        self.chunk
    }

    /// A write the peripheral refused: halve the chunk and try again, down to
    /// [`MIN_CHUNK`]. Returns `false` when there is nothing left to give up —
    /// the caller should report the failure rather than loop.
    pub fn halve(&mut self) -> bool {
        if self.chunk <= MIN_CHUNK {
            return false;
        }
        self.chunk = (self.chunk / 2).max(MIN_CHUNK);
        true
    }

    /// The frames for one message, in order.
    ///
    /// An empty payload is one empty frame, not none: `total` is at least 1, so
    /// a zero-length message still arrives (the page's `Math.max(1, …)`).
    #[must_use]
    pub fn frames(&self, flags: u8, msg_id: u8, payload: &[u8]) -> Vec<Vec<u8>> {
        let chunk = self.chunk.max(1);
        let total = payload.len().div_ceil(chunk).max(1);
        // 65 535 frames is 15 MB at the smallest chunk; nothing this channel
        // carries comes close, and a `total` that does not fit the header
        // would be silently truncated, so it is clamped where it is written.
        let total_field = u16::try_from(total).unwrap_or(u16::MAX);
        (0..total)
            .map(|seq| {
                let start = seq * chunk;
                let end = ((seq + 1) * chunk).min(payload.len());
                let body = payload.get(start..end).unwrap_or_default();
                let seq_field = u16::try_from(seq).unwrap_or(u16::MAX);
                let mut frame = Vec::with_capacity(HEADER + body.len());
                frame.push(flags);
                frame.push(msg_id);
                frame.extend_from_slice(&seq_field.to_be_bytes());
                frame.extend_from_slice(&total_field.to_be_bytes());
                frame.extend_from_slice(body);
                frame
            })
            .collect()
    }
}

#[derive(Clone, Debug)]
struct Pending {
    total: usize,
    flags: u8,
    parts: Vec<Option<Vec<u8>>>,
    got: usize,
    started_ms: u64,
}

/// Collects frames until a message is whole. Out-of-order arrival is fine;
/// a repeated frame is ignored rather than counted twice.
#[derive(Clone, Debug, Default)]
pub struct Reassembler {
    pending: BTreeMap<u8, Pending>,
}

impl Reassembler {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Which message a frame belongs to, for a shell's log line — `None` when
    /// it is too short to be a frame at all.
    ///
    /// It exists so that no peripheral has to reach into a frame itself. The
    /// Android one read the second byte for its diagnostics and pinned it with
    /// a test, which was honest and still the beginning of a second parser:
    /// the first `if` of one, in the shell, where §2 could move without it
    /// noticing. A shell that wants to name a message asks here.
    #[must_use]
    pub fn peek_id(frame: &[u8]) -> Option<u8> {
        if frame.len() < HEADER {
            return None;
        }
        frame.get(1).copied()
    }

    /// Take one frame. `Some` when it completed a message.
    ///
    /// A frame shorter than the header is not a frame; a frame whose `total`
    /// disagrees with what is already collected under that `msgId` starts the
    /// message over, because ids wrap and the previous owner of this one is
    /// gone (the page does the same).
    pub fn accept(&mut self, frame: &[u8], now_ms: u64) -> Option<Message> {
        if frame.len() < HEADER {
            return None;
        }
        let flags = frame[0];
        let msg_id = frame[1];
        let seq = usize::from(u16::from_be_bytes([frame[2], frame[3]]));
        let total = usize::from(u16::from_be_bytes([frame[4], frame[5]]));
        if total == 0 || seq >= total {
            return None;
        }
        let payload = frame.get(HEADER..).unwrap_or_default();

        // An id that comes round again (they wrap at 256) belongs to whoever
        // is sending now, not to whatever never finished under it before.
        let restart = self
            .pending
            .get(&msg_id)
            .is_none_or(|entry| entry.total != total);
        if restart {
            self.pending.insert(
                msg_id,
                Pending {
                    total,
                    flags,
                    parts: vec![None; total],
                    got: 0,
                    started_ms: now_ms,
                },
            );
        }
        let entry = self.pending.get_mut(&msg_id)?;

        if entry.parts.get(seq).is_some_and(Option::is_none) {
            entry.parts[seq] = Some(payload.to_vec());
            entry.got += 1;
        }
        if entry.got < entry.total {
            return None;
        }
        let done = self.pending.remove(&msg_id)?;
        Some(Message {
            flags: done.flags,
            msg_id,
            payload: done.parts.into_iter().flatten().flatten().collect(),
        })
    }

    /// Drop every message that has waited longer than
    /// [`REASSEMBLY_TIMEOUT_MS`], and say which ids went. A shell reports an
    /// error for each — a half-arrived message is not something to wait out.
    pub fn sweep(&mut self, now_ms: u64) -> Vec<u8> {
        let stale: Vec<u8> = self
            .pending
            .iter()
            .filter(|(_, entry)| now_ms.saturating_sub(entry.started_ms) >= REASSEMBLY_TIMEOUT_MS)
            .map(|(id, _)| *id)
            .collect();
        for id in &stale {
            self.pending.remove(id);
        }
        stale
    }

    /// How many messages are half-arrived. For a shell's own assertions.
    #[must_use]
    pub fn pending(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn roundtrip(payload: &[u8], chunk: usize) -> Message {
        let mut framer = Framer::new();
        while framer.chunk() > chunk {
            assert!(framer.halve());
        }
        let id = framer.next_id();
        let frames = framer.frames(FLAG_SEALED, id, payload);
        let mut reassembler = Reassembler::new();
        let mut last = None;
        for frame in &frames {
            last = reassembler.accept(frame, 0);
        }
        last.expect("the frames completed the message")
    }

    #[test]
    fn a_message_survives_being_cut_up() {
        let payload: Vec<u8> = (0..1000u32).map(|n| (n % 251) as u8).collect();
        let whole = roundtrip(&payload, 244);
        assert_eq!(whole.payload, payload);
        assert!(whole.sealed());
        // …and at the floor, where a frame carries 20 bytes.
        assert_eq!(roundtrip(&payload, 20).payload, payload);
    }

    #[test]
    fn the_header_is_the_protocols() {
        let framer = Framer::new();
        let frames = framer.frames(FLAG_SEALED, 0x2a, &[7u8; 500]);
        assert_eq!(frames.len(), 3);
        assert_eq!(frames[0][..HEADER], [1, 0x2a, 0, 0, 0, 3]);
        assert_eq!(frames[1][..HEADER], [1, 0x2a, 0, 1, 0, 3]);
        assert_eq!(frames[2][..HEADER], [1, 0x2a, 0, 2, 0, 3]);
        assert_eq!(frames[0].len(), HEADER + 244);
        assert_eq!(frames[2].len(), HEADER + 12);
    }

    #[test]
    fn an_empty_message_is_one_empty_frame() {
        let framer = Framer::new();
        let frames = framer.frames(0, 9, &[]);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0], vec![0, 9, 0, 0, 0, 1]);
        let mut reassembler = Reassembler::new();
        let message = reassembler.accept(&frames[0], 0).unwrap();
        assert_eq!(message.payload, Vec::<u8>::new());
        assert!(!message.sealed(), "the handshake travels in the clear");
    }

    #[test]
    fn frames_may_arrive_in_any_order_and_twice() {
        let framer = Framer::new();
        let payload: Vec<u8> = (0..700u32).map(|n| n as u8).collect();
        let frames = framer.frames(FLAG_SEALED, 3, &payload);
        assert_eq!(frames.len(), 3);
        let mut reassembler = Reassembler::new();
        assert!(reassembler.accept(&frames[2], 0).is_none());
        assert!(reassembler.accept(&frames[0], 0).is_none());
        // The same frame again is not a second frame.
        assert!(reassembler.accept(&frames[0], 0).is_none());
        let whole = reassembler.accept(&frames[1], 0).unwrap();
        assert_eq!(whole.payload, payload);
        assert_eq!(reassembler.pending(), 0);
    }

    #[test]
    fn two_messages_interleave_without_mixing() {
        let framer = Framer::new();
        let first = framer.frames(FLAG_SEALED, 1, &[0xaa; 300]);
        let second = framer.frames(FLAG_SEALED, 2, &[0xbb; 300]);
        let mut reassembler = Reassembler::new();
        assert!(reassembler.accept(&first[0], 0).is_none());
        assert!(reassembler.accept(&second[0], 0).is_none());
        assert_eq!(reassembler.pending(), 2);
        let done_second = reassembler.accept(&second[1], 0).unwrap();
        let done_first = reassembler.accept(&first[1], 0).unwrap();
        assert_eq!(
            (done_second.msg_id, done_second.payload),
            (2, vec![0xbb; 300])
        );
        assert_eq!(
            (done_first.msg_id, done_first.payload),
            (1, vec![0xaa; 300])
        );
    }

    #[test]
    fn a_message_that_never_finishes_is_swept_not_waited_out() {
        let framer = Framer::new();
        let frames = framer.frames(FLAG_SEALED, 5, &[1u8; 400]);
        let mut reassembler = Reassembler::new();
        assert!(reassembler.accept(&frames[0], 1_000).is_none());
        assert!(reassembler.sweep(5_000).is_empty(), "still within its time");
        assert_eq!(reassembler.sweep(11_000), vec![5]);
        assert_eq!(reassembler.pending(), 0);
        // Its late second frame cannot resurrect a message nobody is waiting for.
        assert!(reassembler.accept(&frames[1], 12_000).is_none());
        assert_eq!(reassembler.pending(), 1, "it starts a new one instead");
    }

    #[test]
    fn a_reused_id_with_a_different_length_starts_over() {
        let framer = Framer::new();
        let long = framer.frames(FLAG_SEALED, 7, &[1u8; 700]);
        let short = framer.frames(FLAG_SEALED, 7, &[2u8; 100]);
        let mut reassembler = Reassembler::new();
        assert!(reassembler.accept(&long[0], 0).is_none());
        // 256 messages later the same id comes round; its frames are not the
        // old ones' siblings.
        let whole = reassembler.accept(&short[0], 0).unwrap();
        assert_eq!(whole.payload, vec![2u8; 100]);
        assert_eq!(reassembler.pending(), 0);
    }

    #[test]
    fn a_frame_can_be_named_without_being_parsed() {
        let framer = Framer::new();
        let frames = framer.frames(FLAG_SEALED, 77, &[1u8; 600]);
        for frame in &frames {
            assert_eq!(Reassembler::peek_id(frame), Some(77));
        }
        assert_eq!(Reassembler::peek_id(&[1, 2, 3]), None, "not a frame");
        assert_eq!(Reassembler::peek_id(&[]), None);
    }

    #[test]
    fn nonsense_is_refused_rather_than_trusted() {
        let mut reassembler = Reassembler::new();
        assert!(
            reassembler.accept(&[1, 2, 3], 0).is_none(),
            "shorter than a header"
        );
        assert!(
            reassembler.accept(&[1, 0, 0, 0, 0, 0], 0).is_none(),
            "a message of no frames"
        );
        assert!(
            reassembler.accept(&[1, 0, 0, 5, 0, 2], 0).is_none(),
            "frame 5 of 2"
        );
        assert_eq!(reassembler.pending(), 0);
    }

    /// The three uuids a peripheral advertises and serves. A typo in one of
    /// them is a device that never appears in the chooser, which is the
    /// hardest possible way to learn about a typo — so they live here, once.
    #[test]
    fn the_uuids_are_the_pages() {
        let page = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../app-web/clearsigning/lib/transport/ble.js"
        ))
        .expect("the page's BLE transport");
        for (name, uuid) in [
            ("SERVICE", SERVICE_UUID),
            ("C2P", C2P_UUID),
            ("P2C", P2C_UUID),
        ] {
            assert!(
                page.contains(&format!("var {name} = '{uuid}'")),
                "{name} is not what the page connects to"
            );
        }
    }

    /// The MTU ladder, which all three peripherals walk: a notify cannot be
    /// split, so the frame must fit the link whole.
    #[test]
    fn a_frame_fits_the_link_it_is_notified_over() {
        let mut framer = Framer::new();
        // The MTU every modern phone negotiates. 244 + 6 would be 250, which
        // an ATT value of 244 cannot carry — this is the bug that sends.
        assert_eq!(framer.fit_to_mtu(247), 238);
        assert_eq!(framer.chunk() + HEADER, 244, "exactly the ATT payload");
        assert_eq!(framer.fit_to_mtu(185), 176, "an older iPhone");
        assert_eq!(
            framer.fit_to_mtu(512),
            DEFAULT_CHUNK,
            "never above the default"
        );
        // The default MTU: 23 - 3 - 6 = 14, under the floor, so the floor.
        assert_eq!(framer.fit_to_mtu(23), MIN_CHUNK);
        assert_eq!(
            framer.fit_to_mtu(0),
            MIN_CHUNK,
            "nonsense still sends something"
        );

        // CoreBluetooth reports the notification's capacity, not the MTU it
        // came from. The two doors must land in the same place.
        let mut core_bluetooth = Framer::new();
        for mtu in [512, 247, 185, 67, 23] {
            let mut from_mtu = Framer::new();
            assert_eq!(
                core_bluetooth.fit_to_value_len(mtu - 3),
                from_mtu.fit_to_mtu(mtu),
                "mtu {mtu}"
            );
        }

        // Every frame of a real message fits what the link takes.
        framer.fit_to_mtu(247);
        let id = framer.next_id();
        for frame in framer.frames(FLAG_SEALED, id, &[7u8; 2_500]) {
            assert!(frame.len() <= 247 - 3, "a frame of {} bytes", frame.len());
        }
    }

    #[test]
    fn ids_wrap_and_the_chunk_has_a_floor() {
        let mut framer = Framer::new();
        framer.last_id = 254;
        assert_eq!(framer.next_id(), 255);
        assert_eq!(framer.next_id(), 0, "0-255 and round again");

        let mut chunks = vec![framer.chunk()];
        while framer.halve() {
            chunks.push(framer.chunk());
        }
        assert_eq!(chunks, vec![244, 122, 61, 30, 20]);
        assert!(!framer.halve(), "nothing left to give up");
    }
}
