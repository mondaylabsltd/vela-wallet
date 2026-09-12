package app.getvela.wallet.feature.browser.core

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.getvela.wallet.core.diagnostics.VelaLog
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.launch
import uniffi.vela_core_uniffi.dappOriginOf

/** What the engine knows about the document it shows — the shell's facts, not the page's claims. */
data class EngineState(
    val url: String = "",
    val origin: String? = null,
    val host: String = "",
    val secure: Boolean = false,
    val title: String = "",
    val favicon: String = "",
    val canBack: Boolean = false,
    val canForward: Boolean = false,
    val loading: Boolean = false,
)

/**
 * One tab's engine: a system `WebView` with the shared provider installed
 * (spec 044). Owned by [BrowserController]; composed by `BrowserPage`.
 */
@SuppressLint("SetJavaScriptEnabled")
class BrowserEngine(
    context: Context,
    val id: String,
    private val onIncoming: (Incoming) -> Unit,
    private val onNavigated: (url: String) -> Unit,
    private val onMeta: (url: String, title: String, favicon: String) -> Unit,
) {
    private val _state = MutableStateFlow(EngineState())
    val state: StateFlow<EngineState> = _state
    private val scripts = ProviderBridge.scripts(context.assets)
    val injectPath: ProviderBridge.InjectPath

    val webView: WebView = WebView(context.applicationContext).apply {
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = true
        settings.setSupportMultipleWindows(false)
        injectPath = ProviderBridge.install(this, scripts)
        ProviderBridge.attach(this, ProviderBridge.HostInterface(webView = this, sink = onIncoming))
        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                if (injectPath == ProviderBridge.InjectPath.PageStarted) ProviderBridge.injectNow(view, scripts)
                update(url, loading = true)
                onNavigated(url)
            }

            override fun onPageFinished(view: WebView, url: String) {
                update(url, loading = false)
                onMeta(url, view.title.orEmpty(), _state.value.favicon)
            }

            override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
                update(url, loading = _state.value.loading)
            }

            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                // Only the two schemes the origin rule knows: anything else
                // (mailto:, intent:, app links) is not this browser's to open.
                val scheme = request.url.scheme
                return !(scheme == "http" || scheme == "https")
            }
        }
        webChromeClient = object : WebChromeClient() {
            override fun onReceivedTitle(view: WebView, title: String?) {
                _state.value = _state.value.copy(title = title.orEmpty())
                onMeta(_state.value.url, title.orEmpty(), _state.value.favicon)
            }

            override fun onReceivedIcon(view: WebView, icon: Bitmap?) = Unit
        }
    }

    private fun update(url: String, loading: Boolean) {
        val origin = dappOriginOf(url)
        _state.value = _state.value.copy(
            url = url,
            origin = origin,
            host = origin?.substringAfter("://")?.substringBefore('/') ?: "",
            secure = url.startsWith("https://"),
            canBack = webView.canGoBack(),
            canForward = webView.canGoForward(),
            loading = loading,
        )
    }

    fun load(url: String) = webView.loadUrl(url)
    fun back() { if (webView.canGoBack()) webView.goBack() }
    fun forward() { if (webView.canGoForward()) webView.goForward() }
    fun reload() = webView.reload()
    fun deliver(json: String) = ProviderBridge.deliver(webView, json)
    fun destroy() = webView.destroy()
}

/**
 * The in-app browser's owner (spec 044). Phase 1: one engine at a time, the
 * request sink, and the open-from-outside seam; phases 2–3 add the explore,
 * history and permissions machines around it.
 */
class BrowserController(
    private val context: Context,
    private val scope: CoroutineScope,
    /** Debug builds expose the engines to Chrome DevTools — the device loop reads a page's own state through it. */
    debuggable: Boolean = false,
) {
    init {
        if (debuggable) WebView.setWebContentsDebuggingEnabled(true)
    }

    private val _current = MutableStateFlow<BrowserEngine?>(null)
    val current: StateFlow<BrowserEngine?> = _current

    private val _incoming = MutableSharedFlow<Pair<String, Incoming>>(extraBufferCapacity = 64)
    /** `(tab id, request)` — what the page asked, with the shell's facts attached. */
    val incoming: SharedFlow<Pair<String, Incoming>> = _incoming

    /** Set when something outside 探索 (a deep link, a dev seam) opened a page: the tab should show. */
    val openRequested = MutableStateFlow(false)

    private var nextId = 1

    /** The address bar's text becomes a URL: a bare host gets `https://`. */
    fun coerceUrl(text: String): String {
        val trimmed = text.trim()
        return when {
            trimmed.isEmpty() -> ""
            trimmed.startsWith("http://") || trimmed.startsWith("https://") -> trimmed
            else -> "https://$trimmed"
        }
    }

    fun open(text: String, fromOutside: Boolean = false) {
        val url = coerceUrl(text)
        if (url.isEmpty()) return
        scope.launch(Dispatchers.Main.immediate) {
            val engine = _current.value ?: BrowserEngine(
                context = context,
                id = "tab-${nextId++}",
                onIncoming = { request ->
                    VelaLog.event("browser.request", "page asked", "method" to request.method, "url" to request.url.take(64))
                    _incoming.tryEmit(_current.value?.id.orEmpty() to request)
                },
                onNavigated = { navigated -> VelaLog.event("browser.nav", "document load", "url" to navigated.take(96)) },
                onMeta = { _, _, _ -> },
            ).also { _current.value = it }
            engine.load(url)
            if (fromOutside) openRequested.value = true
        }
    }

    fun close() {
        _current.value?.destroy()
        _current.value = null
    }

    fun back() = _current.value?.back()
    fun forward() = _current.value?.forward()
}
