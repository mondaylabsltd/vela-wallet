//! Nimiq-style identicons — an exact port of `identicons-esm@1.0.1`.
//!
//! Feature spec: `specs/003-rust-identicon/`. The identicon is the avatar users
//! recognise their accounts by, so two platforms drawing the same address
//! differently is not a cosmetic bug — it breaks a verification signal. Every
//! output here is byte-identical to the JS library the app ships, pinned by the
//! `identicon` conformance suite.
//!
//! # Why this module is more fragile than it looks
//!
//! The hash is a chaotic `f64` iteration whose result is rendered by JavaScript's
//! `Number.prototype.toString(10)` and then sliced **as a decimal string**. Three
//! details are load-bearing, and each has silently broken a naive port:
//!
//! 1. **Float formatting** (research D2). ECMAScript specifies shortest-round-trip
//!    digits *with a particular tie-break*. Rust's own formatter resolves ties
//!    differently (measured: 2 disagreements in 9,068 doubles), so formatting goes
//!    through [`ryu_js`], which implements the ECMAScript rule.
//! 2. **Code units, not code points** (research D3). The upstream reads
//!    `ch.charCodeAt(0)`, so an astral character contributes its *high surrogate*.
//!    `ch as u32` would diverge on every emoji.
//! 3. **Multiply order** (research D8). `(1.0 - a) * a * K` is contract, not style.
//!    No `mul_add`, no reassociation — a "harmless" simplification changes the
//!    avatars of real accounts.
//!
//! Two further upstream quirks are deliberately preserved, not tidied: digit 5 of
//! the hash is read twice (once for `face`, once for `top`), and the `main` colour
//! is adjusted once while `accent` is adjusted in a loop.
//!
//! # Cost
//!
//! The 100-iteration chaos loop is precomputed at compile time for ASCII
//! ([`CHAOS_TABLE`], 1 KB of `.rodata`), so hashing an address is one multiply-add
//! per character. Section artwork is `&'static str`. Assembly does a single
//! `String::with_capacity` of the exact final length. The module holds **no mutable
//! state and no cache** — callers cache, at a bound they choose (research D10).

use std::borrow::Cow;

use base64::Engine as _;

use crate::error::CoreError;
use crate::identicon_features::{BOTTOM, FACE, SIDES, TOP};

// ---------------------------------------------------------------------------
// Palettes and shared SVG fragments (verbatim from identicons-esm)
// ---------------------------------------------------------------------------

/// Palette for the `main` and `accent` colours.
pub const COLORS: [&str; 10] = [
    "#FC8702", "#D94432", "#E9B213", "#1A5493", "#0582CA", "#5961A8", "#21BCA5", "#FA7268",
    "#88B04B", "#795548",
];

/// Palette for the `background` colour. Differs from [`COLORS`] at indices 3 and 5.
pub const BACKGROUND_COLORS: [&str; 10] = [
    "#FC8702", "#D94432", "#E9B213", "#1F2348", "#0582CA", "#5F4B8B", "#21BCA5", "#FA7268",
    "#88B04B", "#795548",
];

/// Inner shadow overlay.
pub const DEFAULT_SHADOW: &str = "<path fill=\"#010101\" d=\"M119.21 80a39.46 39.46 0 0 1-67.13 28.13c10.36 2.33 36 3 49.82-14.28 10.39-12.47 8.31-33.23 4.16-43.26A39.35 39.35 0 0 1 119.21 80\" opacity=\".1\"/>";

/// The hexagon used as the clip path in stock output.
pub const DEFAULT_BACKGROUND_SHAPE: &str = "<path d=\"m126.074 16.999 31.955 55.003a15.92 15.92 0 0 1 2.159 7.999 15.93 15.93 0 0 1-2.159 7.998l-31.955 55.003c-2.867 4.949-8.183 7.998-13.933 7.998H48.225c-5.75 0-11.066-3.049-13.933-7.998L2.337 87.999a15.96 15.96 0 0 1 0-15.997l31.96-55.003a16.048 16.048 0 0 1 5.89-5.854A16.173 16.173 0 0 1 48.23 9h63.91c5.75 0 11.066 3.05 13.933 7.999Z\"/>";

/// Returned by [`create_identicon`] when address validation rejects the input.
pub const IDENTICON_PLACEHOLDER: &str = "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 64 64\" fill=\"none\"><path fill=\"url(#a)\" transform=\"translate(0,4)\" d=\"M62.3 25.4 49.2 2.6A5.3 5.3 0 0 0 44.6 0H18.4c-1.9 0-3.6 1-4.6 2.6L.7 25.4c-1 1.6-1 3.6 0 5.2l13.1 22.8c1 1.6 2.7 2.6 4.6 2.6h26.2c1.9 0 3.6-1 4.6-2.6l13-22.8c1-1.6 1-3.6.1-5.2z\" opacity=\".1\"/><defs><radialGradient id=\"a\" cx=\"0\" cy=\"0\" r=\"1\" gradientTransform=\"matrix(-63.0033 0 0 -56 63 56)\" gradientUnits=\"userSpaceOnUse\"><stop stop-color=\"#260133\"/><stop offset=\"1\" stop-color=\"#1F2348\"/></radialGradient></defs></svg>";

/// [`IDENTICON_PLACEHOLDER`], pre-encoded as a data URI.
pub const IDENTICON_PLACEHOLDER_BASE64: &str = "data:image/svg+xml;base64,PHN2ZyB4bWxucz0iaHR0cDovL3d3dy53My5vcmcvMjAwMC9zdmciIHZpZXdCb3g9IjAgMCA2NCA2NCIgZmlsbD0ibm9uZSI+PHBhdGggZmlsbD0idXJsKCNhKSIgdHJhbnNmb3JtPSJ0cmFuc2xhdGUoMCw0KSIgZD0iTTYyLjMgMjUuNCA0OS4yIDIuNkE1LjMgNS4zIDAgMCAwIDQ0LjYgMEgxOC40Yy0xLjkgMC0zLjYgMS00LjYgMi42TC43IDI1LjRjLTEgMS42LTEgMy42IDAgNS4ybDEzLjEgMjIuOGMxIDEuNiAyLjcgMi42IDQuNiAyLjZoMjYuMmMxLjkgMCAzLjYtMSA0LjYtMi42bDEzLTIyLjhjMS0xLjYgMS0zLjYuMS01LjJ6IiBvcGFjaXR5PSIuMSIvPjxkZWZzPjxyYWRpYWxHcmFkaWVudCBpZD0iYSIgY3g9IjAiIGN5PSIwIiByPSIxIiBncmFkaWVudFRyYW5zZm9ybT0ibWF0cml4KC02My4wMDMzIDAgMCAtNTYgNjMgNTYpIiBncmFkaWVudFVuaXRzPSJ1c2VyU3BhY2VPblVzZSI+PHN0b3Agc3RvcC1jb2xvcj0iIzI2MDEzMyIvPjxzdG9wIG9mZnNldD0iMSIgc3RvcC1jb2xvcj0iIzFGMjM0OCIvPjwvcmFkaWFsR3JhZGllbnQ+PC9kZWZzPjwvc3ZnPg==";

/// Artworks available per section; indices run `1..=SECTION_COUNT`.
pub const SECTION_COUNT: usize = 21;

/// Seed length cap, in UTF-16 code units, applied by [`normalize_seed`].
pub const SEED_MAX_UTF16_LEN: usize = 128;

const CIRCLE_PREFIX: &str = "<circle cx=\"80\" cy=\"80\" r=\"40\" fill=\"";
const CIRCLE_SUFFIX: &str = "\"/>";

/// `<circle …/>` centred on the canvas — the identicon's "head".
#[must_use]
pub fn default_circle_shape(color: &str) -> String {
    let mut out = String::with_capacity(CIRCLE_PREFIX.len() + color.len() + CIRCLE_SUFFIX.len());
    out.push_str(CIRCLE_PREFIX);
    out.push_str(color);
    out.push_str(CIRCLE_SUFFIX);
    out
}

// ---------------------------------------------------------------------------
// The chaotic hash
// ---------------------------------------------------------------------------

const CHAOS_K: f64 = 3.569956786876;
const CHAOS_ITERATIONS: usize = 100;

/// Precomputed for every ASCII code unit — the range every real seed uses.
const CHAOS_TABLE_LEN: usize = 128;

/// `chaosHash` — a logistic-map iteration, and the whole cost of hashing.
///
/// The expression order is part of the algorithm's contract: `(1.0 - a) * a * K`
/// left-associates exactly as the JavaScript does. Never rewrite this as
/// `a.mul_add(…)` or reorder the factors — IEEE-754 would give a different double,
/// and a different double means real users' avatars change (research D8).
fn chaos_hash(number: f64) -> f64 {
    let mut a_n = 1.0 / number;
    let mut i = 0;
    while i < CHAOS_ITERATIONS {
        a_n = (1.0 - a_n) * a_n * CHAOS_K;
        i += 1;
    }
    a_n
}

/// `const fn` twin of [`chaos_hash`], character-for-character identical.
///
/// A property test asserts the two agree bit-for-bit for every table entry, so a
/// toolchain change that altered const-eval float behaviour fails the build instead
/// of silently redrawing avatars (research D4).
const fn chaos_hash_const(number: f64) -> f64 {
    let mut a_n = 1.0 / number;
    let mut i = 0;
    while i < CHAOS_ITERATIONS {
        a_n = (1.0 - a_n) * a_n * CHAOS_K;
        i += 1;
    }
    a_n
}

const fn build_chaos_table() -> [f64; CHAOS_TABLE_LEN] {
    let mut table = [0.0_f64; CHAOS_TABLE_LEN];
    let mut code_unit = 0;
    while code_unit < CHAOS_TABLE_LEN {
        // Upstream feeds `charCodeAt(0) + 3`.
        table[code_unit] = chaos_hash_const((code_unit + 3) as f64);
        code_unit += 1;
    }
    table
}

/// `chaos_hash(cu + 3)` for every ASCII code unit, resolved at compile time.
///
/// 1 KB of read-only data, no lazy initialisation, no lock, no allocation — which
/// is how this stays fast without introducing the mutable state FR-008 forbids.
pub(crate) static CHAOS_TABLE: [f64; CHAOS_TABLE_LEN] = build_chaos_table();

#[inline]
fn chaos_for_code_unit(code_unit: u32) -> f64 {
    match CHAOS_TABLE.get(code_unit as usize) {
        Some(&v) => v,
        // Non-ASCII: rare enough that computing is cheaper than a 512 KB table.
        None => chaos_hash(f64::from(code_unit + 3)),
    }
}

/// `ch.charCodeAt(0)` for a one-character JavaScript string.
///
/// BMP scalars are their own code unit; astral ones yield their **leading
/// surrogate**. Using `ch as u32` here would diverge on every emoji (research D3).
#[inline]
const fn leading_code_unit(ch: char) -> u32 {
    let cp = ch as u32;
    if cp <= 0xFFFF {
        cp
    } else {
        0xD800 + ((cp - 0x1_0000) >> 10)
    }
}

/// Upper bound on `Number::toString` output for the reachable domain.
///
/// The accumulator stays in `(0, 0.5]` (research D7), so the longest form is
/// `0.` + 5 zeros + 17 digits = 24 bytes; the exponential form is shorter. 32 gives
/// slack, and a property test pins the bound for arbitrary `f64`.
const DECIMAL_CAP: usize = 32;

/// Longest possible hash: `slice(4, 21)` yields at most 17 characters.
const HASH_CAP: usize = 17;

/// The decimal digit string every colour and section choice indexes into.
///
/// 13–17 bytes, carried inline so hashing never allocates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdenticonHash {
    buf: [u8; HASH_CAP],
    len: u8,
}

impl IdenticonHash {
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Every byte written is ASCII, taken from ryu-js output or a pad character.
        std::str::from_utf8(&self.buf[..self.len as usize]).unwrap_or("")
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len as usize]
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.len as usize
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True when the seed landed in a regime where the JS library itself produces a
    /// malformed identicon or throws (research D5).
    #[must_use]
    pub fn is_degenerate(&self) -> bool {
        self.as_bytes().iter().any(|b| !b.is_ascii_digit())
    }

    /// The `reverse -> pad -> replace -> slice -> padEnd` tail of `makeHash`.
    fn from_decimal(decimal: &str) -> Self {
        let bytes = decimal.as_bytes();
        let n = bytes.len().min(DECIMAL_CAP);

        // `.split('').reverse().join('')` — ryu-js emits ASCII only, so bytes reverse.
        let mut rev = [0_u8; DECIMAL_CAP];
        let mut i = 0;
        while i < n {
            rev[i] = bytes[n - 1 - i];
            i += 1;
        }

        // `fullHash.charAt(5) || '0'` — charAt past the end is '', which is falsy.
        let pad = if n > 5 { rev[5] } else { b'0' };

        // `.replace('.', padChar)` replaces the FIRST occurrence only. There is at
        // most one '.', and this is a no-op when pad is itself '.'.
        let mut j = 0;
        while j < n {
            if rev[j] == b'.' {
                rev[j] = pad;
                break;
            }
            j += 1;
        }

        // `.slice(4, 21)` then `.padEnd(13, padChar)`
        let start = n.min(4);
        let end = n.min(21);
        let mut buf = [0_u8; HASH_CAP];
        let mut len = 0;
        let mut k = start;
        while k < end && len < HASH_CAP {
            buf[len] = rev[k];
            len += 1;
            k += 1;
        }
        while len < 13 {
            buf[len] = pad;
            len += 1;
        }

        IdenticonHash {
            buf,
            // len <= HASH_CAP = 17, so the cast cannot truncate.
            len: len as u8,
        }
    }
}

impl std::fmt::Display for IdenticonHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq<str> for IdenticonHash {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

/// `makeHash` — total, allocation-free, and byte-identical for **every** input,
/// including the degenerate regimes.
#[must_use]
pub fn make_hash(seed: &str) -> IdenticonHash {
    let mut acc = 0.5_f64;
    for ch in seed.chars() {
        // Multiply order is contract — see the module docs and research D8.
        acc = acc * (1.0 - acc) * chaos_for_code_unit(leading_code_unit(ch));
    }

    // ECMAScript `Number::toString(10)`, tie-break included (research D2).
    let mut ryu = ryu_js::Buffer::new();
    IdenticonHash::from_decimal(ryu.format(acc))
}

// ---------------------------------------------------------------------------
// Params
// ---------------------------------------------------------------------------

/// Which of the four artwork layers a fragment belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Section {
    Face,
    Sides,
    Top,
    Bottom,
}

impl Section {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Section::Face => "face",
            Section::Sides => "sides",
            Section::Top => "top",
            Section::Bottom => "bottom",
        }
    }

    fn table(self) -> &'static [&'static str; SECTION_COUNT + 1] {
        match self {
            Section::Face => &FACE,
            Section::Sides => &SIDES,
            Section::Top => &TOP,
            Section::Bottom => &BOTTOM,
        }
    }
}

/// The three chosen colours.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Colors {
    pub main: &'static str,
    pub background: &'static str,
    pub accent: &'static str,
}

/// The four chosen artwork fragments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sections {
    pub top: &'static str,
    pub sides: &'static str,
    pub face: &'static str,
    pub bottom: &'static str,
}

/// A complete identicon, independent of how it is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdenticonParams {
    pub colors: Colors,
    pub sections: Sections,
}

/// What to do about seeds the JS library cannot render (research D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SeedPolicy {
    /// Reject. The wallet-safe default: an invisible avatar is worse than an error.
    Strict,
    /// Reproduce the JS library's malformed output byte-for-byte, so the parity
    /// claim holds over the whole input domain and not just its usable part.
    JsCompat,
}

/// `sectionToSvg` — `abs(index % 21) + 1` addressing, matching JS `%` semantics
/// (sign follows the dividend).
pub fn section_svg(section: Section, index: i64) -> Result<&'static str, CoreError> {
    let n = (index % SECTION_COUNT as i64).unsigned_abs() as usize + 1;
    section
        .table()
        .get(n)
        .copied()
        .filter(|svg| !svg.is_empty())
        .ok_or_else(|| {
            CoreError::InvalidIdenticonSeed(format!(
                "no artwork for section {} at index {index} (resolved {n})",
                section.as_str()
            ))
        })
}

/// `sectionsToSvg`.
pub fn sections_svg(face: i64, top: i64, sides: i64, bottom: i64) -> Result<Sections, CoreError> {
    Ok(Sections {
        bottom: section_svg(Section::Bottom, bottom)?,
        top: section_svg(Section::Top, top)?,
        sides: section_svg(Section::Sides, sides)?,
        face: section_svg(Section::Face, face)?,
    })
}

/// `colorsToRgb` — resolves the three palette indices, keeping the colours apart.
///
/// Note the asymmetry, which is upstream behaviour and not a bug to tidy: `main` is
/// nudged **once**, `accent` is nudged in a **loop**.
#[must_use]
pub fn colors_from_indices(main: u8, background: u8, accent: u8) -> Colors {
    resolve_colors(Some(main % 10), background % 10, accent % 10)
}

/// `main = None` models JavaScript's `NaN` index, where every `===` comparison is
/// false and `colors[NaN]` is `undefined`.
fn resolve_colors(main: Option<u8>, background: u8, accent: u8) -> Colors {
    let main = main.map(|m| if m == background { (m + 1) % 10 } else { m });

    let mut accent = accent;
    // At most two values are excluded out of ten, so this terminates; the bound is
    // belt-and-braces against a future edit making it spin.
    for _ in 0..10 {
        if Some(accent) == main || accent == background {
            accent = (accent + 1) % 10;
        } else {
            break;
        }
    }

    Colors {
        main: main.map_or("undefined", |m| COLORS[(m % 10) as usize]),
        background: BACKGROUND_COLORS[(background % 10) as usize],
        accent: COLORS[(accent % 10) as usize],
    }
}

/// A single hash character as a digit, or `None` for JavaScript's `NaN`.
///
/// JS `Number(c)` is a number only for a digit: `Number('e')`, `Number('.')` and
/// `Number('-')` are all `NaN`.
fn digit_at(hash: &[u8], i: usize) -> Option<u8> {
    match hash.get(i) {
        Some(b) if b.is_ascii_digit() => Some(b - b'0'),
        _ => None,
    }
}

/// A two-character slice as a number, or `None` for `NaN`.
///
/// Restricting this to two ASCII digits is very slightly stricter than JS (which
/// would read `"1."` as `1`), but the only hash shapes reachable from `make_hash`
/// put a non-digit in *both* positions, so the two agree everywhere they can be
/// reached — and where they cannot, erring beats inventing an avatar.
fn pair_at(hash: &[u8], i: usize) -> Option<i64> {
    let hi = digit_at(hash, i)?;
    let lo = digit_at(hash, i + 1)?;
    Some(i64::from(hi) * 10 + i64::from(lo))
}

fn params_from_hash(
    hash: &IdenticonHash,
    policy: SeedPolicy,
) -> Result<IdenticonParams, CoreError> {
    let h = hash.as_bytes();
    let unrenderable = |what: &str| {
        CoreError::InvalidIdenticonSeed(format!(
            "hash {} has no {what} (the upstream library {})",
            hash.as_str(),
            match policy {
                SeedPolicy::Strict => "renders it as the literal text \"undefined\" or throws",
                SeedPolicy::JsCompat => "throws",
            }
        ))
    };

    // Section indices. Digit 5 is deliberately read twice — as the low digit of
    // `face` and the high digit of `top`. That is upstream behaviour: giving each
    // section its own digits would change every avatar.
    let face = pair_at(h, 3).ok_or_else(|| unrenderable("face index"))?;
    let top = pair_at(h, 5).ok_or_else(|| unrenderable("top index"))?;
    let sides = pair_at(h, 7).ok_or_else(|| unrenderable("sides index"))?;
    let bottom = pair_at(h, 9).ok_or_else(|| unrenderable("bottom index"))?;

    let background = digit_at(h, 2).ok_or_else(|| unrenderable("background colour"))?;
    let accent = digit_at(h, 11).ok_or_else(|| unrenderable("accent colour"))?;
    let main = digit_at(h, 0);
    if main.is_none() && policy == SeedPolicy::Strict {
        return Err(unrenderable("main colour"));
    }

    Ok(IdenticonParams {
        sections: sections_svg(face, top, sides, bottom)?,
        colors: resolve_colors(main, background, accent),
    })
}

/// `getIdenticonsParams` — rejects seeds the algorithm cannot render.
///
/// This is the wallet-safe entry point. For a seed of ~1,000+ characters the
/// upstream library emits `fill="undefined"`, which draws an invisible avatar; for
/// a far rarer class it throws outright. Both become
/// [`CoreError::InvalidIdenticonSeed`] here (research D5).
pub fn identicon_params(seed: &str) -> Result<IdenticonParams, CoreError> {
    params_from_hash(&make_hash(seed), SeedPolicy::Strict)
}

/// `getIdenticonsParams`, bug-compatible with the JS library.
///
/// Reproduces the malformed output (`main` becomes the literal `"undefined"`)
/// instead of rejecting it, and fails only where the library itself throws. Exists
/// so parity can be proven across the entire input domain rather than only the part
/// that renders correctly; production paths should use [`identicon_params`].
pub fn identicon_params_js_compat(seed: &str) -> Result<IdenticonParams, CoreError> {
    params_from_hash(&make_hash(seed), SeedPolicy::JsCompat)
}

// ---------------------------------------------------------------------------
// Assembly
// ---------------------------------------------------------------------------

const STOCK_HEAD: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 160 160\"><defs><clipPath id=\"a\">";
const STOCK_AFTER_SHAPE: &str = "</clipPath></defs><path fill=\"";
const STOCK_AFTER_BACKGROUND: &str = "\" d=\"M0 0h160v160H0z\" clip-path=\"url(#a)\"/><g fill=\"";
const STOCK_AFTER_ACCENT: &str = "\" clip-path=\"url(#a)\" color=\"";
const SVG_OPEN_GROUP: &str = "\">";
const SVG_TAIL: &str = "</g></svg>";

const CIRCULAR_HEAD: &str =
    "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 160 160\"><path fill=\"";
const CIRCULAR_AFTER_BACKGROUND: &str = "\" d=\"M0 0h160v160H0z\"/><g fill=\"";
const CIRCULAR_AFTER_ACCENT: &str = "\" color=\"";

/// Bytes contributed by the four artwork fragments, the circle and the shadow —
/// the part that varies between identicons.
fn body_len(params: &IdenticonParams) -> usize {
    let s = &params.sections;
    CIRCLE_PREFIX.len()
        + params.colors.main.len()
        + CIRCLE_SUFFIX.len()
        + DEFAULT_SHADOW.len()
        + s.top.len()
        + s.sides.len()
        + s.face.len()
        + s.bottom.len()
}

fn push_body(out: &mut String, params: &IdenticonParams) {
    out.push_str(CIRCLE_PREFIX);
    out.push_str(params.colors.main);
    out.push_str(CIRCLE_SUFFIX);
    out.push_str(DEFAULT_SHADOW);
    out.push_str(params.sections.top);
    out.push_str(params.sections.sides);
    out.push_str(params.sections.face);
    out.push_str(params.sections.bottom);
}

/// `assembleSvg` — the library's stock hexagonal output.
///
/// Note the hardcoded `clipPath id="a"`: when several of these share one DOM,
/// `url(#a)` resolves document-wide to the first `#a`, so the clip silently breaks.
/// That is why the wallet renders [`assemble_svg_circular`] instead.
#[must_use]
pub fn assemble_svg(params: &IdenticonParams) -> String {
    let c = &params.colors;
    let capacity = STOCK_HEAD.len()
        + DEFAULT_BACKGROUND_SHAPE.len()
        + STOCK_AFTER_SHAPE.len()
        + c.background.len()
        + STOCK_AFTER_BACKGROUND.len()
        + c.accent.len()
        + STOCK_AFTER_ACCENT.len()
        + c.main.len()
        + SVG_OPEN_GROUP.len()
        + body_len(params)
        + SVG_TAIL.len();

    let mut out = String::with_capacity(capacity);
    out.push_str(STOCK_HEAD);
    out.push_str(DEFAULT_BACKGROUND_SHAPE);
    out.push_str(STOCK_AFTER_SHAPE);
    out.push_str(c.background);
    out.push_str(STOCK_AFTER_BACKGROUND);
    out.push_str(c.accent);
    out.push_str(STOCK_AFTER_ACCENT);
    out.push_str(c.main);
    out.push_str(SVG_OPEN_GROUP);
    push_body(&mut out, params);
    out.push_str(SVG_TAIL);
    debug_assert_eq!(
        out.len(),
        capacity,
        "capacity must be exact — one allocation"
    );
    out
}

/// The wallet's circular variant — what every Vela platform renders.
///
/// Two differences from [`assemble_svg`], both deliberate: the hexagonal clip is
/// dropped (the app clips to a circle in its view layer, so one shape language
/// holds across all avatars), and no SVG `id` is emitted, so many instances can
/// share a DOM safely.
#[must_use]
pub fn assemble_svg_circular(params: &IdenticonParams) -> String {
    let c = &params.colors;
    let capacity = CIRCULAR_HEAD.len()
        + c.background.len()
        + CIRCULAR_AFTER_BACKGROUND.len()
        + c.accent.len()
        + CIRCULAR_AFTER_ACCENT.len()
        + c.main.len()
        + SVG_OPEN_GROUP.len()
        + body_len(params)
        + SVG_TAIL.len();

    let mut out = String::with_capacity(capacity);
    out.push_str(CIRCULAR_HEAD);
    out.push_str(c.background);
    out.push_str(CIRCULAR_AFTER_BACKGROUND);
    out.push_str(c.accent);
    out.push_str(CIRCULAR_AFTER_ACCENT);
    out.push_str(c.main);
    out.push_str(SVG_OPEN_GROUP);
    push_body(&mut out, params);
    out.push_str(SVG_TAIL);
    debug_assert_eq!(
        out.len(),
        capacity,
        "capacity must be exact — one allocation"
    );
    out
}

/// Output encoding for [`format_identicon`] and [`create_identicon`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum IdenticonFormat {
    /// Raw SVG source. JS: `'svg'`.
    #[default]
    Svg,
    /// `data:image/svg+xml;base64,…`. JS: `'image/svg+xml'` (the JS default).
    DataUri,
}

const DATA_URI_PREFIX: &str = "data:image/svg+xml;base64,";

/// `formatIdenticon`.
#[must_use]
pub fn format_identicon(svg: &str, format: IdenticonFormat) -> String {
    match format {
        IdenticonFormat::Svg => svg.to_owned(),
        IdenticonFormat::DataUri => {
            // `btoa` produces standard, padded base64; the SVG is ASCII, so byte
            // semantics match Latin-1 exactly.
            let encoded = base64::engine::general_purpose::STANDARD.encode(svg.as_bytes());
            let mut out = String::with_capacity(DATA_URI_PREFIX.len() + encoded.len());
            out.push_str(DATA_URI_PREFIX);
            out.push_str(&encoded);
            out
        }
    }
}

// ---------------------------------------------------------------------------
// One-shot helpers
// ---------------------------------------------------------------------------

/// Stock hexagonal identicon for a seed.
pub fn identicon_svg(seed: &str) -> Result<String, CoreError> {
    Ok(assemble_svg(&identicon_params(seed)?))
}

/// **The wallet's identicon.** Circular variant, no SVG ids.
pub fn identicon_svg_circular(seed: &str) -> Result<String, CoreError> {
    Ok(assemble_svg_circular(&identicon_params(seed)?))
}

/// Stock identicon as a base64 data URI.
pub fn identicon_data_uri(seed: &str) -> Result<String, CoreError> {
    Ok(format_identicon(
        &assemble_svg(&identicon_params(seed)?),
        IdenticonFormat::DataUri,
    ))
}

// ---------------------------------------------------------------------------
// Seed normalisation (Vela policy — not part of identicons-esm)
// ---------------------------------------------------------------------------

/// Truncate to at most `max` UTF-16 code units, on a character boundary.
///
/// A surrogate pair straddling the limit is dropped whole. JavaScript's
/// `slice(0, 128)` would keep a lone high surrogate there, which Rust `&str`
/// cannot represent — the single remaining divergence, and one that needs a
/// >128-unit seed with an astral character at exactly the boundary.
fn truncate_utf16(s: &str, max: usize) -> &str {
    let mut units = 0_usize;
    for (i, ch) in s.char_indices() {
        let width = ch.len_utf16();
        if units + width > max {
            return s.get(..i).unwrap_or(s);
        }
        units += width;
    }
    s
}

/// Letters Unicode 17 gives no lowercase and std's tables (Unicode 18 since
/// Rust 1.100) do. [`normalize_seed`] leaves them as they are.
///
/// Every avatar drawn so far lowercased its seed as Unicode 17 does: the
/// app's original `seed.toLowerCase()` (V8 in Node 22.22.3 reports Unicode
/// 17.0, and `scripts/verify-identicon-parity.mjs` checks the core against
/// it), and this function on Rust 1.97.1, whose std is Unicode 17.0. A seed's
/// avatar must not change because the compiler's Unicode tables did, so the
/// contract is Unicode 17's lowercasing, frozen.
///
/// The list is the UCD's own difference: UnicodeData.txt field 13 and the
/// unconditional SpecialCasing.txt mappings, 17.0.0 against 18.0.0, give
/// exactly these 20; diffing `char::to_lowercase` over every code point
/// between 1.97.1 and 1.100.0 gives the same 20.
/// `lowercase_map_is_unicode_17s` fails when a toolchain moves std again.
///
/// Not frozen: whether a `Σ` is word-final (`ς`) looks at its neighbours'
/// Cased and Case_Ignorable properties, and letters Unicode 18 assigned are
/// cased in std now. A `Σ` followed by one of them lowercases to `σ` where
/// V8 gives `ς`.
const NO_LOWERCASE_IN_UNICODE_17: [char; 20] = [
    '\u{A7DD}',
    '\u{A7E2}',
    '\u{AB6C}',
    '\u{AB6D}',
    '\u{1DF40}',
    '\u{1DF48}',
    '\u{1DF4A}',
    '\u{1DF4D}',
    '\u{1DF51}',
    '\u{1DF68}',
    '\u{1DF6A}',
    '\u{1DF6C}',
    '\u{1DF6E}',
    '\u{1DF72}',
    '\u{1DF74}',
    '\u{1DF76}',
    '\u{1DF78}',
    '\u{1DF7A}',
    '\u{1DF7C}',
    '\u{1DF7E}',
];

/// `str::to_lowercase`, which applies the final-sigma rule (V8 does too, so
/// it is not done char by char), with [`NO_LOWERCASE_IN_UNICODE_17`] left as
/// they are. Unicode 17 assigns none of them, so to its final-sigma rule each
/// is neither cased nor case-ignorable, a word boundary: lowercasing the text
/// between them separately is that rule exactly.
fn lowercase_unicode_17(seed: &str) -> String {
    if !seed.contains(&NO_LOWERCASE_IN_UNICODE_17[..]) {
        return seed.to_lowercase();
    }
    let mut out = String::with_capacity(seed.len());
    for piece in seed.split_inclusive(&NO_LOWERCASE_IN_UNICODE_17[..]) {
        match piece.char_indices().next_back() {
            Some((at, kept)) if NO_LOWERCASE_IN_UNICODE_17.contains(&kept) => {
                out.push_str(&piece[..at].to_lowercase());
                out.push(kept);
            }
            _ => out.push_str(&piece.to_lowercase()),
        }
    }
    out
}

/// Case- and length-normalises a seed so all four platforms agree.
///
/// Equivalent to the app's long-standing `seed.toLowerCase().slice(0, 128)` —
/// **full Unicode lowercasing** (verified against V8 for all 1,112,064 code points
/// and for string-level context rules such as Greek final sigma), then truncation
/// to [`SEED_MAX_UTF16_LEN`] UTF-16 code units. The order matters: lowercasing can
/// change length (`U+0130` expands to two code points), and JavaScript lowercases
/// first.
///
/// Borrows when the seed is already normalised — the common case, since stored
/// addresses are lowercase ASCII — so the hot path allocates nothing.
///
/// Every platform must route through this. Lowercasing at the call site is exactly
/// how four platforms drift into drawing four different avatars.
///
/// The lowercasing is Unicode 17's, whatever the compiler's: see
/// [`NO_LOWERCASE_IN_UNICODE_17`].
#[must_use]
pub fn normalize_seed(seed: &str) -> Cow<'_, str> {
    // Fast path: ASCII lowercasing is 1:1 on length, so truncation can be checked
    // without allocating, and an already-normalised seed is returned borrowed.
    if seed.is_ascii() {
        let truncated = truncate_utf16(seed, SEED_MAX_UTF16_LEN);
        if !truncated.bytes().any(|b| b.is_ascii_uppercase()) {
            return Cow::Borrowed(truncated);
        }
        return Cow::Owned(truncated.to_ascii_lowercase());
    }
    let lowered = lowercase_unicode_17(seed);
    let truncated = truncate_utf16(&lowered, SEED_MAX_UTF16_LEN);
    if truncated.len() == lowered.len() {
        Cow::Owned(lowered)
    } else {
        Cow::Owned(truncated.to_owned())
    }
}

// ---------------------------------------------------------------------------
// Nimiq address compatibility (createIdenticon's validate-and-normalise path)
// ---------------------------------------------------------------------------

/// Nimiq's base-32 alphabet — note the absent `I`, `O`, `W` and `Z`.
const NIMIQ_ALPHABET: &[u8] = b"0123456789ABCDEFGHJKLMNPQRSTUVXY";

/// `ValidationUtils.isValidAddress`.
///
/// Ported for completeness of the upstream contract. Vela never enables it: Vela
/// addresses are EVM addresses, which fail this, so validating would return the
/// placeholder for every account (research D11).
#[must_use]
pub fn nimiq_is_valid_address(address: &str) -> bool {
    if address.is_empty() {
        return false;
    }
    let stripped: String = address.chars().filter(|c| *c != ' ').collect();
    if stripped.len() != 36 || !stripped.is_char_boundary(2) {
        return false;
    }
    if !stripped
        .get(..2)
        .is_some_and(|p| p.eq_ignore_ascii_case("NQ"))
    {
        return false;
    }
    if !stripped
        .bytes()
        .all(|b| NIMIQ_ALPHABET.contains(&b.to_ascii_uppercase()))
    {
        return false;
    }
    let (head, tail) = stripped.split_at(4);
    nimiq_iban_check(&format!("{tail}{head}")) == Some(1)
}

/// The mod-97 IBAN checksum, chunked exactly as the upstream is.
fn nimiq_iban_check(s: &str) -> Option<u32> {
    let mut digits = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        let up = c.to_ascii_uppercase();
        if up.is_ascii_digit() {
            digits.push(up);
        } else {
            // Upstream: `(code - 55).toString()`. Reachable only after the alphabet
            // check, so this is always a two-digit value in 10..=35.
            digits.push_str(&(u32::from(up as u8).wrapping_sub(55)).to_string());
        }
    }

    let mut tmp = String::new();
    let mut i = 0;
    while i < digits.len() {
        let end = (i + 6).min(digits.len());
        let chunk = digits.get(i..end)?;
        let value: u64 = format!("{tmp}{chunk}").parse().ok()?;
        tmp = (value % 97).to_string();
        i += 6;
    }
    tmp.parse().ok()
}

/// `validateInput` — validates, uppercases, and regroups into blocks of four.
#[must_use]
pub fn nimiq_format_address(address: &str) -> Option<String> {
    if !nimiq_is_valid_address(address) {
        return None;
    }
    let cleaned: String = address
        .chars()
        .filter(|c| *c != '+' && *c != ' ')
        .map(|c| c.to_ascii_uppercase())
        .collect();

    // `match(/.{4}/g)` drops a trailing partial chunk; a valid address is 36 chars,
    // so this always yields exactly nine full chunks.
    let bytes = cleaned.as_bytes();
    let mut out = String::with_capacity(cleaned.len() + cleaned.len() / 4);
    let mut i = 0;
    while i + 4 <= bytes.len() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(cleaned.get(i..i + 4)?);
        i += 4;
    }
    Some(out)
}

/// Options for [`create_identicon`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CreateOptions {
    /// JS: `shouldValidateAddress`. **Defaults to `false` here, where JS defaults to
    /// `true`** — Nimiq validation rejects every EVM address, so the JS default would
    /// return the placeholder for every Vela account. Documented divergence #3.
    pub validate_address: bool,
    /// JS: `format`. Defaults to raw SVG here; JS defaults to the data URI.
    pub format: IdenticonFormat,
}

impl Default for CreateOptions {
    fn default() -> Self {
        CreateOptions {
            validate_address: false,
            format: IdenticonFormat::Svg,
        }
    }
}

/// `createIdenticon` — the full upstream contract, placeholder short-circuits included.
///
/// Two upstream quirks are reproduced rather than tidied: the falsy-input check runs
/// *after* validation is skipped, so an **empty seed returns the placeholder even
/// with `validate_address: false`**; and it returns the placeholder **un-encoded**
/// even when a data URI was requested, because the short-circuit precedes formatting.
pub fn create_identicon(raw_seed: &str, options: CreateOptions) -> Result<String, CoreError> {
    let input: Cow<'_, str> = if options.validate_address {
        match nimiq_format_address(raw_seed) {
            Some(formatted) => Cow::Owned(formatted),
            None => return Ok(IDENTICON_PLACEHOLDER.to_owned()),
        }
    } else {
        Cow::Borrowed(raw_seed)
    };

    // JS `if (!input)` — an empty string is falsy.
    if input.is_empty() {
        return Ok(IDENTICON_PLACEHOLDER.to_owned());
    }

    let svg = assemble_svg(&identicon_params(&input)?);
    Ok(format_identicon(&svg, options.format))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seeds lowercase as Unicode 17 (and V8 in Node 22.22.3) lowercase them,
    /// whatever std's tables say. Expected values are Node 22.22.3's
    /// `toLowerCase()`.
    #[test]
    fn seeds_lowercase_as_unicode_17() {
        for &kept in &NO_LOWERCASE_IN_UNICODE_17 {
            assert_ne!(
                kept.to_lowercase().to_string(),
                kept.to_string(),
                "{kept:?} has a std lowercase"
            );
            assert_eq!(normalize_seed(&kept.to_string()), kept.to_string());
        }
        assert_eq!(normalize_seed("A\u{3a3}\u{A7DD}"), "a\u{3c2}\u{A7DD}");
        assert_eq!(
            normalize_seed("\u{A7DD}\u{391}\u{3a3}"),
            "\u{A7DD}\u{3b1}\u{3c2}"
        );
        assert_eq!(
            normalize_seed("\u{C0}B\u{1DF4D}C\u{3a3}"),
            "\u{e0}b\u{1DF4D}c\u{3c2}"
        );
        assert_eq!(normalize_seed("\u{C0}B\u{3a3}"), "\u{e0}b\u{3c2}");
    }

    /// The lowercase of every `char`, as [`normalize_seed`] sees it (std's
    /// `char::to_lowercase`, [`NO_LOWERCASE_IN_UNICODE_17`] kept), is Unicode
    /// 17's: FNV-1a 64 over every code point and its lowercase equals the
    /// digest of 1.97.1's raw std (Unicode 17.0), `0xc8aa_d735_7475_6d8c`.
    /// On 1.100.0's raw std it is `0x48ba_2236_e0fd_3566`; the 20 kept are the
    /// whole difference.
    #[test]
    fn lowercase_map_is_unicode_17s() {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |value: u32| {
            for byte in value.to_le_bytes() {
                hash ^= u64::from(byte);
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            }
        };
        for ch in (0..=0x10_FFFF_u32).filter_map(char::from_u32) {
            feed(u32::from(ch));
            if NO_LOWERCASE_IN_UNICODE_17.contains(&ch) {
                feed(u32::from(ch));
            } else {
                ch.to_lowercase().for_each(|lower| feed(u32::from(lower)));
            }
            feed(u32::MAX);
        }
        assert_eq!(
            hash,
            0xc8aa_d735_7475_6d8c,
            "std's lowercase map (Unicode {:?}) is no longer Unicode 17's once \
             NO_LOWERCASE_IN_UNICODE_17 is kept: the toolchain moved std's Unicode \
             tables. Diff `char::to_lowercase` over every code point (or the UCD's \
             UnicodeData.txt field 13 + SpecialCasing.txt, 17.0.0 against std's \
             version) and add every code point that gained or changed a lowercase \
             to NO_LOWERCASE_IN_UNICODE_17, or seeds' avatars change.",
            char::UNICODE_VERSION
        );
    }

    /// The load-bearing invariant behind [`CHAOS_TABLE`]: const-eval and runtime
    /// float evaluation must agree bit-for-bit (research D4).
    ///
    /// If a toolchain upgrade ever broke this, every user's avatar would silently
    /// change. Failing the build is the only acceptable outcome, so this is a unit
    /// test rather than something a reviewer has to remember to check.
    #[test]
    fn const_chaos_table_matches_runtime_loop() {
        for (code_unit, &precomputed) in CHAOS_TABLE.iter().enumerate() {
            #[allow(clippy::cast_precision_loss)]
            let runtime = chaos_hash((code_unit + 3) as f64);
            assert_eq!(
                precomputed.to_bits(),
                runtime.to_bits(),
                "chaos table entry {code_unit} diverged from the runtime loop — \
                 const-eval float determinism no longer holds, do NOT ship"
            );
        }
    }

    /// The reachable value domain (research D7). Every chaos value is strictly
    /// inside `(0, 1)`, which is what proves the accumulator strictly decreases and
    /// therefore that the hash never exceeds [`HASH_CAP`] bytes.
    #[test]
    fn chaos_stays_inside_unit_interval_for_every_code_unit() {
        let mut max = f64::MIN;
        for code_unit in 0_u32..=0xFFFF {
            let v = chaos_for_code_unit(code_unit);
            assert!(
                v > 0.0 && v < 1.0,
                "chaos({code_unit}) = {v} escaped (0, 1); the accumulator would stop decreasing"
            );
            if v > max {
                max = v;
            }
        }
        assert!(max < 1.0, "max chaos value {max} must stay below 1.0");
    }

    /// The accumulator's bound is what sizes [`HASH_CAP`]; check it directly over a
    /// spread of seed shapes rather than trusting the algebra alone.
    #[test]
    fn hash_length_stays_within_capacity() {
        let seeds = [
            String::new(),
            "a".to_owned(),
            "0xd8da6bf26964af9d7eed9e03e53415d37aa96045".to_owned(),
            "x".repeat(93),
            "x".repeat(500),
            "x".repeat(2000),
            "\u{1F48E}\u{1F389}".to_owned(),
        ];
        for seed in seeds {
            let hash = make_hash(&seed);
            assert!(
                (13..=HASH_CAP).contains(&hash.len()),
                "hash for a {}-char seed was {} bytes",
                seed.chars().count(),
                hash.len()
            );
        }
    }

    /// The library's own published snapshots — the fastest signal that the float
    /// pipeline is intact.
    #[test]
    fn known_answer_hashes() {
        assert_eq!(make_hash("test").as_str(), "39522148458090");
        assert_eq!(make_hash("hello").as_str(), "7935187296325090");
        assert_eq!(
            make_hash("NQ07 0000 0000 0000 0000 0000 0000 0000 0000").as_str(),
            "113682528368518"
        );
    }

    /// Regime A: strict rejects, JS-compat reproduces the malformed output.
    #[test]
    fn degenerate_seed_policies_differ_as_specified() {
        let seed = "x".repeat(2000);
        assert!(make_hash(&seed).is_degenerate());

        let strict = identicon_params(&seed);
        assert!(strict.is_err(), "strict mode must reject a Regime A seed");

        let compat = identicon_params_js_compat(&seed);
        assert!(
            compat.is_ok(),
            "js-compat must render a Regime A seed, got {compat:?}"
        );
        if let Ok(compat) = compat {
            assert_eq!(compat.colors.main, "undefined");
            assert!(assemble_svg(&compat).contains("undefined"));
        }
    }

    /// The module must stay stateless (FR-008): no cache, no lazy init.
    #[test]
    fn repeated_generation_is_stable() {
        let seed = "0xd8da6bf26964af9d7eed9e03e53415d37aa96045";
        let first = identicon_svg_circular(seed);
        assert!(first.is_ok(), "a 42-character address must render");
        for _ in 0..100 {
            assert_eq!(identicon_svg_circular(seed), first);
        }
    }
}
