//
//  TrustWire.swift
//  VelaWallet
//
//  The `token_trust` machine's view model, in Swift.
//
//  Small, because almost nothing about this machine is a view: it is the
//  wallet's anti-scam core, and what it publishes is a **judged** list of
//  incoming transfers plus whether a scan is running. Every acceptance
//  decision behind that list — the allowlist, the local `topics[2]`
//  re-verification, the metadata gate — happens where the shell cannot reach.
//

import Foundation

/// One judged incoming transfer.
///
/// `symbol`/`decimals` are present only when the core resolved them. A native
/// row has neither, and the shell supplies the chain's own symbol; an ERC-20
/// row without them **never reaches here at all** — the core withholds it
/// (invariant ③), because an 18-decimals guess renders a 6-decimals
/// stablecoin as "+0 tokens".
struct TrustIncomingWire: Decodable, Equatable {
    let id: String
    let chainId: Int
    /// `nil` for the native coin.
    let token: String?
    let isNative: Bool
    let from: String
    /// The RAW on-chain amount, as a decimal string — not divided by decimals.
    let value: String
    let txHash: String
    let blockNumber: Double
    let logIndex: Int
    /// Unix seconds: the block's time, falling back to when it was seen.
    let timestampSec: Double
    let symbol: String?
    let decimals: Int?
}

/// The scan half of the view.
///
/// `sim` — the sign-sheet's asymmetric judgment — is **deliberately absent**.
/// Nothing in this cut simulates a transaction, its `TrustSimJudgment` is a
/// tagged union with three arms, and a mirror written now would be an untested
/// guess at a shape whose only reader arrives with the signing surface. Same
/// rule as the bridge exports: it comes with the code that reads it.
struct TrustViewWire: Decodable, Equatable {
    let address: String?
    /// A poll is in progress. The scan facade waits for this to fall.
    let scanning: Bool
    /// Newest first — block descending, then log index.
    let incoming: [TrustIncomingWire]
}
