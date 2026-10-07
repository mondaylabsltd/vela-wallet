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
        // Square, at the measured 344 — or the screen's width where that is
        // less (issue #321): a fixed 344 ran past the gutters of a 375pt-wide
        // phone, whose sheet then had to scroll to reach its last button.
        .aspectRatio(1, contentMode: .fit)
        .frame(maxWidth: WalletFlowGeometry.qrCard)
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
    /// The hero rung's line, following Dynamic Type exactly as the figure's
    /// `.largeTitle`-relative font does, so the fixed line never clips it.
    @ScaledMetric(relativeTo: .largeTitle) private var heroLine = WalletFlowGeometry.amountHeroLine

    let amount: AmountFieldModel
    var onDenom: () -> Void = {}
    /// The live field; `nil` renders the drawn figure.
    var text: Binding<String>?

    var body: some View {
        VStack(spacing: Tokens.Space.s4) {
            figure
            if amount.denomShown {
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
                .disabled(!amount.denomEnabled)
                .opacity(amount.denomEnabled ? 1 : Tokens.Opacity.disabled)
                .accessibilityLabel(amount.denomLabel)
            } else {
                // No toggle, but the figure's own currency still reads.
                Text(verbatim: amount.fiat)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
            }
            // A control that visibly DECLINES says why. Silence here is how a
            // person taps the same chevron three times.
            if let reason = amount.denomReason {
                Text(verbatim: reason)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
                    .fixedSize(horizontal: false, vertical: true)
            }
        }
        .frame(maxWidth: .infinity)
        .padding(.vertical, Tokens.Space.s24)
    }

    /// What the figure reads right now: the field's own text while it is
    /// typed into (the ladder must follow the keystroke, not the core's echo),
    /// the drawn value otherwise.
    private var shown: String {
        if let text, !amount.locked { return text.wrappedValue }
        return amount.value
    }

    /// The figure, wearing its unit (issue 231): "4.00" alone could be dollars
    /// or coins. A currency symbol leads at the figure's own size; a ticker or
    /// a code follows, smaller and quieter, so the number still reads first.
    ///
    /// **On the hero ladder** (spec 078, the web's `AmountInput`): 46 / 38 / 31
    /// by drawn length, the unit stepping down with the digits, on the hero
    /// rung's line whatever is drawn. The units never give way — the digits
    /// do: a read-only figure past the last rung is cut at its END with "…",
    /// a typed one scrolls inside its own field. The old figure hugged its
    /// digits with `fixedSize`, which also meant it could never shrink, so a
    /// Max of `0.043790209243313861` ran off both sides of the screen.
    private var figure: some View {
        let rung = AmountRung.of(
            figure: shown.isEmpty ? Self.placeholder : shown,
            prefix: amount.unitPrefix, suffix: amount.unitSuffix
        )
        let digits = rung.figure.scaled(textScale)
        let editable = text != nil && !amount.locked
        return HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s0) {
            if let prefix = amount.unitPrefix {
                Text(verbatim: prefix)
                    .typeRole(digits)
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .fixedSize()
            }
            entry(digits)
            if let suffix = amount.unitSuffix {
                Text(verbatim: suffix)
                    .typeRole(rung.unit.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .fixedSize()
                    .padding(
                        .leading,
                        editable ? Tokens.Space.s8 - WalletFlowGeometry.amountCaretSlack : Tokens.Space.s8
                    )
            }
        }
        .frame(height: heroLine * textScale)
        .frame(maxWidth: .infinity)
    }

    /// What an empty field shows.
    private static let placeholder = "0"

    @ViewBuilder private func entry(_ role: TypeRole) -> some View {
        if let text, !amount.locked {
            // As wide as what is in it, and never wider than what is left
            // beside the units: a hidden mirror of the text sets the width
            // (the web's `.sizer`), the field fills it and scrolls past it.
            MirrorWidth {
                Text(verbatim: text.wrappedValue.isEmpty ? Self.placeholder : text.wrappedValue)
                    .typeRole(role)
                    .lineLimit(1)
                    .fixedSize()
                    .padding(.trailing, WalletFlowGeometry.amountCaretSlack)
                    .hidden()
                    .accessibilityHidden(true)
                // `typeRole` is a `Text` extension (the sanctioned styling
                // seam); the field takes the same role's font directly. Each
                // edit is cleaned inside the edit (`AmountTextField` says why
                // a `TextField` cleaned in `onChange` dropped keys).
                AmountTextField(
                    text: text,
                    placeholder: Self.placeholder,
                    font: role.uiFont,
                    color: theme.fgBase,
                    identifier: "send.amount"
                )
            }
        } else {
            Text(verbatim: amount.value)
                .typeRole(role)
                .foregroundStyle(theme.fgBase)
                .lineLimit(1)
                .truncationMode(.tail)
        }
    }
}

/// Which rung of the hero ladder a figure is drawn on — the web's
/// `AmountInput` `size` and the desktop's `amount_hero_rung`, counted the same
/// way: the figure's characters, the prefix's, and HALF the suffix's (it is set
/// smaller). Up to 8 drawn → hero, up to 11 → compact, beyond → tight.
enum AmountRung: Equatable {
    case hero, compact, tight

    static func of(figure: String, prefix: String?, suffix: String?) -> AmountRung {
        let drawn = Double(figure.count + (prefix?.count ?? 0)) + Double(suffix?.count ?? 0) / 2
        if drawn <= WalletFlowGeometry.amountHeroMaxDrawn { return .hero }
        if drawn <= WalletFlowGeometry.amountCompactMaxDrawn { return .compact }
        return .tight
    }

    /// The digits' role on this rung.
    var figure: TypeRole {
        switch self {
        case .hero: Typography.amountEntry
        case .compact: Typography.amountEntryCompact
        case .tight: Typography.amountEntryTight
        }
    }

    /// The unit's role on this rung — a step of the type scale below.
    var unit: TypeRole {
        switch self {
        case .hero: Typography.amountUnit
        case .compact: Typography.amountUnitCompact
        case .tight: Typography.amountUnitTight
        }
    }
}

/// Sizes its SECOND subview (a field) to the width its FIRST (a hidden mirror
/// of the same text) wants, capped at what the parent offers.
///
/// A `TextField` takes every point it is offered, and `fixedSize` pins it to
/// its content with no ceiling — the trap that ran an 18-decimal Max off the
/// screen. This is the ceiling: hugging while it fits, the field's own width
/// once it does not, where the text scrolls to its caret.
struct MirrorWidth: Layout {
    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        guard let mirror = subviews.first else { return .zero }
        let ideal = mirror.sizeThatFits(.unspecified)
        let width = min(ideal.width, proposal.width ?? ideal.width)
        let field = subviews.dropFirst().first?
            .sizeThatFits(ProposedViewSize(width: width, height: proposal.height)) ?? .zero
        return CGSize(width: width, height: max(ideal.height, field.height))
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        for subview in subviews {
            subview.place(
                at: CGPoint(x: bounds.minX, y: bounds.midY), anchor: .leading,
                proposal: ProposedViewSize(width: bounds.width, height: bounds.height)
            )
        }
    }

    /// Both children on one baseline: the field's, which is the text's.
    func explicitAlignment(
        of guide: VerticalAlignment, in bounds: CGRect, proposal: ProposedViewSize,
        subviews: Subviews, cache: inout ()
    ) -> CGFloat? {
        guard let field = subviews.dropFirst().first else { return nil }
        let size = field.sizeThatFits(ProposedViewSize(width: bounds.width, height: bounds.height))
        let top = bounds.midY - size.height / 2
        return field.dimensions(in: ProposedViewSize(width: bounds.width, height: bounds.height))[guide]
            + top
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
    /// An unlimited allowance — a risk, drawn in the danger tone (spec 093).
    var danger = false
    /// A swap's coin back, under the figure (spec 093).
    var received: String? = nil

    var body: some View {
        VStack(alignment: centred ? .center : .leading, spacing: Tokens.Space.s2) {
            Text(verbatim: amount)
                .typeRole(Typography.display.scaled(textScale))
                .foregroundStyle(danger ? theme.errorBase : (positive ? theme.successBase : theme.fgBase))
                .minimumScaleFactor(WalletGeometry.heroMinScale)
                .lineLimit(1)
            if let received {
                Text(verbatim: received)
                    .typeRole(Typography.body.scaled(textScale))
                    .foregroundStyle(theme.successBase)
            }
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
    /// Issue 199: how far the chain's usual time has run, drawn as a ring
    /// OUTSIDE the disc (the disc keeps its one size). `nil` draws none; the
    /// confirmation closes it, green.
    var progress: Double?

    var body: some View {
        VStack(spacing: Tokens.Space.s4) {
            disc
                .overlay { ring }
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
            // Issue #444: a submitted disc breathes, so the wait reads as
            // progress rather than a frozen screen (the web's and Android's).
            .modifier(StatusBreath(active: stage == .submitted && !reduceMotion))
    }

    /// Issue #444: "submitted" always wears the ring — filling as the chain's
    /// usual time passes, and CIRCLING when there is no estimate for the
    /// chain (a dApp's transaction on the in-app browser had none, and the
    /// screen was a still clock). The confirmation closes it, green.
    @ViewBuilder private var ring: some View {
        if stage == .submitted || stage == .confirmed || progress != nil {
            StatusRing(stage: stage, progress: progress)
                // Clears the disc rather than outlining it (the web's --space-md gutter).
                .padding(-Tokens.Space.s12)
                .accessibilityHidden(true)
        }
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

/// The receipt's ring: the fraction of the chain's usual time, or — submitted
/// with no estimate — a quarter arc circling once per
/// `WalletFlowGeometry.statusRoamPeriod`.
private struct StatusRing: View {
    @Environment(\.theme) private var theme
    @Environment(\.accessibilityReduceMotion) private var reduceMotion
    @State private var turned = false

    let stage: ReceiptStage
    let progress: Double?

    private var roaming: Bool { stage == .submitted && progress == nil }

    var body: some View {
        ZStack {
            if stage != .confirmed {
                Circle().stroke(theme.borderBase, lineWidth: WalletFlowGeometry.statusRingStroke)
            }
            Circle()
                .trim(from: 0, to: drawn)
                .stroke(
                    stage == .confirmed ? theme.successBase : theme.accentBase,
                    style: StrokeStyle(lineWidth: WalletFlowGeometry.statusRingStroke, lineCap: .round)
                )
                .rotationEffect(.degrees(-90))
                // One second per step, linear: the screen ticks once a second.
                .animation(reduceMotion ? nil : .linear(duration: 1), value: progress)
                .rotationEffect(.degrees(roaming && turned ? 360 : 0))
        }
        .onAppear(perform: roam)
        .onChange(of: roaming) { _, _ in roam() }
    }

    private var drawn: Double {
        if stage == .confirmed { return 1 }
        return min(1, max(0, progress ?? WalletFlowGeometry.statusRoamArc))
    }

    private func roam() {
        guard roaming, !reduceMotion else {
            turned = false
            return
        }
        withAnimation(.linear(duration: WalletFlowGeometry.statusRoamPeriod).repeatForever(autoreverses: false)) {
            turned = true
        }
    }
}

/// The submitted disc's breath: a slow scale to
/// `WalletFlowGeometry.statusBreatheScale` and back, while `active`.
private struct StatusBreath: ViewModifier {
    let active: Bool
    @State private var inhaled = false

    func body(content: Content) -> some View {
        content
            .scaleEffect(active && inhaled ? WalletFlowGeometry.statusBreatheScale : 1)
            .onAppear(perform: breathe)
            .onChange(of: active) { _, _ in breathe() }
    }

    private func breathe() {
        guard active else {
            inhaled = false
            return
        }
        withAnimation(.easeInOut(duration: WalletFlowGeometry.statusBreathePeriod).repeatForever(autoreverses: true)) {
            inhaled = true
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
    /// Issue #326: the card's own tap — back to the asset picker.
    var onChange: (() -> Void)?

    var body: some View {
        HStack(spacing: Tokens.Space.s12) {
            // The token, and — where the core allows it — the way to another
            // one. Max stays its own button beside it: one tap, one meaning.
            if let change = token.change, let onChange {
                Button(action: onChange) {
                    HStack(spacing: Tokens.Space.s12) {
                        face
                        LucideIcon(.chevronDown, size: LucideIconSize.checkmark)
                            .foregroundStyle(theme.fgMuted)
                    }
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityHint(change)
                .accessibilityIdentifier("send.tokenCard")
            } else {
                face
            }
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

    /// The mark, the symbol and where it is held — the card's face.
    private var face: some View {
        HStack(spacing: Tokens.Space.s12) {
            TokenIconView(mark: token.mark)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(verbatim: token.symbol)
                    .typeRole(Typography.rowTitle.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                // Where it is held and how much: the balance is the figure the
                // amount is typed against, so the line wraps rather than
                // cutting it (087 F28 — "Gnosis · 余额 0.1…" on an iPhone 11
                // with a larger text size, the chevron beside it). One line
                // at the default size; the figure never breaks.
                Text(verbatim: token.detail)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(2)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: Tokens.Space.s8)
        }
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
                            // The return key reads 完成 and puts the keyboard
                            // away (087 F28) — an address is one line.
                            .submitLabel(.done)
                            .accessibilityLabel(field.label)
                            // Named, so a test reaches THIS field rather than
                            // whichever one happens to come first in the tree —
                            // an earlier one typed an address into the amount.
                            .accessibilityIdentifier("send.recipient")
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
                // A token's own contract (spec 096 F12) is a warning, in the
                // warning tone the confirm's tag uses.
                Text(verbatim: note)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(field.noteWarn ? theme.warningBase : theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
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
        VStack(alignment: .trailing, spacing: Tokens.Space.s2) {
            HStack(spacing: Tokens.Space.s8) {
                Text(verbatim: summary.label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                Spacer(minLength: Tokens.Space.s8)
                Text(verbatim: summary.value)
                    .typeRole(Typography.fieldLabel.scaled(textScale))
                    // Over the balance, the figure itself says so; the
                    // sentence under the total says why.
                    .foregroundStyle(summary.over ? theme.errorBase : theme.fgBase)
            }
            if let remaining = summary.remaining {
                Text(verbatim: remaining)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
            }
        }
        .padding(.vertical, Tokens.Space.s8)
    }
}

/// The network-fee row (component 26), on every send form.
///
/// A row and not a card: the fee is a fact about the transfer. The row opens
/// the fee-coin sheet; beside it — outside its own tap, so measuring again
/// never opens the sheet — the refresh control (spec 068; iOS's since 069),
/// and under it the stale line, whose room is kept so Continue never moves
/// under a thumb. The speed control (`FeeSpeedControlView`) lives under that.
struct FeeRowView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let fee: FeeRowModel
    var onOpen: () -> Void = {}
    var onRefresh: (() -> Void)? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            // ONE card (round 3): the figure and, inside it at the trailing
            // edge, a fixed round refresh button — as the web and the desktop
            // draw it. Round 2's full-height block beside the card grew into a
            // slab at the largest text size.
            HStack(alignment: .center, spacing: Tokens.Space.s8) {
                Button(action: onOpen) {
                    // Label and value side by side while both fit whole; the
                    // label on its own line and the value under it otherwise.
                    // It used to break the label inside a word
                    // ("Netzwerkg / ebühr") and cut the value to "0.00421…".
                    TitleAndValue {
                        Text(verbatim: fee.label)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgMuted)
                            .fixedSize(horizontal: false, vertical: true)
                        HStack(alignment: .center, spacing: Tokens.Space.s8) {
                            InlineTokenMark(mark: fee.mark)
                            Text(verbatim: fee.value)
                                .typeRole(Typography.body.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                                .fixedSize(horizontal: false, vertical: true)
                            LucideIcon(.chevronRight, size: LucideIconSize.smallChevron)
                                .foregroundStyle(theme.fgMuted)
                        }
                    }
                    .padding(.vertical, Tokens.Space.s12)
                    .contentShape(Rectangle())
                }
                .buttonStyle(.plain)
                .accessibilityLabel(fee.openLabel)
                if let refreshLabel = fee.refreshLabel {
                    FeeRefreshButton(label: refreshLabel, refreshing: fee.refreshing, onRefresh: onRefresh)
                }
            }
            .padding(.leading, Tokens.Space.s12)
            .padding(.trailing, fee.refreshLabel == nil ? Tokens.Space.s12 : Tokens.Space.s4)
            .background(RoundedRectangle(cornerRadius: Tokens.Radius.r12).fill(theme.bgRaised))
            if fee.refreshLabel != nil {
                // Calm and muted: an old figure is not a fault. Always the
                // line's full height, so nothing jumps when it appears.
                Text(verbatim: fee.staleNote ?? " ")
                    .typeRole(Typography.flowCaption.scaled(textScale))
                    .foregroundStyle(theme.fgSubtle)
                    .padding(.horizontal, Tokens.Space.s12)
            }
        }
    }
}

/// The fee row's round refresh control — the send form's (spec 068), and
/// since spec 079 the signing sheet's too, so "can I ask for a new fee?" has
/// one answer everywhere. Dimmed while a measurement is out — whoever started
/// it — so a second tap is never ambiguous.
struct FeeRefreshButton: View {
    @Environment(\.theme) private var theme

    let label: String
    let refreshing: Bool
    var onRefresh: (() -> Void)?
    /// The disc's fill — `bgSunken` on the send form's raised card; a sunken
    /// card (the signing sheet's) passes the raised colour so the disc shows.
    var disc: Color?

    var body: some View {
        Button { onRefresh?() } label: {
            LucideIcon(.refreshCw, size: LucideIconSize.rowGlyph)
                .foregroundStyle(refreshing ? theme.fgSubtle : theme.fgMuted)
                .frame(width: WalletFlowGeometry.feeRefreshButton,
                       height: WalletFlowGeometry.feeRefreshButton)
                .background(Circle().fill(disc ?? theme.bgSunken))
                // The target is a little larger than the circle.
                .frame(width: WalletFlowGeometry.feeRefreshTarget,
                       height: WalletFlowGeometry.feeRefreshTarget)
                .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .disabled(onRefresh == nil)
        .accessibilityLabel(label)
    }
}

/// The speed control under the fee row (spec 068), folded until opened. Every
/// decision in it is the `fee_speed` core's (spec 069).
///
/// Folded: the word and the tier in force — THEIR default, never a hardcoded
/// one. Opened: the one-shot promise first (somebody about to change one
/// payment needs to know every later one is untouched), then three options —
/// name, its own fee, its gas bid, what it buys, a tick — or, on a network
/// with one speed, that one statement instead.
struct FeeSpeedControlView: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let speed: FeeSpeedModel
    var onToggle: () -> Void = {}
    var onPick: (String) -> Void = { _ in }

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s4) {
            Button(action: onToggle) {
                // Side by side while both fit whole; stacked otherwise — the
                // header read "Geschw…" at an accessibility size (round 3).
                TitleAndValue {
                    Text(verbatim: speed.label)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                        .fixedSize(horizontal: false, vertical: true)
                    HStack(spacing: Tokens.Space.s8) {
                        Text(verbatim: speed.value)
                            .typeRole(Typography.body.scaled(textScale))
                            .foregroundStyle(theme.fgBase)
                            .fixedSize(horizontal: false, vertical: true)
                        LucideIcon(.chevronDown, size: LucideIconSize.smallChevron)
                            .foregroundStyle(theme.fgMuted)
                            .rotationEffect(.degrees(speed.open ? 180 : 0))
                    }
                }
                .padding(.horizontal, Tokens.Space.s12)
                .padding(.vertical, Tokens.Space.s8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .accessibilityAddTraits(speed.open ? [.isSelected] : [])
            // Folded AND open: the screen must never say "Fast" over a
            // Settings row that says "Slow" without saying why.
            if let free = speed.freeNote { note(free) }
            if speed.open {
                if let single = speed.singleNote {
                    note(single)
                } else {
                    note(speed.onceNote)
                    ForEach(speed.options) { option in
                        Button { onPick(option.id) } label: { row(option) }
                            .buttonStyle(.plain)
                            .accessibilityAddTraits(option.selected ? [.isSelected] : [])
                    }
                }
            }
        }
    }

    private func note(_ text: String) -> some View {
        Text(verbatim: text)
            .typeRole(Typography.flowCaption.scaled(textScale))
            .foregroundStyle(theme.fgSubtle)
            .fixedSize(horizontal: false, vertical: true)
            .padding(.horizontal, Tokens.Space.s12)
    }

    /// One speed, on TWO lines (round 2, all four shells): name … fee, then
    /// what it buys … its gas bid. The bid used to sit alone on a middle line
    /// with the description under it, which left a hole under the name.
    ///
    /// The description wraps under itself when the pair does not fit; the bid
    /// never truncates. The bid is the UI font with fixed-width DIGITS — a
    /// monospace face for the whole string (Menlo) read as a different voice.
    /// A chosen row says so in weight and a ✓ in ink, not accent: accent is
    /// for moving money and submitting only.
    private func row(_ option: FeeSpeedOptionModel) -> some View {
        let name = (option.selected ? Typography.bodyStrong : Typography.body).scaled(textScale)
        return HStack(alignment: .top, spacing: Tokens.Space.s8) {
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                // The name and its fee side by side while both fit whole; the
                // fee under the name otherwise — "Stand…" at an accessibility
                // size said nothing (round 3).
                TitleAndValue {
                    Text(verbatim: option.label)
                        .typeRole(name)
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                    Text(verbatim: option.value)
                        .typeRole(Typography.body.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                        .fixedSize(horizontal: false, vertical: true)
                }
                // The reason wraps in its own column; the bid never gives way.
                // Below six of the reason's own characters that column would
                // stack the sentence into a tower, so the bid drops under it
                // instead, whole, on the fee's right edge (the web's rule).
                ReasonAndFigure(
                    minReasonWidth: Typography.flowCaption.scaled(textScale).size
                        * ReasonAndFigure.minReasonChars
                ) {
                    Text(verbatim: option.detail)
                        .typeRole(Typography.flowCaption.scaled(textScale))
                        .foregroundStyle(theme.fgSubtle)
                    // Named, because an unnamed "3,244 wei" under a fee reads
                    // as a second charge. Nothing while this tier measures.
                    if speed.gasPriceLine, let bid = option.gasPrice {
                        Text(verbatim: "\(speed.gasPriceLabel)  \(bid)")
                            .monospacedDigit()
                            .typeRole(Typography.flowCaption.scaled(textScale))
                            .foregroundStyle(theme.fgSubtle)
                            .lineLimit(1)
                            // Only where even a line of its own is too narrow
                            // (an accessibility text size): smaller, never cut,
                            // never off the screen.
                            .minimumScaleFactor(WalletGeometry.heroMinScale)
                    }
                }
            }
            // Its own column, centred on line 1 whatever the text size.
            LucideIcon(.check, size: LucideIconSize.checkmark)
                .foregroundStyle(theme.fgBase)
                .frame(height: name.uiFont.lineHeight)
                .opacity(option.selected ? 1 : 0)
        }
        .padding(.horizontal, Tokens.Space.s12)
        .padding(.vertical, Tokens.Space.s8)
        .background(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .fill(option.selected ? theme.bgRaised : Color.clear)
        )
        .contentShape(Rectangle())
    }
}

/// A speed row's line 2: a reason that wraps, and a figure that does not.
///
/// Side by side while the reason keeps a column of at least `minReasonWidth`
/// (six of its own characters — the web's `flex: 1 1 6em`); below that the
/// figure drops to a line of its own under the reason, trailing-aligned, so a
/// narrow phone at a large text size never gets a one-word tower or a figure
/// pushed off the screen. The reason is never cut in either shape. Both pieces
/// are the same caption type, so top-aligned they share a baseline.
struct ReasonAndFigure: Layout {
    /// How many of the reason's own characters its column may not go below.
    static let minReasonChars: CGFloat = 6

    let minReasonWidth: CGFloat
    var columnGap: CGFloat = Tokens.Space.s8
    var rowGap: CGFloat = Tokens.Space.s2

    private struct Plan {
        let size: CGSize
        let reason: CGRect
        let figure: CGRect?
    }

    private func plan(width proposed: CGFloat?, subviews: Subviews) -> Plan {
        guard let reason = subviews.first else { return Plan(size: .zero, reason: .zero, figure: nil) }
        let figure = subviews.count > 1 ? subviews[1] : nil
        guard let figure else {
            let size = reason.sizeThatFits(ProposedViewSize(width: proposed, height: nil))
            return Plan(size: size, reason: CGRect(origin: .zero, size: size), figure: nil)
        }
        let ideal = figure.sizeThatFits(.unspecified)
        guard let width = proposed else {
            // Unconstrained: one line, side by side.
            let reasonSize = reason.sizeThatFits(.unspecified)
            let total = CGSize(width: reasonSize.width + columnGap + ideal.width,
                               height: max(reasonSize.height, ideal.height))
            return Plan(size: total, reason: CGRect(origin: .zero, size: reasonSize),
                        figure: CGRect(x: reasonSize.width + columnGap, y: 0,
                                       width: ideal.width, height: ideal.height))
        }
        let room = width - ideal.width - columnGap
        if room >= minReasonWidth {
            let reasonSize = reason.sizeThatFits(ProposedViewSize(width: room, height: nil))
            return Plan(
                size: CGSize(width: width, height: max(reasonSize.height, ideal.height)),
                reason: CGRect(origin: .zero, size: CGSize(width: room, height: reasonSize.height)),
                figure: CGRect(x: width - ideal.width, y: 0, width: ideal.width, height: ideal.height)
            )
        }
        let reasonSize = reason.sizeThatFits(ProposedViewSize(width: width, height: nil))
        let figureWidth = min(ideal.width, width)
        let figureSize = figure.sizeThatFits(ProposedViewSize(width: figureWidth, height: nil))
        let top = reasonSize.height + rowGap
        return Plan(
            size: CGSize(width: width, height: top + figureSize.height),
            reason: CGRect(origin: .zero, size: CGSize(width: width, height: reasonSize.height)),
            figure: CGRect(x: width - figureWidth, y: top, width: figureWidth, height: figureSize.height)
        )
    }

    func sizeThatFits(proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) -> CGSize {
        plan(width: proposal.width, subviews: subviews).size
    }

    func placeSubviews(in bounds: CGRect, proposal: ProposedViewSize, subviews: Subviews, cache: inout ()) {
        let laid = plan(width: bounds.width, subviews: subviews)
        subviews.first?.place(
            at: CGPoint(x: bounds.minX + laid.reason.minX, y: bounds.minY + laid.reason.minY),
            proposal: ProposedViewSize(laid.reason.size)
        )
        if let frame = laid.figure, subviews.count > 1 {
            subviews[1].place(
                at: CGPoint(x: bounds.minX + frame.minX, y: bounds.minY + frame.minY),
                proposal: ProposedViewSize(frame.size)
            )
        }
    }
}

// MARK: - The include-network switch (spec 090)

/// An on/off switch with its label — the receive code's "include network",
/// the product's one switch, drawn as web and desktop draw it.
///
/// The whole row is the target, one haptic per flip (`select`: a choice that
/// takes effect). A press stretches the thumb toward where it is going — the
/// shape-deform every pressed control here makes. Monochrome on purpose: the
/// accent belongs to the one action that moves money, so "on" is the ink
/// track. `onChange` is `nil` in the gallery, where the switch is a picture.
struct VelaSwitchRow: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let label: String
    let isOn: Bool
    var onChange: ((Bool) -> Void)?

    /// One flip: the haptic, then the other position asked for — the core
    /// answers with what the switch now is.
    func flip() {
        guard let onChange else { return }
        VelaHaptic.select.play()
        onChange(!isOn)
    }

    var body: some View {
        Button(action: flip) {
            EmptyView()
        }
        .buttonStyle(Style(label: label, isOn: isOn, theme: theme, textScale: textScale))
        .accessibilityLabel(label)
        .accessibilityValue(isOn ? "1" : "0")
        .accessibilityAddTraits(.isToggle)
        .accessibilityIdentifier("receive.includeNetwork")
    }

    private struct Style: ButtonStyle {
        let label: String
        let isOn: Bool
        let theme: Theme
        let textScale: CGFloat

        func makeBody(configuration: Configuration) -> some View {
            HStack(spacing: Tokens.Space.s12) {
                Text(verbatim: label)
                    .typeRole(Typography.rowSub.scaled(textScale))
                    .foregroundStyle(theme.fgBase)
                    .frame(maxWidth: .infinity, alignment: .leading)
                Capsule()
                    .fill(isOn ? theme.fgBase : theme.borderStrong)
                    .frame(
                        width: WalletFlowGeometry.switchTrack.width,
                        height: WalletFlowGeometry.switchTrack.height
                    )
                    .overlay(alignment: isOn ? .trailing : .leading) {
                        Capsule()
                            .fill(theme.bgBase)
                            .frame(
                                width: configuration.isPressed
                                    ? WalletFlowGeometry.switchThumbPressed
                                    : WalletFlowGeometry.switchThumb,
                                height: WalletFlowGeometry.switchThumb
                            )
                            .padding(Tokens.Space.s2)
                    }
            }
            .padding(.vertical, Tokens.Space.s4)
            .contentShape(Rectangle())
            .animation(.easeOut(duration: Tokens.Motion.fast), value: isOn)
            .animation(.easeOut(duration: Tokens.Motion.fast), value: configuration.isPressed)
        }
    }
}
