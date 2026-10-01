//! SC-010: the formatting layer reproduces `src/services/locale-format.ts`
//! byte-identically, and corrects the currency defects it never handled.
//!
//! These are Rust-only unit tests, not a conformance suite. **Noted for the
//! reviewer**: unlike the 18,975 i18n cases, they are *not* replayed on Kotlin,
//! Swift or wasm — no l10n vector file is registered and `REQUIRED_SUITES` is still
//! 11. If SC-002-style cross-platform byte-identity is wanted here too, that needs a
//! fifth vector file and a 12th suite entry (quickstart open question 2).

use vela_core::i18n::{Catalog, I18n, Options};
use vela_core::l10n::{
    format_compact, format_date, format_date_time, format_fiat, format_number, format_time,
    format_token_amount, group_digits, parse_locale_number, Civil, DatePreset, FiatOptions,
    FractionDigits, NumberPreset, TimePreset,
};

const P: [(NumberPreset, &str); 4] = [
    (NumberPreset::CommaDot, "comma_dot"),
    (NumberPreset::DotComma, "dot_comma"),
    (NumberPreset::SpaceComma, "space_comma"),
    (NumberPreset::Indian, "indian"),
];

/// The exact strings `numberFormatOptions()` shows in the Settings picker
/// (`locale-format.ts:236`), which is the app's own documented sample.
#[test]
fn number_preset_samples_match_the_settings_picker() {
    let expected = [
        (NumberPreset::CommaDot, "1,234,567.89"),
        (NumberPreset::DotComma, "1.234.567,89"),
        (NumberPreset::SpaceComma, "1 234 567,89"),
        (NumberPreset::Indian, "12,34,567.89"),
    ];
    for (preset, want) in expected {
        let got = format_number(1_234_567.89, preset, FractionDigits { min: 2, max: 2 });
        assert_eq!(got, want, "{preset:?}");
    }
}

#[test]
fn space_comma_uses_plain_ascii_space_not_nbsp() {
    // CLDR would use U+202F for `fr` and U+00A0 for `ru`. The preset deliberately
    // does neither: the user chose "space", not a locale.
    let got = format_number(
        1_234_567.89,
        NumberPreset::SpaceComma,
        FractionDigits { min: 2, max: 2 },
    );
    assert!(got.contains('\u{0020}'), "must be U+0020");
    assert!(
        !got.contains('\u{00A0}') && !got.contains('\u{202F}'),
        "no NBSP, no narrow NBSP"
    );
}

#[test]
fn indian_grouping_is_last_three_then_twos() {
    assert_eq!(
        group_digits("12345678", NumberPreset::Indian),
        "1,23,45,678"
    );
    assert_eq!(group_digits("1234567", NumberPreset::Indian), "12,34,567");
    assert_eq!(group_digits("1000", NumberPreset::Indian), "1,000");
    assert_eq!(group_digits("999", NumberPreset::Indian), "999");
    // Western grouping, for contrast.
    assert_eq!(
        group_digits("12345678", NumberPreset::CommaDot),
        "12,345,678"
    );
    assert_eq!(group_digits("1000", NumberPreset::DotComma), "1.000");
}

#[test]
fn group_digits_never_routes_through_a_float() {
    // A uint256 base-unit value, far beyond f64's exact integer range. Routing this
    // through a float would silently corrupt the low digits on a wallet's display.
    let huge = "115792089237316195423570985008687907853269984665640564039457584007913129639935";
    let grouped = group_digits(huge, NumberPreset::CommaDot);
    assert_eq!(grouped.replace(',', ""), huge, "no digit may change");
    assert_eq!(&grouped[grouped.len() - 4..], ",935");
}

#[test]
fn trailing_zeros_trim_down_to_min_and_no_further() {
    let p = NumberPreset::CommaDot;
    assert_eq!(
        format_number(1.5, p, FractionDigits { min: 0, max: 4 }),
        "1.5"
    );
    assert_eq!(
        format_number(1.5, p, FractionDigits { min: 2, max: 4 }),
        "1.50"
    );
    assert_eq!(
        format_number(2.0, p, FractionDigits { min: 0, max: 2 }),
        "2"
    );
    assert_eq!(
        format_number(2.0, p, FractionDigits { min: 2, max: 2 }),
        "2.00"
    );
    // min is CLAMPED to max, not rejected (`locale-format.ts:174`).
    assert_eq!(
        format_number(1.239, p, FractionDigits { min: 6, max: 2 }),
        "1.24"
    );
}

#[test]
fn ties_round_away_from_zero_like_js_tofixed() {
    // Rust's own `{:.N}` rounds ties to EVEN and would give "1,234" / "0" / "2".
    // JS `toFixed` rounds ties away from zero, and the app's output is JS's.
    let p = NumberPreset::CommaDot;
    assert_eq!(
        format_number(1234.5, p, FractionDigits { min: 0, max: 0 }),
        "1,235"
    );
    assert_eq!(
        format_number(0.5, p, FractionDigits { min: 0, max: 0 }),
        "1"
    );
    assert_eq!(
        format_number(2.5, p, FractionDigits { min: 0, max: 0 }),
        "3"
    );
    assert_eq!(
        format_number(-1234.5, p, FractionDigits { min: 0, max: 0 }),
        "-1,235"
    );
}

#[test]
fn non_finite_values_render_zero_not_nan() {
    for p in P.map(|x| x.0) {
        assert_eq!(format_number(f64::NAN, p, FractionDigits::default()), "0");
        assert_eq!(
            format_number(f64::INFINITY, p, FractionDigits::default()),
            "0"
        );
        assert_eq!(format_compact(f64::NAN, p), "0");
        assert_eq!(format_token_amount(f64::NAN, p, false), "0");
    }
}

#[test]
fn compact_tiers_match_the_documented_examples() {
    let p = NumberPreset::CommaDot;
    assert_eq!(format_compact(1_234_567.89, p), "1.23M");
    assert_eq!(format_compact(4.5e9, p), "4.5B");
    assert_eq!(format_compact(820.0, p), "820");
    assert_eq!(format_compact(1.5e12, p), "1.5T");
    assert_eq!(format_compact(12_345_678.0, p), "12.3M");
    assert_eq!(format_compact(123_456_789.0, p), "123M");
    assert_eq!(format_compact(-1_234_567.89, p), "-1.23M");
}

#[test]
fn token_ladder_and_its_dust_rule() {
    let p = NumberPreset::CommaDot;
    assert_eq!(format_token_amount(0.0, p, false), "0");
    assert_eq!(format_token_amount(1234.5678, p, false), "1,234.57");
    assert_eq!(format_token_amount(12.34567, p, false), "12.3457");
    assert_eq!(format_token_amount(0.000123456, p, false), "0.000123");
    // The dust rule: 4 digits would round this to "0", so it falls back to 6 —
    // a tiny non-zero balance must never render as a bare 0 in the feed.
    assert_eq!(format_token_amount(0.000012, p, true), "0.000012");
    assert_eq!(format_token_amount(12_345_678.9, p, true), "12.3M");
}

#[test]
fn parse_locale_number_normalises_back_to_canonical() {
    assert_eq!(
        parse_locale_number("1,234.56", NumberPreset::CommaDot),
        "1234.56"
    );
    assert_eq!(
        parse_locale_number("1.234,56", NumberPreset::DotComma),
        "1234.56"
    );
    assert_eq!(
        parse_locale_number("1 234,56", NumberPreset::SpaceComma),
        "1234.56"
    );
    assert_eq!(
        parse_locale_number("1,23,45,678.9", NumberPreset::Indian),
        "12345678.9"
    );
    // A foreign keyboard's plain form still parses under comma_dot.
    assert_eq!(
        parse_locale_number("1234.56", NumberPreset::CommaDot),
        "1234.56"
    );
    // Arabic-Indic and Extended Arabic-Indic digits map to ASCII.
    assert_eq!(
        parse_locale_number("١٢٣٤,٥٦", NumberPreset::SpaceComma),
        "1234.56"
    );
    assert_eq!(
        parse_locale_number("۱۲۳۴,۵۶", NumberPreset::SpaceComma),
        "1234.56"
    );
    // Space grouping strips EVERY whitespace kind, so an NBSP or U+202F pasted
    // from another app parses too.
    assert_eq!(
        parse_locale_number("1\u{00A0}234,56", NumberPreset::SpaceComma),
        "1234.56"
    );
    assert_eq!(
        parse_locale_number("1\u{202F}234,56", NumberPreset::SpaceComma),
        "1234.56"
    );
}

#[test]
fn date_and_time_presets_match_the_settings_picker() {
    // 2026-06-13 13:45 UTC — `locale-format.ts:232`'s SAMPLE.
    let c = Civil::from_unix_millis(1_781_358_300_000, 0);
    assert_eq!(format_date(&c, DatePreset::YmdSlash), "2026/06/13");
    assert_eq!(format_date(&c, DatePreset::MdySlash), "06/13/2026");
    assert_eq!(format_date(&c, DatePreset::DmySlash), "13/06/2026");
    assert_eq!(format_date(&c, DatePreset::DmyDot), "13.06.2026");
    assert_eq!(format_date(&c, DatePreset::Iso), "2026-06-13");
    assert_eq!(
        format_date_time(&c, DatePreset::YmdSlash, TimePreset::H24, "en"),
        "2026/06/13, 13:45"
    );
    assert_eq!(
        format_date_time(&c, DatePreset::MdySlash, TimePreset::H12, "en"),
        "06/13/2026, 1:45 PM"
    );
}

/// Every assertion in `src/__tests__/services/locale-format.test.ts`, transcribed
/// literally rather than paraphrased.
///
/// That file is the app's own statement of what these functions do, so matching it
/// is what "byte-identical to `locale-format.ts`" actually means. Anything I
/// inferred from reading the implementation could be wrong in the same direction
/// the implementation is.
#[test]
fn matches_the_apps_own_locale_format_test_suite() {
    let cd = NumberPreset::CommaDot;
    let dc = NumberPreset::DotComma;
    let sc = NumberPreset::SpaceComma;
    let ind = NumberPreset::Indian;
    let f2 = FractionDigits { min: 2, max: 2 };

    // -- formatNumber -------------------------------------------------------
    assert_eq!(format_number(1_234_567.89, cd, f2), "1,234,567.89");
    assert_eq!(format_number(1_234_567.89, dc, f2), "1.234.567,89");
    assert_eq!(format_number(1_234_567.89, sc, f2), "1 234 567,89");
    assert_eq!(format_number(1_234_567.89, ind, f2), "12,34,567.89");
    assert_eq!(
        format_number(1.0, cd, FractionDigits { min: 0, max: 2 }),
        "1"
    );
    assert_eq!(
        format_number(1.5, cd, FractionDigits { min: 0, max: 2 }),
        "1.5"
    );
    assert_eq!(
        format_number(2_460_539.4, cd, FractionDigits { min: 0, max: 0 }),
        "2,460,539"
    );
    assert_eq!(format_number(-1234.5, cd, f2), "-1,234.50");

    // -- separators ---------------------------------------------------------
    assert_eq!(cd.separators().group, ",");
    assert_eq!(cd.separators().decimal, ".");
    assert_eq!(dc.separators().group, ".");
    assert_eq!(dc.separators().decimal, ",");
    for p in [cd, dc, sc, ind] {
        assert_eq!(
            p.input_separators().group,
            "",
            "input grouping must be empty"
        );
        assert_eq!(p.input_separators().decimal, p.separators().decimal);
    }

    // -- groupDigits --------------------------------------------------------
    assert_eq!(group_digits("1234567", cd), "1,234,567");
    assert_eq!(group_digits("1234567", dc), "1.234.567");
    assert_eq!(group_digits("1234567", sc), "1 234 567");
    assert_eq!(group_digits("1234567", ind), "12,34,567");
    assert_eq!(group_digits("12", cd), "12");
    assert_eq!(group_digits("0", dc), "0");

    // -- parseLocaleNumber --------------------------------------------------
    assert_eq!(parse_locale_number("1,234.56", cd), "1234.56");
    assert_eq!(parse_locale_number("47.284177", cd), "47.284177");
    assert_eq!(parse_locale_number("47,284177", dc), "47.284177");
    assert_eq!(parse_locale_number("1.234,56", dc), "1234.56");
    assert_eq!(parse_locale_number("1 234,56", sc), "1234.56");
    assert_eq!(parse_locale_number("12,34,567.8", ind), "1234567.8");
    assert_eq!(parse_locale_number("٤٧,٥", dc), "47.5");

    // -- formatCompact ------------------------------------------------------
    assert_eq!(format_compact(1_234_567.89, cd), "1.23M");
    assert_eq!(format_compact(12_345_678.9, cd), "12.3M");
    assert_eq!(format_compact(4.5e9, cd), "4.5B");
    assert_eq!(format_compact(123_456_789_012.0, cd), "123B");
    assert_eq!(format_compact(-1_500_000.0, cd), "-1.5M");
    assert_eq!(format_compact(820.0, cd), "820");
    assert_eq!(format_compact(0.5, cd), "0.5");

    // -- formatTokenAmount --------------------------------------------------
    assert_eq!(format_token_amount(0.0, cd, false), "0");
    assert_eq!(format_token_amount(1234.5678, cd, false), "1,234.57");
    assert_eq!(format_token_amount(12.3456, cd, false), "12.3456");
    assert_eq!(format_token_amount(0.000_042_12, cd, false), "0.000042");
    assert_eq!(format_token_amount(12_345_678.9, cd, true), "12.3M");
    assert_eq!(format_token_amount(999_999.0, cd, true), "999,999.00");
    assert_eq!(format_token_amount(0.5, cd, true), "0.5");
    assert_eq!(
        format_token_amount(12_345_678.9, cd, false),
        "12,345,678.90"
    );

    // -- dates & times ------------------------------------------------------
    let d = Civil::from_unix_millis(1_781_358_300_000, 0); // 2026-06-13 13:45
    assert_eq!(format_date(&d, DatePreset::YmdSlash), "2026/06/13");
    assert_eq!(format_date(&d, DatePreset::MdySlash), "06/13/2026");
    assert_eq!(format_date(&d, DatePreset::DmySlash), "13/06/2026");
    assert_eq!(format_date(&d, DatePreset::DmyDot), "13.06.2026");
    assert_eq!(format_date(&d, DatePreset::Iso), "2026-06-13");
    assert_eq!(format_time(&d, TimePreset::H24, "en"), "13:45");
    assert_eq!(format_time(&d, TimePreset::H12, "en"), "1:45 PM");
    let midnight = Civil::from_unix_millis(1_781_309_100_000, 0); // 2026-06-13 00:05
    assert_eq!(format_time(&midnight, TimePreset::H12, "en"), "12:05 AM");
    assert_eq!(format_time(&midnight, TimePreset::H24, "en"), "00:05");
    assert_eq!(
        format_date_time(&d, DatePreset::YmdSlash, TimePreset::H24, "en"),
        "2026/06/13, 13:45"
    );
}

// ---------------------------------------------------------------------------
// Currency — the part that was never right
// ---------------------------------------------------------------------------

#[test]
fn the_21_wrong_decimal_codes_are_corrected() {
    let o = FiatOptions {
        preset: NumberPreset::CommaDot,
        drop_minor_units_above: None,
    };
    // 6 codes CLDR gives 3 digits; the app rendered all of them with 2.
    for c in ["KWD", "BHD", "OMR", "JOD", "TND", "LYD"] {
        assert!(format_fiat(1.0, c, "X", "en", o).ends_with("1.000"), "{c}");
    }
    // 15 codes CLDR gives 0 digits; the app rendered all of them with 2.
    for c in [
        "LBP", "PKR", "MMK", "LAK", "COP", "ALL", "AFN", "IRR", "IQD", "SYP", "YER", "SOS", "BIF",
        "MGA", "SLL",
    ] {
        let got = format_fiat(1234.0, c, "X", "en", o);
        assert!(
            !got.contains('.'),
            "{c}: expected no minor units, got {got}"
        );
    }
}

#[test]
fn the_7_wrong_placement_locales_are_corrected() {
    let o = FiatOptions {
        preset: NumberPreset::CommaDot,
        drop_minor_units_above: None,
    };
    // 5 symbol-after locales.
    for l in ["vi", "ru", "fr", "it", "de"] {
        assert_eq!(
            format_fiat(1234.5, "EUR", "€", l, o),
            "1,234.50\u{00A0}€",
            "{l}"
        );
    }
    // pt-BR: symbol-before, with the NBSP.
    assert_eq!(
        format_fiat(1234.5, "BRL", "R$", "pt-BR", o),
        "R$\u{00A0}1,234.50"
    );
    // id: no-space locale, but `Rp` is alphabetic so currencySpacing applies.
    assert_eq!(format_fiat(1234.0, "IDR", "Rp", "id", o), "Rp\u{00A0}1,234");
    // es-MX is NOT in the wrong set — measured with its own currency it is correct.
    assert_eq!(format_fiat(1234.5, "MXN", "$", "es-MX", o), "$1,234.50");
}

// ---------------------------------------------------------------------------
// Relative time and day headers
// ---------------------------------------------------------------------------

fn engine(lng: &str) -> I18n {
    let en = Catalog::embedded("en").unwrap_or_else(|e| unreachable!("{e}"));
    let mut i = I18n::new(en).unwrap_or_else(|e| unreachable!("{e}"));
    if lng != "en" {
        if let Ok(c) = Catalog::embedded(lng) {
            i.load_catalog(c);
        }
    }
    i.change_language(lng);
    i
}

#[test]
fn relative_time_uses_translated_labels_and_the_compiled_weekday_table() {
    let e = engine("en");
    let now_ms = 1_781_358_300_000_i64;
    let now_s = now_ms / 1000;
    let d = DatePreset::MdySlash;

    assert_eq!(
        e.format_relative_time(now_s, now_ms, 0, d).as_deref(),
        Ok("now")
    );
    assert_eq!(
        e.format_relative_time(now_s - 120, now_ms, 0, d).as_deref(),
        Ok("2m")
    );
    assert_eq!(
        e.format_relative_time(now_s - 7200, now_ms, 0, d)
            .as_deref(),
        Ok("2h")
    );
    assert_eq!(
        e.format_relative_time(now_s - 10800, now_ms, 0, d)
            .as_deref(),
        Ok("3h")
    );
    // Within a week: the short weekday, from the generated table rather than
    // `toLocaleDateString` — the last host-Intl call on this path.
    assert_eq!(
        e.format_relative_time(now_s - 3 * 86_400, now_ms, 0, d)
            .as_deref(),
        Ok("Wed")
    );
    // Beyond a week: the date preset.
    assert_eq!(
        e.format_relative_time(now_s - 30 * 86_400, now_ms, 0, d)
            .as_deref(),
        Ok("05/14/2026")
    );
}

#[test]
fn relative_time_is_localised() {
    let now_ms = 1_781_358_300_000_i64;
    let now_s = now_ms / 1000;
    let d = DatePreset::MdySlash;
    let ja = engine("ja");
    assert_eq!(
        ja.format_relative_time(now_s, now_ms, 0, d).as_deref(),
        Ok("たった今")
    );
    // The weekday comes from the compiled table, not the host.
    assert_eq!(
        ja.format_relative_time(now_s - 3 * 86_400, now_ms, 0, d)
            .as_deref(),
        Ok("水")
    );
    let ru = engine("ru");
    assert_eq!(
        ru.format_relative_time(now_s - 3 * 86_400, now_ms, 0, d)
            .as_deref(),
        Ok("ср")
    );
}

#[test]
fn day_label_compares_civil_days_not_elapsed_hours() {
    let e = engine("en");
    let d = DatePreset::MdySlash;
    // 2026-06-13 00:30 local.
    let now_ms = 1_781_310_600_000_i64;
    let today_s = now_ms / 1000;
    assert_eq!(e.day_label(today_s, now_ms, 0, d).as_deref(), Ok("Today"));
    // 2026-06-12 23:00 — only 1.5 hours earlier, but a DIFFERENT civil day. An
    // elapsed-milliseconds comparison would call this "Today".
    assert_eq!(
        e.day_label(today_s - 5_400, now_ms, 0, d).as_deref(),
        Ok("Yesterday")
    );
    // Older than that falls through to the date preset.
    assert_eq!(
        e.day_label(today_s - 5 * 86_400, now_ms, 0, d).as_deref(),
        Ok("06/08/2026")
    );
}

#[test]
fn bidi_isolation_is_off_by_default_and_wraps_only_when_asked() {
    let e = engine("en");
    let vars = [("addr", vela_core::i18n::Var::Str("Alice"))];
    let opts = Options {
        vars: &vars,
        ..Options::default()
    };
    // Default: byte-identical to i18next, no isolate marks anywhere.
    let got = e.t("activity.toAddr", &opts).unwrap_or_default();
    assert_eq!(got, "to Alice");
    assert!(!got.contains('\u{2068}'), "isolation must be opt-in");
    // Explicit isolation wraps only the substituted value.
    assert_eq!(vela_core::l10n::isolate("Alice"), "\u{2068}Alice\u{2069}");
}

/// Spec 082 RJ15 (G49): a signed balance change never reads `−0`. A delta the
/// ladder rounds to nothing is written exactly; zero is never drawn; the
/// minus is U+2212 and the plus is written.
#[test]
fn a_signed_token_amount_never_reads_minus_zero() {
    use vela_core::l10n::format_signed_token_amount;
    let cd = NumberPreset::CommaDot;
    assert_eq!(
        format_signed_token_amount("-1000", 18, cd).as_deref(),
        Some("\u{2212}0.000000000000001"),
        "1000 wei is not −0"
    );
    assert_eq!(
        format_signed_token_amount("-1000000000000000", 18, cd).as_deref(),
        Some("\u{2212}0.001")
    );
    assert_eq!(format_signed_token_amount("0", 18, cd), None);
    assert_eq!(format_signed_token_amount("-000", 18, cd), None);
    assert_eq!(
        format_signed_token_amount("+2100000000000000000", 18, cd).as_deref(),
        Some("+2.1")
    );
    assert_eq!(
        format_signed_token_amount("-8450000000", 6, cd).as_deref(),
        Some("\u{2212}8,450.00"),
        "the ladder, where it prints a figure"
    );
    // The exact form speaks the preset's marks.
    assert_eq!(
        format_signed_token_amount("-1000", 18, NumberPreset::DotComma).as_deref(),
        Some("\u{2212}0,000000000000001")
    );
    // Unreadable text is no figure.
    for junk in ["", "-", "0x10", "1.5", "abc", "--1"] {
        assert_eq!(format_signed_token_amount(junk, 18, cd), None, "{junk:?}");
    }
}
