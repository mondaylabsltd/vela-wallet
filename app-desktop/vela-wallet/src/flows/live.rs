//! The flow panels' display models, built from what the cores decided.
//!
//! The sibling of `flows/fixtures.rs`, never its replacement — the same split
//! `wallet`, `settings` and `contacts` already have. A signed-in person gets
//! these; the gallery and an unsigned window get the mocks, because a mock is a
//! picture of the design and this is somebody's money.
//!
//! ## Three panels, three cores
//!
//! - **Assets** (DT1L / DT4L) reads `BalanceView.tokens` — the core's own
//!   USD-sorted holdings, with `unpriced_tokens` deciding which rows say so.
//! - **Activity** (DA1L) reads `FeedView.rows`, headers included: the full
//!   panel draws day headings, which is exactly the shape the core produces and
//!   the home preview throws away.
//! - **Receive** (DR1L / DR2L) reads the person's real address and the networks
//!   the wallet actually knows, custom ones included.

use gpui::{Hsla, SharedString};

use vela_core::app::activity_feed::FeedBatchKind;
use vela_core::app::activity_feed::{FeedRow, FeedTxStatus, FeedView};
use vela_core::app::balance_dashboard::{BalanceToken, BalanceView};
use vela_core::app::manage_tokens::MtokView;
use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::payment_request::PaymentRequestView;
use vela_core::app::receive_watch::ReceiveWatchView;
use vela_core::app::send::SendReceiptKind;
use vela_core::l10n::datetime::{Civil, format_time};
use vela_core::l10n::number::format_token_amount;

use crate::flows::FlowStrings;
use crate::wallet::fill;
use vela_core::app::batch_import::{BatchRateStatus, BatchUnit, BatchView};
use vela_core::app::contacts::ContactsView;
use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView, FeeTier, FeeView};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::send::{
    SendAddNetworkMsg, SendAmountWarning, SendHoldReason, SendLockError, SendReceiptStatus,
    SendRecipientDraft, SendStage, SendToken, SendTreasuryAsset, SendTxStatus, SendUnitIssue,
    SendView,
};

use crate::flows::fixtures::{
    AddressCard, AssetsEmpty, AssetsPanel, BatchImport, BatchRow, BreakdownRow, ContactPick,
    CtaState, DepositEntry as FlowDeposit, FactLead, FactRow, FeeRow, FeeSpeedModel,
    FeeSpeedOption, FeeTokenPick, FeeTokenRow, FilterChip, HistoryGroup, HistoryPanel, NetworkRow,
    ReceiptStage, ReceiveGate, ReceiveList, ReceiveQr, RecipientCard, SendConfirm, SendForm,
    SendNotice, SendPick, SendReceipt, StatusChip, StatusTone, SweepForm, SweepRow, TokenMark,
    address_lines,
};
use crate::wallet::fixtures::{AssetRowModel, Fiat, MASK};

/// A chain's colour, from the one table every surface reads.
/// A chain's colour, for anything drawn outside this module — the share
/// card's network pill, today.
#[must_use]
pub fn chain_tint(chain_id: u32) -> Hsla {
    tint(chain_id)
}

fn tint(chain_id: u32) -> Hsla {
    gpui::rgb(crate::settings::model::chain_tint(u64::from(chain_id)).unwrap_or(0x8A_8F_98)).into()
}

/// A chain's name, from the one place that derives it.
/// A chain's display name. `pub(crate)` and shared with the signing
/// sheet: two names for one chain is a badge disagreeing with a fee.
pub(crate) fn chain_name(chain_id: u32) -> String {
    crate::executor::custom_tokens::network_name(chain_id)
}

// ---------------------------------------------------------------------------
// Assets — DT1L / DT4L
// ---------------------------------------------------------------------------

/// One holding as a row.
///
/// The fiat column has three states and they are not interchangeable:
/// a value, an explicit "no price", and the privacy mask. A token whose price
/// nobody could find must say so rather than show `$0.00` — the core keeps
/// `unpriced_tokens` for precisely this, and rendering it as zero would tell
/// somebody their holding is worthless.
fn asset_row(
    token: &BalanceToken,
    unpriced: bool,
    hidden: bool,
    wallet: &crate::wallet::WalletStrings,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> AssetRowModel {
    let amount = token.balance.parse::<f64>().unwrap_or(0.0);
    AssetRowModel {
        logos: crate::marks::token_logos(
            token.chain_id,
            &token.symbol,
            token.token_address.as_deref(),
            &[],
        ),
        ticker: SharedString::from(token.symbol.clone()),
        chain: SharedString::from(chain_name(token.chain_id)),
        badge: tint(token.chain_id),
        balance: if hidden {
            SharedString::from(MASK)
        } else {
            SharedString::from(format_token_amount(
                amount,
                crate::executor::format_prefs::current().number,
                false,
            ))
        },
        fiat: if hidden {
            Fiat::Masked
        } else if unpriced {
            // The same sentence the balance detail already says. Resolving a
            // second key for one wording is how two surfaces start disagreeing
            // about what "no price" is called.
            Fiat::NoPrice(wallet.no_price.clone())
        } else {
            Fiat::Value(SharedString::from(
                currency.text(amount * token.price_usd.unwrap_or(0.0), locale),
            ))
        },
    }
}

/// DT1L, or DT4L when there is nothing to list.
///
/// **The empty state is only shown once the core has ruled.** While the count
/// is in flight (`holdings_loading`, or a balance the core calls unknown) the
/// panel shows an empty list and no guided-empty body — telling somebody their
/// wallet is empty while it is still being read is the assets-panel version of
/// the fake `$0` the hero refuses.
#[must_use]
pub fn assets(
    view: &BalanceView,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    locale: &str,
    filter: Option<u32>,
    currency: &crate::wallet::live::Money,
) -> AssetsPanel {
    let unpriced: std::collections::BTreeSet<(u32, String)> = view
        .unpriced_tokens
        .iter()
        .map(|token| {
            (
                token.chain_id,
                token.token_address.clone().unwrap_or_default(),
            )
        })
        .collect();

    let rows: Vec<AssetRowModel> = crate::wallet::live::visible_token_indices(view, filter)
        .into_iter()
        .filter_map(|index| view.tokens.get(index))
        .map(|token| {
            let key = (
                token.chain_id,
                token.token_address.clone().unwrap_or_default(),
            );
            asset_row(
                token,
                unpriced.contains(&key),
                view.hidden,
                wallet,
                locale,
                currency,
            )
        })
        .collect();

    let settled = !view.holdings_loading && !view.balance_unknown;
    // Empty once the core has actually looked — or when the chosen chain
    // holds nothing while others do. The web's `liveAssets` shows T4 for both:
    // a narrowed list with nothing in it must still say something, and a
    // blank column under a pill reads as a panel that failed to load.
    let filtered_empty = rows.is_empty() && !view.tokens.is_empty();
    AssetsPanel {
        // The chain filter's dots: the chains this person actually holds on,
        // in the order the core sorted them. A filter offering chains with
        // nothing on them is a filter that does nothing.
        filter: Some((
            // Narrowed: this chain's own dot and its name, so the panel says
            // WHICH list this is. The web puts the same fact in the same pill.
            match filter {
                Some(chain_id) => vec![tint(chain_id)],
                None => chain_dots(&view.tokens),
            },
            match filter {
                Some(chain_id) => {
                    SharedString::from(crate::executor::custom_tokens::network_name(chain_id))
                }
                None => s.pill_all.clone(),
            },
            s.assets_add.clone(),
        )),
        search_placeholder: s.assets_search.clone(),
        no_match: s.no_matching_tokens.clone(),
        rows: rows.clone(),
        add_by_address: s.add_by_address.clone(),
        empty: (rows.is_empty() && (settled || filtered_empty)).then(|| AssetsEmpty {
            title: s.assets_empty_title.clone(),
            caption: s.assets_empty_caption.clone(),
            cta: s.add_token_title.clone(),
            hint_title: s.not_showing_title.clone(),
            hint_body: s.not_showing_body.clone(),
        }),
    }
}

/// Up to three chain dots for the filter pill, deduped in holdings order.
fn chain_dots(tokens: &[BalanceToken]) -> Vec<Hsla> {
    let mut seen = Vec::new();
    for token in tokens {
        if !seen.contains(&token.chain_id) {
            seen.push(token.chain_id);
        }
        if seen.len() == 3 {
            break;
        }
    }
    seen.into_iter().map(tint).collect()
}

// ---------------------------------------------------------------------------
// Activity — DA1L
// ---------------------------------------------------------------------------

/// DA1L's three modes — the web's `liveHistory`.
///
/// Rows when there are any; skeletons while the balance core has not ruled
/// (the feed has nothing yet, and "no transactions" would be a guess); and
/// once it has, one line — about THIS network when the list is narrowed to
/// one, because "no transactions" under a filter would read as "none at all".
#[must_use]
pub fn history_panel(
    view: &FeedView,
    balance_unknown: bool,
    filter: Option<u32>,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
) -> HistoryPanel {
    let groups = history(view, s, wallet, hidden);
    let bare = groups.is_empty();
    HistoryPanel {
        groups,
        loading: bare && balance_unknown,
        empty: (bare && !balance_unknown).then(|| match filter {
            Some(_) => s.history_empty_filter.clone(),
            None => s.history_empty.clone(),
        }),
    }
}

/// The full history, grouped by day.
///
/// Unlike the home preview this KEEPS the core's headers: `FeedView::rows`
/// interleaves them because this panel is what they were computed for. The
/// label is the shell's, because "Today" is a fact about the reader's clock and
/// `vela-core` ships no timezone database.
#[must_use]
pub fn history(
    view: &FeedView,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
) -> Vec<HistoryGroup> {
    let mut groups: Vec<HistoryGroup> = Vec::new();
    for row in &view.rows {
        match row {
            FeedRow::Header { day_start_ms, .. } => groups.push(HistoryGroup {
                label: day_label(*day_start_ms, s),
                rows: Vec::new(),
            }),
            FeedRow::Item { item } => {
                let model = crate::wallet::live::activity_row(view, item, wallet, hidden);
                // A feed that opened with an item rather than a header is not a
                // shape the core produces, but drawing the row is better than
                // dropping somebody's transaction over a missing heading.
                if let Some(group) = groups.last_mut() {
                    group.rows.push(model);
                } else {
                    groups.push(HistoryGroup {
                        label: s.today.clone(),
                        rows: vec![model],
                    });
                }
            }
        }
    }
    // A day header the core emitted with nothing under it would draw as a
    // heading over blank space.
    groups.retain(|group| !group.rows.is_empty());
    groups
}

/// The transaction ids the history panel draws, in render order.
///
/// The page binds one listener per id and `panels::history` hands them out in
/// the same walk, so row N's listener opens row N's transaction. Two walks that
/// could disagree would open the wrong record, which on a money screen is worse
/// than opening nothing.
#[must_use]
pub fn history_ids(view: &FeedView) -> Vec<String> {
    let mut seen_header = false;
    let mut ids = Vec::new();
    let mut pending: Vec<String> = Vec::new();
    for row in &view.rows {
        match row {
            FeedRow::Header { .. } => {
                // The previous group's rows are kept only if it had any — the
                // same retain `history` applies, walked the same way.
                ids.append(&mut pending);
                seen_header = true;
            }
            FeedRow::Item { item } => {
                if seen_header {
                    pending.push(item.id.clone());
                } else {
                    ids.push(item.id.clone());
                }
            }
        }
    }
    ids.append(&mut pending);
    ids
}

/// "Today" / "Yesterday" / the date.
///
/// Compared against the shell's own local day boundary, which is the same
/// function the executor stamps records with — asking two different questions
/// about which day it is here is how a row lands under the wrong heading.
pub(crate) fn day_label(day_start_ms: f64, s: &FlowStrings) -> SharedString {
    let today = crate::executor::day_start_ms(crate::executor::now_ms());
    const DAY_MS: f64 = 86_400_000.0;
    if (day_start_ms - today).abs() < DAY_MS / 2.0 {
        return s.today.clone();
    }
    if (day_start_ms - (today - DAY_MS)).abs() < DAY_MS / 2.0 {
        return s.yesterday.clone();
    }
    #[allow(clippy::cast_possible_truncation, reason = "an epoch in ms")]
    let civil = Civil::from_unix_millis(day_start_ms as i64, 0);
    SharedString::from(vela_core::l10n::datetime::format_date(
        &civil,
        crate::executor::format_prefs::current().date,
    ))
}

/// One transaction, in detail — DA2L / DA3L.
///
/// `None` when the id names nothing: a row can be deleted while its panel is
/// open, and drawing a stale detail over a record that no longer exists is
/// worse than closing the column.
#[must_use]
pub fn tx_detail(
    view: &FeedView,
    id: &str,
    s: &FlowStrings,
    hidden: bool,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> Option<crate::flows::fixtures::TxDetail> {
    let item = view.rows.iter().find_map(|row| match row {
        FeedRow::Item { item } if item.id == id => Some(item),
        _ => None,
    })?;
    let incoming = item.direction == vela_core::app::activity_feed::FeedDirection::In;
    let record = view.transactions.iter().find(|record| record.id == item.id);

    let mut facts = Vec::new();
    // Who it was with. The identicon is seeded by the ADDRESS even when a name
    // is known — the avatar is how somebody checks the name is on the address
    // they meant, so seeding it from the name would defeat its purpose.
    if let Some(counterparty) = item.counterparty.as_ref() {
        let named = item.alias.clone();
        facts.push(FactRow {
            label: if incoming {
                s.detail_from.clone()
            } else {
                s.detail_to.clone()
            },
            value: SharedString::from(
                named
                    .clone()
                    .unwrap_or_else(|| crate::wallet::live::shorten_address(counterparty)),
            ),
            lead: FactLead::Identicon(SharedString::from(counterparty.clone())),
            // A name is prose; an address is a string somebody compares
            // character by character, and that needs the mono face.
            mono: named.is_none(),
            copy: Some(SharedString::from(counterparty.clone())),
            note: None,
        });
    }
    facts.push(FactRow {
        label: s.detail_chain.clone(),
        value: SharedString::from(chain_name(item.chain_id)),
        lead: FactLead::Token(TokenMark {
            ticker: SharedString::from(item.symbol.clone()),
            badge: tint(item.chain_id),
            logos: crate::marks::token_logos(item.chain_id, &item.symbol, None, &[]),
        }),
        mono: false,
        copy: None,
        note: None,
    });
    facts.push(FactRow {
        label: s.detail_date.clone(),
        value: SharedString::from(stamp(item.timestamp, s, locale)),
        lead: FactLead::None,
        mono: false,
        copy: None,
        note: None,
    });
    // Only if there IS one. An empty hash row on an off-chain signature invites
    // "which transaction?" — the same reason the mock omits the contract row on
    // a native transfer.
    if let Some(hash) = item.tx_hash.as_ref().filter(|hash| !hash.is_empty()) {
        facts.push(FactRow {
            label: s.detail_hash.clone(),
            value: SharedString::from(hash.clone()),
            lead: FactLead::None,
            mono: true,
            copy: Some(SharedString::from(hash.clone())),
            note: None,
        });
    }

    let status = record.map_or(FeedTxStatus::Confirmed, |record| record.status);
    let (breakdown_title, breakdown) = detail_parts(item, s);
    Some(crate::flows::fixtures::TxDetail {
        breakdown_title,
        breakdown,
        title: SharedString::from(crate::wallet::fill(
            if incoming {
                &s.tx_label_received
            } else {
                &s.tx_label_sent
            },
            "symbol",
            &item.symbol,
        )),
        status: StatusChip {
            text: match status {
                FeedTxStatus::Confirmed => s.status_confirmed.clone(),
                // A pending or failed transfer must not wear the confirmed
                // chip. The words are the feed's, which already has them.
                FeedTxStatus::Pending => s.status_pending.clone(),
                FeedTxStatus::Failed => s.status_failed.clone(),
            },
            tone: match status {
                FeedTxStatus::Confirmed => StatusTone::Success,
                FeedTxStatus::Pending => StatusTone::Info,
                FeedTxStatus::Failed => StatusTone::Error,
            },
        },
        amount: crate::wallet::live::amount_text_of(item, incoming, hidden),
        // The core already valued it, stablecoin fallback and all. `0` means
        // unknown rather than free, so it shows nothing instead of `$0.00`.
        fiat: if hidden || item.usd_value <= 0.0 {
            SharedString::from("")
        } else {
            SharedString::from(currency.text(item.usd_value, locale))
        },
        positive: incoming,
        facts,
        view_on_explorer: s.view_on_explorer.clone(),
        // The transaction's own page, when it has a hash to find it by (the
        // web's `explorerTxURL`). An off-chain signature has nothing to open.
        explorer_url: item
            .tx_hash
            .as_ref()
            .filter(|hash| !hash.is_empty())
            .map(|hash| SharedString::from(format!("{}/tx/{hash}", explorer_root(item.chain_id)))),
        delete_label: Some(s.delete_record.clone()),
    })
}

/// Where the explorer links point for one chain: the chain's own
/// (`custom_tokens::explorer_base` — the built-in's, or the one the person gave
/// a network they added), else Etherscan — the web's
/// `explorerBaseURL(chainId) ?? FALLBACK_EXPLORER`.
fn explorer_root(chain_id: u32) -> String {
    crate::executor::custom_tokens::explorer_base(chain_id)
        .unwrap_or_else(|| crate::settings::fixtures::ETHEREUM_EXPLORER.to_owned())
}

/// A transaction's wall clock: "Today 11:20", "Yesterday 14:02", or the date.
fn stamp(timestamp_sec: f64, s: &FlowStrings, locale: &str) -> String {
    let epoch_ms = timestamp_sec * 1000.0;
    let civil = crate::executor::local_civil(epoch_ms);
    let clock = format_time(
        &civil,
        crate::executor::format_prefs::current().time,
        locale,
    );
    let day = day_label(crate::executor::day_start_ms(epoch_ms), s);
    format!("{day} {clock}")
}

/// Adding a token by contract address — DT3L.
///
/// The core drives the whole panel: it validates the address, decides when a
/// search may run, holds what was found and knows which of them are already
/// added. This turns that into the drawing.
#[must_use]
pub fn add_token(view: &MtokView, s: &FlowStrings) -> crate::flows::fixtures::AddToken {
    use crate::flows::fixtures::{AddTokenResult, StatusChip, StatusTone};
    let found = view.found.first();
    let typed = !view.input_address.trim().is_empty();
    // The web's `liveAddToken`, state for state (078 F-07): searching and
    // not-found are a line, not a card; a found token is a card, with
    // "Added" when it is already in the wallet; nothing typed says nothing.
    let result = if view.detecting {
        AddTokenResult::Note(s.searching_networks.clone())
    } else if let Some(found) = found {
        AddTokenResult::Token {
            mark: TokenMark {
                ticker: SharedString::from(found.symbol.clone()),
                badge: tint(found.chain_id),
                logos: crate::marks::token_logos(
                    found.chain_id,
                    &found.symbol,
                    view.input_address.as_str().into(),
                    &[],
                ),
            },
            name: SharedString::from(found.name.clone()),
            // The mock's own order: symbol, scale, network. The SCALE is on
            // the card because adding a token at the wrong one renders every
            // amount at the wrong magnitude, and this is the last screen where
            // somebody can notice.
            detail: SharedString::from(format!(
                "{} · {} {} · {}",
                found.symbol, s.label_decimals, found.decimals, found.network_name
            )),
            chip: found.added.then(|| StatusChip {
                text: s.token_added.clone(),
                tone: StatusTone::Success,
            }),
        }
    } else if view.native_alias {
        // A native coin that also answers an ERC-20 interface is FOUND by the
        // probe and refused with a reason (spec 060) — not "not found".
        AddTokenResult::Note(SharedString::from(format!(
            "{} — {}",
            s.native_alias_title, s.native_alias_message
        )))
    } else if view.not_found {
        AddTokenResult::Note(SharedString::from(format!(
            "{} — {}",
            s.not_found_title, s.not_found_message
        )))
    } else {
        AddTokenResult::Empty
    };
    crate::flows::fixtures::AddToken {
        tab_erc20: s.tab_erc20.clone(),
        tab_native: s.tab_native.clone(),
        native: false,
        // No network row: the core searches EVERY network at once and reports
        // the ones where the contract resolved, so there is nothing to pick.
        network: None,
        field_label: s.token_address_label.clone(),
        field_value: SharedString::from(view.input_address.clone()),
        field_placeholder: SharedString::from("0x…"),
        // The field says what is wrong with it: a write that failed, or a
        // string that is not a contract address.
        field_error: if view.save_error {
            Some(s.add_token_error_save.clone())
        } else if typed && !view.address_valid {
            Some(s.invalid_contract.clone())
        } else {
            None
        },
        result,
        notice: None,
        cta: s.add_to_wallet.clone(),
        cta_disabled: found.is_none_or(|found| found.added) || view.saving,
    }
}

/// The native tab — a NETWORK by name or chain ID (the web's
/// `liveAddNetworkTab`, 078 F-07), driven by the same `network_admin` wizard
/// the settings screen's Add Network runs. `added` is the chain this panel
/// just added: the core resets its wizard on the add, so the panel keeps what
/// it confirmed to say so.
#[must_use]
pub fn add_network_tab(
    wizard: &vela_core::app::network_admin::NetWizardView,
    query: &str,
    added: Option<&vela_core::app::network_admin::NetChainInfo>,
    s: &FlowStrings,
) -> crate::flows::fixtures::AddToken {
    use crate::flows::fixtures::{
        AddTokenResult, FactLead, FactRow, NetworkSuggestion, StatusChip, StatusTone,
    };
    use vela_core::app::network_admin::{NetWizardErrorKind, NetWizardPhase};

    let mark_of = |chain_id: u32, symbol: &str| TokenMark {
        ticker: SharedString::from(symbol.to_owned()),
        badge: tint(chain_id),
        logos: crate::marks::token_logos(chain_id, symbol, None, &[]),
    };
    let facts = |chain_id: u32, symbol: &str| {
        vec![
            FactRow {
                label: s.label_chain_id.clone(),
                value: SharedString::from(chain_id.to_string()),
                lead: FactLead::None,
                mono: false,
                copy: None,
                note: None,
            },
            FactRow {
                label: s.label_native_token.clone(),
                value: SharedString::from(symbol.to_owned()),
                lead: FactLead::None,
                mono: false,
                copy: None,
                note: None,
            },
        ]
    };
    let info = wizard.chain_info.as_ref();
    let card = |text: SharedString, tone: StatusTone, link: Option<SharedString>| {
        let (chain_id, name, symbol) = match info {
            Some(info) => (info.chain_id, info.name.clone(), info.native_symbol.clone()),
            None => (0, query.to_owned(), String::new()),
        };
        AddTokenResult::Network {
            mark: mark_of(chain_id, &symbol),
            name: SharedString::from(name),
            chip: StatusChip { text, tone },
            link,
            facts: if info.is_some() {
                facts(chain_id, &symbol)
            } else {
                Vec::new()
            },
        }
    };
    let not_found = || {
        AddTokenResult::Note(SharedString::from(
            s.net_picker_empty.replace("{{query}}", query),
        ))
    };
    let incompatible = || {
        card(
            s.not_compatible.clone(),
            StatusTone::Error,
            Some(SharedString::from(format!(
                "{} · {}",
                s.error_not_compatible, s.deploy_contracts
            ))),
        )
    };

    let mut can_add = false;
    let result = if let Some(added) = added {
        AddTokenResult::Network {
            mark: mark_of(added.chain_id, &added.native_symbol),
            name: SharedString::from(added.name.clone()),
            chip: StatusChip {
                text: s.network_added.clone(),
                tone: StatusTone::Success,
            },
            link: None,
            facts: facts(added.chain_id, &added.native_symbol),
        }
    } else if query.trim().is_empty() {
        AddTokenResult::Empty
    } else {
        match wizard.phase {
            NetWizardPhase::Searching => AddTokenResult::Note(s.searching_networks.clone()),
            NetWizardPhase::Idle | NetWizardPhase::Suggested => {
                if wizard.suggestions.is_empty() {
                    not_found()
                } else {
                    AddTokenResult::Suggestions(
                        wizard
                            .suggestions
                            .iter()
                            .map(|entry| NetworkSuggestion {
                                chain_id: entry.chain_id,
                                mark: mark_of(entry.chain_id, &entry.native_currency_symbol),
                                name: SharedString::from(entry.name.clone()),
                                meta: SharedString::from(format!(
                                    "{} {}",
                                    s.label_chain_id, entry.chain_id
                                )),
                            })
                            .collect(),
                    )
                }
            }
            NetWizardPhase::Resolving | NetWizardPhase::Checking => {
                card(s.searching_networks.clone(), StatusTone::Info, None)
            }
            NetWizardPhase::Error => match &wizard.error {
                None | Some(NetWizardErrorKind::NotFound { .. }) => not_found(),
                Some(NetWizardErrorKind::AlreadyAdded { chain_id }) => {
                    let symbol = info.map_or_else(
                        || native_symbol(*chain_id),
                        |info| info.native_symbol.clone(),
                    );
                    AddTokenResult::Network {
                        mark: mark_of(*chain_id, &symbol),
                        name: SharedString::from(
                            info.map_or_else(|| chain_name(*chain_id), |info| info.name.clone()),
                        ),
                        chip: StatusChip {
                            text: s.network_added.clone(),
                            tone: StatusTone::Success,
                        },
                        link: None,
                        facts: facts(*chain_id, &symbol),
                    }
                }
                Some(_) => incompatible(),
            },
            NetWizardPhase::Checked => match &wizard.compat {
                Some(compat) if compat.rpc_failure.is_none() && compat.compatible => {
                    can_add = wizard.can_add;
                    card(s.compatible.clone(), StatusTone::Success, None)
                }
                Some(compat) if compat.rpc_failure.is_none() => incompatible(),
                // Inconclusive is never "not compatible" (invariant ③).
                _ => card(s.unable_to_verify.clone(), StatusTone::Warning, None),
            },
        }
    };

    crate::flows::fixtures::AddToken {
        tab_erc20: s.tab_erc20.clone(),
        tab_native: s.tab_native.clone(),
        native: true,
        network: None,
        field_label: s.net_search_label.clone(),
        field_value: SharedString::from(query.to_owned()),
        field_placeholder: s.net_search_placeholder.clone(),
        field_error: None,
        result,
        notice: None,
        cta: s.add_network_btn.clone(),
        cta_disabled: !can_add,
    }
}

// ---------------------------------------------------------------------------
// Receive — DR1L / DR2L
// ---------------------------------------------------------------------------

/// Every network this wallet can be paid on, each showing the person's own
/// address.
///
/// One address across every EVM chain is the whole point of the Safe: the rows
/// differ by network, never by address, and a person who reads two of them must
/// see the same string twice.
#[must_use]
pub fn receive_list(address: &str, s: &FlowStrings) -> ReceiveList {
    let rows: Vec<NetworkRow> = receivable_chains()
        .into_iter()
        .map(|(chain_id, symbol)| NetworkRow {
            name: SharedString::from(chain_name(chain_id)),
            code: SharedString::from(symbol),
            badge: tint(chain_id),
            address: SharedString::from(shorten(address)),
            address_full: SharedString::from(address.to_owned()),
        })
        .collect();
    ReceiveList {
        subtitle: SharedString::from(crate::wallet::fill(
            &s.networks_line,
            "count",
            &rows.len().to_string(),
        )),
        search_placeholder: s.receive_search.clone(),
        empty_text: s.search_empty.clone(),
        rows,
    }
}

/// One network's QR, with the person's real address under it.
#[must_use]
pub fn receive_qr(
    address: &str,
    name: &str,
    chain_id: u32,
    watch: &ReceiveWatchView,
    pay: &PaymentRequestView,
    s: &FlowStrings,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> ReceiveQr {
    let network = chain_name(chain_id);
    let symbol = receivable_chains()
        .into_iter()
        .find(|(id, _)| *id == chain_id)
        .map_or_else(|| network.clone(), |(_, symbol)| symbol);
    let lines = address_lines(address);
    ReceiveQr {
        title: SharedString::from(crate::wallet::fill(
            &s.qr_title_network,
            "network",
            &network,
        )),
        // The contract line is DR3L's — an ASSET's QR names the token it is
        // for. A network QR has no contract, and inventing one would put a
        // token address under a code that is not for a token.
        contract: None,
        account: AddressCard {
            name: SharedString::from(name.to_owned()),
            // The identicon seed is the ADDRESS, not the name: two accounts a
            // person named the same thing must not draw the same avatar, and an
            // avatar is how somebody checks they are looking at the right one.
            seed: SharedString::from(address.to_owned()),
            lines: (SharedString::from(lines.0), SharedString::from(lines.1)),
        },
        // WHAT the code says is the core's decision, not this file's: in
        // address mode `qr_value` is the bare recipient, and in request mode it
        // is the EIP-681 URI with the amount in it. Encoding the address here
        // would work today and silently ignore an amount the moment the request
        // builder lands.
        qr_payload: (!pay.qr_value.is_empty()).then(|| SharedString::from(pay.qr_value.clone())),
        // Covered until this account has read the warning once. `gate_loading`
        // keeps it covered while the flag is still being read, so a first
        // visit never flashes the code and then hides it.
        gate: (!pay.acknowledged).then(|| ReceiveGate {
            title: s.warning_title.clone(),
            body: s.warning_body.clone(),
            counterfactual: s.warning_counterfactual.clone(),
            confirm: s.warning_confirm.clone(),
            loading: pay.gate_loading,
        }),
        // The core's own answer, not "is there a payload": an address may be
        // ready to copy long before anybody has been told which networks it
        // is safe on.
        can_copy: pay.can_copy,
        // A NETWORK code wears the network's own logo (the web's `chainMark`).
        // The native coin's rule would put Ethereum's mark in the middle of a
        // Base or Arbitrum code — the one picture on this screen that says
        // which network the money should arrive on.
        centre: TokenMark {
            ticker: SharedString::from(symbol.clone()),
            badge: tint(chain_id),
            logos: crate::marks::chain_logos(chain_id),
        },
        warning: s.warning_reminder.clone(),
        save_image: s.save_image.clone(),
        view_on_explorer: s.view_on_explorer.clone(),
        // This account on this network's explorer — the web's
        // `explorerAddressURL`, the same for a network's code and a token's.
        explorer_url: (!address.is_empty())
            .then(|| SharedString::from(format!("{}/address/{address}", explorer_root(chain_id)))),
        contract_copy: None,
        deposits: deposits(watch, locale, currency),
    }
}

/// DR3L — one held token's code, reached from its detail panel.
///
/// The same code as the network's (one address on every chain), about the
/// token instead: its symbol in the title, its contract above the account
/// card, its logo in the centre — the web's `liveReceiveQr` asset variant.
#[must_use]
pub fn receive_token_qr(
    address: &str,
    name: &str,
    token: &BalanceToken,
    watch: &ReceiveWatchView,
    pay: &PaymentRequestView,
    s: &FlowStrings,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> ReceiveQr {
    let mut qr = receive_qr(
        address,
        name,
        token.chain_id,
        watch,
        pay,
        s,
        locale,
        currency,
    );
    qr.title = SharedString::from(fill(
        &fill(&s.qr_title_asset, "symbol", &token.symbol),
        "network",
        &chain_name(token.chain_id),
    ));
    qr.contract = Some((
        s.token_contract.clone(),
        match token.token_address.as_deref() {
            Some(contract) => SharedString::from(shorten(contract)),
            None => s.label_native_token.clone(),
        },
    ));
    // The copy beside that line copies the whole contract, not the shortened
    // one it shows. The chain's own coin has none to copy.
    qr.contract_copy = token.token_address.clone().map(SharedString::from);
    qr.centre = TokenMark {
        ticker: SharedString::from(token.symbol.clone()),
        badge: tint(token.chain_id),
        logos: crate::marks::token_logos(
            token.chain_id,
            &token.symbol,
            token.token_address.as_deref(),
            &[],
        ),
    };
    qr
}

/// What landed while this code was open.
///
/// The core decides WHAT counts as a deposit — the baseline, the comparison,
/// the debounce. This turns its verdict into words: the local wall clock, the
/// amount with its sign, and the network and value beside it.
///
/// `detected` gates the section rather than `deposits.is_empty()`, because they
/// are the core's two separate answers and only the first means "announce
/// this". A list with nothing in it is not a celebration.
fn deposits(
    view: &ReceiveWatchView,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> Vec<FlowDeposit> {
    if !view.detected {
        return Vec::new();
    }
    view.deposits
        .iter()
        .map(|entry| FlowDeposit {
            time: SharedString::from(format_time(
                &crate::executor::local_civil(entry.at_epoch_ms),
                crate::executor::format_prefs::current().time,
                locale,
            )),
            rows: entry
                .items
                .iter()
                .map(|item| {
                    (
                        SharedString::from(format!(
                            "+{} {}",
                            format_token_amount(
                                item.amount,
                                crate::executor::format_prefs::current().number,
                                false
                            ),
                            item.symbol
                        )),
                        SharedString::from(match item.usd {
                            // An unpriced arrival still says which chain it came
                            // in on. Printing `$0.00` beside it would be the
                            // assets panel's mistake on a happier screen.
                            Some(usd) => format!(
                                "{}  {}",
                                chain_name(item.chain_id),
                                currency.text(usd, locale)
                            ),
                            None => chain_name(item.chain_id),
                        }),
                    )
                })
                .collect(),
        })
        .collect()
}

/// The chains a payment can arrive on: the built-ins plus whatever the person
/// added. The same list the balance fetch reads, for the same reason — a
/// network nobody can be paid on is a network nobody has.
pub fn receivable_chains() -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = BUILTIN_CHAINS
        .iter()
        .map(|chain| (chain.chain_id, chain.native_symbol.to_owned()))
        .collect();
    if let Ok(Some(serde_json::Value::Array(items))) =
        crate::executor::storage::read_value(crate::executor::storage::KEY_CUSTOM_NETWORKS)
    {
        for item in items {
            let Some(chain_id) = item
                .get("chainId")
                .and_then(serde_json::Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
            else {
                continue;
            };
            if out.iter().any(|(id, _)| *id == chain_id) {
                continue;
            }
            out.push((
                chain_id,
                item.get("nativeSymbol")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            ));
        }
    }
    out
}

fn shorten(address: &str) -> String {
    if address.len() <= 14 {
        return address.to_owned();
    }
    format!("{}…{}", &address[..6], &address[address.len() - 4..])
}

// ---------------------------------------------------------------------------
// Send (DSD1L–DSD4L, DSD2eL, DSD2fL) — spec 032
// ---------------------------------------------------------------------------

/// Everything the send screens are filled from. The amounts are the core's
/// strings, the gate is `can_continue` / `can_confirm`, the stage is `stage`,
/// and the words are the corpus's. Nothing here decides anything; what this
/// half owns is wording and formatting — which template a value goes into,
/// and how a fee reads.
pub struct SendInputs<'a> {
    pub send: &'a SendView,
    pub fee: &'a FeeView,
    pub s: &'a FlowStrings,
    pub wallet: &'a crate::wallet::WalletStrings,
    pub locale: &'a str,
    /// The currency every `≈` figure on these screens is drawn in. Beside the
    /// locale because it travels with it: both say how a number is written,
    /// and until 2026-09-23 this one was the constant `"USD"` at seven sites
    /// while the wallet home had learned to convert.
    pub money: &'a crate::wallet::live::Money,
    pub identity_name: &'a str,
    pub identity_address: &'a str,
    /// The speed control (spec 068), as the `fee_speed` core decided it.
    /// `None` draws none.
    pub speed: Option<&'a SpeedInputs>,
}

/// The speed core's view, and the fee session pricing each tier — whose
/// fee-coin options format that option's fee, as the fee row formats its own.
pub struct SpeedInputs {
    pub view: FeeSpeedView,
    pub tier_views: Vec<(FeeTier, FeeView)>,
}

impl SpeedInputs {
    fn view_of(&self, tier: FeeTier) -> Option<&FeeView> {
        self.tier_views
            .iter()
            .find(|(candidate, _)| *candidate == tier)
            .map(|(_, view)| view)
    }
}

/// A tier's NAME — the speed itself, never a number. `rapid` is dead (spec
/// 068) and reads as the factory `fast`, the core's own answer for it.
fn tier_name(s: &FlowStrings, tier: FeeTier) -> SharedString {
    match tier {
        FeeTier::Standard => s.gas_tier_standard.clone(),
        FeeTier::Slow => s.gas_tier_slow.clone(),
        FeeTier::Fast | FeeTier::Rapid => s.gas_tier_fast.clone(),
    }
}

/// …and what it buys, the line under the name.
fn tier_hint(s: &FlowStrings, tier: FeeTier) -> SharedString {
    match tier {
        FeeTier::Standard => s.gas_tier_hint_standard.clone(),
        FeeTier::Slow => s.gas_tier_hint_slow.clone(),
        FeeTier::Fast | FeeTier::Rapid => s.gas_tier_hint_fast.clone(),
    }
}

fn native_symbol(chain_id: u32) -> String {
    BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(String::new, |chain| chain.native_symbol.to_owned())
}

fn trimmed(amount: f64) -> String {
    format_token_amount(
        amount,
        crate::executor::format_prefs::current().number,
        false,
    )
}

fn fiat_line(
    usd: Option<f64>,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> Option<SharedString> {
    usd.map(|usd| SharedString::from(format!("≈ {}", currency.text(usd, locale))))
}

/// A settled estimate as one line: the fee coin's amount. `—` while there is
/// no quote — the drawn row shows the label alone rather than a number nobody
/// has agreed to yet.
/// A fee estimate as the words a screen shows.
///
/// `pub(crate)` and shared with the signing sheet: two formatters would be two
/// answers about what a transaction costs, on two screens that price the same
/// operation.
pub(crate) fn fee_text(fee: Option<&FeeEstimateView>) -> String {
    let Some(fee) = fee else {
        return "—".to_owned();
    };
    match &fee.fee_asset {
        FeeAssetView::Erc20 {
            amount,
            decimals,
            symbol,
            ..
        } => {
            let units = amount.parse::<f64>().unwrap_or(0.0) / 10f64.powi(*decimals as i32);
            format!("{} {}", trimmed(units), symbol.clone().unwrap_or_default())
                .trim()
                .to_owned()
        }
        FeeAssetView::Native => {
            let coin = fee.total_wei.parse::<f64>().unwrap_or(0.0) / 1e18;
            format!("{} {}", trimmed(coin), native_symbol(fee.chain_id))
                .trim()
                .to_owned()
        }
    }
}

/// Below this the coin amount is the honest primary and the fiat half is left
/// off (`03-domain-components.md` §3.1): a real fee rounded to "$0.00" reads
/// as free, which is a worse answer than no figure at all.
const FEE_FIAT_MIN_USD: f64 = 0.005;

/// The whole-token figure a price multiplies, and which coin to price.
fn fee_units(fee: &FeeEstimateView) -> (f64, Option<String>) {
    match &fee.fee_asset {
        FeeAssetView::Erc20 {
            amount,
            decimals,
            token,
            ..
        } => (
            amount.parse::<f64>().unwrap_or(0.0) / 10f64.powi(*decimals as i32),
            Some(token.clone()),
        ),
        FeeAssetView::Native => (fee.total_wei.parse::<f64>().unwrap_or(0.0) / 1e18, None),
    }
}

/// The unit price, in USD, of the coin a quote is denominated in.
///
/// The relay's published row first — it priced the quote, so its number is the
/// one the estimate converted through — then the balances the form already
/// carries, which is where the amount's own "≈" line gets its price. `None`
/// when neither knows the coin: a fee row that invents a price is worse than
/// one that shows only the coin.
pub(crate) fn fee_price_usd(
    contract: Option<&str>,
    chain_id: u32,
    send: Option<&SendView>,
    fee: &FeeView,
) -> Option<f64> {
    let same = |other: Option<&str>| match (contract, other) {
        (None, None) => true,
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        _ => false,
    };
    let published = fee
        .options
        .iter()
        .find(|option| same(option.contract.as_deref()))
        .and_then(|option| option.usd_price.as_deref())
        .and_then(|price| price.parse::<f64>().ok())
        .filter(|price| *price > 0.0);
    if published.is_some() {
        return published;
    }
    let send = send?;
    send.selected_token
        .iter()
        .chain(send.tokens.iter())
        .find(|token| token.chain_id == chain_id && same(token.token_address.as_deref()))
        .and_then(|token| token.price_usd)
        .filter(|price| *price > 0.0)
}

/// "0.0021 XDAI · ≈$0.55" — the quote's own amount, and what it costs
/// (issue 201).
///
/// The fee was the one figure on the send screens with no money beside it, so
/// a person who does not track the coin's price could not tell what a transfer
/// cost. The amount is never re-derived here; only the price is looked up.
pub(crate) fn fee_line(
    quote: Option<&FeeEstimateView>,
    send: Option<&SendView>,
    fee: &FeeView,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> String {
    let coin = fee_text(quote);
    let Some(quote) = quote else { return coin };
    let (units, contract) = fee_units(quote);
    let Some(price) = fee_price_usd(contract.as_deref(), quote.chain_id, send, fee) else {
        return coin;
    };
    let usd = units * price;
    if !usd.is_finite() || usd < FEE_FIAT_MIN_USD {
        return coin;
    }
    format!("{coin} · ≈{}", currency.text(usd, locale))
}

/// The fee coin's symbol, for the row's mark.
fn fee_symbol(send: &SendView, fee: &FeeView) -> (String, u32) {
    let quote = send.fee.as_ref().or(fee.fee.as_ref());
    let chain_id = send
        .selected_token
        .as_ref()
        .map(|token| token.chain_id)
        .or_else(|| quote.map(|quote| quote.chain_id))
        .unwrap_or(1);
    let symbol = match quote.map(|quote| &quote.fee_asset) {
        Some(FeeAssetView::Erc20 { symbol, .. }) => {
            symbol.clone().unwrap_or_else(|| native_symbol(chain_id))
        }
        _ => native_symbol(chain_id),
    };
    (symbol, chain_id)
}

fn send_fee_row(i: &SendInputs<'_>) -> FeeRow {
    let (symbol, chain_id) = fee_symbol(i.send, i.fee);
    let in_hand = i.send.fee.as_ref().or(i.fee.fee.as_ref());
    // NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681). On the
    // path where a pick re-measures, the estimate in hand still belongs to the
    // speed just left — the send machine keeps it across a tier change — and
    // "…" is the honest thing to show until this speed's own figure lands.
    let of_another_tier = in_hand.zip(i.speed).is_some_and(|(fee, speed)| {
        vela_core::app::fee_speed::offered(fee.tier) != speed.view.tier
    });
    let quote = in_hand.filter(|_| !of_another_tier);
    let measuring = i.send.fee_busy || i.fee.busy;
    FeeRow {
        // A figure the relay did not quote — a local fallback from defaults —
        // is an ESTIMATE and is labelled as one (spec 038 finding 14).
        label: if quote.is_some_and(|fee| !fee.quoted) {
            i.s.fee_token_estimate.clone()
        } else {
            i.s.network_fee.clone()
        },
        mark: TokenMark {
            ticker: symbol.clone().into(),
            badge: tint(chain_id),
            logos: crate::marks::token_logos(
                chain_id,
                &symbol,
                quote.and_then(|quote| match &quote.fee_asset {
                    FeeAssetView::Erc20 { token, .. } => Some(token.as_str()),
                    FeeAssetView::Native => None,
                }),
                &[],
            ),
        },
        value: if measuring || of_another_tier {
            i.s.fee_pending.clone()
        } else {
            SharedString::from(fee_line(quote, Some(i.send), i.fee, i.locale, i.money))
        },
        refresh: i.speed.map(|_| i.s.fee_refresh.clone()),
        // A measurement is out — whoever started it — the same fact the "…"
        // reads, so the row never claims to be settled and measuring at once.
        refreshing: measuring,
        // `FeeView.stale` had no consumer on the desktop: the 30 s TTL ran out
        // and nothing said so. Not while a fresh measurement is out ("old" is
        // about to stop being true), and not over a row with no figure of its
        // own on it — "from a while ago" is a fact about a number.
        stale_note: (i.fee.stale && !measuring && !of_another_tier && quote.is_some())
            .then(|| i.s.fee_stale.clone()),
    }
}

/// The folded speed control (spec 068), drawn from the `fee_speed` core's
/// view (spec 069). Every figure is that tier's OWN settled quote, echoed by
/// the core; only the words and the fee line are made here.
fn send_speed(i: &SendInputs<'_>) -> Option<Box<FeeSpeedModel>> {
    Some(Box::new(speed_model(
        i.speed?,
        i.s,
        Some(i.send),
        i.fee,
        i.locale,
        i.money,
    )))
}

/// The same control for any fee surface — the dApp signing sheet draws it too
/// (spec 069), with no send view: each option is then written the way that
/// sheet writes its own fee row.
#[must_use]
pub fn speed_model(
    speed: &SpeedInputs,
    s: &FlowStrings,
    send: Option<&SendView>,
    fee: &FeeView,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> FeeSpeedModel {
    let view = &speed.view;
    FeeSpeedModel {
        label: s.fee_speed_label.clone(),
        // THEIR default (or their pick for this send), never a hardcoded one.
        value: tier_name(s, view.tier),
        open: view.open,
        once_note: s.fee_speed_once.clone(),
        free_note: view.free_note.then(|| s.fee_speed_free.clone()),
        single_note: view.single.then(|| s.fee_speed_single.clone()),
        gas_price_label: s.gas_price_label.clone(),
        gas_price_line: view.gas_price_line,
        options: view
            .options
            .iter()
            .map(|option| {
                let value = match (&option.fee, speed.view_of(option.tier)) {
                    (Some(quote), Some(tier_view)) => {
                        SharedString::from(fee_line(Some(quote), send, tier_view, locale, currency))
                    }
                    (Some(quote), None) => {
                        SharedString::from(fee_line(Some(quote), send, fee, locale, currency))
                    }
                    // "…" while this tier's own quote is out, "—" when there
                    // is none to be had.
                    (None, _) if option.measuring => s.fee_pending.clone(),
                    (None, _) => SharedString::from("—"),
                };
                FeeSpeedOption {
                    label: tier_name(s, option.tier),
                    detail: tier_hint(s, option.tier),
                    value,
                    gas_price: option.gas_price.clone().map(SharedString::from),
                    selected: option.selected,
                }
            })
            .collect(),
    }
}

/// A token row for the picker: the balance the core carries, priced by it too.
fn send_token_row(
    token: &SendToken,
    wallet: &crate::wallet::WalletStrings,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> AssetRowModel {
    let amount = token.balance.parse::<f64>().unwrap_or(0.0);
    AssetRowModel {
        logos: crate::marks::token_logos(
            token.chain_id,
            &token.symbol,
            token.token_address.as_deref(),
            &token.logo_urls,
        ),
        ticker: SharedString::from(token.symbol.clone()),
        chain: SharedString::from(chain_name(token.chain_id)),
        badge: tint(token.chain_id),
        balance: SharedString::from(trimmed(amount)),
        fiat: match token.price_usd {
            None => Fiat::NoPrice(wallet.no_price.clone()),
            Some(price) => Fiat::Value(SharedString::from(currency.text(amount * price, locale))),
        },
    }
}

/// SD1's chips: all, the stables, the chains' own coins, the rest — in the
/// order they are drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SendClass {
    #[default]
    All,
    Stable,
    Gas,
    Other,
}

impl SendClass {
    pub const CHIPS: [Self; 4] = [Self::All, Self::Stable, Self::Gas, Self::Other];

    /// Which chip a token answers to — the web's `sendTokenClass`, word for
    /// word. A chain's native coin is what pays its gas; a stable is one by
    /// the symbol table the core's activity feed keeps; everything else is
    /// "other". One rule, so the chip and the row can never disagree.
    #[must_use]
    pub fn of(token: &SendToken) -> Self {
        if token.token_address.is_none() {
            Self::Gas
        } else if vela_core::app::activity_feed::is_stable(&token.symbol) {
            Self::Stable
        } else {
            Self::Other
        }
    }
}

/// The picker's rows after both narrowings, in the core's order: the
/// sidebar's network (the desktop's one network filter — the picker has no
/// second) and the chip.
///
/// Applied to the VIEW, before anything is read from it, so the rows drawn,
/// the listener each row gets and the scope of "select all valuable" are the
/// same list. Narrowing only the drawing would hand row 2's click to whatever
/// token was third before the filter — here, a transfer of the wrong coin.
pub fn narrow_send_tokens(view: &mut SendView, chain: Option<u32>, class: SendClass) {
    view.tokens.retain(|token| {
        chain.is_none_or(|chain| token.chain_id == chain)
            && (class == SendClass::All || SendClass::of(token) == class)
    });
}

/// DSD1L — which token to send, in one of the picker's two modes.
///
/// The rows are the core's holdings, already narrowed (`narrow_send_tokens`);
/// `class` only says which chip is lit.
///
/// `sweeping` is the SHELL's flag, and deliberately: the core's
/// `multi_select_mode` flips only when a selection is CONFIRMED, so before
/// that there is nothing in the view that says whether the checkboxes are
/// showing. Which tokens may be picked, what "all valuable" means and what a
/// sweep moves are all still the core's — the web's port records the same
/// split in the same words (`live-send.ts` `sweepPicking`).
#[must_use]
pub fn send_pick_with(i: &SendInputs<'_>, sweeping: bool, class: SendClass) -> SendPick {
    let s = i.s;
    let mut pick = SendPick {
        selection: None,
        cta_accent: false,
        search_placeholder: s.send_search.clone(),
        no_match: s.no_matching_tokens.clone(),
        filters: SendClass::CHIPS
            .iter()
            .map(|chip| FilterChip {
                label: match chip {
                    SendClass::All => s.filter_all.clone(),
                    SendClass::Stable => s.filter_stable.clone(),
                    SendClass::Gas => s.filter_gas.clone(),
                    SendClass::Other => s.filter_other.clone(),
                },
                selected: *chip == class,
            })
            .collect(),
        rows: i
            .send
            .tokens
            .iter()
            .map(|token| send_token_row(token, i.wallet, i.locale, i.money))
            .collect(),
        cta: s.multi_send_title.clone(),
    };
    if !sweeping {
        return pick;
    }

    let chain = i.send.multi_chain_id;
    let picked = &i.send.multi_selected_ids;
    pick.selection = Some(crate::flows::fixtures::SendSelection {
        selected: i
            .send
            .tokens
            .iter()
            .map(|token| picked.contains(&token.id()))
            .collect(),
        // Dimmed, not dropped: a batch is one chain, and the tokens on the
        // others are still this person's.
        dimmed: i
            .send
            .tokens
            .iter()
            .map(|token| chain.is_some_and(|id| token.chain_id != id))
            .collect(),
        select_all: s.select_all_valuable.clone(),
        notice: chain.map(|chain_id| {
            let name = crate::executor::custom_tokens::network_name(chain_id);
            (
                crate::settings::model::chain_tint(u64::from(chain_id)).unwrap_or(0x8A_8F_98),
                SharedString::from(crate::settings::model::lettermark(&name)),
                SharedString::from(crate::wallet::fill(
                    &s.multi_send_chain_notice,
                    "network",
                    &name,
                )),
            )
        }),
    });
    if !picked.is_empty() {
        pick.cta_accent = true;
        pick.cta = SharedString::from(crate::wallet::fill(
            &crate::wallet::fill(&s.multi_send_continue, "n", &picked.len().to_string()),
            "chain",
            &chain.map_or_else(String::new, crate::executor::custom_tokens::network_name),
        ));
    }
    pick
}

#[cfg(test)]
mod sweep_tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::send::{Send as SendMachine, SendToken, SendView};

    fn token(chain_id: u32, symbol: &str, address: Option<&str>) -> SendToken {
        SendToken {
            network: format!("chain-{chain_id}"),
            chain_id,
            symbol: symbol.to_owned(),
            balance: "10".to_owned(),
            decimals: 18,
            token_address: address.map(str::to_owned),
            price_usd: Some(1.0),
            logo_urls: Vec::new(),
            spam: false,
        }
    }

    /// A real `SendView` with the sweep fields substituted — the same shape
    /// `wallet::live`'s tests use, and for the same reason: this view has
    /// dozens of fields with their own invariants, and a literal I typed would
    /// be a guess about them.
    fn view_with(tokens: Vec<SendToken>, picked: Vec<String>, chain: Option<u32>) -> SendView {
        let host = CoreHost::<SendMachine>::new();
        SendView {
            tokens,
            multi_selected_ids: picked,
            multi_chain_id: chain,
            ..host.view()
        }
    }

    fn inputs<'a>(
        send: &'a SendView,
        fee: &'a vela_core::app::fee_policy::FeeView,
        s: &'a FlowStrings,
        wallet: &'a crate::wallet::WalletStrings,
    ) -> SendInputs<'a> {
        SendInputs {
            send,
            fee,
            s,
            wallet,
            locale: "en-US",
            money: crate::wallet::live::Money::usd(),
            identity_name: "MultiTest",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
        }
    }

    /// The chips narrow by the web's rule, the sidebar's network narrows with
    /// them, and the lit chip is the one the person pressed.
    #[test]
    fn the_picker_narrows_by_class_and_by_the_sidebars_network() {
        let tokens = vec![
            token(1, "ETH", None),
            token(1, "USDT", Some("0xaa")),
            token(1, "PEPE", Some("0xbb")),
            token(100, "xDAI", None),
            token(100, "USDC", Some("0xcc")),
        ];
        let symbols = |class, chain| {
            let mut view = view_with(tokens.clone(), Vec::new(), None);
            narrow_send_tokens(&mut view, chain, class);
            view.tokens
                .iter()
                .map(|token| token.symbol.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(symbols(SendClass::All, None).len(), 5);
        assert_eq!(symbols(SendClass::Gas, None), ["ETH", "xDAI"]);
        assert_eq!(symbols(SendClass::Stable, None), ["USDT", "USDC"]);
        assert_eq!(symbols(SendClass::Other, None), ["PEPE"]);
        assert_eq!(symbols(SendClass::All, Some(100)), ["xDAI", "USDC"]);
        assert_eq!(symbols(SendClass::Stable, Some(100)), ["USDC"]);

        crate::executor::storage::tests::with_temp_state("send-chips", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let view = view_with(tokens, Vec::new(), None);
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::Gas);
            let lit: Vec<bool> = pick.filters.iter().map(|chip| chip.selected).collect();
            assert_eq!(lit, [false, false, true, false]);
        });
    }

    /// The sweep picker says which rows are ticked, which are on the wrong
    /// chain, and how many are going — all of it read from the core's view.
    ///
    /// The greying is the part worth a test: a batch is one chain, and the
    /// rows on the others stay ON SCREEN. A list that silently shortened when
    /// somebody ticked a token would read as a bug, and it is the person's own
    /// money that would appear to have gone.
    #[test]
    fn a_sweep_ticks_what_is_picked_and_greys_the_other_chains() {
        crate::executor::storage::tests::with_temp_state("send-sweep", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let tokens = vec![
                token(100, "xDAI", None),
                token(100, "USDC", Some("0xdd")),
                token(1, "ETH", None),
            ];
            let ids: Vec<String> = tokens.iter().map(SendToken::id).collect();

            // Not sweeping: the one-token list, and no selection at all.
            let plain = view_with(tokens.clone(), Vec::new(), None);
            let pick = send_pick_with(&inputs(&plain, &fee, &s, &wallet), false, SendClass::All);
            assert!(pick.selection.is_none());
            assert!(!pick.cta_accent);
            assert_eq!(pick.cta, s.multi_send_title);

            // Sweeping, nothing picked yet: ticks are showing, nothing is
            // dimmed (no chain is pinned), and the CTA is still the quiet one.
            let empty = view_with(tokens.clone(), Vec::new(), None);
            let pick = send_pick_with(&inputs(&empty, &fee, &s, &wallet), true, SendClass::All);
            let selection = pick
                .selection
                .as_ref()
                .unwrap_or_else(|| unreachable!("sweeping"));
            assert_eq!(selection.selected, vec![false, false, false]);
            assert_eq!(selection.dimmed, vec![false, false, false]);
            assert!(selection.notice.is_none(), "no chain named yet");
            assert!(!pick.cta_accent);

            // Two picked on Gnosis: those two ticked, Ethereum's row dimmed
            // (still listed), the chain named, and the CTA counting.
            let picked = view_with(tokens, vec![ids[0].clone(), ids[1].clone()], Some(100));
            let pick = send_pick_with(&inputs(&picked, &fee, &s, &wallet), true, SendClass::All);
            let selection = pick
                .selection
                .as_ref()
                .unwrap_or_else(|| unreachable!("sweeping"));
            assert_eq!(selection.selected, vec![true, true, false]);
            assert_eq!(selection.dimmed, vec![false, false, true]);
            let (_, letter, text) = selection
                .notice
                .clone()
                .unwrap_or_else(|| unreachable!("a chain is pinned"));
            assert_eq!(letter, "G");
            assert!(text.contains("Gnosis"), "{text}");
            assert!(!text.contains("{{"), "unfilled template: {text}");
            assert!(pick.cta_accent);
            assert!(pick.cta.contains('2'), "the count: {}", pick.cta);
            assert!(pick.cta.contains("Gnosis"), "the chain: {}", pick.cta);
        });
    }
}

#[cfg(test)]
mod treasury_tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::send::{
        Send as SendMachine, SendTreasuryAsset, SendTreasuryStatus, SendView,
    };

    /// The relay cannot pay on this chain — and there is now a way out of
    /// saying so.
    ///
    /// The notice itself has been right since 032 phase 31: the address to
    /// fund, the shortfall, and a retry. What it never had was a way to leave
    /// it. `DismissTreasurySheet` has been in the machine since it was written
    /// and nothing in this shell ever sent it, so the only exit from a stop
    /// that clears on its own schedule was abandoning the send.
    #[test]
    fn the_treasury_stop_can_be_left() {
        crate::executor::storage::tests::with_temp_state("treasury-notice", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let host = CoreHost::<SendMachine>::new();
            let view = SendView {
                treasury_bootstrap: Some(SendTreasuryStatus {
                    chain_id: 100,
                    address: "0xTreasury".to_owned(),
                    asset: SendTreasuryAsset::Native,
                    // Raw base units, as the relay reports them: a 0.02 xDAI
                    // floor against an empty float.
                    balance: "0".to_owned(),
                    floor: "20000000000000000".to_owned(),
                    bootstrap_needed: true,
                    // Gnosis ships with Vela, so its relayer is the operator's
                    // to refill — the core says so when it publishes the sheet.
                    operator_served: true,
                }),
                ..host.view()
            };
            let inputs = SendInputs {
                send: &view,
                fee: &fee,
                s: &s,
                wallet: &wallet,
                locale: "en-US",
                money: crate::wallet::live::Money::usd(),
                identity_name: "MultiTest",
                identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                speed: None,
            };

            let notice = send_notice(&inputs, false).unwrap_or_else(|| unreachable!("a stop"));
            // The retry the core offers, and now the way out of the card.
            assert_eq!(notice.action.as_ref(), Some(&s.funding_check_now));
            assert_eq!(notice.dismiss.as_ref(), Some(&s.funding_close));
            assert_eq!(
                notice_way_out(&inputs, false),
                Some(NoticeWayOut::RetryAfterBootstrap)
            );
            // And it still says WHERE to send and HOW much — a dismiss that
            // cost the facts would be a worse screen, not a kinder one.
            let detail = notice.detail.unwrap_or_default();
            assert!(detail.contains("0xTreasury"), "{detail}");
            assert!(detail.contains("0.02"), "{detail}");
        });
    }
}

/// The three edits a split screen can make to its rows.
///
/// The core offers ONE event for all of them — `RecipientsChanged`, carrying
/// the whole list — so each of these is "the list, with one thing different".
/// They are functions rather than closures inline in the page because the thing
/// that would go wrong is invisible: a rebuild that dropped a row's `id` or its
/// `name` would silently unname a payee and re-key a row the contact picker is
/// aiming at, and nothing downstream would complain.
#[must_use]
pub fn split_amount_edited(
    rows: &[SendRecipientDraft],
    index: usize,
    amount: String,
) -> Vec<SendRecipientDraft> {
    let mut next = rows.to_vec();
    if let Some(row) = next.get_mut(index) {
        if let Some(clean) = amount_edited(&amount, &row.amount) {
            row.amount = clean;
        }
    }
    next
}

/// A split row's address, typed (078 F-06): the whole list goes back to the
/// core with that one row's address replaced — `RecipientsChanged` is the
/// event the machine offers, and it validates the rest.
#[must_use]
pub fn split_address_edited(
    rows: &[SendRecipientDraft],
    index: usize,
    address: &str,
) -> Vec<SendRecipientDraft> {
    let mut next = rows.to_vec();
    if let Some(row) = next.get_mut(index) {
        row.address = address.trim().to_owned();
    }
    next
}

/// An amount field's edit as the core reads it (spec 073;
/// `vela_core::l10n::amount_text` says why): a decimal-comma keyboard's
/// "4,5" is 4.5 — raw, the send machine read 4 in fiat mode, and a custom
/// allowance's parser dropped the comma and allowed 45. `previous` is the
/// field's text before the edit; `None` is an edit with no reading as one
/// figure, and the field keeps what it had.
#[must_use]
pub fn amount_edited(next: &str, previous: &str) -> Option<String> {
    use vela_core::l10n::amount_text;
    amount_text::clean(
        next,
        crate::executor::format_prefs::current().number,
        amount_text::Entry::Unknown,
        Some(previous),
    )
}

#[must_use]
pub fn split_row_removed(rows: &[SendRecipientDraft], index: usize) -> Vec<SendRecipientDraft> {
    rows.iter()
        .enumerate()
        .filter(|(i, _)| *i != index)
        .map(|(_, row)| row.clone())
        .collect()
}

/// A blank row. The id is EMPTY on purpose: the core assigns its own
/// deterministic `rcpt_{n}` (its ported `makeRecipientId`), and a shell-minted
/// id would be a second naming scheme for the same thing.
#[must_use]
pub fn split_row_appended(rows: &[SendRecipientDraft]) -> Vec<SendRecipientDraft> {
    let mut next = rows.to_vec();
    next.push(SendRecipientDraft {
        id: String::new(),
        address: String::new(),
        amount: String::new(),
        name: None,
    });
    next
}

/// The figure "Use X for the empty rows" would copy (the web's `fillEmpty`):
/// offered while the core flags a row's amount as empty and another row has a
/// figure the core accepts. The first such row's, exactly as typed — nothing
/// is computed. `None` outside a split, or when there is nothing to fill.
#[must_use]
pub fn split_fill_source(send: &SendView) -> Option<&str> {
    use vela_core::app::send::SendRowFieldState;
    if !send.split_mode
        || !send
            .split_row_issues
            .iter()
            .any(|row| row.amount == SendRowFieldState::Empty)
    {
        return None;
    }
    send.recipients
        .iter()
        .find(|row| {
            !row.amount.trim().is_empty()
                && send
                    .split_row_issues
                    .iter()
                    .find(|issue| issue.id == row.id)
                    .is_none_or(|issue| issue.amount == SendRowFieldState::Ok)
        })
        .map(|row| row.amount.as_str())
}

/// One amount into every row that has none (the web's `fillEmptyAmounts`): a
/// bulk edit of the drafts, like adding a row. Rows with a figure keep it.
#[must_use]
pub fn split_empty_filled(rows: &[SendRecipientDraft], amount: &str) -> Vec<SendRecipientDraft> {
    rows.iter()
        .map(|row| {
            let mut row = row.clone();
            if row.amount.trim().is_empty() {
                amount.clone_into(&mut row.amount);
            }
            row
        })
        .collect()
}

/// A figure about to be SENT, every digit kept (the web's `exactAmount`):
/// only trailing fractional zeros go — `0.50` reads `0.5`, never rounded.
fn exact_amount(amount: &str) -> &str {
    if !amount.contains('.') {
        return amount;
    }
    amount.trim_end_matches('0').trim_end_matches('.')
}

#[cfg(test)]
mod split_tests {
    use super::*;

    fn row(id: &str, address: &str, amount: &str, name: Option<&str>) -> SendRecipientDraft {
        SendRecipientDraft {
            id: id.to_owned(),
            address: address.to_owned(),
            amount: amount.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    /// One row changes; every other row survives byte for byte.
    ///
    /// Ids and names are the part worth pinning. The contact picker aims at a
    /// row BY ID (`picker_target`), and the payroll importer is where the names
    /// come from — so a rebuild that regenerated ids would point the picker at
    /// a row that no longer exists, and one that dropped names would quietly
    /// turn "Alice" back into an address.
    /// Spec 073: a split row's share and every other amount field run the
    /// core's rule. The cases read the same under every number preset (the
    /// preset is the machine's here), so none is set.
    #[test]
    fn a_share_and_an_amount_run_the_core_amount_rule() {
        let rows = vec![row("rcpt_1", "0xAAA", "2", None)];
        // One typed comma into a figure with no point is the decimal mark.
        assert_eq!(
            split_amount_edited(&rows, 0, "2,".to_owned())[0].amount,
            "2."
        );
        // An edit with no reading as one figure changes nothing: 1.57 is not
        // what "1.5e-7" meant.
        assert_eq!(split_amount_edited(&rows, 0, "1.5e-7".to_owned()), rows);

        assert_eq!(amount_edited("4,", "4").as_deref(), Some("4."));
        assert_eq!(amount_edited("4.5", "4.").as_deref(), Some("4.5"));
        assert_eq!(amount_edited("0x10", ""), None);
    }

    #[test]
    fn editing_one_split_row_leaves_the_others_alone() {
        let rows = vec![
            row("rcpt_1", "0xAAA", "1", Some("Alice")),
            row("rcpt_2", "0xBBB", "2", None),
            row("rcpt_3", "0xCCC", "3", Some("Cara")),
        ];

        let edited = split_amount_edited(&rows, 1, "7.5".to_owned());
        assert_eq!(edited.len(), 3);
        assert_eq!(edited[1].amount, "7.5");
        assert_eq!(edited[1].id, "rcpt_2", "the row keeps its identity");
        assert_eq!(edited[0], rows[0]);
        assert_eq!(edited[2], rows[2]);

        // An index nobody has is not a reason to lose the list.
        assert_eq!(split_amount_edited(&rows, 9, "1".to_owned()), rows);

        // A row's address, typed (078 F-06): that row only, trimmed, its
        // amount and identity kept.
        let typed = split_address_edited(&rows, 2, "  0xDDD ");
        assert_eq!(typed[2].address, "0xDDD");
        assert_eq!(typed[2].amount, "3");
        assert_eq!(typed[2].id, "rcpt_3");
        assert_eq!(typed[0], rows[0]);
        assert_eq!(split_address_edited(&rows, 9, "0x1"), rows);

        let removed = split_row_removed(&rows, 1);
        assert_eq!(removed.len(), 2);
        assert_eq!(removed[0].id, "rcpt_1");
        assert_eq!(removed[1].id, "rcpt_3");
        assert_eq!(removed[1].name.as_deref(), Some("Cara"));

        let appended = split_row_appended(&rows);
        assert_eq!(appended.len(), 4);
        assert!(
            appended[3].id.is_empty(),
            "the core mints the id, not the shell"
        );
        assert!(appended[3].address.is_empty() && appended[3].amount.is_empty());
    }
}

/// The token ids in the order `send_pick` draws them — the page binds one
/// listener per row from this, so row N selects token N.
#[must_use]
pub fn send_token_ids(view: &SendView) -> Vec<String> {
    view.tokens.iter().map(SendToken::id).collect()
}

/// The recipient's trust line: a name the core resolved, else the
/// first-interaction tell (the one that matters for a poisoned look-alike).
fn recipient_note(send: &SendView, s: &FlowStrings) -> Option<SharedString> {
    if let Some(identity) = &send.recipient_identity
        && let Some(name) = &identity.name
    {
        return Some(SharedString::from(match &identity.source {
            Some(source) => format!("{name} · {source}"),
            None => name.clone(),
        }));
    }
    send.recipient_risk
        .as_ref()
        .is_some_and(|risk| risk.first_time == Some(true))
        .then(|| s.first_time_tag.clone())
}

/// The core's live amount validation, in the corpus's words. The symbol a
/// `None` carries is the chain's own coin — the core says the shell resolves
/// it, and it is the fee coin the person is short of.
fn amount_warning_text(
    warning: &SendAmountWarning,
    chain_id: u32,
    s: &FlowStrings,
) -> SharedString {
    let native = || {
        let symbol = native_symbol(chain_id);
        if symbol.is_empty() {
            "gas token".to_owned()
        } else {
            symbol
        }
    };
    match warning {
        SendAmountWarning::NotEnoughToken { symbol } => {
            fill(&s.warn_not_enough_token, "symbol", symbol).into()
        }
        SendAmountWarning::InsufficientForGas { symbol } => fill(
            &s.warn_insufficient_for_gas,
            "sym",
            &symbol.clone().unwrap_or_else(native),
        )
        .into(),
        SendAmountWarning::InsufficientGas { symbol } => fill(
            &s.warn_insufficient_gas,
            "sym",
            &symbol.clone().unwrap_or_else(native),
        )
        .into(),
        SendAmountWarning::NeedGas { symbol } => fill(
            &s.warn_need_gas,
            "sym",
            &symbol.clone().unwrap_or_else(native),
        )
        .into(),
        SendAmountWarning::CannotConvert { code, symbol } => fill(
            &fill(&s.warn_cannot_convert, "code", code),
            "symbol",
            symbol,
        )
        .into(),
    }
}

/// "Can't convert X to Y right now" — the one sentence three different
/// refusals share (the typed figure, the ⇄ control, the confirm gate).
fn cannot_convert(issue: &SendUnitIssue, s: &FlowStrings) -> SharedString {
    fill(
        &fill(&s.warn_cannot_convert, "code", &issue.code),
        "symbol",
        &issue.symbol,
    )
    .into()
}

/// What the core refused, in priority order — the hardest stop first.
///
/// Every branch here reads a field the core computed and this client used to
/// drop on the floor: an over-typed amount left the Continue button inert
/// with nothing on screen to explain it, which is a refusal the person cannot
/// act on. `action` is the way out the core itself offers.
/// The event the notice's way-out means. Derived by the SAME traversal that
/// wrote the sentence, so the button and the words can never disagree about
/// what they are offering.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum NoticeWayOut {
    /// The relay's float was topped up — probe it again.
    RetryAfterBootstrap,
    /// Add the network this locked request names.
    AddNetwork { chain_id: u32 },
    /// Back to the amount field.
    EditAmount,
}

/// The notice a screen shows, if any.
fn send_notice(i: &SendInputs<'_>, confirming: bool) -> Option<SendNotice> {
    build_notice(i, confirming).map(|(notice, _)| notice)
}

/// What its action does. Same function, same order — see [`NoticeWayOut`].
#[must_use]
pub fn notice_way_out(i: &SendInputs<'_>, confirming: bool) -> Option<NoticeWayOut> {
    build_notice(i, confirming).and_then(|(_, way_out)| way_out)
}

fn build_notice(
    i: &SendInputs<'_>,
    confirming: bool,
) -> Option<(SendNotice, Option<NoticeWayOut>)> {
    let (send, s) = (i.send, i.s);
    let chain_id = send
        .selected_token
        .as_ref()
        .map_or(1, |token| token.chain_id);

    // The relay cannot carry anything on this chain until its float is topped
    // up. A stop, and the only one that names an address to send to.
    if let Some(treasury) = &send.treasury_bootstrap {
        let native = treasury.asset == SendTreasuryAsset::Native;
        let decimals = if native { 18 } else { 6 };
        let symbol = if native {
            native_symbol(treasury.chain_id)
        } else {
            "pathUSD".to_owned()
        };
        let short = treasury
            .floor
            .parse::<u128>()
            .unwrap_or(0)
            .saturating_sub(treasury.balance.parse::<u128>().unwrap_or(0));
        #[allow(clippy::cast_precision_loss, reason = "a displayed top-up figure")]
        let short = short as f64 / 10f64.powi(decimals);
        let notice = SendNotice {
            // The way out of the stop itself. Without it the only exit from a
            // treasury that cannot pay is closing the whole journey — the core
            // has had `DismissTreasurySheet` since 026 and nothing sent it.
            dismiss: Some(s.funding_close.clone()),
            title: Some(s.funding_title.clone()),
            body: fill(&s.funding_lead, "symbol", &symbol).into(),
            detail: Some(
                format!(
                    "{} {}  ·  {} {} {symbol}",
                    s.funding_address_label,
                    treasury.address,
                    s.funding_amount_label,
                    trimmed(short),
                )
                .into(),
            ),
            action: Some(s.funding_check_now.clone()),
            error: true,
        };
        return Some((notice, Some(NoticeWayOut::RetryAfterBootstrap)));
    }

    // A locked payment request nobody can fulfil, with the add-network way out.
    if let Some(error) = &send.lock_error {
        let (title, body) = match error {
            SendLockError::Network { chain_id } => (
                s.lock_net_title.clone(),
                SharedString::from(fill(&s.lock_net_body, "chainId", &chain_id.to_string())),
            ),
            SendLockError::Token => (s.lock_token_title.clone(), s.lock_token_body.clone()),
        };
        // The line under the button is the LAST attempt's outcome; the button
        // itself is only offered for a network the wallet could add.
        let detail = send.add_network_msg.as_ref().map(|msg| match msg {
            SendAddNetworkMsg::NetNotFound => s.lock_net_not_found.clone(),
            SendAddNetworkMsg::NetNotCompatible { detail } => detail
                .clone()
                .map_or_else(|| s.lock_net_not_compatible.clone(), SharedString::from),
            SendAddNetworkMsg::NetAddError => s.lock_net_add_error.clone(),
        });
        let way_out = match error {
            SendLockError::Network { chain_id } => Some(NoticeWayOut::AddNetwork {
                chain_id: *chain_id,
            }),
            SendLockError::Token => None,
        };
        let notice = SendNotice {
            dismiss: None,
            title: Some(title),
            body,
            detail,
            action: way_out.is_some().then(|| s.lock_add_network.clone()),
            error: true,
        };
        return Some((notice, way_out));
    }

    // The transfer and its fee draw on the same coin, and together they do not
    // fit. The core computed the ceiling; "Edit amount" is its own event. It
    // comes first in a split too — the core measures it against the rows'
    // total, and it says the most that can be sent, which "exceeds your
    // balance" does not.
    //
    // Every figure is a base-unit decimal string: the shell formats it, and
    // exactly — a rounded "most you can send" could round up past it.
    if let Some(issue) = &send.same_asset_fee_issue {
        let decimals = send
            .selected_token
            .as_ref()
            .map_or(18, |token| token.decimals);
        let human = |base: &str| {
            base.parse::<u128>().map_or_else(
                |_| base.to_owned(),
                |units| vela_core::app::fee_policy::from_base_units(units, decimals),
            )
        };
        let body = fill(
            &fill(
                &fill(
                    &fill(
                        &fill(&s.same_fee_body, "amount", &human(&issue.transfer_amount)),
                        "fee",
                        &human(&issue.fee_amount),
                    ),
                    "total",
                    &human(&issue.total),
                ),
                "balance",
                &human(&issue.balance),
            ),
            "symbol",
            &issue.symbol,
        );
        let notice = SendNotice {
            dismiss: None,
            title: Some(fill(&s.same_fee_title, "symbol", &issue.symbol).into()),
            body: body.into(),
            detail: Some(
                fill(
                    &fill(
                        &s.same_fee_max,
                        "amount",
                        &human(&issue.max_transfer_amount),
                    ),
                    "symbol",
                    &issue.symbol,
                )
                .into(),
            ),
            action: Some(s.same_fee_edit.clone()),
            error: true,
        };
        return Some((notice, Some(NoticeWayOut::EditAmount)));
    }

    // The split rows add up to more than the balance.
    if send.split_over_balance {
        let notice = SendNotice {
            dismiss: None,
            title: Some(s.insufficient_title.clone()),
            body: s.insufficient_body.clone(),
            detail: None,
            action: None,
            error: true,
        };
        return Some((notice, None));
    }

    // A split says nothing more. The amount warning and ⇄'s refusal judge the
    // single form's figure, which a split leaves behind — the same order the
    // web, iOS and Android draw: ceiling, over-balance, then nothing.
    if send.split_mode {
        return None;
    }

    // On the confirm page the amount itself may have stopped resolving — a
    // display-currency commit landing under an open page re-denominates the
    // field, and the slide disarms with nothing said.
    if confirming && let Some(issue) = &send.confirm_amount_issue {
        let notice = SendNotice {
            dismiss: None,
            title: None,
            body: cannot_convert(issue, s),
            detail: None,
            action: Some(s.same_fee_edit.clone()),
            error: true,
        };
        return Some((notice, Some(NoticeWayOut::EditAmount)));
    }

    // The live amount warning — amber: the person is still typing. And when
    // there is none, the ⇄ control's own refusal, which the desktop draws no
    // control for and would otherwise never say.
    let body = send
        .amount_warning
        .as_ref()
        .map(|warning| amount_warning_text(warning, chain_id, s))
        .or_else(|| {
            // The ⇄ row's own sentence (the web's `denomReason`): the swap
            // has no rate, so the figure stays in the token — which is what
            // the person should type in.
            send.denom_toggle_reason.as_ref().map(|issue| {
                SharedString::from(fill(
                    &fill(&s.denom_toggle_no_rate, "code", &issue.code),
                    "symbol",
                    &issue.symbol,
                ))
            })
        })?;
    Some((
        SendNotice {
            dismiss: None,
            title: None,
            body,
            detail: None,
            action: None,
            error: false,
        },
        None,
    ))
}

/// The groups' members as split rows, in the order `contact_pick` draws
/// them — tapping a group ADDS everybody in it to the form, amounts blank,
/// each under the name the book knows them by (`name ?? resolved_name`, the
/// web's `seedGroup`). The core assigns the row ids.
#[must_use]
pub fn contact_group_members(view: &ContactsView) -> Vec<Vec<SendRecipientDraft>> {
    view.groups
        .iter()
        .map(|group| {
            group
                .members
                .iter()
                .map(|member| SendRecipientDraft {
                    id: String::new(),
                    address: member.address.clone(),
                    amount: String::new(),
                    name: member.name.clone().or_else(|| member.resolved_name.clone()),
                })
                .collect()
        })
        .collect()
}

/// What the core says about one split row, in the corpus's words.
///
/// A repeat names the row it repeats (issue 203) — the first occurrence is
/// not the mistake, so it carries nothing. A field is flagged only when there
/// is something IN it the core will not take: an empty one is unfinished, and
/// the gate's own hint already asks for it.
fn split_row_notes(send: &SendView, id: &str, s: &FlowStrings) -> Vec<SharedString> {
    use vela_core::app::send::SendRowFieldState;
    let mut notes = Vec::new();
    if let Some(repeat) = send.split_duplicates.iter().find(|row| row.id == id) {
        notes.push(
            fill(
                &s.recipient_duplicate,
                "n",
                &repeat.first_ordinal.to_string(),
            )
            .into(),
        );
    }
    if let Some(issue) = send.split_row_issues.iter().find(|row| row.id == id) {
        if issue.address == SendRowFieldState::Invalid {
            notes.push(s.batch_bad_address.clone());
        }
        if issue.amount == SendRowFieldState::Invalid {
            notes.push(s.bad_amount.clone());
        }
    }
    notes
}

/// DSD2L / DSD2bL — recipient and amount.
#[must_use]
pub fn send_form(i: &SendInputs<'_>) -> SendForm {
    let (send, s) = (i.send, i.s);
    let token = send.selected_token.as_ref();
    let symbol = token.map(|t| t.symbol.clone()).unwrap_or_default();
    let usd = token
        .and_then(|t| t.price_usd)
        .map(|price| send.token_amount.parse::<f64>().unwrap_or(0.0) * price);
    let split = send.split_mode;

    let header = match token {
        Some(token) => (
            TokenMark {
                ticker: token.symbol.clone().into(),
                badge: tint(token.chain_id),
                logos: crate::marks::token_logos(
                    token.chain_id,
                    &token.symbol,
                    token.token_address.as_deref(),
                    &token.logo_urls,
                ),
            },
            SharedString::from(token.symbol.clone()),
            SharedString::from(format!(
                "{} · {}",
                chain_name(token.chain_id),
                fill(
                    &s.balance_label,
                    "amount",
                    &trimmed(token.balance.parse::<f64>().unwrap_or(0.0))
                )
            )),
            (!split).then(|| s.max.clone()),
        ),
        None => (
            TokenMark {
                ticker: "—".into(),
                badge: tint(1),
                logos: crate::marks::Logos::default(),
            },
            SharedString::from("—"),
            SharedString::default(),
            None,
        ),
    };

    let sweeping = send.multi_select_mode;
    let recipient = (!split).then(|| {
        let lines = if send.recipient.is_empty() {
            (String::new(), String::new())
        } else {
            address_lines(&send.recipient)
        };
        (
            // The field is "Recipient", always (078 F-08): the trust line is
            // a note UNDER it, not the label's replacement — a name the core
            // resolved used to become the field's title.
            s.recipient_label.clone(),
            (lines.0.into(), lines.1.into()),
            SharedString::from(send.recipient.clone()),
        )
    });
    let recipient_note = if sweeping {
        Some(s.multi_send_same_recipient.clone())
    } else if split {
        None
    } else {
        recipient_note(send, s)
    };
    // SD2d (078 F-05): the picked coins, each at the amount it will move —
    // the same reading the confirm lists (`sweep_breakdown`).
    let sweep = sweeping.then(|| {
        let picked: Vec<_> = send
            .tokens
            .iter()
            .filter(|token| send.multi_selected_ids.contains(&token.id()))
            .collect();
        let chain_id = send
            .multi_chain_id
            .or_else(|| picked.first().map(|token| token.chain_id))
            .unwrap_or(1);
        SweepForm {
            summary: fill(
                &fill(&s.multi_send_summary, "n", &picked.len().to_string()),
                "chain",
                &chain_name(chain_id),
            )
            .into(),
            rows: picked
                .iter()
                .map(|token| {
                    let amount = send
                        .multi_specs
                        .iter()
                        .find(|spec| spec.token_address == token.token_address)
                        .map_or(token.balance.as_str(), |spec| spec.amount.as_str());
                    SweepRow {
                        mark: TokenMark {
                            ticker: token.symbol.clone().into(),
                            badge: tint(token.chain_id),
                            logos: crate::marks::token_logos(
                                token.chain_id,
                                &token.symbol,
                                token.token_address.as_deref(),
                                &token.logo_urls,
                            ),
                        },
                        symbol: token.symbol.clone().into(),
                        balance: fill(&s.balance_label, "amount", &trimmed_str(&token.balance))
                            .into(),
                        amount: trimmed_str(amount).into(),
                    }
                })
                .collect(),
        }
    });

    // Which unit the figure is TYPED in: the figure's own code, never the
    // display currency (#231). The line under it is the OTHER denomination —
    // the token while money is typed, the money while the token is (#197).
    let fiat_code = send.amount_fiat_code.as_ref();
    let other_line = match (fiat_code, token) {
        (Some(_), Some(token)) => SharedString::from(format!(
            "≈ {} {}",
            trimmed_str(if send.token_amount.is_empty() {
                "0"
            } else {
                &send.token_amount
            }),
            token.symbol
        )),
        (Some(_), None) => SharedString::default(),
        (None, _) => fiat_line(usd, i.locale, i.money).unwrap_or_default(),
    };

    SendForm {
        token: header,
        sweep,
        recipient_note,
        amount: (!split && !sweeping).then(|| {
            (
                SharedString::from(if send.amount.is_empty() {
                    "0".to_owned()
                } else {
                    send.amount.clone()
                }),
                other_line,
            )
        }),
        amount_unit: (!split && !sweeping)
            .then(|| SharedString::from(fiat_code.cloned().unwrap_or_else(|| symbol.clone()))),
        // ⇄ exists only where the core offers it, and is live only where
        // pressing it would change something; its refusal is the notice's.
        denom_toggle: (!split && !sweeping && send.denom_toggle_shown)
            .then_some(send.denom_toggle_enabled),
        recipient,
        // A sweep is one person by definition: no door into a split.
        add_recipient: (!split && !sweeping).then(|| s.add_recipient.clone()),
        recipients: if split {
            send.recipients
                .iter()
                .enumerate()
                .map(|(index, draft)| RecipientCard {
                    ordinal: fill(&s.recipient_n, "n", &(index + 1).to_string()).into(),
                    name: draft
                        .name
                        .clone()
                        .unwrap_or_else(|| shorten(&draft.address))
                        .into(),
                    seed: draft.address.clone().into(),
                    amount: format!("{} {symbol}", draft.amount)
                        .trim()
                        .to_owned()
                        .into(),
                    notes: split_row_notes(send, &draft.id, s),
                })
                .collect()
        } else {
            Vec::new()
        },
        recipient_actions: if split {
            vec![
                s.add_recipient.clone(),
                s.from_contacts.clone(),
                s.batch_import.clone(),
            ]
        } else {
            Vec::new()
        },
        summary: split.then(|| {
            (
                format!(
                    "{} · {}",
                    s.split_total,
                    s.recipients(send.recipients.len())
                )
                .into(),
                // The core's SUM of the rows (`confirm_amount`); `token_amount`
                // is the single field, empty in a split (spec 038 #D4). Empty
                // when a row cannot be summed yet: a dash, not a bare symbol.
                if send.confirm_amount.is_empty() {
                    SharedString::from("—")
                } else {
                    format!("{} {symbol}", send.confirm_amount)
                        .trim()
                        .to_owned()
                        .into()
                },
            )
        }),
        remaining: send.split_remaining.as_ref().filter(|_| split).map(|left| {
            fill(
                &s.split_remaining,
                "amount",
                format!("{} {symbol}", trimmed_str(left)).trim(),
            )
            .into()
        }),
        summary_over: split && send.split_over_balance,
        fill_empty: split_fill_source(send).map(|amount| {
            fill(
                &s.split_fill_empty,
                "amount",
                format!("{} {symbol}", exact_amount(amount)).trim(),
            )
            .into()
        }),
        pick_contacts: (!split).then(|| s.from_contacts.clone()),
        notice: send_notice(i, false),
        fee: send_fee_row(i),
        speed: send_speed(i),
        // The 15s pre-check is a WAIT, not a refusal: the button says so
        // rather than going dead (busy ≠ disabled).
        cta: if send.estimating_gas {
            s.estimating.clone()
        } else {
            s.continue_btn.clone()
        },
        cta_state: if send.estimating_gas {
            CtaState::Busy
        } else if send.can_continue {
            CtaState::Enabled
        } else {
            CtaState::Disabled
        },
    }
}

/// The two error wordings the core chooses between.
fn tx_error_text(send: &SendView, s: &FlowStrings) -> Option<SharedString> {
    send.tx_error.map(|key| match key {
        vela_core::app::send::SendTxErrorKey::Generic => s.tx_error_generic.clone(),
        vela_core::app::send::SendTxErrorKey::BundlerFund => s.tx_error_bundler_fund.clone(),
    })
}

/// DSD3L — what is about to be signed.
#[must_use]
pub fn send_confirm(i: &SendInputs<'_>) -> SendConfirm {
    let (send, s) = (i.send, i.s);
    let token = send.selected_token.as_ref();
    let symbol = token.map(|t| t.symbol.clone()).unwrap_or_default();
    let chain_id = token.map_or(1, |t| t.chain_id);
    let usd = token
        .and_then(|t| t.price_usd)
        .map(|price| send.confirm_amount.parse::<f64>().unwrap_or(0.0) * price);
    let to_name = send
        .recipient_identity
        .as_ref()
        .and_then(|identity| identity.name.clone());
    let mut facts = vec![
        FactRow {
            label: s.from_label.clone(),
            value: i.identity_name.to_owned().into(),
            lead: FactLead::Identicon(i.identity_address.to_owned().into()),
            mono: false,
            copy: None,
            note: None,
        },
        FactRow {
            label: s.to_label.clone(),
            value: to_name
                .clone()
                .unwrap_or_else(|| shorten(&send.recipient))
                .into(),
            lead: FactLead::Identicon(send.recipient.clone().into()),
            mono: to_name.is_none(),
            copy: None,
            note: None,
        },
        FactRow {
            label: s.detail_chain.clone(),
            value: chain_name(chain_id).into(),
            lead: FactLead::Token(TokenMark {
                ticker: native_symbol(chain_id).into(),
                badge: tint(chain_id),
                logos: crate::marks::chain_logos(chain_id),
            }),
            mono: false,
            copy: None,
            note: None,
        },
        FactRow {
            label: s.est_fee.clone(),
            value: if send.fee_busy || i.fee.busy {
                s.fee_pending.clone()
            } else {
                fee_line(
                    send.fee.as_ref().or(i.fee.fee.as_ref()),
                    Some(send),
                    i.fee,
                    i.locale,
                    i.money,
                )
                .into()
            },
            lead: FactLead::None,
            mono: false,
            copy: None,
            note: None,
        },
    ];
    // The speed, but only when it was CHOSEN for this send, or taken because
    // it was free (spec 068 / issue 686). The confirm is the last screen
    // before a signature: a payment bumped off the usual pace says so here,
    // where a mis-tap is still cheap to undo — and a free upgrade says why.
    // A send at the stored default adds no row.
    if let Some(speed) = i.speed.map(|speed| &speed.view)
        && (speed.picked || speed.free)
    {
        facts.push(FactRow {
            label: s.fee_speed_label.clone(),
            value: tier_name(s, speed.tier),
            lead: FactLead::None,
            mono: false,
            copy: None,
            note: (!speed.picked).then(|| s.fee_speed_free.clone()),
        });
    }
    // The last attempt's error is a NOTICE now, not a subline: several of
    // these have a way out (edit the amount, fund the relay) and a subline
    // cannot carry one.
    let sweep = send
        .multi_select_mode
        .then(|| sweep_breakdown(send, i.locale, i.money));
    let notice = send_notice(i, true).or_else(|| {
        tx_error_text(send, s).map(|body| SendNotice {
            dismiss: None,
            title: None,
            body,
            detail: None,
            action: None,
            error: true,
        })
    });
    SendConfirm {
        // A sweep moves several coins; one mark would name the wrong one.
        mark: if send.multi_select_mode {
            None
        } else {
            token.map(|token| TokenMark {
                ticker: token.symbol.clone().into(),
                badge: tint(token.chain_id),
                logos: crate::marks::token_logos(
                    token.chain_id,
                    &token.symbol,
                    token.token_address.as_deref(),
                    &[],
                ),
            })
        },
        amount: match &sweep {
            // A sweep has no single headline figure: "3 assets".
            Some((rows, _)) => fill(&s.assets_count, "n", &rows.len().to_string()).into(),
            None => format!("{} {symbol}", send.confirm_amount)
                .trim()
                .to_owned()
                .into(),
        },
        subline: match &sweep {
            Some((_, total_usd)) => fill(
                &fill(
                    &s.confirm_total_line,
                    "fiat",
                    &money(*total_usd, i.locale, i.money),
                ),
                "network",
                &chain_name(send.multi_chain_id.unwrap_or(chain_id)),
            )
            .into(),
            None => fiat_line(usd, i.locale, i.money).unwrap_or_default(),
        },
        facts,
        // Spec 038 #D2: a split's confirm lists every recipient by name and
        // amount — what is about to be signed, in full. A sweep lists every
        // coin it moves, at the amount the signature will move.
        breakdown: if let Some((rows, _)) = sweep {
            rows
        } else if send.split_mode {
            send.recipients
                .iter()
                .map(|draft| BreakdownRow {
                    seed: (!draft.address.is_empty())
                        .then(|| SharedString::from(draft.address.clone())),
                    label: draft
                        .name
                        .clone()
                        .unwrap_or_else(|| shorten(&draft.address))
                        .into(),
                    value: format!("{} {symbol}", draft.amount)
                        .trim()
                        .to_owned()
                        .into(),
                })
                .collect()
        } else {
            Vec::new()
        },
        // The core's own verdict, resolved on this page only (single recipient).
        recipient_tag: (!send.split_mode
            && send
                .recipient_risk
                .as_ref()
                .is_some_and(|risk| risk.first_time == Some(true)))
        .then(|| s.first_time_tag.clone()),
        notice,
        cta: s.confirm_send.clone(),
        // Signing and submitting are waits; `can_confirm` is the core's gate
        // (fee settled ∧ nothing re-quoting ∧ no ceiling breach ∧ idle).
        cta_state: if send.sending
            || matches!(
                send.tx_status,
                SendTxStatus::Preparing | SendTxStatus::Signing | SendTxStatus::Submitting
            ) {
            CtaState::Busy
        } else if send.can_confirm {
            CtaState::Enabled
        } else {
            CtaState::Disabled
        },
    }
}

/// One amount of money, in the hero's own formatting (no `≈`).
fn money(usd: f64, locale: &str, currency: &crate::wallet::live::Money) -> String {
    currency.text(usd, locale)
}

/// SD3c — the sweep's rows and their summed value: every picked coin at the
/// amount the signature will move — the core's reserved spec (`multi_specs`,
/// net of the gas the fee coin pays), else the full balance the spec will
/// become (the web's `sweepAmount`). Never a figure summed here on its own.
fn sweep_breakdown(
    send: &SendView,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> (Vec<BreakdownRow>, f64) {
    let mut total_usd = 0.0;
    let rows = send
        .tokens
        .iter()
        .filter(|token| send.multi_selected_ids.contains(&token.id()))
        .map(|token| {
            let amount = send
                .multi_specs
                .iter()
                .find(|spec| spec.token_address == token.token_address)
                .map_or(token.balance.as_str(), |spec| spec.amount.as_str());
            let value = format!("{} {}", trimmed_str(amount), token.symbol);
            let row_usd = token
                .price_usd
                .map(|price| amount.parse::<f64>().unwrap_or(0.0) * price);
            if let Some(row_usd) = row_usd {
                total_usd += row_usd;
            }
            BreakdownRow {
                seed: None,
                label: token.symbol.clone().into(),
                value: match row_usd {
                    Some(row_usd) => format!("{value} · ≈{}", money(row_usd, locale, currency)),
                    None => value,
                }
                .into(),
            }
        })
        .collect();
    (rows, total_usd)
}

/// DSD4L — the receipt.
///
/// The stage comes from `receipt.status`, not from `tx_status`: the core flips
/// `tx_status` to `confirmed` the moment the signature is a fact, while the
/// receipt's own status is what tracks the chain. Reading the wrong one would
/// say the money had arrived while it was still in the air.
#[must_use]
pub fn send_receipt(i: &SendInputs<'_>) -> SendReceipt {
    let (send, s) = (i.send, i.s);
    let token = send.selected_token.as_ref();
    let symbol = token.map(|t| t.symbol.clone()).unwrap_or_default();
    let chain_id = token.map_or(1, |t| t.chain_id);
    let to = send
        .recipient_identity
        .as_ref()
        .and_then(|identity| identity.name.clone())
        .unwrap_or_else(|| shorten(&send.recipient));
    let status = send.receipt.as_ref().map(|receipt| receipt.status);
    let (breakdown_title, breakdown) = receipt_parts(send, s, &symbol);

    // Why it is held, when the core knows: `hold_reason` is the difference
    // between a payment that is queued and one that is over, and between
    // "try again" and "send again at the current fee". Both sentences have
    // been in the corpus since the send flow was written; no shell said
    // either — the web does not read this field yet.
    //
    // Each branch takes only ITS reason. The two model flags are not mutually
    // exclusive — a receipt that was held and then failed still carries
    // `fee_held` — and "will be sent automatically once fees settle" printed
    // under a failure is worse than saying nothing.
    let hold = send
        .receipt
        .as_ref()
        .and_then(|receipt| receipt.hold_reason);
    let held = (hold == Some(SendHoldReason::FeeHold)).then(|| s.tx_held_fees.clone());
    let rejected = (hold == Some(SendHoldReason::FeeRejected)).then(|| s.tx_rejected_fees.clone());

    if status == Some(SendReceiptStatus::Failed) || send.tx_status == SendTxStatus::Error {
        return SendReceipt {
            stage: ReceiptStage::Failed,
            progress: None,
            explorer: None,
            cta_accent: false,
            breakdown_title: None,
            breakdown: Vec::new(),
            title: tx_error_text(send, s).unwrap_or_else(|| s.tx_error_generic.clone()),
            captions: rejected.into_iter().collect(),
            hash: None,
            cta: s.done.clone(),
        };
    }
    if status == Some(SendReceiptStatus::Confirmed) {
        let amount = send
            .receipt
            .as_ref()
            .map(|receipt| receipt.amount.clone())
            .filter(|amount| !amount.is_empty())
            .unwrap_or_else(|| send.confirm_amount.clone());
        return SendReceipt {
            stage: ReceiptStage::Confirmed,
            progress: Some(1.0),
            // The chain is the TOKEN's, never whichever one the wallet is
            // looking at now: a send can confirm after the person moved on.
            explorer: send
                .tx_hash
                .as_ref()
                .filter(|hash| !hash.is_empty())
                .map(|hash| {
                    (
                        s.view_on_explorer.clone(),
                        SharedString::from(format!("{}/tx/{hash}", explorer_root(chain_id))),
                    )
                }),
            cta_accent: true,
            breakdown_title: breakdown_title.clone(),
            breakdown: breakdown.clone(),
            title: fill(
                &fill(&s.tx_confirmed_title, "amount", &amount),
                "symbol",
                &symbol,
            )
            .into(),
            // A split names its count here and its people below; "To " with
            // nobody after it was what the single-recipient line read as.
            captions: vec![
                format!(
                    "{} · {}",
                    breakdown_title
                        .as_ref()
                        .map_or_else(|| fill(&s.to_name, "name", &to), ToString::to_string),
                    chain_name(chain_id)
                )
                .into(),
            ],
            hash: send
                .tx_hash
                .clone()
                .map(|hash| (s.tx_hash.clone(), hash.into())),
            cta: s.done.clone(),
        };
    }
    if status == Some(SendReceiptStatus::Submitted) {
        let eta = send.receipt.as_ref().and_then(|receipt| {
            let (at, typical) = (receipt.submitted_at_ms?, receipt.typical_inclusion_s?);
            let elapsed = ((crate::executor::now_ms() - at) / 1000.0).max(0.0) as u64;
            Some((elapsed, u64::from(typical)))
        });
        return SendReceipt {
            stage: ReceiptStage::Submitted,
            progress: eta.and_then(|(elapsed, typical)| ring_progress(elapsed, typical)),
            explorer: None,
            cta_accent: false,
            breakdown_title: breakdown_title.clone(),
            breakdown: breakdown.clone(),
            title: s.tx_submitted_title.clone(),
            // A held payment is NOT waiting for a confirmation: it is queued
            // until fees settle, and it says so in place of the ordinary wait
            // rather than beside it.
            captions: {
                // Spec 038 #D3: count, don't spin. The core hands over when the
                // relay accepted the op and this chain's usual time; the shell
                // owns the clock and the sentences are the corpus's.
                let mut lines = vec![held.unwrap_or_else(|| s.tx_waiting_confirm.clone())];
                if let Some(receipt) = send.receipt.as_ref()
                    && let (Some(at), Some(typical)) =
                        (receipt.submitted_at_ms, receipt.typical_inclusion_s)
                {
                    let elapsed = ((crate::executor::now_ms() - at) / 1000.0).max(0.0) as u64;
                    lines.push(
                        fill(
                            &fill(&s.tx_typical_time, "chainName", &chain_name(chain_id)),
                            "estSecs",
                            &typical.to_string(),
                        )
                        .into(),
                    );
                    // Inside the usual time the line counts DOWN — "~9s
                    // remaining" is a promise with an end; "almost there"
                    // waits until the usual time has passed, which is when it
                    // is true (the web's `etaLines`).
                    let typical = u64::from(typical);
                    lines.push(if elapsed < typical {
                        fill(
                            &s.tx_remaining,
                            "remaining",
                            &(typical - elapsed).to_string(),
                        )
                        .into()
                    } else if elapsed < typical * 2 {
                        fill(&s.tx_elapsed, "elapsed", &elapsed.to_string()).into()
                    } else {
                        s.tx_slow_confirm.clone()
                    });
                }
                lines
            },
            hash: send
                .tx_hash
                .clone()
                .or_else(|| send.user_op_hash.clone())
                .map(|hash| (s.tx_hash.clone(), hash.into())),
            cta: s.tx_close_background.clone(),
        };
    }
    // Signing or submitting: nothing has been accepted yet.
    SendReceipt {
        stage: ReceiptStage::Submitting,
        progress: None,
        explorer: None,
        cta_accent: false,
        breakdown_title,
        breakdown,
        title: s.tx_submitting.clone(),
        captions: vec![s.tx_preparing.clone(), s.tx_background_hint.clone()],
        hash: None,
        cta: s.tx_close_background.clone(),
    }
}

/// The ring round the receipt's disc while a transaction is on its way — the
/// web's `ringProgress`, one curve for both receipts: it eases toward full and
/// never gets there (about 70% at the chain's typical time, 86% at twice it, a
/// 92% ceiling), so only the confirmation closes the ring. `None` without a
/// typical time, and the ring roams instead of filling.
#[must_use]
pub fn ring_progress(elapsed_s: u64, typical_s: u64) -> Option<f32> {
    if typical_s == 0 {
        return None;
    }
    let elapsed = elapsed_s as f32;
    Some(0.92 * (1. - (-1.4 * elapsed / typical_s.max(1) as f32).exp()))
}

/// Spec 038 #D2: a split's parts on the receipt as on the confirm — from the
/// receipt's own transfers once the core froze them, from the drafts before
/// that. Nothing for a single send or a sweep (a sweep's parts are assets,
/// and its one recipient is already the caption).
fn receipt_parts(
    send: &SendView,
    s: &FlowStrings,
    symbol: &str,
) -> (Option<SharedString>, Vec<BreakdownRow>) {
    let frozen: Vec<BreakdownRow> = match send.receipt.as_ref() {
        Some(receipt) if matches!(receipt.kind, Some(SendReceiptKind::Split)) => receipt
            .transfers
            .iter()
            .map(|transfer| BreakdownRow {
                seed: Some(SharedString::from(transfer.to.clone())),
                label: transfer
                    .to_name
                    .clone()
                    .unwrap_or_else(|| shorten(&transfer.to))
                    .into(),
                value: format!("{} {}", transfer.amount, transfer.symbol)
                    .trim()
                    .to_owned()
                    .into(),
            })
            .collect(),
        _ => Vec::new(),
    };
    let rows = if !frozen.is_empty() {
        frozen
    } else if send.split_mode {
        send.recipients
            .iter()
            .map(|draft| BreakdownRow {
                seed: (!draft.address.is_empty())
                    .then(|| SharedString::from(draft.address.clone())),
                label: draft
                    .name
                    .clone()
                    .unwrap_or_else(|| shorten(&draft.address))
                    .into(),
                value: format!("{} {symbol}", draft.amount)
                    .trim()
                    .to_owned()
                    .into(),
            })
            .collect()
    } else {
        Vec::new()
    };
    if rows.is_empty() {
        return (None, Vec::new());
    }
    let title = s.recipients(rows.len());
    (Some(title.into()), rows)
}

/// Spec 038 #D2: a folded batch row opens to what it folded — the split's
/// recipients by name and avatar, the sweep's assets — under the facts,
/// where the single send's "To" would have been.
fn detail_parts(
    item: &vela_core::app::activity_feed::FeedItem,
    s: &FlowStrings,
) -> (Option<SharedString>, Vec<BreakdownRow>) {
    let Some(batch) = item.batch.as_ref() else {
        return (None, Vec::new());
    };
    let split = batch.kind == FeedBatchKind::Split;
    let rows: Vec<BreakdownRow> = batch
        .transfers
        .iter()
        .map(|transfer| BreakdownRow {
            seed: split.then(|| SharedString::from(transfer.to.clone())),
            label: if split {
                transfer
                    .to_name
                    .clone()
                    .unwrap_or_else(|| shorten(&transfer.to))
                    .into()
            } else {
                transfer.symbol.clone().into()
            },
            value: format!("{} {}", trimmed_str(&transfer.value), transfer.symbol)
                .trim()
                .to_owned()
                .into(),
        })
        .collect();
    if rows.is_empty() {
        return (None, Vec::new());
    }
    let title = split.then(|| s.recipients(rows.len()).into());
    (title, rows)
}

/// A decimal string as the shell prints token amounts.
fn trimmed_str(value: &str) -> String {
    value
        .parse::<f64>()
        .map_or_else(|_| value.to_owned(), trimmed)
}

/// DSD2fL — the fee coin sheet. Every row the relay published, including the
/// ones that cannot pay: which is spendable is `insufficient`, the core's
/// verdict, and hiding a row here would be a second filter beside it.
#[must_use]
pub fn fee_token(i: &SendInputs<'_>) -> FeeTokenPick {
    let chain_id = i.send.selected_token.as_ref().map_or(1, |t| t.chain_id);
    FeeTokenPick {
        hint: i.s.fee_token_hint.clone(),
        estimate_label: i.s.fee_token_estimate.clone(),
        rows: i
            .fee
            .options
            .iter()
            .map(|option| {
                let scale = 10f64.powi(option.decimals as i32);
                let balance = option.balance.parse::<f64>().unwrap_or(0.0) / scale;
                FeeTokenRow {
                    mark: TokenMark {
                        ticker: option.symbol.clone().into(),
                        badge: tint(chain_id),
                        logos: crate::marks::token_logos(
                            chain_id,
                            &option.symbol,
                            option.contract.as_deref(),
                            &[],
                        ),
                    },
                    symbol: option.symbol.clone().into(),
                    balance: fill(&i.s.balance_label, "amount", &trimmed(balance)).into(),
                    fee: match &option.amount {
                        None => "—".into(),
                        Some(amount) => format!(
                            "~{} {}",
                            trimmed(amount.parse::<f64>().unwrap_or(0.0) / scale),
                            option.symbol
                        )
                        .into(),
                    },
                    selected: option.selected,
                    insufficient: option.insufficient,
                }
            })
            .collect(),
    }
}

/// The fee coins in the order `fee_token` draws them (`None` = native).
#[must_use]
pub fn fee_token_contracts(fee: &FeeView) -> Vec<Option<String>> {
    fee.options
        .iter()
        .map(|option| option.contract.clone())
        .collect()
}

/// DSD2eL — the address book, as a picker. The rows are the core's book in
/// its order; the group a person is filed under is the first that lists them.
#[must_use]
pub fn contact_pick(view: &ContactsView, s: &FlowStrings) -> ContactPick {
    let group_of = |address: &str| {
        view.groups
            .iter()
            .find(|group| {
                group
                    .members
                    .iter()
                    .any(|member| member.address.eq_ignore_ascii_case(address))
            })
            .map(|group| SharedString::from(group.name.clone()))
    };
    ContactPick {
        search_placeholder: s.pick_contact_search.clone(),
        scan_row: s.scan_to_fill.clone(),
        groups_title: s.contacts_groups.clone(),
        groups: view
            .groups
            .iter()
            .map(|group| {
                (
                    SharedString::from(group.name.clone()),
                    fill(&s.group_members, "count", &group.members.len().to_string()).into(),
                    tint(100),
                    tint(1),
                )
            })
            .collect(),
        contacts_title: s.contacts_title.clone(),
        contacts: view
            .contacts
            .iter()
            .map(|contact| crate::flows::fixtures::ContactEntry {
                name: contact
                    .name
                    .clone()
                    .or_else(|| contact.resolved_name.clone())
                    .unwrap_or_else(|| shorten(&contact.address))
                    .into(),
                group: group_of(&contact.address),
                address: shorten(&contact.address).into(),
                seed: contact.address.clone().into(),
            })
            .collect(),
    }
}

/// The addresses in the order `contact_pick` draws them.
#[must_use]
pub fn contact_addresses(view: &ContactsView) -> Vec<String> {
    view.contacts
        .iter()
        .map(|contact| contact.address.clone())
        .collect()
}

/// The importer's list as the sheet had it: the rows that parsed and the
/// lines the parser refused, in source order (both carry the same line
/// numbering), so a refused line sits between its neighbours in the sheet —
/// the web's `previewRows`. Every row that will not be paid says why.
fn batch_rows(view: &BatchView, symbol: &str, s: &FlowStrings) -> Vec<BatchRow> {
    use vela_core::app::batch_import::BatchParseReason;
    let fiat = view.unit == BatchUnit::Fiat;
    let mut rows: Vec<(u32, BatchRow)> = view
        .preview
        .iter()
        .map(|row| {
            // The core converted it; an unconvertible row carries no token
            // amount, and showing the raw fiat there would read as if it had.
            let sendable = !row.token_amount.is_empty() && row.token_amount != "0";
            let amount = if sendable {
                format!("{} {symbol}", row.token_amount)
            } else {
                "—".to_owned()
            };
            (
                row.line,
                BatchRow {
                    ok: row.ok,
                    address: row
                        .name
                        .clone()
                        .unwrap_or_else(|| row.address.clone())
                        .into(),
                    // A fiat sheet's figure exactly as the sheet wrote it, so
                    // it can be read back against the sheet — beside what it
                    // became (the mock's "5,000 CNY → 689.66").
                    conversion: if fiat {
                        format!("{} {} → {amount}", row.raw_amount, view.fiat_code).into()
                    } else {
                        amount.into()
                    },
                    note: if row.dup {
                        Some(s.batch_dup.clone())
                    } else if row.valid {
                        None
                    } else {
                        Some(s.batch_bad_address.clone())
                    },
                },
            )
        })
        .collect();
    rows.extend(view.errors.iter().map(|error| {
        (
            error.line,
            BatchRow {
                ok: false,
                address: error.raw.clone().into(),
                conversion: SharedString::default(),
                note: Some(match error.reason {
                    BatchParseReason::NoAddress => s.batch_bad_address.clone(),
                    BatchParseReason::NoAmount => s.bad_amount.clone(),
                }),
            },
        )
    }));
    // Stable: a parsed row and a refused one never share a line number.
    rows.sort_by_key(|(line, _)| *line);
    rows.into_iter().map(|(_, row)| row).collect()
}

/// DSD2cL's total line (the web's `total`): what the import sends, in the
/// token and — for a fiat sheet — in the sheet's currency, against what the
/// account holds. `remaining` is the form's `split_remaining` when this
/// import ADDS to rows already there: then that, not the whole balance, is
/// what it draws from. `None` until a row parses.
#[must_use]
pub fn batch_total(
    view: &BatchView,
    symbol: &str,
    balance: &str,
    remaining: Option<&str>,
    s: &FlowStrings,
) -> Option<crate::flows::fixtures::BatchTotal> {
    let count = view.recipient_count;
    (count > 0).then(|| crate::flows::fixtures::BatchTotal {
        label: format!("{} · {}", s.split_total, s.recipients(count as usize)).into(),
        value: format!("{} {symbol}", view.total_token).into(),
        detail: view
            .total_fiat
            .as_ref()
            .map(|fiat| format!("{fiat} {}", view.fiat_code).into()),
        balance: match remaining {
            Some(left) => fill(
                &s.split_remaining,
                "amount",
                &format!("{} {symbol}", trimmed_str(left)),
            ),
            None => fill(
                &s.balance_label,
                "amount",
                &format!("{} {symbol}", trimmed_str(balance)),
            ),
        }
        .into(),
        over: view
            .over_balance
            .then(|| fill(&s.batch_over_balance, "sym", symbol).into()),
    })
}

/// What an import's total line is read against when it ADDS to rows already
/// on the split form: the core's `split_remaining`. `None` — the whole
/// balance — when the form is empty or the person chose to replace its rows
/// (the web's `formHasRows && !replaces ? remaining : undefined`).
#[must_use]
pub fn batch_remaining(send: &SendView, replaces: bool) -> Option<&str> {
    let form_has_rows =
        (send.split_import_room as usize) < vela_core::app::send::BATCH_MAX_RECIPIENTS;
    send.split_remaining
        .as_deref()
        .filter(|_| form_has_rows && !replaces)
}

/// DSD2cL — the batch importer. The parse, the duplicate check, the
/// fiat→token conversion, the cap and the apply gate are the core's; this
/// only words them. The rule that matters: when no source can price the
/// chosen currency the rate is UNKNOWN and the core refuses to convert — the
/// sheet shows that refusal instead of a number.
#[must_use]
pub fn batch_import(view: &BatchView, symbol: &str, s: &FlowStrings) -> BatchImport {
    let code = view.fiat_code.as_str();
    let rate_label = fill(&s.batch_rate_label, "sym", symbol);
    let rate_value = match (view.rate_status, view.rate_input.is_empty()) {
        (_, false) => format!("{rate_label} {} {code}", view.rate_input),
        (BatchRateStatus::Loading, true) => s.batch_rate_loading.to_string(),
        (BatchRateStatus::Ok, true) => rate_label.clone(),
        (BatchRateStatus::Failed, true) => s.batch_rate_failed.to_string(),
    };
    let rate_hint = if view.rate_status == BatchRateStatus::Failed && view.rate_input.is_empty() {
        fill(&fill(&s.batch_rate_hint, "code", code), "sym", symbol)
    } else if !view.priced && view.unit == BatchUnit::Fiat {
        s.batch_no_price.to_string()
    } else {
        String::new()
    };
    let paste = if let Some(name) = &view.file_name {
        name.clone()
    } else if view.raw_text.is_empty() {
        s.batch_paste_placeholder.to_string()
    } else {
        view.raw_text.clone()
    };
    let rejected = match view.rejected {
        0 => String::new(),
        1 => fill(&s.batch_rejected_one, "count", "1"),
        n => fill(&s.batch_rejected_other, "count", &n.to_string()),
    };
    BatchImport {
        unit_fiat: fill(&s.batch_unit_fiat, "code", code).into(),
        unit_token: fill(&s.batch_unit_token, "sym", symbol).into(),
        fiat_on: view.unit == BatchUnit::Fiat,
        paste: paste.into(),
        import_file: if view.busy {
            s.batch_reading.clone()
        } else {
            format!("{} (xlsx / csv / txt)", s.batch_import_file).into()
        },
        template: if view.template_saved {
            s.batch_template_saved.clone()
        } else {
            s.batch_template.clone()
        },
        rate_section: s.batch_rate_section.clone(),
        rate_value: rate_value.into(),
        rate_hint: rate_hint.into(),
        // Lines READ, not rows kept: the count above a list is the length of
        // that list, refused lines included (the web's `seen`).
        parsed: fill(
            &s.batch_parsed,
            "n",
            &(view.preview.len() + view.errors.len()).to_string(),
        )
        .into(),
        // The page adds it: the line reads the account's balance, which the
        // importer's own view does not carry (`batch_total`).
        total: None,
        rows: batch_rows(view, symbol, s),
        rejected: rejected.into(),
        // A file that could not be read must SAY so — the core raises the
        // flag for exactly that, and a picker that silently does nothing is
        // indistinguishable from one that is broken.
        notice: if view.file_error {
            Some(SendNotice {
                dismiss: None,
                title: Some(s.batch_import_failed_title.clone()),
                body: s.batch_import_failed_body.clone(),
                detail: None,
                action: None,
                error: true,
            })
        // Over the balance is said on the total line (`batch_total`), beside
        // the figure it is about — the web's `overText` — not twice.
        } else if view.over_cap {
            Some(SendNotice {
                dismiss: None,
                title: None,
                body: s.batch_over_cap.clone(),
                detail: None,
                action: None,
                error: false,
            })
        } else {
            None
        },
        rate_reset: view.rate_edited.then(|| s.batch_rate_reset.clone()),
        merge: None,
        cta_enabled: view.can_apply,
        cta: if view.recipient_count == 0 {
            s.batch_apply_empty.clone()
        } else {
            fill(&s.batch_apply, "count", &view.recipient_count.to_string()).into()
        },
    }
}

/// The merge line under the importer (issue #265, the web's `merge`): said
/// only when there is somebody on the form for an import to add to — the
/// core's own count, read back from `split_import_room` — and only once the
/// import can happen, beside the button that does it.
#[must_use]
pub fn batch_merge(
    view: &BatchView,
    split_import_room: u32,
    replaces: bool,
    s: &FlowStrings,
) -> Option<(SharedString, SharedString)> {
    let form_has_rows = (split_import_room as usize) < vela_core::app::send::BATCH_MAX_RECIPIENTS;
    (form_has_rows && view.can_apply).then(|| {
        if replaces {
            (s.batch_replaces_rows.clone(), s.batch_add_instead.clone())
        } else {
            (
                s.batch_adds_to_rows.clone(),
                s.batch_replace_instead.clone(),
            )
        }
    })
}

/// Which panel the send journey is on. The core's `stage` decides the step;
/// the two pickers are the core's flags; the fee sheet is the page's own.
#[must_use]
pub fn send_panel(view: &SendView, fee_picker_open: bool) -> crate::flows::FlowPanel {
    use crate::flows::FlowPanel;
    if view.show_contact_picker {
        return FlowPanel::Dsd2e;
    }
    if view.show_batch_import {
        return FlowPanel::Dsd2c;
    }
    if fee_picker_open {
        return FlowPanel::Dsd2f;
    }
    match view.stage {
        SendStage::SelectToken | SendStage::LockError | SendStage::LockResolving => FlowPanel::Dsd1,
        SendStage::EnterDetails if view.split_mode => FlowPanel::Dsd2b,
        SendStage::EnterDetails => FlowPanel::Dsd2,
        SendStage::Confirm => FlowPanel::Dsd3,
        SendStage::Receipt => FlowPanel::Dsd4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::balance_dashboard::{BalanceDashboard, Event as BalanceEvent};

    fn strings() -> FlowStrings {
        FlowStrings::resolve(&crate::loc::Loc::from_env())
    }

    fn wallet_strings() -> crate::wallet::WalletStrings {
        crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// Issue 201: the fee was the one figure on the send screens with no money
    /// beside it — the amount had its "≈" line, the fee did not, so a person
    /// who does not track the coin's price could not tell what a transfer
    /// cost.
    #[test]
    fn a_fee_says_what_it_costs_when_something_can_price_it() {
        use vela_core::app::fee_policy::{FeeOptionView, FeeTier};
        let quote = FeeEstimateView {
            chain_id: 56,
            total_wei: "91000000000000".to_owned(),
            max_fee_per_gas: "1".to_owned(),
            network_fee_per_gas: "1".to_owned(),
            relayer_fee_per_gas: "0".to_owned(),
            bundler_gas_price: "1".to_owned(),
            in_band_gas_basis: "1".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "1".to_owned(),
            deployed: true,
            tier: FeeTier::Fast,
            quoted: true,
            fee_asset: FeeAssetView::Native,
            fee_recipient: None,
        };
        let mut fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        // Nothing can price the coin: the line is the coin alone, never a
        // figure this file invented.
        assert_eq!(
            fee_line(
                Some(&quote),
                None,
                &fee,
                "en",
                crate::wallet::live::Money::usd()
            ),
            "0.000091 BNB"
        );

        // The relay's published row prices it.
        fee.options = vec![FeeOptionView {
            symbol: "BNB".to_owned(),
            contract: None,
            decimals: 18,
            balance: "1500000000000000000".to_owned(),
            recipient: "0x1".to_owned(),
            usd_balance: "900".to_owned(),
            usd_price: Some("600".to_owned()),
            amount: Some("91000000000000".to_owned()),
            insufficient: false,
            selected: true,
        }];
        assert_eq!(
            fee_line(
                Some(&quote),
                None,
                &fee,
                "en",
                crate::wallet::live::Money::usd()
            ),
            "0.000091 BNB · ≈$0.05"
        );

        // Under half a cent the coin amount is the honest primary: "$0.00"
        // beside a real fee reads as free.
        let dust = FeeEstimateView {
            total_wei: "1000000000000".to_owned(),
            ..quote.clone()
        };
        assert_eq!(
            fee_line(
                Some(&dust),
                None,
                &fee,
                "en",
                crate::wallet::live::Money::usd()
            ),
            "0.000001 BNB"
        );
    }

    /// Device-found: the core resolves `first_time` only while the confirm
    /// page is up (`confirm_probes`), so the form's note never had it. The
    /// page that signs says it — for one recipient, never a split.
    #[test]
    fn the_confirm_says_it_is_the_first_time_sending_here() {
        use vela_core::app::send::{Send, SendRecipientRisk};
        let s = strings();
        let wallet = wallet_strings();
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        let tag = |send: &SendView| {
            send_confirm(&SendInputs {
                send,
                fee: &fee,
                s: &s,
                wallet: &wallet,
                locale: "en",
                money: crate::wallet::live::Money::usd(),
                identity_name: "Golden",
                identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                speed: None,
            })
            .recipient_tag
        };
        let mut send = CoreHost::<Send>::new().view();
        send.recipient = format!("0x{}", "ab".repeat(20));
        assert_eq!(tag(&send), None);
        let risk = |first_time| {
            Some(SendRecipientRisk {
                is_contract: Some(false),
                first_time: Some(first_time),
            })
        };
        send.recipient_risk = risk(false);
        assert_eq!(tag(&send), None);
        send.recipient_risk = risk(true);
        assert_eq!(tag(&send), Some(s.first_time_tag.clone()));
        send.split_mode = true;
        assert_eq!(tag(&send), None, "a split has no one recipient");
    }

    /// A receipt in each of the two states the core can hold one in.
    ///
    /// The `SendView` is the booted core's; only the receipt is substituted,
    /// because reaching a real fee hold means driving a submit and a relay
    /// answer, and what is under test is the sentence, not the pipeline.
    fn receipt_with(
        status: vela_core::app::send::SendReceiptStatus,
        hold: Option<vela_core::app::send::SendHoldReason>,
    ) -> SendReceipt {
        use vela_core::app::send::{Send, SendReceiptView};
        let s = strings();
        let wallet = wallet_strings();
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        let mut send = CoreHost::<Send>::new().view();
        send.receipt = Some(SendReceiptView {
            status,
            hold_reason: hold,
            kind: None,
            transfers: Vec::new(),
            amount: "0.001".to_owned(),
            usd_value: 0.0,
            submitted_at_ms: None,
            typical_inclusion_s: None,
        });
        send_receipt(&SendInputs {
            send: &send,
            fee: &fee,
            s: &s,
            wallet: &wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
        })
    }

    /// 078 F-04: each receipt state is its own stage — the disc's colour and
    /// mark — and only a confirmation wears the accent "Done" and a full ring.
    #[test]
    fn each_receipt_state_draws_its_own_stage() {
        use vela_core::app::send::SendReceiptStatus;
        let confirmed = receipt_with(SendReceiptStatus::Confirmed, None);
        assert_eq!(confirmed.stage, ReceiptStage::Confirmed);
        assert!(confirmed.cta_accent);
        assert_eq!(confirmed.progress, Some(1.0));

        let submitted = receipt_with(SendReceiptStatus::Submitted, None);
        assert_eq!(submitted.stage, ReceiptStage::Submitted);
        assert!(!submitted.cta_accent);
        assert_eq!(submitted.progress, None, "no estimate: the ring roams");

        let failed = receipt_with(SendReceiptStatus::Failed, None);
        assert_eq!(failed.stage, ReceiptStage::Failed);
        assert!(failed.explorer.is_none() && failed.hash.is_none());
    }

    /// The ring eases toward full and never reaches it by waiting: ~70% at
    /// the chain's usual time, under the 92% ceiling at any length.
    #[test]
    fn the_ring_never_closes_by_itself() {
        assert_eq!(ring_progress(10, 0), None);
        assert_eq!(ring_progress(0, 12), Some(0.0));
        let at_typical = ring_progress(12, 12).unwrap_or_default();
        assert!((0.65..0.72).contains(&at_typical), "{at_typical}");
        assert!(ring_progress(10_000, 12).unwrap_or_default() <= 0.92);
    }

    /// Spec 038 #D2: a split's receipt lists every recipient the core froze,
    /// with an avatar each, and counts them where the single send names its
    /// one recipient — never "To " with nobody after it.
    #[test]
    fn a_split_receipt_lists_its_recipients_and_counts_them() {
        use vela_core::app::send::{
            Send, SendReceiptKind, SendReceiptStatus, SendReceiptTransfer, SendReceiptView,
        };
        let s = strings();
        let wallet = wallet_strings();
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        let mut send = CoreHost::<Send>::new().view();
        let transfer = |to: &str, name: Option<&str>, amount: &str| SendReceiptTransfer {
            to: to.to_owned(),
            to_name: name.map(str::to_owned),
            amount: amount.to_owned(),
            symbol: "ETH".to_owned(),
            logo_urls: Vec::new(),
            usd_value: 0.0,
        };
        send.tx_hash = Some("0xtx".to_owned());
        send.receipt = Some(SendReceiptView {
            status: SendReceiptStatus::Confirmed,
            hold_reason: None,
            kind: Some(SendReceiptKind::Split),
            transfers: vec![
                transfer(&format!("0x{}", "cd".repeat(20)), Some("Alice"), "0.2"),
                transfer(&format!("0x{}", "ef".repeat(20)), None, "0.3"),
            ],
            amount: "0.5".to_owned(),
            usd_value: 0.0,
            submitted_at_ms: None,
            typical_inclusion_s: None,
        });
        let receipt = send_receipt(&SendInputs {
            send: &send,
            fee: &fee,
            s: &s,
            wallet: &wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
        });
        let title = s.recipients(2);
        assert_eq!(receipt.breakdown_title.as_deref(), Some(title.as_str()));
        let labels: Vec<&str> = receipt
            .breakdown
            .iter()
            .map(|row| row.label.as_ref())
            .collect();
        assert_eq!(labels[0], "Alice");
        assert!(labels[1].starts_with("0xef"), "{}", labels[1]);
        assert!(receipt.breakdown.iter().all(|row| row.seed.is_some()));
        assert_eq!(receipt.breakdown[1].value.as_ref(), "0.3 ETH");
        assert!(
            receipt.captions[0].contains(&title),
            "{}",
            receipt.captions[0]
        );

        // A single send carries no parts.
        let single = receipt_with(SendReceiptStatus::Confirmed, None);
        assert!(single.breakdown.is_empty());
        assert!(single.breakdown_title.is_none());
    }

    /// A payment queued until fees settle is NOT waiting for a confirmation.
    /// Saying the ordinary wait over it leaves somebody watching a transfer
    /// that will not move for as long as the fee stays up, with nothing on
    /// screen to explain it — and the sentence has been in the corpus the
    /// whole time.
    #[test]
    fn a_payment_held_for_fees_says_it_is_held_for_fees() {
        use vela_core::app::send::{SendHoldReason, SendReceiptStatus};
        let s = strings();
        let held = receipt_with(SendReceiptStatus::Submitted, Some(SendHoldReason::FeeHold));
        assert_eq!(held.captions, vec![s.tx_held_fees.clone()]);
        assert_ne!(
            held.captions,
            vec![s.tx_waiting_confirm.clone()],
            "the ordinary wait must not stand in for the hold"
        );

        // No hold: the ordinary wait, unchanged.
        let plain = receipt_with(SendReceiptStatus::Submitted, None);
        assert_eq!(plain.captions, vec![s.tx_waiting_confirm]);
    }

    /// A fee rejection is not a generic failure: nothing was sent, and the
    /// way out is to send again at the current fee rather than to retry the
    /// same one.
    #[test]
    fn a_fee_rejection_says_what_to_do_instead() {
        use vela_core::app::send::{SendHoldReason, SendReceiptStatus};
        let s = strings();
        let rejected = receipt_with(SendReceiptStatus::Failed, Some(SendHoldReason::FeeRejected));
        assert_eq!(rejected.captions, vec![s.tx_rejected_fees]);
        assert!(
            receipt_with(SendReceiptStatus::Failed, None)
                .captions
                .is_empty(),
            "a failure the core gave no reason for invents none"
        );
        // `fee_held` survives into a failed receipt, so Failed + FeeHold is
        // reachable — and the queued sentence under a failure would be worse
        // than silence.
        assert!(
            receipt_with(SendReceiptStatus::Failed, Some(SendHoldReason::FeeHold))
                .captions
                .is_empty(),
            "the queued sentence never appears under a failure"
        );
    }

    /// The address is not handed over until the warning has been read.
    ///
    /// Two things follow the core's gate, and both were the shell's own guess
    /// until spec 032 phase 38: the code is REPLACED by the warning (a cover
    /// somebody can read around is one they will read around), and the copy
    /// affordance is withheld — `can_copy`, not "is there a payload".
    #[test]
    fn the_receive_screen_asks_before_it_hands_the_address_over() {
        let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
        let watch =
            crate::core_host::CoreHost::<vela_core::app::receive_watch::ReceiveWatch>::new().view();
        let mut pay = pay_view();

        // Fresh account: the flag is being read, so the cover is up and even
        // its button is withheld.
        pay.gate_loading = true;
        pay.acknowledged = false;
        pay.can_copy = false;
        let qr = receive_qr(
            "0xabc",
            "Golden",
            100,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        let gate = qr.gate.as_ref().unwrap_or_else(|| unreachable!("no gate"));
        assert!(
            gate.loading,
            "the button appeared while the flag was loading"
        );
        assert!(!qr.can_copy);

        // Read, not yet acknowledged: the warning stands, with its button.
        pay.gate_loading = false;
        let qr = receive_qr(
            "0xabc",
            "Golden",
            100,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        let gate = qr.gate.as_ref().unwrap_or_else(|| unreachable!("no gate"));
        assert!(!gate.loading);
        assert_eq!(gate.confirm, s.warning_confirm);
        assert!(
            !gate.title.is_empty() && !gate.body.is_empty() && !gate.counterfactual.is_empty(),
            "the warning must say what it is about, and why one address is enough"
        );

        // Acknowledged: the code appears and the address can be copied.
        pay.acknowledged = true;
        pay.can_copy = true;
        let qr = receive_qr(
            "0xabc",
            "Golden",
            100,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        assert!(qr.gate.is_none());
        assert!(qr.can_copy);
    }

    /// A NETWORK code wears the network's logo, not its native coin's: on
    /// Base the coin is ETH, and the coin's rule would put Ethereum's mark in
    /// the middle of a Base code (the web's `chainMark`).
    #[test]
    fn a_network_code_wears_the_network_and_a_token_code_the_token() {
        let s = strings();
        let watch =
            crate::core_host::CoreHost::<vela_core::app::receive_watch::ReceiveWatch>::new().view();
        let pay = pay_view();
        let qr = receive_qr(
            "0xabc",
            "Golden",
            8453,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        assert_eq!(qr.centre.logos, crate::marks::chain_logos(8453));
        assert_ne!(
            qr.centre.logos,
            crate::marks::token_logos(8453, "ETH", None, &[]),
            "a Base code wore Ethereum's mark"
        );
        assert!(qr.contract.is_none(), "a network code names no contract");

        // The token's own code: its mark, its contract, its symbol in the title.
        let usdc = BalanceToken {
            token_address: Some("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913".to_owned()),
            ..token(8453, "USDC", "5", Some(1.0))
        };
        let qr = receive_token_qr(
            "0xabc",
            "Golden",
            &usdc,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        assert_eq!(
            qr.centre.logos,
            crate::marks::token_logos(8453, "USDC", usdc.token_address.as_deref(), &[])
        );
        assert!(qr.title.contains("USDC"), "{}", qr.title);
        let (label, value) = qr
            .contract
            .unwrap_or_else(|| unreachable!("no contract line"));
        assert_eq!(label, s.token_contract);
        assert_eq!(
            value,
            SharedString::from(shorten("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"))
        );
        // The line is shortened; its copy carries the WHOLE contract.
        assert_eq!(
            qr.contract_copy.as_deref(),
            Some("0x833589fcd6edb6e08f4c7c32d4f71b54bda02913")
        );

        // The chain's own coin has no contract, and says so in words.
        let eth = token(8453, "ETH", "1", None);
        let qr = receive_token_qr(
            "0xabc",
            "Golden",
            &eth,
            &watch,
            &pay,
            &s,
            "en",
            crate::wallet::live::Money::usd(),
        );
        assert!(
            qr.contract_copy.is_none(),
            "a native coin has nothing to copy"
        );
        assert_eq!(qr.contract.map(|c| c.1), Some(s.label_native_token.clone()));
    }

    /// A real `PaymentRequestView`, from a booted core.
    fn pay_view() -> PaymentRequestView {
        use vela_core::app::payment_request::{Event as PayEvent, PaymentRequest};
        let mut host = CoreHost::<PaymentRequest>::new();
        let _ = host.dispatch(PayEvent::Start {
            account: "0xabc".to_owned(),
            recipient: "0xabc".to_owned(),
            base_url: "https://getvela.app".to_owned(),
        });
        host.view()
    }

    /// A real `BalanceView`, taken from a booted core rather than hand-written.
    fn view() -> BalanceView {
        let mut host = CoreHost::<BalanceDashboard>::new();
        let _ = host.dispatch(BalanceEvent::AccountChanged {
            address: "0xabc".to_owned(),
        });
        host.view()
    }

    fn token(chain_id: u32, symbol: &str, balance: &str, price: Option<f64>) -> BalanceToken {
        BalanceToken {
            chain_id,
            symbol: symbol.to_owned(),
            name: symbol.to_owned(),
            balance: balance.to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: price,
            spam: false,
        }
    }

    /// A holding renders its amount and its value; one nobody could price says
    /// so instead of showing zero.
    #[test]
    fn an_unpriced_holding_says_so_rather_than_showing_nothing() {
        let mut view = view();
        view.tokens = vec![
            token(100, "xDAI", "0.75897", Some(1.0)),
            token(143, "MON", "12.5", None),
        ];
        view.unpriced_tokens = vec![token(143, "MON", "12.5", None)];

        let panel = assets(
            &view,
            &strings(),
            &wallet_strings(),
            "en-US",
            None,
            crate::wallet::live::Money::usd(),
        );
        assert_eq!(panel.rows.len(), 2);
        assert_eq!(panel.rows[0].ticker, "xDAI");
        assert_eq!(panel.rows[0].chain, "Gnosis");
        assert!(matches!(panel.rows[0].fiat, Fiat::Value(_)));
        // The one the core could not price. `$0.00` here would tell somebody
        // their holding is worthless.
        assert!(
            matches!(panel.rows[1].fiat, Fiat::NoPrice(_)),
            "an unpriced holding must not render as a value"
        );
        assert!(panel.empty.is_none(), "there are rows");
    }

    /// Privacy masks the figure AND the value, on this surface as on the hero.
    #[test]
    fn hiding_the_balance_hides_it_here_too() {
        let mut view = view();
        view.tokens = vec![token(100, "xDAI", "0.75897", Some(1.0))];
        view.hidden = true;

        let panel = assets(
            &view,
            &strings(),
            &wallet_strings(),
            "en-US",
            None,
            crate::wallet::live::Money::usd(),
        );
        assert_eq!(panel.rows[0].balance, MASK);
        assert!(matches!(panel.rows[0].fiat, Fiat::Masked));
        // The unit survives — H5's rule. The figure is what goes.
        assert_eq!(panel.rows[0].ticker, "xDAI");
    }

    /// An empty wallet and a wallet still being counted are different screens.
    #[test]
    fn the_guided_empty_waits_until_the_core_has_ruled() {
        let mut counting = view();
        counting.tokens = Vec::new();
        counting.balance_unknown = true;
        assert!(
            assets(
                &counting,
                &strings(),
                &wallet_strings(),
                "en-US",
                None,
                crate::wallet::live::Money::usd()
            )
            .empty
            .is_none(),
            "still counting: no 'your wallet is empty'"
        );

        let mut loading = view();
        loading.tokens = Vec::new();
        loading.balance_unknown = false;
        loading.holdings_loading = true;
        assert!(
            assets(
                &loading,
                &strings(),
                &wallet_strings(),
                "en-US",
                None,
                crate::wallet::live::Money::usd()
            )
            .empty
            .is_none()
        );

        let mut settled = view();
        settled.tokens = Vec::new();
        settled.balance_unknown = false;
        settled.holdings_loading = false;
        assert!(
            assets(
                &settled,
                &strings(),
                &wallet_strings(),
                "en-US",
                None,
                crate::wallet::live::Money::usd()
            )
            .empty
            .is_some(),
            "the core ruled: genuinely empty"
        );

        // Narrowed to a chain that holds nothing while others hold something:
        // the web's `filteredEmpty` — the empty body, never a blank column.
        // Even while a refresh is still counting, because the rows that do
        // exist already say the chain has nothing on it.
        let mut narrowed = view();
        narrowed.tokens = vec![token(100, "xDAI", "1", Some(1.0))];
        narrowed.holdings_loading = true;
        let panel = assets(
            &narrowed,
            &strings(),
            &wallet_strings(),
            "en-US",
            Some(1),
            crate::wallet::live::Money::usd(),
        );
        assert!(panel.rows.is_empty());
        assert!(
            panel.empty.is_some(),
            "a chain with nothing on it drew a blank column"
        );
    }

    /// The filter dots are the chains this person actually holds on.
    #[test]
    fn the_chain_filter_offers_only_chains_with_something_on_them() {
        let mut view = view();
        view.tokens = vec![
            token(100, "xDAI", "1", Some(1.0)),
            token(100, "USDC", "1", Some(1.0)),
            token(1, "ETH", "1", Some(2000.0)),
            token(56, "BNB", "1", Some(700.0)),
            token(137, "POL", "1", Some(0.4)),
        ];
        let panel = assets(
            &view,
            &strings(),
            &wallet_strings(),
            "en-US",
            None,
            crate::wallet::live::Money::usd(),
        );
        let dots = panel
            .filter
            .as_ref()
            .map(|(dots, _, _)| dots.len())
            .unwrap_or_default();
        // Deduped by chain (Gnosis appears twice) and capped at three.
        assert_eq!(dots, 3);
    }

    /// Every network shows the SAME address — that is what a Safe is.
    #[test]
    fn every_receive_row_shows_one_address() {
        crate::executor::storage::tests::with_temp_state("flows-receive", || {
            const ADDR: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let list = receive_list(ADDR, &strings());
            assert_eq!(list.rows.len(), BUILTIN_CHAINS.len());
            let first = list.rows[0].address.clone();
            for row in &list.rows {
                assert_eq!(row.address, first, "one address, every chain");
            }
            assert!(list.subtitle.contains(&BUILTIN_CHAINS.len().to_string()));

            // A network the person added joins the list, named by their name.
            let networks = serde_json::json!([
                { "chainId": 7_777_777, "nativeSymbol": "TST", "displayName": "My testnet" }
            ]);
            if crate::executor::storage::write_value(
                crate::executor::storage::KEY_CUSTOM_NETWORKS,
                networks,
            )
            .is_err()
            {
                unreachable!("could not seed");
            }
            let list = receive_list(ADDR, &strings());
            assert_eq!(list.rows.len(), BUILTIN_CHAINS.len() + 1);
            assert!(list.rows.iter().any(|row| row.name == "My testnet"));
        });
    }

    /// The QR card carries the real address, and the identicon is seeded by it.
    #[test]
    fn the_qr_card_is_seeded_by_the_address_not_the_name() {
        crate::executor::storage::tests::with_temp_state("flows-qr", || {
            const ADDR: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let quiet = ReceiveWatchView {
                detected: false,
                deposits: Vec::new(),
            };
            let mut pay = pay_view();
            pay.qr_value = ADDR.to_owned();
            let qr = receive_qr(
                ADDR,
                "Everyday wallet",
                100,
                &quiet,
                &pay,
                &strings(),
                "en-US",
                crate::wallet::live::Money::usd(),
            );
            assert_eq!(qr.account.name, "Everyday wallet");
            assert_eq!(qr.account.seed, ADDR, "two same-named accounts must differ");
            // The two halves rejoin into the address the person will paste.
            let joined = format!("{}{}", qr.account.lines.0, qr.account.lines.1);
            assert_eq!(joined, ADDR);
            assert!(qr.title.contains("Gnosis"));
            assert_eq!(qr.centre.ticker, "xDAI");
            // A network code is not a token code: no contract line here.
            assert_eq!(qr.contract, None);
        });
    }

    /// The add-token panel says which thing is true, the web's way (078
    /// F-07): nothing typed says nothing; searching and not-found are a line;
    /// a found token is a card, "Added" when it is already there; and the CTA
    /// acts only when there is something new to add.
    #[test]
    fn the_add_token_panel_distinguishes_its_states() {
        use crate::flows::fixtures::AddTokenResult;
        use vela_core::app::manage_tokens::{Event as MtokEvent, ManageTokens, MtokFound};

        let mut host = CoreHost::<ManageTokens>::new();
        let _ = host.dispatch(MtokEvent::Start);
        let base = host.view();
        let s = strings();

        let idle = add_token(&base, &s);
        assert_eq!(idle.field_value, "");
        assert!(matches!(idle.result, AddTokenResult::Empty));
        assert!(idle.cta_disabled, "nothing found, nothing to add");
        assert_eq!(idle.field_error, None);

        let mut looking = base.clone();
        looking.detecting = true;
        looking.input_address = "0xaaa".to_owned();
        match &add_token(&looking, &s).result {
            AddTokenResult::Note(line) => assert_eq!(*line, s.searching_networks),
            _ => unreachable!("searching is a line"),
        }

        let mut bad = base.clone();
        bad.input_address = "0xnope".to_owned();
        bad.address_valid = false;
        assert_eq!(
            add_token(&bad, &s).field_error,
            Some(s.invalid_contract.clone())
        );

        let mut missing = base.clone();
        missing.not_found = true;
        match &add_token(&missing, &s).result {
            AddTokenResult::Note(line) => {
                assert!(line.contains(s.not_found_title.as_ref()));
                assert!(line.contains(s.not_found_message.as_ref()));
            }
            _ => unreachable!("not found is a line"),
        }

        // A native coin that also answers an ERC-20 interface: refused with
        // its reason (spec 060), not "not found".
        let mut native = base.clone();
        native.native_alias = true;
        match &add_token(&native, &s).result {
            AddTokenResult::Note(line) => {
                assert!(line.contains(s.native_alias_title.as_ref()));
                assert!(!line.contains(s.not_found_title.as_ref()));
            }
            _ => unreachable!(),
        }

        let mut found = base;
        found.found = vec![MtokFound {
            chain_id: 100,
            network_name: "Gnosis".to_owned(),
            name: "USD Coin".to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 6,
            added: false,
        }];
        let card = add_token(&found, &s);
        match &card.result {
            AddTokenResult::Token {
                mark,
                name,
                detail,
                chip,
            } => {
                assert_eq!(*name, "USD Coin");
                assert_eq!(mark.ticker, "USDC");
                assert!(detail.contains("Gnosis"));
                assert!(detail.contains('6'), "no decimals: {detail}");
                assert!(chip.is_none());
            }
            _ => unreachable!(),
        }
        assert!(!card.cta_disabled);

        found.found[0].added = true;
        let card = add_token(&found, &s);
        assert!(matches!(
            &card.result,
            AddTokenResult::Token { chip: Some(_), .. }
        ));
        assert!(card.cta_disabled, "an added token is not added twice");
    }

    /// The native tab: nothing typed says nothing and cannot add; a query the
    /// index matches offers its chains; one it does not says so.
    #[test]
    fn the_native_tab_offers_the_chains_it_matched() {
        use crate::flows::fixtures::AddTokenResult;
        use vela_core::app::network_admin::{NetChainIndexEntry, NetWizardPhase};
        let s = strings();
        let mut wizard = CoreHost::<vela_core::app::network_admin::NetworkAdmin>::new()
            .view()
            .wizard;

        let idle = add_network_tab(&wizard, "", None, &s);
        assert!(idle.native);
        assert!(matches!(idle.result, AddTokenResult::Empty));
        assert!(idle.cta_disabled);

        wizard.phase = NetWizardPhase::Suggested;
        wizard.suggestions = vec![NetChainIndexEntry {
            chain_id: 43_114,
            name: "Avalanche".to_owned(),
            short_name: "avax".to_owned(),
            native_currency_symbol: "AVAX".to_owned(),
            has_logo: true,
        }];
        match add_network_tab(&wizard, "aval", None, &s).result {
            AddTokenResult::Suggestions(rows) => {
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].chain_id, 43_114);
            }
            _ => unreachable!("a match is offered"),
        }

        wizard.suggestions.clear();
        match add_network_tab(&wizard, "zzz", None, &s).result {
            AddTokenResult::Note(line) => assert!(line.contains("zzz")),
            _ => unreachable!("no match says so"),
        }
    }

    /// A transaction's detail, and the row order the listeners are bound in.
    #[test]
    fn a_transaction_opens_its_own_detail_and_the_row_order_matches() {
        crate::executor::storage::tests::with_temp_state("flows-tx-detail", || {
            use vela_core::app::activity_feed::{
                ActivityFeed, Event as FeedEvent, FeedDirection, FeedItem, FeedTxRecord,
            };

            let mut host = CoreHost::<ActivityFeed>::new();
            let _ = host.dispatch(FeedEvent::AccountSwitched {
                address: "0xme".to_owned(),
            });
            let today = crate::executor::day_start_ms(crate::executor::now_ms());
            let item = |id: &str, incoming: bool| FeedItem {
                id: id.to_owned(),
                direction: if incoming {
                    FeedDirection::In
                } else {
                    FeedDirection::Out
                },
                counterparty: Some("0xAbCdEf0000000000000000000000000000000001".to_owned()),
                alias: None,
                value: Some("1.5".to_owned()),
                symbol: "xDAI".to_owned(),
                decimals: Some(18),
                usd_value: 1.5,
                chain_id: 100,
                timestamp: today / 1000.0 + 3600.0,
                day_start_ms: today,
                tx_hash: Some("0xdead".to_owned()),
                batch: None,
            };
            let view = FeedView {
                rows: vec![
                    FeedRow::Header {
                        id: "day-0".to_owned(),
                        day_start_ms: today,
                        timestamp: today / 1000.0,
                    },
                    FeedRow::Item {
                        item: item("a", true),
                    },
                    FeedRow::Item {
                        item: item("b", false),
                    },
                ],
                transactions: vec![FeedTxRecord {
                    id: "b".to_owned(),
                    user_op_hash: String::new(),
                    tx_hash: "0xdead".to_owned(),
                    from: "0xme".to_owned(),
                    to: "0xAbCdEf0000000000000000000000000000000001".to_owned(),
                    to_name: None,
                    value: "1.5".to_owned(),
                    symbol: "xDAI".to_owned(),
                    decimals: 18,
                    logo_urls: None,
                    chain_id: 100,
                    timestamp: today / 1000.0 + 3600.0,
                    day_start_ms: today,
                    status: FeedTxStatus::Pending,
                    kind: None,
                    usd: None,
                }],
                ..host.view()
            };

            // The listeners are bound in the order the rows draw.
            assert_eq!(history_ids(&view), vec!["a".to_owned(), "b".to_owned()]);

            let s = strings();
            let received = tx_detail(
                &view,
                "a",
                &s,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("row a exists"));
            assert!(received.positive);
            assert_eq!(received.amount, "+1.5 xDAI");
            assert_eq!(received.fiat, "$1.50");
            // No stored record for "a", so the status is the confirmed default
            // rather than a guess at something worse.
            assert_eq!(received.status.text, s.status_confirmed);
            // From (not To) for a receipt, plus chain, date and hash.
            assert_eq!(received.facts[0].label, s.detail_from);
            assert_eq!(received.facts[1].label, s.detail_chain);
            assert_eq!(received.facts[2].label, s.detail_date);
            assert_eq!(received.facts[3].label, s.detail_hash);
            assert!(
                received.facts[3].mono,
                "a hash is compared character by character"
            );
            // "View on Explorer" opens this transaction on its own chain.
            assert_eq!(
                received.explorer_url.as_deref(),
                Some("https://gnosisscan.io/tx/0xdead")
            );

            // The sent one, whose stored record says pending — it must NOT
            // wear the confirmed chip.
            let sent = tx_detail(
                &view,
                "b",
                &s,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("row b exists"));
            assert!(!sent.positive);
            assert_eq!(sent.facts[0].label, s.detail_to);
            assert_eq!(sent.status.text, s.status_pending);
            assert!(matches!(sent.status.tone, StatusTone::Info));

            // Privacy masks the figure here as everywhere.
            let hidden = tx_detail(
                &view,
                "a",
                &s,
                true,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("row a exists"));
            assert_eq!(hidden.amount, crate::wallet::fixtures::MASK);
            assert_eq!(hidden.fiat, "");

            // A row that no longer exists has no detail — the panel closes
            // rather than showing a stale one.
            assert!(
                tx_detail(
                    &view,
                    "gone",
                    &s,
                    false,
                    "en-US",
                    crate::wallet::live::Money::usd()
                )
                .is_none()
            );

            // A live record can be deleted from its detail (the web's
            // `deleteLabel`); the mocks cannot.
            assert_eq!(received.delete_label.as_ref(), Some(&s.delete_record));
            let drawn = crate::flows::fixtures::body(crate::flows::FlowPanel::Da2, &s);
            let crate::flows::fixtures::FlowBody::TxDetail(drawn) = drawn else {
                unreachable!("DA2L is a transaction detail")
            };
            assert!(drawn.delete_label.is_none(), "a picture deletes nothing");
        });
    }

    /// The QR encodes what the CORE says, and a live one is never the demo
    /// pattern.
    #[test]
    fn the_code_carries_the_cores_payload_not_the_shells_guess() {
        crate::executor::storage::tests::with_temp_state("flows-qr-payload", || {
            const ADDR: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let quiet = ReceiveWatchView {
                detected: false,
                deposits: Vec::new(),
            };

            // A booted `payment_request` in address mode answers the recipient.
            let mut pay = pay_view();
            pay.qr_value = ADDR.to_owned();
            let qr = receive_qr(
                ADDR,
                "Me",
                100,
                &quiet,
                &pay,
                &strings(),
                "en-US",
                crate::wallet::live::Money::usd(),
            );
            assert_eq!(qr.qr_payload.as_deref(), Some(ADDR));

            // Request mode puts an EIP-681 URI in the same field, and this file
            // must forward it rather than re-deriving the address.
            pay.qr_value = "ethereum:0x88cC@100?value=1.5e18".to_owned();
            let request = receive_qr(
                ADDR,
                "Me",
                100,
                &quiet,
                &pay,
                &strings(),
                "en-US",
                crate::wallet::live::Money::usd(),
            );
            assert_eq!(
                request.qr_payload.as_deref(),
                Some("ethereum:0x88cC@100?value=1.5e18")
            );

            // Before the core has ruled there is nothing to encode, and drawing
            // a decorative code on a screen meant to be scanned is the failure
            // this field exists to end.
            pay.qr_value = String::new();
            let unruled = receive_qr(
                ADDR,
                "Me",
                100,
                &quiet,
                &pay,
                &strings(),
                "en-US",
                crate::wallet::live::Money::usd(),
            );
            assert_eq!(unruled.qr_payload, None);
        });
    }

    /// A deposit is announced when the core says one landed — and only then.
    #[test]
    fn an_arrival_is_announced_only_when_the_core_says_one_landed() {
        use vela_core::app::receive_watch::{DepositEntry, DepositItem};

        let entry = DepositEntry {
            // 2026-09-04T12:34:56Z, shifted into whatever zone this machine is
            // in — the assertion is on shape, not on a clock the test cannot
            // know.
            at_epoch_ms: 1_788_525_296_000.0,
            items: vec![
                DepositItem {
                    symbol: "xDAI".to_owned(),
                    amount: 1.5,
                    chain_id: 100,
                    usd: Some(1.5),
                },
                DepositItem {
                    symbol: "MON".to_owned(),
                    amount: 12.0,
                    chain_id: 143,
                    usd: None,
                },
            ],
        };

        // A list the core has NOT called detected is not a celebration.
        let quiet = ReceiveWatchView {
            detected: false,
            deposits: vec![entry.clone()],
        };
        assert!(
            deposits(&quiet, "en-US", crate::wallet::live::Money::usd()).is_empty(),
            "undetected must not announce"
        );

        let landed = ReceiveWatchView {
            detected: true,
            deposits: vec![entry],
        };
        let announced = deposits(&landed, "en-US", crate::wallet::live::Money::usd());
        assert_eq!(announced.len(), 1);
        assert_eq!(announced[0].rows.len(), 2);
        assert_eq!(announced[0].rows[0].0, "+1.5 xDAI");
        assert!(announced[0].rows[0].1.starts_with("Gnosis"));
        assert!(announced[0].rows[0].1.contains("$1.50"));
        // An unpriced arrival still says which chain it came in on; `$0.00`
        // beside it would be the assets panel's mistake on a happier screen.
        assert_eq!(announced[0].rows[1].0, "+12 MON");
        assert_eq!(announced[0].rows[1].1, "Monad");
        // The time is a wall clock, not an epoch.
        assert!(
            announced[0].time.contains(':') && announced[0].time.len() <= 8,
            "not a clock: {}",
            announced[0].time
        );
    }

    /// The assets panel, end to end, against the golden Safe.
    ///
    /// The hero says one number; this is the screen that has to justify it. A
    /// total nobody can break down is a total nobody can check.
    #[test]
    #[ignore = "reads every chain for a real address"]
    fn the_assets_panel_lists_what_the_hero_totals() {
        use crate::resident::{Answer, Machine};

        crate::executor::storage::tests::with_temp_state("flows-assets-live", || {
            crate::executor::chain_tokens::invalidate();
            crate::executor::chainlink::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

            let mut host = CoreHost::<BalanceDashboard>::new();
            let mut pending = host.dispatch(BalanceEvent::AccountChanged {
                address: GOLDEN.to_owned(),
            });
            for _ in 0..64 {
                let Some(next) = pending.pop() else { break };
                let result = match BalanceDashboard::perform(&next.operation) {
                    Answer::Now(result) | Answer::After(_, result) => result,
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
                };
                pending.extend(host.resolve(next.id, result));
            }
            let view = host.view();
            let panel = assets(
                &view,
                &strings(),
                &wallet_strings(),
                "en-US",
                None,
                crate::wallet::live::Money::usd(),
            );

            for row in &panel.rows {
                println!(
                    "    {:>10} {:<8} {:<12} {}",
                    row.balance,
                    row.ticker,
                    row.chain,
                    match &row.fiat {
                        Fiat::Value(v) => v.to_string(),
                        Fiat::NoPrice(v) => v.to_string(),
                        Fiat::Masked => "•••".to_owned(),
                    }
                );
            }

            assert!(
                !panel.rows.is_empty(),
                "the hero has a total; this has rows"
            );
            assert!(
                panel.empty.is_none(),
                "a funded wallet must not be told it is empty"
            );
            let gnosis = panel
                .rows
                .iter()
                .find(|row| row.chain == "Gnosis" && row.ticker == "xDAI")
                .unwrap_or_else(|| unreachable!("Gnosis xDAI is missing"));
            // The row's own figure, not base units: the same bug the hero test
            // pins, one surface further out.
            let amount: f64 = gnosis
                .balance
                .replace(',', "")
                .parse()
                .unwrap_or_else(|_| unreachable!("not an amount: {}", gnosis.balance));
            assert!(amount > 0.0 && amount < 1_000.0, "implausible: {amount}");
            assert!(matches!(gnosis.fiat, Fiat::Value(_)), "xDAI is priced");

            // And the total the hero shows is the sum of what this lists.
            let listed: f64 = view
                .tokens
                .iter()
                .map(vela_core::app::balance_dashboard::token_usd_value)
                .sum();
            let hero = view
                .display_total_usd
                .unwrap_or_else(|| unreachable!("no total"));
            assert!(
                (listed - hero).abs() < 0.01,
                "the panel lists {listed} and the hero says {hero}"
            );
        });
    }

    /// Issue #265's merge line: said only when somebody is already on the form
    /// and the import can apply, and it offers the other choice.
    #[test]
    fn the_merge_line_speaks_only_when_there_are_rows_to_merge_with() {
        use vela_core::app::batch_import::BatchImport;
        use vela_core::app::send::BATCH_MAX_RECIPIENTS;
        let s = strings();
        let full = BATCH_MAX_RECIPIENTS as u32;
        let ready = BatchView {
            can_apply: true,
            ..CoreHost::<BatchImport>::new().view()
        };

        // An empty form: nothing to add to, so nothing to say.
        assert_eq!(batch_merge(&ready, full, false, &s), None);
        // Rows on the form: adds by default, with the way to replace.
        assert_eq!(
            batch_merge(&ready, full - 2, false, &s),
            Some((
                s.batch_adds_to_rows.clone(),
                s.batch_replace_instead.clone()
            ))
        );
        // …and once the person chose to replace, the other way round.
        assert_eq!(
            batch_merge(&ready, full - 2, true, &s),
            Some((s.batch_replaces_rows.clone(), s.batch_add_instead.clone()))
        );
        // An import that cannot apply says nothing about what it would do.
        let blocked = BatchView {
            can_apply: false,
            ..ready
        };
        assert_eq!(batch_merge(&blocked, full - 2, false, &s), None);
    }

    /// A group pick carries each member's name — the one the person gave, or
    /// the one the book resolved — so the split rows say WHO, not just where.
    #[test]
    fn a_group_pick_carries_each_members_name() {
        use vela_core::app::contacts::{Contact, ContactGroupView, ContactKind, ContactSource};
        let member = |address: &str, name: Option<&str>, resolved: Option<&str>| Contact {
            address: address.to_owned(),
            name: name.map(str::to_owned),
            resolved_name: resolved.map(str::to_owned),
            resolved_source: None,
            kind: ContactKind::Eoa,
            favorite: false,
            note: None,
            tx_count: 0,
            last_used_ms: 0.0,
            first_seen_ms: 0.0,
            source: ContactSource::Manual,
        };
        let view = ContactsView {
            loaded: true,
            sections: Vec::new(),
            contacts: Vec::new(),
            groups: vec![ContactGroupView {
                id: "grp_1".to_owned(),
                name: "Payroll".to_owned(),
                color: None,
                members: vec![
                    member("0xaa", Some("Ana"), Some("ana.eth")),
                    member("0xbb", None, Some("bo.eth")),
                    member("0xcc", None, None),
                ],
            }],
            last_import: None,
            import_failure: None,
            export: None,
            recipient: None,
        };
        let groups = contact_group_members(&view);
        assert_eq!(groups.len(), 1);
        let names: Vec<Option<&str>> = groups[0].iter().map(|r| r.name.as_deref()).collect();
        assert_eq!(names, vec![Some("Ana"), Some("bo.eth"), None]);
        assert!(
            groups[0]
                .iter()
                .all(|r| r.amount.is_empty() && r.id.is_empty()),
            "amounts are the person's to type, ids the core's to give"
        );
        assert_eq!(groups[0][2].address, "0xcc");
    }

    /// DA1L's three modes, the web's `liveHistory`: skeletons while nothing
    /// has been ruled, one line once it has — about the network when the list
    /// is narrowed — and the rows when there are any.
    #[test]
    fn an_empty_history_says_so_and_a_loading_one_waits() {
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};
        let mut host = CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        let feed = FeedView {
            rows: Vec::new(),
            ..host.view()
        };
        let (s, w) = (strings(), wallet_strings());

        let loading = history_panel(&feed, true, None, &s, &w, false);
        assert!(loading.loading && loading.empty.is_none());

        let empty = history_panel(&feed, false, None, &s, &w, false);
        assert!(!empty.loading);
        assert_eq!(empty.empty, Some(s.history_empty.clone()));

        let narrowed = history_panel(&feed, false, Some(100), &s, &w, false);
        assert_eq!(narrowed.empty, Some(s.history_empty_filter.clone()));
        assert_ne!(s.history_empty, s.history_empty_filter);
    }

    /// Day headers become headings, and a heading with nothing under it is not
    /// drawn.
    #[test]
    fn the_history_keeps_the_headers_the_home_preview_drops() {
        use vela_core::app::activity_feed::{
            ActivityFeed, Event as FeedEvent, FeedDirection, FeedItem,
        };

        let mut host = CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        let today = crate::executor::day_start_ms(crate::executor::now_ms());
        let item = |id: &str, day: f64| FeedItem {
            id: id.to_owned(),
            direction: FeedDirection::In,
            counterparty: Some("0xAbCdEf0000000000000000000000000000000001".to_owned()),
            alias: None,
            value: Some("1.5".to_owned()),
            symbol: "xDAI".to_owned(),
            decimals: Some(18),
            usd_value: 0.0,
            chain_id: 100,
            timestamp: day / 1000.0,
            day_start_ms: day,
            tx_hash: None,
            batch: None,
        };
        let view = FeedView {
            rows: vec![
                FeedRow::Header {
                    id: "day-0".to_owned(),
                    day_start_ms: today,
                    timestamp: today / 1000.0,
                },
                FeedRow::Item {
                    item: item("a", today),
                },
                FeedRow::Header {
                    id: "day-1".to_owned(),
                    day_start_ms: today - 86_400_000.0,
                    timestamp: (today - 86_400_000.0) / 1000.0,
                },
                FeedRow::Item {
                    item: item("b", today - 86_400_000.0),
                },
                // A heading the core emitted with nothing under it.
                FeedRow::Header {
                    id: "day-9".to_owned(),
                    day_start_ms: today - 9.0 * 86_400_000.0,
                    timestamp: (today - 9.0 * 86_400_000.0) / 1000.0,
                },
            ],
            ..host.view()
        };

        let s = strings();
        let groups = history(&view, &s, &wallet_strings(), false);
        assert_eq!(groups.len(), 2, "the empty heading is not drawn");
        assert_eq!(groups[0].label, s.today);
        assert_eq!(groups[1].label, s.yesterday);
        assert_eq!(groups[0].rows.len(), 1);
        assert_eq!(groups[0].rows[0].amount, "+1.5");

        // Hidden masks every figure here, as on the hero and the home preview.
        let hidden = history(&view, &s, &wallet_strings(), true);
        assert_eq!(hidden[0].rows[0].amount, crate::wallet::fixtures::MASK);
        assert_eq!(hidden[0].rows[0].unit, "xDAI");
    }
}

/// The speed control on the desktop (spec 069): every decision is the
/// `fee_speed` core's, driven here the way `SendHost` drives it; what these
/// pin is the half this file owns — which words and which figure each
/// decision becomes.
#[cfg(test)]
mod speed_tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::fee_policy::{FeePolicy, FeeTier};
    use vela_core::app::fee_speed::{Event as SpeedEvent, FeeSpeed, TierPreviewQuote, TierQuote};
    use vela_core::app::send::Send as SendMachine;
    use vela_core::l10n::number::NumberPreset;

    fn quote(tier: FeeTier, total_wei: &str, range: Option<(&str, &str)>) -> FeeEstimateView {
        FeeEstimateView {
            chain_id: 100,
            total_wei: total_wei.to_owned(),
            max_fee_per_gas: "0".to_owned(),
            network_fee_per_gas: "0".to_owned(),
            relayer_fee_per_gas: "0".to_owned(),
            bundler_gas_price: "0".to_owned(),
            in_band_gas_basis: "0".to_owned(),
            effective_gas_price: range.map(|(low, _)| low.to_owned()),
            max_gas_price: range.map(|(_, high)| high.to_owned()),
            total_gas: "0".to_owned(),
            deployed: true,
            tier,
            quoted: true,
            fee_asset: FeeAssetView::Native,
            fee_recipient: Some("0xfee".to_owned()),
        }
    }

    /// The core, configured, reported the given sessions — re-reported until
    /// the tier in force settles, the way the host's promotion leaves it.
    fn speed(
        preferred: FeeTier,
        on_form: bool,
        open: bool,
        pick: Option<FeeTier>,
        rows: &[FeeEstimateView],
    ) -> SpeedInputs {
        let mut core = CoreHost::<FeeSpeed>::new();
        let _ = core.dispatch(SpeedEvent::Configure {
            preferred,
            number: NumberPreset::CommaDot,
        });
        let _ = core.dispatch(SpeedEvent::StageChanged { on_form });
        if let Some(tier) = pick {
            let _ = core.dispatch(SpeedEvent::Pick { tier });
        }
        if open {
            let _ = core.dispatch(SpeedEvent::Toggle);
        }
        for _ in 0..3 {
            let tier = core.view().tier;
            let _ = core.dispatch(SpeedEvent::QuotesChanged {
                chain_id: Some(100),
                in_force: TierQuote {
                    busy: false,
                    fee: rows.iter().find(|row| row.tier == tier).cloned(),
                },
                previews: rows
                    .iter()
                    .filter(|row| row.tier != tier)
                    .map(|row| TierPreviewQuote {
                        tier: row.tier,
                        busy: false,
                        fee: Some(row.clone()),
                    })
                    .collect(),
            });
            if core.view().tier == tier {
                break;
            }
        }
        let fee_view = CoreHost::<FeePolicy>::new().view();
        SpeedInputs {
            view: core.view(),
            tier_views: [FeeTier::Fast, FeeTier::Standard, FeeTier::Slow]
                .into_iter()
                .map(|tier| (tier, fee_view.clone()))
                .collect(),
        }
    }

    fn inputs<'a>(
        send: &'a SendView,
        fee: &'a FeeView,
        s: &'a FlowStrings,
        wallet: &'a crate::wallet::WalletStrings,
        speed: Option<&'a SpeedInputs>,
    ) -> SendInputs<'a> {
        SendInputs {
            send,
            fee,
            s,
            wallet,
            locale: "en-US",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Speed",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed,
        }
    }

    fn strings() -> (FlowStrings, crate::wallet::WalletStrings) {
        let loc = crate::loc::Loc::from_env();
        (
            FlowStrings::resolve(&loc),
            crate::wallet::WalletStrings::resolve(&loc),
        )
    }

    /// Optimism-shaped: every tier the same fee, each its own gas bid.
    fn floor_clamped() -> Vec<FeeEstimateView> {
        vec![
            quote(FeeTier::Fast, "10000", Some(("3244", "9000"))),
            quote(FeeTier::Standard, "10000", Some(("2377", "6000"))),
            quote(FeeTier::Slow, "10000", Some(("1937", "4500"))),
        ]
    }

    /// Folded, the control names the tier in force — the stored default —
    /// and nothing else: no options, no note.
    #[test]
    fn folded_it_names_the_tier_in_force() {
        let (s, wallet) = strings();
        let send = CoreHost::<SendMachine>::new().view();
        let fee = CoreHost::<FeePolicy>::new().view();
        let speed = speed(FeeTier::Standard, false, false, None, &floor_clamped());
        let model = send_speed(&inputs(&send, &fee, &s, &wallet, Some(&speed)))
            .unwrap_or_else(|| unreachable!("a live control"));
        assert!(!model.open);
        assert_eq!(model.value, s.gas_tier_standard);
        assert!(model.free_note.is_none());
        let none = send_speed(&inputs(&send, &fee, &s, &wallet, None));
        assert!(none.is_none(), "no sessions behind it, no control");
    }

    /// Open, three options fastest first, each its own fee, its gas bid from
    /// the core, and the line on what it buys.
    #[test]
    fn open_every_option_shows_its_own_figures() {
        let (s, wallet) = strings();
        let send = CoreHost::<SendMachine>::new().view();
        let fee = CoreHost::<FeePolicy>::new().view();
        let speed = speed(FeeTier::Fast, false, true, None, &floor_clamped());
        let model = send_speed(&inputs(&send, &fee, &s, &wallet, Some(&speed)))
            .unwrap_or_else(|| unreachable!("a live control"));
        assert!(model.open);
        assert_eq!(
            model
                .options
                .iter()
                .map(|o| o.label.clone())
                .collect::<Vec<_>>(),
            vec![
                s.gas_tier_fast.clone(),
                s.gas_tier_standard.clone(),
                s.gas_tier_slow.clone()
            ]
        );
        assert_eq!(
            model
                .options
                .iter()
                .map(|o| o.gas_price.clone().map(|g| g.to_string()))
                .collect::<Vec<_>>(),
            vec![
                Some("3,244 ~ 9,000 wei".to_owned()),
                Some("2,377 ~ 6,000 wei".to_owned()),
                Some("1,937 ~ 4,500 wei".to_owned()),
            ]
        );
        assert_eq!(model.options[2].detail, s.gas_tier_hint_slow);
        assert!(model.options[0].selected);
        assert!(
            model
                .options
                .iter()
                .all(|o| o.value != "…" && o.value != "—")
        );
    }

    /// A slower default goes Fast where Fast costs no more — the control
    /// says why, and the confirm restates it with the reason.
    #[test]
    fn a_free_upgrade_is_said_on_the_form_and_the_confirm() {
        let (s, wallet) = strings();
        let send = CoreHost::<SendMachine>::new().view();
        let fee = CoreHost::<FeePolicy>::new().view();
        let speed = speed(FeeTier::Slow, true, false, None, &floor_clamped());
        let i = inputs(&send, &fee, &s, &wallet, Some(&speed));
        let model = send_speed(&i).unwrap_or_else(|| unreachable!("a live control"));
        assert_eq!(model.value, s.gas_tier_fast);
        assert_eq!(model.free_note.as_ref(), Some(&s.fee_speed_free));
        let confirm = send_confirm(&i);
        let row = confirm
            .facts
            .iter()
            .find(|fact| fact.label == s.fee_speed_label)
            .unwrap_or_else(|| unreachable!("the confirm names the speed"));
        assert_eq!(row.value, s.gas_tier_fast);
        assert_eq!(row.note.as_ref(), Some(&s.fee_speed_free));
    }

    /// A pick is restated without a reason; a send at the default adds no row.
    #[test]
    fn the_confirm_restates_a_pick_and_only_a_decision() {
        let (s, wallet) = strings();
        let send = CoreHost::<SendMachine>::new().view();
        let fee = CoreHost::<FeePolicy>::new().view();
        let picked = speed(
            FeeTier::Fast,
            true,
            false,
            Some(FeeTier::Slow),
            &floor_clamped(),
        );
        let confirm = send_confirm(&inputs(&send, &fee, &s, &wallet, Some(&picked)));
        let row = confirm
            .facts
            .iter()
            .find(|fact| fact.label == s.fee_speed_label)
            .unwrap_or_else(|| unreachable!("a pick is restated"));
        assert_eq!(row.value, s.gas_tier_slow);
        assert!(row.note.is_none());

        let untouched = speed(FeeTier::Fast, false, false, None, &floor_clamped());
        let confirm = send_confirm(&inputs(&send, &fee, &s, &wallet, Some(&untouched)));
        assert!(
            !confirm
                .facts
                .iter()
                .any(|fact| fact.label == s.fee_speed_label)
        );
    }

    /// Issue 681: the fee row never shows the speed just left under the new
    /// one's name, and "from a while ago" is a fact about a figure.
    #[test]
    fn the_fee_row_never_wears_another_tiers_figure() {
        let (s, wallet) = strings();
        let mut send = CoreHost::<SendMachine>::new().view();
        send.fee = Some(quote(FeeTier::Fast, "10000", None));
        let mut fee = CoreHost::<FeePolicy>::new().view();
        fee.stale = true;
        let slow = speed(FeeTier::Slow, false, false, None, &[]);
        let row = send_fee_row(&inputs(&send, &fee, &s, &wallet, Some(&slow)));
        assert_eq!(row.value, s.fee_pending);
        assert!(row.stale_note.is_none());
        assert_eq!(row.refresh.as_ref(), Some(&s.fee_refresh));

        // Its own tier's figure, old: the calm line says so.
        send.fee = Some(quote(FeeTier::Slow, "10000", None));
        let row = send_fee_row(&inputs(&send, &fee, &s, &wallet, Some(&slow)));
        assert_ne!(row.value, s.fee_pending);
        assert_eq!(row.stale_note.as_ref(), Some(&s.fee_stale));
    }
}

/// The web-parity pass (#196/#197/#203/#231/#265): each builder reads the
/// core's own answer and words it, as `live-send.ts` / `live-batch.ts` do.
#[cfg(test)]
mod parity_tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::batch_import::BatchPreviewRow;
    use vela_core::app::batch_import::{BatchImport, BatchParseError, BatchParseReason};
    use vela_core::app::send::{
        Send as SendMachine, SendDuplicateRowView, SendFeeIssueView, SendMultiSpecView,
        SendRowFieldState, SendSplitRowIssue,
    };

    fn strings() -> FlowStrings {
        FlowStrings::resolve(&crate::loc::Loc::from_env())
    }

    fn token(chain_id: u32, symbol: &str, address: Option<&str>, balance: &str) -> SendToken {
        SendToken {
            network: format!("chain-{chain_id}"),
            chain_id,
            symbol: symbol.to_owned(),
            balance: balance.to_owned(),
            decimals: 18,
            token_address: address.map(str::to_owned),
            price_usd: Some(2.0),
            logo_urls: Vec::new(),
            spam: false,
        }
    }

    fn draft(id: &str, address: &str, amount: &str) -> SendRecipientDraft {
        SendRecipientDraft {
            id: id.to_owned(),
            address: address.to_owned(),
            amount: amount.to_owned(),
            name: None,
        }
    }

    /// Run `f` with a form built from `view`.
    fn with_inputs<R>(view: &SendView, f: impl FnOnce(&SendInputs<'_>) -> R) -> R {
        let s = strings();
        let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        f(&SendInputs {
            send: view,
            fee: &fee,
            s: &s,
            wallet: &wallet,
            locale: "en-US",
            money: crate::wallet::live::Money::usd(),
            identity_name: "MultiTest",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
        })
    }

    /// #203 / #265: each split row says what the core says about it — the
    /// repeat names the row it repeats, a bad field is named, an empty one is
    /// not — and the total says what is left, or that it is over.
    #[test]
    fn a_split_row_says_why_and_the_total_says_what_is_left() {
        crate::executor::storage::tests::with_temp_state("parity-split", || {
            let s = strings();
            let view = SendView {
                split_mode: true,
                selected_token: Some(token(100, "xDAI", None, "10")),
                recipients: vec![
                    draft("rcpt_1", "0xaa", "1"),
                    draft("rcpt_2", "0xaa", "2"),
                    draft("rcpt_3", "0x12", "1,5"),
                    draft("rcpt_4", "", ""),
                ],
                split_duplicates: vec![SendDuplicateRowView {
                    id: "rcpt_2".to_owned(),
                    first_ordinal: 1,
                }],
                split_row_issues: vec![
                    SendSplitRowIssue {
                        id: "rcpt_3".to_owned(),
                        ordinal: 3,
                        address: SendRowFieldState::Invalid,
                        amount: SendRowFieldState::Invalid,
                    },
                    SendSplitRowIssue {
                        id: "rcpt_4".to_owned(),
                        ordinal: 4,
                        address: SendRowFieldState::Empty,
                        amount: SendRowFieldState::Empty,
                    },
                ],
                confirm_amount: String::new(),
                split_remaining: Some("7".to_owned()),
                ..CoreHost::<SendMachine>::new().view()
            };
            let form = with_inputs(&view, send_form);
            let notes: Vec<Vec<SharedString>> =
                form.recipients.iter().map(|r| r.notes.clone()).collect();
            assert!(
                notes[0].is_empty(),
                "the first occurrence is not the mistake"
            );
            assert_eq!(
                notes[1],
                vec![SharedString::from(fill(&s.recipient_duplicate, "n", "1"))]
            );
            assert_eq!(
                notes[2],
                vec![s.batch_bad_address.clone(), s.bad_amount.clone()]
            );
            assert!(notes[3].is_empty(), "an unfinished row is not wrong");

            let (_, total) = form
                .summary
                .clone()
                .unwrap_or_else(|| unreachable!("split"));
            assert_eq!(total, "—", "a sum the core cannot make is a dash");
            let left = form.remaining.clone().unwrap_or_default();
            assert!(left.contains("7 xDAI"), "{left}");
            assert!(!left.contains("{{"), "{left}");
            assert!(!form.summary_over);
            assert!(form.denom_toggle.is_none(), "a split has no single figure");

            // Over the balance: no "left", and the total in error ink.
            let over = SendView {
                split_remaining: None,
                split_over_balance: true,
                confirm_amount: "12".to_owned(),
                ..view
            };
            let form = with_inputs(&over, send_form);
            assert!(form.remaining.is_none() && form.summary_over);
        });
    }

    /// "Use X for the empty rows" (the web's `fillEmpty`): offered only while
    /// a row is empty and another carries a figure the core accepts; the
    /// figure is copied exactly as typed, into the empty rows only.
    #[test]
    fn a_split_offers_one_amount_for_the_empty_rows() {
        crate::executor::storage::tests::with_temp_state("parity-split-fill", || {
            let s = strings();
            let empty = |id: &str, ordinal: u32| SendSplitRowIssue {
                id: id.to_owned(),
                ordinal,
                address: SendRowFieldState::Ok,
                amount: SendRowFieldState::Empty,
            };
            let view = SendView {
                split_mode: true,
                selected_token: Some(token(100, "xDAI", None, "10")),
                recipients: vec![draft("rcpt_1", "0xaa", "0.50"), draft("rcpt_2", "0xbb", "")],
                split_row_issues: vec![empty("rcpt_2", 2)],
                ..CoreHost::<SendMachine>::new().view()
            };
            assert_eq!(split_fill_source(&view), Some("0.50"));
            let form = with_inputs(&view, send_form);
            assert_eq!(
                form.fill_empty.as_deref(),
                Some(fill(&s.split_fill_empty, "amount", "0.5 xDAI").as_str())
            );

            let filled = split_empty_filled(&view.recipients, "0.50");
            assert_eq!(
                filled[0], view.recipients[0],
                "a row with a figure keeps it"
            );
            assert_eq!(filled[1].amount, "0.50");
            assert_eq!(filled[1].id, "rcpt_2", "the row keeps its identity");

            // No empty row, nothing to fill.
            let none = SendView {
                split_row_issues: Vec::new(),
                ..view.clone()
            };
            assert!(with_inputs(&none, send_form).fill_empty.is_none());

            // A figure the core rejects is never the one that is copied.
            let rejected = SendView {
                recipients: vec![draft("rcpt_1", "0xaa", "1,5"), draft("rcpt_2", "0xbb", "")],
                split_row_issues: vec![
                    SendSplitRowIssue {
                        amount: SendRowFieldState::Invalid,
                        ..empty("rcpt_1", 1)
                    },
                    empty("rcpt_2", 2),
                ],
                ..view.clone()
            };
            assert!(split_fill_source(&rejected).is_none());

            // A single form never offers it.
            let single = SendView {
                split_mode: false,
                ..view
            };
            assert!(with_inputs(&single, send_form).fill_empty.is_none());
        });
    }

    /// A split whose coin also pays the fee: the core's ceiling (measured
    /// against the rows' total) comes first, then the over-balance sentence,
    /// then nothing — the order all four shells draw.
    #[test]
    fn a_split_says_the_same_asset_ceiling_first() {
        crate::executor::storage::tests::with_temp_state("parity-split-ceiling", || {
            let s = strings();
            let split = SendView {
                selected_token: Some(token(100, "USDC", Some("0xusdc"), "0.8")),
                split_mode: true,
                split_over_balance: true,
                // A stale single-form verdict a split must not say.
                amount_warning: Some(SendAmountWarning::NotEnoughToken {
                    symbol: "USDC".to_owned(),
                }),
                ..CoreHost::<SendMachine>::new().view()
            };
            let ceiling = SendView {
                same_asset_fee_issue: Some(SendFeeIssueView {
                    symbol: "USDC".to_owned(),
                    transfer_amount: "750000000000000000".to_owned(),
                    balance: "800000000000000000".to_owned(),
                    fee_amount: "100000000000000000".to_owned(),
                    total: "850000000000000000".to_owned(),
                    max_transfer_amount: "700000000000000000".to_owned(),
                }),
                ..split.clone()
            };
            let notice = with_inputs(&ceiling, |i| send_notice(i, false))
                .unwrap_or_else(|| unreachable!("the ceiling"));
            assert_eq!(
                notice.title.as_deref(),
                Some(fill(&s.same_fee_title, "symbol", "USDC").as_str())
            );
            assert!(notice.body.contains("0.75"), "{}", notice.body);
            assert!(notice.body.contains("0.85"), "{}", notice.body);
            assert!(!notice.body.contains("750000"), "{}", notice.body);
            let detail = notice.detail.clone().unwrap_or_default();
            assert!(detail.contains("0.7 USDC"), "{detail}");

            let over = with_inputs(&split, |i| send_notice(i, false))
                .unwrap_or_else(|| unreachable!("over the balance"));
            assert_eq!(over.body, s.insufficient_body);

            let quiet = SendView {
                split_over_balance: false,
                ..split
            };
            assert!(
                with_inputs(&quiet, |i| send_notice(i, false)).is_none(),
                "a split does not say the single form's warning"
            );
        });
    }

    /// #197 / #231: ⇄ exists only where the core offers it, is live only
    /// where it would change something, and the unit on the figure is the
    /// figure's OWN code — the line under it the other denomination.
    #[test]
    fn the_denomination_toggle_and_the_unit_are_the_cores() {
        crate::executor::storage::tests::with_temp_state("parity-denom", || {
            let base = SendView {
                selected_token: Some(token(100, "xDAI", None, "10")),
                amount: "4".to_owned(),
                token_amount: "4".to_owned(),
                ..CoreHost::<SendMachine>::new().view()
            };
            let form = with_inputs(&base, send_form);
            assert_eq!(form.denom_toggle, None, "not offered, not drawn");
            assert_eq!(form.amount_unit.as_deref(), Some("xDAI"));
            let (_, under) = form.amount.clone().unwrap_or_default();
            assert!(
                under.contains('$'),
                "the money under a token figure: {under}"
            );

            let fiat = SendView {
                amount_fiat_code: Some("EUR".to_owned()),
                token_amount: "2.5".to_owned(),
                denom_toggle_shown: true,
                denom_toggle_enabled: true,
                ..base.clone()
            };
            let form = with_inputs(&fiat, send_form);
            assert_eq!(form.denom_toggle, Some(true));
            assert_eq!(form.amount_unit.as_deref(), Some("EUR"));
            let (_, under) = form.amount.clone().unwrap_or_default();
            assert_eq!(under, "≈ 2.5 xDAI", "the token under a money figure");

            let refused = SendView {
                denom_toggle_shown: true,
                denom_toggle_enabled: false,
                ..base
            };
            let refused = SendView {
                denom_toggle_reason: Some(SendUnitIssue {
                    code: "CNY".to_owned(),
                    symbol: "xDAI".to_owned(),
                }),
                ..refused
            };
            let form = with_inputs(&refused, send_form);
            assert_eq!(form.denom_toggle, Some(false));
            let why = form.notice.map(|n| n.body).unwrap_or_default();
            assert!(why.contains("CNY") && !why.contains("{{"), "{why}");
            assert_ne!(
                why,
                cannot_convert(
                    &SendUnitIssue {
                        code: "CNY".to_owned(),
                        symbol: "xDAI".to_owned()
                    },
                    &strings()
                ),
                "the ⇄ row's own sentence, not the amount warning's"
            );
        });
    }

    /// SD3c: a sweep's confirm lists every coin at the amount the signature
    /// moves — the core's reserved spec, else the full balance.
    #[test]
    fn a_sweep_confirm_lists_each_coins_amount() {
        crate::executor::storage::tests::with_temp_state("parity-sweep", || {
            let s = strings();
            let tokens = vec![
                token(100, "xDAI", None, "10"),
                token(100, "USDC", Some("0xdd"), "5"),
                token(1, "ETH", None, "1"),
            ];
            let view = SendView {
                multi_select_mode: true,
                multi_chain_id: Some(100),
                multi_selected_ids: vec![tokens[0].id(), tokens[1].id()],
                multi_specs: vec![SendMultiSpecView {
                    token_address: None,
                    decimals: 18,
                    amount: "9.5".to_owned(),
                }],
                tokens,
                ..CoreHost::<SendMachine>::new().view()
            };
            let confirm = with_inputs(&view, send_confirm);
            assert_eq!(confirm.amount, fill(&s.assets_count, "n", "2"));
            let rows: Vec<(String, String)> = confirm
                .breakdown
                .iter()
                .map(|row| (row.label.to_string(), row.value.to_string()))
                .collect();
            assert_eq!(rows.len(), 2, "only the picked coins");
            assert_eq!(rows[0].0, "xDAI");
            assert!(
                rows[0].1.starts_with("9.5 xDAI"),
                "the reserved spec: {}",
                rows[0].1
            );
            assert!(
                rows[1].1.starts_with("5 USDC"),
                "the full balance: {}",
                rows[1].1
            );
            assert!(rows[1].1.contains("$10"), "priced: {}", rows[1].1);
            assert!(confirm.subline.contains("Gnosis"), "{}", confirm.subline);
            assert!(
                confirm.subline.contains("$29"),
                "9.5×2 + 5×2: {}",
                confirm.subline
            );
            assert!(confirm.mark.is_none(), "one mark would name the wrong coin");
        });
    }

    fn preview(line: u32, address: &str, raw: &str, token: &str) -> BatchPreviewRow {
        BatchPreviewRow {
            line,
            name: None,
            address: address.to_owned(),
            valid: true,
            dup: false,
            raw_amount: raw.to_owned(),
            token_amount: token.to_owned(),
            ok: true,
        }
    }

    /// The importer's list in sheet order, every unpaid line saying why, a
    /// fiat sheet's own figure beside what it became, and a total measured
    /// against the balance — or against what the form has left.
    #[test]
    fn the_batch_preview_says_why_and_totals_against_the_balance() {
        let s = strings();
        let base = CoreHost::<BatchImport>::new().view();
        let view = BatchView {
            unit: BatchUnit::Fiat,
            fiat_code: "CNY".to_owned(),
            preview: vec![
                preview(1, "0xaa", "5000", "689.66"),
                BatchPreviewRow {
                    dup: true,
                    ok: false,
                    ..preview(3, "0xaa", "10", "1.38")
                },
                BatchPreviewRow {
                    valid: false,
                    ok: false,
                    ..preview(4, "0x12", "10", "1.38")
                },
            ],
            errors: vec![BatchParseError {
                line: 2,
                raw: "bob, lots".to_owned(),
                reason: BatchParseReason::NoAmount,
            }],
            recipient_count: 1,
            total_token: "689.66".to_owned(),
            total_fiat: Some("5000".to_owned()),
            ..base
        };
        let model = batch_import(&view, "USDT", &s);
        let lines: Vec<(&str, Option<&SharedString>)> = model
            .rows
            .iter()
            .map(|row| (row.address.as_ref(), row.note.as_ref()))
            .collect();
        assert_eq!(
            lines,
            vec![
                ("0xaa", None),
                ("bob, lots", Some(&s.bad_amount)),
                ("0xaa", Some(&s.batch_dup)),
                ("0x12", Some(&s.batch_bad_address)),
            ],
            "sheet order, each skipped line with its reason"
        );
        assert_eq!(model.rows[0].conversion, "5000 CNY → 689.66 USDT");
        assert_eq!(model.parsed, fill(&s.batch_parsed, "n", "4"), "lines read");
        assert!(
            model.rows[1].conversion.is_empty(),
            "a refused line has no figure"
        );

        let total = batch_total(&view, "USDT", "1000", None, &s)
            .unwrap_or_else(|| unreachable!("one row parsed"));
        assert_eq!(total.value, "689.66 USDT");
        // One row is "1 recipient", not "1 recipients" (#288).
        assert_eq!(
            total.label.as_ref(),
            format!(
                "{} · {}",
                s.split_total,
                fill(&s.recipient_count_one, "count", "1")
            )
        );
        assert_eq!(total.detail.as_deref(), Some("5000 CNY"));
        assert_eq!(
            total.balance,
            fill(
                &s.balance_label,
                "amount",
                &format!("{} USDT", trimmed_str("1000"))
            )
        );
        assert!(total.over.is_none());

        // Adding to rows already on the form: measured against what is left.
        let adding = batch_total(&view, "USDT", "1000", Some("700"), &s)
            .unwrap_or_else(|| unreachable!("one row parsed"));
        assert_eq!(
            adding.balance,
            fill(
                &s.split_remaining,
                "amount",
                &format!("{} USDT", trimmed_str("700"))
            )
        );

        // Over the balance: said on the total, with the symbol filled in.
        let over = BatchView {
            over_balance: true,
            ..view.clone()
        };
        let total = batch_total(&over, "USDT", "10", None, &s)
            .unwrap_or_else(|| unreachable!("one row parsed"));
        let text = total.over.unwrap_or_default();
        assert!(text.contains("USDT") && !text.contains("{{"), "{text}");
        assert!(
            batch_import(&over, "USDT", &s).notice.is_none(),
            "not said twice"
        );

        // Nothing parsed: no total line at all.
        let empty = BatchView {
            recipient_count: 0,
            ..view
        };
        assert!(batch_total(&empty, "USDT", "1000", None, &s).is_none());
    }

    /// The total is read against what is left only when the import ADDS to
    /// rows already on the form — an empty form, or a replace, draws from the
    /// whole balance (#288).
    #[test]
    fn an_import_that_adds_reads_against_what_is_left() {
        let blank = CoreHost::<SendMachine>::new().view();
        let with_rows = SendView {
            split_import_room: 57,
            split_remaining: Some("7".to_owned()),
            ..blank.clone()
        };
        assert_eq!(batch_remaining(&with_rows, false), Some("7"));
        assert_eq!(batch_remaining(&with_rows, true), None, "a replace");
        let empty_form = SendView {
            split_import_room: u32::try_from(vela_core::app::send::BATCH_MAX_RECIPIENTS)
                .unwrap_or(u32::MAX),
            split_remaining: Some("7".to_owned()),
            ..blank
        };
        assert_eq!(
            batch_remaining(&empty_form, false),
            None,
            "nothing to add to"
        );
    }

    /// "View on Explorer" leads to this account on the network's explorer —
    /// the built-in's, a custom network's own, else Etherscan.
    #[test]
    fn the_explorer_link_is_the_networks_own() {
        crate::executor::storage::tests::with_temp_state("parity-explorer", || {
            assert_eq!(explorer_root(100), "https://gnosisscan.io");
            assert_eq!(explorer_root(424_242), "https://etherscan.io");
            assert!(
                crate::executor::storage::write_value(
                    crate::executor::storage::KEY_CUSTOM_NETWORKS,
                    serde_json::json!([{ "chainId": 424_242, "explorerURL": "https://scan.example/" }]),
                )
                .is_ok()
            );
            assert_eq!(explorer_root(424_242), "https://scan.example");

            let s = strings();
            let watch = CoreHost::<vela_core::app::receive_watch::ReceiveWatch>::new().view();
            let pay = CoreHost::<vela_core::app::payment_request::PaymentRequest>::new().view();
            let qr = receive_qr(
                "0xabc",
                "Golden",
                100,
                &watch,
                &pay,
                &s,
                "en",
                crate::wallet::live::Money::usd(),
            );
            assert_eq!(
                qr.explorer_url.as_deref(),
                Some("https://gnosisscan.io/address/0xabc")
            );
            assert!(qr.contract_copy.is_none(), "a network code has no contract");
        });
    }
}
