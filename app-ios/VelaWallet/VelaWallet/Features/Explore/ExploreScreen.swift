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

import AVFoundation
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
    /// One batch leg's chip / field — the leg index travels with them.
    var onAllowanceLegChip: (Int, String) -> Void = { _, _ in }
    var onAllowanceLegAmount: (Int, String) -> Void = { _, _ in }
    var onSignWith: (String?) -> Void = { _ in }
    /// Issue #262: the fee row's tap and a coin picked from its list.
    var onFee: () -> Void = {}
    var onFeePick: (String) -> Void = { _ in }
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
    /// The camera behind the start page's scan button (issue 273) — the same
    /// app-resident one 发送's scanner uses. `nil` in the gallery, where the
    /// scanner is a picture of a frame.
    var camera: CameraScanner?
    var onSelectTab: (WalletTab) -> Void = { _ in }
    /// A scanned account address or `ethereum:` code — 发送's, through the
    /// core's `scan_resolved` (spec 070 US5).
    var onScanPayment: (String) -> Void = { _ in }
    /// The connection panel's "Switch account": the wallet's one switcher,
    /// presented by the container once the panel is out of the way.
    ///
    /// What it MEANS is the reason it took until 2026-09-23: a grant is pinned
    /// to the address it was given to, deliberately, so that switching the
    /// wallet's account cannot silently hand a site a different identity. An
    /// explicit switch is not silent, and the core already does the right
    /// thing with it — `AccountSwitched` re-pins the grant to the new address,
    /// writes it, and emits `accountsChanged` to the page.
    var onSwitchAccount: () -> Void = {}
    /// Whether that switcher is up. Watched so a site still ASKING gets its
    /// sheet back when the switcher closes.
    var accountSwitcherOpen = false

    @State private var viewOverride: ExploreView?
    /// **Which** sheet is open — never a snapshot of what it said when it
    /// opened. A sheet holding a captured model shows the group you deleted,
    /// the site you unpinned, and — the one that matters — a CONNECTED panel
    /// for a site that is still asking. Android found the same bug on its
    /// group sheet; this one was device-found here.
    @State private var sheet: ExploreSheetKind?
    /// The switcher is opened AFTER this sheet is really gone. Presenting one
    /// sheet in the same breath as dismissing another is how iOS ends up
    /// showing neither.
    @State private var switchAfterDismiss = false
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
    /// The panel is closing to make way for the account switcher — which is
    /// not the person declining a site that is asking.
    @State private var switchingAccount = false
    /// The scanner is up, over the whole screen, as it is in 发送.
    @State private var scanning = false
    /// What the last scanned code turned out not to be, said under the frame.
    @State private var scanRefusal: String?
    /// A picked photo had no code in it.
    @State private var photoEmpty = false
    /// A WalletConnect code was scanned: said in an alert, with the way that
    /// does work.
    @State private var walletConnectRefused = false
    /// Bumped to put the cursor in the start page's field.
    @State private var searchFocus = 0
    /// A one-line confirmation over the page ("Link copied").
    @State private var toast: String?

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

    /// The core's facts about the tab in front (spec 070): its origin, the
    /// account it sees, its chain, whether it is secure, whether it crashed.
    private var currentTab: DbrTabViewWire? {
        guard engine != nil else { return nil }
        return controller?.currentTab
    }

    /// Chrome reads the engine, when there is one, and the fixture otherwise.
    /// Never a blend: a live address bar over a drawn page would be a lie
    /// about what is on screen.
    private var browserHost: String { engine?.host ?? model.browser.host }
    /// The lock tells the truth: the core's judgement of the tab's origin.
    private var browserSecure: Bool {
        engine == nil ? model.browser.secure : (currentTab?.secure ?? false)
    }

    /// The chrome's model, with the engine's facts substituted where it has
    /// them. `engineTick` is read so SwiftUI redraws when navigation state
    /// changes — a `WKWebView`'s properties are not observable.
    private var liveBrowser: BrowserModel {
        guard let engine, let controller else { return model.browser }
        _ = controller.engineTick
        let live = model.browser
        return BrowserModel(
            url: engine.url,
            host: engine.host,
            secure: browserSecure,
            connected: currentTab?.connectedAddress != nil,
            canBack: engine.canGoBack,
            canForward: engine.canGoForward,
            bookmarked: controller.explore.favorites.contains { $0.origin == engine.origin },
            account: live.account,
            tabCount: controller.explore.tabs.count,
            page: live.page
        )
    }

    /// The load's progress, while there is one.
    private var loadProgress: Double? {
        guard let engine, let controller else { return nil }
        _ = controller.engineTick
        return engine.loading ? engine.progress : nil
    }

    /// A tile or a row was tapped.
    private func open(siteId: String) {
        guard let controller else {
            viewOverride = .browsing
            return
        }
        // The "+" tile adds by browsing: put the cursor where a site is typed,
        // then the star pins it. Opening its id would load `https://add`.
        if siteId == TileModel.addId {
            searchFocus += 1
            return
        }
        controller.openSite(id: siteId)
        viewOverride = nil
    }

    /// The ⋯ sheet's items.
    private func pick(menuItem id: String, site: SiteModel) {
        guard let controller else {
            if id == "close" { viewOverride = .start }
            return
        }
        let url = controller.current?.url ?? ""
        switch id {
        case "refresh":
            controller.reload()
        case "share":
            guard let target = URL(string: url), !url.isEmpty else { return }
            share(target)
        case "copy":
            guard !url.isEmpty else { return }
            velaCopy(url)
            show(toast: loc.t("explore.linkCopied"))
        case "favorite":
            controller.toggleFavorite()
        case "system":
            if let target = URL(string: url), !url.isEmpty { UIApplication.shared.open(target) }
        case "disconnect":
            if let origin = currentTab?.origin { controller.revoke(origin: origin) }
        case "close":
            if let tab = controller.explore.selectedTab { controller.closeTab(tab) }
            viewOverride = .start
        default:
            break
        }
    }

    /// The system share sheet, from the window's topmost controller — the
    /// sheet this menu lives in has just gone.
    private func share(_ url: URL) {
        Task { @MainActor in
            // Let the menu's own dismissal finish first: presenting over a
            // sheet that is leaving is refused by UIKit.
            try? await Task.sleep(for: .milliseconds(350))
            guard let presenter = UIApplication.shared.connectedScenes
                .compactMap({ $0 as? UIWindowScene })
                .first(where: { $0.activationState == .foregroundActive })?
                .windows.first(where: \.isKeyWindow)?
                .rootViewController?.topmost
            else { return }
            presenter.present(
                UIActivityViewController(activityItems: [url], applicationActivities: nil),
                animated: true
            )
        }
    }

    private func show(toast text: String) {
        toast = text
        Task { @MainActor in
            try? await Task.sleep(for: .seconds(2))
            if toast == text { toast = nil }
        }
    }

    private var visibleGroups: [GroupModel] {
        model.groups.filter { !hidden.contains($0.id) }
    }

    var body: some View {
        VStack(spacing: Tokens.Space.s0) {
            if scanning {
                scanSurface
            } else {
                content
            }
        }
        .background(theme.bgBase.ignoresSafeArea())
        .overlay(alignment: .bottom) { toastView }
        .environment(\.walletTextScale, 1)
        .sheet(item: $sheet, onDismiss: {
            // Making way for the account switcher is not an answer.
            if switchingAccount {
                switchingAccount = false
                onSwitchAccount()
                return
            }
            // Swiping a consent sheet away is a person declining. A page
            // that went away with the sheet up is the core's case, not this
            // one: it settles that page's requests 4900 on its own.
            if consentOpen {
                consentOpen = false
                controller?.consentRejected()
            }
            if switchAfterDismiss {
                switchAfterDismiss = false
                onSwitchAccount()
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
                    onAllowanceLegChip: onAllowanceLegChip,
                    onAllowanceLegAmount: onAllowanceLegAmount,
                    onSignWith: onSignWith,
                    onFee: onFee,
                    onFeePick: onFeePick,
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
        .alert(loc.t("explore.scan"), isPresented: $walletConnectRefused) {
            Button(loc.t("explore.close"), role: .cancel) {}
        } message: {
            Text(verbatim: loc.t("explore.walletConnectUnsupported"))
        }
        // A site asked. The sheet opens itself, because a request that waited
        // for somebody to find a menu would be a request the page thinks is
        // hanging.
        .onChange(of: controller?.dbr.consent?.origin) { _, asking in
            guard asking != nil else {
                if consentOpen { consentOpen = false; sheet = nil }
                return
            }
            presentConsent()
        }
        // The switcher closed and the site is still asking: its question is
        // put back, with the account now chosen.
        .onChange(of: accountSwitcherOpen) { _, open in
            if !open, controller?.dbr.consent != nil { presentConsent() }
        }
        .onAppear {
            sheet = model.sheet?.kind
            if controller?.dbr.consent != nil { presentConsent() }
        }
        // The viewfinder runs only while it is on screen, as in 发送. A camera
        // left running behind the start page is a light nobody asked for.
        .task(id: scanning) {
            guard let camera else { return }
            if scanning {
                camera.onScan = { text in scanned(text) }
                await camera.start()
            } else {
                camera.stop()
            }
        }
        .onDisappear {
            if scanning { camera?.stop() }
            scanning = false
        }
    }

    /// The consent sheet, over the page that asked.
    private func presentConsent() {
        guard let controller, let consent = controller.dbr.consent, !accountSwitcherOpen else { return }
        // A background tab asked: bring it to the front, so the page behind
        // the sheet is the one the sheet names.
        if controller.explore.tabs.contains(where: { $0.id == consent.tab }) {
            controller.selectTab(consent.tab)
            viewOverride = nil
        }
        consentOpen = true
        sheet = .connection
    }

    @ViewBuilder private var toastView: some View {
        if let toast {
            Text(verbatim: toast)
                .typeRole(Typography.label)
                .foregroundStyle(theme.bgBase)
                .padding(.horizontal, Tokens.Space.s16)
                .padding(.vertical, Tokens.Space.s8)
                .background(theme.fgBase, in: Capsule())
                .padding(.bottom, ExploreGeometry.browserBar + Tokens.Space.s16)
                .transition(.opacity)
                .accessibilityIdentifier("explore.toast")
        }
    }

    /// The three views. Split from `body` so the scanner can stand in for all
    /// of them.
    @ViewBuilder
    private var content: some View {
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
                insecureLabel: loc.t("connect.browser.a11yInsecure"),
                url: engine?.url ?? "",
                addressLabel: loc.t("explore.addressBar"),
                progress: loadProgress,
                // The page keeps running — leaving it is not closing it
                // (FR-013). The tab is still there, and the switcher
                // brings it straight back.
                onClose: { viewOverride = .start },
                onMenu: { sheet = .siteMenu },
                onSubmit: controller == nil ? nil : { text in controller?.open(text) }
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
                    if currentTab?.crashed == true {
                        BrowserCrashedView(
                            title: loc.t("explore.pageCrashedTitle"),
                            detail: loc.t("explore.pageCrashedBody"),
                            reload: loc.t("explore.reload"),
                            onReload: { engine.reload() }
                        )
                    } else if let failure = engine.failure {
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
                removeBookmarkLabel: loc.t("explore.removeFromFavorites"),
                onBack: { controller?.goBack() },
                onForward: { controller?.goForward() },
                onAccount: { sheet = .connection },
                onBookmark: { controller?.toggleFavorite() },
                onTabs: { viewOverride = .tabs }
            )
        case .start:
            startPage
            WalletTabBar(tabs: model.nav, selected: .explore, onSelect: onSelectTab)
        }
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
                    },
                    onScan: {
                        scanRefusal = nil
                        photoEmpty = false
                        scanning = true
                    },
                    focusRequest: searchFocus
                )
                .padding(.bottom, Tokens.Space.s20)

                if let empty = model.empty {
                    ExploreEmptyView(title: empty.title, caption: empty.caption, cta: empty.cta) {
                        // Nothing to open yet. The CTA puts the cursor where
                        // a person can say where to go, rather than opening a
                        // page nobody asked for.
                        if controller == nil { viewOverride = .browsing } else { searchFocus += 1 }
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

    // MARK: - The scanner (issue 273, spec 070 US5)

    /// 发送's scanner surface, over the whole screen. Only the meaning of a
    /// read differs: a web address opens here, a payment code goes to 发送,
    /// and the rest is said for what it is.
    private var scanSurface: some View {
        ScanSurfaceView(
            model: WalletFlowFixtures.scan(loc),
            onClose: { scanning = false },
            onTool: { tool in scanTool(tool) },
            session: liveSession,
            refusalText: scanRefusalText,
            refusalAction: grantAction,
            torchOn: camera?.torchOn ?? false
        )
    }

    /// Handed over only while there are frames to show: a preview layer on a
    /// session with none is a black square where the drawn frame belongs.
    private var liveSession: AVCaptureSession? {
        guard let camera, camera.refusal == nil, camera.running else { return nil }
        return camera.session
    }

    /// 授予权限, when a refused permission is the thing standing in the way.
    private var grantAction: (label: String, act: () -> Void)? {
        guard camera?.refusal == .denied else { return nil }
        return (label: loc.t("componentsUi.scanner.grantPermission"), act: { openSettings() })
    }

    /// What is said under the frame. A code that WAS read and cannot be used
    /// is not a camera failure, so it outranks the camera's own refusals.
    private var scanRefusalText: String? {
        if let scanRefusal { return scanRefusal }
        if photoEmpty { return loc.t("componentsUi.scanner.noQrFoundMsg") }
        return camera?.refusal?.text(loc)
    }

    /// A code decoded, from the camera or a photo.
    ///
    /// An unusable code is said under the frame and the viewfinder comes back
    /// — a poster with the wrong code in frame must not end the scan, and
    /// re-arming at once would read it forever.
    private func scanned(_ text: String) {
        switch ExploreScan.route(text) {
        case .open(let url):
            scanning = false
            controller?.open(url)
            viewOverride = .browsing
        case .send(let payment):
            scanning = false
            onScanPayment(payment)
        case .walletConnect:
            scanning = false
            walletConnectRefused = true
        case .unrecognized:
            scanRefusal = loc.t("explore.scanUnrecognized")
            Task {
                try? await Task.sleep(for: .seconds(2))
                if scanning { await camera?.start() }
            }
        }
    }

    private func scanTool(_ tool: ScanTool) {
        switch tool {
        case .torch: camera?.toggleTorch()
        case .flip: camera?.flip()
        case .gallery:
            Task {
                guard let image = await PhotoPicker.pick() else { return }
                guard let payload = QrDecoder.decode(image: image) else {
                    scanRefusal = nil
                    photoEmpty = true
                    return
                }
                photoEmpty = false
                camera?.stop()
                scanned(payload)
            }
        }
    }

    private func openSettings() {
        guard let url = URL(string: UIApplication.openSettingsURLString) else { return }
        UIApplication.shared.open(url)
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
                    secure: controller == nil || browserSecure,
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
                    onSwitch: {
                        guard controller != nil else { return }
                        switchingAccount = true
                        self.sheet = nil
                    },
                    onDisconnect: {
                        self.sheet = nil
                        controller?.revoke(origin: connection.origin)
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
                    },
                    onPickNetwork: { chainId in
                        controller?.pickSiteChain(origin: connection.origin, chainId: chainId)
                    }
                )
            }
        }
    }
}
