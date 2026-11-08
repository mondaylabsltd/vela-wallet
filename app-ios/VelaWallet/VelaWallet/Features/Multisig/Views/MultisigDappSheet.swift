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
            MsSheet(title: Self.refusalTitle(held?["refusal"], words: words), closeLabel: words.loc.t("common.done"), onClose: leave) {
                MsNote(text: words.t("proposal.siteAsks", ["host": host(held)]))
                MsNote(text: Self.refusalText(held?["refusal"], words: words), tone: .warning, identifier: "multisig.dapp.refused")
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
                // r13-3: the reads before its draft failed (bounded by the
                // core) — why, Decline and Try again; never a spinner for ever.
                if let failure = draftFailure(held) {
                    MsNote(text: failure, tone: .warning, identifier: "multisig.dapp.failed")
                    VelaButton(title: words.t("external.retry"), kind: .secondary) { hub.dappRetry(id) }
                        .accessibilityIdentifier("multisig.dapp.retry")
                    VelaButton(title: words.t("external.decline"), kind: .secondary) { leave() }
                } else {
                    HStack(spacing: Tokens.Space.s8) {
                        ProgressView()
                        Text(prose: words.t("proposal.opening"))
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                    }
                    .accessibilityIdentifier("multisig.dapp.opening")
                }
            }
        }
    }

    /// What a refused request says. §31: a request that would have made this
    /// multisig act for a Safe it owns (`data.reason == "parent_safe"`) is
    /// said in the catalog's sentence the core names (`data.key` —
    /// `parentSafeBlocked` / `parentSafeUnchecked`), with the call and the
    /// Safe. §35 (SEC-PARENT-2): a request that would have made this multisig
    /// call ITSELF to change who controls it (`data.reason == "self_call"`)
    /// is said in its sentence (`data.key`,
    /// `componentsUi.signing.multisigSelfCallBlocked`) with the function.
    /// Any other refusal in the core's own words.
    static func refusalText(_ refusal: MsJSON?, words: MsWords) -> String {
        let data = refusal?["data"]
        let reason = data?["reason"]?.string
        if reason == "parent_safe" || reason == "self_call", let key = data?["key"]?.string, !key.isEmpty {
            return words.loc.t(key, vars: [
                "function": data?["function"]?.string ?? "",
                "safe": AddressText.short(data?["safe"]?.string ?? ""),
            ])
        }
        return refusal?["message"]?.string ?? words.t("action.unsupported")
    }

    /// The refused sheet's title: "Vela can't sign this"
    /// (`componentsUi.signing.selfCallBlockedTitle`) over a site's attempt to
    /// change who controls the multisig (§35) — the account's self-call
    /// sheet's own title — else the proposal's.
    static func refusalTitle(_ refusal: MsJSON?, words: MsWords) -> String {
        refusal?["data"]?["reason"]?.string == "self_call"
            ? words.loc.t(I18nKeys.CoreRound.selfCallBlockedTitle)
            : words.t("proposal.title")
    }

    /// The draft's failure (the machine's `actions["propose"]`), worded.
    private func draftFailure(_ held: MsJSON?) -> String? {
        guard let state = hub.proposalsView.actions["propose"] else { return nil }
        return words.actionFailure(state, chainId: held?["chain_id"]?.int ?? 0)
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
