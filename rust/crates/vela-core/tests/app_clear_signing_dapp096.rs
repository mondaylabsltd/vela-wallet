//! Spec 096 part B — what you see is what you sign, on the real requests.
//!
//! Every fixture under `tests/fixtures/dapp096/` is a request a real dApp
//! sent the extension during the 2026-10-02 pass (PancakeSwap, Uniswap, Aave,
//! CoW, Sky), verbatim. The shell here is a realistic one with no descriptor
//! service: the chain answers `decimals()`/`symbol()` for the tokens these
//! dApps touched, the public 4-byte database answers with unnamed signatures,
//! and every descriptor fetch comes back empty — so whatever reads well here
//! reads well because of what the wallet itself knows.
//!
//! The rules pinned (096 F4–F7, F9): the coin a call sends is said in the
//! readable part; a decoded number is never labelled "Value"; an approval
//! names its token and its spender; amounts read with the token's decimals;
//! well-known contracts are named by the chain they are on; a CoW order says
//! its terms are not shown; and the verb the record keeps is the intent.

#![cfg(feature = "crux")]

mod support;

use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::activity_feed::{
    ActivityFeed, Event as FeedEvent, FeedOperation as FeedOp, FeedRow, FeedShellResult as FeedRes,
    FeedTxRecord,
};
use vela_core::app::clear_signing::{
    ClearDateFormat, ClearLocale, ClearOperation as Op, ClearProbe, ClearProvenance, ClearRisk,
    ClearShellResult as Res, ClearSignResult, ClearSigning, ClearSigningView, ClearSurface,
    ClearTerm, ClearTimeFormat, Event,
};
use vela_core::app::dapp_activity::summarize;

type Sut = DomainDriver<ClearSigning>;

/// 2026-10-02 21:16:40 +08:00 — the minute these requests were made.
const NOW: f64 = 1_790_947_000_000.0;

const WALLET: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
const WBNB: &str = "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c";
const USDC_BSC: &str = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d";

/// What the chain answers about the tokens these dApps touched:
/// `(chain, token, decimals, symbol)`.
const TOKENS: &[(u32, &str, u32, &str)] = &[
    (56, WBNB, 18, "WBNB"),
    (56, USDC_BSC, 18, "USDC"),
    (
        8453,
        "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",
        6,
        "USDC",
    ),
    (
        8453,
        "0x820c137fa70c8691f0e44dc420a5e53c168921dc",
        18,
        "USDS",
    ),
];

/// What the public 4-byte database answers: signatures with no names.
const SELECTOR_DB: &[(&str, &str)] = &[
    ("0x3593564c", "execute(bytes,bytes[],uint256)"),
    ("0x87517c45", "approve(address,address,uint160,uint48)"),
    ("0x474cf53d", "depositETH(address,address,uint16)"),
    ("0x617ba037", "supply(address,uint256,address,uint16)"),
    ("0x69328dec", "withdraw(address,uint256,address)"),
    ("0xec6cb13f", "setPreSignature(bytes,bool)"),
    (
        "0x1a019e37",
        "swapExactIn(address,address,uint256,uint256,address,uint256)",
    ),
];

/// Beijing time, ISO dates, 24-hour clock — the owner's own settings.
fn locale() -> ClearLocale {
    ClearLocale {
        date_format: ClearDateFormat::Iso,
        time_format: ClearTimeFormat::H24,
        tz_offset_minutes: 480,
        ..ClearLocale::default()
    }
}

fn fixture(name: &str) -> Value {
    let path = format!(
        "{}/tests/fixtures/dapp096/{name}",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{path}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
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

/// The realistic shell (module doc).
fn answer(op: &Op) -> Res {
    match op {
        Op::Now => Res::Clock { now_ms: NOW },
        Op::HttpGet { path } => Res::DescriptorFetched {
            path: path.clone(),
            json: None,
        },
        Op::RpcEthCall {
            chain_id,
            to,
            probe,
            ..
        } => {
            let token = TOKENS
                .iter()
                .find(|(chain, addr, _, _)| chain == chain_id && addr.eq_ignore_ascii_case(to));
            let result = match (probe, token) {
                (ClearProbe::Decimals, Some((_, _, decimals, _))) => {
                    Some(abi_word(u64::from(*decimals)))
                }
                (ClearProbe::Symbol, Some((_, _, _, symbol))) => Some(abi_string(symbol)),
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

fn drain(sut: &mut Sut) {
    for _ in 0..500 {
        let Some(op) = sut.outstanding().first().cloned() else {
            return;
        };
        sut.resolve(answer(&op));
    }
    panic!("the request never finished reading");
}

/// The sheet's reading of one fixture, every question answered.
fn read(name: &str) -> ClearSigningView {
    let request = fixture(name);
    let chain_id = u32::try_from(request["chainId"].as_u64().expect("chainId")).expect("chain");
    let params = &request["params"];
    let event = match request["method"].as_str().expect("method") {
        "eth_sendTransaction" => {
            let tx = &params[0];
            let text = |key: &str| tx.get(key).and_then(Value::as_str).map(str::to_owned);
            Event::ResolveTransaction {
                to: text("to"),
                data: text("data"),
                value: text("value"),
                chain_id,
                locale: locale(),
            }
        }
        "wallet_sendCalls" => Event::ResolveBatch {
            params_json: params.to_string(),
            chain_id,
            locale: locale(),
        },
        method if method.starts_with("eth_signTypedData") => Event::ResolveTypedData {
            typed_data_json: params[1].as_str().expect("typed data").to_owned(),
            chain_id,
            locale: locale(),
        },
        other => panic!("no reader for {other}"),
    };
    let mut sut = Sut::new();
    sut.dispatch(event);
    drain(&mut sut);
    let view = sut.view();
    assert!(!view.resolving, "{name}: still reading");
    view
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
        if result.terms_off_chain {
            lines.push("⚠ order terms not shown".to_owned());
        }
    }
    if let Some(batch) = &view.batch {
        lines.push(format!("[Batch of {}]", batch.calls.len()));
        for call in &batch.calls {
            let intent = call.result.as_ref().map_or("?", |r| r.intent.as_str());
            lines.push(format!("  {} · {}", call.index, intent));
            if let Some(result) = &call.result {
                lines.extend(rows(result).into_iter().map(|row| format!("    {row}")));
                if result.terms_off_chain {
                    lines.push("    ⚠ order terms not shown".to_owned());
                }
            }
            if let Some(amount) = call
                .amount
                .as_ref()
                .filter(|_| call.value_wei.as_deref() != Some("0"))
            {
                lines.push(format!("    Amount: −{amount} (coin)"));
            }
            lines.push(format!(
                "    Interacting with: {}",
                call.to_name.clone().or(call.to.clone()).unwrap_or_default()
            ));
        }
    }
    if view.result.is_none() && view.batch.is_none() {
        lines.push(format!("<{:?}>", view.surface));
        if let Some(native) = &view.native_value {
            lines.push(format!("Amount: −{} (coin)", native.amount));
        }
    }
    lines.push(format!("record: {:?}", view.record_intent));
    lines
}

fn has(lines: &[String], line: &str) -> bool {
    lines.iter().any(|l| l.trim() == line)
}

// ---------------------------------------------------------------------------
// The real requests
// ---------------------------------------------------------------------------

/// PancakeSwap USDC → BNB, one atomic batch: approve PancakeSwap's Permit2,
/// Permit2-approve the router, swap. Before 096: "2 · Approve" and "3 ·
/// Execute" with nothing under them but forty hex digits.
#[test]
fn pancakeswap_batch_reads_as_a_swap_with_its_amounts_and_names() {
    let view = read("pancakeswap-batch.json");
    assert_eq!(view.surface, ClearSurface::Batch);
    let lines = readable(&view);
    for line in [
        "1 · Approve",
        "Amount: 2.341714 USDC",
        "Spender: PancakeSwap Permit2",
        "2 · Approve",
        "Spender: PancakeSwap Universal Router",
        "Expires: 2026-11-01, 21:16",
        "Interacting with: PancakeSwap Permit2",
        "3 · Swap",
        "You pay: 2.341714 USDC",
        "You receive (min): 0.002983249209629786 BNB",
        "Deadline: 2026-10-02, 21:35",
        "Interacting with: PancakeSwap Universal Router",
    ] {
        assert!(has(&lines, line), "missing {line:?} in {lines:#?}");
    }
    let calls = &view.batch.as_ref().expect("batch").calls;
    let swap = calls[2].result.as_ref().expect("the swap is read");
    assert_eq!(swap.provenance, ClearProvenance::BuiltIn);
    assert!(!swap.best_effort && !swap.partial);
    assert_eq!(swap.intent_term, Some(ClearTerm::IntentSwap));
    // The coin lands with the account itself: the unwrap names it.
    assert!(swap
        .fields
        .iter()
        .any(|f| f.label == "Recipient" && f.address.as_deref() == Some(WALLET)));
    assert!(
        !lines.iter().any(|l| l.contains("Value")),
        "no number is called Value: {lines:#?}"
    );
    // The meaningful leg is the batch's verb (F9).
    assert_eq!(view.record_intent.as_deref(), Some("Swap"));
}

/// Uniswap's Permit2 `PermitSingle` for WBNB to its BNB-Chain router. Before
/// 096: "−Unlimited", a spender's hex, and no token anywhere.
#[test]
fn uniswap_permit_names_its_token_and_its_spender() {
    let view = read("uniswap-permit.json");
    assert_eq!(view.surface, ClearSurface::ClearSign);
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[Approve]",
            "Amount: Unlimited",
            // BNB Chain's wrapped coin is the registry's there: no address (097 D).
            "Token: WBNB",
            "Spender: Uniswap Universal Router",
            "Expires: 2026-11-01, 21:20",
            "Valid until: 2026-10-02, 21:50",
            // Activity titles a permit by what it is, whatever this says.
            "record: Some(\"Approve\")",
        ]
    );
    let result = view.result.expect("result");
    assert_eq!(result.risk, ClearRisk::Danger, "unlimited stays danger");
    assert_eq!(result.contract_name.as_deref(), Some("Permit2"));
    let token = &result.fields[1];
    assert_eq!(token.token_address.as_deref(), Some(WBNB));
    assert_eq!(token.label_term, Some(ClearTerm::LabelToken));
    assert_eq!(result.fields[0].value_term, Some(ClearTerm::ValueUnlimited));
}

/// Aave `depositETH` sends 0.003 BNB. Before 096: "Value 0" (the referral
/// code) and the 0.003 BNB nowhere in the readable part.
#[test]
fn aave_deposit_eth_says_the_coin_it_sends() {
    let view = read("aave-deposit-eth.json");
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[Supply]",
            "Supply: 0.003 BNB",
            "On behalf of: 0x88cca0...266894",
            "record: Some(\"Supply\")",
        ]
    );
    // The descriptor's own row says the value — the sheet says it once.
    assert_eq!(view.native_value, None);
    let result = view.result.expect("result");
    assert_eq!(
        result.contract_name.as_deref(),
        Some("Aave Wrapped Token Gateway")
    );
    assert_eq!(result.intent_term, Some(ClearTerm::IntentSupply));
}

/// Aave `supply(WBNB, 0.003…)`. Before 096: "Value 3,000,000,000,000,000".
#[test]
fn aave_supply_reads_the_amount_in_its_token() {
    let lines = readable(&read("aave-supply.json"));
    assert_eq!(
        lines,
        vec![
            "[Supply]",
            "Supply: 0.003 WBNB",
            "On behalf of: 0x88cca0...266894",
            "record: Some(\"Supply\")",
        ]
    );
}

/// Aave withdraw-all. Before 096: a 78-digit "Value".
#[test]
fn aave_withdraw_all_reads_all_and_names_the_token() {
    let view = read("aave-withdraw.json");
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[Withdraw]",
            "Withdraw: All",
            // BNB Chain's wrapped coin is the registry's there: no address (097 D).
            "Token: WBNB",
            "Recipient: 0x88cca0...266894",
            "record: Some(\"Withdraw\")",
        ]
    );
    let result = view.result.expect("result");
    assert_eq!(result.fields[0].value_term, Some(ClearTerm::ValueAll));
    assert!(!result.fields[0].warning, "all of one's own is no danger");
}

/// Aave's unlimited WBNB approve of its pool: the token and the pool named.
#[test]
fn aave_unlimited_approve_names_token_and_pool() {
    let lines = readable(&read("aave-approve.json"));
    assert_eq!(
        lines,
        vec![
            "[Approve]",
            "Amount: Unlimited",
            // BNB Chain's wrapped coin is the registry's there: no address (097 D).
            "Token: WBNB",
            "Spender: Aave V3 Pool",
            "record: Some(\"Approve\")",
        ]
    );
}

/// CoW: approve the vault relayer, pre-sign the order. Before 096: "2 · Set
/// pre signature" and nothing else.
#[test]
fn cow_presign_names_the_order_and_says_its_terms_are_not_shown() {
    let view = read("cow-presign.json");
    let lines = readable(&view);
    for line in [
        "1 · Approve",
        "Amount: 0.003 WBNB",
        "Spender: CoW Vault Relayer",
        "2 · Swap",
        "Order: 0x09e95d9c...6abfb9fe",
        "Valid until: 2026-10-02, 22:04",
        "⚠ order terms not shown",
        "Interacting with: CoW Protocol",
    ] {
        assert!(has(&lines, line), "missing {line:?} in {lines:#?}");
    }
    let presign = view.batch.as_ref().expect("batch").calls[1]
        .result
        .clone()
        .expect("read");
    assert!(presign.terms_off_chain);
    assert_eq!(
        presign.risk,
        ClearRisk::Caution,
        "unseen terms are never calm"
    );
    assert_eq!(view.record_intent.as_deref(), Some("Swap"));
}

/// Sky's USDC → USDS convert on Base: Spark's PSM, read in both tokens.
#[test]
fn sky_convert_reads_as_a_swap_through_sparks_psm() {
    let view = read("sky-convert.json");
    let lines = readable(&view);
    for line in [
        "1 · Approve",
        "Amount: 0.034929 USDC",
        "Spender: Spark PSM",
        "2 · Swap",
        "You pay: 0.034929 USDC",
        "You receive (min): 0.034929 USDS",
        "Recipient: 0x88cca0...266894",
        "Interacting with: Spark PSM",
    ] {
        assert!(has(&lines, line), "missing {line:?} in {lines:#?}");
    }
    assert_eq!(view.record_intent.as_deref(), Some("Swap"));
}

/// Every fixture's readable part, for the results report:
/// `cargo test … --test app_clear_signing_dapp096 -- --ignored --nocapture`.
#[test]
#[ignore = "prints the readable text for results-b.md"]
fn print_every_fixtures_readable_part() {
    for name in [
        "pancakeswap-batch.json",
        "uniswap-permit.json",
        "aave-deposit-eth.json",
        "aave-supply.json",
        "aave-withdraw.json",
        "aave-approve.json",
        "cow-presign.json",
        "sky-convert.json",
    ] {
        println!("== {name}");
        for line in readable(&read(name)) {
            println!("{line}");
        }
    }
}

// ---------------------------------------------------------------------------
// The rules, on requests built to isolate them
// ---------------------------------------------------------------------------

fn tx(to: &str, data: &str, value: &str, chain_id: u32) -> ClearSigningView {
    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTransaction {
        to: Some(to.to_owned()),
        data: Some(data.to_owned()),
        value: Some(value.to_owned()),
        chain_id,
        locale: locale(),
    });
    drain(&mut sut);
    sut.view()
}

/// F4: a call nobody could read still says the coin it sends — and a
/// best-effort reading does too, beside numbers that are called numbers.
#[test]
fn the_coin_a_call_sends_is_always_in_the_readable_part() {
    let stranger = "0x7777777777777777777777777777777777777777";
    // Nothing anywhere knows selector 0xdeadbeef: blind.
    let blind = tx(stranger, "0xdeadbeef", "0xaa87bee538000", 56);
    assert_eq!(blind.surface, ClearSurface::BlindTransaction);
    let native = blind.native_value.expect("the coin is said");
    assert_eq!(
        (native.amount.as_str(), native.value_wei.as_str()),
        ("0.003", "3000000000000000")
    );

    // depositETH on a contract the wallet does NOT know: the 4-byte
    // database's guess, best effort — the coin is still said.
    let data = fixture("aave-deposit-eth.json")["params"][0]["data"]
        .as_str()
        .expect("data")
        .to_owned();
    let guessed = tx(stranger, &data, "0xaa87bee538000", 56);
    let result = guessed.result.clone().expect("best effort");
    assert!(result.best_effort);
    assert_eq!(
        guessed.native_value.map(|n| n.amount).as_deref(),
        Some("0.003")
    );
    let labels: Vec<&str> = result.fields.iter().map(|f| f.label.as_str()).collect();
    assert_eq!(labels, vec!["Address", "Address", "Number"]);

    // No value, nothing to say.
    let none = tx(stranger, "0xdeadbeef", "0x0", 56);
    assert_eq!(none.native_value, None);
}

/// F4: a parameter's own name, in words; a named time as its date; the
/// maximum as "Max" — and an unnamed number left as the number it is.
#[test]
fn best_effort_numbers_read_by_their_names() {
    let stranger = "0x7777777777777777777777777777777777777777";
    let sig = "settle(uint256 referralCode,uint256 deadline,uint256 amount,uint256)";
    let selector = vela_core::abi::compute_selector(sig).expect("selector");
    let data = format!(
        "{selector}{:064x}{:064x}{}{:064x}",
        7,
        1_790_948_152_u64,
        "f".repeat(64),
        1_790_948_152_u64
    );
    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTransaction {
        to: Some(stranger.to_owned()),
        data: Some(data),
        value: Some("0x0".to_owned()),
        chain_id: 56,
        locale: locale(),
    });
    for _ in 0..50 {
        let Some(op) = sut.outstanding().first().cloned() else {
            break;
        };
        let res = match &op {
            Op::SelectorDbLookup { .. } => Res::SelectorCandidates {
                sigs: vec![sig.to_owned()],
            },
            other => answer(other),
        };
        sut.resolve(res);
    }
    let result = sut.view().result.expect("best effort");
    let shown: Vec<(String, String)> = result
        .fields
        .iter()
        .map(|f| (f.label.clone(), f.value.clone()))
        .collect();
    assert_eq!(
        shown,
        vec![
            ("Referral code".to_owned(), "7".to_owned()),
            ("Deadline".to_owned(), "2026-10-02, 21:35".to_owned()),
            ("Amount".to_owned(), "Max".to_owned()),
            ("Number".to_owned(), "1,790,948,152".to_owned()),
        ]
    );
}

/// F5: a contract is named only on the chain the wallet checked it on.
#[test]
fn a_built_in_reading_holds_only_on_its_own_chain() {
    let supply = fixture("aave-supply.json")["params"][0]["data"]
        .as_str()
        .expect("data")
        .to_owned();
    let aave_bnb = "0x6807dc923806fe8fd134338eabca509979a7e0cb";
    let on_bnb = tx(aave_bnb, &supply, "0x0", 56).result.expect("read");
    assert_eq!(on_bnb.provenance, ClearProvenance::BuiltIn);
    // The same bytes at the same address on Ethereum: nobody's pool.
    let elsewhere = tx(aave_bnb, &supply, "0x0", 1).result.expect("best effort");
    assert_eq!(elsewhere.provenance, ClearProvenance::SelectorDb);
    assert_eq!(elsewhere.contract_name, None);
}

/// F6: a `PermitBatch` names every token it hands out, with its own cap.
#[test]
fn a_permit2_batch_names_every_token() {
    let typed = json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" },
            ],
            "PermitBatch": [
                { "name": "details", "type": "PermitDetails[]" },
                { "name": "spender", "type": "address" },
                { "name": "sigDeadline", "type": "uint256" },
            ],
            "PermitDetails": [
                { "name": "token", "type": "address" },
                { "name": "amount", "type": "uint160" },
                { "name": "expiration", "type": "uint48" },
                { "name": "nonce", "type": "uint48" },
            ],
        },
        "domain": { "name": "Permit2", "chainId": "56", "verifyingContract": "0x000000000022d473030f116ddee9f6b43ac78ba3" },
        "primaryType": "PermitBatch",
        "message": {
            "details": [
                { "token": WBNB, "amount": "1461501637330902918203684832716283019655932542975", "expiration": "1793539250", "nonce": "0" },
                { "token": USDC_BSC, "amount": "5000000000000000000", "expiration": "1793539250", "nonce": "0" },
            ],
            "spender": "0xdc264714f68d84cf29bc605589405e78bdbe7c9f",
            "sigDeadline": "1790949050",
        },
    });
    let mut sut = Sut::new();
    sut.dispatch(Event::ResolveTypedData {
        typed_data_json: typed.to_string(),
        chain_id: 56,
        locale: locale(),
    });
    drain(&mut sut);
    let lines = readable(&sut.view());
    assert_eq!(
        lines,
        vec![
            "[Approve]",
            "Amount: Unlimited",
            // BNB Chain's wrapped coin is the registry's there: no address (097 D).
            "Token: WBNB",
            "Amount: 5 USDC",
            "Spender: Uniswap Universal Router",
            "Expires: 2026-11-01, 21:20",
            "Valid until: 2026-10-02, 21:50",
            "record: Some(\"Approve\")",
        ]
    );
}

/// F5: a router swap that wraps the call's own coin pays THAT coin — said
/// once, by the swap's own row.
#[test]
fn a_router_swap_paid_in_the_native_coin_says_it_once() {
    // PancakeSwap: WRAP_ETH(router, value) then V3 exact-in WBNB → USDC.
    let wrap = format!("{:064x}{:064x}", 2, 3_000_000_000_000_000_u64);
    let path = format!("{}000064{}", &WBNB[2..], &USDC_BSC[2..]);
    let swap = format!(
        "{:064x}{:064x}{:064x}{:064x}{:064x}{:064x}{}{}",
        1,
        3_000_000_000_000_000_u64,
        1_700_000_000_000_000_000_u64,
        0xa0,
        0,
        path.len() / 2,
        path,
        "0".repeat(64 - (path.len() % 64))
    );
    let inputs = [wrap, swap];
    let mut encoded_inputs = format!("{:064x}", inputs.len());
    let mut offset = inputs.len() * 32;
    let mut tails = String::new();
    for input in &inputs {
        encoded_inputs.push_str(&format!("{offset:064x}"));
        let len = input.len() / 2;
        tails.push_str(&format!("{len:064x}{input}"));
        offset += 32 + len;
    }
    encoded_inputs.push_str(&tails);
    // commands = 0x0b00 (WRAP_ETH, V3_SWAP_EXACT_IN), left-aligned in its word.
    let commands = format!("0b00{}", "0".repeat(60));
    let data = format!(
        "0x3593564c{:064x}{:064x}{:064x}{:064x}{commands}{encoded_inputs}",
        0x60, 0xa0, 1_790_948_152_u64, 2,
    );
    let view = tx(
        "0xd9c500dff816a1da21a48a732d3498bf09dc9aeb",
        &data,
        "0xaa87bee538000",
        56,
    );
    let lines = readable(&view);
    assert_eq!(
        lines,
        vec![
            "[Swap]",
            "You pay: 0.003 BNB",
            "You receive (min): 1.7 USDC",
            "Deadline: 2026-10-02, 21:35",
            "record: Some(\"Swap\")",
        ]
    );
    assert_eq!(view.native_value, None, "the swap's own row said it");
}

// ---------------------------------------------------------------------------
// F9 — Activity states the intent, at the protocol
// ---------------------------------------------------------------------------

/// The record of one fixture, as a shell stores it: the sheet's own verb
/// (`record_intent`) and the summary read at approve time.
fn record_of(name: &str) -> FeedTxRecord {
    let request = fixture(name);
    let chain_id = u32::try_from(request["chainId"].as_u64().expect("chainId")).expect("chain");
    let method = request["method"].as_str().expect("method");
    let origin = request["origin"].as_str().expect("origin");
    let summary = summarize(method, &request["params"], chain_id, origin, None);
    serde_json::from_value(json!({
        "id": name,
        "user_op_hash": "0xop",
        "tx_hash": "",
        "from": WALLET,
        "to": "",
        "to_name": null,
        "value": "0",
        "symbol": "",
        "decimals": 0,
        "logo_urls": null,
        "chain_id": chain_id,
        "timestamp": 1_790_947_000.0,
        "day_start_ms": 0.0,
        "status": "confirmed",
        "kind": "dapp_tx",
        "usd": null,
        "dapp_url": origin,
        "intent": read(name).record_intent,
        "summary": summary,
    }))
    .expect("a feed record")
}

fn row_of(name: &str) -> (Option<ClearTerm>, Option<String>) {
    let mut sut = DomainDriver::<ActivityFeed>::new();
    sut.dispatch(FeedEvent::AccountSwitched {
        address: WALLET.to_owned(),
    });
    let read_id = sut
        .outstanding()
        .iter()
        .find_map(|op| match op {
            FeedOp::ReadTxStore { read_id, .. } => Some(*read_id),
            _ => None,
        })
        .expect("a read");
    sut.resolve(FeedRes::StoreLoaded {
        records: vec![record_of(name)],
        now_ms: NOW,
        read_id,
    });
    let dapp = sut
        .view()
        .rows
        .into_iter()
        .find_map(|row| match row {
            FeedRow::Item { item } => item.dapp,
            FeedRow::Header { .. } => None,
        })
        .expect("a dApp row");
    (dapp.intent_term, dapp.place)
}

/// "Batch on PancakeSwap" and "Contract interaction on app.aave.com" become
/// "Swap on PancakeSwap", "Supply on Aave", "Withdraw on Aave", "Swap on
/// CoW", "Swap on Spark" — the place the protocol the call went to.
#[test]
fn activity_titles_state_the_intent_at_the_protocol() {
    let expect = |name: &str, term: ClearTerm, place: &str| {
        assert_eq!(row_of(name), (Some(term), Some(place.to_owned())), "{name}");
    };
    expect(
        "pancakeswap-batch.json",
        ClearTerm::IntentSwap,
        "PancakeSwap",
    );
    expect("aave-supply.json", ClearTerm::IntentSupply, "Aave");
    expect("aave-deposit-eth.json", ClearTerm::IntentSupply, "Aave");
    expect("aave-withdraw.json", ClearTerm::IntentWithdraw, "Aave");
    expect("aave-approve.json", ClearTerm::IntentApprove, "Aave");
    expect("cow-presign.json", ClearTerm::IntentSwap, "CoW");
    expect("sky-convert.json", ClearTerm::IntentSwap, "Spark");
}
