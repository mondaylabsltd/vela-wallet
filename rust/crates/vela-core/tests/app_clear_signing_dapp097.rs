//! Spec 097 part A — the signing sheet never states something false or
//! unknown as certain, on the requests of the second real-money dApp pass.
//!
//! Every fixture under `tests/fixtures/dapp097/` is a request a real dApp
//! sent the extension on 2026-10-03 (1inch, Aave, PancakeSwap, Uniswap,
//! Curve on BNB Chain), verbatim. The shell here answers as the pass's did:
//! Vela's descriptor service serves the two descriptors it served then
//! (`fixtures/dapp097/descriptors/`, copied from the chain-data repository),
//! the chain answers `decimals()`/`symbol()` for the tokens these dApps
//! touched, and the public 4-byte database answers unnamed signatures.
//!
//! The rules pinned (097 N1, N2, N3, N6, N8 sheet part): an address the ABI
//! carries as a number reads as that address; an amount nobody can scale is
//! never a figure and leaves the reading incomplete; a borrow is money in; a
//! minimum says it is one; a 1inch order says where its proceeds go, until
//! when it stands and which coin arrives; the Curve router and a token the
//! chain alone named are named — the latter never without its address.

#![cfg(feature = "crux")]

mod support;

use serde_json::Value;
use support::DomainDriver;
use vela_core::app::clear_signing::{
    ClearAmountBound, ClearDateFormat, ClearFieldRole, ClearLocale, ClearOperation as Op,
    ClearProbe, ClearProvenance, ClearShellResult as Res, ClearSignField, ClearSignResult,
    ClearSignType, ClearSigning, ClearSigningView, ClearSurface, ClearTerm, ClearTimeFormat, Event,
    UNKNOWN_AMOUNT,
};

type Sut = DomainDriver<ClearSigning>;

/// 2026-10-02 16:03:20 UTC — minutes before the pass's first request.
const NOW: f64 = 1_790_957_000_000.0;

const WALLET: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
const WBNB: &str = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
const USDC_BSC: &str = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d";
const USDT_BSC: &str = "0x55d398326f99059ff775485246999027b3197955";
const CURVE_ROUTER: &str = "0xa72c85c258a81761433b4e8da60505fe3dd551cc";
/// Ethereum's USDC. On BNB Chain these bytes are no token the wallet knows.
const USDC_ETHEREUM: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

/// What the chain answers about the tokens these dApps touched:
/// `(token, decimals, symbol)`, all on BNB Chain. The last is the 097 D
/// stranger: a contract at Ethereum's USDC address that answers "USDC".
const TOKENS: &[(&str, u32, &str)] = &[
    (WBNB, 18, "WBNB"),
    (USDC_BSC, 18, "USDC"),
    (USDT_BSC, 18, "USDT"),
    (USDC_ETHEREUM, 6, "USDC"),
];

/// What the descriptor service served in the pass, by path.
const DESCRIPTORS: &[(&str, &str)] = &[
    (
        "/erc7730/calldata/eip155-56/0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01.json",
        "calldata-eip155-56-0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01.json",
    ),
    (
        "/erc7730/eip712/eip155-56/0x111111125421ca6dc452d289314280a0f8842a65.json",
        "eip712-eip155-56-0x111111125421ca6dc452d289314280a0f8842a65.json",
    ),
];

/// What the public 4-byte database answers: signatures with no names.
const SELECTOR_DB: &[(&str, &str)] = &[(
    "0x5c9c18e2",
    "exchange(address[11],uint256[5][5],uint256,uint256,address[5])",
)];

/// Beijing time, ISO dates, 24-hour clock — the owner's own settings.
fn beijing() -> ClearLocale {
    ClearLocale {
        date_format: ClearDateFormat::Iso,
        time_format: ClearTimeFormat::H24,
        tz_offset_minutes: 480,
        ..ClearLocale::default()
    }
}

/// The same, at UTC — to read a date against the chain's own clock.
fn utc() -> ClearLocale {
    ClearLocale {
        tz_offset_minutes: 0,
        ..beijing()
    }
}

fn read_text(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/dapp097/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&read_text(&path)).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn descriptor_body(path: &str) -> Option<String> {
    DESCRIPTORS
        .iter()
        .find(|(served, _)| *served == path)
        .map(|(_, file)| {
            read_text(&format!(
                "{}/tests/fixtures/dapp097/descriptors/{file}",
                env!("CARGO_MANIFEST_DIR")
            ))
        })
}

fn abi_word(n: u64) -> String {
    format!("0x{n:064x}")
}

fn abi_string(text: &str) -> String {
    let mut hex = format!("{:064x}{:064x}", 32, text.len());
    let mut body: String = text.bytes().map(|b| format!("{b:02x}")).collect();
    while body.len() % 64 != 0 {
        body.push('0');
    }
    hex.push_str(&body);
    format!("0x{hex}")
}

/// The pass's shell (module doc); `chain_silent` makes every token call fail,
/// as a node that is down would.
fn answer(op: &Op, chain_silent: bool) -> Res {
    match op {
        Op::Now => Res::Clock { now_ms: NOW },
        Op::HttpGet { path } => Res::DescriptorFetched {
            path: path.clone(),
            json: descriptor_body(path),
        },
        Op::RpcEthCall {
            chain_id,
            to,
            probe,
            ..
        } => {
            let token = TOKENS
                .iter()
                .find(|(addr, _, _)| *chain_id == 56 && addr.eq_ignore_ascii_case(to))
                .filter(|_| !chain_silent);
            let result = match (probe, token) {
                (ClearProbe::Decimals, Some((_, decimals, _))) => {
                    Some(abi_word(u64::from(*decimals)))
                }
                (ClearProbe::Symbol, Some((_, _, symbol))) => Some(abi_string(symbol)),
                _ => None,
            };
            Res::RpcAnswer {
                probe: *probe,
                chain_id: *chain_id,
                to: to.clone(),
                rpc_error: result.is_none(),
                result,
            }
        }
        Op::SelectorDbLookup { selector } => Res::SelectorCandidates {
            sigs: SELECTOR_DB
                .iter()
                .filter(|(sel, _)| sel == selector)
                .map(|(_, sig)| (*sig).to_owned())
                .collect(),
        },
        Op::Timer { token, .. } => Res::TimedOut { token: *token },
    }
}

fn drain(sut: &mut Sut, chain_silent: bool) {
    for _ in 0..500 {
        let Some(op) = sut.outstanding().first().cloned() else {
            return;
        };
        sut.resolve(answer(&op, chain_silent));
    }
    panic!("the request never finished reading");
}

/// The event a shell dispatches for one request.
fn event_of(request: &Value, chain_id: u32, locale: ClearLocale) -> Event {
    let params = &request["params"];
    match request["method"].as_str().expect("method") {
        "eth_sendTransaction" => {
            let tx = &params[0];
            let text = |key: &str| tx.get(key).and_then(Value::as_str).map(str::to_owned);
            Event::ResolveTransaction {
                to: text("to"),
                data: text("data"),
                value: text("value"),
                chain_id,
                locale,
            }
        }
        "wallet_sendCalls" => Event::ResolveBatch {
            params_json: params.to_string(),
            chain_id,
            locale,
        },
        method if method.starts_with("eth_signTypedData") => Event::ResolveTypedData {
            typed_data_json: params[1].as_str().expect("typed data").to_owned(),
            chain_id,
            locale,
        },
        other => panic!("no reader for {other}"),
    }
}

fn resolve(event: Event, chain_silent: bool) -> ClearSigningView {
    let mut sut = Sut::new();
    sut.dispatch(event);
    drain(&mut sut, chain_silent);
    let view = sut.view();
    assert!(!view.resolving, "still reading");
    view
}

/// The sheet's reading of one fixture, every question answered.
fn read_in(name: &str, locale: ClearLocale) -> ClearSigningView {
    let request = fixture(name);
    let chain_id = u32::try_from(request["chainId"].as_u64().expect("chainId")).expect("chain");
    resolve(event_of(&request, chain_id, locale), false)
}

fn read(name: &str) -> ClearSigningView {
    read_in(name, beijing())
}

/// `label: value` for every field a shell draws (detail rows drop out on
/// three of the four shells, so they are not "the readable part").
fn rows(result: &ClearSignResult) -> Vec<String> {
    result
        .fields
        .iter()
        .filter(|f| !f.detail)
        .map(|f| format!("{}: {}", f.label, f.value))
        .collect()
}

/// The readable part of a sheet, as plain lines — what the report quotes.
fn readable(view: &ClearSigningView) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(result) = &view.result {
        lines.push(format!("[{}]", result.intent));
        if let Some(native) = &view.native_value {
            lines.push(format!("Amount: −{} (coin)", native.amount));
        }
        lines.extend(rows(result));
        if result.partial {
            lines.push("⚠ incomplete".to_owned());
        }
    }
    if let Some(batch) = &view.batch {
        lines.push(format!("[Batch of {}]", batch.calls.len()));
        for call in &batch.calls {
            let intent = call.result.as_ref().map_or("?", |r| r.intent.as_str());
            lines.push(format!("  {} · {}", call.index, intent));
            if let Some(result) = &call.result {
                lines.extend(rows(result).into_iter().map(|row| format!("    {row}")));
            }
            lines.push(format!(
                "    Interacting with: {}",
                call.to_name.clone().or(call.to.clone()).unwrap_or_default()
            ));
        }
    }
    if view.result.is_none() && view.batch.is_none() {
        lines.push(format!("<{:?}>", view.surface));
    }
    lines
}

fn has(lines: &[String], line: &str) -> bool {
    lines.iter().any(|l| l.trim() == line)
}

fn field<'a>(result: &'a ClearSignResult, label: &str) -> &'a ClearSignField {
    result
        .fields
        .iter()
        .find(|f| f.label == label)
        .unwrap_or_else(|| panic!("no {label:?} row in {:#?}", result.fields))
}

/// The native-order request with one 32-byte word of its calldata replaced:
/// word 0 is the tuple's `salt`, 1 `maker`, 2 `receiver`, 3 `makerAsset`, 4
/// `takerAsset`, 5 `makingAmount`, 6 `takingAmount`, 7 `makerTraits`.
fn native_order_with_word(word: usize, hex64: &str) -> Event {
    let request = fixture("oneinch-native-order.json");
    let tx = &request["params"][0];
    let data = tx["data"].as_str().expect("data");
    let at = 10 + word * 64;
    let edited = format!("{}{hex64}{}", &data[..at], &data[at + 64..]);
    Event::ResolveTransaction {
        to: tx["to"].as_str().map(str::to_owned),
        data: Some(edited),
        value: tx["value"].as_str().map(str::to_owned),
        chain_id: 56,
        locale: beijing(),
    }
}

// ---------------------------------------------------------------------------
// N1 — 1inch's native order: the beneficiary, and a token nobody can scale
// ---------------------------------------------------------------------------

/// 1inch BNB → USDC (NativeOrderFactory `create`), read through the
/// descriptor Vela's service served. Before 097: "+2.418… tokens" — as sure
/// as any figure — and "Beneficiary 78098611...991444", the account itself
/// written as a `uint256`.
#[test]
fn oneinch_native_order_reads_its_beneficiary_and_its_token() {
    let view = read("oneinch-native-order.json");
    assert_eq!(view.surface, ClearSurface::ClearSign);
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[create order]",
            "Amount to Send: 0.003 BNB",
            "Receive amount: 2.418146082462759045 USDC",
            "Beneficiary: 0x88cca0...266894",
        ]
    );
    let result = view.result.expect("read");
    assert_eq!(result.provenance, ClearProvenance::Fetched);
    assert_eq!(result.contract_name.as_deref(), Some("NativeOrderFactory"));
    // The uint256 holds the account: drawn as the address it is, with its
    // full form, where every other recipient is drawn.
    let beneficiary = field(&result, "Beneficiary");
    assert_eq!(beneficiary.address.as_deref(), Some(WALLET));
    assert_eq!(beneficiary.role, ClearFieldRole::Recipient);
    // The token the uint256 names is the token the sheet asked the chain about.
    let receive = field(&result, "Receive amount");
    assert_eq!(receive.token_address.as_deref(), Some(USDC_BSC));
    assert!(!receive.unverified && !result.partial);
    assert_eq!(receive.role, ClearFieldRole::ReceiveAmount);
}

/// The same order for a token the wallet does not know on BNB Chain, with
/// the chain silent: the token is known by its address and nothing else, so
/// its amount is no figure — and the reading says it is incomplete. (097 D:
/// BNB Chain's USDC itself is the registry's now, so the derived fixture's
/// order names Ethereum's USDC address, which on this chain is nobody's
/// word.)
#[test]
fn an_amount_whose_token_never_answered_is_unknown_and_incomplete() {
    let request = fixture("oneinch-native-order-unlisted.json");
    let view = resolve(event_of(&request, 56, beijing()), true);
    let result = view.result.expect("read");
    let receive = field(&result, "Receive amount");
    assert_eq!(receive.value, format!("{UNKNOWN_AMOUNT} 0xa0b8..."));
    assert!(receive.unverified);
    assert_eq!(receive.usd_value, None);
    assert!(
        result.partial,
        "an unscaled amount leaves the reading incomplete"
    );
}

/// An amount whose descriptor names no token the reading can read — not an
/// address, not a number that holds one — is not "N tokens" at a guessed 18
/// decimals. Before 097 it was, and `unverified` was false.
#[test]
fn an_amount_with_no_token_is_never_a_guessed_figure() {
    // The takerAsset word as a number wider than 160 bits: no address.
    let view = resolve(native_order_with_word(4, &"f".repeat(64)), false);
    let result = view.result.expect("read");
    let receive = field(&result, "Receive amount");
    assert!(
        receive.value.starts_with(UNKNOWN_AMOUNT),
        "no figure: {}",
        receive.value
    );
    assert!(receive.unverified);
    assert!(result.partial);
}

/// A number wider than an address is shown whole — never cut to 160 bits,
/// never shortened into something that looks like one.
#[test]
fn an_address_row_holding_a_wider_number_shows_the_number() {
    // 2^160: one past the widest address.
    let two_160 = format!("{:0>64}", format!("1{}", "0".repeat(40)));
    let view = resolve(native_order_with_word(2, &two_160), false);
    let result = view.result.expect("read");
    let beneficiary = field(&result, "Beneficiary");
    assert_eq!(
        beneficiary.value,
        "1461501637330902918203684832716283019655932542976"
    );
    assert_eq!(beneficiary.address, None);
}

// ---------------------------------------------------------------------------
// N2 — a borrow is money coming in
// ---------------------------------------------------------------------------

/// Aave `borrow(USDC, 0.3)`. Before 097: "Borrow −0.3 USDC". A repay stays
/// what it is: money out.
#[test]
fn an_aave_borrow_reads_as_money_in_and_a_repay_as_money_out() {
    let borrow = read("aave-borrow.json").result.expect("read");
    assert_eq!(borrow.intent_term, Some(ClearTerm::IntentBorrow));
    let amount = field(&borrow, "Borrow");
    assert_eq!(amount.value, "0.3 USDC");
    assert_eq!(amount.role, ClearFieldRole::ReceiveAmount);

    let repay = read("aave-repay.json").result.expect("read");
    assert_eq!(repay.intent_term, Some(ClearTerm::IntentRepay));
    let amount = field(&repay, "Repay");
    assert_eq!(amount.role, ClearFieldRole::SendAmount);
}

// ---------------------------------------------------------------------------
// N3 — a minimum says it is one
// ---------------------------------------------------------------------------

/// PancakeSwap and Uniswap BNB → USDC: the USDC is the least the swap
/// accepts. The batch legs said "(min)"; a lone swap's hero dropped it. The
/// core now says it on the field itself, wherever the label is not drawn.
#[test]
fn a_single_swaps_minimum_is_marked_as_one() {
    for name in ["pancakeswap-bnb-swap.json", "uniswap-bnb-swap.json"] {
        let result = read(name).result.expect("read");
        let receive = field(&result, "You receive (min)");
        assert_eq!(receive.role, ClearFieldRole::ReceiveAmount, "{name}");
        assert_eq!(receive.bound, Some(ClearAmountBound::Min), "{name}");
        let pay = field(&result, "You pay");
        assert_eq!(pay.bound, None, "{name}: what is paid is exact");
    }
}

// ---------------------------------------------------------------------------
// N6 — a 1inch order says what it commits to
// ---------------------------------------------------------------------------

/// The 1inch USDC → BNB order signature. Before 097 (the descriptor the
/// service served): "1inch Order −1.16 USDC +0.00143… WBNB, To 0x00000000…
/// 00000000" — the zero receiver as a stranger, WBNB where BNB arrives, the
/// minimum unlabelled and the expiry nowhere.
#[test]
fn a_oneinch_order_reads_its_receiver_coin_minimum_and_expiry() {
    let view = read_in("oneinch-order.json", utc());
    assert_eq!(view.surface, ClearSurface::ClearSign);
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[Swap]",
            "You pay: 1.16 USDC",
            // UNWRAP_WETH: the protocol unwraps what the taker pays.
            "You receive (min): 0.001430509396956033 BNB",
            // A zero receiver is the maker: the account itself.
            "Recipient: 0x88cca0...266894",
            // makerTraits bits 80..120 — 1790959330, 16:42:10 UTC.
            "Valid until: 2026-10-02, 16:42",
        ]
    );
    let result = view.result.expect("read");
    // The app's own reading, though the service answered with its own.
    assert_eq!(result.provenance, ClearProvenance::BuiltIn);
    assert_eq!(result.sign_type, ClearSignType::Signature);
    assert_eq!(result.contract_name.as_deref(), Some("1inch Router (V6)"));
    assert_eq!(field(&result, "Recipient").address.as_deref(), Some(WALLET));
    let receive = field(&result, "You receive (min)");
    assert_eq!(receive.bound, Some(ClearAmountBound::Min));
    assert_eq!(receive.token_address, None, "the chain's own coin");
    assert_eq!(view.record_intent.as_deref(), Some("Swap"));
    // Beijing time reads the same moment.
    let beijing = read("oneinch-order.json").result.expect("read");
    assert_eq!(field(&beijing, "Valid until").value, "2026-10-03, 00:42");
}

/// An order that may fill in parts, keeps its wrapped coin and names its own
/// receiver: what leaves is the most that can, what comes back is not called
/// a minimum, WBNB stays WBNB, and the receiver is the one it names.
#[test]
fn a_oneinch_order_that_fills_in_parts_says_so() {
    let request = fixture("oneinch-order.json");
    let mut typed: Value =
        serde_json::from_str(request["params"][1].as_str().expect("typed")).expect("json");
    // Bits 255 and 247 cleared; the expiry and the rest kept.
    typed["message"]["makerTraits"] =
        Value::from("0x0a00000000000000000000001254000002006abfdee200000000000000000000");
    typed["message"]["receiver"] = Value::from("0x7777777777777777777777777777777777777777");
    let view = resolve(
        Event::ResolveTypedData {
            typed_data_json: typed.to_string(),
            chain_id: 56,
            locale: utc(),
        },
        false,
    );
    assert_eq!(
        readable(&view),
        vec![
            "[Swap]",
            "You pay (max): 1.16 USDC",
            "You receive: 0.001430509396956033 WBNB",
            "Recipient: 0x777777...777777",
            "Valid until: 2026-10-02, 16:42",
        ]
    );
}

// ---------------------------------------------------------------------------
// N8 — the Curve router, and tokens named only with their addresses
// ---------------------------------------------------------------------------

/// Curve's unlimited USDC approve of its BNB-Chain router. Before 097:
/// "Token USDC · Spender 0xa72c85...51cc"; the router is Curve's. 097 A put
/// the token's address beside "USDC", because the old address-only table
/// did not know BNB Chain's USDC; it is the wallet's registry stablecoin on
/// this chain, so it stands alone (097 D).
#[test]
fn curve_approve_names_the_router_and_the_registrys_usdc() {
    let view = read("curve-approve.json");
    assert_eq!(
        readable(&view),
        vec![
            "[Approve]",
            "Amount: Unlimited",
            "Token: USDC",
            "Spender: Curve Router",
        ]
    );
    let result = view.result.expect("read");
    let token = field(&result, "Token");
    assert_eq!(token.address, None);
    assert_eq!(token.token_address.as_deref(), Some(USDC_BSC));
    assert_eq!(
        field(&result, "Spender").address.as_deref(),
        Some(CURVE_ROUTER)
    );
}

/// A registry token keeps its symbol alone.
#[test]
fn a_registry_token_row_is_its_symbol_alone() {
    let usdc_mainnet = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    let data = format!("0x095ea7b3{:0>64}{}", &CURVE_ROUTER[2..], "f".repeat(64));
    let view = resolve(
        Event::ResolveTransaction {
            to: Some(usdc_mainnet.to_owned()),
            data: Some(data),
            value: Some("0x0".to_owned()),
            chain_id: 1,
            locale: beijing(),
        },
        false,
    );
    let result = view.result.expect("read");
    let token = field(&result, "Token");
    assert_eq!(token.value, "USDC");
    assert_eq!(token.address, None);
    // Not Curve's router on Ethereum: those bytes are someone else's there.
    assert_eq!(field(&result, "Spender").value, "0xa72c85...d551cc");
}

/// Curve's swap through its router: the 4-byte database reads the call, the
/// wallet names the contract — on BNB Chain only.
#[test]
fn the_curve_router_is_named_on_bnb_chain_only() {
    let on_bnb = read("curve-swap.json").result.expect("best effort");
    assert!(on_bnb.best_effort);
    assert_eq!(on_bnb.contract_name.as_deref(), Some("Curve Router"));
    assert_eq!(on_bnb.owner.as_deref(), Some("Curve"));

    let request = fixture("curve-swap.json");
    let elsewhere = resolve(event_of(&request, 1, beijing()), false)
        .result
        .expect("best effort");
    assert_eq!(elsewhere.contract_name, None);
}

/// PancakeSwap USDC → BNB, one batch: the approve's target is the USDC
/// contract. Before 097 "Interacting with" was its forty hex digits; it is
/// the registry's USDC on this chain, so its symbol alone (097 D).
#[test]
fn a_batch_call_on_a_token_names_the_token() {
    let view = read("pancakeswap-usdc-batch.json");
    let lines = readable(&view);
    for line in [
        "1 · Approve",
        "Interacting with: USDC",
        "Interacting with: PancakeSwap Permit2",
        "Interacting with: PancakeSwap Universal Router",
    ] {
        assert!(has(&lines, line), "missing {line:?} in {lines:#?}");
    }
}

// ---------------------------------------------------------------------------
// 097 D — the registry is per chain
// ---------------------------------------------------------------------------

/// BNB Chain's USDC is the registry's there, 18 decimals: the native order
/// reads its figure with the chain silent, and nothing is incomplete. Before
/// 097 D the address-only table did not know it, so a node that was down
/// left "— 0x8ac7..." and "Incomplete".
#[test]
fn a_registry_token_reads_with_the_chain_silent() {
    let request = fixture("oneinch-native-order.json");
    let view = resolve(event_of(&request, 56, beijing()), true);
    let result = view.result.expect("read");
    let receive = field(&result, "Receive amount");
    assert_eq!(receive.value, "2.418146082462759045 USDC");
    assert!(!receive.unverified);
    assert!(!result.partial);
}

/// Approves at Ethereum's USDC address, on BNB Chain. The registry names
/// those bytes on Ethereum only: here the token is whatever the contract
/// says, so its address goes beside its own answer — and with the chain
/// silent an amount of it is no figure. Before 097 D the address-only table
/// called it USDC with 6 decimals on every chain.
#[test]
fn a_registry_name_holds_on_its_own_chain_only() {
    let approve = |chain_id: u32, amount: &str| Event::ResolveTransaction {
        to: Some(USDC_ETHEREUM.to_owned()),
        data: Some(format!(
            "0x095ea7b3{:0>64}{amount:0>64}",
            &CURVE_ROUTER[2..]
        )),
        value: Some("0x0".to_owned()),
        chain_id,
        locale: beijing(),
    };
    let unlimited = "f".repeat(64);
    let on_bnb = resolve(approve(56, &unlimited), false)
        .result
        .expect("read");
    let token = field(&on_bnb, "Token");
    assert_eq!(token.value, "USDC (0xa0b869...06eb48)");
    assert_eq!(token.address.as_deref(), Some(USDC_ETHEREUM));

    let silent = resolve(approve(56, "11b340"), true).result.expect("read");
    let amount = field(&silent, "Amount");
    assert!(amount.unverified, "{amount:?}");
    assert_eq!(amount.value, format!("{UNKNOWN_AMOUNT} 0xa0b8..."));
    assert!(silent.partial);

    // On Ethereum the same bytes are the registry's USDC: named alone, and
    // read with no question to the chain.
    let on_ethereum = resolve(approve(1, &unlimited), true).result.expect("read");
    assert_eq!(field(&on_ethereum, "Token").value, "USDC");
    let on_ethereum = resolve(approve(1, "11b340"), true).result.expect("read");
    assert_eq!(field(&on_ethereum, "Amount").value, "1.16 USDC");
    assert!(!on_ethereum.partial);
}

/// Every fixture's readable part, for the results report:
/// `cargo test … --test app_clear_signing_dapp097 -- --ignored --nocapture`.
#[test]
#[ignore = "prints the readable text for results-a.md"]
fn print_every_fixtures_readable_part() {
    for name in [
        "oneinch-native-order.json",
        "oneinch-order.json",
        "aave-borrow.json",
        "aave-repay.json",
        "pancakeswap-bnb-swap.json",
        "uniswap-bnb-swap.json",
        "pancakeswap-usdc-batch.json",
        "curve-approve.json",
        "curve-swap.json",
    ] {
        println!("== {name}");
        let view = read(name);
        for line in readable(&view) {
            println!("{line}");
        }
        if let Some(result) = &view.result {
            for f in &result.fields {
                println!("    · {} role={:?} bound={:?}", f.label, f.role, f.bound);
            }
        }
    }
}
