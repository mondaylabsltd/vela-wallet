//! Opening a URL outside the app — the platform's own opener — and, for a
//! verification pass, not.
//!
//! `VELA_OPEN_URL_LOG=<file>` appends each URL to that file instead of
//! handing it to the system, so a pass can prove that a row opens exactly its
//! URL without launching a browser on the machine it runs on (the same
//! env-seam family as `VELA_IMPORT_FILE` and `VELA_BUG_REPORT_ENDPOINT`).

use std::fmt::Write as _;
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

/// Open an address a WEB PAGE chose, as [`command_value`] hands it on, or not
/// at all (083). This covers a page's tapped `mailto:`/`tel:` and the dApp
/// browser's "open in the system browser". The wallet's own fixed links
/// (community, feedback) go through [`open`] and keep their length: a
/// prefilled issue form can be longer than the cap.
pub fn open_from_page(url: &str, cx: &mut gpui::App) {
    if let Some(value) = command_value(url) {
        open(&value, cx);
    }
}

/// The most a page's address may be when it is handed to another program.
/// Chromium's own cap on Windows (`platform_util_win.cc`), for the same reason:
/// what follows is a command line.
const COMMAND_VALUE_MAX: usize = 2048;

/// A page's address as it may reach another program's command line (083).
///
/// On Windows the opener is `ShellExecuteW("open", url)`, and the handler of a
/// `tel:` or `mailto:` is registered as a command with the URL in `"%1"`.
/// WebView2's own launch escapes the value first (Chromium's
/// `EscapeExternalHandlerValue`); once Vela cancels that launch and opens the
/// address itself, nothing did — `tel:1" --flag x` reached the handler's
/// command line with its quote, the classic protocol-handler argument
/// injection. So the same escape, here: every byte percent-encoded except
/// ASCII letters and digits, `-_.!~*'()`, `;/?:@&=+$,#[]`, and a `%` that
/// already starts a valid `%XX`. `None` when the result is over 2048 bytes.
#[must_use]
pub fn command_value(url: &str) -> Option<String> {
    const KEPT: &[u8] = b"-_.!~*'();/?:@&=+$,#[]";
    let bytes = url.as_bytes();
    let mut value = String::with_capacity(bytes.len());
    for (i, &byte) in bytes.iter().enumerate() {
        let escaped_already = byte == b'%'
            && bytes.get(i + 1).is_some_and(u8::is_ascii_hexdigit)
            && bytes.get(i + 2).is_some_and(u8::is_ascii_hexdigit);
        if byte.is_ascii_alphanumeric() || KEPT.contains(&byte) || escaped_already {
            value.push(char::from(byte));
        } else {
            let _ = write!(value, "%{byte:02X}");
        }
    }
    (value.len() <= COMMAND_VALUE_MAX).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 083: a page's quote or space never reaches a handler's command line
    /// as itself — the injection the engine's own launch escaped.
    #[test]
    fn a_pages_address_is_escaped_before_another_program_sees_it() {
        assert_eq!(
            command_value("tel:1\" --x").as_deref(),
            Some("tel:1%22%20--x")
        );
        assert_eq!(
            command_value("mailto:a@b.example?subject=Hi%20there&body=x").as_deref(),
            Some("mailto:a@b.example?subject=Hi%20there&body=x"),
            "an address already escaped is left as it is"
        );
        assert_eq!(
            command_value("mailto:a@b.example?subject=100%&x=%zz").as_deref(),
            Some("mailto:a@b.example?subject=100%25&x=%25zz"),
            "a % that starts no escape is escaped itself"
        );
        assert_eq!(
            command_value("tel:+1 (555) 01<>`{|}^\\\n").as_deref(),
            Some("tel:+1%20(555)%2001%3C%3E%60%7B%7C%7D%5E%5C%0A")
        );
        assert_eq!(
            command_value("mailto:é@b.example").as_deref(),
            Some("mailto:%C3%A9@b.example"),
            "every byte past ASCII"
        );
        assert_eq!(
            command_value("https://a.example/p?q=1#top[0]").as_deref(),
            Some("https://a.example/p?q=1#top[0]"),
            "a web address as the engine writes it is unchanged"
        );
        // Escaping twice changes nothing.
        let once = command_value("tel:1\" --x").unwrap_or_default();
        assert_eq!(command_value(&once).as_deref(), Some(once.as_str()));
    }

    /// Over Chromium's cap, nothing is handed over at all.
    #[test]
    fn an_address_too_long_for_a_command_line_is_not_opened() {
        let prefix = "mailto:a@b.example?body=";
        let at_cap = format!("{prefix}{}", "x".repeat(COMMAND_VALUE_MAX - prefix.len()));
        assert_eq!(at_cap.len(), 2048);
        assert_eq!(command_value(&at_cap).as_deref(), Some(at_cap.as_str()));
        assert_eq!(command_value(&format!("{at_cap}x")), None);
        // The cap is on what would be handed over, escapes included.
        let spaces = format!("{prefix}{}", " ".repeat(700));
        assert!(spaces.len() < 2048);
        assert_eq!(command_value(&spaces), None);
    }
}
