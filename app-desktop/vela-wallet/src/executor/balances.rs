//! The multi-chain balance fetch — the work `BalanceOperation::FetchTokens`
//! hands over whole.
//!
//! That operation names **no chain and no URL**: the core delegates the entire
//! fetch and rules only on what comes back (which totals may be cached, when a
//! partial result may be retried, what an unreachable chain means). So this file
//! is a service, not a mapping.
//!
//! **Ported from** `src/services/wallet-api.ts` @ `c513c4c6` (FR-006), with the
//! two divergences below, both deliberate.
//!
//! ## Two calls per chain, not one
//!
//! The web reads the native coin through Multicall3's own `getEthBalance`,
//! inside the batch. This reads it with `eth_getBalance` and uses the batch only
//! for ERC-20s and quotes, because the two calls answer different questions:
//!
//! - `eth_getBalance` failing means **the chain could not be reached**, and that
//!   is the verdict `failed_chain_ids` carries to the home screen (SC-003).
//! - the batch failing means Multicall3 is not deployed here, or one quoter
//!   reverted. The chain is fine and the person's coin is still theirs.
//!
//! Folding them together makes a chain with no Multicall3 report as unreachable,
//! which is a true-sounding lie about somebody's money.
//!
//! ## Every price comes from a core rule
//!
//! The native coin: `best_native_dex_price` picks the deepest pool across the
//! stables, and `choose_native_price` runs the DEX → local-Chainlink →
//! mainnet-Chainlink ladder with its sanity band. A custom ERC-20:
//! `first_grouped_quote_price` takes the first pool that answers, in the order
//! the groups were built — the preferred stablecoin first, then the rest.
//!
//! The one price this file decides is a stablecoin's $1.00, and that is a
//! MEMBERSHIP verdict rather than a missing factor: the token came from this
//! chain's curated stablecoin list, and ≈$1 is what being on that list means.
//! The web owns it the same way and says so at length.
//!
//! ## Why parallel, and why it streams
//!
//! Twelve chains at up to a few seconds each is a minute of serial waiting, so
//! each chain gets a thread and the answers are joined.
//!
//! Joining is not the end of it, though: the core supports *streaming* partial
//! results (`Event::ChainAssetsArrived`) so the home fills in as chains answer,
//! and until spec 032 phase 12 this file did not use it — one unreachable RPC
//! held the hero on its skeleton for that chain's whole timeout while eleven
//! chains sat answered. [`fetch_all_streaming`] reports each chain from its own
//! thread; the seam that carries those reports into a resident is
//! `resident::Answer::Streaming`. The join still happens, because the SETTLE
//! needs the complete picture.

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;

use serde_json::{Value, json};

use vela_core::app::balance_dashboard::{
    BalanceToken, NativeQuoteGroup, best_native_dex_price, choose_native_price,
    first_grouped_quote_price,
};
use vela_core::app::fee_policy::TEMPO_CHAIN_IDS;
use vela_core::app::network_admin::BUILTIN_CHAINS;

use crate::executor::abi::{self, Call3, McResult};
use crate::executor::chain_tokens::{self, ChainTokenData, DexInfo, StableToken};
use crate::executor::pool::{self, PoolError};
use crate::executor::{chainlink, custom_tokens, token_trust};

/// Uniswap-V3 fee tiers worth asking: 0.05%, 0.3%, PancakeSwap's 0.25%, and 1%
/// for exotic pairs. Each is a separate pool and the deepest one wins.
const FEE_TIERS: [u32; 4] = [500, 3_000, 2_500, 10_000];

/// Per-chain Chainlink native/USD feeds, read inside that chain's own batch —
/// the rung above the Ethereum-mainnet map and below a DEX quote.
///
/// Polygon has no working feed since the MATIC→POL migration; its DEX pools
/// cover it. Verbatim from `wallet-api.ts:47-57`.
const NATIVE_CHAINLINK_FEEDS: &[(u32, &str)] = &[
    (1, "0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419"),
    (56, "0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE"),
    (42161, "0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612"),
    (10, "0x13e3Ee699D1909E989722E753853AE30b17e08c5"),
    (8453, "0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70"),
    (43114, "0x0A77230d17318075983913bC2145DB16C7366156"),
    (100, "0x678df3415fc31947dA4324eC63212874be5a82f8"),
];

/// Every chain the wallet reads for. The built-ins plus whatever the person
/// added — a custom network nobody reads is a network nobody has.
fn chains() -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = BUILTIN_CHAINS
        .iter()
        .map(|chain| (chain.chain_id, chain.native_symbol.to_owned()))
        .collect();
    if let Ok(Some(Value::Array(items))) =
        crate::executor::storage::read_value(crate::executor::storage::KEY_CUSTOM_NETWORKS)
    {
        for item in items {
            let Some(chain_id) = item
                .get("chainId")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
            else {
                continue;
            };
            if out.iter().any(|(id, _)| *id == chain_id) {
                continue;
            }
            let symbol = item
                .get("nativeSymbol")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_owned();
            out.push((chain_id, symbol));
        }
    }
    out
}

/// What a slot is, which is what decides how it gets priced.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Category {
    Native,
    /// Came from this chain's curated stablecoin list.
    Stable,
    Wrapped,
    /// The person added it by contract address.
    Custom,
}

struct Slot {
    symbol: String,
    name: String,
    contract: Option<String>,
    category: Category,
    known_decimals: Option<u32>,
    /// Index into the batch for this slot's `balanceOf`. `None` for the native
    /// coin, which is read by its own call.
    balance_at: Option<usize>,
    /// Index into the batch for this slot's `decimals()`, when it was asked.
    decimals_at: Option<usize>,
}

/// Does this chain have a native coin somebody can hold?
///
/// Tempo does not: its gas is a TIP-20 stablecoin (`fee_policy`'s
/// `TEMPO_DEFAULT_FEE_TOKEN`), and there is no coin behind `eth_getBalance`.
///
/// **MEASURED 2026-09-04.** `rpc.mainnet.tempo.xyz` answers
/// `0x9612084f…6c9b2` — 4.24 × 10^75 base units — to `eth_getBalance` for EVERY
/// address, including `0x…0001` and `0x1111…1111`. It is a constant, not a
/// balance. Rendering it as one puts 4 × 10^57 "USD" in somebody's total, and
/// because Tempo's coin is called USD it prices at exactly $1 (`chainlink::
/// resolve`'s peg), so the junk arrives fully valued rather than unpriced.
///
/// This was invisible before this phase: with every `price_usd` at `None` the
/// total was zero however large the quantity. The two changes that made money
/// visible are the two that made this dangerous.
fn has_native_coin(chain_id: u32) -> bool {
    !TEMPO_CHAIN_IDS.contains(&chain_id)
}

/// One chain's native balance in base units, or `None` if the chain could not
/// be reached.
///
/// `None` and zero are different answers and the core treats them differently:
/// a chain that answered zero is empty, a chain that did not answer is
/// unreachable, and rendering the second as the first is how a wallet quietly
/// under-reports somebody's money.
fn native_raw(chain_id: u32, address: &str) -> Option<String> {
    let response = pool::call(chain_id, "eth_getBalance", json!([address, "latest"]));
    let body = match response {
        Ok(body) => body,
        Err(PoolError::Failed { .. } | PoolError::RangeCap { .. } | PoolError::Unavailable) => {
            return None;
        }
    };
    abi::dec_hex_quantity(body.get("result").and_then(Value::as_str)?)
}

/// Run one `aggregate3` on a chain. An empty vector means the batch did not
/// answer, which is NOT the same as the chain being unreachable.
fn run_batch(chain_id: u32, calls: &[Call3]) -> Vec<McResult> {
    if calls.is_empty() {
        return Vec::new();
    }
    let data = abi::enc_aggregate3(calls);
    let Ok(body) = pool::call(
        chain_id,
        "eth_call",
        json!([{ "to": abi::MULTICALL3, "data": data }, "latest"]),
    ) else {
        return Vec::new();
    };
    body.get("result")
        .and_then(Value::as_str)
        .map(abi::dec_aggregate3)
        .unwrap_or_default()
}

/// The quote calls for one pair, appended to `calls`, with their indices.
///
/// `liquidity-book` and `curve` have no encoder here and fall through to
/// Chainlink — which is a rung, not a failure.
fn push_quote_calls(
    calls: &mut Vec<Call3>,
    dex: &DexInfo,
    token_in: &str,
    token_out: &str,
    amount_in: u128,
) -> Vec<usize> {
    let mut indices = Vec::new();
    match (dex.protocol, dex.quoter_v2, dex.router) {
        ("uniswap-v3", Some(quoter), _) => {
            for fee in FEE_TIERS {
                indices.push(calls.len());
                calls.push(Call3 {
                    target: quoter.to_owned(),
                    call_data: abi::enc_quote_v3(token_in, token_out, amount_in, fee),
                });
            }
        }
        ("solidly", _, Some(router)) => {
            for stable in [false, true] {
                indices.push(calls.len());
                calls.push(Call3 {
                    target: router.to_owned(),
                    call_data: abi::enc_get_amounts_out(amount_in, token_in, token_out, stable),
                });
            }
        }
        _ => {}
    }
    indices
}

/// `10^decimals`, or `None` when the scale is past what a `u128` can hold.
///
/// A token declaring 40 decimals cannot have "one whole token" expressed, so it
/// simply is not quoted. Saturating instead would ask for a quote on a quantity
/// nobody has.
fn one_whole(decimals: u32) -> Option<u128> {
    (decimals <= 38).then(|| 10u128.pow(decimals))
}

/// The order a custom token's stablecoin quotes are tried in: the preferred
/// quote token first, then the rest in the chain's own order.
///
/// `first_grouped_quote_price` takes the FIRST group that answers, so this
/// order is the preference — native USDC's deeper pool wins a tie, and a
/// chain-specific bridge stable still answers when it does not.
fn stable_preference(stables: &[StableToken]) -> Vec<usize> {
    let preferred = chain_tokens::pick_quote_token(stables)
        .and_then(|wanted| stables.iter().position(|s| std::ptr::eq(s, wanted)));
    let mut order: Vec<usize> = (0..stables.len()).collect();
    if let Some(index) = preferred {
        if index > 0 {
            order.remove(index);
            order.insert(0, index);
        }
    }
    order
}

/// One custom token's price, by the core's rules.
///
/// **Path A** quotes the token against each stablecoin, each group scaled by
/// its OWN quote token's decimals — mixing a 6-decimal USDC group with an
/// 18-decimal DAI group under one scale mis-prices by 10^12, which is the bug
/// `first_grouped_quote_price` was extracted to prevent.
///
/// **Path B** quotes it against the wrapped native coin and multiplies by that
/// coin's USD price. One quote token, so one scale — and the wrapped coin
/// mirrors the coin's decimals by construction.
///
/// `None` when neither answers, never a substituted zero: the core has an
/// `Unpriced` notice for exactly this, and a token shown at $0.00 reads as
/// worthless rather than unknown.
fn custom_price(
    slot: &Slot,
    slots: &[Slot],
    quotes: &[(usize, Vec<(Vec<usize>, Option<usize>)>, Vec<usize>)],
    results: &[McResult],
    protocol: Option<&str>,
    native_price: Option<f64>,
    native_decimals: u32,
) -> Option<f64> {
    let position = slots
        .iter()
        .position(|candidate| std::ptr::eq(candidate, slot))?;
    let (_, direct, via_native) = quotes.iter().find(|(index, _, _)| *index == position)?;

    let answered = |at: usize| results.get(at).filter(|result| result.success);
    let decode = |result: &McResult| match protocol {
        Some("solidly") => abi::dec_amounts_out(&result.data),
        _ => abi::dec_u256(&result.data),
    };

    let groups: Vec<NativeQuoteGroup> = direct
        .iter()
        .map(|(indices, decimals_at)| NativeQuoteGroup {
            amounts_out: indices
                .iter()
                .filter_map(|at| answered(*at).and_then(decode))
                .collect(),
            quote_decimals: decimals_at
                .and_then(|at| answered(at))
                .and_then(|result| abi::dec_u8(&result.data))
                .map(u32::from),
        })
        .collect();
    if let Some(price) = first_grouped_quote_price(&groups) {
        return Some(price);
    }

    // Path B needs the coin's own price to convert with; without one, a
    // token-per-coin ratio is not a dollar figure.
    let native_price = native_price?;
    let scale = 10f64.powi(i32::try_from(native_decimals).unwrap_or(i32::MAX));
    for at in via_native {
        let Some(amount) = answered(*at).and_then(decode) else {
            continue;
        };
        let Ok(value) = amount.trim().parse::<f64>() else {
            continue;
        };
        // A zero-output quote is a dead pool, not a free token.
        if value > 0.0 && value.is_finite() {
            return Some(value / scale * native_price);
        }
    }
    None
}

/// One chain's tokens, priced as far as this cut can price them.
fn chain_tokens_for(
    chain_id: u32,
    wallet_symbol: &str,
    address: &str,
    native_raw: &str,
    mainnet_prices: &HashMap<String, f64>,
) -> Vec<BalanceToken> {
    let data = chain_tokens::fetch(chain_id);
    let stables: Vec<StableToken> = data.as_ref().map(|d| d.stables.clone()).unwrap_or_default();
    let wrapped = data.as_ref().and_then(|d| d.wrapped_native.clone());
    let dex = data.as_ref().and_then(|d| d.dex.clone());
    // The wallet's own name for the coin wins over the index's. The index calls
    // Gnosis's coin "XDAI" and every other screen in this app calls it "xDAI";
    // two spellings of one coin across two screens is a bug the person sees.
    let native_symbol = if wallet_symbol.is_empty() {
        data.as_ref()
            .map_or_else(|| "ETH".to_owned(), |d| d.native_symbol.clone())
    } else {
        wallet_symbol.to_owned()
    };
    let native_name = data.as_ref().map_or_else(
        || native_symbol.clone(),
        |d: &ChainTokenData| d.native_name.clone(),
    );
    let native_decimals = data.as_ref().map_or(18, |d| d.native_decimals);

    let mut calls: Vec<Call3> = Vec::new();
    // A chain with no native coin is still READ, for reachability — it simply
    // has no row to show for it.
    let mut slots: Vec<Slot> = if has_native_coin(chain_id) {
        vec![Slot {
            symbol: native_symbol.clone(),
            name: native_name,
            contract: None,
            category: Category::Native,
            known_decimals: Some(native_decimals),
            balance_at: None,
            decimals_at: None,
        }]
    } else {
        Vec::new()
    };
    let native_slots = slots.len();

    for stable in &stables {
        let balance_at = calls.len();
        calls.push(Call3 {
            target: stable.contract.clone(),
            call_data: abi::enc_balance_of(address),
        });
        let decimals_at = calls.len();
        calls.push(Call3 {
            target: stable.contract.clone(),
            call_data: abi::enc_decimals(),
        });
        slots.push(Slot {
            symbol: stable.symbol.clone(),
            name: stable.symbol.clone(),
            contract: Some(stable.contract.clone()),
            category: Category::Stable,
            known_decimals: None,
            balance_at: Some(balance_at),
            decimals_at: Some(decimals_at),
        });
    }

    // Not on a chain whose "wrapped" native IS the native (spec 038, the
    // founder's Celo report): the core knows the two, and listing both would
    // count one holding twice. The address still serves the price quote below.
    if let Some(wrapped) = wrapped
        .as_ref()
        .filter(|w| !vela_core::app::balance_dashboard::wrapped_native_is_the_native(chain_id, w))
    {
        let balance_at = calls.len();
        calls.push(Call3 {
            target: wrapped.clone(),
            call_data: abi::enc_balance_of(address),
        });
        let decimals_at = calls.len();
        calls.push(Call3 {
            target: wrapped.clone(),
            call_data: abi::enc_decimals(),
        });
        slots.push(Slot {
            symbol: format!("W{native_symbol}"),
            name: format!("Wrapped {native_symbol}"),
            contract: Some(wrapped.clone()),
            category: Category::Wrapped,
            known_decimals: None,
            balance_at: Some(balance_at),
            decimals_at: Some(decimals_at),
        });
    }

    for token in custom_tokens::read()
        .into_iter()
        .filter(|token| token.chain_id == chain_id)
    {
        let balance_at = calls.len();
        calls.push(Call3 {
            target: token.contract_address.clone(),
            call_data: abi::enc_balance_of(address),
        });
        slots.push(Slot {
            symbol: token.symbol.clone(),
            name: token.name.clone(),
            contract: Some(token.contract_address.clone()),
            category: Category::Custom,
            // Read from the chain when the token was added, and all-or-nothing
            // then (`manage_tokens.rs`), so it is a fact rather than a guess.
            known_decimals: Some(token.decimals),
            balance_at: Some(balance_at),
            decimals_at: None,
        });
    }

    // The native coin's DEX quotes: one whole coin against EVERY stablecoin,
    // each stable its own group so its own decimals normalize the amount. The
    // core picks the winner — one near-empty pool quoting nonsense is exactly
    // what `best_native_dex_price` exists to survive.
    let mut quote_groups: Vec<(Vec<usize>, Option<usize>)> = Vec::new();
    if let (Some(wrapped), Some(dex), Some(amount_in)) =
        (wrapped.as_ref(), dex.as_ref(), one_whole(native_decimals))
    {
        for (index, stable) in stables.iter().enumerate() {
            let indices = push_quote_calls(&mut calls, dex, wrapped, &stable.contract, amount_in);
            if !indices.is_empty() {
                // The stables follow the native slot, where there is one.
                quote_groups.push((
                    indices,
                    slots.get(native_slots + index).and_then(|s| s.decimals_at),
                ));
            }
        }
    }

    // A custom token's quotes: the token against each stablecoin (path A), and
    // against the wrapped native coin (path B, multiplied by the coin's own
    // price). Ordered preferred-stable first, because
    // `first_grouped_quote_price` takes the FIRST pool that answers and the
    // order is the preference.
    let mut custom_quotes: Vec<(usize, Vec<(Vec<usize>, Option<usize>)>, Vec<usize>)> = Vec::new();
    if let Some(dex) = dex.as_ref() {
        let stable_order = stable_preference(&stables);
        for index in 0..slots.len() {
            if slots[index].category != Category::Custom {
                continue;
            }
            let (Some(contract), Some(amount_in)) = (
                slots[index].contract.clone(),
                slots[index].known_decimals.and_then(one_whole),
            ) else {
                continue;
            };
            let mut direct = Vec::new();
            for stable_index in &stable_order {
                let Some(stable) = stables.get(*stable_index) else {
                    continue;
                };
                let indices =
                    push_quote_calls(&mut calls, dex, &contract, &stable.contract, amount_in);
                if !indices.is_empty() {
                    direct.push((
                        indices,
                        slots
                            .get(native_slots + *stable_index)
                            .and_then(|slot| slot.decimals_at),
                    ));
                }
            }
            let via_native = match wrapped.as_ref() {
                Some(wrapped) => push_quote_calls(&mut calls, dex, &contract, wrapped, amount_in),
                None => Vec::new(),
            };
            if !direct.is_empty() || !via_native.is_empty() {
                custom_quotes.push((index, direct, via_native));
            }
        }
    }

    let local_feed_at = NATIVE_CHAINLINK_FEEDS
        .iter()
        .find(|(id, _)| *id == chain_id)
        .map(|(_, feed)| {
            let at = calls.len();
            calls.push(Call3 {
                target: (*feed).to_owned(),
                call_data: abi::enc_latest_round(),
            });
            at
        });

    let results = run_batch(chain_id, &calls);
    let answered = |at: usize| -> Option<&McResult> { results.get(at).filter(|r| r.success) };

    // The core's rules, given decoded numbers and nothing else.
    let groups: Vec<NativeQuoteGroup> = quote_groups
        .iter()
        .map(|(indices, decimals_at)| NativeQuoteGroup {
            amounts_out: indices
                .iter()
                .filter_map(|at| {
                    let result = answered(*at)?;
                    match dex.as_ref().map(|d| d.protocol) {
                        Some("solidly") => abi::dec_amounts_out(&result.data),
                        _ => abi::dec_u256(&result.data),
                    }
                })
                .collect(),
            // `None` means this group's own `decimals()` read failed, and the
            // core applies its own default to it — never a neighbour's real
            // value, which would mis-scale by orders of magnitude.
            quote_decimals: decimals_at
                .and_then(|at| answered(at))
                .and_then(|result| abi::dec_u8(&result.data))
                .map(u32::from),
        })
        .collect();
    let dex_price = best_native_dex_price(&groups);
    let local_price = local_feed_at
        .and_then(answered)
        .and_then(|result| abi::dec_chainlink_usd(&result.data));
    let mainnet_price = chainlink::resolve(&native_symbol, mainnet_prices);
    let native_price = choose_native_price(dex_price, local_price, mainnet_price).map(|p| p.price);

    let mut tokens = Vec::with_capacity(slots.len());
    for slot in &slots {
        let raw = match slot.balance_at {
            None => native_raw.to_owned(),
            Some(at) => match answered(at).and_then(|result| abi::dec_u256(&result.data)) {
                Some(raw) => raw,
                // A `balanceOf` that did not answer is a token we did not read.
                // Reporting it as zero would be a holding quietly deleted, so it
                // is left out of the list entirely.
                None => continue,
            },
        };
        let decimals = slot.known_decimals.or_else(|| {
            slot.decimals_at
                .and_then(answered)
                .and_then(|result| abi::dec_u8(&result.data))
                .map(u32::from)
        });
        // A token whose scale we could not read is NOT rendered at a guessed 18.
        // The web defaults here; on a 6-decimal stablecoin that prints a balance
        // a trillion times too small, which reads as an answer.
        let Some(decimals) = decimals else {
            continue;
        };
        let price_usd = match slot.category {
            Category::Native | Category::Wrapped => native_price,
            // $1.00 here is not a missing factor defaulting to one: `Stable` is
            // a MEMBERSHIP verdict — the token came from this chain's curated
            // stablecoin list — and ≈$1 is the definition of that membership.
            // The web owns this the same way and says so at length
            // (`wallet-api.ts:498-527`), including why a de-peg gate was
            // considered and rejected.
            Category::Stable => Some(1.0),
            Category::Custom => custom_price(
                slot,
                &slots,
                &custom_quotes,
                &results,
                dex.as_ref().map(|d| d.protocol),
                native_price,
                native_decimals,
            ),
        };
        tokens.push(BalanceToken {
            chain_id,
            symbol: slot.symbol.clone(),
            name: slot.name.clone(),
            // The HUMAN amount, not base units. `token_balance_double` parses
            // this with a fractional part and multiplies it by `price_usd`, so
            // raw units here would multiply every holding by 10^decimals.
            balance: abi::format_raw_balance(&raw, decimals),
            decimals,
            token_address: slot.contract.clone(),
            price_usd,
            spam: false,
        });
    }
    tokens
}

/// Tell the `token_trust` session what this fetch just learned.
///
/// **This fetch IS the observation.** Which chains the account holds on, which
/// ERC-20s it holds there, and what the registry says are that chain's
/// stablecoins — `token_trust` assembles its trusted-contract allowlist from
/// exactly those three, and re-deriving them later means a second multi-chain
/// read of the same facts.
///
/// Without them the core degrades correctly rather than wrongly: an empty
/// held-chains list polls the default payment chains, and a cold registry
/// leaves the trusted set at the person's own tokens plus the native sentinels.
/// Safe, and blind to a plain USDC payment — which is why they are worth
/// feeding.
fn inform_token_trust(address: &str, tokens: &[BalanceToken]) {
    let mut chains: Vec<u32> = Vec::new();
    for token in tokens {
        if !chains.contains(&token.chain_id) {
            chains.push(token.chain_id);
        }
    }
    token_trust::held_chains(address, chains.clone());
    for chain_id in chains {
        token_trust::held_tokens(
            address,
            chain_id,
            tokens
                .iter()
                .filter(|token| token.chain_id == chain_id)
                .filter_map(|token| token.token_address.clone())
                .collect(),
        );
        // Cached for 30 minutes, so this is a map lookup on every fetch after
        // the first.
        if let Some(data) = chain_tokens::fetch(chain_id) {
            token_trust::registry_tokens(
                chain_id,
                data.stables
                    .iter()
                    .map(|stable| stable.contract.clone())
                    .collect(),
                data.wrapped_native.clone(),
            );
        }
    }
}

/// Told when one chain's assets land, from that chain's own thread.
///
/// `Send + Sync` because twelve threads share one: the fan-out below is real
/// threads, not a sequence.
pub type ChainSink = dyn Fn(Vec<BalanceToken>) + Send + Sync;

/// Every chain's balances for one address, fetched in parallel and reported
/// only once they are all in.
///
/// Returns the tokens found and the chains that could not be reached, which the
/// core needs separately: the second list is what lets the home say "this chain
/// is unreachable" instead of adding a silent zero to the total.
///
/// For callers that want a total and nothing else — the account switcher, the
/// tests. The home screen uses [`fetch_all_streaming`].
#[must_use]
pub fn fetch_all(address: &str) -> (Vec<BalanceToken>, Vec<u32>) {
    let silent: Arc<ChainSink> = Arc::new(|_| {});
    fetch_all_streaming(address, &silent)
}

/// The same fan-out, saying what it has found as it finds it.
///
/// Twelve chains answer at twelve different speeds, and the join loop below
/// waits for all of them. Before this, the hero showed a skeleton (or the
/// cached total) until the SLOWEST chain replied — one unreachable RPC and
/// nothing on the screen moved for its entire timeout, even though eleven
/// chains had answered in a fraction of a second.
///
/// Each chain calls `arrived` with its OWN tokens, which is what the core's
/// merge expects: a snapshot replaces the chains it names and leaves the rest
/// alone (`chain_assets_arrived`, "slow chains keep last value"). A chain
/// holding nothing sends an empty snapshot, which correctly changes nothing.
///
/// The settle still happens exactly once, after the join — the core needs the
/// complete picture to decide about the cache write and the failed set, and no
/// snapshot may arrive after it.
#[must_use]
pub fn fetch_all_streaming(
    address: &str,
    arrived: &Arc<ChainSink>,
) -> (Vec<BalanceToken>, Vec<u32>) {
    // One batched read on Ethereum mainnet, before the fan-out, so twelve
    // threads share it instead of racing to fetch the same five feeds.
    let mainnet_prices = chainlink::prices();

    let mut handles = Vec::new();
    for (chain_id, symbol) in chains() {
        let address = address.to_owned();
        let mainnet_prices = mainnet_prices.clone();
        let arrived = Arc::clone(arrived);
        handles.push(
            thread::Builder::new()
                .name(format!("vela-balance-{chain_id}"))
                .spawn(move || match native_raw(chain_id, &address) {
                    Some(raw) => {
                        let found =
                            chain_tokens_for(chain_id, &symbol, &address, &raw, &mainnet_prices);
                        // Reported from this thread, the moment this chain is
                        // in — not after the eleven others.
                        arrived(found.clone());
                        (chain_id, Some(found))
                    }
                    None => (chain_id, None),
                })
                .ok(),
        );
    }

    let mut tokens = Vec::new();
    let mut failed = Vec::new();
    for handle in handles.into_iter().flatten() {
        match handle.join() {
            Ok((_, Some(found))) => tokens.extend(found),
            Ok((chain_id, None)) => failed.push(chain_id),
            // A panicked worker is a chain we did not read. It is not a reason
            // to lose the eleven that answered.
            Err(_) => {}
        }
    }
    inform_token_trust(address, &tokens);
    // Deterministic order: the core sorts for display, but a stable input makes
    // a test's failure readable.
    tokens.sort_by(|a, b| {
        a.chain_id
            .cmp(&b.chain_id)
            .then_with(|| a.token_address.is_some().cmp(&b.token_address.is_some()))
            .then_with(|| a.symbol.cmp(&b.symbol))
    });
    failed.sort_unstable();
    (tokens, failed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every built-in chain is read, and a custom network joins them.
    #[test]
    fn the_fetch_covers_builtins_and_custom_networks() {
        crate::executor::storage::tests::with_temp_state("balances-chains", || {
            assert_eq!(chains().len(), BUILTIN_CHAINS.len());

            let custom = json!([{ "chainId": 7_777_777, "nativeSymbol": "ETH" }]);
            if crate::executor::storage::write_value(
                crate::executor::storage::KEY_CUSTOM_NETWORKS,
                custom,
            )
            .is_err()
            {
                unreachable!("could not seed");
            }
            let all = chains();
            assert_eq!(all.len(), BUILTIN_CHAINS.len() + 1);
            assert!(all.iter().any(|(id, _)| *id == 7_777_777));

            // A custom network duplicating a built-in must not double-read it.
            let dup = json!([{ "chainId": 100, "nativeSymbol": "xDAI" }]);
            if crate::executor::storage::write_value(
                crate::executor::storage::KEY_CUSTOM_NETWORKS,
                dup,
            )
            .is_err()
            {
                unreachable!("could not seed");
            }
            assert_eq!(chains().len(), BUILTIN_CHAINS.len());
        });
    }

    /// A quote is only asked for a scale a whole coin can be expressed in.
    #[test]
    fn an_impossible_scale_is_not_quoted_rather_than_saturated() {
        assert_eq!(one_whole(18), Some(1_000_000_000_000_000_000));
        assert_eq!(one_whole(6), Some(1_000_000));
        assert_eq!(one_whole(0), Some(1));
        assert_eq!(one_whole(38), Some(10u128.pow(38)));
        assert_eq!(one_whole(39), None);
        assert_eq!(one_whole(255), None);
    }

    /// The chain whose `eth_getBalance` is a constant gets no native row.
    #[test]
    fn a_chain_with_no_native_coin_reports_no_native_coin() {
        assert!(has_native_coin(1));
        assert!(has_native_coin(100));
        assert!(has_native_coin(8453));
        for chain_id in TEMPO_CHAIN_IDS {
            assert!(
                !has_native_coin(chain_id),
                "chain {chain_id} settles gas in a TIP-20 stablecoin"
            );
        }
    }

    /// The two DEX protocols encode different calls, and an unknown one encodes
    /// none — falling through to Chainlink rather than sending nonsense.
    #[test]
    fn each_protocol_encodes_only_the_calls_it_can_read_back() {
        let uni = DexInfo {
            protocol: "uniswap-v3",
            quoter_v2: Some("0xquoter"),
            router: None,
        };
        let mut calls = Vec::new();
        let indices = push_quote_calls(&mut calls, &uni, "0xin", "0xout", 1);
        assert_eq!(indices.len(), FEE_TIERS.len());
        assert_eq!(calls.len(), FEE_TIERS.len());

        let aero = DexInfo {
            protocol: "solidly",
            quoter_v2: None,
            router: Some("0xrouter"),
        };
        let mut calls = Vec::new();
        let indices = push_quote_calls(&mut calls, &aero, "0xin", "0xout", 1);
        assert_eq!(indices.len(), 2, "volatile and stable routes");

        let curve = DexInfo {
            protocol: "curve",
            quoter_v2: None,
            router: Some("0xrouter"),
        };
        let mut calls = Vec::new();
        assert!(push_quote_calls(&mut calls, &curve, "0xin", "0xout", 1).is_empty());
        assert!(calls.is_empty());

        // A protocol naming a contract it does not use encodes nothing, rather
        // than sending a `quoteExactInputSingle` to a router.
        let mismatched = DexInfo {
            protocol: "uniswap-v3",
            quoter_v2: None,
            router: Some("0xrouter"),
        };
        let mut calls = Vec::new();
        assert!(push_quote_calls(&mut calls, &mismatched, "0xin", "0xout", 1).is_empty());
    }

    /// The preference order a custom token's quotes are tried in.
    #[test]
    fn the_preferred_stablecoin_is_quoted_first() {
        let stable = |symbol: &str, kind: &str| StableToken {
            symbol: symbol.to_owned(),
            kind: kind.to_owned(),
            contract: format!("0x{symbol}"),
        };
        // USDC.e first in chain order, native USDC second — the preference must
        // move the native one to the front and leave the rest alone.
        let stables = vec![
            stable("USDT", "bridge"),
            stable("USDC", "bridge"),
            stable("USDC", "native"),
            stable("DAI", "native"),
        ];
        assert_eq!(stable_preference(&stables), vec![2, 0, 1, 3]);

        // Already first: nothing moves.
        let leading = vec![stable("USDC", "native"), stable("DAI", "native")];
        assert_eq!(stable_preference(&leading), vec![0, 1]);

        assert!(stable_preference(&[]).is_empty());
    }

    /// The custom-token price rule, as the core applies it.
    ///
    /// Each group is scaled by its OWN quote token's decimals. A 6-decimal USDC
    /// group and an 18-decimal DAI group under one shared scale mis-price by
    /// 10^12, which is the bug the rule was extracted to prevent.
    #[test]
    fn a_custom_tokens_groups_are_each_scaled_by_their_own_quote_token() {
        use vela_core::app::balance_dashboard::first_grouped_quote_price;

        // No USDC pool, a live DAI pool: 1 token → 2.5 DAI, 18 decimals.
        let groups = vec![
            NativeQuoteGroup {
                amounts_out: Vec::new(),
                quote_decimals: Some(6),
            },
            NativeQuoteGroup {
                amounts_out: vec!["2500000000000000000".to_owned()],
                quote_decimals: Some(18),
            },
        ];
        let price = first_grouped_quote_price(&groups)
            .unwrap_or_else(|| unreachable!("the DAI pool answered"));
        assert!((price - 2.5).abs() < 1e-9, "{price}");

        // The first group that ANSWERS wins, not the largest.
        let ordered = vec![
            NativeQuoteGroup {
                amounts_out: vec!["1000000".to_owned()],
                quote_decimals: Some(6),
            },
            NativeQuoteGroup {
                amounts_out: vec!["9000000".to_owned()],
                quote_decimals: Some(6),
            },
        ];
        let price = first_grouped_quote_price(&ordered)
            .unwrap_or_else(|| unreachable!("the first pool answered"));
        assert!(
            (price - 1.0).abs() < 1e-9,
            "the preferred pool must win: {price}"
        );

        // A zero-output quote is a dead pool, not a free token.
        let dead = vec![NativeQuoteGroup {
            amounts_out: vec!["0".to_owned()],
            quote_decimals: Some(6),
        }];
        assert_eq!(first_grouped_quote_price(&dead), None);
        assert_eq!(first_grouped_quote_price(&[]), None);
    }

    /// The golden Safe, across every chain, through the pool.
    ///
    /// The assertion that matters is not the total: it is that Gnosis reports a
    /// real figure AND that chains which fail come back in `failed` rather than
    /// as a zero-balance token. A wallet that renders an unreachable chain as
    /// empty under-reports somebody's money and looks completely normal doing
    /// it.
    #[test]
    #[ignore = "reads every chain for a real address"]
    fn the_golden_safe_reads_across_chains() {
        crate::executor::storage::tests::with_temp_state("balances-live", || {
            chain_tokens::invalidate();
            chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let (tokens, failed) = fetch_all(GOLDEN);

            let chains_seen: std::collections::BTreeSet<u32> =
                tokens.iter().map(|t| t.chain_id).collect();
            println!(
                "  {} chains answered, {} did not, {} tokens",
                chains_seen.len(),
                failed.len(),
                tokens.len()
            );
            for token in &tokens {
                if token.balance != "0" {
                    println!(
                        "    chain {:>5} : {} {} @ {}",
                        token.chain_id,
                        token.balance,
                        token.symbol,
                        token
                            .price_usd
                            .map_or_else(|| "unpriced".to_owned(), |p| format!("${p:.4}"))
                    );
                }
            }
            if !failed.is_empty() {
                println!("    unreachable: {failed:?}");
            }

            // The measured hazard: Tempo answers a 4.24e75 constant to every
            // `eth_getBalance`, and its coin is called USD so it prices at $1.
            // Reachable, readable, and with nothing native to show.
            assert!(
                !tokens
                    .iter()
                    .any(|t| t.chain_id == 4_217 && t.token_address.is_none()),
                "Tempo has no native coin; its eth_getBalance is a constant"
            );

            let gnosis = tokens
                .iter()
                .find(|t| t.chain_id == 100 && t.token_address.is_none())
                .unwrap_or_else(|| unreachable!("Gnosis did not answer: failed={failed:?}"));
            // NOT an exact figure — a wallet balance is not a constant, and an
            // earlier version of this test went red when the Safe's balance
            // moved on-chain. What must hold is that Gnosis ANSWERED.
            let amount: f64 = gnosis
                .balance
                .parse()
                .unwrap_or_else(|_| unreachable!("not an amount: {}", gnosis.balance));
            assert!(amount > 0.0, "the golden Safe is funded");
            // The bug this pins: base units here would be ~7.7e17.
            assert!(
                amount < 1_000.0,
                "balance is the human amount, not base units: {}",
                gnosis.balance
            );
            assert_eq!(gnosis.symbol, "xDAI");
            assert_eq!(gnosis.decimals, 18);
            // xDAI is pegged and reachable by three separate rungs; a price
            // here is the whole point of the phase.
            let price = gnosis
                .price_usd
                .unwrap_or_else(|| unreachable!("xDAI came back unpriced"));
            assert!(price > 0.5 && price < 2.0, "implausible xDAI price {price}");

            // The two lists are disjoint by construction, and that is the
            // property the home screen depends on.
            for chain_id in &failed {
                assert!(
                    !tokens.iter().any(|t| t.chain_id == *chain_id),
                    "chain {chain_id} is both answered and unreachable"
                );
            }
        });
    }

    /// A custom ERC-20, priced by a real DEX quote.
    ///
    /// GNO on Gnosis: a token the person added by hand, which the balance fetch
    /// must both read and price. Before this the row came back with a real
    /// amount and no value at all.
    #[test]
    #[ignore = "quotes a real DEX on Gnosis"]
    fn a_custom_token_is_priced_by_the_chains_own_pools() {
        crate::executor::storage::tests::with_temp_state("balances-custom-price", || {
            chain_tokens::invalidate();
            chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            const GNO: &str = "0x9C58BAcC331c9aa871AFD802DB6379a98e80CEdb";

            let saved = custom_tokens::save(custom_tokens::StoredToken {
                id: format!("100_{GNO}"),
                chain_id: 100,
                contract_address: GNO.to_owned(),
                symbol: "GNO".to_owned(),
                name: "Gnosis Token".to_owned(),
                decimals: 18,
                network_name: "Gnosis".to_owned(),
            });
            assert!(saved, "could not seed the custom token");

            let raw =
                native_raw(100, GOLDEN).unwrap_or_else(|| unreachable!("Gnosis did not answer"));
            let prices = chainlink::prices();
            let tokens = chain_tokens_for(100, "xDAI", GOLDEN, &raw, &prices);

            let gno = tokens
                .iter()
                .find(|token| token.symbol == "GNO")
                .unwrap_or_else(|| unreachable!("the custom token was not read"));
            println!(
                "    GNO: {} @ {}",
                gno.balance,
                gno.price_usd
                    .map_or_else(|| "unpriced".to_owned(), |p| format!("${p:.2}"))
            );
            assert_eq!(gno.decimals, 18);
            assert_eq!(gno.token_address.as_deref(), Some(GNO));

            // A band, not a figure: GNO's price moves, and a test that pins it
            // goes red when the market does. What must hold is that a price
            // arrived and is not a decode artefact.
            let price = gno
                .price_usd
                .unwrap_or_else(|| unreachable!("GNO came back unpriced"));
            assert!(
                price > 1.0 && price < 10_000.0,
                "implausible GNO price {price}"
            );
        });
    }

    /// One chain, in detail: the ERC-20 rows and where each price came from.
    #[test]
    #[ignore = "reads Gnosis for a real address"]
    fn gnosis_reads_its_stablecoins_and_prices_its_coin() {
        crate::executor::storage::tests::with_temp_state("balances-gnosis", || {
            chain_tokens::invalidate();
            chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let raw = native_raw(100, GOLDEN)
                .unwrap_or_else(|| unreachable!("Gnosis did not answer eth_getBalance"));
            let prices = chainlink::prices();
            let tokens = chain_tokens_for(100, "xDAI", GOLDEN, &raw, &prices);

            for token in &tokens {
                println!(
                    "    {:>8} {:<8} {:>2}dp  {}",
                    token.balance,
                    token.symbol,
                    token.decimals,
                    token
                        .price_usd
                        .map_or_else(|| "unpriced".to_owned(), |p| format!("${p:.4}"))
                );
            }

            // The native coin, plus the index's stablecoins, plus WXDAI.
            assert!(tokens.len() >= 3, "only {} rows", tokens.len());
            let native = tokens
                .iter()
                .find(|t| t.token_address.is_none())
                .unwrap_or_else(|| unreachable!("no native row"));
            assert_eq!(
                native.symbol, "xDAI",
                "the wallet's spelling, not the index's"
            );
            assert!(native.price_usd.is_some());

            // Every stablecoin row carries its OWN scale, read from its own
            // contract — USDC's 6 must never be borrowed from DAI's 18.
            for token in tokens.iter().filter(|t| t.symbol.starts_with("USD")) {
                assert_eq!(token.decimals, 6, "{} decimals", token.symbol);
                assert_eq!(token.price_usd, Some(1.0));
            }
        });
    }
}
