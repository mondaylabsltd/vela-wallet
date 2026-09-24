//
//  AmountText.swift
//  VelaWallet
//
//  An amount field's edit, made readable for the core before it is sent on
//  (spec 073; the core's `l10n::amount_text` says why).
//
//  A decimal-comma keypad's "4,5" reached the send machine raw, and in fiat
//  mode it was read as 4 — a different sum, silently; a custom allowance's
//  parser dropped the comma and allowed 45. Every field that takes an amount
//  cleans its edit here, holds what comes back, and sends THAT on, so what is
//  on screen is what the machine has.
//

import VelaCore

enum AmountText {
    /// `next` as the core reads it, `previous` being the field's text before
    /// the edit — how one keystroke is told from a paste (a SwiftUI field
    /// cannot say which it was, so anything but one added character reads as
    /// a paste). `nil`: a paste with no reading as one figure ("1.5e-7"); the
    /// field keeps `previous` and nothing is sent.
    static func clean(_ next: String, previous: String) -> String? {
        amountTextClean(
            raw: next,
            number: Formats.resolve(Formats.current.number).rawValue,
            previous: previous,
            pasted: nil
        )
    }
}
