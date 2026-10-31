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

    // MARK: - Names and networks (`multisig_rules`)

    /// The order networks are offered in for a new multisig, the default first.
    static func likelyNetworks(_ args: [String: Any]) -> [Int] {
        (call("networks.likely", args) as? [Any] ?? []).compactMap { ($0 as? NSNumber)?.intValue }
    }
}
