//! Where the tab on screen begins in the one engine history (spec 083 W15).
//!
//! One webview serves every tab, so every tab change is a navigation and adds
//! an entry: what is behind a tab's first page is other tabs' pages. Back was
//! the engine's alone. After "+" and a typed site it went to the tab left
//! behind. After a page's `target=_blank` it went to the page that opened the
//! new tab, which the strip then recorded as that tab: two dApp tabs, and the
//! new one gone. So a tab keeps a floor, its first page, and Back never goes
//! below it:
//!
//! - **Windows**: once the floor lands, the engine is asked to forget every
//!   other entry (`webview::forget_history_behind`). From its answer on, its
//!   own `can_go_back`/`can_go_forward` are the tab's, and a page's own
//!   `history.back()` is included. If a runtime refuses, the engine's word is
//!   followed anyway, as before 083 (logged): a Back that crosses tabs is the
//!   old fault, while a Back that never works would be a new one.
//! - **macOS** has no such call, so the tab's own entries are counted. A
//!   document landing, a `pushState` or a fragment link adds one. An arrow
//!   moves by one, and what an arrow or a reload lands is not a new entry. A
//!   page's own traversal inside its document (a router's back button)
//!   counts as a step back. This counts what the shell heard, not the
//!   engine's list, and the two can differ. A page that goes back by script
//!   to another document is read as a new page, so Back can cross again, as
//!   it always could on macOS. A page's `history.forward()`, or a reload that
//!   never commits, leaves the count short, and Back stops early. Both are
//!   rare next to what the floor fixes.
//!
//! Decided here with no gpui and no engine in it, so the tests drive every
//! turn.

/// The tab on screen's place in the engine's history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabHistory {
    /// The number of the tab change the floor belongs to. The engine's answer
    /// carries it, so an answer for a tab left since is dropped.
    floor: u64,
    /// The tab's first page has been asked for and has not landed: every entry
    /// the engine has is another tab's.
    entry_pending: bool,
    /// The tab's own entries behind the page on screen, as counted.
    back_depth: u32,
    /// Arrow steps and reloads sent to the engine whose landing (a commit, or
    /// a same-document traversal) is not a new entry.
    in_flight: u32,
    /// The engine answered the ask to forget what is behind the floor
    /// (Windows): its word on the arrows is the one drawn.
    follow_engine: bool,
}

impl Default for TabHistory {
    /// The session's first page is a tab's first page, like any other.
    fn default() -> Self {
        Self {
            floor: 0,
            entry_pending: true,
            back_depth: 0,
            in_flight: 0,
            follow_engine: false,
        }
    }
}

impl TabHistory {
    /// The tab on screen changed and its page was asked for: a tab picked or
    /// opened, the neighbour after a close, or the first page of a start-page
    /// tab.
    pub fn tab_changed(&mut self) {
        *self = Self {
            floor: self.floor + 1,
            ..Self::default()
        };
    }

    /// A document of the tab arrived: a commit, or WebView2's own error page
    /// in its place, which is an entry all the same. Returns `Some(floor)` when
    /// it is the tab's first page, the one to forget everything behind, under
    /// that number.
    pub fn landed(&mut self) -> Option<u64> {
        if self.entry_pending {
            self.entry_pending = false;
            return Some(self.floor);
        }
        if self.in_flight > 0 {
            self.in_flight -= 1;
        } else {
            self.back_depth += 1;
        }
        None
    }

    /// The page added an entry of its own with no load: a `pushState`, or a
    /// fragment link.
    pub fn pushed(&mut self) {
        // Before the tab's first page lands, it is the page being left.
        if self.entry_pending {
            return;
        }
        self.back_depth += 1;
        // A page adds entries only once it is settled, so any step still
        // expected has landed unheard. Otherwise it would swallow the next
        // real page, and Back would stop one page short of the floor.
        self.in_flight = 0;
    }

    /// The page moved through its own entries with no load (a `popstate` that
    /// did not grow the list).
    pub fn popped(&mut self) {
        if self.entry_pending {
            return;
        }
        if self.in_flight > 0 {
            // An arrow's step, landed in the same document.
            self.in_flight -= 1;
        } else {
            // The page's own `history.back()`, which is what a router's back
            // button calls. The rarer `history.forward()` is read the same
            // way, and leaves the count short by two.
            self.back_depth = self.back_depth.saturating_sub(1);
        }
    }

    /// The engine answered the ask to forget every entry behind the floor
    /// numbered `floor`, whether it did or not.
    pub fn engine_answered(&mut self, floor: u64) {
        if floor == self.floor && !self.entry_pending {
            self.follow_engine = true;
        }
    }

    /// Whether Back may act: never before the tab's first page is there, and
    /// never below it.
    #[must_use]
    pub fn may_go_back(&self) -> bool {
        !self.entry_pending && (self.follow_engine || self.back_depth > 0)
    }

    /// Whether Forward may act. What is ahead of a tab whose first page has
    /// not landed is where another tab went back from. Once it has landed,
    /// the new page cut away every forward entry, so what is ahead is the
    /// tab's own.
    #[must_use]
    pub fn may_go_forward(&self) -> bool {
        !self.entry_pending
    }

    /// The engine was sent one step, back (towards the floor) or forward.
    pub fn stepped(&mut self, back: bool) {
        self.in_flight += 1;
        if back {
            self.back_depth = self.back_depth.saturating_sub(1);
        } else {
            self.back_depth += 1;
        }
    }

    /// The page on screen is being reloaded, so its landing is not a new
    /// entry.
    pub fn reloading(&mut self) {
        if !self.entry_pending {
            self.in_flight += 1;
        }
    }

    /// The arrows as drawn: the engine's word, within the tab.
    #[must_use]
    pub fn arrows(&self, engine: [bool; 2]) -> [bool; 2] {
        [
            engine[0] && self.may_go_back(),
            engine[1] && self.may_go_forward(),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The engine as it is after another tab: it can go back, into that tab.
    const OTHER_TAB_BEHIND: [bool; 2] = [true, false];

    /// A tab whose first page has landed, counted (no engine answer yet).
    fn landed_tab() -> TabHistory {
        let mut tab = TabHistory::default();
        tab.tab_changed();
        assert_eq!(tab.landed(), Some(1), "the tab's first page is the floor");
        tab
    }

    /// The review's scenario A: "+", then a typed site. The engine can go
    /// back, to the tab left behind, and Back must not.
    #[test]
    fn a_new_tab_has_nothing_to_go_back_to() {
        let mut tab = TabHistory::default();
        tab.tab_changed();
        assert_eq!(
            tab.arrows(OTHER_TAB_BEHIND),
            [false, false],
            "still loading"
        );
        assert!(!tab.may_go_back() && !tab.may_go_forward());
        assert_eq!(tab.landed(), Some(1));
        assert_eq!(tab.arrows(OTHER_TAB_BEHIND), [false, false], "landed");
        assert!(!tab.may_go_back(), "a click does nothing either");
        // A stale forward entry of another tab is not the tab's.
        let mut pending = TabHistory::default();
        pending.tab_changed();
        assert_eq!(pending.arrows([true, true]), [false, false]);
    }

    /// The review's scenario B: a page's `target=_blank` opens a tab. Back
    /// there must not return to the page that opened it, which the strip
    /// would then record as the new tab.
    #[test]
    fn a_tab_a_page_opened_does_not_go_back_to_its_opener() {
        let mut tab = landed_tab();
        // Two pages in the old tab, then the new tab.
        assert_eq!(tab.landed(), None);
        tab.tab_changed();
        assert_eq!(tab.landed(), Some(2));
        assert!(!tab.may_go_back());
        assert!(!tab.arrows(OTHER_TAB_BEHIND)[0]);
    }

    /// Within the tab, Back goes as far as the tab's own pages and stops at
    /// the first. What an arrow lands is not a new page.
    #[test]
    fn back_goes_through_the_tabs_own_pages_and_stops_at_its_first() {
        let mut tab = landed_tab();
        assert_eq!(tab.landed(), None, "a link");
        tab.pushed(); // a single-page app's route
        assert_eq!(tab.arrows([true, false]), [true, false]);
        // Back from the route lands in the same document, back from the
        // link's page lands a document.
        tab.stepped(true);
        tab.popped();
        tab.stepped(true);
        assert_eq!(tab.landed(), None);
        assert!(!tab.may_go_back(), "the floor");
        assert_eq!(tab.arrows([true, true]), [false, true]);
        assert!(tab.may_go_forward());
        tab.stepped(false);
        assert_eq!(tab.landed(), None);
        assert!(tab.may_go_back(), "forward again, so back again");
    }

    /// An arrow's step inside a single-page app lands with no commit. The
    /// next real page must still count, or Back stops one short of the tab's
    /// first page.
    #[test]
    fn a_step_inside_the_page_does_not_swallow_the_next_page() {
        let mut tab = landed_tab();
        tab.pushed();
        tab.stepped(true); // back to the floor, in the same document
        tab.popped();
        assert!(!tab.may_go_back());
        assert_eq!(tab.landed(), None, "a link to another site");
        assert!(tab.may_go_back(), "back to the dApp");
        tab.stepped(true);
        assert_eq!(tab.landed(), None);
        assert!(!tab.may_go_back(), "and no further");

        // A step whose landing was never heard is settled by the page's next
        // entry of its own: [floor, route, link], and Back from the link.
        let mut unheard = landed_tab();
        unheard.pushed();
        unheard.stepped(true);
        unheard.pushed();
        assert_eq!(unheard.landed(), None);
        unheard.stepped(true);
        assert_eq!(unheard.landed(), None);
        assert!(unheard.may_go_back(), "the floor is still behind the route");
    }

    /// A router's back button is the page's own `history.back()`: a step
    /// towards the floor that no arrow sent.
    #[test]
    fn a_pages_own_back_is_a_step_back() {
        let mut tab = landed_tab();
        tab.pushed();
        tab.popped();
        assert!(!tab.may_go_back());
        // At the floor there is nothing to take away: the count never goes
        // below it, and the page's next route is one to go back from.
        tab.popped();
        tab.pushed();
        assert!(tab.may_go_back());
    }

    /// A reload's landing is the same page, not one more to go back through.
    #[test]
    fn a_reload_is_not_a_page_to_go_back_to() {
        let mut tab = landed_tab();
        tab.reloading();
        tab.landed();
        assert!(!tab.may_go_back());
        // Before the tab's first page lands, a reload counts nothing.
        let mut pending = TabHistory::default();
        pending.tab_changed();
        pending.reloading();
        assert_eq!(pending.landed(), Some(1));
        assert_eq!(pending.landed(), None);
        assert!(pending.may_go_back(), "the next page is a page");
    }

    /// Windows: once the engine answered, its own word is the tab's, a
    /// `pushState` the shell never heard included. An answer for a tab left
    /// since changes nothing.
    #[test]
    fn once_the_engine_answered_its_word_is_the_tabs() {
        let mut tab = landed_tab();
        tab.engine_answered(1);
        assert_eq!(tab.arrows([true, false]), [true, false], "the engine's");
        assert_eq!(tab.arrows([false, false]), [false, false]);
        assert!(tab.may_go_back());
        let mut left = landed_tab();
        left.tab_changed();
        left.engine_answered(1);
        assert_eq!(left.landed(), Some(2));
        left.engine_answered(1);
        assert!(!left.may_go_back(), "the old tab's answer");
        left.engine_answered(2);
        assert!(left.may_go_back());
        // An answer before the page landed is not one for it.
        let mut early = TabHistory::default();
        early.tab_changed();
        early.engine_answered(1);
        assert_eq!(early.landed(), Some(1));
        assert!(!early.may_go_back());
    }

    /// A page's own route before the tab's first page arrived is the page
    /// being left.
    #[test]
    fn a_push_from_the_page_being_left_is_not_the_tabs() {
        let mut tab = TabHistory::default();
        tab.tab_changed();
        tab.pushed();
        tab.popped();
        assert_eq!(tab.landed(), Some(1));
        assert!(!tab.may_go_back());
    }
}
