//
//  AmountTextTests.swift
//  VelaWalletTests
//
//  Spec 073: every amount field cleans its edit through the core's rule, with
//  the field's own text as `previous` — without a paste flag when the caller
//  cannot tell (below), with one from `AmountTextField`, which can (the
//  second suite). The cases read the same under every number preset (the
//  preset is global state other suites set), so none is chosen here; the
//  core's own tests cover each.
//

import Foundation
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
        for text in ["", "0", "0.", "0.0", "0.08", "4", "4.", "0.50", "53.4836"] {
            #expect(AmountText.clean(text, previous: String(text.dropLast())) == text)
        }
    }

    /// Issue #421: "08" stayed in the field, read as 8 — ten times what
    /// somebody who missed the point meant — and Continue stayed lit. A zero
    /// leading a digit is not kept; only a decimal mark may follow it.
    @Test func aZeroLeadingADigitIsNotKept() {
        #expect(AmountText.clean("08", previous: "0") == "8")
        #expect(AmountText.clean("00", previous: "0") == "0")
        #expect(AmountText.clean("0.08", previous: "0.0") == "0.08")
        #expect(AmountText.clean("0.0", previous: "0.") == "0.0")
        #expect(AmountText.clean(".", previous: "") == "0.")
        #expect(AmountText.clean("008.5", previous: "") == "8.5")
        // One typed comma is the decimal mark under every preset: "0,8" is 0.8.
        #expect(AmountText.clean("0,", previous: "0") == "0.")
    }
}

/// `AmountTextField`'s edit: one keystroke or one paste at a time, as UIKit
/// hands it over (`shouldChangeCharactersIn`), so the field never shows raw
/// text for the next key to land on.
///
/// The device test typed "0,5" and read "0." one run in three: the field
/// cleaned in `onChange`, a render after the edit, and the "5" was overwritten
/// by a write-back computed from the text before it. Here every key is applied
/// to what the previous one left, exactly as the field does. Preset-free for
/// the same reason as the suite above.
struct AmountEditTests {
    /// Typed key by key: the text each key lands on is what the last one left.
    private func type(_ keys: String, into start: String = "") -> String? {
        var text = start
        for key in keys {
            let at = NSRange(location: (text as NSString).length, length: 0)
            guard let edit = AmountEdit.apply(String(key), in: at, of: text) else { return nil }
            text = edit.text
        }
        return text
    }

    @Test func aDecimalCommaTypedQuicklyKeepsEveryKey() {
        #expect(type("0,5") == "0.5")
        #expect(type("12,75") == "12.75")
        #expect(type("0,05") == "0.05")
    }

    /// The comma is turned into the mark inside its own edit, so the key
    /// after it lands on "0." — never on a "0," still waiting to be cleaned.
    @Test func theCommaIsCleanedInItsOwnEdit() {
        let comma = AmountEdit.apply(",", in: NSRange(location: 1, length: 0), of: "0")
        #expect(comma == AmountEdit.Result(text: "0.", caret: 2))
        let five = AmountEdit.apply("5", in: NSRange(location: 2, length: 0), of: "0.")
        #expect(five == AmountEdit.Result(text: "0.5", caret: 3))
    }

    /// More than one character at once is a paste, and the field says so:
    /// read whole, never salvaged key by key.
    @Test func aPasteIsReadWhole() {
        #expect(AmountEdit.apply("1,234.56", in: NSRange(location: 0, length: 0), of: "")?.text == "1234.56")
        #expect(AmountEdit.apply("1.5e-7", in: NSRange(location: 0, length: 0), of: "") == nil)
        // Over a selection, and the caret after what was pasted.
        #expect(AmountEdit.apply("12.5", in: NSRange(location: 0, length: 3), of: "0.4")
                == AmountEdit.Result(text: "12.5", caret: 4))
    }

    @Test func aStrayKeyIsDroppedAndTheCaretStays() {
        #expect(AmountEdit.apply("a", in: NSRange(location: 1, length: 0), of: "4")
                == AmountEdit.Result(text: "4", caret: 1))
    }

    @Test func deletingIsAnEditToo() {
        #expect(AmountEdit.apply("", in: NSRange(location: 2, length: 1), of: "0.5")
                == AmountEdit.Result(text: "0.", caret: 2))
    }

    /// Issue #421 key by key: the field shows what the core holds after
    /// every key — "0" then "8" is "8", never "08".
    @Test func aZeroThenADigitIsTheDigit() {
        #expect(type("08") == "8")
        #expect(type("00") == "0")
        #expect(type("012") == "12")
        #expect(type("0.08") == "0.08")
        #expect(type("0.0") == "0.0")
        #expect(type("0,8") == "0.8")
    }

    /// "." becomes "0." — a character nobody typed — and the caret goes after
    /// the mark: left between the "0" and the ".", the next "5" would read
    /// "05." and clean to 5, where 0.5 was being typed.
    @Test func aMarkOnAnEmptyFieldReadsWithItsZero() {
        let point = AmountEdit.apply(".", in: NSRange(location: 0, length: 0), of: "")
        #expect(point == AmountEdit.Result(text: "0.", caret: 2))
        let comma = AmountEdit.apply(",", in: NSRange(location: 0, length: 0), of: "")
        #expect(comma == AmountEdit.Result(text: "0.", caret: 2))
        #expect(type(".5") == "0.5")
        #expect(type(",5") == "0.5")
    }

    /// A zero typed in front of "8" is refused where it was typed.
    @Test func aZeroInFrontOfAFigureChangesNothing() {
        #expect(AmountEdit.apply("0", in: NSRange(location: 0, length: 0), of: "8")
                == AmountEdit.Result(text: "8", caret: 0))
        // A paste: the zeros go, the figure stays, the caret after it.
        #expect(AmountEdit.apply("008.5", in: NSRange(location: 0, length: 0), of: "")
                == AmountEdit.Result(text: "8.5", caret: 3))
    }

    @Test func anEditOutsideTheTextIsRefused() {
        #expect(AmountEdit.apply("5", in: NSRange(location: 9, length: 0), of: "0.") == nil)
    }
}
