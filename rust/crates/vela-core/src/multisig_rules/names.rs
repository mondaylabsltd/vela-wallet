//! A member’s label and kind as this device can tell them (r4D-5) — labels
//! only, never authority: who may act is the genesis’s and each chain’s
//! owners.
//!
//! r8-25 (owner, decided by best practice and the core principle): ONE name
//! per member on every device. The order is the shared names first: the name
//! a multisig here keeps for it (typed by its creator, or carried by an
//! invite — the same words on every device that took the same invite), else
//! the name the member registered for itself in the passkey index (public,
//! the same everywhere), else this device’s own name for one of its accounts
//! (a private nickname — two devices holding one key named it two ways:
//! "Parallel One", "Parallel space"), else its short address (EIP-55). Every
//! label is an unauthenticated hint: none of them decides anything, and a
//! carried one never overwrites what this device keeps (`name_members`).
//!
//! r10 (r9-12 / r8-25 / r4D-5 still seen, decided by best practice and the
//! core principle): the member's OWN published name — the passkey index's,
//! written with its founding key, the same answer on every device — comes
//! FIRST, before what a multisig here keeps. A kept label is one device's
//! (or one inviter's) word for someone else; the published name is the
//! member's own, and only it can be the same on a device that never took the
//! invite. So one address reads the same in every place it is named —
//! headers, the switcher, Activity, Settings, sheets and invites, this
//! device's own accounts included (their local name is the fallback when the
//! index holds none; login keeps the published name too). Every shell draws
//! the accounts machine's `member_labels` for any address it names.
//!
//! The shell keeps the passkey index’s answers (its I/O and its reactive
//! cache) and asks here: `names.label`, `names.kind`, `names.short`.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::{arg, to_json};
use crate::multisig_link::eip55;
use crate::CoreError;

/// `address` as a list shows it: EIP-55, its first eight and last six
/// characters — `0x` and six digits, `…`, six digits (`0x88cCA0…266894`).
///
/// r11-6: THE short form, on every shell and every surface (the web and
/// Android drew 6+6, iOS and the desktop 4+4: `0x88cC…6894`). Six and six,
/// decided by best practice and the core principle: a 4+4 look-alike is a
/// 2^32 search for an address-poisoning sender, 6+6 a 2^48 one, and the
/// short form is what a person compares. Shells call `names.short`
/// (`multisigLinkCall` / uniffi `multisig_link_call`), never their own.
pub fn short_address(address: &str) -> String {
    let full = eip55(address);
    let chars: Vec<char> = full.chars().collect();
    if chars.len() <= 14 {
        return full;
    }
    let head: String = chars[..8].iter().collect();
    let tail: String = chars[chars.len() - 6..].iter().collect();
    format!("{head}…{tail}")
}

fn named(name: Option<&str>) -> Option<&str> {
    name.map(str::trim).filter(|n| !n.is_empty())
}

/// The name to show for `address` (r10, r8-25): `registered` (the member's
/// own name in the passkey index), else `kept` (what a multisig here keeps),
/// else `own` (this device’s account name), else its short address.
pub fn label(
    address: &str,
    own: Option<&str>,
    kept: Option<&str>,
    registered: Option<&str>,
) -> String {
    named(registered)
        .or_else(|| named(kept))
        .or_else(|| named(own))
        .map_or_else(|| short_address(address), str::to_owned)
}

/// A member as a title names it (r11-6) — "Add Interleave
/// (0x88cCA0…266894)": its label and its short form, or the short form alone
/// when the label is none (absent, blank, or the short form itself). ONE
/// rule for every title that names a member — the sheet, Activity rows, the
/// outside page, notifications — with `label` the accounts machine's
/// `member_labels` (or [`label`] where no machine runs).
pub fn who(address: &str, label: Option<&str>) -> String {
    let short = short_address(address);
    match named(label) {
        Some(name) if name != short && !name.eq_ignore_ascii_case(address.trim()) => {
            format!("{name} ({short})")
        }
        _ => short,
    }
}

/// A member change's one title (r12-8), for Activity rows and sheets alike:
/// the corpus key and its parameters, each member named by [`who`] with
/// `labels[lowercase address]` (the accounts machine's `member_labels`, or
/// the name the sheet knows — the same map). The shell words it in the
/// page's own language at the moment it draws it — never a title frozen in
/// the language and with the names of the moment it was written ("Add
/// 0xD40086…de130b" on a zh Activity, "Add Dana Local (…)" on the sheet).
/// `count` is set for a threshold: say the key's plural form for it
/// (`_one` / `_other`). `None` for anything that is not a member change.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChangeTitle {
    pub key: &'static str,
    pub params: BTreeMap<&'static str, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<String>,
}

pub fn change_title(
    action: &crate::multisig::Action,
    labels: &BTreeMap<String, String>,
) -> Option<ChangeTitle> {
    use crate::multisig::Action;
    let name = |address: &str| {
        who(
            address,
            labels
                .get(&address.to_ascii_lowercase())
                .map(String::as_str),
        )
    };
    let one = |key: &'static str, k: &'static str, v: String| ChangeTitle {
        key,
        params: BTreeMap::from([(k, v)]),
        count: None,
    };
    Some(match action {
        Action::AddOwner { owner, .. } => one("home.multisig.change.add", "member", name(owner)),
        Action::RemoveOwner { owner, .. } => {
            one("home.multisig.change.remove", "member", name(owner))
        }
        Action::SwapOwner {
            old_owner,
            new_owner,
        } => ChangeTitle {
            key: "home.multisig.change.replace",
            params: BTreeMap::from([("old", name(old_owner)), ("new", name(new_owner))]),
            count: None,
        },
        Action::ChangeThreshold { threshold } => ChangeTitle {
            key: "home.multisig.action.changeThreshold",
            params: BTreeMap::from([("count", threshold.clone())]),
            count: Some(threshold.clone()),
        },
        _ => return None,
    })
}

/// What a member is, as far as this device can tell: a Vela account (one
/// of this device’s own, or the passkey index holds it), else what a
/// multisig here keeps it as, else `None` — not known (the index’s silence
/// is no answer: it may only have been out of reach).
pub fn kind(own: bool, vela: bool, kept: Option<&str>) -> Option<String> {
    if own || vela {
        return Some("passkey".to_owned());
    }
    kept.filter(|k| matches!(*k, "passkey" | "wallet" | "contract"))
        .map(str::to_owned)
}

/// `names.<name>`, its arguments one JSON object, its answer JSON.
pub fn call(name: &str, args: &Value) -> Result<String, CoreError> {
    let text =
        |key: &str| -> Result<Option<String>, CoreError> { arg::<Option<String>>(args, key) };
    let flag = |key: &str| -> Result<bool, CoreError> {
        Ok(arg::<Option<bool>>(args, key)?.unwrap_or(false))
    };
    match name {
        "short" => to_json(&short_address(&arg::<String>(args, "address")?)),
        "who" => to_json(&who(
            &arg::<String>(args, "address")?,
            text("label")?.as_deref(),
        )),
        "label" => to_json(&label(
            &arg::<String>(args, "address")?,
            text("own")?.as_deref(),
            text("kept")?.as_deref(),
            text("registered")?.as_deref(),
        )),
        "kind" => to_json(&kind(flag("own")?, flag("vela")?, text("kept")?.as_deref())),
        "change" => {
            let action: crate::multisig::Action = arg(args, "action")?;
            let labels: Option<BTreeMap<String, String>> = arg(args, "labels")?;
            let labels: BTreeMap<String, String> = labels
                .unwrap_or_default()
                .into_iter()
                .map(|(k, v)| (k.to_ascii_lowercase(), v))
                .collect();
            to_json(&change_title(&action, &labels))
        }
        other => Err(CoreError::Multisig(format!(
            "unknown names method: {other}"
        ))),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    //! `names.test.ts`’ rules (r4D-5, r5reg-16).
    use super::*;
    use serde_json::json;

    const BOB: &str = "0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc";

    #[test]
    fn a_title_names_a_member_one_way() {
        assert_eq!(who(BOB, None), "0x3C44Cd…4293BC");
        assert_eq!(who(BOB, Some("  ")), "0x3C44Cd…4293BC");
        assert_eq!(who(BOB, Some("0x3C44Cd…4293BC")), "0x3C44Cd…4293BC");
        assert_eq!(who(BOB, Some(BOB)), "0x3C44Cd…4293BC");
        assert_eq!(who(BOB, Some("Interleave")), "Interleave (0x3C44Cd…4293BC)");
        let answer = call("who", &json!({"address": BOB, "label": "Bob"})).unwrap();
        assert_eq!(answer, "\"Bob (0x3C44Cd…4293BC)\"");
    }

    #[test]
    fn a_member_shows_by_its_short_address_until_a_name_answers() {
        assert_eq!(short_address(BOB), "0x3C44Cd…4293BC");
        assert_eq!(label(BOB, None, None, None), "0x3C44Cd…4293BC");
        assert_eq!(label(BOB, None, None, Some("bob.vela")), "bob.vela");
        assert_eq!(
            label(BOB, None, Some("Bob"), Some("bob.vela")),
            "bob.vela",
            "r10: the member's own published name before a kept label"
        );
        assert_eq!(label(BOB, None, Some("Bob"), None), "Bob");
        assert_eq!(
            label(BOB, None, Some("  "), None),
            "0x3C44Cd…4293BC",
            "a blank name is none"
        );
    }

    /// r12-8: the member-change title vectors, through the door.
    #[test]
    fn the_member_title_vectors_hold() {
        let path = format!(
            "{}/tests/vectors/multisig/member-titles.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() >= 6);
        for case in cases {
            let got: Value = serde_json::from_str(&call("change", &case["args"]).unwrap()).unwrap();
            assert_eq!(got, case["expect"], "{}", case["name"]);
        }
    }

    /// The shared vectors every shell replays through the door (r10).
    #[test]
    fn the_name_vectors_hold() {
        let path = format!(
            "{}/tests/vectors/multisig/names.json",
            env!("CARGO_MANIFEST_DIR")
        );
        let v: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let cases = v["cases"].as_array().unwrap();
        assert!(cases.len() >= 6);
        for case in cases {
            let got: String = serde_json::from_str(&call("label", &case["args"]).unwrap()).unwrap();
            assert_eq!(got, case["expect"].as_str().unwrap(), "{}", case["name"]);
        }
    }

    #[test]
    fn one_name_per_member_on_every_device_the_shared_names_first() {
        // r10: the member's own registered name — the same on every device —
        // then what the multisig keeps (the invite's words), before this
        // device's private nickname for its account.
        assert_eq!(
            label(BOB, Some("Parallel space"), Some("Dana"), Some("x")),
            "x"
        );
        assert_eq!(
            label(BOB, Some("Parallel space"), Some("Dana"), None),
            "Dana"
        );
        assert_eq!(
            label(BOB, Some("Parallel space"), None, Some("Interleave")),
            "Interleave"
        );
        assert_eq!(label(BOB, Some("My account"), None, None), "My account");
        assert_eq!(
            kind(true, false, Some("wallet")).as_deref(),
            Some("passkey")
        );
    }

    #[test]
    fn a_kind_is_a_vela_account_found_else_what_is_kept_else_unknown() {
        assert_eq!(
            kind(false, true, Some("wallet")).as_deref(),
            Some("passkey")
        );
        assert_eq!(
            kind(false, false, Some("wallet")).as_deref(),
            Some("wallet")
        );
        assert_eq!(
            kind(false, false, Some("contract")).as_deref(),
            Some("contract")
        );
        assert_eq!(kind(false, false, Some("guess")), None);
        assert_eq!(
            kind(false, false, None),
            None,
            "the index silence is no answer"
        );
    }

    #[test]
    fn asked_through_the_door() {
        let ask = |method: &str, args: Value| {
            crate::multisig_rules::call_json(method, &args)
                .unwrap()
                .unwrap()
        };
        assert_eq!(
            ask("names.label", json!({ "address": BOB, "kept": "Bob" })),
            "\"Bob\""
        );
        assert_eq!(ask("names.kind", json!({ "kept": "wallet" })), "\"wallet\"");
        assert_eq!(ask("names.kind", json!({})), "null");
        assert_eq!(ask("names.short", json!({ "address": "0x12" })), "\"0x12\"");
    }
}
