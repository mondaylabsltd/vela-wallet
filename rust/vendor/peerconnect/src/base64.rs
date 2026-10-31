//! Unpadded base64url (RFC 4648 §5), the only text encoding PeerConnect uses.
//!
//! Decoding is strict: padding, whitespace, foreign characters and non-zero
//! trailing bits are all rejected, so every byte string has exactly one
//! accepted encoding.

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

pub fn encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk.iter().enumerate().fold(0u32, |n, (i, &b)| n | (b as u32) << (16 - 8 * i));
        for i in 0..=chunk.len() {
            out.push(ALPHABET[(n >> (18 - 6 * i) & 63) as usize] as char);
        }
    }
    out
}

pub fn decode(text: &str) -> Option<Vec<u8>> {
    let text = text.as_bytes();
    if text.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    for chunk in text.chunks(4) {
        let mut n = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            n |= (value(c)? as u32) << (18 - 6 * i);
        }
        let len = chunk.len() - 1;
        // Bits below the last whole byte must be zero (canonical encoding).
        if n & (0xFF_FFFF >> (8 * len)) != 0 {
            return None;
        }
        out.extend_from_slice(&n.to_be_bytes()[1..=len]);
    }
    Some(out)
}

/// Decodes exactly `N` bytes.
pub fn decode_array<const N: usize>(text: &str) -> Option<[u8; N]> {
    decode(text)?.try_into().ok()
}

fn value(c: u8) -> Option<u8> {
    Some(match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'-' => 62,
        b'_' => 63,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD as REFERENCE;

    #[test]
    fn matches_reference_implementation() {
        let data: Vec<u8> = (0..=255u8).cycle().take(1000).collect();
        for len in 0..200 {
            let bytes = &data[len..len * 2 + 3];
            let text = encode(bytes);
            assert_eq!(text, REFERENCE.encode(bytes));
            assert_eq!(decode(&text).as_deref(), Some(bytes));
        }
    }

    #[test]
    fn rejects_non_canonical_input() {
        for bad in ["A", "AB=", "AB==", "A+", "A/", "AB C", "AR", "AAB", "-_9"] {
            assert_eq!(decode(bad), None, "{bad:?}");
        }
        assert_eq!(decode("AA"), Some(vec![0]));
        assert_eq!(decode("AAA"), Some(vec![0, 0]));
        assert_eq!(decode(""), Some(vec![]));
    }
}
