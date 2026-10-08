package app.getvela.wallet.feature.browser

import androidx.compose.ui.graphics.Color
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.browser.core.DbrConsentView
import app.getvela.wallet.feature.browser.core.DbrTabView
import app.getvela.wallet.feature.explore.ConnectionModel
import app.getvela.wallet.feature.browser.core.BhistEntry
import app.getvela.wallet.feature.browser.core.BhistView
import app.getvela.wallet.feature.browser.core.EngineState
import app.getvela.wallet.feature.browser.core.ExploreSite
import app.getvela.wallet.feature.browser.core.ExploreView
import app.getvela.wallet.feature.explore.ExploreEmptyCopy
import app.getvela.wallet.feature.explore.ExploreScreenModel
import app.getvela.wallet.feature.explore.ExploreSheet
import app.getvela.wallet.feature.explore.FavoritesSection
import app.getvela.wallet.feature.explore.GroupAction
import app.getvela.wallet.feature.explore.GroupKind
import app.getvela.wallet.feature.explore.GroupManageRow
import app.getvela.wallet.feature.explore.GroupModel
import app.getvela.wallet.feature.explore.SiteModel
import app.getvela.wallet.feature.explore.TabModel
import app.getvela.wallet.feature.explore.TileModel

/**
 * The explore screen from the core's views (spec 044 T019; the desktop's
 * `explore/live.rs`). The drawn model keeps only its words; favourites,
 * groups, recents, tabs and the page's chrome are this device's own.
 *
 * A site's `id` is the URL it opens — what a tap has to hand back.
 */
object ExploreLive {
    /** What a code scanned from 探索 is (issue #273, option (b) of D1, spec 070). */
    sealed class Scanned {
        /** A web address: it opens in the browser. */
        data class Url(val url: String) : Scanned()

        /** An address or an `ethereum:` request: it is a payment, and goes to Send. */
        data class Payment(val text: String) : Scanned()

        /** A WalletConnect pairing: said plainly — this wallet does not connect that way. */
        data object WalletConnect : Scanned()

        data object Unrecognized : Scanned()
    }

    /**
     * `http(s)://` opens as read; a bare host (`app.uniswap.org`,
     * `example.com:8080/x`) gets the address bar's `https://`; an address or an
     * `ethereum:` link is a payment; `wc:` is named. Words are NOT searched: a
     * code must never open as a guessed page.
     */
    fun scanned(text: String): Scanned {
        val trimmed = text.trim()
        if (trimmed.isEmpty() || trimmed.any(Char::isWhitespace)) return Scanned.Unrecognized
        if (trimmed.startsWith("wc:", ignoreCase = true)) return Scanned.WalletConnect
        if (trimmed.startsWith("ethereum:", ignoreCase = true) || ADDRESS.matches(trimmed)) return Scanned.Payment(trimmed)
        val scheme = Regex("^(https?)://(.+)$", RegexOption.IGNORE_CASE).find(trimmed)
        if (scheme != null) return Scanned.Url("${scheme.groupValues[1].lowercase()}://${scheme.groupValues[2]}")
        return if (BARE_HOST.matches(trimmed)) Scanned.Url("https://$trimmed") else Scanned.Unrecognized
    }

    /** Kept for the scan gallery's callers: the web address a code opens, or `null`. */
    fun scannedUrl(text: String): String? = (scanned(text) as? Scanned.Url)?.url

    /** `host.tld[:port][/path|?query|#frag]` — dotted labels, a letter-led last label. */
    private val BARE_HOST = Regex("^([A-Za-z0-9-]+\\.)+[A-Za-z][A-Za-z0-9-]*(:\\d+)?([/?#].*)?$")

    private val ADDRESS = Regex("^0x[0-9a-fA-F]{40}$")

    /** What the connection surfaces need to know about the wallet and the page in front's chain. */
    data class Identity(
        val accountName: String = "",
        val accountAddress: String = "",
        val chainName: String = "",
        val chainDot: Color = Color.Unspecified,
        /** Spec 079: the page's chain, for its logo. */
        val chainId: Int = 0,
    )

    /**
     * What the account holds on each chain, in dollars (spec 079, owner: "需要能看到
     * 这个网络上的余额") — the home screen's own figures, never fetched here. A
     * chain whose read failed and a token with no price count for nothing.
     */
    fun heldUsdByChain(balances: app.getvela.wallet.feature.wallet.core.BalanceView): Map<Int, Double> =
        balances.tokens
            .filter { !it.spam && it.price_usd != null && it.chain_id !in balances.failed_chain_ids }
            .groupBy { it.chain_id }
            .mapValues { (_, rows) -> rows.sumOf { (it.balance.toDoubleOrNull() ?: 0.0) * (it.price_usd ?: 0.0) } }

    /**
     * The connection panel's network rows (spec 079): each with its logo and the
     * account's balance there — nothing rather than a zero or a figure the person
     * chose to hide.
     */
    fun networkOptions(
        networks: List<app.getvela.wallet.feature.settings.core.NetNetworkRow>,
        names: Map<Int, String>,
        siteChain: Int,
        balances: app.getvela.wallet.feature.wallet.core.BalanceView,
        fiat: (Double) -> String,
    ): List<app.getvela.wallet.feature.explore.components.PickerOption> {
        val held = heldUsdByChain(balances)
        return networks.map { n ->
            val chain = n.chain_id.toInt()
            app.getvela.wallet.feature.explore.components.PickerOption(
                id = n.chain_id.toString(),
                label = names[chain] ?: n.chain_id.toString(),
                selected = chain == siteChain,
                logoUrl = app.getvela.wallet.core.marks.Marks.chainLogoUrl(chain),
                amount = held[chain]?.takeIf { it >= MIN_SHOWN_USD && !balances.hidden }?.let(fiat),
            )
        }
    }

    /** The account rows (spec 079): each account's identicon, from its address. */
    fun accountOptions(
        accounts: List<app.getvela.wallet.feature.onboarding.core.SessionAccountRow>,
        active: String,
    ): List<app.getvela.wallet.feature.explore.components.PickerOption> = accounts.map { row ->
        app.getvela.wallet.feature.explore.components.PickerOption(
            id = row.index.toString(),
            label = row.name.ifBlank { shortAddress(row.address) },
            detail = shortAddress(row.address),
            selected = row.address.equals(active, ignoreCase = true),
            identiconSeed = row.address,
        )
    }

    /** Below half a cent the row says nothing: dust is not a balance worth reading. */
    private const val MIN_SHOWN_USD = 0.005

    /**
     * The page's chain could not be reached (spec 079): the pool failed it on
     * its last try — or, spec 082 RF1, one call's first full pass reached
     * none of its endpoints (`unreached_chains`), so the notice shows while
     * the dApp still waits rather than after three passes — and not because
     * the endpoint is merely busy: a rate-limited chain keeps quiet
     * everywhere (the pool's invariant ④). The home banner keeps reading
     * `failed_chains` alone.
     */
    fun chainUnreachable(chainId: Int, pool: app.getvela.wallet.feature.wallet.core.RpcPoolView): Boolean =
        (chainId in pool.failed_chains || chainId in pool.unreached_chains) && chainId !in pool.rate_limited_chains

    fun home(
        fallback: ExploreScreenModel,
        view: ExploreView,
        history: BhistView,
        engine: EngineState?,
        strings: VelaStrings,
        /** The core's view of the tab in front (spec 070) — connection, chain, lock, crash. */
        tab: DbrTabView? = null,
        identity: Identity = Identity(),
        /** Spec 079: each tab's page as it last left the screen. */
        snapshots: Map<String, androidx.compose.ui.graphics.ImageBitmap> = emptyMap(),
        /** Spec 079: the pool's verdicts, for the page's chain notice. */
        pool: app.getvela.wallet.feature.wallet.core.RpcPoolView = app.getvela.wallet.feature.wallet.core.RpcPoolView(),
        chainAsking: Boolean = false,
        /** Spec 099 FR-004: the shown tab came back from a suspension and loaded again. */
        reloaded: Boolean = false,
        /** Spec 099 FR-014: the status line the person last put away for the shown tab. */
        statusSeen: String? = null,
    ): ExploreScreenModel {
        val connected = tab?.connected_address != null
        val favorites = view.favorites.map { tileOf(it) }
        val populated = favorites.isNotEmpty() || history.entries.isNotEmpty()
        val groups = listOfNotNull(recentGroup(history.entries, strings).takeUnless { view.recent_hidden })
        val tabs = view.tabs.map { open ->
            TabModel(
                id = open.id,
                title = open.title.ifBlank { strings.t("explore.startPage") },
                site = open.url?.let { url -> SiteModel(id = url, name = open.title, host = open.host, letter = letterOf(open.host), tint = tintOf(open.host), iconUrls = iconsOf(url)) },
                selected = view.selected_tab == open.id,
                startPage = open.url == null,
                snapshot = snapshots[open.id].takeIf { open.url != null },
            )
        }
        // Spec 082 RE1: what the bar names is the core's rule over the
        // engine's committed, pending and failed addresses — never the drawn
        // fixture's host, and never a lock for a page that is not there. A
        // tab whose renderer died names the document the core last saw.
        val bar = engine?.addressBar() ?: tab?.origin?.let { uniffi.vela_core_uniffi.browserAddressBar(it, null, null) }
        // The star lights for what it pins: the address the bar names (issue
        // #329 — under a failure panel, the one that failed), else the page's
        // origin, or the core's word for a tab whose renderer died.
        val shownOrigin = bar?.url?.takeIf { it.isNotBlank() }?.let { uniffi.vela_core_uniffi.dappOriginOf(it) } ?: engine?.origin ?: tab?.origin
        val bookmarked = shownOrigin != null && view.favorites.any { it.origin == shownOrigin }
        return fallback.copy(
            resume = resume(view, strings),
            // No CTA: there is no curated list behind "browse", and a button
            // that goes nowhere is worse than none (spec 070).
            empty = if (populated) null else ExploreEmptyCopy(strings.t("explore.startTitle"), strings.t("explore.startHint"), ""),
            // The Favorites heading stays on any page with something on it —
            // no favourites yet, or Favorites hidden: its Edit is the way to
            // Manage groups, and with every group hidden the page was left with
            // the search field alone and no way back (issue #330). Hidden, it
            // loses its tiles, not its heading — the desktop's rule (078 W-11).
            favorites = if (!populated) {
                null
            } else {
                FavoritesSection(
                    title = strings.t("explore.favorites"),
                    action = strings.t("explore.edit"),
                    tiles = if (view.favorites_hidden) emptyList() else favorites.map { TileModel.Site(it) } + TileModel.Add(strings.t("explore.add")),
                )
            },
            groups = groups,
            browser = fallback.browser.copy(
                url = bar?.url.orEmpty(),
                host = bar?.host.orEmpty(),
                secure = bar?.lock == "closed",
                lockShown = bar != null && bar.lock != "none",
                canBack = engine?.canBack ?: false,
                canForward = engine?.canForward ?: false,
                bookmarked = bookmarked,
                tabCount = tabs.size,
                connected = connected,
                accountSeed = identity.accountAddress.ifBlank { fallback.browser.accountSeed },
                loading = engine?.loading ?: false,
                progress = engine?.progress ?: 100,
                failed = engine?.failed ?: false,
                failureReason = engine?.failure?.reasonKey?.let { strings.t(it) },
                retrying = engine?.retrying ?: false,
                chainNotice = strings.t("explore.chainDown", mapOf("chain" to identity.chainName))
                    .takeIf { engine != null && tab != null && chainUnreachable(identity.chainId, pool) },
                chainAsking = chainAsking,
                crashed = tab?.crashed ?: false,
                status = status(tab, reloaded, statusSeen, strings),
            ),
            connection = engine?.let { e -> connection(fallback.connection, e, strings, tab, identity) } ?: fallback.connection,
            tabs = tabs,
            // Issue #465: the start page's two sections and nothing else — no
            // groups of the person's own, so no "System" tag to tell them apart.
            groupManageSheet = ExploreSheet.GroupManage(
                title = strings.t("explore.manageGroups"),
                rows = listOf(
                    GroupManageRow("favorites", strings.t("explore.favorites"), strings.t("explore.siteCount", mapOf("n" to view.favorites.size.toString())), hidden = view.favorites_hidden),
                    GroupManageRow("recent", strings.t("explore.recent"), meta = null, hidden = view.recent_hidden),
                ),
            ),
            siteMenuSheet = engine?.let { e ->
                val secure = tab?.secure ?: false
                fallback.siteMenuSheet.copy(
                    site = SiteModel(id = e.url, name = e.title.ifBlank { e.host }, host = e.host, letter = letterOf(e.host), tint = tintOf(e.host), iconUrls = iconsOf(e.url)),
                    // Spec 079 (owner): the lock alone — no "安全站点".
                    statusLine = "",
                    secure = secure,
                    // What each row does now depends on the page: the star's row
                    // unpins a favourite, and Disconnect is offered only to a
                    // site that is connected.
                    items = fallback.siteMenuSheet.items.mapNotNull { item ->
                        when (item.id) {
                            // Greyed, never hidden: the rows under it keep their places.
                            "forward" -> item.copy(enabled = e.canForward)
                            // While a load runs the row stops it (iOS spec 082 RE5).
                            "refresh" -> if (e.loading) item.copy(id = STOP, icon = app.getvela.wallet.core.designsystem.components.VelaIcons.Close, label = strings.t("connect.dapp.stop")) else item
                            "favorite" -> if (bookmarked) item.copy(icon = app.getvela.wallet.core.designsystem.components.VelaIcons.StarFilled, label = strings.t("explore.removeFromFavorites")) else item
                            "disconnect" -> item.takeIf { connected }
                            else -> item
                        }
                    },
                )
            } ?: fallback.siteMenuSheet,
        )
    }

    /** The site menu's refresh row while a load runs (iOS spec 082 RE5): it stops the load. */
    const val STOP = "stop"

    /**
     * The home's resume section (spec 099 navigation) from the core's
     * `resumable` — its rows, order and cap are the core's, never re-sorted or
     * re-capped here. Drawn only while some tab has a page; the header's n is
     * every tab, start pages included (`tabs.count`, the switcher's number).
     * A row names the tab by the core's label rule, the recents' own
     * (`browserSiteLabel`), and its id is the TAB's: a tap resumes it.
     */
    fun resume(view: ExploreView, strings: VelaStrings): app.getvela.wallet.feature.explore.ResumeSection? {
        if (!view.ready || view.resumable.isEmpty()) return null
        return app.getvela.wallet.feature.explore.ResumeSection(
            title = strings.t("explore.openTabs", mapOf("n" to view.tabs.size.toString())),
            action = strings.t("explore.tabs"),
            tabs = view.resumable.map { tab ->
                val label = uniffi.vela_core_uniffi.browserSiteLabel(tab.title, tab.host)
                SiteModel(
                    id = tab.id,
                    name = label.name,
                    host = tab.host,
                    letter = letterOf(tab.host),
                    tint = tintOf(tab.host),
                    subtitle = label.hostLine.orEmpty(),
                    iconUrls = tab.url?.let(::iconsOf).orEmpty(),
                )
            },
        )
    }

    /**
     * Spec 099 FR-014: what the shown tab's status line says, if anything, in
     * this order — it was reloaded to save memory; its page loaded and the
     * wallet was not offered to it (the core's provider state, in its words);
     * the latest request that ended in trouble (the core's note: its line and
     * the method). Nothing when the person put that very line away.
     */
    fun status(tab: DbrTabView?, reloaded: Boolean, seen: String?, strings: VelaStrings): app.getvela.wallet.feature.explore.BrowserStatusModel? {
        val (key, text, warning) = when {
            reloaded -> Triple("reloaded", strings.t(app.getvela.wallet.core.i18n.I18nKeys.BrowserStatus.RELOADED), false)
            tab == null -> return null
            tab.page == app.getvela.wallet.feature.browser.core.DbrPageState.Ready &&
                (tab.provider == app.getvela.wallet.feature.browser.core.DbrProviderState.InsecureOrigin ||
                    tab.provider == app.getvela.wallet.feature.browser.core.DbrProviderState.NoHello) ->
                Triple("provider:${tab.provider.name}", strings.t(tab.provider.key), true)
            else -> {
                val note = tab.last_failure ?: return null
                Triple("${tab.failed_recent}:${note.method}:${note.reason.name}", "${strings.t(note.key)} · ${note.method}", true)
            }
        }
        if (seen == key) return null
        return app.getvela.wallet.feature.explore.BrowserStatusModel(
            seen = key,
            text = text,
            warning = warning,
            details = strings.t(app.getvela.wallet.core.i18n.I18nKeys.BrowserStatus.TITLE),
        )
    }

    /**
     * Spec 099 FR-014: the tab's status panel from the core's record of it
     * (`DbrView.inspector`) — its origin, page and wallet in the core's words,
     * and every request newest first: answered with its time, failed with the
     * reason's line, open with an ellipsis. The copyable report is the core's.
     */
    fun inspector(view: app.getvela.wallet.feature.browser.core.DbrInspectorView?, strings: VelaStrings): app.getvela.wallet.feature.explore.BrowserInspectorModel? {
        view ?: return null
        val keys = app.getvela.wallet.core.i18n.I18nKeys.BrowserStatus
        val rows = view.rows.asReversed().map { row ->
            app.getvela.wallet.feature.explore.BrowserInspectorModel.Row(
                method = row.method,
                outcome = when (row.outcome) {
                    app.getvela.wallet.feature.browser.core.DbrOutcome.Open -> "…"
                    app.getvela.wallet.feature.browser.core.DbrOutcome.Answered ->
                        row.ended_ms?.takeIf { row.started_ms > 0 && it >= row.started_ms }
                            ?.let { ended -> "✓ ${Math.round(ended - row.started_ms)} ms" } ?: "✓"
                    app.getvela.wallet.feature.browser.core.DbrOutcome.Failed ->
                        row.reason?.let { strings.t(it.key) } ?: row.code?.toString().orEmpty()
                },
            )
        }
        return app.getvela.wallet.feature.explore.BrowserInspectorModel(
            title = strings.t(keys.TITLE),
            origin = view.origin.orEmpty(),
            page = strings.t(view.page.key),
            provider = strings.t(view.provider.key),
            requestsTitle = strings.t(keys.REQUESTS),
            rows = rows,
            empty = strings.t(keys.NO_REQUESTS).takeIf { rows.isEmpty() },
            copyLabel = strings.t(keys.COPY),
            copiedLabel = strings.t(keys.COPIED),
            report = view.report,
        )
    }

    /** E7 — the connection sheet for the page in front: the CORE's origin and address, the wallet's own name. */
    fun connection(fallback: ConnectionModel, e: EngineState, strings: VelaStrings, tab: DbrTabView?, identity: Identity): ConnectionModel {
        val connected = tab?.connected_address != null
        val secure = tab?.secure ?: false
        return fallback.copy(
            site = SiteModel(id = e.url, name = e.title.ifBlank { e.host }, host = e.host, letter = letterOf(e.host), tint = tintOf(e.host), iconUrls = iconsOf(e.url)),
            // Spec 079 (owner): the lock alone says https; "已连接" is a fact about
            // the connection, not a claim about the site, so it stays — and so
            // does its absence (spec 097 E): "No active connection".
            statusLine = strings.t(if (connected) "explore.connectedTag" else "home.connEmptyTitle"),
            connected = connected,
            // Not connected: no sentence about what the site can see, and no
            // requests to expect from it.
            explainer = fallback.explainer.takeIf { connected }.orEmpty(),
            footnote = fallback.footnote.takeIf { connected }.orEmpty(),
            accountName = identity.accountName.ifBlank { fallback.accountName },
            accountAddress = shortAddress(tab?.connected_address ?: identity.accountAddress),
            accountSeed = tab?.connected_address ?: identity.accountAddress.ifBlank { fallback.accountSeed },
            networkName = identity.chainName.ifBlank { fallback.networkName },
            networkDot = if (identity.chainDot == Color.Unspecified) fallback.networkDot else identity.chainDot,
            networkLogoUrl = identity.chainId.takeIf { it > 0 }?.let { app.getvela.wallet.core.marks.Marks.chainLogoUrl(it) },
            secure = secure,
        )
    }

    /**
     * The consent card (spec 044 FR-006): the drawn connection sheet in its
     * not-yet-connected form. The origin is the CORE's (`DpermConsentView`);
     * the words are `connect.browser.*`; the site's own name is not asked.
     */
    fun consent(fallback: ConnectionModel, consent: DbrConsentView, strings: VelaStrings, identity: Identity, secure: Boolean): ConnectionModel {
        val host = consent.origin.substringAfter("://").substringBefore('/')
        // Spec 096 F11: the account and the network a Connect shares are the
        // CORE's (`DbrConsentView.address`, `.chain_id`) — the same facts every
        // other shell's consent names. The active account's name goes with it
        // only when it IS that account; the chain's logo follows the asking
        // site's chain, never the tab in front.
        val shared = consent.address?.takeIf { it.isNotBlank() } ?: identity.accountAddress
        val isActive = shared.equals(identity.accountAddress, ignoreCase = true)
        return fallback.copy(
            title = strings.t("connect.browser.title", mapOf("host" to host)),
            site = SiteModel(id = consent.origin, name = host, host = host, letter = letterOf(host), tint = tintOf(host), iconUrls = iconsOf(consent.origin)),
            statusLine = "",
            secure = secure,
            networkLogoUrl = consent.chain_id.takeIf { it > 0 }?.let { app.getvela.wallet.core.marks.Marks.chainLogoUrl(it) },
            primaryAction = true,
            accountName = (if (isActive) identity.accountName else shortAddress(shared)).ifBlank { fallback.accountName },
            accountAddress = shortAddress(shared),
            accountSeed = shared.ifBlank { fallback.accountSeed },
            networkName = identity.chainName.ifBlank { fallback.networkName },
            networkDot = if (identity.chainDot == Color.Unspecified) fallback.networkDot else identity.chainDot,
            // One sentence above the buttons and no footnote (spec 082 RE6):
            // the header's ✕ is the cancel.
            explainer = strings.t("connect.browser.body"),
            disconnect = strings.t("connect.browser.connect"),
            footnote = "",
        )
    }

    /**
     * Spec 100: the add-network sheet, in Settings' words wherever they fit.
     * The core decided everything drawn here; this picks the line.
     */
    fun addNetwork(view: app.getvela.wallet.feature.settings.core.NetDappAddView, strings: VelaStrings): app.getvela.wallet.feature.explore.AddNetworkModel {
        val chain = view.chain_id.toString()
        val rows = buildList {
            add(strings.t("addToken.labelName") to view.name.ifEmpty { strings.t("addToken.chainId", mapOf("chainId" to chain)) })
            add(strings.t("addToken.labelChainId") to chain)
            if (view.native_symbol.isNotEmpty()) add(strings.t("addToken.labelNativeToken") to view.native_symbol)
            view.rpc_host?.let { add(strings.t("addToken.labelRpcUrl") to it) }
            view.explorer_host?.let { add(strings.t("addToken.labelExplorer") to it) }
        }
        val phase = view.phase
        val verdict = phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.Ready ||
            phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.NotCompatible
        val checks = view.compat?.takeIf { verdict && it.rpc_failure == null }?.let { result ->
            result.contracts.map { app.getvela.wallet.feature.settings.CheckItemModel(label = it.name, ok = it.deployed) } +
                app.getvela.wallet.feature.settings.CheckItemModel(label = strings.t("settingsModals.addNetwork.checkSigner"), ok = result.p256_available == true)
        }.orEmpty()
        val pill = when (phase) {
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.Checking ->
                app.getvela.wallet.feature.settings.StatusPillModel(app.getvela.wallet.feature.settings.SettingsTone.Neutral, strings.t("settingsModals.addNetwork.checkingCompatibility"))
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.Ready ->
                app.getvela.wallet.feature.settings.StatusPillModel(app.getvela.wallet.feature.settings.SettingsTone.Ok, strings.t("settingsModals.addNetwork.compatible"))
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.NotCompatible ->
                app.getvela.wallet.feature.settings.StatusPillModel(app.getvela.wallet.feature.settings.SettingsTone.Error, strings.t("settingsModals.addNetwork.incompatible"))
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.CheckFailed ->
                app.getvela.wallet.feature.settings.StatusPillModel(app.getvela.wallet.feature.settings.SettingsTone.Warn, strings.t("settingsModals.addNetwork.unableToVerify"))
            else -> null
        }
        val note = when (phase) {
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.Ready ->
                view.compat?.takeIf { !it.multi_key_ready }?.let { strings.t("settingsModals.addNetwork.singleKeyOnly") }
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.NotCompatible -> strings.t("settingsModals.addNetwork.incompatibleHint")
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.WrongRpc -> strings.t(
                "assets.rpcFixWrongChain",
                mapOf("actual" to (view.reported_chain_id?.toString() ?: ""), "expected" to chain),
            )
            app.getvela.wallet.feature.settings.core.NetDappAddPhase.NoRpc -> strings.t("componentsUi.browserStatus.reason.badRpc")
            else -> null
        }
        val decided = phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.NotCompatible ||
            phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.WrongRpc ||
            phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.NoRpc
        return app.getvela.wallet.feature.explore.AddNetworkModel(
            title = strings.t("settingsModals.addNetwork.modalTitle"),
            lead = strings.t("connect.browser.addLead", mapOf("host" to view.host)),
            site = SiteModel(id = view.origin, name = view.host, host = view.host, letter = letterOf(view.host), tint = tintOf(view.host), iconUrls = iconsOf(view.origin)),
            rows = rows,
            fromSite = if (view.from_site) strings.t("connect.browser.addFromSite") else null,
            pill = pill,
            checksTitle = if (checks.isNotEmpty()) strings.t("settingsModals.addNetwork.compatibilityCheck") else null,
            checks = checks,
            note = note,
            add = if (view.can_add) strings.t("settingsModals.addNetwork.addNetworkBtn") else null,
            retry = if (phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.CheckFailed) strings.t("settingsModals.addNetwork.retry") else null,
            setupTool = if (phase == app.getvela.wallet.feature.settings.core.NetDappAddPhase.NotCompatible) strings.t("settingsModals.addNetwork.openChainSetupTool") else null,
            dismiss = if (decided) strings.t("common.done") else strings.t("connect.browser.cancel"),
        )
    }

    fun shortAddress(address: String): String =
        if (address.length > 12) "${address.take(6)}…${address.takeLast(4)}" else address

    /**
     * A recent row shows the title over the host it actually is — the core's
     * rule (`browserSiteLabel`, spec 082 RE7): a title that is empty or IS
     * the host is said once, as the name, with no second line.
     */
    fun siteOf(entry: BhistEntry): SiteModel {
        val label = uniffi.vela_core_uniffi.browserSiteLabel(entry.title, entry.host)
        return SiteModel(
            id = entry.url,
            name = label.name,
            host = entry.host,
            letter = letterOf(entry.host),
            tint = tintOf(entry.host),
            subtitle = label.hostLine.orEmpty(),
            // The icon the page named when it was visited, then the usual places.
            iconUrls = (listOf(entry.favicon).filter { it.startsWith("https://") } + iconsOf(entry.url)).distinct(),
        )
    }

    fun tileOf(site: ExploreSite): SiteModel = SiteModel(
        id = site.url,
        name = site.name,
        host = site.host,
        letter = letterOf(site.host),
        tint = tintOf(site.host),
        subtitle = site.host,
        iconUrls = iconsOf(site.url),
    )

    /** Spec 079: where a site's icon conventionally lives (the signing header's list), https only. */
    private fun iconsOf(url: String): List<String> =
        uniffi.vela_core_uniffi.dappOriginOf(url)?.let(app.getvela.wallet.feature.signing.SigningLive::siteIconUrls).orEmpty()

    fun recentGroup(entries: List<BhistEntry>, strings: VelaStrings): GroupModel? {
        if (entries.isEmpty()) return null
        return GroupModel(
            id = "recent",
            title = strings.t("explore.recent"),
            kind = GroupKind.Recent,
            action = GroupAction.Clear,
            sites = entries.map { siteOf(it) },
        )
    }

    /** The avatar letter — the core's rule (spec 079): `app.uniswap.org` is "U", not "A". */
    fun letterOf(host: String): String = uniffi.vela_core_uniffi.browserSiteLetter(host)

    /** A stable hue per host (FNV-1a, the desktop's rule): the same site is the same colour everywhere. */
    fun tintOf(host: String): Color {
        var hash = 2_166_136_261L.toInt()
        for (byte in host.toByteArray(Charsets.UTF_8)) {
            hash = hash xor (byte.toInt() and 0xff)
            hash *= 16_777_619
        }
        val hue = ((hash.toLong() and 0xffff_ffffL) % 360L).toFloat()
        return Color.hsl(hue, 0.72f, 0.55f)
    }
}
