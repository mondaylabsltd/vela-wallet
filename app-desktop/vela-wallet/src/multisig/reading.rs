//! What a multisig transaction's calls do, as the core read them
//! (`MultisigReview.reading`, r8-1, r8-2, r8-5, r9-1): every call and every
//! leg of every batch at every level, each with its intent (a send, an
//! allowance and whether it is unlimited, a whole collection, a call nobody
//! reads, a batch too deep to read), its amounts in the decimals the core
//! read or none ("decimals unknown": never worded in 18 assumed) — and the
//! lines the core's concerns about them say, each once.
//!
//! The web's `core/reading-blocks.ts`, word for word, gpui-free so it is
//! tested here against the core's own vectors. Nothing here decodes a byte:
//! the core decided what each call is (the owner's Bybit lesson).

use serde::Deserialize;
use serde_json::Value;
use vela_core::app::multisig_outside::reading::TokenFacts;
use vela_core::multisig_review::Concern;
use vela_core::multisig_review::reading::{Amount, Call, Grant, Intent, Reading};

use super::words;
use crate::loc::Loc;

/// How loud a block is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Caution,
    Danger,
}

/// One fact of a call: its label, its value, whether the value is an
/// address (drawn in the mono face), and the value's own tone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    pub label: String,
    pub value: String,
    pub mono: bool,
    pub tone: Tone,
}

impl Row {
    fn plain(label: String, value: String) -> Self {
        Self {
            label,
            value,
            mono: false,
            tone: Tone::Neutral,
        }
    }

    fn address(label: String, address: &str) -> Self {
        Self {
            label,
            value: vela_core::multisig_link::eip55(address),
            mono: true,
            tone: Tone::Neutral,
        }
    }

    fn toned(mut self, tone: Tone) -> Self {
        self.tone = tone;
        self
    }
}

/// What the sheet draws of the reading, in the signing sheet's own kinds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Block {
    /// The lone call's verb ("Send", "Approve").
    Intent { text: String, tone: Tone },
    /// Facts in a column.
    Rows(Vec<Row>),
    /// Who it reaches, its address whole.
    Party { label: String, address: String },
    /// One leg of a batch ("Call 2.1 · Send") and its facts.
    Card {
        title: String,
        rows: Vec<Row>,
        tone: Tone,
    },
    /// A line about the card above (a batch's runner).
    Source { text: String, tone: Tone },
    /// A concern's line.
    Warning { text: String, tone: Tone },
}

impl Block {
    /// Every word the block says, for the tests' look at what is drawn.
    #[cfg(test)]
    #[must_use]
    pub fn text(&self) -> String {
        let rows = |rows: &[Row]| {
            rows.iter()
                .map(|r| format!("{}: {}", r.label, r.value))
                .collect::<Vec<_>>()
                .join("\n")
        };
        match self {
            Self::Intent { text, .. } | Self::Source { text, .. } | Self::Warning { text, .. } => {
                text.clone()
            }
            Self::Rows(r) => rows(r),
            Self::Party { label, address } => format!("{label}: {address}"),
            Self::Card { title, rows: r, .. } => format!("{title}\n{}", rows(r)),
        }
    }
}

/// The words a reading is said in.
pub struct Ctx<'a> {
    pub loc: &'a Loc,
    /// The network's coin, for a coin the core names by `token: null`.
    pub native_symbol: String,
}

impl Ctx<'_> {
    fn r(&self, key: &str) -> String {
        words::t(self.loc, &format!("review.reading.{key}"))
    }

    fn rv(&self, key: &str, vars: &[(&str, &str)]) -> String {
        words::tv(self.loc, &format!("review.reading.{key}"), vars)
    }
}

/// The part of the one review the reading is drawn from.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct ReadingDraw {
    pub reading: Reading,
    pub concerns: Vec<Concern>,
}

impl ReadingDraw {
    /// The review's JSON (`reviews[...]`, `draft.review`), as the core wrote
    /// it; `None` when it carries no reading.
    #[must_use]
    pub fn of(review: &Value) -> Option<Self> {
        let reading = serde_json::from_value(review.get("reading")?.clone()).ok()?;
        // A concern of a kind this build does not know is not drawn here
        // (its own block, elsewhere, says it); the known ones are kept.
        let concerns = review
            .get("concerns")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .filter_map(|c| serde_json::from_value(c.clone()).ok())
                    .collect()
            })
            .unwrap_or_default();
        Some(Self { reading, concerns })
    }
}

fn short(address: &str) -> String {
    words::short(address)
}

/// A token as said: its symbol, else its contract (never a made-up name).
#[must_use]
pub fn token_name(token: &TokenFacts, ctx: &Ctx<'_>) -> String {
    match &token.token {
        None => ctx.native_symbol.clone(),
        Some(address) => token.symbol.clone().unwrap_or_else(|| short(address)),
    }
}

/// An amount as the core read it: unlimited is "Unlimited"; unknown
/// decimals are the raw integer with "decimals unknown" — never a friendly
/// amount in 18 decimals (r8-2).
#[must_use]
pub fn amount_text(amount: &Amount, token: &TokenFacts, ctx: &Ctx<'_>) -> String {
    if amount.unlimited {
        return ctx.r("unlimited");
    }
    let name = token_name(token, ctx);
    match amount.decimals {
        None => format!(
            "{} {name}",
            ctx.rv("decimalsUnknown", &[("raw", &amount.raw)])
        )
        .trim()
        .to_owned(),
        Some(decimals) => format!("{} {name}", words::units(&amount.raw, decimals))
            .trim()
            .to_owned(),
    }
}

/// What a call is, in one or two words ("Send", "Approve", "Batch of 2 calls").
#[must_use]
pub fn intent_title(intent: &Intent, ctx: &Ctx<'_>) -> String {
    match intent {
        Intent::Send { .. } => ctx.r("send"),
        Intent::SendFrom { .. } => ctx.r("sendFrom"),
        Intent::Approve { grant, .. } => ctx.r(match grant {
            Grant::Approve => "approve",
            Grant::Increase => "increase",
            Grant::Decrease => "decrease",
            Grant::Permit => "permit",
            Grant::DaiPermit => "daiPermit",
            Grant::Permit2 => "permit2",
        }),
        Intent::ApproveAll { granted, .. } => {
            ctx.r(if *granted { "approveAll" } else { "revokeAll" })
        }
        Intent::Batch { calls, .. } if calls.len() == 1 => ctx.r("batchOne"),
        Intent::Batch { calls, .. } => ctx.rv("batchMany", &[("total", &calls.len().to_string())]),
        Intent::Unreadable => ctx.r("unreadable"),
        Intent::Members { .. } => ctx.r("members"),
        Intent::Rename { name } => ctx.rv("rename", &[("name", name)]),
        Intent::Call { .. } => ctx.r("call"),
    }
}

/// A call's tone: danger for what grants it all or cannot be read.
fn intent_tone(intent: &Intent) -> Tone {
    match intent {
        Intent::Approve { amount, grant, .. } => {
            if amount.unlimited && *grant != Grant::Decrease {
                Tone::Danger
            } else {
                Tone::Neutral
            }
        }
        Intent::ApproveAll { granted, .. } => {
            if *granted {
                Tone::Danger
            } else {
                Tone::Neutral
            }
        }
        Intent::Unreadable | Intent::Batch { .. } => Tone::Danger,
        Intent::Call { .. } => Tone::Caution,
        _ => Tone::Neutral,
    }
}

fn coin() -> TokenFacts {
    TokenFacts {
        token: None,
        symbol: None,
        decimals: Some(18),
        known: true,
    }
}

/// The rows of one call, each fact the core read.
fn call_rows(call: &Call, ctx: &Ctx<'_>) -> Vec<Row> {
    let mut rows = Vec::new();
    match &call.intent {
        Intent::Send { token, to, amount } => {
            rows.push(Row::plain(ctx.r("amount"), amount_text(amount, token, ctx)));
            rows.push(Row::address(ctx.r("to"), to));
        }
        Intent::SendFrom {
            token,
            from,
            to,
            amount,
        } => {
            rows.push(Row::plain(ctx.r("amount"), amount_text(amount, token, ctx)));
            rows.push(Row::address(ctx.r("from"), from));
            rows.push(Row::address(ctx.r("to"), to));
        }
        Intent::Approve {
            token,
            spender,
            grant,
            amount,
            ..
        } => {
            let unlimited = amount.unlimited && *grant != Grant::Decrease;
            rows.push(Row::plain(ctx.r("token"), token_name(token, ctx)));
            let value = if *grant == Grant::DaiPermit && unlimited {
                ctx.r("allOfIt")
            } else {
                amount_text(amount, token, ctx)
            };
            let row = Row::plain(ctx.r("allowance"), value);
            rows.push(if unlimited {
                row.toned(Tone::Danger)
            } else {
                row
            });
            rows.push(Row::address(ctx.r("spender"), spender));
        }
        Intent::ApproveAll {
            collection,
            operator,
            granted,
        } => {
            rows.push(Row::address(ctx.r("collection"), collection));
            let row = Row::address(ctx.r("operator"), operator);
            rows.push(if *granted {
                row.toned(Tone::Danger)
            } else {
                row
            });
        }
        Intent::Batch { runner, .. } => rows.push(Row::address(ctx.r("runner"), runner)),
        Intent::Call {
            selector: Some(selector),
        } => rows.push(Row {
            label: ctx.r("function"),
            value: selector.clone(),
            mono: true,
            tone: Tone::Neutral,
        }),
        _ => {}
    }
    let sends = !matches!(call.intent, Intent::Send { .. } | Intent::ApproveAll { .. });
    if sends && call.value.raw != "0" {
        rows.push(Row::plain(
            ctx.r("alsoSends"),
            amount_text(&call.value, &coin(), ctx),
        ));
    }
    rows
}

/// A leg's number, the core's (`Call.leg`, r10-10): "Call 2.1"; `None` for
/// the transaction's lone call — no number, never one joined here.
fn leg_label(call: &Call, ctx: &Ctx<'_>) -> Option<String> {
    let leg = call.leg.as_deref()?;
    Some(words::tv(ctx.loc, "review.callNumber", &[("n", leg)]))
}

/// Each call as a card, a batch's legs after it, every level (r8-5).
fn cards(calls: &[Call], ctx: &Ctx<'_>) -> Vec<Block> {
    let mut out = Vec::new();
    for call in calls {
        let intent = intent_title(&call.intent, ctx);
        out.push(Block::Card {
            title: match leg_label(call, ctx) {
                Some(leg) => format!("{leg} · {intent}"),
                None => intent,
            },
            rows: call_rows(call, ctx),
            tone: intent_tone(&call.intent),
        });
        if let Intent::Batch { calls, .. } = &call.intent {
            out.push(Block::Source {
                text: ctx.r("runnerNote"),
                tone: Tone::Caution,
            });
            out.extend(cards(calls, ctx));
        }
    }
    out
}

/// The reading's blocks: the lone call as the signing sheet draws one (its
/// verb, its amount or allowance, who it reaches), or every call of a batch
/// as a card. Nothing for what the multisig reads as its own (a member
/// change, a rename, a cancellation): those have their own drawing.
#[must_use]
pub fn blocks(reading: &Reading, ctx: &Ctx<'_>) -> Vec<Block> {
    let calls = &reading.calls;
    let Some(only) = calls.first() else {
        return Vec::new();
    };
    let lone =
        calls.len() == 1 && only.path.is_empty() && !matches!(only.intent, Intent::Batch { .. });
    if !lone {
        // A transaction whose one call is itself a batch: its legs, as a batch's.
        return cards(calls, ctx);
    }
    let i = &only.intent;
    let mut out = vec![Block::Intent {
        text: intent_title(i, ctx),
        tone: intent_tone(i),
    }];
    let party = |label: &str, address: &str| Block::Party {
        label: ctx.r(label),
        address: vela_core::multisig_link::eip55(address),
    };
    match i {
        Intent::Send { token, to, amount } => {
            out.push(Block::Rows(vec![Row::plain(
                ctx.r("amount"),
                amount_text(amount, token, ctx),
            )]));
            out.push(party("to", to));
        }
        Intent::Approve { spender, .. } => {
            out.push(Block::Rows(
                call_rows(only, ctx)
                    .into_iter()
                    .filter(|r| !r.mono)
                    .collect(),
            ));
            out.push(party("spender", spender));
        }
        Intent::SendFrom { from, to, .. } => {
            out.push(Block::Rows(
                call_rows(only, ctx)
                    .into_iter()
                    .filter(|r| !r.mono)
                    .collect(),
            ));
            out.push(party("from", from));
            out.push(party("to", to));
        }
        _ => {
            let rows = call_rows(only, ctx);
            if !rows.is_empty() {
                out.push(Block::Rows(rows));
            }
        }
    }
    out
}

/// The addresses the reading names as a party already (lowercased): the
/// review's recipient rows do not say them again (r5reg-15).
#[must_use]
pub fn party_addresses(blocks: &[Block]) -> Vec<String> {
    blocks
        .iter()
        .filter_map(|b| match b {
            Block::Party { address, .. } => Some(address.to_ascii_lowercase()),
            _ => None,
        })
        .collect()
}

/// The reading's title for the sheet's header, or `None` when it has none
/// (the multisig's own: a member change, a rename, a cancellation).
#[must_use]
pub fn title(reading: &Reading, ctx: &Ctx<'_>) -> Option<String> {
    let calls = &reading.calls;
    let first = calls.first()?;
    if calls.len() == 1 && first.path.is_empty() {
        return Some(match &first.intent {
            Intent::Send { token, amount, .. } => {
                format!("{} {}", ctx.r("send"), amount_text(amount, token, ctx))
            }
            i @ Intent::Approve { token, .. } => {
                format!("{} · {}", intent_title(i, ctx), token_name(token, ctx))
            }
            i => intent_title(i, ctx),
        });
    }
    Some(if calls.len() == 1 {
        ctx.r("batchOne")
    } else {
        ctx.rv("batchMany", &[("total", &calls.len().to_string())])
    })
}

/// Whether the reading says more than "a call nobody reads": its title
/// stands in for "Contract call" on a row (r8-1, the web's row rule).
#[must_use]
pub fn says_more_than_a_call(reading: &Reading) -> bool {
    reading
        .calls
        .first()
        .is_some_and(|c| !matches!(c.intent, Intent::Call { .. }))
}

fn find_token<'a>(calls: &'a [Call], address: &str) -> Option<&'a TokenFacts> {
    for call in calls {
        match &call.intent {
            Intent::Send { token, .. }
            | Intent::SendFrom { token, .. }
            | Intent::Approve { token, .. }
                if token
                    .token
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(address)) =>
            {
                return Some(token);
            }
            Intent::Batch { calls, .. } => {
                if let Some(found) = find_token(calls, address) {
                    return Some(found);
                }
            }
            _ => {}
        }
    }
    None
}

/// What one of the reading's concerns says, and how loud; `None` for the
/// review's other concerns (the state guard, recipients, simulation), which
/// have their own blocks.
#[must_use]
pub fn concern_line(concern: &Concern, reading: &Reading, ctx: &Ctx<'_>) -> Option<(String, Tone)> {
    let named = |address: &str| {
        find_token(&reading.calls, address)
            .and_then(|t| t.symbol.clone())
            .unwrap_or_else(|| short(address))
    };
    Some(match concern {
        Concern::UnlimitedApproval { spender, .. } => (
            ctx.rv(
                "unlimitedLine",
                &[("spender", &vela_core::multisig_link::eip55(spender))],
            ),
            Tone::Danger,
        ),
        Concern::NestedBatch => (ctx.r("nestedLine"), Tone::Danger),
        Concern::UnreadableBatch => (ctx.r("unreadableLine"), Tone::Danger),
        Concern::DecimalsUnknown { token } => (
            ctx.rv("decimalsLine", &[("token", &named(token))]),
            Tone::Caution,
        ),
        Concern::ToOwnToken { token } => (
            ctx.rv("ownTokenLine", &[("token", &named(token))]),
            Tone::Danger,
        ),
        Concern::Blind => (ctx.r("blindLine"), Tone::Caution),
        _ => return None,
    })
}

/// The lines the core's concerns about the calls say (r8-1, r8-2, r8-5),
/// each once (the core deduped them, r8-14): an unlimited grant, a nested
/// batch, one that cannot be read, decimals unknown, tokens sent to their
/// own contract, a call nobody reads. Beside a refusal the core has already
/// left out the concern the refusal says (a batch too deep is refused, and
/// its "can't be read" is not a second red box — `multisig_review`), so
/// every concern here is drawn.
#[must_use]
pub fn concern_blocks(draw: &ReadingDraw, ctx: &Ctx<'_>) -> Vec<Block> {
    let mut out: Vec<Block> = Vec::new();
    for c in &draw.concerns {
        if let Some((text, tone)) = concern_line(c, &draw.reading, ctx)
            && !out
                .iter()
                .any(|b| matches!(b, Block::Warning { text: t, .. } if *t == text))
        {
            out.push(Block::Warning { text, tone });
        }
    }
    out
}

/// The concern kinds these lines say — the sheet says none of them again.
#[cfg(test)]
#[must_use]
pub fn is_reading_concern(concern: &Concern) -> bool {
    matches!(
        concern,
        Concern::UnlimitedApproval { .. }
            | Concern::NestedBatch
            | Concern::UnreadableBatch
            | Concern::DecimalsUnknown { .. }
            | Concern::ToOwnToken { .. }
            | Concern::Blind
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const VECTORS: &str =
        include_str!("../../../../rust/crates/vela-core/tests/vectors/multisig/reading.json");

    fn ctx(loc: &Loc) -> Ctx<'_> {
        Ctx {
            loc,
            native_symbol: "USDC".to_owned(),
        }
    }

    /// Every case of the core's vectors, read through the core (as the
    /// proposals machine reads a review) and drawn as the sheet draws it:
    /// every leg at every level, the concern lines, the title — no raw key,
    /// no unfilled variable, and never "Contract call" for a call the core
    /// read as something it can say.
    #[test]
    fn every_vector_is_drawn_from_the_cores_reading() {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let cases = file["cases"].as_array().unwrap();
        assert!(cases.len() >= 20, "the vectors are all here");
        for tag in ["en", "zh"] {
            let loc = Loc::for_tag(tag);
            let ctx = ctx(&loc);
            let call_words = ctx.r("call");
            for case in cases {
                let name = case["name"].as_str().unwrap();
                let args = &case["args"];
                let tx = serde_json::from_value(args["tx"].clone()).unwrap();
                let facts: Vec<TokenFacts> = args
                    .get("tokenFacts")
                    .map(|f| serde_json::from_value(f.clone()).unwrap())
                    .unwrap_or_default();
                let read = vela_core::multisig_rules::review::reading_of(
                    args["chainId"].as_u64().unwrap(),
                    args["safe"].as_str().unwrap(),
                    &tx,
                    &facts,
                )
                .unwrap();
                assert_eq!(
                    read["reading"], case["expect"]["reading"],
                    "{name}: the core's reading"
                );
                let draw = ReadingDraw::of(&read).unwrap();
                let drawn: Vec<Block> = concern_blocks(&draw, &ctx)
                    .into_iter()
                    .chain(blocks(&draw.reading, &ctx))
                    .collect();
                let text: String = drawn
                    .iter()
                    .map(Block::text)
                    .chain(title(&draw.reading, &ctx))
                    .collect::<Vec<_>>()
                    .join("\n");
                assert!(
                    !text.contains("home.multisig"),
                    "{tag} {name}: a raw key in\n{text}"
                );
                assert!(
                    !text.contains("{{"),
                    "{tag} {name}: an unfilled variable in\n{text}"
                );

                // Every call the core read is drawn, at every level.
                let every = draw.reading.every();
                let cards = drawn
                    .iter()
                    .filter(|b| matches!(b, Block::Card { .. }))
                    .count();
                if every.len() > 1 || every.first().is_some_and(|c| !c.path.is_empty()) {
                    assert_eq!(cards, every.len(), "{tag} {name}: a card per call\n{text}");
                    for call in &every {
                        // r10-10: the core's leg, else no number at all.
                        match call.leg.as_deref() {
                            Some(leg) => {
                                let label = words::tv(&loc, "review.callNumber", &[("n", leg)]);
                                assert!(
                                    text.contains(&label),
                                    "{tag} {name}: {label} drawn\n{text}"
                                );
                            }
                            None => assert!(
                                !text.contains(&format!(
                                    "{} ·",
                                    words::tv(&loc, "review.callNumber", &[("n", "")])
                                )),
                                "{tag} {name}: an empty leg number\n{text}"
                            ),
                        }
                    }
                }
                // "Contract call" only where the core read no more.
                let blind = every
                    .iter()
                    .filter(|c| matches!(c.intent, Intent::Call { .. }))
                    .count();
                if blind == 0 {
                    assert!(
                        !text.contains(&call_words),
                        "{tag} {name}: said \"{call_words}\"\n{text}"
                    );
                }
                // Each concern the core raised about the calls is said once.
                for c in draw.concerns.iter().filter(|c| is_reading_concern(c)) {
                    let (line, _) = concern_line(c, &draw.reading, &ctx).unwrap();
                    assert_eq!(text.matches(&line).count(), 1, "{tag} {name}: {line}");
                }
                // Decimals never assumed: an amount the core could not size
                // is its raw integer, said so — never a friendly one.
                for call in &every {
                    let amount = match &call.intent {
                        Intent::Send { amount, .. }
                        | Intent::SendFrom { amount, .. }
                        | Intent::Approve { amount, .. } => amount,
                        _ => continue,
                    };
                    if amount.decimals.is_none() && !amount.unlimited {
                        let raw = ctx.rv("decimalsUnknown", &[("raw", &amount.raw)]);
                        assert!(text.contains(&raw), "{tag} {name}: {raw}\n{text}");
                        assert!(!text.contains("<0.000001"), "{tag} {name}\n{text}");
                    }
                }
                if draw.reading.unlimited {
                    assert!(
                        text.contains(&ctx.r("unlimited"))
                            || text.contains(&ctx.r("allOfIt"))
                            || every.iter().any(|c| matches!(
                                c.intent,
                                Intent::ApproveAll { granted: true, .. }
                            )),
                        "{tag} {name}: unlimited said\n{text}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_cases_the_round_found_read_as_the_web_reads_them() {
        let loc = Loc::for_tag("en");
        let ctx = ctx(&loc);
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let case = |needle: &str| -> ReadingDraw {
            let c = file["cases"]
                .as_array()
                .unwrap()
                .iter()
                .find(|c| c["name"].as_str().unwrap().contains(needle))
                .unwrap();
            ReadingDraw::of(&serde_json::json!({
                "reading": c["expect"]["reading"],
                "concerns": c["expect"]["concerns"],
            }))
            .unwrap()
        };
        let all = |d: &ReadingDraw| {
            concern_blocks(d, &ctx)
                .iter()
                .chain(&blocks(&d.reading, &ctx))
                .map(Block::text)
                .collect::<Vec<_>>()
                .join("\n")
        };

        // r8-2: 0.01 USDC from the registry.
        let usdc = case("Arc's USDC ERC-20 transfer");
        assert_eq!(title(&usdc.reading, &ctx).unwrap(), "Send 0.01 USDC");

        // r8-2: a token nobody read: raw, decimals unknown, never "<0.000001".
        let nodec = case("a token nobody read");
        let text = all(&nodec);
        assert!(
            text.contains("10000 base units · decimals unknown"),
            "{text}"
        );
        assert!(!text.contains("<0.000001"), "{text}");
        assert!(text.contains("didn't say how many decimals"), "{text}");

        // r8-1: an unlimited approve, with its spender and the danger line.
        let approve = case("approve(spender, 2^256-1) of Arc's USDC");
        let text = all(&approve);
        assert_eq!(title(&approve.reading, &ctx).unwrap(), "Approve · USDC");
        assert!(text.contains("Allowance: Unlimited"), "{text}");
        assert!(text.contains("Unlimited allowance:"), "{text}");
        assert!(
            blocks(&approve.reading, &ctx)
                .iter()
                .any(|b| matches!(b, Block::Party { .. }))
        );

        // r8-5: a nested batch: every leg, the inner ones too.
        let nested = case("whose leg calls multiSend");
        let text = all(&nested);
        for n in ["Call 1 ·", "Call 1.1 ·", "Call 1.2 ·"] {
            assert!(text.contains(n), "{n} in\n{text}");
        }
        assert!(text.contains("A batch inside a batch"), "{text}");
        assert!(text.contains("run from this contract"), "{text}");

        // Four deep: the reading alone says it can't be read. (Beside the
        // review's too-deep refusal the core leaves that concern out — one
        // refusal box, `multisig_review` — so the sheet draws what it gets.)
        let deep = case("four deep");
        assert!(all(&deep).contains("can't be read"), "{}", all(&deep));
    }

    /// r15-10: an address row (the deep4 sheet's "To", a spender) holds a
    /// full address, and the sheet draws it in the one rule's two halves —
    /// 22 | 20, never its last character alone on a line.
    #[test]
    fn an_address_rows_value_is_drawn_in_its_two_halves() {
        let row = Row::address(
            "To".to_owned(),
            "0x777e989c821a7f1b031696f044a2ff114261875a",
        );
        assert!(row.mono);
        let (head, tail) = crate::ui::address_halves(&row.value).expect("split in two");
        assert_eq!((head.len(), tail.len()), (22, 20));
        assert_eq!(format!("{head}{tail}"), row.value);
    }
}
