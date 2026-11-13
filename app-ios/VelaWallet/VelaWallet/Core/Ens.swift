//
//  Ens.swift
//  VelaWallet
//
//  ENS nodes, from the core (core-perf-1: `ens_namehash`, `ens_reverse_node`).
//
//  Two unrelated features address contracts by ENS name — the Chainlink fiat
//  feeds (`<ccy>-usd.data.eth`) and the recipient-identity waterfall
//  (`<addr>.addr.reverse`) — and a namehash that disagreed between them, or
//  with another shell's, would resolve one of them to nothing at all,
//  silently. This shell used to fold the labels itself, one keccak crossing
//  per label; the rule is now `vela_core::ens`, the one every shell asks:
//
//      namehash("")    = 0x00…00
//      namehash("a.b") = keccak256(namehash("b") ‖ keccak256("a"))
//

import Foundation
import VelaCore

nonisolated enum Ens {

    /// The 32-byte node for a name (EIP-137). `nil` only if the core answered
    /// something that is not a node — never a zero node in its place, which
    /// is the root's.
    static func namehash(_ name: String) -> Data? {
        node(ensNamehash(name: name))
    }

    /// The reverse node for an address: `namehash("<addr without 0x>.addr.reverse")`,
    /// lowercased as the reverse registry's names are. `nil` for text that is
    /// not an address.
    static func reverseNode(_ address: String) -> Data? {
        (try? ensReverseNode(address: address)).flatMap(node)
    }

    private static func node(_ hex: String) -> Data? {
        guard let data = Data(hexString: hex), data.count == 32 else { return nil }
        return data
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
