//! Design tokens for the desktop app — the only module allowed to name a color
//! or a layout magic number (spec 007 FR-002).
//!
//! Palette values are sampled from the D1/D1L mocks by region-dominant-color
//! clustering (specs/007-desktop-onboarding-gpui/research.md D3); geometry from
//! the same mocks at their 1280×800 logical size (D5).

use gpui::{Hsla, Pixels, Window, px, rgb};

/// Which palette is active. Follows the OS appearance unless `VELA_THEME`
/// pins it (the hook SC-002's screenshot matrix drives).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ThemeMode {
    Light,
    Dark,
}

impl ThemeMode {
    /// `VELA_THEME` override, else the window's current system appearance.
    pub fn detect(window: &Window) -> Self {
        match std::env::var("VELA_THEME").as_deref() {
            Ok("light") => Self::Light,
            Ok("dark") => Self::Dark,
            _ => match window.appearance() {
                gpui::WindowAppearance::Dark | gpui::WindowAppearance::VibrantDark => Self::Dark,
                gpui::WindowAppearance::Light | gpui::WindowAppearance::VibrantLight => Self::Light,
            },
        }
    }

    /// Whether `VELA_THEME` pins the mode (appearance changes are then ignored).
    pub fn is_pinned() -> bool {
        matches!(
            std::env::var("VELA_THEME").as_deref(),
            Ok("light") | Ok("dark")
        )
    }
}

/// Semantic color tokens. Components read these; nobody reads hex.
pub struct Theme {
    /// True for the dark palette — see [`Theme::is_dark`].
    pub dark: bool,
    // backgrounds
    pub bg_base: Hsla,
    pub bg_raised: Hsla,
    pub bg_sunken: Hsla,
    /// The veil behind a modal. Ink at low alpha in BOTH palettes — the design
    /// system's `color.fixed.backdrop`, which the web already uses. Deriving it
    /// from `bg_base` instead makes it white-on-white in the light palette, so
    /// the dialog it is meant to lift off the page does not lift.
    pub backdrop: Hsla,
    // foreground ladder
    pub fg_base: Hsla,
    pub fg_muted: Hsla,
    pub fg_subtle: Hsla,
    pub fg_inverse: Hsla,
    /// The onboarding rail's surface. Light steps DOWN to the sunken tone and
    /// dark stays on the base — the same pair the wallet home's sidebar uses
    /// (spec 015 deviation 4: dark `bg_sunken` is LIGHTER than the canvas and
    /// would invert the hierarchy), so the two rails are visibly one app.
    pub rail_surface: Hsla,
    /// The onboarding rail's step ordinal and its `/03`. A WATERMARK on the
    /// rail's own surface — one step above the background and well below any
    /// text — so the number reads as a graphic the eye can rest on rather than
    /// as something to read.
    pub rail_ordinal: Hsla,
    pub rail_ordinal_soft: Hsla,
    // brand accent + interaction states
    pub accent: Hsla,
    pub accent_hover: Hsla,
    pub accent_active: Hsla,
    // structure
    pub border_card: Hsla,
    pub outline_strong: Hsla,
    pub divider: Hsla,
    /// The 1 px edge between content column and action panel.
    /// Dark mock draws none — the bg step is the separation — so dark uses
    /// `bg_raised` (painted but invisible), keeping one code path.
    pub panel_edge: Hsla,
    // logo (sails identical in both modes; hull themes — research D2)
    pub logo_sail_a: Hsla,
    pub logo_sail_b: Hsla,
    pub logo_hull: Hsla,
    // status colors — EXACT docs/design-tokens.json values
    // (`color-light`/`color-dark` → success/warning/error/info), so all four
    // platforms render identical badge/hint colors. Spec 014 (onboarding
    // flow) reads the `*_base`/`*_soft` set; spec 015 (wallet home) reads
    // `success`/`warning`/`warning_border`. Values overlap by design —
    // unifying the two vocabularies is a recorded follow-up, not a merge
    // decision.
    pub success_base: Hsla,
    pub success_soft: Hsla,
    pub warning_base: Hsla,
    pub warning_soft: Hsla,
    pub error_base: Hsla,
    pub error_soft: Hsla,
    pub info_base: Hsla,
    pub info_soft: Hsla,
    /// The flow patterns' "well" surface: input field, address strip, tech-
    /// details code block, dark secondary action rows, neutral badge fill.
    /// The dark mocks paint these DARKER than the raised panel (bg_sunken
    /// steps the wrong way on the dark ladder), so it is its own token —
    /// mock-sampled dark, `bg_sunken`-equivalent light.
    pub bg_well: Hsla,
    // spec 015 wallet-home aliases (same export values).
    pub success: Hsla,
    pub warning: Hsla,
    pub warning_border: Hsla,
}

fn c(hex: u32) -> Hsla {
    rgb(hex).into()
}

impl Theme {
    /// Which palette this is.
    ///
    /// Carried as a fact rather than inferred from a colour: artwork that is
    /// CHOSEN rather than tinted — a passkey provider's own logo, which ships a
    /// light and a dark cut — has to ask, and comparing a token against a
    /// palette constant would be a guess that breaks the day two palettes share
    /// a value.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        self.dark
    }

    pub fn of(mode: ThemeMode) -> Self {
        match mode {
            ThemeMode::Light => Self::light(),
            ThemeMode::Dark => Self::dark(),
        }
    }

    pub fn light() -> Self {
        Self {
            dark: false,
            bg_base: c(0xfafaf8),
            bg_raised: c(0xffffff),
            bg_sunken: c(0xf5f3ef),
            backdrop: c(0x000000).opacity(0.35),
            fg_base: c(0x1a1a18),
            fg_muted: c(0x6e6b62),
            fg_subtle: c(0x8c887e),
            fg_inverse: c(0xffffff),
            rail_surface: c(0xf5f3ef),
            rail_ordinal: c(0xe1dcd1),
            rail_ordinal_soft: c(0xd3cdc0),
            accent: c(0xe8572a),
            accent_hover: c(0xd14a20),
            accent_active: c(0xbf421c),
            border_card: c(0xecebe4),
            outline_strong: c(0x554b46),
            divider: c(0xecebe4),
            panel_edge: c(0xecebe4),
            logo_sail_a: c(0xff6a45),
            logo_sail_b: c(0xffa98e),
            logo_hull: c(0x554b46),
            success_base: c(0x2d8e5f),
            success_soft: c(0xedfaf2),
            warning_base: c(0x92600a),
            warning_soft: c(0xfff8f0),
            error_base: c(0xc62828),
            error_soft: c(0xfef2f2),
            info_base: c(0x4267f4),
            info_soft: c(0xedf0ff),
            bg_well: c(0xf5f3ef),
            success: c(0x2d8e5f),
            warning: c(0x92600a),
            warning_border: c(0xf0dcc8),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg_base: c(0x141412),
            bg_raised: c(0x1e1e1b),
            bg_sunken: c(0x262622),
            backdrop: c(0x000000).opacity(0.35),
            fg_base: c(0xe8e6e1),
            fg_muted: c(0x9a9790),
            fg_subtle: c(0x85827a),
            fg_inverse: c(0xffffff),
            rail_surface: c(0x141412),
            rail_ordinal: c(0x2e2e27),
            rail_ordinal_soft: c(0x3b3b33),
            accent: c(0xe8572a),
            accent_hover: c(0xf26a40),
            accent_active: c(0xd44d22),
            border_card: c(0x1e1e1b), // no visible card border in the dark mock
            outline_strong: c(0x554b46),
            divider: c(0x2c2c28),
            panel_edge: c(0x1e1e1b),
            logo_sail_a: c(0xff6a45),
            logo_sail_b: c(0xffa98e),
            logo_hull: c(0xded5ce),
            success_base: c(0x3da872),
            success_soft: c(0x132a1e),
            warning_base: c(0xd4a54a),
            warning_soft: c(0x2a2010),
            error_base: c(0xf87171),
            error_soft: c(0x2d1515),
            info_base: c(0x5a7cf6),
            info_soft: c(0x131b33),
            bg_well: c(0x121210),
            success: c(0x3da872),
            warning: c(0xd4a54a),
            warning_border: c(0x3d3020),
        }
    }
}

// ---------------------------------------------------------------------------
// Layout + type scale (mock-measured, research.md D5). 4 px grid except where
// the mock explicitly says otherwise (card column gap = 14).
// ---------------------------------------------------------------------------

/// Design window size; also the minimum — the card grid does not reflow below it.
pub const WINDOW_W: f32 = 1280.;
pub const WINDOW_H: f32 = 800.;

pub const LOGO_SIZE: f32 = 60.;

// ---------------------------------------------------------------------------
// Onboarding, v2 desktop (docs/design/onboarding-desktop-b.html).
//
// Two columns. A rail carries the brand and, during the create journey, which
// step you are on; the screen itself is a measure-width column beside it,
// LEFT-ALIGNED and at its natural height.
//
// That last part is the whole point. The single-column version stretched to
// the window (`flex: 1` + `space-between`), so its empty middle grew with
// every pixel of window height — which is what made a 1280×800 desktop read
// as a phone page pulled tall. A rail uses the width for orientation instead
// of padding, and content that ends where it ends does not gape.
// ---------------------------------------------------------------------------

/// The rail, sized and toned like the wallet home's sidebar so onboarding and
/// the app behind it are visibly the same program.
pub const RAIL_W: f32 = 320.;
pub const RAIL_PAD_X: f32 = 32.;
pub const RAIL_PAD_Y: f32 = 40.;
/// The measure of the rail's step detail and tagline — narrow on purpose, so
/// they wrap into a block rather than running the rail's full width.
pub const RAIL_TEXT_W: f32 = 214.;
/// The short accent rule under the tagline.
pub const RAIL_RULE_W: f32 = 28.;
pub const RAIL_RULE_H: f32 = 2.;

/// Padding of the screen column beside the rail.
pub const CONTENT_PAD_X: f32 = 72.;
pub const CONTENT_PAD_Y: f32 = 64.;

/// Mark ↔ wordmark, in the rail's brand row.
pub const GAP_LOGO_WORDMARK: f32 = 12.;
/// Hero ↔ subtitle.
pub const GAP_HERO_SUB: f32 = 12.;
/// Subtitle ↔ the CTA row.
pub const GAP_HERO_CTA: f32 = 32.;
/// Between the two welcome CTAs, which share one row.
pub const GAP_WELCOME_CTA: f32 = 12.;

/// `letter-spacing: .11em` on the wordmark, as a fraction of the em. gpui has
/// no letter-spacing property at all, so this is applied by hand — see
/// `ui::vela_wordmark`.
pub const WORDMARK_TRACKING: f32 = 0.11;

/// The welcome CTAs are rectangles with a 12px radius, and they sit side by
/// side at their labels' width — a desktop dialog sizes a button to its label;
/// a full-width button is a phone's answer to a thumb.
pub const RADIUS_CTA: f32 = 12.;
pub const CTA_MIN_W: f32 = 176.;
pub const CTA_PAD_X: f32 = 32.;
pub const CTA_H: f32 = 52.;

// ---------------------------------------------------------------------------
// Wallet home (spec 015). Geometry measured on the D1–D3 mocks at their
// 1280×800 logical size.
// ---------------------------------------------------------------------------

/// Fixed sidebar width (column 1).
pub const SIDEBAR_W: f32 = 240.;
/// Fixed third-column width (column 3 — the desktop bottom-sheet stand-in).
pub const THIRD_PANEL_W: f32 = 400.;
/// Inner padding of the sidebar.
pub const SIDEBAR_PAD: f32 = 16.;
/// Top padding of the sidebar header — clears the macOS traffic lights.
pub const SIDEBAR_TOP: f32 = 36.;
/// Content column padding.
pub const WALLET_PAD_X: f32 = 24.;
pub const WALLET_PAD_TOP: f32 = 28.;
/// Row heights / avatar sizes.
pub const WALLET_AVATAR: f32 = 40.;
pub const WALLET_ROW_ICON: f32 = 40.;
pub const WALLET_BADGE: f32 = 12.;
pub const WALLET_NAV_ROW_H: f32 = 40.;
pub const WALLET_CONTROL_H: f32 = 44.;
/// The money-in toast (D1b): the glyph disc, and how far below the top of the
/// window the pill floats. Measured against the phone's own banner — same
/// gesture, a desktop's distance from the eye.
pub const WALLET_TOAST_DISC: f32 = 28.;
pub const WALLET_TOAST_TOP: f32 = 16.;

// ---------------------------------------------------------------------------
// Contacts (spec 018). Geometry measured on the DC1–DC6/M1/M2 mocks at their
// 1280×800 logical size (research.md D9 — named metrics only, no new colors).
// ---------------------------------------------------------------------------

/// Groups rail (column between sidebar and the contact list), DC1.
pub const CONTACTS_RAIL_W: f32 = 216.;

/// Spec 023 desktop SPEC: the settings second-level nav column, measured 216
/// in DST1–DST8. Same width as the contacts group rail and for the same
/// reason — it is the same column, doing the same job, one section over.
pub const SETTINGS_NAV_W: f32 = 216.;

/// The same column, at the person's text size.
///
/// This column exists to hold labels and nothing else, so it grows with them.
/// Left at 216 it could not: at `xlarge` in German,
/// "Transaktionsgeschwindigkeit" ran out of the column, across the divider and
/// onto the network list behind it — measured, not imagined. Truncation in the
/// row is still the backstop; this is what stops it having to.
#[must_use]
pub fn settings_nav_w() -> f32 {
    (SETTINGS_NAV_W * crate::executor::appearance_prefs::text_factor()).round()
}

/// One row of that column.
pub const SETTINGS_NAV_ROW_H: f32 = 36.;

/// The panel's right-hand control column — measured 280 across DST2/DST3, so a
/// dropdown, a segmented control and a slider all end on the same line.
pub const SETTINGS_CONTROL_W: f32 = 280.;

/// The centred dialog DST4b and DSR1 draw — wide enough for a URL in the mono
/// face without becoming a second page.
pub const SETTINGS_DIALOG_W: f32 = 520.;

/// The panel's own content column. Measured on DST7: the storage bar runs
/// 505 -> 1146 inside the 1280 frame, so the content is 640 wide and starts
/// 48 past the nav column's right edge (240 + 216 + 48 = 504).
pub const SETTINGS_PANEL_W: f32 = 640.;

/// That 48. Left-aligned like the wallet's own content column, never centred:
/// centring parks the form in the middle of a wide window and opens a gap the
/// size of the nav column beside it (founder-found, 2026-09-02).
pub const SETTINGS_PANEL_PAD_X: f32 = 48.;
/// Rail row height (全部联系人 / group rows / 新建分组).
pub const CONTACTS_RAIL_ROW_H: f32 = 36.;
/// The `分组` caption block between the 全部联系人 row and the group rows.
pub const CONTACTS_RAIL_LABEL_H: f32 = 32.;
/// Inset from the header hairline down to the first rail row / list section.
pub const CONTACTS_BODY_PAD_TOP: f32 = 16.;
/// Header/CTA control height shared by 添加联系人, 群发转账 and the ⋯ buttons.
pub const CONTACTS_BUTTON_H: f32 = 40.;
/// Page-local search field in the contacts header (DC1: 780 → 1060).
pub const CONTACTS_SEARCH_W: f32 = 280.;
/// Dropdown/context menu card width and row height (M1/M2).
pub const CONTACTS_MENU_W: f32 = 220.;
pub const CONTACTS_MENU_ROW_H: f32 = 44.;
/// Contacts page header band (title + search + 添加联系人 + ⋯), DC1: the
/// hairline under it sits at y = 92 in the mock.
pub const CONTACTS_HEADER_H: f32 = 92.;
/// Height of the gallery chip strip: 28 px chips, 8 px padding either side,
/// and the 1 px hairline. Menus anchored in window coordinates offset by this
/// whenever the gallery chrome is on screen.
pub const GALLERY_BAR_H: f32 = 45.;
/// Contact detail hero avatar (desktop third-column size — measured 48 in DC2).
pub const CONTACTS_HERO_AVATAR: f32 = 48.;
/// Contact row leading avatar (row size, same as the wallet rows).
pub const CONTACTS_ROW_AVATAR: f32 = 40.;

/// Contacts motion contract (spec 018 FR-011): named here so all four
/// platforms share one set of values. The gpui build renders fixture states
/// statically, so production code doesn't consume them yet — the test at the
/// bottom of this file pins them against the cross-platform table.
#[allow(
    dead_code,
    reason = "cross-platform motion contract, asserted by tests"
)]
pub const CONTACTS_MOTION_PANEL_OPEN_MS: u64 = 240;
#[allow(
    dead_code,
    reason = "cross-platform motion contract, asserted by tests"
)]
pub const CONTACTS_MOTION_PANEL_CLOSE_MS: u64 = 200;
#[allow(
    dead_code,
    reason = "cross-platform motion contract, asserted by tests"
)]
pub const CONTACTS_MOTION_CROSSFADE_MS: u64 = 150;
#[allow(
    dead_code,
    reason = "cross-platform motion contract, asserted by tests"
)]
pub const CONTACTS_MOTION_HOVER_MS: u64 = 120;

/// One type size, through the person's text-size setting.
///
/// Every `text_*` below is mock-measured at 100%; this is the only place the
/// multiplier is applied, so a new size cannot forget it. Rounded to whole
/// pixels because a glyph drawn at 13.94px against a hairline measured in
/// whole pixels is how a row starts looking half a pixel wrong.
///
/// **What this does NOT scale** — and the reason the setting was worth wiring
/// carefully: every fixed `px` row height, button height and dialog width in
/// this shell stays put. At the larger stops the type grows inside boxes that
/// do not, which is exactly the class of bug issues 195/198 were. The settings
/// dialog was given a scrolling body for this reason; the rest is seen, not
/// assumed.
fn scaled(base: f32) -> Pixels {
    px((base * crate::executor::appearance_prefs::text_factor()).round())
}

/// Wallet type scale (mock-measured).
pub fn text_balance_hero() -> Pixels {
    scaled(40.)
}
pub fn text_balance_decimals() -> Pixels {
    scaled(24.)
}
pub fn text_row_title() -> Pixels {
    scaled(15.)
}
pub fn text_row_sub() -> Pixels {
    scaled(13.)
}
pub fn text_section() -> Pixels {
    scaled(17.)
}
pub fn text_label() -> Pixels {
    scaled(11.)
}
pub fn text_amount() -> Pixels {
    scaled(15.)
}
pub fn text_unit() -> Pixels {
    scaled(11.)
}
pub fn text_panel_title() -> Pixels {
    scaled(20.)
}
pub fn text_mono_address() -> Pixels {
    scaled(13.)
}

/// Monospace family for addresses. Menlo ships on macOS; the DejaVu face is
/// the broadly-present fallback on the Linux CI/dev images this app targets.
/// The UI face — Plus Jakarta Sans, the family the web declares in
/// `--font-ui` and iOS bundles in `DesignSystem/Fonts`. Named here and applied
/// at every page root (spec 038 finding 8): before this the desktop named a
/// family for MONO only, so all other text fell to gpui's default — the host's
/// system font — one typeface away from the product the web shows.
pub fn font_ui() -> &'static str {
    "Plus Jakarta Sans"
}

/// The four faces the app loads at startup (`main.rs`), from the repo's
/// shared `assets/fonts/`. TTF, because gpui's font database reads TTF/OTF
/// and not the woff2 the web ships. CJK and mono fall back to the host's
/// faces — recorded in `specs/038-first-run-parity/results.md`.
pub const UI_FONT_FILES: [&[u8]; 4] = [
    include_bytes!("../../../assets/fonts/PlusJakartaSans_400Regular.ttf"),
    include_bytes!("../../../assets/fonts/PlusJakartaSans_500Medium.ttf"),
    include_bytes!("../../../assets/fonts/PlusJakartaSans_600SemiBold.ttf"),
    include_bytes!("../../../assets/fonts/PlusJakartaSans_700Bold.ttf"),
];

pub fn font_mono() -> &'static str {
    if cfg!(target_os = "macos") {
        "Menlo"
    } else {
        "DejaVu Sans Mono"
    }
}

// ---------------------------------------------------------------------------
// Launch animation (spec 012). Every value here is shared verbatim with the
// iOS, Android and web apps — see specs/012-launch-animation-lottie/data-model.md
// §4. A transcription error in any one of the four languages is caught by the
// fit-rule test at the bottom of this file, which asserts research D1's table.
// ---------------------------------------------------------------------------

/// Authored length of the animation: 102 frames ÷ 60 fps.
///
/// Desktop reads the real duration off the player rather than trusting this, so
/// production never consumes it — but it is the value the other three apps
/// budget against, and `embedded_assets_load_and_carry_the_authored_timeline`
/// asserts the player agrees with it. Deleting it would remove the only place
/// the four platforms' shared assumption is checked against the actual asset.
#[allow(dead_code, reason = "cross-platform contract, asserted by tests")]
pub const LAUNCH_DURATION_MS: u64 = 1700;
/// How long the finished lockup is held before the hand-off, so the brand
/// registers instead of flashing past (founder direction, 2026-08-05 desktop
/// review; first tried at 2000 ms and cut to 400 on seeing it — long enough to
/// read as a beat, short enough not to feel like waiting).
///
/// This is the one place the feature deliberately spends the user's time. A tap,
/// click or key press skips it, and reduce-motion bypasses it entirely.
pub const LAUNCH_HOLD_MS: u64 = 400;

/// Cross-dissolve between the launch screen and Welcome. `motion.durationSlow` /
/// `motion.entranceFadeUp` — 180 ms (`sheetOut`) was tried first and reads as a
/// cut rather than a dissolve at this scale.
pub const LAUNCH_EXIT_CROSSFADE_MS: u64 = 400;
/// FR-014: nothing presented by now → abandon the animation, show Welcome.
pub const LAUNCH_FIRST_FRAME_BUDGET_MS: u64 = 400;
/// FR-015: measured from the first presented frame, not from construction.
/// Nominal is 1700 play + 400 hold + 400 dissolve = 2500; the rest is slack for
/// a slow machine.
pub const LAUNCH_HARD_CEILING_MS: u64 = 3000;

/// The large-screen composition's core canvas, 680 × 220 (research D1).
pub const LAUNCH_CANVAS_W: f32 = 680.;
pub const LAUNCH_CANVAS_H: f32 = 220.;

/// Box width as a fraction of viewport width — the core canvas divided by the
/// full-bleed canvas it was cropped from (680/1920). NOT a judgement call: at
/// 1920 px this renders the lockup at exactly the authored 29.5 % of screen
/// width, and `scripts/lint-lottie-assets.mjs` fails if a re-crop moves it.
pub const LAUNCH_BOX_W_RATIO: f32 = 680. / 1920.;

/// The desktop window minimum is 1280 × 800, so the form-factor predicate
/// (`width >= height || width >= 768`) is unconditionally large-screen here —
/// which is why production never evaluates it. Kept, and exercised by
/// `desktop_window_is_always_the_large_screen_form_factor`, because it is the
/// same threshold the other three apps branch on: if it ever changes, this is
/// where desktop finds out that its "always large-screen" shortcut still holds.
#[allow(dead_code, reason = "shared threshold, asserted by tests")]
pub const LAUNCH_LARGE_SCREEN_MIN_W: f32 = 768.;

/// Deterministic disable for tests and screenshots (FR-029).
///
/// Existing tests must not sit through the animation, and a sleep long enough
/// to outlast it is exactly the flaky waiting this replaces. Same shape as the
/// `VELA_THEME` override this app already has.
/// The launch animation replays after a week (spec 012's rule, spec 038's
/// desktop). Shared verbatim with the web's `app.html` pre-paint block and
/// `$lib/launch/constants.ts`; the test at the bottom of this file reads the
/// web's copy and asserts the number agrees. Storage key: `vela.launch.played`.
pub const LAUNCH_REPLAY_AFTER_MS: f64 = 604_800_000.;
pub const LAUNCH_PLAYED_KEY: &str = "vela.launch.played";
/// The intro plays once, ever (founder direction, 2026-09-01). Storage key
/// shared with the web.
pub const INTRO_SEEN_KEY: &str = "vela.intro.seen";

/// Is the launch animation due on this start?
pub fn launch_due(last_played_ms: Option<f64>, now_ms: f64) -> bool {
    match last_played_ms {
        None => true,
        Some(last) => !last.is_finite() || now_ms - last >= LAUNCH_REPLAY_AFTER_MS,
    }
}

pub fn launch_disabled() -> bool {
    std::env::var("VELA_SKIP_LAUNCH_ANIMATION").as_deref() == Ok("1")
}

/// Box size for a viewport, per the shared fit rule. Centred by the caller;
/// nothing is clipped or clamped, because the shipped asset is cropped to the
/// motion — the box *is* the artwork.
pub fn launch_box(viewport_w: f32) -> (f32, f32) {
    let w = viewport_w * LAUNCH_BOX_W_RATIO;
    (w, w * LAUNCH_CANVAS_H / LAUNCH_CANVAS_W)
}

pub const RADIUS_CARD: f32 = 16.;

/// The mocks size the two capsules differently: primary 52, secondary 48
/// (both mocks, review-verified to the pixel).
pub const BTN_H_PRIMARY: f32 = 52.;
pub const BTN_H_SECONDARY: f32 = 48.;
// Buttons hug long labels: heights above are minimums, and a wrapped label
// needs breathing room inside the capsule (spec 014 long-locale fix).
pub const BTN_PAD_X: f32 = 24.;
pub const BTN_PAD_Y: f32 = 10.;

// ---------------------------------------------------------------------------
// Onboarding create/login flow patterns (spec 014, docs/design/onboarding mocks).
// Geometry measured on the dark mocks at their ~1:1 panel width.
// ---------------------------------------------------------------------------

/// Outcome status badge: the circle behind the ✓/×/! glyph.
pub const BADGE_CIRCLE: f32 = 56.;
/// Stroke width of the small drawn glyphs (the copy icon, the badge's clock
/// face). Named for the elapsed ring it was measured on; the ring itself is
/// gone with spec 014's progress pattern, the weight it established is not.
pub const RING_STROKE: f32 = 4.;
/// The touch prompt's target disc. Sized like the outcome badge, because it
/// sits in the same place on the same card and is asking for the same amount
/// of attention.
pub const TOUCH_DISC: f32 = 56.;
/// Name field / address strip well height, and the wells' corner radius.
pub const INPUT_H: f32 = 52.;
pub const RADIUS_FIELD: f32 = 12.;
/// Acknowledgment checkbox square, and its corner radius.
pub const ACK_BOX: f32 = 22.;
pub const RADIUS_ACK: f32 = 6.;
/// Flow rhythm gaps (mock-measured: 8 within a group, 16 between rows,
/// 24 between pattern blocks).
pub const FLOW_GAP_SM: f32 = 8.;
pub const FLOW_GAP_MD: f32 = 16.;
pub const FLOW_GAP_LG: f32 = 24.;
/// The progress screen's block rhythm, which is looser than the rest.
pub const FLOW_GAP_XL: f32 = 28.;
/// The gap v2 uses inside a row — mark to sentence, icon to label. Between
/// `FLOW_GAP_SM` and `FLOW_GAP_MD`, and the design uses it everywhere a small
/// leading element sits beside text.
pub const FLOW_GAP_MD_SNUG: f32 = 12.;

// -- v2 flow shell (docs/design/onboarding-new) ----------------------------------

/// The measure of the screen column beside the rail — the SAME on every
/// onboarding screen now, welcome included. A maximum, not a width.
pub const FLOW_COLUMN_W: f32 = 520.;
pub const FLOW_HEADER_PAD_B: f32 = 28.;
/// The ‹ chevron sits closer to its label than the flow rhythm would put it.
pub const FLOW_BACK_GAP: f32 = 7.;
/// A key row is a bordered card, not a hairline-separated row.
pub const KEY_ROW_PAD_X: f32 = 14.;
/// The passkey provider's mark in a key row — the same optical weight as the
/// row's two lines of text stacked, so the logo anchors the row without
/// out-shouting the name.
pub const KEY_ROW_MARK: f32 = 28.;
pub const KEY_ROW_PAD_Y: f32 = 12.;
pub const KEY_ROW_GAP: f32 = 8.;
/// The tick beside the DONE title, and the identicon inside its card.
pub const DONE_CHECK: f32 = 34.;
pub const DONE_AVATAR: f32 = 44.;
/// Vertical padding of a progress task row, which is separated by a rule
/// rather than by a gap.
pub const TASK_ROW_PAD_Y: f32 = 11.;
/// Vertical padding of a DONE key row — the same rule pattern, tighter.
pub const DONE_ROW_PAD_Y: f32 = 10.;

/// Disabled control emphasis (mock A1's dimmed-accent CTA — never a gray fill).
pub const OPACITY_DISABLED: f32 = 0.45;
/// The task spinner's stroke — the emphasis border weight, which is what a
/// 16px arc needs to read as a ring rather than as a hair.
pub const SPINNER_STROKE: f32 = 2.;
/// The spinner a BUSY button turns in place of its label. A notch over the
/// 15px CTA text, so it occupies the label's optical weight rather than
/// looking like a dropped full stop.
pub const BTN_SPINNER: f32 = 18.;
/// Hairline rules (scaffold and outcome dividers).
pub const HAIRLINE: f32 = 1.;
/// The scaffold's close × hit target.
pub const FLOW_CLOSE_HIT: f32 = 32.;
/// Name-field caret width; also the stroke of the thin drawn glyphs.
pub const FLOW_CARET_W: f32 = 1.5;
/// Dev-only state gallery: fixture list column width.
pub const GALLERY_SIDEBAR_W: f32 = 280.;

/// Progress/outcome headline.
pub fn text_flow_headline() -> Pixels {
    scaled(17.)
}
/// Step counter / helper captions.
pub fn text_flow_caption() -> Pixels {
    scaled(12.)
}
/// The glyph inside the status badge circle.
pub fn text_badge_glyph() -> Pixels {
    scaled(26.)
}

/// The v2 wordmark is small, heavy and widely tracked — a label beside the
/// mark, not a title. v1's 42 px display treatment is gone with the two-column
/// welcome it belonged to.
pub fn text_wordmark() -> Pixels {
    scaled(19.)
}
/// The rail's step ordinal, set in the mono face at display size. It is
/// TYPOGRAPHY, not a widget: a stepper drawn as a control reads as chrome
/// bolted to the side of the page, which is exactly what it looked like.
pub fn text_step_ordinal() -> Pixels {
    scaled(104.)
}
pub fn line_height_step_ordinal() -> Pixels {
    px(104. * 0.82)
}
/// The `/03` that follows it.
pub fn text_step_total() -> Pixels {
    scaled(20.)
}
/// The step's name, under the ordinal.
pub fn text_step_name() -> Pixels {
    scaled(20.)
}
/// The rail's tagline, shown before the journey starts and after it ends.
///
/// 26, not the mock's 30: the mock hard-wrapped it after the comma, and the
/// string cannot carry that break — Android and iOS render the same key on
/// one line, and an iOS test asserts its exact value. At 26 the CJK taglines
/// fit the rail's 256px inner measure whole, and the longer latin ones wrap
/// into two lines instead of orphaning a glyph.
pub fn text_rail_tagline() -> Pixels {
    scaled(26.)
}
pub fn line_height_rail_tagline() -> Pixels {
    px(26. * 1.35)
}
/// The one sentence under a step's name.
pub fn line_height_rail_detail() -> Pixels {
    px(13. * 1.6)
}
/// The v2 welcome hero. It carries the screen, so it is nearly twice v1's
/// tagline; the copy ships its own line break rather than relying on a wrap.
pub fn text_hero() -> Pixels {
    scaled(46.)
}
/// `line-height: 1.25` at the hero size.
pub fn line_height_hero() -> Pixels {
    px(46. * 1.25)
}
/// One rung down the hero ladder (46/38/31), for a locale whose headline is too
/// wide for the first. The corpus says which, in `heroTitleFit` — the width is a
/// property of the translation, not of the client: measured at the shipped font
/// the widest authored line runs 6.9em (zh) to 12.8em (fr), and at 46 px the
/// widest of them overruns the 620 px column.
pub fn text_hero_long() -> Pixels {
    scaled(38.)
}
/// `line-height: 1.25` at the long-locale hero size.
pub fn line_height_hero_long() -> Pixels {
    px(38. * 1.25)
}
/// Flow-screen titles (spec 014). Not the welcome hero — that is `text_hero`.
pub fn text_tagline() -> Pixels {
    scaled(26.)
}
pub fn text_card_title() -> Pixels {
    scaled(16.)
}
pub fn text_body() -> Pixels {
    scaled(13.)
}
pub fn text_numeral() -> Pixels {
    scaled(12.)
}
/// Every v2 button label — welcome, flow, sheet — is this size and BOLD.
pub fn text_cta() -> Pixels {
    scaled(15.)
}
/// Flow subtitles and the name field's own text. One notch under
/// `text_card_title`, which the wallet home still uses at 16.
pub fn text_flow_sub() -> Pixels {
    scaled(15.)
}
/// `line-height: 1.5` at 15.
pub fn line_height_flow_sub() -> Pixels {
    px(22.5)
}
/// `line-height: 1.55` at 13 — the acknowledgement and hint sentences, which
/// wrap more than anything else on the screen.
pub fn line_height_ack() -> Pixels {
    px(13. * 1.55)
}
/// `line-height: 1.2` on the 26px flow titles.
pub fn line_height_title() -> Pixels {
    px(26. * 1.2)
}
/// The uppercase field/section label above an input or a list. Tiny, heavy and
/// tracked in the design; gpui cannot track, so the case and the weight carry
/// it (see `ui::vela_wordmark` for the one place hand-tracking was worth it).
pub fn text_section_label() -> Pixels {
    scaled(11.)
}
/// A key row's name, and a progress task's label.
pub fn text_row_name() -> Pixels {
    scaled(14.)
}
/// A key row's provider line, and the mono counters beside a section label.
pub fn text_row_meta() -> Pixels {
    scaled(12.)
}
/// Relaxed body line height (~1.55 at 13 px).
pub fn line_height_body() -> Pixels {
    px(20.)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WCAG 2.x relative luminance of an `Hsla` token.
    fn luminance(color: Hsla) -> f32 {
        let rgba = color.to_rgb();
        let lin = |c: f32| {
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(rgba.r) + 0.7152 * lin(rgba.g) + 0.0722 * lin(rgba.b)
    }

    fn contrast(a: Hsla, b: Hsla) -> f32 {
        let (la, lb) = (luminance(a), luminance(b));
        let (hi, lo) = if la > lb { (la, lb) } else { (lb, la) };
        (hi + 0.05) / (lo + 0.05)
    }

    /// SC-005: every text/background pair on the welcome screen, both themes.
    #[test]
    fn contrast_floor_holds_in_both_themes() {
        for (name, t) in [("light", Theme::light()), ("dark", Theme::dark())] {
            let body = [
                ("fg_base/bg_base", t.fg_base, t.bg_base, 4.5),
                ("fg_muted/bg_base", t.fg_muted, t.bg_base, 4.5),
                ("fg_muted/bg_raised", t.fg_muted, t.bg_raised, 4.5),
                // secondary CTA label — DV-001 is what makes dark pass
                ("fg_base/bg_raised", t.fg_base, t.bg_raised, 4.5),
                // 12 px numeral: decorative-adjacent, 3:1 floor
                ("fg_subtle/bg_raised", t.fg_subtle, t.bg_raised, 3.0),
                // primary CTA label: 16 px semibold on accent, large-text floor
                ("fg_inverse/accent", t.fg_inverse, t.accent, 3.0),
                ("fg_inverse/accent_hover", t.fg_inverse, t.accent_hover, 3.0),
                (
                    "fg_inverse/accent_active",
                    t.fg_inverse,
                    t.accent_active,
                    3.0,
                ),
                // spec 014 flow states: status glyphs/headlines are ≥17 px
                // semibold on the raised panel — large-glyph 3:1 floor.
                ("error_base/bg_raised", t.error_base, t.bg_raised, 3.0),
                ("success_base/bg_raised", t.success_base, t.bg_raised, 3.0),
                ("warning_base/bg_raised", t.warning_base, t.bg_raised, 3.0),
                ("info_base/bg_raised", t.info_base, t.bg_raised, 3.0),
                // text sits directly on the flow wells (address strip, code
                // block, action rows) — body floor for fg, large floor for the
                // error-colored code line.
                ("fg_base/bg_well", t.fg_base, t.bg_well, 4.5),
                ("error_base/bg_well", t.error_base, t.bg_well, 3.0),
                // spec 018 contacts: section letters + rail labels sit on
                // bg_base (11 px semibold, decorative-adjacent — 3:1 floor);
                // the detail-footer 删除联系人 is body text on bg_base.
                ("fg_subtle/bg_base", t.fg_subtle, t.bg_base, 3.0),
                ("error_base/bg_base", t.error_base, t.bg_base, 4.5),
                // the 家人 membership chip is fg_muted on the sunken pill.
                ("fg_muted/bg_sunken", t.fg_muted, t.bg_sunken, 4.5),
            ];
            for (pair, fg, bg, floor) in body {
                let ratio = contrast(fg, bg);
                assert!(ratio >= floor, "{name} {pair}: {ratio:.2} < {floor}");
            }
        }
    }

    /// Spec 018 FR-011: the contacts motion values are a four-platform
    /// contract (third column 240/200 ms, content crossfade 150 ms, hover
    /// 120 ms). Desktop renders fixture states statically, so this pin is the
    /// only executable place the shared numbers live.
    #[test]
    fn contacts_motion_contract_matches_the_spec_table() {
        assert_eq!(CONTACTS_MOTION_PANEL_OPEN_MS, 240);
        assert_eq!(CONTACTS_MOTION_PANEL_CLOSE_MS, 200);
        assert_eq!(CONTACTS_MOTION_CROSSFADE_MS, 150);
        assert_eq!(CONTACTS_MOTION_HOVER_MS, 120);
    }

    /// The accent is the brand constant and identical across modes (research D3).
    #[test]
    fn accent_is_mode_invariant() {
        assert_eq!(Theme::light().accent, Theme::dark().accent);
        assert_eq!(Theme::light().logo_sail_a, Theme::dark().logo_sail_a);
        assert_eq!(Theme::light().logo_sail_b, Theme::dark().logo_sail_b);
    }

    /// Spec 012 FR-011, research D1's table. The same five viewports are
    /// asserted in Swift, Kotlin and TypeScript; the point of repeating them in
    /// four languages is that a transcription slip in any one shows up here
    /// rather than on a user's screen.
    ///
    /// `LAUNCH_LOCKUP_RATIO` is a property of the shipped asset — the lockup is
    /// 566.73 of the 680-wide core canvas — and is verified against the file
    /// itself by `scripts/lint-lottie-assets.mjs --report`.
    #[test]
    fn launch_box_matches_the_authored_proportions() {
        const LAUNCH_LOCKUP_RATIO: f32 = 566.73 / LAUNCH_CANVAS_W;

        // (viewport width, expected box width, expected lockup width)
        let cases = [
            (768.0_f32, 272.0, 226.692),
            (1280.0, 453.333, 377.82),
            (1440.0, 510.0, 425.047),
            (1920.0, 680.0, 566.73),
            (3440.0, 1218.333, 1015.391),
        ];

        for (viewport, want_box_w, want_lockup) in cases {
            let (box_w, box_h) = launch_box(viewport);
            assert!(
                (box_w - want_box_w).abs() < 0.01,
                "viewport {viewport}: box width {box_w} != {want_box_w}"
            );
            assert!(
                (box_h - box_w * LAUNCH_CANVAS_H / LAUNCH_CANVAS_W).abs() < 0.001,
                "viewport {viewport}: box aspect is not the canvas aspect"
            );
            let lockup = box_w * LAUNCH_LOCKUP_RATIO;
            assert!(
                (lockup - want_lockup).abs() < 0.01,
                "viewport {viewport}: lockup {lockup} != {want_lockup}"
            );
            // The whole point of the rule: the lockup holds the authored share
            // of the viewport at every size, with no clamping at either end.
            assert!(
                (lockup / viewport - 0.2952).abs() < 0.001,
                "viewport {viewport}: lockup is {:.4} of the width, authored is 0.2952",
                lockup / viewport
            );
        }
    }

    /// At the authored 1920 width the box IS the core canvas, 1:1 — which is
    /// what makes `LAUNCH_BOX_W_RATIO` a derivation rather than a taste call.
    #[test]
    fn launch_box_is_one_to_one_at_the_authored_width() {
        let (w, h) = launch_box(1920.);
        assert!(
            (w - LAUNCH_CANVAS_W).abs() < 0.001,
            "box width {w} != {LAUNCH_CANVAS_W}"
        );
        assert!(
            (h - LAUNCH_CANVAS_H).abs() < 0.001,
            "box height {h} != {LAUNCH_CANVAS_H}"
        );
    }

    /// The desktop window can never be narrower than 1280, so the shared
    /// form-factor predicate always resolves to the large-screen composition.
    #[test]
    fn desktop_window_is_always_the_large_screen_form_factor() {
        // The shared predicate, applied to the window's MINIMUM size. Written as
        // a function call rather than a const comparison so it exercises the
        // same expression the other three platforms implement.
        let large = |w: f32, h: f32| w >= h || w >= LAUNCH_LARGE_SCREEN_MIN_W;
        assert!(
            large(WINDOW_W, WINDOW_H),
            "the minimum window must resolve large-screen"
        );
        assert!(
            !large(390., 844.),
            "a phone viewport must NOT resolve large-screen"
        );
        assert!(
            large(768., 1024.),
            "a tablet at the threshold must resolve large-screen"
        );
    }
}

#[cfg(test)]
mod fonts_bundled {
    use super::UI_FONT_FILES;

    /// SC-424: the faces travel with the app. Each bundled file is a TrueType
    /// (sfnt version 0x00010000) of a plausible size — a truncated or
    /// mis-copied file would otherwise fail silently at `add_fonts` and the
    /// app would render in the system face with nothing to say why.
    #[test]
    fn the_four_faces_are_truetype_files() {
        for (index, bytes) in UI_FONT_FILES.iter().enumerate() {
            assert_eq!(
                &bytes[..4],
                &[0x00, 0x01, 0x00, 0x00],
                "font {index}: not sfnt/TrueType"
            );
            assert!(
                bytes.len() > 50_000,
                "font {index}: {} bytes is not a whole face",
                bytes.len()
            );
        }
    }
}

#[cfg(test)]
mod launch_gate {
    use super::*;

    /// SC-422: the desktop's replay window is the web's number, read from the
    /// web's own pre-paint block so the two cannot drift.
    #[test]
    fn the_replay_window_is_the_webs() {
        let html = include_str!("../../../app-web/vela-wallet/src/app.html");
        assert!(
            html.contains("604800000"),
            "the web's app.html no longer names the replay window"
        );
        assert_eq!(LAUNCH_REPLAY_AFTER_MS, 604_800_000.);
        assert!(
            html.contains(LAUNCH_PLAYED_KEY),
            "storage key differs from the web"
        );
        assert!(
            html.contains(INTRO_SEEN_KEY),
            "intro key differs from the web"
        );
    }

    #[test]
    fn due_when_never_played_or_older_than_a_week() {
        let now = 1_800_000_000_000.;
        assert!(launch_due(None, now));
        assert!(launch_due(Some(now - LAUNCH_REPLAY_AFTER_MS), now));
        assert!(launch_due(Some(now - LAUNCH_REPLAY_AFTER_MS - 1.), now));
        assert!(!launch_due(Some(now - 1.), now));
        assert!(!launch_due(Some(now - LAUNCH_REPLAY_AFTER_MS + 1.), now));
        assert!(launch_due(Some(f64::NAN), now), "a corrupt flag plays it");
    }
}
