package app.getvela.wallet.dev

import android.app.Activity

/**
 * Debug: the harness launch inputs (`vela.serviceEndpoints`, `vela.rpcOverrides`,
 * `vela.peerconnectRelay`) are honoured — see [ServiceLaunchInputs].
 */
object LaunchInputsBinding {
    /** `true` when the process is restarting to read what was written. */
    fun apply(activity: Activity): Boolean = ServiceLaunchInputs.apply(activity)
}
