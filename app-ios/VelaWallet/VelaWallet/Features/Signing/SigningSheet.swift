//
//  SigningSheet.swift
//  VelaWallet
//
//  The signing sheet (spec 022): the universal block renderer plus a fixed
//  footer — technical details → fee → signer → slide — over the page that
//  asked for the signature, so the site you are dealing with never leaves
//  the screen.
//
//  The header's ✕ is the one way to refuse (spec 079, owner ruling: "除非用户
//  明确关掉，不应该很容易误操作，比如下滑就关掉了" — a stray swipe used to throw
//  the dApp's request away). No swipe closes it: the presenter sets
//  `.interactiveDismissDisabled()`. There is still no big "Reject" button,
//  because a wallet with one teaches people to reach for it without reading.
//
//  After the approval the form gives way to the send receipt's own body
//  (`SigningModel.receipt`); closing then refuses nothing — the core routes
//  the same close to a plain dismiss once the commitment point has passed.
//

import SwiftUI

struct SigningSheet: View {
    @Environment(\.theme) private var theme

    let model: SigningModel
    var onConfirm: () -> Void = {}
    /// An allowance chip was tapped: `requested`, `balance`, `custom`,
    /// `revoke`. The core decides what each means.
    var onAllowanceChip: (String) -> Void = { _ in }
    var onAllowanceAmount: (String) -> Void = { _ in }
    /// One batch leg's chip / field — the leg index travels with them.
    var onAllowanceLegChip: (Int, String) -> Void = { _, _ in }
    var onAllowanceLegAmount: (Int, String) -> Void = { _, _ in }
    /// Issue #262: the fee row's tap (retry, or open / close the coin list)
    /// and a coin picked from that list.
    var onFee: () -> Void = {}
    var onFeePick: (String) -> Void = { _ in }
    /// The speed control (spec 069): `nil` folds or unfolds it; an id picks.
    var onSpeed: (String?) -> Void = { _ in }
    /// Spec 079: the header's ✕ and the receipt's button — the sheet's one
    /// explicit close. `nil` in the gallery, which has nothing to close.
    var onClose: (() -> Void)?
    /// Spec 079: the landed receipt's "view on explorer".
    var onExplorer: () -> Void = {}
    /// Spec 079: the fee row's refresh — measure again.
    var onRefreshFee: (() -> Void)?

    @State private var techOverride: Bool?

    private var techOpen: Binding<Bool> {
        Binding(get: { techOverride ?? model.techOpen }, set: { techOverride = $0 })
    }

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                SigningHeaderView(dapp: model.dapp, network: model.network, own: model.dappOwn,
                                  iconUrls: model.dappIconUrls, networkLogoUrl: model.networkLogoUrl,
                                  onClose: onClose, closeLabel: model.closeLabel)
                    .padding(.top, Tokens.Space.s8)

                // Spec 079: approved — the receipt replaces the form.
                if let receipt = model.receipt {
                    receiptBody(receipt)
                } else {
                    form
                }
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
        }
        .background(theme.bgRaised.ignoresSafeArea())
        .accessibilityLabel(model.panelTitle)
    }

    /// The send receipt's own body and its one button, so a dApp transaction
    /// and a send look the same while they land.
    private func receiptBody(_ receipt: SendReceiptModel) -> some View {
        VStack(spacing: Tokens.Space.s24) {
            SendReceiptBody(model: receipt, onExplorer: onExplorer)
                .padding(.top, Tokens.Space.s16)
            if let onClose {
                VelaButton(title: receipt.cta, kind: receipt.ctaAccent ? .primary : .secondary,
                           action: onClose)
                    .accessibilityIdentifier("signing.receipt.cta")
            }
        }
        .padding(.bottom, Tokens.Space.s16)
    }

    @ViewBuilder
    private var form: some View {
        ForEach(model.blocks) { block in
            blockView(block)
        }

        Divider().overlay(theme.borderBase).padding(.top, Tokens.Space.s4)

        if !model.tech.isEmpty {
            TechDetailsView(tech: model.tech, open: techOpen)
        }
        if let fee = model.fee {
            SigningFeeView(fee: fee, onToggle: onFee, onPick: onFeePick,
                           speed: model.feeSpeed, onSpeed: onSpeed,
                           refresh: onRefreshFee == nil ? nil : model.feeRefresh,
                           onRefresh: onRefreshFee, chevron: model.feeChevron)
        }
        SigningSignerRow(label: model.signer.label, name: model.signer.name,
                         seed: model.signer.seed)
        // Spec 081: a refused request offers no confirm control at
        // all. It is not disabled — it is absent, because the wallet
        // never offered it.
        if let confirm = model.confirm {
            if model.confirmAsButton {
                // Spec 079: one slide per signature — the page's own. The
                // button sends the same approve the slide would.
                VelaButton(title: model.confirmButtonLabel, kind: .primary,
                           enabled: confirm.enabled, action: onConfirm)
                    .padding(.bottom, Tokens.Space.s16)
                    .accessibilityIdentifier("signing.openSigner")
            } else {
                SlideToConfirmView(
                    hint: confirm.hint, action: confirm.action,
                    enabled: confirm.enabled, onConfirm: onConfirm
                )
                .padding(.bottom, Tokens.Space.s16)
            }
        }
    }

    /// The universal renderer: blocks in mock order, out. Nothing here knows
    /// what a swap or a permit IS — which is what lets all 33 scenarios, and
    /// the ones nobody has drawn yet, come out of one code path.
    @ViewBuilder
    private func blockView(_ block: SigningBlock) -> some View {
        switch block {
        case .intent(let text, let tone):
            SigningIntentLabel(text: text, tone: tone)
        case .amount(let line, let card, let note):
            SigningAmountView(line: line, card: card, note: note)
        case .swap(let pay, let receive):
            SigningSwapPair(pay: pay, receive: receive)
        case .nft(let id, let collection):
            SigningNftHero(id: id, collection: collection)
        case .sentence(let text, let tone):
            SigningSentence(text: text, tone: tone)
        case .allowance(let label, let value, let valueTone, let chips, let note, let total, let custom, let leg):
            // A batch leg's card talks to its OWN leg: the core ignores the
            // single-approval events on a batch, which is how these chips
            // were drawn and dead.
            AllowanceEditorView(label: label, value: value, valueTone: valueTone,
                                chips: chips, note: note, resultingTotal: total,
                                custom: custom,
                                onChip: leg.map { leg in { chip in onAllowanceLegChip(leg, chip) } } ?? onAllowanceChip,
                                onCustomAmount: leg.map { leg in { text in onAllowanceLegAmount(leg, text) } } ?? onAllowanceAmount)
        case .party(let label, let name, let address, let badge):
            SigningPartyRow(label: label, name: name, address: address, badge: badge)
        case .rows(let rows):
            SigningRowsView(rows: rows)
        case .warning(let tone, let text):
            SigningWarning(tone: tone, text: text)
        case .positive(let text):
            SigningPositive(text: text)
        case .code(let lines, let note):
            SigningCodeBlock(lines: lines, note: note)
        case .card(let title, let rows, let tone):
            SigningDetailCard(title: title, rows: rows, tone: tone)
        case .balances(let title, let rows, let note, let noteTone):
            SigningBalances(title: title, rows: rows, note: note, noteTone: noteTone)
        }
    }
}
