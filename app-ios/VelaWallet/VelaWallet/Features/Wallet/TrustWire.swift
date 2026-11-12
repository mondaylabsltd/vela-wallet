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
    /// §41 (P2-3): an NFT entering or leaving the account — its collection,
    /// its id, `standard` (`erc721` / `erc1155`, the core's word, kept as
    /// written), which way, and how many (`amount`, a magnitude). Nobody
    /// vouches for it: drawn with its id and the caution, never a price.
    case nft(token: String, tokenId: String, standard: String, direction: TrustSimDirectionWire, amount: String)
    /// §41 (P2-3): the account lets `spender` spend `token` — `unlimited`,
    /// or one NFT (`tokenId`). Drawn in danger, with no figure.
    case approval(token: String, spender: String, unlimited: Bool, tokenId: String?)
    /// §41 (P2-3): an asset or approval log naming the account that Vela
    /// could not read (`token` its emitter): "This changes something Vela
    /// can't read".
    case unread(token: String?)

    private enum CodingKeys: String, CodingKey {
        case type, token, delta, symbol, decimals, inTrustedSet, direction
        case tokenId, standard, amount, spender, unlimited
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
        case "nft":
            self = .nft(
                token: try container.decode(String.self, forKey: .token),
                tokenId: try container.decode(String.self, forKey: .tokenId),
                standard: try container.decode(String.self, forKey: .standard),
                direction: (try? container.decodeIfPresent(TrustSimDirectionWire.self, forKey: .direction))
                    ?? .unreadable,
                amount: try container.decodeIfPresent(String.self, forKey: .amount) ?? "1"
            )
        case "approval":
            self = .approval(
                token: try container.decode(String.self, forKey: .token),
                spender: try container.decode(String.self, forKey: .spender),
                unlimited: try container.decodeIfPresent(Bool.self, forKey: .unlimited) ?? false,
                tokenId: try container.decodeIfPresent(String.self, forKey: .tokenId)
            )
        case "unread":
            self = .unread(token: try? container.decodeIfPresent(String.self, forKey: .token))
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
        case .nft(let token, let tokenId, let standard, let direction, let amount):
            return [
                "type": "nft", "token": token, "token_id": tokenId, "standard": standard,
                "direction": direction.rawValue, "amount": amount,
            ]
        case .approval(let token, let spender, let unlimited, let tokenId):
            var wire: [String: Any] = [
                "type": "approval", "token": token, "spender": spender, "unlimited": unlimited,
            ]
            // Absent when there is none, as the core writes it.
            if let tokenId { wire["token_id"] = tokenId }
            return wire
        case .unread(let token):
            return ["type": "unread", "token": token.map { $0 as Any } ?? NSNull()]
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
    /// Round 16 leftover 5: the networks of this account the scan could not
    /// read on its last look at each — handed to the Activity feed with the
    /// scan's answer (`sync_completed.unread_chains`). Absent reads none.
    var unreadChains: [Int] = []

    private enum CodingKeys: String, CodingKey { case address, scanning, incoming, sim, unreadChains }

    init(address: String?, scanning: Bool, incoming: [TrustIncomingWire], sim: TrustSimViewWire?, unreadChains: [Int] = []) {
        self.address = address
        self.scanning = scanning
        self.incoming = incoming
        self.sim = sim
        self.unreadChains = unreadChains
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        address = try c.decodeIfPresent(String.self, forKey: .address)
        scanning = try c.decode(Bool.self, forKey: .scanning)
        incoming = try c.decode([TrustIncomingWire].self, forKey: .incoming)
        sim = try c.decodeIfPresent(TrustSimViewWire.self, forKey: .sim)
        unreadChains = try c.decodeIfPresent([Int].self, forKey: .unreadChains) ?? []
    }
}
