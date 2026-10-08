//! Send feedback — ST15, the web's `FeedbackBody` (078 S-03), with
//! screenshots (078 round 3; founder: 「Desktop 没有 screenshot picker。有办法
//! 做到吗？能选择文件不就能选择截图吗？」).
//!
//! ## Three ways in, one tray
//!
//! - **Add screenshots** opens the system's multi-file picker. gpui's
//!   `PathPromptOptions` at this pin has no starting directory and no type
//!   filter, so it opens wherever the platform last left it and any file can
//!   be chosen — the decoder is the judge, and a file that is not an image
//!   says so. `VELA_SCREENSHOT_FILES` answers the picker (the
//!   `VELA_IMPORT_FILE` seam: a system dialog is a window no verification
//!   pass can drive).
//! - **Drop** image files anywhere on the form.
//! - **Paste** (⌘V / Ctrl+V) while this page is up: a screenshot copied with
//!   ⌘⌃⇧4, an image copied from a browser, or image FILES copied in Finder or
//!   Explorer. Text on the clipboard keeps pasting as text.
//!
//! Each image is prepared off the frame (`screenshot_prep`): decoded, turned
//! upright, scaled to 1920, re-encoded as a bare JPEG — which is what strips
//! its EXIF — and the tile shows those prepared bytes, the ones sent.
//!
//! ## What the page promises
//!
//! Screenshots are PUBLIC (the founder's ruling), so the line saying so is on
//! screen under the tiles before Send, and a refusal (a sixth image, a file
//! that is not one) is a line of its own ABOVE it — never in its place (v2
//! A1). Nothing about screenshots disables Send; a tile still being prepared
//! is waited for, busy, rather than left behind. While a report is going the
//! form holds still (v3 B9).

use std::path::PathBuf;
use std::sync::Arc;

use gpui::{
    ExternalPaths, ImageSource, InteractiveElement as _, StatefulInteractiveElement as _, img,
};

use super::*;
use crate::executor::bug_report::{self, BugReportOutcome};
use crate::executor::screenshot_prep::{self, Refusal, Tray};

/// The tile, square (v2 A6: `min(72, (row − 4 gaps) / 5)`; the settings
/// column is never narrower than five of them, so it is always 72 here).
const TILE: f32 = 72.;
/// The form's measure (the settings panel always has room for it).
const COLUMN_W: f32 = 560.;
const TILE_GAP: f32 = 12.;
/// The remove badge (v2 A5 / v3 B1), its page-coloured ring, and the tap area
/// that grows OUTWARD from it (v3 B10).
const BADGE: f32 = 22.;
const BADGE_RING: f32 = 2.;
const BADGE_OVERLAP: f32 = 4.;
const REMOVE_HIT: f32 = 44.;

/// A tile whose picture is ready: what is sent and what is drawn, both from
/// the prepared JPEG.
#[derive(Clone)]
pub(super) struct ReadyShot {
    base64: Arc<str>,
    thumb: Arc<gpui::RenderImage>,
    /// The whole picture, for the viewer (C3) — what will be sent.
    full: Arc<gpui::RenderImage>,
    width: u32,
    height: u32,
}

/// The outcome of a send the person walked away from, said where they are
/// now: a toast with the one action that follows from it.
#[derive(Clone)]
pub(super) struct OutcomeToast {
    filed: bool,
    url: String,
}

/// Where a new image comes from.
enum ShotSource {
    Path(PathBuf),
    Bytes(Vec<u8>),
}

/// What the report says and how its last send ended — the part every rule
/// about a stale outcome is written against. Generic over what a ready tile
/// holds, so those rules are tested without drawing a picture.
pub(super) struct Report<T> {
    what: String,
    steps: String,
    tray: Tray<T>,
    result: Option<BugReportOutcome>,
    /// A report the app wrote, not the person (issue 466): the core's area
    /// and dedup key, which it files under. `None`: the person's own report,
    /// filed as "Other" under a fingerprint of their words.
    origin: Option<bug_report::ReportOrigin>,
}

impl<T> Default for Report<T> {
    fn default() -> Self {
        Self {
            what: String::new(),
            steps: String::new(),
            tray: Tray::default(),
            result: None,
            origin: None,
        }
    }
}

impl<T> Report<T> {
    /// The report changed after the endpoint fell back. The form's URL was
    /// built from the words as they were, and its "screenshots can't be
    /// carried over" line from the tiles as they were — both stale now — so
    /// the outcome goes, and Send is the button again: the same rule a
    /// refusal line follows. A filed report is not an outcome an edit undoes.
    fn edited(&mut self) {
        if matches!(self.result, Some(BugReportOutcome::Fallback { .. })) {
            self.result = None;
        }
    }

    fn set_what(&mut self, text: String) {
        if text != self.what {
            // Every word of a seeded report gone: what is typed next is the
            // person's own complaint, and must not be filed — and deduped —
            // as the relay outage the seed was about.
            if text.trim().is_empty() {
                self.origin = None;
            }
            self.what = text;
            self.edited();
        }
    }

    /// Start over on the app's report (issue 466): its words, its area and
    /// key, no tiles, no outcome. The tray is emptied, not replaced, so a
    /// picture still being prepared can never land under a new tile's id.
    fn seed(&mut self, what: String, steps: String, origin: bug_report::ReportOrigin) {
        self.what = what;
        self.steps = steps;
        self.tray.clear();
        self.result = None;
        self.origin = Some(origin);
    }

    /// What Send files: the words, the device facts and the tiles — under
    /// "Other" and a fingerprint of the words, or, for a seeded report,
    /// under the area and dedup key the core chose.
    fn payload(
        &self,
        labels: &bug_report::EnvironmentLabels,
        facts: &bug_report::DeviceFacts,
        screenshots: Vec<String>,
    ) -> bug_report::BugReportPayload {
        let area = self
            .origin
            .as_ref()
            .map_or(bug_report::AREA_OTHER, |origin| origin.area.as_str());
        let payload =
            bug_report::build_bug_report(&self.what, &self.steps, area, labels, facts, screenshots);
        match &self.origin {
            Some(origin) => payload.filed_as(origin),
            None => payload,
        }
    }

    fn set_steps(&mut self, text: String) {
        if text != self.steps {
            self.steps = text;
            self.edited();
        }
    }

    /// New tiles (or refused files): a change either way.
    fn add(&mut self, images: usize, others: usize) -> Vec<u64> {
        let ids = self.tray.add(images, others);
        if images > 0 || others > 0 {
            self.edited();
        }
        ids
    }

    fn remove(&mut self, id: u64) {
        let before = self.tray.len();
        self.tray.remove(id);
        if self.tray.len() != before {
            self.edited();
        }
    }

    fn filed(&self) -> bool {
        matches!(self.result, Some(BugReportOutcome::Filed { .. }))
    }
}

/// The Feedback page's state — the web's `FeedbackBody` locals and the
/// route's `feedbackSending` / `feedbackResult`.
pub(super) struct FeedbackDraft {
    report: Report<ReadyShot>,
    steps_open: bool,
    /// Open by default: the consent line promises the preview is what gets
    /// sent, and a promise is only worth anything next to the thing.
    preview_open: bool,
    sending: bool,
    /// Send was pressed while a tile was still being prepared: the person
    /// attached it and expects it to go, so the send waits for it.
    waiting: bool,
    /// The tile open in the viewer, by id (C3).
    viewer: Option<u64>,
    /// A send that ended while the person was elsewhere.
    toast: Option<OutcomeToast>,
    /// Which toast a dismiss timer belongs to.
    toast_generation: u64,
    what_focus: gpui::FocusHandle,
    steps_focus: gpui::FocusHandle,
    /// Each tile is a tab stop (C1): Enter or Space opens it, and closing the
    /// viewer gives the keyboard back to it (C4).
    tile_focus: std::collections::HashMap<u64, gpui::FocusHandle>,
    /// The add target / add tile, where the keyboard goes when the tile it
    /// was on is removed from the viewer.
    add_focus: gpui::FocusHandle,
    /// The keyboard put focus on the tile or add target that has it — Tab,
    /// or Esc out of the viewer — so its ring may show. gpui's
    /// `focus_visible` asks only whether the LAST input was a key: a click
    /// on the add target, then ⌘V for one more, drew the ring on the stop
    /// the pointer had chosen. A press on the form, or opening the viewer,
    /// clears it.
    keyboard_focus: bool,
    /// `VELA_FEEDBACK_STATE=autosend[-away]`: send for real on the first
    /// frame — and, `true`, leave for the Wallet at once.
    autosend: Option<bool>,
}

impl FeedbackDraft {
    /// A picture is open in the viewer — a dialog over the window.
    pub(super) fn viewer_open(&self) -> bool {
        self.viewer.is_some()
    }

    pub(super) fn new(cx: &mut gpui::App) -> Self {
        Self {
            report: Report::default(),
            steps_open: false,
            preview_open: true,
            sending: false,
            waiting: false,
            viewer: None,
            toast: None,
            toast_generation: 0,
            what_focus: cx.focus_handle().tab_stop(true),
            steps_focus: cx.focus_handle().tab_stop(true),
            tile_focus: std::collections::HashMap::new(),
            add_focus: cx.focus_handle().tab_stop(true),
            keyboard_focus: false,
            autosend: None,
        }
    }

    /// Sending, or waiting on a tile before sending: the form holds still.
    fn busy(&self) -> bool {
        self.sending || self.waiting
    }

    pub(super) fn filed(&self) -> bool {
        self.report.filed()
    }
}

/// A size that grows with the person's text size (v2 A8, v3 B4) — icons and
/// the spinners beside words, which the `theme::text_*` sizes do for type.
fn scaled(base: f32) -> f32 {
    (base * crate::executor::appearance_prefs::text_factor()).round()
}

/// The remove badge's disc (v3 B1): OPAQUE, a fifteenth-and-a-bit of the
/// body colour into the fixed ink (`color.fixed.shadowInk`) — the web's
/// `color-mix(fg-base 15%, shadowInk)`. The ink itself on the light page; a
/// solid neutral a step lighter than the dark page there.
fn badge_disc(theme: &Theme) -> gpui::Hsla {
    let ink: gpui::Rgba = theme.shadow_ink.into();
    let fg: gpui::Rgba = theme.fg_base.into();
    let mix = |a: f32, b: f32| a * 0.15 + b * 0.85;
    gpui::Rgba {
        r: mix(fg.r, ink.r),
        g: mix(fg.g, ink.g),
        b: mix(fg.b, ink.b),
        a: 1.,
    }
    .into()
}

/// A border of any width — gpui's helpers stop at whole pixels, and the
/// success disc's ring is the web's 1.5 (`--border-emphasis`).
fn ring(mut element: Div, width: f32, color: gpui::Hsla) -> Div {
    let width: gpui::AbsoluteLength = px(width).into();
    element.style().border_widths = gpui::EdgesRefinement {
        top: Some(width),
        right: Some(width),
        bottom: Some(width),
        left: Some(width),
    };
    element.border_color(color)
}

fn larger(a: Pixels, b: Pixels) -> Pixels {
    if b > a { b } else { a }
}

fn smaller(a: Pixels, b: Pixels) -> Pixels {
    if b < a { b } else { a }
}

/// A line under the tiles or above the button: an icon on the first line's
/// centre, the words beside it. `text-sm`, as the web's `.shots-note` and
/// `.consent`.
fn note_line(
    window: &Window,
    icon: gpui::AnyElement,
    color: gpui::Hsla,
    text: SharedString,
) -> Div {
    let size = theme::text_label();
    let line = size * 1.5;
    let room = px(COLUMN_W - scaled(14.) - 4.);
    let width = even_width(window, &text, size, room);
    div()
        .flex()
        .items_start()
        .gap(px(4.))
        .text_size(size)
        .line_height(line)
        .text_color(color)
        .child(div().h(line).flex().flex_none().items_center().child(icon))
        .child(div().flex_1().min_w(px(0.)).max_w(width).child(text))
}

/// The width `text` wraps at inside `room`: balanced, so no word or 「钥。」
/// is left alone on the last line, and never opening a line with a closing
/// mark (kinsoku).
fn even_width(window: &Window, text: &SharedString, size: Pixels, room: Pixels) -> Pixels {
    crate::wallet::components::even_wrap_width(window, text, size, room)
}

/// The keyboard's ring (`focus_visible`, and only where the keyboard put
/// focus — `FeedbackDraft::keyboard_focus` — never after a click): solid,
/// 2, the info colour.
fn focus_ring(mut style: gpui::StyleRefinement, color: gpui::Hsla) -> gpui::StyleRefinement {
    style.border_style = Some(gpui::BorderStyle::Solid);
    style.border_2().border_color(color)
}

/// Digits that line up: "1 / 5" and "5 / 5" the same width.
fn tabular() -> gpui::FontFeatures {
    gpui::FontFeatures(Arc::new(vec![("tnum".into(), 1)]))
}

/// A press that did not travel. A trackpad drag that starts on a remove
/// badge and ends on it moves the pointer, and is not "remove this".
fn still_click(event: &gpui::ClickEvent) -> bool {
    match event {
        gpui::ClickEvent::Mouse(click) => {
            let moved = click.up.position - click.down.position;
            f32::from(moved.x).abs() <= 4. && f32::from(moved.y).abs() <= 4.
        }
        _ => true,
    }
}

/// The CTA while its report goes (v2 A3): the fill at full strength — never
/// dimmed — with the turning arc AND the sending words side by side.
fn busy_button(theme: &Theme, label: SharedString, primary: bool) -> Div {
    let (fg, face) = if primary {
        (theme.fg_inverse, div().bg(theme.accent))
    } else {
        (
            theme.fg_muted,
            div().border_1().border_color(theme.border_strong),
        )
    };
    let arc = px(scaled(16.));
    face.w_full()
        .min_h(px(52.))
        .px(px(24.))
        .py(px(8.))
        .rounded(px(12.))
        .flex()
        .items_center()
        .justify_center()
        .gap(px(scaled(10.)))
        .text_size(theme::text_button())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(crate::ui::spinner(fg, arc, px(2.)))
        .child(label)
}

impl WalletPage {
    /// What this device may say about itself in a report, and nothing more
    /// (the web's `deviceFacts`): the build, the platform, the language, the
    /// NAMES of the networks it cannot reach. No failure counters exist on
    /// this shell yet, so that line honestly says none.
    fn feedback_facts(&self, cx: &mut Context<Self>) -> bug_report::DeviceFacts {
        let unreachable = if self.identity.is_some() {
            resident::resident::<BalanceDashboard>(cx)
                .read(cx)
                .view()
                .unreachable_networks
                .iter()
                .map(|network| crate::executor::custom_tokens::network_name(network.chain_id))
                .collect()
        } else {
            Vec::new()
        };
        // Spec 099 FR-014: the browser tab's latest trouble, by layer and
        // reason — the report's "recent failures" line, never a page's params.
        let failures = self
            .shown_tab_view(cx)
            .and_then(|tab| tab.last_failure)
            .map(|note| {
                vec![format!(
                    "dapp: {}/{} ({})",
                    serde_json::to_value(note.layer)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    serde_json::to_value(note.reason)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    note.method
                )]
            })
            .unwrap_or_default();
        bug_report::DeviceFacts {
            version: env!("CARGO_PKG_VERSION").to_owned(),
            commit: env!("VELA_GIT_COMMIT").to_owned(),
            platform: bug_report::desktop_platform(),
            os: bug_report::desktop_os(),
            language: self.locale.to_string(),
            unreachable,
            failures,
        }
    }

    /// Whether the Feedback page is the panel on screen.
    fn feedback_on_screen(&self) -> bool {
        self.section == Section::Settings && self.settings_page == SettingsPage::Feedback
    }

    /// Say how a send the person walked away from ended — the toast lasts
    /// long enough to be acted on, and goes by itself.
    fn show_feedback_toast(&mut self, outcome: &BugReportOutcome, cx: &mut Context<Self>) {
        let toast = match outcome {
            BugReportOutcome::Filed { url, .. } => OutcomeToast {
                filed: true,
                url: url.clone(),
            },
            BugReportOutcome::Fallback { fallback_url, .. } => OutcomeToast {
                filed: false,
                url: fallback_url.clone(),
            },
        };
        self.feedback.toast = Some(toast);
        self.feedback.toast_generation += 1;
        let generation = self.feedback.toast_generation;
        cx.spawn(async move |page, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(10))
                .await;
            page.update(cx, |this, cx| {
                if this.feedback.toast_generation == generation {
                    this.feedback.toast = None;
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// Whether the Feedback form is the thing on screen, taking input.
    fn feedback_open(&self) -> bool {
        self.section == Section::Settings
            && self.settings_page == SettingsPage::Feedback
            && self.settings_dialog.is_none()
            && !self.feedback.filed()
    }

    /// Send pressed: now, or — a tile still being prepared — as soon as it
    /// is ready. Either way the form holds still from here (v3 B9), so the
    /// fields give up the caret.
    fn send_feedback(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.feedback.busy() || self.feedback.report.what.trim().is_empty() {
            return;
        }
        // A send is the next change: the refusal has been read. (A tile that
        // fails while this send waits says so again.)
        self.feedback.report.tray.clear_refusal();
        self.focus_handle.focus(window, cx);
        if self.feedback.report.tray.is_processing() {
            self.feedback.waiting = true;
            cx.notify();
            return;
        }
        self.send_feedback_now(cx);
    }

    /// Send the report, off the frame, and keep what comes back.
    fn send_feedback_now(&mut self, cx: &mut Context<Self>) {
        let screenshots = self
            .feedback
            .report
            .tray
            .ready_for_send()
            .unwrap_or_default()
            .into_iter()
            .map(|shot| shot.base64.to_string())
            .collect();
        let facts = self.feedback_facts(cx);
        let payload = self
            .feedback
            .report
            .payload(&self.settings.bug_labels, &facts, screenshots);
        self.feedback.sending = true;
        self.feedback.report.result = None;
        cx.notify();
        cx.spawn(async move |page, cx| {
            let outcome = cx
                .background_executor()
                .spawn(
                    async move { bug_report::send_bug_report(&payload, &bug_report::endpoint()) },
                )
                .await;
            page.update(cx, |this, cx| {
                // The other road is the one that works now, and its button
                // sits under the note: brought into view rather than left
                // below the fold (v3 B6). gpui draws no accessibility tree to
                // move a reader's focus in, so the scroll is the whole of it.
                if matches!(outcome, BugReportOutcome::Fallback { .. }) {
                    this.settings_scroll.scroll_to_bottom();
                }
                // Left the page while it went: the page still shows the
                // outcome when they come back, and a toast says it now.
                if !this.feedback_on_screen() {
                    this.show_feedback_toast(&outcome, cx);
                }
                this.feedback.sending = false;
                this.feedback.report.result = Some(outcome);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Take images from any of the three doors. The first that fit become
    /// tiles at once — each turning until its picture is ready — and each is
    /// prepared on the background executor.
    fn feedback_add(&mut self, sources: Vec<ShotSource>, others: usize, cx: &mut Context<Self>) {
        if self.feedback.busy() || self.feedback.filed() {
            return;
        }
        let ids = self.feedback.report.add(sources.len(), others);
        for (id, source) in ids.into_iter().zip(sources) {
            cx.spawn(async move |page, cx| {
                let ready = cx
                    .background_executor()
                    .spawn(async move { prepare_shot(source) })
                    .await;
                page.update(cx, |this, cx| this.feedback_prepared(id, ready, cx))
                    .ok();
            })
            .detach();
        }
        cx.notify();
    }

    /// One tile's picture came back — or could not be made, and the tile goes
    /// with the unsupported line. A send waiting on it goes once none is left.
    fn feedback_prepared(&mut self, id: u64, ready: Option<ReadyShot>, cx: &mut Context<Self>) {
        self.feedback.report.tray.finish(id, ready);
        if self.feedback.waiting && !self.feedback.report.tray.is_processing() {
            self.feedback.waiting = false;
            self.send_feedback_now(cx);
        }
        cx.notify();
    }

    /// Paths from the picker, a drop or a Finder copy: images are candidates,
    /// anything else is refused at once and takes no place.
    fn feedback_add_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        let (images, others): (Vec<PathBuf>, Vec<PathBuf>) = paths
            .into_iter()
            .partition(|path| screenshot_prep::is_candidate_path(path));
        let sources = images.into_iter().map(ShotSource::Path).collect();
        self.feedback_add(sources, others.len(), cx);
    }

    /// `autosend`: the pinned files through the real doors — added, prepared
    /// off the frame — and Send pressed; the send waits for the tiles as a
    /// real one does. `away`: the person leaves for the Wallet as it goes.
    fn feedback_autosend(&mut self, away: bool, cx: &mut Context<Self>) {
        let files: Vec<PathBuf> = crate::dev_env::var_os!("VELA_SCREENSHOT_FILES")
            .map(|list| std::env::split_paths(&list).collect())
            .unwrap_or_default();
        self.feedback_add_paths(files, cx);
        if self.feedback.report.tray.is_processing() {
            self.feedback.waiting = true;
        } else {
            self.send_feedback_now(cx);
        }
        if away {
            self.section = Section::Wallet;
        }
        cx.notify();
    }

    /// "Add screenshots": the system's own multi-file picker.
    fn feedback_pick(&mut self, cx: &mut Context<Self>) {
        if self.feedback.busy() {
            return;
        }
        if let Some(list) = crate::dev_env::var_os!("VELA_SCREENSHOT_FILES") {
            let paths = std::env::split_paths(&list).collect();
            self.feedback_add_paths(paths, cx);
            return;
        }
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: None,
        });
        cx.spawn(async move |page, cx| {
            // Cancelled, or the platform declined: nothing happened.
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            page.update(cx, |this, cx| this.feedback_add_paths(paths, cx))
                .ok();
        })
        .detach();
    }

    fn feedback_remove(&mut self, id: u64, cx: &mut Context<Self>) {
        if self.feedback.busy() {
            return;
        }
        self.feedback.report.remove(id);
        self.feedback.tile_focus.remove(&id);
        cx.notify();
    }

    /// Open the viewer at a ready tile (C1) — and take the keyboard with it,
    /// so what is typed while it is up goes nowhere (C4).
    fn feedback_open_viewer(&mut self, id: u64, window: &mut Window, cx: &mut Context<Self>) {
        let ready = self
            .feedback
            .report
            .tray
            .shots()
            .iter()
            .any(|shot| shot.id == id && shot.ready.is_some());
        if self.feedback.busy() || !ready {
            return;
        }
        self.feedback.viewer = Some(id);
        // Its ✕ and Remove are clicks: focus they hand back is the pointer's.
        // Esc says otherwise as it closes.
        self.feedback.keyboard_focus = false;
        self.focus_handle.focus(window, cx);
        cx.notify();
    }

    /// Close the viewer and give the keyboard back to the tile it showed —
    /// or to the add target, when that tile went (C4).
    fn feedback_close_viewer(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.feedback.viewer.take() {
            match self.feedback.tile_focus.get(&id) {
                Some(handle) => handle.focus(window, cx),
                None => self.feedback.add_focus.focus(window, cx),
            }
        }
        cx.notify();
    }

    /// The ready tiles' ids, in order, and where the viewer is among them.
    fn feedback_viewer_place(&self) -> (Vec<u64>, Option<usize>) {
        let ids: Vec<u64> = self
            .feedback
            .report
            .tray
            .shots()
            .iter()
            .filter(|shot| shot.ready.is_some())
            .map(|shot| shot.id)
            .collect();
        let at = self
            .feedback
            .viewer
            .and_then(|id| ids.iter().position(|shown| *shown == id));
        (ids, at)
    }

    /// ← / →: the previous or next picture; at either end it stays.
    fn feedback_viewer_step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let (ids, at) = self.feedback_viewer_place();
        let Some(at) = at else {
            return;
        };
        let next = if forward { at + 1 } else { at.wrapping_sub(1) };
        if let Some(id) = ids.get(next) {
            self.feedback.viewer = Some(*id);
            cx.notify();
        }
    }

    /// "Remove" in the viewer: the same tray rule as the badge. The next
    /// picture shows — the previous when it was the last — and removing the
    /// only one closes the viewer.
    fn feedback_viewer_remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (ids, at) = self.feedback_viewer_place();
        let (Some(at), Some(id)) = (at, self.feedback.viewer) else {
            return;
        };
        self.feedback.report.remove(id);
        self.feedback.tile_focus.remove(&id);
        let rest: Vec<u64> = ids.into_iter().filter(|shown| *shown != id).collect();
        self.feedback.viewer = rest.get(at.min(rest.len().saturating_sub(1))).copied();
        if self.feedback.viewer.is_none() {
            self.feedback.add_focus.focus(window, cx);
        }
        cx.notify();
    }

    /// Keys, heard in the capture phase from the page's root: the viewer's
    /// Esc / ← / → while it is up, and ⌘V / Ctrl+V with an image on the
    /// clipboard while the form is up — whether or not a field has the
    /// caret. `true` when this took the key, and nothing under it sees it.
    pub(super) fn feedback_capture_key(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if self.feedback.viewer.is_some() {
            let ks = &event.keystroke;
            let plain = !ks.modifiers.platform && !ks.modifiers.control && !ks.modifiers.alt;
            match ks.key.as_str() {
                "escape" => {
                    self.feedback.keyboard_focus = true;
                    self.feedback_close_viewer(window, cx);
                    return true;
                }
                "left" if plain => {
                    self.feedback_viewer_step(false, cx);
                    return true;
                }
                "right" if plain => {
                    self.feedback_viewer_step(true, cx);
                    return true;
                }
                _ => return false,
            }
        }
        if !self.feedback_open()
            || crate::ui::edit_chord(&event.keystroke) != Some(crate::ui::EditChord::Paste)
        {
            return false;
        }
        let Some(item) = cx.read_from_clipboard() else {
            return false;
        };
        let mut images = Vec::new();
        let mut paths = Vec::new();
        for entry in item.entries {
            match entry {
                gpui::ClipboardEntry::Image(image) => images.push(ShotSource::Bytes(image.bytes)),
                gpui::ClipboardEntry::ExternalPaths(external) => paths.extend(
                    external
                        .paths()
                        .iter()
                        .filter(|path| screenshot_prep::is_candidate_path(path))
                        .cloned(),
                ),
                gpui::ClipboardEntry::String(_) => {}
            }
        }
        images.extend(paths.into_iter().map(ShotSource::Path));
        if images.is_empty() {
            return false;
        }
        // Inert while a report goes: taken, so no field pastes either, and
        // nothing is added under the spinner.
        if !self.feedback.busy() {
            self.feedback_add(images, 0, cx);
        }
        true
    }

    /// 完成 on the filed state: an empty form for the next report.
    fn feedback_done(&mut self, cx: &mut Context<Self>) {
        let draft = &mut self.feedback;
        draft.report.what.clear();
        draft.report.steps.clear();
        draft.steps_open = false;
        draft.report.tray.clear();
        draft.report.result = None;
        draft.report.origin = None;
        draft.tile_focus.clear();
        cx.notify();
    }

    /// "Report this" on a relay stop (issue 466): the Feedback page — this
    /// app's one reporter, with its preview, consent line, Send and both
    /// endings — opened on the core's report as it stood at the press, filed
    /// under the core's area and key. A page, not a second sheet over the
    /// send: the desktop's reporter is a Settings page, and going there is a
    /// section switch like any other, so the send column goes as it does on
    /// any switch (the stop is what stopped it). The words can be edited; the
    /// key stays the core's so every report of this outage stays one issue.
    ///
    /// A report still going is left alone: the page opens on its progress,
    /// and pressing Report again once it ends starts this one.
    pub(super) fn open_relay_report(
        &mut self,
        report: vela_core::app::send::SendRelayReport,
        cx: &mut Context<Self>,
    ) {
        if !self.feedback.busy() {
            let vela_core::app::send::SendRelayReport {
                what,
                steps,
                area,
                fingerprint,
            } = report;
            let draft = &mut self.feedback;
            draft
                .report
                .seed(what, steps, bug_report::ReportOrigin { area, fingerprint });
            // The steps are part of what is sent: shown, not folded away.
            draft.steps_open = true;
            draft.viewer = None;
            draft.tile_focus.clear();
            draft.keyboard_focus = false;
        }
        self.close_switcher(cx);
        self.section = Section::Settings;
        self.settings_page = SettingsPage::Feedback;
        self.settings_dialog = None;
        self.settings_probed_panel = None;
        self.panel = PanelId::None;
        self.menu = None;
        cx.notify();
    }

    /// `VELA_FEEDBACK_STATE` — this page in one state, on the mock route
    /// (`VELA_PAGE=settings`), for a screenshot pass that cannot click, drop
    /// or paste. The same env-pin family as `VELA_SETTINGS_STATE`. The
    /// tiles are `VELA_SCREENSHOT_FILES`, prepared by the real pipeline.
    /// States: `blank`, `empty`, `one`, `five`, `limit`, `unsupported`,
    /// `processing`, `sending`, `filed`, `dropped`, `fallback`, `viewer` —
    /// and `autosend` / `autosend-away`, which are not pictures but a real
    /// send through the real code (tiles prepared, the endpoint called,
    /// whatever it answers shown), `-away` leaving for the Wallet as it goes.
    /// `relay` is the form a relay stop's "Report this" opens (issue 466),
    /// and `relay-autosend` that report sent for real, the same way.
    pub(super) fn pin_feedback_state(&mut self, state: &str) {
        self.section = Section::Settings;
        self.settings_page = SettingsPage::Feedback;
        self.settings_dialog = None;
        let files: Vec<PathBuf> = crate::dev_env::var_os!("VELA_SCREENSHOT_FILES")
            .map(|list| std::env::split_paths(&list).collect())
            .unwrap_or_default();
        let ready: Vec<ReadyShot> = files
            .into_iter()
            .filter_map(|path| prepare_shot(ShotSource::Path(path)))
            .collect();
        let draft = &mut self.feedback;
        let attach = |count: usize, draft: &mut FeedbackDraft| {
            for shot in ready.iter().cycle().take(count) {
                if let Some(id) = draft.report.tray.add(1, 0).first().copied() {
                    draft.report.tray.finish(id, Some(shot.clone()));
                }
            }
        };
        if state != "blank" {
            draft.report.what = if self.locale.starts_with("zh") {
                "在 Base 上确认发送后，界面卡住了，按钮一直在转圈。".to_owned()
            } else {
                "After I confirmed a send on Base, the screen froze and the button kept spinning."
                    .to_owned()
            };
        }
        let fallback_url = || {
            bug_report::prefilled_issue_url(&bug_report::build_bug_report(
                "fixture",
                "",
                bug_report::AREA_OTHER,
                &bug_report::EnvironmentLabels::default(),
                &bug_report::DeviceFacts::default(),
                Vec::new(),
            ))
        };
        // The button a send is about sits at the bottom: shown, as the real
        // fallback scrolls to it.
        if matches!(state, "sending" | "fallback") {
            self.settings_scroll.scroll_to_bottom();
        }
        let draft = &mut self.feedback;
        match state {
            "one" => attach(1, draft),
            "five" => attach(5, draft),
            "limit" => {
                attach(4, draft);
                draft.report.tray.add(3, 0);
                // The one place left went to the first of the three; show it
                // ready, as it would be a moment later.
                if let Some(id) = draft.report.tray.shots().last().map(|shot| shot.id)
                    && let Some(shot) = ready.first()
                {
                    draft.report.tray.finish(id, Some(shot.clone()));
                }
            }
            "unsupported" => {
                attach(2, draft);
                draft.report.tray.add(0, 1);
            }
            "processing" => {
                attach(2, draft);
                draft.report.tray.add(1, 0);
            }
            "sending" => {
                attach(3, draft);
                draft.sending = true;
            }
            "filed" | "dropped" => {
                draft.report.result = Some(BugReportOutcome::Filed {
                    number: 4242,
                    url: "https://github.com/mondaylabsltd/vela-wallet/issues/4242".to_owned(),
                    deduped: false,
                    screenshots_dropped: if state == "dropped" { 2 } else { 0 },
                });
            }
            "fallback" => {
                attach(2, draft);
                draft.report.result = Some(BugReportOutcome::Fallback {
                    fallback_url: fallback_url(),
                    with_screenshots: true,
                });
            }
            "viewer" => {
                attach(3, draft);
                draft.viewer = draft.report.tray.shots().get(1).map(|shot| shot.id);
            }
            // A REAL send, made before the first frame: a refused connection
            // answers at once, so the page opens on the outcome the endpoint
            // (`VELA_BUG_REPORT_ENDPOINT`) really gave.
            "offline" => {
                attach(2, draft);
                let screenshots = draft
                    .report
                    .tray
                    .ready_for_send()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|shot| shot.base64.to_string())
                    .collect();
                let payload = bug_report::build_bug_report(
                    &draft.report.what,
                    "",
                    bug_report::AREA_OTHER,
                    &self.settings.bug_labels,
                    &bug_report::DeviceFacts {
                        version: env!("CARGO_PKG_VERSION").to_owned(),
                        platform: bug_report::desktop_platform(),
                        os: bug_report::desktop_os(),
                        ..bug_report::DeviceFacts::default()
                    },
                    screenshots,
                );
                draft.report.result = Some(bug_report::send_bug_report(
                    &payload,
                    &bug_report::endpoint(),
                ));
            }
            // The toast a send the person walked away from ends in, over the
            // Wallet — filed, or handed to the form.
            "toast" | "toast-fallback" => {
                let filed = state == "toast";
                draft.toast = Some(OutcomeToast {
                    filed,
                    url: if filed {
                        "https://github.com/mondaylabsltd/vela-wallet/issues/4242".to_owned()
                    } else {
                        fallback_url()
                    },
                });
                self.section = Section::Wallet;
            }
            "autosend" => draft.autosend = Some(false),
            "autosend-away" => draft.autosend = Some(true),
            // Issue 466: seeded as the press seeds it, with the core's words
            // for Unichain's empty treasury, its area and its key.
            "relay" | "relay-autosend" => {
                let (what, steps, origin) = relay_report_sample();
                draft.report.seed(what, steps, origin);
                draft.steps_open = true;
                if state == "relay-autosend" {
                    draft.autosend = Some(false);
                }
            }
            _ => {}
        }
    }

    /// ST15 / the web's `FeedbackBody`: what went wrong, the optional steps,
    /// the screenshots, exactly what will be sent — open, with the consent
    /// note beside the button it is about — and the two ways a send ends.
    pub(super) fn settings_feedback(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        if let Some(BugReportOutcome::Filed {
            number,
            url,
            deduped,
            screenshots_dropped,
        }) = self.feedback.report.result.clone()
        {
            return self.feedback_filed(
                theme,
                window,
                number,
                url,
                deduped,
                screenshots_dropped,
                cx,
            );
        }

        if let Some(away) = self.feedback.autosend.take() {
            self.feedback_autosend(away, cx);
        }
        let busy = self.feedback.busy();
        let info = theme.info_base;
        let s = &self.settings;
        let mut col = div()
            .group("feedback-form")
            .flex()
            .flex_col()
            .gap(px(16.))
            .pt(px(8.))
            .max_w(px(560.))
            // A drop anywhere on the form (the web takes it on the section;
            // a desktop window is a bigger target, and this page has one
            // thing a file could be for).
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.feedback_add_paths(paths.paths().to_vec(), cx);
            }))
            // A press anywhere on the form: whatever it focuses, the pointer
            // chose it, and no key pressed after it draws the ring there.
            .capture_any_mouse_down(cx.listener(|this, _, _, cx| {
                if this.feedback.keyboard_focus {
                    this.feedback.keyboard_focus = false;
                    cx.notify();
                }
            }))
            // Tab walks the form's stops: the two wells, each tile, the add
            // target (C1). Nothing else in this shell binds Tab, so it is
            // the form's to take. (⇧Tab never arrives on macOS at this gpui:
            // a key with no character goes to the input context first.)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                if ks.key != "tab"
                    || ks.modifiers.platform
                    || ks.modifiers.control
                    || ks.modifiers.alt
                {
                    return;
                }
                if ks.modifiers.shift {
                    window.focus_prev(cx);
                } else {
                    window.focus_next(cx);
                }
                this.feedback.keyboard_focus = true;
                cx.stop_propagation();
                cx.notify();
            }));

        let what_placeholder = s.bug_what_placeholder.clone();
        let what_focus = self.feedback.what_focus.clone();
        let page = cx.entity().downgrade();
        col = col.child(crate::ui::text_area(
            "feedback-what",
            theme,
            &self.feedback.report.what,
            what_placeholder,
            4,
            busy,
            &what_focus,
            window,
            move |text, _, cx| {
                let _ = page.update(cx, |this, cx| {
                    if this.feedback.busy() {
                        return;
                    }
                    this.feedback.report.set_what(text);
                    cx.notify();
                });
            },
        ));
        if self.feedback.steps_open {
            let steps_focus = self.feedback.steps_focus.clone();
            let page = cx.entity().downgrade();
            col = col.child(crate::ui::text_area(
                "feedback-steps",
                theme,
                &self.feedback.report.steps,
                s.bug_steps_placeholder.clone(),
                3,
                busy,
                &steps_focus,
                window,
                move |text, _, cx| {
                    let _ = page.update(cx, |this, cx| {
                        if this.feedback.busy() {
                            return;
                        }
                        this.feedback.report.set_steps(text);
                        cx.notify();
                    });
                },
            ));
        } else {
            col = col.child(
                div().flex().child(
                    div()
                        .id("feedback-add-steps")
                        // Inert while sending: no pointer, no press.
                        .when(!busy, |el| el.cursor_pointer().active(|el| el.opacity(0.7)))
                        .text_size(theme::text_body())
                        .text_color(info)
                        .child(s.bug_add_steps.clone())
                        .on_click(cx.listener(|this, _, window, cx| {
                            if this.feedback.busy() {
                                return;
                            }
                            this.feedback.steps_open = true;
                            this.feedback.steps_focus.focus(window, cx);
                            cx.notify();
                        })),
                ),
            );
        }

        col = col.child(self.feedback_screenshots(theme, window, cx));
        col = col.child(self.feedback_preview(theme, window, cx));

        // The consent sits directly above the button it is a promise about —
        // quiet, not boxed (v2 A4): the blue-on-blue box failed 4.5:1.
        let s = &self.settings;
        let consent = s.bug_consent.clone();
        col = col.child(note_line(
            window,
            icon_img(&mut self.icons, Icon::Info, false, info, scaled(14.)).into_any_element(),
            theme.fg_muted,
            consent,
        ));

        // The endpoint could not file it: the other road, with the person's
        // words already in it — amber, because nothing has been lost. Title,
        // body and the screenshots note are three parts, never one glued
        // string (v2 A2).
        let fallback = match &self.feedback.report.result {
            Some(BugReportOutcome::Fallback {
                fallback_url,
                with_screenshots,
            }) => Some((fallback_url.clone(), *with_screenshots)),
            _ => None,
        };
        if let Some((url, with_screenshots)) = fallback.clone() {
            let s = &self.settings;
            // Each paragraph wraps where CJK typesetting allows (kinsoku).
            let room = px(COLUMN_W - 24. - scaled(18.) - 8.);
            let size = theme::text_body();
            let paragraph = |text: SharedString| {
                let width = even_width(window, &text, size, room);
                div().max_w(width).child(text)
            };
            let mut text = div()
                .flex()
                .flex_col()
                .flex_1()
                .min_w(px(0.))
                .gap(px(2.))
                .child(
                    div()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child(s.bug_fallback_title.clone()),
                )
                .child(paragraph(s.bug_fallback_body.clone()));
            // Its own paragraph, 8 apart (the column's 2 + 6): the images are
            // a separate matter (v2 A2).
            if with_screenshots {
                text = text.child(paragraph(s.bug_fallback_screenshots.clone()).pt(px(6.)));
            }
            let body = theme::text_body();
            let line = body * 1.5;
            col = col.child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(8.))
                    .p(px(12.))
                    .rounded(px(12.))
                    .bg(theme.warning_soft)
                    .text_size(body)
                    .line_height(line)
                    .text_color(theme.fg_base)
                    .child(
                        div()
                            .h(line)
                            .flex()
                            .flex_none()
                            .items_center()
                            .child(icon_img(
                                &mut self.icons,
                                Icon::TriangleAlert,
                                false,
                                theme.warning_base,
                                scaled(18.),
                            )),
                    )
                    .child(text),
            );
            let s = &self.settings;
            col = col.child(
                div()
                    .id("feedback-open-github")
                    .cursor_pointer()
                    .active(|el| el.opacity(0.85))
                    .child(crate::flows::components::accent_button(
                        theme,
                        s.bug_open_github.clone(),
                    ))
                    .on_click(move |_, _, cx| crate::executor::opener::open(&url, cx)),
            );
        }

        // Send — a secondary "Try again" once the other road is on screen,
        // since a retry is a real answer to a 429. Busy is the arc and the
        // sending words, never a dimmed button; nothing typed is nothing to
        // file. Screenshots never block it.
        let s = &self.settings;
        let ready = !self.feedback.report.what.trim().is_empty();
        let fallen = fallback.is_some();
        let label = if fallen {
            s.bug_try_again.clone()
        } else {
            s.bug_send.clone()
        };
        let button = if busy {
            busy_button(theme, s.bug_sending.clone(), !fallen)
        } else if !ready {
            crate::flows::components::disabled_accent_button(theme, label)
        } else if fallen {
            crate::flows::components::secondary_button(theme, label)
        } else {
            crate::flows::components::accent_button(theme, label)
        };
        col = col.child(
            div()
                .id("feedback-send")
                .when(ready && !busy, |el| {
                    el.cursor_pointer().active(|el| el.opacity(0.85))
                })
                .child(button)
                .on_click(cx.listener(|this, _, window, cx| this.send_feedback(window, cx))),
        );
        // In the fallback state the block above already offers the form.
        if !fallen {
            col = col.child(
                div().flex().justify_center().child(
                    div()
                        .id("feedback-github-form")
                        .cursor_pointer()
                        .text_size(theme::text_body())
                        .text_color(info)
                        .active(|el| el.opacity(0.7))
                        .child(s.bug_open_github_form.clone())
                        .on_click(|_, _, cx| {
                            crate::executor::opener::open(bug_report::GITHUB_ISSUE_FORM, cx)
                        }),
                ),
            );
        }
        col
    }

    /// The screenshots section: a heading with the hint or the count, then
    /// the one add target, or the row of tiles; the refusal line, then the
    /// public line.
    fn feedback_screenshots(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let busy = self.feedback.busy();
        let s = &self.settings;
        let count = self.feedback.report.tray.len();
        let empty = self.feedback.report.tray.is_empty();
        let max = screenshot_prep::MAX_SCREENSHOTS.to_string();
        let trailing = if empty {
            crate::wallet::fill(&s.bug_screenshots_hint, "max", &max)
        } else {
            format!("{count} / {max}")
        };
        let info_soft = theme.info_soft;
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(8.))
            // The room a drop's tint takes past the column, taken back so the
            // section's contents line up with the fields.
            .mx(px(-4.))
            .p(px(4.))
            .rounded(px(12.))
            .group_drag_over::<ExternalPaths>("feedback-form", move |style| style.bg(info_soft))
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .justify_between()
                    .gap(px(8.))
                    .text_size(theme::text_label())
                    .child(
                        div()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(s.bug_screenshots_label.clone()),
                    )
                    // fg-muted, not fg-subtle: subtle failed 4.5:1 (v3 B8).
                    .child(
                        div()
                            .text_color(theme.fg_muted)
                            .font_features(tabular())
                            .child(SharedString::from(trailing)),
                    ),
            );

        let icon_px = scaled(20.);
        if empty {
            let add = s.bug_add_screenshots.clone();
            let hint = s.bug_drop_hint.clone();
            let raised = theme.bg_raised;
            let add_focus = self.feedback.add_focus.clone();
            let ring_color = theme.info_base;
            let keyboard = self.feedback.keyboard_focus;
            section = section.child(
                div()
                    .id("feedback-add-screenshots")
                    .w_full()
                    .min_h(px(56.))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(12.))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border_strong)
                    .when(!busy, |el| {
                        el.track_focus(&add_focus).when(keyboard, |el| {
                            el.focus_visible(move |style| focus_ring(style, ring_color))
                        })
                    })
                    .bg(theme.bg_sunken)
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_center()
                    .gap(px(2.))
                    .when(!busy, |el| {
                        el.cursor_pointer()
                            .hover(move |el| el.bg(raised))
                            .active(|el| el.opacity(0.85))
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.feedback_pick(cx)))
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .text_size(theme::text_body())
                            .text_color(theme.fg_muted)
                            .child(icon_img(
                                &mut self.icons,
                                Icon::ImagePlus,
                                false,
                                theme.fg_muted,
                                icon_px,
                            ))
                            .child(add),
                    )
                    // Dropping and pasting are pointer-and-keyboard habits,
                    // which is every desktop.
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_muted)
                            .text_center()
                            .child(hint),
                    ),
            );
        } else {
            section = section.child(self.feedback_tiles(theme, busy, cx));
        }

        let s = &self.settings;
        if let Some(refusal) = self.feedback.report.tray.refusal() {
            let words = match refusal {
                Refusal::Limit => {
                    SharedString::from(crate::wallet::fill(&s.bug_screenshots_limit, "max", &max))
                }
                Refusal::Unsupported => s.bug_screenshot_unsupported.clone(),
            };
            section = section.child(note_line(
                window,
                icon_img(
                    &mut self.icons,
                    Icon::TriangleAlert,
                    false,
                    theme.warning_base,
                    scaled(14.),
                )
                .into_any_element(),
                theme.warning_base,
                words,
            ));
        }
        // The founder's ruling, where it counts: before Send, and never
        // displaced by a refusal (v2 A1).
        if !empty {
            let public = self.settings.bug_screenshots_public.clone();
            section = section.child(note_line(
                window,
                icon_img(
                    &mut self.icons,
                    Icon::Eye,
                    false,
                    theme.fg_muted,
                    scaled(14.),
                )
                .into_any_element(),
                theme.fg_muted,
                public,
            ));
        }
        section
    }

    /// One row, always (v2 A6): the tiles, then — while there is room — the
    /// add tile. The row's top padding is the room the remove areas grow
    /// into past the tiles' tops (v3 B10), taken back from the gap under the
    /// heading so the row sits where it would.
    fn feedback_tiles(&mut self, theme: &Theme, busy: bool, cx: &mut Context<Self>) -> Div {
        let reach = REMOVE_HIT - BADGE;
        let mut row = div()
            .flex()
            .gap(px(TILE_GAP))
            .mt(px(-(reach - BADGE_OVERLAP - 2.)))
            .pt(px(reach));
        let disc = badge_disc(theme);
        let white = theme.fg_inverse;
        let ring_color = theme.info_base;
        let keyboard = self.feedback.keyboard_focus;
        let shots: Vec<(u64, Option<Arc<gpui::RenderImage>>)> = self
            .feedback
            .report
            .tray
            .shots()
            .iter()
            .map(|shot| (shot.id, shot.ready.as_ref().map(|r| Arc::clone(&r.thumb))))
            .collect();
        for (id, thumb) in shots {
            let thumb_ready = thumb.is_some();
            let picture = match thumb {
                Some(thumb) => img(ImageSource::Render(thumb))
                    .size_full()
                    .rounded(px(12.))
                    .into_any_element(),
                // Being prepared: the sunken tile with a small turning arc.
                None => div()
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(crate::ui::spinner(theme.fg_muted, px(scaled(20.)), px(2.)))
                    .into_any_element(),
            };
            // The area is the comfortable target, the badge is what is seen:
            // the badge sits in the area's inner corner, overhanging the
            // tile by 4, and the area reaches into the picture only as far
            // as the badge does.
            let inset = REMOVE_HIT - BADGE - BADGE_OVERLAP - BADGE_RING;
            let remove = div()
                .id(("feedback-remove", id))
                .absolute()
                .top(px(-reach))
                .right(px(-reach))
                .size(px(REMOVE_HIT))
                .flex()
                .items_start()
                .justify_end()
                .pt(px(inset))
                .pr(px(inset))
                .when(!busy, |el| {
                    el.cursor_pointer()
                        .active(|el| el.opacity(0.7))
                        .on_click(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                            // The badge overhangs the tile: its press is the
                            // badge's alone, never also "open this one".
                            cx.stop_propagation();
                            if still_click(event) {
                                this.feedback_remove(id, cx);
                            }
                        }))
                })
                .child(
                    div()
                        .size(px(BADGE + 2. * BADGE_RING))
                        .flex_none()
                        .rounded_full()
                        .bg(theme.bg_base)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            div()
                                .size(px(BADGE))
                                .rounded_full()
                                .bg(disc)
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_img(&mut self.icons, Icon::X, false, white, 12.)),
                        ),
                );
            let openable = thumb_ready && !busy;
            let handle = self
                .feedback
                .tile_focus
                .entry(id)
                .or_insert_with(|| cx.focus_handle().tab_stop(true))
                .clone();
            row = row.child(
                div()
                    .id(("feedback-tile", id))
                    .relative()
                    .flex_none()
                    .size(px(TILE))
                    // A tab stop while it can be opened; Enter or Space is
                    // gpui's keyboard click on it (C1).
                    .when(openable, |el| {
                        el.track_focus(&handle)
                            .rounded(px(14.))
                            .when(keyboard, |el| {
                                el.focus_visible(move |style| focus_ring(style, ring_color))
                            })
                    })
                    // The tile is a button (C1): its picture opens the
                    // viewer at it. A tile still being prepared is not
                    // openable, and nothing opens while a report goes (C5).
                    .when(openable, |el| {
                        el.cursor_pointer()
                            .active(|el| el.opacity(0.85))
                            .on_click(cx.listener(
                                move |this, event: &gpui::ClickEvent, window, cx| {
                                    if still_click(event) {
                                        this.feedback_open_viewer(id, window, cx);
                                    }
                                },
                            ))
                    })
                    .child(
                        div()
                            .size_full()
                            .rounded(px(12.))
                            .border_1()
                            .border_color(theme.border_card)
                            .bg(theme.bg_sunken)
                            .overflow_hidden()
                            .child(picture),
                    )
                    .child(remove),
            );
        }
        if self.feedback.report.tray.len() < screenshot_prep::MAX_SCREENSHOTS {
            let raised = theme.bg_raised;
            let add_focus = self.feedback.add_focus.clone();
            row = row.child(
                div()
                    .id("feedback-add-tile")
                    .flex_none()
                    .size(px(TILE))
                    .rounded(px(12.))
                    .border_1()
                    .border_dashed()
                    .border_color(theme.border_strong)
                    .when(!busy, |el| {
                        el.track_focus(&add_focus).when(keyboard, |el| {
                            el.focus_visible(move |style| focus_ring(style, ring_color))
                        })
                    })
                    .bg(theme.bg_sunken)
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(!busy, |el| {
                        el.cursor_pointer()
                            .hover(move |el| el.bg(raised))
                            .active(|el| el.opacity(0.85))
                    })
                    .on_click(cx.listener(|this, _, _, cx| this.feedback_pick(cx)))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::ImagePlus,
                        false,
                        theme.fg_muted,
                        scaled(20.),
                    )),
            );
        }
        row
    }

    /// "What will be sent" (v2 A4, v3 B2): the header in the muted colour
    /// with a chevron that grows with the text; open, plain label / value
    /// rows — no box, no monospace, no colon — each value wrapping under
    /// itself. The PAYLOAD keeps its "label: value" lines; only the drawing
    /// splits them.
    fn feedback_preview(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        let lines =
            bug_report::environment_lines(&self.settings.bug_labels, &self.feedback_facts(cx));
        let open = self.feedback.preview_open;
        let size = theme::text_label();
        let toggle = self.settings.bug_preview_toggle.clone();
        let mut disclosure = div().flex().flex_col().gap(px(8.)).child(
            div().flex().child(
                div()
                    .id("feedback-preview")
                    .flex()
                    .items_center()
                    .gap(px(4.))
                    .cursor_pointer()
                    .text_size(size)
                    .text_color(theme.fg_muted)
                    .active(|el| el.opacity(0.7))
                    .child(toggle)
                    .child(icon_img(
                        &mut self.icons,
                        if open {
                            Icon::ChevronUp
                        } else {
                            Icon::ChevronDown
                        },
                        false,
                        theme.fg_muted,
                        scaled(14.),
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.feedback.preview_open = !this.feedback.preview_open;
                        cx.notify();
                    })),
            ),
        );
        if open {
            let rows: Vec<(Option<String>, String)> = lines
                .iter()
                .map(|line| match line.split_once(": ") {
                    Some((label, value)) => (Some(label.to_owned()), value.to_owned()),
                    None => (None, line.clone()),
                })
                .collect();
            // The label column is the widest label (the web's
            // `fit-content(40%)`): measured, since gpui's grid has only
            // equal columns.
            let widest = rows
                .iter()
                .filter_map(|(label, _)| label.as_deref())
                .map(|label| crate::wallet::components::text_width(window, label, size))
                .fold(px(0.), larger);
            let label_w = smaller(widest + px(1.), px(560. * 0.4));
            let mut body = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .text_size(size)
                .line_height(size * 1.5);
            for (label, value) in rows {
                let row = match label {
                    Some(label) => div()
                        .flex()
                        .items_start()
                        .gap(px(12.))
                        .child(
                            div()
                                .w(label_w)
                                .flex_none()
                                .text_color(theme.fg_muted)
                                .child(SharedString::from(label)),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_color(theme.fg_base)
                                .child(SharedString::from(value)),
                        ),
                    None => div()
                        .text_color(theme.fg_base)
                        .child(SharedString::from(value)),
                };
                body = body.child(row);
            }
            disclosure = disclosure.child(body);
        }
        disclosure
    }

    /// Filed (v2 A7, v3 B3). The number is the whole point: a person who
    /// reported something is owed a way back to it. The block sits at the
    /// 2:3 optical centre of the panel's visible height. No accent — nothing
    /// here moves value — unless images were lost: then the issue page is
    /// where they get added, and "View on GitHub" leads. Done is always the
    /// quiet text button under it.
    #[allow(clippy::too_many_arguments)]
    fn feedback_filed(
        &mut self,
        theme: &Theme,
        window: &Window,
        number: u64,
        url: String,
        deduped: bool,
        dropped: u32,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = &self.settings;
        let body = SharedString::from(crate::wallet::fill(
            if deduped {
                &s.bug_success_deduped
            } else {
                &s.bug_success_new
            },
            "number",
            &number.to_string(),
        ));
        let title = s.bug_success_title.clone();
        let view = s.bug_view_issue.clone();
        let done = s.bug_done.clone();
        let dropped_line = s.bug_screenshots_dropped.clone();
        let measure = px(scaled(300.));
        let body_size = theme::text_body();
        let body_w = even_width(window, &body, body_size, measure);
        let dropped_w = even_width(window, &dropped_line, body_size, measure);

        let disc = ring(
            div()
                .size(px(56.))
                .mb(px(4.))
                .rounded_full()
                .bg(theme.success_soft)
                .flex()
                .items_center()
                .justify_center(),
            1.5,
            theme.success_base.opacity(0.4),
        )
        .child(icon_img(
            &mut self.icons,
            Icon::Check,
            false,
            theme.success_base,
            28.,
        ));

        let mut block = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .text_center()
            .child(disc)
            .child(
                div()
                    .text_size(theme::text_section())
                    .line_height(theme::text_section() * 1.25)
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(title),
            )
            .child(
                div()
                    .w(body_w)
                    .text_size(body_size)
                    .line_height(body_size * 1.5)
                    .text_color(theme.fg_muted)
                    .child(body),
            );
        if dropped > 0 {
            block = block.child(
                div()
                    .w(dropped_w)
                    .text_size(body_size)
                    .line_height(body_size * 1.5)
                    .text_color(theme.warning_base)
                    .child(dropped_line),
            );
        }
        let mut actions = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .w_full()
            .max_w(px(360.))
            .mt(px(12.));
        if !url.is_empty() {
            let button = if dropped > 0 {
                crate::flows::components::accent_button(theme, view)
            } else {
                crate::flows::components::secondary_button(theme, view)
            };
            actions = actions.child(
                div()
                    .id("feedback-view-issue")
                    .w_full()
                    .cursor_pointer()
                    .active(|el| el.opacity(0.85))
                    .child(button)
                    .on_click(move |_, _, cx| crate::executor::opener::open(&url, cx)),
            );
        }
        let muted = theme.fg_muted;
        let base = theme.fg_base;
        actions = actions.child(
            div()
                .id("feedback-done")
                .min_h(px(44.))
                .px(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(12.))
                .cursor_pointer()
                .text_size(body_size)
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(muted)
                .hover(move |el| el.text_color(base))
                .active(|el| el.opacity(0.7))
                .child(done)
                .on_click(cx.listener(|this, _, _, cx| this.feedback_done(cx))),
        );
        block = block.child(actions);

        // The panel gives the filed state its whole visible height
        // (`settings_panel`), so this fills it and puts the block at the 2:3
        // optical centre — two parts of the free room above, three below —
        // under the form's column, whatever the text size. Taller than the
        // room (the largest text in a short window), it scrolls with the
        // panel instead.
        div()
            .flex_1()
            .min_h(px(0.))
            .w_full()
            .max_w(px(COLUMN_W))
            .flex()
            .flex_col()
            .child(div().flex_grow(2.).flex_shrink(1.).flex_basis(px(0.)))
            .child(block.flex_none())
            .child(div().flex_grow(3.).flex_shrink(1.).flex_basis(px(0.)))
    }
}

impl WalletPage {
    /// The screenshot viewer (078 round 3, C3): a lightbox over the whole
    /// window — the picture as it will be sent, at most 90% of the window's
    /// width (less the arrows' room) and 85% of its height; the count and
    /// Remove on solid pills, so no page text behind the dim can ever touch
    /// them; ← / → at the sides (and the arrow keys); ✕ top-right, Esc or a
    /// click on the dim to close. It stays dark in both themes, as a photo
    /// viewer does.
    pub(super) fn feedback_viewer(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        self.feedback.viewer?;
        // The viewer belongs to the Feedback page: leaving it closes it.
        if !self.feedback_on_screen() || self.feedback.busy() {
            self.feedback.viewer = None;
            return None;
        }
        let (ids, at) = self.feedback_viewer_place();
        let Some(at) = at else {
            self.feedback.viewer = None;
            return None;
        };
        let shot = self
            .feedback
            .report
            .tray
            .shots()
            .iter()
            .find(|shot| Some(shot.id) == self.feedback.viewer)
            .and_then(|shot| shot.ready.clone())?;
        let count = ids.len();
        // The dark palette whatever the app's theme: the viewer is dark.
        let dark = Theme::dark();
        let ink = theme.fg_inverse;
        let pill = dark.bg_raised;
        let pill_hover = dark.border_strong;
        let danger = dark.error_base;
        let scrim = gpui::Hsla {
            a: 0.97,
            ..theme.backdrop
        };

        const ARROW: f32 = 44.;
        const ARROW_INSET: f32 = 24.;
        const PILL_H: f32 = 44.;
        const GAP: f32 = 14.;
        let viewport = window.viewport_size();
        let (vw, vh) = (f32::from(viewport.width), f32::from(viewport.height));
        // Beside the picture: an arrow and its inset on each side, and a
        // little air, so an arrow never sits on the picture.
        let room_w = (vw * 0.9).min(vw - 2. * (ARROW_INSET + ARROW + 16.));
        let counter_h = if count > 1 { scaled(28.) + GAP } else { 0. };
        let room_h = vh * 0.85 - counter_h - PILL_H.max(scaled(PILL_H)) - GAP;
        let (w, h) = (shot.width.max(1) as f32, shot.height.max(1) as f32);
        let fit = (room_w.max(80.) / w).min(room_h.max(80.) / h).min(1.);
        let (shown_w, shown_h) = ((w * fit).round(), (h * fit).round());
        let remove = self.settings.bug_remove_from_viewer.clone();
        let counter = SharedString::from(format!("{} / {count}", at + 1));
        let top = CAPTION_H + 12.;
        let middle = vh / 2. - ARROW / 2.;

        let round = |id: &'static str, icons: &mut IconCache, icon: Icon| {
            div()
                .id(id)
                .occlude()
                .absolute()
                .size(px(ARROW))
                .rounded_full()
                .bg(pill)
                .hover(move |el| el.bg(pill_hover))
                .active(|el| el.opacity(0.7))
                .cursor_pointer()
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(icons, icon, false, ink, scaled(18.)))
        };

        let mut layer = div()
            .id("feedback-viewer")
            .occlude()
            .absolute()
            .inset_0()
            .bg(scrim)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(GAP))
            .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
            .on_click(cx.listener(|this, _, window, cx| this.feedback_close_viewer(window, cx)));
        if count > 1 {
            layer = layer.child(
                div()
                    .px(px(12.))
                    .min_h(px(scaled(28.)))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .bg(pill)
                    .text_size(theme::text_row_sub())
                    .font_features(tabular())
                    .text_color(ink.opacity(0.82))
                    .child(counter),
            );
        }
        layer = layer
            .child(
                div()
                    .id("feedback-viewer-picture")
                    .occlude()
                    .w(px(shown_w))
                    .h(px(shown_h))
                    .rounded(px(8.))
                    .overflow_hidden()
                    .child(img(ImageSource::Render(shot.full)).size_full()),
            )
            .child(
                div()
                    .id("feedback-viewer-remove")
                    .occlude()
                    .min_h(px(PILL_H))
                    .px(px(20.))
                    .rounded_full()
                    .flex()
                    .items_center()
                    .bg(pill)
                    .cursor_pointer()
                    .hover(move |el| el.bg(pill_hover))
                    .active(|el| el.opacity(0.7))
                    .text_size(theme::text_body())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(danger)
                    .child(remove)
                    .on_click(
                        cx.listener(|this, _, window, cx| this.feedback_viewer_remove(window, cx)),
                    ),
            )
            .child(
                round("feedback-viewer-close", &mut self.icons, Icon::X)
                    .top(px(top))
                    .right(px(20.))
                    .on_click(
                        cx.listener(|this, _, window, cx| this.feedback_close_viewer(window, cx)),
                    ),
            );
        if at > 0 {
            layer = layer.child(
                round("feedback-viewer-prev", &mut self.icons, Icon::ArrowLeft)
                    .top(px(middle))
                    .left(px(ARROW_INSET))
                    .on_click(cx.listener(|this, _, _, cx| this.feedback_viewer_step(false, cx))),
            );
        }
        if at + 1 < count {
            layer = layer.child(
                round("feedback-viewer-next", &mut self.icons, Icon::ArrowRight)
                    .top(px(middle))
                    .right(px(ARROW_INSET))
                    .on_click(cx.listener(|this, _, _, cx| this.feedback_viewer_step(true, cx))),
            );
        }
        Some(
            layer
                .with_animation(
                    (
                        "feedback-viewer-in",
                        self.feedback.viewer.unwrap_or_default(),
                    ),
                    gpui::Animation::new(std::time::Duration::from_millis(160))
                        .with_easing(gpui::ease_out_quint()),
                    |layer, delta| layer.opacity(delta),
                )
                .into_any_element(),
        )
    }

    /// A send that ended while the person was elsewhere, said where they are
    /// now (founder: 「反馈成功或失败都要有提示吧」): filed — the thank-you's
    /// title and "View on GitHub"; not filed — the fallback's title and "Open
    /// GitHub form". Gone by itself, on ✕, on its action, or once the
    /// Feedback page — which shows the whole outcome — is back on screen.
    pub(super) fn feedback_toast(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.feedback_on_screen() {
            self.feedback.toast = None;
        }
        let toast = self.feedback.toast.clone()?;
        let s = &self.settings;
        let (title, action) = if toast.filed {
            (s.bug_success_title.clone(), s.bug_view_issue.clone())
        } else {
            (s.bug_fallback_title.clone(), s.bug_open_github.clone())
        };
        let (disc, ink, icon) = if toast.filed {
            (theme.success_soft, theme.success_base, Icon::Check)
        } else {
            (theme.warning_soft, theme.warning_base, Icon::TriangleAlert)
        };
        let top = theme::WALLET_TOAST_TOP + if owns_titlebar(window) { CAPTION_H } else { 0. };
        let sunken = theme.bg_sunken;
        let url = toast.url.clone();
        let pill = div()
            .id("feedback-toast")
            .occlude()
            .flex()
            .items_center()
            .gap(px(12.))
            .pl(px(10.))
            .pr(px(8.))
            .py(px(8.))
            .rounded_full()
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.divider)
            .shadow_lg()
            .child(
                div()
                    .size(px(28.))
                    .flex_none()
                    .rounded_full()
                    .bg(disc)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(&mut self.icons, icon, false, ink, 16.)),
            )
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(title),
            )
            .child(
                div()
                    .id("feedback-toast-action")
                    .px(px(12.))
                    .py(px(6.))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(move |el| el.bg(sunken))
                    .active(|el| el.opacity(0.7))
                    .text_size(theme::text_row_sub())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.info_base)
                    .child(action)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        crate::executor::opener::open(&url, cx);
                        this.feedback.toast = None;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("feedback-toast-close")
                    .size(px(28.))
                    .rounded_full()
                    .cursor_pointer()
                    .hover(move |el| el.bg(sunken))
                    .active(|el| el.opacity(0.7))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(
                        &mut self.icons,
                        Icon::X,
                        false,
                        theme.fg_muted,
                        14.,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.feedback.toast = None;
                        cx.notify();
                    })),
            );
        Some(
            div()
                .absolute()
                .top(px(top))
                .left_0()
                .right_0()
                .flex()
                .justify_center()
                .child(
                    pill.with_animation(
                        ("feedback-toast-in", self.feedback.toast_generation),
                        gpui::Animation::new(std::time::Duration::from_millis(320))
                            .with_easing(gpui::ease_out_quint()),
                        |pill, delta| pill.opacity(delta).top(px((delta - 1.) * 12.)),
                    ),
                )
                .into_any_element(),
        )
    }
}

/// The report the core builds for Unichain's empty relay treasury (issue
/// 466), word for word — what the `relay` pins open the form on. A sample
/// for a screenshot pass, which has no relay to stop it.
fn relay_report_sample() -> (String, String, bug_report::ReportOrigin) {
    (
        "Relayer out of gas on Unichain (130)\n\n\
         Treasury: 0x3e59292e18417f814112f731e7163534c6d2fe3c\n\
         Has 0 ETH of its 0.0001 ETH floor (short 0.0001 ETH)."
            .to_owned(),
        "1. Send on Unichain (130)\n\
         2. Continue: the relay's treasury check stopped the send — its relayer is out of gas"
            .to_owned(),
        bug_report::ReportOrigin {
            area: vela_core::app::send::RELAY_REPORT_AREA.to_owned(),
            fingerprint: "relay-gas-130".to_owned(),
        },
    )
}

/// Read, decode, turn, scale, encode, and draw the tile — all off the
/// frame. `None` for anything that is not an image this machine can open.
fn prepare_shot(source: ShotSource) -> Option<ReadyShot> {
    let bytes = match source {
        ShotSource::Path(path) => std::fs::read(path).ok()?,
        ShotSource::Bytes(bytes) => bytes,
    };
    let prepared = screenshot_prep::prepare(&bytes)?;
    let pictures = screenshot_prep::pictures(&prepared.jpeg, screenshot_prep::THUMB_PX)?;
    Some(ReadyShot {
        base64: Arc::from(screenshot_prep::to_base64(&prepared.jpeg)),
        thumb: pictures.thumb,
        full: pictures.full,
        width: prepared.width,
        height: prepared.height,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fallen() -> Report<&'static str> {
        Report {
            what: "Send froze".into(),
            result: Some(BugReportOutcome::Fallback {
                fallback_url: "https://github.com/form?title=Send+froze".into(),
                with_screenshots: false,
            }),
            ..Report::default()
        }
    }

    fn is_fallback(report: &Report<&str>) -> bool {
        matches!(report.result, Some(BugReportOutcome::Fallback { .. }))
    }

    /// Any change after a fallback — a word, the steps, a tile added or
    /// removed — clears it, and Send is the button again: the old form URL
    /// and its screenshots line described a report that no longer exists.
    #[test]
    fn a_change_after_a_fallback_clears_it() {
        let mut report = fallen();
        report.set_what("Send froze".into());
        assert!(is_fallback(&report), "the same words are not a change");
        report.set_what("Send froze on Base".into());
        assert!(!is_fallback(&report));

        let mut report = fallen();
        report.set_steps("1. open".into());
        assert!(!is_fallback(&report));

        // A text-only fallback, then a screenshot: the "can't be carried
        // over" line must not be missing, so the block goes.
        let mut report = fallen();
        let ids = report.add(1, 0);
        assert_eq!(ids.len(), 1);
        assert!(!is_fallback(&report));

        let mut report = fallen();
        let id = report.tray.add(1, 0)[0];
        report.remove(id);
        assert!(!is_fallback(&report));

        // A file refused is a change the person made too.
        let mut report = fallen();
        report.add(0, 1);
        assert!(!is_fallback(&report));
    }

    /// A filed report is not undone by anything typed afterwards.
    #[test]
    fn a_filed_report_survives_edits() {
        let mut report: Report<&str> = Report {
            result: Some(BugReportOutcome::Filed {
                number: 1,
                url: "u".into(),
                deduped: false,
                screenshots_dropped: 0,
            }),
            ..Report::default()
        };
        report.set_what("more".into());
        report.add(1, 0);
        assert!(report.filed());
    }

    fn relay_origin() -> bug_report::ReportOrigin {
        bug_report::ReportOrigin {
            area: "Send".into(),
            fingerprint: "relay-gas-130".into(),
        }
    }

    /// Issue 466: a relay stop's report replaces whatever the form held —
    /// words, tiles, an old outcome — and files under the core's area and
    /// key, through edits of its words; the person's own report files as
    /// "Other" under a fingerprint of their words.
    #[test]
    fn a_seeded_report_files_under_the_cores_area_and_key() {
        let facts = bug_report::DeviceFacts {
            version: "0.9.7".into(),
            ..bug_report::DeviceFacts::default()
        };
        let labels = bug_report::EnvironmentLabels::default();

        let mut report = fallen();
        report.add(2, 0);
        report.seed(
            "Relayer out of gas on Unichain (130)\n\nTreasury: 0x3e59".into(),
            "1. Send on Unichain (130)".into(),
            relay_origin(),
        );
        assert!(report.result.is_none(), "the old outcome went");
        assert_eq!(report.tray.len(), 0, "the old tiles went");
        let ids = report.add(1, 0);
        assert_eq!(ids.len(), 1);
        assert!(ids[0] >= 2, "a fresh tile never reuses an old one's id");

        let payload = report.payload(&labels, &facts, Vec::new());
        assert_eq!(payload.area, "Send");
        assert_eq!(payload.fingerprint, "relay-gas-130");
        assert_eq!(payload.steps, "1. Send on Unichain (130)");
        assert!(
            payload
                .what
                .starts_with("Relayer out of gas on Unichain (130)")
        );

        // A word added: still the outage, still its key.
        report.set_what(format!("{}\nSeen twice today.", report.what));
        assert_eq!(
            report.payload(&labels, &facts, Vec::new()).fingerprint,
            "relay-gas-130"
        );

        // Every word gone and new ones typed: the person's own report now.
        report.set_what(String::new());
        report.set_what("The fee looked wrong".into());
        let own = report.payload(&labels, &facts, Vec::new());
        assert_eq!(own.area, bug_report::AREA_OTHER);
        assert_eq!(
            own.fingerprint,
            bug_report::fingerprint_of("The fee looked wrong", bug_report::AREA_OTHER, "0.9.7")
        );

        // And a report never seeded is the person's from the start.
        let plain: Report<&str> = Report {
            what: "Send froze".into(),
            ..Report::default()
        };
        assert_eq!(
            plain.payload(&labels, &facts, Vec::new()).area,
            bug_report::AREA_OTHER
        );
    }
}
