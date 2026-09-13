//! The only place the `display_currency` machine touches the outside world.
//!
//! Four operations, and the interesting one is the one that answers `None`.
//!
//! ## `resolve_rate` answers `None`, and `None` is not `1`
//!
//! There is no rate source on the desktop until spec 031 wires the RPC pool, so
//! this cut answers "cannot price it right now". The temptation is to answer `1`
//! and move on; the core's own doc explains why that is a defect rather than a
//! default: *"a rate of 1 is a claim (1 USD = 1 CNY)"*, and a fiat-denominated
//! amount multiplied by a defaulted 1 is a real mispayment. The shell's job is
//! to report what it observed. Degrading the display is the core's decision, and
//! it already knows how to make it.
//!
//! ## `read_device_currency` answers `None` too, and that one is a choice
//!
//! Unlike the web — whose core comment says "None on web", because a browser has
//! no region currency — a desktop **does** have one: `Loc::from_env` already
//! resolves `LC_ALL`/`LANG`, and `zh_CN` carries a region. Answering it would be
//! easy and would be wrong today. The core persists a seeded currency **only
//! after a real rate resolves**, precisely because "a seeded currency rendering
//! at the rate-1 fallback (₫78 instead of ₫2,000,000) is strictly worse than
//! staying on USD". With no rate source in this cut, seeding buys a label that
//! could not commit and reset on the next launch. **Both went live together in
//! 031**, which is why they were one debt: a region candidate is only useful
//! once something can price it.

use gpui::App;

use vela_core::app::display_currency::{
    CurrencyOperation, CurrencyShellResult, DisplayCurrency, Event,
};

use crate::executor::{proxy, storage};
use crate::resident::{Answer, Machine};

/// The device region's currency, from the primary locale.
///
/// Deliberately a small table over the locales the app ships rather than a CLDR
/// import: the core only needs a *candidate*, and it will not commit one until a
/// rate resolves, so a wrong guess costs a fetch rather than a wrong figure. A
/// locale with no region — `en`, `zh` — has no region currency, and saying
/// `None` is more honest than assuming a country.
fn region_currency(language: &str) -> Option<String> {
    let region = language.split(['-', '_']).nth(1)?.to_ascii_uppercase();
    let code = match region.as_str() {
        "US" => "USD",
        "CN" => "CNY",
        "TW" => "TWD",
        "HK" => "HKD",
        "JP" => "JPY",
        "KR" => "KRW",
        "VN" => "VND",
        "ID" => "IDR",
        "TR" => "TRY",
        "RU" => "RUB",
        "MX" => "MXN",
        "BR" => "BRL",
        "GB" => "GBP",
        "IN" => "INR",
        "CH" => "CHF",
        "CA" => "CAD",
        "AU" => "AUD",
        // The euro members among the shipped locales.
        "DE" | "FR" | "IT" | "ES" | "NL" | "PT" | "IE" | "AT" | "BE" | "FI" | "GR" => "EUR",
        _ => return None,
    };
    Some(code.to_owned())
}

/// USD → `code`, or `None` when nothing can price it right now.
///
/// One source in this cut: the configured fiat-rate endpoint, whose body is a
/// list of `{ base, quote, rate }`. Web's waterfall tries a Chainlink feed
/// first; that needs an on-chain read and a feed registry, and it is additive —
/// the core's contract is a rate or `None`, not a provenance.
///
/// A non-positive rate is treated as no rate. A "rate" of zero prices every
/// balance at nothing, which is a wrong answer wearing the shape of a right one.
pub(crate) fn resolve_rate(code: &str) -> Option<f64> {
    if code == "USD" {
        return Some(1.0);
    }
    let url = fiat_rates_url();
    let mut response = proxy::agent(std::time::Duration::from_secs(8))
        .get(&url)
        .call()
        .ok()?;
    let mut body = String::new();
    std::io::Read::read_to_string(&mut response.body_mut().as_reader(), &mut body).ok()?;
    let parsed: serde_json::Value = serde_json::from_str(&body).ok()?;
    let rows = parsed.as_array()?;
    rows.iter()
        .find(|row| row.get("quote").and_then(serde_json::Value::as_str) == Some(code))
        .and_then(|row| row.get("rate"))
        .and_then(serde_json::Value::as_f64)
        .filter(|rate| *rate > 0.0)
}

/// The configured endpoint, or the built-in default.
fn fiat_rates_url() -> String {
    storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("fiatRatesURL")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| vela_core::app::network_admin::DEFAULT_FIAT_RATES_URL.to_owned())
}

impl Machine for DisplayCurrency {
    const LABEL: &'static str = "display_currency";

    fn boot_event(_cx: &App) -> Event {
        Event::Refresh
    }

    fn perform(operation: &CurrencyOperation) -> Answer<CurrencyShellResult, Self::Event> {
        match operation {
            // Absent ALWAYS means "the user never chose" — including when the
            // read failed, which is why this cannot surface an error.
            CurrencyOperation::ReadStoredCode => Answer::Now(CurrencyShellResult::StoredCode {
                code: storage::read_value(storage::KEY_DISPLAY_CURRENCY)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(|value| value.as_str())
                    .map(str::to_owned),
            }),

            // Best effort, exactly as every other client's `setCurrency` is: the
            // in-memory choice stays authoritative for this session either way.
            CurrencyOperation::WriteStoredCode { code } => {
                let _ = storage::write_value(
                    storage::KEY_DISPLAY_CURRENCY,
                    serde_json::Value::String(code.clone()),
                );
                Answer::Now(CurrencyShellResult::CodeWritten)
            }

            // Live since 031. A desktop HAS a region — `Loc::from_env` resolves
            // `LC_ALL`/`LANG` — unlike the browser the core's comment was
            // written about. It could not be answered in 030 for a different
            // reason: the core persists a seeded currency only after a real
            // rate resolves, so without a rate source a seed bought a label
            // that could not commit. Both halves land together, which is why
            // they were one debt.
            CurrencyOperation::ReadDeviceCurrency => {
                Answer::Now(CurrencyShellResult::DeviceCurrency {
                    code: region_currency(crate::loc::Loc::from_env().language()),
                })
            }

            // Live since 031. `None` remains an OBSERVATION — "no source could
            // price it right now" — and never a fallback to 1.
            CurrencyOperation::ResolveRate { code } => {
                let code = code.clone();
                Answer::Blocking(Box::new(move || CurrencyShellResult::RateResolved {
                    rate: resolve_rate(&code),
                    code,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::display_currency::CurrencyView;

    fn perform(operation: CurrencyOperation) -> CurrencyShellResult {
        match DisplayCurrency::perform(&operation) {
            Answer::Now(result) => result,
            _ => unreachable!("every currency operation is local"),
        }
    }

    /// Drive the whole loop the way the app does — dispatch, perform, resolve,
    /// until quiescent — without a window.
    ///
    /// `resident.rs` adds gpui's scheduling on top of this; the DECISIONS being
    /// exercised here are the same ones, so a rule that breaks breaks here too.
    fn drive(host: &mut CoreHost<DisplayCurrency>, event: Event) -> CurrencyView {
        let mut pending = host.dispatch(event);
        while let Some(next) = pending.pop() {
            let result = match DisplayCurrency::perform(&next.operation) {
                Answer::Now(result) => result,
                // `resolve_rate` reaches the network since 031, so this driver
                // performs blocking work inline. A test that uses it is a live
                // test and is marked so.
                Answer::Blocking(work) => work(),
                // The reports a streaming operation makes on its way. Dispatched
                // BEFORE its result, which is the order the async pump
                // guarantees and the order `balance_dashboard` depends on.
                Answer::Streaming(work) => {
                    let (reports, result) = crate::resident::run_streaming(work);
                    for report in reports {
                        pending.extend(host.dispatch(report));
                    }
                    result
                }
                Answer::After(_, result) => result,
            };
            pending.extend(host.resolve(next.id, result));
        }
        host.view()
    }

    /// A wallet with no stored preference lands on USD at rate 1 — a real
    /// claim, and the only one that is true by definition.
    #[test]
    fn a_fresh_wallet_commits_usd() {
        storage::tests::with_temp_state("currency-fresh", || {
            let mut host = CoreHost::<DisplayCurrency>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.code, "USD");
            assert_eq!(view.rate, Some(1.0));
            assert!(view.committed, "USD at 1 is committable without a source");
        });
    }

    /// An explicit choice persists immediately — "user choice always wins" —
    /// and comes back PRICED after a relaunch.
    ///
    /// In 030 this test asserted the opposite: `rate: None`, because there was
    /// no rate source and the core refused to invent one. Flipping the arm
    /// flipped the assertion, which is the visible half of the handoff contract
    /// working. The pair still commits ATOMICALLY — a code without its rate is
    /// the wrong-magnitude-balance bug the core's doc opens with.
    #[test]
    #[ignore = "prices a currency against the live rate endpoint"]
    fn a_chosen_currency_comes_back_priced() {
        storage::tests::with_temp_state("currency-chosen", || {
            let mut host = CoreHost::<DisplayCurrency>::new();
            drive(&mut host, Event::Refresh);
            let view = drive(
                &mut host,
                Event::UserChose {
                    code: "JPY".to_owned(),
                },
            );
            assert_eq!(view.code, "JPY", "the person's choice must win");
            let rate = view
                .rate
                .unwrap_or_else(|| unreachable!("the endpoint could not price JPY"));
            assert!(rate > 1.0, "a plausible JPY rate, got {rate}");

            // Relaunch: a fresh core over the same directory.
            let mut relaunched = CoreHost::<DisplayCurrency>::new();
            let after = drive(&mut relaunched, Event::Refresh);
            assert_eq!(after.code, "JPY", "the choice did not survive the relaunch");
            assert!(
                after.rate.is_some(),
                "a committed pair must carry its rate, or the balance renders at the wrong magnitude"
            );
        });
    }

    #[test]
    fn a_chosen_currency_survives_a_relaunch() {
        storage::tests::with_temp_state("currency-roundtrip", || {
            assert_eq!(
                perform(CurrencyOperation::ReadStoredCode),
                CurrencyShellResult::StoredCode { code: None },
                "an unwritten preference must read as 'never chose'"
            );

            perform(CurrencyOperation::WriteStoredCode {
                code: "JPY".to_owned(),
            });

            assert_eq!(
                perform(CurrencyOperation::ReadStoredCode),
                CurrencyShellResult::StoredCode {
                    code: Some("JPY".to_owned())
                }
            );
        });
    }

    /// USD is 1 by definition and needs no source — the one rate that is a
    /// fact rather than a quote.
    #[test]
    fn usd_is_one_without_asking_anybody() {
        assert_eq!(resolve_rate("USD"), Some(1.0));
    }

    /// A region yields a candidate; a language without one yields `None`.
    ///
    /// Saying `None` is more honest than assuming a country: `en` is spoken in
    /// places with a dozen different currencies, and the core will not commit a
    /// seed it cannot price anyway.
    #[test]
    fn a_region_gives_a_candidate_and_a_bare_language_does_not() {
        assert_eq!(region_currency("zh_CN").as_deref(), Some("CNY"));
        assert_eq!(region_currency("ja-JP").as_deref(), Some("JPY"));
        assert_eq!(region_currency("de_DE").as_deref(), Some("EUR"));
        assert_eq!(region_currency("en"), None, "no region, no claim");
        assert_eq!(region_currency("zh"), None);
        assert_eq!(
            region_currency("xx_ZZ"),
            None,
            "an unknown region is not guessed"
        );
    }

    /// Answered, not skipped — the cardinal sin of the executor contract.
    #[test]
    fn the_device_currency_is_answered_rather_than_skipped() {
        match perform(CurrencyOperation::ReadDeviceCurrency) {
            CurrencyShellResult::DeviceCurrency { .. } => {}
            other => unreachable!("wrong variant: {other:?}"),
        }
    }

    /// The rate source, live. `None` would mean the endpoint could not price
    /// a major currency, which is worth failing on rather than shrugging at.
    #[test]
    #[ignore = "hits the configured fiat-rate endpoint"]
    fn the_rate_source_prices_a_major_currency() {
        let rate =
            resolve_rate("CNY").unwrap_or_else(|| unreachable!("the endpoint could not price CNY"));
        println!("  USD -> CNY = {rate}");
        assert!(
            (1.0..100.0).contains(&rate),
            "a plausible CNY rate, got {rate}"
        );
        assert_eq!(resolve_rate("ZZZ"), None, "an unknown code has no rate");
    }
}
