//
//  AddressText.swift
//  VelaWallet
//
//  How an address is shortened for display, in one place.
//
//  Spec 050 needed a fourth copy of `prefix(6)…suffix(4)` for the live contacts
//  rows, which is the point at which a repeated expression becomes a rule worth
//  naming.
//
//  ## The three older copies are NOT folded in, and the reason is a real one
//
//  `RootView.shortenAddress` and `ExploreFixtures.shorten` guard on
//  `count > 14`; `WalletModels.shorten` guards on `count > 10`. For a 42-character
//  address every one of them agrees, so the difference has never been visible —
//  but "never visible for the inputs we happen to send" is not equivalence, and
//  quietly picking one threshold for all three would be a behaviour change
//  disguised as a cleanup. New code uses this; the three copies and their
//  disagreement are recorded as a debt with a named owner instead.
//
//  The form is 6+4 (`0x9F3c…21aE`) because that is what the mocks were drawn
//  against and what every iOS surface already shows. **The web client shortens
//  8+6** (`identity.ts:40-43`), which is a genuine cross-client inconsistency in
//  the drawings rather than in the code; changing either would move strings a
//  screenshot sweep guards, so it is recorded, not silently harmonised.
//

import Foundation

enum AddressText {

    /// `0x9F3cA71b…8021aE` → `0x9F3c…21aE`.
    ///
    /// Anything too short to truncate is returned whole: half an ellipsis over
    /// a malformed address tells a person less than the malformed address does.
    static func short(_ address: String) -> String {
        guard address.count > 14 else { return address }
        // r11-6: an address is said in the core's one short form (`names.short`:
        // 6+6, EIP-55, `0x88cCA0…266894`) on every platform; anything else
        // keeps the cut below.
        if isAddress(address), let said = MsRules.string("names.short", ["address": address]) {
            return said
        }
        return "\(address.prefix(6))\u{2026}\(address.suffix(4))"
    }

    private static func isAddress(_ text: String) -> Bool {
        guard text.count == 42, text.hasPrefix("0x") || text.hasPrefix("0X") else { return false }
        return text.dropFirst(2).allSatisfy(\.isHexDigit)
    }

    /// Hex longer than this is split in two (a 6+6 short form never is).
    private static let splitFrom = 24
    /// Longer hex (calldata) wraps as text does: two halves would not fit.
    private static let splitUpTo = 66

    /// Where a full address (or a hash) may break across lines: between its
    /// two halves only — `0x` + half the hex | the other half, 22 + 20 for an
    /// address. The desktop's rule (`ui/address.rs`) and Android's
    /// (`AddressBreaks`); `nil` when `text` is drawn whole (r13: a member's
    /// last five digits alone on a line).
    static func halves(_ text: String) -> (String, String)? {
        guard text.hasPrefix("0x") || text.hasPrefix("0X") else { return nil }
        guard (splitFrom...splitUpTo).contains(text.count) else { return nil }
        let hex = text.dropFirst(2)
        guard hex.allSatisfy(\.isHexDigit) else { return nil }
        let mid = text.index(text.startIndex, offsetBy: 2 + (hex.count + 1) / 2)
        return (String(text[..<mid]), String(text[mid...]))
    }

    /// `0x777E98…61875a`: hex, an ellipsis, hex — and nothing that could wrap.
    static func isShortForm(_ text: String) -> Bool {
        text.count <= splitFrom + 4
            && (text.hasPrefix("0x") || text.hasPrefix("0X"))
            && text.contains("\u{2026}")
            && !text.contains(where: \.isWhitespace)
    }

    /// The mock's two mono lines: the address split in half, never mid-byte.
    ///
    /// Lives here rather than beside its first caller because spec 051 needed a
    /// second one — the receive screen wraps the same address the contact sheet
    /// does, and two halves computed two ways is how the same address comes to
    /// look like two different ones.
    static func lines(_ address: String) -> [String] {
        guard address.count > 1 else { return [address] }
        let split = address.index(address.startIndex, offsetBy: (address.count + 1) / 2)
        return [String(address[..<split]), String(address[split...])]
    }
}
