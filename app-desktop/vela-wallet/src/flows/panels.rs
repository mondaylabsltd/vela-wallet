//! The flow panel bodies (spec 021) — what the third column holds.
//!
//! Each function takes a body model and returns the `Div` the page drops into
//! its existing `panel_scaffold`. The scaffold owns the title, the back chevron
//! and the close button; these own only what is under them.

use gpui::{
    App, ClickEvent, Div, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    ParentElement, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};

use gpui::StyledImage as _;
use gpui::prelude::FluentBuilder as _;

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{self, Theme};
use crate::wallet::components::{
    activity_row, asset_row, empty_state, icon_img, skeleton_row, token_icon_logos,
};

use super::components::{
    CopyButton, accent_button, address_card, danger_button, disabled_accent_button, fact_row,
    fee_refresh_icon, fee_row, fee_speed_note, fee_speed_option, fee_speed_summary, fee_stale_line,
    filter_chips, flow_search, ghost_button, max_chip, mono_field, network_pill, network_row,
    qr_card, recipient_card, search_empty, search_matches, segmented_toggle, status_chip,
    token_header_card,
};
use super::fixtures::{
    AddToken, AddTokenResult, AssetsPanel, BatchImport, BreakdownRow, ContactPick, CtaState,
    DepositEntry, FeeSpeedModel, FeeTokenPick, FlowBody, HistoryPanel, ReceiveList, ReceiveQr,
    ScanModal, SendConfirm, SendForm, SendNotice, SendPick, SendReceipt, TxDetail,
};

/// One prepared click listener. The page builds these from `cx.listener`
/// before rendering, because a panel body has no entity to listen on.
pub type Click = Box<ClickFn>;
/// What a `Click` holds.
pub type ClickFn = dyn Fn(&ClickEvent, &mut Window, &mut App) + 'static;

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
    /// DSD2L, live: measure the fee again (spec 068).
    pub refresh_fee: Option<Click>,
    /// DSD2L, live: fold or unfold the speed control, and one listener per
    /// option in the order they draw — a pick is one-shot.
    pub toggle_speed: Option<Click>,
    pub pick_speed_rows: Vec<Click>,
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
    /// DT3L, live: the ERC-20 and native halves of the toggle (078 F-07).
    pub add_token_tabs: Option<(Click, Click)>,
    /// DT3L native, live: one listener per suggested chain, in order.
    pub add_token_picks: Vec<Click>,
    /// DSD1L, live: one listener per token row. Empty falls back to
    /// `open_send_form`, which the fixture gives to its first row.
    pub open_send_rows: Vec<Click>,
    /// DSD1L, live: the class chips (全部 / 稳定币 / Gas / 其他), in
    /// `SendClass::CHIPS` order. Empty draws them inert.
    pub send_class_chips: Vec<Click>,
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
    /// DSD2bL, live: each split row's own address, and the book for that row
    /// (078 F-06) — in the order the rows draw.
    pub split_address_fields: Vec<AddressField>,
    pub pick_recipient_rows: Vec<Click>,
    pub remove_recipient_rows: Vec<Click>,
    /// DSD2bL, live: "Use X for the empty rows".
    pub fill_empty: Option<Click>,
    /// DR1L / DT1L / DSD1L / DSD2eL: the search field over the list (078
    /// X-05). The page owns the query; the panel filters with it. `None` draws
    /// the placeholder and filters nothing.
    pub search: Option<AddressField>,
    /// Every copy button in the panel (078 X-06): the page writes the text and
    /// shows the tick. `None` draws the buttons inert.
    pub copy: Option<CopyAction>,
}

/// The page's copy, handed to a panel: which value was copied a moment ago
/// (by key, while its tick shows), and how to copy another.
pub struct CopyAction {
    pub copied: Option<SharedString>,
    pub on_copy: OnCopy,
}

/// Copy `text` (the second) as the value named `key` (the first).
pub type OnCopy = std::rc::Rc<dyn Fn(SharedString, SharedString, &mut Window, &mut App)>;

impl CopyAction {
    /// The button for one value: `key` names it among the panel's copies,
    /// `text` is what goes on the clipboard.
    pub fn button(&self, key: impl Into<SharedString>, text: SharedString) -> CopyButton {
        let key = key.into();
        let copied = self.copied.as_ref() == Some(&key);
        let on_copy = self.on_copy.clone();
        CopyButton {
            copied,
            on_click: Box::new(move |_, window, cx| on_copy(key.clone(), text.clone(), window, cx)),
        }
    }
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
/// One prepared click for two targets — the row and its QR glyph — since a
/// `Click` is a `Box` and cannot be cloned.
fn split_click(click: Option<Click>) -> (Option<Click>, Option<Click>) {
    match click {
        Some(click) => {
            let shared: std::rc::Rc<ClickFn> = std::rc::Rc::from(click);
            let other = shared.clone();
            (
                Some(Box::new(move |event, window, cx| shared(event, window, cx))),
                Some(Box::new(move |event, window, cx| other(event, window, cx))),
            )
        }
        None => (None, None),
    }
}

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
    // A raised surface, as every card on the web's flows (078 F-11).
    let mut list = div()
        .flex()
        .flex_col()
        .px(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_raised);
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
/// [`render`], with a split send form's foot handed back apart from its
/// body (078 F-09): in a split the total, the refusal and Continue travel
/// together at the bottom of the column — a split can be sixty rows long,
/// and what it adds up to is the figure that matters while they are typed.
pub fn render_parts(
    body: &FlowBody,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    actions: PanelActions,
) -> (Div, Option<Div>) {
    if let FlowBody::SendForm(model) = body
        && !model.recipients.is_empty()
    {
        let (body, foot) = send_form_parts(model, theme, icons, identicons, window, actions);
        return (body, Some(foot));
    }
    (
        render(body, theme, icons, identicons, window, actions),
        None,
    )
}

pub fn render(
    body: &FlowBody,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    actions: PanelActions,
) -> Div {
    match body {
        FlowBody::Receive(model) => receive(
            model,
            theme,
            icons,
            window,
            actions.search,
            actions.copy,
            actions.open_qr,
            actions.open_qr_rows,
        ),
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
        FlowBody::TxDetail(model) => tx_detail(
            model,
            theme,
            icons,
            identicons,
            actions.copy,
            actions.delete_tx,
        ),
        FlowBody::Assets(model) => assets(
            model,
            theme,
            icons,
            window,
            actions.search,
            actions.open_add_token,
            actions.open_receive,
        ),
        FlowBody::AddToken(model) => add_token(
            model,
            theme,
            icons,
            identicons,
            window,
            AddTokenActions {
                field: actions.address_field,
                submit: actions.add_to_wallet,
                tabs: actions.add_token_tabs,
                picks: actions.add_token_picks,
            },
        ),
        FlowBody::SendPick(model) => send_pick(
            model,
            theme,
            icons,
            window,
            actions.search,
            actions.open_send_form,
            actions.open_send_rows,
            actions.send_class_chips,
            actions.sweep_select_all,
            actions.send_pick_cta,
            actions.notice_action,
        ),
        FlowBody::SendForm(model) => send_form(model, theme, icons, identicons, window, actions),
        FlowBody::ContactPick(model) => contact_pick(
            model,
            theme,
            icons,
            identicons,
            window,
            actions.search,
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
        FlowBody::SendReceipt(model) => send_receipt(
            model,
            theme,
            icons,
            identicons,
            // The column's height, less its header and bottom padding: what
            // the receipt centres itself in.
            f32::from(window.viewport_size().height) - 120.,
            actions.copy,
            actions.advance,
        ),
        // The page routes this away before it gets here; a column-shaped
        // viewfinder is the thing DS1L exists to avoid.
        FlowBody::Scan(model) => scan_placeholder(model, theme),
    }
}

#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
fn receive(
    model: &ReceiveList,
    theme: &Theme,
    icons: &mut IconCache,
    window: &Window,
    search: Option<AddressField>,
    copy: Option<CopyAction>,
    mut open_qr: Option<Click>,
    per_row: Vec<Click>,
) -> Div {
    let query = search.as_ref().map(|f| f.value.clone()).unwrap_or_default();
    let mut col = column()
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(model.subtitle.clone()),
        )
        .child(flow_search(
            theme,
            icons,
            model.search_placeholder.clone(),
            search,
            window,
        ));
    // A live panel binds one listener per row; the fixture binds one and gives
    // it to the first, because every mock row opens the same address anyway.
    // Indexed, not iterated: a search hides rows, and row N must still open
    // network N.
    let mut per_row = per_row.into_iter().map(Some).collect::<Vec<_>>();
    let mut shown = 0;
    // One flush list, a hairline on each row after the first (078 F-11): a
    // divider as its own child took the panel's 12 gap on BOTH sides, and
    // every network row stood 85 tall to the web's 65.
    let mut list = div().flex().flex_col();
    for (i, row) in model.rows.iter().enumerate() {
        if !search_matches(&query, &row.name) {
            continue;
        }
        let ruled = shown > 0;
        shown += 1;
        let action = per_row
            .get_mut(i)
            .and_then(Option::take)
            .or_else(|| if i == 0 { open_qr.take() } else { None });
        // The row's two buttons, as the web draws them: copy writes this
        // network's address and ticks; the QR glyph opens its code, as a
        // click anywhere on the row does.
        let copy = copy
            .as_ref()
            .map(|copy| copy.button(format!("network:{i}"), row.address_full.clone()));
        let (row_click, qr_click) = split_click(action);
        list = list.child(
            div()
                .when(ruled, |el| el.border_t_1().border_color(theme.divider))
                .child(clickable(
                    ElementId::from(("network", i)),
                    row_click,
                    network_row(theme, icons, row, i, copy, qr_click),
                )),
        );
    }
    col = col.child(list);
    if shown == 0 && !model.rows.is_empty() {
        col = col.child(search_empty(
            theme,
            SharedString::from(crate::wallet::fill(
                &model.empty_text,
                "query",
                query.trim(),
            )),
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
            .text_size(theme::text_row_title())
            .font_weight(gpui::FontWeight::SEMIBOLD)
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
            // Centred 11 subtle, as the web's `.warning` (078 F-11).
            .text_size(theme::text_label())
            .text_center()
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
    copy: Option<CopyAction>,
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
                .text_size(theme::text_amount_detail())
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
        let button = fact
            .copy
            .clone()
            .zip(copy.as_ref())
            .map(|(text, copy)| copy.button(format!("fact:{i}"), text));
        col = col.child(fact_row(theme, icons, identicons, fact, button));
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
    // Under the explorer, filled in the danger colour (the web's
    // `variant="danger"`).
    // It removes the local record only; the chain keeps the transaction.
    if let Some(label) = &model.delete_label {
        col = col.child(clickable(
            "tx-delete",
            delete_tx,
            danger_button(theme, label.clone()),
        ));
    }
    col
}

fn assets(
    model: &AssetsPanel,
    theme: &Theme,
    icons: &mut IconCache,
    window: &Window,
    search: Option<AddressField>,
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
    let query = search.as_ref().map(|f| f.value.clone()).unwrap_or_default();
    col = col.child(flow_search(
        theme,
        icons,
        model.search_placeholder.clone(),
        search,
        window,
    ));

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
                    .rounded(px(16.))
                    .border_1()
                    .border_color(theme.divider)
                    // The web's `HintCard` (078 F-11): the button first, then
                    // the question 13 semibold and the answer 11 on 1.6. The
                    // DT4L mock drew the button last; the web — this spec's
                    // reference — leads with it.
                    .child(div().pb(px(8.)).child(clickable(
                        "assets-empty-cta",
                        open_add_token,
                        ghost_button(theme, empty.cta.clone()),
                    )))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(empty.hint_title.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .line_height(gpui::relative(1.6))
                            .text_color(theme.fg_muted)
                            .child(empty.hint_body.clone()),
                    ),
            );
    }

    let mut shown = 0;
    // One list, row against row with a hairline between, as the web's
    // `Assets` (078 F-11) — the column gap made each row 76 tall.
    let mut list = div().flex().flex_col();
    for (i, row) in model.rows.iter().enumerate() {
        if !search_matches(&query, &format!("{} {}", row.ticker, row.chain)) {
            continue;
        }
        shown += 1;
        list = list.child(
            div()
                .when(shown > 1, |el| el.border_t_1().border_color(theme.divider))
                .child(asset_row(
                    ElementId::from(("flow-asset", i)),
                    theme,
                    icons,
                    row,
                )),
        );
    }
    col = col.child(list);
    if shown == 0 && !model.rows.is_empty() {
        col = col.child(search_empty(theme, model.no_match.clone()));
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

/// DT3L's live bindings; every one `None`/empty in the gallery.
struct AddTokenActions {
    field: Option<AddressField>,
    submit: Option<Click>,
    tabs: Option<(Click, Click)>,
    picks: Vec<Click>,
}

fn add_token(
    model: &AddToken,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    actions: AddTokenActions,
) -> Div {
    let AddTokenActions {
        field: address_field,
        submit: add_to_wallet,
        tabs,
        picks,
    } = actions;
    let mut col = column().child(segmented_toggle(
        theme,
        "add-token-tab",
        model.tab_erc20.clone(),
        model.tab_native.clone(),
        !model.native,
        tabs,
    ));

    if let Some((mark, name)) = &model.network {
        col = col.child(
            div()
                // Raised, with the coin's own circle (078 F-11) — the web's
                // network select, not a sunken well with a bare ticker.
                .flex()
                .items_center()
                .gap(px(12.))
                .p(px(12.))
                .rounded(px(12.))
                .bg(theme.bg_raised)
                .child(crate::wallet::components::token_icon_logos(
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
            // The field's own error line, red under a red hairline — the
            // text field's one error slot (the web's MonoField `error`).
            let strings = crate::ui::NameFieldStrings {
                label: model.field_label.clone(),
                placeholder: if model.field_placeholder.is_empty() {
                    field.placeholder.clone()
                } else {
                    model.field_placeholder.clone()
                },
                helper: SharedString::from(""),
                too_long_hint: model.field_error.clone().unwrap_or_default(),
            };
            crate::ui::text_field(
                "add-token-address",
                theme,
                &strings,
                &field.value,
                model.field_error.is_some(),
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

    // The web's `.name` / `.detail`: a semibold title over a muted line.
    let text = |name: &SharedString, detail: &SharedString| {
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(detail.clone()),
            )
    };
    col = match &model.result {
        AddTokenResult::Empty => col,
        // A line, not a card: nothing has been found to put in one.
        AddTokenResult::Note(line) => col.child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(line.clone()),
        ),
        AddTokenResult::Token {
            mark,
            name,
            detail,
            chip,
        } => {
            let mut card = div()
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
                .child(text(name, detail));
            if let Some(chip) = chip {
                card = card.child(status_chip(theme, chip));
            }
            col.child(card)
        }
        AddTokenResult::Suggestions(rows) => {
            let mut list = div().flex().flex_col();
            let mut picks = picks.into_iter();
            for (index, row) in rows.iter().enumerate() {
                let mut line = div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .py(px(8.))
                    .child(token_icon_logos(
                        theme,
                        row.mark.ticker.as_ref(),
                        row.mark.badge,
                        &row.mark.logos,
                    ))
                    .child(text(&row.name, &row.meta))
                    .child(icon_img(
                        icons,
                        Icon::ChevronRight,
                        false,
                        theme.fg_subtle,
                        12.,
                    ));
                if index > 0 {
                    line = line.border_t_1().border_color(theme.border_card);
                }
                list = list.child(clickable(
                    ElementId::from(("add-token-pick", row.chain_id as usize)),
                    picks.next(),
                    line,
                ));
            }
            col.child(list)
        }
        AddTokenResult::Network {
            mark,
            name,
            chip,
            link,
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
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(name.clone()),
                        )
                        .child(status_chip(theme, chip)),
                );
            if let Some(link) = link {
                card = card.child(
                    div()
                        .pt(px(8.))
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(link.clone()),
                );
            }
            for fact in facts {
                card = card.child(fact_row(theme, icons, identicons, fact, None));
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

    // Nothing new to add, and the button says so rather than taking a
    // press that does nothing (the web's `ctaDisabled`).
    if model.cta_disabled {
        return col.child(disabled_accent_button(theme, model.cta.clone()));
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
        // The web's `loading` (078 F-09): full emphasis, the words kept for
        // the width but not shown, a spinner where they were — busy is a
        // wait, not a refusal, and a faded button read as one.
        CtaState::Busy => clickable(
            id,
            None,
            div()
                .relative()
                .child(button.text_color(gpui::transparent_black()))
                .child(
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(crate::ui::spinner(
                            gpui::Hsla::from(gpui::rgb(0xffffff)),
                            px(20.),
                            px(1.5),
                        )),
                ),
        ),
        CtaState::Disabled => clickable(id, None, button.opacity(0.45)),
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

#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
fn send_pick(
    model: &SendPick,
    theme: &Theme,
    icons: &mut IconCache,
    window: &Window,
    search: Option<AddressField>,
    mut open_form: Option<Click>,
    per_row: Vec<Click>,
    chip_clicks: Vec<Click>,
    select_all: Option<Click>,
    cta: Option<Click>,
    notice_action: Option<Click>,
) -> Div {
    // No network pill here: the sidebar's network filter already narrows
    // these rows, and a second one beside it could disagree with the first.
    // A locked request nobody can fulfil: the refusal and its way out, and
    // nothing to pick (078 W-04).
    if let Some(notice) = &model.lock_notice {
        let _ = (per_row, chip_clicks, select_all, cta, open_form.take());
        return column().child(notice_card(notice, theme, notice_action, None));
    }
    let query = search.as_ref().map(|f| f.value.clone()).unwrap_or_default();
    let mut col = column()
        .child(flow_search(
            theme,
            icons,
            model.search_placeholder.clone(),
            search,
            window,
        ))
        .child(filter_chips(theme, &model.filters, chip_clicks));

    // SD1b's chain lock, in the corpus's own sentence: the first pick names
    // the network and the greying that follows is explained rather than left
    // to be guessed at.
    if let Some((chain_id, colour, letter, text)) = model
        .selection
        .as_ref()
        .and_then(|selection| selection.notice.clone())
    {
        // The web's `NoticeBanner` (078 F-10): raised, padded 8/12, radius 12,
        // 11 on 1.4, with the chain's own logo — the tinted letter only while
        // it loads or where there is none.
        let mark = move || {
            crate::settings::components::chain_mark(letter.clone(), colour, 20.).into_any_element()
        };
        let logo = match crate::marks::chain_logo_url(chain_id) {
            Some(url) => gpui::img(url)
                .size(px(20.))
                .rounded_full()
                .with_loading(mark.clone())
                .with_fallback(mark)
                .into_any_element(),
            None => mark(),
        };
        col = col.child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .py(px(8.))
                .px(px(12.))
                .rounded(px(12.))
                .bg(theme.bg_raised)
                .child(div().flex_none().child(logo))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_label())
                        .line_height(gpui::relative(crate::wallet::components::LINE_BODY))
                        .text_color(theme.fg_muted)
                        .child(text),
                ),
        );
    }

    // A live panel binds one listener per row; the fixture binds one and
    // gives it to the first, because every mock row opens the same drawing.
    // Indexed, not iterated: a search hides rows, and row N must still open
    // token N.
    let mut per_row = per_row.into_iter().map(Some).collect::<Vec<_>>();
    let mut shown = 0;
    // One list, row against row with a hairline between (the web's `li + li`,
    // 078 F-10) — the panel's 12 gap between rows made each 76 tall.
    let mut list = div().flex().flex_col();
    for (i, row) in model.rows.iter().enumerate() {
        if !search_matches(&query, &format!("{} {}", row.ticker, row.chain)) {
            continue;
        }
        shown += 1;
        let action = per_row
            .get_mut(i)
            .and_then(Option::take)
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
        // Selected lifts the whole row, bleeding 8 past the margin at radius
        // 12, as the web's `.selected` does — not an accent tint.
        let mut wrapper = div()
            .when(shown > 1, |el| el.border_t_1().border_color(theme.divider))
            .when(selected, |el| {
                el.rounded(px(12.))
                    .bg(theme.bg_raised)
                    .px(px(8.))
                    .mx(px(-8.))
            });
        wrapper = wrapper.child(if dimmed {
            // Off-network rows are readable and inert. The core refuses them
            // anyway; drawing them as pressable would be an offer it declines.
            drawn.opacity(0.45).into_any_element()
        } else {
            clickable(ElementId::from(("flow-send-row", i)), action, drawn).into_any_element()
        });
        list = list.child(wrapper);
    }
    col = col.child(list);
    if shown == 0 && !model.rows.is_empty() {
        col = col.child(search_empty(theme, model.no_match.clone()));
    }

    if let Some(selection) = model.selection.as_ref() {
        col = col.child(clickable(
            "flow-send-select-all",
            select_all,
            div()
                .py(px(4.))
                .text_size(theme::text_label())
                .text_color(theme.fg_muted)
                .child(selection.select_all.clone()),
        ));
    }

    // A real button, as the web's `Button` (078 F-10): the secondary pill —
    // outlined, muted — while this is the other journey, the primary once
    // tokens are ticked and it is the one being taken. 52 tall, 17 semibold,
    // padded 8 above and 16 below.
    let face = div()
        .h(px(52.))
        .flex()
        .items_center()
        .justify_center()
        .px(px(24.))
        .text_size(theme::text_button())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .line_height(gpui::relative(1.2))
        .child(model.cta.clone());
    let face = if model.cta_accent {
        face.rounded(px(12.))
            .bg(theme.accent)
            .text_color(theme.fg_inverse)
    } else {
        face.rounded_full()
            .border_1()
            .border_color(theme.border_strong)
            .text_color(theme.fg_muted)
    };
    col.child(
        div()
            .pt(px(8.))
            .pb(px(16.))
            .child(clickable("flow-send-cta", cta, face)),
    )
}

fn send_form(
    model: &SendForm,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    actions: PanelActions,
) -> Div {
    let (body, foot) = send_form_parts(model, theme, icons, identicons, window, actions);
    body.child(foot)
}

/// The form, and its foot — the total (in a split), the refusal and
/// Continue, in the web's order: after the fee, together (078 F-09).
fn send_form_parts(
    model: &SendForm,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    mut actions: PanelActions,
) -> (Div, Div) {
    let (mark, symbol, detail, max) = &model.token;
    // The speed control's listeners, taken before the split rows borrow the
    // rest of `actions`.
    let toggle_speed = actions.toggle_speed.take();
    let pick_speed = std::mem::take(&mut actions.pick_speed_rows);
    // Max lives on the token card, as the web's `TokenHeaderCard` draws it. A
    // live form makes that one chip the button rather than drawing a second,
    // working one under the field (#288).
    // A sweep has no one token and no one figure (078 F-05): what stands
    // there is the summary and every picked coin at what it will move.
    if model.sweep.is_some() || model.amount.is_none() {
        actions.amount_field = None;
        actions.tap_max = None;
    }
    // A split's people are its rows (the web's `mode === 'split'` has no
    // single field): the one-recipient card above them was the single form
    // left standing, with nothing to say who it was for.
    if model.recipient.is_none() {
        actions.recipient_field = None;
    }
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
    let mut col = match &model.sweep {
        None => column().child(card),
        Some(sweep) => {
            let mut rows = div().flex().flex_col();
            for (i, row) in sweep.rows.iter().enumerate() {
                rows = rows.child(
                    div()
                        .id(ElementId::from(("sweep-row", i)))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .py(px(12.))
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
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.fg_base)
                                        .child(row.symbol.clone()),
                                )
                                .child(
                                    div()
                                        .text_size(theme::text_row_sub())
                                        .text_color(theme.fg_subtle)
                                        .child(row.balance.clone()),
                                ),
                        )
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(theme.fg_base)
                                .child(row.amount.clone()),
                        ),
                );
            }
            column()
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(sweep.summary.clone()),
                )
                .child(rows)
        }
    };

    if let Some(field) = actions.amount_field.take() {
        // Live: the web's `AmountInput` — the figure large and centred with
        // its unit after it, and the other denomination under it. The value
        // is the CORE's — it validates every keystroke — so the field holds
        // no copy.
        let unit = model
            .amount_unit
            .clone()
            .unwrap_or_else(|| model.token.1.clone());
        let mut block = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(4.))
            .py(px(24.))
            .child(crate::ui::hero_amount_field(
                "send-amount",
                theme,
                &field.value,
                field.placeholder.clone(),
                unit,
                &field.focus,
                window,
                field.on_change,
            ));
        if let Some((_, fiat)) = &model.amount {
            let line = div()
                .flex()
                .items_center()
                .gap(px(4.))
                .text_size(theme::text_row_title())
                .text_color(theme.fg_muted)
                .child(fiat.clone());
            block = block.child(match model.denom_toggle {
                // The chevrons ARE the other denomination's line (the web's
                // `button.fiat`): pressing what it shows is how the figure
                // comes across. Where the swap would change nothing it is
                // drawn dimmed and answers to nothing — the notice below says
                // why.
                Some(enabled) => clickable(
                    "send-denom-toggle",
                    if enabled {
                        actions.toggle_denom.take()
                    } else {
                        None
                    },
                    line.when(!enabled, |el| el.opacity(0.5)).child(
                        div()
                            .flex()
                            .flex_col()
                            .child(icon_img(icons, Icon::ChevronUp, false, theme.fg_muted, 10.))
                            .child(icon_img(
                                icons,
                                Icon::ChevronDown,
                                false,
                                theme.fg_muted,
                                10.,
                            )),
                    ),
                ),
                None => line,
            });
        }
        col = col.child(block);
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
        // Live: the web's `RecipientField` — a quiet label, then one raised
        // card holding the payee's face, the address as it is typed, and the
        // book. The label carries the core's trust line once it has one.
        let label = model
            .recipient
            .as_ref()
            .map(|(label, _, _)| label.clone())
            .unwrap_or_default();
        // A face only for a whole address: a half-typed one would change
        // creature on every keystroke, and none of them would be anybody.
        let whole = field.value.len() == 42 && field.value.starts_with("0x");
        let face = if whole {
            div()
                .flex_none()
                .child(crate::wallet::components::identicon_avatar(
                    identicons,
                    &field.value,
                    36.,
                ))
        } else {
            div()
                .flex_none()
                .size(px(36.))
                .rounded_full()
                .bg(theme.bg_sunken)
        };
        let mut card = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(12.))
            .bg(theme.bg_raised)
            .child(face)
            .child(crate::ui::bare_text_field(
                "send-recipient",
                theme,
                &field.value,
                field.placeholder.clone(),
                &field.focus,
                window,
                field.on_change,
            ));
        if model.pick_contacts.is_some() {
            card = card.child(clickable(
                "flow-pick-contacts",
                actions.open_contact_pick.take(),
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded_full()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(icon_img(
                        icons,
                        Icon::NavContacts,
                        false,
                        theme.fg_muted,
                        18.,
                    )),
            ));
        }
        // The web's `RecipientField` scan button (078 F-01): after the book,
        // the same 36 round button with an 18 glyph — the address that is on
        // a screen, not in the book. Drawn only when a scan is bound: the
        // gallery's form has no camera behind it.
        if let Some(open_scan) = actions.open_scan.take() {
            card = card.child(clickable(
                "flow-scan-recipient",
                Some(open_scan),
                div()
                    .flex()
                    .items_center()
                    .justify_center()
                    .size(px(36.))
                    .rounded_full()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(icon_img(icons, Icon::QrCode, false, theme.fg_muted, 18.)),
            ));
        }
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    // `--text-sm`, as the web's `RecipientField` label.
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(label),
                )
                .child(card)
                // The trust line under the field (`RecipientField`'s `.note`,
                // 078 F-08): who the core says this is, or that it is new.
                .children(model.recipient_note.clone().map(|note| {
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(note)
                })),
        );
    } else if let Some((label, lines, seed)) = &model.recipient {
        // The drawn form: the same raised card as the live one, holding the
        // address it was given. Clicking it opens the book, as the mock does.
        let line = |text: SharedString| {
            div()
                .font_family(theme::font_mono())
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .whitespace_nowrap()
                .child(text)
        };
        let card = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .p(px(12.))
            .rounded(px(12.))
            .bg(theme.bg_raised)
            .child(
                div()
                    .flex_none()
                    .child(crate::wallet::components::identicon_avatar(
                        identicons,
                        seed.as_ref(),
                        36.,
                    )),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(line(lines.0.clone()))
                    .child(line(lines.1.clone())),
            )
            .child(
                div()
                    .flex_none()
                    .size(px(36.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(
                        icons,
                        Icon::NavContacts,
                        false,
                        theme.fg_muted,
                        18.,
                    )),
            );
        col = col.child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(label.clone()),
                )
                .child(clickable(
                    "flow-recipient",
                    actions.open_contact_pick.take(),
                    card,
                )),
        );
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
    let mut addresses = actions.split_address_fields.drain(..);
    let mut picks = actions.pick_recipient_rows.drain(..);
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
                address: addresses.next(),
                pick: picks.next(),
                remove: removes.next(),
            },
            window,
        ));
    }

    // One amount into every row that has none (the web's `.fill`): a quiet
    // underlined line under the rows, not a button competing with Continue.
    if let Some(label) = &model.fill_empty
        && let Some(action) = actions.fill_empty.take()
    {
        col = col.child(
            div().flex().child(clickable(
                "split-fill-empty",
                Some(action),
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .py(px(6.))
                    .child(icon_img(icons, Icon::Copy, false, theme.fg_base, 14.))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .font_weight(gpui::FontWeight::MEDIUM)
                            .text_color(theme.fg_base)
                            .underline()
                            .child(label.clone()),
                    ),
            )),
        );
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

    let mut foot = div().flex().flex_col().gap(px(4.));
    // The web's `SummaryLine` (078 F-09): the label 11 muted with what is
    // left under it, the total 15 bold at the end — the figure the eye goes
    // to, where the desktop printed it as a 13 regular line.
    if let Some((label, value)) = &model.summary {
        foot = foot.child(
            div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .py(px(8.))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_muted)
                                .child(label.clone()),
                        )
                        .when_some(model.remaining.clone(), |el, left| {
                            el.child(
                                div()
                                    .text_size(theme::text_glyph())
                                    .text_color(theme.fg_muted)
                                    .child(left),
                            )
                        }),
                )
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::BOLD)
                        // Over the balance, the total says so in ink before
                        // Continue has to refuse it.
                        .text_color(if model.summary_over {
                            theme.error_base
                        } else {
                            theme.fg_base
                        })
                        .child(value.clone()),
                ),
        );
    }

    // The refusal where the eye is when the button did nothing: one line, 11
    // on 1.4 with a 14 icon, as the web's `.alert` (078 F-09) — the sentence
    // and what can be sent instead, no heading and no "Edit amount": the
    // amount is the field right above it. The one exception is the relay's
    // treasury stop, which the web raises as its own sheet with an address to
    // fund and a way to close it; here it stays the card that holds those.
    if let Some(notice) = &model.notice {
        let one_line = notice.dismiss.is_none();
        foot = foot.child(if one_line {
            alert_line(theme, icons, notice)
        } else {
            notice_card(
                notice,
                theme,
                actions.notice_action.take(),
                actions.notice_dismiss.take(),
            )
        });
    }
    // The fee row, and beside it the refresh control — outside the row's
    // own click, so measuring again never opens the fee-coin sheet.
    // One raised surface, as the web's `FeeRow` draws it: the row that opens
    // the fee-coin sheet, and the refresh at its end — two controls side by
    // side, not one inside the other, so measuring again never opens the
    // sheet.
    let mut fee_line = div()
        .flex()
        .items_center()
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .child(div().flex_1().min_w(px(0.)).child(clickable(
            "flow-fee-row",
            actions.open_fee_token.take(),
            fee_row(theme, icons, &model.fee),
        )));
    if model.fee.refresh.is_some() {
        fee_line = fee_line.child(clickable(
            "flow-fee-refresh",
            actions.refresh_fee.take(),
            fee_refresh_icon(theme, icons, &model.fee),
        ));
    }
    let mut fee_block = div().flex().flex_col().gap(px(4.)).child(fee_line);
    if model.fee.refresh.is_some() {
        fee_block = fee_block.child(fee_stale_line(theme, &model.fee));
    }
    if let Some(speed) = &model.speed {
        fee_block = fee_block.child(speed_control(theme, icons, speed, toggle_speed, pick_speed));
    }
    let foot = foot.child(div().pt(px(8.)).pb(px(16.)).child(cta_button(
        "flow-form-cta",
        theme,
        model.cta.clone(),
        model.cta_state,
        actions.advance.take(),
    )));
    (col.child(fee_block), foot)
}

/// The core's refusal as the web's form prints it: a 14 icon and the words,
/// 11 on 1.4 — red when nothing can proceed, amber while still typing (the
/// desktop's own two tones, which the core's `error` flag carries).
fn alert_line(theme: &Theme, icons: &mut IconCache, notice: &SendNotice) -> Div {
    let color = if notice.error {
        theme.error_base
    } else {
        theme.warning
    };
    div()
        .flex()
        .items_start()
        .gap(px(4.))
        .text_size(theme::text_label())
        .line_height(gpui::relative(crate::wallet::components::LINE_BODY))
        .text_color(color)
        .child(div().flex_none().mt(px(1.)).child(icon_img(
            icons,
            Icon::CircleAlert,
            false,
            color,
            14.,
        )))
        .child(div().flex_1().min_w(px(0.)).child(match &notice.detail {
            Some(detail) => SharedString::from(format!("{} {detail}", notice.body)),
            None => notice.body.clone(),
        }))
}

/// The speed control under the fee row (spec 068): folded, the word and the
/// tier in force; open, the one-shot promise and three options — or, on a
/// network with one speed, that one statement instead.
pub fn speed_control(
    theme: &Theme,
    icons: &mut IconCache,
    speed: &FeeSpeedModel,
    toggle: Option<Click>,
    picks: Vec<Click>,
) -> Div {
    let mut block = div().flex().flex_col().child(clickable(
        "flow-speed-summary",
        toggle,
        fee_speed_summary(theme, icons, speed),
    ));
    // Folded AND open: the screen must never say "Fast" over a Settings row
    // that says "Slow" without saying why.
    if let Some(free) = &speed.free_note {
        block = block.child(fee_speed_note(theme, free));
    }
    if !speed.open {
        return block;
    }
    if let Some(single) = &speed.single_note {
        return block.child(fee_speed_note(theme, single));
    }
    // Said BEFORE the options: somebody about to change one payment's speed
    // needs to know first that every later payment is untouched.
    block = block.child(fee_speed_note(theme, &speed.once_note));
    let mut picks = picks.into_iter();
    for (index, option) in speed.options.iter().enumerate() {
        block = block.child(clickable(
            ("flow-speed-option", index),
            picks.next(),
            fee_speed_option(theme, icons, speed, option),
        ));
    }
    block
}

#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
fn contact_pick(
    model: &ContactPick,
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    window: &Window,
    search: Option<AddressField>,
    open_scan: Option<Click>,
    per_row: Vec<Click>,
    mut group_rows: Vec<Click>,
) -> Div {
    let query = search.as_ref().map(|f| f.value.clone()).unwrap_or_default();
    // Indexed, not iterated: a search hides rows, and row N must still pick
    // contact N.
    let mut per_row = per_row.into_iter().map(Some).collect::<Vec<_>>();
    let mut col = column()
        .child(flow_search(
            theme,
            icons,
            model.search_placeholder.clone(),
            search,
            window,
        ))
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
                .bg(theme.bg_raised)
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
        ));

    // Groups only while nothing is typed, and only when there are some: a
    // search is for a person, and the web drops the section as the query
    // starts (`ContactPick.svelte`).
    let groups: &[_] = if query.trim().is_empty() {
        &model.groups
    } else {
        &[]
    };
    if !groups.is_empty() {
        col = col.child(
            div()
                .pt(px(8.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(model.groups_title.clone()),
        );
    }
    let mut per_group = std::mem::take(&mut group_rows).into_iter();
    for (index, (name, count, first, second)) in groups.iter().enumerate() {
        // The web's group row (078 F-11): 30 discs overlapping 12, the name
        // 15 semibold, the count 11, a 14 chevron; padded 12 on a button's
        // line.
        let row = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(12.))
            .line_height(gpui::relative(crate::wallet::components::LINE_NORMAL))
            // Two overlapping discs stand for "several people" without
            // drawing any of them — a group has no single face to show.
            .child(
                div()
                    .flex()
                    .flex_none()
                    .child(div().size(px(30.)).rounded_full().bg(*first))
                    .child(div().size(px(30.)).rounded_full().bg(*second).ml(px(-12.))),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(count.clone()),
            )
            .child(icon_img(
                icons,
                Icon::ChevronRight,
                false,
                theme.fg_subtle,
                14.,
            ));
        col = col.child(clickable(
            ElementId::from(("flow-group", index)),
            per_group.next(),
            row,
        ));
    }

    col = col.child(
        div()
            .pt(px(8.))
            .text_size(theme::text_label())
            .text_color(theme.fg_subtle)
            .child(model.contacts_title.clone()),
    );

    for (i, contact) in model.contacts.iter().enumerate() {
        if !search_matches(&query, &format!("{} {}", contact.name, contact.address)) {
            continue;
        }
        // The web's `ContactPickRow` (078 F-11): a 30 avatar, the name 15
        // semibold with its group as a raised 10 tag, the address mono 11,
        // a 14 chevron; padded 12 on a button's line.
        let mut name_row = div().flex().items_center().gap(px(4.)).min_w(px(0.)).child(
            div()
                .min_w(px(0.))
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .truncate()
                .child(contact.name.clone()),
        );
        if let Some(group) = &contact.group {
            name_row = name_row.child(
                div()
                    .flex_none()
                    .px(px(4.))
                    .rounded(px(4.))
                    .bg(theme.bg_raised)
                    .text_size(theme::text_glyph())
                    .text_color(theme.fg_muted)
                    .child(group.clone()),
            );
        }
        let entry = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .py(px(12.))
            .line_height(gpui::relative(crate::wallet::components::LINE_NORMAL))
            .child(crate::wallet::components::identicon_avatar(
                identicons,
                contact.seed.as_ref(),
                30.,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(name_row)
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .truncate()
                            .child(contact.address.clone()),
                    ),
            )
            .child(icon_img(
                icons,
                Icon::ChevronRight,
                false,
                theme.fg_subtle,
                14.,
            ));
        col = col.child(clickable(
            ElementId::from(("flow-contact", i)),
            per_row.get_mut(i).and_then(Option::take),
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
    let col = column().child(
        // Paying gas in a stablecoin is unusual enough that someone seeing USDC
        // offered as a fee token will wonder whether they are being asked to
        // send it. Saying what the choice is for, once, is cheaper than a
        // tooltip on each row.
        div()
            .text_size(theme::text_label())
            .text_color(theme.fg_muted)
            .child(model.hint.clone()),
    );
    // One list, the rows flush (the web's `ul`); each padded 12 on a
    // button's line, the chosen one raised (078 F-11).
    let mut list = div().flex().flex_col();
    for (i, row) in model.rows.iter().enumerate() {
        let shell = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(12.))
            .line_height(gpui::relative(crate::wallet::components::LINE_NORMAL));
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
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(row.symbol.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_label())
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
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_glyph())
                            .text_color(theme.fg_subtle)
                            .child(model.estimate_label.clone()),
                    ),
            );
        // Only the chosen row draws the tick; the others keep its 20 column
        // (the web's `.tick`, always laid out), so choosing one does not
        // shift the figures under it — they lined up with nothing before.
        let mut tick = div().w(px(20.)).flex_none().flex().justify_center();
        if row.selected {
            tick = tick.child(icon_img(icons, Icon::Check, false, theme.accent, 16.));
        }
        entry = entry.child(tick);
        // A coin that cannot cover the fee is shown for context and answers
        // to nothing (invariant ⑧) — the listener is dropped, not just dimmed.
        let action = per_row.next();
        let (entry, action) = if row.insufficient {
            (entry.opacity(0.45), None)
        } else {
            (entry, action)
        };
        list = list.child(clickable(
            ElementId::from(("flow-fee-row", i)),
            action,
            entry,
        ));
    }
    col.child(list)
}

fn batch_import(
    model: &BatchImport,
    theme: &Theme,
    icons: &mut IconCache,
    window: &Window,
    mut actions: PanelActions,
) -> Div {
    let toggle = segmented_toggle(
        theme,
        "batch-unit",
        model.unit_fiat.clone(),
        model.unit_token.clone(),
        model.fiat_on,
        actions.batch_unit.take(),
    );
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
                .text_size(theme::text_amount_detail())
                .line_height(gpui::relative(1.2))
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
        .bg(theme.bg_raised);
    for (i, fact) in model.facts.iter().enumerate() {
        if i > 0 {
            card = card.child(divider(theme));
        }
        card = card.child(fact_row(theme, icons, identicons, fact, None));
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
    column_h: f32,
    copy: Option<CopyAction>,
    advance: Option<Click>,
) -> Div {
    let mut col = column().child(status_hero(
        theme,
        icons,
        model.stage,
        model.progress,
        &model.title,
        &model.captions,
    ));

    if !model.breakdown.is_empty() {
        col = col.child(div().pt(px(12.)).child(breakdown_list(
            theme,
            identicons,
            model.breakdown_title.as_ref(),
            &model.breakdown,
        )));
    }

    // The foot (`SendReceipt.svelte`): the hash's two ends with a copy that
    // ticks — the whole hash on the clipboard, 66 characters on one line ran
    // off the column — then the explorer, then the one button.
    let mut foot = div().flex().flex_col().gap(px(8.)).pt(px(24.)).pb(px(16.));
    if let Some((label, value)) = &model.hash {
        let button = copy
            .as_ref()
            .map(|copy| copy.button("receipt-hash", value.clone()));
        let copied = button.as_ref().is_some_and(|button| button.copied);
        let glyph = icon_img(
            icons,
            if copied { Icon::Check } else { Icon::Copy },
            false,
            if copied {
                theme.success_base
            } else {
                theme.fg_subtle
            },
            14.,
        );
        foot = foot.child(
            div()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(4.))
                .pb(px(8.))
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(label.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_base)
                        .child(SharedString::from(crate::wallet::live::shorten_address(
                            value,
                        ))),
                )
                .child(clickable(
                    "receipt-hash-copy",
                    button.map(|button| button.on_click),
                    div().flex().items_center().child(glyph),
                )),
        );
    }
    if let Some((label, url)) = &model.explorer {
        foot = foot.child(explorer_button(
            "receipt-explorer",
            theme,
            label.clone(),
            Some(url),
        ));
    }
    // The CTA is the accent "Done" once the money has landed, and the quiet
    // "Close · keep running" before — load-bearing copy: the transaction does
    // not depend on this panel staying open.
    foot = foot.child(clickable(
        "flow-receipt-cta",
        advance,
        if model.cta_accent {
            accent_button(theme, model.cta.clone())
        } else {
            ghost_button(theme, model.cta.clone())
        },
    ));
    // Issue 199 (the web's `layout="column"`): in a window-tall column the
    // status and the button pinned apart are most of a screen away from each
    // other. One group, 2:3 above the middle — the optical centre, where a
    // dialog would sit. A long split outgrows it and the spacers give way.
    div()
        .min_h(px(column_h.max(0.)))
        .flex()
        .flex_col()
        .child(div().flex_1().flex_grow(2.))
        .child(col.child(foot))
        .child(div().flex_1().flex_grow(3.))
}

/// The receipt's centrepiece — the web's `StatusHero`: one 88 disc for all
/// four stages, so the mark does not resize as the transaction moves — a
/// spinner while submitting, a clock while submitted, a tick once confirmed,
/// a cross on failure. Submitted and confirmed wear a ring OUTSIDE the disc
/// (104 across, 2.5 stroke): it fills as the chain's usual time passes and is
/// the same ring that closes and turns green on the confirmation, so the tick
/// arrives as the end of what the person was watching. Without an estimate
/// the ring roams rather than filling.
pub fn status_hero(
    theme: &Theme,
    icons: &mut IconCache,
    stage: crate::flows::fixtures::ReceiptStage,
    progress: Option<f32>,
    title: &SharedString,
    captions: &[SharedString],
) -> Div {
    use crate::flows::fixtures::ReceiptStage;
    use gpui::{Animation, AnimationExt as _, PathBuilder, Point, canvas};

    let (bg, fg) = match stage {
        ReceiptStage::Submitting => (theme.bg_sunken, theme.accent),
        ReceiptStage::Submitted => (theme.bg_sunken, theme.fg_muted),
        ReceiptStage::Confirmed => (theme.success_soft, theme.success_base),
        ReceiptStage::Failed => (theme.error_soft, theme.error_base),
    };
    let mark: gpui::AnyElement = match stage {
        ReceiptStage::Submitting => crate::ui::spinner(fg, px(30.), px(1.5)),
        ReceiptStage::Submitted => icon_img(icons, Icon::Clock, false, fg, 26.).into_any_element(),
        ReceiptStage::Confirmed => icon_img(icons, Icon::Check, false, fg, 26.).into_any_element(),
        ReceiptStage::Failed => icon_img(icons, Icon::X, false, fg, 26.).into_any_element(),
    };

    // An arc of `sweep` (0–1 of a turn) from `start` (0–1, 0 = 12 o'clock),
    // clockwise, on a circle of radius `r` about the bounds' centre.
    fn paint_arc(
        bounds: gpui::Bounds<gpui::Pixels>,
        start: f32,
        sweep: f32,
        color: gpui::Hsla,
        window: &mut Window,
    ) {
        let stroke = 2.5_f32;
        let cx = f32::from(bounds.origin.x) + f32::from(bounds.size.width) / 2.;
        let cy = f32::from(bounds.origin.y) + f32::from(bounds.size.height) / 2.;
        let r = f32::from(bounds.size.width) / 2. - stroke;
        let at = |turn: f32| {
            let a = (turn - 0.25) * std::f32::consts::TAU;
            Point::new(px(cx + r * a.cos()), px(cy + r * a.sin()))
        };
        let sweep = sweep.clamp(0., 1.);
        if sweep <= 0.001 {
            return;
        }
        let mut pb = PathBuilder::stroke(px(stroke));
        pb.move_to(at(start));
        // Two halves when it is more than half a turn: one SVG arc cannot
        // draw a full circle.
        let mut from = 0_f32;
        while from < sweep {
            let to = (from + 0.49).min(sweep);
            pb.arc_to(
                Point::new(px(r), px(r)),
                px(0.),
                false,
                true,
                at(start + to),
            );
            from = to;
        }
        if let Ok(path) = pb.build() {
            window.paint_path(path, color);
        }
    }

    let track = theme.border_card;
    let (arc_color, ringed) = match stage {
        ReceiptStage::Submitted => (theme.accent, true),
        ReceiptStage::Confirmed => (theme.success_base, true),
        _ => (theme.accent, false),
    };
    let mut disc = div()
        .relative()
        .size(px(88.))
        .flex_none()
        .rounded_full()
        .bg(bg)
        .flex()
        .items_center()
        .justify_center()
        .mb(px(16.));
    if ringed {
        let confirmed = stage == ReceiptStage::Confirmed;
        let ring = div().absolute().top(px(-8.)).left(px(-8.)).size(px(104.));
        disc = disc.child(match (confirmed, progress) {
            (false, None) => ring
                .with_animation(
                    "receipt-ring-roam",
                    Animation::new(std::time::Duration::from_millis(2400)).repeat(),
                    move |el, delta| {
                        el.child(
                            canvas(
                                |_, _, _| (),
                                move |bounds, _, window, _| {
                                    paint_arc(bounds, 0., 1., track, window);
                                    paint_arc(bounds, delta, 0.25, arc_color, window);
                                },
                            )
                            .size_full(),
                        )
                    },
                )
                .into_any_element(),
            (_, drawn) => {
                let drawn = if confirmed { 1. } else { drawn.unwrap_or(0.25) };
                ring.child(
                    canvas(
                        |_, _, _| (),
                        move |bounds, _, window, _| {
                            if !confirmed {
                                paint_arc(bounds, 0., 1., track, window);
                            }
                            paint_arc(bounds, 0., drawn, arc_color, window);
                        },
                    )
                    .size_full(),
                )
                .into_any_element()
            }
        });
    }
    disc = disc.child(mark);

    let mut hero = div()
        .flex()
        .flex_col()
        .items_center()
        .gap(px(4.))
        .pt(px(48.))
        .pb(px(24.))
        .child(disc)
        .child(
            div()
                .text_size(theme::text_panel_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .text_center()
                .child(title.clone()),
        );
    for (i, caption) in captions.iter().enumerate() {
        // The first caption is what the person is waiting to read; the rest
        // say "you can leave" and count — true, useful, and quieter.
        hero = hero.child(
            div()
                .text_center()
                .text_size(if i == 0 {
                    theme::text_row_sub()
                } else {
                    theme::text_label()
                })
                .text_color(if i == 0 {
                    theme.fg_muted
                } else {
                    theme.fg_subtle
                })
                .child(caption.clone()),
        );
    }
    hero
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
#[allow(clippy::too_many_arguments, clippy::allow_attributes)]
pub fn scan_modal(
    model: &ScanModal,
    theme: &Theme,
    icons: &mut IconCache,
    mut tool_actions: Vec<Option<Click>>,
    close: Option<Click>,
    preview: Option<std::sync::Arc<gpui::RenderImage>>,
    notice: Option<SharedString>,
    no_camera: bool,
    width: f32,
) -> Div {
    // The web's desktop scanner (`ScanSurface` as `variant="modal"` in the
    // wallet's `.scan-modal`), measured (078 F-03): `min(90vw, 440)` wide,
    // radius 20 on the canvas colour with `shadow-lg`, padding 0/20/20; a
    // 16-high title row with the 17 bold title and a borderless 36 close; the
    // viewfinder the full width at 3:2 with four 30px corner brackets; the
    // hint at 13, left; the tools as equal outlined buttons with no glyphs.
    // It was 560 wide with a fixed 336 well and pills as wide as their words
    // — "按钮很丑" (owner).
    let inner = width - 40.;
    let frame_h = inner * 2. / 3.;

    let mut tools = div().flex().gap(px(8.));
    let mut bound = tool_actions.drain(..);
    for (i, label) in model.tools.iter().enumerate() {
        // Flip needs a camera to flip; without one it is drawn dimmed, as the
        // web dims a torch a webcam does not have, rather than armed and
        // doing nothing.
        let is_flip = i == 1;
        let dim = is_flip && no_camera;
        let tool = div()
            .w_full()
            .flex()
            .items_center()
            .justify_center()
            .py(px(8.))
            .rounded(px(12.))
            .border_1()
            .border_color(theme.border_strong)
            .text_size(theme::text_label())
            .text_color(theme.fg_base)
            .when(dim, |el| el.opacity(theme::OPACITY_DISABLED))
            .child(label.clone());
        let action = bound.next().flatten().filter(|_| !dim);
        // The WRAPPER is the flex item `clickable` hands back, so it is the
        // one that takes the equal share.
        tools = tools.child(clickable(ElementId::from(("scan-tool", i)), action, tool).flex_1());
    }

    // Brackets, not a border: the frame aims the camera, and a closed
    // rectangle around a code competes with the code's own quiet zone.
    let corner = |top: bool, left: bool| {
        let c = div()
            .absolute()
            .size(px(30.))
            .border_color(theme.fg_base)
            .when(top, |el| el.top_0().border_t(px(1.5)))
            .when(!top, |el| el.bottom_0().border_b(px(1.5)))
            .when(left, |el| el.left_0().border_l(px(1.5)))
            .when(!left, |el| el.right_0().border_r(px(1.5)));
        match (top, left) {
            (true, true) => c.rounded_tl(px(8.)),
            (true, false) => c.rounded_tr(px(8.)),
            (false, true) => c.rounded_bl(px(8.)),
            (false, false) => c.rounded_br(px(8.)),
        }
    };
    let mut frame = div().relative().w_full().h(px(frame_h)).child({
        let well = div()
            .absolute()
            .inset_0()
            .rounded(px(8.))
            .overflow_hidden()
            .bg(theme.bg_sunken);
        match preview {
            Some(image) => well.child(
                gpui::img(gpui::ImageSource::Render(image))
                    .w_full()
                    .h(px(frame_h)),
            ),
            None => well,
        }
    });
    for (top, left) in [(true, true), (true, false), (false, true), (false, false)] {
        frame = frame.child(corner(top, left));
    }

    div()
        .w(px(width))
        .px(px(20.))
        .pb(px(20.))
        .rounded(px(20.))
        .bg(theme.bg_base)
        .shadow(crate::ui::dialog::shadow_lg())
        .flex()
        .flex_col()
        .child(
            div()
                .py(px(16.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(theme::text_button())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(model.title.clone()),
                )
                .child(clickable(
                    "scan-close",
                    close,
                    div()
                        .size(px(36.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .hover(|el| el.bg(theme.bg_sunken))
                        .child(icon_img(icons, Icon::X, false, theme.fg_base, 20.)),
                )),
        )
        .child(frame)
        .child(
            // What is true right now: the hint, or — when the hint is not —
            // why (the web's `notice ?? model.hint`).
            div()
                .py(px(12.))
                .text_size(theme::text_row_sub())
                .line_height(gpui::relative(1.4))
                .text_color(theme.fg_muted)
                .child(notice.unwrap_or_else(|| model.hint.clone())),
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
