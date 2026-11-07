//! r18-7 (S3) guard: no debug-formatted text reaches what the person reads.
//!
//! Settings → a multisig's members once said `Failed { rate_limited: false }`
//! under "Current members on Arc Testnet": the pool's error, formatted with
//! `{:?}` into a read's detail, drawn as prose. Every shell draws the core's
//! words, never a detail, and a detail itself is a stable sentence.
//!
//! This scans the app's own non-test source for `format!` / `write!` /
//! `writeln!` whose format string debug-formats a value (`{:?}`, `{x:?}`,
//! `{:#?}`). Each one left is on the allowlist below with the reason it never
//! reaches the screen — a log line, a dedupe or cache key, a developer
//! probe's stderr, a diagnostics-only field the core never words. A new one
//! fails here until it is fixed or justified; an entry nothing matches any
//! more fails too, so the list cannot go stale.

use std::path::{Path, PathBuf};

/// `(file under src/, a unique piece of the call, why it never reaches the
/// screen)`.
const ALLOWED: &[(&str, &str, &str)] = &[
    (
        "sandbox_probe.rs",
        "ERROR {error:?}",
        "developer probe (VELA_SANDBOX_PROBE): stderr only, compiled out of the store build",
    ),
    (
        "sandbox_probe.rs",
        "MISMATCH {other:?}",
        "developer probe: stderr only",
    ),
    (
        "sandbox_probe.rs",
        "\"{routes:?}\"",
        "developer probe: stderr only",
    ),
    (
        "multisig/dev_script.rs",
        "propose_send answered {end:?}",
        "developer script's log_line",
    ),
    (
        "multisig/dev_script.rs",
        "propose answered {end:?}",
        "developer script's log_line",
    ),
    (
        "multisig/live.rs",
        "format!(\"{failure:?}\")",
        "ShellFailure::Unknown.message is diagnostics only: the core words it as unknown \
         (\"Something went wrong, so nothing was sent\"), proven by \
         an_execution_that_failed_is_said_in_the_cores_words_never_debug_text",
    ),
    (
        "ctap/cable.rs",
        "tunnel CLOSE {frame:?}",
        "the caBLE transport's log",
    ),
    ("ctap/cable.rs", "PSM={psm:?}", "the caBLE transport's log"),
    (
        "wallet/page/multisig.rs",
        "page send: panel {:?}",
        "the developer script's log (dev_script::note)",
    ),
    (
        "wallet/page/multisig.rs",
        "space_notes panel {:?}",
        "the developer script's log (dev_script::note)",
    ),
    (
        "wallet/page/multisig.rs",
        "tabs {:?} selected {:?}",
        "the developer script's log (dev_script::note)",
    ),
    (
        "wallet/page.rs",
        "\"{:?}{:?}{:?}\"",
        "panel_subject(): the scroll's subject key, compared, never drawn",
    ),
    (
        "ctap/usb.rs",
        "malformed reply: {error:?}",
        "UsbError's Display: a passkey failure's diagnostics (the sheet's technical \
         details and the bug report); the sentence is chosen from the failure's kind",
    ),
    (
        "ctap/usb.rs",
        "the security key refused: {status:?}",
        "UsbError's Display: diagnostics only, as above (the CTAP status name)",
    ),
    (
        "ctap/usb.rs",
        "not a CTAP2 message: {error:?}",
        "UsbError::Encode's diagnostics, as above",
    ),
    (
        "wallet/browser_host.rs",
        "provider:{:?}",
        "the status line's dedupe key (the first of the tuple); its words are loc.t",
    ),
    (
        "wallet/browser_host.rs",
        "{}:{}:{:?}",
        "the status line's dedupe key; its words are loc.t(note.key) and the method",
    ),
    (
        "wallet/browser_host.rs",
        "format!(\"{:?}\", failure.class)",
        "the class for the browser's vlog line only",
    ),
    ("executor/proxy.rs", "{route:?}|{}", "the agent cache's key"),
    (
        "executor/signer_integrity.rs",
        "could not be checked ({why:?})",
        "describe(): one line for the log (vlog in trusted_signer::launchable)",
    ),
];

/// The app's own source (`src/`).
fn src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// What each character of a file is: code, or inside a literal or comment.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Code,
    Literal,
    Comment,
}

fn kinds(text: &[char]) -> Vec<Kind> {
    let mut kinds = vec![Kind::Code; text.len()];
    let mut i = 0;
    let at = |i: usize| text.get(i).copied().unwrap_or('\0');
    while i < text.len() {
        let c = text[i];
        if c == '/' && at(i + 1) == '/' {
            while i < text.len() && text[i] != '\n' {
                kinds[i] = Kind::Comment;
                i += 1;
            }
        } else if c == '/' && at(i + 1) == '*' {
            let mut depth = 0;
            while i < text.len() {
                if text[i] == '/' && at(i + 1) == '*' {
                    depth += 1;
                    kinds[i] = Kind::Comment;
                    kinds[i + 1] = Kind::Comment;
                    i += 2;
                } else if text[i] == '*' && at(i + 1) == '/' {
                    depth -= 1;
                    kinds[i] = Kind::Comment;
                    kinds[i + 1] = Kind::Comment;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    kinds[i] = Kind::Comment;
                    i += 1;
                }
            }
        } else if c == 'r'
            && (at(i + 1) == '"' || (at(i + 1) == '#' && matches!(at(i + 2), '"' | '#')))
            && !(i > 0 && (text[i - 1].is_alphanumeric() || text[i - 1] == '_'))
        {
            // A raw string: r"…", r#"…"#.
            let mut hashes = 0;
            let mut j = i + 1;
            while at(j) == '#' {
                hashes += 1;
                j += 1;
            }
            j += 1; // the opening quote
            loop {
                if j >= text.len() {
                    break;
                }
                if text[j] == '"' && (1..=hashes).all(|h| at(j + h) == '#') {
                    j += 1 + hashes;
                    break;
                }
                j += 1;
            }
            for k in kinds.iter_mut().take(j.min(text.len())).skip(i) {
                *k = Kind::Literal;
            }
            i = j;
        } else if c == '"' {
            kinds[i] = Kind::Literal;
            i += 1;
            while i < text.len() {
                kinds[i] = Kind::Literal;
                if text[i] == '\\' {
                    if i + 1 < text.len() {
                        kinds[i + 1] = Kind::Literal;
                    }
                    i += 2;
                    continue;
                }
                i += 1;
                if text[i - 1] == '"' {
                    break;
                }
            }
        } else if c == '\'' {
            // A char literal ('x', '\n', '\u{..}', '\''), not a lifetime.
            let end = if at(i + 1) == '\\' {
                (i + 2..text.len()).find(|&j| text[j] == '\'' && j > i + 2)
            } else if at(i + 2) == '\'' {
                Some(i + 2)
            } else {
                None
            };
            match end {
                Some(end) => {
                    for k in kinds.iter_mut().take(end + 1).skip(i) {
                        *k = Kind::Literal;
                    }
                    i = end + 1;
                }
                None => i += 1,
            }
        } else {
            i += 1;
        }
    }
    kinds
}

/// The index past the bracket matching the one at `open` (code only).
fn matching(text: &[char], kinds: &[Kind], open: usize) -> usize {
    let (o, c) = match text[open] {
        '(' => ('(', ')'),
        '[' => ('[', ']'),
        _ => ('{', '}'),
    };
    let mut depth = 0;
    for i in open..text.len() {
        if kinds[i] != Kind::Code {
            continue;
        }
        if text[i] == o {
            depth += 1;
        } else if text[i] == c {
            depth -= 1;
            if depth == 0 {
                return i + 1;
            }
        }
    }
    text.len()
}

fn find_code(text: &[char], kinds: &[Kind], from: usize, needle: &str) -> Option<usize> {
    let needle: Vec<char> = needle.chars().collect();
    (from..text.len().saturating_sub(needle.len() - 1)).find(|&i| {
        kinds[i] == Kind::Code
            && text[i..i + needle.len()] == needle[..]
            && !(needle[0].is_alphanumeric()
                && i > 0
                && (text[i - 1].is_alphanumeric() || text[i - 1] == '_'))
    })
}

/// The spans of items compiled only under test (`#[cfg(test)]`,
/// `#[cfg(all(test, …))]`), and the file modules they declare.
fn test_items(text: &[char], kinds: &[Kind]) -> (Vec<(usize, usize)>, Vec<String>) {
    let mut spans = Vec::new();
    let mut modules = Vec::new();
    let mut from = 0;
    while let Some(at) = find_code(text, kinds, from, "#[cfg(") {
        let close = matching(text, kinds, at + 1);
        let predicate: String = text[at + 6..close.saturating_sub(2).max(at + 6)]
            .iter()
            .collect();
        let predicate = predicate.trim();
        from = close;
        if !(predicate == "test" || predicate.starts_with("all(test")) {
            continue;
        }
        // The item: to its first `;` or past its braces, whichever is first.
        let mut end = text.len();
        for i in close..text.len() {
            if kinds[i] != Kind::Code {
                continue;
            }
            if text[i] == ';' {
                end = i + 1;
                break;
            }
            if text[i] == '{' {
                end = matching(text, kinds, i);
                break;
            }
        }
        let item: String = text[close..end].iter().collect();
        let words: Vec<&str> = item
            .split(|c: char| !(c.is_alphanumeric() || c == '_' || c == ';'))
            .filter(|w| !w.is_empty())
            .collect();
        if let Some(at) = words.iter().position(|w| *w == "mod")
            && let Some(name) = words.get(at + 1)
            && let Some(name) = name.strip_suffix(';')
        {
            modules.push(name.to_owned());
        }
        spans.push((at, end));
        from = end;
    }
    (spans, modules)
}

/// Every debug-formatting call in non-test code: `(file, line, call)`.
fn findings() -> Vec<(String, usize, String)> {
    let root = src();
    let mut files = Vec::new();
    rust_files(&root, &mut files);
    files.sort();
    let texts: Vec<(PathBuf, Vec<char>)> = files
        .iter()
        .filter_map(|f| {
            Some((
                f.clone(),
                std::fs::read_to_string(f).ok()?.chars().collect(),
            ))
        })
        .collect();
    // The files only a test compiles (`#[cfg(test)] mod name;`), and their
    // directories.
    let mut test_only: Vec<PathBuf> = Vec::new();
    for (file, text) in &texts {
        let kinds = kinds(text);
        let (_, modules) = test_items(text, &kinds);
        let stem = file
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let dir = match stem {
            "main" | "lib" | "mod" => file.parent().map(Path::to_path_buf),
            _ => file.parent().map(|p| p.join(stem)),
        }
        .unwrap_or_default();
        for name in modules {
            test_only.push(dir.join(format!("{name}.rs")));
            test_only.push(dir.join(&name));
        }
    }
    let this = root.join("debug_text_guard.rs");
    let mut out = Vec::new();
    for (file, text) in &texts {
        if *file == this || test_only.iter().any(|t| file.starts_with(t)) {
            continue;
        }
        let kinds = kinds(text);
        let (spans, _) = test_items(text, &kinds);
        let in_test = |i: usize| spans.iter().any(|(a, b)| (*a..*b).contains(&i));
        for name in ["format!(", "write!(", "writeln!("] {
            let mut from = 0;
            while let Some(at) = find_code(text, &kinds, from, name) {
                let open = at + name.len() - 1;
                let end = matching(text, &kinds, open);
                from = end.max(at + 1);
                if in_test(at) {
                    continue;
                }
                let debug = (open..end).any(|i| {
                    kinds[i] == Kind::Literal
                        && text[i] == '?'
                        && text.get(i + 1) == Some(&'}')
                        && matches!(text.get(i - 1), Some(':' | '#'))
                });
                if debug {
                    let line = text[..at].iter().filter(|c| **c == '\n').count() + 1;
                    let call: String = text[at..end].iter().collect();
                    let rel = file
                        .strip_prefix(&root)
                        .unwrap_or(file)
                        .to_string_lossy()
                        .replace('\\', "/");
                    out.push((rel, line, call));
                }
            }
        }
    }
    out
}

#[test]
fn no_debug_formatted_text_reaches_the_person() {
    let found = findings();
    let mut unexplained = Vec::new();
    let mut used = vec![false; ALLOWED.len()];
    for (file, line, call) in &found {
        match ALLOWED
            .iter()
            .position(|(f, needle, _)| f == file && call.contains(needle))
        {
            Some(at) => used[at] = true,
            None => unexplained.push(format!("src/{file}:{line}: {call}")),
        }
    }
    assert!(
        unexplained.is_empty(),
        "debug text formatted outside a test — say it in stable words, or justify it in \
         ALLOWED:\n{}",
        unexplained.join("\n")
    );
    let stale: Vec<_> = ALLOWED
        .iter()
        .zip(&used)
        .filter(|(_, used)| !**used)
        .map(|((f, needle, _), _)| format!("{f}: {needle}"))
        .collect();
    assert!(
        stale.is_empty(),
        "allowlist entries nothing matches: {stale:?}"
    );
}

/// The scanner itself: it sees a debug placeholder in a call, not in a
/// comment, a test, or a plain `{}`.
#[test]
fn the_scanner_sees_what_it_should() {
    let text: Vec<char> = r##"
        // format!("{x:?}") in a comment
        fn a() -> String { format!("{}", 1) }
        fn b() -> String { format!("{x:?}", x = 1) }
        fn c() -> String { format!(r#"{:#?}"#, 1) }
        #[cfg(test)]
        mod tests { fn d() -> String { format!("{:?}", '}') } }
    "##
    .chars()
    .collect();
    let kinds = kinds(&text);
    let (spans, _) = test_items(&text, &kinds);
    assert_eq!(spans.len(), 1);
    let mut hits = 0;
    let mut from = 0;
    while let Some(at) = find_code(&text, &kinds, from, "format!(") {
        let open = at + "format!(".len() - 1;
        let end = matching(&text, &kinds, open);
        from = end;
        if spans.iter().any(|(a, b)| (*a..*b).contains(&at)) {
            continue;
        }
        if (open..end)
            .any(|i| kinds[i] == Kind::Literal && text[i] == '?' && text.get(i + 1) == Some(&'}'))
        {
            hits += 1;
        }
    }
    assert_eq!(hits, 2, "b and c, not a, the comment or the test");
}
