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
import WebKit
import React

@objc(WalletWebView)
class WalletWebView: UIView, WKScriptMessageHandler, WKNavigationDelegate, WKUIDelegate {

  // MARK: RN-exported callbacks
  @objc var onProviderRequest: RCTDirectEventBlock?
  @objc var onNavigationChange: RCTDirectEventBlock?

  private var webView: WKWebView?
  private var pendingURL: URL?
  private var providerScriptInstalled = false
  private var lastSeq = 0 // highest processed outbox seq (native ← RN deliveries)

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
    wv.configuration.userContentController.add(self, name: "velaBridge")
    wv.autoresizingMask = [.flexibleWidth, .flexibleHeight]
    wv.navigationDelegate = self
    wv.uiDelegate = self // for the prompt()-based page→native bridge
    wv.allowsBackForwardNavigationGestures = true
    addSubview(wv)
    webView = wv
    // Timer diagnostic (fires regardless of delegates): 3s after setup, report ON
    // THE VISIBLE PAGE whether this webview's delegates point to self + callback is
    // wired. If __delck never arrives, self.webView is a DIFFERENT instance than the
    // page the dApp runs in (a two-instance interop problem).
    DispatchQueue.main.asyncAfter(deadline: .now() + 3.0) { [weak self] in
      guard let self = self, let w = self.webView else { return }
      let navOk = (w.navigationDelegate as AnyObject?) === self
      let uiOk = (w.uiDelegate as AnyObject?) === self
      w.evaluateJavaScript("window.__delck={nav:\(navOk ? 1 : 0),ui:\(uiOk ? 1 : 0),cb:\(self.onProviderRequest != nil ? 1 : 0),eth:!!window.ethereum};", completionHandler: nil)
    }
  }

  // MARK: Props (set by the view manager)

  /// The full injected bundle (INJECTED_PROVIDER_JS). Registered as a
  /// document-start user script the first time it is set.
  @objc func setInjectedJavaScript(_ source: NSString?) {
    NSLog("[VelaWV] setInjectedJavaScript (control: this path is known to run)")
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
    if message.name == "velaBridge", let raw = message.body as? String { processBridge(raw, in: message.webView) }
  }

  /// WKUIDelegate: the prompt()-based page→native bridge. The shim calls
  /// `window.prompt('velawvbridge:' + payload)`; we read it and dismiss silently.
  func webView(_ webView: WKWebView, runJavaScriptTextInputPanelWithPrompt prompt: String,
               defaultText: String?, initiatedByFrame frame: WKFrameInfo,
               completionHandler: @escaping (String?) -> Void) {
    NSLog("[VelaWV] prompt (uiDelegate fired) prefix=%@", String(prompt.prefix(20)))
    if prompt.hasPrefix("velawvbridge:") {
      processBridge(String(prompt.dropFirst("velawvbridge:".count)), in: webView)
      completionHandler(nil) // dismiss without showing a dialog
      return
    }
    completionHandler(defaultText)
  }

  /// Process one page→native `vela-1193` request envelope (from either channel).
  private func processBridge(_ raw: String, in wv: WKWebView?) {
    // JS-observable diagnostic on the ACTUAL firing webview (unambiguous even if
    // two view instances exist): __nativeGot>0 proves a bridge fired on the page.
    wv?.evaluateJavaScript("window.__nativeGot=(window.__nativeGot||0)+1;window.__hasCb=\(onProviderRequest != nil ? 1 : 0);", completionHandler: nil)
    guard let data = raw.data(using: .utf8),
          let obj = (try? JSONSerialization.jsonObject(with: data)) as? [String: Any],
          (obj["dir"] as? String) == "req" else { return } // ignore the "ready" ping etc.
    // TRUSTED origin from the committed main-frame URL — never from the payload.
    // The shim is injected forMainFrameOnly, so every message is a main-frame message.
    onProviderRequest?([
      "requestId": obj["id"] as? String ?? "",
      "method": obj["method"] as? String ?? "",
      "params": obj["params"] ?? [],
      "origin": currentOrigin(wv),
      "isMainFrame": true,
    ])
  }

  private func currentOrigin(_ wv: WKWebView?) -> String {
    guard let u = wv?.url, let scheme = u.scheme, let host = u.host else { return "" }
    if let port = u.port, !((scheme == "https" && port == 443) || (scheme == "http" && port == 80)) {
      return "\(scheme)://\(host):\(port)"
    }
    return "\(scheme)://\(host)"
  }

  // MARK: WKNavigationDelegate (navigation lifecycle)

  /// Reliable JS→native channel: intercept the custom bridge scheme the shim
  /// navigates a throwaway iframe to (the WKScriptMessageHandler doesn't fire
  /// under the RN New-Arch interop view).
  func webView(_ webView: WKWebView, decidePolicyFor navigationAction: WKNavigationAction, decisionHandler: @escaping (WKNavigationActionPolicy) -> Void) {
    if let url = navigationAction.request.url, url.scheme == "velawvbridge" {
      let full = url.absoluteString
      if let r = full.range(of: "velawvbridge://m/") {
        let enc = String(full[r.upperBound...])
        if let decoded = enc.removingPercentEncoding { processBridge(decoded, in: webView) }
      }
      decisionHandler(.cancel)
      return
    }
    decisionHandler(.allow)
  }

  func webView(_ webView: WKWebView, didCommit navigation: WKNavigation!) {
    NSLog("[VelaWV] didCommit (nav delegate fired) url=%@", webView.url?.absoluteString ?? "?")
    emitNavigation(loading: true)
  }
  func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) { emitNavigation(loading: false) }
  func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) { emitNavigation(loading: false) }
  func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) { emitNavigation(loading: false) }

  private func emitNavigation(loading: Bool) {
    guard let wv = webView else { return }
    onNavigationChange?([
      "url": wv.url?.absoluteString ?? "",
      "title": wv.title ?? "",
      "canGoBack": wv.canGoBack,
      "canGoForward": wv.canGoForward,
      "loading": loading,
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
  }
}
