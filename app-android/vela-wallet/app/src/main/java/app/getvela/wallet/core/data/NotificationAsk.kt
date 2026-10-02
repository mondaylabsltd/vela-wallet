package app.getvela.wallet.core.data

/**
 * The notification permission is asked for at most ONCE per install (087 F14).
 *
 * It is asked in context — the first time a send reaches its receipt, never at
 * launch (research D6) — and a refusal degrades to the in-app receipt. The ask
 * used to run whenever the send flow sat on its receipt without the permission:
 * on every later send, and again on every recreation of the screen (a language
 * switch recreates the activity, and the dialog came straight back). Android
 * itself would keep showing it until the second refusal; a person who said no
 * has answered.
 *
 * iOS needs no mark of its own: `UNUserNotificationCenter` asks only while the
 * status is `notDetermined`, which the first answer ends for good.
 */
object NotificationAsk {
    /**
     * Written once, when the ask is shown. Survives sign-out; an erase — a
     * fresh start — clears it with every other `vela.` key.
     */
    const val KEY = "vela.notificationsAsked"

    /**
     * Whether this caller may show the system's ask: `true` exactly once per
     * install. The mark is written BEFORE the dialog, so a dialog cut short by
     * a recreation does not come back; a store that refuses the write answers
     * `false`, because asking on every receipt is the defect this ends.
     */
    suspend fun claim(store: KeyValueStore): Boolean {
        if (store.read(KEY) != null) return false
        return store.write(KEY, "1")
    }
}
