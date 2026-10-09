//! Settings → Signing pages (spec 102) — read raw, written as the core
//! decided.
//!
//! The list replaces spec 071's one free-text "Trusted Signer page": a person
//! keeps the pages they trust, the official one always first and never
//! stored, and which page an ACCOUNT signs on is that account's own choice
//! (Settings → Account → "Where you review and sign"). Whether an address is a
//! page a browser will sign on, whether it is already saved, and the one-time
//! import of the 071 page are all the core's (`app::signing_pages`); this file
//! is the store.
//!
//! `vela.signingPages` survives sign-out, as every `vela.` preference does: a
//! person's own pages are theirs, not an account's.

use gpui::App;
use serde_json::Value;

use vela_core::app::signing_pages::{
    Event, SigningPages, SigningPagesOperation, SigningPagesShellResult,
};
use vela_core::signing_venue::SigningPage;

use crate::executor::storage;
use crate::resident::{Answer, Machine};

impl Machine for SigningPages {
    const LABEL: &'static str = "signing_pages";

    fn boot_event(_cx: &App) -> Event {
        Event::Refresh
    }

    fn perform(operation: &SigningPagesOperation) -> Answer<SigningPagesShellResult, Self::Event> {
        match operation {
            // Raw, both of them: an unreadable list is the core's to drop,
            // and a read that failed reads as "nothing stored".
            SigningPagesOperation::ReadStored => Answer::Now(SigningPagesShellResult::Stored {
                pages_json: storage::read_value(storage::KEY_SIGNING_PAGES)
                    .ok()
                    .flatten()
                    .map(|value| value.to_string()),
                legacy_url: storage::read_value(storage::KEY_TRUSTED_SIGNER_URL)
                    .ok()
                    .flatten()
                    .as_ref()
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            }),
            // Best effort: the list stays on screen either way, and the next
            // launch reads what made it to disk.
            SigningPagesOperation::WritePages {
                pages,
                remove_legacy_url,
            } => {
                let _ = storage::write_value(
                    storage::KEY_SIGNING_PAGES,
                    serde_json::to_value(pages).unwrap_or_else(|_| Value::Array(Vec::new())),
                );
                if *remove_legacy_url {
                    let _ = storage::remove_value(storage::KEY_TRUSTED_SIGNER_URL);
                }
                Answer::Now(SigningPagesShellResult::Written)
            }
        }
    }
}

/// The saved pages as stored, for the background integrity check at launch
/// (which runs before any screen boots the machine). Entries this build would
/// not store are skipped — the machine drops them the same way.
#[must_use]
pub fn saved() -> Vec<SigningPage> {
    storage::read_value(storage::KEY_SIGNING_PAGES)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_value::<Vec<Value>>(value).ok())
        .unwrap_or_default()
        .into_iter()
        .filter_map(|entry| serde_json::from_value::<SigningPage>(entry).ok())
        .filter(|page| vela_core::trusted_signer::signer_url(&page.url).is_ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core_host::CoreHost;
    use crate::executor::storage::tests::with_temp_state;
    use vela_core::app::signing_pages::SigningPagesView;
    use vela_core::trusted_signer::DEFAULT_SIGNER_URL;

    fn drive(host: &mut CoreHost<SigningPages>, event: Event) -> SigningPagesView {
        let mut pending = host.dispatch(event);
        while let Some(next) = pending.pop() {
            let Answer::Now(result) = SigningPages::perform(&next.operation) else {
                unreachable!("every signing_pages operation is local");
            };
            pending.extend(host.resolve(next.id, result));
        }
        host.view()
    }

    /// A fresh install lists the official page alone, first, and stores
    /// nothing.
    #[test]
    fn a_fresh_install_lists_the_official_page() {
        with_temp_state("signing-pages-fresh", || {
            let mut host = CoreHost::<SigningPages>::new();
            let view = drive(&mut host, Event::Refresh);
            assert!(view.loaded);
            assert_eq!(view.pages.len(), 1);
            assert!(view.pages[0].official);
            assert_eq!(view.pages[0].url, DEFAULT_SIGNER_URL);
            assert_eq!(view.pages[0].domain, "getvela.app");
            assert!(view.saved.is_empty());
        });
    }

    /// A page is added under the key every client reads, named, renamed and
    /// removed — and the next launch reads it back.
    #[test]
    fn a_page_is_added_renamed_removed_and_persists() {
        with_temp_state("signing-pages-persist", || {
            let mut host = CoreHost::<SigningPages>::new();
            drive(&mut host, Event::Refresh);
            let view = drive(
                &mut host,
                Event::PageAdded {
                    url: "http://localhost:8140/clearsigning/".to_owned(),
                    name: " Home ".to_owned(),
                },
            );
            assert_eq!(view.add_error, None);
            assert_eq!(view.pages.len(), 2);
            assert_eq!(view.pages[1].url, "http://localhost:8140/clearsigning/");
            assert_eq!(view.pages[1].name, "Home");
            assert_eq!(view.pages[1].domain, "localhost");
            assert_eq!(saved(), view.saved);

            let mut next = CoreHost::<SigningPages>::new();
            let view = drive(&mut next, Event::Refresh);
            assert_eq!(view.saved.len(), 1, "it survived the launch");

            let view = drive(
                &mut next,
                Event::PageRenamed {
                    url: "http://localhost:8140/clearsigning/".to_owned(),
                    name: "Desk".to_owned(),
                },
            );
            assert_eq!(view.pages[1].name, "Desk");
            let view = drive(
                &mut next,
                Event::PageRemoved {
                    url: "http://localhost:8140/clearsigning/".to_owned(),
                },
            );
            assert_eq!(view.pages.len(), 1);
            assert!(saved().is_empty());
        });
    }

    /// An address a browser will not sign on, or one already saved, is
    /// refused with its reason and nothing is stored.
    #[test]
    fn a_page_is_validated_before_it_is_kept() {
        with_temp_state("signing-pages-refused", || {
            let mut host = CoreHost::<SigningPages>::new();
            drive(&mut host, Event::Refresh);
            let view = drive(
                &mut host,
                Event::PageAdded {
                    url: "http://192.168.1.4/".to_owned(),
                    name: String::new(),
                },
            );
            assert_eq!(view.add_error.as_deref(), Some("insecure"));
            let view = drive(
                &mut host,
                Event::PageAdded {
                    url: "https://sign.getvela.app".to_owned(),
                    name: String::new(),
                },
            );
            assert_eq!(view.add_error.as_deref(), Some("duplicate"));
            assert!(matches!(
                storage::read_value(storage::KEY_SIGNING_PAGES),
                Ok(None)
            ));
        });
    }

    /// 071's one page is imported once, and its old key removed, so a page
    /// the person later removes does not come back from it.
    #[test]
    fn the_071_page_is_imported_once() {
        with_temp_state("signing-pages-import", || {
            let _ = storage::write_value(
                storage::KEY_TRUSTED_SIGNER_URL,
                Value::String("https://sign.example.test/".to_owned()),
            );
            let mut host = CoreHost::<SigningPages>::new();
            let view = drive(&mut host, Event::Refresh);
            assert_eq!(view.saved.len(), 1);
            assert_eq!(view.saved[0].url, "https://sign.example.test/");
            assert!(matches!(
                storage::read_value(storage::KEY_TRUSTED_SIGNER_URL),
                Ok(None)
            ));
        });
    }
}
