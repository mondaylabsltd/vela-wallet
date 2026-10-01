//
//  SigningAftercare.swift
//  VelaWallet
//
//  What a page's request ended as, held after the core closes its sheet (spec
//  079). The core clears the signing sheet the moment it answers the page —
//  with the transaction hash when the operation landed inside the wait, with
//  the operation hash when the wait ran out, with the signature for a message
//  — and the sheet used to vanish at that instant: the person never saw it
//  land. This keeps the ending on screen: a tick that goes away by itself, or,
//  for an operation still on its way, the honest "not landed yet" until the
//  person closes it or the tracker sees it land.
//
//  ## The ending is the core's (spec 082 RA8)
//
//  What an answer stands for (`signEndingOf`) and what the sheet draws once
//  the tracker has had its say (`signEndingState`) are one rule on every
//  client. The derivation this file used to carry drew a landed operation
//  "confirmed" without asking the tracker — and a reverted dApp transaction
//  read 已确认 (W3). Nothing here decides: it holds the core's ending and asks
//  the core again whenever the tracker's entry changes.
//

import Foundation
import VelaCore

struct SigningAftercare: Equatable {
    /// The request's own chain, for the explorer and the header.
    let chainId: Int
    /// The core's `SignEnding` — decoded for the drawing, kept as JSON for
    /// `signEndingState`.
    let ending: Ending
    let endingJson: String

    /// `sign_request::SignEnding`.
    enum Ending: Equatable {
        /// A message was signed.
        case signed
        /// The receipt arrived inside the wait; the page got this hash. Not
        /// "confirmed" until the tracker says so — it may have reverted.
        case landed(txHash: String, userOpHash: String?)
        /// Included and reverted (083): the page got the revert, naming the
        /// transaction.
        case reverted(txHash: String, userOpHash: String?)
        /// The wait ran out; the page got the operation hash and the tracker
        /// keeps following it.
        case stillConfirming(userOpHash: String)
    }

    /// The operation the tracker follows for this ending, when there is one.
    var userOpHash: String? {
        switch ending {
        case .signed: nil
        case .landed(_, let op): op
        case .reverted(_, let op): op
        case .stillConfirming(let op): op
        }
    }

    /// How long a tick stays before it goes by itself: a signature is seen at
    /// a glance, a landing carries a hash worth a second look.
    var tickSeconds: Double {
        if case .signed = ending { return 1.4 }
        return 2.6
    }

    /// The ending an answer stands for, or `nil` when there is nothing to
    /// show — a refusal (the page was told why; the sheet already said so) or
    /// an answer with no result. `payload` is the core's `SignResponsePayload`
    /// as it went to the page; `submittedUserOp` the operation this request
    /// handed the tracker, when it did — the core's `signEndingOf`.
    static func of(
        method: String, chainId: Int, payload: [String: Any], submittedUserOp: String?
    ) -> SigningAftercare? {
        guard let json = (try? signEndingOf(
            method: method, payloadJson: CoreJSON.string(payload), submittedUserOp: submittedUserOp
        )) ?? nil,
              let ending = decode(json)
        else { return nil }
        return SigningAftercare(chainId: chainId, ending: ending, endingJson: json)
    }

    /// What the sheet draws for this ending with the tracker's entry for its
    /// operation (`nil`: not taken yet) — the core's `signEndingState`.
    func state(track: TrackEntryWire?) -> SignEndingStateWire {
        guard let json = try? signEndingState(endingJson: endingJson, trackEntryJson: track?.coreJSON),
              let object = try? CoreJSON.object(json),
              let state = SignEndingStateWire(object)
        else {
            // A shape this build cannot read: still on its way, never a tick.
            return .following(userOpHash: userOpHash ?? "", outcome: "landing", feeHeld: false)
        }
        return state
    }

    private static func decode(_ json: String) -> Ending? {
        guard let object = try? CoreJSON.object(json) else { return nil }
        switch object["type"] as? String {
        case "signed":
            return .signed
        case "landed":
            guard let tx = object["tx_hash"] as? String else { return nil }
            return .landed(txHash: tx, userOpHash: object["user_op_hash"] as? String)
        case "reverted":
            guard let tx = object["tx_hash"] as? String else { return nil }
            return .reverted(txHash: tx, userOpHash: object["user_op_hash"] as? String)
        case "still_confirming":
            guard let op = object["user_op_hash"] as? String else { return nil }
            return .stillConfirming(userOpHash: op)
        default:
            return nil
        }
    }
}

/// `sign_request::SignEndingState` — what an ending looks like once the
/// tracker has had its say.
enum SignEndingStateWire: Equatable {
    /// Tick, "signed", closes by itself.
    case signed
    /// Tick, the short hash, the explorer.
    case confirmed(txHash: String)
    /// Cross: it landed and reverted (a Safe `ExecutionFailure` included).
    case reverted(txHash: String)
    /// Cross: the relay never had it — nothing was sent.
    case notSent
    /// Cross: the relay refused it — nothing was sent, and sending the same
    /// thing again will not help (spec 082 RJ3). Never "try again".
    case refused
    /// Still on its way. `outcome` is the tracker's `TrackOutcome` wire name
    /// (`maybe_sent`, `landing`, `still_confirming`, `unknown`).
    case following(userOpHash: String, outcome: String, feeHeld: Bool)

    init?(_ object: [String: Any]) {
        switch object["type"] as? String {
        case "signed": self = .signed
        case "confirmed": self = .confirmed(txHash: object["tx_hash"] as? String ?? "")
        case "reverted": self = .reverted(txHash: object["tx_hash"] as? String ?? "")
        case "not_sent": self = .notSent
        case "refused": self = .refused
        case "following":
            self = .following(
                userOpHash: object["user_op_hash"] as? String ?? "",
                outcome: object["outcome"] as? String ?? "landing",
                feeHeld: object["fee_held"] as? Bool ?? false
            )
        default: return nil
        }
    }

    /// The transaction a link can open, when there is one.
    var txHash: String? {
        switch self {
        case .confirmed(let tx), .reverted(let tx): tx.isEmpty ? nil : tx
        default: nil
        }
    }
}
