package app.getvela.wallet.core.platform

import app.getvela.wallet.core.data.KeyValueStore
import uniffi.vela_core_uniffi.launchAnimationDue
import uniffi.vela_core_uniffi.launchAnimationStorageKey

/**
 * When the launch animation plays: the core's rule
 * (`vela_core::launch::launch_animation_due`), the one the web and the desktop
 * follow — at most once a week, never when [decide]'s `disabled` (the
 * instrumented tests' deterministic skip). Android used to play it on every
 * cold start.
 *
 * The time it last played lives in the app's store under the core's key
 * (`launchAnimationStorageKey`, the web's `vela.launch.played`), as epoch
 * milliseconds. Something unreadable there is "never played": a corrupt write
 * replays a decoration, it never silences it for good.
 *
 * Both calls are store and core work: off the main thread (MainActivity asks
 * while the splash is still up, after the core has loaded on a worker).
 */
object LaunchGate {

    /** Is the animation due now? */
    suspend fun decide(store: KeyValueStore, nowMs: Long, disabled: Boolean): Boolean {
        val stored = runCatching { store.read(launchAnimationStorageKey()) }.getOrNull()
        return launchAnimationDue(lastPlayedMs(stored), nowMs.toDouble(), disabled)
    }

    /** It is playing now: the week starts here. */
    suspend fun played(store: KeyValueStore, nowMs: Long) {
        runCatching { store.write(launchAnimationStorageKey(), nowMs.toString()) }
    }

    /** The stored text as the core reads it: finite epoch ms, else `null` (never played). */
    fun lastPlayedMs(stored: String?): Double? = stored?.trim()?.toDoubleOrNull()?.takeIf { it.isFinite() }
}
