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
import android.graphics.Bitmap
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

    private var providerScriptInstalled = false
    private var pendingUrl: String? = null
    private var lastSeq = 0 // highest processed outbox seq (native <- RN deliveries)

    init {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = true

        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView?, url: String?, favicon: Bitmap?) = emitNavigation(true)
            override fun onPageFinished(view: WebView?, url: String?) = emitNavigation(false)
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
        if (providerScriptInstalled || source.isEmpty()) return
        if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            WebViewCompat.addDocumentStartJavaScript(this, source, setOf("*"))
            providerScriptInstalled = true
        }
        loadIfReady()
    }

    fun setSource(url: String) {
        pendingUrl = url
        // Defer so the injectedJavaScript prop (same commit) registers before load.
        post { loadIfReady() }
    }

    private fun loadIfReady() {
        val url = pendingUrl ?: return
        if (!providerScriptInstalled) return
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

    private fun emitNavigation(loading: Boolean) {
        val payload = Arguments.createMap().apply {
            putString("url", url ?: "")
            putString("title", title ?: "")
            putBoolean("canGoBack", canGoBack())
            putBoolean("canGoForward", canGoForward())
            putBoolean("loading", loading)
        }
        emitEvent("onNavigationChange", payload)
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
