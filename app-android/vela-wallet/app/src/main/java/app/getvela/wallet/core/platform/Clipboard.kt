package app.getvela.wallet.core.platform

import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import app.getvela.wallet.core.diagnostics.VelaLog

/**
 * The one clipboard writer (spec 048). Before it, every copy control in the
 * flows and the contacts only flipped its tick — the person found out at the
 * paste. A control shows its copied state only when this returned true.
 */
object Clipboard {
    fun copy(context: Context, label: String, text: String): Boolean =
        runCatching {
            val manager = context.getSystemService(ClipboardManager::class.java) ?: return false
            manager.setPrimaryClip(ClipData.newPlainText(label, text))
            VelaLog.event("clipboard", "copied", "label" to label, "chars" to text.length)
            true
        }.getOrElse { error ->
            VelaLog.failure("clipboard", "copy failed", error)
            false
        }
}
