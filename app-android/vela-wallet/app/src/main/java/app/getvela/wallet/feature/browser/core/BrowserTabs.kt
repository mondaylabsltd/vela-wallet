package app.getvela.wallet.feature.browser.core

import app.getvela.wallet.core.crux.Wire
import kotlinx.serialization.Serializable
import kotlinx.serialization.builtins.ListSerializer
import kotlinx.serialization.builtins.serializer
import uniffi.vela_core_uniffi.browserEnginePlan
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

    private const val BLANK = "about:blank"
}
