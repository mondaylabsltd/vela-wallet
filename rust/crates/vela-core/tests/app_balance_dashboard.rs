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
    BalanceNotice, BalanceOperation as Op, BalanceShellResult as Res, BalanceToken, BalanceView,
    Event, LastKnown, NativePriceSource, NativeQuoteGroup, FALLBACK_RETRY_DELAY_MS, LAST_SEEN,
    LAST_SEEN_EMPTY, LAST_SEEN_UNPRICED, MAX_PARTIAL_RETRIES, NOT_READ_YET,
    PARTIAL_RETRY_DELAYS_MS, UNREACHABLE_MANY, UNREACHABLE_ONE, UNREACHABLE_RECHECK_MS,
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
        read_chain_ids: vec![],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW,
    }
}

/// A settle from a shell that says which chains it asked (spec 092).
fn settled_read(tokens: Vec<BalanceToken>, failed: Vec<u32>, read: Vec<u32>) -> Res {
    Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens,
        failed_chain_ids: failed,
        rate_limited_chain_ids: vec![],
        read_chain_ids: read,
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
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
        internal: false,
    });
    let view = sut.view();
    assert!(view.unreachable);
    assert!(
        !view.balance_unknown,
        "the skeleton closed; the reason is different"
    );
    assert_eq!(view.tokens.len(), 0);
    assert_eq!(view.display_total_usd, None, "no figure, not a 0 to ignore");
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
        internal: false,
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
    // … and the unreachable list excludes the self-healing chain.
    let listed: Vec<u32> = view
        .unreachable_networks
        .iter()
        .map(|n| n.chain_id)
        .collect();
    assert_eq!(listed, vec![56]);
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

/// The switcher is money too: while hidden it carries no figure — not the
/// hero's total pinned at open (which the hero never showed), not a cached
/// row, not a live refresh — and the cached total is withheld like the hero's.
/// They are kept, and come back the moment privacy is turned off.
#[test]
fn hidden_withholds_the_switcher_and_the_cached_total() {
    let mut sut = booted(
        ADDR_A,
        Some(90.0),
        settled(
            ADDR_A,
            vec![token(1, "ETH", "100", Some(1.0))],
            vec![],
            vec![],
        ),
    );
    sut.resolve(Res::BalanceCacheWritten);
    sut.dispatch(Event::PrivacyToggled);
    let view = sut.view();
    assert_eq!(view.cached_total_usd, None, "a figure, withheld");
    assert_eq!(view.display_total_usd, None);

    sut.dispatch(Event::SwitcherOpened {
        addresses: vec![ADDR_A.to_owned(), ADDR_B.to_owned()],
    });
    sut.resolve(Res::BalanceCacheWritten);
    sut.resolve(Res::CachedBalancesLoaded {
        balances: vec![BalanceCacheEntry {
            address: ADDR_B.to_owned(),
            usd: 55.0,
        }],
    });
    sut.resolve(Res::AccountAssetsFetched {
        address: ADDR_B.to_owned(),
        tokens: Some(vec![token(1, "ETH", "56", Some(1.0))]),
    });
    let view = sut.view();
    assert!(view.switcher.open, "the switcher still opens, rows and all");
    assert!(view.switcher.hidden);
    assert!(
        view.switcher.balances.is_empty(),
        "no row, and no total, carries a figure: {:?}",
        view.switcher.balances
    );
    assert!(
        !serde_json::to_string(&view).unwrap().contains("56"),
        "no switcher figure leaves the core"
    );

    // Shown again: everything the switcher learned meanwhile is there.
    sut.dispatch(Event::PrivacyToggled);
    let view = sut.view();
    assert!(!view.switcher.hidden);
    assert_eq!(view.cached_total_usd, Some(100.0), "the settled total");
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_A.to_owned(),
        usd: 100.0
    }));
    assert!(view.switcher.balances.contains(&BalanceCacheEntry {
        address: ADDR_B.to_owned(),
        usd: 56.0
    }));
}

/// A shell that predates `switcher.hidden` still decodes the view.
#[test]
fn the_switcher_hidden_flag_defaults_on_the_wire() {
    let sut = booted(ADDR_A, None, settled(ADDR_A, vec![], vec![], vec![]));
    let mut json = serde_json::to_value(sut.view()).unwrap();
    json["switcher"].as_object_mut().unwrap().remove("hidden");
    let view: BalanceView = serde_json::from_value(json).unwrap();
    assert!(!view.switcher.hidden);
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
        read_chain_ids: vec![],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW + 1.0,
    });
    assert!(!sut.view().refreshing);
    assert_eq!(sut.view().last_refreshed_at_ms, Some(NOW + 1.0));
}

/// Issue #462: the hero's "↻ Updated <ago>" control sends exactly the pull
/// above, and reads "Updating…" while `refreshing` holds. It holds through a
/// poll's round that was already out (that round is not the one asked for),
/// and it ends on an error as on a settle — the label's time then stays at
/// the last round that did settle.
#[test]
fn the_refresh_control_spins_until_its_own_round_ends() {
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
    assert_eq!(sut.view().last_refreshed_at_ms, Some(NOW));
    // A poll's round is out when the person taps.
    sut.dispatch(Event::RefreshRequested {
        force: false,
        pull: false,
    });
    assert!(!sut.view().refreshing, "a poll never spins the control");
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: true,
    });
    assert!(sut.view().refreshing);
    // The poll's round lands first: still updating.
    sut.resolve(Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: vec![token(1, "ETH", "5", Some(1.0))],
        failed_chain_ids: vec![],
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW + 1.0,
    });
    assert!(sut.view().refreshing, "the person's own round is still out");
    assert_eq!(sut.view().last_refreshed_at_ms, Some(NOW + 1.0));
    // The person's round errors: the spin ends, and "Updated <ago>" keeps the
    // round that did land.
    sut.resolve(Res::FetchErrored {
        address: ADDR_A.to_owned(),
        pull: true,
        internal: false,
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
// Spec 092 — every network the wallet cannot reach, and what it last read
// ===========================================================================

/// `(chain, line key)` per row, in the order the list draws them.
fn unreachable_rows(view: &BalanceView) -> Vec<(u32, &str)> {
    view.unreachable_networks
        .iter()
        .map(|n| (n.chain_id, n.line_key.as_str()))
        .collect()
}

/// F08: one network is named, more are counted — and the count is every
/// unreachable network, held or not.
#[test]
fn the_home_line_names_one_network_and_counts_several() {
    let sut = booted(ADDR_A, None, settled_read(vec![], vec![56], vec![1, 56]));
    let view = sut.view();
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_ONE));
    assert_eq!(view.unreachable_networks.len(), 1);

    let sut = booted(
        ADDR_A,
        None,
        settled_read(vec![], vec![56, 137, 10], vec![1, 56, 137, 10]),
    );
    let view = sut.view();
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_MANY));
    assert_eq!(view.unreachable_networks.len(), 3);

    let sut = booted(ADDR_A, None, settled_read(vec![], vec![], vec![1, 56]));
    let view = sut.view();
    assert_eq!(view.unreachable_key, None, "every network answered");
    assert!(view.unreachable_networks.is_empty());
}

/// Each row says what was last read there: held (with its worth), held but
/// unpriced, empty, or not read since the account opened.
#[test]
fn each_unreachable_network_says_what_was_last_read_there() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(
            vec![
                token(56, "BNB", "2", Some(300.0)),
                token(137, "MYSTERY", "5", None),
            ],
            vec![],
            vec![1, 56, 137, 10],
        ),
    );
    // Next round: 56, 137 and 10 go quiet; 8453 was never read at all.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(
        vec![token(1, "ETH", "1", Some(1.0))],
        vec![56, 137, 10, 8453],
        vec![1, 56, 137, 10, 8453],
    ));
    let view = sut.view();
    assert_eq!(
        unreachable_rows(&view),
        vec![
            (56, LAST_SEEN),
            (137, LAST_SEEN_UNPRICED),
            (10, LAST_SEEN_EMPTY),
            (8453, NOT_READ_YET),
        ]
    );
    let bnb = &view.unreachable_networks[0];
    assert_eq!(bnb.last_known, LastKnown::Held);
    assert_eq!(bnb.last_seen_usd, Some(600.0));
    assert_eq!(view.unreachable_networks[1].last_known, LastKnown::Held);
    assert_eq!(view.unreachable_networks[1].last_seen_usd, None);
    assert_eq!(view.unreachable_networks[2].last_known, LastKnown::Empty);
    assert_eq!(view.unreachable_networks[3].last_known, LastKnown::NotRead);
}

/// Holdings first, by their worth; then the rest in the wallet's network
/// order (Ethereum, BNB Chain, Polygon, … then added networks by id) —
/// whatever order the shell reported them in.
#[test]
fn held_networks_come_first_then_the_rest_in_network_order() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(
            vec![
                token(10, "ETH", "1", Some(10.0)),
                token(8453, "ETH", "1", Some(50.0)),
            ],
            vec![],
            vec![1, 56, 10, 8453],
        ),
    );
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(
        vec![],
        vec![999_999, 10, 56, 8453, 1],
        vec![1, 56, 10, 8453, 999_999],
    ));
    let order: Vec<u32> = sut
        .view()
        .unreachable_networks
        .iter()
        .map(|n| n.chain_id)
        .collect();
    assert_eq!(order, vec![8453, 10, 1, 56, 999_999]);
}

/// A network that comes back leaves the list and the count at the next
/// settle; the one left is named again.
#[test]
fn a_network_that_comes_back_leaves_the_list_live() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(vec![], vec![56, 137], vec![1, 56, 137]),
    );
    assert_eq!(sut.view().unreachable_networks.len(), 2);
    assert_eq!(
        sut.view().unreachable_key.as_deref(),
        Some(UNREACHABLE_MANY)
    );

    // Opening the list reads every chain again, quietly.
    let ops = sut.dispatch(Event::UnreachableListOpened);
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ADDR_A.to_owned(),
            force: true,
            pull: false
        }]
    );
    assert!(!sut.view().refreshing, "no pull spinner for the list");
    sut.resolve(settled_read(
        vec![token(56, "BNB", "1", Some(300.0))],
        vec![137],
        vec![1, 56, 137],
    ));
    let view = sut.view();
    assert_eq!(unreachable_rows(&view), vec![(137, NOT_READ_YET)]);
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_ONE));

    // A fixed RPC takes its network off at once, before the re-read lands.
    sut.dispatch(Event::FixChainResolved { chain_id: 137 });
    let view = sut.view();
    assert!(view.unreachable_networks.is_empty());
    assert_eq!(view.unreachable_key, None);
}

/// While the list is open it keeps reading: the silent partial retries come
/// first, then a re-read [`UNREACHABLE_RECHECK_MS`] after each read, until the
/// list closes or nothing is left in it.
#[test]
fn an_open_list_keeps_reading_until_it_closes_or_empties() {
    let mut sut = booted(ADDR_A, None, settled_read(vec![], vec![56], vec![1, 56]));
    // Exhaust the silent retries first (they own the timer while they last).
    for _ in 0..MAX_PARTIAL_RETRIES {
        let timer = sut.outstanding().into_iter().find_map(|op| match op {
            Op::StartRetryTimer { timer_id, .. } => Some(timer_id),
            _ => None,
        });
        let Some(timer_id) = timer else {
            panic!("a partial retry is armed");
        };
        sut.resolve(Res::RetryElapsed { timer_id });
        sut.resolve(settled_read(vec![], vec![56], vec![1, 56]));
    }
    assert!(
        !sut.outstanding()
            .iter()
            .any(|op| matches!(op, Op::StartRetryTimer { .. })),
        "nothing re-reads while the list is closed"
    );

    sut.dispatch(Event::UnreachableListOpened);
    let ops = sut.resolve(settled_read(vec![], vec![56], vec![1, 56]));
    let timer_id = ops
        .iter()
        .find_map(|op| match op {
            Op::StartRetryTimer { ms, timer_id } if *ms == UNREACHABLE_RECHECK_MS => {
                Some(*timer_id)
            }
            _ => None,
        })
        .unwrap_or_else(|| panic!("the open list arms its re-read: {ops:?}"));
    let ops = sut.resolve(Res::RetryElapsed { timer_id });
    assert_eq!(
        ops,
        vec![Op::FetchTokens {
            address: ADDR_A.to_owned(),
            force: true,
            pull: false
        }]
    );

    // Closed: the next armed re-read is a late echo and reads nothing.
    let ops = sut.resolve(settled_read(vec![], vec![56], vec![1, 56]));
    let timer_id = ops
        .iter()
        .find_map(|op| match op {
            Op::StartRetryTimer { timer_id, .. } => Some(*timer_id),
            _ => None,
        })
        .unwrap_or_else(|| panic!("armed again: {ops:?}"));
    sut.dispatch(Event::UnreachableListClosed);
    assert!(sut.resolve(Res::RetryElapsed { timer_id }).is_empty());

    // Open again, and everything answers: an empty list stops reading.
    sut.dispatch(Event::UnreachableListOpened);
    let ops = sut.resolve(settled_read(vec![], vec![], vec![1, 56]));
    assert!(
        !ops.iter()
            .any(|op| matches!(op, Op::StartRetryTimer { .. })),
        "nothing left to wait for: {ops:?}"
    );
}

/// What a network last held survives the rounds it stays quiet, and a newer
/// answer replaces it — including an answer that it now holds nothing.
#[test]
fn the_last_read_is_kept_while_quiet_and_replaced_by_a_newer_answer() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(vec![token(56, "BNB", "1", Some(300.0))], vec![], vec![56]),
    );
    for _ in 0..2 {
        sut.dispatch(Event::RefreshRequested {
            force: true,
            pull: false,
        });
        sut.resolve(settled_read(vec![], vec![56], vec![56]));
        let view = sut.view();
        assert_eq!(unreachable_rows(&view), vec![(56, LAST_SEEN)]);
        assert_eq!(view.unreachable_networks[0].last_seen_usd, Some(300.0));
    }
    // It answers holding nothing (spent elsewhere), then goes quiet again.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(vec![], vec![], vec![56]));
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(vec![], vec![56], vec![56]));
    assert_eq!(unreachable_rows(&sut.view()), vec![(56, LAST_SEEN_EMPTY)]);
}

/// The web and the desktop carry a quiet chain's previous rows into the
/// settle; they stand for the last read when this account has none of its
/// own — a shell that does not say which chains it asked still gets "held".
#[test]
fn carried_rows_stand_in_for_a_read_the_core_never_saw() {
    let sut = booted(
        ADDR_A,
        None,
        settled(
            ADDR_A,
            vec![token(56, "BNB", "1", Some(300.0))],
            vec![56],
            vec![],
        ),
    );
    let view = sut.view();
    assert_eq!(unreachable_rows(&view), vec![(56, LAST_SEEN)]);
    assert_eq!(view.unreachable_networks[0].last_seen_usd, Some(300.0));
}

/// Privacy withholds the worth, never the row: the network is still listed,
/// its line still "last seen", and the shell writes its mask.
#[test]
fn privacy_withholds_the_worth_not_the_network() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(vec![token(56, "BNB", "1", Some(300.0))], vec![], vec![56]),
    );
    sut.dispatch(Event::PrivacyToggled);
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(vec![], vec![56], vec![56]));
    let view = sut.view();
    assert_eq!(unreachable_rows(&view), vec![(56, LAST_SEEN)]);
    assert_eq!(view.unreachable_networks[0].last_seen_usd, None);
}

/// Spam is not a holding: a network whose last read listed only spam was
/// empty.
#[test]
fn spam_alone_is_not_a_holding() {
    let mut spam = token(56, "AIRDROP", "9999", Some(1.0));
    spam.spam = true;
    let mut sut = booted(ADDR_A, None, settled_read(vec![spam], vec![], vec![56]));
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settled_read(vec![], vec![56], vec![56]));
    assert_eq!(unreachable_rows(&sut.view()), vec![(56, LAST_SEEN_EMPTY)]);
}

/// Another account's reads are not this one's: a switch forgets them.
#[test]
fn an_account_switch_forgets_the_last_reads() {
    let mut sut = booted(
        ADDR_A,
        None,
        settled_read(vec![token(56, "BNB", "1", Some(300.0))], vec![], vec![56]),
    );
    sut.dispatch(Event::AccountChanged {
        address: ADDR_B.to_owned(),
    });
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_B.to_owned(),
        usd: None,
    });
    sut.resolve(Res::FetchSettled {
        address: ADDR_B.to_owned(),
        pull: false,
        tokens: vec![],
        failed_chain_ids: vec![56],
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![56],
        internal_chain_ids: vec![],
        registry_chain_ids: vec![],
        now_ms: NOW,
    });
    assert_eq!(unreachable_rows(&sut.view()), vec![(56, NOT_READ_YET)]);
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

/// One coin, one spelling: a built-in chain's own coin takes the registry's
/// symbol whatever the chain document said ("XDAI" → "xDAI"); a contract
/// token, and a native row whose symbol is another word, are left as read.
#[test]
fn a_chains_own_coin_is_written_the_wallets_way() {
    const ADDR: &str = "0xabc";
    let mut usdc = token(100, "USDC", "5", Some(1.0));
    usdc.token_address = Some("0xddafbb505ad214d7b80b1f830fccc89b60fb7a83".to_owned());
    let sut = booted(
        ADDR,
        None,
        settled(
            ADDR,
            vec![
                token(100, "XDAI", "0.5", Some(1.0)),
                usdc,
                token(999_999, "ABC", "1", Some(1.0)),
            ],
            vec![],
            vec![],
        ),
    );
    let symbols: Vec<String> = sut.view().tokens.iter().map(|t| t.symbol.clone()).collect();
    assert!(symbols.contains(&"xDAI".to_owned()), "{symbols:?}");
    assert!(symbols.contains(&"USDC".to_owned()), "{symbols:?}");
    assert!(symbols.contains(&"ABC".to_owned()), "{symbols:?}");
    assert!(!symbols.contains(&"XDAI".to_owned()), "{symbols:?}");
}

// ---------------------------------------------------------------------------
// Read plan — which tokens one chain's balance read covers (spec 082 T036,
// RE9, G24)
// ---------------------------------------------------------------------------

mod read_plan {
    use vela_core::app::balance_dashboard::{
        chain_has_native_coin, read_plan, ReadKind, ReadSlot, StableRef, TokenRef,
    };

    const BASE: u32 = 8453;
    const TEMPO: u32 = 4217;
    const CELO: u32 = 42220;
    const BASE_USDC: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
    const BASE_WETH: &str = "0x4200000000000000000000000000000000000006";
    const CELO_GOLD: &str = "0x471EcE3750Da237f93B8E339c536989b8978a438";

    fn stable(symbol: &str, contract: &str) -> StableRef {
        StableRef {
            symbol: symbol.to_owned(),
            contract: contract.to_owned(),
        }
    }

    fn custom(symbol: &str, name: &str, contract: &str, decimals: u8) -> TokenRef {
        TokenRef {
            contract: contract.to_owned(),
            symbol: symbol.to_owned(),
            name: name.to_owned(),
            decimals,
        }
    }

    /// Base's `stables` as ethereum-data.getvela.app served them on
    /// 2026-09-28 (`chains/eip155-8453.json`), in its order.
    fn base_stables() -> Vec<StableRef> {
        vec![
            stable("USDC", BASE_USDC),
            stable("USDT", "0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
            stable("DAI", "0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
            stable("USDbC", "0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),
            stable("EURC", "0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42"),
        ]
    }

    fn shape(plan: &[ReadSlot]) -> Vec<(ReadKind, Option<&str>, &str)> {
        plan.iter()
            .map(|slot| (slot.kind, slot.contract.as_deref(), slot.symbol.as_str()))
            .collect()
    }

    /// G24: the same wallet listed USDC on Base (0.470005) on the desktop and
    /// not on the iPhone, which read only the native coin and custom tokens.
    #[test]
    fn base_lists_usdc() {
        let plan = read_plan(BASE, &base_stables(), Some(BASE_WETH), &[]);
        assert_eq!(
            shape(&plan),
            vec![
                (ReadKind::Native, None, ""),
                (ReadKind::Stable, Some(BASE_USDC), "USDC"),
                (
                    ReadKind::Stable,
                    Some("0xfde4C96c8593536E31F229EA8f37b2ADa2699bb2"),
                    "USDT"
                ),
                (
                    ReadKind::Stable,
                    Some("0x50c5725949A6F0c72E6C4a641F24049A917DB0Cb"),
                    "DAI"
                ),
                (
                    ReadKind::Stable,
                    Some("0xd9aAEc86B65D86f6A7B5B1b0c42FFA531710b6CA"),
                    "USDbC"
                ),
                (
                    ReadKind::Stable,
                    Some("0x60a3E35Cc302bFA44Cb288Bc5a4F316Fdb1adb42"),
                    "EURC"
                ),
                (ReadKind::Wrapped, Some(BASE_WETH), ""),
            ]
        );
        let usdc = &plan[1];
        assert_eq!(usdc.peg_usd, Some(1.0));
        assert_eq!(usdc.known_decimals, None, "decimals are read on chain");
        assert_eq!(usdc.name, "USDC");
        assert!(plan[0].peg_usd.is_none() && plan[6].peg_usd.is_none());
    }

    /// Tempo has no native coin: its gas is a stablecoin, and its
    /// `eth_getBalance` answers a constant for every address.
    #[test]
    fn tempo_has_no_native_slot() {
        assert!(!chain_has_native_coin(TEMPO));
        assert!(!chain_has_native_coin(42431));
        assert!(chain_has_native_coin(BASE));
        let plan = read_plan(
            TEMPO,
            &[
                stable("USDC.e", "0x20C000000000000000000000b9537d11c60E8b50"),
                stable("pathUSD", "0x20C0000000000000000000000000000000000000"),
            ],
            None,
            &[],
        );
        assert!(plan.iter().all(|slot| slot.kind == ReadKind::Stable));
        assert_eq!(plan.len(), 2);
    }

    /// A contract is read once. The person's own entry for a registry
    /// stablecoin lends it their symbol, name and saved decimals; it stays in
    /// the registry's place, priced at its peg.
    #[test]
    fn a_custom_usdc_entry_wins_over_the_registry_one() {
        let mine = custom("USDC", "USD Coin (mine)", &BASE_USDC.to_lowercase(), 6);
        let plan = read_plan(BASE, &base_stables(), Some(BASE_WETH), &[mine]);
        assert_eq!(plan.len(), 7, "no second USDC slot");
        let usdc = &plan[1];
        assert_eq!(usdc.kind, ReadKind::Stable);
        assert_eq!(usdc.name, "USD Coin (mine)");
        assert_eq!(usdc.known_decimals, Some(6));
        assert_eq!(usdc.peg_usd, Some(1.0));
        assert!(plan.iter().all(|slot| slot.kind != ReadKind::Custom));
    }

    /// Native, stables, wrapped, custom — each group in the order given, and
    /// the same answer every time.
    #[test]
    fn order_is_stable() {
        let degen = custom(
            "DEGEN",
            "Degen",
            "0x4ed4E862860beD51a9570b96d89aF5E1B0Efefed",
            18,
        );
        let brett = custom(
            "BRETT",
            "Brett",
            "0x532f27101965dd16442E59d40670FaF5eBB142E4",
            18,
        );
        let tokens = [degen, brett];
        let plan = read_plan(BASE, &base_stables(), Some(BASE_WETH), &tokens);
        let kinds: Vec<ReadKind> = plan.iter().map(|slot| slot.kind).collect();
        assert_eq!(
            kinds,
            vec![
                ReadKind::Native,
                ReadKind::Stable,
                ReadKind::Stable,
                ReadKind::Stable,
                ReadKind::Stable,
                ReadKind::Stable,
                ReadKind::Wrapped,
                ReadKind::Custom,
                ReadKind::Custom,
            ]
        );
        assert_eq!(plan[7].symbol, "DEGEN");
        assert_eq!(plan[8].symbol, "BRETT");
        assert_eq!(plan[8].known_decimals, Some(18));
        assert_eq!(
            plan,
            read_plan(BASE, &base_stables(), Some(BASE_WETH), &tokens)
        );
    }

    /// Celo's "wrapped" native IS the coin (spec 038): no wrapped slot, and a
    /// custom entry for it is not a second copy of the holding.
    #[test]
    fn celo_s_coin_is_read_once() {
        let plan = read_plan(
            CELO,
            &[stable("USDC", "0xcebA9300f2b948710d2653dD7B07f33A8B32118C")],
            Some(CELO_GOLD),
            &[custom("CELO", "Celo", CELO_GOLD, 18)],
        );
        let kinds: Vec<ReadKind> = plan.iter().map(|slot| slot.kind).collect();
        assert_eq!(kinds, vec![ReadKind::Native, ReadKind::Stable]);
    }

    /// Case never makes two slots; the first entry of a contract keeps its
    /// place; blank contracts are not read.
    #[test]
    fn a_contract_is_read_once_whatever_its_case() {
        let gnosis_wxdai = "0xe91D153E0b41518A2Ce8Dd3D7944Fa863463a97d";
        let plan = read_plan(
            100,
            &[
                stable("WXDAI", gnosis_wxdai),
                stable("USDC", "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"),
                stable("USDC", "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"),
                stable("?", "  "),
            ],
            Some(&gnosis_wxdai.to_lowercase()),
            &[
                custom(
                    "GNO",
                    "Gnosis",
                    "0x9C58BAcC331c9aa871AFD802DB6379a98e80CEdb",
                    18,
                ),
                custom(
                    "gno",
                    "gno again",
                    "0x9c58bacc331c9aa871afd802db6379a98e80cedb",
                    18,
                ),
                custom("NONE", "blank", "", 18),
            ],
        );
        assert_eq!(
            shape(&plan),
            vec![
                (ReadKind::Native, None, ""),
                (ReadKind::Stable, Some(gnosis_wxdai), "WXDAI"),
                (
                    ReadKind::Stable,
                    Some("0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83"),
                    "USDC"
                ),
                (
                    ReadKind::Custom,
                    Some("0x9C58BAcC331c9aa871AFD802DB6379a98e80CEdb"),
                    "GNO"
                ),
            ]
        );
    }

    /// The shells can hand the chain data's own `stables` JSON over as it is.
    #[test]
    fn the_chain_data_stables_decode_as_they_are() {
        let json = r#"[{"symbol":"USDC","type":"native","contract":"0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"}]"#;
        let stables: Vec<StableRef> = serde_json::from_str(json).unwrap_or_default();
        assert_eq!(stables, vec![stable("USDC", BASE_USDC)]);
        let custom: Vec<TokenRef> =
            serde_json::from_str(r#"[{"contract":"0xabc","symbol":"X","decimals":6}]"#)
                .unwrap_or_default();
        assert_eq!(custom, vec![self::custom("X", "", "0xabc", 6)]);
        let slot =
            serde_json::to_value(&read_plan(TEMPO, &stables, None, &[])[0]).unwrap_or_default();
        assert_eq!(slot["kind"], "stable");
        assert_eq!(slot["peg_usd"], 1.0);
    }
}

// ===========================================================================
// PR 2 note 11: a fault inside Vela is never "can't reach" a network
// ===========================================================================

/// The balance read never left the app on a chain (issue 483: a request pool
/// nobody started): the home says Vela's own fault, in the fee's sentence
/// for it, and that chain is not counted as a network out of reach — while
/// a real chain-down beside it still is. The total stays honest either way.
#[test]
fn an_internal_fault_is_not_a_network_out_of_reach() {
    use vela_core::app::fee_policy::REASON_INTERNAL_KEY;
    let settle = |failed: Vec<u32>, internal: Vec<u32>| Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: vec![],
        failed_chain_ids: failed,
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![1, 56, 137],
        internal_chain_ids: internal,
        registry_chain_ids: vec![],
        now_ms: NOW,
    };
    // Every failure is internal: no "Can't reach Ethereum".
    let sut = booted(ADDR_A, None, settle(vec![1], vec![1]));
    let view = sut.view();
    assert_eq!(view.unreachable_key, None);
    assert!(view.unreachable_networks.is_empty());
    assert_eq!(view.internal_key.as_deref(), Some(REASON_INTERNAL_KEY));
    assert_eq!(view.internal_chain_ids, vec![1]);
    assert!(view.balance_partial, "a chain did not answer");

    // One internal, one really down: each said as what it is.
    let sut = booted(ADDR_A, None, settle(vec![1, 56], vec![1]));
    let view = sut.view();
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_ONE));
    assert_eq!(unreachable_rows(&view).first().map(|row| row.0), Some(56));
    assert_eq!(view.internal_key.as_deref(), Some(REASON_INTERNAL_KEY));

    // The next round answers: the line goes.
    let mut sut = booted(ADDR_A, None, settle(vec![1], vec![1]));
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settle(vec![], vec![]));
    assert_eq!(sut.view().internal_key, None);
}

// ===========================================================================
// PR 3 note 4: a token list that cannot be loaded is not a network out of reach
// ===========================================================================

/// Tempo has no native coin: its money is the stablecoins its token list
/// names, so with that list unloaded nothing there could be read — and the
/// chain is a failed chain. But its RPC was never asked and is fine: the
/// home says the token list, not "Can't reach Tempo", and the row offers no
/// RPC fix. A network that really did not answer, beside it, still does.
#[test]
fn an_unloaded_token_list_is_not_a_network_out_of_reach() {
    use vela_core::app::balance_dashboard::{UnreachableCause, TOKEN_LIST_UNREACHABLE};
    const TEMPO: u32 = 4_217;
    let settle = |failed: Vec<u32>, registry: Vec<u32>| Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: vec![token(1, "ETH", "1", Some(2_000.0))],
        failed_chain_ids: failed,
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![1, 56, TEMPO],
        internal_chain_ids: vec![],
        registry_chain_ids: registry,
        now_ms: NOW,
    };

    // Alone: the line names the token list, and nothing offers an RPC fix.
    let view = booted(ADDR_A, None, settle(vec![TEMPO], vec![TEMPO])).view();
    assert_eq!(
        view.unreachable_key.as_deref(),
        Some(TOKEN_LIST_UNREACHABLE)
    );
    let [row] = view.unreachable_networks.as_slice() else {
        unreachable!("one row: {:?}", view.unreachable_networks);
    };
    assert_eq!(row.chain_id, TEMPO);
    assert_eq!(row.cause, UnreachableCause::TokenList);
    assert!(!row.rpc_fixable, "its RPC is fine: nothing to fix");
    assert!(view.balance_partial, "still a chain that was not read");

    // Beside a network that is really down: counted, and only the one whose
    // endpoints failed is offered the fix.
    let view = booted(ADDR_A, None, settle(vec![56, TEMPO], vec![TEMPO])).view();
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_MANY));
    let fixable: Vec<(u32, bool)> = view
        .unreachable_networks
        .iter()
        .map(|row| (row.chain_id, row.rpc_fixable))
        .collect();
    assert!(fixable.contains(&(56, true)), "{fixable:?}");
    assert!(fixable.contains(&(TEMPO, false)), "{fixable:?}");

    // A chain that answered cannot have failed for this reason.
    let view = booted(ADDR_A, None, settle(vec![56], vec![TEMPO])).view();
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_ONE));
    assert!(view.unreachable_networks.iter().all(|row| row.rpc_fixable));

    // The list is back: the line goes, and the next account starts clean.
    let mut sut = booted(ADDR_A, None, settle(vec![TEMPO], vec![TEMPO]));
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(settle(vec![], vec![]));
    assert_eq!(sut.view().unreachable_key, None);
}

/// A row written before the cause was said is the network's, and fixable —
/// what every row was.
#[test]
fn an_unreachable_row_from_before_the_cause_reads_as_the_networks() {
    use vela_core::app::balance_dashboard::{UnreachableCause, UnreachableNetwork};
    let old =
        r#"{"chain_id":56,"last_known":"held","last_seen_usd":157.0,"line_key":"assets.lastSeen"}"#;
    let row: UnreachableNetwork = serde_json::from_str(old).unwrap();
    assert_eq!(row.cause, UnreachableCause::Network);
    assert!(row.rpc_fixable);
    let said = serde_json::to_value(&row).unwrap_or_default();
    assert_eq!(said["cause"], "network");
    assert_eq!(said["rpc_fixable"], true);
}

/// The whole fetch threw inside the app: said as that too, never "can't
/// reach".
#[test]
fn a_fetch_that_threw_inside_the_app_says_so() {
    use vela_core::app::fee_policy::REASON_INTERNAL_KEY;
    let sut = booted(
        ADDR_A,
        None,
        Res::FetchErrored {
            address: ADDR_A.to_owned(),
            pull: false,
            internal: true,
        },
    );
    assert_eq!(
        sut.view().internal_key.as_deref(),
        Some(REASON_INTERNAL_KEY)
    );
    let sut = booted(
        ADDR_A,
        None,
        Res::FetchErrored {
            address: ADDR_A.to_owned(),
            pull: false,
            internal: false,
        },
    );
    assert_eq!(sut.view().internal_key, None);
}

/// Every chain a round asked failed and nothing is known: no settled $0.00
/// (and no "Deposit your first asset" under it) — nothing at all, as for a
/// fetch that threw. A round where one chain answered, even with nothing,
/// is a real zero.
#[test]
fn a_round_where_every_chain_failed_is_no_zero() {
    let all_failed = Res::FetchSettled {
        address: ADDR_A.to_owned(),
        pull: false,
        tokens: vec![],
        failed_chain_ids: vec![1, 56],
        rate_limited_chain_ids: vec![],
        read_chain_ids: vec![1, 56],
        internal_chain_ids: vec![1, 56],
        registry_chain_ids: vec![],
        now_ms: NOW,
    };
    let view = booted(ADDR_A, None, all_failed).view();
    assert!(view.unreachable, "nothing known: no figure");
    assert_eq!(view.display_total_usd, None, "unreachable is no $0.00");
    assert!(view.internal_key.is_some());

    let one_answered = booted(ADDR_A, None, settled_read(vec![], vec![56], vec![1, 56])).view();
    assert!(
        !one_answered.unreachable,
        "Ethereum answered: it holds nothing"
    );
    assert_eq!(one_answered.display_total_usd, Some(0.0));

    // With a cache, the cache stands.
    let cached = booted(
        ADDR_A,
        Some(12.0),
        settled_read(vec![], vec![1, 56], vec![1, 56]),
    )
    .view();
    assert!(!cached.unreachable);
}

// ===========================================================================
// PR 3 final note F19: "live" is a zero every chain answered for — never a
// cached zero nothing has read yet
// ===========================================================================

/// A wallet that held nothing last session opens with a cached total of 0.
/// Before its first round ends nothing has been read: the line under the
/// total says "Checking…", not "Live · listening for payments" — which the
/// shells drew there, then swapped for "Can't reach 24 networks" when the
/// round came back. Live is said only by a round that settled with every
/// chain answering.
#[test]
fn a_cached_zero_is_checking_until_a_round_says_it_is_live() {
    use vela_core::app::balance_dashboard::{CHECKING, LIVE_ZERO};
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(0.0),
    });
    let view = sut.view();
    assert_eq!(
        view.display_total_usd,
        Some(0.0),
        "the cached zero is shown"
    );
    assert_eq!(view.checking_key.as_deref(), Some(CHECKING));
    assert_eq!(view.live_key, None, "nothing has read this wallet yet");

    // Every chain answered, holding nothing: now it is live.
    sut.resolve(settled_read(vec![], vec![], vec![1, 56, 100]));
    let view = sut.view();
    assert_eq!(view.checking_key, None);
    assert_eq!(view.live_key.as_deref(), Some(LIVE_ZERO));

    // A later refresh is not "checking" again: what the last round found
    // stands while the next one is out.
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    let view = sut.view();
    assert_eq!(view.checking_key, None);
    assert_eq!(view.live_key.as_deref(), Some(LIVE_ZERO));

    // That round misses a chain: a zero with a network out of reach is an
    // unknown wallet, not a listening one.
    sut.resolve(settled_read(vec![], vec![56], vec![1, 56, 100]));
    let view = sut.view();
    assert_eq!(view.live_key, None);
    assert!(view.unreachable_key.is_some());
}

/// The first round came back with networks out of reach: "Checking…" gives
/// way to "Can't reach", and "live" was never said in between.
#[test]
fn checking_gives_way_to_what_the_first_round_found() {
    use vela_core::app::balance_dashboard::CHECKING;
    let mut sut = boot(ADDR_A);
    sut.resolve(Res::CachedTotalLoaded {
        address: ADDR_A.to_owned(),
        usd: Some(0.0),
    });
    assert_eq!(sut.view().checking_key.as_deref(), Some(CHECKING));
    sut.resolve(settled_read(vec![], vec![1, 56], vec![1, 56, 100]));
    let view = sut.view();
    assert_eq!(view.checking_key, None);
    assert_eq!(view.live_key, None);
    assert_eq!(view.unreachable_key.as_deref(), Some(UNREACHABLE_MANY));
}

/// A read that threw said nothing: over a cached zero it is neither
/// "checking" (it ended) nor "live" (no chain answered) — and a zero that
/// WAS live stops being so when the next read throws.
#[test]
fn a_read_that_threw_is_not_live() {
    let errored = || Res::FetchErrored {
        address: ADDR_A.to_owned(),
        pull: false,
        internal: false,
    };
    let sut = booted(ADDR_A, Some(0.0), errored());
    let view = sut.view();
    assert_eq!((view.checking_key, view.live_key), (None, None));

    let mut sut = booted(
        ADDR_A,
        Some(0.0),
        settled_read(vec![], vec![], vec![1, 56, 100]),
    );
    assert!(sut.view().live_key.is_some());
    sut.dispatch(Event::RefreshRequested {
        force: true,
        pull: false,
    });
    sut.resolve(errored());
    assert_eq!(sut.view().live_key, None);
}

/// Live is a ZERO: a wallet holding something is never "listening", hidden
/// it says nothing about itself, and another account starts over.
#[test]
fn live_is_only_a_shown_settled_zero() {
    let holding = booted(
        ADDR_A,
        None,
        settled_read(
            vec![token(1, "ETH", "1", Some(2_000.0))],
            vec![],
            vec![1, 56],
        ),
    );
    assert_eq!(holding.view().live_key, None);

    let mut sut = booted(ADDR_A, None, settled_read(vec![], vec![], vec![1, 56]));
    assert!(sut.view().live_key.is_some());
    sut.dispatch(Event::PrivacyToggled);
    assert_eq!(
        sut.view().live_key,
        None,
        "hidden: no line about the figure"
    );
    sut.dispatch(Event::PrivacyToggled);

    sut.dispatch(Event::AccountChanged {
        address: ADDR_B.to_owned(),
    });
    let view = sut.view();
    assert_eq!(view.live_key, None);
    assert!(view.checking_key.is_some(), "the new account is being read");
}

/// A view written before these facts reads with neither line.
#[test]
fn a_view_from_before_the_checking_fact_still_reads() {
    let sut = booted(ADDR_A, None, settled_read(vec![], vec![], vec![1]));
    let mut json = serde_json::to_value(sut.view()).unwrap_or_default();
    if let Some(map) = json.as_object_mut() {
        map.remove("checking_key");
        map.remove("live_key");
    }
    let old: BalanceView = serde_json::from_value(json).expect("an older view still reads");
    assert_eq!((old.checking_key, old.live_key), (None, None));
}

/// PR 3 final note F21: the balance breakdown's short status says what is
/// unavailable — the RPC, or the token list — from the core's key.
#[test]
fn the_breakdown_status_names_what_is_unavailable() {
    use vela_core::app::balance_dashboard::{
        STATUS_RPC_UNAVAILABLE, STATUS_TOKEN_LIST_UNAVAILABLE,
    };
    const TEMPO: u32 = 4_217;
    let view = booted(
        ADDR_A,
        None,
        Res::FetchSettled {
            address: ADDR_A.to_owned(),
            pull: false,
            tokens: vec![token(1, "ETH", "1", Some(2_000.0))],
            failed_chain_ids: vec![56, TEMPO],
            rate_limited_chain_ids: vec![],
            read_chain_ids: vec![1, 56, TEMPO],
            internal_chain_ids: vec![],
            registry_chain_ids: vec![TEMPO],
            now_ms: NOW,
        },
    )
    .view();
    let status = |chain: u32| {
        view.unreachable_networks
            .iter()
            .find(|row| row.chain_id == chain)
            .map(|row| row.status_key.clone())
    };
    assert_eq!(status(56).as_deref(), Some(STATUS_RPC_UNAVAILABLE));
    assert_eq!(
        status(TEMPO).as_deref(),
        Some(STATUS_TOKEN_LIST_UNAVAILABLE)
    );
    let i18n = vela_core::i18n::I18n::embedded().expect("embedded corpus");
    let opts = vela_core::i18n::Options::default();
    for key in [STATUS_RPC_UNAVAILABLE, STATUS_TOKEN_LIST_UNAVAILABLE] {
        assert!(i18n.exists(key, &opts), "{key}");
    }
}
