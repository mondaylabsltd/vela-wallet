//! The advisor (spec 103, P2.3): the person's AI agent may give its opinion
//! on the dApp signing request on screen. The tools and their words, whether
//! an opinion is accepted and what of it is kept, and whether it still stands
//! against the sheet are all the core's (`vela_agent::tools::{advise,
//! advice_card, open_signing}` over `agent_review`) — one rule for every
//! shell. This file keeps the last accepted opinion and its card's standing,
//! and words the card — its own card, never a word of Vela's, greyed once it
//! no longer stands. It changes nothing that is signed, no button and none of
//! the wallet's own warnings.

use gpui::{App, Global, SharedString};
use serde_json::Value;
use vela_agent::tools as shared;

/// The last opinion the core accepted, for the sheet.
#[derive(Default)]
struct Advice(Option<Value>);

impl Global for Advice {}

/// The card as the core last stood it against the sheet (`advice_card`'s
/// `{advice, standing, fee_when_given}`). Read off the page, so never while
/// the page itself is drawing ([`refresh`]); `None` while no signing review
/// is open.
#[derive(Default)]
struct CardNow(Option<Value>);

impl Global for CardNow {}

/// Whether an opinion is held — the page asks for a [`refresh`] only then.
#[must_use]
pub fn held(cx: &App) -> bool {
    cx.try_global::<Advice>()
        .is_some_and(|advice| advice.0.is_some())
}

/// Stand the opinion held against the review on the sheet now
/// (`vela_review`'s answer). Deferred by the page: the review reads the page.
pub fn refresh(cx: &mut App) {
    let review = super::tools::current_review(cx);
    stand(&review, cx);
}

/// The card against `review`, through the core's `advice_card`; the windows
/// redraw only when it moved.
fn stand(review: &Value, cx: &mut App) {
    let card = cx
        .try_global::<Advice>()
        .and_then(|advice| advice.0.as_ref())
        .and_then(|advice| shared::advice_card(advice, review));
    if cx.try_global::<CardNow>().is_none_or(|now| now.0 != card) {
        cx.set_global(CardNow(card));
        cx.refresh_windows();
    }
}

/// `vela_advise`: the core judges the opinion against the review on the
/// sheet (`review`) and words the reply; an accepted one is kept for the
/// sheet's card.
pub fn advise(
    args: &Value,
    world: &vela_core::app::intent_rules::World,
    review: &Value,
    cx: &mut App,
) -> clientmcp_client::Reply {
    let (reply, accepted) = shared::advise(world, review, args);
    if let Some(advice) = accepted {
        cx.set_global(Advice(Some(advice)));
        stand(review, cx);
    }
    reply
}

/// The card's words (`componentsUi.signing.agentAdvice.*`).
pub struct AdviceCard {
    pub title: SharedString,
    pub verdict: SharedString,
    /// danger / caution / neutral — the verdict's ink.
    pub tone: crate::signing::Tone,
    pub summary: SharedString,
    pub points: Vec<SharedString>,
    /// "Sources: host, host".
    pub sources: Option<SharedString>,
    pub footer: SharedString,
    /// Why it is greyed, when it no longer stands for the sheet.
    pub stale: Option<SharedString>,
    pub fee_when_given: Option<SharedString>,
}

/// The card for the opinion held, as the core stood it against the review on
/// the sheet — `None` when there is none, or no sheet.
pub fn card(loc: &crate::loc::Loc, cx: &App) -> Option<AdviceCard> {
    words(loc, cx.try_global::<CardNow>()?.0.as_ref()?)
}

/// `advice_card`'s answer, worded.
fn words(loc: &crate::loc::Loc, card: &Value) -> Option<AdviceCard> {
    use crate::signing::Tone;
    let advice = card.get("advice")?;
    let t = |key: &str| loc.t(&format!("componentsUi.signing.agentAdvice.{key}"));
    let text = |key: &str| {
        advice
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned()
    };
    let (verdict, tone) = match text("verdict").as_str() {
        "looks_fine" => (t("looksFine"), Tone::Neutral),
        "caution" => (t("caution"), Tone::Caution),
        "danger" => (t("danger"), Tone::Danger),
        _ => (t("unsure"), Tone::Neutral),
    };
    let strings = |key: &str| -> Vec<String> {
        advice
            .get(key)
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    };
    let hosts = strings("source_hosts");
    let stale = match card.get("standing").and_then(Value::as_str) {
        Some("current") => None,
        Some("fee_changed") => Some(t("feeChanged")),
        // Anything else, or nothing said: never drawn as standing.
        _ => Some(t("stale")),
    };
    let fee_when_given = card.get("fee_when_given").and_then(|fee| {
        let amount = fee.get("amount").and_then(Value::as_str)?;
        let symbol = fee
            .get("symbol")
            .and_then(Value::as_str)
            .unwrap_or_default();
        Some(loc.t_text(
            "componentsUi.signing.agentAdvice.feeWhenGiven",
            "fee",
            format!("{amount} {symbol}").trim(),
        ))
    });
    Some(AdviceCard {
        title: t("title"),
        verdict,
        tone,
        summary: text("summary").into(),
        points: strings("points")
            .into_iter()
            .map(SharedString::from)
            .collect(),
        sources: (!hosts.is_empty())
            .then(|| format!("{}: {}", t("sources"), hosts.join(", ")).into()),
        footer: t("footer"),
        stale,
        fee_when_given,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const MINE: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn allowed() -> vela_core::app::intent_rules::World {
        serde_json::from_value(json!({
            "session": { "has_wallet": true, "address": MINE, "accounts": [
                { "index": 0, "account": { "address": MINE, "name": "Main" } }
            ] },
            "agent_wallets": [MINE],
        }))
        .unwrap()
    }

    fn sheet(digest: &str) -> Value {
        json!({ "note": "…", "signing": { "review_digest": digest, "fee": null } })
    }

    fn loc() -> crate::loc::Loc {
        crate::loc::Loc::for_tag("en")
    }

    /// The digest is the review's `signing.review_digest` alone: one carried
    /// anywhere else in the review is no signing review.
    #[test]
    fn the_digest_is_read_only_from_the_signing_member() {
        let opinion = json!({ "review_digest": "d1", "verdict": "caution", "summary": "New spender." });
        let (_, kept) = shared::advise(&allowed(), &sheet("d1"), &opinion);
        let kept = kept.expect("accepted");
        let elsewhere = json!({ "transfer": { "review_digest": "d1" } });
        assert!(shared::advice_card(&kept, &elsewhere).is_none());
        assert!(shared::advice_card(&kept, &sheet("d1")).is_some());
    }

    /// The card's words follow the core's standing: current is drawn as is,
    /// another version or fee greyed with its reason.
    #[test]
    fn the_card_is_greyed_by_the_cores_standing() {
        let opinion = json!({
            "review_digest": "d1", "verdict": "danger", "summary": "Drains the account.",
            "points": ["Unlimited approval."], "sources": ["https://example.org/a"],
        });
        let (_, kept) = shared::advise(&allowed(), &sheet("d1"), &opinion);
        let kept = kept.expect("accepted");
        let loc = loc();

        let current = shared::advice_card(&kept, &sheet("d1")).unwrap();
        let card = words(&loc, &current).unwrap();
        assert!(card.stale.is_none());
        assert_eq!(card.tone, crate::signing::Tone::Danger);
        assert_eq!(card.points.len(), 1);
        assert!(card.sources.as_deref().unwrap_or_default().contains("example.org"));

        let earlier = shared::advice_card(&kept, &sheet("d2")).unwrap();
        let card = words(&loc, &earlier).unwrap();
        assert_eq!(
            card.stale,
            Some(loc.t("componentsUi.signing.agentAdvice.stale"))
        );
    }

    #[test]
    fn the_tools_are_the_cores() {
        let names: Vec<String> = shared::advisor_definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect();
        assert_eq!(names, ["vela_watch_reviews", "vela_advise"]);
    }
}
