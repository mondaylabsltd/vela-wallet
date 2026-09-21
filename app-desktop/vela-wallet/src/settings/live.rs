//! The settings screen's display models, built from what the cores decided.
//!
//! The **sibling** of `fixtures.rs`, never its replacement. Both produce the
//! same shapes; the screen picks which one feeds it, and the gallery always
//! picks the fixture. That is what keeps every drawn state reviewable after the
//! real data arrives — and what makes "the galleries are unchanged" something a
//! diff can prove rather than something a reviewer has to eyeball.

use gpui::SharedString;

use crate::settings::SettingsStrings;
use crate::settings::fixtures::{Pill, Tone};
use crate::settings::model::{NetworkRowModel, chain_tint, lettermark};

use vela_core::app::display_currency::CurrencyView;
use vela_core::app::network_admin::{
    Event as NetEvent, NetCompatibility, NetEndpointField, NetNetworkRow, NetOverrideField,
    NetProbeHealth, NetProviderId, NetServiceHealth, NetView, NetWizardErrorKind, NetWizardPhase,
    NetWizardView,
};
use vela_core::l10n::currency::format_fiat;
use vela_core::l10n::datetime::{Civil, DatePreset, TimePreset, format_date, format_time};
use vela_core::l10n::number::{FractionDigits, NumberPreset, format_number, format_token_amount};
use vela_core::storage_catalog::StorageGroup as CatalogGroup;

use crate::executor::device_storage::{Report, Usage};
use crate::settings::fixtures::{StorageGroup, StorageItem};

use crate::executor::format_prefs::{self, Choice, Formats};
use crate::settings::components::MenuRow;

/// The sample figure the 本地化 mock prints beside the currency code.
const SAMPLE: f64 = 1234.56;

// ---------------------------------------------------------------------------
// Appearance and the account panel (spec 072)
// ---------------------------------------------------------------------------

/// The theme control's cells, in the order DST2 draws them: Light, Dark,
/// Follow System. The words are the stored ones (`vela.theme`).
pub const THEME_SEGMENTS: [&str; 3] = ["light", "dark", "system"];

/// The avatar control's cells: Initials, Identicon (`vela.avatarStyle`).
pub const AVATAR_SEGMENTS: [&str; 2] = ["initials", "identicon"];

/// Which cell a stored word selects — the first when the word is not one of
/// them, which the core's reader never hands out.
#[must_use]
pub fn segment_of(cells: &[&str], word: &str) -> usize {
    cells.iter().position(|cell| *cell == word).unwrap_or(0)
}

/// The text-size stop a level sits on (`vela.textScale`).
#[must_use]
pub fn text_scale_index(level: &str) -> usize {
    vela_core::prefs::TEXT_SCALE_LEVELS
        .iter()
        .position(|(name, _)| *name == level)
        .unwrap_or(2)
}

fn endonym(tag: &str) -> SharedString {
    SharedString::from(
        crate::settings::fixtures::LOCALE_ENDONYMS
            .iter()
            .find(|(id, _)| *id == tag)
            .map_or(tag, |(_, name)| *name)
            .to_owned(),
    )
}

/// The language row: the chosen language by its own name, or — following the
/// system — the language the system resolves to, "· System" beside it. The
/// web's `languageValue`.
#[must_use]
pub fn language_value(
    pinned: Option<&str>,
    system_language: &str,
    s: &SettingsStrings,
) -> SharedString {
    match pinned {
        Some(tag) => endonym(tag),
        None => SharedString::from(format!("{} · {}", endonym(system_language), s.note_system)),
    }
}

/// The language menu: "Follow System" first, with what it currently resolves
/// to, then every shipped locale by its endonym. The web's `languageRows`.
#[must_use]
pub fn language_menu(
    pinned: Option<&str>,
    system_language: &str,
    s: &SettingsStrings,
) -> Vec<MenuRow> {
    let mut rows = vec![(
        s.language_follow_system.clone(),
        Some(SharedString::from(format!(
            "{} · {}",
            s.note_system,
            endonym(system_language)
        ))),
        pinned.is_none(),
    )];
    rows.extend(
        crate::settings::fixtures::LOCALE_ENDONYMS
            .iter()
            .map(|(id, name)| (SharedString::from(*name), None, pinned == Some(*id))),
    );
    rows
}

/// A language-menu index back to the stored word: row 0 is `auto`.
#[must_use]
pub fn picked_language(index: usize) -> Option<&'static str> {
    match index {
        0 => Some(vela_core::prefs::AUTO_LANGUAGE),
        _ => crate::settings::fixtures::LOCALE_ENDONYMS
            .get(index - 1)
            .map(|(id, _)| *id),
    }
}

/// The currency menu and the code each row picks: the drawn eight, plus the
/// committed code when it is none of them — a choice never disappears from
/// the list that shows it.
#[must_use]
pub fn currency_menu(view: &CurrencyView) -> (Vec<MenuRow>, Vec<String>) {
    let mut codes: Vec<String> = crate::settings::fixtures::CURRENCY_CODES
        .iter()
        .map(|code| (*code).to_owned())
        .collect();
    if !codes.contains(&view.code) && !view.code.is_empty() {
        codes.push(view.code.clone());
    }
    let rows = codes
        .iter()
        .map(|code| {
            let symbol = symbol_for(code);
            (
                SharedString::from(code.clone()),
                (symbol != code).then(|| SharedString::from(symbol.to_owned())),
                *code == view.code,
            )
        })
        .collect();
    (rows, codes)
}

/// One account's total in the display currency (spec 072).
///
/// Converted only when the core priced the currency: `rate: None` is not 1,
/// so an unpriced choice prints the USD figure the core does have rather than
/// dressing it in another currency's symbol.
#[must_use]
pub fn account_total(usd: f64, currency: Option<&CurrencyView>, locale: &str) -> SharedString {
    let options = crate::executor::format_prefs::fiat_options();
    SharedString::from(match currency {
        Some(CurrencyView {
            code,
            rate: Some(rate),
            ..
        }) => format_fiat(usd * rate, code, symbol_for(code), locale, options),
        _ => format_fiat(usd, "USD", "$", locale, options),
    })
}

// ---------------------------------------------------------------------------
// Device storage (spec 072) — the rows are the core's catalog, the numbers
// this device's own
// ---------------------------------------------------------------------------

/// A byte count as the figure and the unit: in 1024s (the core's
/// `bytes_display`), the figure in the person's number format — none below a
/// kilobyte, where "512 B" is exact and "0.5 KB" is noise, one above.
#[must_use]
pub fn bytes_text(bytes: u64, preset: NumberPreset) -> (SharedString, SharedString) {
    let (value, unit) = vela_core::storage_catalog::bytes_display(bytes);
    let places = usize::from(unit != "B");
    (
        SharedString::from(format_number(
            value,
            preset,
            FractionDigits {
                min: places,
                max: places,
            },
        )),
        SharedString::from(unit),
    )
}

fn size_text(bytes: u64, preset: NumberPreset) -> String {
    let (amount, unit) = bytes_text(bytes, preset);
    format!("{amount} {unit}")
}

/// A catalog row's drawn name.
#[must_use]
pub fn storage_item_label(id: &str, s: &SettingsStrings) -> SharedString {
    match id {
        "transactions" => s.item_transactions.clone(),
        "contacts" => s.item_contacts.clone(),
        "custom" => s.item_custom.clone(),
        "browsing" => s.item_browsing.clone(),
        "balances" => s.item_balances.clone(),
        "rates" => s.item_rates.clone(),
        "scan" => s.item_scan.clone(),
        _ => s.item_dapps.clone(),
    }
}

/// A row's meta line, in the row's own unit — the web's `storageItemMeta`.
#[must_use]
pub fn storage_item_meta(
    id: &str,
    usage: Usage,
    s: &SettingsStrings,
    preset: NumberPreset,
) -> SharedString {
    let count = |template: &str| crate::wallet::fill(template, "count", &usage.count.to_string());
    let size = size_text(usage.bytes, preset);
    SharedString::from(match id {
        "transactions" | "browsing" => format!("{} · {size}", count(&s.count_records)),
        "contacts" => format!("{} · {size}", count(&s.count_contacts)),
        "custom" => format!("{} · {size}", count(&s.count_items)),
        "dapps" => count(&s.count_sites),
        _ => size,
    })
}

/// DST7's two measured groups — your data, then the caches — one row per
/// catalog item, in the catalog's order. The connections group is the
/// browser machine's list and is drawn from it, not from here.
#[must_use]
pub fn storage_groups(
    report: &Report,
    s: &SettingsStrings,
    preset: NumberPreset,
) -> Vec<StorageGroup> {
    [
        (CatalogGroup::User, s.storage_user_data.clone(), None),
        (
            CatalogGroup::Cache,
            s.storage_caches.clone(),
            Some(s.storage_clear_all.clone()),
        ),
    ]
    .into_iter()
    .map(|(group, label, action)| StorageGroup {
        label,
        action,
        items: vela_core::storage_catalog::ITEMS
            .iter()
            .filter(|item| item.group == group)
            .map(|item| StorageItem {
                id: item.id,
                label: storage_item_label(item.id, s),
                meta: storage_item_meta(item.id, report.item(item.id), s, preset),
                action: s.storage_clear.clone(),
                // Red where it cannot come back; plain where it rebuilds.
                destructive: group == CatalogGroup::User,
            })
            .collect(),
    })
    .collect()
}

/// The bar's three shares, from the measured bytes, in the drawn colours.
#[must_use]
pub fn storage_segments(report: &Report) -> [(f32, u32); 3] {
    let colors = crate::settings::fixtures::STORAGE_SEGMENTS;
    let share = |group| {
        if report.total_bytes == 0 {
            0.
        } else {
            #[allow(clippy::cast_precision_loss, reason = "a share of a byte count")]
            let share = report.group(group) as f32 / report.total_bytes as f32;
            share
        }
    };
    [
        (share(CatalogGroup::User), colors[0].1),
        (share(CatalogGroup::Cache), colors[1].1),
        (share(CatalogGroup::Sessions), colors[2].1),
    ]
}

// ---------------------------------------------------------------------------
// Save on blur or Enter (spec 072)
// ---------------------------------------------------------------------------

/// An editable settings field whose value the core persists only when the
/// person is done with it.
///
/// Each keystroke is an EDIT — a draft, re-probed where the core re-probes —
/// and leaving the field (or pressing Enter) is the COMMIT. The desktop used
/// to send both on every keystroke, so an override was saved, and the
/// wrong-chain refusal was run, against `https://ma` on the way to typing
/// `https://mainnet…`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldCommit {
    Endpoint(NetEndpointField),
    ProviderKey(NetProviderId),
    Override {
        chain_id: u32,
        field: NetOverrideField,
    },
}

impl FieldCommit {
    /// What one keystroke tells the core.
    #[must_use]
    pub fn edited(self, value: String) -> NetEvent {
        match self {
            FieldCommit::Endpoint(field) => NetEvent::EndpointEdited { field, value },
            FieldCommit::ProviderKey(provider) => NetEvent::ProviderKeyEdited { provider, value },
            FieldCommit::Override { chain_id, field } => NetEvent::OverrideFieldEdited {
                chain_id,
                field,
                value,
            },
        }
    }

    /// What leaving the field tells it — the event that persists.
    #[must_use]
    pub fn committed(self) -> NetEvent {
        match self {
            FieldCommit::Endpoint(field) => NetEvent::EndpointBlurred { field },
            FieldCommit::ProviderKey(provider) => NetEvent::ProviderKeyBlurred { provider },
            FieldCommit::Override { chain_id, .. } => NetEvent::OverrideBlurred { chain_id },
        }
    }
}

/// Symbols for the codes a person can actually reach today.
///
/// Deliberately small. The shell owns the currency catalog — that is the core's
/// division of labour, stated in `display_currency.rs` — but a full catalog is
/// only *useful* once rates exist, because until then no code but USD can be
/// priced at all. It arrives with the rates in spec 031. Unknown codes fall back
/// to the code itself, which `format_fiat` spaces correctly (`CHF 1,234.56`)
/// because CLDR's `currencySpacing` keys off the symbol being alphabetic.
fn symbol_for(code: &str) -> &str {
    match code {
        "USD" => "$",
        "EUR" => "€",
        "GBP" => "£",
        "JPY" | "CNY" => "¥",
        "KRW" => "₩",
        "VND" => "₫",
        "INR" => "₹",
        "HKD" => "HK$",
        other => other,
    }
}

/// The value the 货币 row shows.
///
/// Two cases, and the split is the core's rule rather than a style choice:
///
/// - **priced** — the code and a sample formatted in it.
/// - **unpriced** (`rate: None`) — the code **alone**. Not the code beside a USD
///   figure wearing its symbol, which is the specific lie `rate: None` exists to
///   prevent: "a rate of 1 is a claim (1 USD = 1 CNY)". Showing `¥1,234.56` for
///   an unpriced JPY would assert an exchange rate nobody obtained.
#[must_use]
pub fn currency_row_value(view: &CurrencyView, locale: &str) -> SharedString {
    match view.rate {
        Some(rate) => {
            let sample = format_fiat(
                SAMPLE * rate,
                &view.code,
                symbol_for(&view.code),
                locale,
                crate::executor::format_prefs::fiat_options(),
            );
            SharedString::from(format!("{} · {sample}", view.code))
        }
        None => SharedString::from(view.code.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::core_host::CoreHost;
    use vela_core::app::network_admin::{Event as NetEvent, NetNetworkRow, NetworkAdmin};

    /// A real `NetView` with the rows substituted.
    ///
    /// Built by booting the actual core rather than hand-constructing one: the
    /// wizard, endpoint and provider sub-views have their own invariants, and a
    /// literal I typed would be a guess about them that drifts the first time
    /// they change.
    fn view_with(networks: Vec<NetNetworkRow>) -> NetView {
        let mut host = CoreHost::<NetworkAdmin>::new();
        let _ = host.dispatch(NetEvent::Started);
        NetView {
            loaded: true,
            networks,
            ..host.view()
        }
    }

    fn net_row(
        chain_id: u32,
        name: &str,
        custom: bool,
        health: Option<NetProbeHealth>,
    ) -> NetNetworkRow {
        NetNetworkRow {
            id: format!("chain-{chain_id}"),
            chain_id,
            display_name: name.to_owned(),
            native_symbol: "ETH".to_owned(),
            is_custom: custom,
            rpc_url: String::new(),
            explorer_url: String::new(),
            bundler_url: String::new(),
            rpc_health: health,
            explorer_health: None,
            rpc_chain_mismatch: None,
            rpc_save_deferred: false,
        }
    }

    /// A live Ethereum row is tinted by the same constant the mock is. The
    /// alternative — a second colour table in the live path — is how two
    /// renderings of one network start disagreeing.
    #[test]
    fn a_known_chain_borrows_the_mock_s_brand_colour() {
        let view = view_with(vec![net_row(1, "Ethereum", false, None)]);
        let rows = network_rows(&view);
        let fixture = crate::settings::fixtures::network("ethereum");
        assert_eq!(rows[0].color, fixture.color);
        assert_eq!(rows[0].letter, SharedString::from("E"));
    }

    /// A chain nobody drew gets the neutral, not an invented brand colour.
    #[test]
    fn an_undrawn_chain_gets_the_neutral_tint() {
        let view = view_with(vec![net_row(31_337, "Anvil", true, None)]);
        assert_eq!(network_rows(&view)[0].color, UNTINTED);
    }

    /// Three different core states draw no badge, and the row must not
    /// distinguish them by inventing a zero.
    #[test]
    fn only_a_completed_probe_draws_a_latency_badge() {
        let view = view_with(vec![
            net_row(
                1,
                "Ok",
                false,
                Some(NetProbeHealth::Ok { latency_ms: 182.4 }),
            ),
            net_row(2, "Checking", false, Some(NetProbeHealth::Checking)),
            net_row(3, "Errored", false, Some(NetProbeHealth::Error)),
            net_row(4, "Unprobed", false, None),
        ]);
        let rows = network_rows(&view);
        assert_eq!(rows[0].latency_ms, Some(182), "a measured probe rounds");
        assert_eq!(rows[1].latency_ms, None, "checking is not zero");
        assert_eq!(rows[2].latency_ms, None, "an error is not zero");
        assert_eq!(rows[3].latency_ms, None);
    }

    fn view(code: &str, rate: Option<f64>) -> CurrencyView {
        CurrencyView {
            code: code.to_owned(),
            rate,
            committed: true,
        }
    }

    /// A fresh wallet commits `{USD, 1}`, and the row must then read exactly
    /// what the DST3 mock draws — otherwise going live would silently redraw a
    /// reviewed screen.
    #[test]
    fn a_priced_currency_matches_the_mock() {
        assert_eq!(
            currency_row_value(&view("USD", Some(1.0)), "en"),
            SharedString::from("USD · $1,234.56")
        );
    }

    /// The lie this function exists not to tell. An unpriced JPY must not be
    /// rendered as ¥1,234.56 — that figure is USD, and dressing it in a yen
    /// symbol asserts a rate nobody obtained.
    #[test]
    fn an_unpriced_currency_shows_no_figure_at_all() {
        let value = currency_row_value(&view("JPY", None), "en");
        assert_eq!(value, SharedString::from("JPY"));
        assert!(
            !value.contains("1,234.56"),
            "an unpriced currency must not print a figure it cannot vouch for"
        );
    }

    /// A code with no glyph falls back to itself, and CLDR then separates it
    /// from the digits with a NON-BREAKING space (U+00A0) rather than a plain
    /// one — a currency symbol must not be left at the end of a wrapped line.
    /// Pinned as the literal character it is, because a plain space here would
    /// be a silently different string that only shows up as a bad line break.
    #[test]
    fn an_unknown_code_falls_back_to_itself_with_a_nonbreaking_space() {
        assert_eq!(
            currency_row_value(&view("CHF", Some(1.0)), "en"),
            SharedString::from("CHF · CHF\u{a0}1,234.56")
        );
    }
}

/// A neutral tint for a chain the mocks never drew.
///
/// `#8A8F98` — the design system's muted grey. The alternative is generating a
/// colour from the chain id, which produces a brand-looking colour nobody chose
/// for a network nobody designed.
const UNTINTED: u32 = 0x8A_8F_98;

/// The 网络 rows, from what the core loaded.
///
/// One service endpoint's badge: how it answered, in the words the mocks use.
///
/// `Checking` gets NO pill rather than a neutral one. A grey badge beside a
/// field reads as a verdict, and "we have not asked yet" is not one — the same
/// rule the balance hero applies to a figure it does not have.
#[must_use]
pub fn endpoint_badge(health: &NetServiceHealth, s: &SettingsStrings) -> Option<Pill> {
    match health {
        NetServiceHealth::Checking => None,
        NetServiceHealth::Ok { latency_ms, .. } => {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a measured latency"
            )]
            let ms = latency_ms.max(0.0) as u32;
            Some(crate::settings::fixtures::latency(
                ms,
                (ms >= 1000).then_some(s.network_slow.as_ref()),
            ))
        }
        // Not HTTPS is a REFUSAL to trust, not a slow answer, and it must not
        // wear the same colour as a working endpoint.
        NetServiceHealth::NotHttps => Some(crate::settings::fixtures::pill(
            Tone::Error,
            s.health_https_required.clone(),
        )),
        NetServiceHealth::Unreachable { http_status, .. } => {
            Some(crate::settings::fixtures::pill(
                Tone::Error,
                match http_status {
                    // "HTTP 502" and "Connection failed" are different problems
                    // and lead to different fixes.
                    Some(status) => SharedString::from(format!("HTTP {status}")),
                    None => s.health_offline.clone(),
                },
            ))
        }
        // Reachable, but not the service it must be. A WARNING, not an error:
        // the core does not gate saves on it, so the badge must not look like a
        // refusal.
        NetServiceHealth::InvalidResponse { .. } => Some(crate::settings::fixtures::pill(
            Tone::Warn,
            s.health_invalid.clone(),
        )),
    }
}

/// The tone the field's own border takes.
#[must_use]
pub fn endpoint_tone(health: &NetServiceHealth) -> Option<Tone> {
    match health {
        NetServiceHealth::NotHttps | NetServiceHealth::Unreachable { .. } => Some(Tone::Error),
        NetServiceHealth::Ok { .. } => Some(Tone::Ok),
        NetServiceHealth::Checking | NetServiceHealth::InvalidResponse { .. } => None,
    }
}

/// One override probe's badge — the same "no verdict yet, no badge" rule the
/// service endpoints follow.
#[must_use]
pub fn probe_badge(health: &NetProbeHealth) -> Option<Pill> {
    match health {
        NetProbeHealth::Ok { latency_ms } => {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "a measured latency"
            )]
            let ms = latency_ms.max(0.0) as u32;
            Some(crate::settings::fixtures::latency(ms, None))
        }
        _ => None,
    }
}

/// The wizard's compatibility rows, from what the probe found.
///
/// **An unreachable chain is not an incompatible one** — the core's invariant
/// ③, and the reason `rpc_failure` exists as a separate field. When the probe
/// could not reach a verdict this answers `None` and the caller draws the
/// retry, rather than four red crosses that condemn a chain nobody managed to
/// ask.
#[must_use]
pub fn compat_checks(
    compat: &NetCompatibility,
    s: &SettingsStrings,
) -> Option<Vec<(SharedString, bool)>> {
    if compat.rpc_failure.is_some() {
        return None;
    }
    let deployed = |name: &str| {
        compat
            .contracts
            .iter()
            .find(|contract| contract.name.contains(name))
            .is_some_and(|contract| contract.deployed)
    };
    let mut rows = vec![
        // A product name, not prose: translating it would make the row lie.
        (
            SharedString::from("EntryPoint v0.7"),
            deployed("EntryPoint"),
        ),
        (s.check_safe.clone(), deployed("Safe")),
    ];
    // `None` = never probed, which is not the same as "no P-256". The row is
    // left out rather than drawn as a failure.
    if let Some(available) = compat.p256_available {
        rows.push((s.check_signer.clone(), available));
    }
    let remaining = compat
        .contracts
        .iter()
        .filter(|contract| !contract.name.contains("EntryPoint") && !contract.name.contains("Safe"))
        .count();
    if remaining > 0 {
        let ok = compat
            .contracts
            .iter()
            .filter(|contract| {
                !contract.name.contains("EntryPoint") && !contract.name.contains("Safe")
            })
            .all(|contract| contract.deployed);
        rows.push((
            SharedString::from(crate::wallet::fill(
                &s.check_remaining,
                "count",
                &remaining.to_string(),
            )),
            ok,
        ));
    }
    Some(rows)
}

/// A provider's own name. Not translated: Alchemy is called Alchemy.
#[must_use]
pub fn provider_name(provider: NetProviderId) -> SharedString {
    SharedString::from(match provider {
        NetProviderId::Alchemy => "Alchemy",
        NetProviderId::Drpc => "dRPC",
        NetProviderId::Ankr => "Ankr",
    })
}

/// Where a provider's key is got — the web's `PROVIDER_KEY_URLS`.
#[must_use]
pub fn provider_key_url(provider: NetProviderId) -> &'static str {
    match provider {
        NetProviderId::Alchemy => "https://dashboard.alchemy.com/",
        NetProviderId::Drpc => "https://drpc.org/",
        NetProviderId::Ankr => "https://www.ankr.com/rpc/",
    }
}

/// How many networks this provider's key actually reached, once tested.
///
/// `None` until the test finishes. A count of zero out of zero, printed while
/// the test is still running, reads as "this key works nowhere" — which is a
/// verdict, and the test has not reached one.
#[must_use]
pub fn provider_support(
    provider: &vela_core::app::network_admin::NetProviderView,
    s: &SettingsStrings,
) -> Option<SharedString> {
    let test = provider.test.as_ref().filter(|test| test.done)?;
    let base = crate::wallet::fill(
        &crate::wallet::fill(&s.provider_supports, "count", &test.ok_count.to_string()),
        "total",
        &test.total.to_string(),
    );
    // The average of the ones that ANSWERED. Folding a failed network in as
    // zero would make a half-broken key look fast.
    let answered: Vec<f64> = test
        .results
        .iter()
        .filter(|row| row.ok)
        .map(|row| row.latency_ms)
        .collect();
    if answered.is_empty() {
        return Some(SharedString::from(base));
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "a mean latency, for display"
    )]
    let mean = (answered.iter().sum::<f64>() / answered.len() as f64).max(0.0) as u32;
    Some(SharedString::from(format!(
        "{base} · {}",
        crate::wallet::fill(&s.provider_avg_latency, "ms", &mean.to_string())
    )))
}

/// The core owns which networks exist, in what order, and whether each is
/// custom. This adds only the two things it has no business knowing — a
/// lettermark and a tint — and flattens the probe health to "is there a number".
#[must_use]
pub fn network_rows(view: &NetView) -> Vec<NetworkRowModel> {
    view.networks
        .iter()
        .map(|row| NetworkRowModel {
            id: SharedString::from(row.id.clone()),
            name: SharedString::from(row.display_name.clone()),
            letter: lettermark(&row.display_name),
            color: chain_tint(u64::from(row.chain_id)).unwrap_or(UNTINTED),
            chain_id: u64::from(row.chain_id),
            // A badge is drawn only for a probe that came back with a figure.
            // `Checking` and `Error` are states the core distinguishes and the
            // row draws without a number, exactly as the mock does.
            latency_ms: match row.rpc_health {
                Some(NetProbeHealth::Ok { latency_ms }) => {
                    Some(latency_ms.round().clamp(0.0, f64::from(u32::MAX)) as u32)
                }
                _ => None,
            },
            custom: row.is_custom,
        })
        .collect()
}

/// The line under a network's RPC field, in the order the core's three states
/// deserve.
///
/// A refusal outranks everything: nothing was written and the person is owed
/// the reason. Then the **pending** verdict — because the standing hint says
/// "saved as soon as you leave the field", and while `rpc_save_deferred` is
/// true that sentence is not yet true. Leaving it up is the small version of
/// lesson 3: a screen claiming a write landed before anyone checked.
#[must_use]
pub fn override_hint(row: &NetNetworkRow, s: &SettingsStrings) -> SharedString {
    if let Some(mismatch) = &row.rpc_chain_mismatch {
        return SharedString::from(crate::wallet::fill(
            &crate::wallet::fill(
                &s.rpc_wrong_chain,
                "actual",
                &mismatch.reported_chain_id.to_string(),
            ),
            "expected",
            &mismatch.expected_chain_id.to_string(),
        ));
    }
    if row.rpc_save_deferred {
        return s.network_save_checking.clone();
    }
    s.network_save_hint.clone()
}

/// The one line the add-network dialog owes the person: what the wizard is
/// doing, or why it stopped.
///
/// `Progress` is a wait on something outside the app (the chain index, the RPC
/// probes) and draws a spinner; `Refusal` is a verdict and draws in the error
/// tint. `Idle`, `Suggested` and `Checked` return **nothing**, because in those
/// three the dialog already speaks — the suggestion list, the check list, the
/// CTA.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WizardNotice {
    Progress(SharedString),
    Refusal(SharedString),
}

/// What the core decided the wizard is, in words.
///
/// Every one of these already existed in the corpus: the scan path and the
/// add-token screen refuse in the same four ways (`NetWizardErrorKind` serves
/// both callers — the core's invariant ①), so this adds no new key. The
/// alternative — a silent dialog — is the phase 6 bug in another screen:
/// **the core computed a refusal and the screen kept it to itself.**
#[must_use]
pub fn wizard_notice(view: &NetWizardView, s: &SettingsStrings) -> Option<WizardNotice> {
    match view.phase {
        NetWizardPhase::Idle | NetWizardPhase::Suggested | NetWizardPhase::Checked => None,
        NetWizardPhase::Searching => Some(WizardNotice::Progress(s.wizard_searching.clone())),
        // Resolving the chain and probing its endpoints are one wait as far as
        // the person is concerned; the core keeps them apart for its own
        // generation rules, not for the screen.
        NetWizardPhase::Resolving | NetWizardPhase::Checking => {
            Some(WizardNotice::Progress(s.wizard_checking.clone()))
        }
        NetWizardPhase::Error => view.error.as_ref().map(|kind| {
            WizardNotice::Refusal(match kind {
                NetWizardErrorKind::AlreadyAdded { .. } => s.wizard_already_added.clone(),
                NetWizardErrorKind::NotFound { .. } => s.wizard_not_found.clone(),
                // The chain resolved — its name is the honest subject of the
                // sentence. The query is the fallback for a state the core
                // does not produce today (`chain_info` is set before this
                // error is raised), never a guess dressed as a chain name.
                NetWizardErrorKind::NoRpcEndpoint => {
                    let name = view
                        .chain_info
                        .as_ref()
                        .map_or_else(|| view.query.clone(), |info| info.name.clone());
                    SharedString::from(crate::wallet::fill(&s.wizard_no_rpc, "name", &name))
                }
                NetWizardErrorKind::NotCompatible { .. } => s.wizard_incompatible.clone(),
                // Not a verdict: the probes never reached the chain (spec 038 #E1).
                NetWizardErrorKind::CheckFailed { .. } => s.wizard_unable_to_verify.clone(),
            })
        }),
    }
}

#[cfg(test)]
mod wizard_tests {
    use super::*;

    use crate::core_host::{CoreHost, Pending};
    use vela_core::app::network_admin::{
        Event as NetEvent, NetOperation, NetProviderKeys, NetRawChainData, NetShellResult,
        NetStoredEndpoints, NetworkAdmin,
    };

    fn strings() -> SettingsStrings {
        SettingsStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// A core whose ledger is in.
    ///
    /// Not optional decoration: `select_chain` FAILS CLOSED until the store
    /// has loaded (acting before it could add a duplicate), so a wizard test
    /// on an unloaded core silently exercises nothing.
    fn loaded_host() -> CoreHost<NetworkAdmin> {
        let mut host = CoreHost::<NetworkAdmin>::new();
        let pending = host.dispatch(NetEvent::Started);
        for Pending { id, operation } in pending {
            if matches!(operation, NetOperation::ReadStore) {
                host.resolve(
                    id,
                    NetShellResult::StoreLoaded {
                        custom_networks: Vec::new(),
                        network_configs: Vec::new(),
                        endpoints: NetStoredEndpoints::default(),
                        provider_keys: NetProviderKeys::default(),
                    },
                );
            }
        }
        host
    }

    /// Nothing has happened yet, so there is nothing to say. A notice here
    /// would be noise in an empty dialog.
    #[test]
    fn an_untouched_wizard_says_nothing() {
        let host = loaded_host();
        assert_eq!(wizard_notice(&host.view().wizard, &strings()), None);
    }

    /// Typing arms a search that runs on a debounce and then a network call.
    /// Until phase 6b's rule reached this dialog it drew a still list and no
    /// word about the query being worked on.
    #[test]
    fn a_running_search_says_it_is_running() {
        let mut host = loaded_host();
        host.dispatch(NetEvent::SearchInput {
            query: "gnosis".to_owned(),
        });
        let s = strings();
        assert_eq!(
            wizard_notice(&host.view().wizard, &s),
            Some(WizardNotice::Progress(s.wizard_searching.clone()))
        );
    }

    /// Picking a chain starts a resolve and then a probe race — seconds of
    /// waiting, previously spent staring at an unchanged dialog.
    #[test]
    fn a_chain_being_checked_says_it_is_being_checked() {
        let mut host = loaded_host();
        host.dispatch(NetEvent::ChainSelected {
            chain_id: 7_777_777,
            keep_custom_rpc: false,
        });
        let s = strings();
        assert_eq!(
            wizard_notice(&host.view().wizard, &s),
            Some(WizardNotice::Progress(s.wizard_checking.clone()))
        );
    }

    /// The refusal the person is most likely to meet: they pick a chain the
    /// wallet already has. The core stops the wizard dead (invariant ①) and
    /// draws no CTA — so without this line the dialog just stops responding.
    #[test]
    fn a_chain_already_added_says_so() {
        let mut host = loaded_host();
        // Chain 1 is built in, which is the dedup gate's other half.
        host.dispatch(NetEvent::ChainSelected {
            chain_id: 1,
            keep_custom_rpc: false,
        });
        let s = strings();
        assert_eq!(
            wizard_notice(&host.view().wizard, &s),
            Some(WizardNotice::Refusal(s.wizard_already_added.clone()))
        );
    }

    /// The registry has no such chain.
    #[test]
    fn a_chain_the_registry_does_not_know_says_so() {
        let mut host = loaded_host();
        let pending = host.dispatch(NetEvent::ChainSelected {
            chain_id: 7_777_777,
            keep_custom_rpc: false,
        });
        for Pending { id, operation } in pending {
            if let NetOperation::FetchChainInfo { chain_id } = operation {
                host.resolve(
                    id,
                    NetShellResult::ChainInfo {
                        chain_id,
                        data: None,
                    },
                );
            }
        }
        let s = strings();
        assert_eq!(
            wizard_notice(&host.view().wizard, &s),
            Some(WizardNotice::Refusal(s.wizard_not_found.clone()))
        );
    }

    fn row(mismatch: Option<(u32, u32)>, deferred: bool) -> NetNetworkRow {
        use vela_core::app::network_admin::NetChainMismatch;
        NetNetworkRow {
            id: "chain-100".to_owned(),
            chain_id: 100,
            display_name: "Gnosis".to_owned(),
            native_symbol: "XDAI".to_owned(),
            is_custom: false,
            rpc_url: String::new(),
            explorer_url: String::new(),
            bundler_url: String::new(),
            rpc_health: None,
            explorer_health: None,
            rpc_chain_mismatch: mismatch.map(|(reported, expected)| NetChainMismatch {
                reported_chain_id: reported,
                expected_chain_id: expected,
            }),
            rpc_save_deferred: deferred,
        }
    }

    /// While the chain-id verdict is outstanding the override has NOT been
    /// written, so the standing "saved as soon as you leave the field" must
    /// stand down. It is the same defect as a receipt read off the wrong
    /// field, one size smaller: a write claimed before anyone checked.
    #[test]
    fn a_pending_verdict_does_not_claim_the_save_landed() {
        let s = strings();
        assert_eq!(override_hint(&row(None, false), &s), s.network_save_hint);
        assert_eq!(override_hint(&row(None, true), &s), s.network_save_checking);
    }

    /// A refusal outranks both: nothing was written, and the person watching
    /// nothing happen is owed the reason.
    #[test]
    fn a_refused_endpoint_says_which_chain_it_actually_serves() {
        let hint = override_hint(&row(Some((1, 100)), true), &strings());
        assert!(hint.contains('1') && hint.contains("100"), "{hint}");
    }

    /// The gate the dialog's CTA now trusts.
    ///
    /// `add_confirmed` refuses — silently, by design — unless the wizard is
    /// `Checked` and compatible. `last_added_chain_id` is the only outward
    /// sign that it did NOT refuse, which is why the shell closes the dialog
    /// on that field changing rather than on the press.
    #[test]
    fn an_unchecked_wizard_records_no_add() {
        let mut host = loaded_host();
        host.dispatch(NetEvent::ChainSelected {
            chain_id: 1,
            keep_custom_rpc: false,
        });
        host.dispatch(NetEvent::AddConfirmed {
            now_iso: "2026-09-07T00:00:00.000Z".to_owned(),
        });
        assert_eq!(host.view().last_added_chain_id, None);
    }

    /// A chain that resolved but lists no endpoint. The sentence names the
    /// chain, because by now the wizard knows which one it is — and the custom
    /// RPC field sitting under the notice is the way out.
    #[test]
    fn a_chain_with_no_endpoint_is_named_in_the_refusal() {
        let mut host = loaded_host();
        let pending = host.dispatch(NetEvent::ChainSelected {
            chain_id: 7_777_777,
            keep_custom_rpc: false,
        });
        for Pending { id, operation } in pending {
            if let NetOperation::FetchChainInfo { chain_id } = operation {
                host.resolve(
                    id,
                    NetShellResult::ChainInfo {
                        chain_id,
                        data: Some(NetRawChainData {
                            chain_id: Some(chain_id),
                            name: Some("Zora".to_owned()),
                            short_name: None,
                            native_currency_name: None,
                            native_currency_symbol: None,
                            native_currency_decimals: None,
                            rpc: Vec::new(),
                            explorers: Vec::new(),
                            testnet: false,
                        }),
                    },
                );
            }
        }
        let Some(WizardNotice::Refusal(body)) = wizard_notice(&host.view().wizard, &strings())
        else {
            unreachable!("a chain with no endpoint is refused")
        };
        assert!(
            body.contains("Zora"),
            "the refusal names the chain it refused: {body}"
        );
    }
}

#[cfg(test)]
mod endpoint_tests {
    use super::*;

    fn strings() -> SettingsStrings {
        SettingsStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// A probe that has not answered draws no latency badge, on the override
    /// card as on the service endpoints.
    #[test]
    fn an_override_probe_badges_only_a_measured_answer() {
        assert!(probe_badge(&NetProbeHealth::Checking).is_none());
        assert!(probe_badge(&NetProbeHealth::Error).is_none());
        let ok = probe_badge(&NetProbeHealth::Ok { latency_ms: 45.0 })
            .unwrap_or_else(|| unreachable!("a measured probe has a badge"));
        assert_eq!(ok.label, "45ms");
    }

    /// An unreachable chain gets a retry, never four red crosses.
    #[test]
    fn a_probe_that_could_not_ask_does_not_condemn_the_chain() {
        use vela_core::app::network_admin::{NetContractStatus, NetRpcFailureKind};
        let s = strings();
        let contract = |name: &str, deployed: bool| NetContractStatus {
            name: name.to_owned(),
            address: "0xaaa".to_owned(),
            deployed,
        };
        let compat =
            |rpc_failure: Option<NetRpcFailureKind>, p256: Option<bool>| NetCompatibility {
                chain_id: 7_777_777,
                compatible: rpc_failure.is_none(),
                contracts: vec![
                    contract("EntryPoint v0.7", true),
                    contract("Safe v1.4.1", true),
                    contract("SafeWebAuthnSharedSigner", true),
                ],
                p256_available: p256,
                best_rpc_url: None,
                best_rpc_latency_ms: None,
                rpc_failure,
            };

        // Could not reach a verdict: NO rows. The caller draws a retry, which
        // is the difference between "this chain does not work" and "we could
        // not ask" — the core's invariant ③.
        assert!(
            compat_checks(
                &compat(Some(NetRpcFailureKind::AllProbesFailed), Some(true)),
                &s
            )
            .is_none()
        );

        let rows = compat_checks(&compat(None, Some(true)), &s)
            .unwrap_or_else(|| unreachable!("a reached verdict has rows"));
        assert_eq!(rows[0].0, "EntryPoint v0.7", "a product name, not prose");
        assert!(rows.iter().all(|(_, ok)| *ok));
        assert!(rows.iter().any(|(label, _)| *label == s.check_signer));

        // Never probed for P-256 is not "no P-256": the row is left out rather
        // than drawn as a failure.
        let unprobed = compat_checks(&compat(None, None), &s)
            .unwrap_or_else(|| unreachable!("a reached verdict has rows"));
        assert!(!unprobed.iter().any(|(label, _)| *label == s.check_signer));
    }

    /// A key's support line waits for the test to finish, and averages only
    /// the networks that answered.
    #[test]
    fn a_support_line_waits_for_the_test_and_averages_what_answered() {
        use vela_core::app::network_admin::{
            NetProviderNetRow, NetProviderTestView, NetProviderView,
        };
        let s = strings();
        let provider = |test: Option<NetProviderTestView>| NetProviderView {
            provider: NetProviderId::Alchemy,
            key: "abc".to_owned(),
            has_key: true,
            test,
        };

        // Never tested, and mid-test: no line. "0 of 0" printed while a test is
        // running reads as "this key works nowhere", which is a verdict the
        // test has not reached.
        assert!(provider_support(&provider(None), &s).is_none());
        assert!(
            provider_support(
                &provider(Some(NetProviderTestView {
                    done: false,
                    results: Vec::new(),
                    ok_count: 0,
                    total: 3,
                })),
                &s
            )
            .is_none()
        );

        // Done: two of three, and the average of the two that ANSWERED — 50ms,
        // not 33ms. Folding the failure in as zero makes a half-broken key look
        // fast.
        let line = provider_support(
            &provider(Some(NetProviderTestView {
                done: true,
                results: vec![
                    NetProviderNetRow {
                        chain_id: 1,
                        ok: true,
                        latency_ms: 40.0,
                    },
                    NetProviderNetRow {
                        chain_id: 56,
                        ok: true,
                        latency_ms: 60.0,
                    },
                    NetProviderNetRow {
                        chain_id: 100,
                        ok: false,
                        latency_ms: 0.0,
                    },
                ],
                ok_count: 2,
                total: 3,
            })),
            &s,
        )
        .unwrap_or_else(|| unreachable!("a finished test has a line"));
        assert!(line.contains('2') && line.contains('3'), "{line}");
        assert!(line.contains("50"), "the mean of what answered: {line}");

        // Everything failed: the count, and no average at all.
        let none_ok = provider_support(
            &provider(Some(NetProviderTestView {
                done: true,
                results: vec![NetProviderNetRow {
                    chain_id: 1,
                    ok: false,
                    latency_ms: 0.0,
                }],
                ok_count: 0,
                total: 1,
            })),
            &s,
        )
        .unwrap_or_else(|| unreachable!("a finished test has a line"));
        assert!(!none_ok.contains('·'), "no average to state: {none_ok}");
    }

    /// Each health state gets its own badge — and "checking" gets none.
    #[test]
    fn a_probe_that_has_not_answered_wears_no_verdict() {
        let s = strings();

        // Not asked yet. A grey badge beside a field reads as a verdict, and
        // this is not one — the same rule the hero applies to a figure it does
        // not have.
        assert!(endpoint_badge(&NetServiceHealth::Checking, &s).is_none());
        assert!(endpoint_tone(&NetServiceHealth::Checking).is_none());

        let ok = endpoint_badge(
            &NetServiceHealth::Ok {
                latency_ms: 62.0,
                rate_count: None,
            },
            &s,
        )
        .unwrap_or_else(|| unreachable!("ok has a badge"));
        assert_eq!(ok.label, "62ms");
        assert!(matches!(ok.tone, Tone::Ok));

        // Over a second the pill says WHY it is amber.
        let slow = endpoint_badge(
            &NetServiceHealth::Ok {
                latency_ms: 1_200.0,
                rate_count: None,
            },
            &s,
        )
        .unwrap_or_else(|| unreachable!("ok has a badge"));
        assert!(matches!(slow.tone, Tone::Warn));
        assert!(slow.label.contains("1.2s"));

        // Not HTTPS is a refusal to trust, not a slow answer.
        let insecure = endpoint_badge(&NetServiceHealth::NotHttps, &s)
            .unwrap_or_else(|| unreachable!("not-https has a badge"));
        assert!(matches!(insecure.tone, Tone::Error));
        assert_eq!(insecure.label, s.health_https_required);

        // "HTTP 502" and "offline" are different problems with different fixes.
        let refused = endpoint_badge(
            &NetServiceHealth::Unreachable {
                http_status: Some(502),
                latency_ms: None,
            },
            &s,
        )
        .unwrap_or_else(|| unreachable!("unreachable has a badge"));
        assert_eq!(refused.label, "HTTP 502");
        let offline = endpoint_badge(
            &NetServiceHealth::Unreachable {
                http_status: None,
                latency_ms: None,
            },
            &s,
        )
        .unwrap_or_else(|| unreachable!("unreachable has a badge"));
        assert_eq!(offline.label, s.health_offline);

        // Reachable but not the right service: a WARNING, because the core does
        // not gate saves on it and the badge must not look like a refusal.
        let wrong = endpoint_badge(&NetServiceHealth::InvalidResponse { latency_ms: 40.0 }, &s)
            .unwrap_or_else(|| unreachable!("invalid has a badge"));
        assert!(matches!(wrong.tone, Tone::Warn));
        assert!(endpoint_tone(&NetServiceHealth::InvalidResponse { latency_ms: 40.0 }).is_none());
    }
}

// ---------------------------------------------------------------------------
// Localization — the number, date and clock rows (spec 038 #E3)
// ---------------------------------------------------------------------------

/// The sample every format row and menu prints — the web's `FORMAT_SAMPLE`,
/// 2026-06-13 13:45, so a row here and a row there show the same figure.
pub const FORMAT_SAMPLE: Civil = Civil {
    year: 2026,
    month: 6,
    day: 13,
    hour: 13,
    minute: 45,
    second: 0,
    weekday: 6,
};

/// The web's `1,234,567.89`.
const NUMBER_SAMPLE: f64 = 1_234_567.89;

fn number_example(preset: NumberPreset) -> SharedString {
    SharedString::from(format_token_amount(NUMBER_SAMPLE, preset, false))
}

fn date_example(preset: DatePreset) -> SharedString {
    SharedString::from(format_date(&FORMAT_SAMPLE, preset))
}

fn time_example(preset: TimePreset, locale: &str) -> SharedString {
    SharedString::from(format_time(&FORMAT_SAMPLE, preset, locale))
}

/// What the three rows show: the sample in the preset in force.
#[must_use]
pub fn format_row_values(
    formats: Formats,
    locale: &str,
) -> (SharedString, SharedString, SharedString) {
    (
        number_example(formats.number),
        date_example(formats.date),
        time_example(formats.time, locale),
    )
}

/// The three menus, each opening with "Automatic · System" — the machine's
/// own convention, printed as its example — and then the presets in the
/// order the web lists them. The row in force carries the tick.
pub struct FormatMenus {
    pub number: Vec<MenuRow>,
    pub date: Vec<MenuRow>,
    pub time: Vec<MenuRow>,
}

#[must_use]
pub fn format_menus(
    choice: Choice,
    machine: &Formats,
    locale: &str,
    auto_note: &SharedString,
    indian_note: &SharedString,
) -> FormatMenus {
    let mut number = vec![(
        number_example(machine.number),
        Some(auto_note.clone()),
        choice.number.is_none(),
    )];
    number.extend(format_prefs::NUMBER_OPTIONS.iter().map(|preset| {
        (
            number_example(*preset),
            (*preset == NumberPreset::Indian).then(|| indian_note.clone()),
            choice.number == Some(*preset),
        )
    }));
    let mut date = vec![(
        date_example(machine.date),
        Some(auto_note.clone()),
        choice.date.is_none(),
    )];
    date.extend(
        format_prefs::DATE_OPTIONS
            .iter()
            .map(|preset| (date_example(*preset), None, choice.date == Some(*preset))),
    );
    let mut time = vec![(
        time_example(machine.time, locale),
        Some(auto_note.clone()),
        choice.time.is_none(),
    )];
    time.extend(format_prefs::TIME_OPTIONS.iter().map(|preset| {
        (
            time_example(*preset, locale),
            None,
            choice.time == Some(*preset),
        )
    }));
    FormatMenus { number, date, time }
}

/// A menu index back to a choice: row 0 is "Automatic", the rest the presets
/// in the order [`format_menus`] laid them.
#[must_use]
pub fn picked_number(index: usize) -> Option<NumberPreset> {
    index
        .checked_sub(1)
        .and_then(|i| format_prefs::NUMBER_OPTIONS.get(i).copied())
}

#[must_use]
pub fn picked_date(index: usize) -> Option<DatePreset> {
    index
        .checked_sub(1)
        .and_then(|i| format_prefs::DATE_OPTIONS.get(i).copied())
}

#[must_use]
pub fn picked_time(index: usize) -> Option<TimePreset> {
    index
        .checked_sub(1)
        .and_then(|i| format_prefs::TIME_OPTIONS.get(i).copied())
}

#[cfg(test)]
mod format_tests {
    use super::*;

    fn menus(choice: Choice) -> FormatMenus {
        format_menus(
            choice,
            &format_prefs::auto("en-US"),
            "en",
            &SharedString::from("Automatic · System"),
            &SharedString::from("Indian grouping"),
        )
    }

    #[test]
    fn the_rows_print_the_web_s_sample_in_the_preset_in_force() {
        let (number, date, time) = format_row_values(
            Formats {
                number: NumberPreset::DotComma,
                date: DatePreset::DmyDot,
                time: TimePreset::H12,
            },
            "en",
        );
        assert_eq!(number.as_ref(), "1.234.567,89");
        assert_eq!(date.as_ref(), "13.06.2026");
        assert_eq!(time.as_ref(), "1:45 PM");
    }

    #[test]
    fn a_fresh_wallet_ticks_automatic_and_shows_the_machine_s_example() {
        let m = menus(Choice::default());
        assert_eq!(m.number[0].0.as_ref(), "1,234,567.89");
        assert_eq!(m.number[0].1.as_deref(), Some("Automatic · System"));
        assert!(m.number[0].2);
        assert!(m.number[1..].iter().all(|row| !row.2));
        assert_eq!(m.date[0].0.as_ref(), "06/13/2026");
        assert!(m.date[0].2);
        assert_eq!(m.time[0].0.as_ref(), "1:45 PM");
        assert!(m.time[0].2);
        // The five date presets, the two clocks, the four groupings.
        assert_eq!(m.date.len(), 6);
        assert_eq!(m.time.len(), 3);
        assert_eq!(m.number.len(), 5);
        assert_eq!(m.number[4].1.as_deref(), Some("Indian grouping"));
    }

    #[test]
    fn a_choice_moves_the_tick_off_automatic() {
        let m = menus(Choice {
            number: None,
            date: Some(DatePreset::DmyDot),
            time: Some(TimePreset::H24),
        });
        assert!(!m.date[0].2);
        let ticked: Vec<&str> = m
            .date
            .iter()
            .filter(|row| row.2)
            .map(|row| row.0.as_ref())
            .collect();
        assert_eq!(ticked, ["13.06.2026"]);
        let ticked: Vec<&str> = m
            .time
            .iter()
            .filter(|row| row.2)
            .map(|row| row.0.as_ref())
            .collect();
        assert_eq!(ticked, ["13:45"]);
        assert!(m.number[0].2);
    }

    #[test]
    fn a_menu_index_names_the_preset_the_row_printed() {
        assert_eq!(picked_date(0), None);
        assert_eq!(picked_date(4), Some(DatePreset::DmyDot));
        assert_eq!(picked_date(9), None);
        assert_eq!(picked_time(2), Some(TimePreset::H12));
        assert_eq!(picked_number(3), Some(NumberPreset::SpaceComma));
    }
}

#[cfg(test)]
mod parity_tests {
    use super::*;

    use crate::core_host::{CoreHost, Pending};
    use crate::executor::device_storage::measure_entries;
    use vela_core::app::network_admin::{
        NetOperation, NetProviderKeys, NetShellResult, NetStoredEndpoints, NetworkAdmin,
    };

    fn strings() -> SettingsStrings {
        SettingsStrings::resolve(&crate::loc::Loc::from_env())
    }

    fn entries(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(key, raw)| ((*key).to_owned(), (*raw).to_owned()))
            .collect()
    }

    /// The storage page's rows are the catalog's, in its order and groups: the
    /// four your-data rows in red, the three caches plain with "clear all"
    /// under them, each meta line in its own unit.
    #[test]
    fn the_storage_rows_are_the_catalogs() {
        let s = strings();
        let report = measure_entries(&entries(&[
            ("vela.transactionHistory", r#"[{"a":1},{"a":2}]"#),
            ("vela.contacts", r#"[{"a":1}]"#),
            ("vela.balanceCache", r#"{"x":1}"#),
        ]));
        let groups = storage_groups(&report, &s, NumberPreset::CommaDot);
        assert_eq!(groups.len(), 2, "connections are the browser machine's");
        let ids = |group: &StorageGroup| group.items.iter().map(|item| item.id).collect::<Vec<_>>();
        assert_eq!(
            ids(&groups[0]),
            ["transactions", "contacts", "custom", "browsing"]
        );
        assert_eq!(ids(&groups[1]), ["balances", "rates", "scan"]);
        assert!(groups[0].items.iter().all(|item| item.destructive));
        assert!(groups[1].items.iter().all(|item| !item.destructive));
        assert_eq!(groups[0].label, s.storage_user_data);
        assert_eq!(groups[1].action.as_ref(), Some(&s.storage_clear_all));
        assert_eq!(groups[0].items[0].label, s.item_transactions);

        let transactions = &groups[0].items[0].meta;
        assert!(transactions.contains('2'), "two records: {transactions}");
        assert!(transactions.ends_with(" B"), "{transactions}");
        // A cache row states its size and nothing else; an empty row says 0.
        assert_eq!(groups[1].items[2].meta.as_ref(), "0 B");
    }

    /// Sizes are 1024s — what the operating system says beside them — and the
    /// figure is in the person's own number format.
    #[test]
    fn a_size_is_in_1024s_and_the_persons_number_format() {
        let text = |bytes, preset| {
            let (amount, unit) = bytes_text(bytes, preset);
            format!("{amount} {unit}")
        };
        assert_eq!(text(512, NumberPreset::CommaDot), "512 B");
        assert_eq!(text(0, NumberPreset::CommaDot), "0 B");
        assert_eq!(text(1536, NumberPreset::CommaDot), "1.5 KB");
        assert_eq!(text(1536, NumberPreset::DotComma), "1,5 KB");
        assert_eq!(text(3 * 1024 * 1024 / 2, NumberPreset::CommaDot), "1.5 MB");
    }

    /// The bar is the measured shares, not the mock's 50/30/20.
    #[test]
    fn the_bar_is_the_measured_shares() {
        let report = measure_entries(&entries(&[
            ("vela.contacts", "xxxxxxxx"),
            ("vela.balanceCache", "xxxxxxxx"),
        ]));
        let [user, cache, sessions] = storage_segments(&report);
        assert!(user.0 > 0. && cache.0 > 0.);
        assert!((user.0 + cache.0 - 1.).abs() < 1e-6);
        assert_eq!(sessions.0, 0.);
        let empty = storage_segments(&measure_entries(&[]));
        assert!(
            empty.iter().all(|(share, _)| *share == 0.),
            "nothing to share"
        );
    }

    /// The language menu: follow the system first, naming what it resolves to,
    /// then every shipped locale by its own name — and a pick is the stored
    /// word, `auto` or a supported tag.
    #[test]
    fn the_language_menu_is_follow_system_then_every_locale() {
        let s = strings();
        let rows = language_menu(None, "de", &s);
        assert_eq!(rows.len(), 1 + vela_core::i18n::SUPPORTED.len());
        assert_eq!(rows[0].0, s.language_follow_system);
        assert!(
            rows[0]
                .1
                .as_ref()
                .is_some_and(|note| note.contains("Deutsch"))
        );
        assert!(rows[0].2, "nothing pinned: following the system is ticked");
        let ids: Vec<&str> = crate::settings::fixtures::LOCALE_ENDONYMS
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            ids,
            vela_core::i18n::SUPPORTED,
            "every shipped locale, in order"
        );

        let pinned = language_menu(Some("ja"), "de", &s);
        assert!(!pinned[0].2);
        let ticked: Vec<&str> = pinned
            .iter()
            .filter(|row| row.2)
            .map(|row| row.0.as_ref())
            .collect();
        assert_eq!(ticked, ["日本語"]);

        assert_eq!(picked_language(0), Some("auto"));
        assert_eq!(picked_language(2), Some("zh"));
        assert_eq!(picked_language(99), None);
        assert_eq!(language_value(Some("fr"), "de", &s).as_ref(), "Français");
        assert!(language_value(None, "de", &s).starts_with("Deutsch · "));
    }

    fn currency(code: &str, rate: Option<f64>) -> CurrencyView {
        CurrencyView {
            code: code.to_owned(),
            rate,
            committed: true,
        }
    }

    /// An account's total is in the display currency when the core priced it,
    /// and in USD when it could not — `None` is not a rate of 1.
    #[test]
    fn an_account_total_is_in_the_display_currency_only_when_priced() {
        assert_eq!(
            account_total(10.0, Some(&currency("EUR", Some(0.5))), "en").as_ref(),
            "€5.00"
        );
        let unpriced = account_total(10.0, Some(&currency("JPY", None)), "en");
        assert_eq!(unpriced.as_ref(), "$10.00", "no rate, no conversion");
        assert!(!unpriced.contains('¥'));
    }

    /// The picker ticks the committed code, and a committed code outside the
    /// eight is still on the list that shows it.
    #[test]
    fn the_currency_menu_ticks_the_committed_code_and_keeps_it() {
        let (rows, codes) = currency_menu(&currency("JPY", Some(150.0)));
        assert_eq!(codes.len(), crate::settings::fixtures::CURRENCY_CODES.len());
        let ticked: Vec<&str> = rows
            .iter()
            .filter(|row| row.2)
            .map(|row| row.0.as_ref())
            .collect();
        assert_eq!(ticked, ["JPY"]);
        let (rows, codes) = currency_menu(&currency("CHF", None));
        assert_eq!(codes.last().map(String::as_str), Some("CHF"));
        assert!(rows.last().is_some_and(|row| row.2));
    }

    /// The stored words and the drawn cells agree.
    #[test]
    fn the_theme_and_avatar_cells_are_the_stored_words() {
        assert_eq!(segment_of(&THEME_SEGMENTS, "system"), 2);
        assert_eq!(segment_of(&THEME_SEGMENTS, "light"), 0);
        assert_eq!(segment_of(&AVATAR_SEGMENTS, "identicon"), 1);
        for word in THEME_SEGMENTS {
            assert!(vela_core::prefs::THEMES.contains(&word));
        }
        assert_eq!(text_scale_index("standard"), 2);
        assert_eq!(text_scale_index("xlarge"), 5);
    }

    /// A network-admin core with its store read, and the operations each
    /// event asks the shell for.
    fn loaded() -> CoreHost<NetworkAdmin> {
        let mut host = CoreHost::<NetworkAdmin>::new();
        for Pending { id, operation } in host.dispatch(NetEvent::Started) {
            if matches!(operation, NetOperation::ReadStore) {
                host.resolve(
                    id,
                    NetShellResult::StoreLoaded {
                        custom_networks: Vec::new(),
                        network_configs: Vec::new(),
                        endpoints: NetStoredEndpoints::default(),
                        provider_keys: NetProviderKeys::default(),
                    },
                );
            }
        }
        host
    }

    fn writes(pending: &[Pending<NetOperation>]) -> usize {
        pending
            .iter()
            .filter(|next| {
                matches!(
                    next.operation,
                    NetOperation::WriteServiceEndpoints { .. }
                        | NetOperation::WriteRpcProviders { .. }
                        | NetOperation::WriteNetworkConfigs { .. }
                )
            })
            .count()
    }

    /// Typing saves nothing; leaving the field (or Enter) saves once — for an
    /// endpoint and a provider key alike. Before 072 every keystroke also
    /// sent the blur, so each prefix of a URL was persisted on the way.
    #[test]
    fn a_keystroke_saves_nothing_and_leaving_the_field_saves() {
        let mut host = loaded();
        for field in [
            FieldCommit::Endpoint(NetEndpointField::FiatRates),
            FieldCommit::ProviderKey(NetProviderId::Alchemy),
        ] {
            for typed in ["h", "ht", "https://rates.example"] {
                let pending = host.dispatch(field.edited(typed.to_owned()));
                assert_eq!(writes(&pending), 0, "{field:?} saved on a keystroke");
            }
            let pending = host.dispatch(field.committed());
            assert_eq!(writes(&pending), 1, "{field:?} not saved on leaving");
        }
    }

    /// An override is where it mattered most: the chain-id refusal ran
    /// against half a URL. Typing now only drafts and probes — no verdict, no
    /// write — and leaving the field is what asks the gate.
    #[test]
    fn an_override_is_judged_when_left_not_while_typed() {
        let mut host = loaded();
        let chain_id = 100;
        let _ = host.dispatch(NetEvent::OverrideExpanded { chain_id });
        let field = FieldCommit::Override {
            chain_id,
            field: NetOverrideField::Rpc,
        };
        let row = |host: &CoreHost<NetworkAdmin>| {
            host.view()
                .networks
                .into_iter()
                .find(|row| row.chain_id == chain_id)
                .unwrap_or_else(|| unreachable!("Gnosis is built in"))
        };
        for typed in ["https://ma", "https://mainnet.example"] {
            let pending = host.dispatch(field.edited(typed.to_owned()));
            assert_eq!(writes(&pending), 0);
            let typing = row(&host);
            assert!(typing.rpc_chain_mismatch.is_none());
            assert!(!typing.rpc_save_deferred, "no save is pending while typing");
        }
        let _ = host.dispatch(field.committed());
        assert!(
            row(&host).rpc_save_deferred,
            "leaving the field asks for the save, held for the chain-id verdict"
        );
    }
}
