//
//  MultisigDappSheet.swift
//  VelaWallet
//
//  A site's request to a multisig (M5) — the web's `ProposalSheet` with
//  `dapp`: drawn from the proposals machine's held request (`view.dapp`).
//  Refused (a method a multisig cannot do) it says why; reviewing, it is the
//  review-first sheet (propose, or decline); proposed, it is the
//  transaction's own sheet, with what the site is told and when — it gets
//  the transaction once executed, else "proposed" at its limit or when this
//  closes. Every way out is the request's to answer (`dapp_leave`).
//

import SwiftUI

struct MultisigDappSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let hub: MultisigHub
    let id: String
    let words: MsWords

    private var held: MsJSON? { hub.proposalsView.dapp[id] }

    var body: some View {
        if let held, let hash = held["safe_tx_hash"]?.string, !hash.isEmpty {
            MultisigProposalSheet(
                hub: hub, chainId: held["chain_id"]?.int ?? 0, safeTxHash: hash, words: words,
                onClose: leave, siteNote: siteNote(held)
            )
        } else if held?["stage"]?.string == "refused" {
            MsSheet(title: words.t("proposal.title"), closeLabel: words.loc.t("common.done"), onClose: leave) {
                MsNote(text: words.t("proposal.siteAsks", ["host": host(held)]))
                MsNote(text: held?["refusal"]?["message"]?.string ?? words.t("action.unsupported"), tone: .warning, identifier: "multisig.dapp.refused")
            }
        } else if hub.proposalsView.draft != nil {
            MultisigDraftReviewSheet(
                hub: hub, chainId: held?["chain_id"]?.int ?? 0, words: words,
                siteNote: words.t("proposal.siteAsks", ["host": host(held)]),
                onWithdraw: leave
            )
        } else {
            MsSheet(title: words.t("proposal.title"), closeLabel: words.loc.t("common.done"), onClose: leave) {
                MsNote(text: words.t("proposal.siteAsks", ["host": host(held)]))
                HStack(spacing: Tokens.Space.s8) {
                    ProgressView()
                    Text(verbatim: words.t("proposal.opening"))
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
                .accessibilityIdentifier("multisig.dapp.opening")
            }
        }
    }

    private func host(_ held: MsJSON?) -> String {
        let origin = held?["origin"]?.string ?? ""
        return URL(string: origin)?.host ?? origin
    }

    /// What the site is told, and when (the machine's `told`).
    private func siteNote(_ held: MsJSON) -> String {
        let host = host(held)
        if held["method"]?.string == "wallet_sendCalls" { return words.t("proposal.siteFollows", ["host": host]) }
        if held["told"]?.bool == true { return words.t("proposal.siteTold", ["host": host]) }
        return words.t("proposal.siteWaits", ["host": host])
    }

    private func leave() {
        hub.dappLeave(id)
        if hub.sheet == .dapp(id: id) { hub.sheet = nil }
    }
}
