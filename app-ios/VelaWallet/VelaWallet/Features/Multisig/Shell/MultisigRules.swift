//
//  MultisigRules.swift
//  VelaWallet
//
//  The multisig's pure rules, asked one at a time through the link door
//  (`multisigLinkCall`, the web's `multisigLinkCall`): the link codec
//  (proposal, invite and connect links, row ids), amounts in base units,
//  member labels and kinds, the order networks are offered in, the relay's
//  token schedule. Nothing here decides: each is one question to the core.
//

import Foundation
import VelaCore

enum MsRules {
    /// One rule: `method` with its arguments, the answer as JSON (`nil` when
    /// the core refused the question).
    static func call(_ method: String, _ args: [String: Any] = [:]) -> Any? {
        guard let answer = try? multisigLinkCall(method: method, argsJson: CoreJSON.string(args)),
              let data = answer.data(using: .utf8)
        else { return nil }
        return try? JSONSerialization.jsonObject(with: data, options: [.fragmentsAllowed])
    }

    static func string(_ method: String, _ args: [String: Any] = [:]) -> String? {
        call(method, args) as? String
    }

    // MARK: - Amounts

    /// Base units as the page says them (`1,234.5`; `<0.000001`).
    static func formatUnits(_ amount: String, decimals: Int) -> String {
        string("formatUnits", ["amount": amount, "decimals": decimals]) ?? amount
    }

    /// Base units as a plain decimal string (a record's `value`).
    static func plainUnits(_ amount: String, decimals: Int) -> String {
        string("plainUnits", ["amount": amount, "decimals": decimals]) ?? amount
    }

    /// `1.5` → base units, or `nil` when the text is not an amount.
    static func parseUnits(_ text: String, decimals: Int) -> String? {
        switch call("parseUnits", ["text": text, "decimals": decimals]) {
        case let s as String: return s
        case let n as NSNumber: return n.stringValue
        default: return nil
        }
    }

    // MARK: - Rows and links

    static func executedRowId(chainId: Int, safeTxHash: String, index: Int) -> String {
        string("executedRowId", ["chainId": chainId, "safeTxHash": safeTxHash, "index": index])
            ?? "multisig:\(chainId):\(safeTxHash.lowercased()):\(index)"
    }

    static func waitingRowId(chainId: Int, safeTxHash: String) -> String {
        string("waitingRowId", ["chainId": chainId, "safeTxHash": safeTxHash])
            ?? "multisig:\(chainId):\(safeTxHash.lowercased())"
    }

    /// The multisig transaction an Activity row is, from its id.
    static func transactionOfRow(_ id: String) -> (chainId: Int, safeTxHash: String)? {
        guard let object = call("transactionOfRow", ["id": id]) as? [String: Any],
              let chainId = (object["chainId"] as? NSNumber)?.intValue,
              let hash = object["safeTxHash"] as? String
        else { return nil }
        return (chainId, hash)
    }

    /// The link that opens a proposal on another member's device.
    static func proposalLink(origin: String, locale: String, chainId: Int, multisig: String, safeTxHash: String) -> String? {
        string("proposalLink", [
            "origin": origin, "locale": locale,
            "link": ["chainId": chainId, "multisig": multisig, "safeTxHash": safeTxHash],
        ])
    }

    /// The invite link: the multisig, its networks, its members as this
    /// device names them.
    static func inviteLink(
        origin: String, locale: String, address: String, owners: [String], chainIds: [Int],
        now: [String], known: [String: Any]
    ) -> String? {
        string("inviteLink", [
            "origin": origin, "locale": locale,
            "account": ["address": address, "owners": owners, "chainIds": chainIds],
            "now": now, "known": known,
        ])
    }

    /// r9-12: the invite as the core builds it (`addLink`): its networks and
    /// the row's `invite_members` — founding members, then members now, a
    /// name only where one is known, and the kind.
    static func addLink(origin: String, locale: String, multisig: String, chainIds: [Int], members: Any) -> String? {
        string("addLink", [
            "origin": origin, "locale": locale, "multisig": multisig,
            "extra": ["chainIds": chainIds, "members": members],
        ])
    }

    /// The connect link an outside owner opens in a browser with its wallet.
    static func connectLink(page: String, invite: String?, members: [String], tx: (chainId: Int, multisig: String, safeTxHash: String)?) -> String? {
        var args: [String: Any] = ["page": page, "invite": invite ?? NSNull(), "members": members]
        args["tx"] = tx.map { ["chainId": $0.chainId, "multisig": $0.multisig, "safeTxHash": $0.safeTxHash] } ?? NSNull()
        return string("connectLink", args)
    }

    static func eip55(_ address: String) -> String { string("eip55", ["address": address]) ?? address }

    static func progressLine(template: String, approved: Int, threshold: Int) -> String {
        string("progressLine", ["template": template, "approved": approved, "threshold": threshold]) ?? template
    }

    static var memberNameMax: Int { (call("memberNameMax") as? NSNumber)?.intValue ?? 32 }

    /// Whether `hash` (a URL fragment, with or without `#`) is a multisig link.
    static func isMultisigLink(_ hash: String) -> Bool { call("isMultisigLink", ["hash": hash]) as? Bool ?? false }

    // MARK: - The sign door (§12)

    /// The one rule every sign / propose / approve / execute / reject button
    /// follows (`review.door`): a review's state (`nil`: not asked yet) and
    /// the review (`nil` or `null`: none) in, what the sheet draws and
    /// whether the button may be pressed out.
    static func reviewDoor(state: MsJSON?, review: MsJSON?) -> MsSignDoor {
        let answer = call("review.door", [
            "state": state.map(\.any) ?? NSNull(),
            "review": review.map(\.any) ?? NSNull(),
        ])
        return MsSignDoor(answer: answer)
    }

    /// What `review.missing` says while the list is still being read.
    static let missingOpening = "home.multisig.proposal.opening"

    /// §18 (b), round 14: what a proposal sheet opened WITHOUT a link says
    /// while its transaction is not in the list (`review.missing`): the
    /// FULL catalog key (round 16 leftover 3: `home.multisig.service.busyNote`
    /// is not under `proposal.`) and whether to offer Try again — a multisig
    /// service that did not answer is said as that, never as "it hasn't been
    /// shared yet"; one that answered with its rate limit (`busy`) is said
    /// busy, never "Couldn't reach the multisig service". A core that cannot
    /// be asked: still opening.
    static func missingSaid(loading: Bool, unreadable: Bool, mailbox: Bool?, busy: Bool = false) -> (key: String, retry: Bool) {
        let answer = call("review.missing", [
            "loading": loading,
            "unreadable": unreadable,
            "mailbox": mailbox.map { $0 as Any } ?? NSNull(),
            "busy": busy,
        ]) as? [String: Any]
        return ((answer?["key"] as? String) ?? missingOpening, answer?["retry"] as? Bool ?? false)
    }

    /// `missingSaid` from the proposals view's facts for `chainId` — the
    /// one question every proposal sheet asks.
    static func missingSaid(view: MsProposalsView, chainId: Int) -> (key: String, retry: Bool) {
        missingSaid(
            loading: view.loading,
            unreadable: view.unreadable.contains(chainId),
            mailbox: view.chains.first(where: { $0.chainId == chainId })?.mailbox,
            busy: view.busy
        )
    }

    /// Round 16 leftover 4: a failure's stable KIND and the full catalog key
    /// that says it (`review.failure {detail}`) — `key` `nil` for the kinds
    /// worded from the detail's own facts (`fee_moved`, `cannot_pay`,
    /// `venue_blocked`). No shell reads a detail's text; a core that cannot
    /// be asked says nothing more precise than `unknown`.
    static func failure(_ detail: String?) -> (kind: String, key: String?) {
        guard let answer = call("review.failure", ["detail": detail ?? ""]) as? [String: Any],
              let kind = answer["kind"] as? String
        else { return ("unknown", "home.multisig.proposal.reason.unknown") }
        return (kind, answer["key"] as? String)
    }

    // MARK: - Names and networks (`multisig_rules`)

    /// The order networks are offered in for a new multisig, the default first.
    static func likelyNetworks(_ args: [String: Any]) -> [Int] {
        (call("networks.likely", args) as? [Any] ?? []).compactMap { ($0 as? NSNumber)?.intValue }
    }
}

/// §12: the core's sign door over one review (`MultisigSignDoor`). Nothing
/// here decides: `enabled` is the core's, and only `true` opens a button.
struct MsSignDoor: Equatable {
    /// What the sheet draws in the review's place.
    enum Shows: String, Equatable {
        /// "Reading the transaction…".
        case loading
        /// The review itself.
        case review
        /// The reason, "Nothing can be signed until it's read." and "Try again".
        case failed
    }

    let shows: Shows
    /// The button may be pressed.
    let enabled: Bool
    /// Pressed, it goes through the review's "… anyway" step first.
    let anyway: Bool
    /// `failed`: the reason's corpus key.
    let reasonKey: String?
    /// `failed`: offer "Try again" (`review_again`).
    let retry: Bool

    /// D1 (real-device pass 2): the review is drawn, but neither its
    /// state-change check nor its simulation was read — the core's danger
    /// line (`reasonKey`) goes above it with "Try again", and every press is
    /// "… anyway" (`anyway`).
    var unchecked: Bool { shows == .review && !(reasonKey ?? "").isEmpty }

    init(shows: Shows, enabled: Bool, anyway: Bool, reasonKey: String?, retry: Bool) {
        self.shows = shows
        self.enabled = enabled
        self.anyway = anyway
        self.reasonKey = reasonKey
        self.retry = retry
    }

    /// The door's answer as the core wrote it. One the core did not give (it
    /// refused the question) opens nothing: shut, with "Try again".
    init(answer: Any?) {
        guard let object = answer as? [String: Any],
              let shows = (object["shows"] as? String).flatMap(Shows.init(rawValue:))
        else {
            self = .unanswered
            return
        }
        self.shows = shows
        enabled = object["enabled"] as? Bool == true
        anyway = object["anyway"] as? Bool == true
        reasonKey = object["reason_key"] as? String
        retry = object["retry"] as? Bool == true
    }

    /// No answer from the door: nothing is signed, and the person can ask again.
    static let unanswered = MsSignDoor(shows: .failed, enabled: false, anyway: false, reasonKey: nil, retry: true)
}
