//
//  Typography.swift
//  VelaWallet
//
//  Type roles per docs/design-system.md (complete recipes, never ad-hoc font
//  calls). Every role is the system face — SF Pro, SF Mono for data — and
//  nothing is bundled: CJK falls through to the system's own faces (PingFang,
//  Hiragino, Apple SD Gothic). docs/FONT-POLICY.md is the rule.
//

import CoreText
import SwiftUI

/// The cuts of the system face the type roles draw in.
/// Not `private`: spec 022's demo-page chrome draws a MOCK WEBSITE, which is
/// the one surface that is deliberately not a wallet type role.
enum Face {
    case regular, medium, semiBold, bold
    /// The system monospaced design for data values (addresses, diagnostics).
    case mono

    var weight: Font.Weight {
        switch self {
        case .regular, .mono: .regular
        case .medium: .medium
        case .semiBold: .semibold
        case .bold: .bold
        }
    }

    var uiWeight: UIFont.Weight {
        switch self {
        case .regular, .mono: .regular
        case .medium: .medium
        case .semiBold: .semibold
        case .bold: .bold
        }
    }

    var design: Font.Design { self == .mono ? .monospaced : .default }

    /// The face at `size`, unscaled.
    func font(_ size: CGFloat) -> Font { .system(size: size, weight: weight, design: design) }

    /// The face at `size` as Dynamic Type scales `style` at `dynamicTypeSize`.
    func font(_ size: CGFloat, relativeTo style: Font.TextStyle, at dynamicTypeSize: DynamicTypeSize) -> Font {
        font(UIFontMetrics(forTextStyle: style.uiStyle).scaledValue(for: size, compatibleWith: dynamicTypeSize.traits))
    }

    /// The face at `size`, unscaled, for CoreText / TextKit.
    func uiFont(_ size: CGFloat) -> UIFont {
        self == .mono
            ? .monospacedSystemFont(ofSize: size, weight: uiWeight)
            : .systemFont(ofSize: size, weight: uiWeight)
    }
}

/// A complete typography role: face/size plus the Dynamic Type style it
/// scales relative to (native OS scaling kept deliberately — founder
/// direction), on top of the app-level text scale (设置 → 字号).
struct TypeRole {
    let face: Face
    let size: CGFloat
    let relativeTo: Font.TextStyle
    let leading: CGFloat // line-height multiplier from Tokens.Leading
    /// Whether a caller already multiplied by the person's chosen size.
    ///
    /// `typeRole` applies the size itself, from the environment, so a screen
    /// that never heard of 设置 → 字号 grows with it too — which is how 228 of
    /// the app's 494 text sites came to ignore the setting entirely (settings,
    /// onboarding and the Trusted Signer's sheets among them; found 2026-09-23).
    /// A role that went through [`scaled`] says so here, and is left alone.
    var pinned: Bool = false
    /// Whether the face's contextual alternates are on. A face may turn an `x`
    /// or a `-` after a digit into `×` or `−` — the bundled face the app used
    /// to ship did, so an address read "0×14fB…" (iPhone pass 2026-10-09).
    /// Off for a line that can carry an address or a hash ([`literal`]),
    /// whatever the face: docs/FONT-POLICY.md §4.
    var contextual: Bool = true

    /// The role at the Dynamic Type size in force for `traits` (nil: the
    /// current one). A literal role is a `UIFont` — SwiftUI's `Font` has no
    /// feature settings — and the rest is SwiftUI's own system font.
    func font(in traits: UITraitCollection?) -> Font {
        guard contextual else { return Font(uiFont(in: traits)) }
        let scaled = UIFontMetrics(forTextStyle: relativeTo.uiStyle)
            .scaledValue(for: size, compatibleWith: traits)
        return face.font(scaled)
    }

    /// The role at `dynamicTypeSize` — for a `Text` that has to stay a `Text`
    /// (concatenation), in a view that reads the size from its environment so
    /// it follows a change of it. Any other view uses `roleFont`.
    func font(at dynamicTypeSize: DynamicTypeSize) -> Font { font(in: dynamicTypeSize.traits) }

    /// The role at the Dynamic Type size current when it is read — a snapshot.
    /// Prefer `typeRole`, `roleFont` or `font(at:)`, which follow a change.
    var font: Font { font(in: nil) }

    /// The same role drawn as written: contextual alternates off, so "0x"
    /// stays "0x". For text that can carry an address, a hash or a name that
    /// falls back to one — the rest of the app keeps the face as designed.
    var literal: TypeRole {
        TypeRole(
            face: face, size: size, relativeTo: relativeTo, leading: leading,
            pinned: pinned, contextual: false
        )
    }

    /// The role as a `UIFont`, Dynamic-Type scaled for `traits` (nil: the
    /// current ones) as the SwiftUI one is, with the contextual alternates
    /// off when the role is [`literal`].
    func uiFont(in traits: UITraitCollection?) -> UIFont {
        var base = face.uiFont(size)
        if !contextual {
            let off = base.fontDescriptor.addingAttributes([
                .featureSettings: [[
                    UIFontDescriptor.FeatureKey.type: kContextualAlternatesType,
                    UIFontDescriptor.FeatureKey.selector: kContextualAlternatesOffSelector,
                ]],
            ])
            base = UIFont(descriptor: off, size: size)
        }
        return UIFontMetrics(forTextStyle: relativeTo.uiStyle)
            .scaledFont(for: base, compatibleWith: traits)
    }

    /// Extra spacing SwiftUI needs to reach the token line height.
    var lineSpacing: CGFloat { size * (leading - 1) }

    /// The same role as a `UIFont` at the current Dynamic Type size — for the
    /// TextKit-drawn views and for measuring a line before it is laid out.
    var uiFont: UIFont { uiFont(in: nil) }
}

private extension Font.TextStyle {
    var uiStyle: UIFont.TextStyle {
        switch self {
        case .largeTitle: .largeTitle
        case .title: .title1
        case .title2: .title2
        case .title3: .title3
        case .headline: .headline
        case .subheadline: .subheadline
        case .callout: .callout
        case .footnote: .footnote
        case .caption: .caption1
        case .caption2: .caption2
        default: .body
        }
    }
}

enum Typography {
    /// Wordmark / hero display — text.t32, bold, single-line.
    static let display = TypeRole(face: .bold, size: Tokens.TextSize.t32, relativeTo: .largeTitle, leading: Tokens.Leading.none)
    /// The v2 Welcome headline (spec 019). Sized by `WelcomeGeometry`, which
    /// is where the design's 46/38 pair lives — the DTCG scale stops at 40.
    static let hero = TypeRole(
        face: .bold,
        size: WelcomeGeometry.heroSize,
        relativeTo: .largeTitle,
        leading: WelcomeGeometry.heroLeading
    )
    /// The same headline for a locale that needs the next rung down; picked by
    /// `HeroFit`, never by the view.
    static let heroLong = TypeRole(
        face: .bold,
        size: WelcomeGeometry.heroSizeLong,
        relativeTo: .largeTitle,
        leading: WelcomeGeometry.heroLeading
    )
    /// The v2 wordmark beside the mark: small, heavy, widely tracked — a label,
    /// not a title. The tracking itself is applied at the call site.
    static let wordmark = TypeRole(
        face: .bold,
        size: WelcomeGeometry.wordmarkSize,
        relativeTo: .body,
        leading: Tokens.Leading.none
    )
    /// Screen tagline — text.t17, regular.
    static let tagline = TypeRole(face: .regular, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.normal)
    /// Card / section title — text.t20, semibold.
    static let title = TypeRole(face: .semiBold, size: Tokens.TextSize.t20, relativeTo: .title3, leading: Tokens.Leading.tight)
    /// Body copy — text.t15, regular, reading leading.
    static let body = TypeRole(face: .regular, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Small label (card numeral) — text.t13, medium.
    static let label = TypeRole(face: .medium, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.tight)
    /// Button label — text.t17, semibold, single-line.
    static let button = TypeRole(face: .semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.none)
    /// Flow secondary caption — text.t13, regular, reading leading (flow
    /// helper lines, step counter, sub-captions — spec 014). Named apart from
    /// the wallet `caption` role below, which is a different scale (t11).
    static let flowCaption = TypeRole(face: .regular, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.normal)
    /// Form field label — text.t15, medium (spec 014 name field).
    static let fieldLabel = TypeRole(face: .medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Monospaced data — addresses (spec 014 address strip), in the system
    /// mono face (SF Mono).
    static let mono = TypeRole(face: .mono, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Small monospaced diagnostics — tech-details code/endpoint lines.
    static let monoSmall = TypeRole(face: .mono, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.normal)

    // MARK: Wallet roles (spec 015 — docs/design/wallet mocks)

    /// Hero balance integer part — text.t40, bold, amount leading.
    static let amountHero = TypeRole(face: .bold, size: Tokens.TextSize.t40, relativeTo: .largeTitle, leading: Tokens.Leading.amountHero)
    /// Hero balance decimals — de-emphasised trailing part, text.t26.
    static let amountHeroDecimals = TypeRole(face: .bold, size: Tokens.TextSize.t26, relativeTo: .title2, leading: Tokens.Leading.amountHero)
    /// The send form's figure, one role per rung of the hero ladder
    /// (`WalletFlowGeometry.amountHero…`, picked by `AmountRung`) — bold, the
    /// largest type on the screen because it is what the person came to decide.
    static let amountEntry = TypeRole(face: .bold, size: WalletFlowGeometry.amountHero, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    static let amountEntryCompact = TypeRole(face: .bold, size: WalletFlowGeometry.amountHeroCompact, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    static let amountEntryTight = TypeRole(face: .bold, size: WalletFlowGeometry.amountHeroTight, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    /// …and its unit ("BNB", "PLN"): a word beside a number — medium, and a rung
    /// of the type scale below the figure on each rung (26 / 20 / 17), as the
    /// web's `.suffix` steps `text-3xl` / `2xl` / `xl`.
    static let amountUnit = TypeRole(face: .medium, size: Tokens.TextSize.t26, relativeTo: .title2, leading: Tokens.Leading.none)
    static let amountUnitCompact = TypeRole(face: .medium, size: Tokens.TextSize.t20, relativeTo: .title3, leading: Tokens.Leading.none)
    static let amountUnitTight = TypeRole(face: .medium, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.none)
    /// The confirm page's unit ("xDAI" beside "0.45767"): medium and muted,
    /// in the Send hero's figure-to-unit proportion (`confirmUnitSize`), and
    /// scaling with the `.largeTitle` figure it sits beside.
    static let confirmUnit = TypeRole(face: .medium, size: WalletFlowGeometry.confirmUnitSize, relativeTo: .largeTitle, leading: Tokens.Leading.none)
    /// Body copy, emphasised — text.t15, semibold, body's leading. A chosen
    /// row's name (the speed list): weight says "selected", colour stays ink —
    /// accent is for moving money and submitting only.
    static let bodyStrong = TypeRole(face: .semiBold, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Row title (activity/asset primary line) — text.t17, semibold.
    static let rowTitle = TypeRole(face: .semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.tight)
    /// Row trailing value (amount/balance) — text.t17, semibold.
    static let rowValue = TypeRole(face: .semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.tight)
    /// Row subtitle / status line / day label — text.t13, regular.
    static let rowSub = TypeRole(face: .regular, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.tight)
    /// Action-card label (收款/转账/扫码) and empty-state title — text.t15, medium.
    static let actionLabel = TypeRole(face: .medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Empty-state title — text.t15, semibold.
    static let emptyTitle = TypeRole(face: .semiBold, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Caption (QR caption, gallery chrome) — text.t11, medium.
    static let caption = TypeRole(face: .medium, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.tight)
    /// Token-icon 3-letter glyph — text.t11, semibold.
    static let tokenGlyph = TypeRole(face: .semiBold, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)
    /// Tab-bar item label — text.t10, medium, single-line.
    static let tab = TypeRole(face: .medium, size: Tokens.TextSize.t10, relativeTo: .caption2, leading: Tokens.Leading.none)
    /// The browsing bar's boxed tab count (DESIGN N) — text.t13, semibold.
    static let tabCount = TypeRole(face: .semiBold, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.none)

    // MARK: Contacts roles (spec 018 — docs/design/contacts mocks)

    /// Screen title (通讯录 / 家人) — text.t26, bold (mocks C1/C4).
    static let pageTitle = TypeRole(face: .bold, size: Tokens.TextSize.t26, relativeTo: .title, leading: Tokens.Leading.tight)
    /// A–Z letter-section header + index-rail letters — text.t11, semibold.
    static let sectionLetter = TypeRole(face: .semiBold, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)
    /// Index-rail bubble HUD letter — text.t26, bold.
    static let bubbleLetter = TypeRole(face: .bold, size: Tokens.TextSize.t26, relativeTo: .title, leading: Tokens.Leading.none)
    /// Menu-sheet row label (新建联系人 …) — text.t15, medium.
    static let menuRow = TypeRole(face: .medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Group chip label (家人 / + 分组) — text.t11, medium.
    static let chip = TypeRole(face: .medium, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)

    /// Detail short address / group-member address — system mono, text.t13.
    static let monoAddressDetail = MonoTypeRole(size: Tokens.TextSize.t13, weight: .regular, relativeTo: .footnote)
    /// Full address block (C2 two-line mono) — system mono, text.t15.
    static let monoAddressBlock = MonoTypeRole(size: Tokens.TextSize.t15, weight: .regular, relativeTo: .subheadline)

    /// Middle-truncated address — system monospaced, text.t11.
    static let monoAddress = MonoTypeRole(size: Tokens.TextSize.t11, weight: .regular, relativeTo: .caption)
}

/// The share card's faces (R4, `ShareCardGeometry`).
///
/// Not type roles, on purpose. The card is a picture drawn at fixed sizes —
/// it must not grow with Dynamic Type or 设置 → 字号 — and its lines are
/// MEASURED before they are drawn: the headline shrinks to fit, the name is
/// cut to its room, the account row centres on its own width. So each face is
/// a `UIFont`, measured by CoreText and drawn from CoreText's own runs
/// (`text`), and the width a line was measured at is the width it is drawn at.
enum ShareCardType {
    enum Weight { case medium, bold }

    /// The system face at `weight`. Its own fallback is weight-matched: a
    /// medium line's Chinese is PingFang Medium, its Cyrillic SF Medium — the
    /// faces the web's card falls back to.
    static func sans(_ size: CGFloat, _ weight: Weight) -> UIFont {
        .systemFont(ofSize: size, weight: weight == .bold ? .bold : .medium)
    }

    /// The system mono face, as the app's own address lines use.
    static func mono(_ size: CGFloat) -> UIFont {
        .monospacedSystemFont(ofSize: size, weight: .regular)
    }

    /// How wide `text` draws in `font`, on one line.
    static func width(_ text: String, _ font: UIFont) -> CGFloat {
        (text as NSString).size(withAttributes: [.font: font]).width
    }

    /// `text` in `font` as a `Text` — each run in the face CoreText itself
    /// picked for it, so the line drawn is the line `width` measured.
    ///
    /// SwiftUI does its own fallback, and a bundled face handed to it once
    /// came out with the Chinese and the Russian of the network note in the
    /// REGULAR system face. Resolved here, every run names its face, so the
    /// picture never depends on two engines agreeing.
    static func text(_ text: String, _ font: UIFont) -> Text {
        let line = CTLineCreateWithAttributedString(
            NSAttributedString(string: text, attributes: [.font: font]))
        // In the string's order, not the line's: a right-to-left name's runs
        // come back in visual order, and SwiftUI lays the bidi out itself.
        let runs = (CTLineGetGlyphRuns(line) as? [CTRun] ?? [])
            .sorted { CTRunGetStringRange($0).location < CTRunGetStringRange($1).location }
        let characters = text as NSString
        var drawn = AttributedString()
        for run in runs {
            let range = CTRunGetStringRange(run)
            var piece = AttributedString(characters.substring(
                with: NSRange(location: range.location, length: range.length)))
            let attributes = CTRunGetAttributes(run) as NSDictionary
            if let face = attributes[kCTFontAttributeName],
               CFGetTypeID(face as CFTypeRef) == CTFontGetTypeID() {
                piece.font = Font(face as! CTFont)
            } else {
                piece.font = Font(font as CTFont)
            }
            drawn += piece
        }
        return runs.isEmpty ? Text(verbatim: text).font(Font(font as CTFont)) : Text(drawn)
    }
}

/// A monospaced role (addresses, seeds), in the system mono face (SF Mono).
struct MonoTypeRole {
    let size: CGFloat
    let weight: Font.Weight
    let relativeTo: Font.TextStyle
    var font: Font { .system(size: size, weight: weight, design: .monospaced) }
}

extension TypeRole {
    /// FR-011 wallet text scale: the same role at a multiplied point size
    /// (H7x = 1.35×). Line-height multiplier carries over unchanged.
    ///
    /// The result is [`pinned`] whatever the factor: a caller that did the
    /// arithmetic owns the answer, and `typeRole` must not multiply again.
    func scaled(_ factor: CGFloat) -> TypeRole {
        TypeRole(
            face: face,
            size: size * factor,
            relativeTo: relativeTo,
            leading: leading,
            pinned: true,
            contextual: contextual
        )
    }
}

extension MonoTypeRole {
    func scaled(_ factor: CGFloat) -> MonoTypeRole {
        factor == 1 ? self : MonoTypeRole(size: size * factor, weight: weight, relativeTo: relativeTo)
    }
}

/// A role drawn at the size this person chose (设置 → 字号), unless the caller
/// already applied it.
///
/// The multiplier rides in the environment — `walletTextScale` — so a screen
/// with its own scale (the gallery's 1.35× chip) still overrides it for its
/// subtree, and everything else inherits the one the root sets.
private struct ScaledTypeRole: ViewModifier {
    @Environment(\.walletTextScale) private var scale
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let role: TypeRole

    func body(content: Content) -> some View {
        let drawn = role.pinned ? role : role.scaled(scale)
        return content.font(drawn.font(in: dynamicTypeSize.traits)).lineSpacing(drawn.lineSpacing)
    }
}

/// A role's font alone, at the Dynamic Type size in force — read from the
/// environment, so the view follows a change of it. No text scale and no line
/// spacing: for a field or a label whose caller already scaled the role.
private struct RoleFont: ViewModifier {
    @Environment(\.dynamicTypeSize) private var dynamicTypeSize
    let role: TypeRole

    func body(content: Content) -> some View {
        content.font(role.font(in: dynamicTypeSize.traits))
    }
}

extension DynamicTypeSize {
    /// The trait collection `UIFontMetrics` scales for at this size.
    var traits: UITraitCollection {
        UITraitCollection(preferredContentSizeCategory: UIContentSizeCategory(self))
    }
}

/// The one SF Symbol still in use: the gallery's dev-only theme toggle.
/// Wallet iconography renders through `LucideIcon` (research D2 rev);
/// `LucideIconSize` carries the per-slot point sizes these recipes used to.
enum WalletIconFont {
    static let galleryControl = Font.system(size: 17, weight: .medium)
}

extension Text {
    /// Applies a complete type role (font + line spacing) — the only
    /// sanctioned way to style text outside DesignSystem/.
    func typeRole(_ role: TypeRole) -> some View {
        modifier(ScaledTypeRole(role: role))
    }

    /// Applies a monospaced role (addresses/seeds).
    func monoRole(_ role: MonoTypeRole) -> some View {
        self.font(role.font)
    }
}

extension View {
    /// A role's font for a view that is not a `Text` (a field, a label) —
    /// Dynamic Type followed live, nothing else applied.
    func roleFont(_ role: TypeRole) -> some View {
        modifier(RoleFont(role: role))
    }
}
