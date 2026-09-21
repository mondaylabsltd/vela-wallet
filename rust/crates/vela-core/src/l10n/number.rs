//! Number formatting: presets, grouping, compact tiers, and parsing.
//!
//! Byte-identical to `src/services/locale-format.ts` (FR-019). That file is
//! deliberately **preset-based rather than `Intl`-based**, because Hermes ships
//! incomplete ICU — the same reason this crate carries its own CLDR plural rules.
//! The presets are a product decision too: the user picks a grouping style in
//! Settings, so CLDR does not get to override it (see `l10n::currency`).

/// JS `Number.prototype.toFixed` on a **non-negative** value.
///
/// This exists because Rust's `{:.N}` formatter and JS's `toFixed` disagree at
/// exact ties, and the disagreement is user-visible on every zero-decimal currency:
///
/// | value | JS `toFixed(0)` | Rust `{:.0}` |
/// |---|---|---|
/// | `1234.5` | **1235** | 1234 |
/// | `0.5` | **1** | 0 |
/// | `2.5` | **3** | 2 |
///
/// ECMA-262 picks the integer `n` minimising `|n / 10^f - x|` and, on a tie, the
/// **larger** `n` — ties away from zero, since the sign is split off first. Rust's
/// formatter rounds ties to **even**. `f64::round` happens to have exactly JS's
/// tie rule, so scaling and rounding reproduces it.
///
/// Above 2^53 the scaled product is no longer exact, so the formatter takes over —
/// no wallet amount reaches that, and ties are not representable up there anyway.
fn to_fixed(abs: f64, digits: usize) -> String {
    debug_assert!(abs >= 0.0);
    let scale = 10f64.powi(i32::try_from(digits).unwrap_or(0));
    let scaled = abs * scale;
    if !scaled.is_finite() || scaled >= 9_007_199_254_740_992.0 {
        return format!("{abs:.digits$}");
    }
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::allow_attributes
    )]
    let n = scaled.round() as u64;
    if digits == 0 {
        return n.to_string();
    }
    let mut s = n.to_string();
    if s.len() <= digits {
        // 0.05 at 2 digits is `5`, which must become `0.05` and not `.5`.
        s = format!("{}{}", "0".repeat(digits + 1 - s.len()), s);
    }
    let split = s.len() - digits;
    format!("{}.{}", &s[..split], &s[split..])
}

/// The four number presets a user can pick.
///
/// `auto` is resolved by the host before it reaches the core — detecting it needs
/// `Intl.NumberFormat.formatToParts`, which FR-006 forbids the core from consulting.
///
/// On the wire (spec 069, where a machine first takes one) it is the shells'
/// own preference key without `auto`: `comma_dot`, `dot_comma`, `space_comma`,
/// `indian` — the names every shell already stores, so none needs a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(ts_rs::TS))]
pub enum NumberPreset {
    /// `1,234,567.89`
    #[default]
    CommaDot,
    /// `1.234.567,89`
    DotComma,
    /// `1 234 567,89` — the group separator is a **plain ASCII U+0020**, not NBSP
    /// and not U+202F. CLDR would use U+202F for `fr`; the preset deliberately does
    /// not, because the user chose "space", not "French".
    SpaceComma,
    /// `1,23,45,678.90` — last three digits, then twos.
    Indian,
}

/// The separators a preset writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Separators {
    pub group: &'static str,
    pub decimal: &'static str,
}

impl NumberPreset {
    #[must_use]
    pub fn separators(self) -> Separators {
        match self {
            NumberPreset::CommaDot | NumberPreset::Indian => Separators {
                group: ",",
                decimal: ".",
            },
            NumberPreset::DotComma => Separators {
                group: ".",
                decimal: ",",
            },
            NumberPreset::SpaceComma => Separators {
                group: " ",
                decimal: ",",
            },
        }
    }

    /// Separators for seeding an **editable** numeric input: the decimal mark, but
    /// no grouping — thousands separators must not jump around while typing.
    #[must_use]
    pub fn input_separators(self) -> Separators {
        Separators {
            group: "",
            decimal: self.separators().decimal,
        }
    }

    const fn indian(self) -> bool {
        matches!(self, NumberPreset::Indian)
    }
}

/// Group an integer **digit string**, never an `f64`.
///
/// String-in, string-out is the point: a uint256 base-unit value must never route
/// through a float on a wallet's display path, and this is the entry point the
/// token formatters use.
#[must_use]
pub fn group_digits(digits: &str, preset: NumberPreset) -> String {
    let sep = preset.separators().group;
    let n = digits.len();
    if n <= 3 {
        return digits.to_owned();
    }
    if !preset.indian() {
        let mut out = String::with_capacity(n + n / 3);
        let lead = n % 3;
        if lead > 0 {
            out.push_str(&digits[..lead]);
        }
        let mut i = lead;
        while i + 3 <= n {
            if !out.is_empty() {
                out.push_str(sep);
            }
            out.push_str(&digits[i..i + 3]);
            i += 3;
        }
        return out;
    }
    // Indian: the last three digits, then groups of two.
    let (head, tail) = digits.split_at(n - 3);
    let hn = head.len();
    let mut out = String::with_capacity(n + n / 2);
    let lead = hn % 2;
    if lead > 0 {
        out.push_str(&head[..lead]);
    }
    let mut i = lead;
    while i + 2 <= hn {
        if !out.is_empty() {
            out.push_str(sep);
        }
        out.push_str(&head[i..i + 2]);
        i += 2;
    }
    if !out.is_empty() {
        out.push_str(sep);
    }
    out.push_str(tail);
    out
}

/// Fraction-digit bounds for [`format_number`].
#[derive(Debug, Clone, Copy)]
pub struct FractionDigits {
    pub min: usize,
    pub max: usize,
}

impl Default for FractionDigits {
    fn default() -> Self {
        Self { min: 0, max: 2 }
    }
}

/// Format `value` with the preset's separators and grouping.
///
/// Reproduces `locale-format.ts:170-186` exactly, including two behaviours that
/// look like bugs and are contract: a non-finite value renders `"0"` rather than
/// `"NaN"`/`"inf"`, and `min` is **clamped to** `max` rather than rejected.
#[must_use]
pub fn format_number(value: f64, preset: NumberPreset, digits: FractionDigits) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let sep = preset.separators();
    let max = digits.max;
    let min = digits.min.min(max);

    let sign = if value < 0.0 { "-" } else { "" };
    let fixed = to_fixed(value.abs(), max);

    let (int_part, frac_part) = match fixed.split_once('.') {
        Some((i, f)) => (i.to_owned(), f.to_owned()),
        None => (fixed, String::new()),
    };
    let mut frac = frac_part;
    // Trim trailing zeros DOWN TO min — not below it.
    while frac.len() > min && frac.ends_with('0') {
        frac.pop();
    }

    let grouped = group_digits(&int_part, preset);
    if frac.is_empty() {
        format!("{sign}{grouped}")
    } else {
        format!("{sign}{grouped}{}{frac}", sep.decimal)
    }
}

/// Compact-notation tiers. The suffixes are **deliberately unlocalised** Latin
/// letters: `K`/`M`/`B`/`T` read universally in a wallet and do not depend on host
/// ICU data, whereas CJK myriad groupings would.
const COMPACT_TIERS: [(f64, &str); 4] = [(1e12, "T"), (1e9, "B"), (1e6, "M"), (1e3, "K")];

/// Abbreviated form for large magnitudes — `1234567.89` → `1.23M`.
#[must_use]
pub fn format_compact(value: f64, preset: NumberPreset) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    let sign = if value < 0.0 { "-" } else { "" };
    let abs = value.abs();
    for (threshold, suffix) in COMPACT_TIERS {
        if abs >= threshold {
            let scaled = abs / threshold;
            // 1.23 / 12.3 / 123 — two, one, then zero fraction digits.
            let max = if scaled < 10.0 {
                2
            } else if scaled < 100.0 {
                1
            } else {
                0
            };
            let body = format_number(scaled, preset, FractionDigits { min: 0, max });
            return format!("{sign}{body}{suffix}");
        }
    }
    // Below 1000 there is nothing to abbreviate.
    let max = if abs < 1.0 { 4 } else { 2 };
    format_number(value, preset, FractionDigits { min: 0, max })
}

/// Token-amount precision ladder: `>= 1000` → 2 digits, `>= 1` → 4, `< 1` → 6.
///
/// `compact` is the glanceable-surface variant. Its dust rule is the subtle part:
/// a value that would round to `"0"` at 4 digits falls back to 6, so a tiny
/// non-zero balance never renders as a bare `0` in a feed.
#[must_use]
pub fn format_token_amount(value: f64, preset: NumberPreset, compact: bool) -> String {
    if !value.is_finite() || value == 0.0 {
        return "0".to_owned();
    }
    let abs = value.abs();
    if compact && abs >= 1e6 {
        return format_compact(value, preset);
    }
    if abs >= 1000.0 {
        return format_number(value, preset, FractionDigits { min: 2, max: 2 });
    }
    if abs >= 1.0 {
        return format_number(value, preset, FractionDigits { min: 0, max: 4 });
    }
    if !compact {
        return format_number(value, preset, FractionDigits { min: 0, max: 6 });
    }
    let capped = format_number(value, preset, FractionDigits { min: 0, max: 4 });
    if capped == "0" {
        format_number(value, preset, FractionDigits { min: 0, max: 6 })
    } else {
        capped
    }
}

/// Normalise a user-typed, locale-formatted amount into a **canonical** numeric
/// string — ASCII digits, `.` decimal, no grouping — that a bigint parser can take.
///
/// Order matters and is copied from `locale-format.ts:144-154`: trim, map
/// Arabic-Indic digits, strip grouping, then map the decimal mark. Mapping the
/// decimal mark first would destroy `dot_comma`'s grouping dots.
#[must_use]
pub fn parse_locale_number(text: &str, preset: NumberPreset) -> String {
    let mut s = String::with_capacity(text.len());
    for c in text.trim().chars() {
        // Arabic-Indic (U+0660..U+0669) and Extended Arabic-Indic (U+06F0..U+06F9).
        let mapped = match c as u32 {
            n @ 0x0660..=0x0669 => char::from_digit(n - 0x0660, 10),
            n @ 0x06F0..=0x06F9 => char::from_digit(n - 0x06F0, 10),
            _ => None,
        };
        s.push(mapped.unwrap_or(c));
    }

    let sep = preset.separators();
    let stripped: String = if !sep.group.is_empty() && sep.group.chars().all(char::is_whitespace) {
        // Space grouping: strip EVERY whitespace kind, so an NBSP or U+202F pasted
        // from another app still parses. The preset writes U+0020, but the user did
        // not necessarily type it.
        s.chars().filter(|c| !c.is_whitespace()).collect()
    } else if sep.group.is_empty() {
        s
    } else {
        s.replace(sep.group, "")
    };

    if sep.decimal == "." {
        stripped
    } else {
        stripped.replace(sep.decimal, ".")
    }
}
