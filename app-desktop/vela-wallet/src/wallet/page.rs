//! The wallet page entity (spec 015 US2): sidebar + content + closable third
//! column, plus the gallery chrome that exposes every state (FR-004).
//!
//! One entity serves both `VELA_PAGE=wallet` (D1 default, panels open on
//! interaction) and `VELA_PAGE=gallery` (adds the state-switcher strip,
//! component boards and the identicon board).
//!
//! Spec 018 reuses this shell for 通讯录 rather than building a second page:
//! the sidebar, third column, Esc handling and gallery chrome already exist,
//! so contacts is a `Section` switch on the content column (research.md D1).

use std::sync::Arc;

use gpui::AnimationExt as _;
use gpui::AppContext as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Anchor, Context, Div, ElementId, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, MouseButton, MouseDownEvent, ParentElement, Pixels, Point, Render, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled, Window, anchored, deferred, div, point, px,
};

use crate::contacts::ContactsStrings;
use crate::contacts::components as contacts_components;
use crate::contacts::components::{
    RailState, accent_button, add_chip, address_block, contact_row, destructive_text_button,
    empty_state_cta, ghost_add_row, group_chip, icon_button, menu_card, outline_button, rail_label,
    rail_row, row_divider, search_field, section_letter, text_action,
};
use crate::contacts::fixtures as contacts_fixtures;
use crate::contacts::live as contacts_live;
use crate::contacts::model::ContactRowModel;
use crate::explore::ExploreStrings;
use crate::explore::components as explore_components;
use crate::explore::fixtures::{self as explore_fixtures, GroupAction};
use crate::explore::live as explore_live;
use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::loc::Loc;
use crate::resident;
use crate::session;
use crate::settings::SettingsStrings;

/// How many focus handles the endpoints panel claims before the providers
/// panel starts. Four fields, one per service.
const ENDPOINT_FOCUS_COUNT: usize = 4;

/// Focus slots past the endpoints and the three providers, so no two editable
/// settings fields share a handle — focus is which box the keystrokes go into.
const WIZARD_SEARCH_FOCUS: usize = ENDPOINT_FOCUS_COUNT + 3;
const WIZARD_RPC_FOCUS: usize = WIZARD_SEARCH_FOCUS + 1;
/// Two handles per network card, past everything above.
const OVERRIDE_FOCUS_BASE: usize = WIZARD_RPC_FOCUS + 1;
use crate::executor::dapp_browser::{Forwarded, SigningOrder};

use crate::executor::display_currency;
use crate::executor::format_prefs;
use crate::executor::passkey::WindowHandle;
use crate::hardware;
use crate::settings::components::{
    CalloutTone, ConfirmCopy, callout, chain_logo_mark, chain_mark, check_list, confirm_sheet,
    danger_card, dropdown_menu, dropdown_menu_choices, dropdown_menu_picks, dropdown_trigger,
    editable_url_field, form_row, key_value_row, network_row, rpc_banner, segmented,
    segmented_picks, settings_nav_row, status_pill, storage_bar, storage_group, storage_group_with,
    text_scale, text_scale_picks, url_field,
};
use crate::settings::fixtures::{self as settings_fixtures, SettingsPage, Tone, latency, pill};
use crate::settings::live as settings_live;
use crate::settings::model::NetworkRowModel;
use crate::signing::SigningStrings;
use crate::signing::components as signing_components;
use crate::signing::fixtures as signing_fixtures;
use crate::signing::live as signing_live;
use crate::signing::trusted_signer as signing_trusted_signer;
use crate::theme::{
    self, CONTACTS_BODY_PAD_TOP, CONTACTS_BUTTON_H, CONTACTS_HEADER_H, CONTACTS_HERO_AVATAR,
    CONTACTS_RAIL_LABEL_H, CONTACTS_RAIL_ROW_H, CONTACTS_RAIL_W, GALLERY_BAR_H,
    SETTINGS_PANEL_PAD_X, SETTINGS_PANEL_W, SIDEBAR_PAD, SIDEBAR_TOP, SIDEBAR_W, THIRD_PANEL_W,
    Theme, ThemeMode, WALLET_CONTENT_MAX_W, WALLET_PAD_TOP, WALLET_PAD_X, WALLET_ROW_MEASURE,
};
use crate::wallet::browser_host::{BROWSER_TAB, BrowserHost};
use crate::wallet::live as wallet_live;
use crate::wallet::money::{self, SendHost};
use crate::window_frame::{
    CAPTION_H, frame_tiling, owns_titlebar, round_to_frame, titlebar, window_frame,
};
use vela_core::app::activity_feed::ActivityFeed;
use vela_core::app::balance_dashboard::BalanceDashboard;
use vela_core::app::batch_import::{BatchUnit, Event as BatchEvent};
use vela_core::app::browser_history::BrowserHistory;
use vela_core::app::contacts::{
    ContactExportScope, ContactFileFormat, ContactGroupInput, ContactSaveInput, Contacts,
    Event as ContactEvent,
};
use vela_core::app::dapp_browser::Event as DbrEvent;
use vela_core::app::display_currency::{DisplayCurrency, Event as CurrencyEvent};
use vela_core::app::explore_sites::ExploreSites;
use vela_core::app::fee_policy::Event as FeeEvent;
use vela_core::app::manage_tokens::{Event as MtokEvent, ManageTokens, MtokNetwork};
use vela_core::app::network_admin::{Event as NetEvent, NetOverrideField, NetworkAdmin};
use vela_core::app::payment_request::PaymentRequest;
use vela_core::app::receive_watch::ReceiveWatch;
use vela_core::app::send::{
    Event as SendEvent, SendAlertKind, SendDisplayContext, SendOpenParams, SendRecipientDraft,
};
use vela_core::app::sign_request::{SignErrorKind, SignResponsePayload};

use super::WalletStrings;
use super::components::{
    action_pill, activity_row, asset_row, balance_display, chain_row, empty_state, icon_img,
    identicon_avatar, nav_row, qr_placeholder, section_header, section_header_parts,
    section_header_row, skeleton_row, token_icon, token_icon_logos, wallet_header,
};
use super::fixtures::{self, ADDRESS_FULL, IDENTICON_BOARD_SEEDS, WALLET_NAME};
use crate::flows::{
    FlowEntry, FlowPanel, FlowStep, FlowStrings, fixtures as flow_fixtures, live as flows_live,
    panels,
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PanelId {
    None,
    Receive,
    AssetDetail,
    /// Spec 018 DC2 — the contacts third-column content.
    ContactDetail,
    /// Spec 022 DE3 — what a connected site can and cannot do.
    Connection,
    /// Spec 022 DE4 / DCS1–8 — a signing request, beside the page that raised
    /// it. That adjacency is the desktop's own anti-phishing advantage: the
    /// request and the site making it can be read against each other without
    /// dismissing either.
    Signing,
    /// Spec 021 — whatever is on top of `flows`. The stack is the state; this
    /// variant only says the column belongs to it.
    Flow,
}

/// What the contacts header row adds on top so its search field, buttons and
/// ⋯ start below the drag strip. The row is centred inside
/// `CONTACTS_HEADER_H`, so padding moves the group by only half — 16 buys the
/// ~7px of clearance the 34px strip needs, with a little room to spare.
const CONTACTS_HEADER_CAPTION_PAD: f32 = 16.;

/// A switcher row's artwork (`size="row"`, `--icon-2xl`).
const SWITCHER_IDENTICON: f32 = 30.;
/// The identicon viewer's artwork (`--size-identiconViewer`): big enough to
/// read as a picture rather than an avatar.
const VIEWER_IDENTICON: f32 = 160.;

/// What the gallery chip strip adds on top for the same reason. It is not
/// centred, so this is the clearance itself, less the 8 the bar already had.
fn gallery_bar_caption_pad(caption: bool) -> f32 {
    if caption { CAPTION_H + 4. - 8. } else { 0. }
}

/// Which destination the content column renders. The sidebar's selected nav
/// row derives from this (spec 018 research.md D1).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Section {
    Wallet,
    Contacts,
    /// Spec 022 — the browser. Same shell, third body.
    Explore,
    /// Spec 023: the settings section — a second-level nav plus one panel,
    /// hosted in the same three-column shell contacts already reuses.
    Settings,
}

impl Section {
    /// Whether THIS build has the destination — the web's rule
    /// (`app-web/.../wallet/destinations.ts`), for the same reason.
    ///
    /// Linux has no in-app dApp browser (owner call, 2026-09-24). gpui runs as
    /// a native Wayland client, and Wayland lets no program place another's
    /// page inside its window, so the site could only open in a window of its
    /// own — built, and parked on the local branch `linux-dapp-browser-wip`,
    /// because a connect or a signature then had to be answered back in this
    /// one. Three destinations, rather than a fourth that opens a picture.
    pub const fn available(self) -> bool {
        !(cfg!(target_os = "linux") && matches!(self, Section::Explore))
    }

    /// A place this build cannot show — a pinned or restored Explore on
    /// Linux — opens the wallet instead.
    #[must_use]
    pub const fn or_wallet(self) -> Self {
        if self.available() {
            self
        } else {
            Section::Wallet
        }
    }
}

/// The two anchored menus DC5/DC6 define. Both render through one
/// `menu_card` — the difference is which fixture feeds it and where it hangs.
/// The two centred dialogs the settings section can raise (spec 023).
///
/// The desktop SPEC's rule is that every phone 弹框 becomes either a section of
/// the panel it belongs to or a centred dialog. The account switcher took the
/// first road — it IS the 账户 panel — and these two took the second.
/// Which service panel has announced itself to the core this visit.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SettingsProbe {
    Endpoints,
    Providers,
}

/// A destructive action waiting on its question (spec 072 FR-010): nothing
/// here happens on the first press.
#[derive(Clone, Debug, PartialEq)]
enum Confirm {
    /// One of your-data rows, by the catalog's id — it cannot come back.
    ClearItem(&'static str),
    /// "Clear all caches".
    ClearCaches,
    /// One connected site, named.
    Disconnect { origin: String, name: SharedString },
    /// Every connected site.
    DisconnectAll,
    /// Service endpoints back to Vela's own, what was typed forgotten.
    ResetEndpoints,
    /// One contact, named — the web asks before either delete (078 C-01).
    DeleteContact { address: String, name: SharedString },
    /// One group, named; its contacts stay in the book.
    DeleteGroup { id: String, name: SharedString },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SettingsDialog {
    /// DST4b — search a chain, check it, add it.
    AddNetwork,
    /// DSR1 — one network's RPC is down and this is where it gets fixed.
    FixRpc,
    /// Spec 081 FR-017 — 抹除此设备, asked before it happens. The phone draws
    /// this as a bottom sheet; a wide layout has none, so it is a dialog.
    EraseDevice,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum ContactsMenu {
    /// Header ⋯ dropdown, right-aligned under the button (DC5 / M1).
    Header,
    /// Group-row context menu, top-left at the cursor (DC6 / M2).
    Group,
    /// Spec 022 M3 — the browsing toolbar's ⋯ site menu.
    Site,
    /// Spec 022 M4 — right-click on a favourite tile (DE2).
    Tile,
    /// Spec 032 phase 40 — right-click on a row in Recent. The only way to
    /// reach the core's `DeleteOrigin`, which forgets ONE site.
    Recent,
    /// Spec 034 — right-click on a contact row. Drawn since spec 018, opened
    /// by nothing until now.
    Contact,
    /// Spec 032 phase 41 — "move to a group", listing the person's own
    /// groups. A menu rather than a new picker component: the question is
    /// "which of these", which is what a menu is.
    MoveGroup,
    /// Spec 070 — which network the site on screen is on, ticked; picking
    /// one moves THAT site (`site_chain_picked`) and no other.
    SiteNetwork,
}

/// The identicon, big, beside the address that drew it (078 H-02, the web's
/// `IdenticonViewer`).
#[derive(Clone, Debug)]
struct IdenticonViewer {
    /// The seed, verbatim: what the artwork was drawn from.
    address: String,
    /// "Copied" is showing for this press. A number and not a flag, so an
    /// earlier press's timer cannot take down a later press's tick.
    copied: Option<u64>,
    presses: u64,
}

/// What the explore name dialog is asking for.
#[derive(Clone, Debug)]
enum ExploreAsk {
    /// Rename the favourite at this origin.
    RenameFavorite { origin: String },
    /// Name a new group. `then_add` is the favourite that opened the picker,
    /// which is dropped into the group the moment it exists — a person who
    /// went "move to → new group" meant both halves.
    NewGroup { then_add: Option<String> },
    /// Pin a site by typing where it lives. The start page's "+ 添加" tile was
    /// drawn with a pointer cursor and no listener, so the only way to get a
    /// favourite was to open a site first and use the tab menu.
    NewFavorite,
}

#[derive(Clone, Debug)]
struct ExploreForm {
    ask: ExploreAsk,
    text: String,
}

/// The frame the viewfinder is showing, and the discipline that keeps its
/// texture from leaking.
///
/// `RenderImage::new` mints a fresh `ImageId` and the renderer caches one GPU
/// texture per id, so a preview that published a frame without releasing its
/// predecessor would leak one texture per frame — thirty a second, invisible
/// on screen. `contracts/desktop-frame-pump.md` wrote the rule for the launch
/// animation; a camera is the second thing in this app that needs it.
#[derive(Default)]
struct ScanPreview {
    slot: Option<std::sync::Arc<gpui::RenderImage>>,
}

impl ScanPreview {
    /// Put a frame on screen and hand back the one it replaced.
    fn replace(
        &mut self,
        image: std::sync::Arc<gpui::RenderImage>,
    ) -> Option<std::sync::Arc<gpui::RenderImage>> {
        self.slot.replace(image)
    }

    fn take(&mut self) -> Option<std::sync::Arc<gpui::RenderImage>> {
        self.slot.take()
    }
}

/// The receive card's facts, owned so the composer can borrow them.
struct ShareCardFacts {
    headline: String,
    payload: String,
    name: String,
    lines: (String, String),
    network_note: String,
    network_ticker: String,
    network_tint: gpui::Hsla,
    seed: String,
    wordmark: String,
    file_name: String,
}

impl ShareCardFacts {
    fn as_card(&self) -> crate::flows::share_card::ShareCard<'_> {
        crate::flows::share_card::ShareCard {
            headline: &self.headline,
            payload: &self.payload,
            name: &self.name,
            lines: (&self.lines.0, &self.lines.1),
            network_note: &self.network_note,
            network_ticker: &self.network_ticker,
            network_tint: self.network_tint,
            seed: &self.seed,
            wordmark: &self.wordmark,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GalleryTab {
    D1,
    /// Spec 032 phase 43 — the money-in celebration, drawn: the toast the core
    /// arms and the row it is about, glowing under it.
    D1b,
    D2,
    D3,
    Dc1,
    Dc2,
    Dc3,
    Dc4,
    Dc5,
    Dc6,
    // spec 023 — one chip per settings mock.
    Dst1,
    Dst2,
    Dst3,
    Dst4,
    Dst4b,
    Dst5,
    Dst6,
    Dst7,
    Dst8,
    Dsr1,
    Components,
    ContactsComponents,
    Identicons,
}

impl GalleryTab {
    /// The chip strip, in order. One array so the bar and the inventory test
    /// can never disagree about which states the gallery exposes.
    const ALL: [(GalleryTab, &'static str); 23] = [
        (GalleryTab::D1, "D1"),
        (GalleryTab::D1b, "D1b"),
        (GalleryTab::D2, "D2"),
        (GalleryTab::D3, "D3"),
        (GalleryTab::Dc1, "DC1"),
        (GalleryTab::Dc2, "DC2"),
        (GalleryTab::Dc3, "DC3"),
        (GalleryTab::Dc4, "DC4"),
        (GalleryTab::Dc5, "DC5"),
        (GalleryTab::Dc6, "DC6"),
        (GalleryTab::Dst1, "DST1"),
        (GalleryTab::Dst2, "DST2"),
        (GalleryTab::Dst3, "DST3"),
        (GalleryTab::Dst4, "DST4"),
        (GalleryTab::Dst4b, "DST4b"),
        (GalleryTab::Dst5, "DST5"),
        (GalleryTab::Dst6, "DST6"),
        (GalleryTab::Dst7, "DST7"),
        (GalleryTab::Dst8, "DST8"),
        (GalleryTab::Dsr1, "DSR1"),
        (GalleryTab::Components, "Components"),
        (GalleryTab::ContactsComponents, "Contacts"),
        (GalleryTab::Identicons, "Identicons"),
    ];

    /// Spec 021's chips are generated from `FlowPanel::ALL` rather than listed
    /// again here, so a state cannot be added to the matrix and forgotten in
    /// the gallery.
    fn flow_chips() -> impl Iterator<Item = (FlowPanel, &'static str)> {
        FlowPanel::ALL.into_iter()
    }

    /// The contacts state code this chip reproduces, if any
    /// (data-model.md §Screen states — `dc1`…`dc6`).
    /// The chip `VELA_SETTINGS_STATE` names, if it names one.
    fn from_settings_env() -> Option<GalleryTab> {
        let want = std::env::var("VELA_SETTINGS_STATE").ok()?;
        GalleryTab::ALL
            .into_iter()
            .find_map(|(tab, _)| (tab.settings_state()? == want).then_some(tab))
    }

    /// The chip `VELA_GALLERY_TAB` names, by its own label.
    ///
    /// `VELA_SETTINGS_STATE` opens a settings chip and nothing else, so a state
    /// outside that section — D1b, which is the whole reason this exists — can
    /// only be reached by clicking, and a headless screenshot pass cannot
    /// click. Same env-pin family, same case-insensitive label match
    /// `FlowPanel::from_env` uses.
    fn from_gallery_env() -> Option<GalleryTab> {
        let want = std::env::var("VELA_GALLERY_TAB").ok()?;
        GalleryTab::ALL
            .into_iter()
            .find(|(_, label)| label.eq_ignore_ascii_case(want.trim()))
            .map(|(tab, _)| tab)
    }

    /// The settings state code this chip reproduces, if any (spec 023).
    fn settings_state(self) -> Option<&'static str> {
        match self {
            GalleryTab::Dst1 => Some("dst1"),
            GalleryTab::Dst2 => Some("dst2"),
            GalleryTab::Dst3 => Some("dst3"),
            GalleryTab::Dst4 => Some("dst4"),
            GalleryTab::Dst4b => Some("dst4b"),
            GalleryTab::Dst5 => Some("dst5"),
            GalleryTab::Dst6 => Some("dst6"),
            GalleryTab::Dst7 => Some("dst7"),
            GalleryTab::Dst8 => Some("dst8"),
            GalleryTab::Dsr1 => Some("dsr1"),
            _ => None,
        }
    }

    #[allow(dead_code, reason = "gallery inventory contract, asserted by tests")]
    fn contacts_state(self) -> Option<&'static str> {
        match self {
            GalleryTab::Dc1 => Some("dc1"),
            GalleryTab::Dc2 => Some("dc2"),
            GalleryTab::Dc3 => Some("dc3"),
            GalleryTab::Dc4 => Some("dc4"),
            GalleryTab::Dc5 => Some("dc5"),
            GalleryTab::Dc6 => Some("dc6"),
            _ => None,
        }
    }
}

pub struct WalletPage {
    mode: ThemeMode,
    /// Where the dApp browser opens. The toolbar's address field writes here
    /// and `webview::navigate` acts on it; the mock's Uniswap host is the
    /// default so the live browser starts where the drawings say it does.
    browser_home: String,
    /// Gallery-only appearance override (the VELA_THEME pin still wins at
    /// detect time; this cycles on top for quick eyeballing).
    override_mode: Option<ThemeMode>,
    strings: WalletStrings,
    contacts: ContactsStrings,
    settings: SettingsStrings,
    /// The resolved language tag. `Loc` is consumed at construction for its
    /// strings; l10n formatting needs the tag itself, and re-reading the
    /// environment once per frame to get it would be silly.
    locale: gpui::SharedString,
    explore: ExploreStrings,
    signing: SigningStrings,
    /// Whether the explore column is showing a page or the start page.
    browsing: bool,
    /// Which CS scenario the third column holds when `PanelId::Signing`.
    signing_state: &'static str,
    /// The signing sheet's "view raw data" disclosure. Per sheet: a new
    /// request opens closed, because the last one's decision is not this
    /// one's.
    signing_advanced_open: bool,
    section: Section,
    /// Which settings panel the second-level nav is showing (spec 023).
    settings_page: SettingsPage,
    /// The centred dialog over the settings section, when one is open.
    settings_dialog: Option<SettingsDialog>,
    /// An erase ran and these keys survived (spec 081 FR-017).
    ///
    /// `Some(empty)` never happens: an empty survivor list IS the success, and
    /// the window has left this screen by then. A non-empty one keeps the
    /// person signed in with the reason in the dialog's own callout, because
    /// telling somebody their machine is clean while their history is still on
    /// it is the one outcome this feature cannot have.
    erase_failed: Option<Vec<String>>,
    /// Which network row DST4 has expanded in place, if any.
    ///
    /// A `SharedString` rather than a `&'static str` since spec 030: the ids
    /// come from the core for a real session and from the fixtures for a design
    /// surface, and the screen only ever compares one to another.
    settings_expanded_network: Option<gpui::SharedString>,
    /// Which localization dropdown is open (DST3), by form-row id.
    settings_open_dropdown: Option<&'static str>,
    panel: PanelId,
    /// Spec 021: the open flow panels, deepest last. Empty means no flow.
    ///
    /// A stack, not a single id — the mocks stack: Receive opens a network list
    /// and a network opens its QR; Send runs picker → form → confirm → receipt.
    /// DR2L, DA2L, DT3L and DSD2L all draw a back chevron, and a chevron has to
    /// lead somewhere.
    flows: Vec<FlowPanel>,
    flow_strings: FlowStrings,
    /// DT3L, live: the contract-address field's focus handle.
    ///
    /// The value itself lives in the CORE (`MtokView::input_address`) — it
    /// validates the address and clears the found cards on every keystroke, so
    /// a second copy here would be a second opinion about what was typed.
    add_token_focus: gpui::FocusHandle,
    /// DT3L: the native tab is up (078 F-07) — a network by name or chain ID,
    /// through the `network_admin` wizard, instead of a contract address.
    add_token_native: bool,
    /// DT3L native: the chain this panel just added, and the query it was
    /// found by. The core resets its wizard on the add; this is what lets the
    /// panel go on saying "added" instead of going blank.
    add_net_added: Option<(vela_core::app::network_admin::NetChainInfo, String)>,
    /// The flow list search (078 X-05): which panel the query was typed on,
    /// and the query. A different panel on top clears it, as leaving a
    /// screen on the web drops that screen's query.
    flow_query: (Option<FlowPanel>, String),
    flow_query_focus: gpui::FocusHandle,
    /// What the scanner has to say about the last thing it read (078 F-02),
    /// the camera's own failure aside.
    scan_notice: Option<ScanNotice>,
    /// Payloads the camera reads before this instant are dropped — the web's
    /// two-second re-arm after an unusable code, so the same poster in frame
    /// is not refused thirty times a second.
    scan_quiet_until: Option<std::time::Instant>,
    /// Why the camera is not running, as the last frame reported it.
    scan_camera_failure: Option<crate::executor::camera::CameraFailure>,
    /// A once-a-second redraw is running for a waiting receipt (078 F-04):
    /// the countdown and the ring move with the clock, not with the core.
    receipt_ticking: bool,
    /// A dApp transaction landing in the signing column (078 G-04, spec 077):
    /// the operation, its chain, and when the column raised it.
    dapp_landing: Option<DappLanding>,
    /// The operation a landing was last raised for — dismissed or not. One
    /// landing per operation: the handoff stays in the view after Done, and
    /// gating on "is one showing" raised the same receipt again forever (the
    /// web measured exactly that in its packaged extension).
    dapp_landed_op: Option<String>,
    /// The contacts header search (078 X-05 / C-02): the web filters the
    /// A–Z list as it is typed, in the shell (`letterSections`).
    contacts_query: String,
    /// SR3, the balance breakdown the hero's status line opens (078 H-03).
    balance_detail_open: bool,
    contacts_query_focus: gpui::FocusHandle,
    /// Spec 032: the send journey's two machines, alive while the flow is
    /// open and discarded with it — a second send starts from a fresh
    /// machine, never a resumed one.
    send_host: Option<gpui::Entity<SendHost>>,
    /// The signing journey's four machines, born with a dApp request and gone
    /// when the core hides the sheet. `None` means the panel draws its mock,
    /// which is what the gallery and an unsigned-in window get.
    #[cfg(not(target_os = "linux"))]
    signing_host: Option<gpui::Entity<crate::wallet::signing_host::SigningHost>>,
    /// Where the active wallet's founding record stands on Ethereum (spec 062):
    /// the address it was asked about, and the answer once there is one
    /// (`None` = still asking). Asked of the chain, never of our server, each
    /// time the Account page meets a different account.
    backup_for: Option<String>,
    backup_check: Option<(
        vela_core::registry_backup::BackupState,
        Option<vela_core::registry_backup::BackupCall>,
    )>,
    /// Which passkeys control that same wallet (spec 062), read from the
    /// registry contract; `None` = still asking.
    keys_check: Option<(
        vela_core::wallet_keys::KeysSource,
        Vec<vela_core::wallet_keys::WalletKeyRow>,
    )>,
    /// Which key rows are open, by founding position; and the `row:label` of
    /// the value just copied, for the button's "Copied".
    keys_open: std::collections::HashSet<usize>,
    /// What was copied a moment ago, by key, while its feedback shows —
    /// a tick, a "Copied", a toast (078 X-06). `copied_press` counts the
    /// copies, so an older copy's timer never clears a newer one's tick.
    copied: Option<SharedString>,
    copied_press: u64,
    /// Which favourite the open tile menu is about.
    menu_origin: Option<String>,
    /// The explore name dialog: renaming a tile, or naming a new group.
    ///
    /// One dialog for both, as the contacts one is for its two questions —
    /// they ask the same thing and differ only in which event takes the
    /// answer.
    explore_form: Option<ExploreForm>,
    explore_form_focus: gpui::FocusHandle,
    /// The cap field's focus, kept on the page so typing survives a redraw.
    cap_focus: FocusHandle,
    /// What the open page last called itself, from the bridge's own report.
    /// The star pins with THIS rather than the host, because the host is what
    /// a tile falls back to and a page's title is what a person recognises.
    browser_title: Option<String>,
    /// The browser machine (spec 070): every page's requests, the grants,
    /// each site's chain, the signing line. Born the first time anything needs
    /// it — a page's first message, or Settings listing the connected sites —
    /// and alive for the rest of the session: leaving Explore settles nothing.
    browser_host: Option<gpui::Entity<BrowserHost>>,
    /// The consent sheet last seen, so the connection panel opens when a
    /// NEW question appears rather than on every answer the machine delivers.
    browser_consent: Option<(String, String)>,
    /// The page's sinks are installed once per page, not once per frame.
    #[cfg(not(target_os = "linux"))]
    dapp_requests_armed: bool,
    /// The address bar while it is being typed in — `None` when it shows the
    /// page (or the placeholder). What was typed goes through the core's
    /// `browser_input`, so a word searches and a host gets a scheme.
    address_draft: Option<String>,
    /// The whole draft is selected (a click into the bar, or ⌘A / Ctrl+A):
    /// what is typed or pasted next replaces it.
    address_selected: bool,
    address_focus: gpui::FocusHandle,
    /// The Wallet column's scroll position, for the bar drawn beside it.
    content_scroll: gpui::ScrollHandle,
    /// The third column's body, and which subject it last scrolled for.
    panel_scroll: gpui::ScrollHandle,
    panel_scroll_subject: String,
    /// The sidebar's network list — twenty-odd chains outgrow a short window.
    networks_scroll: gpui::ScrollHandle,
    /// The settings panel's, for the same bar.
    settings_scroll: gpui::ScrollHandle,
    /// The network menu's rows — `(chain, name, current)` — named when it
    /// opened; `menu_origin` says which site it is about.
    site_networks: Vec<(u32, SharedString, bool)>,
    /// `VELA_BROWSER_URL` is applied once, not on every frame the column draws.
    browser_url_pinned: bool,
    send_amount_focus: gpui::FocusHandle,
    send_recipient_focus: gpui::FocusHandle,
    /// DSD2cL, live: the rate field's focus.
    send_rate_focus: gpui::FocusHandle,
    /// DSD2bL, live: one focus handle per split row, made on first use. A
    /// shared handle would send every keystroke to whichever row drew last.
    split_focuses: Vec<gpui::FocusHandle>,
    /// One per split row's address field (078 F-06).
    split_address_focuses: Vec<gpui::FocusHandle>,
    /// DSD2fL is the page's own overlay: the core has no flag for it.
    send_fee_picker: bool,
    /// The scanner's camera, alive only while DS1 is on screen — a capture
    /// nobody stops is a webcam light nobody turned off.
    scan_camera: Option<crate::executor::camera::Session>,
    /// The one preview frame currently on screen, and the texture behind it.
    scan_preview: ScanPreview,
    /// SD1b: the token picker is in sweep mode. The page's, not the core's —
    /// `multi_select_mode` turns on when the selection is CONFIRMED, so before
    /// that nothing in the view says whether the ticks are showing.
    send_sweeping: bool,
    /// The send picker's lit chip. The shell's, as on the web: narrowing a
    /// list is not a fact about the send.
    send_class: flows_live::SendClass,
    /// The native window, for the one platform whose passkey dialog is the
    /// OS's; captured once, because a ceremony runs off the main thread and
    /// cannot reach `Window` from there.
    window_handle: WindowHandle,
    /// The resolved locale, kept for the cable's own dialogs (touch / PIN /
    /// pick), which take it whole.
    loc: Loc,
    /// A survived panic's report, shown as the failure sheet (spec 038).
    crash: Option<crate::outcome::Prompt>,
    /// One per editable settings field, made on first use.
    endpoint_focuses: Vec<gpui::FocusHandle>,
    /// The Trusted Signer page as typed (spec 071), until it is saved — `None`
    /// shows the page `sign_pref` holds. The core validates it on Save, not
    /// per keystroke: half an address is not an error yet.
    signer_page_draft: Option<String>,
    signer_page_focus: Option<gpui::FocusHandle>,
    /// Spec 075: the tunnel, the same way — typed until Save, and the core
    /// says whether it is one (https/wss only, loopback allowed).
    tunnel_focus: Option<gpui::FocusHandle>,
    /// Which network card's probes have been asked for, so opening one asks
    /// once rather than on every frame.
    settings_probed_network: Option<u32>,
    /// Which of the two service panels has already announced itself this
    /// visit. A panel is drawn every frame; its probes must run once.
    settings_probed_panel: Option<SettingsProbe>,
    /// DSR1, live: WHICH unreachable chain the rescue dialog is about.
    settings_fix_chain: Option<u32>,
    /// The custom network the remove confirmation is about: its id and the
    /// name to say back to the person.
    network_remove: Option<(String, SharedString)>,
    /// The destructive action on screen, asked about and not yet done.
    confirm: Option<Confirm>,
    /// Edited settings fields not yet committed, by focus slot: the event
    /// that persists each (spec 072). Leaving the field — or Enter — commits
    /// it; a field left untouched commits nothing.
    field_commits: std::collections::HashMap<usize, settings_live::FieldCommit>,
    /// One blur subscription per settings focus slot, made on the frame after
    /// the slot's handle.
    field_blurs: Vec<gpui::Subscription>,
    /// What the last address-book import did, as a title and a line. Cleared
    /// by acknowledging it.
    import_result: Option<(SharedString, SharedString)>,
    /// The add/edit contact sheet: `Some((address, name))` while it is open.
    ///
    /// The ADDRESS is the identity — the core keys on it and an edit that
    /// changed it would be a delete and an add wearing one button. So editing
    /// an existing contact keeps it fixed and only the name is a draft.
    contact_form: Option<ContactForm>,
    /// 添加成员 / 移入分组 — the web's tick list with a Save (078 C-06).
    pick: Option<PickDialog>,
    pick_query_focus: gpui::FocusHandle,
    /// The contact whose address is being shown as a code, if any.
    contact_qr: Option<(SharedString, SharedString)>,
    /// The group name sheet: `Some((id, name))`, with `None` for a new group.
    /// One dialog for 新建分组 and 重命名分组, because they are one question.
    group_form: Option<(Option<String>, String)>,
    group_form_focus: gpui::FocusHandle,
    contact_form_name_focus: gpui::FocusHandle,
    contact_form_address_focus: gpui::FocusHandle,
    /// D3, live: WHICH holding the asset strip opened, as an index into the
    /// core's sorted `tokens`.
    ///
    /// `None` for the mocks. An index rather than a key because the core's
    /// order IS the identity here — the panel's own model re-reads it and
    /// answers `None` when the list moved underneath.
    asset_detail: Option<usize>,
    /// DA2L, live: WHICH transaction the history stepped into.
    ///
    /// `None` while the mocks draw, and after a record is deleted — the panel
    /// then closes rather than showing a stale detail over a row that no
    /// longer exists.
    tx_detail: Option<String>,
    /// DR2L, live: WHICH network's QR the receive flow stepped into.
    ///
    /// The mock never needed this — every fixture row opened the same picture.
    /// A live receive names a real chain in its title and draws that chain's
    /// mark in the middle of the code, so a person who stepped into "Gnosis"
    /// must not be shown "Ethereum". Defaults to Gnosis, the chain this wallet
    /// is cheapest to be paid on.
    receive_chain: u32,
    /// DR3L, live: the held token whose own code is open — kept as it was
    /// when its detail's 收款 was pressed, because the holdings list can
    /// re-order under an open panel and an index would then name another.
    receive_token: Option<vela_core::app::balance_dashboard::BalanceToken>,
    /// The sidebar's network filter: one chain, or `None` for every network.
    ///
    /// Render state, not a preference — the phone keeps it in component state
    /// and forgets it on relaunch, and so does this. On the page rather than in
    /// a module because ONE page draws the sidebar on every section here: a
    /// chain chosen on 设置 is the one 钱包 then shows.
    chain_filter: Option<u32>,
    /// D1b: the drawn celebration, with no core behind it.
    ///
    /// A LIVE celebration comes from `FeedView::toast` and lasts the core's
    /// 2.8 seconds; this one holds still, because a drawing a reviewer cannot
    /// look at for longer than three seconds is not a drawing.
    celebrating: bool,
    /// What the feed was last told about balance privacy.
    ///
    /// The flag itself belongs to `balance_dashboard`; the feed suppresses its
    /// toast on the core's own invariant ④ and can only do that if it is told.
    /// `None` = never told, so the first frame tells it.
    feed_privacy: Option<bool>,
    /// `None` = 全部联系人; `Some(i)` = the group view for `GROUPS[i]` (DC4).
    group: Option<usize>,
    /// Which contact the third column shows (index into the canon roster).
    contact: usize,
    /// The address the core was last asked to inspect, so opening a panel that
    /// redraws every frame asks once.
    inspected_contact: Option<String>,
    /// The accounts the switcher last announced. `None` = it is not on screen,
    /// and the core has been told so.
    switcher_addresses: Option<Vec<String>>,
    /// The switcher row a "remove this wallet" confirmation is open for
    /// (2026-09-23). A position in the ORIGINAL list, as the core removes by.
    /// Asked inline, under the row, as the web's `AccountsSheetBody` asks it.
    removing_account: Option<usize>,
    /// 078 H-01 — the account switcher, opened from the sidebar header's
    /// name, over whichever section is on screen.
    account_switcher: bool,
    switcher_scroll: gpui::ScrollHandle,
    /// Each `ui::dialog`'s body scroll, by dialog id (078 X-04).
    dialog_scrolls: std::collections::HashMap<&'static str, gpui::ScrollHandle>,
    /// 078 H-02 — the identicon viewer, over everything, switcher included.
    identicon_viewer: Option<IdenticonViewer>,
    /// DC3: the fixture roster is empty.
    contacts_empty: bool,
    /// The open contact's 最近往来 shows every row, not the first three
    /// (078 C-04). Forgotten when another contact opens, as the web's `open`
    /// resets `allActivity`.
    contact_all_activity: bool,
    /// Open anchored menu: which fixture feeds it, the window-coordinate
    /// anchor point, and which of the card's corners sits on that point.
    menu: Option<(ContactsMenu, Point<Pixels>, Anchor)>,
    tab: GalleryTab,
    gallery: bool,
    /// The signed-in account, when there is one.
    ///
    /// `None` means the fixture identity — the design page opened directly with
    /// `VELA_PAGE=wallet`, which is how spec 015's states are reviewed. A real
    /// session replaces the identity and NOTHING else: balances, activity and
    /// networks are still fixtures, and pretending otherwise by hiding them
    /// would make a signed-in wallet look emptier than a fixture one rather
    /// than more honest.
    identity: Option<Identity>,
    icons: IconCache,
    identicons: IdenticonCache,
    focus_handle: FocusHandle,
}

/// The account the header and the receive panel name.
#[derive(Clone, Debug)]
pub struct Identity {
    pub name: SharedString,
    pub address: String,
}

/// What the live send panels bind to (spec 032): the host to dispatch to,
/// and the per-row facts each listener carries.
struct SendBindings {
    host: gpui::Entity<SendHost>,
    token_ids: Vec<String>,
    contact_addresses: Vec<String>,
    fee_contracts: Vec<Option<String>>,
    amount: String,
    recipient: String,
    amount_focus: gpui::FocusHandle,
    recipient_focus: gpui::FocusHandle,
    /// DSD2cL: the rate string the core holds, and the field's focus.
    batch_rate: Option<String>,
    rate_focus: gpui::FocusHandle,
    /// DSD2eL: each group's members as split rows, in drawn order.
    group_members: Vec<Vec<SendRecipientDraft>>,
    /// SD1b: the picker is choosing SEVERAL tokens. A shell flag — the core's
    /// `multi_select_mode` flips only once a selection is confirmed — and the
    /// chain each row is on, so the first pick can name the network.
    sweeping: bool,
    token_chain_ids: Vec<u32>,
    multi_chain_id: Option<u32>,
    /// DSD2bL: the split rows as the core holds them. Every edit sends the
    /// WHOLE list back (`RecipientsChanged`), because that is the event the
    /// machine offers — there is no per-row patch, and inventing one here
    /// would be a second opinion about what a row is.
    recipients: Vec<SendRecipientDraft>,
    split_focuses: Vec<gpui::FocusHandle>,
    split_address_focuses: Vec<gpui::FocusHandle>,
    /// "Use X for the empty rows": the figure the offer copies, when the
    /// form offers it (`flows_live::split_fill_source`).
    fill_empty: Option<String>,
    /// What the notice's way-out means on THIS panel — derived by the same
    /// traversal that wrote the sentence, so the button and the words cannot
    /// disagree about what they are offering.
    way_out: Option<flows_live::NoticeWayOut>,
}

impl Identity {
    /// `0x14fB1f…D1eA5c` — the same middle-truncation every other client uses.
    fn display(&self) -> SharedString {
        let address = &self.address;
        if address.len() <= 14 {
            return SharedString::from(address.clone());
        }
        SharedString::from(format!(
            "{}…{}",
            &address[..8],
            &address[address.len() - 6..]
        ))
    }
}

impl WalletPage {
    pub fn new(gallery: bool, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self::with_section(Section::Wallet, gallery, window, cx);
        if gallery && let Some(tab) = GalleryTab::from_gallery_env() {
            page.select_tab(tab, window);
        }
        page
    }

    /// The wallet as a signed-in person sees it.
    ///
    /// `VELA_SECTION=settings|contacts|explore` starts it on that section
    /// instead of 钱包 — the same env-pin family as `VELA_PAGE` and
    /// `VELA_SETTINGS_STATE`, and added for the same reason that one was: the
    /// LIVE surfaces (spec 030) are only reachable by clicking, so without this
    /// no screenshot pass can ever see one. `VELA_PAGE=settings` is not the
    /// same thing and must not become it — that route has no session behind it
    /// and renders the mocks on purpose.
    pub fn signed_in(identity: Identity, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let section = match std::env::var("VELA_SECTION").as_deref() {
            Ok("settings") => Section::Settings,
            Ok("contacts") => Section::Contacts,
            Ok("explore") => Section::Explore,
            _ => Section::Wallet,
        }
        .or_wallet();
        let mut page = Self::with_section(section, false, window, cx);
        page.identity = Some(identity);
        // Money in flight outlives every screen: the tracker runs from the
        // moment somebody is signed in, not from the moment a send opens.
        crate::executor::tracker::start(cx);
        // And the feed's own 30 s pass, for the same reason in the other
        // direction: money ARRIVING is noticed by a scan nobody asked for, and
        // without this tick the machine performs its boot pipeline once and
        // never celebrates anything, because the first pass never celebrates.
        crate::executor::activity_feed::start_ticks(cx);
        // And the hero's own cadence. Until this call the desktop dispatched
        // exactly one balance event ever — the boot's `AccountChanged` — so the
        // total on screen was the total at launch, restart being the only way
        // to move it. This also hydrates the stored privacy flag, which had
        // been written since spec 030 and never read back.
        crate::executor::balance_dashboard::start_ticks(cx);
        // `VELA_SETTINGS_STATE` picks WHICH panel, on this path too. Without it
        // `VELA_SECTION=settings` can only ever open 账户, so the live 网络 and
        // 本地化 surfaces would still have no way to be screenshotted.
        if section == Section::Settings
            && let Some(tab) = GalleryTab::from_settings_env()
        {
            page.select_tab(tab, window);
        }
        page
    }

    /// What the header, the receive panel and the identicon are drawn from.
    fn identity(&self) -> Identity {
        self.identity.clone().unwrap_or_else(|| Identity {
            name: WALLET_NAME.into(),
            address: ADDRESS_FULL.to_owned(),
        })
    }

    /// `VELA_PAGE=settings` opens straight onto 设置 (spec 023).
    ///
    /// `VELA_SETTINGS_STATE=dst7` picks WHICH panel, the same env-pin family as
    /// `VELA_PAGE`/`VELA_THEME`/`VELA_LANG` and the same seam iOS has. Without
    /// it a screenshot pass can only ever see DST1 — which is how a
    /// left-alignment bug survived review on seven panels it also broke.
    pub fn settings(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut page = Self::with_section(Section::Settings, false, window, cx);
        if let Some(tab) = GalleryTab::from_settings_env() {
            page.select_tab(tab, window);
        }
        page
    }

    /// `VELA_PAGE=contacts` opens straight onto 通讯录 (spec 018 research D1).
    pub fn contacts(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_section(Section::Contacts, false, window, cx)
    }

    /// `VELA_PAGE=explore` opens straight onto the browser (spec 022).
    pub fn explore(window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self::with_section(Section::Explore, false, window, cx)
    }

    fn with_section(
        section: Section,
        gallery: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let loc = Loc::from_env();
        eprintln!(
            "[vela-wallet] wallet: locale `{}`, gallery {gallery}, section {section:?}",
            loc.language()
        );
        let strings = WalletStrings::resolve(&loc);
        let contacts = ContactsStrings::resolve(&loc);
        let settings = SettingsStrings::resolve(&loc);
        let explore = ExploreStrings::resolve(&loc);
        let signing = SigningStrings::resolve(&loc);

        // The window coming back is the desktop's `visibilitychange`: the web
        // refreshes the hero and ticks the feed on it, and this app did
        // neither. Registered for the life of the page; the guard is the
        // identity, because an unsigned window has nothing to refresh.
        cx.observe_window_activation(window, |page, window, cx| {
            if page.identity.is_none() {
                return;
            }
            if window.is_window_active() {
                crate::executor::balance_dashboard::dispatch(
                    vela_core::app::balance_dashboard::Event::AppFocused,
                    cx,
                );
                crate::executor::activity_feed::focus_tick(cx);
                // A reconcile sweep, not a throttled poll: coming back is
                // exactly when a submission somebody walked away from should
                // be settled.
                crate::executor::tracker::focused(cx);
            } else {
                crate::executor::balance_dashboard::dispatch(
                    vela_core::app::balance_dashboard::Event::AppBackgrounded,
                    cx,
                );
            }
        })
        .detach();

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

        let focus_handle = cx.focus_handle();
        focus_handle.focus(window, cx);

        Self {
            mode: ThemeMode::detect(window),
            // The host the drawings browse. `https://` because a webview is
            // not a place to make an exception about transport security.
            browser_home: format!("https://{}", explore_fixtures::uniswap().host),
            override_mode: None,
            strings,
            contacts,
            settings,
            section,
            panel: if FlowPanel::from_env().is_some() {
                PanelId::Flow
            } else {
                PanelId::None
            },
            flows: FlowPanel::from_env()
                .map(FlowPanel::stack)
                .unwrap_or_default(),
            flow_strings: FlowStrings::resolve(&loc),
            receive_chain: 100,
            receive_token: None,
            celebrating: false,
            chain_filter: None,
            feed_privacy: None,
            tx_detail: None,
            asset_detail: None,
            add_token_focus: cx.focus_handle(),
            add_token_native: false,
            add_net_added: None,
            flow_query: (None, String::new()),
            flow_query_focus: cx.focus_handle(),
            scan_notice: None,
            scan_quiet_until: None,
            scan_camera_failure: None,
            receipt_ticking: false,
            dapp_landing: None,
            dapp_landed_op: None,
            contacts_query: String::new(),
            balance_detail_open: false,
            contacts_query_focus: cx.focus_handle(),
            send_host: None,
            #[cfg(not(target_os = "linux"))]
            signing_host: None,
            backup_for: None,
            backup_check: None,
            keys_check: None,
            // `VELA_KEYS_OPEN=1` (debug builds): the first key row starts open, so
            // the details card can be looked at without a click — the same env-pin
            // family as `VELA_PAGE` / `VELA_THEME`.
            keys_open: if cfg!(debug_assertions)
                && std::env::var("VELA_KEYS_OPEN").as_deref() == Ok("1")
            {
                std::collections::HashSet::from([0])
            } else {
                std::collections::HashSet::new()
            },
            copied: None,
            copied_press: 0,
            menu_origin: None,
            explore_form: None,
            explore_form_focus: cx.focus_handle(),
            browser_title: None,
            cap_focus: cx.focus_handle(),
            browser_host: None,
            browser_consent: None,
            #[cfg(not(target_os = "linux"))]
            dapp_requests_armed: false,
            address_draft: None,
            address_selected: false,
            address_focus: cx.focus_handle(),
            content_scroll: gpui::ScrollHandle::new(),
            panel_scroll: gpui::ScrollHandle::new(),
            panel_scroll_subject: String::new(),
            networks_scroll: gpui::ScrollHandle::new(),
            settings_scroll: gpui::ScrollHandle::new(),
            site_networks: Vec::new(),
            browser_url_pinned: false,
            send_amount_focus: cx.focus_handle(),
            send_recipient_focus: cx.focus_handle(),
            send_rate_focus: cx.focus_handle(),
            split_focuses: Vec::new(),
            split_address_focuses: Vec::new(),
            send_fee_picker: false,
            send_sweeping: false,
            send_class: flows_live::SendClass::All,
            scan_camera: None,
            scan_preview: ScanPreview::default(),
            window_handle: crate::onboarding::native_window_handle(window),
            endpoint_focuses: Vec::new(),
            signer_page_draft: None,
            signer_page_focus: None,
            tunnel_focus: None,
            settings_probed_network: None,
            settings_probed_panel: None,
            settings_fix_chain: None,
            network_remove: None,
            confirm: None,
            field_commits: std::collections::HashMap::new(),
            field_blurs: Vec::new(),
            import_result: None,
            contact_form: None,
            pick: None,
            pick_query_focus: cx.focus_handle(),
            contact_qr: None,
            group_form: None,
            group_form_focus: cx.focus_handle(),
            contact_form_name_focus: cx.focus_handle(),
            contact_form_address_focus: cx.focus_handle(),
            locale: gpui::SharedString::from(loc.language().to_owned()),
            explore,
            signing,
            browsing: false,
            signing_state: "cs12",
            signing_advanced_open: false,
            settings_page: SettingsPage::Account,
            settings_dialog: None,
            erase_failed: None,
            settings_expanded_network: None,
            settings_open_dropdown: None,
            group: None,
            contact: 0,
            inspected_contact: None,
            switcher_addresses: None,
            removing_account: None,
            account_switcher: false,
            switcher_scroll: gpui::ScrollHandle::new(),
            dialog_scrolls: std::collections::HashMap::new(),
            identicon_viewer: None,
            contacts_empty: false,
            contact_all_activity: false,
            menu: None,
            tab: match section {
                Section::Wallet | Section::Explore => GalleryTab::D1,
                Section::Contacts => GalleryTab::Dc1,
                Section::Settings => GalleryTab::Dst1,
            },
            gallery,
            identity: None,
            icons: IconCache::default(),
            identicons: IdenticonCache::default(),
            focus_handle,
            loc,
            crash: None,
        }
    }

    /// The confirmation the core opens, with the warning it decided on.
    ///
    /// `pending_upload_warning` is not this screen's judgement: the session
    /// machine asks storage whether any public key never reached the registry
    /// and puts the answer here. A key in that state is one this wallet may not
    /// be able to sign in with from anywhere else yet, which is the one fact
    /// that should give someone pause — so the dialog does not open until the
    /// core has the answer.
    /// What the open receive screen would save.
    ///
    /// Built from the SAME facts the panel is drawing — the identity, the
    /// chain and the core's `qr_value` — so the picture and the screen cannot
    /// disagree about an address. `None` when there is nobody to receive as,
    /// or nothing to encode yet.
    fn receive_share_card(&mut self, cx: &mut Context<Self>) -> Option<ShareCardFacts> {
        let identity = self.identity.clone()?;
        let pay = resident::resident::<PaymentRequest>(cx).read(cx).view();
        // The gate: no card before the warning has been read, for the same
        // reason there is no address on screen (phase 38). A saved picture is
        // the address handed over in the most copyable form there is.
        if pay.qr_value.is_empty() || !pay.can_save {
            return None;
        }
        let lines = flow_fixtures::address_lines(&identity.address);
        let network = crate::flows::live::chain_name(self.receive_chain);
        Some(ShareCardFacts {
            headline: self.flow_strings.share_card_headline.to_string(),
            payload: pay.qr_value.clone(),
            name: identity.name.to_string(),
            lines,
            // The web's own pill string — "{{network}} payments only" — not
            // the screen's long sentence: a pill holds a label, and the two
            // shells' cards should read the same.
            network_note: crate::wallet::fill(
                &self.flow_strings.share_card_note,
                "network",
                &network,
            ),
            // "Vela Wallet", as the web's card signs itself.
            // A token's own code marks the card with the token (the web's
            // `networkMark: balanceTokenMark(token)`); a network's, the network.
            network_ticker: match (self.flows.last(), self.receive_token.as_ref()) {
                (Some(FlowPanel::Dr3), Some(token)) => &token.symbol,
                _ => &network,
            }
            .chars()
            .take(3)
            .collect::<String>()
            .to_uppercase(),
            network_tint: flows_live::chain_tint(self.receive_chain),
            seed: identity.address.to_string(),
            wordmark: "Vela Wallet".to_owned(),
            // The address is in the NAME as well as the picture: a folder of
            // these is unreadable if every one is called vela.png.
            file_name: format!(
                "vela-{}.png",
                &identity.address[..10.min(identity.address.len())]
            ),
        })
    }

    /// 保存图片 — the receive card, composed and written where they say.
    ///
    /// The picture is built from what the screen is already showing, so the
    /// saved card and the open screen cannot disagree about an address. The
    /// dialog opens in the home directory for the reason the contacts export
    /// does: `.` is wherever the binary was launched from, which on a
    /// double-click is nowhere useful.
    fn save_share_card(&mut self, cx: &mut Context<Self>) {
        let Some(model) = self.receive_share_card(cx) else {
            return;
        };
        let theme = Theme::of(self.theme_mode());
        let png = crate::flows::share_card::render_png(&model.as_card(), &theme);
        // Composed BEFORE the dialog: a picture that fails to render must not
        // ask somebody where to put it first.
        let Some(png) = png else {
            return;
        };
        let directory = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let target = cx.prompt_for_new_path(&directory, Some(&model.file_name));
        cx.spawn(async move |_, _| {
            let Ok(Ok(Some(path))) = target.await else {
                return;
            };
            // Best effort, like every other write in this shell: a refused
            // disk is not something to interrupt a receive screen over.
            let _ = std::fs::write(&path, &png);
        })
        .detach();
    }

    /// The explore name sheet — renaming a tile, and naming a new group.
    ///
    /// The contacts dialog's twin, deliberately: same card, same field, same
    /// two buttons. Two dialogs that ask for a name should not look like two
    /// different questions.
    fn explore_form_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let form = self.explore_form.clone()?;
        let e = &self.explore;
        let c = &self.contacts;
        let title = match form.ask {
            ExploreAsk::RenameFavorite { .. } => e.rename.clone(),
            ExploreAsk::NewGroup { .. } => e.new_group.clone(),
            ExploreAsk::NewFavorite => e.add_to_favorites.clone(),
        };
        // The favourite's field takes a URL and nothing else — this dialog
        // searches nothing — so it is named the way the corpus names the place
        // a URL is typed (`explore.addressBar`). It used to promise
        // "搜索 dApp，或输入网址": half of that offer did not exist.
        let placeholder = match form.ask {
            ExploreAsk::NewFavorite => e.address_bar.clone(),
            _ => SharedString::from(""),
        };
        // A name that is only spaces is not a name — the core refuses it, and
        // an armed Save that the core would drop is a button that lies.
        //
        // The favourite's field is held to the same standard against a
        // stricter rule: it takes an ADDRESS, and `save_explore_form` drops
        // anything `coerce_browser_url` refuses. Armed on "notanaddress", the
        // button closed the dialog, pinned nothing, and said nothing.
        let can_save = match form.ask {
            ExploreAsk::NewFavorite => {
                vela_core::app::dapp_session::coerce_browser_url(&form.text).is_some()
            }
            _ => !form.text.trim().is_empty(),
        };
        let focus = self.explore_form_focus.clone();
        // No label: the card's heading is already this field's name, and
        // repeating it verbatim in small caps under itself ("添加到收藏" over
        // "添加到收藏") says the same thing twice. `name_field`'s own rule,
        // the one the create screen has followed since spec 019.
        let strings = crate::ui::NameFieldStrings {
            label: SharedString::from(""),
            placeholder,
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let save_label = c.save.clone();

        let body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .pb(px(16.))
            .child(crate::ui::text_field(
                "explore-form-name",
                theme,
                &strings,
                &form.text,
                false,
                false,
                &focus,
                window,
                {
                    let page = cx.entity();
                    move |text: String, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this: &mut Self, cx| {
                            if let Some(form) = this.explore_form.as_mut() {
                                form.text = text.clone();
                            }
                            cx.notify();
                        });
                    }
                },
            ))
            .child(self.form_save(
                theme,
                "explore-form-save",
                save_label,
                can_save,
                cx.listener(|this, _, _, cx| this.save_explore_form(cx)),
            ));

        Some(
            crate::ui::dialog::dialog(
                "explore-form",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("explore-form"),
                Self::closer(cx, |this, _| this.explore_form = None),
            )
            .into_any_element(),
        )
    }

    /// A dialog form's one answer (the web's `GroupForm`): the primary button
    /// at full width, 8 under the field, dimmed until it can act. The ✕ is
    /// the way out, so there is no Cancel beside it.
    fn form_save(
        &self,
        theme: &Theme,
        id: &'static str,
        label: SharedString,
        can_save: bool,
        on_save: impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static,
    ) -> gpui::AnyElement {
        if can_save {
            crate::flows::components::accent_button(theme, label)
                .mt(px(8.))
                .id(id)
                .cursor_pointer()
                .on_click(on_save)
                .into_any_element()
        } else {
            crate::flows::components::disabled_accent_button(theme, label)
                .mt(px(8.))
                .into_any_element()
        }
    }

    /// Take the typed name and give it to whichever machine asked.
    ///
    /// A new group made from "move to a group" also TAKES the tile: somebody
    /// who went that way meant both halves, and leaving the group empty would
    /// make them do the second half again.
    fn save_explore_form(&mut self, cx: &mut Context<Self>) {
        let Some(form) = self.explore_form.take() else {
            return;
        };
        let name = form.text.trim().to_owned();
        let resident = resident::resident::<ExploreSites>(cx);
        match form.ask {
            ExploreAsk::RenameFavorite { origin } => {
                resident.update(cx, |resident, cx| {
                    resident.dispatch(
                        vela_core::app::explore_sites::Event::FavoriteRenamed { origin, name },
                        cx,
                    );
                });
            }
            ExploreAsk::NewFavorite => {
                // Whatever was typed, coerced the way the address bar coerces
                // it — "uniswap.org" is an address, and the core keys a
                // favourite on the origin it derives from the URL.
                let Some(url) = vela_core::app::dapp_session::coerce_browser_url(&name) else {
                    // Not an address. Refusing is the same answer the address
                    // bar gives; inventing `https://` in front of a typo would
                    // pin a tile to a site that does not exist.
                    return;
                };
                resident.update(cx, |resident, cx| {
                    resident.dispatch(
                        vela_core::app::explore_sites::Event::FavoriteAdded {
                            url,
                            title: None,
                            now_ms: crate::executor::now_ms(),
                        },
                        cx,
                    );
                });
            }
            ExploreAsk::NewGroup { then_add } => {
                resident.update(cx, |resident, cx| {
                    resident.dispatch(
                        vela_core::app::explore_sites::Event::GroupCreated {
                            name,
                            now_ms: crate::executor::now_ms(),
                        },
                        cx,
                    );
                });
                // The id is the core's, so it is read back rather than
                // guessed: the machine makes it unique against what exists.
                if let Some(origin) = then_add
                    && let Some(id) = resident
                        .read(cx)
                        .view()
                        .groups
                        .last()
                        .map(|group| group.id.clone())
                {
                    resident.update(cx, |resident, cx| {
                        resident.dispatch(
                            vela_core::app::explore_sites::Event::GroupMemberAdded { id, origin },
                            cx,
                        );
                    });
                }
            }
        }
        cx.notify();
    }

    /// The group name sheet — 新建分组 and 重命名分组.
    ///
    /// One dialog for both, because they ask the same question and the core
    /// takes the same event: `ContactGroupInput` with `id: None` creates and an
    /// existing id renames in place.
    fn group_form_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (id, name) = self.group_form.clone()?;
        let s = &self.contacts;
        let title = if id.is_some() {
            s.group_rename.clone()
        } else {
            s.group_new.clone()
        };
        // A group with no name is a row nobody can tell from another.
        let can_save = !name.trim().is_empty();
        let focus = self.group_form_focus.clone();
        let strings = crate::ui::NameFieldStrings {
            label: s.group_name_label.clone(),
            placeholder: s.group_name_placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let save_label = s.save.clone();

        let body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .pb(px(16.))
            .child(crate::ui::text_field(
                "group-form-name",
                theme,
                &strings,
                &name,
                false,
                false,
                &focus,
                window,
                {
                    let page = cx.entity();
                    move |text: String, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this: &mut Self, cx| {
                            if let Some((_, name)) = this.group_form.as_mut() {
                                *name = text.clone();
                            }
                            cx.notify();
                        });
                    }
                },
            ))
            .child(self.form_save(
                theme,
                "group-form-save",
                save_label,
                can_save,
                cx.listener(|this, _, _, cx| {
                    let Some((id, name)) = this.group_form.take() else {
                        return;
                    };
                    resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                        resident.dispatch(
                            ContactEvent::GroupSave {
                                input: ContactGroupInput {
                                    id,
                                    name: name.trim().to_owned(),
                                    color: None,
                                    // `None` leaves membership alone — a
                                    // rename must not empty the group.
                                    members: None,
                                },
                            },
                            cx,
                        );
                    });
                    cx.notify();
                }),
            ));

        Some(
            crate::ui::dialog::dialog(
                "group-form",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("group-form"),
                Self::closer(cx, |this, _| this.group_form = None),
            )
            .into_any_element(),
        )
    }

    /// 添加成员 from the open group.
    fn open_member_pick(&mut self, cx: &mut Context<Self>) {
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let Some(group) = self.group.and_then(|index| view.groups.get(index)) else {
            return;
        };
        self.pick = Some(PickDialog {
            subject: PickSubject::Members {
                group_id: group.id.clone(),
            },
            checked: group
                .members
                .iter()
                .map(|member| member.address.to_lowercase())
                .collect(),
            query: String::new(),
        });
        self.menu = None;
        cx.notify();
    }

    /// 移入分组 for one contact.
    fn open_group_pick(&mut self, address: &str, cx: &mut Context<Self>) {
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let lower = address.to_lowercase();
        self.pick = Some(PickDialog {
            subject: PickSubject::Groups {
                address: address.to_owned(),
            },
            checked: view
                .groups
                .iter()
                .filter(|group| {
                    group
                        .members
                        .iter()
                        .any(|member| member.address.to_lowercase() == lower)
                })
                .map(|group| group.id.clone())
                .collect(),
            query: String::new(),
        });
        self.menu = None;
        cx.notify();
    }

    /// The web's `PickList` in its desktop dialog (078 C-06): every contact
    /// or every group, the current ones ticked, a search past six rows, and a
    /// Save that hands the WHOLE ticked set to the core — which is the shape
    /// both of its events take, and which it normalises.
    fn pick_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let pick = self.pick.clone()?;
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let s = &self.contacts;
        let (title, rows): (SharedString, Vec<PickRow>) = match &pick.subject {
            PickSubject::Members { .. } => (
                s.add_member.clone(),
                contacts_live::rows(&view)
                    .into_iter()
                    .map(|row| {
                        let address = row.address_full.to_string();
                        PickRow {
                            id: address.to_lowercase(),
                            name: row.name.clone(),
                            detail: SharedString::from(crate::wallet::live::shorten_address(
                                &address,
                            )),
                            seed: Some(address),
                        }
                    })
                    .collect(),
            ),
            PickSubject::Groups { .. } => (
                s.move_group.clone(),
                view.groups
                    .iter()
                    .map(|group| PickRow {
                        id: group.id.clone(),
                        name: SharedString::from(group.name.clone()),
                        detail: SharedString::from(crate::wallet::fill(
                            &s.group_members,
                            "count",
                            &group.members.len().to_string(),
                        )),
                        seed: None,
                    })
                    .collect(),
            ),
        };
        let total = rows.len();
        let query = pick.query.trim().to_lowercase();
        let shown: Vec<_> = rows
            .into_iter()
            .filter(|row| {
                query.is_empty()
                    || format!("{} {}", row.name, row.detail)
                        .to_lowercase()
                        .contains(&query)
            })
            .collect();
        let empty = s.group_no_contacts.clone();
        let placeholder = s.search_placeholder.clone();
        let save_label = s.save.clone();

        let mut body = div().flex().flex_col().gap(px(8.)).pb(px(16.));
        if total > 6 {
            let page = cx.entity().downgrade();
            let field = crate::flows::panels::AddressField {
                focus: self.pick_query_focus.clone(),
                value: pick.query.clone(),
                placeholder: placeholder.clone(),
                on_change: Box::new(move |text: String, _: &mut Window, cx: &mut gpui::App| {
                    let _ = page.update(cx, |this, cx| {
                        if let Some(pick) = this.pick.as_mut() {
                            pick.query = text;
                        }
                        cx.notify();
                    });
                }),
            };
            body = body.child(crate::flows::components::flow_search(
                theme,
                &mut self.icons,
                placeholder,
                Some(field),
                window,
            ));
        }
        if total == 0 {
            body = body.child(
                div()
                    .py(px(20.))
                    .text_center()
                    .text_size(theme::text_body())
                    .text_color(theme.fg_muted)
                    .child(empty),
            );
        } else {
            let mut list = div()
                .id("pick-list")
                .flex()
                .flex_col()
                .max_h(window.viewport_size().height * 0.5)
                .overflow_y_scroll();
            for (
                index,
                PickRow {
                    id,
                    name,
                    detail,
                    seed,
                },
            ) in shown.into_iter().enumerate()
            {
                let on = pick.checked.contains(&id);
                let avatar: gpui::AnyElement = match seed {
                    Some(seed) => crate::wallet::components::identicon_avatar(
                        &mut self.identicons,
                        &seed,
                        30.,
                    )
                    .into_any_element(),
                    None => div()
                        .size(px(30.))
                        .flex_none()
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(theme.bg_raised)
                        .child(icon_img(
                            &mut self.icons,
                            Icon::UsersRound,
                            false,
                            theme.fg_muted,
                            18.,
                        ))
                        .into_any_element(),
                };
                let mut tick = div()
                    .size(px(20.))
                    .flex_none()
                    .rounded(px(4.))
                    .border_1()
                    .flex()
                    .items_center()
                    .justify_center();
                tick = if on {
                    tick.border_color(theme.accent)
                        .bg(theme.accent)
                        .child(icon_img(
                            &mut self.icons,
                            Icon::Check,
                            false,
                            gpui::Hsla::from(gpui::rgb(0xffffff)),
                            14.,
                        ))
                } else {
                    tick.border_color(theme.border_strong)
                };
                let raised = theme.bg_raised;
                list = list.child(
                    div()
                        .id(ElementId::from(("pick-row", index)))
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .py(px(8.))
                        .px(px(4.))
                        .border_b_1()
                        .border_color(theme.divider)
                        .cursor_pointer()
                        .hover(move |el| el.bg(raised))
                        .child(avatar)
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(0.))
                                .flex()
                                .flex_col()
                                .gap(px(2.))
                                .child(
                                    div()
                                        .text_size(theme::text_body())
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(theme.fg_base)
                                        .overflow_hidden()
                                        .whitespace_nowrap()
                                        .child(name),
                                )
                                .child(
                                    div()
                                        .font_family(theme::font_mono())
                                        .text_size(theme::text_label())
                                        .text_color(theme.fg_muted)
                                        .child(detail),
                                ),
                        )
                        .child(tick)
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(pick) = this.pick.as_mut() {
                                if let Some(at) = pick.checked.iter().position(|c| *c == id) {
                                    pick.checked.remove(at);
                                } else {
                                    pick.checked.push(id.clone());
                                }
                            }
                            cx.notify();
                        })),
                );
            }
            body = body.child(list);
        }
        body = body.child(self.form_save(
            theme,
            "pick-save",
            save_label,
            true,
            cx.listener(|this, _, _, cx| {
                let Some(pick) = this.pick.take() else {
                    return;
                };
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    let event = match pick.subject {
                        PickSubject::Members { group_id } => ContactEvent::SetGroupMembers {
                            id: group_id,
                            members: pick.checked,
                        },
                        PickSubject::Groups { address } => ContactEvent::SetContactGroups {
                            address,
                            group_ids: pick.checked,
                        },
                    };
                    resident.dispatch(event, cx);
                });
                cx.notify();
            }),
        ));

        Some(
            crate::ui::dialog::dialog(
                "contact-pick",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("contact-pick"),
                Self::closer(cx, |this, _| this.pick = None),
            )
            .into_any_element(),
        )
    }

    /// A contact's address as a code, the way the receive card does our own.
    ///
    /// The three pills on the contact panel were drawn in 018 and none of them
    /// did anything until 2026-09-23. 转账 and 收款 had another route (the row's
    /// context menu); **二维码 had none at all**, which is why it needed a
    /// surface rather than a handler. The web's sheet is the model: their
    /// identicon, their name, their address encoded — a picture somebody can
    /// hold up to a phone.
    ///
    /// The identicon is not decoration. It is the anti-forgery mark the receive
    /// redesign leans on: two addresses that read alike do not draw alike.
    fn contact_qr_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (name, address) = self.contact_qr.clone()?;
        let s = &self.contacts;
        let title = s.action_qr.clone();
        let copy_label = if self.copied.as_deref() == Some(CONTACT_QR_COPY) {
            s.copied.clone()
        } else {
            s.copy_address.clone()
        };

        // 21 modules for a 42-character address is not a given, so the module
        // size is derived from the code's own width rather than assumed.
        let code = qrcode::QrCode::new(address.as_bytes()).ok();
        let matrix: Div = match code {
            Some(code) => {
                let width = code.width();
                let module = (220.0 / width as f32).floor().max(2.0);
                let colors = code.to_colors();
                let mut grid = div().flex().flex_col().p(px(12.)).bg(gpui::rgb(0xffffff));
                for row in 0..width {
                    let mut line = div().flex().flex_row();
                    for col in 0..width {
                        let dark =
                            matches!(colors.get(row * width + col), Some(qrcode::Color::Dark));
                        let mut cell = div().size(px(module));
                        if dark {
                            cell = cell.bg(gpui::rgb(0x000000));
                        }
                        line = line.child(cell);
                    }
                    grid = grid.child(line);
                }
                grid
            }
            // An address that cannot be encoded is a bug elsewhere; drawing a
            // fake pattern would be worse than drawing nothing, because a fake
            // one gets photographed.
            None => div(),
        };

        // The web's `ContactQr`: the code, the name at 17 bold, the address in
        // full in the mono face, and a hairline pill that copies it.
        let body = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(8.))
            .pb(px(16.))
            .child(crate::wallet::components::identicon_avatar(
                &mut self.identicons,
                &address,
                56.,
            ))
            .child(
                div()
                    .mt(px(8.))
                    .rounded(px(14.))
                    .overflow_hidden()
                    .child(matrix),
            )
            .child(
                div()
                    .mt(px(8.))
                    .text_size(theme::text_button())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(name),
            )
            .child(
                div()
                    .max_w(px(300.))
                    .font_family(theme::font_mono())
                    .text_size(theme::text_label())
                    .text_color(theme.fg_muted)
                    .text_center()
                    .child(address.clone()),
            )
            .child(
                div()
                    .id("contact-qr-copy")
                    .h(px(36.))
                    .px(px(16.))
                    .flex()
                    .items_center()
                    .rounded_full()
                    .border_1()
                    .border_color(theme.border_card)
                    .bg(theme.bg_raised)
                    .cursor_pointer()
                    .hover(|el| el.opacity(0.92))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.copy_text(
                            CONTACT_QR_COPY,
                            address.to_string(),
                            CONTACTS_COPY_HOLD,
                            cx,
                        );
                    }))
                    .child(copy_label),
            );

        Some(
            crate::ui::dialog::dialog(
                "contact-qr",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("contact-qr"),
                Self::closer(cx, |this, _| this.close_contact_qr()),
            )
            .into_any_element(),
        )
    }

    fn close_contact_qr(&mut self) {
        self.contact_qr = None;
    }

    /// The add/edit contact form — the third column, as the web draws it
    /// (078 C-05): "a sheet sliding up the bottom of a desktop window is a
    /// phone control at the wrong size", and a dialog over the list hid the
    /// list the person was adding to.
    ///
    /// The ADDRESS is the identity the core keys on, so an edit keeps it fixed:
    /// changing it would be a delete and an add wearing one button, and the old
    /// contact would quietly survive.
    fn contact_form_body(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<(SharedString, bool, Div)> {
        let form = self.contact_form.clone()?;
        let s = &self.contacts;
        let title = if form.editing {
            if form.unsaved {
                s.save_to_contacts.clone()
            } else {
                s.edit_title.clone()
            }
        } else {
            s.add_title.clone()
        };
        // The web's gate, both halves: an address the core's `is_address`
        // accepts, AND a name — a contact nobody named is the history row it
        // already was.
        let valid_address = is_evm_address(&form.address);
        let can_save = valid_address && !form.name.trim().is_empty();
        let name_focus = self.contact_form_name_focus.clone();
        let address_focus = self.contact_form_address_focus.clone();

        let name_strings = crate::ui::NameFieldStrings {
            label: s.name_label.clone(),
            placeholder: s.name_placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let address_strings = crate::ui::NameFieldStrings {
            label: s.address_label.clone(),
            placeholder: s.address_placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: s.invalid_address.clone(),
        };

        let mut card = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .pb(px(16.))
            // Tab, between the two fields this form has. Nothing in this shell
            // bound it, so a person who typed a name and pressed Tab — the
            // reflex every other form on their machine has taught them —
            // stayed in the name field and typed the address into it. Handled
            // here rather than in `text_field`, because "what is next" is a
            // property of the form, not of a well: `tab_index` would have to
            // be assigned across the whole window to mean anything.
            .on_key_down({
                let name = name_focus.clone();
                let address = address_focus.clone();
                let editing = form.editing;
                move |event: &gpui::KeyDownEvent, window, cx| {
                    let ks = &event.keystroke;
                    if ks.key != "tab"
                        || ks.modifiers.platform
                        || ks.modifiers.control
                        || ks.modifiers.alt
                    {
                        return;
                    }
                    // An edited contact's address is fixed and not a field, so
                    // there is nowhere for Tab to go.
                    if editing {
                        return;
                    }
                    // Two fields, so Tab is a toggle: pressing it twice is
                    // where you started, and there is no separate backward
                    // move to bind. ⇧Tab is the same toggle when it arrives —
                    // which on this gpui it does not: a keystroke with no
                    // `key_char` (⇧Tab has none) goes to the input context
                    // first and is swallowed there, so the app never sees it
                    // (`gpui_macos/window.rs::handle_key_event`).
                    if name.is_focused(window) {
                        address.focus(window, cx);
                    } else {
                        name.focus(window, cx);
                    }
                    cx.stop_propagation();
                }
            })
            .child(crate::ui::text_field(
                "contact-form-name",
                theme,
                &name_strings,
                &form.name,
                false,
                false,
                &name_focus,
                window,
                {
                    let page = cx.entity();
                    move |text: String, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this: &mut Self, cx| {
                            if let Some(form) = this.contact_form.as_mut() {
                                form.name = text.clone();
                            }
                            cx.notify();
                        });
                    }
                },
            ));

        if form.editing {
            // Fixed, and shown as such — the web's `.fixed-address`: a small
            // muted label over the address in the mono face, whole.
            card = card.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_muted)
                            .child(s.address_label.clone()),
                    )
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_base)
                            .child(SharedString::from(form.address.clone())),
                    ),
            );
        } else {
            card = card.child(crate::ui::text_field(
                "contact-form-address",
                theme,
                &address_strings,
                &form.address,
                // Red, and saying why, once there is something typed that is
                // not an address — not while the field is still empty, which
                // is a person who has not started rather than one who is wrong.
                !form.address.is_empty() && !valid_address,
                false,
                &address_focus,
                window,
                {
                    let page = cx.entity();
                    move |text: String, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this: &mut Self, cx| {
                            if let Some(form) = this.contact_form.as_mut() {
                                form.address = text.trim().to_owned();
                            }
                            cx.notify();
                        });
                    }
                },
            ));
        }

        let save = self.form_save(
            theme,
            "contact-form-save",
            s.save.clone(),
            can_save,
            cx.listener(|this, _, _, cx| {
                let Some(form) = this.contact_form.take() else {
                    return;
                };
                let now_ms = crate::executor::now_ms();
                let address = form.address.clone();
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(
                        ContactEvent::Save {
                            input: ContactSaveInput {
                                address: form.address,
                                // An empty name CLEARS the name — the core
                                // reads `Some("")` that way, and a person who
                                // deleted the text meant to.
                                name: Some(form.name.trim().to_owned()),
                                note: None,
                                favorite: None,
                                kind: None,
                                resolved_name: None,
                                resolved_source: None,
                            },
                            now_ms,
                        },
                        cx,
                    );
                });
                // The column that held the form shows what it made, as the
                // web's does on the desktop.
                this.open_contact_by_address(&address, cx);
                cx.notify();
            }),
        );

        Some((title, form.editing, card.child(save)))
    }

    /// 编辑 on a contact: the form over the core's RECORD, not the row's
    /// display name — the display of a nameless contact is its short address,
    /// and saving that back would name them "0x4444…4444".
    fn open_edit_contact(&mut self, address: &str, window: &mut Window, cx: &mut Context<Self>) {
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let Some(contact) = view
            .contacts
            .iter()
            .find(|contact| contact.address.eq_ignore_ascii_case(address))
        else {
            return;
        };
        self.contact_form = Some(ContactForm {
            editing: true,
            address: contact.address.clone(),
            name: contact.name.clone().unwrap_or_default(),
            unsaved: contact.source == vela_core::app::contacts::ContactSource::Auto,
        });
        self.menu = None;
        window.focus(&self.contact_form_name_focus, cx);
        cx.notify();
    }

    /// What the last address-book import did.
    ///
    /// The RN screen alerts; the desktop has a centred-dialog idiom already, so
    /// this reuses the sign-out dialog's shape with one button. The counts are
    /// the CORE's — it applied existing-wins and is the only thing that knows
    /// how many rows were new.
    fn import_result_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (title, body) = self.import_result.clone()?;
        let done = crate::flows::components::accent_button(theme, self.flow_strings.done.clone())
            .w_auto()
            .id("import-result-ok")
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.import_result = None;
                cx.notify();
            }));
        Some(
            crate::ui::dialog::dialog(
                "import-result",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                div()
                    .flex()
                    .flex_col()
                    .child(crate::ui::dialog::dialog_body(theme, body))
                    .child(crate::ui::dialog::dialog_actions().child(done)),
                &self.dialog_scroll("import-result"),
                Self::closer(cx, |this, _| this.import_result = None),
            )
            .into_any_element(),
        )
    }

    fn sign_out_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let view = session::view(cx);
        let dialog = view.sign_out?;
        let s = &self.strings;

        // What this takes, when it is more than one wallet, goes first; `keeps`
        // is true either way — the address returns, the history is still there
        // — and on a machine holding six it was ALSO how the dialog managed to
        // say nothing about signing in six times (owner, 2026-09-23).
        let (body, note) = if dialog.account_count > 1 {
            (
                SharedString::from(
                    s.sign_out_desc_many
                        .replace("{{count}}", &dialog.account_count.to_string()),
                ),
                Some(s.sign_out_keeps.clone()),
            )
        } else {
            (s.sign_out_keeps.clone(), None)
        };
        // The destructive label changes with the warning, as the shipping
        // client does: "Sign Out Anyway" is the acknowledgement.
        let (confirm, callout) = if dialog.pending_upload_warning {
            (s.sign_out_anyway.clone(), Some(s.sign_out_warning.clone()))
        } else {
            (s.sign_out_title.clone(), None)
        };
        let copy = ConfirmCopy {
            title: SharedString::default(),
            body,
            callout,
            note,
            confirm,
            cancel: Some(s.sign_out_cancel.clone()),
            danger: true,
        };
        let title = s.sign_out_title.clone();
        let sheet = confirm_sheet(
            theme,
            copy,
            cx.listener(|_, _: &gpui::ClickEvent, _, cx| session::sign_out_confirmed(cx)),
            cx.listener(|_, _: &gpui::ClickEvent, _, cx| session::sign_out_dismissed(cx)),
        );
        Some(
            crate::ui::dialog::dialog(
                "sign-out",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                sheet,
                &self.dialog_scroll("sign-out"),
                Self::closer(cx, |_, cx| session::sign_out_dismissed(cx)),
            )
            .into_any_element(),
        )
    }

    /// 078 H-01 — the account switcher, from the sidebar header: the web's
    /// `AccountSwitcher` in its desktop dress, a centred dialog. Every account
    /// on this device, the active one checked, and the two ways to add one.
    fn account_switcher_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.account_switcher {
            return None;
        }
        let session = session::view(cx);
        // Signed out under it, or the last one removed: nothing to switch.
        if session.accounts.is_empty() {
            self.account_switcher = false;
            self.removing_account = None;
            return None;
        }
        let title = self.settings.accounts_title.clone();
        let body = self.accounts_body(theme, &session, true, cx);
        let close_icon = icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.);
        Some(
            crate::ui::dialog::dialog(
                "account-switcher",
                theme,
                window,
                title,
                None,
                close_icon,
                body,
                &self.switcher_scroll,
                cx.listener(|this, _, _, cx| this.close_account_switcher(cx)),
            )
            .into_any_element(),
        )
    }

    /// 078 H-02 — the identicon, big, next to the address that drew it: the
    /// web's `IdenticonViewer`. The artwork is a fingerprint of the address,
    /// which only becomes useful once a person has seen the two together
    /// often enough to recognise one from the other; a 40px avatar in a
    /// header never teaches that.
    ///
    /// As on the web, Escape and Close close it and a click beside the card
    /// does not: it sits over the switcher, and a stray click there would be
    /// read as meant for the list.
    fn identicon_viewer_dialog(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let viewer = self.identicon_viewer.clone()?;
        let s = &self.strings;
        let title = s.viewer_title.clone();
        let caption = s.viewer_caption.clone();
        let close = s.close_viewer.clone();
        let copy = if viewer.copied.is_some() {
            s.viewer_copied.clone()
        } else {
            s.copy_address.clone()
        };
        let body = div()
            .flex()
            .flex_col()
            .items_center()
            .gap(px(12.))
            .text_center()
            .child(
                div()
                    .mb(px(8.))
                    .child(crate::wallet::components::identicon_avatar(
                        &mut self.identicons,
                        &viewer.address,
                        VIEWER_IDENTICON,
                    )),
            )
            .child(
                div()
                    .text_size(theme::text_section())
                    .font_weight(gpui::FontWeight::BOLD)
                    .line_height(gpui::relative(1.2))
                    .text_color(theme.fg_base)
                    .child(title),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .line_height(gpui::relative(1.6))
                    .text_color(theme.fg_muted)
                    .child(caption),
            )
            // The whole address, never shortened: a fingerprint you can only
            // see half of teaches half a habit.
            .child(
                div()
                    .w_full()
                    .p(px(12.))
                    .rounded(px(12.))
                    .bg(theme.bg_sunken)
                    .font_family(theme::font_mono())
                    .text_size(theme::text_label())
                    .line_height(gpui::relative(1.6))
                    .text_color(theme.fg_base)
                    .child(SharedString::from(viewer.address.clone())),
            )
            .child(
                div()
                    .w_full()
                    .mt(px(8.))
                    .flex()
                    .flex_col()
                    .gap(px(12.))
                    .child(
                        crate::flows::components::accent_button(theme, copy)
                            .id("identicon-viewer-copy")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.copy_viewer_address(cx);
                            })),
                    )
                    .child(
                        crate::flows::components::ghost_button(theme, close)
                            .rounded(px(12.))
                            .id("identicon-viewer-close")
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.identicon_viewer = None;
                                cx.notify();
                            })),
                    ),
            );
        Some(
            crate::ui::dialog::scrim("identicon-viewer-scrim", theme)
                .child(
                    crate::ui::dialog::card(
                        "identicon-viewer-card",
                        theme,
                        crate::ui::dialog::PROMPT_CARD_W,
                    )
                    .child(body),
                )
                .into_any_element(),
        )
    }

    /// "Remove this custom network?" — the confirmation the core hands to the
    /// shell in so many words.
    ///
    /// The web dispatches the delete straight off the trash icon. This asks
    /// first, for the reason every other destructive action in this shell asks:
    /// the row carries a chain someone typed an endpoint for, one press away
    /// from a glyph they may have meant to open the card with. The words are
    /// the corpus's own (`settingsModals.network.remove*`), translated in every
    /// locale since the phone drew this dialog.
    fn network_remove_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (id, name) = self.network_remove.clone()?;
        let s = &self.settings;
        let copy = ConfirmCopy {
            title: SharedString::default(),
            // The corpus asks "Remove this custom network?"; the name says
            // WHICH, because the dialog covers the row it is about.
            body: SharedString::from(format!("{} · {name}", s.network_remove_body)),
            callout: None,
            note: None,
            confirm: s.network_remove_confirm.clone(),
            cancel: Some(s.network_remove_cancel.clone()),
            danger: true,
        };
        let title = s.network_remove_title.clone();
        let sheet = confirm_sheet(
            theme,
            copy,
            cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                    resident.dispatch(NetEvent::DeleteConfirmed { id: id.clone() }, cx);
                });
                // The hero was counting that chain a moment ago.
                crate::executor::balance_dashboard::refresh(cx);
                this.network_remove = None;
                this.settings_expanded_network = None;
                cx.notify();
            }),
            cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.network_remove = None;
                cx.notify();
            }),
        );
        Some(
            crate::ui::dialog::dialog(
                "network-remove",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                sheet,
                &self.dialog_scroll("network-remove"),
                Self::closer(cx, |this, _| this.network_remove = None),
            )
            .into_any_element(),
        )
    }

    /// Where the person is: the section, and which settings panel.
    pub fn place(&self) -> (Section, SettingsPage) {
        (self.section, self.settings_page)
    }

    /// A page built for another account opens where the last one was left —
    /// a switch from Settings stays on Settings.
    pub fn restore_place(&mut self, (section, settings_page): (Section, SettingsPage)) {
        self.section = section.or_wallet();
        self.settings_page = settings_page;
    }

    /// Resolve every string again, in the language now in force (spec 072):
    /// a language chosen in Settings applies on the next frame, not the next
    /// launch.
    fn relocalize(&mut self) {
        let loc = Loc::from_env();
        self.strings = WalletStrings::resolve(&loc);
        self.contacts = ContactsStrings::resolve(&loc);
        self.settings = SettingsStrings::resolve(&loc);
        self.explore = ExploreStrings::resolve(&loc);
        self.signing = SigningStrings::resolve(&loc);
        self.flow_strings = FlowStrings::resolve(&loc);
        self.locale = gpui::SharedString::from(loc.language().to_owned());
        self.loc = loc;
    }

    /// Watch each settings field for the moment it is left (spec 072). Made
    /// here because a subscription needs the window mutably, and a field's
    /// handle is made while it is drawn — the frame after, before anybody can
    /// have typed into it.
    fn watch_field_blurs(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        while self.field_blurs.len() < self.endpoint_focuses.len() {
            let index = self.field_blurs.len();
            let handle = self.endpoint_focuses[index].clone();
            let subscription = cx.on_blur(&handle, window, move |this, _, cx| {
                this.commit_field(index, cx);
            });
            self.field_blurs.push(subscription);
        }
    }

    /// An edited field was left: tell the core it may persist it.
    fn commit_field(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(field) = self.field_commits.remove(&index) else {
            return;
        };
        let saved = resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
            resident.dispatch(field.committed(), cx);
            // Refused endpoints do not count as a fix. `rpc_chain_mismatch`
            // is the core's own verdict on the one refusal that matters — an
            // endpoint answering for another chain.
            match field {
                settings_live::FieldCommit::Override { chain_id, .. } => resident
                    .view()
                    .networks
                    .iter()
                    .find(|row| row.chain_id == chain_id)
                    .is_none_or(|row| row.rpc_chain_mismatch.is_none()),
                _ => true,
            }
        });
        if let settings_live::FieldCommit::Override {
            chain_id,
            field: NetOverrideField::Rpc,
        } = field
            && saved
        {
            // The hero has this chain marked failed and its own retry is
            // throttled like any other fetch. The person just repaired the
            // endpoint by hand, which is the moment the web clears the failure
            // and forces one read.
            crate::executor::balance_dashboard::dispatch(
                vela_core::app::balance_dashboard::Event::FixChainResolved { chain_id },
                cx,
            );
            crate::executor::balance_dashboard::refresh(cx);
        }
        cx.notify();
    }

    /// One keystroke into an editable settings field: the core gets the draft,
    /// and the page remembers the field owes a commit.
    fn edit_field(
        page: &gpui::Entity<Self>,
        index: usize,
        field: settings_live::FieldCommit,
        text: String,
        cx: &mut gpui::App,
    ) {
        page.update(cx, |this, _| {
            this.field_commits.insert(index, field);
        });
        resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
            resident.dispatch(field.edited(text), cx);
        });
    }

    /// Enter in a settings field: done with it — which is leaving it, so the
    /// commit is the blur's, and happens once.
    fn enter_leaves(page: &gpui::Entity<Self>) -> impl Fn(&mut Window, &mut gpui::App) + 'static {
        let page = page.clone();
        move |window, cx| {
            let focus = page.read(cx).focus_handle.clone();
            focus.focus(window, cx);
        }
    }

    /// "Erase this device", confirmed (spec 072 FR-011).
    ///
    /// Live dApp sessions first — every grant revoked through the browser
    /// machine, so an open page hears `accountsChanged []` and `disconnect`,
    /// and the page on screen closed — then the sweep, which VERIFIES. Only a
    /// clean store leaves for the first run: something surviving keeps the
    /// person here, signed in, told so in the dialog, with the button live.
    fn erase_device(&mut self, cx: &mut Context<Self>) {
        if let Some(host) = self.browser_host.clone() {
            host.update(cx, |host, cx| host.dispatch(DbrEvent::RevokeAll, cx));
        }
        self.close_browser_page(cx);
        // The browser's OWN store — cookies, localStorage, databases, caches —
        // which no `vela.` key names and an erase that skipped it left behind,
        // so somebody was still signed in to the sites they had visited (main,
        // spec 081 FR-017). On Linux there is no web view and this says so.
        if !crate::webview::clear_browsing_data() {
            eprintln!("[vela-wallet] erase: no web view to clear; browsing data untouched");
        }
        match crate::executor::device_storage::erase() {
            Ok(removed) => {
                eprintln!("[vela-wallet] erase: {} key(s) removed", removed.len());
                self.confirm = None;
                self.settings_dialog = None;
                self.erase_failed = None;
                // What the process still holds about this wallet goes back to
                // a first launch's: the preferences, the formats, a saved
                // index endpoint, the endpoint pools.
                crate::executor::preferences::load();
                format_prefs::reload();
                crate::executor::registry::set_registry_url("");
                crate::executor::pool::refresh(None);
                // No wallet on disk: the session reads that, and the root
                // takes the window to the first run. `reboot` is the whole of
                // it — a `sign_out` alone only opens the confirmation, which is
                // how a wiped machine ended up under a modal asking whether to
                // sign out.
                session::reboot(cx);
            }
            Err(incomplete) => {
                eprintln!(
                    "[vela-wallet] erase incomplete: {} key(s) survived: {:?}",
                    incomplete.remaining.len(),
                    incomplete.remaining
                );
                // WHICH keys, not merely that some did: the dialog prints them,
                // and "something is still here" with no names is not something
                // a person can act on (main, 081).
                self.erase_failed = Some(incomplete.remaining);
            }
        }
        cx.notify();
    }

    /// One storage row's Clear — your data after its question, a cache at
    /// once — and then the machines that mirrored those keys in memory
    /// forget them, so nothing writes a cleared list back.
    fn clear_storage_row(&mut self, id: &'static str, cx: &mut Context<Self>) {
        if let Err(error) = crate::executor::device_storage::clear_item(id) {
            eprintln!("[vela-wallet] storage: {id} could not be cleared: {error}");
        }
        match id {
            "transactions" => resident::forget::<ActivityFeed>(cx),
            "contacts" => resident::forget::<Contacts>(cx),
            "custom" => {
                resident::forget::<NetworkAdmin>(cx);
                resident::forget::<ManageTokens>(cx);
                crate::executor::balance_dashboard::refresh(cx);
            }
            "browsing" => {
                resident::forget::<BrowserHistory>(cx);
                resident::forget::<ExploreSites>(cx);
            }
            _ => crate::executor::balance_dashboard::refresh(cx),
        }
        cx.notify();
    }

    /// "Clear all caches", confirmed — and a fresh read, since the balance
    /// cache the hero paints from is one of them.
    fn clear_caches(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = crate::executor::device_storage::clear_caches() {
            eprintln!("[vela-wallet] storage: caches could not be cleared: {error}");
        }
        crate::executor::balance_dashboard::refresh(cx);
        cx.notify();
    }

    /// The pending question, answered yes.
    fn confirmed(&mut self, cx: &mut Context<Self>) {
        let Some(action) = self.confirm.clone() else {
            return;
        };
        match action {
            // The dialog stays until the erase has an answer.
            Confirm::ClearItem(id) => self.clear_storage_row(id, cx),
            Confirm::ClearCaches => self.clear_caches(cx),
            Confirm::Disconnect { origin, .. } => {
                let host = self.browser_host(cx);
                host.update(cx, |host, cx| {
                    host.dispatch(DbrEvent::RevokeRequested { origin }, cx);
                });
            }
            Confirm::DisconnectAll => {
                let host = self.browser_host(cx);
                host.update(cx, |host, cx| host.dispatch(DbrEvent::RevokeAll, cx));
            }
            Confirm::ResetEndpoints => {
                resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                    resident.dispatch(NetEvent::ResetEndpointsToDefaults, cx);
                });
            }
            Confirm::DeleteContact { address, .. } => {
                let now_ms = crate::executor::now_ms();
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(ContactEvent::Delete { address, now_ms }, cx);
                });
                // A detail panel open on it was about a row that is gone.
                if self.panel == PanelId::ContactDetail {
                    self.panel = PanelId::None;
                }
            }
            Confirm::DeleteGroup { id, .. } => {
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(ContactEvent::GroupDelete { id }, cx);
                });
                // The group that was open no longer exists; the rail falls
                // back to the whole book rather than to whichever group slid
                // into that index.
                self.group = None;
            }
        }
        self.confirm = None;
        cx.notify();
    }

    /// What each question says — every word the corpus's, the web phone's
    /// confirm sheets: a row names itself and carries its group's consequence.
    fn confirm_copy(&self, action: &Confirm) -> ConfirmCopy {
        let s = &self.settings;
        match action {
            Confirm::ClearItem(id) => ConfirmCopy {
                title: settings_live::storage_item_label(id, s),
                body: s.storage_user_data.clone(),
                callout: None,
                note: None,
                confirm: s.storage_clear.clone(),
                cancel: Some(s.cancel.clone()),
                danger: true,
            },
            Confirm::ClearCaches => ConfirmCopy {
                title: s.storage_clear_title.clone(),
                body: s.storage_clear_body.clone(),
                callout: None,
                note: None,
                confirm: s.storage_clear_confirm.clone(),
                cancel: Some(s.cancel.clone()),
                danger: false,
            },
            Confirm::Disconnect { name, .. } => ConfirmCopy {
                title: name.clone(),
                body: s.storage_connections.clone(),
                callout: None,
                note: None,
                confirm: self.explore.disconnect.clone(),
                cancel: Some(s.cancel.clone()),
                danger: true,
            },
            Confirm::DisconnectAll => ConfirmCopy {
                title: s.item_dapps.clone(),
                body: s.storage_connections.clone(),
                callout: None,
                note: None,
                confirm: s.storage_disconnect_all.clone(),
                cancel: Some(s.cancel.clone()),
                danger: true,
            },
            Confirm::ResetEndpoints => ConfirmCopy {
                title: s.endpoints_reset_title.clone(),
                body: s.endpoints_reset_body.clone(),
                callout: None,
                note: None,
                confirm: s.endpoints_reset_confirm.clone(),
                cancel: Some(s.endpoints_reset_cancel.clone()),
                danger: true,
            },
            Confirm::DeleteContact { name, .. } => ConfirmCopy {
                title: self.contacts.delete_title.clone(),
                body: SharedString::from(crate::wallet::fill(
                    &self.contacts.delete_body,
                    "name",
                    name,
                )),
                callout: None,
                note: None,
                confirm: self.contacts.delete.clone(),
                cancel: None,
                danger: true,
            },
            Confirm::DeleteGroup { name, .. } => ConfirmCopy {
                title: self.contacts.group_delete.clone(),
                body: SharedString::from(crate::wallet::fill(
                    &self.contacts.group_delete_body,
                    "name",
                    name,
                )),
                callout: None,
                note: None,
                confirm: self.contacts.delete.clone(),
                cancel: None,
                danger: true,
            },
        }
    }

    /// The question over the window, when one is pending.
    fn confirm_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let action = self.confirm.as_ref()?;
        let mut copy = self.confirm_copy(action);
        let title = std::mem::take(&mut copy.title);
        let sheet = confirm_sheet(
            theme,
            copy,
            cx.listener(|this, _: &gpui::ClickEvent, _, cx| this.confirmed(cx)),
            cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.dismiss_confirm();
                cx.notify();
            }),
        );
        Some(
            crate::ui::dialog::dialog(
                "confirm",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                sheet,
                &self.dialog_scroll("confirm"),
                Self::closer(cx, |this, _| this.dismiss_confirm()),
            )
            .into_any_element(),
        )
    }

    fn dismiss_confirm(&mut self) {
        self.confirm = None;
        self.erase_failed = None;
    }

    /// The ✕ every dialog carries (`--icon-md`, `fg-muted`).
    fn dialog_close_icon(&mut self, theme: &Theme) -> gpui::AnyElement {
        icon_img(&mut self.icons, Icon::X, false, theme.fg_muted, 18.).into_any_element()
    }

    /// A dialog's close — its ✕ and its scrim — as the page's own change.
    fn closer<F: Fn(&mut Self, &mut Context<Self>) + 'static>(
        cx: &mut Context<Self>,
        close: F,
    ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static + use<F> {
        let page = cx.entity().downgrade();
        move |_, _, cx| {
            // Gone only if the page is: nothing left to close.
            let _ = page.update(cx, |this, cx| {
                close(this, cx);
                cx.notify();
            });
        }
    }

    /// A dialog's body scroll, kept per dialog so two stacked ones do not
    /// share a position.
    fn dialog_scroll(&mut self, id: &'static str) -> gpui::ScrollHandle {
        self.dialog_scrolls.entry(id).or_default().clone()
    }

    /// Escape: the dialog on top goes, and only that one — the same order
    /// `render` stacks them in, read from the top. False when none was open.
    fn dismiss_top_dialog(&mut self, cx: &mut Context<Self>) -> bool {
        if self.identicon_viewer.is_some() {
            self.identicon_viewer = None;
        } else if self.account_switcher {
            self.close_account_switcher(cx);
        } else if self.confirm.is_some() {
            self.dismiss_confirm();
        } else if self.network_remove.is_some() {
            self.network_remove = None;
        } else if session::view(cx).sign_out.is_some() {
            session::sign_out_dismissed(cx);
        } else if self.group_form.is_some() {
            self.group_form = None;
        } else if self.pick.is_some() {
            self.pick = None;
        } else if self.explore_form.is_some() {
            self.explore_form = None;
        } else if self.contact_qr.is_some() {
            self.close_contact_qr();
        } else if self.contact_form.is_some() {
            self.contact_form = None;
        } else if self.import_result.is_some() {
            self.import_result = None;
        } else if self.balance_detail_open {
            self.balance_detail_open = false;
        } else if self.settings_dialog.is_some() {
            self.close_settings_dialog(cx);
        } else {
            return false;
        }
        cx.notify();
        true
    }

    /// Close the settings dialog, and forget what the add-network wizard was
    /// doing.
    ///
    /// The core keeps the search text, the chosen chain and its check results
    /// until told otherwise — so reopening 添加网络 came back to somebody else's
    /// half-finished search. The web resets on the same gesture; the reset also
    /// orphans any probe still in flight, which is why it is the core's event
    /// and not a field cleared here.
    fn close_settings_dialog(&mut self, cx: &mut Context<Self>) {
        let was_add = self.settings_dialog == Some(SettingsDialog::AddNetwork);
        self.settings_dialog = None;
        if was_add && self.identity.is_some() {
            resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                resident.dispatch(NetEvent::WizardReset, cx);
            });
        }
    }

    fn theme_mode(&self) -> ThemeMode {
        self.override_mode.unwrap_or(self.mode)
    }

    // -- column 1: sidebar ---------------------------------------------------

    fn sidebar(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.strings;
        // Sunken in both themes, as the web's `.sidebar` is — dark sunken is
        // below the canvas now (078 X-01), so the old dark exception went.
        let bg = theme.bg_sunken;

        let section = self.section;
        let nav = [
            (Icon::NavWallet, s.nav_wallet.clone(), Some(Section::Wallet)),
            (
                Icon::NavContacts,
                s.nav_contacts.clone(),
                Some(Section::Contacts),
            ),
            (
                Icon::NavExplore,
                s.nav_explore.clone(),
                Some(Section::Explore),
            ),
            (
                Icon::NavSettings,
                s.nav_settings.clone(),
                Some(Section::Settings),
            ),
        ];
        let mut nav_col = div().flex().flex_col().gap(px(2.));
        let nav = nav
            .into_iter()
            .filter(|(_, _, destination)| destination.is_none_or(Section::available));
        for (i, (icon, label, destination)) in nav.enumerate() {
            let row = nav_row(
                ElementId::from(("nav", i)),
                theme,
                &mut self.icons,
                icon,
                label,
                destination == Some(section),
            );
            nav_col = nav_col.child(match destination {
                Some(destination) => row.on_click(cx.listener(move |this, _, _, cx| {
                    if destination != Section::Settings {
                        // Leaving 设置 leaves the accounts panel with it.
                        this.close_switcher(cx);
                    }
                    this.section = destination;
                    this.panel = PanelId::None;
                    this.menu = None;
                    cx.notify();
                })),
                None => row,
            });
        }

        let mut networks = div()
            .id("sidebar-networks")
            .track_scroll(&self.networks_scroll)
            .flex()
            .flex_col()
            .gap(px(2.))
            .size_full()
            .overflow_y_scroll();
        let chain_rows = self.chain_models(cx);
        let live = self.identity.is_some();
        for (i, row) in chain_rows.iter().enumerate() {
            let drawn = chain_row(ElementId::from(("chain", i)), theme, &mut self.icons, row);
            // The rows have looked pressable since spec 015 — pointer cursor,
            // hover tint, a check on the selected one — and nothing was
            // listening. The chain rides on the row rather than on its index:
            // the list re-sorts whenever a holding does.
            let chain_id = row.chain_id;
            networks = networks.child(if live {
                drawn
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_chain(chain_id, cx);
                    }))
                    .into_any_element()
            } else {
                drawn.into_any_element()
            });
        }

        div()
            .w(px(SIDEBAR_W))
            .h_full()
            .flex_none()
            .bg(bg)
            .border_r_1()
            .border_color(theme.divider)
            .p(px(SIDEBAR_PAD))
            .pt(px(SIDEBAR_TOP))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child({
                let identity = self.identity();
                // Both halves answer only for a real account: the gallery's
                // header is a picture, as the web's is.
                let (on_identicon, on_account): (
                    Option<crate::wallet::components::HeaderClick>,
                    Option<crate::wallet::components::HeaderClick>,
                ) = if live {
                    let address = identity.address.clone();
                    (
                        Some(Box::new(cx.listener(move |this, _, _, cx| {
                            this.open_identicon_viewer(address.clone(), cx);
                        }))),
                        Some(Box::new(cx.listener(|this, _, _, cx| {
                            this.open_account_switcher(cx);
                        }))),
                    )
                } else {
                    (None, None)
                };
                wallet_header(
                    theme,
                    &mut self.icons,
                    &mut self.identicons,
                    &identity.address,
                    identity.name.clone(),
                    identity.display(),
                    on_identicon,
                    on_account,
                )
            })
            .child(nav_col)
            // The network list is the Wallet's filter, as on the web (spec 028
            // Phase 9, RULING 2): a section with nothing to filter shows none,
            // and the rail ends at the nav.
            .when(self.section == Section::Wallet, |sidebar| {
                sidebar
                    .child(div().h(px(1.)).bg(theme.divider))
                    .child(
                        div()
                            .px(px(12.))
                            .text_size(theme::text_label())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_subtle)
                            .child(self.strings.networks_title.clone()),
                    )
                    .child(
                        div()
                            .relative()
                            .flex_1()
                            .min_h(px(0.))
                            .child(networks)
                            .children(crate::ui::vertical_scrollbar(theme, &self.networks_scroll)),
                    )
            })
    }

    // -- column 2: content ---------------------------------------------------

    fn content(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s_activity = self.strings.section_activity.clone();
        let s_assets = self.strings.section_assets.clone();
        let s_all = self.strings.action_all.clone();
        let s_add = self.strings.action_add.clone();

        let balance = self.balance_model(cx);
        let activity = self.activity_models(cx);
        let assets = self.asset_models(cx);

        let pills = div()
            .flex()
            .gap(px(12.))
            .max_w(px(600.))
            .pt(px(20.))
            .pb(px(24.))
            .child(
                action_pill(
                    "pill-receive",
                    theme,
                    &mut self.icons,
                    Icon::ArrowDownLeft,
                    self.strings.action_receive.clone(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.enter_flow(FlowEntry::Receive, cx);
                    cx.notify();
                })),
            )
            .child(
                action_pill(
                    "pill-send",
                    theme,
                    &mut self.icons,
                    Icon::ArrowUpRight,
                    self.strings.action_send.clone(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.enter_flow(FlowEntry::Send, cx);
                    cx.notify();
                })),
            )
            .child(
                action_pill(
                    "pill-scan",
                    theme,
                    &mut self.icons,
                    Icon::ScanLine,
                    self.strings.action_scan.clone(),
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.enter_flow(FlowEntry::Scan, cx);
                    cx.notify();
                })),
            );

        // The ids behind the preview rows, in the same order they draw. The
        // home drops the core's day headers, so row N here is feed item N.
        let home_tx_ids: Vec<String> = if self.identity.is_some() {
            wallet_live::history_item_ids(&resident::resident::<ActivityFeed>(cx).read(cx).view())
        } else {
            Vec::new()
        };
        // The row that just landed, still glowing. `new_item_id` outlives the
        // toast on purpose — the core clears the toast on its timer and never
        // the glow — so after the pill has gone there is still something on
        // screen that says WHICH row was the news.
        let fresh = self.celebrated_row(&home_tx_ids, cx);
        let mut activity_col = div().flex().flex_col();
        for (i, row) in activity.iter().enumerate() {
            // The day a group files under (the web's `.day`: 11, subtle,
            // 4 above and below) — spec 038 #E3, 078 H-04.
            if let Some(day) = row.day.clone() {
                activity_col = activity_col.child(
                    div()
                        .py(px(4.))
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(day),
                );
            }
            activity_col = activity_col.child(
                div()
                    .id(ElementId::from(("activity", i)))
                    .cursor_pointer()
                    .when(fresh == Some(i), |el| {
                        // A tint behind the row, not a border or a badge:
                        // nothing moves, so the list does not reflow when the
                        // glow lands or when it eventually stops mattering.
                        el.rounded(px(10.)).bg(theme.success_soft)
                    })
                    .child(activity_row(theme, &mut self.icons, row))
                    .on_click({
                        let id = home_tx_ids.get(i).cloned();
                        cx.listener(move |this, _, _, cx| {
                            this.tx_detail = id.clone();
                            this.enter_flow(FlowEntry::TxDetail, cx);
                            cx.notify();
                        })
                    }),
            );
        }

        // The strip is narrowed by the sidebar's filter, and the panel it opens
        // is addressed by index into the UNFILTERED list. Row 0 of "Gnosis" is
        // not holding 0 — so the mapping is carried, not assumed. Getting this
        // wrong opens somebody's ETH panel from their USDC row, and the next
        // thing that panel offers is 转账.
        let asset_indices = self.visible_assets(cx);
        let mut assets_col = div().flex().flex_col();
        for (i, row) in assets.iter().enumerate() {
            let index = asset_indices.get(i).copied();
            assets_col = assets_col.child(
                asset_row(ElementId::from(("asset", i)), theme, &mut self.icons, row).on_click(
                    cx.listener(move |this, _, _, cx| {
                        // WHICH holding, so the panel is about the row that was
                        // clicked rather than about the first one.
                        this.asset_detail = this.identity.is_some().then_some(()).and(index);
                        this.panel = PanelId::AssetDetail;
                        cx.notify();
                    }),
                ),
            );
        }
        // Nothing to list, and the core has said so — or the sidebar's chain
        // holds nothing while others do. The empty state rather than a blank
        // strip, which reads as a list that failed to load (the web's
        // `assetsSection.mode === 'empty'`).
        if assets.is_empty()
            && self.identity.is_some()
            && wallet_live::assets_strip_empty(
                &resident::resident::<BalanceDashboard>(cx).read(cx).view(),
                self.chain_filter,
            )
        {
            assets_col = assets_col.child(empty_state(
                theme,
                &mut self.icons,
                Icon::WalletOutline,
                self.strings.empty_assets_title.clone(),
                self.strings.empty_assets_caption.clone(),
            ));
        }

        // A column that scrolls, and a bar that says so. It was
        // `overflow_hidden`: every holding was laid out and whatever fell
        // below the window's edge was simply cut, with nothing to reach it by.
        // The web's column (`WalletDesktop.svelte`): the content is at most
        // `--layout-maxContentWidth` (800) and sits left, and everything
        // two-ended — the actions, both section headers, both lists — lives
        // in one `--layout-rowMeasure` (560), so no row is wider than the eye
        // can cross. The balance stays outside it: one-ended, and a large one
        // needs the room. Full-screen, the rows used to run the width of the
        // monitor with a token's name at one end and its amount at the other.
        let column = div()
            .id("wallet-content")
            .track_scroll(&self.content_scroll)
            .size_full()
            .overflow_y_scroll()
            .child(
                div()
                    .max_w(px(WALLET_CONTENT_MAX_W))
                    .pl(px(WALLET_PAD_X))
                    .pr(px(32.))
                    .pb(px(32.))
                    .flex()
                    .flex_col()
                    .child(balance_display(
                        theme,
                        &mut self.icons,
                        &balance,
                        // Tap-to-hide, spec 025's gesture — the figure IS the control,
                        // as it is on the phone and on the web. A session is what makes
                        // it real: the fixture hero has no privacy to keep.
                        self.identity.is_some().then(|| {
                            Box::new(|_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                crate::executor::balance_dashboard::dispatch(
                                    vela_core::app::balance_dashboard::Event::PrivacyToggled,
                                    cx,
                                );
                            })
                                as crate::wallet::components::BalanceToggle
                        }),
                        self.identity.is_some().then(|| {
                            Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                                this.open_balance_status(cx);
                            }))
                                as crate::wallet::components::BalanceToggle
                        }),
                    ))
                    .child(
                        div()
                            .max_w(px(WALLET_ROW_MEASURE))
                            .flex()
                            .flex_col()
                            .child(pills)
                            .child(
                                div()
                                    .id("section-activity")
                                    .cursor_pointer()
                                    .child(section_header(
                                        theme,
                                        &mut self.icons,
                                        s_activity,
                                        s_all,
                                    ))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.enter_flow(FlowEntry::Activity, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(activity_col)
                            .child({
                                // The two halves lead to two different panels: the title names
                                // the assets list, and the action reads 添加, so it opens the
                                // add-token panel stacked on it — which is what makes DT3L's
                                // back chevron lead somewhere.
                                let (title, action) =
                                    section_header_parts(theme, &mut self.icons, s_assets, s_add);
                                section_header_row()
                                    .child(
                                        div()
                                            .id("section-assets")
                                            .cursor_pointer()
                                            .child(title)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.enter_flow(FlowEntry::Assets, cx);
                                                cx.notify();
                                            })),
                                    )
                                    .child(
                                        div()
                                            .id("section-assets-add")
                                            .cursor_pointer()
                                            .child(action)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.enter_flow(FlowEntry::AddToken, cx);
                                                cx.notify();
                                            })),
                                    )
                            })
                            .child(assets_col),
                    ),
            );
        // The column scrolls BELOW the window's caption strip, not under it:
        // that strip is the drag region and carries the window's own
        // buttons, and a balance scrolled up beneath ✕ was drawn through it.
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .pt(px(WALLET_PAD_TOP.max(crate::window_frame::CAPTION_H)))
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(column)
                    .children(crate::ui::vertical_scrollbar(theme, &self.content_scroll)),
            )
    }

    // -- column 2 (contacts): header + group rail + sectioned list -----------

    /// Window-space top of the contacts content column: the gallery chip strip
    /// pushes everything down when it is on screen, and the strip is itself
    /// pushed down by the caption row where the page draws one.
    fn contacts_top(&self, window: &Window) -> f32 {
        if self.gallery {
            GALLERY_BAR_H + gallery_bar_caption_pad(owns_titlebar(window))
        } else {
            0.
        }
    }

    /// Where DC5's dropdown hangs when the gallery chip (rather than a click)
    /// opens it: right-aligned under the header hairline, as in the mock.
    fn header_menu_anchor(&self, window: &Window) -> Point<Pixels> {
        point(
            window.viewport_size().width - px(WALLET_PAD_X),
            px(self.contacts_top(window) + CONTACTS_HEADER_H),
        )
    }

    /// Where DC4's group-header ⋯ hangs: right-aligned under that button, one
    /// control height below the top of the content body.
    fn group_header_menu_anchor(&self, window: &Window) -> Point<Pixels> {
        point(
            window.viewport_size().width - px(WALLET_PAD_X),
            px(self.contacts_top(window)
                + CONTACTS_HEADER_H
                + CONTACTS_BODY_PAD_TOP
                + CONTACTS_BUTTON_H),
        )
    }

    /// Where DC6's context menu hangs when the gallery chip opens it: the
    /// trailing edge of the 家人 rail row, which is where a right-click on that
    /// row lands. The interactive path uses the real cursor position instead.
    fn group_menu_anchor(&self, window: &Window) -> Point<Pixels> {
        point(
            px(SIDEBAR_W + WALLET_PAD_X + CONTACTS_RAIL_W),
            px(self.contacts_top(window)
                + CONTACTS_HEADER_H
                + CONTACTS_BODY_PAD_TOP
                + CONTACTS_RAIL_ROW_H
                + CONTACTS_RAIL_LABEL_H
                + CONTACTS_RAIL_ROW_H),
        )
    }

    fn contacts_header(
        &mut self,
        theme: &Theme,
        caption: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let title = self.contacts.title.clone();
        let placeholder = self.contacts.search_placeholder.clone();
        let add = self.contacts.add_contact.clone();

        // The DC1 hairline is inset to the content column's padding, not bled
        // to the sidebar edge — so it is a sibling row, not a bottom border.
        //
        // The row is centred in `CONTACTS_HEADER_H`, which puts the search
        // field's top edge a few pixels inside the drag strip where the page
        // draws its own caption. Padding the row pushes the whole centred
        // group clear of it; the header's own height is unchanged, so the
        // hairline — and every menu anchor hung off it — stays put.
        let row = div()
            .flex_1()
            .flex()
            .items_center()
            .gap(px(16.))
            .px(px(WALLET_PAD_X))
            .when(caption, |el| el.pt(px(CONTACTS_HEADER_CAPTION_PAD)))
            .child(
                div()
                    .text_size(theme::text_panel_title())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(title),
            )
            .child(div().flex_1().min_w(px(0.)))
            .child({
                let live = self.contacts_search_input(theme, placeholder.clone(), window, cx);
                search_field(theme, &mut self.icons, placeholder, Some(live))
            })
            .child(
                outline_button(
                    "contacts-add",
                    theme,
                    &mut self.icons,
                    Some(Icon::UserRoundPlus),
                    add,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.open_add_contact(window, cx);
                })),
            )
            .child(
                icon_button("contacts-more", theme, &mut self.icons, Icon::Ellipsis).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.menu = Some((
                            ContactsMenu::Header,
                            this.header_menu_anchor(window),
                            Anchor::TopRight,
                        ));
                        cx.notify();
                    }),
                ),
            );

        div()
            .flex_none()
            .h(px(CONTACTS_HEADER_H))
            .flex()
            .flex_col()
            .child(row)
            .child(
                div()
                    .mx(px(WALLET_PAD_X))
                    .h(px(1.))
                    .flex_none()
                    .bg(theme.divider),
            )
    }

    /// Nobody in the book (078 C-03): the core's loaded, empty roster — or
    /// the gallery's DC3, which draws it without one.
    fn contacts_book_empty(&self, cx: &mut Context<Self>) -> bool {
        if self.contacts_empty {
            return true;
        }
        if self.identity.is_none() {
            return false;
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        view.loaded && view.contacts.is_empty()
    }

    /// 添加联系人's form, opened on its address — the header's button and the
    /// empty book's both.
    fn open_add_contact(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Only a real session saves anything: the fixture roster is a
        // picture, and a picture must not grow a row.
        if self.identity.is_none() {
            return;
        }
        self.contact_form = Some(ContactForm::default());
        // The address is the first thing to type, so it is the first thing
        // focused.
        window.focus(&self.contact_form_address_focus, cx);
        cx.notify();
    }

    fn contacts_rail(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let all = self.contacts.all_contacts.clone();
        let groups_label = self.contacts.section_groups.clone();
        let new_group = self.contacts.group_new.clone();
        // The count beside 全部联系人. A real session counts its own book; the
        // mock's 12 under somebody's four contacts is the same small lie the
        // group rail told. The WHOLE book, as the web's `allCount`: the search
        // narrows the list beside it, not what "all" means.
        let total = if self.contacts_empty {
            0
        } else if self.identity.is_some() {
            let view = resident::resident::<Contacts>(cx).read(cx).view();
            u32::try_from(view.contacts.len()).unwrap_or(u32::MAX)
        } else {
            contacts_fixtures::TOTAL_CONTACTS
        };
        let selected_group = self.group;
        let empty = self.contacts_empty;

        let mut rail = div()
            .w(px(CONTACTS_RAIL_W))
            .flex_none()
            .flex()
            .flex_col()
            .gap(px(2.))
            .child(
                rail_row(
                    "rail-all",
                    theme,
                    &mut self.icons,
                    None,
                    all,
                    Some(total),
                    if selected_group.is_none() {
                        RailState::Selected
                    } else {
                        RailState::Default
                    },
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    this.group = None;
                    this.menu = None;
                    cx.notify();
                })),
            );

        let groups = self.group_models(cx);
        if !empty && !groups.is_empty() {
            rail = rail.child(rail_label(theme, groups_label));
            for (i, (_, name, count)) in groups.iter().enumerate() {
                rail = rail.child(
                    rail_row(
                        ElementId::from(("rail-group", i)),
                        theme,
                        &mut self.icons,
                        Some(Icon::UsersRound),
                        name.clone(),
                        Some(*count),
                        if selected_group == Some(i) {
                            RailState::Selected
                        } else {
                            RailState::Default
                        },
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.group = Some(i);
                        this.menu = None;
                        cx.notify();
                    }))
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.group = Some(i);
                            this.menu =
                                Some((ContactsMenu::Group, event.position, Anchor::TopLeft));
                            cx.notify();
                        }),
                    ),
                );
            }
        }

        rail.child(
            rail_row(
                "rail-new-group",
                theme,
                &mut self.icons,
                Some(Icon::FolderPlus),
                new_group,
                None,
                RailState::Default,
            )
            .on_click(cx.listener(|this, _, window, cx| {
                if this.identity.is_none() {
                    return;
                }
                this.group_form = Some((None, String::new()));
                window.focus(&this.group_form_focus, cx);
                cx.notify();
            })),
        )
    }

    /// The address of the row the detail panel is about, for a real session.
    ///
    /// `None` on the design surfaces, which is what stops a fixture panel from
    /// mutating a real ledger.
    fn selected_contact_address(&mut self, cx: &mut Context<Self>) -> Option<gpui::SharedString> {
        if self.identity.is_none() {
            return None;
        }
        // The CORE's order, as `self.contact` is (`open_contact_by_address`).
        // This read the A–Z sections — searched, at that — so with the panel
        // on Alice, its copy button copied Bob, and its delete asked to delete
        // Bob under Alice's name (found verifying 078 C-08).
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        contacts_live::rows(&view)
            .into_iter()
            .nth(self.contact)
            .map(|row| row.address_full)
    }

    /// DC2's model for a real session, or `None` to fall back to the mock.
    fn contact_detail_model(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<contacts_fixtures::ContactDetailModel> {
        if self.identity.is_none() {
            return None;
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        if !view.loaded {
            return None;
        }
        // Opening an entry is what asks the core to look the address up. The
        // web dispatches the same event on the same gesture; nothing here ever
        // did, so a contact somebody saved WITHOUT a name stayed nameless
        // forever — the core writes the resolved identity back onto exactly
        // that contact (its `RecipientTrust` write-back), and this shell has
        // been reading `resolved_name` since 031 with nobody filling it.
        //
        // Chain 1 for the classification, as the web says in the same words:
        // mainnet until a send flow names one. Deduped by address here because
        // this runs once per frame; the core dedupes and caches too, and both
        // guards are cheap.
        if let Some(row) = contacts_live::rows(&view).into_iter().nth(self.contact) {
            let address = row.address_full.to_string();
            if self.inspected_contact.as_deref() != Some(address.as_str()) {
                self.inspected_contact = Some(address.clone());
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(
                        ContactEvent::InspectRecipient {
                            chain_id: 1,
                            address,
                        },
                        cx,
                    );
                });
            }
        }
        let hidden = resident::resident::<BalanceDashboard>(cx)
            .read(cx)
            .view()
            .hidden;
        let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
        contacts_live::detail(
            &view,
            self.contact,
            &feed,
            &self.strings,
            &self.flow_strings,
            self.contact_all_activity,
            hidden,
        )
    }

    /// The activity rows: the core's feed for a real session, the mock's
    /// otherwise.
    ///
    /// Privacy comes from the BALANCE view, not the feed's own: every money
    /// surface masks together, and reading two different flags is how one of
    /// them ends up out of step.
    fn activity_models(&mut self, cx: &mut Context<Self>) -> Vec<fixtures::ActivityRowModel> {
        if self.identity.is_none() {
            return fixtures::activity_default(&self.strings);
        }
        let hidden = resident::resident::<BalanceDashboard>(cx)
            .read(cx)
            .view()
            .hidden;
        let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
        wallet_live::activity_rows(&feed, &self.strings, &self.flow_strings, hidden)
    }

    /// The core-list indices behind the home's asset strip, in drawn order.
    ///
    /// The fixture strip has no core behind it, so it maps to nothing — which
    /// is the same `None` the asset panel already falls back on.
    fn visible_assets(&mut self, cx: &mut Context<Self>) -> Vec<usize> {
        if self.identity.is_none() {
            return Vec::new();
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        wallet_live::visible_token_indices(&view, self.chain_filter)
    }

    /// Pick a network — or `None` for all of them.
    ///
    /// Holdings and the feed narrow; the hero total does not. That is the
    /// phone's `selectedChainId` semantics, ported word for word by the web,
    /// and it is a deliberate asymmetry: the filter is about looking through a
    /// list, not about pretending the money on other chains is gone.
    ///
    /// The FEED's half belongs to the core (`ChainFilterChanged`), which
    /// re-emits the day headers around what survives — filtering the drawn rows
    /// instead would leave a date heading over an empty day.
    fn select_chain(&mut self, chain_id: Option<u32>, cx: &mut Context<Self>) {
        if self.chain_filter == chain_id {
            return;
        }
        self.chain_filter = chain_id;
        // An open asset panel was opened by index into the UNFILTERED list, and
        // the list it indexes is about to change shape.
        self.asset_detail = None;
        resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
            resident.dispatch(
                vela_core::app::activity_feed::Event::ChainFilterChanged { chain_id },
                cx,
            );
        });
        cx.notify();
    }

    /// Tell the hero which accounts are on screen — once per opening.
    ///
    /// The core fetches a total for each while the switcher is open and stops
    /// when it closes, so this is a subscription and not a query: opening asks,
    /// leaving must say so, or the app keeps reading other accounts' balances
    /// forever.
    fn sync_switcher(
        &mut self,
        session: &vela_core::app::session::SessionView,
        cx: &mut Context<Self>,
    ) {
        let addresses: Vec<String> = session
            .accounts
            .iter()
            .map(|row| row.account.address.clone())
            .collect();
        if self.switcher_addresses.as_ref() == Some(&addresses) {
            return;
        }
        self.switcher_addresses = Some(addresses.clone());
        crate::executor::balance_dashboard::dispatch(
            vela_core::app::balance_dashboard::Event::SwitcherOpened { addresses },
            cx,
        );
    }

    /// 078 H-01 — the header's name opens the switcher, over whatever
    /// section is on screen. Its first frame tells the core, as the settings
    /// panel's does (`sync_switcher`).
    fn open_account_switcher(&mut self, cx: &mut Context<Self>) {
        self.account_switcher = true;
        self.removing_account = None;
        self.menu = None;
        self.switcher_scroll = gpui::ScrollHandle::new();
        cx.notify();
    }

    /// The switcher dialog goes. The core's subscription goes with it unless
    /// the settings panel it sits over is the same list, still on screen —
    /// which would otherwise re-announce itself on its next frame, and every
    /// account's balance would be fetched again for nothing.
    fn close_account_switcher(&mut self, cx: &mut Context<Self>) {
        if !self.account_switcher {
            return;
        }
        self.account_switcher = false;
        self.removing_account = None;
        if !(self.section == Section::Settings && self.settings_page == SettingsPage::Account) {
            self.close_switcher(cx);
        }
        cx.notify();
    }

    /// One of the header's dialogs is up (078 H-01, H-02). The sidebar is on
    /// screen while a site is, so either can open over the browser column —
    /// and the webview is a NATIVE view that paints over anything gpui draws,
    /// dialog included. While one is up the page is taken off the screen, not
    /// closed (spec 070): the scrim covers where it was.
    #[cfg(not(target_os = "linux"))]
    fn dialog_over_browser(&self) -> bool {
        self.account_switcher || self.identicon_viewer.is_some()
    }

    /// 078 H-02 — every artwork with an address behind it opens this.
    fn open_identicon_viewer(&mut self, address: String, cx: &mut Context<Self>) {
        self.identicon_viewer = Some(IdenticonViewer {
            address,
            copied: None,
            presses: 0,
        });
        cx.notify();
    }

    /// "Copy address", then "Copied" for 1.5 s, as the web's viewer does.
    fn copy_viewer_address(&mut self, cx: &mut Context<Self>) {
        let Some(viewer) = self.identicon_viewer.as_mut() else {
            return;
        };
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(viewer.address.clone()));
        viewer.presses += 1;
        let press = viewer.presses;
        viewer.copied = Some(press);
        cx.notify();
        cx.spawn(async move |page, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(1500))
                .await;
            let _ = page.update(cx, |this, cx| {
                if let Some(viewer) = this.identicon_viewer.as_mut()
                    && viewer.copied == Some(press)
                {
                    viewer.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The switcher left the screen. Idempotent, and called from every path
    /// that can leave it — a subscription nobody closes is a poll.
    fn close_switcher(&mut self, cx: &mut Context<Self>) {
        if self.switcher_addresses.take().is_none() {
            return;
        }
        crate::executor::balance_dashboard::dispatch(
            vela_core::app::balance_dashboard::Event::SwitcherClosed,
            cx,
        );
    }

    /// Tell the feed what the hero is doing about privacy, when it changes.
    ///
    /// The core withholds the toast while balances are hidden — but only if it
    /// knows, and the flag lives in another machine. Called from the one place
    /// that would leak (the toast overlay), and only on a change, so the tell
    /// costs a dispatch per toggle rather than one per frame.
    fn sync_feed_privacy(&mut self, cx: &mut Context<Self>) -> bool {
        let hidden = resident::resident::<BalanceDashboard>(cx)
            .read(cx)
            .view()
            .hidden;
        if self.feed_privacy != Some(hidden) {
            self.feed_privacy = Some(hidden);
            crate::executor::activity_feed::privacy_changed(hidden, cx);
        }
        hidden
    }

    /// Which home row is the one that just landed, as an index into the drawn
    /// preview — `None` when nothing is being celebrated.
    fn celebrated_row(&mut self, home_tx_ids: &[String], cx: &mut Context<Self>) -> Option<usize> {
        if self.identity.is_none() {
            return self.celebrating.then_some(fixtures::CELEBRATED_ROW);
        }
        let new_item_id = resident::resident::<ActivityFeed>(cx)
            .read(cx)
            .view()
            .new_item_id?;
        home_tx_ids.iter().position(|id| *id == new_item_id)
    }

    /// The home's asset strip: the person's holdings, or the mocks'.
    ///
    /// Note what is NOT here — a fallback to the fixture list while a real
    /// wallet is still counting. An empty strip under a counting hero is the
    /// truth; six of somebody else's tokens under it is the wrong screen this
    /// whole cut exists to fix.
    fn asset_models(&mut self, cx: &mut Context<Self>) -> Vec<fixtures::AssetRowModel> {
        if self.identity.is_none() {
            return fixtures::assets_default(&self.strings);
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        let money = self.money(cx);
        wallet_live::asset_rows(
            &view,
            &self.strings,
            &self.locale,
            self.chain_filter,
            &money,
        )
    }

    /// The home's network list: the chains this person actually holds on.
    fn chain_models(&mut self, cx: &mut Context<Self>) -> Vec<fixtures::ChainRowModel> {
        if self.identity.is_none() {
            return fixtures::chains(&self.strings);
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        wallet_live::chain_rows(&view, &self.strings, self.chain_filter)
    }

    /// Tell the core a service panel is on screen — once per visit.
    ///
    /// Both events start probes, and a probe per frame would be a network
    /// request per frame. Cleared when the section changes, so coming back
    /// re-checks rather than showing whatever the last visit measured.
    fn settings_opened(&mut self, panel: SettingsProbe, cx: &mut Context<Self>) {
        if self.settings_probed_panel == Some(panel) {
            return;
        }
        self.settings_probed_panel = Some(panel);
        resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
            resident.dispatch(
                match panel {
                    SettingsProbe::Endpoints => NetEvent::EndpointsOpened,
                    SettingsProbe::Providers => NetEvent::ProvidersOpened,
                },
                cx,
            );
        });
    }

    /// The focus handle for one editable settings field, made on first use.
    ///
    /// A handle per field rather than one shared: focus is which box the
    /// keystrokes go into, and a shared handle would send them to whichever
    /// field drew last.
    fn endpoint_focus(&mut self, index: usize, cx: &mut Context<Self>) -> gpui::FocusHandle {
        while self.endpoint_focuses.len() <= index {
            self.endpoint_focuses.push(cx.focus_handle());
        }
        self.endpoint_focuses[index].clone()
    }

    /// D3's model: the selected holding, or the mock's BNB.
    fn asset_detail_model(&mut self, cx: &mut Context<Self>) -> fixtures::AssetDetailModel {
        let Some(index) = self.asset_detail else {
            return fixtures::asset_detail_default(&self.strings);
        };
        if self.identity.is_none() {
            return fixtures::asset_detail_default(&self.strings);
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
        let money = self.money(cx);
        wallet_live::asset_detail(&view, &feed, index, &self.strings, &self.locale, &money)
            // The holding is gone — a refresh re-ordered the list under an open
            // panel. The mock is NOT a substitute: it would silently swap which
            // asset somebody is looking at, and the next thing they do on this
            // panel is send it.
            .unwrap_or_else(|| fixtures::AssetDetailModel {
                logos: crate::marks::Logos::default(),
                ticker: SharedString::from(""),
                badge: gpui::rgb(0x8A_8F_98).into(),
                amount: SharedString::from(""),
                sub: SharedString::from(""),
                facts: Vec::new(),
                activity: Vec::new(),
                activity_ids: Vec::new(),
                explorer_url: None,
                contract_copy: None,
            })
    }

    /// The balance hero: the core's figure for a real session, the mock's
    /// otherwise.
    ///
    /// Note what is NOT here — a fallback to the fixture when the core has not
    /// answered yet. `wallet::live::balance` renders a skeleton for `None`, and
    /// substituting `$1,383.28` while a real wallet is still counting would be
    /// the app showing somebody a stranger's money and calling it theirs.
    fn balance_model(&mut self, cx: &mut Context<Self>) -> fixtures::BalanceModel {
        if self.identity.is_none() {
            return fixtures::balance_default(&self.strings);
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        let money = self.money(cx);
        wallet_live::balance(&view, &self.strings, &self.locale, &money)
    }

    /// The group rail: the person's own groups, or the mocks'.
    ///
    /// Each carries its id, because the menu that acts on a group has to name
    /// WHICH one to the core and an index into a reorderable list is not a name.
    fn group_models(&mut self, cx: &mut Context<Self>) -> Vec<(SharedString, SharedString, u32)> {
        if self.identity.is_none() {
            return contacts_fixtures::GROUPS
                .iter()
                .map(|group| {
                    (
                        SharedString::from(group.name),
                        SharedString::from(group.name),
                        group.count,
                    )
                })
                .collect();
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        if !view.loaded {
            // The core has not ruled. An empty rail, not a fixture one — the
            // same rule the roster beside it already follows.
            return Vec::new();
        }
        contacts_live::groups(&view)
    }

    /// The roster: the core's book for a real session, the mocks' otherwise.
    fn contact_sections(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Vec<(gpui::SharedString, Vec<ContactRowModel>)> {
        if self.identity.is_none() {
            let query = self.contacts_query.trim().to_lowercase();
            return contacts_fixtures::sections_model()
                .into_iter()
                .map(|(letter, rows)| {
                    let rows: Vec<ContactRowModel> = rows
                        .into_iter()
                        .filter(|row| {
                            query.is_empty()
                                || row.name.to_lowercase().contains(&query)
                                || row.address_full.to_lowercase().contains(&query)
                        })
                        .collect();
                    (letter, rows)
                })
                .filter(|(_, rows)| !rows.is_empty())
                .collect();
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        if !view.loaded {
            // The core has not ruled yet. An empty roster, not a fixture one —
            // a person's address book must never show somebody else's while it
            // waits (spec 030 FR-008).
            return Vec::new();
        }
        contacts_live::sections(&view, &self.contacts_query)
    }

    /// The header search's input and its ✕ (078 X-05 / C-02).
    fn contacts_search_input(
        &mut self,
        theme: &Theme,
        placeholder: SharedString,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> crate::contacts::components::SearchInput {
        let page = cx.entity().downgrade();
        let input = crate::ui::search_input(
            "contacts-search",
            theme,
            &self.contacts_query,
            placeholder,
            &self.contacts_query_focus,
            window,
            move |text, _, cx| {
                let _ = page.update(cx, |this, cx| {
                    this.contacts_query = text;
                    cx.notify();
                });
            },
        )
        .into_any_element();
        let clear = (!self.contacts_query.is_empty()).then(|| {
            div()
                .id("contacts-search-clear")
                .flex_none()
                .flex()
                .items_center()
                .cursor_pointer()
                .child(icon_img(
                    &mut self.icons,
                    Icon::X,
                    false,
                    theme.fg_subtle,
                    14.,
                ))
                .on_click(cx.listener(|this, _, window, cx| {
                    this.contacts_query.clear();
                    // The web's clear leaves the person where they were
                    // typing, ready for the next name.
                    this.contacts_query_focus.focus(window, cx);
                    cx.notify();
                }))
                .into_any_element()
        });
        crate::contacts::components::SearchInput {
            input,
            focused: self.contacts_query_focus.is_focused(window),
            clear,
        }
    }

    /// DC1: the A–Z sectioned roster.
    ///
    /// Rows are identified by ADDRESS, never by their position in this list.
    /// The list is A–Z (`contacts_live::sections`) and everything that acts on
    /// "the selected contact" — the detail panel, the context menu, 移入分组,
    /// 复制地址, 删除 — reads `contacts_live::rows`, which is the CORE's order.
    /// Setting `self.contact` to the A–Z position therefore opened, menued and
    /// acted on whoever happened to sit at that position in the other order.
    /// Found on a desktop with four contacts: clicking Alice opened Dave, and
    /// right-clicking Alice → 移入分组 wrote **Dave's** address into the group.
    fn contacts_list(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected_address = match self.panel {
            PanelId::ContactDetail => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                contacts_live::rows(&view)
                    .get(self.contact)
                    .map(|row| row.address_full.to_string())
            }
            _ => None,
        };
        let mut list = div()
            .id("contacts-list")
            .flex_1()
            .min_w(px(0.))
            .min_h(px(0.))
            .overflow_y_scroll()
            .flex()
            .flex_col();

        let sections = self.contact_sections(cx);
        if sections.is_empty() && !self.contacts_query.trim().is_empty() {
            // A search that leaves nobody says so, and what it searched for
            // (`contacts.noResults`), rather than showing an empty column.
            return list.child(div().pt(px(48.)).child(empty_state(
                theme,
                &mut self.icons,
                Icon::Search,
                contacts_fixtures::no_results_label(&self.contacts, self.contacts_query.trim()),
                self.contacts.search_placeholder.clone(),
            )));
        }
        let mut index = 0usize;
        for (letter, rows) in sections {
            list = list.child(section_letter(theme, letter));
            let last = rows.len() - 1;
            for (i, contact) in rows.iter().enumerate() {
                let at = index;
                let address = contact.address_full.to_string();
                let menu_address = address.clone();
                let is_selected = selected_address
                    .as_deref()
                    .is_some_and(|picked| picked.eq_ignore_ascii_case(&address));
                list = list.child(
                    contact_row(
                        ElementId::from(("contact", at)),
                        theme,
                        &mut self.identicons,
                        contact,
                        is_selected,
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_contact_by_address(&address, cx);
                    }))
                    // The contact menu has been drawn since spec 018 and lived
                    // only on the component board. It is the desktop's entry
                    // to 移入分组 — DC2's own comment says so — so without it
                    // groups could be created, renamed and deleted and never
                    // filled.
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if this.open_contact_by_address(&menu_address, cx) {
                                this.menu =
                                    Some((ContactsMenu::Contact, event.position, Anchor::TopLeft));
                                cx.notify();
                            }
                        }),
                    ),
                );
                if i != last {
                    list = list.child(row_divider(theme));
                }
                index += 1;
            }
        }
        list
    }

    /// DC4: the group view — header with the accent 群发转账, member rows, the
    /// ghost 添加成员 row and the caption line.
    fn contacts_group_view(&mut self, theme: &Theme, group: usize, cx: &mut Context<Self>) -> Div {
        // The group this view is ABOUT. Falling back to whichever fixture sat
        // at that index would put somebody else's members under this group's
        // name — with a 群发转账 button above them.
        let live = self.identity.is_some().then(|| {
            let view = resident::resident::<Contacts>(cx).read(cx).view();
            contacts_live::group_members(&view, group)
        });
        let (name, members) = match live {
            Some(Some((name, members))) => (name, members),
            Some(None) => (SharedString::from(""), Vec::new()),
            None => {
                let fixture = contacts_fixtures::GROUPS[group];
                (
                    SharedString::from(fixture.name),
                    contacts_fixtures::group_members_model(group),
                )
            }
        };
        let count = u32::try_from(members.len()).unwrap_or(u32::MAX);
        // Who 群发转账 is about, captured before the header consumes the rows.
        let batch_targets: Vec<(String, String)> = members
            .iter()
            .map(|m| (m.address_full.to_string(), m.name.to_string()))
            .collect();
        let members_label = contacts_fixtures::members_count_label(&self.contacts, count);
        let caption = contacts_fixtures::batch_send_caption(&self.contacts, count);
        let batch_send = self.contacts.batch_send.clone();
        let add_member = self.contacts.add_member.clone();

        let header = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .pb(px(12.))
            .child(
                div()
                    .text_size(theme::text_section())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(name.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(members_label),
            )
            .child(div().flex_1().min_w(px(0.)))
            .child(
                accent_button("group-batch-send", theme, &mut self.icons, None, batch_send)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.batch_send_to_group(&batch_targets, cx);
                    })),
            )
            .child(
                icon_button("group-more", theme, &mut self.icons, Icon::Ellipsis).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.menu = Some((
                            ContactsMenu::Group,
                            this.group_header_menu_anchor(window),
                            Anchor::TopRight,
                        ));
                        cx.notify();
                    }),
                ),
            );

        let mut column = div().flex_1().min_w(px(0.)).flex().flex_col().child(header);
        let last = members.len().saturating_sub(1);
        for (i, member) in members.iter().enumerate() {
            // A member row opens and menus exactly like the same row in the
            // directory. Without this a group was a dead end: from inside one
            // you could not open, pay or edit anybody in it.
            let address = member.address_full.to_string();
            let menu_address = address.clone();
            column = column.child(
                contact_row(
                    ElementId::from(("member", i)),
                    theme,
                    &mut self.identicons,
                    member,
                    false,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_contact_by_address(&address, cx);
                }))
                .on_mouse_down(
                    MouseButton::Right,
                    cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                        if this.open_contact_by_address(&menu_address, cx) {
                            this.menu =
                                Some((ContactsMenu::Contact, event.position, Anchor::TopLeft));
                            cx.notify();
                        }
                    }),
                ),
            );
            if i != last {
                column = column.child(row_divider(theme));
            }
        }
        column
            .child(row_divider(theme))
            // 添加成员 has been drawn on this screen since spec 018 with no
            // listener — the other half of the membership question phase 2
            // answered from the contact's side.
            .child(
                ghost_add_row("group-add-member", theme, &mut self.icons, add_member).on_click(
                    cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        this.open_member_pick(cx);
                    }),
                ),
            )
            .child(
                div()
                    .pt(px(12.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(caption),
            )
    }

    /// Point `self.contact` at whoever holds this address, and open them.
    ///
    /// The detail panel and the contact menu both read `self.contact` as an
    /// index into the directory, so a row that knows only an address has to
    /// find that index rather than invent one — picking the wrong index would
    /// put somebody else's details under this person's name.
    fn open_contact_by_address(&mut self, address: &str, cx: &mut Context<Self>) -> bool {
        if self.identity.is_none() {
            return false;
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let Some(index) = contacts_live::rows(&view)
            .iter()
            .position(|row| row.address_full.eq_ignore_ascii_case(address))
        else {
            return false;
        };
        if self.contact != index || self.panel != PanelId::ContactDetail {
            self.contact_all_activity = false;
        }
        self.contact = index;
        self.panel = PanelId::ContactDetail;
        self.menu = None;
        cx.notify();
        true
    }

    /// 群发转账 — the group's members, brought to the send form.
    ///
    /// The same rule the web states: **a group of one is a send to that one**,
    /// and two or more seed split mode. An empty group cannot reach here (its
    /// caption says why), and seeding appends rather than replaces, so a form
    /// somebody had already started typing into keeps what is in it.
    fn batch_send_to_group(&mut self, members: &[(String, String)], cx: &mut Context<Self>) {
        if members.is_empty() {
            return;
        }
        self.section = Section::Wallet;
        self.send_sweeping = false;
        self.flows = FlowPanel::entry(FlowEntry::Send);
        self.panel = PanelId::Flow;
        if let [(address, _)] = members {
            self.open_send(
                SendOpenParams {
                    prefilled_recipient: Some(address.clone()),
                    ..SendOpenParams::default()
                },
                cx,
            );
            cx.notify();
            return;
        }
        self.open_send(SendOpenParams::default(), cx);
        let recipients: Vec<SendRecipientDraft> = members
            .iter()
            .map(|(address, name)| SendRecipientDraft {
                id: String::new(),
                address: address.clone(),
                amount: String::new(),
                name: (!name.is_empty()).then(|| name.clone()),
            })
            .collect();
        if let Some(host) = self.send_host.clone() {
            host.update(cx, |host, cx| {
                host.dispatch(SendEvent::AppendSplitRecipients { recipients }, cx);
            });
        }
        cx.notify();
    }

    /// DC3: the centred empty state with both CTAs — add opens the form,
    /// import the file picker, as the web's `empty-primary` / `-secondary`.
    fn contacts_empty_view(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let actions = self.identity.is_some().then(|| {
            (
                Box::new(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    this.open_add_contact(window, cx);
                })) as contacts_components::MenuAction,
                Box::new(cx.listener(|_, _: &gpui::ClickEvent, _, cx| {
                    Self::import_contacts(None, cx);
                })) as contacts_components::MenuAction,
            )
        });
        let title = self.contacts.empty.clone();
        let caption = self.contacts.empty_hint.clone();
        let primary = self.contacts.add_contact.clone();
        let secondary = self.contacts.import_file.clone();
        div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .items_center()
            .justify_center()
            .child(empty_state_cta(
                theme,
                &mut self.icons,
                title,
                caption,
                primary,
                secondary,
                actions,
            ))
    }

    fn contacts_content(
        &mut self,
        theme: &Theme,
        caption: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let header = self.contacts_header(theme, caption, window, cx);
        let rail = self.contacts_rail(theme, cx);
        // Empty first, as the web orders it: a group opened on a book with
        // nobody in it has nobody to list.
        let body: gpui::AnyElement = if self.contacts_book_empty(cx) {
            self.contacts_empty_view(theme, cx).into_any_element()
        } else if let Some(group) = self.group {
            self.contacts_group_view(theme, group, cx)
                .into_any_element()
        } else {
            self.contacts_list(theme, cx).into_any_element()
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_hidden()
            .flex()
            .flex_col()
            .child(header)
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .flex()
                    .gap(px(WALLET_PAD_X))
                    .px(px(WALLET_PAD_X))
                    .pt(px(CONTACTS_BODY_PAD_TOP))
                    .child(rail)
                    .child(body),
            )
    }

    /// DC2's third-column body: hero, pill actions, address, 最近往来, footer.
    fn contact_detail_body(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        // The address the panel is about. For a real session it comes from the
        // core's own roster, so a delete removes the row a person is looking at
        // rather than whatever the mock had at that index.
        let live_address = self.selected_contact_address(cx);
        // The panel drew a FIXTURE while its delete and copy acted on the real
        // contact — so somebody clicking their cousin saw Alice's name, Alice's
        // avatar and Alice's address, and the delete removed the cousin. A
        // mismatch is worse than a mock: a mock is honestly a picture, and this
        // was a picture with a live weapon attached.
        let model = self.contact_detail_model(cx).unwrap_or_else(|| {
            let contact = contacts_fixtures::CONTACTS
                [self.contact.min(contacts_fixtures::CONTACTS.len() - 1)];
            contacts_fixtures::contact_detail(&self.contacts, &contact)
        });
        let address_label = self.contacts.address_label.clone();
        // Copy the address the panel is ABOUT — the core's, for a real session.
        // Copying the mock's would put a stranger's address on somebody's
        // clipboard, and the next thing that happens to a copied address is a
        // paste into a send field.
        let copy_address: Option<contacts_components::MenuAction> =
            live_address.clone().map(|address| {
                Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                    this.copy_text(CONTACTS_TOAST, address.to_string(), CONTACTS_COPY_HOLD, cx);
                })) as contacts_components::MenuAction
            });
        let recent = self.contacts.recent_activity.clone();
        let view_all = self.contacts.view_all_activity.clone();
        let edit = self.contacts.edit.clone();
        let delete = self.contacts.delete_contact.clone();
        let delete_name = model.name.clone();
        let send = self.contacts.action_send.clone();
        let receive = self.contacts.action_receive.clone();
        let qr = self.contacts.action_qr.clone();

        // The membership chips, and the web's `+` chip after them — 移入分组
        // from where the groups are shown, not only from a right-click
        // (078 C-06).
        let mut chips = div().flex().flex_wrap().gap(px(4.));
        for chip in &model.chips {
            chips = chips.child(group_chip(theme, chip.clone()));
        }
        if let Some(address) = live_address.clone() {
            chips = chips.child(
                div()
                    .id("contact-add-group")
                    .cursor_pointer()
                    .child(add_chip(
                        theme,
                        &mut self.icons,
                        self.contacts.move_group.clone(),
                    ))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.open_group_pick(&address, cx);
                    })),
            );
        }
        // Nobody has named this contact: the way to, under the name — "Save to
        // contacts" over a history suggestion, "Edit" otherwise (the web's
        // `nameAction`, issue 191).
        let name_action = live_address.clone().and_then(|address| {
            let view = resident::resident::<Contacts>(cx).read(cx).view();
            let contact = view
                .contacts
                .iter()
                .find(|contact| contact.address.eq_ignore_ascii_case(&address))?;
            if contact.name.as_deref().is_some_and(|name| !name.is_empty()) {
                return None;
            }
            let label = if contact.source == vela_core::app::contacts::ContactSource::Auto {
                self.contacts.save_to_contacts.clone()
            } else {
                self.contacts.edit.clone()
            };
            Some((address, label))
        });
        let name_action = name_action.map(|(address, label)| {
            div()
                .id("contact-name-action")
                .cursor_pointer()
                .child(contacts_components::name_action_pill(
                    theme,
                    &mut self.icons,
                    label,
                ))
                .on_click(cx.listener(move |this, _, window, cx| {
                    this.open_edit_contact(&address, window, cx);
                }))
        });

        let hero = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(crate::wallet::components::identicon_avatar(
                &mut self.identicons,
                model.seed.as_ref(),
                CONTACTS_HERO_AVATAR,
            ))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .items_start()
                    .gap(px(8.))
                    .child(
                        div()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .whitespace_nowrap()
                            .truncate()
                            .child(model.name.clone()),
                    )
                    .children(name_action)
                    .child(chips),
            );

        // What the three pills DO. They have been drawn since 018 and did
        // nothing at all; 转账 and 收款 at least had the row's context menu,
        // and 二维码 had no other route anywhere in the app.
        let send_to = model.address_full.to_string();
        let qr_name = model.name.clone();
        let qr_address = SharedString::from(model.address_full.to_string());
        let actions = div()
            .flex()
            // Three words in a long language at the largest text size are
            // wider than this panel: unwrapped, 二维码 was drawn half off the
            // right edge of the window. The chips above wrap for the same
            // reason.
            .flex_wrap()
            .gap(px(10.))
            .child(
                action_pill(
                    "contact-send",
                    theme,
                    &mut self.icons,
                    Icon::ArrowUpRight,
                    send,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    // The same route the row's 转账 takes: the send flow with
                    // this person prefilled. The core takes the prefill; the
                    // shell does not type into its own screen.
                    this.section = Section::Wallet;
                    this.send_sweeping = false;
                    this.flows = FlowPanel::entry(FlowEntry::Send);
                    this.panel = PanelId::Flow;
                    this.open_send(
                        SendOpenParams {
                            prefilled_recipient: Some(send_to.clone()),
                            ..SendOpenParams::default()
                        },
                        cx,
                    );
                    cx.notify();
                })),
            )
            .child(
                action_pill(
                    "contact-receive",
                    theme,
                    &mut self.icons,
                    Icon::ArrowDownLeft,
                    receive,
                )
                .on_click(cx.listener(|this, _, _, cx| {
                    // My own address — what a person needs when the answer to
                    // "how do I pay you" is asked of them.
                    this.section = Section::Wallet;
                    this.enter_flow(FlowEntry::Receive, cx);
                    cx.notify();
                })),
            )
            .child(
                action_pill("contact-qr", theme, &mut self.icons, Icon::QrCode, qr).on_click(
                    cx.listener(move |this, _, _, cx| {
                        this.contact_qr = Some((qr_name.clone(), qr_address.clone()));
                        cx.notify();
                    }),
                ),
            );

        let mut activity = div().flex().flex_col().child(
            div()
                .pb(px(4.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(recent),
        );
        for row in &model.activity {
            activity = activity.child(activity_row(theme, &mut self.icons, row));
        }
        // The web: nothing between us says so; otherwise the link, which
        // opens the whole history in place (078 C-04).
        activity = if model.activity_empty {
            activity.child(
                div()
                    .pt(px(8.))
                    .text_size(theme::text_body())
                    .text_color(theme.fg_muted)
                    .child(self.contacts.no_activity.clone()),
            )
        } else {
            activity.child(div().pt(px(10.)).child(
                text_action("contact-view-all", theme, &mut self.icons, None, view_all).on_click(
                    cx.listener(|this, _, _, cx| {
                        this.contact_all_activity = true;
                        cx.notify();
                    }),
                ),
            ))
        };

        let footer = div()
            .flex()
            .items_center()
            .justify_between()
            .pt(px(14.))
            .border_t_1()
            .border_color(theme.divider)
            .child({
                // Edit opens the same sheet with the address FIXED — it is the
                // key the core stores under, so changing it would be a delete
                // and an add wearing one button, and the old contact would
                // quietly survive.
                let editing = live_address.clone();
                text_action(
                    "contact-edit",
                    theme,
                    &mut self.icons,
                    Some(Icon::Pencil),
                    edit,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let Some(address) = editing.clone() else {
                        return;
                    };
                    this.open_edit_contact(&address, window, cx);
                }))
            })
            .child(
                destructive_text_button("contact-delete", theme, delete).on_click(cx.listener(
                    move |this, _, _, cx| {
                        // Only a real session deletes anything: the fixture
                        // panel is a picture, and a picture must not mutate a
                        // ledger.
                        let Some(address) = live_address.clone() else {
                            return;
                        };
                        // Asked first, as the web asks (078 C-01).
                        this.confirm = Some(Confirm::DeleteContact {
                            address: address.to_string(),
                            name: delete_name.clone(),
                        });
                        cx.notify();
                    },
                )),
            );

        div()
            .h_full()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(hero)
            .child(actions)
            .child(div().h(px(1.)).bg(theme.divider))
            .child(address_block(
                theme,
                &mut self.icons,
                address_label,
                model.address_full.clone(),
                copy_address,
            ))
            .child(div().h(px(1.)).bg(theme.divider))
            .child(activity)
            .child(div().flex_1().min_h(px(0.)))
            .child(footer)
    }

    // -- column 3: the closable panel (desktop bottom-sheet stand-in) --------

    fn panel_scaffold(
        &mut self,
        theme: &Theme,
        title: SharedString,
        body: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        self.panel_scaffold_with(theme, title, None, false, body, cx)
    }

    /// `panel_scaffold`, with the two things the flow panels need: a chevron
    /// beside the title inside the SAME bar (the mocks draw one row, not a
    /// close floating above a title), and the hairline the flow mocks rule
    /// under it.
    fn panel_scaffold_with(
        &mut self,
        theme: &Theme,
        title: SharedString,
        lead: Option<gpui::AnyElement>,
        underline: bool,
        body: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        // The web's `ThirdPanel` (`wallet/ui/ThirdPanel.svelte`): header
        // 16/24 with a gap of 8 — its top here is the caption strip's
        // clearance, which the browser does not have — a 36px close with a
        // 20px icon, and content that SCROLLS, padded 0/24/24. It was
        // `overflow_hidden`: a long asset history, the signing column's
        // slider and every long flow were cut off at the bottom.
        let mut heading = div()
            .flex_1()
            .min_w(px(0.))
            .flex()
            .items_center()
            .gap(px(8.));
        if let Some(lead) = lead {
            heading = heading.child(lead);
        }
        heading = heading.child(
            div()
                .text_size(theme::text_panel_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(title),
        );
        let mut bar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(24.))
            .pt(px(SIDEBAR_TOP))
            .pb(px(16.));
        if underline {
            bar = bar.border_b_1().border_color(theme.divider);
        }
        div()
            .w(px(THIRD_PANEL_W))
            .h_full()
            .flex_none()
            .bg(theme.bg_base)
            .border_l_1()
            .border_color(theme.divider)
            .flex()
            .flex_col()
            .child(
                bar.child(heading).child(
                    div()
                        .id("panel-close")
                        .flex_none()
                        .size(px(36.))
                        .rounded_full()
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .hover(|el| el.bg(theme.bg_sunken))
                        .child(crate::wallet::components::close_icon(
                            theme,
                            &mut self.icons,
                        ))
                        .on_click(cx.listener(|this, _, _, cx| {
                            // The contact form's column closes the FORM: the
                            // detail it was opened over is still there.
                            if this.section == Section::Contacts && this.contact_form.is_some() {
                                this.contact_form = None;
                                cx.notify();
                                return;
                            }
                            // Closing the SIGNING column is an answer, and the
                            // core decides which one: a request not yet
                            // submitted is rejected (4001), one already
                            // submitted or failed is merely dismissed, and a
                            // close over the top-up cancels the funding. This
                            // file used to just hide the column, which left
                            // the dApp's promise hanging until a navigation
                            // happened to settle it.
                            #[cfg(not(target_os = "linux"))]
                            if this.panel == PanelId::Signing
                                && let Some(host) = this.signing_host.clone()
                            {
                                host.update(cx, |host, cx| {
                                    host.dispatch_sign(
                                        vela_core::app::sign_request::Event::SwipeDismissed,
                                        cx,
                                    );
                                });
                            }
                            this.panel = PanelId::None;
                            cx.notify();
                        })),
                ),
            )
            .child({
                // A new subject starts at its top: the scroll of the last
                // asset, flow step or request is not this one's.
                let subject = format!(
                    "{:?}{:?}{:?}",
                    self.panel,
                    self.flows.last(),
                    self.asset_detail
                );
                if self.panel_scroll_subject != subject {
                    self.panel_scroll_subject = subject;
                    self.panel_scroll.set_offset(gpui::point(px(0.), px(0.)));
                }
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(
                        div()
                            .id("panel-body")
                            .track_scroll(&self.panel_scroll)
                            .size_full()
                            .overflow_y_scroll()
                            .px(px(24.))
                            .pb(px(24.))
                            .child(body),
                    )
                    .children(crate::ui::vertical_scrollbar(theme, &self.panel_scroll))
            })
    }

    /// The flow column: spec 015's panel scaffold plus the back chevron the
    /// wallet-2 mocks draw beside the title.
    fn flow_scaffold(
        &mut self,
        theme: &Theme,
        title: SharedString,
        back: Option<SharedString>,
        body: Div,
        cx: &mut Context<Self>,
    ) -> Div {
        // The root of a flow has nowhere to step back TO — closing the column
        // and stepping back one level are different gestures, and only the
        // close button should offer the first.
        let Some(_label) = back else {
            return self.panel_scaffold_with(theme, title, None, true, body, cx);
        };
        let chevron = div()
            .id("flow-back")
            .w(px(28.))
            .h(px(28.))
            .rounded(px(14.))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer()
            .hover(|el| el.bg(theme.bg_sunken))
            .child(crate::wallet::components::icon_img(
                &mut self.icons,
                Icon::ChevronLeft,
                false,
                theme.fg_muted,
                16.,
            ))
            .on_click(cx.listener(|this, _, _, cx| {
                this.flow_back(cx);
            }));
        self.panel_scaffold_with(
            theme,
            title,
            Some(chevron.into_any_element()),
            true,
            body,
            cx,
        )
    }

    /// DT3L's two tabs, and — on the native one — the network search in the
    /// field, its suggestions and its CTA (078 F-07, the web's
    /// `liveAddNetworkTab` handlers). The ERC-20 tab keeps the bindings
    /// `flow_actions` gave it.
    fn bind_add_token_tabs(&mut self, actions: &mut panels::PanelActions, cx: &mut Context<Self>) {
        actions.add_token_tabs = Some((
            Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                if this.add_token_native {
                    this.add_token_native = false;
                    this.add_net_added = None;
                    // A half-finished network search does not wait behind the
                    // other tab: the web resets it on the same switch.
                    resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                        if !resident.view().wizard.query.is_empty() {
                            resident.dispatch(NetEvent::WizardReset, cx);
                        }
                    });
                    cx.notify();
                }
            })) as panels::Click,
            Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.add_token_native = true;
                cx.notify();
            })) as panels::Click,
        ));
        if !self.add_token_native {
            return;
        }

        let wizard = resident::resident::<NetworkAdmin>(cx)
            .read(cx)
            .view()
            .wizard;
        let value = match &self.add_net_added {
            Some((_, query)) => query.clone(),
            None => wizard.query.clone(),
        };
        let page = cx.entity().downgrade();
        actions.address_field = Some(panels::AddressField {
            focus: self.add_token_focus.clone(),
            value,
            placeholder: self.flow_strings.net_search_placeholder.clone(),
            on_change: Box::new(
                move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                    // A new search is a new question: the last add's card goes.
                    let _ = page.update(cx, |this, _| this.add_net_added = None);
                    resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                        resident.dispatch(NetEvent::SearchInput { query: text }, cx);
                    });
                },
            ),
        });
        actions.add_token_picks = wizard
            .suggestions
            .iter()
            .map(|entry| {
                let chain_id = entry.chain_id;
                Box::new(cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                    resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                        resident.dispatch(
                            NetEvent::ChainSelected {
                                chain_id,
                                keep_custom_rpc: false,
                            },
                            cx,
                        );
                    });
                    cx.notify();
                })) as panels::Click
            })
            .collect();
        actions.add_to_wallet = Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            // Added on the core's word, not the press: `add_confirmed`
            // refuses silently unless the wizard is Checked and
            // compatible, and `last_added_chain_id` moving is the only
            // sign it did not.
            let now_iso = crate::executor::now_iso();
            let added = resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                let view = resident.view();
                let info = view.wizard.chain_info.clone()?;
                let query = view.wizard.query.clone();
                let before = view.last_added_chain_id;
                resident.dispatch(NetEvent::AddConfirmed { now_iso }, cx);
                (resident.view().last_added_chain_id != before).then_some((info, query))
            });
            if let Some(added) = added {
                this.add_net_added = Some(added);
                // A chain nobody has counted yet: the same forced read the
                // settings dialog makes on its add.
                crate::executor::balance_dashboard::refresh(cx);
            }
            cx.notify();
        })) as panels::Click);
    }

    /// Open a flow from the wallet home (spec 021 SC-002).
    fn enter_flow(&mut self, entry: FlowEntry, cx: &mut Context<Self>) {
        self.enter_flow_with(entry, SendOpenParams::default(), cx);
    }

    /// Open a flow, with what a send should open on — a token's own 转账
    /// names that token (the web's `enter('send', { assetId })`).
    fn enter_flow_with(
        &mut self,
        entry: FlowEntry,
        params: SendOpenParams,
        cx: &mut Context<Self>,
    ) {
        self.flows = FlowPanel::entry(entry);
        self.panel = PanelId::Flow;
        self.send_host = None;
        self.send_fee_picker = false;
        if entry == FlowEntry::AddToken {
            // Every add starts on ERC-20 with a clean network search, as the
            // web's panel does on open.
            self.add_token_native = false;
            self.add_net_added = None;
            if self.identity.is_some() {
                resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                    resident.dispatch(NetEvent::WizardReset, cx);
                });
            }
        }
        if entry == FlowEntry::Send && self.identity.is_some() {
            // A fresh journey starts on the one-token list, whatever the last
            // one ended in.
            self.send_sweeping = false;
            self.open_send(params, cx);
        }
        // A person looking at one network who presses 收款 means THAT network.
        // The web makes the same jump for the same reason; without it the
        // filter says Gnosis and the code that opens is Ethereum's.
        if entry == FlowEntry::Receive
            && let Some(chain_id) = self.chain_filter
        {
            self.receive_chain = chain_id;
        }
    }

    /// Spec 032: the send journey's machines, born with the flow. The mocks
    /// keep drawing when there is no account to send from.
    fn open_send(&mut self, params: SendOpenParams, cx: &mut Context<Self>) {
        let Some(account) = money::active_account() else {
            return;
        };
        let display = self.send_display(cx);
        let window_handle = self.window_handle;
        let host = cx.new(|cx| SendHost::open(account, params, display, window_handle, cx));
        cx.observe(&host, |_, _, cx| cx.notify()).detach();
        self.send_host = Some(host);
        // Every send starts on 全部: a chip left lit from the last one would
        // hide tokens with nothing on screen saying why.
        self.send_class = flows_live::SendClass::All;
    }

    /// The display currency, as the send machine's context: its code, the
    /// USD rate the core committed (`None` = unpriceable, never 1), and the
    /// fiat input's precision.
    fn send_display(&self, cx: &mut Context<Self>) -> SendDisplayContext {
        let view = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        SendDisplayContext {
            fiat_decimals: if matches!(view.code.as_str(), "JPY" | "KRW" | "VND" | "IDR") {
                0
            } else {
                2
            },
            rate: view.rate,
            code: view.code,
        }
    }

    /// The live send machines' views, when the flow is live.
    /// The picker's rows narrowed to the sidebar's network and the lit chip —
    /// on the token picker only. BOTH the drawing and the bindings go through
    /// this, so a row and the listener it gets are the same token.
    fn narrow_for_picker(&self, panel: FlowPanel, view: &mut vela_core::app::send::SendView) {
        if panel == FlowPanel::Dsd1 {
            flows_live::narrow_send_tokens(view, self.chain_filter, self.send_class);
        }
    }

    fn send_views(
        &self,
        cx: &Context<Self>,
    ) -> Option<(
        vela_core::app::send::SendView,
        vela_core::app::fee_policy::FeeView,
    )> {
        let host = self.send_host.as_ref()?.read(cx);
        Some((host.view.clone(), host.fee_view().clone()))
    }

    /// The speed control's inputs (spec 069): the core's view, and the fee
    /// session pricing each offered tier.
    fn send_speed(&self, cx: &Context<Self>) -> Option<flows_live::SpeedInputs> {
        let host = self.send_host.as_ref()?.read(cx);
        Some(flows_live::SpeedInputs {
            view: host.speed_view().clone(),
            tier_views: host.speed_tier_views(),
        })
    }

    /// Which panel the flow column shows. For a live send the CORE's stage
    /// decides, and the stack is rebuilt from it so the chevron stays truthful;
    /// a core that asked to leave takes the machines with it.
    fn sync_send_flow(&mut self, cx: &mut Context<Self>) -> Option<FlowPanel> {
        if let Some(host) = self.send_host.clone() {
            if host.read(cx).closed {
                self.send_host = None;
                self.send_fee_picker = false;
                self.flows.clear();
                self.panel = PanelId::None;
                return None;
            }
            let view = &host.read(cx).view;
            let panel = flows_live::send_panel(view, self.send_fee_picker);
            self.flows = panel.stack();
            // The core's scanner rides on top of whatever the send is on: the
            // column keeps the form, and `scan_overlay` sees DS1 last and
            // draws the modal over it (078 F-01).
            if view.show_scanner {
                self.flows.push(FlowPanel::Ds1);
            }
            return Some(panel);
        }
        self.flows.last().copied()
    }

    /// Back, one level. A live send asks the CORE to step back — it owns the
    /// step — and the page only closes what it opened itself.
    fn flow_back(&mut self, cx: &mut Context<Self>) {
        if let Some(host) = self.send_host.clone() {
            match self.flows.last().copied() {
                Some(FlowPanel::Dsd2e) => host.update(cx, |host, cx| {
                    host.dispatch(SendEvent::CloseContactPicker, cx);
                }),
                Some(FlowPanel::Dsd2c) => host.update(cx, |host, cx| {
                    host.dispatch(SendEvent::CloseBatchImport, cx);
                }),
                Some(FlowPanel::Dsd2f) => self.send_fee_picker = false,
                Some(FlowPanel::Dsd1) | None => {
                    self.send_host = None;
                    self.flows.clear();
                    self.panel = PanelId::None;
                }
                Some(_) => host.update(cx, |host, cx| host.dispatch(SendEvent::Back, cx)),
            }
            cx.notify();
            return;
        }
        self.flows.pop();
        if self.flows.is_empty() {
            self.panel = PanelId::None;
        }
        cx.notify();
    }

    /// What the live send panels bind to.
    fn send_bindings(&mut self, panel: FlowPanel, cx: &mut Context<Self>) -> Option<SendBindings> {
        // What currency every `≈` figure below is drawn in.
        let currency = self.money(cx);
        let host = self.send_host.clone()?;
        let (mut view, fee) = self.send_views(cx)?;
        self.narrow_for_picker(panel, &mut view);
        let contact_addresses = if panel == FlowPanel::Dsd2e {
            flows_live::contact_addresses(&resident::resident::<Contacts>(cx).read(cx).view())
        } else {
            Vec::new()
        };
        let batch_rate = self.send_host.as_ref().and_then(|host| {
            host.read(cx)
                .batch_view
                .as_ref()
                .map(|batch| batch.rate_input.clone())
        });
        let identity = self.identity();
        let way_out = flows_live::notice_way_out(
            &flows_live::SendInputs {
                send: &view,
                fee: &fee,
                s: &self.flow_strings,
                wallet: &self.strings,
                locale: &self.locale,
                money: &currency,
                identity_name: &identity.name,
                identity_address: &identity.address,
                speed: None,
            },
            panel == FlowPanel::Dsd3,
        );
        let group_members = if panel == FlowPanel::Dsd2e {
            flows_live::contact_group_members(&resident::resident::<Contacts>(cx).read(cx).view())
        } else {
            Vec::new()
        };
        Some(SendBindings {
            host,
            token_ids: flows_live::send_token_ids(&view),
            contact_addresses,
            fee_contracts: flows_live::fee_token_contracts(&fee),
            amount: view.amount.clone(),
            recipient: view.recipient.clone(),
            amount_focus: self.send_amount_focus.clone(),
            recipient_focus: self.send_recipient_focus.clone(),
            batch_rate,
            rate_focus: self.send_rate_focus.clone(),
            group_members,
            way_out,
            split_focuses: {
                // One per row, made on first use and kept: focus is which box
                // the keystrokes go into, and a row that lost its handle
                // between frames would lose the caret mid-amount.
                while self.split_focuses.len() < view.recipients.len() {
                    self.split_focuses.push(cx.focus_handle());
                }
                self.split_focuses[..view.recipients.len()].to_vec()
            },
            split_address_focuses: {
                while self.split_address_focuses.len() < view.recipients.len() {
                    self.split_address_focuses.push(cx.focus_handle());
                }
                self.split_address_focuses[..view.recipients.len()].to_vec()
            },
            recipients: view.recipients.clone(),
            fill_empty: flows_live::split_fill_source(&view).map(str::to_owned),
            sweeping: self.send_sweeping,
            token_chain_ids: view.tokens.iter().map(|token| token.chain_id).collect(),
            multi_chain_id: view.multi_chain_id,
        })
    }

    /// The Trusted Signer's four dialogs (specs 071 and 075), over whichever flow
    /// started the attempt — the signing column or the send: where the signer
    /// is, the cross-device pairing, the wait, and its last word. The buttons
    /// speak to the attempt through its channel; the host redraws when the
    /// channel answers.
    fn trusted_signer_prompt(
        &mut self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let mut channels = Vec::new();
        #[cfg(not(target_os = "linux"))]
        if let Some(host) = self.signing_host.as_ref() {
            channels.push(host.read(cx).trusted_signer());
        }
        if let Some(host) = self.send_host.as_ref() {
            channels.push(host.read(cx).trusted_signer());
        }
        let channel = channels
            .into_iter()
            .find(|channel| channel.waiting() || channel.ended().is_some())?;
        let card = if channel.waiting() {
            let (reopen, cancel) = (Arc::clone(&channel), channel);
            signing_trusted_signer::waiting_card(
                theme,
                &self.loc,
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| reopen.reopen(),
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| cancel.cancel(),
            )
        } else {
            let refusal = channel.ended()?;
            signing_trusted_signer::ended_card(
                theme,
                &self.loc,
                refusal,
                move |_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| channel.forget(),
            )
        };
        Some(
            div()
                .id("trusted-signer-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
                .child(card)
                .into_any_element(),
        )
    }

    /// The cable's dialogs and the core's alert, over the send flow.
    fn send_prompts(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let host = self.send_host.clone()?;
        let (touch, qr, pin, pick, alert) = {
            let read = host.read(cx);
            (
                read.touch_waiting(),
                read.qr_showing(),
                read.pin
                    .as_ref()
                    .map(|pin| (pin.request.clone(), pin.value.clone(), pin.focus.clone())),
                read.pick.clone(),
                read.alert.clone(),
            )
        };
        let scrim = |id: &'static str| {
            div()
                .id(id)
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
        };
        if let Some((request, value, focus)) = pin {
            let on_change = {
                let host = host.clone();
                move |text: String, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| {
                        if let Some(pin) = host.pin.as_mut() {
                            pin.value = text;
                        }
                        cx.notify();
                    });
                }
            };
            let on_confirm = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    let value = host.read(cx).pin.as_ref().map(|pin| pin.value.clone());
                    host.update(cx, |host, cx| host.answer_pin(value, cx));
                }
            };
            let on_cancel = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| host.answer_pin(None, cx));
                }
            };
            let card = hardware::pin_card(
                theme, &self.loc, &request, &value, &focus, window, on_change, on_confirm,
                on_cancel,
            );
            return Some(scrim("send-pin-scrim").child(card).into_any_element());
        }
        if let Some(choices) = pick {
            let on_pick = {
                let host = host.clone();
                move |index: &usize, _: &mut Window, cx: &mut gpui::App| {
                    let index = *index;
                    host.update(cx, |host, cx| host.answer_choice(Some(index), cx));
                }
            };
            let on_cancel = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| host.answer_choice(None, cx));
                }
            };
            let card = hardware::pick_card(theme, &self.loc, &choices, on_pick, on_cancel);
            return Some(scrim("send-pick-scrim").child(card).into_any_element());
        }
        if let Some(payload) = qr {
            let on_cancel = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| host.cancel_qr(cx));
                }
            };
            let card = hardware::qr_card(theme, &self.loc, &payload, on_cancel);
            return Some(scrim("send-qr-scrim").child(card).into_any_element());
        }
        if let Some(waiting) = touch {
            let on_cancel = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| host.cancel_touch(cx));
                }
            };
            let card = hardware::touch_card(theme, &self.loc, &waiting, on_cancel);
            return Some(scrim("send-touch-scrim").child(card).into_any_element());
        }
        if let Some(kind) = alert {
            let (title, body) = self.send_alert_words(&kind);
            let mut card = div()
                .w(px(400.))
                .flex()
                .flex_col()
                .gap(px(12.))
                .p(px(24.))
                .rounded(px(20.))
                .bg(theme.bg_raised)
                .border_1()
                .border_color(theme.border_card)
                .child(
                    div()
                        .text_size(theme::text_panel_title())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(title),
                );
            if let Some(body) = body {
                card = card.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(body),
                );
            }
            let dismiss = {
                let host = host.clone();
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    host.update(cx, |host, cx| host.acknowledge_alert(cx));
                }
            };
            card = card.child(
                div()
                    .id("send-alert-ok")
                    .h(px(CONTACTS_BUTTON_H))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(theme.accent)
                    .text_size(theme::text_row_title())
                    .text_color(theme.fg_inverse)
                    .child(self.flow_strings.done.clone())
                    .on_click(dismiss),
            );
            return Some(scrim("send-alert-scrim").child(card).into_any_element());
        }
        None
    }

    /// The core's alert kind, in the corpus's words. Semantic keys only —
    /// the core never hands over a sentence.
    fn send_alert_words(&self, kind: &SendAlertKind) -> (SharedString, Option<SharedString>) {
        let t = |key: &str| self.loc.t(key);
        match kind {
            SendAlertKind::InvalidAddress => (
                t("send.alertInvalidAddressTitle"),
                Some(t("send.alertInvalidAddressBody")),
            ),
            SendAlertKind::InvalidAmount => (
                t("send.alertInvalidAmountTitle"),
                Some(t("send.alertInvalidAmountBody")),
            ),
            SendAlertKind::InsufficientBalance { .. } | SendAlertKind::SplitOverBalance => (
                t("send.alertInsufficientBalanceTitle"),
                Some(t("send.alertInsufficientBalanceBody")),
            ),
            SendAlertKind::LoadTokensFailed => (t("send.alertLoadTokensError"), None),
            SendAlertKind::EstimateFailed { .. } => (
                t("send.alertEstimateFailedTitle"),
                Some(t("send.alertEstimateFailedBody")),
            ),
            SendAlertKind::AccountUnavailable => (t("send.alertAccountUnavailableBody"), None),
        }
    }

    /// DSD2cL's total line, which reads what the importer's own view does not
    /// carry: the balance of the coin being split.
    ///
    /// An import that SEEDS the form (or replaces its rows) is measured
    /// against the whole balance; one that adds to rows already there draws
    /// from `split_remaining` instead — the web's `formHasRows && !replaces`.
    fn dress_batch_total(&self, body: &mut flow_fixtures::FlowBody, cx: &mut Context<Self>) {
        let flow_fixtures::FlowBody::BatchImport(model) = body else {
            return;
        };
        let Some(host) = self.send_host.as_ref() else {
            return;
        };
        let host = host.read(cx);
        let (Some(batch), Some(token)) =
            (host.batch_view.as_ref(), host.view.selected_token.as_ref())
        else {
            return;
        };
        model.total = flows_live::batch_total(
            batch,
            &token.symbol,
            &token.balance,
            flows_live::batch_remaining(&host.view, host.batch_replaces),
            &self.flow_strings,
        );
    }

    /// The panel's body: the cores' for a real session, the mocks' otherwise.
    ///
    /// Not every panel has a live source yet — Send is 032's, and the scanner
    /// has no data at all. Those fall through to the fixture, which is the
    /// design and is honest about being one. What must NOT happen is a live
    /// panel silently borrowing a mock's numbers, so each live arm is written
    /// out rather than defaulted.
    fn flow_body(&mut self, panel: FlowPanel, cx: &mut Context<Self>) -> flow_fixtures::FlowBody {
        // What currency every `≈` figure below is drawn in.
        let currency = self.money(cx);
        let Some(identity) = self.identity.clone() else {
            return flow_fixtures::body(panel, &self.flow_strings);
        };
        match panel {
            FlowPanel::Dt1 | FlowPanel::Dt4 => {
                let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
                flow_fixtures::FlowBody::Assets(flows_live::assets(
                    &view,
                    &self.flow_strings,
                    &self.strings,
                    &self.locale,
                    self.chain_filter,
                    &currency,
                ))
            }
            FlowPanel::Da1 => {
                // Privacy comes from the BALANCE view, not the feed's own flag:
                // every money surface masks together.
                let balance = resident::resident::<BalanceDashboard>(cx).read(cx).view();
                let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
                flow_fixtures::FlowBody::History(flows_live::history_panel(
                    &feed,
                    balance.balance_unknown,
                    self.chain_filter,
                    &self.flow_strings,
                    &self.strings,
                    balance.hidden,
                ))
            }
            FlowPanel::Dr1 => flow_fixtures::FlowBody::Receive(flows_live::receive_list(
                &identity.address,
                &self.flow_strings,
            )),
            FlowPanel::Dr2 => {
                // Reading the resident BOOTS it, which is what starts the
                // watcher: the machine's own boot event is `Start`. So opening
                // this panel begins watching for money, and that is US3's
                // "a deposit lands and is noticed without a manual refresh".
                let watch = resident::resident::<ReceiveWatch>(cx).read(cx).view();
                // And `payment_request` decides WHAT the code says — the bare
                // recipient today, an EIP-681 URI once an amount can be asked
                // for. Encoding the address here instead would work now and
                // silently drop the amount later.
                let pay = resident::resident::<PaymentRequest>(cx).read(cx).view();
                flow_fixtures::FlowBody::ReceiveQr(flows_live::receive_qr(
                    &identity.address,
                    &identity.name,
                    self.receive_chain,
                    &watch,
                    &pay,
                    &self.flow_strings,
                    &self.locale,
                    &currency,
                ))
            }
            // Send (DSD*), the scanner, the asset QR and add-token still draw
            // the mock. Each is named so the next person sees a list rather
            // than a wildcard.
            FlowPanel::Da2 | FlowPanel::Da3 => {
                let hidden = resident::resident::<BalanceDashboard>(cx)
                    .read(cx)
                    .view()
                    .hidden;
                let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
                self.tx_detail
                    .as_ref()
                    .and_then(|id| {
                        flows_live::tx_detail(
                            &feed,
                            id,
                            &self.flow_strings,
                            hidden,
                            &self.locale,
                            &currency,
                        )
                    })
                    .map_or_else(
                        // The record is gone. The mock is not a substitute for
                        // it — that would show somebody a stranger's
                        // transaction under their own history — so the panel
                        // draws nothing and the chevron leads back.
                        || {
                            flow_fixtures::FlowBody::History(flow_fixtures::HistoryPanel {
                                groups: Vec::new(),
                                loading: false,
                                empty: None,
                            })
                        },
                        flow_fixtures::FlowBody::TxDetail,
                    )
            }
            FlowPanel::Dt3 if self.add_token_native => {
                let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
                let (added, query) = match &self.add_net_added {
                    Some((info, query)) => (Some(info), query.as_str()),
                    None => (None, view.wizard.query.as_str()),
                };
                flow_fixtures::FlowBody::AddToken(flows_live::add_network_tab(
                    &view.wizard,
                    query,
                    added,
                    &self.flow_strings,
                ))
            }
            FlowPanel::Dt3 => {
                let view = resident::resident::<ManageTokens>(cx).read(cx).view();
                flow_fixtures::FlowBody::AddToken(flows_live::add_token(&view, &self.flow_strings))
            }
            // Spec 032: the send journey reads its own two machines. The mocks
            // keep drawing when no flow is live (the gallery, an unsigned
            // window), and for the batch importer until its phase lands.
            FlowPanel::Dsd1
            | FlowPanel::Dsd2
            | FlowPanel::Dsd2b
            | FlowPanel::Dsd2e
            | FlowPanel::Dsd2f
            | FlowPanel::Dsd3
            | FlowPanel::Dsd4 => match self.send_views(cx) {
                Some((mut send, fee)) => {
                    self.narrow_for_picker(panel, &mut send);
                    let identity = self.identity();
                    let speed = self.send_speed(cx);
                    let inputs = flows_live::SendInputs {
                        send: &send,
                        fee: &fee,
                        s: &self.flow_strings,
                        wallet: &self.strings,
                        locale: &self.locale,
                        money: &currency,
                        identity_name: &identity.name,
                        identity_address: &identity.address,
                        speed: speed.as_ref(),
                    };
                    match panel {
                        FlowPanel::Dsd1 => {
                            flow_fixtures::FlowBody::SendPick(flows_live::send_pick_with(
                                &inputs,
                                self.send_sweeping,
                                self.send_class,
                            ))
                        }
                        FlowPanel::Dsd2 | FlowPanel::Dsd2b => {
                            flow_fixtures::FlowBody::SendForm(flows_live::send_form(&inputs))
                        }
                        FlowPanel::Dsd2e => {
                            let contacts = resident::resident::<Contacts>(cx).read(cx).view();
                            flow_fixtures::FlowBody::ContactPick(flows_live::contact_pick(
                                &contacts,
                                &self.flow_strings,
                            ))
                        }
                        FlowPanel::Dsd2f => {
                            flow_fixtures::FlowBody::FeeToken(flows_live::fee_token(&inputs))
                        }
                        FlowPanel::Dsd3 => {
                            flow_fixtures::FlowBody::SendConfirm(flows_live::send_confirm(&inputs))
                        }
                        _ => {
                            flow_fixtures::FlowBody::SendReceipt(flows_live::send_receipt(&inputs))
                        }
                    }
                }
                None => flow_fixtures::body(panel, &self.flow_strings),
            },
            // Spec 032 phase 5: the importer reads its own machine, which the
            // host opens when the send machine shows the sheet.
            FlowPanel::Dsd2c => {
                let live = self.send_host.as_ref().and_then(|host| {
                    let host = host.read(cx);
                    let symbol = host.view.selected_token.as_ref()?.symbol.clone();
                    let batch = host.batch_view.clone()?;
                    Some((
                        batch,
                        symbol,
                        host.view.split_import_room,
                        host.batch_replaces,
                    ))
                });
                match live {
                    Some((batch, symbol, room, replaces)) => {
                        let mut model =
                            flows_live::batch_import(&batch, &symbol, &self.flow_strings);
                        model.merge =
                            flows_live::batch_merge(&batch, room, replaces, &self.flow_strings);
                        flow_fixtures::FlowBody::BatchImport(model)
                    }
                    None => flow_fixtures::body(panel, &self.flow_strings),
                }
            }
            // A token's own code, when its detail opened one; the mock
            // otherwise (the gallery, `VELA_FLOW=DR3`).
            FlowPanel::Dr3 => match self.receive_token.clone() {
                Some(token) => {
                    let watch = resident::resident::<ReceiveWatch>(cx).read(cx).view();
                    let pay = resident::resident::<PaymentRequest>(cx).read(cx).view();
                    flow_fixtures::FlowBody::ReceiveQr(flows_live::receive_token_qr(
                        &identity.address,
                        &identity.name,
                        &token,
                        &watch,
                        &pay,
                        &self.flow_strings,
                        &self.locale,
                        &currency,
                    ))
                }
                None => flow_fixtures::body(panel, &self.flow_strings),
            },
            FlowPanel::Ds1 | FlowPanel::Dt3b => flow_fixtures::body(panel, &self.flow_strings),
        }
    }

    /// Take one step deeper into the open flow.
    ///
    /// `FlowPanel::step` is the only place that knows where a step leads, so a
    /// step the mocks do not draw is a no-op here rather than a wrong panel.
    fn push_step(&mut self, step: FlowStep) {
        let Some(current) = self.flows.last().copied() else {
            return;
        };
        if let Some(next) = current.step(step) {
            self.flows.push(next);
        }
    }

    /// One bound listener that pushes `step` onto the flow stack.
    fn step_action(step: FlowStep, cx: &mut Context<Self>) -> panels::Click {
        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
            this.push_step(step);
            cx.notify();
        }))
    }

    /// The listeners this panel's affordances answer to.
    ///
    /// Bound from `FlowPanel::step`, so an affordance is live exactly when the
    /// mocks draw somewhere for it to go — the chevron and the destination
    /// cannot drift apart.
    fn flow_actions(
        panel: FlowPanel,
        live: bool,
        tx_ids: Vec<String>,
        focus: &gpui::FocusHandle,
        address: &str,
        placeholder: &SharedString,
        send: Option<SendBindings>,
        cx: &mut Context<Self>,
    ) -> panels::PanelActions {
        let bind = |step: FlowStep, cx: &mut Context<Self>| {
            panel.step(step).map(|_| Self::step_action(step, cx))
        };
        let mut actions = panels::PanelActions {
            open_qr: bind(FlowStep::ReceiveQr, cx),
            // "I Understand": the gate the core keeps per account. Bound for
            // every panel — the receive screen is the only one that draws it,
            // and binding it there and nowhere else is what a `None` in the
            // other panels already says.
            acknowledge: Some(Box::new(cx.listener(|_, _: &gpui::ClickEvent, _, cx| {
                resident::resident::<PaymentRequest>(cx).update(cx, |resident, cx| {
                    resident.dispatch(vela_core::app::payment_request::Event::Acknowledge, cx);
                });
                cx.notify();
            })) as panels::Click),
            save_image: Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.save_share_card(cx);
            })) as panels::Click),
            open_qr_rows: Vec::new(),
            open_tx: bind(FlowStep::TxDetail, cx),
            open_tx_rows: Vec::new(),
            open_send_form: bind(FlowStep::SendForm, cx),
            open_fee_token: bind(FlowStep::FeeToken, cx),
            refresh_fee: None,
            toggle_speed: None,
            pick_speed_rows: Vec::new(),
            open_contact_pick: bind(FlowStep::ContactPick, cx),
            open_add_token: bind(FlowStep::AddToken, cx),
            open_receive: None,
            delete_tx: None,
            open_scan: bind(FlowStep::Scan, cx),
            add_recipient: bind(FlowStep::AddRecipient, cx),
            open_batch_import: bind(FlowStep::BatchImport, cx),
            advance: bind(FlowStep::SendConfirm, cx).or(bind(FlowStep::SendReceipt, cx)),
            address_field: None,
            add_to_wallet: None,
            add_token_tabs: None,
            add_token_picks: Vec::new(),
            open_send_rows: Vec::new(),
            send_class_chips: Vec::new(),
            sweep_select_all: None,
            send_pick_cta: None,
            amount_field: None,
            recipient_field: None,
            tap_max: None,
            toggle_denom: None,
            pick_contact_rows: Vec::new(),
            fee_rows: Vec::new(),
            batch_unit: None,
            batch_paste: None,
            batch_pick_file: None,
            batch_template: None,
            batch_rate_field: None,
            batch_rate_reset: None,
            batch_merge: None,
            notice_action: None,
            notice_dismiss: None,
            pick_group_rows: Vec::new(),
            split_amount_fields: Vec::new(),
            split_address_fields: Vec::new(),
            pick_recipient_rows: Vec::new(),
            remove_recipient_rows: Vec::new(),
            fill_empty: None,
            search: None,
            copy: None,
        };
        // DR1L, live: one listener per network row, each remembering WHICH
        // chain it opened. The fixture keeps its single first-row listener,
        // because every mock row opens the same picture and binding twelve
        // identical closures to say so would be noise.
        if live && panel == FlowPanel::Dr1 && panel.step(FlowStep::ReceiveQr).is_some() {
            actions.open_qr_rows = flows_live::receivable_chains()
                .into_iter()
                .map(|(chain_id, _)| -> panels::Click {
                    Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        this.receive_chain = chain_id;
                        this.push_step(FlowStep::ReceiveQr);
                        cx.notify();
                    }))
                })
                .collect();
        }

        // DA1L, live: one listener per row, each carrying the id of the
        // transaction it opens. `flows_live::history_ids` walks the feed the
        // same way `panels::history` draws it, so row N opens record N — two
        // walks that could disagree would open the wrong transaction, which on
        // a money screen is worse than opening nothing.
        if live && panel == FlowPanel::Da1 && panel.step(FlowStep::TxDetail).is_some() {
            actions.open_tx_rows = tx_ids
                .into_iter()
                .map(|id| -> panels::Click {
                    Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        this.tx_detail = Some(id.clone());
                        this.push_step(FlowStep::TxDetail);
                        cx.notify();
                    }))
                })
                .collect();
        }

        // DT4L, live: the empty state's caption says "tap here to see your
        // address and receive tokens", so tapping it opens Receive — the
        // web's `.empty-tap`, `go('receive')`.
        if live && matches!(panel, FlowPanel::Dt1 | FlowPanel::Dt4) {
            actions.open_receive = Some(Box::new(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.enter_flow(FlowEntry::Receive, cx);
                    cx.notify();
                },
            )));
        }

        // DA2L, live: delete this record (the web's `deleteSelectedTx`). The
        // feed tombstones it and drops the row at once (`DeleteRequested`);
        // the chain keeps the transaction. The detail has nothing left to
        // show, so the column steps back to the list it came from.
        if live && matches!(panel, FlowPanel::Da2 | FlowPanel::Da3) {
            actions.delete_tx = Some(Box::new(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    let Some(id) = this.tx_detail.take() else {
                        return;
                    };
                    resident::resident::<ActivityFeed>(cx).update(cx, |resident, cx| {
                        resident.dispatch(
                            vela_core::app::activity_feed::Event::DeleteRequested { id },
                            cx,
                        );
                    });
                    this.flow_back(cx);
                },
            )));
        }

        // DT3L, live: a real field, and a CTA that saves what it found.
        if live && panel == FlowPanel::Dt3 {
            actions.address_field = Some(panels::AddressField {
                focus: focus.clone(),
                value: address.to_owned(),
                placeholder: placeholder.clone(),
                on_change: Box::new(
                    move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                        // Straight into the core: it owns validation, it clears the
                        // found cards, and it decides when a search may run. A
                        // shell-side copy would be a second opinion about what the
                        // person typed.
                        let entity = resident::resident::<ManageTokens>(cx);
                        entity.update(cx, |resident, cx| {
                            resident.dispatch(MtokEvent::AddressInput { s: text }, cx);
                        });
                        // The desktop drawing has no search button — the found
                        // card simply appears — so the search fires as soon as
                        // the core says the address is one. WHEN to ask is the
                        // shell's; whether the ask may RUN is still the core's,
                        // which ignores a request while one is in flight.
                        let view = entity.read(cx).view();
                        if view.address_valid && !view.detecting && view.found.is_empty() {
                            let networks = flows_live::receivable_chains()
                                .into_iter()
                                .map(|(chain_id, _)| MtokNetwork {
                                    chain_id,
                                    name: crate::executor::custom_tokens::network_name(chain_id),
                                })
                                .collect();
                            entity.update(cx, |resident, cx| {
                                resident.dispatch(MtokEvent::DetectRequested { networks }, cx);
                            });
                        }
                    },
                ),
            });
            actions.add_to_wallet = Some(Box::new(
                move |_: &gpui::ClickEvent, _window: &mut Window, cx: &mut gpui::App| {
                    let entity = resident::resident::<ManageTokens>(cx);
                    let found = entity.read(cx).view().found.first().map(|f| f.chain_id);
                    // Nothing found means nothing to save. The CTA is drawn
                    // either way because the panel is one drawing; what it must
                    // not do is save a token nobody looked up.
                    if let Some(chain_id) = found {
                        entity.update(cx, |resident, cx| {
                            resident.dispatch(MtokEvent::SaveRequested { chain_id }, cx);
                        });
                    }
                },
            ));
        }

        // DSD4's CTA is "close · keep running": the transfer outlives the
        // panel, so the last step out of the flow is out of the column.
        if panel == FlowPanel::Dsd4 {
            actions.advance = Some(Box::new(cx.listener(
                |this, _: &gpui::ClickEvent, _, cx| {
                    this.flows.clear();
                    this.panel = PanelId::None;
                    cx.notify();
                },
            )));
        }

        // Spec 032, live: every affordance on the send panels is an EVENT to
        // the core, which owns the step, the validation and the gates. The
        // page's own hand is the fee sheet, the one overlay the core has no
        // flag for.
        if let Some(send) = send {
            let host = send.host;
            let to_host = |event: SendEvent| -> panels::Click {
                let host = host.clone();
                Box::new(
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        host.update(cx, |host, cx| host.dispatch(event.clone(), cx));
                    },
                )
            };
            // The way out the core's own refusal offered. `EditAmount` is its
            // recovery from a blocked confirmation; the other two are the
            // retries it defines.
            // Leaving the treasury stop. The core has had this event since
            // the machine was written and nothing ever sent it, so the only
            // way out of "the relay cannot pay on this chain" was closing the
            // whole journey.
            actions.notice_dismiss = Some(to_host(SendEvent::DismissTreasurySheet));
            actions.notice_action = send.way_out.map(|way_out| match way_out {
                flows_live::NoticeWayOut::RetryAfterBootstrap => {
                    to_host(SendEvent::RetryAfterBootstrap)
                }
                flows_live::NoticeWayOut::AddNetwork { chain_id } => {
                    to_host(SendEvent::AddNetworkTapped { chain_id })
                }
                flows_live::NoticeWayOut::EditAmount => to_host(SendEvent::EditAmount),
            });
            match panel {
                FlowPanel::Dsd1 => {
                    actions.open_send_form = None;
                    actions.send_class_chips = flows_live::SendClass::CHIPS
                        .iter()
                        .map(|&class| -> panels::Click {
                            Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                this.send_class = class;
                                cx.notify();
                            }))
                        })
                        .collect();
                    let sweeping = send.sweeping;
                    let chain_ids = send.token_chain_ids.clone();
                    let pinned = send.multi_chain_id;
                    actions.open_send_rows = send
                        .token_ids
                        .iter()
                        .cloned()
                        .zip(chain_ids)
                        .map(|(token_id, chain_id)| -> panels::Click {
                            if !sweeping {
                                return to_host(SendEvent::SelectToken { token_id });
                            }
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    host.update(cx, |host, cx| {
                                        // A batch is one chain, and the FIRST
                                        // pick is what names it — after that
                                        // the core refuses every other chain.
                                        // Emptying the selection unpins it, so
                                        // starting over needs no way out of
                                        // the screen.
                                        if pinned.is_none() {
                                            host.dispatch(
                                                SendEvent::SetMultiNetwork {
                                                    chain_id: Some(chain_id),
                                                },
                                                cx,
                                            );
                                        }
                                        host.dispatch(
                                            SendEvent::ToggleMultiToken {
                                                token_id: token_id.clone(),
                                            },
                                            cx,
                                        );
                                    });
                                },
                            )
                        })
                        .collect();
                    // The scope is what the picker is SHOWING; which of those
                    // count as valuable stays the core's answer.
                    let visible_ids = send.token_ids.clone();
                    actions.sweep_select_all = sweeping.then(|| {
                        let host = host.clone();
                        Box::new(
                            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                host.update(cx, |host, cx| {
                                    host.dispatch(
                                        SendEvent::ToggleAllMultiTokens {
                                            visible_ids: visible_ids.clone(),
                                        },
                                        cx,
                                    );
                                });
                            },
                        ) as panels::Click
                    });
                    // One slot, two jobs: enter the sweep, or confirm what it
                    // has ticked. Which one it is, is what the label already
                    // says.
                    actions.send_pick_cta = Some(if sweeping {
                        to_host(SendEvent::ConfirmMultiSelection)
                    } else {
                        Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                            this.send_sweeping = true;
                            cx.notify();
                        })) as panels::Click
                    });
                }
                FlowPanel::Dsd2 | FlowPanel::Dsd2b => {
                    // DSD2bL: a split row's own amount, and the X that has
                    // been drawn on that card since spec 021 with nothing
                    // behind it. Every edit sends the WHOLE list back, because
                    // `RecipientsChanged` is the event the machine offers.
                    // …and its own address and book (078 F-06).
                    for (index, focus) in send.split_address_focuses.iter().enumerate() {
                        let rows = send.recipients.clone();
                        actions.split_address_fields.push(panels::AddressField {
                            focus: focus.clone(),
                            value: rows
                                .get(index)
                                .map(|r| r.address.clone())
                                .unwrap_or_default(),
                            placeholder: SharedString::from("0x…"),
                            on_change: Box::new({
                                let host = host.clone();
                                move |address: String, _: &mut Window, cx: &mut gpui::App| {
                                    let next =
                                        flows_live::split_address_edited(&rows, index, &address);
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::RecipientsChanged { recipients: next },
                                            cx,
                                        );
                                    });
                                }
                            }),
                        });
                        if let Some(id) = send.recipients.get(index).map(|r| r.id.clone()) {
                            actions
                                .pick_recipient_rows
                                .push(to_host(SendEvent::OpenContactPicker { target: Some(id) }));
                        }
                    }
                    for (index, focus) in send.split_focuses.iter().enumerate() {
                        let rows = send.recipients.clone();
                        actions.split_amount_fields.push(panels::AddressField {
                            focus: focus.clone(),
                            value: rows
                                .get(index)
                                .map(|r| r.amount.clone())
                                .unwrap_or_default(),
                            placeholder: SharedString::from("0"),
                            on_change: Box::new({
                                let host = host.clone();
                                move |amount: String, _: &mut Window, cx: &mut gpui::App| {
                                    let next = flows_live::split_amount_edited(
                                        &rows,
                                        index,
                                        amount.clone(),
                                    );
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::RecipientsChanged { recipients: next },
                                            cx,
                                        );
                                    });
                                }
                            }),
                        });
                        let rows = send.recipients.clone();
                        actions.remove_recipient_rows.push({
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    let next = flows_live::split_row_removed(&rows, index);
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::RecipientsChanged { recipients: next },
                                            cx,
                                        );
                                    });
                                },
                            ) as panels::Click
                        });
                    }
                    // "Use X for the empty rows": the whole list back, the
                    // empty rows carrying the typed figure.
                    if let Some(amount) = send.fill_empty.clone() {
                        let rows = send.recipients.clone();
                        let host = host.clone();
                        actions.fill_empty = Some(Box::new(
                            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                let next = flows_live::split_empty_filled(&rows, &amount);
                                host.update(cx, |host, cx| {
                                    host.dispatch(
                                        SendEvent::RecipientsChanged { recipients: next },
                                        cx,
                                    );
                                });
                            },
                        ) as panels::Click);
                    }
                    // "+ 添加收款人" in split mode appends a BLANK row rather
                    // than stepping panels — the same words do the same thing
                    // on the web, and the core assigns the row's id.
                    if !send.recipients.is_empty() {
                        let rows = send.recipients.clone();
                        let host = host.clone();
                        actions.add_recipient = Some(Box::new(
                            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                let next = flows_live::split_row_appended(&rows);
                                host.update(cx, |host, cx| {
                                    host.dispatch(
                                        SendEvent::RecipientsChanged { recipients: next },
                                        cx,
                                    );
                                });
                            },
                        ) as panels::Click);
                    }
                    let previous = send.amount.clone();
                    actions.amount_field = Some(panels::AddressField {
                        focus: send.amount_focus,
                        value: send.amount,
                        placeholder: SharedString::from("0"),
                        on_change: Box::new({
                            let host = host.clone();
                            move |amount: String, _: &mut Window, cx: &mut gpui::App| {
                                // Spec 073: the core's amount rule first.
                                let Some(amount) = flows_live::amount_edited(&amount, &previous)
                                else {
                                    return;
                                };
                                host.update(cx, |host, cx| {
                                    host.dispatch(SendEvent::SetAmount { amount }, cx);
                                });
                            }
                        }),
                    });
                    actions.recipient_field = Some(panels::AddressField {
                        focus: send.recipient_focus,
                        value: send.recipient,
                        placeholder: SharedString::from("0x…"),
                        on_change: Box::new({
                            let host = host.clone();
                            move |recipient: String, _: &mut Window, cx: &mut gpui::App| {
                                host.update(cx, |host, cx| {
                                    host.dispatch(SendEvent::SetRecipient { recipient }, cx);
                                });
                            }
                        }),
                    });
                    actions.tap_max = Some(to_host(SendEvent::TapMax));
                    // ⇄: the core owns the swap — whether it is possible, and
                    // what becomes of the figure; the page only says it was
                    // pressed (the web's `toggle_fiat_input`, #197).
                    actions.toggle_denom = Some(to_host(SendEvent::ToggleFiatInput));
                    actions.open_contact_pick =
                        Some(to_host(SendEvent::OpenContactPicker { target: None }));
                    // "+ add recipient" turns one payee into a split — but on
                    // a split it is the blank-row append bound above, which
                    // this used to overwrite: the core ignores
                    // `EnterSplitMode` inside a split, so the pill was dead.
                    if send.recipients.is_empty() {
                        actions.add_recipient = Some(to_host(SendEvent::EnterSplitMode));
                    }
                    actions.open_batch_import = Some(to_host(SendEvent::OpenBatchImport));
                    actions.open_fee_token = Some(Box::new(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            this.send_fee_picker = true;
                            cx.notify();
                        },
                    )));
                    // Spec 069: measure again, and the speed control — every
                    // decision behind it is the `fee_speed` core's.
                    let on_host =
                        |step: fn(&mut SendHost, &mut Context<SendHost>)| -> panels::Click {
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    host.update(cx, step);
                                },
                            )
                        };
                    actions.refresh_fee = Some(on_host(SendHost::refresh_fee));
                    actions.toggle_speed = Some(on_host(SendHost::toggle_speed));
                    actions.pick_speed_rows = host
                        .read(cx)
                        .speed_view()
                        .options
                        .iter()
                        .map(|option| {
                            let (host, tier) = (host.clone(), option.tier);
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    host.update(cx, |host, cx| host.pick_speed(tier, cx));
                                },
                            ) as panels::Click
                        })
                        .collect();
                    actions.advance = Some(to_host(SendEvent::Continue));
                    // The recipient field's scan (078 F-01). The scanner is the
                    // CORE's state, as on the web: `OpenScanner` raises it and
                    // `ScanResolved` fills the form and takes it down.
                    actions.open_scan = Some(to_host(SendEvent::OpenScanner));
                }
                FlowPanel::Dsd2e => {
                    // The picker's "scan to fill" row opens the same scanner;
                    // the scan IS the pick, and the core closes the picker with
                    // it (issue #270).
                    actions.open_scan = Some(to_host(SendEvent::OpenScanner));
                    // A pick lands in the field AND closes the sheet. The core
                    // at this branch point leaves the sheet up after
                    // `PickedAddress`; spec 028's core closes it itself, after
                    // which the second event is a no-op — the pair is kept so
                    // either core makes the same screen.
                    // A whole group is ADDED to whoever is already on the
                    // form, at amounts the person still has to type — the
                    // same hand-off web calls 群发转账. It used to seed, which
                    // replaces: picking a second group threw the first away.
                    // The core assigns the row ids.
                    actions.pick_group_rows = send
                        .group_members
                        .into_iter()
                        .map(|members| -> panels::Click {
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    let recipients = members.clone();
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::AppendSplitRecipients { recipients },
                                            cx,
                                        );
                                        host.dispatch(SendEvent::CloseContactPicker, cx);
                                    });
                                },
                            )
                        })
                        .collect();
                    actions.pick_contact_rows = send
                        .contact_addresses
                        .into_iter()
                        .map(|address| -> panels::Click {
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::PickedAddress {
                                                address: address.clone(),
                                            },
                                            cx,
                                        );
                                        host.dispatch(SendEvent::CloseContactPicker, cx);
                                    });
                                },
                            )
                        })
                        .collect();
                }
                FlowPanel::Dsd2f => {
                    actions.fee_rows = send
                        .fee_contracts
                        .into_iter()
                        .map(|token| -> panels::Click {
                            Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                this.send_fee_picker = false;
                                if let Some(host) = this.send_host.clone() {
                                    let token = token.clone();
                                    host.update(cx, |host, cx| {
                                        host.fee_dispatch(
                                            FeeEvent::SelectFeeAsset {
                                                token: token.clone(),
                                            },
                                            cx,
                                        );
                                        host.dispatch(SendEvent::ChooseFeeToken { token }, cx);
                                    });
                                }
                                cx.notify();
                            }))
                        })
                        .collect();
                }
                FlowPanel::Dsd2c => {
                    let to_batch = |event: BatchEvent| -> panels::Click {
                        let host = host.clone();
                        Box::new(
                            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                host.update(cx, |host, cx| host.batch_dispatch(event.clone(), cx));
                            },
                        )
                    };
                    actions.batch_unit = Some((
                        to_batch(BatchEvent::SetUnit {
                            unit: BatchUnit::Fiat,
                        }),
                        to_batch(BatchEvent::SetUnit {
                            unit: BatchUnit::Token,
                        }),
                    ));
                    actions.batch_paste = Some(Box::new({
                        let host = host.clone();
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            host.update(cx, |host, cx| host.paste_into_batch(cx));
                        }
                    }));
                    actions.batch_pick_file = Some(to_batch(BatchEvent::PickFileRequested));
                    actions.batch_template = Some(to_batch(BatchEvent::SaveTemplateRequested));
                    actions.batch_rate_reset = Some(to_batch(BatchEvent::ResetRateToAuto));
                    actions.batch_merge = Some(Box::new({
                        let host = host.clone();
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            host.update(cx, |host, cx| host.toggle_batch_merge(cx));
                        }
                    }));
                    actions.batch_rate_field = Some(panels::AddressField {
                        focus: send.rate_focus,
                        value: send.batch_rate.unwrap_or_default(),
                        placeholder: SharedString::from("0"),
                        on_change: Box::new({
                            let host = host.clone();
                            move |text: String, _: &mut Window, cx: &mut gpui::App| {
                                host.update(cx, |host, cx| {
                                    host.batch_dispatch(BatchEvent::EditRate { text }, cx);
                                });
                            }
                        }),
                    });
                    actions.advance = Some(to_batch(BatchEvent::Apply));
                }
                FlowPanel::Dsd3 => actions.advance = Some(to_host(SendEvent::SlideConfirm)),
                FlowPanel::Dsd4 => actions.advance = Some(to_host(SendEvent::Done)),
                _ => {}
            }
        }
        actions
    }

    fn receive_body(&mut self, theme: &Theme) -> Div {
        let s = &self.strings;
        let picker = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .p(px(12.))
            .rounded(px(14.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border_card)
            .child(token_icon(theme, "BNB", fixtures::chain_bnb()))
            .child(
                div()
                    .flex_1()
                    .min_w(px(0.))
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child("BNB"),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(fixtures::receive_network_detail(s)),
                    ),
            );

        let address_box = div()
            .p(px(14.))
            .rounded(px(10.))
            .bg(theme.bg_sunken)
            .font_family(theme::font_mono())
            .text_size(theme::text_mono_address())
            .text_color(theme.fg_base)
            .child(SharedString::from(self.identity().address));

        // The button said "copy address" and copied nothing. A receive screen's
        // whole job is to hand an address over, and the two ways it does that —
        // the code and this button — were both decorative until 031.
        let address = self.identity().address;
        let copy = div()
            .id("copy-address")
            .h(px(48.))
            .rounded(px(14.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.outline_strong)
            .flex()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .cursor_pointer()
            .hover(|el| el.bg(theme.bg_sunken))
            .text_size(theme::text_row_title())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(theme.fg_base)
            .child(crate::wallet::components::copy_icon(theme, &mut self.icons))
            .child(s.copy_address.clone())
            .on_click(move |_, _, cx: &mut gpui::App| {
                cx.write_to_clipboard(gpui::ClipboardItem::new_string(address.clone()));
            });

        let warning = div()
            .p(px(14.))
            .rounded(px(14.))
            .bg(theme.warning_soft)
            .border_1()
            .border_color(theme.warning_border)
            .flex()
            .flex_col()
            .gap(px(6.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.warning)
                    .child(crate::wallet::components::warning_icon(
                        theme,
                        &mut self.icons,
                    ))
                    .child(s.warning_title.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(s.warning_reminder.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(fixtures::receive_networks_line(s)),
            );

        div()
            .flex()
            .flex_col()
            .gap(px(14.))
            .child(picker)
            .child(qr_placeholder(theme, s.qr_caption.clone(), px(220.)))
            .child(
                div()
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.fg_subtle)
                    .child(s.address_label.clone()),
            )
            .child(address_box)
            .child(copy)
            .child(warning)
    }

    fn asset_detail_body(
        &mut self,
        model: &fixtures::AssetDetailModel,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = &self.strings;
        // The holding this panel is about, as the core holds it NOW — the
        // doors below act on it. `None` for the mock and for a holding a
        // refresh took away, and then the doors do nothing rather than act
        // on whichever row took its place.
        let token = self
            .asset_detail
            .filter(|_| self.identity.is_some())
            .and_then(|index| {
                resident::resident::<BalanceDashboard>(cx)
                    .read(cx)
                    .view()
                    .tokens
                    .get(index)
                    .filter(|token| token.symbol.as_str() == model.ticker.as_ref())
                    .cloned()
            });

        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(token_icon_logos(
                theme,
                model.ticker.as_ref(),
                model.badge,
                &model.logos,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .child(model.amount.clone()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(model.sub.clone()),
                    ),
            );

        let buttons = div()
            .flex()
            .gap(px(12.))
            .child(
                action_pill(
                    "detail-send",
                    theme,
                    &mut self.icons,
                    Icon::ArrowUpRight,
                    s.detail_send.clone(),
                )
                // `--size-control-md`: the panel's pair is a size down from
                // the home's actions (`AssetDetailPanel.svelte`).
                .h(px(44.))
                .on_click({
                    // 转账 from a token: the form with THAT token chosen.
                    let token = token.clone();
                    cx.listener(move |this, _, _, cx| {
                        let Some(token) = token.as_ref() else { return };
                        this.asset_detail = None;
                        this.enter_flow_with(
                            FlowEntry::Send,
                            wallet_live::token_send_params(token),
                            cx,
                        );
                        cx.notify();
                    })
                }),
            )
            .child(
                action_pill(
                    "detail-receive",
                    theme,
                    &mut self.icons,
                    Icon::ArrowDownLeft,
                    s.detail_receive.clone(),
                )
                .h(px(44.))
                .on_click({
                    // 收款 from a token: its own code, no picker in between —
                    // the token already names its chain.
                    let token = token.clone();
                    cx.listener(move |this, _, _, cx| {
                        let Some(token) = token.clone() else { return };
                        this.asset_detail = None;
                        this.receive_chain = token.chain_id;
                        this.receive_token = Some(token);
                        this.enter_flow(FlowEntry::ReceiveToken, cx);
                        cx.notify();
                    })
                }),
            );

        // A hairline above the first fact and under every one (the web's
        // `.facts` / `.fact`), and the Contract's copy — the whole address
        // for the clipboard, its two ends for reading (078 H-06).
        let mut facts = div()
            .flex()
            .flex_col()
            .border_t_1()
            .border_color(theme.border_card);
        let copied = self.copied.as_deref() == Some("asset-contract");
        for (label, value) in model.facts.iter().cloned() {
            let copy = (label == s.label_contract)
                .then(|| model.contract_copy.clone())
                .flatten();
            let mut value_side = div().flex().items_center().gap(px(4.)).child(
                div()
                    .text_size(theme::text_row_sub())
                    .font_weight(gpui::FontWeight::MEDIUM)
                    .text_color(theme.fg_base)
                    .child(value),
            );
            if let Some(address) = copy {
                value_side = value_side.child(
                    div()
                        .id("asset-contract-copy")
                        .size(px(20.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .child(icon_img(
                            &mut self.icons,
                            if copied { Icon::Check } else { Icon::Copy },
                            false,
                            if copied {
                                theme.success_base
                            } else {
                                theme.fg_subtle
                            },
                            14.,
                        ))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.copy_text(
                                "asset-contract",
                                address.to_string(),
                                std::time::Duration::from_millis(150),
                                cx,
                            );
                        })),
                );
            }
            facts = facts.child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(px(12.))
                    .py(px(12.))
                    // The web's line: 13 at `leading-normal`. gpui's default
                    // leading made each fact ~4px taller than the web's.
                    .line_height(gpui::relative(1.4))
                    .border_b_1()
                    .border_color(theme.border_card)
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(label),
                    )
                    .child(value_side),
            );
        }

        // No explorer for this chain: the same words in the system's
        // unavailable look (the web's disabled button, spec 081 #18) — never a
        // link to a wrong page, and not a gap where the link was either.
        let explorer_line = div()
            .id("detail-explorer")
            .flex()
            .items_center()
            .gap(px(4.))
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .child(s.view_on_explorer.clone())
            .child(crate::wallet::components::chevron_icon(
                theme,
                &mut self.icons,
            ));
        let explorer = Some(match model.explorer_url.clone() {
            Some(url) => explorer_line
                .cursor_pointer()
                .hover(|el| el.text_color(theme.fg_base))
                .on_click(move |_, _, cx| cx.open_url(&url)),
            None => explorer_line.opacity(theme::OPACITY_DISABLED),
        });

        let mut tx = div().flex().flex_col().child(
            div()
                .pt(px(8.))
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(s.label_transactions.clone()),
        );
        for (i, row) in model.activity.iter().enumerate() {
            // Each row opens its own detail — the id from the same walk that
            // drew it. The mock's rows are nobody's and open nothing.
            let id = model.activity_ids.get(i).cloned();
            tx = tx.child(
                div()
                    .id(ElementId::from(("detail-activity", i)))
                    .when(id.is_some(), |el| el.cursor_pointer())
                    .child(activity_row(theme, &mut self.icons, row))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        let Some(id) = id.clone() else { return };
                        this.asset_detail = None;
                        this.tx_detail = Some(id);
                        this.enter_flow(FlowEntry::TxDetail, cx);
                        cx.notify();
                    })),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(head)
            .child(buttons)
            .child(facts)
            .children(explorer)
            .child(tx)
    }

    // -- gallery chrome ------------------------------------------------------

    fn chip(
        &self,
        id: impl Into<ElementId>,
        theme: &Theme,
        label: SharedString,
        active: bool,
    ) -> Stateful<Div> {
        let chip = div()
            .id(id)
            .h(px(28.))
            .px(px(12.))
            .rounded(px(14.))
            .flex()
            .items_center()
            .cursor_pointer()
            .text_size(theme::text_row_sub())
            .border_1()
            .child(label);
        if active {
            chip.bg(theme.accent)
                .border_color(theme.accent)
                .text_color(theme.fg_inverse)
        } else {
            chip.bg(theme.bg_raised)
                .border_color(theme.border_card)
                .text_color(theme.fg_muted)
                .hover(|el| el.bg(theme.bg_sunken))
        }
    }

    /// One gallery chip = one mock state (FR-004: ≤ 2 interactions from the
    /// gallery root). Every field the mocks differ in is set here, so the
    /// states cannot leak into each other.
    /// Gallery-only: show one flow panel, with the stack the mocks imply so
    /// its back chevron behaves the way it does in the app.
    fn select_flow(&mut self, panel: FlowPanel) {
        self.section = Section::Wallet;
        self.menu = None;
        self.flows = panel.stack();
        self.panel = PanelId::Flow;
    }

    fn select_tab(&mut self, tab: GalleryTab, window: &Window) {
        self.tab = tab;
        self.section = match tab {
            GalleryTab::Dc1
            | GalleryTab::Dc2
            | GalleryTab::Dc3
            | GalleryTab::Dc4
            | GalleryTab::Dc5
            | GalleryTab::Dc6 => Section::Contacts,
            GalleryTab::Dst1
            | GalleryTab::Dst2
            | GalleryTab::Dst3
            | GalleryTab::Dst4
            | GalleryTab::Dst4b
            | GalleryTab::Dst5
            | GalleryTab::Dst6
            | GalleryTab::Dst7
            | GalleryTab::Dst8
            | GalleryTab::Dsr1 => Section::Settings,
            _ => Section::Wallet,
        };
        self.flows.clear();
        // Spec 023: which panel, which dialog, and which row is expanded — set
        // together so one chip is one mock and the states cannot leak.
        self.settings_page = match tab {
            GalleryTab::Dst2 => SettingsPage::Appearance,
            GalleryTab::Dst3 => SettingsPage::Localization,
            GalleryTab::Dst4 | GalleryTab::Dst4b => SettingsPage::Networks,
            GalleryTab::Dst5 => SettingsPage::RpcProviders,
            GalleryTab::Dst6 => SettingsPage::Endpoints,
            GalleryTab::Dst7 => SettingsPage::Storage,
            GalleryTab::Dst8 => SettingsPage::About,
            _ => SettingsPage::Account,
        };
        self.settings_dialog = match tab {
            GalleryTab::Dst4b => Some(SettingsDialog::AddNetwork),
            GalleryTab::Dsr1 => Some(SettingsDialog::FixRpc),
            _ => None,
        };
        // DST4 opens Ethereum in place — the one row the mock has expanded.
        self.settings_expanded_network =
            (tab == GalleryTab::Dst4).then(|| gpui::SharedString::from("ethereum"));
        // DST3 is the only state with an open dropdown, and it hangs off 数字格式.
        self.settings_open_dropdown = (tab == GalleryTab::Dst3).then_some("number");
        self.panel = match tab {
            GalleryTab::D2 => PanelId::Receive,
            GalleryTab::D3 => PanelId::AssetDetail,
            GalleryTab::Dc2 => PanelId::ContactDetail,
            _ => PanelId::None,
        };
        // D1b is D1 with the celebration up. Set here rather than read from a
        // core, because the drawing has none — and cleared by every other chip,
        // so a toast cannot leak onto the state next door.
        self.celebrating = tab == GalleryTab::D1b;
        self.contact = 0;
        self.contacts_empty = tab == GalleryTab::Dc3;
        self.group = match tab {
            GalleryTab::Dc4 | GalleryTab::Dc6 => Some(0),
            _ => None,
        };
        self.menu = match tab {
            GalleryTab::Dc5 => Some((
                ContactsMenu::Header,
                self.header_menu_anchor(window),
                Anchor::TopRight,
            )),
            GalleryTab::Dc6 => Some((
                ContactsMenu::Group,
                self.group_menu_anchor(window),
                Anchor::TopLeft,
            )),
            _ => None,
        };
    }

    fn gallery_bar(&mut self, theme: &Theme, caption: bool, cx: &mut Context<Self>) -> Div {
        let tabs = GalleryTab::ALL;
        let mut bar = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .px(px(16.))
            .py(px(8.))
            .bg(theme.bg_base)
            .border_b_1()
            .border_color(theme.divider)
            // Clear the traffic lights on macOS.
            .pl(px(84.))
            // …and the drag strip where the page draws its own caption: a tab
            // under it would hit-test as caption on Windows and never see the
            // click. Same idiom as the traffic-light padding above.
            .pt(px(8. + gallery_bar_caption_pad(caption)));
        for (i, (tab, label)) in tabs.into_iter().enumerate() {
            let active = self.tab == tab;
            bar = bar.child(
                self.chip(ElementId::from(("tab", i)), theme, label.into(), active)
                    .on_click(cx.listener(move |this, _, window, cx| {
                        this.select_tab(tab, window);
                        cx.notify();
                    })),
            );
        }
        // Spec 021's nineteen chips, generated from the matrix.
        for (i, (panel, label)) in GalleryTab::flow_chips().enumerate() {
            let active = self.panel == PanelId::Flow && self.flows.last() == Some(&panel);
            bar = bar.child(
                self.chip(
                    ElementId::from(("flow-tab", i)),
                    theme,
                    label.into(),
                    active,
                )
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.select_flow(panel);
                    cx.notify();
                })),
            );
        }
        let mode_label: SharedString = match self.theme_mode() {
            ThemeMode::Light => "light".into(),
            ThemeMode::Dark => "dark".into(),
        };
        bar.child(div().flex_1()).child(
            self.chip("toggle-theme", theme, mode_label, false)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.override_mode = Some(match this.theme_mode() {
                        ThemeMode::Light => ThemeMode::Dark,
                        ThemeMode::Dark => ThemeMode::Light,
                    });
                    cx.notify();
                })),
        )
    }

    fn board(theme: &Theme, title: &str, body: Div) -> Div {
        div()
            .w(px(560.))
            .p(px(16.))
            .rounded(px(12.))
            .bg(theme.bg_base)
            .border_1()
            .border_color(theme.divider)
            .flex()
            .flex_col()
            .gap(px(10.))
            .child(
                div()
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_subtle)
                    .child(SharedString::from(title.to_owned())),
            )
            .child(body)
    }

    fn components_tab(&mut self, theme: &Theme) -> Stateful<Div> {
        let s_clone = fixtures::balance_variants(&self.strings);
        let mut balances = div().flex().flex_col().gap(px(16.));
        for model in &s_clone {
            balances = balances.child(balance_display(theme, &mut self.icons, model, None, None));
        }

        let mut rows = div().flex().flex_col();
        for row in fixtures::activity_default(&self.strings) {
            rows = rows.child(activity_row(theme, &mut self.icons, &row));
        }
        for row in fixtures::activity_masked(&self.strings) {
            rows = rows.child(activity_row(theme, &mut self.icons, &row));
        }

        let mut assets = div().flex().flex_col();
        for (i, row) in fixtures::assets_variants(&self.strings).iter().enumerate() {
            assets = assets.child(asset_row(
                ElementId::from(("board-asset", i)),
                theme,
                &mut self.icons,
                row,
            ));
        }

        let mut chains_col = div().flex().flex_col().gap(px(2.));
        for (i, row) in fixtures::chains(&self.strings).iter().enumerate() {
            chains_col = chains_col.child(chain_row(
                ElementId::from(("board-chain", i)),
                theme,
                &mut self.icons,
                row,
            ));
        }

        let empties = div()
            .flex()
            .gap(px(16.))
            .child(empty_state(
                theme,
                &mut self.icons,
                Icon::Inbox,
                self.strings.empty_activity_title.clone(),
                self.strings.empty_activity_caption.clone(),
            ))
            .child(empty_state(
                theme,
                &mut self.icons,
                Icon::WalletOutline,
                self.strings.empty_assets_title.clone(),
                self.strings.empty_assets_caption.clone(),
            ));

        let skeletons = div()
            .flex()
            .flex_col()
            .child(skeleton_row(theme))
            .child(skeleton_row(theme));

        let qr = qr_placeholder(theme, self.strings.qr_caption.clone(), px(160.));

        div()
            .id("components-scroll")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(24.))
            .bg(theme.bg_sunken)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .child(Self::board(theme, "BalanceDisplay", balances))
                    .child(Self::board(theme, "ActivityRow", rows))
                    .child(Self::board(theme, "AssetRow", assets))
                    .child(Self::board(theme, "ChainFilterList", chains_col))
                    .child(Self::board(theme, "EmptyState", empties))
                    .child(Self::board(theme, "SkeletonRow", skeletons))
                    .child(Self::board(theme, "QRPlaceholder", qr)),
            )
    }

    /// The contacts component board (data-model.md §Component boards). Every
    /// new component and its variants, plus the identicon board over the 8+1
    /// canon seeds and the placeholder.
    fn contacts_components_tab(&mut self, theme: &Theme) -> Stateful<Div> {
        let s_all = self.contacts.all_contacts.clone();
        let s_groups = self.contacts.section_groups.clone();
        let s_new_group = self.contacts.group_new.clone();
        let s_add_member = self.contacts.add_member.clone();
        let s_search = self.contacts.search_placeholder.clone();
        let s_address = self.contacts.address_label.clone();
        let s_recent = self.contacts.recent_activity.clone();
        let s_empty = self.contacts.empty.clone();
        let s_empty_hint = self.contacts.empty_hint.clone();
        let s_add_contact = self.contacts.add_contact.clone();
        let s_import = self.contacts.import_file.clone();
        let s_batch = self.contacts.batch_send.clone();
        let s_delete = self.contacts.delete_contact.clone();
        let no_results = contacts_fixtures::no_results_label(&self.contacts, "zzz");

        // ContactRow: default · selected · long-name truncation · member.
        let mut rows = div().flex().flex_col();
        for (i, (contact, selected)) in [
            (contacts_fixtures::CONTACTS[0], false),
            (contacts_fixtures::CONTACTS[1], true),
            (contacts_fixtures::CONTACTS[2], false),
            (contacts_fixtures::COUSIN, false),
        ]
        .into_iter()
        .enumerate()
        {
            rows = rows.child(contact_row(
                ElementId::from(("board-contact", i)),
                theme,
                &mut self.identicons,
                &contacts_fixtures::row_model(contact),
                selected,
            ));
            rows = rows.child(row_divider(theme));
        }

        // GroupRail rows: all-contacts (selected) · group · drop-target · new.
        let mut rail = div().flex().flex_col().gap(px(2.));
        rail = rail.child(rail_row(
            "board-rail-all",
            theme,
            &mut self.icons,
            None,
            s_all,
            Some(contacts_fixtures::TOTAL_CONTACTS),
            RailState::Selected,
        ));
        rail = rail.child(rail_label(theme, s_groups));
        for (i, group) in contacts_fixtures::GROUPS.iter().enumerate() {
            rail = rail.child(rail_row(
                ElementId::from(("board-rail-group", i)),
                theme,
                &mut self.icons,
                Some(Icon::UsersRound),
                SharedString::from(group.name),
                Some(group.count),
                // 交易所 shows the drag-over variant (desktop SPEC).
                if i == 2 {
                    RailState::DropTarget
                } else {
                    RailState::Default
                },
            ));
        }
        rail = rail.child(rail_row(
            "board-rail-new",
            theme,
            &mut self.icons,
            Some(Icon::FolderPlus),
            s_new_group,
            None,
            RailState::Default,
        ));

        let search = search_field(theme, &mut self.icons, s_search, None);

        let dropdown = menu_card(
            theme,
            &mut self.icons,
            &contacts_fixtures::header_dropdown(&self.contacts),
            // The component board is a picture of the menu, not a menu.
            Vec::new(),
        );
        let group_menu = menu_card(
            theme,
            &mut self.icons,
            &contacts_fixtures::group_context(&self.contacts),
            // The component board is a picture of the menu, not a menu.
            Vec::new(),
        );
        let contact_menu = menu_card(
            theme,
            &mut self.icons,
            &contacts_fixtures::contact_context(&self.contacts),
            // The component board is a picture of the menu, not a menu.
            Vec::new(),
        );
        let menus = div()
            .flex()
            .gap(px(16.))
            .child(dropdown)
            .child(group_menu)
            .child(contact_menu);

        let chips = div()
            .flex()
            .gap(px(6.))
            .child(group_chip(theme, "家人".into()))
            .child(add_chip(
                theme,
                &mut self.icons,
                self.contacts.section_groups.clone(),
            ));

        let address = address_block(
            theme,
            &mut self.icons,
            s_address,
            contacts_fixtures::CONTACTS[0].address_full.into(),
            // The component board is a picture: there is no address here worth
            // putting on a real clipboard.
            None,
        );

        let mut recent = div().flex().flex_col().child(
            div()
                .pb(px(4.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(s_recent),
        );
        for row in contacts_fixtures::alice_activity(&self.contacts) {
            recent = recent.child(activity_row(theme, &mut self.icons, &row));
        }

        let empties = div()
            .flex()
            .gap(px(16.))
            .child(empty_state_cta(
                theme,
                &mut self.icons,
                s_empty,
                s_empty_hint,
                s_add_contact,
                s_import,
                None,
            ))
            .child(empty_state(
                theme,
                &mut self.icons,
                Icon::Search,
                no_results,
                self.contacts.search_placeholder.clone(),
            ));

        let ghost = div().child(ghost_add_row(
            "board-ghost",
            theme,
            &mut self.icons,
            s_add_member,
        ));

        let buttons = div()
            .flex()
            .gap(px(12.))
            .child(accent_button(
                "board-accent",
                theme,
                &mut self.icons,
                None,
                s_batch,
            ))
            .child(outline_button(
                "board-outline",
                theme,
                &mut self.icons,
                Some(Icon::UserRoundPlus),
                self.contacts.add_contact.clone(),
            ))
            .child(icon_button(
                "board-icon",
                theme,
                &mut self.icons,
                Icon::Ellipsis,
            ))
            .child(destructive_text_button(
                "board-destructive",
                theme,
                s_delete,
            ));

        let mut seeds = div().flex().flex_wrap().gap(px(16.));
        for (i, seed) in contacts_fixtures::IDENTICON_CANON_SEEDS
            .into_iter()
            .enumerate()
        {
            let caption: SharedString = if seed.is_empty() {
                "(empty)".into()
            } else {
                seed.into()
            };
            let _ = i;
            seeds = seeds.child(
                div()
                    .w(px(96.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(6.))
                    .child(identicon_avatar(&mut self.identicons, seed, 48.))
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .whitespace_nowrap()
                            .truncate()
                            .w_full()
                            .text_center()
                            .child(caption),
                    ),
            );
        }

        div()
            .id("contacts-components-scroll")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(24.))
            .bg(theme.bg_sunken)
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .gap(px(16.))
                    .child(Self::board(theme, "ContactRow", rows))
                    .child(Self::board(theme, "GroupRail", rail))
                    .child(Self::board(theme, "SearchField", search))
                    .child(Self::board(theme, "DropdownMenu / ContextMenu", menus))
                    .child(Self::board(theme, "GroupChips", chips))
                    .child(Self::board(theme, "AddressBlock", address))
                    .child(Self::board(theme, "RecentActivity", recent))
                    .child(Self::board(theme, "EmptyStateCTA", empties))
                    .child(Self::board(theme, "GhostAddRow", ghost))
                    .child(Self::board(theme, "Buttons", buttons))
                    .child(Self::board(theme, "IdenticonAvatar (canon seeds)", seeds)),
            )
    }

    fn identicons_tab(&mut self, theme: &Theme) -> Stateful<Div> {
        let mut wrap = div().flex().flex_wrap().gap(px(24.));
        for seed in IDENTICON_BOARD_SEEDS {
            let caption: SharedString = if seed.is_empty() {
                "(empty)".into()
            } else {
                seed.into()
            };
            wrap = wrap.child(
                div()
                    .w(px(160.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .child(identicon_avatar(&mut self.identicons, seed, 56.))
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .whitespace_nowrap()
                            .truncate()
                            .w_full()
                            .text_center()
                            .child(caption),
                    ),
            );
        }
        div()
            .id("identicons-scroll")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(24.))
            .bg(theme.bg_sunken)
            .child(wrap)
    }

    // -- spec 023: the settings section --------------------------------------

    /// Column 2 of the settings section: the 216px second-level nav.
    ///
    /// The same column the contacts group rail occupies, doing the same job one
    /// section over — which is why it reuses the width rather than inventing a
    /// second one.
    fn settings_nav(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let title = self.settings.title.clone();
        let current = self.settings_page;
        let mut col = div().flex().flex_col().gap(px(2.)).child(
            div()
                .px(px(12.))
                .pb(px(16.))
                // One line, truncated if it must be. Wrapping broke
                // "Einstellungen" into "Einstellung" / "en" at `xlarge` — a
                // hyphenless break inside a word, which reads as a rendering
                // fault rather than a long title.
                .w_full()
                .min_w(px(0.))
                .whitespace_nowrap()
                .truncate()
                .text_size(theme::text_panel_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(title),
        );
        for (i, page) in SettingsPage::ALL.into_iter().enumerate() {
            let label = page.label(&self.settings);
            let row = settings_nav_row(
                ElementId::from(("settings-nav", i)),
                theme,
                &mut self.icons,
                page.icon(),
                label,
                page == current,
            );
            col = col.child(row.on_click(cx.listener(move |this, _, _, cx| {
                if page != SettingsPage::Account {
                    // The switcher left the screen. A subscription nobody
                    // closes is a poll of every account's balances.
                    this.close_switcher(cx);
                }
                this.settings_page = page;
                this.settings_dialog = None;
                // Leaving a service panel forgets that it announced itself, so
                // coming back re-probes rather than showing what the last visit
                // measured.
                this.settings_probed_panel = None;
                cx.notify();
            })));
        }

        div()
            .w(px(theme::settings_nav_w()))
            .h_full()
            .flex_none()
            // `.flex()` is load-bearing, not decoration: without it `h_full`
            // does not resolve and the column stopped at its last row, leaving
            // its background and right border hanging in mid-air.
            .flex()
            .flex_col()
            .bg(theme.bg_sunken)
            .border_r_1()
            .border_color(theme.divider)
            .p(px(SIDEBAR_PAD))
            .pt(px(SIDEBAR_TOP))
            .child(col)
    }

    /// Column 3: the panel the nav selected.
    fn settings_panel(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        let (title, description) = match self.settings_page {
            SettingsPage::Account => (self.settings.nav_account.clone(), None),
            SettingsPage::Appearance => (self.settings.nav_appearance.clone(), None),
            SettingsPage::Localization => (
                self.settings.nav_localization.clone(),
                Some(self.settings.number_subtitle.clone()),
            ),
            SettingsPage::Networks => (
                self.settings.nav_networks.clone(),
                Some(self.settings.networks_subtitle.clone()),
            ),
            SettingsPage::RpcProviders => (self.settings.nav_rpc_providers.clone(), None),
            SettingsPage::Endpoints => (self.settings.nav_endpoints.clone(), None),
            SettingsPage::FeeSpeed => (
                self.settings.fee_speed_title.clone(),
                Some(self.settings.fee_speed_subtitle.clone()),
            ),
            SettingsPage::Signing => (
                self.settings.nav_signing.clone(),
                Some(self.settings.signing_subtitle.clone()),
            ),
            SettingsPage::Storage => (
                self.settings.nav_storage.clone(),
                Some(self.settings.storage_subtitle.clone()),
            ),
            SettingsPage::About => (self.settings.nav_about.clone(), None),
        };

        let mut head = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(16.))
            .pb(px(24.))
            .child({
                // `flex_1` + `min_w(0)`, or this column takes its INTRINSIC
                // width in the row: a description that does not wrap then runs
                // off the right edge of the window instead of folding onto a
                // second line. Seen at `xlarge` on Transaction speed, in both
                // English and German, once the nav column beside it grew.
                let mut titles = div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w(px(0.))
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .child(title),
                    );
                if let Some(description) = description {
                    titles = titles.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(description),
                    );
                }
                titles
            });

        // 添加网络 sits in the panel header, next to the list it adds to.
        if self.settings_page == SettingsPage::Networks {
            let add = self.settings.add_network.clone();
            head = head.child(
                div()
                    .id("settings-add-network")
                    .h(px(36.))
                    .px(px(16.))
                    .rounded(px(10.))
                    .flex()
                    .flex_none()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .bg(theme.bg_raised)
                    .border_1()
                    .border_color(theme.divider)
                    .hover(|el| el.bg(theme.bg_sunken))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.settings_dialog = Some(SettingsDialog::AddNetwork);
                        cx.notify();
                    }))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::Plus,
                        false,
                        theme.fg_base,
                        14.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(add),
                    ),
            );
        }

        let body = match self.settings_page {
            SettingsPage::Account => self.settings_account(theme, cx),
            SettingsPage::Appearance => self.settings_appearance(theme, cx),
            SettingsPage::Localization => self.settings_localization(theme, cx),
            SettingsPage::Networks => self.settings_networks(theme, window, cx),
            SettingsPage::RpcProviders => self.settings_providers(theme, window, cx),
            SettingsPage::Endpoints => self.settings_endpoints(theme, window, cx),
            SettingsPage::FeeSpeed => self.settings_fee_speed(theme, cx),
            SettingsPage::Signing => self.settings_signing(theme, window, cx),
            SettingsPage::Storage => self.settings_storage(theme, cx),
            SettingsPage::About => self.settings_about(theme, cx),
        };

        // The banner DSR1 draws over the wallet, kept above the panel content
        // so it reads as a condition of the app rather than of this panel.
        //
        // Live since 031, and this is SC-003's visible half: the fetch reports
        // an unreachable chain separately from an empty one, the core turns
        // that into `banner_chain_ids` (failed MINUS rate-limited), and this is
        // where the person finally sees it. A correct verdict nobody is shown
        // is, from the chair in front of the screen, no verdict.
        let live = self.identity.is_some().then(|| {
            let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
            (wallet_live::unreachable_chips(&view), view.banner_chain_ids)
        });
        let banner_chain_ids = live
            .as_ref()
            .map(|(_, ids)| ids.clone())
            .unwrap_or_default();
        let live_chips = live.map(|(chips, _)| chips);
        let action = self.settings.rpc_fix_action.clone();
        let banner = match live_chips {
            Some(chips) if !chips.is_empty() => {
                let text = SharedString::from(crate::wallet::fill(
                    &self.settings.rpc_unavailable_multiple,
                    "count",
                    &chips.len().to_string(),
                ));
                let chips = chips
                    .into_iter()
                    .enumerate()
                    .map(|(index, (letter, color, name))| {
                        // Each chip opens ITS chain's editor. A single "fix"
                        // button that always opened the first one would send
                        // somebody to repair a network that is working.
                        let chain_id = banner_chain_ids.get(index).copied();
                        let on_click: Option<panels::Click> = chain_id.map(|chain_id| {
                            Box::new(cx.listener(move |this: &mut Self, _, _, cx| {
                                this.settings_fix_chain = Some(chain_id);
                                this.settings_dialog = Some(SettingsDialog::FixRpc);
                                cx.notify();
                            })) as panels::Click
                        });
                        (letter, color, name, action.clone(), on_click)
                    })
                    .collect();
                Some(rpc_banner(theme, &mut self.icons, text, chips))
            }
            // A real session with every chain reachable shows nothing — the
            // mock state cannot override a live "all well".
            Some(_) => None,
            None if self.settings_dialog == Some(SettingsDialog::FixRpc) => {
                let text = settings_fixtures::banner_text(&self.settings);
                let chips = settings_fixtures::BANNER_CHAINS
                    .iter()
                    .map(|id| {
                        let n = settings_fixtures::network(id);
                        (
                            SharedString::from(n.letter),
                            n.color,
                            SharedString::from(n.name),
                            action.clone(),
                            None,
                        )
                    })
                    .collect();
                Some(rpc_banner(theme, &mut self.icons, text, chips))
            }
            None => None,
        };

        let panel = div()
            .id("settings-panel")
            .track_scroll(&self.settings_scroll)
            .size_full()
            .overflow_y_scroll()
            // Left-aligned against the nav column, exactly as the wallet's own
            // content column is. The padding is the panel's, the cap is the
            // content's: a settings form stretched to a 2000px window is a
            // different screen from the one that was designed.
            .px(px(SETTINGS_PANEL_PAD_X))
            .pb(px(48.))
            .child(
                div()
                    .max_w(px(SETTINGS_PANEL_W))
                    .child(head)
                    .when_some(banner, |el, banner| {
                        el.child(div().pb(px(24.)).child(banner))
                    })
                    .child(body),
            );
        // Scrolls below the caption strip, as the Wallet column does: a
        // network row scrolled up under the window's buttons put its caret
        // through ─.
        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .pt(px(WALLET_PAD_TOP.max(crate::window_frame::CAPTION_H)))
            .flex()
            .flex_col()
            .child(
                div()
                    .relative()
                    .flex_1()
                    .min_h(px(0.))
                    .child(panel)
                    .children(crate::ui::vertical_scrollbar(theme, &self.settings_scroll)),
            )
    }

    /// DST1 — the accounts, the way out, and the one irreversible button.
    /// DST1, live: every account this person has, with the active one marked.
    ///
    /// The list is the switcher's (`accounts_body`), as the web's settings
    /// page draws the same `AccountsSheetBody` its switcher dialog does.
    fn settings_account_live(
        &mut self,
        theme: &Theme,
        session: &vela_core::app::session::SessionView,
        cx: &mut Context<Self>,
    ) -> Div {
        self.ensure_backup_check(cx);
        let s = &self.settings;
        let sign_out = s.sign_out_button.clone();
        let sign_out_desc = s.sign_out_desc.clone();
        let erase_title = s.erase_title.clone();
        let erase_subtitle = s.erase_subtitle.clone();
        let erase_confirm = s.erase_confirm.clone();
        div()
            .flex()
            .flex_col()
            .child(self.accounts_body(theme, session, false, cx))
            .child(self.settings_account_footer(
                theme,
                sign_out,
                sign_out_desc,
                erase_title,
                erase_subtitle,
                erase_confirm,
                cx,
            ))
    }

    /// The web's `AccountsSheetBody` (078 H-01): a summary, one row per
    /// account, and the two ways to add one. Drawn by the header's switcher
    /// dialog and by Settings → Account, as the web draws the one component in
    /// both — `in_dialog` is the switcher, whose every answer also closes it.
    ///
    /// Clicking another row switches to it. `SwitchAccount` has existed since
    /// 019 with nothing to trigger it — "an event with no control is dead
    /// code", as `session.rs` put it about this very event — and the control
    /// was drawn all along.
    fn accounts_body(
        &mut self,
        theme: &Theme,
        session: &vela_core::app::session::SessionView,
        in_dialog: bool,
        cx: &mut Context<Self>,
    ) -> Div {
        let accounts_count = self.settings.accounts_count.clone();
        let accounts_total = self.settings.accounts_total.clone();
        // Drawing this list IS the switcher opening: the core refreshes every
        // listed account's total while it is up and answers in
        // `switcher.balances`. Nobody ever told it, so the panel could only
        // say how MANY accounts there were — and its sentence ended on a
        // dangling "·" waiting for the half this adds.
        self.sync_switcher(session, cx);
        let summary_count = crate::wallet::fill(
            &accounts_count,
            "count",
            &session.accounts.len().to_string(),
        );

        let switcher = resident::resident::<BalanceDashboard>(cx)
            .read(cx)
            .view()
            .switcher;
        // The display currency the totals are stated in (spec 072) — the
        // web's switcher prints them the way the hero would.
        let currency = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        // "1 accounts · Total $0.75". The sum is over what is actually KNOWN —
        // an account with no cached figure contributes nothing rather than
        // making the sentence wait for it.
        let known_total: f64 = session
            .accounts
            .iter()
            .filter_map(|row| {
                switcher
                    .balances
                    .iter()
                    .find(|entry| entry.address.eq_ignore_ascii_case(&row.account.address))
                    .map(|entry| entry.usd)
            })
            .sum();
        let summary = gpui::SharedString::from(format!(
            "{summary_count}{}",
            crate::wallet::fill(
                &accounts_total,
                "amount",
                &settings_live::account_total(known_total, Some(&currency), &self.locale),
            )
        ));
        // Two copies of this list can be on screen at once — the dialog over
        // the settings panel — so each names its own rows.
        let (art_id, row_id, remove_id, confirm_id, cancel_id) = if in_dialog {
            (
                "switcher-art",
                "switcher-account",
                "switcher-remove",
                "switcher-remove-confirm",
                "switcher-remove-cancel",
            )
        } else {
            (
                "settings-art",
                "settings-account",
                "settings-remove",
                "settings-remove-confirm",
                "settings-remove-cancel",
            )
        };
        // The inline question belongs to the surface on top: with the dialog
        // up, the panel behind it does not ask as well.
        let asking = if in_dialog == self.account_switcher {
            self.removing_account
        } else {
            None
        };
        let remove_label = self.strings.account_remove.clone();
        let remove_body = self.strings.account_remove_body.clone();
        let cancel_label = self.strings.sign_out_cancel.clone();

        let mut list = div().flex().flex_col();
        for row in &session.accounts {
            let active = row.index == session.active_index;
            // This account's own total, when the core has one for it. A row
            // with no cached figure says nothing rather than $0 — the hero's
            // invariant ② applies to every account, not just the active one.
            let total = switcher
                .balances
                .iter()
                .find(|entry| entry.address.eq_ignore_ascii_case(&row.account.address))
                .map(|entry| {
                    // In the chosen currency, like every other total this
                    // shell prints: `Money` converts only when the endpoint
                    // priced the code, and draws USD when it could not.
                    settings_live::account_total(entry.usd, Some(&currency), &self.locale)
                });
            // The core's own index, not the loop's: it survives a display
            // reorder, which is exactly what invariant ⑦ is about.
            let index = row.index;
            let address = row.account.address.clone();
            let display = crate::wallet::live::shorten_address(&address);

            // The artwork sits BESIDE the row's button, as on the web: it
            // opens the identicon viewer on this account's address, and
            // answers a different question from the row.
            let artwork = div()
                .id(ElementId::from((art_id, index)))
                .flex_none()
                .rounded_full()
                .cursor_pointer()
                .active(|el| el.opacity(0.85))
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &address,
                    SWITCHER_IDENTICON,
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.open_identicon_viewer(address.clone(), cx);
                }));

            let name_colour = if active { theme.accent } else { theme.fg_base };
            let mut pick = div()
                .id(ElementId::from((row_id, index)))
                .flex_1()
                .min_w(px(0.))
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(name_colour)
                                .truncate()
                                .child(gpui::SharedString::from(row.account.name.clone())),
                        )
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(gpui::SharedString::from(display)),
                        ),
                )
                .children(total.map(|total| {
                    div()
                        .flex_none()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(total)
                }));
            if active {
                pick = pick.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.accent,
                    18.,
                ));
            }
            // In the dialog every row answers, and every answer closes it —
            // the one you are on included, which is how the web's does it. On
            // the settings page only the OTHER rows are a switch: clicking the
            // one you are already on should do nothing, not re-run a switch.
            if in_dialog || !active {
                pick = pick
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        if !active {
                            session::switch_account(index, cx);
                        }
                        if in_dialog {
                            this.close_account_switcher(cx);
                        }
                        cx.notify();
                    }));
            }

            // Taking ONE wallet off this device (2026-09-23). Its own click
            // target, so it cannot be hit by somebody aiming at the row.
            let remove = div()
                .id(ElementId::from((remove_id, index)))
                .flex_none()
                .p(px(2.))
                .rounded(px(6.))
                .cursor_pointer()
                .hover(|el| el.bg(theme.bg_sunken))
                .child(icon_img(
                    &mut self.icons,
                    Icon::X,
                    false,
                    theme.fg_subtle,
                    18.,
                ))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.removing_account = Some(index);
                    cx.notify();
                }));

            list = list.child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .border_b_1()
                    .border_color(theme.divider)
                    .child(artwork)
                    .child(pick)
                    .child(remove),
            );

            // Asked before it happens, under the row it is about: the
            // affordance sits in a list whose whole purpose is switching, one
            // press from a row somebody meant to land on.
            if asking == Some(index) {
                list = list.child(
                    div()
                        .pt(px(8.))
                        .pb(px(12.))
                        .flex()
                        .flex_col()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_muted)
                                .child(remove_body.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .gap(px(4.))
                                .child(
                                    crate::flows::components::danger_button(
                                        theme,
                                        remove_label.clone(),
                                    )
                                    .id(ElementId::from((confirm_id, index)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        move |this, _, _, cx| {
                                            this.removing_account = None;
                                            session::remove_account(index, cx);
                                            // The list under the switcher just
                                            // changed: leaving it open would put
                                            // the next row where the pointer is.
                                            if in_dialog {
                                                this.close_account_switcher(cx);
                                            }
                                            cx.notify();
                                        },
                                    )),
                                )
                                .child(
                                    crate::flows::components::ghost_button(
                                        theme,
                                        cancel_label.clone(),
                                    )
                                    .rounded(px(12.))
                                    .id(ElementId::from((cancel_id, index)))
                                    .cursor_pointer()
                                    .on_click(cx.listener(
                                        |this, _, _, cx| {
                                            this.removing_account = None;
                                            cx.notify();
                                        },
                                    )),
                                ),
                        ),
                );
            }
        }

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(px(12.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(summary),
            )
            .child(list)
            // Onboarding over this wallet (spec 072): the root shows it while
            // the session says an account is being added, and a new account
            // established — or "back" — returns here.
            .child(self.account_buttons(theme, true, cx))
    }

    fn settings_account(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let summary = settings_fixtures::accounts_summary(s);
        let sign_out = s.sign_out_button.clone();
        let sign_out_desc = s.sign_out_desc.clone();
        let erase_title = s.erase_title.clone();
        let erase_subtitle = s.erase_subtitle.clone();
        let erase_confirm = s.erase_confirm.clone();

        // Live since 031. The comment this replaced said "the core exposes no
        // account list yet" — `SessionView.accounts` does, and has since 019.
        // So a person with three wallets saw their own on the first row and two
        // strangers' under it.
        let session = session::view(cx);
        if !session.accounts.is_empty() {
            return self.settings_account_live(theme, &session, cx);
        }

        let mut list = div().flex().flex_col();
        for (i, account) in settings_fixtures::ACCOUNTS.iter().enumerate() {
            // The active account wears the REAL identity when there is one; the
            // other two stay fixtures, because the core exposes no account list
            // yet and inventing one would be the screen lying about how many
            // wallets this person has.
            let (name, display, seed) = if i == 0 {
                let identity = self.identity();
                (
                    identity.name.clone(),
                    identity.display(),
                    identity.address.clone(),
                )
            } else {
                (
                    gpui::SharedString::from(account.name),
                    gpui::SharedString::from(account.address_display),
                    account.address_full.to_owned(),
                )
            };
            let active = i == 0;
            let mut row = div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .child(crate::wallet::components::identicon_avatar(
                    &mut self.identicons,
                    &seed,
                    40.,
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(if active { theme.accent } else { theme.fg_base })
                                .child(name),
                        )
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_subtle)
                                .child(display),
                        ),
                )
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(account.amount),
                );
            if active {
                row = row.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.accent,
                    18.,
                ));
            }
            list = list.child(row).child(div().h(px(1.)).bg(theme.divider));
        }

        div()
            .flex()
            .flex_col()
            .child(
                div()
                    .pb(px(12.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(summary),
            )
            .child(list)
            .child(self.account_buttons(theme, false, cx))
            .child(div().h(px(1.)).bg(theme.divider).my(px(32.)))
            .child(
                div()
                    .id("settings-sign-out")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| session::sign_out(cx)))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::LogOut,
                        false,
                        theme.fg_base,
                        18.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(sign_out),
                    ),
            )
            .child(
                div()
                    .pt(px(8.))
                    .pb(px(24.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(sign_out_desc),
            )
            // No handler: this is the board the window draws before anybody has
            // signed in, and there is nothing on it to erase. The LIVE account
            // panel's card (`settings_account_footer`) is the one that acts.
            .child(danger_card(
                theme,
                erase_title,
                erase_subtitle,
                erase_confirm,
                None,
            ))
    }

    /// Ask the chain where the active wallet's founding record stands, once
    /// per account this page meets (spec 062). Blocking reads, so off the
    /// frame; a stale answer for a previous account is dropped.
    fn ensure_backup_check(&mut self, cx: &mut Context<Self>) {
        let Some(account) = money::active_account() else {
            return;
        };
        if self.backup_for.as_deref() == Some(account.address.as_str()) {
            return;
        }
        // `VELA_SIGN_PROBE=1` (debug builds): raise the wallet's own signing column
        // once, with its "Sign with" list open, so the column can be LOOKED at —
        // there is no way to click this app from a shell. A zero-value call to
        // itself on Gnosis; nothing is signed unless somebody slides.
        #[cfg(all(debug_assertions, not(target_os = "linux")))]
        if std::env::var("VELA_SIGN_PROBE").as_deref() == Ok("1") {
            self.open_backup_signing(
                vela_core::registry_backup::BackupCall {
                    chain_id: 100,
                    to: account.address.clone(),
                    value: "0".to_owned(),
                    data: "0x".to_owned(),
                },
                cx,
            );
            if let Some(host) = self.signing_host.clone() {
                host.update(cx, |host, cx| host.sign_with(None, cx));
            }
        }
        self.backup_for = Some(account.address.clone());
        self.backup_check = None;
        self.keys_check = None;
        self.copied = None;
        // The record's key list in founding order; a legacy record is one key.
        let device: Vec<vela_core::wallet_keys::DeviceKey> = if account.keys.is_empty() {
            vec![vela_core::wallet_keys::DeviceKey {
                credential_id: account.id.clone(),
                public_key_hex: account.public_key_hex.clone(),
                name: account.name.clone(),
                transports: String::new(),
                signer_origin: None,
            }]
        } else {
            account
                .keys
                .iter()
                .map(|key| vela_core::wallet_keys::DeviceKey {
                    credential_id: key.credential_id.clone(),
                    public_key_hex: key.public_key_hex.clone(),
                    name: key.name.clone(),
                    transports: key.transports.clone(),
                    signer_origin: key.signer_origin.clone(),
                })
                .collect()
        };
        let keys_address = account.address.clone();
        cx.spawn(async move |page, cx| {
            let asked = keys_address.clone();
            let answer = cx
                .background_executor()
                .spawn(
                    async move { crate::executor::registry::wallet_keys(&keys_address, &device) },
                )
                .await;
            page.update(cx, |page, cx| {
                if page.backup_for.as_deref() == Some(asked.as_str()) {
                    page.keys_check = Some(answer);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
        let address = account.address.clone();
        let key = account.keys.first().map_or_else(
            || account.public_key_hex.clone(),
            |key| key.public_key_hex.clone(),
        );
        cx.spawn(async move |page, cx| {
            let asked = address.clone();
            let answer = cx
                .background_executor()
                .spawn(async move {
                    crate::executor::registry::ethereum_backup_check(
                        &address,
                        &key,
                        vela_core::registry_backup::TARGET_CHAIN,
                    )
                })
                .await;
            page.update(cx, |page, cx| {
                if page.backup_for.as_deref() == Some(asked.as_str()) {
                    page.backup_check = Some(answer);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// The Ethereum backup row (spec 062): one line, three states, a button
    /// only while there is something to do. Nothing at all when the registry is
    /// not on Ethereum or the wallet has no record there to copy.
    fn backup_row(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        use vela_core::registry_backup::BackupState;
        let s = &self.settings;
        let (subtitle, colour, call) = match &self.backup_check {
            None => (s.backup_checking.clone(), theme.fg_subtle, None),
            Some((BackupState::BackedUp, _)) => (s.backup_backed_up.clone(), theme.success, None),
            Some((BackupState::NotBackedUp, call)) => (
                s.backup_not_backed_up.clone(),
                theme.fg_subtle,
                call.clone(),
            ),
            Some((BackupState::CouldNotCheck, _)) => {
                (s.backup_could_not_check.clone(), theme.fg_subtle, None)
            }
            Some((BackupState::Unavailable | BackupState::NotRegistered, _)) => return None,
        };
        let title = s.backup_title.clone();
        let actionable = call.is_some();
        let mut row = div()
            .id("settings-ethereum-backup")
            .flex()
            .items_center()
            // The key rows' own mark column, so the block reads as one list.
            .gap(px(12.))
            .child(
                div()
                    .size(px(theme::KEY_ROW_MARK))
                    .flex_none()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(
                        &mut self.icons,
                        Icon::Upload,
                        false,
                        theme.fg_subtle,
                        18.,
                    )),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(2.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(title),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(colour)
                            .child(subtitle),
                    ),
            );
        if actionable {
            row = row
                .cursor_pointer()
                .on_click(cx.listener(move |page, _, _, cx| {
                    if let Some(call) = call.clone() {
                        page.open_backup_signing(call, cx);
                    }
                }));
        }
        Some(div().child(row.py(px(14.))))
    }

    /// The keys that control this wallet, with their Ethereum backup beneath
    /// them (spec 062) — one block. A person offered "back up your keys" is
    /// owed the sight of them first: what each is called, who is holding it,
    /// whether it is synced. De-containered and hairline-divided like the rest
    /// of the page; only the backup is a button.
    fn keys_block(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        use vela_core::wallet_keys::KeysSource;
        let s = &self.settings;
        let title = s.keys_title.clone();
        let subtitle = s.keys_subtitle.clone();
        let from_device = s.keys_from_device.clone();
        let synced = s.keys_synced.clone();
        let not_synced = s.keys_not_synced.clone();
        let key_n = s.keys_key_n.clone();
        let lines = (
            s.keys_provider_platform.clone(),
            s.keys_provider_generic.clone(),
            s.keys_provider_security_key.clone(),
        );
        // Spec 075: the fourth place a key can live, in the "Sign with"
        // sheet's own words — the caption for a key behind a page, and the
        // label on the line naming which page.
        let trusted_signer = trusted_signer_words(s);
        let user_verified = s.keys_user_verified.clone();
        let labels = (
            s.keys_public_key.clone(),
            s.keys_credential.clone(),
            s.keys_transport.clone(),
            s.keys_attestation.clone(),
        );
        let copy_label = s.keys_copy.clone();
        let copied_label = s.keys_copied.clone();
        let explain = s.backup_explain.clone();
        let check = self.keys_check.clone();

        let mut header = div().flex().items_baseline().gap(px(8.)).child(
            div()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(title),
        );
        if let Some((_, keys)) = &check {
            header = header.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(keys.len().to_string()),
            );
        }
        let mut block = div()
            .flex()
            .flex_col()
            .child(div().h(px(1.)).bg(theme.divider).my(px(32.)))
            .child(header)
            .child(
                div()
                    .pt(px(4.))
                    .pb(px(8.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(subtitle),
            );

        match check {
            // The shape of one row, so the block does not jump when the answer lands.
            None => {
                block = block.child(
                    div()
                        .flex()
                        .items_center()
                        .gap(px(12.))
                        .py(px(14.))
                        .border_b_1()
                        .border_color(theme.divider)
                        .child(
                            div()
                                .size(px(theme::KEY_ROW_MARK))
                                .rounded_full()
                                .bg(theme.divider),
                        )
                        .child(
                            div()
                                .w(px(160.))
                                .h(px(10.))
                                .rounded_full()
                                .bg(theme.divider),
                        ),
                );
            }
            Some((source, keys)) => {
                for (index, key) in keys.iter().enumerate() {
                    let name = if key.name.is_empty() {
                        key_n.replace("{{n}}", &(index + 1).to_string())
                    } else {
                        key.name.clone()
                    };
                    let holder = key_holder(key, &lines, &trusted_signer);
                    let body = key.public_key_hex.trim_start_matches("04");
                    let fingerprint = (body.len() >= 8)
                        .then(|| format!("{}…{}", &body[..4], &body[body.len() - 4..]));

                    let mut row = div().flex().items_center().gap(px(12.)).py(px(14.));
                    if let Some(mark) = super::components::passkey_mark(
                        &mut self.identicons,
                        &key.aaguid,
                        theme.is_dark(),
                        theme::KEY_ROW_MARK,
                    ) {
                        row = row.child(mark);
                    } else if let Some(mark) = super::components::passkey_fallback_mark(
                        &mut self.identicons,
                        &key.authenticator_attachment,
                        &key.transports,
                        key.method == "security_key",
                        theme,
                        theme::KEY_ROW_MARK,
                    ) {
                        row = row.child(mark);
                    } else {
                        row = row.child(
                            div()
                                .size(px(theme::KEY_ROW_MARK))
                                .flex_none()
                                .flex()
                                .items_center()
                                .justify_center()
                                .child(icon_img(
                                    &mut self.icons,
                                    Icon::Lock,
                                    false,
                                    theme.fg_subtle,
                                    18.,
                                )),
                        );
                    }
                    let mut meta = div()
                        .flex()
                        .items_center()
                        .gap(px(6.))
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_subtle)
                        .child(holder);
                    if let Some(fingerprint) = fingerprint {
                        meta = meta
                            .child("·")
                            .child(div().font_family(theme::font_mono()).child(fingerprint));
                    }
                    row = row.child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .text_color(theme.fg_base)
                                    .child(name),
                            )
                            .child(meta),
                    );
                    // The registry explorer's pills; one nobody can vouch for is not drawn.
                    let mut pills: Vec<(gpui::SharedString, gpui::Hsla)> = Vec::new();
                    if key.user_verified == Some(true) {
                        pills.push((user_verified.clone(), theme.info_base));
                    }
                    if let Some(is_synced) = key.synced {
                        pills.push(if is_synced {
                            (synced.clone(), theme.success)
                        } else {
                            (not_synced.clone(), theme.fg_subtle)
                        });
                    }
                    for (text, colour) in pills {
                        row = row.child(
                            div()
                                .flex_none()
                                .px(px(8.))
                                .py(px(2.))
                                .rounded_full()
                                .border_1()
                                .border_color(colour)
                                .text_size(theme::text_row_sub())
                                .text_color(colour)
                                .child(text),
                        );
                    }

                    // What the row opens onto: the explorer's facts, the two a
                    // person pastes elsewhere copyable. Nothing to open when only
                    // the device answered.
                    let transport = [
                        key.authenticator_attachment.as_str(),
                        key.transports.as_str(),
                    ]
                    .into_iter()
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
                    .join(" · ");
                    // Spec 075: WHICH page, first, because where a key lives is
                    // the one fact about it a person cannot look up elsewhere.
                    // Empty for every other key, and an empty value is dropped
                    // below — so no ordinary row grows a line.
                    let signer_page = key_page(key);
                    let details: Vec<(gpui::SharedString, String, bool, bool)> = [
                        (trusted_signer.clone(), signer_page.clone(), false, true),
                        (
                            labels.0.clone(),
                            if key.public_key_hex.is_empty() {
                                String::new()
                            } else {
                                format!("0x{}", key.public_key_hex)
                            },
                            true,
                            true,
                        ),
                        (labels.1.clone(), key.credential_id.clone(), true, true),
                        (
                            gpui::SharedString::from("AAGUID"),
                            key.aaguid.clone(),
                            true,
                            false,
                        ),
                        (labels.2.clone(), transport, false, false),
                        (labels.3.clone(), key.attestation_hex.clone(), true, false),
                    ]
                    .into_iter()
                    .filter(|(_, value, _, _)| !value.is_empty())
                    .collect();
                    // A row with no credential id came from the device alone and
                    // has nothing worth opening — unless it lives behind a
                    // page, which is exactly the row whose whereabouts a person
                    // wants to check when the registry is unreachable.
                    let expandable = !key.credential_id.is_empty() || !signer_page.is_empty();
                    let is_open = self.keys_open.contains(&index);
                    let mut stateful = row.id(("settings-key-row", index));
                    if expandable {
                        stateful = stateful
                            .child(icon_img(
                                &mut self.icons,
                                if is_open {
                                    Icon::ChevronUp
                                } else {
                                    Icon::ChevronDown
                                },
                                false,
                                theme.fg_subtle,
                                14.,
                            ))
                            .cursor_pointer()
                            .on_click(cx.listener(move |page, _, _, cx| {
                                if !page.keys_open.remove(&index) {
                                    page.keys_open.insert(index);
                                }
                                cx.notify();
                            }));
                    }
                    let mut entry = div()
                        .flex()
                        .flex_col()
                        .border_b_1()
                        .border_color(theme.divider)
                        .child(stateful);
                    if expandable && is_open {
                        let mut card = div()
                            .flex()
                            .flex_col()
                            .gap(px(12.))
                            .p(px(16.))
                            .mb(px(12.))
                            .rounded(px(12.))
                            .border_1()
                            .border_color(theme.divider)
                            .bg(theme.bg_sunken);
                        for (slot, (label, value, mono, copyable)) in
                            details.into_iter().enumerate()
                        {
                            let mut shown = div()
                                .flex_1()
                                .min_w(px(0.))
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_base)
                                .child(gpui::SharedString::from(value.clone()));
                            if mono {
                                shown = shown.font_family(theme::font_mono());
                            }
                            let mut line = div()
                                .flex()
                                .items_start()
                                .gap(px(12.))
                                .child(
                                    div()
                                        .w(px(110.))
                                        .flex_none()
                                        .text_size(theme::text_row_sub())
                                        .text_color(theme.fg_subtle)
                                        .child(label),
                                )
                                .child(shown);
                            if copyable {
                                let copy_id = format!("{index}:{slot}");
                                let copy_id = format!("key:{copy_id}");
                                let done = self.copied.as_deref() == Some(copy_id.as_str());
                                line = line.child(
                                    div()
                                        .id(("settings-key-copy", index * 8 + slot))
                                        .flex_none()
                                        .px(px(8.))
                                        .py(px(2.))
                                        .rounded(px(6.))
                                        .border_1()
                                        .border_color(theme.divider)
                                        .text_size(theme::text_row_sub())
                                        .text_color(theme.fg_subtle)
                                        .cursor_pointer()
                                        .child(if done {
                                            copied_label.clone()
                                        } else {
                                            copy_label.clone()
                                        })
                                        .on_click(cx.listener(move |page, _, _, cx| {
                                            // `KeysBlock`: "Copied" for 1.2 s, then
                                            // the button is a button again.
                                            page.copy_text(
                                                copy_id.clone(),
                                                value.clone(),
                                                std::time::Duration::from_millis(1200),
                                                cx,
                                            );
                                        })),
                                );
                            }
                            card = card.child(line);
                        }
                        entry = entry.child(card);
                    }
                    block = block.child(entry);
                }
                if source == KeysSource::Device {
                    block = block.child(
                        div()
                            .py(px(8.))
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(from_device),
                    );
                }
            }
        }
        match self.backup_row(theme, cx) {
            // PUBLIC keys: "back up keys" read as handing over the keys themselves.
            Some(backup) => block.child(backup).child(
                div()
                    .pl(px(theme::KEY_ROW_MARK + 12.))
                    .pb(px(8.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(explain),
            ),
            None => block,
        }
    }

    /// The backup is one transaction the wallet asks ITSELF to sign: the same
    /// signing column a dApp request opens, so the estimate, the fee, the
    /// funding guidance, the passkey and the receipt are the existing ones.
    #[cfg(not(target_os = "linux"))]
    fn open_backup_signing(
        &mut self,
        call: vela_core::registry_backup::BackupCall,
        cx: &mut Context<Self>,
    ) {
        let Some(account) = money::active_account() else {
            return;
        };
        let request = backup_request(&account.address, &call);
        // After it closes, ask again: the row should say what is true now.
        self.backup_for = None;
        self.open_signing_request(&account, request, None, cx);
    }

    #[cfg(target_os = "linux")]
    fn open_backup_signing(
        &mut self,
        _call: vela_core::registry_backup::BackupCall,
        _cx: &mut Context<Self>,
    ) {
    }

    /// "Create a new account" / "Sign in to an existing account". Live, they
    /// open onboarding over this wallet; on the design surfaces they are the
    /// mock's picture.
    /// Create, or sign in to one you already have — the web's `Button`
    /// pair, side by side at their labels' width (`layout="inline"`,
    /// padding-inline 32), not the 40px rows the settings panel drew.
    fn account_buttons(&mut self, theme: &Theme, live: bool, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        // At their labels' width, but allowed to give way: two English
        // labels at padding 32 already fill the 472px a dialog leaves, and
        // a longer locale wraps inside its button rather than running out of
        // the card.
        let create = crate::flows::components::accent_button(theme, s.account_create.clone())
            .w_auto()
            .flex_initial()
            .min_w(px(0.))
            .px(px(32.))
            .id("settings-create-account")
            .cursor_pointer();
        let sign_in = crate::flows::components::ghost_button(theme, s.account_sign_in.clone())
            .rounded(px(12.))
            .w_auto()
            .flex_initial()
            .min_w(px(0.))
            .px(px(32.))
            .id("settings-sign-in-account")
            .cursor_pointer();
        let (create, sign_in) = if live {
            (
                create.on_click(cx.listener(|this, _, _, cx| {
                    this.close_account_switcher(cx);
                    this.close_switcher(cx);
                    session::add_account(session::AddAccount::Create, cx);
                })),
                sign_in.on_click(cx.listener(|this, _, _, cx| {
                    this.close_account_switcher(cx);
                    this.close_switcher(cx);
                    session::add_account(session::AddAccount::SignIn, cx);
                })),
            )
        } else {
            (create, sign_in)
        };
        div()
            .flex()
            .gap(px(12.))
            .pt(px(24.))
            .child(create)
            .child(sign_in)
    }

    /// Sign out and erase, shared by the live and mock account panels.
    #[allow(clippy::too_many_arguments, reason = "one footer, two call sites")]
    fn settings_account_footer(
        &mut self,
        theme: &Theme,
        sign_out: gpui::SharedString,
        sign_out_desc: gpui::SharedString,
        erase_title: gpui::SharedString,
        erase_subtitle: gpui::SharedString,
        erase_confirm: gpui::SharedString,
        cx: &mut Context<Self>,
    ) -> Div {
        let keys = self.keys_block(theme, cx);
        div()
            .flex()
            .flex_col()
            .child(keys)
            .child(div().h(px(1.)).bg(theme.divider).my(px(32.)))
            .child(
                div()
                    .id("settings-sign-out-live")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _, _, cx| session::sign_out(cx)))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::LogOut,
                        false,
                        theme.fg_base,
                        18.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(sign_out),
                    ),
            )
            .child(
                div()
                    .pt(px(8.))
                    .pb(px(24.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(sign_out_desc),
            )
            // The one irreversible control asks first (spec 072 FR-010, and
            // spec 081 FR-017 — it had no handler at all from the day it was
            // drawn). It asks; `SettingsDialog::EraseDevice` confirms;
            // `erase_device` acts.
            .child(danger_card(
                theme,
                erase_title,
                erase_subtitle,
                erase_confirm,
                Some(Box::new(cx.listener(|this, _, _, cx| {
                    this.erase_failed = None;
                    this.settings_dialog = Some(SettingsDialog::EraseDevice);
                    cx.notify();
                }))),
            ))
    }

    /// DST2 — language, text size, theme. (The avatar style is retired, spec
    /// 074: every avatar is the identicon.)
    ///
    /// Live since 072: each control stores its choice under the key every Vela
    /// shares and puts it in force on the next frame.
    fn settings_appearance(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let language = s.language.clone();
        let scale_label = s.text_scale.clone();
        let theme_label = s.theme_title.clone();
        let themes = [
            (Some(Icon::Sun), s.theme_light.clone()),
            (Some(Icon::Moon), s.theme_dark.clone()),
            (Some(Icon::Monitor), s.theme_auto.clone()),
        ];

        // Both the size stops and the theme segments were drawn and not wired
        // before this (`segmented` and `text_scale` took no handler at all —
        // "spec 023 is UI only", in the component's own doc), so the thumb sat
        // on a stop that meant nothing. They are live below, on the web's own
        // store keys and words, so one person's one choice reads the same on
        // both clients.
        //
        // The avatar row main wired here is NOT revived: spec 074 retired the
        // setting — every avatar is the identicon — and a control for a
        // preference the app no longer has is the same lie as a control that
        // does nothing.
        if self.identity.is_none() {
            let language_value = gpui::SharedString::from(format!("简体中文 · {}", s.note_system));
            // Which theme cell reads as chosen follows the appearance the
            // window is actually in — a settings screen that says "Light"
            // while drawing dark is the one thing this row must never do.
            let theme_index = match self.theme_mode() {
                ThemeMode::Light => 0,
                ThemeMode::Dark => 1,
            };
            let language_control = dropdown_trigger(theme, &mut self.icons, language_value);
            let scale_control = text_scale(theme, 7, 3);
            let theme_control = segmented(theme, &mut self.icons, &themes, theme_index);
            return div()
                .flex()
                .flex_col()
                .child(form_row(theme, language, language_control))
                .child(form_row(theme, scale_label, scale_control))
                .child(form_row(theme, theme_label, theme_control));
        }

        let prefs = crate::executor::preferences::current();
        let pinned = crate::executor::preferences::pinned_language();
        // What "follow the system" follows, by the locale it resolves to.
        let system_language = vela_core::i18n::resolve_language(&crate::loc::system_tag()).language;
        let page = cx.entity();

        let language_value = settings_live::language_value(pinned.as_deref(), &system_language, s);
        let menu = (self.settings_open_dropdown == Some("language")).then(|| {
            let rows = settings_live::language_menu(pinned.as_deref(), &system_language, s);
            let page = page.clone();
            dropdown_menu_choices(
                "language-menu",
                theme,
                &mut self.icons,
                &rows,
                move |index, _, cx| {
                    page.update(cx, |this, cx| {
                        if let Some(word) = settings_live::picked_language(index) {
                            crate::executor::preferences::set_language(word);
                            this.relocalize();
                        }
                        this.settings_open_dropdown = None;
                        cx.notify();
                    });
                },
            )
        });
        let trigger = dropdown_trigger(theme, &mut self.icons, language_value);
        let language_control = div()
            .id("settings-dropdown-language")
            .relative()
            .w_full()
            .cursor_pointer()
            .on_click(cx.listener(|this, _, _, cx| {
                this.settings_open_dropdown = if this.settings_open_dropdown == Some("language") {
                    None
                } else {
                    Some("language")
                };
                cx.notify();
            }))
            .child(trigger)
            .when_some(menu, |el, menu| el.child(deferred(menu).with_priority(1)));

        let scale_control = text_scale_picks(
            theme,
            vela_core::prefs::TEXT_SCALE_LEVELS.len(),
            settings_live::text_scale_index(prefs.text_scale),
            {
                let page = page.clone();
                move |index, window, cx| {
                    if let Some((level, _)) = vela_core::prefs::TEXT_SCALE_LEVELS.get(index) {
                        crate::executor::preferences::set_text_scale(level);
                    }
                    // Every text on screen changes size, not only this page's.
                    window.refresh();
                    page.update(cx, |_, cx| cx.notify());
                }
            },
        );
        let theme_control = segmented_picks(
            "settings-theme",
            theme,
            &mut self.icons,
            &themes,
            settings_live::segment_of(&settings_live::THEME_SEGMENTS, prefs.theme),
            move |index, window, cx| {
                crate::executor::preferences::set_theme(settings_live::THEME_SEGMENTS[index]);
                // "Follow System" reads the window's appearance again.
                let mode = ThemeMode::detect(window);
                page.update(cx, |this, cx| {
                    this.mode = mode;
                    cx.notify();
                });
            },
        );

        div()
            .flex()
            .flex_col()
            .child(form_row(theme, language, language_control))
            .child(form_row(theme, scale_label, scale_control))
            .child(form_row(theme, theme_label, theme_control))
    }

    /// What the 货币 row shows — the core's committed currency for a real
    /// session, the mock's literal for the design surfaces.
    ///
    /// Gated on `identity`: `VELA_PAGE=settings` and the gallery are design
    /// surfaces with no session behind them, and a fixture screen quietly
    /// reading live state is how a gallery stops being reviewable.
    /// The currency this screen's money is drawn in.
    ///
    /// Signed out there is no committed pair and nothing to convert, so it is
    /// USD — the same thing every fixture board shows.
    fn money(&self, cx: &mut Context<Self>) -> wallet_live::Money {
        if self.identity.is_none() {
            return wallet_live::Money::default();
        }
        let view = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        wallet_live::Money::new(&view.code, view.rate)
    }

    fn currency_value(&self, cx: &mut Context<Self>) -> gpui::SharedString {
        if self.identity.is_none() {
            return gpui::SharedString::from("USD · $1,234.56");
        }
        let view = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        settings_live::currency_row_value(&view, &self.locale)
    }

    /// Ask the rate endpoint which currencies it can price, once per session.
    ///
    /// Off the UI thread: it is an HTTP call, and the menu is already on screen
    /// when it starts. Until it answers the menu holds USD alone — honest,
    /// because USD is the only code the wallet can price without asking anybody.
    fn load_currency_catalog(&mut self, cx: &mut Context<Self>) {
        if self.identity.is_none() || display_currency::priced_currencies().len() > 1 {
            return;
        }
        cx.spawn(async move |page, cx| {
            cx.background_executor()
                .spawn(async move { display_currency::refresh_priced_currencies() })
                .await;
            page.update(cx, |_, cx| cx.notify()).ok();
        })
        .detach();
    }

    /// DST3 — currency, number, date and time formats.
    ///
    /// One of the four can be OPEN, and the mock's is 数字格式. The menu is an
    /// absolutely-positioned child of that row's control cell so it lies over
    /// the rows beneath instead of pushing them down — the desktop SPEC's
    /// "浮层需逃出容器裁剪" rule.
    ///
    /// A session (spec 038 #E3) reads the presets in force and lets the
    /// number, date and time rows choose; the design surfaces keep the mock's
    /// literals and its one drawn menu, gated on `identity` like every other
    /// live surface here.
    fn settings_localization(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let auto_note =
            gpui::SharedString::from(format!("{} · {}", s.note_automatic, s.note_system));
        let live = if self.identity.is_some() {
            let formats = format_prefs::current();
            let (number, date, time) = settings_live::format_row_values(formats, &self.locale);
            let menus = settings_live::format_menus(
                format_prefs::choice(),
                &format_prefs::machine(),
                &self.locale,
                &auto_note,
                &s.note_indian,
            );
            Some((number, date, time, menus))
        } else {
            None
        };
        let (number_value, date_value, time_value) = match live.as_ref() {
            Some((number, date, time, _)) => (number.clone(), date.clone(), time.clone()),
            None => ("1,234,567.89".into(), "2026/06/13".into(), "13:45".into()),
        };
        let rows: [(&'static str, gpui::SharedString, gpui::SharedString); 4] = [
            ("currency", s.currency.clone(), self.currency_value(cx)),
            ("number", s.number_format.clone(), number_value),
            ("date", s.date_format.clone(), date_value),
            ("time", s.time_format.clone(), time_value),
        ];
        let number_menu: [(gpui::SharedString, Option<gpui::SharedString>, bool); 5] = [
            ("1,234,567.89".into(), Some(auto_note), true),
            ("1,234,567.89".into(), None, false),
            ("1.234.567,89".into(), None, false),
            ("1 234 567,89".into(), None, false),
            ("12,34,567.89".into(), Some(s.note_indian.clone()), false),
        ];

        let open = self.settings_open_dropdown;
        let page = cx.entity();
        let mut col = div().flex().flex_col();
        for (id, label, value) in rows {
            let is_open = open == Some(id);
            let trigger = dropdown_trigger(theme, &mut self.icons, value);
            let menu = if !is_open {
                None
            } else if let Some((_, _, _, menus)) = live.as_ref() {
                if id == "currency" {
                    // The currency row's menu is not one of the three format
                    // menus: its options come from what the rate endpoint can
                    // price, and picking one goes to a different machine.
                    let priced = display_currency::priced_currencies();
                    let view = resident::resident::<DisplayCurrency>(cx).read(cx).view();
                    let rows = settings_live::currency_menu(&priced, &view.code, &self.locale);
                    let codes: Vec<String> = priced.into_iter().map(|(code, _)| code).collect();
                    let page = page.clone();
                    Some(dropdown_menu_picks(
                        theme,
                        &mut self.icons,
                        &rows,
                        move |index, _, cx| {
                            let Some(code) = codes.get(index).cloned() else {
                                return;
                            };
                            resident::resident::<DisplayCurrency>(cx).update(cx, |resident, cx| {
                                resident.dispatch(CurrencyEvent::UserChose { code }, cx);
                            });
                            page.update(cx, |this, cx| {
                                this.settings_open_dropdown = None;
                                cx.notify();
                            });
                        },
                    ))
                } else {
                    let rows = match id {
                        "number" => Some(&menus.number),
                        "date" => Some(&menus.date),
                        "time" => Some(&menus.time),
                        _ => None,
                    };
                    rows.map(|rows| {
                        let page = page.clone();
                        dropdown_menu_picks(theme, &mut self.icons, rows, move |index, _, cx| {
                            page.update(cx, |this, cx| {
                                this.pick_format(id, index);
                                this.settings_open_dropdown = None;
                                cx.notify();
                            });
                        })
                    })
                }
            } else {
                (id == "number").then(|| dropdown_menu(theme, &mut self.icons, &number_menu))
            };
            let control = div()
                .id(SharedString::from(format!("settings-dropdown-{id}")))
                .relative()
                .w_full()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.settings_open_dropdown = if this.settings_open_dropdown == Some(id) {
                        None
                    } else {
                        Some(id)
                    };
                    if this.settings_open_dropdown == Some("currency") {
                        this.load_currency_catalog(cx);
                    }
                    cx.notify();
                }))
                .child(trigger)
                // `deferred`, the same escape the contacts menus take: gpui
                // paints in child order, so an open menu drawn inside row 2 was
                // painted over by rows 3 and 4 — the date and time triggers sat
                // on top of it and swallowed one of its options.
                .when_some(menu, |el, menu| el.child(deferred(menu).with_priority(1)));
            col = col.child(form_row(theme, label, control));
        }
        col
    }
    fn pick_format(&mut self, id: &'static str, index: usize) {
        match id {
            "number" => format_prefs::set_number(settings_live::picked_number(index)),
            "date" => format_prefs::set_date(settings_live::picked_date(index)),
            "time" => format_prefs::set_time(settings_live::picked_time(index)),
            _ => {}
        }
    }

    /// DST4 — the network list, expanding one row in place.
    /// The 网络 rows: the core's for a real session, the mocks' otherwise.
    ///
    /// Gated on `identity` like every other live surface here — `VELA_PAGE=
    /// settings` and the gallery have no session behind them, and a design
    /// surface quietly reading live state stops being reviewable.
    fn network_rows(&mut self, cx: &mut Context<Self>) -> Vec<NetworkRowModel> {
        if self.identity.is_none() {
            return settings_fixtures::network_rows();
        }
        let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
        if !view.loaded {
            // The core has not ruled yet. An EMPTY list, not a fixture one:
            // a person's own settings screen must never show somebody else's
            // networks while it waits (spec 030 FR-008).
            return Vec::new();
        }
        settings_live::network_rows(&view)
    }

    fn settings_networks(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        let page = cx.entity();
        let expanded = self.settings_expanded_network.clone();
        let rows = self.network_rows(cx);
        let mut col = div().flex().flex_col();
        for (i, n) in rows.into_iter().enumerate() {
            let meta = settings_fixtures::chain_meta(&self.settings, n.chain_id);
            let badge = n.latency_ms.map(|ms| latency(ms, None));
            let tag = n.custom.then(|| self.settings.network_custom.clone());
            let is_expanded = expanded.as_ref() == Some(&n.id);
            let row = network_row(
                ElementId::from(("settings-network", i)),
                theme,
                &mut self.icons,
                n.chain_id,
                n.letter.clone(),
                n.color,
                n.name.clone(),
                meta,
                badge.as_ref(),
                tag,
                n.custom,
                is_expanded,
                // A custom network can be removed; a built-in one has no trash
                // to press. The page asks before it happens — the core's own
                // note says the confirm dialog is the shell's, and this shell
                // already has that dialog four times over.
                (n.custom && self.identity.is_some()).then(|| {
                    let page = page.clone();
                    let id = n.id.clone();
                    let name = n.name.clone();
                    Box::new(
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            page.update(cx, |this, cx| {
                                this.network_remove = Some((id.to_string(), name.clone()));
                                cx.notify();
                            });
                        },
                    ) as crate::contacts::components::MenuAction
                }),
            );
            let id = n.id.clone();
            col = col
                .child(row.on_click(cx.listener(move |this, _, _, cx| {
                    this.settings_expanded_network =
                        if this.settings_expanded_network.as_ref() == Some(&id) {
                            None
                        } else {
                            Some(id.clone())
                        };
                    cx.notify();
                })))
                .child(div().h(px(1.)).bg(theme.divider));
            if is_expanded {
                // The row model carries a u64 for the tint table; the core
                // speaks u32. A chain id past 2^32 is not one.
                let chain_id = u32::try_from(n.chain_id).unwrap_or(0);
                col = col.child(self.settings_network_detail(theme, chain_id, i, window, cx));
            }
        }
        col
    }

    /// The two editable overrides under an expanded network card.
    ///
    /// **The RPC field is the only place in this app where a save can be
    /// REFUSED.** The core probes what the person typed and, if it answers
    /// `eth_chainId` with another chain's id, writes nothing — because a
    /// "Gnosis" endpoint that actually serves Polygon would send somebody's
    /// money to the wrong chain. 030 proved that refusal against a real
    /// endpoint; this is the field it refuses.
    fn network_override_fields(
        &mut self,
        theme: &Theme,
        chain_id: u32,
        index: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> (Div, Div) {
        // Opening the card is what starts the two probes. Dispatched here
        // rather than from the click, because the card can also open from a
        // restored state where no click happened.
        if self.settings_probed_network != Some(chain_id) {
            self.settings_probed_network = Some(chain_id);
            resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                resident.dispatch(NetEvent::OverrideExpanded { chain_id }, cx);
            });
        }

        let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
        let row = view
            .networks
            .iter()
            .find(|row| row.chain_id == chain_id)
            .cloned();
        let Some(row) = row else {
            return (div(), div());
        };

        let rpc_badge = row.rpc_health.as_ref().and_then(settings_live::probe_badge);
        // The explorer is probed too, and the field has the same badge slot
        // the RPC one uses — it was being passed `None`, so a measured
        // explorer looked exactly like an unprobed one.
        let explorer_badge = row
            .explorer_health
            .as_ref()
            .and_then(settings_live::probe_badge);
        // The refusal, in words, over the hint. A person who just watched
        // nothing happen needs to be told why, and "saved" would be a lie —
        // which it also is for the seconds the verdict is still outstanding.
        let hint = settings_live::override_hint(&row, &self.settings);
        let refused = row.rpc_chain_mismatch.is_some();
        let rpc_focus = self.endpoint_focus(OVERRIDE_FOCUS_BASE + index * 2, cx);
        let explorer_focus = self.endpoint_focus(OVERRIDE_FOCUS_BASE + index * 2 + 1, cx);

        // A keystroke is a draft the core re-probes; leaving the field (or
        // Enter) is the save, behind its chain-id gate. Until 072 both went on
        // every keystroke, so the wrong-chain refusal ran against half a URL.
        let page = cx.entity();
        let rpc_slot = OVERRIDE_FOCUS_BASE + index * 2;
        let edit = |slot: usize, field: NetOverrideField| {
            let page = page.clone();
            move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                Self::edit_field(
                    &page,
                    slot,
                    settings_live::FieldCommit::Override { chain_id, field },
                    text,
                    cx,
                );
            }
        };

        (
            editable_url_field(
                ElementId::from(("override-rpc", index)),
                theme,
                Some(self.settings.rpc_url.clone()),
                &row.rpc_url,
                self.settings.rpc_url.clone(),
                rpc_badge.as_ref(),
                Some(hint),
                refused.then_some(Tone::Error),
                &rpc_focus,
                window,
                edit(rpc_slot, NetOverrideField::Rpc),
                Self::enter_leaves(&page),
            ),
            editable_url_field(
                ElementId::from(("override-explorer", index)),
                theme,
                Some(self.settings.explorer.clone()),
                &row.explorer_url,
                self.settings.explorer.clone(),
                explorer_badge.as_ref(),
                None,
                None,
                &explorer_focus,
                window,
                edit(rpc_slot + 1, NetOverrideField::Explorer),
                Self::enter_leaves(&page),
            ),
        )
    }

    /// The editor DST4 opens under the expanded row. No identity line: the row
    /// above it already says which chain this is.
    fn settings_network_detail(
        &mut self,
        theme: &Theme,
        chain_id: u32,
        index: usize,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let (rpc, explorer) = if self.identity.is_some() {
            self.network_override_fields(theme, chain_id, index, window, cx)
        } else {
            let s = &self.settings;
            (
                url_field(
                    theme,
                    Some(s.rpc_url.clone()),
                    gpui::SharedString::from(settings_fixtures::ETHEREUM_RPC),
                    Some(&latency(45, None)),
                    Some(s.network_save_hint.clone()),
                    None,
                    None,
                ),
                url_field(
                    theme,
                    Some(s.explorer.clone()),
                    gpui::SharedString::from(settings_fixtures::ETHEREUM_EXPLORER),
                    None,
                    None,
                    None,
                    None,
                ),
            )
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .my(px(12.))
            .p(px(16.))
            .rounded(px(10.))
            .bg(theme.bg_sunken)
            .border_1()
            .border_color(theme.divider)
            .child(rpc)
            .child(explorer)
    }

    /// DST5 — one card per RPC provider.
    fn settings_providers(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut col = div().flex().flex_col().gap(px(32.)).child(
            div()
                .pb(px(8.))
                .text_size(theme::text_row_sub())
                .line_height(px(20.))
                .text_color(theme.fg_muted)
                .child(self.settings.providers_desc.clone()),
        );

        // Live since 031. An API key is a CREDENTIAL, and the field it goes in
        // was read-only until now — so a person with a paid Alchemy plan had no
        // way to use it.
        if self.identity.is_some() {
            // Same gesture, same event: opening the panel tests the keys the
            // person already has, so the pills are about now and not about the
            // last time somebody typed.
            self.settings_opened(SettingsProbe::Providers, cx);
            let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
            let page = cx.entity();
            for (i, provider) in view.providers.iter().enumerate() {
                let badge = if provider.has_key {
                    pill(Tone::Ok, self.settings.provider_connected.clone())
                } else {
                    pill(Tone::Neutral, self.settings.provider_not_set.clone())
                };
                let id = provider.provider;
                // Index past the endpoint fields so the two panels' handles
                // cannot collide.
                let focus = self.endpoint_focus(ENDPOINT_FOCUS_COUNT + i, cx);
                col = col.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(12.))
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(8.))
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w(px(0.))
                                        .text_size(theme::text_panel_title())
                                        .font_weight(gpui::FontWeight::BOLD)
                                        .text_color(theme.fg_base)
                                        .child(settings_live::provider_name(id)),
                                )
                                .child(status_pill(theme, &badge))
                                // No key yet: where to get one, as the web
                                // offers — the page opens in the browser.
                                .when(!provider.has_key, |el| {
                                    let url = settings_live::provider_key_url(id);
                                    el.child(
                                        div()
                                            .id(ElementId::from(("provider-get-key", i)))
                                            .cursor_pointer()
                                            .text_size(theme::text_row_sub())
                                            .text_color(theme.info_base)
                                            .on_click(move |_, _, cx| cx.open_url(url))
                                            .child(SharedString::from(format!(
                                                "{} →",
                                                self.settings.provider_get_key
                                            ))),
                                    )
                                })
                                // The explicit re-run, on the row that carries
                                // the verdict it rewrites. A key blur already
                                // tests; this is for the person who changed
                                // nothing and wants to know whether it works
                                // NOW — the only question this page is ever
                                // opened to answer.
                                .child(
                                    div()
                                        .id(ElementId::from(("provider-test", i)))
                                        .px(px(12.))
                                        .py(px(6.))
                                        .rounded(px(8.))
                                        .cursor_pointer()
                                        .border_1()
                                        .border_color(theme.divider)
                                        .hover(|el| el.border_color(theme.outline_strong))
                                        .text_size(theme::text_row_sub())
                                        .text_color(theme.fg_base)
                                        .child(self.settings.provider_test.clone())
                                        .on_click(cx.listener(move |_, _, _, cx| {
                                            resident::resident::<NetworkAdmin>(cx).update(
                                                cx,
                                                |resident, cx| {
                                                    resident.dispatch(
                                                        NetEvent::ProviderTestRequested {
                                                            provider: id,
                                                        },
                                                        cx,
                                                    );
                                                },
                                            );
                                            cx.notify();
                                        })),
                                ),
                        )
                        .child(editable_url_field(
                            ElementId::from(("provider", i)),
                            theme,
                            None,
                            &provider.key,
                            self.settings.provider_not_set.clone(),
                            None,
                            settings_live::provider_support(provider, &self.settings),
                            None,
                            &focus,
                            window,
                            {
                                // A keystroke is a draft; leaving the field is
                                // what persists it — and what DROPS a provider
                                // whose key was cleared (invariant ⑦).
                                let page = page.clone();
                                move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                                    Self::edit_field(
                                        &page,
                                        ENDPOINT_FOCUS_COUNT + i,
                                        settings_live::FieldCommit::ProviderKey(id),
                                        text,
                                        cx,
                                    );
                                }
                            },
                            Self::enter_leaves(&page),
                        )),
                );
            }
            return col;
        }

        for p in &settings_fixtures::PROVIDERS {
            let connected = !p.key.is_empty();
            let badge = if connected {
                pill(Tone::Ok, self.settings.provider_connected.clone())
            } else {
                pill(Tone::Neutral, self.settings.provider_not_set.clone())
            };
            let value = if connected {
                gpui::SharedString::from(p.key)
            } else {
                self.settings.provider_not_set.clone()
            };
            let support = settings_fixtures::provider_support(&self.settings, p);
            let mut card = div()
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .child(
                            div()
                                .text_size(theme::text_panel_title())
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(theme.fg_base)
                                .child(p.name),
                        )
                        .child(status_pill(theme, &badge)),
                )
                .child(url_field(
                    theme,
                    None,
                    value,
                    None,
                    None,
                    None,
                    Some(if connected {
                        self.settings.provider_check_key.clone()
                    } else {
                        self.settings.provider_get_key.clone()
                    }),
                ));
            if let Some(support) = support {
                card = card.child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(support),
                );
            }
            col = col.child(card);
        }
        col
    }

    /// DST6 — the four services the wallet leans on.
    fn settings_endpoints(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let copy = settings_fixtures::endpoint_copy(&self.settings);
        let mut col = div().flex().flex_col().gap(px(24.)).child(
            div()
                .text_size(theme::text_row_sub())
                .line_height(px(20.))
                .text_color(theme.fg_muted)
                .child(self.settings.endpoints_desc.clone()),
        );

        // Live since 031. 030 recorded this panel as "unfinished rather than
        // blocked", and it was: the core already probes all four endpoints,
        // holds the drafts and persists them on blur behind its own gate. What
        // was missing was a field somebody could type in.
        if self.identity.is_some() {
            // Opening the panel is what starts the four probes — the web
            // dispatches the same event on the same gesture. Nobody ever sent
            // it here, so every endpoint badge said "checking" until a
            // keystroke re-probed it.
            self.settings_opened(SettingsProbe::Endpoints, cx);
            let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
            let page = cx.entity();
            for (i, endpoint) in view.endpoints.iter().enumerate() {
                let (label, hint) = copy.get(i).cloned().unwrap_or_default();
                let badge = settings_live::endpoint_badge(&endpoint.health, &self.settings);
                let field = endpoint.field;
                let focus = self.endpoint_focus(i, cx);
                col = col.child(editable_url_field(
                    ElementId::from(("endpoint", i)),
                    theme,
                    Some(label),
                    &endpoint.value,
                    // The DEFAULT is the placeholder, so an empty field shows
                    // what it will fall back to rather than nothing.
                    gpui::SharedString::from(endpoint.default_value.clone()),
                    badge.as_ref(),
                    Some(hint),
                    settings_live::endpoint_tone(&endpoint.health),
                    &focus,
                    window,
                    {
                        // A keystroke is a draft; leaving the field (or Enter)
                        // is the core's blur, which persists and re-probes —
                        // once, for the whole URL, not for every prefix of it.
                        let page = page.clone();
                        move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                            Self::edit_field(
                                &page,
                                i,
                                settings_live::FieldCommit::Endpoint(field),
                                text,
                                cx,
                            );
                        }
                    },
                    Self::enter_leaves(&page),
                ));
            }
            return self.endpoints_footer(col, theme, cx);
        }

        for (i, endpoint) in settings_fixtures::ENDPOINTS.iter().enumerate() {
            let (label, hint) = copy[i].clone();
            // Over a second the pill says WHY it is amber. Without the word a
            // reader has to know that 1.2s is bad and 45ms is not, which is
            // exactly the comparison the unit change already breaks.
            let slow = self.settings.network_slow.clone();
            let badge = latency(
                endpoint.latency_ms,
                (endpoint.latency_ms >= 1000).then_some(slow.as_ref()),
            );
            col = col.child(url_field(
                theme,
                Some(label),
                gpui::SharedString::from(endpoint.url),
                Some(&badge),
                Some(hint),
                None,
                None,
            ));
        }
        self.endpoints_footer(col, theme, cx)
    }

    /// The reset / self-host row under the endpoint fields.
    ///
    /// "恢复默认" was drawn as a label with a refresh glyph beside it and no
    /// listener — which is the worst version of an affordance: a person who has
    /// typed a bad endpoint reads a way out that does nothing. Live now, and
    /// only where there is a core to reset (the mock has no endpoints to
    /// restore).
    fn endpoints_footer(&mut self, col: Div, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let live = self.identity.is_some();
        let reset = div()
            .id("settings-endpoints-reset")
            .flex()
            .items_center()
            .gap(px(8.))
            .when(live, |el| el.cursor_pointer())
            .child(icon_img(
                &mut self.icons,
                Icon::RefreshCw,
                false,
                if live { theme.accent } else { theme.fg_muted },
                14.,
            ))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(if live { theme.accent } else { theme.fg_muted })
                    .child(self.settings.endpoints_reset.clone()),
            );
        col.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .pt(px(16.))
                .child(if live {
                    reset
                        // Asks first (FR-010): every address the person typed
                        // goes.
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.confirm = Some(Confirm::ResetEndpoints);
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    reset.into_any_element()
                })
                .child(panels::clickable(
                    "settings-endpoints-guide",
                    live.then(|| {
                        Box::new(|_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            cx.open_url(crate::onboarding_flow::SELF_HOSTING_URL);
                        }) as panels::Click
                    }),
                    div()
                        .id("endpoints-self-hosting-guide")
                        .cursor_pointer()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.info_base)
                        .child(self.settings.endpoints_guide.clone()),
                )),
        )
    }

    /// DST7 — how much of this device Vela is using, and what can be given back.
    /// The default transaction speed (spec 069): the three speeds, each with
    /// the line on what it buys, the stored one ticked. Choosing one commits
    /// and persists at once — `fee_tier_pref` — and a send already open
    /// follows it until the person picks a speed on that send.
    fn settings_fee_speed(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        use vela_core::app::fee_policy::FeeTier;
        use vela_core::app::fee_tier_pref::{Event as FeeTierPrefEvent, FeeTierPref};
        let view = resident::resident::<FeeTierPref>(cx).read(cx).view();
        let s = &self.flow_strings;
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(4.))
            .rounded(px(12.))
            .bg(theme.bg_sunken);
        for (index, tier) in view.offered.iter().copied().enumerate() {
            let (name, hint) = match tier {
                FeeTier::Standard => (
                    s.gas_tier_standard.clone(),
                    s.gas_tier_hint_standard.clone(),
                ),
                FeeTier::Slow => (s.gas_tier_slow.clone(), s.gas_tier_hint_slow.clone()),
                FeeTier::Fast | FeeTier::Rapid => {
                    (s.gas_tier_fast.clone(), s.gas_tier_hint_fast.clone())
                }
            };
            let selected = tier == view.tier;
            list = list.child(
                div()
                    .id(ElementId::from(("settings-fee-speed", index)))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme.bg_raised))
                    .hover(|row| row.bg(theme.bg_raised))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        resident::resident::<FeeTierPref>(cx).update(cx, |pref, cx| {
                            pref.dispatch(FeeTierPrefEvent::UserChose { tier }, cx);
                        });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .text_color(if selected {
                                        theme.accent
                                    } else {
                                        theme.fg_base
                                    })
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(hint),
                            ),
                    )
                    .child(div().w(px(16.)).children(selected.then(|| {
                        icon_img(&mut self.icons, Icon::Check, false, theme.accent, 16.)
                    }))),
            );
        }
        div().flex().flex_col().max_w(px(560.)).child(list)
    }

    /// How this device signs by default (spec 071): the five ways in the
    /// core's order, the stored one ticked — choosing commits and persists at
    /// once (`sign_pref`) — and the Trusted Signer's page under them. Every
    /// signing sheet STARTS at the choice; none writes back to it.
    fn settings_signing(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        use vela_core::app::sign_pref::{Event as SignPrefEvent, SignPref};
        let view = resident::resident::<SignPref>(cx).read(cx).view();
        let s = &self.settings;
        let mut list = div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .p(px(4.))
            .rounded(px(12.))
            .bg(theme.bg_sunken);
        for (index, method) in view.offered.iter().enumerate() {
            let name = s
                .sign_with_options
                .iter()
                .find(|(id, _)| id == method)
                .map_or_else(
                    || SharedString::from(method.clone()),
                    |(_, title)| title.clone(),
                );
            // The one choice that is not a place a passkey is says what it is.
            let line = (method == vela_core::trusted_signer::METHOD)
                .then(|| s.trusted_signer_body.clone());
            let selected = *method == view.method;
            let chosen = method.clone();
            list = list.child(
                div()
                    .id(ElementId::from(("settings-sign-with", index)))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .px(px(12.))
                    .py(px(10.))
                    .rounded(px(10.))
                    .cursor_pointer()
                    .when(selected, |row| row.bg(theme.bg_raised))
                    .hover(|row| row.bg(theme.bg_raised))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        let method = chosen.clone();
                        resident::resident::<SignPref>(cx).update(cx, |pref, cx| {
                            pref.dispatch(SignPrefEvent::MethodChosen { method }, cx);
                        });
                        cx.notify();
                    }))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .flex_1()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .text_color(if selected {
                                        theme.accent
                                    } else {
                                        theme.fg_base
                                    })
                                    .child(name),
                            )
                            .children(line.map(|line| {
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(line)
                            })),
                    )
                    .child(div().w(px(16.)).children(selected.then(|| {
                        icon_img(&mut self.icons, Icon::Check, false, theme.accent, 16.)
                    }))),
            );
        }

        // The page. Its badge is where it is — "Official", or the host a
        // person chose — and the field holds what they are typing until Save
        // hands it to the core, which normalises it or says why not.
        let s = &self.settings;
        let badge = pill(
            Tone::Neutral,
            if view.signer_url_is_default {
                s.signer_page_official.clone()
            } else {
                SharedString::from(page_host(&view.signer_url))
            },
        );
        let refused = match view.signer_url_error.as_deref() {
            Some("insecure") => Some(s.signer_page_insecure.clone()),
            Some(_) => Some(s.signer_page_invalid.clone()),
            None => None,
        };
        let (title, subtitle, foreign, save, reset) = (
            s.signer_page_title.clone(),
            s.signer_page_subtitle.clone(),
            s.signer_page_foreign.clone(),
            s.signer_page_save.clone(),
            s.signer_page_reset.clone(),
        );
        let focus = self
            .signer_page_focus
            .get_or_insert_with(|| cx.focus_handle())
            .clone();
        let typed = self
            .signer_page_draft
            .clone()
            .unwrap_or_else(|| view.signer_url.clone());
        let page = cx.entity();
        let mut section = div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(editable_url_field(
                "settings-signer-page",
                theme,
                Some(title),
                &typed,
                SharedString::from(vela_core::trusted_signer::DEFAULT_SIGNER_URL),
                Some(&badge),
                Some(subtitle),
                refused.is_some().then_some(Tone::Error),
                &focus,
                window,
                move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                    page.update(cx, |page, cx| {
                        page.signer_page_draft = Some(text);
                        cx.notify();
                    });
                },
                // Saved by its own button, which asks the core; Enter does
                // not stand in for it.
                |_, _| {},
            ));
        if let Some(refused) = refused {
            section = section.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.error_base)
                    .child(refused),
            );
        }
        // A page elsewhere can show a request but not sign it: said beside
        // the address rather than discovered at the worst moment (R5).
        if !view.signer_uses_wallet_passkeys {
            section = section.child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.warning_base)
                    .child(foreign),
            );
        }
        // Drawn like the networks panel's header action: a settings panel has
        // no primary CTA, and the accent means "this moves the money".
        let mut actions = div().flex().items_center().gap(px(16.)).child(
            div()
                .id("settings-signer-page-save")
                .h(px(36.))
                .px(px(16.))
                .rounded(px(10.))
                .flex()
                .flex_none()
                .items_center()
                .cursor_pointer()
                .bg(theme.bg_raised)
                .border_1()
                .border_color(theme.divider)
                .hover(|el| el.bg(theme.bg_sunken))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(save)
                .on_click(cx.listener(move |page, _, _, cx| {
                    let text = page
                        .signer_page_draft
                        .clone()
                        .unwrap_or_else(|| typed.clone());
                    let stored = resident::resident::<SignPref>(cx).update(cx, |pref, cx| {
                        pref.dispatch(SignPrefEvent::SignerUrlSubmitted { text }, cx);
                        pref.view().signer_url_error.is_none()
                    });
                    // Taken: the field shows the page as the core stored it.
                    // Refused: what was typed stays, under its reason.
                    if stored {
                        page.signer_page_draft = None;
                    }
                    cx.notify();
                })),
        );
        if !view.signer_url_is_default {
            actions = actions.child(
                div()
                    .id("settings-signer-page-reset")
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .cursor_pointer()
                    .on_click(cx.listener(|page, _, _, cx| {
                        resident::resident::<SignPref>(cx).update(cx, |pref, cx| {
                            pref.dispatch(SignPrefEvent::SignerUrlReset, cx);
                        });
                        page.signer_page_draft = None;
                        cx.notify();
                    }))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::RefreshCw,
                        false,
                        theme.accent,
                        14.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.accent)
                            .child(reset),
                    ),
            );
        }
        div()
            .flex()
            .flex_col()
            .gap(px(32.))
            .max_w(px(560.))
            .child(list)
            .child(section.child(actions))
    }

    fn settings_storage(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        // Live since 031; from the core's catalog since 072 — the rows, which
        // key is whose, what a cache is — with this device's own numbers.
        let live = self.identity.is_some();
        let preset = format_prefs::current().number;
        let report = live.then(crate::executor::device_storage::measure);
        let s = &self.settings;
        let (amount, unit, count, segments) = match &report {
            Some(report) => {
                let (amount, unit) = settings_live::bytes_text(report.total_bytes, preset);
                (
                    amount,
                    unit,
                    report.key_count,
                    settings_live::storage_segments(report),
                )
            }
            None => (
                gpui::SharedString::from(settings_fixtures::STORAGE_AMOUNT),
                gpui::SharedString::from(settings_fixtures::STORAGE_UNIT),
                settings_fixtures::STORAGE_RECORDS as usize,
                settings_fixtures::STORAGE_SEGMENTS,
            ),
        };
        let summary = gpui::SharedString::from(crate::wallet::fill(
            &s.storage_summary,
            "count",
            &count.to_string(),
        ));
        let mut col = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_baseline()
                    .gap(px(8.))
                    .pb(px(16.))
                    .child(
                        div()
                            .text_size(theme::text_balance_hero())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .child(amount),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(unit),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(summary),
                    ),
            )
            .child(storage_bar(theme, &segments));

        let Some(report) = report else {
            // The design surfaces: the mock's three groups, drawn and inert.
            for group in settings_fixtures::storage_groups(&self.settings) {
                let action = group.action.clone();
                col = col.child(storage_group(theme, &group));
                if let Some(action) = action {
                    col = col.child(
                        div()
                            .pt(px(16.))
                            .text_size(theme::text_row_sub())
                            .text_color(theme.info_base)
                            .child(action),
                    );
                }
            }
            return col;
        };

        let page = cx.entity();
        for (key, group) in
            ["storage-user", "storage-cache"]
                .into_iter()
                .zip(settings_live::storage_groups(
                    &report,
                    &self.settings,
                    preset,
                ))
        {
            // Your data asks first — it cannot come back. A cache clears at
            // once: it rebuilds on its own, and asking would only teach a
            // person to click through the questions that matter.
            let actions = group
                .items
                .iter()
                .map(|item| {
                    let (page, id, destructive) = (page.clone(), item.id, item.destructive);
                    Some(Box::new(
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            page.update(cx, |this, cx| {
                                if destructive {
                                    this.confirm = Some(Confirm::ClearItem(id));
                                    cx.notify();
                                } else {
                                    this.clear_storage_row(id, cx);
                                }
                            });
                        },
                    ) as panels::Click)
                })
                .collect();
            let clear_all = group.action.clone();
            col = col.child(storage_group_with(key, theme, &group, actions));
            if let Some(clear_all) = clear_all {
                col = col.child(panels::clickable(
                    "storage-clear-caches",
                    Some(Box::new(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            this.confirm = Some(Confirm::ClearCaches);
                            cx.notify();
                        },
                    ))),
                    div()
                        .pt(px(16.))
                        .text_size(theme::text_row_sub())
                        .text_color(theme.info_base)
                        .child(clear_all),
                ));
            }
        }
        // The connections group is the browser machine's list, live: one row
        // per connected site, each with its own Disconnect — the web's
        // `withLiveConnections`.
        col.child(self.storage_connections(theme, cx))
    }

    /// Settings → Storage → Connections, from the browser machine.
    ///
    /// A revoke here is the same event the connection panel sends: the grant
    /// leaves the file AND the live session, and any open page of that site
    /// hears `accountsChanged []` and `disconnect`. "Disconnect all" is the
    /// Storage clear, and reaches the live session too (spec 070 FR-017).
    /// Both ask first (spec 072).
    fn storage_connections(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let host = self.browser_host(cx);
        let sites = host.read(cx).view.sites.clone();
        let s = &self.settings;
        let page = cx.entity();
        let revoke_all = || -> Option<panels::Click> {
            let page = page.clone();
            Some(Box::new(
                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                    page.update(cx, |this, cx| {
                        this.confirm = Some(Confirm::DisconnectAll);
                        cx.notify();
                    });
                },
            ))
        };
        if sites.is_empty() {
            // Nothing connected: the one row, saying so. Its action has
            // nothing to do and is not armed.
            let group = settings_fixtures::StorageGroup {
                label: s.storage_connections.clone(),
                action: None,
                items: vec![settings_fixtures::StorageItem {
                    id: "dapps",
                    label: s.item_dapps.clone(),
                    meta: SharedString::from(crate::wallet::fill(&s.count_sites, "count", "0")),
                    action: s.storage_disconnect_all.clone(),
                    destructive: true,
                }],
            };
            return storage_group_with("storage-sessions", theme, &group, vec![None]);
        }
        let group = settings_fixtures::StorageGroup {
            label: s.storage_connections.clone(),
            action: None,
            items: sites
                .iter()
                .map(|site| settings_fixtures::StorageItem {
                    id: "dapps",
                    label: signing_live::dapp_identity(&site.origin).0,
                    meta: crate::contacts::model::shorten(&site.address),
                    // Singular: this row cuts off ONE site. "Disconnect all"
                    // on a row that disconnects one is the label somebody
                    // taps meaning something else.
                    action: self.explore.disconnect.clone(),
                    destructive: true,
                })
                .collect(),
        };
        let actions = sites
            .iter()
            .map(|site| {
                let (page, origin) = (page.clone(), site.origin.clone());
                Some(Box::new(
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        page.update(cx, |this, cx| this.revoke_site(origin.clone(), cx));
                    },
                ) as panels::Click)
            })
            .collect();
        div()
            .flex()
            .flex_col()
            .child(storage_group_with(
                "storage-sessions",
                theme,
                &group,
                actions,
            ))
            .child(panels::clickable(
                "storage-disconnect-all",
                revoke_all(),
                div()
                    .pt(px(16.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.error_base)
                    .child(s.storage_disconnect_all.clone()),
            ))
    }

    /// What one 清除 is about, in the row's own words.
    ///
    /// Nothing new is written for it: the title is the row's label, the body is
    /// its GROUP's label — which is where the consequence is spelled out — and
    /// the confirm word is the row's own action. iOS composes the same sheet
    /// the same way.
    fn clear_storage_target(
        &self,
        id: Option<&'static str>,
    ) -> (SharedString, SharedString, SharedString, bool) {
        let groups = settings_fixtures::storage_groups(&self.settings);
        let Some(id) = id else {
            return (
                self.settings.storage_clear_all.clone(),
                self.settings.storage_caches.clone(),
                self.settings.storage_clear_all.clone(),
                false,
            );
        };
        for group in &groups {
            if let Some(item) = group.items.iter().find(|item| item.id == id) {
                return (
                    item.label.clone(),
                    group.label.clone(),
                    item.action.clone(),
                    item.destructive,
                );
            }
        }
        (
            self.settings.storage_clear.clone(),
            SharedString::from(""),
            self.settings.storage_clear.clone(),
            true,
        )
    }

    /// DST8 — the build, the technical inventory, the three links.
    fn settings_about(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        // A signed-in window states the crate's OWN version. The panel said
        // v1.0.0 while the crate was 0.1.1, and a bug report that quotes it
        // names a version that does not exist.
        let live = self.identity.is_some();
        let mut col = div()
            .flex()
            .flex_col()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(16.))
                    .pb(px(24.))
                    // DST8 draws the mark beside the tagline; without it the
                    // panel opens on two lines of grey text and no brand.
                    .child(crate::ui::vela_mark(theme, px(44.)))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(4.))
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .text_color(theme.fg_muted)
                                    .child(s.about_tagline.clone()),
                            )
                            .child(
                                div()
                                    .font_family(theme::font_mono())
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(settings_fixtures::about_version(s, live)),
                            ),
                    ),
            )
            .child(
                div()
                    .pb(px(4.))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(s.about_section_technical.clone()),
            );
        // The network count is the CORE's, not the mock's: the drawn 12 was
        // the built-in list of a year ago, and About is the page somebody
        // opens to check what they are running.
        let networks = u32::try_from(
            vela_core::app::network_admin::BUILTIN_CHAINS.len()
                + self
                    .identity
                    .is_some()
                    .then(|| {
                        resident::resident::<NetworkAdmin>(cx)
                            .read(cx)
                            .view()
                            .networks
                            .iter()
                            .filter(|row| row.is_custom)
                            .count()
                    })
                    .unwrap_or(0),
        )
        .unwrap_or(u32::MAX);
        for (label, value, mono) in settings_fixtures::about_rows_with(&self.settings, networks) {
            col = col.child(key_value_row(
                theme,
                &mut self.icons,
                label,
                value,
                mono,
                false,
                None,
            ));
        }
        col = col.child(
            div()
                .pt(px(24.))
                .pb(px(4.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(self.settings.about_section_links.clone()),
        );
        for (label, value, url) in settings_fixtures::about_links(&self.settings) {
            col = col.child(key_value_row(
                theme,
                &mut self.icons,
                label,
                value,
                true,
                true,
                Some(url),
            ));
        }
        col.child(
            div()
                .pt(px(24.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(self.settings.about_footer.clone()),
        )
    }

    /// The centred dialog over the settings section (DST4b / DSR1).
    fn settings_dialog_overlay(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let kind = self.settings_dialog?;
        let s = &self.settings;
        let (title, subtitle) = match kind {
            // The subtitle names the chain the wizard has RESOLVED, and there
            // is none until somebody picks one. The mock's "Zora · 链 ID 7777777"
            // sat over a live wizard that had resolved nothing, which is the
            // dialog telling somebody it already knows what they are adding.
            SettingsDialog::AddNetwork if self.identity.is_some() => (
                s.add_network.clone(),
                resident::resident::<NetworkAdmin>(cx)
                    .read(cx)
                    .view()
                    .wizard
                    .chain_info
                    .map(|info| {
                        gpui::SharedString::from(format!(
                            "{} · {}",
                            info.name,
                            settings_fixtures::chain_meta(s, u64::from(info.chain_id))
                        ))
                    }),
            ),
            SettingsDialog::AddNetwork => (
                s.add_network.clone(),
                Some(gpui::SharedString::from(format!(
                    "Zora · {}",
                    settings_fixtures::chain_meta(s, settings_fixtures::ZORA_CHAIN_ID)
                ))),
            ),
            SettingsDialog::FixRpc => (s.rpc_fix_title.clone(), None),
            SettingsDialog::EraseDevice => (s.erase_title.clone(), Some(s.erase_subtitle.clone())),
        };

        let body = match kind {
            SettingsDialog::AddNetwork if self.identity.is_some() => {
                self.settings_add_network_live(theme, window, cx)
            }
            SettingsDialog::AddNetwork => self.settings_add_network_body(theme),
            SettingsDialog::FixRpc => match self.settings_fix_chain {
                Some(chain_id) if self.identity.is_some() => {
                    self.settings_fix_rpc_live(theme, chain_id, window, cx)
                }
                _ => self.settings_fix_rpc_body(theme),
            },
            SettingsDialog::EraseDevice => self.settings_erase_body(theme, cx),
        };

        // The body scrolls, the header does not, and the card stops at 80% of
        // the window (`ui::dialog`). It once had no cap and nothing to scroll:
        // at the 1280x800 minimum the add-network verdict reached within a few
        // points of both window edges in English, and one more checklist row,
        // one step up in text scale or a longer language put the CTA below
        // the window with no way to reach it.
        Some(
            crate::ui::dialog::dialog(
                "settings-dialog",
                theme,
                window,
                title,
                subtitle,
                self.dialog_close_icon(theme),
                body,
                &self.dialog_scroll("settings-dialog"),
                Self::closer(cx, |this, cx| this.close_settings_dialog(cx)),
            )
            .into_any_element(),
        )
    }

    /// DST4b's body, live: type a chain, see its verdict, add it.
    ///
    /// 030 proved the pipeline end to end by dispatching the events by hand
    /// (SC-001: Zora 12→13, still there after a relaunch). What it could not do
    /// was let a person type — the search box was a placeholder. This is that
    /// box.
    fn settings_add_network_live(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = &self.settings;
        let description = s.add_network_desc.clone();
        let search_placeholder = s.search_placeholder.clone();
        let custom_title = s.custom_rpc_title.clone();
        let custom_placeholder = s.custom_rpc_placeholder.clone();
        let checks_title = s.compatibility_check.clone();
        let cta = s.add_network.clone();
        let retry = s.recheck.clone();
        let hover_accent = theme.accent_hover;

        let view = resident::resident::<NetworkAdmin>(cx).read(cx).view();
        let wizard = view.wizard.clone();
        let search_focus = self.endpoint_focus(WIZARD_SEARCH_FOCUS, cx);
        let rpc_focus = self.endpoint_focus(WIZARD_RPC_FOCUS, cx);

        let mut col = div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(description),
            )
            .child(editable_url_field(
                ElementId::from("wizard-search"),
                theme,
                None,
                &wizard.query,
                search_placeholder,
                None,
                None,
                None,
                &search_focus,
                window,
                move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                    resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                        resident.dispatch(NetEvent::SearchInput { query: text }, cx);
                    });
                },
                // The search runs as it is typed; there is nothing to commit.
                |_, _| {},
            ));

        // The suggestions the index answered with. Each one is a click that
        // resolves and checks that chain — which is the whole pipeline 030
        // proved and could not reach from the keyboard.
        for (i, entry) in wizard.suggestions.iter().take(6).enumerate() {
            let chain_id = entry.chain_id;
            let selected = wizard
                .chain_info
                .as_ref()
                .is_some_and(|c| c.chain_id == chain_id);
            col = col.child(
                div()
                    .id(ElementId::from(("wizard-suggestion", i)))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .p(px(8.))
                    .rounded(px(10.))
                    .cursor_pointer()
                    .when(selected, |el| el.bg(theme.bg_sunken))
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(chain_logo_mark(
                        u64::from(chain_id),
                        crate::settings::model::lettermark(&entry.name),
                        crate::settings::model::chain_tint(u64::from(chain_id))
                            .unwrap_or(0x8A_8F_98),
                        32.,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(gpui::SharedString::from(entry.name.clone())),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(gpui::SharedString::from(chain_id.to_string())),
                    )
                    .on_click(cx.listener(move |_, _, _, cx| {
                        resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                NetEvent::ChainSelected {
                                    chain_id,
                                    // A fresh pick throws away a custom RPC
                                    // typed for a DIFFERENT chain. Keeping it
                                    // would check chain A against chain B's
                                    // endpoint.
                                    keep_custom_rpc: false,
                                },
                                cx,
                            );
                        });
                        cx.notify();
                    })),
            );
        }

        // What the wizard is doing, or why it stopped. Everything below this
        // line — the check list, the RPC field, the CTA — draws only once the
        // core has a verdict, so without it the dialog answers a click with
        // nothing at all. (The 032 phase 6 rule, applied to somebody else's
        // screen: the core computed a refusal; the screen must say it.)
        if let Some(notice) = settings_live::wizard_notice(&wizard, &self.settings) {
            col = col.child(match notice {
                settings_live::WizardNotice::Progress(body) => div()
                    .flex()
                    .items_center()
                    .gap(px(10.))
                    .child(crate::ui::spinner(theme.fg_subtle, px(14.), px(2.)))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(body),
                    ),
                settings_live::WizardNotice::Refusal(body) => div()
                    .p(px(12.))
                    .rounded(px(12.))
                    .bg(theme.error_soft)
                    .border_1()
                    .border_color(theme.error_base)
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(body),
            });
        }

        if let Some(compat) = wizard.compat.as_ref() {
            match settings_live::compat_checks(compat, &self.settings) {
                Some(checks) => {
                    // The VERDICT, before the list that explains it. The core
                    // reaches one and the live dialog never said it: every
                    // other client shows this pill, and after spec 081 gave the
                    // checklist a crossed row, a desktop reader saw a red cross
                    // and a warning with nothing anywhere saying the chain
                    // works. `settings.compatible` had been loaded and unused.
                    let badge = settings_fixtures::pill(
                        if compat.compatible {
                            settings_fixtures::Tone::Ok
                        } else {
                            settings_fixtures::Tone::Error
                        },
                        if compat.compatible {
                            self.settings.compatible.clone()
                        } else {
                            self.settings.wizard_incompatible.clone()
                        },
                    );
                    col = col.child(
                        div()
                            .flex()
                            .items_center()
                            .child(status_pill(theme, &badge)),
                    );
                    col = col.child(check_list(theme, &mut self.icons, checks_title, &checks));
                    // Spec 081 FR-009. The core can say "this chain works" and
                    // "a wallet with more than one key cannot be created here"
                    // at the same time; both are true, and the second one is
                    // the sentence a person with several passkeys needs.
                    if compat.compatible && !compat.multi_key_ready {
                        col = col.child(crate::settings::components::callout(
                            theme,
                            &mut self.icons,
                            crate::settings::components::CalloutTone::Warning,
                            self.settings.single_key_only.clone(),
                        ));
                    }
                    // Spec 081: an INCOMPATIBLE verdict needs somewhere to go.
                    // The web has offered both of these since it was wired;
                    // desktop drew the red rows, then the custom-RPC field,
                    // and then nothing — so a person could type the RPC that
                    // would have changed the answer and have no way to ask
                    // again. The re-check keeps what they typed, for the same
                    // reason the retry below does.
                    if !compat.compatible {
                        let chain_id = compat.chain_id;
                        col = col.child(
                            div()
                                .id("wizard-recheck-rpc")
                                .flex()
                                .items_center()
                                .justify_center()
                                .h(px(CONTACTS_BUTTON_H))
                                .rounded(px(12.))
                                .cursor_pointer()
                                .border_1()
                                .border_color(theme.outline_strong)
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(self.settings.recheck_with_rpc.clone())
                                .on_click(cx.listener(move |_, _, _, cx| {
                                    resident::resident::<NetworkAdmin>(cx).update(
                                        cx,
                                        |resident, cx| {
                                            resident.dispatch(
                                                NetEvent::ChainSelected {
                                                    chain_id,
                                                    keep_custom_rpc: true,
                                                },
                                                cx,
                                            );
                                        },
                                    );
                                    cx.notify();
                                })),
                        );
                        col = col.child(
                            div()
                                .id("wizard-chain-setup-tool")
                                .flex()
                                .items_center()
                                .justify_center()
                                .cursor_pointer()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.info_base)
                                .child(self.settings.open_chain_setup_tool.clone())
                                .on_click(|_, _, cx| {
                                    cx.open_url(crate::onboarding_flow::CHAIN_SETUP_URL);
                                }),
                        );
                    }
                }
                // The probe could not reach a verdict. A retry, never a
                // condemnation — the core's invariant ③, and the difference
                // between "this chain does not work" and "we could not ask".
                None => {
                    let chain_id = compat.chain_id;
                    col = col.child(
                        div()
                            .id("wizard-retry")
                            .flex()
                            .items_center()
                            .justify_center()
                            .h(px(CONTACTS_BUTTON_H))
                            .rounded(px(12.))
                            .cursor_pointer()
                            .border_1()
                            .border_color(theme.outline_strong)
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(retry)
                            .on_click(cx.listener(move |_, _, _, cx| {
                                resident::resident::<NetworkAdmin>(cx).update(
                                    cx,
                                    |resident, cx| {
                                        resident.dispatch(
                                            NetEvent::ChainSelected {
                                                chain_id,
                                                // A recheck KEEPS the typed RPC:
                                                // it is often the reason to recheck.
                                                keep_custom_rpc: true,
                                            },
                                            cx,
                                        );
                                    },
                                );
                                cx.notify();
                            })),
                    );
                }
            }
        }

        col = col.child(editable_url_field(
            ElementId::from("wizard-rpc"),
            theme,
            Some(custom_title),
            &wizard.custom_rpc,
            custom_placeholder,
            None,
            None,
            None,
            &rpc_focus,
            window,
            move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                    resident.dispatch(NetEvent::CustomRpcEdited { value: text }, cx);
                });
            },
            // A draft the wizard's own Add button commits.
            |_, _| {},
        ));

        // The CTA renders only when the core says it may. `can_add` is its
        // whole judgement — resolved, checked, compatible, not already added —
        // and re-deriving any part of it here would be a second opinion about
        // whether a chain is safe to add.
        if wizard.can_add {
            col = col.child(
                div()
                    .id("settings-add-network-confirm")
                    .h(px(CONTACTS_BUTTON_H))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(theme.accent)
                    .hover(move |el| el.bg(hover_accent))
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_inverse)
                    .child(cta)
                    .on_click(cx.listener(|this, _, _, cx| {
                        let now_iso = crate::executor::now_iso();
                        // Close on the core's word, not on the click. Every
                        // gate in `add_confirmed` — not loaded, not Checked,
                        // not compatible — refuses by returning `done()`, so a
                        // dialog that closes itself would be the phase 6
                        // pattern in its worst form: the person's press
                        // disappears the screen and nothing was added. If the
                        // core did not record it, the dialog stays up with its
                        // state, and the notice above says why.
                        let added =
                            resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                                let before = resident.view().last_added_chain_id;
                                resident.dispatch(NetEvent::AddConfirmed { now_iso }, cx);
                                resident.view().last_added_chain_id != before
                            });
                        if added {
                            this.close_settings_dialog(cx);
                            // A chain the person just added is a chain nobody
                            // has counted yet. The web forces the same read at
                            // the same moment; without it the new network sits
                            // in the list contributing nothing until something
                            // else happens to refresh.
                            crate::executor::balance_dashboard::refresh(cx);
                        }
                        cx.notify();
                    })),
            );
        }
        col
    }

    /// DST4b's body: the chosen chain, its verdict, and the CTA.
    fn settings_add_network_body(&mut self, theme: &Theme) -> Div {
        let s = &self.settings;
        let checks = settings_fixtures::compatibility_checks(s, true);
        let badge = pill(Tone::Ok, s.compatible.clone());
        let best = gpui::SharedString::from(crate::wallet::fill(
            &s.best_rpc,
            "latencyMs",
            &settings_fixtures::ZORA_BEST_RPC_MS.to_string(),
        ));
        let checks_title = s.compatibility_check.clone();
        let custom_title = s.custom_rpc_title.clone();
        let custom_placeholder = s.custom_rpc_placeholder.clone();
        let cta = s.add_network.clone();
        let hover_accent = theme.accent_hover;

        let search_placeholder = s.search_placeholder.clone();
        let description = s.add_network_desc.clone();

        div()
            .flex()
            .flex_col()
            .gap(px(20.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(description),
            )
            .child(
                div()
                    .h(px(44.))
                    .px(px(12.))
                    .rounded(px(10.))
                    .bg(theme.bg_sunken)
                    .border_1()
                    .border_color(theme.divider)
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::Search,
                        false,
                        theme.fg_subtle,
                        16.,
                    ))
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(search_placeholder),
                    ),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(chain_mark("Z".into(), 0x8c8c8c, 32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_panel_title())
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.fg_base)
                                    .child("Zora"),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(best),
                            ),
                    )
                    .child(status_pill(theme, &badge)),
            )
            .child(check_list(theme, &mut self.icons, checks_title, &checks))
            .child(url_field(
                theme,
                Some(custom_title),
                custom_placeholder,
                None,
                None,
                None,
                None,
            ))
            .child(
                div()
                    .id("settings-add-network-confirm")
                    .h(px(CONTACTS_BUTTON_H))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(theme.accent)
                    .hover(move |el| el.bg(hover_accent))
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_inverse)
                    .child(cta),
            )
    }

    /// DSR1's body: one network is unreachable, and this is where it is fixed.
    /// DSR1, live: the chain the banner named, and a field that really saves.
    ///
    /// The banner has been telling the truth since phase 11 and pointing at a
    /// dialog that could not act on it. This closes that loop — and it is the
    /// one place in the app where a save can be REFUSED, because an endpoint
    /// that answers `eth_chainId` with another chain's id would route somebody's
    /// money to the wrong chain.
    fn settings_fix_rpc_live(
        &mut self,
        theme: &Theme,
        chain_id: u32,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let s = &self.settings;
        let badge = pill(Tone::Error, s.offline.clone());
        let warning = s.rpc_fix_warning.clone();
        let providers_hint = s.rpc_providers_hint.clone();
        let report = s.rpc_report.clone();
        let name = crate::executor::custom_tokens::network_name(chain_id);
        let letter = crate::settings::model::lettermark(&name);
        let colour = crate::settings::model::chain_tint(u64::from(chain_id)).unwrap_or(0x8A_8F_98);
        let meta = settings_fixtures::chain_meta(s, u64::from(chain_id));

        let mut chips = div().flex().flex_wrap().gap(px(8.));
        for (index, (provider, url)) in settings_fixtures::RPC_PROVIDER_LINKS
            .into_iter()
            .enumerate()
        {
            // Each names a place to get a working endpoint, and goes there.
            let hover = theme.outline_strong;
            chips = chips.child(
                div()
                    .id(("rpc-fix-provider", index))
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .bg(theme.bg_raised)
                    .border_1()
                    .border_color(theme.divider)
                    .hover(move |el| el.border_color(hover))
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .on_click(move |_, _, cx| cx.open_url(url))
                    .child(provider),
            );
        }

        // The SAME field the network card opens, deliberately. Two editors for
        // one override is two places a refusal has to be worded, and they would
        // drift.
        let (rpc, _) = self.network_override_fields(theme, chain_id, 0, window, cx);

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(chain_logo_mark(u64::from(chain_id), letter, colour, 32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_panel_title())
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.fg_base)
                                    .child(gpui::SharedString::from(name)),
                            )
                            .child(
                                div()
                                    .font_family(theme::font_mono())
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(meta),
                            ),
                    )
                    .child(status_pill(theme, &badge)),
            )
            .child(callout(
                theme,
                &mut self.icons,
                CalloutTone::Warning,
                warning,
            ))
            .child(rpc)
            // No save button. The field persists on its own — the core's blur
            // IS the save, behind its own gate — and a button that only
            // sometimes saves is worse than no button.
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(providers_hint),
            )
            .child(chips)
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.info_base)
                    .child(report),
            )
    }

    fn settings_fix_rpc_body(&mut self, theme: &Theme) -> Div {
        let s = &self.settings;
        let n = settings_fixtures::network(settings_fixtures::RPC_FIX_CHAIN);
        let meta = gpui::SharedString::from(format!(
            "{} · {}",
            settings_fixtures::chain_meta(s, n.chain_id),
            settings_fixtures::RPC_FIX_SYMBOL
        ));
        let badge = pill(Tone::Error, s.offline.clone());
        let warning = s.rpc_fix_warning.clone();
        let label = s.rpc_fix_label.clone();
        let cta = s.rpc_fix_save.clone();
        let providers_hint = s.rpc_providers_hint.clone();
        let report = s.rpc_report.clone();
        let hover_accent = theme.accent_hover;

        let mut chips = div().flex().flex_wrap().gap(px(8.));
        for (name, _) in settings_fixtures::RPC_PROVIDER_LINKS {
            chips = chips.child(
                div()
                    .px(px(12.))
                    .py(px(8.))
                    .rounded(px(8.))
                    .bg(theme.bg_raised)
                    .border_1()
                    .border_color(theme.divider)
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(name),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(chain_mark(n.letter.into(), n.color, 32.))
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_panel_title())
                                    .font_weight(gpui::FontWeight::BOLD)
                                    .text_color(theme.fg_base)
                                    .child(n.name),
                            )
                            .child(
                                div()
                                    .font_family(theme::font_mono())
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_subtle)
                                    .child(meta),
                            ),
                    )
                    .child(status_pill(theme, &badge)),
            )
            .child(callout(
                theme,
                &mut self.icons,
                CalloutTone::Warning,
                warning,
            ))
            .child(url_field(
                theme,
                Some(label),
                gpui::SharedString::from(settings_fixtures::RPC_FIX_URL),
                None,
                None,
                Some(Tone::Error),
                None,
            ))
            .child(
                div()
                    .id("settings-fix-rpc-save")
                    .h(px(CONTACTS_BUTTON_H))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .bg(theme.accent)
                    .hover(move |el| el.bg(hover_accent))
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_inverse)
                    .child(cta),
            )
            .child(
                div()
                    .text_size(theme::text_label())
                    .text_color(theme.fg_subtle)
                    .child(providers_hint),
            )
            .child(chips)
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.info_base)
                    .child(report),
            )
    }

    fn settings_erase_body(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let desc = s.erase_desc.clone();
        let keeps = s.erase_keeps.clone();
        let loses = s.erase_loses.clone();
        let confirm = s.erase_confirm.clone();
        let cancel = s.erase_cancel.clone();
        // A partial wipe names what stayed, in the dialog itself — the person
        // is still signed in, and the button is still live so they can retry.
        let failed = self.erase_failed.as_ref().map(|left| {
            gpui::SharedString::from(format!("{} ({})", s.erase_failed, left.join(", ")))
        });
        let mut body = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .text_color(theme.fg_base)
                    .child(desc),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(keeps),
            )
            .child(callout(theme, &mut self.icons, CalloutTone::Danger, loses));
        if let Some(failed) = failed {
            body = body.child(callout(theme, &mut self.icons, CalloutTone::Danger, failed));
        }
        body.child(
            div()
                .id("settings-erase-confirm")
                .h(px(CONTACTS_BUTTON_H))
                .rounded(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .bg(theme.error_base)
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_inverse)
                .on_click(cx.listener(|this, _, _, cx| this.erase_device(cx)))
                .child(confirm),
        )
        .child(
            div()
                .id("settings-erase-cancel")
                .h(px(CONTACTS_BUTTON_H))
                .rounded(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .border_1()
                .border_color(theme.outline_strong)
                .text_size(theme::text_row_title())
                .text_color(theme.fg_base)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_settings_dialog(cx);
                    cx.notify();
                }))
                .child(cancel),
        )
    }

    // -- column 2: explore (spec 022 DE1–DE4) --------------------------------

    /// The history row the open menu is about: its url and its title.
    ///
    /// Resolved from the ORIGIN the right-click recorded, against the core's
    /// CURRENT list — never from a copy taken when the menu opened, because a
    /// visit can land in between and the row under the cursor is the one the
    /// person means.
    fn take_menu_entry(&mut self, cx: &mut Context<Self>) -> Option<(String, String)> {
        let origin = self.menu_origin.clone()?;
        resident::resident::<BrowserHistory>(cx)
            .read(cx)
            .view()
            .entries
            .iter()
            .find(|entry| entry.origin == origin)
            .map(|entry| (entry.url.clone(), entry.title.clone()))
    }

    /// `VELA_BROWSER_URL=<url>` opens the in-app browser straight onto a site.
    ///
    /// Same env-pin family as `VELA_SCAN_FILE`, and the same reason: the
    /// address bar is a text field, and a headless run — or any screenshot
    /// pass on a machine where synthetic keystrokes do not land — can never
    /// type into one. Without this, the whole dApp path (inject → connect →
    /// signing sheet) can only ever be verified by a person with a keyboard.
    ///
    /// Applied once, on the first frame the browser column draws.
    fn browser_url_from_env(&mut self, cx: &mut Context<Self>) {
        if self.browser_url_pinned {
            return;
        }
        self.browser_url_pinned = true;
        let Ok(url) = std::env::var("VELA_BROWSER_URL") else {
            return;
        };
        if url.trim().is_empty() {
            return;
        }
        self.browsing = true;
        self.browser_home = url.clone();
        #[cfg(not(target_os = "linux"))]
        crate::webview::navigate(&url);
        cx.notify();
    }

    /// Show the tab somebody picked.
    ///
    /// One webview, so a switch is a navigation. A tab with no url is the
    /// start page — the wallet's own screen, not a blank document.
    fn select_browser_tab(&mut self, id: &str, url: Option<&str>, cx: &mut Context<Self>) {
        resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
            resident.dispatch(
                vela_core::app::explore_sites::Event::TabSelected { id: id.to_owned() },
                cx,
            );
        });
        match url {
            Some(url) => {
                self.browsing = true;
                self.browser_home = url.to_owned();
                #[cfg(not(target_os = "linux"))]
                crate::webview::navigate(url);
            }
            None => self.browsing = false,
        }
        cx.notify();
    }

    /// Close a tab, and follow the core to whatever it selected next.
    ///
    /// The neighbour rule is the machine's (right, then left); this only obeys
    /// the answer — a shell that picked its own next tab would be a second
    /// opinion about where a person just went.
    fn close_browser_tab(&mut self, id: &str, cx: &mut Context<Self>) {
        let resident = resident::resident::<ExploreSites>(cx);
        // Only the tab on screen has a document behind it: the others are
        // URLs the one webview will load when somebody picks them.
        let was_shown = resident.read(cx).view().selected_tab.as_deref() == Some(id);
        resident.update(cx, |resident, cx| {
            resident.dispatch(
                vela_core::app::explore_sites::Event::TabClosed { id: id.to_owned() },
                cx,
            );
        });
        let view = resident.read(cx).view();
        let next = view
            .selected_tab
            .as_ref()
            .and_then(|id| view.tabs.iter().find(|tab| &tab.id == id))
            .and_then(|tab| tab.url.clone());
        match next {
            // The neighbour's page replaces this one, and its hello is what
            // retires the closed page's document in the core.
            Some(url) => {
                self.browsing = true;
                self.browser_home = url.clone();
                #[cfg(not(target_os = "linux"))]
                crate::webview::navigate(&url);
            }
            // Nothing left, or a start-page tab: the wallet's own screen.
            None => {
                self.browsing = false;
                if was_shown {
                    self.close_browser_page(cx);
                }
            }
        }
        cx.notify();
    }

    /// The page on screen is closed, not merely hidden: its requests are
    /// settled (4900), a sheet showing one of them closes, and the document
    /// stops running. Hiding the webview alone would leave the page alive and
    /// able to ask for things nobody can see.
    fn close_browser_page(&mut self, cx: &mut Context<Self>) {
        if let Some(host) = self.browser_host.clone() {
            host.update(cx, |host, cx| {
                host.dispatch(
                    DbrEvent::TabClosed {
                        tab: BROWSER_TAB.to_owned(),
                    },
                    cx,
                );
            });
        }
        #[cfg(not(target_os = "linux"))]
        crate::webview::navigate("about:blank");
    }

    /// The browser column: tab strip, toolbar, then either the start page or
    /// the page being browsed. The start page is the same vocabulary the phone
    /// draws — favourites grid, groups of rows — at desktop width.
    fn explore_content(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        if self.identity.is_some() {
            self.browser_url_from_env(cx);
        }
        let browsing = self.browsing;
        // The person's own tabs once they are signed in. ONE webview serves
        // them all, so switching re-navigates rather than swapping a live
        // page — the honest limitation of a single native subview, and the
        // reason a tab remembers a url rather than a session.
        let explore_tabs = resident::resident::<ExploreSites>(cx).read(cx).view();
        let live_tabs =
            self.identity.is_some() && explore_tabs.ready && !explore_tabs.tabs.is_empty();
        let tabs = if live_tabs {
            explore_live::tab_models(&explore_tabs, &self.explore)
        } else {
            explore_fixtures::tabs(&self.explore, browsing)
        };
        let actions = if self.identity.is_some() && explore_tabs.ready {
            let ids: Vec<String> = explore_tabs.tabs.iter().map(|tab| tab.id.clone()).collect();
            let urls: Vec<Option<String>> = explore_tabs
                .tabs
                .iter()
                .map(|tab| tab.url.clone())
                .collect();
            explore_components::TabActions {
                select: ids
                    .iter()
                    .zip(urls.iter())
                    .map(|(id, url)| {
                        let (id, url) = (id.clone(), url.clone());
                        Some(
                            Box::new(cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                                page.select_browser_tab(&id, url.as_deref(), cx);
                            })) as panels::Click,
                        )
                    })
                    .collect(),
                close: ids
                    .iter()
                    .map(|id| {
                        let id = id.clone();
                        Some(
                            Box::new(cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                                page.close_browser_tab(&id, cx);
                            })) as panels::Click,
                        )
                    })
                    .collect(),
                new_tab: Some(Box::new(cx.listener(|page, _: &gpui::ClickEvent, _, cx| {
                    // A new tab is the START page: a browser does not
                    // decide where somebody wants to go next.
                    resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                        resident.dispatch(
                            vela_core::app::explore_sites::Event::TabOpened {
                                url: None,
                                title: None,
                                now_ms: crate::executor::now_ms(),
                            },
                            cx,
                        );
                    });
                    page.browsing = false;
                    cx.notify();
                })) as panels::Click),
            }
        } else {
            explore_components::TabActions::default()
        };
        let strip = explore_components::tab_strip_with(
            theme,
            &mut self.icons,
            &tabs,
            self.explore.new_tab.clone(),
            self.explore.close_tab.clone(),
            actions,
        );
        let identity = self.identity();
        // A REAL page, in a real session. The mock keeps drawing for the
        // gallery and for a window nobody has signed in to, which is the same
        // fork every other live surface takes — and it is what keeps
        // `sweep-gallery.sh` from opening a webview per state.
        #[cfg(not(target_os = "linux"))]
        let live_browser = browsing && self.identity.is_some();
        #[cfg(target_os = "linux")]
        let live_browser = false;
        // What the browser machine says about the page on screen: its site,
        // whether that site is connected, whether its lock may be drawn, and
        // whether its renderer died.
        let tab_view = if live_browser {
            self.browser_host
                .as_ref()
                .and_then(|host| host.read(cx).tab().cloned())
        } else {
            None
        };
        // Whether the page on screen is already a favourite — by ORIGIN, the
        // key the core dedupes favourites on. The star then removes it: a
        // star that can only add is a star that cannot be undone.
        let favorite_origin =
            tab_view
                .as_ref()
                .and_then(|tab| tab.origin.clone())
                .filter(|origin| {
                    explore_tabs.ready
                        && explore_tabs
                            .favorites
                            .iter()
                            .any(|site| &site.origin == origin)
                });
        // The two trailing affordances open different things, so the page — not
        // the component — carries their listeners.
        let star = explore_components::toolbar_control(
            theme,
            &mut self.icons,
            Icon::Star,
            if favorite_origin.is_some() {
                theme.accent
            } else {
                theme.fg_base
            },
        );
        let dots = explore_components::toolbar_control(
            theme,
            &mut self.icons,
            Icon::Ellipsis,
            theme.fg_base,
        );
        // The green dot IS the connection state, so live it is the core's
        // word — a grant for the site on screen — and never "a page is open".
        let connected = if live_browser {
            tab_view
                .as_ref()
                .is_some_and(|tab| tab.connected_address.is_some())
        } else {
            browsing
        };
        let chip = explore_components::account_chip(
            theme,
            &mut self.identicons,
            identity.name.clone(),
            &identity.address,
            connected,
        );
        let trailing = div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                // The star pins the page that is open. It reads the url from
                // the WEBVIEW, never from the address bar's text: what is
                // pinned has to be the document that is actually loaded.
                div()
                    .id("toolbar-star")
                    .cursor_pointer()
                    .child(star)
                    .on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                        if let Some(origin) = favorite_origin.clone() {
                            resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                                resident.dispatch(
                                    vela_core::app::explore_sites::Event::FavoriteRemoved {
                                        origin,
                                    },
                                    cx,
                                );
                            });
                            cx.notify();
                            return;
                        }
                        #[cfg(not(target_os = "linux"))]
                        if let Some(url) = crate::webview::current_url() {
                            // The page's own title when it has reported one;
                            // the host otherwise. The core keeps whichever
                            // arrives until somebody renames the tile.
                            let title = this
                                .browser_title
                                .clone()
                                .or_else(crate::webview::host)
                                .unwrap_or_default();
                            resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                                resident.dispatch(
                                    vela_core::app::explore_sites::Event::FavoriteAdded {
                                        url,
                                        // The page's own title arrives with the
                                        // next meta report; the host is what is
                                        // certainly true right now, and the core
                                        // lets a later title replace it while
                                        // nobody has renamed the tile.
                                        title: (!title.is_empty()).then_some(title),
                                        now_ms: crate::executor::now_ms(),
                                    },
                                    cx,
                                );
                            });
                        }
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("site-menu")
                    .cursor_pointer()
                    .child(dots)
                    .on_click(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                        this.menu = Some((ContactsMenu::Site, event.position(), Anchor::TopRight));
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .id("account-chip")
                    .cursor_pointer()
                    .child(chip)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.panel = if this.panel == PanelId::Connection {
                            PanelId::None
                        } else {
                            PanelId::Connection
                        };
                        cx.notify();
                    })),
            );
        // The host beside the lock is the WEBVIEW's, never the fixture's: a
        // label fed from anywhere else is a claim about which origin is loaded,
        // and that is the one thing browser chrome must not get wrong. Before
        // the first page — and in the mock — it stays the drawn host.
        #[cfg(not(target_os = "linux"))]
        let host = if live_browser {
            crate::webview::host()
                .map_or_else(|| explore_fixtures::uniswap().host, SharedString::from)
        } else {
            explore_fixtures::uniswap().host
        };
        #[cfg(target_os = "linux")]
        let host = explore_fixtures::uniswap().host;
        // The lock tells the truth: the core's judgement of the site on
        // screen, the same one that decides whether it may ask to sign.
        let secure = !live_browser || tab_view.as_ref().is_none_or(|tab| tab.secure);
        let crashed = tab_view.as_ref().is_some_and(|tab| tab.crashed);
        let bar = explore_components::AddressBar {
            browsing,
            host,
            secure,
            placeholder: self.explore.search_placeholder.clone(),
            draft: self.address_draft.clone().map(SharedString::from),
            selected: self.address_selected,
        };
        let field = explore_components::address_field(theme, &mut self.icons, &bar);
        // Editable once somebody is signed in: a click takes the page's URL
        // into the field, Enter goes wherever the core's `browser_input` says
        // the text means — a URL, or a search for it.
        let address: gpui::AnyElement = if self.identity.is_some() {
            div()
                .id("address-bar")
                .track_focus(&self.address_focus)
                .size_full()
                .px(px(12.))
                .flex()
                .items_center()
                .justify_center()
                .overflow_hidden()
                .cursor_text()
                .child(field)
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                    this.edit_address(window, cx);
                }))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| {
                    this.address_key(event, cx);
                }))
                .into_any_element()
        } else {
            field.into_any_element()
        };

        // Drawn and inert in the mock; three real listeners in a live session.
        #[cfg(not(target_os = "linux"))]
        let nav: Option<explore_components::NavActions> = live_browser.then(|| {
            [
                Box::new(|_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                    crate::webview::back();
                }) as panels::Click,
                Box::new(|_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                    crate::webview::forward();
                }),
                Box::new(|_: &gpui::ClickEvent, _: &mut Window, _: &mut gpui::App| {
                    crate::webview::reload();
                }),
            ]
        });
        #[cfg(target_os = "linux")]
        let nav: Option<explore_components::NavActions> = None;

        let toolbar = explore_components::toolbar(theme, &mut self.icons, address, trailing, nav);

        let body: gpui::AnyElement = if live_browser && crashed {
            // The renderer is gone and the page with it; the core has already
            // settled what it asked. Said, with the way back — never a blank
            // rectangle that looks like a page still loading.
            #[cfg(not(target_os = "linux"))]
            crate::webview::hide();
            self.page_crashed(theme, cx).into_any_element()
        } else if live_browser {
            #[cfg(not(target_os = "linux"))]
            {
                let home = self.browser_home.clone();
                let covered = self.dialog_over_browser();
                self.arm_dapp_requests(cx);
                gpui::canvas(
                    |_, _, _| (),
                    move |bounds, (), window, cx| {
                        // Placed from the PAINT pass of the element that owns
                        // this rectangle, so the webview follows the column
                        // through a resize and through the signing panel
                        // opening beside it. Not while a dialog is up over it:
                        // render took it off the screen for that.
                        if !covered {
                            crate::webview::place(bounds, window, &home, cx);
                        }
                    },
                )
                .size_full()
                .into_any_element()
            }
            #[cfg(target_os = "linux")]
            unreachable!("live_browser is false on linux")
        } else if browsing {
            explore_components::demo_page(&explore_fixtures::demo_page())
                .child(
                    // The site's own button is what raises a signing request;
                    // the wallet never invents one.
                    div()
                        .id("demo-action")
                        .absolute()
                        .size_full()
                        .cursor_pointer()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.panel = PanelId::Signing;
                            cx.notify();
                        })),
                )
                .into_any_element()
        } else {
            self.explore_start(theme, cx).into_any_element()
        };

        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .flex()
            .flex_col()
            .bg(theme.bg_base)
            .child(strip)
            .child(toolbar)
            .child(body)
    }

    /// The renderer behind the page died (spec 070 FR-013; macOS reports it).
    ///
    /// Said, with the way back. The browser machine has already settled what
    /// the page had asked (4900) and closed any sheet it raised; Reload starts
    /// a new document, whose hello clears this state.
    fn page_crashed(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        div()
            .flex_1()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(12.))
            .p(px(32.))
            .child(icon_img(
                &mut self.icons,
                Icon::TriangleAlert,
                false,
                theme.warning_base,
                28.,
            ))
            .child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(self.explore.page_crashed_title.clone()),
            )
            .child(
                div()
                    .max_w(px(360.))
                    .text_center()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(self.explore.page_crashed_body.clone()),
            )
            .child(
                outline_button(
                    ElementId::from("page-reload"),
                    theme,
                    &mut self.icons,
                    Some(Icon::RefreshCw),
                    self.explore.reload.clone(),
                )
                .on_click(cx.listener(|_, _: &gpui::ClickEvent, _, cx| {
                    #[cfg(not(target_os = "linux"))]
                    crate::webview::reload();
                    cx.notify();
                })),
            )
    }

    /// The address bar takes the keyboard: it starts from the page that is
    /// open, so editing a URL is editing THAT URL, not retyping it.
    fn edit_address(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.address_draft.is_none() {
            #[cfg(not(target_os = "linux"))]
            let current = if self.browsing {
                crate::webview::current_url().filter(|url| url != "about:blank")
            } else {
                None
            };
            #[cfg(target_os = "linux")]
            let current: Option<String> = None;
            let current = current.unwrap_or_default();
            // As a browser does: the URL taken into the field is selected, so
            // typing replaces it and a paste lands in its place.
            self.address_selected = !current.is_empty();
            self.address_draft = Some(current);
        }
        window.focus(&self.address_focus, cx);
        cx.notify();
    }

    /// One key in the address bar. Enter goes where the core's
    /// `browser_input` says the text means: a URL as typed, a bare host with
    /// a scheme, anything else a search — the same rule on every client.
    fn address_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let Some(draft) = self.address_draft.as_mut() else {
            return;
        };
        let ks = &event.keystroke;
        let selected = self.address_selected && !draft.is_empty();
        // The same four chords as every other well (`ui::edit_chord`): ⌘ on
        // macOS, Ctrl on Windows and Linux.
        if let Some(chord) = crate::ui::edit_chord(ks) {
            match chord {
                crate::ui::EditChord::SelectAll => self.address_selected = !draft.is_empty(),
                crate::ui::EditChord::Copy => {
                    if selected {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(draft.clone()));
                    }
                }
                crate::ui::EditChord::Cut => {
                    if selected {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(draft.clone()));
                        draft.clear();
                        self.address_selected = false;
                    }
                }
                crate::ui::EditChord::Paste => {
                    if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
                        if selected {
                            draft.clear();
                        }
                        // A pasted URL is one line; a newline in it is the end.
                        draft.push_str(text.lines().next().unwrap_or_default().trim());
                        self.address_selected = false;
                    }
                }
            }
            cx.notify();
            return;
        }
        match ks.key.as_str() {
            "enter" => {
                cx.stop_propagation();
                self.address_selected = false;
                let typed = self.address_draft.take().unwrap_or_default();
                if let Some(url) = vela_core::app::dapp_rpc::browser_input(&typed) {
                    self.open_typed_url(url, cx);
                }
            }
            "escape" => {
                // One layer only: the typing stops, the column stays.
                cx.stop_propagation();
                self.address_selected = false;
                self.address_draft = None;
            }
            "backspace" | "delete" if selected => {
                draft.clear();
                self.address_selected = false;
            }
            "backspace" => {
                draft.pop();
            }
            "left" | "right" | "home" | "end" => self.address_selected = false,
            _ if ks.modifiers.platform || ks.modifiers.control || ks.modifiers.alt => return,
            _ => match &ks.key_char {
                // A selection is replaced by whatever is typed over it.
                Some(ch) if !ch.chars().any(char::is_control) => {
                    if selected {
                        draft.clear();
                    }
                    draft.push_str(ch);
                    self.address_selected = false;
                }
                _ => return,
            },
        }
        cx.notify();
    }

    /// Open what the address bar resolved to, in the page on screen — as a
    /// favourite or a history row does.
    fn open_typed_url(&mut self, url: String, cx: &mut Context<Self>) {
        self.browsing = true;
        #[cfg(not(target_os = "linux"))]
        crate::webview::navigate(&url);
        self.browser_home = url;
        cx.notify();
    }

    /// Point the browser's strings, loads and titles at this page.
    ///
    /// Installed once, and re-installing is harmless — each sink replaces
    /// itself, which is what a page rebuilt after a route change needs.
    ///
    /// The hop is DEFERRED on purpose. wry calls its handlers from a platform
    /// callback, and `AsyncApp::update` borrows the app cell; doing that
    /// synchronously inside another borrow is a panic in a wallet. So each
    /// sink spawns onto the foreground executor and the work lands on the next
    /// run-loop turn — in the order the platform reported it, which is the
    /// order the core reads documents in (a load's commit, then the new
    /// document's hello).
    #[cfg(not(target_os = "linux"))]
    fn arm_dapp_requests(&mut self, cx: &mut Context<Self>) {
        if self.dapp_requests_armed {
            return;
        }
        self.dapp_requests_armed = true;
        let page = cx.entity().downgrade();
        let async_cx = cx.to_async();
        crate::webview::on_message_to(Box::new(move |message| {
            let page = page.clone();
            async_cx
                .spawn(async move |cx| {
                    page.update(cx, |page, cx| page.browser_message(message, cx))
                        .ok();
                })
                .detach();
        }));
        let page = cx.entity().downgrade();
        let async_cx = cx.to_async();
        crate::webview::on_load_to(Box::new(move |load| {
            let page = page.clone();
            async_cx
                .spawn(async move |cx| {
                    page.update(cx, |page, cx| page.browser_load(load, cx)).ok();
                })
                .detach();
        }));
        // What the page calls itself, for the history the start screen reads.
        // A visit is recorded when the DOCUMENT settles, not when a
        // navigation starts: a load that fails or is cancelled is not a place
        // anybody went, and the title only exists once the document parsed.
        let page = cx.entity().downgrade();
        let async_cx = cx.to_async();
        crate::webview::on_meta_to(Box::new(move |url, title, favicon| {
            let page = page.clone();
            async_cx
                .spawn(async move |cx| {
                    page.update(cx, |page, cx| {
                        page.browser_title = (!title.is_empty()).then(|| title.clone());
                        // The strip follows the document, from the one place
                        // that knows a page settled. A navigation with no tab
                        // OPENS one: the first page a person opens is the
                        // first tab they have, and a strip that stayed empty
                        // while a site was on screen would be lying about
                        // where they are.
                        let explore = resident::resident::<ExploreSites>(cx);
                        let selected = explore.read(cx).view().selected_tab;
                        explore.update(cx, |resident, cx| {
                            let event = match selected {
                                Some(id) => vela_core::app::explore_sites::Event::TabNavigated {
                                    id,
                                    url: url.clone(),
                                    title: (!title.is_empty()).then(|| title.clone()),
                                },
                                None => vela_core::app::explore_sites::Event::TabOpened {
                                    url: Some(url.clone()),
                                    title: (!title.is_empty()).then(|| title.clone()),
                                    now_ms: crate::executor::now_ms(),
                                },
                            };
                            resident.dispatch(event, cx);
                        });
                        resident::resident::<BrowserHistory>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::browser_history::Event::VisitRecorded {
                                    url,
                                    // Empty is ABSENT, not an empty title: the
                                    // core's rule is that a report without one
                                    // must not clobber a title already
                                    // captured, and "" would clobber it.
                                    title: (!title.is_empty()).then_some(title),
                                    favicon: (!favicon.is_empty()).then_some(favicon),
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                    })
                    .ok();
                })
                .detach();
        }));
    }

    /// One string from the page, for the browser machine — which decides
    /// everything about it: whether it is a hello or a request, which method,
    /// which answer, which document the answer is for.
    #[cfg(not(target_os = "linux"))]
    fn browser_message(&mut self, message: crate::webview::PageMessage, cx: &mut Context<Self>) {
        let host = self.browser_host(cx);
        host.update(cx, |host, cx| {
            host.page_message(
                message.sender,
                message.is_main_frame,
                message.message_json,
                cx,
            );
        });
    }

    /// A document load, for the browser machine. None of these settles
    /// anything by itself; the machine knows which document is gone.
    #[cfg(not(target_os = "linux"))]
    fn browser_load(&mut self, load: crate::webview::Load, cx: &mut Context<Self>) {
        let tab = BROWSER_TAB.to_owned();
        let event = match load {
            crate::webview::Load::Started(url) => DbrEvent::NavigationStarted { tab, url },
            crate::webview::Load::Finished(url) => DbrEvent::LoadFinished { tab, url },
            crate::webview::Load::Crashed => DbrEvent::RendererGone { tab },
        };
        let host = self.browser_host(cx);
        host.update(cx, |host, cx| host.dispatch(event, cx));
    }

    /// The browser machine, born the first time anything needs it, and the
    /// wiring that watches what it decides.
    fn browser_host(&mut self, cx: &mut Context<Self>) -> gpui::Entity<BrowserHost> {
        if let Some(host) = &self.browser_host {
            return host.clone();
        }
        let host = cx.new(BrowserHost::new);
        cx.observe(&host, Self::browser_host_changed).detach();
        self.browser_host = Some(host.clone());
        host
    }

    /// What the browser machine decided, acted on once.
    fn browser_host_changed(&mut self, host: gpui::Entity<BrowserHost>, cx: &mut Context<Self>) {
        let (orders, consent) = host.update(cx, |host, _| {
            (
                host.take_orders(),
                host.view
                    .consent
                    .as_ref()
                    .map(|consent| (consent.tab.clone(), consent.origin.clone())),
            )
        });
        // A NEW question has to be in front of somebody to be answered. Only
        // when it appears: the machine notifies on every read it answers, and
        // a panel that re-opened on each would be impossible to close.
        if consent.is_some() && consent != self.browser_consent {
            self.panel = PanelId::Connection;
        }
        self.browser_consent = consent;
        for order in orders {
            match order {
                SigningOrder::Forward(forwarded) => self.open_dapp_signing(forwarded, cx),
                SigningOrder::Cancel { id, .. } => self.cancel_dapp_signing(&id, cx),
            }
        }
        cx.notify();
    }

    /// The signing column's answer to a site, handed to the machine — which
    /// knows whether the document that asked is still there, delivers it
    /// exactly once, and opens the next request waiting in line.
    fn answer_dapp_signing(
        &mut self,
        tab: &str,
        id: String,
        payload: SignResponsePayload,
        user_op_hash: Option<String>,
        cx: &mut Context<Self>,
    ) {
        let host = self.browser_host(cx);
        host.update(cx, |host, cx| {
            host.dispatch(
                DbrEvent::SigningAnswered {
                    tab: tab.to_owned(),
                    id,
                    payload,
                    user_op_hash,
                },
                cx,
            );
        });
    }

    /// A request the browser machine forwarded. The four signing machines are
    /// born here — for the account the site was SHOWN.
    ///
    /// Pinned by construction: the column signs as the granted account itself,
    /// so a site granted account A can never be answered by B because B
    /// happened to be active (spec 070 defect 2).
    #[cfg(not(target_os = "linux"))]
    fn open_dapp_signing(&mut self, forwarded: Forwarded, cx: &mut Context<Self>) {
        // One sheet at a time is the core's rule, and it never forwards a
        // second request while it has one open. What it cannot see is the
        // wallet asking ITSELF (the Ethereum backup): that sheet is neither
        // replaced nor replacing, and the site hears -32002 — "busy, ask
        // again" — so the line moves on.
        if self.signing_busy(cx) {
            self.answer_dapp_signing(&forwarded.tab, forwarded.id, busy_answer(), None, cx);
            return;
        }
        let Some(account) = money::account_by_address(&forwarded.granted_address) else {
            // The account the site was granted is not in this wallet any more.
            // Nobody else signs in its place.
            self.answer_dapp_signing(
                &forwarded.tab,
                forwarded.id,
                SignResponsePayload::Err {
                    code: 4100,
                    kind: SignErrorKind::UnauthorizedAccount,
                    message: None,
                },
                None,
                cx,
            );
            return;
        };
        let request = crate::wallet::signing_host::IncomingRequest {
            id: forwarded.id,
            method: forwarded.method,
            params_json: forwarded.params_json,
            origin: forwarded.origin,
            transport_id: crate::wallet::signing_host::BROWSER_TRANSPORT.to_owned(),
            // The SITE's chain, per origin, as the machine keeps it.
            chain_id: forwarded.chain_id,
            granted_address: Some(forwarded.granted_address),
        };
        self.open_signing_request(&account, request, Some(forwarded.tab), cx);
    }

    /// No signing column on this platform, and no page to have asked. Refused
    /// rather than left hanging, should that ever change.
    #[cfg(target_os = "linux")]
    fn open_dapp_signing(&mut self, forwarded: Forwarded, cx: &mut Context<Self>) {
        self.answer_dapp_signing(&forwarded.tab, forwarded.id, busy_answer(), None, cx);
    }

    /// The page that asked is gone and already has its 4900: close its sheet,
    /// unanswered. A column showing something else is left alone.
    #[cfg(not(target_os = "linux"))]
    fn cancel_dapp_signing(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(host) = self.signing_host.clone() else {
            return;
        };
        let ours = {
            let host = host.read(cx);
            host.transport_id == crate::wallet::signing_host::BROWSER_TRANSPORT
                && host.request_id == id
        };
        if !ours {
            return;
        }
        host.update(cx, |host, cx| {
            host.dispatch_sign(
                vela_core::app::sign_request::Event::TransportDropped {
                    transport_id: crate::wallet::signing_host::BROWSER_TRANSPORT.to_owned(),
                },
                cx,
            );
        });
        // Gone whatever the machine did with it: a decoded intent must never
        // outlive the page that asked for it.
        self.signing_host = None;
        if self.panel == PanelId::Signing {
            self.panel = PanelId::None;
        }
        cx.notify();
    }

    #[cfg(target_os = "linux")]
    fn cancel_dapp_signing(&mut self, _id: &str, _cx: &mut Context<Self>) {}

    /// Is the column taken by a request that is still open? A column that
    /// has answered — showing a receipt, an error — is not: its request is
    /// over, and the next one may take its place.
    /// No signing column is open. Always so on Linux, which has none to open:
    /// `signing_host` exists only where the dApp browser does.
    fn no_signing_host(&self) -> bool {
        #[cfg(not(target_os = "linux"))]
        return self.signing_host.is_none();
        #[cfg(target_os = "linux")]
        return true;
    }

    #[cfg(not(target_os = "linux"))]
    fn signing_busy(&self, cx: &gpui::App) -> bool {
        self.signing_host.as_ref().is_some_and(|host| {
            let host = host.read(cx);
            !host.closed && !host.responded
        })
    }

    /// The four signing machines for one request — a page's, or the wallet's own.
    ///
    /// Never REPLACES a request that is still open. Until spec 070 this
    /// function swapped the column's host for the new one, which silently
    /// dropped whatever the first request was waiting on. A site's requests
    /// are serialised by the browser machine now; the wallet's own (the
    /// backup) finds the column busy and shows it instead — the row can be
    /// pressed again once the site's request is done. `tab` is the browser
    /// tab a site's request came from, `None` for the wallet's own.
    #[cfg(not(target_os = "linux"))]
    fn open_signing_request(
        &mut self,
        account: &vela_core::app::Account,
        request: crate::wallet::signing_host::IncomingRequest,
        tab: Option<String>,
        cx: &mut Context<Self>,
    ) {
        if self.signing_busy(cx) {
            self.panel = PanelId::Signing;
            cx.notify();
            return;
        }
        let window_handle = self.window_handle;
        let account = account.clone();
        let host = cx.new(|cx| {
            crate::wallet::signing_host::SigningHost::open(&account, request, window_handle, cx)
        });
        let observed = tab.clone();
        cx.observe(&host, move |page, host, cx| {
            page.signing_host_changed(&host, observed.as_deref(), cx);
        })
        .detach();
        self.signing_host = Some(host.clone());
        self.panel = PanelId::Signing;
        // A new request is what the column is about now.
        self.dapp_landing = None;
        // A new request opens closed: the last one's decision to look at the
        // bytes is not this one's.
        self.signing_advanced_open = false;
        crate::signing::components::reset_slide();
        // The same question, once, for a request the core answered before
        // any observation fires — a refusal on arrival.
        self.signing_host_changed(&host, tab.as_deref(), cx);
    }

    /// What a signing column said, acted on: a site's answer goes to the
    /// browser machine, and a column the core closed goes.
    #[cfg(not(target_os = "linux"))]
    fn signing_host_changed(
        &mut self,
        host: &gpui::Entity<crate::wallet::signing_host::SigningHost>,
        tab: Option<&str>,
        cx: &mut Context<Self>,
    ) {
        let answers = host.update(cx, |host, _| host.take_answers());
        let closed = host.read(cx).closed;
        // The transaction was handed to the tracker: it is landing HERE (the
        // web's `watchLanding`). Raised once per operation.
        if self.signing_host.as_ref() == Some(host)
            && let Some(handoff) = host.read(cx).view.tracker_handoff.clone()
            && self.dapp_landed_op.as_deref() != Some(handoff.user_op_hash.as_str())
        {
            self.dapp_landed_op = Some(handoff.user_op_hash.clone());
            self.dapp_landing = Some(DappLanding {
                op_hash: handoff.user_op_hash,
                chain_id: handoff.chain_id,
                raised_at_ms: crate::executor::now_ms(),
            });
        }
        // WHETHER there is a column is the core's answer, not this file's —
        // and only for the column on screen: a column already replaced (its
        // request over) must not close the one that took its place.
        if self.signing_host.as_ref() == Some(host) {
            if closed {
                // The request is over: the column goes, and so do its
                // machines — a decoded intent must never outlive the request
                // it decoded.
                self.signing_host = None;
                // …unless its transaction is landing: the column stays for
                // the receipt until the person is done with it.
                if self.panel == PanelId::Signing && self.dapp_landing.is_none() {
                    self.panel = PanelId::None;
                }
            } else {
                self.panel = PanelId::Signing;
            }
        }
        if let Some(tab) = tab {
            for answer in answers {
                self.answer_dapp_signing(tab, answer.id, answer.payload, answer.user_op_hash, cx);
            }
        }
        // Nothing after this may reopen the column. A merge (145de4f7) left
        // `signing_host = Some(host); panel = Signing` here — the old ending
        // of `open_signing_request` — which put back the host the branch
        // above had just dropped: every Close (the Ethereum backup's first)
        // reopened the column it closed.
        cx.notify();
    }

    /// DE1/DE2's start page.
    fn explore_start(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let mut column = div()
            .id("explore-start")
            .flex_1()
            .min_h(px(0.))
            .overflow_y_scroll()
            .p(px(32.))
            .flex()
            .flex_col();

        // The person's own grid once they are signed in; the drawn one before
        // that, which is what the gallery reviews. Same fork as Recent.
        let explore_view = resident::resident::<ExploreSites>(cx).read(cx).view();
        let live_grid = self.identity.is_some() && explore_view.ready;
        let favorites: Vec<explore_fixtures::SiteModel> = if live_grid {
            explore_view
                .favorites
                .iter()
                .map(explore_live::tile_of)
                .collect()
        } else {
            explore_fixtures::favorites()
        };
        // What each tile opens and what its menu acts on — the ORIGIN, which
        // is the site's identity, kept beside the row so a click and a
        // right-click cannot disagree about which site they mean.
        let origins: Vec<(String, String)> = if live_grid {
            explore_view
                .favorites
                .iter()
                .map(|site| (site.origin.clone(), site.url.clone()))
                .collect()
        } else {
            favorites
                .iter()
                .map(|site| {
                    let url = format!("https://{}", site.host);
                    (url.clone(), url)
                })
                .collect()
        };
        column = column.child(section_header(
            theme,
            &mut self.icons,
            self.explore.favorites.clone(),
            self.explore.edit.clone(),
        ));

        let mut grid = div().flex().flex_wrap().gap(px(12.)).py(px(12.));
        for (i, site) in favorites.iter().enumerate() {
            grid = grid.child(
                explore_components::site_tile(ElementId::from(("tile", i)), theme, site)
                    .on_click({
                        // A favourite opens THAT site, at the url it was
                        // pinned at — which is deeper than the origin when
                        // somebody pinned the page they actually work on.
                        let url = origins
                            .get(i)
                            .map(|(_, url)| url.clone())
                            .unwrap_or_default();
                        cx.listener(move |this, _, _, cx| {
                            this.browsing = true;
                            this.browser_home = url.clone();
                            #[cfg(not(target_os = "linux"))]
                            crate::webview::navigate(&url);
                            cx.notify();
                        })
                    })
                    .on_mouse_down(MouseButton::Right, {
                        // The menu acts on the site it was opened over.
                        let origin = origins.get(i).map(|(origin, _)| origin.clone());
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.menu_origin = origin.clone();
                            this.menu = Some((ContactsMenu::Tile, event.position, Anchor::TopLeft));
                            cx.notify();
                        })
                    }),
            );
        }
        // The "add" affordance is left OUT when the grid is full rather than
        // drawn and refusing — the core says which, and a control that cannot
        // work is worse than an absent one.
        if !(live_grid && explore_view.favorites_full) {
            grid = grid.child(
                explore_components::add_tile(
                    ElementId::from("tile-add"),
                    theme,
                    &mut self.icons,
                    self.explore.add.clone(),
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    this.explore_form = Some(ExploreForm {
                        ask: ExploreAsk::NewFavorite,
                        text: String::new(),
                    });
                    window.focus(&this.explore_form_focus, cx);
                    cx.notify();
                })),
            );
        }
        column = column.child(grid);

        // Recent is the core's; everything below it is still drawn, because
        // nothing in `vela-core` owns favourites or custom groups yet. The
        // drawn Recent group is DROPPED when the live one exists rather than
        // shown beside it — two "Recent" headings, one of them invented, is
        // the fixture leaking that phases 22, 26 and 27 each had to close.
        let history = resident::resident::<BrowserHistory>(cx).read(cx).view();
        let live_recent = explore_live::recent_group(&history.entries, &self.explore);
        // …and so are the person's own groups. The drawn ones (交易 / 预测市场)
        // are mock CONTENT, not chrome: they go the moment there is a real
        // book to show, exactly as the drawn Recent does.
        let live_groups = if live_grid {
            explore_live::custom_groups(&explore_view)
        } else {
            Vec::new()
        };
        let groups = explore_fixtures::groups(&self.explore)
            .into_iter()
            .filter(|group| !(group.id == "recent" && self.identity.is_some()))
            .filter(|group| !(live_grid && group.id != "recent"))
            .chain(live_groups)
            .collect::<Vec<_>>();
        for (group_index, group) in live_recent.into_iter().chain(groups).enumerate() {
            let action = match group.action {
                explore_fixtures::GroupAction::Clear => self.explore.clear.clone(),
                explore_fixtures::GroupAction::Edit => self.explore.edit.clone(),
                explore_fixtures::GroupAction::Menu => SharedString::from("⋯"),
            };
            // The Clear on the live Recent heading clears the core's history.
            // The drawn groups' actions stay inert: there is no machine behind
            // a custom group, and a button that looked identical but deleted
            // nothing would be the worse of the two lies.
            let clearable = group.id == "recent" && matches!(group.action, GroupAction::Clear);
            let (title_half, action_half) =
                section_header_parts(theme, &mut self.icons, group.title.clone(), action);
            let action_half = if clearable {
                action_half
                    .id("recent-clear")
                    .cursor_pointer()
                    .on_click(cx.listener(|_, _: &gpui::ClickEvent, _, cx| {
                        resident::resident::<BrowserHistory>(cx).update(cx, |resident, cx| {
                            resident.dispatch(vela_core::app::browser_history::Event::ClearAll, cx);
                        });
                        cx.notify();
                    }))
                    .into_any_element()
            } else {
                action_half.into_any_element()
            };
            column = column.child(section_header_row().child(title_half).child(action_half));
            let mut rows = div().flex().flex_col();
            for (i, site) in group.sites.iter().enumerate() {
                rows = rows.child(
                    explore_components::site_row(
                        // By the group's place on the page, not its kind: every
                        // custom group is "custom", and row 0 of each one
                        // shared an id — gpui then treats them as one element
                        // (078 E-02).
                        ElementId::NamedInteger(
                            SharedString::from(format!("explore-group-{group_index}")),
                            i as u64,
                        ),
                        theme,
                        &mut self.identicons,
                        site,
                    )
                    .on_mouse_down(MouseButton::Right, {
                        // The row menu is armed only where a machine is behind
                        // it — the live Recent group. A drawn group's rows
                        // stay inert rather than opening a menu whose Delete
                        // has nothing to delete.
                        let origin = explore_live::live_origin(&history.entries, &site.host);
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            if let Some(origin) = origin.clone() {
                                this.menu_origin = Some(origin);
                                this.menu =
                                    Some((ContactsMenu::Recent, event.position, Anchor::TopLeft));
                                cx.notify();
                            }
                        })
                    })
                    .on_click({
                        // The row's OWN site (078 E-02). A Recent row opens
                        // where the person left off, verbatim — that is what
                        // the core stores the whole URL for; a custom group's
                        // row opens the url its site was pinned at. It used to
                        // look every row up in the HISTORY by host, and a
                        // pinned site nobody had visited yet found nothing —
                        // yet the column still switched to the browser, onto
                        // whatever page was loaded last.
                        let url = site.open_url();
                        cx.listener(move |this, _, _, cx| {
                            this.browser_home = url.clone();
                            #[cfg(not(target_os = "linux"))]
                            crate::webview::navigate(&url);
                            this.browsing = true;
                            cx.notify();
                        })
                    }),
                );
            }
            column = column.child(rows);
        }

        column
    }

    /// The question a site is asking, and the two answers.
    ///
    /// The origin is drawn from the CORE's consent view, which got it from
    /// the transport — never from anything the page said about itself. That
    /// is the whole point of the row: it is the fact the person is being
    /// asked to judge.
    fn consent_body(
        &mut self,
        theme: &Theme,
        consent: &vela_core::app::dapp_browser::DbrConsentView,
        cx: &mut Context<Self>,
    ) -> Div {
        // The same derivation the signing header uses. Two ways of turning an
        // origin into a name is two answers to "who is asking", and the sheet
        // and this panel are asking about the same site.
        let (host, _, letter) = signing_live::dapp_identity(&consent.origin);
        let title = crate::signing::fill(&self.explore.consent_title, &[("host", &host)]);
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(explore_components::letter_avatar(letter, theme.accent, 40.))
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(theme.fg_base)
                            .child(SharedString::from(title)),
                    ),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(self.explore.consent_body.clone()),
            )
            .child(
                div()
                    .id("consent-approve")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        let host = this.browser_host(cx);
                        host.update(cx, |host, cx| {
                            host.dispatch(
                                DbrEvent::ConsentApproved {
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                    }))
                    .child(crate::flows::components::accent_button(
                        theme,
                        self.explore.consent_connect.clone(),
                    )),
            )
            .child(
                outline_button(
                    ElementId::from("consent-reject"),
                    theme,
                    &mut self.icons,
                    None,
                    self.explore.consent_cancel.clone(),
                )
                .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let host = this.browser_host(cx);
                    host.update(cx, |host, cx| host.dispatch(DbrEvent::ConsentRejected, cx));
                })),
            )
    }

    /// DE3's third column — what a connected site can and cannot do.
    ///
    /// One panel, two moments: the site ASKING to connect, and the site that
    /// already is. The consent comes first because the core only ever offers
    /// it when the answer is still open, and because there is no bottom sheet
    /// on this shell to put it in — the founder's ruling, and the third
    /// column is where a decision about the browser belongs anyway.
    fn connection_body(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        if let Some(consent) = self
            .browser_host
            .as_ref()
            .and_then(|host| host.read(cx).view.consent.clone())
        {
            return self.consent_body(theme, &consent, cx);
        }
        // WHICH site, whether it is connected, and which chain it is on, from
        // the browser machine's view of the page on screen. The mock's
        // `app.uniswap.org · Connected` under a live browser is the phase-22
        // defect wearing a different hat: this panel is a statement about a
        // specific site's access to this wallet, and naming the wrong one is
        // the only thing it can get seriously wrong.
        let mock = explore_fixtures::uniswap();
        let live = self
            .identity
            .as_ref()
            .filter(|_| self.browsing)
            .and(self.browser_host.as_ref())
            .and_then(|host| host.read(cx).tab().cloned());
        let (site_host, site_letter, site_tint, connected, secure, chain_id, origin) = match &live {
            Some(tab) => {
                let (host, _, letter) =
                    signing_live::dapp_identity(tab.origin.as_deref().unwrap_or_default());
                (
                    host,
                    letter,
                    theme.accent,
                    tab.connected_address.is_some(),
                    tab.secure,
                    tab.chain_id,
                    tab.origin.clone(),
                )
            }
            // The drawing's chain: Gnosis, as the mock always showed it.
            None => (
                mock.host.clone(),
                mock.letter.clone(),
                mock.tint,
                true,
                true,
                100,
                None,
            ),
        };
        // "Secure" and "Connected" are claims, so each is only made when the
        // core says so: a lock for the origin, a grant for the site.
        let status = match (secure, connected) {
            (true, true) => SharedString::from(format!(
                "{} · {}",
                self.explore.secure_site, self.explore.connected_tag
            )),
            (true, false) => self.explore.secure_site.clone(),
            (false, true) => self.explore.connected_tag.clone(),
            (false, false) => SharedString::default(),
        };
        let identity = self.identity();
        let e = &self.explore;
        // The site's own network, which the person can change here — the
        // site hears `chainChanged`, and every other site stays where it is.
        let network_row = div()
            .id("site-network")
            .flex()
            .items_center()
            .justify_between()
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(self.explore.network.clone()),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(
                        div()
                            .w(px(8.))
                            .h(px(8.))
                            .rounded_full()
                            .bg(fixtures::chain_ethereum()),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            // The chain THIS SITE is on — the one its
                            // `eth_chainId` answers and a signature from it
                            // is quoted, routed and submitted on. Not a
                            // column-wide chain: there is none any more.
                            .child(SharedString::from(crate::flows::live::chain_name(chain_id))),
                    )
                    .when(origin.is_some(), |row| {
                        row.child(icon_img(
                            &mut self.icons,
                            Icon::ChevronDown,
                            false,
                            theme.fg_muted,
                            14.,
                        ))
                    }),
            );
        let network_row = match origin.clone() {
            Some(origin) => network_row.cursor_pointer().on_click(cx.listener(
                move |this, event: &gpui::ClickEvent, _, cx| {
                    this.open_site_networks(origin.clone(), chain_id, event.position(), cx);
                },
            )),
            None => network_row,
        };
        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(explore_components::letter_avatar(
                        site_letter,
                        site_tint,
                        40.,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.fg_base)
                                    .child(site_host),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(if connected {
                                        theme.success_base
                                    } else {
                                        theme.fg_muted
                                    })
                                    .child(status),
                            ),
                    ),
            )
            .child(row_divider(theme))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .child(identicon_avatar(
                        &mut self.identicons,
                        &identity.address,
                        40.,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .flex_1()
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.fg_base)
                                    .child(identity.name.clone()),
                            )
                            .child(
                                div()
                                    .font_family("monospace")
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_muted)
                                    .child(identity.display()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(e.switch_account.clone()),
                    ),
            )
            .child(row_divider(theme))
            .child(network_row)
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(self.explore.connection_explainer.clone()),
            )
            .child({
                let button = outline_button(
                    ElementId::from("disconnect"),
                    theme,
                    &mut self.icons,
                    None,
                    self.explore.disconnect.clone(),
                );
                // The site on screen, by name: the core revokes the grant and
                // tells every open page of that site (`accountsChanged []` and
                // `disconnect`). Nothing to revoke, nothing armed.
                match origin.filter(|_| connected) {
                    Some(origin) => {
                        button.on_click(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.revoke_site(origin.clone(), cx);
                        }))
                    }
                    None => button,
                }
            })
            .child(
                div()
                    .text_center()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(self.explore.auto_request_hint.clone()),
            )
    }

    /// The networks a site can be put on, as a menu under the network row.
    ///
    /// The list is the wallet's own (built-ins plus what the person added),
    /// named once here rather than on every frame the menu is open. The
    /// machine is told the list first, so a network added in Settings a
    /// moment ago is one it accepts.
    fn open_site_networks(
        &mut self,
        origin: String,
        current: u32,
        position: gpui::Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let host = self.browser_host(cx);
        host.update(cx, |host, cx| host.follow_networks(cx));
        self.site_networks = crate::wallet::signing_host::known_chain_ids()
            .into_iter()
            .map(|chain_id| {
                (
                    chain_id,
                    SharedString::from(crate::flows::live::chain_name(chain_id)),
                    chain_id == current,
                )
            })
            .collect();
        self.menu_origin = Some(origin);
        self.menu = Some((ContactsMenu::SiteNetwork, position, Anchor::TopRight));
        cx.notify();
    }

    /// Disconnect one site, from wherever the person asked — after asking
    /// them (spec 072): the grant goes, and every open page of that site
    /// hears `accountsChanged []` and `disconnect`.
    fn revoke_site(&mut self, origin: String, cx: &mut Context<Self>) {
        let name = signing_live::dapp_identity(&origin).0;
        self.confirm = Some(Confirm::Disconnect { origin, name });
        cx.notify();
    }

    /// DE4 / DCS1–8's third column — the signing request itself.
    /// The request, as text. Empty when there is no live request — the drawn
    /// boards have no payload of their own, and inventing one would put bytes
    /// on screen that nobody is being asked to sign.
    fn signing_raw_rows(&self, cx: &mut Context<Self>) -> Vec<(SharedString, SharedString)> {
        // Linux has no in-app browser, so no dApp request ever reaches this
        // shell there and `signing_host` does not exist — the same cut
        // `connection_body` makes a few hundred lines down.
        #[cfg(target_os = "linux")]
        {
            let _ = cx;
            return Vec::new();
        }
        #[cfg(not(target_os = "linux"))]
        {
            let Some(host) = self.signing_host.as_ref() else {
                return Vec::new();
            };
            let host = host.read(cx);
            let (method, params) = host.raw.clone();
            let mut rows = vec![(
                self.signing.tech_function.clone(),
                SharedString::from(method.clone()),
            )];
            match crate::executor::sign_request::calls_of(&method, &params) {
                // A transaction: each leg's destination and its calldata, which is
                // what a person compares against the summary above.
                Some(calls) => {
                    for (i, call) in calls.iter().enumerate() {
                        let label = if calls.len() > 1 {
                            SharedString::from(format!(
                                "{} {}",
                                self.signing.label_interacting,
                                i + 1
                            ))
                        } else {
                            self.signing.label_interacting.clone()
                        };
                        rows.push((label, SharedString::from(call.to.clone())));
                        rows.push((
                            self.signing.tech_raw_data.clone(),
                            SharedString::from(call.data.clone()),
                        ));
                    }
                }
                // A message or typed data: the payload itself.
                None => rows.push((
                    self.signing.tech_raw_data.clone(),
                    SharedString::from(params),
                )),
            }
            rows
        }
    }

    fn signing_body(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        // What currency every `≈` figure below is drawn in.
        let currency = self.money(cx);
        // The live sheet when a request is open, the mock otherwise — the same
        // fork every other surface takes, and what keeps the 33 drawn
        // scenarios reviewable after real requests arrive.
        let mut model = signing_fixtures::build(self.signing_state, &self.signing);
        // Which of the two things this column is: the request, or the gas
        // account it cannot pay from.
        let mut funding = false;
        // Spec 081: the core refused the request outright.
        let mut refused = false;
        // The speed control under the fee (spec 069) — the send form's own,
        // and the tiers its options pick, in order.
        let mut signing_speed: Option<flow_fixtures::FeeSpeedModel> = None;
        let mut speed_tiers: Vec<vela_core::app::fee_policy::FeeTier> = Vec::new();
        #[cfg(not(target_os = "linux"))]
        if let Some(host) = self.signing_host.as_ref() {
            let host = host.read(cx);
            let fee = host.fee_view();
            // The gas account cannot pay: the sheet SWAPS to the top-up and
            // shows nothing else. Not stacked, not appended — the core calls
            // this surface "the in-sheet funding swap (BUG-1: never a stacked
            // second modal)", and a request drawn under a top-up prompt is a
            // person deciding two things at once.
            if host.view.surface == vela_core::app::sign_request::SignSurface::Funding {
                model.blocks = signing_live::funding_blocks(&host.view, &self.signing);
                model.confirm_label = self.signing.funding_check_now.clone();
                // Armed on its own terms: this slide is not a signature, it is
                // "I have sent it, look again". The three-machine AND governs
                // signing, and applying it here would leave the only way out
                // of a top-up shut.
                model.confirm_enabled = true;
                funding = true;
                // The header and the fee card belong to the request, not to
                // the top-up: the person is being asked for one thing here.
                model.fee = signing_fixtures::FeeModel::Hidden;
            } else if host.view.blocked.is_some() {
                // Spec 081: the core refused this request — it would have
                // changed who controls the account. The refusal is the whole
                // sheet. The decoded body, the simulation and the cap editor
                // all describe a transaction that will never be signed, and
                // reading them invites the question "so why can't I?", which
                // the refusal already answers.
                model.blocks = signing_live::status_blocks(&host.view, &self.signing);
                refused = true;
                model.fee = signing_fixtures::FeeModel::Hidden;
            } else {
                // ALWAYS the core's, never "the core's if it has any". The old
                // `if !blocks.is_empty()` left the GALLERY's blocks under a live
                // header for every surface the live builder had nothing for —
                // which was four of the core's six, resolution included. A drawn
                // swap under a true header is the worst thing this column can say.
                model.blocks = signing_live::blocks(&host.clear_view, &host.facts, &self.signing);
                // What the chain says it would MOVE, under what the site says
                // it would do. Last, because it is the answer to everything
                // above it — and the one part of this sheet a site cannot
                // write.
                model.blocks.extend(signing_live::sim_blocks(
                    &host.sim,
                    host.sim_unavailable,
                    host.chain_id,
                    &self.signing,
                ));
                // …and what the pipeline is doing, under it. Appended rather than
                // mixed in: what this request IS comes first, what the wallet is
                // doing about it second.
                // The cap editor, from the guard. Placed before the pipeline's
                // status so the decision comes above what the wallet is doing
                // about it — and drawn at all only on the surface the core calls
                // the editor, never over a permit (which cannot be capped) or a
                // batch (whose per-leg editors are still owed).
                if let Some((editor, _)) =
                    signing_live::guard_editor(&host.guard_view, &self.signing)
                {
                    model.blocks.push(editor);
                }
                model
                    .blocks
                    .extend(signing_live::status_blocks(&host.view, &self.signing));
            }
            // WHO is asking, from the request. The mock's Uniswap header on a
            // live request is the one fact the person is judging, wrong.
            let (name, dapp_host, letter) = signing_live::dapp_identity(&host.origin);
            model.dapp_name = name;
            model.dapp_host = dapp_host;
            model.dapp_letter = letter;
            // The wallet's own request (the key backup) is not a site: its own
            // mark and name, and no host. A site gets its own icon over its
            // initial — https only, never over a channel anybody could answer on.
            let own = host.transport_id == crate::wallet::signing_host::WALLET_TRANSPORT;
            model.dapp_own = own;
            if own {
                model.dapp_name = gpui::SharedString::from("Vela Wallet");
                model.dapp_host = gpui::SharedString::default();
                // Matched on the VERIFIED registry address, never on "it is ours"
                // alone and never on the English words: the first look at this
                // column headed a plain self-transfer "备份公钥".
                let is_backup = host.clear_view.result.as_ref().is_some_and(|result| {
                    result.verified
                        && result.contract_address.as_deref().is_some_and(|address| {
                            address.eq_ignore_ascii_case(vela_core::registry_backup::REGISTRY)
                        })
                });
                // …and its built-in lines, in the person's language. The core's
                // are English, like the descriptors beside them; this one is
                // OURS. First-party + the intent block it opens with.
                if is_backup
                    && let Some(signing_fixtures::Block::Intent { text, .. }) =
                        model.blocks.first_mut()
                {
                    *text = self.signing.backup_intent.clone();
                }
                let mut relabelled = 0;
                for block in model.blocks.iter_mut().filter(|_| is_backup) {
                    if let signing_fixtures::Block::Rows(rows) = block {
                        for row in rows.iter_mut() {
                            if let Some(label) = self.signing.backup_labels.get(relabelled) {
                                row.0 = label.clone();
                                relabelled += 1;
                            }
                        }
                    }
                }
            } else if let Some(base) = host.origin.strip_prefix("https://") {
                let base = base.split('/').next().unwrap_or_default();
                if !base.is_empty() {
                    model.dapp_icon_urls = vec![
                        gpui::SharedString::from(format!("https://{base}/apple-touch-icon.png")),
                        gpui::SharedString::from(format!("https://{base}/favicon.ico")),
                    ];
                }
            }
            model.network_logo = crate::marks::chain_logo_url(host.chain_id);
            // …and on which chain, from the request too. The fixture's badge
            // said Ethereum over a Gnosis fee.
            model.network_name =
                gpui::SharedString::from(crate::flows::live::chain_name(host.chain_id));
            let speed_tier = Some(host.speed_view().tier);
            if !refused {
                model.fee = signing_live::fee_model(
                    &host.clear_view,
                    fee,
                    host.chain_id,
                    host.fee_open,
                    &self.signing,
                    &self.locale,
                    speed_tier,
                    &currency,
                );
                model.confirm_label = signing_live::confirm_label(&host.clear_view, &self.signing);
                model.confirm_enabled = signing_live::confirm_enabled(
                    &host.view,
                    &host.guard_view,
                    &host.clear_view,
                    fee,
                    speed_tier,
                );
            } else {
                // No confirm control at all. It is not disabled — it is
                // absent, because the wallet never offered it.
                model.confirm_label = gpui::SharedString::default();
                model.confirm_enabled = false;
            }
            if !funding && !refused && !signing_live::off_chain(&host.clear_view) {
                speed_tiers = host
                    .speed_view()
                    .options
                    .iter()
                    .map(|option| option.tier)
                    .collect();
                signing_speed = Some(flows_live::speed_model(
                    &flows_live::SpeedInputs {
                        view: host.speed_view().clone(),
                        tier_views: host.speed_tier_views(),
                    },
                    &self.flow_strings,
                    None,
                    fee,
                    &self.locale,
                    &currency,
                ));
            }
        }
        #[cfg(target_os = "linux")]
        let _ = cx;

        // Passed ONLY when the three machines agreed. A shut slide that still
        // carried an action would be a control the core said no to, waiting
        // for a click to say yes.
        #[cfg(not(target_os = "linux"))]
        let confirm_action: Option<panels::Click> =
            (model.confirm_enabled && self.signing_host.is_some()).then(|| {
                Box::new(cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                    if let Some(host) = page.signing_host.as_ref() {
                        host.update(cx, |host, cx| {
                            if funding {
                                // "I have topped it up" — the core re-runs the
                                // pre-check with the opts it saved, so the
                                // request resumes rather than starting over.
                                host.dispatch_sign(
                                    vela_core::app::sign_request::Event::FundingCompleteTapped,
                                    cx,
                                );
                            } else {
                                host.approve(cx);
                            }
                        });
                    }
                })) as panels::Click
            });
        #[cfg(target_os = "linux")]
        let confirm_action: Option<panels::Click> = None;
        let mut column = div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(signing_components::header(theme, &model));

        // What is typed in the cap field, from the core. Read before the
        // block loop borrows `self`.
        #[cfg(not(target_os = "linux"))]
        let cap_text: String = self
            .signing_host
            .as_ref()
            .and_then(|host| host.read(cx).guard_view.editor.clone())
            .map(|editor| editor.custom_text)
            .unwrap_or_default();
        #[cfg(target_os = "linux")]
        let cap_text = String::new();

        // The allowance chips are a control when a machine is behind them.
        // One dispatch per chip, in the order the block lists them; the mock
        // gets none and draws exactly what the gallery has always drawn.
        #[cfg(not(target_os = "linux"))]
        let chip_modes: Vec<vela_core::app::approval_guard::GuardEditorMode> = self
            .signing_host
            .as_ref()
            .and_then(|host| signing_live::guard_editor(&host.read(cx).guard_view, &self.signing))
            .map(|(_, modes)| modes)
            .unwrap_or_default();
        #[cfg(target_os = "linux")]
        let chip_modes: Vec<vela_core::app::approval_guard::GuardEditorMode> = Vec::new();

        for item in &model.blocks {
            let armed =
                matches!(item, signing_fixtures::Block::Allowance { .. }) && !chip_modes.is_empty();
            if armed {
                // The cap field, live: the value is the CORE's `custom_text`,
                // so a keystroke it rejected never appears as though it had
                // been taken, and every keystroke goes back to the machine
                // that validates it.
                // Same platform pair as `cap_text` and `chip_modes` above: on
                // Linux there is no signing host, so there is nothing to type
                // into — the block draws the way the gallery draws it, without
                // a field that would swallow keystrokes.
                #[cfg(target_os = "linux")]
                let field: Option<panels::AddressField> = None;
                #[cfg(not(target_os = "linux"))]
                let field = matches!(
                    item,
                    signing_fixtures::Block::Allowance {
                        custom: Some(_),
                        ..
                    }
                )
                .then(|| {
                    let host = self.signing_host.clone();
                    let previous = cap_text.clone();
                    panels::AddressField {
                        focus: self.cap_focus.clone(),
                        value: cap_text.clone(),
                        placeholder: SharedString::from("0"),
                        on_change: Box::new(
                            move |text: String, _: &mut Window, cx: &mut gpui::App| {
                                // Spec 073: the cap's parser drops every
                                // comma, so a raw "4,5" allowed 45.
                                let Some(text) = flows_live::amount_edited(&text, &previous) else {
                                    return;
                                };
                                if let Some(host) = host.as_ref() {
                                    host.update(cx, |host, cx| {
                                        host.dispatch_guard(
                                        vela_core::app::approval_guard::Event::CustomAmountChanged {
                                            text,
                                        },
                                        cx,
                                    );
                                    });
                                }
                            },
                        ),
                    }
                });
                let actions = chip_modes
                    .iter()
                    .map(|mode| {
                        let mode = *mode;
                        Some(
                            Box::new(cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                                #[cfg(not(target_os = "linux"))]
                                if let Some(host) = page.signing_host.as_ref() {
                                    host.update(cx, |host, cx| {
                                        host.dispatch_guard(
                                            vela_core::app::approval_guard::Event::PresetSelected {
                                                mode,
                                            },
                                            cx,
                                        );
                                    });
                                }
                                cx.notify();
                            })) as crate::flows::panels::Click,
                        )
                    })
                    .collect();
                column = column.child(signing_components::block_with_actions(
                    theme,
                    &mut self.icons,
                    item,
                    actions,
                    field,
                    window,
                ));
            } else {
                column = column.child(signing_components::block(theme, &mut self.icons, item));
            }
        }

        // The disclosure. It was a chevron and a sentence with no listener —
        // the only route from the decoded summary to what is actually being
        // signed, and it did not go there. What it opens is the REQUEST, not a
        // second decoding of it: the method, and for a transaction each call's
        // destination and calldata; for a message or typed data, the payload.
        // A person checking a summary against the bytes needs the bytes.
        let open = self.signing_advanced_open && !refused;
        // A refused request shows no bytes (the web's `live.ts` hides the
        // request data when blocked): they describe a transaction that will
        // never be signed, and reading them only invites "so why can't I?".
        let raw_rows = if refused {
            Vec::new()
        } else {
            self.signing_raw_rows(cx)
        };
        column = column
            .when(!refused, |column| column.child(row_divider(theme)))
            .when(!refused, |column| {
                column.child(
                    div()
                        .id("signing-advanced")
                        .py(px(10.))
                        .flex()
                        .items_center()
                        .gap(px(8.))
                        .when(!raw_rows.is_empty(), |el| {
                            el.cursor_pointer().on_click(cx.listener(|this, _, _, cx| {
                                this.signing_advanced_open = !this.signing_advanced_open;
                                cx.notify();
                            }))
                        })
                        .child(icon_img(
                            &mut self.icons,
                            if open {
                                Icon::ChevronDown
                            } else {
                                Icon::ChevronRight
                            },
                            false,
                            theme.fg_muted,
                            12.,
                        ))
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_muted)
                                .child(self.signing.advanced_toggle.clone()),
                        ),
                )
            });
        if open {
            for (label, value) in raw_rows {
                column = column.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(2.))
                        .pb(px(10.))
                        .child(
                            div()
                                .text_size(theme::text_label())
                                .text_color(theme.fg_subtle)
                                .child(label),
                        )
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_label())
                                .text_color(theme.fg_base)
                                .child(value),
                        ),
                );
            }
        }
        let (on_fee, on_fee_pick) = self.fee_actions(cx);
        if let Some(fee) =
            signing_components::fee(theme, &mut self.icons, &model.fee, on_fee, on_fee_pick)
        {
            let mut fee_block = div().flex().flex_col().gap(px(4.)).child(fee);
            if let Some(speed) = &signing_speed {
                // The same control, the same clicks, as the send form's.
                #[cfg(not(target_os = "linux"))]
                let toggle: Option<panels::Click> = Some(Box::new(cx.listener(
                    |page, _: &gpui::ClickEvent, _, cx| {
                        if let Some(host) = page.signing_host.as_ref() {
                            host.update(cx, |host, cx| host.toggle_speed(cx));
                        }
                    },
                )));
                #[cfg(not(target_os = "linux"))]
                let picks: Vec<panels::Click> = speed_tiers
                    .iter()
                    .map(|tier| {
                        let tier = *tier;
                        Box::new(cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                            if let Some(host) = page.signing_host.as_ref() {
                                host.update(cx, |host, cx| host.pick_speed(tier, cx));
                            }
                        })) as panels::Click
                    })
                    .collect();
                #[cfg(target_os = "linux")]
                let (toggle, picks): (Option<panels::Click>, Vec<panels::Click>) =
                    (None, Vec::new());
                fee_block = fee_block.child(panels::speed_control(
                    theme,
                    &mut self.icons,
                    speed,
                    toggle,
                    picks,
                ));
            }
            column = column.child(fee_block);
        }
        column = column
            .child(signing_components::signer_row(
                theme,
                &mut self.identicons,
                model.signer_label.clone(),
                model.signer_name.clone(),
                &model.signer_seed,
            ))
            .children(self.sign_with_row(theme, cx))
            .children((!model.confirm_label.is_empty()).then(|| {
                signing_components::slide_to_confirm(
                    theme,
                    &mut self.icons,
                    model.confirm_label.clone(),
                    model.confirm_enabled,
                    confirm_action,
                )
            }));
        // A refused request's one way out (the web's `dismissOnly`): Close,
        // full width, the same dismissal as the ✕. It had none — the confirm
        // is absent, rightly, and nothing stood in its place.
        if refused {
            column = column.child(
                div()
                    .id("signing-refused-close")
                    .cursor_pointer()
                    .child(crate::flows::components::ghost_button(
                        theme,
                        self.signing.close.clone(),
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        #[cfg(not(target_os = "linux"))]
                        if let Some(host) = this.signing_host.clone() {
                            host.update(cx, |host, cx| {
                                host.dispatch_sign(
                                    vela_core::app::sign_request::Event::SwipeDismissed,
                                    cx,
                                );
                            });
                        }
                        this.panel = PanelId::None;
                        cx.notify();
                    })),
            );
        }
        column
    }

    /// "Sign with · Automatic ⌄" — WHERE the passkey that signs this request
    /// is (founder, 2026-09-19: creating and signing in let a person choose;
    /// signing took the first key's stored route), or the Trusted Signer's page
    /// (spec 071). Per request — the host lives for one, and starts at the
    /// default Settings keeps. Which key the choice pins is the core's
    /// (`sign_route`). Opens in place: a dialog over the signing column is a
    /// modal under a modal.
    #[cfg(not(target_os = "linux"))]
    fn sign_with_row(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<Div> {
        let (method, open) = {
            let host = self.signing_host.as_ref()?.read(cx);
            (host.sign_method.clone(), host.sign_with_open)
        };
        let options = self.signing.sign_with_options.clone();
        let value = options
            .iter()
            .find(|(id, _)| *id == method)
            .map_or_else(|| options[0].1.clone(), |(_, title)| title.clone());
        fn pick(
            id: Option<&'static str>,
            cx: &mut Context<WalletPage>,
        ) -> impl Fn(&gpui::ClickEvent, &mut Window, &mut gpui::App) + 'static {
            cx.listener(move |page, _: &gpui::ClickEvent, _, cx| {
                if let Some(host) = page.signing_host.clone() {
                    host.update(cx, |host, cx| {
                        host.sign_with(id, cx);
                        cx.notify();
                    });
                }
                cx.notify();
            })
        }
        let mut block = div().flex().flex_col().gap(px(8.)).child(
            div()
                .id("signing-sign-with")
                .flex()
                .items_center()
                .gap(px(8.))
                .cursor_pointer()
                .on_click(pick(None, cx))
                .child(
                    div()
                        .flex_1()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(self.signing.sign_with.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(value),
                )
                .child(icon_img(
                    &mut self.icons,
                    if open {
                        Icon::ChevronUp
                    } else {
                        Icon::ChevronDown
                    },
                    false,
                    theme.fg_muted,
                    14.,
                )),
        );
        if open {
            let mut list = div()
                .flex()
                .flex_col()
                .p(px(4.))
                .rounded(px(12.))
                .bg(theme.bg_sunken);
            for (index, (id, title)) in options.into_iter().enumerate() {
                let selected = id == method;
                let mut words = div().flex_1().flex().flex_col().gap(px(2.)).child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(if selected {
                            theme.fg_base
                        } else {
                            theme.fg_muted
                        })
                        .child(title),
                );
                // The one choice that is not a place a passkey is says what
                // it is — the create flow's lines only describe making a key.
                if id == vela_core::trusted_signer::METHOD {
                    words = words.child(
                        div()
                            .text_size(theme::text_label())
                            .text_color(theme.fg_subtle)
                            .child(self.signing.trusted_signer_body.clone()),
                    );
                }
                let mut option = div()
                    .id(("signing-sign-with-option", index))
                    .flex()
                    .items_center()
                    .px(px(16.))
                    .py(px(10.))
                    .rounded(px(8.))
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_raised))
                    .on_click(pick(Some(id), cx))
                    .child(words);
                if selected {
                    option = option.child(icon_img(
                        &mut self.icons,
                        Icon::Check,
                        false,
                        theme.accent,
                        14.,
                    ));
                }
                list = list.child(option);
            }
            block = block.child(list);
        }
        Some(block)
    }

    #[cfg(target_os = "linux")]
    fn sign_with_row(&mut self, _theme: &Theme, _cx: &mut Context<Self>) -> Option<Div> {
        None
    }

    /// The fee row's tap, and one listener per fee coin in the relay's order
    /// (the web's `onfee` / `onfeepick`). `None` for a coin the core says
    /// cannot pay — it is drawn, never picked. Nothing for the mocks.
    #[cfg(not(target_os = "linux"))]
    fn fee_actions(
        &mut self,
        cx: &mut Context<Self>,
    ) -> (Option<panels::Click>, Vec<Option<panels::Click>>) {
        let Some(host) = self.signing_host.clone() else {
            return (None, Vec::new());
        };
        let options = host.read(cx).fee_view().options.clone();
        let on_row: panels::Click = Box::new({
            let host = host.clone();
            move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                host.update(cx, |host, cx| host.fee_tapped(cx));
            }
        });
        let on_pick = options
            .into_iter()
            .map(|option| {
                (!option.insufficient).then(|| -> panels::Click {
                    let host = host.clone();
                    Box::new(
                        move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                            let token = option.contract.clone();
                            host.update(cx, |host, cx| host.pick_fee(token, cx));
                        },
                    )
                })
            })
            .collect();
        (Some(on_row), on_pick)
    }

    #[cfg(target_os = "linux")]
    fn fee_actions(
        &mut self,
        _cx: &mut Context<Self>,
    ) -> (Option<panels::Click>, Vec<Option<panels::Click>>) {
        (None, Vec::new())
    }

    fn wallet_columns(
        &mut self,
        theme: &Theme,
        caption: bool,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Div {
        let mut columns = div()
            .flex_1()
            .min_h(px(0.))
            .flex()
            .child(self.sidebar(theme, cx));
        columns = match self.section {
            Section::Wallet => columns.child(self.content(theme, cx)),
            Section::Contacts => columns.child(self.contacts_content(theme, caption, window, cx)),
            Section::Explore => columns.child(self.explore_content(theme, cx)),
            Section::Settings => columns
                .child(self.settings_nav(theme, cx))
                .child(self.settings_panel(theme, window, cx)),
        };
        // The contact form takes the third column over whatever it was about
        // (078 C-05); closing it gives the column back.
        let form = if self.section == Section::Contacts {
            self.contact_form_body(theme, window, cx)
        } else {
            None
        };
        if let Some((title, editing, body)) = form {
            // An edit came from a detail it can step back to; an add has
            // nothing behind it but the close.
            let back = editing.then(|| {
                div()
                    .id("contact-form-back")
                    .w(px(28.))
                    .h(px(28.))
                    .rounded(px(14.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .child(crate::wallet::components::icon_img(
                        &mut self.icons,
                        Icon::ChevronLeft,
                        false,
                        theme.fg_muted,
                        16.,
                    ))
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.contact_form = None;
                        cx.notify();
                    }))
                    .into_any_element()
            });
            return columns.child(self.panel_scaffold_with(theme, title, back, false, body, cx));
        }
        columns = match self.panel {
            PanelId::None => columns,
            PanelId::Receive => {
                let body = self.receive_body(theme);
                let title = self.strings.receive_title.clone();
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::AssetDetail => {
                let model = self.asset_detail_model(cx);
                let title = model.ticker.clone();
                let body = self.asset_detail_body(&model, theme, cx);
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::ContactDetail => {
                let body = self.contact_detail_body(theme, cx);
                let title = self.contacts.section_contacts.clone();
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::Connection => {
                let body = self.connection_body(theme, cx);
                let title = self.explore.connection_title.clone();
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::Signing if self.dapp_landing.is_some() && self.no_signing_host() => {
                let body = self.dapp_receipt_body(theme, window, cx);
                let title = self.signing.panel_title.clone();
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::Signing => {
                let body = self.signing_body(theme, window, cx);
                let title = self.signing.panel_title.clone();
                columns.child(self.panel_scaffold(theme, title, body, cx))
            }
            PanelId::Flow => match self.sync_send_flow(cx) {
                // DS1L is a centred modal over the window, not a column — the
                // page root draws it; see `scan_overlay`.
                None | Some(FlowPanel::Ds1) => columns,
                Some(panel) => {
                    let send = self.send_bindings(panel, cx);
                    let mut body = self.flow_body(panel, cx);
                    self.dress_batch_total(&mut body, cx);
                    let tx_ids = if self.identity.is_some() && panel == FlowPanel::Da1 {
                        flows_live::history_ids(
                            &resident::resident::<ActivityFeed>(cx).read(cx).view(),
                        )
                    } else {
                        Vec::new()
                    };
                    // A live send names the coin in its title; the mock's is
                    // USDT. With no coin named, the title is the plain action
                    // word — NOT the mock's, which is a token this person may
                    // not hold. `selected_token` is null for a reachable
                    // reason: a hand-off from the address book opens the form
                    // on the recipient before the token list answers (the
                    // core's deliberate optimism), and that fetch took ~17s on
                    // a cold start here — seventeen seconds of "发送 USDT" on a
                    // wallet holding ETH and xDAI. The web's rule, same
                    // reason (`flows/live-send.ts`).
                    let title = match (&send, panel) {
                        // A sweep is several coins: "Send tokens", never the
                        // first pick's "Send ETH" (the web's `multiSendTitle`).
                        (Some(_), FlowPanel::Dsd2 | FlowPanel::Dsd2b)
                            if self
                                .send_views(cx)
                                .is_some_and(|(view, _)| view.multi_select_mode) =>
                        {
                            self.flow_strings.multi_send_title.clone()
                        }
                        (Some(_), FlowPanel::Dsd2 | FlowPanel::Dsd2b) => self
                            .send_views(cx)
                            .and_then(|(view, _)| view.selected_token)
                            .map_or_else(
                                || self.flow_strings.send_action.clone(),
                                |token| {
                                    SharedString::from(crate::wallet::fill(
                                        &self.flow_strings.send_title,
                                        "symbol",
                                        &token.symbol,
                                    ))
                                },
                            ),
                        _ => flow_fixtures::panel_title(panel, &self.flow_strings),
                    };
                    let address = if self.identity.is_some() && panel == FlowPanel::Dt3 {
                        resident::resident::<ManageTokens>(cx)
                            .read(cx)
                            .view()
                            .input_address
                    } else {
                        String::new()
                    };
                    let focus = self.add_token_focus.clone();
                    let placeholder = self.flow_strings.token_address_label.clone();
                    let mut actions = Self::flow_actions(
                        panel,
                        self.identity.is_some(),
                        tx_ids,
                        &focus,
                        &address,
                        &placeholder,
                        send,
                        cx,
                    );
                    actions.search = Some(self.flow_search_field(panel, cx));
                    if panel == FlowPanel::Dt3 && self.identity.is_some() {
                        self.bind_add_token_tabs(&mut actions, cx);
                    }
                    if panel == FlowPanel::Dsd4 {
                        self.tick_receipt(cx);
                    }
                    actions.copy = Some(self.flow_copy_action(cx));
                    let rendered = panels::render(
                        &body,
                        theme,
                        &mut self.icons,
                        &mut self.identicons,
                        window,
                        actions,
                    );
                    // The chevron appears only once the column is more than one
                    // level deep: closing the whole column is not the same
                    // gesture as stepping back one.
                    let back = (self.flows.len() > 1).then(|| self.flow_strings.back.clone());
                    columns.child(self.flow_scaffold(theme, title, back, rendered, cx))
                }
            },
        };
        columns
    }

    /// The hero's status line, pressed (078 H-03) — the web's `openRescue`:
    /// an unreachable chain opens ITS RPC editor, the first of them when
    /// several are down; anything else opens the breakdown.
    fn open_balance_status(&mut self, cx: &mut Context<Self>) {
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        match view.banner_chain_ids.first() {
            Some(&chain_id) => {
                self.settings_fix_chain = Some(chain_id);
                self.settings_dialog = Some(SettingsDialog::FixRpc);
            }
            None => self.balance_detail_open = true,
        }
        cx.notify();
    }

    /// SR3, the balance by network (`settings/ui/BalanceDetailBody.svelte`)
    /// in the desktop's dialog.
    fn balance_detail_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if !self.balance_detail_open || self.identity.is_none() {
            return None;
        }
        let view = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        let money = self.money(cx);
        let detail = wallet_live::balance_detail(&view, &self.strings, &self.locale, &money);
        let s = &self.strings;
        let section = |text: SharedString| {
            div()
                .mt(px(16.))
                .mb(px(4.))
                .text_size(theme::text_row_sub())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_base)
                .child(text)
        };
        let row = |line: &wallet_live::DetailChain, retry: Option<gpui::AnyElement>| {
            let name = crate::executor::custom_tokens::network_name(line.chain_id);
            let mut text = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(line.name.clone()),
                );
            if let Some((status, failed)) = &line.status {
                text = text.child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(if *failed {
                            theme.error_base
                        } else {
                            theme.fg_subtle
                        })
                        .child(status.clone()),
                );
            }
            div()
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .border_b_1()
                .border_color(theme.border_card)
                .child(chain_logo_mark(
                    u64::from(line.chain_id),
                    crate::settings::model::lettermark(&name),
                    crate::settings::model::chain_tint(u64::from(line.chain_id))
                        .unwrap_or(0x8A_8F_98),
                    32.,
                ))
                .child(text)
                .children(line.amount.clone().map(|amount| {
                    div()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .child(amount)
                }))
                .children(retry)
        };

        let mut body = div().flex().flex_col().child(
            div()
                .mb(px(16.))
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(detail.summary.clone()),
        );
        body = body.child(section(s.detail_networks_label.clone())).child(
            div()
                .mb(px(8.))
                .text_size(theme::text_label())
                .line_height(gpui::relative(1.4))
                .text_color(theme.fg_subtle)
                .child(s.detail_networks_note.clone()),
        );
        for line in &detail.pending {
            let retry = line.retry.then(|| {
                let chain_id = line.chain_id;
                div()
                    .id(("balance-detail-retry", line.chain_id as usize))
                    .flex_none()
                    .cursor_pointer()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.info_base)
                    .child(s.detail_retry.clone())
                    .on_click(cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                        // As the RPC editor's Done does: clear the chain's
                        // failure and force one read — the core's own retry
                        // is throttled like any other fetch.
                        crate::executor::balance_dashboard::dispatch(
                            vela_core::app::balance_dashboard::Event::FixChainResolved { chain_id },
                            cx,
                        );
                        crate::executor::balance_dashboard::refresh(cx);
                        cx.notify();
                    }))
                    .into_any_element()
            });
            body = body.child(row(line, retry));
        }
        body = body.child(section(s.detail_updated.clone()));
        for line in &detail.done {
            body = body.child(row(line, None));
        }
        if !detail.unpriced.is_empty() {
            body = body.child(section(s.balance_unpriced.clone()));
            for line in &detail.unpriced {
                body = body.child(row(line, None));
            }
        }
        let title = s.detail_title.clone();
        Some(
            crate::ui::dialog::dialog(
                "balance-detail",
                theme,
                window,
                title,
                None,
                self.dialog_close_icon(theme),
                body.pb(px(8.)),
                &self.dialog_scroll("balance-detail"),
                Self::closer(cx, |this, _| this.balance_detail_open = false),
            )
            .into_any_element(),
        )
    }

    /// Copy `text`, and show that it was copied — under `key` — for `hold`
    /// (078 X-06). The web's holds: a tick 150 ms, a key row's "Copied"
    /// 1.2 s, the contacts toast 1.5 s.
    fn copy_text(
        &mut self,
        key: impl Into<SharedString>,
        text: String,
        hold: std::time::Duration,
        cx: &mut Context<Self>,
    ) {
        cx.write_to_clipboard(gpui::ClipboardItem::new_string(text));
        self.copied = Some(key.into());
        self.copied_press += 1;
        let press = self.copied_press;
        cx.notify();
        cx.spawn(async move |page, cx| {
            cx.background_executor().timer(hold).await;
            let _ = page.update(cx, |this, cx| {
                if this.copied_press == press {
                    this.copied = None;
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The flow panels' copy buttons: a tick for 150 ms (`ReceiveList`,
    /// `TxDetail` — long enough to register, short enough that three copies
    /// in a row never leave two ticks standing).
    fn flow_copy_action(&self, cx: &mut Context<Self>) -> panels::CopyAction {
        let page = cx.entity().downgrade();
        panels::CopyAction {
            copied: self.copied.clone(),
            on_copy: std::rc::Rc::new(move |key, text, _, cx| {
                let _ = page.update(cx, |this, cx| {
                    this.copy_text(
                        key,
                        text.to_string(),
                        std::time::Duration::from_millis(150),
                        cx,
                    );
                });
            }),
        }
    }

    /// The contacts route's "Copied" toast: centred 32 above the bottom, a
    /// pill in the ink colour, while a contact's address is on the clipboard
    /// from the list or the detail panel.
    fn contacts_toast(&self, theme: &Theme) -> Option<Div> {
        if self.section != Section::Contacts || self.copied.as_deref() != Some(CONTACTS_TOAST) {
            return None;
        }
        Some(
            div()
                .absolute()
                .left_0()
                .right_0()
                .bottom(px(32.))
                .flex()
                .justify_center()
                .child(
                    div()
                        .px(px(16.))
                        .py(px(8.))
                        .rounded_full()
                        .bg(theme.fg_base)
                        .text_color(theme.bg_base)
                        .text_size(theme::text_label())
                        .shadow(crate::ui::dialog::shadow_lg())
                        .child(self.contacts.copied.clone()),
                ),
        )
    }

    /// Keep a submitted receipt counting: redraw once a second while the send
    /// sits on its receipt with the relay holding the op — "~9s remaining" is
    /// a number a person reads, and it has to change (the web's `setInterval`).
    /// Stops by itself once the receipt settles or the panel goes.
    fn tick_receipt(&mut self, cx: &mut Context<Self>) {
        fn waiting(this: &WalletPage, cx: &gpui::App) -> bool {
            this.flows.last() == Some(&FlowPanel::Dsd4)
                && this.send_host.as_ref().is_some_and(|host| {
                    host.read(cx).view.receipt.as_ref().is_some_and(|receipt| {
                        receipt.status == vela_core::app::send::SendReceiptStatus::Submitted
                    })
                })
        }
        if self.receipt_ticking || !waiting(self, cx) {
            return;
        }
        self.receipt_ticking = true;
        cx.spawn(async move |page, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let go_on = page
                    .update(cx, |this, cx| {
                        let go_on = waiting(this, cx);
                        if go_on {
                            cx.notify();
                        } else {
                            this.receipt_ticking = false;
                        }
                        go_on
                    })
                    .unwrap_or(false);
                if !go_on {
                    return;
                }
            }
        })
        .detach();
    }

    /// The landing receipt (the web's `DappReceipt` over `StatusHero`): where
    /// a dApp transaction stands, read from the tracker the signing host
    /// already handed it to — submitting until the tracker has it, submitted
    /// with the ring, confirmed with its transaction hash and explorer, or
    /// failed. Nothing here answers the request: the site was answered before
    /// this was drawn, so Done closes a surface, never a conversation.
    fn dapp_receipt_body(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        use crate::flows::fixtures::ReceiptStage;
        use vela_core::app::tx_tracker::TrackStatus;
        let Some(landing) = self.dapp_landing.clone() else {
            return div();
        };
        let entry = resident::resident::<vela_core::app::tx_tracker::TxTracker>(cx)
            .read(cx)
            .view()
            .entries
            .into_iter()
            .find(|entry| entry.user_op_hash.eq_ignore_ascii_case(&landing.op_hash));
        let s = &self.signing;
        // `Unreachable` is NOT a failure: the wallet could not ask, which is
        // not the chain saying no, and a cross for it would be a verdict this
        // wallet does not have.
        let (stage, title, captions, hash, explorer) = match entry.as_ref() {
            None => (
                ReceiptStage::Submitting,
                s.receipt_confirming.clone(),
                vec![],
                None,
                None,
            ),
            Some(entry) => match (entry.status, entry.tx_hash.clone()) {
                (TrackStatus::Confirmed, Some(tx)) => {
                    let url = crate::executor::custom_tokens::explorer_base(landing.chain_id)
                        .map(|base| SharedString::from(format!("{base}/tx/{tx}")));
                    (
                        ReceiptStage::Confirmed,
                        s.receipt_confirmed.clone(),
                        vec![],
                        Some((s.receipt_tx_hash.clone(), tx)),
                        url.map(|url| (s.receipt_explorer.clone(), url)),
                    )
                }
                (TrackStatus::Dropped | TrackStatus::Rejected, _) => (
                    ReceiptStage::Failed,
                    s.receipt_failed.clone(),
                    vec![s.receipt_failed_hint.clone()],
                    Some((s.receipt_op_hash.clone(), landing.op_hash.clone())),
                    None,
                ),
                _ => (
                    ReceiptStage::Submitted,
                    s.receipt_submitted.clone(),
                    vec![s.receipt_confirming_hint.clone()],
                    // The OPERATION hash: there is no transaction until it
                    // lands, and labelling one as the other sends a person to
                    // search an explorer for nothing.
                    Some((s.receipt_op_hash.clone(), landing.op_hash.clone())),
                    None,
                ),
            },
        };
        let progress = match stage {
            ReceiptStage::Submitted => {
                let since = entry
                    .as_ref()
                    .and_then(|entry| entry.submitted_at_ms)
                    .unwrap_or(landing.raised_at_ms);
                let elapsed = ((crate::executor::now_ms() - since) / 1000.).max(0.) as u64;
                vela_core::app::network_admin::typical_inclusion_s(landing.chain_id)
                    .and_then(|typical| flows_live::ring_progress(elapsed, u64::from(typical)))
            }
            ReceiptStage::Confirmed => Some(1.),
            _ => None,
        };
        if stage == ReceiptStage::Submitted {
            self.tick_landing(cx);
        }

        let hero = panels::status_hero(theme, &mut self.icons, stage, progress, &title, &captions);
        // Centred in the column, as the web's `.receipt` (`min-height: 100%;
        // justify-content: center`): the column's height less its header.
        let mut body = div()
            .min_h(px((f32::from(window.viewport_size().height) - 120.).max(0.)))
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap(px(8.))
            .p(px(16.))
            .child(hero);
        if let Some((label, value)) = hash {
            // A click copies the whole hash and answers with a tick for 1.5 s
            // where the hash was (`DappReceipt`'s own affordance).
            let copied = self.copied.as_deref() == Some("dapp-receipt-hash");
            let short = if value.len() > 20 {
                format!("{}…{}", &value[..10], &value[value.len() - 8..])
            } else {
                value.clone()
            };
            body = body.child(
                div()
                    .id("dapp-receipt-hash")
                    .flex()
                    .items_baseline()
                    .gap(px(4.))
                    .cursor_pointer()
                    .text_size(theme::text_row_sub())
                    .child(div().text_color(theme.fg_muted).child(label))
                    .child(
                        div()
                            .font_family(theme::font_mono())
                            .text_color(if copied {
                                theme.success_base
                            } else {
                                theme.fg_base
                            })
                            .child(SharedString::from(if copied {
                                "✓".to_owned()
                            } else {
                                short
                            })),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.copy_text(
                            "dapp-receipt-hash",
                            value.clone(),
                            std::time::Duration::from_millis(1500),
                            cx,
                        );
                    })),
            );
        }
        if let Some((label, url)) = explorer {
            body = body.child(
                div()
                    .id("dapp-receipt-explorer")
                    .cursor_pointer()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.accent)
                    .child(label)
                    .on_click(move |_, _, cx| cx.open_url(&url)),
            );
        }
        body.child(
            div().mt(px(8.)).w_full().child(
                // `<Button variant="primary">` — the default pill shape.
                crate::flows::components::accent_button(theme, self.signing.receipt_done.clone())
                    .rounded_full()
                    .id("dapp-receipt-done")
                    .cursor_pointer()
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.dapp_landing = None;
                        if this.panel == PanelId::Signing && this.no_signing_host() {
                            this.panel = PanelId::None;
                        }
                        cx.notify();
                    })),
            ),
        )
    }

    /// Keep a landing receipt's ring moving: redraw once a second while it
    /// waits (the web's `nowMs` tick). Stops by itself.
    fn tick_landing(&mut self, cx: &mut Context<Self>) {
        if self.receipt_ticking {
            return;
        }
        self.receipt_ticking = true;
        cx.spawn(async move |page, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let go_on = page
                    .update(cx, |this, cx| {
                        let go_on = this.dapp_landing.is_some() && this.panel == PanelId::Signing;
                        if go_on {
                            cx.notify();
                        } else {
                            this.receipt_ticking = false;
                        }
                        go_on
                    })
                    .unwrap_or(false);
                if !go_on {
                    return;
                }
            }
        })
        .detach();
    }

    /// The query under the flow panel on top, as the field the panel draws.
    fn flow_search_field(
        &mut self,
        panel: FlowPanel,
        cx: &mut Context<Self>,
    ) -> panels::AddressField {
        if self.flow_query.0 != Some(panel) {
            self.flow_query = (Some(panel), String::new());
        }
        let page = cx.entity().downgrade();
        panels::AddressField {
            focus: self.flow_query_focus.clone(),
            value: self.flow_query.1.clone(),
            placeholder: SharedString::default(),
            on_change: Box::new(move |text, _, cx| {
                let _ = page.update(cx, |this, cx| {
                    this.flow_query.1 = text;
                    cx.notify();
                });
            }),
        }
    }

    /// The anchored menu overlay (DC5/DC6). Appended last in the page root and
    /// deferred so it paints above the columns; `occlude` keeps clicks off the
    /// list underneath and `on_mouse_down_out` dismisses it (research.md D2).
    /// DS1L — the scanner, centred over a dimmed window.
    ///
    /// The one flow the third column does not host: a scanner is a viewfinder,
    /// and a 400px column is the wrong shape for one. Same scrim idiom as the
    /// sign-out dialog.
    /// The money-in celebration, floating over whatever is on screen.
    ///
    /// Over the WINDOW rather than over the wallet column, because money
    /// landing is not news about the screen somebody happens to be on: the
    /// address book and the settings panel are as good a place to be told as
    /// the home is. It clears the caption strip where the page draws one, and
    /// the gallery chip bar where that is up, so it never covers chrome.
    ///
    /// The entrance is the phone's, in the phone's numbers: 320 ms, fading up
    /// through twelve pixels. What follows is the core's — the toast leaves
    /// when `FeedView::toast` goes, which is 2.8 s later on the machine's own
    /// timer, and this shell has no opinion about when that is.
    fn receipt_toast(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let text = if self.identity.is_some() {
            // Reading privacy here is what ARMS the suppression: the core
            // withholds the toast while the hero is masked, and it only knows
            // to because of this call.
            self.sync_feed_privacy(cx);
            let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
            wallet_live::receipt_toast(&feed, &self.strings)?
        } else {
            self.celebrating
                .then(|| fixtures::receipt_toast(&self.strings))?
        };

        let top = theme::WALLET_TOAST_TOP
            + if self.gallery {
                GALLERY_BAR_H + gallery_bar_caption_pad(owns_titlebar(window))
            } else if owns_titlebar(window) {
                CAPTION_H
            } else {
                0.
            };
        let pill = crate::wallet::components::receipt_toast(theme, &mut self.icons, text);
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
                        "receipt-toast",
                        gpui::Animation::new(std::time::Duration::from_millis(320))
                            .with_easing(gpui::ease_out_quint()),
                        |pill, delta| pill.opacity(delta).top(px((delta - 1.) * 12.)),
                    ),
                )
                .into_any_element(),
        )
    }

    fn scan_overlay(
        &mut self,
        theme: &Theme,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        if self.flows.last() != Some(&FlowPanel::Ds1) {
            // Left the scanner: the camera goes with it, and so does the frame
            // still holding a texture.
            self.stop_camera(window);
            return None;
        }
        self.pump_camera(window, cx);
        let flow_fixtures::FlowBody::Scan(model) =
            flow_fixtures::body(FlowPanel::Ds1, &self.flow_strings)
        else {
            return None;
        };
        // Two tools: a picture off the disk, and flipping a camera this
        // desktop does not have yet. The first is live; the second is drawn
        // and inert rather than armed and lying — the rule the site menu's
        // three unowned items already follow.
        let tools: Vec<Option<panels::Click>> = vec![
            Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                this.scan_from_file(cx);
            })) as panels::Click),
            None,
        ];
        // The camera, when this platform has one and the person let us. The
        // frame is published under the same rule the launch animation's pump
        // follows (`contracts/desktop-frame-pump.md`): one `RenderImage` on
        // screen at a time, and the evicted one HANDED BACK so its GPU texture
        // cannot be forgotten — a preview mints one per frame, so leaking them
        // would be ~30 textures a second.
        let preview = self.scan_preview.slot.clone();
        let close: panels::Click = Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
            this.close_scanner(cx);
        }));
        let notice = self.scan_notice_text();
        let no_camera = self.scan_camera_failure.is_some();
        let width = (f32::from(window.viewport_size().width) * 0.9).min(440.);
        let card = panels::scan_modal(
            &model,
            theme,
            &mut self.icons,
            tools,
            Some(close),
            preview,
            notice,
            no_camera,
            width,
        );
        Some(
            div()
                .id("scan-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
                .on_click(cx.listener(|this, _, _, cx| this.close_scanner(cx)))
                .child(
                    // The card is not the scrim. Without this, a click on
                    // anything IN the modal — its tools included — bubbled to
                    // the scrim's dismiss and closed the scanner: the tool
                    // fired, and then the screen it opened was cleared behind
                    // it. Every other overlay in this page already stops the
                    // press here; this one was drawn before it had anything to
                    // press.
                    div()
                        .id("scan-card")
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(card),
                )
                .into_any_element(),
        )
    }

    /// The sentence under the viewfinder when the hint is not true — the
    /// web's `scanNotice`, in its order: a code that was read and cannot be
    /// used, a picture with no code, then why the camera is not there.
    fn scan_notice_text(&self) -> Option<SharedString> {
        use crate::executor::camera::CameraFailure;
        let s = &self.flow_strings;
        match (self.scan_notice, self.scan_camera_failure) {
            (Some(ScanNotice::Unusable), _) => Some(s.scan_invalid.clone()),
            (Some(ScanNotice::NothingFound), _) => Some(s.scan_no_qr.clone()),
            (None, Some(CameraFailure::Denied)) => Some(s.scan_permission.clone()),
            (None, Some(CameraFailure::Absent)) => Some(s.scan_no_camera.clone()),
            (None, Some(CameraFailure::Unavailable)) => Some(s.scan_unavailable.clone()),
            (None, None) => None,
        }
    }

    /// Close the scanner — and ONLY the scanner.
    ///
    /// The X, Escape and a click on the dimmed window all land here. The
    /// scanner is an overlay this page put up, so closing it takes itself off
    /// the stack and nothing else: opened from a send form, the form is still
    /// there underneath. (The scrim used to `flows.clear()`, which threw the
    /// form away with it and left its `send_host` behind.) The camera stops on
    /// the next frame, when `scan_overlay` sees DS1 is no longer on top.
    fn close_scanner(&mut self, cx: &mut Context<Self>) {
        // Inside a live send the scanner is the core's; it takes it down, or
        // the next frame's stack would put it straight back.
        if let Some(host) = self.send_host.clone() {
            host.update(cx, |host, cx| host.dispatch(SendEvent::CloseScanner, cx));
        }
        self.flows.retain(|panel| *panel != FlowPanel::Ds1);
        if self.flows.is_empty() {
            self.panel = PanelId::None;
        }
        cx.notify();
    }

    /// Keep the viewfinder fed while the scanner is open.
    ///
    /// Started on the first frame DS1 draws rather than on the click that
    /// opened it, because the scanner can also be reached by a restored flow
    /// stack (`VELA_FLOW=DS1`) where no click happened.
    fn pump_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scan_camera.is_none() {
            // A scanner just opened: what the last one said is not true of it.
            self.scan_notice = None;
            self.scan_quiet_until = None;
            self.scan_camera = Some(crate::executor::camera::start());
            // The camera has its own thread and no way to reach this window;
            // this asks for a repaint at roughly the rate a preview needs one,
            // and stops the moment the scanner is gone.
            cx.spawn(async move |page, cx| {
                loop {
                    cx.background_executor()
                        .timer(std::time::Duration::from_millis(60))
                        .await;
                    let alive = page
                        .update(cx, |this, cx| {
                            let alive = this.scan_camera.is_some();
                            if alive {
                                cx.notify();
                            }
                            alive
                        })
                        .unwrap_or(false);
                    if !alive {
                        return;
                    }
                }
            })
            .detach();
        }

        let Some(session) = self.scan_camera.as_ref() else {
            return;
        };
        // A code the camera saw. Taken once, so one QR held up to the lens
        // starts one send.
        if let Some(text) = session.take_payload() {
            let quiet = self
                .scan_quiet_until
                .is_some_and(|until| std::time::Instant::now() < until);
            if !quiet && self.scan_resolved(text, cx) {
                self.stop_camera(window);
                return;
            }
        }
        let Some(session) = self.scan_camera.as_ref() else {
            return;
        };
        let (frame, failure) = session.snapshot();
        self.scan_camera_failure = failure;
        let Some(frame) = frame else {
            return;
        };
        // `RenderImage` wants premultiplied BGRA; the camera hands over RGBA.
        // Swapping in place is two moves per pixel and keeps the one copy this
        // path already makes.
        let (width, height) = frame.dimensions();
        let mut bytes = frame.into_raw();
        for pixel in bytes.chunks_exact_mut(4) {
            pixel.swap(0, 2);
        }
        let Some(bgra) = image::RgbaImage::from_raw(width, height, bytes) else {
            return;
        };
        let image = std::sync::Arc::new(gpui::RenderImage::new(vec![image::Frame::new(bgra)]));
        if let Some(evicted) = self.scan_preview.replace(image) {
            let _ = window.drop_image(evicted);
        }
    }

    /// Stop the capture and release the frame on screen. Idempotent.
    fn stop_camera(&mut self, window: &mut Window) {
        self.scan_camera = None;
        if let Some(evicted) = self.scan_preview.take() {
            let _ = window.drop_image(evicted);
        }
    }

    /// A QR code out of a picture on this machine.
    ///
    /// The desktop's scanner has been a drawing since spec 021: a viewfinder
    /// and a "from gallery" button with nothing behind either. This is the half
    /// that needs no camera — a screenshot, a saved share card, a photo
    /// somebody sent — and the half that works on Windows and Linux too.
    ///
    /// What the code MEANS is not decided here. The shell tokenizes
    /// (`eip681::parse`, ported with both of its refusals) and the core rules:
    /// inside a live send, `ScanResolved` decides whether the screen locks and
    /// to what; from the home there is no session yet, so the code OPENS one,
    /// prefilled and locked exactly when the request names a chain to lock to.
    fn scan_from_file(&mut self, cx: &mut Context<Self>) {
        // `VELA_SCAN_FILE=<path>` skips the dialog — the same env-seam family
        // as `VELA_GALLERY_TAB` and `VELA_FLOW`, and for the same reason: a
        // file dialog is a system window this app cannot drive, so without it
        // no screenshot pass and no headless run can ever reach the far side
        // of a scan.
        if let Ok(path) = std::env::var("VELA_SCAN_FILE") {
            self.scan_notice = None;
            match std::fs::read(&path)
                .ok()
                .and_then(|bytes| crate::executor::qr::decode_first(&bytes))
            {
                Some(text) => {
                    self.scan_resolved(text, cx);
                }
                None => self.scan_notice = Some(ScanNotice::NothingFound),
            }
            cx.notify();
            return;
        }
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |page, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            let Ok(bytes) = std::fs::read(&path) else {
                return;
            };
            let found = crate::executor::qr::decode_first(&bytes);
            page.update(cx, |this, cx| match found {
                Some(text) => {
                    this.scan_resolved(text, cx);
                }
                // No code in that picture. The modal stays up and says so: a
                // person who picked the wrong file wants to pick another one,
                // not to be returned to the wallet — and not to wonder whether
                // the button did anything.
                None => {
                    this.scan_notice = Some(ScanNotice::NothingFound);
                    cx.notify();
                }
            })
            .ok();
        })
        .detach();
    }

    /// What a scanned string does, in the two places a scan can happen.
    ///
    /// Returns whether the scan was acted on. `false` is a code that is not a
    /// payment, read outside a send: the scanner stays up and says "Invalid
    /// QR" (078 F-02) — it used to close without a word.
    fn scan_resolved(&mut self, text: String, cx: &mut Context<Self>) -> bool {
        use vela_core::app::send::SendScan;
        let request = crate::flows::eip681::parse(&text);
        if self.send_host.is_none()
            && request.is_none()
            && !crate::flows::eip681::is_hex_address(text.trim())
        {
            self.scan_notice = Some(ScanNotice::Unusable);
            self.scan_quiet_until =
                Some(std::time::Instant::now() + std::time::Duration::from_secs(2));
            cx.notify();
            return false;
        }
        self.scan_notice = None;
        self.flows.retain(|panel| *panel != FlowPanel::Ds1);

        // Inside a live send the CORE rules on it — the shell only tokenizes.
        if let Some(host) = self.send_host.clone() {
            let scan = match request {
                Some(request) => SendScan::Request {
                    recipient: request.recipient,
                    chain_id: request.chain_id,
                    token_address: request.token_address,
                    amount_base_units: request.amount_base_units,
                },
                None => SendScan::Text { data: text },
            };
            host.update(cx, |host, cx| {
                host.dispatch(SendEvent::ScanResolved { scan }, cx);
            });
            if self.flows.is_empty() {
                self.flows = FlowPanel::entry(FlowEntry::Send);
            }
            self.panel = PanelId::Flow;
            cx.notify();
            return true;
        }

        // From the home there is no session yet. A request that names a chain
        // opens Send LOCKED to it; anything else opens Send with the recipient
        // filled in and editable.
        let (params, recipient) = match request {
            Some(request) => {
                let locked = request.chain_id.is_some();
                (
                    SendOpenParams {
                        prefilled_recipient: Some(request.recipient.clone()),
                        prefilled_chain_id: request.chain_id.map(|id| id.to_string()),
                        prefilled_token_address: request.token_address,
                        prefilled_amount_base: request.amount_base_units,
                        locked,
                        ..SendOpenParams::default()
                    },
                    request.recipient,
                )
            }
            None => {
                let address = text.trim().to_owned();
                if !crate::flows::eip681::is_hex_address(&address) {
                    // Refused above, before the scanner came down; kept so a
                    // string can never start a send.
                    return false;
                }
                (
                    SendOpenParams {
                        prefilled_recipient: Some(address.clone()),
                        ..SendOpenParams::default()
                    },
                    address,
                )
            }
        };
        let _ = recipient;
        self.section = Section::Wallet;
        self.send_sweeping = false;
        self.flows = FlowPanel::entry(FlowEntry::Send);
        self.panel = PanelId::Flow;
        self.open_send(params, cx);
        cx.notify();
        true
    }

    /// Read an address-book backup and hand it to the core.
    ///
    /// The shell reads and PARSES; the core applies existing-wins and counts
    /// what happened. Which of those two halves is which is the reason
    /// `ImportParsed` takes already-parsed rows rather than a file.
    fn import_contacts(into_group: Option<String>, cx: &mut Context<Self>) {
        // `VELA_IMPORT_FILE=<path>` answers the picker — the `VELA_SCAN_FILE`
        // seam, for the same reason: a system file dialog is a window no
        // verification pass can drive.
        let pinned = std::env::var_os("VELA_IMPORT_FILE").map(std::path::PathBuf::from);
        let paths = pinned.is_none().then(|| {
            cx.prompt_for_paths(gpui::PathPromptOptions {
                files: true,
                directories: false,
                multiple: false,
                prompt: None,
            })
        });
        cx.spawn(async move |page, cx| {
            let path = match (pinned, paths) {
                (Some(path), _) => path,
                (None, Some(paths)) => {
                    let Ok(Ok(Some(paths))) = paths.await else {
                        // Cancelled, or the platform declined. Nothing to
                        // report: the person closed a dialog.
                        return;
                    };
                    let Some(path) = paths.into_iter().next() else {
                        return;
                    };
                    path
                }
                (None, None) => return,
            };
            let filename = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            // Reading the bytes is the shell's job, and its own way to fail.
            // Everything ABOUT those bytes — is this JSON, does this CSV have
            // an address column, is it empty — belongs to the core (028 moved
            // it into `app/contacts_io.rs`), because a file the web refuses
            // must not import as "0 contacts added" here.
            let Ok(content) = std::fs::read_to_string(&path) else {
                page.update(cx, |this, cx| {
                    this.import_result = Some((
                        this.contacts.import_fail_title.clone(),
                        this.contacts.import_fail_body.clone(),
                    ));
                    cx.notify();
                })
                .ok();
                return;
            };
            let now_ms = crate::executor::now_ms();
            page.update(cx, |this, cx| {
                let view = resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(
                        ContactEvent::ImportFile {
                            content,
                            filename,
                            // 导入到本组 names its group; the header's and the
                            // empty book's import are the whole book.
                            into_group: into_group.clone(),
                            now_ms,
                        },
                        cx,
                    );
                    resident.view()
                });
                // A refusal and a report are mutually exclusive, and the
                // refusal comes FIRST — a file that was rejected wrote
                // nothing, and "added 0, skipped 0" would describe that as a
                // successful import of an empty address book.
                let refused = (
                    this.contacts.import_fail_title.clone(),
                    this.contacts.import_fail_body.clone(),
                );
                this.import_result = Some(match (view.import_failure, view.last_import) {
                    (Some(_), _) => refused,
                    // The COUNTS are the core's — it applied existing-wins and
                    // knows what actually happened. An import that reports
                    // nothing is a feature that looks broken.
                    (None, Some(report)) => (
                        this.contacts.import_done_title.clone(),
                        SharedString::from(crate::wallet::fill(
                            &crate::wallet::fill(
                                &this.contacts.import_done_body,
                                "added",
                                &report.added.to_string(),
                            ),
                            "skipped",
                            &report.skipped.to_string(),
                        )),
                    ),
                    // Neither: `ImportFile` fails closed until the ledger is
                    // loaded, so nothing was read and nothing was written.
                    // Drawing no dialog here would be this whole sweep's own
                    // defect committed on the way out of it.
                    (None, None) => refused,
                });
                // The words are on this screen now; the core's one-shot line
                // must not survive into the next import.
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(ContactEvent::ImportAcknowledged, cx);
                });
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// Write the whole address book where the person points.
    ///
    /// The extension decides the format, because that is the choice the save
    /// dialog already asked them to make — a `.csv` that contains JSON is a
    /// file nothing opens. The BYTES are the core's: one serializer, so a
    /// backup taken on the desktop restores on the web.
    fn export_contacts(cx: &mut Context<Self>) {
        // The save dialog opens where a person keeps their files, not where
        // this app keeps its state. `.` would open wherever the binary was
        // launched from, which on a double-click is nowhere useful.
        let directory = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
        let target = cx.prompt_for_new_path(&directory, Some("vela-contacts.json"));
        cx.spawn(async move |page, cx| {
            let Ok(Ok(Some(path))) = target.await else {
                return;
            };
            let format = if path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("csv"))
            {
                ContactFileFormat::Csv
            } else {
                ContactFileFormat::Json
            };
            let exported_at_iso = crate::executor::now_iso();
            page.update(cx, |_, cx| {
                let file = resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(
                        ContactEvent::ExportRequested {
                            scope: ContactExportScope::All,
                            format,
                            exported_at_iso,
                        },
                        cx,
                    );
                    resident.view().export
                });
                let Some(file) = file else {
                    // The core produced nothing to hand over. Never write an
                    // empty file over the path somebody chose.
                    return;
                };
                if let Err(error) = std::fs::write(&path, &file.content) {
                    eprintln!("[vela-wallet] contacts export: {}: {error}", path.display());
                    // The one-shot stays in the view: nothing was handed over.
                    return;
                }
                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                    resident.dispatch(ContactEvent::ExportTaken, cx);
                });
            })
            .ok();
        })
        .detach();
    }

    /// What each row of an open menu does, positionally.
    ///
    /// 030 called this component blocked because its items "carry no action".
    /// They can carry one; what most of them still have nowhere to GO is a
    /// different problem, and the ones that do are wired here. An item with no
    /// entry stays inert rather than pretending — a menu row that highlights
    /// and does nothing is worse than one that plainly does not.
    fn menu_actions(
        &mut self,
        kind: ContactsMenu,
        cx: &mut Context<Self>,
    ) -> Vec<Option<contacts_components::MenuAction>> {
        // The mocks' menus are pictures. Only a real session acts.
        if self.identity.is_none() {
            return Vec::new();
        }
        match kind {
            // 重命名 / 导入 / 导出 need a text dialog and a file picker, neither
            // of which the desktop draws yet. 删除分组 needs neither.
            ContactsMenu::Group => {
                let Some(index) = self.group else {
                    return Vec::new();
                };
                let Some((id, name, _)) = self.group_models(cx).get(index).cloned() else {
                    return Vec::new();
                };
                let group_name = name.clone();
                let rename = (id.clone(), name);
                vec![
                    // 重命名分组 — the same dialog 新建分组 opens, with the id
                    // filled in, because they ask one question.
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.group_form =
                                Some((Some(rename.0.to_string()), rename.1.to_string()));
                            this.menu = None;
                            window.focus(&this.group_form_focus, cx);
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                    // 导入到本组 / 导出本组 still need a per-group file path the
                    // core has no event for; the whole-book pair is wired.
                    None,
                    None,
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            // Asked first, as the web asks (078 C-01).
                            this.confirm = Some(Confirm::DeleteGroup {
                                id: id.to_string(),
                                name: group_name.clone(),
                            });
                            this.menu = None;
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                ]
            }
            // 导入通讯录 / 导出全部通讯录.
            ContactsMenu::Header => vec![
                Some(Box::new(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx: &mut Context<Self>| {
                        this.menu = None;
                        Self::import_contacts(None, cx);
                    },
                )) as contacts_components::MenuAction),
                Some(Box::new(cx.listener(
                    |this, _: &gpui::ClickEvent, _, cx: &mut Context<Self>| {
                        this.menu = None;
                        Self::export_contacts(cx);
                    },
                )) as contacts_components::MenuAction),
            ],
            // The site menu, in the order it is drawn: refresh, share, add to
            // favourites, open in a new tab, disconnect, close.
            //
            // Three of the six have a machine (or a webview) behind them; the
            // other three are favourites and tabs, which nothing in
            // `vela-core` owns yet. `None` leaves an item drawn and inert
            // rather than armed and lying — the same rule the allowance chips
            // follow.
            ContactsMenu::Site => vec![
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.menu = None;
                    #[cfg(not(target_os = "linux"))]
                    crate::webview::reload();
                    cx.notify();
                })) as contacts_components::MenuAction),
                None,
                None,
                None,
                // Disconnect is the CORE's: the site on screen, by name — its
                // grant goes, and every open page of it hears
                // `accountsChanged []` and `disconnect`.
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.menu = None;
                    let origin = this
                        .browser_host
                        .as_ref()
                        .and_then(|host| host.read(cx).tab().and_then(|tab| tab.origin.clone()));
                    if let Some(origin) = origin {
                        this.revoke_site(origin, cx);
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
                // Close closes the PAGE — the tab on screen, as the phones'
                // "Close page" does. Its requests are settled by the core
                // when its document goes; merely leaving Explore settles
                // nothing.
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.menu = None;
                    this.panel = PanelId::None;
                    let selected = resident::resident::<ExploreSites>(cx)
                        .read(cx)
                        .view()
                        .selected_tab;
                    match selected {
                        Some(id) => this.close_browser_tab(&id, cx),
                        None => {
                            this.browsing = false;
                            this.close_browser_page(cx);
                        }
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
            ],
            // "Move to a group": the person's own groups, newest last, with
            // "new group" at the top. The index is the position in the SAME
            // list the menu was built from — read again here rather than
            // captured, so a group made in between cannot shift the answer.
            // The contact's own menu, in the order it is drawn: send, receive,
            // copy, edit, move to a group, delete. Every one of them has
            // somewhere to go on this shell — which is why it is opened at
            // last.
            ContactsMenu::Contact => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                let Some(row) = contacts_live::rows(&view).into_iter().nth(self.contact) else {
                    return Vec::new();
                };
                let address = row.address_full.to_string();
                let name = row.name.to_string();
                let send_to = address.clone();
                let copy_me = address.clone();
                let edit_address = address.clone();
                let delete_address = address.clone();
                let delete_label = SharedString::from(if name.is_empty() {
                    address.clone()
                } else {
                    name.clone()
                });
                vec![
                    // 转账 — the send flow, opened with this person in the
                    // recipient field. The core takes the prefill; the shell
                    // does not type into its own screen.
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.menu = None;
                            this.section = Section::Wallet;
                            this.send_sweeping = false;
                            this.flows = FlowPanel::entry(FlowEntry::Send);
                            this.panel = PanelId::Flow;
                            this.open_send(
                                SendOpenParams {
                                    prefilled_recipient: Some(send_to.clone()),
                                    ..SendOpenParams::default()
                                },
                                cx,
                            );
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                    // 收款 — my own address, which is what a person needs when
                    // the answer to "how do I pay you" is asked of them.
                    Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                        this.menu = None;
                        this.section = Section::Wallet;
                        this.enter_flow(FlowEntry::Receive, cx);
                        cx.notify();
                    })) as contacts_components::MenuAction),
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.menu = None;
                            this.copy_text(CONTACTS_TOAST, copy_me.clone(), CONTACTS_COPY_HOLD, cx);
                        })) as contacts_components::MenuAction,
                    ),
                    // 编辑 — the same sheet 新建联系人 opens, with the address
                    // fixed: the address IS the identity, and an edit that
                    // changed it would be a delete and an add wearing one
                    // button.
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.open_edit_contact(&edit_address, window, cx);
                        })) as contacts_components::MenuAction,
                    ),
                    // 移入分组 — the tick list, as the web's `group-pick`.
                    Some({
                        let address = address.clone();
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.open_group_pick(&address, cx);
                        })) as contacts_components::MenuAction
                    }),
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.menu = None;
                            // Asked first, as the web asks (078 C-01).
                            this.confirm = Some(Confirm::DeleteContact {
                                address: delete_address.clone(),
                                name: delete_label.clone(),
                            });
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                ]
            }

            // One row per network, in the order the menu drew them. The site
            // is the one the menu was opened about, never "whatever is on
            // screen now": a navigation in between must not move another site.
            ContactsMenu::SiteNetwork => self
                .site_networks
                .iter()
                .map(|(chain_id, _, _)| {
                    let chain_id = *chain_id;
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            let origin = this.menu_origin.take();
                            this.menu = None;
                            if let Some(origin) = origin {
                                let host = this.browser_host(cx);
                                host.update(cx, |host, cx| {
                                    host.dispatch(
                                        DbrEvent::SiteChainPicked { origin, chain_id },
                                        cx,
                                    );
                                });
                            }
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    )
                })
                .collect(),
            ContactsMenu::MoveGroup => {
                let ids: Vec<String> = resident::resident::<ExploreSites>(cx)
                    .read(cx)
                    .view()
                    .groups
                    .iter()
                    .map(|group| group.id.clone())
                    .collect();
                let mut actions: Vec<Option<contacts_components::MenuAction>> = vec![Some(
                    Box::new(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        // Name it first; the tile joins it the moment it
                        // exists.
                        let origin = this.menu_origin.take();
                        this.menu = None;
                        this.explore_form = Some(ExploreForm {
                            ask: ExploreAsk::NewGroup { then_add: origin },
                            text: String::new(),
                        });
                        window.focus(&this.explore_form_focus, cx);
                        cx.notify();
                    })) as contacts_components::MenuAction,
                )];
                for id in ids {
                    actions.push(Some(Box::new(cx.listener(
                        move |this, _: &gpui::ClickEvent, _, cx| {
                            let origin = this.menu_origin.take();
                            this.menu = None;
                            if let Some(origin) = origin {
                                resident::resident::<ExploreSites>(cx).update(
                                    cx,
                                    |resident, cx| {
                                        resident.dispatch(
                                        vela_core::app::explore_sites::Event::GroupMemberAdded {
                                            id: id.clone(),
                                            origin,
                                        },
                                        cx,
                                    );
                                    },
                                );
                            }
                            cx.notify();
                        },
                    ))
                        as contacts_components::MenuAction));
                }
                actions
            }

            // A row in Recent: open it in a new tab, pin it, or forget it.
            // All three belong to a core — the tabs', the favourites' and the
            // history's — which is why this menu could be armed the day it
            // was drawn.
            ContactsMenu::Recent => vec![
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let entry = this.take_menu_entry(cx);
                    this.menu = None;
                    if let Some((url, title)) = entry {
                        resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::explore_sites::Event::TabOpened {
                                    url: Some(url.clone()),
                                    title: Some(title),
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                        this.browsing = true;
                        this.browser_home = url.clone();
                        #[cfg(not(target_os = "linux"))]
                        crate::webview::navigate(&url);
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let entry = this.take_menu_entry(cx);
                    this.menu = None;
                    if let Some((url, title)) = entry {
                        resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::explore_sites::Event::FavoriteAdded {
                                    url,
                                    title: Some(title),
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let origin = this.menu_origin.take();
                    this.menu = None;
                    if let Some(origin) = origin {
                        resident::resident::<BrowserHistory>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::browser_history::Event::DeleteOrigin { origin },
                                cx,
                            );
                        });
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
            ],
            // The favourite tile's menu, in the order it is drawn: open in a
            // new tab, rename, move to a group, remove.
            //
            // Remove is the core's `FavoriteRemoved`, which also takes the
            // site out of every group it was in. Rename and move need an
            // input and a group picker that no desktop scenario draws yet,
            // and a new tab needs the strip (still owed) — all three stay
            // drawn and inert rather than armed and lying.
            ContactsMenu::Tile => vec![
                // Open in a new tab — the strip exists now (phase 37).
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let origin = this.menu_origin.take();
                    this.menu = None;
                    let entry = origin.and_then(|origin| {
                        resident::resident::<ExploreSites>(cx)
                            .read(cx)
                            .view()
                            .favorites
                            .iter()
                            .find(|site| site.origin == origin)
                            .map(|site| (site.url.clone(), site.name.clone()))
                    });
                    if let Some((url, title)) = entry {
                        resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::explore_sites::Event::TabOpened {
                                    url: Some(url.clone()),
                                    title: Some(title),
                                    now_ms: crate::executor::now_ms(),
                                },
                                cx,
                            );
                        });
                        this.browsing = true;
                        this.browser_home = url.clone();
                        #[cfg(not(target_os = "linux"))]
                        crate::webview::navigate(&url);
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
                // Rename — the tile's own name, which the core then keeps
                // against every later visit.
                Some(
                    Box::new(cx.listener(|this, _: &gpui::ClickEvent, window, cx| {
                        let origin = this.menu_origin.take();
                        this.menu = None;
                        if let Some(origin) = origin {
                            let current = resident::resident::<ExploreSites>(cx)
                                .read(cx)
                                .view()
                                .favorites
                                .iter()
                                .find(|site| site.origin == origin)
                                .map(|site| site.name.clone())
                                .unwrap_or_default();
                            this.explore_form = Some(ExploreForm {
                                ask: ExploreAsk::RenameFavorite { origin },
                                // Prefilled with what it is called now: a
                                // rename usually edits a name rather than
                                // replacing it.
                                text: current,
                            });
                            window.focus(&this.explore_form_focus, cx);
                        }
                        cx.notify();
                    })) as contacts_components::MenuAction,
                ),
                // Move to a group — the picker, which keeps the origin.
                Some(
                    Box::new(cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                        this.menu =
                            Some((ContactsMenu::MoveGroup, event.position(), Anchor::TopLeft));
                        cx.notify();
                    })) as contacts_components::MenuAction,
                ),
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    let origin = this.menu_origin.take();
                    this.menu = None;
                    if let Some(origin) = origin {
                        resident::resident::<ExploreSites>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                vela_core::app::explore_sites::Event::FavoriteRemoved { origin },
                                cx,
                            );
                        });
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
            ],
        }
    }

    fn menu_overlay(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let (kind, position, anchor) = self.menu?;
        let model = match kind {
            ContactsMenu::Header => contacts_fixtures::header_dropdown(&self.contacts),
            ContactsMenu::Group => contacts_fixtures::group_context(&self.contacts),
            ContactsMenu::Site => explore_fixtures::site_menu(&self.explore),
            ContactsMenu::Recent => explore_fixtures::recent_menu(&self.explore),
            ContactsMenu::MoveGroup => explore_fixtures::group_pick_menu(
                &self.explore,
                &resident::resident::<ExploreSites>(cx)
                    .read(cx)
                    .view()
                    .groups
                    .iter()
                    .map(|group| group.name.clone())
                    .collect::<Vec<_>>(),
            ),
            ContactsMenu::Tile => explore_fixtures::tile_menu(&self.explore),
            ContactsMenu::SiteNetwork => explore_fixtures::network_pick_menu(
                &self
                    .site_networks
                    .iter()
                    .map(|(_, name, current)| (name.clone(), *current))
                    .collect::<Vec<_>>(),
            ),
            ContactsMenu::Contact => contacts_fixtures::contact_context(&self.contacts),
        };
        let actions = self.menu_actions(kind, cx);
        let card = menu_card(theme, &mut self.icons, &model, actions);
        Some(
            deferred(
                anchored()
                    .anchor(anchor)
                    .position(position)
                    .snap_to_window_with_margin(px(8.))
                    .child(
                        div()
                            .id("contacts-menu")
                            .occlude()
                            .on_mouse_down_out(cx.listener(|this, _: &MouseDownEvent, _, cx| {
                                this.menu = None;
                                cx.notify();
                            }))
                            .child(card),
                    ),
            )
            .with_priority(1)
            .into_any_element(),
        )
    }
}

impl Render for WalletPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.watch_field_blurs(window, cx);
        let theme = Theme::of(self.theme_mode());
        // A survived panic (spec 038): the failure sheet, "Something went
        // wrong", with the report behind the disclosure.
        if self.crash.is_none()
            && let Some(detail) = crate::panic_report::take()
        {
            self.crash = Some(crate::outcome::Prompt::new(
                vela_core::app::PromptKind::CreateFailed { detail },
                false,
                0,
            ));
        }

        // Windows and Linux CSD have no system caption, so the page draws one
        // (spec 015 results.md deviation 5 assumed Windows was a native path;
        // `appears_transparent` means it is not). Where it lands over content,
        // that content is pushed clear of it below.
        let caption = owns_titlebar(window);

        // The browser is a NATIVE subview: it does not disappear because the
        // route changed, so every frame that is not drawing the browser column
        // takes it off the screen. Miss this and a webview floats over the
        // wallet. `place` turns it back on in the same frame it is drawn.
        //
        // Hidden is not closed (spec 070). The page keeps running, the core
        // keeps answering it, and a sheet it raised stays up: until 070 this
        // frame told the core the browser had CLOSED, so looking at the
        // wallet for a moment failed a connect or a signature the person was
        // in the middle of. Only closing the tab or the page going away
        // settles its requests.
        #[cfg(not(target_os = "linux"))]
        if !(self.section == Section::Explore && self.browsing && self.identity.is_some())
            || self.dialog_over_browser()
        {
            crate::webview::hide();
        }
        // Typing stops when the address bar loses the keyboard.
        if self.address_draft.is_some() && !self.address_focus.is_focused(window) {
            self.address_draft = None;
            self.address_selected = false;
        }

        // The column was closed under a live send: its machines go with it.
        if self.panel != PanelId::Flow && self.send_host.is_some() {
            self.send_host = None;
            self.send_fee_picker = false;
        }

        let body = if self.gallery {
            let bar = self.gallery_bar(&theme, caption, cx);
            let content: gpui::AnyElement = match self.tab {
                GalleryTab::Components => self.components_tab(&theme).into_any_element(),
                GalleryTab::ContactsComponents => {
                    self.contacts_components_tab(&theme).into_any_element()
                }
                GalleryTab::Identicons => self.identicons_tab(&theme).into_any_element(),
                // The bar already cleared the caption row for the page.
                _ => self
                    .wallet_columns(&theme, false, window, cx)
                    .into_any_element(),
            };
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(bar)
                .child(content)
        } else {
            div()
                .size_full()
                .flex()
                .flex_col()
                .child(self.wallet_columns(&theme, caption, window, cx))
        };

        // Something changed what the hero is counting — a custom token added or
        // removed — from a place that had no way to say so. Drained here
        // because this frame IS that moment: the change is what caused it.
        if self.identity.is_some() && crate::executor::balance_dashboard::take_invalidation() {
            crate::executor::balance_dashboard::refresh(cx);
        }

        let scan = self.scan_overlay(&theme, window, cx);
        let toast = self.receipt_toast(&theme, window, cx);
        let send_prompt = self.send_prompts(&theme, window, cx);
        let trusted_signer_prompt = self.trusted_signer_prompt(&theme, cx);
        let menu = self.menu_overlay(&theme, cx);
        let sign_out = self.sign_out_dialog(&theme, window, cx);
        let network_remove = self.network_remove_dialog(&theme, window, cx);
        let account_switcher = self.account_switcher_dialog(&theme, window, cx);
        let identicon_viewer = self.identicon_viewer_dialog(&theme, cx);
        let confirm = self.confirm_dialog(&theme, window, cx);
        let settings_dialog = self.settings_dialog_overlay(&theme, window, cx);
        let import_result = self.import_result_dialog(&theme, window, cx);
        let group_form = self.group_form_dialog(&theme, window, cx);
        let pick = self.pick_dialog(&theme, window, cx);
        let explore_form = self.explore_form_dialog(&theme, window, cx);
        let contact_qr = self.contact_qr_dialog(&theme, window, cx);
        let balance_detail = self.balance_detail_dialog(&theme, window, cx);
        let mut root = div()
            .size_full()
            .font_family(theme::font_ui())
            .relative()
            .bg(theme.bg_base)
            .text_color(theme.fg_base)
            .child(body);
        if let Some(scan) = scan {
            root = root.child(scan);
        }
        // Above the columns and below every dialog: a celebration must not
        // land on top of a question somebody is being asked.
        if let Some(toast) = toast {
            root = root.child(toast);
        }
        if let Some(toast) = self.contacts_toast(&theme) {
            root = root.child(toast);
        }
        // The cable's dialogs and the core's alert, over the send flow.
        if let Some(prompt) = send_prompt {
            root = root.child(prompt);
        }
        // The Trusted Signer's, over the flow that is waiting on its page.
        if let Some(prompt) = trusted_signer_prompt {
            root = root.child(prompt);
        }
        if let Some(menu) = menu {
            root = root.child(menu);
        }
        // Over everything, including the anchored menu: it is the one dialog
        // whose answer changes which screen the app is on.
        if let Some(settings_dialog) = settings_dialog {
            root = root.child(settings_dialog);
        }
        if let Some(balance_detail) = balance_detail {
            root = root.child(balance_detail);
        }
        // The import's answer, over the menu it was started from.
        if let Some(import_result) = import_result {
            root = root.child(import_result);
        }
        if let Some(pick) = pick {
            root = root.child(pick);
        }
        if let Some(contact_qr) = contact_qr {
            root = root.child(contact_qr);
        }
        if let Some(explore_form) = explore_form {
            root = root.child(explore_form);
        }
        if let Some(group_form) = group_form {
            root = root.child(group_form);
        }
        if let Some(sign_out) = sign_out {
            root = root.child(sign_out);
        }
        if let Some(network_remove) = network_remove {
            root = root.child(network_remove);
        }
        if let Some(confirm) = confirm {
            root = root.child(confirm);
        }
        if let Some(account_switcher) = account_switcher {
            root = root.child(account_switcher);
        }
        // Over everything, the switcher included: its rows' artwork opens it,
        // and a viewer under the surface that opened it is a click that did
        // nothing.
        if let Some(identicon_viewer) = identicon_viewer {
            root = root.child(identicon_viewer);
        }
        if let Some(prompt) = &self.crash {
            let entity = cx.entity();
            root = root.child(crate::outcome::outcome_sheet(
                &theme,
                &self.loc,
                prompt,
                move |id, _window, cx| {
                    entity.update(cx, |page, cx| {
                        use crate::outcome::ActionId;
                        match id {
                            ActionId::ToggleDetails => {
                                if let Some(prompt) = page.crash.as_mut() {
                                    prompt.details_expanded = !prompt.details_expanded;
                                }
                            }
                            ActionId::ReportError => {
                                if let Some(details) =
                                    page.crash.as_ref().and_then(|p| p.details.clone())
                                {
                                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(details));
                                }
                            }
                            ActionId::Accept | ActionId::Decline | ActionId::EditIndexEndpoint => {
                                page.crash = None;
                            }
                        }
                        cx.notify();
                    });
                },
            ));
        }
        let root = root
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                // Esc peels one layer at a time: the dialog on top first, then
                // the anchored menu, then the third column (desktop SPEC
                // keyboard map).
                if ks.key == "escape" && this.dismiss_top_dialog(cx) {
                    return;
                }
                if ks.key == "escape" && this.flows.last() == Some(&FlowPanel::Ds1) {
                    this.close_scanner(cx);
                    return;
                }
                if ks.key == "escape" {
                    if this.menu.is_some() {
                        this.menu = None;
                        cx.notify();
                    } else if this.panel != PanelId::None {
                        // Escape is the ✕: a signing column tells its core,
                        // as the web's `onclose` does, or the dApp's request
                        // hangs and every later one is refused as busy.
                        #[cfg(not(target_os = "linux"))]
                        if this.panel == PanelId::Signing
                            && let Some(host) = this.signing_host.clone()
                        {
                            host.update(cx, |host, cx| {
                                host.dispatch_sign(
                                    vela_core::app::sign_request::Event::SwipeDismissed,
                                    cx,
                                );
                            });
                        }
                        this.panel = PanelId::None;
                        cx.notify();
                    }
                }
                if ks.key == "f11" {
                    window.toggle_fullscreen();
                }
            }));

        // Square corners would poke out of the frame's rounded border.
        let root = match frame_tiling(window) {
            Some(tiling) => round_to_frame(root, tiling),
            None => root,
        };
        // Last child: the caption buttons paint over the page, not under it.
        let root = if caption {
            root.child(titlebar(&theme, window, px(CAPTION_H)))
        } else {
            root
        };

        window_frame(root, &theme, window)
    }
}

/// A dApp transaction landing in the signing column (078 G-04).
#[derive(Clone, Debug)]
struct DappLanding {
    op_hash: String,
    chain_id: u32,
    raised_at_ms: f64,
}

/// What the scanner read that it cannot act on (078 F-02).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ScanNotice {
    /// A code was read and is not a payment or an address.
    Unusable,
    /// A picked picture had no code in it.
    NothingFound,
}

/// The contacts route's copies (078 X-06): the toast's key, and the QR
/// dialog's pill's; the web holds both 1.5 s.
const CONTACTS_TOAST: &str = "contacts-toast";
const CONTACT_QR_COPY: &str = "contact-qr";
const CONTACTS_COPY_HOLD: std::time::Duration = std::time::Duration::from_millis(1500);

/// The Ethereum key backup, as a request for the SHARED signing sheet.
///
/// A pure function so the sheet it goes to can be pinned by a test: the backup
/// is an ordinary `eth_sendTransaction` on the wallet's own transport, which
/// means it gets the same "Sign with" row every other signature gets — the
/// core's five routes, the Trusted Signer among them (spec 075). A backup with a
/// sheet of its own would be the one signature a person could not route.
#[cfg(not(target_os = "linux"))]
fn backup_request(
    address: &str,
    call: &vela_core::registry_backup::BackupCall,
) -> crate::wallet::signing_host::IncomingRequest {
    let params = serde_json::json!([{
        "from": address,
        "to": call.to,
        "value": "0x0",
        "data": call.data,
    }]);
    crate::wallet::signing_host::IncomingRequest {
        id: format!("vela-ethereum-backup-{}", crate::executor::now_ms()),
        method: "eth_sendTransaction".to_owned(),
        params_json: params.to_string(),
        origin: "https://getvela.app".to_owned(),
        transport_id: crate::wallet::signing_host::WALLET_TRANSPORT.to_owned(),
        chain_id: call.chain_id,
        granted_address: None,
    }
}

/// The host of a signer page address, for its badge — the address itself is
/// already in the field under it.
fn page_host(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    rest.split(['/', '?', '#'])
        .next()
        .unwrap_or_default()
        .to_owned()
}

/// The Trusted Signer's caption, taken out of the list the "Sign with" sheet and
/// the Settings page both draw from.
///
/// Not a key of its own: the corpus's paths are pinned, and one way of signing
/// named twice is the drift spec 075 was raised over. The core always offers
/// the route (`wallet_keys::SIGN_METHODS`), which
/// `settings::tests::the_sign_with_words_resolve` pins, so the search finds it.
fn trusted_signer_words(s: &SettingsStrings) -> SharedString {
    s.sign_with_options
        .iter()
        .find(|(method, _)| *method == vela_core::trusted_signer::METHOD)
        .map(|(_, words)| words.clone())
        .unwrap_or_default()
}

/// Who is holding a key, for the line under its name in the keys list.
///
/// `lines` is the fallback trio in the core's method order — built-in passkey,
/// phone or tablet, security key — used when no catalog can name the vault.
///
/// Spec 075: a key minted on a Trusted Signer page ran its ceremony in a browser,
/// so the authenticator reports `platform` and the AAGUID catalog names
/// whatever vault answered on the page's own side. Both of those describe the
/// side of the page this wallet cannot reach, and the Android device pass of
/// 2026-09-22 found the result: a key made on the page, captioned as this
/// machine's built-in passkey — the opposite of where the key is. So the page
/// outranks both the report and the vault's name. The core decides it
/// (`WalletKeyRow::method` is `trusted_signer` exactly when the row carries a
/// `signer_origin`); this only says it.
fn key_holder(
    key: &vela_core::wallet_keys::WalletKeyRow,
    lines: &(SharedString, SharedString, SharedString),
    trusted_signer: &SharedString,
) -> SharedString {
    if key.method == vela_core::trusted_signer::METHOD {
        return trusted_signer.clone();
    }
    if !key.provider_name.is_empty() {
        return SharedString::from(key.provider_name.clone());
    }
    match key.method.as_str() {
        "security_key" => lines.2.clone(),
        "hybrid" => lines.1.clone(),
        _ => lines.0.clone(),
    }
}

/// Which page a key lives behind, for its details; empty when it lives behind
/// none.
///
/// "A Trusted Signer" is no answer to "where is my key" for a person who has used
/// two of them, so the row that says the route names the deployment as well.
fn key_page(key: &vela_core::wallet_keys::WalletKeyRow) -> String {
    key.signer_origin
        .clone()
        .filter(|origin| !origin.is_empty())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Linux has three destinations, as the web does** (owner call,
    /// 2026-09-24): no in-app browser, so no Explore — not in the sidebar, and
    /// not through a pinned or restored place. Everywhere else, four.
    #[test]
    fn explore_is_a_destination_only_where_there_is_a_browser() {
        let linux = cfg!(target_os = "linux");
        assert_eq!(Section::Explore.available(), !linux);
        assert_eq!(Section::Explore.or_wallet() == Section::Wallet, linux);
        for section in [Section::Wallet, Section::Contacts, Section::Settings] {
            assert!(section.available());
            assert_eq!(section.or_wallet(), section);
        }
    }

    /// **A key minted on a Trusted Signer page is captioned as the page, and says
    /// which page** (spec 075, the Android device pass of 2026-09-22).
    ///
    /// The pass found such a key drawn as the phone's built-in passkey, because
    /// a page runs its ceremony in a browser and the authenticator therefore
    /// reports `platform` — the far side of the page, the one side this wallet
    /// cannot reach. The row is built here from the account record's own keys,
    /// so the whole chain is walked: a `DeviceKey` carrying the origin the
    /// record stored, through the core's row builder, to the two lines a person
    /// reads. And the caption is the "Sign with" sheet's own words, so the keys
    /// list and the picker never name one route two ways.
    #[test]
    fn a_key_behind_a_page_is_captioned_as_the_trusted_signer() {
        let loc = Loc::from_env();
        let s = SettingsStrings::resolve(&loc);
        let lines = (
            s.keys_provider_platform.clone(),
            s.keys_provider_generic.clone(),
            s.keys_provider_security_key.clone(),
        );
        let trusted_signer = super::trusted_signer_words(&s);
        assert!(
            !trusted_signer.is_empty(),
            "the Trusted Signer's caption went missing from the sheet's list"
        );
        assert_eq!(
            Some(trusted_signer.clone()),
            SigningStrings::resolve(&loc)
                .sign_with_options
                .iter()
                .find(|(method, _)| *method == vela_core::trusted_signer::METHOD)
                .map(|(_, words)| words.clone()),
            "the keys list and the sheet disagree about what this route is called"
        );

        // As `ensure_backup_check` builds them: one key behind a page, one on
        // the end of a cable.
        let page = "https://sign.example.test";
        let device = [
            vela_core::wallet_keys::DeviceKey {
                credential_id: "cred0".to_owned(),
                public_key_hex: "04".to_owned() + &"ab".repeat(64),
                name: "Behind the page".to_owned(),
                transports: String::new(),
                signer_origin: Some(page.to_owned()),
            },
            vela_core::wallet_keys::DeviceKey {
                credential_id: "cred1".to_owned(),
                public_key_hex: "04".to_owned() + &"cd".repeat(64),
                name: "On the desk".to_owned(),
                transports: "usb".to_owned(),
                signer_origin: None,
            },
        ];
        let rows = match vela_core::wallet_keys::step("", &device, &[]) {
            vela_core::wallet_keys::KeysStep::Done { keys, .. } => keys,
            vela_core::wallet_keys::KeysStep::Ask { .. } => {
                unreachable!("asked with no address, so nobody can be asked")
            }
        };
        assert_eq!(rows.len(), 2);

        // The key behind the page: captioned as the route, naming the page.
        assert_eq!(rows[0].method, vela_core::trusted_signer::METHOD);
        assert_eq!(
            super::key_holder(&rows[0], &lines, &trusted_signer),
            trusted_signer
        );
        assert_ne!(
            super::key_holder(&rows[0], &lines, &trusted_signer),
            lines.0,
            "the page's key is drawn as this device's built-in passkey"
        );
        assert_eq!(super::key_page(&rows[0]), page);

        // Even when a catalog names the vault that answered: that vault is on
        // the far side of the page, and where the key lives is the page.
        let named = vela_core::wallet_keys::WalletKeyRow {
            provider_name: "Apple Passwords".to_owned(),
            ..rows[0].clone()
        };
        assert_eq!(
            super::key_holder(&named, &lines, &trusted_signer),
            trusted_signer
        );

        // And an ordinary key is untouched: the USB key still reads as one, and
        // its details grow no page line.
        assert_eq!(
            super::key_holder(&rows[1], &lines, &trusted_signer),
            lines.2
        );
        assert!(super::key_page(&rows[1]).is_empty());
        let vaulted = vela_core::wallet_keys::WalletKeyRow {
            provider_name: "1Password".to_owned(),
            ..rows[1].clone()
        };
        assert_eq!(
            super::key_holder(&vaulted, &lines, &trusted_signer).as_ref(),
            "1Password"
        );
    }

    /// **The Ethereum key backup gets the Trusted Signer too** (spec 075: "创建/
    /// 登录/转账/dapp签名/公钥备份 等" — the owner listed the backup with the rest).
    ///
    /// It gets it by being an ordinary request on the shared signing sheet
    /// rather than a sheet of its own: the same `eth_sendTransaction` on the
    /// wallet's own transport that a send is, so the same "Sign with" row with
    /// the same five routes. If this ever grew its own sheet, the backup would
    /// be the one signature a person could not route — and the backup is the
    /// signature that matters most to a wallet living behind a signer page.
    #[test]
    #[cfg(not(target_os = "linux"))]
    fn the_key_backup_goes_to_the_shared_signing_sheet() {
        let call = vela_core::registry_backup::BackupCall {
            chain_id: 1,
            to: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
            value: "0".to_owned(),
            data: "0xabcdef".to_owned(),
        };
        let address = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let request = backup_request(address, &call);
        assert_eq!(request.method, "eth_sendTransaction");
        assert_eq!(
            request.transport_id,
            crate::wallet::signing_host::WALLET_TRANSPORT,
            "the backup is the wallet's own request, not a site's"
        );
        assert_eq!(request.granted_address, None);
        assert_eq!(request.chain_id, 1);
        // The calls the sheet reads out of it are the backup's own.
        let calls = vela_core::sign_message::is_message_method(&request.method);
        assert!(!calls, "a backup is submitted, not signed as a message");
        assert!(request.params_json.contains(&call.data));
        assert!(request.params_json.contains(address));
        // And the row it lands under offers every route the core knows.
        assert!(
            vela_core::wallet_keys::SIGN_METHODS.contains(&vela_core::trusted_signer::METHOD),
            "the shared sheet does not offer the Trusted Signer"
        );
    }

    /// The save button is available exactly when the address is one.
    ///
    /// The core refuses a malformed address anyway; saying so before the press
    /// is what keeps the button from looking available for something it will
    /// not do.
    #[test]
    fn only_a_real_address_can_be_saved() {
        assert!(super::is_evm_address(
            "0x88cCA0EeDbF2C4426110bbFc998F048689266894"
        ));
        assert!(super::is_evm_address(
            "0X88cca0eedbf2c4426110bbfc998f048689266894"
        ));
        // One character short, one over, non-hex, no prefix, empty.
        assert!(!super::is_evm_address(
            "0x88cCA0EeDbF2C4426110bbFc998F04868926689"
        ));
        assert!(!super::is_evm_address(
            "0x88cCA0EeDbF2C4426110bbFc998F0486892668944"
        ));
        assert!(!super::is_evm_address(
            "0xZZcCA0EeDbF2C4426110bbFc998F048689266894"
        ));
        assert!(!super::is_evm_address(
            "88cCA0EeDbF2C4426110bbFc998F048689266894"
        ));
        assert!(!super::is_evm_address(""));
    }

    /// FR-004 / data-model.md §Screen states: the gallery chip strip exposes
    /// exactly the desktop state inventory, in canon order, each reachable in
    /// one click from the gallery root. `dc2n` is deliberately absent — the
    /// window minimum is 1280 wide, so the narrow overlay is unreachable on
    /// native desktop (research.md D6).
    #[test]
    fn gallery_exposes_every_desktop_contacts_state() {
        let codes: Vec<&str> = GalleryTab::ALL
            .iter()
            .filter_map(|(tab, _)| tab.contacts_state())
            .collect();
        assert_eq!(codes, crate::contacts::fixtures::DESKTOP_STATES);

        // Chip labels are gallery chrome and stay untranslated (spec 018).
        let labels: Vec<&str> = GalleryTab::ALL.iter().map(|(_, label)| *label).collect();
        assert_eq!(
            labels,
            [
                "D1",
                "D1b",
                "D2",
                "D3",
                "DC1",
                "DC2",
                "DC3",
                "DC4",
                "DC5",
                "DC6",
                "DST1",
                "DST2",
                "DST3",
                "DST4",
                "DST4b",
                "DST5",
                "DST6",
                "DST7",
                "DST8",
                "DSR1",
                "Components",
                "Contacts",
                "Identicons",
            ]
        );
    }

    /// Spec 023's half of the same contract: one chip per settings mock, in
    /// the order `settings::fixtures::DESKTOP_STATES` declares.
    #[test]
    fn gallery_exposes_every_desktop_settings_state() {
        let codes: Vec<&str> = GalleryTab::ALL
            .iter()
            .filter_map(|(tab, _)| tab.settings_state())
            .collect();
        assert_eq!(codes, crate::settings::fixtures::DESKTOP_STATES);
    }

    /// The Trusted Signer page's badge names where the page is; the address
    /// itself is in the field under it.
    #[test]
    fn a_signer_page_badge_is_its_host() {
        assert_eq!(
            page_host("https://sign.example.com/vela/"),
            "sign.example.com"
        );
        assert_eq!(page_host("http://localhost:8140/"), "localhost:8140");
        assert_eq!(page_host("https://[::1]:9/?x#y"), "[::1]:9");
    }

    /// The second-level nav is the phone's settings list with the rows
    /// collapsed to their titles — same ids, same order, so somebody who
    /// learned one knows the other.
    #[test]
    fn settings_nav_covers_every_panel() {
        assert_eq!(SettingsPage::ALL.len(), 10);
        assert_eq!(SettingsPage::ALL[0], SettingsPage::Account);
        assert_eq!(SettingsPage::ALL[6], SettingsPage::FeeSpeed);
        // "Sign with" beside the speed (spec 071).
        assert_eq!(SettingsPage::ALL[7], SettingsPage::Signing);
        assert_eq!(SettingsPage::ALL[9], SettingsPage::About);
    }

    /// A latency under a second reads "45ms" in the ok tone; a slow one flips
    /// to seconds AND to warning, because "1.2s" beside "45ms" is otherwise a
    /// smaller-looking number.
    #[test]
    fn latency_pill_changes_unit_and_tone_at_one_second() {
        let fast = latency(45, None);
        assert_eq!(fast.label.as_ref(), "45ms");
        assert_eq!(fast.tone, Tone::Ok);

        let slow = latency(1200, None);
        assert_eq!(slow.label.as_ref(), "1.2s");
        assert_eq!(slow.tone, Tone::Warn);
    }

    /// The desktop list drops the two networks DST4 puts below the fold, and
    /// keeps the custom tail — which is the one row with a bin on it.
    #[test]
    fn desktop_network_list_matches_the_mock() {
        let ids = crate::settings::fixtures::DESKTOP_NETWORK_IDS;
        assert_eq!(ids.len(), 6);
        assert!(!ids.contains(&"gnosis"));
        assert!(!ids.contains(&"tempo"));
        assert!(crate::settings::fixtures::network("xlayer").custom);
        assert!(!crate::settings::fixtures::network("ethereum").custom);
    }

    /// Every id in the fixture list resolves — `network()` panics otherwise,
    /// and it is called from the render path.
    #[test]
    fn every_fixture_network_id_resolves() {
        for id in crate::settings::fixtures::DESKTOP_NETWORK_IDS {
            assert_eq!(crate::settings::fixtures::network(id).id, id);
        }
        for id in crate::settings::fixtures::BANNER_CHAINS {
            assert_eq!(crate::settings::fixtures::network(id).id, id);
        }
    }
}

/// -32002, "a request is already open": the site may ask again. What a site
/// hears when the column is busy with the wallet's OWN request, which the
/// browser machine cannot see.
fn busy_answer() -> SignResponsePayload {
    SignResponsePayload::Err {
        code: -32002,
        kind: SignErrorKind::SubmitFailed,
        message: Some("Another signing request is already open in Vela".to_owned()),
    }
}

/// Is this a well-formed EVM address?
///
/// The core refuses a malformed one anyway; saying so before the press is what
/// keeps the save button from looking available for something it will not do.
fn is_evm_address(value: &str) -> bool {
    value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .is_some_and(|body| body.len() == 40 && body.bytes().all(|b| b.is_ascii_hexdigit()))
}

/// What a pick list is choosing, and for whom.
#[derive(Clone)]
enum PickSubject {
    /// 添加成员: which contacts this group holds.
    Members { group_id: String },
    /// 移入分组: which groups hold this contact.
    Groups { address: String },
}

/// One row of a pick list. `seed` is the address behind a contact's
/// identicon; a group has none and wears the users tile.
struct PickRow {
    id: String,
    name: SharedString,
    detail: SharedString,
    seed: Option<String>,
}

/// The tick list's draft: the ids ticked NOW, which the core hears only on
/// Save — a dismissed list changes nothing, as the web's.
#[derive(Clone)]
struct PickDialog {
    subject: PickSubject,
    checked: Vec<String>,
    query: String,
}

/// The add/edit contact sheet's draft.
#[derive(Clone, Default)]
struct ContactForm {
    /// `true` when the address is fixed — an edit, not an add.
    editing: bool,
    address: String,
    name: String,
    /// The row is the core's history suggestion (`ContactSource::Auto`):
    /// saving is what makes it a contact, and the title says so.
    unsaved: bool,
}
