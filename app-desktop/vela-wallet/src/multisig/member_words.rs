//! A multisig's members and its record, said (AUDIT M1, M3): the web's
//! `MemberChanges.svelte`, `MemberChangeRule.svelte` and
//! `MultisigSettings.svelte` wording, over what the core decided — the
//! members machine's plan, refusals and entry notes, a review's member
//! changes, the accounts machine's rename plan and backup state. Nothing is
//! decided here: each function puts the corpus' words (`home.multisig.*`) on
//! one of the core's values. gpui-free, so every line is unit-tested.

use serde_json::Value;
use vela_core::app::multisig_accounts::{BackupState, RenameBlocked, RenamePlan};
use vela_core::app::multisig_members::{EntryNote, MemberRule, PlanNetwork, Recovery, RuleChip};
use vela_core::multisig::{Action, MemberRefusal, MemberVerdict};
use vela_core::multisig_reading::{ChangeKind, MemberChanges, Rule};

use super::words::{self, t, tc, tv};
use crate::loc::Loc;

/// "2 of 3" (`members.ruleValue`): the stepper's value and a plan's sides.
#[must_use]
pub fn rule_value(loc: &Loc, m: u64, n: u64) -> String {
    tv(
        loc,
        "members.ruleValue",
        &[("m", &m.to_string()), ("n", &n.to_string())],
    )
}

/// "2 of 3" as a review's rule reads (`ruleText`, `create.thresholdValue`).
#[must_use]
pub fn rule_text(loc: &Loc, rule: &Rule) -> String {
    tv(
        loc,
        "create.thresholdValue",
        &[
            ("m", &rule.threshold.to_string()),
            ("n", &rule.owners.to_string()),
        ],
    )
}

/// The rule as a requirement: "Needs 2 of 3 members" (`ruleTitle`).
#[must_use]
pub fn rule_title(loc: &Loc, rule: &MemberRule) -> String {
    words::rule_title(loc, rule.m, rule.n)
}

/// A switcher row's chip (r9-14, §9.2): the followed multisig's from the
/// proposals machine, any other's as the accounts machine keeps it
/// (`AccountRow.rule`) — "Last known — read again when you open it" beside
/// a rule a chain said in an earlier session.
#[must_use]
pub fn row_chip(
    loc: &Loc,
    rule: &vela_core::app::multisig_accounts::RowRule,
    followed: Option<&RuleChip>,
) -> String {
    match followed {
        Some(chip) => self::chip(loc, chip).0,
        None => {
            let text = self::chip(loc, &rule.chip).0;
            if rule.from_chain && rule.last_known {
                format!("{text} · {}", t(loc, "presence.ruleLastKnown"))
            } else {
                text
            }
        }
    }
}

/// The chip and its title (`ruleChip`): "Needs 2 of 3", or "Rule differs
/// by network" with each rule in the title.
#[must_use]
pub fn chip(loc: &Loc, chip: &RuleChip) -> (String, String) {
    match chip {
        RuleChip::Needs { rule } => (words::rule_chip(loc, rule.m, rule.n), rule_title(loc, rule)),
        RuleChip::Differs { rules } => (
            t(loc, "presence.ruleDiffers"),
            rules
                .iter()
                .map(|r| rule_title(loc, r))
                .collect::<Vec<_>>()
                .join(" · "),
        ),
    }
}

/// A member change refused on a network, in one sentence
/// (`members.refuse.<kind>`, its fields as the sentence's variables).
#[must_use]
pub fn refusal_text(loc: &Loc, refusal: &MemberRefusal) -> String {
    let value = serde_json::to_value(refusal).unwrap_or(Value::Null);
    let kind = value
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or("notAnAddress")
        .to_owned();
    let vars: Vec<(String, String)> = value
        .as_object()
        .into_iter()
        .flatten()
        .filter(|(k, _)| k.as_str() != "kind")
        .map(|(k, v)| {
            let text = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            (k.clone(), text)
        })
        .collect();
    let pairs: Vec<(&str, &str)> = vars.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    tv(loc, &format!("members.refuse.{kind}"), &pairs)
}

/// A member change's refusal the core wrote as JSON in a `refused` reason
/// (`memberRefusal`), else `None`.
#[must_use]
pub fn refusal_of(reason: &str) -> Option<MemberRefusal> {
    if !reason.trim_start().starts_with('{') {
        return None;
    }
    serde_json::from_str(reason).ok()
}

/// What one network's part of a plan says, and whether it is a refusal:
/// "Gnosis: 2 of 3 → 2 of 4", or "Gnosis: Already a member there."
#[must_use]
pub fn plan_line(loc: &Loc, row: &PlanNetwork, network: &str) -> (String, bool) {
    match &row.verdict {
        MemberVerdict::Ready { before, after, .. } => (
            tv(
                loc,
                "members.onNetwork",
                &[
                    ("network", network),
                    (
                        "before",
                        &rule_value(loc, before.threshold, before.owners.len() as u64),
                    ),
                    (
                        "after",
                        &rule_value(loc, after.threshold, after.owners.len() as u64),
                    ),
                ],
            ),
            false,
        ),
        MemberVerdict::Refused { refusal } => {
            (format!("{network}: {}", refusal_text(loc, refusal)), true)
        }
    }
}

/// Why what is typed is no member (`entryNote`).
#[must_use]
pub fn entry_note(loc: &Loc, note: &EntryNote) -> String {
    match note {
        EntryNote::InvalidAddress => t(loc, "send.invalidAddress"),
        EntryNote::InvalidChecksum => t(loc, "send.invalidChecksum"),
        EntryNote::NoOne => t(loc, "members.refuse.noOne"),
        EntryNote::NameNotFound { name } => tv(loc, "members.nameNotFound", &[("name", name)]),
        EntryNote::NameUnavailable { name } => {
            tv(loc, "members.nameUnavailable", &[("name", name)])
        }
    }
}

/// The passkey-native recovery note under the approvals.
#[must_use]
pub fn recovery(loc: &Loc, recovery: &Recovery) -> String {
    match recovery {
        Recovery::Replaceable { m, n } => tv(
            loc,
            "members.recovery",
            &[("m", &m.to_string()), ("n", &n.to_string())],
        ),
        Recovery::EveryMember => t(loc, "members.recoveryAll"),
    }
}

/// A change waiting, titled (`title` in `MemberChanges.svelte`); `who`
/// names a member.
#[must_use]
pub fn waiting_title(loc: &Loc, action: &Action, who: &dyn Fn(&str) -> String) -> String {
    match action {
        Action::AddOwner { owner, .. } => tv(loc, "members.changeAdd", &[("name", &who(owner))]),
        Action::RemoveOwner { owner, .. } => {
            tv(loc, "members.changeRemove", &[("name", &who(owner))])
        }
        Action::SwapOwner {
            old_owner,
            new_owner,
        } => tv(
            loc,
            "members.changeReplace",
            &[("old", &who(old_owner)), ("new", &who(new_owner))],
        ),
        // r9: "Require 1 approval" — plural by the count.
        Action::ChangeThreshold { threshold } => tc(
            loc,
            "action.changeThreshold",
            threshold.parse().unwrap_or(0),
        ),
        _ => String::new(),
    }
}

/// What a review's member changes say (`changeLines`, `MemberChangeRule`):
/// each change, the rule it meets and the one it leaves ("Approvals: 2 of
/// 3 → 2 of 4"), the caution, and that it would fail where it does not fit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeLines {
    pub changes: Vec<String>,
    pub rule: String,
    pub caution: String,
    pub does_not_fit: Option<String>,
}

#[must_use]
pub fn change_lines(
    loc: &Loc,
    reading: &MemberChanges,
    network: &str,
    who: &dyn Fn(&str) -> String,
) -> ChangeLines {
    let changes = reading
        .changes
        .iter()
        .map(|c| {
            let added = c.added.as_deref().unwrap_or_default();
            let removed = c.removed.as_deref().unwrap_or_default();
            let text = match c.kind {
                ChangeKind::Add => tv(loc, "change.add", &[("member", &who(added))]),
                ChangeKind::Remove => tv(loc, "change.remove", &[("member", &who(removed))]),
                ChangeKind::Replace => tv(
                    loc,
                    "change.replace",
                    &[("old", &who(removed)), ("new", &who(added))],
                ),
                ChangeKind::Threshold => t(loc, "change.threshold"),
            };
            match c.call {
                Some(n) => format!("#{n} · {text}"),
                None => text,
            }
        })
        .collect();
    ChangeLines {
        changes,
        rule: tv(
            loc,
            "change.rule",
            &[
                ("before", &rule_text(loc, &reading.before)),
                ("after", &rule_text(loc, &reading.after)),
            ],
        ),
        caution: tv(loc, "change.caution", &[("network", network)]),
        does_not_fit: (!reading.fits)
            .then(|| tv(loc, "change.doesNotFit", &[("network", network)])),
    }
}

/// Where the shared name's rename stands (the accounts machine's
/// `plan_rename`), as Settings draws it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenameStand {
    /// Not known yet: nothing said.
    Asking,
    /// No chain to rename it on, said as itself (r4a-12, r7-10).
    Blocked(String),
    /// Its record's chain, where it is not deployed: a rename never deploys it.
    NotDeployed(String),
    /// It can be proposed on `chain_id`; `note` says where it runs.
    Can { chain_id: u64, note: String },
}

/// What a rename plan says (`MultisigSettings.svelte`'s rename block);
/// `network` names a chain.
#[must_use]
pub fn rename_stand(
    loc: &Loc,
    plan: Option<&RenamePlan>,
    network: &dyn Fn(u64) -> String,
) -> RenameStand {
    let Some(plan) = plan else {
        return RenameStand::Asking;
    };
    let Some(chain_id) = plan.chain_id else {
        return RenameStand::Blocked(match plan.blocked {
            Some(RenameBlocked::Pending) => tv(
                loc,
                "settings.renamePending",
                // Where a record on its way lands: the service's home chain.
                &[("network", &network(plan.lands_on.unwrap_or(100)))],
            ),
            Some(RenameBlocked::Unrecorded) => t(loc, "settings.renameNotRecorded"),
            Some(RenameBlocked::Unreachable) => t(loc, "add.unavailable"),
            Some(RenameBlocked::Elsewhere) | None => t(loc, "settings.renameUnavailable"),
        });
    };
    match plan.deployed {
        Some(true) => RenameStand::Can {
            chain_id,
            note: tv(
                loc,
                "settings.renameNote",
                &[("network", &network(chain_id))],
            ),
        },
        Some(false) => RenameStand::NotDeployed(tv(
            loc,
            "settings.renameNotDeployed",
            &[("network", &network(chain_id))],
        )),
        None => RenameStand::Asking,
    }
}

/// "Using a custom multisig service (127.0.0.1:8788)" while the service is
/// not the default one (`CustomServiceNote`): a fact, never a warning.
#[must_use]
pub fn custom_service(loc: &Loc, url: &str) -> Option<String> {
    let url = url.trim();
    if url.is_empty() || url == vela_core::app::multisig_shell::DEFAULT_GENESIS_SERVICE_URL {
        return None;
    }
    // The URL's host (`new URL(url).host`), else the URL as it is.
    let host = url
        .split_once("://")
        .map(|(_, rest)| rest.split(['/', '?', '#']).next().unwrap_or(rest))
        .filter(|h| !h.is_empty())
        .unwrap_or(url);
    Some(tv(loc, "service.customNote", &[("host", host)]))
}

/// The backup line (`backupLine`); `None` while it is being checked.
#[must_use]
pub fn backup_line(loc: &Loc, state: Option<&BackupState>) -> Option<String> {
    Some(match state? {
        BackupState::Checking => return None,
        BackupState::BackedUp | BackupState::Sent => t(loc, "settings.backedUp"),
        BackupState::NotBackedUp | BackupState::Failed => t(loc, "settings.notBackedUp"),
        BackupState::Unavailable => t(loc, "settings.unavailable"),
        BackupState::CouldNotCheck => t(loc, "settings.couldNotCheck"),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use vela_core::multisig::MemberSet;
    use vela_core::multisig_reading::MemberChange as Reading;

    fn en() -> Loc {
        Loc::for_tag("en")
    }

    fn net(id: u64) -> String {
        format!("Net{id}")
    }

    /// r9-14 (§9.2): a multisig not followed wears the rule the accounts
    /// machine keeps — the chain's "2 of 4", never the founding "2 of 3" —
    /// with "Last known" when a chain said it in an earlier session; the
    /// followed one wears the proposals machine's chip.
    #[test]
    fn a_switcher_row_wears_the_chains_rule() {
        use vela_core::app::multisig_accounts::RowRule;
        let loc = en();
        let chain = RuleChip::Needs {
            rule: vela_core::app::multisig_members::MemberRule::of(2, 4),
        };
        let now = RowRule {
            chip: chain.clone(),
            from_chain: true,
            last_known: false,
        };
        assert_eq!(row_chip(&loc, &now, None), "Needs 2 of 4");
        let kept = RowRule {
            last_known: true,
            ..now.clone()
        };
        let said = row_chip(&loc, &kept, None);
        assert!(said.starts_with("Needs 2 of 4 · "), "{said}");
        assert!(said.contains(&t(&loc, "presence.ruleLastKnown")), "{said}");
        let founding = RowRule::default();
        assert!(
            !row_chip(&loc, &founding, None).contains('·'),
            "the founding rule is no chain's"
        );
        let followed = RuleChip::Needs {
            rule: vela_core::app::multisig_members::MemberRule::of(3, 4),
        };
        assert_eq!(row_chip(&loc, &kept, Some(&followed)), "Needs 3 of 4");
    }

    #[test]
    fn a_review_says_the_rule_it_meets_and_leaves() {
        let reading = MemberChanges {
            changes: vec![Reading {
                call: None,
                kind: ChangeKind::Add,
                added: Some("0xD".to_owned()),
                removed: None,
                before: Rule {
                    threshold: 2,
                    owners: 3,
                },
                after: Rule {
                    threshold: 2,
                    owners: 4,
                },
                fits: true,
            }],
            before: Rule {
                threshold: 2,
                owners: 3,
            },
            after: Rule {
                threshold: 2,
                owners: 4,
            },
            fits: true,
        };
        let lines = change_lines(&en(), &reading, "Gnosis", &|a: &str| format!("<{a}>"));
        assert_eq!(lines.rule, "Approvals: 2 of 3 → 2 of 4");
        assert_eq!(lines.changes, vec!["Add <0xD>".to_owned()]);
        assert!(lines.caution.contains("Gnosis"));
        assert_eq!(lines.does_not_fit, None);
        let misfit = MemberChanges {
            fits: false,
            ..reading
        };
        assert!(
            change_lines(&en(), &misfit, "Gnosis", &|a: &str| a.to_owned())
                .does_not_fit
                .is_some_and(|l| l.contains("Gnosis"))
        );
    }

    #[test]
    fn a_plan_row_is_its_rule_or_its_refusal() {
        let set = |m: u64, n: usize| MemberSet {
            owners: (0..n).map(|i| format!("0x{i}")).collect(),
            threshold: m,
        };
        let refused = PlanNetwork {
            chain_id: 100,
            verdict: MemberVerdict::Refused {
                refusal: MemberRefusal::BelowThreshold {
                    owners: 1,
                    threshold: 2,
                },
            },
            proposal: None,
        };
        let (line, bad) = plan_line(&en(), &refused, "Gnosis");
        assert!(bad);
        assert_eq!(
            line,
            "Gnosis: That would leave 1 members while 2 must approve. Lower the approvals needed, or replace the member instead."
        );
        let zero = || "0x0".to_owned();
        let ready = PlanNetwork {
            chain_id: 100,
            verdict: MemberVerdict::Ready {
                tx: Box::new(vela_core::multisig::SafeTx {
                    to: zero(),
                    value: zero(),
                    data: "0x".to_owned(),
                    operation: 0,
                    safe_tx_gas: zero(),
                    base_gas: zero(),
                    gas_price: zero(),
                    gas_token: zero(),
                    refund_receiver: zero(),
                    nonce: zero(),
                }),
                before: set(2, 3),
                after: set(2, 4),
            },
            proposal: None,
        };
        assert_eq!(
            plan_line(&en(), &ready, "Gnosis"),
            ("Gnosis: 2 of 3 → 2 of 4".to_owned(), false)
        );
    }

    #[test]
    fn a_refusal_in_a_reason_is_read_back() {
        let json = serde_json::to_string(&MemberRefusal::ChangeWaiting).unwrap();
        assert_eq!(refusal_of(&json), Some(MemberRefusal::ChangeWaiting));
        assert_eq!(refusal_of("the chain did not answer"), None);
        assert_eq!(
            refusal_text(&en(), &MemberRefusal::TooMany { max: 10 }),
            "A multisig can have at most 10 members."
        );
    }

    #[test]
    fn a_rename_is_said_as_where_it_stands() {
        let plan = |chain_id, deployed, blocked, lands_on| RenamePlan {
            chain_id,
            deployed,
            blocked,
            lands_on,
        };
        let loc = en();
        assert_eq!(rename_stand(&loc, None, &net), RenameStand::Asking);
        let RenameStand::Blocked(pending) = rename_stand(
            &loc,
            Some(&plan(None, None, Some(RenameBlocked::Pending), Some(8453))),
            &net,
        ) else {
            panic!("pending");
        };
        assert!(pending.contains("Net8453"), "{pending}");
        assert_eq!(
            rename_stand(
                &loc,
                Some(&plan(None, None, Some(RenameBlocked::Unrecorded), None)),
                &net
            ),
            RenameStand::Blocked(t(&loc, "settings.renameNotRecorded"))
        );
        assert_eq!(
            rename_stand(
                &loc,
                Some(&plan(None, None, Some(RenameBlocked::Unreachable), None)),
                &net
            ),
            RenameStand::Blocked(t(&loc, "add.unavailable"))
        );
        assert_eq!(
            rename_stand(
                &loc,
                Some(&plan(None, None, Some(RenameBlocked::Elsewhere), None)),
                &net
            ),
            RenameStand::Blocked(t(&loc, "settings.renameUnavailable"))
        );
        assert!(matches!(
            rename_stand(&loc, Some(&plan(Some(100), Some(false), None, None)), &net),
            RenameStand::NotDeployed(text) if text.contains("Net100")
        ));
        assert!(matches!(
            rename_stand(&loc, Some(&plan(Some(100), Some(true), None, None)), &net),
            RenameStand::Can { chain_id: 100, .. }
        ));
    }

    #[test]
    fn the_chip_says_one_rule_or_that_they_differ() {
        let loc = en();
        let one = RuleChip::Needs {
            rule: MemberRule::of(2, 3),
        };
        assert_eq!(chip(&loc, &one).0, "Needs 2 of 3");
        let two = RuleChip::Differs {
            rules: vec![MemberRule::of(2, 3), MemberRule::of(2, 4)],
        };
        let (label, title) = chip(&loc, &two);
        assert_eq!(label, "Rule differs by network");
        assert_eq!(title, "Needs 2 of 3 members · Needs 2 of 4 members");
    }

    #[test]
    fn a_custom_service_is_named_by_its_host() {
        let loc = en();
        assert_eq!(
            custom_service(
                &loc,
                vela_core::app::multisig_shell::DEFAULT_GENESIS_SERVICE_URL
            ),
            None
        );
        assert_eq!(
            custom_service(&loc, "http://127.0.0.1:8788/api").as_deref(),
            Some("Using a custom multisig service (127.0.0.1:8788)")
        );
    }

    #[test]
    fn entry_notes_and_waiting_titles_read_the_corpus() {
        let loc = en();
        assert_eq!(
            entry_note(
                &loc,
                &EntryNote::NameNotFound {
                    name: "a.eth".to_owned()
                }
            ),
            "a.eth doesn't point to an address."
        );
        assert_eq!(
            waiting_title(
                &loc,
                &Action::ChangeThreshold {
                    threshold: "3".to_owned()
                },
                &|a: &str| a.to_owned()
            ),
            "Require 3 approvals"
        );
        assert_eq!(rule_value(&loc, 2, 4), "2 of 4");
        assert_eq!(backup_line(&loc, Some(&BackupState::Checking)), None);
        assert_eq!(
            backup_line(&loc, Some(&BackupState::Sent)),
            Some("On Ethereum".to_owned())
        );
    }
}
