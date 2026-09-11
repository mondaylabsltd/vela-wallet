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
use crate::executor::passkey::WindowHandle;
use crate::hardware;
use crate::settings::components::{
    CalloutTone, callout, chain_mark, check_list, danger_card, dropdown_menu, dropdown_trigger,
    editable_url_field, form_row, key_value_row, network_row, rpc_banner, segmented,
    settings_nav_row, status_pill, storage_bar, storage_group, text_scale, url_field,
};
use crate::settings::fixtures::{self as settings_fixtures, SettingsPage, Tone, latency, pill};
use crate::settings::live as settings_live;
use crate::settings::model::NetworkRowModel;
use crate::signing::SigningStrings;
use crate::signing::components as signing_components;
use crate::signing::fixtures as signing_fixtures;
use crate::signing::live as signing_live;
use crate::theme::{
    self, CONTACTS_BODY_PAD_TOP, CONTACTS_BUTTON_H, CONTACTS_HEADER_H, CONTACTS_HERO_AVATAR,
    CONTACTS_RAIL_LABEL_H, CONTACTS_RAIL_ROW_H, CONTACTS_RAIL_W, GALLERY_BAR_H, SETTINGS_DIALOG_W,
    SETTINGS_NAV_W, SETTINGS_PANEL_PAD_X, SETTINGS_PANEL_W, SIDEBAR_PAD, SIDEBAR_TOP, SIDEBAR_W,
    THIRD_PANEL_W, Theme, ThemeMode, WALLET_PAD_TOP, WALLET_PAD_X,
};
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
use vela_core::app::display_currency::DisplayCurrency;
use vela_core::app::explore_sites::ExploreSites;
use vela_core::app::fee_policy::Event as FeeEvent;
use vela_core::app::manage_tokens::{Event as MtokEvent, ManageTokens, MtokNetwork};
use vela_core::app::network_admin::{Event as NetEvent, NetOverrideField, NetworkAdmin};
use vela_core::app::payment_request::PaymentRequest;
use vela_core::app::receive_watch::ReceiveWatch;
use vela_core::app::send::{
    Event as SendEvent, SendAlertKind, SendDisplayContext, SendOpenParams, SendRecipientDraft,
};

use super::WalletStrings;
use super::components::{
    action_pill, activity_row, asset_row, balance_display, chain_row, empty_state, icon_img,
    identicon_avatar, nav_row, qr_placeholder, section_header, section_header_parts,
    section_header_row, sidebar_search, skeleton_row, token_icon, wallet_header,
};
use super::fixtures::{self, ADDRESS_FULL, IDENTICON_BOARD_SEEDS, WALLET_NAME};
use crate::flows::components::mono_field;
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SettingsDialog {
    /// DST4b — search a chain, check it, add it.
    AddNetwork,
    /// DSR1 — one network's RPC is down and this is where it gets fixed.
    FixRpc,
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
    /// Spec 034 — which contacts this group holds, ticked. The same question
    /// as `ContactGroups`, asked from the group's screen.
    GroupMembers,
    /// Spec 034 — which groups this contact is in, ticked. A menu rather than
    /// a dialog for the same reason the explore one is: the question is
    /// "which of these", and the answer is visible on every row.
    ContactGroups,
    /// Spec 032 phase 41 — "move to a group", listing the person's own
    /// groups. A menu rather than a new picker component: the question is
    /// "which of these", which is what a menu is.
    MoveGroup,
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
    section: Section,
    /// Which settings panel the second-level nav is showing (spec 023).
    settings_page: SettingsPage,
    /// The centred dialog over the settings section, when one is open.
    settings_dialog: Option<SettingsDialog>,
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
    /// Spec 032: the send journey's two machines, alive while the flow is
    /// open and discarded with it — a second send starts from a fresh
    /// machine, never a resumed one.
    send_host: Option<gpui::Entity<SendHost>>,
    /// The signing journey's four machines, born with a dApp request and gone
    /// when the core hides the sheet. `None` means the panel draws its mock,
    /// which is what the gallery and an unsigned-in window get.
    #[cfg(not(target_os = "linux"))]
    signing_host: Option<gpui::Entity<crate::wallet::signing_host::SigningHost>>,
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
    /// The permissions machine for the browser column. Born with the first
    /// request a page makes, because that is the first moment there is an
    /// origin to judge.
    #[cfg(not(target_os = "linux"))]
    browser_host: Option<gpui::Entity<crate::wallet::browser_host::BrowserHost>>,
    /// The request sink is installed once per page, not once per frame.
    #[cfg(not(target_os = "linux"))]
    dapp_requests_armed: bool,
    /// `VELA_BROWSER_URL` is applied once, not on every frame the column draws.
    browser_url_pinned: bool,
    send_amount_focus: gpui::FocusHandle,
    send_recipient_focus: gpui::FocusHandle,
    /// DSD2cL, live: the rate field's focus.
    send_rate_focus: gpui::FocusHandle,
    /// DSD2bL, live: one focus handle per split row, made on first use. A
    /// shared handle would send every keystroke to whichever row drew last.
    split_focuses: Vec<gpui::FocusHandle>,
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
    /// The native window, for the one platform whose passkey dialog is the
    /// OS's; captured once, because a ceremony runs off the main thread and
    /// cannot reach `Window` from there.
    window_handle: WindowHandle,
    /// The resolved locale, kept for the cable's own dialogs (touch / PIN /
    /// pick), which take it whole.
    loc: Loc,
    /// One per editable settings field, made on first use.
    endpoint_focuses: Vec<gpui::FocusHandle>,
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
    /// What the last address-book import did, as a title and a line. Cleared
    /// by acknowledging it.
    import_result: Option<(SharedString, SharedString)>,
    /// The add/edit contact sheet: `Some((address, name))` while it is open.
    ///
    /// The ADDRESS is the identity — the core keys on it and an edit that
    /// changed it would be a delete and an add wearing one button. So editing
    /// an existing contact keeps it fixed and only the name is a draft.
    contact_form: Option<ContactForm>,
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
    /// The chain the in-app BROWSER is on: what a connected site was told by
    /// `eth_chainId`, what `wallet_switchEthereumChain` moves, and the chain a
    /// signature from that site is quoted, routed and submitted on.
    ///
    /// Its own field since spec 037. It used to BE `receive_chain`, and the
    /// two meanings had nothing to do with each other: tapping "Ethereum" on
    /// the receive screen silently retargeted a connected dApp's next
    /// signature to Ethereum, and a site's chain switch silently changed which
    /// chain your receive QR was for. Same default — Gnosis, the chain this
    /// wallet is cheapest on — and nothing else shared.
    browser_chain: u32,
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
    /// DC3: the fixture roster is empty.
    contacts_empty: bool,
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
    /// DSD2eL: each group's member addresses, in drawn order.
    group_members: Vec<Vec<String>>,
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
        };
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
            browser_chain: 100,
            celebrating: false,
            chain_filter: None,
            feed_privacy: None,
            tx_detail: None,
            asset_detail: None,
            add_token_focus: cx.focus_handle(),
            send_host: None,
            #[cfg(not(target_os = "linux"))]
            signing_host: None,
            menu_origin: None,
            explore_form: None,
            explore_form_focus: cx.focus_handle(),
            browser_title: None,
            cap_focus: cx.focus_handle(),
            #[cfg(not(target_os = "linux"))]
            browser_host: None,
            #[cfg(not(target_os = "linux"))]
            dapp_requests_armed: false,
            browser_url_pinned: false,
            send_amount_focus: cx.focus_handle(),
            send_recipient_focus: cx.focus_handle(),
            send_rate_focus: cx.focus_handle(),
            split_focuses: Vec::new(),
            send_fee_picker: false,
            send_sweeping: false,
            scan_camera: None,
            scan_preview: ScanPreview::default(),
            window_handle: crate::onboarding::native_window_handle(window),
            endpoint_focuses: Vec::new(),
            settings_probed_network: None,
            settings_probed_panel: None,
            settings_fix_chain: None,
            network_remove: None,
            import_result: None,
            contact_form: None,
            group_form: None,
            group_form_focus: cx.focus_handle(),
            contact_form_name_focus: cx.focus_handle(),
            contact_form_address_focus: cx.focus_handle(),
            locale: gpui::SharedString::from(loc.language().to_owned()),
            explore,
            signing,
            browsing: false,
            signing_state: "cs12",
            settings_page: SettingsPage::Account,
            settings_dialog: None,
            settings_expanded_network: None,
            settings_open_dropdown: None,
            group: None,
            contact: 0,
            inspected_contact: None,
            switcher_addresses: None,
            contacts_empty: false,
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
        }
    }

    /// The way out.
    ///
    /// Session state is app-resident and `allowed_route` decides the screen, so
    /// without this row a signed-in desktop has no path back to Welcome at all
    /// — the route guard is a one-way door. It renders only for a REAL session:
    /// the fixture identity (`VELA_PAGE=wallet`) is a design surface with no
    /// session behind it, and offering to sign out of nothing would be a button
    /// that cannot work.
    fn sign_out_row(&self, theme: &Theme, cx: &mut Context<Self>) -> gpui::AnyElement {
        if self.identity.is_none() {
            return div().into_any_element();
        }
        let hover = theme.bg_sunken;
        div()
            .id("sign-out")
            .mt(px(4.))
            .px(px(12.))
            .py(px(8.))
            .rounded(px(8.))
            .flex_none()
            .cursor_pointer()
            .text_size(theme::text_row_sub())
            .text_color(theme.fg_muted)
            .hover(move |style| style.bg(hover).text_color(theme.error_base))
            .on_click(cx.listener(|_, _, _, cx| session::sign_out(cx)))
            .child(self.strings.sign_out_button.clone())
            .into_any_element()
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
            network_ticker: network.chars().take(3).collect::<String>().to_uppercase(),
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
        };
        // A name that is only spaces is not a name — the core refuses it, and
        // an armed Save that the core would drop is a button that lies.
        let can_save = !form.text.trim().is_empty();
        let hover_accent = theme.accent_hover;
        let focus = self.explore_form_focus.clone();
        let strings = crate::ui::NameFieldStrings {
            label: title.clone(),
            placeholder: SharedString::from(""),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let cancel = c.cancel.clone();
        let save_label = c.save.clone();

        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
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
            )
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
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .id("explore-form-cancel")
                            .flex_1()
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
                            .child(cancel)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.explore_form = None;
                                cx.notify();
                            })),
                    )
                    .child({
                        let save = div()
                            .id("explore-form-save")
                            .flex_1()
                            .h(px(CONTACTS_BUTTON_H))
                            .rounded(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(save_label);
                        if can_save {
                            save.cursor_pointer()
                                .bg(theme.accent)
                                .hover(move |el| el.bg(hover_accent))
                                .text_color(theme.fg_inverse)
                                .on_click(cx.listener(|this, _, _, cx| this.save_explore_form(cx)))
                        } else {
                            save.bg(theme.bg_sunken).text_color(theme.fg_subtle)
                        }
                    }),
            );

        Some(
            div()
                .id("explore-form-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.bg_base.opacity(0.55))
                .child(card)
                .into_any_element(),
        )
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
        let hover_accent = theme.accent_hover;
        let focus = self.group_form_focus.clone();
        let strings = crate::ui::NameFieldStrings {
            label: s.group_name_label.clone(),
            placeholder: s.group_name_placeholder.clone(),
            helper: SharedString::from(""),
            too_long_hint: SharedString::from(""),
        };
        let cancel = s.cancel.clone();
        let save_label = s.save.clone();

        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
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
            )
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
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .child(
                        div()
                            .id("group-form-cancel")
                            .flex_1()
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
                            .child(cancel)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.group_form = None;
                                cx.notify();
                            })),
                    )
                    .child({
                        let save = div()
                            .id("group-form-save")
                            .flex_1()
                            .h(px(CONTACTS_BUTTON_H))
                            .rounded(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .text_size(theme::text_row_title())
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(save_label);
                        if can_save {
                            save.cursor_pointer()
                                .bg(theme.accent)
                                .hover(move |el| el.bg(hover_accent))
                                .text_color(theme.fg_inverse)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let Some((id, name)) = this.group_form.take() else {
                                        return;
                                    };
                                    resident::resident::<Contacts>(cx).update(
                                        cx,
                                        |resident, cx| {
                                            resident.dispatch(
                                                ContactEvent::GroupSave {
                                                    input: ContactGroupInput {
                                                        id,
                                                        name: name.trim().to_owned(),
                                                        color: None,
                                                        // `None` leaves membership
                                                        // alone — a rename must not
                                                        // empty the group.
                                                        members: None,
                                                    },
                                                },
                                                cx,
                                            );
                                        },
                                    );
                                    cx.notify();
                                }))
                        } else {
                            save.bg(theme.bg_sunken).text_color(theme.fg_subtle)
                        }
                    }),
            );

        Some(
            div()
                .id("group-form-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.bg_base.opacity(0.55))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card)
                .into_any_element(),
        )
    }

    /// The add/edit contact sheet.
    ///
    /// 030 recorded "there is no add/edit form sheet on desktop" as a design
    /// gap. By 031 the app had a dialog idiom (four of them), an editable text
    /// field and every word this form needs already in the corpus — so what was
    /// left was composition, not design.
    ///
    /// The ADDRESS is the identity the core keys on, so an edit keeps it fixed:
    /// changing it would be a delete and an add wearing one button, and the old
    /// contact would quietly survive.
    fn contact_form_dialog(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let form = self.contact_form.clone()?;
        let s = &self.contacts;
        let title = if form.editing {
            s.edit_title.clone()
        } else {
            s.add_title.clone()
        };
        // The core refuses a malformed address anyway; this is the same rule
        // said before the press rather than after it, so the button does not
        // look available for something it will not do.
        let can_save = is_evm_address(&form.address);
        let hover_accent = theme.accent_hover;
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
            too_long_hint: SharedString::from(""),
        };

        let mut card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
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
            )
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
            // Fixed, and shown as such: this is what the panel is about.
            card = card.child(mono_field(
                theme,
                Some(s.address_label.clone()),
                SharedString::from(form.address.clone()),
            ));
        } else {
            card = card.child(crate::ui::text_field(
                "contact-form-address",
                theme,
                &address_strings,
                &form.address,
                // Red once there is something typed that is not an address —
                // not while the field is still empty, which is a person who has
                // not started rather than one who is wrong.
                !form.address.is_empty() && !can_save,
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

        let buttons = div()
            .flex()
            .gap(px(12.))
            .child(
                div()
                    .id("contact-form-cancel")
                    .flex_1()
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
                    .child(s.cancel.clone())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.contact_form = None;
                        cx.notify();
                    })),
            )
            .child({
                let save = div()
                    .id("contact-form-save")
                    .flex_1()
                    .h(px(CONTACTS_BUTTON_H))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(s.save.clone());
                if can_save {
                    save.cursor_pointer()
                        .bg(theme.accent)
                        .hover(move |el| el.bg(hover_accent))
                        .text_color(theme.fg_inverse)
                        .on_click(cx.listener(|this, _, _, cx| {
                            let Some(form) = this.contact_form.take() else {
                                return;
                            };
                            let now_ms = crate::executor::now_ms();
                            resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                                resident.dispatch(
                                    ContactEvent::Save {
                                        input: ContactSaveInput {
                                            address: form.address,
                                            // An empty name CLEARS the name —
                                            // the core reads `Some("")` that
                                            // way, and a person who deleted the
                                            // text meant to.
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
                            cx.notify();
                        }))
                } else {
                    // Dimmed, not hidden: the button is where it will be, and
                    // the address field beside it says what is missing.
                    save.bg(theme.bg_sunken).text_color(theme.fg_subtle)
                }
            });

        Some(
            div()
                .id("contact-form-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.bg_base.opacity(0.55))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card.child(buttons))
                .into_any_element(),
        )
    }

    /// What the last address-book import did.
    ///
    /// The RN screen alerts; the desktop has a centred-dialog idiom already, so
    /// this reuses the sign-out dialog's shape with one button. The counts are
    /// the CORE's — it applied existing-wins and is the only thing that knows
    /// how many rows were new.
    fn import_result_dialog(
        &self,
        theme: &Theme,
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (title, body) = self.import_result.clone()?;
        let hover_accent = theme.accent_hover;
        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
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
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.fg_muted)
                    .child(body),
            )
            .child(
                div()
                    .id("import-result-ok")
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
                    .child(self.flow_strings.done.clone())
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.import_result = None;
                        cx.notify();
                    })),
            );
        Some(
            div()
                .id("import-result-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.bg_base.opacity(0.55))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card)
                .into_any_element(),
        )
    }

    fn sign_out_dialog(&self, theme: &Theme, cx: &mut Context<Self>) -> Option<gpui::AnyElement> {
        let view = session::view(cx);
        let dialog = view.sign_out?;
        let s = &self.strings;

        let mut card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
            .rounded(px(20.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border_card)
            .child(
                div()
                    .text_size(theme::text_panel_title())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(s.sign_out_title.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.fg_muted)
                    .child(s.sign_out_keeps.clone()),
            );

        if dialog.pending_upload_warning {
            card = card.child(
                div()
                    .p(px(12.))
                    .rounded(px(10.))
                    .bg(theme.warning_soft)
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.fg_base)
                    .child(s.sign_out_warning.clone()),
            );
        }

        // The destructive label changes with the warning, as the shipping
        // client does: "Sign Out Anyway" is the acknowledgement.
        let confirm_label = if dialog.pending_upload_warning {
            s.sign_out_anyway.clone()
        } else {
            s.sign_out_title.clone()
        };
        let hover_confirm = theme.error_base;
        let hover_cancel = theme.bg_sunken;
        card = card.child(
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(
                    div()
                        .id("sign-out-confirm")
                        .h(px(44.))
                        .rounded(px(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .bg(theme.error_soft)
                        .text_size(theme::text_row_title())
                        .text_color(theme.error_base)
                        .hover(move |style| style.bg(hover_confirm).text_color(theme.fg_inverse))
                        .on_click(cx.listener(|_, _, _, cx| session::sign_out_confirmed(cx)))
                        .child(confirm_label),
                )
                .child(
                    div()
                        .id("sign-out-cancel")
                        .h(px(44.))
                        .rounded(px(12.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_pointer()
                        .text_size(theme::text_row_title())
                        .text_color(theme.fg_base)
                        .hover(move |style| style.bg(hover_cancel))
                        .on_click(cx.listener(|_, _, _, cx| session::sign_out_dismissed(cx)))
                        .child(s.sign_out_cancel.clone()),
                ),
        );

        Some(
            div()
                .id("sign-out-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card)
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
        cx: &mut Context<Self>,
    ) -> Option<gpui::AnyElement> {
        let (id, name) = self.network_remove.clone()?;
        let s = &self.settings;
        let hover_confirm = theme.error_base;
        let hover_cancel = theme.bg_sunken;
        let card = div()
            .w(px(400.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .p(px(28.))
            .rounded(px(20.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border_card)
            .child(
                div()
                    .text_size(theme::text_panel_title())
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(s.network_remove_title.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .line_height(px(20.))
                    .text_color(theme.fg_muted)
                    // The corpus asks "Remove this custom network?"; the name
                    // says WHICH, because the dialog covers the row it is about.
                    .child(SharedString::from(format!(
                        "{} · {name}",
                        s.network_remove_body
                    ))),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(8.))
                    .child(
                        div()
                            .id("network-remove-confirm")
                            .h(px(44.))
                            .rounded(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .bg(theme.error_soft)
                            .text_size(theme::text_row_title())
                            .text_color(theme.error_base)
                            .hover(move |style| {
                                style.bg(hover_confirm).text_color(theme.fg_inverse)
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                resident::resident::<NetworkAdmin>(cx).update(
                                    cx,
                                    |resident, cx| {
                                        resident.dispatch(
                                            NetEvent::DeleteConfirmed { id: id.clone() },
                                            cx,
                                        );
                                    },
                                );
                                // The hero was counting that chain a moment ago.
                                crate::executor::balance_dashboard::refresh(cx);
                                this.network_remove = None;
                                this.settings_expanded_network = None;
                                cx.notify();
                            }))
                            .child(s.network_remove_confirm.clone()),
                    )
                    .child(
                        div()
                            .id("network-remove-cancel")
                            .h(px(44.))
                            .rounded(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .hover(move |style| style.bg(hover_cancel))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.network_remove = None;
                                cx.notify();
                            }))
                            .child(s.network_remove_cancel.clone()),
                    ),
            );

        Some(
            div()
                .id("network-remove-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card)
                .into_any_element(),
        )
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
        let bg = match self.theme_mode() {
            ThemeMode::Light => theme.bg_sunken,
            ThemeMode::Dark => theme.bg_base,
        };

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
        for (i, (icon, label, destination)) in nav.into_iter().enumerate() {
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

        let mut networks = div().flex().flex_col().gap(px(2.)).flex_1().min_h(px(0.));
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
                wallet_header(
                    theme,
                    &mut self.icons,
                    &mut self.identicons,
                    &identity.address,
                    identity.name.clone(),
                    identity.display(),
                )
            })
            .child(nav_col)
            .child(div().h(px(1.)).bg(theme.divider))
            .child(
                div()
                    .px(px(12.))
                    .text_size(theme::text_label())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_subtle)
                    .child(self.strings.networks_title.clone()),
            )
            .child(networks)
            .child(self.sign_out_row(theme, cx))
            .child(sidebar_search(
                theme,
                &mut self.icons,
                self.strings.search_placeholder.clone(),
            ))
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

        div()
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_hidden()
            .px(px(WALLET_PAD_X))
            .pt(px(WALLET_PAD_TOP))
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
                    }) as crate::wallet::components::BalanceToggle
                }),
            ))
            .child(pills)
            .child(
                div()
                    .id("section-activity")
                    .cursor_pointer()
                    .child(section_header(theme, &mut self.icons, s_activity, s_all))
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
                let (title, action) = section_header_parts(theme, &mut self.icons, s_assets, s_add);
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
            .child(assets_col)
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

    fn contacts_header(&mut self, theme: &Theme, caption: bool, cx: &mut Context<Self>) -> Div {
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
            .child(search_field(theme, &mut self.icons, placeholder))
            .child(
                outline_button(
                    "contacts-add",
                    theme,
                    &mut self.icons,
                    Some(Icon::UserRoundPlus),
                    add,
                )
                .on_click(cx.listener(|this, _, window, cx| {
                    // Only a real session saves anything: the fixture roster is
                    // a picture, and a picture must not grow a row.
                    if this.identity.is_none() {
                        return;
                    }
                    this.contact_form = Some(ContactForm::default());
                    // The address is the first thing to type, so it is the
                    // first thing focused.
                    window.focus(&this.contact_form_address_focus, cx);
                    cx.notify();
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

    fn contacts_rail(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let all = self.contacts.all_contacts.clone();
        let groups_label = self.contacts.section_groups.clone();
        let new_group = self.contacts.group_new.clone();
        // The count beside 全部联系人. A real session counts its own book; the
        // mock's 12 under somebody's four contacts is the same small lie the
        // group rail told.
        let total = if self.contacts_empty {
            0
        } else if self.identity.is_some() {
            u32::try_from(
                self.contact_sections(cx)
                    .iter()
                    .map(|(_, rows)| rows.len())
                    .sum::<usize>(),
            )
            .unwrap_or(u32::MAX)
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
        self.contact_sections(cx)
            .into_iter()
            .flat_map(|(_, rows)| rows)
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
        contacts_live::detail(&view, self.contact, &feed, &self.strings, hidden)
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
        wallet_live::activity_rows(&feed, &self.strings, hidden)
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

    /// Every group, and whether the open contact is in it.
    ///
    /// The pair is what the menu draws AND what the next tap sends back, so
    /// the tick a person sees and the set the core is given cannot disagree.
    fn contact_group_state(&mut self, cx: &mut Context<Self>) -> Vec<(SharedString, bool)> {
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let Some(address) = contacts_live::rows(&view)
            .into_iter()
            .nth(self.contact)
            .map(|row| row.address_full.to_string().to_lowercase())
        else {
            return Vec::new();
        };
        view.groups
            .iter()
            .map(|group| {
                (
                    SharedString::from(group.name.clone()),
                    group
                        .members
                        .iter()
                        .any(|member| member.address.to_lowercase() == address),
                )
            })
            .collect()
    }

    /// Every contact, and whether the open group holds it.
    ///
    /// The book's own order, so the menu reads like the list behind it.
    fn group_member_state(&mut self, cx: &mut Context<Self>) -> Vec<(SharedString, bool)> {
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        let Some(group) = self.group.and_then(|index| view.groups.get(index)) else {
            return Vec::new();
        };
        let members: Vec<String> = group
            .members
            .iter()
            .map(|member| member.address.to_lowercase())
            .collect();
        contacts_live::rows(&view)
            .into_iter()
            .map(|row| {
                let address = row.address_full.to_string().to_lowercase();
                (row.name.clone(), members.contains(&address))
            })
            .collect()
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
        wallet_live::asset_rows(&view, &self.strings, &self.locale, self.chain_filter)
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
        wallet_live::asset_detail(&view, &feed, index, &self.strings, &self.locale)
            // The holding is gone — a refresh re-ordered the list under an open
            // panel. The mock is NOT a substitute: it would silently swap which
            // asset somebody is looking at, and the next thing they do on this
            // panel is send it.
            .unwrap_or_else(|| fixtures::AssetDetailModel {
                ticker: SharedString::from(""),
                badge: gpui::rgb(0x8A_8F_98).into(),
                amount: SharedString::from(""),
                sub: SharedString::from(""),
                facts: Vec::new(),
                activity: Vec::new(),
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
        wallet_live::balance(&view, &self.strings, &self.locale)
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
            return contacts_fixtures::sections_model();
        }
        let view = resident::resident::<Contacts>(cx).read(cx).view();
        if !view.loaded {
            // The core has not ruled yet. An empty roster, not a fixture one —
            // a person's address book must never show somebody else's while it
            // waits (spec 030 FR-008).
            return Vec::new();
        }
        contacts_live::sections(&view)
    }

    /// DC1: the A–Z sectioned roster.
    fn contacts_list(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Stateful<Div> {
        let selected = match self.panel {
            PanelId::ContactDetail => Some(self.contact),
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

        let mut index = 0usize;
        for (letter, rows) in self.contact_sections(cx) {
            list = list.child(section_letter(theme, letter));
            let last = rows.len() - 1;
            for (i, contact) in rows.iter().enumerate() {
                let at = index;
                list = list.child(
                    contact_row(
                        ElementId::from(("contact", at)),
                        theme,
                        &mut self.identicons,
                        contact,
                        selected == Some(at),
                    )
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.contact = at;
                        this.panel = PanelId::ContactDetail;
                        this.menu = None;
                        cx.notify();
                    }))
                    // The contact menu has been drawn since spec 018 and lived
                    // only on the component board. It is the desktop's entry
                    // to 移入分组 — DC2's own comment says so — so without it
                    // groups could be created, renamed and deleted and never
                    // filled.
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                            this.contact = at;
                            this.menu =
                                Some((ContactsMenu::Contact, event.position, Anchor::TopLeft));
                            cx.notify();
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
            .child(accent_button(
                "group-batch-send",
                theme,
                &mut self.icons,
                None,
                batch_send,
            ))
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
            column = column.child(contact_row(
                ElementId::from(("member", i)),
                theme,
                &mut self.identicons,
                member,
                false,
            ));
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
                    cx.listener(|this, event: &gpui::ClickEvent, _, cx| {
                        this.menu = Some((
                            ContactsMenu::GroupMembers,
                            event.position(),
                            Anchor::TopLeft,
                        ));
                        cx.notify();
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

    /// DC3: the centred empty state with both CTAs.
    fn contacts_empty_view(&mut self, theme: &Theme) -> Div {
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
            ))
    }

    fn contacts_content(&mut self, theme: &Theme, caption: bool, cx: &mut Context<Self>) -> Div {
        let header = self.contacts_header(theme, caption, cx);
        let rail = self.contacts_rail(theme, cx);
        let body: gpui::AnyElement = if self.contacts_empty {
            self.contacts_empty_view(theme).into_any_element()
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
                Box::new(
                    move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                        cx.write_to_clipboard(gpui::ClipboardItem::new_string(address.to_string()));
                    },
                ) as contacts_components::MenuAction
            });
        let recent = self.contacts.recent_activity.clone();
        let view_all = self.contacts.view_all_activity.clone();
        let edit = self.contacts.edit.clone();
        let delete = self.contacts.delete_contact.clone();
        let send = self.contacts.action_send.clone();
        let receive = self.contacts.action_receive.clone();
        let qr = self.contacts.action_qr.clone();

        // DC2 shows membership pills only: the desktop entry point for adding
        // a group is the contact context menu's 移入分组, so the mobile
        // `+ 分组` chip stays off this panel (it lives on the component board).
        let mut chips = div().flex().flex_wrap().gap(px(6.));
        for chip in &model.chips {
            chips = chips.child(group_chip(theme, chip.clone()));
        }

        let hero = div()
            .flex()
            .items_center()
            .gap(px(14.))
            .child(identicon_avatar(
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
                    .gap(px(6.))
                    .child(
                        div()
                            .text_size(theme::text_panel_title())
                            .font_weight(gpui::FontWeight::BOLD)
                            .text_color(theme.fg_base)
                            .whitespace_nowrap()
                            .truncate()
                            .child(model.name.clone()),
                    )
                    .child(chips),
            );

        let actions = div()
            .flex()
            .gap(px(10.))
            .child(action_pill(
                "contact-send",
                theme,
                &mut self.icons,
                Icon::ArrowUpRight,
                send,
            ))
            .child(action_pill(
                "contact-receive",
                theme,
                &mut self.icons,
                Icon::ArrowDownLeft,
                receive,
            ))
            .child(action_pill(
                "contact-qr",
                theme,
                &mut self.icons,
                Icon::QrCode,
                qr,
            ));

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
        activity = activity.child(div().pt(px(10.)).child(text_action(
            "contact-view-all",
            theme,
            &mut self.icons,
            None,
            view_all,
        )));

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
                let editing = live_address.clone().map(|address| ContactForm {
                    editing: true,
                    address: address.to_string(),
                    name: model.name.to_string(),
                });
                text_action(
                    "contact-edit",
                    theme,
                    &mut self.icons,
                    Some(Icon::Pencil),
                    edit,
                )
                .on_click(cx.listener(move |this, _, window, cx| {
                    let Some(form) = editing.clone() else {
                        return;
                    };
                    this.contact_form = Some(form);
                    window.focus(&this.contact_form_name_focus, cx);
                    cx.notify();
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
                        let now_ms = crate::executor::now_ms();
                        resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                            resident.dispatch(
                                ContactEvent::Delete {
                                    address: address.to_string(),
                                    now_ms,
                                },
                                cx,
                            );
                        });
                        // The panel was about a row that no longer exists.
                        this.panel = PanelId::None;
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
        let mut heading = div().flex().items_center().gap(px(6.));
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
            .justify_between()
            .px(px(20.))
            .pt(px(SIDEBAR_TOP))
            .pb(px(8.));
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
                        .w(px(32.))
                        .h(px(32.))
                        .rounded(px(16.))
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
            .child(
                div()
                    .flex_1()
                    .min_h(px(0.))
                    .overflow_hidden()
                    .px(px(20.))
                    .pb(px(20.))
                    .child(body),
            )
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

    /// Open a flow from the wallet home (spec 021 SC-002).
    fn enter_flow(&mut self, entry: FlowEntry, cx: &mut Context<Self>) {
        self.flows = FlowPanel::entry(entry);
        self.panel = PanelId::Flow;
        self.send_host = None;
        self.send_fee_picker = false;
        if entry == FlowEntry::Send && self.identity.is_some() {
            // A fresh journey starts on the one-token list, whatever the last
            // one ended in.
            self.send_sweeping = false;
            self.open_send(SendOpenParams::default(), cx);
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
    fn send_views(
        &self,
        cx: &Context<Self>,
    ) -> Option<(
        vela_core::app::send::SendView,
        vela_core::app::fee_policy::FeeView,
    )> {
        let host = self.send_host.as_ref()?.read(cx);
        Some((host.view.clone(), host.fee_view.clone()))
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
            let panel = flows_live::send_panel(&host.read(cx).view, self.send_fee_picker);
            self.flows = panel.stack();
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
        let host = self.send_host.clone()?;
        let (view, fee) = self.send_views(cx)?;
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
                identity_name: &identity.name,
                identity_address: &identity.address,
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
            recipients: view.recipients.clone(),
            sweeping: self.send_sweeping,
            token_chain_ids: view.tokens.iter().map(|token| token.chain_id).collect(),
            multi_chain_id: view.multi_chain_id,
        })
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
            let card = hardware::qr_card(theme, &self.loc, &payload);
            return Some(scrim("send-qr-scrim").child(card).into_any_element());
        }
        if let Some(waiting) = touch {
            let card = hardware::touch_card(theme, &self.loc, &waiting);
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

    /// The panel's body: the cores' for a real session, the mocks' otherwise.
    ///
    /// Not every panel has a live source yet — Send is 032's, and the scanner
    /// has no data at all. Those fall through to the fixture, which is the
    /// design and is honest about being one. What must NOT happen is a live
    /// panel silently borrowing a mock's numbers, so each live arm is written
    /// out rather than defaulted.
    fn flow_body(&mut self, panel: FlowPanel, cx: &mut Context<Self>) -> flow_fixtures::FlowBody {
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
                ))
            }
            FlowPanel::Da1 => {
                // Privacy comes from the BALANCE view, not the feed's own flag:
                // every money surface masks together.
                let hidden = resident::resident::<BalanceDashboard>(cx)
                    .read(cx)
                    .view()
                    .hidden;
                let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
                flow_fixtures::FlowBody::History(flows_live::history(
                    &feed,
                    &self.flow_strings,
                    &self.strings,
                    hidden,
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
                        flows_live::tx_detail(&feed, id, &self.flow_strings, hidden, &self.locale)
                    })
                    .map_or_else(
                        // The record is gone. The mock is not a substitute for
                        // it — that would show somebody a stranger's
                        // transaction under their own history — so the panel
                        // draws nothing and the chevron leads back.
                        || flow_fixtures::FlowBody::History(Vec::new()),
                        flow_fixtures::FlowBody::TxDetail,
                    )
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
                Some((send, fee)) => {
                    let identity = self.identity();
                    let inputs = flows_live::SendInputs {
                        send: &send,
                        fee: &fee,
                        s: &self.flow_strings,
                        wallet: &self.strings,
                        locale: &self.locale,
                        identity_name: &identity.name,
                        identity_address: &identity.address,
                    };
                    match panel {
                        FlowPanel::Dsd1 => flow_fixtures::FlowBody::SendPick(
                            flows_live::send_pick_with(&inputs, self.send_sweeping),
                        ),
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
                    Some((batch, symbol))
                });
                match live {
                    Some((batch, symbol)) => flow_fixtures::FlowBody::BatchImport(
                        flows_live::batch_import(&batch, &symbol, &self.flow_strings),
                    ),
                    None => flow_fixtures::body(panel, &self.flow_strings),
                }
            }
            FlowPanel::Dr3 | FlowPanel::Ds1 | FlowPanel::Dt3b => {
                flow_fixtures::body(panel, &self.flow_strings)
            }
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
            open_contact_pick: bind(FlowStep::ContactPick, cx),
            open_add_token: bind(FlowStep::AddToken, cx),
            open_scan: bind(FlowStep::Scan, cx),
            add_recipient: bind(FlowStep::AddRecipient, cx),
            open_batch_import: bind(FlowStep::BatchImport, cx),
            advance: bind(FlowStep::SendConfirm, cx).or(bind(FlowStep::SendReceipt, cx)),
            address_field: None,
            add_to_wallet: None,
            open_send_rows: Vec::new(),
            sweep_select_all: None,
            send_pick_cta: None,
            amount_field: None,
            recipient_field: None,
            tap_max: None,
            pick_contact_rows: Vec::new(),
            fee_rows: Vec::new(),
            batch_unit: None,
            batch_paste: None,
            batch_pick_file: None,
            batch_template: None,
            batch_rate_field: None,
            batch_rate_reset: None,
            notice_action: None,
            notice_dismiss: None,
            pick_group_rows: Vec::new(),
            split_amount_fields: Vec::new(),
            remove_recipient_rows: Vec::new(),
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
                    actions.amount_field = Some(panels::AddressField {
                        focus: send.amount_focus,
                        value: send.amount,
                        placeholder: SharedString::from("0"),
                        on_change: Box::new({
                            let host = host.clone();
                            move |amount: String, _: &mut Window, cx: &mut gpui::App| {
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
                    actions.open_contact_pick =
                        Some(to_host(SendEvent::OpenContactPicker { target: None }));
                    actions.add_recipient = Some(to_host(SendEvent::EnterSplitMode));
                    actions.open_batch_import = Some(to_host(SendEvent::OpenBatchImport));
                    actions.open_fee_token = Some(Box::new(cx.listener(
                        |this, _: &gpui::ClickEvent, _, cx| {
                            this.send_fee_picker = true;
                            cx.notify();
                        },
                    )));
                    actions.advance = Some(to_host(SendEvent::Continue));
                }
                FlowPanel::Dsd2e => {
                    actions.open_scan = None;
                    // A pick lands in the field AND closes the sheet. The core
                    // at this branch point leaves the sheet up after
                    // `PickedAddress`; spec 028's core closes it itself, after
                    // which the second event is a no-op — the pair is kept so
                    // either core makes the same screen.
                    // A whole group seeds a split with everybody in it, at
                    // amounts the person still has to type — the same hand-off
                    // web calls 群发转账. The core assigns the row ids.
                    actions.pick_group_rows = send
                        .group_members
                        .into_iter()
                        .map(|members| -> panels::Click {
                            let host = host.clone();
                            Box::new(
                                move |_: &gpui::ClickEvent, _: &mut Window, cx: &mut gpui::App| {
                                    let recipients = members
                                        .iter()
                                        .map(|address| SendRecipientDraft {
                                            id: String::new(),
                                            address: address.clone(),
                                            amount: String::new(),
                                            name: None,
                                        })
                                        .collect();
                                    host.update(cx, |host, cx| {
                                        host.dispatch(
                                            SendEvent::SeedSplitRecipients { recipients },
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

    fn asset_detail_body(&mut self, model: &fixtures::AssetDetailModel, theme: &Theme) -> Div {
        let s = &self.strings;

        let head = div()
            .flex()
            .items_center()
            .gap(px(12.))
            .child(token_icon(theme, model.ticker.as_ref(), model.badge))
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
            .child(action_pill(
                "detail-send",
                theme,
                &mut self.icons,
                Icon::ArrowUpRight,
                s.detail_send.clone(),
            ))
            .child(action_pill(
                "detail-receive",
                theme,
                &mut self.icons,
                Icon::ArrowDownLeft,
                s.detail_receive.clone(),
            ));

        let mut facts = div().flex().flex_col();
        for (i, (label, value)) in model.facts.iter().cloned().enumerate() {
            let mut row = div()
                .flex()
                .items_center()
                .justify_between()
                .py(px(12.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(label),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::MEDIUM)
                        .text_color(theme.fg_base)
                        .child(value),
                );
            if i > 0 {
                row = row.border_t_1().border_color(theme.divider);
            }
            facts = facts.child(row);
        }

        let explorer = div()
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

        let mut tx = div().flex().flex_col().child(
            div()
                .pt(px(8.))
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::BOLD)
                .text_color(theme.fg_base)
                .child(s.label_transactions.clone()),
        );
        for row in &model.activity {
            tx = tx.child(activity_row(theme, &mut self.icons, row));
        }

        div()
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(head)
            .child(buttons)
            .child(facts)
            .child(explorer)
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
            balances = balances.child(balance_display(theme, &mut self.icons, model, None));
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

        let search = search_field(theme, &mut self.icons, s_search);

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
            .w(px(SETTINGS_NAV_W))
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
    fn settings_panel(
        &mut self,
        theme: &Theme,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
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
                let mut titles = div().flex().flex_col().gap(px(6.)).child(
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
            SettingsPage::Appearance => self.settings_appearance(theme),
            SettingsPage::Localization => self.settings_localization(theme, cx),
            SettingsPage::Networks => self.settings_networks(theme, window, cx),
            SettingsPage::RpcProviders => self.settings_providers(theme, window, cx),
            SettingsPage::Endpoints => self.settings_endpoints(theme, window, cx),
            SettingsPage::Storage => self.settings_storage(theme),
            SettingsPage::About => self.settings_about(theme),
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

        div()
            .id("settings-panel")
            .flex_1()
            .min_w(px(0.))
            .h_full()
            .overflow_y_scroll()
            // Left-aligned against the nav column, exactly as the wallet's own
            // content column is. The padding is the panel's, the cap is the
            // content's: a settings form stretched to a 2000px window is a
            // different screen from the one that was designed.
            .px(px(SETTINGS_PANEL_PAD_X))
            .pt(px(WALLET_PAD_TOP))
            .pb(px(48.))
            .child(
                div()
                    .max_w(px(SETTINGS_PANEL_W))
                    .child(head)
                    .when_some(banner, |el, banner| {
                        el.child(div().pb(px(24.)).child(banner))
                    })
                    .child(body),
            )
    }

    /// DST1 — the accounts, the way out, and the one irreversible button.
    /// DST1, live: every account this person has, with the active one marked.
    ///
    /// Clicking another row switches to it. `SwitchAccount` has existed since
    /// 019 with nothing to trigger it — "an event with no control is dead
    /// code", as `session.rs` put it about this very event — and the control
    /// was drawn all along.
    fn settings_account_live(
        &mut self,
        theme: &Theme,
        session: &vela_core::app::session::SessionView,
        cx: &mut Context<Self>,
    ) -> Div {
        let accounts_count = self.settings.accounts_count.clone();
        let accounts_total = self.settings.accounts_total.clone();
        self.sync_switcher(session, cx);
        let s = &self.settings;
        // The COUNT is real; the total beside it is not stated at all, because
        // the switcher's cached per-account totals are a `balance_dashboard`
        // read this panel does not do. A figure that covers one account and is
        // labelled "total" would be worse than no figure.
        // Opening this panel IS the switcher opening: the core refreshes every
        // listed account's total while it is up and answers in
        // `switcher.balances`. Nobody ever told it, so this panel could only
        // say how MANY accounts there were — and its sentence ended on a
        // dangling "·" waiting for the half this adds.
        let summary_count = crate::wallet::fill(
            &accounts_count,
            "count",
            &session.accounts.len().to_string(),
        );
        let sign_out = s.sign_out_button.clone();
        let sign_out_desc = s.sign_out_desc.clone();
        let erase_title = s.erase_title.clone();
        let erase_subtitle = s.erase_subtitle.clone();
        let erase_confirm = s.erase_confirm.clone();

        let switcher = resident::resident::<BalanceDashboard>(cx)
            .read(cx)
            .view()
            .switcher;
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
                &vela_core::l10n::currency::format_fiat(
                    known_total,
                    "USD",
                    "$",
                    &self.locale,
                    vela_core::l10n::currency::FiatOptions::default(),
                ),
            )
        ));
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
                    // USD, like every other total this shell prints. The
                    // display-currency machine owns conversion and its rate can
                    // be `None` — which is NOT 1 — so a converted figure here
                    // would be the one place in the app that guessed.
                    gpui::SharedString::from(vela_core::l10n::currency::format_fiat(
                        entry.usd,
                        "USD",
                        "$",
                        &self.locale,
                        vela_core::l10n::currency::FiatOptions::default(),
                    ))
                });
            // The core's own index, not the loop's: it survives a display
            // reorder, which is exactly what invariant ⑦ is about.
            let index = row.index;
            let address = row.account.address.clone();
            let display = crate::wallet::live::shorten_address(&address);
            let mut card = div()
                .id(ElementId::from(("settings-account", index)))
                .flex()
                .items_center()
                .gap(px(12.))
                .py(px(12.))
                .child(identicon_avatar(&mut self.identicons, &address, 40.))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(gpui::SharedString::from(row.account.name.clone())),
                        )
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_subtle)
                                .child(gpui::SharedString::from(display)),
                        ),
                );
            if let Some(total) = total {
                card = card.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(total),
                );
            }
            if active {
                card = card.child(icon_img(
                    &mut self.icons,
                    Icon::Check,
                    false,
                    theme.accent,
                    18.,
                ));
            } else {
                // Only the OTHER rows are a switch. Clicking the one you are
                // already on should do nothing, not re-run a switch.
                card = card
                    .cursor_pointer()
                    .hover(|el| el.bg(theme.bg_sunken))
                    .on_click(cx.listener(move |_, _, _, cx| {
                        session::switch_account(index, cx);
                        cx.notify();
                    }));
            }
            list = list.child(card).child(div().h(px(1.)).bg(theme.divider));
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
            // The create / sign-in buttons need a route back INTO onboarding
            // from a signed-in window, which is a navigation decision this cut
            // does not make. Drawing them dead would be worse than not drawing
            // them: a button that highlights and does nothing is a promise
            // broken every time it is pressed.
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

    fn settings_account(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let summary = settings_fixtures::accounts_summary(s);
        let create = s.account_create.clone();
        let sign_in = s.account_sign_in.clone();
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
                .child(identicon_avatar(&mut self.identicons, &seed, 40.))
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

        let hover_accent = theme.accent_hover;
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
            .child(
                div()
                    .flex()
                    .gap(px(12.))
                    .pt(px(24.))
                    .child(
                        div()
                            .id("settings-create-account")
                            .h(px(CONTACTS_BUTTON_H))
                            .px(px(32.))
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
                            .child(create),
                    )
                    .child(
                        div()
                            .id("settings-sign-in-account")
                            .h(px(CONTACTS_BUTTON_H))
                            .px(px(32.))
                            .rounded(px(12.))
                            .flex()
                            .items_center()
                            .justify_center()
                            .cursor_pointer()
                            .border_1()
                            .border_color(theme.outline_strong)
                            .text_size(theme::text_row_title())
                            .text_color(theme.fg_base)
                            .child(sign_in),
                    ),
            )
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
            .child(danger_card(
                theme,
                erase_title,
                erase_subtitle,
                erase_confirm,
            ))
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
        div()
            .flex()
            .flex_col()
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
            .child(danger_card(
                theme,
                erase_title,
                erase_subtitle,
                erase_confirm,
            ))
    }

    /// DST2 — language, text size, theme, avatar style.
    fn settings_appearance(&mut self, theme: &Theme) -> Div {
        let s = &self.settings;
        let language = s.language.clone();
        let language_value = gpui::SharedString::from(format!("简体中文 · {}", s.note_system));
        let scale_label = s.text_scale.clone();
        let theme_label = s.theme_title.clone();
        let avatar_label = s.avatar_title.clone();
        let themes = [
            (Some(Icon::Sun), s.theme_light.clone()),
            (Some(Icon::Moon), s.theme_dark.clone()),
            (Some(Icon::Monitor), s.theme_auto.clone()),
        ];
        let avatars = [
            (None, s.avatar_initials.clone()),
            (None, s.avatar_identicon.clone()),
        ];
        // Which theme cell reads as chosen follows the appearance the window is
        // actually in — a settings screen that says "Light" while drawing dark
        // is the one thing this row must never do.
        let theme_index = match self.theme_mode() {
            ThemeMode::Light => 0,
            ThemeMode::Dark => 1,
        };

        let language_control = dropdown_trigger(theme, &mut self.icons, language_value);
        let scale_control = text_scale(theme, 7, 3);
        let theme_control = segmented(theme, &mut self.icons, &themes, theme_index);
        let avatar_control = segmented(theme, &mut self.icons, &avatars, 1);

        div()
            .flex()
            .flex_col()
            .child(form_row(theme, language, language_control))
            .child(form_row(theme, scale_label, scale_control))
            .child(form_row(theme, theme_label, theme_control))
            .child(form_row(theme, avatar_label, avatar_control))
    }

    /// What the 货币 row shows — the core's committed currency for a real
    /// session, the mock's literal for the design surfaces.
    ///
    /// Gated on `identity` for the same reason `sign_out_row` is: `VELA_PAGE=
    /// settings` and the gallery are design surfaces with no session behind
    /// them, and a fixture screen quietly reading live state is how a gallery
    /// stops being reviewable.
    fn currency_value(&self, cx: &mut Context<Self>) -> gpui::SharedString {
        if self.identity.is_none() {
            return gpui::SharedString::from("USD · $1,234.56");
        }
        let view = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        settings_live::currency_row_value(&view, &self.locale)
    }

    /// DST3 — currency, number, date and time formats.
    ///
    /// One of the four can be OPEN, and the mock's is 数字格式. The menu is an
    /// absolutely-positioned child of that row's control cell so it lies over
    /// the rows beneath instead of pushing them down — the desktop SPEC's
    /// "浮层需逃出容器裁剪" rule.
    fn settings_localization(&mut self, theme: &Theme, cx: &mut Context<Self>) -> Div {
        let s = &self.settings;
        let auto_note =
            gpui::SharedString::from(format!("{} · {}", s.note_automatic, s.note_system));
        let rows: [(&'static str, gpui::SharedString, gpui::SharedString); 4] = [
            ("currency", s.currency.clone(), self.currency_value(cx)),
            (
                "number",
                s.number_format.clone(),
                gpui::SharedString::from("1,234,567.89"),
            ),
            (
                "date",
                s.date_format.clone(),
                gpui::SharedString::from("2026/06/13"),
            ),
            (
                "time",
                s.time_format.clone(),
                gpui::SharedString::from("13:45"),
            ),
        ];
        let number_menu: [(gpui::SharedString, Option<gpui::SharedString>, bool); 5] = [
            ("1,234,567.89".into(), Some(auto_note), true),
            ("1,234,567.89".into(), None, false),
            ("1.234.567,89".into(), None, false),
            ("1 234 567,89".into(), None, false),
            ("12,34,567.89".into(), Some(s.note_indian.clone()), false),
        ];

        let open = self.settings_open_dropdown;
        let mut col = div().flex().flex_col();
        for (id, label, value) in rows {
            let is_open = open == Some(id);
            let trigger = dropdown_trigger(theme, &mut self.icons, value);
            let menu = (is_open && id == "number")
                .then(|| dropdown_menu(theme, &mut self.icons, &number_menu));
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

        let edit = move |field: NetOverrideField| {
            move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                let saved = resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                    resident.dispatch(
                        NetEvent::OverrideFieldEdited {
                            chain_id,
                            field,
                            value: text.clone(),
                        },
                        cx,
                    );
                    resident.dispatch(NetEvent::OverrideBlurred { chain_id }, cx);
                    // Refused endpoints do not count as a fix. `rpc_chain_mismatch`
                    // is the core's own verdict on the one refusal that matters —
                    // an endpoint answering for another chain.
                    resident
                        .view()
                        .networks
                        .iter()
                        .find(|row| row.chain_id == chain_id)
                        .is_none_or(|row| row.rpc_chain_mismatch.is_none())
                });
                if saved && field == NetOverrideField::Rpc {
                    // The hero has this chain marked failed and its own retry is
                    // throttled like any other fetch. The person just repaired
                    // the endpoint by hand, which is the moment the web clears
                    // the failure and forces one read.
                    crate::executor::balance_dashboard::dispatch(
                        vela_core::app::balance_dashboard::Event::FixChainResolved { chain_id },
                        cx,
                    );
                    crate::executor::balance_dashboard::refresh(cx);
                }
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
                edit(NetOverrideField::Rpc),
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
                edit(NetOverrideField::Explorer),
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
                            move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                                let entity = resident::resident::<NetworkAdmin>(cx);
                                entity.update(cx, |resident, cx| {
                                    // Edited, then blurred — the blur is what
                                    // persists, and it also DROPS a provider
                                    // whose key was cleared (invariant ⑦).
                                    resident.dispatch(
                                        NetEvent::ProviderKeyEdited {
                                            provider: id,
                                            value: text.clone(),
                                        },
                                        cx,
                                    );
                                    resident.dispatch(
                                        NetEvent::ProviderKeyBlurred { provider: id },
                                        cx,
                                    );
                                });
                            },
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
                    move |text: String, _window: &mut Window, cx: &mut gpui::App| {
                        let entity = resident::resident::<NetworkAdmin>(cx);
                        entity.update(cx, |resident, cx| {
                            // Edited, then blurred. The desktop has no blur
                            // event of its own yet, and the core's blur is what
                            // PERSISTS — so a keystroke that never blurred
                            // would be a setting the next launch has never
                            // heard of. Re-probing per keystroke is the cost;
                            // the core debounces nothing here and neither does
                            // the RN screen.
                            resident.dispatch(
                                NetEvent::EndpointEdited {
                                    field,
                                    value: text.clone(),
                                },
                                cx,
                            );
                            resident.dispatch(NetEvent::EndpointBlurred { field }, cx);
                        });
                    },
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
                        .on_click(cx.listener(|_, _, _, cx| {
                            resident::resident::<NetworkAdmin>(cx).update(cx, |resident, cx| {
                                resident.dispatch(NetEvent::ResetEndpointsToDefaults, cx);
                            });
                            cx.notify();
                        }))
                        .into_any_element()
                } else {
                    reset.into_any_element()
                })
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.info_base)
                        .child(self.settings.endpoints_guide.clone()),
                ),
        )
    }

    /// DST7 — how much of this device Vela is using, and what can be given back.
    fn settings_storage(&mut self, theme: &Theme) -> Div {
        let s = &self.settings;
        // Live since 031. The panel told everybody 2.4 MB / 216 records, and a
        // person deciding whether to clear a cache deserves their own number.
        let (amount, unit, records) = if self.identity.is_some() {
            let (bytes, records) = crate::executor::storage::usage();
            let (amount, unit) = human_bytes(bytes);
            (amount, unit, records)
        } else {
            (
                gpui::SharedString::from(settings_fixtures::STORAGE_AMOUNT),
                gpui::SharedString::from(settings_fixtures::STORAGE_UNIT),
                settings_fixtures::STORAGE_RECORDS,
            )
        };
        let summary = gpui::SharedString::from(crate::wallet::fill(
            &s.storage_summary,
            "count",
            &records.to_string(),
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
            .child(storage_bar(theme, &settings_fixtures::STORAGE_SEGMENTS));
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
        col
    }

    /// DST8 — the build, the technical inventory, the three links.
    fn settings_about(&mut self, theme: &Theme) -> Div {
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
        for (label, value, mono) in settings_fixtures::about_rows(&self.settings) {
            col = col.child(key_value_row(
                theme,
                &mut self.icons,
                label,
                value,
                mono,
                false,
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
        for (label, value) in settings_fixtures::about_links(&self.settings) {
            col = col.child(key_value_row(
                theme,
                &mut self.icons,
                label,
                value,
                true,
                true,
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
        };

        let mut header = div()
            .flex()
            .items_start()
            .justify_between()
            .gap(px(12.))
            .child({
                let mut titles = div().flex().flex_col().gap(px(6.)).child(
                    div()
                        .text_size(theme::text_panel_title())
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(title),
                );
                if let Some(subtitle) = subtitle {
                    titles = titles.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_subtle)
                            .child(subtitle),
                    );
                }
                titles
            });
        header = header.child(
            div()
                .id("settings-dialog-close")
                .size(px(32.))
                .flex_none()
                .rounded_full()
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .justify_center()
                .cursor_pointer()
                .on_click(cx.listener(|this, _, _, cx| {
                    this.close_settings_dialog(cx);
                    cx.notify();
                }))
                .child(icon_img(
                    &mut self.icons,
                    Icon::X,
                    false,
                    theme.fg_muted,
                    18.,
                )),
        );

        let card = div()
            .w(px(SETTINGS_DIALOG_W))
            .flex()
            .flex_col()
            .gap(px(20.))
            .p(px(28.))
            .rounded(px(16.))
            .bg(theme.bg_raised)
            .border_1()
            .border_color(theme.border_card)
            .child(header)
            .child(body);

        Some(
            div()
                .id("settings-dialog-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.bg_base.opacity(0.55))
                .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(card)
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
                    .child(chain_mark(
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
                    col = col.child(check_list(theme, &mut self.icons, checks_title, &checks));
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
        for provider in settings_fixtures::RPC_PROVIDER_LINKS {
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
                    .child(chain_mark(letter, colour, 32.))
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
        for name in settings_fixtures::RPC_PROVIDER_LINKS {
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
            Some(url) => {
                self.browsing = true;
                self.browser_home = url.clone();
                #[cfg(not(target_os = "linux"))]
                crate::webview::navigate(&url);
            }
            // Nothing left, or a start-page tab: the wallet's own screen.
            None => self.browsing = false,
        }
        cx.notify();
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
        // The two trailing affordances open different things, so the page — not
        // the component — carries their listeners.
        let star =
            explore_components::toolbar_control(theme, &mut self.icons, Icon::Star, theme.fg_base);
        let dots = explore_components::toolbar_control(
            theme,
            &mut self.icons,
            Icon::Ellipsis,
            theme.fg_base,
        );
        let chip = explore_components::account_chip(
            theme,
            &mut self.identicons,
            identity.name.clone(),
            &identity.address,
            browsing,
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
                    .on_click(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
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
        // A REAL page, in a real session. The mock keeps drawing for the
        // gallery and for a window nobody has signed in to, which is the same
        // fork every other live surface takes — and it is what keeps
        // `sweep-gallery.sh` from opening a webview per state.
        #[cfg(not(target_os = "linux"))]
        let live_browser = browsing && self.identity.is_some();
        #[cfg(target_os = "linux")]
        let live_browser = false;

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

        let toolbar = explore_components::toolbar(
            theme,
            &mut self.icons,
            browsing,
            host,
            self.explore.search_placeholder.clone(),
            trailing,
            nav,
        );

        let body: gpui::AnyElement = if live_browser {
            #[cfg(not(target_os = "linux"))]
            {
                let home = self.browser_home.clone();
                self.arm_dapp_requests(cx);
                gpui::canvas(
                    |_, _, _| (),
                    move |bounds, (), window, _| {
                        // Placed from the PAINT pass of the element that owns
                        // this rectangle, so the webview follows the column
                        // through a resize and through the signing panel
                        // opening beside it.
                        crate::webview::place(bounds, window, &home);
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

    /// Point the browser's requests at this page.
    ///
    /// Installed once, and re-installing is harmless — the sink replaces
    /// itself, which is what a page rebuilt after a route change needs.
    ///
    /// The hop is DEFERRED on purpose. wry calls its ipc handler from a
    /// platform callback, and `AsyncApp::update` borrows the app cell; doing
    /// that synchronously inside another borrow is a panic in a wallet. So the
    /// sink spawns onto the foreground executor and the work lands on the next
    /// run-loop turn instead.
    #[cfg(not(target_os = "linux"))]
    fn arm_dapp_requests(&mut self, cx: &mut Context<Self>) {
        if self.dapp_requests_armed {
            return;
        }
        self.dapp_requests_armed = true;
        let page = cx.entity().downgrade();
        let async_cx = cx.to_async();
        crate::webview::on_request_to(Box::new(move |incoming| {
            let page = page.clone();
            async_cx
                .spawn(async move |cx| {
                    page.update(cx, |page, cx| page.browser_request(incoming, cx))
                        .ok();
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
        let page = cx.entity().downgrade();
        let async_cx = cx.to_async();
        crate::webview::on_navigation_to(Box::new(move |url| {
            let page = page.clone();
            async_cx
                .spawn(async move |cx| {
                    page.update(cx, |page, cx| page.browser_navigated(url, cx))
                        .ok();
                })
                .detach();
        }));
    }

    /// A page asked for something. EVERY request goes to the core.
    ///
    /// Until spec 032 phase 27 this file kept a list of "signing methods" and
    /// refused the rest with 4900, so no real dApp could connect: a site asks
    /// `eth_chainId` and `eth_requestAccounts` long before it asks for a
    /// signature. Which requests are reads, which need consent, which may be
    /// forwarded and which must be refused is `dapp_permissions`' judgment —
    /// with the security rules inside it (a cross-origin frame, an insecure
    /// origin, a grant pinned to its own address), none of which a method
    /// list in a shell can express.
    #[cfg(not(target_os = "linux"))]
    fn browser_request(&mut self, incoming: crate::webview::Incoming, cx: &mut Context<Self>) {
        let host = match self.browser_host.clone() {
            Some(host) => host,
            None => {
                let Some(host) = self.open_browser_host(cx) else {
                    // No account: nothing to connect anything to. Refused
                    // rather than dropped — a promise that never settles is
                    // the worst outcome this transport has.
                    crate::webview::refuse_unsupported(&incoming.id, &incoming.method);
                    return;
                };
                host
            }
        };
        host.update(cx, |host, cx| {
            host.dispatch(
                vela_core::app::dapp_permissions::Event::ProviderRequest {
                    id: incoming.id,
                    method: incoming.method,
                    params_json: incoming.params_json,
                    origin: incoming.origin,
                    // TRUE by construction, not by assumption: wry's
                    // `with_initialization_script` is main-frame-only
                    // (wry-0.56.1/src/lib.rs:1011 — it calls
                    // `with_initialization_script_for_main_only(js, true)`),
                    // so no sub-frame has a provider to ask with, and every
                    // envelope that reaches the ipc handler came from the
                    // document whose URL the origin was read from.
                    is_main_frame: true,
                },
                cx,
            );
        });
    }

    /// A document load started. The core settles what the last one left open.
    #[cfg(not(target_os = "linux"))]
    fn browser_navigated(&mut self, url: String, cx: &mut Context<Self>) {
        let Some(host) = self.browser_host.clone() else {
            return;
        };
        host.update(cx, |host, cx| {
            host.dispatch(
                vela_core::app::dapp_permissions::Event::NavigationStarted { url },
                cx,
            );
        });
    }

    /// The permissions machine, and the wiring that watches what it decides.
    #[cfg(not(target_os = "linux"))]
    fn open_browser_host(
        &mut self,
        cx: &mut Context<Self>,
    ) -> Option<gpui::Entity<crate::wallet::browser_host::BrowserHost>> {
        let account = money::active_account()?;
        // ALL of them: a grant is pinned to the address it was made for, so
        // the machine judges against every address this wallet has, not the
        // one that happens to be active.
        let addresses = money::account_addresses();
        let chain_id = self.browser_chain;
        let host = cx.new(|cx| {
            crate::wallet::browser_host::BrowserHost::new(
                addresses,
                account.address.clone(),
                chain_id,
                cx,
            )
        });
        cx.observe(&host, Self::browser_host_changed).detach();
        // The document is ALREADY open — this machine is born on the first
        // request a page makes, which is after its load. Without being told,
        // the core has no `current_origin` and never resolves the connected
        // chip, so a site that is granted shows as an unnamed "?" with no
        // connection, and `Disconnect` has nothing to revoke. The URL, not
        // the origin: `NavigationStarted` derives its own, and it is that
        // derivation the grants are keyed by.
        if let Some(url) = crate::webview::current_url() {
            host.update(cx, |host, cx| {
                host.dispatch(
                    vela_core::app::dapp_permissions::Event::NavigationStarted { url },
                    cx,
                );
            });
        }
        self.browser_host = Some(host.clone());
        Some(host)
    }

    /// What the permissions machine decided, acted on once.
    #[cfg(not(target_os = "linux"))]
    fn browser_host_changed(
        &mut self,
        host: gpui::Entity<crate::wallet::browser_host::BrowserHost>,
        cx: &mut Context<Self>,
    ) {
        let (forwarded, settled, consent) = host.update(cx, |host, _| {
            (
                host.take_forwarded(),
                host.take_settled(),
                host.view.consent.is_some(),
            )
        });
        if settled {
            // The request those machines were decoding is over — the core
            // said so, and a decoded intent must never outlive its request.
            self.signing_host = None;
        }
        if consent {
            // The question has to be in front of somebody to be answered.
            self.panel = PanelId::Connection;
        }
        for request in forwarded {
            self.route_forwarded(request, cx);
        }
        cx.notify();
    }

    /// A request the core forwarded, to whoever answers it.
    ///
    /// The core routes on PERMISSION and hands over everything it does not
    /// answer itself; that remainder is three different things — a signature,
    /// a fact about this wallet, and a read of the chain — and telling them
    /// apart is the shell's table in every client (`executor::dapp_rpc`, the
    /// extension's own). Refusing what is not on it is the load-bearing half:
    /// a catch-all read bucket would proxy `eth_signTransaction` to a public
    /// node and make the wallet an open relay for any site it renders.
    #[cfg(not(target_os = "linux"))]
    fn route_forwarded(
        &mut self,
        request: crate::wallet::browser_host::Forwarded,
        cx: &mut Context<Self>,
    ) {
        use crate::executor::dapp_rpc::{self, Route};

        let chain_id = self.browser_chain;
        match dapp_rpc::classify(&request.method) {
            Route::Sign => self.open_signing(request, cx),
            Route::State => {
                // The chain the wallet is on, in the two notations the two
                // methods are specified in. `net_version` is decimal — a hex
                // answer there is a string comparison every dApp fails.
                let answer = if request.method == "net_version" {
                    serde_json::Value::String(chain_id.to_string())
                } else {
                    serde_json::Value::String(dapp_rpc::hex_chain_id(chain_id))
                };
                crate::webview::respond_json(&request.id, &answer);
                self.answered(&request.id, cx);
            }
            Route::Switch => self.switch_browser_chain(&request, cx),
            Route::Ack => {
                // Acknowledged, and nothing changed. A network or a token is
                // added in this wallet's own settings, by a person looking at
                // it — never because a page asked while it had the floor.
                crate::webview::respond_json(&request.id, &serde_json::Value::Null);
                self.answered(&request.id, cx);
            }
            Route::Read { bundler } => self.proxy_read(&request, chain_id, bundler, cx),
            Route::Unsupported => {
                crate::webview::refuse_unsupported(&request.id, &request.method);
                self.answered(&request.id, cx);
            }
        }
    }

    /// EIP-3326. The wallet moves, and the page is told by the core.
    #[cfg(not(target_os = "linux"))]
    fn switch_browser_chain(
        &mut self,
        request: &crate::wallet::browser_host::Forwarded,
        cx: &mut Context<Self>,
    ) {
        let Some(chain_id) = crate::executor::dapp_rpc::switch_chain_param(&request.params_json)
        else {
            // -32602: the request named no chain. Not 4902, which would say
            // "that chain is not added" about a chain nobody named.
            crate::webview::respond_error(&request.id, -32602, "No chainId in the request");
            self.answered(&request.id, cx);
            return;
        };
        if !crate::wallet::signing_host::known_chain_ids().contains(&chain_id) {
            // 4902 is the code a dApp watches for to offer "add this network".
            crate::webview::respond_error(&request.id, 4902, "This chain is not added");
            self.answered(&request.id, cx);
            return;
        }
        self.browser_chain = chain_id;
        // `null` is the success answer EIP-3326 specifies, and the
        // `chainChanged` event is the CORE's to emit — it owns what a
        // connected page is told, and a shell that emitted its own could
        // announce a chain to a site that is not connected.
        crate::webview::respond_json(&request.id, &serde_json::Value::Null);
        self.answered(&request.id, cx);
        if let Some(host) = self.browser_host.clone() {
            host.update(cx, |host, cx| {
                host.dispatch(
                    vela_core::app::dapp_permissions::Event::ChainChanged { chain_id },
                    cx,
                );
            });
        }
        cx.notify();
    }

    /// One read, through the pool that every other read in this app uses.
    ///
    /// On a worker: the pool blocks, and a frame that waited on a dApp's
    /// `eth_getLogs` would be a wallet that freezes because a page asked it a
    /// question.
    #[cfg(not(target_os = "linux"))]
    fn proxy_read(
        &mut self,
        request: &crate::wallet::browser_host::Forwarded,
        chain_id: u32,
        bundler: bool,
        cx: &mut Context<Self>,
    ) {
        let id = request.id.clone();
        let method = request.method.clone();
        let params: serde_json::Value =
            serde_json::from_str(&request.params_json).unwrap_or_else(|_| serde_json::json!([]));
        let host = self.browser_host.clone();
        cx.spawn(async move |_, cx| {
            let answered = id.clone();
            let call_id = id.clone();
            let body = cx
                .background_executor()
                .spawn(async move {
                    if bundler {
                        crate::executor::pool::bundler_call(chain_id, &method, params)
                    } else {
                        crate::executor::pool::call(chain_id, &method, params)
                    }
                })
                .await;
            match body {
                // The node's own envelope, unpacked: its `result` is the
                // answer, and its `error` is an error the page must see as
                // one rather than as a null result.
                Ok(body) => match body.get("error") {
                    Some(error) => crate::webview::respond_error(
                        &call_id,
                        error
                            .get("code")
                            .and_then(serde_json::Value::as_i64)
                            .and_then(|code| i32::try_from(code).ok())
                            .unwrap_or(-32603),
                        error
                            .get("message")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or("The node refused this call"),
                    ),
                    None => crate::webview::respond_json(
                        &call_id,
                        body.get("result").unwrap_or(&serde_json::Value::Null),
                    ),
                },
                // Every endpoint failed. -32603 rather than a silent drop: a
                // promise that never settles is the worst answer this
                // transport can give.
                Err(_) => {
                    crate::webview::respond_error(&call_id, -32603, "No node could be reached")
                }
            }
            if let Some(host) = host {
                host.update(cx, |host, _| host.answered(&answered));
            }
        })
        .detach();
    }

    /// That id is settled; the permissions machine no longer owes it an
    /// answer if the document goes away.
    #[cfg(not(target_os = "linux"))]
    fn answered(&mut self, id: &str, cx: &mut Context<Self>) {
        if let Some(host) = self.browser_host.clone() {
            host.update(cx, |host, _| host.answered(id));
        }
    }

    /// A request the core forwarded. The four signing machines are born here.
    #[cfg(not(target_os = "linux"))]
    fn open_signing(
        &mut self,
        incoming: crate::wallet::browser_host::Forwarded,
        cx: &mut Context<Self>,
    ) {
        let Some(account) = money::active_account() else {
            // No account, no signature. Refused rather than dropped.
            crate::webview::refuse_unsupported(&incoming.id, &incoming.method);
            return;
        };
        let request = crate::wallet::signing_host::IncomingRequest {
            id: incoming.id,
            method: incoming.method,
            params_json: incoming.params_json,
            origin: incoming.origin,
            // One browser, one transport. The id is what will pick between
            // transports when there is more than one, and it must be stable
            // for the life of this one.
            transport_id: "browser".to_owned(),
            chain_id: self.browser_chain,
        };
        let window_handle = self.window_handle;
        let host = cx.new(|cx| {
            crate::wallet::signing_host::SigningHost::open(&account, request, window_handle, cx)
        });
        cx.observe(&host, |page, host, cx| {
            // WHETHER there is a column is the core's answer, not this
            // file's. Most of what a dApp sends — `eth_chainId`,
            // `eth_accounts` — is answered without a person ever seeing it,
            // and a panel that opened for every request would put a signing
            // sheet in front of somebody for a page merely asking which chain
            // it is on.
            if host.read(cx).closed {
                // The request is over: the column goes, and so do its
                // machines — a decoded intent must never outlive the request
                // it decoded.
                page.signing_host = None;
                if page.panel == PanelId::Signing {
                    page.panel = PanelId::None;
                }
            } else {
                page.panel = PanelId::Signing;
            }
            cx.notify();
        })
        .detach();
        // The same question, once, for a request the core answers before any
        // observation fires.
        if host.read(cx).closed {
            return;
        }
        self.signing_host = Some(host);
        self.panel = PanelId::Signing;
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
            grid = grid.child(explore_components::add_tile(
                ElementId::from("tile-add"),
                theme,
                &mut self.icons,
                self.explore.add.clone(),
            ));
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
        for group in live_recent.into_iter().chain(groups) {
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
                        ElementId::from((group.id, i)),
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
                        // Where the person left off, verbatim — that is what
                        // the core stores the whole URL for. A row that opened
                        // the origin instead would send somebody back to a
                        // front page they had already navigated away from.
                        let url = site.host.to_string();
                        cx.listener(move |this, _, _, cx| {
                            if let Some(entry) = resident::resident::<BrowserHistory>(cx)
                                .read(cx)
                                .view()
                                .entries
                                .iter()
                                .find(|entry| entry.host == url)
                            {
                                this.browser_home = entry.url.clone();
                                #[cfg(not(target_os = "linux"))]
                                crate::webview::navigate(&entry.url);
                            }
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
    #[cfg(not(target_os = "linux"))]
    fn consent_body(
        &mut self,
        theme: &Theme,
        consent: &vela_core::app::dapp_permissions::DpermConsentView,
        cx: &mut Context<Self>,
    ) -> Div {
        use vela_core::app::dapp_permissions::Event as DpermEvent;

        // The same derivation the signing header uses. Two ways of turning an
        // origin into a name is two answers to "who is asking", and the sheet
        // and this panel are asking about the same site.
        let (host, _, letter) = signing_live::dapp_identity(&consent.origin);
        let title = crate::signing::fill(&self.explore.consent_title, &[("host", &host)]);
        let approve = self.browser_host.clone();
        let reject = self.browser_host.clone();
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
                    .on_click(cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                        if let Some(host) = approve.as_ref() {
                            host.update(cx, |host, cx| {
                                host.dispatch(
                                    DpermEvent::ConsentApproved {
                                        now_ms: crate::executor::now_ms(),
                                    },
                                    cx,
                                );
                            });
                        }
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
                .on_click(cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                    if let Some(host) = reject.as_ref() {
                        host.update(cx, |host, cx| {
                            host.dispatch(DpermEvent::ConsentRejected, cx);
                        });
                    }
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
        #[cfg(not(target_os = "linux"))]
        if let Some(consent) = self
            .browser_host
            .as_ref()
            .and_then(|host| host.read(cx).view.consent.clone())
        {
            return self.consent_body(theme, &consent, cx);
        }
        #[cfg(target_os = "linux")]
        let _ = cx;
        // WHICH site, and whether it is connected at all, from the core. The
        // mock's `app.uniswap.org · Connected` under a live browser is the
        // phase-22 defect wearing a different hat: this panel is a statement
        // about a specific site's access to this wallet, and naming the wrong
        // one is the only thing it can get seriously wrong.
        let mock = explore_fixtures::uniswap();
        #[cfg(not(target_os = "linux"))]
        let live = self
            .browser_host
            .as_ref()
            .map(|host| host.read(cx))
            .map(|host| {
                (
                    host.view.current_origin.clone(),
                    host.view.connected_address.clone(),
                )
            });
        #[cfg(target_os = "linux")]
        let live: Option<(Option<String>, Option<String>)> = None;
        let (site_host, site_letter, site_tint, connected) = match live {
            Some((origin, address)) => {
                let (host, _, letter) =
                    signing_live::dapp_identity(origin.as_deref().unwrap_or_default());
                (host, letter, theme.accent, address.is_some())
            }
            None => (mock.host.clone(), mock.letter.clone(), mock.tint, true),
        };
        let identity = self.identity();
        let e = &self.explore;
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
                                    // "Connected" is a claim, so it is only
                                    // made when the core says an address is
                                    // granted to this origin.
                                    .child(if connected {
                                        SharedString::from(format!(
                                            "{} · {}",
                                            e.secure_site, e.connected_tag
                                        ))
                                    } else {
                                        e.secure_site.clone()
                                    }),
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
                            .child(self.explore.switch_account.clone()),
                    ),
            )
            .child(row_divider(theme))
            .child(
                div()
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
                                    // The chain this browser is actually on —
                                    // the one a `wallet_switchEthereumChain`
                                    // moved it to, and the one a signature
                                    // from this site would be asked on. The
                                    // drawn "Ethereum" over a Gnosis fee is
                                    // the contradiction phase 22 found on the
                                    // signing sheet.
                                    .child(SharedString::from(crate::flows::live::chain_name(
                                        self.browser_chain,
                                    ))),
                            ),
                    ),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(self.explore.connection_explainer.clone()),
            )
            .child({
                #[cfg(not(target_os = "linux"))]
                let revoke = self.browser_host.clone();
                let button = outline_button(
                    ElementId::from("disconnect"),
                    theme,
                    &mut self.icons,
                    None,
                    self.explore.disconnect.clone(),
                );
                // `None` means "the origin in front of us" — the core's own
                // distinction, because revoking a NAMED origin is silent and
                // revoking the current one owes the page a disconnect event.
                #[cfg(not(target_os = "linux"))]
                let button = button.on_click(cx.listener(move |_, _: &gpui::ClickEvent, _, cx| {
                    if let Some(host) = revoke.as_ref() {
                        host.update(cx, |host, cx| {
                            host.dispatch(
                                vela_core::app::dapp_permissions::Event::RevokeRequested {
                                    origin: None,
                                },
                                cx,
                            );
                        });
                    }
                }));
                button
            })
            .child(
                div()
                    .text_center()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_subtle)
                    .child(self.explore.auto_request_hint.clone()),
            )
    }

    /// DE4 / DCS1–8's third column — the signing request itself.
    fn signing_body(&mut self, theme: &Theme, window: &Window, cx: &mut Context<Self>) -> Div {
        // The live sheet when a request is open, the mock otherwise — the same
        // fork every other surface takes, and what keeps the 33 drawn
        // scenarios reviewable after real requests arrive.
        let mut model = signing_fixtures::build(self.signing_state, &self.signing);
        // Which of the two things this column is: the request, or the gas
        // account it cannot pay from.
        let mut funding = false;
        #[cfg(not(target_os = "linux"))]
        if let Some(host) = self.signing_host.as_ref() {
            let host = host.read(cx);
            let fee = &host.fee_view;
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
            // …and on which chain, from the request too. The fixture's badge
            // said Ethereum over a Gnosis fee.
            model.network_name =
                gpui::SharedString::from(crate::flows::live::chain_name(host.chain_id));
            model.fee = signing_live::fee_model(&host.clear_view, fee, &self.signing);
            model.confirm_label = signing_live::confirm_label(&host.clear_view, &self.signing);
            model.confirm_enabled =
                signing_live::confirm_enabled(&host.view, &host.guard_view, fee);
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
                    panels::AddressField {
                        focus: self.cap_focus.clone(),
                        value: cap_text.clone(),
                        placeholder: SharedString::from("0"),
                        on_change: Box::new(
                            move |text: String, _: &mut Window, cx: &mut gpui::App| {
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

        column = column
            .child(row_divider(theme))
            // The disclosure, collapsed — the universal fallback renderer's
            // entrance. Its five layers live in the phone shells today; the
            // desktop mocks (DCS1–8) draw only this row.
            .child(
                div()
                    .py(px(10.))
                    .flex()
                    .items_center()
                    .gap(px(8.))
                    .child(icon_img(
                        &mut self.icons,
                        Icon::ChevronRight,
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
            );
        if let Some(fee) = signing_components::fee(theme, &mut self.icons, &model.fee) {
            column = column.child(fee);
        }
        column = column
            .child(signing_components::signer_row(
                theme,
                &mut self.identicons,
                model.signer_label.clone(),
                model.signer_name.clone(),
                &model.signer_seed,
            ))
            .child(signing_components::slide_to_confirm(
                theme,
                &mut self.icons,
                model.confirm_label.clone(),
                model.confirm_enabled,
                confirm_action,
            ));
        column
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
            Section::Contacts => columns.child(self.contacts_content(theme, caption, cx)),
            Section::Explore => columns.child(self.explore_content(theme, cx)),
            Section::Settings => columns
                .child(self.settings_nav(theme, cx))
                .child(self.settings_panel(theme, window, cx)),
        };
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
                let body = self.asset_detail_body(&model, theme);
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
                    let body = self.flow_body(panel, cx);
                    let tx_ids = if self.identity.is_some() && panel == FlowPanel::Da1 {
                        flows_live::history_ids(
                            &resident::resident::<ActivityFeed>(cx).read(cx).view(),
                        )
                    } else {
                        Vec::new()
                    };
                    // A live send names the coin in its title; the mock's is USDT.
                    let title = match (&send, panel) {
                        (Some(_), FlowPanel::Dsd2 | FlowPanel::Dsd2b) => self
                            .send_views(cx)
                            .and_then(|(view, _)| view.selected_token)
                            .map_or_else(
                                || flow_fixtures::panel_title(panel, &self.flow_strings),
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
                    let actions = Self::flow_actions(
                        panel,
                        self.identity.is_some(),
                        tx_ids,
                        &focus,
                        &address,
                        &placeholder,
                        send,
                        cx,
                    );
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
        let card = panels::scan_modal(&model, theme, &mut self.icons, tools, preview);
        Some(
            div()
                .id("scan-scrim")
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.backdrop)
                .on_click(cx.listener(|this, _, _, cx| {
                    this.flows.clear();
                    this.panel = PanelId::None;
                    cx.notify();
                }))
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

    /// Keep the viewfinder fed while the scanner is open.
    ///
    /// Started on the first frame DS1 draws rather than on the click that
    /// opened it, because the scanner can also be reached by a restored flow
    /// stack (`VELA_FLOW=DS1`) where no click happened.
    fn pump_camera(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.scan_camera.is_none() {
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
            self.stop_camera(window);
            self.scan_resolved(text, cx);
            return;
        }
        let (frame, _failed) = session.snapshot();
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
            if let Ok(bytes) = std::fs::read(&path)
                && let Some(text) = crate::executor::qr::decode_first(&bytes)
            {
                self.scan_resolved(text, cx);
            }
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
            let Some(text) = crate::executor::qr::decode_first(&bytes) else {
                // No code in that picture. The modal stays up: a person who
                // picked the wrong file wants to pick another one, not to be
                // returned to the wallet.
                return;
            };
            page.update(cx, |this, cx| this.scan_resolved(text, cx))
                .ok();
        })
        .detach();
    }

    /// What a scanned string does, in the two places a scan can happen.
    fn scan_resolved(&mut self, text: String, cx: &mut Context<Self>) {
        use vela_core::app::send::SendScan;
        let request = crate::flows::eip681::parse(&text);
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
            return;
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
                    // Not a payment and not an address: nothing to open. The
                    // scanner closes rather than starting a send to a string.
                    self.panel = PanelId::None;
                    cx.notify();
                    return;
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
    }

    /// Read an address-book backup and hand it to the core.
    ///
    /// The shell reads and PARSES; the core applies existing-wins and counts
    /// what happened. Which of those two halves is which is the reason
    /// `ImportParsed` takes already-parsed rows rather than a file.
    fn import_contacts(cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(gpui::PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: None,
        });
        cx.spawn(async move |page, cx| {
            let Ok(Ok(Some(paths))) = paths.await else {
                // Cancelled, or the platform declined. Nothing to report: the
                // person closed a dialog.
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
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
                            // 导入到本组 has no file path of its own yet; the
                            // header's import is the whole book.
                            into_group: None,
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
                            resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                                resident
                                    .dispatch(ContactEvent::GroupDelete { id: id.to_string() }, cx);
                            });
                            // The group that was open no longer exists; the
                            // rail falls back to the whole book rather than to
                            // whichever group slid into that index.
                            this.group = None;
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
                        Self::import_contacts(cx);
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
                // Disconnect is the CORE's: `None` means "the origin in front
                // of us", which owes the page a disconnect event, and a named
                // one would be revoked silently.
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.menu = None;
                    #[cfg(not(target_os = "linux"))]
                    if let Some(host) = this.browser_host.clone() {
                        host.update(cx, |host, cx| {
                            host.dispatch(
                                vela_core::app::dapp_permissions::Event::RevokeRequested {
                                    origin: None,
                                },
                                cx,
                            );
                        });
                    }
                    cx.notify();
                })) as contacts_components::MenuAction),
                // Close leaves the browser. The permissions machine hears
                // `BrowserClosed` from the same place it always did — the
                // frame that stops drawing the column.
                Some(Box::new(cx.listener(|this, _: &gpui::ClickEvent, _, cx| {
                    this.menu = None;
                    this.browsing = false;
                    this.panel = PanelId::None;
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
                            cx.write_to_clipboard(gpui::ClipboardItem::new_string(copy_me.clone()));
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                    // 编辑 — the same sheet 新建联系人 opens, with the address
                    // fixed: the address IS the identity, and an edit that
                    // changed it would be a delete and an add wearing one
                    // button.
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, window, cx| {
                            this.menu = None;
                            this.contact_form = Some(ContactForm {
                                address: edit_address.clone(),
                                name: name.clone(),
                                editing: true,
                            });
                            window.focus(&this.contact_form_name_focus, cx);
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                    // 移入分组 — the picker this shell has been pointing at
                    // since spec 018 (DC2's comment) and never opened.
                    Some(
                        Box::new(cx.listener(move |this, event: &gpui::ClickEvent, _, cx| {
                            this.menu = Some((
                                ContactsMenu::ContactGroups,
                                event.position(),
                                Anchor::TopLeft,
                            ));
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                    Some(
                        Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                            this.menu = None;
                            resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                                resident.dispatch(
                                    ContactEvent::Delete {
                                        address: delete_address.clone(),
                                        now_ms: crate::executor::now_ms(),
                                    },
                                    cx,
                                );
                            });
                            cx.notify();
                        })) as contacts_components::MenuAction,
                    ),
                ]
            }

            // One tap per group: the whole membership goes back, with this one
            // flipped. The core normalises the set — a shell that sent "add"
            // and "remove" separately would be inventing two events where the
            // machine offers one.
            ContactsMenu::ContactGroups => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                let Some(row) = contacts_live::rows(&view).into_iter().nth(self.contact) else {
                    return Vec::new();
                };
                let address = row.address_full.to_string();
                let lower = address.to_lowercase();
                let groups: Vec<(String, bool)> = view
                    .groups
                    .iter()
                    .map(|group| {
                        (
                            group.id.clone(),
                            group
                                .members
                                .iter()
                                .any(|member| member.address.to_lowercase() == lower),
                        )
                    })
                    .collect();
                groups
                    .iter()
                    .map(|(id, _member)| {
                        let address = address.clone();
                        let id = id.clone();
                        let groups = groups.clone();
                        Some(
                            Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                this.menu = None;
                                let group_ids = contacts_live::set_after_toggle(&groups, &id);
                                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                                    resident.dispatch(
                                        ContactEvent::SetContactGroups {
                                            address: address.clone(),
                                            group_ids,
                                        },
                                        cx,
                                    );
                                });
                                cx.notify();
                            })) as contacts_components::MenuAction,
                        )
                    })
                    .collect()
            }

            // The same toggle from the group's side. One tap sends the whole
            // membership back — `SetGroupMembers` carries the set, and the
            // core normalises it.
            ContactsMenu::GroupMembers => {
                let view = resident::resident::<Contacts>(cx).read(cx).view();
                let Some(group) = self.group.and_then(|index| view.groups.get(index)) else {
                    return Vec::new();
                };
                let id = group.id.clone();
                let members: Vec<String> = group
                    .members
                    .iter()
                    .map(|member| member.address.to_lowercase())
                    .collect();
                let current: Vec<(String, bool)> = contacts_live::rows(&view)
                    .into_iter()
                    .map(|row| {
                        let address = row.address_full.to_string();
                        let member = members.contains(&address.to_lowercase());
                        (address, member)
                    })
                    .collect();
                current
                    .iter()
                    .map(|(address, _)| {
                        let id = id.clone();
                        let address = address.clone();
                        let current = current.clone();
                        Some(
                            Box::new(cx.listener(move |this, _: &gpui::ClickEvent, _, cx| {
                                this.menu = None;
                                let members = contacts_live::set_after_toggle(&current, &address);
                                resident::resident::<Contacts>(cx).update(cx, |resident, cx| {
                                    resident.dispatch(
                                        ContactEvent::SetGroupMembers {
                                            id: id.clone(),
                                            members,
                                        },
                                        cx,
                                    );
                                });
                                cx.notify();
                            })) as contacts_components::MenuAction,
                        )
                    })
                    .collect()
            }

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
            ContactsMenu::Contact => contacts_fixtures::contact_context(&self.contacts),
            ContactsMenu::ContactGroups => {
                contacts_fixtures::contact_group_pick(&self.contact_group_state(cx))
            }
            ContactsMenu::GroupMembers => {
                contacts_fixtures::group_member_pick(&self.group_member_state(cx))
            }
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
        let theme = Theme::of(self.theme_mode());

        // Windows and Linux CSD have no system caption, so the page draws one
        // (spec 015 results.md deviation 5 assumed Windows was a native path;
        // `appears_transparent` means it is not). Where it lands over content,
        // that content is pushed clear of it below.
        let caption = owns_titlebar(window);

        // The browser is a NATIVE subview: it does not disappear because the
        // route changed, so every frame that is not drawing the browser column
        // takes it off the screen. Miss this and a webview floats over the
        // wallet. `place` turns it back on in the same frame it is drawn.
        #[cfg(not(target_os = "linux"))]
        if !(self.section == Section::Explore && self.browsing && self.identity.is_some()) {
            crate::webview::hide();
            // Leaving the browser is `BrowserClosed` to the core, which
            // settles what the page left pending with its own 4900 — the
            // alternative is a dApp promise that never resolves because
            // somebody clicked away from the column.
            if let Some(host) = self.browser_host.take() {
                host.update(cx, |host, cx| {
                    host.dispatch(vela_core::app::dapp_permissions::Event::BrowserClosed, cx);
                });
            }
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
        let menu = self.menu_overlay(&theme, cx);
        let sign_out = self.sign_out_dialog(&theme, cx);
        let network_remove = self.network_remove_dialog(&theme, cx);
        let settings_dialog = self.settings_dialog_overlay(&theme, window, cx);
        let import_result = self.import_result_dialog(&theme, cx);
        let contact_form = self.contact_form_dialog(&theme, window, cx);
        let group_form = self.group_form_dialog(&theme, window, cx);
        let explore_form = self.explore_form_dialog(&theme, window, cx);
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
        // The cable's dialogs and the core's alert, over the send flow.
        if let Some(prompt) = send_prompt {
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
        // The import's answer, over the menu it was started from.
        if let Some(import_result) = import_result {
            root = root.child(import_result);
        }
        if let Some(contact_form) = contact_form {
            root = root.child(contact_form);
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
        let root = root
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let ks = &event.keystroke;
                // Esc peels one layer at a time: the sign-out dialog first (it
                // is on top), then the anchored menu, then the third column
                // (desktop SPEC keyboard map).
                if ks.key == "escape" && session::view(cx).sign_out.is_some() {
                    session::sign_out_dismissed(cx);
                    cx.notify();
                    return;
                }
                if ks.key == "escape" && this.network_remove.is_some() {
                    this.network_remove = None;
                    cx.notify();
                    return;
                }
                if ks.key == "escape" && this.settings_dialog.is_some() {
                    this.close_settings_dialog(cx);
                    cx.notify();
                    return;
                }
                if ks.key == "escape" {
                    if this.menu.is_some() {
                        this.menu = None;
                        cx.notify();
                    } else if this.panel != PanelId::None {
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

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Bytes read the way a file manager on this machine reads them.
    #[test]
    fn a_file_size_is_stated_in_the_unit_a_person_can_compare() {
        let (amount, unit) = super::human_bytes(512);
        assert_eq!((amount.as_ref(), unit.as_ref()), ("512", "B"));
        // 1024 base, because that is what the OS says beside it.
        let (amount, unit) = super::human_bytes(1536);
        assert_eq!((amount.as_ref(), unit.as_ref()), ("1.5", "KB"));
        let (amount, unit) = super::human_bytes(3 * 1024 * 1024 / 2);
        assert_eq!((amount.as_ref(), unit.as_ref()), ("1.5", "MB"));
        // No decimal below KB: "1.5 KB" of 1536 bytes is a real number, but
        // "0.5 KB" of 512 is noise where "512 B" is exact.
        let (amount, unit) = super::human_bytes(0);
        assert_eq!((amount.as_ref(), unit.as_ref()), ("0", "B"));
    }

    /// The receive screen's chain and the browser's chain are not the same
    /// thing, and for one commit they were one field.
    ///
    /// This is a defence against the shape of that bug rather than a test of a
    /// function: a page holding both defaults to Gnosis for each, and moving
    /// one must not move the other. Tapping "Ethereum" on the receive screen
    /// used to retarget a connected dApp's next signature — its quote, its
    /// RPC, its submit — to Ethereum, and a site's own `wallet_switchEthereumChain`
    /// used to change which chain your receive QR was for.
    #[test]
    fn the_receive_chain_and_the_browser_chain_are_two_fields() {
        let source = include_str!("page.rs");
        // The browser's chain is what a signing request is opened on.
        assert!(
            source.contains("chain_id: self.browser_chain,"),
            "a dApp request must be raised on the chain the BROWSER is on"
        );
        // And the receive screen's rows still write their own.
        assert!(
            source.contains("this.receive_chain = chain_id;"),
            "the receive rows still choose the receive chain"
        );
        // Neither name may appear in the other's job. `switch_browser_chain`
        // is the one place a site can move a chain, and it moves the browser's.
        let switch = source
            .split("fn switch_browser_chain")
            .nth(1)
            .unwrap_or_default();
        let body = switch.split("\n    fn ").next().unwrap_or_default();
        assert!(
            body.contains("self.browser_chain = chain_id;"),
            "a chain switch moves the browser"
        );
        assert!(
            !body.contains("self.receive_chain"),
            "a chain switch must not touch the receive screen"
        );
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

    /// The second-level nav is the phone's settings list with the rows
    /// collapsed to their titles — same ids, same order, so somebody who
    /// learned one knows the other.
    #[test]
    fn settings_nav_covers_every_panel() {
        assert_eq!(SettingsPage::ALL.len(), 8);
        assert_eq!(SettingsPage::ALL[0], SettingsPage::Account);
        assert_eq!(SettingsPage::ALL[7], SettingsPage::About);
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

/// The add/edit contact sheet's draft.
#[derive(Clone, Default)]
struct ContactForm {
    /// `true` when the address is fixed — an edit, not an add.
    editing: bool,
    address: String,
    name: String,
}

/// Bytes as the figure and the unit the hero draws them as.
///
/// KB and MB at 1024, because that is what a file manager on this machine will
/// say and a person comparing the two numbers should not have to know which
/// convention each used. One decimal past KB, none below: "1536 B" is a real
/// number and "1.5 KB" of it is noise.
fn human_bytes(bytes: u64) -> (SharedString, SharedString) {
    #[allow(clippy::cast_precision_loss, reason = "a file size, for display")]
    let value = bytes as f64;
    if bytes < 1024 {
        return (
            SharedString::from(bytes.to_string()),
            SharedString::from("B"),
        );
    }
    if bytes < 1024 * 1024 {
        return (
            SharedString::from(format!("{:.1}", value / 1024.0)),
            SharedString::from("KB"),
        );
    }
    (
        SharedString::from(format!("{:.1}", value / (1024.0 * 1024.0))),
        SharedString::from("MB"),
    )
}
