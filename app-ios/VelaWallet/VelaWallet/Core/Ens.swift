//
//  Ens.swift
//  VelaWallet
//
//  EIP-137 namehash, over the bridge's keccak.
//
//  Ported from `app-web/vela-wallet/src/lib/services/ens.ts`, and it lives on
//  its own for the same reason that file does: two unrelated features address
//  contracts by ENS name — the Chainlink fiat feeds (`<ccy>-usd.data.eth`) and
//  the recipient-identity waterfall (`<addr>.addr.reverse`) — and a namehash
//  that disagreed between them would resolve one of the two to nothing at all,
//  silently.
//
//      namehash("")    = 0x00…00
//      namehash("a.b") = keccak256(namehash("b") ‖ keccak256("a"))
//

import Foundation
import VelaCore

enum Ens {

    /// The 32-byte node for a name. Not a second hash implementation — the
    /// keccak is `vela-core`'s (FR-006).
    static func namehash(_ name: String) -> Data {
        var node = Data(repeating: 0, count: 32)
        guard !name.isEmpty else { return node }
        for label in name.split(separator: ".").reversed() {
            node = keccak256(data: node + keccak256(data: Data(label.utf8)))
        }
        return node
    }

    /// The reverse node for an address: `namehash("<addr without 0x>.addr.reverse")`.
    ///
    /// Lowercased, because the reverse registry's names are — an address in
    /// checksum case hashes to a different node and resolves to nothing.
    static func reverseNode(_ address: String) -> Data {
        let tail = address.hasPrefix("0x") || address.hasPrefix("0X")
            ? String(address.dropFirst(2)) : address
        return namehash("\(tail.lowercased()).addr.reverse")
    }

    /// The low 20 bytes of a 32-byte word, as a `0x` address.
    static func addressWord(_ data: Data) -> String? {
        guard data.count >= 32 else { return nil }
        return "0x" + data.subdata(in: 12..<32).hexString
    }

    static func isZeroAddress(_ address: String) -> Bool {
        let digits = address.hasPrefix("0x") ? String(address.dropFirst(2)) : address
        return !digits.isEmpty && digits.allSatisfy { $0 == "0" }
    }
}
