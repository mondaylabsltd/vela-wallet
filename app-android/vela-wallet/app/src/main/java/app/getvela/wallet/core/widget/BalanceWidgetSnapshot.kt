package app.getvela.wallet.core.widget

import android.content.Context
import app.getvela.wallet.core.format.RelativeTime
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings
import app.getvela.wallet.feature.wallet.BalanceModel
import app.getvela.wallet.feature.wallet.BalanceStateKind
import app.getvela.wallet.feature.wallet.WalletLive

/**
 * What the home-screen widget shows (native parity Phase 3): the home's own
 * total as the person last saw it — never a figure of the widget's making.
 *
 * The widget does no network and runs no balance read. The wallet route
 * writes this whenever its hero settles on a figure (or on the mask), with
 * the core's own time of that read (`BalanceView.last_refreshed_at_ms`), so
 * the widget can say honestly how old it is ("Updated 2h"). A hero that is
 * still reading, or the testnet space's "no value", writes nothing: the last
 * real figure stands, with its age; so does a figure the core has not
 * dated yet (the cached rows a cold start draws first). Hidden on the home is
 * hidden here.
 */
data class BalanceWidgetSnapshot(
    /** "$1,383" — the hero's integer part, currency sign included; `null` when masked. */
    val integer: String?,
    /** "28" — the hero's cents; `null` when masked or none. */
    val decimals: String?,
    val decimalMark: String,
    /** The home's balance was hidden: the widget shows the mask. */
    val masked: Boolean,
    /** When the core last read the figure (ms since the epoch); `null` when it never said. */
    val atMs: Long?,
    /** The account it belongs to, by its name. */
    val account: String,
) {
    /** What the wallet route does with the hero it just drew. */
    sealed interface Write {
        data class Save(val snapshot: BalanceWidgetSnapshot) : Write

        /** Still reading, or nothing of value to show: the last figure stands. */
        data object Keep : Write
    }

    companion object {
        /**
         * The write for the home's hero [hero] — the board the person sees,
         * minus any chain filter. [testnet]: the testnet space's coins are
         * not a value, and the widget is the main space's.
         */
        fun of(hero: BalanceModel, atMs: Double?, account: String, testnet: Boolean): Write = when {
            testnet -> Write.Keep
            hero.state == BalanceStateKind.Hidden ->
                Write.Save(BalanceWidgetSnapshot(null, null, hero.decimalMark, masked = true, atMs = atMs?.toLong(), account = account))
            hero.state == BalanceStateKind.Loading || hero.state == BalanceStateKind.NoValue || hero.integer == null -> Write.Keep
            // A figure the core has not dated (the last visit's cached rows,
            // drawn before this visit's first read settles) never replaces a
            // dated one: the widget would lose the only honest thing it says.
            atMs == null -> Write.Keep
            else -> Write.Save(
                BalanceWidgetSnapshot(hero.integer, hero.decimals, hero.decimalMark, masked = false, atMs = atMs?.toLong(), account = account),
            )
        }

        private const val PREFS = "vela.widget"

        fun read(context: Context): BalanceWidgetSnapshot? {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            if (!prefs.getBoolean("present", false)) return null
            return BalanceWidgetSnapshot(
                integer = prefs.getString("integer", null),
                decimals = prefs.getString("decimals", null),
                decimalMark = prefs.getString("decimalMark", ".") ?: ".",
                masked = prefs.getBoolean("masked", false),
                atMs = prefs.getLong("atMs", -1L).takeIf { it >= 0 },
                account = prefs.getString("account", "").orEmpty(),
            )
        }

        /** Whether anything changed — the widget is redrawn only then. */
        fun save(context: Context, snapshot: BalanceWidgetSnapshot): Boolean {
            if (read(context) == snapshot) return false
            context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit()
                .putBoolean("present", true)
                .putString("integer", snapshot.integer)
                .putString("decimals", snapshot.decimals)
                .putString("decimalMark", snapshot.decimalMark)
                .putBoolean("masked", snapshot.masked)
                .putLong("atMs", snapshot.atMs ?: -1L)
                .putString("account", snapshot.account)
                .apply()
            return true
        }

        /** No wallet on this device any more (signed out, erased): the widget forgets the figure. */
        fun clear(context: Context): Boolean {
            val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            if (!prefs.getBoolean("present", false)) return false
            prefs.edit().clear().apply()
            return true
        }
    }
}

/** The widget's words and figure, drawn from a snapshot — JVM-testable. */
data class BalanceWidgetModel(
    val label: String,
    /** The figure's integer part, or the mask; `null` when there is no wallet yet. */
    val integer: String?,
    /** ".28", decimal mark included; `null` when none. */
    val decimals: String?,
    /** "Updated 2h · Main" — how old the figure is, and whose. */
    val footnote: String?,
) {
    companion object {
        fun of(snapshot: BalanceWidgetSnapshot?, strings: VelaStrings, nowMs: Long): BalanceWidgetModel {
            val label = strings.t(I18nKeys.Wallet.TOTAL_BALANCE)
            if (snapshot == null) return BalanceWidgetModel(label, null, null, null)
            val updated = snapshot.atMs?.let { at ->
                strings.t(I18nKeys.Wallet.LAST_UPDATED, mapOf("ago" to RelativeTime.ago(at.toDouble(), nowMs, strings)))
            }
            val footnote = listOfNotNull(updated, snapshot.account.takeIf { it.isNotBlank() }).joinToString(" · ").ifEmpty { null }
            return if (snapshot.masked) {
                BalanceWidgetModel(label, WalletLive.BALANCE_MASK, null, footnote)
            } else {
                BalanceWidgetModel(label, snapshot.integer, snapshot.decimals?.let { snapshot.decimalMark + it }, footnote)
            }
        }
    }
}
