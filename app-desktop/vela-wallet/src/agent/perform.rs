//! Doing an intent (spec 103): the calls the desktop's own controls make — the
//! same resident, the same event, the same page method — so a person's click
//! and an agent's request reach the rule that owns them by one road. The
//! web's `perform.ts`, `networks.ts` and `tokens.ts`, action for action.
//!
//! What only the open page can do (open a screen, a preference the page
//! redraws by, erasing, clearing a store) is its own method, reached through
//! [`super::on_page`]. Seeing the change land is the runner's
//! (`intent_rules::verdict`).

use std::time::Duration;

use gpui::{App, AsyncApp};
use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};
use vela_core::app::balance_dashboard::{BalanceDashboard, Event as BalanceEvent};
use vela_core::app::contacts::{
    ContactGroupInput, ContactSaveInput, Contacts, Event as ContactEvent,
};
use vela_core::app::display_currency::{DisplayCurrency, Event as CurrencyEvent};
use vela_core::app::fee_policy::FeeTier;
use vela_core::app::fee_tier_pref::{Event as FeeTierPrefEvent, FeeTierPref};
use vela_core::app::intent_rules::World;
use vela_core::app::intents::{Action, Actor, Preference, Reason, ReasonCode};
use vela_core::app::manage_tokens::{Event as MtokEvent, ManageTokens, MtokNetwork};
use vela_core::app::network_admin::{
    Event as NetEvent, NetEndpointField, NetOverrideField, NetProviderId, NetView, NetWizardPhase,
    NetworkAdmin,
};

use super::intents::Performed;
use crate::resident;
use crate::session;

fn reason(code: ReasonCode, detail: &str) -> Performed {
    Performed::Refused(Reason {
        code,
        detail: detail.to_owned(),
    })
}

/// A core value by its own name, as the web's refusals read it: an enum's
/// word, or a tagged one's `type`.
fn named(value: &impl serde::Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(word)) => word,
        Ok(other) => other
            .get("type")
            .and_then(serde_json::Value::as_str)
            .map_or_else(|| other.to_string(), str::to_owned),
        Err(_) => String::new(),
    }
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The session indexes its wallets its own way: read from it, by address.
fn index_of(world: &World, address: &str) -> Option<usize> {
    world
        .session
        .as_ref()?
        .accounts
        .iter()
        .find(|row| same(&row.account.address, address))
        .map(|row| row.index)
}

async fn sleep(cx: &mut AsyncApp, ms: u64) {
    cx.background_executor()
        .timer(Duration::from_millis(ms))
        .await;
}

/// Read `read` until it answers, or `ms` pass.
async fn until<T>(
    cx: &mut AsyncApp,
    ms: u64,
    mut read: impl FnMut(&mut App) -> Option<T>,
) -> Option<T> {
    let mut waited = 0;
    loop {
        if let Some(value) = cx.update(&mut read) {
            return Some(value);
        }
        if waited >= ms {
            return None;
        }
        sleep(cx, 50).await;
        waited += 50;
    }
}

/// What this shell cannot do at all, whoever asks: refused before anyone is
/// asked to approve it. The desktop shares no usage statistics.
pub fn unavailable(action: &Action) -> Option<Reason> {
    match action {
        Action::SetPreference {
            preference: Preference::Analytics(_),
        } => Some(Reason {
            code: ReasonCode::Refused,
            detail: "this desktop wallet shares no usage statistics: there is nothing to switch"
                .to_owned(),
        }),
        _ => None,
    }
}

pub async fn perform(
    action: &Action,
    world: &World,
    now_ms: f64,
    actor: Actor,
    cx: &mut AsyncApp,
) -> Performed {
    let page_gone = || reason(ReasonCode::NotReady, "the wallet is not open");
    match action {
        Action::Show { screen } => {
            match super::on_page(cx, |page, _, cx| page.agent_show(screen, cx)) {
                None => page_gone(),
                Some(Some(refusal)) => Performed::refused(refusal),
                Some(None) => Performed::Done,
            }
        }
        Action::SwitchAccount { address } | Action::RemoveWallet { address } => {
            let Some(index) = index_of(world, address) else {
                return Performed::refused("no wallet on this device has that address");
            };
            let switch = matches!(action, Action::SwitchAccount { .. });
            cx.update(|cx| {
                if switch {
                    session::switch_account(index, cx);
                } else {
                    session::remove_account(index, cx);
                }
            });
            Performed::Done
        }
        Action::SignOut => {
            cx.update(session::sign_out);
            Performed::Done
        }
        Action::SetPreference { preference } => set_preference(preference, cx),
        Action::SaveContact { address, name } => {
            contacts(
                cx,
                ContactEvent::Save {
                    input: ContactSaveInput {
                        address: address.trim().to_owned(),
                        name: Some(name.trim().to_owned()),
                        note: None,
                        favorite: None,
                        kind: None,
                        resolved_name: None,
                        resolved_source: None,
                    },
                    now_ms,
                },
            );
            Performed::Done
        }
        Action::DeleteContact { address } => {
            contacts(
                cx,
                ContactEvent::Delete {
                    address: address.clone(),
                    now_ms,
                },
            );
            Performed::Done
        }
        Action::SaveGroup { id, name } => {
            contacts(
                cx,
                ContactEvent::GroupSave {
                    input: ContactGroupInput {
                        id: id.clone(),
                        name: name.trim().to_owned(),
                        color: None,
                        members: None,
                    },
                },
            );
            Performed::Done
        }
        Action::DeleteGroup { id } => {
            contacts(cx, ContactEvent::GroupDelete { id: id.clone() });
            Performed::Done
        }
        Action::SetGroupMembers { id, members } => {
            contacts(
                cx,
                ContactEvent::SetGroupMembers {
                    id: id.clone(),
                    members: members.clone(),
                },
            );
            Performed::Done
        }
        Action::SetContactGroups { address, group_ids } => {
            contacts(
                cx,
                ContactEvent::SetContactGroups {
                    address: address.clone(),
                    group_ids: group_ids.clone(),
                },
            );
            Performed::Done
        }
        Action::SetContactFavorite { address, .. } => {
            // The star is a toggle: the check already said it is not so yet.
            contacts(
                cx,
                ContactEvent::ToggleFavorite {
                    address: address.clone(),
                    now_ms,
                },
            );
            Performed::Done
        }
        Action::EraseDevice => match super::on_page(cx, |page, _, cx| page.agent_erase(cx)) {
            None => page_gone(),
            Some(Some(refusal)) => Performed::refused(refusal),
            Some(None) => Performed::Done,
        },
        Action::ClearStorageItem { item } => {
            match super::on_page(cx, |page, _, cx| page.agent_clear_storage(item, cx)) {
                None => page_gone(),
                Some(()) => Performed::Done,
            }
        }
        Action::ClearCaches => {
            match super::on_page(cx, |page, _, cx| page.agent_clear_caches(cx)) {
                None => page_gone(),
                Some(()) => Performed::Done,
            }
        }
        Action::ConfigureAgent {
            enabled,
            bridge_url,
            client_token,
        } => {
            let change = super::settings::AgentChange {
                enabled: *enabled,
                url: bridge_url.clone(),
                token: client_token.clone(),
            };
            cx.update(|cx| super::configure(&change, cx));
            Performed::Done
        }
        Action::PrepareTransfer {
            chain_id,
            token,
            to,
            amount,
        } => super::transfers::prepare(*chain_id, token.as_deref(), to, amount, actor, cx).await,
        Action::PrepareBatchTransfer {
            chain_id,
            token,
            recipients,
        } => {
            super::transfers::prepare_batch(*chain_id, token.as_deref(), recipients, actor, cx)
                .await
        }
        Action::SubmitTransfer { operation } => super::transfers::submit(operation, actor, cx),
        Action::CancelTransfer { operation } => super::transfers::cancel(operation, cx),
        Action::AddToken { chain_id, address } => add_token(*chain_id, address, cx).await,
        Action::DeleteActivity { id } => {
            // History's "Delete record": the feed tombstones it and drops the row.
            cx.update(|cx| {
                resident::resident::<ActivityFeed>(cx).update(cx, |feed, cx| {
                    feed.dispatch(FeedEvent::DeleteRequested { id: id.clone() }, cx);
                });
            });
            Performed::Done
        }
        Action::SetBalancePrivacy { hidden } => {
            // The figure is a toggle: pressed only when it is not already so.
            cx.update(|cx| {
                let now = resident::resident::<BalanceDashboard>(cx)
                    .read(cx)
                    .view()
                    .hidden;
                if now != *hidden {
                    crate::executor::balance_dashboard::dispatch(BalanceEvent::PrivacyToggled, cx);
                }
            });
            Performed::Done
        }
        Action::RefreshBalances => match super::on_page(cx, |page, _, cx| page.agent_refresh(cx)) {
            None => page_gone(),
            Some(()) => Performed::Done,
        },
        Action::AddNetwork { chain_id, rpc_url } => {
            add_network(*chain_id, rpc_url.as_deref(), cx).await
        }
        Action::SetNetworkEndpoint {
            chain_id,
            field,
            url,
        } => set_network_endpoint(*chain_id, *field, url, cx).await,
        Action::RemoveNetwork { chain_id } => {
            cx.update(|cx| {
                let view = networks(cx);
                let id = view
                    .networks
                    .iter()
                    .find(|row| row.chain_id == *chain_id && row.is_custom)
                    .map_or_else(|| format!("custom-{chain_id}"), |row| row.id.clone());
                net(cx, NetEvent::DeleteConfirmed { id });
                // As Settings' own delete does: the balances stop asking that chain.
                crate::executor::balance_dashboard::refresh(cx);
            });
            Performed::Done
        }
        Action::SetProviderKey { provider, key } => {
            cx.update(|cx| set_provider_key(*provider, key, cx));
            Performed::Done
        }
        Action::SetServiceEndpoint { field, url } => {
            cx.update(|cx| set_service_endpoint(*field, url, cx));
            Performed::Done
        }
        Action::ResetServiceEndpoints => {
            cx.update(|cx| net(cx, NetEvent::ResetEndpointsToDefaults));
            Performed::Done
        }
    }
}

fn contacts(cx: &mut AsyncApp, event: ContactEvent) {
    cx.update(|cx| {
        resident::resident::<Contacts>(cx).update(cx, |book, cx| book.dispatch(event, cx));
    });
}

fn set_preference(preference: &Preference, cx: &mut AsyncApp) -> Performed {
    match preference {
        // The page redraws by these: it sets them as its own controls do.
        Preference::Theme(_)
        | Preference::Language(_)
        | Preference::TextScale(_)
        | Preference::NumberFormat(_)
        | Preference::DateFormat(_)
        | Preference::TimeFormat(_) => {
            match super::on_page(cx, |page, window, cx| {
                page.agent_preference(preference, window, cx)
            }) {
                Some(()) => Performed::Done,
                None => reason(ReasonCode::NotReady, "the wallet is not open"),
            }
        }
        Preference::Currency(code) => {
            cx.update(|cx| {
                resident::resident::<DisplayCurrency>(cx).update(cx, |currency, cx| {
                    currency.dispatch(CurrencyEvent::UserChose { code: code.clone() }, cx);
                });
            });
            Performed::Done
        }
        Preference::FeeSpeed(tier) => {
            if *tier != FeeTier::Rapid {
                let tier = *tier;
                cx.update(|cx| {
                    resident::resident::<FeeTierPref>(cx).update(cx, |pref, cx| {
                        pref.dispatch(FeeTierPrefEvent::UserChose { tier }, cx);
                    });
                });
            }
            Performed::Done
        }
        // Refused at the check (`unavailable`), before anyone is asked.
        Preference::Analytics(_) => Performed::refused(
            "this desktop wallet shares no usage statistics: there is nothing to switch",
        ),
    }
}

// ---------------------------------------------------------------------------
// Networks and services: the events Settings' network pages send to the
// network ledger, in the order its controls send them — open the card, edit
// the field, leave it. Whether a change held is read back from its view.
// ---------------------------------------------------------------------------

fn networks(cx: &mut App) -> NetView {
    resident::resident::<NetworkAdmin>(cx).read(cx).view()
}

fn net(cx: &mut App, event: NetEvent) {
    resident::resident::<NetworkAdmin>(cx).update(cx, |ledger, cx| ledger.dispatch(event, cx));
}

/// How long the add wizard may take to look a chain up and check it.
const CHECK_MS: u64 = 45_000;
/// How long the RPC save gate may take to hear the endpoint's chain id.
const GATE_MS: u64 = 20_000;

/// Settings → Add network: look the chain up, try `rpc_url` first if given,
/// check the wallet can serve it, and add it. A wizard already checked for
/// this chain (the person's own, at its "Add network") is not checked again.
async fn add_network(chain_id: u32, rpc_url: Option<&str>, cx: &mut AsyncApp) -> Performed {
    cx.update(|cx| {
        let wizard = networks(cx).wizard;
        let checked = wizard.phase == NetWizardPhase::Checked
            && wizard.can_add
            && wizard
                .chain_info
                .as_ref()
                .is_some_and(|info| info.chain_id == chain_id)
            && rpc_url.is_none_or(|url| wizard.custom_rpc.trim() == url.trim());
        if !checked {
            net(cx, NetEvent::WizardReset);
            net(
                cx,
                NetEvent::ChainSelected {
                    chain_id,
                    keep_custom_rpc: false,
                },
            );
            if let Some(url) = rpc_url {
                net(
                    cx,
                    NetEvent::CustomRpcEdited {
                        value: url.trim().to_owned(),
                    },
                );
                net(
                    cx,
                    NetEvent::ChainSelected {
                        chain_id,
                        keep_custom_rpc: true,
                    },
                );
            }
        }
    });
    let settled = until(cx, CHECK_MS, |cx| {
        let wizard = networks(cx).wizard;
        if let Some(error) = wizard
            .error
            .as_ref()
            .filter(|_| wizard.phase == NetWizardPhase::Error)
        {
            return Some(Err(format!("add network: {}", named(error))));
        }
        if wizard.phase == NetWizardPhase::Checked {
            if wizard.can_add {
                return Some(Ok(()));
            }
            let why = wizard
                .compat
                .as_ref()
                .and_then(|compat| {
                    compat
                        .blocker
                        .as_ref()
                        .map(named)
                        .or_else(|| compat.rpc_failure.as_ref().map(named))
                })
                .unwrap_or_else(|| "not compatible".to_owned());
            return Some(Err(format!("add network: {why}")));
        }
        None
    })
    .await;
    match settled {
        None => reason(
            ReasonCode::NotObserved,
            "the network could not be checked in time",
        ),
        Some(Err(refusal)) => Performed::refused(refusal),
        Some(Ok(())) => {
            let now_iso = crate::executor::now_iso();
            cx.update(|cx| net(cx, NetEvent::AddConfirmed { now_iso }));
            Performed::Done
        }
    }
}

/// One network's card: open it, edit the field, leave it (which saves). A
/// field already showing `url` is only left. An RPC goes through the core's
/// save gate: one answering for another chain is not written, and the field
/// is put back to what is in effect.
async fn set_network_endpoint(
    chain_id: u32,
    field: NetOverrideField,
    url: &str,
    cx: &mut AsyncApp,
) -> Performed {
    let shown = move |view: &NetView| {
        view.networks
            .iter()
            .find(|row| row.chain_id == chain_id)
            .map(|row| match field {
                NetOverrideField::Rpc => row.rpc_url.clone(),
                NetOverrideField::Explorer => row.explorer_url.clone(),
            })
    };
    let wanted = url.trim().to_owned();
    let before = cx.update(|cx| {
        let before = shown(&networks(cx));
        if before.as_deref() != Some(wanted.as_str()) {
            net(cx, NetEvent::OverrideExpanded { chain_id });
            net(
                cx,
                NetEvent::OverrideFieldEdited {
                    chain_id,
                    field,
                    value: wanted.clone(),
                },
            );
        }
        net(cx, NetEvent::OverrideBlurred { chain_id });
        before
    });
    if field != NetOverrideField::Rpc {
        return Performed::Done;
    }
    let gate = until(cx, GATE_MS, |cx| {
        let view = networks(cx);
        let row = view.networks.iter().find(|row| row.chain_id == chain_id)?;
        if row.rpc_chain_mismatch.is_some() {
            return Some(false);
        }
        (!row.rpc_save_deferred).then_some(true)
    })
    .await;
    if gate == Some(false) {
        if let Some(before) = before {
            cx.update(|cx| {
                net(
                    cx,
                    NetEvent::OverrideFieldEdited {
                        chain_id,
                        field,
                        value: before,
                    },
                );
            });
        }
        return Performed::refused(
            "that RPC answers for another chain; the network keeps the RPC it had",
        );
    }
    Performed::Done
}

/// RPC providers: open the page (it seeds the drafts), type the key, leave
/// it. A key already in its field is only left — opening the page again
/// would reseed every draft.
fn set_provider_key(provider: NetProviderId, key: &str, cx: &mut App) {
    let view = networks(cx);
    let typed = view
        .providers
        .iter()
        .find(|row| row.provider == provider)
        .is_some_and(|row| row.key == key.trim());
    if !typed {
        net(cx, NetEvent::ProvidersOpened);
        net(
            cx,
            NetEvent::ProviderKeyEdited {
                provider,
                value: key.trim().to_owned(),
            },
        );
    }
    net(cx, NetEvent::ProviderKeyBlurred { provider });
}

/// Service endpoints: open the page, type the URL, leave it — or only leave it.
fn set_service_endpoint(field: NetEndpointField, url: &str, cx: &mut App) {
    let view = networks(cx);
    let typed = view
        .endpoints
        .iter()
        .find(|row| row.field == field)
        .is_some_and(|row| row.value == url.trim());
    if !typed {
        net(cx, NetEvent::EndpointsOpened);
        net(
            cx,
            NetEvent::EndpointEdited {
                field,
                value: url.trim().to_owned(),
            },
        );
    }
    net(cx, NetEvent::EndpointBlurred { field });
}

// ---------------------------------------------------------------------------
// Tokens: the add-token sheet's own events — the address, the search, the
// save — for the one network named.
// ---------------------------------------------------------------------------

/// How long the search and the save may each take.
const TOKEN_STEP_MS: u64 = 30_000;

async fn add_token(chain_id: u32, address: &str, cx: &mut AsyncApp) -> Performed {
    let address = address.trim().to_owned();
    let Some(name) = cx.update(|_| {
        crate::flows::live::receivable_chains()
            .into_iter()
            .find(|(id, _)| *id == chain_id)
            .map(|_| crate::executor::custom_tokens::network_name(chain_id))
    }) else {
        return reason(
            ReasonCode::NotFound,
            "the wallet has no network with that chain id",
        );
    };
    let tokens = |cx: &mut App| resident::resident::<ManageTokens>(cx);
    cx.update(|cx| {
        tokens(cx).update(cx, |sheet, cx| {
            sheet.dispatch(MtokEvent::AddressInput { s: address.clone() }, cx);
        });
    });
    let typed = until(cx, TOKEN_STEP_MS, |cx| {
        let view = tokens(cx).read(cx).view();
        (view.input_address == address).then_some(view.address_valid)
    })
    .await;
    if typed != Some(true) {
        return reason(ReasonCode::InvalidInput, "that is not a contract address");
    }
    cx.update(|cx| {
        tokens(cx).update(cx, |sheet, cx| {
            sheet.dispatch(
                MtokEvent::DetectRequested {
                    networks: vec![MtokNetwork { chain_id, name }],
                },
                cx,
            );
        });
    });
    let searched = until(cx, TOKEN_STEP_MS, |cx| {
        let view = tokens(cx).read(cx).view();
        (!view.detecting && (!view.found.is_empty() || view.not_found || view.native_alias))
            .then_some(view)
    })
    .await;
    let Some(searched) = searched else {
        return reason(
            ReasonCode::NotObserved,
            "the token could not be looked up in time",
        );
    };
    if searched.native_alias {
        return Performed::refused("that address is the network's own coin, already in the wallet");
    }
    let Some(card) = searched.found.iter().find(|card| card.chain_id == chain_id) else {
        return reason(
            ReasonCode::NotFound,
            "no token answers at that address on that network",
        );
    };
    if card.added {
        return Performed::Done;
    }
    cx.update(|cx| {
        tokens(cx).update(cx, |sheet, cx| {
            sheet.dispatch(MtokEvent::SaveRequested { chain_id }, cx)
        });
    });
    let saved = until(cx, TOKEN_STEP_MS, |cx| {
        let view = tokens(cx).read(cx).view();
        let stored = view
            .found
            .iter()
            .any(|card| card.chain_id == chain_id && card.added);
        (!view.saving && (view.save_error || stored)).then_some(view.save_error)
    })
    .await;
    match saved {
        None => reason(
            ReasonCode::NotObserved,
            "the token was not seen stored in time",
        ),
        Some(true) => Performed::refused("the token could not be stored"),
        Some(false) => Performed::Done,
    }
}
