//! A member’s label and kind as this device can tell them (r4D-5) — labels
//! only, never authority: who may act is the genesis’s and each chain’s
//! owners. The order: this device’s own name for one of its accounts, else
//! the name a multisig here keeps for it (typed by its creator, or carried
//! by an invite), else the name the member registered for itself in the
//! passkey index, else its short address (EIP-55).
//!
//! The shell keeps the passkey index’s answers (its I/O and its reactive
//! cache) and asks here: `names.label`, `names.kind`, `names.short`.

use serde_json::Value;

use super::{arg, to_json};
use crate::multisig_link::eip55;
use crate::CoreError;

/// `address` as a list shows it: EIP-55, its first eight and last six
/// characters (the wallet’s own short form).
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

/// The name to show for `address`: `own` (this device’s account name),
/// else `kept` (what a multisig here keeps), else `registered` (the passkey
/// index’s), else its short address.
pub fn label(
    address: &str,
    own: Option<&str>,
    kept: Option<&str>,
    registered: Option<&str>,
) -> String {
    named(own)
        .or_else(|| named(kept))
        .or_else(|| named(registered))
        .map_or_else(|| short_address(address), str::to_owned)
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
        "label" => to_json(&label(
            &arg::<String>(args, "address")?,
            text("own")?.as_deref(),
            text("kept")?.as_deref(),
            text("registered")?.as_deref(),
        )),
        "kind" => to_json(&kind(flag("own")?, flag("vela")?, text("kept")?.as_deref())),
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
    fn a_member_shows_by_its_short_address_until_a_name_answers() {
        assert_eq!(short_address(BOB), "0x3C44Cd…4293BC");
        assert_eq!(label(BOB, None, None, None), "0x3C44Cd…4293BC");
        assert_eq!(label(BOB, None, None, Some("bob.vela")), "bob.vela");
        assert_eq!(
            label(BOB, None, Some("Bob"), Some("bob.vela")),
            "Bob",
            "kept before the index"
        );
        assert_eq!(
            label(BOB, None, Some("  "), None),
            "0x3C44Cd…4293BC",
            "a blank name is none"
        );
    }

    #[test]
    fn this_devices_own_name_for_its_accounts_comes_first() {
        assert_eq!(
            label(BOB, Some("My account"), Some("Not me"), Some("x")),
            "My account"
        );
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
