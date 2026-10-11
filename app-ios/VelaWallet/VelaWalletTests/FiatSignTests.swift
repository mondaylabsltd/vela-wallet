//
//  FiatSignTests.swift
//  VelaWalletTests
//
//  One sign for a currency, on every figure and in Settings (device walks
//  2026-10-10 / 10-11: the iPhone wrote 「¥34.03」 where the Android phone
//  wrote 「CN¥188.44」 under a Settings row that said 「CNY · ¥」).
//
//  The sign is the core's `fiatSymbol(code:language:)` and nothing else: the
//  shell keeps no table. What is pinned here is that every place this shell
//  writes a fiat figure — the one formatter behind the hero, the holdings,
//  the fee and the rest; the send form's unit; Settings' row and each row of
//  its picker — asks it, in the app's language.
//

import Foundation
import Testing
import VelaCore
@testable import VelaWallet

@MainActor
struct FiatSignTests {
    private let zh = Loc(overrideTag: "zh", preferredLanguages: [])
    private let en = Loc(overrideTag: "en", preferredLanguages: [])
    private let ja = Loc(overrideTag: "ja", preferredLanguages: [])
    private let hk = Loc(overrideTag: "zh-HK", preferredLanguages: [])

    private func display(_ code: String, _ loc: Loc) -> WalletLive.Display {
        .from(CurrencyViewWire(code: code, rate: 2, committed: true), language: loc.resolvedLanguage)
    }

    /// The yuan is the bare "¥" to a Chinese reader and "CN¥" to anybody
    /// else; the yen the reverse; the Hong Kong dollar is never a bare "$".
    @Test func theYuanTheYenAndTheDollars() {
        #expect(FiatSign.symbol("CNY", language: "zh") == "¥")
        #expect(FiatSign.symbol("CNY", language: "zh-TW") == "¥")
        #expect(FiatSign.symbol("CNY", language: "zh-HK") == "¥")
        #expect(FiatSign.symbol("CNY", language: "en") == "CN¥")
        #expect(FiatSign.symbol("CNY", language: "ja") == "CN¥")
        #expect(FiatSign.symbol("JPY", language: "zh") == "JP¥")
        #expect(FiatSign.symbol("JPY", language: "en") == "¥")
        #expect(FiatSign.symbol("JPY", language: "ja") == "¥")
        for language in Loc.supported {
            #expect(FiatSign.symbol("USD", language: language) == "$")
            #expect(FiatSign.symbol("HKD", language: language) == "HK$")
            #expect(FiatSign.symbol("CNY", language: language) != FiatSign.symbol("JPY", language: language),
                    "\(language): the yuan and the yen are written alike")
        }
        // It is the core's answer, not a table of this shell's.
        for code in ["USD", "EUR", "GBP", "CNY", "JPY", "KRW", "HKD", "VND", "CHF", "PLN", "BRL", "XPF"] {
            for language in ["zh", "en"] {
                #expect(FiatSign.symbol(code, language: language) == fiatSymbol(code: code, language: language))
            }
        }
        #expect(FiatSign.symbol("", language: "en").isEmpty)
    }

    /// Every fiat figure is written by the one formatter, with that sign.
    @Test func everyFigureWearsTheSignOfTheReadersLanguage() {
        #expect(display("CNY", zh).fiat(94.22) == "¥188.44")
        #expect(display("CNY", en).fiat(94.22) == "CN¥188.44")
        #expect(display("JPY", zh).fiat(94.22) == "JP¥188.44")
        #expect(display("JPY", en).fiat(94.22) == "¥188.44")
        #expect(display("JPY", ja).fiat(94.22) == "¥188.44")
        #expect(display("CNY", hk).fiat(94.22) == "¥188.44")
        #expect(display("HKD", zh).fiat(94.22) == "HK$188.44")
        #expect(display("HKD", en).fiat(94.22) == "HK$188.44")
        #expect(display("USD", zh).fiat(94.22) == "$188.44")
        // The hero's two parts carry it on the integer.
        #expect(display("CNY", zh).fiatParts(94.22)?.integer == "¥188")
        #expect(display("CNY", en).fiatParts(94.22)?.integer == "CN¥188")
        // A sign that ends in a letter stands a no-break space off the
        // digits, as the core's own formatter writes it; a glyph does not.
        #expect(display("CHF", en).fiat(94.22) == "CHF\u{00A0}188.44")
        #expect(display("PLN", en).fiat(94.22) == "zł\u{00A0}188.44")
        #expect(display("ZAR", en).fiat(94.22) == "R\u{00A0}188.44")
        #expect(display("BRL", en).fiat(94.22) == "R$188.44")
        #expect(display("XPF", en).fiat(94.22) == "XPF\u{00A0}188.44", "a code no table knows is written with its code")
        // The live home builds its display from the app's own language.
        let home = WalletFixtures.buildMobileState(.h1, loc: zh)
        let view = BalanceCoreScene.zeroWallet()?.settled
        if let view {
            let cny = CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
            #expect(WalletLive.apply(view, currency: cny, on: home, loc: zh).balance.integer == "¥0")
            #expect(WalletLive.apply(view, currency: cny, on: home, loc: en).balance.integer == "CN¥0")
        }
    }

    /// Settings' currency row and every row of its picker: the same sign —
    /// and no two rows of the picker alike.
    @Test func settingsSaysTheSameSignAndNoTwoRowsShareOne() {
        for loc in [zh, en, ja] {
            let language = loc.resolvedLanguage
            let base = SettingsFixtures.build(.st5, loc: loc)
            for entry in CurrencyCatalog.entries {
                let view = CurrencyViewWire(code: entry.code, rate: 2, committed: true)
                let model = SettingsLive.withCurrency(view, on: base, loc: loc)
                let sign = fiatSymbol(code: entry.code, language: language)

                // The row: "CNY · ¥2,469.12" — the sign every figure wears.
                let row = model.sections.flatMap(\.rows).first { $0.id == "currency" }?.value
                #expect(row == "\(entry.code) · \(sign)2,469.12", "\(language) \(entry.code): \(row ?? "nil")")
                #expect(row == SettingsLive.currencyRowValue(view, language: language))
                #expect(display(entry.code, loc).fiat(1_234.56) == "\(sign)2,469.12", "the row and a figure disagree")

                // The picker.
                let picker = model.currencySheet.rows
                #expect(picker.map(\.id) == CurrencyCatalog.entries.map(\.code))
                #expect(picker.first { $0.id == entry.code }?.glyph == sign)
                #expect(picker.filter(\.selected).map(\.id) == [entry.code])
            }
            let signs = SettingsLive.withCurrency(
                CurrencyViewWire(code: "USD", rate: 1, committed: true), on: base, loc: loc
            ).currencySheet.rows.compactMap(\.glyph)
            #expect(signs.count == CurrencyCatalog.entries.count)
            #expect(Set(signs).count == signs.count, "\(language): two rows of the picker share a sign: \(signs)")
            // The drawn board asks the core as well.
            let drawn = base.currencySheet.rows.compactMap(\.glyph)
            #expect(Set(drawn).count == drawn.count, "\(language): the drawn picker repeats a sign: \(drawn)")
            #expect(drawn == CurrencyCatalog.entries.map { fiatSymbol(code: $0.code, language: language) })
        }
    }

    /// The owner's two phones, side by side: Chinese, CNY — one string.
    @Test func theOwnersRowAndBalanceAgree() {
        let view = CurrencyViewWire(code: "CNY", rate: 7.1, committed: true)
        #expect(SettingsLive.currencyRowValue(view, language: zh.resolvedLanguage) == "CNY · ¥8,765.38")
        #expect(display("CNY", zh).fiat(26.5422535) == "¥53.08")
        #expect(!SettingsLive.currencyRowValue(view, language: zh.resolvedLanguage).contains("CN¥"))
    }
}
