//! The create journey, as five screens.
//!
//! Everything here is a rendering of `CreateView`. **The mapping in
//! [`Screen::of`] is the whole of the create UI's logic** — there is no other
//! decision in this file, and a client that implements that table has no create
//! logic of its own (data-model §3).
//!
//! What spec 014 had here is gone: the in-place action-column swap, the five
//! `CreatePanelState` variants, the elapsed-seconds ring. The v2 design makes
//! the flow the whole page and keeps a modal for FAILURES only, which is
//! [`crate::outcome`]. What survived is the `ui/` atoms, unchanged — they were
//! built against the same tokens the redesign kept.
//!
//! ## The three gates, and why they are visible
//!
//! The core enforces at most seven keys, every key confirmed, and a second key
//! when the only one is not backed up. Each surfaces here as a disabled control
//! WITH ITS REASON WRITTEN NEXT TO IT rather than as a press that quietly does
//! nothing. A person who cannot finish is owed the sentence that says why.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, Div, FocusHandle, FontWeight, ImageSource, InteractiveElement as _,
    IntoElement as _, ParentElement, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled, Window, div, img, px,
};

use vela_core::app::create_wallet::{CreateKeyRow, CreateStage, CreateView, SubmitLabel};
use vela_core::app::{KeyMethod, StatusKey};

use crate::hardware::METHOD_ICON_PX;
use crate::identicon::IdenticonCache;
use crate::loc::Loc;
use crate::passkey_directory::PasskeyDirectory;
use crate::passkey_icons::{Palette, PasskeyIcon, PasskeyIconCache};
use crate::theme::{
    self, FLOW_GAP_LG, FLOW_GAP_MD, FLOW_GAP_SM, HAIRLINE, OPACITY_DISABLED, RADIUS_FIELD, Theme,
};
use crate::ui::{
    ButtonState, ButtonVariant, NameFieldStrings, ack_row, name_field, spinner, vela_button_opts,
    vela_button_state,
};
use crate::wallet::components::{identicon_avatar, passkey_fallback_mark, passkey_mark};

/// The founding-set cap, mirroring the core's `MAX_MULTI_KEYS`.
pub const MAX_KEYS: usize = 7;

pub use crate::theme::FLOW_COLUMN_W;

/// The privacy and terms URLs the legal acknowledgement links to. Absolute,
/// because they are the marketing site's pages and not app routes.
pub const PRIVACY_URL: &str = "https://getvela.app/privacy";
pub const TERMS_URL: &str = "https://getvela.app/terms";

// ---------------------------------------------------------------------------
// Screen selection — the whole of the create UI's logic
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Name,
    Keys,
    Progress,
    Retry,
    Done,
}

impl Screen {
    /// data-model §3's table, verbatim.
    ///
    /// The one refinement over `stage` alone: a busy machine reporting a
    /// PROGRESS status has left the key list and is deriving, so the progress
    /// screen takes over until it lands. `setting_up_identity` is deliberately
    /// not a progress status — it happens before the key list exists, and
    /// belongs to the Name screen's status line.
    pub fn of(view: &CreateView) -> Self {
        match view.stage {
            CreateStage::Created => Self::Done,
            CreateStage::SyncFailed => Self::Retry,
            CreateStage::AddKeys if view.busy && progress_for(view.status).is_some() => {
                Self::Progress
            }
            CreateStage::AddKeys => Self::Keys,
            CreateStage::Form if view.busy && progress_for(view.status).is_some() => Self::Progress,
            CreateStage::Form => Self::Name,
        }
    }
}

/// The progress screen's active row and percentage (data-model §3, research D9).
///
/// Derived from the stage the core reported, never from elapsed time: a bar
/// that advances on a timer tells the person something the wallet does not
/// know, and the moment they are most owed the truth is while their key set is
/// being frozen.
pub fn progress_for(status: Option<StatusKey>) -> Option<(usize, u32)> {
    match status? {
        StatusKey::VerifyingIdentity | StatusKey::ExtractingKey => Some((0, 33)),
        StatusKey::ComputingAddress => Some((1, 62)),
        StatusKey::SyncingKey => Some((2, 100)),
        _ => None,
    }
}

/// The three task rows, in order.
const PROGRESS_TASKS: [&str; 3] = [
    "onboarding.create.taskVerifyKey",
    "onboarding.create.taskDeriveAddress",
    "onboarding.create.taskWriteIndex",
];

/// The transient status line's corpus key. Exhaustive: a new `StatusKey` in the
/// core stops this compiling rather than rendering a blank line.
pub fn status_key(status: StatusKey) -> &'static str {
    match status {
        StatusKey::SettingUpIdentity => "onboarding.create.statusSettingUpIdentity",
        StatusKey::VerifyingIdentity => "onboarding.create.statusVerifyingIdentity",
        StatusKey::ExtractingKey => "onboarding.create.statusExtractingKey",
        StatusKey::ComputingAddress => "onboarding.create.statusComputingAddress",
        StatusKey::SyncingKey => "onboarding.create.statusSyncingKey",
        StatusKey::SetupCancelled => "onboarding.create.statusSetupCancelled",
        StatusKey::VerifyCancelled => "onboarding.create.statusVerifyCancelled",
    }
}

/// A key row's provider line.
///
/// Keyed off what the AUTHENTICATOR REPORTED, not off the method the person
/// chose: `method` is the choice, `kind` is the report, and on desktop the two
/// legitimately disagree. The FIRST key is minted before the key screen exists,
/// so it carries `KeyMethod::default()` (platform) while being, unavoidably, a
/// USB security key. Labelling that row from the choice would be the shell
/// repeating a default back to the person as though it were a fact.
///
/// The classification itself moved into the core as `CreateKeyRow::kind`
/// (issue #207). The heuristic this used to run here called EVERY
/// cross-platform key a security key, which mislabelled a phone reached by
/// scanning a code — the same mistake the web client's icon made.
///
/// The three lines name the same three places the method picker names, in the
/// picker's own words, so the row reads as an answer to the question that was
/// asked. "This device" is true here in a way it is not in settings: this key
/// was minted seconds ago on the machine in front of the person.
///
/// This is the FALLBACK line. When the core's AAGUID catalog knows the model,
/// the row shows the vault's own name and mark instead ("Apple Passwords",
/// "1Password") — the richer line the design drew. The catalog covers software
/// passkey providers, not the hundreds of hardware models in the FIDO metadata
/// service, so this stays the answer for a USB key.
fn provider_line(key: &CreateKeyRow) -> &'static str {
    match key.kind {
        KeyMethod::Platform => "onboarding.create.methodPlatformTitle",
        KeyMethod::Hybrid => "onboarding.create.methodHybridTitle",
        KeyMethod::SecurityKey => "onboarding.create.providerSecurityKey",
        // Spec 075: a key the Clear Signer minted lives behind its page, and
        // the row says so in the picker's own words.
        KeyMethod::ClearSigner => "componentsUi.signing.clearSignerTitle",
    }
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

/// What a control on these screens can ask for. The host translates each into a
/// core event (or, for the three presentation-only ones, into its own state).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlowEvent {
    NameChanged(String),
    AckToggled(usize),
    Submit,
    StartOver,
    AddKey(KeyMethod),
    ConfirmKey(usize),
    RemoveKey(usize),
    FinishKeys,
    RetryUpload,
    EnterWallet,
    /// The top-left affordance. The CORE owns whether there is a step to go
    /// back to (`can_go_back`); leaving the flow entirely is the host's,
    /// because the core has no idea what contains it.
    Back,
    /// Presentation only: expand or collapse the add-method list.
    TogglePicker,
    /// Presentation only: the address was copied.
    CopyAddress,
    /// Presentation only: a method the desktop cannot run was pressed.
    MethodUnavailable(KeyMethod),
}

pub type FlowSink = Rc<dyn Fn(FlowEvent, &mut Window, &mut App)>;

/// Everything the screens read. All of it is either `CreateView` or the host's
/// own presentation state — there is no third source.
pub struct FlowHost<'a> {
    pub theme: &'a Theme,
    pub loc: &'a Loc,
    pub view: &'a CreateView,
    pub name_focus: &'a FocusHandle,
    /// The DONE card's avatar. A `RefCell` because rasterizing needs `&mut`
    /// and the whole flow renders from a shared `&FlowHost`.
    pub identicons: &'a RefCell<IdenticonCache>,
    /// The method rows' marks (spec 038).
    pub passkey_icons: &'a RefCell<PasskeyIconCache>,
    /// Names and marks for models the compiled catalog cannot name. Read-only
    /// here: the page does the asking, from its own render pass.
    pub directory: &'a RefCell<PasskeyDirectory>,
    /// The add-method list is expanded.
    pub picker_open: bool,
    /// The Done screen's transient 已复制 feedback.
    pub copied: bool,
    pub sink: FlowSink,
}

fn emit(sink: &FlowSink, event: FlowEvent) -> impl Fn(&mut Window, &mut App) + 'static {
    let sink = sink.clone();
    move |window, cx| sink(event.clone(), window, cx)
}

// ---------------------------------------------------------------------------
// The shell
// ---------------------------------------------------------------------------

/// The whole flow: a back affordance and one screen.
///
/// The three-segment bar and the flow's name that used to head every step are
/// gone (founder call, 2026-08-25): a meter over a journey whose every screen
/// already says what it is measured decoration rather than progress, and the
/// label repeated the heading directly under it.
pub fn render_create_flow(host: &FlowHost<'_>, window: &Window) -> Div {
    let screen = Screen::of(host.view);
    let theme = host.theme;

    // No way back out of a running ceremony: the keys are being frozen, and the
    // only honest control is none.
    let can_leave = screen != Screen::Progress && screen != Screen::Done;
    let back = if can_leave {
        let on_back = emit(&host.sink, FlowEvent::Back);
        div()
            .id("flow-back")
            .flex()
            .items_center()
            .gap(px(theme::FLOW_BACK_GAP))
            .cursor_pointer()
            .text_size(theme::text_body())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(theme.fg_muted)
            .hover(|s| s.text_color(theme.fg_base))
            .on_click(move |_, window, cx| on_back(window, cx))
            // The chevron is set larger than its label and on a line height of
            // 1, so it reads as a mark rather than as a character of the word.
            .child(
                div()
                    .text_size(theme::text_flow_sub())
                    .line_height(theme::text_flow_sub())
                    .child("‹"),
            )
            .child(host.loc.t("onboarding.common.back"))
    } else {
        // Unreachable: the header this belongs to is not drawn at all when
        // there is no way back. Kept as an expression rather than an Option so
        // the row below stays one shape.
        div().id("flow-back-absent")
    };

    let body = match screen {
        Screen::Name => render_name(host, window),
        Screen::Keys => render_keys(host),
        Screen::Progress => render_progress(host),
        Screen::Retry => render_retry(host),
        Screen::Done => render_done(host),
    };

    // The header is a way back and NOTHING else, set off from the screen below
    // it by 28. The screen itself is `flex: 1`, so its own bottom spacer can
    // push a CTA onto the page's bottom edge.
    //
    // **It is drawn only where you can still leave.** On PROGRESS and DONE
    // there is nowhere to go back to — a running ceremony cannot be abandoned
    // and a finished one has its own way on — so the row is not drawn at all.
    //
    // **The step bar and the flow label are gone** (founder call, 2026-08-25,
    // and the same conclusion these two screens had already reached from the
    // other side): a three-segment meter over a journey whose every screen
    // says what it is was decoration, and "Create Wallet" restated the title
    // directly underneath it. The web, iOS and Android shells now match.
    let header = can_leave.then(|| {
        div()
            .w_full()
            .flex()
            .items_center()
            .flex_none()
            .pb(px(theme::FLOW_HEADER_PAD_B))
            .child(back)
    });

    // NOT `h_full`, and the body does not stretch. A screen ends where its
    // content ends; the window's leftover height stays leftover. Pinning the
    // CTA to the bottom edge is a phone's answer — the screen is the height of
    // a hand there — and on a desktop window it opened a hole in the middle
    // that grew with every pixel of height. That hole was the whole reason
    // this page read as a phone page pulled tall.
    div()
        .w_full()
        .max_w(px(FLOW_COLUMN_W))
        .flex()
        .flex_col()
        .children(header)
        .child(body)
}

/// The create journey's length, as the rail counts it.
pub const FLOW_STEPS: usize = 3;

/// Which step the rail names, and the keys for what it says about it.
///
/// `None` once the wallet exists: DONE hands the rail back to the brand,
/// because at that point nobody is asking where they are.
pub fn rail_step(view: &CreateView) -> Option<(usize, &'static str, &'static str)> {
    match Screen::of(view) {
        Screen::Name => Some((
            1,
            "onboarding.create.stepNamingLabel",
            "onboarding.create.stepNamingDetail",
        )),
        Screen::Keys => Some((
            2,
            "onboarding.create.stepKeysLabel",
            "onboarding.create.stepKeysDetail",
        )),
        // Retry is not a fourth step — it is the third one having failed to
        // land, and the rail keeps saying so.
        Screen::Progress | Screen::Retry => Some((
            3,
            "onboarding.create.stepCreateLabel",
            "onboarding.create.stepCreateDetail",
        )),
        Screen::Done => None,
    }
}

fn title(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_tagline())
        .line_height(theme::line_height_title())
        .font_weight(FontWeight::BOLD)
        .text_color(theme.fg_base)
        .child(text)
}

fn subtitle(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_flow_sub())
        .line_height(theme::line_height_flow_sub())
        .text_color(theme.fg_muted)
        .child(text)
}

/// The tiny uppercase label that heads a field or a list section.
fn section_label(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_section_label())
        .font_weight(FontWeight::SEMIBOLD)
        .text_color(theme.fg_muted)
        .child(SharedString::from(text.to_uppercase()))
}

/// A mono counter — key count, percentage — set beside a section label.
fn mono_meta(theme: &Theme, text: SharedString) -> Div {
    div()
        .font_family(theme::font_mono())
        .text_size(theme::text_row_meta())
        .text_color(theme.fg_muted)
        .child(text)
}

fn caption(theme: &Theme, text: SharedString) -> Div {
    div()
        .text_size(theme::text_flow_caption())
        .text_color(theme.fg_subtle)
        .child(text)
}

/// A flow CTA's three states, out of the core's two flags.
///
/// `busy` wins: the machine is working on what this button started, so the
/// button says so rather than dimming as if the action had become unavailable.
/// Some of these waits swap the whole screen for the progress list, and some
/// (a passkey prompt that has not opened yet) do not — this covers the second
/// kind, which used to be a dimmed button and nothing else.
fn submit_state(can_submit: bool, busy: bool) -> ButtonState {
    if busy {
        ButtonState::Busy
    } else {
        ButtonState::from_enabled(can_submit)
    }
}

// ---------------------------------------------------------------------------
// Name
// ---------------------------------------------------------------------------

/// Which link inside the legal acknowledgement was pressed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LegalLink {
    Privacy,
    Terms,
}

impl LegalLink {
    pub fn url(self) -> &'static str {
        match self {
            Self::Privacy => PRIVACY_URL,
            Self::Terms => TERMS_URL,
        }
    }
}

fn render_name(host: &FlowHost<'_>, window: &Window) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let view = host.view;

    // No label and no helper (spec 019). The heading directly above the field
    // already says "name your wallet", so a label restated it — and what the
    // helper said, that the name is stored on-chain, is now `ack0`, where a
    // person has to look at it rather than past it. Both render as nothing when
    // empty rather than as an empty line box.
    let strings = NameFieldStrings {
        label: SharedString::default(),
        placeholder: loc.t("onboarding.create.accountNamePlaceholder"),
        helper: SharedString::default(),
        too_long_hint: loc.t("onboarding.create.nameTooLong"),
    };
    let sink = host.sink.clone();
    let editable = view.name_editable;
    let field = name_field(
        theme,
        &strings,
        &view.name,
        view.name_too_long,
        host.name_focus,
        window,
        move |value, window, cx| {
            if editable {
                sink(FlowEvent::NameChanged(value), window, cx);
            }
        },
    );

    // The legal row's two inline links, located by byte range in the assembled
    // sentence. Built by concatenation rather than by interpolation because the
    // ranges have to be exact, and `{{var}}` fills would move them per locale.
    let ack2_lead = loc.t("onboarding.create.ack2");
    let ack2_privacy = loc.t("onboarding.create.ack2PrivacyPolicy");
    let ack2_and = loc.t("onboarding.create.ack2And");
    let ack2_terms = loc.t("onboarding.create.ack2Terms");
    let ack2_period = loc.t("onboarding.create.ack2Period");
    let privacy_at = ack2_lead.len();
    let and_at = privacy_at + ack2_privacy.len();
    let terms_at = and_at + ack2_and.len();
    let legal = SharedString::from(format!(
        "{ack2_lead}{ack2_privacy}{ack2_and}{ack2_terms}{ack2_period}"
    ));

    let sink_ack0 = host.sink.clone();
    let sink_ack1 = host.sink.clone();
    let sink_ack2 = host.sink.clone();
    let acks = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(theme::FLOW_GAP_MD_SNUG))
        .child(ack_row::<LegalLink>(
            0,
            theme,
            view.acks.first().copied().unwrap_or(false),
            loc.t("onboarding.create.ack0"),
            Vec::new(),
            move |window, cx| sink_ack0(FlowEvent::AckToggled(0), window, cx),
            |_, _, _| {},
        ))
        // Three gates, each a FACT about where something ends up (`ACK_COUNT`).
        // The recovery assurance that used to sit here described a BENEFIT, and
        // mixing one of those into a list of consequences teaches people to skim
        // the list — so it is gone, and row 1 now names where the PRIVATE key
        // stays, which the old pair never said out loud.
        .child(ack_row::<LegalLink>(
            1,
            theme,
            view.acks.get(1).copied().unwrap_or(false),
            loc.t("onboarding.create.ack1"),
            Vec::new(),
            move |window, cx| sink_ack1(FlowEvent::AckToggled(1), window, cx),
            |_, _, _| {},
        ))
        .child(ack_row(
            2,
            theme,
            view.acks.get(2).copied().unwrap_or(false),
            legal,
            vec![
                (privacy_at..and_at, LegalLink::Privacy),
                (terms_at..terms_at + ack2_terms.len(), LegalLink::Terms),
            ],
            move |window, cx| sink_ack2(FlowEvent::AckToggled(2), window, cx),
            // The links open a browser and are NOT the checkbox: pressing one
            // must not tick the box the sentence belongs to.
            move |link: LegalLink, _window, cx| cx.open_url(link.url()),
        ));

    // The gates sit against the button they gate: the spacer above them absorbs
    // the free height, so they are never stranded mid-screen. A checklist a
    // cursor reaches before the sentence does is one nobody reads.
    //
    // They and the button are ONE block, bound by 16 rather than the column's
    // 24 — that is what v2 draws, and it is what makes them read as the button's
    // preconditions rather than as the last paragraph of the page.
    let mut bottom = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_MD))
        .child(acks);

    // A cancelled ceremony is a quiet status line with the draft intact, never
    // a sheet: there is a form to come back to, and the design reserves the
    // modal for the sign-in path where there is not.
    if let Some(status) = view.status
        && progress_for(Some(status)).is_none()
    {
        bottom = bottom.child(caption(theme, loc.t(status_key(status))));
    }

    let submit_key = match view.submit_label {
        SubmitLabel::Create => "onboarding.create.nextBtn",
        SubmitLabel::FinishVerify => "onboarding.create.finishVerifyBtn",
    };
    let sink_submit = host.sink.clone();
    bottom = bottom.child(vela_button_state(
        "flow-submit",
        ButtonVariant::Primary,
        loc.t(submit_key),
        submit_state(view.can_submit, view.busy),
        theme,
        move |_, window, cx| sink_submit(FlowEvent::Submit, window, cx),
    ));

    if view.show_start_over {
        let on_start_over = emit(&host.sink, FlowEvent::StartOver);
        bottom = bottom.child(
            div()
                .id("flow-start-over")
                .w_full()
                .flex()
                .justify_center()
                .cursor_pointer()
                .text_size(theme::text_body())
                .text_color(theme.fg_muted)
                .hover(|s| s.text_color(theme.fg_base))
                .on_click(move |_, window, cx| on_start_over(window, cx))
                .child(loc.t("onboarding.create.startOverBtn")),
        );
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_LG))
        .child(title(theme, loc.t("onboarding.create.nameTitle")))
        .child(field)
        .child(bottom)
}

// ---------------------------------------------------------------------------
// Keys
// ---------------------------------------------------------------------------

fn render_keys(host: &FlowHost<'_>) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let view = host.view;
    let full = view.keys.len() >= MAX_KEYS;

    let headline = if view.needs_second_key {
        loc.t("onboarding.create.keysTitleBlocked")
    } else {
        loc.t("onboarding.create.keysTitle")
    };
    let sub = if view.needs_second_key {
        loc.t("onboarding.create.keysSubtitleBlocked")
    } else if full {
        loc.t("onboarding.create.keysSubtitleFull")
    } else {
        loc.t("onboarding.create.keysSubtitle")
    };

    let mut column = div().w_full().flex().flex_col().gap(px(FLOW_GAP_MD)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(FLOW_GAP_SM))
            .child(title(theme, headline))
            .child(subtitle(theme, sub)),
    );

    if view.needs_second_key {
        column = column.child(
            div()
                .w_full()
                .flex()
                .items_start()
                .gap(px(theme::FLOW_GAP_MD_SNUG))
                .px(px(FLOW_GAP_MD))
                .py(px(14.))
                .rounded(px(RADIUS_FIELD))
                .bg(theme.warning_soft)
                .child(
                    div()
                        .size(px(7.))
                        .flex_none()
                        .mt(px(6.))
                        .rounded_full()
                        .bg(theme.accent),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_body())
                        .line_height(theme::line_height_flow_sub())
                        .text_color(theme.fg_base)
                        .child(loc.t("onboarding.create.needSecondKeyHint")),
                ),
        );
    }

    #[allow(clippy::cast_precision_loss, clippy::allow_attributes)]
    let counter = loc.t_vars(
        "onboarding.create.keyCount",
        &[
            ("current", view.keys.len() as f64),
            ("max", MAX_KEYS as f64),
        ],
    );
    let mut rows = div().w_full().flex().flex_col().gap(px(theme::KEY_ROW_GAP));
    for (index, key) in view.keys.iter().enumerate() {
        rows = rows.child(key_row(host, index, key));
    }
    column = column.child(
        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .w_full()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(section_label(theme, loc.t("onboarding.create.keysLabel")))
                    .child(mono_meta(theme, counter)),
            )
            .child(rows),
    );

    // Add-key control, with the cap stated on it rather than behind it.
    let on_toggle = emit(&host.sink, FlowEvent::TogglePicker);
    let add_label = if full {
        loc.t("onboarding.create.keyLimitReached")
    } else {
        loc.t("onboarding.create.addKeyBtn")
    };
    // v2 draws this as an outlined button in its own right — same rectangle as
    // the CTA below it, one notch shorter — not as a bare row in the list.
    let mut add = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_SM))
        .child({
            let row = div()
                .id("flow-add-key")
                .w_full()
                .h(px(theme::BTN_H_SECONDARY))
                .flex_none()
                .flex()
                .items_center()
                .justify_center()
                .gap(px(FLOW_GAP_SM))
                .rounded(px(theme::RADIUS_CTA))
                .border_1()
                .border_color(theme.divider)
                .text_size(theme::text_row_name())
                .font_weight(FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(div().text_size(theme::text_card_title()).child("+"))
                .child(div().child(add_label));
            if view.can_add_key {
                let accent = theme.accent;
                row.cursor_pointer()
                    .hover(move |s| s.border_color(accent))
                    .on_click(move |_, window, cx| on_toggle(window, cx))
            } else {
                row.opacity(OPACITY_DISABLED)
            }
        });
    // An EMPTY list keeps the methods expanded: the first key's method is the
    // person's choice too, and an empty list with a collapsed "+" is a
    // puzzle, not a step.
    if (host.picker_open || view.keys.is_empty()) && view.can_add_key {
        add = add.child(method_picker(host));
    }
    column = column.child(add);

    column = column.child(caption(theme, loc.t("onboarding.create.keysHint")));
    let finish_label = if view.needs_second_key {
        loc.t("onboarding.create.addSecondKeyBtn")
    } else {
        loc.t("onboarding.create.createWalletBtn")
    };
    let sink_finish = host.sink.clone();
    column.child(vela_button_state(
        "flow-finish",
        ButtonVariant::Primary,
        finish_label,
        submit_state(view.can_finish, view.busy),
        theme,
        move |_, window, cx| sink_finish(FlowEvent::FinishKeys, window, cx),
    ))
}

fn key_row(host: &FlowHost<'_>, index: usize, key: &CreateKeyRow) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let busy = host.view.busy;

    // ONE trailing slot, as the design draws it. A key that has not confirmed
    // its membership has no status to show yet, so the retry TAKES that slot
    // rather than crowding in beside it.
    let trailing: AnyElement = if key.confirmed {
        // Bare coloured text, not a filled pill: v2 puts the row itself in a
        // bordered card, and a second fill inside it is one surface too many.
        // Cloud-synced or device-bound: a KIND of passkey, not a fault. The
        // device-bound badge is neutral (`fg_muted`, as on the web): a security
        // key or Windows Hello cannot be synced, so a warning tone — which this
        // once was, reading `未同步` / "Not synced" — told people to go and do
        // something there is nothing to do about. The real risk, every key being
        // device-bound, is the "add a second key" hint's job. Where the key
        // lives is the provider line's, above.
        //
        // Read straight off `synced`, not through `synced_known`: the web list
        // must draw nothing when nobody could read the attestation, because it
        // also shows keys fetched from the registry. This list only ever holds
        // keys minted in this very flow, and a registration that yields a
        // public key always yields a readable 20-byte summary — so the unknown
        // case cannot arrive here.
        let (label, fg) = if key.synced {
            (
                loc.t("onboarding.create.keySyncedBadge"),
                theme.success_base,
            )
        } else {
            (
                loc.t("onboarding.create.keyDeviceOnlyBadge"),
                theme.fg_muted,
            )
        };
        div()
            .flex_none()
            .text_size(theme::text_section_label())
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(fg)
            .child(label)
            .into_any_element()
    } else {
        let on_confirm = emit(&host.sink, FlowEvent::ConfirmKey(index));
        let row = div()
            .id(("flow-confirm-key", index as u64))
            .flex_none()
            .px(px(FLOW_GAP_SM))
            .py(px(2.))
            .rounded(px(theme::RADIUS_ACK))
            .bg(theme.warning_soft)
            .text_size(theme::text_flow_caption())
            .text_color(theme.warning_base)
            .child(loc.t("onboarding.create.confirmKeyBtn"));
        if busy {
            row.opacity(OPACITY_DISABLED).into_any_element()
        } else {
            row.cursor_pointer()
                .on_click(move |_, window, cx| on_confirm(window, cx))
                .into_any_element()
        }
    };

    // A bordered card on the page colour, not a hairline-separated row: v2
    // gives every key its own edge and 8px of air, so a list of three reads as
    // three things rather than as a table.
    let mut row = div()
        .w_full()
        .flex()
        .items_center()
        .gap(px(theme::FLOW_GAP_MD_SNUG))
        .px(px(theme::KEY_ROW_PAD_X))
        .py(px(theme::KEY_ROW_PAD_Y))
        .rounded(px(RADIUS_FIELD))
        .bg(theme.bg_base)
        .border_1()
        .border_color(theme.divider);

    // The vault's own mark, when the catalog knows the model. Nothing when it
    // does not — the line below already says what is known, and an invented
    // placeholder logo would say something that is not.
    // Who is holding this key: the compiled catalog's name, then the
    // directory's for a model no catalog carries, then the method line.
    let holder: Option<SharedString> = if key.provider_name.is_empty() {
        host.directory
            .borrow()
            .holder(&key.aaguid, theme.is_dark())
            .map(|found| SharedString::from(found.name.clone()))
    } else {
        Some(SharedString::from(key.provider_name.clone()))
    };

    let listed_mark = host
        .directory
        .borrow()
        .holder(&key.aaguid, theme.is_dark())
        .and_then(|holder| holder.icon_url.as_deref().map(str::to_owned))
        .and_then(|url| {
            host.directory
                .borrow()
                .mark(&url, theme::KEY_ROW_MARK as u32)
        });

    if let Some(mark) = passkey_mark(
        &mut host.identicons.borrow_mut(),
        &key.aaguid,
        host.theme.is_dark(),
        theme::KEY_ROW_MARK,
    ) {
        row = row.child(mark);
    } else if let Some(image) = listed_mark {
        // The directory's own artwork for a model no build could carry.
        row = row.child(
            img(ImageSource::Render(image))
                .w(px(theme::KEY_ROW_MARK))
                .h(px(theme::KEY_ROW_MARK))
                .flex_none(),
        );
    } else if let Some(mark) = passkey_fallback_mark(
        &mut host.identicons.borrow_mut(),
        &key.authenticator_attachment,
        &key.transports,
        key.method == KeyMethod::SecurityKey,
        theme,
        theme::KEY_ROW_MARK,
    ) {
        // Not a brand, but not nothing either: the authenticator said it is a
        // security key, and a USB transport says which kind.
        row = row.child(mark);
    }

    let mut row = row
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_name())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(SharedString::from(key.name.clone())),
                )
                .child(
                    div()
                        .text_size(theme::text_row_meta())
                        .text_color(theme.fg_muted)
                        .child(holder.unwrap_or_else(|| loc.t(provider_line(key)))),
                ),
        )
        .child(trailing);

    // Row 0 is the pinned key: not removable, and its name IS the wallet name.
    if index > 0 {
        let on_remove = emit(&host.sink, FlowEvent::RemoveKey(index));
        let remove = div()
            .id(("flow-remove-key", index as u64))
            .flex_none()
            .size(px(theme::FLOW_CLOSE_HIT))
            .rounded_full()
            .flex()
            .items_center()
            .justify_center()
            .text_size(theme::text_card_title())
            .text_color(theme.fg_muted)
            .child("×");
        row = row.child(if busy {
            remove.opacity(OPACITY_DISABLED)
        } else {
            remove
                .cursor_pointer()
                .hover(|s| s.bg(theme.bg_sunken))
                .on_click(move |_, window, cx| on_remove(window, cx))
        });
    }

    row
}

/// The four ways to mint a founding key (spec 075 added the Clear Signer).
///
/// **One of them may not run here, and it says so.** `Platform` needs a system
/// passkey service, which only Windows provides in this app's reach. Hiding it
/// would leave a person wondering whether their laptop's fingerprint reader was
/// supposed to work; showing it greyed with a reason answers that in one line.
fn method_picker(host: &FlowHost<'_>) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    // See `hardware::signin_method_card`: only Windows has a platform
    // authenticator this shell can reach, and only when Hello is enrolled.
    let this_device = crate::executor::passkey::platform_supported();

    let palette = Palette {
        ink: theme.fg_muted,
        muted: theme.fg_subtle,
        paper: theme.bg_base,
    };
    let entry = |method: KeyMethod, title_key: &str, body: SharedString, available: bool| {
        let sink = host.sink.clone();
        let event = if available {
            FlowEvent::AddKey(method)
        } else {
            FlowEvent::MethodUnavailable(method)
        };
        let mark = host.passkey_icons.borrow_mut().image(
            PasskeyIcon::for_method(method),
            palette,
            METHOD_ICON_PX,
        );
        let row = div()
            .id(("flow-method", method as u64))
            .w_full()
            .flex()
            .items_center()
            .gap(px(FLOW_GAP_MD))
            .py(px(FLOW_GAP_MD))
            .border_b_1()
            .border_color(theme.divider)
            .child(
                img(ImageSource::Render(mark))
                    .w(px(METHOD_ICON_PX as f32))
                    .h(px(METHOD_ICON_PX as f32))
                    .flex_none(),
            )
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_card_title())
                            .text_color(theme.fg_base)
                            .child(loc.t(title_key)),
                    )
                    .child(caption(theme, body)),
            );
        if available {
            row.cursor_pointer()
                .hover(|s| s.bg(theme.bg_sunken))
                .on_click(move |_, window, cx| sink(event.clone(), window, cx))
        } else {
            row.opacity(OPACITY_DISABLED)
        }
    };

    div()
        .w_full()
        .flex()
        .flex_col()
        .child(caption(theme, loc.t("onboarding.create.addMethodLabel")))
        .child(entry(
            KeyMethod::SecurityKey,
            "onboarding.create.methodSecurityKeyTitle",
            loc.t("onboarding.create.methodSecurityKeyBody"),
            true,
        ))
        .child(entry(
            KeyMethod::Platform,
            "onboarding.create.methodPlatformTitle",
            loc.t(if this_device {
                "onboarding.create.methodPlatformBody"
            } else {
                "onboarding.create.securityKeyRequiredBody"
            }),
            this_device,
        ))
        .child(entry(
            // The scan method is live on desktop now: it shows a QR, and a phone
            // that scans it becomes the authenticator over caBLE.
            KeyMethod::Hybrid,
            "onboarding.create.methodHybridTitle",
            loc.t("onboarding.create.methodHybridBody"),
            true,
        ))
        // Spec 075: the fourth route, and the only one that is OURS. Never
        // greyed: a signer page is reachable from every desktop, on this
        // machine's own browser over a loopback socket or on the phone in the
        // person's hand over the relay — the ceremony asks which.
        .child(entry(
            KeyMethod::ClearSigner,
            "componentsUi.signing.clearSignerTitle",
            loc.t("componentsUi.signing.clearSignerBody"),
            true,
        ))
}

// ---------------------------------------------------------------------------
// Progress
// ---------------------------------------------------------------------------

/// The wallet is being made.
///
/// **One progress indicator, not three.** The design's mock carried a
/// DERIVING ADDRESS meter above the task list, and drawn against the real
/// machine it was redundant twice over: the step bar at the top of the shell
/// already says where the journey is, and the task list below says it exactly,
/// by name. Worse, the meter's label named ONE phase while a different task
/// was running, and its percentage is not measured — it is the same three
/// statuses the list is already showing, divided by three. A number that looks
/// measured and is not is worse than no number.
fn render_progress(host: &FlowHost<'_>) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let (active, _percent) = progress_for(host.view.status).unwrap_or((0, 0));

    #[allow(clippy::cast_precision_loss, clippy::allow_attributes)]
    let subtitle_text = loc.t_vars(
        "onboarding.create.progressSubtitle",
        &[("count", host.view.keys.len() as f64)],
    );

    // Rule-separated rows, not a gapped list: the tasks are a ledger of what
    // has happened, and v2 draws them as one.
    let mut tasks = div().w_full().flex().flex_col().gap(px(2.));
    for (index, key) in PROGRESS_TASKS.iter().enumerate() {
        // The RUNNING row turns. Each of these three waits on something outside
        // the app — a passkey prompt, a derivation, a network write — and a
        // still dot says nothing about whether anything is still happening
        // (founder call, 2026-08-25).
        let mark: AnyElement = if index < active {
            div()
                .text_size(theme::text_row_meta())
                .text_color(theme.success_base)
                .child("✓")
                .into_any_element()
        } else if index == active {
            spinner(theme.accent, px(FLOW_GAP_MD), px(theme::SPINNER_STROKE))
        } else {
            div()
                .text_size(theme::text_row_meta())
                .text_color(theme.fg_subtle)
                .child("○")
                .into_any_element()
        };
        tasks = tasks.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(theme::FLOW_GAP_MD_SNUG))
                .py(px(theme::TASK_ROW_PAD_Y))
                .border_b_1()
                .border_color(theme.divider)
                .child(
                    div()
                        .w(px(FLOW_GAP_MD))
                        .h(px(FLOW_GAP_MD))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(mark),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_row_name())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(if index <= active {
                            theme.fg_base
                        } else {
                            theme.fg_subtle
                        })
                        .child(loc.t(key)),
                ),
        );
    }

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(theme::FLOW_GAP_XL))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(title(theme, loc.t("onboarding.create.progressTitle")))
                .child(subtitle(theme, subtitle_text)),
        )
        .child(tasks)
}

// ---------------------------------------------------------------------------
// Retry
// ---------------------------------------------------------------------------

/// The keys were minted; the group never landed.
///
/// Nothing is lost and nothing is re-minted: the core keeps the whole founding
/// set and a pending record it wrote BEFORE the first publish attempt, so retry
/// resumes at the publish. That is why the primary here is a retry, and why
/// starting over is the quiet secondary rather than the obvious escape.
fn render_retry(host: &FlowHost<'_>) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let view = host.view;

    let mut column = div().w_full().flex().flex_col().gap(px(FLOW_GAP_LG)).child(
        div()
            .flex()
            .flex_col()
            .gap(px(FLOW_GAP_SM))
            .child(title(theme, loc.t("onboarding.create.syncFailedTitle")))
            .child(subtitle(
                theme,
                loc.t("onboarding.create.syncFailedMessage"),
            ))
            .child(caption(theme, loc.t("onboarding.create.syncFailedHint"))),
    );

    if let Some(detail) = &view.sync_error_detail {
        column = column.child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(div().h(px(HAIRLINE)).w_full().bg(theme.divider))
                .child(caption(theme, loc.t("onboarding.create.technicalDetails")))
                .child(
                    div()
                        .w_full()
                        .p(px(FLOW_GAP_MD))
                        .rounded(px(RADIUS_FIELD))
                        .bg(theme.bg_well)
                        .font_family(theme::font_mono())
                        .text_size(theme::text_flow_caption())
                        .text_color(theme.error_base)
                        .child(SharedString::from(detail.clone())),
                ),
        );
    }

    let sink_retry = host.sink.clone();
    let sink_over = host.sink.clone();
    column
        // The retry carries the wait; the way out beside it only closes.
        .child(vela_button_state(
            "flow-retry-upload",
            ButtonVariant::Primary,
            loc.t("onboarding.create.retryUploadBtn"),
            submit_state(true, view.busy),
            theme,
            move |_, window, cx| sink_retry(FlowEvent::RetryUpload, window, cx),
        ))
        .child(vela_button_opts(
            "flow-start-over",
            ButtonVariant::Row,
            loc.t("onboarding.create.startOverBtn"),
            !view.busy,
            theme,
            move |_, window, cx| sink_over(FlowEvent::StartOver, window, cx),
        ))
}

// ---------------------------------------------------------------------------
// Done
// ---------------------------------------------------------------------------

/// The address as v2 draws it: bare mono text under a rule, wrapping on any
/// character, with no well and no button.
///
/// It is still copyable — the design dropped the affordance, but dropping the
/// FUNCTION would mean a person who has just been handed the one identifier
/// their money lives at has no way to take it with them. The whole line is the
/// target, and the confirmation replaces it in place.
fn done_address(
    theme: &Theme,
    address: String,
    copied: bool,
    loc: &Loc,
    on_copy: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let (text, color) = if copied {
        (loc.t("onboarding.common.copied"), theme.success_base)
    } else {
        (SharedString::from(address), theme.fg_muted)
    };
    div()
        .id("flow-done-address")
        .w_full()
        .border_t_1()
        .border_color(theme.divider)
        .pt(px(theme::FLOW_GAP_MD_SNUG))
        .cursor_pointer()
        .font_family(theme::font_mono())
        .text_size(theme::text_body())
        .font_weight(FontWeight::MEDIUM)
        .line_height(theme::line_height_flow_sub())
        .text_color(color)
        .hover(|s| s.text_color(theme.fg_base))
        .on_click(move |_, window, cx| on_copy(window, cx))
        .child(text)
}

/// The wallet exists.
///
/// This is the first moment an address is shown, and that ordering is a RULE
/// rather than a layout choice: the core withholds `address` until the group
/// has landed and the account is saved, because an address shown earlier is an
/// address someone can fund before the wallet is reachable.
fn render_done(host: &FlowHost<'_>) -> Div {
    let theme = host.theme;
    let loc = host.loc;
    let view = host.view;
    let address = view.address.clone().unwrap_or_default();
    let wallet_name = view
        .keys
        .first()
        .map_or_else(|| view.name.clone(), |key| key.name.clone());

    let mut keys = div().w_full().flex().flex_col();
    for key in &view.keys {
        // The DONE list is a recap, not the editable list from KEYS: v2 sets
        // it in the muted body size with a bare badge and no fill.
        let (label, fg) = if key.synced {
            (
                loc.t("onboarding.create.keySyncedBadge"),
                theme.success_base,
            )
        } else {
            (
                loc.t("onboarding.create.keyDeviceOnlyBadge"),
                theme.fg_muted,
            )
        };
        keys = keys.child(
            div()
                .w_full()
                .flex()
                .items_center()
                .gap(px(10.))
                .py(px(theme::DONE_ROW_PAD_Y))
                .border_b_1()
                .border_color(theme.divider)
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_body())
                        .text_color(theme.fg_muted)
                        .child(SharedString::from(key.name.clone())),
                )
                .child(
                    div()
                        .flex_none()
                        .text_size(theme::text_section_label())
                        .font_weight(FontWeight::SEMIBOLD)
                        .text_color(fg)
                        .child(label),
                ),
        );
    }

    #[allow(clippy::cast_precision_loss, clippy::allow_attributes)]
    let success_body = loc.t_vars(
        "onboarding.create.successMessage",
        &[("count", view.keys.len() as f64)],
    );
    let on_copy = emit(&host.sink, FlowEvent::CopyAddress);
    let sink_enter = host.sink.clone();

    div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_LG))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(theme::FLOW_GAP_MD_SNUG))
                        .child(
                            div()
                                .size(px(theme::DONE_CHECK))
                                .flex_none()
                                .rounded_full()
                                .bg(theme.success_soft)
                                .flex()
                                .items_center()
                                .justify_center()
                                .text_size(theme::text_flow_sub())
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.success_base)
                                .child("✓"),
                        )
                        .child(title(theme, loc.t("onboarding.create.successTitle"))),
                )
                .child(subtitle(theme, success_body)),
        )
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(14.))
                .p(px(FLOW_GAP_MD))
                .rounded(px(RADIUS_FIELD))
                .bg(theme.bg_base)
                .border_1()
                .border_color(theme.divider)
                // Avatar beside the name, then the address under a rule. The
                // caption that used to sit here DESCRIBED the identicon ("an
                // identity pattern generated from the address") because there
                // was no identicon to look at; now there is, and a caption
                // narrating a picture next to the picture is noise. The
                // "wallet address" label goes for the same reason — a 42-
                // character 0x string in mono under a wallet's name is not
                // mistakable for anything else.
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(14.))
                        .child(identicon_avatar(
                            &mut host.identicons.borrow_mut(),
                            &address,
                            theme::DONE_AVATAR,
                        ))
                        .child(
                            div()
                                .text_size(theme::text_flow_sub())
                                .font_weight(FontWeight::BOLD)
                                .text_color(theme.fg_base)
                                .child(SharedString::from(wallet_name)),
                        ),
                )
                .child(done_address(theme, address, host.copied, loc, on_copy)),
        )
        .child(keys)
        .child(caption(theme, loc.t("onboarding.create.verifyHint")))
        .child(vela_button_opts(
            "flow-enter-wallet",
            ButtonVariant::Primary,
            loc.t("onboarding.create.enterWalletBtn"),
            true,
            theme,
            move |_, window, cx| sink_enter(FlowEvent::EnterWallet, window, cx),
        ))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn view(stage: CreateStage, busy: bool, status: Option<StatusKey>) -> CreateView {
        CreateView {
            stage,
            name: String::new(),
            name_editable: true,
            name_too_long: false,
            acks: vec![false, false],
            can_submit: false,
            submit_label: SubmitLabel::Create,
            busy,
            status,
            show_start_over: false,
            address: None,
            sync_error_detail: None,
            can_go_back: false,
            keys: Vec::new(),
            can_add_key: true,
            can_finish: false,
            needs_second_key: false,
        }
    }

    /// The first key is a USB key wearing the core's default method, and the
    /// row must say what it IS.
    ///
    /// `Submit` mints the founding key before the key screen exists, so the
    /// core sends `KeyMethod::default()` — platform. On this platform that is a
    /// placeholder, not a report: the thing on the desk is a security key. The
    /// core now settles that in `kind`; the shell only renders it.
    #[test]
    fn a_usb_key_reads_as_a_security_key_whatever_the_method_says() {
        let usb = CreateKeyRow {
            name: "Everyday wallet".to_owned(),
            authenticator_attachment: "cross-platform".to_owned(),
            transports: "usb".to_owned(),
            confirmed: true,
            synced: false,
            synced_known: true,
            aaguid: String::new(),
            provider_name: String::new(),
            method: KeyMethod::Platform,
            kind: KeyMethod::SecurityKey,
        };
        assert_eq!(
            provider_line(&usb),
            "onboarding.create.providerSecurityKey",
            "the report outranks the default"
        );

        // With nothing reported, the core leaves the choice standing.
        let unreported = CreateKeyRow {
            authenticator_attachment: String::new(),
            transports: String::new(),
            kind: KeyMethod::Platform,
            ..usb.clone()
        };
        assert_eq!(
            provider_line(&unreported),
            "onboarding.create.methodPlatformTitle"
        );

        // Issue #207: a phone reached by scanning a code is not a fob, and the
        // row must not call it one.
        let phone = CreateKeyRow {
            transports: "hybrid,internal".to_owned(),
            kind: KeyMethod::Hybrid,
            ..usb.clone()
        };
        assert_eq!(provider_line(&phone), "onboarding.create.methodHybridTitle");
    }

    /// data-model §3's screen-selection table, which is the whole of the create
    /// UI's logic. If this drifts, a screen appears at the wrong moment — and
    /// the moment that matters is the progress screen, which is the only one
    /// with no way back out.
    #[test]
    fn the_screen_table_matches_the_data_model() {
        assert_eq!(
            Screen::of(&view(CreateStage::Form, false, None)),
            Screen::Name
        );
        assert_eq!(
            Screen::of(&view(CreateStage::AddKeys, false, None)),
            Screen::Keys
        );
        assert_eq!(
            Screen::of(&view(CreateStage::SyncFailed, false, None)),
            Screen::Retry
        );
        assert_eq!(
            Screen::of(&view(CreateStage::Created, false, None)),
            Screen::Done
        );
        assert_eq!(
            Screen::of(&view(
                CreateStage::AddKeys,
                true,
                Some(StatusKey::ComputingAddress)
            )),
            Screen::Progress,
            "a busy machine deriving an address has left the key list"
        );
    }

    /// `setting_up_identity` happens BEFORE the key list exists, so it belongs
    /// to the Name screen's status line and must not take the page over.
    /// Neither may a cancellation, which is the one thing the v2 design is
    /// explicit about not making modal.
    #[test]
    fn the_pre_key_and_cancelled_statuses_stay_on_the_name_screen() {
        for status in [
            StatusKey::SettingUpIdentity,
            StatusKey::SetupCancelled,
            StatusKey::VerifyCancelled,
        ] {
            assert_eq!(progress_for(Some(status)), None, "{status:?}");
            assert_eq!(
                Screen::of(&view(CreateStage::Form, true, Some(status))),
                Screen::Name,
                "{status:?}"
            );
        }
    }

    /// The three progress rows advance only on what the core reported.
    #[test]
    fn progress_positions_come_from_the_reported_stage() {
        assert_eq!(
            progress_for(Some(StatusKey::VerifyingIdentity)),
            Some((0, 33))
        );
        assert_eq!(progress_for(Some(StatusKey::ExtractingKey)), Some((0, 33)));
        assert_eq!(
            progress_for(Some(StatusKey::ComputingAddress)),
            Some((1, 62))
        );
        assert_eq!(progress_for(Some(StatusKey::SyncingKey)), Some((2, 100)));
        assert_eq!(progress_for(None), None);
        assert_eq!(PROGRESS_TASKS.len(), 3);
    }
}
