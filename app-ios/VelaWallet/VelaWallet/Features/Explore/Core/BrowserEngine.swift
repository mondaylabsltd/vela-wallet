//
//  BrowserEngine.swift
//  VelaWallet
//
//  One tab: one `WKWebView`, its navigation state, and the provider inside it.
//
//  The engine is owned by `BrowserController`, never by a view. A
//  `UIViewRepresentable` that owns its web view loses the page every time
//  SwiftUI rebuilds the view — and rebuilds happen for reasons that have
//  nothing to do with browsing, which is how a sheet opening reloads somebody's
//  dApp mid-transaction.
//
//  ## What this class decides: nothing
//
//  It reports what WebKit says — the page's messages with the FRAME's origin,
//  a document committed, a load finished or failed, the renderer died — and
//  `dapp_browser` decides what each means (spec 070). It keeps `url`, `host`,
//  `title`, `progress`, `canGoBack`/`canGoForward` and `loading` for the
//  chrome; whether the page is secure, connected or crashed is the core's.
//
//  ## A closed tab is torn down, not merely forgotten
//
//  Before 070 a closed tab's web view kept its script handler — which retained
//  the engine, which retained the web view — so it never died, kept running
//  its page's JavaScript and could still raise a signing sheet. `tearDown()`
//  removes the handler and the script, stops the load and drops the delegates
//  and observers; the handler is held weakly from the start, so there is no
//  cycle to break.
//

import Foundation
import WebKit
import UIKit
import Observation
import VelaCore

@MainActor
@Observable
final class BrowserEngine: NSObject {

    /// Shared by every tab, so a login on one is a login on the next — which
    /// is what a browser means by "tabs".
    private static let processPool = WKProcessPool()

    let id: String

    private(set) var url: String = ""
    /// Through the core's own rule. `""` before the first navigation, and for
    /// anything the rule gives no origin.
    private(set) var origin: String = ""
    private(set) var host: String = ""
    private(set) var title: String = ""
    /// The page's own icon, as a URL — recorded for history, never fetched
    /// here (a tile's mark is drawn: see `ExploreLive`).
    private(set) var favicon: String = ""
    private(set) var canGoBack: Bool = false
    private(set) var canGoForward: Bool = false
    private(set) var loading: Bool = false
    /// `estimatedProgress`, 0…1, for the bar under the address.
    private(set) var progress: Double = 0

    /// Why the last navigation did not happen — the core's class for it, the
    /// corpus key of its sentence and whether retrying can help (spec 079,
    /// `browserLoadClassify`) — or `nil` when nothing has failed since the
    /// last load that got through.
    ///
    /// Both failure callbacks once turned a dead navigation into `loading =
    /// false` and said nothing else, so a page that could not be reached drew
    /// a white rectangle with an empty address bar — 058 found
    /// `app.uniswap.org` doing exactly that on the founder's phone. Until 079
    /// the panel then printed the system's own sentence and a code.
    private(set) var failure: BrowserLoadFailure?
    /// Spec 079: the page as it was last seen, for the tab switcher's card.
    private(set) var snapshot: UIImage?
    /// Wide enough for a card, small enough that a dozen tabs cost little.
    static let snapshotWidth: CGFloat = 360

    /// The corpus key of the failure's reason, for screens that draw it.
    var failureReasonKey: String? { failure?.reasonKey }
    /// Where the failed navigation was going. `webView.url` is `nil` after a
    /// provisional failure — the URL is discarded with the navigation.
    private(set) var failedURL: String = ""
    /// A retry of the failed page is running (spec 079): the panel stays up
    /// and says so, and gives way only to a page that got through.
    private(set) var retrying = false
    /// The delay of the automatic retry now scheduled, if one is.
    private(set) var retryPendingMs: UInt32?
    /// Whether this tab's page is on screen and the app in front — the only
    /// time a failed page retries by itself (spec 079 FR-012).
    private(set) var onScreen = false
    private var appActive = true
    /// Automatic attempts made for the failure on screen; a new address or
    /// the person's own Retry starts the count again.
    private var retryAttempt: UInt32 = 0
    private var retryTask: Task<Void, Never>?
    /// The main document's HTTP status this navigation, when it had one — a
    /// site's own 404 page is not a visit.
    private var navHttpStatus: Int?
    /// How a load is asked of WebKit. A seam, so tests drive the engine
    /// without a socket.
    var loader: (URLRequest) -> Void

    /// Where the load hairline starts the moment a load is asked for
    /// (spec 079): on a slow network the engine's own progress starts only
    /// at the commit, seconds later, and a tap that changes nothing reads as
    /// a tap that did nothing.
    static let requestedProgress = 0.1

    let webView: WKWebView

    /// One string from the page's bridge, with WebKit's facts about the frame
    /// that sent it.
    var onPageMessage: (_ frameOrigin: String, _ isMainFrame: Bool, _ body: String) -> Void = { _, _, _ in }
    /// A new document committed in this tab.
    var onNavigationStarted: (_ url: String) -> Void = { _ in }
    /// A load finished — or failed, provisionally or after commit.
    var onLoadFinished: (_ url: String) -> Void = { _ in }
    /// The web content process died.
    var onRendererGone: () -> Void = {}
    /// The page's URL or title changed — a load, or a single-page app moving
    /// itself with `pushState`, which no navigation callback reports.
    var onMeta: (_ url: String, _ title: String) -> Void = { _, _ in }
    /// A page finished loading, with whatever title and icon it ended up with.
    var onVisited: (_ url: String, _ title: String, _ favicon: String) -> Void = { _, _, _ in }
    /// Navigation state changed — the chrome redraws.
    var onStateChanged: () -> Void = {}

    private var observations: [NSKeyValueObservation] = []
    private var tornDown = false

    init(id: String) {
        self.id = id

        let configuration = WKWebViewConfiguration()
        configuration.processPool = Self.processPool
        configuration.websiteDataStore = .default()
        // A page that autoplays a video the moment it opens is a page nobody
        // asked to hear.
        configuration.mediaTypesRequiringUserActionForPlayback = .all
        // One tab, one web view. A page cannot conjure a second.
        configuration.preferences.javaScriptCanOpenWindowsAutomatically = false

        let webView = WKWebView(frame: .zero, configuration: configuration)
        self.webView = webView
        self.loader = { [weak webView] request in webView?.load(request) }
        super.init()

        ProviderBridge.install(into: configuration.userContentController, handler: self)
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = true
        #if DEBUG
        // Safari ▸ 开发 ▸ this device. The equivalent of the DevTools Android
        // attached over adb, and the only way to see what a page's own
        // JavaScript thinks.
        if #available(iOS 16.4, *) { webView.isInspectable = true }
        #endif
        observe()
    }

    // MARK: - Driving

    /// A new address: the old failure and its retries are over.
    func load(_ text: String) {
        guard !tornDown, let target = URL(string: text) else { return }
        cancelRetry()
        retryAttempt = 0
        failure = nil
        retrying = false
        requested()
        loader(URLRequest(url: target))
    }

    func goBack() {
        guard webView.canGoBack else { return }
        requested()
        webView.goBack()
    }

    func goForward() {
        guard webView.canGoForward else { return }
        requested()
        webView.goForward()
    }

    /// Reload — or, on a failed page, the person's Retry: the panel stays,
    /// saying it is retrying, and the automatic count starts again.
    ///
    /// `WKWebView.reload()` reloads the CURRENT document, and a provisional
    /// failure left none: on that path the page has to be asked for again.
    func reload() {
        cancelRetry()
        if failure != nil {
            retryAttempt = 0
            retry()
            return
        }
        requested()
        if webView.url == nil, !url.isEmpty, let target = URL(string: url) {
            loader(URLRequest(url: target))
            return
        }
        webView.reload()
    }

    /// The person or the page asked for a load (spec 079): progress shows
    /// now, not when the engine commits. The address bar keeps the committed
    /// host until then.
    func requested() {
        guard !tornDown else { return }
        progress = max(loading ? progress : 0, Self.requestedProgress)
        loading = true
        onStateChanged()
    }

    /// One more attempt at the page that failed. The failure stays — the
    /// panel with it — until a page gets through.
    private func retry() {
        guard !tornDown else { return }
        let target = failedURL.isEmpty ? url : failedURL
        guard let request = URL(string: target).map({ URLRequest(url: $0) }) else { return }
        retrying = true
        requested()
        loader(request)
    }

    // MARK: - Retrying by itself (spec 079)

    /// This tab's page came on screen, or left it. A page that failed while
    /// out of sight retries once it is looked at again; one leaving is
    /// photographed for the tab switcher while it is still drawn.
    func setOnScreen(_ shown: Bool) {
        onScreen = shown
        if shown { resumeRetry() } else { cancelRetry(); captureSnapshot() }
    }

    /// Photograph the page as it is now (spec 079, the tab switcher's card).
    /// Only a page in a window can be drawn; `done` runs either way.
    func captureSnapshot(_ done: @escaping () -> Void = {}) {
        guard !tornDown, webView.window != nil, webView.bounds.width > 0 else {
            done()
            return
        }
        let configuration = WKSnapshotConfiguration()
        configuration.snapshotWidth = NSNumber(value: Double(Self.snapshotWidth))
        webView.takeSnapshot(with: configuration) { [weak self] image, _ in
            MainActor.assumeIsolated {
                if let image, let self, !self.tornDown { self.snapshot = image }
                done()
            }
        }
    }

    /// The app came to the front, or left it.
    func setAppActive(_ active: Bool) {
        appActive = active
        active ? resumeRetry() : cancelRetry()
    }

    private func resumeRetry() {
        guard let failure, retryTask == nil, !retrying else { return }
        scheduleRetry(failure)
    }

    /// The core's schedule (2 s, 5 s, 10 s for the network classes; once at
    /// 3 s for "other"; never for a wrong name or a certificate), and only
    /// while this page is on screen and the app in front.
    private func scheduleRetry(_ failure: BrowserLoadFailure) {
        cancelRetry()
        guard failure.autoRetry, onScreen, appActive,
              let wait = browserLoadRetryDelayMs(class: failure.class, attempt: retryAttempt + 1)
        else { return }
        retryPendingMs = wait
        retryTask = Task { @MainActor [weak self] in
            try? await Task.sleep(nanoseconds: UInt64(wait) * 1_000_000)
            guard let self, !Task.isCancelled else { return }
            self.fireRetry()
        }
    }

    /// The scheduled attempt, now. Tests call it rather than waiting.
    func fireRetry() {
        retryTask = nil
        retryPendingMs = nil
        guard failure != nil, onScreen, appActive else { return }
        retryAttempt += 1
        retry()
    }

    private func cancelRetry() {
        retryTask?.cancel()
        retryTask = nil
        retryPendingMs = nil
    }

    /// Post one message into this tab's page. The bridge drops it unless it is
    /// addressed to the document now showing.
    func deliver(_ messageJson: String) {
        guard !tornDown, let expression = ProviderBridge.deliverExpression(messageJson) else { return }
        webView.evaluateJavaScript(expression, in: nil, in: .page) { _ in }
    }

    /// The tab closed. Idempotent.
    func tearDown() {
        guard !tornDown else { return }
        tornDown = true
        cancelRetry()
        observations.forEach { $0.invalidate() }
        observations.removeAll()
        let controller = webView.configuration.userContentController
        ProviderBridge.uninstall(from: controller)
        webView.stopLoading()
        webView.navigationDelegate = nil
        webView.uiDelegate = nil
        // The page's scripts stop with its document; an empty one is the only
        // public way to end a document without the web view going away.
        webView.loadHTMLString("", baseURL: nil)
    }

    // MARK: - State

    /// KVO on what a single-page app changes without a navigation: the URL,
    /// the title, and how far a load has got.
    private func observe() {
        observations = [
            webView.observe(\.estimatedProgress, options: [.new]) { [weak self] webView, _ in
                let value = webView.estimatedProgress
                MainActor.assumeIsolated { self?.progressChanged(value) }
            },
            webView.observe(\.url, options: [.new]) { [weak self] _, _ in
                MainActor.assumeIsolated { self?.metaChanged() }
            },
            webView.observe(\.title, options: [.new]) { [weak self] _, _ in
                MainActor.assumeIsolated { self?.metaChanged() }
            },
        ]
    }

    private func progressChanged(_ value: Double) {
        guard !tornDown else { return }
        progress = value
        onStateChanged()
    }

    private func metaChanged() {
        guard !tornDown, webView.url != nil else { return }
        update(loading: loading)
        guard !url.isEmpty, !origin.isEmpty else { return }
        onMeta(url, title)
    }

    private func update(loading: Bool) {
        // A provisional failure discards the URL, so fall back to the one the
        // navigation was for: an address bar that empties itself tells a
        // person their tap did nothing.
        let current = webView.url?.absoluteString ?? (failure != nil ? failedURL : url)
        url = current
        origin = ProviderBridge.origin(of: current)
        host = Self.hostOf(origin: origin)
        // The document's own title — never the previous page's kept over a
        // nil (spec 079 F2: `?? title` recorded one site under another's).
        title = webView.url == nil ? title : (webView.title ?? "")
        canGoBack = webView.canGoBack
        canGoForward = webView.canGoForward
        self.loading = loading
        onStateChanged()
    }

    static func hostOf(origin: String) -> String {
        guard let separator = origin.range(of: "://") else { return origin }
        let rest = origin[separator.upperBound...]
        return String(rest.prefix { $0 != "/" })
    }

    /// The document's own address, title and icon, absolute, in ONE read —
    /// three reads could straddle two documents (spec 079 R3). Evaluated in
    /// the page's own world, never the provider's.
    static let pageFactsScript = """
    (() => { const l = document.querySelector('link[rel~="icon"], link[rel="apple-touch-icon"]');
      return JSON.stringify({ href: location.href, title: document.title, icon: l && l.href ? l.href : '' }); })()
    """

    /// The visit a finished load is — the core's rule (`browserLoadVisit`):
    /// nothing for a failed load, an error status, a non-web address or an
    /// engine document; otherwise the page's own url, title and icon.
    static func visit(facts: Any?, httpStatus: Int?) -> BrowserVisit? {
        guard let text = facts as? String,
              let object = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any],
              let href = object["href"] as? String
        else { return nil }
        let icon = (object["icon"] as? String).flatMap { $0.isEmpty ? nil : $0 }
        return browserLoadVisit(
            url: href, title: object["title"] as? String ?? "", icon: icon,
            mainFrameFailed: false, httpStatus: httpStatus.flatMap { UInt16(exactly: $0) }
        )
    }

    /// A load finished: read the page once, and record it only if the core
    /// says it is a visit. Never fetched here — the icon is the page's claim.
    private func recordVisit() {
        let status = navHttpStatus
        webView.evaluateJavaScript(Self.pageFactsScript, in: nil, in: .defaultClient) { [weak self] result in
            MainActor.assumeIsolated {
                guard let self, !self.tornDown,
                      case .success(let facts) = result,
                      let visit = Self.visit(facts: facts, httpStatus: status)
                else { return }
                self.favicon = visit.favicon ?? ""
                self.onVisited(visit.url, visit.title ?? "", visit.favicon ?? "")
            }
        }
    }
}

// MARK: - The page's messages

extension BrowserEngine: WKScriptMessageHandler {
    nonisolated func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        // The bridge posts strings; anything else is not the bridge.
        guard let body = message.body as? String else { return }
        let frameOrigin = ProviderBridge.frameOrigin(of: message.frameInfo)
        let isMainFrame = message.frameInfo.isMainFrame
        MainActor.assumeIsolated {
            guard !tornDown else { return }
            onPageMessage(frameOrigin, isMainFrame, body)
        }
    }
}

// MARK: - Navigation

extension BrowserEngine: WKNavigationDelegate {

    nonisolated func webView(
        _ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!
    ) {
        let attempt = webView.url?.absoluteString
        MainActor.assumeIsolated { provisionalStarted(attempt: attempt) }
    }

    /// A new attempt remembers where it is going, because that is the only
    /// moment the URL is knowable if this one fails too. It does NOT clear a
    /// failure (spec 079): the panel stays through a retry and gives way only
    /// to a page that commits.
    func provisionalStarted(attempt: String?) {
        guard !tornDown else { return }
        navHttpStatus = nil
        if let attempt, !attempt.isEmpty { failedURL = attempt }
        update(loading: true)
    }

    /// The document changed: THIS is when the core hears a navigation began.
    ///
    /// Not at the provisional start. A provisional load that fails leaves the
    /// old document on screen and alive; telling the core at the start would
    /// have it retire that document when the load failed, and the page still
    /// showing would be answered by nobody. On iOS the commit also precedes
    /// the new document's `hello` — the document-start script runs in the
    /// document the commit created — so the new page's warm-up requests are
    /// never mistaken for the old page's (research R2).
    nonisolated func webView(
        _ webView: WKWebView, didCommit navigation: WKNavigation!
    ) {
        MainActor.assumeIsolated { committed() }
    }

    /// The site answered and its document is on its way: whatever failed
    /// before is over. (WebKit draws no error page of its own, so a commit is
    /// always the site — unlike Android's WebView.)
    func committed() {
        guard !tornDown else { return }
        clearFailure()
        update(loading: true)
        onNavigationStarted(url)
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated { finished() }
    }

    func finished() {
        guard !tornDown else { return }
        clearFailure()
        update(loading: false)
        onLoadFinished(url)
        if !origin.isEmpty { recordVisit() }
        captureSnapshot()
    }

    private func clearFailure() {
        cancelRetry()
        retryAttempt = 0
        failure = nil
        retrying = false
    }

    /// The main document's response: its HTTP status, for the visit rule.
    nonisolated func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationResponse: WKNavigationResponse,
        decisionHandler: @escaping (WKNavigationResponsePolicy) -> Void
    ) {
        let status = navigationResponse.isForMainFrame
            ? (navigationResponse.response as? HTTPURLResponse)?.statusCode
            : nil
        MainActor.assumeIsolated {
            if let status { navHttpStatus = status }
            decisionHandler(.allow)
        }
    }

    /// A failure AFTER the document committed: the page is on screen, partly
    /// drawn, and a subresource or a same-document navigation gave up.
    ///
    /// Deliberately silent on screen: a full-screen "couldn't load" over a
    /// page that IS there hides a working dApp (058's correction). The core is
    /// still told — a document that committed and never said hello is gone.
    nonisolated func webView(
        _ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error
    ) {
        MainActor.assumeIsolated {
            update(loading: false)
            onLoadFinished(url)
        }
    }

    nonisolated func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError error: Error
    ) {
        let nsError = error as NSError
        let attempt = nsError.userInfo[NSURLErrorFailingURLStringErrorKey] as? String
        MainActor.assumeIsolated {
            provisionalFailed(code: nsError.code, domain: nsError.domain, attempt: attempt)
        }
    }

    /// A navigation that never committed, through the core's classifier.
    func provisionalFailed(code: Int, domain: String, attempt: String?) {
        guard !tornDown else { return }
        if let attempt, !attempt.isEmpty { failedURL = attempt }
        fail(code: code, domain: domain)
        onLoadFinished(url)
    }

    /// The renderer died (memory pressure, a crash). The app does not: the
    /// tab says so and offers a reload, and the core settles every request
    /// the page had open (spec 070 FR-013).
    nonisolated func webViewWebContentProcessDidTerminate(_ webView: WKWebView) {
        MainActor.assumeIsolated {
            guard !tornDown else { return }
            loading = false
            progress = 0
            onRendererGone()
            onStateChanged()
        }
    }

    /// A navigation that did not happen, classified by the core (spec 079)
    /// and shown with its reason; retried on the core's schedule when a
    /// retry can help.
    private func fail(code: Int, domain: String) {
        // Not a failure (the core's `None`): a cancelled navigation — a page
        // that navigates while the last request is in flight cancels it, and
        // every redirect chain does this — or a frame load interrupted by a
        // new one. A retry cut short that way is simply over.
        guard let classified = browserLoadClassify(
            platform: "apple", code: Int64(code), domain: domain, certificate: false
        ) else {
            retrying = false
            update(loading: false)
            return
        }
        failure = classified
        retrying = false
        update(loading: false)
        print("[vela-wallet] browser load failed: \(failedURL) — \(domain) \(code) → \(classified.class)")
        scheduleRetry(classified)
    }

    /// `http` and `https` load here; `about:blank` and `about:srcdoc` load
    /// in frames. Anything else is handed to the system — and ONLY for a link
    /// the person tapped in the top frame.
    ///
    /// A page that could open `tel:`, `mailto:` or an app's own scheme by
    /// script, or from an ad iframe, would be a page that can launch anything
    /// installed without anyone asking for it. The system still asks the
    /// person before it leaves the app.
    nonisolated func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        let target = navigationAction.request.url
        let isMainFrame = navigationAction.targetFrame?.isMainFrame ?? true
        switch Self.policy(
            scheme: target?.scheme,
            isMainFrame: isMainFrame,
            linkActivated: navigationAction.navigationType == .linkActivated
        ) {
        case .allow:
            // A new document the page asked for (a link, a form, a script):
            // progress from the tap, not from the commit (spec 079). Not for a
            // jump within the same document — no load follows one.
            if isMainFrame, let target {
                let current = webView.url
                MainActor.assumeIsolated {
                    if Self.leavesDocument(from: current, to: target) { requested() }
                }
            }
            decisionHandler(.allow)
        case .cancel:
            decisionHandler(.cancel)
        case .handToSystem:
            decisionHandler(.cancel)
            if let target {
                MainActor.assumeIsolated { UIApplication.shared.open(target) }
            }
        }
    }

    enum Policy: Equatable { case allow, cancel, handToSystem }

    /// Whether going to `target` loads a new document: anything but a jump to
    /// a fragment of the page already showing.
    nonisolated static func leavesDocument(from current: URL?, to target: URL) -> Bool {
        guard let current else { return true }
        func bare(_ url: URL) -> String {
            var components = URLComponents(url: url, resolvingAgainstBaseURL: false)
            components?.fragment = nil
            return components?.string ?? url.absoluteString
        }
        return bare(current) != bare(target) || target.fragment == nil
    }

    /// The rule above, as a pure function.
    nonisolated static func policy(scheme: String?, isMainFrame: Bool, linkActivated: Bool) -> Policy {
        let scheme = scheme?.lowercased() ?? ""
        if scheme == "http" || scheme == "https" { return .allow }
        if scheme == "about" || scheme == "blob" || scheme == "data" {
            // Frames and documents the page builds itself. A top-level `data:`
            // document is a phishing staple, so only a subframe may have one.
            return isMainFrame && scheme != "about" ? .cancel : .allow
        }
        if scheme == "javascript" || scheme == "file" || scheme.isEmpty { return .cancel }
        return isMainFrame && linkActivated ? .handToSystem : .cancel
    }
}

// MARK: - Windows

extension BrowserEngine: WKUIDelegate {
    /// `target="_blank"` loads in **this** tab.
    ///
    /// Returning a new web view would make a second engine SwiftUI never
    /// mounts and the controller never knows about — a page that keeps running
    /// with the provider inside it and nobody watching.
    nonisolated func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        if navigationAction.targetFrame == nil, let target = navigationAction.request.url {
            let scheme = target.scheme?.lowercased() ?? ""
            if scheme == "http" || scheme == "https" {
                webView.load(URLRequest(url: target))
            }
        }
        return nil
    }

    // MARK: JavaScript dialogs (spec 070)
    //
    // Without these WebKit answers every `alert()` at once and every
    // `confirm()` with false, so a page asking "Leave without saving?" never
    // asked. A page that is not on screen gets the same instant answer: it
    // cannot put a dialog over the wallet. The buttons are UIKit's own
    // localized "OK"/"Cancel" — the words Safari's dialogs use.

    nonisolated func webView(
        _ webView: WKWebView,
        runJavaScriptAlertPanelWithMessage message: String,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping @MainActor @Sendable () -> Void
    ) {
        MainActor.assumeIsolated {
            guard let host = Self.presenter(for: webView) else { return completionHandler() }
            let alert = UIAlertController(title: frame.securityOrigin.host, message: message, preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: Self.systemWord("OK"), style: .default) { _ in completionHandler() })
            host.present(alert, animated: true)
        }
    }

    nonisolated func webView(
        _ webView: WKWebView,
        runJavaScriptConfirmPanelWithMessage message: String,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping @MainActor @Sendable (Bool) -> Void
    ) {
        MainActor.assumeIsolated {
            guard let host = Self.presenter(for: webView) else { return completionHandler(false) }
            let alert = UIAlertController(title: frame.securityOrigin.host, message: message, preferredStyle: .alert)
            alert.addAction(UIAlertAction(title: Self.systemWord("Cancel"), style: .cancel) { _ in completionHandler(false) })
            alert.addAction(UIAlertAction(title: Self.systemWord("OK"), style: .default) { _ in completionHandler(true) })
            host.present(alert, animated: true)
        }
    }

    nonisolated func webView(
        _ webView: WKWebView,
        runJavaScriptTextInputPanelWithPrompt prompt: String,
        defaultText: String?,
        initiatedByFrame frame: WKFrameInfo,
        completionHandler: @escaping @MainActor @Sendable (String?) -> Void
    ) {
        MainActor.assumeIsolated {
            guard let host = Self.presenter(for: webView) else { return completionHandler(nil) }
            let alert = UIAlertController(title: frame.securityOrigin.host, message: prompt, preferredStyle: .alert)
            alert.addTextField { field in field.text = defaultText }
            alert.addAction(UIAlertAction(title: Self.systemWord("Cancel"), style: .cancel) { _ in completionHandler(nil) })
            alert.addAction(UIAlertAction(title: Self.systemWord("OK"), style: .default) { [weak alert] _ in
                completionHandler(alert?.textFields?.first?.text ?? "")
            })
            host.present(alert, animated: true)
        }
    }

    /// The controller a dialog may be shown from: the topmost one, and only
    /// when it is the one showing the page. `nil` when the page is off screen
    /// or covered — a page's dialog never lands on a wallet sheet.
    @MainActor
    private static func presenter(for webView: WKWebView) -> UIViewController? {
        guard let window = webView.window, var top = window.rootViewController else { return nil }
        while let presented = top.presentedViewController { top = presented }
        return webView.isDescendant(of: top.view) ? top : nil
    }

    private nonisolated static func systemWord(_ key: String) -> String {
        Bundle(for: UIApplication.self).localizedString(forKey: key, value: key, table: nil)
    }
}
