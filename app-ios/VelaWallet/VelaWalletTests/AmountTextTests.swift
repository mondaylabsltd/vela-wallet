//
//  AmountTextTests.swift
//  VelaWalletTests
//
//  Spec 073: every SwiftUI amount field cleans its edit through the core's
//  rule, with the field's own text as `previous` and no paste flag. The
//  cases read the same under every number preset (the preset is global state
//  other suites set), so none is chosen here; the core's own tests cover each.
//

import Testing
@testable import VelaWallet

struct AmountTextTests {
    @Test func oneTypedCommaIsTheDecimalMark() {
        #expect(AmountText.clean("4,", previous: "4") == "4.")
        #expect(AmountText.clean("4.5", previous: "4.") == "4.5")
    }

    @Test func moreThanOneCharacterAtOnceReadsAsAPaste() {
        #expect(AmountText.clean("1.234,56", previous: "") == "1234.56")
        #expect(AmountText.clean("1,234.56", previous: "") == "1234.56")
        // How a tiny balance is printed on many screens: refused, never 1.57.
        #expect(AmountText.clean("1.5e-7", previous: "") == nil)
        #expect(AmountText.clean("0x10", previous: "2") == nil)
    }

    @Test func aKeyTypedIntoAFigureNeverMovesItAThousandfold() {
        #expect(AmountText.clean("1.234,56", previous: "1.23456") == "1.23456")
    }

    @Test func aCleanFigureIsLeftAsTyped() {
        for text in ["", "0", "4", "4.", ".5", "0.50", "53.4836"] {
            #expect(AmountText.clean(text, previous: String(text.dropLast())) == text)
        }
    }
}
