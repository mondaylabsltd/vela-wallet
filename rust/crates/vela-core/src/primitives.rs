//! Foundation codecs and hashes.
//!
//! Replaces src/services/{hex,sha256,eth-crypto}.ts (and the parallel
//! EthCrypto.swift). Hashing/CREATE2/EIP-55 come from alloy-primitives + sha2 —
//! never hand-rolled here (FR-005). Behavior is pinned by
//! `tests/vectors/primitives.json`; intentional strictness changes vs the TS
//! originals are enumerated in contracts/core-api.md.

use crate::CoreError;
use alloy_primitives::{keccak256 as alloy_keccak256, Address, U256};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use sha2::{Digest, Sha256};

/// Keccak-256 (0x01 domain padding — NOT NIST SHA-3).
pub fn keccak256(data: &[u8]) -> Vec<u8> {
    alloy_keccak256(data).to_vec()
}

/// SHA-256 (FIPS 180-4).
pub fn sha256(data: &[u8]) -> Vec<u8> {
    Sha256::digest(data).to_vec()
}

/// Lowercase hex encoding, optionally 0x-prefixed.
pub fn to_hex(data: &[u8], prefixed: bool) -> String {
    let mut out = String::with_capacity(data.len() * 2 + 2);
    if prefixed {
        out.push_str("0x");
    }
    for b in data {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

/// Strict hex decode: optional 0x prefix, mixed case allowed, empty allowed.
/// Rejects odd length and any non-hex character (the TS original silently
/// coerced junk pairs to 0x00 — enumerated divergence).
pub fn from_hex(s: &str) -> Result<Vec<u8>, CoreError> {
    let clean = s.strip_prefix("0x").unwrap_or(s);
    if clean.len() % 2 != 0 {
        return Err(CoreError::InvalidHex(format!(
            "odd length {} in hex string",
            clean.len()
        )));
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    let bytes = clean.as_bytes();
    for i in (0..bytes.len()).step_by(2) {
        let pair = std::str::from_utf8(&bytes[i..i + 2])
            .map_err(|_| CoreError::InvalidHex("non-ascii character".to_owned()))?;
        let b = u8::from_str_radix(pair, 16)
            .map_err(|_| CoreError::InvalidHex(format!("invalid hex pair `{pair}`")))?;
        out.push(b);
    }
    Ok(out)
}

/// Canonical JSON-RPC QUANTITY: 0x-prefixed, lowercase, minimal (no leading
/// zeros; `0x0` for zero).
///
/// This sits on the dApp tx path (`value`/`gas`/`nonce` forwarded to
/// `eth_estimateGas`/`eth_call`), where input is whatever a dApp emitted, so it
/// accepts everything JS `BigInt()` accepted — surrounding whitespace, a
/// leading `+`, and case-insensitive `0x`/`0b`/`0o` radix prefixes — plus the
/// legacy empty/`0x` → `0x0` shortcut several call sites rely on. What it does
/// NOT do is the legacy clamp: negative or unparseable input errors instead of
/// silently becoming `0x0` (enumerated divergence). Above 2^256-1 also errors
/// (unreachable for any real chain quantity).
pub fn to_quantity(value: &str) -> Result<String, CoreError> {
    let trimmed = value.trim_matches(|c: char| c.is_ascii_whitespace());
    let trimmed = trimmed.strip_prefix('+').unwrap_or(trimmed);
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("0x") {
        return Ok("0x0".to_owned());
    }
    let invalid = || CoreError::InvalidQuantity(format!("not a quantity: `{value}`"));
    let (body, radix) = match trimmed.get(..2) {
        Some(p) if p.eq_ignore_ascii_case("0x") => (&trimmed[2..], 16),
        Some(p) if p.eq_ignore_ascii_case("0b") => (&trimmed[2..], 2),
        Some(p) if p.eq_ignore_ascii_case("0o") => (&trimmed[2..], 8),
        _ => (trimmed, 10),
    };
    if body.is_empty() || !body.chars().all(|c| c.is_digit(radix)) {
        return Err(invalid());
    }
    let n = U256::from_str_radix(body, u64::from(radix)).map_err(|_| invalid())?;
    if n.is_zero() {
        return Ok("0x0".to_owned());
    }
    Ok(format!("0x{n:x}"))
}

/// EIP-55 mixed-case checksum. Input may be any case, 0x optional; output is
/// always 0x-prefixed and checksummed. Standard EIP-55 (no EIP-1191 chain id).
pub fn checksum_address(address_hex: &str) -> Result<String, CoreError> {
    Ok(parse_address(address_hex)?.to_checksum(None))
}

/// What an address a person or an agent ENTERED reads as — typed, pasted,
/// scanned, imported or handed over. The one rule every place that takes a
/// recipient refuses by (send-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressEntry {
    /// `0x` and 40 hex digits, in one case or correctly checksummed.
    Valid,
    /// Not `0x` and 40 hex digits at all.
    Malformed,
    /// Mixed case that is not its EIP-55 checksum. The case IS the checksum:
    /// one wrong letter or a tampered copy shows here and nowhere else, so
    /// it is refused, never quietly lowercased into "valid". All-lowercase
    /// and all-uppercase carry no checksum and pass.
    BadChecksum,
}

/// [`AddressEntry`] of `address`, as given (callers trim what a person typed).
pub fn address_entry(address: &str) -> AddressEntry {
    let bytes = address.as_bytes();
    if !(bytes.len() == 42
        && bytes.starts_with(b"0x")
        && bytes[2..].iter().all(u8::is_ascii_hexdigit))
    {
        return AddressEntry::Malformed;
    }
    let body = &bytes[2..];
    let mixed = body.iter().any(u8::is_ascii_uppercase) && body.iter().any(u8::is_ascii_lowercase);
    if !mixed {
        return AddressEntry::Valid;
    }
    match checksum_address(address) {
        Ok(checksummed) if checksummed == address => AddressEntry::Valid,
        _ => AddressEntry::BadChecksum,
    }
}

/// First 4 bytes of keccak256 over the CANONICAL signature (`uint`→`uint256`
/// normalization, named params stripped). Callers passing already-canonical
/// signatures get the same selector the TS original produced.
pub fn function_selector(signature: &str) -> Result<Vec<u8>, CoreError> {
    let f = alloy_json_abi::Function::parse(signature)
        .map_err(|e| CoreError::AbiParse(format!("`{signature}`: {e}")))?;
    Ok(f.selector().to_vec())
}

/// CREATE2: `keccak256(0xff ‖ deployer ‖ salt ‖ init_code_hash)[12..]`,
/// checksummed.
pub fn create2_address(
    deployer_hex: &str,
    salt: &[u8],
    init_code_hash: &[u8],
) -> Result<String, CoreError> {
    let deployer = parse_address(deployer_hex)?;
    let salt: [u8; 32] = salt.try_into().map_err(|_| {
        CoreError::InvalidAddress(format!("salt must be 32 bytes, got {}", salt.len()))
    })?;
    let hash: [u8; 32] = init_code_hash.try_into().map_err(|_| {
        CoreError::InvalidAddress(format!(
            "init_code_hash must be 32 bytes, got {}",
            init_code_hash.len()
        ))
    })?;
    Ok(deployer.create2(salt, hash).to_checksum(None))
}

/// Base64url encode, no padding.
pub fn to_base64url(data: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(data)
}

/// Base64url decode. The url-safe alphabet is REQUIRED — standard-alphabet
/// `+`/`/` are rejected (the TS original accepted them via `atob` — enumerated
/// divergence). Correct `=` padding is tolerated and stripped; EXCESS padding
/// is rejected, matching the legacy decoder (which re-padded to a multiple of
/// four and let `atob` throw). Surrounding whitespace, which `atob` silently
/// ignored, is also rejected (enumerated divergence).
pub fn from_base64url(s: &str) -> Result<Vec<u8>, CoreError> {
    let body = s.trim_end_matches('=');
    let pad = s.len() - body.len();
    // A base64 group is 4 chars; a correctly padded string carries exactly the
    // padding that completes the final group (0, 1 or 2 '=' — never 3+).
    let expected_pad = (4 - body.len() % 4) % 4;
    if pad > expected_pad || pad > 2 {
        return Err(CoreError::InvalidBase64Url(format!(
            "`{s}`: {pad} padding characters, expected at most {expected_pad}"
        )));
    }
    URL_SAFE_NO_PAD
        .decode(body)
        .map_err(|e| CoreError::InvalidBase64Url(format!("`{s}`: {e}")))
}

/// ABI word: address left-padded to 32 bytes.
pub fn abi_encode_address(address_hex: &str) -> Result<Vec<u8>, CoreError> {
    let addr = parse_address(address_hex)?;
    let mut out = vec![0u8; 32];
    out[12..].copy_from_slice(addr.as_slice());
    Ok(out)
}

/// ABI word: uint256 big-endian from a hex string (0x optional — bare strings
/// are interpreted as HEX, mirroring the TS `abiEncodeUint256Hex`).
pub fn abi_encode_uint256(value_hex: &str) -> Result<Vec<u8>, CoreError> {
    let clean = value_hex.strip_prefix("0x").unwrap_or(value_hex);
    if clean.is_empty() || !clean.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(CoreError::InvalidQuantity(format!(
            "not a hex uint256: `{value_hex}`"
        )));
    }
    let n = U256::from_str_radix(clean, 16)
        .map_err(|e| CoreError::InvalidQuantity(format!("`{value_hex}`: {e}")))?;
    Ok(n.to_be_bytes::<32>().to_vec())
}

/// ABI word: raw bytes right-padded to 32 (input ≤ 32 bytes).
pub fn abi_encode_bytes32(data: &[u8]) -> Result<Vec<u8>, CoreError> {
    if data.len() > 32 {
        return Err(CoreError::InvalidHex(format!(
            "bytes32 data too long: {} bytes (max 32)",
            data.len()
        )));
    }
    let mut out = vec![0u8; 32];
    out[..data.len()].copy_from_slice(data);
    Ok(out)
}

/// Shared 20-byte address parser: 0x optional, any case, strict hex.
pub(crate) fn parse_address(address_hex: &str) -> Result<Address, CoreError> {
    let bytes = from_hex(address_hex)
        .map_err(|_| CoreError::InvalidAddress(format!("not hex: `{address_hex}`")))?;
    let arr: [u8; 20] = bytes.as_slice().try_into().map_err(|_| {
        CoreError::InvalidAddress(format!(
            "expected 20 bytes, got {} in `{address_hex}`",
            bytes.len()
        ))
    })?;
    Ok(Address::from(arr))
}
