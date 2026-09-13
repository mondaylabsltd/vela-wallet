//! Rules of clear-signing resolution and risk adjudication, one test per rule.
//!
//! Ports the jest vectors for `siwe.ts`, `decode-sign-message.ts` and the
//! resolution scenarios of `clear-signing.test.ts` onto the machine, plus the
//! inventory invariants (decimals trust, ERC-165 caching, partial/expired
//! risk floors, blind-over-half-truth, stale-run dropping). The shell is the
//! test: descriptor bodies, RPC answers and timeouts are handed in explicitly.

#![cfg(feature = "crux")]

mod support;

use serde_json::json;
use support::DomainDriver;
use vela_core::abi::compute_selector;
use vela_core::app::clear_signing::{
    ClearConfirm, ClearDangerClass, ClearFieldRole, ClearLocale, ClearOperation as Op, ClearProbe,
    ClearRisk, ClearShellResult as Res, ClearSignMethod, ClearSignType, ClearSigning,
    ClearSiweBinding, ClearSurface, Event,
};

type Sut = DomainDriver<ClearSigning>;

/// 2025-08-09T00:40:00Z-ish, epoch ms — after the 1750000000s permit deadline.
const NOW: f64 = 1_754_700_000_000.0;

const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
const BAYC: &str = "0xbc4ca0eda7647a8ab7c2061c2e118a18a936f13d";
const UNKNOWN_TOKEN: &str = "0x1234567890abcdef1234567890abcdef12345678";
const VITALIK: &str = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
const SPENDER: &str = "0x111111125421ca6dc452d289314280a0f8842a65";
const UNIV2: &str = "0x7a250d5630b4cf539739df2c5dacb4c659f2488d";

fn pad(hexish: &str) -> String {
    let clean = hexish.trim_start_matches("0x").to_lowercase();
    format!("{clean:0>64}")
}

fn pad_u128(v: u128) -> String {
    format!("{v:064x}")
}

fn text_hex(s: &str) -> String {
    let mut out = String::from("0x");
    for b in s.as_bytes() {
        out.push_str(&format!("{b:02x}"));
    }
    out
}

fn supports_data(iface: &str) -> String {
    format!("0x01ffc9a7{iface}{}", "0".repeat(56))
}

fn timer_token(ops: &[Op]) -> u32 {
    ops.iter()
        .find_map(|op| match op {
            Op::Timer { token, .. } => Some(*token),
            _ => None,
        })
        .expect("a timer op")
}

/// Start a transaction resolution and hand the core its clock.
fn resolve_tx(sut: &mut Sut, to: &str, data: &str, value: &str) -> Vec<Op> {
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some(to.to_owned()),
        data: Some(data.to_owned()),
        value: Some(value.to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert_eq!(
        ops,
        vec![Op::Now],
        "every resolution starts by asking the clock"
    );
    sut.resolve(Res::Clock { now_ms: NOW })
}

fn message_view(
    method: ClearSignMethod,
    payload: &str,
    origin: Option<&str>,
) -> vela_core::app::clear_signing::ClearMessageView {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::MessagePresented {
        method,
        params: vec![payload.to_owned()],
        request_origin: origin.map(str::to_owned),
    });
    assert!(ops.is_empty(), "message analysis is pure — no shell work");
    sut.view().message.expect("message analyzed")
}

fn personal(
    payload: &str,
    origin: Option<&str>,
) -> vela_core::app::clear_signing::ClearMessageView {
    message_view(ClearSignMethod::PersonalSign, payload, origin)
}

fn siwe_text() -> String {
    [
        "app.uniswap.org wants you to sign in with your Ethereum account:",
        "0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1",
        "",
        "Sign in to Uniswap",
        "",
        "URI: https://app.uniswap.org",
        "Version: 1",
        "Chain ID: 1",
        "Nonce: 8a3b9f2c",
        "Issued At: 2026-01-01T00:00:00.000Z",
    ]
    .join("\n")
}

// ---------------------------------------------------------------------------
// SIWE parsing + domain binding (siwe.test.ts, ported)
// ---------------------------------------------------------------------------

#[test]
fn siwe_canonical_message_parses() {
    let view = personal(&siwe_text(), None);
    let siwe = view.siwe.expect("SIWE detected");
    assert_eq!(siwe.domain, "app.uniswap.org");
    assert_eq!(
        siwe.address.as_deref(),
        Some("0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1")
    );
    assert_eq!(siwe.uri.as_deref(), Some("https://app.uniswap.org"));
    assert_eq!(siwe.chain_id, Some(1));
    assert_eq!(siwe.nonce.as_deref(), Some("8a3b9f2c"));
    assert_eq!(siwe.statement.as_deref(), Some("Sign in to Uniswap"));
}

#[test]
fn siwe_plain_prose_is_not_a_sign_in() {
    assert!(personal("gm, please sign this to continue", None)
        .siwe
        .is_none());
    let view = personal("gm, please sign this to continue", None);
    assert_eq!(view.danger_class, ClearDangerClass::Plain);
}

/// siwe.ts:33-36 — CRLF payloads must not silently disable phishing
/// detection via a trailing `\r` on the line-1 anchor.
#[test]
fn siwe_crlf_normalized_keeps_phish_detection_armed() {
    let crlf = siwe_text().replace('\n', "\r\n");
    let view = personal(&crlf, Some("https://uniswaq.app"));
    let siwe = view.siwe.expect("CRLF SIWE still parses");
    assert_eq!(siwe.domain, "app.uniswap.org");
    assert_eq!(
        siwe.nonce.as_deref(),
        Some("8a3b9f2c"),
        "no stray \\r in fields"
    );
    assert_eq!(view.binding, Some(ClearSiweBinding::Mismatch));
    assert_eq!(view.danger_class, ClearDangerClass::SiwePhish);
}

/// siwe.ts:45 — a userinfo-spoofed domain never gets a binding check at all.
#[test]
fn siwe_userinfo_domain_is_rejected_as_siwe() {
    let spoof = siwe_text().replace("app.uniswap.org wants", "app.uniswap.org@evil.com wants");
    let view = personal(&spoof, Some("https://evil.com"));
    assert!(view.siwe.is_none(), "treated as plain text, not a sign-in");
    assert_eq!(view.danger_class, ClearDangerClass::Plain);
}

#[test]
fn siwe_path_or_scheme_domain_is_rejected_as_siwe() {
    let with_path = siwe_text().replace("app.uniswap.org wants", "evil.com/app.uniswap.org wants");
    assert!(personal(&with_path, None).siwe.is_none());
    let with_scheme = siwe_text().replace("app.uniswap.org wants", "https://app.uniswap.org wants");
    assert!(personal(&with_scheme, None).siwe.is_none());
}

#[test]
fn siwe_binding_ok_when_origin_matches() {
    let view = personal(&siwe_text(), Some("https://app.uniswap.org"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Ok));
    assert_eq!(view.danger_class, ClearDangerClass::SiweOk);
    // A bare host origin matches too.
    let view = personal(&siwe_text(), Some("app.uniswap.org"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Ok));
}

#[test]
fn siwe_mismatch_flags_phishing() {
    let view = personal(&siwe_text(), Some("https://uniswaq.app"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Mismatch));
    assert_eq!(view.danger_class, ClearDangerClass::SiwePhish);
}

#[test]
fn siwe_unknown_when_origin_missing_never_asserts_a_match() {
    let view = personal(&siwe_text(), None);
    assert_eq!(view.binding, Some(ClearSiweBinding::Unknown));
    // Unknown renders the calm sign-in layout; only Mismatch escalates.
    assert_eq!(view.danger_class, ClearDangerClass::SiweOk);
}

#[test]
fn siwe_host_ignores_port_and_trailing_fqdn_dot() {
    let view = personal(&siwe_text(), Some("https://app.uniswap.org:443"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Ok));
    let dotted = siwe_text().replace("app.uniswap.org wants", "app.uniswap.org. wants");
    let view = personal(&dotted, Some("https://app.uniswap.org"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Ok));
}

/// siwe.ts:88-92 — an unparseable origin is `unknown`, never a half-parsed
/// host that could spuriously match.
#[test]
fn siwe_unparseable_origin_is_unknown() {
    let view = personal(&siwe_text(), Some("not a url"));
    assert_eq!(view.binding, Some(ClearSiweBinding::Unknown));
    assert_eq!(view.danger_class, ClearDangerClass::SiweOk);
}

// ---------------------------------------------------------------------------
// personal_sign decoding (decode-sign-message.test.ts, ported)
// ---------------------------------------------------------------------------

/// Issue #82 — the biubiu default message with a trailing emoji is TEXT.
#[test]
fn hex_payload_with_emoji_decodes_as_text() {
    let view = personal(&text_hex("Hello from biubiu.tools 👋"), None);
    assert!(view.is_hex);
    assert_eq!(
        view.decoded_text.as_deref(),
        Some("Hello from biubiu.tools 👋")
    );
    assert!(!view.non_printable);
    assert_eq!(view.danger_class, ClearDangerClass::Plain);
}

#[test]
fn hex_payload_with_cjk_and_accents_decodes_as_text() {
    assert_eq!(
        personal(&text_hex("Café résumé"), None)
            .decoded_text
            .as_deref(),
        Some("Café résumé")
    );
    assert_eq!(
        personal(&text_hex("签名消息"), None)
            .decoded_text
            .as_deref(),
        Some("签名消息")
    );
    assert_eq!(
        personal(&text_hex("Test sign message"), None)
            .decoded_text
            .as_deref(),
        Some("Test sign message")
    );
}

#[test]
fn multiline_and_tabbed_text_stays_text() {
    let siwe = "example.com wants you to sign in\n\nURI: https://example.com\ntab\tend";
    let view = personal(&text_hex(siwe), None);
    assert_eq!(view.decoded_text.as_deref(), Some(siwe));
    assert!(!view.non_printable);
}

#[test]
fn raw_32_byte_hash_falls_back_to_hex_preview() {
    let hash = format!("0x{}", "de1a".repeat(16));
    let view = personal(&hash, None);
    let preview = view.binary_preview.expect("binary preview");
    assert!(preview.starts_with("0x"));
    assert!(!preview.contains('\u{fffd}'));
    assert!(view.non_printable);
    assert_eq!(view.danger_class, ClearDangerClass::OpaqueHash);
}

#[test]
fn nul_control_byte_forces_the_binary_fallback() {
    let view = personal("0x414200", None);
    assert_eq!(view.binary_preview.as_deref(), Some("0x414200"));
    assert_eq!(view.danger_class, ClearDangerClass::OpaqueHash);
}

#[test]
fn long_binary_preview_truncates_with_ellipsis() {
    let long = format!("0x{}", "01".repeat(64));
    let view = personal(&long, None);
    assert!(view.binary_preview.expect("preview").ends_with("..."));
}

/// decode-sign-message.ts:44-48 — the single hex predicate (invariant ⑥):
/// a bare `deadbeef` or odd-length `0xabc` is a MESSAGE, shown (and signed)
/// verbatim as UTF-8.
#[test]
fn non_hex_payload_is_shown_verbatim() {
    let view = personal("deadbeef", None);
    assert!(!view.is_hex);
    assert_eq!(view.decoded_text.as_deref(), Some("deadbeef"));
    assert!(
        !view.non_printable,
        "canon predicate, not the view-side ASCII one"
    );

    let view = personal("0xabc", None);
    assert!(!view.is_hex);
    assert_eq!(view.decoded_text.as_deref(), Some("0xabc"));
}

/// SigningSheet.tsx:465-470 (invariant ⑦) — eth_sign NEVER renders the calm
/// message view, however readable its payload.
#[test]
fn eth_sign_is_always_the_danger_class() {
    let view = message_view(ClearSignMethod::EthSign, &text_hex("hello there"), None);
    assert_eq!(view.danger_class, ClearDangerClass::EthSign);
    assert!(view.siwe.is_none(), "no sign-in treatment for eth_sign");

    // Even a SIWE-shaped payload gets the hard warning.
    let view = message_view(
        ClearSignMethod::EthSign,
        &text_hex(&siwe_text()),
        Some("https://app.uniswap.org"),
    );
    assert_eq!(view.danger_class, ClearDangerClass::EthSign);
}

// ---------------------------------------------------------------------------
// resolveTransaction — trivial ends
// ---------------------------------------------------------------------------

#[test]
fn plain_native_transfer_resolves_blind_with_no_shell_work() {
    for data in [None, Some(String::new()), Some("0x".to_owned())] {
        let mut sut = Sut::new();
        let ops = sut.dispatch(Event::ResolveTransaction {
            to: Some(VITALIK.to_owned()),
            data,
            value: Some("0xde0b6b3a7640000".to_owned()),
            chain_id: 1,
            locale: ClearLocale::default(),
        });
        assert!(ops.is_empty());
        let view = sut.view();
        assert!(!view.resolving);
        assert!(view.resolved);
        assert!(view.result.is_none(), "native transfer UI, not clear-sign");
    }
}

#[test]
fn create2_deploy_renders_calm_with_predicted_address() {
    let mut sut = Sut::new();
    let data = format!("0x{}{}", "11".repeat(32), "6001600155");
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some("0x4e59B44847b379578588920cA78FbF26c0B4956C".to_owned()),
        data: Some(data),
        value: None,
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert!(ops.is_empty(), "deploy detection is pure");
    let result = sut.view().result.expect("deploy result");
    assert_eq!(result.intent, "Deploy contract");
    assert_eq!(result.contract_name.as_deref(), Some("CREATE2 Deployer"));
    assert_eq!(result.risk, ClearRisk::Normal);
    assert_eq!(result.fields.len(), 1);
    assert_eq!(result.fields[0].label, "New contract");
    assert_eq!(result.fields[0].format, "addressName");
    assert_eq!(
        result.contract_address.as_deref(),
        Some("0x4e59b44847b379578588920ca78fbf26c0b4956c")
    );
}

#[test]
fn raw_create_renders_calm_without_prediction() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: None,
        data: Some("0x60016001556001600255".to_owned()),
        value: None,
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert!(ops.is_empty());
    let result = sut.view().result.expect("deploy result");
    assert_eq!(result.intent, "Deploy contract");
    assert!(result.contract_name.is_none());
    assert!(result.fields.is_empty());
    assert!(result.contract_address.is_none());
}

/// SigningSheet.tsx:441-447 (invariant ⑦) — while the descriptor resolves,
/// the view is "resolving", never a blind view that flashes first.
#[test]
fn resolving_never_flashes_blind() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    let ops = resolve_tx(&mut sut, USDC, &transfer, "0x0");
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        }]
    );
    let view = sut.view();
    assert!(view.resolving);
    assert!(!view.resolved);
    assert!(view.result.is_none());
}

// ---------------------------------------------------------------------------
// Interface descriptors + known-token decimals
// ---------------------------------------------------------------------------

#[test]
fn erc20_transfer_decodes_with_known_decimals_and_usd() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    assert!(ops.is_empty(), "known token: no probes, no warm");

    let result = sut.view().result.expect("clear-sign result");
    assert_eq!(result.intent, "Send");
    assert_eq!(result.sign_type, ClearSignType::Transaction);
    assert!(
        !result.verified,
        "interface descriptor is not contract-specific"
    );
    assert_eq!(result.risk, ClearRisk::Normal);
    assert_eq!(result.contract_address.as_deref(), Some(USDC));

    let amount = &result.fields[0];
    assert_eq!(amount.label, "Amount");
    assert_eq!(amount.value, "1,000 USDC");
    assert_eq!(amount.role, ClearFieldRole::SendAmount);
    assert_eq!(amount.token_address.as_deref(), Some(USDC));
    assert_eq!(amount.usd_value, Some(1000.0));
    assert!(!amount.unverified);

    let to = &result.fields[1];
    assert_eq!(to.label, "To");
    assert_eq!(to.value, "0xd8da6b...a96045");
    assert_eq!(to.role, ClearFieldRole::Recipient);
    assert_eq!(to.address.as_deref(), Some(VITALIK));
}

#[test]
fn unlimited_approve_reads_danger() {
    let mut sut = Sut::new();
    let approve = format!("0x095ea7b3{}{}", pad(SPENDER), "f".repeat(64));
    resolve_tx(&mut sut, USDC, &approve, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    // approve shares a selector with ERC-721 — the chain is asked first.
    assert_eq!(
        ops,
        vec![
            Op::RpcEthCall {
                chain_id: 1,
                to: USDC.to_owned(),
                data: supports_data("80ac58cd"),
                probe: ClearProbe::SupportsErc721,
            },
            Op::RpcEthCall {
                chain_id: 1,
                to: USDC.to_owned(),
                data: supports_data("d9b67a26"),
                probe: ClearProbe::SupportsErc1155,
            },
            Op::Timer {
                ms: 3_000,
                token: 1
            },
        ]
    );
    // Both probes revert — a plain ERC-20, definitively.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc721,
        chain_id: 1,
        to: USDC.to_owned(),
        result: None,
        rpc_error: true,
    });
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc1155,
        chain_id: 1,
        to: USDC.to_owned(),
        result: None,
        rpc_error: true,
    });

    let result = sut.view().result.expect("approve result");
    assert_eq!(result.intent, "Approve");
    assert_eq!(
        result.risk,
        ClearRisk::Danger,
        "unlimited approval is danger"
    );
    let amount = &result.fields[0];
    assert!(amount.warning);
    assert_eq!(amount.value, "Unlimited");
    let spender = &result.fields[1];
    assert_eq!(spender.role, ClearFieldRole::Spender);
}

#[test]
fn limited_approve_reads_caution() {
    let mut sut = Sut::new();
    let approve = format!("0x095ea7b3{}{}", pad(SPENDER), pad_u128(500_000_000));
    resolve_tx(&mut sut, USDC, &approve, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc721,
        chain_id: 1,
        to: USDC.to_owned(),
        result: None,
        rpc_error: true,
    });
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc1155,
        chain_id: 1,
        to: USDC.to_owned(),
        result: None,
        rpc_error: true,
    });
    let result = sut.view().result.expect("approve result");
    assert_eq!(result.risk, ClearRisk::Caution);
    assert!(!result.fields.iter().any(|f| f.warning));
    assert_eq!(result.fields[0].value, "500 USDC");
    assert_eq!(result.fields[0].usd_value, Some(500.0));
}

// ---------------------------------------------------------------------------
// ERC-165 disambiguation (invariants ② and ④)
// ---------------------------------------------------------------------------

/// clear-signing.ts:164-171 (invariant ④) — the shared transferFrom selector
/// is judged via ERC-165 BEFORE rendering: an NFT's tokenId must never render
/// as a fungible amount.
#[test]
fn shared_selector_transfer_from_is_judged_before_rendering() {
    let mut sut = Sut::new();
    let transfer_from = format!(
        "0x23b872dd{}{}{}",
        pad("af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1"),
        pad(VITALIK),
        pad_u128(6529)
    );
    resolve_tx(&mut sut, BAYC, &transfer_from, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{BAYC}.json"),
        json: None,
    });
    let token = timer_token(&ops);
    let yes = format!("0x{}", pad("1"));
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc721,
        chain_id: 1,
        to: BAYC.to_owned(),
        result: Some(yes),
        rpc_error: false,
    });
    let ops = sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc1155,
        chain_id: 1,
        to: BAYC.to_owned(),
        result: None,
        rpc_error: true,
    });
    assert!(ops.is_empty());

    let result = sut.view().result.expect("NFT result");
    assert_eq!(result.intent, "Transfer NFT");
    let token_id = result
        .fields
        .iter()
        .find(|f| f.label == "Token ID")
        .expect("token id");
    assert_eq!(token_id.value, "#6,529");
    assert!(
        !result
            .fields
            .iter()
            .any(|f| f.role == ClearFieldRole::SendAmount),
        "a tokenId must never read as a fungible send amount"
    );

    // The stale 3s timer may fire later; it must be a no-op.
    let ops = sut.resolve(Res::TimedOut { token });
    assert!(ops.is_empty());
}

/// clear-signing.ts:186-201 (invariant ②) — a definitive verdict is cached:
/// the second request for the same contract asks the chain nothing.
#[test]
fn erc165_definitive_verdict_is_cached() {
    let mut sut = Sut::new();
    let transfer_from = format!(
        "0x23b872dd{}{}{}",
        pad("af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1"),
        pad(VITALIK),
        pad_u128(6529)
    );
    resolve_tx(&mut sut, BAYC, &transfer_from, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{BAYC}.json"),
        json: None,
    });
    let token = timer_token(&ops);
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc721,
        chain_id: 1,
        to: BAYC.to_owned(),
        result: Some(format!("0x{}", pad("1"))),
        rpc_error: false,
    });
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc1155,
        chain_id: 1,
        to: BAYC.to_owned(),
        result: None,
        rpc_error: true,
    });
    sut.resolve(Res::TimedOut { token }); // drain the stale timer

    // Second request: the negative descriptor cache AND the ERC-165 cache
    // answer everything — zero shell operations.
    let ops = resolve_tx(&mut sut, BAYC, &transfer_from, "0x0");
    assert!(ops.is_empty(), "cached verdicts, no re-probe, no re-fetch");
    assert_eq!(
        sut.view().result.expect("cached NFT result").intent,
        "Transfer NFT"
    );
}

/// clear-signing.ts:186-201 (invariant ②) — RPC-unreachable is UNKNOWN, not
/// "not an NFT": nothing is cached and the next request re-probes.
#[test]
fn erc165_unreachable_is_never_cached_as_a_verdict() {
    let mut sut = Sut::new();
    let approve = format!("0x095ea7b3{}{}", pad(SPENDER), pad_u128(1));
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &approve, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    let token = timer_token(&ops);
    // Both probes unreachable → render as ERC-20 now, cache NOTHING.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc721,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    let ops = sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::SupportsErc1155,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    // The run continues into the decimals warm for the unknown token.
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::RpcEthCall {
            probe: ClearProbe::Decimals,
            ..
        }
    )));
    let warm_token = timer_token(&ops);
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Decimals,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    // The symbol probe rides beside the decimals one (spec 032 phase 23).
    // This token has none to give, so the address fallback stands.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Symbol,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    assert_eq!(sut.view().result.expect("approve").intent, "Approve");
    sut.resolve(Res::TimedOut { token }); // stale 3s timer
    sut.resolve(Res::TimedOut { token: warm_token }); // stale 4s timer

    // Next request for the same contract: the probes are issued AGAIN.
    let ops = resolve_tx(&mut sut, UNKNOWN_TOKEN, &approve, "0x0");
    assert!(
        ops.iter().any(|op| matches!(
            op,
            Op::RpcEthCall {
                probe: ClearProbe::SupportsErc721,
                ..
            }
        )),
        "unreachable was not cached — the chain is asked again"
    );
}

/// The 3s race: on timeout render ERC-20 (uncached); the probes stay in
/// flight and their LATE definitive answers still teach the cache — exactly
/// the TS module-singleton behavior.
#[test]
fn erc165_timeout_falls_back_and_late_answers_still_cache() {
    let mut sut = Sut::new();
    let transfer_from = format!(
        "0x23b872dd{}{}{}",
        pad("af5e8917831ef08a64e18b2cde9f8f5d32c7b3e1"),
        pad(VITALIK),
        pad_u128(7)
    );
    resolve_tx(&mut sut, BAYC, &transfer_from, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{BAYC}.json"),
        json: None,
    });
    let token = timer_token(&ops);

    // Supersede the run before the probes answer — their answers arrive
    // stale, and must STILL fill the cache.
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000));
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some(USDC.to_owned()),
        data: Some(transfer),
        value: Some("0x0".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert_eq!(ops, vec![Op::Now]);

    // The stale probe answers (definitive: an ERC-721).
    assert!(sut
        .resolve(Res::RpcAnswer {
            probe: ClearProbe::SupportsErc721,
            chain_id: 1,
            to: BAYC.to_owned(),
            result: Some(format!("0x{}", pad("1"))),
            rpc_error: false,
        })
        .is_empty());
    assert!(sut
        .resolve(Res::RpcAnswer {
            probe: ClearProbe::SupportsErc1155,
            chain_id: 1,
            to: BAYC.to_owned(),
            result: None,
            rpc_error: true,
        })
        .is_empty());
    assert!(sut.resolve(Res::TimedOut { token }).is_empty());

    // Finish the superseding USDC run.
    sut.resolve(Res::Clock { now_ms: NOW });
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    assert_eq!(sut.view().result.expect("usdc transfer").intent, "Send");

    // A fresh BAYC request now hits the cache the late answers filled.
    let ops = resolve_tx(&mut sut, BAYC, &transfer_from, "0x0");
    assert!(ops.is_empty(), "late definitive answers taught the cache");
    assert_eq!(sut.view().result.expect("NFT").intent, "Transfer NFT");
}

/// The timeout itself decides erc20 for THIS render but caches nothing.
#[test]
fn erc165_timeout_renders_erc20_without_caching() {
    let mut sut = Sut::new();
    let approve = format!("0x095ea7b3{}{}", pad(SPENDER), pad_u128(5));
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &approve, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    let token = timer_token(&ops);
    // The probes never answer.
    sut.drop_oldest();
    sut.drop_oldest();
    let ops = sut.resolve(Res::TimedOut { token });
    // ERC-20 fallback → decimals warm for the unknown token.
    let warm_token = timer_token(&ops);
    // Neither warm call answers: the decimals probe and the symbol probe that
    // now rides beside it (spec 032 phase 23). Dropping one would hand the
    // timer's answer to the other.
    sut.drop_oldest();
    sut.drop_oldest();
    sut.resolve(Res::TimedOut { token: warm_token });
    let result = sut
        .view()
        .result
        .expect("approve rendered despite timeouts");
    assert_eq!(result.intent, "Approve");

    // Nothing was cached: a fresh request probes again.
    let ops = resolve_tx(&mut sut, UNKNOWN_TOKEN, &approve, "0x0");
    assert!(ops.iter().any(|op| matches!(
        op,
        Op::RpcEthCall {
            probe: ClearProbe::SupportsErc721,
            ..
        }
    )));
}

// ---------------------------------------------------------------------------
// Decimals trust (invariant ①)
// ---------------------------------------------------------------------------

/// clear-signing.ts:362-363 (invariant ①) — an unknown token's decimals are
/// NEVER silently assumed: the chain is asked; a failed lookup means 18 + an
/// explicit `unverified` flag, and risk floors at caution.
#[test]
fn unknown_token_decimals_are_never_assumed_silently() {
    let mut sut = Sut::new();
    let transfer = format!(
        "0xa9059cbb{}{}",
        pad(VITALIK),
        pad_u128(500_000_000_000_000_000)
    );
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &transfer, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    assert_eq!(
        ops[0],
        Op::RpcEthCall {
            chain_id: 1,
            to: UNKNOWN_TOKEN.to_owned(),
            data: "0x313ce567".to_owned(),
            probe: ClearProbe::Decimals,
        },
        "decimals() is fetched on-chain before formatting"
    );
    let warm_token = timer_token(&ops);

    let ops = sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Decimals,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });

    assert!(
        ops.is_empty(),
        "decimals alone does not finish the warm step"
    );

    // The symbol probe rides the same round trip and gates with it (phase 25).
    // This token has no symbol to give either, so the address fallback stands.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Symbol,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    let result = sut.view().result.expect("transfer result");
    let amount = &result.fields[0];
    assert!(
        amount.unverified,
        "failed lookup ⇒ 18 + explicit unverified flag"
    );
    // NOT "0.5". Unknown decimals mean an unknown magnitude, and the sheet
    // says so rather than printing a number scaled by a guess.
    assert_eq!(amount.value, format!("— {}...", &UNKNOWN_TOKEN[..6]));
    assert_eq!(
        result.risk,
        ClearRisk::Caution,
        "unverified floors risk at caution"
    );
    assert!(amount.usd_value.is_none());

    sut.resolve(Res::TimedOut { token: warm_token });
}

#[test]
fn onchain_decimals_resolve_scale_and_cache() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(500_000_000));
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &transfer, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    let warm_token = timer_token(&ops);
    let ops = sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Decimals,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: Some(format!("0x{}", pad("8"))),
        rpc_error: false,
    });
    // The symbol probe rides beside the decimals one (spec 032 phase 23).
    // This token has none to give, so the address fallback stands.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Symbol,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: None,
        rpc_error: false,
    });
    assert!(ops.is_empty());
    let result = sut.view().result.expect("transfer result");
    let amount = &result.fields[0];
    assert!(!amount.unverified);
    assert_eq!(amount.value, format!("5 {}...", &UNKNOWN_TOKEN[..6]));
    assert_eq!(result.risk, ClearRisk::Normal);
    sut.resolve(Res::TimedOut { token: warm_token });

    // Cached: the second request needs no warm at all.
    let ops = resolve_tx(&mut sut, UNKNOWN_TOKEN, &transfer, "0x0");
    assert!(ops.is_empty(), "decimals cached — no second eth_call");
    assert!(!sut.view().result.expect("result").fields[0].unverified);
}

/// A symbol that answers AFTER the decimals still reaches the sheet.
///
/// Phase 23 added the probe and did not gate on it, which running the desktop
/// showed to be the same as not having it: both probes ride one round trip and
/// land milliseconds apart (622ms and 642ms against Gnosis), so the sheet
/// formatted on the decimals answer and the symbol only ever reached the cache
/// — a token was shown as `0x2a22…` on every first sight. The warm step now
/// waits for both. The 4s cap still bounds the wait; only the common case moved.
#[test]
fn a_symbol_arriving_after_the_decimals_is_still_on_the_sheet() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(500_000_000));
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &transfer, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    let warm_token = timer_token(&ops);

    let ops = sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Decimals,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: Some(format!("0x{}", pad("6"))),
        rpc_error: false,
    });
    assert!(ops.is_empty());
    assert!(
        sut.view().result.is_none(),
        "the decimals answer alone must not conclude the run — the symbol is \
         still in flight, and concluding here is what threw it away"
    );

    // "USDC.e", the dynamic layout.
    sut.resolve(Res::RpcAnswer {
        probe: ClearProbe::Symbol,
        chain_id: 1,
        to: UNKNOWN_TOKEN.to_owned(),
        result: Some(
            "0x\
             0000000000000000000000000000000000000000000000000000000000000020\
             0000000000000000000000000000000000000000000000000000000000000006\
             555344432e650000000000000000000000000000000000000000000000000000"
                .to_owned(),
        ),
        rpc_error: false,
    });

    let result = sut.view().result.expect("transfer result");
    let amount = &result.fields[0];
    assert!(!amount.unverified);
    assert_eq!(
        amount.value, "500 USDC.e",
        "the chain answered what this token is called and the sheet says it"
    );
    sut.resolve(Res::TimedOut { token: warm_token });
}

/// The 4s warm cap: on timeout the sheet shows the SAFE fallback (18 +
/// unverified), and the still-in-flight lookup teaches the cache late.
#[test]
fn decimals_timeout_shows_safe_fallback_and_late_answer_caches() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(500_000_000));
    resolve_tx(&mut sut, UNKNOWN_TOKEN, &transfer, "0x0");
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{UNKNOWN_TOKEN}.json"),
        json: None,
    });
    let warm_token = timer_token(&ops);

    // Supersede so the decimals answer arrives stale, then time the warm out.
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some(UNKNOWN_TOKEN.to_owned()),
        data: Some(transfer.clone()),
        value: Some("0x0".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert_eq!(ops, vec![Op::Now]);

    // Stale decimals answer → cache is taught anyway.
    assert!(sut
        .resolve(Res::RpcAnswer {
            probe: ClearProbe::Decimals,
            chain_id: 1,
            to: UNKNOWN_TOKEN.to_owned(),
            result: Some(format!("0x{}", pad("8"))),
            rpc_error: false,
        })
        .is_empty());
    // Its symbol twin, in issue order — this driver answers the queue from the
    // front, so skipping one hands the NEXT answer to the wrong question.
    assert!(sut
        .resolve(Res::RpcAnswer {
            probe: ClearProbe::Symbol,
            chain_id: 1,
            to: UNKNOWN_TOKEN.to_owned(),
            result: None,
            rpc_error: false,
        })
        .is_empty());
    assert!(sut.resolve(Res::TimedOut { token: warm_token }).is_empty());

    // The superseding run finds the cache warm: no eth_call, verified scale.
    let ops = sut.resolve(Res::Clock { now_ms: NOW });
    assert!(ops.is_empty(), "descriptor cached + decimals cached");
    let result = sut.view().result.expect("result");
    assert!(!result.fields[0].unverified);
    assert_eq!(
        result.fields[0].value,
        format!("5 {}...", &UNKNOWN_TOKEN[..6])
    );
}

// ---------------------------------------------------------------------------
// Blind-over-half-truth + partial (invariants ③ and ④)
// ---------------------------------------------------------------------------

/// clear-signing.ts:587-590 (invariant ④) — zero resolved fields is a blind
/// sign; the pipeline walks every remaining level and honestly gives up.
#[test]
fn zero_resolved_fields_falls_through_to_blind() {
    let mut sut = Sut::new();
    let sig = "doThing(uint256 a)";
    let selector = compute_selector(sig).expect("selector");
    let data = format!("{selector}{}", pad_u128(1));
    let target = "0x9999999999999999999999999999999999999999";

    resolve_tx(&mut sut, target, &data, "0x0");
    let descriptor = json!({
        "display": { "formats": { sig: { "intent": "Do thing", "fields": [
            { "path": "missing1", "label": "X", "format": "raw" },
            { "path": "missing2", "label": "Y", "format": "raw" },
        ] } } }
    })
    .to_string();
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{target}.json"),
        json: Some(descriptor),
    });
    // 0/2 fields resolved → null → not a token selector → ERC fallbacks.
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: "/erc7730/ercs/calldata-erc20-tokens.json".to_owned(),
        }]
    );
    let ops = sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc20-tokens.json".to_owned(),
        json: None,
    });
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: "/erc7730/ercs/calldata-erc721-nfts.json".to_owned(),
        }]
    );
    let ops = sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc721-nfts.json".to_owned(),
        json: None,
    });
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: "/erc7730/ercs/calldata-erc4626-vaults.json".to_owned(),
        }]
    );
    let ops = sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc4626-vaults.json".to_owned(),
        json: None,
    });
    // 4. Last resort: the 4-byte selector database.
    assert_eq!(
        ops,
        vec![Op::SelectorDbLookup {
            selector: selector.clone(),
        }]
    );
    let ops = sut.resolve(Res::SelectorCandidates { sigs: vec![] });
    assert!(ops.is_empty());
    let view = sut.view();
    assert!(view.resolved);
    assert!(view.result.is_none(), "honest blind sign, not a half-truth");
}

/// clear-signing.ts:1266-1270 (invariant ③) — an incomplete decode shows
/// what resolved but flags `partial` and floors risk at caution.
#[test]
fn partial_decode_floors_risk_at_caution() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    let descriptor = json!({
        "context": { "contract": {} },
        "display": { "formats": { "transfer(address _to, uint256 _value)": {
            "intent": "Send",
            "fields": [
                { "path": "_value", "label": "Amount", "format": "tokenAmount", "visible": "always" },
                { "path": "_to", "label": "To", "format": "addressName", "visible": "always" },
                { "path": "nonexistent1", "label": "Field 3", "format": "raw", "visible": "always" },
                { "path": "nonexistent2", "label": "Field 4", "format": "raw", "visible": "always" },
                { "path": "nonexistent3", "label": "Field 5", "format": "raw", "visible": "always" },
                { "path": "nonexistent4", "label": "Field 6", "format": "raw", "visible": "always" },
            ],
        } } }
    })
    .to_string();
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: Some(descriptor),
    });
    assert!(ops.is_empty());
    let result = sut.view().result.expect("partial result");
    assert!(result.partial);
    assert!(!result.fields.is_empty());
    assert_eq!(result.risk, ClearRisk::Caution);
    assert!(result.verified, "contract-specific descriptor");
}

/// An already-expired deadline can't be trusted either — caution, and the
/// field carries the `expired` flag (invariant ③).
#[test]
fn expired_deadline_floors_risk_at_caution() {
    let mut sut = Sut::new();
    let sig = "queueThing(uint256 deadline)";
    let selector = compute_selector(sig).expect("selector");
    let data = format!("{selector}{}", pad_u128(1_750_000_000)); // past vs NOW
    let target = "0x8888888888888888888888888888888888888888";
    resolve_tx(&mut sut, target, &data, "0x0");
    let descriptor = json!({
        "display": { "formats": { sig: { "intent": "Queue", "fields": [
            { "path": "deadline", "label": "Deadline", "format": "date" },
        ] } } }
    })
    .to_string();
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{target}.json"),
        json: Some(descriptor),
    });
    let result = sut.view().result.expect("result");
    assert!(result.fields[0].expired);
    assert_eq!(result.risk, ClearRisk::Caution);
}

// ---------------------------------------------------------------------------
// Local descriptors (clear-signing.test.ts wave-2 ports)
// ---------------------------------------------------------------------------

#[test]
fn local_uniswap_v2_swap_renders_richly_without_network() {
    let mut sut = Sut::new();
    let weth = "c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
    let deadline: u128 = 1_755_000_000; // future vs NOW — a live swap
    let swap = format!(
        "0x38ed1739{}{}{}{}{}{}{}{}",
        pad_u128(1_000_000_000),           // amountIn: 1000 USDC (6dp)
        pad_u128(500_000_000_000_000_000), // amountOutMin: 0.5 WETH
        pad("a0"),                         // path offset
        pad(VITALIK),                      // to
        pad_u128(deadline),
        pad("2"), // path length
        pad(USDC),
        pad(weth),
    );
    let ops = resolve_tx(&mut sut, UNIV2, &swap, "0x0");
    assert!(
        ops.is_empty(),
        "local descriptor + known tokens: zero round-trips"
    );

    let result = sut.view().result.expect("swap result");
    assert_eq!(result.intent, "Swap");
    assert_eq!(result.contract_name.as_deref(), Some("Uniswap V2 Router"));
    assert!(result.verified);
    assert_eq!(result.risk, ClearRisk::Normal);

    let send = result
        .fields
        .iter()
        .find(|f| f.role == ClearFieldRole::SendAmount)
        .expect("send field");
    assert!(send.value.contains("USDC"));
    assert!(send.value.contains("1,000"));
    let recv = result
        .fields
        .iter()
        .find(|f| f.role == ClearFieldRole::ReceiveAmount)
        .expect("receive field");
    assert!(recv.value.contains("WETH"));
    assert!(recv.value.contains("0.5"));
}

#[test]
fn lido_stake_reads_amount_from_msg_value() {
    let mut sut = Sut::new();
    let data = format!("0xa1903eab{}", pad("0"));
    let ops = resolve_tx(
        &mut sut,
        "0xae7ab96520de3a18e5e111b5eaab095312d7fe84",
        &data,
        "0xde0b6b3a7640000", // 1 ETH
    );
    assert!(ops.is_empty());
    let result = sut.view().result.expect("stake result");
    assert_eq!(result.intent, "Stake");
    assert_eq!(result.risk, ClearRisk::Safe);
    assert_eq!(result.fields[0].value, "1 ETH");
}

#[test]
fn wsteth_wrap_resolves_token_from_metadata_constant() {
    let mut sut = Sut::new();
    let data = format!("0xea598cb0{}", pad("de0b6b3a7640000"));
    let ops = resolve_tx(
        &mut sut,
        "0x7f39c581f595b53c5cb19bd0b3f8da6c935e2ca0",
        &data,
        "0x0",
    );
    assert!(ops.is_empty());
    let result = sut.view().result.expect("wrap result");
    assert_eq!(result.intent, "Wrap");
    assert!(result.fields[0].value.contains("stETH"));
    assert!(result.fields[0].value.starts_with('1'));
}

// ---------------------------------------------------------------------------
// Best-effort via the 4-byte selector DB
// ---------------------------------------------------------------------------

/// clear-signing.ts:450-480 — with no descriptor anywhere, the selector DB
/// recovers the function: decoded generically, risk caution, `best_effort`,
/// params tucked into the detail panel.
#[test]
fn best_effort_decode_never_blind_signs_silently() {
    let mut sut = Sut::new();
    let sig = "mintTokens(address to,uint256 amount)";
    let selector = compute_selector(sig).expect("selector");
    let data = format!("{selector}{}{}", pad(VITALIK), pad_u128(1_000_000));
    let target = "0x7777777777777777777777777777777777777777";

    resolve_tx(&mut sut, target, &data, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{target}.json"),
        json: None,
    });
    sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc20-tokens.json".to_owned(),
        json: None,
    });
    sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc721-nfts.json".to_owned(),
        json: None,
    });
    let ops = sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/calldata-erc4626-vaults.json".to_owned(),
        json: None,
    });
    assert_eq!(ops, vec![Op::SelectorDbLookup { selector }]);

    // The first candidate is garbage; the machine keeps the one that decodes.
    let ops = sut.resolve(Res::SelectorCandidates {
        sigs: vec!["garbage(".to_owned(), sig.to_owned()],
    });
    assert!(ops.is_empty());

    let result = sut.view().result.expect("best-effort result");
    assert_eq!(result.intent, "Mint tokens", "humanized function name");
    assert!(result.best_effort);
    assert!(!result.verified);
    assert_eq!(result.risk, ClearRisk::Caution, "decoded but unverified");
    assert!(result.contract_name.is_none());

    let to = &result.fields[0];
    assert_eq!(to.label, "to");
    assert!(to.detail, "best-effort params live in the detail panel");
    assert_eq!(to.address.as_deref(), Some(VITALIK));
    let amount = &result.fields[1];
    assert_eq!(amount.label, "amount");
    assert_eq!(amount.value, "1,000,000");
}

// ---------------------------------------------------------------------------
// EIP-712 typed data
// ---------------------------------------------------------------------------

fn permit_typed_data() -> String {
    json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" },
            ],
            "Permit": [
                { "name": "owner", "type": "address" },
                { "name": "spender", "type": "address" },
                { "name": "value", "type": "uint256" },
                { "name": "nonce", "type": "uint256" },
                { "name": "deadline", "type": "uint256" },
            ],
        },
        "primaryType": "Permit",
        "domain": {
            "name": "USD Coin",
            "chainId": 1,
            "verifyingContract": "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
        },
        "message": {
            "owner": "0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1",
            "spender": "0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD",
            "value": "1000000000",
            "nonce": "0",
            "deadline": "1750000000",
        },
    })
    .to_string()
}

fn permit_descriptor() -> String {
    json!({
        "context": { "eip712": {} },
        "display": { "formats": {
            "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)": {
                "intent": "Authorize spending of tokens",
                "fields": [
                    { "path": "spender", "label": "Spender", "format": "raw", "visible": "always" },
                    { "path": "value", "label": "Max spending amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" }, "visible": "always" },
                    { "path": "deadline", "label": "Valid until", "format": "date", "params": { "encoding": "timestamp" } },
                    { "path": "owner", "label": "Owner", "visible": "never" },
                    { "path": "nonce", "label": "Nonce", "visible": "never" },
                ],
            },
        } },
    })
    .to_string()
}

#[test]
fn eip712_permit_resolves_via_erc2612_fallback() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::ResolveTypedData {
        typed_data_json: permit_typed_data(),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert_eq!(ops, vec![Op::Now]);
    let ops = sut.resolve(Res::Clock { now_ms: NOW });
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: format!("/erc7730/eip712/eip155-1/{USDC}.json"),
        }]
    );
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/eip712/eip155-1/{USDC}.json"),
        json: None,
    });
    assert_eq!(
        ops,
        vec![Op::HttpGet {
            path: "/erc7730/ercs/eip712-erc2612-permit.json".to_owned(),
        }]
    );
    let ops = sut.resolve(Res::DescriptorFetched {
        path: "/erc7730/ercs/eip712-erc2612-permit.json".to_owned(),
        json: Some(permit_descriptor()),
    });
    assert!(ops.is_empty());

    let result = sut.view().result.expect("permit result");
    assert_eq!(result.intent, "Authorize spending of tokens");
    assert_eq!(result.sign_type, ClearSignType::Signature);
    assert!(!result.verified, "ERC fallback is not contract-specific");
    assert_eq!(result.contract_address.as_deref(), Some(USDC));

    let labels: Vec<&str> = result.fields.iter().map(|f| f.label.as_str()).collect();
    assert!(labels.contains(&"Spender"));
    assert!(!labels.contains(&"Owner"), "visible: never is filtered");
    assert!(!labels.contains(&"Nonce"));

    let amount = result
        .fields
        .iter()
        .find(|f| f.label == "Max spending amount")
        .expect("amount field");
    assert_eq!(
        amount.value, "1,000 USDC",
        "@.to binds the verifying contract"
    );
    assert_eq!(amount.usd_value, Some(1000.0));

    let deadline = result
        .fields
        .iter()
        .find(|f| f.label == "Valid until")
        .expect("deadline field");
    assert!(deadline.expired, "June 2025 deadline is past NOW");
    assert_eq!(result.risk, ClearRisk::Caution);
}

#[test]
fn eip712_contract_entry_is_keyed_by_typehash_and_verified() {
    let mut sut = Sut::new();
    let encode_type =
        "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)";
    let type_hash = {
        let hash = vela_core::primitives::keccak256(encode_type.as_bytes());
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    };
    let mut root = serde_json::Map::new();
    root.insert(
        type_hash,
        json!({
            "metadata": { "contractName": "USD Coin", "owner": "Circle" },
            "display": { "formats": {
                "Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)": {
                    "intent": "Permit",
                    "fields": [
                        { "path": "spender", "label": "Spender", "format": "addressName" },
                    ],
                },
            } },
        }),
    );
    let body = serde_json::Value::Object(root).to_string();

    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: permit_typed_data(),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    sut.resolve(Res::Clock { now_ms: NOW });
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/eip712/eip155-1/{USDC}.json"),
        json: Some(body),
    });
    assert!(ops.is_empty(), "entry matched — no permit fallback fetch");

    let result = sut.view().result.expect("entry result");
    assert_eq!(result.intent, "Permit");
    assert!(result.verified);
    assert_eq!(result.contract_name.as_deref(), Some("USD Coin"));
    assert_eq!(result.owner.as_deref(), Some("Circle"));
    assert_eq!(result.fields[0].role, ClearFieldRole::Spender);
    assert_eq!(result.risk, ClearRisk::Caution, "permit intent");
}

#[test]
fn eip712_without_verifying_contract_is_blind() {
    let mut sut = Sut::new();
    let typed = json!({
        "types": { "EIP712Domain": [], "Test": [{ "name": "x", "type": "uint256" }] },
        "primaryType": "Test",
        "domain": {},
        "message": { "x": "1" },
    })
    .to_string();
    let ops = sut.dispatch(Event::ResolveTypedData {
        typed_data_json: typed,
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert!(ops.is_empty());
    assert!(sut.view().resolved);
    assert!(sut.view().result.is_none());
}

#[test]
fn eip712_unparseable_json_is_blind() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::ResolveTypedData {
        typed_data_json: "{not json".to_owned(),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert!(ops.is_empty());
    assert!(sut.view().resolved);
    assert!(sut.view().result.is_none());
}

// ---------------------------------------------------------------------------
// Staleness + supersession
// ---------------------------------------------------------------------------

/// A slower previous request must never overwrite the current one
/// (`SigningSheet.tsx:242-244`) — the attempt guard drops its late result.
#[test]
fn stale_resolution_result_is_dropped() {
    let mut sut = Sut::new();
    let transfer_a = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer_a, "0x0");

    // Request B supersedes while A's descriptor fetch is in flight.
    let transfer_b = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(77_000_000));
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some(UNKNOWN_TOKEN.to_owned()),
        data: Some(transfer_b),
        value: Some("0x0".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert_eq!(ops, vec![Op::Now]);

    // A's answer lands late — with a descriptor that WOULD have resolved.
    let descriptor = json!({
        "display": { "formats": { "transfer(address to,uint256 amount)": {
            "intent": "Send", "fields": [
                { "path": "amount", "label": "Amount", "format": "raw" },
            ],
        } } }
    })
    .to_string();
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: Some(descriptor),
    });
    assert!(ops.is_empty(), "stale answer drives nothing");
    let view = sut.view();
    assert!(view.resolving, "run B is still in flight");
    assert!(view.result.is_none(), "A's late result never lands");
}

/// A message request supersedes an in-flight resolution outright.
#[test]
fn message_presented_cancels_inflight_resolution() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");

    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![text_hex("hello")],
        request_origin: None,
    });
    let view = sut.view();
    assert!(!view.resolving);
    assert!(view.message.is_some());

    // The abandoned descriptor answer changes nothing.
    let ops = sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    assert!(ops.is_empty());
    let view = sut.view();
    assert!(view.message.is_some());
    assert!(view.result.is_none());
}

#[test]
fn cleared_resets_the_surface() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    assert!(sut.view().result.is_some());

    let _ = sut.dispatch(Event::Cleared);
    let view = sut.view();
    assert!(!view.resolving);
    assert!(!view.resolved);
    assert!(view.result.is_none());
    assert!(view.message.is_none());
}

// ---------------------------------------------------------------------------
// Sheet dispatch verdicts (inventory ⑨, ⑩, ㉑, ㉓, ㉔)
// ---------------------------------------------------------------------------

/// Deliberately a RAW literal, not `json!`: `serde_json::Map` is a `BTreeMap`
/// here, so building the fixture through `json!` would alphabetise `message`
/// before the projection ever saw it — and the ordering is what this asserts.
fn typed_json() -> &'static str {
    r#"{
        "types": {
            "EIP712Domain": [{ "name": "name", "type": "string" }],
            "CustomOrder": [{ "name": "maker", "type": "address" }]
        },
        "primaryType": "CustomOrder",
        "domain": { "name": "Unknown Protocol", "verifyingContract": "0x1234567890ABCDEF1234567890abcdef12345678" },
        "message": {
            "maker": "0xaF5e8917831Ef08A64e18b2Cde9f8f5D32C7b3e1",
            "amount": "5000000000000000000",
            "expiry": "1750000000",
            "salt": "0xabcdef1234567890abcdef1234567890abcdef1234567890abcdef1234567890"
        }
    }"#
}

fn start_typed(sut: &mut Sut, raw: &str) -> Vec<Op> {
    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: raw.to_owned(),
        chain_id: 1,
        locale: ClearLocale::default(),
    })
}

/// ⑨ — a descriptor still resolving holds the sheet on `Loading`. A blind
/// surface must never flash before the clear one.
#[test]
fn surface_holds_loading_until_resolution_concludes() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    assert_eq!(sut.view().surface, ClearSurface::Loading);

    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    assert_eq!(sut.view().surface, ClearSurface::ClearSign);
}

/// ⑨ — an undecodable contract call lands on the blind TRANSACTION surface,
/// an undecodable typed payload on the blind TYPED one. Same "no result",
/// two different screens.
#[test]
fn blind_outcomes_split_by_request_kind() {
    let mut sut = Sut::new();
    // A selector no descriptor and no 4-byte entry knows.
    let ops = resolve_tx(&mut sut, UNKNOWN_TOKEN, "0xdeadbeef", "0x0");
    assert!(!ops.is_empty());
    loop {
        let outstanding = sut.outstanding();
        if outstanding.is_empty() {
            break;
        }
        match &outstanding[0] {
            Op::HttpGet { path } => {
                let path = path.clone();
                sut.resolve(Res::DescriptorFetched { path, json: None });
            }
            Op::SelectorDbLookup { .. } => {
                sut.resolve(Res::SelectorCandidates { sigs: vec![] });
            }
            other => panic!("unexpected op {other:?}"),
        }
    }
    assert!(sut.view().result.is_none());
    assert_eq!(sut.view().surface, ClearSurface::BlindTransaction);
    assert_eq!(sut.view().confirm, ClearConfirm::Confirm);

    let mut sut = Sut::new();
    // No `verifyingContract` — resolves blind immediately.
    start_typed(&mut sut, r#"{"primaryType":"X","domain":{},"message":{}}"#);
    assert_eq!(sut.view().surface, ClearSurface::BlindTypedData);
    assert_eq!(sut.view().confirm, ClearConfirm::Sign);
}

/// ⑨ — `eth_sign` never reaches the calm message view, even though both
/// arrive as `MessagePresented`.
#[test]
fn eth_sign_and_personal_sign_take_different_surfaces() {
    let mut sut = Sut::new();
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![text_hex("hello")],
        request_origin: None,
    });
    assert_eq!(sut.view().surface, ClearSurface::MessageSign);
    assert!(!sut.view().danger_haptic);

    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::EthSign,
        params: vec![
            "0x0000000000000000000000000000000000000000".to_owned(),
            "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658".to_owned(),
        ],
        request_origin: None,
    });
    assert_eq!(sut.view().surface, ClearSurface::EthSign);
    assert!(sut.view().danger_haptic, "eth_sign always buzzes");
}

/// ⑩ — `eth_sign(address, data)` displays `params[1]`; showing `params[0]`
/// would put the ADDRESS where the opaque digest belongs. A malformed
/// single-param request still shows what it has.
#[test]
fn eth_sign_reads_the_second_param_and_falls_back_to_the_first() {
    let digest = "0x9c22ff5f21f0b81b113e63f7db6da94fedef11b2119b4088b89664fb9a3cb658";
    let mut sut = Sut::new();
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::EthSign,
        params: vec![
            "0x0000000000000000000000000000000000000000".to_owned(),
            digest.to_owned(),
        ],
        request_origin: None,
    });
    assert_eq!(sut.view().message.expect("analyzed").payload, digest);

    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::EthSign,
        params: vec![digest.to_owned()],
        request_origin: None,
    });
    assert_eq!(sut.view().message.expect("analyzed").payload, digest);

    // personal_sign always signs params[0]; params[1] is the account.
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![
            text_hex("hello"),
            "0x0000000000000000000000000000000000000000".to_owned(),
        ],
        request_origin: None,
    });
    assert_eq!(
        sut.view().message.expect("analyzed").payload,
        text_hex("hello")
    );
}

/// ㉑ — the haptic and the red banner come from ONE adjudication, so they can
/// never disagree.
#[test]
fn siwe_phishing_drives_the_same_verdict_as_the_banner() {
    let mut sut = Sut::new();
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![text_hex(&siwe_text())],
        request_origin: Some("https://uniswap-airdrop.xyz".to_owned()),
    });
    let view = sut.view();
    assert_eq!(
        view.message.expect("analyzed").danger_class,
        ClearDangerClass::SiwePhish
    );
    assert!(view.danger_haptic);

    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![text_hex(&siwe_text())],
        request_origin: Some("https://app.uniswap.org".to_owned()),
    });
    let view = sut.view();
    assert_eq!(
        view.message.expect("analyzed").danger_class,
        ClearDangerClass::SiweOk
    );
    assert!(!view.danger_haptic, "a bound sign-in never buzzes");
}

/// ㉓ — the raw typed projection keeps PAYLOAD order, caps at five rows and
/// mid-truncates a long hex blob. Alphabetising would change which five rows
/// the user reads.
#[test]
fn blind_typed_projection_keeps_payload_order_and_truncates() {
    let mut sut = Sut::new();
    start_typed(&mut sut, typed_json());
    let blind = sut.view().blind_typed.expect("projected up front");
    assert_eq!(blind.primary_type.as_deref(), Some("CustomOrder"));
    assert!(blind.has_domain);
    assert_eq!(blind.domain_name.as_deref(), Some("Unknown Protocol"));
    assert_eq!(
        blind.verifying_contract.as_deref(),
        Some("0x1234567890abcdef1234567890abcdef12345678"),
        "lowercased for the explorer link"
    );
    let keys: Vec<&str> = blind.fields.iter().map(|f| f.key.as_str()).collect();
    assert_eq!(keys, vec!["maker", "amount", "expiry", "salt"]);
    assert_eq!(
        blind.fields[3].value, "0xabcdef12…34567890",
        "a 32-byte salt is mid-truncated, never a two-line hex wall"
    );
    assert_eq!(blind.fields[1].value, "5000000000000000000");

    // Six fields → only the first five are shown.
    let mut sut = Sut::new();
    start_typed(
        &mut sut,
        r#"{"message":{"a":1,"b":2,"c":3,"d":4,"e":5,"f":6}}"#,
    );
    let blind = sut.view().blind_typed.expect("projected");
    assert_eq!(blind.fields.len(), 5);
    assert_eq!(blind.fields[0].value, "1");
    assert!(!blind.has_domain);
}

/// ㉓ — a payload that isn't even an object projects to nothing rather than
/// throwing; the caution banner still renders.
#[test]
fn blind_typed_projection_survives_hostile_payloads() {
    let mut sut = Sut::new();
    start_typed(&mut sut, "\"not an object\"");
    let blind = sut.view().blind_typed.expect("projected");
    assert!(blind.fields.is_empty());
    assert!(blind.primary_type.is_none());
    assert_eq!(sut.view().surface, ClearSurface::BlindTypedData);

    let mut sut = Sut::new();
    start_typed(&mut sut, "{not json");
    let blind = sut.view().blind_typed.expect("projected");
    assert!(blind.fields.is_empty());
    assert_eq!(sut.view().surface, ClearSurface::BlindTypedData);
}

/// ㉔ — confirm semantics, never the words. A signature reads "Sign"; a
/// decoded transaction reads "Confirm {intent}"; a plain native transfer
/// reads "Confirm Send" to match its own eyebrow.
#[test]
fn confirm_semantics_follow_the_resolved_request() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    let view = sut.view();
    let intent = view.result.expect("decoded").intent;
    assert_eq!(view.confirm, ClearConfirm::ConfirmIntent { intent });

    // A plain native send — no calldata, nothing to resolve.
    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTransaction {
        to: Some(VITALIK.to_owned()),
        data: Some("0x".to_owned()),
        value: Some("0xde0b6b3a7640000".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    let view = sut.view();
    assert!(view.resolved && !view.resolving);
    assert_eq!(view.surface, ClearSurface::BlindTransaction);
    assert_eq!(
        view.confirm,
        ClearConfirm::ConfirmIntent {
            intent: "send".to_owned()
        }
    );

    // A message is always "Sign"; eth_sign falls back to the neutral verb.
    let mut sut = Sut::new();
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::PersonalSign,
        params: vec![text_hex("hello")],
        request_origin: None,
    });
    assert_eq!(sut.view().confirm, ClearConfirm::Sign);
    sut.dispatch(Event::MessagePresented {
        method: ClearSignMethod::EthSign,
        params: vec![text_hex("hello")],
        request_origin: None,
    });
    assert_eq!(sut.view().confirm, ClearConfirm::Confirm);

    // Nothing presented — neutral, never "Approve".
    let mut sut = Sut::new();
    sut.dispatch(Event::Cleared);
    assert_eq!(sut.view().surface, ClearSurface::None);
    assert_eq!(sut.view().confirm, ClearConfirm::Confirm);
}

/// ② — a CREATE2 deployment is a calm "Deploy contract" that resolves
/// synchronously onto the clear surface, never a red blind "Unknown".
#[test]
fn create2_deployment_resolves_calmly_on_the_clear_surface() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::ResolveTransaction {
        to: Some("0x4E59b44847b379578588920cA78FbF26c0B4956C".to_owned()),
        data: Some(
            "0x0000000000000000000000000000000000000000000000000000000000000000600a".to_owned(),
        ),
        value: Some("0x0".to_owned()),
        chain_id: 1,
        locale: ClearLocale::default(),
    });
    assert!(ops.is_empty(), "deployment detection needs no network");
    let view = sut.view();
    assert_eq!(view.surface, ClearSurface::ClearSign);
    assert_eq!(
        view.result.expect("deploy result").intent,
        "Deploy contract"
    );
}

/// ㉓ — numbers print the way the engine that rendered today's screen prints
/// them. `serde_json` alone would show `1e21` as `1000000000000000000000`
/// and an integral float as `100.0`.
#[test]
fn blind_typed_numbers_print_as_javascript_does() {
    let mut sut = Sut::new();
    start_typed(
        &mut sut,
        r#"{"message":{"int":100,"huge":1e21,"tiny":1e-7,"frac":1.5}}"#,
    );
    let blind = sut.view().blind_typed.expect("projected");
    let values: Vec<&str> = blind.fields.iter().map(|f| f.value.as_str()).collect();
    assert_eq!(values, vec!["100", "1e+21", "1e-7", "1.5"]);
}

/// ㉓ — a nested struct is rendered as JSON in the order the payload wrote it.
/// Alphabetising it (which `serde_json::Map` would) changes a line the user
/// reads on a security surface.
#[test]
fn blind_typed_nested_struct_keeps_its_own_key_order() {
    let mut sut = Sut::new();
    start_typed(
        &mut sut,
        r#"{"message":{"details":{"token":"0xAA","amount":"1000000000","expiration":1799999999}}}"#,
    );
    let blind = sut.view().blind_typed.expect("projected");
    assert_eq!(
        blind.fields[0].value,
        r#"{"token":"0xAA","amount":"1000000000","expiration":179999999"#
    );
}

// ---------------------------------------------------------------------------
// Burn detection — a token sent to its OWN contract
// ---------------------------------------------------------------------------
//
// The single-call twin of `approval_guard`'s `any_to_own_token`. It lives here
// because this machine is the one that owns BOTH facts it needs (the resolved
// recipient fields and the contract being called), and because a burn that
// warns in a batch and stays silent on its own is the worst possible split —
// the single send is the far more common entry point.

/// The rule itself: a `transfer` whose recipient is the token contract.
#[test]
fn transfer_to_the_token_itself_is_flagged_as_a_burn() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(USDC), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });

    let result = sut.view().result.expect("clear-sign result");
    assert_eq!(result.fields[1].role, ClearFieldRole::Recipient);
    assert!(
        result.to_own_token,
        "recipient IS the contract being called — the tokens are burned",
    );
}

/// A dApp routinely sends `tx.to` EIP-55 CHECKSUMMED while every resolved field
/// address is lowercased. Two independent things keep that from hiding a burn:
/// `start_tx` normalises `to` at ingest, and the rule compares
/// case-insensitively anyway (`to_own_token`'s own unit test). This pins the
/// OUTCOME, so losing either one is a failing test rather than a silent
/// downgrade of a danger banner to nothing.
#[test]
fn a_checksummed_tx_to_does_not_hide_the_burn() {
    const USDC_EIP55: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(USDC_EIP55), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC_EIP55, &transfer, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });

    let result = sut.view().result.expect("clear-sign result");
    assert_eq!(
        result.contract_address.as_deref(),
        Some(USDC),
        "`tx.to` is normalised at ingest",
    );
    assert_eq!(result.fields[1].address.as_deref(), Some(USDC));
    assert!(result.to_own_token, "case must not decide a burn warning");
}

/// The control: an ordinary transfer to a wallet is not a burn.
#[test]
fn transfer_to_a_wallet_is_not_a_burn() {
    let mut sut = Sut::new();
    let transfer = format!("0xa9059cbb{}{}", pad(VITALIK), pad_u128(1_000_000_000));
    resolve_tx(&mut sut, USDC, &transfer, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });

    assert!(!sut.view().result.expect("result").to_own_token);
}

/// A SPENDER equal to the contract is not a burn — `approve(token, …)` grants an
/// allowance, it moves nothing. Only the `recipient` role counts, exactly as in
/// `approval_guard`, whose recipient lists come from the same role filter.
#[test]
fn a_spender_equal_to_the_contract_is_not_a_burn() {
    let mut sut = Sut::new();
    let approve = format!("0x095ea7b3{}{}", pad(USDC), pad_u128(1_000_000));
    resolve_tx(&mut sut, USDC, &approve, "0x0");
    sut.resolve(Res::DescriptorFetched {
        path: format!("/erc7730/calldata/eip155-1/{USDC}.json"),
        json: None,
    });
    // approve shares its selector with ERC-721 — settle the disambiguation.
    for probe in [ClearProbe::SupportsErc721, ClearProbe::SupportsErc1155] {
        sut.resolve(Res::RpcAnswer {
            probe,
            chain_id: 1,
            to: USDC.to_owned(),
            result: None,
            rpc_error: true,
        });
    }

    let result = sut.view().result.expect("result");
    assert!(
        result
            .fields
            .iter()
            .all(|f| f.role != ClearFieldRole::Recipient),
        "approve resolves a spender, never a recipient",
    );
    assert!(!result.to_own_token);
}
