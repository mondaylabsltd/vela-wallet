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
    )

    fun home(
        fallback: ExploreScreenModel,
        view: ExploreView,
        history: BhistView,
        engine: EngineState?,
        strings: VelaStrings,
        /** The core's view of the tab in front (spec 070) — connection, chain, lock, crash. */
        tab: DbrTabView? = null,
        identity: Identity = Identity(),
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
                site = open.url?.let { url -> SiteModel(id = url, name = open.title, host = open.host, letter = letterOf(open.host), tint = tintOf(open.host)) },
                selected = view.selected_tab == open.id,
                startPage = open.url == null,
            )
        }
        // The page's origin, or — when its renderer died and there is no engine —
        // the core's word for what the tab shows.
        val shownOrigin = engine?.origin ?: tab?.origin
        val bookmarked = shownOrigin != null && view.favorites.any { it.origin == shownOrigin }
        return fallback.copy(
            tabCountLabel = tabs.size.takeIf { it > 0 }?.toString(),
            // No CTA: there is no curated list behind "browse", and a button
            // that goes nowhere is worse than none (spec 070).
            empty = if (populated) null else ExploreEmptyCopy(strings.t("explore.startTitle"), strings.t("explore.startHint"), ""),
            favorites = if (favorites.isEmpty() || view.favorites_hidden) {
                null
            } else {
                FavoritesSection(
                    title = strings.t("explore.favorites"),
                    action = strings.t("explore.edit"),
                    tiles = favorites.map { TileModel.Site(it) } + TileModel.Add(strings.t("explore.add")),
                )
            },
            groups = groups,
            browser = fallback.browser.copy(
                url = engine?.url ?: fallback.browser.url,
                host = engine?.host?.ifBlank { null } ?: tab?.origin?.substringAfter("://") ?: fallback.browser.host,
                secure = tab?.secure ?: (engine == null && fallback.browser.secure),
                canBack = engine?.canBack ?: false,
                canForward = engine?.canForward ?: false,
                bookmarked = bookmarked,
                tabCount = tabs.size,
                connected = connected,
                accountName = identity.accountName.ifBlank { fallback.browser.accountName },
                accountSeed = identity.accountAddress.ifBlank { fallback.browser.accountSeed },
                loading = engine?.loading ?: false,
                progress = engine?.progress ?: 100,
                failed = engine?.failed ?: false,
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
                    site = SiteModel(id = e.url, name = e.title.ifBlank { e.host }, host = e.host, letter = letterOf(e.host), tint = tintOf(e.host)),
                    statusLine = if (secure) strings.t("explore.secureSite") else strings.t("connect.browser.a11yInsecure"),
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
            site = SiteModel(id = e.url, name = e.title.ifBlank { e.host }, host = e.host, letter = letterOf(e.host), tint = tintOf(e.host)),
            statusLine = listOfNotNull(
                if (secure) strings.t("explore.secureSite") else strings.t("connect.browser.a11yInsecure"),
                strings.t("explore.connectedTag").takeIf { connected },
            ).joinToString(" · "),
            accountName = identity.accountName.ifBlank { fallback.accountName },
            accountAddress = shortAddress(tab?.connected_address ?: identity.accountAddress),
            accountSeed = tab?.connected_address ?: identity.accountAddress.ifBlank { fallback.accountSeed },
            networkName = identity.chainName.ifBlank { fallback.networkName },
            networkDot = if (identity.chainDot == Color.Unspecified) fallback.networkDot else identity.chainDot,
            secure = secure,
        )
    }

    /**
     * The consent card (spec 044 FR-006): the drawn connection sheet in its
     * not-yet-connected form. The origin is the CORE's (`DpermConsentView`);
     * the words are `connect.browser.*`; the site's own name is not asked.
     */
    fun consent(fallback: ConnectionModel, consent: DbrConsentView, strings: VelaStrings, identity: Identity): ConnectionModel {
        val host = consent.origin.substringAfter("://").substringBefore('/')
        val secure = consent.origin.startsWith("https://")
        return fallback.copy(
            title = strings.t("connect.browser.title", mapOf("host" to host)),
            site = SiteModel(id = consent.origin, name = host, host = host, letter = letterOf(host), tint = tintOf(host)),
            statusLine = if (secure) strings.t("explore.secureSite") else strings.t("connect.browser.a11yInsecure"),
            secure = secure,
            accountName = identity.accountName.ifBlank { fallback.accountName },
            accountAddress = shortAddress(identity.accountAddress),
            accountSeed = identity.accountAddress.ifBlank { fallback.accountSeed },
            networkName = identity.chainName.ifBlank { fallback.networkName },
            networkDot = if (identity.chainDot == Color.Unspecified) fallback.networkDot else identity.chainDot,
            explainer = strings.t("connect.browser.body"),
            disconnect = strings.t("connect.browser.connect"),
            footnote = strings.t("connect.browser.cancel"),
        )
    }

    fun shortAddress(address: String): String =
        if (address.length > 12) "${address.take(6)}…${address.takeLast(4)}" else address

    /** A recent row shows the title over the host it actually is. */
    fun siteOf(entry: BhistEntry): SiteModel = SiteModel(
        id = entry.url,
        name = entry.title.trim().ifEmpty { entry.host },
        host = entry.host,
        letter = letterOf(entry.host),
        tint = tintOf(entry.host),
        subtitle = entry.host,
    )

    fun tileOf(site: ExploreSite): SiteModel = SiteModel(
        id = site.url,
        name = site.name,
        host = site.host,
        letter = letterOf(site.host),
        tint = tintOf(site.host),
        subtitle = site.host,
    )

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

    /** The first letter or digit of the host, upper-cased; `?` for none. */
    fun letterOf(host: String): String =
        host.firstOrNull { it.isLetterOrDigit() && it.code < 128 }?.uppercaseChar()?.toString() ?: "?"

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
