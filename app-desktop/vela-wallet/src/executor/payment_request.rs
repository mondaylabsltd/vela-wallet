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

/// A payment link (`/pay?to=…&chain=…`) — the one the receive screen shares
/// (`build_pay_link`), opened here: by the OS (`velawallet://pay?…`), pasted
/// or scanned (`https://getvela.app/pay?…`).
///
/// `None`: not a payment link at all, the caller reads it another way.
/// `Some(None)`: a payment link the core refused — the untrusted `/pay`
/// grammar is the core's (`LinkOpened`, `validate_pay_query`: a strict
/// amount, a real address, a numeric chain), so nothing here parses an
/// amount. `Some(Some(pay))`: the request, as the core read it.
#[must_use]
pub fn pay_link(text: &str) -> Option<Option<vela_core::app::payment_request::PayRequest>> {
    let url = url::Url::parse(text.trim()).ok()?;
    let pay_path = |path: &str| {
        matches!(path.trim_end_matches('/'), "" | "/pay")
            || path.trim_end_matches('/').ends_with("/pay")
    };
    let is_pay = match url.scheme() {
        "velawallet" => url.host_str() == Some("pay"),
        "https" => {
            url.host_str()
                .is_some_and(|host| host == "getvela.app" || host.ends_with(".getvela.app"))
                && pay_path(url.path())
        }
        _ => false,
    };
    if !is_pay || !url.query_pairs().any(|(key, _)| key == "to") {
        return None;
    }
    let get = |name: &str| {
        url.query_pairs()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.into_owned())
    };
    let mut core = crate::core_host::CoreHost::<PaymentRequest>::new();
    // `link_opened` asks the shell nothing: one dispatch, then the view.
    let _ = core.dispatch(Event::LinkOpened {
        to: get("to"),
        chain: get("chain"),
        token: get("token"),
        amount: get("amount"),
        sym: get("sym"),
        dec: get("dec"),
        net: get("net"),
    });
    let view = core.view();
    Some(if view.pay_valid == Some(true) {
        view.pay
    } else {
        None
    })
}

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

    /// WIRE-AUDIT §2 (`LinkOpened`): a payment link is read by the core's
    /// `/pay` grammar — a valid one is its request, a malformed one is
    /// refused, and anything else is not a payment link at all.
    #[test]
    fn a_pay_link_is_read_by_the_cores_grammar() {
        const TO: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let link = format!("https://getvela.app/pay?to={TO}&chain=100&amount=1.5&sym=xDAI&dec=18");
        let pay = pay_link(&link)
            .flatten()
            .unwrap_or_else(|| unreachable!("a valid link"));
        assert_eq!(pay.recipient, TO);
        assert_eq!(pay.chain_id, 100);
        assert_eq!(pay.amount_base.as_deref(), Some("1500000000000000000"));
        // The OS's own spelling, on a wallet that registered the scheme.
        let scheme = format!("velawallet://pay?to={TO}&chain=8453");
        assert_eq!(
            pay_link(&scheme).flatten().map(|pay| pay.chain_id),
            Some(8453)
        );
        // Refused by the core: "1e18" is not an amount a person typed.
        let bad = format!("https://wallet.getvela.app/pay?to={TO}&chain=1&amount=1e18");
        assert_eq!(pay_link(&bad), Some(None));
        // Not a payment link: another site, the signer's callback, a bare address.
        assert_eq!(
            pay_link(&format!("https://evil.example/pay?to={TO}&chain=1")),
            None
        );
        assert_eq!(pay_link("velawallet://sign-result?t=x"), None);
        assert_eq!(pay_link(TO), None);
    }
}
