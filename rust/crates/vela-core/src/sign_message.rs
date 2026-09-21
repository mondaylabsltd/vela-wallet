//! What a site's message request asks the account to sign — the ONE hash
//! every shell and the Clear Signer's page (`lib/digest.js`) derive before the
//! Safe's own `SafeMessage` wrap (`user_op::compute_safe_message_hash`).
//!
//! Three shells each carried a copy of this, and two had drifted: they read
//! typed data from `params[1]` always, where `eth_signTypedData` /
//! `eth_signTypedData_v1` carry it FIRST — the pick
//! `app::sign_request::extract_request_chain_id` (and the page) use. A drift
//! here is a signature over something the site never asked for, or a Clear
//! Signer answer refused as the wrong challenge.

use serde_json::Value;

/// The methods signed as a message rather than submitted.
#[must_use]
pub fn is_message_method(method: &str) -> bool {
    method == "personal_sign" || method == "eth_sign" || method.contains("signTypedData")
}

/// The request's own hash: `personal_sign` / `eth_sign` are the EIP-191
/// envelope over their bytes (hex when the payload is hex, its UTF-8 text
/// otherwise; `eth_sign` is `[address, data]`, EIP-1474), typed data is its
/// EIP-712 digest. `None` when there is nothing to sign.
#[must_use]
pub fn original_hash(method: &str, params_json: &str) -> Option<Vec<u8>> {
    let params: Value = serde_json::from_str(params_json).ok()?;
    let params = params.as_array()?;
    if method == "personal_sign" || method == "eth_sign" {
        let payload = params
            .get(usize::from(method == "eth_sign"))?
            .as_str()
            .filter(|payload| !payload.is_empty())?;
        let bytes = match payload.strip_prefix("0x") {
            Some(hex) if hex.len() % 2 == 0 && hex.bytes().all(|b| b.is_ascii_hexdigit()) => {
                crate::primitives::from_hex(payload).ok()?
            }
            _ => payload.as_bytes().to_vec(),
        };
        let mut preimage = format!("\u{19}Ethereum Signed Message:\n{}", bytes.len()).into_bytes();
        preimage.extend_from_slice(&bytes);
        return Some(crate::primitives::keccak256(&preimage));
    }
    if !method.contains("signTypedData") {
        return None;
    }
    let legacy = method == "eth_signTypedData" || method == "eth_signTypedData_v1";
    let raw = if legacy {
        params.first()
    } else {
        params
            .get(1)
            .filter(|data| !data.is_null())
            .or(params.first())
    }?;
    let json = raw.as_str().map_or_else(|| raw.to_string(), str::to_owned);
    crate::eip712::hash_typed_data(&json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TYPED: &str = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"},{"name":"chainId","type":"uint256"}],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Ether Mail","chainId":1},"message":{"contents":"Hello"}}"#;

    #[test]
    fn personal_sign_is_eip191_over_hex_or_text() {
        let hex = original_hash("personal_sign", r#"["0x68656c6c6f","0xabc"]"#);
        let text = original_hash("personal_sign", r#"["hello","0xabc"]"#);
        assert_eq!(hex, text, "0x68656c6c6f is the bytes of \"hello\"");
        let mut preimage = b"\x19Ethereum Signed Message:\n5".to_vec();
        preimage.extend_from_slice(b"hello");
        assert_eq!(hex, Some(crate::primitives::keccak256(&preimage)));
        assert_eq!(
            original_hash("eth_sign", r#"["0xabc","0x68656c6c6f"]"#),
            hex,
            "eth_sign swaps the params"
        );
        assert_eq!(original_hash("personal_sign", r#"[""]"#), None);
    }

    #[test]
    fn typed_data_is_picked_where_each_method_carries_it() {
        let expected = crate::eip712::hash_typed_data(TYPED).ok();
        assert!(expected.is_some());
        let as_string = serde_json::to_string(TYPED).unwrap_or_default();
        let v4 = format!(r#"["0xabc",{as_string}]"#);
        assert_eq!(original_hash("eth_signTypedData_v4", &v4), expected);
        let v4_object = format!(r#"["0xabc",{TYPED}]"#);
        assert_eq!(original_hash("eth_signTypedData_v4", &v4_object), expected);
        let legacy = format!(r#"[{as_string},"0xabc"]"#);
        assert_eq!(
            original_hash("eth_signTypedData", &legacy),
            expected,
            "the legacy name carries the data first"
        );
        assert_eq!(original_hash("eth_signTypedData_v1", &legacy), expected);
        let missing = format!(r#"[{as_string},null]"#);
        assert_eq!(
            original_hash("eth_signTypedData_v3", &missing),
            expected,
            "a missing second falls back to the first"
        );
        assert_eq!(original_hash("eth_sendTransaction", "[{}]"), None);
    }
}
