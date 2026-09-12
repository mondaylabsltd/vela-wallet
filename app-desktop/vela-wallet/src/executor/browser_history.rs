//! Where the browser column has been.
//!
//! Three operations, all on one key: read `vela.browserHistory`, write it back,
//! or delete it. The core decides what the list must BECOME — dedupe by origin,
//! recency order, the cap, and the rule that an update with no title must not
//! clobber a title already captured; this only stores what it is handed.
//!
//! ## The key, and why it is spelled the same
//!
//! `vela.browserHistory` is the cross-client name. A wallet directory copied
//! from the phone brings its history with it, and a desktop that invented its
//! own key would silently start from nothing beside a file that already had
//! the answer.
//!
//! ## A corrupt list is an empty one
//!
//! The port's `read()` catches and answers `[]`. Kept, and it is the right
//! shape here too: history is a convenience, and refusing to open the browser
//! because one stored row is malformed trades a whole feature for a row.

use gpui::App;
use serde_json::Value;

use vela_core::app::browser_history::{
    BhistEntry, BhistOperation, BhistShellResult, BrowserHistory, Event,
};

use crate::executor::storage;
use crate::resident::{Answer, Machine};

/// The cross-client key.
const HISTORY_KEY: &str = "vela.browserHistory";

impl Machine for BrowserHistory {
    const LABEL: &'static str = "browser_history";

    /// Hydrate once. The core's own note: after that the mirror is
    /// authoritative, and re-reading mid-session could only replay what this
    /// core already wrote.
    fn boot_event(_cx: &App) -> Event {
        Event::Start
    }

    fn perform(operation: &BhistOperation) -> Answer<BhistShellResult, Self::Event> {
        match operation {
            BhistOperation::ReadHistory => Answer::Now(BhistShellResult::Loaded {
                entries: read_entries(),
            }),
            BhistOperation::WriteHistory { entries } => {
                write_entries(entries);
                Answer::Now(BhistShellResult::Written)
            }
            BhistOperation::RemoveHistory => {
                // The key is REMOVED rather than written as `[]` — ported
                // verbatim, and the difference is visible to the next reader:
                // an absent key is a browser that has never been used, an
                // empty array is one that was cleared.
                let _ = storage::remove_value(HISTORY_KEY);
                Answer::Now(BhistShellResult::Written)
            }
        }
    }
}

fn read_entries() -> Vec<BhistEntry> {
    let Ok(Some(Value::Array(rows))) = storage::read_value(HISTORY_KEY) else {
        return Vec::new();
    };
    // A row that will not parse is SKIPPED, not fatal — the same rule the
    // contacts and network ledgers follow.
    rows.into_iter()
        .filter_map(|row| serde_json::from_value::<BhistEntry>(row).ok())
        .collect()
}

fn write_entries(entries: &[BhistEntry]) {
    let Ok(value) = serde_json::to_value(entries) else {
        return;
    };
    // Best effort, as the port's `write()` is: the in-memory mirror stays
    // authoritative, and there is nothing true to tell somebody about a
    // history row that did not reach the disk.
    let _ = storage::write_value(HISTORY_KEY, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(origin: &str, title: &str, at: f64) -> BhistEntry {
        BhistEntry {
            origin: origin.to_owned(),
            url: format!("{origin}/page"),
            host: origin.trim_start_matches("https://").to_owned(),
            title: title.to_owned(),
            favicon: String::new(),
            last_visited_ms: at,
        }
    }

    /// Round trip under the shared key, in the stored order.
    #[test]
    fn history_round_trips_under_the_shared_key() {
        storage::tests::with_temp_state("bhist-roundtrip", || {
            let entries = vec![
                entry("https://app.uniswap.org", "Uniswap", 1_757_000_000_000.0),
                entry("https://polymarket.com", "Polymarket", 1_756_000_000_000.0),
            ];
            write_entries(&entries);
            assert_eq!(read_entries(), entries);

            // Cleared means the key is gone, not an empty list left behind.
            let _ = storage::remove_value(HISTORY_KEY);
            assert!(read_entries().is_empty());
            assert!(
                storage::read_value(HISTORY_KEY).ok().flatten().is_none(),
                "clearing wrote an empty list instead of removing the key"
            );
        });
    }

    /// One unreadable row costs that row, never the whole history.
    #[test]
    fn a_malformed_row_is_skipped_rather_than_fatal() {
        storage::tests::with_temp_state("bhist-corrupt", || {
            let good = entry("https://app.uniswap.org", "Uniswap", 1.0);
            let rows = serde_json::json!([
                { "origin": "https://broken.example" },
                serde_json::to_value(&good).unwrap_or(Value::Null),
            ]);
            if storage::write_value(HISTORY_KEY, rows).is_err() {
                unreachable!("could not seed");
            }
            assert_eq!(read_entries(), vec![good]);
        });
    }

    /// A document that is not a list reads as no history at all.
    #[test]
    fn a_corrupt_document_reads_as_empty() {
        storage::tests::with_temp_state("bhist-nonarray", || {
            if storage::write_value(HISTORY_KEY, serde_json::json!("nonsense")).is_err() {
                unreachable!("could not seed");
            }
            assert!(read_entries().is_empty());
        });
    }
}
