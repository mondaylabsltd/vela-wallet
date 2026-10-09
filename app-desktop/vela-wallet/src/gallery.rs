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
    crate::dev_env::flag!("VELA_GALLERY")
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
    /// The trusted page's dialogs, here for the same reason as the cable's:
    /// each needs a browser and a page in a particular state, so they are
    /// among the screens a reviewer is least able to reach on purpose. A
    /// `Refusal` is how an attempt ended; `NotOpened` why a page never was.
    TrustedSignerEnded(
        crate::executor::trusted_signer::Refusal,
        Option<crate::executor::trusted_signer::NotOpened>,
    ),
    /// Spec 102 D4: the hand-off card, with its page's integrity line and —
    /// for a transaction — the fee row the sheet settled (label, value).
    Handoff(
        crate::executor::send::Handoff,
        vela_core::trusted_signer::launch::IntegrityLine,
        Option<(&'static str, &'static str)>,
    ),
    /// Spec 102: the sign-in chooser — three places and "Use my own signing
    /// page", or (with a page) that page heading the places.
    SignIn(Option<String>),
    /// Spec 102: "Use my own signing page" — the picker, its pages and their
    /// lines (url, label, domain, official, line).
    OwnPages(
        Vec<(
            String,
            String,
            String,
            bool,
            vela_core::trusted_signer::launch::IntegrityLine,
        )>,
    ),
}

/// A check that admitted `version` — "matches Vela's published build list"
/// for one this build ships, "trusted on this device" for another — a
/// minute before the shot.
fn checked(version: &str) -> vela_core::trusted_signer::launch::IntegrityLine {
    vela_core::trusted_signer::launch::IntegrityLine::of(
        &vela_core::trusted_signer::integrity::Verdict::Open,
        version,
        Some((crate::executor::now_ms() as u64).saturating_sub(60_000)),
    )
}

/// The official page's line, as a check of its launch version reads.
fn official_checked() -> vela_core::trusted_signer::launch::IntegrityLine {
    checked(vela_core::trusted_signer::integrity::LAUNCH)
}

/// A self-hosted page's line: its own build, trusted on this device.
fn own_checked() -> vela_core::trusted_signer::launch::IntegrityLine {
    checked("7e57c0de5e1f0000000000000000000000000000000000000000000000000000")
}

/// A self-hosted page serving a build nobody decided about yet: "Version
/// 3f9a1c22 is new to Vela. Trust it on this device?"
fn asks_to_trust() -> vela_core::trusted_signer::launch::IntegrityLine {
    vela_core::trusted_signer::launch::IntegrityLine::of(
        &vela_core::trusted_signer::integrity::Verdict::AskToTrust {
            actual: "3f9a1c22b7e04d5a9c8e7f6a5b4c3d2e1f0a9b8c7d6e5f4a3b2c1d0e9f8a7b6c".to_owned(),
        },
        "",
        None,
    )
}

/// A check that did not complete.
fn could_not_check() -> vela_core::trusted_signer::launch::IntegrityLine {
    vela_core::trusted_signer::launch::IntegrityLine::of(
        &vela_core::trusted_signer::integrity::Verdict::CouldNotCheck(
            vela_core::trusted_signer::integrity::CheckFailure::Unreachable,
        ),
        "",
        None,
    )
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
        // Spec 102: three places, always; the domain is Vela's until the
        // person picks their own page, which they may before the first key.
        signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
        signing_page: None,
        can_choose_page: true,
        add_methods: KeyMethod::ALL.to_vec(),
    }
}

/// The page the "own page" fixtures are made on.
const OWN_PAGE: &str = "https://sign.example.com/";

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
    let authenticator_attachment = if platform_key {
        "platform".to_owned()
    } else {
        "cross-platform".to_owned()
    };
    let transports = if platform_key {
        "internal,hybrid".to_owned()
    } else {
        "usb".to_owned()
    };
    CreateKeyRow {
        // What the core would decide from this shape, rather than a second
        // opinion the gallery invents: the row the gallery draws has to be one
        // the machine could really emit (issue #207).
        kind: vela_core::passkey::reported_method(&authenticator_attachment, &transports)
            .unwrap_or(method),
        authenticator_attachment,
        transports,
        name: name.to_owned(),
        confirmed,
        synced,
        synced_known: true,
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
    // The first key: the three places, and — the one moment a wallet's
    // signing domain can be chosen — "Use my own signing page" under them.
    flow("keys · first key", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
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
    // Spec 102: "Use my own signing page" chosen before the first key — the
    // page heads the list, with its domain and integrity line, and the three
    // places are minted ON it.
    flow("keys · on my own page", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.signing_domain = "sign.example.com".to_owned();
        view.signing_page = Some(OWN_PAGE.to_owned());
        view
    });
    // …and once a key exists, the page is a fact of the wallet: no way back.
    flow("keys · a page's own set", {
        let mut view = base_view();
        view.stage = CreateStage::AddKeys;
        view.can_go_back = true;
        view.keys = vec![key("Everyday wallet", KeyMethod::Platform, true, false)];
        view.needs_second_key = true;
        view.signing_domain = "sign.example.com".to_owned();
        view.signing_page = Some(OWN_PAGE.to_owned());
        view.can_choose_page = false;
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
            cancellable: false,
        }),
    );
    hardware(
        "touch · fingerprint",
        Fixture::Touch(TouchRequest {
            kind: TouchKind::Fingerprint,
            product: "YubiKey Bio".to_owned(),
            remote: false,
            cancellable: false,
        }),
    );
    hardware(
        "touch · several keys",
        Fixture::Touch(TouchRequest {
            kind: TouchKind::Select,
            product: String::new(),
            remote: false,
            cancellable: false,
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
    // The trusted page's own dialogs, in their own group — they are not the
    // cable's, and a reviewer looking for "the page" should find them
    // together.
    let mut signer = |code: &'static str, fixture: Fixture| {
        out.push(Entry {
            group: "Trusted page",
            code,
            fixture,
        });
    };
    // The key labels the plan carries (D-17): a key named after the wallet
    // is named by its place; one the person named, by its name.
    let by_place = |method: &str| vela_core::signing_venue::KeyLabel::of(None, "", method);
    let official_handoff = crate::executor::send::Handoff {
        page: Some(vela_core::trusted_signer::DEFAULT_SIGNER_URL.to_owned()),
        block: None,
        key_label: by_place("hybrid"),
    };
    // What the sheet settled before the hand-off: the fee in its coin, its
    // fiat, and the speed it is priced at.
    const FEE: Option<(&str, &str)> = Some(("componentsUi.gas.networkFee", "0.0012 USDC · ≈$0.01"));
    signer(
        "hand-off · matches",
        Fixture::Handoff(official_handoff.clone(), official_checked(), FEE),
    );
    signer(
        "hand-off · checking",
        Fixture::Handoff(
            official_handoff.clone(),
            vela_core::trusted_signer::launch::IntegrityLine::checking(),
            FEE,
        ),
    );
    signer(
        "hand-off · could not check",
        Fixture::Handoff(official_handoff.clone(), could_not_check(), FEE),
    );
    signer(
        "hand-off · a message, no fee",
        Fixture::Handoff(official_handoff.clone(), official_checked(), None),
    );
    signer(
        "hand-off · self-hosted, named key",
        Fixture::Handoff(
            crate::executor::send::Handoff {
                page: Some(OWN_PAGE.to_owned()),
                block: None,
                key_label: vela_core::signing_venue::KeyLabel::of(
                    Some("YubiKey 5C"),
                    "Savings",
                    "security_key",
                ),
            },
            own_checked(),
            FEE,
        ),
    );
    signer(
        "hand-off · self-hosted, new version",
        Fixture::Handoff(
            crate::executor::send::Handoff {
                page: Some(OWN_PAGE.to_owned()),
                block: None,
                key_label: by_place("security_key"),
            },
            asks_to_trust(),
            FEE,
        ),
    );
    signer(
        "hand-off · keys out of reach",
        Fixture::Handoff(
            crate::executor::send::Handoff {
                page: None,
                block: Some(vela_core::signing_venue::VenueBlock::AppCannotReach {
                    domain: "sign.example.com".to_owned(),
                }),
                key_label: by_place("platform"),
            },
            vela_core::trusted_signer::launch::IntegrityLine::checking(),
            None,
        ),
    );
    signer("sign in · methods", Fixture::SignIn(None));
    signer(
        "sign in · on my own page",
        Fixture::SignIn(Some(OWN_PAGE.to_owned())),
    );
    signer(
        "own page · picker",
        Fixture::OwnPages(vec![
            (
                vela_core::trusted_signer::DEFAULT_SIGNER_URL.to_owned(),
                String::new(),
                vela_core::signing_venue::APP_DOMAIN.to_owned(),
                true,
                official_checked(),
            ),
            (
                OWN_PAGE.to_owned(),
                "Home".to_owned(),
                "sign.example.com".to_owned(),
                false,
                own_checked(),
            ),
            (
                "https://signer.example.org/".to_owned(),
                String::new(),
                "signer.example.org".to_owned(),
                false,
                asks_to_trust(),
            ),
            (
                "http://localhost:8140/clearsigning/".to_owned(),
                String::new(),
                "localhost".to_owned(),
                false,
                could_not_check(),
            ),
        ]),
    );
    signer(
        "ended · nothing came back in time",
        Fixture::TrustedSignerEnded(crate::executor::trusted_signer::Refusal::TimedOut, None),
    );
    signer(
        "ended · page not opened",
        Fixture::TrustedSignerEnded(
            crate::executor::trusted_signer::Refusal::NotOpened,
            Some(crate::executor::trusted_signer::NotOpened::Integrity(
                could_not_check(),
            )),
        ),
    );
    let mut sheet = |code: &'static str, kind: PromptKind, confirmable: bool| {
        out.push(Entry {
            group: "Failures",
            code,
            fixture: Fixture::Sheet { kind, confirmable },
        });
    };
    sheet(
        "unsupported",
        PromptKind::NotSupportedCreate { security_key: true },
        false,
    );
    sheet(
        "unsupported · login",
        PromptKind::NotSupportedLogin { security_key: true },
        false,
    );
    sheet("not discoverable", PromptKind::NotDiscoverable, false);
    sheet("incompatible", PromptKind::IncompatibleCreate, false);
    sheet("incompatible · login", PromptKind::IncompatibleLogin, false);
    sheet("recover offer", PromptKind::RecoverOffer, true);
    sheet("recover failed", PromptKind::RecoverFailed, false);
    sheet(
        "create failed · unknown",
        PromptKind::create_failed("the security key returned no pinUvAuthToken".to_owned()),
        false,
    );
    sheet(
        "create failed · network",
        PromptKind::create_failed("Register failed: connection refused".to_owned()),
        false,
    );
    sheet(
        "create failed · server",
        PromptKind::create_failed("Register failed: http status: 503".to_owned()),
        false,
    );
    sheet(
        "create failed · timeout",
        PromptKind::create_failed("Register timed out after 120s".to_owned()),
        false,
    );
    sheet(
        "create failed · no key",
        PromptKind::create_failed(
            "No security key is plugged in. Insert one and try again.".to_owned(),
        ),
        false,
    );
    sheet(
        "sign-in failed",
        PromptKind::sign_in_failed("the security key holds no Vela passkey".to_owned()),
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
    /// The app's glyphs, for the signing-page rows and the hand-off card.
    icons: RefCell<crate::icons::IconCache>,
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
        // An index, or a fixture's own code (`hand-off · checking`) — so a
        // screenshot pass names the state it means, not where it sits today.
        let selected = crate::dev_env::var!("VELA_GALLERY_STATE")
            .and_then(|raw| {
                raw.parse::<usize>()
                    .ok()
                    .or_else(|| entries.iter().position(|entry| entry.code == raw.trim()))
            })
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
            icons: RefCell::default(),
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
                // The page's row from a fixture line: a review screen that
                // reached the network would be a review of the network.
                let own_page = view.signing_page.as_deref().map(|url| {
                    crate::signing::pages::page_row(
                        &self.loc,
                        url,
                        "",
                        &view.signing_domain,
                        false,
                        &own_checked(),
                    )
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
                    own_page: own_page.as_ref(),
                    icons: &self.icons,
                    copied: self.copied,
                    sink,
                };
                render_create_flow(&host, window)
            }
            // The cable's three dialogs, rendered bare: the gallery IS the
            // backdrop, and each card's own scrim would cover the sidebar.
            Fixture::Touch(request) => {
                crate::hardware::touch_card(theme, &self.loc, request, |_, _, _| {})
            }
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
            // Spec 075: bare, like the cable's — the gallery IS the backdrop.
            Fixture::TrustedSignerEnded(refusal, not_opened) => {
                crate::signing::trusted_signer::ended_card(
                    theme,
                    &self.loc,
                    self.loc.t("componentsUi.signing.handoffTitle"),
                    crate::signing::trusted_signer::ended_words(
                        &self.loc,
                        *refusal,
                        not_opened.as_ref(),
                    ),
                    |_, _, _| {},
                )
            }
            // On the signing column's own surface and width, as the wallet
            // draws it beside a request.
            Fixture::Handoff(handoff, line, fee) => {
                let mut model =
                    crate::signing::trusted_signer::handoff_model(&self.loc, handoff, Some(line));
                // The fee row in the corpus's words: the label, the figure
                // the sheet's formatter would draw, and the speed's name.
                model.fee =
                    fee.map(
                        |(label, figure)| crate::signing::trusted_signer::HandoffFeeRow {
                            label: self.loc.t(label),
                            figure: SharedString::from(figure),
                            speed: Some(self.loc.t("send.gasTier.standard")),
                        },
                    );
                let on_open: crate::signing::trusted_signer::Click = Box::new(|_, _, _| {});
                let on_recheck: crate::signing::trusted_signer::Click = Box::new(|_, _, _| {});
                let on_trust: crate::signing::trusted_signer::Click = Box::new(|_, _, _| {});
                div()
                    .w(px(theme::THIRD_PANEL_W))
                    .p(px(24.))
                    .rounded(px(RADIUS_CARD))
                    .bg(theme.bg_raised)
                    .border_1()
                    .border_color(theme.border_card)
                    .child(crate::signing::trusted_signer::handoff_card(
                        theme,
                        &mut self.icons.borrow_mut(),
                        &model,
                        Some(on_open),
                        Some(on_recheck),
                        Some(on_trust),
                    ))
            }
            Fixture::SignIn(page) => {
                let row = page.as_deref().map(|url| {
                    crate::signing::pages::page_row(
                        &self.loc,
                        url,
                        "",
                        &vela_core::signing_venue::domain_of_page(url),
                        false,
                        &own_checked(),
                    )
                });
                let own_page = crate::hardware::OwnPage {
                    chosen: row.as_ref(),
                    icons: &self.icons,
                    on_open: Rc::new(|_, _| {}),
                    on_clear: Rc::new(|_, _| {}),
                    on_page: row.is_some(),
                };
                crate::hardware::signin_method_card(
                    theme,
                    &self.loc,
                    &self.passkey_icons,
                    Some(own_page),
                    std::sync::Arc::new(|_, _, _| {}),
                    |_, _, _| {},
                )
            }
            Fixture::OwnPages(pages) => {
                let rows: Vec<crate::signing::pages::PageRow> = pages
                    .iter()
                    .map(|(url, name, domain, official, line)| {
                        crate::signing::pages::page_row(
                            &self.loc, url, name, domain, *official, line,
                        )
                    })
                    .collect();
                let add = crate::settings::components::editable_url_field(
                    "gallery-own-page-add",
                    theme,
                    Some(self.loc.t("settings.signing.pageAdd")),
                    "",
                    SharedString::from("https://sign.example.com"),
                    None,
                    None,
                    None,
                    &self.name_focus,
                    window,
                    |_, _, _| {},
                    |_, _| {},
                );
                crate::signing::pages::own_page_sheet(
                    theme,
                    &mut self.icons.borrow_mut(),
                    &self.loc,
                    &rows,
                    Rc::new(|_, _, _| {}),
                    Rc::new(|_, _, _| {}),
                    add,
                    |_, _, _| {},
                )
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

    /// Issue 460, as the gallery shows it: every failure sheet's badge is
    /// the exclamation — the red ones included — and none is a ×.
    #[test]
    fn no_failure_sheet_wears_a_close_glyph() {
        use crate::outcome::BadgeVariant;
        let loc = crate::loc::Loc::from_env();
        let mut errors = 0;
        for entry in entries() {
            let Fixture::Sheet { kind, .. } = &entry.fixture else {
                continue;
            };
            let badge = OutcomeKind::for_prompt(kind).spec(&loc).badge;
            errors += usize::from(badge == BadgeVariant::Error);
            let glyph = crate::ui::badge_glyph(badge);
            assert!(
                glyph.is_none_or(|glyph| glyph == "!"),
                "`{}` draws {glyph:?}",
                entry.code
            );
        }
        assert!(errors > 0, "the gallery shows at least one red badge");
    }
}
