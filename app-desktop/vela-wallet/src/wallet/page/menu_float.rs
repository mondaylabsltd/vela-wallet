//! Spec 099: a menu that drops over the in-app page, kept visible WITH it.
//!
//! The page is a NATIVE view — WKWebView on macOS, WebView2 on Windows —
//! layered above gpui's canvas, so a menu gpui paints where the page is lies
//! under it: the tab menu's rows below the strip could be neither seen nor
//! clicked, and the site's own menus answered by taking the page off the
//! screen while they were open (the owner: the page "vanishes"). Now:
//!
//! - **macOS** draws the menu in a window of its own, placed where the menu
//!   would hang in the wallet's window, so the page stays under it. gpui's
//!   `WindowKind::AnchoredPopup` is the API made for this, but at the pinned
//!   rev macOS refuses it (`PopupNotSupportedError` — "Native popups are not
//!   implemented on macOS yet", `vendor/gpui_macos/src/platform.rs`), so it is
//!   `WindowKind::PopUp` — the same non-activating `NSPanel` at the pop-up
//!   level, without the anchoring — placed here, in the display's points. It
//!   never takes the keyboard: the wallet's window stays the active one, its
//!   traffic lights lit and its activation observers quiet, as under a
//!   system menu. A press anywhere else and Esc close it (`watch`), as do the
//!   wallet's window going to the background or moving (`page.rs`).
//! - **Windows** keeps spec 083 D2: the menu is drawn in the window and cuts
//!   its own hole in the webview's window (`webview::cut_out`).
//! - Where neither can be done — the float refused — the menu is drawn in
//!   the window and the page steps aside while it is open, as dialogs make
//!   it. Never a menu painted under the page.

use gpui::{Anchor, Bounds, Pixels, Point, Size, point, px, size};

use super::ContactsMenu;

/// How a menu that drops over the page is kept visible with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum OverPage {
    /// In a window of its own above the page (macOS): the page stays.
    Float,
    /// In the wallet's window, through a hole cut out of the page's (Windows,
    /// spec 083 D2): the page stays.
    CutOut,
    /// In the wallet's window, with the page off the screen while it is open.
    HidePage,
}

/// The menus that hang over the page: a tab's, from the strip above it, and
/// the site's network picker, which grows left from the Connection column
/// beside it. Every other menu opens where no page is.
pub(super) fn drops_over_page(kind: ContactsMenu) -> bool {
    matches!(kind, ContactsMenu::Tab | ContactsMenu::SiteNetwork)
}

/// The rule. `floats`: the platform draws a menu in a window of its own
/// (macOS); `cuts_out`: it can cut a hole in the page's window (Windows);
/// `refused`: the float was asked for and could not be opened.
pub(super) fn over_page(floats: bool, cuts_out: bool, refused: bool) -> OverPage {
    if floats && !refused {
        OverPage::Float
    } else if cuts_out {
        OverPage::CutOut
    } else {
        OverPage::HidePage
    }
}

/// [`over_page`] for the platform this was built for.
pub(super) fn over_page_here(refused: bool) -> OverPage {
    over_page(
        cfg!(target_os = "macos"),
        cfg!(target_os = "windows"),
        refused,
    )
}

/// What the float does next, given the menu it was opened (or refused) for
/// and the open menu that drops over the page, if any.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FloatStep {
    Keep,
    Close,
    /// Close whatever is up and open one for the wanted menu.
    Open,
}

/// One float per menu: the same menu keeps its float (or its refusal — a
/// refused menu is not asked for again every frame); another menu replaces
/// it; no menu closes it.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(super) fn float_step<K: PartialEq>(shown: Option<&K>, wanted: Option<&K>) -> FloatStep {
    match (shown, wanted) {
        (None, None) => FloatStep::Keep,
        (Some(_), None) => FloatStep::Close,
        (Some(shown), Some(wanted)) if shown == wanted => FloatStep::Keep,
        (_, Some(_)) => FloatStep::Open,
    }
}

/// Room around the card for its shadow, which the float's window has to hold
/// (`shadow_lg` reaches 22 below the card and 12 to its sides).
pub(super) const FLOAT_PAD: f32 = 24.;
/// The closest the card comes to the screen's edge — the in-window menu's
/// `snap_to_window_with_margin(8)`.
const FLOAT_EDGE: f32 = 8.;

/// Where the float's window goes, in its display's points (top-left origin —
/// the space `Window::bounds` reports and `WindowOptions::window_bounds`
/// takes on macOS).
///
/// `window` is the wallet's window on that display; `titlebar` how far its
/// content starts below the frame's top (none under the full-size content
/// view the wallet uses); `at` the press, in the content's coordinates;
/// `anchor` the card's corner that sits on it, as in the in-window menu;
/// `card` the card's size; `visible` the display's usable area. The card is
/// kept on the screen; the window is the card with [`FLOAT_PAD`] all round.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(super) fn float_bounds(
    window: Bounds<Pixels>,
    titlebar: Pixels,
    at: Point<Pixels>,
    anchor: Anchor,
    card: Size<Pixels>,
    visible: Bounds<Pixels>,
) -> Bounds<Pixels> {
    let f = f32::from;
    let (press_x, press_y) = (
        f(window.origin.x) + f(at.x),
        f(window.origin.y) + f(titlebar) + f(at.y),
    );
    let (w, h) = (f(card.width), f(card.height));
    let x = match anchor {
        Anchor::TopLeft | Anchor::BottomLeft | Anchor::LeftCenter => press_x,
        Anchor::TopRight | Anchor::BottomRight | Anchor::RightCenter => press_x - w,
        Anchor::TopCenter | Anchor::BottomCenter => press_x - w / 2.,
    };
    let y = match anchor {
        Anchor::TopLeft | Anchor::TopRight | Anchor::TopCenter => press_y,
        Anchor::BottomLeft | Anchor::BottomRight | Anchor::BottomCenter => press_y - h,
        Anchor::LeftCenter | Anchor::RightCenter => press_y - h / 2.,
    };
    // A card wider or taller than the screen keeps its top-left corner on it.
    let keep = |value: f32, low: f32, high: f32| value.min(high).max(low);
    let x = keep(
        x,
        f(visible.left()) + FLOAT_EDGE,
        f(visible.right()) - FLOAT_EDGE - w,
    );
    let y = keep(
        y,
        f(visible.top()) + FLOAT_EDGE,
        f(visible.bottom()) - FLOAT_EDGE - h,
    );
    Bounds {
        origin: point(px(x - FLOAT_PAD), px(y - FLOAT_PAD)),
        size: size(px(w + 2. * FLOAT_PAD), px(h + 2. * FLOAT_PAD)),
    }
}

/// What the float makes of a press or a key meant for this app while it is
/// open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Seen {
    /// Not the float's business.
    Pass,
    /// Close the menu, and let the event go on to what it was for — a press
    /// on another tab both closes the menu and picks the tab, as a press
    /// outside the in-window menu does.
    Dismiss,
    /// Close the menu, and nothing else hears it.
    DismissAndSwallow,
}

/// A press: in the float it is a row's (or the room around the card, which
/// the float closes on itself); anywhere else — the wallet's own drawing,
/// the page, the titlebar — it closes the menu.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(super) fn seen_press(in_float: bool) -> Seen {
    if in_float { Seen::Pass } else { Seen::Dismiss }
}

/// `kVK_Escape`.
const ESCAPE: u16 = 53;

/// A key: Esc closes the menu and goes no further — not into the page, not
/// into the address bar; every other key is somebody else's.
#[cfg_attr(not(any(target_os = "macos", test)), allow(dead_code))]
pub(super) fn seen_key(key_code: u16) -> Seen {
    if key_code == ESCAPE {
        Seen::DismissAndSwallow
    } else {
        Seen::Pass
    }
}

#[cfg(target_os = "macos")]
pub(super) use mac::MenuFloat;

/// No float off macOS: `menu_over_page` reads "never refused" and the
/// platform's own rule decides.
#[cfg(not(target_os = "macos"))]
#[derive(Default)]
pub(super) struct MenuFloat;

#[cfg(not(target_os = "macos"))]
impl MenuFloat {
    pub(super) fn refused(&self) -> bool {
        false
    }
}

#[cfg(target_os = "macos")]
mod mac {
    use futures::StreamExt as _;
    use futures::channel::mpsc::UnboundedSender;
    use gpui::{
        Anchor, AnyElement, AppContext as _, Bounds, Context, DisplayId, Entity, Hsla,
        InteractiveElement as _, IntoElement, ParentElement as _, Pixels, Point, Render,
        Styled as _, Subscription, WeakEntity, Window, WindowBackgroundAppearance, WindowBounds,
        WindowHandle, WindowKind, WindowOptions, div, px,
    };
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, Bool};
    use objc2::{class, msg_send, msg_send_id, sel};

    use super::super::{ContactsMenu, WalletPage};
    use super::{FLOAT_PAD, FloatStep, Seen, drops_over_page, float_bounds, float_step};
    use crate::theme::{self, Theme};

    /// The menu a float is for: `WalletPage::menu` as it was opened.
    type MenuKey = (ContactsMenu, Point<Pixels>, Anchor);

    /// The page's float: at most one, for one menu.
    #[derive(Default)]
    pub(in crate::wallet::page) struct MenuFloat {
        /// The menu it is open, opening, or refused for.
        key: Option<MenuKey>,
        /// Which float this is. A float that has been replaced may still
        /// report — its first frame, a dismissal already on its way — and
        /// is told apart by this.
        generation: u64,
        stage: Stage,
    }

    #[derive(Default)]
    enum Stage {
        #[default]
        Idle,
        /// Asked for; the window opens at the end of this frame.
        Opening,
        Open {
            window: WindowHandle<FloatView>,
            /// Removed with the float (`Drop`).
            watch: Option<Watch>,
        },
        /// The platform said no: the menu is drawn in the wallet's window.
        Refused,
    }

    impl MenuFloat {
        pub(in crate::wallet::page) fn refused(&self) -> bool {
            matches!(self.stage, Stage::Refused)
        }

        /// Up, or on its way up.
        fn floating(&self) -> bool {
            matches!(self.stage, Stage::Opening | Stage::Open { .. })
        }

        /// `generation` is the float that is up, or on its way up.
        fn is_current(&self, generation: u64) -> bool {
            generation == self.generation && self.floating()
        }

        fn begin(&mut self, key: MenuKey) -> u64 {
            self.generation += 1;
            self.key = Some(key);
            self.stage = Stage::Opening;
            self.generation
        }

        fn refuse(&mut self, generation: u64) {
            if generation == self.generation {
                self.stage = Stage::Refused;
            }
        }

        /// The window for `generation` is open. Handed back when the page
        /// has moved on in the meantime, for the caller to close.
        fn opened(
            &mut self,
            generation: u64,
            window: WindowHandle<FloatView>,
            watch: Option<Watch>,
        ) -> Result<(), (WindowHandle<FloatView>, Option<Watch>)> {
            if generation == self.generation && matches!(self.stage, Stage::Opening) {
                self.stage = Stage::Open { window, watch };
                Ok(())
            } else {
                Err((window, watch))
            }
        }

        /// Down: the watch goes now, the window at the end of this effect
        /// cycle — it may be the one whose event is being handled.
        fn close(&mut self, cx: &mut gpui::App) {
            self.key = None;
            if let Stage::Open { window, watch } = std::mem::take(&mut self.stage) {
                drop(watch);
                remove_later(window, cx);
            }
        }
    }

    fn remove_later(window: WindowHandle<FloatView>, cx: &mut gpui::App) {
        cx.defer(move |cx| {
            window
                .update(cx, |_, window, _| window.remove_window())
                .ok();
        });
    }

    impl WalletPage {
        /// Every frame: the open menu, if it drops over the page, has its
        /// float; anything else has none.
        pub(in crate::wallet::page) fn sync_menu_float(
            &mut self,
            window: &Window,
            cx: &mut Context<Self>,
        ) {
            let wanted = self.menu.filter(|(kind, _, _)| drops_over_page(*kind));
            match float_step(self.menu_float.key.as_ref(), wanted.as_ref()) {
                FloatStep::Keep => {}
                FloatStep::Close => self.menu_float.close(cx),
                FloatStep::Open => {
                    self.menu_float.close(cx);
                    let Some(key) = wanted else {
                        return;
                    };
                    let generation = self.menu_float.begin(key);
                    match self.float_place(key, window, cx) {
                        // Opened at the end of the effect cycle, with no
                        // entity on the stack: the window draws its first
                        // frame as it opens, and that frame asks this page
                        // for the card.
                        Some((display, bounds)) => {
                            let page = cx.entity();
                            cx.defer(move |cx| open(&page, generation, display, bounds, cx));
                        }
                        None => self.menu_float.refuse(generation),
                    }
                }
            }
        }

        /// Where the float for `key` goes, and on which display.
        fn float_place(
            &mut self,
            (kind, at, anchor): MenuKey,
            window: &Window,
            cx: &mut Context<Self>,
        ) -> Option<(DisplayId, Bounds<Pixels>)> {
            let display = window.display(cx)?;
            let frame = window.bounds();
            let titlebar = (frame.size.height - window.viewport_size().height).max(px(0.));
            let card = self.menu_card_size_of(kind, cx);
            Some((
                display.id(),
                float_bounds(frame, titlebar, at, anchor, card, display.visible_bounds()),
            ))
        }

        /// The card float `generation` draws — `None` once it is not the
        /// float on screen. With the ink its words are written in.
        fn float_card(
            &mut self,
            generation: u64,
            cx: &mut Context<Self>,
        ) -> Option<(AnyElement, Hsla)> {
            if !self.menu_float.is_current(generation) {
                return None;
            }
            let (kind, _, _) = self.menu?;
            let theme = Theme::of(self.theme_mode());
            let card = self.menu_card_of(kind, &theme, cx);
            Some((card.into_any_element(), theme.fg_base))
        }

        /// A press elsewhere or Esc, reported by float `generation`'s watch.
        /// Only the menu that float was opened for closes: a press on
        /// another tab may already have opened that tab's menu.
        fn float_dismissed(&mut self, generation: u64, cx: &mut Context<Self>) {
            if self.menu_float.is_current(generation) {
                self.dismiss_menu_float(cx);
            }
        }

        /// The floating menu closes, as an in-window one would have — now,
        /// not at the next frame, which a hidden window may not draw. Also
        /// when the wallet's window goes to the background, moves or
        /// resizes.
        pub(in crate::wallet::page) fn dismiss_menu_float(&mut self, cx: &mut Context<Self>) {
            if self.menu_float.floating() && self.menu == self.menu_float.key {
                self.menu = None;
                self.menu_float.close(cx);
                cx.notify();
            }
        }
    }

    /// Opens float `generation` for `page`, unless the page has moved on.
    fn open(
        page: &Entity<WalletPage>,
        generation: u64,
        display: DisplayId,
        bounds: Bounds<Pixels>,
        cx: &mut gpui::App,
    ) {
        if !page.read(cx).menu_float.is_current(generation) {
            return;
        }
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: None,
            // Never the keyboard: the wallet's window stays the active one.
            focus: false,
            show: true,
            kind: WindowKind::PopUp,
            is_movable: false,
            is_resizable: false,
            is_minimizable: false,
            display_id: Some(display),
            // The room around the card is its shadow's, and shows through.
            window_background: WindowBackgroundAppearance::Transparent,
            ..WindowOptions::default()
        };
        let opened = cx.open_window(options, |window, cx| {
            cx.new(|cx| FloatView::new(page, generation, window, cx))
        });
        let window = match opened {
            Ok(window) => window,
            Err(error) => {
                crate::diag::vlog!(
                    "menu float",
                    "refused ({error}): the menu is drawn in the window"
                );
                page.update(cx, |page, cx| {
                    page.menu_float.refuse(generation);
                    cx.notify();
                });
                return;
            }
        };
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let watch = window
            .update(cx, |_, window, _| ns_window(window))
            .ok()
            .flatten()
            .and_then(|panel| {
                as_menu_panel(panel);
                watch(panel, generation, tx)
            });
        page.update(cx, |page, cx| {
            if let Err((window, watch)) = page.menu_float.opened(generation, window, watch) {
                drop(watch);
                remove_later(window, cx);
                return;
            }
            // Ends when the watch is removed and AppKit lets its handlers go.
            cx.spawn(async move |page, cx| {
                while let Some(generation) = rx.next().await {
                    if page
                        .update(cx, |page, cx| page.float_dismissed(generation, cx))
                        .is_err()
                    {
                        break;
                    }
                }
            })
            .detach();
        });
    }

    /// The float's view: the page's card, drawn as it is in the window.
    pub(in crate::wallet::page) struct FloatView {
        page: WeakEntity<WalletPage>,
        generation: u64,
        _subscriptions: [Subscription; 2],
    }

    impl FloatView {
        fn new(
            page: &Entity<WalletPage>,
            generation: u64,
            window: &mut Window,
            cx: &mut Context<Self>,
        ) -> Self {
            Self {
                page: page.downgrade(),
                generation,
                _subscriptions: [
                    // The card follows the page: a balance landing in the
                    // network picker, the theme, the strings.
                    cx.observe(page, |_, _, cx| cx.notify()),
                    // No page, no menu (the window closed under it).
                    cx.observe_release_in(page, window, |_, _, window, _| {
                        window.remove_window();
                    }),
                ],
            }
        }
    }

    impl Render for FloatView {
        fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
            let generation = self.generation;
            let shown = self
                .page
                .update(cx, |page, cx| page.float_card(generation, cx))
                .ok()
                .flatten();
            let root = div().size_full();
            let Some((card, ink)) = shown else {
                return root;
            };
            let page = self.page.clone();
            root.flex()
                .items_start()
                .p(px(FLOAT_PAD))
                .font_family(theme::font_ui())
                .text_color(ink)
                // A press in the room around the card — its shadow — closes
                // the menu, as a press anywhere else does.
                .on_any_mouse_down(move |_, _, cx| {
                    page.update(cx, |page, cx| page.float_dismissed(generation, cx))
                        .ok();
                })
                .child(
                    div()
                        .on_any_mouse_down(|_, _, cx| cx.stop_propagation())
                        .child(card),
                )
        }
    }

    /// The float's `NSWindow`, from the content view gpui hands
    /// `raw-window-handle`.
    fn ns_window(window: &Window) -> Option<*mut AnyObject> {
        use wry::raw_window_handle::{HasWindowHandle, RawWindowHandle};
        // Through the trait: `Window` has an inherent `window_handle()` too.
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return None;
        };
        let view = appkit.ns_view.as_ptr().cast::<AnyObject>();
        // SAFETY: gpui's content view, alive as long as its window.
        let panel: *mut AnyObject = unsafe { msg_send![view, window] };
        (!panel.is_null()).then_some(panel)
    }

    /// The `NSPanel` gpui made for the float, made to behave as a menu: a
    /// press in it never makes it key (so the wallet's window keeps the
    /// keyboard, and its activation observers stay quiet); no titlebar
    /// buttons; no window shadow — the card draws its own, as in the window.
    fn as_menu_panel(panel: *mut AnyObject) {
        // SAFETY: a live NSWindow (`ns_window`); every message is NSWindow's
        // own, and the NSPanel one is asked for first.
        unsafe {
            let is_panel: Bool =
                msg_send![panel, respondsToSelector: sel!(setBecomesKeyOnlyIfNeeded:)];
            if is_panel.as_bool() {
                let _: () = msg_send![panel, setBecomesKeyOnlyIfNeeded: Bool::YES];
            }
            let _: () = msg_send![panel, setHasShadow: Bool::NO];
            // NSWindowCloseButton, NSWindowMiniaturizeButton, NSWindowZoomButton.
            for button in [0_usize, 1, 2] {
                let control: *mut AnyObject = msg_send![panel, standardWindowButton: button];
                if !control.is_null() {
                    let _: () = msg_send![control, setHidden: Bool::YES];
                }
            }
        }
    }

    /// The float's watch on presses and keys while it is open: AppKit event
    /// monitors, removed when it is dropped.
    pub(in crate::wallet::page) struct Watch(Vec<Retained<AnyObject>>);

    impl Drop for Watch {
        fn drop(&mut self) {
            for monitor in &self.0 {
                // SAFETY: a monitor `add{Local,Global}MonitorForEvents…`
                // returned, removed once.
                unsafe {
                    let _: () = msg_send![class!(NSEvent), removeMonitor: &**monitor];
                }
            }
        }
    }

    /// NSEventMaskLeftMouseDown | NSEventMaskRightMouseDown |
    /// NSEventMaskOtherMouseDown.
    const PRESSES: u64 = (1 << 1) | (1 << 3) | (1 << 25);
    /// NSEventMaskKeyDown.
    const KEYS: u64 = 1 << 10;

    type Handler = block2::Block<dyn Fn(*mut AnyObject) -> *mut AnyObject>;

    /// Watches presses and keys for float `generation`, whose window is
    /// `panel`: what `seen_press` / `seen_key` call a dismissal is sent down
    /// `tx`, and the page closes the menu.
    ///
    /// One monitor per kind of event, so neither handler asks an event what
    /// it is: `keyCode` raises on a mouse event. And one for presses in other
    /// apps: a tab right-clicked while this app was in the background opens
    /// its menu without bringing the app forward, and a press elsewhere then
    /// never reaches this app's own monitors (mouse events need no
    /// permission to be watched; keys would, and are not).
    fn watch(panel: *mut AnyObject, generation: u64, tx: UnboundedSender<u64>) -> Option<Watch> {
        let float = panel as usize;
        let elsewhere = {
            let tx = tx.clone();
            block2::RcBlock::new(move |_event: *mut AnyObject| {
                let _ = tx.unbounded_send(generation);
            })
        };
        let presses = {
            let tx = tx.clone();
            block2::RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
                // SAFETY: the event AppKit is about to send; its window may
                // be nil and is only compared.
                let window: *mut AnyObject = unsafe { msg_send![event, window] };
                respond(
                    super::seen_press(window as usize == float),
                    event,
                    generation,
                    &tx,
                )
            })
        };
        let keys = block2::RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
            // SAFETY: a key event — the mask admits nothing else.
            let code: u16 = unsafe { msg_send![event, keyCode] };
            respond(super::seen_key(code), event, generation, &tx)
        });
        let add = |mask: u64, handler: &Handler| -> Option<Retained<AnyObject>> {
            // SAFETY: AppKit copies the handler and keeps it until the
            // monitor is removed (`Watch::drop`).
            unsafe {
                msg_send_id![
                    class!(NSEvent),
                    addLocalMonitorForEventsMatchingMask: mask,
                    handler: handler
                ]
            }
        };
        // SAFETY: as `add`; the handler returns nothing, as a global one must.
        let global: Option<Retained<AnyObject>> = unsafe {
            msg_send_id![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: PRESSES,
                handler: &*elsewhere
            ]
        };
        let monitors: Vec<_> = [add(PRESSES, &presses), add(KEYS, &keys), global]
            .into_iter()
            .flatten()
            .collect();
        (!monitors.is_empty()).then_some(Watch(monitors))
    }

    fn respond(
        seen: Seen,
        event: *mut AnyObject,
        generation: u64,
        tx: &UnboundedSender<u64>,
    ) -> *mut AnyObject {
        match seen {
            Seen::Pass => event,
            Seen::Dismiss => {
                let _ = tx.unbounded_send(generation);
                event
            }
            Seen::DismissAndSwallow => {
                let _ = tx.unbounded_send(generation);
                std::ptr::null_mut()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 099: the menus over the page — a tab's, the network picker —
    /// never hide it where a float or a hole can be had, and do hide it
    /// where neither can (no float on this platform, or the float refused):
    /// never a menu painted under the page.
    #[test]
    fn a_menu_over_the_page_keeps_it_where_it_can() {
        // macOS.
        assert_eq!(over_page(true, false, false), OverPage::Float);
        assert_eq!(over_page(true, false, true), OverPage::HidePage, "refused");
        // Windows (spec 083 D2's hole).
        assert_eq!(over_page(false, true, false), OverPage::CutOut);
        // No popups and no hole: the page steps aside.
        assert_eq!(over_page(false, false, false), OverPage::HidePage);
        assert_eq!(over_page(false, false, true), OverPage::HidePage);
        // This build.
        assert_eq!(
            over_page_here(false),
            if cfg!(target_os = "macos") {
                OverPage::Float
            } else if cfg!(target_os = "windows") {
                OverPage::CutOut
            } else {
                OverPage::HidePage
            }
        );
    }

    /// Which menus those are: the two that hang over the page. The rest
    /// open over the start page, the contacts or the settings.
    #[test]
    fn only_the_tab_menu_and_the_network_picker_drop_over_the_page() {
        assert!(drops_over_page(ContactsMenu::Tab));
        assert!(drops_over_page(ContactsMenu::SiteNetwork));
        for kind in [
            ContactsMenu::Header,
            ContactsMenu::Group,
            ContactsMenu::Tile,
            ContactsMenu::Recent,
            ContactsMenu::Contact,
            ContactsMenu::MoveGroup,
        ] {
            assert!(!drops_over_page(kind), "{kind:?}");
        }
    }

    /// One float per menu: opened once, kept while the menu is, replaced by
    /// another tab's menu, closed with the menu — and a refused menu is not
    /// asked for again every frame.
    #[test]
    fn one_float_per_menu() {
        let (a, b) = (1, 2);
        assert_eq!(float_step::<i32>(None, None), FloatStep::Keep);
        assert_eq!(float_step(None, Some(&a)), FloatStep::Open);
        assert_eq!(float_step(Some(&a), Some(&a)), FloatStep::Keep);
        assert_eq!(float_step(Some(&a), Some(&b)), FloatStep::Open);
        assert_eq!(float_step(Some(&a), None), FloatStep::Close);
    }

    fn bounds(x: f32, y: f32, w: f32, h: f32) -> Bounds<Pixels> {
        Bounds {
            origin: point(px(x), px(y)),
            size: size(px(w), px(h)),
        }
    }

    /// The float hangs where the in-window menu would: the anchor corner on
    /// the press, in the display's points — the window's place plus the
    /// press — with the shadow's room around it.
    #[test]
    fn the_float_hangs_where_the_menu_would() {
        let screen = bounds(0., 25., 1_440., 875.);
        let window = bounds(100., 80., 1_280., 800.);
        let card = size(px(220.), px(199.));
        // A tab's menu: its top-left at the press.
        let float = float_bounds(
            window,
            px(0.),
            point(px(300.), px(20.)),
            Anchor::TopLeft,
            card,
            screen,
        );
        assert_eq!(
            float,
            bounds(400. - FLOAT_PAD, 100. - FLOAT_PAD, 268., 247.)
        );
        // The network picker: its top-right at the press.
        let float = float_bounds(
            window,
            px(0.),
            point(px(1_200.), px(200.)),
            Anchor::TopRight,
            size(px(300.), px(146.)),
            screen,
        );
        assert_eq!(
            float.origin,
            point(px(1_000. - FLOAT_PAD), px(280. - FLOAT_PAD))
        );
        // A window with a titlebar above its content: below it.
        let float = float_bounds(
            window,
            px(28.),
            point(px(300.), px(20.)),
            Anchor::TopLeft,
            card,
            screen,
        );
        assert_eq!(float.origin.y, px(128. - FLOAT_PAD));
    }

    /// Near the screen's edge the card stays on the screen, 8 points in, as
    /// the in-window menu stays in the window.
    #[test]
    fn the_float_stays_on_the_screen() {
        let screen = bounds(0., 25., 1_440., 875.);
        let window = bounds(200., 100., 1_280., 800.);
        let card = size(px(220.), px(199.));
        let float = float_bounds(
            window,
            px(0.),
            point(px(1_200.), px(760.)),
            Anchor::TopLeft,
            card,
            screen,
        );
        assert_eq!(
            float.origin,
            point(
                px(1_440. - 8. - 220. - FLOAT_PAD),
                px(900. - 8. - 199. - FLOAT_PAD)
            )
        );
        // Left of the screen's left edge: on it.
        let float = float_bounds(
            bounds(-50., 100., 1_280., 800.),
            px(0.),
            point(px(10.), px(10.)),
            Anchor::TopRight,
            card,
            screen,
        );
        assert_eq!(float.origin.x, px(8. - FLOAT_PAD));
    }

    /// The float's window is sized before anything is laid out in it, so it
    /// is the card's own measure: four rows, the divider after the first.
    #[test]
    fn the_tab_menu_card_is_sized_by_its_rows() {
        let strings = crate::explore::ExploreStrings::resolve(&crate::loc::Loc::from_env());
        let menu = crate::explore::fixtures::tab_menu(&strings);
        assert_eq!(
            crate::contacts::components::menu_card_size(&menu),
            size(px(220.), px(4. * 44. + 9. + 14.))
        );
        let plain = crate::contacts::fixtures::MenuModel {
            items: menu.items.clone(),
            divider_after: None,
        };
        assert_eq!(
            crate::contacts::components::menu_card_size(&plain),
            size(px(220.), px(4. * 44. + 14.))
        );
    }

    /// A press anywhere but the float closes the menu and still lands; Esc
    /// closes it and lands nowhere; other keys are not the float's.
    #[test]
    fn a_press_elsewhere_or_esc_closes_the_float() {
        assert_eq!(seen_press(false), Seen::Dismiss);
        assert_eq!(seen_press(true), Seen::Pass);
        assert_eq!(seen_key(53), Seen::DismissAndSwallow);
        assert_eq!(seen_key(36), Seen::Pass, "return");
        assert_eq!(seen_key(0), Seen::Pass, "a");
    }
}
