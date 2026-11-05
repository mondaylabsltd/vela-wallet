package app.getvela.wallet.core.platform

/**
 * r14-2: whether the intent an activity was created with is new — to be
 * acted on (a link, a tapped notification, a shortcut, shared text) — or
 * one already acted on.
 *
 * Android hands an activity its STARTING intent again whenever it recreates
 * it: a configuration change, a process killed and its task brought back
 * (the new intent of that launch then arrives through `onNewIntent`), or the
 * task reopened from Recents (`FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY`). Only a
 * first creation's intent is the person's ask now.
 */
object LaunchIntent {
    /** `Intent.FLAG_ACTIVITY_LAUNCHED_FROM_HISTORY` (no Android types here: tested on the JVM). */
    const val LAUNCHED_FROM_HISTORY = 0x00100000

    fun firstDelivery(noSavedState: Boolean, flags: Int): Boolean =
        noSavedState && (flags and LAUNCHED_FROM_HISTORY) == 0
}
