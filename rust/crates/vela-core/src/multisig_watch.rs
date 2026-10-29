//! What an execution in flight says, read from the facts the shell gathered
//! (round 4: state, sync and history).
//!
//! A pure rule — the shell asks the chain and the relay, the core says what
//! the answers mean. From the chain's nonce, the transaction that executed
//! the safeTxHash (if any) and what the relay says of this device's own
//! operation ([`execution_verdict`]): whether it was executed here, executed
//! first by another member, replaced by another transaction at its number,
//! refused, still on its way, or past its time — never "Executing…" without
//! end.
//!
//! Round 5: a verdict never contradicts this device's own receipt. A log
//! lookup is an index a lagging node may not have caught up with, so the
//! receipt of this device's own landed transaction ([`OurReceipt`]) is read
//! first and wins: a transaction whose own receipt shows the
//! `ExecutionSuccess` was executed here, whatever a lookup elsewhere says;
//! and without that receipt read, a lookup's "none" never becomes "another
//! transaction ran first" — that is said only once the receipt shows it did
//! not execute.
//!
//! (Reading a history in windows uses the pool's own rule for a window a node
//! refuses as too wide, `app::rpc_pool::get_logs_range_cap`.)

use serde::{Deserialize, Serialize};

/// What a lookup of the transaction that executed a safeTxHash found.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExecutedIn {
    /// Its `ExecutionSuccess` is in this transaction.
    #[serde(rename_all = "camelCase")]
    Found { tx_hash: String },
    /// The chain shows none (read to the head).
    None,
    /// The chain could not be read.
    Unknown,
}

/// What the relay says of this device's own operation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum OurOp {
    /// On chain, in this transaction (whatever its calls did).
    #[serde(rename_all = "camelCase")]
    Landed { tx_hash: String },
    /// The relay holds it, not on chain yet.
    Pending,
    /// The relay refused it before any block, or never took it.
    Refused,
    /// The relay did not answer.
    Unknown,
}

/// What the receipt of this device's own landed transaction shows of the
/// transaction it executes: the one fact about it no log index can lag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OurReceipt {
    /// It holds the multisig's `ExecutionSuccess` for this transaction.
    Executed,
    /// It was read, and holds none: this device's transaction did not execute it.
    NotExecuted,
    /// Not read (no landed transaction, or no node answered for it).
    #[default]
    Unread,
}

/// The facts the shell gathered about one execution in flight.
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFacts {
    /// Whether the chain's nonce is past the transaction's (`None`: unread).
    pub nonce_used: Option<bool>,
    pub executed_in: ExecutedIn,
    pub our_op: OurOp,
    /// What the receipt of this device's landed transaction shows (read only
    /// once [`OurOp::Landed`]; absent from an older shell: unread).
    #[serde(default)]
    pub receipt: OurReceipt,
    /// How long it has been watched, and how long it may be.
    pub elapsed_ms: u64,
    pub limit_ms: u64,
}

/// Where an execution in flight stands.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExecutionVerdict {
    /// Executed by this device's operation.
    #[serde(rename_all = "camelCase")]
    Executed { tx_hash: String },
    /// Another member's transaction executed it first. `paid`: this device's
    /// operation landed too (and did nothing but pay its fee).
    #[serde(rename_all = "camelCase")]
    ExecutedByOther { tx_hash: String, paid: bool },
    /// Another transaction at its number ran first: it can never run.
    #[serde(rename_all = "camelCase")]
    Replaced { paid: bool },
    /// This device's operation landed and did not execute it (its call failed).
    #[serde(rename_all = "camelCase")]
    Failed { tx_hash: String },
    /// The relay refused it, nothing landed: nothing was sent, try again.
    NotSent,
    /// Its number is used, and the chain cannot say by what right now.
    Unknown,
    /// Still on its way.
    Waiting,
    /// Past its time, its number unused, nothing landed: it may still land.
    TimedOut,
}

/// The rule: what the gathered facts say of an execution in flight.
pub fn execution_verdict(f: &ExecutionFacts) -> ExecutionVerdict {
    use ExecutionVerdict as V;
    let used = f.nonce_used == Some(true);
    match (&f.our_op, &f.executed_in, f.receipt) {
        // Its own receipt says it executed: executed here, whatever a lookup
        // on another node (or a stale one) says.
        (OurOp::Landed { tx_hash }, _, OurReceipt::Executed) => V::Executed {
            tx_hash: tx_hash.clone(),
        },
        (OurOp::Landed { tx_hash }, ExecutedIn::Found { tx_hash: by }, _) => {
            if tx_hash.eq_ignore_ascii_case(by) {
                V::Executed {
                    tx_hash: tx_hash.clone(),
                }
            } else {
                V::ExecutedByOther {
                    tx_hash: by.clone(),
                    paid: true,
                }
            }
        }
        // Landed and did not execute it: the number used by another
        // transaction, or its own call failed.
        (OurOp::Landed { .. }, ExecutedIn::None, OurReceipt::NotExecuted) if used => {
            V::Replaced { paid: true }
        }
        (
            OurOp::Landed { tx_hash },
            ExecutedIn::None | ExecutedIn::Unknown,
            OurReceipt::NotExecuted,
        ) if !used => V::Failed {
            tx_hash: tx_hash.clone(),
        },
        // Landed, its receipt unread: no lookup's "none" says what it did.
        (OurOp::Landed { .. }, _, _) if used => V::Unknown,
        (OurOp::Landed { .. }, _, _) => V::Waiting,
        (_, ExecutedIn::Found { tx_hash }, _) => V::ExecutedByOther {
            tx_hash: tx_hash.clone(),
            paid: false,
        },
        (_, ExecutedIn::None, _) if used => V::Replaced { paid: false },
        (_, ExecutedIn::Unknown, _) if used => V::Unknown,
        (OurOp::Refused, _, _) => V::NotSent,
        _ if f.elapsed_ms >= f.limit_ms => V::TimedOut,
        _ => V::Waiting,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn facts(nonce_used: Option<bool>, executed_in: ExecutedIn, our_op: OurOp) -> ExecutionFacts {
        ExecutionFacts {
            nonce_used,
            executed_in,
            our_op,
            receipt: OurReceipt::Unread,
            elapsed_ms: 5_000,
            limit_ms: 180_000,
        }
    }

    fn found(h: &str) -> ExecutedIn {
        ExecutedIn::Found { tx_hash: h.into() }
    }

    #[test]
    fn executed_here_when_its_execution_is_this_devices_operation() {
        let v = execution_verdict(&facts(
            Some(true),
            found("0xAA"),
            OurOp::Landed {
                tx_hash: "0xaa".into(),
            },
        ));
        assert_eq!(
            v,
            ExecutionVerdict::Executed {
                tx_hash: "0xaa".into()
            }
        );
    }

    /// r4b-1: the race's loser is told within one round, never "Executing…".
    #[test]
    fn the_race_loser_is_told_another_member_executed_it_and_nothing_was_sent() {
        for op in [OurOp::Pending, OurOp::Refused, OurOp::Unknown] {
            assert_eq!(
                execution_verdict(&facts(Some(true), found("0xbb"), op)),
                ExecutionVerdict::ExecutedByOther {
                    tx_hash: "0xbb".into(),
                    paid: false
                }
            );
        }
        // Its own operation landed too, apart: it paid, and is told so.
        assert_eq!(
            execution_verdict(&facts(
                Some(true),
                found("0xbb"),
                OurOp::Landed {
                    tx_hash: "0xcc".into()
                }
            )),
            ExecutionVerdict::ExecutedByOther {
                tx_hash: "0xbb".into(),
                paid: true
            }
        );
    }

    #[test]
    fn a_used_number_with_no_execution_is_a_replacement_and_unread_is_unknown() {
        assert_eq!(
            execution_verdict(&facts(Some(true), ExecutedIn::None, OurOp::Pending)),
            ExecutionVerdict::Replaced { paid: false }
        );
        assert_eq!(
            execution_verdict(&facts(Some(true), ExecutedIn::Unknown, OurOp::Pending)),
            ExecutionVerdict::Unknown
        );
    }

    #[test]
    fn refused_is_not_sent_and_the_wait_is_bounded() {
        assert_eq!(
            execution_verdict(&facts(Some(false), ExecutedIn::None, OurOp::Refused)),
            ExecutionVerdict::NotSent
        );
        assert_eq!(
            execution_verdict(&facts(Some(false), ExecutedIn::None, OurOp::Pending)),
            ExecutionVerdict::Waiting
        );
        let mut late = facts(None, ExecutedIn::Unknown, OurOp::Pending);
        late.elapsed_ms = 180_000;
        assert_eq!(execution_verdict(&late), ExecutionVerdict::TimedOut);
    }

    fn landed(h: &str, receipt: OurReceipt) -> ExecutionFacts {
        let mut f = facts(
            Some(true),
            ExecutedIn::None,
            OurOp::Landed { tx_hash: h.into() },
        );
        f.receipt = receipt;
        f
    }

    #[test]
    fn landed_without_executing_is_a_failure_with_its_fee_paid() {
        let mut f = landed("0xdd", OurReceipt::NotExecuted);
        f.nonce_used = Some(false);
        assert_eq!(
            execution_verdict(&f),
            ExecutionVerdict::Failed {
                tx_hash: "0xdd".into()
            }
        );
        // Its receipt unread, the number unused (a node behind): still on its way.
        f.receipt = OurReceipt::Unread;
        assert_eq!(execution_verdict(&f), ExecutionVerdict::Waiting);
    }

    /// Round 5: a successful execution was once said as "Another transaction
    /// ran first" — a lookup on a node behind the head found no
    /// `ExecutionSuccess`. Its own receipt is the word, and wins.
    #[test]
    fn its_own_receipt_is_never_contradicted_by_a_lookup_on_a_lagging_node() {
        // The lookup found none (the node is behind), the number is used: the
        // receipt shows it executed here.
        assert_eq!(
            execution_verdict(&landed("0xee", OurReceipt::Executed)),
            ExecutionVerdict::Executed {
                tx_hash: "0xee".into()
            }
        );
        // Nor does an unread history or an unused number on a stale node.
        for (used, executed_in) in [
            (Some(true), ExecutedIn::Unknown),
            (Some(false), ExecutedIn::None),
            (None, ExecutedIn::Unknown),
        ] {
            let mut f = landed("0xee", OurReceipt::Executed);
            f.nonce_used = used;
            f.executed_in = executed_in;
            assert_eq!(
                execution_verdict(&f),
                ExecutionVerdict::Executed {
                    tx_hash: "0xee".into()
                }
            );
        }
        // Its receipt unread: "none" is never "another transaction ran first".
        assert_eq!(
            execution_verdict(&landed("0xee", OurReceipt::Unread)),
            ExecutionVerdict::Unknown
        );
        let mut unread_history = landed("0xee", OurReceipt::Unread);
        unread_history.executed_in = ExecutedIn::Unknown;
        assert_eq!(
            execution_verdict(&unread_history),
            ExecutionVerdict::Unknown
        );
        // Read, and it did not execute: then another transaction did run first.
        assert_eq!(
            execution_verdict(&landed("0xee", OurReceipt::NotExecuted)),
            ExecutionVerdict::Replaced { paid: true }
        );
    }

    #[test]
    fn the_facts_read_from_the_shells_json() {
        let f: ExecutionFacts = serde_json::from_str(
            r#"{"nonceUsed":true,"executedIn":{"kind":"found","txHash":"0x1"},"ourOp":{"kind":"pending"},"elapsedMs":1,"limitMs":2}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(execution_verdict(&f)).unwrap(),
            serde_json::json!({"kind":"executedByOther","txHash":"0x1","paid":false})
        );
        let landed: ExecutionFacts = serde_json::from_str(
            r#"{"nonceUsed":true,"executedIn":{"kind":"none"},"ourOp":{"kind":"landed","txHash":"0x2"},"receipt":"executed","elapsedMs":1,"limitMs":2}"#,
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(execution_verdict(&landed)).unwrap(),
            serde_json::json!({"kind":"executed","txHash":"0x2"})
        );
    }
}
