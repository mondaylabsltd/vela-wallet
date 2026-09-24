//! Language-code normalisation, the resolve hierarchy, and candidate-key order.
//!
//! Under the app's config (`load: 'currentOnly'`, `fallbackLng: 'en'`,
//! `supportedLngs`) the hierarchy is always `[active, "en"]`, or `["en"]` when the
//! active language *is* `en`. **`zh-TW` therefore never reads `zh`**
//! (`i18next.js:1010-1011`): a key present in `zh` but absent from `zh-TW` resolves
//! to the *English* value, skipping Simplified entirely. That is correct behaviour
//! under this configuration, not a bug to fix.
//!
//! Normalisation is asymmetric, and the corpus pins all 17 rows:
//!
//! | input | result | why |
//! |---|---|---|
//! | `zh-tw`, `ZH-TW`, `ZH-tw`, `zH-Tw` | `zh-TW` | canonicalised — the code contains `-` |
//! | `ZH`, `DE`, `De` | `en` | **not** canonicalised; bare codes skip the rewrite, so they miss `supportedLngs` and fall to `fallbackLng` |
//! | `zh_TW` | `zh` | `_` is not a subtag separator, so only the language part survives — Traditional silently becomes Simplified |
//! | `zh-Hant`, `zh-Hant-TW` | `zh` | script-only match is not in `supportedLngs`, so the language part wins |
//! | `es-AR` | `es-MX` | prefix match against `supportedLngs` |
//!
//! This table describes `change_language`. Per-call `t(key, {lng})` is a
//! **different** function with no recovery ladder — `zh_TW`, `zh-Hant`,
//! `zh-Hant-TW` and `es-AR` all fall through to `en` there. The corpus pins the two
//! paths separately (D4 path-sensitivity finding 1).

use super::plural::{plural_category, plural_category_legacy, PluralMode};

/// The locales the app ships, in `src/i18n/index.ts` order. Public so a
/// conformance harness can tell a supported tag from one that falls through to
/// the fallback.
pub const SUPPORTED: [&str; 15] = [
    "en", "zh", "zh-TW", "zh-HK", "ja", "ko", "vi", "id", "tr", "es-MX", "pt-BR", "fr", "de", "ru",
    "it",
];
pub(crate) const FALLBACK: &str = "en";

/// What each locale calls ITSELF, in its own script.
///
/// A language row that says 简体中文 to somebody reading German is not a small
/// wording slip — it tells them the app is in a language they can see it is
/// not. Endonyms are deliberately NOT translated: the one person who most
/// needs to read "日本語" is the one who has ended up in Japanese by accident
/// and cannot read anything else on the screen.
///
/// Unknown tags answer with the tag, which is at least true. Lives here rather
/// than in a shell because [`SUPPORTED`] is here, and four copies of a list of
/// language names is four lists that drift.
#[must_use]
pub fn endonym(tag: &str) -> &str {
    match tag {
        "en" => "English",
        "zh" => "简体中文",
        "zh-TW" => "繁體中文（台灣）",
        "zh-HK" => "繁體中文（香港）",
        "ja" => "日本語",
        "ko" => "한국어",
        "vi" => "Tiếng Việt",
        "id" => "Bahasa Indonesia",
        "tr" => "Türkçe",
        "es-MX" => "Español (México)",
        "pt-BR" => "Português (Brasil)",
        "fr" => "Français",
        "de" => "Deutsch",
        "ru" => "Русский",
        "it" => "Italiano",
        other => other,
    }
}

/// Text direction of a locale.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

impl Dir {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Dir::Ltr => "ltr",
            Dir::Rtl => "rtl",
        }
    }
}

/// The outcome of normalising a requested language code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageState {
    /// The normalised requested tag.
    pub language: String,
    /// `None` when the requested tag matched nothing in `supportedLngs`. In this
    /// configuration the fallback is itself supported, so it is always `Some`.
    pub resolved_language: Option<String>,
    /// The resolve hierarchy — `[active, "en"]`, or `["en"]`.
    pub languages: Vec<String>,
}

fn is_supported(code: &str) -> bool {
    SUPPORTED.contains(&code)
}

/// `getCleanedCode` (`i18next.js:191`): replaces the **first** `_` with `-`, and
/// only that one. `String.prototype.replace` with a string pattern is not global,
/// which is why `zh_TW_x` would keep its second underscore.
fn cleaned_code(code: &str) -> String {
    match code.find('_') {
        Some(i) => {
            let mut out = String::with_capacity(code.len());
            out.push_str(&code[..i]);
            out.push('-');
            out.push_str(&code[i + 1..]);
            out
        }
        None => code.to_owned(),
    }
}

/// `Intl.getCanonicalLocales(code)[0]` for the subtag shapes a language tag can
/// take: language lowercase, script title-case, region upper-case, everything else
/// lower-case.
fn canonicalize(code: &str) -> String {
    let mut out = String::with_capacity(code.len());
    for (i, part) in code.split('-').enumerate() {
        if i > 0 {
            out.push('-');
        }
        if i == 0 {
            out.push_str(&part.to_ascii_lowercase());
        } else if part.len() == 4 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            // script
            let mut cs = part.chars();
            if let Some(f) = cs.next() {
                out.push(f.to_ascii_uppercase());
                out.push_str(&cs.as_str().to_ascii_lowercase());
            }
        } else if part.len() == 2 && part.chars().all(|c| c.is_ascii_alphabetic()) {
            // region
            out.push_str(&part.to_ascii_uppercase());
        } else {
            out.push_str(&part.to_ascii_lowercase());
        }
    }
    out
}

/// `formatLanguageCode` (`i18next.js:932-950`).
///
/// **Only canonicalises codes containing `-`.** A bare `ZH` is returned unchanged
/// and therefore misses `supportedLngs`, which is why `ZH` resolves to `en` while
/// `ZH-tw` resolves to `zh-TW`. The asymmetry is upstream's, and it is pinned by the
/// corpus.
pub(crate) fn format_language_code(code: &str) -> String {
    if code.contains('-') {
        canonicalize(code)
    } else {
        code.to_owned()
    }
}

/// `getScriptPartFromCode` (`i18next.js:919-927`). `None` for a two-part tag, which
/// is why `zh-Hant` is reachable only through its language part.
fn script_part(code: &str) -> Option<String> {
    let cleaned = cleaned_code(code);
    if !cleaned.contains('-') {
        return None;
    }
    let mut parts: Vec<&str> = cleaned.split('-').collect();
    if parts.len() == 2 {
        return None;
    }
    parts.pop();
    if parts.last().is_some_and(|p| p.eq_ignore_ascii_case("x")) {
        return None;
    }
    Some(format_language_code(&parts.join("-")))
}

/// `getLanguagePartFromCode` (`i18next.js:911-917`).
fn language_part(code: &str) -> String {
    let cleaned = cleaned_code(code);
    if !cleaned.contains('-') {
        return cleaned;
    }
    format_language_code(cleaned.split('-').next().unwrap_or(&cleaned))
}

/// `getBestMatchFromCodes` (`i18next.js:957-982`) — the recovery ladder that
/// `change_language` runs and the per-call `lng` option does **not**.
fn best_match(code: &str) -> String {
    let formatted = format_language_code(code);
    if is_supported(&formatted) {
        return formatted;
    }
    if let Some(sc) = script_part(code) {
        if is_supported(&sc) {
            return sc;
        }
    }
    let lng_only = language_part(code);
    if is_supported(&lng_only) {
        return lng_only;
    }
    // The prefix search: `es-AR` -> `es-MX`, because `es-MX` starts with `es`.
    for supported in SUPPORTED {
        if supported == lng_only {
            return supported.to_owned();
        }
        if !supported.contains('-') && !lng_only.contains('-') {
            continue;
        }
        if supported.contains('-')
            && !lng_only.contains('-')
            && supported.split('-').next() == Some(lng_only.as_str())
        {
            return supported.to_owned();
        }
        if supported.starts_with(&lng_only) && lng_only.len() > 1 {
            return supported.to_owned();
        }
    }
    FALLBACK.to_owned()
}

/// `toResolveHierarchy` (`i18next.js:997-1019`) under `load: 'currentOnly'`.
///
/// `currentOnly` skips **both** the script-part and language-part `addCode` calls,
/// which is what collapses the hierarchy to at most two entries and is why `zh-TW`
/// never reads `zh`.
fn resolve_hierarchy(code: &str) -> Vec<String> {
    let mut codes = Vec::with_capacity(2);
    let formatted = format_language_code(code);
    if is_supported(&formatted) {
        codes.push(formatted);
    }
    if !codes.iter().any(|c| c == FALLBACK) {
        codes.push(FALLBACK.to_owned());
    }
    codes
}

/// Normalise a requested language code the way `changeLanguage` does, and derive
/// its resolve hierarchy.
#[must_use]
pub fn resolve_language(requested: &str) -> LanguageState {
    let language = best_match(requested);
    let languages = resolve_hierarchy(&language);
    LanguageState {
        resolved_language: Some(language.clone()),
        language,
        languages,
    }
}

/// The `&'static` entry of [`SUPPORTED`] matching `requested`, after the same
/// canonicalisation `formatLanguageCode` applies. Returns `None` for an
/// unsupported tag — the per-call path has no recovery ladder.
///
/// Borrowing the static entry rather than returning a `String` is what keeps the
/// per-call `lng` path allocation-free.
#[must_use]
pub(crate) fn supported_tag(requested: &str) -> Option<&'static str> {
    if !requested.contains('-') {
        return SUPPORTED.iter().copied().find(|s| *s == requested);
    }
    let canonical = canonicalize(requested);
    SUPPORTED.iter().copied().find(|s| *s == canonical)
}

/// The per-call `{lng}` path, which has **no** recovery ladder: an unsupported tag
/// falls straight through to the fallback rather than degrading to its language
/// part. `t('k', {lng: 'zh_TW'})` therefore renders English where
/// `change_language('zh_TW')` renders Simplified Chinese.
#[must_use]
pub fn resolve_lng_option(requested: &str) -> Vec<String> {
    let formatted = format_language_code(requested);
    let mut codes = Vec::with_capacity(2);
    if is_supported(&formatted) {
        codes.push(formatted);
    }
    if !codes.iter().any(|c| c == FALLBACK) {
        codes.push(FALLBACK.to_owned());
    }
    codes
}

/// Text direction for an arbitrary BCP-47 tag.
///
/// Unlike i18next — which asks the host for `Intl.Locale(...).textInfo` and
/// therefore inherits Hermes' gaps — this is a compiled-in RTL language table
/// (FR-023). The list is the RTL set i18next itself hardcodes (`i18next.js:2119`).
#[must_use]
pub fn dir_of(lng: &str) -> Dir {
    const RTL: [&str; 24] = [
        "ar", "shu", "sqr", "ssh", "xaa", "yhd", "yud", "aao", "abh", "abv", "acm", "acq", "acw",
        "he", "iw", "ps", "sd", "ug", "ur", "yi", "dv", "ku", "fa", "nqo",
    ];
    let primary = lng.split(['-', '_']).next().unwrap_or(lng);
    if RTL.contains(&primary) {
        Dir::Rtl
    } else {
        Dir::Ltr
    }
}

// ---------------------------------------------------------------------------
// Candidate keys
// ---------------------------------------------------------------------------

/// The candidate keys for one `(key, language code)` pair, in the order i18next
/// actually tries them.
///
/// i18next **pushes** onto `finalKeys` (`i18next.js:823-852`) and **pops**
/// (`:854-860`), so the try order is the reverse of the source order:
///
/// ```text
/// key_ctx_zero -> key_ctx_plural -> key_ctx -> key_zero -> key_plural -> key
/// ```
///
/// **The bare key is a last resort, reached only after the plural-suffixed lookup
/// misses.** That is the only reason the 11 corpus keys per locale that interpolate
/// `{{count}}` with no plural siblings (defect A5, 165 entries) render at all: a
/// port that tried the bare key *first* would break the plural keys, and a port that
/// omitted it would break those same 165.
pub(crate) struct Candidates<'a> {
    /// All candidates concatenated into ONE buffer. Six `String`s here was the
    /// single largest source of per-call allocation (measured: 14 per `t()`
    /// against a budget of 2), and every one of them was thrown away microseconds
    /// later.
    buf: &'a str,
    ranges: [(usize, usize); 6],
    len: usize,
}

impl Candidates<'_> {
    /// Iterate in try order — LIFO relative to how i18next builds the list.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &str> {
        self.ranges[..self.len]
            .iter()
            .rev()
            .map(move |&(s, e)| &self.buf[s..e])
    }
}

/// Whether `count` triggers plural handling at all.
///
/// `opt.count !== undefined && !isString(opt.count)` — so a **string** `count`
/// produces no plural candidate, and `t('send.recipientCount', {count: '3'})`
/// returns the raw key because only `_one`/`_other` exist. Reproduced, not repaired.
pub(crate) fn needs_plural(count: Option<&super::Count>) -> Option<f64> {
    match count {
        Some(super::Count::Num(n)) => Some(*n),
        // `Number(null)` is 0 and `Number({})` is NaN, and both are still
        // `!== undefined`, so both DO pluralise. Only a string count opts out.
        Some(super::Count::Null) => Some(0.0),
        Some(super::Count::Object) => Some(f64::NAN),
        _ => None,
    }
}

/// The plural suffix for one language code, as a borrowed `&'static str`.
fn plural_suffix_for(code: &str, n: f64, ordinal: bool, mode: PluralMode) -> &'static str {
    match mode {
        PluralMode::Cldr => {
            if ordinal {
                ordinal_category(code, n).suffix()
            } else {
                plural_category(code, n).suffix()
            }
        }
        PluralMode::Legacy => plural_category_legacy(n).suffix(),
    }
}

/// Build the candidate list for `key` under language `code`, writing into `scratch`.
///
/// i18next **pushes** onto `finalKeys` (`i18next.js:823-852`) and **pops**
/// (`:854-860`), so the try order is the reverse of the source order:
///
/// ```text
/// key_ctx_zero -> key_ctx_plural -> key_ctx -> key_zero -> key_plural -> key
/// ```
///
/// **The bare key is a last resort, reached only after the plural-suffixed lookup
/// misses.** That is the only reason the 11 corpus keys per locale that interpolate
/// `{{count}}` with no plural siblings (defect A5, 164 entries) render at all: a
/// port that tried the bare key *first* would break the plural keys, and a port that
/// omitted it would break those same 164.
///
/// `scratch` is reused across the (at most two) language codes, so the whole
/// candidate machinery costs one allocation per `t()` rather than six.
pub(crate) fn candidates_for<'a>(
    scratch: &'a mut String,
    key: &str,
    code: &str,
    count: Option<f64>,
    context: Option<&str>,
    ordinal: bool,
    mode: PluralMode,
) -> Candidates<'a> {
    scratch.clear();
    let mut ranges = [(0usize, 0usize); 6];
    let mut len = 0usize;
    let push = |scratch: &mut String,
                parts: &[&str],
                ranges: &mut [(usize, usize); 6],
                len: &mut usize| {
        if *len >= 6 {
            return;
        }
        let start = scratch.len();
        for p in parts {
            scratch.push_str(p);
        }
        ranges[*len] = (start, scratch.len());
        *len += 1;
    };

    push(scratch, &[key], &mut ranges, &mut len);

    let plural_suffix = count.map(|n| plural_suffix_for(code, n, ordinal, mode));
    // `_zero` is i18next's own extension, not CLDR (`i18next.js:602`): an EXTRA
    // candidate for cardinal count == 0, appended on top of the real category and —
    // because of the pop order — tried first.
    let needs_zero = count.is_some_and(|n| n == 0.0) && !ordinal;

    if let Some(sfx) = plural_suffix {
        if ordinal {
            push(scratch, &[key, "_ordinal", sfx], &mut ranges, &mut len);
            // The de-prefixed fallback (`i18next.js:832-833`).
            push(scratch, &[key, sfx], &mut ranges, &mut len);
        } else {
            push(scratch, &[key, sfx], &mut ranges, &mut len);
        }
        if needs_zero {
            push(scratch, &[key, "_zero"], &mut ranges, &mut len);
        }
    }

    if let Some(ctx) = context.filter(|c| !c.is_empty()) {
        push(scratch, &[key, "_", ctx], &mut ranges, &mut len);
        if let Some(sfx) = plural_suffix {
            push(scratch, &[key, "_", ctx, sfx], &mut ranges, &mut len);
            if needs_zero {
                push(scratch, &[key, "_", ctx, "_zero"], &mut ranges, &mut len);
            }
        }
    }

    Candidates {
        buf: scratch.as_str(),
        ranges,
        len,
    }
}

/// CLDR **ordinal** categories./// CLDR **ordinal** categories. Only four of the fifteen shipped locales have a
/// non-trivial ordinal rule; the rest are `other`-only (research.md D8).
fn ordinal_category(locale: &str, count: f64) -> super::plural::Category {
    use super::plural::Category;
    let primary = locale.split(['-', '_']).next().unwrap_or(locale);
    if !count.is_finite() {
        return Category::Other;
    }
    let n = count.abs();
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::allow_attributes
    )]
    let i = if n >= u64::MAX as f64 {
        u64::MAX
    } else {
        n as u64
    };
    match primary {
        // en: one: n % 10 = 1 and n % 100 != 11
        //     two: n % 10 = 2 and n % 100 != 12
        //     few: n % 10 = 3 and n % 100 != 13
        "en" => {
            let (m10, m100) = (i % 10, i % 100);
            if m10 == 1 && m100 != 11 {
                Category::One
            } else if m10 == 2 && m100 != 12 {
                Category::Two
            } else if m10 == 3 && m100 != 13 {
                Category::Few
            } else {
                Category::Other
            }
        }
        // it: many: n = 11, 8, 80, 800
        "it" => {
            if matches!(i, 11 | 8 | 80 | 800) {
                Category::Many
            } else {
                Category::Other
            }
        }
        // fr, vi: one: n = 1
        "fr" | "vi" => {
            if i == 1 && n.fract() == 0.0 {
                Category::One
            } else {
                Category::Other
            }
        }
        _ => Category::Other,
    }
}

#[cfg(test)]
mod endonym_tests {
    use super::{endonym, SUPPORTED};

    /// Every shipped locale names itself, in its own script — and no two name
    /// themselves the same, which is what a picker needs to be usable.
    #[test]
    fn every_supported_locale_names_itself() {
        let mut seen: Vec<&str> = Vec::new();
        for tag in SUPPORTED {
            let name = endonym(tag);
            assert_ne!(
                name, tag,
                "{tag} falls through to its own tag — a picker would show a language code"
            );
            assert!(!name.is_empty());
            assert!(!seen.contains(&name), "{name} names two locales");
            seen.push(name);
        }
    }

    /// An unknown tag answers with the tag. Wrong is worse than unhelpful: a
    /// silent "English" would tell somebody the app is in a language it is not.
    #[test]
    fn an_unknown_tag_is_returned_as_itself() {
        assert_eq!(endonym("xx-YY"), "xx-YY");
    }
}
