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
    var standing: MsTagged? { view.standing(item) }
    var timeline: [MsTagged] { view.timeline(item) }
    var signers: MsSigners? { view.signers(item) }
    var execution: MsJSON? { view.execution(item) }
    var approveState: MsTagged? { view.action("approve:\(hash)") }
    var executeState: MsTagged? { view.action("execute:\(hash)") }

    // MARK: - The one review (§3.6)

    /// Where the machine's review stands: not in yet (`holding` — every
    /// signing control waits, as the signing sheet's confirm waits for its
    /// reading), could not be gathered (`failed`: said, with Retry), or in.
    enum ReviewState: Equatable {
        case holding
        case failed
        case ready(MsJSON)
    }

    var reviewState: ReviewState {
        guard let review = view.review(item) else { return .holding }
        return review.isNull ? .failed : .ready(review)
    }

    /// The review, once in.
    var review: MsJSON? {
        if case .ready(let review) = reviewState { return review }
        return nil
    }

    /// No signature is offered before the review says what is signed: the
    /// web's `reviewHolding` (no blind signing).
    var reviewHolding: Bool { reviewState == .holding }

    /// The last review could not read the chain (its state, its simulation,
    /// or the whole review): asked again soon (the web's `machineUnread`).
    var reviewUnread: Bool {
        switch reviewState {
        case .holding: return false
        case .failed: return true
        case .ready(let review):
            return review["state"]?["verdict"]?.kind == "unavailable"
                || review["simulation"]?["kind"]?.string == "unreachable"
        }
    }

    var title: String { words.actionTitleNow(chainId: chainId, action: item.action, nonce: item.nonce) }

    /// The chip under the title: a finished one says how it ended, never a
    /// count against today's members (r4a-8, r8-17).
    var statusLine: String {
        if isDone, executedTx != nil { return words.t("proposal.executed") }
        if isDone, replaced { return words.t("proposal.voided") }
        return words.standing(standing, item: item)
    }

    /// Who approved, by address (lowercased).
    var approvedBy: Set<String> { Set(item.approvals.map { $0.owner.lowercased() }) }

    /// The members on this network now (the chain's word), else the link's
    /// read, else the original.
    var members: [String] {
        view.chain(chainId)?.members?.owners
            ?? view.linked.flatMap { $0.chainId == chainId ? $0.members?.owners : nil }
            ?? account?.owners ?? []
    }

    var isDone: Bool { item.status == "done" }
    var isReady: Bool { item.status == "ready" }

    /// It ran, or its number was used: nothing more to sign, ask or check.
    var past: Bool { isDone || executedTx != nil }

    /// Replaced at its number (the machine's timeline ends so).
    var replaced: Bool { timeline.last?.kind == "replaced" }

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

    /// The core's sign rule, once the review is in: may this be signed at
    /// all (`allowed` false: nothing signed, no override), and only with the
    /// "… anyway" friction (`anyway`).
    var signAllowed: Bool { review?["sign"]?["allowed"]?.bool ?? true }
    var signAnyway: Bool { review.map { $0["sign"]?["anyway"]?.bool ?? false } ?? false }

    /// A signing control may be pressed: the review is in (or could not be
    /// gathered — the machine's gate reads it once more before the passkey)
    /// and its rule does not refuse it.
    var signable: Bool { !reviewHolding && signAllowed }

    /// The label a signing control wears: "… anyway" when the core asks
    /// for that friction (the web's `anywayLabel`).
    func signLabel(_ plain: String, anyway: String) -> String {
        words.t(signAnyway ? anyway : plain)
    }

    /// What the "… anyway" step names before it signs: the review's
    /// concerns, in its words — said again where the person confirms.
    var anywayReasons: [String] {
        guard let review else { return [] }
        return (review["concerns"]?.array ?? []).compactMap { MsReviewBody.concernText($0, review: review, words: words) }
    }

    // MARK: - An executed one (r8-17): as it was when it ran

    /// The machine's frozen facts (its receipt's, else what this device
    /// recorded sending it) — only once the chain says it ran.
    var frozen: MsJSON? { executedTx == nil ? nil : view.frozen(item) }

    /// Whom the approvals list names: the members whose approvals counted
    /// when it ran; a finished one's verified approvals while those are not
    /// known (never a "Waiting" row on a transaction that can no longer
    /// move); else this network's members now. The web's `listed`.
    var listed: [String] {
        if let frozen { return frozen["approvers"]?.array.compactMap(\.string) ?? [] }
        if isDone { return item.approvals.filter(\.counts).map(\.owner) }
        return members
    }

    /// One member's line on the list.
    enum MemberWord: Equatable {
        case executed, approved, notNeeded, waiting

        /// Its corpus key (`home.multisig.*`).
        var key: String {
            switch self {
            case .executed: "proposal.executed"
            case .approved: "proposal.approved"
            case .notNeeded: "proposal.notNeeded"
            case .waiting: "proposal.waiting"
            }
        }
    }

    func memberWord(_ owner: String) -> MemberWord {
        if let frozen, frozen["executorApproved"]?.bool == true,
           frozen["executor"]?.string?.caseInsensitiveCompare(owner) == .orderedSame {
            return .executed
        }
        if frozen != nil || isDone { return .approved }
        if item.approvals.contains(where: { $0.counts && $0.owner.caseInsensitiveCompare(owner) == .orderedSame }) {
            return .approved
        }
        return item.approved >= item.threshold ? .notNeeded : .waiting
    }

    /// The approvals heading's count: "Executed" for one that ran (never
    /// "1/2" against today's rule), nothing for one otherwise finished.
    var approvalsCount: String {
        if executedTx != nil { return words.t("proposal.executed") }
        if isDone { return replaced ? words.t("proposal.voided") : "" }
        return "\(item.approved)/\(item.threshold)"
    }

    /// "Executed by Eve · 2 of 2 approvals counted when it was executed" —
    /// or, with no facts read, that which approvals counted isn't recorded.
    var executedFacts: String? {
        guard executedTx != nil else { return nil }
        guard let frozen else { return words.t("proposal.approvalsSigned") }
        var parts: [String] = []
        if let executor = frozen["executor"]?.string {
            parts.append(words.t("proposal.executedBy", ["name": words.label(executor)]))
        }
        parts.append(words.t("proposal.countedThen", [
            "approved": String(frozen["approvers"]?.array.count ?? 0),
            "threshold": frozen["threshold"]?.string ?? String(item.threshold),
        ]))
        return parts.joined(separator: " · ")
    }

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

    /// The members still to approve (verified approvals count; nothing once
    /// it can no longer move).
    var stillNeeded: [String] {
        guard !past else { return [] }
        let have = Set(item.approvals.filter(\.counts).map { $0.owner.lowercased() })
        return members.filter { !have.contains($0.lowercased()) }
    }

    /// Enough approved: nobody else is asked (r4D-6).
    var enough: Bool { item.approved >= item.threshold || signers?.enough == true }

    /// r7-8: the machine's one partition of the members still to approve —
    /// each said once. Nothing once enough approved, or once it is past.
    private func part(_ list: [String]?) -> [String] {
        guard !past, !enough else { return [] }
        let missing = Set(stillNeeded.map { $0.lowercased() })
        return (list ?? []).filter { missing.contains($0.lowercased()) }
    }

    /// Outside wallets (a plain account, or one nothing shows is Vela's):
    /// the outside-wallet link — never the Vela link they cannot use.
    var outside: [String] { part(signers?.outside) }
    /// Vela members elsewhere: they approve in Vela, from the link (r4-9).
    var inVela: [String] { part(signers?.inVela) }
    /// Nothing says yet how they sign: both ways, said once.
    var either: [String] { part(signers?.either) }
    /// Smart-contract members: they approve from their own app — said,
    /// nothing offered.
    var contract: [String] { part(signers?.contract) }

    /// The members the Vela link is for: Vela's, nothing-says-yet ones, and
    /// those still being read — never this device's, never a contract or a
    /// surely outside wallet (the web's `velaMissing`).
    var velaMissing: [String] {
        guard !past, !enough else { return [] }
        let rows = signers?.rows ?? []
        let outsideSet = Set(outside.map { $0.lowercased() })
        let contractSet = Set(contract.map { $0.lowercased() })
        return stillNeeded.filter { owner in
            let key = owner.lowercased()
            guard words.localName(owner) == nil, !outsideSet.contains(key), !contractSet.contains(key) else { return false }
            let kind = rows.first { $0.owner.lowercased() == key }?.kind
            return inVela.contains { $0.lowercased() == key } || either.contains { $0.lowercased() == key }
                || kind == nil
        }
    }

    /// The Vela link section: while approvals are missing, for the Vela
    /// members — or when nothing else asks anyone (the web's condition).
    var showsVelaLink: Bool {
        guard !past, item.approved < item.threshold, !stillNeeded.isEmpty else { return false }
        return !velaMissing.isEmpty || (outside.isEmpty && contract.isEmpty)
    }

    var signerLines: [(text: String, id: String)] {
        var out: [(String, String)] = []
        if !inVela.isEmpty {
            out.append((words.t("proposal.approvesInVela", ["names": inVela.map(words.label).joined(separator: ", ")]), "inVela"))
        }
        if !either.isEmpty {
            out.append((words.t("proposal.eitherWay", ["names": either.map(words.label).joined(separator: ", ")]), "either"))
        }
        return out.map { (text: $0.0, id: $0.1) }
    }

    /// r7: smart-contract members still to approve, said honestly.
    var contractLine: String? {
        contract.isEmpty ? nil
            : words.t("proposal.contractMembers", ["names": contract.map(words.label).joined(separator: ", ")])
    }

    /// The multisig this transaction is of: the one shown, else the link's.
    var multisig: String {
        account?.address
            ?? view.linked.flatMap { $0.safeTxHash.caseInsensitiveCompare(hash) == .orderedSame ? $0.safe : nil }
            ?? view.followed ?? ""
    }

    /// The proposal's link, for another member's Vela.
    var link: String? {
        MsRules.proposalLink(
            origin: MsRules.string("shareOrigin", ["origin": "app://ios"]) ?? "https://getvela.app",
            locale: MultisigServiceURL.locale, chainId: chainId,
            multisig: multisig, safeTxHash: hash
        )
    }

    /// r8-8 (r4C-13): an outside member's own link — Vela's connect page,
    /// naming the transaction, which it reads, reviews and signs from alone.
    /// Only while approvals are missing and an outside member is among them.
    var outsideLink: String? {
        guard !past, item.approved < item.threshold, !outside.isEmpty, !multisig.isEmpty else { return nil }
        return MsRules.connectLink(
            page: MultisigServiceURL.connectPage(), invite: nil, members: outside,
            tx: (chainId: chainId, multisig: multisig, safeTxHash: hash)
        )
    }

    /// "Approve with another wallet" (M4): an outside member's wallet over a
    /// live session — offered for a member the machine says signs outside,
    /// or one nothing says yet (the web's `signWithWallet`).
    var offersOutsideWallet: Bool { !(outside + either).isEmpty }

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
    var onAddNetwork: ((Int) -> Void)? = nil
    /// A site's request (M5): what the site is told, and when.
    var siteNote: String? = nil

    @State private var member: String?
    @State private var executor: String?
    @State private var copied = false
    @State private var outsideCopied = false
    @State private var releaseWatch: (() -> Void)?
    @State private var confirmingReject = false
    /// The "… anyway" step the core asked for, while it is up (r8-1).
    @State private var anyway: MsAnywayStep?
    /// r8-19: the review has been out longer than the machine's own cap on
    /// a look: the sheet says it couldn't check, with Retry, not a spinner.
    @State private var reviewWaitedOut = false

    /// How long a review may be out before the sheet stops saying
    /// "Checking…" — the machine's `PREVIEW_CAP_MS` (a look past it is
    /// `unchecked`), so the sheet never outwaits the core.
    static let reviewWaitCap: Duration = .seconds(15)
    /// How often an open sheet asks for its review again (the web's
    /// `REVIEW_FRESH_MS`), and while the last could not read the chain
    /// (`REVIEW_RETRY_MS`).
    static let reviewFreshSeconds: Double = 30
    static let reviewRetrySeconds: Double = 5

    /// Which signature an "… anyway" step is for.
    enum SignKind { case approve, approveExecute, execute }

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
        // The review, its signers and (once done) how it ran, asked whenever
        // what they are of changes: the item arriving (a link, a list read
        // after the sheet opened), its members, it finishing (r8-17, r8-19).
        .task(id: askKey) { await keepAsking() }
        .msAnywayStep($anyway, cancel: words.t("proposal.rejectCancel"))
    }

    // MARK: - Asking the machine

    /// What the asks are of: the item being there, its members, done or not.
    private var askKey: String {
        guard let item else { return "none" }
        let members = view.chain(chainId)?.members
        return "\(item.safeTxHash.lowercased()):\(item.status):\(members?.owners.joined(separator: ",") ?? ""):\(members?.threshold ?? 0)"
    }

    /// The review now, again every `reviewFreshSeconds` while the sheet is
    /// up and every `reviewRetrySeconds` while the last could not read the
    /// chain; the signers once per change; a finished one's facts once — and
    /// its review once (its hashes and fields; nothing is re-checked).
    private func keepAsking() async {
        guard let item else { return }
        reviewWaitedOut = false
        askReview()
        hub.sendProposal(["type": "read_signers", "chain_id": chainId, "safe_tx_hash": item.safeTxHash])
        if item.status == "done" {
            hub.sendProposal(["type": "timeline", "chain_id": chainId, "safe_tx_hash": item.safeTxHash])
            return
        }
        let started = ContinuousClock.now
        var lastAsk = ContinuousClock.now
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(1))
            guard let current = self.item else { continue }
            let model = MsProposalModel(item: current, view: view, words: words, account: nil)
            if model.reviewHolding, ContinuousClock.now - started >= Self.reviewWaitCap {
                reviewWaitedOut = true
            }
            let since = ContinuousClock.now - lastAsk
            let due = model.reviewUnread || model.reviewHolding ? Self.reviewRetrySeconds : Self.reviewFreshSeconds
            if since >= .seconds(due) {
                askReview()
                lastAsk = ContinuousClock.now
            }
        }
    }

    private func askReview() {
        guard let item else { return }
        hub.sendProposal(["type": "review", "chain_id": chainId, "safe_tx_hash": item.safeTxHash, "blind": false])
    }

    private func anywayTitle(_ kind: SignKind) -> String {
        switch kind {
        case .approve: words.t("review.approveAnyway")
        case .approveExecute: words.t("review.approveExecuteAnyway")
        case .execute: words.t("review.executeAnyway")
        }
    }

    /// A signature: at once, or — when the core's rule asks for "… anyway"
    /// — after the person confirms past the review's reasons (r8-1).
    private func signing(_ model: MsProposalModel, _ kind: SignKind, _ run: @escaping () -> Void) {
        guard model.signable else { return }
        if model.signAnyway {
            anyway = MsAnywayStep(title: anywayTitle(kind), reasons: model.anywayReasons, run: run)
        } else {
            run()
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
        if let siteNote { MsNote(text: siteNote, identifier: "multisig.proposal.site") }
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
        reviewPart(model)
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

    // MARK: - The review (§3.6)

    /// The one review, in its states: a finished one draws only what was
    /// signed (nothing is re-checked); one not in yet says so for at most
    /// `reviewWaitCap` and then that it couldn't be checked, with Retry
    /// (r8-19 — never a spinner without end); one that could not be
    /// gathered says so, with Retry.
    @ViewBuilder private func reviewPart(_ model: MsProposalModel) -> some View {
        switch model.reviewState {
        case .ready(let review):
            MsReviewBody(review: review, chainId: chainId, words: words, past: model.past)
        case .holding where model.past:
            EmptyView()
        case .holding where !reviewWaitedOut:
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(verbatim: words.loc.t("componentsUi.funding.checking"))
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("multisig.review.checking")
        case .holding, .failed:
            if !model.past {
                HStack(alignment: .top, spacing: Tokens.Space.s8) {
                    MsNote(text: words.t("proposal.cantCheck", ["network": words.network(chainId)]),
                           tone: .warning, identifier: "multisig.review.unread")
                    Button(words.t("service.retry")) { askReview() }
                        .font(Typography.actionLabel.scaled(textScale).font)
                        .foregroundStyle(theme.fgBase)
                        .accessibilityIdentifier("multisig.review.retry")
                }
            }
        }
    }

    // MARK: - Approvals

    private func approvals(_ model: MsProposalModel) -> some View {
        let count = model.approvalsCount
        return MsSection(title: count.isEmpty ? words.t("proposal.approvals") : "\(words.t("proposal.approvals")) · \(count)") {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                ForEach(model.listed, id: \.self) { member in
                    let word = model.memberWord(member)
                    let done = word == .approved || word == .executed
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
                        Text(verbatim: words.t(word.key))
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(done ? theme.successBase : theme.fgSubtle)
                    }
                    .padding(.vertical, Tokens.Space.s8)
                    .accessibilityElement(children: .combine)
                    .accessibilityIdentifier("multisig.proposal.member")
                }
                if let facts = model.executedFacts {
                    MsNote(text: facts, identifier: "multisig.proposal.executedFacts")
                }
                if let line = model.contractLine {
                    MsNote(text: line, identifier: "multisig.proposal.signers.contract")
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
                    // core-ms-2: the members here whose ONE press approves and
                    // executes it (`approve_execute`) — the machine's word, with
                    // its look at the fee; executing pays it, approving does not.
                    let oneTouch = model.view.approveExecute(model.item)
                    let completes = !oneTouch.isEmpty
                    if completes {
                        VelaButton(
                            title: model.signLabel("proposal.approveAndExecute", anyway: "review.approveExecuteAnyway"),
                            kind: model.signAnyway ? .danger : .primary,
                            enabled: model.signable,
                            loading: approving || executing,
                            haptic: .confirm
                        ) {
                            let chosen = member.flatMap { m in oneTouch.first { $0.caseInsensitiveCompare(m) == .orderedSame } }
                            let executor = chosen ?? oneTouch[0]
                            signing(model, .approveExecute) {
                                hub.sendProposal([
                                    "type": "approve_execute", "chain_id": chainId, "safe_tx_hash": model.hash,
                                    "executor": executor,
                                ])
                            }
                        }
                        .accessibilityIdentifier("multisig.proposal.approveExecute")
                        MsNote(text: words.t("proposal.feeNote"))
                    }
                    VelaButton(
                        // Approving alone beside the one press is the plain
                        // second choice; alone, it wears the review's rule.
                        title: completes ? words.t("proposal.approve")
                            : model.signLabel("proposal.approve", anyway: "review.approveAnyway"),
                        kind: completes ? .secondary : (model.signAnyway ? .danger : .primary),
                        enabled: model.signable,
                        loading: approving && !completes,
                        haptic: .confirm
                    ) {
                        signing(model, .approve) { approve(model) }
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
                        title: model.signLabel("proposal.execute", anyway: "review.executeAnyway"),
                        kind: model.signAnyway ? .danger : .primary,
                        enabled: !model.executeHeld(by: chosen) && model.signable,
                        loading: executing,
                        haptic: .confirm
                    ) {
                        signing(model, .execute) { execute(model, executor: chosen) }
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

    /// Who still has to approve, and how each can — the machine's split
    /// (r7-8, `signers`): the Vela link for Vela members (and those nothing
    /// says yet), the outside-wallet link with its code for an outside
    /// wallet, and "Approve with another wallet" for a member whose wallet
    /// is at hand (r8-8: the web's and the desktop's sheet).
    @ViewBuilder private func asking(_ model: MsProposalModel) -> some View {
        if model.showsVelaLink {
            MsSection(title: words.t("proposal.invite")) {
                MsNote(text: words.t("proposal.inviteHint"))
                ForEach(model.signerLines, id: \.id) { line in
                    MsNote(text: line.text, identifier: "multisig.proposal.signers.\(line.id)")
                }
                if let link = model.link {
                    linkActions(link, copiedFlag: $copied, share: "multisig.proposal.share", copy: "multisig.proposal.copyLink")
                }
            }
            .accessibilityIdentifier("multisig.proposal.invite")
        }
        if let link = model.outsideLink {
            // r4C-13: an outside member's own link — Vela's connect page, which
            // reads, reviews and signs the transaction alone.
            MsSection(title: words.t("external.outsideLinkTitle")) {
                MsNote(text: words.t("external.outsideLinkHint"))
                Text(verbatim: words.t("proposal.stillNeeded", ["missing": model.outside.map(words.label).joined(separator: ", ")]))
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
                if let modules = QrCode.modules(link) {
                    MsQrView(modules: modules)
                        .frame(maxWidth: 240)
                        .aspectRatio(1, contentMode: .fit)
                        .frame(maxWidth: .infinity)
                        .accessibilityLabel(words.t("external.outsideLinkTitle"))
                        .accessibilityIdentifier("multisig.proposal.outsideQr")
                }
                Text(verbatim: link)
                    .typeRole(Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .textSelection(.enabled)
                    .lineLimit(3)
                    .truncationMode(.middle)
                    .accessibilityIdentifier("multisig.proposal.outsideLink")
                linkActions(link, copiedFlag: $outsideCopied, share: "multisig.proposal.outsideShare", copy: "multisig.proposal.outsideCopy")
            }
            .accessibilityIdentifier("multisig.proposal.outsideInvite")
        }
        if model.offersOutsideWallet {
            VelaButton(title: words.t("external.signWithWallet"), kind: .secondary) {
                hub.sendProposal(["type": "approve_outside", "chain_id": chainId, "safe_tx_hash": model.hash])
            }
            .accessibilityIdentifier("multisig.proposal.outside")
        }
    }

    private func linkActions(_ link: String, copiedFlag: Binding<Bool>, share: String, copy: String) -> some View {
        HStack(spacing: Tokens.Space.s12) {
            ShareLink(item: link) {
                Label(words.t("proposal.sendLink"), systemImage: "square.and.arrow.up")
            }
            .accessibilityIdentifier(share)
            Button(copiedFlag.wrappedValue ? words.t("proposal.copied") : words.t("proposal.copyLink")) {
                UIPasteboard.general.string = link
                copiedFlag.wrappedValue = true
                VelaHaptic.select.play()
            }
            .accessibilityIdentifier(copy)
        }
        .font(Typography.actionLabel.scaled(textScale).font)
        .foregroundStyle(theme.fgBase)
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
        // The review, the signers and the timeline: `keepAsking`, once the
        // item is there (a link's may still be opening).
        hub.sendProposal(["type": "timeline", "chain_id": chainId, "safe_tx_hash": safeTxHash])
        hub.sendProposal(["type": "refresh", "fresh": false])
    }

    /// Each executor's look, asked whenever the core read what it does —
    /// not only once ready (the web's `looked`): the one-press
    /// Approve and execute is offered only on a fresh look (core-ms-2).
    private func askPreviews(force: Bool = false) {
        guard let item, item.status != "done", item.action != nil else { return }
        for executor in item.canExecute {
            hub.sendProposal(["type": "preview", "chain_id": chainId, "safe_tx_hash": item.safeTxHash, "executor": executor])
        }
        _ = force
    }

    private func approve(_ model: MsProposalModel) {
        guard let owner = member ?? model.defaultMember else { return }
        hub.sendProposal(["type": "approve", "chain_id": chainId, "safe_tx_hash": model.hash, "owner": owner])
        let key = "approve:\(model.hash)"
        Task { @MainActor in
            let settled = await hub.proposals.until(timeout: 600) { view -> MsTagged? in
                let actions = view["actions"] as? [String: Any] ?? [:]
                guard let raw = actions[key] ?? actions.first(where: { $0.key.lowercased() == key.lowercased() })?.value,
                      let state = MultisigMachine.decode(MsTagged.self, from: raw)
                else { return nil }
                return state.kind == "working" ? nil : state
            }
            if settled?.kind == "done" {
                VelaHaptic.approvalCounted.play()
            } else if settled != nil, settled?.kind != "cancelled" {
                VelaHaptic.reject.play()
            }
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
