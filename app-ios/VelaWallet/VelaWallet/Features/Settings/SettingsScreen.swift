//
//  SettingsScreen.swift
//  VelaWallet
//
//  The settings surface (spec 023, ST1–ST16 + SR1–SR5).
//
//  One screen, not sixteen. The mocks are a page (`home` plus seven pushed
//  sub-pages) crossed with an overlay (nine sheets), and everything inside both
//  is assembled from `Components/Settings`. Which page and which overlay a
//  state shows is DATA — the fixture layer says so — so the gallery pins a
//  state by handing over a model, and the real app moves between them by
//  tapping.
//
//  Navigation is local `@State` seeded from the model. Business state is not
//  wired: the callbacks are how the root view hooks the two behaviours that
//  already exist (signing out, and leaving for another tab).
//

import SwiftUI

struct SettingsScreen: View {
    @Environment(\.theme) private var theme
    let model: SettingsScreenModel
    let loc: Loc
    var onSelectTab: (WalletTab) -> Void = { _ in }
    var onSignOut: () -> Void = {}
    var onOpenContacts: () -> Void = {}
    /// What the add-network wizard raises (spec 050).
    ///
    /// Defaulted to no-ops so every gallery board and fixture call site is
    /// unchanged — the wizard stays a picture there, which is what keeps the
    /// screenshot sweep meaningful.
    var networkActions = SettingsNetworkActions()

    @State private var page: SettingsPage
    @State private var overlay: SettingsOverlay
    @State private var advancedOpen: Bool

    init(
        model: SettingsScreenModel,
        loc: Loc,
        onSelectTab: @escaping (WalletTab) -> Void = { _ in },
        onSignOut: @escaping () -> Void = {},
        onOpenContacts: @escaping () -> Void = {},
        networkActions: SettingsNetworkActions = SettingsNetworkActions()
    ) {
        self.model = model
        self.loc = loc
        self.onSelectTab = onSelectTab
        self.onSignOut = onSignOut
        self.onOpenContacts = onOpenContacts
        self.networkActions = networkActions
        // Seeds, not bindings: a gallery state pins where this opens, and a
        // person tapping owns it from then on.
        _page = State(initialValue: model.page)
        _overlay = State(initialValue: model.overlay)
        _advancedOpen = State(initialValue: model.state == .st1b)
    }

    var body: some View {
        // SR5 replaces the whole screen: it blocks both creating and signing
        // in, so there is nothing behind it to go back to.
        if model.state == .sr5 {
            IndexDownScreen(model: model.indexDown)
        } else {
            VStack(spacing: 0) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 0) {
                        header
                        if !model.rescue { pageBody }
                    }
                    .padding(.horizontal, Tokens.Space.s24)
                    .padding(.bottom, Tokens.Space.s32)
                }
                WalletTabBar(
                    tabs: model.tabs,
                    selected: model.rescue ? .wallet : .settings,
                    onSelect: onSelectTab
                )
            }
            .background(theme.bgBase.ignoresSafeArea())
            .sheet(item: sheetBinding) { overlay in
                SettingsSheet(model: model, overlay: overlay,
                              onDismiss: { self.overlay = .none },
                              onSignOut: onSignOut)
                    .themed(theme.scheme)
            }
        }
    }

    /// `.none` is a state, not an absence, so it is filtered out here rather
    /// than modelled as an optional everywhere else.
    private var sheetBinding: Binding<SettingsOverlay?> {
        Binding(
            get: { overlay == .none ? nil : overlay },
            set: { if $0 == nil { overlay = .none } }
        )
    }

    @ViewBuilder private var header: some View {
        if model.rescue {
            // SR2–SR4 are sheets over ANOTHER screen (the wallet, the send
            // flow), so the body behind them is a dimmed title rather than the
            // settings list. Drawing settings behind "fix Polygon's RPC" would
            // put a screen there that the person was never on.
            Text(model.backdropTitle)
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgSubtle.opacity(Tokens.Opacity.dim))
                .padding(.top, Tokens.Space.s32)
                .padding(.bottom, Tokens.Space.s16)
            if let banner = model.rpcBanner {
                RpcBannerView(banner: banner)
            }
        } else if page == .home {
            Text(model.title)
                .typeRole(Typography.display)
                .foregroundStyle(theme.fgBase)
                .padding(.top, Tokens.Space.s32)
                .padding(.bottom, Tokens.Space.s16)
        } else {
            SettingsNavHeader(
                title: pageTitle.title,
                subtitle: pageTitle.subtitle,
                backLabel: model.closeLabel,
                onBack: { page = .home }
            )
        }
    }

    private var pageTitle: (title: String, subtitle: String?) {
        switch page {
        case .networks: (model.networksTitle, model.networksSubtitle)
        case .networkDetail: (model.networkDetail.title, model.networkDetail.subtitle)
        case .addNetwork: (model.addNetwork.title, model.addNetwork.subtitle)
        case .rpcProviders: (model.rpcProviders.title, model.rpcProviders.subtitle)
        case .endpoints: (model.endpoints.title, nil)
        case .storage: (model.storage.title, model.storage.subtitle)
        case .about: (model.about.title, nil)
        case .home: (model.title, nil)
        }
    }

    @ViewBuilder private var pageBody: some View {
        switch page {
        case .home: homeBody
        case .networks: networksBody
        case .networkDetail: NetworkDetailBody(detail: model.networkDetail)
        case .addNetwork: AddNetworkBody(panel: model.addNetwork, actions: networkActions)
        case .rpcProviders: RpcProvidersBody(panel: model.rpcProviders)
        case .endpoints: EndpointsBody(panel: model.endpoints)
        case .storage: StorageBody(panel: model.storage,
                                   onClearCaches: { overlay = .clearCaches })
        case .about: AboutBody(panel: model.about)
        }
    }

    @ViewBuilder private var homeBody: some View {
        SettingsAccountRow(account: model.account) { overlay = .accounts }

        ForEach(model.sections) { section in
            if let label = section.label {
                SettingsSectionLabel(
                    label: label,
                    collapsible: section.collapsible,
                    collapsed: section.collapsible && !advancedOpen,
                    onToggle: { advancedOpen.toggle() }
                )
            }
            if !(section.collapsible && !advancedOpen) {
                ForEach(Array(section.rows.enumerated()), id: \.element.id) { index, row in
                    SettingsRow(row: row, divider: index < section.rows.count - 1, onTap: select)
                }
            }
            // The three appearance controls are not rows: they are the control
            // itself, shown inline under 语言 (ST1).
            if section.appearanceControls {
                TextScaleSlider(model: model.textScale)
                SettingsSegmentedControl(model: model.theme)
                    .padding(.bottom, Tokens.Space.s12)
                SettingsSegmentedControl(model: model.avatar)
            }
        }

        Text(model.signOutLabel)
            .typeRole(Typography.fieldLabel)
            .foregroundStyle(theme.fgMuted)
            .frame(maxWidth: .infinity)
            .padding(.top, Tokens.Space.s32)
            .padding(.bottom, Tokens.Space.s24)
            .contentShape(Rectangle())
            .onTapGesture { overlay = .signOut }

        DangerCard(title: model.eraseTitle, subtitle: model.eraseSubtitle) {
            overlay = .eraseDevice
        }
    }

    @ViewBuilder private var networksBody: some View {
        ForEach(model.networks) { row in
            SettingsNetworkRow(row: row, deleteLabel: model.addNetworkLabel) { _ in
                page = .networkDetail
            }
        }
        // A link, not a CTA: adding a network is navigation, and accent is
        // reserved for actions that move value.
        HStack(spacing: Tokens.Space.s8) {
            LucideIcon(.plus, size: LucideIconSize.action)
            Text(model.addNetworkLabel)
                .typeRole(Typography.fieldLabel)
                .fontWeight(.semibold)
        }
        .foregroundStyle(theme.infoBase)
        .frame(maxWidth: .infinity, minHeight: 44)
        .padding(.top, Tokens.Space.s24)
        .contentShape(Rectangle())
        .onTapGesture { page = .addNetwork }
    }

    /// Rows a tap navigates from; everything else opens an overlay.
    private func select(_ id: String) {
        switch id {
        case "contacts": onOpenContacts()
        case "networks": page = .networks
        case "rpc-providers": page = .rpcProviders
        case "add-network": page = .addNetwork
        case "endpoints": page = .endpoints
        case "storage": page = .storage
        case "about": page = .about
        case "language": overlay = .language
        case "currency": overlay = .currency
        case "number-format": overlay = .numberFormat
        case "date-format": overlay = .dateFormat
        case "time-format": overlay = .timeFormat
        case "feedback": overlay = .feedback
        default: break
        }
    }
}

/// Back arrow + title + optional second line (ST9/ST9b/ST10/ST11/ST12/…).
private struct SettingsNavHeader: View {
    @Environment(\.theme) private var theme
    let title: String
    let subtitle: String?
    let backLabel: String
    let onBack: () -> Void

    var body: some View {
        HStack(alignment: .top, spacing: Tokens.Space.s8) {
            LucideIcon(.chevronLeft, size: LucideIconSize.tab)
                .foregroundStyle(theme.fgBase)
                .frame(width: 44, height: 44)
                .contentShape(Rectangle())
                .onTapGesture(perform: onBack)
                .accessibilityLabel(backLabel)
            VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                Text(title)
                    .typeRole(Typography.title)
                    .foregroundStyle(theme.fgBase)
                if let subtitle {
                    Text(subtitle)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
            }
            .padding(.top, Tokens.Space.s8)
            Spacer()
        }
        .padding(.top, Tokens.Space.s16)
        .padding(.bottom, Tokens.Space.s12)
    }
}

// MARK: - Page bodies

private struct NetworkDetailBody: View {
    @Environment(\.theme) private var theme
    let detail: NetworkDetailModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s24) {
            HStack(spacing: Tokens.Space.s12) {
                ChainMark(mark: detail.mark)
                VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                    Text(detail.name)
                        .typeRole(Typography.title)
                        .foregroundStyle(theme.fgBase)
                    Text(detail.note)
                        .typeRole(Typography.flowCaption)
                        .foregroundStyle(theme.fgSubtle)
                }
                Spacer()
                StatusPill(pill: detail.badge)
            }
            SettingsUrlField(field: detail.rpc)
            if let callout = detail.callout { SettingsCallout(callout: callout) }
            SettingsUrlField(field: detail.explorer)
        }
    }
}

private struct AddNetworkBody: View {
    @Environment(\.theme) private var theme
    let panel: AddNetworkModel
    var actions = SettingsNetworkActions()

    /// The two editable fields' local text.
    ///
    /// Local, and seeded from the model, because the core is the authority on
    /// what has been COMMITTED while a half-typed URL is nobody's business but
    /// this view's. Committing on submit or on leaving the field is what hands
    /// it over.
    @State private var query = ""
    @State private var customRpc = ""

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s16) {
            if let candidate = panel.candidate {
                HStack(spacing: Tokens.Space.s12) {
                    ChainMark(mark: candidate.mark)
                    VStack(alignment: .leading, spacing: Tokens.Space.s2) {
                        Text(candidate.name)
                            .typeRole(Typography.title)
                            .foregroundStyle(theme.fgBase)
                        Text(candidate.meta)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    Spacer()
                    if let badge = candidate.badge { StatusPill(pill: badge) }
                }
                if let title = panel.checksTitle {
                    SettingsCheckList(title: title, items: panel.checks)
                }
                if let custom = panel.customRpc {
                    SettingsUrlField(
                        field: custom,
                        text: actions.isLive ? $customRpc : nil,
                        onCommit: { actions.onEditCustomRpc(customRpc) }
                    )
                }
                if let callout = panel.callout { SettingsCallout(callout: callout) }
                // An outline CTA plus a re-check link when it cannot be added:
                // an action you cannot take should not be dressed as the action
                // you came for.
                if let primary = panel.primary {
                    VelaButton(title: primary, kind: .primary) { actions.onConfirmAdd() }
                }
                if let secondary = panel.secondary {
                    VelaButton(title: secondary, kind: .secondary) {}
                }
                if let recheck = panel.recheck {
                    Text(recheck)
                        .typeRole(Typography.flowCaption)
                        .fontWeight(.semibold)
                        .foregroundStyle(theme.infoBase)
                        .frame(maxWidth: .infinity)
                        .padding(.top, Tokens.Space.s8)
                        .onTapGesture { actions.onEditCustomRpc(customRpc) }
                }
            } else {
                SettingsUrlField(
                    field: UrlFieldModel(id: "search", label: "", value: "",
                                         placeholder: panel.searchPlaceholder),
                    text: actions.isLive ? $query : nil,
                    onCommit: { actions.onSearch(query) }
                )
                // The core debounces the search itself, so every keystroke can
                // go straight to it — a shell-side timer here would be a second
                // one, racing the first.
                .onChange(of: query) { _, value in actions.onSearch(value) }
                ForEach(panel.results) { row in
                    SettingsNetworkRow(row: row)
                        .contentShape(Rectangle())
                        .onTapGesture {
                            if let chainId = row.chainId { actions.onSelectChain(chainId) }
                        }
                }
            }
        }
        .onAppear { customRpc = panel.customRpc?.value ?? "" }
    }
}

/// What the add-network wizard can raise (spec 050).
///
/// A struct of closures rather than five parameters, because the wizard's
/// controls arrive together and are wired together; `isLive` is what tells the
/// drawn fields whether there is anything on the other end. A gallery board
/// leaves it default, and every field stays the picture it was drawn as.
struct SettingsNetworkActions {
    var onSearch: (String) -> Void = { _ in }
    var onSelectChain: (Int) -> Void = { _ in }
    var onEditCustomRpc: (String) -> Void = { _ in }
    var onConfirmAdd: () -> Void = {}
    /// `false` for fixtures — the fields render as `Text`, exactly as drawn.
    var isLive = false
}

private struct RpcProvidersBody: View {
    @Environment(\.theme) private var theme
    let panel: RpcProvidersModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s32) {
            Text(panel.description)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
            ForEach(panel.providers) { provider in
                VStack(alignment: .leading, spacing: Tokens.Space.s12) {
                    HStack {
                        Text(provider.name)
                            .typeRole(Typography.title)
                            .foregroundStyle(theme.fgBase)
                        Spacer()
                        StatusPill(pill: provider.badge)
                    }
                    SettingsUrlField(field: provider.field)
                    if let support = provider.support {
                        Text(support)
                            .typeRole(Typography.label)
                            .foregroundStyle(theme.fgSubtle)
                    }
                    if let link = provider.link {
                        Text(link)
                            .typeRole(Typography.label)
                            .foregroundStyle(theme.infoBase)
                    }
                }
            }
        }
    }
}

private struct EndpointsBody: View {
    @Environment(\.theme) private var theme
    let panel: EndpointsModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s24) {
            Text(panel.description)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgMuted)
            ForEach(panel.fields) { SettingsUrlField(field: $0) }
            HStack(spacing: Tokens.Space.s8) {
                LucideIcon(.refreshCw, size: LucideIconSize.rowGlyph)
                Text(panel.reset).typeRole(Typography.flowCaption)
            }
            .foregroundStyle(theme.fgMuted)
            .frame(maxWidth: .infinity)
            .padding(.top, Tokens.Space.s16)
        }
    }
}

private struct StorageBody: View {
    @Environment(\.theme) private var theme
    let panel: StorageModel
    let onClearCaches: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            HStack(alignment: .lastTextBaseline, spacing: Tokens.Space.s8) {
                Text(panel.amount)
                    .typeRole(Typography.display)
                    .foregroundStyle(theme.fgBase)
                Text(panel.unit)
                    .typeRole(Typography.fieldLabel)
                    .fontWeight(.semibold)
                    .foregroundStyle(theme.fgBase)
                Text(panel.summary)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
            }
            .padding(.bottom, Tokens.Space.s16)
            StorageBar(segments: panel.segments)
            ForEach(panel.groups) { group in
                StorageGroupView(group: group, onGroupAction: onClearCaches)
            }
        }
    }
}

private struct AboutBody: View {
    @Environment(\.theme) private var theme
    let panel: AboutModel

    var body: some View {
        VStack(alignment: .leading, spacing: 0) {
            VStack(spacing: Tokens.Space.s4) {
                VelaMark(size: 40)
                Text(panel.tagline)
                    .typeRole(Typography.fieldLabel)
                    .foregroundStyle(theme.fgMuted)
                Text(panel.version)
                    .typeRole(Typography.monoSmall)
                    .foregroundStyle(theme.fgSubtle)
            }
            .frame(maxWidth: .infinity)
            .padding(.vertical, Tokens.Space.s24)

            Text(panel.sectionTechnical)
                .typeRole(Typography.flowCaption)
                .foregroundStyle(theme.fgSubtle)
                .padding(.bottom, Tokens.Space.s8)
            ForEach(panel.rows) { KeyValueRow(row: $0) }

            Spacer().frame(height: Tokens.Space.s24)
            ForEach(panel.links) { KeyValueRow(row: $0) }

            Text(panel.footer)
                .typeRole(Typography.label)
                .foregroundStyle(theme.fgSubtle)
                .frame(maxWidth: .infinity)
                .padding(.top, Tokens.Space.s24)
        }
    }
}

/// SR1's amber banner: the count of unreachable networks, then one chip per
/// network with its own 修复. Per-chain rather than one global button, because
/// the fix IS per chain — a shared button would have to ask which one first.
private struct RpcBannerView: View {
    @Environment(\.theme) private var theme
    let banner: RpcBannerModel

    var body: some View {
        VStack(alignment: .leading, spacing: Tokens.Space.s12) {
            HStack(spacing: Tokens.Space.s8) {
                LucideIcon(.triangleAlert, size: LucideIconSize.action)
                Text(banner.text)
                    .typeRole(Typography.flowCaption)
                    .fontWeight(.semibold)
            }
            .foregroundStyle(theme.warningBase)
            HStack(spacing: Tokens.Space.s8) {
                ForEach(banner.chips) { chip in
                    HStack(spacing: Tokens.Space.s8) {
                        ChainMark(mark: chip.mark, size: 20)
                        Text(chip.name)
                            .typeRole(Typography.flowCaption)
                            .foregroundStyle(theme.fgBase)
                        // The only accent on this banner: what fixes it.
                        Text(chip.action)
                            .typeRole(Typography.flowCaption)
                            .fontWeight(.semibold)
                            .foregroundStyle(theme.accentBase)
                    }
                    .padding(Tokens.Space.s8)
                    .background(theme.bgBase, in: Capsule())
                }
            }
        }
        .padding(Tokens.Space.s16)
        .background(theme.warningSoft, in: RoundedRectangle(cornerRadius: Tokens.Radius.r12))
        .padding(.vertical, Tokens.Space.s12)
    }
}

/// SR5 — the passkey index is unreachable. The endpoint is editable right here,
/// because "the service is down" and "you pointed it at the wrong host" look
/// identical from the inside, and only one is something the person can fix.
private struct IndexDownScreen: View {
    @Environment(\.theme) private var theme
    let model: IndexDownModel

    var body: some View {
        ScrollView {
            VStack(spacing: Tokens.Space.s16) {
                VelaMark(size: 40).padding(.bottom, Tokens.Space.s8)
                Text(model.title)
                    .typeRole(Typography.display)
                    .foregroundStyle(theme.fgBase)
                    .multilineTextAlignment(.center)
                Text(model.subtitle)
                    .typeRole(Typography.flowCaption)
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
                SettingsCallout(callout: model.callout)
                SettingsUrlField(field: model.field)
                VelaButton(title: model.primary, kind: .primary) {}
                VelaButton(title: model.secondary, kind: .secondary) {}
                Text(model.footer)
                    .typeRole(Typography.label)
                    .foregroundStyle(theme.fgSubtle)
                    .multilineTextAlignment(.center)
            }
            .padding(.horizontal, Tokens.Space.s24)
            .padding(.vertical, Tokens.Space.s48)
        }
        .background(theme.bgBase.ignoresSafeArea())
    }
}
