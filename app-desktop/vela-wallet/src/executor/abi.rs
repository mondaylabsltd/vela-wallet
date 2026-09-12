//! The ABI encoders the read path needs, and the decoders that read the answers
//! back.
//!
//! **Ported from** `src/services/abi.ts` @ `4e98efb4` (FR-006). Same selectors,
//! same layouts, same sanity caps — a divergence here is a wrong number on
//! somebody's home screen, so this is a transcription rather than a redesign.
//!
//! ## Why this is not `vela-core`
//!
//! 031's spec says the encoders "need a Rust home" and points at `vela-core`,
//! where `alloy-dyn-abi` already lives. They are not there, deliberately:
//!
//! - `rust/pkg-web` is a **committed build product** and CI's
//!   `build-web.mjs --check` rebuilds it and compares. Adding a module to
//!   `vela-core` mid-031 means regenerating a cross-client artifact for code
//!   only the desktop calls.
//! - Android and iOS cannot use a `vela-core` module unless it is exported
//!   through uniffi, and that is a bindings + binary-size change spec 033 is
//!   already required to probe before it signs a plan.
//!
//! So this lives where its only caller lives, and promoting it is 033's move,
//! made with 033's size measurement in hand.
//!
//! ## u256 is a decimal string, not a `u128`
//!
//! Every 32-byte word that carries a quantity is decoded to **decimal digits**,
//! not to an integer type. A token with 18 decimals and a large supply reaches
//! 10^39, past `u128::MAX`, and the two ways of coping with that in an integer
//! type — saturating or refusing — are both a wrong balance. The core takes
//! these as strings anyway (`NativeQuoteGroup::amounts_out`), so the string is
//! the honest shape.

/// Multicall3's canonical deployment — the same address on every EVM chain.
pub const MULTICALL3: &str = "0xcA11bde05977b3631167028862bE2a173976CA11";

/// Selectors: the first 4 bytes of `keccak256` over the canonical signature.
/// Constants because they are facts, not computations.
mod sel {
    pub const AGGREGATE3: &str = "82ad56cb"; // aggregate3((address,bool,bytes)[])
    pub const BALANCE_OF: &str = "70a08231"; // balanceOf(address)
    pub const DECIMALS: &str = "313ce567"; // decimals()
    pub const SYMBOL: &str = "95d89b41"; // symbol()
    pub const NAME: &str = "06fdde03"; // name()
    pub const QUOTE_V3: &str = "c6a5026a"; // quoteExactInputSingle((address,address,uint256,uint24,uint160))
    pub const GET_AMOUNTS_OUT: &str = "5509a1ac"; // getAmountsOut(uint256,(address,address,bool,address)[])
    pub const LATEST_ROUND_DATA: &str = "feaf968c"; // latestRoundData()
    pub const ALLOWANCE: &str = "dd62ed3e"; // allowance(address,address)
}

/// One entry of an `aggregate3` batch. `allow_failure` is always true on the
/// read path: one dead pool must not lose the eleven balances batched with it.
pub struct Call3 {
    pub target: String,
    pub call_data: String,
}

/// One `aggregate3` answer. `data` is `0x` when the call reverted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McResult {
    pub success: bool,
    pub data: String,
}

// ---------------------------------------------------------------------------
// Hex helpers
// ---------------------------------------------------------------------------

fn body(hex: &str) -> &str {
    hex.strip_prefix("0x")
        .or(hex.strip_prefix("0X"))
        .unwrap_or(hex)
}

/// A `u128` as a left-padded 32-byte word.
fn word(value: u128) -> String {
    format!("{value:064x}")
}

/// An address as a left-padded 32-byte word.
fn addr(address: &str) -> String {
    let clean = body(address).to_lowercase();
    format!("{clean:0>64}")
}

/// Right-pad call data to a 32-byte boundary.
fn pad_right(hex: &str) -> String {
    let clean = body(hex);
    let target = clean.len().div_ceil(64) * 64;
    format!("{clean:0<target$}")
}

/// The 32-byte word at hex-char position `pos`, as `u64`.
///
/// `None` when the word does not fit — an offset or length that needs more than
/// 64 bits is a malformed answer, not a very large array, and the web's
/// `Number(BigInt(...))` would have silently produced a garbage index.
fn word_u64(hex: &str, pos: usize) -> Option<u64> {
    let slice = hex.get(pos..pos.checked_add(64)?)?;
    if !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // The high 48 hex chars must be zero for the value to fit in u64.
    if slice[..48].bytes().any(|b| b != b'0') {
        return None;
    }
    u64::from_str_radix(&slice[48..], 16).ok()
}

/// Hex digits of a 32-byte word, as decimal digits.
///
/// Schoolbook base-2^16 division, because the value can exceed every integer
/// type Rust has. Returns `None` for a slice that is not 64 hex characters.
fn word_decimal(slice: &str) -> Option<String> {
    if slice.len() != 64 || !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let mut limbs: Vec<u32> = (0..16)
        .map(|i| u32::from_str_radix(&slice[i * 4..i * 4 + 4], 16).unwrap_or(0))
        .collect();
    let mut digits = Vec::new();
    while limbs.iter().any(|limb| *limb != 0) {
        let mut carry: u32 = 0;
        for limb in &mut limbs {
            let value = (carry << 16) | *limb;
            *limb = value / 10;
            carry = value % 10;
        }
        digits.push(b'0' + u8::try_from(carry).unwrap_or(0));
    }
    if digits.is_empty() {
        return Some("0".to_owned());
    }
    digits.reverse();
    String::from_utf8(digits).ok()
}

// ---------------------------------------------------------------------------
// Multicall3
// ---------------------------------------------------------------------------

/// ABI-encode `aggregate3(Call3[])`.
///
/// `Call3 = (address target, bool allowFailure, bytes callData)`. The tuple
/// holds a dynamic member, so every element needs an offset pointer and the
/// element bodies follow the pointer block.
#[must_use]
pub fn enc_aggregate3(calls: &[Call3]) -> String {
    let mut out = String::from(sel::AGGREGATE3);
    out.push_str(&word(0x20)); // offset to the one dynamic param
    out.push_str(&word(calls.len() as u128));

    let elements: Vec<String> = calls
        .iter()
        .map(|call| {
            let data = body(&call.call_data);
            let mut element = addr(&call.target);
            element.push_str(&word(1)); // allowFailure — always, see Call3
            element.push_str(&word(0x60)); // offset to the bytes member
            element.push_str(&word((data.len() / 2) as u128));
            element.push_str(&pad_right(data));
            element
        })
        .collect();

    let mut offset = (calls.len() * 32) as u128;
    for element in &elements {
        out.push_str(&word(offset));
        offset += (element.len() / 2) as u128;
    }
    for element in &elements {
        out.push_str(element);
    }
    format!("0x{out}")
}

/// ABI-decode `aggregate3 -> (bool success, bytes returnData)[]`.
///
/// Every bound is checked and every failure is an empty vector or a
/// `success: false` row — never a panic. This decodes bytes an endpoint we do
/// not control chose to send.
#[must_use]
pub fn dec_aggregate3(hex: &str) -> Vec<McResult> {
    let data = body(hex);
    if data.len() < 128 {
        return Vec::new();
    }
    let Some(array_offset) = word_u64(data, 0).and_then(|v| usize::try_from(v).ok()) else {
        return Vec::new();
    };
    let Some(array_at) = array_offset.checked_mul(2) else {
        return Vec::new();
    };
    if array_at.saturating_add(64) > data.len() {
        return Vec::new();
    }
    let Some(len) = word_u64(data, array_at).and_then(|v| usize::try_from(v).ok()) else {
        return Vec::new();
    };
    // The same sanity cap the web carries. An answer claiming ten thousand
    // results is a malformed answer, and trusting it is a very long loop.
    if len == 0 || len > 10_000 {
        return Vec::new();
    }
    let offsets_at = array_at + 64;

    let mut out = Vec::with_capacity(len);
    for i in 0..len {
        let Some(offset_pos) = offsets_at.checked_add(i * 64) else {
            break;
        };
        if offset_pos + 64 > data.len() {
            break;
        }
        let Some(element_at) = word_u64(data, offset_pos)
            .and_then(|v| usize::try_from(v).ok())
            .and_then(|v| v.checked_mul(2))
            .and_then(|v| offsets_at.checked_add(v))
        else {
            break;
        };
        if element_at + 128 > data.len() {
            out.push(McResult {
                success: false,
                data: "0x".to_owned(),
            });
            continue;
        }
        let success = word_u64(data, element_at).is_some_and(|v| v != 0);
        let Some(bytes_at) = word_u64(data, element_at + 64)
            .and_then(|v| usize::try_from(v).ok())
            .and_then(|v| v.checked_mul(2))
            .and_then(|v| element_at.checked_add(v))
        else {
            out.push(McResult {
                success,
                data: "0x".to_owned(),
            });
            continue;
        };
        if bytes_at + 64 > data.len() {
            out.push(McResult {
                success,
                data: "0x".to_owned(),
            });
            continue;
        }
        let byte_len = word_u64(data, bytes_at)
            .and_then(|v| usize::try_from(v).ok())
            .unwrap_or(0);
        let end = bytes_at
            .checked_add(64)
            .and_then(|start| start.checked_add(byte_len.checked_mul(2)?));
        let payload = match end {
            Some(end) if byte_len > 0 && end <= data.len() => {
                format!("0x{}", &data[bytes_at + 64..end])
            }
            _ => "0x".to_owned(),
        };
        out.push(McResult {
            success,
            data: payload,
        });
    }
    out
}

// ---------------------------------------------------------------------------
// Calls
// ---------------------------------------------------------------------------

/// `balanceOf(address)`.
#[must_use]
pub fn enc_balance_of(address: &str) -> String {
    format!("0x{}{}", sel::BALANCE_OF, addr(address))
}

/// `allowance(owner, spender)` — what the spender may ALREADY move.
///
/// `increaseAllowance` is an increment, not a replacement, so the sheet cannot
/// say what the approval will come to without this: the resulting total is
/// existing + increment, and showing only the increment understates what the
/// person is agreeing to.
#[must_use]
pub fn enc_allowance(owner: &str, spender: &str) -> String {
    format!("0x{}{}{}", sel::ALLOWANCE, addr(owner), addr(spender))
}

/// `decimals()`.
///
/// Multicall3's own `getEthBalance(address)` is deliberately NOT here. The
/// native coin is read by `eth_getBalance` instead, because a failed batch and
/// an unreachable chain are different verdicts and only one of them may reach
/// `failed_chain_ids` — see `balances.rs`'s header.
#[must_use]
pub fn enc_decimals() -> String {
    format!("0x{}", sel::DECIMALS)
}

/// `symbol()`.
#[must_use]
pub fn enc_symbol() -> String {
    format!("0x{}", sel::SYMBOL)
}

/// `name()`.
#[must_use]
pub fn enc_name() -> String {
    format!("0x{}", sel::NAME)
}

/// Chainlink `latestRoundData()`.
#[must_use]
pub fn enc_latest_round() -> String {
    format!("0x{}", sel::LATEST_ROUND_DATA)
}

/// Uniswap V3 `quoteExactInputSingle((tokenIn,tokenOut,amountIn,fee,limit))`.
///
/// A static tuple, so the encoding is the fields concatenated.
#[must_use]
pub fn enc_quote_v3(token_in: &str, token_out: &str, amount_in: u128, fee: u32) -> String {
    format!(
        "0x{}{}{}{}{}{}",
        sel::QUOTE_V3,
        addr(token_in),
        addr(token_out),
        word(amount_in),
        word(u128::from(fee)),
        word(0), // sqrtPriceLimitX96
    )
}

/// Aerodrome / Velodrome V2 `getAmountsOut(uint256, Route[])`, single hop.
///
/// `Route = (address from, address to, bool stable, address factory)`, with a
/// zero factory meaning the router's default.
#[must_use]
pub fn enc_get_amounts_out(
    amount_in: u128,
    token_in: &str,
    token_out: &str,
    stable: bool,
) -> String {
    format!(
        "0x{}{}{}{}{}{}{}{}",
        sel::GET_AMOUNTS_OUT,
        word(amount_in),
        word(0x40), // offset to Route[]
        word(1),    // routes.length
        addr(token_in),
        addr(token_out),
        word(u128::from(stable)),
        word(0), // factory = default
    )
}

// ---------------------------------------------------------------------------
// Decoders
// ---------------------------------------------------------------------------

/// The first word as decimal digits. `None` for a short or malformed answer,
/// which is a different thing from `"0"`.
#[must_use]
pub fn dec_u256(hex: &str) -> Option<String> {
    let data = body(hex);
    word_decimal(data.get(..64)?)
}

/// The first word as `uint8` — `decimals()`'s shape.
#[must_use]
pub fn dec_u8(hex: &str) -> Option<u8> {
    let data = body(hex);
    let slice = data.get(..64)?;
    if !slice.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // Anything above the last byte means this is not a uint8.
    if slice[..62].bytes().any(|b| b != b'0') {
        return None;
    }
    u8::from_str_radix(&slice[62..], 16).ok()
}

/// The last element of `getAmountsOut -> uint256[]`, as decimal digits.
///
/// A single-hop route answers `[amountIn, amountOut]`, so the second word of
/// the array is the quote.
#[must_use]
pub fn dec_amounts_out(hex: &str) -> Option<String> {
    let data = body(hex);
    word_decimal(data.get(192..256)?)
}

/// Chainlink `latestRoundData()`'s `answer`, scaled by the 8 decimals every
/// USD feed uses.
///
/// A negative answer decodes to `None` rather than a negative price: the feeds
/// this reads are USD prices, and a negative one means the word was not what we
/// think it is.
#[must_use]
pub fn dec_chainlink_usd(hex: &str) -> Option<f64> {
    let data = body(hex);
    let slice = data.get(64..128)?;
    // int256 sign bit — a real USD feed never sets it.
    if slice.starts_with([
        '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'A', 'B', 'C', 'D', 'E', 'F',
    ]) {
        return None;
    }
    let decimal = word_decimal(slice)?;
    let value: f64 = decimal.parse().ok()?;
    let usd = value / 1e8;
    (usd.is_finite() && usd > 0.0).then_some(usd)
}

/// An `eth_getBalance`-style hex QUANTITY as decimal digits.
///
/// A quantity is minimally encoded (`0xab12`, not a padded word), which is why
/// this is not [`dec_u256`]: left-padding first is the whole difference.
#[must_use]
pub fn dec_hex_quantity(hex: &str) -> Option<String> {
    let clean = body(hex);
    if clean.is_empty() || clean.len() > 64 {
        return None;
    }
    word_decimal(&format!("{clean:0>64}"))
}

/// An ABI `string` return — `name()` and `symbol()`.
///
/// Two layouts, because ERC-20 predates the convention: the usual
/// `[offset][length][data]`, and the **bytes32** a legacy token (MKR and its
/// generation) returns as a single fixed word. A declared length that does not
/// fit the payload is read as the second shape rather than trusted, because an
/// out-of-range length is what a bytes32 answer looks like to an offset reader.
///
/// Decoded as UTF-8 so a multibyte symbol (`USD₮0`) survives, and `None` rather
/// than a lossy replacement, because a mojibake symbol saved into somebody's
/// token list is there forever.
#[must_use]
pub fn dec_string(hex: &str) -> Option<String> {
    let data = body(hex);
    if data.len() < 64 {
        return None;
    }
    // A single word with no header: bytes32.
    if data.len() < 128 {
        return utf8_from_hex(data.get(..64)?);
    }
    let length = word_u64(data, 64).and_then(|v| usize::try_from(v).ok())?;
    let end = length.checked_mul(2).and_then(|len| len.checked_add(128));
    match end {
        Some(end) if length > 0 && length <= 4096 && end <= data.len() => {
            utf8_from_hex(data.get(128..end)?)
        }
        // Not offset-encoded after all — read the head as bytes32.
        _ => utf8_from_hex(data.get(..64)?),
    }
}

/// Hex bytes as UTF-8, stopping at the first NUL (a bytes32 answer is
/// zero-padded, and a C-string terminator is not part of the name).
fn utf8_from_hex(hex: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    for i in 0..hex.len() / 2 {
        let byte = u8::from_str_radix(hex.get(i * 2..i * 2 + 2)?, 16).ok()?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    let text = String::from_utf8(bytes).ok()?;
    let text = text.trim().to_owned();
    (!text.is_empty()).then_some(text)
}

/// Raw integer digits and a scale, as the human amount the core reads.
///
/// **This is the shape `BalanceToken::balance` has**, and it is not obvious:
/// the field holds a *decimal* amount, because `token_balance_double` parses it
/// with a fractional part and multiplies it by `price_usd`. Handing it raw base
/// units would multiply every holding by 10^decimals — invisible while prices
/// are `None` (anything times zero is zero) and a wildly wrong total the moment
/// one arrives.
#[must_use]
pub fn format_raw_balance(raw: &str, decimals: u32) -> String {
    let digits: String = raw.trim_start_matches('0').to_owned();
    if digits.is_empty() {
        return "0".to_owned();
    }
    let decimals = decimals as usize;
    if decimals == 0 {
        return digits;
    }
    let (whole, fraction) = if digits.len() <= decimals {
        ("0".to_owned(), format!("{digits:0>decimals$}"))
    } else {
        (
            digits[..digits.len() - decimals].to_owned(),
            digits[digits.len() - decimals..].to_owned(),
        )
    };
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole
    } else {
        format!("{whole}.{fraction}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The batch encoding, against a hand-checked layout.
    ///
    /// One call with 4 bytes of data: selector, head offset, length, then the
    /// element pointer and the element itself.
    #[test]
    fn one_call_encodes_to_the_layout_multicall3_expects() {
        let encoded = enc_aggregate3(&[Call3 {
            target: "0xcA11bde05977b3631167028862bE2a173976CA11".to_owned(),
            call_data: "0x313ce567".to_owned(),
        }]);
        let data = body(&encoded);
        assert_eq!(&data[..8], sel::AGGREGATE3);
        // head: offset 0x20, then length 1.
        assert_eq!(word_u64(data, 8), Some(0x20));
        assert_eq!(word_u64(data, 8 + 64), Some(1));
        // one element pointer, 32 bytes past the pointer block.
        assert_eq!(word_u64(data, 8 + 128), Some(32));
        // the element: target, allowFailure = 1, bytes offset 0x60, length 4.
        let element = 8 + 192;
        assert!(data[element..element + 64].ends_with("ca11bde05977b3631167028862be2a173976ca11"));
        assert_eq!(word_u64(data, element + 64), Some(1));
        assert_eq!(word_u64(data, element + 128), Some(0x60));
        assert_eq!(word_u64(data, element + 192), Some(4));
        assert!(data[element + 256..].starts_with("313ce567"));
    }

    /// A round trip through a real `aggregate3` answer shape: two results, one
    /// successful with a 32-byte word, one reverted with empty bytes.
    #[test]
    fn the_batch_answer_decodes_success_and_revert_apart() {
        let w = |v: u128| word(v);
        // offset to array, length 2, two element pointers, two elements.
        let element_ok = format!("{}{}{}{}", w(1), w(0x40), w(32), w(1_000_000));
        let element_bad = format!("{}{}{}", w(0), w(0x40), w(0));
        let pointers = format!("{}{}", w(64), w(64 + (element_ok.len() / 2) as u128));
        let encoded = format!(
            "0x{}{}{}{}{}",
            w(0x20),
            w(2),
            pointers,
            element_ok,
            element_bad
        );

        let results = dec_aggregate3(&encoded);
        assert_eq!(results.len(), 2);
        assert!(results[0].success);
        assert_eq!(dec_u256(&results[0].data).as_deref(), Some("1000000"));
        assert!(!results[1].success);
        assert_eq!(results[1].data, "0x");
    }

    /// Bytes an endpoint we do not control chose to send must not panic.
    #[test]
    fn malformed_answers_decode_to_nothing_rather_than_panicking() {
        for hex in [
            "",
            "0x",
            "0xdeadbeef",
            // an array offset past the end of the payload
            &format!("0x{}{}", word(0xffff), word(2)),
            // a length claiming more results than the cap allows
            &format!("0x{}{}", word(0x20), word(99_999)),
            // odd-length body
            "0x123",
        ] {
            assert!(dec_aggregate3(hex).is_empty(), "{hex} decoded to results");
        }
    }

    /// A quantity past `u128::MAX` survives, because refusing it or saturating
    /// it are both a wrong balance.
    #[test]
    fn a_quantity_larger_than_u128_decodes_exactly() {
        // 2^255, which no integer type in this program can hold.
        let hex = format!("0x8{}", "0".repeat(63));
        let expected =
            "57896044618658097711785492504343953926634992332820282019728792003956564819968";
        assert_eq!(dec_u256(&hex).as_deref(), Some(expected));
    }

    /// `decimals()` is a `uint8` and a word that is not one answers `None` —
    /// a garbage scale is worse than an absent one, because it renders.
    #[test]
    fn decimals_reads_the_last_byte_and_refuses_anything_wider() {
        assert_eq!(dec_u8(&format!("0x{}", word(6))), Some(6));
        assert_eq!(dec_u8(&format!("0x{}", word(18))), Some(18));
        assert_eq!(dec_u8(&format!("0x{}", word(256))), None);
        assert_eq!(dec_u8("0x"), None);
    }

    /// The Chainlink feed's second word, scaled by 8, and the two shapes that
    /// must not price.
    #[test]
    fn a_chainlink_answer_prices_and_a_negative_one_does_not() {
        // roundId, answer = 3_012.34 * 1e8, then the rest.
        let hex = format!("0x{}{}{}", word(1), word(301_234_000_000), word(0));
        let price = dec_chainlink_usd(&hex).unwrap_or_else(|| unreachable!("no price"));
        assert!((price - 3_012.34).abs() < 1e-6, "{price}");

        // A negative int256 answer: the sign bit set.
        let negative = format!("0x{}{}{}", word(1), "f".repeat(64), word(0));
        assert_eq!(dec_chainlink_usd(&negative), None);
        // Zero is not a price either.
        assert_eq!(
            dec_chainlink_usd(&format!("0x{}{}", word(1), word(0))),
            None
        );
        assert_eq!(dec_chainlink_usd("0x"), None);
    }

    /// The field the core actually reads.
    ///
    /// `token_balance_double` parses this with a fractional part, so raw base
    /// units here multiply the holding by 10^decimals.
    #[test]
    fn a_raw_quantity_becomes_the_human_amount_the_core_multiplies_by_a_price() {
        assert_eq!(format_raw_balance("769970000000000000", 18), "0.76997");
        assert_eq!(format_raw_balance("1000000", 6), "1");
        assert_eq!(format_raw_balance("1500000", 6), "1.5");
        assert_eq!(format_raw_balance("0", 18), "0");
        assert_eq!(format_raw_balance("1", 18), "0.000000000000000001");
        assert_eq!(format_raw_balance("42", 0), "42");
        // What the core does with it, which is the reason for the shape.
        let human = format_raw_balance("769970000000000000", 18);
        let value = vela_core::app::balance_dashboard::token_balance_double(&human);
        assert!((value - 0.769_97).abs() < 1e-12, "{value}");
    }

    /// Both `string` shapes, and the answers that must not become a name.
    #[test]
    fn a_symbol_decodes_from_either_layout_and_refuses_the_rest() {
        // The usual layout: offset 0x20, length 4, "USDC".
        let usdc = format!("0x{}{}{:0<64}", word(0x20), word(4), "55534443");
        assert_eq!(dec_string(&usdc).as_deref(), Some("USDC"));

        // Legacy bytes32 — one word, zero-padded. "MKR".
        let mkr = format!("0x{:0<64}", "4d4b52");
        assert_eq!(dec_string(&mkr).as_deref(), Some("MKR"));

        // A multibyte symbol must survive intact: "USD₮0".
        let tether = format!("0x{}{}{:0<64}", word(0x20), word(7), "555344e282ae30");
        assert_eq!(dec_string(&tether).as_deref(), Some("USD\u{20ae}0"));

        // A length past the payload is read as bytes32, not trusted.
        let lying = format!("0x{:0<64}{}", "4d4b52", word(9_999));
        assert_eq!(dec_string(&lying).as_deref(), Some("MKR"));

        // Nothing that could be saved as a garbage symbol.
        assert_eq!(dec_string("0x"), None);
        assert_eq!(dec_string("0xzz"), None);
        assert_eq!(dec_string(&format!("0x{}", word(0))), None, "all padding");
        // Invalid UTF-8 must not become a replacement character.
        let invalid = format!("0x{}{}{:0<64}", word(0x20), word(2), "fffe");
        assert_eq!(dec_string(&invalid), None);
    }

    /// `eth_getBalance` answers a minimal quantity, not a padded word.
    #[test]
    fn a_hex_quantity_decodes_without_being_a_full_word() {
        assert_eq!(dec_hex_quantity("0x0").as_deref(), Some("0"));
        assert_eq!(dec_hex_quantity("0xab12").as_deref(), Some("43794"));
        assert_eq!(
            dec_hex_quantity("0xaaf96eb9d0d0000").as_deref(),
            Some("770000000000000000")
        );
        assert_eq!(dec_hex_quantity("0x").as_deref(), None);
        assert_eq!(dec_hex_quantity("0xzz").as_deref(), None);
    }

    /// A single-hop `getAmountsOut` answers `[amountIn, amountOut]`.
    #[test]
    fn a_route_quote_reads_the_output_not_the_input() {
        let hex = format!(
            "0x{}{}{}{}",
            word(0x20),
            word(2),
            word(1_000_000_000_000_000_000),
            word(3_012_340_000)
        );
        assert_eq!(dec_amounts_out(&hex).as_deref(), Some("3012340000"));
        assert_eq!(dec_amounts_out("0x").as_deref(), None);
    }

    /// The quote encodings, against their selectors and argument order.
    #[test]
    fn the_quote_encodings_carry_their_selector_and_arguments() {
        let v3 = enc_quote_v3("0xaaa", "0xbbb", 1_000_000, 3_000);
        assert!(v3.starts_with(&format!("0x{}", sel::QUOTE_V3)));
        assert_eq!(body(&v3).len(), 8 + 5 * 64);

        let solidly = enc_get_amounts_out(1_000_000, "0xaaa", "0xbbb", true);
        assert!(solidly.starts_with(&format!("0x{}", sel::GET_AMOUNTS_OUT)));
        assert_eq!(body(&solidly).len(), 8 + 7 * 64);
        // The `stable` flag is the 6th word and must be the one that moved.
        let volatile = enc_get_amounts_out(1_000_000, "0xaaa", "0xbbb", false);
        assert_ne!(solidly, volatile);
    }
}
