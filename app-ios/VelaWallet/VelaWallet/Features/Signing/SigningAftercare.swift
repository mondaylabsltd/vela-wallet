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
//  Ported from Android's `SigningAftercare.kt` (079 US1).
//

import Foundation

enum SigningAftercare: Equatable {
    /// A message was signed.
    case signed(chainId: Int)
    /// The operation landed inside the wait; the page got this hash.
    case landed(chainId: Int, txHash: String)
    /// The wait ran out; the page got the operation hash and the tracker keeps
    /// following it.
    case stillConfirming(chainId: Int, userOpHash: String)

    var chainId: Int {
        switch self {
        case .signed(let chainId), .landed(let chainId, _), .stillConfirming(let chainId, _): chainId
        }
    }

    /// How long a tick stays before it goes by itself: a signature is seen at
    /// a glance, a landing carries a hash worth a second look.
    var tickSeconds: Double {
        if case .signed = self { return 1.4 }
        return 2.6
    }

    private static let transactionMethods: Set<String> = ["eth_sendTransaction", "wallet_sendCalls"]

    /// The ending an answer stands for, or `nil` when there is nothing to
    /// show — a refusal (the page was told why; the sheet already said so) or
    /// an answer with no result. `payload` is the core's `SignResponsePayload`
    /// as it went to the page; `submittedUserOp` the operation the relay
    /// accepted for this request, when it did.
    static func of(
        method: String, chainId: Int, payload: [String: Any], submittedUserOp: String?
    ) -> SigningAftercare? {
        guard payload["type"] as? String == "ok",
              let result = payload["result"] as? String, !result.isEmpty
        else { return nil }
        guard transactionMethods.contains(method) else { return .signed(chainId: chainId) }
        if let submittedUserOp, result.caseInsensitiveCompare(submittedUserOp) == .orderedSame {
            return .stillConfirming(chainId: chainId, userOpHash: submittedUserOp)
        }
        return .landed(chainId: chainId, txHash: result)
    }
}
