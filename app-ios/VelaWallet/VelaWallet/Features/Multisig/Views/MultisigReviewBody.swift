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
    @State private var tokens: [String: (symbol: String, decimals: Int?)] = [:]
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
            // r9-9: the core's own reason, said once.
            MsNote(text: words.refusalLine(review), tone: .danger, identifier: "multisig.review.blocked")
        }
    }

    /// A concern in words — every kind, for the "… anyway" step, which
    /// names what the person is signing past.
    static func concernText(_ concern: MsJSON, review: MsJSON? = nil, words: MsWords) -> String? {
        switch concern.kind {
        case "notSigned":
            return words.refusalLine(review)
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

    /// What the verdict's place holds, as the core's `simulation` says it.
    enum MovesSaid: Equatable {
        /// Nothing: the core withholds what moves (`kind` `withheld` —
        /// refused for a reason other than the state guard's, a delegatecall
        /// or a batch too deep), or there is no line to say.
        case nothing
        /// The balance rows.
        case deltas
        /// One line: the core's no-change key (quiet), or its notice.
        case line(key: String, reason: String?, tone: MsNote.Tone)
    }

    /// The core's `simulation` in the verdict's place — never "No asset
    /// changes" for a simulation the core withheld.
    static func movesSaid(_ review: MsJSON) -> MovesSaid {
        guard let sim = review["simulation"] else { return .nothing }
        switch sim["kind"]?.string {
        case "withheld":
            return .nothing
        case "deltas":
            if !(sim["deltas"]?.array ?? []).isEmpty { return .deltas }
            return .line(key: sim["no_change_key"]?.string ?? "componentsUi.signing.simResultNoChange",
                         reason: nil, tone: .neutral)
        default:
            guard let key = sim["notice_key"]?.string, !key.isEmpty else { return .nothing }
            return .line(key: key, reason: sim["revert_reason"]?.string,
                         tone: sim["notice_risk"]?.string == "danger" ? .danger : .warning)
        }
    }

    /// R22 (2a): the core's line about who checked what moves
    /// (`simulation.source_key` — the person's own node alone); `nil` for a
    /// check two independent nodes agreed on, or none.
    @MainActor
    static func sourceLine(_ review: MsJSON, words: MsWords) -> String? {
        guard let key = review["simulation"]?["source_key"]?.string, !key.isEmpty else { return nil }
        let text = words.loc.t(key)
        return text == key ? nil : text
    }

    @ViewBuilder private var moves: some View {
        let sim = review["simulation"]
        let deltas = sim?["deltas"]?.array ?? []
        switch Self.movesSaid(review) {
        case .nothing:
            EmptyView()
        case .deltas:
            // r11 S4: headed as the review says (`moves_title_key`, "Balance
            // changes"), as on the web, the outside page and the desktop.
            MsSection(title: words.loc.t(review["movesTitleKey"]?.string
                                         ?? review["moves_title_key"]?.string
                                         ?? "componentsUi.signing.balanceChangesTitle")) {
                ForEach(Array(deltas.enumerated()), id: \.offset) { _, delta in
                    if let said = deltaText(delta) {
                        Text(prose: said.text)
                            .typeRole(Typography.bodyStrong.scaled(textScale))
                            .foregroundStyle(said.negative ? theme.fgBase : theme.successBase)
                            .accessibilityIdentifier("multisig.review.delta")
                    }
                }
                if review["simulationSource"]?.string == "probe" {
                    MsNote(text: words.t("review.movesProbe"))
                }
                // R22: checked by the person's own node alone — said quietly.
                if let source = Self.sourceLine(review, words: words) {
                    MsNote(text: source, identifier: "multisig.review.simSource")
                }
            }
        case .line(let key, let reason, let tone) where tone == .neutral:
            MsNote(text: reason.map { words.loc.t(key, vars: ["reason": $0]) } ?? words.loc.t(key))
            if let source = Self.sourceLine(review, words: words) {
                MsNote(text: source, identifier: "multisig.review.simSource")
            }
        case .line(let key, let reason, let tone):
            MsNote(
                text: reason.map { words.loc.t(key, vars: ["reason": $0]) } ?? words.loc.t(key),
                tone: tone,
                identifier: "multisig.review.sim"
            )
        }
    }

    /// One balance change, as the core writes a signed amount
    /// (`formatSignedTokenAmount`: U+2212 for a minus, the plus written, a
    /// zero change not drawn) — r11 S4: never a hyphen.
    private func deltaText(_ delta: MsJSON) -> (text: String, negative: Bool)? {
        let raw = delta["delta"]?.string ?? "0"
        var facts = words.facts(chainId: chainId, token: delta["token"]?.string)
        if let token = delta["token"]?.string, !token.isEmpty, let read = tokens[token.lowercased()] {
            facts = MsWords.TokenFacts(symbol: read.symbol, decimals: read.decimals ?? facts.decimals)
        }
        // r12-1: decimals nobody read — the signed base units, said so;
        // never a figure in 18 decimals assumed.
        guard let decimals = facts.decimals else {
            let negative = raw.hasPrefix("-")
            let digits = negative ? String(raw.dropFirst()) : raw
            guard digits.contains(where: { ("1"..."9").contains($0) }) else { return nil }
            let sign = negative ? "\u{2212}" : "+"
            return (sign + words.amount(digits, facts), negative)
        }
        return Self.signedDelta(raw, decimals: decimals, symbol: facts.symbol)
    }

    /// `raw` signed base units → "−0.01 USDC" (the core's figure).
    static func signedDelta(_ raw: String, decimals: Int, symbol: String) -> (text: String, negative: Bool)? {
        guard let figure = SimDeltas.deltaText(raw, decimals: decimals) else { return nil }
        return ("\(figure) \(symbol)", figure.hasPrefix("\u{2212}"))
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
                let drawn = MsReading.partyAddresses(MsReading.blocks(review, chainId: chainId, words: words))
                ForEach(Array(list.enumerated()), id: \.offset) { index, recipient in
                    // One the reading names already is said once, there (r5reg-15).
                    if !drawn.contains((recipient["address"]?.string ?? "").lowercased()) {
                        if index > 0 { MsDivider() }
                        recipientRow(recipient)
                    }
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
            Text(prose: label)
                .typeRole(Typography.label.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            if let name, name.lowercased() != address.lowercased() {
                Text(prose: name)
                    .typeRole(Typography.fieldLabel.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
            AddressTextView(MsRules.eip55(address), role: Typography.monoSmall.scaled(textScale))
                .foregroundStyle(theme.fgBase)
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
                    Text(prose: change["call"]?.int.map { "#\($0) · \(text)" } ?? text)
                        .typeRole(Typography.bodyStrong.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                }
                let before = changes["before"], after = changes["after"]
                Text(prose: words.t("change.rule", [
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
                Text(prose: words.t("review.stateTitle"))
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.errorBase)
                MsNote(text: words.t("review.stateBody", ["network": network]), tone: .danger)
                ForEach(Array(writeLines(verdict?["writes"]?.array ?? []).enumerated()), id: \.offset) { _, line in
                    Text(prose: line)
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
            MsNote(text: Self.unreadStateLine(verdict, network: network, words: words), tone: .warning,
                   identifier: "multisig.review.state.unread")
        case "withheld":
            // r9-9: refused for another reason, said once above — nothing in
            // the guard's place (no "changes nothing" beside "won't sign").
            EmptyView()
        default:
            EmptyView()
        }
    }

    /// The state guard's could-not-check line, by the core's reason (R22):
    /// one node's word (`uncorroborated`) and two that disagree
    /// (`disagreed`) are said as what they are; any other reason is
    /// "couldn't read" as before.
    @MainActor
    static func unreadStateLine(_ verdict: MsJSON?, network: String, words: MsWords) -> String {
        switch verdict?["reason"]?.string {
        case "uncorroborated": words.t("review.stateUnconfirmed", ["network": network])
        case "disagreed": words.t("review.stateDisagreed", ["network": network])
        default: words.t("review.stateUnread", ["network": network])
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
                Text(prose: words.t("review.hashesTitle"))
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
            Text(prose: words.t("review.techTitle"))
                .font(Typography.actionLabel.scaled(textScale).font)
                .foregroundStyle(theme.fgMuted)
        }
        .tint(theme.fgMuted)
    }
}

/// §12: a review that is not drawn — still being read, or not read — in its
/// place, as the core's sign door says (`review.door`). Loading is a calm
/// line; failed is its reason, that nothing can be signed until it is read,
/// and "Try again" (the machine's `review_again`). The button it stands
/// above stays shut either way: that is the door's `enabled`, not this.
struct MsReviewPending: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let door: MsSignDoor
    let words: MsWords
    /// The review's network, by name (r14-13: "Couldn't reach {{network}} to
    /// read this transaction.").
    var network: String = ""
    let tryAgain: () -> Void

    var body: some View {
        switch door.shows {
        case .review:
            if door.unchecked {
                // D1: drawn, but neither safety check read — said in danger,
                // with Try again; the button below signs only "… anyway".
                VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                    MsNote(text: Self.uncheckedLine(door, words: words), tone: .danger,
                           identifier: "multisig.review.unchecked")
                    if door.retry {
                        VelaButton(title: words.t("review.state.tryAgain"), kind: .secondary, action: tryAgain)
                            .accessibilityIdentifier("multisig.review.tryAgain")
                    }
                }
            }
        case .loading:
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(prose: words.t("review.state.loading"))
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("multisig.review.loading")
        case .failed:
            VStack(alignment: .leading, spacing: Tokens.Space.s8) {
                MsNote(text: Self.failedLine(door, words: words, network: network), tone: .warning, identifier: "multisig.review.failed")
                if door.retry {
                    VelaButton(title: words.t("review.state.tryAgain"), kind: .secondary, action: tryAgain)
                        .accessibilityIdentifier("multisig.review.tryAgain")
                }
            }
        }
    }

    /// D1: the danger line over a review neither of whose checks was read,
    /// in the core's words.
    @MainActor
    static func uncheckedLine(_ door: MsSignDoor, words: MsWords) -> String {
        words.loc.t(door.reasonKey ?? "home.multisig.review.state.unchecked")
    }

    /// The reason (the core's key) and what it means: nothing is signed.
    @MainActor
    static func failedLine(_ door: MsSignDoor, words: MsWords, network: String = "") -> String {
        let blocked = words.t("review.state.blocked")
        guard let key = door.reasonKey else { return blocked }
        return "\(words.loc.t(key, vars: ["network": network])) \(blocked)"
    }
}
