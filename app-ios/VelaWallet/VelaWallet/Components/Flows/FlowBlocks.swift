//
//  FlowBlocks.swift
//  VelaWallet
//
//  The blocks of the wallet flows (spec 021 components 8, 16–22, 24–26).
//

import SwiftUI

/// The account card above every QR (component 17): whose address this is,
/// spelled out in full, with one copy button.
///
/// The address wraps to exactly two lines and never truncates. R2 is the
/// screen a person reads an address OFF, and an ellipsis in the middle of it
/// would defeat the only job the screen has.
struct AddressCardView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let account: AddressCardModel
    var copied = false
    var onCopy: () -> Void = {}

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            IdenticonAvatar(seed: account.identiconSeed, size: WalletGeometry.avatar)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: account.name)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                ForEach(Array(account.lines.enumerated()), id: \.offset) { _, line in
                    Text(verbatim: line)
                        .monoRole(Typography.monoAddress.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                }
            }
            Spacer(minLength: Tokens.Space.s8)
            FlowIconButton(
                glyph: copied ? .check : .copy,
                label: account.copyLabel,
                tint: copied ? theme.successBase : theme.fgMuted,
                action: onCopy
            )
        }
        .padding(.vertical, Tokens.Space.s12)
    }
}

/// The receive QR card (component 18) — R2, R3 and R4.
///
/// Two decisions carried from the SPEC sheet:
///
/// - **A fixed square.** The card does not scale with the text. At 1.35× the
///   copy around it grows and the screen scrolls; the code stays the size it
///   was, because a code that shrinks to make room for its caption stops
///   scanning.
/// - **Something in the middle.** The network mark on R2, the token on R3, the
///   account's own identicon on the share card — R4's centre is an
///   anti-forgery mark: a card whose address was doctored would carry artwork
///   that no longer matches the characters printed under it.
///
/// The modules are the deterministic demo pattern spec 015 established, never
/// real encoded data — a code that looked scannable but was not would be worse
/// than one that plainly is not.
struct QrCardView<Centre: View>: View {
    @Environment(\.theme) private var theme

    let label: String
    /// A REAL code's modules, row-major. `nil` keeps the drawn demo pattern,
    /// which is what the gallery and the screenshot sweep render.
    ///
    /// There is deliberately no fallback the other way: a screen with a real
    /// address that cannot encode it must not quietly show the demo pattern —
    /// see `QrCode`.
    var modules: [[Bool]]?
    @ViewBuilder let centre: () -> Centre

    var body: some View {
        ZStack {
            Canvas { context, size in
                let cells = modules ?? QrPattern.cells
                let count = cells.count
                let module = min(size.width, size.height) / CGFloat(count)
                for r in 0..<count {
                    for c in 0..<cells[r].count where cells[r][c] {
                        let rect = CGRect(
                            x: CGFloat(c) * module,
                            y: CGFloat(r) * module,
                            // Rounded up so neighbouring modules meet: a
                            // hairline of white between them is what a camera
                            // reads as a broken code.
                            width: module.rounded(.up),
                            height: module.rounded(.up)
                        )
                        context.fill(Path(rect), with: .color(WalletGeometry.qrInk))
                    }
                }
            }
            .padding(WalletFlowGeometry.qrCardPadding)
            centre()
                .padding(Tokens.Space.s2)
                // The cut-out reads as part of the card, so it takes the card's
                // white rather than a theme surface that would flip underneath.
                .background(Circle().fill(WalletGeometry.qrCard))
        }
        .frame(width: WalletFlowGeometry.qrCard, height: WalletFlowGeometry.qrCard)
        // White in BOTH appearances: a code is read by a camera, and inverting
        // it in dark mode is the classic way to make one unscannable.
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r16).fill(WalletGeometry.qrCard))
        .accessibilityElement()
        .accessibilityLabel(label)
    }
}

/// The deterministic demo pattern (spec 015 data-model.md, ported here).
///
/// Three standard finder squares plus xorshift32-seeded noise. Identical on
/// every platform and every run, so screenshots diff cleanly. Denser than the
/// spec-015 placeholder because R2 draws the code large, where 21 modules read
/// as a chequerboard rather than a code.
enum QrPattern {
    static let modules = 29
    private static let seed: UInt32 = 0xbeef

    static let cells: [[Bool]] = {
        var s = seed
        func next() -> UInt32 {
            s ^= s << 13
            s ^= s >> 17
            s ^= s << 5
            return s
        }
        let n = modules
        return (0..<n).map { r in
            (0..<n).map { c in
                let inFinder = (r < 7 && c < 7) || (r < 7 && c >= n - 7) || (r >= n - 7 && c < 7)
                if inFinder {
                    let lr = r < 7 ? r : r - (n - 7)
                    let lc = c < 7 ? c : c - (n - 7)
                    return min(lr, lc, 6 - lr, 6 - lc) != 1
                }
                return (next() & 3) == 0 ? false : next() % 2 == 0
            }
        }
    }()
}

/// SD2's amount (component 8): the number, big and centred, with its fiat
/// equivalent and the toggle that swaps which of the two you type.
///
/// The figure is the largest type on the screen because it is the one thing
/// the person came to decide. The fiat line stays subordinate even when the
/// denominations swap — the amount being ENTERED leads, whichever it is.
struct AmountInputView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let amount: AmountFieldModel
    var onDenom: () -> Void = {}
    /// The live field; `nil` renders the drawn figure.
    var text: Binding<String>?

    var body: some View {
        VStack(spacing: Tokens.Space.s4) {
            if let text {
                // `typeRole` is a `Text` extension (the sanctioned styling
                // seam); a `TextField` takes the same role's font directly.
                TextField("0", text: text)
                    .font(Typography.amountHero.scaled(textScale).font)
                    .foregroundStyle(theme.fgBase)
                    .multilineTextAlignment(.center)
                    .keyboardType(.decimalPad)
                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                    .lineLimit(1)
                    .accessibilityIdentifier("send.amount")
            } else {
                Text(verbatim: amount.value)
                    .typeRole(Typography.amountHero.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .minimumScaleFactor(WalletGeometry.heroMinScale)
                    .lineLimit(1)
            }
            Button(action: onDenom) {
                HStack(spacing: Tokens.Space.s2) {
                    Text(verbatim: amount.fiat)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgMuted)
                    LucideIcon(.chevronsUpDown, size: LucideIconSize.smallChevron)
                        .foregroundStyle(theme.fgMuted)
                }
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityLabel(amount.denomLabel)
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, Tokens.Space.s24)
    }
}

/// The big signed amount (component 19): A2's and A3's transaction figure,
/// T2's balance, SD3's confirmation total.
///
/// Money in is green; money out is plain ink, not red. Red in this product
/// means something went wrong, and a transfer you chose to make did not.
struct AmountHeroView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let amount: String
    let fiat: String
    var positive = false
    var centred = false

    var body: some View {
        VStack(alignment: centred ? .center : .leading, spacing: Tokens.Space.s2) {
            Text(verbatim: amount)
                .typeRole(Typography.display.scaled(textScale))
                .foregroundStyle(positive ? theme.successBase : theme.fgBase)
                .minimumScaleFactor(WalletGeometry.heroMinScale)
                .lineLimit(1)
            Text(verbatim: fiat)
                .typeRole(Typography.body.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
        }
        .frame(maxWidth: .infinity, alignment: centred ? .center : .leading)
        .padding(.top, Tokens.Space.s12)
        .padding(.bottom, Tokens.Space.s16)
    }
}

/// The send receipt's centrepiece (component 20) — SD4a's spinner, SD4b's
/// clock, SD4c's tick, and the failure cross.
///
/// One disc size for all four so the mark does not resize as the transaction
/// moves between them: the person is watching this circle, and a circle that
/// jumps when the state changes reads as a new screen rather than as progress
/// on the one they were already looking at.
struct StatusHeroView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var spinning = false

    let stage: ReceiptStage
    let title: String
    let captions: [String]

    var body: some View {
        VStack(spacing: Tokens.Space.s4) {
            disc
                .padding(.bottom, Tokens.Space.s16)
            Text(verbatim: title)
                .typeRole(Typography.title.scaled(textScale))
                .foregroundStyle(theme.fgBase)
                .multilineTextAlignment(.center)
            ForEach(Array(captions.enumerated()), id: \.offset) { index, caption in
                Text(verbatim: caption)
                    // The second caption is the one that says "you can leave" —
                    // true, useful, and not what the person is waiting to read.
                    .typeRole(
                        index == 0
                            ? Typography.body.scaled(textScale)
                            : Typography.rowSub.scaled(textScale)
                    )
                    .foregroundStyle(index == 0 ? theme.fgMuted : theme.fgSubtle)
                    .multilineTextAlignment(.center)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.top, Tokens.Space.s48)
        .padding(.bottom, Tokens.Space.s24)
    }

    private var disc: some View {
        Circle()
            .fill(discFill)
            .frame(width: WalletFlowGeometry.statusHero, height: WalletFlowGeometry.statusHero)
            .overlay { mark }
    }

    @ViewBuilder private var mark: some View {
        switch stage {
        case .submitting:
            Circle()
                .trim(from: 0, to: 0.75)
                .stroke(
                    theme.accentBase,
                    style: StrokeStyle(
                        lineWidth: WalletFlowGeometry.statusSpinnerStroke, lineCap: .round
                    )
                )
                .frame(width: WalletFlowGeometry.statusSpinner, height: WalletFlowGeometry.statusSpinner)
                .rotationEffect(.degrees(spinning ? 360 : 0))
                .onAppear {
                    guard !reduceMotion else { return }
                    // One revolution at the CTA spinner's speed: one wait speed
                    // in the product, not one per surface.
                    withAnimation(.linear(duration: Tokens.Motion.slow * 2).repeatForever(autoreverses: false)) {
                        spinning = true
                    }
                }
        case .submitted:
            LucideIcon(.clock, size: LucideIconSize.flowStatus).foregroundStyle(theme.fgMuted)
        case .confirmed:
            LucideIcon(.check, size: LucideIconSize.flowStatus).foregroundStyle(theme.successBase)
        case .failed:
            LucideIcon(.close, size: LucideIconSize.flowStatus).foregroundStyle(theme.errorBase)
        }
    }

    private var discFill: Color {
        switch stage {
        case .submitting, .submitted: theme.bgSunken
        case .confirmed: theme.successSoft
        case .failed: theme.errorSoft
        }
    }
}

/// T4's guidance card (component 21): the CTA on top, then the question a
/// person with an empty asset list is actually asking — "it arrived, so why
/// can't I see it?" — and its answer.
///
/// The question is set as a heading rather than as body copy because it is the
/// part someone scanning the screen needs to recognise as theirs.
struct HintCardView<CTA: View>: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let title: String
    let body_: String
    @ViewBuilder let cta: () -> CTA

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            cta()
                .padding(.bottom, Tokens.Space.s4)
            Text(verbatim: title)
                .typeRole(Typography.emptyTitle.scaled(textScale))
                .foregroundStyle(theme.fgBase)
            Text(verbatim: body_)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
        }
        .padding(Tokens.Space.s12)
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r16)
                .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
        )
    }
}

/// The inline explanation banner (component 22): SD1b's "these are greyed out
/// because a multi-token send stays on one network", SD2d's "every token goes
/// to the same address".
///
/// It exists because both screens do something surprising — grey out rows a
/// person can see, or accept one address for several tokens — and the cheapest
/// fix for a surprise is to say why, next to it.
struct NoticeBannerView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let text: String
    var mark: TokenMarkModel?

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            if let mark { InlineTokenMark(mark: mark) }
            Text(verbatim: text)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgMuted)
                .fixedSize(horizontal: false, vertical: true)
            Spacer(minLength: Tokens.Space.s0)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .padding(.vertical, Tokens.Space.s8)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
    }
}

/// The send form's token card (component 16): which token is being sent, off
/// which chain, out of how much — and the Max that fills the amount with it.
struct TokenHeaderCardView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let token: SendTokenCardModel
    var onMax: () -> Void = {}

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            TokenIconView(ticker: token.mark.ticker, badgeColor: token.mark.badgeColor)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: token.symbol)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                Text(verbatim: token.detail)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
            }
            Spacer(minLength: Tokens.Space.s8)
            if let max = token.max {
                Button(action: onMax) {
                    Text(verbatim: max)
                        .typeRole(Typography.chip.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .padding(.horizontal, Tokens.Space.s12)
                        .padding(.vertical, Tokens.Space.s4)
                        .background(Capsule().fill(theme.bgSunken))
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
        .padding(Tokens.Space.s12)
        .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
    }
}

/// SD2's and SD2d's recipient field.
///
/// The identicon sits INSIDE the field, next to the characters it is drawn
/// from. Address poisoning works by matching the first and last few characters
/// of an address you have used before; the artwork is the part that does not
/// match, and it only helps if it is where the eye already is.
struct RecipientFieldView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let field: RecipientFieldModel
    var onPick: () -> Void = {}
    var onScan: () -> Void = {}
    /// The live field. `nil` renders exactly as drawn — which is what the
    /// gallery and the screenshot sweep get, so they stay pixel-identical
    /// (the same mode-not-a-type trick `SettingsUrlField` uses since 050).
    var text: Binding<String>?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Text(verbatim: field.label)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            HStack(spacing: Tokens.Space.s8) {
                IdenticonAvatar(seed: field.identiconSeed, size: WalletGeometry.rowIcon)
                VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                    if let text {
                        TextField("", text: text)
                            .font(Typography.monoAddressDetail.scaled(textScale).font)
                            .foregroundStyle(theme.fgBase)
                            .textInputAutocapitalization(.never)
                            .autocorrectionDisabled()
                            .accessibilityLabel(field.label)
                    } else {
                        ForEach(Array(field.lines.enumerated()), id: \.offset) { _, line in
                            Text(verbatim: line)
                                .monoRole(Typography.monoAddressDetail.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                        }
                    }
                }
                Spacer(minLength: Tokens.Space.s4)
                FlowIconButton(glyph: .userRound, label: field.pickLabel, action: onPick)
                if let scanLabel = field.scanLabel {
                    FlowIconButton(glyph: .qrCode, label: scanLabel, action: onScan)
                }
            }
            .padding(Tokens.Space.s12)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
            if let note = field.note {
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
    }
}

/// SD2b's three ways to add a recipient (component 24): by hand, from
/// contacts, or from a spreadsheet.
///
/// Outline pills, never accent: they add a ROW to a form, and the accent in
/// this product is reserved for the button that actually moves the money.
struct GhostPillRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let items: [RecipientActionModel]
    var onSelect: (RecipientAction) -> Void = { _ in }

    var body: some View {
        HStack(spacing: Tokens.Space.s4) {
            ForEach(items) { item in
                Button { onSelect(item.id) } label: {
                    Text(verbatim: item.label)
                        .typeRole(Typography.chip.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .lineLimit(1)
                        .frame(maxWidth: .infinity)
                        .padding(.vertical, Tokens.Space.s8)
                        .overlay(
                            Capsule().stroke(
                                theme.borderStrong, lineWidth: Tokens.BorderWidth.hairline
                            )
                        )
                        .contentShape(Capsule())
                }
                .buttonStyle(.plain)
            }
        }
    }
}

/// The total line above the fee (component 25).
///
/// Deliberately not a fact row: that row is a fact ABOUT the transaction
/// inside a card, and this is a running sum of what the form above it says.
struct SummaryLineView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let summary: SummaryLineModel

    var body: some View {
        HStack(spacing: Tokens.Space.s8) {
            Text(verbatim: summary.label)
                .typeRole(Typography.rowSub.scaled(textScale))
                .foregroundStyle(theme.fgSubtle)
            Spacer(minLength: Tokens.Space.s8)
            Text(verbatim: summary.value)
                .typeRole(Typography.fieldLabel.scaled(textScale))
                .foregroundStyle(theme.fgBase)
        }
        .padding(.vertical, Tokens.Space.s8)
    }
}

/// The network-fee row (component 26), on every send form.
///
/// A row and not a card: the fee is a fact about the transfer, and the only
/// thing to DO with it is change which token pays it — which is what the
/// chevron opens. The SPEC sheet is explicit that the tier picker does not
/// live here: the fee is shown, not chosen.
struct FeeRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let fee: FeeRowModel
    var onOpen: () -> Void = {}

    var body: some View {
        Button(action: onOpen) {
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: fee.label)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                Spacer(minLength: Tokens.Space.s8)
                InlineTokenMark(mark: fee.mark)
                Text(verbatim: fee.value)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .lineLimit(1)
                LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                    .foregroundStyle(theme.fgMuted)
            }
            .padding(Tokens.Space.s12)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityLabel(fee.openLabel)
    }
}
