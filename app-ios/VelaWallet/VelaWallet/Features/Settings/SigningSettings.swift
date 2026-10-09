//
//  SigningSettings.swift
//  VelaWallet
//
//  Spec 102 in Settings: two surfaces, one per thing a person chooses.
//
//  - **Settings → Signing pages** (device-wide): the pages this device trusts
//    to show and sign requests. The official page is always first and cannot
//    be removed; others are added by address, renamed, removed. Each row says
//    the domain its keys live on and this phone's integrity line for it. The
//    071 free-text "Trusted Signer page" field is gone — a field anyone can be
//    talked into editing is a social-engineering door.
//  - **Account → Where you review and sign** (per account, this device): In
//    Vela, or one of those pages. A choice that cannot reach the account's
//    keys (R1) is drawn disabled with the core's reason; an account on its own
//    domain is locked to pages there (R2). Keys never change.
//
//  Every judgement is the core's: `SigningPagesCore`'s list, `signingPlan`,
//  `signingVenueChoices` and its `VenueBlock`s, the integrity rulings
//  (`SignerPageChecks`). This file is models, one projection and the views.
//

import SwiftUI
import VelaCore

// MARK: - Models

/// One row of "Where you review and sign".
struct VenueChoiceRowModel: Identifiable, Equatable {
    let id: String
    let venue: SigningVenueWire
    let title: String
    /// What it is: "Vela's own signing sheet", or the page's host.
    let subtitle: String
    /// A page's integrity line.
    var line: SignerIntegrityLine?
    /// Why it cannot reach this account's keys — the row is disabled.
    var reason: String?
    let active: Bool

    var enabled: Bool { reason == nil }
}

/// The account's "Where you review and sign": its row, and its sheet.
struct VenueSettingModel {
    static let rowId = "signing-venue"

    let row: SettingsRowModel
    let title: String
    let subtitle: String
    /// "Keys on getvela.app" — the account's signing domain.
    let domainLine: String
    let inVela: VenueChoiceRowModel?
    /// "On a trusted page", and what that means.
    let pagesHeader: String
    let pagesBody: String
    let pages: [VenueChoiceRowModel]
}

/// One saved page in Settings → Signing pages.
struct SigningPageRowModel: Identifiable, Equatable {
    var id: String { url }
    let url: String
    let title: String
    let host: String
    let domainLine: String
    let line: SignerIntegrityLine
    /// The official page cannot be renamed or removed.
    let official: Bool
    /// The person's own label, for the rename field.
    let label: String
}

/// Settings → Signing pages.
struct SigningPagesPageModel {
    static let rowId = "signing-pages"

    let title: String
    let subtitle: String
    var rows: [SigningPageRowModel]
    let addLabel: String
    /// The core's refusal of the last address added, as a corpus key.
    var addErrorKey: String?
    /// Edits are offered only once the list has been read.
    var loaded: Bool
    let rename: String
    let remove: String
    let save: String
    let cancel: String
}

/// What the two surfaces raise. Absent in the gallery.
struct SigningSettingsActions {
    var onChooseVenue: (SigningVenueWire) -> Void = { _ in }
    var onAddPage: (String) -> Void = { _ in }
    var onRenamePage: (_ url: String, _ name: String) -> Void = { _, _ in }
    var onRemovePage: (String) -> Void = { _ in }
    /// A list of pages is on screen: check them.
    var onPagesShown: () -> Void = {}
}

// MARK: - The projection

extension SettingsLive {

    /// Spec 102: the account's venue row and sheet, the signing pages page,
    /// and — for an account on its own domain — the keys block's domain line.
    /// `plan` is the account's (`signingPlan`), `nil` before it is read or for
    /// an account the core cannot read: no row then, never a guessed one.
    static func withSigning(
        plan: SigningPlanWire?,
        pages: SigningPagesViewWire?,
        line: (String) -> SignerIntegrityLine,
        on model: SettingsScreenModel,
        loc: Loc
    ) -> SettingsScreenModel {
        var copy = model
        let saved = pages?.saved ?? []
        copy.signingPages = signingPagesPage(pages, line: line, loc: loc)
        copy.venue = plan.map { venueSetting($0, saved: saved, rows: pages?.pages ?? [], line: line, loc: loc) }
        if let plan, !plan.onAppDomain {
            copy.keys?.domainLine = loc.t("settings.signing.keysOn", vars: ["domain": plan.domain])
        }
        return copy
    }

    static func signingPagesPage(
        _ view: SigningPagesViewWire?, line: (String) -> SignerIntegrityLine, loc: Loc
    ) -> SigningPagesPageModel {
        let rows = (view?.pages ?? SigningPagesViewWire.initial?.pages ?? []).map { page in
            SigningPageRowModel(
                url: page.url,
                title: SigningPageNames.name(url: page.url, label: page.name, official: page.official, loc: loc),
                host: SigningPageNames.host(page.url),
                domainLine: loc.t("settings.signing.keysOn", vars: ["domain": page.domain]),
                line: line(page.url),
                official: page.official,
                label: page.name
            )
        }
        return SigningPagesPageModel(
            title: loc.t("settings.signing.title"),
            subtitle: loc.t("settings.signing.subtitle"),
            rows: rows,
            addLabel: loc.t("settings.signing.pageAdd"),
            addErrorKey: SigningPagesViewWire.addErrorKey(view?.addError),
            loaded: view?.loaded ?? false,
            // Borrowed until the corpus has settings-own words for them.
            rename: loc.t("explore.rename"),
            remove: loc.t("settingsModals.network.removeConfirm"),
            save: loc.t("settings.signing.pageSave"),
            cancel: loc.t("common.cancel")
        )
    }

    static func venueSetting(
        _ plan: SigningPlanWire, saved: [SigningPageWire], rows: [SigningPageRowWire],
        line: (String) -> SignerIntegrityLine, loc: Loc
    ) -> VenueSettingModel {
        let choices = VenueChoiceWire.choices(domain: plan.domain, active: plan.venue, saved: saved)
        let inVelaWords = venueWords(row: "in_vela")
        func model(_ choice: VenueChoiceWire) -> VenueChoiceRowModel {
            switch choice.venue {
            case .inVela:
                return VenueChoiceRowModel(
                    id: choice.id, venue: choice.venue,
                    title: inVelaWords.map { loc.t($0.titleKey) } ?? "",
                    subtitle: inVelaWords?.lineKey.map { loc.t($0) } ?? "",
                    reason: choice.blocked?.text(loc), active: choice.active
                )
            case .page(let url):
                let label = choice.name.isEmpty
                    ? rows.first { SignerPageChecks.key($0.url) == SignerPageChecks.key(url) }?.name ?? ""
                    : choice.name
                return VenueChoiceRowModel(
                    id: choice.id, venue: choice.venue,
                    title: SigningPageNames.name(url: url, label: label, official: choice.official, loc: loc),
                    subtitle: SigningPageNames.host(url),
                    line: line(url),
                    reason: choice.blocked?.text(loc), active: choice.active
                )
            }
        }
        let inVela = choices.first { $0.venue == .inVela }.map(model)
        let pages = choices.filter { $0.venue != .inVela }.map(model)
        let active = (inVela.map { [$0] } ?? []).first(where: \.active) ?? pages.first(where: \.active)
        let pageWords = venueWords(row: "page")
        let value: String
        if let active, active.venue != .inVela {
            value = "\(pageWords.map { loc.t($0.titleKey) } ?? "") · \(active.title)"
        } else {
            value = active?.title ?? ""
        }
        return VenueSettingModel(
            row: SettingsRowModel(
                id: VenueSettingModel.rowId,
                title: loc.t("settings.venue.title"),
                icon: .lock,
                subtitle: value
            ),
            title: loc.t("settings.venue.title"),
            subtitle: loc.t("settings.venue.subtitle"),
            domainLine: loc.t("settings.signing.keysOn", vars: ["domain": plan.domain]),
            inVela: inVela,
            pagesHeader: pageWords.map { loc.t($0.titleKey) } ?? "",
            pagesBody: pageWords?.lineKey.map { loc.t($0) } ?? "",
            pages: pages
        )
    }
}

// MARK: - Settings → Signing pages

struct SigningPagesBody: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let panel: SigningPagesPageModel
    let actions: SigningSettingsActions?

    @State private var renaming: SigningPageRowModel?
    @State private var removing: SigningPageRowModel?
    @State private var renameDraft = ""

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            VStack(spacing: 0) {
                ForEach(Array(panel.rows.enumerated()), id: \.element.id) { index, row in
                    if index > 0 { Divider().overlay(theme.borderBase) }
                    SigningPageSettingsRow(
                        loc: loc, row: row, panel: panel,
                        onRename: actions != nil && panel.loaded && !row.official ? {
                            renameDraft = row.label
                            renaming = row
                        } : nil,
                        onRemove: actions != nil && panel.loaded && !row.official ? { removing = row } : nil
                    )
                }
            }
            .background(theme.bgRaised, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
            .overlay(
                RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                    .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
            )

            if let actions, panel.loaded {
                SigningPageAddField(loc: loc, errorKey: panel.addErrorKey, onAdd: actions.onAddPage)
            }
        }
        .padding(.top, Tokens.Space.s8)
        .alert(panel.rename, isPresented: Binding(get: { renaming != nil }, set: { if !$0 { renaming = nil } })) {
            TextField(renaming?.host ?? "", text: $renameDraft)
            Button(panel.save) {
                if let row = renaming { actions?.onRenamePage(row.url, renameDraft) }
                renaming = nil
            }
            Button(panel.cancel, role: .cancel) { renaming = nil }
        }
        .confirmationDialog(
            removing?.title ?? "",
            isPresented: Binding(get: { removing != nil }, set: { if !$0 { removing = nil } }),
            titleVisibility: .visible
        ) {
            Button(panel.remove, role: .destructive) {
                if let row = removing { actions?.onRemovePage(row.url) }
                removing = nil
            }
            Button(panel.cancel, role: .cancel) { removing = nil }
        }
    }
}

/// One page: its name, host, the domain its keys live on, and its line.
private struct SigningPageSettingsRow: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let row: SigningPageRowModel
    let panel: SigningPagesPageModel
    var onRename: (() -> Void)?
    var onRemove: (() -> Void)?

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s12) {
            LucideIcon(row.official ? .lock : .globe, size: LucideIconSize.action)
                .foregroundStyle(row.official ? theme.accentBase : theme.fgMuted)
                .frame(width: LucideIconSize.action)
                .padding(.top, Tokens.Space.s2)
                .accessibilityHidden(true)
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(row.title)
                    .typeRole(Typography.fieldLabel)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                Text(row.host)
                    .typeRole(Typography.monoSmall)
                    .foregroundStyle(theme.fgMuted)
                    .lineLimit(1)
                    .truncationMode(.middle)
                Text(row.domainLine)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                IntegrityLineView(loc: loc, line: row.line)
                    .padding(.top, Tokens.Space.s2)
            }
            .frame(maxWidth: .infinity, alignment: .leading)
            if onRename != nil || onRemove != nil {
                Menu {
                    if let onRename { Button(panel.rename, action: onRename) }
                    if let onRemove { Button(panel.remove, role: .destructive, action: onRemove) }
                } label: {
                    LucideIcon(.ellipsis, size: LucideIconSize.action)
                        .foregroundStyle(theme.fgSubtle)
                        .frame(minWidth: Tokens.Layout.hitTarget, minHeight: Tokens.Layout.hitTarget)
                }
                .accessibilityIdentifier("signingPage.menu.\(row.host)")
            }
        }
        .padding(Tokens.Space.s16)
        .accessibilityElement(children: .contain)
        .accessibilityIdentifier("signingPage.row.\(row.host)")
    }
}

// MARK: - Where you review and sign

/// The sheet's body: what the setting is, the account's domain, Vela's own
/// sheet, then the trusted pages — a page that cannot reach this account's
/// keys disabled, with the core's reason under it.
struct VenueSheetBody: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let model: VenueSettingModel
    var onChoose: ((SigningVenueWire) -> Void)?

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(model.title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                Text(model.subtitle)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
                HStack(spacing: Tokens.Space.s4) {
                    LucideIcon(.lock, size: LucideIconSize.smallChevron).foregroundStyle(theme.fgMuted)
                    Text(model.domainLine)
                        .typeRole(Typography.label)
                        .foregroundStyle(theme.fgMuted)
                }
                .padding(.top, Tokens.Space.s4)
                .accessibilityIdentifier("venue.domain")
            }

            if let inVela = model.inVela {
                group([inVela])
            }

            VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                Text(model.pagesHeader)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgMuted)
                Text(model.pagesBody)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .fixedSize(horizontal: false, vertical: true)
            }
            .padding(.top, Tokens.Space.s4)
            group(model.pages)
        }
        .padding(.bottom, Tokens.Space.s16)
    }

    private func group(_ rows: [VenueChoiceRowModel]) -> some View {
        VStack(spacing: 0) {
            ForEach(Array(rows.enumerated()), id: \.element.id) { index, row in
                if index > 0 { Divider().overlay(theme.borderBase) }
                VenueChoiceRow(loc: loc, row: row) { onChoose?(row.venue) }
            }
        }
        .background(theme.bgBase, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .overlay(
            RoundedRectangle(cornerRadius: Tokens.Radius.r12)
                .stroke(theme.borderBase, lineWidth: Tokens.BorderWidth.hairline)
        )
    }
}

private struct VenueChoiceRow: View {
    @Environment(\.theme) private var theme
    let loc: Loc
    let row: VenueChoiceRowModel
    let onTap: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            // The active row is SELECTED, not disabled: drawn at full strength,
            // and a tap on it changes nothing. Only a row that cannot reach
            // the keys is disabled.
            Button { if !row.active { onTap() } } label: {
                HStack(alignment: .top, spacing: Tokens.Space.s12) {
                    ZStack {
                        Circle()
                            .stroke(row.active ? theme.accentBase : theme.borderStrong,
                                    lineWidth: Tokens.BorderWidth.emphasis)
                        if row.active {
                            Circle().fill(theme.accentBase).padding(Tokens.Space.s4)
                        }
                    }
                    .frame(width: Tokens.Space.s20, height: Tokens.Space.s20)
                    .padding(.top, Tokens.Space.s2)
                    VStack(alignment: .leading, spacing: Tokens.Space.s4) {
                        Text(row.title)
                            .typeRole(Typography.rowTitle)
                            .foregroundStyle(theme.fgBase)
                        Text(row.subtitle)
                            .typeRole(row.venue == .inVela ? Typography.flowCaption : Typography.monoSmall)
                            .foregroundStyle(theme.fgMuted)
                        if row.reason == nil, let line = row.line {
                            IntegrityLineView(loc: loc, line: line)
                                .padding(.top, Tokens.Space.s2)
                        }
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .multilineTextAlignment(.leading)
                }
                .padding(.horizontal, Tokens.Space.s16)
                .padding(.top, Tokens.Space.s16)
                .padding(.bottom, row.reason == nil ? Tokens.Space.s16 : Tokens.Space.s8)
                .contentShape(Rectangle())
            }
            .buttonStyle(.plain)
            .disabled(!row.enabled)
            .accessibilityAddTraits(row.active ? .isSelected : [])
            .accessibilityIdentifier("venue.\(row.venue.pageUrl.map(SigningPageNames.host) ?? "inVela")")

            // Why it cannot be chosen — outside the disabled control, so it is
            // read at full strength (R1: never a dimmed row with no reason).
            if let reason = row.reason {
                HStack(alignment: .firstTextBaseline, spacing: Tokens.Space.s4) {
                    LucideIcon(.info, size: LucideIconSize.smallChevron)
                        .foregroundStyle(theme.fgMuted)
                        .accessibilityHidden(true)
                    Text(reason)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgMuted)
                        .fixedSize(horizontal: false, vertical: true)
                }
                .padding(.leading, Tokens.Space.s16 + Tokens.Space.s20 + Tokens.Space.s12)
                .padding(.trailing, Tokens.Space.s16)
                .padding(.bottom, Tokens.Space.s16)
                .accessibilityIdentifier("venue.reason")
            }
        }
    }
}

// MARK: - The gallery

/// The boards' signing facts: an account in Vela on `getvela.app`, the
/// official page matching, and somebody's own page that cannot reach this
/// account's keys.
enum SigningSettingsFixtures {
    static let plan = SigningPlanWire(
        domain: "getvela.app", venue: .page(url: "https://sign.getvela.app/"),
        key: KeyRouteWire(credentialId: "a1b2c3d4", method: "platform", transports: "internal")
    )

    static let pages = SigningPagesViewWire(
        pages: SigningPageFixtures.pages,
        saved: [SigningPageWire(url: SigningPageFixtures.ownPage)],
        loaded: true
    )

    static func apply(on model: SettingsScreenModel, loc: Loc) -> SettingsScreenModel {
        SettingsLive.withSigning(
            plan: plan, pages: pages, line: SigningPageFixtures.line, on: model, loc: loc
        )
    }
}
