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
        state: BalanceState::Normal,
        integer: "$1,383".into(),
        decimals: Some("28".into()),
        live: None,
        status: None,
    }
}

/// Component-board balance variants (gallery Components tab).
pub fn balance_variants(s: &WalletStrings) -> Vec<BalanceModel> {
    vec![
        balance_default(s),
        BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::ZeroLive,
            integer: "$0".into(),
            decimals: Some("00".into()),
            live: Some(s.live_indicator.clone()),
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::Loading,
            integer: "".into(),
            decimals: None,
            live: None,
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::Hidden,
            integer: BALANCE_MASK.into(),
            decimals: None,
            live: None,
            status: None,
        },
        BalanceModel {
            label: s.total_balance.clone(),
            state: BalanceState::Normal,
            integer: "$1,383".into(),
            decimals: Some("46".into()),
            live: None,
            status: Some((StatusKind::Warning, s.balance_unpriced.clone())),
        },
        BalanceModel {
            label: s.total_balance.clone(),
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
    }
}

/// The four D1 activity rows, timestamps included (desktop subtitles carry
/// `· <day> <clock>` per the D1 mock).
pub fn activity_default(s: &WalletStrings) -> Vec<ActivityRowModel> {
    let today = s.today.as_ref();
    let yesterday = s.yesterday.as_ref();
    vec![
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
    ]
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
        },
        AssetRowModel {
            ticker: "BNB".into(),
            chain: "BNB Chain".into(),
            badge: chain_bnb(),
            balance: MASK.into(),
            fiat: Fiat::Masked,
        },
        AssetRowModel {
            ticker: "WBTC".into(),
            chain: "以太坊主网 Ethereum".into(),
            badge: chain_ethereum(),
            balance: "0.00000042".into(),
            fiat: Fiat::Value("$0.03".into()),
        },
        AssetRowModel {
            ticker: "USDT".into(),
            chain: "Ethereum".into(),
            badge: chain_ethereum(),
            balance: "1,234,567.8901".into(),
            fiat: Fiat::Value("$1,234,567.89".into()),
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
    /// `0.8533 BNB`.
    pub amount: SharedString,
    /// `$496.46 · BNB Chain`.
    pub sub: SharedString,
    pub facts: Vec<(SharedString, SharedString)>,
    pub activity: Vec<ActivityRowModel>,
}

/// D3 as the mocks draw it.
#[must_use]
pub fn asset_detail_default(s: &WalletStrings) -> AssetDetailModel {
    AssetDetailModel {
        ticker: "BNB".into(),
        badge: chain_bnb(),
        amount: "0.8533 BNB".into(),
        sub: "$496.46 · BNB Chain".into(),
        facts: bnb_facts(s),
        activity: bnb_activity(s),
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
