//! The multisig screens' words (`home.multisig.*` in the vela-core corpus,
//! the on-demand catalog `loc.rs` merges): what the core decided, said. The
//! web's `core/display.ts` wording, nothing decided here — an amount is the
//! core's base units in the token's decimals (`multisig_link::format_units`),
//! a member is named by the core's `names` rule.

use vela_core::multisig::Action;

use crate::loc::Loc;

/// A token's symbol and decimals, as read for wording (`tokenInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenInfo {
    pub symbol: String,
    pub decimals: u32,
}

/// `home.multisig.<key>`.
#[must_use]
pub fn key(path: &str) -> String {
    format!("home.multisig.{path}")
}

/// `home.multisig.<path>` in `loc`.
#[must_use]
pub fn t(loc: &Loc, path: &str) -> String {
    loc.t(&key(path)).to_string()
}

/// `home.multisig.<path>` with text variables.
#[must_use]
pub fn tv(loc: &Loc, path: &str, vars: &[(&str, &str)]) -> String {
    loc.t_texts(&key(path), vars).to_string()
}

/// A plural `home.multisig.<path>` (`_one` / `_other` …, the core's rule).
#[must_use]
pub fn tc(loc: &Loc, path: &str, count: usize) -> String {
    loc.t_count(&key(path), count).to_string()
}

/// Base units in `decimals`, as the core writes an amount (`formatUnits`).
#[must_use]
pub fn units(amount: &str, decimals: u32) -> String {
    vela_core::multisig_link::format_units(amount, decimals).unwrap_or_else(|_| amount.to_owned())
}

/// The rule's words: "Needs 2 of 3 members", or the one-member form.
#[must_use]
pub fn rule_title(loc: &Loc, m: u64, n: u64) -> String {
    if n == 1 {
        return t(loc, "panel.titleSolo");
    }
    tv(
        loc,
        "panel.title",
        &[("m", &m.to_string()), ("n", &n.to_string())],
    )
}

/// The header chip: "Needs 2 of 3", or "One member".
#[must_use]
pub fn rule_chip(loc: &Loc, m: u64, n: u64) -> String {
    if n == 1 {
        return t(loc, "panel.chipSolo");
    }
    tv(
        loc,
        "panel.chip",
        &[("m", &m.to_string()), ("n", &n.to_string())],
    )
}

/// What a proposal is, in one line (the web's `actionTitle`): `who` names
/// a member, `token` reads a token's symbol and decimals.
pub fn action_title(
    loc: &Loc,
    action: Option<&Action>,
    nonce: &str,
    who: &dyn Fn(&str) -> String,
    token: &dyn Fn(Option<&str>) -> TokenInfo,
) -> String {
    let Some(action) = action else {
        return t(loc, "action.unsupported");
    };
    match action {
        Action::Transfer { token: t_, amount, .. } => {
            let info = token(t_.as_deref());
            tv(
                loc,
                "action.send",
                &[
                    ("amount", &units(amount, info.decimals)),
                    ("symbol", &info.symbol),
                ],
            )
        }
        Action::Batch { calls } => {
            let all = calls.iter().all(|c| matches!(c.action, Action::Transfer { .. }));
            tv(
                loc,
                if all {
                    "action.batch"
                } else {
                    "action.batchCalls"
                },
                &[("total", &calls.len().to_string())],
            )
        }
        Action::Cancel => tv(loc, "action.cancel", &[("nonce", nonce)]),
        Action::AddOwner { owner, .. } => tv(loc, "change.add", &[("member", &who(owner))]),
        Action::RemoveOwner { owner, .. } => tv(loc, "change.remove", &[("member", &who(owner))]),
        Action::SwapOwner {
            old_owner,
            new_owner,
        } => tv(
            loc,
            "change.replace",
            &[("old", &who(old_owner)), ("new", &who(new_owner))],
        ),
        // r9: "Require 1 approval" — plural by the count.
        Action::ChangeThreshold { threshold } => tc(
            loc,
            "action.changeThreshold",
            threshold.parse().unwrap_or(0),
        ),
        Action::Rename { name } => tv(loc, "action.rename", &[("name", name)]),
        Action::Call { value, .. } => {
            let zero = vela_core::app::multisig_shell::quantity(value).is_none_or(|v| v == 0);
            if zero {
                return t(loc, "action.call");
            }
            let info = token(None);
            tv(
                loc,
                "action.callWithValue",
                &[
                    ("amount", &units(value, info.decimals)),
                    ("symbol", &info.symbol),
                ],
            )
        }
    }
}

/// The short EIP-55 form of an address (`0x1234…cdef`).
#[must_use]
pub fn short(address: &str) -> String {
    let full = vela_core::multisig_link::eip55(address);
    if full.len() <= 14 {
        return full;
    }
    format!("{}…{}", &full[..6], &full[full.len() - 4..])
}

/// The token's facts on `chain_id`: the chain's coin for `None`, else the
/// contract's own symbol and decimals, read once (BLOCKS on a contract).
#[must_use]
pub fn token_info(chain_id: u64, token: Option<&str>) -> TokenInfo {
    let chain = u32::try_from(chain_id).unwrap_or_default();
    match token {
        None => TokenInfo {
            symbol: crate::flows::live::native_symbol(chain),
            decimals: 18,
        },
        Some(address) => crate::executor::send::token_metadata(chain, address).map_or(
            TokenInfo {
                symbol: String::new(),
                decimals: 18,
            },
            |meta| TokenInfo {
                symbol: meta.symbol,
                decimals: meta.decimals,
            },
        ),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn coin(_: Option<&str>) -> TokenInfo {
        TokenInfo {
            symbol: "xDAI".to_owned(),
            decimals: 18,
        }
    }

    #[test]
    fn a_proposal_is_titled_as_the_web_titles_it() {
        let loc = Loc::for_tag("en");
        let who = |a: &str| format!("<{a}>");
        let title = |action: Option<&Action>| action_title(&loc, action, "4", &who, &coin);
        assert_eq!(
            title(Some(&Action::Transfer {
                token: None,
                to: "0x1".to_owned(),
                amount: "1500000000000000000".to_owned()
            })),
            "Send 1.5 xDAI"
        );
        assert_eq!(title(Some(&Action::Cancel)), "Cancel transaction #4");
        assert_eq!(
            title(Some(&Action::AddOwner {
                owner: "0xab".to_owned(),
                threshold: "2".to_owned()
            })),
            "Add <0xab>"
        );
        assert_eq!(
            title(Some(&Action::ChangeThreshold {
                threshold: "3".to_owned()
            })),
            "Require 3 approvals"
        );
        assert_eq!(title(None), "A transaction Vela doesn't sign");
        assert_eq!(
            title(Some(&Action::Call {
                to: "0x1".to_owned(),
                value: "0".to_owned(),
                data: "0x".to_owned()
            })),
            "Contract call"
        );
    }

    #[test]
    fn the_rule_says_a_multisig_of_one_as_one() {
        let loc = Loc::for_tag("en");
        assert_eq!(rule_title(&loc, 2, 3), "Needs 2 of 3 members");
        assert_eq!(rule_chip(&loc, 2, 3), "Needs 2 of 3");
        assert_eq!(rule_chip(&loc, 1, 1), "One member");
        let zh = Loc::for_tag("zh");
        assert!(!rule_chip(&zh, 2, 3).contains("home.multisig"));
    }

    #[test]
    fn plural_presence_takes_the_languages_form() {
        let loc = Loc::for_tag("en");
        assert_eq!(tc(&loc, "presence.needsYou", 1), "1 needs your approval");
        assert_eq!(tc(&loc, "presence.needsYou", 2), "2 need your approval");
    }
}
