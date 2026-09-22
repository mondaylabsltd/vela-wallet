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
    NetCompatibility, NetNetworkRow, NetProbeHealth, NetProviderId, NetServiceHealth, NetView,
    NetWizardErrorKind, NetWizardPhase, NetWizardView,
};
use vela_core::l10n::currency::format_fiat;
use vela_core::l10n::datetime::{Civil, DatePreset, TimePreset, format_date, format_time};
use vela_core::l10n::number::{NumberPreset, format_token_amount};

use crate::executor::format_prefs::{self, Choice, Formats};
use crate::settings::components::MenuRow;

/// The sample figure the 本地化 mock prints beside the currency code.
const SAMPLE: f64 = 1234.56;

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
    // The multi-key pair is reported on its own line (`single_key_only`), not
    // folded into these rows: a chain that is missing only those is a working
    // chain for a one-key wallet, and a red "Safe contracts" row would say the
    // opposite.
    let core_contracts = || compat.contracts.iter().filter(|c| !c.multi_key_only);
    let deployed = |name: &str| {
        core_contracts()
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
    let others =
        || core_contracts().filter(|c| !c.name.contains("EntryPoint") && !c.name.contains("Safe"));
    // The multi-key pair gets its OWN row rather than being folded into the
    // aggregates above — the callout beside the list says what it means, but a
    // list in which nothing is crossed while a warning sits under it reads as
    // a warning about nothing (measured on the live dialog, spec 081 FR-009).
    // A product name, like EntryPoint's: translating it would make the row lie.
    let multi_key: Vec<_> = compat
        .contracts
        .iter()
        .filter(|c| c.multi_key_only)
        .collect();
    if !multi_key.is_empty() {
        rows.push((
            SharedString::from("Safe Passkey Signer"),
            multi_key.iter().all(|contract| contract.deployed),
        ));
    }
    let remaining = others().count();
    if remaining > 0 {
        let ok = others().all(|contract| contract.deployed);
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
            multi_key_only: false,
        };
        let compat =
            |rpc_failure: Option<NetRpcFailureKind>, p256: Option<bool>| NetCompatibility {
                chain_id: 7_777_777,
                compatible: rpc_failure.is_none(),
                multi_key_ready: rpc_failure.is_none(),
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

    /// Spec 081 FR-009. The two passkey-signer contracts belong to a wallet
    /// with more than one key. When they are the only thing missing, the rows
    /// above must stay green — folding them into "Safe contracts" would tell
    /// a one-key owner their chain is broken when it is not.
    #[test]
    fn the_multi_key_contracts_do_not_redden_the_rows_a_one_key_wallet_needs() {
        use vela_core::app::network_admin::NetContractStatus;
        let s = strings();
        let contract = |name: &str, deployed: bool, multi_key_only: bool| NetContractStatus {
            name: name.to_owned(),
            address: "0xaaa".to_owned(),
            deployed,
            multi_key_only,
        };
        let compat = NetCompatibility {
            chain_id: 7_777_777,
            compatible: true,
            multi_key_ready: false,
            contracts: vec![
                contract("EntryPoint v0.7", true, false),
                contract("Safe L2", true, false),
                contract("MultiSend", true, false),
                contract("Safe Passkey Signer Factory", false, true),
                contract("Safe Passkey Signer Singleton", false, true),
            ],
            p256_available: Some(true),
            best_rpc_url: None,
            best_rpc_latency_ms: None,
            rpc_failure: None,
        };

        let rows = compat_checks(&compat, &s)
            .unwrap_or_else(|| unreachable!("a reached verdict has rows"));
        let multi_key_row = rows
            .iter()
            .find(|(label, _)| label.contains("Passkey"))
            .unwrap_or_else(|| unreachable!("the multi-key pair has its own row"));
        assert!(!multi_key_row.1, "and it is the one that is crossed");
        assert!(
            rows.iter()
                .filter(|(label, _)| !label.contains("Passkey"))
                .all(|(_, ok)| *ok),
            "every row a one-key wallet depends on is satisfied: {rows:?}"
        );
        // And the count in the "N more" row counts only those contracts.
        assert!(
            rows.iter()
                .any(|(label, _)| label.contains('1') && !label.contains('3')),
            "one remaining contract, not three: {rows:?}"
        );
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
