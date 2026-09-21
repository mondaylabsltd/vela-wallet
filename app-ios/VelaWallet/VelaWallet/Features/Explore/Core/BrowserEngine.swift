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

    /// Why the last navigation did not happen, in the SYSTEM's words, or `nil`
    /// when nothing has failed since the last successful load.
    ///
    /// Both failure callbacks once turned a dead navigation into `loading =
    /// false` and said nothing else, so a page that could not be reached drew
    /// a white rectangle with an empty address bar — 058 found
    /// `app.uniswap.org` doing exactly that on the founder's phone.
    private(set) var failure: String?
    /// Where the failed navigation was going. `webView.url` is `nil` after a
    /// provisional failure — the URL is discarded with the navigation.
    private(set) var failedURL: String = ""

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

        self.webView = WKWebView(frame: .zero, configuration: configuration)
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

    func load(_ text: String) {
        guard !tornDown, let target = URL(string: text) else { return }
        webView.load(URLRequest(url: target))
    }

    func goBack() { webView.goBack() }
    func goForward() { webView.goForward() }

    /// Reload — or re-attempt the navigation that failed.
    ///
    /// `WKWebView.reload()` reloads the CURRENT document, and a provisional
    /// failure left none: on that path the page has to be asked for again.
    func reload() {
        if failure != nil, !failedURL.isEmpty {
            load(failedURL)
            return
        }
        if webView.url == nil, !url.isEmpty {
            load(url)
            return
        }
        webView.reload()
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
        title = webView.title ?? title
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

    /// The page's declared icon, or the origin's `/favicon.ico`. Read from the
    /// page's DOM once it has loaded; never fetched here.
    private func readFavicon() {
        let script = """
        (() => { const l = document.querySelector('link[rel~="icon"], link[rel="apple-touch-icon"]');
          return l && l.href ? l.href : ''; })()
        """
        let fallback = origin.isEmpty ? "" : origin + "/favicon.ico"
        webView.evaluateJavaScript(script, in: nil, in: .defaultClient) { [weak self] result in
            MainActor.assumeIsolated {
                guard let self, !self.tornDown else { return }
                var href = fallback
                if case .success(let value) = result, let text = value as? String,
                   text.hasPrefix("https://") || text.hasPrefix("http://") {
                    href = text
                }
                self.favicon = href
                self.onVisited(self.url, self.title, href)
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
        MainActor.assumeIsolated {
            // A new attempt clears the last failure — and remembers where it
            // is going, because that is the only moment the URL is knowable if
            // this one fails too.
            failure = nil
            if let attempt, !attempt.isEmpty { failedURL = attempt }
            update(loading: true)
        }
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
        MainActor.assumeIsolated {
            failure = nil
            update(loading: true)
            onNavigationStarted(url)
        }
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated {
            failure = nil
            update(loading: false)
            onLoadFinished(url)
            if !origin.isEmpty { readFavicon() }
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
        let described = Self.describe(error)
        let attempt = (error as NSError)
            .userInfo[NSURLErrorFailingURLStringErrorKey] as? String
        MainActor.assumeIsolated {
            if let attempt, !attempt.isEmpty { failedURL = attempt }
            fail(described)
            onLoadFinished(url)
        }
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

    /// A navigation that did not happen, recorded and reported.
    private func fail(_ described: String) {
        // Cancellation is not a failure: a page that navigates while the last
        // request is in flight cancels it, and every redirect chain does this.
        guard described != Self.cancelled else {
            update(loading: false)
            return
        }
        failure = described
        update(loading: false)
        print("[vela-wallet] browser load failed: \(failedURL) — \(described)")
    }

    nonisolated static let cancelled = "cancelled"

    /// The system's own words for what went wrong, kept short enough to sit
    /// under a sentence from the corpus.
    nonisolated static func describe(_ error: Error) -> String {
        let nsError = error as NSError
        guard nsError.domain == NSURLErrorDomain else { return nsError.localizedDescription }
        if nsError.code == NSURLErrorCancelled { return cancelled }
        return "\(nsError.localizedDescription) (\(nsError.code))"
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
        switch Self.policy(
            scheme: target?.scheme,
            isMainFrame: navigationAction.targetFrame?.isMainFrame ?? true,
            linkActivated: navigationAction.navigationType == .linkActivated
        ) {
        case .allow:
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
}
