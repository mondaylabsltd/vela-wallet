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
    func reload() { webView.reload() }

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
        let current = webView.url?.absoluteString ?? ""
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
        MainActor.assumeIsolated {
            update(loading: true)
            onNavigationStarted(url)
        }
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated {
            update(loading: false)
            onMeta(url, title, favicon)
        }
    }

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
        MainActor.assumeIsolated { update(loading: false) }
    }

    nonisolated func webView(
        _ webView: WKWebView, didCommit navigation: WKNavigation!
    ) {
        MainActor.assumeIsolated { update(loading: true) }
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
