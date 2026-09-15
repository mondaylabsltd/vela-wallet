//
//  MtokWire.swift
//  VelaWallet
//
//  The `manage_tokens` machine's view model, in Swift.
//
//  The panel is small and its states are all one screen: what was typed,
//  whether it is an address, what the chains answered, and what is already
//  saved. Every one of those judgements is the core's.
//

import Foundation

/// One found card — a chain that answered with usable metadata.
///
/// Only chains whose `name()` AND `symbol()` resolved appear here (the core's
/// invariant ②: no symbol, no listing). A card is therefore always addable.
struct MtokFoundWire: Decodable, Equatable {
    let chainId: Int
    let networkName: String
    let name: String
    let symbol: String
    let decimals: Int
    /// Recomputed live against the CURRENT input, so a card cannot claim
    /// "added" for an address somebody has since edited.
    let added: Bool
}

/// One saved token, as the manage list reads it back.
struct MtokCustomTokenWire: Decodable, Equatable {
    let id: String
    let chainId: Int
    let contractAddress: String
    let symbol: String
    let name: String
    let decimals: Int
    let networkName: String
}

struct MtokViewWire: Decodable, Equatable {
    /// As typed — the core owns the field's text, so the input binds to this.
    let inputAddress: String
    let addressValid: Bool
    let detecting: Bool
    /// Registry order.
    let found: [MtokFoundWire]
    /// True only while the write itself is in flight.
    let saving: Bool
    let customTokens: [MtokCustomTokenWire]
    /// Every chain answered and none of them knew this contract.
    let notFound: Bool
    let saveError: Bool
}
