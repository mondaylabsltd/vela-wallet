//! RGB PNG encoding: per-row filter chosen by the usual minimum-sum-of-
//! absolute-differences heuristic, then zlib with a greedy LZ77 + fixed-
//! Huffman deflate. Not the smallest PNG possible; small enough, and short.

const MAX_MATCH: usize = 258;
const WINDOW: usize = 32 * 1024;
const HASH_BITS: u32 = 15;
const MAX_CHAIN: usize = 96;
/// A match this long is good enough: stop searching (zlib's `nice_length`).
const NICE_MATCH: usize = 128;

pub fn encode_rgb(width: u32, height: u32, rgb: &[u8]) -> Vec<u8> {
    let stride = width as usize * 3;
    assert_eq!(rgb.len(), stride * height as usize);
    let mut raw = Vec::with_capacity((stride + 1) * height as usize);
    let mut prev = vec![0u8; stride];
    let mut trial = vec![0u8; stride];
    let mut best = vec![0u8; stride];
    for row in rgb.chunks_exact(stride) {
        let mut best_kind = 0u8;
        let mut best_cost = u64::MAX;
        for kind in 0..5u8 {
            filter(kind, row, &prev, &mut trial);
            let cost: u64 = trial
                .iter()
                .map(|&b| u64::from((b as i8).unsigned_abs()))
                .sum();
            if cost < best_cost {
                best_cost = cost;
                best_kind = kind;
                best.copy_from_slice(&trial);
            }
        }
        raw.push(best_kind);
        raw.extend_from_slice(&best);
        prev.copy_from_slice(row);
    }

    let mut out = Vec::new();
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&width.to_be_bytes());
    ihdr.extend_from_slice(&height.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, truecolour, deflate, adaptive, no interlace
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

fn filter(kind: u8, row: &[u8], prev: &[u8], out: &mut [u8]) {
    for i in 0..row.len() {
        let a = if i >= 3 { row[i - 3] } else { 0 };
        let b = prev[i];
        let c = if i >= 3 { prev[i - 3] } else { 0 };
        let predictor = match kind {
            0 => 0,
            1 => a,
            2 => b,
            3 => ((u16::from(a) + u16::from(b)) / 2) as u8,
            _ => paeth(a, b, c),
        };
        out[i] = row[i].wrapping_sub(predictor);
    }
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let p = i16::from(a) + i16::from(b) - i16::from(c);
    let (pa, pb, pc) = (
        (p - i16::from(a)).abs(),
        (p - i16::from(b)).abs(),
        (p - i16::from(c)).abs(),
    );
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xedb8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(data: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for chunk in data.chunks(5552) {
        for &x in chunk {
            a += u32::from(x);
            b += a;
        }
        a %= 65521;
        b %= 65521;
    }
    (b << 16) | a
}

struct Bits {
    out: Vec<u8>,
    acc: u64,
    n: u32,
}

impl Bits {
    fn put(&mut self, value: u32, len: u32) {
        self.acc |= u64::from(value) << self.n;
        self.n += len;
        while self.n >= 8 {
            self.out.push(self.acc as u8);
            self.acc >>= 8;
            self.n -= 8;
        }
    }
    /// Huffman codes go out most-significant bit first.
    fn code(&mut self, code: u32, len: u32) {
        self.put(code.reverse_bits() >> (32 - len), len);
    }
    fn finish(mut self) -> Vec<u8> {
        if self.n > 0 {
            self.out.push(self.acc as u8);
        }
        self.out
    }
}

fn literal(bits: &mut Bits, sym: u32) {
    match sym {
        0..=143 => bits.code(0x30 + sym, 8),
        144..=255 => bits.code(0x190 + (sym - 144), 9),
        256..=279 => bits.code(sym - 256, 7),
        _ => bits.code(0xc0 + (sym - 280), 8),
    }
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn emit_match(bits: &mut Bits, len: usize, dist: usize) {
    let li = LEN_BASE
        .iter()
        .rposition(|&b| usize::from(b) <= len)
        .unwrap_or(0);
    literal(bits, 257 + li as u32);
    bits.put(
        (len - usize::from(LEN_BASE[li])) as u32,
        u32::from(LEN_EXTRA[li]),
    );
    let di = DIST_BASE
        .iter()
        .rposition(|&b| usize::from(b) <= dist)
        .unwrap_or(0);
    bits.code(di as u32, 5);
    bits.put(
        (dist - usize::from(DIST_BASE[di])) as u32,
        u32::from(DIST_EXTRA[di]),
    );
}

pub fn zlib(data: &[u8]) -> Vec<u8> {
    let mut bits = Bits {
        out: vec![0x78, 0x01],
        acc: 0,
        n: 0,
    };
    bits.put(1, 1); // BFINAL
    bits.put(1, 2); // BTYPE = fixed Huffman
    let hash_size = 1usize << HASH_BITS;
    let mut head = vec![usize::MAX; hash_size];
    let mut prev = vec![usize::MAX; WINDOW];
    let hash = |i: usize| -> usize {
        let v = (u32::from(data[i]) << 16) | (u32::from(data[i + 1]) << 8) | u32::from(data[i + 2]);
        (v.wrapping_mul(0x9e37_79b1) >> (32 - HASH_BITS)) as usize
    };
    let insert = |i: usize, head: &mut Vec<usize>, prev: &mut Vec<usize>| {
        if i + 2 < data.len() {
            let h = hash(i);
            prev[i % WINDOW] = head[h];
            head[h] = i;
        }
    };
    let mut i = 0;
    while i < data.len() {
        let mut best_len = 0;
        let mut best_dist = 0;
        if i + 2 < data.len() {
            let mut cand = head[hash(i)];
            let mut chain = 0;
            let max = (data.len() - i).min(MAX_MATCH);
            while cand != usize::MAX && i - cand < WINDOW && chain < MAX_CHAIN {
                let mut l = 0;
                while l < max && data[cand + l] == data[i + l] {
                    l += 1;
                }
                if l > best_len {
                    best_len = l;
                    best_dist = i - cand;
                    if l >= NICE_MATCH.min(max) {
                        break;
                    }
                }
                let next = prev[cand % WINDOW];
                if next == usize::MAX || next >= cand {
                    break;
                }
                cand = next;
                chain += 1;
            }
        }
        if best_len >= 3 {
            emit_match(&mut bits, best_len, best_dist);
            for k in i..i + best_len {
                insert(k, &mut head, &mut prev);
            }
            i += best_len;
        } else {
            literal(&mut bits, u32::from(data[i]));
            insert(i, &mut head, &mut prev);
            i += 1;
        }
    }
    literal(&mut bits, 256);
    let mut out = bits.finish();
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}
