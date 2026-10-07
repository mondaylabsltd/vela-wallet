//! The outcome catalog, and the one modal the v2 flow has.
//!
//! ## Where spec 014's eighteen outcomes went
//!
//! The v2 design demonstrates its failure sheet with two examples. That is a
//! PATTERN, not an inventory, and research D13's decision was to RE-SKIN the
//! eighteen rather than reduce them. Re-skinned is what happened — but eight of
//! them are no longer SHEETS, because v2 gave them somewhere better to live:
//!
//! | 014 outcome | v2 |
//! | --- | --- |
//! | `Created` | the Done screen |
//! | `SignedIn` | the wallet itself |
//! | `SyncFailed` | the Retry screen, with the whole key list intact |
//! | `VerifyStuck` | the Name screen, with the 完成验证 submit label |
//! | `CancelledSetup` / `CancelledVerify` | the Name screen's quiet status line |
//! | `LoginCancelled` | the same, on the sign-in path |
//! | `AccountNotFound` | a `sign_in_failed` prompt carrying the registry's words |
//!
//! Their COPY survives — that is what D13 asked for, and the corpus keys are
//! unchanged. What did not survive is the idea that all eighteen are the same
//! kind of thing. A cancelled ceremony with a filled-in form behind it is not a
//! modal, and making it one was 014's container talking. So this file names the
//! ten a sheet can actually raise, and the eight above are found in
//! `onboarding_flow.rs` instead of here.
//!
//! ## What the catalog is allowed to decide
//!
//! The badge and the copy. Never the flow.
//!
//! In spec 014 each outcome also carried its own action stack, because the
//! panel had no core behind it. In v2 the buttons come from the CORE:
//! `confirmable` selects a two-button dialog whose answer is a business
//! decision, and every other prompt gets one dismiss. So the refinement below —
//! reading a failure's own words to choose between "the network" and "the
//! server" — selects a SENTENCE and nothing else. If it guesses wrong the
//! person reads a slightly less apt title; they never end up on a different
//! path, because there is no other path to end up on.
//!
//! ## The sheet is the only modal
//!
//! The whole journey is a full page. Only failures are modal, because a failure
//! genuinely does stop everything until it is acknowledged. On this form factor
//! that is a centred 400 px card over a scrim — no drag handle, which is the
//! mobile affordance.

use gpui::{
    App, Div, FontWeight, InteractiveElement as _, MouseButton, ParentElement, SharedString,
    Stateful, StatefulInteractiveElement as _, Styled, Window, div, px,
};

use vela_core::app::PromptKind;

use crate::loc::Loc;
use crate::theme::{
    self, FLOW_GAP_LG, FLOW_GAP_MD, FLOW_GAP_SM, HAIRLINE, OPACITY_DISABLED, RADIUS_FIELD, Theme,
};
use crate::ui::{ButtonVariant, status_badge, vela_button};

/// The centred card's width, from the v2 desktop mock.
pub const SHEET_W: f32 = 400.;
pub const SHEET_PAD: f32 = 28.;
pub const SHEET_RADIUS: f32 = 20.;
/// The scrim behind it. Opacity rather than a token colour so the same value
/// reads correctly on both themes.
pub const SCRIM_OPACITY: f32 = 0.55;

/// Every press the sheet can emit.
///
/// Trimmed from spec 014, whose ids addressed a container the redesign removed.
/// What is left is the ANSWER — which the core owns — plus the two affordances
/// that deliberately do NOT answer, because opening the endpoint settings or
/// copying a diagnostic is not a reply to "did you accept?".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionId {
    /// `prompt_answered { accepted: true }`.
    Accept,
    /// `prompt_answered { accepted: false }`. A dismissal produces this too.
    Decline,
    /// Opens the endpoint surface. Leaves the sheet up.
    EditIndexEndpoint,
    /// Copies the diagnostics for a bug report. Leaves the sheet up.
    ReportError,
    ToggleDetails,
}

/// Badge circle variants (spec 014 data-model §3).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BadgeVariant {
    Warning,
    Error,
    Timeout,
    Info,
}

/// The ten a failure sheet can raise. See the module note for the eight that
/// are screens now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutcomeKind {
    /// A key is plugged in and this process cannot open it.
    KeyUnreadable,
    Network,
    Server,
    Timeout,
    Unsupported,
    Incompatible,
    NotDiscoverable,
    Unknown,
    RecoverOffer,
    RecoverFailed,
    SignInFailed,
    /// The link to the other device failed, not the authenticator — the
    /// core's `phone_link` (issue #446): scan again, not "set up Face ID".
    PhoneLink,
}

/// The copy and the badge for one outcome. No actions: see the module note.
#[derive(Clone, Debug)]
pub struct OutcomeSpec {
    pub badge: BadgeVariant,
    pub headline: SharedString,
    pub body: SharedString,
}

impl OutcomeKind {
    /// The pure `kind → OutcomeSpec` catalog.
    pub fn spec(self, loc: &Loc) -> OutcomeSpec {
        let (badge, headline, body) = match self {
            Self::KeyUnreadable => (
                BadgeVariant::Error,
                loc.t("onboarding.create.keyUnreadableTitle"),
                loc.t("onboarding.create.keyUnreadableBody"),
            ),
            Self::Network => (
                BadgeVariant::Error,
                loc.t("onboarding.common.networkTitle"),
                loc.t("onboarding.common.networkBody"),
            ),
            Self::Server => (
                BadgeVariant::Error,
                loc.t("onboarding.common.serverTitle"),
                loc.t("onboarding.common.serverBody"),
            ),
            Self::Timeout => (
                BadgeVariant::Timeout,
                loc.t("onboarding.common.timeoutTitle"),
                // The publish's own budget: `await_task` gives up at 120 s.
                loc.t_vars("onboarding.common.timeoutBody", &[("seconds", 120.)]),
            ),
            // NOT `onboarding.common.unsupportedTitle/Body`, which says "this
            // device has no usable biometric authentication". That is true of a
            // phone whose fingerprint reader is off; on a desktop it is a
            // non-sequitur, because no desktop here has a biometric path at all
            // and the wallet never asked for one. `not_supported` on this
            // platform means one thing — there is no security key it can reach
            // — and the corpus already carries that sentence.
            Self::Unsupported => (
                BadgeVariant::Error,
                loc.t("onboarding.create.securityKeyRequiredTitle"),
                loc.t("onboarding.create.securityKeyRequiredBody"),
            ),
            Self::Incompatible => (
                BadgeVariant::Error,
                loc.t("onboarding.common.incompatibleTitle"),
                loc.t("onboarding.common.incompatibleBody"),
            ),
            Self::NotDiscoverable => (
                BadgeVariant::Warning,
                loc.t("onboarding.common.notDiscoverableTitle"),
                loc.t("onboarding.common.notDiscoverableBody"),
            ),
            Self::Unknown => (
                BadgeVariant::Error,
                loc.t("onboarding.common.unknownTitle"),
                loc.t("onboarding.common.unknownBody"),
            ),
            Self::RecoverOffer => (
                BadgeVariant::Info,
                loc.t("onboarding.login.recoverOfferTitle"),
                loc.t("onboarding.login.recoverOfferBody"),
            ),
            Self::RecoverFailed => (
                BadgeVariant::Error,
                loc.t("onboarding.login.recoverFailedTitle"),
                loc.t("onboarding.login.recoverFailedBody"),
            ),
            Self::SignInFailed => (
                BadgeVariant::Error,
                loc.t("onboarding.login.alertSignInFailedTitle"),
                loc.t("onboarding.login.signInFailedBody"),
            ),
            Self::PhoneLink => (
                BadgeVariant::Error,
                loc.t("onboarding.common.unknownTitle"),
                loc.t("onboarding.common.phoneLinkFailed"),
            ),
        };

        OutcomeSpec {
            badge,
            headline,
            body,
        }
    }

    /// Which outcome a core prompt is.
    ///
    /// Nine `PromptKind` variants, ten outcomes: the two that carry a `detail`
    /// string are refined by reading it, which is where the extra ones come
    /// from. See the module note on what that refinement is allowed to change —
    /// a sentence, never a path.
    pub fn for_prompt(kind: &PromptKind) -> Self {
        match kind {
            // Every route a desktop has is a security key, so `security_key`
            // changes nothing here: the sheet already talks about the key.
            PromptKind::NotSupportedCreate { .. } | PromptKind::NotSupportedLogin { .. } => {
                Self::Unsupported
            }
            PromptKind::NotDiscoverable => Self::NotDiscoverable,
            PromptKind::IncompatibleCreate | PromptKind::IncompatibleLogin => Self::Incompatible,
            PromptKind::RecoverOffer => Self::RecoverOffer,
            PromptKind::RecoverFailed => Self::RecoverFailed,
            PromptKind::CreateFailed {
                phone_link: true, ..
            }
            | PromptKind::SignInFailed {
                phone_link: true, ..
            } => Self::PhoneLink,
            PromptKind::CreateFailed { detail, .. } => refine(detail, Self::Unknown),
            PromptKind::SignInFailed { detail, .. } => refine(detail, Self::SignInFailed),
        }
    }
}

/// Read a failure's own words for a shape the corpus has better copy for.
///
/// The messages come from this app's own executor — `registry.rs` prefixes
/// every one with its label, and `usb.rs` with its own — so these are not
/// guesses at somebody else's error strings. `fallback` is what a message that
/// matches nothing keeps, which is the honest answer rather than a nearest fit.
fn refine(detail: &str, fallback: OutcomeKind) -> OutcomeKind {
    let lowered = detail.to_lowercase();
    if lowered.contains("could not be opened") {
        // `usb.rs` phrases the permission failure that way, and it is the one
        // refinement that changes what a person should DO rather than only how
        // the failure is described: the key is right there and the fix is a
        // udev rule, not another port.
        OutcomeKind::KeyUnreadable
    } else if lowered.contains("timed out") || lowered.contains("timeout") {
        OutcomeKind::Timeout
    } else if lowered.contains("no security key") || lowered.contains("usb hid unavailable") {
        OutcomeKind::Unsupported
    } else if lowered.contains("discoverable") {
        OutcomeKind::NotDiscoverable
    } else if lowered.contains("http status")
        || lowered.contains("failed: 4")
        || lowered.contains("failed: 5")
    {
        OutcomeKind::Server
    } else if lowered.contains("dns")
        || lowered.contains("connection")
        || lowered.contains("host")
        || lowered.contains("tls")
    {
        OutcomeKind::Network
    } else {
        fallback
    }
}

// ---------------------------------------------------------------------------
// The sheet
// ---------------------------------------------------------------------------

/// What the sheet is currently showing, and what answering it means.
///
/// Held by whichever screen raised it. `answer` is not optional: a dismissal is
/// `accepted: false`, because a prompt the core is waiting on must always get
/// an answer — leaving it unanswered strands the machine.
#[derive(Clone, Debug)]
pub struct Prompt {
    pub kind: PromptKind,
    /// The core's word for "this answer changes the flow".
    pub confirmable: bool,
    /// The effect id this prompt is answering.
    pub effect_id: u64,
    /// Runtime diagnostics, when the prompt carries any.
    pub details: Option<String>,
    pub details_expanded: bool,
}

impl Prompt {
    pub fn new(kind: PromptKind, confirmable: bool, effect_id: u64) -> Self {
        // The two prompts that carry the platform's own words are the two whose
        // detail belongs in the disclosure rather than in the body.
        let details = match &kind {
            PromptKind::CreateFailed { detail, .. } | PromptKind::SignInFailed { detail, .. } => {
                Some(detail.clone())
            }
            _ => None,
        };
        Self {
            kind,
            confirmable,
            effect_id,
            details,
            details_expanded: false,
        }
    }

    /// Whether the endpoint affordance belongs on this sheet: it does when the
    /// failure was the registry not answering, and nowhere else.
    pub fn offers_endpoint(&self) -> bool {
        matches!(
            OutcomeKind::for_prompt(&self.kind),
            OutcomeKind::Network | OutcomeKind::Server | OutcomeKind::Timeout
        )
    }
}

/// The centred failure card, over a scrim.
///
/// `on_action` receives every press. The two answering ids close the sheet and
/// resolve the core's prompt; the other two are affordances that leave it open,
/// because opening the endpoint settings or copying a diagnostic is not an
/// answer to "did you accept?".
pub fn outcome_sheet(
    theme: &Theme,
    loc: &Loc,
    prompt: &Prompt,
    on_action: impl Fn(ActionId, &mut Window, &mut App) + Clone + 'static,
) -> Stateful<Div> {
    let spec = OutcomeKind::for_prompt(&prompt.kind).spec(loc);

    let mut card = div()
        .w(px(SHEET_W))
        .flex()
        .flex_col()
        .items_center()
        .gap(px(FLOW_GAP_LG))
        .p(px(SHEET_PAD))
        .rounded(px(SHEET_RADIUS))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(theme.border_card)
        .child(status_badge(theme, spec.badge))
        .child(
            div()
                .w_full()
                .flex()
                .flex_col()
                .gap(px(FLOW_GAP_SM))
                .child(
                    div()
                        .text_size(theme::text_flow_headline())
                        .font_weight(FontWeight::BOLD)
                        .text_color(theme.fg_base)
                        .child(spec.headline),
                )
                .child(
                    div()
                        .text_size(theme::text_body())
                        .line_height(theme::line_height_body())
                        .text_color(theme.fg_muted)
                        .child(spec.body),
                ),
        );

    if let Some(details) = &prompt.details {
        card = card.child(details_block(
            theme,
            loc,
            details,
            prompt.details_expanded,
            on_action.clone(),
        ));
    }

    let (primary_id, primary_key, secondary) = if prompt.confirmable {
        (
            ActionId::Accept,
            "onboarding.login.recoverConfirm",
            Some((ActionId::Decline, "onboarding.login.recoverCancel")),
        )
    } else {
        (ActionId::Decline, "onboarding.common.back", None)
    };

    let mut actions = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_MD))
        .child({
            let on_action = on_action.clone();
            vela_button(
                "outcome-primary",
                ButtonVariant::Primary,
                loc.t(primary_key),
                theme,
                move |_, window, cx| on_action(primary_id, window, cx),
            )
        });

    if let Some((id, key)) = secondary {
        let on_action = on_action.clone();
        actions = actions.child(vela_button(
            "outcome-secondary",
            ButtonVariant::Row,
            loc.t(key),
            theme,
            move |_, window, cx| on_action(id, window, cx),
        ));
    }

    if prompt.offers_endpoint() {
        let on_action = on_action.clone();
        actions = actions.child(vela_button(
            "outcome-endpoint",
            ButtonVariant::Row,
            loc.t("onboarding.common.editIndexEndpoint"),
            theme,
            move |_, window, cx| on_action(ActionId::EditIndexEndpoint, window, cx),
        ));
    }

    card = card.child(actions);

    // The scrim swallows clicks. It does NOT dismiss: every prompt here is
    // something the core is waiting on, and a stray click outside a card is not
    // an answer a person meant to give.
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme.bg_base.opacity(SCRIM_OPACITY))
        .id("outcome-scrim")
        .on_mouse_down(MouseButton::Left, |_, _, _| {})
        .child(card)
}

/// The 技术详情 disclosure.
fn details_block(
    theme: &Theme,
    loc: &Loc,
    details: &str,
    expanded: bool,
    on_action: impl Fn(ActionId, &mut Window, &mut App) + Clone + 'static,
) -> Div {
    let toggle_action = on_action.clone();
    let header = div()
        .id("outcome-details-toggle")
        .w_full()
        .flex()
        .items_center()
        .gap(px(FLOW_GAP_SM))
        .cursor_pointer()
        .text_size(theme::text_flow_caption())
        .text_color(theme.fg_muted)
        .on_click(move |_, window, cx| toggle_action(ActionId::ToggleDetails, window, cx))
        .child(if expanded { "▾" } else { "▸" })
        .child(loc.t("onboarding.create.technicalDetails"));

    let mut block = div()
        .w_full()
        .flex()
        .flex_col()
        .gap(px(FLOW_GAP_SM))
        .child(div().h(px(HAIRLINE)).w_full().bg(theme.divider))
        .child(header);

    if expanded {
        let copy_action = on_action.clone();
        block = block
            .child(
                div()
                    .w_full()
                    .p(px(FLOW_GAP_MD))
                    .rounded(px(RADIUS_FIELD))
                    .bg(theme.bg_well)
                    .font_family(theme::font_mono())
                    .text_size(theme::text_flow_caption())
                    .text_color(theme.error_base)
                    .child(SharedString::from(details.to_owned())),
            )
            .child(
                div()
                    .id("outcome-report")
                    .cursor_pointer()
                    .text_size(theme::text_flow_caption())
                    .text_color(theme.fg_muted)
                    .hover(|s| s.opacity(1. - OPACITY_DISABLED))
                    .on_click(move |_, window, cx| copy_action(ActionId::ReportError, window, cx))
                    .child(loc.t("onboarding.common.reportError")),
            );
    }
    block
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `not_supported` on a desktop is not a sentence about biometrics.
    ///
    /// The shared catalog's copy — "This device has no usable biometric
    /// authentication" — is true of a phone with its fingerprint reader off. No
    /// desktop here has a biometric path at all, and the wallet never asked for
    /// one, so on this platform the sheet has exactly one thing to say: plug in
    /// a security key.
    #[test]
    fn the_unsupported_sheet_talks_about_security_keys_not_biometrics() {
        // Compared as KEYS, not as words: the assertion is about which corpus
        // entry this outcome pulls, and asserting on the English would make the
        // test pass or fail depending on the developer's `LANG`.
        let loc = Loc::from_env();
        let spec = OutcomeKind::Unsupported.spec(&loc);
        assert_eq!(
            spec.body,
            loc.t("onboarding.create.securityKeyRequiredBody"),
            "the desktop sheet must say what to plug in"
        );
        assert_ne!(
            spec.body,
            loc.t("onboarding.common.unsupportedBody"),
            "…and must not say this device has no usable biometric authentication"
        );
        assert_eq!(
            spec.headline,
            loc.t("onboarding.create.securityKeyRequiredTitle")
        );
    }

    /// The prompt a missing key produces really is the one above.
    #[test]
    fn a_missing_key_prompt_lands_on_unsupported() {
        assert_eq!(
            OutcomeKind::for_prompt(&PromptKind::NotSupportedCreate { security_key: true }),
            OutcomeKind::Unsupported
        );
    }
}
