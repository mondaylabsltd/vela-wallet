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
//!
//! The feed carries a SPLIT (one send to two people, PR 3 note 15): its
//! total is a figure and so is each recipient's share, in the row and in the
//! detail the row opens — where the shares are listed. Every row of the feed
//! is opened here, by what it is rather than by a list of ids, so a row
//! added to the fixture is replayed the day it is added.

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

pub(super) fn fixture() -> Value {
    serde_json::from_str(FIXTURE).unwrap_or_else(|error| unreachable!("the fixture reads: {error}"))
}

pub(super) fn views(fixture: &Value, side: &str) -> (BalanceView, FeedView) {
    let balance = serde_json::from_value(fixture[side]["balance"].clone())
        .unwrap_or_else(|error| unreachable!("{side} balance: {error}"));
    let feed = serde_json::from_value(fixture[side]["feed"].clone())
        .unwrap_or_else(|error| unreachable!("{side} feed: {error}"));
    (balance, feed)
}

pub(super) fn strings() -> (WalletStrings, FlowStrings) {
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

pub(super) fn activity_text(rows: &[ActivityRowModel]) -> Vec<String> {
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

pub(super) fn asset_text(rows: &[AssetRowModel]) -> Vec<String> {
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

pub(super) fn detail_text(detail: &TxDetail) -> Vec<String> {
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
    // Every row's detail, by what the row is: a transfer (a split among
    // them — its detail lists each recipient's share) or a dApp interaction.
    let details = |dapp: bool| -> Vec<String> {
        feed_items(feed)
            .into_iter()
            .filter(|item| item.dapp.is_some() == dapp)
            .flat_map(|item| {
                crate::flows::live::tx_detail(feed, &item.id, &flow, &wallet, hidden, "en", money)
                    .map(|detail| detail_text(&detail))
                    .unwrap_or_else(|| unreachable!("row {} opens", item.id))
            })
            .collect()
    };
    out.push(("transfer_detail", details(false)));
    out.push(("dapp_detail", details(true)));
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

/// The feed's rows, headers aside.
pub(super) fn feed_items(feed: &FeedView) -> Vec<&vela_core::app::activity_feed::FeedItem> {
    feed.rows
        .iter()
        .filter_map(|row| match row {
            FeedRow::Item { item } => Some(item),
            FeedRow::Header { .. } => None,
        })
        .collect()
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

/// (5) The split (PR 3 note 15): one send to two people. Shown, its total
/// and both shares are said — the total on the row, on the home's cut and in
/// History; the shares in the detail the row opens. Hidden, none of the
/// three is said anywhere, and each masked figure keeps its unit
/// ("•••• USDC", the core's `masked_amount`) — the total AND each share. A
/// hidden split used to mask its total and still list who got how much.
#[test]
fn a_split_hides_its_total_and_every_share() {
    use vela_core::app::privacy::masked_amount;
    const RUNS: [&str; 3] = ["683", "214", "469"];
    let fixture = fixture();
    let (wallet, flow) = strings();
    let money = Money::usd();
    for run in RUNS {
        assert!(
            forbidden(&fixture).iter().any(|known| known == run),
            "`{run}` is one of the fixture's figures"
        );
    }

    for side in ["shown", "hidden"] {
        let (_, feed) = views(&fixture, side);
        let hidden = feed.hidden;
        assert_eq!(hidden, side == "hidden");
        let split = feed_items(&feed)
            .into_iter()
            .find(|item| item.batch.is_some())
            .unwrap_or_else(|| unreachable!("the fixture has a split"));
        let batch = split
            .batch
            .as_ref()
            .unwrap_or_else(|| unreachable!("a batch"));
        assert_eq!(batch.transfers.len(), 2, "two recipients");
        assert!(split.figure_maskable, "a split's total is money");
        let unit = masked_amount("USDC");
        assert_eq!(unit, "•••• USDC");

        // The row — on History, and on the home's cut of three.
        let row = crate::wallet::live::activity_row(split, &wallet, hidden);
        let history: Vec<String> = crate::flows::live::history(&feed, &flow, &wallet, hidden)
            .iter()
            .flat_map(|group| activity_text(&group.rows))
            .collect();
        let home = activity_text(&crate::wallet::live::activity_rows(
            &feed, &wallet, &flow, hidden,
        ));
        assert!(
            feed.home_rows
                .iter()
                .any(|row| matches!(row, FeedRow::Item { item } if item.id == split.id)),
            "the home draws the split too"
        );
        // Its detail: the total over the list of who got how much.
        let detail =
            crate::flows::live::tx_detail(&feed, &split.id, &flow, &wallet, hidden, "en", money)
                .unwrap_or_else(|| unreachable!("the split opens"));
        assert_eq!(detail.breakdown.len(), 2, "each recipient has a line");
        let shares: Vec<&str> = detail
            .breakdown
            .iter()
            .map(|line| line.value.as_ref())
            .collect();
        let in_detail = detail_text(&detail);

        if hidden {
            assert_eq!(row.amount.as_ref(), crate::wallet::fixtures::MASK);
            assert_eq!(row.unit.as_ref(), "USDC", "the row keeps its unit");
            assert_eq!(detail.amount.as_ref(), unit, "the total keeps its unit");
            assert_eq!(shares, vec![unit.as_str(), unit.as_str()]);
            assert!(detail.fiat.is_empty(), "no worth either: {}", detail.fiat);
            for run in RUNS {
                for (surface, texts) in [
                    ("the row", activity_text(std::slice::from_ref(&row))),
                    ("History", history.clone()),
                    ("the home", home.clone()),
                    ("the detail", in_detail.clone()),
                ] {
                    assert!(
                        !says(&texts, run),
                        "hidden: {surface} says `{run}`: {texts:?}"
                    );
                }
            }
            // Who it went to is not a figure: the names stay.
            assert_eq!(detail.breakdown[0].label.as_ref(), "Bea");
        } else {
            assert!(row.amount.contains("683.75"), "{}", row.amount);
            assert!(says(&history, "683") && says(&home, "683"));
            assert!(detail.amount.contains("683.75"), "{}", detail.amount);
            assert_eq!(shares, vec!["214.5 USDC", "469.25 USDC"]);
            for run in RUNS {
                assert!(says(&in_detail, run), "shown: the detail says `{run}`");
            }
        }
    }
}
