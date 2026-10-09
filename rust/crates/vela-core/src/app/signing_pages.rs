//! Machine — the signing pages this device keeps (spec 102, Settings →
//! Signing pages). It replaces spec 071's single "Trusted Signer page"
//! preference (`sign_pref`).
//!
//! ```text
//! refresh ─► LoadingStored ─► commit {pages}
//!                         └─► (a 071 page stored) import it once ─► persist
//! page_added ─┬─ a usable page not yet saved ─► persist + commit
//!             └─ invalid / insecure / already saved ─► refused, nothing stored
//! page_renamed / page_removed ─► persist + commit
//! version_trusted ─┬─ a self-hosted page, a sha256 ─► persist + commit
//!                  │   (the page saved first when it was not)
//!                  └─ the official page, not a hash ─► refused, nothing stored
//! ```
//!
//! **A list, not a field.** The 071 Settings field held one free-text address
//! that every signature opened — a door a social-engineering message walks
//! straight through ("just paste this address"). Now a person keeps a list of
//! pages they trust; the official page is always first and cannot be removed;
//! and which page an ACCOUNT signs on is that account's own choice
//! ([`crate::signing_venue::venue_choices`]), refused when the page cannot
//! reach its keys (R1). Removing a page from the list changes no account.
//!
//! The shell owns the keys (`vela.signingPages`, and the 071
//! `vela.trustedSignerUrl` it imports once and then removes, both under the
//! `vela.` prefix that survives sign-out) and the words; the core decides what
//! may be stored. Integrity is not this machine's: a row's integrity line comes
//! from the check (`trusted_signer::launch`), which the shell runs.
//!
//! **Trusting a version (spec 076 FR-009, spec 102 core round).** A
//! self-hosted page serving a build Vela did not publish can only end in
//! "Version 3f9a1c22 is new to Vela. Trust it on this device?"
//! ([`crate::trusted_signer::launch::IntegrityState::AskToTrust`]). The
//! answer is stored HERE, on that page ([`SigningPage::trusted`]) — so a yes
//! vouches for that deployment and nothing else — and the shell passes the
//! page's list as `trusted` the next time it checks it, which then reads
//! "trusted on this device" and opens. Never for the official page: nobody can
//! vouch for bytes `sign.getvela.app` was not supposed to serve, and an "allow
//! it anyway" there is a social-engineering door.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::signing_venue::{domain_of_page, SigningPage};
use crate::trusted_signer::{self, SignerUrlError};

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SigningPagesOperation"))]
pub enum SigningPagesOperation {
    /// Read `vela.signingPages` and the 071 `vela.trustedSignerUrl`, raw.
    ReadStored,
    /// Persist the saved pages to `vela.signingPages` — the official page is
    /// never among them. With `remove_legacy_url`, also remove
    /// `vela.trustedSignerUrl`: its page has just been imported, and leaving
    /// it would import it again after the person removed it.
    WritePages {
        pages: Vec<SigningPage>,
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        remove_legacy_url: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "SigningPagesShellResult")
)]
pub enum SigningPagesShellResult {
    /// What is stored, raw: `pages_json` is `vela.signingPages` as text,
    /// `legacy_url` `vela.trustedSignerUrl`. `None` for a key that is absent.
    Stored {
        #[serde(default)]
        pages_json: Option<String>,
        #[serde(default)]
        legacy_url: Option<String>,
    },
    Written,
}

impl Operation for SigningPagesOperation {
    type Output = SigningPagesShellResult;
}

#[effect]
pub enum SigningPagesEffect {
    Render(RenderOperation),
    Shell(SigningPagesOperation),
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SigningPagesEvent"))]
pub enum Event {
    /// Mount / focus — re-read. Coalesced while a read is in flight.
    Refresh,
    /// "Add a page": the address as typed, and an optional label.
    PageAdded {
        url: String,
        #[serde(default)]
        name: String,
    },
    /// A saved page's new label (empty clears it).
    PageRenamed { url: String, name: String },
    /// Forget a saved page. The official page cannot be removed.
    PageRemoved { url: String },
    /// "Trust this version": the person trusts `version` (the full sha256 the
    /// check served — `SignerPageAdmission::version_to_trust`) for the page at
    /// `url`, on this device. A page that is not saved yet is saved with it.
    /// Refused for the official page and for anything that is not a sha256.
    VersionTrusted { url: String, version: String },
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: SigningPagesShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model / view
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    /// Never read: edits are refused until the list is known, or the first
    /// write would replace pages it never saw.
    #[default]
    Unread,
    LoadingStored,
    Idle,
}

/// Why an address was not added.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AddError {
    Url(SignerUrlError),
    Duplicate,
}

#[derive(Default)]
pub struct Model {
    /// Saved pages, normalised, official excluded, each once.
    pages: Vec<SigningPage>,
    add_error: Option<AddError>,
    phase: Phase,
    attempt: u64,
}

/// One row of Settings → Signing pages.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SigningPageRow {
    pub url: String,
    /// The person's label; empty ⇒ the shell names the row by its host (or,
    /// for the official page, "Official").
    pub name: String,
    /// The domain whose keys this page can use (R1) — drawn on the row, so a
    /// person sees before choosing it which accounts it can sign for.
    pub domain: String,
    /// The official page: always first, never removed or renamed.
    pub official: bool,
    /// Versions of this page the person trusted on this device — what the
    /// shell passes as `trusted` when it checks this page. Always empty for
    /// the official page.
    #[serde(default)]
    pub trusted: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SigningPagesView {
    /// The official page, then the saved ones in the order they were added.
    pub pages: Vec<SigningPageRow>,
    /// The saved pages as stored — the input
    /// [`crate::signing_venue::venue_choices`] takes.
    pub saved: Vec<SigningPage>,
    /// `"invalid"` | `"insecure"` | `"duplicate"` — the last address was not
    /// added and nothing was stored. `None` otherwise.
    pub add_error: Option<String>,
    /// The list has been read; until then it holds only the official page and
    /// edits are not offered.
    pub loaded: bool,
}

#[derive(Default)]
pub struct SigningPages;

impl App for SigningPages {
    type Event = Event;
    type Model = Model;
    type ViewModel = SigningPagesView;
    type Effect = SigningPagesEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<SigningPagesEffect, Event> {
        match event {
            Event::Refresh => {
                if model.phase == Phase::LoadingStored {
                    return Command::done();
                }
                model.attempt += 1;
                model.phase = Phase::LoadingStored;
                shell(model, SigningPagesOperation::ReadStored)
            }
            Event::PageAdded { url, name } => {
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                let url = match trusted_signer::signer_url(&url) {
                    Ok(url) => url,
                    Err(error) => {
                        model.add_error = Some(AddError::Url(error));
                        return render();
                    }
                };
                if is_official(&url) || model.pages.iter().any(|page| page.url == url) {
                    model.add_error = Some(AddError::Duplicate);
                    return render();
                }
                model.add_error = None;
                model.pages.push(SigningPage::new(url, name.trim()));
                write(model, false)
            }
            Event::PageRenamed { url, name } => {
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                let url = normal(&url);
                let Some(page) = model.pages.iter_mut().find(|page| page.url == url) else {
                    return Command::done();
                };
                let name = name.trim().to_owned();
                if page.name == name {
                    return Command::done();
                }
                page.name = name;
                write(model, false)
            }
            Event::PageRemoved { url } => {
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                let url = normal(&url);
                let before = model.pages.len();
                model.pages.retain(|page| page.url != url);
                if model.pages.len() == before {
                    return Command::done();
                }
                model.add_error = None;
                write(model, false)
            }
            Event::VersionTrusted { url, version } => {
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                let Ok(url) = trusted_signer::signer_url(&url) else {
                    return Command::done();
                };
                let Some(version) = trusted_signer::integrity::normalize_hash(&version) else {
                    return Command::done();
                };
                if is_official(&url) || trusted_signer::integrity::is_official(&url) {
                    return Command::done();
                }
                let index = match model.pages.iter().position(|page| page.url == url) {
                    Some(index) => index,
                    None => {
                        model.pages.push(SigningPage::new(url, ""));
                        model.pages.len() - 1
                    }
                };
                let page = &mut model.pages[index];
                if page.trusted.contains(&version) {
                    return Command::done();
                }
                page.trusted.push(version);
                write(model, false)
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    return Command::done();
                }
                match (model.phase, result) {
                    (
                        Phase::LoadingStored,
                        SigningPagesShellResult::Stored {
                            pages_json,
                            legacy_url,
                        },
                    ) => {
                        model.phase = Phase::Idle;
                        model.pages = read_pages(pages_json.as_deref());
                        // 071's one page, imported once: added when it is a
                        // usable page not already here, and the old key
                        // removed either way, so a page the person later
                        // removes does not come back from it.
                        let Some(legacy) = legacy_url else {
                            return render();
                        };
                        if let Ok(url) = trusted_signer::signer_url(&legacy) {
                            if !is_official(&url) && !model.pages.iter().any(|p| p.url == url) {
                                model.pages.push(SigningPage::new(url, ""));
                            }
                        }
                        write(model, true)
                    }
                    _ => Command::done(),
                }
            }
        }
    }

    fn view(&self, model: &Model) -> SigningPagesView {
        let official = trusted_signer::DEFAULT_SIGNER_URL;
        let mut pages = vec![SigningPageRow {
            url: official.to_owned(),
            name: String::new(),
            domain: domain_of_page(official),
            official: true,
            trusted: Vec::new(),
        }];
        pages.extend(model.pages.iter().map(|page| SigningPageRow {
            url: page.url.clone(),
            name: page.name.clone(),
            domain: domain_of_page(&page.url),
            official: false,
            trusted: page.trusted.clone(),
        }));
        SigningPagesView {
            pages,
            saved: model.pages.clone(),
            add_error: model.add_error.map(|error| {
                match error {
                    AddError::Url(SignerUrlError::Invalid) => "invalid",
                    AddError::Url(SignerUrlError::Insecure) => "insecure",
                    AddError::Duplicate => "duplicate",
                }
                .to_owned()
            }),
            loaded: model.phase == Phase::Idle,
        }
    }
}

fn is_official(url: &str) -> bool {
    url == trusted_signer::DEFAULT_SIGNER_URL
}

fn normal(url: &str) -> String {
    trusted_signer::signer_url(url).unwrap_or_else(|_| url.trim().to_owned())
}

/// The stored list, re-validated: an entry this build would not store (or
/// the official page, or a repeat) is dropped rather than offered. Anything
/// that is not a list reads as none.
fn read_pages(raw: Option<&str>) -> Vec<SigningPage> {
    let stored: Vec<serde_json::Value> = raw
        .and_then(|text| serde_json::from_str(text).ok())
        .unwrap_or_default();
    let mut pages: Vec<SigningPage> = Vec::new();
    for entry in stored {
        let Ok(page) = serde_json::from_value::<SigningPage>(entry) else {
            continue;
        };
        let Ok(url) = trusted_signer::signer_url(&page.url) else {
            continue;
        };
        if is_official(&url) || pages.iter().any(|kept| kept.url == url) {
            continue;
        }
        let mut trusted: Vec<String> = Vec::new();
        for version in page
            .trusted
            .iter()
            .filter_map(|v| trusted_signer::integrity::normalize_hash(v))
        {
            if !trusted.contains(&version) {
                trusted.push(version);
            }
        }
        pages.push(SigningPage {
            url,
            name: page.name.trim().to_owned(),
            trusted,
        });
    }
    pages
}

fn write(model: &mut Model, remove_legacy_url: bool) -> Command<SigningPagesEffect, Event> {
    model.attempt += 1;
    shell(
        model,
        SigningPagesOperation::WritePages {
            pages: model.pages.clone(),
            remove_legacy_url,
        },
    )
}

fn shell(model: &Model, operation: SigningPagesOperation) -> Command<SigningPagesEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for SigningPagesEffect {
    type Op = SigningPagesOperation;
    fn into_shell(self) -> Option<crux_core::Request<SigningPagesOperation>> {
        match self {
            SigningPagesEffect::Render(_) => None,
            SigningPagesEffect::Shell(request) => Some(request),
        }
    }
}
