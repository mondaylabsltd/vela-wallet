package app.getvela.wallet.core.platform

import android.content.Context
import android.content.Intent
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import app.getvela.wallet.MainActivity
import app.getvela.wallet.R
import app.getvela.wallet.core.diagnostics.VelaLog
import app.getvela.wallet.core.i18n.I18nKeys
import app.getvela.wallet.core.i18n.VelaStrings

/**
 * The launcher's long-press shortcuts — Send, Receive, Scan (native parity
 * Phase 3), the home dock's three actions in its order.
 *
 * Dynamic, not `shortcuts.xml`: the words are the shared corpus's
 * (`componentsUi.dock.*`, the dock's own labels) in the app's language, which
 * only the i18n runtime knows, so they are published at start and again
 * whenever the language changes. Each opens MainActivity with its
 * [LaunchEntry.Flow] action; the wallet route enters the flow once a wallet
 * is in front (and an app with no wallet yet simply opens).
 */
object LauncherShortcuts {
    fun publish(context: Context, strings: VelaStrings) {
        val shortcuts = LaunchEntry.Flow.entries.mapIndexed { rank, flow ->
            val label = strings.t(labelKey(flow))
            ShortcutInfoCompat.Builder(context, flow.shortcutId)
                .setShortLabel(label)
                .setLongLabel(label)
                .setIcon(IconCompat.createWithResource(context, iconOf(flow)))
                .setRank(rank)
                .setIntent(
                    Intent(context, MainActivity::class.java)
                        .setAction(flow.action)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP),
                )
                .build()
        }
        val published = runCatching { ShortcutManagerCompat.setDynamicShortcuts(context, shortcuts) }.getOrDefault(false)
        VelaLog.event("shortcuts", "published", "count" to shortcuts.size, "ok" to published, "language" to strings.language)
    }

    /** The dock's own label for the action ("Send" / "转账"…). */
    fun labelKey(flow: LaunchEntry.Flow): String = when (flow) {
        LaunchEntry.Flow.Send -> I18nKeys.Wallet.ACTION_SEND
        LaunchEntry.Flow.Receive -> I18nKeys.Wallet.ACTION_RECEIVE
        LaunchEntry.Flow.Scan -> I18nKeys.Wallet.ACTION_SCAN
    }

    private fun iconOf(flow: LaunchEntry.Flow): Int = when (flow) {
        LaunchEntry.Flow.Send -> R.drawable.ic_shortcut_send
        LaunchEntry.Flow.Receive -> R.drawable.ic_shortcut_receive
        LaunchEntry.Flow.Scan -> R.drawable.ic_shortcut_scan
    }
}
