//! `ethereum:` payment URIs, tokenized.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/eip681.ts`
//! (`parseEIP681`, `parseAmount`), whose own header explains why the grammar
//! lives in the shells: Hermes has no WebAssembly, so the native tier must run
//! its own parser and a core one could only ever be a second implementation.
//! This is the desktop's — the third — and it keeps the two refusals that were
//! bought with real bugs.
//!
//! This file only TOKENIZES. Whether a scan locks the send screen, whether a
//! chainless request may lock at all, and how base units become a figure are
//! all `send.rs`'s (`scan_resolved` / `resolve_locked_request`).

/// A parsed request, in the shape the core's `SendScan::Request` takes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Eip681 {
    pub recipient: String,
    pub chain_id: Option<u32>,
    /// `None` = the chain's own coin.
    pub token_address: Option<String>,
    /// Base units, as a decimal string — the wire shape, so no u128 has to
    /// survive a JSON boundary it may not fit through.
    pub amount_base_units: Option<String>,
}

#[must_use]
pub fn is_hex_address(s: &str) -> bool {
    let s = s.trim();
    s.len() == 42 && s.starts_with("0x") && s[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// `ethereum:<target>[@chain][/function][?params]`, tolerantly — the legacy
/// `pay-` prefix and scientific-notation amounts included.
///
/// `None` means "this is not a payment request", and every caller already has
/// a path for that: the scanner explains, the send screen drops the raw text
/// into an editable recipient field.
#[must_use]
pub fn parse(input: &str) -> Option<Eip681> {
    let trimmed = input.trim();
    let rest = trimmed
        .get(..9)
        .filter(|prefix| prefix.eq_ignore_ascii_case("ethereum:"))
        .map(|_| &trimmed[9..])?;
    let rest = rest.strip_prefix("pay-").unwrap_or(rest);

    let (path, query) = match rest.find('?') {
        Some(at) => (&rest[..at], &rest[at + 1..]),
        None => (rest, ""),
    };
    let params = parse_query(query);

    let (target_with_chain, function_name) = match path.find('/') {
        Some(at) => (&path[..at], &path[at + 1..]),
        None => (path, ""),
    };
    let (target, chain_str) = match target_with_chain.find('@') {
        Some(at) => (target_with_chain[..at].trim(), &target_with_chain[at + 1..]),
        None => (target_with_chain.trim(), ""),
    };
    if target.is_empty() {
        return None;
    }
    let chain_id = (!chain_str.is_empty() && chain_str.bytes().all(|b| b.is_ascii_digit()))
        .then(|| chain_str.parse::<u32>().ok())
        .flatten();

    if function_name == "transfer" {
        let recipient = params
            .iter()
            .find(|(key, _)| key == "address")
            .map(|(_, value)| value.trim().to_owned())
            .unwrap_or_default();
        if !is_hex_address(target) || !is_hex_address(&recipient) {
            return None;
        }
        // `uint256` ONLY. In a `/transfer` URI, EIP-681's `value` is the ether
        // sent ALONG WITH the call, not the token argument — reading it as
        // token base units mixed two units and two decimals, and a URI saying
        // "attach 1 ETH" (value=1e18) prefilled a LOCKED send of 10^12 USDC.
        let amount = params
            .iter()
            .find(|(key, _)| key == "uint256")
            .and_then(|(_, value)| parse_amount(value));
        return Some(Eip681 {
            recipient,
            chain_id,
            token_address: Some(target.to_owned()),
            amount_base_units: amount,
        });
    }

    // Any OTHER function is not a payment this can honour. It used to fall
    // through to the native branch, where `target` — the contract the call was
    // addressed to — became the RECIPIENT: `ethereum:<token>@1/approve?…`
    // opened a locked send of the chain's coin to the token contract, which is
    // a burn dressed as the payment somebody thought they scanned.
    if !function_name.is_empty() {
        return None;
    }

    if !is_hex_address(target) {
        return None;
    }
    Some(Eip681 {
        recipient: target.to_owned(),
        chain_id,
        token_address: None,
        amount_base_units: params
            .iter()
            .find(|(key, _)| key == "value")
            .and_then(|(_, value)| parse_amount(value)),
    })
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| match pair.find('=') {
            Some(at) => (
                decode_component(&pair[..at]),
                decode_component(&pair[at + 1..]),
            ),
            None => (decode_component(pair), String::new()),
        })
        .collect()
}

/// `decodeURIComponent`, for the parts of it a payment URI can carry.
fn decode_component(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|hex| u8::from_str_radix(hex, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// An integer amount, accepting decimals and scientific notation, as a decimal
/// STRING.
///
/// A negative amount answers `None` rather than zero: a request for minus one
/// coin is not a request for none of them, it is a request this cannot honour.
fn parse_amount(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }
    let (sign, rest) = match raw.as_bytes()[0] {
        b'+' => (1i32, &raw[1..]),
        b'-' => (-1, &raw[1..]),
        _ => (1, raw),
    };
    if sign < 0 {
        return None;
    }
    let (mantissa, exp) = match rest.find(['e', 'E']) {
        Some(at) => (&rest[..at], rest[at + 1..].parse::<i32>().ok()?),
        None => (rest, 0),
    };
    let (int_digits, frac_digits) = match mantissa.find('.') {
        Some(at) => (&mantissa[..at], &mantissa[at + 1..]),
        None => (mantissa, ""),
    };
    if int_digits.is_empty() && frac_digits.is_empty() {
        return None;
    }
    if !int_digits
        .bytes()
        .chain(frac_digits.bytes())
        .all(|b| b.is_ascii_digit())
    {
        return None;
    }
    let digits: String = format!("{int_digits}{frac_digits}");
    let decimal_exp = exp - i32::try_from(frac_digits.len()).ok()?;
    let value = digits.trim_start_matches('0');
    let value = if value.is_empty() { "0" } else { value };

    if decimal_exp >= 0 {
        let zeros = usize::try_from(decimal_exp).ok()?;
        // A cap, not a policy: an exponent nobody could mean is refused rather
        // than turned into a megabyte of zeros.
        if zeros > 78 {
            return None;
        }
        return Some(if value == "0" {
            "0".to_owned()
        } else {
            format!("{value}{}", "0".repeat(zeros))
        });
    }
    // Negative exponent: truncate, because base units are integral.
    let drop = usize::try_from(-decimal_exp).ok()?;
    if drop >= value.len() {
        return Some("0".to_owned());
    }
    Some(value[..value.len() - drop].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const TOKEN: &str = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83";

    /// The four canonical forms this app itself writes.
    #[test]
    fn the_shapes_this_app_writes_read_back() {
        let native = parse(&format!("ethereum:{ME}@100"))
            .unwrap_or_else(|| unreachable!("a native request"));
        assert_eq!(native.recipient, ME);
        assert_eq!(native.chain_id, Some(100));
        assert!(native.token_address.is_none());
        assert!(native.amount_base_units.is_none());

        let with_value = parse(&format!("ethereum:{ME}@100?value=1500000000000000000"))
            .unwrap_or_else(|| unreachable!("a native request with an amount"));
        assert_eq!(
            with_value.amount_base_units.as_deref(),
            Some("1500000000000000000")
        );

        let token = parse(&format!("ethereum:{TOKEN}@100/transfer?address={ME}"))
            .unwrap_or_else(|| unreachable!("a token request"));
        assert_eq!(token.recipient, ME);
        assert_eq!(token.token_address.as_deref(), Some(TOKEN));
        assert!(token.amount_base_units.is_none());

        let token_amount = parse(&format!(
            "ethereum:{TOKEN}@100/transfer?address={ME}&uint256=1500000"
        ))
        .unwrap_or_else(|| unreachable!("a token request with an amount"));
        assert_eq!(token_amount.amount_base_units.as_deref(), Some("1500000"));
    }

    /// The legacy `pay-` prefix, and the scientific notation other wallets
    /// emit — both accepted, because a person scanning a code did not choose
    /// which wallet wrote it.
    #[test]
    fn the_tolerant_forms_are_accepted() {
        let legacy = parse(&format!("ethereum:pay-{ME}@1?value=2.014e18"))
            .unwrap_or_else(|| unreachable!("a legacy request"));
        assert_eq!(legacy.recipient, ME);
        assert_eq!(
            legacy.amount_base_units.as_deref(),
            Some("2014000000000000000")
        );

        // Upper case scheme, and a chainless request (the core decides whether
        // a chainless one may lock).
        let bare =
            parse(&format!("ETHEREUM:{ME}")).unwrap_or_else(|| unreachable!("a chainless request"));
        assert!(bare.chain_id.is_none());
    }

    /// The two refusals that were bought with real bugs.
    #[test]
    fn a_call_that_is_not_a_payment_is_refused() {
        // `approve` used to fall through to the native branch, where the TOKEN
        // became the recipient: a locked send of the chain's coin to the
        // contract — a burn dressed as the payment somebody scanned.
        assert!(
            parse(&format!(
                "ethereum:{TOKEN}@1/approve?address={ME}&uint256=1"
            ))
            .is_none()
        );

        // In a `/transfer`, `value` is ether sent ALONG WITH the call, not the
        // token argument. Reading it as base units priced a "attach 1 ETH"
        // request as 10^12 USDC.
        let transfer = parse(&format!(
            "ethereum:{TOKEN}@1/transfer?address={ME}&value=1000000000000000000"
        ))
        .unwrap_or_else(|| unreachable!("a token request"));
        assert!(
            transfer.amount_base_units.is_none(),
            "value is not the token amount"
        );
    }

    /// Everything that is not a request answers `None` — and the callers all
    /// have a path for that, so refusing is not a dead end.
    #[test]
    fn nonsense_is_not_a_request() {
        assert!(parse("").is_none());
        assert!(parse("https://getvela.app").is_none());
        assert!(parse(ME).is_none(), "a bare address is not a URI");
        assert!(parse("ethereum:").is_none());
        assert!(parse("ethereum:notanaddress@1").is_none());
        // A negative amount is not "no amount": it is a request this cannot
        // honour, and the field simply carries nothing.
        let negative = parse(&format!("ethereum:{ME}@1?value=-5"))
            .unwrap_or_else(|| unreachable!("the request itself parses"));
        assert!(negative.amount_base_units.is_none());
    }

    /// A percent-encoded query survives — the address param is the one that
    /// matters, and some encoders escape everything.
    #[test]
    fn a_percent_encoded_query_decodes() {
        let encoded = format!(
            "ethereum:{TOKEN}@100/transfer?address=%30x{}&uint256=7",
            &ME[2..]
        );
        let request = parse(&encoded).unwrap_or_else(|| unreachable!("a token request"));
        assert_eq!(request.recipient, ME);
        assert_eq!(request.amount_base_units.as_deref(), Some("7"));
    }
}
