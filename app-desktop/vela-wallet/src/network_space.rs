//! The mainnet space and the testnet space (spec 105) — DESKTOP.
//!
//! One device-wide switch: which networks the whole wallet shows. The mainnet
//! space holds networks whose assets have real value; the testnet space,
//! testnets and developer networks with nothing of real value. Every list a
//! person sees — the balances read and the total, assets, activity, receive,
//! send's networks, the chains a dApp can use — is [`space_networks`] or goes
//! through [`retain`], never the whole list. What must know every chain (the
//! RPC pool, a lookup by id, a name service) reads [`all_networks`].
//!
//! Which space a stored value names, and which space a network belongs to,
//! are the core's to say (`vela_core::app::network_space`); this file only
//! reads the store and tells the machines when the answer changed.
//!
//! ## Read per frame, so read once per write
//!
//! The header label, the bar and the home's rows ask on every frame, and the
//! store is one JSON document read whole. The answer is kept beside the
//! store's write count ([`storage::generation`]) and the document's path, and
//! read again only when either moved — a switch, a network added, moved or
//! removed are all writes.

use std::path::PathBuf;
use std::sync::Mutex;

use gpui::App;
use serde_json::Value;
use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};
use vela_core::app::balance_dashboard::{BalanceDashboard, BalanceView};
use vela_core::app::network_admin::{BUILTIN_CHAINS, NetCustomNetwork};
pub use vela_core::app::network_space::Elsewhere;
use vela_core::app::network_space::{KEY, NetworkSpace, chain_is_testnet};
use vela_core::app::shown_networks::{self, NetShownNetworks};

use crate::executor::storage;
use crate::resident;

/// What the store said, and when.
struct Snapshot {
    generation: u64,
    path: Option<PathBuf>,
    space: NetworkSpace,
    custom: Vec<NetCustomNetwork>,
    /// Which Vela networks shown on the person's word they show; `None`
    /// (never settled on this shell yet) shows every one.
    shown: Option<NetShownNetworks>,
}

static SNAPSHOT: Mutex<Option<Snapshot>> = Mutex::new(None);

/// `f` over the store's current answer, read again only after a write.
fn with_snapshot<R>(f: impl FnOnce(&Snapshot) -> R) -> R {
    // The count before the read: a write landing during it moves the count
    // past the one this answer is filed under, and the next ask reads again.
    let generation = storage::generation();
    let path = storage::path().ok();
    let mut guard = SNAPSHOT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let fresh = guard
        .as_ref()
        .is_some_and(|snapshot| snapshot.generation == generation && snapshot.path == path);
    if !fresh {
        let stored = storage::read_value(KEY).ok().flatten();
        *guard = Some(Snapshot {
            generation,
            path,
            space: NetworkSpace::parse(stored.as_ref().and_then(Value::as_str)),
            custom: crate::executor::network_admin::read_store_custom_networks(),
            shown: storage::read_value(shown_networks::KEY)
                .ok()
                .flatten()
                .and_then(|value| serde_json::from_value(value).ok()),
        });
    }
    match guard.as_ref() {
        Some(snapshot) => f(snapshot),
        None => unreachable!("filled above"),
    }
}

/// The space in front. A device starts in the mainnet space.
#[must_use]
pub fn current() -> NetworkSpace {
    with_snapshot(|snapshot| snapshot.space)
}

/// The testnet space is in front: nothing on screen has real value.
#[must_use]
pub fn testnet() -> bool {
    current() == NetworkSpace::Testnet
}

/// Whether `chain_id` belongs to the space in front — the core's word on
/// which space a network is (`chain_is_testnet`): the built-in table's, else
/// the person's for one they added, else a testnet.
#[must_use]
pub fn in_space(chain_id: u32) -> bool {
    with_snapshot(|snapshot| {
        chain_is_testnet(chain_id, &snapshot.custom) == (snapshot.space == NetworkSpace::Testnet)
            && shown_networks::is_shown(chain_id, snapshot.shown.as_ref())
    })
}

/// Whether `chain_id` belongs to the space NOT in front — the same core
/// word (`chain_is_testnet`). A network the person took off their lists is
/// in neither: it is not somewhere a switch would show.
#[must_use]
pub fn in_other_space(chain_id: u32) -> bool {
    with_snapshot(|snapshot| {
        chain_is_testnet(chain_id, &snapshot.custom) != (snapshot.space == NetworkSpace::Testnet)
            && shown_networks::is_shown(chain_id, snapshot.shown.as_ref())
    })
}

/// r10-4, r13-1: where an account used on `chain_ids` lives, when it is not
/// in the space in front — the core's rule
/// ([`network_space::lives_elsewhere`](vela_core::app::network_space::lives_elsewhere),
/// the shared vector `vectors/multisig/other-space.json`), over the person's
/// lists of the space in front and of the other one (a network they took
/// off is in neither). `None`: nothing to say.
#[must_use]
pub fn lives_elsewhere(chain_ids: &[u32]) -> Option<Elsewhere> {
    if chain_ids.is_empty() {
        return None;
    }
    let all: Vec<u32> = all_networks().into_iter().map(|(id, _)| id).collect();
    let here: Vec<u32> = all.iter().copied().filter(|id| in_space(*id)).collect();
    let other: Vec<u32> = all
        .iter()
        .copied()
        .filter(|id| in_other_space(*id))
        .collect();
    vela_core::app::network_space::lives_elsewhere(chain_ids, &here, &other, current())
}

/// `ids` without the other space's chains, nor a Vela network out of the
/// person's lists (`shown_networks`), in their order.
#[must_use]
pub fn retain(ids: Vec<u32>) -> Vec<u32> {
    with_snapshot(|snapshot| {
        let testnet = snapshot.space == NetworkSpace::Testnet;
        ids.into_iter()
            .filter(|id| chain_is_testnet(*id, &snapshot.custom) == testnet)
            .filter(|id| shown_networks::is_shown(*id, snapshot.shown.as_ref()))
            .collect()
    })
}

/// Every network the wallet has, both spaces, with its own coin: the
/// built-ins, then whatever the person added. A network added that is now
/// built in (Arc Testnet, say) is the built-in one: listed, and read, once.
#[must_use]
pub fn all_networks() -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = BUILTIN_CHAINS
        .iter()
        .map(|chain| (chain.chain_id, chain.native_symbol.to_owned()))
        .collect();
    with_snapshot(|snapshot| {
        for network in &snapshot.custom {
            if !out.iter().any(|(id, _)| *id == network.chain_id) {
                out.push((network.chain_id, network.native_symbol.clone()));
            }
        }
    });
    out
}

/// The networks of the space in front, with their coins — what the balances
/// read and the total counts, and what receive, send and the dApps offer.
#[must_use]
pub fn space_networks() -> Vec<(u32, String)> {
    let all = all_networks();
    let ids = retain(all.iter().map(|(id, _)| *id).collect());
    all.into_iter().filter(|(id, _)| ids.contains(id)).collect()
}

/// The space's chain ids, in [`space_networks`]' order.
#[must_use]
pub fn space_chain_ids() -> Vec<u32> {
    space_networks().into_iter().map(|(id, _)| id).collect()
}

/// The person's switch, or the agent's they allowed (`set_network_space`,
/// [`crate::agent`]'s perform): stored for the device, and every machine
/// that lists networks told. The page's own lists (the browser's chains) are
/// the page's to follow (`WalletPage::network_space_switched`).
pub fn switch(space: NetworkSpace, cx: &mut App) {
    // Best effort, as every preference: the switch holds for this run even
    // where the store would not take it — the snapshot is read again only
    // after a write, so it is set here too.
    let _ = storage::write_value(KEY, Value::String(space.as_str().to_owned()));
    {
        let mut guard = SNAPSHOT
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(snapshot) = guard.as_mut() {
            snapshot.space = space;
        }
    }
    // A switch of space starts the account over in the core (home-7): the
    // other space's failures, holdings and cached total are not said here,
    // the cache is read again for this space and the chains forced — then
    // the rows this space last showed (the web's `space_changed` and
    // `#restoreRows`).
    if crate::session::view(cx).address.is_empty() {
        return;
    }
    tell_feed(cx);
    crate::executor::balance_dashboard::space_switched(cx);
}

/// The space's network set changed — a switch, a network added, moved to
/// the other space or removed: the feed draws only the space's records, and
/// the balances are read again, forced, so the total counts only it.
pub fn networks_changed(cx: &mut App) {
    // Signed out, nobody's balances or records are booted: nothing to tell,
    // and asking them would boot them for nobody.
    if crate::session::view(cx).address.is_empty() {
        return;
    }
    tell_feed(cx);
    crate::executor::balance_dashboard::refresh(cx);
}

/// The feed's `SpaceChainsChanged`: the space's chains, as they stand.
pub fn feed_event() -> FeedEvent {
    FeedEvent::SpaceChainsChanged {
        chain_ids: Some(space_chain_ids()),
    }
}

fn tell_feed(cx: &mut App) {
    resident::resident::<ActivityFeed>(cx).update(cx, |feed, cx| {
        feed.dispatch(feed_event(), cx);
    });
}

/// The balance view as the page draws it: only the space in front's
/// holdings. Until a round read after a switch has settled, the other
/// space's holdings — and a total that counted them — are not shown (the
/// web's `inSpaceView`).
#[must_use]
pub fn in_space_view(mut view: BalanceView) -> BalanceView {
    let before = view.tokens.len();
    view.tokens.retain(|token| in_space(token.chain_id));
    if view.tokens.len() != before {
        view.display_total_usd = None;
    }
    view
}

/// The live balance view, through [`in_space_view`].
pub fn balance_view(cx: &mut App) -> BalanceView {
    in_space_view(resident::resident::<BalanceDashboard>(cx).read(cx).view())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage::tests::with_temp_state;
    use serde_json::json;

    fn custom(chain_id: u32, is_testnet: Option<bool>) -> Value {
        let mut record = json!({
            "id": format!("custom-{chain_id}"),
            "displayName": "Mine",
            "chainId": chain_id,
            "nativeSymbol": "MINE",
        });
        if let Some(testnet) = is_testnet {
            record["isTestnet"] = json!(testnet);
        }
        record
    }

    /// A device starts in the mainnet space; what the store says is the
    /// core's to read, and anything it cannot is the mainnet space.
    #[test]
    fn a_device_starts_in_the_mainnet_space() {
        with_temp_state("space-default", || {
            assert_eq!(current(), NetworkSpace::Mainnet);
            assert!(storage::write_value(KEY, json!("testnet")).is_ok());
            assert_eq!(current(), NetworkSpace::Testnet);
            assert!(storage::write_value(KEY, json!("moon")).is_ok());
            assert_eq!(current(), NetworkSpace::Mainnet);
        });
    }

    /// Each space lists only its own networks: the built-in table's word,
    /// the person's for one they added, and an older record — nobody's word —
    /// is a testnet. Both together are every network, once.
    #[test]
    fn each_space_lists_only_its_own_networks() {
        with_temp_state("space-lists", || {
            let added = storage::write_value(
                storage::KEY_CUSTOM_NETWORKS,
                json!([
                    custom(4242, Some(false)),
                    custom(31337, None),
                    custom(7777, Some(true))
                ]),
            );
            assert!(added.is_ok());
            let mainnets = space_chain_ids();
            assert_eq!(
                mainnets.len(),
                BUILTIN_CHAINS.iter().filter(|c| !c.testnet).count() + 1
            );
            assert!(mainnets.contains(&1) && mainnets.contains(&4242));
            assert!(!mainnets.contains(&11_155_111) && !mainnets.contains(&31337));

            assert!(storage::write_value(KEY, json!("testnet")).is_ok());
            let testnets = space_chain_ids();
            assert_eq!(
                testnets.len(),
                BUILTIN_CHAINS.iter().filter(|c| c.testnet).count() + 2
            );
            assert!(testnets.contains(&11_155_111) && testnets.contains(&31337));
            assert!(testnets.contains(&7777) && !testnets.contains(&1));
            assert_eq!(mainnets.len() + testnets.len(), all_networks().len());
            assert!(in_space(7777) && !in_space(4242));
        });
    }

    /// r10-4: a multisig used only on Arc Testnet, shown from the mainnet
    /// space, lives in the testnet space — said; from the testnet space, or
    /// used on a network of each, there is nothing to say.
    #[test]
    fn an_account_of_the_other_space_says_where_it_lives() {
        with_temp_state("space-lives-elsewhere", || {
            // r13-1: the core's answer, keys and all — no copy of the rule here.
            let arc = lives_elsewhere(&[5_042_002]).expect("lives in the testnet space");
            assert_eq!(arc.chain_ids, [5_042_002]);
            assert_eq!(arc.space, NetworkSpace::Testnet);
            assert_eq!(arc.line_key, "home.multisig.add.usedOn");
            assert_eq!(arc.heading_key, "networkSpace.headingTestnet");
            assert_eq!(arc.switch_key, "networkSpace.toTestnet");
            assert!(lives_elsewhere(&[5_042_002, 100]).is_none());
            assert!(lives_elsewhere(&[100]).is_none());
            assert!(lives_elsewhere(&[]).is_none());
            // A network Vela does not have is on no list: never named.
            assert!(lives_elsewhere(&[777_777]).is_none());
            assert!(storage::write_value(KEY, json!("testnet")).is_ok());
            assert!(lives_elsewhere(&[5_042_002]).is_none());
            let base = lives_elsewhere(&[100, 8453]).expect("lives in the mainnet space");
            assert_eq!(base.chain_ids, [100, 8453]);
            assert_eq!(base.space, NetworkSpace::Mainnet);
            assert_eq!(base.switch_key, "networkSpace.toMainnet");
        });
    }

    /// Clearing the caches keeps the space: it is the person's setting.
    #[test]
    fn clearing_caches_keeps_the_space() {
        with_temp_state("space-clear-caches", || {
            assert!(storage::write_value(KEY, json!("testnet")).is_ok());
            assert!(crate::executor::device_storage::clear_caches().is_ok());
            assert_eq!(current(), NetworkSpace::Testnet);
        });
    }

    /// A holding of the other space is not drawn, and a total that counted
    /// it is not shown.
    #[test]
    fn the_other_spaces_holdings_are_not_drawn() {
        with_temp_state("space-view", || {
            let token = |chain_id: u32| vela_core::app::balance_dashboard::BalanceToken {
                chain_id,
                ..crate::wallet::fixtures::held_view().tokens[0].clone()
            };
            let view = BalanceView {
                tokens: vec![token(1), token(11_155_111)],
                display_total_usd: Some(10.0),
                ..crate::wallet::fixtures::held_view()
            };
            let mainnet = in_space_view(view.clone());
            assert_eq!(mainnet.tokens.len(), 1);
            assert_eq!(mainnet.display_total_usd, None);

            let only_mainnet = BalanceView {
                tokens: vec![token(1)],
                ..view
            };
            assert_eq!(
                in_space_view(only_mainnet).display_total_usd,
                Some(10.0),
                "nothing to take away: the total stands"
            );
        });
    }
}
