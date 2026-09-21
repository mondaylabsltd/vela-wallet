//! Welcome, and the two ways in.
//!
//! One column at every width: brand, headline, and two buttons. Spec 014's
//! two-column layout — feature-card grid on the left, action panel on the
//! right, flow swapped in place — is gone. The v2 design makes the flow a full
//! page of its own, so there is no second pane for anything to swap into.
//!
//! **Creating a wallet is a stepped journey and takes over the page.** Signing
//! in has no steps — one ceremony, and you are either in or you are not — so it
//! runs here, and speaks only through the button's busy state and the failure
//! sheet.
//!
//! ## What this page owns
//!
//! Both onboarding machines, the ceremony channel that carries "touch your key"
//! and "type your PIN" up from a background thread, the failure sheet, and the
//! endpoint surface. It is the whole shell for onboarding; `main.rs` decides
//! only whether onboarding is the route at all.

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, Context, Div, FocusHandle, FontWeight, InteractiveElement as _, IntoElement, KeyDownEvent,
    MouseButton, ParentElement, Render, SharedString, Stateful, StatefulInteractiveElement as _,
    Styled, Window, div, px,
};

use vela_core::app::create_wallet::{CreateView, CreateWallet};
use vela_core::app::login::{Login, LoginView};
use vela_core::app::shell::{ShellOperation, ShellResult};

use crate::ceremony::CeremonyChannel;
use crate::core_host::{CoreHost, Pending};
use crate::executor::passkey::WindowHandle;
use crate::executor::{
    self, Performed,
    passkey::{CredentialChoice, PinRequest},
    registry, storage,
};
use crate::hardware;
use crate::identicon::IdenticonCache;
use crate::intro;
use crate::loc::Loc;
use crate::onboarding_flow::{self, FLOW_STEPS, FlowEvent, FlowHost, FlowSink, render_create_flow};
use crate::outcome::{ActionId, Prompt, SHEET_PAD, SHEET_RADIUS, SHEET_W, outcome_sheet};
use crate::passkey_directory::{self, PasskeyDirectory};
use crate::passkey_icons::PasskeyIconCache;
use crate::session;
use crate::theme::{
    self, CONTENT_PAD_X, CONTENT_PAD_Y, FLOW_COLUMN_W, FLOW_GAP_LG, FLOW_GAP_MD, GAP_HERO_CTA,
    GAP_HERO_SUB, GAP_WELCOME_CTA, Theme, ThemeMode,
};
use crate::ui::{
    ButtonState, ButtonVariant, LaunchAnimation, NameFieldStrings, RailSlot, onboarding_rail,
    text_field, vela_button, welcome_cta, welcome_cta_state,
};
use crate::window_frame::{
    CAPTION_H, FRAME_SHADOW, frame_tiling, owns_titlebar, round_to_frame, titlebar, window_frame,
};

/// Height of the drag strip where the page draws its own caption.
///
/// `CAPTION_H`, not the 96 the v1 welcome could afford. A drag area hit-tests
/// as `HTCAPTION` on Windows and the OS then routes its input down the
/// non-client path, so ANY control under it is dead — and v2's flow puts the
/// ‹ 返回 link at the top of the page, 44px down. 96 swallowed it.
const DRAG_STRIP_H: f32 = CAPTION_H;

/// How often the screen looks at the ceremony channel while work is in flight.
///
/// A poll rather than a wake-up because the two facts it carries originate on a
/// thread that is blocked inside a USB read — it has no gpui handle to notify
/// with, and giving it one would mean holding an app context across an await
/// inside a synchronous CTAP2 exchange. 120 ms is well under the threshold at
/// which a "touch your key" line reads as late, and the loop stops the moment
/// both machines go idle.
const CEREMONY_TICK_MS: u64 = 120;

/// Which machine a prompt belongs to. A prompt is an effect the CORE is waiting
/// on, so answering it has to reach the right one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Machine {
    Create,
    Login,
}

/// The endpoint surface's editable state.
struct EndpointSurface {
    url: String,
    focus: FocusHandle,
    /// Opened by the health probe rather than by a press. Dismissing it must
    /// not re-open it on the next frame, so the automatic open happens once.
    automatic: bool,
}

/// The PIN dialog's state. `request` is what the authenticator told us about
/// itself; `value` is what the person has typed so far.
struct PinDialog {
    request: PinRequest,
    value: String,
    focus: FocusHandle,
}

pub struct OnboardingPage {
    mode: ThemeMode,
    loc: Loc,
    focus_handle: FocusHandle,
    launch: Option<LaunchAnimation>,
    /// The DONE card's avatar (spec 015 D1's rasterizer, reused).
    identicons: RefCell<IdenticonCache>,
    /// The method rows' marks (spec 038).
    passkey_icons: RefCell<PasskeyIconCache>,
    /// Names and marks for models the compiled catalog cannot name — asked
    /// once per AAGUID, from the render pass that first needs one.
    directory: RefCell<PasskeyDirectory>,

    /// The first-run intro is up (spec 038). `None` once left — or on every
    /// run after the first, unless `VELA_INTRO=1` asks for it.
    intro: Option<intro::IntroState>,
    /// The create journey has taken over the page.
    creating: bool,
    create: CoreHost<CreateWallet>,
    create_view: CreateView,
    name_focus: FocusHandle,
    picker_open: bool,
    copied: bool,
    /// The sign-in method picker is open — the person tapped "I already have a
    /// wallet" and is choosing this device, a phone by scan, or a security key.
    signin_methods_open: bool,

    login: CoreHost<Login>,
    login_view: LoginView,

    channel: Arc<CeremonyChannel>,
    /// The native window, for the one platform whose passkey dialog is the
    /// OS's rather than ours. Captured at construction because a ceremony runs
    /// on a background thread and cannot reach `Window` from there.
    window_handle: WindowHandle,
    /// The one modal. `Some` ⇒ a machine is waiting for an answer.
    prompt: Option<(Machine, Prompt)>,
    pin: Option<PinDialog>,
    /// The key offered several wallets and one has to be chosen.
    pick: Option<Vec<CredentialChoice>>,
    endpoint: Option<EndpointSurface>,
    /// The person closed the endpoint card the probe had opened. Recorded
    /// HERE rather than as `endpoint == None`, because `None` is also the
    /// state that makes the probe open it again — which is exactly what made
    /// Close hand the card straight back (spec 038 finding 6). Cleared by a
    /// re-probe (`save_endpoint` → `Event::Start`), so a new answer from the
    /// index is allowed to raise it once more.
    endpoint_dismissed: bool,
    /// Opened over a signed-in wallet to add another account (spec 072), so
    /// every way out of it leads back to that wallet rather than to a first
    /// run nobody is on.
    adding: bool,
    /// Whether the ceremony-channel poll is running.
    ///
    /// A bool, NOT the `Task`. gpui cancels a task when its handle is dropped,
    /// and the poll decides for itself when to stop — so holding the handle
    /// means the loop drops the very task it is running inside, which aborts
    /// the process rather than ending the loop. The task is detached and stops
    /// by returning; this flag is only what keeps a second one from starting.
    watching: bool,
}

impl OnboardingPage {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let loc = Loc::from_env();
        eprintln!(
            "[vela-wallet] onboarding: locale resolved to `{}`",
            loc.language()
        );

        // Restyle when the OS appearance flips, unless VELA_THEME pins it.
        let page = cx.weak_entity();
        window
            .observe_window_appearance(move |window, cx| {
                if ThemeMode::is_pinned() {
                    return;
                }
                let mode = ThemeMode::detect(window);
                if let Some(page) = page.upgrade() {
                    page.update(cx, |this, cx| {
                        this.mode = mode;
                        cx.notify();
                    });
                }
            })
            .detach();

        let mode = ThemeMode::detect(window);
        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        // A saved endpoint override is the person's, and it applies before the
        // first probe — otherwise the health check tests the default and the
        // sign-in tests theirs.
        if let Some(url) = storage::load_registry_endpoint() {
            registry::set_registry_url(&url);
        }

        let create = CoreHost::<CreateWallet>::new();
        let login = CoreHost::<Login>::new();
        let create_view = create.view();
        let login_view = login.view();

        let mut page = Self {
            // Spec 038: the 7-day replay window the web has had since spec 012.
            // Before this the desktop played it on EVERY start.
            launch: if theme::launch_disabled()
                || !theme::launch_due(
                    storage::read_epoch_ms(theme::LAUNCH_PLAYED_KEY),
                    crate::executor::now_ms(),
                ) {
                None
            } else {
                Some(LaunchAnimation::new(mode, cx))
            },
            mode,
            loc,
            focus_handle,
            identicons: RefCell::default(),
            passkey_icons: RefCell::default(),
            directory: RefCell::default(),
            intro: if std::env::var("VELA_INTRO").as_deref() == Ok("1")
                || storage::read_epoch_ms(theme::INTRO_SEEN_KEY).is_none()
            {
                Some(intro::IntroState::default())
            } else {
                None
            },
            creating: false,
            create,
            create_view,
            name_focus: cx.focus_handle(),
            picker_open: false,
            copied: false,
            signin_methods_open: false,
            login,
            login_view,
            endpoint_dismissed: false,
            channel: CeremonyChannel::new(),
            window_handle: native_window_handle(window),
            prompt: None,
            pin: None,
            pick: None,
            endpoint: None,
            adding: false,
            watching: false,
        };

        // The reachability probe starts with the page. On the web this waits
        // for a press because the core is a 3.4 MB download; here it is already
        // in the binary, so the person learns the index is unreachable BEFORE
        // they press a button rather than after.
        let pending = page.login.dispatch(vela_core::app::login::Event::Start);
        page.pump_login(pending, cx);
        page
    }

    /// Onboarding over a signed-in wallet: "create a new account" goes
    /// straight into the journey, "sign in to an existing one" straight to its
    /// methods. No launch animation and no introduction — this is a task
    /// somebody started from Settings, not a first launch.
    pub fn adding(entry: session::AddAccount, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self::new(window, cx);
        page.adding = true;
        page.launch = None;
        page.intro = None;
        match entry {
            session::AddAccount::Create => page.start_create(cx),
            session::AddAccount::SignIn => page.signin_methods_open = true,
        }
        page
    }

    /// Back to the wallet this was opened over, with no account added.
    fn leave_adding(&mut self, cx: &mut Context<Self>) {
        session::add_account_cancelled(cx);
        cx.notify();
    }

    // -- the two entrances --------------------------------------------------

    fn start_create(&mut self, cx: &mut Context<Self>) {
        self.creating = true;
        self.picker_open = false;
        self.copied = false;
        let pending = self
            .create
            .dispatch(vela_core::app::create_wallet::Event::Start);
        self.pump_create(pending, cx);
        cx.notify();
    }

    fn sign_in(&mut self, method: vela_core::app::KeyMethod, cx: &mut Context<Self>) {
        self.signin_methods_open = false;
        if self.login_view.busy {
            return;
        }
        // The method routes: `SecurityKey` runs the app-owned USB ceremony,
        // `Hybrid` shows a QR and signs in through a phone over caBLE. (Platform
        // has no route on the desktop and the picker presents it as unavailable.)
        let pending = self
            .login
            .dispatch(vela_core::app::login::Event::SignIn { method });
        self.pump_login(pending, cx);
        cx.notify();
    }

    /// One intro event. The state decides; the page marks it seen and routes.
    fn on_intro_event(
        &mut self,
        event: intro::IntroEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.intro.as_mut() else {
            return;
        };
        if let Some(exit) = state.apply(event) {
            // Seen: whichever way this ends, they have read it. Marked on the
            // press rather than on success, so a cancelled passkey prompt
            // drops them on Welcome, not back into the introduction.
            storage::write_epoch_ms(theme::INTRO_SEEN_KEY, crate::executor::now_ms());
            self.intro = None;
            match exit {
                intro::IntroExit::Skip => {}
                intro::IntroExit::Create => self.start_create(cx),
                intro::IntroExit::SignIn => self.signin_methods_open = true,
            }
        }
        cx.notify();
    }

    /// Leaving the create journey. The core is discarded WITH its drafts, which
    /// is why this is only reachable from a screen that has none in flight —
    /// `render_create_flow` withholds the back affordance during progress.
    fn leave_create(&mut self, cx: &mut Context<Self>) {
        self.channel.close();
        self.channel = CeremonyChannel::new();
        self.create = CoreHost::<CreateWallet>::new();
        self.create_view = self.create.view();
        self.creating = false;
        self.picker_open = false;
        self.prompt = None;
        self.pin = None;
        self.pick = None;
        // Opened to add an account: leaving the journey is leaving the task.
        if self.adding {
            self.leave_adding(cx);
        }
        cx.notify();
    }

    // -- the effect pumps ---------------------------------------------------

    fn pump_create(&mut self, pending: Vec<Pending<ShellOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            self.perform(Machine::Create, effect, cx);
        }
        self.create_view = self.create.view();
        self.ensure_watcher(cx);
        cx.notify();
    }

    fn pump_login(&mut self, pending: Vec<Pending<ShellOperation>>, cx: &mut Context<Self>) {
        for effect in pending {
            self.perform(Machine::Login, effect, cx);
        }
        self.login_view = self.login.view();
        self.ensure_watcher(cx);
        cx.notify();
    }

    /// Start one operation.
    ///
    /// Two of the eighteen never leave this thread: a prompt is the screen, and
    /// a completion is a hand-off to the session. Everything else is blocking
    /// work — a USB device, TLS, a person's finger — and goes to the background
    /// executor with the answer routed back by effect id.
    fn perform(
        &mut self,
        machine: Machine,
        effect: Pending<ShellOperation>,
        cx: &mut Context<Self>,
    ) {
        match &effect.operation {
            ShellOperation::Prompt { kind, confirmable } => {
                self.prompt = Some((machine, Prompt::new(kind.clone(), *confirmable, effect.id)));
                // Deliberately NOT resolved here: the core is waiting for an
                // answer a person has not given yet.
                return;
            }
            ShellOperation::CompleteOnboarding { mode } => {
                session::account_established(mode.clone(), cx);
                self.resolve(machine, effect.id, ShellResult::OnboardingCompleted, cx);
                return;
            }
            _ => {}
        }

        // The OS dialog on Windows parents itself to this. gpui hands over a
        // real window handle; every other platform ignores it.
        let ceremony = self.channel.ceremony(self.window_handle);
        let operation = effect.operation.clone();
        let id = effect.id;
        cx.spawn(async move |page, cx| {
            let result = cx
                .background_executor()
                .spawn(async move {
                    match executor::perform(&operation, &ceremony) {
                        Performed::Now(result) => *result,
                        // Unreachable: the two screen-owned operations returned
                        // above. Answered rather than dropped, because a core
                        // left waiting on an effect nobody will resolve is a
                        // flow that hangs with no error.
                        Performed::Screen => ShellResult::PromptAnswered { accepted: false },
                    }
                })
                .await;
            page.update(cx, |page, cx| page.resolve(machine, id, result, cx))
                .ok();
        })
        .detach();
    }

    fn resolve(&mut self, machine: Machine, id: u64, result: ShellResult, cx: &mut Context<Self>) {
        match machine {
            Machine::Create => {
                let pending = self.create.resolve(id, result);
                self.pump_create(pending, cx);
            }
            Machine::Login => {
                let pending = self.login.resolve(id, result);
                self.pump_login(pending, cx);
            }
        }
    }

    /// Poll the ceremony channel while anything is in flight.
    fn ensure_watcher(&mut self, cx: &mut Context<Self>) {
        if self.watching || (self.create.is_idle() && self.login.is_idle()) {
            return;
        }
        self.watching = true;
        // Detached: the loop ends by RETURNING, never by having its handle
        // dropped. `page.update` also fails once the screen is gone, which is
        // the other way out.
        cx.spawn(async move |page, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(CEREMONY_TICK_MS))
                    .await;
                let keep_going = page.update(cx, |page, cx| page.tick(cx)).unwrap_or(false);
                if !keep_going {
                    break;
                }
            }
        })
        .detach();
    }

    /// One poll. Returns whether to keep polling.
    fn tick(&mut self, cx: &mut Context<Self>) -> bool {
        if let Some(request) = self.channel.pending_pin() {
            if self
                .pin
                .as_ref()
                .is_none_or(|open| open.request.retry != request.retry)
            {
                self.pin = Some(PinDialog {
                    request,
                    value: String::new(),
                    focus: cx.focus_handle(),
                });
            }
        } else if self.pin.is_some() {
            // The ceremony took the answer; the dialog's job is done.
            self.pin = None;
        }

        let asking = self.channel.pending_choice();
        if self.pick.is_some() != asking.is_some() {
            self.pick = asking;
        }

        let busy = !self.create.is_idle() || !self.login.is_idle();
        cx.notify();
        if !busy {
            // Only clears the flag. The task ends because this returns `false`.
            self.watching = false;
        }
        busy
    }

    // -- what the screens ask for -------------------------------------------

    fn on_flow_event(&mut self, event: FlowEvent, window: &mut Window, cx: &mut Context<Self>) {
        use vela_core::app::create_wallet::Event as CreateEvent;

        let core_event = match event {
            FlowEvent::NameChanged(name) => CreateEvent::NameChanged { name },
            FlowEvent::AckToggled(index) => CreateEvent::AckToggled { index },
            FlowEvent::Submit => CreateEvent::Submit,
            FlowEvent::StartOver => CreateEvent::StartOver,
            FlowEvent::AddKey(method) => {
                self.picker_open = false;
                CreateEvent::AddKey {
                    // Empty ⇒ the core labels it "Key N". The desktop has no
                    // per-key rename control yet, and inventing a name here
                    // would put a shell's guess where the core has a rule.
                    name: String::new(),
                    method,
                }
            }
            FlowEvent::ConfirmKey(index) => CreateEvent::ConfirmKey { index },
            FlowEvent::RemoveKey(index) => CreateEvent::RemoveKey { index },
            FlowEvent::FinishKeys => CreateEvent::FinishKeys,
            FlowEvent::RetryUpload => CreateEvent::RetryUpload,
            FlowEvent::EnterWallet => CreateEvent::EnterWallet,
            FlowEvent::Back => {
                // The CORE owns whether there is a step to go back to; leaving
                // the flow entirely is this page's, because the core has no
                // idea what contains it.
                if self.create_view.can_go_back {
                    CreateEvent::GoBack
                } else {
                    self.leave_create(cx);
                    return;
                }
            }
            FlowEvent::TogglePicker => {
                self.picker_open = !self.picker_open;
                cx.notify();
                return;
            }
            FlowEvent::CopyAddress => {
                if let Some(address) = &self.create_view.address {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(address.clone()));
                    self.copied = true;
                    cx.notify();
                }
                return;
            }
            FlowEvent::MethodUnavailable(_) => {
                // The picker already says why in the row itself; pressing it is
                // not an error to report, just nothing to do.
                return;
            }
        };

        // The name field keeps focus across a keystroke-driven re-render.
        if matches!(core_event, CreateEvent::NameChanged { .. }) {
            self.name_focus.focus(window, cx);
        }
        let pending = self.create.dispatch(core_event);
        self.pump_create(pending, cx);
    }

    fn on_sheet_action(&mut self, id: ActionId, cx: &mut Context<Self>) {
        let Some((machine, prompt)) = self.prompt.as_mut() else {
            return;
        };
        match id {
            ActionId::ToggleDetails => {
                prompt.details_expanded = !prompt.details_expanded;
                cx.notify();
            }
            ActionId::ReportError => {
                if let Some(details) = prompt.details.clone() {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(details));
                }
            }
            ActionId::EditIndexEndpoint => {
                self.open_endpoint(false, cx);
            }
            ActionId::Accept | ActionId::Decline => {
                let accepted = id == ActionId::Accept;
                let machine = *machine;
                let effect_id = prompt.effect_id;
                self.prompt = None;
                self.resolve(
                    machine,
                    effect_id,
                    ShellResult::PromptAnswered { accepted },
                    cx,
                );
            }
        }
    }

    fn open_endpoint(&mut self, automatic: bool, cx: &mut Context<Self>) {
        if self.endpoint.is_some() {
            return;
        }
        self.endpoint = Some(EndpointSurface {
            url: registry::registry_url(),
            focus: cx.focus_handle(),
            automatic,
        });
        cx.notify();
    }

    fn save_endpoint(&mut self, cx: &mut Context<Self>) {
        let Some(surface) = self.endpoint.take() else {
            return;
        };
        // Pointing somewhere new is a fresh question to the index; its answer
        // may open the card again.
        self.endpoint_dismissed = false;
        registry::set_registry_url(&surface.url);
        // Persisted through the same file every other setting lives in. A write
        // failure is not fatal — the endpoint is already applied in memory —
        // but it is worth saying, because the next launch will not have it.
        if let Err(error) = storage::save_registry_endpoint(&registry::registry_url()) {
            eprintln!("[vela-wallet] endpoint could not be saved: {error}");
        }
        // Re-probe: `Event::Start` resets the health state, which is exactly
        // what pointing somewhere new should do.
        let pending = self.login.dispatch(vela_core::app::login::Event::Start);
        self.pump_login(pending, cx);
    }

    fn answer_choice(&mut self, index: Option<usize>, cx: &mut Context<Self>) {
        self.channel.answer_choice(index);
        self.pick = None;
        cx.notify();
    }

    fn answer_pin(&mut self, value: Option<String>, cx: &mut Context<Self>) {
        self.channel.answer_pin(value);
        self.pin = None;
        cx.notify();
    }

    fn t(&self, leaf: &str) -> SharedString {
        self.loc.t(&format!("onboarding.welcome.{leaf}"))
    }

    // -- welcome ------------------------------------------------------------

    /// The welcome, beside the rail. The brand is not here any more — it is in
    /// the rail, where it stays for the whole journey — so this is the hero,
    /// the line under it, and the two ways in.
    fn welcome(&self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        let mut top = div()
            .flex()
            .flex_col()
            .gap(px(GAP_HERO_SUB))
            .child({
                // The copy carries its own line break — every locale breaks
                // where its own sentence wants to, not where the measure
                // happens to run out — and it carries its own SIZE for the
                // same reason: the widest authored line is twice as wide in
                // French as in Chinese. An unrecognised value keeps the
                // design's size.
                let long = self.t("heroTitleFit").as_ref() == "long";
                let (size, leading) = if long {
                    (theme::text_hero_long(), theme::line_height_hero_long())
                } else {
                    (theme::text_hero(), theme::line_height_hero())
                };
                div()
                    .text_size(size)
                    .line_height(leading)
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(self.t("heroTitle"))
            })
            .child(
                div()
                    .text_size(theme::text_flow_sub())
                    .line_height(theme::line_height_flow_sub())
                    .text_color(theme.fg_muted)
                    .child(self.t("heroSubtitle")),
            );

        // The registry is unreachable. Sign-in stays attemptable — the CORE
        // decides that, not this screen — so this only says so, and offers the
        // surface that can fix it. It belongs to the copy block: hung off the
        // buttons it would float in the middle of the page.
        if self.login_view.transport_failed {
            // This machine could not get out — the person's network, not our
            // service. A different endpoint would not help, so no card and
            // no click (spec 038 SC-421).
            top = top.child(
                div()
                    .id("transport-warning")
                    .text_size(theme::text_flow_caption())
                    .line_height(theme::line_height_body())
                    .text_color(theme.warning_base)
                    .child(self.loc.t("onboarding.common.networkBody")),
            );
        } else if self.login_view.endpoint_unreachable {
            top = top.child(
                div()
                    .id("endpoint-warning")
                    .cursor_pointer()
                    .text_size(theme::text_flow_caption())
                    .line_height(theme::line_height_body())
                    .text_color(theme.warning_base)
                    .on_click(cx.listener(|this, _, _, cx| this.open_endpoint(false, cx)))
                    .child(self.loc.t("onboarding.settings.warningText")),
            );
        }

        let buttons = div()
            .flex()
            .flex_row()
            .gap(px(GAP_WELCOME_CTA))
            .child(welcome_cta(
                "create-wallet",
                ButtonVariant::Primary,
                self.t("createWallet"),
                !self.login_view.busy,
                theme,
                cx.listener(|this, _, _, cx| this.start_create(cx)),
            ))
            // Signing in has no screen of its own — the system passkey prompt
            // is the next thing the person sees, and it does not arrive in the
            // same frame as the press — so this button IS the progress
            // indicator for that wait.
            .child(welcome_cta_state(
                "already-have-wallet",
                ButtonVariant::Secondary,
                self.t("alreadyHaveWallet"),
                if self.login_view.busy {
                    ButtonState::Busy
                } else {
                    ButtonState::Enabled
                },
                theme,
                cx.listener(|this, _, _, cx| {
                    this.signin_methods_open = true;
                    cx.notify();
                }),
            ));

        let endpoint = self.endpoint_surface(theme, window, cx);
        // Over a signed-in wallet the welcome is a detour — a sign-in that
        // failed lands here — so it carries the way back to that wallet.
        let back = self.adding.then(|| {
            div()
                .id("adding-back")
                .pb(px(FLOW_GAP_LG))
                .cursor_pointer()
                .text_size(theme::text_flow_sub())
                .text_color(theme.fg_muted)
                .hover(|el| el.text_color(theme.fg_base))
                .on_click(cx.listener(|this, _, _, cx| this.leave_adding(cx)))
                .child(SharedString::from(format!(
                    "‹ {}",
                    self.loc.t("onboarding.common.back")
                )))
        });
        div()
            .w_full()
            .max_w(px(FLOW_COLUMN_W))
            .flex()
            .flex_col()
            .children(back)
            .child(top)
            .child(buttons.mt(px(GAP_HERO_CTA)))
            .children(endpoint)
    }

    // -- overlays -----------------------------------------------------------

    /// The three dialogs the CABLE raises, wrapped in this page's scrim.
    ///
    /// The cards themselves live in [`crate::hardware`] and know nothing about
    /// this screen — which is what lets the gallery render the real ones rather
    /// than a copy that can drift from them.
    fn touch_prompt(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let waiting = self.channel.touch_waiting()?;
        let on_cancel = cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            this.channel.cancel_touch();
            cx.notify();
        });
        Some(
            scrim(theme, "touch-scrim")
                .child(hardware::touch_card(theme, &self.loc, &waiting, on_cancel)),
        )
    }

    /// The caBLE QR, while a hybrid ceremony waits for the phone to scan it. It
    /// clears itself the moment the tunnel is up (the ceremony sets it to
    /// `None`), before the on-phone touch prompt takes its place.
    fn qr_prompt(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let payload = self.channel.qr_showing()?;
        let on_cancel = cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            this.channel.cancel_qr();
            cx.notify();
        });
        Some(
            scrim(theme, "qr-scrim")
                .child(hardware::qr_card(theme, &self.loc, &payload, on_cancel)),
        )
    }

    /// The sign-in method picker — the same three methods creating a wallet
    /// offers, so a wallet living on a phone or a security key is reachable, not
    /// just whatever a platform passkey would default to.
    fn signin_method_prompt(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        if !self.signin_methods_open {
            return None;
        }
        let on_pick: std::sync::Arc<dyn Fn(vela_core::app::KeyMethod, &mut Window, &mut App)> = {
            let page = cx.entity();
            std::sync::Arc::new(move |method, _window, cx| {
                page.update(cx, |page, cx| page.sign_in(method, cx));
            })
        };
        let card = hardware::signin_method_card(
            theme,
            &self.loc,
            &self.passkey_icons,
            on_pick,
            cx.listener(|this, _, _, cx| {
                this.signin_methods_open = false;
                // Opened from Settings to sign in to another account:
                // dismissing the methods is going back to that wallet.
                if this.adding && !this.login_view.busy {
                    this.leave_adding(cx);
                }
                cx.notify();
            }),
        );
        Some(scrim(theme, "signin-methods-scrim").child(card))
    }

    fn wallet_picker(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<Stateful<Div>> {
        let choices = self.pick.as_ref()?;
        let card = hardware::pick_card(
            theme,
            &self.loc,
            choices,
            {
                // `cx.listener` is not `Clone`, and the picker needs one
                // handler per row — so this is the weak-entity form by hand.
                let page = cx.entity();
                move |index: &usize, _window: &mut Window, cx: &mut App| {
                    let index = *index;
                    page.update(cx, |page, cx| page.answer_choice(Some(index), cx));
                }
            },
            cx.listener(|this, _, _, cx| this.answer_choice(None, cx)),
        );
        Some(scrim(theme, "wallet-pick-scrim").child(card))
    }

    fn pin_dialog(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let dialog = self.pin.as_ref()?;
        let page = cx.entity();
        let card = hardware::pin_card(
            theme,
            &self.loc,
            &dialog.request,
            &dialog.value,
            &dialog.focus,
            window,
            move |next: String, _window: &mut Window, cx: &mut App| {
                page.update(cx, |page, cx| {
                    if let Some(dialog) = page.pin.as_mut() {
                        dialog.value = next;
                        cx.notify();
                    }
                });
            },
            cx.listener(|this, _, _, cx| {
                let typed = this.pin.as_ref().map(|dialog| dialog.value.clone());
                this.answer_pin(typed.filter(|value| !value.is_empty()), cx);
            }),
            cx.listener(|this, _, _, cx| this.answer_pin(None, cx)),
        );
        Some(scrim(theme, "pin-scrim").child(card))
    }

    fn endpoint_surface(
        &self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<Stateful<Div>> {
        let surface = self.endpoint.as_ref()?;
        let strings = NameFieldStrings {
            label: self.loc.t("onboarding.settings.endpointUrlLabel"),
            placeholder: SharedString::from(registry::DEFAULT_REGISTRY_URL),
            helper: self.loc.t("onboarding.settings.passkeyHint"),
            too_long_hint: self.loc.t("onboarding.settings.warningText"),
        };

        let mut card = div()
            .w(px(SHEET_W))
            .flex()
            .flex_col()
            .gap(px(FLOW_GAP_LG))
            .p(px(SHEET_PAD))
            .rounded(px(SHEET_RADIUS))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border_card)
            .child(
                div()
                    .text_size(theme::text_flow_caption())
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.fg_muted)
                    .child(self.loc.t("onboarding.settings.sectionPasskeyIndex")),
            );

        if surface.automatic || self.login_view.endpoint_unreachable {
            card = card.child(
                div()
                    .text_size(theme::text_body())
                    .line_height(theme::line_height_body())
                    .text_color(theme.warning_base)
                    .child(self.loc.t("onboarding.settings.warningText")),
            );
        }

        card = card
            .child(text_field(
                "endpoint-field",
                theme,
                &strings,
                &surface.url,
                false,
                false,
                &surface.focus,
                window,
                {
                    let page = cx.entity();
                    move |next: String, _window: &mut Window, cx: &mut App| {
                        page.update(cx, |page, cx| {
                            if let Some(surface) = page.endpoint.as_mut() {
                                surface.url = next;
                                cx.notify();
                            }
                        });
                    }
                },
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(FLOW_GAP_MD))
                    .child(vela_button(
                        "endpoint-save",
                        ButtonVariant::Primary,
                        self.loc.t("onboarding.common.retry"),
                        theme,
                        cx.listener(|this, _, _, cx| this.save_endpoint(cx)),
                    ))
                    .child(vela_button(
                        "endpoint-reset",
                        ButtonVariant::Row,
                        self.loc.t("onboarding.settings.resetToDefault"),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            if let Some(surface) = this.endpoint.as_mut() {
                                surface.url = registry::DEFAULT_REGISTRY_URL.to_owned();
                                cx.notify();
                            }
                        }),
                    ))
                    .child(vela_button(
                        "endpoint-close",
                        ButtonVariant::Row,
                        self.loc.t("onboarding.common.close"),
                        theme,
                        cx.listener(|this, _, _, cx| {
                            this.endpoint = None;
                            this.endpoint_dismissed = true;
                            cx.notify();
                        }),
                    )),
            );

        // A card under the two ways in, not a scrim over them: the endpoint
        // is a setting, and a setting must never stand between a person and
        // the front door (spec 038 SC-420).
        Some(card.id("endpoint-card").mt(px(FLOW_GAP_LG)))
    }
}

/// The OS-level handle for this app's window.
///
/// Only Windows uses it: `WebAuthNAuthenticatorMakeCredential` needs somewhere
/// to hang its dialog, and giving it the wallet's real window is what makes
/// that dialog take focus and sit in the right place. gpui implements
/// `raw_window_handle`, so this is a read rather than the 1×1 helper window a
/// library without a window of its own has to invent.
pub(crate) fn native_window_handle(window: &Window) -> WindowHandle {
    #[cfg(windows)]
    {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // Called through the trait, not as a method: `Window` has an INHERENT
        // `window_handle()` returning gpui's own `AnyWindowHandle`, and an
        // inherent method always wins over a trait method of the same name, so
        // `window.window_handle()` would silently resolve to the wrong one.
        match HasWindowHandle::window_handle(window).map(|handle| handle.as_raw()) {
            Ok(RawWindowHandle::Win32(win32)) => isize::from(win32.hwnd),
            // Zero is what the API takes to mean "no parent". The dialog still
            // appears; it just may not take focus, which is a worse experience
            // than a crash would be honest about — so it is logged.
            _ => {
                eprintln!(
                    "[vela-wallet] no native window handle; the passkey dialog may not take focus"
                );
                0
            }
        }
    }
    #[cfg(not(windows))]
    {
        let _ = window;
        0
    }
}

/// The full-bleed dim behind a modal. It swallows clicks and does NOT dismiss:
/// everything shown over it is something someone is waiting on an answer to,
/// and a stray click outside a card is not an answer a person meant to give.
fn scrim(theme: &Theme, id: &'static str) -> Stateful<Div> {
    div()
        .id(id)
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.bg_base.opacity(crate::outcome::SCRIM_OPACITY))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
}

impl OnboardingPage {
    /// Ask the directory about every drafted key whose model this build cannot
    /// name, and about the marks those answers point at.
    ///
    /// Driven from render because that is where the key list exists, and
    /// guarded by `claim` so a redraw does not re-ask. Nothing here blocks the
    /// frame: the answer arrives later and notifies.
    fn ask_directory(&mut self, dark: bool, cx: &mut Context<Self>) {
        let size = theme::KEY_ROW_MARK as u32;
        for key in &self.create_view.keys {
            if !key.provider_name.is_empty() || key.aaguid.is_empty() {
                continue;
            }
            let aaguid = key.aaguid.clone();
            let id = format!("{}|{dark}", aaguid.to_ascii_lowercase());
            if self.directory.borrow_mut().claim(id) {
                let asked = aaguid.clone();
                cx.spawn(async move |page, cx| {
                    let holder = cx
                        .background_executor()
                        .spawn({
                            let asked = asked.clone();
                            async move { passkey_directory::fetch_holder(&asked, dark) }
                        })
                        .await;
                    page.update(cx, |page, cx| {
                        page.directory.borrow_mut().settle(&asked, dark, holder);
                        cx.notify();
                    })
                    .ok();
                })
                .detach();
            }

            let icon = self
                .directory
                .borrow()
                .holder(&aaguid, dark)
                .and_then(|holder| holder.icon_url.clone());
            let Some(url) = icon else { continue };
            if !self.directory.borrow_mut().claim_mark(&url, size) {
                continue;
            }
            cx.spawn(async move |page, cx| {
                let image = cx
                    .background_executor()
                    .spawn({
                        let url = url.clone();
                        async move { passkey_directory::fetch_mark(&url, size) }
                    })
                    .await;
                page.update(cx, |page, cx| {
                    page.directory.borrow_mut().settle_mark(&url, size, image);
                    cx.notify();
                })
                .ok();
            })
            .detach();
        }
    }
}

impl Render for OnboardingPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::of(self.mode);
        // A survived panic (spec 038): the ordinary failure sheet, "Something
        // went wrong", with the report behind the disclosure. Effect id 0
        // resolves nothing when the sheet closes — there is no effect.
        if self.prompt.is_none()
            && let Some(detail) = crate::panic_report::take()
        {
            self.prompt = Some((
                Machine::Login,
                Prompt::new(
                    vela_core::app::PromptKind::CreateFailed { detail },
                    false,
                    0,
                ),
            ));
        }
        let tiling = frame_tiling(window);

        // A modal takes the keyboard on the frame it appears, and only then:
        // re-focusing every frame would fight anything else the person clicks.
        for handle in [
            self.pin.as_ref().map(|dialog| dialog.focus.clone()),
            self.endpoint.as_ref().map(|surface| surface.focus.clone()),
        ]
        .into_iter()
        .flatten()
        {
            if !handle.is_focused(window) {
                handle.focus(window, cx);
            }
        }

        // Opened by the probe, once. A person who dismissed it has answered —
        // and stays answered until the index is asked again.
        if endpoint_should_auto_open(
            self.login_view.endpoint_unreachable && !self.login_view.transport_failed,
            self.endpoint.is_some(),
            self.endpoint_dismissed,
            self.creating,
        ) {
            self.open_endpoint(true, cx);
        }

        let content = if self.creating {
            self.ask_directory(theme.is_dark(), cx);
            let entity = cx.entity();
            let sink: FlowSink = Rc::new(move |event, window, cx| {
                entity.update(cx, |page, cx| page.on_flow_event(event, window, cx));
            });
            let host = FlowHost {
                theme: &theme,
                passkey_icons: &self.passkey_icons,
                loc: &self.loc,
                view: &self.create_view,
                name_focus: &self.name_focus,
                identicons: &self.identicons,
                directory: &self.directory,
                picker_open: self.picker_open,
                copied: self.copied,
                sink,
            };
            render_create_flow(&host, window)
        } else {
            if self.intro.is_some() {
                let entity = cx.entity();
                let sink: intro::IntroSink = Rc::new(move |event, window, cx| {
                    entity.update(cx, |page, cx| page.on_intro_event(event, window, cx));
                });
                // Two disjoint fields borrowed at once — the state mutably
                // (the art cache), the strings immutably.
                match self.intro.as_mut() {
                    Some(state) => intro::render_intro(state, &theme, &self.loc, sink),
                    None => self.welcome(&theme, window, cx),
                }
            } else {
                self.welcome(&theme, window, cx)
            }
        };

        // The rail's slot. Inside the journey it names the step; outside it —
        // the welcome, and DONE once the wallet exists — it carries the
        // product's line, so the sequence reads brand → 01 → 02 → 03 → brand.
        let slot = match self
            .creating
            .then(|| onboarding_flow::rail_step(&self.create_view))
        {
            Some(Some((ordinal, name, detail))) => RailSlot::Step {
                ordinal,
                total: FLOW_STEPS,
                name: self.loc.t(name),
                detail: self.loc.t(detail),
            },
            _ => RailSlot::Tagline(self.loc.t("onboarding.welcome.desktopTagline")),
        };
        let rail = onboarding_rail(&theme, slot);

        // The screen column: left-aligned beside the rail, at its natural
        // height. The welcome is the one screen that centres — it is a cover,
        // not a form, and it has no header row to hold a baseline steady.
        let column = div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .when(!self.creating, |el| el.justify_center())
            .px(px(CONTENT_PAD_X))
            .py(px(CONTENT_PAD_Y))
            .child(content);

        let content = div().size_full().flex().child(rail).child(column);

        let page = div()
            .size_full()
            .flex()
            .text_color(theme.fg_base)
            .child(content);

        // The animation centres itself on the CONTENT width; under client-side
        // decorations the viewport is wider than the window by the shadow band
        // on each untiled side.
        let mut viewport_w = f32::from(window.viewport_size().width);
        if let Some(tiling) = tiling {
            if !tiling.left {
                viewport_w -= f32::from(FRAME_SHADOW);
            }
            if !tiling.right {
                viewport_w -= f32::from(FRAME_SHADOW);
            }
        }
        let overlay = self
            .launch
            .as_mut()
            .and_then(|launch| launch.render(viewport_w, window, cx));
        let page_opacity = self.launch.as_ref().map_or(1., |l| l.page_opacity());

        let mut root = div()
            .size_full()
            .relative()
            .font_family(theme::font_ui())
            .bg(theme.bg_base)
            .child(page.opacity(page_opacity));

        // The three modals, in the order they can stack: a PIN request belongs
        // to a ceremony that a prompt has not been raised about yet, and the
        // endpoint surface can be opened FROM a prompt — so it goes on top.
        if let Some((_, prompt)) = &self.prompt {
            let entity = cx.entity();
            root = root.child(outcome_sheet(
                &theme,
                &self.loc,
                prompt,
                move |id, _window, cx| {
                    entity.update(cx, |page, cx| page.on_sheet_action(id, cx));
                },
            ));
        }
        if let Some(picker) = self.signin_method_prompt(&theme, cx) {
            root = root.child(picker);
        }
        if let Some(qr) = self.qr_prompt(&theme, cx) {
            root = root.child(qr);
        }
        if let Some(prompt) = self.touch_prompt(&theme, cx) {
            root = root.child(prompt);
        }
        if let Some(picker) = self.wallet_picker(&theme, cx) {
            root = root.child(picker);
        }
        if let Some(dialog) = self.pin_dialog(&theme, window, cx) {
            root = root.child(dialog);
        }

        let draws_titlebar = owns_titlebar(window);
        let root = match tiling {
            Some(tiling) => round_to_frame(root, tiling),
            None => root,
        };
        let root = root
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                let macos_chord = cfg!(target_os = "macos")
                    && ks.key == "f"
                    && ks.modifiers.control
                    && ks.modifiers.platform;
                if ks.key == "f11" || macos_chord {
                    window.toggle_fullscreen();
                }
                // Escape dismisses the QR, as it dismisses everything else that
                // covers a window.
                if ks.key == "escape" && this.channel.qr_showing().is_some() {
                    this.channel.cancel_qr();
                    cx.notify();
                    return;
                }
                if ks.key == "escape"
                    && this
                        .channel
                        .touch_waiting()
                        .is_some_and(|waiting| waiting.cancellable)
                {
                    this.channel.cancel_touch();
                    cx.notify();
                    return;
                }
                // The intro pages by keyboard too (spec 038 SC-416).
                if this.intro.is_some() {
                    let event = match ks.key.as_str() {
                        "right" => Some(intro::IntroEvent::Next),
                        "left" => Some(intro::IntroEvent::Prev),
                        _ => None,
                    };
                    if let Some(event) = event {
                        this.on_intro_event(event, window, cx);
                    }
                }
            }));
        let root = if draws_titlebar {
            root.child(titlebar(&theme, window, px(DRAG_STRIP_H)))
        } else {
            root
        };

        let root = match overlay {
            None => {
                // The animation has finished (or was never due). Record the
                // finish ONCE — this arm runs every frame afterwards.
                if self.launch.take().is_some() {
                    storage::write_epoch_ms(theme::LAUNCH_PLAYED_KEY, crate::executor::now_ms());
                }
                root
            }
            Some(overlay) => {
                let overlay = overlay.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _, _, cx| {
                        if let Some(launch) = this.launch.as_mut() {
                            launch.skip();
                        }
                        cx.notify();
                    }),
                );
                let overlay = match tiling {
                    Some(tiling) => round_to_frame(overlay, tiling),
                    None => overlay,
                };
                root.child(overlay)
            }
        };

        window_frame(root, &theme, window)
    }
}

/// Whether the probe's verdict should raise the endpoint card on this frame.
///
/// Pure, so the one rule that trapped the front door (spec 038 finding 6) is
/// a unit test rather than a render pass: a dismissed card stays dismissed
/// until a re-probe clears the flag, and the create journey never gets one.
fn endpoint_should_auto_open(
    unreachable: bool,
    open: bool,
    dismissed: bool,
    creating: bool,
) -> bool {
    unreachable && !open && !dismissed && !creating
}

#[cfg(test)]
mod endpoint_rule {
    use super::endpoint_should_auto_open;

    #[test]
    fn opens_once_for_an_unreachable_index() {
        assert!(endpoint_should_auto_open(true, false, false, false));
        assert!(!endpoint_should_auto_open(false, false, false, false));
        assert!(!endpoint_should_auto_open(true, true, false, false));
    }

    #[test]
    fn close_means_closed_until_the_index_is_asked_again() {
        // SC-419: the frame after Close must not re-open it …
        assert!(!endpoint_should_auto_open(true, false, true, false));
        // … and a re-probe (dismissed cleared) may.
        assert!(endpoint_should_auto_open(true, false, false, false));
    }

    #[test]
    fn never_during_the_create_journey() {
        assert!(!endpoint_should_auto_open(true, false, false, true));
    }
}
