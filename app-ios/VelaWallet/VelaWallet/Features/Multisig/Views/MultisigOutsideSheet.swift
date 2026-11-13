//
//  MultisigOutsideSheet.swift
//  VelaWallet
//
//  "Approve with another wallet" (M4) — the web's `ExternalSignSheet`: a
//  member who signs with an outside wallet (MetaMask, Rabby, …) opens Vela's
//  connect page in the browser where that wallet is — by this link, or by
//  scanning the code — and the page and this device meet in a PeerConnect
//  room. The session machine asks the wallet, follows it (connecting, the
//  wrong account, switching network, confirming) and verifies the answer
//  before the proposals machine counts it. Every word is the machine's
//  state; the relay's own trouble is said with Retry.
//

import SwiftUI
import UIKit

struct MultisigOutsideSheet: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    let outside: MultisigOutsideSessionModel
    let words: MsWords
    var onClose: () -> Void

    @State private var copied = false

    var body: some View {
        MsSheet(title: words.t("external.sheetTitle"), closeLabel: words.loc.t("common.done"), tall: true, onClose: close) {
            let resumed = outside.view["resumed"]?.bool == true
            MsNote(text: resumed
                ? words.t("external.resumeBody", ["wallet": outside.view["wallet"]?.string ?? words.t("external.ownerWallet")])
                : words.t("external.scanBody"))
            if let link = outside.link {
                if !resumed, let modules = QrCode.modules(link) {
                    MsQrView(modules: modules)
                        .frame(maxWidth: .infinity)
                        .aspectRatio(1, contentMode: .fit)
                        .accessibilityLabel(words.t("external.sheetTitle"))
                        .accessibilityIdentifier("multisig.outside.qr")
                }
                Text(verbatim: link)
                    .typeRole(Typography.monoSmall.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .textSelection(.enabled)
                    .lineLimit(4)
                    .accessibilityIdentifier("multisig.outside.link")
                HStack(spacing: Tokens.Space.s12) {
                    ShareLink(item: link) { Label(words.t("proposal.share"), systemImage: "square.and.arrow.up") }
                    Button(copied ? words.t("external.copied") : words.t("external.copyLink")) {
                        UIPasteboard.general.string = link
                        copied = true
                        VelaHaptic.select.play()
                    }
                    .accessibilityIdentifier("multisig.outside.copy")
                }
                .roleFont(Typography.actionLabel.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            }
            phaseLine
            relayLine
            failureLine
            if outside.view["outcome"]?.kind == "approved" {
                MsNote(text: words.t("external.signedBy", ["owner": words.label(outside.view["pairedAccount"]?.string ?? "")]), tone: .positive, identifier: "multisig.outside.approved")
            }
            actions
        }
    }

    // MARK: - Where it stands

    @ViewBuilder private var phaseLine: some View {
        let wallet = outside.view["wallet"]?.string ?? words.t("external.ownerWallet")
        let text: String? = switch outside.phase {
        case "waiting": outside.view["away"]?.bool == true ? words.t("external.pageAway") : words.t("external.waiting")
        case "connecting": words.t("external.connecting")
        case "wrongAccount":
            words.t("external.wrongAccount", ["account": MsWords.short(outside.view["accounts"]?.array.first?.string ?? "")])
        case "switching": words.t("external.switching", ["chain": outside.view["chain"]?.string ?? ""])
        case "confirming": words.t("external.inWallet", ["wallet": wallet, "elapsed": elapsed])
        default: nil
        }
        if let text {
            HStack(spacing: Tokens.Space.s8) {
                if outside.phase != "done" { ProgressView() }
                Text(prose: text)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .accessibilityElement(children: .combine)
            .accessibilityIdentifier("multisig.outside.phase")
        }
    }

    private var elapsed: String {
        guard let since = outside.view["since"]?.int, since > 0 else { return "" }
        let seconds = max(0, Int(Date().timeIntervalSince1970) - since / 1000)
        return "\(seconds) s"
    }

    @ViewBuilder private var relayLine: some View {
        if let relay = outside.relay, relay.kind == "retrying" || relay.kind == "failed" {
            // Round 16 leftover 4: the reason in the catalog's words (or the
            // relay's host) — never a transport error's text.
            let reason = MsWords.relayReason(why: relay["why"]?.string, detail: relay["detail"]?.string, words: words)
            HStack(alignment: .top) {
                MsNote(text: reason.map { reason in
                    relay.kind == "failed"
                        ? words.t("external.relayFailed", ["reason": reason])
                        : words.t("external.relayUnreachable", ["reason": reason])
                } ?? words.t("external.velaOffline"), tone: .warning, identifier: "multisig.outside.relay")
                Button(words.t("external.retry")) { outside.retryRelay() }
                    .roleFont(Typography.actionLabel.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
            }
        }
    }

    @ViewBuilder private var failureLine: some View {
        if let text = MsWords.outsideFailure(
            outside.failure, words: words, chain: outside.view["chain"]?.string ?? "",
            wallet: outside.view["wallet"]?.string ?? words.t("external.ownerWallet")
        ) {
            MsNote(text: text, tone: .warning, identifier: "multisig.outside.failure")
        }
    }

    @ViewBuilder private var actions: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            if outside.failure != nil {
                VelaButton(title: words.t("external.askAgain"), kind: .ink) { outside.askAgain() }
                    .accessibilityIdentifier("multisig.outside.askAgain")
            }
            if outside.phase == "confirming" {
                VelaButton(title: words.t("external.stopWaiting"), kind: .secondary) { outside.stopWaiting() }
            }
            if outside.view["pairedAccount"]?.string != nil, outside.phase != "done" {
                Button(words.t("external.unpair")) { outside.anotherWallet(unpair: true) }
                    .roleFont(Typography.actionLabel.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .accessibilityIdentifier("multisig.outside.unpair")
            }
        }
    }

    private func close() {
        outside.close()
        onClose()
    }
}
