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
    /// r9-13: how the last Reject of it ended, when there is something to
    /// say (`MultisigHub.rejectFailure`) — its review stood in for this
    /// sheet while it was up.
    var rejectState: MsTagged?
    /// §12: the core's sign door over this transaction's review — asked
    /// once per model (every signing control below reads it).
    let door: MsSignDoor

    init(item: MsProposalItem, view: MsProposalsView, words: MsWords, account: MsAccount?, rejectState: MsTagged? = nil) {
        self.item = item
        self.view = view
        self.words = words
        self.account = account
        self.rejectState = rejectState
        let review = view.review(item)
        door = MsRules.reviewDoor(state: view.reviewState(item), review: review.flatMap { $0.isNull ? nil : $0 })
    }

    var chainId: Int { item.chainId }
    var hash: String { item.safeTxHash }
    var standing: MsTagged? { view.standing(item) }
    var timeline: [MsTagged] { view.timeline(item) }
    var signers: MsSigners? { view.signers(item) }
    var execution: MsJSON? { view.execution(item) }
    var approveState: MsTagged? { view.action("approve:\(hash)") }
    var executeState: MsTagged? { view.action("execute:\(hash)") }
    /// r11 one-press: the look stopped the last press — its line (the
    /// action's words) stands in the fee line's place, never beside it.
    var executeStopped: Bool { executeState?.kind == "stopped" }

    // MARK: - The one review (§3.6)

    /// Where the machine's review stands, as the core's door says (§12):
    /// being read (`loading` — every signing control waits), not read
    /// (`failed`: its reason, with "Try again"), or read.
    enum ReviewState: Equatable {
        case loading
        case failed
        case ready(MsJSON)
    }

    var reviewState: ReviewState {
        switch door.shows {
        case .loading: return .loading
        case .failed: return .failed
        case .review:
            guard let review = view.review(item), !review.isNull else { return .failed }
            return .ready(review)
        }
    }

    /// The review, once in.
    var review: MsJSON? {
        if case .ready(let review) = reviewState { return review }
        return nil
    }

    /// No signature is offered before the review says what is signed: the
    /// web's `reviewHolding` (no blind signing).
    var reviewHolding: Bool { reviewState == .loading }

    /// The review drawn could not read part of the chain (its state, its
    /// simulation): asked again soon (the web's `machineUnread`) — the
    /// review stays drawn meanwhile. One not read at all is the person's
    /// "Try again" (`review_again`), never asked behind their back.
    var reviewUnread: Bool {
        switch reviewState {
        case .loading, .failed: return false
        case .ready(let review):
            return review["state"]?["verdict"]?.kind == "unavailable"
                || review["simulation"]?["kind"]?.string == "unreachable"
        }
    }

    /// r10-8: the core's reading names the call ("Approve · USDC", "Batch
    /// of 1 call"), as on the web and the desktop; else the proposal's line.
    var title: String { words.title(item, safe: multisig.isEmpty ? nil : multisig, review: review) }

    /// The chip under the title: a finished one says how it ended, never a
    /// count against today's members (r4a-8, r8-17).
    var statusLine: String {
        if isDone, executedTx != nil { return words.t("proposal.executed") }
        if isDone, replaced { return words.t("proposal.voided") }
        // r16-8: being executed from here — the press's passkey and its send,
        // before the chain has it — is "Executing…", never the mailbox's
        // "1/2 approved": the one press carries the last approval itself.
        if executingHere { return words.t("status.executing") }
        return words.standing(standing, item: item)
    }

    /// r16-8: this device is executing it now (its Execute or one press is
    /// out, or the core follows its flight).
    var executingHere: Bool {
        !isDone && (executeState?.kind == "working" || item.executing)
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

    /// A signing control may be pressed: the core's door is open (§12 —
    /// the review is read and its sign rule allows it). Nothing else opens
    /// it: not a review still being read, not one that failed, not a
    /// review without a sign rule.
    var signable: Bool { door.enabled }
    /// Only with the "… anyway" friction (the door's `anyway`).
    var signAnyway: Bool { door.anyway }

    /// The label a signing control wears: "… anyway" when the core asks
    /// for that friction (the web's `anywayLabel`).
    func signLabel(_ plain: String, anyway: String) -> String {
        words.t(signAnyway ? anyway : plain)
    }

    /// What the "… anyway" step names before it signs: the review's
    /// concerns, in its words — said again where the person confirms.
    var anywayReasons: [String] {
        guard let review else { return [] }
        return MsReviewBody.reasons(review, words: words)
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
        // r9-9: a refusal is said once, by the review's body in the core's
        // words; with no review to draw, the review's place says why (§12,
        // `MsReviewPending`) and nothing is signed.
        if item.cancelledBy != nil, item.action?.kind != "cancel" {
            out.append((words.t("proposal.cancelPending"), .warning, "cancelPending"))
        }
        if !item.sameNonce.isEmpty, !isDone {
            out.append((words.t("proposal.conflict", ["nonce": item.nonce]), .warning, "conflict"))
        }
        if item.status == "queued" {
            // r11 (core `waits_for`): every number it waits behind.
            switch item.waitsFor.count {
            case 0: break
            case 1:
                out.append((words.t("proposal.queuedNote", ["nonce": item.waitsFor[0]]), .neutral, "queued"))
            default:
                out.append((words.t("proposal.queuedNoteMany",
                                    ["nonces": words.loc.list(item.waitsFor.map { "#\($0)" })]), .neutral, "queued"))
            }
        }
        if item.renameUndeployed {
            out.append((words.t("proposal.renameUndeployed", ["network": words.network(chainId)]), .warning, "renameUndeployed"))
        }
        // r12 S4: "Can't run now — …" is the chip under the title
        // (`statusLine`, the row's one word); a note beside it said the same
        // fact twice, a third time with the fee line's "would fail".
        if let entry = outboxEntry {
            // r8-16: on its way now is no warning — nothing was refused.
            out.append((outboxLine(entry), entry["sending"]?.bool == true ? .neutral : .warning, "outbox"))
        }
        if let line = executionLine { out.append(line) }
        // r15-5: why a press did not go is said beside the button pressed
        // (`pressFailures`); here only when no button is drawn for it.
        out.append(contentsOf: pressFailures(at: .top).map { ($0.text, $0.tone, $0.id) })
        // r16-5: a Reject's failure is said beside Reject (`rejectLine`);
        // here only when no Reject is drawn.
        if !offersReject, let line = rejectLine {
            out.append((line.text, line.tone, "rejectFailed"))
        }
        // r9-9: "none is signed in on this device" only when the core says
        // no member of it is here (`members_here`), whatever this one asks.
        if !isDone, item.membersHere?.isEmpty ?? (canApprove.isEmpty && !approvedByHere && account != nil
            && (account?.owners.count ?? 0) > 0 && canExecute.isEmpty) {
            out.append((words.t("proposal.notMember"), .neutral, "notMember"))
        }
        return out.map { (text: $0.0, tone: $0.1, id: $0.2) }
    }

    /// r15-5: where a press's failure is said — beside the button that was
    /// pressed, never only at the head of a long sheet the person has
    /// scrolled away from (the fee-moved stop on iOS read as "nothing
    /// happened"): Approve's (and the one press's) under Approve, Execute's
    /// under Execute, at the head only when no such button is drawn.
    enum PressArea { case approve, execute, top }

    /// Where Execute's buttons and their notes are drawn.
    var executeArea: PressArea {
        if isDone { return .top }
        if isReady, !canExecute.isEmpty { return .execute }
        return canApprove.isEmpty ? .top : .approve
    }

    /// Where Approve's notes are drawn.
    var approveArea: PressArea { !isDone && !canApprove.isEmpty ? .approve : .top }

    /// Why the last Approve / Execute (or one press) did not go, for `area`
    /// — the core's words (`actionFailure`), each said once.
    func pressFailures(at area: PressArea) -> [(text: String, tone: MsNote.Tone, id: String)] {
        var out: [(text: String, tone: MsNote.Tone, id: String)] = []
        if approveArea == area, let failure = words.actionFailure(approveState, chainId: chainId) {
            out.append((failure, .danger, "approveFailed"))
        }
        if executeArea == area, let failure = words.actionFailure(executeState, chainId: chainId),
           !out.contains(where: { $0.text == failure }) {
            out.append((failure, .danger, "executeFailed"))
        }
        return out
    }

    /// r16-4: the one press does both or nothing (the core, §26). A fee that
    /// could not be worked out or checked — its look, or its stop at the
    /// press (`stopped {unpriced | unchecked}`) — offers "Check again" (a new
    /// look) under the one press, as under Execute and on Android and the web.
    func oneTouchChecksFee(_ presser: String) -> Bool {
        let unlooked: Set<String> = ["unpriced", "unchecked"]
        if let stop = executeState, stop.kind == "stopped" { return unlooked.contains(stop["preview"]?.kind ?? "") }
        return unlooked.contains(preview(for: presser)?.kind ?? "")
    }

    /// r16-5 (Android's finding, iOS parity): how the last Reject ended, said
    /// beside Reject with Try again — never only at the head of a sheet the
    /// person has scrolled to its foot.
    /// Reject is drawn: a member here can act on it, and it is no
    /// cancellation and has none waiting.
    var offersReject: Bool {
        !isDone && (!canApprove.isEmpty || approvedByHere || !canExecute.isEmpty)
            && item.action?.kind != "cancel" && item.cancelledBy == nil
    }

    var rejectLine: (text: String, tone: MsNote.Tone)? {
        guard let state = rejectState, let failure = words.actionFailure(state, chainId: chainId) else { return nil }
        return (failure, ["unread", "mailboxUnreadable"].contains(state.kind) ? .warning : .danger)
    }

    /// r15-14: "The network fee is paid by the executing member's account."
    /// is said once — under the one press when it is offered, else where
    /// Execute would be while it is not ready.
    func feeNoteUnderExecute(oneTouchOffered: Bool) -> Bool { !isReady && !oneTouchOffered }

    /// One of this device's members approved it already.
    var approvedByHere: Bool {
        !item.canApprove.isEmpty ? false : item.approvals.contains { approval in
            (view.chain(chainId)?.members?.owners ?? []).contains { $0.lowercased() == approval.owner.lowercased() }
                && words.localName(approval.owner) != nil
        }
    }

    /// This transaction's entry in the machine's outbox (signed here, not in
    /// the mailbox yet). Its key is `chainId:safe:safeTxHash:owner`.
    var outboxEntry: MsJSON? {
        guard item.unsent else { return nil }
        let marker = ":\(hash.lowercased()):"
        return view.outbox.first {
            let key = $0.key.lowercased()
            return key.hasPrefix("\(chainId):") && key.contains(marker)
        }?.value
    }

    /// "Send again" only when nothing is on its way and no retry is due:
    /// the web's (r8-16).
    var offersSendAgain: Bool {
        guard item.unsent else { return false }
        guard let entry = outboxEntry else { return true }
        return entry["sending"]?.bool != true && entry["retryAt"]?.int == nil
    }

    func outboxLine(_ entry: MsJSON) -> String {
        if entry["sending"]?.bool == true || entry["why"]?.string == "sending" {
            return words.t("proposal.outboxSending")
        }
        // Sent again on the person's word: "Not sent yet" (with Send again).
        guard let retryAt = entry["retryAt"]?.int else { return words.t("proposal.outboxNotSent") }
        let seconds: Int? = max(1, (retryAt - Int(Date().timeIntervalSince1970 * 1000) + 999) / 1000)
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
        // Real-device S4: "Executed" is said once, by the executed row with
        // its explorer link (`executedRow`, from the same hash) — never a
        // second line above it.
        case "executed": return executedTx == nil ? (words.t("proposal.executed"), .positive, "executed") : nil
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
            let feeText = "\(MsRules.formatUnits(amount, decimals: MsWords.coinDecimals)) \(symbol)"
            if preview["canPay"]?.bool == false {
                return (words.t("proposal.cannotPay", [
                    "name": words.label(executor), "symbol": symbol, "fee": feeText,
                    "balance": "\(MsRules.formatUnits(preview["balance"]?.string ?? "0", decimals: MsWords.coinDecimals)) \(symbol)",
                ]), .warning)
            }
            return (words.t("proposal.feeLine", ["fee": feeText, "name": words.label(executor)]), .neutral)
        case "fails": return (words.t("proposal.wouldFail", ["reason": words.whyFails(preview["why"], chainId: chainId)]), .danger)
        case "unpriced": return (words.t("proposal.feeUnpriced"), .warning)
        // D8 (real-device pass 2): "you can still approve" only where
        // Approve can be pressed — never under one shut while the review is
        // read (or not read).
        case "unchecked": return (words.t(door.enabled ? "proposal.feeUnchecked" : "proposal.feeUncheckedHeld"), .warning)
        // r14-4: the core tells a setup it could not check (`unverified`:
        // "won't deploy it there yet") from one it read and refused
        // (`refused`: "won't deploy it", and why) — never one for the other.
        case "unverified": return (words.t("authority.deployUnverified", ["network": words.network(chainId)]), .warning)
        case "refused": return (words.deployRefused(preview["authority"], chainId: chainId), .danger)
        case "spent": return (words.t("proposal.doneNote"), .neutral)
        default: return nil
        }
    }

    /// §24: the fee figure this sheet draws for `executor`'s execution (base
    /// units, as the core gave it) — what a press under it carries as
    /// `shown_fee`, so the core signs exactly that figure or stops with
    /// `fee_moved` (§19.11). `nil` when no figure is drawn: no priced look,
    /// or the stop's own line stands in the fee line's place. Read from the
    /// model the sheet painted, never from a view not drawn yet.
    func shownFee(for executor: String) -> String? {
        guard !executeStopped, let look = preview(for: executor), look.kind == "priced",
              let amount = look["fee"]?["amount"]?.string, !amount.isEmpty
        else { return nil }
        return amount
    }

    /// §24: the press that executes — Execute, or the one press "Approve and
    /// execute" (`oneTouch`) — carrying the figure drawn with it (`shownFee`).
    func executeEvent(executor: String, oneTouch: Bool) -> [String: Any] {
        var event: [String: Any] = [
            "type": oneTouch ? "approve_execute" : "execute",
            "chain_id": chainId, "safe_tx_hash": hash, "executor": executor,
        ]
        if let fee = shownFee(for: executor) { event["shown_fee"] = fee }
        return event
    }

    /// core-ms-2: whose ONE press approves and executes it — the member
    /// picked, when the machine offers it that press, else the first it
    /// offers; `nil` when it offers none. Its look is the fee said before
    /// the press (r9 S4).
    func oneTouchExecutor(chosen: String?) -> String? {
        let offered = view.approveExecute(item)
        if let chosen, let match = offered.first(where: { $0.caseInsensitiveCompare(chosen) == .orderedSame }) {
            return match
        }
        return offered.first
    }

    /// r10 S4: why the member's press approves only, when theirs would be
    /// the last approval needed — the machine offered no one press because
    /// its look at them says they cannot pay, the call would fail, or it may
    /// not be executed here. Said before the person presses (the web's
    /// `execute-cant-pay`); `nil` when one press is offered, when their
    /// approval would not complete it, or while nothing has been looked at
    /// (the look is asked; Approve stands meanwhile, as on the web).
    func approveOnlyReason(chosen: String?) -> (text: String, tone: MsNote.Tone)? {
        guard !isDone, view.approveExecute(item).isEmpty else { return nil }
        guard let approver = chosen ?? defaultMember,
              canApprove.contains(where: { $0.caseInsensitiveCompare(approver) == .orderedSame }),
              canExecute.contains(where: { $0.caseInsensitiveCompare(approver) == .orderedSame }),
              item.approved + 1 >= item.threshold,
              let look = preview(for: approver)
        else { return nil }
        switch look.kind {
        case "priced" where look["canPay"]?.bool == false:
            // Another member here who can pay is named, as on the web.
            if let other = canExecute.first(where: { executor in
                executor.caseInsensitiveCompare(approver) != .orderedSame
                    && preview(for: executor)?.kind == "priced" && preview(for: executor)?["canPay"]?.bool != false
            }) {
                return (words.t("proposal.cantPayOther", ["name": words.label(approver), "other": words.label(other)]), .warning)
            }
            return feeLine(for: approver)
        case "fails", "refused", "unverified":
            return feeLine(for: approver)
        default:
            return nil
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
            words.t("proposal.feeSetup", ["network": words.network(chainId), "amount": words.coinAmount($0, chainId: chainId)])
        } ?? words.t("proposal.feeSetupPlain", ["network": words.network(chainId)])
        return [head] + lines
    }

    /// The preview stops it (would fail, nobody can pay): Execute waits.
    func executeHeld(by executor: String) -> Bool {
        guard let preview = preview(for: executor) else { return false }
        if ["fails", "refused", "unverified", "spent"].contains(preview.kind ?? "") { return true }
        return preview.kind == "priced" && preview["canPay"]?.bool == false
    }

    // MARK: - Asking the others (`signers`)

    /// The members still to approve (verified approvals count; nothing once
    /// it can no longer move).
    var stillNeeded: [String] {
        guard !past, !refusedForWhatItIs else { return [] }
        let have = Set(item.approvals.filter(\.counts).map { $0.owner.lowercased() })
        return members.filter { !have.contains($0.lowercased()) }
    }

    /// r11 S4: Vela refuses it for what it is (a delegatecall, a refund, a
    /// batch too deep) — the core leaves nobody who can approve it
    /// (`can_approve` empty) and says why (`refused`): no member is asked.
    var refusedForWhatItIs: Bool {
        guard item.canApprove.isEmpty, let refused = review?["refused"] else { return false }
        return !refused.isNull
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
            out.append((words.t("proposal.approvesInVela", ["names": words.loc.list(inVela.map(words.label))]), "inVela"))
        }
        if !either.isEmpty {
            out.append((words.t("proposal.eitherWay", ["names": words.loc.list(either.map(words.label))]), "either"))
        }
        return out.map { (text: $0.0, id: $0.1) }
    }

    /// r7: smart-contract members still to approve, said honestly.
    var contractLine: String? {
        contract.isEmpty ? nil
            : words.t("proposal.contractMembers", ["names": words.loc.list(contract.map(words.label))])
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

    /// r9-7: a member still to approve whom the machine's split has not
    /// placed yet — its answer not in, or a read that did not answer (a
    /// network out of reach, a read bounded right after this device's own
    /// proposal, when every look goes out at once). The sheet asks the
    /// machine again (`read_signers`); nothing is offered on a guess
    /// meanwhile. This device's own members are never read.
    var signersUnread: Bool {
        guard !past, !enough else { return false }
        let others = stillNeeded.filter { words.localName($0) == nil && !canApprove.contains($0) }
        guard !others.isEmpty else { return false }
        guard let signers else { return true }
        return others.contains { owner in
            signers.rows.first { $0.owner.caseInsensitiveCompare(owner) == .orderedSame }?.kind == nil
        }
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

    /// When a step happened, as the web's timeline says it under the step
    /// (`formatDateTime` of the machine's `at_ms`) — nothing when the machine
    /// has no time for it (r9 S4: History had no timestamps).
    func timelineWhen(_ step: MsTagged) -> String? {
        guard let ms = step["at_ms"]?.double, ms > 0 else { return nil }
        return Formats.dateTime(Date(timeIntervalSince1970: ms / 1000))
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

    // §12: how long a review is read before it is `failed {timedOut}` is
    // the core's bound (`REVIEW_TIMEOUT_MS`), never this sheet's.
    /// How often an open sheet asks for its review again (the web's
    /// `REVIEW_FRESH_MS`), and while the last could not read the chain
    /// (`REVIEW_RETRY_MS`).
    static let reviewFreshSeconds: Double = 30
    static let reviewRetrySeconds: Double = 5
    /// r9-7: while a member still to approve is unplaced, the split is asked
    /// again this often (past the core's own bound on one read,
    /// `SIGNER_READ_MS`, 8 s), at most `signersRetries` times per change.
    static let signersRetrySeconds: Double = 10
    static let signersRetries = 6

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
                content(MsProposalModel(
                    item: item, view: view, words: words, account: hub.accountsView.activeRow?.account,
                    rejectState: hub.rejectFailure(chainId: chainId, safeTxHash: item.safeTxHash)
                ))
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
        // r9-7: the approvals that count, too (the web's `signersKey` over
        // the members still to approve): this device's own proposal lands
        // with its approval, and the split is asked for again then.
        let counted = item.approvals.filter(\.counts).map { $0.owner.lowercased() }.sorted().joined(separator: ",")
        return "\(item.safeTxHash.lowercased()):\(item.status):\(members?.owners.joined(separator: ",") ?? ""):\(members?.threshold ?? 0):\(counted)"
    }

    /// The review now, again every `reviewFreshSeconds` while the sheet is
    /// up and every `reviewRetrySeconds` while the last could not read the
    /// chain; the signers once per change; a finished one's facts once — and
    /// its review once (its hashes and fields; nothing is re-checked).
    private func keepAsking() async {
        guard let item else { return }
        askReview()
        hub.sendProposal(["type": "read_signers", "chain_id": chainId, "safe_tx_hash": item.safeTxHash])
        if item.status == "done" {
            hub.sendProposal(["type": "timeline", "chain_id": chainId, "safe_tx_hash": item.safeTxHash])
            return
        }
        var lastAsk = ContinuousClock.now
        var lastSignersAsk = ContinuousClock.now
        var signersAsked = 0
        while !Task.isCancelled {
            try? await Task.sleep(for: .seconds(1))
            guard let current = self.item else { continue }
            let model = MsProposalModel(item: current, view: view, words: words, account: nil)
            // r9-7: a member the split has not placed is asked for again —
            // bounded, so a network that never answers costs a few reads.
            if model.signersUnread, signersAsked < Self.signersRetries,
               ContinuousClock.now - lastSignersAsk >= .seconds(Self.signersRetrySeconds) {
                hub.sendProposal(["type": "read_signers", "chain_id": chainId, "safe_tx_hash": current.safeTxHash])
                lastSignersAsk = ContinuousClock.now
                signersAsked += 1
            }
            // §12: a review being read is the core's to bound, and one that
            // failed waits for the person's "Try again" — asked again here
            // only once read (fresh, or sooner while part of it is unread).
            guard case .ready = model.reviewState else { continue }
            let since = ContinuousClock.now - lastAsk
            let due = model.reviewUnread ? Self.reviewRetrySeconds : Self.reviewFreshSeconds
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

    /// §12: the person's "Try again" on a review that was not read.
    private func reviewAgain() {
        guard let item else { return }
        hub.sendProposal(["type": "review_again", "chain_id": chainId, "safe_tx_hash": item.safeTxHash])
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

    /// §18 (b): the core's words for a sheet opened without a link whose
    /// transaction is not in the list (`nil` while the multisig is not one
    /// this sheet's network is read for yet and nothing is loading).
    private var missingSaid: (key: String, retry: Bool)? {
        MsRules.missingSaid(
            loading: view.loading,
            unreadable: view.unreadable.contains(chainId),
            mailbox: view.chains.first(where: { $0.chainId == chainId })?.mailbox
        )
    }

    @ViewBuilder private var linkedState: some View {
        let linked = view.linked
        if let failure = linked?.failure(words) {
            // r13-3 / §18 item 1: a read that failed (`failed`, `unreadable`,
            // `notFound`) — the read named, and Try again (said only when the
            // core offers it); never "Opening the transaction…" for ever,
            // never "it hasn't been shared".
            MsNote(text: failure, tone: .warning, identifier: "multisig.proposal.openFailed")
            VelaButton(title: words.t("review.state.tryAgain"), kind: .secondary) { hub.retryLinked() }
                .accessibilityIdentifier("multisig.proposal.openRetry")
        } else if linked?.stand == "notInMailbox" {
            MsNote(text: words.t("proposal.notInMailbox"), tone: .warning, identifier: "multisig.proposal.notInMailbox")
        } else if linked == nil, let missing = missingSaid, missing.key != "proposal.opening" {
            // §18 (b), round 14: opened without a link and not in the list —
            // the core's words for why (a silent service is said as that,
            // with Try again), never an endless "Opening the transaction…".
            MsNote(
                text: words.t(missing.key, ["network": words.network(chainId)]),
                tone: .warning, identifier: "multisig.proposal.missing"
            )
            if missing.retry {
                VelaButton(title: words.t("review.state.tryAgain"), kind: .secondary) { hub.retryLinked() }
                    .accessibilityIdentifier("multisig.proposal.missingRetry")
            }
        } else {
            HStack(spacing: Tokens.Space.s8) {
                ProgressView()
                Text(prose: words.t("proposal.opening"))
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
            Text(prose: model.title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .fixedSize(horizontal: false, vertical: true)
                .accessibilityAddTraits(.isHeader)
                .accessibilityIdentifier("multisig.proposal.title")
            HStack(spacing: Tokens.Space.s8) {
                MsChip(text: model.statusLine, tone: model.standing?.kind == "blocked" ? .warning : .neutral)
                    .accessibilityIdentifier("multisig.proposal.status")
                Text(prose: "\(words.network(chainId)) · \(words.t("action.numberValue", ["nonce": model.item.nonce]))")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        // ms-21 / r8-8: a link's transaction read on a device with none of its
        // members — said, and nothing was added or switched.
        if let linked = view.linked, linked.preview,
           linked.safeTxHash.caseInsensitiveCompare(model.hash) == .orderedSame {
            MsNote(text: words.t("proposal.previewNote"), identifier: "multisig.proposal.preview")
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

    /// The one review, in its states (§12, the core's door): a finished
    /// one draws only what was signed (nothing is re-checked); one being
    /// read says so calmly — bounded by the core, which says `failed` past
    /// it (r8-19: never a spinner without end); one not read says why, that
    /// nothing can be signed, and "Try again".
    @ViewBuilder private func reviewPart(_ model: MsProposalModel) -> some View {
        switch model.reviewState {
        case .ready(let review):
            // D1: neither safety check read — the door's danger line and
            // Try again, above the review it still draws.
            if !model.past, model.door.unchecked {
                MsReviewPending(door: model.door, words: words, network: words.network(chainId), tryAgain: reviewAgain)
            }
            MsReviewBody(review: review, chainId: chainId, words: words, past: model.past)
        case .loading, .failed:
            if !model.past {
                MsReviewPending(door: model.door, words: words, network: words.network(chainId), tryAgain: reviewAgain)
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
                            let short = MsWords.short(member)
                            let label = words.label(member)
                            // An unnamed member is its address, said once.
                            Text(prose: label)
                                .typeRole((label == short ? Typography.monoSmall : Typography.fieldLabel).scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                            if label != short {
                                Text(prose: short)
                                    .typeRole(Typography.monoSmall.scaled(textScale))
                                    .foregroundStyle(theme.fgSubtle)
                            }
                        }
                        Spacer(minLength: Tokens.Space.s8)
                        Text(prose: words.t(word.key))
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
                        // r9 S4: the fee before the passkey — the machine's
                        // look for the member whose press executes (the web
                        // draws the executor's priced look under its one press).
                        let presser = model.oneTouchExecutor(chosen: member) ?? oneTouch[0]
                        if !model.executeStopped, let line = model.feeLine(for: presser) {
                            MsNote(text: line.text, tone: line.tone, identifier: "multisig.proposal.approveExecuteFee")
                        }
                        ForEach(model.setupLines(for: presser), id: \.self) { line in
                            MsNote(text: line)
                        }
                        VelaButton(
                            title: model.signLabel("proposal.approveAndExecute", anyway: "review.approveExecuteAnyway"),
                            kind: model.signAnyway ? .danger : .primary,
                            enabled: model.signable,
                            loading: approving || executing,
                            haptic: .confirm
                        ) {
                            let executor = model.oneTouchExecutor(chosen: member) ?? oneTouch[0]
                            // §24: the press carries the fee drawn above it (`shown_fee`).
                            signing(model, .approveExecute) {
                                hub.sendProposal(model.executeEvent(executor: executor, oneTouch: true))
                            }
                        }
                        .accessibilityIdentifier("multisig.proposal.approveExecute")
                        MsNote(text: words.t("proposal.feeNote"))
                    } else if let why = model.approveOnlyReason(chosen: member) {
                        // r10 S4: Approve alone, where theirs completes it —
                        // said why (cannot pay, would fail), never silently.
                        MsNote(text: why.text, tone: why.tone, identifier: "multisig.proposal.approveOnly")
                    }
                    VelaButton(
                        // Approving alone beside the one press is the second
                        // choice; either way it wears the review's rule — D1:
                        // "Approve anyway" in danger, never a plain Approve
                        // under a review nothing checked.
                        title: model.signLabel("proposal.approve", anyway: "review.approveAnyway"),
                        kind: model.signAnyway ? .danger : (completes ? .secondary : .primary),
                        enabled: model.signable,
                        loading: approving && !completes,
                        haptic: .confirm
                    ) {
                        signing(model, .approve) { approve(model) }
                    }
                    .accessibilityIdentifier("multisig.proposal.approve")
                    // r15-5: why the press did not go, here where it was pressed
                    // (the fee-moved stop: the new figure, for the next press).
                    pressFailures(model, at: .approve)
                    // r16-4: a fee not worked out (the look's, or the one press's
                    // stop) is asked again here — never a press with nothing to do.
                    if completes, model.signable,
                       model.oneTouchChecksFee(model.oneTouchExecutor(chosen: member) ?? oneTouch[0]) {
                        Button(words.t("proposal.feeRetry")) { askPreviews(force: true) }
                            .font(Typography.actionLabel.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                            .accessibilityIdentifier("multisig.proposal.oneTouchFeeRetry")
                    }
                }
                if model.isReady, !model.canExecute.isEmpty {
                    let chosen = executor ?? model.canExecute.first ?? ""
                    if model.canExecute.count > 1 {
                        memberPicker(model.canExecute, selection: Binding(get: { chosen }, set: { executor = $0 }))
                    }
                    if !model.executeStopped, let line = model.feeLine(for: chosen) {
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
                    pressFailures(model, at: .execute)
                    if ["unpriced", "unchecked", "unverified"].contains(model.preview(for: chosen)?.kind ?? "") {
                        Button(words.t("proposal.feeRetry")) { askPreviews(force: true) }
                            .font(Typography.actionLabel.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                    }
                } else if model.feeNoteUnderExecute(oneTouchOffered: !model.canApprove.isEmpty && !model.view.approveExecute(model.item).isEmpty) {
                    // r15-14: said once — the one press says it under itself.
                    MsNote(text: words.t("proposal.feeNote"))
                }
                if model.offersSendAgain {
                    VelaButton(title: words.t("proposal.outboxSendAgain"), kind: .ink) {
                        hub.sendProposal(["type": "send_again", "chain_id": chainId, "safe_tx_hash": model.hash])
                    }
                    .accessibilityIdentifier("multisig.proposal.sendAgain")
                }
                if model.offersReject {
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
                            Text(prose: words.t("proposal.rejectHint", count: model.item.threshold, [
                                "threshold": String(model.item.threshold), "count": String(model.item.threshold),
                            ]))
                        }
                    // r16-5: why the last Reject did not go, beside it, with Try again.
                    if let line = model.rejectLine {
                        HStack(alignment: .top, spacing: Tokens.Space.s12) {
                            MsNote(text: line.text, tone: line.tone, identifier: "multisig.proposal.rejectFailed")
                            Button(words.t("review.state.tryAgain")) { reject(model) }
                                .font(Typography.actionLabel.scaled(textScale).font)
                                .foregroundStyle(theme.fgBase)
                                .accessibilityIdentifier("multisig.proposal.rejectRetry")
                        }
                    }
                }
            }
            .padding(.top, Tokens.Space.s8)
        }
    }

    /// r15-5: the failures said beside the buttons of `area`.
    @ViewBuilder private func pressFailures(_ model: MsProposalModel, at area: MsProposalModel.PressArea) -> some View {
        ForEach(model.pressFailures(at: area), id: \.id) { note in
            MsNote(text: note.text, tone: note.tone, identifier: "multisig.proposal.note.\(note.id)")
        }
    }

    private func memberPicker(_ members: [String], selection: Binding<String?>) -> some View {
        Picker(words.t("proposal.as"), selection: selection) {
            ForEach(members, id: \.self) { address in
                Text(prose: words.label(address)).tag(Optional(address))
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
            // A container of its own: its rows keep their identifiers.
            .accessibilityElement(children: .contain)
            .accessibilityIdentifier("multisig.proposal.invite")
        }
        if let link = model.outsideLink {
            // r4C-13: an outside member's own link — Vela's connect page, which
            // reads, reviews and signs the transaction alone.
            MsSection(title: words.t("external.outsideLinkTitle")) {
                MsNote(text: words.t("external.outsideLinkHint"))
                Text(prose: words.t("proposal.stillNeeded", ["missing": words.loc.list(model.outside.map(words.label))]))
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
            .accessibilityElement(children: .contain)
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
        let lines = model.timeline.compactMap { step in
            model.timelineLine(step).map { (text: $0, when: model.timelineWhen(step)) }
        }
        if !lines.isEmpty {
            MsSection(title: words.t("timeline.heading")) {
                ForEach(Array(lines.enumerated()), id: \.offset) { _, line in
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(prose: line.text)
                            .typeRole(Typography.rowSub.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                            .fixedSize(horizontal: false, vertical: true)
                        if let when = line.when {
                            Text(prose: when)
                                .typeRole(Typography.flowCaption.scaled(textScale))
                                .foregroundStyle(theme.fgSubtle)
                                .accessibilityIdentifier("multisig.proposal.history.when")
                        }
                    }
                    .accessibilityElement(children: .combine)
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

    /// §24: `model` is the sheet as painted when pressed — its fee figure
    /// travels with the press (`shown_fee`); a quote that landed since
    /// signs nothing (`fee_moved`, said beside Execute).
    private func execute(_ model: MsProposalModel, executor: String) {
        hub.sendProposal(model.executeEvent(executor: executor, oneTouch: false))
    }

    /// r9-13: the cancellation's review first (the hub's review sheet, the
    /// member acting here offered there); signed only once proposed there.
    private func reject(_ model: MsProposalModel) {
        let owner = member ?? model.defaultMember ?? model.canExecute.first
        Task { @MainActor in
            await hub.reject(chainId: chainId, safeTxHash: model.hash, nonce: model.item.nonce, as: owner)
        }
    }

    private func close() {
        if view.linked != nil { hub.sendProposal(["type": "close_linked"]) }
        onClose()
    }
}

// MARK: - A link's read that failed (§18 item 1)

extension MsLinkedView {
    /// The core offers Try again (`retry`): every stand whose read failed —
    /// `failed`, `unreadable`, `notFound` — on every shell alike.
    var offersTryAgain: Bool { retry }

    /// The core's words for the read that failed (`reason_key`), or `nil`
    /// when the core offers no Try again (reading, found, `notInMailbox`).
    func failure(_ words: MsWords) -> String? {
        guard retry, let reasonKey else { return nil }
        return words.unreadText(reasonKey, chainId: chainId)
    }
}
