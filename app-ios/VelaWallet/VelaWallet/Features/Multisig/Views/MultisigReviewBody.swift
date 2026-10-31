//
//  MultisigReviewBody.swift
//  VelaWallet
//
//  The one review every signer signs from (§3.6, the owner's Bybit lesson):
//  the core's `MultisigReview` — what moves, who it reaches, the state-diff
//  guard, the member changes and the rule they leave, every concern, the
//  SafeTx fields and the three hashes — drawn, never decoded again here.
//  The web's `ReviewBody.svelte` / `review-blocks.ts`, in this app's blocks.
//

import SwiftUI

struct MsReviewBody: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let review: MsJSON
    let chainId: Int
    let words: MsWords
    /// A finished transaction: nothing is re-checked, only what was signed
    /// (its fields and hashes) is drawn — the web's `past`.
    var past = false
    /// A token's symbol and decimals, read once for the moves.
    @State private var tokens: [String: (symbol: String, decimals: Int)] = [:]
    @State private var techOpen = false

    private var network: String { words.network(chainId) }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if !past {
                concerns
                // What its calls do, every leg — the core's reading, under
                // the lines its concerns say (the web's order).
                let reading = MsReading.concernBlocks(review, words: words)
                    + MsReading.blocks(review, chainId: chainId, words: words)
                if !reading.isEmpty { MsReadingView(blocks: reading) }
                stateGuard
                recipients
                memberChanges
                moves
            }
            technical
        }
        .task(id: tokenAddresses) { await readTokens() }
    }

    // MARK: - Concerns

    /// Each thing the review says, said ONCE (r8-14): the block that carries
    /// it says it — the verdict's place a simulation that could not run or
    /// would revert, a recipient's row its first time, its contract and its
    /// look-alike, the guard its own verdict, the reading's lines what the
    /// calls do (`MsReading.concernBlocks`); a refusal is the sheet's own
    /// note. What no block carries is drawn here: nothing is signed.
    @ViewBuilder private var concerns: some View {
        if review["sign"]?["allowed"]?.bool == false {
            MsNote(text: words.t("review.blocked"), tone: .danger, identifier: "multisig.review.blocked")
        }
    }

    /// A concern in words — every kind, for the "… anyway" step, which
    /// names what the person is signing past.
    static func concernText(_ concern: MsJSON, review: MsJSON? = nil, words: MsWords) -> String? {
        switch concern.kind {
        case "notSigned":
            return words.t("proposal.refused", ["reason": concern["why"]?.string ?? ""])
        case "stateChange":
            return words.t("review.stateTitle")
        case "stateUnread":
            return nil
        case "lookalike":
            return words.t("review.lookalike", [
                "address": concern["address"]?.string ?? "",
                "label": concern["label"]?.string ?? "",
                "known": concern["of"]?.string ?? "",
            ])
        case "toContract":
            return words.t("review.toContract", ["address": concern["address"]?.string ?? ""])
        case "firstTime":
            return words.loc.t("componentsUi.signing.firstTimeTag") + " · " + MsWords.short(concern["address"]?.string ?? "")
        case "reverts":
            return words.loc.t("componentsUi.signing.simWillFail")
        case "unsimulated":
            return words.loc.t("componentsUi.signing.simUnavailableWarning")
        default:
            // The reading's own (r8-1, r8-2, r8-5), a call nobody reads too.
            return MsReading.concernLine(concern, review: review, words: words)?.text
        }
    }

    /// The core's tone for each (`Concern::tone`): refused and danger kinds
    /// in the danger ink, the cautions in the warning one.
    static func tone(_ concern: MsJSON) -> MsNote.Tone {
        switch concern.kind {
        case "notSigned", "stateChange", "lookalike", "reverts": .danger
        default: .warning
        }
    }

    // MARK: - What moves (the simulation, or the probe's balances)

    @ViewBuilder private var moves: some View {
        let sim = review["simulation"]
        let deltas = sim?["deltas"]?.array ?? []
        if sim?["kind"]?.string == "deltas", !deltas.isEmpty {
            MsSection(title: words.loc.t("componentsUi.signing.simResultLabel")) {
                ForEach(Array(deltas.enumerated()), id: \.offset) { _, delta in
                    let text = deltaText(delta)
                    Text(verbatim: text)
                        .typeRole(Typography.bodyStrong.scaled(textScale))
                        .foregroundStyle(text.hasPrefix("-") ? theme.fgBase : theme.successBase)
                        .accessibilityIdentifier("multisig.review.delta")
                }
                if review["simulationSource"]?.string == "probe" {
                    MsNote(text: words.t("review.movesProbe"))
                }
            }
        } else if sim?["kind"]?.string == "deltas" {
            MsNote(text: words.loc.t(sim?["no_change_key"]?.string ?? "componentsUi.signing.simResultNoChange"))
        } else if let key = sim?["notice_key"]?.string, !key.isEmpty {
            let reason = sim?["revert_reason"]?.string
            MsNote(
                text: reason.map { words.loc.t(key, vars: ["reason": $0]) } ?? words.loc.t(key),
                tone: sim?["notice_risk"]?.string == "danger" ? .danger : .warning,
                identifier: "multisig.review.sim"
            )
        }
    }

    private func deltaText(_ delta: MsJSON) -> String {
        let raw = delta["delta"]?.string ?? "0"
        let negative = raw.hasPrefix("-")
        let magnitude = negative ? String(raw.dropFirst()) : raw
        let info: (symbol: String, decimals: Int)
        if let token = delta["token"]?.string, !token.isEmpty {
            info = tokens[token.lowercased()] ?? (MsWords.short(token), 18)
        } else {
            info = (words.symbol(chainId), 18)
        }
        return "\(negative ? "-" : "+")\(MsRules.formatUnits(magnitude, decimals: info.decimals)) \(info.symbol)"
    }

    private var tokenAddresses: [String] {
        (review["simulation"]?["deltas"]?.array ?? []).compactMap { $0["token"]?.string }
    }

    private func readTokens() async {
        for token in tokenAddresses where tokens[token.lowercased()] == nil {
            if let info = await words.hub?.tokenInfo(chainId: chainId, token: token) {
                tokens[token.lowercased()] = info
            }
        }
    }

    // MARK: - Who it reaches

    @ViewBuilder private var recipients: some View {
        let list = review["recipients"]?.array ?? []
        if !list.isEmpty {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                ForEach(Array(list.enumerated()), id: \.offset) { index, recipient in
                    if index > 0 { MsDivider() }
                    recipientRow(recipient)
                    // What is wrong with this recipient, under its own row
                    // (the web's `recipientBlocks`) — said once, here.
                    if let lookalike = recipient["lookalike"], !lookalike.isNull {
                        MsNote(text: words.t("review.lookalike", [
                            "address": recipient["address"]?.string ?? "",
                            "label": lookalike["label"]?.string ?? "",
                            "known": lookalike["address"]?.string ?? "",
                        ]), tone: .danger, identifier: "multisig.review.lookalike")
                    }
                    if recipient["contractCue"]?.bool == true {
                        MsNote(text: words.t("review.toContract", ["address": recipient["address"]?.string ?? ""]),
                               tone: .warning, identifier: "multisig.review.toContract")
                    }
                }
            }
        }
    }

    private func recipientRow(_ r: MsJSON) -> some View {
        let address = r["address"]?.string ?? ""
        let role: String = switch r["role"]?.string {
        case "contract": words.t("review.roleContract")
        case "member": words.t("review.roleMember")
        default: words.t("review.roleRecipient")
        }
        let label = r["call"]?.int.map { "\(words.t("review.callNumber", ["n": String($0)])) · \(role)" } ?? role
        let name = words.localName(address) ?? words.keptName(address) ?? r["known"]?["label"]?.string
        var tags: [String] = []
        if r["firstTimeCue"]?.bool == true { tags.append(words.loc.t("componentsUi.signing.firstTimeTag")) }
        if r["contractCue"]?.bool == true { tags.append(words.loc.t("componentsUi.signing.contractTag")) }
        let whose: String? = r["known"]?["kind"]?.string == "own" ? words.t("review.knownOwn")
            : (r["known"]?["kind"]?.string == "member" && r["role"]?.string != "member" ? words.t("review.knownMember") : nil)
        return VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: label)
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            if let name, name.lowercased() != address.lowercased() {
                Text(verbatim: name)
                    .typeRole(Typography.fieldLabel.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
            Text(verbatim: MsRules.eip55(address))
                .typeRole(Typography.monoSmall.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .textSelection(.enabled)
            if !tags.isEmpty {
                MsChip(text: tags.joined(separator: " · "), tone: r["lookalike"].map { $0.isNull ? .warning : .danger } ?? .warning)
            } else if let whose {
                MsChip(text: whose)
            }
        }
        .padding(.vertical, Tokens.Space.s8)
        .accessibilityElement(children: .combine)
    }

    // MARK: - Member changes (the rule they leave)

    @ViewBuilder private var memberChanges: some View {
        if let changes = review["memberChanges"], !changes.isNull {
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                ForEach(Array((changes["changes"]?.array ?? []).enumerated()), id: \.offset) { _, change in
                    let text = changeText(change)
                    Text(verbatim: change["call"]?.int.map { "#\($0) · \(text)" } ?? text)
                        .typeRole(Typography.bodyStrong.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                }
                let before = changes["before"], after = changes["after"]
                Text(verbatim: words.t("change.rule", [
                    "before": words.ruleText(m: before?["threshold"]?.int ?? 0, n: before?["owners"]?.int ?? 0),
                    "after": words.ruleText(m: after?["threshold"]?.int ?? 0, n: after?["owners"]?.int ?? 0),
                ]))
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .accessibilityIdentifier("multisig.review.rule")
                MsNote(text: words.t("change.caution", ["network": network]), tone: .warning)
                if changes["fits"]?.bool == false {
                    MsNote(text: words.t("change.doesNotFit", ["network": network]), tone: .danger)
                }
            }
        }
    }

    private func changeText(_ change: MsJSON) -> String {
        switch change["kind"]?.string {
        case "add": words.t("change.add", ["member": words.who(change["added"]?.string ?? "")])
        case "remove": words.t("change.remove", ["member": words.who(change["removed"]?.string ?? "")])
        case "replace":
            words.t("change.replace", [
                "old": words.who(change["removed"]?.string ?? ""), "new": words.who(change["added"]?.string ?? ""),
            ])
        default: words.t("change.threshold")
        }
    }

    // MARK: - The state-diff guard

    @ViewBuilder private var stateGuard: some View {
        let state = review["state"]
        let verdict = state?["verdict"]
        let source = state?["source"]?.string == "trace" ? words.t("review.sourceTrace") : words.t("review.sourceProbe")
        switch verdict?.kind {
        case "undeclared":
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                Text(verbatim: words.t("review.stateTitle"))
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
                MsNote(text: words.t("review.stateBody", ["network": network]), tone: .danger)
                ForEach(Array(writeLines(verdict?["writes"]?.array ?? []).enumerated()), id: \.offset) { _, line in
                    Text(verbatim: line)
                        .typeRole(Typography.monoSmall.scaled(textScale))
                        .foregroundStyle(theme.errorBase)
                        .fixedSize(horizontal: false, vertical: true)
                }
                MsNote(text: source)
            }
            .accessibilityIdentifier("multisig.review.state.undeclared")
        case "unchanged", "declared":
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                MsNote(
                    text: words.t(verdict?.kind == "declared" ? "review.stateDeclared" : "review.stateUnchanged", ["network": network]),
                    tone: .positive, identifier: "multisig.review.state.ok"
                )
                MsNote(text: source)
            }
        case "unavailable":
            MsNote(text: words.t("review.stateUnread", ["network": network]), tone: .warning, identifier: "multisig.review.state.unread")
        default:
            EmptyView()
        }
    }

    private func writeLines(_ writes: [MsJSON]) -> [String] {
        var out: [String] = []
        for w in writes {
            let lines: [String]
            switch w.kind {
            case "singleton": lines = [words.t("review.writeSingleton", ["before": w["before"]?.string ?? "", "after": w["after"]?.string ?? ""])]
            case "module": lines = [words.t(w["enabled"]?.bool == true ? "review.writeModuleOn" : "review.writeModuleOff", ["address": w["address"]?.string ?? ""])]
            case "guard": lines = [words.t("review.writeGuard", ["after": w["after"]?.string ?? ""])]
            case "moduleGuard": lines = [words.t("review.writeModuleGuard", ["after": w["after"]?.string ?? ""])]
            case "handler": lines = [words.t("review.writeHandler", ["after": w["after"]?.string ?? ""])]
            case "owners":
                lines = (w["added"]?.array ?? []).map { words.t("review.writeAdded", ["address": $0.string ?? ""]) }
                    + (w["removed"]?.array ?? []).map { words.t("review.writeRemoved", ["address": $0.string ?? ""]) }
            case "ownerEntry": lines = [words.t("review.writeOwnerEntry", ["address": w["address"]?.string ?? ""])]
            case "threshold": lines = [words.t("review.writeThreshold", ["before": w["before"]?.string ?? "", "after": w["after"]?.string ?? ""])]
            case "ownerCount": lines = [words.t("review.writeOwnerCount", ["after": w["after"]?.string ?? ""])]
            case "nonce": lines = [words.t("review.writeNonce")]
            default: lines = [words.t("review.writeStorage", ["slot": w["slot"]?.string ?? ""])]
            }
            for line in lines where !out.contains(line) { out.append(line) }
        }
        return out
    }

    // MARK: - Technical details (the SafeTx fields, the hashes signed)

    @ViewBuilder private var technical: some View {
        DisclosureGroup(isExpanded: $techOpen) {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                ForEach(Array((review["fields"]?.array ?? []).enumerated()), id: \.offset) { _, field in
                    MsRow(label: field["name"]?.string ?? "", value: field["value"]?.string ?? "", mono: true)
                }
                Text(verbatim: words.t("review.hashesTitle"))
                    .typeRole(Typography.label.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .padding(.top, Tokens.Space.s8)
                let hashes = review["hashes"]
                MsRow(label: words.t("external.domainHash"), value: hashes?["domainHash"]?.string ?? "", mono: true, copyable: true)
                MsRow(label: words.t("external.messageHash"), value: hashes?["messageHash"]?.string ?? "", mono: true, copyable: true)
                MsRow(label: words.t("external.safeTxHash"), value: hashes?["safeTxHash"]?.string ?? "", mono: true, copyable: true)
            }
            .accessibilityIdentifier("multisig.review.tech")
        } label: {
            Text(verbatim: words.t("review.techTitle"))
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgMuted)
        }
        .tint(theme.fgMuted)
    }
}
