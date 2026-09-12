package app.getvela.wallet.dev

import android.app.Application

/**
 * Debug: the door exists. Phase 2 (spec 043 US0) replaces this stub with the
 * real provider — the fixture signer over `vela-dev-fixtures-uniffi`, the
 * badge, the persisted flag. Until then the debug build behaves as release.
 */
object ParallelSpaceBinding {
    @Suppress("UNUSED_PARAMETER")
    fun install(app: Application) = Unit
}
