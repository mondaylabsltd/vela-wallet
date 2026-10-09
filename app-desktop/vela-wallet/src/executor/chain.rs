//! The four chain reads the submit path needs, through the pool.
//!
//! **Ported from** the RPC helpers of
//! `app-web/vela-wallet/src/lib/services/safe-transaction.ts` @ `origin/main`
//! (`isDeployed`, `getNonce`, `incrementNonceCache`, `fetchRawGasSignals`,
//! `verifyChainReady`). Small, and load-bearing in one specific way each:
//!
//! - **Deployment status is correctness-critical.** It decides whether the
//!   operation carries `initCode`. A transient failure guessed as "deployed"
//!   ships an empty initCode for a fresh account (AA20, every new user's first
//!   send fails); guessed as "undeployed" it attaches initCode to a live one
//!   (AA10). Neither guess is safe, so an INDETERMINATE read is an error the
//!   caller retries, and only a definitive answer is trusted.
//! - **A nonce failure is not permission to sign nonce 0.** A deployed wallet
//!   that signed `0x0` would burn a passkey prompt on an op the relay rejects
//!   (AA25). The error is typed so the caller can fail before signing.
//! - **The gas signals are handed over raw.** The tip-inclusive price rule
//!   (`derive_chain_gas_price`) is `fee_policy`'s; feeding it a derived value
//!   would apply the rule twice.
//!
//! And one read after the submit (083): how an operation ended inside the
//! bundle transaction that carried it — that transaction's own status says
//! nothing about it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::fee_policy::{
    ChainGasPrice, DeploymentRead, GasSignals, derive_chain_gas_price, is_tempo_chain,
    min_gas_price_wei,
};
use vela_core::app::tx_tracker::user_op_outcome_in_logs;
use vela_core::primitives::{abi_encode_address, function_selector, to_hex};
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::parse_hex_quantity;

use crate::executor::{pool, relay};

/// `GAS_PRICE_CACHE_TTL` (`safe-transaction.ts:2482`).
const GAS_PRICE_TTL: Duration = Duration::from_secs(15);

const INDETERMINATE_DEPLOYMENT: &str = "Could not verify the account deployment status — the network may be unstable. Please try again.";
const INDETERMINATE_NONCE: &str =
    "Could not fetch the account nonce — the network may be unstable. Please try again.";

/// Once deployed, always deployed (irreversible), so only `true` is cached.
static DEPLOYED: Mutex<Option<HashMap<String, ()>>> = Mutex::new(None);
static GAS_PRICES: Mutex<Option<HashMap<u32, (ChainGasPrice, Instant)>>> = Mutex::new(None);
static CHAIN_READY: Mutex<Option<HashMap<u32, ()>>> = Mutex::new(None);

fn key(chain_id: u32, address: &str) -> String {
    format!("{chain_id}:{}", address.to_lowercase())
}

/// Whether `address` has code (`isDeployed`). `Err` is an INDETERMINATE
/// read — an unreachable chain, an RPC error, or a non-string result — and
/// the caller must retry rather than guess.
pub fn is_deployed(address: &str, chain_id: u32) -> Result<bool, String> {
    match read_deployment(address, chain_id, false) {
        DeploymentRead::Read { deployed } => Ok(deployed),
        DeploymentRead::Unreachable { .. } | DeploymentRead::Internal { .. } => {
            Err(INDETERMINATE_DEPLOYMENT.to_owned())
        }
    }
}

/// The account read the fee machine asks for (`FeeOperation::ReadDeployment`,
/// issue #483), told the way the core words it: the chain answered, the
/// chain's nodes did not (rate-limited or not — the pool's own signal), or the
/// read never left the app (the pool thread is gone) — `internal`, never "the
/// chain is down".
///
/// `fresh` (a run after a failure) reads past the held answer. Only `true`
/// is ever held — deployment is irreversible — and a failure never is, so a
/// retry after a failed read is always a new read.
pub fn read_deployment(address: &str, chain_id: u32, fresh: bool) -> DeploymentRead {
    let cache_key = key(chain_id, address);
    if !fresh
        && let Ok(cache) = DEPLOYED.lock()
        && cache
            .as_ref()
            .is_some_and(|map| map.contains_key(&cache_key))
    {
        return DeploymentRead::Read { deployed: true };
    }
    // The speed sessions and a prewarm ask together; one `eth_getCode`.
    static IN_FLIGHT: crate::executor::single_flight::SingleFlight<String, DeploymentRead> =
        crate::executor::single_flight::SingleFlight::new();
    IN_FLIGHT.run(cache_key.clone(), || {
        let read = deployment_of(pool::call(
            chain_id,
            "eth_getCode",
            json!([address, "latest"]),
        ));
        if read == (DeploymentRead::Read { deployed: true })
            && let Ok(mut cache) = DEPLOYED.lock()
        {
            cache.get_or_insert_with(HashMap::new).insert(cache_key, ());
        }
        read
    })
}

/// An `eth_getCode` answer, as the fee machine reads it.
fn deployment_of(answer: Result<Value, pool::PoolError>) -> DeploymentRead {
    let body = match answer {
        Ok(body) => body,
        Err(pool::PoolError::Failed { rate_limited }) => {
            return DeploymentRead::Unreachable { rate_limited };
        }
        // `eth_getCode` has no range to cap; a node that says so is a node
        // that did not answer the question.
        Err(pool::PoolError::RangeCap { .. }) => {
            return DeploymentRead::Unreachable {
                rate_limited: false,
            };
        }
        Err(pool::PoolError::Unavailable) => {
            return DeploymentRead::Internal {
                kind: "rpc: pool_unavailable".to_owned(),
            };
        }
    };
    // A node that answered with an error, or with something that is not code,
    // did not tell us — the chain's side, not ours.
    let code = (body.get("error").is_none())
        .then(|| body.get("result").and_then(Value::as_str))
        .flatten();
    match code {
        Some(code) => DeploymentRead::Read {
            deployed: code != "0x" && code.len() > 2,
        },
        None => DeploymentRead::Unreachable {
            rate_limited: false,
        },
    }
}

/// `EntryPoint.getNonce(safe, 0)` (`getNonce`), as the RPC spelled it —
/// read fresh every time. `Err` is a genuine RPC failure: a fresh account
/// answers a valid zero word, never an error.
///
/// There is no cache and no local "+1" any more: the core holds a second
/// operation's confirm while the first is in flight
/// (`tx_tracker::in_flight_ops`), so a bumped nonce could only park an
/// operation at N+1 behind an N the relay may yet refuse.
pub fn nonce(safe: &str, chain_id: u32) -> Result<String, String> {
    let mut data = function_selector("getNonce(address,uint192)").map_err(|e| e.to_string())?;
    data.extend(abi_encode_address(safe).map_err(|e| e.to_string())?);
    data.extend([0u8; 32]);
    let body = pool::call(
        chain_id,
        "eth_call",
        json!([{ "to": ENTRY_POINT, "data": to_hex(&data, true) }, "latest"]),
    )
    .map_err(|_| INDETERMINATE_NONCE.to_owned())?;
    if body.get("error").is_some() {
        return Err(INDETERMINATE_NONCE.to_owned());
    }
    let Some(nonce) = body.get("result").and_then(Value::as_str) else {
        return Err(INDETERMINATE_NONCE.to_owned());
    };
    Ok(nonce.to_owned())
}

/// How long the submit waits for the head before POSTing without it.
const HEAD_BUDGET: Duration = Duration::from_secs(5);

/// The chain's head block, best effort (spec 082 ruling 8): read once before
/// a submit's first POST, so the tracker's relay-independent landing check
/// knows where the operation can first appear. `None` when the chain did not
/// answer within [`HEAD_BUDGET`] — the tracker then scans back from the head.
pub fn head_block(chain_id: u32) -> Option<u64> {
    let body = pool::call_within(chain_id, "eth_blockNumber", json!([]), HEAD_BUDGET).ok()?;
    let hex = body.get("result").and_then(Value::as_str)?;
    u64::from_str_radix(hex.trim_start_matches("0x"), 16).ok()
}

/// The EntryPoint's word on how one operation inside a bundle ended:
/// `UserOperationEvent(bytes32 indexed userOpHash, address indexed sender,
/// address indexed paymaster, uint256 nonce, bool success, uint256
/// actualGasCost, uint256 actualGasUsed)`.
#[cfg(test)]
const USER_OPERATION_EVENT: &str =
    "UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)";

/// How `user_op_hash` ended inside bundle transaction `tx_hash`, read from
/// that transaction's receipt on the chain's own node (083): `Some(true)`
/// executed, `Some(false)` reverted. `None` while the node has no receipt
/// yet, or when the receipt carries no event for the operation — neither is
/// an outcome.
pub fn user_op_outcome_in(
    tx_hash: &str,
    user_op_hash: &str,
    chain_id: u32,
    budget: Duration,
) -> Option<bool> {
    let body = pool::call_within(
        chain_id,
        "eth_getTransactionReceipt",
        json!([tx_hash]),
        budget,
    )
    .ok()?;
    user_op_outcome(body.get("result")?, user_op_hash)
}

/// Did `user_op_hash` execute, by a bundle transaction's receipt? `false`
/// when the EntryPoint's `UserOperationEvent` for it says `success: false`
/// — and when its own execution logged a Safe `ExecutionFailure`: the
/// EntryPoint counts that op a success while nothing happened, which the
/// tracker already rules a failure (spec 038 #D1). `None` when the receipt
/// carries no event for the op.
///
/// A bundle holds several operations, so only the op's OWN execution logs
/// count: the EntryPoint (v0.7) validates every op, emits `BeforeExecution`,
/// then runs each op and closes it with its `UserOperationEvent` — the logs
/// since the previous boundary are that op's.
///
/// The rule is the core's (`tx_tracker::user_op_outcome_in_logs`), the one
/// the tracker and the relay receipt's answer read too.
pub fn user_op_outcome(receipt: &Value, user_op_hash: &str) -> Option<bool> {
    let logs: Vec<_> = receipt
        .get("logs")?
        .as_array()?
        .iter()
        .filter_map(relay::to_trust_log)
        .collect();
    user_op_outcome_in_logs(&logs, user_op_hash)
}

/// The three chain price signals, raw, as decimal strings (`fetchRawGasSignals`).
/// `None` is a failed or skipped read; only the core turns an absent
/// `eth_gasPrice` into the 5-gwei default.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RawGasSignals {
    pub eth_gas_price: Option<String>,
    pub base_fee: Option<String>,
    pub priority_fee: Option<String>,
}

fn quantity_text(value: Option<&Value>) -> Option<String> {
    parse_hex_quantity(Some(value?.as_str()?))
        .ok()
        .map(|n| n.to_string())
}

pub fn raw_gas_signals(chain_id: u32, want_tip: bool) -> RawGasSignals {
    read_gas_signals(chain_id, want_tip).0
}

/// [`raw_gas_signals`], and whether the reading is COMPLETE: every leg that was
/// asked for answered, and the price is positive. Only a complete reading may
/// be held (`fee_signals`, issue 212) — without a positive `eth_gasPrice` the
/// core falls to its 5-gwei default, and without the tip it under-prices
/// Gnosis ~40×, and neither guess may be pinned for 15 s.
pub fn read_gas_signals(chain_id: u32, want_tip: bool) -> (RawGasSignals, bool) {
    let gas_price = pool::call(chain_id, "eth_gasPrice", json!([])).ok();
    let block = pool::call(chain_id, "eth_getBlockByNumber", json!(["latest", false])).ok();
    let tip = want_tip
        .then(|| pool::call(chain_id, "eth_maxPriorityFeePerGas", json!([])).ok())
        .flatten();
    let signals = RawGasSignals {
        eth_gas_price: quantity_text(gas_price.as_ref().and_then(|b| b.get("result"))),
        base_fee: quantity_text(
            block
                .as_ref()
                .and_then(|b| b.pointer("/result/baseFeePerGas")),
        ),
        priority_fee: quantity_text(tip.as_ref().and_then(|b| b.get("result"))),
    };
    // A block that ANSWERED without `baseFeePerGas` is a real pre-London
    // reading, not a failed leg; a block that did not answer is.
    let block_answered = block
        .as_ref()
        .and_then(|b| b.get("result"))
        .is_some_and(Value::is_object);
    // What may be held is the core's rule (`fee_policy::gas_signals_cacheable`).
    let complete = vela_core::app::fee_policy::gas_signals_cacheable(
        signals.eth_gas_price.as_deref(),
        block_answered,
        want_tip,
        signals.priority_fee.as_deref(),
    );
    (signals, complete)
}

/// `getGasPrices`: the derived network price, cached 15 s, with the 5-gwei
/// default when even `eth_gasPrice` is absent. Used by the send-time
/// fallback quote and Tempo's reimbursement; the confirm-screen quote is
/// `fee_policy`'s and reads the raw signals instead.
pub fn chain_gas_price(chain_id: u32) -> ChainGasPrice {
    if let Ok(cache) = GAS_PRICES.lock()
        && let Some((price, at)) = cache.as_ref().and_then(|map| map.get(&chain_id))
        && at.elapsed() < GAS_PRICE_TTL
    {
        return *price;
    }
    let want_tip = !is_tempo_chain(chain_id);
    let signals = raw_gas_signals(chain_id, want_tip);
    let parse = |text: &Option<String>| text.as_deref().and_then(|t| t.parse::<u128>().ok());
    let derived = match parse(&signals.eth_gas_price) {
        Some(eth_gas_price) => {
            let tip_measured = want_tip && signals.priority_fee.is_some();
            let derived = derive_chain_gas_price(&GasSignals {
                chain_id,
                eth_gas_price,
                base_fee: parse(&signals.base_fee).unwrap_or(0),
                priority_fee: parse(&signals.priority_fee).unwrap_or(0),
                tip_measured: Some(tip_measured),
            });
            if derived.gas_price > 0 {
                Some(derived)
            } else {
                None
            }
        }
        None => None,
    };
    // The static fallback obeys the chain's floor too: on Arc, 5 gwei doubled
    // by the bundler margin is under the 20 gwei minimum, and Arc discards an
    // underpriced operation SILENTLY (spec 060). `min_gas_price_wei` is 0
    // everywhere else, so this is a no-op on every other chain.
    let floor = min_gas_price_wei(chain_id);
    let price = derived.unwrap_or(ChainGasPrice {
        gas_price: 5_000_000_000u128.max(floor),
        base_fee: 5_000_000_000u128.max(floor),
        priority_fee: 0,
        tip_measured: false,
    });
    if let Ok(mut cache) = GAS_PRICES.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(chain_id, (price, Instant::now()));
    }
    price
}

/// Forget the cached price before a submit — stale prices on a volatile
/// chain are "gas price too low" rejections (`_gasPriceCache.delete`).
pub fn forget_gas_price(chain_id: u32) {
    if let Ok(mut cache) = GAS_PRICES.lock()
        && let Some(map) = cache.as_mut()
    {
        map.remove(&chain_id);
    }
}

/// `verifyChainReady`: the EntryPoint exists here, once per chain.
pub fn verify_chain_ready(chain_id: u32) -> Result<(), String> {
    if let Ok(cache) = CHAIN_READY.lock()
        && cache
            .as_ref()
            .is_some_and(|map| map.contains_key(&chain_id))
    {
        return Ok(());
    }
    if !is_deployed(ENTRY_POINT, chain_id)? {
        return Err("This network is not ready yet. Required smart contracts (EntryPoint) are not deployed. Please activate this network in Settings → Transaction Services.".to_owned());
    }
    if let Ok(mut cache) = CHAIN_READY.lock() {
        cache.get_or_insert_with(HashMap::new).insert(chain_id, ());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::tx_tracker::SAFE_EXECUTION_FAILURE_TOPIC;
    use vela_core::primitives::keccak256;

    /// Issue #483: the account read says its cause the way the fee machine
    /// words it — the chain's nodes out of reach (rate-limited or not), or a
    /// pool that could not take the read at all, which is Vela's own fault
    /// and never "can't reach the chain".
    #[test]
    fn the_account_read_names_its_real_cause() {
        assert_eq!(
            deployment_of(Ok(json!({"result": "0x6080"}))),
            DeploymentRead::Read { deployed: true }
        );
        assert_eq!(
            deployment_of(Ok(json!({"result": "0x"}))),
            DeploymentRead::Read { deployed: false }
        );
        for rate_limited in [false, true] {
            assert_eq!(
                deployment_of(Err(pool::PoolError::Failed { rate_limited })),
                DeploymentRead::Unreachable { rate_limited }
            );
        }
        assert_eq!(
            deployment_of(Ok(
                json!({"error": {"code": -32000, "message": "header not found"}})
            )),
            DeploymentRead::Unreachable {
                rate_limited: false
            }
        );
        assert!(matches!(
            deployment_of(Err(pool::PoolError::Unavailable)),
            DeploymentRead::Internal { .. }
        ));
    }

    /// 083: how an operation ended, from the bundle transaction's receipt —
    /// the EntryPoint's `UserOperationEvent` for THAT op, and no Safe
    /// `ExecutionFailure` in its own execution (#D1). A bundle carries other
    /// operations, and other contracts emit look-alike logs; neither is this
    /// op's outcome.
    #[test]
    fn an_operations_outcome_is_its_own_entry_point_event() {
        let topic = "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f";
        assert_eq!(
            to_hex(&keccak256(USER_OPERATION_EVENT.as_bytes()), true),
            topic
        );
        let ours = format!("0x{}", "ab".repeat(32));
        let theirs = format!("0x{}", "ef".repeat(32));
        let word = |value: u8| format!("{value:064x}");
        let entry_point = ENTRY_POINT.to_lowercase();
        let event = |op: &str, address: &str, success: u8| {
            json!({
                "address": address,
                "topics": [topic, op, format!("0x{:064x}", 1), format!("0x{:064x}", 0)],
                "data": format!("0x{}{}{}{}", word(7), word(success), word(9), word(9)),
            })
        };
        let before_execution = json!({
            "address": entry_point,
            "topics": [to_hex(&keccak256(b"BeforeExecution()"), true)],
            "data": "0x",
        });
        let safe_failure = json!({
            "address": "0x88cca0eedbf2c4426110bbfc998f048689266894",
            "topics": [SAFE_EXECUTION_FAILURE_TOPIC],
            "data": format!("0x{}{}", word(1), word(0)),
        });
        let receipt = |logs: Vec<Value>| json!({ "status": "0x1", "logs": logs });

        let reverted = receipt(vec![
            event(&theirs, &entry_point, 1),
            event(&ours, &entry_point, 0),
        ]);
        assert_eq!(
            user_op_outcome(&reverted, &ours),
            Some(false),
            "status 0x1, and still reverted"
        );
        assert_eq!(user_op_outcome(&reverted, &theirs), Some(true));
        let ours_in_capitals = format!("0x{}", "AB".repeat(32));
        assert_eq!(
            user_op_outcome(
                &receipt(vec![event(&ours, ENTRY_POINT, 1)]),
                &ours_in_capitals
            ),
            Some(true),
            "hex case is not identity"
        );

        // #D1: the EntryPoint says success, the Safe inside says it failed.
        // Only in the op's own execution — the logs since the last boundary.
        let hollow = receipt(vec![
            safe_failure.clone(), // validation-phase noise, before execution
            before_execution,
            safe_failure,
            event(&theirs, &entry_point, 1),
            event(&ours, &entry_point, 1),
        ]);
        assert_eq!(
            user_op_outcome(&hollow, &theirs),
            Some(false),
            "its execution logged the failure"
        );
        assert_eq!(
            user_op_outcome(&hollow, &ours),
            Some(true),
            "another op's failure is not this one's"
        );

        let impostor = receipt(vec![event(
            &ours,
            "0x1111111111111111111111111111111111111111",
            1,
        )]);
        assert_eq!(user_op_outcome(&impostor, &ours), None);
        assert_eq!(user_op_outcome(&receipt(Vec::new()), &ours), None);
        assert_eq!(user_op_outcome(&Value::Null, &ours), None);
    }

    // -- live (`cargo test executor::chain -- --ignored --test-threads=1`) --

    const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    /// The golden Safe is deployed on Gnosis and has a nonce; the EntryPoint
    /// is there; the gas signals carry a tip (Gnosis's whole price).
    #[test]
    #[ignore = "real network"]
    fn live_the_golden_safe_reads_on_gnosis() {
        verify_chain_ready(100).unwrap_or_else(|e| unreachable!("{e}"));
        assert!(is_deployed(GOLDEN, 100).unwrap_or_else(|e| unreachable!("{e}")));
        let nonce = nonce(GOLDEN, 100).unwrap_or_else(|e| unreachable!("{e}"));
        println!("golden nonce: {nonce}");
        assert!(parse_hex_quantity(Some(&nonce)).is_ok());
        let signals = raw_gas_signals(100, true);
        println!("gas signals: {signals:?}");
        assert!(signals.eth_gas_price.is_some());
        let price = chain_gas_price(100);
        println!("derived: {price:?}");
        assert!(price.gas_price > 0);
    }
}
