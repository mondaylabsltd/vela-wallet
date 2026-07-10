// WalletWebView (Android) — a wallet-owned android.webkit.WebView for the in-app
// dApp browser. Android counterpart of the iOS WKWebView view.
//
// Responsibilities (see docs/dapp-browser/ARCHITECTURE.md):
//   • Inject the shared EIP-1193/6963 provider at DOCUMENT START, main frame only
//     (AndroidX WebViewCompat.addDocumentStartJavaScript) so window.ethereum is
//     present before page JS and immune to the page CSP.
//   • Receive provider requests on the "velaBridge" WebMessageListener, stamping
//     the TRUSTED sourceOrigin + isMainFrame the framework supplies — NEVER an
//     origin from the page payload.
//   • Deliver responses/events via window.__velaRespond / window.__velaEmit.
//   • Report navigation lifecycle for the URL bar + settle-on-navigation.
package com.velawallet.webview

import android.annotation.SuppressLint
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.webkit.JsPromptResult
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactContext
import com.facebook.react.bridge.WritableArray
import com.facebook.react.bridge.WritableMap
import com.facebook.react.uimanager.UIManagerHelper
import com.facebook.react.uimanager.events.Event
import org.json.JSONArray
import org.json.JSONObject

@SuppressLint("SetJavaScriptEnabled")
class WalletWebView(context: Context) : WebView(context) {

    private var injectedSource: String? = null // provider bundle (readiness signal)
    private var documentStartSupported = false // false → late-inject fallback on old WebView
    private var pendingUrl: String? = null
    private var isLoading = false
    private var lastFavicon = "" // resolved favicon URL for the current document
    private var lastError = "" // latched main-frame load error (cleared on next load start)
    private var lastSeq = 0 // highest processed outbox seq (native <- RN deliveries)

    init {
        // Debug builds only: expose this WebView to chrome://inspect so on-device
        // page geometry / provider issues are diagnosable (the dApp browser is the
        // least observable surface in the app). No-op in release.
        if ((context.applicationInfo.flags and android.content.pm.ApplicationInfo.FLAG_DEBUGGABLE) != 0) {
            setWebContentsDebuggingEnabled(true)
        }
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = true
        // In-app-browser viewport parity with react-native-webview's scalesPageToFit
        // default: honor the page's <meta viewport>. (Not related to the vh/dvh=0 bug
        // — that was a WRAP_CONTENT LayoutParams issue, fixed in onSizeChanged.)
        settings.useWideViewPort = true
        settings.loadWithOverviewMode = true
        // window.open / target=_blank: route the new-window request into THIS view
        // (single-tab), matching iOS createWebViewWith. Without this Android silently
        // no-ops window.open (returns null), breaking dApps that use it for OAuth /
        // WalletConnect deep links.
        settings.setSupportMultipleWindows(true)
        settings.javaScriptCanOpenWindowsAutomatically = true

        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView?, url: String?, favicon: Bitmap?) {
                isLoading = true
                lastFavicon = "" // new document — drop the previous page's favicon
                lastError = "" // new load — clear any prior error so the overlay hides
                // Fallback for pre-DOCUMENT_START_SCRIPT devices: inject as early as we
                // can (not before the first byte, but before most dApps read ethereum).
                if (!documentStartSupported) {
                    injectedSource?.let { view?.evaluateJavascript(it, null) }
                }
                emitNavigation(true)
            }

            override fun onPageFinished(view: WebView?, url: String?) {
                isLoading = false
                emitNavigation(false)
                resolveFavicon(view) // async — re-emits nav once the favicon is known
            }

            // http(s) loads here. Every other scheme is handed to the OS — but ONLY
            // for a real MAIN-FRAME navigation WITH a user gesture, so a hidden
            // cross-origin iframe or a programmatic redirect cannot silently launch
            // an external app or our own velawallet:// deep link. javascript:/file:/
            // data: are neither navigated nor opened.
            override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
                val uri = request?.url ?: return false
                val scheme = uri.scheme?.lowercase() ?: ""
                if (scheme == "http" || scheme == "https" || scheme == "about") return false
                if (scheme != "javascript" && scheme != "file" && scheme != "data" && scheme != "velawallet" &&
                    request.isForMainFrame && request.hasGesture()
                ) {
                    try {
                        context.startActivity(Intent(Intent.ACTION_VIEW, uri).apply {
                            addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        })
                    } catch (_: Exception) { /* no handler app — swallow */ }
                }
                return true // never navigate the WebView to a non-web scheme
            }

            override fun onReceivedError(view: WebView?, request: WebResourceRequest?, error: WebResourceError?) {
                if (request?.isForMainFrame != true) return // ignore sub-resource failures
                isLoading = false
                lastError = error?.description?.toString() ?: "Failed to load"
                emitNavigation(false)
            }

            // SPA client-side routing (history.pushState/replaceState) fires this but
            // NOT onPageStarted/Finished — refresh the URL bar + canGoBack for it.
            override fun doUpdateVisitedHistory(view: WebView?, url: String?, isReload: Boolean) {
                emitNavigation(isLoading)
            }
        }

        // window.open → load in this same view (single-tab browser).
        webChromeClient = object : android.webkit.WebChromeClient() {
            // Fallback page→native channel: on devices without WEB_MESSAGE_LISTENER
            // the shim calls window.prompt('velawvbridge:' + payload) — same channel
            // iOS uses (WKUIDelegate). Without this override the payload dies in the
            // un-handled prompt and every provider request silently hangs.
            override fun onJsPrompt(
                view: WebView?, url: String?, message: String?, defaultValue: String?, result: JsPromptResult?,
            ): Boolean {
                if (message != null && message.startsWith(BRIDGE_PROMPT_PREFIX)) {
                    // SECURITY: stamp the origin from the INITIATING frame's url (a
                    // framework value, not the payload). isMainFrame: the frame's
                    // origin must match the top document's — a same-origin iframe is
                    // the same principal; a cross-origin one stamps false and is
                    // rejected downstream (4100), matching the iOS prompt path.
                    val frameOrigin = originOf(url)
                    val topOrigin = originOf(this@WalletWebView.url)
                    onBridgePayload(
                        message.removePrefix(BRIDGE_PROMPT_PREFIX),
                        frameOrigin,
                        frameOrigin.isNotEmpty() && frameOrigin == topOrigin,
                    )
                    result?.cancel() // dismiss without showing a dialog (prompt → null)
                    return true
                }
                return super.onJsPrompt(view, url, message, defaultValue, result)
            }

            override fun onCreateWindow(
                view: WebView?, isDialog: Boolean, isUserGesture: Boolean, resultMsg: android.os.Message?,
            ): Boolean {
                val transport = resultMsg?.obj as? WebView.WebViewTransport ?: return false
                // A throwaway WebView that captures the target URL and loads it in the main view.
                val capture = WebView(context)
                capture.webViewClient = object : WebViewClient() {
                    override fun shouldOverrideUrlLoading(v: WebView?, req: WebResourceRequest?): Boolean {
                        req?.url?.let { pendingUrl = it.toString(); loadUrl(it.toString()) }
                        capture.destroy()
                        return true
                    }
                }
                transport.webView = capture
                resultMsg.sendToTarget()
                return true
            }
        }

        // page -> native: the injected shim calls window.velaBridge.postMessage(string).
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
            WebViewCompat.addWebMessageListener(
                this,
                "velaBridge",
                setOf("*"),
            ) { _, message, sourceOrigin, isMainFrame, _ ->
                onBridgeMessage(message, sourceOrigin?.toString() ?: "", isMainFrame)
            }
        }
    }

    // vh/dvh = 0 fix — a real on-device bug (verified via chrome://inspect against
    // Uniswap / PancakeSwap / 1inch / Sushi): every viewport-height CSS unit resolved
    // to 0 in this WebView, so any dApp bottom sheet positioned with dvh (a wallet
    // drawer, a cookie banner) slid entirely below the viewport and the user saw only
    // its dim scrim. ROOT CAUSE: RN gives this legacy view a default LayoutParams
    // height of WRAP_CONTENT, and Android WebView (AwContents) reads a wrap_content
    // height as "size to content" — which pins the viewport-percentage basis to 0
    // regardless of the EXACTLY measure spec, the correct laid-out height, or
    // window.innerHeight (all of which stay right). Pin the LayoutParams height to the
    // real laid-out size so the vh basis tracks the visible viewport. RN keeps driving
    // position via layout(); this only changes the height the WebView reports to
    // Chromium.
    override fun onSizeChanged(w: Int, h: Int, oldW: Int, oldH: Int) {
        super.onSizeChanged(w, h, oldW, oldH)
        if (w > 0 && h > 0) {
            val lp = layoutParams
            if (lp != null && lp.height != h) {
                lp.height = h
                layoutParams = lp
            }
            // First real size — run the initial load that loadIfReady held back until
            // the view had a viewport, so Chromium fixes the vh basis at the visible
            // height on the very first layout.
            loadIfReady()
        }
    }

    // --- props (from the view manager) --------------------------------------

    fun setInjectedJavaScriptSource(source: String) {
        if (injectedSource != null || source.isEmpty()) return
        injectedSource = source
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(this, source, setOf("*"))
            documentStartSupported = true
        }
        // Load regardless of feature support — an old WebView still browses (with the
        // onPageStarted late-inject fallback) instead of hanging on a blank screen.
        loadIfReady()
    }

    fun setSource(url: String) {
        pendingUrl = url
        // Defer so the injectedJavaScript prop (same commit) registers before load.
        post { loadIfReady() }
    }

    private fun loadIfReady() {
        val url = pendingUrl ?: return
        if (injectedSource == null) return // wait for the provider bundle prop
        // vh/dvh fix (part 2 — the load-time half). Chromium fixes the basis for
        // viewport-percentage units (vh/dvh/svh) when the main frame is FIRST laid
        // out, and never revises it. Under Fabric interop this view has size 0 at the
        // moment BrowserScreen mounts and sets the props, so loading now would freeze
        // the vh basis at 0 for this WebView's whole life (a reload does NOT reset it
        // — it's frame-level, not document-level). Wait for the first real size
        // (onSizeChanged → loadIfReady) so the basis is established at the visible
        // height and 100dvh resolves correctly.
        if (width <= 0 || height <= 0) return
        pendingUrl = null
        loadUrl(url)
    }

    /**
     * native -> page deliveries, as a seq-tracked JSON queue prop (parity with iOS).
     * Props flow reliably under the New-Arch interop layer; process items whose seq
     * exceeds the last one we ran.
     */
    fun setOutboxSource(json: String) {
        val arr = try { JSONArray(json) } catch (e: Exception) { return }
        for (i in 0 until arr.length()) {
            val item = arr.optJSONObject(i) ?: continue
            val seq = item.optInt("seq", -1)
            if (seq <= lastSeq) continue
            lastSeq = seq
            when (item.optString("t")) {
                "res" -> evaluateJavascript(
                    "window.__velaRespond && window.__velaRespond(" +
                        "${JSONObject.quote(item.optString("id"))}, ${jsLit(item.opt("result"))}, ${jsLit(item.opt("error"))});",
                    null,
                )
                "evt" -> evaluateJavascript(
                    "window.__velaEmit && window.__velaEmit(" +
                        "${JSONObject.quote(item.optString("event"))}, ${jsLit(item.opt("data"))});",
                    null,
                )
                "nav" -> when (item.optString("action")) {
                    "back" -> if (canGoBack()) goBack()
                    "forward" -> if (canGoForward()) goForward()
                    "reload" -> reload()
                }
            }
        }
    }

    private fun jsLit(v: Any?): String = when (v) {
        null, JSONObject.NULL -> "null"
        is JSONObject -> v.toString()
        is JSONArray -> v.toString()
        is String -> JSONObject.quote(v)
        is Boolean, is Int, is Long, is Double -> v.toString()
        else -> JSONObject.quote(v.toString())
    }

    // --- native -> page ------------------------------------------------------

    fun deliverResponse(id: String, resultJson: String, errorJson: String) {
        val idLit = JSONObject.quote(id)
        val resultLit = if (resultJson.isEmpty()) "undefined" else resultJson
        val errorLit = if (errorJson.isEmpty()) "null" else errorJson
        evaluateJavascript("window.__velaRespond && window.__velaRespond($idLit, $resultLit, $errorLit);", null)
    }

    fun deliverEvent(event: String, dataJson: String) {
        val eventLit = JSONObject.quote(event)
        val dataLit = if (dataJson.isEmpty()) "null" else dataJson
        evaluateJavascript("window.__velaEmit && window.__velaEmit($eventLit, $dataLit);", null)
    }

    // --- page -> native ------------------------------------------------------

    private fun onBridgeMessage(message: WebMessageCompat, sourceOrigin: String, isMainFrame: Boolean) {
        val raw = message.data ?: return
        onBridgePayload(raw, sourceOrigin, isMainFrame)
    }

    /** One page→native `vela-1193` request envelope, from either channel
     *  (WebMessageListener or the prompt fallback). `sourceOrigin`/`isMainFrame`
     *  are TRUSTED, framework-derived values — never read from the payload. */
    private fun onBridgePayload(raw: String, sourceOrigin: String, isMainFrame: Boolean) {
        val obj = try { JSONObject(raw) } catch (e: Exception) { return }
        if (obj.optString("dir") != "req") return // ignore the "ready" ping etc.

        val payload: WritableMap = Arguments.createMap().apply {
            putString("requestId", obj.optString("id"))
            putString("method", obj.optString("method"))
            putArray("params", jsonArrayToWritable(obj.optJSONArray("params") ?: JSONArray()))
            putString("origin", sourceOrigin)       // TRUSTED — from the framework, not the page
            putBoolean("isMainFrame", isMainFrame)
        }
        emitEvent(EVENT_PROVIDER_REQUEST, payload)
    }

    // `error` always reflects the LATCHED main-frame error: onReceivedError sets
    // lastError, and every later emit (onPageFinished / doUpdateVisitedHistory /
    // favicon) carries it forward instead of clobbering it with "" — otherwise the
    // guaranteed onPageFinished after a failed load would hide the error overlay.
    private fun emitNavigation(loading: Boolean) {
        val payload = Arguments.createMap().apply {
            putString("url", url ?: "")
            putString("title", title ?: "")
            putBoolean("canGoBack", canGoBack())
            putBoolean("canGoForward", canGoForward())
            putBoolean("loading", loading)
            putString("error", lastError)
            putString("favicon", lastFavicon)
        }
        emitEvent(EVENT_NAVIGATION_CHANGE, payload)
    }

    /** Resolve the page's declared favicon (absolute URL) and re-emit navigation. */
    private fun resolveFavicon(view: WebView?) {
        view?.evaluateJavascript(FAVICON_JS) { raw ->
            val href = try {
                if (raw.isNullOrEmpty() || raw == "null") "" else JSONArray("[$raw]").optString(0, "")
            } catch (e: Exception) {
                ""
            }
            if (href.isNotEmpty()) {
                lastFavicon = href
                emitNavigation(isLoading)
            }
        }
    }

    companion object {
        // Shared verbatim with the iOS view — reads the best-declared favicon as an
        // absolute URL (link.href resolves relatives), falling back to /favicon.ico.
        private const val FAVICON_JS =
            "(function(){try{var s=['link[rel~=\"icon\"]','link[rel=\"shortcut icon\"]'," +
                "'link[rel=\"apple-touch-icon\"]'];for(var i=0;i<s.length;i++){var l=document.querySelector(s[i]);" +
                "if(l&&l.href)return l.href;}return location.origin+'/favicon.ico';}catch(e){return '';}})()"

        private const val BRIDGE_PROMPT_PREFIX = "velawvbridge:"

        // Native emit names must be "top"-prefixed under Fabric/interop event
        // normalization (same convention react-native-webview relies on); the view
        // manager maps them to the JS registration names onProviderRequest /
        // onNavigationChange.
        const val EVENT_PROVIDER_REQUEST = "topProviderRequest"
        const val EVENT_NAVIGATION_CHANGE = "topNavigationChange"

        /** `scheme://host[:port]` of a URL string; "" when unparsable. */
        private fun originOf(url: String?): String {
            if (url.isNullOrEmpty()) return ""
            return try {
                val u = Uri.parse(url)
                val scheme = u.scheme ?: return ""
                val host = u.host ?: return ""
                val port = u.port
                val isDefault = port == -1 ||
                    (scheme == "https" && port == 443) || (scheme == "http" && port == 80)
                if (isDefault) "$scheme://$host" else "$scheme://$host:$port"
            } catch (_: Exception) {
                ""
            }
        }
    }

    /** A direct event carrying a WritableMap payload (onProviderRequest /
     *  onNavigationChange). Dispatched through UIManagerHelper — the supported
     *  path under RN bridgeless; the legacy RCTEventEmitter.receiveEvent hop is
     *  deprecated and slated to stop working when interop is disabled. */
    private class WalletWebViewEvent(
        surfaceId: Int,
        viewId: Int,
        private val name: String,
        private val payload: WritableMap,
    ) : Event<WalletWebViewEvent>(surfaceId, viewId) {
        override fun getEventName(): String = name
        override fun getEventData(): WritableMap = payload
    }

    private fun emitEvent(name: String, payload: WritableMap) {
        val reactContext = context as ReactContext
        val surfaceId = UIManagerHelper.getSurfaceId(this)
        UIManagerHelper.getEventDispatcherForReactTag(reactContext, id)
            ?.dispatchEvent(WalletWebViewEvent(surfaceId, id, name, payload))
    }

    // --- JSON -> RN writable conversion -------------------------------------

    private fun jsonArrayToWritable(arr: JSONArray): WritableArray {
        val out = Arguments.createArray()
        for (i in 0 until arr.length()) {
            when (val v = arr.get(i)) {
                is JSONObject -> out.pushMap(jsonObjectToWritable(v))
                is JSONArray -> out.pushArray(jsonArrayToWritable(v))
                is Boolean -> out.pushBoolean(v)
                is Int -> out.pushInt(v)
                is Long -> out.pushDouble(v.toDouble())
                is Double -> out.pushDouble(v)
                JSONObject.NULL -> out.pushNull()
                else -> out.pushString(v.toString())
            }
        }
        return out
    }

    private fun jsonObjectToWritable(obj: JSONObject): WritableMap {
        val out = Arguments.createMap()
        val keys = obj.keys()
        while (keys.hasNext()) {
            val k = keys.next()
            when (val v = obj.get(k)) {
                is JSONObject -> out.putMap(k, jsonObjectToWritable(v))
                is JSONArray -> out.putArray(k, jsonArrayToWritable(v))
                is Boolean -> out.putBoolean(k, v)
                is Int -> out.putInt(k, v)
                is Long -> out.putDouble(k, v.toDouble())
                is Double -> out.putDouble(k, v)
                JSONObject.NULL -> out.putNull(k)
                else -> out.putString(k, v.toString())
            }
        }
        return out
    }
}
