//! Canonical settings fixtures (spec 023) — the desktop port of the web's
//! `lib/settings/fixtures.ts`, which is itself read off `design/settings/`.
//!
//! Numbers, URLs, latencies and colours are DATA and identical across
//! platforms, so a reviewer comparing the four clients is comparing the same
//! wallet. Labels resolve through `SettingsStrings`; components never format.

use gpui::SharedString;

use crate::icons::Icon;
use crate::wallet::fill;

use super::SettingsStrings;

/// The gallery state inventory (desktop). One id per mock in
/// `design/settings/`, checked by `gallery_exposes_every_desktop_settings_state`.
#[allow(dead_code, reason = "gallery inventory contract, asserted by tests")]
pub const DESKTOP_STATES: [&str; 10] = [
    "dst1", "dst2", "dst3", "dst4", "dst4b", "dst5", "dst6", "dst7", "dst8", "dsr1",
];

/// Which panel the second-level nav is showing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettingsPage {
    Account,
    Appearance,
    Localization,
    Networks,
    RpcProviders,
    Endpoints,
    Storage,
    About,
}

impl SettingsPage {
    /// The nav column, in order. One array so the rail and the tests can never
    /// disagree about what the section contains.
    pub const ALL: [SettingsPage; 8] = [
        SettingsPage::Account,
        SettingsPage::Appearance,
        SettingsPage::Localization,
        SettingsPage::Networks,
        SettingsPage::RpcProviders,
        SettingsPage::Endpoints,
        SettingsPage::Storage,
        SettingsPage::About,
    ];

    pub fn icon(self) -> Icon {
        match self {
            SettingsPage::Account => Icon::UsersRound,
            SettingsPage::Appearance => Icon::Sun,
            SettingsPage::Localization => Icon::Coins,
            SettingsPage::Networks => Icon::Network,
            SettingsPage::RpcProviders => Icon::Server,
            SettingsPage::Endpoints => Icon::Zap,
            SettingsPage::Storage => Icon::HardDrive,
            SettingsPage::About => Icon::Info,
        }
    }

    pub fn label(self, s: &SettingsStrings) -> SharedString {
        match self {
            SettingsPage::Account => s.nav_account.clone(),
            SettingsPage::Appearance => s.nav_appearance.clone(),
            SettingsPage::Localization => s.nav_localization.clone(),
            SettingsPage::Networks => s.nav_networks.clone(),
            SettingsPage::RpcProviders => s.nav_rpc_providers.clone(),
            SettingsPage::Endpoints => s.nav_endpoints.clone(),
            SettingsPage::Storage => s.nav_storage.clone(),
            SettingsPage::About => s.nav_about.clone(),
        }
    }
}

/// Status-pill tone. `Neutral` is unset/idle, not failed.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tone {
    Ok,
    Warn,
    Error,
    Neutral,
}

#[derive(Clone, Debug)]
pub struct Pill {
    pub tone: Tone,
    pub label: SharedString,
    pub dot: bool,
}

pub fn pill(tone: Tone, label: impl Into<SharedString>) -> Pill {
    Pill {
        tone,
        label: label.into(),
        dot: true,
    }
}

/// `45ms`, or `在线 · 45ms` when a prefix is given. Over a second the tone
/// steps down to warning and the unit becomes seconds, which is the only way
/// "1.2s" reads as slow rather than as a very small number.
pub fn latency(ms: u32, prefix: Option<&str>) -> Pill {
    let tone = if ms >= 1000 { Tone::Warn } else { Tone::Ok };
    let value = if ms >= 1000 {
        format!("{:.1}s", f64::from(ms) / 1000.)
    } else {
        format!("{ms}ms")
    };
    let label = match prefix {
        Some(p) => format!("{p} · {value}"),
        None => value,
    };
    pill(tone, label)
}

// -- accounts -----------------------------------------------------------------

/// The three accounts DST1 lists. The first is the active one.
pub struct AccountFixture {
    pub name: &'static str,
    pub address_full: &'static str,
    pub address_display: &'static str,
    pub amount: &'static str,
}

pub const ACCOUNTS: [AccountFixture; 3] = [
    AccountFixture {
        name: "大表哥",
        address_full: "0x14fB1f4E2b9C7a5D8e3F6a1B4c7D9e2F5a8B1D1eA5c",
        address_display: "0x14fB...D1eA5c",
        amount: "$3,140.22",
    },
    AccountFixture {
        name: "旅行基金",
        address_full: "0x9a01c4E7b2F5a8D3e6C9b1A4d7F0e3B6c9D277C2b",
        address_display: "0x9a01...77C2b",
        amount: "$122.18",
    },
    AccountFixture {
        name: "试验田",
        address_full: "0x3Ce4f7A0b3D6e9C2a5F8b1E4d7C0a3F6b9E2A90f1",
        address_display: "0x3Ce4...A90f1",
        amount: "$0.00",
    },
];

pub const TOTAL_BALANCE: &str = "$3,262.40";

/// "3 个账户 · 总计 $3,262.40" — composed here, because the order of the two
/// clauses is a translation concern and a component must never learn one.
pub fn accounts_summary(s: &SettingsStrings) -> SharedString {
    let count = fill(&s.accounts_count, "count", &ACCOUNTS.len().to_string());
    let total = fill(&s.accounts_total, "amount", TOTAL_BALANCE);
    SharedString::from(format!("{count}{total}"))
}

// -- networks -----------------------------------------------------------------

/// The eight networks ST9/DST4 list, in order.
///
/// `color` is the chain's own brand colour as 0xRRGGBB, converted through
/// `gpui::rgb` at render time. Brand colours, not theme tokens: they belong to
/// Ethereum and BNB, and must not flip with the appearance.
pub struct NetworkFixture {
    pub id: &'static str,
    pub name: &'static str,
    pub letter: &'static str,
    pub color: u32,
    pub chain_id: u64,
    pub latency_ms: u32,
    pub custom: bool,
}

pub const NETWORKS: [NetworkFixture; 8] = [
    NetworkFixture {
        id: "ethereum",
        name: "Ethereum",
        letter: "E",
        color: 0x627eea,
        chain_id: 1,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "bnb",
        name: "BNB Chain",
        letter: "B",
        color: 0xf0b90b,
        chain_id: 56,
        latency_ms: 128,
        custom: false,
    },
    NetworkFixture {
        id: "polygon",
        name: "Polygon",
        letter: "P",
        color: 0x8247e5,
        chain_id: 137,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "arbitrum",
        name: "Arbitrum",
        letter: "A",
        color: 0x28a0f0,
        chain_id: 42161,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "base",
        name: "Base",
        letter: "B",
        color: 0x0052ff,
        chain_id: 8453,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "gnosis",
        name: "Gnosis",
        letter: "G",
        color: 0x2e9e7e,
        chain_id: 100,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "tempo",
        name: "Tempo",
        letter: "T",
        color: 0x8c8c8c,
        chain_id: 4217,
        latency_ms: 45,
        custom: false,
    },
    NetworkFixture {
        id: "xlayer",
        name: "X Layer",
        letter: "X",
        color: 0x8c8c8c,
        chain_id: 196,
        latency_ms: 0,
        custom: true,
    },
];

/// DST4 shows five built-ins plus the custom tail; Gnosis and Tempo fall below
/// the fold in the mock, and the desktop list is the same data cut the same way.
pub const DESKTOP_NETWORK_IDS: [&str; 6] =
    ["ethereum", "bnb", "polygon", "arbitrum", "base", "xlayer"];

pub const NETWORK_COUNT: u32 = 12;

/// "链 1" — the line under each network's name.
pub fn chain_meta(s: &SettingsStrings, chain_id: u64) -> SharedString {
    SharedString::from(fill(&s.chain_id, "chainId", &chain_id.to_string()))
}

pub const ETHEREUM_RPC: &str = "https://eth.llamarpc.com";
pub const ETHEREUM_EXPLORER: &str = "https://etherscan.io";

// -- add network (DST4b) ------------------------------------------------------

pub const ZORA_CHAIN_ID: u64 = 7_777_777;
pub const ZORA_BEST_RPC_MS: u32 = 182;

/// The four requirements ST10b/ST10c check. Both states show all four — a
/// shortened list would hide WHICH one failed, which is the only useful part
/// of an "incompatible" answer.
pub fn compatibility_checks(s: &SettingsStrings, ok: bool) -> [(SharedString, bool); 4] {
    [
        // A product name, not prose: translating it would make the row lie.
        // EntryPoint is deployed everywhere, so it passes in both states — the
        // three that follow are what "incompatible" is actually about.
        (SharedString::from("EntryPoint v0.7"), true),
        (s.check_safe.clone(), ok),
        (s.check_signer.clone(), ok),
        (
            SharedString::from(fill(&s.check_remaining, "count", "8")),
            ok,
        ),
    ]
}

// -- rpc providers ------------------------------------------------------------

pub struct ProviderFixture {
    pub name: &'static str,
    pub key: &'static str,
    pub supported: u32,
    pub avg_latency_ms: Option<u32>,
}

pub const PROVIDERS: [ProviderFixture; 3] = [
    ProviderFixture {
        name: "Alchemy",
        key: "alch_k3y...9fQ2",
        supported: 12,
        avg_latency_ms: Some(112),
    },
    ProviderFixture {
        name: "dRPC",
        key: "",
        supported: 0,
        avg_latency_ms: None,
    },
    ProviderFixture {
        name: "Ankr",
        key: "",
        supported: 8,
        avg_latency_ms: None,
    },
];

/// "支持 12 个网络，共 12 个 · 平均 112ms".
pub fn provider_support(s: &SettingsStrings, p: &ProviderFixture) -> Option<SharedString> {
    if p.supported == 0 {
        return None;
    }
    let base = fill(
        &fill(&s.provider_supports, "count", &p.supported.to_string()),
        "total",
        &NETWORK_COUNT.to_string(),
    );
    Some(SharedString::from(match p.avg_latency_ms {
        Some(ms) => format!(
            "{base} · {}",
            fill(&s.provider_avg_latency, "ms", &ms.to_string())
        ),
        None => base,
    }))
}

// -- endpoints ----------------------------------------------------------------

pub struct EndpointFixture {
    pub url: &'static str,
    pub latency_ms: u32,
}

pub const ENDPOINTS: [EndpointFixture; 4] = [
    EndpointFixture {
        url: "https://ethereum-data.awesometools.dev",
        latency_ms: 62,
    },
    EndpointFixture {
        url: "https://p256-index-rs.getvela.app",
        latency_ms: 88,
    },
    EndpointFixture {
        url: "https://vela-relay.getvela.app",
        latency_ms: 104,
    },
    EndpointFixture {
        url: "https://vela-currency.getvela.app/v2/…",
        latency_ms: 1200,
    },
];

/// Label + hint per endpoint, in `ENDPOINTS` order.
pub fn endpoint_copy(s: &SettingsStrings) -> [(SharedString, SharedString); 4] {
    [
        (
            s.endpoint_chain_data.clone(),
            s.endpoint_chain_data_hint.clone(),
        ),
        (s.endpoint_passkey.clone(), s.endpoint_passkey_hint.clone()),
        (s.endpoint_relay.clone(), s.endpoint_relay_hint.clone()),
        (s.endpoint_fiat.clone(), s.endpoint_fiat_hint.clone()),
    ]
}

// -- storage ------------------------------------------------------------------

pub const STORAGE_AMOUNT: &str = "2.4";
pub const STORAGE_UNIT: &str = "MB";
pub const STORAGE_RECORDS: u32 = 216;

/// The bar's three shares. Blue = your data, green = cache, grey = sessions.
pub const STORAGE_SEGMENTS: [(f32, u32); 3] = [(0.5, 0x5a7cf6), (0.3, 0x3da872), (0.2, 0x85827a)];

pub struct StorageItem {
    pub label: SharedString,
    pub meta: SharedString,
    /// 清除 for most rows; 断开全部 for the dApp sessions one.
    pub action: SharedString,
    pub destructive: bool,
}

pub struct StorageGroup {
    pub label: SharedString,
    pub items: Vec<StorageItem>,
    /// The 清除全部缓存 link under the cache group.
    pub action: Option<SharedString>,
}

/// The three groups, with their consequence spelled out in the group label —
/// which is what makes the red 清除 in the first group and the plain one in
/// the second read as the same word meaning two different things.
pub fn storage_groups(s: &SettingsStrings) -> Vec<StorageGroup> {
    let records = |n: u32| fill(&s.count_records, "count", &n.to_string());
    vec![
        StorageGroup {
            label: s.storage_user_data.clone(),
            action: None,
            items: vec![
                StorageItem {
                    label: s.item_transactions.clone(),
                    meta: SharedString::from(format!("{} · 1.0 MB", records(200))),
                    action: s.storage_clear.clone(),
                    destructive: true,
                },
                StorageItem {
                    label: s.item_contacts.clone(),
                    meta: SharedString::from(format!(
                        "{} · 42 KB",
                        fill(&s.count_contacts, "count", "18")
                    )),
                    action: s.storage_clear.clone(),
                    destructive: true,
                },
                StorageItem {
                    label: s.item_custom.clone(),
                    meta: SharedString::from(format!(
                        "{} · 12 KB",
                        fill(&s.count_items, "count", "5")
                    )),
                    action: s.storage_clear.clone(),
                    destructive: true,
                },
                StorageItem {
                    label: s.item_browsing.clone(),
                    meta: SharedString::from(format!("{} · 58 KB", records(31))),
                    action: s.storage_clear.clone(),
                    destructive: true,
                },
            ],
        },
        StorageGroup {
            label: s.storage_caches.clone(),
            action: Some(s.storage_clear_all.clone()),
            items: vec![
                StorageItem {
                    label: s.item_balances.clone(),
                    meta: SharedString::from("0.6 MB"),
                    action: s.storage_clear.clone(),
                    destructive: false,
                },
                StorageItem {
                    label: s.item_rates.clone(),
                    meta: SharedString::from("96 KB"),
                    action: s.storage_clear.clone(),
                    destructive: false,
                },
                StorageItem {
                    label: s.item_scan.clone(),
                    meta: SharedString::from("31 KB"),
                    action: s.storage_clear.clone(),
                    destructive: false,
                },
            ],
        },
        StorageGroup {
            label: s.storage_connections.clone(),
            action: None,
            items: vec![StorageItem {
                label: s.item_dapps.clone(),
                meta: SharedString::from(fill(&s.count_sites, "count", "4")),
                action: s.storage_disconnect_all.clone(),
                destructive: true,
            }],
        },
    ]
}

// -- about --------------------------------------------------------------------

/// The mock's version. NOT the app's — see [`about_version`].
pub const APP_VERSION: &str = "1.0.0";
pub const APP_COMMIT: &str = "6ab8f";

/// The version line.
///
/// `live` reads `CARGO_PKG_VERSION`, because the panel said **v1.0.0 (6ab8f)**
/// while the crate was 0.1.1 — and a bug report that quotes a version names one
/// that does not exist. The COMMIT stays the mock's for now: there is no
/// build-time git hash in this crate, and a made-up one is the half of that
/// line that was already wrong.
pub fn about_version(s: &SettingsStrings, live: bool) -> SharedString {
    let version = if live {
        env!("CARGO_PKG_VERSION")
    } else {
        APP_VERSION
    };
    SharedString::from(fill(
        &fill(&s.about_version, "version", version),
        "commit",
        APP_COMMIT,
    ))
}

/// Label / value / mono, in DST8's order.
pub fn about_rows(s: &SettingsStrings) -> Vec<(SharedString, SharedString, bool)> {
    vec![
        (
            s.about_wallet_label.clone(),
            s.about_wallet_value.clone(),
            true,
        ),
        (s.about_auth_label.clone(), s.about_auth_value.clone(), true),
        (
            s.about_account_label.clone(),
            s.about_account_value.clone(),
            false,
        ),
        (
            s.about_signer_label.clone(),
            s.about_signer_value.clone(),
            false,
        ),
        (
            s.about_networks_label.clone(),
            SharedString::from(fill(
                &s.about_networks_value,
                "count",
                &NETWORK_COUNT.to_string(),
            )),
            false,
        ),
    ]
}

pub fn about_links(s: &SettingsStrings) -> Vec<(SharedString, SharedString)> {
    vec![
        (
            s.about_link_website.clone(),
            SharedString::from("getvela.app"),
        ),
        (
            s.about_link_github.clone(),
            SharedString::from("github.com/mondaylabsltd/vela-wallet"),
        ),
        (s.about_link_safe.clone(), SharedString::from("safe.global")),
    ]
}

// -- rescue (DSR1) ------------------------------------------------------------

pub const RPC_FIX_CHAIN: &str = "polygon";
pub const RPC_FIX_URL: &str = "https://polygon-rpc.com";
pub const RPC_FIX_SYMBOL: &str = "POL";

/// The two unreachable networks DSR1's banner names, and the count above them.
pub const BANNER_CHAINS: [&str; 2] = ["polygon", "gnosis"];

pub fn banner_text(s: &SettingsStrings) -> SharedString {
    SharedString::from(fill(
        &s.rpc_unavailable_multiple,
        "count",
        &BANNER_CHAINS.len().to_string(),
    ))
}

/// The four places SR2/DSR1 point at for a working endpoint.
pub const RPC_PROVIDER_LINKS: [&str; 4] = ["Alchemy", "QuickNode", "dRPC", "Chainlist"];

/// Find a network fixture by id. Panics only on a typo in this file's own
/// constants, which a test catches before anybody runs the app.
pub fn network(id: &str) -> &'static NetworkFixture {
    NETWORKS
        .iter()
        .find(|n| n.id == id)
        .expect("settings fixture ids are internal constants")
}

/// The desktop 网络 rows as the mocks draw them.
///
/// An ADAPTER, not a new fixture: every value still comes from the constants
/// above. It exists so the screen can take a `Vec<NetworkRowModel>` from either
/// side of the seam and not know which.
#[must_use]
pub fn network_rows() -> Vec<crate::settings::model::NetworkRowModel> {
    DESKTOP_NETWORK_IDS
        .into_iter()
        .map(|id| {
            let n = network(id);
            crate::settings::model::NetworkRowModel {
                id: SharedString::from(n.id),
                name: SharedString::from(n.name),
                letter: SharedString::from(n.letter),
                color: n.color,
                chain_id: n.chain_id,
                latency_ms: (!n.custom).then_some(n.latency_ms),
                custom: n.custom,
            }
        })
        .collect()
}

#[cfg(test)]
mod row_adapter_tests {
    use super::*;

    /// The adapter must reproduce what `page.rs` drew before the seam existed,
    /// value for value. If it does not, going live silently redrew a reviewed
    /// screen — which is the failure spec 030 FR-003 is written against.
    #[test]
    fn the_row_adapter_reproduces_the_mock_exactly() {
        let rows = network_rows();
        assert_eq!(rows.len(), DESKTOP_NETWORK_IDS.len());
        for (row, id) in rows.iter().zip(DESKTOP_NETWORK_IDS) {
            let n = network(id);
            assert_eq!(row.id, SharedString::from(n.id));
            assert_eq!(row.name, SharedString::from(n.name));
            assert_eq!(row.letter, SharedString::from(n.letter));
            assert_eq!(row.color, n.color);
            assert_eq!(row.chain_id, n.chain_id);
            assert_eq!(row.custom, n.custom);
            // The old code drew a badge for every non-custom row, and none for
            // a custom one.
            assert_eq!(
                row.latency_ms,
                (!n.custom).then_some(n.latency_ms),
                "the badge rule moved"
            );
        }
    }
}
