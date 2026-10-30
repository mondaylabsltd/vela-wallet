//! What a machine's answers carry of its view: the generic host's half of
//! "an unchanged view is not sent again".
//!
//! Every answer a bridge hands a shell (`dispatch`, `resolve_effect`) used to
//! carry the WHOLE view, and every shell parsed, decoded and redrew it even
//! when nothing in it had moved. On iOS that was ~3 s of main thread per
//! launch: the activity feed rebuilt its 200 rows for each of 200 name
//! answers, and the RPC pool re-sent itself at every step of every call.
//!
//! [`ViewHost`] sits between a bridge and its `Core` and decides two things:
//!
//! 1. **Whether to build the view at all.** A machine that asks for a render
//!    every time its view can change opts in with
//!    [`super::SplitEffect::RENDERS_EVERY_CHANGE`]: the host then builds the
//!    view only after an answer that rendered, and otherwise keeps the one it
//!    built last. The feed is such a machine; it holds its name answers back
//!    and renders them in batches (`activity_feed`), so 200 names are a dozen
//!    rebuilds, not 200.
//! 2. **Whether to send it.** A shell that opts in
//!    ([`ViewHost::skip_unchanged_views`]) gets `view_version` on every answer
//!    and `view` only when it differs from the last one this host sent it.
//!    No `view` means: keep the one you have. A shell that never opts in gets
//!    exactly the answers it always got.
//!
//! A machine may also name top-level view fields that move at nearly every
//! step and that few shells read ([`super::SplitEffect::VOLATILE_VIEW_KEYS`]:
//! the RPC pool's `pending_urls`, which only a hedging shell reads). For an
//! opted-in shell they leave `view`, so they no longer defeat the unchanged
//! check; a shell that reads them asks for them and gets them in `volatile`,
//! again only when they moved.
//!
//! Every knob is opt-in, per machine and per shell. Nothing here knows a
//! product.

use serde_json::{Map, Value};

/// The view half of one bridge: what was built, what was sent.
#[derive(Debug)]
pub struct ViewHost {
    renders_every_change: bool,
    volatile_keys: &'static [&'static str],
    skip_unchanged: bool,
    include_volatile: bool,
    /// The last view built: the whole view for a shell that has not opted
    /// in; without its volatile fields for one that has.
    view: Option<Value>,
    /// The volatile fields as last built (opted-in shells only).
    volatile: Option<Value>,
    /// Moves exactly when the built view differs from the one before it.
    version: u64,
    sent_version: Option<u64>,
    sent_volatile: Option<Value>,
}

/// What one answer carries of the view.
#[derive(Debug, PartialEq)]
pub struct ViewAnswer<'a> {
    /// `None`: the shell already holds this view (opted-in shells only).
    pub view: Option<&'a Value>,
    /// The view's version, for an opted-in shell.
    pub view_version: Option<u64>,
    /// The volatile fields, for a shell that asked for them, when they moved.
    pub volatile: Option<&'a Value>,
}

impl ViewHost {
    /// A host for a machine with these knobs; every shell starts on the
    /// classic answers (the whole view, every time).
    #[must_use]
    pub fn new(renders_every_change: bool, volatile_keys: &'static [&'static str]) -> Self {
        Self {
            renders_every_change,
            volatile_keys,
            skip_unchanged: false,
            include_volatile: false,
            view: None,
            volatile: None,
            version: 0,
            sent_version: None,
            sent_volatile: None,
        }
    }

    /// The shell opts in: from the next answer on, `view` only when it
    /// changed, and `view_version` always. `include_volatile`: the shell
    /// reads the machine's volatile fields and wants them, in `volatile`.
    /// The next answer carries the view whatever it is.
    pub fn skip_unchanged_views(&mut self, include_volatile: bool) {
        self.skip_unchanged = true;
        self.include_volatile = include_volatile;
        self.view = None;
        self.volatile = None;
        self.sent_version = None;
        self.sent_volatile = None;
    }

    /// Must the view be built after this answer? `rendered`: the machine
    /// asked for a render in it.
    #[must_use]
    pub fn needs_view(&self, rendered: bool) -> bool {
        self.view.is_none() || rendered || !self.renders_every_change
    }

    /// The view just built, as JSON.
    pub fn store(&mut self, mut view: Value) {
        if !self.skip_unchanged {
            // The classic answer: nothing is compared, nothing is held back.
            self.view = Some(view);
            return;
        }
        if !self.volatile_keys.is_empty() {
            let mut moved = Map::new();
            if let Value::Object(fields) = &mut view {
                for key in self.volatile_keys {
                    if let Some(value) = fields.remove(*key) {
                        moved.insert((*key).to_owned(), value);
                    }
                }
            }
            self.volatile = Some(Value::Object(moved));
        }
        if self.view.as_ref() != Some(&view) {
            self.version += 1;
            self.view = Some(view);
        }
    }

    /// What this answer carries, and the record that it was sent.
    pub fn answer(&mut self) -> ViewAnswer<'_> {
        if !self.skip_unchanged {
            return ViewAnswer {
                view: self.view.as_ref(),
                view_version: None,
                volatile: None,
            };
        }
        let send_view = self.sent_version != Some(self.version);
        self.sent_version = Some(self.version);
        let send_volatile = self.include_volatile
            && self.volatile.is_some()
            && self.sent_volatile.as_ref() != self.volatile.as_ref();
        if send_volatile {
            self.sent_volatile.clone_from(&self.volatile);
        }
        ViewAnswer {
            view: if send_view { self.view.as_ref() } else { None },
            view_version: Some(self.version),
            volatile: if send_volatile { self.volatile.as_ref() } else { None },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sent(host: &mut ViewHost, view: Value) -> (Option<Value>, Option<u64>, Option<Value>) {
        host.store(view);
        let answer = host.answer();
        (
            answer.view.cloned(),
            answer.view_version,
            answer.volatile.cloned(),
        )
    }

    #[test]
    fn a_shell_that_never_opted_in_gets_the_whole_view_every_time() {
        let mut host = ViewHost::new(false, &["pending"]);
        let view = json!({ "a": 1, "pending": { "c1": ["x"] } });
        for _ in 0..3 {
            assert_eq!(sent(&mut host, view.clone()), (Some(view.clone()), None, None));
        }
    }

    #[test]
    fn an_unchanged_view_is_sent_once() {
        let mut host = ViewHost::new(false, &[]);
        host.skip_unchanged_views(false);
        assert_eq!(sent(&mut host, json!({ "a": 1 })), (Some(json!({ "a": 1 })), Some(1), None));
        assert_eq!(sent(&mut host, json!({ "a": 1 })), (None, Some(1), None));
        assert_eq!(sent(&mut host, json!({ "a": 2 })), (Some(json!({ "a": 2 })), Some(2), None));
        // Back to what it was is a change too: the shell holds { a: 2 }.
        assert_eq!(sent(&mut host, json!({ "a": 1 })), (Some(json!({ "a": 1 })), Some(3), None));
    }

    #[test]
    fn opting_in_again_sends_the_view_again() {
        let mut host = ViewHost::new(false, &[]);
        host.skip_unchanged_views(false);
        let _ = sent(&mut host, json!({ "a": 1 }));
        host.skip_unchanged_views(false);
        assert!(host.needs_view(false));
        assert_eq!(sent(&mut host, json!({ "a": 1 })).0, Some(json!({ "a": 1 })));
    }

    #[test]
    fn a_machine_that_renders_every_change_is_built_only_after_a_render() {
        let mut host = ViewHost::new(true, &[]);
        assert!(host.needs_view(false), "nothing built yet");
        host.store(json!({ "a": 1 }));
        assert!(!host.needs_view(false));
        assert!(host.needs_view(true));
        // A machine without the promise is built after every answer.
        let mut plain = ViewHost::new(false, &[]);
        plain.store(json!({ "a": 1 }));
        assert!(plain.needs_view(false));
    }

    #[test]
    fn volatile_fields_leave_the_view_and_come_only_to_a_shell_that_asks() {
        let view = |pending: Value| json!({ "a": 1, "pending": pending });
        let mut quiet = ViewHost::new(false, &["pending"]);
        quiet.skip_unchanged_views(false);
        assert_eq!(sent(&mut quiet, view(json!({}))), (Some(json!({ "a": 1 })), Some(1), None));
        assert_eq!(sent(&mut quiet, view(json!({ "c1": ["x"] }))), (None, Some(1), None));

        let mut hedging = ViewHost::new(false, &["pending"]);
        hedging.skip_unchanged_views(true);
        assert_eq!(
            sent(&mut hedging, view(json!({}))),
            (Some(json!({ "a": 1 })), Some(1), Some(json!({ "pending": {} })))
        );
        assert_eq!(
            sent(&mut hedging, view(json!({ "c1": ["x"] }))),
            (None, Some(1), Some(json!({ "pending": { "c1": ["x"] } })))
        );
        assert_eq!(sent(&mut hedging, view(json!({ "c1": ["x"] }))), (None, Some(1), None));
    }
}
