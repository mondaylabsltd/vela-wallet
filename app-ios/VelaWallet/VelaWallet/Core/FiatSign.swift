//
//  FiatSign.swift
//  VelaWallet
//
//  The sign a fiat figure is written with — the core's, in the app's language.
//
//  Every shell used to keep a table of its own. This one's had eight
//  currencies, "¥" for the yuan AND the yen and "$" for the US AND the Hong
//  Kong dollar; Android asked the platform instead, and on the owner's two
//  phones one balance read 「CN¥188.44」 and the other 「¥34.03」 (device walks
//  2026-10-10 / 10-11). The rule is one now, `l10n::fiat_symbol`:
//
//  - a glyph of its own is the symbol (€ £ ₩ ₫ ₹ …);
//  - "$" alone is the US dollar's, in every language; every other dollar and
//    peso carries its country (HK$, CA$, A$ …);
//  - "¥" alone is the reader's own of the two that share it — the yuan in
//    Chinese, the yen everywhere else — and the other carries its country
//    (JP¥ / CN¥);
//  - a currency with no glyph anybody would recognise, or a code the table
//    has never heard of, is its code.
//
//  So within one language no two currencies are written alike, and a figure's
//  sign is the same string on the home total, a holding's worth, a fee,
//  Settings' currency row and every row of its picker. There is no table
//  here and nothing to look a glyph up in: this asks the core.
//

import Foundation
import VelaCore

enum FiatSign {
    /// The sign for `code`, read by somebody whose app is in `language` (the
    /// app's language tag — `Loc.resolvedLanguage`). Empty for no code.
    static func symbol(_ code: String, language: String) -> String {
        let code = code.trimmingCharacters(in: .whitespaces)
        return code.isEmpty ? "" : fiatSymbol(code: code, language: language)
    }

    /// The sign as it LEADS a figure: a sign that ends in a letter — a code
    /// ("CHF"), "zł", "Rp", "R" — stands a no-break space off the digits, as
    /// the core's own `format_fiat` writes it ("CHF 12.50"); a glyph does not
    /// ("$12.50", "HK$12.50", "CN¥12.50").
    static func lead(_ code: String, language: String) -> String {
        let sign = symbol(code, language: language)
        guard let last = sign.last, last.isLetter else { return sign }
        return sign + "\u{00A0}"
    }

    /// Whether `code` has no sign but itself — a figure in it is written
    /// with its code, and a line that already names the code says it once.
    static func isCode(_ code: String, language: String) -> Bool {
        symbol(code, language: language).caseInsensitiveCompare(code.trimmingCharacters(in: .whitespaces)) == .orderedSame
    }
}
