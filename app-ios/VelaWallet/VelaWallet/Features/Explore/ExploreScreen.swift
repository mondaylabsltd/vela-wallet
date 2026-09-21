//
//  ExploreScreen.swift
//  VelaWallet
//
//  The Explore tab (spec 022 FR-002): one surface with three views — the
//  start page, a page being browsed, and the tab switcher — assembled from
//  the component vocabulary. Screens compose components, never re-implement
//  them (the spec-015 rule).
//
//  Every E-state renders from fixtures alone; what a person DOES here is
//  local view state layered over the model, so swapping the model (a locale
//  change, the gallery's state picker) still lands.
//

import SwiftUI

struct ExploreScreen: View {
    @Environment(\.theme) private var theme
    @Environment(\.walletTextScale) private var textScale

    let model: ExploreHomeModel
    let loc: Loc
    /// A signing request the page can raise, in the gallery. Fixture-driven.
    var signing: SigningModel?
    /// A **live** signing request. Present means a page is asking for a
    /// signature right now, and the sheet is up.
    var signingLive: SigningModel?
    var onSigningConfirm: () -> Void = {}
    /// The spending-cap chips and the person's own figure. The core decides
    /// what each means; the sheet only reports the tap.
    var onAllowanceChip: (String) -> Void = { _ in }
    var onAllowanceAmount: (String) -> Void = { _ in }
    var onSignWith: (String?) -> Void = { _ in }
    /// The live sheet's speed control (spec 069): `nil` folds, an id picks.
    var onSpeed: (String?) -> Void = { _ in }
    /// The sheet went away without a tap. The core routes what that means by
    /// phase — a refusal before the commitment, a dismissal after it — so the
    /// shell reports the gesture and decides nothing.
    var onSigningDismissed: () -> Void = {}
    /// The live browser. `nil` is the gallery: every E-state still renders
    /// from fixtures, and a gallery that ran somebody else's JavaScript would
    /// not be a gallery.
    var controller: BrowserController?
    var onSelectTab: (WalletTab) -> Void = { _ in }

    @State private var viewOverride: ExploreView?
    /// **Which** sheet is open — never a snapshot of what it said when it
    /// opened. A sheet holding a captured model shows the group you deleted,
    /// the site you unpinned, and — the one that matters — a CONNECTED panel
    /// for a site that is still asking. Android found the same bug on its
    /// group sheet; this one was device-found here.
    @State private var sheet: ExploreSheetKind?
    @State private var signingUp = false
    /// Groups hidden here rather than in the fixture: hiding is something a
    /// person does, and the sheet has to show it happening.
    @State private var hidden: Set<String> = []
    /// Naming a new group. The drawn sheet has a 新建分组 row and **no field**
    /// to type into, so the name is asked for with the platform's own prompt
    /// — the same call the document picker and the share sheet make elsewhere.
    /// Recorded as a deviation from the drawing.
    @State private var namingGroup = false
    @State private var groupName = ""
    /// The open sheet is a site ASKING to connect, not a review of one that
    /// already is. Kept so a swipe can be read as the refusal it is.
    @State private var consentOpen = false

    /// Which of the three views is on screen.
    ///
    /// A person's own choice wins. Otherwise, **when the browser is live the
    /// tabs decide**: a tab with a page showing means the browsing view, and
    /// no such tab means the start page. Before this the view came from the
    /// fixture, so a page could load, run, and be invisible — the browser was
    /// working and the screen was still drawing the start page over it.
    private var view: ExploreView {
        if let viewOverride { return viewOverride }
        if controller != nil { return engine != nil ? .browsing : .start }
        return model.view
    }

    /// The engine in front of the person, if the browser is live and a tab
    /// with a page is selected.
    private var engine: BrowserEngine? { controller?.current }

    /// Chrome reads the engine, when there is one, and the fixture otherwise.
    /// Never a blend: a live address bar over a drawn page would be a lie
    /// about what is on screen.
    private var browserHost: String { engine?.host ?? model.browser.host }
    private var browserSecure: Bool { engine?.secure ?? model.browser.secure }

    /// The chrome's model, with the engine's facts substituted where it has
    /// them. `engineTick` is read so SwiftUI redraws when navigation state
    /// changes — a `WKWebView`'s properties are not observable.
    private var liveBrowser: BrowserModel {
        guard let engine, let controller else { return model.browser }
        _ = controller.engineTick
        var live = model.browser
        live = BrowserModel(
            url: engine.url,
            host: engine.host,
            secure: engine.secure,
            connected: controller.permissions.isConnected,
            canBack: engine.canGoBack,
            canForward: engine.canGoForward,
            bookmarked: controller.explore.favorites.contains { $0.origin == engine.origin },
            account: live.account,
            tabCount: controller.explore.tabs.count,
            page: live.page
        )
        return live
    }

    /// A tile or a row was tapped.
    private func open(siteId: String) {
        guard let controller else {
            viewOverride = .browsing
            return
        }
        controller.openSite(id: siteId)
        viewOverride = nil
    }

    /// The ⋯ sheet's seven items.
    private func pick(menuItem id: String, site: SiteModel) {
        guard let controller else {
            if id == "close" { viewOverride = .start }
            return
        }
        switch id {
        case "refresh":
            controller.reload()
        case "copy":
            UIPasteboard.general.string = controller.current?.url ?? site.host
        case "favorite":
            guard let engine = controller.current, !engine.url.isEmpty else { return }
            if controller.explore.favorites.contains(where: { $0.origin == engine.origin }) {
                controller.removeFavorite(origin: engine.origin)
            } else {
                controller.addFavorite(url: engine.url, title: engine.title)
            }
        case "system":
            if let url = URL(string: controller.current?.url ?? "") { UIApplication.shared.open(url) }
        case "disconnect":
            controller.revoke()
        case "close":
            if let tab = controller.explore.selectedTab { controller.closeTab(tab) }
            viewOverride = .start
        default:
            // 分享 is 056's: the share sheet is a platform port that cut
            // builds, and a menu row that silently did nothing would be worse
            // than one that is honestly not here yet.
            break
        }
    }

    private var visibleGroups: [GroupModel] {
        model.groups.filter { !hidden.contains($0.id) }
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            switch view {
            case .tabs:
                ExploreTabsScreen(
                    tabs: model.tabs, copy: model.tabsScreen,
                    onDone: { viewOverride = nil },
                    onOpen: { id in
                        controller?.selectTab(id)
                        viewOverride = controller == nil ? .browsing : nil
                    },
                    onClose: { id in
                        controller?.closeTab(id)
                        // With a live browser the strip decides what is left;
                        // the core never leaves it empty or unselected.
                        viewOverride = controller == nil ? .start : nil
                    },
                    onNew: {
                        controller?.newTab()
                        viewOverride = .start
                    },
                    onCloseAll: {
                        controller?.closeAllTabs()
                        viewOverride = .start
                    }
                )
            case .browsing:
                AddressBarView(
                    host: browserHost, secure: browserSecure,
                    secureLabel: loc.t("explore.secureSite"),
                    closeLabel: loc.t("explore.closePage"),
                    menuLabel: loc.t("explore.siteMenu"),
                    // The page keeps running — leaving it is not closing it
                    // (FR-013). The tab is still there, and the switcher
                    // brings it straight back.
                    onClose: { viewOverride = .start },
                    onMenu: { sheet = .siteMenu }
                )
                if let engine {
                    ZStack {
                        BrowserWebView(engine: engine)
                            .accessibilityIdentifier("explore.page")
                        // A page that could not be reached SAYS SO. Before
                        // 058 both failure callbacks set `loading = false` and
                        // nothing else, so an unreachable dApp was a white
                        // rectangle under an empty address bar — silence a
                        // person can only read as "the app is broken".
                        if let failure = engine.failure {
                            BrowserFailureView(
                                title: loc.t("connect.browser.loadFailed"),
                                detail: failure,
                                retry: loc.t("connect.browser.retry"),
                                onRetry: { engine.reload() }
                            )
                        }
                    }
                } else {
                    ScrollView {
                        DemoPageView(page: model.browser.page) {
                            if signing != nil { signingUp = true }
                        }
                    }
                }
                BrowserToolbarView(
                    browser: liveBrowser,
                    backLabel: loc.t("explore.back"),
                    forwardLabel: loc.t("explore.forward"),
                    accountLabel: loc.t("explore.account"),
                    connectedLabel: loc.t("explore.connectedTag"),
                    bookmarkLabel: loc.t("explore.addToFavorites"),
                    tabsLabel: loc.t("explore.tabs"),
                    onBack: { controller?.goBack() },
                    onForward: { controller?.goForward() },
                    onAccount: { sheet = .connection },
                    onBookmark: {
                        guard let engine, !engine.url.isEmpty else { return }
                        controller?.addFavorite(url: engine.url, title: engine.title)
                    },
                    onTabs: { viewOverride = .tabs }
                )
            case .start:
                startPage
                WalletTabBar(tabs: model.nav, selected: .explore, onSelect: onSelectTab)
            }
        }
        .background(theme.bgBase.ignoresSafeArea())
        .environment(\.walletTextScale, 1)
        .sheet(item: $sheet, onDismiss: {
            // Swiping a consent sheet away is a person declining. The core
            // has a different, wider case — a window that closes with no
            // answer at all settles 4900 — and it reaches that through
            // `browser_closed` / `navigation_started`, never through here.
            if consentOpen {
                consentOpen = false
                controller?.consentRejected()
            }
        }) { sheet in
            sheetContent(sheet)
                .presentationDragIndicator(.visible)
                .presentationDetents([.medium, .large])
                .presentationCornerRadius(Tokens.Radius.r20)
                .presentationBackground(theme.bgBase)
        }
        .sheet(isPresented: $signingUp) {
            if let signing {
                SigningSheet(model: signing, onConfirm: { signingUp = false })
                    .presentationDragIndicator(.visible)
                    .presentationDetents([.large])
                    .presentationCornerRadius(Tokens.Radius.r20)
                    .presentationBackground(theme.bgRaised)
            }
        }
        .sheet(isPresented: Binding(
            get: { signingLive != nil },
            set: { open in if !open { onSigningDismissed() } }
        )) {
            if let signingLive {
                SigningSheet(
                    model: signingLive,
                    onConfirm: onSigningConfirm,
                    onAllowanceChip: onAllowanceChip,
                    onAllowanceAmount: onAllowanceAmount,
                    onSignWith: onSignWith,
                    onSpeed: onSpeed
                )
                    .presentationDragIndicator(.visible)
                    .presentationDetents([.large])
                    .presentationCornerRadius(Tokens.Radius.r20)
                    .presentationBackground(theme.bgRaised)
            }
        }
        .alert(loc.t("explore.newGroup"), isPresented: $namingGroup) {
            TextField(loc.t("explore.newGroup"), text: $groupName)
            Button(loc.t("explore.close"), role: .cancel) { groupName = "" }
            Button(loc.t("explore.done")) {
                let name = groupName.trimmingCharacters(in: .whitespacesAndNewlines)
                groupName = ""
                guard !name.isEmpty else { return }
                controller?.createGroup(name: name)
            }
        }
        // A site asked. The sheet opens itself, because a request that waited
        // for somebody to find a menu would be a request the page thinks is
        // hanging.
        .onChange(of: controller?.permissions.consent?.origin) { _, asking in
            guard asking != nil else {
                if consentOpen { consentOpen = false; sheet = nil }
                return
            }
            consentOpen = true
            sheet = .connection
        }
        .onAppear { sheet = model.sheet?.kind }
    }

    private var startPage: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: Tokens.Space.s0) {
                HStack {
                    Text(verbatim: model.title)
                        .typeRole(Typography.display.scaled(textScale))
                        .foregroundStyle(theme.fgBase)
                    Spacer()
                    if let count = model.tabCountLabel {
                        Button {
                            viewOverride = .tabs
                        } label: {
                            Text(verbatim: count)
                                .typeRole(Typography.label.scaled(textScale))
                                .foregroundStyle(theme.fgBase)
                                .frame(minWidth: ExploreGeometry.tabCount,
                                       minHeight: ExploreGeometry.tabCount)
                                .overlay(
                                    RoundedRectangle(cornerRadius: Tokens.Radius.r4)
                                        .stroke(theme.fgBase,
                                                lineWidth: Tokens.BorderWidth.emphasis)
                                )
                        }
                        .buttonStyle(.plain)
                        .accessibilityLabel(model.tabsScreen.title)
                    }
                }
                .padding(.top, Tokens.Space.s20)
                .padding(.bottom, Tokens.Space.s16)

                ExploreSearchField(
                    placeholder: model.searchPlaceholder, scanLabel: model.scanLabel,
                    onSubmit: { text in
                        controller?.open(text)
                        viewOverride = .browsing
                    }
                )
                .padding(.bottom, Tokens.Space.s20)

                if let empty = model.empty {
                    ExploreEmptyView(title: empty.title, caption: empty.caption, cta: empty.cta) {
                        // Nothing to open yet. The CTA puts the cursor where
                        // a person can say where to go, rather than opening a
                        // page nobody asked for.
                        if controller == nil { viewOverride = .browsing }
                    }
                }

                if let favorites = model.favorites {
                    WalletSectionHeader(
                        title: favorites.title, action: favorites.action,
                        onAction: { sheet = .groupManage }
                    )
                    LazyVGrid(
                        columns: Array(repeating: GridItem(.flexible(), spacing: Tokens.Space.s8),
                                       count: ExploreGeometry.tileColumns),
                        spacing: Tokens.Space.s20
                    ) {
                        ForEach(favorites.tiles) { tile in
                            SiteTileView(tile: tile) { id in open(siteId: id) }
                        }
                    }
                    .padding(.vertical, Tokens.Space.s12)
                }

                ForEach(visibleGroups) { group in
                    WalletSectionHeader(
                        title: group.title,
                        action: group.action == .clear ? loc.t("explore.clear") : "⋯",
                        onAction: {
                            if group.action == .clear {
                                controller?.clearRecent()
                            } else {
                                sheet = .groupManage
                            }
                        }
                    )
                    ForEach(group.sites) { site in
                        SiteRowView(site: site) { id in open(siteId: id) }
                    }
                }
            }
            .padding(.horizontal, Tokens.Layout.screenPaddingX)
            .padding(.bottom, Tokens.Space.s24)
        }
    }

    @ViewBuilder
    private func sheetContent(_ kind: ExploreSheetKind) -> some View {
        // Read the CURRENT model, every render. See `sheet`'s own comment.
        switch kind.resolved(in: model) {
        case .groupManage(let title, let rows, let newGroup):
            ScrollView {
                GroupManageSheetView(
                    title: title,
                    rows: rows.map { row in
                        var copy = row
                        copy.hidden = hidden.contains(row.id) || row.hidden
                        return copy
                    },
                    newGroup: newGroup,
                    closeLabel: loc.t("explore.close"),
                    hideLabel: loc.t("explore.hide"),
                    showLabel: loc.t("explore.show"),
                    deleteLabel: loc.t("explore.delete"),
                    onClose: { self.sheet = nil },
                    onToggle: { id in
                        guard let controller else {
                            if hidden.contains(id) { hidden.remove(id) } else { hidden.insert(id) }
                            return
                        }
                        let isHidden = rows.first { $0.id == id }?.hidden ?? false
                        switch id {
                        case "favorites", "recent":
                            controller.setSystemGroupHidden(id, hidden: !isHidden)
                        default:
                            controller.setGroupHidden(id: id, hidden: !isHidden)
                        }
                    },
                    onDelete: { id in controller?.deleteGroup(id: id) },
                    onNew: { namingGroup = true }
                )
            }
        case .siteMenu(let site, let statusLine, let items):
            ScrollView {
                SiteMenuSheetView(
                    site: site, statusLine: statusLine, items: items,
                    closeLabel: loc.t("explore.close"),
                    onClose: { self.sheet = nil },
                    onPick: { id in
                        self.sheet = nil
                        pick(menuItem: id, site: site)
                    }
                )
            }
        case .connection(let connection):
            ScrollView {
                ConnectionPanelView(
                    connection: connection, closeLabel: loc.t("explore.close"),
                    onClose: { self.sheet = nil },
                    onDisconnect: {
                        self.sheet = nil
                        controller?.revoke()
                    },
                    onApprove: {
                        consentOpen = false
                        self.sheet = nil
                        controller?.consentApproved()
                    },
                    onReject: {
                        consentOpen = false
                        self.sheet = nil
                        controller?.consentRejected()
                    }
                )
            }
        }
    }
}
