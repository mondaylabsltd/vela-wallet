//
//  MultisigDraftSheets.swift
//  VelaWallet
//
//  Two sheets a signature may wait on:
//
//  - the review-first sheet (§3.6): a proposal the machine drew — a send
//    from the multisig, a rename, a member change, a dApp's calls — shown
//    with its one review BEFORE any passkey; the person proposes it (as one
//    of this device's members) or withdraws it, and nothing is signed until
//    they do;
//  - the high-risk question (§3.5): on this network something besides the
//    members can act for the multisig. Signing anyway is the person's word.
//

import SwiftUI

struct MultisigDraftReviewSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let chainId: Int
    let words: MsWords
    /// A site's request (M5): who asks, said first.
    var siteNote: String?
    /// The person's way out, when it is a request's to answer (M5).
    var onWithdraw: (() -> Void)?

    @State private var member: String?
    /// The "… anyway" step the core asked for, while it is up (r8-1).
    @State private var anyway: MsAnywayStep?

    private var draft: MsDraft? { hub.proposalsView.draft }
    private var state: MsTagged? { hub.proposalsView.action("propose") }

    var body: some View {
        MsSheet(title: words.t("proposal.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: withdraw) {
            if let siteNote { MsNote(text: siteNote, identifier: "multisig.draft.site") }
            if let draft {
                let item = draft.item
                Text(verbatim: words.title(item, review: draft.review.isNull ? nil : draft.review))
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("multisig.draft.title")
                Text(verbatim: "\(words.network(item.chainId)) · \(words.t("action.numberValue", ["nonce": item.nonce]))")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                // r9-9: why it is not signed is the review's one line (its
                // body below), never the core's English reason.
                if let refusal = item.refusal, !refusal.isEmpty, draft.review.isNull {
                    MsNote(text: words.refusalLine(nil), tone: .danger)
                }
                if !draft.review.isNull {
                    MsReviewBody(review: draft.review, chainId: item.chainId, words: words)
                }
                if let failure = words.actionFailure(state, chainId: item.chainId) {
                    MsNote(text: failure, tone: .danger, identifier: "multisig.draft.failed")
                }
                actions(draft)
            } else {
                ProgressView()
            }
        }
        .interactiveDismissDisabled(state?.kind == "working")
        .msAnywayStep($anyway, cancel: words.t("proposal.rejectCancel"))
    }

    @ViewBuilder private func actions(_ draft: MsDraft) -> some View {
        let rule = MsSignRule(review: draft.review)
        let allowed = rule.allowed
        let anyway = rule.anyway
        let working = state?.kind == "working"
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if draft.membersHere.count > 1 {
                Picker(words.t("proposal.as"), selection: Binding(
                    get: { member ?? first(draft) ?? draft.membersHere.first },
                    set: { member = $0 }
                )) {
                    ForEach(draft.membersHere, id: \.self) { address in
                        Text(verbatim: words.label(address)).tag(Optional(address))
                    }
                }
                .pickerStyle(.menu)
                .tint(theme.fgBase)
                .accessibilityIdentifier("multisig.draft.as")
            }
            // core-ms-2: ready on its proposer's approval (a site's
            // transaction) — proposed and executed in one press.
            if draft.executeNow, !draft.membersHere.isEmpty {
                let title = words.t(anyway ? "review.approveExecuteAnyway" : "proposal.approveAndExecute")
                VelaButton(
                    title: title,
                    kind: anyway ? .danger : .primary,
                    enabled: allowed,
                    loading: working,
                    haptic: .confirm
                ) {
                    signing(rule, title: title, draft: draft) {
                        hub.sendProposal(["type": "propose_execute", "as_member": (member ?? first(draft)).map { $0 as Any } ?? NSNull()])
                    }
                }
                .accessibilityIdentifier("multisig.draft.proposeExecute")
                MsNote(text: words.t("proposal.feeNote"))
            }
            let proposeTitle = words.t(draft.membersHere.isEmpty ? "send.proposeOnly" : (anyway ? "review.proposeAnyway" : "send.propose"))
            VelaButton(
                title: proposeTitle,
                kind: draft.executeNow && !draft.membersHere.isEmpty ? .secondary : (anyway ? .danger : .primary),
                enabled: allowed,
                loading: working && !draft.executeNow,
                haptic: .confirm
            ) {
                signing(rule, title: words.t(draft.membersHere.isEmpty ? "send.proposeOnly" : "review.proposeAnyway"), draft: draft) {
                    hub.confirmDraft(as: member ?? first(draft))
                }
            }
            .accessibilityIdentifier("multisig.draft.propose")
            VelaButton(title: words.t("proposal.dontPropose"), kind: .secondary, enabled: !working) {
                withdraw()
            }
            .accessibilityIdentifier("multisig.draft.withdraw")
        }
        .padding(.top, Tokens.Space.s8)
    }

    /// The member offered first: the one the person was acting as (a Reject's
    /// member, r9-13) when it is one of this device's members of it, else
    /// the machine's default.
    private func first(_ draft: MsDraft) -> String? {
        Self.firstMember(draft, acting: hub.reviewAs)
    }

    static func firstMember(_ draft: MsDraft, acting: String?) -> String? {
        if let acting, let here = draft.membersHere.first(where: { $0.caseInsensitiveCompare(acting) == .orderedSame }) {
            return here
        }
        return draft.defaultMember
    }

    private func withdraw() {
        if let onWithdraw { onWithdraw() } else { hub.withdrawDraft() }
    }

    /// At once, or — when the core's rule asks for "… anyway" — after the
    /// person confirms past the review's reasons (r8-1).
    private func signing(_ rule: MsSignRule, title: String, draft: MsDraft, _ run: @escaping () -> Void) {
        guard rule.allowed else { return }
        if rule.anyway {
            anyway = MsAnywayStep(title: title, reasons: rule.reasons(draft.review, words: words), run: run)
        } else {
            run()
        }
    }
}

/// The core's sign rule over a review (`MultisigReview.sign`): `allowed`
/// false — nothing is signed, no override; `anyway` — signed only past a
/// danger-styled "… anyway" step that names the review's reasons.
struct MsSignRule: Equatable {
    let allowed: Bool
    let anyway: Bool

    init(review: MsJSON?) {
        allowed = review?["sign"]?["allowed"]?.bool ?? true
        anyway = review?["sign"]?["anyway"]?.bool ?? false
    }

    /// The review's concerns, each in its words.
    @MainActor
    func reasons(_ review: MsJSON?, words: MsWords) -> [String] {
        (review?["concerns"]?.array ?? []).compactMap { MsReviewBody.concernText($0, review: review, words: words) }
    }
}

/// One "… anyway" step waiting on the person.
struct MsAnywayStep: Identifiable {
    let id = UUID()
    let title: String
    let reasons: [String]
    let run: () -> Void
}

extension View {
    /// The "… anyway" step (r8-1): the reasons, then the danger confirm or
    /// a way back. Nothing is signed until the person confirms.
    func msAnywayStep(_ step: Binding<MsAnywayStep?>, cancel: String) -> some View {
        confirmationDialog(
            step.wrappedValue?.title ?? "",
            isPresented: Binding(get: { step.wrappedValue != nil }, set: { if !$0 { step.wrappedValue = nil } }),
            titleVisibility: .visible,
            presenting: step.wrappedValue
        ) { pending in
            Button(pending.title, role: .destructive) {
                step.wrappedValue = nil
                pending.run()
            }
            .accessibilityIdentifier("multisig.anyway.confirm")
            Button(cancel, role: .cancel) { step.wrappedValue = nil }
        } message: { pending in
            Text(verbatim: pending.reasons.joined(separator: "\n"))
        }
    }
}

/// §3.5: the person's word on signing for a high-risk multisig.
struct MultisigRiskSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let ask: MultisigRiskAsk
    let words: MsWords

    var body: some View {
        MsSheet(title: words.t("authority.signRiskTitle"), closeLabel: words.loc.t("common.done"), onClose: { ask.answer(false) }) {
            MsNote(text: words.t("authority.signRiskBody", ["network": words.network(ask.chainId)]), tone: .danger)
            ForEach(Array(ask.findings.enumerated()), id: \.offset) { _, finding in
                Text(verbatim: words.finding(finding))
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
            }
            VelaButton(title: words.t("authority.signRiskConfirm"), kind: .danger, haptic: .confirm) {
                ask.answer(true)
            }
            .accessibilityIdentifier("multisig.risk.sign")
            VelaButton(title: words.t("authority.signRiskCancel"), kind: .secondary) {
                ask.answer(false)
            }
            .accessibilityIdentifier("multisig.risk.cancel")
        }
    }
}
