package app.getvela.wallet.feature.browser.core

import android.annotation.SuppressLint
import android.content.Context
import android.graphics.Bitmap
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.getvela.wallet.core.diagnostics.VelaLog
import uniffi.vela_core_uniffi.DappPermissionsCore
import org.json.JSONObject
import org.json.JSONArray
import app.getvela.wallet.feature.wallet.core.RpcResult
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.wallet.core.FeedExecutor
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
    /** The person's own endpoints: a page's chain reads go through them (FR-005). */
    private val pool: RpcPool? = null,
    /** The feed's store: the "connected to" row (`type: "connect"`). */
    private val feed: FeedExecutor? = null,
    /** The relay: receipts for the user operations this browser submitted for pages. */
    private val relay: RelayClient? = null,
    /** The chains this wallet has — the settings machine's rows; a page may switch only to one of them. */
    private val knownChains: () -> List<Int> = { emptyList() },
    /** Debug builds expose the engines to Chrome DevTools — the device loop reads a page's own state through it. */
    debuggable: Boolean = false,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {
    init {
        if (debuggable) WebView.setWebContentsDebuggingEnabled(true)
    }

    // -- permissions (spec 044 phase 3) ----------------------------------------

    /** The chain the browser is on. The desktop starts on Gnosis too. */
    private val _browserChain = MutableStateFlow(100)
    val browserChain: StateFlow<Int> = _browserChain

    /** Which tab a request came from, so the answer goes back to it and nowhere else. */
    private val requestTab = HashMap<String, String>()

    /** Forwarded requests not yet answered: a navigation settles them with the core's error. */
    private val openIds = LinkedHashSet<String>()

    private val router: RequestRouter = RequestRouter(
        object : RequestRouter.Ports {
            override fun browserChain(): Int = _browserChain.value
            override fun knownChains(): List<Int> = this@BrowserController.knownChains()
            override fun switchChain(chainId: Int) {
                _browserChain.value = chainId
                dpermHost.dispatch(DpermEvent.ChainChanged(chainId), DpermEvent.serializer())
            }

            override suspend fun poolCall(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject? {
                val pool = pool ?: return null
                val list = (0 until params.length()).map { params.opt(it) }
                return (pool.call(chainId, method, list, if (bundler) RpcKind.Bundler else RpcKind.Rpc) as? RpcResult.Body)?.json
            }

            override fun respond(id: String, json: JSONObject) = answer(id, json)
            override fun sign(id: String, method: String, paramsJson: String, origin: String) {
                val handler = onSignRequest
                if (handler == null) {
                    answer(id, BrowserExecutor.errorJson(id, 4900, "Vela cannot answer $method yet"))
                    return
                }
                handler(SignRequest(id = id, method = method, paramsJson = paramsJson, origin = origin, transportId = requestTab[id] ?: _current.value?.id.orEmpty(), chainId = _browserChain.value))
            }

            override suspend fun receiptFor(userOpHash: String): RequestRouter.Receipt? {
                if (userOpHash.lowercase() !in knownOps) return null
                val relay = relay ?: return RequestRouter.Receipt.Pending
                return when (val receipt = relay.userOpReceipt(_browserChain.value, userOpHash)) {
                    is RelayClient.ReceiptAnswer.Resolved -> RequestRouter.Receipt.Landed(receipt.txHash)
                    else -> RequestRouter.Receipt.Pending
                }
            }
        },
    )

    /** A signature request with the shell's facts attached, for the signing controller (phase 4). */
    data class SignRequest(val id: String, val method: String, val paramsJson: String, val origin: String, val transportId: String, val chainId: Int)

    /** Phase 4 binds the signing controller here. */
    var onSignRequest: ((SignRequest) -> Unit)? = null

    /** User-operation hashes this browser answered pages with; their receipt lookups are translated. */
    private val knownOps = java.util.Collections.synchronizedSet(HashSet<String>())

    fun rememberUserOp(hash: String) { knownOps += hash.lowercase() }

    /** The signing controller's answer to a page, delivered to the tab that asked. */
    fun answerFromSigning(transportId: String, id: String, json: JSONObject) {
        openIds.remove(id)
        requestTab.remove(id)
        (engines[transportId] ?: _current.value)?.deliver(json.toString())
        VelaLog.event("browser.answer", "page answered by signing", "id" to id.take(12), "kind" to if (json.has("error")) "error:${json.getJSONObject("error").optInt("code")}" else "result")
    }

    private val browserExecutor: BrowserExecutor = BrowserExecutor(
        store = store,
        ports = object : BrowserExecutor.Ports {
            override fun respond(id: String, json: JSONObject) = answer(id, json)
            override fun emit(json: JSONObject) {
                _current.value?.deliver(json.toString())
            }

            override fun settleForwarded(code: Int, message: String) {
                val open = openIds.toList()
                openIds.clear()
                open.forEach { id -> answer(id, BrowserExecutor.errorJson(id, code, message)) }
            }

            override fun saveConnectionRecord(row: JSONObject) {
                val feed = feed ?: return
                scope.launch(Dispatchers.IO) { feed.writeRecords(listOf(row)) }
            }

            override fun forward(id: String, method: String, paramsJson: String, origin: String) {
                openIds += id
                scope.launch(Dispatchers.Main.immediate) {
                    router.route(id, method, paramsJson, origin)
                }
            }
        },
    )

    private val dpermHost: CoreHost<DpermView> = CoreHost(
        bridge = DappPermissionsCore().asBridge(),
        scope = scope,
        initial = DpermView(),
        serializer = DpermView.serializer(),
        perform = JsonShell.perform(DpermOperation.serializer(), DpermShellResult.serializer(), browserExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(DpermOperation.serializer(), DpermShellResult.serializer(), fallback = DpermShellResult.Ack, answer = browserExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("browser.perm.fault", "core fault", error) },
    )

    /** Consent, the connected address, the current origin — the core's. */
    val permissions: StateFlow<DpermView> = dpermHost.view

    /** An answer to the page that asked; the id is retired once answered. */
    private fun answer(id: String, json: JSONObject) {
        openIds.remove(id)
        val tabId = requestTab.remove(id)
        val engine = tabId?.let { engines[it] } ?: _current.value
        engine?.deliver(json.toString())
        VelaLog.event("browser.answer", "page answered", "id" to id.take(12), "kind" to if (json.has("error")) "error:${json.getJSONObject("error").optInt("code")}" else "result")
    }

    /** The session's accounts: told to the machine as the desktop tells it at birth, and on every change. */
    fun accountsChanged(addresses: List<String>, active: String?) {
        dpermHost.dispatch(DpermEvent.AccountsUpdated(addresses.takeIf { it.isNotEmpty() }), DpermEvent.serializer())
        if (!active.isNullOrBlank()) dpermHost.dispatch(DpermEvent.AccountSwitched(active, now()), DpermEvent.serializer())
        dpermHost.dispatch(DpermEvent.ChainChanged(_browserChain.value), DpermEvent.serializer())
    }

    fun consentApproved() = dpermHost.dispatch(DpermEvent.ConsentApproved(now()), DpermEvent.serializer())
    fun consentRejected() = dpermHost.dispatch(DpermEvent.ConsentRejected, DpermEvent.serializer())
    fun revoke(origin: String? = null) = dpermHost.dispatch(DpermEvent.RevokeRequested(origin ?: permissions.value.current_origin), DpermEvent.serializer())

    /** The document the machine judges requests against: the SELECTED tab's, told on every load and every tab switch. */
    private fun navigated(url: String) = dpermHost.dispatch(DpermEvent.NavigationStarted(url), DpermEvent.serializer())

    private val historyLoaded = CompletableDeferred<Unit>()
    private val exploreExecutor = ExploreExecutor(store)
    private val bhistExecutor = BhistExecutor(store, onLoaded = { historyLoaded.complete(Unit) })

    private val exploreHost: CoreHost<ExploreView> = CoreHost(
        bridge = ExploreSitesCore().asBridge(),
        scope = scope,
        initial = ExploreView(),
        serializer = ExploreView.serializer(),
        perform = JsonShell.perform(ExploreOperation.serializer(), ExploreShellResult.serializer(), exploreExecutor::perform),
        escapedFailure = JsonShell.escapedFailure(ExploreOperation.serializer(), ExploreShellResult.serializer(), fallback = ExploreShellResult.Written, answer = exploreExecutor::neutralAnswer),
        onFault = { error -> VelaLog.failure("browser.explore.fault", "core fault", error) },
    )

    private val bhistHost: CoreHost<BhistView> = CoreHost(
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
        if (_current.value !== engine) {
            _current.value = engine
            // The machine judges requests against the selected document. Told
            // here too, because it may have been born after the page loaded
            // (the desktop's second run-time finding).
            engine?.state?.value?.url?.takeIf { it.isNotBlank() }?.let { navigated(it) }
        }
    }

    private fun newEngine(tabId: String) = BrowserEngine(
        context = context,
        id = tabId,
        onIncoming = { request ->
            VelaLog.event("browser.request", "page asked", "method" to request.method, "url" to request.url.take(64))
            _incoming.tryEmit(tabId to request)
            requestTab[request.id] = tabId
            dpermHost.dispatch(
                DpermEvent.ProviderRequest(
                    id = request.id,
                    method = request.method,
                    params_json = request.paramsJson,
                    // The WEBVIEW's URL through the core's own origin rule — never the envelope's claim.
                    origin = dappOriginOf(request.url).orEmpty(),
                    // True by construction: the bridge posts only from the top frame.
                    is_main_frame = true,
                ),
                DpermEvent.serializer(),
            )
        },
        onNavigated = { url ->
            VelaLog.event("browser.nav", "document load", "url" to url.take(96))
            exploreHost.dispatch(ExploreEvent.TabNavigated(id = tabId, url = url, title = null), ExploreEvent.serializer())
            if (_current.value?.id == tabId) navigated(url)
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
