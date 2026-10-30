# N-D1 spike — an accessibility tree for the desktop (gpui + AccessKit)

2026-10-30. Question: which gpui/zed pin gives this app a screen-reader
accessibility tree (VoiceOver / Narrator / Orca), and what does moving to it cost?

## Verdict: GO — and no pin bump is needed

**The current pin already ships AccessKit.** `Cargo.lock` pins gpui at zed
`c97b7c0ea41acf56e9726964eca10c2078e921b0` (2026-07-28). AccessKit landed upstream in
zed PR #56065 (merged 2026-05-27, `1d029c5f`), before that pin. Verified in the
checkout (`~/.cargo/git/checkouts/zed-*/c97b7c0`):

- zed's workspace `Cargo.toml` carries `accesskit 0.24`, `accesskit_macos 0.26`,
  `accesskit_windows 0.33.1`, `accesskit_unix 0.21`; this app's `Cargo.lock` already
  resolves them (26 `accesskit` lines).
- `gpui::Application::with_platform` creates the adapter; only
  `Application::new_inaccessible` opts out. This app starts through
  `gpui_platform::application()` (`src/main.rs`), i.e. **accessibility is ON today** —
  the tree is just empty, because no element names a role.
- Element API (`crates/gpui/src/elements/div.rs`, guide `crates/gpui/src/_accessibility.rs`):
  `.id(..)` + `.role(Role::…)` make a node; `.aria_label`, `aria_description`,
  `aria_value`, `aria_selected`, `aria_expanded`, `aria_toggled`, numeric
  value/min/max/step, level, position; `.on_a11y_action(..)`; `on_click` registers the
  Click action. `window.is_a11y_active()`, `debug_a11y_tree_json()` for tests.
- The vendored `app-desktop/vendor/gpui_macos` keeps the AccessKit adapter.

So `src/settings/mod.rs:95-96` ("gpui draws no accessibility tree at this pin") is
wrong about gpui: the app supplies no nodes.

## Two things to fix first

1. **A leak on macOS, live today.** Upstream PR #64143 (merged 2026-09-20,
   `0eda7703f6c8`) breaks a reference cycle between the AccessKit adapter and the
   window: without it every closed window's view, Metal layer and renderer are never
   freed. Upstream thought it hit only the opt-in path; this app is on that path by
   default. Backport: drop `accesskit_adapter` before `renderer.destroy()` in the
   vendored `MacWindow::drop` (`vendor/gpui_macos/src/window.rs`). Small; needs a
   window open/close leak check.
2. **Nothing to bump.** The only later API worth having is `.accessibility_id(..)`
   (PR #61926, 2026-08-07: AXIdentifier / AutomationId / AccessibleId) — stable ids for
   a UI driver (T4). zed main is ~1,370 commits past the pin, needs Rust 1.98.1 (this
   app: 1.97.1) and a re-vendor of `gpui_macos` with the spec-095 private-API removals.
   Not worth it for N-D1; revisit for T4, or backport `accessibility_id` alone.

## Plan (N-D1, ~3–5 days)

1. Backport #64143 into `vendor/gpui_macos`; leak test (open/close 20 windows, RSS flat).
2. Shared components first, each with `.id().role().aria_label()` from the
   translated corpus: `cta_button` (Button), `toggle_row` (Switch, `aria_toggled`),
   `SegmentedToggle`/chips (RadioButton, `aria_selected`), `editable_url_field` and
   the editor (TextInput), dialogs (Dialog, focus trap), the sidebar (Tab/List),
   network/contact/token rows (ListItem). Loop-built children get their own ids.
3. Text: wrap prose in labelled parents (`text!()` or an `aria_label` on the row) —
   bare `SharedString` children are not reported.
4. A test that snapshots `debug_a11y_tree_json()` for home, send confirm, settings;
   then manual passes: Accessibility Inspector + VoiceOver (macOS), Narrator + Inspect
   (Windows), Orca + Accerciser (Linux).
5. Fix the comment at `settings/mod.rs:95-96`; record in README.
6. Only on a future bump: re-verify the three README "Known gpui quirks" (display
   origin `(0,0)`, nil `mainMenu` fullscreen reveal, `hasVisibleWindows` reopen) on two
   displays.

## Sources

- https://github.com/zed-industries/zed/pull/56065 (AccessKit)
- https://github.com/zed-industries/zed/pull/64143 (adapter/window cycle)
- https://github.com/zed-industries/zed/pull/61926 (`accessibility_id`)
- https://api.github.com/repos/zed-industries/zed/commits/c97b7c0ea41acf56e9726964eca10c2078e921b0
