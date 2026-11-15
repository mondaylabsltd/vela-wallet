//! getvela.app's hashing and signing, behind a plain C ABI (no wasm-bindgen).
//!
//! ```text
//! sc_alloc(len) -> ptr                  a buffer the host writes into
//! sc_free(ptr, len)
//! sc_keccak256(in, len, out32)          Keccak-256 (Ethereum's, not SHA3-256)
//! sc_address(key32, out20) -> 1 | 0     the address of a secret key; 0 = not a valid key
//! sc_sign(key32, hash32, out65) -> 1|0  ECDSA over a 32-byte hash: r ‖ s ‖ recovery id
//! ```
//!
//! The module has no randomness of its own (wasm32-unknown-unknown has none):
//! a new key is 32 bytes the host draws from its CSPRNG, checked here with
//! `sc_address` — out of range (0 or ≥ n) is refused and the host draws again.
//!
//! Signatures are RFC 6979 deterministic and low-s normalized (k256's
//! secp256k1 normalizes s and flips the recovery id with it), which is what
//! Ethereum requires.

use k256::ecdsa::SigningKey;
use sha3::{Digest, Keccak256};

pub fn keccak256(data: &[u8]) -> [u8; 32] {
    Keccak256::digest(data).into()
}

/// The address of a secret key: the low 20 bytes of the Keccak-256 of its
/// uncompressed public point (without the 0x04 prefix).
pub fn address(secret: &[u8; 32]) -> Option<[u8; 20]> {
    let key = SigningKey::from_slice(secret).ok()?;
    let point = key.verifying_key().to_sec1_point(false);
    let hash = keccak256(&point.as_bytes()[1..]);
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[12..]);
    Some(out)
}

/// `r ‖ s ‖ recovery id` over a 32-byte prehash.
pub fn sign(secret: &[u8; 32], prehash: &[u8; 32]) -> Option<[u8; 65]> {
    let key = SigningKey::from_slice(secret).ok()?;
    let (sig, id) = key.sign_prehash_recoverable(prehash);
    let mut out = [0u8; 65];
    out[..64].copy_from_slice(&sig.to_bytes());
    out[64] = id.to_byte();
    Some(out)
}

#[no_mangle]
pub extern "C" fn sc_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len);
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr`/`len` must come from one `sc_alloc` call.
#[no_mangle]
pub unsafe extern "C" fn sc_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len));
}

/// # Safety
/// `input` must point at `len` initialised bytes, `out` at 32 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn sc_keccak256(input: *const u8, len: usize, out: *mut u8) {
    let data = std::slice::from_raw_parts(input, len);
    std::slice::from_raw_parts_mut(out, 32).copy_from_slice(&keccak256(data));
}

/// # Safety
/// `key` must point at 32 initialised bytes, `out` at 20 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn sc_address(key: *const u8, out: *mut u8) -> u32 {
    match address(&*(key as *const [u8; 32])) {
        Some(a) => {
            std::slice::from_raw_parts_mut(out, 20).copy_from_slice(&a);
            1
        }
        None => 0,
    }
}

/// # Safety
/// `key` and `hash` must point at 32 initialised bytes each, `out` at 65 writable bytes.
#[no_mangle]
pub unsafe extern "C" fn sc_sign(key: *const u8, hash: *const u8, out: *mut u8) -> u32 {
    match sign(&*(key as *const [u8; 32]), &*(hash as *const [u8; 32])) {
        Some(s) => {
            std::slice::from_raw_parts_mut(out, 65).copy_from_slice(&s);
            1
        }
        None => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn keccak_is_not_sha3() {
        assert_eq!(
            hex(&keccak256(b"")),
            "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
        );
    }

    #[test]
    fn anvil_key_zero_is_f39f() {
        let mut key = [0u8; 32];
        let h = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        for i in 0..32 {
            key[i] = u8::from_str_radix(&h[i * 2..i * 2 + 2], 16).unwrap();
        }
        assert_eq!(
            hex(&address(&key).unwrap()),
            "f39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn out_of_range_keys_are_refused() {
        assert!(address(&[0u8; 32]).is_none());
        assert!(address(&[0xff; 32]).is_none());
        assert!(sign(&[0u8; 32], &[1u8; 32]).is_none());
    }

    #[test]
    fn signatures_are_low_s() {
        let half_n: [u8; 32] = [
            0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0x5d, 0x57, 0x6e, 0x73, 0x57, 0xa4, 0x50, 0x1d, 0xdf, 0xe9, 0x2f, 0x46,
            0x68, 0x1b, 0x20, 0xa0,
        ];
        for seed in 1..=64u8 {
            let s = sign(&[seed; 32], &keccak256(&[seed])).unwrap();
            assert!(s[32..64] <= half_n[..], "high s for seed {seed}");
            assert!(s[64] <= 1);
        }
    }
}
