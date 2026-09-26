//
//  Typography.swift
//  VelaWallet
//
//  Type roles per docs/design-system.md (complete recipes, never ad-hoc font
//  calls). Plus Jakarta Sans is bundled (DesignSystem/Fonts); CJK falls
//  through to the system face — DV-003, matching the shipped RN app.
//

import SwiftUI

/// Bundled Plus Jakarta Sans PostScript names (see DesignSystem/Fonts/).
/// Not `private`: spec 022's demo-page chrome draws a MOCK WEBSITE, which is
/// the one surface that is deliberately not a wallet type role.
enum FontName {
    static let regular = "PlusJakartaSans-Regular"
    static let medium = "PlusJakartaSans-Medium"
    static let semiBold = "PlusJakartaSans-SemiBold"
    static let bold = "PlusJakartaSans-Bold"
    /// System monospaced face for data values (addresses, diagnostics).
    static let mono = "Menlo-Regular"
}

/// A complete typography role: family/weight/size plus the Dynamic Type
/// style it scales relative to (native OS scaling kept deliberately —
/// founder direction; the app-level text-scale setting is out of scope).
struct TypeRole {
    let fontName: String
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

    var font: Font { .custom(fontName, size: size, relativeTo: relativeTo) }
    /// Extra spacing SwiftUI needs to reach the token line height.
    var lineSpacing: CGFloat { size * (leading - 1) }

    /// The same role as a `UIFont`, Dynamic-Type scaled like the SwiftUI one.
    /// For the single TextKit-drawn view in the app (`AckRow`'s legal line).
    var uiFont: UIFont {
        let base = UIFont(name: fontName, size: size) ?? .systemFont(ofSize: size)
        return UIFontMetrics(forTextStyle: relativeTo.uiStyle).scaledFont(for: base)
    }
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
    static let display = TypeRole(fontName: FontName.bold, size: Tokens.TextSize.t32, relativeTo: .largeTitle, leading: Tokens.Leading.none)
    /// The v2 Welcome headline (spec 019). Sized by `WelcomeGeometry`, which
    /// is where the design's 46/38 pair lives — the DTCG scale stops at 40.
    static let hero = TypeRole(
        fontName: FontName.bold,
        size: WelcomeGeometry.heroSize,
        relativeTo: .largeTitle,
        leading: WelcomeGeometry.heroLeading
    )
    /// The same headline for a locale that needs the next rung down; picked by
    /// `HeroFit`, never by the view.
    static let heroLong = TypeRole(
        fontName: FontName.bold,
        size: WelcomeGeometry.heroSizeLong,
        relativeTo: .largeTitle,
        leading: WelcomeGeometry.heroLeading
    )
    /// The v2 wordmark beside the mark: small, heavy, widely tracked — a label,
    /// not a title. The tracking itself is applied at the call site.
    static let wordmark = TypeRole(
        fontName: FontName.bold,
        size: WelcomeGeometry.wordmarkSize,
        relativeTo: .body,
        leading: Tokens.Leading.none
    )
    /// Screen tagline — text.t17, regular.
    static let tagline = TypeRole(fontName: FontName.regular, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.normal)
    /// Card / section title — text.t20, semibold.
    static let title = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t20, relativeTo: .title3, leading: Tokens.Leading.tight)
    /// Body copy — text.t15, regular, reading leading.
    static let body = TypeRole(fontName: FontName.regular, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Small label (card numeral) — text.t13, medium.
    static let label = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.tight)
    /// Button label — text.t17, semibold, single-line.
    static let button = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.none)
    /// Flow secondary caption — text.t13, regular, reading leading (flow
    /// helper lines, step counter, sub-captions — spec 014). Named apart from
    /// the wallet `caption` role below, which is a different scale (t11).
    static let flowCaption = TypeRole(fontName: FontName.regular, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.normal)
    /// Form field label — text.t15, medium (spec 014 name field).
    static let fieldLabel = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Monospaced data — addresses (spec 014 address strip). Menlo is the
    /// system mono face; data values, like CJK, fall outside Jakarta (DV-003).
    static let mono = TypeRole(fontName: FontName.mono, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Small monospaced diagnostics — tech-details code/endpoint lines.
    static let monoSmall = TypeRole(fontName: FontName.mono, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.normal)

    // MARK: Wallet roles (spec 015 — docs/design/wallet mocks)

    /// Hero balance integer part — text.t40, bold, amount leading.
    static let amountHero = TypeRole(fontName: FontName.bold, size: Tokens.TextSize.t40, relativeTo: .largeTitle, leading: Tokens.Leading.amountHero)
    /// Hero balance decimals — de-emphasised trailing part, text.t26.
    static let amountHeroDecimals = TypeRole(fontName: FontName.bold, size: Tokens.TextSize.t26, relativeTo: .title2, leading: Tokens.Leading.amountHero)
    /// The send form's figure, one role per rung of the hero ladder
    /// (`WalletFlowGeometry.amountHero…`, picked by `AmountRung`) — bold, the
    /// largest type on the screen because it is what the person came to decide.
    static let amountEntry = TypeRole(fontName: FontName.bold, size: WalletFlowGeometry.amountHero, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    static let amountEntryCompact = TypeRole(fontName: FontName.bold, size: WalletFlowGeometry.amountHeroCompact, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    static let amountEntryTight = TypeRole(fontName: FontName.bold, size: WalletFlowGeometry.amountHeroTight, relativeTo: .largeTitle, leading: Tokens.Leading.tight)
    /// …and its unit ("BNB", "PLN"): a word beside a number — medium, and a rung
    /// of the type scale below the figure on each rung (26 / 20 / 17), as the
    /// web's `.suffix` steps `text-3xl` / `2xl` / `xl`.
    static let amountUnit = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t26, relativeTo: .title2, leading: Tokens.Leading.none)
    static let amountUnitCompact = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t20, relativeTo: .title3, leading: Tokens.Leading.none)
    static let amountUnitTight = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.none)
    /// The confirm page's unit ("xDAI" beside "0.45767"): medium and muted,
    /// in the Send hero's figure-to-unit proportion (`confirmUnitSize`), and
    /// scaling with the `.largeTitle` figure it sits beside.
    static let confirmUnit = TypeRole(fontName: FontName.medium, size: WalletFlowGeometry.confirmUnitSize, relativeTo: .largeTitle, leading: Tokens.Leading.none)
    /// Body copy, emphasised — text.t15, semibold, body's leading. A chosen
    /// row's name (the speed list): weight says "selected", colour stays ink —
    /// accent is for moving money and submitting only.
    static let bodyStrong = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.normal)
    /// Row title (activity/asset primary line) — text.t17, semibold.
    static let rowTitle = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.tight)
    /// Row trailing value (amount/balance) — text.t17, semibold.
    static let rowValue = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t17, relativeTo: .body, leading: Tokens.Leading.tight)
    /// Row subtitle / status line / day label — text.t13, regular.
    static let rowSub = TypeRole(fontName: FontName.regular, size: Tokens.TextSize.t13, relativeTo: .footnote, leading: Tokens.Leading.tight)
    /// Action-card label (收款/转账/扫码) and empty-state title — text.t15, medium.
    static let actionLabel = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Empty-state title — text.t15, semibold.
    static let emptyTitle = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Caption (QR caption, gallery chrome) — text.t11, medium.
    static let caption = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.tight)
    /// Token-icon 3-letter glyph — text.t11, semibold.
    static let tokenGlyph = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)
    /// Tab-bar item label — text.t10, medium, single-line.
    static let tab = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t10, relativeTo: .caption2, leading: Tokens.Leading.none)

    // MARK: Contacts roles (spec 018 — docs/design/contacts mocks)

    /// Screen title (通讯录 / 家人) — text.t26, bold (mocks C1/C4).
    static let pageTitle = TypeRole(fontName: FontName.bold, size: Tokens.TextSize.t26, relativeTo: .title, leading: Tokens.Leading.tight)
    /// A–Z letter-section header + index-rail letters — text.t11, semibold.
    static let sectionLetter = TypeRole(fontName: FontName.semiBold, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)
    /// Index-rail bubble HUD letter — text.t26, bold.
    static let bubbleLetter = TypeRole(fontName: FontName.bold, size: Tokens.TextSize.t26, relativeTo: .title, leading: Tokens.Leading.none)
    /// Menu-sheet row label (新建联系人 …) — text.t15, medium.
    static let menuRow = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t15, relativeTo: .subheadline, leading: Tokens.Leading.tight)
    /// Group chip label (家人 / + 分组) — text.t11, medium.
    static let chip = TypeRole(fontName: FontName.medium, size: Tokens.TextSize.t11, relativeTo: .caption, leading: Tokens.Leading.none)

    /// Detail short address / group-member address — system mono, text.t13.
    static let monoAddressDetail = MonoTypeRole(size: Tokens.TextSize.t13, weight: .regular, relativeTo: .footnote)
    /// Full address block (C2 two-line mono) — system mono, text.t15.
    static let monoAddressBlock = MonoTypeRole(size: Tokens.TextSize.t15, weight: .regular, relativeTo: .subheadline)

    /// Middle-truncated address — system monospaced, text.t11.
    static let monoAddress = MonoTypeRole(size: Tokens.TextSize.t11, weight: .regular, relativeTo: .caption)
}

/// A monospaced role (addresses, seeds). Uses the system mono face — the
/// bundled Jakarta family has no mono cut; addresses are ASCII-only.
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
            fontName: fontName,
            size: size * factor,
            relativeTo: relativeTo,
            leading: leading,
            pinned: true
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
    let role: TypeRole

    func body(content: Content) -> some View {
        let drawn = role.pinned ? role : role.scaled(scale)
        return content.font(drawn.font).lineSpacing(drawn.lineSpacing)
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
