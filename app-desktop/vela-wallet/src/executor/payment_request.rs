//! The only place the `payment_request` machine touches the outside world.
//!
//! Two operations, one flag: has this account seen the "a payment link is
//! public" warning. Per account, because a second wallet on the same machine is
//! a different person's understanding.

use gpui::App;
use serde_json::Value;

use vela_core::app::payment_request::{
    Event, PaymentRequest, PaymentRequestOperation, PaymentRequestShellResult,
};

use crate::executor::storage;
use crate::resident::{Answer, Machine};
use crate::session;

/// `vela.payAck.<account>` — one key per account, the Expo shape.
fn ack_key(account: &str) -> String {
    format!("vela.payAck.{}", account.to_lowercase())
}

impl Machine for PaymentRequest {
    const LABEL: &'static str = "payment_request";

    fn boot_event(cx: &App) -> Event {
        let view = session::view(cx);
        Event::Start {
            account: view.address.clone(),
            recipient: view.address,
            // The share base. A pay link points at the web wallet, which is
            // where a recipient can actually open it — a desktop URL scheme
            // would be a link most people cannot follow.
            base_url: "https://getvela.app".to_owned(),
        }
    }

    fn perform(
        operation: &PaymentRequestOperation,
    ) -> Answer<PaymentRequestShellResult, Self::Event> {
        match operation {
            PaymentRequestOperation::ReadAck { account } => {
                Answer::Now(PaymentRequestShellResult::AckFlag {
                    // Absent means "never acknowledged", and so does unreadable:
                    // showing the warning twice is a small cost, skipping it is
                    // not.
                    acknowledged: storage::read_value(&ack_key(account))
                        .ok()
                        .flatten()
                        .and_then(|value| match value {
                            Value::Bool(flag) => Some(flag),
                            Value::String(text) => Some(text == "1"),
                            _ => None,
                        })
                        .unwrap_or(false),
                })
            }
            PaymentRequestOperation::WriteAck { account } => {
                let _ = storage::write_value(&ack_key(account), Value::String("1".to_owned()));
                Answer::Now(PaymentRequestShellResult::AckWritten)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn perform(operation: PaymentRequestOperation) -> PaymentRequestShellResult {
        match PaymentRequest::perform(&operation) {
            Answer::Now(result) => result,
            _ => unreachable!("both operations are local"),
        }
    }

    /// The flag is per account, and an unread one means "not yet".
    #[test]
    fn the_acknowledgement_is_per_account_and_defaults_to_unseen() {
        storage::tests::with_temp_state("pay-ack", || {
            let a = "0xAAA".to_owned();
            let b = "0xBBB".to_owned();

            match perform(PaymentRequestOperation::ReadAck { account: a.clone() }) {
                PaymentRequestShellResult::AckFlag { acknowledged } => {
                    assert!(!acknowledged, "nobody has seen it yet");
                }
                other => unreachable!("wrong variant: {other:?}"),
            }

            perform(PaymentRequestOperation::WriteAck { account: a.clone() });

            match perform(PaymentRequestOperation::ReadAck { account: a }) {
                PaymentRequestShellResult::AckFlag { acknowledged } => assert!(acknowledged),
                other => unreachable!("wrong variant: {other:?}"),
            }
            // The other account has not seen it. A second wallet on this machine
            // is a different person's understanding.
            match perform(PaymentRequestOperation::ReadAck { account: b }) {
                PaymentRequestShellResult::AckFlag { acknowledged } => assert!(!acknowledged),
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }

    /// Case is not identity: the same account written one way must read back
    /// the other.
    #[test]
    fn the_key_is_case_insensitive() {
        storage::tests::with_temp_state("pay-ack-case", || {
            perform(PaymentRequestOperation::WriteAck {
                account: "0xAbCd".to_owned(),
            });
            match perform(PaymentRequestOperation::ReadAck {
                account: "0xabcd".to_owned(),
            }) {
                PaymentRequestShellResult::AckFlag { acknowledged } => assert!(acknowledged),
                other => unreachable!("wrong variant: {other:?}"),
            }
        });
    }
}
