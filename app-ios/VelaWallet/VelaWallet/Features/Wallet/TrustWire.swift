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
    /// Unix seconds: the time of the transfer's own block, as the chain gave
    /// it — never a clock's (invariant ⑨). A transfer whose block could not
    /// be read is not here yet.
    let timestampSec: Double
    let symbol: String?
    let decimals: Int?
}

/// Which way an unverified token moves (`TrustSimDirection`) — all the core
/// tells a sheet about it.
enum TrustSimDirectionWire: String, Decodable, Equatable {
    /// The account receives it: "+".
    case `in`
    /// It leaves the account: "−".
    case out
    /// A move of nothing (the simulation's figure was a zero): never a row.
    case still
    /// The figure was no signed number, so there is no direction to state:
    /// the row, its caution, and no sign.
    case unreadable

    /// A direction this build has never heard of is one it cannot state:
    /// the row with its caution and no sign, never a view that fails.
    init(from decoder: Decoder) throws {
        let raw = try decoder.singleValueContainer().decode(String.self)
        self = TrustSimDirectionWire(rawValue: raw) ?? .unreadable
    }
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
    /// `inTrustedSet`: the coin is one this wallet already trusts, not merely
    /// one whose `symbol()` answered — carried so the judgment can be handed
    /// back to the core verbatim (`wire`), never read here.
    case erc20Trusted(token: String, delta: String, symbol: String, decimals: Int, inTrustedSet: Bool = false)
    /// A direction and a caution, and **no figure** (PR 3, fix B). The
    /// simulation's number for a token nobody vouches for is whatever the
    /// site being signed for chose to emit, so the core hands over only which
    /// way it moves: this case cannot hold the amount, and a sheet cannot
    /// print what it was never given. (It held the raw `delta` until then,
    /// "to read the sign from", and one client printed it: 「未验证代币
    /// +5,000,000,000,000,000,000,000.00」.)
    case erc20Unverified(token: String?, direction: TrustSimDirectionWire)

    private enum CodingKeys: String, CodingKey {
        case type, token, delta, symbol, decimals, inTrustedSet, direction
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.container(keyedBy: CodingKeys.self)
        switch try container.decode(String.self, forKey: .type) {
        case "native":
            self = .native(delta: try container.decode(String.self, forKey: .delta))
        case "erc20_trusted":
            self = .erc20Trusted(
                token: try container.decode(String.self, forKey: .token),
                delta: try container.decode(String.self, forKey: .delta),
                symbol: try container.decode(String.self, forKey: .symbol),
                decimals: try container.decode(Int.self, forKey: .decimals),
                inTrustedSet: try container.decodeIfPresent(Bool.self, forKey: .inTrustedSet) ?? false
            )
        default:
            // The direction is the core's word. A judgment written before
            // it had one (`{"type":"erc20_unverified","delta":"…"}`) still
            // reads — its figure is not read at all, here or anywhere — and
            // has no direction this file may state for it. (A stored record's
            // lines never come through here: `TxRecords.toWire` hands them to
            // the core as stored, and the core reads both shapes.)
            self = .erc20Unverified(
                token: try? container.decodeIfPresent(String.self, forKey: .token),
                direction: (try? container.decodeIfPresent(TrustSimDirectionWire.self, forKey: .direction))
                    ?? .unreadable
            )
        }
    }

    /// The judgment as the core wrote it (`TrustSimJudgment`), for the
    /// approve's `balance_changes` (083 F1, spec 093): the record keeps the
    /// sheet's own verdict, and the feed re-reads it — nothing is re-judged
    /// or re-formatted here.
    var wire: [String: Any] {
        switch self {
        case .native(let delta):
            return ["type": "native", "delta": delta]
        case .erc20Trusted(let token, let delta, let symbol, let decimals, let inTrustedSet):
            var wire: [String: Any] = [
                "type": "erc20_trusted", "token": token, "delta": delta,
                "symbol": symbol, "decimals": decimals,
            ]
            // Absent when false, as the core writes it.
            if inTrustedSet { wire["in_trusted_set"] = true }
            return wire
        case .erc20Unverified(let token, let direction):
            return [
                "type": "erc20_unverified", "token": token.map { $0 as Any } ?? NSNull(),
                "direction": direction.rawValue,
            ]
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
    /// The verdict's quiet line when the checked answer moves nothing of the
    /// person's: `componentsUi.signing.simResultNoChange`, "No asset
    /// changes" — set by the core once `ready`, with no judgment or every
    /// one a zero. **The sheet says the line exactly when this key is there**
    /// and never picks the case, or the sentence, itself. `nil` while
    /// resolving, whenever something moves, and on a view from before the
    /// key.
    var noChangeKey: String? = nil
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
