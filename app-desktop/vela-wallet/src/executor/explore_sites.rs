//! The browser's own memory on disk: `vela.explore`.
//!
//! Two operations, one key, one document. The machine decides what the
//! document must become — dedupe by origin, the caps, what a group is, which
//! tab is selected; this reads it and writes it back.
//!
//! An unreadable or absent document answers `None`, which the core reads as an
//! empty start page: a browser nobody has arranged yet is not an error, and a
//! start page that refused to draw because one stored field is odd would trade
//! the whole surface for a field.

use gpui::App;
#[cfg(test)]
use serde_json::Value;

use vela_core::app::explore_sites::{
    Event, ExploreDoc, ExploreOperation, ExploreShellResult, ExploreSites,
};

use crate::executor::storage;
use crate::resident::{Answer, Machine};

/// The cross-client key. A wallet directory copied from another client brings
/// its favourites with it, and a desktop that invented its own name would sit
/// beside a document that already had the answer.
const EXPLORE_KEY: &str = "vela.explore";

impl Machine for ExploreSites {
    const LABEL: &'static str = "explore_sites";

    fn boot_event(_cx: &App) -> Event {
        Event::Start
    }

    fn perform(operation: &ExploreOperation) -> Answer<ExploreShellResult, Self::Event> {
        match operation {
            ExploreOperation::ReadExplore => {
                Answer::Now(ExploreShellResult::Loaded { doc: read_doc() })
            }
            ExploreOperation::WriteExplore { doc } => {
                write_doc(doc);
                Answer::Now(ExploreShellResult::Written)
            }
        }
    }
}

fn read_doc() -> Option<ExploreDoc> {
    let value = storage::read_value(EXPLORE_KEY).ok()??;
    // A document that will not parse is NO document rather than a failure —
    // the core's own contract for this read.
    serde_json::from_value::<ExploreDoc>(value).ok()
}

fn write_doc(doc: &ExploreDoc) {
    let Ok(value) = serde_json::to_value(doc) else {
        return;
    };
    // Best effort. The mirror in the machine stays authoritative, so a
    // swallowed storage error costs the next launch's memory and nothing in
    // this session — the same bargain the history's write makes.
    let _ = storage::write_value(EXPLORE_KEY, value);
}

/// Is there anything stored at all? Used by nothing but the tests today; kept
/// because a reader of this file will ask.
#[cfg(test)]
fn stored() -> Option<Value> {
    storage::read_value(EXPLORE_KEY).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::explore_sites::{ExploreGroup, ExploreSite};

    fn site(origin: &str) -> ExploreSite {
        ExploreSite {
            origin: origin.to_owned(),
            url: format!("{origin}/app"),
            host: origin.trim_start_matches("https://").to_owned(),
            name: "Curve".to_owned(),
            renamed: true,
            added_ms: 1_757_000_000_000.0,
        }
    }

    /// The document round-trips whole, under the shared key.
    #[test]
    fn the_document_round_trips_under_the_shared_key() {
        storage::tests::with_temp_state("explore-roundtrip", || {
            let doc = ExploreDoc {
                favorites: vec![site("https://curve.fi")],
                groups: vec![ExploreGroup {
                    id: "g-1".to_owned(),
                    name: "Trading".to_owned(),
                    members: vec!["https://curve.fi".to_owned()],
                    hidden: false,
                    created_ms: 1_757_000_000_000.0,
                }],
                ..ExploreDoc::default()
            };
            write_doc(&doc);

            let raw = stored().unwrap_or_else(|| unreachable!("nothing was written"));
            assert!(
                raw.get("favorites").is_some() && raw.get("groups").is_some(),
                "the stored shape is the document's own field names"
            );
            assert_eq!(read_doc(), Some(doc));
        });
    }

    /// A document from a future version — or a hand-edited one — reads as an
    /// empty start page rather than as a failure somebody has to dismiss.
    #[test]
    fn an_unreadable_document_is_an_empty_start_page() {
        storage::tests::with_temp_state("explore-corrupt", || {
            if storage::write_value(EXPLORE_KEY, serde_json::json!("nonsense")).is_err() {
                unreachable!("could not seed");
            }
            assert_eq!(read_doc(), None);
        });
    }

    /// Issue #425: the document a build before the name rule wrote — a
    /// favourite named after an engine's error page, no `name_rule` — reads
    /// whole (the core, not this store, replaces the name with the host), and
    /// the rule's marker survives the round trip, or every launch would read
    /// the document as one from before the rule again.
    #[test]
    fn a_document_from_before_the_name_rule_reads_and_the_rule_survives_the_store() {
        storage::tests::with_temp_state("explore-name-rule", || {
            let before = serde_json::json!({
                "favorites": [{
                    "origin": "https://app.uniswap.org", "url": "https://app.uniswap.org/",
                    "host": "app.uniswap.org", "name": "网页无法打开", "renamed": false,
                    "added_ms": 1_759_051_383_000.0_f64
                }],
                "groups": [], "tabs": [], "selected_tab": null, "hidden_system": []
            });
            if storage::write_value(EXPLORE_KEY, before).is_err() {
                unreachable!("could not seed");
            }
            let mut doc =
                read_doc().unwrap_or_else(|| unreachable!("the old document was refused"));
            assert_eq!(doc.name_rule, 0);
            assert_eq!(doc.favorites[0].name, "网页无法打开");

            doc.name_rule = vela_core::app::explore_sites::NAME_RULE;
            doc.favorites[0].name = "app.uniswap.org".to_owned();
            write_doc(&doc);
            assert_eq!(read_doc(), Some(doc));
        });
    }

    /// Missing lists are empty lists: an older document that predates tabs
    /// still opens, because every field carries `serde(default)`.
    #[test]
    fn an_older_document_without_tabs_still_opens() {
        storage::tests::with_temp_state("explore-partial", || {
            if storage::write_value(EXPLORE_KEY, serde_json::json!({ "favorites": [] })).is_err() {
                unreachable!("could not seed");
            }
            let doc = read_doc().unwrap_or_else(|| unreachable!("a partial document was refused"));
            assert!(doc.tabs.is_empty() && doc.groups.is_empty());
        });
    }
}
