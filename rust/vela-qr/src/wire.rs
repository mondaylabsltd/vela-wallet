//! A [`Code`] as bytes, for hosts that read it across a C ABI — the web's wasm
//! module (`wasm/`) and the native library (`ffi/`, Android over JNA). One
//! format, so one parser per language: `qr-wasm.ts` on the web, `QrDecoder.kt`
//! on Android.
//!
//! Little-endian:
//!
//! ```text
//! u8  format = 1
//! u8  part flag; if 1: u8 index (0-based), u8 count
//! u32 segment count, then per segment:
//!     u8 candidate count, then per candidate, in the order to try:
//!         u8 0, u32 len, UTF-8 text   decoded by this crate: take it
//!         u8 1, u8 len, WHATWG label  decode the raw bytes strictly with the
//!                                     platform (TextDecoder, java.nio.charset)
//!     u32 len, the segment's raw bytes
//! ```
//!
//! A segment with no candidates (binary data, an unknown ECI) has no text.

use crate::content::{Charset, Code};

/// The format version written first.
pub const FORMAT: u8 = 1;

/// `code` in the wire format.
#[must_use]
pub fn encode(code: &Code) -> Vec<u8> {
    let mut out = vec![FORMAT];
    match code.part {
        Some(p) => out.extend_from_slice(&[1, p.index, p.count]),
        None => out.push(0),
    }
    put_u32(&mut out, code.segments.len());
    for seg in &code.segments {
        let at = out.len();
        out.push(0);
        let mut n = 0u8;
        for c in seg.candidates() {
            if let Charset::Table(label) = c {
                out.push(1);
                out.push(label.len() as u8);
                out.extend_from_slice(label.as_bytes());
                n += 1;
            } else if let Some(text) = c.decode(&seg.bytes) {
                // Decoded here: nothing after it can be reached.
                out.push(0);
                put_u32(&mut out, text.len());
                out.extend_from_slice(text.as_bytes());
                n += 1;
                break;
            }
        }
        out[at] = n;
        put_u32(&mut out, seg.bytes.len());
        out.extend_from_slice(&seg.bytes);
    }
    out
}

fn put_u32(out: &mut Vec<u8>, v: usize) {
    out.extend_from_slice(&u32::try_from(v).unwrap_or(u32::MAX).to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::{Part, Segment};

    #[test]
    fn a_text_segment_and_a_table_segment() {
        let code = Code {
            segments: vec![
                Segment {
                    charset: Charset::Ascii,
                    guessed: false,
                    bytes: b"ab".to_vec(),
                },
                Segment {
                    charset: Charset::Table("shift_jis"),
                    guessed: false,
                    bytes: vec![0x93, 0xFA],
                },
            ],
            part: Some(Part {
                index: 1,
                count: 2,
                parity: 0,
            }),
        };
        let mut want = vec![1, 1, 1, 2, 2, 0, 0, 0];
        want.extend_from_slice(&[1, 0, 2, 0, 0, 0, b'a', b'b', 2, 0, 0, 0, b'a', b'b']);
        want.extend_from_slice(&[1, 1, 9]);
        want.extend_from_slice(b"shift_jis");
        want.extend_from_slice(&[2, 0, 0, 0, 0x93, 0xFA]);
        assert_eq!(encode(&code), want);
    }

    #[test]
    fn binary_data_has_no_candidates() {
        let code = Code {
            segments: vec![Segment {
                charset: Charset::Binary,
                guessed: false,
                bytes: vec![0, 1],
            }],
            part: None,
        };
        assert_eq!(encode(&code), vec![1, 0, 1, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1]);
    }
}
