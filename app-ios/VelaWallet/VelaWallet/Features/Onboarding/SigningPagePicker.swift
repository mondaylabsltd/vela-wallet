//
//  SigningPagePicker.swift
//  VelaWallet
//
//  "Use a trusted signing page" (spec 102, D6): the choosers' advanced entry.
//  Its list is Vela's official signing page and the self-hosted ones.
//
//  The create and sign-in choosers list where a key LIVES — this device, a
//  phone or tablet, a USB key — and nothing else. Where a person reviews and
//  signs is a separate choice, and this is the one place a NEW wallet makes
//  it: Vela's own sheet, or one of the pages this device trusts
//  (Settings → Signing pages). Each page says the two things that decide
//  whether to pick it:
//
//  - the domain its keys would live on — a page on somebody's own domain
//    mints keys only it can use, so the wallet is bound to that page;
//  - its integrity line — this phone's check of the very bytes it would open
//    (`SignerPageChecks`), never a claim of "verified".
//
//  Nothing here decides: the core normalises the page and derives the
//  wallet's signing domain from it (`signing_page_chosen`, `sign_in {page}`).
//

import SwiftUI
import VelaCore

/// One page of the picker, already in words.
struct SigningPageChoiceModel: Equatable, Identifiable {
    /// `nil` is Vela's own sheet.
    let url: String?
    let title: String
    /// The page's host — or, for Vela's own sheet, what it is.
    let subtitle: String
    /// "Keys on {{domain}}" — `nil` for Vela's own sheet.
    let domainLine: String?
    /// The page's integrity line — `nil` for Vela's own sheet.
    let line: SignerIntegrityLine?
    let selected: Bool

    var id: String { url ?? "in_vela" }
}

enum SigningPagePickerModel {
    /// Vela's own sheet first, then every page this device trusts (official
    /// first), each with its domain and line.
    static func choices(
        pages: [SigningPageRowWire], selected: String?, loc: Loc,
        line: (String) -> SignerIntegrityLine
    ) -> [SigningPageChoiceModel] {
        let inVela = venueWords(row: "in_vela")
        let selectedKey = selected.map(SignerPageChecks.key)
        return [
            SigningPageChoiceModel(
                url: nil,
                title: inVela.map { loc.t($0.titleKey) } ?? "",
                subtitle: inVela?.lineKey.map { loc.t($0) } ?? "",
                domainLine: nil, line: nil, selected: selected == nil
            ),
        ] + pages.map { page in
            SigningPageChoiceModel(
                url: page.url,
                title: SigningPageNames.name(
                    url: page.url, label: page.name, official: page.official, domain: page.domain, loc: loc
                ),
                subtitle: SigningPageNames.host(page.url),
                domainLine: page.domain.isEmpty ? nil : loc.t("settings.signing.keysOn", vars: ["domain": page.domain]),
                line: line(page.url),
                selected: selectedKey == SignerPageChecks.key(page.url)
            )
        }
    }
}

/// The picker itself — a sheet's body.
struct SigningPagePicker: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let choices: [SigningPageChoiceModel]
    let onPick: (String?) -> Void
    /// "Add a page": the address as typed. `nil` hides the field (a gallery).
    var onAdd: ((String) -> Void)?
    /// The core's refusal of the last address added, as a corpus key.
    var addError: String?
    var onClose: (() -> Void)?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s16) {
                header
                VStack(spacing: 0) {
                    ForEach(Array(choices.enumerated()), id: \.element.id) { index, choice in
                        if index > 0 { Divider().overlay(theme.borderBase) }
                        SigningPageChoiceRow(loc: loc, choice: choice) { onPick(choice.url) }
                    }
                }
                .background(theme.bgRaised, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
                .overlay(
                    RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                        .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
                )
                if let onAdd { SigningPageAddField(loc: loc, errorKey: addError, onAdd: onAdd) }
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.vertical, Tokens.Space.s24)
        }
        .scrollBounceBehavior(.basedOnSize)
        .background(theme.bgBase.ignoresSafeArea())
        .accessibilityIdentifier("signingPagePicker")
    }

    private var header: some View {
        let entry = venueWords(row: "signing_page")
        return HStack(alignment: .top) {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(entry.map { loc.t($0.titleKey) } ?? "")
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                Text(entry?.lineKey.map { loc.t($0) } ?? "")
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgMuted)
                    .fixedSize(horizontal: false, vertical: true)
            }
            Spacer(minLength: Tokens.Space.s8)
            if let onClose {
                Button(action: onClose) {
                    LucideIcon(.close, size: LucideIconSize.action).foregroundStyle(theme.fgSubtle)
                }
                .frame(minWidth: Tokens.Layout.hitTarget, minHeight: Tokens.Layout.hitTarget)
                .accessibilityLabel(loc.t("common.cancel"))
            }
        }
    }

}

/// "Add a page": the address, typed, and Save. What may be stored is the
/// core's call; its refusal is drawn under the field.
struct SigningPageAddField: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    /// The corpus key of the last refusal.
    var errorKey: String?
    let onAdd: (String) -> Void

    @State private var draft = ""

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s8) {
            Text(loc.t("settings.signing.pageAdd"))
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgMuted)
            HStack(spacing: Tokens.Space.s8) {
                TextField("", text: $draft, prompt: Text(verbatim: "https://").foregroundStyle(theme.fgSubtle))
                    .textInputAutocapitalization(.never)
                    .autocorrectionDisabled()
                    .keyboardType(.URL)
                    .font(Typography.mono.font)
                    .foregroundStyle(theme.fgBase)
                    .padding(.horizontal, Tokens.Space.s12)
                    .frame(minHeight: Tokens.Layout.hitTarget)
                    .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r8))
                    .accessibilityIdentifier("signingPage.addField")
                    .onSubmit(save)
                Button(action: save) {
                    Text(loc.t("settings.signing.pageSave"))
                        .typeRole(Typography.actionLabel)
                        .foregroundStyle(theme.accentBase)
                        .padding(.horizontal, Tokens.Space.s12)
                        .frame(minHeight: Tokens.Layout.hitTarget)
                }
                .accessibilityIdentifier("signingPage.addSave")
            }
            if let errorKey {
                Text(loc.t(errorKey))
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.errorBase)
                    .fixedSize(horizontal: false, vertical: true)
                    .accessibilityIdentifier("signingPage.addError")
            }
        }
    }

    private func save() {
        let typed = draft.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !typed.isEmpty else { return }
        onAdd(typed)
        draft = ""
    }
}

/// One row: a radio, the page's name and host, the domain its keys live on,
/// and its integrity line.
struct SigningPageChoiceRow: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let choice: SigningPageChoiceModel
    let onTap: () -> Void

    var body: some View {
        Button(action: onTap) {
            HStack(alignment: .top, spacing: Tokens.Space.s12) {
                ZStack {
                    Circle()
                        .stroke(choice.selected ? theme.accentBase : theme.borderStrong,
                                lineWidth: Tokens.BorderWidth.emphasis)
                    if choice.selected {
                        Circle().fill(theme.accentBase).padding(Tokens.Space.s4)
                    }
                }
                .frame(width: Tokens.Space.s20, height: Tokens.Space.s20)
                .padding(.top, Tokens.Space.s2)

                VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                    Text(choice.title)
                        .typeRole(Typography.rowTitle)
                        .foregroundStyle(theme.fgBase)
                    // A page's host, unless its name already says it.
                    if choice.url == nil || !choice.title.contains(choice.subtitle) {
                        Text(choice.subtitle)
                            .typeRole(choice.url == nil ? Typography.flowCaption : Typography.monoSmall)
                            .foregroundStyle(theme.fgMuted)
                            .lineLimit(2)
                    }
                    if let domain = choice.domainLine {
                        Text(domain)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgMuted)
                    }
                    if let line = choice.line {
                        IntegrityLineView(loc: loc, line: line)
                            .padding(.top, Tokens.Space.s2)
                    }
                }
                .frame(maxWidth: .infinity, alignment: .leading)
                .multilineTextAlignment(.leading)
            }
            .padding(Tokens.Space.s16)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityAddTraits(choice.selected ? .isSelected : [])
        .accessibilityIdentifier("signingPage.\(choice.url.map(SigningPageNames.host) ?? "inVela")")
    }
}

/// The chosen page, above a create's key list or a sign-in's places: which
/// page, which domain the keys belong to, and its line. Tappable to change
/// while the choice is still open.
struct ChosenSigningPageCard: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let choice: SigningPageChoiceModel
    var onChange: (() -> Void)?

    var body: some View {
        let content = HStack(alignment: .center, spacing: Tokens.Space.s12) {
            ZStack {
                Circle().fill(theme.accentSoft)
                LucideIcon(.globe, size: LucideIconSize.rowGlyph).foregroundStyle(theme.accentBase)
            }
            .frame(width: Tokens.Layout.hitTarget - Tokens.Space.s8, height: Tokens.Layout.hitTarget - Tokens.Space.s8)
            .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(choice.title)
                    .typeRole(Typography.rowTitle)
                    .foregroundStyle(theme.fgBase)
                if !choice.title.contains(choice.subtitle) {
                    Text(choice.subtitle)
                        .typeRole(Typography.monoSmall)
                        .foregroundStyle(theme.fgMuted)
                }
                if let domain = choice.domainLine {
                    Text(domain)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                }
                if let line = choice.line {
                    IntegrityLineView(loc: loc, line: line).padding(.top, Tokens.Space.s2)
                }
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            .multilineTextAlignment(.leading)
            if onChange != nil {
                LucideIcon(.chevronRight, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgSubtle)
                    .accessibilityHidden(true)
            }
        }
        .padding(Tokens.Space.s16)
        .background(theme.bgSunken, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .accessibilityIdentifier("signingPage.chosen")

        if let onChange {
            Button(action: onChange) { content }.buttonStyle(.plain)
        } else {
            content
        }
    }
}

/// The choosers' advanced entry row — "Use a trusted signing page" and its
/// line, set apart from the three places it is not one of.
struct SigningPageEntry: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let onTap: () -> Void

    var body: some View {
        let words = venueWords(row: "signing_page")
        Button(action: onTap) {
            HStack(spacing: Tokens.Space.s12) {
                LucideIcon(.globe, size: LucideIconSize.rowGlyph)
                    .foregroundStyle(theme.fgMuted)
                    .accessibilityHidden(true)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(words.map { loc.t($0.titleKey) } ?? "")
                        .typeRole(Typography.bodyStrong)
                        .foregroundStyle(theme.fgBase)
                    Text(words?.lineKey.map { loc.t($0) } ?? "")
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                        .multilineTextAlignment(.leading)
                        .fixedSize(horizontal: false, vertical: true)
                }
                Spacer()
                Image(systemName: "chevron.right").foregroundStyle(theme.fgSubtle)
            }
            .frame(minHeight: Tokens.Layout.hitTarget)
            .padding(.vertical, Tokens.Space.s8)
            .contentShape(Rectangle())
        }
        .buttonStyle(.plain)
        .accessibilityIdentifier("chooser.signingPage")
    }
}

/// The gallery's pages: the official one checked and matching, and a
/// self-hosted page whose check could not run — the two lines a person most
/// often meets. Data, never shown outside the gallery and the screenshot tests.
enum SigningPageFixtures {
    static let selfHosted = "https://sign.example.com/"

    static let pages: [SigningPageRowWire] = [
        SigningPageRowWire(url: "https://sign.getvela.app/", name: "", domain: "getvela.app", official: true),
        SigningPageRowWire(url: selfHosted, name: "", domain: "sign.example.com", official: false),
    ]

    static func line(_ url: String) -> SignerIntegrityLine {
        url == selfHosted
            ? SignerIntegrityLine(
                state: .couldNotCheck, version: "", checkedAtMs: nil,
                key: "componentsUi.signing.integrity.couldNotCheck", opens: false
            )
            : SignerIntegrityLine(
                state: .matches, version: "0ba8ee8c",
                checkedAtMs: UInt64(Date().timeIntervalSince1970 * 1000) - 120_000,
                key: "componentsUi.signing.integrity.matches", opens: true
            )
    }
}
