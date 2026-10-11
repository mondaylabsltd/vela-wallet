//
//  CurrencyCatalog.swift
//  VelaWallet
//
//  Which currencies the picker offers, and what each one is called.
//
//  Not the core's, deliberately. `display_currency` owns the *choice* and
//  whether a code can be priced right now; the LIST is provider-driven — the FX
//  endpoint decides what it can quote — so the names are data here rather than
//  120 corpus strings.
//
//  **Not their symbols.** What a currency's figure is written with is the
//  core's rule (`FiatSign`, `fiatSymbol(code:language:)`), and Settings' row
//  and picker ask it like every figure does. This list used to keep a glyph
//  per code — "¥" for the yuan AND the yen, "$" for the US AND the Hong Kong
//  dollar — so two rows of one picker read alike.
//
//  Shared by `SettingsFixtures` and `SettingsLive` since spec 050, for the same
//  reason `ContactsLabels` is: the gallery board and the live screen must offer
//  the same eight currencies, and two copies of that list is how they stop.
//
//  Until 051 owns the rate path this is the eight the drawing shows. A
//  provider-driven list replaces the constant, not the shape.
//

import Foundation

enum CurrencyCatalog {

    struct Entry {
        let code: String
        /// Provider data, not corpus copy.
        let name: String
    }

    static let entries: [Entry] = [
        Entry(code: "USD", name: "US Dollar"),
        Entry(code: "EUR", name: "Euro"),
        Entry(code: "GBP", name: "British Pound"),
        Entry(code: "CNY", name: "Chinese Yuan"),
        Entry(code: "JPY", name: "Japanese Yen"),
        Entry(code: "KRW", name: "South Korean Won"),
        Entry(code: "HKD", name: "Hong Kong Dollar"),
        Entry(code: "VND", name: "Vietnamese Dong"),
    ]

    /// `nil` for a code the catalog does not know — which is a real case: the
    /// core will happily carry a code this list has never heard of, because the
    /// person's device region chose it.
    static func entry(_ code: String) -> Entry? {
        entries.first { $0.code == code }
    }
}
