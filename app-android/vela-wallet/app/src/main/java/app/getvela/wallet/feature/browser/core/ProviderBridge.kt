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

    private val ordinaryScript: String by lazy { dappProviderScript("android", false) }
    private val debugScript: String by lazy { dappProviderScript("android", true) }

    /**
     * The script for Settings' debug mode (spec 091): off, a secure context
     * only (spec 088); on, http on this device's own network too. Both are
     * the core's, built once each.
     */
    fun script(debugMode: Boolean): String = if (debugMode) debugScript else ordinaryScript

    /**
     * Installs the provider (document start, every frame — the script itself
     * does nothing below the top frame) for [debugMode], and the listener.
     * `null` when this WebView cannot run either: the page then loads with NO
     * wallet in it, which is honest, rather than with a provider injected too
     * late to be seen or on a channel that cannot tell frames apart.
     */
    fun install(
        webView: WebView,
        debugMode: Boolean,
        onMessage: (json: String, sourceOrigin: String, isMainFrame: Boolean) -> Unit,
    ): ProviderScript? {
        val documentStart = WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)
        val listener = WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)
        if (!documentStart || !listener) {
            VelaLog.event("browser.inject", "provider unavailable", "documentStart" to documentStart.toString(), "listener" to listener.toString())
            return null
        }
        // Called on the UI thread, in the order the page posted.
        WebViewCompat.addWebMessageListener(webView, HOST, setOf("*")) { _, message, sourceOrigin, isMainFrame, _ ->
            val data = message.data ?: return@addWebMessageListener
            onMessage(data, sourceOrigin.toString(), isMainFrame)
        }
        val provider = ProviderScript(debugMode) { script -> WebViewCompat.addDocumentStartJavaScript(webView, script, setOf("*"))::remove }
        VelaLog.event("browser.inject", "provider installed")
        return provider
    }

    /** Lazy: a JVM test reads [script] without a main looper. */
    private val main by lazy { android.os.Handler(android.os.Looper.getMainLooper()) }

    /**
     * A string for the page's `__velaDeliver`, which drops it unless it names
     * the page's own document.
     *
     * Through the main looper, NOT `webView.post`: a view that is not attached
     * to a window QUEUES what it is posted until it is attached again, so every
     * answer to a tab in the background waited for the person to bring that tab
     * back (device-found, spec 070 — a background page's `eth_chainId` never
     * came back).
     */
    fun deliver(webView: WebView, json: String) {
        main.post { webView.evaluateJavascript("window.__velaDeliver && window.__velaDeliver(${JSONObject.quote(json)})", null) }
    }
}

/**
 * The provider script one WebView carries (spec 091): the core's script for
 * the debug mode in force, swapped when the mode changes. A WebView reads its
 * document-start scripts when a document starts, so a swap applies to the
 * tab's NEXT document; the page already open keeps what it started with
 * (turning debug mode off still withdraws the wallet at once — the core
 * retires every document it no longer offers).
 *
 * [add] installs a document-start script and hands back how to remove it:
 * `WebViewCompat.addDocumentStartJavaScript`'s handler in the app, a fake in
 * the JVM tests.
 */
class ProviderScript(debugMode: Boolean, private val add: (script: String) -> () -> Unit) {
    var debugMode: Boolean = debugMode
        private set

    private var remove: () -> Unit = add(ProviderBridge.script(debugMode))

    fun swap(debugMode: Boolean) {
        if (debugMode == this.debugMode) return
        remove()
        remove = add(ProviderBridge.script(debugMode))
        this.debugMode = debugMode
    }
}
