//! Every capability the person has is an intent (spec 103) — on the desktop,
//! kept true by this ledger, as `coverage.test.ts` keeps it on the web. It
//! names everything the desktop's screens do to the wallet's machines: each
//! core event a screen sends, each direct call into the session, the
//! preferences, the storage and the residents. A new one fails here until it
//! is placed:
//!
//! - `Intent` — it commits that catalogued action (the agent can do it too);
//! - `Step`   — one step of that action's own sequence;
//! - `View`   — what is shown or how, not what the wallet holds;
//! - `Machine`— a machine's input (a tick, a fee, a boot), not the person's;
//! - `None`   — not a capability, and why;
//! - `Later`  — a capability not yet offered to an agent, and why: the gaps,
//!   in one place, never silent.
//!
//! And what the person's own clicks ask through the intents — `invoke_person`
//! — is listed, so a commit point that stops (or starts) going through them
//! is seen.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug)]
enum Place {
    Intent(&'static str),
    Step(&'static str),
    View(&'static str),
    Machine(&'static str),
    None(&'static str),
    Later(&'static str),
}

use Place::{Intent, Later, Machine, None as NotOne, Step, View};

/// The screens: what the person sees and clicks, and the hosts that drive a
/// screen's own machines.
const SCREENS: [&str; 12] = [
    "wallet",
    "flows",
    "settings",
    "signing",
    "contacts",
    "explore",
    "onboarding.rs",
    "onboarding_flow.rs",
    "hardware.rs",
    "intro.rs",
    "gallery.rs",
    "main.rs",
];

/// gpui's own input events, not the wallet's.
const GPUI_EVENTS: [&str; 8] = [
    "ClickEvent",
    "KeyDownEvent",
    "KeyUpEvent",
    "MouseDownEvent",
    "MouseUpEvent",
    "MouseMoveEvent",
    "ScrollWheelEvent",
    "ModifiersChangedEvent",
];

const FILE: &str = "a file the person picks or saves on this device";
const DAPP: &str = "a dApp's own request, answered by the person to that dApp";
const ONBOARD: &str = "signing in or creating a wallet: the passkey's own ceremony";
const BOUNDARY: &str = "where this account signs is the signing boundary itself (spec 102)";
const SWEEP: &str = "several tokens to one address (sweep)";
const SIGN: &str = "answering a dApp's signing request";
const BROWSE: &str = "browsing in the in-app dApp browser";
/// Spec 105: which space a network the person added belongs to is their word
/// on what has real value. An agent that could move a testnet into the
/// mainnet space could make test coins read as money.
const SPACE_OF: &str = "which space a network belongs to is the person's word on what has value";

/// The core events the screens send, by the name a screen spells them.
const EVENTS: &[(&str, Place)] = &[
    ("BalanceEvent::RefreshRequested", Machine("the balances")),
    ("BatchEvent::Apply", Step("prepare_batch_transfer")),
    ("BatchEvent::EditRate", Step("prepare_batch_transfer")),
    (
        "BatchEvent::Open",
        View("the list importer shown; the list is the step"),
    ),
    ("BatchEvent::PickFileRequested", NotOne(FILE)),
    (
        "BatchEvent::ResetRateToAuto",
        Step("prepare_batch_transfer"),
    ),
    ("BatchEvent::SaveTemplateRequested", NotOne(FILE)),
    ("BatchEvent::SetFiatCode", Step("prepare_batch_transfer")),
    ("BatchEvent::SetRawText", Step("prepare_batch_transfer")),
    ("BatchEvent::SetUnit", Step("prepare_batch_transfer")),
    (
        "ClearEvent::MessagePresented",
        Machine("decoding what is signed"),
    ),
    (
        "ClearEvent::ResolveBatch",
        Machine("decoding what is signed"),
    ),
    (
        "ClearEvent::ResolveTransaction",
        Machine("decoding what is signed"),
    ),
    (
        "ClearEvent::ResolveTypedData",
        Machine("decoding what is signed"),
    ),
    ("ContactEvent::ExportRequested", NotOne(FILE)),
    ("ContactEvent::ExportTaken", NotOne(FILE)),
    ("ContactEvent::ImportAcknowledged", NotOne(FILE)),
    ("ContactEvent::ImportFile", NotOne(FILE)),
    ("ContactEvent::InspectRecipient", Step("show")),
    ("CreateEvent::AckToggled", Later(ONBOARD)),
    ("CreateEvent::AddKey", Later(ONBOARD)),
    ("CreateEvent::ConfirmKey", Later(ONBOARD)),
    ("CreateEvent::EnterWallet", Later(ONBOARD)),
    ("CreateEvent::FinishKeys", Later(ONBOARD)),
    ("CreateEvent::GoBack", Later(ONBOARD)),
    ("CreateEvent::NameChanged", Later(ONBOARD)),
    ("CreateEvent::RemoveKey", Later(ONBOARD)),
    ("CreateEvent::RetryUpload", Later(ONBOARD)),
    ("CreateEvent::SigningPageChosen", Later(ONBOARD)),
    ("CreateEvent::StartOver", Later(ONBOARD)),
    ("CreateEvent::Submit", Later(ONBOARD)),
    ("DbrEvent::AddNetworkAnswered", NotOne(DAPP)),
    ("DbrEvent::ConsentApproved", NotOne(DAPP)),
    ("DbrEvent::ConsentRejected", NotOne(DAPP)),
    (
        "DbrEvent::InspectorClosed",
        View("a site's connection shown"),
    ),
    (
        "DbrEvent::InspectorOpened",
        View("a site's connection shown"),
    ),
    ("DbrEvent::LoadFinished", Machine("the page in the browser")),
    (
        "DbrEvent::NavigationStarted",
        Machine("the page in the browser"),
    ),
    ("DbrEvent::RendererGone", Machine("the page in the browser")),
    ("DbrEvent::RevokeAll", Step("clear_storage_item")),
    (
        "DbrEvent::RevokeRequested",
        Later("disconnecting one connected site (clear_storage_item dapps disconnects them all)"),
    ),
    ("DbrEvent::SigningAnswered", NotOne(DAPP)),
    ("DbrEvent::SiteChainPicked", NotOne(DAPP)),
    ("FeeEvent::BalanceChangesMeasured", Machine("the fee quote")),
    ("FeeEvent::QuoteRequested", Machine("the fee quote")),
    ("FeeEvent::Requote", Machine("the fee quote")),
    (
        "FeeEvent::SelectFeeAsset",
        Later("paying the fee in another token, per transfer"),
    ),
    ("FeedEvent::AccountSwitched", Machine("the account")),
    ("FlowEvent::AckToggled", Later(ONBOARD)),
    ("FlowEvent::AddKey", Later(ONBOARD)),
    ("FlowEvent::Back", Later(ONBOARD)),
    ("FlowEvent::ChooseOwnPage", Later(ONBOARD)),
    ("FlowEvent::ClearOwnPage", Later(ONBOARD)),
    ("FlowEvent::ConfirmKey", Later(ONBOARD)),
    ("FlowEvent::CopyAddress", Later(ONBOARD)),
    ("FlowEvent::EnterWallet", Later(ONBOARD)),
    ("FlowEvent::FinishKeys", Later(ONBOARD)),
    ("FlowEvent::MethodUnavailable", Later(ONBOARD)),
    ("FlowEvent::NameChanged", Later(ONBOARD)),
    ("FlowEvent::RemoveKey", Later(ONBOARD)),
    ("FlowEvent::RetryUpload", Later(ONBOARD)),
    ("FlowEvent::StartOver", Later(ONBOARD)),
    ("FlowEvent::Submit", Later(ONBOARD)),
    ("FlowEvent::TogglePicker", Later(ONBOARD)),
    (
        "GuardEvent::ApprovalDetected",
        Machine("decoding what is signed"),
    ),
    ("IntroEvent::Create", View("the first-run slides")),
    ("IntroEvent::DragEnd", View("the first-run slides")),
    ("IntroEvent::DragMove", View("the first-run slides")),
    ("IntroEvent::DragStart", View("the first-run slides")),
    ("IntroEvent::Next", View("the first-run slides")),
    ("IntroEvent::Prev", View("the first-run slides")),
    ("IntroEvent::SignIn", View("the first-run slides")),
    ("IntroEvent::Skip", View("the first-run slides")),
    ("MtokEvent::AddressInput", Step("add_token")),
    ("MtokEvent::DetectRequested", Step("add_token")),
    ("MtokEvent::SaveRequested", Intent("add_token")),
    ("NetEvent::AddByChainIdRequested", Step("add_network")),
    ("NetEvent::ChainSelected", Step("add_network")),
    ("NetEvent::CustomRpcEdited", Step("add_network")),
    ("NetEvent::CustomSpaceChanged", NotOne(SPACE_OF)),
    ("NetEvent::DappAddApproved", NotOne(DAPP)),
    ("NetEvent::DappAddCancelled", NotOne(DAPP)),
    ("NetEvent::DappAddDeclined", NotOne(DAPP)),
    ("NetEvent::DappAddRequested", NotOne(DAPP)),
    ("NetEvent::DappAddRetried", NotOne(DAPP)),
    ("NetEvent::EndpointBlurred", Intent("set_service_endpoint")),
    ("NetEvent::EndpointEdited", Step("set_service_endpoint")),
    ("NetEvent::EndpointsOpened", Step("set_service_endpoint")),
    ("NetEvent::OverrideBlurred", Intent("set_network_endpoint")),
    ("NetEvent::OverrideExpanded", Step("set_network_endpoint")),
    (
        "NetEvent::OverrideFieldEdited",
        Step("set_network_endpoint"),
    ),
    ("NetEvent::ProviderKeyBlurred", Intent("set_provider_key")),
    ("NetEvent::ProviderKeyEdited", Step("set_provider_key")),
    (
        "NetEvent::ProviderTestRequested",
        Later("testing a provider key; its answer is only drawn in Settings"),
    ),
    ("NetEvent::ProvidersOpened", Step("set_provider_key")),
    ("NetEvent::SearchInput", Step("add_network")),
    ("NetEvent::WizardReset", Step("add_network")),
    ("NetEvent::WizardTestnetChosen", Step("add_network")),
    ("PagesEvent::PageAdded", NotOne(BOUNDARY)),
    ("PagesEvent::PageRemoved", NotOne(BOUNDARY)),
    ("PagesEvent::PageRenamed", NotOne(BOUNDARY)),
    ("PagesEvent::Refresh", Machine("a read")),
    (
        "SendEvent::AddNetworkTapped",
        View("goes to add a network (add_network)"),
    ),
    (
        "SendEvent::AppendSplitRecipients",
        Step("prepare_batch_transfer"),
    ),
    ("SendEvent::Back", Step("cancel_transfer")),
    ("SendEvent::ChangeToken", Step("prepare_transfer")),
    (
        "SendEvent::ChooseFeeToken",
        Later("paying the fee in another token, per transfer"),
    ),
    (
        "SendEvent::CloseBatchImport",
        View("the list importer shown; the list is the step"),
    ),
    (
        "SendEvent::CloseContactPicker",
        View("a picker; the recipient is the step"),
    ),
    (
        "SendEvent::CloseScanner",
        NotOne("the camera, which an agent has no use for"),
    ),
    ("SendEvent::ConfirmMultiSelection", Later(SWEEP)),
    ("SendEvent::Continue", Step("prepare_transfer")),
    (
        "SendEvent::DismissRelayUnreachable",
        View("a notice dismissed"),
    ),
    (
        "SendEvent::DismissTreasurySheet",
        View("a notice dismissed"),
    ),
    ("SendEvent::DisplayChanged", Machine("the display currency")),
    ("SendEvent::Done", View("the receipt closed")),
    ("SendEvent::EditAmount", Step("prepare_transfer")),
    ("SendEvent::EnterSplitMode", Step("prepare_batch_transfer")),
    ("SendEvent::FeeBusyChanged", Machine("the fee quote")),
    ("SendEvent::FeeFailedChanged", Machine("the fee quote")),
    ("SendEvent::FeeTokenChanged", Machine("the fee quote")),
    ("SendEvent::FeeUpdated", Machine("the fee quote")),
    ("SendEvent::HoldingsUpdated", Machine("the balances")),
    ("SendEvent::InFlightOps", Machine("the chain tracker")),
    ("SendEvent::OpSigned", Machine("the passkey's ceremony")),
    (
        "SendEvent::Open",
        Machine("the send opening on what it is for"),
    ),
    (
        "SendEvent::OpenBatchImport",
        View("the list importer shown; the list is the step"),
    ),
    (
        "SendEvent::OpenContactPicker",
        View("a picker; the recipient is the step"),
    ),
    (
        "SendEvent::OpenScanner",
        NotOne("the camera, which an agent has no use for"),
    ),
    ("SendEvent::PickedAddress", Step("prepare_transfer")),
    ("SendEvent::ReceiptUpdate", Machine("the chain tracker")),
    (
        "SendEvent::RecipientsChanged",
        Step("prepare_batch_transfer"),
    ),
    (
        "SendEvent::RetryAfterBootstrap",
        View("the review's own retry"),
    ),
    (
        "SendEvent::RetryRelayUnreachable",
        View("the review's own retry"),
    ),
    (
        "SendEvent::ScanResolved",
        NotOne("the camera, which an agent has no use for"),
    ),
    (
        "SendEvent::SeedSplitRecipients",
        Step("prepare_batch_transfer"),
    ),
    ("SendEvent::SelectToken", Step("prepare_transfer")),
    ("SendEvent::SetAmount", Step("prepare_transfer")),
    ("SendEvent::SetMultiNetwork", Later(SWEEP)),
    ("SendEvent::SetRecipient", Step("prepare_transfer")),
    (
        "SendEvent::SigningStarted",
        Machine("the passkey's ceremony"),
    ),
    ("SendEvent::TapMax", Step("prepare_transfer")),
    ("SendEvent::ToggleAllMultiTokens", Later(SWEEP)),
    (
        "SendEvent::ToggleFiatInput",
        View("typing the amount in fiat or in the token"),
    ),
    ("SendEvent::ToggleMultiToken", Later(SWEEP)),
    ("SendEvent::TokensPartial", Machine("the balances")),
    ("SignEvent::AccountsChanged", Machine("the accounts")),
    ("SignEvent::ApproveTapped", Later(SIGN)),
    (
        "SignEvent::FundingCompleteTapped",
        View("the request's funding notice answered"),
    ),
    ("SignEvent::InFlightOps", Machine("the chain tracker")),
    ("SignEvent::NetworksChanged", Machine("the networks")),
    ("SignEvent::OpTracked", Machine("the chain tracker")),
    (
        "SignEvent::RequestArrived",
        Machine("a dApp's request arriving"),
    ),
    ("SignEvent::SimSettled", Machine("the simulation")),
    ("SignEvent::SimStarted", Machine("the simulation")),
    ("SignEvent::SwipeDismissed", Later(SIGN)),
    ("SigningPagesEvent::VersionTrusted", NotOne(BOUNDARY)),
    (
        "SitesEvent::SystemGroupHiddenSet",
        View("which site groups the browser shows"),
    ),
    ("SpeedEvent::Configure", Machine("the fee quote")),
    (
        "SpeedEvent::Pick",
        Later("choosing one transfer's own speed (the default is set_preference fee_speed)"),
    ),
    ("SpeedEvent::QuotesChanged", Machine("the fee quote")),
    ("SpeedEvent::Reset", Machine("the fee quote")),
    ("SpeedEvent::StageChanged", Machine("the fee quote")),
    ("SpeedEvent::Toggle", View("the speed choices shown")),
    (
        "activity_feed::Event::ChainFilterChanged",
        View("the network filter"),
    ),
    ("approval_guard::Event::CustomAmountChanged", Later(SIGN)),
    ("approval_guard::Event::LegCustomAmountChanged", Later(SIGN)),
    ("approval_guard::Event::LegPresetSelected", Later(SIGN)),
    ("approval_guard::Event::PresetSelected", Later(SIGN)),
    (
        "balance_dashboard::Event::AppBackgrounded",
        Machine("the window's focus"),
    ),
    (
        "balance_dashboard::Event::AppFocused",
        Machine("the window's focus"),
    ),
    (
        "balance_dashboard::Event::FixChainResolved",
        View("a network's fix shown"),
    ),
    (
        "balance_dashboard::Event::SwitcherClosed",
        View("the account switcher shown"),
    ),
    (
        "balance_dashboard::Event::SwitcherOpened",
        View("the account switcher shown"),
    ),
    (
        "balance_dashboard::Event::UnreachableListClosed",
        View("a list shown"),
    ),
    (
        "balance_dashboard::Event::UnreachableListOpened",
        View("a list shown"),
    ),
    (
        "browser_history::Event::ClearAll",
        Step("clear_storage_item"),
    ),
    (
        "browser_history::Event::DeleteOrigin",
        Later("forgetting one site of the browsing history"),
    ),
    (
        "browser_history::Event::VisitRecorded",
        Machine("a page visited"),
    ),
    ("create_wallet::Event::SigningPageChosen", Later(ONBOARD)),
    ("create_wallet::Event::Start", Later(ONBOARD)),
    ("explore_sites::Event::FavoriteAdded", Later(BROWSE)),
    ("explore_sites::Event::FavoriteRemoved", Later(BROWSE)),
    ("explore_sites::Event::FavoriteRenamed", Later(BROWSE)),
    (
        "explore_sites::Event::PageLoaded",
        Machine("the page in the browser"),
    ),
    ("explore_sites::Event::TabClosed", Later(BROWSE)),
    ("explore_sites::Event::TabNavigated", Later(BROWSE)),
    ("explore_sites::Event::TabOpened", Later(BROWSE)),
    ("explore_sites::Event::TabSelected", Later(BROWSE)),
    ("explore_sites::Event::TabsClosed", Later(BROWSE)),
    ("login::Event::SignIn", Later(ONBOARD)),
    ("login::Event::Start", Later(ONBOARD)),
    (
        "payment_request::Event::Acknowledge",
        View("what the receive code carries"),
    ),
    (
        "payment_request::Event::AssetPicked",
        View("what the receive code carries"),
    ),
    (
        "payment_request::Event::IncludeNetworkChanged",
        View("what the receive code carries"),
    ),
    (
        "sign_request::Event::FundingCompleteTapped",
        View("the request's funding notice answered"),
    ),
    (
        "sign_request::Event::RetryTapped",
        View("the request's own retry"),
    ),
    ("sign_request::Event::SwipeDismissed", Later(SIGN)),
    (
        "sign_request::Event::TransportDropped",
        Machine("the dApp's connection"),
    ),
    ("tx_tracker::Event::Submitted", Machine("the chain tracker")),
];

/// Direct calls into the session, the preferences, the storage and the
/// residents' own functions.
const CALLS: &[(&str, Place)] = &[
    (
        "activity_feed::contact_changed",
        View("a contact's page shown"),
    ),
    ("activity_feed::focus_tick", Step("refresh_balances")),
    ("activity_feed::is_stable", Machine("a read")),
    (
        "activity_feed::privacy_changed",
        Step("set_balance_privacy"),
    ),
    ("activity_feed::reconciled", Machine("the chain tracker")),
    ("activity_feed::set_visible", View("the history shown")),
    ("activity_feed::start_ticks", Machine("a tick")),
    (
        "balance_dashboard::dispatch",
        Machine("its events are placed above"),
    ),
    ("balance_dashboard::pull", Intent("refresh_balances")),
    ("balance_dashboard::refresh", Intent("refresh_balances")),
    ("balance_dashboard::settled", Machine("a read")),
    ("balance_dashboard::start_ticks", Machine("a tick")),
    ("balance_dashboard::take_invalidation", Machine("a read")),
    ("device_storage::clear_caches", Intent("clear_caches")),
    ("device_storage::clear_item", Intent("clear_storage_item")),
    ("device_storage::erase", Intent("erase_device")),
    ("format_prefs::choice", Machine("a read")),
    ("format_prefs::current", Machine("a read")),
    ("format_prefs::date_choice_word", Machine("a read")),
    ("format_prefs::date_word", Machine("a read")),
    ("format_prefs::fiat_options", Machine("a read")),
    ("format_prefs::machine", Machine("a read")),
    ("format_prefs::number_word", Machine("a read")),
    (
        "format_prefs::reload",
        Machine("the store read again after an erase"),
    ),
    ("format_prefs::set_date_word", Intent("set_preference")),
    ("format_prefs::set_number_word", Intent("set_preference")),
    ("format_prefs::set_time_word", Intent("set_preference")),
    ("format_prefs::time_choice_word", Machine("a read")),
    ("format_prefs::time_word", Machine("a read")),
    ("preferences::boot", Machine("boot")),
    ("preferences::current", Machine("a read")),
    ("preferences::debug_mode", Machine("a read")),
    (
        "preferences::load",
        Machine("the store read again after an erase"),
    ),
    ("preferences::pinned_language", Machine("a read")),
    (
        "preferences::set_debug_mode",
        NotOne("a developer's switch (spec 091), hidden in a release build"),
    ),
    ("preferences::set_language", Intent("set_preference")),
    ("preferences::set_text_scale", Intent("set_preference")),
    ("preferences::set_theme", Intent("set_preference")),
    (
        "resident::forget::<ActivityFeed>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<BrowserHistory>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<Contacts>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<ExploreSites>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<ManageTokens>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<NetworkAdmin>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<PaymentRequest>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "resident::forget::<ReceiveWatch>",
        Machine("a store cleared under its machine: it boots from disk again"),
    ),
    (
        "session::account_established",
        Machine("the account the pages were given"),
    ),
    ("session::add_account", Later(ONBOARD)),
    ("session::add_account_cancelled", Later(ONBOARD)),
    ("session::adding_account", Machine("a read")),
    ("session::boot", Machine("boot")),
    ("session::choose_venue", NotOne(BOUNDARY)),
    ("session::reboot", Step("erase_device")),
    ("session::sign_out_confirmed", Step("sign_out")),
    ("session::sign_out_dismissed", Step("sign_out")),
    ("session::view", Machine("a read")),
    ("tracker::focused", Machine("the chain tracker")),
    ("tracker::in_flight_news", Machine("the chain tracker")),
    ("tracker::start", Machine("the chain tracker")),
    ("tracker::submitted", Machine("the chain tracker")),
    ("tracker::withdrawn", Machine("the chain tracker")),
];

/// What the person's clicks ask through the intents (spec 103 D3): the same
/// way in as an agent's, by the same action. The web's list, action for action.
const PERSON: &[&str] = &[
    "add_network",
    "clear_caches",
    "clear_storage_item",
    "configure_agent",
    "delete_activity",
    "delete_contact",
    "delete_group",
    "erase_device",
    "refresh_balances",
    "remove_network",
    "remove_wallet",
    "reset_service_endpoints",
    "save_contact",
    "save_group",
    "set_balance_privacy",
    "set_contact_groups",
    "set_group_members",
    "set_network_space",
    "set_preference",
    "sign_out",
    "submit_transfer",
    "switch_account",
];

fn sources() -> Vec<(PathBuf, String)> {
    fn walk(path: &Path, found: &mut Vec<(PathBuf, String)>) {
        if path.is_dir() {
            let mut entries: Vec<_> = std::fs::read_dir(path)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .collect();
            entries.sort();
            for entry in entries {
                walk(&entry, found);
            }
        } else if path.extension().is_some_and(|ext| ext == "rs")
            && let Ok(text) = std::fs::read_to_string(path)
        {
            found.push((path.to_path_buf(), text));
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut found = Vec::new();
    for screen in SCREENS {
        walk(&root.join(screen), &mut found);
    }
    found
}

fn is_ident(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn ident_after(text: &str) -> &str {
    let end = text.find(|ch: char| !is_ident(ch)).unwrap_or(text.len());
    &text[..end]
}

fn ident_before(text: &str) -> &str {
    let start = text.rfind(|ch: char| !is_ident(ch)).map_or(0, |at| at + 1);
    &text[start..]
}

/// `SendEvent::SetAmount`, and `activity_feed::Event::DeleteRequested` where a
/// screen names the event by its module.
fn events(text: &str, found: &mut BTreeMap<String, String>, file: &str) {
    let mut from = 0;
    while let Some(at) = text[from..].find("Event::") {
        let at = from + at;
        from = at + "Event::".len();
        let variant = ident_after(&text[from..]);
        if variant.is_empty() || !variant.starts_with(|ch: char| ch.is_ascii_uppercase()) {
            continue;
        }
        let head = ident_before(&text[..at]);
        let key = if head.is_empty() {
            // `module::Event::Variant`.
            let before = &text[..at];
            let Some(module) = before.strip_suffix("::").map(ident_before) else {
                continue;
            };
            if module.is_empty() {
                continue;
            }
            format!("{module}::Event::{variant}")
        } else {
            let name = format!("{head}Event");
            if GPUI_EVENTS.contains(&name.as_str())
                || !head.starts_with(|ch: char| ch.is_ascii_uppercase())
            {
                continue;
            }
            format!("{name}::{variant}")
        };
        found.entry(key).or_insert_with(|| file.to_owned());
    }
}

/// `session::switch_account(`, `resident::forget::<Contacts>(` …
fn calls(text: &str, found: &mut BTreeMap<String, String>, file: &str) {
    const MODULES: [&str; 8] = [
        "session",
        "preferences",
        "format_prefs",
        "device_storage",
        "balance_dashboard",
        "activity_feed",
        "tracker",
        "resident",
    ];
    for module in MODULES {
        let prefix = format!("{module}::");
        let mut from = 0;
        while let Some(at) = text[from..].find(&prefix) {
            let at = from + at;
            from = at + prefix.len();
            if text[..at].ends_with(is_ident) {
                continue;
            }
            let name = ident_after(&text[from..]);
            let rest = &text[from + name.len()..];
            let key = if module == "resident" && name == "forget" && rest.starts_with("::<") {
                let ty = ident_after(&rest[3..]);
                format!("resident::forget::<{ty}>")
            } else if rest.starts_with('(') && name.starts_with(|ch: char| ch.is_ascii_lowercase())
            {
                if module == "resident" {
                    continue; // reading a resident is not a call into one
                }
                format!("{module}::{name}")
            } else {
                continue;
            };
            found.entry(key).or_insert_with(|| file.to_owned());
        }
    }
}

/// `invoke_person(Action::X …` and `Self::prefer(` (a preference).
fn person(text: &str, found: &mut BTreeSet<String>) {
    for call in ["invoke_person(", "invoke_person_then("] {
        let mut from = 0;
        while let Some(at) = text[from..].find(call) {
            from += at + call.len();
            let rest = text[from..].trim_start();
            if let Some(rest) = rest.strip_prefix("Action::") {
                found.insert(ident_after(rest).to_owned());
            } else if rest.starts_with("action") {
                // A variable: the pick sheet's, named where it is built.
                continue;
            }
        }
    }
    if text.contains("Self::prefer(") {
        found.insert("SetPreference".to_owned());
    }
}

fn scan() -> (
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    BTreeSet<String>,
) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let (mut dispatched, mut called, mut asked) =
        (BTreeMap::new(), BTreeMap::new(), BTreeSet::new());
    for (path, text) in sources() {
        let file = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .display()
            .to_string();
        // Tests describe the screens; they are not the screens. A file's
        // test module is its last item.
        let text = text
            .find("\n#[cfg(test)]\nmod ")
            .map_or(text.as_str(), |at| &text[..at]);
        events(text, &mut dispatched, &file);
        calls(text, &mut called, &file);
        person(text, &mut asked);
    }
    (dispatched, called, asked)
}

fn unplaced(ledger: &[(&str, Place)], found: &BTreeMap<String, String>) -> Vec<String> {
    found
        .iter()
        .filter(|(name, _)| !ledger.iter().any(|(placed, _)| placed == *name))
        .map(|(name, file)| format!("{name} ({file})"))
        .collect()
}

fn stale(ledger: &[(&'static str, Place)], found: &BTreeMap<String, String>) -> Vec<&'static str> {
    ledger
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !found.contains_key(*name))
        .collect()
}

/// `SetPreference` as the catalog spells it: `set_preference`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if ch.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[test]
fn places_every_event_a_screen_sends() {
    let (dispatched, _, _) = scan();
    assert_eq!(
        unplaced(EVENTS, &dispatched),
        Vec::<String>::new(),
        "place each in this ledger"
    );
    assert_eq!(
        stale(EVENTS, &dispatched),
        Vec::<&str>::new(),
        "no longer sent: drop them"
    );
}

#[test]
fn places_every_direct_call_a_screen_makes() {
    let (_, called, _) = scan();
    assert_eq!(
        unplaced(CALLS, &called),
        Vec::<String>::new(),
        "place each in this ledger"
    );
    assert_eq!(
        stale(CALLS, &called),
        Vec::<&str>::new(),
        "no longer called: drop them"
    );
}

#[test]
fn lists_what_the_persons_clicks_ask_through_the_intents() {
    let (_, _, asked) = scan();
    let asked: Vec<String> = asked.iter().map(|name| snake(name)).collect();
    let mut listed: Vec<String> = PERSON.iter().map(|name| (*name).to_owned()).collect();
    listed.sort();
    assert_eq!(asked, listed);
}

#[test]
fn names_only_actions_the_catalog_has() {
    let catalog: BTreeSet<String> = vela_core::app::intents::catalog()
        .into_iter()
        .map(|entry| entry.action)
        .collect();
    let named: Vec<&str> = EVENTS
        .iter()
        .chain(CALLS)
        .filter_map(|(_, place)| match place {
            Intent(action) | Step(action) => Some(*action),
            _ => None,
        })
        .chain(PERSON.iter().copied())
        .filter(|action| !catalog.contains(*action))
        .collect();
    assert_eq!(named, Vec::<&str>::new());
    // Every kind of place is used; a reason is never empty.
    for (name, place) in EVENTS.iter().chain(CALLS) {
        let (View(why) | Machine(why) | NotOne(why) | Later(why) | Intent(why) | Step(why)) = place;
        assert!(!why.is_empty(), "{name}");
    }
}
