//! How this device signs by default (spec 071): the stored "Sign with" and
//! the Clear Signer's page — read raw, written as chosen.
//!
//! `fee_tier_pref`'s shape against the same store, so there is one way a
//! committed preference reaches disk here. Both values go back to the core
//! RAW: whether a string is a method this build offers, and whether an
//! address is a page a browser will sign on, are `sign_pref`'s calls — an
//! unknown method reads as "never chose" and is left on disk for the newer
//! build that wrote it.
//!
//! Both keys survive sign-out, which clears only the accounts and the active
//! index: how a person signs belongs to them and the device.

use gpui::App;
use serde_json::Value;

use vela_core::app::sign_pref::{Event, SignPref, SignPrefOperation, SignPrefShellResult};

use crate::executor::storage;
use crate::resident::{Answer, Machine};

impl Machine for SignPref {
    const LABEL: &'static str = "sign_pref";

    fn boot_event(_cx: &App) -> Event {
        Event::Refresh
    }

    fn perform(operation: &SignPrefOperation) -> Answer<SignPrefShellResult, Self::Event> {
        let stored = |key: &str| {
            storage::read_value(key)
                .ok()
                .flatten()
                .as_ref()
                .and_then(Value::as_str)
                .map(str::to_owned)
        };
        match operation {
            // Absent ALWAYS means "never chose" — including a read that
            // failed, which is why this cannot surface an error.
            SignPrefOperation::ReadStored => Answer::Now(SignPrefShellResult::Stored {
                method: stored(storage::KEY_SIGN_METHOD),
                signer_url: stored(storage::KEY_CLEAR_SIGNER_URL),
            }),
            // Best effort, both: the committed choice stays on screen either
            // way, and the next launch simply reads the old value.
            SignPrefOperation::WriteMethod { method } => {
                let _ =
                    storage::write_value(storage::KEY_SIGN_METHOD, Value::String(method.clone()));
                Answer::Now(SignPrefShellResult::Written)
            }
            SignPrefOperation::WriteSignerUrl { url } => {
                let _ = match url {
                    Some(url) => storage::write_value(
                        storage::KEY_CLEAR_SIGNER_URL,
                        Value::String(url.clone()),
                    ),
                    // The official page is the absence of a choice, so a
                    // later default can move without rewriting anybody's.
                    None => storage::remove_value(storage::KEY_CLEAR_SIGNER_URL),
                };
                Answer::Now(SignPrefShellResult::Written)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use crate::executor::storage::tests::with_temp_state;
    use vela_core::app::sign_pref::SignPrefView;
    use vela_core::clear_signer::DEFAULT_SIGNER_URL;

    fn drive(host: &mut CoreHost<SignPref>, event: Event) -> SignPrefView {
        let mut pending = host.dispatch(event);
        while let Some(next) = pending.pop() {
            let Answer::Now(result) = SignPref::perform(&next.operation) else {
                unreachable!("every sign_pref operation is local");
            };
            pending.extend(host.resolve(next.id, result));
        }
        host.view()
    }

    /// A fresh install signs the way the wallet always did, on the official
    /// page, and offers all five ways in the core's order.
    #[test]
    fn a_fresh_install_is_automatic_on_the_official_page() {
        with_temp_state("sign-pref-fresh", || {
            let mut host = CoreHost::<SignPref>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.method, "auto");
            assert!(!view.method_committed);
            assert_eq!(view.signer_url, DEFAULT_SIGNER_URL);
            assert!(view.signer_url_is_default);
            assert!(view.signer_uses_wallet_passkeys);
            assert_eq!(
                view.offered,
                vela_core::wallet_keys::SIGN_METHODS.map(str::to_owned)
            );
        });
    }

    /// Both choices are written under the keys every client reads, and the
    /// next launch reads them back.
    #[test]
    fn the_choices_persist_across_launches() {
        with_temp_state("sign-pref-persist", || {
            let mut host = CoreHost::<SignPref>::new();
            drive(&mut host, Event::Refresh);
            drive(
                &mut host,
                Event::MethodChosen {
                    method: "clear_signer".to_owned(),
                },
            );
            let view = drive(
                &mut host,
                Event::SignerUrlSubmitted {
                    text: "localhost:8140".to_owned(),
                },
            );
            assert_eq!(view.signer_url, "https://localhost:8140/");
            assert_eq!(
                storage::read_value(storage::KEY_SIGN_METHOD).ok().flatten(),
                Some(Value::String("clear_signer".to_owned()))
            );
            assert_eq!(
                storage::read_value(storage::KEY_CLEAR_SIGNER_URL)
                    .ok()
                    .flatten(),
                Some(Value::String("https://localhost:8140/".to_owned()))
            );

            let mut next = CoreHost::<SignPref>::new();
            let view = drive(&mut next, Event::Refresh);
            assert_eq!(view.method, "clear_signer");
            assert!(view.method_committed);
            assert_eq!(view.signer_url, "https://localhost:8140/");
            assert!(!view.signer_url_is_default);
            // Not a `getvela.app` page: it can show a request, not sign it.
            assert!(!view.signer_uses_wallet_passkeys);
        });
    }

    /// An address a browser will not sign on is refused with its reason and
    /// nothing is stored; the reset removes the key rather than pinning the
    /// official page.
    #[test]
    fn a_page_is_validated_and_the_reset_removes_it() {
        with_temp_state("sign-pref-page", || {
            let mut host = CoreHost::<SignPref>::new();
            drive(&mut host, Event::Refresh);
            let view = drive(
                &mut host,
                Event::SignerUrlSubmitted {
                    text: "http://192.168.1.4/".to_owned(),
                },
            );
            assert_eq!(view.signer_url_error.as_deref(), Some("insecure"));
            assert_eq!(view.signer_url, DEFAULT_SIGNER_URL);
            assert!(matches!(
                storage::read_value(storage::KEY_CLEAR_SIGNER_URL),
                Ok(None)
            ));

            drive(
                &mut host,
                Event::SignerUrlSubmitted {
                    text: "http://localhost:8140/".to_owned(),
                },
            );
            assert!(
                storage::read_value(storage::KEY_CLEAR_SIGNER_URL)
                    .ok()
                    .flatten()
                    .is_some()
            );
            let view = drive(&mut host, Event::SignerUrlReset);
            assert!(view.signer_url_is_default);
            assert_eq!(view.signer_url_error, None);
            assert!(matches!(
                storage::read_value(storage::KEY_CLEAR_SIGNER_URL),
                Ok(None)
            ));
        });
    }

    /// A method this build does not offer reads as "never chose" and stays
    /// on disk for the build that wrote it.
    #[test]
    fn an_unknown_method_reads_as_unset() {
        with_temp_state("sign-pref-unknown", || {
            let _ = storage::write_value(
                storage::KEY_SIGN_METHOD,
                Value::String("smoke_signals".to_owned()),
            );
            let mut host = CoreHost::<SignPref>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.method, "auto");
            assert!(!view.method_committed);
            assert_eq!(
                storage::read_value(storage::KEY_SIGN_METHOD).ok().flatten(),
                Some(Value::String("smoke_signals".to_owned()))
            );
        });
    }
}
