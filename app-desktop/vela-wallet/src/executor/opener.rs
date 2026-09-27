//! Opening a URL outside the app — the platform's own opener — and, for a
//! verification pass, not.
//!
//! `VELA_OPEN_URL_LOG=<file>` appends each URL to that file instead of
//! handing it to the system, so a pass can prove that a row opens exactly its
//! URL without launching a browser on the machine it runs on (the same
//! env-seam family as `VELA_IMPORT_FILE` and `VELA_BUG_REPORT_ENDPOINT`).

use std::io::Write as _;

/// Open `url` in the platform's handler for it (the browser; the X,
/// Telegram or Discord app when one claims the link).
pub fn open(url: &str, cx: &mut gpui::App) {
    if let Some(log) = std::env::var_os("VELA_OPEN_URL_LOG") {
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log)
        {
            let _ = writeln!(file, "{url}");
        }
        return;
    }
    cx.open_url(url);
}
