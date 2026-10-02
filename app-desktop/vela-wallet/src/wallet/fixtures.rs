//! Canonical wallet fixtures — the desktop port of
//! `specs/015-wallet-home-ui/data-model.md`. Content is verbatim from the
//! mocks (spec FR-012); chain colors are fixture data, not theme tokens.

use gpui::{Hsla, SharedString, rgb};

use super::{WalletStrings, fill};

pub const WALLET_NAME: &str = "大表哥";
pub const ADDRESS_FULL: &str = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c";
pub const NETWORK_COUNT: u32 = 8;

/// Identicon-board seeds (US3): the cross-platform eyeball parity set.
pub const IDENTICON_BOARD_SEEDS: [&str; 6] = [
    ADDRESS_FULL,
    "0xd8da6bf26964af9d7eed9e03e53415d37aa96045",
    "alice",
    "bob",
    "0x9F3c00000000000000000000000000000000021aE",
    "",
];

pub fn chain_bnb() -> Hsla {
    rgb(0xf0b90b).into()
}
pub fn chain_ethereum() -> Hsla {
    rgb(0x627eea).into()
}
pub fn chain_arbitrum() -> Hsla {
    rgb(0x28a0f0).into()
}
pub fn chain_gnosis() -> Hsla {
    rgb(0x21bca5).into()
}
pub fn chain_base() -> Hsla {
    rgb(0x0052ff).into()
}
pub fn chain_polygon() -> Hsla {
    rgb(0x8247e5).into()
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ActivityKind {
    Sent,
    Received,
    Dapp,
}

#[derive(Clone)]
pub struct ActivityRowModel {
    pub kind: ActivityKind,
    pub title: SharedString,
    pub subtitle: SharedString,
    pub amount: SharedString,
    pub unit: SharedString,
    pub positive: bool,
    pub badge: Hsla,
    /// The chain's logo for the avatar's badge (§8.3; issue 201). `None` on a
    /// drawn row — the coloured dot is the fallback, not the intent.
    pub badge_logo: Option<SharedString>,
    /// The day this row opens, when it is the first of its day — "Today",
    /// "Yesterday", a date — drawn above it on the home (078 H-04). `None`
    /// for every other row, and everywhere else the row is drawn.
    pub day: Option<SharedString>,
    /// A dApp swap's one coin back, under its figure — "≈ +0.000037 ETH"
    /// (083 F1): what the wallet's simulation expected when the person
    /// approved, never a promise. `None` on every other row.
    pub received: Option<SharedString>,
    /// The figure is an unlimited allowance a dApp was granted (spec 093):
    /// drawn in the danger tone.
    pub danger: bool,
}

#[derive(Clone)]
pub enum Fiat {
    Value(SharedString),
    NoPrice(SharedString),
    Masked,
}

#[derive(Clone)]
pub struct AssetRowModel {
    pub ticker: SharedString,
    pub chain: SharedString,
    pub badge: Hsla,
    pub balance: SharedString,
    pub fiat: Fiat,
    /// The endpoint's logos for this holding (issue 201).
    pub logos: crate::marks::Logos,
}

#[derive(Clone)]
pub struct ChainRowModel {
    pub name: SharedString,
    /// `None` = the neutral all-networks dot.
    pub dot: Option<Hsla>,
    pub count: u32,
    pub selected: bool,
    /// Which chain this row IS — `None` on the all-networks row, which is the
    /// same `None` the filter itself uses. Carried on the row rather than
    /// derived from its position, because a click has to name a chain and a
    /// position is only a chain until the list re-sorts.
    pub chain_id: Option<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BalanceState {
    Normal,
    ZeroLive,
    Loading,
    Hidden,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StatusKind {
    Warning,
    Refreshing,
}

#[derive(Clone)]
pub struct BalanceModel {
    pub label: SharedString,
    /// The code the figure is actually drawn in — `总余额 · ZAR`.
    ///
    /// Carried rather than assumed: the hero used to write `· USD` verbatim, so
    /// a wallet converted to ZAR said `总余额 · USD` over `ZAR 157.34`. It is
    /// [`crate::wallet::live::Money`]'s EFFECTIVE code, which is USD whenever
    /// the rate is missing — the label and the figure then agree, because the
    /// figure is dollars too.
    pub currency: SharedString,
    pub state: BalanceState,
    pub integer: SharedString,
    pub decimals: Option<SharedString>,
    pub live: Option<SharedString>,
    pub status: Option<(StatusKind, SharedString)>,
}

pub const MASK: &str = "••••";
pub const BALANCE_MASK: &str = "••••••";

/// The default balance every D-state shows.
pub fn balance_default(s: &WalletStrings) -> BalanceModel {
    BalanceModel {
        label: s.total_balance.clone(),
        currency: "USD".into(),
        state: BalanceState::Normal,
        integer: "$1,383".into(),
        decimals: Some("28".into()),
        live: None,
        status: None,
    }
}

/// Spec 092's gallery state (DSR6): the core's view with three networks out
/// of reach — one last seen holding $4,500, one last seen empty, one never
/// read — drawn through the same live builders as a session.
#[must_use]
pub fn unreachable_view() -> vela_core::app::balance_dashboard::BalanceView {
    use vela_core::app::balance_dashboard::{
        BalanceSwitcherView, BalanceView, LAST_SEEN, LAST_SEEN_EMPTY, LastKnown, NOT_READ_YET,
        UNREACHABLE_MANY, UnreachableNetwork,
    };
    let row =
        |chain_id: u32, last_known: LastKnown, usd: Option<f64>, key: &str| UnreachableNetwork {
            chain_id,
            last_known,
            last_seen_usd: usd,
            line_key: key.to_owned(),
        };
    BalanceView {
        address: Some(ADDRESS_FULL.to_owned()),
        display_total_usd: Some(4_500.0),
        balance_unknown: false,
        balance_partial: true,
        unreachable: false,
        notice: None,
        hidden: false,
        refreshing: false,
        last_refreshed_at_ms: None,
        tokens: Vec::new(),
        unpriced_tokens: Vec::new(),
        failed_chain_ids: vec![1, 56, 137],
        rate_limited_chain_ids: Vec::new(),
        unreachable_networks: vec![
            row(1, LastKnown::Held, Some(4_500.0), LAST_SEEN),
            row(56, LastKnown::Empty, None, LAST_SEEN_EMPTY),
            row(137, LastKnown::NotRead, None, NOT_READ_YET),
        ],
        unreachable_key: Some(UNREACHABLE_MANY.to_owned()),
        holdings_loading: false,
        cached_total_usd: Some(4_500.0),
        switcher: BalanceSwitcherView {
            open: false,
            loading: false,
            balances: Vec::new(),
        },
    }
}

/// Component-board balance variants (gallery Components tab).
pub fn balance_variants(s: &WalletStrings) -> Vec<BalanceModel> {
    vec![
        balance_default(s),
        BalanceModel {
            label: s.total_balance.clone(),
            currency: "USD".into(),
            state: BalanceState::ZeroLive,
            integer: "$0".into(),
            decimals: Some("00".into()),
            live: Some(s.live_indicator.clone()),
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            currency: "USD".into(),
            state: BalanceState::Loading,
            integer: "".into(),
            decimals: None,
            live: None,
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            currency: "USD".into(),
            state: BalanceState::Hidden,
            integer: BALANCE_MASK.into(),
            decimals: None,
            live: None,
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            currency: "USD".into(),
            state: BalanceState::Normal,
            integer: "$1,383".into(),
            decimals: Some("46".into()),
            live: None,
            status: Some((StatusKind::Warning, s.balance_unpriced.clone())),
        },
        BalanceModel {
            label: s.total_balance.clone(),
            currency: "USD".into(),
            state: BalanceState::Normal,
            integer: "$1,383".into(),
            decimals: Some("28".into()),
            live: None,
            status: Some((StatusKind::Refreshing, s.balance_stale.clone())),
        },
    ]
}

/// Which D1 row the celebration is about: the `+120 USDT` receipt at index 1.
///
/// The drawn state and the live one must be about the same thing — a glow on a
/// row the toast is not about would teach the drawing's reader the wrong rule
/// — so the index and the sentence below are two halves of one fixture.
pub const CELEBRATED_ROW: usize = 1;

/// D1b's celebration, in the words the corpus uses for the live one.
///
/// `+120 USDT` is what the row at [`CELEBRATED_ROW`] says, so the pill and the
/// row underneath it agree; formatted here rather than parsed back out of that
/// row's amount string, which is the reverse-parse the core removed.
pub fn receipt_toast(s: &WalletStrings) -> SharedString {
    SharedString::from(fill(
        &fill(&s.toast_received, "amount", "120"),
        "token",
        "USDT",
    ))
}

fn row(
    s: &WalletStrings,
    kind: ActivityKind,
    subtitle: String,
    amount: &str,
    unit: &str,
    positive: bool,
    badge: Hsla,
) -> ActivityRowModel {
    let title = match kind {
        ActivityKind::Sent => s.label_sent.clone(),
        ActivityKind::Received => s.label_received.clone(),
        ActivityKind::Dapp => s.label_dapp.clone(),
    };
    ActivityRowModel {
        kind,
        title,
        subtitle: subtitle.into(),
        amount: amount.into(),
        unit: unit.into(),
        positive,
        badge,
        badge_logo: None,
        day: None,
        received: None,
        danger: false,
    }
}

/// The four D1 activity rows, timestamps included (desktop subtitles carry
/// `· <day> <clock>` per the D1 mock).
pub fn activity_default(s: &WalletStrings) -> Vec<ActivityRowModel> {
    let today = s.today.as_ref();
    let yesterday = s.yesterday.as_ref();
    let mut rows = vec![
        row(
            s,
            ActivityKind::Sent,
            format!("{} · {today} 14:02", fill(&s.to_name, "name", "hold on")),
            "−2",
            "POL",
            false,
            chain_polygon(),
        ),
        row(
            s,
            ActivityKind::Received,
            format!(
                "{} · {today} 11:20",
                fill(&s.from_name, "name", "0x9F3c…21aE")
            ),
            "+120",
            "USDT",
            true,
            chain_ethereum(),
        ),
        row(
            s,
            ActivityKind::Dapp,
            format!("PancakeSwap · {today} 09:41"),
            "−0.05",
            "BNB",
            false,
            chain_bnb(),
        ),
        row(
            s,
            ActivityKind::Received,
            format!(
                "{} · {yesterday} 20:15",
                fill(&s.from_name, "name", "Alice")
            ),
            "+50",
            "USDC",
            true,
            chain_base(),
        ),
    ];
    // The web's D1 files them under their days (spec 038 #E3).
    rows[0].day = Some(s.today.clone());
    rows[3].day = Some(s.yesterday.clone());
    rows
}

/// Masked variants for the component board (H5's rule: dots, units kept).
pub fn activity_masked(s: &WalletStrings) -> Vec<ActivityRowModel> {
    activity_default(s)
        .into_iter()
        .map(|mut r| {
            r.amount = MASK.into();
            r
        })
        .collect()
}

pub fn assets_default(s: &WalletStrings) -> Vec<AssetRowModel> {
    let value = |v: &str| Fiat::Value(v.into());
    let asset = |ticker: &str, chain: &str, badge: Hsla, balance: &str, fiat: Fiat| AssetRowModel {
        ticker: ticker.into(),
        chain: chain.into(),
        badge,
        balance: balance.into(),
        fiat,
        logos: crate::marks::Logos::default(),
    };
    let _ = s;
    vec![
        asset("BNB", "BNB Chain", chain_bnb(), "0.8533", value("$496.46")),
        asset(
            "ETH",
            "Arbitrum",
            chain_arbitrum(),
            "0.2253",
            value("$422.62"),
        ),
        asset(
            "ETH",
            "Ethereum",
            chain_ethereum(),
            "0.0689",
            value("$129.25"),
        ),
        asset("XDAI", "Gnosis", chain_gnosis(), "74.3965", value("$74.38")),
        asset(
            "USDT",
            "Ethereum",
            chain_ethereum(),
            "53.4836",
            value("$53.48"),
        ),
        asset("USDC", "Polygon", chain_polygon(), "12.04", value("$12.04")),
    ]
}

/// Component-board asset variants: no-price (H4), masked (H5), extremes (H7).
pub fn assets_variants(s: &WalletStrings) -> Vec<AssetRowModel> {
    vec![
        AssetRowModel {
            ticker: "CAKE".into(),
            chain: "BNB Chain".into(),
            badge: chain_bnb(),
            balance: "18.20".into(),
            fiat: Fiat::NoPrice(s.no_price.clone()),
            logos: crate::marks::Logos::default(),
        },
        AssetRowModel {
            ticker: "BNB".into(),
            chain: "BNB Chain".into(),
            badge: chain_bnb(),
            balance: MASK.into(),
            fiat: Fiat::Masked,
            logos: crate::marks::Logos::default(),
        },
        AssetRowModel {
            ticker: "WBTC".into(),
            chain: "以太坊主网 Ethereum".into(),
            badge: chain_ethereum(),
            balance: "0.00000042".into(),
            fiat: Fiat::Value("$0.03".into()),
            logos: crate::marks::Logos::default(),
        },
        AssetRowModel {
            ticker: "USDT".into(),
            chain: "Ethereum".into(),
            badge: chain_ethereum(),
            balance: "1,234,567.8901".into(),
            fiat: Fiat::Value("$1,234,567.89".into()),
            logos: crate::marks::Logos::default(),
        },
    ]
}

pub fn chains(s: &WalletStrings) -> Vec<ChainRowModel> {
    let chain = |name: &str, chain_id: u32, dot: Hsla, count: u32| ChainRowModel {
        name: name.into(),
        dot: Some(dot),
        count,
        selected: false,
        chain_id: Some(chain_id),
    };
    vec![
        ChainRowModel {
            name: s.all_networks.clone(),
            dot: None,
            count: NETWORK_COUNT,
            selected: true,
            chain_id: None,
        },
        chain("BNB Chain", 56, chain_bnb(), 1),
        chain("Ethereum", 1, chain_ethereum(), 3),
        chain("Arbitrum", 42_161, chain_arbitrum(), 1),
        chain("Gnosis", 100, chain_gnosis(), 1),
        chain("Base", 8_453, chain_base(), 1),
        chain("Polygon", 137, chain_polygon(), 1),
    ]
}

/// D3's per-asset activity (BNB): the dApp row plus an older literal-dated one.
pub fn bnb_activity(s: &WalletStrings) -> Vec<ActivityRowModel> {
    let today = s.today.as_ref();
    vec![
        row(
            s,
            ActivityKind::Dapp,
            format!("PancakeSwap · {today} 09:41"),
            "−0.05",
            "BNB",
            false,
            chain_bnb(),
        ),
        row(
            s,
            ActivityKind::Received,
            // 8月1日 is fixture data (a literal date), verbatim per FR-012.
            format!("{} · 8月1日", fill(&s.from_name, "name", "0x21aE…9F3c")),
            "+0.9",
            "BNB",
            true,
            chain_bnb(),
        ),
    ]
}

/// D3, as one model.
///
/// Added in 031 so the live panel and the mock render through one body. The
/// fixture constructor below reproduces the mock's content exactly, so the
/// gallery is unchanged.
#[derive(Clone)]
pub struct AssetDetailModel {
    pub ticker: SharedString,
    pub badge: Hsla,
    /// The endpoint's logos for this holding (issue 201).
    pub logos: crate::marks::Logos,
    /// `0.8533 BNB`.
    pub amount: SharedString,
    /// `$496.46 · BNB Chain`.
    pub sub: SharedString,
    pub facts: Vec<(SharedString, SharedString)>,
    pub activity: Vec<ActivityRowModel>,
    /// The feed id behind each `activity` row, in drawn order — what a row
    /// opens. Empty in the mock: its rows are nobody's transactions.
    pub activity_ids: Vec<String>,
    /// Where "view on explorer" leads (the web's `tokenExplorerURL`). `None`
    /// for a chain with no explorer, and for the mock.
    pub explorer_url: Option<SharedString>,
    /// The whole contract address, for the Contract fact's copy (078 H-06) —
    /// the fact shows its two ends. `None` for a chain's own coin.
    pub contract_copy: Option<SharedString>,
}

/// D3 as the mocks draw it.
#[must_use]
pub fn asset_detail_default(s: &WalletStrings) -> AssetDetailModel {
    AssetDetailModel {
        logos: crate::marks::Logos::default(),
        ticker: "BNB".into(),
        badge: chain_bnb(),
        amount: "0.8533 BNB".into(),
        sub: "$496.46 · BNB Chain".into(),
        facts: bnb_facts(s),
        activity: bnb_activity(s),
        activity_ids: Vec::new(),
        // The web's D3 links its explorer; the mock's chain has one.
        explorer_url: Some("https://bscscan.com".into()),
        contract_copy: None,
    }
}

/// D3 fact rows.
pub fn bnb_facts(s: &WalletStrings) -> Vec<(SharedString, SharedString)> {
    vec![
        (s.label_name.clone(), "BNB".into()),
        (
            s.label_price.clone(),
            fill(&fill(&s.price_value, "symbol", "BNB"), "value", "$581.85").into(),
        ),
        (s.label_contract.clone(), s.native_token.clone()),
        (s.label_decimals.clone(), "18".into()),
    ]
}

/// D2 token-picker detail line: `BNB Chain · 链 ID 56`.
pub fn receive_network_detail(s: &WalletStrings) -> SharedString {
    fill(&fill(&s.network_detail, "name", "BNB Chain"), "id", "56").into()
}

/// D2 warning-card footnote: `同一地址，通用于全部 8 个网络`.
pub fn receive_networks_line(s: &WalletStrings) -> SharedString {
    fill(&s.networks_line, "count", &NETWORK_COUNT.to_string()).into()
}

/// Spec 093's three dApp interactions as the store hands them to the feed —
/// a swap on Uniswap, a Permit2 permit with no limit, a Sign-In with
/// Ethereum — each carrying the summary the signing core wrote at approve
/// time. Newest first: the swap at `now_sec`, the permit a minute before it,
/// the sign-in a minute before that. Fed to the REAL feed core by the
/// row-mapping and detail tests (and by the screenshot harness), so every
/// word the rows say is the core's.
#[cfg(test)]
pub fn dapp_activity_records(now_sec: f64) -> Vec<vela_core::app::activity_feed::FeedTxRecord> {
    use vela_core::app::activity_feed::{FeedTxKind, FeedTxRecord, FeedTxStatus};
    use vela_core::app::dapp_activity::{DappAction, DappSummary};
    use vela_core::app::token_trust::TrustSimJudgment;

    const ROUTER: &str = "0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad";
    const PERMIT2: &str = "0x000000000022d473030f116ddee9f6b43ac78ba3";
    const USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    let record = |id: &str, kind: FeedTxKind, at: f64, summary: DappSummary| FeedTxRecord {
        id: id.to_owned(),
        user_op_hash: String::new(),
        tx_hash: String::new(),
        from: "0xme".to_owned(),
        to: String::new(),
        to_name: None,
        value: "0".to_owned(),
        symbol: String::new(),
        decimals: 0,
        logo_urls: None,
        chain_id: 1,
        timestamp: at,
        day_start_ms: crate::executor::day_start_ms(at * 1000.0),
        status: FeedTxStatus::Confirmed,
        kind: Some(kind),
        usd: None,
        dapp_url: Some("https://app.uniswap.org".to_owned()),
        intent: None,
        balance_changes: None,
        calldata: None,
        call_data: None,
        summary: Some(summary),
        settlement: None,
    };
    let swap = FeedTxRecord {
        user_op_hash: "0x5c1e3fa0b2d4c6e8f0a1b3c5d7e9f1a3b5c7d9e1f3a5b7c9d1e3f5a7b9c1d3e5"
            .to_owned(),
        tx_hash: "0x9f2c4e5d6a7b8c9d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f".to_owned(),
        to: ROUTER.to_owned(),
        value: "0x0".to_owned(),
        symbol: "ETH".to_owned(),
        decimals: 18,
        intent: Some("Swap".to_owned()),
        balance_changes: Some(vec![
            TrustSimJudgment::Erc20Trusted {
                token: USDC.to_owned(),
                delta: "-100000000".to_owned(),
                symbol: "USDC".to_owned(),
                decimals: 6,
                in_trusted_set: true,
            },
            TrustSimJudgment::Native {
                delta: "30000000000000000".to_owned(),
            },
        ]),
        calldata: Some(true),
        call_data: Some("0x3593564c".to_owned()),
        ..record(
            "dapp-1-swap",
            FeedTxKind::DappTx,
            now_sec,
            DappSummary {
                action: DappAction::Call,
                calls: 1,
                contract: Some(ROUTER.to_owned()),
                ..DappSummary::default()
            },
        )
    };
    let permit = record(
        "dapp-2-permit",
        FeedTxKind::SignTypedData,
        now_sec - 60.0,
        DappSummary {
            action: DappAction::Permit,
            contract: Some(PERMIT2.to_owned()),
            spender: Some(ROUTER.to_owned()),
            token: Some(USDC.to_owned()),
            symbol: Some("USDC".to_owned()),
            decimals: Some(6),
            unlimited: true,
            primary_type: Some("PermitSingle".to_owned()),
            ..DappSummary::default()
        },
    );
    let sign_in = record(
        "dapp-3-siwe",
        FeedTxKind::SignMessage,
        now_sec - 120.0,
        DappSummary {
            action: DappAction::SignIn,
            signin_domain: Some("app.uniswap.org".to_owned()),
            ..DappSummary::default()
        },
    );
    vec![swap, permit, sign_in]
}

/// The REAL feed core over `records`, loaded the way the executor answers it
/// for the account `0xme` — what Activity draws from those records.
#[cfg(test)]
pub fn core_feed(
    records: Vec<vela_core::app::activity_feed::FeedTxRecord>,
) -> vela_core::app::activity_feed::FeedView {
    core_feed_host(records).view()
}

/// [`core_feed`]'s machine itself, for a test that tells it more (a contact
/// page opening).
#[cfg(test)]
pub fn core_feed_host(
    records: Vec<vela_core::app::activity_feed::FeedTxRecord>,
) -> crate::core_host::CoreHost<vela_core::app::activity_feed::ActivityFeed> {
    use vela_core::app::activity_feed::{
        ActivityFeed, Event as FeedEvent, FeedOperation, FeedShellResult,
    };
    let mut host = crate::core_host::CoreHost::<ActivityFeed>::new();
    let mut pending = host.dispatch(FeedEvent::AccountSwitched {
        address: "0xme".to_owned(),
    });
    for _ in 0..16 {
        let Some(next) = pending.pop() else {
            break;
        };
        let result = match &next.operation {
            FeedOperation::ReadTxStore { read_id, .. } => FeedShellResult::StoreLoaded {
                records: records.clone(),
                now_ms: 1_756_000_000_000.0,
                read_id: *read_id,
            },
            FeedOperation::ScanIncomingTransfers { .. } => {
                FeedShellResult::SyncCompleted { new_count: 0 }
            }
            FeedOperation::ResolveRecipientIdentity { addr } => FeedShellResult::AliasResolved {
                addr: addr.clone(),
                name: None,
            },
            FeedOperation::Timer { .. } => continue,
            FeedOperation::DeleteTxRecord { id } => {
                FeedShellResult::DeleteCommitted { id: id.clone() }
            }
            FeedOperation::Haptic => FeedShellResult::HapticPlayed,
        };
        pending.extend(host.resolve(next.id, result));
    }
    host
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;

    /// FR-012: with the zh locale the assembled strings match the mocks
    /// verbatim (this is the desktop twin of the web fixtures test).
    #[test]
    fn zh_fixtures_match_the_mocks() {
        // SAFETY: test-local env pin, same pattern the loc tests rely on.
        unsafe { std::env::set_var("VELA_LANG", "zh") };
        let loc = Loc::from_env();
        let s = WalletStrings::resolve(&loc);

        let rows = activity_default(&s);
        assert_eq!(rows[0].title.as_ref(), "已发送");
        assert_eq!(rows[0].subtitle.as_ref(), "至 hold on · 今天 14:02");
        assert_eq!(rows[3].subtitle.as_ref(), "来自 Alice · 昨天 20:15");

        assert_eq!(receive_network_detail(&s).as_ref(), "BNB Chain · 链 ID 56");
        assert_eq!(
            receive_networks_line(&s).as_ref(),
            "同一地址，通用于全部 8 个网络"
        );

        let facts = bnb_facts(&s);
        assert_eq!(facts[1].1.as_ref(), "1 BNB = $581.85");
        assert_eq!(facts[2].1.as_ref(), "原生代币");

        let chain_rows = chains(&s);
        assert_eq!(chain_rows[0].name.as_ref(), "所有网络");
        assert_eq!(chain_rows.len(), 7);
    }
}
