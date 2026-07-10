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
import com.facebook.react.uimanager.events.RCTEventEmitter
import org.json.JSONArray
import org.json.JSONObject

@SuppressLint("SetJavaScriptEnabled")
class WalletWebView(context: Context) : WebView(context) {

    private var injectedSource: String? = null // provider bundle (readiness signal)
    private var documentStartSupported = false // false → late-inject fallback on old WebView
    private var pendingUrl: String? = null
    private var isLoading = false
    private var lastFavicon = "" // resolved favicon URL for the current document
    private var lastSeq = 0 // highest processed outbox seq (native <- RN deliveries)

    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = true

        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView?, url: String?, favicon: Bitmap?) {
                isLoading = true
                lastFavicon = "" // new document — drop the previous page's favicon
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

            // http(s) loads here; every other scheme is handed to the OS (mailto:,
            // tel:, wc:, itms-apps:, app links) or dropped (javascript:/file:/data:).
            override fun shouldOverrideUrlLoading(view: WebView?, request: WebResourceRequest?): Boolean {
                val uri = request?.url ?: return false
                val scheme = uri.scheme?.lowercase() ?: ""
                if (scheme == "http" || scheme == "https" || scheme == "about") return false
                if (scheme != "javascript" && scheme != "file" && scheme != "data") {
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
                emitNavigation(false, error?.description?.toString() ?: "Failed to load")
            }

            // SPA client-side routing (history.pushState/replaceState) fires this but
            // NOT onPageStarted/Finished — refresh the URL bar + canGoBack for it.
            override fun doUpdateVisitedHistory(view: WebView?, url: String?, isReload: Boolean) {
                emitNavigation(isLoading)
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
        val obj = try { JSONObject(raw) } catch (e: Exception) { return }
        if (obj.optString("dir") != "req") return // ignore the "ready" ping etc.

        val payload: WritableMap = Arguments.createMap().apply {
            putString("requestId", obj.optString("id"))
            putString("method", obj.optString("method"))
            putArray("params", jsonArrayToWritable(obj.optJSONArray("params") ?: JSONArray()))
            putString("origin", sourceOrigin)       // TRUSTED — from the framework, not the page
            putBoolean("isMainFrame", isMainFrame)
        }
        emitEvent("onProviderRequest", payload)
    }

    private fun emitNavigation(loading: Boolean, error: String = "") {
        val payload = Arguments.createMap().apply {
            putString("url", url ?: "")
            putString("title", title ?: "")
            putBoolean("canGoBack", canGoBack())
            putBoolean("canGoForward", canGoForward())
            putBoolean("loading", loading)
            putString("error", error)
            putString("favicon", lastFavicon)
        }
        emitEvent("onNavigationChange", payload)
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
    }

    private fun emitEvent(name: String, payload: WritableMap) {
        val reactContext = context as ReactContext
        reactContext.getJSModule(RCTEventEmitter::class.java).receiveEvent(id, name, payload)
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
