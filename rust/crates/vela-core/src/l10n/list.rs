//! A list inside a sentence, joined the way the locale joins one (r10
//! follow-up).
//!
//! "Used on Base, Arbitrum and Ethereum", "transactions #1, #2 and #3": the
//! desktop joined every such list with ", " in every language — zh read
//! "交易 #1, #2, #3" where it writes "#1、#2和#3". iOS and Android draw lists
//! with the system's formatter (`ListFormatter`); the web with
//! `Intl.ListFormat`. A shell with neither — the desktop — joins here, from
//! the same CLDR "and" pattern those formatters use (`list_data`, extracted
//! from ICU by `scripts/gen-i18n.mjs`).
//!
//! Pure: no host locale facility (the module's rule 1).

use super::list_data;

/// `items` as one "and" list in `locale` (`zh`, `pt-BR`, …; an unknown one is
/// English, as the resolver falls back): none is the empty string, one is
/// itself, two are joined by the pair's word, more by the list's separators
/// — the first, the middle ones, and the one before the last.
#[must_use]
pub fn format_list<S: AsRef<str>>(locale: &str, items: &[S]) -> String {
    let (_, two, start, middle, end) = row(locale);
    match items {
        [] => String::new(),
        [one] => one.as_ref().to_owned(),
        [first, second] => format!("{}{two}{}", first.as_ref(), second.as_ref()),
        [first, rest @ .., last] => {
            let mut out = String::from(first.as_ref());
            for (i, item) in rest.iter().enumerate() {
                out.push_str(if i == 0 { start } else { middle });
                out.push_str(item.as_ref());
            }
            out.push_str(end);
            out.push_str(last.as_ref());
            out
        }
    }
}

/// The locale's row: itself (`zh-HK`, `zh_hk`), else the first shipped one
/// of its language (`pt` -> `pt-BR`, `zh-CN` -> `zh`), else English.
fn row(
    locale: &str,
) -> &'static (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    &'static str,
) {
    let wanted = locale.trim().replace('_', "-");
    let language = |tag: &str| tag.split('-').next().unwrap_or(tag).to_ascii_lowercase();
    list_data::LISTS
        .iter()
        .find(|r| r.0.eq_ignore_ascii_case(&wanted))
        .or_else(|| {
            list_data::LISTS
                .iter()
                .find(|r| language(r.0) == language(&wanted))
        })
        .unwrap_or_else(|| list_data::row("en"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_list_is_joined_the_way_its_locale_joins_one() {
        let three = ["Base", "Arbitrum", "Ethereum"];
        assert_eq!(format_list("en", &three), "Base, Arbitrum, and Ethereum");
        assert_eq!(format_list("zh", &three), "Base、Arbitrum和Ethereum");
        assert_eq!(format_list("zh-HK", &three), "Base、Arbitrum及Ethereum");
        assert_eq!(format_list("ja", &three), "Base、Arbitrum、Ethereum");
        assert_eq!(format_list("de", &three), "Base, Arbitrum und Ethereum");
        assert_eq!(format_list("zh", &["#1", "#2", "#3"]), "#1、#2和#3");
        assert_eq!(format_list("fr", &["A", "B"]), "A et B");
        assert_eq!(format_list("ru", &["A"]), "A");
        assert_eq!(format_list::<&str>("ru", &[]), "");
        let four = ["A", "B", "C", "D"];
        assert_eq!(format_list("en", &four), "A, B, C, and D");
        assert_eq!(format_list("ko", &four), "A, B, C 및 D");
    }

    #[test]
    fn an_unknown_locale_is_english_and_a_region_falls_back_to_its_language() {
        assert_eq!(format_list("xx", &["A", "B"]), "A and B");
        assert_eq!(format_list("pt", &["A", "B"]), "A e B");
        assert_eq!(format_list("zh-CN", &["A", "B", "C"]), "A、B和C");
    }
}
