//
//  PaymentRequestWire.swift
//  VelaWallet
//
//  The `payment_request` machine's view, in Swift — the half a deep link needs.
//
//  Views are `Decodable` through `CoreJSON.decoder`; operations and results
//  stay dictionaries.
//

import Foundation

/// A `/pay` link the core accepted, with everything a prefilled send needs.
struct PayRequestWire: Decodable, Equatable {
    let recipient: String
    let chainId: Int
    /// `nil` = the chain's own coin.
    let tokenAddress: String?
    /// The human amount exactly as the headline shows it, or `nil` for an open
    /// request — somebody asking to be paid without saying how much.
    let amount: String?
    /// Base units as a decimal string, present iff `amount` is.
    let amountBase: String?
    let symbol: String
    let decimals: Int
    let networkName: String
    let eip681Uri: String
}

struct PaymentRequestAssetWire: Decodable, Equatable {
    let chainId: Int
    let tokenAddress: String?
    let symbol: String
    let decimals: Int
    let networkName: String
}

struct PaymentRequestViewWire: Decodable, Equatable {
    let gateLoading: Bool
    let acknowledged: Bool
    let canCopy: Bool
    let canSave: Bool
    let asset: PaymentRequestAssetWire
    let amount: String
    let eip681Uri: String
    let payLink: String
    let qrValue: String
    let copyPayload: String
    let hasAmount: Bool
    /// `nil` until a link has been opened; then the core's verdict on it.
    let payValid: Bool?
    /// Present only when `payValid` is `true`.
    let pay: PayRequestWire?
}
