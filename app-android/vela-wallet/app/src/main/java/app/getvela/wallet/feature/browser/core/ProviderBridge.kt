package app.getvela.wallet.feature.browser.core

import android.webkit.WebView
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONObject
import uniffi.vela_core_uniffi.dappProviderScript

/**
 * How a page reaches the wallet (spec 070).
 *
 * The script is the CORE's (`dapp_rpc::provider_script`): THE provider the
 * extension ships and the desktop and iOS inject, plus the one bridge. Nothing
 * here assembles it, strips module keywords or types a bridge — three shells
 * used to, each a little differently.
 *
 * The channel back is a `WebMessageListener`, not `addJavascriptInterface`:
 * the platform itself says which frame posted and from which origin. The
 * interface it replaced was visible to every frame and the shell stamped
 * every message with the TOP page's URL and "main frame" — so an ad iframe
 * could ask for a signature in the host dApp's name.
 */
object ProviderBridge {
    /** The bridge's object name, `VelaHost.postMessage(…)` in the page. */
    private const val HOST = "VelaHost"

    val script: String by lazy { dappProviderScript("android") }

    /**
     * Installs the provider (document start, every frame — the script itself
     * does nothing below the top frame) and the listener. `false` when this
     * WebView cannot run either: the page then loads with NO wallet in it,
     * which is honest, rather than with a provider injected too late to be
     * seen or on a channel that cannot tell frames apart.
     */
    fun install(webView: WebView, onMessage: (json: String, sourceOrigin: String, isMainFrame: Boolean) -> Unit): Boolean {
        val documentStart = WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)
        val listener = WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)
        if (!documentStart || !listener) {
            VelaLog.event("browser.inject", "provider unavailable", "documentStart" to documentStart.toString(), "listener" to listener.toString())
            return false
        }
        // Called on the UI thread, in the order the page posted.
        WebViewCompat.addWebMessageListener(webView, HOST, setOf("*")) { _, message, sourceOrigin, isMainFrame, _ ->
            val data = message.data ?: return@addWebMessageListener
            onMessage(data, sourceOrigin.toString(), isMainFrame)
        }
        WebViewCompat.addDocumentStartJavaScript(webView, script, setOf("*"))
        VelaLog.event("browser.inject", "provider installed")
        return true
    }

    /** A string for the page's `__velaDeliver`, which drops it unless it names the page's own document. */
    fun deliver(webView: WebView, json: String) {
        webView.post { webView.evaluateJavascript("window.__velaDeliver && window.__velaDeliver(${JSONObject.quote(json)})", null) }
    }
}
