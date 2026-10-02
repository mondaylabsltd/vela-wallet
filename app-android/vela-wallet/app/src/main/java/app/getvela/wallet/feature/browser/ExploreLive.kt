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
    ): ExploreScreenModel {
        val connected = tab?.connected_address != null
        val favorites = view.favorites.map { tileOf(it) }
        val populated = favorites.isNotEmpty() || history.entries.isNotEmpty() || view.groups.isNotEmpty()
        val groups = buildList {
            if (!view.recent_hidden) recentGroup(history.entries, strings)?.let { add(it) }
            addAll(customGroups(view))
        }
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
            tabCountLabel = tabs.size.takeIf { it > 0 }?.toString(),
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
            ),
            connection = engine?.let { e -> connection(fallback.connection, e, strings, tab, identity) } ?: fallback.connection,
            tabs = tabs,
            groupManageSheet = ExploreSheet.GroupManage(
                title = strings.t("explore.manageGroups"),
                newGroup = strings.t("explore.newGroup"),
                rows = buildList {
                    add(GroupManageRow("favorites", strings.t("explore.favorites"), strings.t("explore.siteCount", mapOf("n" to view.favorites.size.toString())), system = true, hidden = view.favorites_hidden))
                    add(GroupManageRow("recent", strings.t("explore.recent"), strings.t("explore.systemGroup"), system = true, hidden = view.recent_hidden))
                    view.groups.forEach { group ->
                        add(GroupManageRow(group.id, group.name, strings.t("explore.siteCount", mapOf("n" to group.sites.size.toString())), system = false, hidden = group.hidden))
                    }
                },
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
                            "favorite" -> if (bookmarked) item.copy(label = strings.t("explore.removeFromFavorites")) else item
                            "disconnect" -> item.takeIf { connected }
                            else -> item
                        }
                    },
                )
            } ?: fallback.siteMenuSheet,
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

    fun customGroups(view: ExploreView): List<GroupModel> = view.groups
        .filter { !it.hidden }
        .map { group ->
            GroupModel(
                id = group.id,
                title = group.name,
                kind = GroupKind.Custom,
                action = GroupAction.Menu,
                sites = group.sites.map { tileOf(it) },
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
