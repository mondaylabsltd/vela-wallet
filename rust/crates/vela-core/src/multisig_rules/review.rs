//! What a multisig transaction's calls do (r8-1, r8-2, r8-5), the part of
//! the one review that is read from the bytes alone: `review.reading
//! {chainId, safe, tx, tokenFacts?}` answers `{reading, concerns, anyway,
//! refused}` — the same [`crate::multisig_review::reading`] every review
//! carries, with what it alone says and how hard the confirm is. The shared
//! vectors (`tests/vectors/multisig/reading.json`) replay it on every
//! platform.

use std::collections::BTreeMap;

use serde_json::{json, Value};

use super::{arg, to_json};
use crate::app::multisig_outside::reading::TokenFacts;
use crate::multisig::{self, SafeTx};
use crate::multisig_review::{reading, Concern};
use crate::CoreError;

/// The reading of `tx` and what it alone says.
pub fn reading_of(
    chain_id: u64,
    safe: &str,
    tx: &SafeTx,
    token_facts: &[TokenFacts],
) -> Result<Value, CoreError> {
    let review = multisig::review(chain_id, safe, tx)?;
    let tokens: BTreeMap<String, TokenFacts> = token_facts
        .iter()
        .filter_map(|t| Some((t.token.clone()?.to_ascii_lowercase(), t.clone())))
        .collect();
    let read = reading::read(chain_id, tx, &review, &tokens);
    let concerns = reading::concerns(&read);
    let refused = review.refusal.is_some() || concerns.contains(&Concern::UnreadableBatch);
    let anyway = !refused
        && concerns.iter().any(|c| {
            matches!(
                c,
                Concern::UnlimitedApproval { .. }
                    | Concern::NestedBatch
                    | Concern::ToOwnToken { .. }
                    | Concern::Blind
            )
        });
    Ok(json!({
        "reading": read,
        "concerns": concerns,
        "anyway": anyway,
        "refused": refused,
    }))
}

/// `review.reading`.
pub fn call(method: &str, args: &Value) -> Result<String, CoreError> {
    match method {
        "reading" => {
            let chain_id: u64 = arg(args, "chainId")?;
            let safe: String = arg(args, "safe")?;
            let tx: SafeTx = arg(args, "tx")?;
            let facts: Option<Vec<TokenFacts>> = arg(args, "tokenFacts")?;
            to_json(&reading_of(
                chain_id,
                &safe,
                &tx,
                &facts.unwrap_or_default(),
            )?)
        }
        other => Err(CoreError::Multisig(format!("review.{other}: no such rule"))),
    }
}
