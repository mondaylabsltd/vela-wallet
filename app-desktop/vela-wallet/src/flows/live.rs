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
use vela_core::app::activity_feed::{
    FeedDapp, FeedDappContent, FeedDappOperation, FeedFact, FeedItem, FeedRow, FeedTxStatus,
    FeedView,
};
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
use vela_core::app::batch_import::{BatchFileFailure, BatchRateStatus, BatchUnit, BatchView};
use vela_core::app::contacts::ContactsView;
use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView, FeeTier, FeeView};
use vela_core::app::fee_speed::FeeSpeedView;
use vela_core::app::send::{
    SendAddNetworkMsg, SendAmountWarning, SendHoldReason, SendLockError, SendNameSource, SendPayee,
    SendReceiptCoin, SendReceiptStatus, SendRecipientDraft, SendStage, SendToken, SendTxStatus,
    SendUnitIssue, SendView,
};

use crate::flows::fixtures::{
    AddressCard, AssetsEmpty, AssetsPanel, BatchImport, BatchRow, BreakdownRow, ContactPick,
    CtaState, DepositEntry as FlowDeposit, FactLead, FactRow, FeeRow, FeeSpeedModel,
    FeeSpeedOption, FeeTokenPick, FeeTokenRow, FilterChip, HistoryGroup, HistoryPanel, NetworkRow,
    NetworkSwitch, ReceiptStage, ReceiveGate, ReceiveList, ReceiveQr, RecipientCard, SendConfirm,
    SendForm, SendNotice, SendPick, SendReceipt, StatusChip, StatusTone, SweepForm, SweepRow,
    TokenMark, address_lines,
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
        // No filter row (078 T067): the web's Assets screen has none, and
        // this one's pill and "Add" answered no click. Which chain the list
        // is narrowed to is the sidebar's selected network.
        filter: None,
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

// ---------------------------------------------------------------------------
// Activity — DA1L
// ---------------------------------------------------------------------------

/// DA1L's three modes — the web's `liveHistory`.
///
/// Rows when there are any; skeletons while the balance core has not ruled
/// (the feed has nothing yet, and "no transactions" would be a guess); and
/// once it has, one line — the core's `history_empty_key` (spec 082 RG5):
/// about THIS network when the feed is narrowed to one, because "no
/// transactions" under a filter would read as "none at all". Which line is
/// the core's; loading versus empty stays here.
#[must_use]
pub fn history_panel(
    view: &FeedView,
    balance_unknown: bool,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
) -> HistoryPanel {
    let groups = history(view, s, wallet, hidden);
    let bare = groups.is_empty();
    HistoryPanel {
        groups,
        loading: bare && balance_unknown,
        empty: (bare && !balance_unknown).then(|| s.history_empty_of(&view.history_empty_key)),
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
                let model = crate::wallet::live::activity_row(item, wallet, hidden);
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
    day_word(day_start_ms, &s.today, &s.yesterday)
}

/// [`day_label`] with its two words given — for a surface that holds the
/// wallet's strings rather than the flows' (a contact's rows say their day on
/// their second line, `FeedLine::Day`). The same corpus words, the same rule.
pub(crate) fn day_word(
    day_start_ms: f64,
    today: &SharedString,
    yesterday: &SharedString,
) -> SharedString {
    let now = crate::executor::day_start_ms(crate::executor::now_ms());
    const DAY_MS: f64 = 86_400_000.0;
    if (day_start_ms - now).abs() < DAY_MS / 2.0 {
        return today.clone();
    }
    if (day_start_ms - (now - DAY_MS)).abs() < DAY_MS / 2.0 {
        return yesterday.clone();
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
///
/// A dApp's row (a transaction or a signature) is [`dapp_detail`]'s: since
/// spec 093 its facts are the core's, not this function's.
#[must_use]
pub fn tx_detail(
    view: &FeedView,
    id: &str,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> Option<crate::flows::fixtures::TxDetail> {
    let item = feed_item(view, id)?;
    if let Some(dapp) = item.dapp.as_ref() {
        return Some(dapp_detail(item, dapp, s, wallet, hidden, locale, currency));
    }
    let incoming = item.direction == vela_core::app::activity_feed::FeedDirection::In;

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
            danger: false,
            detail: None,
        });
    }
    facts.push(network_fact(item.chain_id, s));
    facts.push(FactRow {
        label: s.detail_date.clone(),
        value: SharedString::from(stamp(item.timestamp, s, locale)),
        lead: FactLead::None,
        mono: false,
        copy: None,
        note: None,
        danger: false,
        detail: None,
    });
    // Only if there IS one. An empty hash row invites "which transaction?" —
    // the same reason the mock omits the contract row on a native transfer.
    if let Some(hash) = item.tx_hash.as_ref().filter(|hash| !hash.is_empty()) {
        facts.push(hash_fact(&s.detail_hash, hash));
    }

    let (breakdown_title, breakdown) = detail_parts(item, s);
    Some(crate::flows::fixtures::TxDetail {
        breakdown_title,
        breakdown,
        title: match item.kind {
            vela_core::app::activity_feed::FeedTxKind::DappTx => s.tx_label_dapp.clone(),
            _ => SharedString::from(crate::wallet::fill(
                if incoming {
                    &s.tx_label_received
                } else {
                    &s.tx_label_sent
                },
                "symbol",
                &item.symbol,
            )),
        },
        status: Some(status_chip(item.status, s)),
        note: None,
        amount: crate::wallet::live::amount_text_of(item, incoming, hidden),
        fiat: fiat_of(item, hidden, locale, currency),
        positive: incoming,
        danger: false,
        facts,
        technical: None,
        view_on_explorer: s.view_on_explorer.clone(),
        explorer_url: explorer_tx_url(item),
        delete_label: Some(s.delete_record.clone()),
    })
}

/// The feed item `id` names, if it is still there.
fn feed_item<'a>(view: &'a FeedView, id: &str) -> Option<&'a FeedItem> {
    view.rows.iter().find_map(|row| match row {
        FeedRow::Item { item } if item.id == id => Some(item),
        _ => None,
    })
}

/// A dApp interaction in detail (spec 093) — its header as its row says it
/// (title, and the money it moved or the allowance it granted), a chip for a
/// transaction or the off-chain note for a signature, then the core's facts
/// in the core's order, each labelled by the key its kind names, and the
/// collapsed "Technical details" the core listed. Nothing here decides which
/// facts there are.
fn dapp_detail(
    item: &FeedItem,
    dapp: &FeedDapp,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> crate::flows::fixtures::TxDetail {
    let (amount, fiat, danger, positive) = match (dapp.allowance.as_ref(), dapp.received.as_ref()) {
        (Some(allowance), _) => (
            crate::wallet::live::allowance_text(allowance, wallet, hidden),
            SharedString::default(),
            allowance.unlimited,
            false,
        ),
        // Spec 097 N5: nothing left and something came back (a borrow) —
        // what came back is the figure.
        (None, Some(back)) if item.value.is_none() => (
            SharedString::from(
                format!(
                    "{} {}",
                    crate::wallet::live::change_figure(back, hidden),
                    back.symbol
                )
                .trim()
                .to_owned(),
            ),
            SharedString::default(),
            false,
            back.direction == vela_core::app::activity_feed::FeedDirection::In,
        ),
        (None, _) => (
            crate::wallet::live::amount_text_of(item, false, hidden),
            fiat_of(item, hidden, locale, currency),
            false,
            false,
        ),
    };
    crate::flows::fixtures::TxDetail {
        title: crate::wallet::live::dapp_title(dapp, wallet),
        status: (!dapp.off_chain).then(|| status_chip(item.status, s)),
        // A signature says it was off-chain; a failed operation says why
        // (spec 097 N4), under its chip.
        note: if dapp.off_chain {
            Some(s.detail_off_chain.clone())
        } else {
            dapp.failure.map(|failure| failure_text(failure, s))
        },
        breakdown_title: None,
        breakdown: Vec::new(),
        amount,
        fiat,
        positive,
        danger,
        facts: dapp
            .facts
            .iter()
            .map(|fact| dapp_fact(fact, dapp, s, wallet, hidden, locale))
            .collect(),
        technical: (!dapp.technical.is_empty()).then(|| crate::flows::fixtures::Technical {
            toggle: s.detail_technical.clone(),
            lines: None,
        }),
        view_on_explorer: s.view_on_explorer.clone(),
        explorer_url: explorer_tx_url(item),
        delete_label: Some(s.delete_record.clone()),
    }
}

/// One of the core's facts as a row (spec 093): its label, and its value in
/// the reader's words and format.
fn dapp_fact(
    fact: &FeedFact,
    dapp: &FeedDapp,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    hidden: bool,
    locale: &str,
) -> FactRow {
    let plain = |label: &SharedString, value: SharedString| FactRow {
        label: label.clone(),
        value,
        lead: FactLead::None,
        mono: false,
        copy: None,
        note: None,
        danger: false,
        detail: None,
    };
    match fact {
        FeedFact::Site { site } => plain(&s.detail_app, SharedString::from(site.clone())),
        FeedFact::Network { chain_id } => network_fact(*chain_id, s),
        // Who got the money (spec 082 RJ16): a plain send's recipient, or the
        // one a token transfer names — never the token contract it was called
        // on. The core names them as the row does.
        FeedFact::Recipient { address, name } => party_fact(&s.detail_to, address, name.as_deref()),
        // A call that paid nobody: the contract it went to (083 F3).
        FeedFact::Contract { address, name } => {
            party_fact(&s.detail_contract, address, name.as_deref())
        }
        FeedFact::Spender { address, name } => {
            party_fact(&s.detail_spender, address, name.as_deref())
        }
        FeedFact::SpendingCap { allowance } => FactRow {
            danger: allowance.unlimited,
            ..plain(
                &s.detail_spending_cap,
                crate::wallet::live::allowance_text(allowance, wallet, hidden),
            )
        },
        FeedFact::Expires { at } => plain(
            &s.detail_expires,
            at.map_or_else(
                || s.detail_no_expiry.clone(),
                |at| SharedString::from(moment(at, locale)),
            ),
        ),
        // The sheet's "Balance changes" lines, one per line, as the row's
        // figure draws them ("≈ −100 USDC").
        FeedFact::BalanceChanges => plain(
            &s.detail_changes,
            SharedString::from(
                dapp.changes
                    .iter()
                    .map(|change| {
                        let figure = crate::wallet::live::change_figure(change, hidden);
                        let name = if !change.verified {
                            s.detail_unverified_token.to_string()
                        } else if change.symbol.is_empty() {
                            // A native coin the chain table does not name: the
                            // sheet's own placeholder, not a guessed ticker.
                            "—".to_owned()
                        } else {
                            change.symbol.clone()
                        };
                        format!("{figure} {name}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n"),
            ),
        ),
        FeedFact::Date { timestamp } => plain(
            &s.detail_date,
            SharedString::from(stamp(*timestamp, s, locale)),
        ),
        // The technical section's lines (`technical_lines`).
        FeedFact::Operation { operation } => plain(
            &s.detail_operation,
            match operation {
                FeedDappOperation::ContractInteraction => s.op_contract_interaction.clone(),
                FeedDappOperation::Batch { calls } => {
                    SharedString::from(fill(&s.op_batch, "count", &calls.to_string()))
                }
                FeedDappOperation::Signature => s.op_signature.clone(),
                FeedDappOperation::TypedDataSignature => s.op_typed_data.clone(),
            },
        ),
        // Its text is the stored request, which only `technical_lines` reads.
        FeedFact::Content { content } => {
            plain(&content_label(*content, s), SharedString::default())
        }
        FeedFact::PrimaryType { name } => plain(&s.detail_type, SharedString::from(name.clone())),
        FeedFact::Hash { tx_hash } => hash_fact(&s.detail_hash, tx_hash),
        FeedFact::UserOpHash { hash } => hash_fact(&s.detail_user_op_hash, hash),
    }
}

/// What a stored request holds, as its label names it.
fn content_label(content: FeedDappContent, s: &FlowStrings) -> SharedString {
    match content {
        FeedDappContent::CallData => s.content_call_data.clone(),
        FeedDappContent::TypedData => s.content_typed_data.clone(),
        FeedDappContent::Message => s.content_message.clone(),
    }
}

/// A recipient, a contract or a spender: its name when the core gives one
/// (the row's for a recipient, the built-in one for a contract), else its
/// short address in the mono face — copyable either way, beside the
/// identicon seeded by the address.
fn party_fact(label: &SharedString, address: &str, name: Option<&str>) -> FactRow {
    FactRow {
        label: label.clone(),
        value: SharedString::from(name.map_or_else(
            || crate::wallet::live::shorten_address(address),
            str::to_owned,
        )),
        lead: FactLead::Identicon(SharedString::from(address.to_owned())),
        mono: name.is_none(),
        copy: Some(SharedString::from(address.to_owned())),
        note: None,
        danger: false,
        detail: None,
    }
}

/// A hash, shortened like every address on this panel with the whole of it
/// on the copy button (083 F2): 66 characters on one line ran off the
/// column's right edge.
fn hash_fact(label: &SharedString, hash: &str) -> FactRow {
    FactRow {
        label: label.clone(),
        value: SharedString::from(crate::wallet::live::shorten_address(hash)),
        lead: FactLead::None,
        mono: true,
        copy: Some(SharedString::from(hash.to_owned())),
        note: None,
        danger: false,
        detail: None,
    }
}

/// The network a record is on, wearing the NETWORK's mark: its own logo over
/// its coin's letters, never a badge (the core's kind rule). It used to wear
/// the record's coin, and the native coin's home-chain rule put Ethereum's
/// logo beside "Base" on every ETH sent there; the coin is the amount's, not
/// the network's.
fn network_fact(chain_id: u32, s: &FlowStrings) -> FactRow {
    FactRow {
        label: s.detail_chain.clone(),
        value: SharedString::from(chain_name(chain_id)),
        lead: FactLead::Token(network_mark(chain_id)),
        mono: false,
        copy: None,
        note: None,
        danger: false,
        detail: None,
    }
}

/// Where it stands is the core's (spec 082 RG1): the row's status, which
/// only the tracker moves off pending.
fn status_chip(status: FeedTxStatus, s: &FlowStrings) -> StatusChip {
    StatusChip {
        text: match status {
            FeedTxStatus::Confirmed => s.status_confirmed.clone(),
            // A pending or failed transfer must not wear the confirmed chip.
            // The words are the feed's, which already has them.
            FeedTxStatus::Pending => s.status_pending.clone(),
            FeedTxStatus::Failed => s.status_failed.clone(),
            // 087 F04: pending, and nothing will settle it — not failed.
            FeedTxStatus::Unknown => s.status_unknown.clone(),
        },
        // Info keeps the delete quiet (`panels::delete_style`): a record
        // nothing settles may have been sent, like a pending one.
        tone: match status {
            FeedTxStatus::Confirmed => StatusTone::Success,
            FeedTxStatus::Pending | FeedTxStatus::Unknown => StatusTone::Info,
            FeedTxStatus::Failed => StatusTone::Error,
        },
    }
}

/// Why a dApp operation failed, in the words its request ended with (spec
/// 097 N4) — the core's reason, worded.
fn failure_text(
    failure: vela_core::app::tx_tracker::TrackFailure,
    s: &FlowStrings,
) -> SharedString {
    use vela_core::app::tx_tracker::TrackFailure;
    match failure {
        TrackFailure::Reverted => s.failed_reverted.clone(),
        TrackFailure::Refused => s.failed_refused.clone(),
        TrackFailure::NotSent => s.failed_not_sent.clone(),
    }
}

/// The core already valued it, stablecoin fallback and all — and says when
/// it knows no price (`priced`, spec 097 N7): unknown rather than free, so it
/// shows nothing instead of `$0.00`.
fn fiat_of(
    item: &FeedItem,
    hidden: bool,
    locale: &str,
    currency: &crate::wallet::live::Money,
) -> SharedString {
    if hidden || !item.priced {
        SharedString::from("")
    } else {
        SharedString::from(currency.text(item.usd_value, locale))
    }
}

/// The transaction's own page, when it has a hash to find it by (the web's
/// `explorerTxURL`). An off-chain signature has nothing to open.
fn explorer_tx_url(item: &FeedItem) -> Option<SharedString> {
    item.tx_hash
        .as_ref()
        .filter(|hash| !hash.is_empty())
        .map(|hash| SharedString::from(format!("{}/tx/{hash}", explorer_root(item.chain_id))))
}

/// A dApp record's "Technical details", opened (spec 093): the core's lines
/// in its order. `request` is the record's stored request, which the page
/// read from the store when the section was opened — `None` when the record
/// kept none, which the content line then says.
#[must_use]
pub fn technical_lines(
    item: &FeedItem,
    request: Option<&str>,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    locale: &str,
) -> Vec<crate::flows::fixtures::TechnicalLine> {
    use crate::flows::fixtures::TechnicalLine;
    let Some(dapp) = item.dapp.as_ref() else {
        return Vec::new();
    };
    dapp.technical
        .iter()
        .map(|fact| match fact {
            // The core's text for what the record kept (typed data as its
            // document, a message as its words, call data as its params);
            // nothing kept says so.
            FeedFact::Content { content } => {
                let text = request.and_then(|request| {
                    vela_core::app::dapp_activity::request_display(*content, request)
                });
                TechnicalLine::Text {
                    label: content_label(*content, s),
                    missing: text.is_none(),
                    text: text.map_or_else(|| s.content_missing.clone(), SharedString::from),
                }
            }
            // Nothing here is a balance: hashes and names, never masked.
            other => TechnicalLine::Fact(dapp_fact(other, dapp, s, wallet, false, locale)),
        })
        .collect()
}

/// Opens a dApp record's "Technical details" on a detail already built for
/// it (spec 093) — the page calls this only while the section is open, with
/// the request it read from the store then. Nothing for any other record.
pub fn open_technical(
    detail: &mut crate::flows::fixtures::TxDetail,
    view: &FeedView,
    id: &str,
    request: Option<&str>,
    s: &FlowStrings,
    wallet: &crate::wallet::WalletStrings,
    locale: &str,
) {
    let (Some(technical), Some(item)) = (detail.technical.as_mut(), feed_item(view, id)) else {
        return;
    };
    technical.lines = Some(technical_lines(item, request, s, wallet, locale));
}

/// The "Technical details" a tap leaves (spec 093): folded when they were
/// open on `id`; else open on `id`, with its stored request read through
/// `read` — on this tap, once, and never before one.
pub fn technical_toggled(
    open: Option<(String, Option<String>)>,
    id: String,
    read: impl FnOnce(&str) -> Option<String>,
) -> Option<(String, Option<String>)> {
    match open {
        Some((current, _)) if current == id => None,
        _ => {
            let request = read(&id);
            Some((id, request))
        }
    }
}

/// A moment as the detail states it: its date and its time on the reader's
/// clock — a permit's expiry, not a transaction's "Today 11:20".
fn moment(epoch_sec: f64, locale: &str) -> String {
    let civil = crate::executor::local_civil(epoch_sec * 1000.0);
    let prefs = crate::executor::format_prefs::current();
    format!(
        "{} {}",
        vela_core::l10n::datetime::format_date(&civil, prefs.date),
        format_time(&civil, prefs.time, locale)
    )
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

    // A network being added is drawn as itself (the core's kind rule): its
    // own logo, no badge. The native coin's rule put Ethereum's logo on every
    // ETH L2 the wizard found. Chain 0 — nothing resolved yet — asks for none.
    let mark_of = |chain_id: u32, symbol: &str| TokenMark {
        ticker: SharedString::from(symbol.to_owned()),
        badge: tint(chain_id),
        logos: crate::marks::chain_logos(chain_id),
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
                danger: false,
                detail: None,
            },
            FactRow {
                label: s.label_native_token.clone(),
                value: SharedString::from(symbol.to_owned()),
                lead: FactLead::None,
                mono: false,
                copy: None,
                note: None,
                danger: false,
                detail: None,
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
            // The chain's own logo — the one the sidebar's network filter and
            // this network's QR centre wear. The row drew only the lettermark,
            // so twenty-four networks read as a column of "ETH" circles.
            logos: crate::marks::chain_logos(chain_id),
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
        // Spec 090: the switch and its hint are the core's — whether it is
        // offered, where it sits, and when the hint shows. The code above
        // already says what the switch chose (`qr_value`).
        network: pay.network_switch.then(|| NetworkSwitch {
            label: s.include_network.clone(),
            on: pay.include_network,
            hint: pay.network_hint.then(|| s.include_network_hint.clone()),
        }),
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

/// The wallet's one shortening — by character, never by byte (083 H2
/// review).
fn shorten(address: &str) -> String {
    crate::wallet::live::shorten_address(address)
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
    /// When the relay put the send on the network, as the tracker learned it
    /// (spec 099 R6) — the landing's countdown starts there, never before.
    pub relay_sent_at_ms: Option<f64>,
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

/// A network's own coin: the built-in's, or the one the person gave a network
/// they added (`receivable_chains` reads the same list). Empty only for a
/// chain the wallet does not know.
pub(crate) fn native_symbol(chain_id: u32) -> String {
    if let Some(chain) = BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
    {
        return chain.native_symbol.to_owned();
    }
    receivable_chains()
        .into_iter()
        .find(|(id, _)| *id == chain_id)
        .map_or_else(String::new, |(_, symbol)| symbol)
}

/// A NETWORK drawn as itself — a network row or fact, a notice that locks a
/// chain, the add-network wizard: its own logo over its coin's letters, and
/// never a badge (the core's `chain_mark`).
pub(crate) fn network_mark(chain_id: u32) -> TokenMark {
    TokenMark {
        ticker: SharedString::from(native_symbol(chain_id)),
        badge: tint(chain_id),
        logos: crate::marks::chain_logos(chain_id),
    }
}

/// A token figure held as a float (a fee in its coin, a parsed balance), on
/// the one token-amount rule ([`crate::wallet::live::token_amount_text`]).
/// Rust prints a float's shortest round-trip digits without an exponent, so
/// the rule reads the same figure a string would have given it.
fn trimmed(amount: f64) -> String {
    if !amount.is_finite() {
        return "0".to_owned();
    }
    crate::wallet::live::token_amount_text(&amount.to_string())
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

/// The coin the fee row names, for its mark: its symbol, its contract
/// (`None` for the chain's own coin) and the chain it is paid on.
///
/// Symbol and contract come from the SAME estimate, the one in hand. The
/// contract used to come from the estimate of the speed showing, which is
/// none while a newly picked speed is measured — so for that moment a USDC fee
/// was "USDC" with no contract, and the native coin's rule drew the chain's
/// logo in its place. The coin does not change with the speed.
fn fee_coin(send: &SendView, fee: &FeeView) -> (String, Option<String>, u32) {
    let quote = send.fee.as_ref().or(fee.fee.as_ref());
    let chain_id = send
        .selected_token
        .as_ref()
        .map(|token| token.chain_id)
        .or_else(|| quote.map(|quote| quote.chain_id))
        .unwrap_or(1);
    match quote.map(|quote| &quote.fee_asset) {
        Some(FeeAssetView::Erc20 { symbol, token, .. }) => (
            symbol.clone().unwrap_or_else(|| native_symbol(chain_id)),
            Some(token.clone()),
            chain_id,
        ),
        _ => (native_symbol(chain_id), None, chain_id),
    }
}

fn send_fee_row(i: &SendInputs<'_>) -> FeeRow {
    let (symbol, contract, chain_id) = fee_coin(i.send, i.fee);
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
            logos: crate::marks::token_logos(chain_id, &symbol, contract.as_deref(), &[]),
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
    // Issue #312: a code that named a network decides which network is on
    // screen — the core lists only its holdings, and a sidebar filter left on
    // another network must not hide them all.
    let chain = view.request_chain_id.or(chain);
    view.tokens.retain(|token| {
        chain.is_none_or(|chain| token.chain_id == chain)
            && (class == SendClass::All || SendClass::of(token) == class)
    });
}

/// A line about one network, above the picker's rows: `template`'s
/// `{{network}}` filled with its name, and the chain's mark beside it.
fn chain_notice(chain_id: u32, template: &str) -> (u32, u32, SharedString, SharedString) {
    let name = crate::executor::custom_tokens::network_name(chain_id);
    (
        chain_id,
        crate::settings::model::chain_tint(u64::from(chain_id)).unwrap_or(0x8A_8F_98),
        SharedString::from(crate::settings::model::lettermark(&name)),
        SharedString::from(crate::wallet::fill(template, "network", &name)),
    )
}

/// Issue #332: the picker's "To" line — the recipient the core already holds,
/// worded as the confirm page words it (`payee_fact`), so the person sees
/// whom they are paying while they choose what. Nobody held, no line;
/// artwork only for a real address.
fn pick_recipient(i: &SendInputs<'_>) -> Option<FactRow> {
    single_payee(i.send).map(|payee| payee_fact(&i.s.to_label, &payee, i.s))
}

/// The one recipient of a single send or a sweep, as the core names them
/// (`payees[0]`, spec 097 F) — or, before the core holds a whole address,
/// what was typed, unnamed. `None` while nothing is typed, and on a split,
/// which has no one recipient.
fn single_payee(send: &SendView) -> Option<SendPayee> {
    let address = send.recipient.trim();
    if send.split_mode || address.is_empty() {
        return None;
    }
    Some(send.payees.first().cloned().unwrap_or_else(|| SendPayee {
        address: address.to_owned(),
        name: None,
        name_source: None,
    }))
}

/// The tag beside a payee's name — whose word the name is, as the core
/// decided (spec 097 F, S2): the public registry's says "Vela User", a name
/// service's its own label ("ENS"), the person's own word nothing.
fn payee_tag(payee: &SendPayee, s: &FlowStrings) -> Option<SharedString> {
    match payee.name_source.as_ref()? {
        SendNameSource::Own => None,
        SendNameSource::Registry => Some(s.vela_user.clone()),
        SendNameSource::Service { label } => Some(label.clone().into()),
    }
}

/// A payee's name as every send surface draws it — "Wallet · Vela User",
/// "bob.eth · ENS", "Savings" — or `None` when the core gave no name.
fn payee_name(payee: &SendPayee, s: &FlowStrings) -> Option<String> {
    let name = payee.name.as_deref()?;
    Some(match payee_tag(payee, s) {
        Some(tag) => format!("{name} · {tag}"),
        None => name.to_owned(),
    })
}

/// How a payee is named on a row (spec 097 F, S2): the name alone — the line
/// a long name may cut — over the line nothing cuts, whose word the name is
/// and the short address it stands for ("Vela User · 0x14fB…eA5c"); or the
/// short address alone, in mono. A name never stands in for the address on
/// the page that signs, and a long registered name cannot push the tag out
/// of sight.
fn payee_lines(payee: &SendPayee, s: &FlowStrings) -> (SharedString, bool, Option<SharedString>) {
    let short = shorten(&payee.address);
    match payee.name.as_deref() {
        Some(name) => {
            let detail = match payee_tag(payee, s) {
                Some(tag) => format!("{tag} · {short}"),
                None => short,
            };
            (name.to_owned().into(), false, Some(detail.into()))
        }
        None => (short.into(), true, None),
    }
}

/// The "To" row of the confirm and the picker: the payee's lines, and the
/// identicon seeded with the whole address (a tap shows it in full).
fn payee_fact(label: &SharedString, payee: &SendPayee, s: &FlowStrings) -> FactRow {
    let (value, mono, detail) = payee_lines(payee, s);
    FactRow {
        label: label.clone(),
        value,
        mono,
        detail,
        lead: if crate::flows::eip681::is_hex_address(&payee.address) {
            FactLead::Identicon(payee.address.clone().into())
        } else {
            FactLead::None
        },
        copy: None,
        note: None,
        danger: false,
    }
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
        recipient: pick_recipient(i),
        network_notice: i
            .send
            .request_chain_id
            .map(|chain_id| chain_notice(chain_id, &s.share_card_note)),
        selection: None,
        lock_notice: i
            .send
            .lock_error
            .as_ref()
            .and_then(|_| send_notice(i, false)),
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
        notice: chain.map(|chain_id| chain_notice(chain_id, &s.multi_send_chain_notice)),
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
            relay_sent_at_ms: None,
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

    /// Issue #332: a code scanned from the home lands on the picker with the
    /// address it read, and the picker said nothing of it. It says whom the
    /// money is for now — as the confirm words it — and nothing when nobody
    /// is held.
    #[test]
    fn the_picker_says_whom_the_money_is_for() {
        const PAYEE: &str = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141";
        crate::executor::storage::tests::with_temp_state("send-pick-recipient", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let mut view = view_with(vec![token(1, "ETH", None)], Vec::new(), None);

            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            assert!(pick.recipient.is_none(), "nobody held, no line");

            view.recipient = PAYEE.to_owned();
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            let line = pick
                .recipient
                .unwrap_or_else(|| unreachable!("the scan said nothing"));
            assert_eq!(line.label, s.to_label);
            assert_eq!(line.value.as_ref(), shorten(PAYEE));
            assert!(line.mono);
            assert!(matches!(line.lead, FactLead::Identicon(ref seed) if seed.as_ref() == PAYEE));

            // The sweep's picker is about the same person, named as the
            // confirm names them (spec 097 F): the core's payee — the name,
            // whose word it is, and the short address under it. (It used to
            // read `recipient_identity.name` alone.)
            view.payees = vec![vela_core::app::send::SendPayee {
                address: PAYEE.to_owned(),
                name: Some("alice.eth".to_owned()),
                name_source: Some(vela_core::app::send::SendNameSource::Service {
                    label: "ENS".to_owned(),
                }),
            }];
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), true, SendClass::All);
            let line = pick
                .recipient
                .unwrap_or_else(|| unreachable!("the sweep lost them"));
            assert_eq!(line.value.as_ref(), "alice.eth");
            assert_eq!(
                line.detail.as_deref(),
                Some(format!("ENS · {}", shorten(PAYEE)).as_str())
            );
            assert!(!line.mono);

            // Text that is not an address is shown as read, with no artwork
            // (the core names no payee until the address is whole).
            view.recipient = "hello".to_owned();
            view.payees.clear();
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            let line = pick.recipient.unwrap_or_else(|| unreachable!("held"));
            assert!(matches!(line.lead, FactLead::None));
        });
    }

    /// Issue #312: a code that named a network. The core has already narrowed
    /// the list to it; the picker says which network, and the sidebar's
    /// filter left on another one does not hide the payer's holdings there.
    #[test]
    fn the_picker_says_which_network_a_code_named() {
        crate::executor::storage::tests::with_temp_state("send-pick-network", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let mut view = view_with(vec![token(56, "BNB", None)], Vec::new(), None);
            view.request_chain_id = Some(56);

            let mut narrowed = view.clone();
            narrow_send_tokens(&mut narrowed, Some(100), SendClass::All);
            assert_eq!(
                narrowed.tokens.len(),
                1,
                "the sidebar's Gnosis hid BNB Chain"
            );

            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            let (chain_id, _, _, text) = pick
                .network_notice
                .unwrap_or_else(|| unreachable!("the named network went unsaid"));
            assert_eq!(chain_id, 56);
            let name = crate::executor::custom_tokens::network_name(56);
            assert_eq!(
                text.as_ref(),
                crate::wallet::fill(&s.share_card_note, "network", &name)
            );

            view.request_chain_id = None;
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            assert!(
                pick.network_notice.is_none(),
                "no network named, nothing said"
            );
        });
    }

    /// 078 W-04: a locked request on a chain the wallet lacks is refused ON
    /// the list the stage shows — with "Add this network" as its way out, and
    /// the same button busy, taking no press, while the add is out.
    #[test]
    fn a_locked_request_on_a_missing_chain_offers_to_add_it() {
        crate::executor::storage::tests::with_temp_state("send-lock-net", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let mut view = view_with(vec![token(1, "ETH", None)], Vec::new(), None);
            view.lock_error = Some(SendLockError::Network { chain_id: 59144 });

            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            let notice = pick.lock_notice.unwrap_or_else(|| unreachable!("refused"));
            assert_eq!(notice.action.as_ref(), Some(&s.lock_add_network));
            assert!(notice.body.contains("59144"), "{}", notice.body);
            assert_eq!(
                notice_way_out(&inputs(&view, &fee, &s, &wallet), false),
                Some(NoticeWayOut::AddNetwork { chain_id: 59144 })
            );

            view.adding_network = true;
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            let notice = pick.lock_notice.unwrap_or_else(|| unreachable!("refused"));
            assert_eq!(notice.action.as_ref(), Some(&s.lock_adding_network));
            assert_eq!(
                notice_way_out(&inputs(&view, &fee, &s, &wallet), false),
                None
            );

            // No refusal, no card: the ordinary list.
            view.lock_error = None;
            let pick = send_pick_with(&inputs(&view, &fee, &s, &wallet), false, SendClass::All);
            assert!(pick.lock_notice.is_none());
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
            let (_, _, letter, text) = selection
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
        Send as SendMachine, SendTreasuryAsset, SendTreasuryCoin, SendTreasuryStatus, SendView,
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
                    // …and words the figures in Gnosis's own coin (#422).
                    coin: Some(SendTreasuryCoin {
                        symbol: Some("xDAI".to_owned()),
                        balance: "0".to_owned(),
                        floor: "0.02".to_owned(),
                        suggested: "0.02".to_owned(),
                    }),
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
                relay_sent_at_ms: None,
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
            assert!(detail.contains("0.02 xDAI"), "{detail}");
            // ...and copies it: read off the screen it is 42 characters to retype.
            assert_eq!(
                notice.copy,
                Some((s.funding_copy.clone(), "0xTreasury".into()))
            );
            // Spec 098 §4: what it has against what it needs, and that it is
            // watching — and spec 080's finding, gone: no "fee reserve".
            assert!(detail.contains(s.funding_watching.as_ref()), "{detail}");
            assert!(detail.contains(s.funding_disclaimer.as_ref()), "{detail}");
            assert_eq!(notice.title.as_ref(), Some(&s.funding_title));
            assert_eq!(
                notice.body.as_ref(),
                s.funding_lead.as_str(),
                "the operator's relayer"
            );
        });
    }

    /// Spec 098 §2: a relay that cannot serve the chain is its own stop —
    /// not the funding one — with its own way back and its own way out.
    #[test]
    fn a_relay_that_cannot_serve_the_chain_says_so_and_whose_it_is() {
        crate::executor::storage::tests::with_temp_state("unreachable-notice", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let host = CoreHost::<SendMachine>::new();
            for (operator_served, lead) in [
                (true, s.unreachable_operator_lead.clone()),
                (false, s.unreachable_custom_lead.clone()),
            ] {
                let view = SendView {
                    relay_unreachable: Some(vela_core::app::send::SendRelayUnreachable {
                        chain_id: 1337,
                        operator_served,
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
                    relay_sent_at_ms: None,
                };
                let notice = send_notice(&inputs, false).unwrap_or_else(|| unreachable!("a stop"));
                assert_eq!(notice.title.as_ref(), Some(&s.unreachable_title));
                assert_eq!(notice.body, lead);
                assert_eq!(notice.dismiss.as_ref(), Some(&s.unreachable_close));
                assert_eq!(notice.copy, None, "nothing to fund");
                assert_eq!(
                    notice_way_out(&inputs, false),
                    Some(NoticeWayOut::RetryRelayUnreachable)
                );
                // Only a network the person added is theirs to point elsewhere.
                assert_eq!(notice.detail.is_some(), !operator_served);
            }
        });
    }

    /// Issue 466: both relay stops offer "Report this" exactly while the
    /// core has a report for them — which it builds only on a network Vela
    /// ships, whose relayer is the operator's. On a network the person
    /// added there is nobody to tell, and no button.
    #[test]
    fn a_relay_stop_offers_report_this_only_with_the_cores_report() {
        crate::executor::storage::tests::with_temp_state("relay-report-466", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let host = CoreHost::<SendMachine>::new();
            let report = |fingerprint: &str| vela_core::app::send::SendRelayReport {
                what: "Relayer out of gas on Unichain (130)".to_owned(),
                steps: "1. Send on Unichain (130)".to_owned(),
                area: "Send".to_owned(),
                fingerprint: fingerprint.to_owned(),
            };
            let treasury = |served: bool| SendTreasuryStatus {
                chain_id: 130,
                address: "0xTreasury".to_owned(),
                asset: SendTreasuryAsset::Native,
                balance: "0".to_owned(),
                floor: "100000000000000".to_owned(),
                bootstrap_needed: true,
                operator_served: served,
                coin: None,
            };
            let notice_of = |view: &SendView| {
                let inputs = SendInputs {
                    send: view,
                    fee: &fee,
                    s: &s,
                    wallet: &wallet,
                    locale: "en-US",
                    money: crate::wallet::live::Money::usd(),
                    identity_name: "MultiTest",
                    identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                    speed: None,
                    relay_sent_at_ms: None,
                };
                send_notice(&inputs, false).unwrap_or_else(|| unreachable!("a stop"))
            };

            let served = SendView {
                treasury_bootstrap: Some(treasury(true)),
                relay_report: Some(report("relay-gas-130")),
                ..host.view()
            };
            assert_eq!(notice_of(&served).report, Some(s.funding_report.clone()));
            let custom = SendView {
                treasury_bootstrap: Some(treasury(false)),
                relay_report: None,
                ..host.view()
            };
            assert_eq!(notice_of(&custom).report, None);

            let unreachable = |report: Option<vela_core::app::send::SendRelayReport>| SendView {
                relay_unreachable: Some(vela_core::app::send::SendRelayUnreachable {
                    chain_id: 130,
                    operator_served: report.is_some(),
                }),
                relay_report: report,
                ..host.view()
            };
            assert_eq!(
                notice_of(&unreachable(Some(report("relay-unreachable-130")))).report,
                Some(s.unreachable_report.clone())
            );
            assert_eq!(notice_of(&unreachable(None)).report, None);
            // The corpus's own words for the two buttons, not key echoes.
            assert!(!s.funding_report.contains("componentsUi"));
            assert!(!s.unreachable_report.contains("componentsUi"));

            // No other notice grows the button: the same-coin ceiling, say.
            let plain = host.view();
            assert!(
                send_notice(
                    &SendInputs {
                        send: &plain,
                        fee: &fee,
                        s: &s,
                        wallet: &wallet,
                        locale: "en-US",
                        money: crate::wallet::live::Money::usd(),
                        identity_name: "MultiTest",
                        identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                        speed: None,
                        relay_sent_at_ms: None,
                    },
                    false
                )
                .is_none_or(|notice| notice.report.is_none())
            );
        });
    }

    /// Issue #422: the stop's coin and figures are the core's. On a Xiaomi
    /// a Polygon send was asked for "0.0001 ETH" — another chain's stop, in
    /// a coin a name lookup guessed. This card writes what the core gives
    /// and nothing it does not: no figures, no amount.
    #[test]
    fn the_treasury_stop_writes_the_cores_coin_and_nothing_it_did_not_give() {
        crate::executor::storage::tests::with_temp_state("treasury-coin-422", || {
            let s = FlowStrings::resolve(&crate::loc::Loc::from_env());
            let wallet = crate::wallet::WalletStrings::resolve(&crate::loc::Loc::from_env());
            let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
            let host = CoreHost::<SendMachine>::new();
            let polygon = |coin: Option<SendTreasuryCoin>| SendView {
                treasury_bootstrap: Some(SendTreasuryStatus {
                    chain_id: 137,
                    address: "0xTreasury".to_owned(),
                    asset: SendTreasuryAsset::Native,
                    balance: "40000000000000".to_owned(),
                    floor: "100000000000000".to_owned(),
                    bootstrap_needed: true,
                    operator_served: true,
                    coin,
                }),
                ..host.view()
            };
            let detail_of = |view: &SendView| {
                let inputs = SendInputs {
                    send: view,
                    fee: &fee,
                    s: &s,
                    wallet: &wallet,
                    locale: "en-US",
                    money: crate::wallet::live::Money::usd(),
                    identity_name: "MultiTest",
                    identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                    speed: None,
                    relay_sent_at_ms: None,
                };
                let notice = send_notice(&inputs, false).unwrap_or_else(|| unreachable!("a stop"));
                notice.detail.unwrap_or_default().to_string()
            };

            let detail = detail_of(&polygon(Some(SendTreasuryCoin {
                symbol: Some("POL".to_owned()),
                balance: "0.00004".to_owned(),
                floor: "0.0001".to_owned(),
                suggested: "0.00006".to_owned(),
            })));
            assert!(
                detail.contains(&format!("{} 0.00006 POL", s.funding_amount_label)),
                "{detail}"
            );
            assert!(!detail.contains("ETH"), "{detail}");

            let detail = detail_of(&polygon(None));
            assert!(
                detail.contains("0xTreasury"),
                "still where to send: {detail}"
            );
            assert!(
                !detail.contains(s.funding_amount_label.as_ref()),
                "no amount nobody read: {detail}"
            );
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
        if let Some(clean) = amount_edited(&amount, &amount_to_input(&row.amount)) {
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

/// The core's figure as the field shows it (078 M-04) — the web's
/// `amountToInput`: the core stores and echoes a dot, a decimal-comma person
/// reads a comma. Max's "0,00075" and the typing both, so the field never
/// switches marks under the person's hands.
#[must_use]
pub fn amount_to_input(canonical: &str) -> String {
    amount_to_input_with(canonical, crate::executor::format_prefs::current().number)
}

#[must_use]
pub fn amount_to_input_with(
    canonical: &str,
    preset: vela_core::l10n::number::NumberPreset,
) -> String {
    let decimal = preset.separators().decimal;
    if decimal == "." {
        canonical.to_owned()
    } else {
        canonical.replacen('.', decimal, 1)
    }
}

/// An amount field's edit as the core reads it (spec 073;
/// `vela_core::l10n::amount_text` says why): a decimal-comma keyboard's
/// "4,5" is 4.5 — raw, the send machine read 4 in fiat mode, and a custom
/// allowance's parser dropped the comma and allowed 45. `next` is the field's
/// text after the edit and `previous` what it showed before — both in the
/// person's mark (078 M-04); the answer is the core's dot-decimal figure.
/// `None` is an edit with no reading as one figure, and the field keeps what
/// it had.
#[must_use]
pub fn amount_edited(next: &str, previous: &str) -> Option<String> {
    amount_edited_with(
        next,
        previous,
        crate::executor::format_prefs::current().number,
    )
}

/// [`amount_edited`] under a named preset — the seam a test types through
/// without touching the person's global choice.
#[must_use]
pub fn amount_edited_with(
    next: &str,
    previous: &str,
    preset: vela_core::l10n::number::NumberPreset,
) -> Option<String> {
    use vela_core::l10n::amount_text;
    amount_text::clean(next, preset, amount_text::Entry::Unknown, Some(previous))
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

    /// Issue #421 through the page's own round trip: the field's text goes
    /// through `amount_edited` to the core, and the field shows the core's
    /// figure back through `amount_to_input` — so what each key leaves on
    /// screen is exactly what the core holds. "0" then "8" is "8"; only a
    /// decimal mark may follow a leading zero, in the person's own mark.
    #[test]
    fn a_zero_leading_a_digit_is_not_kept_in_any_amount_field() {
        use vela_core::l10n::number::NumberPreset;

        /// Keys typed at the end of the field, one at a time: the field text
        /// after each, and the figure the core was handed last.
        fn type_keys(keys: &str, preset: NumberPreset) -> (String, String) {
            let (mut field, mut core) = (String::new(), String::new());
            for key in keys.chars() {
                let next = format!("{field}{key}");
                if let Some(clean) = amount_edited_with(&next, &field, preset) {
                    core = clean;
                    field = amount_to_input_with(&core, preset);
                }
            }
            (field, core)
        }

        for preset in [NumberPreset::CommaDot, NumberPreset::Indian] {
            assert_eq!(type_keys("08", preset), ("8".into(), "8".into()));
            assert_eq!(type_keys("00", preset), ("0".into(), "0".into()));
            assert_eq!(type_keys("0.08", preset), ("0.08".into(), "0.08".into()));
            assert_eq!(type_keys("0.0", preset), ("0.0".into(), "0.0".into()));
            assert_eq!(type_keys(".", preset), ("0.".into(), "0.".into()));
            assert_eq!(type_keys(".5", preset), ("0.5".into(), "0.5".into()));
        }
        for preset in [NumberPreset::DotComma, NumberPreset::SpaceComma] {
            // The person's mark on screen, the core's dot underneath.
            assert_eq!(type_keys("0,8", preset), ("0,8".into(), "0.8".into()));
            assert_eq!(type_keys("08", preset), ("8".into(), "8".into()));
            assert_eq!(type_keys(",", preset), ("0,".into(), "0.".into()));
            assert_eq!(type_keys("0,0", preset), ("0,0".into(), "0.0".into()));
        }
        // A paste: the zeros go, the figure stays.
        assert_eq!(
            amount_edited_with("008.5", "", NumberPreset::CommaDot).as_deref(),
            Some("8.5")
        );
        // A split row's share and the custom allowance call the same rule.
        let rows = vec![row("rcpt_1", "0xAAA", "0", None)];
        assert_eq!(
            split_amount_edited(&rows, 0, "08".to_owned())[0].amount,
            "8"
        );
        assert_eq!(amount_edited("08", "0").as_deref(), Some("8"));
        assert_eq!(amount_edited("00", "0").as_deref(), Some("0"));
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

/// The recipient's trust line: that it is a token's own contract (spec 096
/// F12, the core's verdict — said before anything else), else the payee's
/// name with whose word it is (spec 097 F: "Wallet · Vela User", never the
/// resolver's raw "passkey"), else the first-interaction tell (the one that
/// matters for a poisoned look-alike).
fn recipient_note(send: &SendView, s: &FlowStrings) -> Option<SharedString> {
    if send.recipient_is_token_contract {
        return Some(s.recipient_token_contract.clone());
    }
    if let Some(name) = single_payee(send).and_then(|payee| payee_name(&payee, s)) {
        return Some(name.into());
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
    /// The network's RPC or the relay was changed — run the pre-check again
    /// (spec 098 §2).
    RetryRelayUnreachable,
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

    // Spec 098 §2: the relay cannot serve this chain at all. Gas would not
    // help, so this is not the funding stop — and before 098 the send went on
    // to the passkey here and failed after the person had signed.
    if let Some(sheet) = &send.relay_unreachable {
        let notice = SendNotice {
            dismiss: Some(s.unreachable_close.clone()),
            title: Some(s.unreachable_title.clone()),
            // Whose it is to fix is the core's verdict, as on the funding stop.
            body: if sheet.operator_served {
                s.unreachable_operator_lead.clone()
            } else {
                s.unreachable_custom_lead.clone()
            },
            detail: (!sheet.operator_served).then(|| s.unreachable_hint.clone()),
            action: Some(s.unreachable_retry.clone()),
            copy: None,
            // Issue 466: the operator is told through the reporter — only
            // where the core built a report, which is only where the relay
            // is Vela's (a network the build ships).
            report: send
                .relay_report
                .as_ref()
                .map(|_| s.unreachable_report.clone()),
            error: true,
        };
        return Some((notice, Some(NoticeWayOut::RetryRelayUnreachable)));
    }

    // The relay cannot carry anything on this chain until its float is topped
    // up. A stop, and the only one that names an address to send to.
    if let Some(treasury) = &send.treasury_bootstrap {
        // Every figure, and the coin it is in, is the core's (issue #422): the
        // stop's own chain's coin, the relay's shortfall for that chain. This
        // shell used to name the coin from the built-in list and do the
        // arithmetic in `f64`; it now writes only the decimal mark.
        let amount_line = treasury.coin.as_ref().map(|coin| {
            let symbol = coin.symbol.as_deref().unwrap_or_default();
            let mark = |figure: &str| crate::wallet::live::with_decimal_mark(figure.to_owned());
            // Spec 098 §4: what it has against what it needs, in its own coin.
            let balance_line = fill(
                &fill(
                    &fill(&s.funding_balance_line, "balance", &mark(&coin.balance)),
                    "floor",
                    &mark(&coin.floor),
                ),
                "symbol",
                symbol,
            );
            format!(
                "  ·  {} {} {symbol}\n{balance_line}",
                s.funding_amount_label,
                mark(&coin.suggested),
            )
        });
        let notice = SendNotice {
            // The way out of the stop itself. Without it the only exit from a
            // treasury that cannot pay is closing the whole journey — the core
            // has had `DismissTreasurySheet` since 026 and nothing sent it.
            dismiss: Some(s.funding_close.clone()),
            title: Some(s.funding_title.clone()),
            // Whose relayer it is — the operator's on a network Vela ships,
            // the person's on one they added (spec 060) — is the core's verdict.
            body: SharedString::from(if treasury.operator_served {
                s.funding_lead.clone()
            } else {
                s.funding_custom_lead.clone()
            }),
            detail: Some(
                format!(
                    "{} {}{}\n{}\n{}",
                    s.funding_address_label,
                    treasury.address,
                    // No figures the core could read: no amount, rather than
                    // a zero nobody measured.
                    amount_line.unwrap_or_default(),
                    s.funding_disclaimer,
                    // The core asks the relay again every 10 s and closes this
                    // once funded (spec 098 §4); "Retry" is for the impatient.
                    s.funding_watching,
                )
                .into(),
            ),
            action: Some(s.funding_check_now.clone()),
            // Spec 098 §4: the address is what a person needs to fund it —
            // read off the screen it is 42 characters to retype.
            copy: Some((s.funding_copy.clone(), treasury.address.clone().into())),
            // Issue 466: the lead says telling the operator is the fastest
            // fix; this is how — the core's report, through the reporter.
            report: send.relay_report.as_ref().map(|_| s.funding_report.clone()),
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
            // While an add is out the button says so and takes no press.
            SendLockError::Network { chain_id } if !send.adding_network => {
                Some(NoticeWayOut::AddNetwork {
                    chain_id: *chain_id,
                })
            }
            SendLockError::Network { .. } | SendLockError::Token => None,
        };
        let action = match error {
            SendLockError::Network { .. } if send.adding_network => {
                Some(s.lock_adding_network.clone())
            }
            SendLockError::Network { .. } => Some(s.lock_add_network.clone()),
            SendLockError::Token => None,
        };
        let notice = SendNotice {
            dismiss: None,
            title: Some(title),
            body,
            detail,
            action,
            copy: None,
            report: None,
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
        let exact = |base: &str| {
            base.parse::<u128>().map_or_else(
                |_| base.to_owned(),
                |units| vela_core::app::fee_policy::from_base_units(units, decimals),
            )
        };
        // The one token-amount rule — and the ceiling cut DOWN, so the figure
        // it offers can be typed back and still clear the fee.
        let human = |base: &str| trimmed_str(&exact(base));
        let ceiling = |base: &str| crate::wallet::live::token_amount_text_down(&exact(base));
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
                        &ceiling(&issue.max_transfer_amount),
                    ),
                    "symbol",
                    &issue.symbol,
                )
                .into(),
            ),
            action: Some(s.same_fee_edit.clone()),
            copy: None,
            report: None,
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
            copy: None,
            report: None,
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
    // field, and the confirm disarms with nothing said.
    if confirming && let Some(issue) = &send.confirm_amount_issue {
        let notice = SendNotice {
            dismiss: None,
            title: None,
            body: cannot_convert(issue, s),
            detail: None,
            action: Some(s.same_fee_edit.clone()),
            copy: None,
            report: None,
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
            copy: None,
            report: None,
            error: false,
        },
        None,
    ))
}

/// The groups' members as split rows, in the order `contact_pick` draws
/// them — tapping a group ADDS everybody in it to the form, amounts blank,
/// each under the name the PERSON gave them. The core assigns the row ids.
///
/// Only `name`, never `resolved_name` (spec 097 F, S2): a split row's name is
/// the person's own word (the core calls it `own` and draws it untagged), and
/// a resolved name is the registry's or a name service's — anyone's word for
/// that address. Seeded as a row name, it would pass for the person's own.
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
                    name: member.name.clone(),
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
    let recipient_note = if sweeping && !send.recipient_is_token_contract {
        Some(s.multi_send_same_recipient.clone())
    } else if split {
        None
    } else {
        recipient_note(send, s)
    };
    let recipient_note_warn = !split && send.recipient_is_token_contract;
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
        recipient_note_warn,
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
                    address: draft.name.as_ref().map(|_| shorten(&draft.address).into()),
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
        summary_detail: None,
        remaining: send.split_remaining.as_ref().filter(|_| split).map(|left| {
            fill(
                &s.split_remaining,
                "amount",
                format!(
                    "{} {symbol}",
                    crate::wallet::live::token_amount_text_down(left)
                )
                .trim(),
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
    let mut facts = vec![
        FactRow {
            label: s.from_label.clone(),
            value: i.identity_name.to_owned().into(),
            lead: FactLead::Identicon(i.identity_address.to_owned().into()),
            mono: false,
            copy: None,
            note: None,
            danger: false,
            detail: None,
        },
        FactRow {
            label: s.detail_chain.clone(),
            value: chain_name(chain_id).into(),
            lead: FactLead::Token(network_mark(chain_id)),
            mono: false,
            copy: None,
            note: None,
            danger: false,
            detail: None,
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
            danger: false,
            detail: None,
        },
    ];
    // Who is paid (spec 097 F, S2): the payee the core names, with the short
    // address under any name — never a name alone, and never the stale
    // single `recipient` on a split, whose people are the rows below.
    if let Some(payee) = single_payee(send) {
        facts.insert(1, payee_fact(&s.to_label, &payee, s));
    }
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
            danger: false,
            detail: None,
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
            copy: None,
            report: None,
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
            // One transfer reads as the rows and the form read it — the
            // ladder, not the eighteen digits of a balance less a fee (a Max
            // wrote "0.00254" on the form; the confirm used to repeat it to
            // the wei). A split's total stays exact: it is the sum of rows.
            None if send.split_mode => format!("{} {symbol}", send.confirm_amount)
                .trim()
                .to_owned()
                .into(),
            None => trimmed_str(&send.confirm_amount).into(),
        },
        // The unit beside a single transfer's figure, as the form draws it.
        amount_unit: (sweep.is_none() && !send.split_mode && !symbol.is_empty())
            .then(|| SharedString::from(symbol.clone())),
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
            // Spec 097 F: each row is the core's payee at the same index —
            // a name over its short address, or the address alone in mono.
            send.recipients
                .iter()
                .enumerate()
                .map(|(index, draft)| {
                    let payee = send
                        .payees
                        .get(index)
                        .cloned()
                        .unwrap_or_else(|| SendPayee {
                            address: draft.address.trim().to_owned(),
                            name: None,
                            name_source: None,
                        });
                    let (label, mono, detail) = payee_lines(&payee, s);
                    BreakdownRow {
                        seed: (!payee.address.is_empty())
                            .then(|| SharedString::from(payee.address.clone())),
                        label,
                        mono,
                        detail,
                        value: format!("{} {symbol}", draft.amount)
                            .trim()
                            .to_owned()
                            .into(),
                    }
                })
                .collect()
        } else {
            Vec::new()
        },
        // The core's own verdicts: a token's own contract (spec 096 F12)
        // first, else the first time, resolved on this page only (single
        // recipient).
        recipient_tag: if send.recipient_is_token_contract {
            Some(s.recipient_token_contract.clone())
        } else {
            (!send.split_mode
                && send
                    .recipient_risk
                    .as_ref()
                    .is_some_and(|risk| risk.first_time == Some(true)))
            .then(|| s.first_time_tag.clone())
        },
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
                mono: false,
                detail: None,
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
    // The relay holds it while it tops up its gas on the chain: why it waits,
    // in place of a wait that reads like the network's (098 follow-up).
    let funding = (hold == Some(SendHoldReason::RelayFunding)).then(|| s.tx_relay_funding.clone());

    // Spec 082 RA4/RA10: the relay never had it — nothing was sent, which is
    // exactly what the generic failure says. Never the fee-rejected words: no
    // fee was ever refused.
    if status == Some(SendReceiptStatus::NotSent) {
        return SendReceipt {
            stage: ReceiptStage::Failed,
            progress: None,
            explorer: None,
            cta_accent: false,
            breakdown_title: None,
            breakdown: Vec::new(),
            title: s.tx_error_generic.clone(),
            captions: Vec::new(),
            hash: None,
            cta: s.done.clone(),
        };
    }
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
    // Spec 082 RA10 (ruling 1): the reply was lost and the payment may be on
    // its way. Still "submitting", with the one sentence that is true now, the
    // hash Vela is following, and a way to close — never a retry, which would
    // be a second payment.
    if status == Some(SendReceiptStatus::MaybeSent) {
        return SendReceipt {
            stage: ReceiptStage::Submitted,
            progress: None,
            explorer: None,
            cta_accent: false,
            breakdown_title: breakdown_title.clone(),
            breakdown: breakdown.clone(),
            title: s.tx_submitting.clone(),
            captions: vec![s.tx_maybe_sent.clone()],
            hash: send
                .user_op_hash
                .clone()
                .map(|hash| (s.tx_hash.clone(), hash.into())),
            cta: s.tx_close_background.clone(),
        };
    }
    if status == Some(SendReceiptStatus::Confirmed) {
        // Spec 097 F (S3): what was sent, from the receipt's own coins — one
        // coin is "Sent 0.5 ETH" (a split's TOTAL); several have no one
        // figure and are listed below "Sent", never the first standing for
        // all. A receipt that lists none reads as it always did.
        let coins = send
            .receipt
            .as_ref()
            .map_or(&[][..], |receipt| receipt.coins.as_slice());
        let title = match coins {
            [coin] => fill(
                &fill(&s.tx_confirmed_title, "amount", &trimmed_str(&coin.amount)),
                "symbol",
                &coin.symbol,
            )
            .into(),
            [_, _, ..] => s.tx_sent.clone(),
            [] => {
                let amount = send
                    .receipt
                    .as_ref()
                    .map(|receipt| receipt.amount.clone())
                    .filter(|amount| !amount.is_empty())
                    .unwrap_or_else(|| send.confirm_amount.clone());
                fill(
                    &fill(&s.tx_confirmed_title, "amount", &trimmed_str(&amount)),
                    "symbol",
                    &symbol,
                )
                .into()
            }
        };
        // A split names its count where a single send or a sweep names its
        // one recipient; a sweep's "N assets" heads its list, not this line.
        let recipients = breakdown_title
            .as_ref()
            .filter(|_| sweep_coins(send).is_empty());
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
            title,
            // A split names its count here and its people below; "To " with
            // nobody after it was what the single-recipient line read as.
            captions: vec![
                format!(
                    "{} · {}",
                    recipients.map_or_else(|| fill(&s.to_name, "name", &to), ToString::to_string),
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
        // Spec 099 R6: the core's one countdown, from the relay's send.
        let typical = send
            .receipt
            .as_ref()
            .and_then(|receipt| receipt.typical_inclusion_s);
        let pace = vela_core::app::tx_tracker::landing_pace(
            i.relay_sent_at_ms,
            typical,
            crate::executor::now_ms(),
        );
        return SendReceipt {
            stage: ReceiptStage::Submitted,
            progress: pace.progress,
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
                use vela_core::app::tx_tracker::LandingLine;
                // Before the relay has sent it, the landing says the relay is
                // sending it — never a chain's countdown over the relay's own
                // queue (spec 099 R6).
                let waiting = pace.line == LandingLine::Waiting;
                let mut lines = vec![held.or_else(|| funding.clone()).unwrap_or_else(|| {
                    if waiting {
                        s.tx_relay_sending.clone()
                    } else {
                        s.tx_waiting_confirm.clone()
                    }
                })];
                // Nothing is on the network while the relay funds itself, so
                // there is no confirmation time to count down.
                if funding.is_none()
                    && !waiting
                    && pace.line != LandingLine::None
                    && let Some(typical) = typical
                {
                    lines.push(
                        fill(
                            &fill(&s.tx_typical_time, "chainName", &chain_name(chain_id)),
                            "estSecs",
                            &typical.to_string(),
                        )
                        .into(),
                    );
                    let seconds = pace.seconds.to_string();
                    lines.push(match pace.line {
                        LandingLine::Remaining => {
                            fill(&s.tx_remaining, "remaining", &seconds).into()
                        }
                        LandingLine::Elapsed => fill(&s.tx_elapsed, "elapsed", &seconds).into(),
                        _ => s.tx_slow_confirm.clone(),
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
///
/// The core's curve (`tx_tracker::landing_pace`, spec 099 R6).
#[must_use]
pub fn ring_progress(elapsed_s: u64, typical_s: u64) -> Option<f32> {
    let typical = u16::try_from(typical_s).ok()?;
    #[allow(clippy::cast_precision_loss, reason = "seconds as ms")]
    let now = elapsed_s as f64 * 1000.0;
    vela_core::app::tx_tracker::landing_pace(Some(0.0), Some(typical), now).progress
}

/// The coins a sweep's receipt lists (spec 097 F, S3): every coin the
/// operation sent, when it sent several. Empty for one coin — a single send,
/// a split, or a sweep whose native line the gas reserve dropped — which the
/// title names.
fn sweep_coins(send: &SendView) -> &[SendReceiptCoin] {
    match send.receipt.as_ref() {
        Some(receipt) if receipt.coins.len() > 1 => &receipt.coins,
        _ => &[],
    }
}

/// Spec 038 #D2: a split's parts on the receipt as on the confirm — from the
/// receipt's own transfers once the core froze them, from the drafts before
/// that. A sweep's parts are its coins, every one it sent (spec 097 F, S3:
/// its success screen named the first coin and dropped the rest); its one
/// recipient is already the caption. Nothing for a single send.
fn receipt_parts(
    send: &SendView,
    s: &FlowStrings,
    symbol: &str,
) -> (Option<SharedString>, Vec<BreakdownRow>) {
    let coins = sweep_coins(send);
    if !coins.is_empty() {
        let rows = coins
            .iter()
            .map(|coin| BreakdownRow {
                seed: None,
                label: coin.symbol.clone().into(),
                mono: false,
                detail: None,
                value: format!("{} {}", trimmed_str(&coin.amount), coin.symbol)
                    .trim()
                    .to_owned()
                    .into(),
            })
            .collect();
        let title = fill(&s.assets_count, "n", &coins.len().to_string());
        return (Some(title.into()), rows);
    }
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
                mono: false,
                detail: None,
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
                mono: false,
                detail: None,
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
            mono: false,
            detail: None,
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
    crate::wallet::live::token_amount_text(value)
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
                address_full: contact.address.clone().into(),
            })
            .collect(),
    }
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
                    // The sheet's name over the address, as the web draws
                    // them (078 T062) — the name ALONE hid which address it
                    // would pay.
                    name: row.name.clone().map(Into::into),
                    address: crate::contacts::model::shorten(&row.address),
                    seed: Some(row.address.clone().into()),
                    amount: amount.into(),
                    // A fiat sheet's figure exactly as the sheet wrote it, so
                    // it can be read back against the sheet under what it
                    // became.
                    source: fiat.then(|| format!("{} {}", row.raw_amount, view.fiat_code).into()),
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
                name: None,
                address: error.raw.clone().into(),
                seed: None,
                amount: SharedString::default(),
                source: None,
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
                &format!(
                    "{} {symbol}",
                    crate::wallet::live::token_amount_text_down(left)
                ),
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
        unit_caption: s.batch_unit_caption.clone(),
        unit_fiat: fill(&s.batch_unit_fiat, "code", code).into(),
        unit_token: fill(&s.batch_unit_token, "sym", symbol).into(),
        fiat_on: view.unit == BatchUnit::Fiat,
        paste: paste.into(),
        paste_empty: view.file_name.is_none() && view.raw_text.is_empty(),
        import_file: if view.busy {
            s.batch_reading.clone()
        } else {
            s.batch_import_file.clone()
        },
        template: if view.template_saved {
            s.batch_template_saved.clone()
        } else {
            s.batch_template.clone()
        },
        template_saved: view.template_saved,
        // What the picker takes — or, once a file was picked, which file the
        // rows below came from (the web's `formats` / `fileName`).
        formats: view
            .file_name
            .clone()
            .map_or_else(|| crate::flows::fixtures::BATCH_FORMATS.into(), Into::into),
        file_named: view.file_name.is_some(),
        rate_section: s.batch_rate_section.clone(),
        rate_value: rate_value.into(),
        rate_equation: None,
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
                // 087: a legacy code page says how to save the file, not
                // "use a CSV" — which it is.
                body: if view.file_failure == Some(BatchFileFailure::UnsupportedEncoding) {
                    s.batch_import_failed_encoding.clone()
                } else {
                    s.batch_import_failed_body.clone()
                },
                detail: None,
                action: None,
                copy: None,
                report: None,
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
                copy: None,
                report: None,
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

    /// 078 M-04: the field speaks the person's decimal mark; the core's dot
    /// is only its storage.
    #[test]
    fn the_amount_field_shows_the_persons_decimal_mark() {
        use vela_core::l10n::number::NumberPreset;
        assert_eq!(
            amount_to_input_with("0.00075", NumberPreset::CommaDot),
            "0.00075"
        );
        assert_eq!(
            amount_to_input_with("0.00075", NumberPreset::Indian),
            "0.00075"
        );
        assert_eq!(
            amount_to_input_with("0.00075", NumberPreset::DotComma),
            "0,00075"
        );
        assert_eq!(
            amount_to_input_with("12.5", NumberPreset::SpaceComma),
            "12,5"
        );
        assert_eq!(amount_to_input_with("12", NumberPreset::DotComma), "12");
        assert_eq!(amount_to_input_with("", NumberPreset::DotComma), "");
        // …and what is typed over it reads back as the core's figure.
        let clean = vela_core::l10n::amount_text::clean(
            "0,000756",
            NumberPreset::DotComma,
            vela_core::l10n::amount_text::Entry::Unknown,
            Some("0,00075"),
        );
        assert_eq!(clean.as_deref(), Some("0.000756"));
    }
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
            spent_by_operation: false,
            short: None,
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
                relay_sent_at_ms: None,
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
        // Spec 096 F12: a token's own contract is said first.
        send.split_mode = false;
        send.recipient_is_token_contract = true;
        assert_eq!(tag(&send), Some(s.recipient_token_contract.clone()));
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
        use vela_core::app::send::{Send, SendReceiptCoin, SendReceiptView};
        let s = strings();
        let wallet = wallet_strings();
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        let mut send = CoreHost::<Send>::new().view();
        send.receipt = Some(SendReceiptView {
            status,
            hold_reason: hold,
            kind: None,
            transfers: Vec::new(),
            coins: vec![SendReceiptCoin {
                amount: "0.001".to_owned(),
                symbol: "ETH".to_owned(),
                logo_urls: Vec::new(),
                token_address: None,
                usd_value: 0.0,
            }],
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
            relay_sent_at_ms: None,
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

    /// Spec 082 RA10 (ruling 1): a payment whose reply was lost is still
    /// "submitting", captioned "it may have been sent — don't send it again",
    /// with a way to close and no retry; one the relay never had is the plain
    /// failure — never "fees stayed above…", since no fee was refused, even
    /// when the core still carries a fee reason.
    #[test]
    fn a_lost_reply_may_have_been_sent_and_a_never_held_op_was_not_sent() {
        use vela_core::app::send::{SendHoldReason, SendReceiptStatus};
        let s = strings();
        let maybe = receipt_with(SendReceiptStatus::MaybeSent, None);
        assert_eq!(maybe.stage, ReceiptStage::Submitted);
        assert_eq!(maybe.title, s.tx_submitting);
        assert_eq!(maybe.captions, vec![s.tx_maybe_sent.clone()]);
        assert_eq!(maybe.cta, s.tx_close_background);
        assert!(maybe.explorer.is_none());

        let not_sent = receipt_with(
            SendReceiptStatus::NotSent,
            Some(SendHoldReason::FeeRejected),
        );
        assert_eq!(not_sent.stage, ReceiptStage::Failed);
        assert_eq!(not_sent.title, s.tx_error_generic);
        assert!(not_sent.captions.is_empty(), "never the fee-rejected words");
        assert_eq!(not_sent.cta, s.done);
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
            Send, SendReceiptCoin, SendReceiptKind, SendReceiptStatus, SendReceiptTransfer,
            SendReceiptView,
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
            // The core's one coin for a split: the TOTAL of its rows.
            coins: vec![SendReceiptCoin {
                amount: "0.5".to_owned(),
                symbol: "ETH".to_owned(),
                logo_urls: Vec::new(),
                token_address: None,
                usd_value: 0.0,
            }],
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
            relay_sent_at_ms: None,
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
        // Spec 097 F: the title is the split's TOTAL, in the coin it moved —
        // not the first row's figure, and not whichever coin is selected now
        // (none is, on this booted view).
        assert_eq!(
            receipt.title.as_ref(),
            fill(
                &fill(&s.tx_confirmed_title, "amount", "0.5"),
                "symbol",
                "ETH"
            ),
            "Sent 0.5 ETH"
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
        // The relay topping up its gas: why it waits, in place of the wait.
        let funding = receipt_with(
            SendReceiptStatus::Submitted,
            Some(SendHoldReason::RelayFunding),
        );
        assert_eq!(funding.captions, vec![s.tx_relay_funding.clone()]);
        assert_ne!(
            held.captions,
            vec![s.tx_waiting_confirm.clone()],
            "the ordinary wait must not stand in for the hold"
        );

        // No hold, and no word yet that the relay sent it (spec 099 R6): the
        // relay is sending it, and nothing counts down.
        let plain = receipt_with(SendReceiptStatus::Submitted, None);
        assert_eq!(plain.captions, vec![s.tx_relay_sending.clone()]);
        assert_eq!(plain.progress, None);
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

    /// No filter row over the assets (078 T067): the web's Assets screen has
    /// none, and the desktop's pill and "Add" answered no click. Narrowed or
    /// not, the sidebar's selected network says which list this is.
    #[test]
    fn the_assets_panel_draws_no_filter_row() {
        let mut view = view();
        view.tokens = vec![
            token(100, "xDAI", "1", Some(1.0)),
            token(1, "ETH", "1", Some(2000.0)),
        ];
        for narrowed in [None, Some(100)] {
            let panel = assets(
                &view,
                &strings(),
                &wallet_strings(),
                "en-US",
                narrowed,
                crate::wallet::live::Money::usd(),
            );
            assert!(panel.filter.is_none(), "narrowed = {narrowed:?}");
        }
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

    /// The wizard draws a network being added as itself (the core's kind
    /// rule): Base wears Base's own logo and no badge — the native coin's rule
    /// put Ethereum's logo on every ETH L2 it found — and a card whose chain
    /// has not resolved yet asks the endpoint for nothing (never chain 0).
    #[test]
    fn the_native_tab_wears_each_networks_own_logo() {
        crate::executor::storage::tests::with_temp_state("flows-wizard-marks", || {
            use crate::flows::fixtures::AddTokenResult;
            use vela_core::app::network_admin::{NetChainIndexEntry, NetWizardPhase};
            let s = strings();
            let mut wizard = CoreHost::<vela_core::app::network_admin::NetworkAdmin>::new()
                .view()
                .wizard;
            wizard.phase = NetWizardPhase::Suggested;
            wizard.suggestions = vec![NetChainIndexEntry {
                chain_id: 8453,
                name: "Base".to_owned(),
                short_name: "base".to_owned(),
                native_currency_symbol: "ETH".to_owned(),
                has_logo: true,
            }];
            match add_network_tab(&wizard, "base", None, &s).result {
                AddTokenResult::Suggestions(rows) => {
                    let mark = &rows[0].mark;
                    assert_eq!(mark.ticker.as_ref(), "ETH");
                    assert_eq!(
                        mark.logos.logo_urls,
                        vec![SharedString::from(
                            "https://ethereum-data.getvela.app/chainlogos/eip155-8453.png"
                        )]
                    );
                    assert!(mark.logos.badge_hidden && mark.logos.badge_logo.is_none());
                }
                _ => unreachable!("a match is offered"),
            }

            wizard.suggestions.clear();
            wizard.phase = NetWizardPhase::Resolving;
            wizard.chain_info = None;
            match add_network_tab(&wizard, "base", None, &s).result {
                AddTokenResult::Network { mark, .. } => {
                    assert!(mark.logos.logo_urls.is_empty(), "nothing asked of chain 0");
                    assert!(mark.logos.badge_hidden);
                }
                _ => unreachable!("a resolving card"),
            }
        });
    }

    /// The kind rule on the detail page: ETH sent on Base is a BASE record,
    /// so its network line wears Base's own logo, no badge — not Ethereum's,
    /// which the coin's rule gives the coin. On the person's endpoint, read
    /// when the row is built.
    #[test]
    fn the_network_line_wears_the_networks_logo_not_the_coins() {
        crate::executor::storage::tests::with_temp_state("flows-network-fact", || {
            use vela_core::app::activity_feed::FeedTxRecord;
            let record = |endpoint: &str| {
                let _ = crate::executor::storage::write_value(
                    crate::executor::storage::KEY_SERVICE_ENDPOINTS,
                    serde_json::json!({ "ethereumDataURL": endpoint }),
                );
                FeedTxRecord {
                    id: "base-eth".to_owned(),
                    user_op_hash: String::new(),
                    tx_hash: "0xbeef".to_owned(),
                    from: "0xme".to_owned(),
                    to: "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141".to_owned(),
                    to_name: None,
                    value: "0.01".to_owned(),
                    symbol: "ETH".to_owned(),
                    decimals: 18,
                    logo_urls: None,
                    chain_id: 8453,
                    timestamp: 1_756_000_000.0,
                    day_start_ms: 0.0,
                    status: FeedTxStatus::Confirmed,
                    kind: None,
                    usd: None,
                    dapp_url: None,
                    intent: None,
                    balance_changes: None,
                    calldata: None,
                    call_data: None,
                    summary: None,
                    settlement: None,
                }
            };
            let (s, w) = (strings(), wallet_strings());
            for (endpoint, logo) in [
                (
                    "",
                    "https://ethereum-data.getvela.app/chainlogos/eip155-8453.png",
                ),
                (
                    "https://data.example/",
                    "https://data.example/chainlogos/eip155-8453.png",
                ),
            ] {
                let view = crate::wallet::fixtures::core_feed(vec![record(endpoint)]);
                let detail = tx_detail(
                    &view,
                    "base-eth",
                    &s,
                    &w,
                    false,
                    "en-US",
                    crate::wallet::live::Money::usd(),
                )
                .unwrap_or_else(|| unreachable!("the row exists"));
                let network = detail
                    .facts
                    .iter()
                    .find(|fact| fact.label == s.detail_chain)
                    .unwrap_or_else(|| unreachable!("the detail names its network"));
                assert_eq!(network.value.as_ref(), "Base");
                match &network.lead {
                    FactLead::Token(mark) => {
                        assert_eq!(mark.ticker.as_ref(), "ETH");
                        assert_eq!(
                            mark.logos.logo_urls,
                            vec![SharedString::from(logo)],
                            "{endpoint:?}"
                        );
                        assert!(mark.logos.badge_hidden, "a network wears no badge");
                    }
                    _ => unreachable!("the network line leads with the network's mark"),
                }
            }
        });
    }

    /// A transaction's detail, and the row order the listeners are bound in.
    /// Spec 082 RG2 (T072), spec 093: a dApp's transaction opens under its
    /// row's own title, with the status the core gave its row and, first,
    /// the site that asked.
    #[test]
    fn a_dapp_transaction_detail_names_who_asked() {
        use vela_core::app::activity_feed::{FeedTxKind, FeedTxRecord};
        let record = FeedTxRecord {
            id: "dapp-1-tx".to_owned(),
            user_op_hash: "0xop".to_owned(),
            tx_hash: String::new(),
            from: "0xme".to_owned(),
            to: "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141".to_owned(),
            to_name: Some("Ann".to_owned()),
            value: "0x38d7ea4c68000".to_owned(),
            symbol: "xDAI".to_owned(),
            decimals: 18,
            logo_urls: None,
            chain_id: 100,
            timestamp: 1_756_000_000.0,
            day_start_ms: 0.0,
            status: FeedTxStatus::Pending,
            kind: Some(FeedTxKind::DappTx),
            usd: None,
            dapp_url: Some("http://127.0.0.1:8137".to_owned()),
            intent: None,
            balance_changes: None,
            calldata: None,
            call_data: None,
            summary: None,
            settlement: None,
        };
        let view = crate::wallet::fixtures::core_feed(vec![record]);
        let (s, w) = (strings(), wallet_strings());
        let detail = tx_detail(
            &view,
            "dapp-1-tx",
            &s,
            &w,
            false,
            "en-US",
            crate::wallet::live::Money::usd(),
        )
        .unwrap_or_else(|| unreachable!("the row exists"));
        let row = crate::wallet::live::activity_rows(&view, &w, &s, false);
        assert_eq!(
            detail.title, row[0].title,
            "the header says what the row says"
        );
        assert_eq!(
            detail.status.as_ref().map(|chip| &chip.text),
            Some(&s.status_pending)
        );
        assert_eq!(detail.note, None, "a transaction wears its chip");
        assert_eq!(detail.facts[0].label, s.detail_app);
        assert_eq!(detail.facts[0].value.as_ref(), "127.0.0.1:8137");
        // A plain send names who got it, by the name the row knows them by,
        // with the whole address on the copy button (spec 082 RJ16).
        let paid = &detail.facts[2];
        assert_eq!(
            (&paid.label, paid.value.as_ref(), paid.mono),
            (&s.detail_to, "Ann", false)
        );
        assert_eq!(
            paid.copy.as_deref(),
            Some("0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141")
        );
    }

    /// Spec 082 RJ16 (G52, DX-W3): a relay-refused dApp record for a USDC
    /// `transfer(0x7687…D141, 10^30)` named the token contract 0xDDAf…7A83
    /// as 接收方 and offered the explorer for an op that never reached the
    /// chain (its stored "tx hash" was the op hash). Stored exactly as
    /// `sign_request::persist_record` writes it and read the executor's way:
    /// the recipient is the transfer's, and there is nothing to open. A call
    /// that is not a transfer names its contract, as the contract. Both are
    /// the core's facts (spec 093), in its order, and neither shows a hash.
    #[test]
    fn a_dapp_record_names_who_got_it_and_offers_no_explorer_without_a_tx() {
        use vela_core::app::activity_feed::{
            ActivityFeed, Event as FeedEvent, FeedOperation, FeedShellResult,
        };
        const OP: &str = "0xa974c5dd0a00000000000000000000000000000000000000000000000000beef";
        const USDC: &str = "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83";
        let transfer = format!(
            "0xa9059cbb{:0>64}{:064x}",
            "76875e38fc6bc2dedcaed807ce00782db5c0d141",
            10u128.pow(30)
        );
        let row = |id: &str, data: &str| {
            serde_json::json!({
                "id": id,
                "userOpHash": OP,
                "txHash": OP,
                "from": "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
                "to": USDC,
                "value": "0x0",
                "symbol": "xDAI",
                "decimals": 18,
                "chainId": 100,
                "timestamp": 1_756_000_000,
                "status": "failed",
                "type": "dapp_tx",
                "dappOrigin": "http://127.0.0.1:8141",
                "signedRequest": serde_json::json!([{ "to": USDC, "data": data }]).to_string(),
                "requestTruncated": false,
                "maybeSent": false,
            })
        };
        let records: Vec<_> = [
            row("dapp-transfer", &transfer),
            row(
                "dapp-swap",
                "0x38ed173900000000000000000000000000000000000000000000000000000000000000ff",
            ),
        ]
        .iter()
        .filter_map(crate::executor::activity_feed::to_record)
        .collect();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].call_data.as_deref(), Some(transfer.as_str()));

        let mut host = CoreHost::<ActivityFeed>::new();
        let mut pending = host.dispatch(FeedEvent::AccountSwitched {
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
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
                FeedOperation::ResolveRecipientIdentity { addr } => {
                    FeedShellResult::AliasResolved {
                        addr: addr.clone(),
                        name: None,
                    }
                }
                _ => continue,
            };
            pending.extend(host.resolve(next.id, result));
        }
        let view = host.view();
        let s = strings();
        let detail = |id: &str| {
            tx_detail(
                &view,
                id,
                &s,
                &wallet_strings(),
                false,
                "zh",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("{id} is in the feed"))
        };

        let refused = detail("dapp-transfer");
        assert_eq!(
            refused.status.as_ref().map(|chip| &chip.text),
            Some(&s.status_failed)
        );
        assert!(
            refused.explorer_url.is_none(),
            "an op hash is not a transaction"
        );
        for (id, party) in [
            ("dapp-transfer", &s.detail_to),
            ("dapp-swap", &s.detail_contract),
        ] {
            let drawn = detail(id);
            let labels: Vec<&SharedString> = drawn.facts.iter().map(|fact| &fact.label).collect();
            assert_eq!(
                labels,
                vec![&s.detail_app, &s.detail_chain, party, &s.detail_date],
                "{id}: one party — who got the money, else the contract"
            );
            assert_eq!(drawn.facts[0].value.as_ref(), "127.0.0.1:8141", "{id}");
            assert!(
                !drawn.facts.iter().any(|fact| fact.label == s.detail_hash),
                "{id}: no hash row for a transaction that never was"
            );
        }
        let refused = detail("dapp-transfer");
        assert_eq!(
            refused.facts[2].value.as_ref(),
            "0x7687…D141",
            "the recipient, not the contract"
        );
        assert!(
            refused.facts[2].copy.as_deref().is_some_and(
                |copy| copy.eq_ignore_ascii_case("0x76875e38fc6bc2dedcaed807ce00782db5c0d141")
            ),
            "the whole address on the copy button"
        );
        assert_eq!(
            detail("dapp-swap").facts[2].copy.as_deref(),
            Some(USDC.to_lowercase().as_str()),
            "the contract, as the contract"
        );
    }

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
                kind: if incoming {
                    vela_core::app::activity_feed::FeedTxKind::Receive
                } else {
                    vela_core::app::activity_feed::FeedTxKind::Send
                },
                // The row's status is the core's (spec 082 RG1): the stored
                // record's, which says pending for the sent one.
                status: if incoming {
                    vela_core::app::activity_feed::FeedTxStatus::Confirmed
                } else {
                    vela_core::app::activity_feed::FeedTxStatus::Pending
                },
                site: None,
                counterparty_role: Default::default(),
                dapp: None,
                subtitle: Vec::new(),
                priced: true,
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
                    dapp_url: None,
                    intent: None,
                    balance_changes: None,
                    calldata: None,
                    call_data: None,
                    summary: None,
                    settlement: None,
                }],
                ..host.view()
            };

            // The listeners are bound in the order the rows draw.
            assert_eq!(history_ids(&view), vec!["a".to_owned(), "b".to_owned()]);

            let s = strings();
            let w = wallet_strings();
            let received = tx_detail(
                &view,
                "a",
                &s,
                &w,
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
            assert_eq!(
                received.status.as_ref().map(|chip| &chip.text),
                Some(&s.status_confirmed)
            );
            assert!(received.technical.is_none(), "a transfer has none");
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

            // The sent one, whose row says pending — it must NOT wear the
            // confirmed chip.
            let sent = tx_detail(
                &view,
                "b",
                &s,
                &w,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("row b exists"));
            assert!(!sent.positive);
            assert_eq!(sent.facts[0].label, s.detail_to);
            let chip = sent
                .status
                .as_ref()
                .unwrap_or_else(|| unreachable!("a transfer wears a chip"));
            assert_eq!(chip.text, s.status_pending);
            assert!(matches!(chip.tone, StatusTone::Info));

            // Privacy masks the figure here as everywhere.
            let hidden = tx_detail(
                &view,
                "a",
                &s,
                &w,
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
                    &w,
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

    /// Spec 093, through the REAL core, in Chinese: the swap opens to its
    /// row's title and figure, its chip, and the core's facts in the core's
    /// order — the site, the network wearing the coin, the contract by its
    /// built-in name (copyable as its address), the sheet's balance changes,
    /// the date — with the explorer link its hash gives, and "Technical
    /// details" folded: the operation, the call data the record kept (read
    /// only now), the transaction's hash and the operation's.
    #[test]
    fn a_swap_opens_to_the_cores_facts_and_technical_details() {
        let loc = crate::loc::Loc::for_language("zh");
        let (s, w) = (
            FlowStrings::resolve(&loc),
            crate::wallet::WalletStrings::resolve(&loc),
        );
        let view = crate::wallet::fixtures::core_feed(
            crate::wallet::fixtures::dapp_activity_records(1_756_000_000.0),
        );
        let open = |hidden: bool| {
            tx_detail(
                &view,
                "dapp-1-swap",
                &s,
                &w,
                hidden,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the swap is in the feed"))
        };
        let detail = open(false);
        assert_eq!(detail.title.as_ref(), "在 Uniswap 兑换");
        assert_eq!(detail.amount.as_ref(), "≈ \u{2212}100 USDC");
        assert!(!detail.danger);
        assert_eq!(
            detail.status.as_ref().map(|chip| &chip.text),
            Some(&s.status_confirmed)
        );
        let facts: Vec<(&str, &str)> = detail
            .facts
            .iter()
            .map(|fact| (fact.label.as_ref(), fact.value.as_ref()))
            .collect();
        assert_eq!(facts[0], ("应用", "app.uniswap.org"));
        assert_eq!(facts[1].0, "网络");
        assert_eq!(facts[2], ("合约", "Uniswap Universal Router"));
        assert_eq!(facts[3], ("余额变化", "≈ \u{2212}100 USDC\n≈ +0.03 ETH"));
        assert_eq!(facts[4].0, "日期");
        assert_eq!(facts.len(), 5);
        assert_eq!(
            detail.facts[2].copy.as_deref(),
            Some("0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad")
        );
        // The network line wears the NETWORK's mark (the core's kind rule):
        // Ethereum's own logo over its coin's letters, no badge — never the
        // swapped coin's.
        match &detail.facts[1].lead {
            FactLead::Token(mark) => {
                assert_eq!(mark.ticker.as_ref(), "ETH");
                assert_eq!(mark.logos, crate::marks::chain_logos(1));
            }
            _ => unreachable!("the network line leads with the network's mark"),
        }
        assert!(detail.explorer_url.as_ref().is_some_and(|url| {
            url.ends_with("0x9f2c4e5d6a7b8c9d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f")
        }));
        // Privacy masks the figures and keeps the rest.
        let hidden = open(true);
        assert_eq!(
            hidden.facts[3].value.as_ref(),
            "≈ \u{2212}•••• USDC\n≈ +•••• ETH"
        );

        // Folded until opened; opened, the core's lines in its order.
        let technical = detail
            .technical
            .as_ref()
            .unwrap_or_else(|| unreachable!("a dApp record has technical details"));
        assert_eq!(technical.toggle.as_ref(), "技术细节");
        assert!(
            technical.lines.is_none(),
            "nothing read before it is opened"
        );
        let mut opened = detail.clone();
        open_technical(
            &mut opened,
            &view,
            "dapp-1-swap",
            Some(r#"[{"to":"0x3fc9","data":"0x3593564c"}]"#),
            &s,
            &w,
            "zh-CN",
        );
        let lines = opened
            .technical
            .and_then(|technical| technical.lines)
            .unwrap_or_default();
        let drawn: Vec<(String, String)> = lines
            .iter()
            .map(|line| match line {
                crate::flows::fixtures::TechnicalLine::Fact(fact) => {
                    (fact.label.to_string(), fact.value.to_string())
                }
                crate::flows::fixtures::TechnicalLine::Text { label, text, .. } => {
                    (label.to_string(), text.to_string())
                }
            })
            .collect();
        assert_eq!(drawn[0], ("操作".to_owned(), "合约交互".to_owned()));
        assert_eq!(drawn[1].0, "调用数据");
        assert!(
            drawn[1].1.contains("\"data\": \"0x3593564c\""),
            "{}",
            drawn[1].1
        );
        assert_eq!(drawn[2], ("哈希".to_owned(), "0x9f2c…6e7f".to_owned()));
        assert_eq!(
            drawn[3],
            ("UserOp 哈希".to_owned(), "0x5c1e…d3e5".to_owned())
        );
        assert_eq!(lines.len(), 4);
    }

    /// Spec 097, through the real core: the pass's Aave borrow, closed by
    /// the tracker with what its receipt proved, opens to the USDC it
    /// brought in (no "−" for nothing, no fiat for an unknown price); the
    /// refused withdraw says why under its chip; the 1inch order's BNB has no
    /// price — no fiat, never "$0.00".
    #[test]
    fn a_landed_or_failed_dapp_row_says_what_the_chain_proved() {
        use vela_core::app::activity_feed::{FeedTxKind, FeedTxRecord};
        use vela_core::app::dapp_activity::{DappAction, DappSummary};
        use vela_core::app::tx_tracker::{TrackFailure, TrackMove, TrackSettlement};
        let pool = "0x6807dc923806fe8fd134338eabca509979a7e0cb";
        let record = |id: &str, at: f64, status: FeedTxStatus| FeedTxRecord {
            id: id.to_owned(),
            user_op_hash: format!("0x{:0>64}", id.len()),
            tx_hash: String::new(),
            from: "0xme".to_owned(),
            to: pool.to_owned(),
            to_name: None,
            value: "0x0".to_owned(),
            symbol: "BNB".to_owned(),
            decimals: 18,
            logo_urls: None,
            chain_id: 56,
            timestamp: at,
            day_start_ms: 0.0,
            status,
            kind: Some(FeedTxKind::DappTx),
            usd: None,
            dapp_url: Some("https://app.aave.com".to_owned()),
            intent: Some("Borrow".to_owned()),
            balance_changes: None,
            calldata: Some(true),
            call_data: None,
            summary: Some(DappSummary {
                action: DappAction::Call,
                calls: 1,
                contract: Some(pool.to_owned()),
                ..DappSummary::default()
            }),
            settlement: None,
        };
        let mut borrow = record("dapp-1-tx", 1_756_000_000.0, FeedTxStatus::Confirmed);
        borrow.tx_hash = format!("0x{}", "cb".repeat(32));
        borrow.settlement = Some(TrackSettlement {
            moved: Some(vec![
                TrackMove {
                    token: Some("0xcdbbed5606d9c5c98eeedd67933991dc17f0c68d".to_owned()),
                    delta: "300000000000000001".to_owned(),
                },
                TrackMove {
                    token: Some("0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d".to_owned()),
                    delta: "300000000000000000".to_owned(),
                },
            ]),
            failure: None,
        });
        // The scan's "Received" of the same 0.3 USDC names the coin.
        let received = FeedTxRecord {
            id: "rx-1".to_owned(),
            user_op_hash: String::new(),
            from: pool.to_owned(),
            to: "0xme".to_owned(),
            value: "0.3".to_owned(),
            symbol: "USDC".to_owned(),
            kind: Some(FeedTxKind::Receive),
            dapp_url: None,
            intent: None,
            calldata: None,
            summary: None,
            ..borrow.clone()
        };
        let mut withdraw = record("dapp-2-tx", 1_756_000_100.0, FeedTxStatus::Failed);
        withdraw.intent = Some("Withdraw".to_owned());
        withdraw.settlement = Some(TrackSettlement {
            moved: None,
            failure: Some(TrackFailure::Refused),
        });
        let mut order = record("dapp-3-tx", 1_756_000_200.0, FeedTxStatus::Confirmed);
        order.value = "0xaa87bee538000".to_owned();
        order.intent = Some("create order".to_owned());
        order.dapp_url = Some("https://1inch.com".to_owned());
        // What the sheet's reading named (the descriptor service's 1inch).
        let factory = "0xe12e0f117d23a5ccc57f8935cd8c4e80cd91ff01";
        order.to = factory.to_owned();
        order.summary = Some(DappSummary {
            action: DappAction::Call,
            calls: 1,
            contract: Some(factory.to_owned()),
            contract_name: Some("NativeOrderFactory".to_owned()),
            owner: Some("1inch".to_owned()),
            ..DappSummary::default()
        });
        let view = crate::wallet::fixtures::core_feed(vec![borrow, received, withdraw, order]);
        // English, whatever another test left the process locale at.
        let loc = crate::loc::Loc::for_language("en");
        let (s, w) = (
            FlowStrings::resolve(&loc),
            crate::wallet::WalletStrings::resolve(&loc),
        );
        let open = |id: &str| {
            tx_detail(
                &view,
                id,
                &s,
                &w,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("{id} is in the feed"))
        };
        let borrow = open("dapp-1-tx");
        assert_eq!(borrow.title.as_ref(), "Borrow on Aave");
        assert_eq!(borrow.amount.as_ref(), "+0.3 USDC");
        assert!(borrow.positive);
        assert_eq!(borrow.fiat.as_ref(), "");
        let withdraw = open("dapp-2-tx");
        assert_eq!(
            withdraw.status.as_ref().map(|chip| chip.text.clone()),
            Some(s.status_failed.clone())
        );
        assert_eq!(withdraw.note.as_ref(), Some(&s.failed_refused));
        assert_eq!(
            withdraw.note.as_ref().map(|note| note.to_string()),
            Some("The network refused it \u{2014} nothing was sent.".to_owned())
        );
        let order = open("dapp-3-tx");
        assert_eq!(order.title.as_ref(), "Create order on 1inch");
        assert!(
            order
                .facts
                .iter()
                .any(|fact| fact.value.as_ref() == "NativeOrderFactory")
        );
        assert_eq!(order.amount.as_ref(), "\u{2212}0.003 BNB");
        assert_eq!(
            order.fiat.as_ref(),
            "",
            "no price known: no fiat, not $0.00"
        );
        let rows = crate::wallet::live::activity_rows(&view, &w, &s, false);
        let borrow_row = rows
            .iter()
            .find(|row| row.title.as_ref() == "Borrow on Aave")
            .unwrap_or_else(|| unreachable!("the borrow's row"));
        assert!(
            borrow_row.received.is_some(),
            "the folded Received still shows"
        );
    }

    /// Spec 093: a permit opens to what it granted — the allowance in the
    /// danger tone, no status chip but the off-chain note (nothing was sent),
    /// who may spend, the cap, that it never expires — and its technical
    /// details say it was a typed-data signature of a `PermitSingle`, with
    /// the record's data, or the corpus's "not recorded" when it kept none.
    /// A sign-in has neither an allowance nor a figure.
    #[test]
    fn a_permit_opens_to_what_it_granted() {
        let loc = crate::loc::Loc::for_language("zh");
        let (s, w) = (
            FlowStrings::resolve(&loc),
            crate::wallet::WalletStrings::resolve(&loc),
        );
        let view = crate::wallet::fixtures::core_feed(
            crate::wallet::fixtures::dapp_activity_records(1_756_000_000.0),
        );
        let open = |id: &str| {
            tx_detail(
                &view,
                id,
                &s,
                &w,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("{id} is in the feed"))
        };
        let permit = open("dapp-2-permit");
        assert_eq!(permit.title.as_ref(), "在 Uniswap 授权签名");
        assert_eq!(permit.amount.as_ref(), "无限额 USDC");
        assert!(permit.danger);
        assert!(permit.status.is_none(), "a signature settles nothing");
        assert_eq!(permit.note.as_ref(), Some(&s.detail_off_chain));
        assert!(permit.explorer_url.is_none());
        let facts: Vec<(&str, &str, bool)> = permit
            .facts
            .iter()
            .map(|fact| (fact.label.as_ref(), fact.value.as_ref(), fact.danger))
            .collect();
        assert_eq!(facts[0], ("应用", "app.uniswap.org", false));
        assert_eq!(facts[1].0, "网络");
        assert_eq!(facts[2], ("被授权方", "Uniswap Universal Router", false));
        assert_eq!(facts[3], ("授权上限", "无限额 USDC", true));
        assert_eq!(facts[4], ("过期时间", "该授权永不过期", false));
        assert_eq!(facts[5].0, "日期");
        assert_eq!(facts.len(), 6);

        let lines_with = |request: Option<&str>| {
            let mut opened = permit.clone();
            open_technical(
                &mut opened,
                &view,
                "dapp-2-permit",
                request,
                &s,
                &w,
                "zh-CN",
            );
            opened
                .technical
                .and_then(|technical| technical.lines)
                .unwrap_or_default()
                .into_iter()
                .map(|line| match line {
                    crate::flows::fixtures::TechnicalLine::Fact(fact) => {
                        (fact.label.to_string(), fact.value.to_string(), false)
                    }
                    crate::flows::fixtures::TechnicalLine::Text {
                        label,
                        text,
                        missing,
                    } => (label.to_string(), text.to_string(), missing),
                })
                .collect::<Vec<_>>()
        };
        let kept = lines_with(Some(
            r#"["0x88cCA0EeDbF2C4426110bbFc998F048689266894","{\"primaryType\":\"PermitSingle\"}"]"#,
        ));
        assert_eq!(
            kept[0],
            ("操作".to_owned(), "结构化数据签名".to_owned(), false)
        );
        assert_eq!(kept[1].0, "签名数据");
        assert!(!kept[1].2);
        // The document inside the string, laid out as JSON.
        assert!(
            kept[1].1.contains("\"primaryType\": \"PermitSingle\""),
            "{}",
            kept[1].1
        );
        assert_eq!(
            kept[2],
            ("类型".to_owned(), "PermitSingle".to_owned(), false)
        );
        assert_eq!(kept.len(), 3, "a signature has no hash of either kind");
        // Nothing kept — no record, or the core's `""` for a request whose
        // shape alone was past the cut — says so.
        for kept in [None, Some(""), Some("  ")] {
            let none = lines_with(kept);
            assert_eq!(
                none[1],
                ("签名数据".to_owned(), s.content_missing.to_string(), true),
                "{kept:?}"
            );
        }

        let sign_in = open("dapp-3-siwe");
        assert_eq!(sign_in.title.as_ref(), "在 app.uniswap.org 登录");
        assert_eq!((sign_in.amount.as_ref(), sign_in.fiat.as_ref()), ("", ""));
        assert!(sign_in.status.is_none() && sign_in.note.is_some());
        let labels: Vec<&str> = sign_in
            .facts
            .iter()
            .map(|fact| fact.label.as_ref())
            .collect();
        assert_eq!(labels, vec!["应用", "网络", "日期"]);

        // A message's content is the core's reading of it: its words, not
        // its hex.
        let mut opened = sign_in.clone();
        open_technical(
            &mut opened,
            &view,
            "dapp-3-siwe",
            Some(r#"["0x48656c6c6f2c20776f726c64","0x88cCA0EeDbF2C4426110bbFc998F048689266894"]"#),
            &s,
            &w,
            "zh-CN",
        );
        let message = opened
            .technical
            .and_then(|technical| technical.lines)
            .unwrap_or_default()
            .into_iter()
            .find_map(|line| match line {
                crate::flows::fixtures::TechnicalLine::Text {
                    label,
                    text,
                    missing,
                } => Some((label.to_string(), text.to_string(), missing)),
                crate::flows::fixtures::TechnicalLine::Fact(_) => None,
            });
        assert_eq!(
            message,
            Some(("消息".to_owned(), "Hello, world".to_owned(), false))
        );
    }

    /// 083 F2: every transaction's hash fits the panel — shortened like the
    /// addresses around it, the whole of it on the copy button. Drawn whole,
    /// its 66 characters ran off the column's right edge on one line.
    #[test]
    fn every_transaction_detail_fits_its_hash() {
        use vela_core::app::activity_feed::{
            ActivityFeed, Event as FeedEvent, FeedDirection, FeedItem,
        };

        const HASH: &str = "0x9f2c4e5d6a7b8c9d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f";
        let mut host = CoreHost::<ActivityFeed>::new();
        let _ = host.dispatch(FeedEvent::AccountSwitched {
            address: "0xme".to_owned(),
        });
        let row = |id: &str, incoming: bool| FeedRow::Item {
            item: FeedItem {
                id: id.to_owned(),
                direction: if incoming {
                    FeedDirection::In
                } else {
                    FeedDirection::Out
                },
                counterparty: Some("0xAbCdEf0000000000000000000000000000000001".to_owned()),
                alias: None,
                value: Some("1.5".to_owned()),
                symbol: "USDC".to_owned(),
                decimals: Some(6),
                usd_value: 1.5,
                chain_id: 8453,
                timestamp: 1_759_100_000.0,
                day_start_ms: 0.0,
                tx_hash: Some(HASH.to_owned()),
                batch: None,
                kind: vela_core::app::activity_feed::FeedTxKind::DappTx,
                status: vela_core::app::activity_feed::FeedTxStatus::Confirmed,
                site: None,
                counterparty_role: Default::default(),
                dapp: None,
                subtitle: Vec::new(),
                priced: true,
            },
        };
        let view = FeedView {
            rows: vec![row("received", true), row("sent", false)],
            ..host.view()
        };
        let (s, w) = (strings(), wallet_strings());
        for id in ["received", "sent"] {
            let detail = tx_detail(
                &view,
                id,
                &s,
                &w,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists"));
            let hash = detail
                .facts
                .iter()
                .find(|fact| fact.label == s.detail_hash)
                .unwrap_or_else(|| unreachable!("{id} has a hash row"));
            assert_eq!(hash.value.as_ref(), "0x9f2c…6e7f", "{id}");
            assert!(hash.mono, "still compared character by character");
            assert_eq!(hash.copy.as_ref().map(AsRef::as_ref), Some(HASH), "{id}");
            // The explorer still opens the whole of it.
            assert!(
                detail
                    .explorer_url
                    .as_ref()
                    .is_some_and(|url| url.ends_with(HASH)),
                "{id}"
            );
        }
    }

    /// Spec 093: a tap opens the technical details on the record shown,
    /// reading its stored request then — once; a second tap folds them
    /// without reading; a tap on another record opens that one's.
    #[test]
    fn the_technical_details_read_the_store_only_when_opened() {
        let reads = std::cell::Cell::new(0);
        let read = |id: &str| {
            reads.set(reads.get() + 1);
            Some(format!("request of {id}"))
        };
        let opened = technical_toggled(None, "a".to_owned(), read);
        assert_eq!(
            opened,
            Some(("a".to_owned(), Some("request of a".to_owned())))
        );
        assert_eq!(reads.get(), 1);
        assert_eq!(
            technical_toggled(opened.clone(), "a".to_owned(), read),
            None
        );
        assert_eq!(reads.get(), 1, "folding reads nothing");
        assert_eq!(
            technical_toggled(opened, "b".to_owned(), read),
            Some(("b".to_owned(), Some("request of b".to_owned())))
        );
        assert_eq!(reads.get(), 2);
    }

    /// 083 F1 through the core (spec 093): a token the sheet could not verify
    /// is listed among the balance changes by name and direction only, never
    /// with a number; a dApp row whose record kept no lines lists none.
    #[test]
    fn a_dapp_detail_lists_an_unverified_token_without_a_figure() {
        use vela_core::app::token_trust::TrustSimJudgment;
        let mut records = crate::wallet::fixtures::dapp_activity_records(1_756_000_000.0);
        if let Some(changes) = records[0].balance_changes.as_mut() {
            changes.push(TrustSimJudgment::Erc20Unverified {
                token: Some("0x00000000000000000000000000000000000bad01".to_owned()),
                delta: "1000000000000000000000".to_owned(),
            });
        }
        let mut bare = records[0].clone();
        bare.id = "dapp-0-bare".to_owned();
        bare.timestamp -= 600.0;
        bare.balance_changes = None;
        bare.tx_hash =
            "0x1111111111111111111111111111111111111111111111111111111111111111".to_owned();
        records.push(bare);
        let view = crate::wallet::fixtures::core_feed(records);
        let (s, w) = (strings(), wallet_strings());
        let open = |id: &str| {
            tx_detail(
                &view,
                id,
                &s,
                &w,
                false,
                "en-US",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("{id} is in the feed"))
        };
        let swap = open("dapp-1-swap");
        let changes = swap
            .facts
            .iter()
            .find(|fact| fact.label == s.detail_changes)
            .unwrap_or_else(|| unreachable!("the swap lists what it moved"));
        assert_eq!(
            changes.value.lines().last(),
            Some(format!("+ {}", s.detail_unverified_token).as_str())
        );
        let bare = open("dapp-0-bare");
        assert!(
            bare.facts.iter().all(|fact| fact.label != s.detail_changes),
            "no lines kept, none listed"
        );
        assert!(bare.breakdown.is_empty() && bare.breakdown_title.is_none());
    }

    /// Spec 090: the switch under the code is the core's — off by default with
    /// the bare address; on, the code names the network on screen, the calm
    /// hint rides under it, and Vela's own scanner reads the code back to the
    /// same address and chain.
    #[test]
    fn the_network_switch_is_the_cores_and_its_code_scans_back() {
        use vela_core::app::payment_request::{Event as PayEvent, PaymentRequest};
        crate::executor::storage::tests::with_temp_state("flows-qr-network", || {
            const ADDR: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let quiet = ReceiveWatchView {
                detected: false,
                deposits: Vec::new(),
            };
            let s = strings();
            let mut host = CoreHost::<PaymentRequest>::new();
            let _ = host.dispatch(PayEvent::Start {
                account: ADDR.to_owned(),
                recipient: ADDR.to_owned(),
                base_url: "https://getvela.app".to_owned(),
            });
            let _ = host.dispatch(PayEvent::AssetPicked {
                chain_id: 100,
                token_address: None,
                symbol: "xDAI".to_owned(),
                decimals: 18,
                network_name: "Gnosis".to_owned(),
            });
            let draw = |pay: &PaymentRequestView| {
                receive_qr(
                    ADDR,
                    "Me",
                    100,
                    &quiet,
                    pay,
                    &s,
                    "en-US",
                    crate::wallet::live::Money::usd(),
                )
            };

            let off = draw(&host.view());
            let switch = off.network.as_ref().expect("the switch is offered");
            assert!(!switch.on);
            assert!(switch.hint.is_none());
            assert_eq!(switch.label, s.include_network);
            assert_eq!(off.qr_payload.as_deref(), Some(ADDR));

            let _ = host.dispatch(PayEvent::IncludeNetworkChanged { include: true });
            let on = draw(&host.view());
            let switch = on.network.as_ref().expect("the switch is offered");
            assert!(switch.on);
            assert_eq!(switch.hint.as_ref(), Some(&s.include_network_hint));
            let code = on.qr_payload.as_deref().unwrap_or_default();
            assert_eq!(code, format!("ethereum:{ADDR}@100"));
            let scanned = crate::flows::eip681::parse(code).expect("Vela's scanner reads it");
            assert_eq!(scanned.recipient, ADDR);
            assert_eq!(scanned.chain_id, Some(100));
            assert_eq!(scanned.token_address, None);
            assert_eq!(scanned.amount_base_units, None);
            // People paste addresses: copy is still the bare one.
            assert_eq!(host.view().copy_payload, ADDR);

            // Request mode's code always names its network: no switch there.
            let _ = host.dispatch(PayEvent::ModeChanged {
                mode: vela_core::app::payment_request::Mode::Request,
            });
            assert!(draw(&host.view()).network.is_none());
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

    /// A group pick carries each member's name — the one the person gave —
    /// so the split rows say WHO, not just where.
    ///
    /// Spec 097 F: never the name the book RESOLVED ("bo.eth" below). A split
    /// row's name is the person's own word — the core draws it untagged — and
    /// a resolved name is the registry's or a name service's, anyone's word
    /// for that address; seeded as a row name it passed for the person's own.
    /// That member's row is the bare address now, which the confirm sets in
    /// mono.
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
            import_failure_key: None,
            export: None,
            recipient: None,
        };
        let groups = contact_group_members(&view);
        assert_eq!(groups.len(), 1);
        let names: Vec<Option<&str>> = groups[0].iter().map(|r| r.name.as_deref()).collect();
        assert_eq!(names, vec![Some("Ana"), None, None]);
        assert!(
            groups[0]
                .iter()
                .all(|r| r.amount.is_empty() && r.id.is_empty()),
            "amounts are the person's to type, ids the core's to give"
        );
        assert_eq!(groups[0][2].address, "0xcc");
    }

    /// Issue 467: every row of the picker carries the WHOLE address of the
    /// contact it draws — what its press picks — in the core's order, so
    /// a re-sort moves the address with its row instead of handing row N's
    /// press to whoever is Nth now.
    #[test]
    fn every_contact_row_carries_its_own_address() {
        use vela_core::app::contacts::{Contact, ContactKind, ContactSource};
        let contact = |address: &str, name: Option<&str>| Contact {
            address: address.to_owned(),
            name: name.map(str::to_owned),
            resolved_name: None,
            resolved_source: None,
            kind: ContactKind::Eoa,
            favorite: false,
            note: None,
            tx_count: 0,
            last_used_ms: 0.0,
            first_seen_ms: 0.0,
            source: ContactSource::Manual,
        };
        let book = |contacts: Vec<Contact>| ContactsView {
            loaded: true,
            sections: Vec::new(),
            contacts,
            groups: Vec::new(),
            last_import: None,
            import_failure: None,
            import_failure_key: None,
            export: None,
            recipient: None,
        };
        let ana = "0xaaaa000000000000000000000000000000000001";
        let bo = "0xbbbb000000000000000000000000000000000002";
        let s = strings();
        let rows = |view: &ContactsView| -> Vec<(String, String)> {
            contact_pick(view, &s)
                .contacts
                .iter()
                .map(|row| (row.name.to_string(), row.address_full.to_string()))
                .collect()
        };
        let before = book(vec![contact(ana, Some("Ana")), contact(bo, Some("Bo"))]);
        assert_eq!(
            rows(&before),
            vec![
                ("Ana".to_owned(), ana.to_owned()),
                ("Bo".to_owned(), bo.to_owned())
            ]
        );
        // The core re-sorted (Bo was just paid): each name keeps its address.
        let after = book(vec![contact(bo, Some("Bo")), contact(ana, Some("Ana"))]);
        assert_eq!(
            rows(&after),
            vec![
                ("Bo".to_owned(), bo.to_owned()),
                ("Ana".to_owned(), ana.to_owned())
            ]
        );
        // The row shows the short form; the press has the whole one.
        let row = &contact_pick(&after, &s).contacts[0];
        assert_ne!(row.address, row.address_full);
        assert_eq!(row.address_full.as_ref(), bo);
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

        let loading = history_panel(&feed, true, &s, &w, false);
        assert!(loading.loading && loading.empty.is_none());

        let empty = history_panel(&feed, false, &s, &w, false);
        assert!(!empty.loading);
        assert_eq!(empty.empty, Some(s.history_empty.clone()));

        // Narrowed: the core says which line (spec 082 RG5), from the filter
        // it was told.
        let _ = host.dispatch(FeedEvent::ChainFilterChanged {
            chain_id: Some(100),
        });
        let narrowed_feed = FeedView {
            rows: Vec::new(),
            ..host.view()
        };
        let narrowed = history_panel(&narrowed_feed, false, &s, &w, false);
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
            kind: vela_core::app::activity_feed::FeedTxKind::Receive,
            status: vela_core::app::activity_feed::FeedTxStatus::Confirmed,
            site: None,
            counterparty_role: Default::default(),
            dapp: None,
            subtitle: Vec::new(),
            priced: false,
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
            relay_sent_at_ms: None,
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

    /// Open, three options fastest first, each its own fee and its gas bid
    /// from the core — and nothing on what a speed buys: in the picker for
    /// one transaction the figures say it (the type has no such field).
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

    /// A USDC fee keeps USDC's own logo while a newly picked speed is
    /// measured. The coin does not change with the speed; the row used to
    /// take the coin's NAME from the estimate in hand and its CONTRACT from
    /// the speed's own estimate — none, for that moment — so "USDC" fell to
    /// the native coin's rule and wore the chain's logo.
    #[test]
    fn the_fee_coin_keeps_its_logo_while_another_speed_is_measured() {
        crate::executor::storage::tests::with_temp_state("flows-fee-coin-mark", || {
            let (s, wallet) = strings();
            let usdc = "0xddafbb505ad214d7b80b1f830fccc89b60fb7a83";
            let mut paid_in_usdc = quote(FeeTier::Fast, "10000", None);
            paid_in_usdc.fee_asset = FeeAssetView::Erc20 {
                token: usdc.to_owned(),
                decimals: 6,
                amount: "1".to_owned(),
                symbol: Some("USDC".to_owned()),
            };
            let mut send = CoreHost::<SendMachine>::new().view();
            send.fee = Some(paid_in_usdc);
            let fee = CoreHost::<FeePolicy>::new().view();
            let usdc_logos = crate::marks::token_logos(100, "USDC", Some(usdc), &[]);
            assert!(usdc_logos.logo_urls[0].contains("/assets/eip155-100/"));
            // Fast is the estimate's own speed; Slow is a pick still measuring.
            for tier in [FeeTier::Fast, FeeTier::Slow] {
                let picked = speed(tier, false, false, None, &[]);
                let row = send_fee_row(&inputs(&send, &fee, &s, &wallet, Some(&picked)));
                assert_eq!(row.mark.ticker.as_ref(), "USDC");
                assert_eq!(row.mark.logos, usdc_logos, "{tier:?}");
            }
        });
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
            relay_sent_at_ms: None,
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
        assert_eq!(model.rows[0].amount, "689.66 USDT");
        assert_eq!(model.rows[0].source.as_deref(), Some("5000 CNY"));
        assert_eq!(model.parsed, fill(&s.batch_parsed, "n", "4"), "lines read");
        assert!(
            model.rows[1].amount.is_empty() && model.rows[1].seed.is_none(),
            "a refused line has no figure and nobody to draw"
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

/// Spec 097 F (real-money pass, S2 + S3): the page that signs names the
/// ADDRESS beside any name, saying whose word the name is; the receipt lists
/// every coin the operation sent. In English, whatever the machine speaks.
#[cfg(test)]
mod payee_tests {
    use super::*;
    use crate::core_host::CoreHost;
    use vela_core::app::send::{
        Event as SendEvent, Send as SendMachine, SendAccountRef, SendChainInfo, SendDisplayContext,
        SendOpenParams, SendOperation, SendReceiptKind, SendReceiptView, SendRecipientIdentity,
        SendShellResult,
    };

    /// The developer wallet of the real-money pass, which the public
    /// registry calls "Wallet".
    const DEV_WALLET: &str = "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c";
    /// One of the person's own accounts.
    const SAVINGS: &str = "0x031d7D57c99CAF891e1C250554691Fd12D84772b";
    const BOB: &str = "0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141";
    /// What the desktop's resolver answers for each, `(address, name,
    /// source)` — its own source labels (`executor::identity`).
    const NAMES: [(&str, &str, &str); 3] = [
        (DEV_WALLET, "Wallet", "passkey"),
        (SAVINGS, "Savings", "self"),
        (BOB, "bob.eth", "ENS"),
    ];

    fn en() -> (FlowStrings, crate::wallet::WalletStrings) {
        let loc = crate::loc::Loc::for_tag("en");
        (
            FlowStrings::resolve(&loc),
            crate::wallet::WalletStrings::resolve(&loc),
        )
    }

    /// Run `f` over the screens' inputs for `send`, in English.
    fn with_en<R>(send: &SendView, f: impl FnOnce(&SendInputs<'_>, &FlowStrings) -> R) -> R {
        let (s, wallet) = en();
        let fee = CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        let inputs = SendInputs {
            send,
            fee: &fee,
            s: &s,
            wallet: &wallet,
            locale: "en",
            money: crate::wallet::live::Money::usd(),
            identity_name: "Golden",
            identity_address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894",
            speed: None,
            relay_sent_at_ms: None,
        };
        f(&inputs, &s)
    }

    /// `event` into the real machine, each question it asks answered inline:
    /// the token list, and who each address is from [`NAMES`]. The timers
    /// and the fee quote stay outstanding, as `wallet::money`'s harness
    /// leaves them — nothing here is about the fee.
    fn drive(host: &mut CoreHost<SendMachine>, event: SendEvent) {
        let mut pending = host.dispatch(event);
        while let Some(effect) = pending.pop() {
            let result = match &effect.operation {
                SendOperation::FetchTokens { .. } => SendShellResult::TokensLoaded {
                    tokens: Some(vec![xdai()]),
                    chains: vec![SendChainInfo {
                        chain_id: 100,
                        network: "gnosis".to_owned(),
                        native_symbol: "xDAI".to_owned(),
                    }],
                },
                SendOperation::ResolveIdentity { address } => SendShellResult::IdentityResolved {
                    identity: NAMES
                        .iter()
                        .find(|(known, ..)| known.eq_ignore_ascii_case(address))
                        .map(|(_, name, source)| SendRecipientIdentity {
                            name: Some((*name).to_owned()),
                            source: Some((*source).to_owned()),
                        }),
                },
                SendOperation::ResolveRisk { .. } => SendShellResult::RiskResolved { risk: None },
                SendOperation::SimulateCalls { .. } => {
                    SendShellResult::SimResolved { sim_json: None }
                }
                SendOperation::LoadAccountCredential { .. } => SendShellResult::AccountCredential {
                    public_key_hex: Some("04aa".to_owned()),
                },
                SendOperation::ShowAlert { .. } => SendShellResult::AlertAcknowledged,
                SendOperation::PrewarmFees { .. } => SendShellResult::FeesPrewarmed,
                _ => continue,
            };
            pending.extend(host.resolve(effect.id, result));
        }
    }

    fn xdai() -> SendToken {
        SendToken {
            network: "gnosis".to_owned(),
            chain_id: 100,
            symbol: "xDAI".to_owned(),
            balance: "5".to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: Some(1.0),
            logo_urls: Vec::new(),
            spam: false,
        }
    }

    /// A real send form on xDAI, paying `recipient`.
    fn form_paying(recipient: &str) -> CoreHost<SendMachine> {
        let mut host = CoreHost::<SendMachine>::new();
        drive(
            &mut host,
            SendEvent::Open {
                account: Some(SendAccountRef {
                    id: "cred0".to_owned(),
                    address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
                    name: None,
                }),
                params: SendOpenParams::default(),
                display: SendDisplayContext::default(),
            },
        );
        drive(
            &mut host,
            SendEvent::SelectToken {
                token_id: xdai().id(),
            },
        );
        drive(
            &mut host,
            SendEvent::SetRecipient {
                recipient: recipient.to_owned(),
            },
        );
        host
    }

    fn to_row(facts: &[FactRow], s: &FlowStrings) -> Option<FactRow> {
        facts.iter().find(|fact| fact.label == s.to_label).cloned()
    }

    /// S2: the confirm's To row said "Wallet" — a name anyone can register
    /// in the public registry — with the address one tap away, and the
    /// form's line printed the resolver's raw source ("Wallet · passkey").
    /// Through the real machine, all three places now say the name is the
    /// registry's, and the two rows carry the address under it.
    #[test]
    fn a_registry_name_is_tagged_and_never_stands_for_the_address() {
        crate::executor::storage::tests::with_temp_state("payee-registry", || {
            let view = form_paying(DEV_WALLET).view();
            with_en(&view, |i, s| {
                let note = send_form(i)
                    .recipient_note
                    .unwrap_or_else(|| unreachable!("the form named nobody"));
                assert_eq!(note.as_ref(), "Wallet · Vela User");
                assert!(!note.contains("passkey"), "{note}");

                let confirm = send_confirm(i);
                let to = to_row(&confirm.facts, s).unwrap_or_else(|| unreachable!("no To row"));
                // The name may be cut; the tag rides with the address on the
                // line that never is.
                assert_eq!(to.value.as_ref(), "Wallet");
                assert_eq!(to.detail.as_deref(), Some("Vela User · 0x14fB…eA5c"));
                // Issue #423: the first line is the NAME — "Vela User" is
                // whose word it is, on the line under it, never the name.
                assert_ne!(to.value.as_ref(), s.vela_user.as_ref());
                assert!(!to.mono, "a name is words, not an address");
                assert!(
                    matches!(to.lead, FactLead::Identicon(ref seed) if seed.as_ref() == DEV_WALLET),
                    "the identicon opens the address in full"
                );

                let pick = pick_recipient(i).unwrap_or_else(|| unreachable!("no To line"));
                assert_eq!(pick.value, to.value);
                assert_eq!(pick.detail, to.detail);
            });
        });
    }

    /// The person's own name for an account is drawn as they wrote it — no
    /// tag — and the address is still under it; a name service's name
    /// carries the service's own label.
    #[test]
    fn an_own_name_is_untagged_and_a_service_name_says_which() {
        crate::executor::storage::tests::with_temp_state("payee-own", || {
            let view = form_paying(SAVINGS).view();
            with_en(&view, |i, s| {
                assert_eq!(
                    send_form(i).recipient_note.as_deref(),
                    Some("Savings"),
                    "the person's own word, untagged"
                );
                let to =
                    to_row(&send_confirm(i).facts, s).unwrap_or_else(|| unreachable!("no To row"));
                assert_eq!(to.value.as_ref(), "Savings");
                assert_eq!(to.detail.as_deref(), Some("0x031d…772b"));
            });

            let view = form_paying(BOB).view();
            with_en(&view, |i, s| {
                let to =
                    to_row(&send_confirm(i).facts, s).unwrap_or_else(|| unreachable!("no To row"));
                assert_eq!(to.value.as_ref(), "bob.eth");
                assert_eq!(to.detail.as_deref(), Some("ENS · 0x7687…D141"));
            });
        });
    }

    /// A split's confirm names each row as the core names its payee — the
    /// person's own name over the short address, an unnamed row by its
    /// address in mono — and has NO single To row: the address typed before
    /// the rows were seeded is still in `recipient`, and the confirm used to
    /// print it as whom the split paid.
    #[test]
    fn a_split_confirm_names_each_row_and_has_no_single_recipient() {
        crate::executor::storage::tests::with_temp_state("payee-split", || {
            let mut host = form_paying(DEV_WALLET);
            let row = |address: &str, name: Option<&str>, amount: &str| SendRecipientDraft {
                id: String::new(),
                address: address.to_owned(),
                amount: amount.to_owned(),
                name: name.map(str::to_owned),
            };
            drive(
                &mut host,
                SendEvent::SeedSplitRecipients {
                    recipients: vec![row(SAVINGS, Some("Mum"), "1"), row(DEV_WALLET, None, "2")],
                },
            );
            let view = host.view();
            assert!(view.split_mode);
            assert_eq!(view.recipient, DEV_WALLET, "the stale single recipient");
            with_en(&view, |i, s| {
                let confirm = send_confirm(i);
                assert!(
                    to_row(&confirm.facts, s).is_none(),
                    "a split pays its rows, not `recipient`"
                );
                let rows = &confirm.breakdown;
                assert_eq!(rows.len(), 2);
                assert_eq!(rows[0].label.as_ref(), "Mum");
                assert_eq!(rows[0].detail.as_deref(), Some("0x031d…772b"));
                assert!(!rows[0].mono);
                assert_eq!(rows[0].seed.as_deref(), Some(SAVINGS));
                // The registry's "Wallet" is not a row's name: a row is named
                // only in the person's own word.
                assert_eq!(rows[1].label.as_ref(), "0x14fB…eA5c");
                assert!(rows[1].mono, "an address alone is set in mono");
                assert!(rows[1].detail.is_none());
                assert_eq!(rows[1].value.as_ref(), "2 xDAI");
                // Nothing on the page that signs carries the registry's name.
                assert!(
                    confirm
                        .facts
                        .iter()
                        .all(|fact| !fact.value.contains("Wallet")),
                    "no stale To row"
                );
            });
        });
    }

    fn coin(amount: &str, symbol: &str, token_address: Option<&str>) -> SendReceiptCoin {
        SendReceiptCoin {
            amount: amount.to_owned(),
            symbol: symbol.to_owned(),
            logo_urls: Vec::new(),
            token_address: token_address.map(str::to_owned),
            usd_value: 0.0,
        }
    }

    /// S3: a two-coin sweep's success screen said "Sent 0.000418 ETH" while
    /// 0.034929 USDC moved in the same operation. It lists both coins under
    /// "2 assets", titled "Sent", and still says whom it went to.
    #[test]
    fn a_two_coin_sweep_receipt_lists_both_coins() {
        crate::executor::storage::tests::with_temp_state("payee-sweep-receipt", || {
            let mut view = CoreHost::<SendMachine>::new().view();
            view.multi_select_mode = true;
            view.recipient = DEV_WALLET.to_owned();
            view.recipient_identity = Some(SendRecipientIdentity {
                name: Some("Wallet".to_owned()),
                source: Some("passkey".to_owned()),
            });
            view.tx_hash = Some("0xtx".to_owned());
            let receipt = |status| SendReceiptView {
                status,
                hold_reason: None,
                kind: Some(SendReceiptKind::MultiSelect),
                transfers: Vec::new(),
                coins: vec![
                    coin("0.000418", "ETH", None),
                    coin(
                        "0.034929",
                        "USDC",
                        Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913"),
                    ),
                ],
                amount: String::new(),
                usd_value: 1.0,
                submitted_at_ms: None,
                typical_inclusion_s: None,
            };
            for status in [SendReceiptStatus::Submitted, SendReceiptStatus::Confirmed] {
                view.receipt = Some(receipt(status));
                with_en(&view, |i, s| {
                    let shown = send_receipt(i);
                    assert_eq!(shown.breakdown_title.as_deref(), Some("2 assets"));
                    let rows: Vec<(&str, &str)> = shown
                        .breakdown
                        .iter()
                        .map(|row| (row.label.as_ref(), row.value.as_ref()))
                        .collect();
                    assert_eq!(
                        rows,
                        [("ETH", "0.000418 ETH"), ("USDC", "0.034929 USDC")],
                        "{status:?}: every coin the operation sent"
                    );
                    if status == SendReceiptStatus::Confirmed {
                        assert_eq!(shown.title, s.tx_sent);
                        assert_eq!(shown.title.as_ref(), "Sent");
                        assert_ne!(shown.title.as_ref(), "Sent 0.000418 ETH");
                        // One recipient: the caption still names them, not
                        // the count of coins.
                        assert!(
                            shown.captions[0].starts_with("To Wallet"),
                            "{}",
                            shown.captions[0]
                        );
                    }
                });
            }
        });
    }

    /// One coin is still "Sent 0.5 ETH" — from the receipt's coin, not the
    /// token the form has selected now (none, on this view).
    #[test]
    fn a_one_coin_receipt_names_its_figure_and_coin() {
        crate::executor::storage::tests::with_temp_state("payee-one-coin", || {
            let mut view = CoreHost::<SendMachine>::new().view();
            view.receipt = Some(SendReceiptView {
                status: SendReceiptStatus::Confirmed,
                hold_reason: None,
                kind: None,
                transfers: Vec::new(),
                coins: vec![coin("0.5", "ETH", None)],
                amount: "0.5".to_owned(),
                usd_value: 0.0,
                submitted_at_ms: None,
                typical_inclusion_s: None,
            });
            with_en(&view, |i, _| {
                let shown = send_receipt(i);
                assert_eq!(shown.title.as_ref(), "Sent 0.5 ETH");
                assert!(shown.breakdown.is_empty(), "one coin is the title's");
            });
            // A sweep whose native line the gas reserve dropped sent one coin
            // too: the title names it, and no "1 assets" list repeats it.
            if let Some(receipt) = view.receipt.as_mut() {
                receipt.kind = Some(SendReceiptKind::MultiSelect);
            }
            with_en(&view, |i, _| {
                let shown = send_receipt(i);
                assert_eq!(shown.title.as_ref(), "Sent 0.5 ETH");
                assert!(shown.breakdown.is_empty());
                assert!(shown.breakdown_title.is_none());
            });
        });
    }
}
