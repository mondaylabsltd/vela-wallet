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
import androidx.compose.ui.graphics.asImageBitmap
import kotlinx.coroutines.launch
import org.json.JSONArray
import org.json.JSONObject
import uniffi.vela_core_uniffi.BrowserHistoryCore
import uniffi.vela_core_uniffi.DappBrowserCore
import uniffi.vela_core_uniffi.ExploreSitesCore
import uniffi.vela_core_uniffi.dappBrowserInput
import uniffi.vela_core_uniffi.dappExternalPageHost
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
    /** Spec 079: why it failed — the core's class and the corpus key of its sentence. */
    val failure: uniffi.vela_core_uniffi.BrowserLoadFailure? = null,
    /** Spec 079: a retry of the failed page is running; the panel stays and says so. */
    val retrying: Boolean = false,
    /** `false` when this WebView cannot carry the provider (no document-start script / message listener). */
    val wallet: Boolean = true,
    /**
     * Spec 082 RE1: the committed document's URL — set by the commit callback
     * (`onPageStarted` of a load that did not fail, a same-document change),
     * never by a load merely asked for. `null` before anything committed.
     */
    val shown: String? = null,
    /** A load asked for (the person's, a retry, back/forward, a link the page followed) and not yet committed. */
    val pending: String? = null,
    /** The address whose failure panel is up. */
    val failedUrl: String? = null,
) {
    /**
     * What the address bar names — the core's rule (`browserAddressBar`): the
     * failed host with no lock while the panel is up, else the committed
     * document with its lock, else a pending load in an EMPTY tab with no
     * lock. A load under way never renames a tab that shows a document (the
     * page-initiated spoof), and nothing here guesses a host.
     */
    fun addressBar(): uniffi.vela_core_uniffi.BrowserAddressBar =
        uniffi.vela_core_uniffi.browserAddressBar(shown, pending, failedUrl.takeIf { failed })
}

/**
 * The load watchdog's one decision (spec 082 RE2), in the engine's units: a
 * load under way this long, not committed, with `WebView.getProgress()` at
 * [progressPercent], is given up — the core's rule; a slow site that is
 * getting somewhere is never cut.
 */
object LoadWatch {
    fun givesUp(elapsedMs: Long, committed: Boolean, progressPercent: Int): Boolean =
        uniffi.vela_core_uniffi.browserLoadShouldGiveUp(
            elapsedMs.coerceIn(0L, UInt.MAX_VALUE.toLong()).toUInt(),
            committed,
            progressPercent.coerceIn(0, 100) / 100.0,
        )

    /** How long a load may go before the watchdog asks. */
    fun giveUpMs(): Long = uniffi.vela_core_uniffi.browserLoadGiveUpMs().toLong()
}

/** Where the load hairline starts the moment a load is asked for (spec 079). */
private const val REQUESTED_PROGRESS = 10

/** A tab card's snapshot width — two cards to a row on a phone, sharp at 3×. */
private const val SNAPSHOT_WIDTH_PX = 360

/**
 * One tab's engine: a system `WebView` with the core's provider installed
 * (spec 044, rebuilt in 070) for Settings' debug mode (spec 091). Owned by
 * [BrowserController]; composed by `BrowserPage`, which lends it the activity
 * while it is on screen.
 */
@SuppressLint("SetJavaScriptEnabled")
class BrowserEngine(
    private val appContext: Context,
    val id: String,
    private val listener: Listener,
    debugMode: Boolean = false,
) {
    interface Listener {
        fun pageMessage(tab: String, json: String, sourceOrigin: String, isMainFrame: Boolean)
        fun navigationStarted(tab: String, url: String)
        /** [failed]: the main frame failed during this load (what finished is an engine error page). */
        fun loadFinished(tab: String, url: String, title: String, failed: Boolean, httpStatus: Int?)
        /** Same-document navigation (`pushState`) or a new title: what the tab shows, not a load. */
        fun shown(tab: String, url: String, title: String)
        fun rendererGone(tab: String)
        /** Spec 079: what the page looked like as it left the screen (the tab switcher's card); `null` clears it. */
        fun snapshot(tab: String, image: androidx.compose.ui.graphics.ImageBitmap?)
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

    // Spec 079 — one navigation's facts, and the retry schedule for a failed page.
    private val mainLooper = android.os.Handler(android.os.Looper.getMainLooper())
    /** This navigation hit a main-frame error: what finishes is the engine's error page, not the site. */
    private var navFailed = false
    /** The main document's HTTP status, when it was an error (the site's own 404 page is not a visit). */
    private var navHttpStatus: Int? = null
    private var retryAttempt = 0
    private var retryTask: Runnable? = null

    // Spec 082 RE2 — the load watchdog: armed at every load asked for,
    // disarmed by the commit, a failure, the finish or teardown.
    private var watchdog: Runnable? = null
    private var watchStartedAt = 0L
    /** The load being watched has committed a document. */
    private var committed = true
    /** The document before the last commit — restored when that commit was an error page. */
    private var previousShown: String? = null
    /** The load in flight is one the page started (a link, a script) — retried by hand only (RE3). */
    private var pageStarted = false

    /** The provider script this WebView carries; `null` when it cannot carry one. */
    private var provider: ProviderScript? = null

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
        provider = ProviderBridge.install(this, debugMode) { json, origin, isMainFrame -> listener.pageMessage(tabId, json, origin, isMainFrame) }
        _state.value = _state.value.copy(wallet = provider != null)
        webViewClient = object : WebViewClient() {
            override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                // A failed page's panel stays through its retry (spec 079): it
                // gives way only to a load that finishes without an error.
                navFailed = false
                navHttpStatus = null
                // The commit (RE1): what the bar names from now on — undone
                // below if this document turns out to be the engine's error page.
                committed = true
                disarmWatchdog()
                previousShown = _state.value.shown
                update(url) { it.copy(loading = true, progress = maxOf(it.progress.takeIf { _ -> it.loading } ?: 0, REQUESTED_PROGRESS), shown = url, pending = null) }
                listener.navigationStarted(tabId, url)
            }

            override fun onPageFinished(view: WebView, url: String) {
                disarmWatchdog()
                if (navFailed) {
                    update(url) { it.copy(loading = false, progress = 100, retrying = false) }
                } else {
                    retryAttempt = 0
                    cancelRetry()
                    update(url) { it.copy(loading = false, progress = 100, failed = false, failure = null, retrying = false, shown = url, pending = null, failedUrl = null) }
                }
                listener.loadFinished(tabId, url, view.title.orEmpty(), navFailed, navHttpStatus)
            }

            override fun onReceivedHttpError(view: WebView, request: WebResourceRequest, errorResponse: android.webkit.WebResourceResponse) {
                if (request.isForMainFrame) navHttpStatus = errorResponse.statusCode
            }

            override fun doUpdateVisitedHistory(view: WebView, url: String, isReload: Boolean) {
                // A same-document change (pushState) is the document's own; an
                // error page's history entry is not a document — nor is its
                // title ("网页无法打开") the tab's name (issue #329).
                update(url) { if (navFailed) it else it.copy(shown = url) }
                if (!navFailed) listener.shown(tabId, url, view.title.orEmpty())
            }

            override fun onReceivedError(view: WebView, request: WebResourceRequest, error: WebResourceError) {
                if (!request.isForMainFrame) return
                VelaLog.event("browser.load", "main frame failed", "code" to error.errorCode.toString())
                // The core's rule (spec 079): the class, its sentence, whether retrying helps.
                val failure = uniffi.vela_core_uniffi.browserLoadClassify("android", error.errorCode.toLong(), null, false) ?: return
                val url = request.url.toString()
                // The error page committed under the failing address: that is
                // not a document the person saw, so the bar goes back to theirs.
                if (_state.value.shown == url) _state.value = _state.value.copy(shown = previousShown)
                failed(failure, url)
            }

            @SuppressLint("WebViewClientOnReceivedSslError")
            override fun onReceivedSslError(view: WebView, handler: SslErrorHandler, error: SslError) {
                // Never proceed: a page on a broken certificate is not the site it names.
                handler.cancel()
                if (error.url == view.url || view.url.isNullOrBlank()) {
                    uniffi.vela_core_uniffi.browserLoadClassify("android", 0L, null, true)?.let { failed(it, error.url) }
                }
            }

            override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                val scheme = request.url.scheme?.lowercase()
                if (scheme == "http" || scheme == "https") {
                    // A link the page follows: progress from the tap, not from the commit.
                    if (request.isForMainFrame) requested(request.url.toString(), byPage = true)
                    return false
                }
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
                // The engine's error page has a title too ("网页无法打开"); it is not the page's.
                if (navFailed) return
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

    /**
     * The person or the page asked for a load (spec 079): progress shows now,
     * not when the engine commits — on a slow network the commit is seconds
     * away, and a tap that changes nothing reads as a tap that did nothing.
     * The address bar keeps the committed host until then.
     */
    private fun requested(url: String?, byPage: Boolean = false) {
        pageStarted = byPage
        _state.value = _state.value.copy(
            loading = true,
            progress = maxOf(if (_state.value.loading) _state.value.progress else 0, REQUESTED_PROGRESS),
            pending = url ?: _state.value.pending,
        )
        armWatchdog()
    }

    private fun failed(failure: uniffi.vela_core_uniffi.BrowserLoadFailure, url: String?) {
        navFailed = true
        disarmWatchdog()
        val state = _state.value
        _state.value = state.copy(
            failed = true, failure = failure, loading = false, retrying = false,
            failedUrl = url?.takeIf { it.isNotBlank() } ?: state.pending ?: state.failedUrl ?: state.url,
            pending = null,
        )
        scheduleRetry(failure)
    }

    /**
     * Spec 082 RE2: nothing may look frozen for a minute. At every load asked
     * for, the core's give-up time starts; the commit, a failure, the finish
     * or teardown stop it. Off screen it is not armed (a page in the
     * background is not being waited on); back on screen a load still under
     * way gets the full budget again.
     */
    private fun armWatchdog() {
        disarmWatchdog()
        committed = false
        if (!attached) return
        watchStartedAt = android.os.SystemClock.elapsedRealtime()
        val task = Runnable {
            watchdog = null
            giveUpIfStalled()
        }
        watchdog = task
        mainLooper.postDelayed(task, LoadWatch.giveUpMs())
    }

    private fun disarmWatchdog() {
        watchdog?.let(mainLooper::removeCallbacks)
        watchdog = null
    }

    private fun giveUpIfStalled() {
        val elapsed = android.os.SystemClock.elapsedRealtime() - watchStartedAt
        val progress = webView.progress
        val host = _state.value.pending?.let(::dappOriginOf)?.substringAfter("://").orEmpty()
        if (!LoadWatch.givesUp(elapsed, committed, progress)) {
            VelaLog.event("browser.load", "slow, still getting somewhere", "host" to host, "ms" to elapsed, "progress" to progress)
            return
        }
        VelaLog.event("browser.load", "stalled: given up", "host" to host, "ms" to elapsed, "progress" to progress)
        // What `stopLoading` finishes is not a page: the panel stays.
        navFailed = true
        webView.stopLoading()
        failed(uniffi.vela_core_uniffi.browserLoadStalled(), _state.value.pending)
    }

    /**
     * The core's schedule (2 s, 5 s, 10 s for the network classes; never for a
     * wrong name or certificate), and only while this tab is on screen — a
     * page in the background retries when it comes back.
     */
    private fun scheduleRetry(failure: uniffi.vela_core_uniffi.BrowserLoadFailure) {
        cancelRetry()
        if (!failure.autoRetry || !attached) return
        val wait = uniffi.vela_core_uniffi.browserLoadRetryDelayMs(failure.`class`, (retryAttempt + 1).toUInt()) ?: return
        val task = Runnable {
            retryTask = null
            retryAttempt += 1
            retry()
        }
        retryTask = task
        mainLooper.postDelayed(task, wait.toLong())
    }

    private fun cancelRetry() {
        retryTask?.let(mainLooper::removeCallbacks)
        retryTask = null
    }

    private fun retry() {
        _state.value = _state.value.copy(retrying = true)
        val again = _state.value.failedUrl
        requested(again, byPage = pageStarted)
        // A load the watchdog gave up on never reached the WebView's own
        // history (it still holds the page before, or nothing): ask for the
        // failed address itself — a reload would load the wrong page.
        if (again != null && webView.url != again) webView.loadUrl(again) else webView.reload()
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
        // A page that failed while off screen retries now it is looked at.
        _state.value.failure?.takeIf { retryTask == null && !_state.value.retrying }?.let(::scheduleRetry)
        // A load still under way gets the watchdog's full budget again.
        if (_state.value.loading && !committed && watchdog == null) armWatchdog()
    }

    /**
     * Spec 082 RE3: the calls reach a server again. A failure the network
     * explains is loaded again now (the core's `browserLoadRetryWhenNetworkReturns`),
     * with its count started over — but a load the page itself started is
     * the person's to retry.
     */
    fun networkCameBack() {
        val failure = _state.value.failure ?: return
        retryAttempt = 0
        if (!attached || pageStarted || _state.value.retrying) return
        if (!uniffi.vela_core_uniffi.browserLoadRetryWhenNetworkReturns(failure.`class`)) return
        VelaLog.event("browser.load", "network back: retrying", "class" to failure.`class`)
        cancelRetry()
        retry()
    }

    /** Off screen. The page keeps its state; nothing of it is painted or prompts. */
    fun detach() {
        attached = false
        cancelRetry()
        disarmWatchdog()
        listener.snapshot(tabId, capture())
        context.baseContext = appContext
        webView.onPause()
    }

    fun load(url: String) {
        // A new address: the old failure and its retries are over.
        cancelRetry()
        retryAttempt = 0
        _state.value = _state.value.copy(failed = false, failure = null, retrying = false, failedUrl = null)
        requested(url)
        webView.loadUrl(url)
    }

    /**
     * The site menu's Stop while a load runs (iOS spec 082 RE5): the load is
     * let go, the page that was showing stays, and nothing retries it.
     */
    fun stop() {
        if (!_state.value.loading) return
        cancelRetry()
        disarmWatchdog()
        committed = true
        webView.stopLoading()
        _state.value = _state.value.copy(loading = false, progress = 100, pending = null, retrying = false)
    }

    fun back() { if (webView.canGoBack()) { requested(historyUrl(-1)); webView.goBack() } }
    fun forward() { if (webView.canGoForward()) { requested(historyUrl(1)); webView.goForward() } }

    /** The address [step] entries away in this tab's history, if there is one. */
    private fun historyUrl(step: Int): String? = runCatching {
        val list = webView.copyBackForwardList()
        list.getItemAtIndex(list.currentIndex + step)?.url
    }.getOrNull()

    /** Reload — or, on a failed page, the person's retry: the panel stays, saying so, and the count starts again. */
    fun reload() {
        cancelRetry()
        if (_state.value.failure != null) {
            retryAttempt = 0
            retry()
        } else {
            requested(_state.value.shown)
            webView.reload()
        }
    }
    /**
     * The page as it was last seen, scaled to a card (spec 079 — the switcher
     * drew the same grey bars for every tab). Nothing for a failed page — what
     * the WebView holds then is the engine's error page, which the person
     * never saw behind the Vela panel.
     */
    private fun capture(): androidx.compose.ui.graphics.ImageBitmap? {
        val width = webView.width
        val height = webView.height
        if (width <= 0 || height <= 0 || _state.value.failure != null || _state.value.url.isBlank()) return null
        val scale = SNAPSHOT_WIDTH_PX.toFloat() / width
        return runCatching {
            val bitmap = android.graphics.Bitmap.createBitmap(SNAPSHOT_WIDTH_PX, (height * scale).toInt().coerceAtLeast(1), android.graphics.Bitmap.Config.ARGB_8888)
            val canvas = android.graphics.Canvas(bitmap)
            canvas.scale(scale, scale)
            // The WebView scrolls itself: its draw starts at the document's top.
            canvas.translate(-webView.scrollX.toFloat(), -webView.scrollY.toFloat())
            webView.draw(canvas)
            bitmap.asImageBitmap()
        }.getOrNull()
    }

    fun deliver(json: String) = ProviderBridge.deliver(webView, json)

    private var visualRequest = 0L

    /**
     * [then], on the main thread, once the page's content as it is now is
     * ready for the WebView's next draw (`postVisualStateCallback`) — how the
     * page view knows a WebView put back on screen has something to show.
     * `false` when this WebView cannot say (then is never called).
     */
    fun whenDrawn(then: () -> Unit): Boolean {
        if (!androidx.webkit.WebViewFeature.isFeatureSupported(androidx.webkit.WebViewFeature.VISUAL_STATE_CALLBACK)) return false
        visualRequest += 1
        androidx.webkit.WebViewCompat.postVisualStateCallback(webView, visualRequest) { then() }
        return true
    }

    /** Settings' debug mode changed (spec 091): this tab's next document gets the script for it. */
    fun debugModeChanged(on: Boolean) {
        provider?.swap(on)
    }

    fun destroy() {
        attached = false
        cancelRetry()
        disarmWatchdog()
        (webView.parent as? android.view.ViewGroup)?.removeView(webView)
        webView.stopLoading()
        webView.destroy()
    }
}

/**
 * Settings' debug mode in the in-app browser (spec 091), kept by
 * [BrowserController]. The core's page gate hears it at start and on every
 * change (`debug_mode_changed`); every open engine swaps its script
 * ([ProviderScript]) and a new engine starts with [on].
 *
 * A WebView reads its script when a document starts, so the new script
 * applies to each tab's NEXT document: turned on, a LAN page already open has
 * no provider until it loads again. Turned off, the wallet is withdrawn at
 * once all the same — the core retires every open document it no longer
 * offers (its requests answered 4900, a sheet showing one closed).
 */
class BrowserDebugMode(
    initial: Boolean,
    private val dispatch: (DbrEvent) -> Unit,
    private val swapScripts: (on: Boolean) -> Unit,
) {
    var on: Boolean = initial
        private set

    init {
        dispatch(DbrEvent.DebugModeChanged(initial))
    }

    fun set(on: Boolean) {
        if (on == this.on) return
        this.on = on
        dispatch(DbrEvent.DebugModeChanged(on))
        swapScripts(on)
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
    /** Settings' debug mode at start (spec 091); [debugModeChanged] follows it after. */
    debugMode: Boolean = false,
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

    /** Which chains the pool could not reach on its last try (spec 079 — the page's chain notice reads it). */
    val poolView: StateFlow<app.getvela.wallet.feature.wallet.core.RpcPoolView> =
        pool?.view ?: MutableStateFlow(app.getvela.wallet.feature.wallet.core.RpcPoolView())

    private val _chainAsking = MutableStateFlow(false)

    /** A chain-notice retry is in flight. */
    val chainAsking: StateFlow<Boolean> = _chainAsking

    /**
     * The chain notice's Retry: one block-number read through the pool. Its
     * answer is the pool's to judge — a usable answer clears the chain's
     * failure and the notice goes with it; nothing here decides that.
     */
    fun askChain(chainId: Int) {
        val pool = pool ?: return
        if (_chainAsking.value) return
        _chainAsking.value = true
        scope.launch {
            try {
                pool.call(chainId, "eth_blockNumber")
            } finally {
                _chainAsking.value = false
            }
        }
    }

    /** Set by the container: open the signing sheet for a forwarded request. */
    var onForwardToSigning: ((DbrOperation.ForwardToSigning) -> Unit)? = null

    /** Set by the container: the page behind this request is gone — close its sheet. */
    var onCancelSigning: ((tab: String, id: String) -> Unit)? = null

    /**
     * Spec 100: set by the container — carry a page's add-network request to
     * `network_admin` (`dapp_add_requested`). Unset, the request is answered
     * "declined" here: nobody can show the sheet.
     */
    var onForwardToAddNetwork: ((DbrOperation.ForwardToAddNetwork) -> Unit)? = null

    /** Spec 100: set by the container — the page behind an add-network request is gone (`dapp_add_cancelled`). */
    var onCancelAddNetwork: ((tab: String, id: String) -> Unit)? = null

    // -- the core ------------------------------------------------------------------

    private val executor = BrowserExecutor(
        store = store,
        now = now,
        ports = object : BrowserExecutor.Ports {
            override fun deliver(tab: String, messageJson: String) {
                engines[tab]?.deliver(messageJson)
            }

            override suspend fun read(chainId: Int, method: String, params: JSONArray, bundler: Boolean): BrowserExecutor.ReadAnswer {
                val pool = pool ?: return BrowserExecutor.ReadAnswer.NoAnswer()
                val list = (0 until params.length()).map { params.opt(it) }
                // The pool's own word on why nothing answered (spec 099 FR-009):
                // every endpoint rate-limiting is said as such, never as "down".
                return when (val result = pool.call(chainId, method, list, if (bundler) RpcKind.Bundler else RpcKind.Rpc)) {
                    is RpcResult.Body -> BrowserExecutor.ReadAnswer.Body(result.json)
                    is RpcResult.Failed -> BrowserExecutor.ReadAnswer.NoAnswer(rateLimited = result.rateLimited)
                    is RpcResult.RangeCapped -> BrowserExecutor.ReadAnswer.NoAnswer()
                }
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

            override fun forwardToAddNetwork(operation: DbrOperation.ForwardToAddNetwork) {
                val open = onForwardToAddNetwork
                if (open == null) {
                    addNetworkAnswered(operation.tab, operation.id, DappAddOutcome.Declined)
                } else {
                    open(operation)
                }
            }

            override fun cancelAddNetwork(tab: String, id: String) {
                onCancelAddNetwork?.invoke(tab, id)
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

    /** Settings' debug mode (spec 091) — stated to the core now, before any page message can matter. */
    private val debug = BrowserDebugMode(debugMode, dispatch = { dispatch(it) }) { on ->
        engines.values.forEach { it.debugModeChanged(on) }
    }

    /** Settings' debug mode changed (spec 091). */
    fun debugModeChanged(on: Boolean) = debug.set(on)

    /** Every wallet address and the active one: the core re-pins every grant to it. */
    fun accountsChanged(addresses: List<String>, active: String?) {
        dispatch(DbrEvent.AccountsUpdated(addresses.takeIf { it.isNotEmpty() }))
        if (!active.isNullOrBlank()) dispatch(DbrEvent.AccountSwitched(active, now()))
    }

    /** The chains a site may switch or add to — the wallet's networks. */
    fun networksChanged(chainIds: List<Int>) = dispatch(DbrEvent.NetworksChanged(chainIds))

    /** The signing sheet's answer for a forwarded request — delivered by the core, exactly once. */
    fun signingAnswered(tab: String, id: String, payload: SignResponsePayload, userOpHash: String?) =
        dispatch(DbrEvent.SigningAnswered(tab = tab, id = id, payload = payload, user_op_hash = userOpHash, now_ms = now()))

    /** Spec 100: the add-network sheet's ending for a forwarded request — the core answers the page, exactly once. */
    fun addNetworkAnswered(tab: String, id: String, outcome: DappAddOutcome) =
        dispatch(DbrEvent.AddNetworkAnswered(tab = tab, id = id, outcome = outcome, now_ms = now()))

    fun consentApproved() = dispatch(DbrEvent.ConsentApproved(now()))
    fun consentRejected() = dispatch(DbrEvent.ConsentRejected(now()))

    // -- spec 099 FR-014: the tab's status entry and its panel ----------------------

    /** The status panel of [tab] opened: the core's view carries that tab's whole record until it closes. */
    fun inspectorOpened(tab: String) = dispatch(DbrEvent.InspectorOpened(tab))

    fun inspectorClosed() = dispatch(DbrEvent.InspectorClosed)

    private val _statusSeen = MutableStateFlow<Map<String, String>>(emptyMap())

    /** Per tab, the status line last put away (its ✕, or its Details) — gone until it says something else. */
    val statusSeen: StateFlow<Map<String, String>> = _statusSeen

    fun putStatusAway(tab: String, seen: String) {
        _statusSeen.value = _statusSeen.value + (tab to seen)
    }

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

    private val _snapshots = MutableStateFlow<Map<String, androidx.compose.ui.graphics.ImageBitmap>>(emptyMap())

    /** Spec 079: each tab's page as it last left the screen, for the switcher's cards. */
    val snapshots: StateFlow<Map<String, androidx.compose.ui.graphics.ImageBitmap>> = _snapshots

    /**
     * Tabs whose renderer died. They get no new engine until the person asks
     * for a reload — a page that kills its renderer on load would otherwise
     * be recreated forever.
     */
    private val crashed = HashSet<String>()

    /**
     * Each tab's last document that loaded WITHOUT failing — the core's visit
     * (`browserLoadVisit`), read from the document itself. What the star
     * names a favourite after (`browserPinnedTitle`, issue #329): never the
     * engine's error page, never the page before's title.
     */
    private val lastVisits = HashMap<String, uniffi.vela_core_uniffi.BrowserVisit>()

    /** Set when something outside 探索 (a deep link, a dev seam) opened a page: the tab should show. */
    val openRequested = MutableStateFlow(false)

    /**
     * Tabs whose engine the core's plan let go of (spec 099 FR-004): their URL
     * and title stay in the strip, their page does not. Shown again, such a
     * tab loads again and says so.
     */
    private val suspended = HashSet<String>()

    private val _reloadedTab = MutableStateFlow<String?>(null)

    /** The tab that came back from a suspension and is still the one shown — its chrome says "reloaded to save memory". */
    val reloadedTab: StateFlow<String?> = _reloadedTab

    /**
     * The tab whose page the person asked for (iOS's `pageWanted`, per tab —
     * spec 099 navigation). [reconcile] mints an engine — and so runs a page's
     * scripts — only for this tab. A tab restored at launch, or one selected
     * behind 探索's home (a neighbour of a closed tab, a tab left for the
     * wallet that lost its engine), stays dormant until somebody resumes it:
     * a resume row, the switcher, an open, a reload. An engine that already
     * exists is kept either way; this only gates making one.
     */
    private var wanted: String? = null

    /**
     * An open went to a NEW tab the core has yet to make: the tabs that were
     * there before it. The selected tab that is not one of them is the page
     * that was asked for ([reconcile] reads it once).
     */
    private var wantOpened: Set<String>? = null
        set(value) {
            field = value
            _pageComing.value = value != null || awaiting != null
        }

    /**
     * A tab a resume or an open asked the core to select, before the core has
     * (spec 099 navigation). Until it has — and while [wantOpened] waits for
     * a new tab — NO page is in front: the one there now is the dApp the
     * person left for the home, and showing it would flash it up over the
     * home for the moment before the page they asked for.
     */
    private var awaiting: String? = null
        set(value) {
            field = value
            _pageComing.value = value != null || wantOpened != null
        }

    private val _pageComing = MutableStateFlow(false)

    /**
     * An open or a resume is on its way and its page is not in front yet
     * ([awaiting], [wantOpened]): the screen stays where the person asked
     * from — the home, the switcher — until it is.
     */
    val pageComing: StateFlow<Boolean> = _pageComing

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
        // A request opened or settled changes which tabs are busy: the plan is asked again.
        scope.launch(Dispatchers.Main.immediate) {
            dbrHost.view.collect { planEngines() }
        }
    }

    /**
     * Let go of the engines the core's rule says to (spec 099 FR-004,
     * `browser_tabs::plan_engines`): never the selected tab, never one with
     * anything open. Each one goes the way a closed page goes — its WebView
     * destroyed — but the tab stays, and the browser machine hears that its
     * page is gone (the desktop's `page_gone_events`). [pressure]: the system
     * asked the app to free memory.
     */
    private fun planEngines(pressure: Boolean = false) {
        if (!started) return
        val explore = exploreHost.view.value
        if (!explore.ready || engines.isEmpty()) return
        val plan = BrowserTabs.plan(BrowserTabs.input(explore, dapp.value, engines.keys, pressure))
        for (tab in plan.suspend) {
            val engine = engines.remove(tab) ?: continue
            VelaLog.event("browser.tabs", "suspended", "tab" to tab, "pressure" to pressure, "live" to engines.size)
            engine.destroy()
            if (_current.value === engine) _current.value = null
            suspended += tab
            BrowserTabs.pageGone(tab, now()).forEach(::dispatch)
        }
    }

    /**
     * The system asked for memory back (`onTrimMemory`, `onLowMemory`): the
     * core's plan under pressure keeps the selected tab and the busy ones only.
     */
    fun memoryPressure() = planEngines(pressure = true)

    /**
     * The selected tab gets an engine when it has a URL; closed tabs lose theirs
     * (and the core hears it, so their requests are settled); tabs not in front
     * are paused.
     */
    private fun reconcile(view: ExploreView) {
        val alive = view.tabs.map { it.id }.toSet()
        engines.keys.filter { it !in alive }.forEach { id ->
            engines.remove(id)?.destroy()
            dispatch(DbrEvent.TabClosed(id, now()))
        }
        // A suspended tab that was closed has no page to tell the core about —
        // only its id goes, and the core settles whatever it still had.
        suspended.filter { it !in alive }.forEach { id ->
            suspended -= id
            dispatch(DbrEvent.TabClosed(id, now()))
        }
        if (_snapshots.value.keys.any { it !in alive }) _snapshots.value = _snapshots.value.filterKeys { it in alive }
        crashed.retainAll(alive)
        lastVisits.keys.retainAll(alive)
        _statusSeen.value.keys.filter { it !in alive }.takeIf { it.isNotEmpty() }?.let { gone -> _statusSeen.value = _statusSeen.value - gone.toSet() }
        val selected = view.tabs.firstOrNull { it.id == view.selected_tab } ?: view.tabs.firstOrNull()
        // The new tab an open asked for is the page that was wanted.
        val opened = selected != null && wantOpened?.let { before -> selected.id !in before } == true
        if (opened) wanted = selected!!.id
        // The selection a resume or an open asked for has landed (or its tab is gone).
        val landed = awaiting?.let { id -> selected?.id == id || id !in alive } == true
        // "Reloaded to save memory" lasts until the person leaves that tab.
        if (_reloadedTab.value != null && _reloadedTab.value != selected?.id) _reloadedTab.value = null
        // An engine that exists stays; one is MADE only for the tab somebody
        // asked to see ([wanted]) — a restored or landed-away tab runs no page.
        // Nothing is in front while an open or a resume is still on its way.
        val asking = (wantOpened != null && !opened) || (awaiting != null && !landed)
        val engine = selected?.url?.takeIf { !asking && selected.id !in crashed && (selected.id in engines || selected.id == wanted) }?.let { url ->
            engines.getOrPut(selected.id) {
                // Woken: a tab the plan let go of loads its page again, and says so.
                if (suspended.remove(selected.id)) {
                    VelaLog.event("browser.tabs", "woken: reloading", "tab" to selected.id)
                    _reloadedTab.value = selected.id
                }
                newEngine(selected.id).also { it.load(url) }
            }
        }
        // A tab not in front keeps its page but runs no animations or media.
        engines.values.filter { it !== engine }.forEach { it.webView.onPause() }
        if (_current.value !== engine) _current.value = engine
        // Only now the wait is over ([pageComing]): the page it waited for is in front.
        if (opened) wantOpened = null
        if (landed) awaiting = null
        // A tab selected, opened or closed: the core says which engines may go.
        planEngines()
    }

    private fun newEngine(tabId: String) = BrowserEngine(
        appContext = context.applicationContext,
        id = tabId,
        debugMode = debug.on,
        listener = object : BrowserEngine.Listener {
            override fun pageMessage(tab: String, json: String, sourceOrigin: String, isMainFrame: Boolean) =
                dispatch(DbrEvent.PageMessage(tab = tab, frame_origin = sourceOrigin, is_main_frame = isMainFrame, message_json = json, now_ms = now()))

            override fun navigationStarted(tab: String, url: String) {
                VelaLog.event("browser.nav", "document load", "url" to url.take(96))
                dispatch(DbrEvent.NavigationStarted(tab, url, now()))
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = null), ExploreEvent.serializer())
            }

            override fun loadFinished(tab: String, url: String, title: String, failed: Boolean, httpStatus: Int?) {
                dispatch(DbrEvent.LoadFinished(tab, url, now()))
                // A failed load leaves the tab's own title alone — the engine's
                // error page ("网页无法打开") is not the site (spec 079).
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = title.ifBlank { null }.takeUnless { failed }), ExploreEvent.serializer())
                if (failed) return
                // One visit per load, recorded when the load finishes — its
                // address, title and icon read in ONE script from the document
                // itself, then put through the core's visit rule (spec 079: the
                // Xiaomi saved bscscan.com under Uniswap's title and icon, and
                // an error page as a visit).
                val engine = engines[tab] ?: return
                engine.webView.evaluateJavascript(PAGE_FACTS_JS) { raw ->
                    val facts = runCatching { JSONObject(JSONObject("{\"v\":$raw}").getString("v")) }.getOrNull() ?: return@evaluateJavascript
                    val visit = uniffi.vela_core_uniffi.browserLoadVisit(
                        url = facts.optString("href"),
                        title = facts.optString("title"),
                        icon = facts.optString("icon").ifBlank { null },
                        mainFrameFailed = false,
                        httpStatus = httpStatus?.toUShort(),
                    ) ?: return@evaluateJavascript
                    lastVisits[tab] = visit
                    // Issue #425: the site's good load titles a favourite its
                    // host stands in for — one pinned while it had failed, or
                    // one whose stored name (an error page's) the core reset.
                    exploreHost.dispatch(ExploreEvent.PageLoaded(url = visit.url, title = visit.title), ExploreEvent.serializer())
                    scope.launch {
                        historyLoaded.await()
                        bhistHost.dispatch(BhistEvent.VisitRecorded(url = visit.url, title = visit.title, favicon = visit.favicon, now_ms = now()), BhistEvent.serializer())
                    }
                }
            }

            override fun shown(tab: String, url: String, title: String) {
                if (url.isBlank()) return
                exploreHost.dispatch(ExploreEvent.TabNavigated(id = tab, url = url, title = title.ifBlank { null }), ExploreEvent.serializer())
            }

            override fun rendererGone(tab: String) {
                crashed += tab
                _snapshots.value = _snapshots.value - tab
                engines.remove(tab)?.destroy()
                if (_current.value?.id == tab) _current.value = null
                dispatch(DbrEvent.RendererGone(tab, now()))
            }

            override fun snapshot(tab: String, image: androidx.compose.ui.graphics.ImageBitmap?) {
                _snapshots.value = if (image == null) _snapshots.value - tab else _snapshots.value + (tab to image)
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

    /** A page another app or website asked this wallet to open, and the host the person is asked about. */
    data class ExternalPage(val url: String, val host: String)

    /**
     * Spec 088 FR-004: `velawallet://open?url=…` waits here for the person.
     * Nothing loads — and no page meets the provider — until they say yes
     * to the host on the sheet ([answerExternal]).
     */
    val externalPage = MutableStateFlow<ExternalPage?>(null)

    /**
     * Ask before opening a page from outside. The core decides whether the
     * link may be opened at all (https, a plain host) and which host to show;
     * anything else is dropped in silence, like any link this app does not
     * handle.
     */
    fun askToOpen(url: String) {
        val host = dappExternalPageHost(url) ?: run {
            VelaLog.event("browser.external", "link refused", "scheme" to url.substringBefore(':').take(12))
            return
        }
        externalPage.value = ExternalPage(url, host)
    }

    /** The sheet's answer: open the page as asked, or forget it. */
    fun answerExternal(open: Boolean) {
        val page = externalPage.value ?: return
        externalPage.value = null
        if (open) open(page.url, fromOutside = true)
    }

    /**
     * Opens a page where the core's open target says (`browser_open_target`,
     * spec 099 navigation): the page on screen when its own bar asked
     * ([onPage]); over the home, a tab already on a picked site ([kind]
     * [ExploreOpenKind.Site]) resumed as it was left, else the selected
     * start-page tab's first page, else a NEW tab — never a load over a live
     * dApp from the home. A full strip takes no new tab: the core then names
     * a tab it can spare (a start page, else the one used longest ago, never
     * the dApp just left), which [loadInto] selects. [fromOutside] (a deep link, a scan, the
     * external-page sheet, a launch URL) is an address, and says so to the
     * host once the open is in the view, so 探索 lands on it.
     */
    fun open(text: String, kind: ExploreOpenKind = ExploreOpenKind.Address, onPage: Boolean = false, fromOutside: Boolean = false) {
        val url = coerceUrl(text)
        if (url.isEmpty()) return
        start()
        // The machine drops a mutation before its document has loaded
        // (device-found: an open from a deep link raced the load and no tab
        // appeared). Every intent below waits for `ready` first.
        scope.launch(Dispatchers.Main.immediate) {
            val view = exploreHost.view.first { it.ready }
            val target = BrowserTabs.openTarget(view, shown = view.selected_tab, onPage = onPage && !fromOutside, url = url, kind = kind)
            VelaLog.event("browser.tabs", "open", "target" to target::class.simpleName, "kind" to kind.name, "onPage" to onPage)
            val applied: Long? = when (target) {
                is ExploreOpenTarget.Load -> loadInto(view, target.id, url)
                is ExploreOpenTarget.Resume -> resume(target.id)
                ExploreOpenTarget.NewTab -> {
                    wantOpened = view.tabs.map { it.id }.toSet()
                    // Nothing in front until the new tab is: the dApp left for
                    // the home would flash up over it first.
                    _current.value = null
                    exploreHost.dispatchNumbered(ExploreEvent.TabOpened(url = url, title = null, now_ms = now()), ExploreEvent.serializer())
                }
            }
            // Once the open is in the view: the new tab's engine is made now
            // (the collector may not have run yet), and the host asked where
            // 探索 lands only then — before it, the landing would name the
            // page that was there.
            if (applied != null) settle(applied)
            if (target == ExploreOpenTarget.NewTab) reconcile(exploreHost.view.value)
            // An open the core never applied waits no longer: the page in front comes back.
            if (wantOpened != null || awaiting != null) {
                wantOpened = null
                awaiting = null
                reconcile(exploreHost.view.value)
            }
            if (fromOutside) openRequested.value = true
        }
    }

    /**
     * The core's `Load`: the page on screen is told directly; a start-page
     * tab gets its first page, and a crashed one comes back to life on the
     * address. Such an engine is made HERE, not left to the next view change —
     * the same address twice changes nothing. The number of the event the
     * load sent, when it sent one.
     */
    private fun loadInto(view: ExploreView, id: String, url: String): Long? {
        val tab = view.tabs.firstOrNull { it.id == id } ?: return null
        wanted = tab.id
        // A tab not selected (the core's pick in a full strip) is selected
        // first; until the core has, no page is in front ([awaiting]).
        val selecting = if (view.selected_tab != tab.id) {
            awaiting = tab.id
            _current.value = null
            exploreHost.dispatchNumbered(ExploreEvent.TabSelected(tab.id), ExploreEvent.serializer())
        } else null
        val live = engines[tab.id]?.takeIf { tab.id !in crashed }
        if (live != null) {
            live.load(url)
            return selecting
        }
        crashed -= tab.id
        val navigated = exploreHost.dispatchNumbered(ExploreEvent.TabNavigated(id = tab.id, url = url, title = null), ExploreEvent.serializer())
        val engine = newEngine(tab.id).also { engines[tab.id] = it }
        engine.load(url)
        // In front now when its tab already is; else once the selection lands.
        if (selecting == null) _current.value = engine
        return navigated
    }

    /** Waits, briefly, for the core to have applied event [number]. */
    private suspend fun settle(number: Long) {
        kotlinx.coroutines.withTimeoutOrNull(SETTLE_MS) { exploreHost.commits.first { exploreHost.applied(number) } }
    }

    private fun whenReady(block: () -> Unit) {
        start()
        scope.launch(Dispatchers.Main.immediate) {
            exploreHost.view.first { it.ready }
            block()
        }
    }

    fun newTab() = whenReady { exploreHost.dispatch(ExploreEvent.TabOpened(url = null, title = null, now_ms = now()), ExploreEvent.serializer()) }

    /**
     * A tab the person asked to see — a resume row, the switcher, the
     * core's landing on a page: it gets its page, live as it was left, or
     * loaded again when it had none. Asked for even when it is the tab
     * already selected: that is how a tab a launch left dormant wakes.
     */
    fun selectTab(id: String) {
        val selecting = resume(id) ?: return
        // A selection the core never applied waits no longer: the page in front comes back.
        scope.launch(Dispatchers.Main.immediate) {
            settle(selecting)
            if (awaiting == id) {
                awaiting = null
                reconcile(exploreHost.view.value)
            }
        }
    }

    /**
     * [selectTab]'s work; the number of the selection's event when one was
     * sent. Until the core has selected [id], no page is in front
     * ([awaiting]): the one there is another tab's.
     */
    private fun resume(id: String): Long? {
        wanted = id
        val view = exploreHost.view.value
        if (view.selected_tab == id) {
            awaiting = null
            if (started) reconcile(view)
            return null
        }
        awaiting = id
        if (_current.value?.id != id) _current.value = null
        return exploreHost.dispatchNumbered(ExploreEvent.TabSelected(id), ExploreEvent.serializer())
    }

    /**
     * 探索 landed on its home (spec 099 navigation): nothing is shown, so no
     * tab is asked for — the one left for the wallet keeps the engine it has,
     * and none is woken behind the home until a resume.
     */
    fun landedHome() {
        wanted = null
    }

    /** The tab whose request waits on the person — the core's `browser_waiting_tab` over the browser machine's view. */
    fun waitingTab(): String? = BrowserTabs.waitingTab(dbrHost.viewJson.value, dapp.value)

    /** The site menu's Stop: the page in front's load is let go. */
    fun stop() {
        _current.value?.stop()
    }

    fun closeTab(id: String) = exploreHost.dispatch(ExploreEvent.TabClosed(id), ExploreEvent.serializer())

    /**
     * Spec 099: Chrome's batch closes. Which tabs a scope takes is the core's
     * ([BrowserTabs.closedBy]); they go in one event, and [reconcile] lets go
     * of their engines and tells the browser machine, as for a single close.
     */
    fun closeTabs(scope: TabCloseScope) {
        val ids = closedBy(scope)
        if (ids.isEmpty()) return
        VelaLog.event("browser.tabs", "closed", "scope" to scope::class.simpleName, "n" to ids.size)
        exploreHost.dispatch(ExploreEvent.TabsClosed(ids), ExploreEvent.serializer())
    }

    /** The tabs [scope] would close now — the core's answer; a tab's menu greys out a close that takes none. */
    fun closedBy(scope: TabCloseScope): List<String> = BrowserTabs.closedBy(exploreHost.view.value.tabs, scope)

    fun closeOtherTabs(keep: String) = closeTabs(TabCloseScope.Others(keep))
    fun closeTabsToRight(of: String) = closeTabs(TabCloseScope.Right(of))
    fun closeAllTabs() = closeTabs(TabCloseScope.All)

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

    /**
     * Spec 082 RE3: the network came back (`netHealthStep`'s `came_back`).
     * Every failed page's count starts over; the page in front is loaded
     * again now when its failure is one the network explains.
     */
    fun networkCameBack() {
        val front = _current.value
        engines.values.filter { it !== front }.forEach { it.networkCameBack() }
        front?.networkCameBack()
    }

    /** The device pass's renderer death (debug builds only; `chrome://crash` kills the renderer on purpose). */
    fun debugCrashRenderer() {
        _current.value?.webView?.loadUrl("chrome://crash")
    }

    /** Reload the page in front — or, when its renderer died, bring the tab back to life. */
    fun reload() {
        val selected = exploreHost.view.value.let { view -> view.tabs.firstOrNull { it.id == view.selected_tab } } ?: return
        if (selected.id in crashed) {
            crashed -= selected.id
            wanted = selected.id
            reconcile(exploreHost.view.value)
        } else {
            _current.value?.reload()
        }
    }

    /**
     * The star: pins the page in front, or unpins it when it already is a favourite.
     *
     * It acts on what the bar names (the core's `browserAddressBar`: under a
     * failure panel, the address that failed), and the name is the core's rule
     * (`browserPinnedTitle`): that site's last good title, else its host. The
     * engine's own title was the WebView's error page's ("网页无法打开") when
     * the page had failed, or the page before's (issue #329).
     */
    fun toggleFavorite() {
        val engine = _current.value ?: return
        val url = engine.state.value.addressBar().url
        if (url.isBlank()) return
        val origin = dappOriginOf(url)
        if (origin != null && exploreHost.view.value.favorites.any { it.origin == origin }) {
            removeFavorite(origin)
        } else {
            val title = uniffi.vela_core_uniffi.browserPinnedTitle(url, lastVisits[engine.id])
            exploreHost.dispatch(ExploreEvent.FavoriteAdded(url = url, title = title, now_ms = now()), ExploreEvent.serializer())
        }
    }

    fun removeFavorite(origin: String) = exploreHost.dispatch(ExploreEvent.FavoriteRemoved(origin), ExploreEvent.serializer())
    fun setSystemGroupHidden(group: ExploreSystemGroup, hidden: Boolean) =
        exploreHost.dispatch(ExploreEvent.SystemGroupHiddenSet(group, hidden), ExploreEvent.serializer())
    fun clearRecent() = bhistHost.dispatch(BhistEvent.ClearAll, BhistEvent.serializer())

    private companion object {
        /** How long an open waits for the core to apply it before carrying on regardless. */
        const val SETTLE_MS = 5_000L

        /**
         * The document's own address, title and icon, absolute, in one read —
         * three reads could straddle two documents (spec 079 R3).
         */
        const val PAGE_FACTS_JS =
            "(function(){var l=document.querySelector(\"link[rel~='icon']\");" +
                "return JSON.stringify({href:location.href,title:document.title,icon:l?l.href:''})})()"
    }
}
