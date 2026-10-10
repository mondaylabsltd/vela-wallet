//
//  SigningSheet.swift
//  VelaWallet
//
//  The signing sheet (spec 022): who is asking and the ✕, then the universal
//  block renderer, the technical details, the fee and the signer — a body
//  that scrolls — over a confirm that does not, on the page that asked for
//  the signature, so the site you are dealing with never leaves the screen.
//
//  The header is pinned above the body. Its ✕ is the ONE way to refuse, and
//  the body brings a tall verdict into view by itself: with the header in
//  the scroll, a short screen kept the confirm in sight and scrolled the
//  refusal — and the name of who is asking — out of it.
//
//  The confirm and the one line a shut gate says are a footer OUTSIDE the
//  scroll, pinned to the bottom of the sheet (the Send confirm's own frame,
//  `FlowFooter`): always whole, always in one place, whatever the body holds.
//  The simulation's verdict is shown whole in the body — its place grows to
//  it (`SigningVerdictRoom`) — and when it lands the body brings it into
//  view. Before, the confirm was the last thing in one scroll: a verdict
//  taller than its place was cut inside a scroll of its own, and anything
//  that grew above the confirm pushed it down, or off the screen.
//
//  The confirm is one tap (issue #461): the shared primary button the Send
//  screen's Confirm is, labelled with the action alone. The slide it replaces
//  was the one confirm in the app that wanted a drag; the passkey or Face ID
//  prompt that follows is the second step.
//
//  The wallet's own request (the key backup, the core's `first_party`) has no
//  requester: the header is one row, its headline and the ✕.
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
    /// Spec 096 F8: the failed receipt's Try again.
    var onRetry: (() -> Void)?

    @State private var techOverride: Bool?
    /// The confirm's note, the last time it was said. The note comes and goes
    /// with the gate — "正在计算网络费用…" on every re-quote, refresh and new
    /// speed — and each time it went, the content under a sheet scrolled to
    /// its end shrank and pulled everything down a line (41 pt; the Android
    /// device moved ~33 px, the web 29). Once said, its line stays, holding
    /// these words invisibly while the gate is open (the web's rule).
    @State private var heldNote: String?
    /// What the verdict's place last held, so a landing is told from the
    /// sheet's first frame. A reference, not view state: recording it draws
    /// nothing.
    @State private var verdictSeen = VerdictSeenBox()
    /// What of the body is out of sight — scrolled under the pinned header,
    /// still to come under the pinned confirm: each draws the edge the body
    /// runs under, and only then.
    @State private var bodyEdges = BodyEdges()

    private var techOpen: Binding<Bool> {
        Binding(get: { techOverride ?? model.techOpen }, set: { techOverride = $0 })
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            // Who is asking, on which network, and the ✕ — in every form of
            // the sheet (the receipt's close is the same control), above the
            // scroll, with the gap the first block always had under it.
            SigningHeaderView(dapp: model.dapp, network: model.network, own: model.dappOwn,
                              headline: model.headline,
                              iconUrls: model.dappIconUrls, networkLogoUrl: model.networkLogoUrl,
                              onClose: onClose, closeLabel: model.closeLabel)
                .padding(.top, Tokens.Space.s8)
                .padding(.horizontal, Tokens.Layout.screenPaddingX)
                .padding(.bottom, Tokens.Space.s16)
                .overlay(alignment: .bottom) { edge(shown: bodyEdges.above) }
            ScrollViewReader { scroll in
                ScrollView {
                    VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                        // Spec 079: approved — the receipt replaces the form.
                        if let receipt = model.receipt {
                            receiptBody(receipt)
                        } else {
                            form(scroll)
                        }
                    }
                    .padding(.horizontal, Tokens.Layout.screenPaddingX)
                    // Scrolled to its end, the last row stands a gap above
                    // the confirm — the gap it always had.
                    .padding(.bottom, footerShown ? Tokens.Space.s4 : Tokens.Space.s0)
                    .onGeometryChange(for: BodyEdges.self) { content in
                        BodyEdges(content: content)
                    } action: { edges in
                        bodyEdges = edges
                    }
                }
                .accessibilityLabel(model.panelTitle)
            }
            footer
        }
        .background(theme.bgRaised.ignoresSafeArea())
        .onChange(of: saidNote, initial: true) { _, said in
            if let said { heldNote = said }
        }
    }

    /// The confirm's note as the gate says it now; `nil` while it is open.
    private var saidNote: String? {
        guard let confirm = model.confirm, !confirm.enabled else { return nil }
        return model.confirmBlockLine
    }

    /// The send receipt's own body and its one button, so a dApp transaction
    /// and a send look the same while they land.
    private func receiptBody(_ receipt: SendReceiptModel) -> some View {
        VStack(spacing: Tokens.Space.s24) {
            SendReceiptBody(model: receipt, onExplorer: onExplorer)
                .padding(.top, Tokens.Space.s16)
            if let onClose {
                // Spec 096 F8: a failure that sent nothing — Done answers the
                // page, Try again (the primary) goes back to review.
                if let retry = receipt.retry, let onRetry {
                    HStack(spacing: Tokens.Space.s12) {
                        VelaButton(title: receipt.cta, kind: .secondary, action: onClose)
                            .accessibilityIdentifier("signing.receipt.cta")
                        VelaButton(title: retry, kind: .primary, action: onRetry)
                            .accessibilityIdentifier("signing.receipt.retry")
                    }
                } else {
                    VelaButton(title: receipt.cta, kind: receipt.ctaAccent ? .primary : .secondary,
                               action: onClose)
                        .accessibilityIdentifier("signing.receipt.cta")
                }
            }
        }
        .padding(.bottom, Tokens.Space.s16)
    }

    @ViewBuilder
    private func form(_ scroll: ScrollViewProxy) -> some View {
        if let handoff = model.handoff {
            // The fee, its speed and its coin are chosen here, before the
            // hand-off (D-18): the sheet's own fee row stays, with every
            // control it has, and the card says no fee of its own — the fee
            // is said once. Then whose account signs, as on every sheet —
            // the order every shell draws: fee, account, hand-off, Open.
            feeView
                .padding(.top, Tokens.Space.s8)
            SigningSignerRow(label: model.signer.label, name: model.signer.name,
                             seed: model.signer.seed)
            // Spec 102 D4: the page is the authority — the hand-off card in
            // place of a second preview, and only what still stands under it.
            HandoffCardView(model: handoff)
            ForEach(model.handoffBlocks) { block in
                blockView(block)
            }
        } else {
            // The wallet's own request: its intent is already the header's title.
            ForEach(model.formItems) { item in
                switch item {
                case .block(let block):
                    blockView(block)
                case .verdict(let inner):
                    // Final note F2: the simulation's verdict lands in a
                    // place the sheet already kept, so the usual one moves
                    // nothing. A taller one is shown whole — the place grows
                    // to it — and either way the body shows it as it lands.
                    let landed = landedVerdict
                    SigningVerdictRoom {
                        ForEach(inner) { block in blockView(block) }
                    }
                    .id(SigningFormItem.verdictId)
                    .onGeometryChange(for: VerdictSeen.self) { place in
                        VerdictSeen(landed: landed, place: place)
                    } action: { seen in
                        reveal(seen, in: scroll)
                    }
                }
            }

            Divider().overlay(theme.borderBase).padding(.top, Tokens.Space.s4)

            if !model.tech.isEmpty {
                TechDetailsView(tech: model.tech, open: techOpen)
            }
            feeView
            SigningSignerRow(label: model.signer.label, name: model.signer.name,
                             seed: model.signer.seed)
        }
    }

    // MARK: - The confirm, pinned

    /// A form with a confirm: the footer is drawn.
    private var footerShown: Bool { model.receipt == nil && model.confirm != nil }

    /// The confirm and the one line under it, outside the scroll: pinned to
    /// the bottom of the sheet, whole, at ONE place — with no verdict yet,
    /// under every kind of verdict, under a tall one, with the coin list or
    /// the technical details open.
    ///
    /// The line's room is part of that place. In a footer that stands on the
    /// sheet's bottom edge, a line that came and went under the button would
    /// lift the button and drop it again: so the room is always there —
    /// the line's own words while the gate says them, the last words it said
    /// held unseen and unread once it opens (`heldNote`), a blank line before
    /// it has said anything.
    @ViewBuilder
    private var footer: some View {
        // Spec 081: a refused request offers no confirm control at
        // all. It is not disabled — it is absent, because the wallet
        // never offered it. Spec 079: approved, the receipt has its own
        // button.
        if model.receipt == nil, let confirm = model.confirm {
            let note = confirm.enabled ? nil : model.confirmBlockLine
            FlowFooter {
                VStack(spacing: Tokens.Space.s8) {
                    // Issue #461: one tap, full width, shut while the core's
                    // gate is (`confirm.enabled`). The approve leaves the form
                    // for the receipt in the same pass, so the button is never
                    // seen dimmed as a "busy" — busy is the receipt, not a
                    // disabled control. Spec 079: an account that signs on the
                    // Trusted Signer's page says where it goes instead — that
                    // page's slide is the consent.
                    VelaButton(title: model.confirmAsButton ? model.confirmButtonLabel : confirm.action,
                               kind: .primary, enabled: confirm.enabled, action: onConfirm)
                        // The Trusted Signer route's button opens a page; it
                        // signs nothing here, so it carries its own hook
                        // (Android and desktop: `signing-open-signer`).
                        .accessibilityIdentifier(model.confirmAsButton ? "signing.openSigner" : "signing.confirm")
                    // Spec 099 R7: a shut confirm says why — the core's line
                    // for the part of the gate that is shut, and the action
                    // that opens it.
                    if let note {
                        confirmNote(note)
                            .accessibilityIdentifier("signing.confirmBlock")
                    } else {
                        confirmNote(heldNote ?? " ")
                            .hidden()
                            .accessibilityHidden(true)
                    }
                }
            }
            // A body with more to come ends at an edge, not under a
            // floating button (the Send confirm's hairline over its pinned
            // card).
            .overlay(alignment: .top) { edge(shown: bodyEdges.below) }
        }
    }

    /// The hairline a pinned part draws where the body runs under it. Drawn
    /// OVER the part, so it takes no room: the header and the confirm are
    /// where they are with the edge and without.
    private func edge(shown: Bool) -> some View {
        FlowDivider()
            .opacity(shown ? 1 : 0)
            .accessibilityHidden(true)
    }

    /// What of the body its viewport does not show.
    private nonisolated struct BodyEdges: Equatable, Sendable {
        /// Scrolled under the header.
        var above = false
        /// Still to come, under the confirm.
        var below = false

        init() {}

        init(content: GeometryProxy) {
            guard let visible = content.bounds(of: .scrollView) else { return }
            above = visible.minY > 0.5
            below = visible.maxY < content.size.height - 0.5
        }
    }

    // MARK: - The verdict, in view

    /// The landed verdict's blocks, by id; `nil` while the simulation is out
    /// (the place holds "Checking…") and where no place is kept.
    private var landedVerdict: [String]? {
        guard let place = model.verdictPlace, place.count > 0,
              place.at >= 0, place.at + place.count <= model.blocks.count
        else { return nil }
        return model.blocks[place.at ..< place.at + place.count].map(\.id)
    }

    /// What the body needs to know of the verdict's place to show it.
    private nonisolated struct VerdictSeen: Equatable, Sendable {
        let landed: [String]?
        let height: CGFloat
        /// The whole place can stand inside the body's viewport.
        let fits: Bool

        init(landed: [String]?, place: GeometryProxy) {
            self.landed = landed
            height = place.size.height
            fits = place.bounds(of: .scrollView).map { place.size.height <= $0.height } ?? true
        }
    }

    private final class VerdictSeenBox {
        var last: VerdictSeen?
    }

    /// The verdict is in view when it lands, and when it grows: a verdict
    /// under the body's fold, over a live confirm, is a verdict nobody read.
    /// Its whole place when the viewport can hold it — the least scroll that
    /// shows all of it, none if it already is — else from its top. Its first
    /// frame is shown without a glide; a later landing glides.
    ///
    /// Only a landing or a growth: the placeholder is not a verdict, and a
    /// viewport that merely changed (the keyboard) pulls nothing.
    private func reveal(_ seen: VerdictSeen, in scroll: ScrollViewProxy) {
        let last = verdictSeen.last
        verdictSeen.last = seen
        guard seen.landed != nil else { return }
        let first = last == nil
        guard first || seen.landed != last?.landed || seen.height > (last?.height ?? 0) else { return }
        let anchor: UnitPoint? = seen.fits ? nil : .top
        // After this layout pass, which is the one that placed the verdict.
        DispatchQueue.main.async {
            if first {
                scroll.scrollTo(SigningFormItem.verdictId, anchor: anchor)
            } else {
                withAnimation(.easeOut(duration: Self.revealSeconds)) {
                    scroll.scrollTo(SigningFormItem.verdictId, anchor: anchor)
                }
            }
        }
    }

    private static let revealSeconds = 0.25

    /// The fee row, its speed control and its coin list — the same in Vela
    /// and in the hand-off.
    @ViewBuilder
    private var feeView: some View {
        if let fee = model.fee {
            SigningFeeView(fee: fee, onToggle: onFee, onPick: onFeePick,
                           speed: model.feeSpeed, onSpeed: onSpeed,
                           refresh: onRefreshFee == nil ? nil : model.feeRefresh,
                           onRefresh: onRefreshFee, chevron: model.feeChevron,
                           measuring: model.feeRefresh?.refreshing ?? false,
                           reserve: model.feeReserve)
        }
    }

    private func confirmNote(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.rowSub)
            .foregroundStyle(theme.fgMuted)
            .multilineTextAlignment(.center)
            .fixedSize(horizontal: false, vertical: true)
            .frame(maxWidth: .infinity)
    }

    /// The universal renderer: blocks in mock order, out. Nothing here knows
    /// what a swap or a permit IS — which is what lets all 33 scenarios, and
    /// the ones nobody has drawn yet, come out of one code path.
    @ViewBuilder
    private func blockView(_ block: SigningBlock) -> some View {
        switch block {
        case .intent(let text, let tone):
            // The eyebrow over a hero. The wallet's own request has no figure
            // to lead with (issue #314): its intent is the header's title.
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
