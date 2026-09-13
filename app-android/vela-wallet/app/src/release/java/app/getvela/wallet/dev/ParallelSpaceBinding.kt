package app.getvela.wallet.dev

import android.app.Application

/**
 * Release: no door, no keys. `ParallelSpaceHook` stays uninstalled, every
 * query answers "not active", and the fixture cdylib is not in the package.
 */
object ParallelSpaceBinding {
    @Suppress("UNUSED_PARAMETER")
    fun install(app: Application) = Unit
}
