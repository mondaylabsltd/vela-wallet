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
//  Shared by `SettingsFixtures` and `SettingsLive` since spec 050, for the same
//  reason `ContactsLabels` is: the gallery board and the live screen must offer
//  the same eight currencies with the same symbols, and two copies of that list
//  is how they stop.
//
//  Until 051 owns the rate path this is the eight the drawing shows. A
//  provider-driven list replaces the constant, not the shape.
//

import Foundation

enum CurrencyCatalog {

    struct Entry {
        let code: String
        /// The symbol the picker's leading badge draws. Not unique — HKD and
        /// USD share `$`, CNY and JPY share `¥` — which is why the code is the
        /// identifier and this is only decoration.
        let glyph: String
        /// Provider data, not corpus copy.
        let name: String
    }

    static let entries: [Entry] = [
        Entry(code: "USD", glyph: "$", name: "US Dollar"),
        Entry(code: "EUR", glyph: "€", name: "Euro"),
        Entry(code: "GBP", glyph: "£", name: "British Pound"),
        Entry(code: "CNY", glyph: "¥", name: "Chinese Yuan"),
        Entry(code: "JPY", glyph: "¥", name: "Japanese Yen"),
        Entry(code: "KRW", glyph: "₩", name: "South Korean Won"),
        Entry(code: "HKD", glyph: "$", name: "Hong Kong Dollar"),
        Entry(code: "VND", glyph: "₫", name: "Vietnamese Dong"),
    ]

    /// `nil` for a code the catalog does not know — which is a real case: the
    /// core will happily carry a code this list has never heard of, because the
    /// person's device region chose it.
    static func entry(_ code: String) -> Entry? {
        entries.first { $0.code == code }
    }
}
