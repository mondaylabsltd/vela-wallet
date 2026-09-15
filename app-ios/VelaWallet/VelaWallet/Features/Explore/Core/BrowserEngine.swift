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
//  It reports `url`, `origin`, `host`, `secure`, `title`, `canGoBack`,
//  `canGoForward` and `loading`. The origin is the core's `dappOriginOf`,
//  not a substring. Whether a page may be told anything is
//  `dapp_permissions`'.
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
    /// anything the rule gives no origin — a `file://` page, say, which is
    /// exactly why the test harness is served over loopback.
    private(set) var origin: String = ""
    private(set) var host: String = ""
    private(set) var secure: Bool = false
    private(set) var title: String = ""
    private(set) var favicon: String = ""
    private(set) var canGoBack: Bool = false
    private(set) var canGoForward: Bool = false
    private(set) var loading: Bool = false

    /// Why the last navigation did not happen, in the SYSTEM's words, or `nil`
    /// when nothing has failed since the last successful load.
    ///
    /// **This is the half that was missing.** Both failure callbacks turned a
    /// dead navigation into `loading = false` and said nothing else, so a page
    /// that could not be reached drew a white rectangle with an empty address
    /// bar — 058 found `app.uniswap.org` doing exactly that on the founder's
    /// phone, for sixty seconds, in silence.
    private(set) var failure: String?
    /// Where the failed navigation was going. `webView.url` is `nil` after a
    /// provisional failure — the URL is discarded with the navigation — which
    /// is why the address bar went blank and why this is kept separately.
    private(set) var failedURL: String = ""

    let webView: WKWebView

    /// A request arrived from the page.
    var onIncoming: (ProviderIncoming) -> Void = { _ in }
    /// A fresh document load started. The permissions machine settles every
    /// pending request on this.
    var onNavigationStarted: (String) -> Void = { _ in }
    /// A page settled, with whatever title and icon it ended up with.
    var onMeta: (_ url: String, _ title: String, _ favicon: String) -> Void = { _, _, _ in }
    /// Navigation state changed — the chrome redraws.
    var onStateChanged: () -> Void = {}

    init(id: String, bundle: Bundle = .main) {
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

        ProviderBridge.install(
            into: configuration.userContentController, handler: self, bundle: bundle
        )
        webView.navigationDelegate = self
        webView.uiDelegate = self
        webView.allowsBackForwardNavigationGestures = true
        #if DEBUG
        // Safari ▸ 开发 ▸ this device. The equivalent of the DevTools Android
        // attached over adb, and the only way to see what a page's own
        // JavaScript thinks.
        if #available(iOS 16.4, *) { webView.isInspectable = true }
        #endif
    }

    // MARK: - Driving

    func load(_ text: String) {
        guard let target = URL(string: Self.coerce(text)) else { return }
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
        webView.reload()
    }

    /// What a person typed. A bare host becomes `https://`; a full URL is left
    /// alone. Search is not this cut's — a query that is not a URL simply does
    /// not load, rather than being sent to a search engine nobody chose.
    static func coerce(_ text: String) -> String {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.isEmpty { return "" }
        if trimmed.hasPrefix("http://") || trimmed.hasPrefix("https://") { return trimmed }
        return "https://" + trimmed
    }

    // MARK: - State

    private func update(loading: Bool) {
        // A provisional failure discards the URL, so fall back to the one the
        // navigation was for: an address bar that empties itself tells a
        // person their tap did nothing, when what happened is that the page
        // refused to come.
        let current = webView.url?.absoluteString ?? (failure != nil ? failedURL : "")
        url = current
        origin = ProviderBridge.origin(of: current)
        host = Self.hostOf(origin: origin)
        secure = current.hasPrefix("https://")
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
}

// MARK: - The page's messages

extension BrowserEngine: WKScriptMessageHandler {
    nonisolated func userContentController(
        _ userContentController: WKUserContentController,
        didReceive message: WKScriptMessage
    ) {
        // `WKScriptMessageHandler` delivers on the main thread, and the URL
        // must be read here: a document-start script asks before any
        // navigation callback has said where it is.
        MainActor.assumeIsolated {
            guard let incoming = ProviderBridge.incoming(from: message) else { return }
            onIncoming(incoming)
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
            onNavigationStarted(url)
        }
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated {
            failure = nil
            update(loading: false)
            onMeta(url, title, favicon)
        }
    }

    /// A failure AFTER the document committed: the page is on screen, partly
    /// drawn, and a subresource or a same-document navigation gave up.
    ///
    /// Deliberately silent, and this is a correction of 058's first attempt.
    /// A full-screen "couldn't load" over a page that IS there hides a working
    /// dApp — it is how the browser acceptance suite went from two failures to
    /// eight: the panel covered the test page and every provider assertion
    /// read as "the page did not hear the announcement". The panel is for the
    /// case with nothing behind it, which is the provisional one.
    nonisolated func webView(
        _ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error
    ) {
        MainActor.assumeIsolated { update(loading: false) }
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
        }
    }

    nonisolated func webView(
        _ webView: WKWebView, didCommit navigation: WKNavigation!
    ) {
        MainActor.assumeIsolated {
            failure = nil
            update(loading: true)
        }
    }

    /// A navigation that did not happen, recorded and reported.
    private func fail(_ described: String) {
        // Cancellation is not a failure: a page that navigates while the last
        // request is in flight cancels it, and every redirect chain does this.
        // Drawing "couldn't load" there would put an error over a page that is
        // loading perfectly well.
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
    ///
    /// `nonisolated` because both failure callbacks are, and the error is a
    /// value: reading it does not need the actor the state does.
    nonisolated static func describe(_ error: Error) -> String {
        let nsError = error as NSError
        guard nsError.domain == NSURLErrorDomain else { return nsError.localizedDescription }
        if nsError.code == NSURLErrorCancelled { return cancelled }
        return "\(nsError.localizedDescription) (\(nsError.code))"
    }

    /// Only `http` and `https` load here.
    ///
    /// Those are the two schemes the origin rule knows, and a browser that
    /// followed `mailto:` or a custom app scheme would be handing a page the
    /// ability to launch anything installed. Anything else goes to the system,
    /// which asks the person first.
    nonisolated func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        let scheme = navigationAction.request.url?.scheme?.lowercased() ?? ""
        if scheme == "http" || scheme == "https" {
            decisionHandler(.allow)
            return
        }
        decisionHandler(.cancel)
        if let target = navigationAction.request.url {
            MainActor.assumeIsolated { UIApplication.shared.open(target) }
        }
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
            webView.load(URLRequest(url: target))
        }
        return nil
    }
}
