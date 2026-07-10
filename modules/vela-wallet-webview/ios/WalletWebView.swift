// WalletWebView (iOS) — a wallet-owned WKWebView for the in-app dApp browser.
//
// Responsibilities (see docs/dapp-browser/ARCHITECTURE.md):
//   • Inject the shared EIP-1193/6963 provider (inpage.js + protocol.js + shim)
//     at DOCUMENT START, main frame only, so `window.ethereum` exists before any
//     page JS runs and is immune to the page CSP.
//   • Receive provider requests on the `velaBridge` message handler and bubble
//     them to RN, stamping the TRUSTED origin from `frameInfo.securityOrigin`
//     and `frameInfo.isMainFrame` — NEVER an origin the page put in the payload.
//   • Deliver responses/events back into the page via `window.__velaRespond` /
//     `window.__velaEmit` (the injected shim owns the vela-1193 envelope).
//   • Report navigation lifecycle so RN can drive the URL bar and settle pending
//     signing on navigation.
//
// This is a classic RN view (legacy RCTViewManager), copied into the app target
// by plugins/with-native-modules.js, mirroring vela-passkey / vela-app-group.
import Foundation
import UIKit
import WebKit
import React

/// Reads the page's best-declared favicon as an ABSOLUTE url (link.href resolves
/// relative paths), falling back to the origin's /favicon.ico. Shared verbatim with
/// the Android view so both platforms report the same thing.
private let FAVICON_JS = """
(function(){try{var s=['link[rel~="icon"]','link[rel="shortcut icon"]','link[rel="apple-touch-icon"]'];\
for(var i=0;i<s.length;i++){var l=document.querySelector(s[i]);if(l&&l.href)return l.href;}\
return location.origin+'/favicon.ico';}catch(e){return '';}})()
"""

@objc(WalletWebView)
class WalletWebView: UIView, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate {

  // MARK: RN-exported callbacks
  @objc var onProviderRequest: RCTDirectEventBlock?
  @objc var onNavigationChange: RCTDirectEventBlock?

  private var webView: WKWebView?
  private var pendingURL: URL?
  private var providerScriptInstalled = false
  private var lastSeq = 0 // highest processed outbox seq (native ← RN deliveries)
  private var lastFavicon = "" // resolved favicon URL for the current document

  // MARK: Lifecycle

  override init(frame: CGRect) {
    super.init(frame: frame)
    setUpWebView()
  }

  required init?(coder: NSCoder) {
    super.init(coder: coder)
    setUpWebView()
  }

  private func setUpWebView() {
    let config = WKWebViewConfiguration()
    config.userContentController = WKUserContentController()
    config.websiteDataStore = .default() // persistent cookies / localStorage (browser-completeness comes later)
    if #available(iOS 14.0, *) {
      config.defaultWebpagePreferences.allowsContentJavaScript = true
    }

    let wv = WKWebView(frame: bounds, configuration: config)
    // Page → native channel. Register on the LIVE controller (post-init): WKWebView
    // COPIES the configuration, and while user scripts survive that copy (injection
    // works), a script-message-handler added to the pre-init controller does NOT —
    // which is exactly why injection succeeded but messages never arrived.
    // `.postMessage` in the shim then lands in userContentController(_:didReceive:).
    //
    // Register via a WEAK proxy: WKUserContentController retains its message handler
    // strongly, and self owns the webView (→ configuration → userContentController),
    // so registering `self` directly is a retain cycle that leaks the whole WKWebView
    // on every browser open (deinit never runs). The proxy holds self weakly.
    wv.configuration.userContentController.add(WeakScriptMessageHandler(self), name: "velaBridge")
    wv.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    wv.navigationDelegate = self // weak on WKWebView — no cycle
    wv.uiDelegate = self // weak on WKWebView — for the prompt()-based page→native bridge
    wv.allowsBackForwardNavigationGestures = true
    addSubview(wv)
    webView = wv
  }

  // MARK: Props (set by the view manager)

  /// The full injected bundle (INJECTED_PROVIDER_JS). Registered as a
  /// document-start user script the first time it is set.
  @objc func setInjectedJavaScript(_ source: NSString?) {
    guard !providerScriptInstalled, let source = source as String?, !source.isEmpty else { return }
    let script = WKUserScript(source: source, injectionTime: .atDocumentStart, forMainFrameOnly: true)
    webView?.configuration.userContentController.addUserScript(script)
    providerScriptInstalled = true
    loadIfReady()
  }

  /// The URL to load. Deferred to the next run-loop tick so the injectedJavaScript
  /// prop (same commit batch) is registered before the first navigation begins.
  @objc func setSourceURL(_ value: NSString?) {
    guard let str = value as String?, let url = URL(string: str) else { return }
    pendingURL = url
    DispatchQueue.main.async { [weak self] in self?.loadIfReady() }
  }

  private func loadIfReady() {
    guard providerScriptInstalled, let url = pendingURL, let wv = webView else { return }
    pendingURL = nil
    wv.load(URLRequest(url: url))
  }

  /// native → page deliveries, as a seq-tracked JSON queue prop. Props flow
  /// reliably under the New-Arch interop layer (unlike NativeModules view-manager
  /// commands, which are unreachable in bridgeless mode). Process items whose seq
  /// exceeds the last one we ran.
  @objc func setOutbox(_ value: NSString?) {
    guard let s = value as String?, let data = s.data(using: .utf8),
          let arr = (try? JSONSerialization.jsonObject(with: data)) as? [[String: Any]] else { return }
    for item in arr {
      guard let seq = item["seq"] as? Int, seq > lastSeq else { continue }
      lastSeq = seq
      switch item["t"] as? String {
      case "res":
        evaluate("window.__velaRespond && window.__velaRespond(\(jsString(item["id"] as? String ?? "")), \(jsonLit(item["result"])), \(jsonLit(item["error"])));")
      case "evt":
        evaluate("window.__velaEmit && window.__velaEmit(\(jsString(item["event"] as? String ?? "")), \(jsonLit(item["data"])));")
      case "nav":
        switch item["action"] as? String {
        case "back": goBack()
        case "forward": goForward()
        case "reload": reloadPage()
        default: break
        }
      default:
        break
      }
    }
  }

  // MARK: Native → page (invoked by the view manager commands)

  func deliverResponse(id: String, resultJson: String, errorJson: String) {
    let idLit = jsString(id)
    let resultLit = resultJson.isEmpty ? "undefined" : resultJson
    let errorLit = errorJson.isEmpty ? "null" : errorJson
    evaluate("window.__velaRespond && window.__velaRespond(\(idLit), \(resultLit), \(errorLit));")
  }

  func deliverEvent(event: String, dataJson: String) {
    let eventLit = jsString(event)
    let dataLit = dataJson.isEmpty ? "null" : dataJson
    evaluate("window.__velaEmit && window.__velaEmit(\(eventLit), \(dataLit));")
  }

  func goBack() { if webView?.canGoBack == true { webView?.goBack() } }
  func goForward() { if webView?.canGoForward == true { webView?.goForward() } }
  func reloadPage() { webView?.reload() }

  private func evaluate(_ js: String) {
    DispatchQueue.main.async { [weak self] in self?.webView?.evaluateJavaScript(js, completionHandler: nil) }
  }

  // MARK: WKScriptMessageHandler (page → native)

  func userContentController(_ userContentController: WKUserContentController, didReceive message: WKScriptMessage) {
    // Kept as a fallback; on-device the interop-registered WKScriptMessageHandler
    // does not deliver, so the shim uses the custom-scheme channel (below) instead.
    if message.name == "velaBridge", let raw = message.body as? String {
      processBridge(raw, from: message.frameInfo)
    }
  }

  /// WKUIDelegate: the prompt()-based page→native bridge. The shim calls
  /// `window.prompt('velawvbridge:' + payload)`; we read it and dismiss silently.
  func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String,
               defaultText: String?, initiatedByFrame frame: WKFrameInfo,
               completionHandler: @escaping (String?) -> Void) {
    if prompt.hasPrefix("velawvbridge:") {
      // SECURITY: stamp origin/isMainFrame from the INITIATING frame, so a
      // cross-origin subframe that calls window.prompt('velawvbridge:…') directly
      // (it can — prompt is a native global, no shim needed) cannot masquerade as
      // the main frame. The WebViewTransport 4100s any isMainFrame=false request.
      processBridge(String(prompt.dropFirst("velawvbridge:".count)), from: frame)
      completionHandler(nil) // dismiss without showing a dialog
      return
    }
    // Not our bridge — a real page prompt(). Return nil (= user dismissed) rather
    // than defaultText (which falsely reports "user confirmed the default value").
    completionHandler(nil)
  }

  /// Process one page→native `vela-1193` request envelope (from either channel).
  /// `frame` is the WKFrameInfo the request actually came from — its
  /// `securityOrigin` and `isMainFrame` are the TRUSTED source of both fields
  /// (never a value the page put in the payload, never the top-frame URL).
  private func processBridge(_ raw: String, from frame: WKFrameInfo) {
    guard let data = raw.data(using: .utf8),
          let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
          (obj["dir"] as? String) == "req" else { return } // ignore the "ready" ping etc.
    onProviderRequest?([
      "requestId": obj["id"] as? String ?? "",
      "method": obj["method"] as? String ?? "",
      "params": obj["params"] ?? [],
      "origin": originString(from: frame.securityOrigin),
      "isMainFrame": frame.isMainFrame,
    ])
  }

  // MARK: WKNavigationDelegate (navigation lifecycle)

  /// Reliable JS→native channel: intercept the custom bridge scheme the shim
  /// navigates a throwaway iframe to (the WKScriptMessageHandler doesn't fire
  /// under the RN New-Arch interop view).
  func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
    guard let url = navigationAction.request.url else { decisionHandler(.allow); return }
    let scheme = url.scheme?.lowercased() ?? ""

    // Reliable JS→native channel: the shim navigates a throwaway iframe to this
    // custom scheme (the WKScriptMessageHandler doesn't fire under the RN interop).
    if scheme == "velawvbridge" {
      let full = url.absoluteString
      if let r = full.range(of: "velawvbridge://m/") {
        let enc = String(full[r.upperBound...])
        // SECURITY: stamp from navigationAction.sourceFrame (the frame that issued
        // the bridge navigation), not the top frame — same guard as the prompt path.
        if let decoded = enc.removingPercentEncoding {
          processBridge(decoded, from: navigationAction.sourceFrame)
        }
      }
      decisionHandler(.cancel)
      return
    }

    // Normal web content loads in the WebView.
    if scheme == "http" || scheme == "https" || scheme == "about" || scheme == "blob" {
      decisionHandler(.allow)
      return
    }

    // External scheme (mailto:, tel:, wc:, itms-apps:, https-app-links, …): hand to
    // the OS — but ONLY for a real main-frame link tap. A programmatic redirect
    // (`location.href='facetime://…'`) or a hidden cross-origin iframe is
    // navigationType .other / a subframe, and must NOT be able to launch external
    // apps or our own velawallet:// deep link without a user gesture. Dangerous
    // local schemes (javascript:, file:, data:) are never loaded nor opened.
    decisionHandler(.cancel)
    let userTapped = navigationAction.navigationType == .linkActivated && navigationAction.sourceFrame.isMainFrame
    if userTapped, scheme != "javascript", scheme != "file", scheme != "data", scheme != "velawallet" {
      UIApplication.shared.open(url, options: [:], completionHandler: nil)
    }
  }

  /// target=_blank / window.open — WKWebView otherwise does nothing (targetFrame
  /// is nil). Load the request in THIS view so "view tx / docs / twitter" links work.
  func webView(_ webView: WKWebView, createWebViewWith configuration: WKWebViewConfiguration,
               for navigationAction: WKNavigationAction, windowFeatures: WKWindowFeatures) -> WKWebView? {
    if navigationAction.targetFrame == nil, let url = navigationAction.request.url,
       let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" {
      webView.load(URLRequest(url: url))
    }
    return nil
  }

  func webView(_ webView: WKWebView, didStartProvisionalNavigation navigation: WKNavigation!) {
    lastFavicon = "" // new document — drop the previous page's favicon
    // Emit loading EARLY (before didCommit) so the DNS/TLS/first-byte wait — the
    // slowest phase — shows a progress bar instead of a blank screen.
    emitNavigation(loading: true)
  }
  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    emitNavigation(loading: true)
  }
  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
    emitNavigation(loading: false)
    resolveFavicon() // async — emits a follow-up nav update once the URL is known
  }
  func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
    emitNavigation(loading: false, error: error)
  }
  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
    emitNavigation(loading: false, error: error)
  }

  /// Resolve the page's declared favicon (absolute URL) and re-emit navigation so
  /// the URL bar can show it. Falls back to /favicon.ico.
  private func resolveFavicon() {
    guard let wv = webView else { return }
    let urlAtCall = wv.url
    wv.evaluateJavaScript(FAVICON_JS) { [weak self] result, _ in
      guard let self = self, let w = self.webView, w.url == urlAtCall else { return } // navigated away — drop the stale result
      if let href = result as? String, !href.isEmpty {
        self.lastFavicon = href
        self.emitNavigation(loading: false)
      }
    }
  }

  private func emitNavigation(loading: Bool, error: Error? = nil) {
    guard let wv = webView else { return }
    // Ignore the benign "cancelled" that fires when we redirect an external scheme
    // or start a new load over an in-flight one — it isn't a real page failure.
    let ns = error as NSError?
    let isCancel = ns?.domain == NSURLErrorDomain && ns?.code == NSURLErrorCancelled
    onNavigationChange?([
      "url": wv.url?.absoluteString ?? "",
      "title": wv.title ?? "",
      "canGoBack": wv.canGoBack,
      "canGoForward": wv.canGoForward,
      "loading": loading,
      "error": (error != nil && !isCancel) ? (ns?.localizedDescription ?? "Failed to load") : "",
      "favicon": lastFavicon,
    ])
  }

  // MARK: Helpers

  private func originString(from o: WKSecurityOrigin) -> String {
    if o.protocol.isEmpty && o.host.isEmpty { return "" }
    let scheme = o.protocol
    let host = o.host
    let isDefaultPort = (scheme == "https" && o.port == 443) || (scheme == "http" && o.port == 80) || o.port == 0
    return isDefaultPort ? "\(scheme)://\(host)" : "\(scheme)://\(host):\(o.port)"
  }

  /// Serialize an arbitrary JSON value into a JS literal (`"0x1"`, `[…]`, `{…}`,
  /// `1`, `true`, or `null`). Wrap in an array so scalars serialize, then strip.
  private func jsonLit(_ value: Any?) -> String {
    guard let value = value, !(value is NSNull) else { return "null" }
    if let d = try? JSONSerialization.data(withJSONObject: [value]),
       let s = String(data: d, encoding: .utf8) {
      return String(s.dropFirst().dropLast())
    }
    return "null"
  }

  /// JSON-encode a string into a safe JS string literal.
  private func jsString(_ s: String) -> String {
    if let d = try? JSONSerialization.data(withJSONObject: [s]),
       let arr = String(data: d, encoding: .utf8) {
      // arr is `["..."]` — strip the surrounding brackets to get the bare literal.
      return String(arr.dropFirst().dropLast())
    }
    return "\"\""
  }

  deinit {
    webView?.configuration.userContentController.removeScriptMessageHandler(forName: "velaBridge")
    webView?.stopLoading()
  }
}

/// A weak forwarder so WKUserContentController (which retains its message handler
/// strongly) does not retain the WalletWebView — otherwise self → webView →
/// configuration → userContentController → self is a cycle that leaks the whole
/// WKWebView on every browser open. See setUpWebView().
private final class WeakScriptMessageHandler: NSObject, WKScriptMessageHandler {
  private weak var delegate: WKScriptMessageHandler?
  init(_ delegate: WKScriptMessageHandler) { self.delegate = delegate }
  func userContentController(_ controller: WKUserContentController, didReceive message: WKScriptMessage) {
    delegate?.userContentController(controller, didReceive: message)
  }
}
