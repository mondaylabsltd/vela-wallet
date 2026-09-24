package app.getvela.wallet.feature.browser.core

import android.annotation.SuppressLint
import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.content.MutableContextWrapper
import android.graphics.Bitmap
import android.net.Uri
import android.net.http.SslError
import android.webkit.JsPromptResult
import android.webkit.JsResult
import android.webkit.PermissionRequest
import android.webkit.RenderProcessGoneDetail
import android.webkit.SslErrorHandler
import android.webkit.WebChromeClient
import android.webkit.WebResourceError
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.getvela.wallet.core.crux.CoreHost
import app.getvela.wallet.core.crux.JsonShell
import app.getvela.wallet.core.crux.asBridge
import app.getvela.wallet.core.data.KeyValueStore
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.feature.send.core.RelayClient
import app.getvela.wallet.feature.signing.core.SignResponsePayload
import app.getvela.wallet.feature.wallet.core.FeedExecutor
import app.getvela.wallet.feature.wallet.core.RpcKind
import app.getvela.wallet.feature.wallet.core.RpcPool
import app.getvela.wallet.feature.wallet.core.RpcResult
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableSharedFlow
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.SharedFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.BrowserHistoryCore
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.ExploreSitesCore
import uniffi.vela_core_uniffi.dappBrowserInput
import uniffi.vela_core_uniffi.dappOriginOf

/** What the engine knows about the document it shows — the platform's facts, not the page's claims. */
data class EngineState(
    val url: String = "",
    val origin: String? = null,
    val host: String = "",
    val title: String = "",
    val canBack: Boolean = false,
    val canForward: Boolean = false,
    val loading: Boolean = false,
    /** 0–100 while loading. */
    val progress: Int = 0,
    /** The main frame failed to load (network, TLS): the page shows the retry panel instead. */
    val failed: Boolean = false,
    /** `false` when this WebView cannot carry the provider (no document-start script / message listener). */
    val wallet: Boolean = true,
)

/**
 * One tab's engine: a system `WebView` with the core's provider installed
 * (spec 044, rebuilt in 070). Owned by [BrowserController]; composed by
 * `BrowserPage`, which lends it the activity while it is on screen.
 */
@SuppressLint("SetJavaScriptEnabled")
class BrowserEngine(
    private val appContext: Context,
    val id: String,
    private val listener: Listener,
) {
    interface Listener {
        fun pageMessage(tab: String, json: String, sourceOrigin: String, isMainFrame: Boolean)
        fun navigationStarted(tab: String, url: String)
        fun loadFinished(tab: String, url: String, title: String)
        /** Same-document navigation (`pushState`) or a new title: what the tab shows, not a load. */
        fun shown(tab: String, url: String, title: String)
        fun rendererGone(tab: String)
        /** A main-frame link to a scheme this browser does not open itself, followed by a person's tap. */
        fun externalLink(tab: String, uri: Uri)
    }

    private val _state = MutableStateFlow(EngineState())
    val state: StateFlow<EngineState> = _state

    /**
     * The WebView's context is swapped for the activity while the page is on
     * screen (JavaScript dialogs need a window) and back to the application
     * when it leaves — an engine outlives any one activity.
     */
    private val context = MutableContextWrapper(appContext)
    private var attached = false

    /** The tab this engine shows. (Inside the WebView's own scope `id` is the VIEW's id.) */
    private val tabId: String = id

    val webView: WebView = WebView(context).apply {
        // MATCH_PARENT, and not for layout's sake: a WebView left at the
        // default WRAP_CONTENT gives Chromium no viewport HEIGHT, and every
        // `vh` unit on the page resolves to ZERO while `innerHeight` reports
        // the real number. Uniswap's connect sheet is
        // `max-height: calc(100vh - 72px)`, so it computed to 0px and the
        // sheet — listing this wallet, "已检测到" — collapsed to one pixel
        // behind its own scrim. The page looked frozen and the wallet looked
        // undetected (owner, 2026-09-23; found with 100vh/100dvh/100svh all
        // measuring 0 in the live page).
        layoutParams = android.view.ViewGroup.LayoutParams(
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
            android.view.ViewGroup.LayoutParams.MATCH_PARENT,
        )
        settings.javaScriptEnabled = true
        settings.domStorageEnabled = true
        settings.databaseEnabled = true
        settings.mediaPlaybackRequiresUserGesture = true
        // `target=_blank` and `window.open` load here, in the same tab.
        settings.setSupportMultipleWindows(false)
        val wallet = ProviderBridge.install(this) { json, origin, isMainFrame -> listener.pageMessage(tabId, json, origin, isMainFrame) }
        _state.value = _state.value.copy(wallet = wallet)
        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                update(url) { it.copy(loading = true, failed = false, progress = 0) }
                listener.navigationStarted(tabId, url)
            }

            override fun onPageFinished(view: WebView, url: String) {
                update(url) { it.copy(loading = false, progress = 100) }
                listener.loadFinished(tabId, url, view.title.orEmpty())
            }

            override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
                update(url) { it }
                listener.shown(tabId, url, view.title.orEmpty())
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (!request.isForMainFrame) return
                VelaLog.event("browser.load", "main frame failed", "code" to error.errorCode.toString())
                _state.value = _state.value.copy(failed = true, loading = false)
            }

            @SuppressLint("WebViewClientOnReceivedSslError")
            override fun onReceivedSslError(view: WebView, handler: SslErrorHandler, error: SslError) {
                // Never proceed: a page on a broken certificate is not the site it names.
                handler.cancel()
                if (error.url == view.url || view.url.isNullOrBlank()) _state.value = _state.value.copy(failed = true, loading = false)
            }

            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val scheme = request.url.scheme?.lowercase()
                if (scheme == "http" || scheme == "https") return false
                // Another scheme leaves this browser only for the page a person
                // is looking at, on a person's tap — never from a subframe or a
                // script: an ad must not be able to launch another app.
                if (request.isForMainFrame && request.hasGesture()) listener.externalLink(tabId, request.url)
                return true
            }

            override fun onRenderProcessGone(view: WebView, detail: RenderProcessGoneDetail): Boolean {
                VelaLog.event("browser.renderer", "renderer gone", "crashed" to detail.didCrash().toString())
                listener.rendererGone(tabId)
                // Handled: the controller destroys this WebView and the tab shows
                // a reload panel. Returning false would take the app down with it.
                return true
            }
        }
        webChromeClient = object : WebChromeClient() {
            override fun onProgressChanged(view: WebView, newProgress: Int) {
                _state.value = _state.value.copy(progress = newProgress)
            }

            override fun onReceivedTitle(view: WebView, title: String?) {
                _state.value = _state.value.copy(title = title.orEmpty())
                listener.shown(tabId, _state.value.url, title.orEmpty())
            }

            // A page off screen cannot put a dialog over the wallet.
            override fun onJsAlert(view: WebView, url: String, message: String, result: JsResult): Boolean = dialogOffScreen(result)
            override fun onJsConfirm(view: WebView, url: String, message: String, result: JsResult): Boolean = dialogOffScreen(result)
            override fun onJsPrompt(view: WebView, url: String, message: String, defaultValue: String?, result: JsPromptResult): Boolean =
                dialogOffScreen(result)

            // Camera, microphone, MIDI: this browser grants a page none of them.
            override fun onPermissionRequest(request: PermissionRequest) = request.deny()
        }
    }

    private fun dialogOffScreen(result: JsResult): Boolean {
        if (attached) return false
        result.cancel()
        return true
    }

    private fun update(url: String, change: (EngineState) -> EngineState) {
        val origin = dappOriginOf(url)
        _state.value = change(_state.value).copy(
            url = url,
            origin = origin,
            host = origin?.substringAfter("://")?.substringBefore('/') ?: "",
            canBack = webView.canGoBack(),
            canForward = webView.canGoForward(),
        )
    }

    /** On screen: dialogs get a window, timers and animations run. */
    fun attach(activity: Context) {
        context.baseContext = activity
        attached = true
        webView.onResume()
    }

    /** Off screen. The page keeps its state; nothing of it is painted or prompts. */
    fun detach() {
        attached = false
        context.baseContext = appContext
        webView.onPause()
    }

    fun load(url: String) = webView.loadUrl(url)
    fun back() { if (webView.canGoBack()) webView.goBack() }
    fun forward() { if (webView.canGoForward()) webView.goForward() }
    fun reload() {
        _state.value = _state.value.copy(failed = false)
        webView.reload()
    }
    fun deliver(json: String) = ProviderBridge.deliver(webView, json)
    fun destroy() {
        attached = false
        (webView.parent as? android.view.ViewGroup)?.removeView(webView)
        webView.stopLoading()
        webView.destroy()
    }
}

/**
 * The in-app browser's owner (spec 044, rebuilt on the core's `dapp_browser`
 * in 070).
 *
 * The core decides everything about a page's requests — which tab and which
 * document asked, what the method is, which chain the site is on, whether the
 * site may see an address or ask for a signature, and which answer goes back.
 * This owns the ENGINES (one WebView per open tab with a URL), feeds the core
 * what the platform observed, and performs what the core asks: store, deliver,
 * read, hand to the signing sheet.
 */
class BrowserController(
    private val context: Context,
    private val scope: CoroutineScope,
    store: KeyValueStore,
    /** The person's own endpoints: a page's chain reads go through them. */
    private val pool: RpcPool? = null,
    /** The feed's store: the "connected to" row (`type: "connect"`). */
    private val feed: FeedExecutor? = null,
    /** The relay: receipts for the user operations this browser submitted for pages. */
    private val relay: RelayClient? = null,
    /** Debug builds expose the engines to Chrome DevTools — the device loop reads a page's own state through it. */
    debuggable: Boolean = false,
    private val now: () -> Double = { System.currentTimeMillis().toDouble() },
) {
    init {
        if (debuggable) WebView.setWebContentsDebuggingEnabled(true)
    }

    /** Something the person should be told, once (a scheme this wallet does not speak, say). */
    sealed class Notice {
        data object WalletConnectUnsupported : Notice()
    }

    private val _notices = MutableSharedFlow<Notice>(extraBufferCapacity = 8)
    val notices: SharedFlow<Notice> = _notices

    /** Set by the container: open the signing sheet for a forwarded request. */
    var onForwardToSigning: ((DbrOperation.ForwardToSigning) -> Unit)? = null

    /** Set by the container: the page behind this request is gone — close its sheet. */
    var onCancelSigning: ((tab: String, id: String) -> Unit)? = null

    // -- the core ------------------------------------------------------------------

    private val executor = BrowserExecutor(
        store = store,
        ports = object : BrowserExecutor.Ports {
            override fun deliver(tab: String, messageJson: String) {
                engines[tab]?.deliver(messageJson)
            }

            override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): JSONObject? {
                val pool = pool ?: return null
                val list = (0 until params.length()).map { params.opt(it) }
                return (pool.call(chainId, method, list, if (bundler) RpcKind.Bundler else RpcKind.Rpc) as? RpcResult.Body)?.json
            }

            override suspend fun userOpTxHash(chainId: Int, userOpHash: String): String? =
                (relay?.userOpReceipt(chainId, userOpHash) as? RelayClient.ReceiptAnswer.Resolved)?.txHash

            override fun forwardToSigning(operation: DbrOperation.ForwardToSigning) {
                val open = onForwardToSigning
                if (open == null) {
                    signingAnswered(operation.tab, operation.id, SignResponsePayload.Err(4100, app.getvela.wallet.feature.signing.core.SignErrorKind.UnauthorizedAccount, "No wallet account available"), null)
                } else {
                    open(operation)
                }
            }

            override fun cancelSigning(tab: String, id: String) {
                onCancelSigning?.invoke(tab, id)
            }

            override fun saveConnectionRecord(row: JSONObject) {
                val feed = feed ?: return
                scope.launch(Dispatchers.IO) { feed.writeRecords(listOf(row)) }
            }
        },
    )

    private val dbrHost: CoreHost<DbrView> = CoreHost(
        bridge = DappBrowserCore().asBridge(),
        scope = scope,
        initial = DbrView(),
        serializer = DbrView.serializer(),
        perform = JsonShell.perform(DbrOperation.serializer(), DbrShellResult.serializer(), executor::perform),
        escapedFailure = JsonShell.escapedFailure(DbrOperation.serializer(), DbrShellResult.serializer(), fallback = DbrShellResult.Ack, answer = executor::neutralAnswer),
        onFault = { error -> VelaLog.failure("browser.dapp.fault", "core fault", error) },
    )

    /** Tabs, consent, connected sites, the signing line — the core's. */
    val dapp: StateFlow<DbrView> = dbrHost.view

    private fun dispatch(event: DbrEvent) = dbrHost.dispatch(event, DbrEvent.serializer())

    init {
        dispatch(DbrEvent.Start)
    }

    /** Every wallet address and the active one: the core re-pins every grant to it. */
    fun accountsChanged(addresses: List<String>, active: String?) {
        dispatch(DbrEvent.AccountsUpdated(addresses.takeIf { it.isNotEmpty() }))
        if (!active.isNullOrBlank()) dispatch(DbrEvent.AccountSwitched(active, now()))
    }

    /** The chains a site may switch or add to — the wallet's networks. */
    fun networksChanged(chainIds: List<Int>) = dispatch(DbrEvent.NetworksChanged(chainIds))

    /** The signing sheet's answer for a forwarded request — delivered by the core, exactly once. */
    fun signingAnswered(tab: String, id: String, payload: SignResponsePayload, userOpHash: String?) =
        dispatch(DbrEvent.SigningAnswered(tab = tab, id = id, payload = payload, user_op_hash = userOpHash))

    fun consentApproved() = dispatch(DbrEvent.ConsentApproved(now()))
    fun consentRejected() = dispatch(DbrEvent.ConsentRejected)

    /** Disconnect a site — the page in front's when `origin` is null. */
    fun revoke(origin: String? = null) {
        val target = origin ?: currentTabView()?.origin ?: return
        dispatch(DbrEvent.RevokeRequested(target))
    }

    /** Settings cleared every grant: the live core forgets them too, and open pages hear it. */
    fun revokeAll() = dispatch(DbrEvent.RevokeAll)

    /** The person picked a network for the page in front. */
    fun pickSiteChain(chainId: Int) {
        val origin = currentTabView()?.origin ?: return
        dispatch(DbrEvent.SiteChainPicked(origin, chainId))
    }

    /** The core's view of the tab in front. */
    fun currentTabView(): DbrTabView? {
        val selected = exploreHost.view.value.selected_tab ?: return null
        return dapp.value.tabs.firstOrNull { it.tab == selected }
    }

    // -- favourites, groups, tabs, recents --------------------------------------------

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

    // -- engines ------------------------------------------------------------------

    private val engines = HashMap<String, BrowserEngine>()
    private val _current = MutableStateFlow<BrowserEngine?>(null)

    /** The selected tab's engine, when that tab shows a live page. */
    val current: StateFlow<BrowserEngine?> = _current

    /**
     * Tabs whose renderer died. They get no new engine until the person asks
     * for a reload — a page that kills its renderer on load would otherwise
     * be recreated forever.
     */
    private val crashed = HashSet<String>()

    /** Set when something outside 探索 (a deep link, a dev seam) opened a page: the tab should show. */
    val openRequested = MutableStateFlow(false)

    private var started = false

    /** Loads the documents once; every entry into 探索 calls it. */
    fun start() {
        if (started) return
        started = true
        exploreHost.dispatch(ExploreEvent.Start, ExploreEvent.serializer())
        bhistHost.dispatch(BhistEvent.Start, BhistEvent.serializer())
        scope.launch(Dispatchers.Main.immediate) {
            exploreHost.view.collect { view -> reconcile(view) }
        }
    }

    /**
     * The selected tab gets an engine when it has a URL; closed tabs lose theirs
     * (and the core hears it, so their requests are settled); tabs not in front
     * are paused.
     */
    private fun reconcile(view: ExploreView) {
        val alive = view.tabs.map { it.id }.toSet()
        engines.keys.filter { it !in alive }.forEach { id ->
            engines.remove(id)?.destroy()
            dispatch(DbrEvent.TabClosed(id))
        }
        crashed.retainAll(alive)
        val selected = view.tabs.firstOrNull { it.id == view.selected_tab } ?: view.tabs.firstOrNull()
        val engine = selected?.url?.takeIf { selected.id !in crashed }?.let { url ->
            engines.getOrPut(selected.id) { newEngine(selected.id).also { it.load(url) } }
        }
        // A tab not in front keeps its page but runs no animations or media.
        engines.values.filter { it !== engine }.forEach { it.webView.onPause() }
        if (_current.value !== engine) _current.value = engine
    }

    private fun newEngine(tabId: String) = BrowserEngine(
        appContext = context.applicationContext,
        id = tabId,
        listener = object : BrowserEngine.Listener {
            override fun pageMessage(tab: String, json: String, sourceOrigin: String, isMainFrame: Boolean) =
                dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = sourceOrigin, is_main_frame = isMainFrame, message_json = json))

            override fun navigationStarted(tab: String, url: String) {
                VelaLog.event("browser.nav", "document load", "url" to url.take(96))
                dispatch(DbrEvent.NavigationStarted(tab, url))
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = null), ExploreEvent.serializer())
            }

            override fun loadFinished(tab: String, url: String, title: String) {
                dispatch(DbrEvent.LoadFinished(tab, url))
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = title.ifBlank { null }), ExploreEvent.serializer())
                // One visit per load, recorded when the load finishes and the
                // title is known — not once more for every title the page sets.
                val engine = engines[tab] ?: return
                engine.webView.evaluateJavascript(FAVICON_JS) { raw ->
                    val favicon = raw?.trim('"')?.takeIf { it.startsWith("https://") || it.startsWith("http://") }
                    scope.launch {
                        historyLoaded.await()
                        bhistHost.dispatch(BhistEvent.VisitRecorded(url = url, title = title.ifBlank { null }, favicon = favicon, now_ms = now()), BhistEvent.serializer())
                    }
                }
            }

            override fun shown(tab: String, url: String, title: String) {
                if (url.isBlank()) return
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = title.ifBlank { null }), ExploreEvent.serializer())
            }

            override fun rendererGone(tab: String) {
                crashed += tab
                engines.remove(tab)?.destroy()
                if (_current.value?.id == tab) _current.value = null
                dispatch(DbrEvent.RendererGone(tab))
            }

            override fun externalLink(tab: String, uri: Uri) = openExternal(uri)
        },
    )

    /**
     * A scheme the browser does not open itself, on a person's tap in the page
     * in front. `wc:` is said plainly — this wallet does not connect that way;
     * `intent:` is sanitised the way Chrome does (browsable, no component, no
     * selector) with its fallback URL loaded here; anything that reaches files
     * or scripts is never handed on.
     */
    private fun openExternal(uri: Uri) {
        when (uri.scheme?.lowercase()) {
            "wc" -> _notices.tryEmit(Notice.WalletConnectUnsupported)
            "javascript", "file", "content", "data", "blob", "about" -> Unit
            "intent" -> {
                val intent = runCatching { Intent.parseUri(uri.toString(), Intent.URI_INTENT_SCHEME) }.getOrNull() ?: return
                intent.addCategory(Intent.CATEGORY_BROWSABLE)
                intent.component = null
                intent.selector = null
                intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                try {
                    context.startActivity(intent)
                } catch (_: ActivityNotFoundException) {
                    intent.getStringExtra("browser_fallback_url")?.let { fallback -> dappBrowserInput(fallback)?.let { _current.value?.load(it) } }
                }
            }
            else -> try {
                context.startActivity(Intent(Intent.ACTION_VIEW, uri).addCategory(Intent.CATEGORY_BROWSABLE).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
            } catch (_: ActivityNotFoundException) {
                VelaLog.event("browser.external", "no app for scheme", "scheme" to uri.scheme.orEmpty())
            }
        }
    }

    /** Address-bar text → what loads: a URL, or a search (the core's rule, every shell's). */
    fun coerceUrl(text: String): String = dappBrowserInput(text).orEmpty()

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
                // A start-page tab gets its first page; a crashed one comes back
                // to life on the new address. Its engine is made HERE, not left to
                // the next view change — the same address twice changes nothing.
                selected != null && (selected.url == null || selected.id in crashed) -> {
                    crashed -= selected.id
                    exploreHost.dispatch(ExploreEvent.TabNavigated(id = selected.id, url = url, title = null), ExploreEvent.serializer())
                    val engine = newEngine(selected.id).also { engines[selected.id] = it }
                    engine.load(url)
                    _current.value = engine
                }
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

    /** Back inside the page; `false` when the page has no history left (the caller leaves the page). */
    fun back(): Boolean {
        val engine = _current.value ?: return false
        if (!engine.webView.canGoBack()) return false
        engine.back()
        return true
    }

    fun forward() = _current.value?.forward()

    /** The device pass's renderer death (debug builds only; `chrome://crash` kills the renderer on purpose). */
    fun debugCrashRenderer() {
        _current.value?.webView?.loadUrl("chrome://crash")
    }

    /** Reload the page in front — or, when its renderer died, bring the tab back to life. */
    fun reload() {
        val selected = exploreHost.view.value.let { view -> view.tabs.firstOrNull { it.id == view.selected_tab } } ?: return
        if (selected.id in crashed) {
            crashed -= selected.id
            reconcile(exploreHost.view.value)
        } else {
            _current.value?.reload()
        }
    }

    /** The star: pins the page in front, or unpins it when it already is a favourite. */
    fun toggleFavorite() {
        val state = _current.value?.state?.value ?: return
        if (state.url.isBlank()) return
        val origin = state.origin
        if (origin != null && exploreHost.view.value.favorites.any { it.origin == origin }) {
            removeFavorite(origin)
        } else {
            exploreHost.dispatch(ExploreEvent.FavoriteAdded(url = state.url, title = state.title.ifBlank { null }, now_ms = now()), ExploreEvent.serializer())
        }
    }

    fun removeFavorite(origin: String) = exploreHost.dispatch(ExploreEvent.FavoriteRemoved(origin), ExploreEvent.serializer())
    fun createGroup(name: String) = exploreHost.dispatch(ExploreEvent.GroupCreated(name = name, now_ms = now()), ExploreEvent.serializer())
    fun deleteGroup(id: String) = exploreHost.dispatch(ExploreEvent.GroupDeleted(id), ExploreEvent.serializer())
    fun setGroupHidden(id: String, hidden: Boolean) = exploreHost.dispatch(ExploreEvent.GroupHiddenSet(id, hidden), ExploreEvent.serializer())
    fun setSystemGroupHidden(group: ExploreSystemGroup, hidden: Boolean) =
        exploreHost.dispatch(ExploreEvent.SystemGroupHiddenSet(group, hidden), ExploreEvent.serializer())
    fun clearRecent() = bhistHost.dispatch(BhistEvent.ClearAll, BhistEvent.serializer())

    private companion object {
        /** The page's own icon link, absolute — the recents row shows it. */
        const val FAVICON_JS = "(function(){var l=document.querySelector(\"link[rel~='icon']\");return l?l.href:''})()"
    }
}
