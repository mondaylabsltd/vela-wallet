//
//  MultisigReading.swift
//  VelaWallet
//
//  What the transaction's calls do, as the core read them (r8-1, r8-2,
//  r8-5): `MultisigReview.reading` — every call and every leg of every
//  batch at every level, each with its intent (a send, an allowance and
//  whether it is unlimited, an approval of a whole collection, a call nobody
//  reads, a batch too deep to read), its amounts with the decimals the core
//  read or none ("decimals unknown": never an amount worded with 18
//  assumed) — and the lines the core's concerns about them say, each once.
//  Drawn in the signing sheet's own blocks, in the corpus's
//  `home.multisig.review.reading.*` words: the web's `reading-blocks.ts`,
//  never a decoding of its own (the owner's Bybit lesson).
//

import SwiftUI

enum MsReading {

    @MainActor
    private static func r(_ key: String, _ words: MsWords, _ vars: [String: String] = [:]) -> String {
        words.t("review.reading.\(key)", vars)
    }

    private static func short(_ address: String) -> String { MsWords.short(address) }

    // MARK: - Words for amounts and tokens

    /// A token as said: its symbol, else its contract (never a made-up name);
    /// the network's coin for `token: null`.
    @MainActor
    static func tokenName(_ token: MsJSON?, chainId: Int, words: MsWords) -> String {
        guard let address = token?["token"]?.string else { return words.symbol(chainId) }
        return token?["symbol"]?.string ?? short(address)
    }

    /// An amount as the core read it: unlimited is "Unlimited"; unknown
    /// decimals are the raw integer, "decimals unknown" — never a friendly
    /// amount in 18 decimals (r8-2).
    @MainActor
    static func amountText(_ amount: MsJSON?, token: MsJSON?, chainId: Int, words: MsWords) -> String {
        if amount?["unlimited"]?.bool == true { return r("unlimited", words) }
        let name = tokenName(token, chainId: chainId, words: words)
        let raw = amount?["raw"]?.string ?? "0"
        guard let decimals = amount?["decimals"]?.int else {
            return "\(r("decimalsUnknown", words, ["raw": raw])) \(name)".trimmingCharacters(in: .whitespaces)
        }
        return "\(MsRules.formatUnits(raw, decimals: decimals)) \(name)".trimmingCharacters(in: .whitespaces)
    }

    // MARK: - Intents

    /// What a call is, in one or two words.
    @MainActor
    static func intentTitle(_ intent: MsJSON, words: MsWords) -> String {
        switch intent.kind {
        case "send": return r("send", words)
        case "sendFrom": return r("sendFrom", words)
        case "approve":
            let grant = intent["grant"]?.string ?? "approve"
            return ["approve", "increase", "decrease", "permit", "daiPermit", "permit2"].contains(grant)
                ? r(grant, words) : r("approve", words)
        case "approveAll": return r(intent["granted"]?.bool == true ? "approveAll" : "revokeAll", words)
        case "batch":
            let count = intent["calls"]?.array.count ?? 0
            return count == 1 ? r("batchOne", words) : r("batchMany", words, ["total": String(count)])
        case "unreadable": return r("unreadable", words)
        case "members": return r("members", words)
        case "rename": return r("rename", words, ["name": intent["name"]?.string ?? ""])
        default: return r("call", words)
        }
    }

    /// A call's tone: danger for what grants it all or cannot be read.
    static func intentTone(_ intent: MsJSON) -> SigningTone {
        switch intent.kind {
        case "approve":
            return intent["amount"]?["unlimited"]?.bool == true && intent["grant"]?.string != "decrease" ? .danger : .neutral
        case "approveAll": return intent["granted"]?.bool == true ? .danger : .neutral
        case "unreadable", "batch": return .danger
        case "call": return .caution
        default: return .neutral
        }
    }

    /// The rows of one call, each fact the core read.
    @MainActor
    private static func callRows(_ call: MsJSON, chainId: Int, words: MsWords) -> [SigningRow] {
        let i = call["intent"] ?? .null
        var rows: [SigningRow] = []
        let eip = { (key: String) in MsRules.eip55(i[key]?.string ?? "") }
        switch i.kind {
        case "send":
            rows.append(SigningRow(label: r("amount", words), value: amountText(i["amount"], token: i["token"], chainId: chainId, words: words)))
            rows.append(SigningRow(label: r("to", words), value: eip("to"), mono: true))
        case "sendFrom":
            rows.append(SigningRow(label: r("amount", words), value: amountText(i["amount"], token: i["token"], chainId: chainId, words: words)))
            rows.append(SigningRow(label: r("from", words), value: eip("from"), mono: true))
            rows.append(SigningRow(label: r("to", words), value: eip("to"), mono: true))
        case "approve":
            let unlimited = i["amount"]?["unlimited"]?.bool == true && i["grant"]?.string != "decrease"
            rows.append(SigningRow(label: r("token", words), value: tokenName(i["token"], chainId: chainId, words: words)))
            rows.append(SigningRow(
                label: r("allowance", words),
                value: i["grant"]?.string == "daiPermit" && unlimited
                    ? r("allOfIt", words) : amountText(i["amount"], token: i["token"], chainId: chainId, words: words),
                valueTone: unlimited ? .danger : .neutral
            ))
            rows.append(SigningRow(label: r("spender", words), value: eip("spender"), mono: true))
        case "approveAll":
            rows.append(SigningRow(label: r("collection", words), value: eip("collection"), mono: true))
            rows.append(SigningRow(label: r("operator", words), value: eip("operator"),
                                   valueTone: i["granted"]?.bool == true ? .danger : .neutral, mono: true))
        case "batch":
            rows.append(SigningRow(label: r("runner", words), value: eip("runner"), mono: true))
        case "call":
            if let selector = i["selector"]?.string { rows.append(SigningRow(label: r("function", words), value: selector, mono: true)) }
        default:
            break
        }
        // The coin a call sends along (a send says its own).
        if i.kind != "send", i.kind != "approveAll", let raw = call["value"]?["raw"]?.string, raw != "0", !raw.isEmpty {
            rows.append(SigningRow(label: r("alsoSends", words),
                                   value: amountText(call["value"], token: nil, chainId: chainId, words: words)))
        }
        return rows
    }

    // MARK: - Blocks

    /// Each call as a card, a batch's legs after it, every level (r8-5).
    @MainActor
    private static func cards(_ calls: [MsJSON], chainId: Int, words: MsWords) -> [SigningBlock] {
        calls.flatMap { call -> [SigningBlock] in
            let intent = call["intent"] ?? .null
            let path = (call["path"]?.array ?? []).compactMap(\.int).map(String.init).joined(separator: ".")
            let card = SigningBlock.card(
                title: "\(words.t("review.callNumber", ["n": path])) · \(intentTitle(intent, words: words))",
                rows: callRows(call, chainId: chainId, words: words),
                tone: intentTone(intent)
            )
            guard intent.kind == "batch" else { return [card] }
            return [card, .source(text: r("runnerNote", words), tone: .caution)]
                + cards(intent["calls"]?.array ?? [], chainId: chainId, words: words)
        }
    }

    /// The reading's blocks: the lone call as the signing sheet draws one
    /// (its verb, its amount or allowance, who it reaches), or every call of
    /// a batch as a card. Nothing for what the multisig reads as its own (a
    /// member change, a rename, a cancellation): those have their own drawing.
    @MainActor
    static func blocks(_ review: MsJSON, chainId: Int, words: MsWords) -> [SigningBlock] {
        let calls = review["reading"]?["calls"]?.array ?? []
        guard let only = calls.first else { return [] }
        let lone = calls.count == 1 && (only["path"]?.array.isEmpty ?? true) && only["intent"]?.kind != "batch"
        guard lone, let i = only["intent"] else { return cards(calls, chainId: chainId, words: words) }
        var out: [SigningBlock] = [.intent(text: intentTitle(i, words: words), tone: intentTone(i))]
        // Who it reaches, once (r5reg-15): the name this device or the review
        // knows, the full address under it, and the review's own cue for it
        // ("First time sending here") — the recipient's row is not repeated.
        let party = { (label: String, key: String) -> SigningBlock in
            let address = MsRules.eip55(i[key]?.string ?? "")
            let recipient = (review["recipients"]?.array ?? [])
                .first { $0["address"]?.string?.lowercased() == address.lowercased() }
            let name = words.localName(address) ?? words.keptName(address) ?? recipient?["known"]?["label"]?.string
            return .party(label: r(label, words), name: name ?? short(address), address: address,
                          badge: recipient.flatMap { badge($0, words: words) })
        }
        switch i.kind {
        case "send":
            out.append(.rows([SigningRow(label: r("amount", words), value: amountText(i["amount"], token: i["token"], chainId: chainId, words: words))]))
            out.append(party("to", "to"))
        case "approve", "sendFrom":
            out.append(.rows(callRows(only, chainId: chainId, words: words).filter { !$0.mono }))
            if i.kind == "approve" {
                out.append(party("spender", "spender"))
            } else {
                out.append(party("from", "from"))
                out.append(party("to", "to"))
            }
        default:
            let rows = callRows(only, chainId: chainId, words: words)
            if !rows.isEmpty { out.append(.rows(rows)) }
        }
        return out
    }

    /// What the review knows of a recipient, as its row's badge: the core's
    /// cues (first time, a contract), else whose it is (the web's
    /// `recipientBadge`).
    @MainActor
    static func badge(_ recipient: MsJSON, words: MsWords) -> PartyBadge? {
        var tags: [String] = []
        if recipient["firstTimeCue"]?.bool == true { tags.append(words.loc.t("componentsUi.signing.firstTimeTag")) }
        if recipient["contractCue"]?.bool == true { tags.append(words.loc.t("componentsUi.signing.contractTag")) }
        if !tags.isEmpty {
            let lookalike = recipient["lookalike"].map { !$0.isNull } ?? false
            return PartyBadge(text: tags.joined(separator: " · "), tone: lookalike ? .danger : .caution)
        }
        switch recipient["known"]?["kind"]?.string {
        case "own": return PartyBadge(text: words.t("review.knownOwn"), tone: .neutral)
        case "member" where recipient["role"]?.string != "member": return PartyBadge(text: words.t("review.knownMember"), tone: .neutral)
        default: return nil
        }
    }

    /// The addresses the reading names as a party already (lowercased): the
    /// review's recipient rows do not say them again (r5reg-15).
    static func partyAddresses(_ blocks: [SigningBlock]) -> Set<String> {
        Set(blocks.compactMap { if case .party(_, _, let address?, _) = $0 { return address.lowercased() } else { return nil } })
    }

    // MARK: - The concerns about the calls

    /// The lines the core's concerns about the calls say (r8-1, r8-2, r8-5),
    /// each once (the core deduped them, r8-14) and drawn above the reading:
    /// an unlimited grant, a nested batch, one that cannot be read, decimals
    /// unknown, tokens sent to their own contract, a call nobody reads.
    @MainActor
    static func concernBlocks(_ review: MsJSON, words: MsWords) -> [SigningBlock] {
        var out: [SigningBlock] = []
        var said: Set<String> = []
        for concern in review["concerns"]?.array ?? [] {
            guard let line = concernLine(concern, review: review, words: words), !said.contains(line.text) else { continue }
            said.insert(line.text)
            out.append(.warning(tone: line.tone, text: line.text))
        }
        return out
    }

    /// The concern kinds these lines say — never a second line elsewhere.
    static let readingConcerns: Set<String> = [
        "unlimitedApproval", "nestedBatch", "unreadableBatch", "decimalsUnknown", "toOwnToken", "blind",
    ]

    @MainActor
    static func concernLine(_ concern: MsJSON, review: MsJSON?, words: MsWords) -> (text: String, tone: SigningTone)? {
        func named(_ address: String) -> String {
            findToken(review?["reading"]?["calls"]?.array ?? [], address)?["symbol"]?.string ?? short(address)
        }
        switch concern.kind {
        case "unlimitedApproval":
            return (r("unlimitedLine", words, ["spender": MsRules.eip55(concern["spender"]?.string ?? "")]), .danger)
        case "nestedBatch": return (r("nestedLine", words), .danger)
        case "unreadableBatch": return (r("unreadableLine", words), .danger)
        case "decimalsUnknown": return (r("decimalsLine", words, ["token": named(concern["token"]?.string ?? "")]), .caution)
        case "toOwnToken": return (r("ownTokenLine", words, ["token": named(concern["token"]?.string ?? "")]), .danger)
        case "blind": return (r("blindLine", words), .caution)
        default: return nil
        }
    }

    private static func findToken(_ calls: [MsJSON], _ address: String) -> MsJSON? {
        for call in calls {
            let i = call["intent"] ?? .null
            if ["send", "sendFrom", "approve"].contains(i.kind ?? ""),
               let token = i["token"], token["token"]?.string?.lowercased() == address.lowercased() {
                return token
            }
            if i.kind == "batch", let found = findToken(i["calls"]?.array ?? [], address) { return found }
        }
        return nil
    }
}

/// The reading's blocks, drawn with the signing sheet's own atoms.
struct MsReadingView: View {
    let blocks: [SigningBlock]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                switch block {
                case .intent(let text, let tone):
                    SigningIntentLabel(text: text, tone: tone)
                case .card(let title, let rows, let tone):
                    SigningDetailCard(title: title, rows: rows, tone: tone)
                case .warning(let tone, let text):
                    SigningWarning(tone: tone, text: text)
                case .rows(let rows):
                    SigningRowsView(rows: rows)
                case .party(let label, let name, let address, let badge):
                    SigningPartyRow(label: label, name: name, address: address, badge: badge)
                case .source(let text, let tone):
                    SigningSourceLine(text: text, tone: tone)
                case .sentence(let text, let tone):
                    SigningSentence(text: text, tone: tone)
                default:
                    EmptyView()
                }
            }
        }
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("multisig.review.reading")
    }
}
