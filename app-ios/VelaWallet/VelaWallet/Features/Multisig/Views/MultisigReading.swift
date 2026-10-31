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
//  assumed). Drawn in the signing sheet's own blocks and words — never
//  decoded here (the owner's Bybit lesson: the shell draws, the core reads).
//

import SwiftUI

enum MsReading {

    /// The reading's blocks, in order: one card per call (a batch's legs
    /// under its own card, every level), the danger and caution lines the
    /// reading carries beside the call they are of.
    @MainActor
    static func blocks(_ review: MsJSON, chainId: Int, words: MsWords) -> [SigningBlock] {
        guard let reading = review["reading"], !reading.isNull else { return [] }
        var out: [SigningBlock] = []
        for call in reading["calls"]?.array ?? [] {
            append(call, chainId: chainId, words: words, into: &out)
        }
        // Said once, beside the calls, from the core's own concerns (r8-14).
        for concern in review["concerns"]?.array ?? [] {
            if let line = concernLine(concern, words: words) { out.append(line) }
        }
        return out
    }

    /// The concern kinds the reading says itself — never a second line.
    static let readingConcerns: Set<String> = [
        "unlimitedApproval", "nestedBatch", "unreadableBatch", "decimalsUnknown", "toOwnToken",
    ]

    @MainActor
    private static func concernLine(_ concern: MsJSON, words: MsWords) -> SigningBlock? {
        guard let text = concernText(concern, words: words) else { return nil }
        return .warning(tone: concern.kind == "decimalsUnknown" ? .caution : .danger, text: text)
    }

    /// A reading's concern in words; `nil` for one its cards say alone (a
    /// nested batch: every leg is drawn, in the danger tone).
    @MainActor
    static func concernText(_ concern: MsJSON, words: MsWords) -> String? {
        switch concern.kind {
        case "unlimitedApproval": return words.loc.t("componentsUi.signing.unlimitedWarning")
        case "unreadableBatch": return words.t("action.unsupported")
        case "decimalsUnknown": return words.loc.t("componentsUi.signingApprove.decimalsUnverified")
        case "toOwnToken": return words.loc.t("componentsUi.signing.tokenToContractWarning")
        default: return nil
        }
    }

    @MainActor
    private static func append(_ call: MsJSON, chainId: Int, words: MsWords, into out: inout [SigningBlock]) {
        let s = { (key: String) in words.loc.t("componentsUi.signing.\(key)") }
        let a = { (key: String) in words.loc.t("componentsUi.signingApprove.\(key)") }
        let path = (call["path"]?.array ?? []).compactMap(\.int)
        let leg = path.isEmpty ? nil : words.t("review.callNumber", ["n": path.map(String.init).joined(separator: ".")])
        func titled(_ title: String) -> String { leg.map { "\($0) · \(title)" } ?? title }
        let intent = call["intent"] ?? .null
        var rows: [SigningRow] = []
        var tone: SigningTone = .neutral
        let title: String
        switch intent.kind {
        case "send":
            title = titled(s("intentSend"))
            rows.append(amountRow(s("labelAmount"), intent["amount"], token: intent["token"], chainId: chainId, words: words))
            rows.append(addressRow(s("labelTo"), intent["to"]?.string, words: words))
        case "sendFrom":
            title = titled(s("intentTransfer"))
            rows.append(amountRow(s("labelAmount"), intent["amount"], token: intent["token"], chainId: chainId, words: words))
            rows.append(addressRow(s("labelFrom"), intent["from"]?.string, words: words))
            rows.append(addressRow(s("labelTo"), intent["to"]?.string, words: words))
        case "approve":
            let grant = intent["grant"]?.string
            let unlimited = intent["amount"]?["unlimited"]?.bool == true && grant != "decrease"
            title = titled(grant == "permit" || grant == "daiPermit" || grant == "permit2"
                ? s("intentAuthorizeSpending")
                : (grant == "decrease" ? s("intentRevoke") : s("intentApprove")))
            rows.append(tokenRow(a("tokenLabel"), intent["token"], words: words))
            rows.append(addressRow(s("labelSpender"), intent["spender"]?.string, words: words))
            if unlimited {
                rows.append(SigningRow(label: s("labelMaxSpendingAmount"), value: s("valueUnlimited"), valueTone: .danger))
                tone = .danger
            } else {
                rows.append(amountRow(grant == "decrease" ? a("reduceBy") : s("labelMaxSpendingAmount"), intent["amount"],
                                      token: intent["token"], chainId: chainId, words: words))
            }
            if let expires = intent["expires"]?.string, let seconds = Double(expires) {
                rows.append(SigningRow(label: s("labelExpires"), value: Formats.dateTime(Date(timeIntervalSince1970: seconds))))
            }
        case "approveAll":
            let granted = intent["granted"]?.bool == true
            title = titled(granted ? s("intentApproveAllNfts") : a("revokeAccess"))
            rows.append(addressRow(a("collectionLabel"), intent["collection"]?.string, words: words))
            rows.append(addressRow(a("operatorLabel"), intent["operator"]?.string, words: words))
            if granted {
                rows.append(SigningRow(label: s("labelApproved"), value: a("allNfts"), valueTone: .danger))
                tone = .danger
            }
        case "batch":
            // A batch called inside the transaction: its own card, in the
            // danger tone (you sign the outer layer, the inner one spends),
            // then every leg under it, every level.
            let legs = intent["calls"]?.array ?? []
            title = titled(s("batchIntent"))
            rows.append(SigningRow(label: s("batchCall"), value: MsRules.eip55(intent["runner"]?.string ?? ""), mono: true))
            rows.append(SigningRow(label: s("batchIntent"),
                                   value: words.loc.t("componentsUi.signing.batchSubtitle", vars: ["count": String(legs.count)])))
            out.append(.card(title: title, rows: rows + valueRows(call, chainId: chainId, words: words), tone: .danger))
            for inner in legs { append(inner, chainId: chainId, words: words, into: &out) }
            return
        case "unreadable":
            // Too deep, cut short, or run as delegatecall: not read, and
            // nothing is signed (the core's rule).
            title = titled(s("batchCall"))
            rows.append(addressRow(s("labelTo"), call["to"]?.string, words: words))
            out.append(.card(title: title, rows: rows, tone: .danger))
            return
        case "members":
            // The member change's own block (`memberChanges`) says it.
            return
        case "rename":
            title = titled(words.t("action.rename", ["name": intent["name"]?.string ?? ""]))
        default:
            // A call the core cannot read: what it calls, nothing guessed.
            title = titled(s("batchCall"))
            rows.append(addressRow(s("labelTo"), call["to"]?.string, words: words))
            if let selector = intent["selector"]?.string {
                rows.append(SigningRow(label: s("techFunction"), value: selector, mono: true))
            }
            tone = .caution
        }
        out.append(.card(title: title, rows: rows + valueRows(call, chainId: chainId, words: words), tone: tone))
    }

    /// The coin a call sends along, when it sends any.
    @MainActor
    private static func valueRows(_ call: MsJSON, chainId: Int, words: MsWords) -> [SigningRow] {
        let value = call["value"]
        guard let raw = value?["raw"]?.string, raw != "0", !raw.isEmpty else { return [] }
        // A send of the coin itself already says its amount.
        if call["intent"]?.kind == "send", call["intent"]?["token"]?["token"]?.isNull ?? true { return [] }
        return [amountRow(words.loc.t("componentsUi.signing.valueLabel"), value, token: nil, chainId: chainId, words: words)]
    }

    /// An amount as the core read it: its exact decimal with its symbol;
    /// with no decimals read, the base units and that the decimals are
    /// unknown — never a friendly figure.
    @MainActor
    static func amountText(_ amount: MsJSON?, token: MsJSON?, chainId: Int, words: MsWords) -> (text: String, tone: SigningTone) {
        let symbol: String = {
            guard let token, !token.isNull, let address = token["token"]?.string else { return words.symbol(chainId) }
            return token["symbol"]?.string ?? MsWords.short(address)
        }()
        if amount?["unlimited"]?.bool == true {
            return (words.loc.t("componentsUi.signing.valueUnlimited"), .danger)
        }
        if let plain = amount?["plain"]?.string {
            return ("\(plain) \(symbol)", .neutral)
        }
        // No decimals read: the base units as signed, said as an amount
        // nobody could word (the reading's `decimalsUnknown` line says why).
        let raw = amount?["raw"]?.string ?? "0"
        return ("\(words.loc.t("componentsUi.signing.amountUnknown")) · \(raw) · \(symbol)", .caution)
    }

    @MainActor
    private static func amountRow(_ label: String, _ amount: MsJSON?, token: MsJSON?, chainId: Int, words: MsWords) -> SigningRow {
        let said = amountText(amount, token: token, chainId: chainId, words: words)
        return SigningRow(label: label, value: said.text, valueTone: said.tone)
    }

    @MainActor
    private static func tokenRow(_ label: String, _ token: MsJSON?, words: MsWords) -> SigningRow {
        let address = token?["token"]?.string ?? ""
        let symbol = token?["symbol"]?.string
        return SigningRow(label: label, value: symbol.map { "\($0) · \(MsWords.short(address))" } ?? MsRules.eip55(address),
                          mono: symbol == nil)
    }

    @MainActor
    private static func addressRow(_ label: String, _ address: String?, words: MsWords) -> SigningRow {
        let address = address ?? ""
        let name = words.localName(address) ?? words.keptName(address)
        return SigningRow(label: label, value: name.map { "\($0) · \(MsWords.short(address))" } ?? MsRules.eip55(address),
                          mono: name == nil)
    }
}

/// The reading's blocks, drawn with the signing sheet's own atoms.
struct MsReadingView: View {
    let blocks: [SigningBlock]

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            ForEach(Array(blocks.enumerated()), id: \.offset) { _, block in
                switch block {
                case .card(let title, let rows, let tone):
                    SigningDetailCard(title: title, rows: rows, tone: tone)
                case .warning(let tone, let text):
                    SigningWarning(tone: tone, text: text)
                case .rows(let rows):
                    SigningRowsView(rows: rows)
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
