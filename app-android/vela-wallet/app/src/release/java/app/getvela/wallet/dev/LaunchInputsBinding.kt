package app.getvela.wallet.dev

import android.app.Activity

/**
 * Release: no harness inputs. `MainActivity` is exported, so any installed app
 * may start it with extras; a release build must never let one re-point its
 * endpoints or RPCs. The debug implementation (`ServiceLaunchInputs`) is not in
 * this source set, and `scripts/check-release-apk.sh` refuses a package that
 * carries it.
 */
object LaunchInputsBinding {
    @Suppress("UNUSED_PARAMETER", "FunctionOnlyReturningConstant")
    fun apply(activity: Activity): Boolean = false
}
