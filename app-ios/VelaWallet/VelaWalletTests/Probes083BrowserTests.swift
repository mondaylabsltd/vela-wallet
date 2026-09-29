//
//  Probes083BrowserTests.swift
//  VelaWalletTests
//
//  Spec 084 CHECK PASS (not a fix): probes that drive the REAL BrowserEngine
//  (a real WKWebView, hosted in the app on the simulator) for the browser
//  findings of 083 — H8, W2, W3, W4, W5, W6, W7, DNS, I-10, I-11, I-14 — and
//  write what they OBSERVED to the probe log. They assert nothing about the
//  product, only that the scenario ran.
//
//  Pages come from the Mac: 127.0.0.1:8000 (the test dApp, origin A), :8001
//  (origin B, the cross-origin iframe), :8002 (a slow page and a download).
//
//  "With a gesture" = the page's script is run through
//  WKWebView.evaluateJavaScript, which WebKit treats as user-initiated (the
//  same trust a tap gives); "without a gesture" = the same call made from a
//  setTimeout longer than the 1 s transient-activation window.
//

import Foundation
import Network
import Testing
import WebKit
import VelaCore
@testable import VelaWallet

private enum BProbe {
    static let path = "/private/tmp/claude-501/-Volumes-data-production-agent-2-vela-wallet/7177859b-46e8-4e29-be2f-0104b6530a2c/scratchpad/probe083-ios.log"
    static func log(_ id: String, _ text: String) {
        let line = "PROBE083|\(id)|\(text)\n"
        print(line, terminator: "")
        if let data = line.data(using: .utf8) {
            if let handle = FileHandle(forWritingAtPath: path) {
                handle.seekToEndOfFile(); handle.write(data); try? handle.close()
            } else {
                FileManager.default.createFile(atPath: path, contents: data)
            }
        }
    }
}

@MainActor
private func pause(_ seconds: Double) async {
    try? await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
}

/// Polls until `done` or `timeout` seconds; returns whether `done` held.
@MainActor
private func waitFor(_ timeout: Double, _ done: () -> Bool) async -> Bool {
    let deadline = Date().addingTimeInterval(timeout)
    while Date() < deadline {
        if done() { return true }
        await pause(0.02)
    }
    return done()
}

@MainActor
private func describe(_ engine: BrowserEngine) -> String {
    "url=\(engine.url) origin=\(engine.origin) host=\(engine.host) title=«\(engine.title)» failedURL=\(engine.failedURL) failure=\(engine.failure.map { "\($0.class)/\($0.reasonKey)" } ?? "nil") loading=\(engine.loading) retryPendingMs=\(String(describing: engine.retryPendingMs))"
}

// MARK: - Navigation recorder (I-W6, I-W7, I-10, I-11)

/// Stands in for the engine as WebKit's delegate to RECORD what WebKit hands
/// the shell (navigation type, frames), then applies the shell's OWN pure rule
/// (`BrowserEngine.policy`) so the outcome is what the shipped code decides —
/// without ever calling UIApplication.open for a hand-off.
@MainActor
final class NavRecorder: NSObject, WKNavigationDelegate, WKUIDelegate {
    let engine: BrowserEngine
    var events: [String] = []
    var finishes = 0
    var subFrame: WKFrameInfo?
    var createWebViewCalls = 0

    init(engine: BrowserEngine) {
        self.engine = engine
        super.init()
        engine.webView.navigationDelegate = self
        engine.webView.uiDelegate = self
    }

    nonisolated func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        let target = navigationAction.request.url
        let isMainFrame = navigationAction.targetFrame?.isMainFrame ?? true
        let policy = BrowserEngine.policy(
            scheme: target?.scheme, isMainFrame: isMainFrame,
            linkActivated: navigationAction.navigationType == .linkActivated
        )
        let source = navigationAction.sourceFrame
        let typeName: String
        switch navigationAction.navigationType {
        case .linkActivated: typeName = "linkActivated"
        case .formSubmitted: typeName = "formSubmitted"
        case .backForward: typeName = "backForward"
        case .reload: typeName = "reload"
        case .formResubmitted: typeName = "formResubmitted"
        case .other: typeName = "other"
        @unknown default: typeName = "unknown"
        }
        let line = "nav type=\(typeName) url=\(target?.absoluteString ?? "nil") targetFrame=\(navigationAction.targetFrame == nil ? "nil" : (isMainFrame ? "main" : "sub")) sourceFrame=\(source.isMainFrame ? "main" : "sub")@\(source.securityOrigin.host):\(source.securityOrigin.port) -> shellPolicy=\(policy)"
        MainActor.assumeIsolated {
            events.append(line)
            if let frame = navigationAction.targetFrame, !frame.isMainFrame { subFrame = frame }
        }
        decisionHandler(policy == .allow ? .allow : .cancel)
    }

    nonisolated func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        MainActor.assumeIsolated { finishes += 1 }
    }

    nonisolated func webView(
        _ webView: WKWebView,
        createWebViewWith configuration: WKWebViewConfiguration,
        for navigationAction: WKNavigationAction,
        windowFeatures: WKWindowFeatures
    ) -> WKWebView? {
        let source = navigationAction.sourceFrame
        let line = "createWebViewWith url=\(navigationAction.request.url?.absoluteString ?? "nil") targetFrame=\(navigationAction.targetFrame == nil ? "nil" : "some") sourceFrame=\(source.isMainFrame ? "main" : "sub")@\(source.securityOrigin.host):\(source.securityOrigin.port)"
        MainActor.assumeIsolated {
            events.append(line)
            createWebViewCalls += 1
        }
        // The shipped body, on the shipped engine.
        return MainActor.assumeIsolated {
            engine.webView(webView, createWebViewWith: configuration, for: navigationAction, windowFeatures: windowFeatures)
        }
    }
}

private let baseHTML = """
<!doctype html><meta charset="utf-8"><title>BASE</title>
<body>
<a id="mail" href="mailto:a@b.c">mail</a>
<a id="store" href="itms-apps://apps.apple.com/app/id284882215">store</a>
<a id="sms" href="sms:10086">sms</a>
<a id="short" href="shortcuts://">shortcuts</a>
<a id="tel" href="tel:10086">tel</a>
<a id="blank" target="_blank" href="http://127.0.0.1:8001/attacker.html">blank</a>
<iframe id="f" src="http://127.0.0.1:8001/frame-attack.html" style="width:300px;height:100px"></iframe>
</body>
"""

@MainActor
@Suite(.serialized)
struct Probes083Browser {

    private func eval(_ engine: BrowserEngine, _ js: String, frame: WKFrameInfo? = nil) async -> String {
        await withCheckedContinuation { continuation in
            engine.webView.evaluateJavaScript(js, in: frame, in: .page) { result in
                switch result {
                case .success(let value): continuation.resume(returning: "\(value ?? "nil")")
                case .failure(let error): continuation.resume(returning: "ERR \((error as NSError).localizedDescription)")
                }
            }
        }
    }

    /// Fresh engine + recorder showing baseHTML with its iframe loaded.
    private func freshPage() async -> (BrowserEngine, NavRecorder) {
        let engine = BrowserEngine(id: "nav-\(UUID().uuidString)")
        let rec = NavRecorder(engine: engine)
        engine.webView.loadHTMLString(baseHTML, baseURL: URL(string: "http://127.0.0.1:8000/"))
        _ = await waitFor(20) { rec.finishes >= 1 && rec.subFrame != nil }
        await pause(1.5)
        rec.events.removeAll()
        return (engine, rec)
    }

    private func settleAndReport(_ id: String, _ what: String, _ engine: BrowserEngine, _ rec: NavRecorder, extra: String = "") async {
        await pause(2.6)   // covers the 1.5 s timers plus the load
        let page = await eval(engine, "document.title + ' @ ' + location.href")
        BProbe.log(id, "\(what) | tab now=«\(page)» createWebViewCalls=\(rec.createWebViewCalls) \(extra)| events=\(rec.events)")
    }

    // MARK: I-W7 / I-10 : schemes, with and without a gesture

    @Test func iW7_schemesWithGesture() async {
        for (label, js) in [
            ("tapped mailto link (a.click via evaluateJavaScript)", "document.getElementById('mail').click()"),
            ("tapped itms-apps link", "document.getElementById('store').click()"),
            ("tapped shortcuts:// link", "document.getElementById('short').click()"),
            ("tapped tel: link", "document.getElementById('tel').click()"),
            ("tapped sms: link", "document.getElementById('sms').click()"),
            ("location.href='mailto:' (script)", "location.href='mailto:a@b.c'"),
            ("location.href='tel:10086' (script)", "location.href='tel:10086'"),
        ] {
            let (engine, rec) = await freshPage()
            _ = await eval(engine, js)
            await settleAndReport("I-W7g", label, engine, rec)
            engine.tearDown()
        }
    }

    @Test func iW7_i10_schemesWithoutGesture() async {
        for (label, js) in [
            ("NO GESTURE script a.click() on mailto", "setTimeout(()=>document.getElementById('mail').click(),1500)"),
            ("NO GESTURE script a.click() on itms-apps", "setTimeout(()=>document.getElementById('store').click(),1500)"),
            ("NO GESTURE script a.click() on sms", "setTimeout(()=>document.getElementById('sms').click(),1500)"),
            ("NO GESTURE script a.click() on tel", "setTimeout(()=>document.getElementById('tel').click(),1500)"),
            ("NO GESTURE created <a href=itms-apps> appended and clicked", "setTimeout(()=>{const a=document.createElement('a');a.href='itms-apps://apps.apple.com/app/id284882215';document.body.append(a);a.click()},1500)"),
            ("NO GESTURE location='mailto:'", "setTimeout(()=>{location.href='mailto:a@b.c'},1500)"),
        ] {
            let (engine, rec) = await freshPage()
            _ = await eval(engine, js)
            await settleAndReport("I-10", label, engine, rec)
            engine.tearDown()
        }
    }

    // MARK: I-W6 : target=_blank / window.open

    @Test func iW6_newWindowRequests() async {
        // with a gesture
        do {
            let (engine, rec) = await freshPage()
            _ = await eval(engine, "document.getElementById('blank').click()")
            await settleAndReport("I-W6g", "tapped target=_blank link", engine, rec)
            engine.tearDown()
        }
        do {
            let (engine, rec) = await freshPage()
            let result = await eval(engine, "String(window.open('http://127.0.0.1:8001/attacker.html'))")
            await settleAndReport("I-W6g", "window.open() with gesture returned «\(result)»", engine, rec)
            engine.tearDown()
        }
        // without a gesture
        do {
            let (engine, rec) = await freshPage()
            _ = await eval(engine, "window.__wo='pending'; setTimeout(()=>{window.__wo=String(window.open('http://127.0.0.1:8001/attacker.html'))},1500)")
            await pause(2.4)
            let result = await eval(engine, "window.__wo")
            await settleAndReport("I-W6n", "window.open() NO gesture returned «\(result)»", engine, rec)
            engine.tearDown()
        }
        do {
            let (engine, rec) = await freshPage()
            _ = await eval(engine, "setTimeout(()=>document.getElementById('blank').click(),1500)")
            await settleAndReport("I-W6n", "NO gesture a[target=_blank].click()", engine, rec)
            engine.tearDown()
        }
    }

    // MARK: I-11 : a cross-origin iframe

    @Test func i11_crossOriginIframe() async {
        for (label, sel, gesture) in [
            ("iframe TAPPED target=_blank -> http", "blank", true),
            ("iframe TAPPED target=_blank -> itms-apps", "store", true),
            ("iframe TAPPED mailto link", "mail", true),
            ("iframe NO GESTURE target=_blank -> http", "blank", false),
            ("iframe NO GESTURE target=_blank -> itms-apps", "store", false),
            ("iframe NO GESTURE mailto", "mail", false),
        ] {
            let (engine, rec) = await freshPage()
            guard let frame = rec.subFrame else {
                BProbe.log("I-11", "\(label) | no sub frame captured")
                engine.tearDown(); continue
            }
            let click = "document.getElementById('\(sel)').click()"
            _ = await eval(engine, gesture ? click : "setTimeout(()=>{\(click)},1500)", frame: frame)
            await settleAndReport("I-11", label, engine, rec)
            engine.tearDown()
        }
    }

    // MARK: I-W7 step 5 : downloads

    @Test func iW7_downloadsThroughTheRealEngine() async {
        for path in ["a.bin", "a.zip"] {
            let engine = BrowserEngine(id: "dl-\(UUID().uuidString)")
            engine.load("http://127.0.0.1:8000/")
            _ = await waitFor(20) { !engine.loading && !engine.title.isEmpty }
            BProbe.log("I-W7dl", "before: \(describe(engine))")
            engine.load("http://127.0.0.1:8002/\(path)")
            let failed = await waitFor(15) { engine.failure != nil }
            await pause(1.0)
            let shown = await eval(engine, "document.title + ' @ ' + location.href")
            BProbe.log("I-W7dl", "\(path) failed=\(failed) after load: \(describe(engine)) | webView.url=\(engine.webView.url?.absoluteString ?? "nil") | page actually showing=«\(shown)»")
            if failed {
                engine.reload()
                await pause(3.5)
                BProbe.log("I-W7dl", "\(path) after 重试 (reload): \(describe(engine)) retrying=\(engine.retrying)")
            }
            // Navigating away: does the dApp come back, or only the newly asked page?
            engine.load("http://127.0.0.1:8000/")
            _ = await waitFor(15) { engine.failure == nil && !engine.loading }
            BProbe.log("I-W7dl", "\(path) after navigating back to the dApp: \(describe(engine))")
            engine.tearDown()
        }
    }

    // MARK: I-H8 / I-W4 / I-DNS : failures on a USED tab and on a FRESH tab

    @Test func iH8_failedSecondNavigation() async {
        for target in ["https://expired.badssl.com/", "https://vela-083-nohost.invalid/"] {
            let engine = BrowserEngine(id: "h8-\(UUID().uuidString)")
            var metas: [String] = []
            engine.onMeta = { url, title in metas.append("\(url)|\(title)") }
            engine.load("http://127.0.0.1:8000/")
            _ = await waitFor(20) { !engine.loading && !engine.title.isEmpty }
            BProbe.log("I-H8", "used tab, page loaded: \(describe(engine))")
            metas.removeAll()
            engine.load(target)
            let failed = await waitFor(45) { engine.failure != nil }
            await pause(2.0)
            BProbe.log("I-H8", "after failed 2nd navigation to \(target) failed=\(failed): \(describe(engine)) | onMeta calls after the load=\(metas)")
            engine.tearDown()
        }
    }

    @Test func iW4_iDNS_freshTabFailures() async {
        for target in [
            "https://expired.badssl.com/", "https://self-signed.badssl.com/",
            "https://wrong.host.badssl.com/", "https://untrusted-root.badssl.com/",
            "https://vela-083-nohost.invalid/",
        ] {
            let engine = BrowserEngine(id: "fresh-\(UUID().uuidString)")
            engine.setOnScreen(true)
            let started = Date()
            engine.load(target)
            let failed = await waitFor(45) { engine.failure != nil }
            let took = Date().timeIntervalSince(started)
            await pause(0.6)
            BProbe.log("I-W4-DNS", "FRESH \(target) failed=\(failed) after \(String(format: "%.1f", took))s: \(describe(engine)) autoRetry=\(engine.failure.map { "\($0.autoRetry)" } ?? "?")")
            engine.tearDown()
        }
    }

    // MARK: I-W2 : the address the bar names during a slow load

    @Test func iW2_hostDuringASlowLoad() async {
        let engine = BrowserEngine(id: "w2-\(UUID().uuidString)")
        var metas: [String] = []
        engine.onMeta = { url, title in metas.append("\(url)|«\(title)»") }
        engine.load("http://127.0.0.1:8000/")
        _ = await waitFor(20) { !engine.loading && !engine.title.isEmpty }
        BProbe.log("I-W2", "settled on the dApp: \(describe(engine))")
        metas.removeAll()
        engine.load("http://127.0.0.1:8002/slow")
        await pause(0.3)
        BProbe.log("I-W2", "t+0.3s: \(describe(engine)) | onMeta=\(metas)")
        await pause(2.7)
        BProbe.log("I-W2", "t+3.0s: \(describe(engine)) | onMeta=\(metas)")
        _ = await waitFor(12) { engine.title == "SlowPage" }
        BProbe.log("I-W2", "after commit: \(describe(engine)) | onMeta=\(metas)")
        engine.tearDown()
    }

    // MARK: I-W5 : the renderer dies

    @Test func iW5_rendererCrash() async {
        let engine = BrowserEngine(id: "w5-\(UUID().uuidString)")
        var gone = 0
        engine.onRendererGone = { gone += 1 }
        engine.load("http://127.0.0.1:8000/")
        _ = await waitFor(20) { !engine.loading && !engine.title.isEmpty }
        let pid = (engine.webView.value(forKey: "_webProcessIdentifier") as? NSNumber)?.int32Value ?? 0
        BProbe.log("I-W5", "before: \(describe(engine)) webContentPid=\(pid)")
        if pid > 1 {
            kill(pid, SIGKILL)
            let saw = await waitFor(10) { gone > 0 }
            BProbe.log("I-W5", "onRendererGone fired=\(saw) count=\(gone) after: \(describe(engine))")
            engine.reload()
            _ = await waitFor(20) { !engine.loading && engine.title == "Vela test dApp" || engine.title != "" }
            let ok = await eval(engine, "typeof window.ethereum")
            BProbe.log("I-W5", "after reload: \(describe(engine)) typeof ethereum=\(ok)")
        }
        engine.tearDown()
    }

    // MARK: I-14 : a host that accepts and never answers

    @Test func i14_silentHostPanelDelay() async {
        let listener = try! NWListener(using: .tcp, on: .any)
        var held: [NWConnection] = []
        listener.newConnectionHandler = { connection in
            held.append(connection)
            connection.start(queue: .main)
        }
        var port: UInt16 = 0
        listener.stateUpdateHandler = { state in
            if case .ready = state { port = listener.port?.rawValue ?? 0 }
        }
        listener.start(queue: .main)
        _ = await waitFor(5) { port != 0 }
        let engine = BrowserEngine(id: "i14-\(UUID().uuidString)")
        engine.setOnScreen(true)
        let started = Date()
        engine.load("http://127.0.0.1:\(port)/")
        var firstPanel: Double?
        _ = await waitFor(95) {
            if engine.failure != nil, firstPanel == nil { firstPanel = Date().timeIntervalSince(started) }
            return firstPanel != nil
        }
        BProbe.log("I-14", "silent accepting host: panel first appeared after \(firstPanel.map { String(format: "%.1f", $0) } ?? "never (95 s)")s: \(describe(engine))")
        if firstPanel != nil {
            await pause(20)
            BProbe.log("I-14", "20 s later (retry schedule): \(describe(engine)) retrying=\(engine.retrying)")
        }
        engine.tearDown()
        listener.cancel()
        held.forEach { $0.cancel() }
    }
}


@MainActor
@Suite(.serialized)
struct Probes083Panels {

    /// The panel's words and the retry schedule on a REAL refused connection
    /// (the closest a hermetic run gets to the fault proxy's `drop`).
    @Test func iW3_refusedLoadRetriesAndPanelWords() async {
        let loc = Loc(overrideTag: "zh", preferredLanguages: [])
        let engine = BrowserEngine(id: "w3-\(UUID().uuidString)")
        engine.setOnScreen(true)
        let started = Date()
        engine.load("http://127.0.0.1:9/")
        var seen: [String] = []
        let end = Date().addingTimeInterval(22)
        var lastKey = ""
        while Date() < end {
            let key = "failure=\(engine.failure.map { "\($0.class)" } ?? "nil") pending=\(String(describing: engine.retryPendingMs)) retrying=\(engine.retrying) loading=\(engine.loading)"
            if key != lastKey {
                seen.append("t+\(String(format: "%.1f", Date().timeIntervalSince(started)))s \(key)")
                lastKey = key
            }
            await pause(0.05)
        }
        BProbe.log("I-W3", "closed port http://127.0.0.1:9/ : \(seen.joined(separator: " ; "))")
        BProbe.log("I-W3", "panel words: title=«\(loc.t("explore.loadFailed"))» offline=«\(loc.t("explore.loadOffline"))» notFound=«\(loc.t("explore.loadNotFound"))» certificate=«\(loc.t("explore.loadCertificate"))» retrying=«\(loc.t("explore.loadRetrying"))»")
        engine.tearDown()
    }
}
