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
    /// r9-12: the core's names (`member_labels`) when given; else the hub's.
    var labels: [String: String]? = nil

    func t(_ key: String, _ vars: [String: String] = [:]) -> String {
        loc.t("home.multisig.\(key)", vars: vars)
    }

    func t(_ key: String, count: Int, _ vars: [String: String] = [:]) -> String {
        loc.t("home.multisig.\(key)", count: count, vars: vars)
    }

    // MARK: - Names

    /// "0xd9c79C…a20432": the core's one short form (r11-6, `names.short`:
    /// 6+6, EIP-55 — the same on every platform; this drew 4+4).
    static func short(_ address: String) -> String {
        MsRules.string("names.short", ["address": address]) ?? AddressText.short(MsRules.eip55(address))
    }

    /// A member's name on this device, or `nil` when it is not one of its accounts.
    func localName(_ address: String) -> String? {
        localNames[address.lowercased()]
    }

    /// What the multisigs here keep of a member (a label, never authority).
    func keptName(_ address: String) -> String? {
        hub?.accountsView.knownMembers[address.lowercased()]?.name.flatMap { $0.isEmpty ? nil : $0 }
    }

    /// r9-12: the core's ONE name for a member (`member_labels`: kept, then
    /// the passkey index, then this device's own name, then the short
    /// address) — the same on every device.
    /// A member with no name at all is labelled by the core with its own
    /// short address (`names.short`): that is no name — the row draws the
    /// address as it always does, not the address twice.
    func coreLabel(_ address: String) -> String? {
        guard let label = (labels ?? hub?.accountsView.memberLabels)?[address.lowercased()], !label.isEmpty,
              label != MsRules.string("names.short", ["address": address])
        else { return nil }
        return label
    }

    /// A member as a line names it: its name and short address, never the
    /// name alone — a name is a claim, the address is who gets the say.
    func who(_ address: String) -> String {
        // r11-6: the core's `names.who` over the one label.
        let name = coreLabel(address) ?? localName(address) ?? keptName(address)
        var args: [String: Any] = ["address": address]
        if let name { args["label"] = name }
        if let said = MsRules.string("names.who", args) { return said }
        let short = Self.short(address)
        guard let name, name != short else { return short }
        return "\(name) (\(short))"
    }

    /// A member's label: the core's (r9-12); before the core has named it
    /// (no hub, a member it does not know yet), this device's name, else
    /// the kept one, else short.
    func label(_ address: String) -> String {
        coreLabel(address) ?? localName(address) ?? keptName(address) ?? Self.short(address)
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

    /// r9-14: a `MultisigRuleChip` as its chip says it — "Needs 2 of 4",
    /// "One member", or that the rule differs by network — never the
    /// record's founding numbers when the core gave one.
    func chipText(_ chip: MsJSON?, fallback account: MsAccount) -> String {
        guard let chip, !chip.isNull else { return ruleChip(m: account.threshold, n: account.owners.count) }
        if chip.kind == "differs" { return t("presence.ruleDiffers") }
        let rule = chip["rule"]
        return ruleChip(m: rule?["m"]?.int ?? account.threshold, n: rule?["n"]?.int ?? account.owners.count)
    }

    /// The same, as a title ("Needs 2 of 4 members").
    func chipTitle(_ chip: MsJSON?, fallback account: MsAccount) -> String {
        guard let chip, !chip.isNull else { return ruleTitle(m: account.threshold, n: account.owners.count) }
        if chip.kind == "differs" { return t("presence.ruleDiffers") }
        let rule = chip["rule"]
        return ruleTitle(m: rule?["m"]?.int ?? account.threshold, n: rule?["n"]?.int ?? account.owners.count)
    }

    /// A row's rule (`AccountRow.rule`): its chip, and "last known" when a
    /// chain said it but no read this session did.
    func rowRule(_ row: MsAccountRow) -> (chip: String, title: String, lastKnown: String?) {
        let chip = row.rule?["chip"]
        let lastKnown = row.rule?["last_known"]?.bool == true && row.rule?["from_chain"]?.bool == true
        return (chipText(chip, fallback: row.account), chipTitle(chip, fallback: row.account),
                lastKnown ? t("presence.ruleLastKnown") : nil)
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

    // MARK: - Amounts (r12-1: never a token's decimals assumed)

    /// The network's own coin is counted in the EVM's unit, wei: 18
    /// decimals by the protocol (the desktop's `TokenInfo::coin`). This is
    /// the ONLY 18 the multisig screens know — a token's decimals are the
    /// core's or nobody's, never this figure.
    static let coinDecimals = ChainCatalog.coinDecimals

    /// A token as an amount is worded in it: its symbol and decimals, the
    /// decimals `nil` when nobody read them.
    struct TokenFacts: Equatable {
        var symbol: String
        var decimals: Int?
    }

    /// The network's coin (`token` nil or empty), else what the core read
    /// of the token (`MultisigProposalsView.tokens`) — `decimals` nil when
    /// it read none, the symbol its contract's short form then.
    func facts(chainId: Int, token: String?) -> TokenFacts {
        guard let token, !token.isEmpty else { return TokenFacts(symbol: symbol(chainId), decimals: Self.coinDecimals) }
        let core = coreToken(chainId, token)
        return TokenFacts(symbol: core?.symbol ?? Self.short(token), decimals: core?.decimals)
    }

    /// `raw` base units in `facts`: "0.5 USDC", or — decimals unknown —
    /// "500000 base units · decimals unknown USDC" (the review's own
    /// words, `review.reading.decimalsUnknown`), never a figure in 18.
    func amount(_ raw: String, _ facts: TokenFacts) -> String {
        guard let decimals = facts.decimals else {
            return "\(t("review.reading.decimalsUnknown", ["raw": raw])) \(facts.symbol)".trimmingCharacters(in: .whitespaces)
        }
        return "\(MsRules.formatUnits(raw, decimals: decimals)) \(facts.symbol)".trimmingCharacters(in: .whitespaces)
    }

    /// An amount in the network's coin (a fee, a call's value).
    func coinAmount(_ raw: String, chainId: Int) -> String {
        amount(raw, facts(chainId: chainId, token: nil))
    }

    // MARK: - A transaction's one line

    /// "Send 500 USDC", "Add Bob (0x12…34)" — the proposal's own title.
    func actionTitle(chainId: Int, action: MsJSON?, nonce: String) async -> String {
        guard let action, !action.isNull else { return t("action.unsupported") }
        // r8-2: a token's facts are the core's (`tokens`) — read before any
        // guess; one it could not read is said raw, never in 18 decimals.
        if action.kind == "transfer", action["token"]?.string == nil || coreToken(chainId, action["token"]?.string) != nil {
            return actionTitleNow(chainId: chainId, action: action, nonce: nonce)
        }
        switch action.kind {
        case "transfer":
            guard let info = await hub?.tokenInfo(chainId: chainId, token: action["token"]?.string),
                  let decimals = info.decimals,
                  info.symbol != AddressText.short(action["token"]?.string ?? "")
            else { return actionTitleNow(chainId: chainId, action: action, nonce: nonce) }
            return t("action.send", [
                "amount": MsRules.formatUnits(action["amount"]?.string ?? "0", decimals: decimals),
                "symbol": info.symbol,
            ])
        default:
            return actionTitleNow(chainId: chainId, action: action, nonce: nonce)
        }
    }

    /// r10-8: a transaction's line — a contract call as the core reads its
    /// bytes ("Approve · USDC", "Batch of 1 call"; never only "Contract
    /// call" for one the core can say), anything else as `actionTitleNow`
    /// says it. The machine's review's reading when the sheet has one, else
    /// the core's pure reading of the bytes (`review.reading`, the desktop's
    /// `ms_reading_title`) with the token facts the proposals machine read.
    func title(_ item: MsProposalItem, safe: String? = nil, review: MsJSON? = nil) -> String {
        if item.action?.kind == "call" || item.action == nil,
           let reading = review?["reading"] ?? callReading(item, safe: safe),
           MsReading.saysMoreThanACall(reading),
           let title = MsReading.title(reading, chainId: item.chainId, words: self) {
            return title
        }
        return actionTitleNow(chainId: item.chainId, action: item.action, nonce: item.nonce)
    }

    /// The core's reading of a call's bytes, `nil` when it reads none (or
    /// there is no multisig to read it for). Kept per transaction and token
    /// facts: a row is drawn many times and the bytes never change.
    func callReading(_ item: MsProposalItem, safe: String?) -> MsJSON? {
        guard let safe = safe ?? hub?.proposalsView.followed, !safe.isEmpty, !item.tx.isNull else { return nil }
        let prefix = "\(item.chainId):"
        let facts = (hub?.proposalsView.tokens ?? [:])
            .filter { $0.key.hasPrefix(prefix) }
            .sorted { $0.key < $1.key }
            .map(\.value)
        // The bytes themselves are in the key, not only the hash a row
        // names: a hash is the machine's word for them, the reading is of them.
        let bytes = "\(item.tx["to"]?.string ?? ""):\(item.tx["value"]?.string ?? ""):\(item.tx["operation"]?.int ?? 0):\(item.tx["data"]?.string ?? "")"
        let key = "\(item.chainId):\(safe.lowercased()):\(item.safeTxHash.lowercased()):\(bytes):\(facts.map { "\($0["symbol"]?.string ?? "")/\($0["decimals"]?.int ?? -1)" })"
        if let kept = Self.readings[key] { return kept }
        let answer = MsRules.call("review.reading", [
            "chainId": item.chainId, "safe": safe, "tx": item.tx.any, "tokenFacts": facts.map(\.any),
        ]) as? [String: Any]
        let reading = answer.map { MsJSON(any: $0["reading"]) }
        if Self.readings.count > 256 { Self.readings.removeAll() }
        Self.readings[key] = reading
        return reading
    }

    /// `callReading`'s answers, by transaction and token facts.
    private static var readings: [String: MsJSON?] = [:]

    /// The core's facts for a token the proposals shown move (r8-2,
    /// `MultisigProposalsView.tokens`): symbol and decimals, either `nil`
    /// when unread.
    func coreToken(_ chainId: Int, _ token: String?) -> (symbol: String?, decimals: Int?)? {
        guard let token, let facts = hub?.proposalsView.tokens["\(chainId):\(token.lowercased())"] else { return nil }
        return (facts["symbol"]?.string, facts["decimals"]?.int)
    }

    /// The same, without reading a token (a transfer of the chain's coin, or
    /// of a token whose symbol is not read yet, says the coin it knows).
    func actionTitleNow(chainId: Int, action: MsJSON?, nonce: String, token: (symbol: String, decimals: Int)? = nil) -> String {
        guard let action, !action.isNull else { return t("action.unsupported") }
        switch action.kind {
        case "transfer":
            let amount = action["amount"]?.string ?? "0"
            guard let address = action["token"]?.string else {
                return t("action.send", ["amount": MsRules.formatUnits(amount, decimals: Self.coinDecimals), "symbol": symbol(chainId)])
                    .trimmingCharacters(in: .whitespaces)
            }
            if let token {
                return t("action.send", ["amount": MsRules.formatUnits(amount, decimals: token.decimals), "symbol": token.symbol])
                    .trimmingCharacters(in: .whitespaces)
            }
            // r8-2: the core's facts; decimals unread — the base units, said
            // so, never a figure in 18 decimals ("Send <0.000001").
            let facts = coreToken(chainId, address)
            let name = facts?.symbol ?? Self.short(address)
            guard let decimals = facts?.decimals else {
                return t("action.send", ["amount": t("review.reading.decimalsUnknown", ["raw": amount]), "symbol": name])
                    .trimmingCharacters(in: .whitespaces)
            }
            return t("action.send", ["amount": MsRules.formatUnits(amount, decimals: decimals), "symbol": name])
                .trimmingCharacters(in: .whitespaces)
        case "batch":
            let calls = action["calls"]?.array ?? []
            let allTransfers = calls.allSatisfy { $0["action"]?.kind == "transfer" }
            return t(allTransfers ? "action.batch" : "action.batchCalls", ["total": String(calls.count)])
        case "cancel":
            return t("action.cancel", ["nonce": nonce])
        case "addOwner", "removeOwner", "swapOwner", "changeThreshold":
            // r12-8: the core's one title rule (`names.change`), the same on
            // a sheet and on an Activity row.
            if let title = memberChangeTitle(action) { return title }
            switch action.kind {
            case "addOwner":
                return t("change.add", ["member": who(action["owner"]?.string ?? "")])
            case "removeOwner":
                return t("change.remove", ["member": who(action["owner"]?.string ?? "")])
            case "swapOwner":
                return t("change.replace", [
                    "old": who(action["oldOwner"]?.string ?? ""), "new": who(action["newOwner"]?.string ?? ""),
                ])
            default:
                // "Require 1 approval" — the corpus's plural family (r9).
                let threshold = action["threshold"]?.string ?? ""
                return t("action.changeThreshold", count: Int(threshold) ?? 0, ["threshold": threshold, "count": threshold])
            }
        case "rename":
            return t("action.rename", ["name": action["name"]?.string ?? ""])
        case "call":
            let value = action["value"]?.string ?? "0"
            if value == "0" || value.isEmpty { return t("action.call") }
            return t("action.callWithValue", [
                "amount": MsRules.formatUnits(value, decimals: Self.coinDecimals), "symbol": symbol(chainId),
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

    /// r12-8: a member change's title by the core's one rule (link door
    /// `names.change {action, labels}` → `{key, params, count?}`), each
    /// member named by `names.who` with the core's labels (`member_labels`),
    /// worded in this page's language when it is drawn. `nil`: not a member
    /// change, or a core without the rule.
    func memberChangeTitle(_ action: MsJSON?) -> String? {
        guard let action, !action.isNull else { return nil }
        // The core's labels (`member_labels`: kept, index, this device's own
        // names); this device's own names only where it has none yet.
        let labels = localNames.merging((labels ?? hub?.accountsView.memberLabels) ?? [:]) { _, core in core }
        guard let answer = MsRules.call("names.change", ["action": action.any, "labels": labels]) as? [String: Any],
              let key = answer["key"] as? String
        else { return nil }
        let params = (answer["params"] as? [String: Any] ?? [:]).mapValues { "\($0)" }
        let said: String
        if let count = answer["count"].flatMap({ ($0 as? String).flatMap(Int.init) ?? ($0 as? NSNumber)?.intValue }) {
            said = loc.t(key, count: count, vars: params)
        } else {
            said = loc.t(key, vars: params)
        }
        return said == key ? nil : said
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
            return t("status.cannotRun", ["reason": blockReason(standing?["block"], chainId: item.chainId)])
        default:
            return t("status.progress", ["approved": String(item.approved), "threshold": String(item.threshold)])
        }
    }

    /// Why a ready transaction cannot run now (`MultisigReadyBlock`).
    func blockReason(_ block: MsJSON?, chainId: Int) -> String {
        guard let block else { return "" }
        if block.kind == "nobodyPays" { return t("status.nobodyPays") }
        return whyFails(block["why"], chainId: chainId)
    }

    /// `short` (token, have, need) or `reverts` (reason).
    ///
    /// r12-1: the figures are the core's — its token facts for the shortfall
    /// (on the reason itself, else the proposals view's `tokens`), never 18
    /// decimals assumed: "the multisig holds 0, and this needs <0.000001"
    /// for a 0.01 USDC send was a 6-decimals token read in 18. Decimals
    /// nobody read are said as base units, "decimals unknown".
    func whyFails(_ why: MsJSON?, chainId: Int) -> String {
        guard let why else { return t("proposal.uncheckedReason") }
        switch why.kind {
        case "short":
            let facts = shortFacts(why, chainId: chainId)
            func amount(_ key: String) -> String {
                let value = why[key]
                // The core's amount (`MultisigAmount`, r12-1): its `said` is
                // the words — "0.01 USDC", or the base units with "decimals
                // unknown" — decimals and symbol resolved by the core.
                if let said = Self.said(value?["said"], loc: loc) { return said }
                // An older core: base units, a string, in the facts it gave.
                let raw = value?["base_units"]?.string ?? value?["raw"]?.string ?? value?.string ?? "0"
                var own = facts
                if let decimals = value?["decimals"]?.int { own.decimals = decimals }
                return self.amount(raw, own)
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

    /// A `MultisigAmount.said` in words: `known` its `text`; a token whose
    /// decimals nobody could read, the corpus's `key` with `{{raw}}`; `nil`
    /// for no `said` at all.
    static func said(_ said: MsJSON?, loc: Loc) -> String? {
        guard let said, !said.isNull else { return nil }
        switch said.kind {
        case "known":
            return said["text"]?.string
        case "decimals_unknown":
            let key = said["key"]?.string ?? "home.multisig.review.reading.decimalsUnknown"
            let text = loc.t(key, vars: ["raw": said["raw"]?.string ?? ""])
            return text == key ? nil : text
        default:
            return nil
        }
    }

    /// The token a shortfall is in, as the core said it: `token` the address
    /// (null: the network's coin) or the token's facts `{token|address,
    /// symbol, decimals}`; a `symbol` / `decimals` on the reason itself
    /// first; else the proposals view's facts for that address.
    func shortFacts(_ why: MsJSON, chainId: Int) -> TokenFacts {
        let token = why["token"]
        let address = token?.string ?? token?["token"]?.string ?? token?["address"]?.string
        var facts = facts(chainId: chainId, token: address)
        if let symbol = why["symbol"]?.string ?? token?["symbol"]?.string, !symbol.isEmpty { facts.symbol = symbol }
        if let decimals = why["decimals"]?.int ?? token?["decimals"]?.int { facts.decimals = decimals }
        return facts
    }

    // MARK: - Why nothing is signed (r9-9)

    /// The ONE refusal line: the core's own reason, by its corpus key
    /// (`MultisigReview.refused.key` — `home.multisig.review.refusedWhy.*`,
    /// or `review.blocked` for the state guard's), never a shell's guess
    /// nor the core's English `refusal`. `review.blocked` only while a
    /// review without the field is drawn.
    func refusalLine(_ review: MsJSON?) -> String {
        if let key = review?["refused"]?["key"]?.string, !key.isEmpty {
            let text = loc.t(key)
            if text != key { return text }
        }
        return t("review.blocked")
    }

    // MARK: - Why an action did not go (`MultisigActionState`)

    /// The action's failure in words; `nil` for one that says nothing
    /// (working, done, the person's own cancel).
    func actionFailure(_ state: MsTagged?, chainId: Int) -> String? {
        guard let state else { return nil }
        switch state.kind {
        case "working", "done", "cancelled", "outsideAsked": return nil
        case "mailboxUnreadable": return t("proposal.mailboxDown")
        case "unread": return unreadText(state["reason_key"]?.string, chainId: chainId)
        case "noMemberHere": return t("proposal.notMember")
        case "signingRefused": return t("authority.refusedOn", ["network": network(chainId)])
        case "signingBlocked": return t("review.blocked")
        case "signingUnchecked": return t("authority.signUnchecked", ["network": network(chainId)])
        case "riskDeclined": return nil
        // Round 16 leftover 4: the core's own refusal carries its English
        // (`reason`) — said in the catalog's words, never that text.
        case "refused": return t("review.refusedWhy.other")
        case "mailboxRefused": return t("proposal.reason.serviceRefused")
        case "stopped":
            // r11 (core): by the look's kind — it stands in the fee line's
            // place (`stoppedReplacesFee`), never beside it.
            switch state["preview"]?["kind"]?.string {
            case "unpriced": return t("proposal.feeUnpriced")
            case "unchecked": return t("proposal.feeUnchecked")
            // r14-4: a setup not checked is said as that, never as a finding.
            case "unverified": return t("authority.deployUnverified", ["network": network(chainId)])
            case "refused": return deployRefused(state["preview"]?["authority"], chainId: chainId)
            default:
                return t("proposal.wouldFail", ["reason": whyFails(state["preview"]?["why"] ?? state["preview"], chainId: chainId)])
            }
        case "noOutsideSigner":
            return t("proposal.failed", ["reason": t("proposal.noOutsideSigner")])
        case "failed": return failureText(state["detail"]?.string, chainId: chainId)
        default: return t("proposal.reason.unknown")
        }
    }

    /// r13-3: the read the core named (`home.multisig.proposal.unread.*`,
    /// the service's `mailboxDown`, or — round 16 leftover 3 — its busy
    /// words `home.multisig.service.busyNote`), `{{network}}` the network's
    /// name. Any full catalog key reads (`said`).
    func unreadText(_ key: String?, chainId: Int) -> String {
        said(key ?? "home.multisig.proposal.unread.chain", chainId: chainId)
    }

    /// Any catalog key the core hands a sheet (`reason_key`, a
    /// `review.missing` key, a failure's key) in words, `{{network}}` the
    /// network's name: the FULL key wherever it lives (round 16 leftover 3:
    /// `home.multisig.service.busyNote` is not under `proposal.`), or a leaf
    /// under `home.multisig.`. Never a key on the screen: one neither reads
    /// is "Something went wrong".
    func said(_ key: String, chainId: Int) -> String {
        let vars = ["network": network(chainId)]
        let full = loc.t(key, vars: vars)
        if full != key { return full }
        let leaf = t(key, vars)
        return leaf != "home.multisig.\(key)" ? leaf : t("proposal.reason.unknown")
    }

    /// Round 16 leftover 4: the create sheet's draft the core could not
    /// make (`draft_error` is the core's English, never drawn).
    var createDraftFailed: String { t("proposal.reason.unknown") }

    /// The created multisig the service did not record (`refused` is the
    /// core's English, never drawn): it is kept here, and Settings offers
    /// "Share it again" — the catalog's words for exactly that.
    var createRefused: String { t("settings.shareAgainNote") }

    /// r18-8 (§28, §32): the core's word for a write this test build refused
    /// before it left, and its sentence ("This test build won't write to the
    /// live service; point it at a local one.").
    static let productionWriteRefusedKind = "production_write_refused"
    /// The core's key (uniffi `production_write_refused_key`) — no copy here.
    static let productionWriteRefusedKey = VelaCore.productionWriteRefusedKey()
    /// r18-8 (§28, §32): this test build refused to write to the live
    /// service — the catalog's sentence (`common.testBuild.productionWriteRefused`),
    /// never "can't be reached", "didn't answer" or "offline".
    var productionWriteRefused: String { loc.t(Self.productionWriteRefusedKey) }

    /// The created multisig's `create.refused`, in words: the test build's
    /// refusal (`production_write_refused`) its own sentence; any other —
    /// the service answered and did not take it — "Share it again".
    func createRefused(_ refused: String?) -> String {
        refused == Self.productionWriteRefusedWord ? productionWriteRefused : createRefused
    }

    /// The core's word for a write this test build refused (§28): the
    /// accounts machine's `create.refused`, an outbox entry's `why`.
    static let productionWriteRefusedWord = "production_write_refused"

    /// Why Vela's side is not on the signing relay (`RelayStatus.why`), as
    /// the reason the relay line fills in: the relay's refusal and a pass
    /// not got in the catalog's words (the web's `relayText`), the relay
    /// unreached by its HOST (the core's `detail` for `unreachable`) — never
    /// a transport error's text. `nil`: nothing sayable as a reason (the
    /// line says Vela is reconnecting instead).
    static func relayReason(why: String?, detail: String?, words: MsWords) -> String? {
        switch why {
        case "refused": return words.t("external.relayRefused")
        case "unreachable":
            guard let host = detail, !host.isEmpty,
                  host.range(of: #"^[A-Za-z0-9.-]+(:[0-9]+)?$"#, options: .regularExpression) != nil
            else { return nil }
            return host
        default: return words.t("external.relayNoToken")
        }
    }

    /// The outside signing session's failure (`Failure`), in words. A room
    /// that could not be opened (`noRoom`) carries a transport error's text
    /// as its `reason` — never drawn: "Something went wrong", with the
    /// sheet's Try again.
    static func outsideFailure(_ failure: MsJSON?, words: MsWords, chain: String, wallet: String) -> String? {
        guard let failure else { return nil }
        switch failure.kind {
        case "connectDeclined": return words.t("external.connectDeclined")
        case "walletDeclined": return words.t("external.declined")
        case "otherChain": return words.t("external.otherChain", ["chain": chain])
        case "badSignature": return words.t("external.badSignature")
        case "stoppedWaiting": return words.t("external.stoppedWaiting")
        case "noRoom": return words.t("proposal.reason.unknown")
        default: return words.t("external.stillWaiting", ["wallet": wallet])
        }
    }

    /// A failed action's detail in words — round 16 leftover 4: by its
    /// stable KIND through the core's door (`review.failure`), never its
    /// text. The kinds the catalog says alone are their key's words
    /// (`{{network}}`: the detail's network, else `chainId`'s); the fee that
    /// moved, what the executor cannot pay and the venue's block are worded
    /// from the detail's own facts. Plain text, an unknown kind, an error's
    /// English: "Something went wrong, so nothing was sent."
    func failureText(_ detail: String?, chainId: Int? = nil) -> String {
        let said = MsRules.failure(detail)
        let object = MultisigShellFailure.read(detail) ?? [:]
        switch said.kind {
        // r14-9 (the core's rule): the fee moved since the press — nothing
        // signed; said with the new figure (its fee line's own words), so the
        // next press is on it.
        case "fee_moved":
            let chain = (object["chainId"] as? NSNumber)?.intValue ?? chainId ?? 0
            let fee = "\(MsRules.formatUnits(object["amount"] as? String ?? "0", decimals: Self.coinDecimals)) \(symbol(chain))"
            return t("proposal.failed", ["reason": t("proposal.feeLine", [
                "fee": fee, "name": label(object["executor"] as? String ?? ""),
            ])])
        case "cannot_pay":
            return t("proposal.failed", ["reason": t("status.nobodyPays")])
        case "venue_blocked":
            if let block = object["block"] as? [String: Any],
               let wire = MultisigMachine.decode(VenueBlockWire.self, from: block) {
                let line = wire.text(loc)
                if !line.isEmpty { return line }
            }
            return t("proposal.reason.unknown")
        case "passkey_failed":
            // Real-device S3: the core's sentence for the platform's error
            // (`passkey_error_words`) — a catalog KEY this shell put in
            // `message`, said only when the catalog has it; never text.
            if let key = object["message"] as? String, key.hasPrefix("onboarding.common.") {
                let words = loc.t(key)
                if words != key { return t("proposal.failed", ["reason": words]) }
            }
        default:
            break
        }
        guard let key = said.key else { return t("proposal.reason.unknown") }
        let chain = (object["chain_id"] as? NSNumber)?.intValue
            ?? (object["chainId"] as? NSNumber)?.intValue ?? chainId ?? 0
        return self.said(key, chainId: chain)
    }

    // MARK: - Authority findings (§3.5)

    /// r14-4: a deploy the core read and refused on `chainId` (the look's
    /// `refused`, its `authority {kind: refused, reasons}`): "Vela won't
    /// deploy this multisig on X:" and each reason once.
    func deployRefused(_ authority: MsJSON?, chainId: Int) -> String {
        var lines = [t("authority.deployRefused", ["network": network(chainId)])]
        for reason in authority?["reasons"]?.array ?? [] {
            let line = finding(.object(["chainId": .number(Double(chainId)), "reason": reason]))
            if !lines.contains(line) { lines.append(line) }
        }
        return lines.joined(separator: " ")
    }

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
        // §37 (review 1-13): a member that is a contract nothing ties to its
        // own creation on that network.
        case "ownerNotBound": return t("authority.ownerNotBound", ["network": chain, "address": address])
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
