//
//  SignedDigits.swift
//  VelaWallet
//
//  Signed integer arithmetic on decimal STRINGS.
//
//  Every amount that crosses the core boundary is a decimal string in an
//  asset's smallest unit, and the values here are 32-byte words — far past what
//  `Int64` holds and past the exact range of `Double`. Swift's standard library
//  has no big integer, and `TokenReads` already does the unsigned half of this
//  (multiply and add) for the same reason.
//
//  This is the signed half: what netting a wallet's transfers in and out of one
//  token needs, and nothing else.
//

import Foundation

/// A signed integer as a decimal string, with the sign carried separately so
/// the digit arithmetic never has to think about it.
struct SignedDigits: Equatable {
    /// `true` when the value is negative. Zero is never negative.
    private(set) var negative: Bool
    /// Digits, most significant first, no leading zeros. `"0"` for zero.
    private(set) var digits: String

    static let zero = SignedDigits(negative: false, digits: "0")

    var isZero: Bool { digits == "0" }

    /// The value as the core reads it: `-` for negative, digits otherwise.
    var text: String { isZero ? "0" : (negative ? "-" + digits : digits) }

    /// A decimal string, with or without a sign. `nil` for anything that is not
    /// one — never a zero, because "this is not a number" and "this is nothing"
    /// are different answers.
    init?(_ value: String) {
        var text = value.trimmingCharacters(in: .whitespaces)
        var negative = false
        if text.hasPrefix("-") {
            negative = true
            text.removeFirst()
        } else if text.hasPrefix("+") {
            text.removeFirst()
        }
        guard !text.isEmpty, text.allSatisfy(\.isNumber) else { return nil }
        let trimmed = String(text.drop { $0 == "0" })
        self.digits = trimmed.isEmpty ? "0" : trimmed
        self.negative = self.digits == "0" ? false : negative
    }

    private init(negative: Bool, digits: String) {
        self.negative = digits == "0" ? false : negative
        self.digits = digits
    }

    /// A 32-byte hex word (with or without `0x`) as an unsigned value.
    ///
    /// Only the first word is read: a `Transfer` log's `data` carries the value
    /// in its first 32 bytes and anything after it belongs to another field.
    static func firstWord(hex: String) -> SignedDigits? {
        var text = hex.hasPrefix("0x") || hex.hasPrefix("0X") ? String(hex.dropFirst(2)) : hex
        guard !text.isEmpty else { return nil }
        text = String(text.prefix(64))
        var value = SignedDigits.zero
        for character in text {
            guard let digit = character.hexDigitValue else { return nil }
            value = value.multiplied(by: 16).adding(SignedDigits(negative: false, digits: String(digit)))
        }
        return value
    }

    func adding(_ other: SignedDigits) -> SignedDigits {
        if negative == other.negative {
            return SignedDigits(negative: negative, digits: Self.sum(digits, other.digits))
        }
        switch Self.compare(digits, other.digits) {
        case .orderedSame:
            return .zero
        case .orderedDescending:
            return SignedDigits(negative: negative, digits: Self.difference(digits, other.digits))
        case .orderedAscending:
            return SignedDigits(negative: other.negative, digits: Self.difference(other.digits, digits))
        }
    }

    func subtracting(_ other: SignedDigits) -> SignedDigits {
        adding(SignedDigits(negative: !other.negative, digits: other.digits))
    }

    func multiplied(by factor: Int) -> SignedDigits {
        guard factor != 0, !isZero else { return .zero }
        var carry = 0
        var out = ""
        for character in digits.reversed() {
            let product = (character.wholeNumberValue ?? 0) * abs(factor) + carry
            out.append(Character(String(product % 10)))
            carry = product / 10
        }
        while carry > 0 {
            out.append(Character(String(carry % 10)))
            carry /= 10
        }
        let result = String(String(out.reversed()).drop { $0 == "0" })
        return SignedDigits(
            negative: negative != (factor < 0),
            digits: result.isEmpty ? "0" : result
        )
    }

    // MARK: - Unsigned digit arithmetic

    private static func sum(_ a: String, _ b: String) -> String {
        var carry = 0
        var out = ""
        let left = Array(a.reversed()), right = Array(b.reversed())
        for index in 0..<max(left.count, right.count) {
            let total = (index < left.count ? left[index].wholeNumberValue ?? 0 : 0)
                + (index < right.count ? right[index].wholeNumberValue ?? 0 : 0) + carry
            out.append(Character(String(total % 10)))
            carry = total / 10
        }
        if carry > 0 { out.append(Character(String(carry))) }
        let result = String(String(out.reversed()).drop { $0 == "0" })
        return result.isEmpty ? "0" : result
    }

    /// `a - b`, where `a >= b`.
    private static func difference(_ a: String, _ b: String) -> String {
        var borrow = 0
        var out = ""
        let left = Array(a.reversed()), right = Array(b.reversed())
        for index in 0..<left.count {
            var digit = (left[index].wholeNumberValue ?? 0) - borrow
                - (index < right.count ? right[index].wholeNumberValue ?? 0 : 0)
            if digit < 0 {
                digit += 10
                borrow = 1
            } else {
                borrow = 0
            }
            out.append(Character(String(digit)))
        }
        let result = String(String(out.reversed()).drop { $0 == "0" })
        return result.isEmpty ? "0" : result
    }

    private static func compare(_ a: String, _ b: String) -> ComparisonResult {
        if a.count != b.count { return a.count < b.count ? .orderedAscending : .orderedDescending }
        if a == b { return .orderedSame }
        return a < b ? .orderedAscending : .orderedDescending
    }
}
