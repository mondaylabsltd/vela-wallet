package app.getvela.wallet.navigation

import androidx.compose.runtime.saveable.Saver
import androidx.compose.runtime.saveable.listSaver
import app.getvela.wallet.feature.browser.core.BrowserTabs
import app.getvela.wallet.feature.browser.core.ExploreEntry
import app.getvela.wallet.feature.browser.core.ExploreLanding
import app.getvela.wallet.feature.browser.core.ExploreView

/**
 * One visit to 探索 as the host keeps it (spec 099 navigation): its number,
 * what brought 探索 up (the core's `explore_landing` question), the tab whose
 * request waited on the person as it was entered, and — once the landing has
 * been acted on — where it landed.
 *
 * Kept whole and saved whole ([Saver]). A route pushed over the wallet
 * (设置, 通讯录) takes the browser's composition down with it, and Back
 * brings back the SAME visit: it must not be landed again. Before this the
 * settled landing lived in a plain `remember` while the waiting tab was kept
 * for the whole visit, so answering W's consent, switching to X, then 设置 and
 * Back ran the landing again from the saved waiting tab and selected W —
 * the person was pulled back to a tab they had already answered and left.
 * Settling now drops the waiting tab, and the settled landing survives.
 */
data class ExploreVisit(
    /** Counts visits: a new one (another section, a re-tap, a page from outside) starts the screen from its landing. */
    val number: Int = 0,
    val entry: ExploreEntry = ExploreEntry.Section,
    /** `browser_waiting_tab` as 探索 was entered; `null` once the landing is settled — it was asked once. */
    val waiting: String? = null,
    /** Where this visit landed, once acted on; `null` while the core's view is still settling. */
    val settled: ExploreLanding? = null,
) {
    /** A new visit: [entry] and the tab [waiting] on the person now, nothing settled. */
    fun entered(entry: ExploreEntry, waiting: String?): ExploreVisit = ExploreVisit(number + 1, entry, waiting, settled = null)

    /** Where this visit shows: the settled landing, else the core's for [view] as it is now. */
    fun landing(view: ExploreView): ExploreLanding = settled ?: BrowserTabs.landing(view, entry, waiting)

    /**
     * The visit settled on [view] — the landing to act on is its [settled] —
     * or `null` when it already settled, so there is nothing to act on again
     * (a route pushed over the wallet and Back, a configuration change).
     */
    fun settle(view: ExploreView): ExploreVisit? =
        if (settled != null) null else copy(waiting = null, settled = BrowserTabs.landing(view, entry, waiting))

    companion object {
        private const val UNSETTLED = ""
        private const val HOME = "home"
        private const val TAB = "tab:"

        private fun word(landing: ExploreLanding?): String = when (landing) {
            null -> UNSETTLED
            ExploreLanding.Home -> HOME
            is ExploreLanding.Tab -> TAB + landing.id
        }

        private fun landingOf(word: String): ExploreLanding? = when {
            word == HOME -> ExploreLanding.Home
            word.startsWith(TAB) -> ExploreLanding.Tab(word.removePrefix(TAB))
            else -> null
        }

        /** Bundle-safe: an Int and three Strings ("" for none). */
        val Saver: Saver<ExploreVisit, Any> = listSaver(
            save = { listOf(it.number, it.entry.name, it.waiting.orEmpty(), word(it.settled)) },
            restore = { saved ->
                ExploreVisit(
                    number = saved[0] as Int,
                    entry = ExploreEntry.entries.firstOrNull { it.name == saved[1] } ?: ExploreEntry.Section,
                    waiting = (saved[2] as String).ifEmpty { null },
                    settled = landingOf(saved[3] as String),
                )
            },
        )
    }
}
