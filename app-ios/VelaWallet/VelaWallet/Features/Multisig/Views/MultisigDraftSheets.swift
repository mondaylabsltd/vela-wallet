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

    @State private var member: String?

    private var draft: MsDraft? { hub.proposalsView.draft }
    private var state: MsTagged? { hub.proposalsView.action("propose") }

    var body: some View {
        MsSheet(title: words.t("proposal.title"), closeLabel: words.loc.t("common.done"), tall: true, onClose: withdraw) {
            if let draft {
                let item = draft.item
                Text(verbatim: words.actionTitleNow(chainId: item.chainId, action: item.action, nonce: item.nonce))
                    .typeRole(Typography.title.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityAddTraits(.isHeader)
                    .accessibilityIdentifier("multisig.draft.title")
                Text(verbatim: "\(words.network(item.chainId)) · \(words.t("action.numberValue", ["nonce": item.nonce]))")
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                if let refusal = item.refusal, !refusal.isEmpty {
                    MsNote(text: words.t("proposal.refused", ["reason": refusal]), tone: .danger)
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
    }

    @ViewBuilder private func actions(_ draft: MsDraft) -> some View {
        let allowed = draft.review["sign"]?["allowed"]?.bool ?? true
        let anyway = draft.review["sign"]?["anyway"]?.bool ?? false
        let working = state?.kind == "working"
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if draft.membersHere.count > 1 {
                Picker(words.t("proposal.as"), selection: Binding(
                    get: { member ?? draft.defaultMember ?? draft.membersHere.first },
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
            VelaButton(
                title: words.t(draft.membersHere.isEmpty ? "send.proposeOnly" : (allowed ? "send.propose" : "review.proposeAnyway")),
                kind: .primary,
                enabled: allowed || anyway,
                loading: working,
                haptic: .confirm
            ) {
                hub.confirmDraft(as: member ?? draft.defaultMember)
            }
            .accessibilityIdentifier("multisig.draft.propose")
            VelaButton(title: words.t("proposal.dontPropose"), kind: .secondary, enabled: !working) {
                withdraw()
            }
            .accessibilityIdentifier("multisig.draft.withdraw")
        }
        .padding(.top, Tokens.Space.s8)
    }

    private func withdraw() {
        hub.withdrawDraft()
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
