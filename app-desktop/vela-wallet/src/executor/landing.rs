//! Has a submitted operation landed — and how (083).
//!
//! A dApp's `eth_sendTransaction` resolves to a transaction hash, and a site
//! reads that hash on its OWN node. Two things follow, and both were wrong on
//! the device (Uniswap, USDC → ETH):
//!
//! - **The bundle transaction's status is not the operation's.** A bundle
//!   whose operation reverted still succeeds (status `0x1`); only the
//!   EntryPoint's `UserOperationEvent.success` says what happened to the
//!   operation. A receipt saying `success: false` answered the page with the
//!   hash anyway, and the site called a swap done that moved nothing.
//! - **The userOpHash is not a transaction.** No node knows it. Answered with
//!   it when a 90 s window ran out, the site waited on "pending" forever.
//!
//! So the wait here ends only on an OUTCOME — [`Landing`] — or on the cap
//! (`sign_request::PAGE_WAIT_CAP_MS`, the core's number), and the page's
//! answer is the core's to choose from it.
//!
//! "Executed" is the tracker's rule, not a looser one: `success: true` AND no
//! Safe `ExecutionFailure` in the operation's OWN execution logs (spec 038
//! #D1) — a bundle neighbour's failure is not this operation's.
//!
//! What is asked, in order of cost:
//!
//! 1. `eth_getUserOperationReceipt` — the transaction hash and `success`
//!    together, every poll.
//! 2. `eth_getUserOperationByHash` — the relay's own record of the op, for
//!    the transaction that carried it, should the receipt lag; that
//!    transaction's receipt on the node then says how the op ended. Every
//!    12 s, the tracker's status cadence.
//! 3. `eth_getUserOperationStatus` — a relay that REFUSED the operation after
//!    accepting it (Vela's extension; an older relay answers -32601 and this
//!    says nothing). Every 12 s too.

use std::time::{Duration, Instant};

use vela_core::app::tx_tracker::{
    STATUS_POLL_INTERVAL_MS, TrackLifecycle, WAIT_WINDOW_MS, op_execution_failed,
    receipt_interval_ms,
};

use crate::executor::{chain, relay};

/// How a submitted operation ended, as far as its page's answer goes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Landing {
    /// Included and executed: the hash of the transaction that carried it.
    Landed(String),
    /// Included and REVERTED: the hash of the transaction that carried it.
    Reverted(String),
    /// The relay refused it after accepting it — it will not land.
    Refused,
}

impl Landing {
    fn of(success: bool, tx_hash: String) -> Self {
        if success {
            Self::Landed(tx_hash)
        } else {
            Self::Reverted(tx_hash)
        }
    }
}

/// The answers one look can get, as [`look_with`] reads them — the wire's
/// business stays in `relay` and `chain`.
pub struct Sources<R, B, T, S>
where
    R: FnOnce() -> Option<(bool, String)>,
    B: FnOnce() -> Option<String>,
    T: FnOnce(&str) -> Option<bool>,
    S: FnOnce() -> Option<TrackLifecycle>,
{
    /// The receipt: `(success, transaction hash)`.
    pub receipt: R,
    /// `eth_getUserOperationByHash`'s transaction hash.
    pub by_hash: B,
    /// That transaction's receipt on the node: the op's `success`.
    pub in_transaction: T,
    /// The relay's lifecycle for the op.
    pub status: S,
}

/// One look at an operation. `deep` adds the two slower questions (by-hash,
/// status); without it only the receipt is asked. `None` is "not yet" —
/// never "failed": a relay that could not be reached is not an operation
/// that did not happen.
pub fn look_with<R, B, T, S>(sources: Sources<R, B, T, S>, deep: bool) -> Option<Landing>
where
    R: FnOnce() -> Option<(bool, String)>,
    B: FnOnce() -> Option<String>,
    T: FnOnce(&str) -> Option<bool>,
    S: FnOnce() -> Option<TrackLifecycle>,
{
    if let Some((success, tx_hash)) = (sources.receipt)() {
        return Some(Landing::of(success, tx_hash));
    }
    if !deep {
        return None;
    }
    // A transaction hash alone is not an outcome: the op inside it may have
    // reverted. Without the node's word on that, keep waiting.
    if let Some(tx_hash) = (sources.by_hash)()
        && let Some(success) = (sources.in_transaction)(&tx_hash)
    {
        return Some(Landing::of(success, tx_hash));
    }
    ((sources.status)() == Some(TrackLifecycle::Rejected)).then_some(Landing::Refused)
}

/// [`look_with`] on the wire, every question within what is left of
/// `budget`.
fn look(user_op_hash: &str, chain_id: u32, budget: Duration, deep: bool) -> Option<Landing> {
    let deadline = Instant::now() + budget;
    let left = move || deadline.saturating_duration_since(Instant::now());
    look_with(
        Sources {
            receipt: || {
                relay::user_op_receipt_within(user_op_hash, chain_id, budget)
                    .resolution
                    .map(|resolution| (executed(&resolution, user_op_hash), resolution.tx_hash))
            },
            by_hash: || relay::user_op_transaction_within(user_op_hash, chain_id, left()),
            in_transaction: |tx_hash: &str| {
                chain::user_op_outcome_in(tx_hash, user_op_hash, chain_id, left())
            },
            status: || {
                relay::user_op_status_within(user_op_hash, chain_id, left())
                    .map(|(status, _)| status)
            },
        },
        deep,
    )
}

/// Did the operation a receipt describes EXECUTE? The EntryPoint's `success`
/// — and no Safe `ExecutionFailure` in its own execution, the success the
/// EntryPoint counts while nothing happened (spec 038 #D1, the tracker's
/// rule, applied to the page's answer too). The receipt's logs are the whole
/// bundle's: another account's failure in it must not fail this op, or the
/// page hears "reverted" for a swap that happened and the user sends it again.
pub fn executed(resolution: &relay::Resolution, user_op_hash: &str) -> bool {
    resolution.confirmed && !op_execution_failed(&resolution.logs, user_op_hash)
}

/// How long to wait before the next look, `elapsed` into the wait: the
/// tracker's own cadence — every 3 s while its window is open, every 12 s
/// after — so the page's wait and the tracker ask the relay alike.
fn cadence(elapsed: Duration) -> Duration {
    let age_ms = elapsed.as_secs_f64() * 1000.0;
    Duration::from_secs_f64(receipt_interval_ms(age_ms < WAIT_WINDOW_MS, age_ms) / 1000.0)
}

/// Wait for `user_op_hash` to land, for at most `cap`. `None` when the cap
/// went by with no outcome.
pub fn await_landing(user_op_hash: &str, chain_id: u32, cap: Duration) -> Option<Landing> {
    let every_deep = Duration::from_secs_f64(STATUS_POLL_INTERVAL_MS / 1000.0);
    let mut next_deep = every_deep;
    let started = Instant::now();
    wait_within(cap, cadence, |left| {
        // The first deep look waits one interval, as the tracker's status
        // poll does: "not landed yet" is by far the common case.
        let deep = started.elapsed() >= next_deep;
        if deep {
            next_deep = started.elapsed() + every_deep;
        }
        look(user_op_hash, chain_id, left, deep)
    })
}

/// Ask `poll` until it answers or `budget` is spent, sleeping `every(elapsed)`
/// between asks — and giving each ask only what is LEFT of the budget (spec
/// 079, device-found on Android: with the relay unreachable one poll hung for
/// its own timeouts and retries, and the page waited 268 s for a two-minute
/// wait). The page is answered on time; the tracker keeps following the
/// operation after.
pub fn wait_within<T>(
    budget: Duration,
    every: impl Fn(Duration) -> Duration,
    mut poll: impl FnMut(Duration) -> Option<T>,
) -> Option<T> {
    let started = Instant::now();
    let deadline = started + budget;
    loop {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        if let Some(answer) = poll(left) {
            return Some(answer);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return None;
        }
        std::thread::sleep(every(started.elapsed()).min(left));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TX: &str = "0x09c35478682ddce770c02e4aed27c9f0260f7b2f54ae6a7a194f50b70d604c9f";

    /// What each source answers in one test look.
    #[derive(Clone, Default)]
    struct World {
        receipt: Option<(bool, &'static str)>,
        by_hash: Option<&'static str>,
        in_transaction: Option<bool>,
        status: Option<TrackLifecycle>,
    }

    fn look_in(world: &World, deep: bool) -> Option<Landing> {
        look_with(
            Sources {
                receipt: || world.receipt.map(|(ok, tx)| (ok, tx.to_owned())),
                by_hash: || world.by_hash.map(str::to_owned),
                in_transaction: |tx: &str| {
                    assert_eq!(tx, TX, "the node is asked about THAT transaction");
                    world.in_transaction
                },
                status: || world.status,
            },
            deep,
        )
    }

    /// S2: the receipt's `success` decides — a receipt is not a success.
    #[test]
    fn a_receipt_says_landed_or_reverted() {
        let landed = World {
            receipt: Some((true, TX)),
            ..World::default()
        };
        assert_eq!(
            look_in(&landed, false),
            Some(Landing::Landed(TX.to_owned()))
        );
        let reverted = World {
            receipt: Some((false, TX)),
            ..World::default()
        };
        assert_eq!(
            look_in(&reverted, false),
            Some(Landing::Reverted(TX.to_owned())),
            "a reverted op's receipt is still a receipt — and not a success"
        );
    }

    /// #D1 on the page's answer too: the EntryPoint's `success: true` over a
    /// Safe that logged `ExecutionFailure` moved nothing, and is no success —
    /// when the failure is in THIS op's execution. A bundle neighbour's is
    /// not (083 receipts review).
    #[test]
    fn a_success_the_safe_did_not_execute_is_not_one() {
        use vela_core::app::token_trust::TrustReceiptLog;
        use vela_core::app::tx_tracker::SAFE_EXECUTION_FAILURE_TOPIC;
        use vela_core::primitives::{keccak256, to_hex};
        use vela_core::safe::ENTRY_POINT;
        let ours = format!("0x{}", "ab".repeat(32));
        let theirs = format!("0x{}", "ef".repeat(32));
        let resolution = |confirmed: bool, logs: Vec<TrustReceiptLog>| relay::Resolution {
            confirmed,
            tx_hash: TX.to_owned(),
            sender: None,
            logs,
        };
        assert!(executed(&resolution(true, Vec::new()), &ours));
        assert!(!executed(&resolution(false, Vec::new()), &ours));
        let failure = TrustReceiptLog {
            address: "0x88cca0eedbf2c4426110bbfc998f048689266894".to_owned(),
            topics: vec![SAFE_EXECUTION_FAILURE_TOPIC.to_owned()],
            data: "0x".to_owned(),
        };
        assert!(!executed(&resolution(true, vec![failure.clone()]), &ours));

        let word = |value: u8| format!("{value:064x}");
        let event = |op: &str| TrustReceiptLog {
            address: ENTRY_POINT.to_owned(),
            topics: vec![
                to_hex(
                    &keccak256(
                        b"UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)",
                    ),
                    true,
                ),
                op.to_owned(),
                word(1),
                word(0),
            ],
            data: format!("0x{}{}{}{}", word(7), word(1), word(9), word(9)),
        };
        let bundle = vec![failure, event(&theirs), event(&ours)];
        assert!(
            executed(&resolution(true, bundle.clone()), &ours),
            "a neighbour's failure in the bundle is not ours"
        );
        assert!(!executed(&resolution(true, bundle), &theirs));
    }

    /// S3: the by-hash lookup names the transaction; the node's receipt of it
    /// says how the op ended. A hash with no word on the op is not an answer.
    #[test]
    fn a_transaction_hash_alone_answers_nothing() {
        let named = World {
            by_hash: Some(TX),
            ..World::default()
        };
        assert_eq!(look_in(&named, true), None, "no outcome yet: keep waiting");
        assert_eq!(
            look_in(
                &World {
                    in_transaction: Some(false),
                    ..named.clone()
                },
                true
            ),
            Some(Landing::Reverted(TX.to_owned()))
        );
        let executed = World {
            in_transaction: Some(true),
            ..named
        };
        assert_eq!(
            look_in(&executed, true),
            Some(Landing::Landed(TX.to_owned()))
        );
        assert_eq!(
            look_in(&executed, false),
            None,
            "a shallow look asks the receipt alone"
        );
    }

    /// A relay that refused the op after accepting it ends the wait; any
    /// other lifecycle — or none, from a relay without the method — does not.
    #[test]
    fn only_a_refusal_ends_the_wait_without_a_transaction() {
        let refused = World {
            status: Some(TrackLifecycle::Rejected),
            ..World::default()
        };
        assert_eq!(look_in(&refused, true), Some(Landing::Refused));
        for status in [
            None,
            Some(TrackLifecycle::Queued),
            Some(TrackLifecycle::Submitted),
            Some(TrackLifecycle::NotFound),
            Some(TrackLifecycle::Failed),
        ] {
            let world = World {
                status,
                ..World::default()
            };
            assert_eq!(look_in(&world, true), None, "{status:?}");
        }
    }

    /// The tracker's cadence: 3 s while its window is open, 12 s after.
    #[test]
    fn the_wait_asks_as_often_as_the_tracker() {
        assert_eq!(cadence(Duration::ZERO), Duration::from_secs(3));
        assert_eq!(cadence(Duration::from_secs(119)), Duration::from_secs(3));
        assert_eq!(cadence(Duration::from_secs(120)), Duration::from_secs(12));
        assert_eq!(cadence(Duration::from_secs(599)), Duration::from_secs(12));
    }

    #[test]
    fn every_poll_gets_only_what_is_left_of_the_wait() {
        let budget = Duration::from_millis(300);
        let started = Instant::now();
        let mut given = Vec::new();
        let answer: Option<()> = wait_within(
            budget,
            |_| Duration::from_millis(40),
            |left| {
                given.push(left);
                // A relay that holds every call for as long as it is allowed.
                std::thread::sleep(left.min(Duration::from_millis(120)));
                None
            },
        );
        assert!(answer.is_none(), "no receipt is not a failure, it is none");
        assert!(
            started.elapsed() < budget + Duration::from_millis(150),
            "the wait ends on time: {:?}",
            started.elapsed()
        );
        assert!(given.len() >= 2, "it asks again while there is time");
        assert!(given.iter().all(|left| *left <= budget));
        assert!(
            given.windows(2).all(|pair| pair[1] < pair[0]),
            "each ask gets less: {given:?}"
        );
        // An answer ends the wait at once.
        assert_eq!(
            wait_within(budget, |_| Duration::from_millis(40), |_| Some("0xtx")),
            Some("0xtx")
        );
    }
}
