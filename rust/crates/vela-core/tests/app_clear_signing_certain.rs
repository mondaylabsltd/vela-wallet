//! What is CERTAIN and what is INTERPRETED (clear-signing audit, cs-1…cs-19).
//!
//! The sheet must always be able to tell a reading Vela made from the bytes
//! from one a description gave it, and must never let a description rename
//! what the bytes say. Every case here is driven through the machine by a
//! scripted shell: descriptor bodies, chain answers, code, the 4-byte
//! database. The adversarial cases are the audit's own: a fetched descriptor
//! that calls an unlimited `approve` "Claim" and its spender "Reward from", a
//! token address labelled "Network fee paid to", WETH's mainnet address on
//! Gnosis, a token that calls itself USDC.

#![cfg(feature = "crux")]

mod support;

use serde_json::json;
use support::DomainDriver;
use vela_core::abi::compute_selector;
use vela_core::app::clear_signing::{
    ClearConfirm, ClearFieldRole, ClearLineSource, ClearLocale, ClearOperation as Op, ClearProbe,
    ClearProvenance, ClearRisk, ClearShellResult as Res, ClearSignResult, ClearSigning,
    ClearSigningView, ClearSurface, ClearTerm, Event,
};

type Sut = DomainDriver<ClearSigning>;

const NOW: f64 = 1_790_000_000_000.0;
const ME: &str = "0x85c3815e2a775ed845e0b64ba0421e9c3bf3ac88";
const THIEF: &str = "0x0000000000000000000000000000000000badbad";
const SWEEPER: &str = "0x48aba9cb175899e9aa05b8819b8fac9ef9d8a0da";
const FAKE_USDC: &str = "0x09803ad8ec9d9e9972dbc5e87c8ee00b9435b3f3";
const REWARDS: &str = "0x618a3cd671d8e4d1efa8ac9fee42dbcdada5713a";
const NFT: &str = "0x7eca6b925e9849cbd1b7c07bd29b14de26c67d86";
const GNOSIS_USDC: &str = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83";
const WXDAI: &str = "0xe91d153e0b41518a2ce8dd3d7944fa863463a97d";
const WETH_MAINNET: &str = "0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2";
const V3_ROUTER_2: &str = "0x68b3465833fb72a70ecdf485e0e4c7bd8665fc45";
const MAX: &str = "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";

fn word(hexish: &str) -> String {
    format!("{:0>64}", hexish.trim_start_matches("0x").to_lowercase())
}

fn num(v: u128) -> String {
    format!("{v:064x}")
}

fn abi_string(text: &str) -> String {
    let mut body = hex(text.as_bytes());
    while body.len() % 64 != 0 {
        body.push('0');
    }
    format!("0x{}{}{body}", num(32), num(text.len() as u128))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// What the scripted shell knows.
#[derive(Default)]
struct Shell {
    /// path → descriptor body.
    descriptors: Vec<(String, String)>,
    /// (address, decimals, symbol) answered by the chain.
    tokens: Vec<(&'static str, u64, &'static str)>,
    /// Addresses with no code.
    empty: Vec<&'static str>,
    /// selector → signatures.
    selectors: Vec<(String, String)>,
    /// (address, name) an NFT collection answers `name()` with.
    names: Vec<(&'static str, &'static str)>,
}

impl Shell {
    fn answer(&self, op: &Op) -> Res {
        match op {
            Op::Now => Res::Clock { now_ms: NOW },
            Op::HttpGet { path } => Res::DescriptorFetched {
                path: path.clone(),
                json: self
                    .descriptors
                    .iter()
                    .find(|(p, _)| p == path)
                    .map(|(_, body)| body.clone()),
            },
            Op::RpcGetCode { chain_id, address } => Res::RpcAnswer {
                probe: ClearProbe::Code,
                chain_id: *chain_id,
                to: address.clone(),
                result: Some(
                    if self.empty.iter().any(|a| a.eq_ignore_ascii_case(address)) {
                        "0x".to_owned()
                    } else {
                        "0x6080604052".to_owned()
                    },
                ),
                rpc_error: false,
            },
            Op::RpcEthCall {
                chain_id,
                to,
                probe,
                ..
            } => {
                let token = self
                    .tokens
                    .iter()
                    .find(|(a, _, _)| a.eq_ignore_ascii_case(to));
                let result = match (probe, token) {
                    (ClearProbe::Decimals, Some((_, decimals, _))) => {
                        Some(format!("0x{}", num(u128::from(*decimals))))
                    }
                    (ClearProbe::Symbol, Some((_, _, symbol))) => Some(abi_string(symbol)),
                    (ClearProbe::Name, _) => self
                        .names
                        .iter()
                        .find(|(a, _)| a.eq_ignore_ascii_case(to))
                        .map(|(_, name)| abi_string(name)),
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
                sigs: self
                    .selectors
                    .iter()
                    .filter(|(s, _)| s == selector)
                    .map(|(_, sig)| sig.clone())
                    .collect(),
            },
            Op::Timer { token, .. } => Res::TimedOut { token: *token },
        }
    }
}

fn tx(chain_id: u32, to: &str, data: &str, value: &str, probe_code: bool) -> Event {
    Event::ResolveTransaction {
        to: Some(to.to_owned()),
        data: Some(data.to_owned()),
        value: Some(value.to_owned()),
        chain_id,
        locale: ClearLocale::default(),
        from: Some(ME.to_owned()),
        probe_code,
    }
}

/// Read one request to its end. Timers are answered only when nothing else
/// is outstanding — the order a real shell's answers come in.
fn read(shell: &Shell, event: Event) -> ClearSigningView {
    let mut sut = Sut::new();
    sut.dispatch(event);
    for _ in 0..500 {
        let outstanding = sut.outstanding();
        let Some(next) = outstanding
            .iter()
            .position(|op| !matches!(op, Op::Timer { .. }))
            .or_else(|| (!outstanding.is_empty()).then_some(0))
        else {
            let view = sut.view();
            assert!(!view.resolving, "still reading");
            return view;
        };
        let op = outstanding[next].clone();
        let answer = shell.answer(&op);
        sut.resolve_matching(|o| *o == op, answer);
    }
    panic!("the request never finished reading");
}

fn result(view: &ClearSigningView) -> ClearSignResult {
    view.result.clone().expect("a reading")
}

fn sel(sig: &str) -> String {
    compute_selector(sig).expect("selector")
}

fn approve_data(spender: &str, amount_word: &str) -> String {
    format!("0x095ea7b3{}{}", word(spender), amount_word)
}

/// The audit's evil descriptor shape: a third party's file for a contract.
fn evil(sig: &str, intent: &str, fields: serde_json::Value, name: &str, owner: &str) -> String {
    json!({
        "context": { "contract": { "deployments": [{ "chainId": 100, "address": "0x0" }] } },
        "metadata": { "owner": owner, "contractName": name },
        "display": { "formats": { sig: { "intent": intent, "fields": fields } } }
    })
    .to_string()
}

fn path_for(chain_id: u32, address: &str) -> String {
    format!("/erc7730/calldata/eip155-{chain_id}/{address}.json")
}

// ---------------------------------------------------------------------------
// cs-3: certain decode wins
// ---------------------------------------------------------------------------

/// The audit's w26: a fetched descriptor for the attacker's own token calls
/// `approve(sweeper, MAX)` "Claim", labels the spender "Reward from" and hides
/// the amount. Vela decodes the approval itself; the descriptor's words are
/// quoted in danger lines and change nothing else — not the intent, not the
/// labels, not the button.
#[test]
fn a_description_cannot_rename_an_approval() {
    let shell = Shell {
        descriptors: vec![(
            path_for(100, FAKE_USDC),
            evil(
                "approve(address spender,uint256 amount)",
                "Claim",
                json!([
                    { "path": "spender", "label": "Reward from", "format": "addressName" },
                    { "path": "amount", "label": "Amount", "format": "raw", "visible": "never" }
                ]),
                "USDC Rewards",
                "Circle",
            ),
        )],
        tokens: vec![(FAKE_USDC, 6, "USDC")],
        ..Shell::default()
    };
    let view = read(
        &shell,
        tx(100, FAKE_USDC, &approve_data(SWEEPER, MAX), "0x0", true),
    );
    let r = result(&view);
    assert_eq!(
        r.intent, "Approve",
        "the bytes' action, not the description's"
    );
    assert_eq!(r.provenance, ClearProvenance::Standard);
    assert_eq!(r.reading.source, ClearLineSource::Vela);
    assert_eq!(
        r.reading.contradictions,
        vec!["Claim".to_owned(), "Reward from".to_owned()],
        "each word the description used against the decode is quoted"
    );
    assert_eq!(r.risk, ClearRisk::Danger);
    let spender = r
        .fields
        .iter()
        .find(|f| f.role == ClearFieldRole::Spender)
        .expect("the spender is drawn");
    assert_eq!(
        spender.label, "Spender",
        "the description cannot relabel a spender"
    );
    assert_eq!(spender.address.as_deref(), Some(SWEEPER));
    assert!(
        r.fields.iter().any(|f| f.warning && f.value == "Unlimited"),
        "the hidden amount is shown, and it is unlimited"
    );
    // The descriptor's name for the contract stays — as ITS word.
    assert_eq!(r.contract_name.as_deref(), Some("USDC Rewards"));
    assert_eq!(r.reading.contract_named_by, Some(ClearLineSource::Fetched));
    assert_eq!(r.reading.descriptor_owner.as_deref(), Some("Circle"));
    assert_eq!(
        view.confirm,
        ClearConfirm::ConfirmIntent {
            intent: "Approve".to_owned(),
            intent_term: Some(ClearTerm::IntentApprove)
        },
        "the button is the certain action"
    );
    assert!(r.reading.allowance);
}

/// A descriptor that agrees with the decode is no danger.
#[test]
fn a_description_that_agrees_raises_nothing() {
    let shell = Shell {
        descriptors: vec![(
            path_for(100, GNOSIS_USDC),
            evil(
                "approve(address spender,uint256 amount)",
                "Approve",
                json!([
                    { "path": "spender", "label": "Spender", "format": "addressName" },
                    { "path": "amount", "label": "Allowance", "format": "tokenAmount" }
                ]),
                "USD Coin",
                "Circle",
            ),
        )],
        ..Shell::default()
    };
    let r = result(&read(
        &shell,
        tx(
            100,
            GNOSIS_USDC,
            &approve_data(REWARDS, &num(100_000_000)),
            "0x0",
            false,
        ),
    ));
    assert!(r.reading.contradictions.is_empty());
    assert_eq!(
        r.risk,
        ClearRisk::Caution,
        "an approval is caution, no more"
    );
    assert_eq!(
        r.fields[0].value, "100 USDC",
        "the registry's token, priced"
    );
    assert_eq!(r.fields[0].usd_value, Some(100.0));
}

/// w27: a description labels the token address "Network fee paid to". The
/// line keeps its value under a plain label, and the claim is quoted.
#[test]
fn a_description_cannot_borrow_the_wallets_words() {
    let sig = "sweepTokens_1y1r(address token,uint256 amount)";
    let data = format!(
        "{}{}{}",
        sel(sig),
        word(GNOSIS_USDC),
        num(5_000_000_000_000)
    );
    let shell = Shell {
        descriptors: vec![(
            path_for(100, SWEEPER),
            evil(
                sig,
                "Claim airdrop",
                json!([
                    { "path": "token", "label": "Network fee paid to", "format": "addressName" }
                ]),
                "Gnosis Airdrop",
                "Gnosis",
            ),
        )],
        ..Shell::default()
    };
    let view = read(&shell, tx(100, SWEEPER, &data, "0x0", true));
    let r = result(&view);
    assert_eq!(r.reading.source, ClearLineSource::Fetched);
    assert_eq!(
        r.reading.contradictions,
        vec!["Network fee paid to".to_owned()]
    );
    assert_eq!(r.fields[0].label, "Address");
    assert_eq!(r.fields[0].address.as_deref(), Some(GNOSIS_USDC));
    assert_eq!(r.fields[0].source, ClearLineSource::Fetched);
    assert_eq!(r.risk, ClearRisk::Danger);
    assert_eq!(r.contract_name.as_deref(), Some("Gnosis Airdrop"));
    assert_eq!(r.reading.contract_named_by, Some(ClearLineSource::Fetched));
    assert_eq!(
        r.reading.descriptor_path.as_deref(),
        Some(path_for(100, SWEEPER).as_str())
    );
    // cs-7: a description's intent is never the button.
    assert_eq!(view.confirm, ClearConfirm::Confirm);
}

// ---------------------------------------------------------------------------
// cs-1: token identity
// ---------------------------------------------------------------------------

/// w02: a token outside the registry that answers `symbol()` "USDC" is drawn
/// as "USDC?", unpriced, its address kept — whatever it calls itself.
#[test]
fn a_token_that_calls_itself_usdc_is_not_usdc() {
    let shell = Shell {
        tokens: vec![(FAKE_USDC, 6, "USDC")],
        ..Shell::default()
    };
    let data = format!("0xa9059cbb{}{}", word(THIEF), num(250_000_000));
    let r = result(&read(&shell, tx(100, FAKE_USDC, &data, "0x0", true)));
    let amount = &r.fields[0];
    assert_eq!(amount.value, "250 USDC?");
    assert!(amount.token_named_by_contract);
    assert_eq!(amount.usd_value, None, "no $1 peg for a stranger's USDC");
    assert_eq!(amount.token_address.as_deref(), Some(FAKE_USDC));
    // The registry's USDC on the same chain is USDC.
    let data = format!("0xa9059cbb{}{}", word(THIEF), num(250_000_000));
    let r = result(&read(
        &Shell::default(),
        tx(100, GNOSIS_USDC, &data, "0x0", true),
    ));
    assert_eq!(r.fields[0].value, "250 USDC");
    assert!(!r.fields[0].token_named_by_contract);
    assert_eq!(r.fields[0].usd_value, Some(250.0));
}

// ---------------------------------------------------------------------------
// cs-2: recipients
// ---------------------------------------------------------------------------

fn exact_input_single(token_in: &str, token_out: &str, recipient: &str, min_out: u128) -> String {
    let sig = "exactInputSingle((address,address,uint24,address,uint256,uint256,uint160))";
    format!(
        "{}{}{}{}{}{}{}{}",
        sel(sig),
        word(token_in),
        word(token_out),
        num(500),
        word(recipient),
        num(250_000_000),
        num(min_out),
        num(0)
    )
}

const USDC_MAINNET: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

/// w11: a swap whose output goes to a stranger, accepting 1 wei back. The
/// reading says the recipient is not this account, raises the risk to
/// danger, and flags the dust minimum.
#[test]
fn a_swap_paying_a_stranger_says_so() {
    let data = exact_input_single(USDC_MAINNET, WETH_MAINNET, THIEF, 1);
    let r = result(&read(
        &Shell::default(),
        tx(1, V3_ROUTER_2, &data, "0x0", true),
    ));
    assert_eq!(r.reading.source, ClearLineSource::BuiltIn);
    assert!(r.reading.goes_elsewhere);
    assert!(r.reading.dust_minimum);
    assert_eq!(r.risk, ClearRisk::Danger);
    let recipient = r
        .fields
        .iter()
        .find(|f| f.role == ClearFieldRole::Recipient)
        .expect("recipient");
    assert!(!recipient.this_account);
}

/// The same swap paying this account: "this account", nothing elsewhere.
#[test]
fn a_swap_paying_this_account_says_this_account() {
    let data = exact_input_single(USDC_MAINNET, WETH_MAINNET, ME, 1_000_000_000_000_000);
    let r = result(&read(
        &Shell::default(),
        tx(1, V3_ROUTER_2, &data, "0x0", true),
    ));
    let recipient = r
        .fields
        .iter()
        .find(|f| f.role == ClearFieldRole::Recipient)
        .expect("recipient");
    assert!(recipient.this_account);
    assert!(!r.reading.goes_elsewhere);
    assert!(!r.reading.dust_minimum);
    assert_ne!(r.risk, ClearRisk::Danger);
}

/// A plain ERC-20 send to somebody else is what a send is — no danger line.
#[test]
fn a_send_to_someone_else_is_not_elsewhere() {
    let data = format!("0xa9059cbb{}{}", word(THIEF), num(1_000_000));
    let r = result(&read(
        &Shell::default(),
        tx(100, GNOSIS_USDC, &data, "0x0", true),
    ));
    assert!(!r.reading.goes_elsewhere);
}

// ---------------------------------------------------------------------------
// cs-13: chain scope and code
// ---------------------------------------------------------------------------

/// w15: WETH's mainnet address on Gnosis holds no code. No name, no
/// descriptor — a blind call that only sends its coin, with the warning.
#[test]
fn a_known_name_needs_code_on_this_chain() {
    let shell = Shell {
        empty: vec![WETH_MAINNET],
        ..Shell::default()
    };
    let deposit = sel("deposit()");
    let view = read(
        &shell,
        tx(100, WETH_MAINNET, &deposit, "0xde0b6b3a7640000", true),
    );
    assert!(
        view.result.is_none(),
        "nothing reads code that is not there"
    );
    assert_eq!(view.surface, ClearSurface::BlindTransaction);
    assert!(view.no_code);
    assert!(view.sign_anyway);
    assert_eq!(
        view.native_value.as_ref().map(|v| v.amount.as_str()),
        Some("1")
    );
}

/// Even with no code check, WETH's descriptor is Ethereum's only.
#[test]
fn a_known_contract_is_named_on_its_own_chains_only() {
    let deposit = sel("deposit()");
    let view = read(
        &Shell::default(),
        tx(100, WETH_MAINNET, &deposit, "0xde0b6b3a7640000", false),
    );
    assert!(
        view.result
            .as_ref()
            .is_none_or(|r| r.intent != "Wrap ETH" && r.contract_name.is_none()),
        "no Wrap ETH on Gnosis"
    );
    // On Ethereum it is WETH.
    let view = read(
        &Shell::default(),
        tx(1, WETH_MAINNET, &deposit, "0xde0b6b3a7640000", true),
    );
    let r = result(&view);
    assert_eq!(r.intent, "Wrap ETH");
    assert_eq!(r.reading.contract_named_by, Some(ClearLineSource::BuiltIn));
    assert!(!view.no_code);
    // A built-in descriptor's intent is an interpretation: no button word.
    assert_eq!(view.confirm, ClearConfirm::Confirm);
}

// ---------------------------------------------------------------------------
// cs-7, cs-19: the 4-byte database
// ---------------------------------------------------------------------------

/// w18/w19: a public-database name stays a quote; the call is a "Contract
/// call", the button is "Confirm".
#[test]
fn a_database_name_is_no_headline() {
    let sig = "claimAirdrop_e9x(address,uint256)";
    let selector = sel(sig);
    let data = format!("{selector}{}{}", word(GNOSIS_USDC), num(5));
    let shell = Shell {
        selectors: vec![(selector.clone(), sig.to_owned())],
        ..Shell::default()
    };
    let view = read(&shell, tx(100, SWEEPER, &data, "0x0", true));
    let r = result(&view);
    assert_eq!(r.intent, "Contract call");
    assert_eq!(r.intent_term, Some(ClearTerm::IntentContractCall));
    assert_eq!(r.reading.selector_name.as_deref(), Some(sig));
    assert_eq!(r.reading.source, ClearLineSource::SelectorDb);
    assert_eq!(view.confirm, ClearConfirm::Confirm);
    assert!(!view.sign_anyway);
}

/// w13: a multicall's inner calls are read where their shape is Vela's own.
#[test]
fn a_multicall_says_what_each_call_is() {
    let inner_transfer = format!("a9059cbb{}{}", word(THIEF), num(5));
    let claim = format!(
        "{}{}{}",
        &sel("claimRewards(address,uint256)")[2..],
        word(GNOSIS_USDC),
        num(1)
    );
    let sig = "multicall(bytes[])";
    let selector = sel(sig);
    // bytes[] with two elements.
    let elem = |h: &str| {
        let bytes = h.len() / 2;
        let mut body = h.to_owned();
        while body.len() % 64 != 0 {
            body.push('0');
        }
        format!("{}{body}", num(bytes as u128))
    };
    let first = elem(&claim);
    let second = elem(&inner_transfer);
    let data = format!(
        "{selector}{}{}{}{}{first}{second}",
        num(32),
        num(2),
        num(64),
        num(64 + (first.len() as u128) / 2),
    );
    let shell = Shell {
        selectors: vec![(selector.clone(), sig.to_owned())],
        ..Shell::default()
    };
    let r = result(&read(&shell, tx(100, REWARDS, &data, "0x0", true)));
    let calls: Vec<&str> = r
        .fields
        .iter()
        .filter(|f| f.label.starts_with("Call "))
        .map(|f| f.value.as_str())
        .collect();
    assert_eq!(calls.len(), 2, "{:?}", r.fields);
    assert!(
        calls[0].starts_with("0x9a99b4f0 · 68 bytes"),
        "{}",
        calls[0]
    );
    assert_eq!(calls[1], format!("transfer · to {THIEF} · amount 5"));
}

// ---------------------------------------------------------------------------
// cs-15: shape-only matches
// ---------------------------------------------------------------------------

/// w16: the generic ERC-4626 file matched by selector on a contract that is
/// no vault is a SHAPE — never the descriptor service's word about it.
#[test]
fn a_vault_shape_is_a_shape() {
    let sig = "deposit(uint256 assets,address receiver)";
    let shell = Shell {
        descriptors: vec![(
            "/erc7730/ercs/calldata-erc4626-vaults.json".to_owned(),
            json!({
                "display": { "formats": { sig: {
                    "intent": "Deposit",
                    "fields": [
                        { "path": "assets", "label": "Amount", "format": "tokenAmount", "params": { "tokenPath": "@.to" } },
                        { "path": "receiver", "label": "Send shares to", "format": "addressName" }
                    ]
                } } }
            })
            .to_string(),
        )],
        ..Shell::default()
    };
    let data = format!("{}{}{}", sel(sig), num(1_000_000), word(ME));
    let view = read(&shell, tx(100, REWARDS, &data, "0x1bc16d674ec80000", true));
    let r = result(&view);
    assert_eq!(r.reading.source, ClearLineSource::Shape);
    assert_eq!(r.reading.shape_intent.as_deref(), Some("Deposit"));
    assert!(r.contract_name.is_none());
    assert_eq!(view.confirm, ClearConfirm::Confirm);
}

// ---------------------------------------------------------------------------
// cs-16, cs-17, cs-18: allowances
// ---------------------------------------------------------------------------

/// w06: increaseAllowance adds to an allowance, and says so.
#[test]
fn an_increase_reads_as_an_increase() {
    let data = format!("0x39509351{}{}", word(REWARDS), num(50_000_000));
    let r = result(&read(
        &Shell::default(),
        tx(100, GNOSIS_USDC, &data, "0x0", true),
    ));
    assert_eq!(
        r.fields[0].label_term,
        Some(ClearTerm::LabelAddsToAllowance)
    );
    assert_eq!(r.fields[0].value, "50 USDC");
    assert!(r.reading.allowance);
    assert!(r.reading.allowance_adds);
}

/// w10: setApprovalForAll draws its operator, not a raw "Approved true", and
/// `false` is a revoke.
#[test]
fn approve_all_has_no_raw_flag_and_false_revokes() {
    let shell = Shell::default();
    let grant = format!("0xa22cb465{}{}", word(REWARDS), num(1));
    let mut sut = Sut::new();
    sut.dispatch(tx(100, NFT, &grant, "0x0", false));
    // The ERC-165 probes: answer as an ERC-721.
    for _ in 0..50 {
        let outstanding = sut.outstanding();
        let Some(op) = outstanding.first().cloned() else {
            break;
        };
        let answer = match &op {
            Op::RpcEthCall {
                probe: ClearProbe::SupportsErc721,
                chain_id,
                to,
                ..
            } => Res::RpcAnswer {
                probe: ClearProbe::SupportsErc721,
                chain_id: *chain_id,
                to: to.clone(),
                result: Some(format!("0x{}", num(1))),
                rpc_error: false,
            },
            other => shell.answer(other),
        };
        sut.resolve_matching(|o| *o == op, answer);
    }
    let r = result(&sut.view());
    assert_eq!(r.intent, "Approve all NFTs");
    assert!(
        r.fields.iter().all(|f| f.label != "Approved"),
        "{:?}",
        r.fields
    );
    let revoke = format!("0xa22cb465{}{}", word(REWARDS), num(0));
    let r = result(&read(&shell, tx(100, NFT, &revoke, "0x0", false)));
    assert_eq!(r.intent, "Revoke");
}

/// An approve-all names its collection — its contract's own `name()`, the
/// contract's word, plain and capped — not only a short address; and a
/// collection that answers no name is still read, by its address.
#[test]
fn approve_all_names_its_collection_by_its_own_answer() {
    let grant = format!("0xa22cb465{}{}", word(REWARDS), num(1));
    let shell = Shell {
        names: vec![(NFT, "Gnosis \u{202E}Punks\u{202C}  Club")],
        ..Shell::default()
    };
    let r = result(&read(&shell, tx(100, NFT, &grant, "0x0", false)));
    assert_eq!(r.intent, "Approve all NFTs");
    assert_eq!(
        r.reading.collection_name.as_deref(),
        Some("Gnosis Punks Club")
    );
    assert!(
        r.contract_name.is_none(),
        "the collection's word is no contract name"
    );

    let nameless = result(&read(&Shell::default(), tx(100, NFT, &grant, "0x0", false)));
    assert_eq!(nameless.intent, "Approve all NFTs");
    assert!(nameless.reading.collection_name.is_none());
}

/// The contract line names a token Vela's registry lists on a single call,
/// as a batch's row does — not "no name known" over USDC's own transfer.
#[test]
fn a_registry_token_names_its_contract_on_a_single_call() {
    let send = format!("0xa9059cbb{}{}", word(THIEF), num(1_000_000));
    let r = result(&read(
        &Shell::default(),
        tx(100, GNOSIS_USDC, &send, "0x0", false),
    ));
    assert_eq!(r.contract_name.as_deref(), Some("USDC"));
    assert_eq!(r.reading.contract_named_by, Some(ClearLineSource::BuiltIn));
    // A token nobody lists is nobody's to name.
    let unknown = result(&read(
        &Shell::default(),
        tx(100, FAKE_USDC, &send, "0x0", false),
    ));
    assert!(unknown.contract_name.is_none());
}

/// w07: an ERC-2612 permit is Vela's own reading, an allowance, with its
/// deadline on the sheet.
#[test]
fn a_permit_is_certain_and_shows_its_deadline() {
    let typed = json!({
        "types": {
            "EIP712Domain": [
                { "name": "name", "type": "string" },
                { "name": "version", "type": "string" },
                { "name": "chainId", "type": "uint256" },
                { "name": "verifyingContract", "type": "address" }
            ],
            "Permit": [
                { "name": "owner", "type": "address" },
                { "name": "spender", "type": "address" },
                { "name": "value", "type": "uint256" },
                { "name": "nonce", "type": "uint256" },
                { "name": "deadline", "type": "uint256" }
            ]
        },
        "primaryType": "Permit",
        "domain": { "name": "USD Coin", "version": "2", "chainId": 100, "verifyingContract": FAKE_USDC },
        "message": { "owner": ME, "spender": REWARDS, "value": "1000000", "nonce": 0, "deadline": 1893456000 }
    });
    let shell = Shell {
        tokens: vec![(FAKE_USDC, 6, "USDC")],
        ..Shell::default()
    };
    let view = read(
        &shell,
        Event::ResolveTypedData {
            typed_data_json: typed.to_string(),
            chain_id: 100,
            locale: ClearLocale::default(),
            from: Some(ME.to_owned()),
        },
    );
    let r = result(&view);
    assert_eq!(r.reading.source, ClearLineSource::Vela);
    assert!(r.reading.allowance);
    assert!(
        r.fields.iter().any(|f| f.label == "Valid until"),
        "the deadline is on the sheet: {:?}",
        r.fields
    );
    let amount = r
        .fields
        .iter()
        .find(|f| f.format == "tokenAmount")
        .expect("amount");
    assert_eq!(amount.value, "1 USDC?");
    assert_eq!(amount.usd_value, None);
}

// ---------------------------------------------------------------------------
// cs-14: per-leg attribution in a batch
// ---------------------------------------------------------------------------

#[test]
fn every_leg_of_a_batch_carries_its_source() {
    let unknown = format!("{}{}", sel("zzUnknown_9f3(uint256)"), num(7));
    let params = json!([{
        "calls": [
            { "to": GNOSIS_USDC, "data": format!("0xa9059cbb{}{}", word(THIEF), num(1_000_000)), "value": "0x0" },
            { "to": REWARDS, "data": unknown, "value": "0xde0b6b3a7640000" },
            { "to": WETH_MAINNET, "data": sel("deposit()"), "value": "0x1" }
        ]
    }]);
    let shell = Shell {
        empty: vec![WETH_MAINNET],
        ..Shell::default()
    };
    let view = read(
        &shell,
        Event::ResolveBatch {
            params_json: params.to_string(),
            chain_id: 100,
            locale: ClearLocale::default(),
            from: Some(ME.to_owned()),
            probe_code: true,
        },
    );
    let batch = view.batch.expect("a batch");
    assert_eq!(
        batch.calls[0].result.as_ref().map(|r| r.reading.source),
        Some(ClearLineSource::Vela)
    );
    assert!(batch.calls[1].result.is_none(), "nobody read call 2");
    assert!(!batch.calls[1].no_code);
    assert!(batch.calls[2].no_code, "call 3's target holds no code");
    assert!(batch.calls[2].to_name.is_none());
    let _ = WXDAI;
}
