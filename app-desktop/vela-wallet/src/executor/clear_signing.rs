//! What a transaction actually does — the reads behind the six-rung ladder.
//!
//! `clear_signing` decides; this file only fetches, calls, waits and reports.
//! The division matters more here than anywhere else in the app, because every
//! rung of the ERC-7730 degradation ladder is a JUDGMENT about how much is
//! known, and a shell that pre-judged would collapse rungs silently:
//!
//! - a descriptor that 404s is not a descriptor that failed to parse
//! - an `eth_call` that REVERTED is not one that could not be reached (the
//!   first says "this is not an ERC-721", the second says "we could not ask")
//! - a selector nothing recognises is not a selector we forgot to look up
//!
//! So the answers go back raw: body-or-`None`, result-or-`None` plus a
//! separate `rpc_error` flag, `[]` for no candidates. The core reads those and
//! picks the rung.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use serde_json::Value;

use crate::executor::{chain_tokens, pool, proxy};
use crate::resident::{Answer, Machine};

use vela_core::app::clear_signing::{ClearOperation, ClearShellResult, ClearSigning, Event};

/// `NET_TIMEOUTS.descriptor` — the web's 5s budget for a descriptor.
const DESCRIPTOR_TIMEOUT: Duration = Duration::from_secs(5);
/// The selector databases' own budget (`selector-registry.ts`).
const SELECTOR_TIMEOUT: Duration = Duration::from_secs(6);

impl Machine for ClearSigning {
    const LABEL: &'static str = "clear_signing";

    /// Resets to exactly the state it is already in: `Cleared` sets the run,
    /// the result and the message to `None`, which is what a pristine model
    /// has, and bumps the generation counter — which orphans nothing, because
    /// nothing is in flight. The machine starts working on its first
    /// `ResolveTransaction`, dispatched by the sheet; this is the `fee_policy`
    /// shape, for the same reason (owned by a screen, not by the process).
    fn boot_event(_cx: &gpui::App) -> Event {
        Event::Cleared
    }

    fn perform(operation: &ClearOperation) -> Answer<ClearShellResult, Self::Event> {
        match operation {
            ClearOperation::HttpGet { path } => {
                let path = path.clone();
                Answer::Blocking(Box::new(move || ClearShellResult::DescriptorFetched {
                    json: descriptor(&path),
                    path,
                }))
            }

            ClearOperation::RpcEthCall {
                chain_id,
                to,
                data,
                probe,
            } => {
                let (chain_id, to, data, probe) = (*chain_id, to.clone(), data.clone(), *probe);
                Answer::Blocking(Box::new(move || {
                    let (result, rpc_error) = eth_call(chain_id, &to, &data);
                    ClearShellResult::RpcAnswer {
                        probe,
                        chain_id,
                        to,
                        result,
                        rpc_error,
                    }
                }))
            }

            ClearOperation::SelectorDbLookup { selector } => {
                let selector = selector.clone();
                Answer::Blocking(Box::new(move || ClearShellResult::SelectorCandidates {
                    sigs: lookup_selector(&selector),
                }))
            }

            // gpui's timer rather than a parked thread: the ladder arms one of
            // these per probe, and a run that asks three questions would cost
            // three threads doing nothing.
            ClearOperation::Timer { ms, token } => Answer::After(
                Duration::from_millis(u64::from(*ms)),
                ClearShellResult::TimedOut { token: *token },
            ),

            ClearOperation::Now => Answer::Now(ClearShellResult::Clock {
                now_ms: crate::executor::now_ms(),
            }),
        }
    }
}

/// One ERC-7730 descriptor. Body on 200; `None` for every other outcome —
/// a 404, a timeout and a dead network are one answer here because they are
/// one answer to the core: no descriptor, take the next rung.
fn descriptor(path: &str) -> Option<String> {
    let url = format!("{}{path}", chain_tokens::data_base());
    proxy::agent(DESCRIPTOR_TIMEOUT)
        .get(&url)
        .header("accept", "application/json")
        .call()
        .ok()
        .filter(|response| response.status().as_u16() == 200)
        .and_then(|mut response| response.body_mut().read_to_string().ok())
}

/// A routed `eth_call`, with the one distinction the core cannot re-derive.
///
/// `(Some(result), false)` — it answered.
/// `(None, true)` — it REVERTED. For an ERC-165 probe that is a real answer:
/// this contract is not that interface.
/// `(None, false)` — we could not ask. Which is not the same thing, and the
/// ladder drops a rung on one and not the other.
fn eth_call(chain_id: u32, to: &str, data: &str) -> (Option<String>, bool) {
    let Ok(envelope) = pool::call(
        chain_id,
        "eth_call",
        serde_json::json!([{ "to": to, "data": data }, "latest"]),
    ) else {
        return (None, false);
    };
    if envelope.get("error").is_some() {
        return (None, true);
    }
    (
        envelope
            .get("result")
            .and_then(Value::as_str)
            .map(str::to_owned),
        false,
    )
}

/// Candidate signatures for a selector, most-likely first, deduped.
///
/// Three databases, asked at once and merged rather than raced: they disagree,
/// and a selector collision is normal — the core tries each candidate and
/// keeps whichever decodes cleanly. The openchain-shaped hosts lead the merge
/// because they are spam-filtered; 4byte.directory fills the gaps.
fn lookup_selector(selector_hex: &str) -> Vec<String> {
    let selector = normalise_selector(selector_hex);
    let Some(selector) = selector else {
        return Vec::new();
    };
    if let Some(hit) = cache().lock().ok().and_then(|c| c.get(&selector).cloned()) {
        return hit;
    }

    let mut workers = Vec::new();
    for host in [
        "https://api.4byte.sourcify.dev",
        "https://api.openchain.xyz",
    ] {
        let selector = selector.clone();
        workers.push(thread::spawn(move || from_openchain(host, &selector)));
    }
    let fourbyte = selector.clone();
    workers.push(thread::spawn(move || from_4byte(&fourbyte)));

    let mut merged: Vec<String> = Vec::new();
    for worker in workers {
        // A panicked lookup is one source missing, never the end of the run.
        for sig in worker.join().unwrap_or_default() {
            if !merged.contains(&sig) {
                merged.push(sig);
            }
        }
    }
    if let Ok(mut cache) = cache().lock() {
        cache.insert(selector, merged.clone());
    }
    merged
}

/// `0x` + eight hex, lowercased. Anything else is not a selector, and asking
/// three databases about it would be three requests for nothing.
fn normalise_selector(raw: &str) -> Option<String> {
    let body = raw.strip_prefix("0x").unwrap_or(raw);
    let selector: String = body.chars().take(8).collect::<String>().to_lowercase();
    (selector.len() == 8 && selector.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("0x{selector}"))
}

fn from_openchain(host: &str, selector: &str) -> Vec<String> {
    let url = format!("{host}/signature-database/v1/lookup?function={selector}&filter=true");
    let Some(body) = get_json(&url) else {
        return Vec::new();
    };
    body.get("result")
        .and_then(|r| r.get("function"))
        .and_then(|f| f.get(selector))
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("name").and_then(Value::as_str))
                .filter(|name| name.contains('('))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// 4byte.directory, ordered by id ascending — the lowest id is the canonical
/// one, and the API does not order for us.
fn from_4byte(selector: &str) -> Vec<String> {
    let url = format!("https://www.4byte.directory/api/v1/signatures/?hex_signature={selector}");
    let Some(body) = get_json(&url) else {
        return Vec::new();
    };
    let Some(results) = body.get("results").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut rows: Vec<(u64, String)> = results
        .iter()
        .filter_map(|row| {
            let sig = row.get("text_signature").and_then(Value::as_str)?;
            sig.contains('(').then(|| {
                (
                    row.get("id").and_then(Value::as_u64).unwrap_or(0),
                    sig.to_owned(),
                )
            })
        })
        .collect();
    rows.sort_by_key(|(id, _)| *id);
    rows.into_iter().map(|(_, sig)| sig).collect()
}

fn get_json(url: &str) -> Option<Value> {
    proxy::agent(SELECTOR_TIMEOUT)
        .get(url)
        .header("accept", "application/json")
        .call()
        .ok()
        .filter(|response| response.status().as_u16() == 200)
        .and_then(|mut response| response.body_mut().read_json::<Value>().ok())
}

/// Selector → candidates, for this process.
///
/// The web caches a miss as `null` and returns `[]` for it; this stores the
/// empty vector, which has the same effect: asked twice, three databases are
/// asked once.
fn cache() -> &'static Mutex<HashMap<String, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What counts as a selector, before three databases are asked about it.
    #[test]
    fn only_eight_hex_digits_are_worth_asking_about() {
        assert_eq!(
            normalise_selector("0xA9059CBB").as_deref(),
            Some("0xa9059cbb"),
            "lowercased"
        );
        assert_eq!(
            normalise_selector("a9059cbb").as_deref(),
            Some("0xa9059cbb"),
            "the 0x is optional"
        );
        // Whole calldata: the selector is its first four bytes.
        assert_eq!(
            normalise_selector("0xa9059cbb0000000000000000000000001234").as_deref(),
            Some("0xa9059cbb")
        );
        assert_eq!(normalise_selector("0xa905").as_deref(), None, "too short");
        assert_eq!(normalise_selector("0xzzzzzzzz").as_deref(), None, "not hex");
        assert_eq!(normalise_selector("").as_deref(), None);
    }

    /// The best-known selector there is, through the real databases.
    ///
    /// `transfer(address,uint256)` is what every ERC-20 send is, so if the
    /// merge ever stops finding it, the generic decode rung is gone and every
    /// unrecognised transfer falls to blind signing.
    #[test]
    #[ignore = "asks the public selector databases"]
    fn live_the_transfer_selector_resolves() {
        let sigs = lookup_selector("0xa9059cbb");
        assert!(
            sigs.iter().any(|s| s == "transfer(address,uint256)"),
            "the canonical signature is among the candidates: {sigs:?}"
        );

        // Asked twice, the databases are asked once: the second call is the
        // cache, and a signing sheet re-resolving a selector per keystroke
        // would hammer three public services.
        let again = lookup_selector("0xa9059cbb");
        assert_eq!(again, sigs);
    }
}
