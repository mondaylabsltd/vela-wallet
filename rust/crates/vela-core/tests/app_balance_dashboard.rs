//! Rules of the balance-dashboard machine, one test per rule.
//!
//! Inventory invariants ①–⑩ each have at least one test named after the rule;
//! the native-coin pricing vectors pin `wallet-api.ts:289-427` (deepest pool,
//! per-stable decimals, the DEX↔Chainlink sanity band — the X Layer WOKB
//! incident included), and the `parseFloat` port is pinned against JS
//! semantics. The machine tests drive the core exactly the way the shell
//! will: dispatch an event, answer the operations one at a time.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::balance_dashboard::{
    best_group_price, best_native_dex_price, choose_native_price, first_grouped_quote_price,
    pegged_native_usd, token_balance_double, token_usd_value, BalanceCacheEntry, BalanceDashboard,
    BalanceNotice, BalanceOperation as Op, BalanceShellResult as Res, BalanceToken, Event,
    NativePriceSource, NativeQuoteGroup, FALLBACK_RETRY_DELAY_MS, MAX_PARTIAL_RETRIES,
    PARTIAL_RETRY_DELAYS_MS,
};

type Sut = DomainDriver<BalanceDashboard>;

const ADDR_A: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const ADDR_B: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const NOW: f64 = 1_700_000_000_000.0;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn token(chain_id: u32, symbol: &str, balance: &str, price_usd: Option<f64>) -> BalanceToken {
    BalanceToken {
        chain_id,
        symbol: symbol.to_owned(),
        name: symbol.to_owned(),
        balance: balance.to_owned(),
        decimals: 18,
        token_address: None,
        price_usd,
        spam: false,
    }
}

fn settled(address: &str, tokens: Vec<BalanceToken>, failed: Vec<u32>, limited: Vec<u32>) -> Res {
    Res::FetchSettled {
        address: address.to_owned(),
        pull: false,
        tokens,
        failed_chain_ids: failed,
        rate_limited_chain_ids: limited,
        now_ms: NOW,
    }
}

/// Fresh machine with the account resolved; the cache read and the initial
/// fetch are left outstanding (resolve them in that order).
fn boot(address: &str) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::AccountChanged {
        address: address.to_owned(),
    });
    assert_eq!(
        ops,
        vec![
            Op::ReadBalanceCache {
                address: address.to_owned()
            },
            Op::FetchTokens {
                address: address.to_owned(),
                force: false,
                pull: false
            },
        ]
    );
    sut
}

/// Booted machine whose cache answered `cached` and whose first fetch settled
/// with `result`.
fn booted(address: &str, cached: Option<f64>, result: Res) -> Sut {
    let mut sut = boot(address);
    let ops = sut.resolve(Res::CachedTotalLoaded {
        address: address.to_owned(),
        usd: cached,
    });
    assert!(ops.is_empty());
    sut.resolve(result);
    sut
}

// ===========================================================================
// Pure value math — the parseFloat port (f64 verbatim, open question 5)
// ===========================================================================

#[test]
fn token_balance_double_ports_parse_float_or_zero() {
    // Clean decimal shapes formatRawBalance emits.
    assert_eq!(token_balance_double("1.5"), 1.5);
    assert_eq!(token_balance_double("0"), 0.0);
    assert_eq!(token_balance_double("12.34"), 12.34);
    // parseFloat quirks pinned: prefix scan, whitespace, exponent, NaN → 0.
    assert_eq!(token_balance_double(""), 0.0); // NaN || 0
    assert_eq!(token_balance_double("abc"), 0.0); // NaN || 0
    assert_eq!(token_balance_double("5abc"), 5.0); // longest numeric prefix
    assert_eq!(token_balance_double(" 7"), 7.0); // leading whitespace trimmed
    assert_eq!(token_balance_double("1e3"), 1000.0);
    assert_eq!(token_balance_double("2.5e-1"), 0.25);
    assert_eq!(token_balance_double("1e"), 1.0); // dangling exponent ignored
                                                 // `-0 || 0` is 0 in JS — the sign never survives.
    assert_eq!(token_balance_double("-0").to_bits(), 0f64.to_bits());
}

#[test]
fn token_usd_value_treats_a_missing_price_as_zero() {
    assert_eq!(
        token_usd_value(&token(1, "ETH", "2", Some(1868.70))),
        3737.4
    );
    assert_eq!(token_usd_value(&token(1, "MYSTERY", "1000", None)), 0.0);
    assert_eq!(token_usd_value(&token(1, "DUST", "0", Some(5.0))), 0.0);
}

// ===========================================================================
// Native-coin pricing — `wallet-api.ts:289-427` vectors
// ===========================================================================

#[test]
fn best_group_price_takes_the_deepest_pool_and_skips_zero_quotes() {
    // Two fee tiers answered; the more-liquid pool returns more output.
    let group = NativeQuoteGroup {
        amounts_out: vec!["5000000".to_owned(), "81000000".to_owned()],
        quote_decimals: Some(6),
    };
    assert_eq!(best_group_price(&group), Some(81.0));
    // `amountOut > 0n`: zero output is not a price.
    let zeroes = NativeQuoteGroup {
        amounts_out: vec!["0".to_owned()],
        quote_decimals: Some(6),
    };
    assert_eq!(best_group_price(&zeroes), None);
    assert_eq!(
        best_group_price(&NativeQuoteGroup {
            amounts_out: vec![],
            quote_decimals: Some(6)
        }),
        None
    );
    // A failed decimals() read defaults to 6 (`wallet-api.ts:379`).
    let defaulted = NativeQuoteGroup {
        amounts_out: vec!["2000000".to_owned()],
        quote_decimals: None,
    };
    assert_eq!(best_group_price(&defaulted), Some(2.0));
}

/// The X Layer WOKB case: the broken USDC pool quotes OKB at ~$5 while the
/// liquid USD₮0 pool holds ~$81 — the max across groups routes around the
/// junk. Each group is normalized by its OWN stable's decimals first.
#[test]
fn native_dex_price_maxes_across_stable_groups_with_their_own_decimals() {
    let groups = vec![
        NativeQuoteGroup {
            amounts_out: vec!["5000000".to_owned()], // $5 in 6-dp USDC
            quote_decimals: Some(6),
        },
        NativeQuoteGroup {
            amounts_out: vec!["81000000".to_owned()], // $81 in 6-dp USD₮0
            quote_decimals: Some(6),
        },
    ];
    assert_eq!(best_native_dex_price(&groups), Some(81.0));

    // USDC=6 vs DAI=18 must never share one scale: 6e6 @6dp is $6, and
    // 5e18 @18dp is $5 — the RAW amounts compare the other way around.
    let mixed = vec![
        NativeQuoteGroup {
            amounts_out: vec!["5000000000000000000".to_owned()], // $5 in DAI
            quote_decimals: Some(18),
        },
        NativeQuoteGroup {
            amounts_out: vec!["6000000".to_owned()], // $6 in USDC
            quote_decimals: Some(6),
        },
    ];
    assert_eq!(best_native_dex_price(&mixed), Some(6.0));
    assert_eq!(best_native_dex_price(&[]), None);
}

#[test]
fn first_grouped_quote_price_takes_the_preferred_venue_not_the_deepest_pool() {
    // The two rules answer different questions and must not converge. Groups
    // arrive in the shell's preference order — native USDC, then any USDC,
    // then USDT — and the FIRST that answers wins, even when a later one
    // quotes higher. For an arbitrary token the preferred venue is the
    // trustworthy one; a deeper pool elsewhere may be a different asset with a
    // similar ticker.
    let groups = vec![
        NativeQuoteGroup {
            amounts_out: vec!["2000000".to_owned()], // $2 on the preferred stable
            quote_decimals: Some(6),
        },
        NativeQuoteGroup {
            amounts_out: vec!["9000000".to_owned()], // $9 somewhere else
            quote_decimals: Some(6),
        },
    ];
    assert_eq!(first_grouped_quote_price(&groups), Some(2.0));
    // The native rule, on the same input, takes the deepest.
    assert_eq!(best_native_dex_price(&groups), Some(9.0));
}

#[test]
fn first_grouped_quote_price_scales_each_group_by_its_own_decimals() {
    // **The 10^12 trap.** A chain whose stablecoin list holds both a 6-decimal
    // USDC and an 18-decimal DAI, where only the DAI pool answers. Scaling that
    // quote by the neighbouring USDC's decimals prices the token a trillion
    // times too high — and that number reaches a portfolio total, a sort order
    // and an ingest valuation.
    let groups = vec![
        NativeQuoteGroup {
            amounts_out: vec![], // USDC: dead pool
            quote_decimals: Some(6),
        },
        NativeQuoteGroup {
            amounts_out: vec!["250000000000000000".to_owned()], // 0.25 DAI
            quote_decimals: Some(18),
        },
    ];
    assert_eq!(first_grouped_quote_price(&groups), Some(0.25));
}

#[test]
fn first_grouped_quote_price_defaults_within_its_own_group() {
    // A failed `decimals()` read falls back to THIS group's default, never to
    // a neighbour's real value.
    let groups = vec![NativeQuoteGroup {
        amounts_out: vec!["3000000".to_owned()],
        quote_decimals: None, // ⇒ DEFAULT_QUOTE_DECIMALS (6)
    }];
    assert_eq!(first_grouped_quote_price(&groups), Some(3.0));
}

#[test]
fn first_grouped_quote_price_refuses_zero_and_junk() {
    // A zero-output quote is a dead pool, not a free token; it must not price,
    // and it must not stop a later group from doing so.
    let groups = vec![
        NativeQuoteGroup {
            amounts_out: vec!["0".to_owned(), "not a number".to_owned()],
            quote_decimals: Some(6),
        },
        NativeQuoteGroup {
            amounts_out: vec!["1500000".to_owned()],
            quote_decimals: Some(6),
        },
    ];
    assert_eq!(first_grouped_quote_price(&groups), Some(1.5));
    assert_eq!(first_grouped_quote_price(&[]), None);
}

#[test]
fn choose_native_price_prefers_dex_only_inside_the_sanity_band() {
    // Inside (0.5, 2.0): DEX wins.
    let picked = choose_native_price(Some(81.0), Some(80.0), None).unwrap();
    assert_eq!(picked.price, 81.0);
    assert_eq!(picked.source, NativePriceSource::Dex);
    // Broken pool far below Chainlink → Chainlink(sanity).
    let picked = choose_native_price(Some(5.0), Some(80.0), None).unwrap();
    assert_eq!(picked.price, 80.0);
    assert_eq!(picked.source, NativePriceSource::ChainlinkSanity);
    // Far above → also Chainlink(sanity).
    let picked = choose_native_price(Some(200.0), Some(80.0), None).unwrap();
    assert_eq!(picked.source, NativePriceSource::ChainlinkSanity);
    // The band is EXCLUSIVE at both edges (`ratio > 0.5 && ratio < 2.0`).
    let picked = choose_native_price(Some(40.0), Some(80.0), None).unwrap();
    assert_eq!(picked.source, NativePriceSource::ChainlinkSanity);
    let picked = choose_native_price(Some(160.0), Some(80.0), None).unwrap();
    assert_eq!(picked.source, NativePriceSource::ChainlinkSanity);
}

#[test]
fn choose_native_price_walks_the_source_ladder() {
    // DEX alone.
    let picked = choose_native_price(Some(81.0), None, None).unwrap();
    assert_eq!(
        (picked.price, picked.source),
        (81.0, NativePriceSource::Dex)
    );
    // Local feed preferred over the Ethereum-mainnet fallback.
    let picked = choose_native_price(None, Some(80.0), Some(79.0)).unwrap();
    assert_eq!(
        (picked.price, picked.source),
        (80.0, NativePriceSource::ChainlinkLocal)
    );
    let picked = choose_native_price(None, None, Some(79.0)).unwrap();
    assert_eq!(
        (picked.price, picked.source),
        (79.0, NativePriceSource::ChainlinkEth)
    );
    assert_eq!(choose_native_price(None, None, None), None);
    // The decode gate travels with the local feed: 0 is not a price, so the
    // band compares against the Ethereum fallback instead.
    let picked = choose_native_price(Some(81.0), Some(0.0), Some(80.0)).unwrap();
    assert_eq!(
        (picked.price, picked.source),
        (81.0, NativePriceSource::Dex)
    );
}

// ===========================================================================
// Machine — account lifecycle and the display rule
// ===========================================================================

/// ②: nothing known yet (no live data, no cache, first fetch in flight) →
/// skeleton, never a fake $0 that later jumps to the real value.
#[test]
fn unknown_balance_is_a_skeleton_never_a_fake_zero() {
    let mut sut = boot(ADDR_A);
    let view = sut.view();
    assert!(view.balance_unknown);
    assert_eq!(view.display_total_usd, None);

    // Cache: nothing stored → still skeleton.
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: None,
    });
    assert!(sut.view().balance_unknown);

    // A COMPLETE empty settle is a genuine $0 wallet — skeleton off, and the
    // honest zero is persisted as last-known-good.
    let ops = sut.resolve(settled(ADDR_A, vec![], vec![], vec![]));
    assert_eq!(
        ops,
        vec![Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 0.0
        }]
    );
    let view = sut.view();
    assert!(!view.balance_unknown);
    assert_eq!(view.display_total_usd, Some(0.0));
}

/// The cached total paints the hero before any live data arrives — and drives
/// the holdings-list loading hint (`HomeScreen.tsx:271`).
#[test]
fn cached_total_paints_the_hero_before_live_data() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(1234.5),
    });
    let view = sut.view();
    assert!(!view.balance_unknown);
    assert_eq!(view.display_total_usd, Some(1234.5));
    assert!(view.holdings_loading, "tokens empty + cached > 0");
}

/// ①: a partial live sum must never undershow — the hero takes
/// `max(live, cached)`.
#[test]
fn partial_result_never_undershows_max_of_live_and_cached() {
    // Live undercount (a chain failed): cached 100 beats live 40.
    let sut = booted(
        ADDR_A,
        Some(100.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "40", Some(1.0))],
            vec![56],
            vec![],
        ),
    );
    let view = sut.view();
    assert!(view.balance_partial);
    assert_eq!(view.display_total_usd, Some(100.0));

    // Live overtook the cache: the larger number wins.
    let sut = booted(
        ADDR_A,
        Some(100.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "150", Some(1.0))],
            vec![56],
            vec![],
        ),
    );
    assert_eq!(sut.view().display_total_usd, Some(150.0));
}

/// An unpriced HELD token also marks the sum partial — and shows up in the
/// detail sheet's list (spam excluded).
#[test]
fn unpriced_held_token_marks_partial_and_lists_in_the_detail_sheet() {
    let mut spam = token(1, "AIRDROP", "9999", None);
    spam.spam = true;
    let sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![
                token(1, "ETH", "1", Some(100.0)),
                token(1, "MYSTERY", "5", None),
                spam,
            ],
            vec![],
            vec![],
        ),
    );
    let view = sut.view();
    assert!(view.balance_partial);
    assert_eq!(view.unpriced_tokens.len(), 1);
    assert_eq!(view.unpriced_tokens[0].symbol, "MYSTERY");
}

/// ④: mid-refresh, chains that already answered replace their tokens while
/// slow chains keep their last value — the total never drops to $0.
#[test]
fn slow_chains_keep_their_last_value_mid_refresh() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![
                token(1, "ETH", "10", Some(1.0)),
                token(56, "BNB", "20", Some(1.0)),
            ],
            vec![],
            vec![],
        ),
    );
    assert_eq!(sut.view().display_total_usd, Some(30.0));

    // A new refresh streams: chain 1 answered (higher), chain 56 still slow.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: true,
    });
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(1, "ETH", "15", Some(1.0))],
    });
    let view = sut.view();
    // #188: the LIST streams (56 kept its last value, 1 updated), and the
    // figure follows it UP — what is already known shows at once.
    assert_eq!(view.display_total_usd, Some(35.0), "rose with the stream");
    assert_eq!(view.tokens.len(), 2);
    assert_eq!(view.tokens[0].symbol, "BNB", "sorted by USD value desc");
    assert!(view.refreshing);
    sut.resolve(settled(
        ADDR_A,
        vec![
            token(1, "ETH", "15", Some(1.0)),
            token(56, "BNB", "20", Some(1.0)),
        ],
        vec![],
        vec![],
    ));
    assert_eq!(
        sut.view().display_total_usd,
        Some(35.0),
        "and the settle agrees"
    );
}

/// #188, a first load over a cached total: the figure starts at the cache and
/// only RISES as chains answer, and "some tokens couldn't be priced" must not
/// flash before the prices have arrived.
#[test]
fn the_figure_only_rises_while_loading_and_unpriced_waits_for_settle() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(62.0),
    });
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(1, "ETH", "50", Some(1.0))],
    });
    assert_eq!(
        sut.view().display_total_usd,
        Some(62.0),
        "below the cache: the cache holds"
    );
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(56, "BNB", "40", Some(1.0))],
    });
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(137, "MATIC", "5", None)],
    });
    let view = sut.view();
    assert_eq!(view.display_total_usd, Some(90.0), "rose past the cache");
    assert_eq!(view.tokens.len(), 3, "the list streams");
    assert_eq!(view.notice, None, "no unpriced notice mid-stream");
    // Settle: everything priced now.
    sut.resolve(settled(
        ADDR_A,
        vec![
            token(1, "ETH", "50", Some(1.0)),
            token(56, "BNB", "40", Some(1.0)),
            token(137, "MATIC", "5", Some(1.0)),
        ],
        vec![],
        vec![],
    ));
    let view = sut.view();
    assert_eq!(view.display_total_usd, Some(95.0));
    assert_eq!(view.notice, None);
    assert!(!view.unreachable);
}

/// #188, the cold first load — the reporter's screen, made fast: the first
/// chain with a holding ends the skeleton, and the figure grows as the others
/// answer. It never steps down mid-load.
#[test]
fn a_cold_first_load_shows_what_it_has_and_only_grows() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: None,
    });
    assert_eq!(
        sut.view().display_total_usd,
        None,
        "nothing known: skeleton"
    );
    // A chain that lists only an unpriced token is no figure yet: the
    // skeleton stays, never a $0.00.
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(137, "MYSTERY", "5", None)],
    });
    let view = sut.view();
    assert!(view.balance_unknown);
    assert_eq!(view.display_total_usd, None, "still a skeleton, not $0.00");
    assert_eq!(view.tokens.len(), 1, "the list streams");
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(1, "ETH", "50", Some(1.0))],
    });
    let view = sut.view();
    assert!(!view.balance_unknown);
    assert_eq!(view.display_total_usd, Some(50.0), "shown at once");
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(56, "BNB", "40", Some(1.0))],
    });
    assert_eq!(sut.view().display_total_usd, Some(90.0), "grew");
    // A later snapshot re-reports chain 1 lower: the list follows, the figure
    // does not step down before the settle.
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(1, "ETH", "20", Some(1.0))],
    });
    let view = sut.view();
    assert_eq!(view.display_total_usd, Some(90.0), "never down mid-load");
    assert_eq!(view.notice, None);
    sut.resolve(settled(
        ADDR_A,
        vec![
            token(1, "ETH", "20", Some(1.0)),
            token(56, "BNB", "40", Some(1.0)),
        ],
        vec![],
        vec![],
    ));
    assert_eq!(sut.view().display_total_usd, Some(60.0), "the settle rules");
}

/// #188's second half — "after a send the total keeps the old number". A
/// wallet holding one unpriced token is `balance_partial` for ever, and the
/// `max(live, cached)` floor pinned its total at the pre-send figure. The
/// floor is for chains that did NOT answer; every chain answered here, so
/// the priced sum is shown, persisted, and the switcher row moves with it.
#[test]
fn a_send_lowers_the_figure_even_with_an_unpriced_token_held() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(150.0),
    });
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![
            token(1, "ETH", "100", Some(1.0)),
            token(1, "MYSTERY", "5", None),
        ],
        vec![],
        vec![],
    ));
    let view = sut.view();
    assert!(
        view.balance_partial,
        "the unpriced token still marks partial"
    );
    assert_eq!(
        view.display_total_usd,
        Some(100.0),
        "the priced sum, not the stale floor"
    );
    assert!(
        ops.contains(&Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 100.0
        }),
        "every chain answered: persisted — {ops:?}"
    );
    assert!(
        ops.iter()
            .any(|op| matches!(op, Op::StartRetryTimer { .. })),
        "the unpriced token still earns its silent retries — {ops:?}"
    );
    assert_eq!(view.cached_total_usd, Some(100.0));

    // 40 ETH leaves; the refresh the confirmation triggers settles lower.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![
            token(1, "ETH", "60", Some(1.0)),
            token(1, "MYSTERY", "5", None),
        ],
    });
    assert_eq!(
        sut.view().display_total_usd,
        Some(100.0),
        "held while the fetch is out — down only at settle"
    );
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![
            token(1, "ETH", "60", Some(1.0)),
            token(1, "MYSTERY", "5", None),
        ],
        vec![],
        vec![],
    ));
    let view = sut.view();
    assert_eq!(view.display_total_usd, Some(60.0), "the send shows");
    assert!(ops.contains(&Op::WriteBalanceCache {
        address: ADDR_A.to_owned(),
        usd: 60.0
    }));
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_A.to_owned(),
        usd: 60.0
    }));

    // A chain that did NOT answer is a different matter: the floor stands.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "10", Some(1.0))],
        vec![56],
        vec![],
    ));
    assert_eq!(
        sut.view().display_total_usd,
        Some(60.0),
        "max(live, cached)"
    );
    assert!(
        ops.iter()
            .all(|op| !matches!(op, Op::WriteBalanceCache { .. })),
        "a round missing a chain is never persisted — {ops:?}"
    );
}

/// A wallet emptied by a send is a live $0, and the figure the NEXT refresh
/// holds at is that $0 — not the total it had before.
#[test]
fn an_emptied_wallet_holds_at_its_new_zero() {
    let mut sut = booted(
        ADDR_A,
        Some(100.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "100", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled(ADDR_A, vec![], vec![], vec![]));
    assert_eq!(sut.view().display_total_usd, Some(0.0));
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    assert_eq!(
        sut.view().display_total_usd,
        Some(0.0),
        "held at the new zero, not the old total"
    );
}

/// Spec 038 finding 15: a first launch with the network cut is "unreachable",
/// never a settled-looking $0.00.
#[test]
fn an_errored_first_load_is_unreachable_not_zero() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: None,
    });
    sut.resolve(Res::FetchErrored {
        address: ADDR_A.to_owned(),
        pull: false,
    });
    let view = sut.view();
    assert!(view.unreachable);
    assert!(
        !view.balance_unknown,
        "the skeleton closed; the reason is different"
    );
    assert_eq!(view.tokens.len(), 0);
    // A later successful fetch clears it.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "1", Some(1.0))],
        vec![],
        vec![],
    ));
    assert!(!sut.view().unreachable);
}

/// ⑤: a previous account's slow answers can never paint the new account —
/// dropped by construction (attempt tag on results, address tag on streams).
#[test]
fn stale_account_results_are_dropped_by_construction() {
    let mut sut = boot(ADDR_A);
    // Switch before anything for A resolves.
    sut.dispatch(Event::AccountChanged {
        address: ADDR_B.to_owned(),
    });

    // A's cache read answers late → dropped (attempt).
    assert!(sut
        .resolve(Res::CachedTotalLoaded {
            address: ADDR_A.to_owned(),
            usd: Some(500.0),
        })
        .is_empty());
    assert_eq!(sut.view().cached_total_usd, None);

    // A's fetch settles late → dropped: no cache write, no tokens, still B.
    assert!(sut
        .resolve(settled(
            ADDR_A,
            vec![token(1, "ETH", "9", Some(1.0))],
            vec![],
            vec![]
        ))
        .is_empty());
    let view = sut.view();
    assert_eq!(view.address.as_deref(), Some(ADDR_B));
    assert!(view.tokens.is_empty());
    assert!(view.balance_unknown, "B still knows nothing");

    // A's stream arrives late → address tag drops it.
    sut.dispatch(Event::ChainAssetsArrived {
        address: ADDR_A.to_owned(),
        tokens: vec![token(1, "ETH", "9", Some(1.0))],
    });
    assert!(sut.view().tokens.is_empty());
}

/// The account-change reset — and its two verbatim non-resets
/// (`rateLimitedChainIds`, `lastRefreshedAt` survive the switch).
#[test]
fn account_switch_resets_balance_state_but_keeps_the_ported_quirks() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "10", Some(1.0))],
            vec![10],
            vec![10],
        ),
    );
    let view = sut.view();
    assert_eq!(view.rate_limited_chain_ids, vec![10]);
    assert_eq!(view.last_refreshed_at_ms, Some(NOW));

    sut.dispatch(Event::AccountChanged {
        address: ADDR_B.to_owned(),
    });
    let view = sut.view();
    assert!(view.tokens.is_empty());
    assert!(view.failed_chain_ids.is_empty());
    assert_eq!(view.cached_total_usd, None);
    assert!(view.balance_unknown);
    // Ported verbatim: the reset never touches these two.
    assert_eq!(view.rate_limited_chain_ids, vec![10]);
    assert_eq!(view.last_refreshed_at_ms, Some(NOW));
}

// ===========================================================================
// Machine — the silent retry ladder and the notice gate
// ===========================================================================

/// ③: a routine hiccup never shouts "still updating" — three silent forced
/// retries at [1500, 4000, 8000]ms come first; only exhaustion allows the
/// notice.
#[test]
fn three_silent_retries_then_notice() {
    let mut sut = booted(
        ADDR_A,
        Some(100.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "40", Some(1.0))],
            vec![56],
            vec![],
        ),
    );

    for (round, expected_ms) in PARTIAL_RETRY_DELAYS_MS.iter().enumerate() {
        // The grace: no notice while retries remain.
        assert_eq!(sut.view().notice, None, "round {round}: silent");
        // The timer that was just armed fires …
        let timer_id = u32::try_from(round).unwrap() + 1;
        let ops = sut.resolve(Res::RetryElapsed { timer_id });
        // … and forces a refetch (never a pull — no spinner).
        assert_eq!(
            ops,
            vec![Op::FetchTokens {
                address: ADDR_A.to_owned(),
                force: true,
                pull: false
            }],
            "round {round} at {expected_ms}ms"
        );
        assert!(!sut.view().refreshing);
        sut.resolve(settled(
            ADDR_A,
            vec![token(1, "ETH", "40", Some(1.0))],
            vec![56],
            vec![],
        ));
    }

    // Budget exhausted and STILL incomplete — now the notice is honest, and
    // no further timer is armed.
    let view = sut.view();
    assert_eq!(view.notice, Some(BalanceNotice::StillUpdating));
    assert!(sut.outstanding().is_empty(), "no fourth retry");
    // max(live, cached) still protects the number the whole time.
    assert_eq!(view.display_total_usd, Some(100.0));
}

/// The delay table is consumed in order; each partial settle arms exactly one
/// timer with the escalating delay.
#[test]
fn retry_delays_escalate_in_table_order() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: None,
    });
    let mut seen = Vec::new();
    let mut ops = sut.resolve(settled(ADDR_A, vec![], vec![56], vec![]));
    for round in 0u32..MAX_PARTIAL_RETRIES {
        let [Op::StartRetryTimer { ms, timer_id }] = ops.as_slice() else {
            panic!("round {round}: expected exactly one armed timer, got {ops:?}");
        };
        seen.push(*ms);
        let fetch = sut.resolve(Res::RetryElapsed {
            timer_id: *timer_id,
        });
        assert_eq!(fetch.len(), 1);
        ops = sut.resolve(settled(ADDR_A, vec![], vec![56], vec![]));
    }
    assert_eq!(seen, PARTIAL_RETRY_DELAYS_MS.to_vec());
    assert!(ops.is_empty(), "exhausted: no timer, notice instead");
    assert_eq!(sut.view().notice, Some(BalanceNotice::StillUpdating));
    // The `?? 8000` fallback constant is pinned too.
    assert_eq!(FALLBACK_RETRY_DELAY_MS, 8_000);
}

/// A clean result resets the budget and clears the notice, so a later hiccup
/// gets its own grace.
#[test]
fn clean_result_resets_the_retry_budget_and_notice() {
    let mut sut = booted(ADDR_A, None, settled(ADDR_A, vec![], vec![56], vec![]));
    // Exhaust the budget.
    for round in 0..MAX_PARTIAL_RETRIES {
        sut.resolve(Res::RetryElapsed {
            timer_id: round + 1,
        });
        sut.resolve(settled(ADDR_A, vec![], vec![56], vec![]));
    }
    assert_eq!(sut.view().notice, Some(BalanceNotice::StillUpdating));

    // A later poll settles COMPLETE: notice off, budget refilled …
    sut.dispatch(Event::RefreshRequested {
        force: false,
        pull: false,
    });
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "5", Some(1.0))],
        vec![],
        vec![],
    ));
    assert_eq!(
        ops,
        vec![Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 5.0
        }]
    );
    assert_eq!(sut.view().notice, None);

    // … so the next hiccup starts back at 1500ms.
    sut.dispatch(Event::RefreshRequested {
        force: false,
        pull: false,
    });
    let ops = sut.resolve(settled(ADDR_A, vec![], vec![56], vec![]));
    assert_eq!(
        ops,
        vec![Op::StartRetryTimer {
            ms: PARTIAL_RETRY_DELAYS_MS[0],
            timer_id: MAX_PARTIAL_RETRIES + 1
        }]
    );
}

/// A settle cancels any armed timer (`clearTimeout`); its late echo must not
/// trigger a second refetch.
#[test]
fn stale_retry_timer_never_fires() {
    let mut sut = booted(ADDR_A, None, settled(ADDR_A, vec![], vec![56], vec![]));
    // A complete settle lands before the timer fires → timer cancelled.
    sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "5", Some(1.0))],
        vec![],
        vec![],
    ));
    // The shell's timer still fires eventually — dropped, no fetch.
    let ops = sut.resolve(Res::RetryElapsed { timer_id: 1 });
    assert!(ops.is_empty());
}

/// A fetch that THREW keeps last-known everything and only closes the
/// skeleton (`catch {}` + `setBootstrapped`).
#[test]
fn fetch_error_closes_the_skeleton_but_keeps_last_known() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(100.0),
    });
    sut.resolve(Res::FetchErrored {
        address: ADDR_A.to_owned(),
        pull: false,
    });
    let view = sut.view();
    assert!(!view.balance_unknown);
    assert_eq!(view.display_total_usd, Some(100.0), "cache still paints");
    assert_eq!(view.notice, None);
    assert!(sut.outstanding().is_empty(), "no retry timer for a throw");
}

// ===========================================================================
// Machine — invariant ⑥: the cache write gate
// ===========================================================================

#[test]
fn partial_totals_never_touch_the_cache() {
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(100.0),
    });
    // Partial settle: NO WriteBalanceCache — a partial write would poison the
    // max(live, cached) floor. (The retry timer is the only operation.)
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "150", Some(1.0))],
        vec![56],
        vec![],
    ));
    assert!(
        ops.iter()
            .all(|op| !matches!(op, Op::WriteBalanceCache { .. })),
        "partial result must not be persisted: {ops:?}"
    );
    assert_eq!(sut.view().cached_total_usd, Some(100.0), "unchanged");

    // Complete settle: persisted, and the model's floor moves with it.
    sut.resolve(Res::RetryElapsed { timer_id: 1 });
    let ops = sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "150", Some(1.0))],
        vec![],
        vec![],
    ));
    assert_eq!(
        ops,
        vec![Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 150.0
        }]
    );
    assert_eq!(sut.view().cached_total_usd, Some(150.0));
}

// ===========================================================================
// Machine — invariant ⑦: rate-limited chains self-heal quietly
// ===========================================================================

#[test]
fn rate_limited_chains_fall_back_quietly_without_banner() {
    let sut = booted(
        ADDR_A,
        Some(100.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "40", Some(1.0))],
            vec![196, 56],
            vec![196],
        ),
    );
    let view = sut.view();
    // The balance quietly stays on the max(live, cached) fallback …
    assert!(view.balance_partial);
    assert_eq!(view.display_total_usd, Some(100.0));
    // … and the "fix your RPC" banner excludes the self-healing chain.
    assert_eq!(view.banner_chain_ids, vec![56]);
    assert_eq!(view.failed_chain_ids, vec![196, 56]);
    assert_eq!(view.rate_limited_chain_ids, vec![196]);
}

/// The notice wording split (`HomeScreen.tsx:125`): failed chains → "still
/// updating"; only-unpriced → "couldn't be priced".
#[test]
fn notice_kind_follows_the_failure_shape() {
    // Exhaust the budget with an unpriced-only partial.
    let mut sut = booted(
        ADDR_A,
        None,
        settled(ADDR_A, vec![token(1, "MYSTERY", "5", None)], vec![], vec![]),
    );
    for round in 0..MAX_PARTIAL_RETRIES {
        sut.resolve(Res::RetryElapsed {
            timer_id: round + 1,
        });
        sut.resolve(settled(
            ADDR_A,
            vec![token(1, "MYSTERY", "5", None)],
            vec![],
            vec![],
        ));
    }
    assert_eq!(sut.view().notice, Some(BalanceNotice::Unpriced));
}

// ===========================================================================
// Machine — invariant ⑧: balance privacy (hand-the-phone-over model)
// ===========================================================================

#[test]
fn privacy_toggle_persists_masks_the_fiat_and_wins_the_hydrate_race() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "5", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    assert_eq!(sut.view().display_total_usd, Some(5.0));

    // The eye tap: persisted immediately, fiat withheld by construction.
    let ops = sut.dispatch(Event::PrivacyToggled);
    assert_eq!(ops, vec![Op::WritePrivacy { hidden: true }]);
    let view = sut.view();
    assert!(view.hidden);
    assert_eq!(view.display_total_usd, None, "fiat never leaves the core");

    // The async hydrate lands AFTER the tap → the user's tap wins.
    sut.dispatch(Event::PrivacyHydrated { hidden: false });
    assert!(sut.view().hidden, "hydrate must not overwrite the toggle");
}

#[test]
fn hidden_survives_restart_via_hydrate() {
    // A relaunch: the shell reads '1' from storage and feeds it first.
    let mut sut = boot(ADDR_A);
    sut.dispatch(Event::PrivacyHydrated { hidden: true });
    let view = sut.view();
    assert!(view.hidden, "hiding before handing the phone over sticks");
    // Toggling back works and persists '0'.
    let ops = sut.dispatch(Event::PrivacyToggled);
    assert_eq!(ops, vec![Op::WritePrivacy { hidden: false }]);
    assert!(!sut.view().hidden);
}

// ===========================================================================
// Machine — invariant ⑨: refresh cadence rules
// ===========================================================================

#[test]
fn manual_pull_forces_past_the_ttl_and_drives_the_spinner() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "5", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    // A user pull MUST re-hit RPC: force bypasses the shell's 5-min TTL.
    let ops = sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: true,
    });
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ADDR_A.to_owned(),
            force: true,
            pull: true
        }]
    );
    assert!(sut.view().refreshing);
    // The settle (echoing pull) releases the spinner.
    sut.resolve(Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: true,
        tokens: vec![token(1, "ETH", "5", Some(1.0))],
        failed_chain_ids: vec![],
        rate_limited_chain_ids: vec![],
        now_ms: NOW + 1.0,
    });
    assert!(!sut.view().refreshing);
    assert_eq!(sut.view().last_refreshed_at_ms, Some(NOW + 1.0));
}

#[test]
fn polls_never_run_backgrounded_but_focus_reloads() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "5", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    sut.dispatch(Event::AppBackgrounded);
    // A poller tick while backgrounded: dropped, no operation.
    let ops = sut.dispatch(Event::RefreshRequested {
        force: false,
        pull: false,
    });
    assert!(ops.is_empty(), "isAppActive gate");
    // Foregrounding reloads immediately (the focus effect).
    let ops = sut.dispatch(Event::AppFocused);
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ADDR_A.to_owned(),
            force: false,
            pull: false
        }]
    );
}

#[test]
fn refresh_before_any_account_is_a_no_op() {
    let mut sut = Sut::new();
    assert!(sut
        .dispatch(Event::RefreshRequested {
            force: true,
            pull: true
        })
        .is_empty());
    assert!(sut.dispatch(Event::AppFocused).is_empty());
}

// ===========================================================================
// Machine — fix flow
// ===========================================================================

#[test]
fn fix_resolved_removes_the_chain_and_reloads() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "5", Some(1.0))],
            vec![100, 56],
            vec![],
        ),
    );
    let ops = sut.dispatch(Event::FixChainResolved { chain_id: 100 });
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ADDR_A.to_owned(),
            force: false,
            pull: false
        }]
    );
    let view = sut.view();
    assert_eq!(
        view.failed_chain_ids,
        vec![56],
        "only the fixed chain leaves"
    );
}

// ===========================================================================
// Machine — invariant ⑩: the account switcher paints cache first
// ===========================================================================

/// Spec 038: the figure the hero shows after a complete settle is the figure
/// the switcher's row for the same account shows — not whatever that row's
/// own fetch found at another instant.
#[test]
fn a_complete_settle_is_the_switcher_s_figure_for_the_active_account() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "100", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    sut.resolve(Res::BalanceCacheWritten);
    assert!(sut.view().switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_A.to_owned(),
        usd: 100.0
    }));

    // Prices move; the next settle moves the hero — and the row with it.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled(
        ADDR_A,
        vec![token(1, "ETH", "100", Some(2.0))],
        vec![],
        vec![],
    ));
    let view = sut.view();
    assert_eq!(view.display_total_usd, Some(200.0));
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_A.to_owned(),
        usd: 200.0
    }));
    // A partial settle writes nothing — the row keeps the last complete figure.
}

#[test]
fn celo_s_wrapped_native_is_the_native_and_weth_is_not() {
    use vela_core::app::balance_dashboard::wrapped_native_is_the_native;
    assert!(wrapped_native_is_the_native(
        42220,
        "0x471EcE3750Da237f93B8E339c536989b8978a438"
    ));
    assert!(!wrapped_native_is_the_native(
        1,
        "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"
    ));
    assert!(!wrapped_native_is_the_native(
        1,
        "0x471ece3750da237f93b8e339c536989b8978a438"
    ));
}

#[test]
fn switcher_paints_cache_before_refreshing_every_account() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(1, "ETH", "100", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    // consume the complete-settle cache write ack
    sut.resolve(Res::BalanceCacheWritten);

    // Open: the hero's current total is poked into the cache (ported
    // verbatim), then the batch read runs. The modal is NOT open yet.
    let ops = sut.dispatch(Event::SwitcherOpened {
        addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()],
    });
    assert_eq!(
        ops,
        vec![
            Op::WriteBalanceCache {
                address: ADDR_A.to_owned(),
                usd: 100.0
            },
            Op::ReadBalanceCacheMany {
                addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()]
            },
        ]
    );
    assert!(!sut.view().switcher.open);
    sut.resolve(Res::BalanceCacheWritten);

    // The cache answers → the modal opens INSTANTLY with numbers on every
    // row (the active row pinned to the hero's value), and every account
    // refreshes in the background.
    let ops = sut.resolve(Res::CachedBalancesLoaded {
        balances: vec![BalanceCacheEntry {
            address: ADDR_B.to_owned(),
            usd: 55.0,
        }],
    });
    assert_eq!(
        ops,
        vec![
            Op::FetchAccountAssets {
                address: ADDR_A.to_owned()
            },
            Op::FetchAccountAssets {
                address: ADDR_B.to_owned()
            },
        ]
    );
    let view = sut.view();
    assert!(view.switcher.open);
    assert!(view.switcher.loading);
    assert_eq!(
        view.switcher.balances,
        vec![
            BalanceCacheEntry {
                address: ADDR_B.to_owned(),
                usd: 55.0
            },
            BalanceCacheEntry {
                address: ADDR_A.to_owned(),
                usd: 100.0
            },
        ]
    );

    // A's live refresh lands: row updates and the total is persisted.
    let ops = sut.resolve(Res::AccountAssetsFetched {
        address: ADDR_A.to_owned(),
        tokens: Some(vec![token(1, "ETH", "120", Some(1.0))]),
    });
    assert_eq!(
        ops,
        vec![Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 120.0
        }]
    );
    let view = sut.view();
    assert!(view.switcher.loading, "B still refreshing");
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_A.to_owned(),
        usd: 120.0
    }));

    // B's refresh fails: best effort — the row keeps its cached 55.
    sut.resolve(Res::BalanceCacheWritten);
    sut.resolve(Res::AccountAssetsFetched {
        address: ADDR_B.to_owned(),
        tokens: None,
    });
    let view = sut.view();
    assert!(!view.switcher.loading);
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_B.to_owned(),
        usd: 55.0
    }));

    // Closing is a plain dismiss.
    sut.dispatch(Event::SwitcherClosed);
    assert!(!sut.view().switcher.open);
}

/// The verbatim quirk pinned: opening the switcher while the balance is still
/// unknown pokes a $0 into the cache — exactly what `openSwitcher` ships.
#[test]
fn switcher_open_while_unknown_pokes_zero_verbatim() {
    let mut sut = boot(ADDR_A);
    let ops = sut.dispatch(Event::SwitcherOpened {
        addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()],
    });
    assert_eq!(
        ops[0],
        Op::WriteBalanceCache {
            address: ADDR_A.to_owned(),
            usd: 0.0
        },
        "displayTotal is 0 while unknown — ported verbatim"
    );
}

/// A stray batch-read answer with no open pending must not pop the modal.
#[test]
fn cached_balances_without_a_pending_open_are_ignored() {
    let mut sut = booted(ADDR_A, None, settled(ADDR_A, vec![], vec![], vec![]));
    sut.resolve(Res::BalanceCacheWritten);
    // Nothing outstanding matches, so drive it as a dropped-op echo: open a
    // switcher, then switch accounts — the late answer must be dropped.
    sut.dispatch(Event::SwitcherOpened {
        addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()],
    });
    sut.dispatch(Event::AccountChanged {
        address: ADDR_B.to_owned(),
    });
    // Resolve the stale WriteBalanceCache + ReadBalanceCacheMany answers.
    assert!(sut.resolve(Res::BalanceCacheWritten).is_empty());
    assert!(sut
        .resolve(Res::CachedBalancesLoaded {
            balances: vec![BalanceCacheEntry {
                address: ADDR_A.to_owned(),
                usd: 1.0
            }],
        })
        .is_empty());
    let view = sut.view();
    assert!(!view.switcher.open, "stale open must not pop on account B");
    // The stale answer's figure (1.0 for A) never lands; what A's row holds
    // is its own last complete settle (0.0), which is the settle's business.
    assert!(!view
        .switcher
        .balances
        .iter()
        .any(|entry| entry.address == ADDR_A && entry.usd == 1.0));
}

/// The peg table (spec 060). It exists so the rule is written ONCE: before
/// this, `symbol == "USD" ⇒ $1` lived in four shells in four languages, and a
/// second pegged symbol would have meant four chances to disagree about what a
/// coin is worth.
#[test]
fn pegged_native_usd_prices_dollar_native_coins() {
    // Tempo — unchanged behaviour, the reason the rule existed at all.
    assert_eq!(pegged_native_usd("USD"), Some(1.0));
    assert_eq!(pegged_native_usd("usd"), Some(1.0));
    // Arc — the native coin IS USDC.
    assert_eq!(pegged_native_usd("USDC"), Some(1.0));
    // Stable — the native coin IS USDT0.
    assert_eq!(pegged_native_usd("USDT0"), Some(1.0));
    assert_eq!(pegged_native_usd("usdc"), Some(1.0));
    assert_eq!(pegged_native_usd(" USDC "), Some(1.0));
}

/// A peg that answered for a volatile coin would be far worse than no peg:
/// every holding of it would read as a dollar.
#[test]
fn pegged_native_usd_refuses_everything_it_cannot_prove() {
    for symbol in [
        "ETH", "BNB", "POL", "AVAX", "MON", "XDAI", "WLD", "OKB", "MNT", "KAIA", "CELO", "PLUME",
        "XRP", "", "  ", "USDX", "EURC",
    ] {
        assert_eq!(
            pegged_native_usd(symbol),
            None,
            "{symbol} must not be pegged to a dollar"
        );
    }
}

/// The peg feeds the SAME ladder every other chain uses — it supplies a price,
/// it does not bypass the sanity band or change any rung's precedence.
#[test]
fn a_pegged_price_still_flows_through_the_normal_ladder() {
    let peg = pegged_native_usd("USDC").unwrap();
    // No DEX and no local feed on Arc: the peg is the answer.
    let picked = choose_native_price(None, None, Some(peg)).unwrap();
    assert_eq!(picked.price, 1.0);
}
