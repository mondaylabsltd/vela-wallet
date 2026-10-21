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
use vela_core::app::intents::{
    authorize, catalog, check, check_json, validate, Action, Actor, Approver, Decision, Grants,
    Preference, ReasonCode, Recipient, Risk, Screen, MAX_RECIPIENTS,
};
use vela_core::app::network_admin::{NetEndpointField, NetOverrideField, NetProviderId};

const ALICE: &str = "0x00000000219ab540356cbb839cbe05303d7705fa";
const MINE: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

const ON: Grants = Grants {
    agent_enabled: true,
};

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
        },
        Action::PrepareTransfer {
            chain_id: 1,
            token: None,
            to: ALICE.to_owned(),
            amount: "0.5".to_owned(),
        },
        Action::SubmitTransfer {
            operation: "transfer-1".to_owned(),
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
    ]
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
    for action in [
        Action::ConfigureAgent {
            enabled: None,
            bridge_url: Some("wss://elsewhere.example/connect".to_owned()),
            client_token: None,
        },
    ] {
        for grants in [ON, Grants::default()] {
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
        let decision = authorize(Actor::Agent, &action, &ON);
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
        authorize(Actor::Agent, &analytics, &ON),
        Decision::Approve {
            by: Approver::Wallet
        }
    );
    let theme = Action::SetPreference {
        preference: Preference::Theme("dark".to_owned()),
    };
    assert_eq!(authorize(Actor::Agent, &theme, &ON), Decision::Allow);
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
    };
    let checked = check(Actor::Agent, &malformed_and_forbidden, &ON);
    assert_eq!(denied(&checked.decision), Some(ReasonCode::PersonOnly));
    assert_eq!(checked.kind.as_deref(), Some("configure_agent"));

    let malformed = Action::DeleteContact {
        address: "0x1".to_owned(),
    };
    let checked = check(Actor::Agent, &malformed, &ON);
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
        let checked = check_json(Actor::Agent, json, &ON);
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
        &ON,
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
    let check = serde_json::to_value(check(Actor::Agent, &Action::SignOut, &ON)).expect("json");
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
    assert_eq!(authorize(Actor::Agent, &prepare, &ON), Decision::Allow);
}

/// An agent's confirm only asks the person (owner, 2026-10-21): it signs
/// nothing, so it needs no grant — and none exists to give.
#[test]
fn an_agents_confirm_only_asks_the_person() {
    let submit = Action::SubmitTransfer {
        operation: "transfer-1".to_owned(),
    };
    assert_eq!(authorize(Actor::Agent, &submit, &ON), Decision::Allow);
    assert_eq!(authorize(Actor::Person, &submit, &ON), Decision::Allow);
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
        authorize(Actor::Agent, &with(vec![to("1")]), &ON),
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
    assert_eq!(validate(&service("")), Ok(()), "a cleared field is the default");
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
            authorize(Actor::Agent, &action, &ON),
            Decision::Approve {
                by: Approver::Wallet
            },
            "{action:?}"
        );
    }
}
