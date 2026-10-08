package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.SerialName
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import uniffi.vela_core_uniffi.browserEnginePlan
import uniffi.vela_core_uniffi.browserLitTab
import uniffi.vela_core_uniffi.browserOpenTarget
import uniffi.vela_core_uniffi.browserWaitingTab
import uniffi.vela_core_uniffi.exploreLanding
import uniffi.vela_core_uniffi.exploreTabsClosedBy

/*
 * Which tabs keep a live engine (spec 099 R2, FR-004) — the core's
 * `browser_tabs::plan_engines`, mirrored for `browserEnginePlan`. The rule is
 * the core's alone: the selected tab and every busy tab are kept, the rest
 * most-recently-used first up to the cap (one under memory pressure), and
 * nothing here ever wakes a tab. This file only gathers the facts and reads
 * the answer.
 */

/** What the plan decides from. Ids are explore tab ids. */
@Serializable
data class EngineInput(
    /** Every open tab, in strip order (`ExploreView.tabs`). */
    val tabs: List<String>,
    val selected: String? = null,
    /** Most recently used first (`ExploreView.recent_tabs`). */
    val recent: List<String> = emptyList(),
    /** Tabs the browser machine reports busy (`DbrTabView.busy`). */
    val busy: List<String> = emptyList(),
    /** Tabs whose engine exists now — the shell's own fact. */
    val live: List<String> = emptyList(),
    /** The system asked the app to free memory. */
    val pressure: Boolean = false,
)

/** The engines to let go of now. */
@Serializable
data class EnginePlan(val suspend: List<String> = emptyList())

/*
 * Spec 099 navigation — where 探索 lands and where an opened site goes, the
 * core's `browser_tabs::{explore_landing, open_target, lit_tab, waiting_tab}`.
 * The rules are the core's alone; these are their words, transcribed.
 */

/** What brought 探索 up (`ExploreEntry`) — the landing's question. */
@Serializable
enum class ExploreEntry {
    /** 探索 chosen from another section — the first tap after a launch included. */
    @SerialName("section") Section,

    /** 探索 chosen again while it is up: the way back to its home from a page. */
    @SerialName("reselect") Reselect,

    /** A page opened from outside — a deep link, a scan, the external-page sheet, a launch URL. */
    @SerialName("page_opened") PageOpened,
}

/** What 探索 shows on entry (`ExploreLanding`). */
@Serializable
sealed class ExploreLanding {
    /** The home: search, the resume rows, favourites, recents. Nothing closed, nothing loaded. */
    @Serializable
    @SerialName("home")
    data object Home : ExploreLanding()

    /** This tab's page — always a strip tab that has one. */
    @Serializable
    @SerialName("tab")
    data class Tab(val id: String) : ExploreLanding()
}

/** How an open was asked for (`ExploreOpenKind`). */
@Serializable
enum class ExploreOpenKind {
    /** Typed into a bar, or handed in from outside: it names a page. */
    @SerialName("address") Address,

    /** A favourite or a recent dApp picked from the home: it names a site. */
    @SerialName("site") Site,
}

/** Where an open goes (`ExploreOpenTarget`). */
@Serializable
sealed class ExploreOpenTarget {
    /** Load it in this tab: the page on screen, or a start-page tab's first page. */
    @Serializable
    @SerialName("load")
    data class Load(val id: String) : ExploreOpenTarget()

    /** This tab is already on the site: show it as it was left, no load. */
    @Serializable
    @SerialName("resume")
    data class Resume(val id: String) : ExploreOpenTarget()

    /** A new tab onto the address: nothing open is replaced. */
    @Serializable
    @SerialName("new_tab")
    data object NewTab : ExploreOpenTarget()
}

object BrowserTabs {
    /** The plan's input from the two machines' views and the engines this shell holds. */
    fun input(explore: ExploreView, dapp: DbrView, live: Collection<String>, pressure: Boolean = false): EngineInput =
        EngineInput(
            tabs = explore.tabs.map { it.id },
            selected = explore.selected_tab,
            recent = explore.recent_tabs,
            busy = dapp.tabs.filter { it.busy }.map { it.tab },
            live = live.toList(),
            pressure = pressure,
        )

    /** The core's answer; nothing suspended when it cannot read the input (a core fault, never a guess). */
    fun plan(input: EngineInput): EnginePlan {
        val json = browserEnginePlan(Wire.json.encodeToString(EngineInput.serializer(), input)) ?: return EnginePlan()
        return runCatching { Wire.json.decodeFromString(EnginePlan.serializer(), json) }.getOrDefault(EnginePlan())
    }

    /**
     * A suspended tab's page is gone but the tab stays (the desktop's
     * `page_gone_events`): to the browser machine its document navigated
     * away to nothing, so whatever it had is settled as a page that left, and
     * its next document is a new one that says hello.
     */
    fun pageGone(tab: String, nowMs: Double): List<DbrEvent> = listOf(
        DbrEvent.NavigationStarted(tab = tab, url = BLANK, now_ms = nowMs),
        DbrEvent.LoadFinished(tab = tab, url = BLANK, now_ms = nowMs),
    )

    /**
     * Which tabs a batch close takes (spec 099) — the core's
     * `explore_sites::tabs_closed_by` over the strip as the explore view has
     * it: every tab but `keep`, every tab right of `of`, or every tab. This
     * shell never decides it; empty when the core cannot read the input.
     */
    fun closedBy(tabs: List<ExploreTab>, scope: TabCloseScope): List<String> {
        val json = exploreTabsClosedBy(
            Wire.json.encodeToString(ListSerializer(ExploreTab.serializer()), tabs),
            Wire.json.encodeToString(TabCloseScope.serializer(), scope),
        ) ?: return emptyList()
        return runCatching { Wire.json.decodeFromString(ListSerializer(String.serializer()), json) }.getOrDefault(emptyList())
    }

    // -- spec 099 navigation ------------------------------------------------------

    private fun viewJson(view: ExploreView): String = Wire.json.encodeToString(ExploreView.serializer(), view)

    private fun word(entry: ExploreEntry): String = Wire.json.encodeToString(ExploreEntry.serializer(), entry).trim('"')

    private fun word(kind: ExploreOpenKind): String = Wire.json.encodeToString(ExploreOpenKind.serializer(), kind).trim('"')

    /**
     * Where 探索 lands (`explore_landing`): its home for an entry from
     * another section or a re-tap, the page for a page opened from outside,
     * and the tab of a request that waits on the person whatever the entry.
     * The home when the core cannot read the input — the landing that closes
     * and loads nothing.
     */
    fun landing(view: ExploreView, entry: ExploreEntry, waiting: String?): ExploreLanding {
        val json = exploreLanding(viewJson(view), word(entry), waiting) ?: return ExploreLanding.Home
        return runCatching { Wire.json.decodeFromString(ExploreLanding.serializer(), json) }.getOrDefault(ExploreLanding.Home)
    }

    /**
     * Where an open goes (`browser_open_target`). [shown] is the tab whose
     * page is in the view; [onPage] whether that page is on screen and its
     * address bar asked — false from the home and for every open from
     * outside. A new tab when the core cannot read the input: never a load
     * over a live page.
     */
    fun openTarget(view: ExploreView, shown: String?, onPage: Boolean, url: String, kind: ExploreOpenKind): ExploreOpenTarget {
        val json = browserOpenTarget(viewJson(view), shown, onPage, url, word(kind)) ?: return ExploreOpenTarget.NewTab
        return runCatching { Wire.json.decodeFromString(ExploreOpenTarget.serializer(), json) }.getOrDefault(ExploreOpenTarget.NewTab)
    }

    /** The tab that is "this tab" (`browser_lit_tab`): the shown one on a page; over the home only a selected start-page tab. */
    fun litTab(view: ExploreView, shown: String?, onPage: Boolean): String? = browserLitTab(viewJson(view), shown, onPage)

    /**
     * The tab whose request is in front of the person (`browser_waiting_tab`):
     * the consent's, else the signature's, else the add-network sheet's.
     * Read from the core's own JSON of the browser view when the shell has
     * it (a mirror may leave fields out), else from this shell's copy.
     */
    fun waitingTab(dappViewJson: String?, dapp: DbrView): String? =
        browserWaitingTab(dappViewJson ?: Wire.json.encodeToString(DbrView.serializer(), dapp))

    private const val BLANK = "about:blank"
}
