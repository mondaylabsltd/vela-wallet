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
                Text(prose: words.title(item, review: draft.review.isNull ? nil : draft.review))
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("multisig.draft.title")
                Text(prose: "\(words.network(item.chainId)) · \(words.t("action.numberValue", ["nonce": item.nonce]))")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                // §12: the review once read; until then (or when it could
                // not be), the core's door says what stands in its place.
                // r9-9: why it is not signed is the review's one line (its
                // body), never the core's English reason.
                let door = Self.door(draft)
                if door.shows == .review, !draft.review.isNull {
                    // D1: neither safety check read — the danger line and
                    // Try again above the review it still draws.
                    if door.unchecked {
                        MsReviewPending(door: door, words: words) {
                            hub.sendProposal(["type": "review_again"])
                        }
                    }
                    MsReviewBody(review: draft.review, chainId: item.chainId, words: words)
                } else {
                    MsReviewPending(door: door, words: words, network: words.network(item.chainId)) {
                        hub.sendProposal(["type": "review_again"])
                    }
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

    /// §12: the core's sign door over the draft's review — "Propose and
    /// approve" (and "Approve and execute") opens only on its `enabled`.
    static func door(_ draft: MsDraft) -> MsSignDoor {
        MsRules.reviewDoor(state: draft.reviewState, review: draft.review)
    }

    @ViewBuilder private func actions(_ draft: MsDraft) -> some View {
        let rule = Self.door(draft)
        let allowed = rule.enabled
        let anyway = rule.anyway
        let working = state?.kind == "working"
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if draft.membersHere.count > 1 {
                Picker(words.t("proposal.as"), selection: Binding(
                    get: { member ?? first(draft) ?? draft.membersHere.first },
                    set: { member = $0 }
                )) {
                    ForEach(draft.membersHere, id: \.self) { address in
                        Text(prose: words.label(address)).tag(Optional(address))
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
    private func signing(_ rule: MsSignDoor, title: String, draft: MsDraft, _ run: @escaping () -> Void) {
        guard rule.enabled else { return }
        if rule.anyway {
            anyway = MsAnywayStep(title: title, reasons: MsReviewBody.reasons(draft.review, words: words), run: run)
        } else {
            run()
        }
    }
}

extension MsReviewBody {
    /// What an "… anyway" step names before it signs: the review's
    /// concerns, each in its words.
    @MainActor
    static func reasons(_ review: MsJSON?, words: MsWords) -> [String] {
        (review?["concerns"]?.array ?? []).compactMap { concernText($0, review: review, words: words) }
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
            Text(prose: pending.reasons.joined(separator: "\n"))
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
                Text(prose: words.finding(finding))
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
