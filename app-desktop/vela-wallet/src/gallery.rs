//! Dev-only state gallery: every screen and every failure the v2 onboarding
//! flow can show, side by side and reachable in one keypress.
//!
//! Reached only when `VELA_GALLERY=1` — the same env switch family as
//! `VELA_THEME` and `VELA_LANG`; release users never see it.
//!
//! ## Why the fixtures are `CreateView` values
//!
//! Because that is what the real screens read. Spec 014's gallery browsed a
//! parallel `CreatePanelState` enum invented for the purpose, which meant the
//! gallery could look right while the flow looked wrong. Here a fixture IS the
//! view model the core emits, so a screen that renders correctly in the gallery
//! renders correctly in the flow — the two cannot disagree, because there is
//! only one thing being rendered.
//!
//! The failure fixtures go one step further: they are `PromptKind`s, so
//! selecting "network" exercises the refinement in [`crate::outcome`] rather
//! than naming its result. A refinement that stops working shows up here as the
//! wrong card, which is the point.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    Context, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    ParentElement, Render, SharedString, StatefulInteractiveElement as _, Styled, Window, div, px,
};

use vela_core::app::create_wallet::{CreateKeyRow, CreateStage, CreateView, SubmitLabel};
use vela_core::app::{KeyMethod, PromptKind, StatusKey};

use crate::ctap::usb::{TouchKind, TouchRequest};
use crate::executor::passkey::{CredentialChoice, PinRequest};
use crate::identicon::IdenticonCache;
use crate::loc::Loc;
use crate::onboarding_flow::{FLOW_COLUMN_W, FlowEvent, FlowHost, FlowSink, render_create_flow};
use crate::outcome::{ActionId, Prompt, outcome_sheet};
use crate::theme::{
    self, FLOW_GAP_LG, FLOW_GAP_MD, FLOW_GAP_SM, GALLERY_SIDEBAR_W, RADIUS_CARD, Theme, ThemeMode,
};

/// The gallery gate. Same shape as `VELA_THEME` / `VELA_SKIP_LAUNCH_ANIMATION`.
pub fn gallery_enabled() -> bool {
    std::env::var("VELA_GALLERY").as_deref() == Ok("1")
}

/// The address every Done fixture shows — full 42 chars; display truncates,
/// copy does not.
const FIXTURE_ADDRESS: &str = "0x44EEC06897ff7ab8C7f16819511A64bA168A6D33";

enum Fixture {
    Flow(CreateView),
    Sheet {
        kind: PromptKind,
        confirmable: bool,
    },
    /// The three dialogs that belong to the CABLE rather than to the core, and
    /// so appear in no `CreateView` and no `PromptKind`. They are the states a
    /// reviewer is least able to reach on purpose — each needs a particular
    /// authenticator in a particular condition — which is exactly why they are
    /// here.
    Touch(TouchRequest),
    Pin(PinRequest),
    Pick(Vec<CredentialChoice>),
}

struct Entry {
    group: &'static str,
    code: &'static str,
    fixture: Fixture,
}

fn base_view() -> CreateView {
    CreateView {
        stage: CreateStage::Form,
        name: String::new(),
        name_editable: true,
        name_too_long: false,
        acks: vec![false, false],
        can_submit: false,
        submit_label: SubmitLabel::Create,
        busy: false,
        status: None,
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

/// A platform key carries a resolvable AAGUID; a security key deliberately
/// carries none, so the gallery shows the named case and the degradation on one
/// screen — hardware models live in the FIDO metadata service, not in the app's
/// provider catalog.
fn key(name: &str, method: KeyMethod, confirmed: bool, synced: bool) -> CreateKeyRow {
    let platform_key = method != KeyMethod::SecurityKey;
    let aaguid = if platform_key {
        "fbfc3007-154e-4ecc-8c0b-6e020557d7bd"
    } else {
        ""
    };
    CreateKeyRow {
        name: name.to_owned(),
        authenticator_attachment: if platform_key {
            "platform".to_owned()
        } else {
            "cross-platform".to_owned()
        },
        transports: if platform_key {
            "internal,hybrid".to_owned()
        } else {
            "usb".to_owned()
        },
        confirmed,
        synced,
        aaguid: aaguid.to_owned(),
        provider_name: vela_core::passkey::provider_name(aaguid)
            .unwrap_or_default()
            .to_owned(),
        method,
    }
}

fn choice(name: &str, credential_id: &str) -> CredentialChoice {
    CredentialChoice {
        name: name.to_owned(),
        credential_id: credential_id.to_owned(),
        product: "YubiKey 5C NFC".to_owned(),
    }
}

fn entries() -> Vec<Entry> {
    let mut out = Vec::new();
    let mut flow = |code: &'static str, view: CreateView| {
        out.push(Entry {
            group: "Create",
            code,
            fixture: Fixture::Flow(view),
        });
    };

    flow("name · empty", base_view());
    flow("name · filled", {
        let mut view = base_view();
        view.name = "Everyday wallet".to_owned();
        view.acks = vec![true, true];
        view.can_submit = true;
        view
    });
    flow("name · too long", {
        let mut view = base_view();
        view.name = "A wallet name that will not fit a WebAuthn user handle".to_owned();
        view.name_too_long = true;
        view
    });
    flow("name · draft waiting", {
        let mut view = base_view();
        view.name = "Everyday wallet".to_owned();
        view.name_editable = false;
        view.acks = vec![true, true];
        view.can_submit = true;
        view.submit_label = SubmitLabel::FinishVerify;
        view.show_start_over = true;
        view.status = Some(StatusKey::VerifyCancelled);
        view
    });
    flow("keys · one, needs a second", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.keys = vec![key("Everyday wallet", KeyMethod::SecurityKey, true, false)];
        view.needs_second_key = true;
        view
    });
    flow("keys · two, ready", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.keys = vec![
            key("Everyday wallet", KeyMethod::SecurityKey, true, false),
            key("Key 2", KeyMethod::SecurityKey, true, true),
        ];
        view.can_finish = true;
        view
    });
    flow("keys · unconfirmed row", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.keys = vec![
            key("Everyday wallet", KeyMethod::SecurityKey, true, true),
            key("Key 2", KeyMethod::SecurityKey, false, true),
        ];
        view
    });
    flow("keys · at the cap", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.keys = (0..7)
            .map(|index| {
                key(
                    &format!("Key {}", index + 1),
                    KeyMethod::SecurityKey,
                    true,
                    true,
                )
            })
            .collect();
        view.can_add_key = false;
        view.can_finish = true;
        view
    });
    for (code, status) in [
        ("progress · verify", StatusKey::VerifyingIdentity),
        ("progress · derive", StatusKey::ComputingAddress),
        ("progress · publish", StatusKey::SyncingKey),
    ] {
        flow(code, {
            let mut view = base_view();
            view.stage = CreateStage::AddKeys;
            view.busy = true;
            view.status = Some(status);
            view.keys = vec![
                key("Everyday wallet", KeyMethod::SecurityKey, true, true),
                key("Key 2", KeyMethod::SecurityKey, true, true),
            ];
            view
        });
    }
    flow("retry · publish failed", {
        let mut view = base_view();
        view.stage = CreateStage::SyncFailed;
        view.sync_error_detail =
            Some("Register failed: 503 · p256-index-v2.getvela.app".to_owned());
        view.keys = vec![key("Everyday wallet", KeyMethod::SecurityKey, true, true)];
        view
    });
    flow("done", {
        let mut view = base_view();
        view.stage = CreateStage::Created;
        view.address = Some(FIXTURE_ADDRESS.to_owned());
        view.keys = vec![
            key("Everyday wallet", KeyMethod::SecurityKey, true, true),
            key("Key 2", KeyMethod::SecurityKey, true, false),
        ];
        view
    });

    let mut hardware = |code: &'static str, fixture: Fixture| {
        out.push(Entry {
            group: "Security key",
            code,
            fixture,
        });
    };
    hardware(
        "touch · button",
        Fixture::Touch(TouchRequest {
            kind: TouchKind::Presence,
            product: "YubiKey 5C NFC".to_owned(),
            remote: false,
        }),
    );
    hardware(
        "touch · fingerprint",
        Fixture::Touch(TouchRequest {
            kind: TouchKind::Fingerprint,
            product: "YubiKey Bio".to_owned(),
            remote: false,
        }),
    );
    hardware(
        "touch · several keys",
        Fixture::Touch(TouchRequest {
            kind: TouchKind::Select,
            product: String::new(),
            remote: false,
        }),
    );
    hardware(
        "pin · first ask",
        Fixture::Pin(PinRequest {
            product: "YubiKey 5C NFC".to_owned(),
            device: "/dev/fixture".to_owned(),
            retries: Some(8),
            retry: false,
        }),
    );
    hardware(
        "pin · refused",
        Fixture::Pin(PinRequest {
            product: "YubiKey 5C NFC".to_owned(),
            device: "/dev/fixture".to_owned(),
            retries: Some(2),
            retry: true,
        }),
    );
    hardware(
        "pin · no count",
        Fixture::Pin(PinRequest {
            product: "Security key".to_owned(),
            device: "/dev/fixture".to_owned(),
            retries: None,
            retry: false,
        }),
    );
    hardware(
        "pick · two wallets",
        Fixture::Pick(vec![
            choice("Everyday wallet", "aa11bb22cc33dd44"),
            choice("Savings", "ee55ff66aa77bb88"),
        ]),
    );
    hardware(
        "pick · same name twice",
        Fixture::Pick(vec![
            choice("Everyday wallet", "aa11bb22cc33dd44"),
            choice("Everyday wallet", "ee55ff66aa77bb88"),
        ]),
    );
    hardware(
        "pick · one unnamed",
        Fixture::Pick(vec![
            choice("Everyday wallet", "aa11bb22cc33dd44"),
            choice("", "ee55ff66aa77bb88"),
        ]),
    );
    // A dozen rows — the case that once grew the card past the window (no
    // scroll, title off-screen, cancel unreachable). The rows region must
    // scroll inside the card while title and cancel stay put.
    hardware(
        "pick · a dozen wallets",
        Fixture::Pick(
            (0..12)
                .map(|index| {
                    choice(
                        &format!("Wallet {}", index + 1),
                        &format!("{index:02}11bb22cc33dd44"),
                    )
                })
                .collect(),
        ),
    );

    // The failure sheet, one row per outcome the catalog names. The two that
    // carry a detail string are driven through the refinement rather than
    // around it, so this list is also a check on it.
    let mut sheet = |code: &'static str, kind: PromptKind, confirmable: bool| {
        out.push(Entry {
            group: "Failures",
            code,
            fixture: Fixture::Sheet { kind, confirmable },
        });
    };
    sheet("unsupported", PromptKind::NotSupportedCreate, false);
    sheet("unsupported · login", PromptKind::NotSupportedLogin, false);
    sheet("not discoverable", PromptKind::NotDiscoverable, false);
    sheet("incompatible", PromptKind::IncompatibleCreate, false);
    sheet("incompatible · login", PromptKind::IncompatibleLogin, false);
    sheet("recover offer", PromptKind::RecoverOffer, true);
    sheet("recover failed", PromptKind::RecoverFailed, false);
    sheet(
        "create failed · unknown",
        PromptKind::CreateFailed {
            detail: "the security key returned no pinUvAuthToken".to_owned(),
        },
        false,
    );
    sheet(
        "create failed · network",
        PromptKind::CreateFailed {
            detail: "Register failed: connection refused".to_owned(),
        },
        false,
    );
    sheet(
        "create failed · server",
        PromptKind::CreateFailed {
            detail: "Register failed: http status: 503".to_owned(),
        },
        false,
    );
    sheet(
        "create failed · timeout",
        PromptKind::CreateFailed {
            detail: "Register timed out after 120s".to_owned(),
        },
        false,
    );
    sheet(
        "create failed · no key",
        PromptKind::CreateFailed {
            detail: "No security key is plugged in. Insert one and try again.".to_owned(),
        },
        false,
    );
    sheet(
        "sign-in failed",
        PromptKind::SignInFailed {
            detail: "the security key holds no Vela passkey".to_owned(),
        },
        false,
    );
    out
}

pub struct GalleryView {
    mode: ThemeMode,
    loc: Loc,
    entries: Vec<Entry>,
    selected: usize,
    /// The one thing a fixture is allowed to remember between frames: the
    /// picker and the copy feedback are presentation, not view-model state.
    picker_open: bool,
    copied: bool,
    details_expanded: bool,
    /// The PIN fixture is typeable, so the masking can be looked at.
    pin_value: String,
    name_focus: FocusHandle,
    focus_handle: FocusHandle,
    identicons: RefCell<IdenticonCache>,
    passkey_icons: RefCell<crate::passkey_icons::PasskeyIconCache>,
    /// Always empty here: the gallery's keys are fixtures, and a review screen
    /// that reached the network would be a review of the network.
    directory: RefCell<crate::passkey_directory::PasskeyDirectory>,
}

impl GalleryView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loc = Loc::from_env();
        eprintln!(
            "[vela-wallet] gallery: locale resolved to `{}`",
            loc.language()
        );
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);
        let entries = entries();
        // `VELA_GALLERY_STATE=<n>` opens straight onto one fixture. It exists
        // because a machine without screen-recording permission cannot drive
        // the arrow keys OR take a picture, and "launch it once per fixture and
        // see whether it survives a frame" is the only end-to-end check left —
        // `scripts/sweep-gallery.sh` is that loop.
        let selected = std::env::var("VELA_GALLERY_STATE")
            .ok()
            .and_then(|raw| raw.parse::<usize>().ok())
            .filter(|index| *index < entries.len())
            .unwrap_or(0);
        eprintln!(
            "[vela-wallet] gallery: {} states, opening `{}`",
            entries.len(),
            entries[selected].code
        );
        Self {
            mode: ThemeMode::detect(window),
            loc,
            entries,
            selected,
            picker_open: false,
            copied: false,
            details_expanded: false,
            pin_value: String::new(),
            name_focus: cx.focus_handle(),
            focus_handle,
            identicons: RefCell::default(),
            passkey_icons: RefCell::default(),
            directory: RefCell::default(),
        }
    }

    fn select(&mut self, ix: usize, cx: &mut Context<Self>) {
        if ix >= self.entries.len() {
            return;
        }
        self.selected = ix;
        // Every fixture is entered fresh: a disclosure left open in one state
        // must not appear opened in the next.
        self.picker_open = false;
        self.copied = false;
        self.details_expanded = false;
        self.pin_value.clear();
        cx.notify();
    }

    fn sidebar(&self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let toggle_label = match self.mode {
            ThemeMode::Light => "Dark",
            ThemeMode::Dark => "Light",
        };
        let hover_bg = theme.bg_sunken;
        let header = div()
            .px(px(FLOW_GAP_LG))
            .py(px(FLOW_GAP_MD))
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(theme::text_card_title())
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("State Gallery"),
            )
            .child(
                div()
                    .id("theme-toggle")
                    .px(px(FLOW_GAP_MD))
                    .py(px(FLOW_GAP_SM))
                    .rounded(px(RADIUS_CARD / 2.))
                    .border_1()
                    .border_color(theme.divider)
                    .text_size(theme::text_flow_caption())
                    .text_color(theme.fg_muted)
                    .cursor_pointer()
                    .hover(move |s| s.bg(hover_bg))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.mode = match this.mode {
                            ThemeMode::Light => ThemeMode::Dark,
                            ThemeMode::Dark => ThemeMode::Light,
                        };
                        cx.notify();
                    }))
                    .child(toggle_label),
            );

        let mut list = div()
            .id("gallery-list")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .pb(px(FLOW_GAP_LG))
            .flex()
            .flex_col();
        let mut last_group = "";
        for (ix, entry) in self.entries.iter().enumerate() {
            if entry.group != last_group {
                last_group = entry.group;
                list = list.child(
                    div()
                        .px(px(FLOW_GAP_LG))
                        .pt(px(FLOW_GAP_LG))
                        .pb(px(FLOW_GAP_SM))
                        .text_size(theme::text_numeral())
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(theme.fg_subtle)
                        .child(entry.group),
                );
            }
            let selected = ix == self.selected;
            let row_bg = if selected {
                theme.bg_sunken
            } else {
                theme.bg_raised
            };
            let row_hover = theme.bg_sunken;
            list = list.child(
                div()
                    .id(("gallery-row", ix as u64))
                    .mx(px(FLOW_GAP_MD))
                    .px(px(FLOW_GAP_MD))
                    .py(px(FLOW_GAP_SM))
                    .rounded(px(RADIUS_CARD / 2.))
                    .bg(row_bg)
                    .cursor_pointer()
                    .hover(move |s| s.bg(row_hover))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(ix, cx)))
                    .child(
                        div()
                            .text_size(theme::text_body())
                            .font_weight(if selected {
                                FontWeight::SEMIBOLD
                            } else {
                                FontWeight::NORMAL
                            })
                            .text_color(theme.fg_base)
                            .child(SharedString::from(entry.code)),
                    ),
            );
        }

        div()
            .w(px(GALLERY_SIDEBAR_W))
            .h_full()
            .flex_none()
            .bg(theme.bg_raised)
            .border_r_1()
            .border_color(theme.divider)
            .flex()
            .flex_col()
            .child(header)
            .child(div().h(px(theme::HAIRLINE)).w_full().bg(theme.divider))
            .child(list)
    }

    fn stage(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> gpui::Stateful<Div> {
        let entity = cx.entity();
        let body: Div = match &self.entries[self.selected].fixture {
            Fixture::Flow(view) => {
                let sink: FlowSink = Rc::new(move |event, _window, cx| {
                    entity.update(cx, |this, cx| match event {
                        // The gallery is a viewer, not a driver: only the two
                        // presentation-local interactions do anything, and no
                        // press reaches a core, because there is no core here.
                        FlowEvent::TogglePicker => {
                            this.picker_open = !this.picker_open;
                            cx.notify();
                        }
                        FlowEvent::CopyAddress => {
                            this.copied = true;
                            cx.notify();
                        }
                        other => eprintln!("[vela-wallet] gallery: {other:?}"),
                    });
                });
                let host = FlowHost {
                    theme,
                    loc: &self.loc,
                    view,
                    name_focus: &self.name_focus,
                    identicons: &self.identicons,
                    passkey_icons: &self.passkey_icons,
                    // The gallery asks nobody: its keys are fixtures, and a
                    // review screen that reached the network would be a review
                    // of the network.
                    directory: &self.directory,
                    picker_open: self.picker_open,
                    copied: self.copied,
                    sink,
                };
                render_create_flow(&host, window)
            }
            // The cable's three dialogs, rendered bare: the gallery IS the
            // backdrop, and each card's own scrim would cover the sidebar.
            Fixture::Touch(request) => crate::hardware::touch_card(theme, &self.loc, request),
            Fixture::Pin(request) => crate::hardware::pin_card(
                theme,
                &self.loc,
                request,
                &self.pin_value,
                &self.name_focus,
                window,
                {
                    let this = entity.clone();
                    move |next: String, _window: &mut Window, cx: &mut gpui::App| {
                        this.update(cx, |this, cx| {
                            this.pin_value = next;
                            cx.notify();
                        });
                    }
                },
                |_, _, _| {},
                |_, _, _| {},
            ),
            Fixture::Pick(choices) => {
                crate::hardware::pick_card(theme, &self.loc, choices, |_, _, _| {}, |_, _, _| {})
            }
            Fixture::Sheet { kind, confirmable } => {
                let mut prompt = Prompt::new(kind.clone(), *confirmable, 0);
                prompt.details_expanded = self.details_expanded;
                // Rendered inline rather than over a scrim: the gallery IS the
                // backdrop, and a full-bleed dim would cover the sidebar.
                div()
                    .w(px(FLOW_COLUMN_W))
                    .flex()
                    .justify_center()
                    .child(outcome_sheet(
                        theme,
                        &self.loc,
                        &prompt,
                        move |id, _window, cx| {
                            entity.update(cx, |this, cx| {
                                if id == ActionId::ToggleDetails {
                                    this.details_expanded = !this.details_expanded;
                                    cx.notify();
                                }
                            });
                        },
                    ))
            }
        };

        div()
            .id("gallery-stage")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_y_scroll()
            .flex()
            .justify_center()
            .items_start()
            .py(px(FLOW_GAP_LG * 2.))
            .child(body)
    }
}

impl Render for GalleryView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(self.mode);

        div()
            .size_full()
            .flex()
            .font_family(theme::font_ui())
            .bg(theme.bg_base)
            .text_color(theme.fg_base)
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                match event.keystroke.key.as_str() {
                    "down" => {
                        let next = (this.selected + 1).min(this.entries.len().saturating_sub(1));
                        this.select(next, cx);
                    }
                    "up" => {
                        let prev = this.selected.saturating_sub(1);
                        this.select(prev, cx);
                    }
                    _ => {}
                }
            }))
            .child(self.sidebar(&theme, cx))
            .child(self.stage(&theme, window, cx))
    }
}

/// Sanity for the one thing this file asserts about the product: that the
/// gallery's failure rows still land on the outcomes they are named after.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::outcome::OutcomeKind;

    #[test]
    fn the_failure_fixtures_refine_to_the_outcomes_they_are_named_for() {
        let expected = [
            ("create failed · network", OutcomeKind::Network),
            ("create failed · server", OutcomeKind::Server),
            ("create failed · timeout", OutcomeKind::Timeout),
            ("create failed · no key", OutcomeKind::Unsupported),
            ("create failed · unknown", OutcomeKind::Unknown),
            ("sign-in failed", OutcomeKind::SignInFailed),
        ];
        for entry in entries() {
            let Fixture::Sheet { kind, .. } = &entry.fixture else {
                continue;
            };
            if let Some((_, want)) = expected.iter().find(|(code, _)| *code == entry.code) {
                assert_eq!(
                    OutcomeKind::for_prompt(kind),
                    *want,
                    "fixture `{}` no longer refines to {want:?}",
                    entry.code
                );
            }
        }
    }
}
