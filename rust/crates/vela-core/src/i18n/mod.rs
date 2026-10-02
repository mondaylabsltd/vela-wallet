//! Translation resolution, byte-faithful to `i18next@26.3.1`.
//!
//! Feature spec: `specs/004-rust-i18n/`. The authoritative exported surface is
//! `contracts/i18n-api.md`; the 18,975-case corpus in `tests/vectors/i18n-*.json`
//! is generated from the real JS package and pins byte-identical behaviour.
//!
//! Conformance target is **MODE A** — `i18next` running with a complete
//! `Intl.PluralRules`. The degraded Hermes behaviour (`count === 1 ? one : other`)
//! is reproducible via [`PluralMode::Legacy`] so the delta stays enumerable, but it
//! is a defect being closed, not a contract.

pub mod catalog;
pub mod interpolate;
mod paths;
pub mod plural;
pub mod resolve;

pub use catalog::{Catalog, Lookup};
pub use plural::{
    plural_categories, plural_category, plural_category_legacy, plural_suffix,
    plural_suffix_legacy, plural_suffixes, plural_suffixes_legacy,
};
pub use plural::{Category, PluralMode};
pub use resolve::{apple_localizations, resolve_language, Dir, LanguageState, SUPPORTED};

/// Canonicalise a BCP-47 tag the way i18next's `formatLanguageCode` does — which
/// only rewrites codes containing `-`, so bare `ZH` is returned unchanged.
#[must_use]
pub fn canonical_tag(code: &str) -> String {
    resolve::format_language_code(code)
}

use crate::error::CoreError;
use std::borrow::Cow;

// ---------------------------------------------------------------------------
// Per-call options
// ---------------------------------------------------------------------------

/// A `count`, in the three shapes i18next can receive one.
///
/// The distinction is load-bearing: a **string** `count` silently disables plural
/// resolution in i18next (`Intl.PluralRules.select` is never reached, so the raw
/// key comes back), and a **BigInt** makes it throw. Coercing either would be
/// "helpful" and wrong.
#[derive(Debug, Clone, PartialEq)]
pub enum Count {
    /// A JS number. May be fractional, negative, or NaN.
    Num(f64),
    /// A string `count` — plural resolution does not happen at all.
    Str(String),
    /// A BigInt `count` — i18next throws `TypeError`; we return `I18nInvalidCount`.
    BigInt(i128),
    /// `count: null`. Still triggers plural handling (`count !== undefined`), and
    /// `Number(null)` is 0, so it selects the zero category. Renders as `""`.
    Null,
    /// A non-numeric object `count`. Triggers plural handling; `Number({})` is
    /// `NaN`, which selects `other`. Renders as `[object Object]`.
    Object,
}

/// An interpolation value, carrying the JS type distinctions that change output.
#[derive(Debug, Clone, PartialEq)]
pub enum Var<'a> {
    Str(&'a str),
    Num(f64),
    Bool(bool),
    /// `null` — renders as the empty string.
    Null,
    /// Present as an own property but `undefined` — renders as the empty string.
    /// Distinct from *absent*, which leaves the literal `{{var}}` on screen
    /// (`skipOnVariables: true`). Two code paths, two user-visible results.
    Undefined,
    /// `[object Object]`.
    Object,
    /// A JS array — renders as its comma-joined elements.
    Array(&'a [&'a str]),
}

/// Per-call options. Borrowed so the hot path allocates nothing;
/// see [`OwnedOptions`] when the caller cannot borrow.
#[derive(Debug, Default, Clone)]
pub struct Options<'a> {
    pub count: Option<Count>,
    pub context: Option<&'a str>,
    pub default_value: Option<&'a str>,
    /// Per-call language override. **Not** the same code path as
    /// [`I18n::change_language`] — proved by the corpus: `zh_TW` resolves to `zh`
    /// through `change_language` but falls through to `en` here.
    pub lng: Option<&'a str>,
    pub ordinal: bool,
    /// Interpolation variables, in insertion order.
    pub vars: &'a [(&'a str, Var<'a>)],
    /// `defaultValue_one`, `defaultValue_other`, … keyed by the bare category name.
    pub default_value_variants: &'a [(&'a str, &'a str)],
    /// Set when the caller passed an option i18next answers with a non-string
    /// (`returnObjects`, `returnDetails`, an array `defaultValue` under
    /// `joinArrays`). `t()` rejects up front.
    pub unsupported: Option<&'a str>,
    /// A per-call namespace override. Anything other than `translation` misses,
    /// because the app runs a single namespace.
    pub ns: Option<&'a str>,
    /// `keySeparator: false` — the key is looked up as ONE literal property rather
    /// than a dotted path.
    pub key_separator_off: bool,
    /// `nsSeparator: false` — a `:` in the key is not a namespace separator.
    pub ns_separator_off: bool,
    /// `defaultValue` was an object or array. i18next returns the value itself,
    /// which is a non-string, so `t()` answers with the object diagnostic.
    pub default_value_object: bool,
}

impl<'a> Options<'a> {
    /// The `defaultValue_<category>` variant for `category`, if supplied.
    fn default_value_for(&self, category: &str) -> Option<&'a str> {
        self.default_value_variants
            .iter()
            .find(|(k, _)| *k == category)
            .map(|(_, v)| *v)
    }
}

/// Owned mirror of [`Options`], for callers that decode options at runtime —
/// principally the conformance runner, which builds them from JSON.
#[derive(Debug, Default, Clone)]
pub struct OwnedOptions {
    pub count: Option<Count>,
    pub context: Option<String>,
    pub default_value: Option<String>,
    /// `defaultValue_one`, `defaultValue_other`, … keyed by the bare category name.
    /// i18next picks among these with a `||` chain driven by the **active**
    /// language's suffix, not the per-fallback-code one.
    pub default_value_variants: Vec<(String, String)>,
    pub lng: Option<String>,
    pub ns: Option<String>,
    pub ordinal: bool,
    pub key_separator_off: bool,
    pub ns_separator_off: bool,
    pub default_value_object: bool,
    pub vars: Vec<(String, OwnedVar)>,
    /// Options this engine deliberately does not support, because i18next returns
    /// a non-string for them (`returnObjects`, `returnDetails`, `joinArrays`).
    /// Phase 3 turns any entry here into [`CoreError::I18nUnsupportedOption`], which
    /// is what the corpus expects — a Rust `t()` is string-typed by construction.
    pub unsupported: Vec<String>,
}

/// Borrow buffers for [`OwnedOptions::as_options`], so the borrowed [`Options`]
/// can point at slices without the owned form allocating on every call.
#[derive(Debug, Default)]
pub struct Scratch<'a> {
    vars: Vec<(&'a str, Var<'a>)>,
    variants: Vec<(&'a str, &'a str)>,
}

/// Owned mirror of [`Var`].
#[derive(Debug, Clone, PartialEq)]
pub enum OwnedVar {
    Str(String),
    Num(f64),
    Bool(bool),
    Null,
    Undefined,
    Object,
    /// A JS array, stored **already joined** with `,` — which is exactly how one
    /// interpolates. Nested arrays flatten first (`[[1],[2]]` -> `"1,2"`).
    Array(String),
}

impl OwnedOptions {
    /// Borrow as [`Options`]. The returned value borrows `self` and the scratch
    /// buffers, which must outlive it.
    pub fn as_options<'a>(&'a self, scratch: &'a mut Scratch<'a>) -> Options<'a> {
        let Scratch {
            vars: scratch,
            variants,
        } = scratch;
        scratch.clear();
        variants.clear();
        for (k, v) in &self.default_value_variants {
            variants.push((k.as_str(), v.as_str()));
        }
        for (name, v) in &self.vars {
            let borrowed = match v {
                OwnedVar::Str(s) => Var::Str(s.as_str()),
                OwnedVar::Num(n) => Var::Num(*n),
                OwnedVar::Bool(b) => Var::Bool(*b),
                OwnedVar::Null => Var::Null,
                OwnedVar::Undefined => Var::Undefined,
                OwnedVar::Object => Var::Object,
                // The owned form already holds the JOINED text (see `OwnedVar::Array`),
                // so it borrows as a plain string. Borrowing `&[&str]` here would need
                // an arena for the element pointers, for no observable difference:
                // a JS array interpolates as `Array.prototype.join(",")` and nothing
                // downstream can see the elements.
                OwnedVar::Array(joined) => Var::Str(joined.as_str()),
            };
            scratch.push((name.as_str(), borrowed));
        }
        Options {
            count: self.count.clone(),
            context: self.context.as_deref(),
            default_value: self.default_value.as_deref(),
            lng: self.lng.as_deref(),
            ordinal: self.ordinal,
            vars: scratch.as_slice(),
            default_value_variants: variants.as_slice(),
            unsupported: self.unsupported.first().map(String::as_str),
            ns: self.ns.as_deref(),
            key_separator_off: self.key_separator_off,
            ns_separator_off: self.ns_separator_off,
            default_value_object: self.default_value_object,
        }
    }

    /// The first unsupported option name, if any. Phase 3's `t()` checks this
    /// before doing any work, so the corpus's three `I18nUnsupportedOption` cases
    /// fail fast rather than resolving and then being discarded.
    #[must_use]
    pub fn first_unsupported(&self) -> Option<&str> {
        self.unsupported.first().map(String::as_str)
    }
}

// ---------------------------------------------------------------------------
// The engine
// ---------------------------------------------------------------------------

/// One engine: an active language, a resolve hierarchy, and at most two resident
/// catalogs (the active locale and the pinned `en` fallback).
///
/// `en` is a **field, not a slot**, so FR-013's "the fallback is pinned" is a
/// type-level guarantee — there is nowhere to put a third catalog, so a language
/// switch cannot leak (FR-012).
#[derive(Debug)]
pub struct I18n {
    en: Catalog,
    active: Option<Catalog>,
    state: LanguageState,
    plural_mode: PluralMode,
}

impl I18n {
    /// Construct with the pinned `en` fallback. Errors if `en.lang() != "en"`.
    pub fn new(en: Catalog) -> Result<Self, CoreError> {
        if en.lang() != "en" {
            return Err(CoreError::I18nCatalogUnavailable(format!(
                "fallback catalog must be `en`, got `{}`",
                en.lang()
            )));
        }
        Ok(Self {
            en,
            active: None,
            state: resolve::resolve_language("en"),
            plural_mode: PluralMode::Cldr,
        })
    }

    /// Construct from the compiled-in `en` catalog (cargo feature `i18n-en`).
    pub fn embedded() -> Result<Self, CoreError> {
        Self::new(Catalog::embedded("en")?)
    }

    /// Consuming builder for the plural mode. Default is [`PluralMode::Cldr`].
    ///
    /// Construction-time and never per-call: a mid-session switch would make the
    /// FR-007 parity claim meaningless.
    #[must_use]
    pub fn with_plural_mode(mut self, mode: PluralMode) -> Self {
        self.plural_mode = mode;
        self
    }

    // -- catalog lifecycle (FR-012 .. FR-016) -------------------------------

    /// Make `catalog` the active locale's catalog (FR-012).
    ///
    /// Returns the catalog it displaced, so the caller can *observe* the drop
    /// rather than trust it. There is one active slot and `en` is a separate
    /// field, so residency is bounded at two by the type — not by a policy anyone
    /// has to remember.
    pub fn load_catalog(&mut self, catalog: Catalog) -> Option<Catalog> {
        self.active.replace(catalog)
    }

    /// Release `lang` if it is the active catalog (FR-013).
    ///
    /// Returns `None` when `lang` is not resident — including when it is `en`:
    /// releasing the pinned fallback is not expressible, because `en` is a field
    /// rather than a slot. Release is a plain `Drop`; there is no cache to
    /// invalidate, which is what keeps FR-024's flat-memory claim true.
    pub fn release_catalog(&mut self, lang: &str) -> Option<Catalog> {
        match &self.active {
            Some(c) if c.lang() == lang => self.active.take(),
            _ => None,
        }
    }

    /// Which locales are resident. Lets a host tell "fell back to en" from
    /// "translated", which the returned string alone cannot express.
    #[must_use]
    pub fn resident_locales(&self) -> Vec<&str> {
        match &self.active {
            Some(c) => vec![c.lang(), self.en.lang()],
            None => vec![self.en.lang()],
        }
    }

    /// Total bytes attributable to the resident catalogs (the SC-005 instrument).
    #[must_use]
    pub fn resident_bytes(&self) -> usize {
        self.en.resident_bytes() + self.active.as_ref().map_or(0, Catalog::resident_bytes)
    }

    /// Whether `lng` is resident. `false` means a `t()` for it falls through to `en`.
    #[must_use]
    pub fn is_resident(&self, lng: &str) -> bool {
        lng == self.en.lang() || self.active.as_ref().is_some_and(|c| c.lang() == lng)
    }

    /// The catalog for `code`, or `None` when that locale is not resident.
    ///
    /// The language comparison happens **before** any read, so a stale active
    /// catalog during a mid-flight language switch is skipped rather than
    /// rendered — a race between a render and a catalog load must never show a
    /// third language.
    fn catalog_for(&self, code: &str) -> Option<&Catalog> {
        if let Some(c) = &self.active {
            if c.lang() == code {
                return Some(c);
            }
        }
        if self.en.lang() == code {
            return Some(&self.en);
        }
        None
    }

    // -- the seven i18next members -----------------------------------------

    /// Set the active language. Infallible and synchronous; it does **not** load a
    /// catalog — the core has no I/O.
    pub fn change_language(&mut self, lng: &str) -> LanguageState {
        self.state = resolve::resolve_language(lng);
        self.state.clone()
    }

    #[must_use]
    pub fn language(&self) -> &str {
        &self.state.language
    }

    #[must_use]
    pub fn resolved_language(&self) -> Option<&str> {
        self.state.resolved_language.as_deref()
    }

    #[must_use]
    pub fn languages(&self) -> &[String] {
        &self.state.languages
    }

    #[must_use]
    pub fn plural_mode(&self) -> PluralMode {
        self.plural_mode
    }

    // -- L10n helpers that need a translated label ---------------------------

    /// Compact relative time — `"now"`, `"2m"`, `"3h"`, `"Sat"`, `"06/13/2026"`.
    ///
    /// Replaces `src/services/activity.ts:111`. `Result` only because the first
    /// three branches resolve `time.now` / `time.minutesShort` / `time.hoursShort`
    /// through `t()`; the arithmetic itself is infallible.
    ///
    /// The weekday branch is the one that changes: `activity.ts:121` calls
    /// `d.toLocaleDateString(lng, { weekday: 'short' })`, the last host-`Intl`
    /// dependency on this path and unreliable on Hermes for the same reason the
    /// plural rules were. It now reads the compiled-in table.
    pub fn format_relative_time(
        &self,
        ts_seconds: i64,
        now_ms: i64,
        utc_offset_minutes: i32,
        date_preset: crate::l10n::DatePreset,
    ) -> Result<String, CoreError> {
        let diff = (now_ms.div_euclid(1000) - ts_seconds).max(0);
        let opts = Options::default();
        if diff < 45 {
            return self.t("time.now", &opts);
        }
        let civil = crate::l10n::Civil::from_unix_millis(
            ts_seconds.saturating_mul(1000),
            utc_offset_minutes,
        );
        if diff < 3_600 {
            let n = (diff as f64 / 60.0).round();
            return self.t_with_n("time.minutesShort", n);
        }
        if diff < 86_400 {
            let n = (diff as f64 / 3_600.0).round();
            return self.t_with_n("time.hoursShort", n);
        }
        if diff < 7 * 86_400 {
            return Ok(crate::l10n::weekday_name(&civil, &self.state.language).to_owned());
        }
        Ok(crate::l10n::format_date(&civil, date_preset))
    }

    /// Resolve `key` with a single numeric `{{n}}` variable.
    fn t_with_n(&self, key: &str, n: f64) -> Result<String, CoreError> {
        let mut buf = ryu_js::Buffer::new();
        let rendered = buf.format(n);
        let vars = [("n", Var::Str(rendered))];
        let opts = Options {
            vars: &vars,
            ..Options::default()
        };
        self.t(key, &opts)
    }

    /// Activity-feed date header — `"Today"`, `"Yesterday"`, else the date preset.
    ///
    /// Replaces `src/services/activity.ts:134`. The comparison is between **civil
    /// days**, not a 24-hour difference: at 00:30 local, an event from 23:00 the
    /// previous evening is "Yesterday", which a millisecond subtraction gets wrong.
    pub fn day_label(
        &self,
        ts_seconds: i64,
        now_ms: i64,
        utc_offset_minutes: i32,
        date_preset: crate::l10n::DatePreset,
    ) -> Result<String, CoreError> {
        let then = crate::l10n::Civil::from_unix_millis(
            ts_seconds.saturating_mul(1000),
            utc_offset_minutes,
        );
        let now = crate::l10n::Civil::from_unix_millis(now_ms, utc_offset_minutes);
        let opts = Options::default();
        match now.days_between(&then) {
            d if d <= 0 => self.t("time.today", &opts),
            1 => self.t("time.yesterday", &opts),
            _ => Ok(crate::l10n::format_date(&then, date_preset)),
        }
    }

    /// Text direction of the active language.
    #[must_use]
    pub fn dir(&self) -> Dir {
        resolve::dir_of(&self.state.language)
    }

    /// Text direction of an arbitrary tag.
    #[must_use]
    pub fn dir_of(lng: &str) -> Dir {
        resolve::dir_of(lng)
    }

    /// Whether `key` resolves to anything. A branch node counts as present.
    #[must_use]
    pub fn exists(&self, key: &str, opts: &Options<'_>) -> bool {
        !matches!(self.lookup(key, opts), Resolved::Missing)
    }

    /// Resolve `key`.
    ///
    /// Returns the key itself when nothing resolves and no `default_value`
    /// applies — i18next's behaviour, and not an error.
    pub fn t(&self, key: &str, opts: &Options<'_>) -> Result<String, CoreError> {
        self.translate(key, opts, 0)
    }

    /// First key that resolves wins.
    ///
    /// When **none** resolve, returns the **last** key, because i18next indexes
    /// `keys[keys.length - 1]`.
    pub fn t_first(&self, keys: &[&str], opts: &Options<'_>) -> Result<String, CoreError> {
        let Some((last, rest)) = keys.split_last() else {
            return Err(CoreError::I18nEmptyKeyList(
                "t_first called with no keys".to_owned(),
            ));
        };
        for k in rest {
            if !matches!(self.lookup(k, opts), Resolved::Missing) {
                return self.translate(k, opts, 0);
            }
        }
        self.translate(last, opts, 0)
    }

    /// `getFixedT` — pins a language, namespace and/or key prefix.
    ///
    /// `get_fixed_t` sets the per-call `lng` (`i18next.js:2038`) and therefore
    /// inherits the **per-call** semantics, so a `FixedT` pinned to `zh_TW`
    /// resolves English. That is i18next's behaviour, not a bug in the port.
    #[must_use]
    pub fn get_fixed_t<'a>(
        &'a self,
        lng: Option<&'a str>,
        key_prefix: Option<&'a str>,
    ) -> FixedT<'a> {
        FixedT {
            engine: self,
            lng,
            key_prefix,
        }
    }

    // -- resolution ---------------------------------------------------------

    /// The codes to try, in order. At most two, and **borrowed** — cloning the
    /// language strings on every `t()` was three of the fourteen allocations the
    /// SC-007 bench caught.
    fn hierarchy<'a>(&'a self, opts: &Options<'a>) -> ([&'a str; 2], usize) {
        match opts.lng {
            // The per-call path has NO recovery ladder — a different function
            // upstream, and the corpus pins the two separately. It resolves to a
            // `&'static` entry of SUPPORTED, so it allocates nothing either.
            Some(l) => match resolve::supported_tag(l) {
                Some(tag) if tag != resolve::FALLBACK => ([tag, resolve::FALLBACK], 2),
                _ => ([resolve::FALLBACK, ""], 1),
            },
            None => {
                let langs = &self.state.languages;
                match langs.len() {
                    0 => ([resolve::FALLBACK, ""], 1),
                    1 => ([langs[0].as_str(), ""], 1),
                    _ => ([langs[0].as_str(), langs[1].as_str()], 2),
                }
            }
        }
    }

    fn lookup(&self, key: &str, opts: &Options<'_>) -> Resolved<'_> {
        // A namespace prefix is stripped and otherwise ignored: the app runs a
        // single `translation` namespace, and an unknown namespace makes i18next
        // return the bare key.
        let (key, foreign_ns) = split_namespace(key, opts);
        if foreign_ns {
            return Resolved::Missing;
        }
        let key = key.as_ref();
        // `keySeparator: false` looks the key up as ONE literal property. The
        // corpus has zero literal-dot JSON keys (research D6), so a dotted key
        // under this option can only miss — matching a flattened path here would
        // resolve something i18next never would.
        if opts.key_separator_off && key.contains('.') {
            return Resolved::Missing;
        }

        let count = resolve::needs_plural(opts.count.as_ref());
        let (codes, n_codes) = self.hierarchy(opts);
        // One scratch buffer for the whole candidate sweep, reused across both
        // language codes — six `String`s per call was the bench's biggest finding.
        let mut scratch = String::with_capacity(key.len() + 24);
        for code in &codes[..n_codes] {
            let Some(catalog) = self.catalog_for(code) else {
                continue;
            };
            // The plural suffix is recomputed PER CODE, not once per call: under
            // `fr` with a count of 1,000,000 this looks up `_many` in fr, misses,
            // then looks up `_other` in en. That recomputation is exactly why four
            // locales currently leak English for large numbers.
            let candidates = resolve::candidates_for(
                &mut scratch,
                key,
                code,
                count,
                opts.context,
                opts.ordinal,
                self.plural_mode,
            );
            for candidate in candidates.iter() {
                match catalog.get(candidate) {
                    Lookup::Value(v) => return Resolved::Value(v),
                    Lookup::Branch => return Resolved::Branch,
                    Lookup::Missing => {}
                }
            }
        }
        Resolved::Missing
    }

    /// The `defaultValue` precedence chain (`i18next.js:603`).
    ///
    /// The suffix comes from the **active** language, not from each fallback code.
    /// `default_value: null` and own-property `undefined` are ignored (the key
    /// echoes) while `default_value: ""` is honoured — two adjacent falsy values
    /// with opposite outcomes.
    fn default_value<'a>(&self, opts: &'a Options<'a>) -> Option<&'a str> {
        let count = resolve::needs_plural(opts.count.as_ref());
        if let Some(n) = count {
            let active = &self.state.language;
            if n == 0.0 && !opts.ordinal {
                if let Some(v) = opts.default_value_for("zero") {
                    return Some(v);
                }
            }
            let cat = match self.plural_mode {
                PluralMode::Cldr => plural::plural_category(active, n),
                PluralMode::Legacy => plural::plural_category_legacy(n),
            };
            if let Some(v) = opts.default_value_for(cat.name()) {
                return Some(v);
            }
        }
        opts.default_value
    }

    fn translate(&self, key: &str, opts: &Options<'_>, depth: usize) -> Result<String, CoreError> {
        // Options i18next answers with a non-string. A Rust `t()` is string-typed
        // by construction, so these are typed errors rather than silent coercions.
        if let Some(name) = opts.unsupported {
            return Err(CoreError::I18nUnsupportedOption(name.to_owned()));
        }
        if let Some(Count::BigInt(v)) = &opts.count {
            return Err(CoreError::I18nInvalidCount(format!(
                "BigInt count {v} — Intl.PluralRules.select throws on a BigInt"
            )));
        }

        let owned_key;
        let raw: &str = match self.lookup(key, opts) {
            Resolved::Value(v) => v,
            // A branch node is NOT an error. The diagnostic names the ACTIVE
            // language, not the language the value was found in (`i18next.js:618`).
            Resolved::Branch => {
                return Ok(format!(
                    "key '{key} ({})' returned an object instead of string.",
                    self.state.language
                ))
            }
            Resolved::Missing => {
                // An object or array `defaultValue` is itself a non-string, so
                // i18next answers with the same diagnostic a branch node produces.
                if opts.default_value_object {
                    return Ok(format!(
                        "key '{key} ({})' returned an object instead of string.",
                        self.state.language
                    ));
                }
                match self.default_value(opts) {
                    Some(d) => d,
                    // Nothing resolved and no default: the key echoes — with its
                    // namespace already stripped.
                    None => {
                        owned_key = split_namespace(key, opts).0;
                        owned_key.as_ref()
                    }
                }
            }
        };

        // ONE allocation for the output: `interpolate` copies straight out of the
        // catalog slice rather than through an intermediate owned copy.
        let interpolated = interpolate::interpolate(raw, opts)?;
        // `skipOnVariables` (i18next `extendTranslation`, `nestBef < nestAft`):
        // when filling the variables added a `$t()` call the template did not
        // have, nothing in the string is expanded — the variable's call and
        // the template's own stay as text. A value from outside (a revert
        // reason, a token symbol, a page's title) can therefore never pull a
        // wallet sentence into the one it is shown in. Counted with the
        // matcher expansion itself uses.
        if interpolated.contains("$t(") && nest_calls(&interpolated) > nest_calls(raw) {
            return Ok(interpolated);
        }
        self.expand_nesting(interpolated, opts, depth)
    }

    /// Expand `$t(...)` references against the **active** locale.
    fn expand_nesting(
        &self,
        input: String,
        opts: &Options<'_>,
        depth: usize,
    ) -> Result<String, CoreError> {
        if depth >= interpolate::MAX_NEST_DEPTH || !input.contains("$t(") {
            return Ok(input);
        }
        let mut out = String::with_capacity(input.len());
        let mut rest = input.as_str();
        while let Some(call) = interpolate::find_nest(rest) {
            out.push_str(&rest[..call.range.start]);
            // The nested call inherits the outer options but NOT its
            // `default_value` — otherwise a self-reference would recurse on the
            // same default forever. i18next breaks the same cycle at depth 1.
            let nested_opts = Options {
                count: call.count.map(Count::Num).or_else(|| opts.count.clone()),
                context: opts.context,
                default_value: None,
                lng: opts.lng,
                ordinal: opts.ordinal,
                vars: opts.vars,
                default_value_variants: &[],
                unsupported: None,
                ns: opts.ns,
                key_separator_off: opts.key_separator_off,
                ns_separator_off: opts.ns_separator_off,
                default_value_object: false,
            };
            out.push_str(&self.translate(&call.key, &nested_opts, depth + 1)?);
            rest = &rest[call.range.end..];
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// How many `$t()` calls [`I18n::expand_nesting`] would make in `s`: the same
/// left-to-right walk, stopping where it stops (a call with no closing
/// parenthesis). i18next counts its nesting-regexp matches for the same rule.
fn nest_calls(s: &str) -> usize {
    let mut count = 0;
    let mut rest = s;
    while let Some(call) = interpolate::find_nest(rest) {
        count += 1;
        rest = &rest[call.range.end..];
    }
    count
}

/// What a candidate-key sweep found.
enum Resolved<'a> {
    Value(&'a str),
    Branch,
    Missing,
}

/// `looksLikeObjectPath` (`i18next.js:144-158`).
///
/// A key containing a space, comma, `?`, `!` or `;` is natural language rather
/// than a namespaced path — **unless** the part before its first `.` is clean, in
/// which case it is still treated as a path. Without this, `"Price: $1.00"` would
/// be torn apart on its colon and render as `" $1.00"`.
fn looks_like_object_path(key: &str) -> bool {
    const CHARS: [char; 5] = [' ', ',', '?', '!', ';'];
    let has = |s: &str| s.contains(CHARS);
    if !has(key) {
        return true;
    }
    matches!(key.find('.'), Some(ki) if ki > 0 && !has(&key[..ki]))
}

/// `extractFromKey` (`i18next.js:507-530`). Returns `(key, foreign_namespace)`.
///
/// Returns a `Cow` because the overwhelmingly common case — a key with no `:` —
/// must not allocate. Copying the key on every `t()` was the third of the three
/// allocations the SC-007 bench was still catching after the candidate buffer and
/// the interpolator were fixed.
fn split_namespace<'k>(key: &'k str, opts: &Options<'_>) -> (Cow<'k, str>, bool) {
    // An explicit `ns` option that is not the app's single namespace misses
    // outright, whatever the key looks like.
    if opts.ns.is_some_and(|n| n != "translation") {
        return (Cow::Borrowed(key), true);
    }
    if opts.ns_separator_off || !key.contains(':') {
        return (Cow::Borrowed(key), false);
    }
    // `seemsNaturalLanguage` short-circuits the split. The app sets neither
    // separator at init, so the `userDefined*` guards are both false here.
    if !opts.key_separator_off && !looks_like_object_path(key) {
        return (Cow::Borrowed(key), false);
    }
    // A `$t()` reference is never split (`i18next.js:515-521`).
    if key.contains("$t(") {
        return (Cow::Borrowed(key), false);
    }
    let mut parts: Vec<&str> = key.split(':').collect();
    let ns = parts.remove(0);
    // The REMAINING parts rejoin with the KEY separator, not the namespace one —
    // which is why `translation::common.cancel` becomes `.common.cancel`.
    let joined = parts.join(if opts.key_separator_off { "" } else { "." });
    (Cow::Owned(joined), ns != "translation")
}

/// A `t` with a language, namespace and/or key prefix pinned.
#[derive(Debug)]
pub struct FixedT<'a> {
    engine: &'a I18n,
    lng: Option<&'a str>,
    key_prefix: Option<&'a str>,
}

impl FixedT<'_> {
    /// Resolve `key` under the pinned settings.
    pub fn t(&self, key: &str, opts: &Options<'_>) -> Result<String, CoreError> {
        let full;
        let key = match self.key_prefix {
            Some(p) => {
                full = format!("{p}.{key}");
                full.as_str()
            }
            None => key,
        };
        let mut o = opts.clone();
        if self.lng.is_some() {
            o.lng = self.lng;
        }
        self.engine.t(key, &o)
    }
}

/// Interpolate `template` in isolation, without a key lookup. Exposed because the
/// corpus pins interpolation separately from resolution (`i18n_interpolate`).
pub fn interpolate(template: &str, opts: &Options<'_>) -> Result<String, CoreError> {
    interpolate::interpolate(template, opts)
}
