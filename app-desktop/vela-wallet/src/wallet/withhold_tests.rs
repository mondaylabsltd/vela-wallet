//! The withhold rule, on every surface this shell draws money in the display
//! currency (PR 3 notes 9 and 27).
//!
//! The core's rule (`app::display_currency`): until the display currency is
//! committed (`CurrencyView.committed`) **no fiat figure is drawn — on any
//! surface** — and withholding never moves the layout. The surfaces are the
//! core's own list, `FIAT_SURFACES`; this file has one builder per name in
//! it, and the two lists are held equal, so a surface added to the core is a
//! failing test here until this shell has drawn it both ways.
//!
//! Each surface is built twice from the same views — the shared privacy
//! fixture's real balance and feed, shown — once with the currency on its
//! way and once with it landed:
//!
//! - **waiting**: nothing it draws carries a fiat figure — not the
//!   placeholder's dollars, not the person's currency, not a dash or a zero
//!   standing in for one — while what is NOT fiat (a token amount, a name, a
//!   network) is still drawn;
//! - **landed**: the figure is there, in the person's money.
//!
//! "Nothing moves" is the other half, and it is a fact about layout: each
//! withheld figure's place is kept by what draws it (the hero's skeleton in
//! the figure's own line box, a holding's bar, an empty line under an
//! amount, a row's own height). That half is measured on the boards
//! (`VELA_CURRENCY_PENDING=<code>` against `<code>:landed`): the frames
//! differ only on the rows the figures are on.

use vela_core::app::display_currency::{CurrencyView, FIAT_SURFACES};

use super::privacy_tests::{
    activity_text, asset_text, detail_text, feed_items, fixture, strings, views,
};
use crate::wallet::live::Money;

/// The display currency before the core commits one: the USD/1 placeholder,
/// with the person's stored choice on its way.
fn waiting() -> CurrencyView {
    CurrencyView {
        code: "USD".to_owned(),
        rate: Some(1.0),
        committed: false,
        pending: Some("CNY".to_owned()),
    }
}

/// The same choice, committed.
fn landed() -> CurrencyView {
    CurrencyView {
        code: "CNY".to_owned(),
        rate: Some(7.0),
        committed: true,
        pending: None,
    }
}

/// A send of a quarter of an ETH with its fee quoted — the form, the review
/// and the signing sheet price it.
fn send() -> (
    vela_core::app::send::SendView,
    vela_core::app::fee_policy::FeeView,
) {
    let eth = vela_core::app::send::SendToken {
        network: "ethereum".to_owned(),
        chain_id: 1,
        symbol: "ETH".to_owned(),
        balance: "0.5".to_owned(),
        decimals: 18,
        token_address: None,
        price_usd: Some(2_469.0),
        logo_urls: Vec::new(),
        spam: false,
    };
    let mut send = crate::core_host::CoreHost::<vela_core::app::send::Send>::new().view();
    send.tokens = vec![eth.clone()];
    send.selected_token = Some(eth);
    send.amount = "0.25".to_owned();
    send.token_amount = "0.25".to_owned();
    send.confirm_amount = "0.25".to_owned();
    send.recipient = "0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE".to_owned();
    (send, crate::signing::fixtures::settled_fee())
}

/// What one surface draws, as text, in `currency`.
type Surface = fn(&CurrencyView) -> Vec<String>;

/// Every surface of the core's list, with the builder that draws it here.
/// In the core's order.
const SURFACES: [(&str, Surface); 12] = [
    ("home_total", home_total),
    ("holdings", holdings),
    ("account_switcher", account_switcher),
    ("token_detail", token_detail),
    ("assets", assets),
    ("balance_detail", balance_detail),
    ("activity_row", activity_row),
    ("activity_detail", activity_detail),
    ("send_coin_list", send_coin_list),
    ("send_form", send_form),
    ("signing_sheet", signing_sheet),
    ("settings_total", settings_total),
];

fn shown() -> (
    vela_core::app::balance_dashboard::BalanceView,
    vela_core::app::activity_feed::FeedView,
) {
    views(&fixture(), "shown")
}

fn home_total(currency: &CurrencyView) -> Vec<String> {
    let (balance, _) = shown();
    let hero = crate::wallet::live::balance(&balance, &strings().0, "en", &Money::of(currency));
    // The figure is drawn in two sizes — the whole part, then the minor
    // units — and is one figure.
    vec![match hero.decimals {
        Some(minor) => format!("{}.{minor}", hero.integer),
        None => hero.integer.to_string(),
    }]
}

fn holdings(currency: &CurrencyView) -> Vec<String> {
    let (balance, _) = shown();
    asset_text(&crate::wallet::live::asset_rows(
        &balance,
        &strings().0,
        "en",
        None,
        &Money::of(currency),
    ))
}

fn account_switcher(currency: &CurrencyView) -> Vec<String> {
    let (balance, _) = shown();
    let (total, rows) = crate::settings::live::switcher_figures(
        &balance.switcher,
        &[
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ],
        Some(currency),
        "en",
    );
    // A row with no figure is still a row: its place is there either way.
    let mut out = vec![total.to_string()];
    out.extend(
        rows.into_iter()
            .map(|row| row.map(|row| row.to_string()).unwrap_or_default()),
    );
    out
}

/// A token's page: its worth, and its price.
fn token_detail(currency: &CurrencyView) -> Vec<String> {
    let (balance, feed) = shown();
    let (wallet, _) = strings();
    let mut out = Vec::new();
    for index in 0..balance.tokens.len() {
        let detail = crate::wallet::live::asset_detail(
            &balance,
            &feed,
            index,
            &wallet,
            "en",
            &Money::of(currency),
        )
        .unwrap_or_else(|| unreachable!("token {index} has a page"));
        out.push(detail.amount.to_string());
        out.push(detail.sub.to_string());
        out.extend(detail.facts.iter().map(|(_, value)| value.to_string()));
    }
    out
}

fn assets(currency: &CurrencyView) -> Vec<String> {
    let (balance, _) = shown();
    let (wallet, flow) = strings();
    asset_text(
        &crate::flows::live::assets(&balance, &flow, &wallet, "en", None, &Money::of(currency))
            .rows,
    )
}

/// The breakdown sheet — its total, a figure per network — and the
/// unreachable list's "last seen".
fn balance_detail(currency: &CurrencyView) -> Vec<String> {
    let (balance, _) = shown();
    let (wallet, _) = strings();
    let money = Money::of(currency);
    let detail = crate::wallet::live::balance_detail(&balance, &wallet, "en", &money);
    let mut out = vec![detail.summary.to_string()];
    for line in detail
        .pending
        .iter()
        .chain(&detail.done)
        .chain(&detail.unpriced)
    {
        out.push(
            line.amount
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
        );
        // A token nobody prices is listed by its amount — not fiat.
        out.push(
            line.status
                .as_ref()
                .map(|(text, _)| text.to_string())
                .unwrap_or_default(),
        );
    }
    out.extend(
        crate::wallet::live::unreachable_list(&balance, &wallet, "en", &money)
            .rows
            .iter()
            .map(|row| row.line.to_string()),
    );
    // The network picker's figure beside each network the wallet holds on:
    // the network is listed either way, with or without its figure.
    let per_network = crate::wallet::live::network_balances(&balance, "en", &money);
    let mut networks: Vec<u32> = balance.tokens.iter().map(|token| token.chain_id).collect();
    networks.sort_unstable();
    networks.dedup();
    out.extend(networks.iter().map(|chain_id| {
        per_network
            .get(chain_id)
            .map(ToString::to_string)
            .unwrap_or_default()
    }));
    out
}

/// An activity row. This shell's row says the coin and how much of it and
/// carries no worth at all — so it takes no currency, and there is nothing
/// on it to withhold. Built here so that stays true: a fiat figure added to
/// the row without the rule fails `landed_and_waiting_differ_only_by_fiat`.
fn activity_row(_currency: &CurrencyView) -> Vec<String> {
    let (_, feed) = shown();
    let (wallet, flow) = strings();
    let mut out = activity_text(&crate::wallet::live::activity_rows(
        &feed, &wallet, &flow, false,
    ));
    out.extend(
        crate::flows::live::history(&feed, &flow, &wallet, false)
            .iter()
            .flat_map(|group| activity_text(&group.rows)),
    );
    out
}

/// A transfer's detail and a dApp row's: the worth under the amount.
fn activity_detail(currency: &CurrencyView) -> Vec<String> {
    let (_, feed) = shown();
    let (wallet, flow) = strings();
    feed_items(&feed)
        .into_iter()
        .flat_map(|item| {
            crate::flows::live::tx_detail(
                &feed,
                &item.id,
                &flow,
                &wallet,
                false,
                "en",
                &Money::of(currency),
            )
            .map(|detail| detail_text(&detail))
            .unwrap_or_else(|| unreachable!("row {} opens", item.id))
        })
        .collect()
}

fn send_inputs_text(
    currency: &CurrencyView,
    draw: impl Fn(&crate::flows::live::SendInputs<'_>) -> Vec<String>,
) -> Vec<String> {
    let (send, fee) = send();
    let (wallet, flow) = strings();
    draw(&crate::flows::live::SendInputs {
        send: &send,
        fee: &fee,
        s: &flow,
        wallet: &wallet,
        locale: "en",
        money: &Money::of(currency),
        identity_name: "Golden",
        identity_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        speed: None,
        relay_sent_at_ms: None,
    })
}

fn send_coin_list(currency: &CurrencyView) -> Vec<String> {
    send_inputs_text(currency, |inputs| {
        asset_text(
            &crate::flows::live::send_pick_with(
                inputs,
                false,
                crate::flows::live::SendClass::default(),
            )
            .rows,
        )
    })
}

/// The form — the "≈" line under the amount, the fee row — and the review:
/// its line under the amount and its fee fact.
fn send_form(currency: &CurrencyView) -> Vec<String> {
    send_inputs_text(currency, |inputs| {
        let form = crate::flows::live::send_form(inputs);
        let confirm = crate::flows::live::send_confirm(inputs);
        let (amount, other_line) = form.amount.unwrap_or_default();
        let mut out = vec![
            amount.to_string(),
            other_line.to_string(),
            crate::flows::live::send_fee_row(inputs).value.to_string(),
            confirm.amount.to_string(),
            confirm.subline.to_string(),
            crate::flows::live::confirm_fee_fact(inputs)
                .value
                .to_string(),
        ];
        out.extend(confirm.facts.iter().map(|fact| fact.value.to_string()));
        out
    })
}

/// The signing sheet: the fee's money. (This shell's sheet draws a balance
/// change as the coin and how much of it, with no worth beside it.)
fn signing_sheet(currency: &CurrencyView) -> Vec<String> {
    let s = crate::signing::SigningStrings::resolve(&crate::loc::Loc::for_language("en"));
    match crate::signing::fixtures::settled_fee_row(&s, &Money::of(currency)) {
        crate::signing::fixtures::FeeModel::OnChain { value, .. } => vec![value.to_string()],
        other => unreachable!("a transfer's fee is on-chain: {}", fee_kind(&other)),
    }
}

fn fee_kind(model: &crate::signing::fixtures::FeeModel) -> &'static str {
    match model {
        crate::signing::fixtures::FeeModel::OnChain { .. } => "on-chain",
        crate::signing::fixtures::FeeModel::OffChain(_) => "off-chain",
        crate::signing::fixtures::FeeModel::Hidden => "hidden",
    }
}

/// Settings' accounts header: the count, and the total's clause.
fn settings_total(currency: &CurrencyView) -> Vec<String> {
    let total = crate::settings::live::account_total(1_234.5, Some(currency), "en");
    vec![
        total.to_string(),
        crate::settings::live::accounts_summary("3 accounts", " · Total {{amount}}", &total)
            .to_string(),
    ]
}

/// Does a text carry a fiat figure — in any money, or a stand-in for one?
/// A currency sign (no coin in the fixture wears one); or what is left of a
/// figure's phrase when the figure is cut out carelessly: a dash drawn where
/// it goes, an "≈" or a "·" with nothing after it, a sentence about an
/// amount with none in it.
fn says_fiat(text: &str) -> bool {
    let text = text.trim();
    text.contains(['$', '¥', '€', '£'])
        || text == "—"
        || text.ends_with('≈')
        || text.ends_with('·')
        || text.ends_with("Total")
        || text.ends_with("Last seen")
        || text.contains("—")
            && (text.starts_with("Total") || text.starts_with("Last seen") || text.contains('≈'))
}

/// This shell's list of fiat surfaces IS the core's: same names, same order,
/// nothing missing and nothing of its own.
#[test]
fn every_fiat_surface_of_the_core_is_drawn_here() {
    let here: Vec<&str> = SURFACES.iter().map(|(name, _)| *name).collect();
    assert_eq!(here, FIAT_SURFACES, "the core's FIAT_SURFACES");
}

/// Waiting: no surface draws a fiat figure — not in the placeholder's
/// dollars, not in the person's currency, and not a dash or a zero in a
/// figure's place.
#[test]
fn no_surface_draws_a_fiat_figure_before_the_currency_commits() {
    for (name, surface) in SURFACES {
        let texts = surface(&waiting());
        for text in &texts {
            assert!(
                !says_fiat(text),
                "{name} says fiat while waiting: {text:?} in {texts:?}"
            );
        }
    }
    // The one helper, asked directly: it has no figure to give.
    assert_eq!(Money::of(&waiting()).figure(1_234.5, "en"), None);
    assert!(!Money::of(&waiting()).committed());
}

/// Landed: every surface that draws money draws it, in the person's
/// currency and never in dollars — so the waiting test above is about
/// surfaces that really have a figure to withhold.
#[test]
fn every_surface_draws_its_figure_once_the_currency_lands() {
    for (name, surface) in SURFACES {
        let texts = surface(&landed());
        if name == "activity_row" {
            // No worth on this shell's row, whatever the currency.
            assert!(
                !texts.iter().any(|text| says_fiat(text)),
                "{name}: {texts:?}"
            );
            continue;
        }
        assert!(
            texts.iter().any(|text| text.contains('¥')),
            "{name} draws no figure once the currency has landed: {texts:?}"
        );
        assert!(
            !texts.iter().any(|text| text.contains('$')),
            "{name} draws dollars under a committed CNY: {texts:?}"
        );
    }
}

/// What is not fiat is drawn as always while the currency is on its way: a
/// token amount is not in the display currency. Send and the signing sheet
/// stay decidable — the amount, the fee in its coin.
#[test]
fn a_token_amount_is_never_withheld() {
    let waiting = waiting();
    let has = |texts: &[String], what: &str| texts.iter().any(|text| text.contains(what));

    let form = send_form(&waiting);
    assert!(has(&form, "0.25"), "the amount being sent: {form:?}");
    assert!(has(&form, "ETH"), "the fee in its coin: {form:?}");
    let fee = signing_sheet(&waiting);
    assert!(
        has(&fee, "ETH"),
        "the signing sheet's fee in its coin: {fee:?}"
    );
    assert!(has(&holdings(&waiting), "376"), "a holding's balance");
    assert!(
        has(&send_coin_list(&waiting), "0.5"),
        "the coin list's balances"
    );
    assert!(has(&token_detail(&waiting), "376"), "a token page's amount");
    assert!(
        has(&activity_detail(&waiting), "289.5"),
        "a transfer's amount"
    );
}

/// The two frames differ ONLY by fiat: every text a surface draws while
/// waiting is the landed text with its figure taken out — never a different
/// sentence, never a line that is absent in one and present in the other.
/// The count of texts is the count of places: each withheld figure keeps
/// its place (an empty text keeps its line), which is what "nothing moves"
/// rests on.
#[test]
fn landed_and_waiting_differ_only_by_fiat() {
    for (name, surface) in SURFACES {
        let before = surface(&waiting());
        let after = surface(&landed());
        assert_eq!(
            before.len(),
            after.len(),
            "{name}: a place appeared or went when the currency landed"
        );
        for (was, is) in before.iter().zip(&after) {
            if says_fiat(is) {
                // The figure's own place: empty, or what stood beside the
                // figure (the fee's coin, the network's name).
                assert!(
                    was.is_empty() || is.contains(was.as_str()),
                    "{name}: {was:?} became {is:?}"
                );
            } else {
                assert_eq!(was, is, "{name}: a text with no figure changed");
            }
        }
    }
}

/// The line under the hero's figure is not a fiat figure, and does not wait
/// with it: a wallet with a network out of reach says so over the skeleton,
/// in the same words the landed frame says under the figure. It used to
/// arrive WITH the figure — and pushed the whole page down a row when the
/// currency landed (measured on the DSR6 board: every row under the hero
/// moved 37 pt).
#[test]
fn the_heros_line_does_not_wait_for_the_currency() {
    use crate::wallet::fixtures::BalanceState;
    let (wallet, _) = strings();
    for view in [
        crate::wallet::fixtures::unreachable_view(),
        crate::wallet::fixtures::token_list_view(),
        crate::wallet::fixtures::internal_view(),
    ] {
        let before = crate::wallet::live::balance(&view, &wallet, "en", &Money::of(&waiting()));
        let after = crate::wallet::live::balance(&view, &wallet, "en", &Money::of(&landed()));
        assert_eq!(before.state, BalanceState::Loading, "the figure waits");
        assert!(before.integer.is_empty() && before.decimals.is_none());
        assert_eq!(after.state, BalanceState::Normal);
        assert!(after.integer.contains('¥'), "{}", after.integer);
        assert!(after.status.is_some(), "this wallet has something to say");
        assert_eq!(before.status, after.status, "the line is there already");
        // And the one slot the block keeps for it says the same thing in
        // both frames.
        assert_eq!(
            crate::wallet::components::SlotLine::of(&before),
            crate::wallet::components::SlotLine::of(&after)
        );
    }
    // A wallet with nothing wrong has nothing to say, waiting or landed.
    let held = crate::wallet::fixtures::held_view();
    for currency in [waiting(), landed()] {
        let hero = crate::wallet::live::balance(&held, &wallet, "en", &Money::of(&currency));
        assert_eq!(hero.status, None);
        assert_eq!(
            crate::wallet::components::SlotLine::of(&hero),
            crate::wallet::components::SlotLine::Empty
        );
    }
}
