//! The shared hidden-balance fixture, replayed through every surface builder
//! this shell has (correctness batch item 2).
//!
//! `rust/crates/vela-core/tests/fixtures/privacy-hidden.json` is a real
//! balance and feed the core produced, shown and hidden, with distinctive
//! digit runs (`forbidden`). Every shell replays it: shown, each run appears
//! on some surface; hidden, no masked surface says one and its figures read
//! the mask; the surfaces the person starts on purpose (Send) keep theirs;
//! and a feed row's figure masks exactly when the core says it is money
//! (`figure_maskable`). The signing sheet and Receive's "arrived" list take
//! no privacy flag at all — they cannot mask by construction.

use gpui::SharedString;
use serde_json::Value;

use vela_core::app::activity_feed::{FeedRow, FeedView};
use vela_core::app::balance_dashboard::BalanceView;
use vela_core::app::contacts::ContactsView;

use crate::flows::FlowStrings;
use crate::flows::fixtures::{BreakdownRow, FactRow, TxDetail};
use crate::wallet::WalletStrings;
use crate::wallet::fixtures::{ActivityRowModel, AssetRowModel, Fiat};
use crate::wallet::live::Money;

const FIXTURE: &str =
    include_str!("../../../../rust/crates/vela-core/tests/fixtures/privacy-hidden.json");

fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap_or_else(|error| unreachable!("the fixture reads: {error}"))
}

fn views(fixture: &Value, side: &str) -> (BalanceView, FeedView) {
    let balance = serde_json::from_value(fixture[side]["balance"].clone())
        .unwrap_or_else(|error| unreachable!("{side} balance: {error}"));
    let feed = serde_json::from_value(fixture[side]["feed"].clone())
        .unwrap_or_else(|error| unreachable!("{side} feed: {error}"));
    (balance, feed)
}

fn strings() -> (WalletStrings, FlowStrings) {
    let loc = crate::loc::Loc::for_language("en");
    (WalletStrings::resolve(&loc), FlowStrings::resolve(&loc))
}

/// The one contact the feed's `contact_rows` are about (Bea).
fn contacts() -> ContactsView {
    let mut view = crate::core_host::CoreHost::<vela_core::app::contacts::Contacts>::new().view();
    view.contacts = vec![
        serde_json::from_value(serde_json::json!({
            "address": "0xdddddddddddddddddddddddddddddddddddddddd",
            "name": "Bea",
            "resolved_name": null,
            "resolved_source": null,
            "kind": "unknown",
            "favorite": false,
            "note": null,
            "tx_count": 1,
            "last_used_ms": 1_699_913_600_000.0_f64,
            "first_seen_ms": 1_699_913_600_000.0_f64,
            "source": "manual",
        }))
        .unwrap_or_else(|error| unreachable!("a contact: {error}")),
    ];
    view
}

fn activity_text(rows: &[ActivityRowModel]) -> Vec<String> {
    rows.iter()
        .flat_map(|row| {
            [
                row.title.to_string(),
                row.subtitle.to_string(),
                row.amount.to_string(),
                row.unit.to_string(),
                row.received
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ]
        })
        .collect()
}

fn fiat_text(fiat: &Fiat) -> String {
    match fiat {
        Fiat::Value(text) | Fiat::NoPrice(text) => text.to_string(),
        Fiat::Masked => crate::wallet::fixtures::MASK.to_owned(),
        Fiat::Pending => String::new(),
    }
}

fn asset_text(rows: &[AssetRowModel]) -> Vec<String> {
    rows.iter()
        .flat_map(|row| [row.balance.to_string(), fiat_text(&row.fiat)])
        .collect()
}

fn fact_text(facts: &[FactRow]) -> Vec<String> {
    facts
        .iter()
        .flat_map(|fact| {
            [
                fact.value.to_string(),
                fact.detail
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
                fact.note
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ]
        })
        .collect()
}

fn breakdown_text(rows: &[BreakdownRow]) -> Vec<String> {
    rows.iter()
        .flat_map(|row| {
            [
                row.label.to_string(),
                row.value.to_string(),
                row.detail
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default(),
            ]
        })
        .collect()
}

fn detail_text(detail: &TxDetail) -> Vec<String> {
    let mut out = vec![detail.amount.to_string(), detail.fiat.to_string()];
    out.extend(fact_text(&detail.facts));
    out.extend(breakdown_text(&detail.breakdown));
    out
}

/// Every masked surface's output, by surface name (the fixture's
/// `masked_surfaces`), for one side of the fixture.
fn masked_surfaces(balance: &BalanceView, feed: &FeedView) -> Vec<(&'static str, Vec<String>)> {
    let (wallet, flow) = strings();
    let money = Money::usd();
    let hidden = feed.hidden;
    let mut out = Vec::new();

    let hero = crate::wallet::live::balance(balance, &wallet, "en", money);
    out.push((
        "home_total",
        vec![
            hero.integer.to_string(),
            hero.decimals.map(|d| d.to_string()).unwrap_or_default(),
            hero.live.map(|d| d.to_string()).unwrap_or_default(),
        ],
    ));
    out.push((
        "holdings",
        asset_text(&crate::wallet::live::asset_rows(
            balance, &wallet, "en", None, money,
        )),
    ));
    out.push((
        "assets",
        asset_text(&crate::flows::live::assets(balance, &flow, &wallet, "en", None, money).rows),
    ));
    let mut token_detail = Vec::new();
    for index in 0..balance.tokens.len() {
        if let Some(detail) =
            crate::wallet::live::asset_detail(balance, feed, index, &wallet, "en", money)
        {
            token_detail.push(detail.amount.to_string());
            token_detail.push(detail.sub.to_string());
            token_detail.extend(detail.facts.iter().map(|(_, value)| value.to_string()));
            token_detail.extend(activity_text(&detail.activity));
        }
    }
    out.push(("token_detail", token_detail));
    out.push((
        "home_activity",
        activity_text(&crate::wallet::live::activity_rows(
            feed, &wallet, &flow, hidden,
        )),
    ));
    out.push((
        "history",
        crate::flows::live::history(feed, &flow, &wallet, hidden)
            .iter()
            .flat_map(|group| activity_text(&group.rows))
            .collect(),
    ));
    let detail_of = |id: &str| {
        crate::flows::live::tx_detail(feed, id, &flow, &wallet, hidden, "en", money)
            .map(|detail| detail_text(&detail))
            .unwrap_or_default()
    };
    out.push((
        "transfer_detail",
        ["received", "sent"]
            .iter()
            .flat_map(|id| detail_of(id))
            .collect(),
    ));
    out.push((
        "dapp_detail",
        ["swap", "permit", "permit-unlimited", "signature"]
            .iter()
            .flat_map(|id| detail_of(id))
            .collect(),
    ));
    out.push((
        "contact_activity",
        crate::contacts::live::detail(&contacts(), 0, feed, &wallet, true, hidden)
            .map(|detail| activity_text(&detail.activity))
            .unwrap_or_default(),
    ));
    let (total, rows) = crate::settings::live::switcher_figures(
        &balance.switcher,
        &[
            "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb",
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ],
        None,
        "en",
    );
    let mut switcher = vec![total.to_string()];
    switcher.extend(rows.into_iter().flatten().map(|row| row.to_string()));
    out.push(("account_switcher", switcher));
    let detail = crate::wallet::live::balance_detail(balance, &wallet, "en", money);
    let mut balance_detail = vec![detail.summary.to_string()];
    for chain in detail
        .pending
        .iter()
        .chain(&detail.done)
        .chain(&detail.unpriced)
    {
        balance_detail.push(
            chain
                .amount
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_default(),
        );
    }
    balance_detail.extend(
        crate::wallet::live::unreachable_list(balance, &wallet, "en", money)
            .rows
            .iter()
            .map(|row| row.line.to_string()),
    );
    out.push(("balance_detail", balance_detail));
    out.push((
        "network_picker",
        crate::wallet::live::network_balances(balance, "en", money)
            .values()
            .map(ToString::to_string)
            .collect(),
    ));
    out.push((
        "receipt_toast",
        crate::wallet::live::receipt_toast(feed, &wallet)
            .map(|toast| vec![toast.to_string()])
            .unwrap_or_default(),
    ));
    out
}

fn says(texts: &[String], run: &str) -> bool {
    texts.iter().any(|text| text.contains(run))
}

fn forbidden(fixture: &Value) -> Vec<String> {
    fixture["forbidden"]
        .as_array()
        .unwrap_or_else(|| unreachable!("forbidden runs"))
        .iter()
        .filter_map(|run| run.as_str().map(str::to_owned))
        .collect()
}

/// (1) Shown: every forbidden run is said somewhere — the replay really
/// reaches the figures it then checks are hidden.
#[test]
fn shown_every_figure_appears_somewhere() {
    let fixture = fixture();
    let (balance, feed) = views(&fixture, "shown");
    assert!(!balance.hidden && !feed.hidden);
    let surfaces = masked_surfaces(&balance, &feed);
    let all: Vec<String> = surfaces
        .iter()
        .flat_map(|(_, texts)| texts.clone())
        .collect();
    for run in forbidden(&fixture) {
        assert!(says(&all, &run), "shown: `{run}` appears on no surface");
    }
}

/// (2) Hidden: no masked surface says any forbidden run, and the figures it
/// still draws read the mask — the switcher's rows and total included.
#[test]
fn hidden_no_masked_surface_says_a_figure() {
    let fixture = fixture();
    let (balance, feed) = views(&fixture, "hidden");
    assert!(balance.hidden && feed.hidden && balance.switcher.hidden);
    let mask = fixture["mask"].as_str().unwrap_or_default();
    assert_eq!(mask, crate::wallet::fixtures::MASK);
    assert_eq!(
        fixture["balance_mask"].as_str().unwrap_or_default(),
        crate::wallet::fixtures::BALANCE_MASK
    );
    let surfaces = masked_surfaces(&balance, &feed);
    let named: Vec<&str> = surfaces.iter().map(|(name, _)| *name).collect();
    for surface in fixture["masked_surfaces"].as_array().into_iter().flatten() {
        let surface = surface.as_str().unwrap_or_default();
        assert!(named.contains(&surface), "{surface} is replayed");
    }
    for (surface, texts) in &surfaces {
        for run in forbidden(&fixture) {
            assert!(
                !says(texts, &run),
                "hidden: {surface} says `{run}`: {texts:?}"
            );
        }
    }
    let masked = |name: &str| {
        surfaces
            .iter()
            .find(|(surface, _)| *surface == name)
            .is_some_and(|(_, texts)| says(texts, mask))
    };
    for surface in [
        "holdings",
        "assets",
        "token_detail",
        "home_activity",
        "history",
        "transfer_detail",
        "dapp_detail",
        "contact_activity",
        "account_switcher",
        "balance_detail",
        "network_picker",
    ] {
        assert!(masked(surface), "{surface} draws the mask");
    }
    let (total, rows) = crate::settings::live::switcher_figures(
        &balance.switcher,
        &["0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"],
        None,
        "en",
    );
    assert_eq!(total.as_ref(), mask);
    assert_eq!(rows, vec![Some(SharedString::from(mask))]);
    assert_eq!(
        crate::wallet::live::balance(&balance, &strings().0, "en", Money::usd()).state,
        crate::wallet::fixtures::BalanceState::Hidden
    );
}

/// (3) Hidden: Send keeps its figures — you cannot choose an amount you
/// cannot see. Its picker draws the same tokens, unmasked.
#[test]
fn hidden_send_still_shows_the_figures() {
    let fixture = fixture();
    let (balance, _) = views(&fixture, "hidden");
    let (wallet, flow) = strings();
    let mut send = crate::core_host::CoreHost::<vela_core::app::send::Send>::new().view();
    send.tokens = balance
        .tokens
        .iter()
        .map(|token| vela_core::app::send::SendToken {
            network: crate::flows::live::chain_name(token.chain_id),
            chain_id: token.chain_id,
            symbol: token.symbol.clone(),
            balance: token.balance.clone(),
            decimals: token.decimals,
            token_address: token.token_address.clone(),
            price_usd: token.price_usd,
            logo_urls: Vec::new(),
            spam: false,
        })
        .collect();
    let fee = crate::core_host::CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
    let pick = crate::flows::live::send_pick_with(
        &crate::flows::live::SendInputs {
            send: &send,
            fee: &fee,
            s: &flow,
            wallet: &wallet,
            locale: "en",
            money: Money::usd(),
            identity_name: "Golden",
            identity_address: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            speed: None,
            relay_sent_at_ms: None,
        },
        false,
        crate::flows::live::SendClass::default(),
    );
    let texts = asset_text(&pick.rows);
    for run in ["418", "376"] {
        assert!(says(&texts, run), "Send's picker shows `{run}`: {texts:?}");
    }
}

/// (4) A feed row's figure masks exactly when the core says it is money
/// (`figure_maskable`) — an unlimited allowance and a signature with no
/// figure never wear the mask.
#[test]
fn a_row_masks_exactly_when_its_figure_is_money() {
    let fixture = fixture();
    let (_, feed) = views(&fixture, "hidden");
    let (wallet, _) = strings();
    let expected = fixture["figure_maskable"]
        .as_object()
        .unwrap_or_else(|| unreachable!("figure_maskable per row"));
    let mut seen = 0;
    for row in &feed.rows {
        let FeedRow::Item { item } = row else {
            continue;
        };
        let want = expected
            .get(&item.id)
            .and_then(Value::as_bool)
            .unwrap_or_else(|| unreachable!("row {} has a verdict", item.id));
        assert_eq!(item.figure_maskable, want, "{}", item.id);
        let drawn = crate::wallet::live::activity_row(item, &wallet, true);
        assert_eq!(
            drawn.amount.as_ref() == crate::wallet::fixtures::MASK,
            want,
            "{}: drew {:?}",
            item.id,
            drawn.amount
        );
        seen += 1;
    }
    assert_eq!(seen, expected.len());
}
