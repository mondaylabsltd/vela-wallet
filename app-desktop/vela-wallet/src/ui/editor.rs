//! The text editor inside every well — single-line and multi-line.
//!
//! ## Why this exists
//!
//! The wells were caret-less: `on_key_down` appended `key_char` to the value
//! and popped it on backspace, the arrow keys and Home/End were swallowed, a
//! click only focused, and nothing registered gpui's platform input handler —
//! so the caret sat at the end forever, and an IME (Pinyin, Kana, Hangul) had
//! nowhere to put its composition (founder, 2026-09-27: 「光标不能切换到之前的
//! 行」「四个方向键和鼠标单击都没用」「似乎还不支持中文输入」).
//!
//! Now each well is an [`Editor`] element over an [`EditorModel`]:
//!
//! - **Text arrives through the platform's text input** — the element
//!   registers an [`InputHandler`] in paint, which is the road both plain
//!   typing and the IME's marked (composing) text take on macOS and
//!   Windows. The key handler never inserts a printable character itself,
//!   so nothing is typed twice.
//! - **Keys move and edit** ([`editor_model::apply_key`]): ←/→ by grapheme,
//!   ⌥ (Ctrl) by word, ⌘←/⌘→ and Home/End to the line's ends, ↑/↓ between
//!   visual lines keeping the column, ⌘↑/⌘↓ to the text's ends; Shift
//!   extends; ⌫ ⌦ and their word/line variants; the four clipboard chords.
//! - **The mouse places the caret** — a click where it lands, Shift-click
//!   and a drag to select, a double-click for a word, a triple for all.
//!
//! ## Where the state lives
//!
//! The wells are functions of `(value, focus, on_change)`, called from
//! twenty-one places; none of them owns an entity. So each well's model,
//! its last layout and its scroll live here, keyed by its focus handle —
//! the same key the old selection flag used — and dropped once the handle
//! is. At render the caller's value is synced in: the same text keeps the
//! caret where it is, which is what makes the caret survive a re-render.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui::{
    App, AvailableSpace, Bounds, ContentMask, DispatchPhase, Element, ElementId, FocusHandle,
    FontWeight, GlobalElementId, Hsla, InputHandler, InspectorElementId, IntoElement, KeyDownEvent,
    LayoutId, LineLayout, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad,
    Pixels, Point, ShapedLine, SharedString, Style, TextAlign, TextRun, UTF16Selection,
    UnderlineStyle, WeakFocusHandle, Window, WrappedLine, fill, point, px, relative, size,
};

use super::editor_model::{self, EditorModel, KeyResult, Lines, OneLine};

/// What a well does with its new text — the caller's `on_change`, shared by
/// the key handler and the platform input handler.
pub type OnChange = Rc<dyn Fn(String, &mut Window, &mut App)>;

/// The caret's width, as the wells have always drawn it.
const CARET_W: f32 = crate::theme::FLOW_CARET_W;
/// A masked well shows one of these per character.
const BULLET: char = '•';

/// How a well's text is drawn.
#[derive(Clone)]
pub struct EditorStyle {
    pub family: SharedString,
    /// The placeholder's face — the UI face even where the text is mono (the
    /// recipient line), because a hint is words, not an address.
    pub placeholder_family: SharedString,
    pub weight: FontWeight,
    pub size: Pixels,
    pub line_height: Pixels,
    pub color: Hsla,
    pub placeholder: Hsla,
    pub caret: Hsla,
    pub selection: Hsla,
}

impl EditorStyle {
    /// A face as a field shapes it: with the UI face's features
    /// (`theme::font_ui_features` — contextual alternates OFF).
    ///
    /// A field shapes its own runs, so it does not inherit the page root's
    /// features the way drawn text does, and `gpui::font` alone leaves them
    /// at the face's defaults: Plus Jakarta Sans then turns an `x` after a
    /// digit into `×`. The add-token field's own hint read "0×…", and a
    /// contract address typed into it "0×dAC1…" (PR 3 final notes, the "0×"
    /// check — iOS's bug, alive here in the one place the roots do not
    /// reach). The mono face has no such rule to lose.
    fn face(&self, family: &SharedString) -> gpui::Font {
        gpui::Font {
            weight: self.weight,
            features: crate::theme::font_ui_features(),
            ..gpui::font(family.clone())
        }
    }

    fn font(&self) -> gpui::Font {
        self.face(&self.family)
    }

    fn placeholder_run(&self, len: usize) -> TextRun {
        TextRun {
            font: self.face(&self.placeholder_family),
            ..self.run(len, self.placeholder)
        }
    }

    fn run(&self, len: usize, color: Hsla) -> TextRun {
        TextRun {
            len,
            font: self.font(),
            color,
            background_color: None,
            underline: None,
            strikethrough: None,
        }
    }
}

/// How the text wraps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// One line that scrolls sideways to keep the caret in view.
    None,
    /// Wrapped at the well's width, `min_rows` tall at rest and growing
    /// with the text; Enter is a new line.
    Soft { min_rows: u16 },
    /// One logical line — an address — wrapped into two even halves past 22
    /// characters, as the address cards split them (a clipped address hides
    /// exactly the characters a poisoning attack changes).
    Halves,
}

impl Wrap {
    fn multiline(self) -> bool {
        matches!(self, Wrap::Soft { .. })
    }
}

// --- the registry --------------------------------------------------------

/// One laid-out visual line, in DISPLAY offsets (bullets, when masked).
#[derive(Clone)]
struct VisualLine {
    start: usize,
    end: usize,
    /// Where this line's hard line starts in the display text.
    base: usize,
    /// The unwrapped x where this visual line starts.
    start_x: Pixels,
    layout: Arc<LineLayout>,
    /// It wraps into the next line (its `end` is the next line's `start`).
    soft: bool,
}

/// The well as it was last drawn: enough to turn a point into an offset and
/// an offset into a point between frames.
#[derive(Clone)]
struct Snapshot {
    /// The window position of line 0's left edge (after any scroll).
    origin: Point<Pixels>,
    line_height: Pixels,
    lines: Vec<VisualLine>,
    display: String,
    /// The model text this was laid out for.
    real: String,
    mask: bool,
}

impl Snapshot {
    fn to_display(&self, real: usize) -> usize {
        if self.mask {
            let real = real.min(self.real.len());
            self.real[..real].chars().count() * BULLET.len_utf8()
        } else {
            real
        }
    }

    fn to_real(&self, display: usize) -> usize {
        if self.mask {
            let n = display / BULLET.len_utf8();
            self.real
                .char_indices()
                .nth(n)
                .map_or(self.real.len(), |(at, _)| at)
        } else {
            display
        }
    }

    /// The display offset under a window position, clamped into the text.
    fn hit(&self, position: Point<Pixels>) -> usize {
        let y = position.y - self.origin.y;
        let line = if y < px(0.) {
            0
        } else {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let row = (f32::from(y) / f32::from(self.line_height)) as usize;
            row.min(self.count().saturating_sub(1))
        };
        self.offset_at(line, f32::from(position.x - self.origin.x))
    }

    /// The window bounds of a display offset's caret position.
    fn point_of(&self, display: usize) -> Point<Pixels> {
        let line = editor_model::line_of(self, display);
        point(
            self.origin.x + px(self.x_of(line, display)),
            self.origin.y + self.line_height * line as f32,
        )
    }
}

impl Lines for Snapshot {
    fn count(&self) -> usize {
        self.lines.len().max(1)
    }

    fn range(&self, line: usize) -> Range<usize> {
        self.lines
            .get(line)
            .map_or(0..self.display.len(), |v| v.start..v.end)
    }

    fn x_of(&self, line: usize, offset: usize) -> f32 {
        let Some(v) = self.lines.get(line) else {
            return 0.;
        };
        let local = offset.clamp(v.start, v.end) - v.base;
        f32::from(v.layout.x_for_index(local) - v.start_x)
    }

    fn offset_at(&self, line: usize, x: f32) -> usize {
        let Some(v) = self.lines.get(line) else {
            return 0;
        };
        let local = v.layout.closest_index_for_x(px(x.max(0.)) + v.start_x);
        let mut at = (v.base + local).clamp(v.start, v.end);
        // Past a soft-wrapped line's last character is the next line's
        // start, where the caret would be drawn instead: stop before it.
        if v.soft && at == v.end && v.end > v.start {
            at = self.display[..v.end]
                .char_indices()
                .next_back()
                .map_or(v.start, |(i, _)| i);
        }
        while at > 0 && !self.display.is_char_boundary(at) {
            at -= 1;
        }
        at
    }
}

struct Slot {
    focus: WeakFocusHandle,
    model: EditorModel,
    snapshot: Option<Snapshot>,
    scroll_x: Pixels,
    dragging: bool,
    /// The last key, click or text — the caret is solid for a beat after
    /// it, then blinks.
    activity: Instant,
}

thread_local! {
    /// Every well's editor, by focus handle.
    static SLOTS: RefCell<Vec<Slot>> = const { RefCell::new(Vec::new()) };
}

/// The well's slot, made on first use. Never call back into the page from
/// inside `f` — the registry is borrowed.
fn with_slot<R>(focus: &FocusHandle, f: impl FnOnce(&mut Slot) -> R) -> R {
    SLOTS.with_borrow_mut(|slots| {
        slots.retain(|slot| slot.focus.upgrade().is_some());
        let at = if let Some(at) = slots.iter().position(|slot| slot.focus == *focus) {
            at
        } else {
            slots.push(Slot {
                focus: focus.downgrade(),
                model: EditorModel::default(),
                snapshot: None,
                scroll_x: px(0.),
                dragging: false,
                activity: Instant::now(),
            });
            slots.len() - 1
        };
        f(&mut slots[at])
    })
}

/// Half a blink: the caret is on this long, then off this long.
const BLINK: Duration = Duration::from_millis(530);

thread_local! {
    /// The focused well's last activity and the moment it was last drawn —
    /// what the blink timer reads. The timer runs while a focused well keeps
    /// being drawn and stops on its own a moment after none is.
    static BLINKING: Cell<Option<(Instant, Instant)>> = const { Cell::new(None) };
    static BLINK_TIMER: Cell<bool> = const { Cell::new(false) };
}

/// Whether the caret is in its "on" half, `since` the last activity.
fn caret_on(since: Instant) -> bool {
    (since.elapsed().as_millis() / BLINK.as_millis()).is_multiple_of(2)
}

/// Keep the focused caret blinking: one redraw at each half-blink boundary,
/// for this window, while a focused well is being drawn in it.
fn keep_blinking(activity: Instant, window: &Window, cx: &mut App) {
    BLINKING.set(Some((activity, Instant::now())));
    if BLINK_TIMER.get() {
        return;
    }
    BLINK_TIMER.set(true);
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        while let Some((activity, drawn)) = BLINKING.get() {
            if drawn.elapsed() > BLINK * 3 {
                break;
            }
            let into = activity.elapsed().as_millis() % BLINK.as_millis();
            let wait = BLINK.as_millis() - into + 5;
            cx.background_executor()
                .timer(Duration::from_millis(u64::try_from(wait).unwrap_or(530)))
                .await;
            if handle.update(cx, |_, window, _| window.refresh()).is_err() {
                break;
            }
        }
        BLINK_TIMER.set(false);
    })
    .detach();
}

/// Whether the IME is composing in this well — while it is, Enter and Esc
/// are the IME's, never the page's.
#[must_use]
pub fn is_composing(focus: &FocusHandle) -> bool {
    try_slot(focus, |slot| slot.model.marked().is_some()).unwrap_or(false)
}

/// The well's slot, only if it has one.
fn try_slot<R>(focus: &FocusHandle, f: impl FnOnce(&mut Slot) -> R) -> Option<R> {
    SLOTS.with_borrow_mut(|slots| slots.iter_mut().find(|slot| slot.focus == *focus).map(f))
}

fn current_text(focus: &FocusHandle) -> String {
    with_slot(focus, |slot| slot.model.text().to_owned())
}

/// Sync the caller's value into the well's model — at render, before the
/// element is built. The same text moves nothing.
pub fn sync(focus: &FocusHandle, value: &str) {
    with_slot(focus, |slot| slot.model.sync(value));
}

/// Select the whole text — a field entered with a value already in it (the
/// browser's address bar takes the page's URL in, selected, as a browser
/// does, so typing replaces it and a paste lands in its place).
pub fn select_all(focus: &FocusHandle) {
    with_slot(focus, |slot| slot.model.select_all());
}

// --- the handlers a well wires -------------------------------------------

/// The well's keys: moves and edits (see the module note). A printable key
/// is left alone — it arrives as text through the input handler.
pub fn key_down(
    focus: &FocusHandle,
    wrap: Wrap,
    mask: bool,
    on_change: OnChange,
) -> impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static {
    let focus = focus.clone();
    move |event: &KeyDownEvent, window, cx| {
        let multiline = wrap.multiline();
        let result = with_slot(&focus, |slot| {
            slot.activity = Instant::now();
            let one = OneLine(slot.model.text().len());
            let lines: &dyn Lines = match &slot.snapshot {
                Some(snapshot) if multiline && !mask && snapshot.real == slot.model.text() => {
                    snapshot
                }
                _ => &one,
            };
            editor_model::apply_key(&mut slot.model, &event.keystroke, lines, multiline, mask)
        });
        trace(format_args!("key {:?} -> {result:?}", event.keystroke.key));
        match result {
            KeyResult::Ignored => return,
            KeyResult::Moved => {}
            KeyResult::Edited => on_change(current_text(&focus), window, cx),
            KeyResult::Copy(text) => cx.write_to_clipboard(gpui::ClipboardItem::new_string(text)),
            KeyResult::Cut(text) => {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
                on_change(current_text(&focus), window, cx);
            }
            KeyResult::Paste => {
                let pasted = cx
                    .read_from_clipboard()
                    .and_then(|item| item.text())
                    .map(|text| editor_model::sanitize(&text, multiline))
                    .unwrap_or_default();
                if !pasted.is_empty() {
                    with_slot(&focus, |slot| slot.model.insert(&pasted));
                    on_change(current_text(&focus), window, cx);
                }
            }
        }
        window.refresh();
        cx.stop_propagation();
    }
}

/// A press anywhere on the well: focus, and the caret where it landed —
/// Shift extends, a double-click takes the word, a triple everything. A drag
/// that follows selects (see [`Editor`]'s paint).
pub fn mouse_down(
    focus: &FocusHandle,
    on_change: OnChange,
) -> impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static {
    let focus = focus.clone();
    move |event: &MouseDownEvent, window, cx| {
        focus.focus(window, cx);
        let committed = with_slot(&focus, |slot| {
            slot.activity = Instant::now();
            let offset = slot
                .snapshot
                .as_ref()
                .filter(|snapshot| snapshot.real == slot.model.text())
                .map_or(slot.model.text().len(), |snapshot| {
                    snapshot.to_real(snapshot.hit(event.position))
                });
            // A click while the IME composes COMMITS what is composed — what
            // native macOS text fields do (founder: behave like the Mac) — so
            // the text is unchanged and the click lands exactly where aimed.
            let committed = slot.model.marked().is_some();
            slot.model.unmark();
            match event.click_count {
                0 | 1 => {
                    slot.model.move_to(offset, event.modifiers.shift);
                    slot.dragging = true;
                }
                2 => {
                    slot.model.select_word_at(offset);
                    slot.dragging = false;
                }
                _ => {
                    slot.model.select_all();
                    slot.dragging = false;
                }
            }
            committed
        });
        if committed {
            on_change(current_text(&focus), window, cx);
        }
        window.refresh();
    }
}

// --- the element ---------------------------------------------------------

/// The drawn text of a well: text, selection, composition, caret.
pub struct Editor {
    focus: FocusHandle,
    placeholder: SharedString,
    style: EditorStyle,
    wrap: Wrap,
    mask: bool,
    /// As wide as its text (the send form's figure, centred beside its
    /// unit) rather than as wide as the well.
    fit: bool,
    on_change: OnChange,
}

/// A well's editor. Call [`sync`] with the caller's value first.
pub fn editor(
    focus: &FocusHandle,
    placeholder: SharedString,
    style: EditorStyle,
    wrap: Wrap,
    on_change: OnChange,
) -> Editor {
    Editor {
        focus: focus.clone(),
        placeholder,
        style,
        wrap,
        mask: false,
        fit: false,
        on_change,
    }
}

impl Editor {
    /// Bullets instead of the characters (a security key's PIN).
    #[must_use]
    pub fn masked(mut self, mask: bool) -> Self {
        self.mask = mask;
        self
    }

    /// As wide as the text, at most the room there is.
    #[must_use]
    pub fn fit_content(mut self) -> Self {
        self.fit = true;
        self
    }

    fn display(&self, real: &str) -> String {
        if self.mask {
            BULLET.to_string().repeat(real.chars().count())
        } else {
            real.to_owned()
        }
    }
}

/// The wrap width a [`Wrap`] asks for, at `width`.
fn wrap_width(
    wrap: Wrap,
    display: &str,
    style: &EditorStyle,
    width: Option<Pixels>,
    window: &Window,
) -> Option<Pixels> {
    match wrap {
        Wrap::None => None,
        Wrap::Soft { .. } => width,
        Wrap::Halves => {
            let chars = display.chars().count();
            if chars <= 22 {
                return width;
            }
            let mid = display
                .char_indices()
                .nth(chars.div_ceil(2))
                .map_or(display.len(), |(at, _)| at);
            let line = window.text_system().shape_line(
                SharedString::from(display.to_owned()),
                style.size,
                &[style.run(display.len(), style.color)],
                None,
            );
            let half = line.x_for_index(mid) + px(0.5);
            Some(width.map_or(half, |width| if half < width { half } else { width }))
        }
    }
}

fn shape(
    window: &Window,
    display: &str,
    runs: &[TextRun],
    style: &EditorStyle,
    wrap: Option<Pixels>,
) -> Vec<WrappedLine> {
    if display.is_empty() {
        return Vec::new();
    }
    window
        .text_system()
        .shape_text(
            SharedString::from(display.to_owned()),
            style.size,
            runs,
            wrap,
            None,
        )
        .map(|lines| lines.into_iter().collect())
        .unwrap_or_default()
}

fn visual_lines(shaped: &[WrappedLine]) -> Vec<VisualLine> {
    let mut lines = Vec::new();
    let mut base = 0;
    for wrapped in shaped {
        let layout = Arc::clone(&wrapped.unwrapped_layout);
        let mut starts: Vec<(usize, Pixels)> = vec![(0, px(0.))];
        for boundary in wrapped.wrap_boundaries() {
            let glyph = &layout.runs[boundary.run_ix].glyphs[boundary.glyph_ix];
            starts.push((glyph.index, glyph.position.x));
        }
        for (i, (start, start_x)) in starts.iter().enumerate() {
            let end = starts.get(i + 1).map_or(layout.len, |(next, _)| *next);
            lines.push(VisualLine {
                start: base + start,
                end: base + end,
                base,
                start_x: *start_x,
                layout: Arc::clone(&layout),
                soft: i + 1 < starts.len(),
            });
        }
        base += layout.len + 1;
    }
    if lines.is_empty() {
        lines.push(VisualLine {
            start: 0,
            end: 0,
            base: 0,
            start_x: px(0.),
            layout: Arc::new(LineLayout::default()),
            soft: false,
        });
    }
    lines
}

pub struct EditorPrepaint {
    shaped: Vec<WrappedLine>,
    placeholder: Option<ShapedLine>,
    origin: Point<Pixels>,
    selection: Vec<PaintQuad>,
    caret: Option<PaintQuad>,
    clip: Bounds<Pixels>,
}

impl IntoElement for Editor {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Editor {
    type RequestLayoutState = ();
    type PrepaintState = EditorPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let line_height = self.style.line_height;
        let real = current_text(&self.focus);
        let display = self.display(&real);
        let mut style = Style::default();
        style.min_size.width = px(0.).into();
        if self.wrap == Wrap::None {
            style.size.height = line_height.into();
            if !self.fit {
                style.size.width = relative(1.).into();
                return (window.request_layout(style, [], cx), ());
            }
            let text_style = self.style.clone();
            let empty = display.is_empty();
            let shown = if empty {
                self.placeholder.to_string()
            } else {
                display
            };
            let id = window.request_measured_layout(style, move |known, available, window, _| {
                let run = if empty {
                    text_style.placeholder_run(shown.len())
                } else {
                    text_style.run(shown.len(), text_style.color)
                };
                let line = window.text_system().shape_line(
                    SharedString::from(shown.clone()),
                    text_style.size,
                    &[run],
                    None,
                );
                let wanted = line.width + px(CARET_W * 2.);
                let width = known.width.unwrap_or(match available.width {
                    AvailableSpace::Definite(room) if room < wanted => room,
                    _ => wanted,
                });
                size(width, line_height)
            });
            return (id, ());
        }
        style.size.width = relative(1.).into();
        let text_style = self.style.clone();
        let wrap = self.wrap;
        let min_rows = match wrap {
            Wrap::Soft { min_rows } => usize::from(min_rows),
            _ => 1,
        };
        let id = window.request_measured_layout(style, move |known, available, window, _| {
            let width = known.width.or(match available.width {
                AvailableSpace::Definite(room) => Some(room),
                _ => None,
            });
            let wrap_at = wrap_width(wrap, &display, &text_style, width, window);
            let shaped = shape(
                window,
                &display,
                &[text_style.run(display.len(), text_style.color)],
                &text_style,
                wrap_at,
            );
            let rows: usize = shaped
                .iter()
                .map(|line| line.wrap_boundaries().len() + 1)
                .sum();
            size(
                width.unwrap_or(px(0.)),
                line_height * rows.max(min_rows) as f32,
            )
        });
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        let focused = self.focus.is_focused(window);
        let style = self.style.clone();
        let (real, selection, cursor, marked, activity) = with_slot(&self.focus, |slot| {
            (
                slot.model.text().to_owned(),
                slot.model.selection(),
                slot.model.cursor(),
                slot.model.marked(),
                slot.activity,
            )
        });
        let display = self.display(&real);
        let mut probe = Snapshot {
            origin: bounds.origin,
            line_height: style.line_height,
            lines: Vec::new(),
            display: display.clone(),
            real: real.clone(),
            mask: self.mask,
        };

        // The composition is underlined, as every IME's host draws it.
        let mut runs = Vec::new();
        match marked
            .as_ref()
            .map(|range| probe.to_display(range.start)..probe.to_display(range.end))
        {
            Some(range) if !range.is_empty() => {
                runs.push(style.run(range.start, style.color));
                runs.push(TextRun {
                    underline: Some(UnderlineStyle {
                        color: Some(style.color),
                        thickness: px(1.),
                        wavy: false,
                    }),
                    ..style.run(range.len(), style.color)
                });
                runs.push(style.run(display.len() - range.end, style.color));
            }
            _ => runs.push(style.run(display.len(), style.color)),
        }
        runs.retain(|run| run.len > 0);

        let wrap_at = wrap_width(self.wrap, &display, &style, Some(bounds.size.width), window);
        let shaped = shape(window, &display, &runs, &style, wrap_at);
        probe.lines = visual_lines(&shaped);

        // One line scrolls sideways to keep the caret in view, and shows its
        // start again once it is left.
        let caret_display = probe.to_display(cursor);
        let scroll = if self.wrap == Wrap::None {
            let text_width = probe.lines.first().map_or(px(0.), |line| line.layout.width);
            let caret_x = px(probe.x_of(0, caret_display));
            with_slot(&self.focus, |slot| {
                let room = bounds.size.width - px(CARET_W);
                let mut scroll = if focused { slot.scroll_x } else { px(0.) };
                if focused {
                    if caret_x < scroll {
                        scroll = caret_x;
                    } else if caret_x - scroll > room {
                        scroll = caret_x - room;
                    }
                }
                let most = text_width + px(CARET_W) - bounds.size.width;
                if scroll > most {
                    scroll = if most > px(0.) { most } else { px(0.) };
                }
                if scroll < px(0.) {
                    scroll = px(0.);
                }
                slot.scroll_x = scroll;
                scroll
            })
        } else {
            px(0.)
        };
        probe.origin = point(bounds.origin.x - scroll, bounds.origin.y);

        let line_height = style.line_height;
        let mut quads = Vec::new();
        if focused && !selection.is_empty() {
            let (from, to) = (
                probe.to_display(selection.start),
                probe.to_display(selection.end),
            );
            for (index, line) in probe.lines.iter().enumerate() {
                if to < line.start || from > line.end || (from == to) {
                    continue;
                }
                let start = from.max(line.start);
                let end = to.min(line.end);
                let x0 = px(probe.x_of(index, start));
                let mut x1 = px(probe.x_of(index, end));
                // A selection running past a hard line break shows that it
                // takes the break too.
                if to > line.end && !line.soft {
                    x1 += style.size * 0.35;
                }
                if x1 <= x0 {
                    continue;
                }
                quads.push(fill(
                    Bounds::from_corners(
                        point(
                            probe.origin.x + x0,
                            probe.origin.y + line_height * index as f32,
                        ),
                        point(
                            probe.origin.x + x1,
                            probe.origin.y + line_height * (index + 1) as f32,
                        ),
                    ),
                    style.selection,
                ));
            }
        }

        if focused {
            keep_blinking(activity, window, cx);
        }
        let blink_on = marked.is_some() || caret_on(activity);
        let caret =
            (focused && blink_on && (selection.is_empty() || marked.is_some())).then(|| {
                let at = probe.point_of(caret_display);
                let height = if style.size * 1.25 < line_height {
                    style.size * 1.25
                } else {
                    line_height
                };
                fill(
                    Bounds::new(
                        point(at.x, at.y + (line_height - height) / 2.),
                        size(px(CARET_W), height),
                    ),
                    style.caret,
                )
            });

        let placeholder = display.is_empty().then(|| {
            window.text_system().shape_line(
                self.placeholder.clone(),
                style.size,
                &[style.placeholder_run(self.placeholder.len())],
                None,
            )
        });

        let origin = probe.origin;
        with_slot(&self.focus, |slot| slot.snapshot = Some(probe));
        // A wrapped well's caret may sit on its right edge: let it show.
        let clip = if self.wrap == Wrap::None {
            bounds
        } else {
            Bounds::new(
                bounds.origin,
                size(bounds.size.width + px(CARET_W * 2.), bounds.size.height),
            )
        };
        EditorPrepaint {
            shaped,
            placeholder,
            origin,
            selection: quads,
            caret,
            clip,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        window.handle_input(
            &self.focus,
            EditorInput {
                focus: self.focus.clone(),
                on_change: Rc::clone(&self.on_change),
                multiline: self.wrap.multiline(),
            },
            cx,
        );

        // A drag after a press on the well selects, even past its edges.
        let focus = self.focus.clone();
        window.on_mouse_event(move |event: &MouseMoveEvent, phase, window, _| {
            if phase != DispatchPhase::Bubble || event.pressed_button != Some(MouseButton::Left) {
                return;
            }
            let moved = try_slot(&focus, |slot| {
                if !slot.dragging {
                    return false;
                }
                let Some(snapshot) = slot.snapshot.as_ref() else {
                    return false;
                };
                let at = snapshot.to_real(snapshot.hit(event.position));
                slot.model.move_to(at, true);
                true
            });
            if moved == Some(true) {
                window.refresh();
            }
        });
        let focus = self.focus.clone();
        window.on_mouse_event(move |_: &MouseUpEvent, phase, _, _| {
            if phase == DispatchPhase::Bubble {
                try_slot(&focus, |slot| slot.dragging = false);
            }
        });

        let line_height = self.style.line_height;
        window.with_content_mask(
            Some(ContentMask {
                bounds: prepaint.clip,
            }),
            |window| {
                for quad in prepaint.selection.drain(..) {
                    window.paint_quad(quad);
                }
                let mut y = prepaint.origin.y;
                for line in &prepaint.shaped {
                    let _ = line.paint(
                        point(prepaint.origin.x, y),
                        line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                    y += line_height * (line.wrap_boundaries().len() + 1) as f32;
                }
                if let Some(placeholder) = prepaint.placeholder.as_ref() {
                    let _ = placeholder.paint(
                        prepaint.origin,
                        line_height,
                        TextAlign::Left,
                        None,
                        window,
                        cx,
                    );
                }
                if let Some(caret) = prepaint.caret.take() {
                    window.paint_quad(caret);
                }
            },
        );
    }
}

/// `VELA_EDITOR_TRACE=1`: the platform's text-input calls on stderr — how to
/// see what an IME actually sends a well.
fn trace(what: std::fmt::Arguments<'_>) {
    thread_local! {
        static ON: bool = crate::dev_env::var_os!("VELA_EDITOR_TRACE").is_some();
    }
    if ON.with(|on| *on) {
        eprintln!("[vela-wallet] editor: {what}");
    }
}

// --- the platform's text input -------------------------------------------

/// What the platform calls with typed text and the IME's composition —
/// `insertText`, `setMarkedText`, `unmarkText`, and the queries it places
/// its candidate window with.
struct EditorInput {
    focus: FocusHandle,
    on_change: OnChange,
    multiline: bool,
}

impl EditorInput {
    fn changed(&self, window: &mut Window, cx: &mut App) {
        (self.on_change)(current_text(&self.focus), window, cx);
        window.refresh();
    }
}

impl InputHandler for EditorInput {
    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<UTF16Selection> {
        let (range, reversed) = with_slot(&self.focus, |slot| slot.model.selected_utf16());
        trace(format_args!("selectedRange -> {range:?}"));
        Some(UTF16Selection { range, reversed })
    }

    fn marked_text_range(&mut self, _window: &mut Window, _cx: &mut App) -> Option<Range<usize>> {
        let marked = with_slot(&self.focus, |slot| slot.model.marked_utf16());
        trace(format_args!("markedRange -> {marked:?}"));
        marked
    }

    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<String> {
        let (text, actual) = with_slot(&self.focus, |slot| slot.model.text_for_utf16(range_utf16));
        *adjusted_range = Some(actual);
        Some(text)
    }

    fn replace_text_in_range(
        &mut self,
        replacement_range: Option<Range<usize>>,
        text: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text = editor_model::sanitize(text, self.multiline);
        trace(format_args!(
            "insertText {text:?} over {replacement_range:?}"
        ));
        with_slot(&self.focus, |slot| {
            slot.activity = Instant::now();
            slot.model.replace_utf16(replacement_range, &text);
        });
        self.changed(window, cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let text = editor_model::sanitize(new_text, self.multiline);
        trace(format_args!(
            "setMarkedText {text:?} over {range_utf16:?} caret {new_selected_range:?}"
        ));
        with_slot(&self.focus, |slot| {
            slot.activity = Instant::now();
            slot.model
                .replace_and_mark_utf16(range_utf16, &text, new_selected_range);
        });
        self.changed(window, cx);
    }

    fn unmark_text(&mut self, window: &mut Window, _cx: &mut App) {
        trace(format_args!("unmarkText"));
        with_slot(&self.focus, |slot| slot.model.unmark());
        window.refresh();
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<Bounds<Pixels>> {
        with_slot(&self.focus, |slot| {
            let snapshot = slot.snapshot.as_ref()?;
            let start = snapshot.to_display(slot.model.byte_at_utf16(range_utf16.start));
            let end = snapshot.to_display(slot.model.byte_at_utf16(range_utf16.end));
            let from = snapshot.point_of(start);
            let to = snapshot.point_of(end);
            let width = if to.y == from.y && to.x > from.x {
                to.x - from.x
            } else {
                px(CARET_W)
            };
            Some(Bounds::new(from, size(width, snapshot.line_height)))
        })
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Option<usize> {
        with_slot(&self.focus, |slot| {
            let snapshot = slot.snapshot.as_ref()?;
            let real = snapshot.to_real(snapshot.hit(point));
            Some(slot.model.to_utf16(real))
        })
    }

    fn accepts_text_input(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }

    /// A well only ever wants text: printable keys go to the IME first, so a
    /// composition is never cut short by a key binding.
    fn prefers_ime_for_printable_keys(&mut self, _window: &mut Window, _cx: &mut App) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The "0×" check (PR 3 final notes). A field shapes its own text, so
    /// the page root's `calt` off never reached it: the add-token field's
    /// hint "0x…" was drawn "0×…", and an address typed there "0×dAC1…".
    /// Every run a field shapes — what is typed, the composing run, the
    /// placeholder — carries the UI face's features, in the UI face and the
    /// mono one alike.
    #[test]
    fn a_field_shapes_hex_as_written() {
        let calt = |font: &gpui::Font| {
            font.features
                .0
                .iter()
                .find(|(tag, _)| &**tag == "calt")
                .map(|(_, value)| *value)
        };
        for family in [crate::theme::font_ui(), crate::theme::font_mono()] {
            let style = EditorStyle {
                family: family.into(),
                placeholder_family: crate::theme::font_ui().into(),
                weight: gpui::FontWeight::NORMAL,
                size: px(15.),
                line_height: px(21.),
                color: gpui::black(),
                placeholder: gpui::black(),
                caret: gpui::black(),
                selection: gpui::black(),
            };
            let typed = style.run("0x100".len(), style.color);
            assert_eq!(calt(&typed.font), Some(0), "{family}: what is typed");
            assert_eq!(&*typed.font.family, family);
            let hint = style.placeholder_run("0x…".len());
            assert_eq!(calt(&hint.font), Some(0), "{family}: the placeholder");
            assert_eq!(&*hint.font.family, crate::theme::font_ui());
            // …and it is the face the page roots set, feature for feature.
            assert_eq!(typed.font.features, crate::theme::font_ui_features());
        }
    }

    /// A mask shows one bullet per character and maps offsets both ways.
    #[test]
    fn a_masked_offset_maps_both_ways() {
        let snapshot = Snapshot {
            origin: point(px(0.), px(0.)),
            line_height: px(20.),
            lines: Vec::new(),
            display: BULLET.to_string().repeat(4),
            real: "1234".into(),
            mask: true,
        };
        assert_eq!(snapshot.to_display(2), 2 * BULLET.len_utf8());
        assert_eq!(snapshot.to_real(2 * BULLET.len_utf8()), 2);
        assert_eq!(snapshot.to_real(99), 4);
    }

    #[test]
    fn soft_wrap_is_the_only_multiline_mode() {
        assert!(Wrap::Soft { min_rows: 3 }.multiline());
        assert!(!Wrap::None.multiline());
        assert!(!Wrap::Halves.multiline());
    }
}
