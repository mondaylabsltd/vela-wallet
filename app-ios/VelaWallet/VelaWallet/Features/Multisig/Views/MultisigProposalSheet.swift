//
//  MultisigProposalSheet.swift
//  VelaWallet
//
//  One multisig transaction (M2) — the web's `ProposalSheet.svelte`: what it
//  is (the one review every signer signs from), where it stands (the core's
//  one word), who has approved and how the rest can (`signers`), what
//  executing it costs and whether it would run now (the machine's preview,
//  paced by the machine), its history, and the actions this device may take:
//  approve, execute, reject, send again, ask the others.
//
//  Every decision is the proposals machine's. A signature always follows a
//  fresh chain read the machine makes itself (the signing gate, §3.5), and a
//  page venue approves on the member's trusted page.
//

import SwiftUI
import UIKit

/// What the sheet draws, read from the machine's view — apart from the view,
/// so a test reads every branch.
@MainActor
struct MsProposalModel {
    let item: MsProposalItem
    let view: MsProposalsView
    let words: MsWords
    let account: MsAccount?

    var chainId: Int { item.chainId }
    var hash: String { item.safeTxHash }
    var review: MsJSON? { view.review(item) }
    var standing: MsTagged? { view.standing(item) }
    var timeline: [MsTagged] { view.timeline(item) }
    var signers: MsSigners? { view.signers(item) }
    var execution: MsJSON? { view.execution(item) }
    var approveState: MsTagged? { view.action("approve:\(hash)") }
    var executeState: MsTagged? { view.action("execute:\(hash)") }

    var title: String { words.actionTitleNow(chainId: chainId, action: item.action, nonce: item.nonce) }
    var statusLine: String { words.standing(standing, item: item) }

    /// Who approved, by address (lowercased).
    var approvedBy: Set<String> { Set(item.approvals.map { $0.owner.lowercased() }) }

    /// The members on this network now (the chain's word), else the original.
    var members: [String] {
        view.chain(chainId)?.members?.owners ?? account?.owners ?? []
    }

    var isDone: Bool { item.status == "done" }
    var isReady: Bool { item.status == "ready" }

    /// This device's members that can still approve.
    var canApprove: [String] { item.canApprove }
    /// This device's accounts that can execute it.
    var canExecute: [String] { item.canExecute }

    /// The member this device approves as by default: the one used last, else the first.
    var defaultMember: String? {
        if let last = view.lastActor, canApprove.contains(where: { $0.lowercased() == last.lowercased() }) {
            return canApprove.first { $0.lowercased() == last.lowercased() }
        }
        return canApprove.first
    }

    /// The core's sign rule: may this be signed at all (the state guard, a
    /// refusal) — `anyway`: only with the "… anyway" friction.
    var signAllowed: Bool { review?["sign"]?["allowed"]?.bool ?? true }
    var signAnyway: Bool { review?["sign"]?["anyway"]?.bool ?? false }

    /// The lines that say where it stands, in order.
    var notes: [(text: String, tone: MsNote.Tone, id: String)] {
        var out: [(String, MsNote.Tone, String)] = []
        if let refusal = item.refusal, !refusal.isEmpty {
            out.append((words.t("proposal.refused", ["reason": refusal]), .danger, "refused"))
        }
        if item.cancelledBy != nil, item.action?.kind != "cancel" {
            out.append((words.t("proposal.cancelPending"), .warning, "cancelPending"))
        }
        if !item.sameNonce.isEmpty, !isDone {
            out.append((words.t("proposal.conflict", ["nonce": item.nonce]), .warning, "conflict"))
        }
        if item.status == "queued" {
            out.append((words.t("proposal.queuedNote", ["nonce": previousNonce]), .neutral, "queued"))
        }
        if item.renameUndeployed {
            out.append((words.t("proposal.renameUndeployed", ["network": words.network(chainId)]), .warning, "renameUndeployed"))
        }
        if standing?.kind == "blocked" {
            out.append((words.t("status.cannotRun", ["reason": words.blockReason(standing?["block"])]), .warning, "blocked"))
        }
        if item.unsent, let entry = view.outbox.first(where: { $0.key.lowercased().hasSuffix(hash.lowercased()) })?.value {
            out.append((outboxLine(entry), .warning, "outbox"))
        }
        if let line = executionLine { out.append(line) }
        if let failure = words.actionFailure(approveState, chainId: chainId) {
            out.append((failure, .danger, "approveFailed"))
        }
        if let failure = words.actionFailure(executeState, chainId: chainId) {
            out.append((failure, .danger, "executeFailed"))
        }
        if canApprove.isEmpty, !approvedByHere, !isDone, account != nil, (account?.owners.count ?? 0) > 0, canExecute.isEmpty {
            out.append((words.t("proposal.notMember"), .neutral, "notMember"))
        }
        return out.map { (text: $0.0, tone: $0.1, id: $0.2) }
    }

    /// One of this device's members approved it already.
    var approvedByHere: Bool {
        !item.canApprove.isEmpty ? false : item.approvals.contains { approval in
            (view.chain(chainId)?.members?.owners ?? []).contains { $0.lowercased() == approval.owner.lowercased() }
                && words.localName(approval.owner) != nil
        }
    }

    private var previousNonce: String {
        let n = (Int(item.nonce) ?? 1) - 1
        return String(max(0, n))
    }

    private func outboxLine(_ entry: MsJSON) -> String {
        let seconds = entry["retryAt"]?.int.map { max(0, ($0 - Int(Date().timeIntervalSince1970 * 1000)) / 1000) }
        switch entry["why"]?.string {
        case "busy": return words.t("proposal.outboxBusy", ["seconds": String(seconds ?? 0)])
        case "away", "unreachable": return words.t("proposal.outboxAway", ["seconds": String(seconds ?? 0)])
        default: return words.t("proposal.outboxNotSent")
        }
    }

    /// How an execution followed here ended (`executions`), in words.
    private var executionLine: (String, MsNote.Tone, String)? {
        guard let verdict = execution else { return nil }
        let paid = verdict["paid"]?.bool == true
        switch verdict.kind {
        case "executed": return (words.t("proposal.executed"), .positive, "executed")
        case "executedByOther": return (words.t(paid ? "proposal.lostRacePaid" : "proposal.lostRace"), .warning, "lostRace")
        case "replaced": return (words.t(paid ? "proposal.replacedFirstPaid" : "proposal.replacedFirst", ["nonce": item.nonce]), .warning, "replaced")
        case "failed": return (words.t("proposal.execFailed"), .danger, "execFailed")
        case "notSent": return (words.t("proposal.notSent"), .danger, "notSent")
        case "unknown": return (words.t("proposal.cantCheckUsed", ["network": words.network(chainId)]), .warning, "unknown")
        case "timedOut": return (words.t("proposal.notLanded"), .warning, "timedOut")
        case "waiting": return (words.t("proposal.executing"), .neutral, "waiting")
        default: return nil
        }
    }

    /// The executed transaction's hash, when the chain says it ran.
    var executedTx: String? {
        if let tx = execution?["txHash"]?.string { return tx }
        for step in timeline where step.kind == "executed" {
            if let tx = step["tx_hash"]?.string { return tx }
        }
        return nil
    }

    // MARK: - The executor's look (`previews`)

    func preview(for executor: String) -> MsJSON? { view.preview(item, executor: executor) }

    /// The fee line under Execute, for `executor`.
    func feeLine(for executor: String) -> (text: String, tone: MsNote.Tone)? {
        guard let preview = preview(for: executor) else { return (words.t("proposal.feeChecking"), .neutral) }
        switch preview.kind {
        case "priced":
            let fee = preview["fee"]
            let amount = fee?["amount"]?.string ?? "0"
            let symbol = words.symbol(chainId)
            let feeText = "\(MsRules.formatUnits(amount, decimals: 18)) \(symbol)"
            if preview["canPay"]?.bool == false {
                return (words.t("proposal.cannotPay", [
                    "name": words.label(executor), "symbol": symbol, "fee": feeText,
                    "balance": "\(MsRules.formatUnits(preview["balance"]?.string ?? "0", decimals: 18)) \(symbol)",
                ]), .warning)
            }
            return (words.t("proposal.feeLine", ["fee": feeText, "name": words.label(executor)]), .neutral)
        case "fails": return (words.t("proposal.wouldFail", ["reason": words.whyFails(preview["why"])]), .danger)
        case "unpriced": return (words.t("proposal.feeUnpriced"), .warning)
        case "unchecked": return (words.t("proposal.feeUnchecked"), .warning)
        case "refused": return (words.t("authority.deployUnverified", ["network": words.network(chainId)]), .danger)
        case "spent": return (words.t("proposal.doneNote"), .neutral)
        default: return nil
        }
    }

    /// The setup an execution carries (deploying the multisig, activating accounts).
    func setupLines(for executor: String) -> [String] {
        guard let preview = preview(for: executor), preview.kind == "priced", let setup = preview["setup"] else { return [] }
        var lines: [String] = []
        if setup["multisig"]?.bool == true { lines.append(words.t("proposal.setupMultisig")) }
        for member in setup["members"]?.array ?? [] {
            if let address = member.string { lines.append(words.t("proposal.setupMember", ["name": words.label(address)])) }
        }
        if setup["executor"]?.bool == true { lines.append(words.t("proposal.setupExecutor", ["name": words.label(executor)])) }
        guard !lines.isEmpty else { return [] }
        let head = setup["amount"]?.string.map {
            words.t("proposal.feeSetup", ["network": words.network(chainId), "amount": "\(MsRules.formatUnits($0, decimals: 18)) \(words.symbol(chainId))"])
        } ?? words.t("proposal.feeSetupPlain", ["network": words.network(chainId)])
        return [head] + lines
    }

    /// The preview stops it (would fail, nobody can pay): Execute waits.
    func executeHeld(by executor: String) -> Bool {
        guard let preview = preview(for: executor) else { return false }
        if preview.kind == "fails" || preview.kind == "refused" || preview.kind == "spent" { return true }
        return preview.kind == "priced" && preview["canPay"]?.bool == false
    }

    // MARK: - Asking the others (`signers`)

    var stillNeeded: [String] {
        let have = approvedBy
        return members.filter { !have.contains($0.lowercased()) }
    }

    var signerLines: [(text: String, id: String)] {
        guard let signers, !isDone else { return [] }
        var out: [(String, String)] = []
        if !signers.inVela.isEmpty {
            out.append((words.t("proposal.approvesInVela", ["names": signers.inVela.map(words.label).joined(separator: ", ")]), "inVela"))
        }
        if !signers.either.isEmpty {
            out.append((words.t("proposal.eitherWay", ["names": signers.either.map(words.label).joined(separator: ", ")]), "either"))
        }
        if !signers.contract.isEmpty {
            out.append((words.t("proposal.contractMembers", ["names": signers.contract.map(words.label).joined(separator: ", ")]), "contract"))
        }
        return out.map { (text: $0.0, id: $0.1) }
    }

    /// The proposal's link, for another member's Vela.
    var link: String? {
        MsRules.proposalLink(
            origin: MsRules.string("shareOrigin", ["origin": "app://ios"]) ?? "https://getvela.app",
            locale: MultisigServiceURL.locale, chainId: chainId,
            multisig: account?.address ?? view.followed ?? "", safeTxHash: hash
        )
    }

    // MARK: - History

    func timelineLine(_ step: MsTagged) -> String? {
        switch step.kind {
        case "created": return words.t("timeline.created", ["name": words.label(step["who"]?.string ?? "")])
        case "approved":
            let who = words.label(step["who"]?.string ?? "")
            var line = words.t(step["by_executing"]?.bool == true ? "timeline.approvedByExecuting" : "timeline.approved", ["name": who])
            if let counted = step["counted"]?.bool {
                line += " · " + words.t(counted ? "timeline.counted" : "timeline.notCounted")
            }
            return line
        case "executing": return words.t("proposal.executing")
        case "executed":
            if let who = step["who"]?.string { return words.t("timeline.executedBy", ["name": words.label(who)]) }
            return words.t("timeline.executed")
        case "cancelProposed": return words.t("proposal.cancelPending")
        case "replaced": return words.t("proposal.voided")
        default: return nil
        }
    }
}

struct MultisigProposalSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.openURL) private var openURL
    let hub: MultisigHub
    let chainId: Int
    let safeTxHash: String
    let words: MsWords
    var onClose: () -> Void
    /// Add the network a proposal is on (a network this device lacks).
    var onAddNetwork: ((Int) -> Void)?

    @State private var member: String?
    @State private var executor: String?
    @State private var copied = false
    @State private var releaseWatch: (() -> Void)?
    @State private var confirmingReject = false

    private var view: MsProposalsView { hub.proposalsView }
    private var item: MsProposalItem? {
        view.item(chainId: chainId, hash: safeTxHash) ?? view.linked.flatMap {
            $0.chainId == chainId && $0.safeTxHash.caseInsensitiveCompare(safeTxHash) == .orderedSame ? $0.item : nil
        }
    }

    var body: some View {
        MsSheet(title: words.t("proposal.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: close) {
            if let item {
                content(MsProposalModel(item: item, view: view, words: words, account: hub.accountsView.activeRow?.account))
            } else {
                linkedState
            }
        }
        .onAppear(perform: appeared)
        .onDisappear {
            releaseWatch?()
            releaseWatch = nil
        }
        .task(id: chainId) {
            // The machine decides whether the last look is stale; a sheet
            // only asks every few seconds (docs/multisig-machines.md §1.4).
            while !Task.isCancelled {
                askPreviews()
                try? await Task.sleep(nanoseconds: 4_000_000_000)
            }
        }
    }

    // MARK: - States before the item reads

    @ViewBuilder private var linkedState: some View {
        let linked = view.linked
        switch linked?.stand {
        case "notInMailbox": MsNote(text: words.t("proposal.notInMailbox"), tone: .warning, identifier: "multisig.proposal.notInMailbox")
        case "notFound": MsNote(text: words.t("proposal.notFound"), tone: .warning, identifier: "multisig.proposal.notFound")
        case "unreadable": MsNote(text: words.t("proposal.cantCheck", ["network": words.network(chainId)]), tone: .warning)
        default:
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(verbatim: words.t("proposal.opening"))
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("multisig.proposal.opening")
        }
        if linked?.preview == true {
            MsNote(text: words.t("proposal.previewNote"))
        }
    }

    // MARK: - The transaction

    @ViewBuilder private func content(_ model: MsProposalModel) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(verbatim: model.title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("multisig.proposal.title")
            HStack(spacing: Tokens.Space.s8) {
                MsChip(text: model.statusLine, tone: model.standing?.kind == "blocked" ? .warning : .neutral)
                    .accessibilityIdentifier("multisig.proposal.status")
                Text(verbatim: "\(words.network(chainId)) · \(words.t("action.numberValue", ["nonce": model.item.nonce]))")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        ForEach(model.notes, id: \.id) { note in
            MsNote(text: note.text, tone: note.tone, identifier: "multisig.proposal.note.\(note.id)")
        }
        if let tx = model.executedTx {
            executedRow(tx)
        }
        if let review = model.review {
            MsReviewBody(review: review, chainId: chainId, words: words)
        } else {
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(verbatim: words.loc.t("componentsUi.signing.simRechecking"))
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
        }
        approvals(model)
        actions(model)
        asking(model)
        history(model)
        MsRow(label: words.t("proposal.hash"), value: model.hash, mono: true, copyable: true)
        MsNote(text: words.t("proposal.noExpiry"))
    }

    private func executedRow(_ tx: String) -> some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            MsNote(text: words.t("proposal.executed"), tone: .positive, identifier: "multisig.proposal.executed")
            if let url = ExplorerLinks.tx(chainId: chainId, hash: tx, store: VelaStore()) {
                Button(words.t("proposal.explorer")) { openURL(url) }
                    .font(Typography.actionLabel.scaled(textScale).font)
                    .foregroundStyle(theme.fgBase)
                    .accessibilityIdentifier("multisig.proposal.explorer")
            }
        }
    }

    // MARK: - Approvals

    private func approvals(_ model: MsProposalModel) -> some View {
        MsSection(title: "\(words.t("proposal.approvals")) · \(model.item.approved)/\(model.item.threshold)") {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                ForEach(model.members, id: \.self) { member in
                    let done = model.approvedBy.contains(member.lowercased())
                    HStack(spacing: Tokens.Space.s12) {
                        IdenticonAvatar(seed: member, size: 28, tappable: false)
                        VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                            Text(verbatim: words.label(member))
                                .typeRole(Typography.fieldLabel.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                            Text(verbatim: MsWords.short(member))
                                .typeRole(Typography.monoSmall.scaled(textScale))
                                .foregroundStyle(theme.fgSubtle)
                        }
                        Spacer(minLength: Tokens.Space.s8)
                        Text(verbatim: words.t(done ? "proposal.approved"
                            : (model.item.approved >= model.item.threshold ? "proposal.notNeeded" : "proposal.waiting")))
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(done ? theme.successBase : theme.fgSubtle)
                    }
                    .padding(.vertical, Tokens.Space.s8)
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("multisig.proposal.member")
                }
            }
        }
    }

    // MARK: - Actions

    @ViewBuilder private func actions(_ model: MsProposalModel) -> some View {
        let approving = model.approveState?.kind == "working"
        let executing = model.executeState?.kind == "working" || model.item.executing
        if !model.isDone {
            VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                if !model.canApprove.isEmpty {
                    if model.canApprove.count > 1 {
                        memberPicker(model.canApprove, selection: Binding(
                            get: { member ?? model.defaultMember },
                            set: { member = $0 }
                        ))
                    }
                    VelaButton(
                        title: words.t(model.signAllowed ? "proposal.approve" : "review.approveAnyway"),
                        kind: .primary,
                        enabled: model.signAllowed || model.signAnyway,
                        loading: approving,
                        haptic: .confirm
                    ) {
                        approve(model)
                    }
                    .accessibilityIdentifier("multisig.proposal.approve")
                }
                if model.isReady, !model.canExecute.isEmpty {
                    let chosen = executor ?? model.canExecute.first ?? ""
                    if model.canExecute.count > 1 {
                        memberPicker(model.canExecute, selection: Binding(get: { chosen }, set: { executor = $0 }))
                    }
                    if let line = model.feeLine(for: chosen) {
                        MsNote(text: line.text, tone: line.tone, identifier: "multisig.proposal.fee")
                    }
                    ForEach(model.setupLines(for: chosen), id: \.self) { line in
                        MsNote(text: line)
                    }
                    VelaButton(
                        title: words.t(model.signAllowed ? "proposal.execute" : "review.executeAnyway"),
                        kind: .primary,
                        enabled: !model.executeHeld(by: chosen) && (model.signAllowed || model.signAnyway),
                        loading: executing,
                        haptic: .confirm
                    ) {
                        execute(model, executor: chosen)
                    }
                    .accessibilityIdentifier("multisig.proposal.execute")
                    if model.preview(for: chosen)?.kind == "unpriced" || model.preview(for: chosen)?.kind == "unchecked" {
                        Button(words.t("proposal.feeRetry")) { askPreviews(force: true) }
                            .font(Typography.actionLabel.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                    }
                } else if !model.isReady {
                    MsNote(text: words.t("proposal.feeNote"))
                }
                if model.item.unsent {
                    VelaButton(title: words.t("proposal.outboxSendAgain"), kind: .ink) {
                        hub.sendProposal(["type": "send_again", "chain_id": chainId, "safe_tx_hash": model.hash])
                    }
                    .accessibilityIdentifier("multisig.proposal.sendAgain")
                }
                if !model.canApprove.isEmpty || model.approvedByHere || !model.canExecute.isEmpty,
                   model.item.action?.kind != "cancel", model.item.cancelledBy == nil {
                    Button(words.t("proposal.reject")) { confirmingReject = true }
                        .font(Typography.actionLabel.scaled(textScale).font)
                        .foregroundStyle(theme.errorBase)
                        .accessibilityIdentifier("multisig.proposal.reject")
                        .confirmationDialog(
                            words.t("proposal.reject"), isPresented: $confirmingReject, titleVisibility: .visible
                        ) {
                            Button(words.t("proposal.reject"), role: .destructive) { reject(model) }
                            Button(words.t("proposal.rejectCancel"), role: .cancel) {}
                        } message: {
                            Text(verbatim: words.t("proposal.rejectHint", ["threshold": String(model.item.threshold)]))
                        }
                }
            }
            .padding(.top, Tokens.Space.s8)
        }
    }

    private func memberPicker(_ members: [String], selection: Binding<String?>) -> some View {
        Picker(words.t("proposal.as"), selection: selection) {
            ForEach(members, id: \.self) { address in
                Text(verbatim: words.label(address)).tag(Optional(address))
            }
        }
        .pickerStyle(.menu)
        .tint(theme.fgBase)
        .accessibilityIdentifier("multisig.proposal.as")
    }

    // MARK: - Asking the others

    @ViewBuilder private func asking(_ model: MsProposalModel) -> some View {
        if !model.isDone, !model.stillNeeded.isEmpty {
            MsSection(title: words.t("proposal.invite")) {
                MsNote(text: words.t("proposal.stillNeeded", ["missing": model.stillNeeded.map(words.label).joined(separator: ", ")]))
                ForEach(model.signerLines, id: \.id) { line in
                    MsNote(text: line.text, identifier: "multisig.proposal.signers.\(line.id)")
                }
                if let link = model.link {
                    HStack(spacing: Tokens.Space.s12) {
                        ShareLink(item: link) {
                            Label(words.t("proposal.sendLink"), systemImage: "square.and.arrow.up")
                        }
                        .accessibilityIdentifier("multisig.proposal.share")
                        Button(copied ? words.t("proposal.copied") : words.t("proposal.copyLink")) {
                            UIPasteboard.general.string = link
                            copied = true
                            VelaHaptic.select.play()
                        }
                        .accessibilityIdentifier("multisig.proposal.copyLink")
                    }
                    .font(Typography.actionLabel.scaled(textScale).font)
                    .foregroundStyle(theme.fgBase)
                }
                if let signers = model.signers, !(signers.outside + signers.either).isEmpty {
                    VelaButton(title: words.t("external.signWithWallet"), kind: .secondary) {
                        hub.sendProposal(["type": "approve_outside", "chain_id": chainId, "safe_tx_hash": model.hash])
                    }
                    .accessibilityIdentifier("multisig.proposal.outside")
                }
            }
        }
    }

    // MARK: - History

    @ViewBuilder private func history(_ model: MsProposalModel) -> some View {
        let lines = model.timeline.compactMap(model.timelineLine)
        if !lines.isEmpty {
            MsSection(title: words.t("timeline.heading")) {
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    Text(verbatim: line)
                        .typeRole(Typography.rowSub.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
            }
            .accessibilityIdentifier("multisig.proposal.history")
        }
    }

    // MARK: - Verbs

    private func appeared() {
        releaseWatch = hub.watch()
        hub.sendProposal(["type": "review", "chain_id": chainId, "safe_tx_hash": safeTxHash, "blind": false])
        hub.sendProposal(["type": "read_signers", "chain_id": chainId, "safe_tx_hash": safeTxHash])
        hub.sendProposal(["type": "timeline", "chain_id": chainId, "safe_tx_hash": safeTxHash])
        hub.sendProposal(["type": "refresh", "fresh": false])
    }

    private func askPreviews(force: Bool = false) {
        guard let item, item.status == "ready" else { return }
        for executor in item.canExecute {
            hub.sendProposal(["type": "preview", "chain_id": chainId, "safe_tx_hash": item.safeTxHash, "executor": executor])
        }
        _ = force
    }

    private func approve(_ model: MsProposalModel) {
        guard let owner = member ?? model.defaultMember else { return }
        hub.sendProposal(["type": "approve", "chain_id": chainId, "safe_tx_hash": model.hash, "owner": owner])
        Task { @MainActor in
            let settled = await hub.proposals.until(timeout: 600) { view in
                let state = MultisigMachine.decode(MsProposalsView.self, from: view)?.action("approve:\(model.hash)")
                return state?.kind == "working" ? nil : state
            }
            if settled?.kind == "done" { VelaHaptic.approvalCounted.play() }
            else if settled != nil, settled?.kind != "cancelled" { VelaHaptic.reject.play() }
        }
    }

    private func execute(_ model: MsProposalModel, executor: String) {
        hub.sendProposal(["type": "execute", "chain_id": chainId, "safe_tx_hash": model.hash, "executor": executor])
    }

    private func reject(_ model: MsProposalModel) {
        let owner = member ?? model.defaultMember ?? model.canExecute.first
        Task { @MainActor in
            _ = await hub.propose(
                chainId: chainId,
                proposal: ["kind": "cancel", "nonce": model.item.nonce],
                address: hub.accountsView.active,
                review: false,
                as: owner
            )
        }
    }

    private func close() {
        if view.linked != nil { hub.sendProposal(["type": "close_linked"]) }
        onClose()
    }
}
