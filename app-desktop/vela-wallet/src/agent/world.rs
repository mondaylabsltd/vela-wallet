//! The wallet as the intents' judgements read it: the residents' views, the
//! preferences this shell keeps itself, the space in front, and the agent's
//! switch.
//!
//! The core's `World` takes "facts" — the views' own field names, only what
//! the judgements read — so the views go in as they are, through their JSON.
//! While nobody is signed in only the session is read: the other residents
//! belong to an account, and reading one boots it.

use gpui::App;
use serde_json::{Map, Value, json};
use vela_core::app::activity_feed::ActivityFeed;
use vela_core::app::balance_dashboard::BalanceDashboard;
use vela_core::app::contacts::Contacts;
use vela_core::app::display_currency::DisplayCurrency;
use vela_core::app::fee_tier_pref::FeeTierPref;
use vela_core::app::intent_rules::World;
use vela_core::app::intents::Operation;
use vela_core::app::network_admin::NetworkAdmin;

use crate::executor::{format_prefs, preferences};
use crate::resident;
use crate::session;

/// The preferences in the catalog's spellings. The desktop shares no usage
/// statistics, so `analytics` is always off.
pub fn preferences() -> Value {
    let prefs = preferences::current();
    let (number, date, time) = format_prefs::words();
    json!({
        "theme": prefs.theme,
        "language": prefs.language,
        "text_scale": prefs.text_scale,
        "number_format": number,
        "date_format": date,
        "time_format": time,
        "analytics": false,
    })
}

/// The views, as JSON, under the names `World` reads.
pub fn views(cx: &mut App, operations: &[Operation], agent_enabled: bool) -> Value {
    let session = session::view(cx);
    let signed_in = !session.address.is_empty();
    let mut world = Map::new();
    world.insert("session".into(), json!(session));
    if signed_in {
        let contacts = resident::resident::<Contacts>(cx).read(cx).view();
        let balance = resident::resident::<BalanceDashboard>(cx).read(cx).view();
        let feed = resident::resident::<ActivityFeed>(cx).read(cx).view();
        let networks = resident::resident::<NetworkAdmin>(cx).read(cx).view();
        let currency = resident::resident::<DisplayCurrency>(cx).read(cx).view();
        let fee_tier = resident::resident::<FeeTierPref>(cx).read(cx).view();
        world.insert("contacts".into(), json!(contacts));
        world.insert("balance".into(), json!(balance));
        world.insert("feed".into(), json!(feed));
        // The ledger has not read its stores yet: nothing to judge by.
        if networks.loaded {
            world.insert("networks".into(), json!(networks));
        }
        world.insert("currency".into(), json!(currency));
        world.insert("fee_tier".into(), json!(fee_tier));
    }
    // Spec 107: Explore — the tabs, the lists, a dApp's request.
    if signed_in && let Some(page) = super::page(cx).and_then(|page| page.upgrade()) {
        let explore = resident::resident::<vela_core::app::explore_sites::ExploreSites>(cx)
            .read(cx)
            .view();
        let history = resident::resident::<vela_core::app::browser_history::BrowserHistory>(cx)
            .read(cx)
            .view();
        let facts = page.read(cx).agent_explore_facts(&explore, &history, cx);
        world.insert("explore".into(), json!(facts));
    }
    world.insert("operations".into(), json!(operations));
    world.insert("preferences".into(), preferences());
    world.insert("agent_enabled".into(), Value::Bool(agent_enabled));
    // Spec 107 E8: the wallets the person lets the agent use.
    let wallets = cx
        .try_global::<super::Agent>()
        .map(|agent| agent.config.wallets.clone())
        .unwrap_or_default();
    world.insert("agent_wallets".into(), json!(wallets));
    // The space in front (spec 105): the agent reads only its networks, and
    // a transfer on the other space's is refused.
    world.insert(
        "network_space".into(),
        json!(crate::network_space::current()),
    );
    Value::Object(world)
}

/// The world the judgements read.
pub fn world(cx: &mut App, operations: &[Operation], agent_enabled: bool) -> World {
    serde_json::from_value(views(cx, operations, agent_enabled)).unwrap_or_default()
}
