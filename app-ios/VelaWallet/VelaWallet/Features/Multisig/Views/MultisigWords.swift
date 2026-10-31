//
//  MultisigWords.swift
//  VelaWallet
//
//  How a multisig and its transactions read on screen — the web's
//  `core/display.ts` and `shell/proposal-failure.ts`: member names (this
//  device's own, or the name a multisig keeps — never text that came with a
//  link or a proposal), a transaction's one line, a rule, why an action did
//  not go. Every verdict is the core's; this only puts the corpus's words on
//  it (`home.multisig.*`, the on-demand catalog Loc merges in).
//

import Foundation
import VelaCore

@MainActor
struct MsWords {
    let loc: Loc
    /// The hub, for what this device knows of a member (its accounts, the
    /// names the multisigs here keep). `nil`: addresses only.
    var hub: MultisigHub?
    /// This device's accounts (address → name).
    var localNames: [String: String] = [:]

    func t(_ key: String, _ vars: [String: String] = [:]) -> String {
        loc.t("home.multisig.\(key)", vars: vars)
    }

    func t(_ key: String, count: Int, _ vars: [String: String] = [:]) -> String {
        loc.t("home.multisig.\(key)", count: count, vars: vars)
    }

    // MARK: - Names

    /// "0xd9c79C…a20432": short, in its EIP-55 case.
    static func short(_ address: String) -> String {
        AddressText.short(MsRules.eip55(address))
    }

    /// A member's name on this device, or `nil` when it is not one of its accounts.
    func localName(_ address: String) -> String? {
        localNames[address.lowercased()]
    }

    /// What the multisigs here keep of a member (a label, never authority).
    func keptName(_ address: String) -> String? {
        hub?.accountsView.knownMembers[address.lowercased()]?.name.flatMap { $0.isEmpty ? nil : $0 }
    }

    /// A member as a line names it: its name and short address, never the
    /// name alone — a name is a claim, the address is who gets the say.
    func who(_ address: String) -> String {
        let short = Self.short(address)
        let name = localName(address) ?? keptName(address)
        guard let name, name != short else { return short }
        return "\(name) (\(short))"
    }

    /// A member's label: this device's name, else the kept one, else short.
    func label(_ address: String) -> String {
        localName(address) ?? keptName(address) ?? Self.short(address)
    }

    /// A multisig's name: its record's (when displayable), else this
    /// device's label, else its rule ("Multisig 2/3").
    func name(of account: MsAccount, rowName: String? = nil) -> String {
        if let rowName, !rowName.isEmpty { return rowName }
        if let recorded = account.genesisName, Self.validName(recorded) { return recorded }
        if let label = account.label, !label.isEmpty { return label }
        return t("switcher.rowName", ["m": String(account.threshold), "n": String(account.owners.count)])
    }

    static func validName(_ name: String) -> Bool {
        (try? multisigCall(method: "validName", argsJson: CoreJSON.string(["name": name]))) == "true"
    }

    /// "Needs 2 of 3 members" — "Needs its only member's approval" for one.
    func ruleTitle(m: Int, n: Int) -> String {
        n == 1 ? t("panel.titleSolo") : t("panel.title", ["m": String(m), "n": String(n)])
    }

    /// The header's chip: "Needs 2 of 3", "One member".
    func ruleChip(m: Int, n: Int) -> String {
        n == 1 ? t("panel.chipSolo") : t("panel.chip", ["m": String(m), "n": String(n)])
    }

    func ruleText(m: Int, n: Int) -> String {
        t("create.thresholdValue", ["m": String(m), "n": String(n)])
    }

    // MARK: - Networks

    func network(_ chainId: Int) -> String {
        ChainCatalog.meta(chainId)?.displayName ?? "Chain \(chainId)"
    }

    func symbol(_ chainId: Int) -> String {
        ChainCatalog.meta(chainId)?.nativeSymbol ?? "ETH"
    }

    // MARK: - A transaction's one line

    /// "Send 500 USDC", "Add Bob (0x12…34)" — the proposal's own title.
    func actionTitle(chainId: Int, action: MsJSON?, nonce: String) async -> String {
        guard let action, !action.isNull else { return t("action.unsupported") }
        switch action.kind {
        case "transfer":
            let fallback: (symbol: String, decimals: Int) = (symbol(chainId), 18)
            let info = await hub?.tokenInfo(chainId: chainId, token: action["token"]?.string) ?? fallback
            return t("action.send", [
                "amount": MsRules.formatUnits(action["amount"]?.string ?? "0", decimals: info.decimals),
                "symbol": info.symbol,
            ])
        default:
            return actionTitleNow(chainId: chainId, action: action, nonce: nonce)
        }
    }

    /// The same, without reading a token (a transfer of the chain's coin, or
    /// of a token whose symbol is not read yet, says the coin it knows).
    func actionTitleNow(chainId: Int, action: MsJSON?, nonce: String, token: (symbol: String, decimals: Int)? = nil) -> String {
        guard let action, !action.isNull else { return t("action.unsupported") }
        switch action.kind {
        case "transfer":
            let info = token ?? ((action["token"]?.string).map { _ in ("", 18) } ?? (symbol(chainId), 18))
            return t("action.send", [
                "amount": MsRules.formatUnits(action["amount"]?.string ?? "0", decimals: info.decimals),
                "symbol": info.symbol,
            ]).trimmingCharacters(in: .whitespaces)
        case "batch":
            let calls = action["calls"]?.array ?? []
            let allTransfers = calls.allSatisfy { $0["action"]?.kind == "transfer" }
            return t(allTransfers ? "action.batch" : "action.batchCalls", ["total": String(calls.count)])
        case "cancel":
            return t("action.cancel", ["nonce": nonce])
        case "addOwner":
            return t("change.add", ["member": who(action["owner"]?.string ?? "")])
        case "removeOwner":
            return t("change.remove", ["member": who(action["owner"]?.string ?? "")])
        case "swapOwner":
            return t("change.replace", [
                "old": who(action["oldOwner"]?.string ?? ""), "new": who(action["newOwner"]?.string ?? ""),
            ])
        case "changeThreshold":
            return t("action.changeThreshold", ["threshold": action["threshold"]?.string ?? ""])
        case "rename":
            return t("action.rename", ["name": action["name"]?.string ?? ""])
        case "call":
            let value = action["value"]?.string ?? "0"
            if value == "0" || value.isEmpty { return t("action.call") }
            return t("action.callWithValue", [
                "amount": MsRules.formatUnits(value, decimals: 18), "symbol": symbol(chainId),
            ])
        default:
            return t("action.unsupported")
        }
    }

    /// Who a transaction pays, when it pays one address.
    static func recipient(_ action: MsJSON?) -> String? {
        guard let action else { return nil }
        if action.kind == "transfer" || action.kind == "call" { return action["to"]?.string }
        return nil
    }

    // MARK: - A row's one word (`standings`)

    func standing(_ standing: MsTagged?, item: MsProposalItem) -> String {
        switch standing?.kind {
        case "unsent": return t("status.unsent")
        case "executing": return t("status.executing")
        case "queued": return t("status.queued")
        case "ready": return t("status.ready")
        case "yourTurn": return t("status.yourTurn")
        case "progress":
            return t("status.progress", [
                "approved": standing?["approved"]?.string ?? String(item.approved),
                "threshold": standing?["threshold"]?.string ?? String(item.threshold),
            ])
        case "blocked":
            return t("status.cannotRun", ["reason": blockReason(standing?["block"])])
        default:
            return t("status.progress", ["approved": String(item.approved), "threshold": String(item.threshold)])
        }
    }

    /// Why a ready transaction cannot run now (`MultisigReadyBlock`).
    func blockReason(_ block: MsJSON?) -> String {
        guard let block else { return "" }
        if block.kind == "nobodyPays" { return t("status.nobodyPays") }
        return whyFails(block["why"])
    }

    /// `short` (token, have, need) or `reverts` (reason).
    func whyFails(_ why: MsJSON?) -> String {
        guard let why else { return t("proposal.uncheckedReason") }
        switch why.kind {
        case "short":
            let decimals = why["token"]?["decimals"]?.int ?? 18
            let symbol = why["token"]?["symbol"]?.string ?? ""
            func amount(_ key: String) -> String {
                "\(MsRules.formatUnits(why[key]?.string ?? "0", decimals: decimals)) \(symbol)".trimmingCharacters(in: .whitespaces)
            }
            return t("proposal.whyShort", ["have": amount("have"), "need": amount("need")])
        case "reverts":
            if let reason = why["reason"]?.string, !reason.isEmpty {
                return t("proposal.whyReverts", ["reason": reason])
            }
            return t("proposal.whyRevertsPlain")
        default:
            return t("proposal.uncheckedReason")
        }
    }

    // MARK: - Why an action did not go (`MultisigActionState`)

    /// The action's failure in words; `nil` for one that says nothing
    /// (working, done, the person's own cancel).
    func actionFailure(_ state: MsTagged?, chainId: Int) -> String? {
        guard let state else { return nil }
        switch state.kind {
        case "working", "done", "cancelled", "outsideAsked": return nil
        case "mailboxUnreadable": return t("proposal.mailboxDown")
        case "noMemberHere": return t("proposal.notMember")
        case "signingRefused": return t("authority.refusedOn", ["network": network(chainId)])
        case "signingBlocked": return t("review.blocked")
        case "signingUnchecked": return t("authority.signUnchecked", ["network": network(chainId)])
        case "riskDeclined": return nil
        case "refused": return t("proposal.refused", ["reason": state["reason"]?.string ?? ""])
        case "mailboxRefused": return t("proposal.reason.serviceRefused")
        case "stopped":
            return t("proposal.wouldFail", ["reason": whyFails(state["preview"]?["why"] ?? state["preview"])])
        case "noOutsideSigner":
            return t("proposal.failed", ["reason": t("proposal.noOutsideSigner")])
        case "failed": return failureText(state["detail"]?.string)
        default: return t("proposal.reason.unknown")
        }
    }

    /// A shell failure (`MultisigShellFailure`'s JSON), or the core's own
    /// words, said as the corpus says them.
    func failureText(_ detail: String?) -> String {
        guard let object = MultisigShellFailure.read(detail) else {
            guard let detail, !detail.isEmpty else { return t("proposal.reason.unknown") }
            return t("proposal.failed", ["reason": detail])
        }
        switch object["kind"] as? String {
        case "passkey":
            if object["timedOut"] as? Bool == true { return t("proposal.reason.passkeyTimedOut") }
            switch object["failure"] as? String {
            case "cancelled": return t("proposal.reason.passkeyClosed")
            case "not_discoverable", "not_supported": return t("proposal.reason.passkeyMissing")
            default: return t("proposal.reason.passkeyFailed")
            }
        case "passkey_check": return t("proposal.reason.passkeyFailed")
        case "venue_blocked":
            if let block = object["block"] as? [String: Any],
               let wire = MultisigMachine.decode(VenueBlockWire.self, from: block) {
                let line = wire.text(loc)
                if !line.isEmpty { return line }
            }
            return t("proposal.reason.unknown")
        case "page_declined": return loc.t(TrustedSignerNotice.closed.key)
        case "page_sign_failed", "page_approval":
            switch object["page"] as? String ?? object["failure"] as? String {
            case "unanswered": return loc.t(TrustedSignerNotice.timeout.key)
            case "mismatch": return loc.t(TrustedSignerNotice.mismatch.key)
            case "not_opened": return loc.t(TrustedSignerNotice.unavailable.key)
            default: return loc.t(TrustedSignerNotice.refused.key)
            }
        case "not_sent": return t("proposal.notSent")
        case "cannot_pay":
            return t("proposal.failed", ["reason": t("status.nobodyPays")])
        default:
            let message = (object["message"] as? String) ?? ""
            return message.isEmpty ? t("proposal.reason.unknown") : t("proposal.failed", ["reason": message])
        }
    }

    // MARK: - Authority findings (§3.5)

    /// One finding (`{from: 'genesis', chainId, reason}` or `{from: 'chain',
    /// chainId, risk}`), in words.
    func finding(_ finding: MsJSON) -> String {
        let chain = finding["chainId"]?.int.map(network) ?? ""
        if finding["from"]?.string == "chain", let risk = finding["risk"] {
            let address = risk["address"]?.string ?? ""
            switch risk.kind {
            case "module": return t("authority.riskModule", ["network": chain, "address": address])
            case "guard": return t("authority.riskGuard", ["network": chain, "address": address])
            case "handler": return t("authority.riskHandler", ["network": chain, "address": address])
            default: return t("authority.riskSingleton", ["network": chain, "address": address])
            }
        }
        let reason = finding["reason"] ?? finding
        let address = reason["address"]?.string ?? reason["receiver"]?.string ?? ""
        switch reason.kind {
        case "module": return t("authority.module", ["address": address])
        case "handler": return t("authority.handler", ["address": address])
        case "missingCode": return t("authority.missingCode", ["network": chain, "address": address])
        case "wrongCode": return t("authority.wrongCode", ["network": chain, "address": address])
        case "unknownFactory": return t("authority.unknownFactory", ["address": address])
        case "unknownSingleton": return t("authority.unknownSingleton", ["address": address])
        case "setupCode": return t("authority.setupCode", ["address": address])
        case "payment": return t("authority.payment", ["address": address])
        case "wrongAddress": return t("authority.wrongAddress")
        case "notSetup": return t("authority.notSetup")
        case "badMembers": return t("authority.badMembers")
        default: return t("authority.unverified", ["network": chain])
        }
    }
}
