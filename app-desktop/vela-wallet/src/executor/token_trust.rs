//! The `token_trust` session, and the only place it touches the outside world.
//!
//! Seven operations: the incoming-transfer scan (`eth_blockNumber`,
//! `eth_getLogs`, `eth_getBlockByNumber`), a batched ERC-20 metadata read, and
//! the custom-token ledger.
//!
//! **Ported from** `src/services/transfer-monitor.ts` and
//! `src/services/token-metadata.ts` @ `c513c4c6` (FR-006). Every *decision* —
//! which contracts are trusted, whether a log is admissible, whether a
//! simulation delta may be believed, when a token may be written — is
//! `token_trust.rs`'s 1,937 lines and is not re-derived here.
//!
//! ## A session on a thread, not a gpui resident
//!
//! This machine's callers are background workers doing blocking HTTP: the
//! activity feed's `ScanIncomingTransfers` runs a whole discovery pipeline and
//! needs the answer on the thread it is already on, and 032's receipt-confirmed
//! auto-add will be the same shape. That is `pool.rs`'s situation, not
//! `resident.rs`'s, so it gets `pool.rs`'s answer: one thread, one `OnceLock`,
//! and callers block on a reply channel.
//!
//! **One session matters here for a specific reason.** The trusted-contract
//! allowlist is assembled from THREE inputs that arrive separately — the chains
//! this account holds on, the ERC-20s it holds, and the registry's canonical
//! stablecoins per chain. A second session would start with none of them, and
//! the core would correctly degrade to "customs plus the native sentinels" —
//! which is safe but means a plain USDC payment is never discovered. Sharing
//! the session is what makes the snapshots worth feeding.
//!
//! ## The range cap is the pool's word, not a string match
//!
//! `eth_getLogs` fails two ways that must not be confused: the endpoint is
//! broken, or the endpoint is fine and the span was too wide. Only the second
//! is worth retrying with a narrower window, and only the first is worth
//! failing over — the next endpoint usually has the same cap, and banning a
//! healthy endpoint over it is how a pool loses its best RPC.
//!
//! The classification already exists: `rpc_pool` parses the wording and answers
//! `PoolError::RangeCap { max_span }`. This file maps that one error onto
//! `TrustLogsOutcome::RangeCapped` and everything else onto `Failed`. It does
//! not read an error message.
//!
//! ## A wave of operations runs in parallel
//!
//! A poll over six chains issues six `eth_blockNumber`s at once, then six
//! `eth_getLogs`, then up to 25 block-timestamp reads per chain. Performed one
//! at a time that is a minute of waiting for a screen that is supposed to
//! notice money arriving. Each pending operation is independent, so the whole
//! wave runs on its own threads and the answers are resolved in order.

use std::sync::mpsc::{Sender, channel};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde_json::{Value, json};

use vela_core::app::token_trust::{
    Event, TokenTrust, TrustIncomingView, TrustLogsOutcome, TrustMetaEntry, TrustOperation,
    TrustRawLog, TrustShellResult, TrustTokenMeta,
};

use crate::core_host::CoreHost;
use crate::executor::pool::{self, PoolError};
use crate::executor::{abi, custom_tokens};

/// One routed call, returning the `result` member.
fn rpc(chain_id: u32, method: &str, params: Value) -> Result<Value, PoolError> {
    pool::call(chain_id, method, params)
        .map(|body| body.get("result").cloned().unwrap_or(Value::Null))
}

/// One raw log, as the core reads it. A log missing the fields that identify it
/// is dropped rather than defaulted: an entry with no transaction hash cannot
/// be de-duped, and a receipt that cannot be de-duped is a row that reappears.
fn to_raw_log(value: &Value) -> Option<TrustRawLog> {
    Some(TrustRawLog {
        address: value.get("address").and_then(Value::as_str)?.to_owned(),
        topics: value
            .get("topics")
            .and_then(Value::as_array)?
            .iter()
            .filter_map(|topic| topic.as_str().map(str::to_owned))
            .collect(),
        data: value
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or("0x")
            .to_owned(),
        transaction_hash: value
            .get("transactionHash")
            .and_then(Value::as_str)?
            .to_owned(),
        // The core reads an absent one as `0x0`, so absent is passed on as
        // absent rather than substituted here.
        block_number: value
            .get("blockNumber")
            .and_then(Value::as_str)
            .map(str::to_owned),
        log_index: value
            .get("logIndex")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

/// `symbol()` and `decimals()` for many addresses, in one `aggregate3`.
///
/// **Every requested address is answered**, resolved or not: the core records
/// an unresolvable token as a fact worth remembering, and a missing entry would
/// instead read as a question never asked.
///
/// All-or-nothing per token. A symbol with no scale renders an amount at the
/// wrong magnitude, which is worse than an unnamed token.
fn erc20_meta(chain_id: u32, addrs: &[String]) -> Vec<TrustMetaEntry> {
    let mut calls = Vec::with_capacity(addrs.len() * 2);
    for addr in addrs {
        calls.push(abi::Call3 {
            target: addr.clone(),
            call_data: abi::enc_symbol(),
        });
        calls.push(abi::Call3 {
            target: addr.clone(),
            call_data: abi::enc_decimals(),
        });
    }
    let results = if calls.is_empty() {
        Vec::new()
    } else {
        rpc(
            chain_id,
            "eth_call",
            json!([{ "to": abi::MULTICALL3, "data": abi::enc_aggregate3(&calls) }, "latest"]),
        )
        .ok()
        .and_then(|value| value.as_str().map(abi::dec_aggregate3))
        .unwrap_or_default()
    };

    addrs
        .iter()
        .enumerate()
        .map(|(index, addr)| {
            let answered = |at: usize| {
                results
                    .get(at)
                    .filter(|result| result.success)
                    .map(|result| result.data.as_str())
            };
            let symbol = answered(index * 2).and_then(abi::dec_string);
            let decimals = answered(index * 2 + 1).and_then(abi::dec_u8);
            TrustMetaEntry {
                addr: addr.clone(),
                meta: match (symbol, decimals) {
                    (Some(symbol), Some(decimals)) => Some(TrustTokenMeta {
                        symbol,
                        decimals: u32::from(decimals),
                    }),
                    _ => None,
                },
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// The session
// ---------------------------------------------------------------------------

enum Request {
    /// A fact the shell learned. Fire and forget — nothing waits on a snapshot.
    Learned(Box<Event>),
    /// One scan, driven to quiescence. The reply is what it found.
    Poll {
        address: String,
        reply: Sender<Vec<TrustIncomingView>>,
    },
    /// Judge one simulation's deltas (spec 037). UNTRUSTED input, and the core
    /// says so in its own header: this path can reach no write at all.
    Judge {
        address: String,
        chain_id: u32,
        deltas: Vec<vela_core::app::token_trust::TrustAssetDelta>,
        reply: Sender<Vec<vela_core::app::token_trust::TrustSimJudgment>>,
    },
}

static SESSION: OnceLock<Mutex<Sender<Request>>> = OnceLock::new();

fn sender() -> &'static Mutex<Sender<Request>> {
    SESSION.get_or_init(|| {
        let (tx, rx) = channel::<Request>();
        thread::Builder::new()
            .name("vela-token-trust".to_owned())
            .spawn(move || run(&rx))
            .ok();
        Mutex::new(tx)
    })
}

fn tell(event: Event) {
    if let Ok(tx) = sender().lock() {
        let _ = tx.send(Request::Learned(Box::new(event)));
    }
}

/// Which chains this account holds anything on — the scan set.
///
/// An empty list is not a gap the shell should paper over: the core reads it as
/// "brand-new wallet" and falls back to the main payment chains, so a first
/// receipt is still caught.
pub fn held_chains(address: &str, chain_ids: Vec<u32>) {
    tell(Event::HeldChainsSnapshot {
        address: address.to_owned(),
        chain_ids,
    });
}

/// The ERC-20 contracts this account holds on one chain — the trusted receive
/// set. A cold cache means an empty set means everything unverified, which is
/// the safe direction and the core's own words.
pub fn held_tokens(address: &str, chain_id: u32, tokens: Vec<String>) {
    tell(Event::HeldTokensSnapshot {
        address: address.to_owned(),
        chain_id,
        tokens,
    });
}

/// A chain's canonical stablecoins and its wrapped native coin, from the token
/// registry. These are what make a plain USDC payment discoverable at all: the
/// `eth_getLogs` allowlist is assembled from them plus the person's own tokens.
pub fn registry_tokens(chain_id: u32, stables: Vec<String>, wrapped_native: Option<String>) {
    tell(Event::RegistryTokensSnapshot {
        chain_id,
        stables,
        wrapped_native,
    });
}

/// A confirmed user operation's AUTHENTIC receipt logs — the single auto-add
/// entry point (invariant ⑤). `from` is the sender the receipt names; the
/// core nets the deltas for that address and admits what passes.
pub fn receipt_confirmed(
    from: &str,
    chain_id: u32,
    logs: Vec<vela_core::app::token_trust::TrustReceiptLog>,
) {
    tell(Event::ReceiptLogsConfirmed {
        from: from.to_owned(),
        chain_id,
        logs,
    });
}

/// Run one scan and return what it found. **Blocks** — call it from a worker.
///
/// The core is single-flight: a poll requested while one is running is ignored
/// and the next tick retries. So an empty answer can mean "nothing new" or
/// "already scanning", and both are the same instruction to the caller: do
/// nothing this tick.
#[must_use]
pub fn poll(address: &str) -> Vec<TrustIncomingView> {
    let (reply, answer) = channel();
    {
        let Ok(tx) = sender().lock() else {
            return Vec::new();
        };
        if tx
            .send(Request::Poll {
                address: address.to_owned(),
                reply,
            })
            .is_err()
        {
            return Vec::new();
        }
    }
    answer.recv().unwrap_or_default()
}

/// Judge one simulation's deltas. **Blocks** — call it from a worker.
///
/// Which of these may be shown with a confident amount is the core's
/// asymmetric rule (invariant ⑥): an OUTFLOW renders whenever metadata
/// resolved, because the real token emits its own log and an outflow cannot be
/// understated; an INFLOW renders a number only when the token is trusted,
/// because a site can emit any `Transfer` it likes from a contract it controls.
/// The shell asks and draws; it never decides which side of that line a token
/// falls on.
#[must_use]
pub fn judge(
    address: &str,
    chain_id: u32,
    deltas: Vec<vela_core::app::token_trust::TrustAssetDelta>,
) -> Vec<vela_core::app::token_trust::TrustSimJudgment> {
    if deltas.is_empty() {
        return Vec::new();
    }
    let (reply, answer) = channel();
    {
        let Ok(tx) = sender().lock() else {
            return Vec::new();
        };
        if tx
            .send(Request::Judge {
                address: address.to_owned(),
                chain_id,
                deltas,
                reply,
            })
            .is_err()
        {
            return Vec::new();
        }
    }
    answer.recv().unwrap_or_default()
}

fn run(rx: &std::sync::mpsc::Receiver<Request>) {
    let mut host = CoreHost::<TokenTrust>::new();
    while let Ok(request) = rx.recv() {
        match request {
            Request::Learned(event) => {
                let pending = host.dispatch(*event);
                drain(&mut host, pending);
            }
            Request::Poll { address, reply } => {
                let pending = host.dispatch(Event::PollRequested { address });
                drain(&mut host, pending);
                let _ = reply.send(host.view().incoming);
            }
            Request::Judge {
                address,
                chain_id,
                deltas,
                reply,
            } => {
                let pending = host.dispatch(Event::SimDeltasComputed {
                    address,
                    chain_id,
                    deltas,
                });
                drain(&mut host, pending);
                let _ = reply.send(
                    host.view()
                        .sim
                        .filter(|sim| sim.ready)
                        .map(|sim| sim.judgments)
                        .unwrap_or_default(),
                );
            }
        }
    }
}

/// Perform every pending operation, in waves, until the machine is quiescent.
///
/// The wave runs in parallel because its members are independent — six chains'
/// block numbers have nothing to say to each other — and serially it is a
/// minute of waiting. The ANSWERS are resolved back in order, on this thread,
/// because the core is not `Send` and does not want to be.
fn drain(
    host: &mut CoreHost<TokenTrust>,
    mut pending: Vec<crate::core_host::Pending<TrustOperation>>,
) {
    // A cap, not a timeout: a machine that kept asking would spin this thread
    // forever, and a poll needs a handful of waves.
    for _ in 0..32 {
        if pending.is_empty() {
            return;
        }
        let mut workers = Vec::with_capacity(pending.len());
        for next in pending.drain(..) {
            let id = next.id;
            workers.push(
                thread::Builder::new()
                    .name("vela-trust-op".to_owned())
                    .spawn(move || (id, perform(&next.operation)))
                    .ok(),
            );
        }
        for worker in workers.into_iter().flatten() {
            // A panicked worker is an operation the core will wait on forever.
            // There is nothing honest to answer in its place, so the poll ends
            // with what it has rather than inventing a result.
            let Ok((id, result)) = worker.join() else {
                continue;
            };
            pending.extend(host.resolve(id, result));
        }
    }
}

fn perform(operation: &TrustOperation) -> TrustShellResult {
    match operation {
        TrustOperation::RpcBlockNumber { address, chain_id } => TrustShellResult::BlockNumber {
            address: address.clone(),
            chain_id: *chain_id,
            block_hex: rpc(*chain_id, "eth_blockNumber", json!([]))
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned)),
        },

        TrustOperation::RpcGetLogs {
            address,
            chain_id,
            from_block,
            to_block,
            recipient_topic,
            contracts,
        } => {
            let filter = json!({
                "fromBlock": from_block,
                "toBlock": to_block,
                // topics[1] is the sender and is deliberately unfiltered: this
                // asks who RECEIVED, from anyone.
                "topics": [
                    vela_core::app::token_trust::TRANSFER_TOPIC,
                    Value::Null,
                    recipient_topic,
                ],
                // The allowlist. An EMPTY list is not "no filter": the core
                // asks with the contracts it trusts, and dropping the key would
                // widen the query to every token on the chain — which is
                // precisely the spam channel the allowlist exists to close.
                "address": contracts,
            });
            let outcome = match pool::call(*chain_id, "eth_getLogs", json!([filter])) {
                Ok(body) => TrustLogsOutcome::Ok {
                    logs: body
                        .get("result")
                        .and_then(Value::as_array)
                        .map(|logs| logs.iter().filter_map(to_raw_log).collect())
                        .unwrap_or_default(),
                },
                // The pool already parsed the endpoint's wording. A cap it
                // could not put a number to arrives as 0, which the core reads
                // as "narrow conservatively".
                Err(PoolError::RangeCap { max_span }) => TrustLogsOutcome::RangeCapped {
                    cap: if max_span.is_finite() && max_span > 0.0 {
                        #[allow(
                            clippy::cast_possible_truncation,
                            clippy::cast_sign_loss,
                            reason = "a block span the endpoint stated"
                        )]
                        {
                            max_span as u32
                        }
                    } else {
                        0
                    },
                },
                Err(PoolError::Failed { .. } | PoolError::Unavailable) => TrustLogsOutcome::Failed,
            };
            TrustShellResult::Logs {
                address: address.clone(),
                chain_id: *chain_id,
                outcome,
            }
        }

        TrustOperation::RpcGetBlockByNumber {
            address,
            chain_id,
            block,
        } => {
            let header = rpc(*chain_id, "eth_getBlockByNumber", json!([block, false])).ok();
            let timestamp_sec = header
                .as_ref()
                .and_then(|value| value.get("timestamp"))
                .and_then(Value::as_str)
                .and_then(|hex| u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
                // Precision: 2^53 seconds is past any block this wallet will
                // see, but a garbage word is not, and it must not become a
                // plausible date.
                .filter(|seconds| *seconds < (1u64 << 53))
                .map(|seconds| {
                    #[allow(clippy::cast_precision_loss, reason = "guarded above")]
                    {
                        seconds as f64
                    }
                });
            TrustShellResult::BlockTimestamp {
                address: address.clone(),
                chain_id: *chain_id,
                block_number: abi::dec_hex_quantity(block)
                    .and_then(|digits| digits.parse::<f64>().ok())
                    .unwrap_or(0.0),
                // `None` = the lookup failed; the core falls the transfer back
                // to "now" itself.
                timestamp_sec,
                now_ms: crate::executor::now_ms(),
            }
        }

        TrustOperation::MulticallErc20Meta { chain_id, addrs } => TrustShellResult::ErcMeta {
            chain_id: *chain_id,
            entries: erc20_meta(*chain_id, addrs),
        },

        TrustOperation::ReadCustomTokens => TrustShellResult::CustomTokens {
            // `Some(vec![])` is "there are none"; `None` would be "the read
            // failed", which fails the admission closed. A read that returned
            // nothing is the first of those.
            tokens: Some(
                custom_tokens::read()
                    .iter()
                    .map(custom_tokens::StoredToken::to_trust)
                    .collect(),
            ),
        },

        TrustOperation::WriteCustomToken { token } => TrustShellResult::TokenWritten {
            ok: custom_tokens::save(custom_tokens::StoredToken::from(token)),
        },

        // The balance fetch reads the token list on every run, so there is no
        // separate token cache to drop on the desktop. Answered, because a
        // skipped operation leaves the core waiting forever.
        TrustOperation::InvalidateTokenCache { .. } => TrustShellResult::CacheInvalidated,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::storage;
    use vela_core::app::token_trust::TrustCustomToken;

    /// A log that cannot be identified is dropped, not defaulted.
    #[test]
    fn a_log_without_an_identity_is_dropped_rather_than_invented() {
        let good = json!({
            "address": "0xaaa",
            "topics": ["0xddf2", "0x00", "0x11"],
            "data": "0x2a",
            "transactionHash": "0xdead",
            "blockNumber": "0x10",
            "logIndex": "0x1",
        });
        let log = to_raw_log(&good).unwrap_or_else(|| unreachable!("a complete log"));
        assert_eq!(log.address, "0xaaa");
        assert_eq!(log.topics.len(), 3);
        assert_eq!(log.block_number.as_deref(), Some("0x10"));

        // No transaction hash: nothing to de-dupe by, so the row would come
        // back on every poll.
        assert_eq!(
            to_raw_log(&json!({ "address": "0xaaa", "topics": [] })),
            None
        );
        // No address: nothing to check against the allowlist.
        assert_eq!(
            to_raw_log(&json!({ "topics": [], "transactionHash": "0xdead" })),
            None
        );
        // An absent block number stays absent — the core reads it as `0x0`,
        // and substituting one here would hide that from its own rule.
        let sparse = json!({
            "address": "0xaaa",
            "topics": [],
            "transactionHash": "0xdead",
        });
        let log = to_raw_log(&sparse).unwrap_or_else(|| unreachable!("identifiable"));
        assert_eq!(log.block_number, None);
        assert_eq!(
            log.data, "0x",
            "an absent data member is empty, not missing"
        );
    }

    /// The ledger this machine shares with `manage_tokens`.
    #[test]
    fn a_token_admitted_here_is_the_same_record_the_panel_writes() {
        storage::tests::with_temp_state("trust-ledger", || {
            let token = TrustCustomToken {
                id: "100_0xaaa".to_owned(),
                chain_id: 100,
                contract_address: "0xaaa".to_owned(),
                symbol: "AAA".to_owned(),
                name: "Triple A".to_owned(),
                decimals: 6,
            };
            assert_eq!(
                super::perform(&TrustOperation::WriteCustomToken {
                    token: token.clone()
                }),
                TrustShellResult::TokenWritten { ok: true }
            );

            // Read back through this machine's own operation…
            match super::perform(&TrustOperation::ReadCustomTokens) {
                TrustShellResult::CustomTokens { tokens } => {
                    let tokens = tokens.unwrap_or_else(|| unreachable!("the read succeeded"));
                    assert_eq!(tokens, vec![token]);
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
            // …and the derived display name the core does not carry is there.
            assert_eq!(custom_tokens::read()[0].network_name, "Gnosis");
        });
    }

    /// An empty ledger is `Some(vec![])`, never `None`.
    ///
    /// `None` means the READ failed and fails the admission closed. Conflating
    /// the two would make a wallet with no custom tokens look like a wallet
    /// whose storage is broken.
    #[test]
    fn no_custom_tokens_is_not_a_failed_read() {
        storage::tests::with_temp_state("trust-empty-ledger", || {
            match super::perform(&TrustOperation::ReadCustomTokens) {
                TrustShellResult::CustomTokens { tokens } => {
                    assert_eq!(tokens, Some(Vec::new()));
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// The cache invalidation is answered even though there is nothing to drop.
    #[test]
    fn the_cache_invalidation_is_answered_rather_than_skipped() {
        assert_eq!(
            super::perform(&TrustOperation::InvalidateTokenCache {
                address: "0xabc".to_owned()
            }),
            TrustShellResult::CacheInvalidated
        );
    }

    /// The whole scan, live: snapshots in, a poll out.
    ///
    /// The golden Safe has no recent incoming transfer, so what this proves is
    /// that the pipeline RUNS — the allowlist assembles from the snapshots, the
    /// chains are read, the logs come back — rather than that it finds
    /// something. A test that needed somebody to send money to pass is a test
    /// that fails for the wrong reason.
    #[test]
    #[ignore = "polls several chains for a real address"]
    fn a_poll_runs_the_whole_pipeline_over_the_chains_it_was_told_about() {
        storage::tests::with_temp_state("trust-poll-live", || {
            crate::executor::chain_tokens::invalidate();
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

            // The three snapshots the balance fetch normally supplies.
            held_chains(GOLDEN, vec![100]);
            held_tokens(GOLDEN, 100, Vec::new());
            let data = crate::executor::chain_tokens::fetch(100)
                .unwrap_or_else(|| unreachable!("no chain index"));
            registry_tokens(
                100,
                data.stables.iter().map(|s| s.contract.clone()).collect(),
                data.wrapped_native.clone(),
            );

            let started = std::time::Instant::now();
            let found = poll(GOLDEN);
            println!(
                "  polled Gnosis in {:?}, {} incoming",
                started.elapsed(),
                found.len()
            );
            for transfer in &found {
                println!(
                    "    {} {} from {} on {}",
                    transfer.value,
                    transfer.symbol.as_deref().unwrap_or("?"),
                    transfer.from,
                    transfer.chain_id
                );
            }

            // A wave of RPCs run in parallel; serially this would be minutes.
            assert!(
                started.elapsed().as_secs() < 60,
                "the poll should not take a minute"
            );
            // Whatever it found is FOR this address and on a chain we asked
            // about — a scan that returned somebody else's transfer would be
            // the failure mode the topic filter and the allowlist exist to
            // prevent.
            for transfer in &found {
                assert_eq!(transfer.chain_id, 100);
                assert!(!transfer.tx_hash.is_empty());
            }
        });
    }

    /// The pipeline against an address that actually receives money.
    ///
    /// The golden Safe is quiet, so the poll above proves the machinery runs
    /// and not that it FINDS. This points it at a busy Curve pool on Gnosis,
    /// which takes USDC constantly.
    ///
    /// It cannot assert a non-empty result: whether anything landed in the last
    /// hundred blocks is a stranger's business, and a test that goes red
    /// because somebody stopped trading is reporting the wrong thing. What it
    /// asserts is what must hold about anything it DOES find — and the printed
    /// count is the evidence a person reads.
    ///
    /// Measured 2026-09-04: five USDC transfers, metadata resolved, block times
    /// read, in 14 seconds.
    #[test]
    #[ignore = "polls a busy third-party address on Gnosis"]
    fn a_poll_finds_real_transfers_with_their_metadata_resolved() {
        storage::tests::with_temp_state("trust-busy-live", || {
            crate::executor::chain_tokens::invalidate();
            // A Curve 3pool on Gnosis. Not ours, and that is the point: this
            // address receives whether or not anybody is testing.
            const BUSY: &str = "0x7f90122BF0700F9E7e1F688fe926940E8839F353";

            held_chains(BUSY, vec![100]);
            held_tokens(BUSY, 100, Vec::new());
            let data = crate::executor::chain_tokens::fetch(100)
                .unwrap_or_else(|| unreachable!("no chain index"));
            registry_tokens(
                100,
                data.stables.iter().map(|s| s.contract.clone()).collect(),
                data.wrapped_native.clone(),
            );

            let found = poll(BUSY);
            println!("  {} incoming in the scan window", found.len());
            for transfer in &found {
                println!(
                    "    {} {} from {}",
                    transfer.value,
                    transfer.symbol.as_deref().unwrap_or("?"),
                    transfer.from
                );
            }

            for transfer in &found {
                // Admitted means the allowlist let it through, which means it
                // is a token the registry or this wallet named — so metadata
                // MUST have resolved. An admitted transfer with no symbol would
                // be stored at a guessed scale.
                assert!(
                    transfer.symbol.is_some() && transfer.decimals.is_some(),
                    "admitted without metadata: {transfer:?}"
                );
                // A raw quantity, which the shell scales exactly once on the
                // way into the store.
                assert!(
                    transfer.value.chars().all(|c| c.is_ascii_digit()),
                    "not base units: {}",
                    transfer.value
                );
                assert!(!transfer.from.is_empty() && !transfer.tx_hash.is_empty());
                // A block timestamp, not a fallback to now: the difference is
                // whether the feed files it under the right day.
                assert!(transfer.timestamp_sec > 1_700_000_000.0);
                assert_eq!(transfer.chain_id, 100);
            }
        });
    }

    /// Every requested address is answered, and a token that answered only half
    /// is `None` rather than half a token.
    #[test]
    #[ignore = "reads real ERC-20s on Gnosis"]
    fn a_batched_metadata_read_answers_every_address_it_was_given() {
        storage::tests::with_temp_state("trust-meta-live", || {
            // USDC on Gnosis, then an address with no contract behind it.
            let addrs = vec![
                "0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83".to_owned(),
                "0x0000000000000000000000000000000000000001".to_owned(),
            ];
            let result = super::perform(&TrustOperation::MulticallErc20Meta {
                chain_id: 100,
                addrs: addrs.clone(),
            });
            match result {
                TrustShellResult::ErcMeta { chain_id, entries } => {
                    assert_eq!(chain_id, 100);
                    for entry in &entries {
                        println!("    {} → {:?}", entry.addr, entry.meta);
                    }
                    assert_eq!(entries.len(), addrs.len(), "every address is answered");
                    let usdc = entries[0]
                        .meta
                        .clone()
                        .unwrap_or_else(|| unreachable!("USDC did not answer"));
                    assert_eq!(usdc.decimals, 6);
                    assert!(!usdc.symbol.is_empty());
                    // Looked up and unresolvable is a fact, not a default.
                    assert_eq!(entries[1].meta, None);
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// The scan's three RPCs, against a real chain.
    #[test]
    #[ignore = "reads Gnosis"]
    fn the_scan_reads_a_block_its_timestamp_and_its_logs() {
        storage::tests::with_temp_state("trust-scan-live", || {
            const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
            let block_hex = match super::perform(&TrustOperation::RpcBlockNumber {
                address: GOLDEN.to_owned(),
                chain_id: 100,
            }) {
                TrustShellResult::BlockNumber { block_hex, .. } => {
                    block_hex.unwrap_or_else(|| unreachable!("no block number"))
                }
                other => unreachable!("wrong variant: {other:?}"),
            };
            let latest = abi::dec_hex_quantity(&block_hex)
                .and_then(|digits| digits.parse::<u64>().ok())
                .unwrap_or_else(|| unreachable!("not a quantity: {block_hex}"));
            println!("    latest block: {latest}");
            assert!(latest > 30_000_000, "Gnosis is well past this height");

            match super::perform(&TrustOperation::RpcGetBlockByNumber {
                address: GOLDEN.to_owned(),
                chain_id: 100,
                block: block_hex.clone(),
            }) {
                TrustShellResult::BlockTimestamp {
                    block_number,
                    timestamp_sec,
                    now_ms,
                    ..
                } => {
                    let seconds =
                        timestamp_sec.unwrap_or_else(|| unreachable!("no block timestamp"));
                    println!("    block {block_number} at {seconds}");
                    #[allow(clippy::cast_precision_loss, reason = "a block height")]
                    let expected = latest as f64;
                    assert!((block_number - expected).abs() < 1.0);
                    // The header's time is within an hour of this machine's,
                    // which is what says we read a timestamp and not a word.
                    assert!((seconds * 1000.0 - now_ms).abs() < 3_600_000.0);
                }
                other => unreachable!("wrong variant: {other:?}"),
            }

            // The allowlist restricted to one real token: an answer, not a cap
            // and not a failure.
            let recipient_topic =
                format!("0x{:0>64}", GOLDEN.trim_start_matches("0x").to_lowercase());
            match super::perform(&TrustOperation::RpcGetLogs {
                address: GOLDEN.to_owned(),
                chain_id: 100,
                from_block: format!("0x{:x}", latest.saturating_sub(50)),
                to_block: format!("0x{latest:x}"),
                recipient_topic,
                contracts: vec!["0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83".to_owned()],
            }) {
                TrustShellResult::Logs { outcome, .. } => {
                    println!("    logs: {outcome:?}");
                    // Zero logs in fifty blocks is the expected answer and is
                    // still `Ok` — nothing found is not a failure.
                    assert!(
                        matches!(outcome, TrustLogsOutcome::Ok { .. }),
                        "a healthy endpoint over 50 blocks must not fail"
                    );
                }
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }
}
