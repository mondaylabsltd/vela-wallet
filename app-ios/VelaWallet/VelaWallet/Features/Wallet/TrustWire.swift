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

/// The core's verdict on ONE simulated balance change.
///
/// Asymmetric by design (`token_trust::judge_delta`), and the asymmetry is the
/// point: an OUTFLOW renders whenever the token's metadata resolved, because
/// the real token emits its own log and an outflow cannot be understated; an
/// INFLOW renders a confident number only for a token already in the trusted
/// set, because a `Transfer` log is something anybody can emit.
enum TrustSimJudgmentWire: Decodable, Equatable {
    /// A native value move. Always rendered — the chain's symbol and 18
    /// decimals are the shell's vocabulary.
    case native(delta: String)
    /// Metadata resolved AND (an outflow, or a trusted-set inflow).
    case erc20Trusted(token: String, delta: String, symbol: String, decimals: Int)
    /// Direction and caution, and **no attacker-controlled amount**.
    case erc20Unverified(token: String?, delta: String)

    private enum CodingKeys: String, CodingKey {
        case type, token, delta, symbol, decimals
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        let type = try container.decode(String.self, forKey: .type)
        let delta = try container.decode(String.self, forKey: .delta)
        switch type {
        case "native":
            self = .native(delta: delta)
        case "erc20_trusted":
            self = .erc20Trusted(
                token: try container.decode(String.self, forKey: .token),
                delta: delta,
                symbol: try container.decode(String.self, forKey: .symbol),
                decimals: try container.decode(Int.self, forKey: .decimals)
            )
        default:
            self = .erc20Unverified(
                token: try container.decodeIfPresent(String.self, forKey: .token),
                delta: delta
            )
        }
    }

    /// Positive means the wallet RECEIVES. The sign lives in the core's
    /// string, and reading it here is the shell's one arithmetic fact.
    var incoming: Bool { !(delta.hasPrefix("-")) }

    var delta: String {
        switch self {
        case .native(let delta): delta
        case .erc20Trusted(_, let delta, _, _): delta
        case .erc20Unverified(_, let delta): delta
        }
    }
}

/// The sign sheet's half: whether the simulation has answered, and its verdict
/// per asset.
///
/// **`ready` is the gate.** Judgments read before it are a previous request's,
/// and a balance block from the wrong transaction is worse than none.
struct TrustSimViewWire: Decodable, Equatable {
    let ready: Bool
    let judgments: [TrustSimJudgmentWire]
}

/// The scan half of the view.
struct TrustViewWire: Decodable, Equatable {
    let address: String?
    /// A poll is in progress. The scan facade waits for this to fall.
    let scanning: Bool
    /// Newest first — block descending, then log index.
    let incoming: [TrustIncomingWire]
    /// The sign sheet's simulation verdict (spec 055). `nil` until something
    /// has been simulated for this account.
    let sim: TrustSimViewWire?
}
