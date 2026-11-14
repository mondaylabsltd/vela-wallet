//! A sampled module grid → its text: format information, codewords, error
//! correction, and the data bitstream.
//!
//! Ported from zxing-cpp `QRDecoder.cpp`, `QRBitMatrixParser.cpp`,
//! `QRFormatInformation.cpp` and ZXing `DataBlock` (Apache-2.0, see NOTICE).
//!
//! Every mode the standard defines is read: numeric, alphanumeric, byte,
//! Kanji and Hanzi (turned back into their Shift_JIS / GB2312 bytes — that is
//! arithmetic, not a table), ECI, FNC1 and structured append. What the bytes
//! say as text is `content`'s question.

use crate::bitmatrix::BitMatrix;
use crate::content::{Charset, Code, Part, Segment};
use crate::rs;
use crate::version::{self, EcLevel};

// ---------------------------------------------------------------------------
// Format information
// ---------------------------------------------------------------------------

const FORMAT_MASK_QR: u32 = 0x5412;
const FORMAT_MASK_MODEL1: u32 = 0x2825;

/// ISO 18004:2015 Annex C, Table C.1: the 32 valid format words, masked.
const MASKED_FORMATS: [u32; 32] = [
    0x5412, 0x5125, 0x5E7C, 0x5B4B, 0x45F9, 0x40CE, 0x4F97, 0x4AA0, 0x77C4, 0x72F3, 0x7DAA, 0x789D,
    0x662F, 0x6318, 0x6C41, 0x6976, 0x1689, 0x13BE, 0x1CE7, 0x19D0, 0x0762, 0x0255, 0x0D0C, 0x083B,
    0x355F, 0x3068, 0x3F31, 0x3A06, 0x24B4, 0x2183, 0x2EDA, 0x2BED,
];

struct Format {
    ec: EcLevel,
    mask: u32,
    mirrored: bool,
}

fn mirror15(bits: u32) -> u32 {
    bits.reverse_bits() >> 17
}

/// Both copies of the format information, each also read as if the symbol
/// were mirrored; the closest valid word within 3 bits wins.
fn decode_format(bits1: u32, bits2: u32) -> Option<Format> {
    // The second copy carries the always-dark module: drop it (at bit 8, or at
    // bit 7 when mirrored).
    let mirrored2 = mirror15(((bits2 >> 1) & 0b111_1111_1000_0000) | (bits2 & 0b111_1111));
    let bits2 = ((bits2 >> 1) & 0b111_1111_0000_0000) | (bits2 & 0b1111_1111);
    let candidates = [bits1, bits2, mirror15(bits1), mirrored2];
    let (mut best_dist, mut best_mask, mut best_data, mut best_index) = (u32::MAX, 0, 0, 0);
    // 0: no mask at all, written by some faulty generators.
    for mask in [FORMAT_MASK_QR, 0, FORMAT_MASK_MODEL1] {
        for (index, &bits) in candidates.iter().enumerate() {
            for masked in MASKED_FORMATS {
                let pattern = masked ^ FORMAT_MASK_QR;
                let dist = ((bits ^ mask) ^ pattern).count_ones();
                if dist < best_dist {
                    (best_dist, best_mask, best_data, best_index) =
                        (dist, mask, pattern >> 10, index);
                }
            }
        }
    }
    if best_dist > 3 || best_mask == FORMAT_MASK_MODEL1 {
        return None;
    }
    Some(Format {
        ec: EcLevel::from_bits(best_data >> 3),
        mask: best_data & 7,
        mirrored: best_index > 1,
    })
}

fn read_format(bits: &BitMatrix) -> Option<Format> {
    let d = bits.height();
    let mut b1 = 0u32;
    let push = |v: &mut u32, x: usize, y: usize| *v = (*v << 1) | u32::from(bits.get(x, y));
    for x in 0..6 {
        push(&mut b1, x, 8);
    }
    push(&mut b1, 7, 8);
    push(&mut b1, 8, 8);
    push(&mut b1, 8, 7);
    for y in (0..=5).rev() {
        push(&mut b1, 8, y);
    }
    let mut b2 = 0u32;
    for y in (d - 8..d).rev() {
        push(&mut b2, 8, y);
    }
    for x in d - 8..d {
        push(&mut b2, x, 8);
    }
    decode_format(b1, b2)
}

// ---------------------------------------------------------------------------
// Codewords
// ---------------------------------------------------------------------------

fn mask_bit(mask: u32, x: usize, y: usize) -> bool {
    match mask {
        0 => (y + x) % 2 == 0,
        1 => y % 2 == 0,
        2 => x % 3 == 0,
        3 => (y + x) % 3 == 0,
        4 => ((y / 2) + (x / 3)) % 2 == 0,
        5 => (y * x) % 6 == 0,
        6 => ((y * x) % 6) < 3,
        _ => (y + x + ((y * x) % 3)) % 2 == 0,
    }
}

/// The data modules in the standard zig-zag, unmasked, as bytes.
fn read_codewords(bits: &BitMatrix, version: u32, format: &Format) -> Option<Vec<u8>> {
    let function = version::function_pattern(version);
    let d = bits.height();
    let total = version::total_codewords(version);
    let mut out = Vec::with_capacity(total);
    let (mut byte, mut read, mut up) = (0u8, 0usize, true);
    let mut x = d as isize - 1;
    while x > 0 {
        if x == 6 {
            // Skip the vertical timing pattern.
            x -= 1;
        }
        for row in 0..d {
            let y = if up { d - 1 - row } else { row };
            for col in 0..2 {
                let xx = (x - col) as usize;
                if !function.get(xx, y) {
                    let module = if format.mirrored {
                        bits.get(y, xx)
                    } else {
                        bits.get(xx, y)
                    };
                    byte = (byte << 1) | u8::from(mask_bit(format.mask, xx, y) != module);
                    read += 1;
                    if read % 8 == 0 {
                        out.push(core::mem::take(&mut byte));
                    }
                }
            }
        }
        up = !up;
        x -= 2;
    }
    (out.len() == total).then_some(out)
}

/// De-interleave the codewords into blocks, correct each, return the data.
fn correct_blocks(raw: &[u8], version: u32, ec: EcLevel) -> Option<Vec<u8>> {
    let (ec_per_block, c1, d1, c2, d2) = version::info(version).ec[ec as usize];
    let ec_per_block = usize::from(ec_per_block);
    let mut blocks: Vec<(usize, Vec<u8>)> = Vec::new();
    for (count, data) in [(c1, d1), (c2, d2)] {
        for _ in 0..count {
            blocks.push((
                usize::from(data),
                vec![0u8; usize::from(data) + ec_per_block],
            ));
        }
    }
    let n = blocks.len();
    if n == 0 {
        return None;
    }
    // All blocks hold the same amount of data except the last few, which hold one more.
    let shorter_total = blocks[0].1.len();
    let longer_from = blocks
        .iter()
        .position(|b| b.1.len() != shorter_total)
        .unwrap_or(n);
    let shorter_data = shorter_total - ec_per_block;
    let mut it = raw.iter().copied();
    for i in 0..shorter_data {
        for b in blocks.iter_mut() {
            b.1[i] = it.next()?;
        }
    }
    for b in blocks.iter_mut().skip(longer_from) {
        b.1[shorter_data] = it.next()?;
    }
    for i in shorter_data..shorter_total {
        for (j, b) in blocks.iter_mut().enumerate() {
            let at = if j < longer_from { i } else { i + 1 };
            b.1[at] = it.next()?;
        }
    }
    let mut data = Vec::with_capacity(blocks.iter().map(|b| b.0).sum());
    for (n_data, mut cw) in blocks {
        rs::correct(&mut cw, ec_per_block)?;
        data.extend_from_slice(&cw[..n_data]);
    }
    Some(data)
}

// ---------------------------------------------------------------------------
// The data bitstream
// ---------------------------------------------------------------------------

struct Bits<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Bits<'_> {
    fn available(&self) -> usize {
        self.bytes.len() * 8 - self.pos
    }
    fn peek(&self, n: usize) -> Option<u32> {
        if n > 32 || n > self.available() {
            return None;
        }
        let mut v = 0u32;
        for i in self.pos..self.pos + n {
            v = (v << 1) | u32::from((self.bytes[i / 8] >> (7 - i % 8)) & 1);
        }
        Some(v)
    }
    fn read(&mut self, n: usize) -> Option<u32> {
        let v = self.peek(n)?;
        self.pos += n;
        Some(v)
    }
}

const ALPHANUMERIC: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

/// Character-count field widths: numeric, alphanumeric, byte, Kanji/Hanzi.
fn count_bits(version: u32, mode: u32) -> usize {
    let row = match version {
        1..=9 => [10, 9, 8, 8],
        10..=26 => [12, 11, 16, 10],
        _ => [14, 13, 16, 12],
    };
    match mode {
        1 => row[0],
        2 => row[1],
        4 => row[2],
        _ => row[3],
    }
}

fn push_digits(out: &mut Vec<u8>, value: u32, digits: u32) -> Option<()> {
    if value >= 10u32.pow(digits) {
        return None;
    }
    for i in (0..digits).rev() {
        out.push(b'0' + ((value / 10u32.pow(i)) % 10) as u8);
    }
    Some(())
}

/// Segments being built: text from numeric/alphanumeric modes is ASCII and
/// joins the current run when that run's charset is ASCII-compatible.
struct Builder {
    segments: Vec<Segment>,
    /// The charset an ECI declared for byte segments; `None` = undeclared.
    eci: Option<Charset>,
}

impl Builder {
    fn push(&mut self, charset: Charset, guessed: bool, bytes: &[u8]) {
        if let Some(last) = self.segments.last_mut() {
            let same = last.charset == charset && last.guessed == guessed;
            // ASCII is a subset of every charset a byte segment can be in
            // except the UTF-16/32 ones (and binary).
            let ascii_into_last = charset == Charset::Ascii
                && !matches!(
                    last.charset,
                    Charset::Utf16Be
                        | Charset::Utf16Le
                        | Charset::Utf32Be
                        | Charset::Utf32Le
                        | Charset::Binary
                        | Charset::Unknown(_)
                );
            if same || ascii_into_last {
                last.bytes.extend_from_slice(bytes);
                return;
            }
        }
        self.segments.push(Segment {
            charset,
            guessed,
            bytes: bytes.to_vec(),
        });
    }

    fn ascii(&mut self, bytes: &[u8]) {
        self.push(Charset::Ascii, false, bytes);
    }

    fn data(&mut self, bytes: &[u8]) {
        match self.eci {
            Some(c) => self.push(c, false, bytes),
            None => self.push(Charset::Latin1, true, bytes),
        }
    }

    /// Undeclared runs: settle each on its most likely charset.
    fn finish(mut self) -> Vec<Segment> {
        for s in &mut self.segments {
            if s.charset == Charset::Ascii && !s.bytes.is_ascii() {
                s.guessed = true;
            }
            if s.guessed {
                s.charset = s.candidates()[0];
            }
        }
        self.segments
    }
}

fn decode_bitstream(bytes: &[u8], version: u32) -> Option<Code> {
    let mut bits = Bits { bytes, pos: 0 };
    let mut b = Builder {
        segments: Vec::new(),
        eci: None,
    };
    let mut part = None;
    let mut gs1 = false;
    loop {
        // End of data: fewer than 4 bits left, or the terminator 0000.
        let left = bits.available().min(4);
        if left == 0 || bits.peek(left)? == 0 {
            break;
        }
        match bits.read(4)? {
            1 => {
                let mut count = bits.read(count_bits(version, 1))?;
                let mut digits = Vec::with_capacity(count as usize);
                while count > 0 {
                    let n = count.min(3);
                    push_digits(&mut digits, bits.read(1 + 3 * n as usize)?, n)?;
                    count -= n;
                }
                b.ascii(&digits);
            }
            2 => {
                let count = bits.read(count_bits(version, 2))?;
                let mut chars = Vec::with_capacity(count as usize);
                for _ in 0..count / 2 {
                    let pair = bits.read(11)?;
                    chars.push(*ALPHANUMERIC.get((pair / 45) as usize)?);
                    chars.push(*ALPHANUMERIC.get((pair % 45) as usize)?);
                }
                if count % 2 == 1 {
                    chars.push(*ALPHANUMERIC.get(bits.read(6)? as usize)?);
                }
                if gs1 {
                    // GS1: "%%" is a literal '%', a lone '%' the FNC1 separator.
                    let mut out = Vec::with_capacity(chars.len());
                    let mut i = 0;
                    while i < chars.len() {
                        if chars[i] == b'%' && chars.get(i + 1) == Some(&b'%') {
                            out.push(b'%');
                            i += 1;
                        } else {
                            out.push(if chars[i] == b'%' { 0x1D } else { chars[i] });
                        }
                        i += 1;
                    }
                    chars = out;
                }
                b.ascii(&chars);
            }
            4 => {
                let count = bits.read(count_bits(version, 4))?;
                let mut data = Vec::with_capacity(count as usize);
                for _ in 0..count {
                    data.push(bits.read(8)? as u8);
                }
                b.data(&data);
            }
            8 => {
                // Kanji: 13 bits → a Shift_JIS double byte.
                let count = bits.read(count_bits(version, 8))?;
                let mut sjis = Vec::with_capacity(2 * count as usize);
                for _ in 0..count {
                    let v = bits.read(13)?;
                    let mut w = ((v / 0xC0) << 8) | (v % 0xC0);
                    w += if w < 0x1F00 { 0x8140 } else { 0xC140 };
                    sjis.extend_from_slice(&(w as u16).to_be_bytes());
                }
                b.push(Charset::Table("shift_jis"), false, &sjis);
            }
            13 => {
                // Hanzi (GB/T 18284): subset 1 is GB2312; 13 bits → a double byte.
                if bits.read(4)? != 1 {
                    return None;
                }
                let count = bits.read(count_bits(version, 13))?;
                let mut gb = Vec::with_capacity(2 * count as usize);
                for _ in 0..count {
                    let v = bits.read(13)?;
                    let mut w = ((v / 0x60) << 8) | (v % 0x60);
                    w += if w < 0x0A00 { 0xA1A1 } else { 0xA6A1 };
                    gb.extend_from_slice(&(w as u16).to_be_bytes());
                }
                b.push(Charset::Table("gb18030"), false, &gb);
            }
            7 => {
                let first = bits.read(8)?;
                let eci = if first & 0x80 == 0 {
                    first & 0x7F
                } else if first & 0xC0 == 0x80 {
                    ((first & 0x3F) << 8) | bits.read(8)?
                } else if first & 0xE0 == 0xC0 {
                    ((first & 0x1F) << 16) | bits.read(16)?
                } else {
                    return None;
                };
                b.eci = Some(Charset::from_eci(eci));
            }
            3 => {
                // Structured append: this symbol is one of several.
                let index = bits.read(4)? as u8;
                let count = bits.read(4)? as u8 + 1;
                let parity = bits.read(8)? as u8;
                part = Some(Part {
                    index,
                    count,
                    parity,
                });
            }
            5 => gs1 = true, // FNC1 in first position
            9 => {
                // FNC1 in second position: an application indicator first.
                if !b.segments.is_empty() {
                    return None;
                }
                let app = bits.read(8)?;
                if app < 100 {
                    let mut d = Vec::with_capacity(2);
                    push_digits(&mut d, app, 2)?;
                    b.ascii(&d);
                } else if (165..=190).contains(&app) || (197..=222).contains(&app) {
                    b.ascii(&[(app - 100) as u8]);
                } else {
                    return None;
                }
            }
            _ => return None,
        }
    }
    Some(Code {
        segments: b.finish(),
        part,
    })
}

/// A sampled grid's content, or `None` if any step fails its checks.
pub(crate) fn decode(bits: &BitMatrix) -> Option<Code> {
    if bits.width() != bits.height() {
        return None;
    }
    let version = version::from_dimension(bits.width())?;
    let format = read_format(bits)?;
    let raw = read_codewords(bits, version, &format)?;
    let data = correct_blocks(&raw, version, format.ec)?;
    decode_bitstream(&data, version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MSB-first bit writer for hand-built bitstreams.
    struct W(Vec<bool>);
    impl W {
        fn put(&mut self, v: u32, n: usize) -> &mut Self {
            for i in (0..n).rev() {
                self.0.push((v >> i) & 1 == 1);
            }
            self
        }
        fn bytes(&self) -> Vec<u8> {
            let mut out = vec![0u8; self.0.len().div_ceil(8) + 1];
            for (i, &bit) in self.0.iter().enumerate() {
                out[i / 8] |= u8::from(bit) << (7 - i % 8);
            }
            out
        }
    }

    #[test]
    fn every_format_word_decodes_and_survives_three_bit_errors() {
        for (i, &masked) in MASKED_FORMATS.iter().enumerate() {
            let f = decode_format(masked, 0).expect("exact");
            assert_eq!(f.mask, i as u32 & 7);
            assert!(!f.mirrored);
            assert!(decode_format(masked ^ 0b1_0000_0100_0001, 0).is_some());
        }
    }

    #[test]
    fn numeric_segment() {
        // "01234567" (numeric, v1): 0001 0000001000 0000001100 0101011001 1000011 0000
        let bytes = [0x10, 0x20, 0x0C, 0x56, 0x61, 0x80, 0xEC, 0x11];
        assert_eq!(
            decode_bitstream(&bytes, 1)
                .and_then(|c| c.text())
                .as_deref(),
            Some("01234567")
        );
    }

    #[test]
    fn kanji_mode_becomes_shift_jis_bytes() {
        // ISO 18004 example: 点 (0x935F) → 0x0D9F, 茗 (0xE4AA) → 0x1AAA.
        let mut w = W(Vec::new());
        w.put(8, 4)
            .put(2, 8)
            .put(0x0D9F, 13)
            .put(0x1AAA, 13)
            .put(0, 4);
        let code = decode_bitstream(&w.bytes(), 1).expect("decodes");
        assert_eq!(
            code.segments,
            vec![Segment {
                charset: Charset::Table("shift_jis"),
                guessed: false,
                bytes: vec![0x93, 0x5F, 0xE4, 0xAA]
            }]
        );
        assert_eq!(code.text(), None, "the platform's decoder reads Shift_JIS");
    }

    #[test]
    fn hanzi_mode_becomes_gb2312_bytes() {
        // 中 = GB2312 0xD6D0: (0xD6D0 - 0xA6A1) = 0x302F → 0x30 * 0x60 + 0x2F.
        let mut w = W(Vec::new());
        w.put(13, 4)
            .put(1, 4)
            .put(1, 8)
            .put(0x30 * 0x60 + 0x2F, 13)
            .put(0, 4);
        let code = decode_bitstream(&w.bytes(), 1).expect("decodes");
        assert_eq!(code.segments[0].bytes, vec![0xD6, 0xD0]);
        assert_eq!(code.segments[0].charset, Charset::Table("gb18030"));
    }

    #[test]
    fn eci_switches_the_byte_charset_and_digits_stay_apart_from_utf16() {
        let mut w = W(Vec::new());
        // ECI 25 (UTF-16BE), byte "中" = 4E 2D, then numeric "7".
        w.put(7, 4)
            .put(25, 8)
            .put(4, 4)
            .put(2, 8)
            .put(0x4E, 8)
            .put(0x2D, 8)
            .put(1, 4)
            .put(1, 10)
            .put(7, 4)
            .put(0, 4);
        let code = decode_bitstream(&w.bytes(), 1).expect("decodes");
        assert_eq!(code.text().as_deref(), Some("中7"));
        assert_eq!(code.segments.len(), 2);
    }

    #[test]
    fn structured_append_is_read_and_marked() {
        let mut w = W(Vec::new());
        // Part 2 of 3, parity 0x5A, then byte "ab".
        w.put(3, 4)
            .put(1, 4)
            .put(2, 4)
            .put(0x5A, 8)
            .put(4, 4)
            .put(2, 8)
            .put(u32::from(b'a'), 8)
            .put(u32::from(b'b'), 8)
            .put(0, 4);
        let code = decode_bitstream(&w.bytes(), 1).expect("decodes");
        assert_eq!(
            code.part,
            Some(Part {
                index: 1,
                count: 3,
                parity: 0x5A
            })
        );
        assert_eq!(code.text().as_deref(), Some("ab"));
    }

    #[test]
    fn an_unknown_eci_keeps_the_bytes() {
        let mut w = W(Vec::new());
        // ECI 1000 (two-byte form 10xxxxxx xxxxxxxx), byte 0xC8.
        w.put(7, 4)
            .put(0x8000 | 1000, 16)
            .put(4, 4)
            .put(1, 8)
            .put(0xC8, 8)
            .put(0, 4);
        let code = decode_bitstream(&w.bytes(), 1).expect("decodes");
        assert_eq!(code.segments[0].charset, Charset::Unknown(1000));
        assert_eq!(code.segments[0].bytes, vec![0xC8]);
        assert_eq!(code.text(), None);
    }
}
