//! The multisig screens' words (`home.multisig.*` in the vela-core corpus,
//! the on-demand catalog `loc.rs` merges): what the core decided, said. The
//! web's `core/display.ts` wording, nothing decided here — an amount is the
//! core's base units in the token's decimals (`multisig_link::format_units`),
//! a member is named by the core's `names` rule.

use vela_core::multisig::Action;

use crate::loc::Loc;

/// A token's symbol and decimals, as read for wording (`tokenInfo`).
/// `decimals` `None`: nobody said (r8-2) — an amount in it is its raw
/// integer, "decimals unknown", never worded in 18 assumed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TokenInfo {
    pub symbol: String,
    pub decimals: Option<u32>,
}

impl TokenInfo {
    /// The network's coin: 18 decimals, its symbol.
    #[must_use]
    pub fn coin(symbol: String) -> Self {
        Self {
            symbol,
            decimals: Some(18),
        }
    }
}

/// `amount` (base units) in `info`, as a title says it: "1.5 USDC", or the
/// raw integer with "decimals unknown" when nobody said (r8-2).
#[must_use]
pub fn amount_words(loc: &Loc, amount: &str, info: &TokenInfo) -> String {
    match info.decimals {
        Some(decimals) => format!("{} {}", units(amount, decimals), info.symbol),
        None => format!(
            "{} {}",
            tv(loc, "review.reading.decimalsUnknown", &[("raw", amount)]),
            info.symbol
        ),
    }
    .trim()
    .to_owned()
}

/// r21-2 (§36): a FEE of `amount` (base units) in `info` — the core's fee
/// figure (`fee_policy::fee_figure`), rounded UP at the native coin's six
/// places so the line never reads below what the execution is charged.
/// Decimals nobody said: [`amount_words`]'s raw integer.
#[must_use]
pub fn fee_words(loc: &Loc, amount: &str, info: &TokenInfo) -> String {
    match info.decimals {
        Some(decimals) => format!(
            "{} {}",
            crate::flows::live::fee_figure_text(
                amount,
                decimals,
                crate::flows::live::FEE_NATIVE_PLACES
            ),
            info.symbol
        )
        .trim()
        .to_owned(),
        None => amount_words(loc, amount, info),
    }
}

/// A `MultisigAmount` (r12-1) as its `said` words: `known` its `text`; a
/// token whose decimals nobody could read, the corpus `key` with `{{raw}}`.
/// `None` for a value with no `said` (an older core's base units).
#[must_use]
pub fn said_amount(loc: &Loc, amount: &serde_json::Value) -> Option<String> {
    let said = amount.get("said")?;
    match said.get("kind").and_then(serde_json::Value::as_str)? {
        "known" => said
            .get("text")
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned),
        "decimals_unknown" => {
            let key = said
                .get("key")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("home.multisig.review.reading.decimalsUnknown");
            let raw = said
                .get("raw")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let text = loc.t_texts(key, &[("raw", raw)]).to_string();
            (text != key).then_some(text)
        }
        _ => None,
    }
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
        Action::Transfer {
            token: t_, amount, ..
        } => {
            let info = token(t_.as_deref());
            match info.decimals {
                Some(decimals) => tv(
                    loc,
                    "action.send",
                    &[
                        ("amount", &units(amount, decimals)),
                        ("symbol", &info.symbol),
                    ],
                ),
                // r8-2: never "Send <0.000001" in 18 assumed.
                None => format!(
                    "{} {}",
                    t(loc, "review.reading.send"),
                    amount_words(loc, amount, &info)
                ),
            }
        }
        Action::Batch { calls } => {
            let all = calls
                .iter()
                .all(|c| matches!(c.action, Action::Transfer { .. }));
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
                    ("amount", &units(value, info.decimals.unwrap_or(18))),
                    ("symbol", &info.symbol),
                ],
            )
        }
    }
}

/// The short EIP-55 form of an address — the core's one short form
/// (`names::short_address`, r11-6: `0x88cCA0…266894`, 6+6, the same on
/// every platform).
#[must_use]
pub fn short(address: &str) -> String {
    vela_core::multisig_rules::names::short_address(address)
}

/// A member named in a title (r11-6, the core's `names::who`): "Interleave
/// (0x88cCA0…266894)", or the short form alone when nothing names it —
/// `label` is the accounts view's `member_labels` entry.
#[must_use]
pub fn who(address: &str, label: Option<&str>) -> String {
    vela_core::multisig_rules::names::who(address, label)
}

/// The token facts the proposals machine keeps (`view.tokens`).
pub type ViewTokens =
    std::collections::BTreeMap<String, vela_core::app::multisig_outside::reading::TokenFacts>;

/// The token's facts on `chain_id` as the proposals machine read them
/// (`view.tokens`, `<chainId>:<token lowercase>`: the registry's, else the
/// contract's own answer — r8-2), else the wallet's registry; the coin for
/// `None`. Reads nothing: a token nobody read yet has no decimals and is
/// named by its contract — never a symbol or 18 decimals made up.
#[must_use]
pub fn token_facts(tokens: &ViewTokens, chain_id: u64, token: Option<&str>) -> TokenInfo {
    let chain = u32::try_from(chain_id).unwrap_or_default();
    let Some(address) = token else {
        return TokenInfo::coin(crate::flows::live::native_symbol(chain));
    };
    let key = format!("{chain_id}:{}", address.to_ascii_lowercase());
    let facts = tokens.get(&key).cloned().unwrap_or_else(|| {
        vela_core::app::multisig_outside::reading::TokenFacts::of(
            chain_id,
            Some(address),
            &std::collections::BTreeMap::new(),
        )
    });
    TokenInfo {
        symbol: facts.symbol.unwrap_or_else(|| short(address)),
        decimals: facts.decimals,
    }
}

/// The token's facts on `chain_id`: the chain's coin for `None`, else the
/// wallet's registry, else the contract's own symbol and decimals, read once
/// (BLOCKS on a contract: never from a render). A contract that does not
/// answer leaves the decimals unknown (r8-2), never 18.
#[must_use]
pub fn token_info(chain_id: u64, token: Option<&str>) -> TokenInfo {
    let chain = u32::try_from(chain_id).unwrap_or_default();
    let Some(address) = token else {
        return TokenInfo::coin(crate::flows::live::native_symbol(chain));
    };
    let registry = vela_core::app::multisig_outside::reading::TokenFacts::of(
        chain_id,
        Some(address),
        &std::collections::BTreeMap::new(),
    );
    if registry.known {
        return TokenInfo {
            symbol: registry.symbol.unwrap_or_else(|| short(address)),
            decimals: registry.decimals,
        };
    }
    crate::executor::send::token_metadata(chain, address).map_or(
        TokenInfo {
            symbol: short(address),
            decimals: None,
        },
        |meta| TokenInfo {
            symbol: meta.symbol,
            decimals: Some(meta.decimals),
        },
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn coin(_: Option<&str>) -> TokenInfo {
        TokenInfo::coin("xDAI".to_owned())
    }

    /// r8-2: a token nobody sized is said raw, never "<0.000001" in 18; the
    /// proposals view's facts word it once read.
    #[test]
    fn a_token_without_decimals_is_said_raw() {
        let loc = Loc::for_tag("en");
        let who = |a: &str| a.to_owned();
        let unknown = |_: Option<&str>| TokenInfo {
            symbol: "0x0000…0b0b".to_owned(),
            decimals: None,
        };
        let title = action_title(
            &loc,
            Some(&Action::Transfer {
                token: Some("0x00000000000000000000000000000000000b0b0b".to_owned()),
                to: "0x1".to_owned(),
                amount: "1000000".to_owned(),
            }),
            "1",
            &who,
            &unknown,
        );
        assert_eq!(
            title,
            "Send 1000000 base units · decimals unknown 0x0000…0b0b"
        );

        let mut tokens = ViewTokens::new();
        tokens.insert(
            "5042002:0x00000000000000000000000000000000000b0b0b".to_owned(),
            vela_core::app::multisig_outside::reading::TokenFacts {
                token: Some("0x00000000000000000000000000000000000b0b0b".to_owned()),
                symbol: Some("BOB".to_owned()),
                decimals: Some(6),
                known: false,
            },
        );
        let bob = token_facts(
            &tokens,
            5_042_002,
            Some("0x00000000000000000000000000000000000B0B0B"),
        );
        assert_eq!(
            bob,
            TokenInfo {
                symbol: "BOB".to_owned(),
                decimals: Some(6)
            }
        );
        // Arc's USDC: the registry's 6, never 18.
        let usdc = token_facts(
            &tokens,
            5_042_002,
            Some("0x3600000000000000000000000000000000000000"),
        );
        assert_eq!(usdc.decimals, Some(6));
        let nobody = token_facts(
            &tokens,
            5_042_002,
            Some("0x00000000000000000000000000000000000c0c0c"),
        );
        assert_eq!(nobody.decimals, None);
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
