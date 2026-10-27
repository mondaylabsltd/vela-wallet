//! Rules of the wallet's intents (spec 103), one test per rule.
//!
//! What every shell must agree on before it does anything an agent — or the
//! person — asks:
//!
//! - who may ask for what: an agent never touches its own connection, does
//!   nothing while switched off, and waits for the person on manage-level;
//! - that malformed input is refused with a code an agent can act on;
//! - that the catalog names every action once, under its wire name.

#![cfg(feature = "crux")]

use vela_core::app::fee_policy::FeeTier;
use vela_core::app::intent_rules::{
    approval_detail, examine, verdict, Baseline, ConnectedSiteFacts, DappRequestFacts, Examined,
    ExploreFacts, SiteFacts, TabFacts, Verdict, World,
};
use vela_core::app::intents::{
    authorize, catalog, check, check_json, shows, validate, Action, Actor, Approver, BrowseStep,
    Decision, Grants, Preference, ReasonCode, Recipient, Risk, Screen, MAX_RECIPIENTS,
};
use vela_core::app::network_admin::{NetEndpointField, NetOverrideField, NetProviderId};

const ALICE: &str = "0x00000000219ab540356cbb839cbe05303d7705fa";
const MINE: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

/// The agent on, with the test wallets allowed (spec 107 E8).
fn on() -> Grants {
    Grants {
        agent_enabled: true,
        wallets: vec![MINE.to_owned(), ALICE.to_owned()],
        wallet: MINE.to_owned(),
    }
}

fn one_of_each() -> Vec<Action> {
    vec![
        Action::Show {
            screen: Screen::Home,
        },
        Action::SwitchAccount {
            address: MINE.to_owned(),
        },
        Action::SetPreference {
            preference: Preference::Theme("dark".to_owned()),
        },
        Action::SaveContact {
            address: ALICE.to_owned(),
            name: "Alice".to_owned(),
        },
        Action::DeleteContact {
            address: ALICE.to_owned(),
        },
        Action::SaveGroup {
            id: None,
            name: "Work".to_owned(),
        },
        Action::DeleteGroup {
            id: "g1".to_owned(),
        },
        Action::SetGroupMembers {
            id: "g1".to_owned(),
            members: vec![ALICE.to_owned()],
        },
        Action::SetContactGroups {
            address: ALICE.to_owned(),
            group_ids: vec!["g1".to_owned()],
        },
        Action::RemoveWallet {
            address: MINE.to_owned(),
        },
        Action::SignOut,
        Action::EraseDevice,
        Action::ClearStorageItem {
            item: "transactions".to_owned(),
        },
        Action::ClearCaches,
        Action::ConfigureAgent {
            enabled: Some(true),
            bridge_url: None,
            client_token: None,
            wallets: None,
        },
        Action::PrepareTransfer {
            chain_id: 1,
            token: None,
            to: ALICE.to_owned(),
            amount: "0.5".to_owned(),
        },
        Action::SubmitTransfer {
            operation: "transfer-1".to_owned(),
            review: None,
        },
        Action::CancelTransfer {
            operation: "transfer-1".to_owned(),
        },
        Action::PrepareBatchTransfer {
            chain_id: 1,
            token: Some(MINE.to_owned()),
            recipients: vec![
                Recipient {
                    to: ALICE.to_owned(),
                    amount: "1".to_owned(),
                },
                Recipient {
                    to: MINE.to_owned(),
                    amount: "2.5".to_owned(),
                },
            ],
        },
        Action::AddToken {
            chain_id: 1,
            address: MINE.to_owned(),
        },
        Action::DeleteActivity {
            id: "1-0xabc-0".to_owned(),
        },
        Action::SetBalancePrivacy { hidden: true },
        Action::RefreshBalances,
        Action::SetNetworkSpace {
            space: vela_core::app::network_space::NetworkSpace::Testnet,
        },
        Action::SetContactFavorite {
            address: ALICE.to_owned(),
            favorite: true,
        },
        Action::AddNetwork {
            chain_id: 8453,
            rpc_url: Some("https://mainnet.base.org".to_owned()),
        },
        Action::SetNetworkEndpoint {
            chain_id: 1,
            field: NetOverrideField::Rpc,
            url: "https://rpc.example".to_owned(),
        },
        Action::RemoveNetwork { chain_id: 8453 },
        Action::SetProviderKey {
            provider: NetProviderId::Alchemy,
            key: "abc123".to_owned(),
        },
        Action::SetServiceEndpoint {
            field: NetEndpointField::FiatRates,
            url: "https://rates.example".to_owned(),
        },
        Action::ResetServiceEndpoints,
        Action::OpenSite {
            url: "https://app.uniswap.org".to_owned(),
            new_tab: false,
        },
        Action::Browse {
            to: BrowseStep::Back,
        },
        Action::CloseTab {
            id: "t1".to_owned(),
        },
        Action::CloseAllTabs,
        Action::SetSiteFavorite {
            url: "https://app.uniswap.org".to_owned(),
            favorite: true,
        },
        Action::ClearRecentSites,
        Action::ForgetSite {
            origin: "https://app.uniswap.org".to_owned(),
        },
        Action::DisconnectSite {
            origin: "https://app.uniswap.org".to_owned(),
        },
        Action::SetSiteNetwork {
            origin: "https://app.uniswap.org".to_owned(),
            chain_id: 8453,
        },
        Action::RejectDappRequest {
            id: "r1".to_owned(),
        },
        Action::AskPersonDappRequest {
            id: "r1".to_owned(),
        },
    ]
}

// ---------------------------------------------------------------------------
// Explore (spec 107)
// ---------------------------------------------------------------------------

const UNISWAP: &str = "https://app.uniswap.org";

fn explore() -> ExploreFacts {
    ExploreFacts {
        browser: true,
        tabs: vec![
            TabFacts {
                id: "t1".into(),
                url: format!("{UNISWAP}/swap"),
                title: "Uniswap".into(),
                current: true,
            },
            TabFacts {
                id: "t2".into(),
                url: String::new(),
                title: String::new(),
                current: false,
            },
        ],
        favorites: vec![],
        recent: vec![SiteFacts {
            origin: UNISWAP.into(),
            url: format!("{UNISWAP}/swap"),
            title: "Uniswap".into(),
        }],
        connected: vec![ConnectedSiteFacts {
            origin: UNISWAP.into(),
            chain_id: Some(1),
            address: MINE.into(),
        }],
        pending: Some(DappRequestFacts {
            id: "r1".into(),
            kind: "sign".into(),
            origin: UNISWAP.into(),
            shown: false,
        }),
        can_go_back: true,
        can_go_forward: false,
    }
}

fn world_with(explore: ExploreFacts) -> World {
    World {
        explore: Some(explore),
        ..World::default()
    }
}

#[test]
fn an_agent_opens_sites_like_a_typed_address() {
    let open = |url: &str| Action::OpenSite {
        url: url.to_owned(),
        new_tab: true,
    };
    assert_eq!(validate(&open(UNISWAP)), Ok(()));
    assert_eq!(
        validate(&open("http://192.168.1.20:3000")),
        Ok(()),
        "the person's own network"
    );
    assert_eq!(validate(&open("http://localhost:5173/x")), Ok(()));
    for refused in [
        "http://example.com",
        "javascript:alert(1)",
        "file:///etc/passwd",
        "uniswap.org",
        "",
    ] {
        assert!(validate(&open(refused)).is_err(), "{refused}");
    }
    // Browsing is ordinary use: no approval (E1). What a site may do is the person's (E3).
    assert_eq!(
        authorize(Actor::Agent, &open(UNISWAP), &on()),
        Decision::Allow
    );
    assert_eq!(
        authorize(
            Actor::Agent,
            &Action::RejectDappRequest { id: "r1".into() },
            &on()
        ),
        Decision::Allow
    );
    for manage in [
        Action::DisconnectSite {
            origin: UNISWAP.into(),
        },
        Action::SetSiteNetwork {
            origin: UNISWAP.into(),
            chain_id: 8453,
        },
    ] {
        assert_eq!(
            authorize(Actor::Agent, &manage, &on()),
            Decision::Approve {
                by: Approver::Wallet
            },
            "{manage:?}"
        );
    }
    assert!(
        validate(&Action::DisconnectSite {
            origin: format!("{UNISWAP}/swap")
        })
        .is_err(),
        "an origin, not a page"
    );
    assert_eq!(shows(&open(UNISWAP)), Some(Screen::Explore));
    assert_eq!(
        approval_detail(&Action::SetSiteNetwork {
            origin: UNISWAP.into(),
            chain_id: 8453
        })
        .as_deref(),
        Some("https://app.uniswap.org · chain 8453")
    );
}

#[test]
fn explore_actions_are_judged_against_what_explore_shows() {
    let world = world_with(explore());
    let go = |action: Action| examine(&action, &world);
    assert_eq!(
        go(Action::Browse {
            to: BrowseStep::Back
        }),
        Examined::Go
    );
    assert!(matches!(
        go(Action::Browse {
            to: BrowseStep::Forward
        }),
        Examined::Refused { .. }
    ));
    assert!(matches!(
        go(Action::CloseTab { id: "t9".into() }),
        Examined::Refused { .. }
    ));
    assert_eq!(
        go(Action::SetSiteNetwork {
            origin: UNISWAP.into(),
            chain_id: 1
        }),
        Examined::Already
    );
    assert_eq!(
        go(Action::SetSiteNetwork {
            origin: UNISWAP.into(),
            chain_id: 8453
        }),
        Examined::Go
    );
    assert!(matches!(
        go(Action::DisconnectSite {
            origin: "https://evil.example".into()
        }),
        Examined::Refused { .. }
    ));
    assert_eq!(
        go(Action::AskPersonDappRequest { id: "r1".into() }),
        Examined::Go
    );
    assert!(matches!(
        go(Action::RejectDappRequest { id: "r2".into() }),
        Examined::Refused { .. }
    ));
    assert!(
        matches!(
            examine(
                &Action::OpenSite {
                    url: UNISWAP.into(),
                    new_tab: false
                },
                &World::default()
            ),
            Examined::Refused { .. }
        ),
        "not ready without Explore's view"
    );

    // Held once Explore shows it.
    let mut baseline = Baseline::default();
    let mut after = explore();
    after.pending = None;
    after.connected.clear();
    after.tabs.retain(|tab| tab.id != "t2");
    let world_after = world_with(after);
    for action in [
        Action::RejectDappRequest { id: "r1".into() },
        Action::DisconnectSite {
            origin: UNISWAP.into(),
        },
        Action::CloseTab { id: "t2".into() },
        Action::OpenSite {
            url: format!("{UNISWAP}/pool"),
            new_tab: false,
        },
    ] {
        assert_eq!(
            verdict(&action, &world_after, &mut baseline),
            Verdict::Held,
            "{action:?}"
        );
        assert_eq!(
            verdict(&action, &world, &mut baseline),
            if matches!(action, Action::OpenSite { .. }) {
                Verdict::Held
            } else {
                Verdict::NotYet
            },
            "{action:?} before"
        );
    }
}

#[test]
fn the_web_wallet_only_opens_a_site_in_a_browser_tab() {
    let mut web = explore();
    web.browser = false;
    let world = world_with(web);
    assert_eq!(
        examine(
            &Action::OpenSite {
                url: UNISWAP.into(),
                new_tab: true
            },
            &world
        ),
        Examined::Go
    );
    for action in [
        Action::Browse {
            to: BrowseStep::Reload,
        },
        Action::CloseAllTabs,
        Action::Show {
            screen: Screen::Explore,
        },
    ] {
        assert!(
            matches!(examine(&action, &world), Examined::Refused { .. }),
            "{action:?}"
        );
    }
}

fn is_transfer(action: &Action) -> bool {
    matches!(
        action,
        Action::PrepareTransfer { .. }
            | Action::PrepareBatchTransfer { .. }
            | Action::SubmitTransfer { .. }
            | Action::CancelTransfer { .. }
    )
}

fn denied(decision: &Decision) -> Option<ReasonCode> {
    match decision {
        Decision::Deny { reason } => Some(reason.code),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Who may ask
// ---------------------------------------------------------------------------

/// The agent's own connection and grants are the person's alone: an agent
/// that re-pointed its bridge would hand the wallet to whoever runs the other
/// one. Refused whether or not the agent is switched on.
#[test]
fn an_agent_may_not_touch_its_own_connection() {
    for action in [Action::ConfigureAgent {
        enabled: None,
        bridge_url: Some("wss://elsewhere.example/connect".to_owned()),
        client_token: None,
        wallets: None,
    }] {
        for grants in [on(), Grants::default()] {
            let decision = authorize(Actor::Agent, &action, &grants);
            assert_eq!(
                denied(&decision),
                Some(ReasonCode::PersonOnly),
                "{action:?}"
            );
        }
    }
}

/// The person may ask for anything — their UI has already asked them what is
/// worth asking — and needs no second approval.
#[test]
fn the_person_may_ask_for_everything() {
    for action in one_of_each() {
        assert_eq!(
            authorize(Actor::Person, &action, &Grants::default()),
            Decision::Allow,
            "{action:?}"
        );
    }
}

#[test]
fn a_switched_off_agent_may_ask_for_nothing() {
    for action in one_of_each() {
        let decision = authorize(Actor::Agent, &action, &Grants::default());
        assert!(
            matches!(
                denied(&decision),
                Some(ReasonCode::AgentOff | ReasonCode::PersonOnly)
            ),
            "{action:?}: {decision:?}"
        );
    }
}

/// Operate-level actions go ahead; manage-level ones wait for the person —
/// in the wallet's own approval, or, for signing out, on the session's sheet
/// (which carries the unsent-data warning a generic approval would not).
#[test]
fn an_agents_manage_level_request_waits_for_the_person() {
    for action in one_of_each() {
        if action.person_only() || is_transfer(&action) {
            continue;
        }
        let decision = authorize(Actor::Agent, &action, &on());
        match action.risk() {
            Risk::Observe | Risk::Operate => assert_eq!(decision, Decision::Allow, "{action:?}"),
            Risk::Manage | Risk::Transact => {
                let by = if matches!(action, Action::SignOut) {
                    Approver::Owner
                } else {
                    Approver::Wallet
                };
                assert_eq!(decision, Decision::Approve { by }, "{action:?}");
            }
        }
    }
}

/// Usage statistics leave the device: from an agent, that is manage-level.
#[test]
fn sharing_usage_statistics_is_manage_level() {
    let analytics = Action::SetPreference {
        preference: Preference::Analytics(true),
    };
    assert_eq!(analytics.risk(), Risk::Manage);
    assert_eq!(
        authorize(Actor::Agent, &analytics, &on()),
        Decision::Approve {
            by: Approver::Wallet
        }
    );
    let theme = Action::SetPreference {
        preference: Preference::Theme("dark".to_owned()),
    };
    assert_eq!(authorize(Actor::Agent, &theme, &on()), Decision::Allow);
}

/// The space decides which networks the whole wallet shows (spec 105): an
/// agent asks, the person allows.
#[test]
fn switching_spaces_is_the_persons_to_allow() {
    use vela_core::app::network_space::NetworkSpace;
    let switch = Action::SetNetworkSpace {
        space: NetworkSpace::Testnet,
    };
    assert_eq!(switch.risk(), Risk::Manage);
    assert_eq!(
        authorize(Actor::Agent, &switch, &on()),
        Decision::Approve {
            by: Approver::Wallet
        }
    );
    assert_eq!(authorize(Actor::Person, &switch, &on()), Decision::Allow);
    let parsed: Action = serde_json::from_value(
        serde_json::json!({ "type": "set_network_space", "space": "testnet" }),
    )
    .expect("an action");
    assert_eq!(parsed, switch);
}

/// The active wallet is what an agent reads and drafts from: switching it is
/// the person's to allow. Their own switch is theirs at once.
#[test]
fn switching_wallets_is_the_persons_to_allow() {
    let switch = Action::SwitchAccount {
        address: MINE.to_owned(),
    };
    assert_eq!(switch.risk(), Risk::Manage);
    assert_eq!(
        authorize(Actor::Agent, &switch, &on()),
        Decision::Approve {
            by: Approver::Wallet
        }
    );
    assert_eq!(authorize(Actor::Person, &switch, &on()), Decision::Allow);
}

/// Hiding the balances is the person's privacy: an agent may add to it, and
/// asks before taking it away.
#[test]
fn showing_hidden_balances_is_the_persons_to_allow() {
    let show = Action::SetBalancePrivacy { hidden: false };
    assert_eq!(show.risk(), Risk::Manage);
    assert_eq!(
        authorize(Actor::Agent, &show, &on()),
        Decision::Approve {
            by: Approver::Wallet
        }
    );
    let hide = Action::SetBalancePrivacy { hidden: true };
    assert_eq!(authorize(Actor::Agent, &hide, &on()), Decision::Allow);
    assert_eq!(authorize(Actor::Person, &show, &on()), Decision::Allow);
}

// ---------------------------------------------------------------------------
// What is well formed
// ---------------------------------------------------------------------------

#[test]
fn every_catalogued_example_is_well_formed() {
    for action in one_of_each() {
        assert_eq!(validate(&action), Ok(()), "{action:?}");
    }
}

#[test]
fn addresses_names_and_ids_must_be_real() {
    let bad = [
        Action::SaveContact {
            address: "0x123".to_owned(),
            name: "Short".to_owned(),
        },
        Action::SaveContact {
            address: ALICE.to_owned(),
            name: "   ".to_owned(),
        },
        Action::SwitchAccount {
            address: "alice.eth".to_owned(),
        },
        Action::SetGroupMembers {
            id: "g1".to_owned(),
            members: vec![ALICE.to_owned(), "nope".to_owned()],
        },
        Action::SaveGroup {
            id: Some(String::new()),
            name: "Work".to_owned(),
        },
        Action::ClearStorageItem {
            item: "everything".to_owned(),
        },
        Action::Show {
            screen: Screen::Settings {
                page: Some("secrets".to_owned()),
            },
        },
    ];
    for action in bad {
        let refusal = validate(&action).expect_err("refused");
        assert_eq!(refusal.code, ReasonCode::InvalidInput, "{action:?}");
    }
}

/// A preference outside the sets every shell shares is refused, naming the
/// ones that are allowed.
#[test]
fn a_preference_must_be_one_the_wallet_offers() {
    let refusal = validate(&Action::SetPreference {
        preference: Preference::Theme("neon".to_owned()),
    })
    .expect_err("refused");
    assert!(refusal.detail.contains("dark"), "{}", refusal.detail);

    for bad in [
        Preference::Language("xx".to_owned()),
        Preference::TextScale("huge".to_owned()),
        Preference::NumberFormat("roman".to_owned()),
        Preference::Currency("usd".to_owned()),
        Preference::FeeSpeed(FeeTier::Rapid),
    ] {
        let action = Action::SetPreference {
            preference: bad.clone(),
        };
        assert!(validate(&action).is_err(), "{bad:?}");
    }
    for good in [
        Preference::Language("auto".to_owned()),
        Preference::Language("zh-TW".to_owned()),
        Preference::TextScale("xlarge".to_owned()),
        Preference::Currency("JPY".to_owned()),
        Preference::FeeSpeed(FeeTier::Slow),
    ] {
        let action = Action::SetPreference {
            preference: good.clone(),
        };
        assert_eq!(validate(&action), Ok(()), "{good:?}");
    }
}

/// Who comes before what: an agent asking for the person's own setting is
/// told so, however malformed its request.
#[test]
fn the_check_refuses_who_before_what() {
    let malformed_and_forbidden = Action::ConfigureAgent {
        enabled: None,
        bridge_url: Some("not a url".to_owned()),
        client_token: None,
        wallets: None,
    };
    let checked = check(Actor::Agent, &malformed_and_forbidden, &on());
    assert_eq!(denied(&checked.decision), Some(ReasonCode::PersonOnly));
    assert_eq!(checked.kind.as_deref(), Some("configure_agent"));

    let malformed = Action::DeleteContact {
        address: "0x1".to_owned(),
    };
    let checked = check(Actor::Agent, &malformed, &on());
    assert_eq!(denied(&checked.decision), Some(ReasonCode::InvalidInput));
    assert_eq!(checked.risk, Some(Risk::Manage));
}

#[test]
fn json_that_is_no_action_is_refused_saying_why() {
    for json in [
        "",
        "{}",
        r#"{"type":"launch_rockets"}"#,
        r#"{"type":"save_contact"}"#,
    ] {
        let checked = check_json(Actor::Agent, json, &on());
        assert_eq!(checked.kind, None, "{json}");
        assert_eq!(
            denied(&checked.decision),
            Some(ReasonCode::InvalidInput),
            "{json}"
        );
    }
    let checked = check_json(
        Actor::Agent,
        r#"{"type":"set_preference","preference":{"kind":"fee_speed","value":"fast"}}"#,
        &on(),
    );
    assert_eq!(checked.decision, Decision::Allow);
    assert_eq!(checked.kind.as_deref(), Some("set_preference"));
}

// ---------------------------------------------------------------------------
// The catalog and the wire
// ---------------------------------------------------------------------------

/// Every action is in the catalog once, under the name it has on the wire.
#[test]
fn every_action_is_catalogued_under_its_wire_name() {
    let catalog = catalog();
    let actions = one_of_each();
    assert_eq!(catalog.len(), actions.len(), "one entry per action");
    for action in actions {
        let wire = serde_json::to_value(&action).expect("serializes");
        assert_eq!(wire["type"], action.kind(), "{action:?}");
        let entries = catalog
            .iter()
            .filter(|entry| entry.action == action.kind())
            .count();
        assert_eq!(entries, 1, "{} catalogued once", action.kind());
    }
    let person_only: Vec<&str> = catalog
        .iter()
        .filter(|entry| entry.person_only)
        .map(|entry| entry.action.as_str())
        .collect();
    assert_eq!(person_only, ["configure_agent"]);
}

#[test]
fn actions_read_and_write_their_wire_form() {
    let show: Action =
        serde_json::from_str(r#"{"type":"show","screen":{"type":"settings","page":"agent"}}"#)
            .expect("parses");
    assert_eq!(
        show,
        Action::Show {
            screen: Screen::Settings {
                page: Some("agent".to_owned())
            }
        }
    );
    let check = serde_json::to_value(check(Actor::Agent, &Action::SignOut, &on())).expect("json");
    assert_eq!(
        check,
        serde_json::json!({
            "kind": "sign_out",
            "risk": "manage",
            "decision": { "decision": "approve", "by": "owner" }
        })
    );
}

// ---------------------------------------------------------------------------
// The tools' input schemas
// ---------------------------------------------------------------------------

/// Each catalogued example's fields are what its schema describes: nothing
/// the schema leaves out, nothing it requires missing.
#[test]
fn every_schema_describes_its_actions_fields() {
    let catalog = catalog();
    for action in one_of_each() {
        let entry = catalog
            .iter()
            .find(|entry| entry.action == action.kind())
            .expect("catalogued");
        let schema = &entry.input_schema;
        assert_eq!(schema["type"], "object", "{}", action.kind());
        let properties = schema["properties"].as_object().expect("properties");
        let wire = serde_json::to_value(&action).expect("serializes");
        for field in wire.as_object().expect("object").keys() {
            if field != "type" {
                assert!(properties.contains_key(field), "{}.{field}", action.kind());
            }
        }
        for required in schema["required"].as_array().expect("required") {
            let field = required.as_str().expect("name");
            assert!(
                wire.get(field).is_some(),
                "{} requires {field}",
                action.kind()
            );
        }
    }
}

/// A tool never offers a value the check then refuses.
#[test]
fn every_value_a_schema_offers_is_one_the_check_accepts() {
    let catalog = catalog();
    let schema = |kind: &str| {
        catalog
            .iter()
            .find(|entry| entry.action == kind)
            .expect("catalogued")
            .input_schema
            .clone()
    };
    let preference = schema("set_preference");
    let branches = preference["properties"]["preference"]["oneOf"]
        .as_array()
        .expect("branches");
    assert_eq!(branches.len(), 9, "one branch per preference");
    for branch in branches {
        let kind = branch["properties"]["kind"]["const"]
            .as_str()
            .expect("kind");
        let Some(values) = branch["properties"]["value"]["enum"].as_array() else {
            continue;
        };
        for value in values {
            let action: Action = serde_json::from_value(serde_json::json!({
                "type": "set_preference",
                "preference": { "kind": kind, "value": value }
            }))
            .expect("parses");
            assert_eq!(validate(&action), Ok(()), "{kind} = {value}");
        }
    }
    for item in schema("clear_storage_item")["properties"]["item"]["enum"]
        .as_array()
        .expect("items")
    {
        let action = Action::ClearStorageItem {
            item: item.as_str().expect("id").to_owned(),
        };
        assert_eq!(validate(&action), Ok(()), "{item}");
    }
}

// ---------------------------------------------------------------------------
// Transfers
// ---------------------------------------------------------------------------

/// A draft moves nothing: the review it lands on is where the person decides.
#[test]
fn an_agent_may_prepare_a_transfer_without_a_grant() {
    let prepare = Action::PrepareTransfer {
        chain_id: 5_042_002,
        token: Some(MINE.to_owned()),
        to: ALICE.to_owned(),
        amount: "max".to_owned(),
    };
    assert_eq!(prepare.risk(), Risk::Transact);
    assert_eq!(authorize(Actor::Agent, &prepare, &on()), Decision::Allow);
}

/// An agent's confirm only asks the person (owner, 2026-10-21): it signs
/// nothing, so it needs no grant — and none exists to give.
#[test]
fn an_agents_confirm_only_asks_the_person() {
    let submit = Action::SubmitTransfer {
        operation: "transfer-1".to_owned(),
        review: None,
    };
    assert_eq!(authorize(Actor::Agent, &submit, &on()), Decision::Allow);
    assert_eq!(authorize(Actor::Person, &submit, &on()), Decision::Allow);
    assert_eq!(
        denied(&authorize(Actor::Agent, &submit, &Grants::default())),
        Some(ReasonCode::AgentOff)
    );
    let catalog = catalog();
    let entry = catalog
        .iter()
        .find(|entry| entry.action == "submit_transfer")
        .map(|entry| entry.description.as_str());
    assert!(entry.is_some_and(|text| text.contains("Nothing is signed by this call")));
}

#[test]
fn a_transfer_amount_is_a_positive_decimal_or_max() {
    let with = |amount: &str| Action::PrepareTransfer {
        chain_id: 1,
        token: None,
        to: ALICE.to_owned(),
        amount: amount.to_owned(),
    };
    for good in ["1", "0.5", ".5", "1000000.000001", "max"] {
        assert_eq!(validate(&with(good)), Ok(()), "{good}");
    }
    for bad in [
        "", "0", "0.000", "-1", "1,5", "1.", "1e18", "MAX", "all", "1.2.3",
    ] {
        assert!(validate(&with(bad)).is_err(), "{bad}");
    }
    let no_chain = Action::PrepareTransfer {
        chain_id: 0,
        token: None,
        to: ALICE.to_owned(),
        amount: "1".to_owned(),
    };
    assert!(validate(&no_chain).is_err());
}

#[test]
fn a_batch_needs_real_recipients_each_with_an_amount() {
    let with = |recipients: Vec<Recipient>| Action::PrepareBatchTransfer {
        chain_id: 1,
        token: None,
        recipients,
    };
    let to = |amount: &str| Recipient {
        to: ALICE.to_owned(),
        amount: amount.to_owned(),
    };
    assert_eq!(validate(&with(vec![to("1"), to("0.5")])), Ok(()));
    assert!(validate(&with(Vec::new())).is_err(), "nobody to send to");
    assert!(
        validate(&with(vec![to("max")])).is_err(),
        "max is for one recipient"
    );
    assert!(validate(&with(vec![to("0")])).is_err());
    assert!(validate(&with(vec![to("1"); MAX_RECIPIENTS + 1])).is_err());
    let stranger = Recipient {
        to: "bob.eth".to_owned(),
        amount: "1".to_owned(),
    };
    assert!(validate(&with(vec![stranger])).is_err());
    assert_eq!(
        authorize(Actor::Agent, &with(vec![to("1")]), &on()),
        Decision::Allow,
        "a draft, like one recipient's"
    );
}

#[test]
fn every_new_screen_is_well_formed_or_refused() {
    for screen in [
        Screen::Receive { chain_id: None },
        Screen::Receive {
            chain_id: Some(5_042_002),
        },
        Screen::Asset {
            chain_id: 1,
            token: None,
        },
        Screen::Activity {
            id: "1-0xabc-0".to_owned(),
        },
    ] {
        assert_eq!(
            validate(&Action::Show {
                screen: screen.clone()
            }),
            Ok(()),
            "{screen:?}"
        );
    }
    for screen in [
        Screen::Receive { chain_id: Some(0) },
        Screen::Asset {
            chain_id: 1,
            token: Some("nope".to_owned()),
        },
        Screen::Activity { id: " ".to_owned() },
    ] {
        assert!(
            validate(&Action::Show {
                screen: screen.clone()
            })
            .is_err(),
            "{screen:?}"
        );
    }
}

#[test]
fn network_changes_are_manage_level_and_need_real_urls() {
    let set = |url: &str| Action::SetNetworkEndpoint {
        chain_id: 1,
        field: NetOverrideField::Rpc,
        url: url.to_owned(),
    };
    assert_eq!(validate(&set("")), Ok(()), "empty restores the default");
    assert_eq!(validate(&set("http://127.0.0.1:8545")), Ok(()));
    for bad in ["rpc.example", "https://", "ftp://x", "https://a b"] {
        assert!(validate(&set(bad)).is_err(), "{bad}");
    }
    let service = |url: &str| Action::SetServiceEndpoint {
        field: NetEndpointField::FiatRates,
        url: url.to_owned(),
    };
    assert_eq!(
        validate(&service("")),
        Ok(()),
        "a cleared field is the default"
    );
    assert!(validate(&service("rates.example")).is_err());
    for action in one_of_each().into_iter().filter(|action| {
        matches!(
            action,
            Action::AddNetwork { .. }
                | Action::SetNetworkEndpoint { .. }
                | Action::RemoveNetwork { .. }
                | Action::SetProviderKey { .. }
                | Action::SetServiceEndpoint { .. }
                | Action::ResetServiceEndpoints
        )
    }) {
        assert_eq!(
            authorize(Actor::Agent, &action, &on()),
            Decision::Approve {
                by: Approver::Wallet
            },
            "{action:?}"
        );
    }
}

/// Every change an agent makes opens where it is seen — or shows itself.
#[test]
fn every_action_shows_where_its_change_is_seen() {
    use vela_core::app::intents::SETTINGS_PAGES;
    for action in one_of_each() {
        match shows(&action) {
            Some(Screen::Settings { page: Some(page) }) => {
                assert!(SETTINGS_PAGES.contains(&page.as_str()), "{page}");
            }
            Some(_) => {}
            None => assert!(
                is_transfer(&action)
                    || matches!(
                        action,
                        Action::SignOut
                            | Action::EraseDevice
                            | Action::ConfigureAgent { .. }
                            // A dApp's request shows on its own sheet.
                            | Action::RejectDappRequest { .. }
                            | Action::AskPersonDappRequest { .. }
                    ),
                "{} shows nothing",
                action.kind()
            ),
        }
    }
    let save: Action = serde_json::from_value(serde_json::json!({
        "type": "save_contact", "address": "0x1111111111111111111111111111111111111111", "name": "Alice"
    }))
    .unwrap();
    assert_eq!(
        shows(&save),
        Some(Screen::Contact {
            address: "0x1111111111111111111111111111111111111111".into()
        })
    );
}

#[test]
fn explore_reads_the_browser_machines_own_views() {
    use vela_core::app::browser_history::{BhistEntry, BhistView};
    use vela_core::app::dapp_browser::{DbrConsentView, DbrSiteView, DbrSigningView, DbrView};
    use vela_core::app::explore_sites::ExploreView;
    use vela_core::app::intent_rules::{dapp_request_target, explore_facts};

    let mut explore: ExploreView = serde_json::from_value(serde_json::json!({
        "favorites": [], "tabs": [], "selected_tab": null, "favorites_hidden": false,
        "recent_hidden": false, "favorites_full": false, "tabs_full": false
    }))
    .unwrap_or_default();
    explore.tabs = serde_json::from_value(serde_json::json!([
        { "id": "t1", "url": "https://app.uniswap.org/swap", "title": "Uniswap", "host": "app.uniswap.org" },
        { "id": "t2", "url": null, "title": "", "host": "" }
    ]))
    .expect("tabs");
    explore.selected_tab = Some("t1".into());
    let history = BhistView {
        entries: vec![BhistEntry {
            origin: UNISWAP.into(),
            url: format!("{UNISWAP}/swap"),
            host: "app.uniswap.org".into(),
            title: "Uniswap".into(),
            favicon: String::new(),
            last_visited_ms: 1.0,
        }],
    };
    let mut dapp = DbrView {
        sites: vec![DbrSiteView {
            origin: UNISWAP.into(),
            address: MINE.into(),
            chain_id: 8453,
            granted_at_ms: 1.0,
        }],
        ..DbrView::default()
    };
    dapp.consent = Some(DbrConsentView {
        tab: "t1".into(),
        origin: UNISWAP.into(),
        methods: vec!["eth_requestAccounts".into()],
        address: Some(MINE.into()),
        chain_id: 1,
        lock: Default::default(),
    });

    let facts = explore_facts(&explore, &dapp, &history, true, false, None);
    assert_eq!(facts.tabs.len(), 2);
    assert!(facts.tabs[0].current);
    assert_eq!(facts.tabs[1].url, "");
    assert_eq!(facts.recent[0].origin, UNISWAP);
    assert_eq!(facts.connected[0].chain_id, Some(8453));
    let pending = facts.pending.expect("the connect request");
    assert_eq!((pending.id.as_str(), pending.kind.as_str(), pending.shown), ("connect:t1", "connect", false));
    let shown = explore_facts(&explore, &dapp, &history, true, false, Some("connect:t1"));
    assert!(shown.pending.expect("shown").shown);

    dapp.consent = None;
    dapp.signing = Some(DbrSigningView { tab: "t1".into(), id: "42".into() });
    let signing = explore_facts(&explore, &dapp, &history, false, false, None).pending.expect("signing");
    assert_eq!(signing.id, "sign:t1:42");
    let target = dapp_request_target(&signing.id).expect("parses");
    assert_eq!((target.kind.as_str(), target.tab.as_str(), target.id.as_deref()), ("sign", "t1", Some("42")));
    assert_eq!(dapp_request_target("connect:t1").expect("connect").id, None);
    for bad in ["", "sign:t1", "connect:", "pay:t1:1", "sign::1"] {
        assert_eq!(dapp_request_target(bad), None, "{bad}");
    }
}

#[test]
fn a_sites_network_change_names_the_site_both_networks_and_the_wallet() {
    use vela_core::app::intent_rules::approval_detail_in;
    let world: World = serde_json::from_value(serde_json::json!({
        "session": {
            "address": MINE,
            "accounts": [
                { "index": 0, "account": { "address": MINE, "name": "Agent test" } },
                { "index": 1, "account": { "address": ALICE, "name": "Main" } }
            ]
        },
        "networks": { "networks": [
            { "chain_id": 1, "display_name": "Ethereum" },
            { "chain_id": 8453, "display_name": "Base" }
        ] },
        "explore": { "browser": true, "connected": [
            { "origin": UNISWAP, "chain_id": 1, "address": MINE }
        ] }
    }))
    .expect("a world");
    assert_eq!(
        approval_detail_in(&Action::SetSiteNetwork { origin: UNISWAP.into(), chain_id: 8453 }, &world).as_deref(),
        Some("app.uniswap.org · Ethereum (1) → Base (8453) · Agent test (0xaaaa…aaaa)")
    );
    assert_eq!(
        approval_detail_in(&Action::DisconnectSite { origin: UNISWAP.into() }, &world).as_deref(),
        Some("app.uniswap.org · Agent test (0xaaaa…aaaa)")
    );
    assert_eq!(
        approval_detail_in(&Action::SwitchAccount { address: ALICE.into() }, &world).as_deref(),
        Some("Agent test (0xaaaa…aaaa) → Main (0x0000…05fa)")
    );
    // Without what it needs, it still says what it can.
    assert_eq!(
        approval_detail_in(&Action::SetSiteNetwork { origin: UNISWAP.into(), chain_id: 10 }, &World::default()).as_deref(),
        Some("app.uniswap.org · no network yet → chain 10")
    );
}

/// The S22 run: a browser the person has not opened yet has no views — the
/// agent still sees Explore (empty), so it can open a site, which starts it.
#[test]
fn a_browser_not_started_yet_reads_as_empty() {
    let facts: serde_json::Value =
        serde_json::from_str(&vela_core::app::intent_json::explore_facts("null", "null", "null", false, false, None))
            .expect("json");
    assert_eq!(facts["browser"], true);
    assert_eq!(facts["tabs"], serde_json::json!([]));
}
