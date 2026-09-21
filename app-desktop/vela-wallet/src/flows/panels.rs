//! The flow panel bodies (spec 021) — what the third column holds.
//!
//! Each function takes a body model and returns the `Div` the page drops into
//! its existing `panel_scaffold`. The scaffold owns the title, the back chevron
//! and the close button; these own only what is under them.

use gpui::{
    App, ClickEvent, Div, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};

use gpui::prelude::FluentBuilder as _;

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{self, Theme};
use crate::wallet::components::{
    activity_row, asset_row, empty_state, icon_img, skeleton_row, token_icon_logos,
};

use super::components::{
    accent_button, address_card, fact_row, fee_row, filter_chips, flow_search, ghost_button,
    inline_mark, max_chip, mono_field, network_pill, network_row, qr_card, recipient_card,
    segmented_toggle, status_chip, token_header_card,
};
use super::fixtures::{
    AddToken, AddTokenResult, AssetsPanel, BatchImport, BreakdownRow, ContactPick, CtaState,
    DepositEntry, FeeTokenPick, FlowBody, HistoryPanel, ReceiveList, ReceiveQr, ScanModal,
    SendConfirm, SendForm, SendNotice, SendPick, SendReceipt, TxDetail,
};

/// One prepared click listener. The page builds these from `cx.listener`
/// before rendering, because a panel body has no entity to listen on.
pub type Click = Box<dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static>;

/// The steps a panel can take, as listeners the page has already bound.
///
/// `None` means the affordance is inert — which is what the gallery wants, and
/// what a panel with nowhere to go should be. A chevron that leads nowhere is
/// a defect; an absent listener on a gallery chip is not.
#[derive(Default)]
pub struct PanelActions {
    /// DR1L: a row's QR icon opens that network's code.
    ///
    /// The fixture panel binds ONE listener here and gives it to the first row,
    /// because every mock row opens the same address anyway. A live panel binds
    /// `open_qr_rows` instead — the rows are real networks and each one opens
    /// its own code, so which row was clicked is now information.
    pub open_qr: Option<Click>,
    /// DR2L: "I Understand" on the pre-receive warning — the gate that has to
    /// be passed once per account before the address is handed over.
    pub acknowledge: Option<Click>,
    /// DR2L: 保存图片 — the share card as a PNG.
    pub save_image: Option<Click>,
    /// DR1L, live: one listener per network row. Empty falls back to `open_qr`.
    pub open_qr_rows: Vec<Click>,
    /// DA1L: a row opens its transaction.
    ///
    /// The fixture binds one and gives it to the first row — every mock row
    /// opens the same drawing. See `open_qr` for why a live panel needs more.
    pub open_tx: Option<Click>,
    /// DA1L, live: one listener per row, in the order the groups render.
    pub open_tx_rows: Vec<Click>,
    /// DSD1L: a row opens the send form for that token.
    pub open_send_form: Option<Click>,
    /// DSD2L: the fee row and the recipient picker.
    pub open_fee_token: Option<Click>,
    pub open_contact_pick: Option<Click>,
    /// DSD2L's recipient pills — one more payee, or a pasted list of them.
    pub add_recipient: Option<Click>,
    pub open_batch_import: Option<Click>,
    /// DT1L's "add a token by address".
    pub open_add_token: Option<Click>,
    /// DT4L, live: the empty state itself. Its caption says "tap here to see
    /// your address", so it opens Receive (the web's `.empty-tap`).
    pub open_receive: Option<Click>,
    /// DA2L, live: removes this record from the feed (the chain keeps it).
    pub delete_tx: Option<Click>,
    /// DSD2eL's scan row — the address that is on a screen, not in the book.
    pub open_scan: Option<Click>,
    /// The panel's own CTA — continue, confirm, done.
    pub advance: Option<Click>,
    /// DT3L, live: the contract-address field, editable.
    ///
    /// `None` draws the mock's read-only well — a gallery has nothing to look
    /// a contract up on, and a field that accepts typing and then does nothing
    /// is worse than one that plainly does not.
    pub address_field: Option<AddressField>,
    /// DT3L, live: "add to wallet" on the found card.
    pub add_to_wallet: Option<Click>,
    /// DSD1L, live: one listener per token row. Empty falls back to
    /// `open_send_form`, which the fixture gives to its first row.
    pub open_send_rows: Vec<Click>,
    /// SD1b, live: "select all valuable", and the CTA that either enters the
    /// sweep or confirms the tokens ticked in it.
    pub sweep_select_all: Option<Click>,
    pub send_pick_cta: Option<Click>,
    /// DSD2L, live: the amount and the recipient, editable. `None` draws the
    /// mock's static figures.
    pub amount_field: Option<AddressField>,
    pub recipient_field: Option<AddressField>,
    /// DSD2L, live: the Max chip.
    pub tap_max: Option<Click>,
    /// DSD2L, live: ⇄ — type the amount in money, or back in the token (#197).
    pub toggle_denom: Option<Click>,
    /// DSD2eL, live: one listener per contact row, in the book's order.
    pub pick_contact_rows: Vec<Click>,
    /// DSD2fL, live: one listener per fee-coin row, in the relay's order.
    pub fee_rows: Vec<Click>,
    /// DSD2cL, live: the unit toggle's two halves (fiat, token).
    pub batch_unit: Option<(Click, Click)>,
    /// DSD2cL, live: the paste box reads the clipboard when clicked — the
    /// desktop's paste, since the drawn box is not a text editor.
    pub batch_paste: Option<Click>,
    pub batch_pick_file: Option<Click>,
    pub batch_template: Option<Click>,
    /// DSD2cL, live: the rate, editable — the shown string IS the applied rate.
    pub batch_rate_field: Option<AddressField>,
    pub batch_rate_reset: Option<Click>,
    /// DSD2cL, live: the merge line's "Replace them instead" / "Add to them
    /// instead".
    pub batch_merge: Option<Click>,
    /// DSD2L / DSD3L, live: the way out the core's refusal offers — edit the
    /// amount, add the network, check the top-up again.
    pub notice_action: Option<Click>,
    /// The relay-treasury stop's "Not now" — the core's `DismissTreasurySheet`.
    pub notice_dismiss: Option<Click>,
    /// DSD2eL, live: one listener per GROUP row — a whole group seeds a split.
    pub pick_group_rows: Vec<Click>,
    /// DSD2bL, live: each split row's own amount field and its remove — in the
    /// order the rows draw, so row N edits payee N.
    pub split_amount_fields: Vec<AddressField>,
    pub remove_recipient_rows: Vec<Click>,
}

/// An editable field the page owns the state of.
pub struct AddressField {
    pub focus: FocusHandle,
    pub value: String,
    pub placeholder: SharedString,
    pub on_change: Box<dyn Fn(String, &mut Window, &mut App) + 'static>,
}

/// Wrap an element so it answers to a click, when the page bound one.
///
/// The listener is MOVED in: each affordance is rendered once per pass, and an
/// action with no listener renders as a plain element rather than as a
/// cursor-pointer that does nothing.
pub fn clickable(id: impl Into<ElementId>, action: Option<Click>, body: impl IntoElement) -> Div {
    // The wrapper stays a plain `Div` so callers can keep composing columns;
    // the identified element lives inside it, because `.id()` changes the type.
    let wrap = div().flex().flex_col();
    match action {
        Some(action) => wrap.child(
            div()
                .id(id)
                .cursor_pointer()
                .child(body)
                .on_click(move |event, window, cx| action(event, window, cx)),
        ),
        None => wrap.child(body),
    }
}

/// "View on Explorer", opening the page it names — or the plain drawn button
/// where there is none (the mocks, a transaction with no hash).
fn explorer_button(
    id: &'static str,
    theme: &Theme,
    label: SharedString,
    url: Option<&SharedString>,
) -> Div {
    let open = url.cloned().map(|url| -> Click {
        Box::new(move |_: &ClickEvent, _: &mut Window, cx: &mut App| cx.open_url(&url))
    });
    clickable(id, open, ghost_button(theme, label))
}

/// A vertical stack with the panel's own rhythm.
fn column() -> Div {
    div().flex().flex_col().gap(px(12.))
}

/// The hairline that separates rows in every list here.
/// The parts of a batch (spec 038 #D2): every recipient of a split by name
/// and avatar, every asset of a sweep by name — one list, drawn the same on
/// the confirm, the receipt and the transaction detail, so what was signed,
/// what is landing and what landed read as one thing.
fn breakdown_list(
    theme: &Theme,
    identicons: &mut IdenticonCache,
    title: Option<&SharedString>,
    rows: &[BreakdownRow],
) -> Div {
    let mut block = div().flex().flex_col().gap(px(6.));
    if let Some(title) = title {
        block = block.child(
            div()
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(title.clone()),
        );
    }
    let mut list = div()
        .flex()
        .flex_col()
        .px(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_sunken);
    for item in rows {
        let mut row = div().flex().items_center().gap(px(8.)).py(px(8.));
        if let Some(seed) = item.seed.as_ref() {
            row = row.child(crate::wallet::components::identicon_avatar(
                identicons,
                seed.as_ref(),
                24.,
            ));
        }
        list = list.child(
            row.child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .whitespace_nowrap()
                    .truncate()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(item.label.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(item.value.clone()),
            ),
        );
    }
    block.child(list)
}

fn divider(theme: &Theme) -> Div {
    div().h(px(1.)).bg(theme.divider)
}

/// Dispatch one body model to its panel. `Scan` never arrives — the page draws
/// DS1L as a centred modal instead of a column.
pub fn render(
    body: &FlowBody,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    actions: PanelActions,
) -> Div {
    match body {
        FlowBody::Receive(model) => {
            receive(model, theme, icons, actions.open_qr, actions.open_qr_rows)
        }
        FlowBody::ReceiveQr(model) => receive_qr(
            model,
            theme,
            icons,
            identicons,
            actions.acknowledge,
            actions.save_image,
        ),
        FlowBody::History(model) => {
            history(model, theme, icons, actions.open_tx, actions.open_tx_rows)
        }
        FlowBody::TxDetail(model) => tx_detail(model, theme, icons, identicons, actions.delete_tx),
        FlowBody::Assets(model) => assets(
            model,
            theme,
            icons,
            actions.open_add_token,
            actions.open_receive,
        ),
        FlowBody::AddToken(model) => add_token(
            model,
            theme,
            icons,
            identicons,
            window,
            actions.address_field,
            actions.add_to_wallet,
        ),
        FlowBody::SendPick(model) => send_pick(
            model,
            theme,
            icons,
            actions.open_send_form,
            actions.open_send_rows,
            actions.sweep_select_all,
            actions.send_pick_cta,
        ),
        FlowBody::SendForm(model) => send_form(model, theme, icons, identicons, window, actions),
        FlowBody::ContactPick(model) => contact_pick(
            model,
            theme,
            icons,
            identicons,
            actions.open_scan,
            actions.pick_contact_rows,
            actions.pick_group_rows,
        ),
        FlowBody::FeeToken(model) => fee_token(model, theme, icons, actions.fee_rows),
        FlowBody::BatchImport(model) => batch_import(model, theme, icons, window, actions),
        FlowBody::SendConfirm(model) => send_confirm(
            model,
            theme,
            icons,
            identicons,
            actions.advance,
            actions.notice_action,
            actions.notice_dismiss,
        ),
        FlowBody::SendReceipt(model) => {
            send_receipt(model, theme, icons, identicons, actions.advance)
        }
        // The page routes this away before it gets here; a column-shaped
        // viewfinder is the thing DS1L exists to avoid.
        FlowBody::Scan(model) => scan_placeholder(model, theme),
    }
}

fn receive(
    model: &ReceiveList,
    theme: &Theme,
    icons: &mut IconCache,
    mut open_qr: Option<Click>,
    per_row: Vec<Click>,
) -> Div {
    let mut col = column()
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(model.subtitle.clone()),
        )
        .child(flow_search(theme, icons, model.search_placeholder.clone()));
    // A live panel binds one listener per row; the fixture binds one and gives
    // it to the first, because every mock row opens the same address anyway.
    let mut per_row = per_row.into_iter();
    for (i, row) in model.rows.iter().enumerate() {
        if i > 0 {
            col = col.child(divider(theme));
        }
        let action = per_row
            .next()
            .or_else(|| if i == 0 { open_qr.take() } else { None });
        col = col.child(clickable(
            ElementId::from(("network", i)),
            action,
            network_row(theme, icons, row),
        ));
    }
    col
}

fn receive_qr(
    model: &ReceiveQr,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    acknowledge: Option<Click>,
    save_image: Option<Click>,
) -> Div {
    let mut col = column().child(
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_base)
            .child(model.title.clone()),
    );

    if let Some((label, value)) = &model.contract {
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(label.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_mono_address())
                        .text_color(theme.fg_base)
                        .child(value.clone()),
                )
                // The copy beside it copies the WHOLE contract — the line is
                // shortened. Drawn inert where there is nothing to copy.
                .child({
                    let copy = model.contract_copy.clone().map(|contract| -> Click {
                        Box::new(move |_: &ClickEvent, _: &mut Window, cx: &mut App| {
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(
                                contract.to_string(),
                            ));
                        })
                    });
                    clickable(
                        "receive-contract-copy",
                        copy,
                        icon_img(icons, Icon::Copy, false, theme.fg_subtle, 13.),
                    )
                }),
        );
    }

    // The gate comes FIRST, and it replaces the code rather than sitting over
    // it: a warning somebody can read around is one they will read around,
    // and the whole point is that the address is not handed over until this
    // has been read once.
    if let Some(gate) = &model.gate {
        return col.child(receive_gate(theme, gate, acknowledge));
    }

    col.child(address_card(
        theme,
        icons,
        identicons,
        model.account.name.clone(),
        model.account.seed.as_ref(),
        model.account.lines.clone(),
        // The whole address, rejoined from the two halves the card draws. A
        // receive screen's job is to hand this over, and the two ways it does
        // that — the code and this card — were both decorative until 031.
        //
        // `can_copy` is the CORE's answer, and it is not "is there a payload":
        // an address can be ready long before anybody has been told which
        // networks it is safe on.
        (model.qr_payload.is_some() && model.can_copy).then(|| {
            SharedString::from(format!(
                "{}{}",
                model.account.lines.0, model.account.lines.1
            ))
        }),
    ))
    .child(div().flex().justify_center().child(qr_card(
        theme,
        // The card is white in BOTH palettes, so its cut-out mark is
        // drawn against the LIGHT theme rather than the active one —
        // the dark palette's disc would punch an unreadable hole in a
        // code that a camera still has to resolve.
        Some(token_icon_logos(
            &Theme::light(),
            model.centre.ticker.as_ref(),
            model.centre.badge,
            &model.centre.logos,
        )),
        model.qr_payload.as_deref(),
    )))
    .child(
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_subtle)
            .child(model.warning.clone()),
    )
    .child(clickable(
        "receive-save-image",
        save_image,
        ghost_button(theme, model.save_image.clone()),
    ))
    .child(explorer_button(
        "receive-explorer",
        theme,
        model.view_on_explorer.clone(),
        model.explorer_url.as_ref(),
    ))
    .children(deposit_section(&model.deposits, theme))
}

/// Money that landed while the code was open.
///
/// Ported from `ReceiveScreen.tsx`'s `depositBox`: an open, de-boxed section
/// under a hairline — no filled card — with success ink on the dot and the
/// amount only, and the time, network and value in plain muted text. The
/// restraint is the design's: a celebration that shouts is one somebody learns
/// to distrust.
///
/// Absent when there is nothing, rather than an empty container: a hairline
/// with nothing under it reads as a section that failed to load.
fn deposit_section(deposits: &[DepositEntry], theme: &Theme) -> Option<Div> {
    if deposits.is_empty() {
        return None;
    }
    let mut section = div()
        .flex()
        .flex_col()
        .pt(px(16.))
        .border_t_1()
        .border_color(theme.border_card);
    for (i, entry) in deposits.iter().enumerate() {
        let mut group = div().flex().flex_col().py(px(10.));
        if i > 0 {
            group = group.border_t_1().border_color(theme.border_card);
        }
        group = group.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .w(px(6.))
                        .h(px(6.))
                        .rounded_full()
                        .bg(theme.success_base),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(entry.time.clone()),
                ),
        );
        for (amount, meta) in &entry.rows {
            group = group.child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    // Inset past the dot so amounts align under the time.
                    .pl(px(14.))
                    .pt(px(4.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.success_base)
                            .child(amount.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(meta.clone()),
                    ),
            );
        }
        section = section.child(group);
    }
    Some(section)
}

fn history(
    model: &HistoryPanel,
    theme: &Theme,
    icons: &mut IconCache,
    mut open_tx: Option<Click>,
    per_row: Vec<Click>,
) -> Div {
    let mut col = div().flex().flex_col();
    if model.loading {
        return col
            .child(skeleton_row(theme))
            .child(skeleton_row(theme))
            .child(skeleton_row(theme));
    }
    // A history with nothing in it is a fact, not a problem: one quiet line
    // rather than an illustrated empty state (the web's `.empty`).
    if let Some(text) = &model.empty {
        return col.child(
            div()
                .py(px(48.))
                .text_center()
                .text_size(theme::text_row_title())
                .text_color(theme.fg_muted)
                .child(text.clone()),
        );
    }
    let groups = &model.groups;
    // Row order here IS the order the page bound its listeners in, because both
    // walk the same groups. A live panel binds one per row; the fixture binds
    // one and gives it to the first.
    let mut per_row = per_row.into_iter();
    let mut index = 0usize;
    for group in groups {
        col = col.child(
            div()
                .pt(px(12.))
                .pb(px(4.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(group.label.clone()),
        );
        for row in &group.rows {
            let action = per_row
                .next()
                .or_else(|| if index == 0 { open_tx.take() } else { None });
            col = col.child(clickable(
                ElementId::from(("history", index)),
                action,
                activity_row(theme, icons, row),
            ));
            index += 1;
        }
    }
    col
}

fn tx_detail(
    model: &TxDetail,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    delete_tx: Option<Click>,
) -> Div {
    let mut col = column()
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(model.title.clone()),
                )
                .child(status_chip(theme, &model.status)),
        )
        .child(
            div()
                .text_size(theme::text_balance_hero())
                .font_weight(gpui::FontWeight::BOLD)
                // Money in is green; money out is plain ink, not red. Red means
                // something went wrong, and a transfer you chose to make did not.
                .text_color(if model.positive {
                    theme.success_base
                } else {
                    theme.fg_base
                })
                .child(model.amount.clone()),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(model.fiat.clone()),
        );

    for (i, fact) in model.facts.iter().enumerate() {
        if i > 0 {
            col = col.child(divider(theme));
        }
        col = col.child(fact_row(theme, icons, identicons, fact));
    }
    if !model.breakdown.is_empty() {
        col = col.child(breakdown_list(
            theme,
            identicons,
            model.breakdown_title.as_ref(),
            &model.breakdown,
        ));
    }
    col = col.child(explorer_button(
        "tx-explorer",
        theme,
        model.view_on_explorer.clone(),
        model.explorer_url.as_ref(),
    ));
    // Under the explorer, in the danger colour (the web's `variant="danger"`).
    // It removes the local record only; the chain keeps the transaction.
    if let Some(label) = &model.delete_label {
        col = col.child(clickable(
            "tx-delete",
            delete_tx,
            ghost_button(theme, label.clone()).text_color(theme.error_base),
        ));
    }
    col
}

fn assets(
    model: &AssetsPanel,
    theme: &Theme,
    icons: &mut IconCache,
    open_add_token: Option<Click>,
    open_receive: Option<Click>,
) -> Div {
    let mut col = column();
    if let Some((dots, label, add)) = &model.filter {
        col = col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(network_pill(theme, icons, dots, label.clone()))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(add.clone()),
                ),
        );
    }
    col = col.child(flow_search(theme, icons, model.search_placeholder.clone()));

    if let Some(empty) = &model.empty {
        return col
            .child(clickable(
                "assets-empty-receive",
                open_receive,
                empty_state(
                    theme,
                    icons,
                    Icon::WalletOutline,
                    empty.title.clone(),
                    empty.caption.clone(),
                ),
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(14.))
                    .border_1()
                    .border_color(theme.border_card)
                    // Question, answer, then the button — DT4L puts the CTA at
                    // the BOTTOM of the card, because it is what to do about
                    // the paragraph above it, not a heading for it.
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(empty.hint_title.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(empty.hint_body.clone()),
                    )
                    .child(clickable(
                        "assets-empty-cta",
                        open_add_token,
                        ghost_button(theme, empty.cta.clone()),
                    )),
            );
    }

    for (i, row) in model.rows.iter().enumerate() {
        col = col.child(asset_row(
            ElementId::from(("flow-asset", i)),
            theme,
            icons,
            row,
        ));
    }
    // DT1L hangs this centred and quiet under the list — it is the way out of
    // "my token is missing", not a call to action competing with the rows.
    col.child(clickable(
        "assets-add-by-address",
        open_add_token,
        div()
            .flex()
            .justify_center()
            .py(px(8.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(model.add_by_address.clone()),
    ))
}

fn add_token(
    model: &AddToken,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    address_field: Option<AddressField>,
    add_to_wallet: Option<Click>,
) -> Div {
    let mut col = column().child(segmented_toggle(
        theme,
        model.tab_erc20.clone(),
        model.tab_native.clone(),
        !model.native,
    ));

    if let Some((mark, name)) = &model.network {
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(10.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .child(inline_mark(theme, mark))
                .child(
                    div()
                        .flex_1()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(name.clone()),
                )
                .child(icon_img(
                    icons,
                    Icon::ChevronDown,
                    false,
                    theme.fg_muted,
                    14.,
                )),
        );
    }

    col = col.child(match address_field {
        Some(field) => {
            let strings = crate::ui::NameFieldStrings {
                label: model.field_label.clone(),
                placeholder: field.placeholder.clone(),
                helper: SharedString::from(""),
                too_long_hint: SharedString::from(""),
            };
            crate::ui::text_field(
                "add-token-address",
                theme,
                &strings,
                &field.value,
                false,
                false,
                &field.focus,
                window,
                field.on_change,
            )
        }
        None => mono_field(
            theme,
            Some(model.field_label.clone()),
            model.field_value.clone(),
        ),
    });

    col = match &model.result {
        AddTokenResult::Token { mark, name, detail } => col.child(
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border_card)
                .child(token_icon_logos(
                    theme,
                    mark.ticker.as_ref(),
                    mark.badge,
                    &mark.logos,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(name.clone()),
                        )
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_muted)
                                .child(detail.clone()),
                        ),
                ),
        ),
        AddTokenResult::Network {
            mark,
            name,
            chip,
            facts,
        } => {
            let mut card = div()
                .flex()
                .flex_col()
                .p(px(12.))
                .rounded(px(12.))
                .border_1()
                .border_color(theme.border_card)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .child(token_icon_logos(
                            theme,
                            mark.ticker.as_ref(),
                            mark.badge,
                            &mark.logos,
                        ))
                        .child(
                            div()
                                .flex_1()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(name.clone()),
                        )
                        .child(status_chip(theme, chip)),
                );
            for fact in facts {
                card = card.child(fact_row(theme, icons, identicons, fact));
            }
            col.child(card)
        }
    };

    // The write that failed, said above the button that failed to do it.
    // `live.rs` has filled this from `MtokView.save_error` since phase 6; the
    // panel dropped it on the floor, and the only thing that noticed was a
    // dead-code warning nobody read. Same defect as the picker that eats a
    // file: the core said no and the screen went on looking fine.
    if let Some(notice) = &model.notice {
        col = col.child(notice_card(notice, theme, None, None));
    }

    col.child(clickable(
        ElementId::from("add-token-cta"),
        add_to_wallet,
        accent_button(theme, model.cta.clone()),
    ))
}

/// What the core refused, drawn where the person is looking.
///
/// Amber while they are still typing, red when nothing can proceed as things
/// stand. The action is the way out the CORE offered — never a button this
/// file invented.
fn notice_card(
    notice: &SendNotice,
    theme: &Theme,
    action: Option<Click>,
    dismiss: Option<Click>,
) -> Div {
    let (tint, border) = if notice.error {
        (theme.error_soft, theme.error_base)
    } else {
        (theme.warning_soft, theme.warning_border)
    };
    let mut card = div()
        .flex()
        .flex_col()
        .gap(px(6.))
        .p(px(12.))
        .rounded(px(12.))
        .bg(tint)
        .border_1()
        .border_color(border);
    if let Some(title) = &notice.title {
        card = card.child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(if notice.error {
                    theme.error_base
                } else {
                    theme.warning_base
                })
                .child(title.clone()),
        );
    }
    card = card.child(
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_base)
            .child(notice.body.clone()),
    );
    if let Some(detail) = &notice.detail {
        card = card.child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(detail.clone()),
        );
    }
    // Two ways out of the same card: the retry the core offered, and — where
    // the stop has one — leaving it. Side by side, the retry first, because
    // that is the one a person came here to press.
    let mut row = div().flex().gap(px(8.));
    let mut any = false;
    if let Some(label) = &notice.action {
        any = true;
        row = row.child(clickable(
            "flow-notice-action",
            action,
            pill(theme, label.clone()),
        ));
    }
    if let Some(label) = &notice.dismiss {
        any = true;
        row = row.child(clickable(
            "flow-notice-dismiss",
            dismiss,
            div()
                .px(px(12.))
                .py(px(6.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(label.clone()),
        ));
    }
    if any { card.child(row) } else { card }
}

/// The panel's CTA in the state the core put it in. A shut button is drawn
/// shut and answers to nothing; a busy one keeps its accent — busy is not
/// disabled, and a person who pressed once should see the press took.
fn cta_button(
    id: &'static str,
    theme: &Theme,
    label: SharedString,
    state: CtaState,
    action: Option<Click>,
) -> Div {
    let button = accent_button(theme, label);
    match state {
        CtaState::Enabled => clickable(id, action, button),
        CtaState::Busy => clickable(id, None, button.opacity(0.7)),
        CtaState::Disabled => clickable(id, None, button.opacity(0.4)),
    }
}

/// A bordered pill for a secondary action — the recipient-row actions and,
/// live, the Max chip and the address-book affordance beside a typed field.
fn pill(theme: &Theme, label: SharedString) -> Div {
    div()
        .px(px(12.))
        .py(px(6.))
        .rounded(px(999.))
        .border_1()
        .border_color(theme.border_card)
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::text_row_sub())
        .text_color(theme.fg_base)
        .child(label)
}

fn send_pick(
    model: &SendPick,
    theme: &Theme,
    icons: &mut IconCache,
    mut open_form: Option<Click>,
    per_row: Vec<Click>,
    select_all: Option<Click>,
    cta: Option<Click>,
) -> Div {
    let (dots, pill_label) = &model.pill;
    let mut col = column()
        .child(flow_search(theme, icons, model.search_placeholder.clone()))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap(px(8.))
                .child(filter_chips(theme, &model.filters))
                .child(network_pill(theme, icons, dots, pill_label.clone()).flex_none()),
        );

    // SD1b's chain lock, in the corpus's own sentence: the first pick names
    // the network and the greying that follows is explained rather than left
    // to be guessed at.
    if let Some((colour, letter, text)) = model
        .selection
        .as_ref()
        .and_then(|selection| selection.notice.clone())
    {
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(10.))
                .p(px(12.))
                .rounded(px(10.))
                .bg(theme.bg_well)
                .child(crate::settings::components::chain_mark(letter, colour, 20.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_row_sub())
                        .line_height(px(18.))
                        .text_color(theme.fg_muted)
                        .child(text),
                ),
        );
    }

    // A live panel binds one listener per row; the fixture binds one and
    // gives it to the first, because every mock row opens the same drawing.
    let mut per_row = per_row.into_iter();
    for (i, row) in model.rows.iter().enumerate() {
        let action = per_row
            .next()
            .or_else(|| if i == 0 { open_form.take() } else { None });
        let selected = model
            .selection
            .as_ref()
            .and_then(|s| s.selected.get(i).copied())
            .unwrap_or(false);
        let dimmed = model
            .selection
            .as_ref()
            .and_then(|s| s.dimmed.get(i).copied())
            .unwrap_or(false);
        let drawn = asset_row(ElementId::from(("flow-send", i)), theme, icons, row);
        let mut wrapper = div().when(selected, |el| {
            el.rounded(px(10.)).bg(theme.accent.opacity(0.10))
        });
        wrapper = wrapper.child(if dimmed {
            // Off-network rows are readable and inert. The core refuses them
            // anyway; drawing them as pressable would be an offer it declines.
            drawn.opacity(0.4).into_any_element()
        } else {
            clickable(ElementId::from(("flow-send-row", i)), action, drawn).into_any_element()
        });
        col = col.child(wrapper);
    }

    if let Some(selection) = model.selection.as_ref() {
        col = col.child(clickable(
            "flow-send-select-all",
            select_all,
            div()
                .py(px(8.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(selection.select_all.clone()),
        ));
    }

    // DSD1L sets this as a quiet centred link, not a button: sending several
    // tokens at once is a different journey, not the main one on this panel.
    // Once tokens ARE ticked it becomes the accent action, because then it is.
    let label = div()
        .flex()
        .justify_center()
        .py(px(8.))
        .text_size(theme::text_row_sub())
        .text_color(if model.cta_accent {
            theme.fg_inverse
        } else {
            theme.fg_muted
        })
        .child(model.cta.clone());
    col.child(clickable(
        "flow-send-cta",
        cta,
        if model.cta_accent {
            div()
                .h(px(44.))
                .rounded(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.accent)
                .child(label)
        } else {
            label
        },
    ))
}

fn send_form(
    model: &SendForm,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    mut actions: PanelActions,
) -> Div {
    let (mark, symbol, detail, max) = &model.token;
    // Max lives on the token card, as the web's `TokenHeaderCard` draws it. A
    // live form makes that one chip the button rather than drawing a second,
    // working one under the field (#288).
    let live = actions.amount_field.is_some();
    let mut card = token_header_card(
        theme,
        mark,
        symbol.clone(),
        detail.clone(),
        max.clone().filter(|_| !live),
    );
    if live && let Some(max) = max {
        card = card.child(clickable(
            "send-max",
            actions.tap_max.take(),
            max_chip(theme, max.clone()),
        ));
    }
    let mut col = column().child(card);

    if let Some(field) = actions.amount_field.take() {
        // Live: a real field, labelled with the coin it counts in, the fiat
        // line under it and the Max chip beside it. The value is the CORE's
        // — it validates every keystroke — so the field holds no copy.
        let strings = crate::ui::NameFieldStrings {
            // The unit the figure is typed in — money or the token (#231).
            label: model
                .amount_unit
                .clone()
                .unwrap_or_else(|| model.token.1.clone()),
            placeholder: field.placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let block = column().child(crate::ui::text_field(
            "send-amount",
            theme,
            &strings,
            &field.value,
            false,
            false,
            &field.focus,
            window,
            field.on_change,
        ));
        let mut under = div().flex().items_center().justify_between().gap(px(8.));
        if let Some((_, fiat)) = &model.amount {
            let line = div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted);
            under = under.child(match model.denom_toggle {
                // ⇄ IS the other denomination's line (the web's `button.fiat`):
                // pressing what it shows is how the figure comes across. Where
                // the swap would change nothing it is drawn dimmed and answers
                // to nothing — the notice below says why.
                Some(enabled) => clickable(
                    "send-denom-toggle",
                    if enabled {
                        actions.toggle_denom.take()
                    } else {
                        None
                    },
                    line.when(!enabled, |el| el.opacity(0.5))
                        .child(SharedString::from(format!("⇄  {fiat}"))),
                ),
                None => line.child(fiat.clone()),
            });
        }
        col = col.child(block.child(under));
    } else if let Some((value, fiat)) = &model.amount {
        col = col.child(
            div()
                .flex()
                .flex_col()
                .items_center()
                .py(px(16.))
                .child(
                    div()
                        .text_size(theme::text_balance_hero())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(value.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(fiat.clone()),
                ),
        );
    }

    if let Some(field) = actions.recipient_field.take() {
        // Live: the address is typed (or pasted), and the book is one pill
        // away. The label carries the core's trust line once it has one.
        let label = model
            .recipient
            .as_ref()
            .map(|(label, _, _)| label.clone())
            .unwrap_or_default();
        let strings = crate::ui::NameFieldStrings {
            label,
            placeholder: field.placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        col = col.child(crate::ui::text_field(
            "send-recipient",
            theme,
            &strings,
            &field.value,
            false,
            false,
            &field.focus,
            window,
            field.on_change,
        ));
        if let Some(pick) = &model.pick_contacts {
            col = col.child(div().flex().child(clickable(
                "flow-pick-contacts",
                actions.open_contact_pick.take(),
                pill(theme, pick.clone()),
            )));
        }
    } else if let Some((label, lines, seed)) = &model.recipient {
        col = col
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(label.clone()),
            )
            .child(clickable(
                "flow-recipient",
                actions.open_contact_pick.take(),
                address_card(
                    theme,
                    icons,
                    identicons,
                    lines.0.clone(),
                    seed.as_ref(),
                    (lines.1.clone(), SharedString::default()),
                    // The send form's recipient row: the whole card OPENS the
                    // contact picker, so a copy click inside it would fight the
                    // row it sits in.
                    None,
                ),
            ));
    }

    if let Some(add) = &model.add_recipient {
        col = col.child(clickable(
            "flow-add-recipient",
            actions.add_recipient.take(),
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(SharedString::from(format!("+  {add}"))),
        ));
    }

    let mut amounts = actions.split_amount_fields.drain(..);
    let mut removes = actions.remove_recipient_rows.drain(..);
    for (index, recipient) in model.recipients.iter().enumerate() {
        col = col.child(recipient_card(
            theme,
            icons,
            identicons,
            recipient,
            index,
            crate::flows::components::RecipientRowActions {
                amount: amounts.next(),
                remove: removes.next(),
            },
            window,
        ));
    }

    if !model.recipient_actions.is_empty() {
        // The pills are ordered the way the fixture builds them — one more
        // payee, the contact book, a pasted list — so each gets the listener
        // that matches the label the mock prints on it.
        let mut bound = [
            actions.add_recipient.take(),
            actions.open_contact_pick.take(),
            actions.open_batch_import.take(),
        ];
        let mut pills = div().flex().gap(px(6.));
        for (i, label) in model.recipient_actions.iter().enumerate() {
            let pill = div()
                .flex_1()
                .py(px(8.))
                .rounded(px(999.))
                .border_1()
                .border_color(theme.border_card)
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(label.clone());
            let action = bound.get_mut(i).and_then(Option::take);
            pills = pills
                .child(clickable(ElementId::from(("recipient-action", i)), action, pill).flex_1());
        }
        col = col.child(pills);
    }

    if let Some((label, value)) = &model.summary {
        col = col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(label.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                // Over the balance, the total says so in ink
                                // before Continue has to refuse it.
                                .text_color(if model.summary_over {
                                    theme.error_base
                                } else {
                                    theme.fg_base
                                })
                                .child(value.clone()),
                        )
                        .when_some(model.remaining.clone(), |el, left| {
                            el.child(
                                div()
                                    .text_size(theme::text_label())
                                    .text_color(theme.fg_muted)
                                    .child(left),
                            )
                        }),
                ),
        );
    }

    if let Some(notice) = &model.notice {
        col = col.child(notice_card(
            notice,
            theme,
            actions.notice_action.take(),
            actions.notice_dismiss.take(),
        ));
    }
    col.child(clickable(
        "flow-fee-row",
        actions.open_fee_token.take(),
        fee_row(theme, icons, &model.fee),
    ))
    .child(cta_button(
        "flow-form-cta",
        theme,
        model.cta.clone(),
        model.cta_state,
        actions.advance.take(),
    ))
}

fn contact_pick(
    model: &ContactPick,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    open_scan: Option<Click>,
    per_row: Vec<Click>,
    mut group_rows: Vec<Click>,
) -> Div {
    let mut per_row = per_row.into_iter();
    let mut col = column()
        .child(flow_search(theme, icons, model.search_placeholder.clone()))
        // Scan sits above the saved people: most sends go to someone already in
        // the book, but the ones that don't are the ones where a person is
        // holding a phone in one hand and an address in the other.
        .child(clickable(
            "flow-scan-row",
            open_scan,
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .p(px(12.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .child(icon_img(icons, Icon::QrCode, false, theme.fg_subtle, 15.))
                .child(
                    div()
                        .flex_1()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(model.scan_row.clone()),
                )
                .child(icon_img(
                    icons,
                    Icon::ChevronRight,
                    false,
                    theme.fg_subtle,
                    12.,
                )),
        ))
        .child(
            div()
                .pt(px(4.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(model.groups_title.clone()),
        );

    let mut per_group = std::mem::take(&mut group_rows).into_iter();
    for (index, (name, count, first, second)) in model.groups.iter().enumerate() {
        let row = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(8.))
            // Two overlapping discs stand for "several people" without
            // drawing any of them — a group has no single face to show.
            .child(
                div()
                    .flex()
                    .child(div().w(px(28.)).h(px(28.)).rounded(px(14.)).bg(*first))
                    .child(
                        div()
                            .w(px(28.))
                            .h(px(28.))
                            .rounded(px(14.))
                            .bg(*second)
                            .ml(px(-10.)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(theme::text_row_title())
                    .text_color(theme.fg_base)
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(count.clone()),
            );
        col = col.child(clickable(
            ElementId::from(("flow-group", index)),
            per_group.next(),
            row,
        ));
    }

    col = col.child(
        div()
            .pt(px(4.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_subtle)
            .child(model.contacts_title.clone()),
    );

    for (i, contact) in model.contacts.iter().enumerate() {
        let mut name_row = div().flex().items_center().gap(px(6.)).child(
            div()
                .text_size(theme::text_row_title())
                .text_color(theme.fg_base)
                .child(contact.name.clone()),
        );
        if let Some(group) = &contact.group {
            name_row = name_row.child(
                div()
                    .px(px(6.))
                    .rounded(px(4.))
                    .bg(theme.bg_sunken)
                    .text_size(theme::text_label())
                    .text_color(theme.fg_muted)
                    .child(group.clone()),
            );
        }
        let entry = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(8.))
            .child(crate::wallet::components::identicon_avatar(
                identicons,
                contact.seed.as_ref(),
                28.,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(name_row)
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_mono_address())
                            .text_color(theme.fg_subtle)
                            .child(contact.address.clone()),
                    ),
            )
            .child(icon_img(
                icons,
                Icon::ChevronRight,
                false,
                theme.fg_subtle,
                12.,
            ));
        col = col.child(clickable(
            ElementId::from(("flow-contact", i)),
            per_row.next(),
            entry,
        ));
    }
    col
}

fn fee_token(
    model: &FeeTokenPick,
    theme: &Theme,
    icons: &mut IconCache,
    per_row: Vec<Click>,
) -> Div {
    let mut per_row = per_row.into_iter();
    let mut col = column().child(
        // Paying gas in a stablecoin is unusual enough that someone seeing USDC
        // offered as a fee token will wonder whether they are being asked to
        // send it. Saying what the choice is for, once, is cheaper than a
        // tooltip on each row.
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(model.hint.clone()),
    );
    for (i, row) in model.rows.iter().enumerate() {
        let shell = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(10.))
            .rounded(px(12.));
        let shell = if row.selected {
            shell.bg(theme.bg_raised)
        } else {
            shell
        };
        let mut entry = shell
            .child(token_icon_logos(
                theme,
                row.mark.ticker.as_ref(),
                row.mark.badge,
                &row.mark.logos,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(row.symbol.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(row.balance.clone()),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_end()
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(row.fee.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(model.estimate_label.clone()),
                    ),
            );
        // Only the chosen row draws the tick; the others leave the space, so
        // choosing one does not shift the rows under it.
        if row.selected {
            entry = entry.child(icon_img(icons, Icon::Check, false, theme.accent, 14.));
        }
        // A coin that cannot cover the fee is shown for context and answers
        // to nothing (invariant ⑧) — the listener is dropped, not just dimmed.
        let action = per_row.next();
        let (entry, action) = if row.insufficient {
            (entry.opacity(0.45), None)
        } else {
            (entry, action)
        };
        col = col.child(clickable(
            ElementId::from(("flow-fee-row", i)),
            action,
            entry,
        ));
    }
    col
}

fn batch_import(
    model: &BatchImport,
    theme: &Theme,
    icons: &mut IconCache,
    window: &Window,
    mut actions: PanelActions,
) -> Div {
    // Live: two clickable halves drawn exactly like the one segmented control;
    // the mock keeps the component itself.
    let toggle = match actions.batch_unit.take() {
        Some((fiat, token)) => {
            let seg = |label: SharedString, on: bool| {
                let base = div()
                    .py(px(8.))
                    .rounded(px(10.))
                    .flex()
                    .items_center()
                    .justify_center();
                let base = if on { base.bg(theme.bg_raised) } else { base };
                base.text_size(theme::text_row_sub())
                    .text_color(if on { theme.fg_base } else { theme.fg_muted })
                    .child(label)
            };
            div()
                .flex()
                .gap(px(2.))
                .p(px(2.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .child(
                    clickable(
                        "batch-unit-fiat",
                        Some(fiat),
                        seg(model.unit_fiat.clone(), model.fiat_on),
                    )
                    .flex_1(),
                )
                .child(
                    clickable(
                        "batch-unit-token",
                        Some(token),
                        seg(model.unit_token.clone(), !model.fiat_on),
                    )
                    .flex_1(),
                )
        }
        None => segmented_toggle(
            theme,
            model.unit_fiat.clone(),
            model.unit_token.clone(),
            model.fiat_on,
        ),
    };
    let mut col = column()
        .child(toggle)
        .child(clickable(
            "batch-paste",
            actions.batch_paste.take(),
            mono_field(theme, None, model.paste.clone()),
        ))
        .child(
            div()
                .flex()
                .justify_center()
                .gap(px(8.))
                .child(clickable(
                    "batch-pick-file",
                    actions.batch_pick_file.take(),
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(model.import_file.clone()),
                ))
                .child(clickable(
                    "batch-template",
                    actions.batch_template.take(),
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(model.template.clone()),
                )),
        )
        .child(divider(theme));
    col = match actions.batch_rate_field.take() {
        Some(field) => {
            let strings = crate::ui::NameFieldStrings {
                label: model.rate_section.clone(),
                placeholder: field.placeholder.clone(),
                helper: SharedString::from(""),
                too_long_hint: SharedString::from(""),
            };
            let mut row = div()
                .flex()
                .items_end()
                .gap(px(8.))
                .child(div().flex_1().child(crate::ui::text_field(
                    "batch-rate",
                    theme,
                    &strings,
                    &field.value,
                    false,
                    false,
                    &field.focus,
                    window,
                    field.on_change,
                )));
            if let Some(reset) = &model.rate_reset {
                row = row.child(clickable(
                    "batch-rate-reset",
                    actions.batch_rate_reset.take(),
                    pill(theme, reset.clone()),
                ));
            }
            col.child(row)
        }
        None => col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(model.rate_section.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(model.rate_value.clone()),
                ),
        ),
    };
    col = col
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(model.rate_hint.clone()),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(model.parsed.clone()),
        );

    for row in &model.rows {
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(6.))
                .child(icon_img(
                    icons,
                    if row.ok { Icon::Check } else { Icon::X },
                    false,
                    if row.ok {
                        theme.success_base
                    } else {
                        theme.error_base
                    },
                    13.,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .font_family(theme::font_mono())
                        .text_size(theme::text_mono_address())
                        .text_color(theme.fg_base)
                        .child(row.address.clone()),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        // A refused line has no figure; its reason alone.
                        .when(!row.conversion.is_empty(), |el| {
                            el.child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_muted)
                                    .child(row.conversion.clone()),
                            )
                        })
                        // Why this line will not be paid, beside the line.
                        .when_some(row.note.clone(), |el, note| {
                            el.child(
                                div()
                                    .text_size(theme::text_label())
                                    .text_color(theme.error_base)
                                    .child(note),
                            )
                        }),
                ),
        );
    }
    if let Some(total) = &model.total {
        col = col.child(batch_total_line(total, theme));
    }

    col = col.child(
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.error_base)
            .child(model.rejected.clone()),
    );
    if let Some(notice) = &model.notice {
        col = col.child(notice_card(notice, theme, None, None));
    }
    // What the import does to the rows already on the form, beside the button
    // that does it — and the way to choose the other, because either can be
    // what is meant.
    if let Some((note, action)) = &model.merge {
        col = col.child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_x(px(8.))
                .text_size(theme::text_row_sub())
                .child(div().text_color(theme.fg_muted).child(note.clone()))
                .child(clickable(
                    "batch-merge",
                    actions.batch_merge.take(),
                    div()
                        .cursor_pointer()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.accent)
                        .child(action.clone()),
                )),
        );
    }
    // Bad rows are marked and skipped, never silently dropped, and the CTA
    // counts only the good ones — a button that says "Import 3" and imports 2
    // is how someone underpays a contractor. A gate the core shut is drawn
    // shut: dimmed, and it answers to nothing.
    let cta = accent_button(theme, model.cta.clone());
    if model.cta_enabled {
        col.child(clickable("batch-apply", actions.advance.take(), cta))
    } else {
        col.child(cta.opacity(0.4))
    }
}

/// DSD2cL's total: the figure the import sends and, under it, what it draws
/// from — or, in error ink, that it draws more than there is.
fn batch_total_line(total: &super::fixtures::BatchTotal, theme: &Theme) -> Div {
    let over = total.over.is_some();
    div()
        .flex()
        .items_start()
        .justify_between()
        .gap(px(8.))
        .pt(px(10.))
        .border_t_1()
        .border_color(theme.divider)
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(total.label.clone()),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .items_end()
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(if over {
                            theme.error_base
                        } else {
                            theme.fg_base
                        })
                        .child(total.value.clone()),
                )
                .when_some(total.detail.clone(), |el, detail| {
                    el.child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_muted)
                            .child(detail),
                    )
                })
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(if over {
                            theme.error_base
                        } else {
                            theme.fg_muted
                        })
                        .child(total.over.clone().unwrap_or_else(|| total.balance.clone())),
                ),
        )
}

fn send_confirm(
    model: &SendConfirm,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    advance: Option<Click>,
    notice_action: Option<Click>,
    notice_dismiss: Option<Click>,
) -> Div {
    let mut hero = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(8.))
        .py(px(12.));
    // The coin, drawn (founder, 2026-09-17): the confirm page named it in
    // words while every row beneath it carried art.
    if let Some(mark) = &model.mark {
        hero = hero.child(token_icon_logos(
            theme,
            mark.ticker.as_ref(),
            mark.badge,
            &mark.logos,
        ));
    }
    let mut col = column().child(
        hero.child(
            div()
                .text_size(theme::text_balance_hero())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(model.amount.clone()),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(model.subline.clone()),
        ),
    );

    let mut card = div()
        .flex()
        .flex_col()
        .px(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_sunken);
    for (i, fact) in model.facts.iter().enumerate() {
        if i > 0 {
            card = card.child(divider(theme));
        }
        card = card.child(fact_row(theme, icons, identicons, fact));
    }
    col = col.child(card);

    // The first-time tell, under the facts it is about, in the warning colour.
    if let Some(tag) = &model.recipient_tag {
        col = col.child(
            div()
                .px(px(12.))
                .text_size(theme::text_row_sub())
                .font_weight(gpui::FontWeight::MEDIUM)
                .text_color(theme.warning_base)
                .child(tag.clone()),
        );
    }

    if !model.breakdown.is_empty() {
        col = col.child(breakdown_list(theme, identicons, None, &model.breakdown));
    }

    if let Some(notice) = &model.notice {
        col = col.child(notice_card(notice, theme, notice_action, notice_dismiss));
    }
    // Per the SPEC sheet this is the ONE accent CTA in the whole send journey.
    col.child(cta_button(
        "flow-confirm-cta",
        theme,
        model.cta.clone(),
        model.cta_state,
        advance,
    ))
}

fn send_receipt(
    model: &SendReceipt,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    advance: Option<Click>,
) -> Div {
    let mut col = column().child(
        div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(6.))
            .py(px(24.))
            .child(
                div()
                    .w(px(88.))
                    .h(px(88.))
                    .rounded(px(44.))
                    .bg(theme.bg_sunken)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(icons, Icon::RefreshCw, false, theme.fg_muted, 26.)),
            )
            .child(
                div()
                    .text_size(theme::text_panel_title())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(model.title.clone()),
            ),
    );

    for (i, caption) in model.captions.iter().enumerate() {
        col = col.child(
            div()
                .flex()
                .justify_center()
                .text_size(theme::text_row_sub())
                // The second caption is the one that says "you can leave" —
                // true, useful, and not what the person is waiting to read.
                .text_color(if i == 0 {
                    theme.fg_muted
                } else {
                    theme.fg_subtle
                })
                .child(caption.clone()),
        );
    }

    if !model.breakdown.is_empty() {
        col = col.child(breakdown_list(
            theme,
            identicons,
            model.breakdown_title.as_ref(),
            &model.breakdown,
        ));
    }

    if let Some((label, value)) = &model.hash {
        col = col.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(label.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_mono_address())
                        .text_color(theme.fg_base)
                        .child(value.clone()),
                ),
        );
    }

    // "Close · keep running" is load-bearing copy: the transaction does not
    // depend on this panel staying open.
    col.child(clickable(
        "flow-receipt-cta",
        advance,
        ghost_button(theme, model.cta.clone()),
    ))
}

/// DS1L's body if it ever reached the column. It does not — the page draws the
/// scanner as a centred modal — so this is the honest fallback rather than a
/// second viewfinder implementation.
fn scan_placeholder(model: &ScanModal, theme: &Theme) -> Div {
    column().child(
        div()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(model.hint.clone()),
    )
}

/// DS1L — the scanner, centred over a dimmed window.
///
/// A scanner is a viewfinder and a 400px column is the wrong shape for one, so
/// this is the single flow the third column does not host.
pub fn scan_modal(
    model: &ScanModal,
    theme: &Theme,
    icons: &mut IconCache,
    mut tool_actions: Vec<Option<Click>>,
    close: Option<Click>,
    preview: Option<std::sync::Arc<gpui::RenderImage>>,
) -> Div {
    let mut tools = div().flex().gap(px(8.));
    let mut bound = tool_actions.drain(..);
    for (i, label) in model.tools.iter().enumerate() {
        tools = tools.child(clickable(
            ElementId::from(("scan-tool", i)),
            bound.next().flatten(),
            ghost_button(theme, label.clone()),
        ));
    }

    div()
        .w(px(560.))
        .p(px(24.))
        .rounded(px(20.))
        .bg(theme.bg_base)
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(theme::text_panel_title())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(model.title.clone()),
                )
                // A BUTTON, not a drawing of one. For as long as the scanner had
                // a camera this was an 18px picture of an X with nothing behind
                // it, inside a card that swallows every press — so the only way
                // out was a click on the dimmed window around it, which nobody
                // finds: the founder had to quit the app to change method
                // (2026-09-19). 32px of target, because 18 is an icon's size
                // and not a finger's or a trackpad's.
                .child(clickable(
                    "scan-close",
                    close,
                    div()
                        .size(px(32.))
                        .rounded_full()
                        .bg(theme.bg_sunken)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(icon_img(icons, Icon::X, false, theme.fg_muted, 18.)),
                )),
        )
        .child(
            // The viewfinder is landscape, not square — roughly what a webcam
            // hands you, and what DS1L measures. With a camera running it
            // holds the camera; without one it stays the empty well it has
            // always been, and the toolbar's other door still works.
            {
                let well = div()
                    .w_full()
                    .h(px(336.))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .bg(theme.bg_sunken);
                match preview {
                    Some(image) => well.child(
                        gpui::img(gpui::ImageSource::Render(image))
                            .w_full()
                            .h(px(336.)),
                    ),
                    None => well,
                }
            },
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(model.hint.clone()),
        )
        .child(tools)
}

/// The warning that stands where the code will be.
///
/// Not an overlay ON the code: a cover somebody can read around is one they
/// will read around. The button is withheld while the flag is still being
/// read — a button that appears a frame later is one somebody clicks twice,
/// and the second click would land on whatever took its place.
fn receive_gate(
    theme: &Theme,
    gate: &crate::flows::fixtures::ReceiveGate,
    act: Option<Click>,
) -> Div {
    let mut card = column()
        .p(px(20.))
        .rounded(px(16.))
        .border_1()
        .border_color(theme.border_card)
        .child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(gate.title.clone()),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(gate.body.clone()),
        )
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(gate.counterfactual.clone()),
        );
    if !gate.loading {
        card = card.child(clickable(
            "receive-ack",
            act,
            accent_button(theme, gate.confirm.clone()),
        ));
    }
    card
}
