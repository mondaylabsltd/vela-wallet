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
        return "\(address.prefix(6))\u{2026}\(address.suffix(4))"
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
