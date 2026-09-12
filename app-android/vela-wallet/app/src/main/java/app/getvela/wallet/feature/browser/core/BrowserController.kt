package app.getvela.wallet.feature.browser.core

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.getvela.wallet.core.diagnostics.VelaLog
import uniffi.vela_core_uniffi.ExploreSitesCore
import uniffi.vela_core_uniffi.BrowserHistoryCore
import kotlinx.coroutines.CompletableDeferred
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.CoreHost
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
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
 * The in-app browser's owner (spec 044).
 *
 * The core owns the tabs, the favourites, the groups and the recents
 * (`explore_sites`, `browser_history`); this owns the ENGINES — one system
 * WebView per open tab with a URL — and the request sink the provider
 * bridge feeds. Phase 3 adds the permissions machine beside them.
 *
 * Found in phase 0: the history machine records nothing before its store
 * has answered, and its view has no "ready" flag — so the first visit waits
 * for the executor's answer, not for the view.
 */
class BrowserController(
    private val context: Context,
    private val scope: CoroutineScope,
    store: KeyValueStore,
    /** Debug builds expose the engines to Chrome DevTools — the device loop reads a page's own state through it. */
    debuggable: Boolean = false,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {
    init {
        if (debuggable) WebView.setWebContentsDebuggingEnabled(true)
    }

    private val historyLoaded = CompletableDeferred<Unit>()
    private val exploreExecutor = ExploreExecutor(store)
    private val bhistExecutor = BhistExecutor(store, onLoaded = { historyLoaded.complete(Unit) })

    private val exploreHost = CoreHost(
        bridge = ExploreSitesCore().asBridge(),
        scope = scope,
        initial = ExploreView(),
        serializer = ExploreView.serializer(),
        perform = JsonShell.perform(ExploreOperation.serializer(), ExploreShellResult.serializer(), exploreExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(ExploreOperation.serializer(), ExploreShellResult.serializer(), fallback = ExploreShellResult.Written, answer = exploreExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("browser.explore.fault", "core fault", error) },
    )

    private val bhistHost = CoreHost(
        bridge = BrowserHistoryCore().asBridge(),
        scope = scope,
        initial = BhistView(),
        serializer = BhistView.serializer(),
        perform = JsonShell.perform(BhistOperation.serializer(), BhistShellResult.serializer(), bhistExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(BhistOperation.serializer(), BhistShellResult.serializer(), fallback = BhistShellResult.Written, answer = bhistExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("browser.history.fault", "core fault", error) },
    )

    /** Favourites, groups, tabs — the core's. */
    val explore: StateFlow<ExploreView> = exploreHost.view

    /** Recents — the core's. */
    val history: StateFlow<BhistView> = bhistHost.view

    private val engines = HashMap<String, BrowserEngine>()
    private val _current = MutableStateFlow<BrowserEngine?>(null)

    /** The selected tab's engine, when that tab shows a page. */
    val current: StateFlow<BrowserEngine?> = _current

    private val _incoming = MutableSharedFlow<Pair<String, Incoming>>(extraBufferCapacity = 64)

    /** `(tab id, request)` — what a page asked, with the shell's facts attached. */
    val incoming: SharedFlow<Pair<String, Incoming>> = _incoming

    /** Set when something outside 探索 (a deep link, a dev seam) opened a page: the tab should show. */
    val openRequested = MutableStateFlow(false)

    private var started = false

    /** Loads the two documents once; every entry into 探索 calls it. */
    fun start() {
        if (started) return
        started = true
        exploreHost.dispatch(ExploreEvent.Start, ExploreEvent.serializer())
        bhistHost.dispatch(BhistEvent.Start, BhistEvent.serializer())
        scope.launch(Dispatchers.Main.immediate) {
            exploreHost.view.collect { view -> reconcile(view) }
        }
    }

    /** The selected tab gets an engine when it has a URL; closed tabs lose theirs. */
    private fun reconcile(view: ExploreView) {
        val alive = view.tabs.map { it.id }.toSet()
        engines.keys.filter { it !in alive }.forEach { id -> engines.remove(id)?.destroy() }
        val selected = view.tabs.firstOrNull { it.id == view.selected_tab } ?: view.tabs.firstOrNull()
        val engine = selected?.url?.let { url ->
            engines.getOrPut(selected.id) { newEngine(selected.id).also { it.load(url) } }
        }
        if (_current.value !== engine) _current.value = engine
    }

    private fun newEngine(tabId: String) = BrowserEngine(
        context = context,
        id = tabId,
        onIncoming = { request ->
            VelaLog.event("browser.request", "page asked", "method" to request.method, "url" to request.url.take(64))
            _incoming.tryEmit(tabId to request)
        },
        onNavigated = { url ->
            VelaLog.event("browser.nav", "document load", "url" to url.take(96))
            exploreHost.dispatch(ExploreEvent.TabNavigated(id = tabId, url = url, title = null), ExploreEvent.serializer())
        },
        onMeta = { url, title, favicon ->
            exploreHost.dispatch(ExploreEvent.TabNavigated(id = tabId, url = url, title = title.ifBlank { null }), ExploreEvent.serializer())
            scope.launch {
                historyLoaded.await()
                bhistHost.dispatch(BhistEvent.VisitRecorded(url = url, title = title.ifBlank { null }, favicon = favicon.ifBlank { null }, now_ms = now()), BhistEvent.serializer())
            }
        },
    )

    /** The address bar's text becomes a URL: a bare host gets `https://`. */
    fun coerceUrl(text: String): String {
        val trimmed = text.trim()
        return when {
            trimmed.isEmpty() -> ""
            trimmed.startsWith("http://") || trimmed.startsWith("https://") -> trimmed
            else -> "https://$trimmed"
        }
    }

    /**
     * Opens a page: in the selected tab when it already shows one, as the
     * selected start-page tab's first page otherwise, or in a new tab.
     */
    fun open(text: String, fromOutside: Boolean = false) {
        val url = coerceUrl(text)
        if (url.isEmpty()) return
        start()
        // The machine drops a mutation before its document has loaded
        // (device-found: an open from a deep link raced the load and no tab
        // appeared). Every intent below waits for `ready` first.
        scope.launch(Dispatchers.Main.immediate) {
            val view = exploreHost.view.first { it.ready }
            val selected = view.tabs.firstOrNull { it.id == view.selected_tab }
            when {
                selected != null && engines[selected.id] != null -> engines.getValue(selected.id).load(url)
                selected != null && selected.url == null ->
                    exploreHost.dispatch(ExploreEvent.TabNavigated(id = selected.id, url = url, title = null), ExploreEvent.serializer())
                else -> exploreHost.dispatch(ExploreEvent.TabOpened(url = url, title = null, now_ms = now()), ExploreEvent.serializer())
            }
            if (fromOutside) openRequested.value = true
        }
    }

    private fun whenReady(block: () -> Unit) {
        start()
        scope.launch(Dispatchers.Main.immediate) {
            exploreHost.view.first { it.ready }
            block()
        }
    }

    fun newTab() = whenReady { exploreHost.dispatch(ExploreEvent.TabOpened(url = null, title = null, now_ms = now()), ExploreEvent.serializer()) }
    fun selectTab(id: String) = exploreHost.dispatch(ExploreEvent.TabSelected(id), ExploreEvent.serializer())
    fun closeTab(id: String) = exploreHost.dispatch(ExploreEvent.TabClosed(id), ExploreEvent.serializer())
    fun closeAllTabs() = exploreHost.view.value.tabs.forEach { closeTab(it.id) }

    /** The page's close button: the tab goes, its engine with it. */
    fun close() {
        exploreHost.view.value.selected_tab?.let { closeTab(it) }
    }

    fun back() = _current.value?.back()
    fun forward() = _current.value?.forward()
    fun reload() = _current.value?.reload()

    fun addFavorite() {
        val state = _current.value?.state?.value ?: return
        if (state.url.isBlank()) return
        exploreHost.dispatch(ExploreEvent.FavoriteAdded(url = state.url, title = state.title.ifBlank { null }, now_ms = now()), ExploreEvent.serializer())
    }

    fun removeFavorite(origin: String) = exploreHost.dispatch(ExploreEvent.FavoriteRemoved(origin), ExploreEvent.serializer())
    fun createGroup(name: String) = exploreHost.dispatch(ExploreEvent.GroupCreated(name = name, now_ms = now()), ExploreEvent.serializer())
    fun deleteGroup(id: String) = exploreHost.dispatch(ExploreEvent.GroupDeleted(id), ExploreEvent.serializer())
    fun setGroupHidden(id: String, hidden: Boolean) = exploreHost.dispatch(ExploreEvent.GroupHiddenSet(id, hidden), ExploreEvent.serializer())
    fun setSystemGroupHidden(group: ExploreSystemGroup, hidden: Boolean) =
        exploreHost.dispatch(ExploreEvent.SystemGroupHiddenSet(group, hidden), ExploreEvent.serializer())
    fun clearRecent() = bhistHost.dispatch(BhistEvent.ClearAll, BhistEvent.serializer())
    fun deliver(tabId: String, json: String) = engines[tabId]?.deliver(json)
}
