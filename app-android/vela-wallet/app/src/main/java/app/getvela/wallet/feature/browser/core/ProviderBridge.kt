package app.getvela.wallet.feature.browser.core

import android.annotation.SuppressLint
import android.content.res.AssetManager
import android.webkit.JavascriptInterface
import android.webkit.WebView
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import app.getvela.wallet.core.diagnostics.VelaLog
import org.json.JSONObject

/** One EIP-1193 request, with the two facts the page cannot forge attached by this side of the boundary. */
class Incoming(
    val id: String,
    val method: String,
    val paramsJson: String,
    /** The WEBVIEW's own URL at the moment the envelope arrived — never the envelope's. */
    val url: String,
)

/**
 * The page-side provider and its bridge (spec 044 T011, research D2).
 *
 * The provider is the SAME two files the extension ships and the desktop
 * injects (`extension/lib/protocol.js` + `extension/inpage.js`), copied into
 * the APK's assets at build time. `inpage.js` is an ES module — it opens
 * with `import { CHANNEL, … } from './lib/protocol.js'` — and a
 * document-start script is classic, so the two module keywords are removed
 * HERE, exactly as the desktop's `provider_script()` does, and the pair is
 * wrapped in one scope so the constants reach the provider and nothing on
 * the page. Injected verbatim it would be a syntax error, and the failure is
 * silent: no wallet in the page, every request never happens.
 *
 * The bridge is the desktop's `BRIDGE_JS` with `window.ipc.postMessage`
 * replaced by the `VelaHost` interface, only from the top frame.
 */
object ProviderBridge {
    private const val CHANNEL = "vela-1193"

    /** The content script's half, in eleven lines. */
    private val BRIDGE_JS = """
(() => {
  if (window.top !== window) return;
  const CHANNEL = '$CHANNEL';
  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
    // The host adds the origin. Anything this envelope claims about who it
    // is would be the page describing itself.
    VelaHost.post(JSON.stringify({ id: d.id, method: d.method, params: d.params }));
  });
  window.__velaDeliver = (json) => {
    const m = JSON.parse(json);
    window.postMessage({ ch: CHANNEL, ...m }, window.location.origin);
  };
})();
""".trimIndent()

    /** The provider as ONE classic script, from the synced assets. */
    fun providerScript(assets: AssetManager): String {
        val constants = assets.open("provider/protocol.js").bufferedReader().readText()
            .lines().joinToString("\n") { it.removePrefix("export ") }
        val provider = assets.open("provider/inpage.js").bufferedReader().readText()
            .lines().filter { !it.trimStart().startsWith("import ") }.joinToString("\n")
        return "(() => {\n$constants\n$provider\n})();"
    }

    /** Both scripts, in the order the page needs them. */
    fun scripts(assets: AssetManager): List<String> = listOf(providerScript(assets), BRIDGE_JS)

    /** How the scripts reached the page: the answer the device pass records. */
    enum class InjectPath { DocumentStart, PageStarted }

    /**
     * Installs the provider before the page's own scripts run when the
     * engine can (`DOCUMENT_START_SCRIPT`, WebView ≥ 90); otherwise the
     * caller must inject at `onPageStarted`, which is later than some pages
     * look. Either way the path is logged and returned.
     */
    fun install(webView: WebView, scripts: List<String>): InjectPath {
        val path = if (WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) {
            scripts.forEach { WebViewCompat.addDocumentStartJavaScript(webView, it, setOf("*")) }
            InjectPath.DocumentStart
        } else {
            InjectPath.PageStarted
        }
        VelaLog.event("browser.inject", "provider installed", "path" to path.name)
        return path
    }

    /** The fallback path: run both scripts now (the page may already have looked). */
    fun injectNow(webView: WebView, scripts: List<String>) {
        scripts.forEach { webView.evaluateJavascript(it, null) }
    }

    /** An answer or an event, in the envelope the provider already listens for. */
    fun deliver(webView: WebView, json: String) {
        webView.post { webView.evaluateJavascript("window.__velaDeliver(${JSONObject.quote(json)})", null) }
    }

    /**
     * The `VelaHost` object the bridge calls. WebView invokes it on a
     * background thread; the sink hops to wherever it needs to be.
     */
    class HostInterface(private val webView: WebView, private val sink: (Incoming) -> Unit) {
        @JavascriptInterface
        fun post(json: String) {
            val envelope = runCatching { JSONObject(json) }.getOrNull() ?: return
            val id = envelope.optString("id").ifBlank { return }
            val method = envelope.optString("method").ifBlank { return }
            val params = envelope.opt("params")?.toString() ?: "[]"
            // Bounded before it reaches anything: a page can post a megabyte.
            if (params.length > 256 * 1024) return
            // The URL is read on the UI thread, from the engine, at the moment
            // of the request: a document-start script asks BEFORE
            // `onPageStarted` has told anyone where it is (device-found).
            webView.post {
                sink(Incoming(id = id, method = method, paramsJson = params, url = webView.url.orEmpty()))
            }
        }
    }

    @SuppressLint("JavascriptInterface")
    fun attach(webView: WebView, host: HostInterface) {
        webView.addJavascriptInterface(host, "VelaHost")
    }
}
