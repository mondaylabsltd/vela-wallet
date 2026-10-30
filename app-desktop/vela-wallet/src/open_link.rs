//! A URL the operating system hands this app — `velawallet://…` from the
//! scheme handler (macOS: an Apple Event to the running app; Windows and
//! Linux: an argument, relayed to the running wallet by `scheme_relay`).
//!
//! Two kinds, routed here and nowhere else:
//!
//! - the Trusted Signer's answer (`velawallet://sign-result?…`) — an event
//!   for a pending request, `trusted_signer::deliver_callback`;
//! - a payment link (`velawallet://pay?to=…&chain=…`, the `/pay` grammar the
//!   receive screen shares) — the wallet opens Send on it, the request read
//!   by the core (`payment_request::Event::LinkOpened`), exactly as a scanned
//!   or pasted link is.
//!
//! Nothing else is acted on. A payment link only ever opens the Send FORM,
//! prefilled: the person reviews and confirms as for any other send.

use std::sync::{Mutex, OnceLock};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};

/// The sending end, and the receiving end until the window takes it.
type Links = Mutex<(UnboundedSender<String>, Option<UnboundedReceiver<String>>)>;

/// The payment links that arrived, for the window to open.
fn pay_links() -> &'static Links {
    static LINKS: OnceLock<Links> = OnceLock::new();
    LINKS.get_or_init(|| {
        let (tx, rx) = unbounded();
        Mutex::new((tx, Some(rx)))
    })
}

/// Whether `url` is one this app answers — the second process's question
/// before it hands an argument to the running wallet.
#[must_use]
pub fn is_ours(url: &str) -> bool {
    url.starts_with(vela_core::trusted_signer::CALLBACK_URL)
        || crate::executor::payment_request::pay_link(url).is_some()
}

/// Route one URL the OS handed over. `true` when something took it.
pub fn deliver(url: &str) -> bool {
    if url.starts_with(vela_core::trusted_signer::CALLBACK_URL) {
        return crate::executor::trusted_signer::deliver_callback(url);
    }
    if crate::executor::payment_request::pay_link(url).is_some() {
        return pay_links()
            .lock()
            .is_ok_and(|links| links.0.unbounded_send(url.to_owned()).is_ok());
    }
    false
}

/// The window's end: the payment links, as they arrive. `None` once taken.
pub fn take_pay_links() -> Option<UnboundedReceiver<String>> {
    pay_links().lock().ok().and_then(|mut links| links.1.take())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_signers_answer_and_payment_links_are_ours() {
        assert!(is_ours(
            "velawallet://pay?to=0x88cCA0EeDbF2C4426110bbFc998F048689266894&chain=100"
        ));
        assert!(!is_ours("velawallet://settings"));
        assert!(!is_ours("https://example.org/"));
    }
}
