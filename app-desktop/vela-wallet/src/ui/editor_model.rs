//! The editing model under every text well: the text, a caret and a
//! selection, the IME's composing ("marked") range, and the column a run of
//! ↑/↓ keeps. Pure — no gpui — so every rule here is a unit test.
//!
//! It replaces the wells' old design, which had no caret at all: typing
//! appended, the arrow keys were swallowed, a click only focused, and a
//! selection was all or nothing. Offsets are UTF-8 byte offsets on grapheme
//! boundaries; the IME speaks UTF-16, and the conversions are here too.
//!
//! What the model cannot know is how the text is laid out, so the moves that
//! depend on it (↑/↓, line start/end) take a [`Lines`] — the drawn element's
//! visual lines, soft wraps included, or a fake monospace grid in the tests.

use std::ops::Range;

use gpui::Keystroke;
use unicode_segmentation::UnicodeSegmentation as _;

/// The laid-out text, as visual lines.
pub trait Lines {
    /// How many visual lines (at least one, even for empty text).
    fn count(&self) -> usize;
    /// A visual line's byte range. `end` stops before a hard line break; a
    /// soft-wrapped line's `end` is the next line's `start`.
    fn range(&self, line: usize) -> Range<usize>;
    /// The x of `offset` within `line`, from that line's left edge.
    fn x_of(&self, line: usize, offset: usize) -> f32;
    /// The offset in `line` closest to `x`, within that line's range.
    fn offset_at(&self, line: usize, x: f32) -> usize;
}

/// The visual line a caret at `offset` is drawn on. At a soft wrap the same
/// offset ends one line and starts the next; the caret belongs to the next
/// one, where typing would put the character.
pub fn line_of(lines: &dyn Lines, offset: usize) -> usize {
    let count = lines.count();
    for line in 0..count {
        let range = lines.range(line);
        if offset < range.end {
            return line;
        }
        if offset == range.end {
            let soft_wrap = line + 1 < count && lines.range(line + 1).start == offset;
            if !soft_wrap {
                return line;
            }
        }
    }
    count.saturating_sub(1)
}

/// The whole text as one line — a single-line well, whose ↑/↓ and line
/// ends are the text's ends and need no layout.
pub struct OneLine(pub usize);

impl Lines for OneLine {
    fn count(&self) -> usize {
        1
    }
    fn range(&self, _: usize) -> Range<usize> {
        0..self.0
    }
    fn x_of(&self, _: usize, offset: usize) -> f32 {
        offset as f32
    }
    fn offset_at(&self, _: usize, x: f32) -> usize {
        // Never used for layout; a caret column in characters.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let at = x.max(0.) as usize;
        at.min(self.0)
    }
}

/// What a key did, for the element to finish: the model has already moved
/// or edited; clipboard work needs the app.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyResult {
    /// Not an editing key — it goes on to the page (Tab, Esc, a shortcut),
    /// or, for a printable key, to the platform's text input.
    Ignored,
    /// The caret or the selection moved.
    Moved,
    /// The text changed.
    Edited,
    /// Put this on the clipboard.
    Copy(String),
    /// Put this on the clipboard; it has been cut from the text.
    Cut(String),
    /// Read the clipboard and [`EditorModel::insert`] it.
    Paste,
}

/// How many undo steps a well keeps.
const HISTORY: usize = 200;

/// The text and the selection at one moment — an undo step.
#[derive(Clone, Debug, PartialEq, Eq)]
struct State {
    text: String,
    anchor: usize,
    head: usize,
}

/// What kind of edit this is, for grouping undo steps: a run of typing is
/// one step and a run of ⌫ is one step, as in every editor; anything else
/// (a paste, a cut, a word deleted, a selection typed over) is a step of its
/// own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditKind {
    Typing,
    Deleting,
    Other,
}

#[derive(Clone, Debug, Default)]
pub struct EditorModel {
    text: String,
    anchor: usize,
    head: usize,
    marked: Option<Range<usize>>,
    /// The x a run of ↑/↓ keeps across short lines; any other move or edit
    /// forgets it.
    goal_x: Option<f32>,
    /// The text and the selection before the last edit — so a caller that
    /// refuses an edit (hands the old value back) gets the old caret back
    /// instead of one thrown to the end.
    before: Option<(String, usize, usize)>,
    undo: Vec<State>,
    redo: Vec<State>,
    /// The group the last edit opened, while it is still open — a caret move
    /// closes it.
    group: Option<EditKind>,
    /// The state before the IME began composing: the whole composition,
    /// committed, is ONE undo step back to it.
    composing_from: Option<State>,
}

impl EditorModel {
    /// A model holding `text`, the caret at its end.
    #[cfg(test)]
    #[must_use]
    pub fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            anchor: text.len(),
            head: text.len(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Where the caret is: the moving end of the selection.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.head
    }

    /// The selection, ordered.
    #[must_use]
    pub fn selection(&self) -> Range<usize> {
        self.anchor.min(self.head)..self.anchor.max(self.head)
    }

    #[must_use]
    pub fn has_selection(&self) -> bool {
        self.anchor != self.head
    }

    /// The IME's composing text, if any.
    #[must_use]
    pub fn marked(&self) -> Option<Range<usize>> {
        self.marked.clone()
    }

    #[must_use]
    pub fn selected_text(&self) -> &str {
        &self.text[self.selection()]
    }

    /// The caller's value, at render. The same text: nothing moves — this is
    /// what keeps the caret where it is across a re-render. The text before
    /// the last edit: the caller refused that edit, and the caret goes back
    /// with it. Anything else was set from outside (a reset, a paste the
    /// core normalised): the caret goes to the end, as a browser puts it.
    pub fn sync(&mut self, value: &str) {
        if self.text == value {
            return;
        }
        if let Some((text, anchor, head)) = self.before.take()
            && text == value
        {
            // The refused edit's undo step goes with it.
            let restored = State { text, anchor, head };
            if self.undo.last() == Some(&restored) {
                self.undo.pop();
            }
            self.text = restored.text;
            self.anchor = restored.anchor;
            self.head = restored.head;
        } else {
            // Set from outside — a reset, a new draft: the history was
            // another text's, and undoing into it would bring that back.
            self.text = value.to_owned();
            self.anchor = self.text.len();
            self.head = self.text.len();
            self.undo.clear();
            self.redo.clear();
        }
        self.marked = None;
        self.composing_from = None;
        self.group = None;
        self.goal_x = None;
    }

    fn state(&self) -> State {
        State {
            text: self.text.clone(),
            anchor: self.anchor,
            head: self.head,
        }
    }

    fn restore(&mut self, state: State) {
        self.before = Some((self.text.clone(), self.anchor, self.head));
        self.text = state.text;
        self.anchor = state.anchor;
        self.head = state.head;
        self.marked = None;
        self.composing_from = None;
        self.group = None;
        self.goal_x = None;
    }

    /// Open an undo step for an edit of `kind`, unless it continues the
    /// group already open.
    fn record(&mut self, kind: EditKind) {
        if kind == EditKind::Other || self.group != Some(kind) {
            self.push_undo(self.state());
        }
        self.group = (kind != EditKind::Other).then_some(kind);
    }

    fn push_undo(&mut self, state: State) {
        if self.undo.len() >= HISTORY {
            self.undo.remove(0);
        }
        self.undo.push(state);
        self.redo.clear();
    }

    /// ⌘Z: back one step. `false` when there is nothing to undo — or the
    /// IME is composing, which owns the keys until it commits.
    pub fn undo(&mut self) -> bool {
        if self.marked.is_some() {
            return false;
        }
        let Some(previous) = self.undo.pop() else {
            return false;
        };
        self.redo.push(self.state());
        self.restore(previous);
        true
    }

    /// ⇧⌘Z: forward again.
    pub fn redo(&mut self) -> bool {
        if self.marked.is_some() {
            return false;
        }
        let Some(next) = self.redo.pop() else {
            return false;
        };
        self.undo.push(self.state());
        self.restore(next);
        true
    }

    fn clamp(&self, offset: usize) -> usize {
        let mut at = offset.min(self.text.len());
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// Put the caret at `offset`, or — `extend` — move only the selection's
    /// moving end there.
    pub fn move_to(&mut self, offset: usize, extend: bool) {
        let at = self.clamp(offset);
        self.head = at;
        if !extend {
            self.anchor = at;
        }
        self.group = None;
        self.goal_x = None;
    }

    /// The grapheme boundary before `offset`.
    #[must_use]
    pub fn prev_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .rev()
            .find_map(|(at, _)| (at < offset).then_some(at))
            .unwrap_or(0)
    }

    /// The grapheme boundary after `offset`.
    #[must_use]
    pub fn next_grapheme(&self, offset: usize) -> usize {
        self.text
            .grapheme_indices(true)
            .find_map(|(at, _)| (at > offset).then_some(at))
            .unwrap_or(self.text.len())
    }

    /// The start of the word at or before `offset` (UAX #29 words; spaces
    /// and punctuation are skipped over, as ⌥← does).
    #[must_use]
    pub fn prev_word(&self, offset: usize) -> usize {
        let mut target = 0;
        for (start, segment) in self.text.split_word_bound_indices() {
            if start >= offset {
                break;
            }
            if segment.chars().any(char::is_alphanumeric) {
                target = start;
            }
        }
        target
    }

    /// The end of the word at or after `offset`.
    #[must_use]
    pub fn next_word(&self, offset: usize) -> usize {
        for (start, segment) in self.text.split_word_bound_indices() {
            let end = start + segment.len();
            if end > offset && segment.chars().any(char::is_alphanumeric) {
                return end;
            }
        }
        self.text.len()
    }

    /// ←: a selection collapses to its start; otherwise one grapheme back.
    pub fn left(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().start, false);
        } else {
            self.move_to(self.prev_grapheme(self.head), extend);
        }
    }

    /// →: a selection collapses to its end; otherwise one grapheme on.
    pub fn right(&mut self, extend: bool) {
        if self.has_selection() && !extend {
            self.move_to(self.selection().end, false);
        } else {
            self.move_to(self.next_grapheme(self.head), extend);
        }
    }

    pub fn word_left(&mut self, extend: bool) {
        let from = if self.has_selection() && !extend {
            self.selection().start
        } else {
            self.head
        };
        self.move_to(self.prev_word(from), extend);
    }

    pub fn word_right(&mut self, extend: bool) {
        let from = if self.has_selection() && !extend {
            self.selection().end
        } else {
            self.head
        };
        self.move_to(self.next_word(from), extend);
    }

    /// The start of the caret's visual line (Home, ⌘←).
    pub fn line_start(&mut self, lines: &dyn Lines, extend: bool) {
        let line = line_of(lines, self.head);
        self.move_to(lines.range(line).start, extend);
    }

    /// The end of the caret's visual line (End, ⌘→). On a soft-wrapped
    /// line that is before its last character — the offset past it is the
    /// next line's start, where the caret would be drawn.
    pub fn line_end(&mut self, lines: &dyn Lines, extend: bool) {
        let line = line_of(lines, self.head);
        let range = lines.range(line);
        let soft_wrap = line + 1 < lines.count() && lines.range(line + 1).start == range.end;
        let end = if soft_wrap && range.end > range.start {
            self.prev_grapheme(range.end)
        } else {
            range.end
        };
        self.move_to(end, extend);
    }

    pub fn doc_start(&mut self, extend: bool) {
        self.move_to(0, extend);
    }

    pub fn doc_end(&mut self, extend: bool) {
        self.move_to(self.text.len(), extend);
    }

    /// ↑: the line above, at the column the run of ↑/↓ started from; from
    /// the first line, the start of the text.
    pub fn up(&mut self, lines: &dyn Lines, extend: bool) {
        self.vertical(lines, extend, false);
    }

    /// ↓: the line below; from the last line, the end of the text.
    pub fn down(&mut self, lines: &dyn Lines, extend: bool) {
        self.vertical(lines, extend, true);
    }

    fn vertical(&mut self, lines: &dyn Lines, extend: bool, down: bool) {
        let from = match (self.has_selection() && !extend, down) {
            (true, false) => self.selection().start,
            (true, true) => self.selection().end,
            (false, _) => self.head,
        };
        let line = line_of(lines, from);
        let goal = self.goal_x.unwrap_or_else(|| lines.x_of(line, from));
        let target = if down {
            if line + 1 >= lines.count() {
                self.text.len()
            } else {
                lines.offset_at(line + 1, goal)
            }
        } else if line == 0 {
            0
        } else {
            lines.offset_at(line - 1, goal)
        };
        self.move_to(target, extend);
        self.goal_x = Some(goal);
    }

    pub fn select_all(&mut self) {
        self.anchor = 0;
        self.head = self.text.len();
        self.group = None;
        self.goal_x = None;
    }

    /// A double-click: the word under `offset` (or the run of spaces).
    pub fn select_word_at(&mut self, offset: usize) {
        let at = self.clamp(offset);
        for (start, segment) in self.text.split_word_bound_indices() {
            let end = start + segment.len();
            if at < end || end == self.text.len() {
                self.anchor = start;
                self.head = end;
                self.group = None;
                self.goal_x = None;
                return;
            }
        }
        self.move_to(at, false);
    }

    /// Replace `range` with `new`, the caret after it — an undo step of
    /// `kind`.
    fn replace(&mut self, range: Range<usize>, new: &str, kind: EditKind) {
        self.record(kind);
        self.replace_raw(range, new);
    }

    /// The edit itself, outside the history (the IME's marked text).
    fn replace_raw(&mut self, range: Range<usize>, new: &str) {
        self.before = Some((self.text.clone(), self.anchor, self.head));
        let range = self.clamp(range.start)..self.clamp(range.end);
        self.text.replace_range(range.clone(), new);
        let caret = range.start + new.len();
        self.anchor = caret;
        self.head = caret;
        self.marked = None;
        self.goal_x = None;
    }

    /// Paste `new` over the selection (or the composing text): a step of
    /// its own.
    pub fn insert(&mut self, new: &str) {
        let range = self.marked.clone().unwrap_or_else(|| self.selection());
        self.replace(range, new, EditKind::Other);
    }

    /// A key that types — Enter's line break — in the typing group.
    pub fn type_text(&mut self, new: &str) {
        let range = self.selection();
        let kind = if range.is_empty() {
            EditKind::Typing
        } else {
            EditKind::Other
        };
        self.replace(range, new, kind);
    }

    /// ⌫: the selection, or one grapheme back. `false` when there was
    /// nothing to delete.
    pub fn backspace(&mut self) -> bool {
        if self.has_selection() {
            self.replace(self.selection(), "", EditKind::Other);
            return true;
        }
        if self.head == 0 {
            return false;
        }
        self.replace(
            self.prev_grapheme(self.head)..self.head,
            "",
            EditKind::Deleting,
        );
        true
    }

    /// ⌦: the selection, or one grapheme on.
    pub fn delete_forward(&mut self) -> bool {
        if self.has_selection() {
            self.replace(self.selection(), "", EditKind::Other);
            return true;
        }
        if self.head >= self.text.len() {
            return false;
        }
        self.replace(
            self.head..self.next_grapheme(self.head),
            "",
            EditKind::Deleting,
        );
        true
    }

    /// ⌥⌫ (Ctrl+⌫ elsewhere): back to the start of the word.
    pub fn delete_word_back(&mut self) -> bool {
        if self.has_selection() {
            return self.backspace();
        }
        let start = self.prev_word(self.head);
        if start == self.head {
            return false;
        }
        self.replace(start..self.head, "", EditKind::Other);
        true
    }

    /// ⌥⌦ (Ctrl+⌦ elsewhere): on to the end of the word.
    pub fn delete_word_forward(&mut self) -> bool {
        if self.has_selection() {
            return self.backspace();
        }
        let end = self.next_word(self.head);
        if end == self.head {
            return false;
        }
        self.replace(self.head..end, "", EditKind::Other);
        true
    }

    /// ⌘⌫: back to the start of the visual line.
    pub fn delete_to_line_start(&mut self, lines: &dyn Lines) -> bool {
        if self.has_selection() {
            return self.backspace();
        }
        let start = lines.range(line_of(lines, self.head)).start;
        if start == self.head {
            return self.backspace();
        }
        self.replace(start..self.head, "", EditKind::Other);
        true
    }

    // --- the IME's side: UTF-16 offsets ---------------------------------

    #[must_use]
    pub fn to_utf16(&self, offset: usize) -> usize {
        self.text[..self.clamp(offset)].encode_utf16().count()
    }

    #[must_use]
    pub fn byte_at_utf16(&self, units: usize) -> usize {
        byte_of_utf16(&self.text, units)
    }

    fn range_from_utf16(&self, range: &Range<usize>) -> Range<usize> {
        let start = self.byte_at_utf16(range.start);
        start..self.byte_at_utf16(range.end).max(start)
    }

    /// The selection in UTF-16 units, and whether it runs backwards.
    #[must_use]
    pub fn selected_utf16(&self) -> (Range<usize>, bool) {
        let selection = self.selection();
        (
            self.to_utf16(selection.start)..self.to_utf16(selection.end),
            self.head < self.anchor,
        )
    }

    #[must_use]
    pub fn marked_utf16(&self) -> Option<Range<usize>> {
        self.marked
            .as_ref()
            .map(|range| self.to_utf16(range.start)..self.to_utf16(range.end))
    }

    /// The text of a UTF-16 range, and the range it actually covers.
    #[must_use]
    pub fn text_for_utf16(&self, range: Range<usize>) -> (String, Range<usize>) {
        let bytes = self.range_from_utf16(&range);
        let actual = self.to_utf16(bytes.start)..self.to_utf16(bytes.end);
        (self.text[bytes].to_owned(), actual)
    }

    /// `insertText`: commit `new` over `range` (UTF-16), or over the
    /// composing text, or over the selection — and end the composition.
    pub fn replace_utf16(&mut self, range: Option<Range<usize>>, new: &str) {
        let committing = self.marked.is_some();
        let bytes = range
            .map(|range| self.range_from_utf16(&range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        if committing {
            // The whole composition is one step, back to before it began.
            let from = self.composing_from.take().unwrap_or_else(|| self.state());
            self.push_undo(from);
            self.group = None;
            self.replace_raw(bytes, new);
            return;
        }
        let typed = bytes.is_empty() && new.graphemes(true).count() == 1;
        let kind = if typed {
            EditKind::Typing
        } else {
            EditKind::Other
        };
        self.replace(bytes, new, kind);
    }

    /// `setMarkedText`: the composition so far replaces `range` (or the
    /// previous composition, or the selection) and is marked;
    /// `selected` is the caret inside it, relative to `new`, in UTF-16.
    pub fn replace_and_mark_utf16(
        &mut self,
        range: Option<Range<usize>>,
        new: &str,
        selected: Option<Range<usize>>,
    ) {
        let bytes = range
            .map(|range| self.range_from_utf16(&range))
            .or_else(|| self.marked.clone())
            .unwrap_or_else(|| self.selection());
        if self.marked.is_none() && self.composing_from.is_none() {
            self.composing_from = Some(self.state());
        }
        self.replace_raw(bytes.clone(), new);
        self.marked = (!new.is_empty()).then(|| bytes.start..bytes.start + new.len());
        if self.marked.is_none() {
            // The composition was shortened to nothing: nothing to undo.
            self.composing_from = None;
        }
        if let Some(selected) = selected {
            let start = bytes.start + byte_of_utf16(new, selected.start);
            let end = bytes.start + byte_of_utf16(new, selected.end);
            self.anchor = start;
            self.head = end.max(start);
        }
    }

    /// `unmarkText`: the composition stays as typed and stops composing —
    /// one undo step, like a commit.
    pub fn unmark(&mut self) {
        if self.marked.take().is_some()
            && let Some(from) = self.composing_from.take()
            && from.text != self.text
        {
            self.push_undo(from);
            self.group = None;
        }
    }
}

/// The byte offset of the `units`-th UTF-16 unit of `text` (clamped).
fn byte_of_utf16(text: &str, units: usize) -> usize {
    let mut seen = 0;
    for (at, ch) in text.char_indices() {
        if seen >= units {
            return at;
        }
        seen += ch.len_utf16();
    }
    text.len()
}

/// Undo (`Some(false)`) or redo (`Some(true)`): ⌘Z and ⇧⌘Z on a Mac;
/// Ctrl+Z, Ctrl+Y and Ctrl+Shift+Z elsewhere.
#[must_use]
pub fn history_chord(keystroke: &Keystroke) -> Option<bool> {
    let m = &keystroke.modifiers;
    if m.alt {
        return None;
    }
    let key = keystroke.key.as_str();
    if cfg!(target_os = "macos") {
        (m.platform && !m.control && key == "z").then_some(m.shift)
    } else if m.control && !m.platform {
        match key {
            "z" => Some(m.shift),
            "y" if !m.shift => Some(true),
            _ => None,
        }
    } else {
        None
    }
}

/// What a pasted or IME-committed string may put in a well: a single-line
/// well takes no control characters at all (a trailing newline is what turns
/// a good address into one the core refuses); a multi-line one keeps its
/// line breaks, a Windows `\r\n` becoming one `\n`.
#[must_use]
pub fn sanitize(text: &str, multiline: bool) -> String {
    if multiline {
        text.replace("\r\n", "\n")
            .replace('\r', "\n")
            .chars()
            .filter(|c| *c == '\n' || !c.is_control())
            .collect()
    } else {
        text.chars().filter(|c| !c.is_control()).collect()
    }
}

/// One key, as every text well reads it — the platform's own chords (⌘ and
/// ⌥ on a Mac, Ctrl elsewhere). Printable characters are NOT handled here:
/// they reach the well through the platform's text input, which is also the
/// road the IME's composition takes. While the IME is composing, every key
/// is the IME's.
pub fn apply_key(
    model: &mut EditorModel,
    keystroke: &Keystroke,
    lines: &dyn Lines,
    multiline: bool,
    mask: bool,
) -> KeyResult {
    if model.marked.is_some() {
        return KeyResult::Ignored;
    }
    let m = &keystroke.modifiers;
    let mac = cfg!(target_os = "macos");
    let shift = m.shift;
    // A word at a time: ⌥ on a Mac, Ctrl elsewhere.
    let by_word = if mac { m.alt } else { m.control };
    // The line's ends: ⌘←/⌘→ on a Mac (Home/End everywhere).
    let by_line = mac && m.platform;
    // The text's ends: ⌘↑/⌘↓ on a Mac, Ctrl+Home/End elsewhere.
    let by_doc = if mac { m.platform } else { m.control };

    if let Some(forward) = history_chord(keystroke) {
        let changed = if forward { model.redo() } else { model.undo() };
        return if changed {
            KeyResult::Edited
        } else {
            KeyResult::Moved
        };
    }

    if let Some(chord) = super::name_field::edit_chord(keystroke) {
        return match chord {
            super::name_field::EditChord::SelectAll => {
                model.select_all();
                KeyResult::Moved
            }
            // A masked well holds a PIN: it may be pasted into, never read
            // back out.
            super::name_field::EditChord::Copy => {
                if mask || !model.has_selection() {
                    KeyResult::Ignored
                } else {
                    KeyResult::Copy(model.selected_text().to_owned())
                }
            }
            super::name_field::EditChord::Cut => {
                if mask || !model.has_selection() {
                    KeyResult::Ignored
                } else {
                    let cut = model.selected_text().to_owned();
                    model.backspace();
                    KeyResult::Cut(cut)
                }
            }
            super::name_field::EditChord::Paste => KeyResult::Paste,
        };
    }

    let moved = |model: &mut EditorModel, go: &dyn Fn(&mut EditorModel)| {
        go(model);
        KeyResult::Moved
    };
    match keystroke.key.as_str() {
        "left" => moved(model, &|model| {
            if by_line {
                model.line_start(lines, shift);
            } else if by_word {
                model.word_left(shift);
            } else {
                model.left(shift);
            }
        }),
        "right" => moved(model, &|model| {
            if by_line {
                model.line_end(lines, shift);
            } else if by_word {
                model.word_right(shift);
            } else {
                model.right(shift);
            }
        }),
        "up" => moved(model, &|model| {
            if (mac && m.platform) || !multiline {
                model.doc_start(shift);
            } else {
                model.up(lines, shift);
            }
        }),
        "down" => moved(model, &|model| {
            if (mac && m.platform) || !multiline {
                model.doc_end(shift);
            } else {
                model.down(lines, shift);
            }
        }),
        "home" => moved(model, &|model| {
            if by_doc {
                model.doc_start(shift);
            } else {
                model.line_start(lines, shift);
            }
        }),
        "end" => moved(model, &|model| {
            if by_doc {
                model.doc_end(shift);
            } else {
                model.line_end(lines, shift);
            }
        }),
        "pageup" => moved(model, &|model| model.doc_start(shift)),
        "pagedown" => moved(model, &|model| model.doc_end(shift)),
        "backspace" => {
            let changed = if mac && m.platform {
                model.delete_to_line_start(lines)
            } else if by_word {
                model.delete_word_back()
            } else {
                model.backspace()
            };
            if changed {
                KeyResult::Edited
            } else {
                KeyResult::Moved
            }
        }
        "delete" => {
            let changed = if by_word {
                model.delete_word_forward()
            } else {
                model.delete_forward()
            };
            if changed {
                KeyResult::Edited
            } else {
                KeyResult::Moved
            }
        }
        "enter" if multiline && !m.platform && !m.control && !m.alt => {
            model.type_text("\n");
            KeyResult::Edited
        }
        _ => KeyResult::Ignored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A monospace grid: every character one unit wide, `width` characters
    /// to a visual line, a hard break at each `\n` — enough layout to test
    /// the caret's maths against.
    struct Grid {
        lines: Vec<Range<usize>>,
        text: String,
    }

    impl Grid {
        fn new(text: &str, width: usize) -> Self {
            let mut lines = Vec::new();
            let mut start = 0;
            for hard in text.split('\n') {
                let chars: Vec<(usize, char)> = hard.char_indices().collect();
                if chars.is_empty() {
                    lines.push(start..start);
                }
                for chunk in chars.chunks(width) {
                    let from = start + chunk[0].0;
                    let last = chunk[chunk.len() - 1];
                    lines.push(from..start + last.0 + last.1.len_utf8());
                }
                start += hard.len() + 1;
            }
            Self {
                lines,
                text: text.to_owned(),
            }
        }
    }

    impl Lines for Grid {
        fn count(&self) -> usize {
            self.lines.len()
        }
        fn range(&self, line: usize) -> Range<usize> {
            self.lines[line].clone()
        }
        fn x_of(&self, line: usize, offset: usize) -> f32 {
            self.text[self.lines[line].start..offset].chars().count() as f32
        }
        fn offset_at(&self, line: usize, x: f32) -> usize {
            let range = self.lines[line].clone();
            let chars: Vec<usize> = self.text[range.clone()]
                .char_indices()
                .map(|(byte, _)| byte)
                .collect();
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let column = x.round().max(0.) as usize;
            if let Some(byte) = chars.get(column) {
                return range.start + byte;
            }
            // Past the end: a soft-wrapped line stops before its last
            // character, as the drawn layout does.
            let soft_wrap = line + 1 < self.lines.len() && self.lines[line + 1].start == range.end;
            match chars.last() {
                Some(last) if soft_wrap => range.start + last,
                _ => range.end,
            }
        }
    }

    fn key(spec: &str) -> Keystroke {
        Keystroke::parse(spec).unwrap_or_else(|error| unreachable!("{spec}: {error}"))
    }

    /// Typing goes through the text-input road, as it does in the app.
    fn typed(model: &mut EditorModel, text: &str) {
        for ch in text.chars() {
            model.replace_utf16(None, &ch.to_string());
        }
    }

    fn press(model: &mut EditorModel, spec: &str, lines: &dyn Lines, multiline: bool) -> KeyResult {
        apply_key(model, &key(spec), lines, multiline, false)
    }

    fn one(model: &EditorModel) -> OneLine {
        OneLine(model.text().len())
    }

    /// The founder's first finding: the caret could not move at all.
    #[test]
    fn two_lefts_then_a_letter_goes_before_the_b() {
        let mut model = EditorModel::default();
        typed(&mut model, "abc");
        for _ in 0..2 {
            let lines = one(&model);
            assert_eq!(press(&mut model, "left", &lines, false), KeyResult::Moved);
        }
        typed(&mut model, "X");
        assert_eq!(model.text(), "aXbc");
        let lines = one(&model);
        press(&mut model, "right", &lines, false);
        typed(&mut model, "Y");
        assert_eq!(model.text(), "aXbYc");
    }

    /// A click at the x of "b" puts the caret before it.
    #[test]
    fn a_click_at_b_types_before_b() {
        let mut model = EditorModel::new("abc");
        let grid = Grid::new(model.text(), 80);
        let at = grid.offset_at(0, 1.);
        model.move_to(at, false);
        typed(&mut model, "X");
        assert_eq!(model.text(), "aXbc");
    }

    /// ↑/↓ cross soft-wrapped lines and keep the column across a short one.
    #[test]
    fn up_and_down_keep_the_column_across_wrapped_lines() {
        // "abcdefgh" wraps at 4 → "abcd" | "efgh"; then "xy" (short);
        // then "0123456789" wraps → "0123" | "4567" | "89".
        let text = "abcdefgh\nxy\n0123456789";
        let grid = Grid::new(text, 4);
        assert_eq!(grid.count(), 6);
        let mut model = EditorModel::new(text);
        model.move_to(3, false); // before "d", column 3
        press(&mut model, "down", &grid, true);
        assert_eq!(model.cursor(), 7, "before \"h\" on the wrapped half");
        press(&mut model, "down", &grid, true);
        assert_eq!(
            &text[model.cursor()..],
            "\n0123456789",
            "the end of the short line"
        );
        press(&mut model, "down", &grid, true);
        assert_eq!(&text[model.cursor()..], "3456789", "back to column 3");
        press(&mut model, "up", &grid, true);
        press(&mut model, "up", &grid, true);
        assert_eq!(model.cursor(), 7, "and back up the same way");
        press(&mut model, "up", &grid, true);
        press(&mut model, "up", &grid, true);
        assert_eq!(model.cursor(), 0, "↑ on the first line goes to the start");
        model.move_to(text.len() - 1, false);
        press(&mut model, "down", &grid, true);
        assert_eq!(
            model.cursor(),
            text.len(),
            "↓ on the last line goes to the end"
        );
    }

    /// A click on line 2 of 3 lands in line 2, at the column clicked.
    #[test]
    fn a_click_on_the_second_of_three_lines() {
        let text = "first line\nsecond\nthird one";
        let grid = Grid::new(text, 80);
        let mut model = EditorModel::new(text);
        let at = grid.offset_at(1, 3.);
        model.move_to(at, false);
        typed(&mut model, "_");
        assert_eq!(model.text(), "first line\nsec_ond\nthird one");
        assert_eq!(line_of(&grid, model.cursor()), 1);
    }

    /// The caret on line 1 of three, then typing — the founder's multi-line
    /// finding, at the model.
    #[test]
    fn the_caret_goes_back_to_the_first_line() {
        let text = "one\ntwo\nthree";
        let grid = Grid::new(text, 80);
        let mut model = EditorModel::new(text);
        press(&mut model, "up", &grid, true);
        press(&mut model, "up", &grid, true);
        assert_eq!(line_of(&grid, model.cursor()), 0);
        typed(&mut model, "!");
        assert_eq!(model.text(), "one!\ntwo\nthree");
    }

    /// At a soft wrap the caret is drawn at the start of the next line, and
    /// End on a wrapped line stays on it.
    #[test]
    fn a_soft_wrap_belongs_to_the_next_line() {
        let grid = Grid::new("abcdefgh", 4);
        assert_eq!(line_of(&grid, 4), 1);
        assert_eq!(line_of(&grid, 3), 0);
        let mut model = EditorModel::new("abcdefgh");
        model.move_to(1, false);
        let lines: &dyn Lines = &grid;
        model.line_end(lines, false);
        assert_eq!(model.cursor(), 3);
        model.move_to(5, false);
        model.line_start(lines, false);
        assert_eq!(model.cursor(), 4);
        model.line_end(lines, false);
        assert_eq!(model.cursor(), 8, "the last line ends at the text's end");
    }

    /// Home/End and the platform's line and text chords.
    #[test]
    fn home_end_and_the_line_and_text_chords() {
        let text = "one\ntwo\nthree";
        let grid = Grid::new(text, 80);
        let mut model = EditorModel::new(text);
        model.move_to(5, false); // "t|wo"
        press(&mut model, "home", &grid, true);
        assert_eq!(model.cursor(), 4);
        press(&mut model, "end", &grid, true);
        assert_eq!(model.cursor(), 7);
        let (line_start, doc_start, doc_end) = if cfg!(target_os = "macos") {
            ("cmd-left", "cmd-up", "cmd-down")
        } else {
            ("home", "ctrl-home", "ctrl-end")
        };
        press(&mut model, line_start, &grid, true);
        assert_eq!(model.cursor(), 4);
        press(&mut model, doc_end, &grid, true);
        assert_eq!(model.cursor(), text.len());
        press(&mut model, doc_start, &grid, true);
        assert_eq!(model.cursor(), 0);
    }

    /// Shift extends from where the caret was; a plain arrow collapses.
    #[test]
    fn shift_extends_and_a_plain_arrow_collapses() {
        let text = "one\ntwo";
        let grid = Grid::new(text, 80);
        let mut model = EditorModel::new(text);
        model.move_to(1, false);
        press(&mut model, "shift-right", &grid, true);
        press(&mut model, "shift-down", &grid, true);
        assert_eq!(model.selection(), 1..6);
        assert_eq!(model.selected_text(), "ne\ntw");
        press(&mut model, "left", &grid, true);
        assert_eq!(model.selection(), 1..1);
        let (line_end, _) = if cfg!(target_os = "macos") {
            ("shift-cmd-right", ())
        } else {
            ("shift-end", ())
        };
        press(&mut model, line_end, &grid, true);
        assert_eq!(model.selected_text(), "ne");
        typed(&mut model, "!");
        assert_eq!(model.text(), "o!\ntwo");
    }

    /// A grapheme cluster is one step and one ⌫.
    #[test]
    fn graphemes_are_one_step() {
        // e + combining acute, a flag, a family emoji with joiners.
        let text = "e\u{301}🇨🇳👨‍👩‍👧x";
        let mut model = EditorModel::new(text);
        let lines = one(&model);
        press(&mut model, "left", &lines, false);
        assert_eq!(&model.text()[model.cursor()..], "x");
        press(&mut model, "left", &lines, false);
        assert_eq!(&model.text()[model.cursor()..], "👨‍👩‍👧x");
        press(&mut model, "left", &lines, false);
        press(&mut model, "left", &lines, false);
        assert_eq!(model.cursor(), 0, "the é is one step too");
        model.move_to(text.len() - 1, false);
        assert!(model.backspace());
        assert_eq!(model.text(), "e\u{301}🇨🇳x");
    }

    /// A word at a time, with the platform's modifier.
    #[test]
    fn a_word_at_a_time() {
        let word = if cfg!(target_os = "macos") {
            "alt"
        } else {
            "ctrl"
        };
        let mut model = EditorModel::new("send to 0xabc, now");
        let lines = one(&model);
        press(&mut model, &format!("{word}-left"), &lines, false);
        assert_eq!(&model.text()[model.cursor()..], "now");
        press(&mut model, &format!("{word}-left"), &lines, false);
        assert_eq!(&model.text()[model.cursor()..], "0xabc, now");
        press(&mut model, &format!("{word}-right"), &lines, false);
        assert_eq!(&model.text()[model.cursor()..], ", now");
        let back = format!("{word}-backspace");
        press(&mut model, &back, &lines, false);
        assert_eq!(model.text(), "send to , now");
    }

    /// The IME's composition: "ni" is marked where the caret is — the middle
    /// of a line — then committed as "你" in its place.
    #[test]
    fn a_pinyin_composition_lands_at_the_caret() {
        let mut model = EditorModel::new("ab\ncd");
        model.move_to(1, false); // a|b
        model.replace_and_mark_utf16(None, "n", Some(1..1));
        model.replace_and_mark_utf16(None, "ni", Some(2..2));
        assert_eq!(model.text(), "anib\ncd");
        assert_eq!(model.marked(), Some(1..3));
        assert_eq!(model.marked_utf16(), Some(1..3));
        // Keys belong to the IME while it composes.
        let lines = one(&model);
        assert_eq!(press(&mut model, "left", &lines, false), KeyResult::Ignored);
        assert_eq!(
            press(&mut model, "backspace", &lines, false),
            KeyResult::Ignored
        );
        model.replace_utf16(None, "你");
        assert_eq!(model.text(), "a你b\ncd");
        assert_eq!(model.marked(), None);
        assert_eq!(model.cursor(), 1 + "你".len());
        assert_eq!(model.selected_utf16(), (2..2, false));
    }

    /// ⌫ during composition is the IME shortening its marked text, which
    /// edits the composition — never the text around it.
    #[test]
    fn backspace_while_composing_edits_the_composition() {
        let mut model = EditorModel::new("xy");
        model.move_to(1, false);
        model.replace_and_mark_utf16(None, "nih", Some(3..3));
        model.replace_and_mark_utf16(None, "ni", Some(2..2));
        assert_eq!(model.text(), "xniy");
        model.replace_and_mark_utf16(None, "", None);
        assert_eq!(model.text(), "xy");
        assert_eq!(model.marked(), None);
        assert_eq!(model.cursor(), 1);
    }

    /// UTF-16 ranges over text with astral characters.
    #[test]
    fn utf16_ranges_round_trip() {
        let model = EditorModel::new("a😀你b");
        assert_eq!(model.to_utf16(model.text().len()), 5);
        assert_eq!(model.byte_at_utf16(3), 1 + 4);
        let (text, actual) = model.text_for_utf16(1..4);
        assert_eq!(text, "😀你");
        assert_eq!(actual, 1..4);
    }

    /// A refused edit (the caller hands the old value back) restores the old
    /// caret; an outside value puts it at the end; the same value keeps it.
    #[test]
    fn the_caret_survives_a_re_render() {
        let mut model = EditorModel::new("12345");
        model.move_to(2, false);
        model.sync("12345");
        assert_eq!(model.cursor(), 2, "a re-render moves nothing");
        typed(&mut model, "x");
        model.sync("12345"); // the amount field refused the letter
        assert_eq!(model.text(), "12345");
        assert_eq!(model.cursor(), 2);
        model.sync("");
        assert_eq!(model.cursor(), 0);
        model.sync("reset from outside");
        assert_eq!(model.cursor(), "reset from outside".len());
    }

    /// Enter is a line in a multi-line well and nothing in a single-line one.
    #[test]
    fn enter_is_a_line_only_where_there_are_lines() {
        let mut model = EditorModel::new("ab");
        model.move_to(1, false);
        let lines = one(&model);
        assert_eq!(
            press(&mut model, "enter", &lines, false),
            KeyResult::Ignored
        );
        assert_eq!(press(&mut model, "enter", &lines, true), KeyResult::Edited);
        assert_eq!(model.text(), "a\nb");
    }

    /// Copy and cut need a selection, and never read a masked well.
    #[test]
    fn copy_cut_and_the_mask() {
        let (all, copy, cut) = if cfg!(target_os = "macos") {
            ("cmd-a", "cmd-c", "cmd-x")
        } else {
            ("ctrl-a", "ctrl-c", "ctrl-x")
        };
        let mut model = EditorModel::new("1234");
        let lines = one(&model);
        assert_eq!(press(&mut model, copy, &lines, false), KeyResult::Ignored);
        press(&mut model, all, &lines, false);
        assert_eq!(
            apply_key(&mut model, &key(copy), &lines, false, true),
            KeyResult::Ignored
        );
        assert_eq!(
            press(&mut model, copy, &lines, false),
            KeyResult::Copy("1234".into())
        );
        assert_eq!(
            press(&mut model, cut, &lines, false),
            KeyResult::Cut("1234".into())
        );
        assert_eq!(model.text(), "");
    }

    fn undo_key() -> &'static str {
        if cfg!(target_os = "macos") {
            "cmd-z"
        } else {
            "ctrl-z"
        }
    }

    fn redo_keys() -> Vec<&'static str> {
        if cfg!(target_os = "macos") {
            vec!["cmd-shift-z"]
        } else {
            vec!["ctrl-y", "ctrl-shift-z"]
        }
    }

    /// A run of typing is one step; a caret move starts the next.
    #[test]
    fn typing_undoes_as_one_step_until_the_caret_moves() {
        let mut model = EditorModel::default();
        typed(&mut model, "hello");
        let lines = one(&model);
        press(&mut model, "left", &lines, false);
        typed(&mut model, "XY");
        assert_eq!(model.text(), "hellXYo");
        let lines = one(&model);
        assert_eq!(
            press(&mut model, undo_key(), &lines, false),
            KeyResult::Edited
        );
        assert_eq!(model.text(), "hello");
        assert_eq!(model.cursor(), 4, "the caret goes back with the text");
        press(&mut model, undo_key(), &lines, false);
        assert_eq!(model.text(), "");
        // Nothing left: still consumed, nothing changes.
        assert_eq!(
            press(&mut model, undo_key(), &lines, false),
            KeyResult::Moved
        );
        for redo in redo_keys() {
            let mut again = model.clone();
            let lines = one(&again);
            assert_eq!(
                press(&mut again, redo, &lines, false),
                KeyResult::Edited,
                "{redo}"
            );
            assert_eq!(again.text(), "hello", "{redo}");
        }
    }

    /// A run of ⌫ is one step; a paste is a step of its own; a new edit
    /// forgets what could be redone.
    #[test]
    fn deletes_group_and_pastes_stand_alone() {
        let mut model = EditorModel::new("abcdef");
        model.backspace();
        model.backspace();
        model.insert("PASTE");
        assert_eq!(model.text(), "abcdPASTE");
        assert!(model.undo());
        assert_eq!(model.text(), "abcd");
        assert!(model.undo());
        assert_eq!(model.text(), "abcdef");
        assert!(model.redo());
        assert_eq!(model.text(), "abcd");
        typed(&mut model, "z");
        assert!(!model.redo(), "a new edit clears the redo list");
    }

    /// The whole IME composition, committed, is one step back to before it
    /// began — never "n", "ni", "nih" one by one.
    #[test]
    fn an_ime_commit_is_one_undo_step() {
        let mut model = EditorModel::new("ab");
        model.move_to(1, false);
        for step in ["n", "ni", "nih", "niha", "nihao"] {
            let len = step.encode_utf16().count();
            model.replace_and_mark_utf16(None, step, Some(len..len));
        }
        model.replace_utf16(None, "你好");
        assert_eq!(model.text(), "a你好b");
        assert!(model.undo());
        assert_eq!(model.text(), "ab");
        assert_eq!(model.cursor(), 1);
        assert!(model.redo());
        assert_eq!(model.text(), "a你好b");
        // ⌘Z belongs to the IME while it composes.
        model.replace_and_mark_utf16(None, "x", Some(1..1));
        assert!(!model.undo());
    }

    /// A caller that refuses an edit takes its undo step back too.
    #[test]
    fn a_refused_edit_leaves_no_undo_step() {
        let mut model = EditorModel::new("12");
        typed(&mut model, "x");
        model.sync("12");
        assert!(!model.undo(), "nothing was really typed");
        model.sync("reset");
        assert!(!model.undo(), "an outside value starts a new history");
    }

    /// A click while composing COMMITS the composition (as native macOS
    /// fields do): the composed text stays, is one undo step, and the click
    /// lands where it was aimed.
    #[test]
    fn a_click_mid_composition_commits_it() {
        let text = "one\ntwo";
        let mut model = EditorModel::new(text);
        model.move_to(1, false);
        model.replace_and_mark_utf16(None, "hao", Some(3..3));
        assert_eq!(model.text(), "ohaone\ntwo");
        // What the editor's mouse-down does.
        model.unmark();
        assert_eq!(model.marked(), None);
        assert_eq!(model.text(), "ohaone\ntwo", "the composed text stays");
        let grid = Grid::new(model.text(), 80);
        let at = grid.offset_at(1, 1.);
        model.move_to(at, false);
        typed(&mut model, "_");
        assert_eq!(model.text(), "ohaone\nt_wo");
        assert!(model.undo(), "the typed character undoes first");
        assert!(model.undo(), "then the committed composition, as one step");
        assert_eq!(model.text(), text);
    }

    #[test]
    fn a_double_click_takes_the_word() {
        let mut model = EditorModel::new("hello big world");
        model.select_word_at(7);
        assert_eq!(model.selected_text(), "big");
        model.select_word_at(15);
        assert_eq!(model.selected_text(), "world");
    }

    #[test]
    fn sanitize_keeps_lines_only_where_there_are_lines() {
        assert_eq!(sanitize("a\r\nb\tc", true), "a\nbc");
        assert_eq!(sanitize("0xabc\n", false), "0xabc");
    }
}
